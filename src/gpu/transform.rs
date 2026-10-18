use super::*;
use crate::transform::FloatingSelection;
use std::sync::Arc;

impl Gpu {
    pub fn transform_selection(
        &self,
        floating: &FloatingSelection,
        bilinear: bool,
        linear_light: bool,
    ) -> Result<Document> {
        self.check_document(&floating.base)?;
        floating.transform.validate()?;
        let count = crate::document::pixel_count(floating.width, floating.height)?;
        ensure!(
            floating.pixels.len() == count && floating.mask.len() == count,
            "Invalid floating pixels or mask"
        );
        ensure!(
            (count as u64 * 4) <= self.device.limits().max_storage_buffer_binding_size,
            "Floating source exceeds GPU storage limit"
        );
        if !floating.pasted && floating.transform == floating.original_transform {
            return Ok(floating.before.clone());
        }
        let t = floating.transform;
        let (sin, cos) = t.angle.to_radians().sin_cos();
        let row0 = [cos / t.scale[0], sin / t.scale[0]];
        let row1 = [-sin / t.scale[1], cos / t.scale[1]];
        let floats = [
            row0[0],
            row0[1],
            floating.width as f32 * 0.5 - row0[0] * t.center[0] - row0[1] * t.center[1],
            0.,
            row1[0],
            row1[1],
            floating.height as f32 * 0.5 - row1[0] * t.center[0] - row1[1] * t.center[1],
            0.,
        ];
        let mut params = vec![
            floating.base.width,
            floating.base.height,
            floating.width,
            floating.height,
        ];
        params.extend(floats.map(f32::to_bits));
        params.extend([
            u32::from(bilinear),
            u32::from(floating.include_pixels),
            u32::from(linear_light),
            0,
        ]);
        let mask: Vec<u32> = floating.mask.iter().map(|&v| u32::from(v)).collect();
        let input = self.buffer(
            "Floating source",
            bytemuck::cast_slice(&floating.pixels),
            wgpu::BufferUsages::STORAGE,
        );
        let coverage = self.buffer(
            "Floating mask",
            bytemuck::cast_slice(&mask),
            wgpu::BufferUsages::STORAGE,
        );
        let base = self.buffer(
            "Transform background",
            bytemuck::cast_slice(&floating.base.layers[floating.base.active].pixels),
            wgpu::BufferUsages::STORAGE,
        );
        let bytes = u64::from(floating.base.width) * u64::from(floating.base.height) * 4;
        let output = self.output_buffer(bytes);
        let selection = self.output_buffer(bytes);
        let uniform = self.buffer(
            "Transform parameters",
            bytemuck::cast_slice(&params),
            wgpu::BufferUsages::UNIFORM,
        );
        let entries: Vec<_> = [&input, &coverage, &base, &output, &selection, &uniform]
            .into_iter()
            .enumerate()
            .map(|(binding, b)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Floating selection"),
            layout: &self.transformer.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.transformer);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(
                floating.base.width.div_ceil(16),
                floating.base.height.div_ceil(16),
                1,
            );
        }
        self.queue.submit([encoder.finish()]);
        let mut result = floating.base.clone();
        if floating.include_pixels {
            result.layers[result.active].pixels = Arc::new(self.read_pixels(&output)?);
        }
        result.selection = Some(Arc::new(
            self.read_pixels(&selection)?
                .into_iter()
                .map(|p| p[0])
                .collect(),
        ));
        Ok(result)
    }
}
