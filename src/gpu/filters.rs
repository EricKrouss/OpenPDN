//! Float intermediates remain on the GPU until the final selection-masked
//! writeback. Kernel construction follows the recovered 5.1.12 effects.
use super::*;

fn gaussian_kernel(sigma: f64, support: f64) -> Vec<[f32; 4]> {
    if sigma == 0. {
        return vec![[0., 0., 1., 0.]];
    }
    let radius = (sigma * support).ceil() as i32;
    let mut weights = vec![0.; (radius * 2 + 1) as usize];
    // ConvertToPointSampleKernel aggregates the 1/8 pixel Gaussian samples
    // into bilinear point weights; keep the accumulation in double precision.
    for step in -radius * 8..=radius * 8 {
        let position = f64::from(step) / 8.;
        let lo = position.floor() as i32;
        let frac = position - f64::from(lo);
        let weight = (-position * position / (2. * sigma * sigma)).exp();
        weights[(lo + radius) as usize] += weight * (1. - frac);
        if frac != 0. {
            weights[(lo + radius + 1) as usize] += weight * frac;
        }
    }
    let sum: f64 = weights.iter().sum();
    weights
        .into_iter()
        .enumerate()
        .map(|(i, weight)| [(i as i32 - radius) as f32, 0., (weight / sum) as f32, 0.])
        .collect()
}
fn square_kernel(radius: f32) -> Vec<[f32; 4]> {
    let extent = radius.ceil() as i32;
    (-extent..=extent)
        .map(|x| {
            [
                x as f32,
                0.,
                (1. - (x.unsigned_abs() as f32 - radius).max(0.)) / (1. + radius * 2.),
                0.,
            ]
        })
        .collect()
}
fn motion_kernel(values: [f32; 8]) -> Vec<[f32; 4]> {
    let angle = f64::from(values[0]).to_radians();
    let radius = f64::from(values[1]) / if values[2] != 0. { 2. } else { 1. };
    let sigma = radius / 3.;
    let lo = if values[2] != 0. {
        (-radius).floor() as i32
    } else {
        0
    };
    let hi = radius.ceil() as i32;
    let mut weights = std::collections::BTreeMap::<(i32, i32), f64>::new();
    for step in lo * 8..=hi * 8 {
        let t = f64::from(step) / 8.;
        let x = -angle.cos() * t;
        let y = angle.sin() * t;
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let fx = x - x.floor();
        let fy = y - y.floor();
        let gaussian = (-t * t / (2. * sigma * sigma)).exp();
        for (dx, dy, w) in [
            (0, 0, (1. - fx) * (1. - fy)),
            (1, 0, fx * (1. - fy)),
            (0, 1, (1. - fx) * fy),
            (1, 1, fx * fy),
        ] {
            if w != 0. {
                *weights.entry((iy + dy, ix + dx)).or_default() += gaussian * w;
            }
        }
    }
    let sum: f64 = weights.values().sum();
    weights
        .into_iter()
        .map(|((y, x), w)| [x as f32, y as f32, (w / sum) as f32, 0.])
        .collect()
}

