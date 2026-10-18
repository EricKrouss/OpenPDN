use openpdn::{
    document::{Document, Pixel},
    gpu::{Adjustment as Op, Gpu},
};
use std::sync::Arc;

fn mirror(x: i32, n: i32) -> usize {
    let x = x.rem_euclid(n * 2);
    if x >= n {
        (n * 2 - 1 - x) as usize
    } else {
        x as usize
    }
}
fn linear(x: f64) -> f64 {
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(x: f64) -> f64 {
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1. / 2.4) - 0.055
    }
}
fn square_oracle(doc: &Document, radius: f64, gamma: f64) -> Vec<Pixel> {
    let extent = radius.ceil() as i32;
    (0..doc.width * doc.height)
        .map(|i| {
            let x = (i % doc.width) as i32;
            let y = (i / doc.width) as i32;
            let mut sum = [0.; 4];
            // Independent direct 2D convolution, without quantized intermediates.
            for dy in -extent..=extent {
                for dx in -extent..=extent {
                    let w = (1. - (f64::from(dx.abs()) - radius).max(0.))
                        * (1. - (f64::from(dy.abs()) - radius).max(0.))
                        / (1. + radius * 2.).powi(2);
                    let p = doc.layers[0].pixels[mirror(y + dy, doc.height as i32)
                        * doc.width as usize
                        + mirror(x + dx, doc.width as i32)];
                    let alpha = f64::from(p[3]) / 255.;
                    for c in 0..3 {
                        sum[c] += linear(f64::from(p[c]) / 255.).powf(gamma) * alpha * w;
                    }
                    sum[3] += alpha * w;
                }
            }
            let mut p = [0; 4];
            if sum[3] > 0. {
                for c in 0..3 {
                    p[c] = (srgb((sum[c] / sum[3]).powf(1. / gamma)).clamp(0., 1.) * 255.).round()
                        as u8;
                }
            }
            p[3] = (sum[3].clamp(0., 1.) * 255.).round() as u8;
            p
        })
        .collect()
}
fn fixture() -> Document {
    let mut doc = Document::new(17, 13, [0; 4]).unwrap();
    doc.layers[0].pixels = Arc::new(
        (0..221)
            .map(|i| {
                [
                    (i * 43 % 256) as u8,
                    (i * 71 % 256) as u8,
                    (i * 17 % 256) as u8,
                    (i * 53 % 256) as u8,
                ]
            })
            .collect(),
    );
    doc
}

fn surface_oracle(doc: &Document, radius: i32, threshold: f64) -> Vec<Pixel> {
    let area = ((2 * radius + 1).pow(2) + 2) / 4;
    (0..doc.width * doc.height)
        .map(|i| {
            let center = doc.layers[0].pixels[i as usize].map(|x| f64::from(x) / 255.);
            let mut sum = [0.; 4];
            let mut weights = [0.; 4];
            for y in -radius..=radius {
                for x in -radius..=radius {
                    if x * x + y * y > area {
                        continue;
                    }
                    let at = mirror(i as i32 / doc.width as i32 + y, doc.height as i32)
                        * doc.width as usize
                        + mirror(i as i32 % doc.width as i32 + x, doc.width as i32);
                    let sample = doc.layers[0].pixels[at].map(|x| f64::from(x) / 255.);
                    for c in 0..4 {
                        let w = (1. - (sample[c] - center[c]).abs() * 0.96 / threshold).max(0.);
                        sum[c] += sample[c] * w;
                        weights[c] += w;
                    }
                }
            }
            std::array::from_fn(|c| (sum[c] / weights[c] * 255.).round() as u8)
        })
        .collect()
}

fn shadow_alpha_sample(doc: &Document, x: f64, y: f64) -> f64 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let tx = x - x.floor();
    let ty = y - y.floor();
    let mut result = 0.;
    for dy in 0..=1 {
        for dx in 0..=1 {
            if (0..doc.width as i32).contains(&(ix + dx))
                && (0..doc.height as i32).contains(&(iy + dy))
            {
                let w = (if dx == 0 { 1. - tx } else { tx }) * (if dy == 0 { 1. - ty } else { ty });
                result += w * f64::from(
                    doc.layers[0].pixels
                        [(iy + dy) as usize * doc.width as usize + (ix + dx) as usize][3],
                ) / 255.;
            }
        }
    }
    result
}

