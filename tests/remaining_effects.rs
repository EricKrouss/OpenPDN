//! GPU checks for the final fourteen entries. Reference fixtures validate
//! original kernels/tables separately; these tests also exercise transactions'
//! selection boundary, deterministic repeats and numerical corner cases.
use openpdn::{
    document::{Document, Pixel},
    gpu::{Adjustment as Op, Gpu},
};
use std::{collections::BTreeSet, sync::Arc};
fn hardware() -> Gpu {
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert_ne!(
        gpu.info.device_type,
        eframe::wgpu::DeviceType::Cpu,
        "Software adapter cannot prove hardware acceleration"
    );
    gpu
}
fn image() -> Document {
    let mut doc = Document::new(23, 19, [0; 4]).unwrap();
    doc.layers[0].pixels = Arc::new(
        (0_i32..437)
            .map(|i| {
                let x = i % 23;
                let y = i / 23;
                if (x - 11).pow(2) + (y - 9).pow(2) <= 9 {
                    [245, 18, 26, 255]
                } else {
                    [
                        (i * 37 % 256) as u8,
                        (i * 71 % 256) as u8,
                        (i * 13 % 256) as u8,
                        if i % 7 == 0 {
                            0
                        } else if i % 5 == 0 {
                            128
                        } else {
                            255
                        },
                    ]
                }
            })
            .collect(),
    );
    doc
}
fn cases() -> Vec<(Op, [f32; 8])> {
    [
        Op::Bokeh,
        Op::SketchBlur,
        Op::Quantize,
        Op::Crystalize,
        Op::Dents,
        Op::Pixelate,
        Op::RedEye,
        Op::Sharpen,
        Op::Straighten,
        Op::Clouds,
        Op::Julia,
        Op::Mandelbrot,
        Op::Turbulence,
        Op::EdgeDetect,
    ]
    .into_iter()
    .map(|op| {
        let mut v = op.parameters();
        match op {
            Op::Bokeh => {
                v[0] = 2.5;
                v[1] = 0.3;
            }
            Op::SketchBlur => {
                v[0] = 4.25;
                v[1] = 35.;
                v[2] = 2.;
            }
            Op::Quantize => v[1] = 6.,
            Op::Crystalize => {
                v[0] = 4.;
                v[7] = 17.;
            }
            Op::Dents => {
                v[1] = 80.;
                v[2] = 30.;
                v[3] = 20.;
                v[4] = 32.;
                v[7] = 7.;
            }
            Op::Pixelate => {
                v[0] = 4.;
                v[1] = 0.2;
            }
            Op::Straighten => v[0] = 12.,
            Op::Clouds => {
                v[0] = 7.;
                v[1] = 0.7;
                v[2] = 2.;
                v[4] = 0xff0000 as f32;
                v[5] = 128.;
                v[6] = 255.;
            }
            Op::Mandelbrot => {
                v[0] = 2.;
                v[1] = 0.3;
            }
            Op::Turbulence => {
                v[1] = 5.;
                v[2] = 16.;
                v[4] = 17.;
                v[5] = 2.;
            }
            Op::EdgeDetect => {
                v[1] = 0.8;
                v[2] = 1.;
                v[3] = 1.;
            }
            _ => {}
        }
        (op, v)
    })
    .collect()
}
fn byte(v: f64) -> u8 {
    (v * 255.).round().clamp(0., 255.) as u8
}
fn linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(v: f64) -> f64 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.max(0.).powf(1. / 2.4) - 0.055
    }
}
fn close(a: &[Pixel], b: &[Pixel], tolerance: u8, label: &str) {
    for (i, (a, b)) in a.iter().zip(b).enumerate() {
        for c in 0..4 {
            assert!(
                a[c].abs_diff(b[c]) <= tolerance,
                "{label} pixel {i} {a:?} != {b:?}"
            );
        }
    }
}
fn bokeh_oracle(doc: &Document) -> Vec<Pixel> {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-bokeh.json")).unwrap();
    let case = data
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["radius"] == 2.5 && v["quality"] == 3)
        .unwrap();
    let kernels = case["kernels"].as_array().unwrap();
    let parameters: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-effect-tables.json")).unwrap();
    let params = parameters["bokeh_parameters"][2].as_array().unwrap();
    let mirror = |v: i32, size: u32| {
        let r = v.rem_euclid(size as i32 * 2);
        r.min(size as i32 * 2 - 1 - r) as usize
    };
    (0..doc.width * doc.height)
        .map(|i| {
            let x = (i % doc.width) as i32;
            let y = (i / doc.width) as i32;
            let mut result = [0.; 4];
            for (component, k) in kernels.iter().enumerate() {
                let real = k["real"].as_array().unwrap();
                let imag = k["imaginary"].as_array().unwrap();
                let z = params[component][2].as_f64().unwrap();
                let w = params[component][3].as_f64().unwrap();
                let n = (real.len() / 2) as i32;
                for dy in -n..=n {
                    for dx in -n..=n {
                        let ix = (dx + n) as usize;
                        let iy = (dy + n) as usize;
                        let rx = real[ix].as_f64().unwrap();
                        let ry = real[iy].as_f64().unwrap();
                        let ax = imag[ix].as_f64().unwrap();
                        let ay = imag[iy].as_f64().unwrap();
                        let weight = z * (rx * ry - ax * ay) + w * (rx * ay + ax * ry);
                        let pixel = doc.layers[0].pixels[mirror(y + dy, doc.height)
                            * doc.width as usize
                            + mirror(x + dx, doc.width)];
                        let alpha = f64::from(pixel[3]) / 255.;
                        for c in 0..3 {
                            result[c] +=
                                linear(f64::from(pixel[c]) / 255.).powf(1.3) * alpha * weight;
                        }
                        result[3] += alpha * weight;
                    }
                }
            }
            let alpha = result[3];
            let mut out = [byte(alpha); 4];
            for c in 0..3 {
                out[c] = byte(srgb((result[c] / alpha.max(1e-20)).max(0.).powf(1. / 1.3)));
            }
            out
        })
        .collect()
}
#[test]
#[ignore = "Requires an explicit hardware GPU run"]
fn hardware_remaining_effects() {
    let gpu = hardware();
    eprintln!(
        "Remaining effects: {} / {:?}",
        gpu.info.name, gpu.info.backend
    );
    let doc = image();
    for (op, v) in cases() {
        let result = gpu.adjust_parameters(&doc, op, v).unwrap();
        assert_eq!(result.len(), 437);
        assert_ne!(
            result.as_slice(),
            doc.layers[0].pixels.as_slice(),
            "{} inert",
            op.name()
        );
        assert_eq!(
            result,
            gpu.adjust_parameters(&doc, op, v).unwrap(),
            "{} unstable repeat",
            op.name()
        );
        let mut clipped = doc.clone();
        clipped.select_rect((4, 4), (18, 15), false);
        let masked = gpu.adjust_parameters(&clipped, op, v).unwrap();
        for (i, pixel) in masked.iter().enumerate() {
            if !clipped.selected(i) {
                assert_eq!(
                    *pixel,
                    doc.layers[0].pixels[i],
                    "{} selection leak",
                    op.name()
                );
            }
        }
        eprintln!("{} compute/selection/repeat passed", op.name());
    }
    let (op, values) = cases()[0];
    close(
        &gpu.adjust_parameters(&doc, op, values).unwrap(),
        &bokeh_oracle(&doc),
        2,
        "original Bokeh kernel direct 2D oracle",
    );
    for op in [
        Op::Bokeh,
        Op::SketchBlur,
        Op::Dents,
        Op::Sharpen,
        Op::Straighten,
    ] {
        let mut v = op.parameters();
        v[0] = 0.;
        assert_eq!(
            gpu.adjust_parameters(&doc, op, v).unwrap(),
            *doc.layers[0].pixels,
            "{} neutral",
            op.name()
        );
    }
    for down in 0..6 {
        for up in 0..3 {
            let mut v = Op::Pixelate.parameters();
            v[0] = 3.;
            v[3] = down as f32;
            v[4] = up as f32;
            assert_eq!(
                gpu.adjust_parameters(&doc, Op::Pixelate, v).unwrap().len(),
                437
            );
        }
    }
    let tiny = Document::new(1, 1, [23, 57, 98, 128]).unwrap();
    for op in [
        Op::Pixelate,
        Op::Crystalize,
        Op::Dents,
        Op::Bokeh,
        Op::SketchBlur,
        Op::Straighten,
    ] {
        let mut v = op.parameters();
        if op == Op::Straighten {
            v[0] = 45.;
        }
        let result = gpu.adjust_parameters(&tiny, op, v).unwrap();
        close(&result, &[[23, 57, 98, 128]], 1, op.name());
    }
    for percentile in [0., 1., 49., 50., 51., 99., 100.] {
        let mut v = Op::SketchBlur.parameters();
        v[0] = 25.;
        v[1] = percentile;
        assert_eq!(
            gpu.adjust_parameters(&doc, Op::SketchBlur, v)
                .unwrap()
                .len(),
            437
        );
    }
    for algorithm in [0., 1.] {
        for dither in [0., 1., 7., 8.] {
            let mut v = Op::Quantize.parameters();
            v[0] = algorithm;
            v[1] = 6.;
            v[2] = dither;
            let result = gpu.adjust_parameters(&doc, Op::Quantize, v).unwrap();
            assert!(result.iter().copied().collect::<BTreeSet<_>>().len() <= 6);
            assert!(result.iter().all(|c| c[3] == 0 || c[3] == 255));
        }
    }
    let mut v = Op::Turbulence.parameters();
    v[1] = 3.;
    v[2] = 8.;
    v[5] = 2.;
    let tile = gpu.adjust_parameters(&doc, Op::Turbulence, v).unwrap();
    for y in 0..19 {
        for x in 0..15 {
            assert_eq!(tile[y * 23 + x], tile[y * 23 + x + 8]);
        }
    }
    v[3] = 1.;
    assert_ne!(
        tile,
        gpu.adjust_parameters(&doc, Op::Turbulence, v).unwrap()
    );
    for op in [Op::Clouds, Op::Julia, Op::Mandelbrot, Op::Turbulence] {
        for mode in 0..46 {
            let mut v = op.parameters();
            v[if op == Op::Clouds {
                2
            } else if op == Op::Julia {
                4
            } else {
                5
            }] = mode as f32;
            let out = gpu.adjust_parameters(&doc, op, v).unwrap();
            assert_eq!(out.len(), 437);
        }
    }
}

