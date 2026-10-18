//! Separable, alpha-correct GPU resampling. Weights are prepared on the CPU;
//! both convolution passes and color conversion run on the rendering device.
use super::*;
use crate::document::pixel_count;
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resampler {
    Nearest,
    Linear,
    Cubic,
    Lanczos3,
    Fant,
}
impl Resampler {
    pub const ALL: [Self; 5] = [
        Self::Cubic,
        Self::Linear,
        Self::Lanczos3,
        Self::Fant,
        Self::Nearest,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Nearest => "Nearest Neighbor",
            Self::Linear => "Bilinear",
            Self::Cubic => "Bicubic",
            Self::Lanczos3 => "Lanczos 3",
            Self::Fant => "Fant (area)",
        }
    }
}
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Tap {
    index: u32,
    weight: f32,
}
fn weights(src: u32, dst: u32, method: Resampler) -> (Vec<[u32; 2]>, Vec<Tap>) {
    let ratio = f64::from(src) / f64::from(dst);
    let spread = ratio.max(1.);
    let mut spans = Vec::with_capacity(dst as usize);
    let mut taps = Vec::new();
    for i in 0..dst {
        let center = (f64::from(i) + 0.5) * ratio - 0.5;
        let start = taps.len();
        if method == Resampler::Nearest {
            taps.push(Tap {
                index: (center + 0.5).floor().clamp(0., f64::from(src - 1)) as u32,
                weight: 1.,
            });
        } else if method == Resampler::Fant {
            let left = f64::from(i) * ratio;
            let right = f64::from(i + 1) * ratio;
            for n in left.floor() as i32..right.ceil() as i32 {
                let w = right.min(f64::from(n + 1)) - left.max(f64::from(n));
                if w > 0. {
                    taps.push(Tap {
                        index: n.clamp(0, src as i32 - 1) as u32,
                        weight: (w / ratio) as f32,
                    });
                }
            }
        } else {
            let radius = match method {
                Resampler::Linear => 1.,
                Resampler::Cubic => 2.,
                _ => 3.,
            };
            let mut sum = 0.;
            let mut row = Vec::new();
            for n in
                (center - radius * spread).ceil() as i32..=(center + radius * spread).floor() as i32
            {
                let x = ((f64::from(n) - center) / spread).abs();
                let w = match method {
                    Resampler::Linear => (1. - x).max(0.),
                    Resampler::Cubic => {
                        if x < 1. {
                            (1.5 * x - 2.5) * x * x + 1.
                        } else if x < 2. {
                            ((-0.5 * x + 2.5) * x - 4.) * x + 2.
                        } else {
                            0.
                        }
                    }
                    _ => {
                        if x < 1e-12 {
                            1.
                        } else if x < 3. {
                            let p = std::f64::consts::PI * x;
                            p.sin() / p * (p / 3.).sin() / (p / 3.)
                        } else {
                            0.
                        }
                    }
                };
                if w != 0. {
                    row.push((n.clamp(0, src as i32 - 1) as u32, w));
                    sum += w;
                }
            }
            for (index, weight) in row {
                taps.push(Tap {
                    index,
                    weight: (weight / sum) as f32,
                });
            }
        }
        spans.push([start as u32, (taps.len() - start) as u32]);
    }
    (spans, taps)
}
impl Gpu {
    pub fn resize_document(
        &self,
        doc: &Document,
        width: u32,
        height: u32,
        method: Resampler,
        linear_light: bool,
    ) -> Result<Document> {
        self.check_document(doc)?;
        pixel_count(width, height)?;
        if (width, height) == (doc.width, doc.height) {
            return Ok(doc.clone());
        }
        let mut out = doc.clone();
        out.width = width;
        out.height = height;
        out.selection = None;
        let limits = self.device.limits();
        ensure!(
            width <= limits.max_texture_dimension_2d && height <= limits.max_texture_dimension_2d,
            "Image exceeds GPU texture dimensions"
        );
        let total = u64::from(width) * u64::from(height) * doc.layers.len() as u64 * 4;
        ensure!(
            total <= limits.max_storage_buffer_binding_size,
            "Resized layers exceed GPU storage limit"
        );
        // Choose the smaller intermediate, so mixed enlarge/shrink sizes remain bounded.
        let horizontal =
            u64::from(width) * u64::from(doc.height) <= u64::from(doc.width) * u64::from(height);
        let (iw, ih) = if horizontal {
            (width, doc.height)
        } else {
            (doc.width, height)
        };
        let intermediate_size = u64::from(iw) * u64::from(ih) * 16;
        ensure!(
            intermediate_size <= limits.max_storage_buffer_binding_size
                && intermediate_size <= limits.max_buffer_size,
            "Resize intermediate exceeds GPU storage limit"
        );
        let stage_sizes = if horizontal {
            [(doc.width, width), (doc.height, height)]
        } else {
            [(doc.height, height), (doc.width, width)]
        };
        let prepared: Vec<_> = stage_sizes
            .iter()
            .map(|&(src, dst)| {
                let (spans, taps) = weights(src, dst, method);
                (
                    self.buffer(
                        "Resize spans",
                        bytemuck::cast_slice(&spans),
                        wgpu::BufferUsages::STORAGE,
                    ),
                    self.buffer(
                        "Resize taps",
                        bytemuck::cast_slice(&taps),
                        wgpu::BufferUsages::STORAGE,
                    ),
                )
            })
            .collect();
        let intermediate = self.output_buffer(intermediate_size);
        let output = self.output_buffer(u64::from(width) * u64::from(height) * 4);
        for (layer, source) in out.layers.iter_mut().zip(&doc.layers) {
            let input = self.buffer(
                "Resize source",
                bytemuck::cast_slice(&source.pixels),
                wgpu::BufferUsages::STORAGE,
            );
            for (stage, prepared_stage) in prepared.iter().enumerate() {
                let (sw, sh, dw, dh, axis) = if stage == 0 {
                    (doc.width, doc.height, iw, ih, u32::from(!horizontal))
                } else {
                    (iw, ih, width, height, u32::from(horizontal))
                };
                let params = [
                    sw,
                    sh,
                    dw,
                    dh,
                    axis,
                    stage as u32,
                    u32::from(method == Resampler::Nearest),
                    u32::from(linear_light),
                ];
                let uniform = self.buffer(
                    "Resize parameters",
                    bytemuck::cast_slice(&params),
                    wgpu::BufferUsages::UNIFORM,
                );
                let bindings = [
                    &input,
                    &intermediate,
                    &output,
                    &prepared_stage.0,
                    &prepared_stage.1,
                    &uniform,
                ];
                let entries: Vec<_> = bindings
                    .iter()
                    .enumerate()
                    .map(|(binding, b)| wgpu::BindGroupEntry {
                        binding: binding as u32,
                        resource: b.as_entire_binding(),
                    })
                    .collect();
                let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Resize resources"),
                    layout: &self.resizer.get_bind_group_layout(0),
                    entries: &entries,
                });
                let mut encoder = self.device.create_command_encoder(&Default::default());
                {
                    let mut pass =
                        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
                    pass.set_pipeline(&self.resizer);
                    pass.set_bind_group(0, &group, &[]);
                    pass.dispatch_workgroups(dw.div_ceil(16), dh.div_ceil(16), 1);
                }
                self.queue.submit([encoder.finish()]);
            }
            layer.pixels = Arc::new(self.read_pixels(&output)?);
        }
        self.check_document(&out)?;
        Ok(out)
    }
}