fn shadow_oracle(doc: &Document, values: [f32; 8]) -> Vec<Pixel> {
    let sigma = f64::from(values[0] / 3.);
    let radius = (sigma * 3.).ceil() as i32;
    let angle = -f64::from(values[2]).to_radians();
    let shift = [
        f64::from(values[1]) * angle.cos(),
        f64::from(values[1]) * angle.sin(),
    ];
    let color = values[4] as u32;
    let color =
        [color >> 16 & 255, color >> 8 & 255, color & 255].map(|c| linear(f64::from(c) / 255.));
    (0..doc.width * doc.height)
        .map(|i| {
            // Direct 2D subpixel Gaussian integration: independent of the GPU's
            // aggregated, separable point kernel and intermediate buffers.
            let mut alpha = 0.;
            let mut weights = 0.;
            let at = [
                f64::from(i % doc.width) - shift[0],
                f64::from(i / doc.width) - shift[1],
            ];
            let frac = [at[0] - at[0].floor(), at[1] - at[1].floor()];
            for y in -radius * 8..=radius * 8 {
                for x in -radius * 8..=radius * 8 {
                    let offset = [f64::from(x) / 8., f64::from(y) / 8.];
                    let w = if sigma == 0. {
                        1.
                    } else {
                        (-(offset[0].powi(2) + offset[1].powi(2)) / (2. * sigma * sigma)).exp()
                    };
                    // The native graph translates the filtered image. Its
                    // bilinear phase follows the Gaussian point aggregation.
                    for dy in 0..=1 {
                        for dx in 0..=1 {
                            let phase = (if dx == 0 { 1. - frac[0] } else { frac[0] })
                                * (if dy == 0 { 1. - frac[1] } else { frac[1] });
                            alpha += w
                                * phase
                                * shadow_alpha_sample(
                                    doc,
                                    at[0].floor() + f64::from(dx) + offset[0],
                                    at[1].floor() + f64::from(dy) + offset[1],
                                );
                        }
                    }
                    weights += w;
                }
            }
            alpha = alpha / weights * f64::from(values[3]);
            let original = doc.layers[0].pixels[i as usize].map(|v| f64::from(v) / 255.);
            let mut output = [0; 4];
            let a = if values[5] != 0. {
                alpha
            } else {
                original[3] + alpha * (1. - original[3])
            };
            for c in 0..3 {
                let premul = if values[5] != 0. {
                    color[c] * alpha
                } else {
                    linear(original[c]) * original[3] + color[c] * alpha * (1. - original[3])
                };
                if a > 0. {
                    output[c] = (srgb(premul / a).clamp(0., 1.) * 255.).round() as u8;
                }
            }
            output[3] = (a * 255.).round() as u8;
            output
        })
        .collect()
}

