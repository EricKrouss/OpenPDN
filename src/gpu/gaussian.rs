//! Direct2D Gaussian optimization plans and paired bilinear taps recovered from
//! the native 10.0.19041.329 implementation. No native code is linked at runtime.
use super::*;

#[derive(Clone, Debug)]
pub struct GaussianPlan {
    pub pre: f32,
    pub post: f32,
    pub cubic: bool,
    pub taps: Vec<[f32; 4]>,
}
impl GaussianPlan {
    pub fn new(sigma: f32, quality: u32) -> Result<Self> {
        ensure!(
            sigma.is_finite() && (0. ..=100.).contains(&sigma),
            "Gaussian sigma must be 0..100"
        );
        ensure!(
            (1..=3).contains(&quality),
            "Optimized Gaussian quality must be 1..3"
        );
        let (factor, threshold, support) = if quality == 3 {
            (12., 2., 3.)
        } else {
            (8., 1.5, 2.5)
        };
        let pre = (factor / (sigma + 6.)).clamp(0.1, 1.);
        let post = if sigma > threshold { 0.5 } else { 1. };
        let effective = sigma * pre;
        let count = (effective * support).ceil().max(1.).mul_add(0.5, 0.).ceil() as usize;
        let mut taps = Vec::with_capacity(count);
        let gaussian = |x: f32| {
            if effective == 0. {
                f32::from(x == 0.)
            } else {
                (x.powf(2.) / (effective.powf(2.) * -2.)).exp()
            }
        };
        let mut sum = 0.;
        for i in 0..count {
            let x = 2. * i as f32;
            let a = gaussian(x) * if i == 0 { 0.5 } else { 1. };
            let b = gaussian(x + 1.);
            let weight = a + b;
            taps.push([x + b / weight, 0., weight, 0.]);
            sum += weight;
        }
        sum *= 2.;
        for tap in &mut taps {
            tap[2] /= sum;
        }
        Ok(Self {
            pre,
            post,
            cubic: quality != 1,
            taps,
        })
    }
    pub fn matrices(&self) -> [[f32; 6]; 2] {
        let scale = 1. / (self.pre * self.post);
        let offset = -(0.5 - f64::from(self.post) * 0.5) as f32 * scale;
        [
            [self.pre, 0., 0., self.pre, 0., 0.],
            [scale, 0., 0., scale, offset, offset],
        ]
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Parameters {
    dims: [u32; 4],
    origins: [i32; 4],
    stage: [u32; 4],
    geometry: [f32; 4],
    reconstruction: [f32; 4],
}
impl Gpu {
    pub(super) fn optimized_gaussian(
        &self,
        doc: &Document,
        values: [f32; 8],
    ) -> Result<Vec<Pixel>> {
        let plan = GaussianPlan::new(values[0] / 3., values[2] as u32)?;
        if values[0] == 0. {
            return Ok(doc.layers[doc.active].pixels.as_ref().clone());
        }
        // Keep a halo outside the document. Mirroring a truncated reduced image
        // would shift fractional edges; sample the original mirror extension.
        let pad = plan.taps.last().unwrap()[0].ceil() as u32 + 12;
        let reduced = [doc.width, doc.height].map(|n| (n as f32 * plan.pre).ceil() as u32);
        let final_size =
            [doc.width, doc.height].map(|n| (n as f32 * plan.pre * plan.post).ceil() as u32);
        let pre_size = [reduced[0] + pad * 2, reduced[1] + pad * 2];
        let horizontal = [final_size[0] + 8, pre_size[1]];
        let vertical = [final_size[0] + 8, final_size[1] + 8];
        let sizes = [pre_size, horizontal, vertical];
        for size in sizes {
            let bytes = u64::from(size[0]) * u64::from(size[1]) * 16;
            ensure!(
                bytes <= self.device.limits().max_storage_buffer_binding_size
                    && bytes <= self.device.limits().max_buffer_size,
                "Gaussian intermediate exceeds GPU storage limit"
            );
            ensure!(
                size[0].div_ceil(16) <= self.device.limits().max_compute_workgroups_per_dimension
                    && size[1].div_ceil(16)
                        <= self.device.limits().max_compute_workgroups_per_dimension,
                "Gaussian intermediate exceeds GPU dispatch limits"
            );
        }
        let packed = self.buffer(
            "Gaussian original",
            bytemuck::cast_slice(doc.layers[doc.active].pixels.as_slice()),
            wgpu::BufferUsages::STORAGE,
        );
        let selected: Vec<u32> = (0..doc.width as usize * doc.height as usize)
            .map(|i| u32::from(doc.selected(i)))
            .collect();
        let selection = self.buffer(
            "Gaussian selection",
            bytemuck::cast_slice(&selected),
            wgpu::BufferUsages::STORAGE,
        );
        let taps = self.buffer(
            "Gaussian paired taps",
            bytemuck::cast_slice(&plan.taps),
            wgpu::BufferUsages::STORAGE,
        );
        let buffers = sizes.map(|n| self.output_buffer(u64::from(n[0]) * u64::from(n[1]) * 16));
        let dummy = self.output_buffer(16);
        let output = self.output_buffer(u64::from(doc.width) * u64::from(doc.height) * 4);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let origins = [[-(pad as i32); 2], [-4, -(pad as i32)], [-4; 2]];
        for stage in 0..4 {
            let (input, destination, input_size, input_origin, output_size, output_origin) =
                if stage == 0 {
                    (
                        &dummy,
                        &buffers[0],
                        [doc.width, doc.height],
                        [0; 2],
                        pre_size,
                        origins[0],
                    )
                } else if stage == 3 {
                    (
                        &buffers[2],
                        &dummy,
                        vertical,
                        origins[2],
                        [doc.width, doc.height],
                        [0; 2],
                    )
                } else {
                    (
                        &buffers[stage - 1],
                        &buffers[stage],
                        sizes[stage - 1],
                        origins[stage - 1],
                        sizes[stage],
                        origins[stage],
                    )
                };
            let matrix = plan.matrices()[1];
            let parameters = Parameters {
                dims: [doc.width, doc.height, output_size[0], output_size[1]],
                origins: [
                    input_origin[0],
                    input_origin[1],
                    output_origin[0],
                    output_origin[1],
                ],
                stage: [
                    stage as u32,
                    u32::from(plan.cubic),
                    input_size[0],
                    input_size[1],
                ],
                geometry: [plan.pre, plan.post, 1. + values[1], 0.],
                reconstruction: [matrix[0], matrix[4], 0., 0.],
            };
            let uniform = self.buffer(
                "Gaussian parameters",
                bytemuck::bytes_of(&parameters),
                wgpu::BufferUsages::UNIFORM,
            );
            let entries: Vec<_> = [
                &packed,
                &selection,
                input,
                destination,
                &output,
                &taps,
                &uniform,
            ]
            .into_iter()
            .enumerate()
            .map(|(i, buffer)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: buffer.as_entire_binding(),
            })
            .collect();
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Gaussian stage"),
                layout: &self.gaussian.get_bind_group_layout(0),
                entries: &entries,
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.gaussian);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(output_size[0].div_ceil(16), output_size[1].div_ceil(16), 1);
        }
        self.queue.submit([encoder.finish()]);
        self.read_pixels(&output)
    }
}
