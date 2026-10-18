use openpdn::{
    document::*,
    gpu::{Adjustment, Gpu},
};
use serde::Deserialize;
use std::sync::Arc;

#[test]
#[ignore = "Requires native hardware; tonal transfers, morphology and emboss"]
fn gpu_tonal_and_neighborhood_operations() {
    use openpdn::tonal::{Curves, Levels};
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert_ne!(
        gpu.info.device_type,
        eframe::egui_wgpu::wgpu::DeviceType::Cpu
    );
    eprintln!(
        "Content/tonal GPU: {} / {:?}",
        gpu.info.name, gpu.info.backend
    );
    let mut doc = Document::new(256, 3, TRANSPARENT).unwrap();
    doc.layers[0].pixels = Arc::new(
        (0..768)
            .map(|i| {
                [
                    (i % 256) as u8,
                    (255 - i % 256) as u8,
                    127,
                    ((i * 19) % 256) as u8,
                ]
            })
            .collect(),
    );
    for (table, luminosity) in [
        (Levels::default().table().unwrap(), false),
        (Curves::default().table().unwrap(), true),
    ] {
        assert_eq!(
            gpu.transfer(&doc, &table, luminosity).unwrap(),
            *doc.layers[0].pixels
        );
    }
    let levels = Levels {
        input_black: [0.2, 0.1, 0.3],
        input_white: [0.8, 0.9, 0.7],
        gamma: [0.5, 2., 1.],
        output_black: [0., 0.1, 0.2],
        output_white: [1., 0.8, 0.9],
    };
    let table = levels.table().unwrap();
    doc.select_rect((10, 1), (240, 1), false);
    let result = gpu.transfer(&doc, &table, false).unwrap();
    for (i, (a, p)) in result.iter().zip(doc.layers[0].pixels.iter()).enumerate() {
        if doc.selected(i) {
            for c in 0..3 {
                assert!((a[c] as i32 - (table[p[c] as usize][c] * 255.).round() as i32).abs() <= 1);
            }
            assert_eq!(a[3], p[3]);
        } else {
            assert_eq!(a, p);
        }
    }
    let mut curves = Curves::default();
    curves.points[0] = vec![(0, 255), (255, 0)];
    let table = curves.table().unwrap();
    let result = gpu.transfer(&doc, &table, true).unwrap();
    assert_ne!(result, *doc.layers[0].pixels);
    assert!(
        result
            .iter()
            .zip(doc.layers[0].pixels.iter())
            .enumerate()
            .all(|(i, (a, p))| if doc.selected(i) {
                a[3] == p[3]
            } else {
                a == p
            })
    );
    assert!(gpu.transfer(&doc, &[[f32::NAN; 4]; 2], false).is_err());
    assert!(gpu.transfer(&doc, &[], false).is_err());
    let mut doc = Document::new(7, 7, TRANSPARENT).unwrap();
    Arc::make_mut(&mut doc.layers[0].pixels)[24] = [200, 80, 10, 255];
    let dilated = gpu
        .adjust_configured(&doc, Adjustment::Morphology, [3., 3., 0., 0.])
        .unwrap();
    assert_eq!(dilated.iter().filter(|p| p[3] > 0).count(), 9);
    assert_eq!(dilated[0], TRANSPARENT);
    doc.select_rect((3, 3), (3, 3), false);
    let selected = gpu
        .adjust_configured(&doc, Adjustment::Morphology, [5., 5., 0., 0.])
        .unwrap();
    assert_eq!(
        selected, *doc.layers[0].pixels,
        "Separable prepass must not modify pixels outside the selection"
    );
    doc.selection = None;
    let mut full = Document::new(7, 7, [200, 80, 10, 255]).unwrap();
    let eroded = gpu
        .adjust_configured(&full, Adjustment::Morphology, [3., 3., 1., 0.])
        .unwrap();
    assert_eq!(eroded.iter().filter(|p| p[3] > 0).count(), 25);
    assert_eq!(eroded[24], [200, 80, 10, 255]);
    full.select_rect((3, 3), (3, 3), false);
    let output = gpu.adjust(&full, Adjustment::Emboss).unwrap();
    assert_eq!(output[24], [128, 128, 128, 255]);
    for (i, p) in output.iter().enumerate() {
        if i != 24 {
            assert_eq!(*p, [200, 80, 10, 255]);
        }
    }
    let huge = gpu
        .adjust_configured(&doc, Adjustment::Morphology, [100., 100., 0., 0.])
        .unwrap();
    assert!(huge.iter().all(|p| *p == [200, 80, 10, 255]));
}