#[test]
#[ignore = "Requires a native GPU; float filters and native adjustment arithmetic"]
fn hardware_float_filters() {
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert_ne!(
        gpu.info.device_type,
        eframe::egui_wgpu::wgpu::DeviceType::Cpu
    );
    eprintln!("Float filters: {} / {:?}", gpu.info.name, gpu.info.backend);
    let doc = fixture();
    for (radius, threshold) in [(1, 1.), (3, 15.), (6, 100.)] {
        let result = gpu
            .adjust_configured(&doc, Op::SurfaceBlur, [radius as f32, threshold, 0., 0.])
            .unwrap();
        let expected = surface_oracle(&doc, radius, f64::from(threshold) / 100.);
        for (a, b) in result.iter().zip(expected) {
            for c in 0..4 {
                assert!(
                    (i32::from(a[c]) - i32::from(b[c])).abs() <= 1,
                    "Surface {radius}/{threshold}: {a:?} vs {b:?}"
                );
            }
        }
    }
    for values in [
        [0., 2.5, 35., 0.7, 0x37a9d2 as f32, 0., 0., 0.],
        [3.2, 8., 0., 1., 0x37a9d2 as f32, 1., 0., 0.],
        [3.2, 6.5, -60., 0.5, 0., 0., 0., 0.],
    ] {
        let result = gpu.adjust_parameters(&doc, Op::DropShadow, values).unwrap();
        let expected = shadow_oracle(&doc, values);
        for (i, (a, b)) in result.iter().zip(expected).enumerate() {
            for c in 0..4 {
                assert!(
                    (i32::from(a[c]) - i32::from(b[c])).abs() <= 1,
                    "Shadow {values:?} pixel {i}: {a:?} vs {b:?}"
                );
            }
        }
    }
    // Uniform opaque fields isolate the recovered Screen/Darken and Overlay
    // graphs from the Gaussian optimization and native color-profile boundary.
    for color in [[30, 60, 90, 255], [180, 120, 210, 255]] {
        let flat = Document::new(3, 2, color).unwrap();
        let c = color[..3]
            .iter()
            .map(|v| f64::from(*v) / 255.)
            .collect::<Vec<_>>();
        let y = c[0] * 0.299 + c[1] * 0.587 + c[2] * 0.114;
        let expected: Pixel = std::array::from_fn(|channel| {
            if channel == 3 {
                255
            } else if 2. * y <= 0.5 {
                0
            } else {
                ((2. * c[channel] - c[channel] * c[channel]) * 255.).round() as u8
            }
        });
        for pixel in gpu.adjust(&flat, Op::InkSketch).unwrap() {
            for channel in 0..4 {
                assert!(
                    (i32::from(pixel[channel]) - i32::from(expected[channel])).abs() <= 1,
                    "Ink uniform: {pixel:?} vs {expected:?}"
                );
            }
        }
        let warmth = 0.2;
        let dst = [y * (1. + warmth), y, y * (1. - warmth)];
        let expected: Pixel = std::array::from_fn(|channel| {
            if channel == 3 {
                255
            } else {
                let x = dst[channel];
                let v = if x < 0.5 {
                    2. * x * c[channel]
                } else {
                    1. - 2. * (1. - x) * (1. - c[channel])
                };
                (v.clamp(0., 1.) * 255.).round() as u8
            }
        });
        for pixel in gpu
            .adjust_configured(&flat, Op::SoftenPortrait, [0., 0., 20., 0.])
            .unwrap()
        {
            for channel in 0..4 {
                assert!(
                    (i32::from(pixel[channel]) - i32::from(expected[channel])).abs() <= 1,
                    "Portrait uniform: {pixel:?} vs {expected:?}"
                );
            }
        }
    }
    for (radius, gamma) in [(0.25, 1.), (2.5, 0.4), (6., 3.)] {
        let result = gpu
            .adjust_configured(&doc, Op::SquareBlur, [radius, gamma - 1., 0., 0.])
            .unwrap();
        let expected = square_oracle(&doc, f64::from(radius), f64::from(gamma));
        for (i, (a, b)) in result.iter().zip(&expected).enumerate() {
            for c in 0..4 {
                assert!(
                    (i32::from(a[c]) - i32::from(b[c])).abs() <= 1,
                    "Square {radius}/{gamma} pixel {i} channel {c}: {a:?} vs {b:?}"
                );
            }
        }
    }
    let gains: Vec<[f32; 4]> =
        serde_json::from_str(include_str!("fixtures/native-temperature.json")).unwrap();
    for [temperature, tint, red, blue] in gains {
        let result = gpu
            .adjust_configured(&doc, Op::TemperatureTint, [temperature, tint, 0., 0.])
            .unwrap();
        for (a, b) in result.iter().zip(doc.layers[0].pixels.iter()) {
            for (c, gain) in [(0, red), (1, 1.), (2, blue)] {
                assert!(
                    (i32::from(a[c]) - (f32::from(b[c]) * gain).clamp(0., 255.).round() as i32)
                        .abs()
                        <= 1
                );
            }
            assert_eq!(a[3], b[3]);
        }
    }
    let operations = [
        Op::HighlightsShadows,
        Op::SquareBlur,
        Op::GaussianBlur,
        Op::Fragment,
        Op::RadialBlur,
        Op::ZoomBlur,
        Op::Bulge,
        Op::Twist,
        Op::PolarInversion,
        Op::TileReflection,
        Op::Glow,
        Op::PencilSketch,
        Op::MotionBlur,
        Op::InkSketch,
        Op::SurfaceBlur,
        Op::SoftenPortrait,
        Op::DropShadow,
    ];
    for op in operations {
        let mut values = op.parameters();
        if op == Op::HighlightsShadows {
            values[..3].copy_from_slice(&[-50., 70., 30.]);
        }
        let full = gpu.adjust_parameters(&doc, op, values).unwrap();
        assert_ne!(full, *doc.layers[0].pixels, "{} is inert", op.name());
        let mut selected = doc.clone();
        selected.select_rect((4, 3), (11, 9), false);
        let output = gpu.adjust_parameters(&selected, op, values).unwrap();
        for (i, pixel) in output.iter().enumerate() {
            if !selected.selected(i) {
                assert_eq!(
                    *pixel,
                    doc.layers[0].pixels[i],
                    "{} selection pixel {i}",
                    op.name()
                );
            } else if !matches!(op, Op::Bulge | Op::Twist | Op::PolarInversion) {
                assert_eq!(
                    *pixel,
                    full[i],
                    "{} must sample outside the selection",
                    op.name()
                );
            }
        }
        if op == Op::HighlightsShadows {
            assert!(
                output
                    .iter()
                    .zip(doc.layers[0].pixels.iter())
                    .all(|(a, b)| a[3] == b[3])
            );
        }
        let mut invalid = values;
        invalid[0] = f32::NAN;
        assert!(gpu.adjust_parameters(&doc, op, invalid).is_err());
        invalid = values;
        invalid[0] = op.controls()[0].2 + 1.;
        assert!(gpu.adjust_parameters(&doc, op, invalid).is_err());
    }
    for (op, index, value) in [
        (Op::SquareBlur, 0, 0.),
        (Op::GaussianBlur, 0, 0.),
        (Op::Fragment, 1, 0.),
        (Op::RadialBlur, 0, 0.),
        (Op::ZoomBlur, 0, 1.),
        (Op::Bulge, 0, 0.),
        (Op::Twist, 0, 0.),
        (Op::PolarInversion, 0, 0.),
        (Op::TileReflection, 2, 0.),
    ] {
        let mut parameters = op.parameters();
        parameters[index] = value;
        assert_eq!(
            gpu.adjust_parameters(&doc, op, parameters).unwrap(),
            *doc.layers[0].pixels,
            "{} neutral/hidden RGB",
            op.name()
        );
    }
    assert_eq!(
        gpu.adjust(&doc, Op::HighlightsShadows).unwrap(),
        *doc.layers[0].pixels
    );
    for (op, values) in [
        (Op::Bulge, [-3., -1., 1., 3., 8., 0., 0., 0.]),
        (Op::Twist, [-200., 0.01, 2., -2., 8., 0., 0., 0.]),
        (Op::PolarInversion, [-8., 0., 0., 1., 8., 0., 0., 0.]),
        (Op::TileReflection, [180., 1., -200., 2., 8., 0., 0., 0.]),
        (Op::ZoomBlur, [4., 4., 2., -2., 8., 0., 0., 0.]),
        (Op::GaussianBlur, [300., -0.99, 4., 0., 0., 0., 0., 0.]),
        (Op::InkSketch, [99., 100., 0., 0., 0., 0., 0., 0.]),
        (Op::SurfaceBlur, [50., 100., 0., 0., 0., 0., 0., 0.]),
        (Op::SoftenPortrait, [10., -20., 20., 0., 0., 0., 0., 0.]),
        (
            Op::DropShadow,
            [100., 100., -180., 1., 16777215., 1., 0., 0.],
        ),
    ] {
        assert_eq!(gpu.adjust_parameters(&doc, op, values).unwrap().len(), 221);
    }
    let single = Document::new(1, 1, [80, 120, 160, 128]).unwrap();
    for op in [
        Op::SquareBlur,
        Op::GaussianBlur,
        Op::Fragment,
        Op::RadialBlur,
        Op::ZoomBlur,
        Op::Bulge,
        Op::Twist,
        Op::PolarInversion,
        Op::TileReflection,
        Op::MotionBlur,
    ] {
        let mut values = op.parameters();
        if op == Op::Fragment {
            values[1] = 0.5;
        }
        if matches!(op, Op::Bulge | Op::PolarInversion | Op::TileReflection) {
            values[3] = 0.;
        }
        assert_eq!(
            gpu.adjust_parameters(&single, op, values).unwrap()[0],
            single.layers[0].pixels[0],
            "{} single pixel",
            op.name()
        );
    }
}