#[test]
#[ignore = "Requires an explicit hardware GPU run"]
fn hardware_native_red_eye() {
    #[derive(serde::Deserialize)]
    struct Case {
        width: u32,
        height: u32,
        radius: u32,
        background: u32,
        strength: u32,
        input: Vec<Pixel>,
        output: Vec<Pixel>,
    }
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/native-redeye.json")).unwrap();
    let gpu = hardware();
    for case in cases {
        let mut doc = Document::new(case.width, case.height, [0; 4]).unwrap();
        doc.layers[0].pixels = Arc::new(case.input);
        let mut v = Op::RedEye.parameters();
        v[0] = case.strength as f32;
        let result = gpu.adjust_parameters(&doc, Op::RedEye, v).unwrap();
        close(
            &result,
            &case.output,
            1,
            &format!(
                "native Red Eye radius {} background {} strength {}",
                case.radius, case.background, case.strength
            ),
        );
    }
}

#[test]
#[ignore = "Requires an explicit hardware GPU run"]
fn hardware_red_eye_selection() {
    let gpu = hardware();
    let mut doc = Document::new(45, 35, [170, 140, 120, 255]).unwrap();
    for y in 7_i32..28 {
        for x in 7_i32..38 {
            if ((x - 16).pow(2) + (y - 17).pow(2) < 16) || ((x - 30).pow(2) + (y - 17).pow(2) < 16)
            {
                Arc::make_mut(&mut doc.layers[0].pixels)[(y * 45 + x) as usize][..3]
                    .copy_from_slice(&[245, 18, 26]);
            }
        }
    }
    // Two components and holes: intermediate area correction must never leak
    // beyond the final mask, including on repeated strength passes.
    let mut mask = vec![0; 45 * 35];
    for y in 5..30 {
        for x in 5..40 {
            if !(22..=24).contains(&x) {
                mask[y * 45 + x] = 255;
            }
        }
    }
    mask[17 * 45 + 16] = 0;
    mask[17 * 45 + 30] = 0;
    doc.selection = Some(Arc::new(mask));
    let original = doc.layers[0].pixels.clone();
    let result = gpu
        .adjust_parameters(&doc, Op::RedEye, Op::RedEye.parameters())
        .unwrap();
    assert_ne!(result.as_slice(), original.as_slice());
    for (i, pixel) in result.iter().enumerate() {
        if !doc.selected(i) {
            assert_eq!(*pixel, original[i], "mask leaked at {i}");
        }
    }
    let mut hidden = doc.clone();
    hidden.layers[0].pixels = Arc::new(vec![[245, 18, 26, 0]; 45 * 35]);
    assert_eq!(
        gpu.adjust_parameters(&hidden, Op::RedEye, Op::RedEye.parameters())
            .unwrap(),
        *hidden.layers[0].pixels
    );
}