#[derive(Deserialize)]
struct Fixture {
    mode: usize,
    dst: Pixel,
    src: Pixel,
    opacity: u8,
    composite: Pixel,
}

// Explicit opt-in: a skipped GPU test is never reported as evidence of acceleration.
#[test]
#[ignore = "Requires a working native GPU; run cargo test --test gpu -- --ignored --nocapture"]
fn gpu_matches_original_and_selection_boundaries() {
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    eprintln!(
        "GPU test adapter: {} / {:?} / {:?}",
        gpu.info.name, gpu.info.backend, gpu.info.device_type
    );
    assert_ne!(
        gpu.info.device_type,
        eframe::egui_wgpu::wgpu::DeviceType::Cpu,
        "Software adapter is not hardware validation"
    );
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-blends.json")).unwrap();
    for m in 0..14 {
        for opacity in [0, 1, 63, 127, 128, 191, 254, 255] {
            let cases: Vec<_> = fixtures
                .iter()
                .filter(|f| f.mode == m && f.opacity == opacity)
                .collect();
            let mut doc = Document::new(cases.len() as u32, 1, TRANSPARENT).unwrap();
            doc.layers[0].pixels = Arc::new(cases.iter().map(|f| f.dst).collect());
            doc.add_layer();
            doc.layers[1].pixels = Arc::new(cases.iter().map(|f| f.src).collect());
            doc.layers[1].opacity = opacity;
            doc.layers[1].blend_mode = BlendMode::ALL[m];
            let output = gpu.compose(&doc).unwrap();
            assert_eq!(
                gpu.read_pixels(&output.pixels).unwrap(),
                cases.iter().map(|f| f.composite).collect::<Vec<_>>(),
                "{:?} opacity {opacity}",
                BlendMode::ALL[m]
            );
        }
    }
    let table = include_bytes!("fixtures/paintnet-5.1.12-blend-components.bin");
    let mut doc = Document::new(256, 256, TRANSPARENT).unwrap();
    doc.layers[0].pixels = Arc::new((0..65536).map(|i| [(i / 256) as u8, 0, 0, 255]).collect());
    doc.add_layer();
    doc.layers[1].pixels = Arc::new((0..65536).map(|i| [(i % 256) as u8, 0, 0, 255]).collect());
    for (m, mode) in BlendMode::ALL.iter().enumerate() {
        doc.layers[1].blend_mode = *mode;
        let rendered = gpu.compose(&doc).unwrap();
        let actual = gpu.read_pixels(&rendered.pixels).unwrap();
        for (i, p) in actual.iter().enumerate() {
            assert_eq!(p[0], table[m * 65536 + i], "{mode:?} at {i}");
        }
    }
    doc.layers[1].visible = false;
    let rendered = gpu.compose(&doc).unwrap();
    assert_eq!(gpu.read_pixels(&rendered.pixels).unwrap(), doc.composite());
    let mut doc = Document::new(4, 3, [10, 20, 30, 128]).unwrap();
    doc.select_rect((1, 1), (2, 1), false);
    for op in [Adjustment::InvertColors, Adjustment::InvertAlpha] {
        let pixels = gpu.adjust(&doc, op).unwrap();
        for (i, p) in pixels.iter().enumerate() {
            let expected = if doc.selected(i) {
                match op {
                    Adjustment::InvertColors => [245, 235, 225, 128],
                    Adjustment::InvertAlpha => [10, 20, 30, 127],
                    _ => unreachable!(),
                }
            } else {
                [10, 20, 30, 128]
            };
            assert_eq!(*p, expected);
        }
    }
}