impl Gpu {
    pub(super) fn filter_image(
        &self,
        doc: &Document,
        op: Adjustment,
        values: [f32; 8],
    ) -> Result<Vec<Pixel>> {
        self.filter_image_with_state(doc, op, values, &mut redeye::State::default())
    }
    fn filter_image_with_state(
        &self,
        doc: &Document,
        op: Adjustment,
        values: [f32; 8],
        red_eye_state: &mut redeye::State,
    ) -> Result<Vec<Pixel>> {
        if op == Adjustment::GaussianBlur && values[2] < 4. {
            return self.optimized_gaussian(doc, values);
        }
        if op == Adjustment::RedEye && values[0] > 1. {
            let mut intermediate = doc.clone();
            let mut v = values;
            v[0] = 1.;
            for iteration in 0..values[0] as u32 {
                v[1] = f32::from(iteration + 1 < values[0] as u32);
                let pixels = self.filter_image_with_state(&intermediate, op, v, red_eye_state)?;
                intermediate.layers[doc.active].pixels = std::sync::Arc::new(pixels);
            }
            let mut result = intermediate.layers[doc.active].pixels.as_ref().clone();
            for (i, pixel) in result.iter_mut().enumerate() {
                if !doc.selected(i) {
                    *pixel = doc.layers[doc.active].pixels[i];
                }
            }
            return Ok(result);
        }
        let count = doc.width * doc.height;
        ensure!(
            count.div_ceil(256) <= self.device.limits().max_compute_workgroups_per_dimension,
            "Filter exceeds GPU dispatch limits"
        );
        ensure!(
            u64::from(count) * 16 <= self.device.limits().max_storage_buffer_binding_size,
            "Filter float buffer exceeds GPU storage limit"
        );
        let neutral = match op {
            Adjustment::HighlightsShadows => values[..3].iter().all(|v| *v == 0.),
            Adjustment::SquareBlur | Adjustment::GaussianBlur | Adjustment::RadialBlur => {
                values[0] == 0.
            }
            Adjustment::Fragment => values[1] == 0.,
            Adjustment::ZoomBlur => values[0] == 1.,
            Adjustment::Bulge | Adjustment::Twist | Adjustment::PolarInversion => values[0] == 0.,
            Adjustment::TileReflection => values[2] == 0.,
            Adjustment::Median
            | Adjustment::Bokeh
            | Adjustment::SketchBlur
            | Adjustment::Sharpen
            | Adjustment::Straighten => values[0] == 0.,
            Adjustment::Dents => values[0] == 0. || values[1] == 0.,
            Adjustment::Pixelate => values[0] == 1.,
            Adjustment::ReduceNoise => values[0] == 0. || values[1] == 0.,
            Adjustment::AddNoise | Adjustment::FrostedGlass => values[0] == 0.,
            _ => false,
        };
        if neutral {
            return Ok(doc.layers[doc.active].pixels.as_ref().clone());
        }
        let boosted = matches!(
            op,
            Adjustment::SquareBlur | Adjustment::GaussianBlur | Adjustment::Bokeh
        );
        let linear = boosted
            || matches!(
                op,
                Adjustment::DropShadow
                    | Adjustment::OilPainting
                    | Adjustment::FrostedGlass
                    | Adjustment::Crystalize
                    | Adjustment::Dents
                    | Adjustment::Pixelate
                    | Adjustment::Straighten
                    | Adjustment::Julia
                    | Adjustment::Mandelbrot
            );
        let straight_output = matches!(
            op,
            Adjustment::Median
                | Adjustment::Outline
                | Adjustment::ReduceNoise
                | Adjustment::AddNoise
                | Adjustment::SketchBlur
                | Adjustment::RedEye
        );
        let gamma = if boosted { 1. + values[1] } else { 1. };
        let kernel = match op {
            Adjustment::SquareBlur => square_kernel(values[0]),
            Adjustment::GaussianBlur => gaussian_kernel(f64::from(values[0] / 3.), 6.),
            Adjustment::HighlightsShadows => gaussian_kernel(f64::from(values[3]), 3.),
            Adjustment::Glow => gaussian_kernel(f64::from(values[0] * 1.25 / 3.), 3.),
            Adjustment::PencilSketch => gaussian_kernel(f64::from(values[0] * 1.5 / 3.), 3.),
            Adjustment::InkSketch => gaussian_kernel(2.5, 3.),
            Adjustment::SoftenPortrait => gaussian_kernel(f64::from(values[0] * 1.25), 3.),
            Adjustment::DropShadow => gaussian_kernel(f64::from(values[0] / 3.), 3.),
            Adjustment::RedEye => redeye::parameters(doc, red_eye_state),
            Adjustment::Quantize => {
                palette::build(doc.layers[doc.active].pixels.as_slice(), values)
            }
            Adjustment::Bokeh => kernels::bokeh(f64::from(values[0]), values[2] as usize),
            Adjustment::SketchBlur => kernels::sketch(values[0], values[2] as usize),
            Adjustment::MotionBlur => motion_kernel(values),
            Adjustment::Turbulence => kernels::turbulence(values[4] as u32, values[1], values[2]),
            Adjustment::Pixelate => {
                let bounds = doc
                    .selection_bounds()
                    .unwrap_or([0, 0, doc.width, doc.height]);
                let center = [
                    bounds[0] as f32 + bounds[2] as f32 * (1. + values[1]) * 0.5,
                    bounds[1] as f32 + bounds[3] as f32 * (1. + values[2]) * 0.5,
                ];
                let origin = center.map(|c| (c * (1. - 1. / values[0])).floor() - 2.);
                let width = (doc.width as f32 / values[0]).ceil() + 6.;
                let height = (doc.height as f32 / values[0]).ceil() + 6.;
                vec![[origin[0], origin[1], width, height]]
            }
            Adjustment::Dents => {
                let bounds = doc
                    .selection_bounds()
                    .unwrap_or([0, 0, doc.width, doc.height]);
                let scale_r = 800. / f64::from(bounds[2].min(bounds[3])) / f64::from(values[0]);
                let detail = 1. + f64::from(values[2]) / 10.;
                let limit = scale_r.log(0.5).floor();
                let detail = if detail > limit && limit >= 1. {
                    limit
                } else {
                    detail
                };
                let angle = f64::from(values[4]).to_radians();
                vec![
                    [
                        scale_r as f32,
                        (std::f64::consts::TAU * f64::from(values[3]) / 10.) as f32,
                        detail as f32,
                        values[2] / 100.,
                    ],
                    [
                        (f64::from(values[1]) / 100. / scale_r) as f32,
                        angle.cos() as f32,
                        angle.sin() as f32,
                        0.,
                    ],
                ]
            }
            Adjustment::EdgeDetect => gaussian_kernel(f64::from(values[1]), 3.),
            Adjustment::AddNoise => noise::lookup_table()
                .iter()
                .map(|v| [*v as f32, 0., 0., 0.])
                .collect(),
            Adjustment::FrostedGlass => {
                // The original caches one unexposed Random.Shared.Next seed
                // per sample map, with Fibonacci sample counts (no RGSS).
                // A saved stream seed regenerates those independent 31-bit
                // seeds, retaining them across previews, quality and repeat.
                let mut state = values[6] as u32 | ((values[7] as u32) << 16);
                let instance = state & 255;
                let count = [1, 2, 3, 5, 8, 13, 21, 34][values[3] as usize - 1];
                (0..count)
                    .map(|_| {
                        state = noise::pcg(state);
                        let seed = (state & 0x7fffffff) ^ instance;
                        [
                            (seed & 65535) as f32,
                            (seed >> 16) as f32,
                            values[1].powf(values[2]),
                            values[0].powf(values[2]),
                        ]
                    })
                    .collect()
            }
            _ => vec![[0., 0., 1., 0.]],
        };
        let source = self.buffer(
            "Filter source pixels",
            bytemuck::cast_slice(doc.layers[doc.active].pixels.as_slice()),
            wgpu::BufferUsages::STORAGE,
        );
        let selected: Vec<u32> = (0..count as usize)
            .map(|i| doc.selected(i) as u32)
            .collect();
        let mask = self.buffer(
            "Filter selection",
            bytemuck::cast_slice(&selected),
            wgpu::BufferUsages::STORAGE,
        );
        let cache_count = if op == Adjustment::Pixelate {
            (kernel[0][2] * kernel[0][3]) as u32
        } else {
            count
        };
        ensure!(
            kernel.len() as u64 * 16 <= self.device.limits().max_storage_buffer_binding_size,
            "Filter kernel exceeds GPU storage limit"
        );
        ensure!(
            u64::from(count.max(cache_count)) * 16
                <= self.device.limits().max_storage_buffer_binding_size,
            "Filter cache exceeds GPU storage limit"
        );
        let kernel = self.buffer(
            "Filter kernel",
            bytemuck::cast_slice(&kernel),
            wgpu::BufferUsages::STORAGE,
        );
        let float_count = count.max(cache_count);
        let a = self.output_buffer(u64::from(float_count) * 16);
        let b = self.output_buffer(u64::from(float_count) * 16);
        let c = self.output_buffer(if matches!(op, Adjustment::Bokeh | Adjustment::Quantize) {
            u64::from(count) * 16
        } else {
            16
        });
        let d = self.output_buffer(if matches!(op, Adjustment::Bokeh | Adjustment::Quantize) {
            u64::from(count) * 16
        } else {
            16
        });
        let dummy = self.output_buffer(16);
        let output = self.output_buffer(u64::from(count) * 4);
        let bounds = doc
            .selection_bounds()
            .unwrap_or([0, 0, doc.width, doc.height])
            .map(|v| v as f32);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut pass = |operation: u32,
                        axis: u32,
                        input: &wgpu::Buffer,
                        destination: &wgpu::Buffer,
                        controls: [f32; 8]| {
            let mut uniform = Vec::with_capacity(20);
            uniform.extend_from_slice(&[doc.width, doc.height, operation, axis]);
            uniform.extend(controls.map(f32::to_bits));
            uniform.extend_from_slice(&[
                linear as u32,
                gamma.to_bits(),
                straight_output as u32,
                u32::from(op == Adjustment::RedEye && values[1] != 0.),
            ]);
            uniform.extend(bounds.map(f32::to_bits));
            let params = self.buffer(
                "Filter parameters",
                bytemuck::cast_slice(&uniform),
                wgpu::BufferUsages::UNIFORM,
            );
            let buffers = [
                &source,
                &mask,
                input,
                destination,
                &output,
                &params,
                &kernel,
                if operation == 8 || operation == 42 {
                    &c
                } else {
                    &dummy
                },
            ];
            let entries: Vec<_> = buffers
                .iter()
                .enumerate()
                .map(|(i, b)| wgpu::BindGroupEntry {
                    binding: i as u32,
                    resource: b.as_entire_binding(),
                })
                .collect();
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Filter resources"),
                layout: &self.filter_layout,
                entries: &entries,
            });
            let mut compute = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Floating point filter pass"),
                timestamp_writes: None,
            });
            let pipeline = self.filter_pipeline(operation);
            compute.set_pipeline(&pipeline);
            compute.set_bind_group(0, &group, &[]);
            compute.dispatch_workgroups(
                if operation == 60 {
                    cache_count.div_ceil(256)
                } else if operation == 42 && controls[2] != 0. {
                    doc.height.div_ceil(256)
                } else {
                    count.div_ceil(256)
                },
                1,
                1,
            );
        };
        pass(0, 0, &b, &a, values);
        let final_input = match op {
            Adjustment::Quantize => {
                if values[2] == 0. {
                    pass(op as u32, 0, &a, &b, values);
                } else {
                    for diagonal in 0..doc.width + 2 * (doc.height - 1) {
                        pass(op as u32, diagonal, &a, &b, values);
                    }
                }
                &b
            }
            Adjustment::Bokeh => {
                let size = 2 * values[0].ceil() as u32 + 1;
                for component in 0..values[2] as u32 {
                    let mut v = values;
                    v[3] = (component * size) as f32;
                    v[4] = size as f32;
                    pass(6, 0, &a, &b, v);
                    pass(7, 0, &a, &c, v);
                    pass(8, u32::from(component != 0), &b, &d, v);
                }
                &d
            }
            Adjustment::Pixelate => {
                pass(60, 0, &a, &b, values);
                pass(op as u32, 0, &b, &a, values);
                &a
            }
            Adjustment::RedEye => {
                pass(op as u32, 0, &a, &b, values);
                &b
            }
            Adjustment::Turbulence => {
                pass(op as u32, 0, &a, &b, values);
                &b
            }
            Adjustment::SketchBlur => {
                pass(op as u32, 0, &a, &b, values);
                &b
            }

            Adjustment::SquareBlur | Adjustment::GaussianBlur | Adjustment::HighlightsShadows => {
                pass(2, 0, &a, &b, values);
                pass(2, 1, &b, &a, values);
                if op == Adjustment::HighlightsShadows {
                    pass(3, 0, &a, &b, values);
                    let h = values[0] / 100.;
                    let s = values[1] / 100.;
                    let c = values[2] / 100.;
                    let controls = [
                        4.0_f32.powf(h * if h > 0. { 2. } else { 1.5 }),
                        5.0_f32.powf(s * if s > 0. { 0.8 } else { 1.5 }),
                        c * if c > 0. { 2. } else { 1. },
                        0.,
                        0.,
                        0.,
                        0.,
                        0.,
                    ];
                    pass(4, 0, &b, &a, controls);
                    &a
                } else {
                    &a
                }
            }
            Adjustment::Glow
            | Adjustment::PencilSketch
            | Adjustment::InkSketch
            | Adjustment::SoftenPortrait => {
                pass(2, 0, &a, &b, values);
                pass(2, 1, &b, &a, values);
                pass(op as u32, 0, &a, &b, values);
                &b
            }
            Adjustment::Fragment
            | Adjustment::RadialBlur
            | Adjustment::ZoomBlur
            | Adjustment::Bulge
            | Adjustment::Twist
            | Adjustment::PolarInversion
            | Adjustment::TileReflection => {
                pass(op as u32, 0, &a, &b, values);
                &b
            }
            Adjustment::MotionBlur => {
                pass(op as u32, 0, &a, &b, values);
                &b
            }
            Adjustment::SurfaceBlur
            | Adjustment::Median
            | Adjustment::Outline
            | Adjustment::OilPainting
            | Adjustment::ReduceNoise
            | Adjustment::AddNoise
            | Adjustment::FrostedGlass => {
                pass(op as u32, 0, &a, &b, values);
                &b
            }
            Adjustment::EdgeDetect => {
                if values[1] > 0. {
                    pass(2, 0, &a, &b, values);
                    pass(2, 1, &b, &a, values);
                }
                pass(op as u32, 0, &a, &b, values);
                &b
            }
            Adjustment::Crystalize
            | Adjustment::Dents
            | Adjustment::Straighten
            | Adjustment::Sharpen
            | Adjustment::Clouds
            | Adjustment::Julia
            | Adjustment::Mandelbrot => {
                pass(op as u32, 0, &a, &b, values);
                &b
            }
            Adjustment::DropShadow => {
                // Translate each separable axis before convolution. This keeps
                // shadows which extend outside the source and return after the
                // translation, without clipping an intermediate shadow image.
                pass(5, 0, &a, &b, values);
                pass(5, 1, &b, &a, values);
                pass(op as u32, 0, &a, &b, values);
                &b
            }
            _ => unreachable!(),
        };
        pass(
            1,
            0,
            final_input,
            if std::ptr::eq(final_input, &a) {
                &b
            } else {
                &a
            },
            values,
        );
        self.queue.submit([encoder.finish()]);
        self.read_pixels(&output)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn original_managed_gaussian_kernels() {
        #[derive(serde::Deserialize)]
        struct Case {
            sigma: f64,
            weights: Vec<f32>,
        }
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../tests/fixtures/paintnet-5.1.12-gaussian.json"
        ))
        .unwrap();
        for case in cases {
            let kernel = super::gaussian_kernel(case.sigma, 6.);
            assert_eq!(kernel.len(), case.weights.len());
            for (a, b) in kernel.iter().zip(&case.weights) {
                assert!(
                    (a[2] - b).abs() < 0.00000002,
                    "sigma {}: {} vs {b}",
                    case.sigma,
                    a[2]
                );
            }
        }
    }
    #[test]
    fn kernels_normalized_and_symmetric() {
        for kernel in [
            super::square_kernel(0.25),
            super::square_kernel(6.75),
            super::gaussian_kernel(0.1, 6.),
            super::gaussian_kernel(6., 6.),
        ] {
            assert!((kernel.iter().map(|v| v[2]).sum::<f32>() - 1.).abs() < 1e-6);
            for (a, b) in kernel.iter().zip(kernel.iter().rev()) {
                assert_eq!(a[2], b[2]);
            }
        }
    }
}