#[test]
#[ignore = "Requires a working native GPU; explicit reference-table and shader-equation checks"]
fn gpu_posterize_reference_tables_and_straight_alpha_adjustments() {
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert_ne!(
        gpu.info.device_type,
        eframe::egui_wgpu::wgpu::DeviceType::Cpu
    );
    eprintln!("Adjustment GPU: {} / {:?}", gpu.info.name, gpu.info.backend);
    let table = include_bytes!("fixtures/paintnet-5.1.12-posterize.bin");
    let mut doc = Document::new(256, 1, TRANSPARENT).unwrap();
    doc.layers[0].pixels = Arc::new((0..256).map(|v| [v as u8; 4]).collect());
    for count in 2..=64 {
        let output = gpu
            .adjust_configured(&doc, Adjustment::Posterize, [count as f32; 4])
            .unwrap();
        for (i, p) in output.iter().enumerate() {
            assert_eq!(
                *p,
                [table[(count - 2) * 256 + i]; 4],
                "level {count}, input {i}"
            );
        }
    }
    assert_eq!(
        gpu.adjust_configured(&doc, Adjustment::Posterize, [0.; 4])
            .unwrap(),
        *doc.layers[0].pixels
    );
    doc.select_rect((30, 0), (180, 0), false);
    doc.layers[0].pixels = Arc::new(
        (0..256)
            .map(|v| [v as u8, (255 - v) as u8, 80, (v / 2) as u8])
            .collect(),
    );
    for op in [Adjustment::BlackWhite, Adjustment::Sepia] {
        for intensity in [0., 50., 100.] {
            let output = gpu
                .adjust_configured(&doc, op, [intensity, 0., 0., 0.])
                .unwrap();
            for (i, p) in output.iter().enumerate() {
                let source = doc.layers[0].pixels[i];
                if !doc.selected(i) {
                    assert_eq!(*p, source);
                    continue;
                }
                let gray = (source[0] as f32 * 0.299
                    + source[1] as f32 * 0.587
                    + source[2] as f32 * 0.114)
                    / 255.;
                let expected = if op == Adjustment::BlackWhite {
                    [gray; 3]
                } else {
                    [
                        gray.powf(1. - 0.4 * intensity / 100.),
                        gray,
                        gray.powf(1. + 0.4 * intensity / 100.),
                    ]
                };
                for c in 0..3 {
                    assert!(
                        p[c].abs_diff((expected[c] * 255.).round() as u8) <= 1,
                        "{op:?}, input {i}, channel {c}"
                    );
                }
                assert_eq!(p[3], source[3]);
            }
        }
    }
    assert!(
        gpu.adjust_configured(&doc, Adjustment::Sepia, [f32::NAN; 4])
            .is_err()
    );
}

#[test]
#[ignore = "Requires a native GPU; original gradient geometry and alpha interpolation fixtures"]
fn gpu_original_gradients_and_new_image_operations() {
    use openpdn::painting::*;
    #[derive(Deserialize)]
    struct Geometry {
        shape: usize,
        repeat: usize,
        values: Vec<f32>,
    }
    #[derive(Deserialize)]
    struct Colors {
        a: Pixel,
        b: Pixel,
        values: Vec<Pixel>,
    }
    #[derive(Deserialize)]
    struct Gradients {
        geometry: Vec<Geometry>,
        colors: Vec<Colors>,
    }
    let fixture: Gradients =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-gradients.json")).unwrap();
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert_ne!(
        gpu.info.device_type,
        eframe::egui_wgpu::wgpu::DeviceType::Cpu
    );
    eprintln!(
        "Gradient/effects GPU: {} / {:?}",
        gpu.info.name, gpu.info.backend
    );
    let doc = Document::new(17, 11, TRANSPARENT).unwrap();
    for case in fixture.geometry {
        let options = GradientOptions {
            shape: GradientShape::ALL[case.shape],
            repeat: GradientRepeat::ALL[case.repeat],
            overwrite: true,
            ..Default::default()
        };
        let actual = gpu
            .gradient(&doc, (2, 3), (7, 4), [0, 0, 0, 255], [255; 4], options)
            .unwrap();
        for (i, p) in actual.iter().enumerate() {
            let expected = (case.values[i] * 255.) as u8;
            assert!(
                p[0].abs_diff(expected) <= 1,
                "gradient {}, repeat {}, xy ({},{}): {} versus {}",
                case.shape,
                case.repeat,
                i % 17,
                i / 17,
                p[0],
                expected
            );
            assert_eq!(p[3], 255);
        }
    }
    let mut doc = Document::new(256, 1, TRANSPARENT).unwrap();
    for case in fixture.colors {
        let actual = gpu
            .gradient(
                &doc,
                (0, 0),
                (255, 0),
                case.a,
                case.b,
                GradientOptions {
                    overwrite: true,
                    ..Default::default()
                },
            )
            .unwrap();
        for (i, (a, b)) in actual.iter().zip(case.values).enumerate() {
            assert_eq!(
                *a, b,
                "alpha endpoints {} / {}, weight {i}",
                case.a[3], case.b[3]
            );
        }
    }
    doc.layers[0].pixels = Arc::new(
        (0..256)
            .map(|v| [v as u8, (255 - v) as u8, 80, 128])
            .collect(),
    );
    doc.select_rect((30, 0), (180, 0), false);
    for op in [
        Adjustment::BrightnessContrast,
        Adjustment::Exposure,
        Adjustment::HueSaturation,
    ] {
        let actual = gpu.adjust(&doc, op).unwrap();
        for (i, p) in actual.iter().enumerate() {
            assert!(
                p.iter()
                    .zip(doc.layers[0].pixels[i])
                    .all(|(a, b)| a.abs_diff(b) <= 1),
                "Neutral {op:?} at {i}"
            );
        }
    }
    for contrast in [-100., -50., 0., 50., 100.] {
        for brightness in [-100., 0., 100.] {
            let output = gpu
                .adjust_configured(
                    &doc,
                    Adjustment::BrightnessContrast,
                    [brightness, contrast, 0., 0.],
                )
                .unwrap();
            for (i, p) in output.iter().enumerate() {
                let src = doc.layers[0].pixels[i];
                if !doc.selected(i) {
                    assert_eq!(*p, src);
                    continue;
                }
                let gray =
                    (0.299 * src[0] as f64 + 0.587 * src[1] as f64 + 0.114 * src[2] as f64) / 255.;
                for c in 0..3 {
                    let expected = if contrast == 100. {
                        if gray + brightness as f64 / 255. < 0.5 {
                            0.
                        } else {
                            255.
                        }
                    } else {
                        let (mul, div) = if contrast < 0. {
                            (contrast as f64 + 100., 100.)
                        } else if contrast > 0. {
                            (100., 100. - contrast as f64)
                        } else {
                            (1., 1.)
                        };
                        let (pre, post) = if div == 100. {
                            (0., brightness as f64 / 255.)
                        } else {
                            (brightness as f64 / 255., 0.)
                        };
                        (src[c] as f64 / 255. + (gray - 0.5 + pre) * mul / div + 0.5 - gray + post)
                            .clamp(0., 1.)
                            * 255.
                    };
                    assert!(
                        p[c].abs_diff(expected.round() as u8) <= 1,
                        "Brightness {brightness}, contrast {contrast}, input {i}"
                    );
                }
                assert_eq!(p[3], src[3]);
            }
        }
    }
    let hue_doc = Document::new(1, 1, [255, 0, 0, 128]).unwrap();
    assert_eq!(
        gpu.adjust_configured(&hue_doc, Adjustment::HueSaturation, [120., 100., 0., 0.])
            .unwrap(),
        vec![[0, 255, 0, 128]]
    );
    assert_eq!(
        gpu.adjust_configured(&hue_doc, Adjustment::HueSaturation, [-120., 100., 0., 0.])
            .unwrap(),
        vec![[0, 0, 255, 128]]
    );
    assert_eq!(
        gpu.adjust_configured(&hue_doc, Adjustment::HueSaturation, [0., 0., 0., 0.])
            .unwrap(),
        vec![[128, 128, 128, 128]]
    );
    for exposure in [-200., -100., 100., 200.] {
        let output = gpu
            .adjust_configured(&doc, Adjustment::Exposure, [exposure, 0., 0., 0.])
            .unwrap();
        for (i, p) in output.iter().enumerate() {
            let src = doc.layers[0].pixels[i];
            if !doc.selected(i) {
                assert_eq!(*p, src);
                continue;
            }
            for c in 0..3 {
                let s = src[c] as f64 / 255.;
                let linear = if s <= 0.04045 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                } * 2f64.powf(exposure as f64 / 100.);
                let expected = if linear <= 0.0031308 {
                    linear * 12.92
                } else {
                    1.055 * linear.powf(1. / 2.4) - 0.055
                };
                assert!(p[c].abs_diff((expected.clamp(0., 1.) * 255.).round() as u8) <= 1);
            }
            assert_eq!(p[3], src[3]);
        }
    }
    let mut image = Document::new(9, 7, [180, 120, 80, 255]).unwrap();
    image.select_rect((2, 1), (6, 5), false);
    let vignette = gpu.adjust(&image, Adjustment::Vignette).unwrap();
    assert_eq!(
        vignette[3 * 9 + 4],
        [180, 120, 80, 255],
        "Selection bounds must center the vignette"
    );
    assert!(vignette[11][0] < 180);
    for (i, p) in vignette.iter().enumerate() {
        if !image.selected(i) {
            assert_eq!(*p, [180, 120, 80, 255]);
        }
        assert_eq!(p[3], 255);
    }
    assert_eq!(
        gpu.adjust_configured(&image, Adjustment::Vignette, [0., 0., 0.5, 0.])
            .unwrap(),
        *image.layers[0].pixels
    );
    for angle in [-180., -90., 0., 45., 90., 180.] {
        let relief = gpu
            .adjust_configured(&image, Adjustment::Relief, [angle, 0., 0., 0.])
            .unwrap();
        for p in relief {
            assert!(
                p.iter()
                    .zip([180, 120, 80, 255])
                    .all(|(a, b)| a.abs_diff(b) <= 1),
                "Relief must preserve a constant field with mirrored borders"
            );
        }
    }
    let mut ramp = Document::new(5, 3, [0, 0, 0, 255]).unwrap();
    ramp.layers[0].pixels = Arc::new((0..15).map(|i| [(i % 5 * 10) as u8, 0, 0, 255]).collect());
    let relief = gpu
        .adjust_configured(&ramp, Adjustment::Relief, [0., 0., 0., 0.])
        .unwrap();
    for y in 0..3 {
        for (x, value) in [24u8, 58, 68, 78, 64].into_iter().enumerate() {
            assert!(
                relief[y * 5 + x][0].abs_diff(value) <= 1,
                "Directional Relief kernel at {x},{y}"
            );
        }
    }
    let hidden = [12, 34, 56, 0];
    assert_eq!(
        gpu.gradient(
            &ramp,
            (0, 0),
            (0, 0),
            [255; 4],
            hidden,
            GradientOptions {
                overwrite: true,
                ..Default::default()
            }
        )
        .unwrap(),
        vec![hidden; 15]
    );
    let reversed = gpu
        .gradient(
            &ramp,
            (0, 0),
            (4, 0),
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            GradientOptions {
                overwrite: true,
                reverse: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(reversed[0], [0, 0, 255, 255]);
    assert_eq!(reversed[4], [255, 0, 0, 255]);
    let output = gpu
        .gradient(
            &image,
            (2, 1),
            (6, 5),
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            GradientOptions {
                alpha_only: true,
                ..Default::default()
            },
        )
        .unwrap();
    for (i, p) in output.iter().enumerate() {
        assert_eq!(&p[..3], &[180, 120, 80]);
        if !image.selected(i) {
            assert_eq!(p[3], 255);
        }
    }
    assert!(
        gpu.adjust_configured(&image, Adjustment::Exposure, [201., 0., 0., 0.])
            .is_err()
    );
    assert!(
        gpu.adjust_configured(&image, Adjustment::Relief, [f32::INFINITY, 0., 0., 0.])
            .is_err()
    );
}
