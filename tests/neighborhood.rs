//! Independent scalar references for the recovered 5.1.12 neighborhood effects.
//! Median uses sorted order statistics and an interval index, rather than the
//! GPU's repeated population counts. Oil uses double precision linear RGB.
use openpdn::{
    document::{Document, Pixel},
    gpu::{Adjustment as Op, Gpu},
};
use std::sync::Arc;

fn fixture() -> Document {
    let mut doc = Document::new(11, 7, [0; 4]).unwrap();
    doc.layers[0].pixels = Arc::new(
        (0..77)
            .map(|i| {
                [
                    (i * 43 % 256) as u8,
                    (i * 71 % 256) as u8,
                    (i * 17 % 256) as u8,
                    if i % 5 == 0 { 0 } else { (i * 53 % 256) as u8 },
                ]
            })
            .collect(),
    );
    doc
}
fn sample(doc: &Document, x: i32, y: i32, mirrored: bool) -> Pixel {
    let edge = |v: i32, size: u32| {
        if mirrored {
            let r = v.rem_euclid(size as i32 * 2);
            r.min(size as i32 * 2 - 1 - r) as usize
        } else {
            v.clamp(0, size as i32 - 1) as usize
        }
    };
    doc.layers[0].pixels[edge(y, doc.height) * doc.width as usize + edge(x, doc.width)]
}
fn offsets(radius: i32, square: bool) -> Vec<(i32, i32)> {
    let limit = ((radius * 2 + 1).pow(2) + 2) / 4;
    (-radius..=radius)
        .flat_map(|y| {
            (-radius..=radius)
                .filter_map(move |x| (square || x * x + y * y <= limit).then_some((x, y)))
        })
        .collect()
}
fn percentile(doc: &Document, x: i32, y: i32, radius: i32, p: f32, quality: u32) -> [f64; 4] {
    let samples: Vec<_> = offsets(radius, false)
        .iter()
        .map(|(dx, dy)| sample(doc, x + dx, y + dy, false))
        .collect();
    let bounds: Vec<_> = offsets(radius, quality > 6)
        .iter()
        .map(|(dx, dy)| sample(doc, x + dx, y + dy, false))
        .collect();
    std::array::from_fn(|c| {
        let mut sorted: Vec<_> = samples.iter().map(|v| v[c]).collect();
        sorted.sort_unstable();
        if p == 0. {
            return f64::from(sorted[0]) / 255.;
        }
        if p == 1. {
            return f64::from(*sorted.last().unwrap()) / 255.;
        }
        let min = f64::from(f32::from(bounds.iter().map(|v| v[c]).min().unwrap()) / 255.);
        let max = f64::from(f32::from(bounds.iter().map(|v| v[c]).max().unwrap()) / 255.);
        // Positive normal half-float spacing; RGBA8 has no nonzero subnormals.
        let spacing = |v: f64| 2_f64.powi(v.log2().floor() as i32 - 10);
        let lo = if min == 0. {
            0.
        } else {
            (min / spacing(min)).floor() * spacing(min)
        };
        let hi = if max == 0. {
            0.
        } else {
            (max / spacing(max)).ceil() * spacing(max)
        };
        if lo == hi {
            return lo;
        }
        let target = (samples.len() as f32 * p).ceil() as usize;
        let value = f64::from(f32::from(sorted[target - 1]) / 255.);
        let subdivisions = f64::from(1u32 << quality);
        let index =
            (((value - lo) / (hi - lo) * subdivisions).ceil() - 1.).clamp(0., subdivisions - 1.);
        lo + (index + 0.5) * (hi - lo) / subdivisions
    })
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
        1.055 * v.powf(1. / 2.4) - 0.055
    }
}
fn pcg(input: u32) -> u32 {
    let state = input.wrapping_mul(747796405).wrapping_add(2891336453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277803737);
    (word >> 22) ^ word
}
fn combine(a: u32, b: u32) -> u32 {
    a.wrapping_mul(33) ^ b
}
fn pixel_seed(instance: u32, x: u32, y: u32) -> u32 {
    combine(
        instance,
        combine(
            pcg((x as f32 + 0.5).to_bits()),
            pcg((y as f32 + 0.5).to_bits()),
        ),
    )
}
fn next(seed: &mut u32) -> f32 {
    *seed = pcg(*seed);
    (*seed >> 8) as f32 / 16777216.
}
fn stochastic_oracle(doc: &Document, op: Op, v: [f32; 8]) -> Vec<Pixel> {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-noise.json")).unwrap();
    let lut: Vec<i32> = fixture["lookup_table"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_i64().unwrap() as i32)
        .collect();
    let instance = v[6] as u32 | ((v[7] as u32) << 16);
    let mut stream = instance;
    let count = if op == Op::FrostedGlass {
        [1, 2, 3, 5, 8, 13, 21, 34][v[3] as usize - 1]
    } else {
        1
    };
    let seeds: Vec<u32> = (0..count)
        .map(|_| {
            stream = pcg(stream);
            (stream & 0x7fffffff) ^ (instance & 255)
        })
        .collect();
    (0..doc.width * doc.height)
        .map(|i| {
            let original = doc.layers[0].pixels[i as usize];
            if !doc.selected(i as usize) || v[0] == 0. {
                return original;
            }
            let x = i % doc.width;
            let y = i / doc.width;
            if op == Op::AddNoise {
                let mut seed = pixel_seed(instance ^ v[3] as u32, x, y);
                if next(&mut seed) > v[2] / 100. {
                    return original;
                }
                let channels: [i32; 3] =
                    std::array::from_fn(|_| lut[(next(&mut seed) * 16384.) as usize]);
                let gray = (4899 * channels[0] + 9618 * channels[1] + 1867 * channels[2]) >> 14;
                let sat = ((v[1] / 100.) * 4096.) as i32;
                let intensity = (v[0] / 100.) * 100.;
                let dev = (intensity * intensity / 4.) as i32;
                std::array::from_fn(|c| {
                    if c == 3 {
                        original[3]
                    } else {
                        let noise = gray + (((channels[c] - gray) * sat) >> 12);
                        (i32::from(original[c]) + ((noise * dev + 32768) >> 16)).clamp(0, 255) as u8
                    }
                })
            } else {
                let mut sum = [0.; 4];
                for instance in &seeds {
                    let mut seed = pixel_seed(*instance ^ v[4] as u32, x, y);
                    let angle = f64::from(next(&mut seed) * std::f32::consts::TAU);
                    let t = next(&mut seed);
                    let min = v[1].powf(v[2]);
                    let max = v[0].powf(v[2]);
                    let radius = f64::from((min + t * (max - min)).abs().powf(1. / v[2]));
                    let at = [
                        f64::from(x) + radius * angle.cos(),
                        f64::from(y) + radius * angle.sin(),
                    ];
                    let lo = at.map(|n| n.floor() as i32);
                    let frac = at.map(|n| n - n.floor());
                    for dy in 0..=1 {
                        for dx in 0..=1 {
                            let c = sample(doc, lo[0] + dx, lo[1] + dy, true)
                                .map(|b| f64::from(b) / 255.);
                            let weight = (if dx == 0 { 1. - frac[0] } else { frac[0] })
                                * (if dy == 0 { 1. - frac[1] } else { frac[1] });
                            for k in 0..3 {
                                sum[k] += linear(c[k]) * c[3] * weight;
                            }
                            sum[3] += c[3] * weight;
                        }
                    }
                }
                let alpha = sum[3];
                for c in &mut sum[..3] {
                    *c = if alpha > 0. { srgb(*c / alpha) } else { 0. };
                }
                sum[3] /= seeds.len() as f64;
                sum.map(|c| (c.clamp(0., 1.) * 255.).round() as u8)
            }
        })
        .collect()
}
fn oracle(doc: &Document, op: Op, v: [f32; 8]) -> Vec<Pixel> {
    (0..doc.width * doc.height)
        .map(|i| {
            let x = (i % doc.width) as i32;
            let y = (i / doc.width) as i32;
            let center = doc.layers[0].pixels[i as usize].map(|b| f64::from(b) / 255.);
            let mut result = match op {
                Op::Median => percentile(doc, x, y, v[0] as i32, v[1] / 100., v[2] as u32),
                Op::Outline => {
                    let r = (v[0] as i32).min(50);
                    let lo = percentile(doc, x, y, r, (1. - v[1] / 100.) * 0.5, v[2] as u32);
                    let hi = percentile(doc, x, y, r, (1. + v[1] / 100.) * 0.5, v[2] as u32);
                    [
                        1. - (hi[0] - lo[0]),
                        1. - (hi[1] - lo[1]),
                        1. - (hi[2] - lo[2]),
                        hi[3],
                    ]
                }
                Op::OilPainting => {
                    let bins = ((f64::from(v[1]) * 256.) as usize).clamp(4, 256);
                    let samples: Vec<_> = offsets(v[0] as i32, v[2] != 0.)
                        .iter()
                        .map(|(dx, dy)| {
                            let p = sample(doc, x + dx, y + dy, false).map(|b| f64::from(b) / 255.);
                            [linear(p[0]), linear(p[1]), linear(p[2]), p[3]]
                        })
                        .collect();
                    let bin = |s: &[f64; 4]| {
                        ((s[0] * 0.299 + s[1] * 0.587 + s[2] * 0.114).clamp(0., 1.)
                            * (bins - 1) as f64) as usize
                    };
                    let mut histogram = [0; 256];
                    for s in &samples {
                        histogram[bin(s)] += 1;
                    }
                    let mode = (0..bins)
                        .max_by_key(|b| (histogram[*b], std::cmp::Reverse(*b)))
                        .unwrap();
                    let mut sum = [0.; 4];
                    for s in samples.iter().filter(|s| bin(s) == mode) {
                        for c in 0..3 {
                            sum[c] += s[c] * s[3];
                        }
                        sum[3] += s[3];
                    }
                    let alpha = sum[3];
                    for c in &mut sum[..3] {
                        *c = if alpha > 0. { srgb(*c / alpha) } else { 0. };
                    }
                    sum[3] /= f64::from(histogram[mode]);
                    sum
                }
                Op::ReduceNoise => {
                    let samples = offsets(v[0] as i32, false);
                    let mut below = [0.; 4];
                    for (dx, dy) in &samples {
                        let s = sample(doc, x + dx, y + dy, true).map(|b| f64::from(b) / 255.);
                        for c in 0..4 {
                            below[c] += f64::from(s[c] < center[c]);
                        }
                    }
                    let factor = -0.2
                        * f64::from(v[1])
                        * (1. - 0.75 * (center[0] * 0.299 + center[1] * 0.587 + center[2] * 0.114));
                    std::array::from_fn(|c| {
                        if c == 3 {
                            center[3]
                        } else {
                            center[c] + (below[c] / samples.len() as f64 - center[c]) * factor
                        }
                    })
                }
                _ => unreachable!(),
            };
            if !doc.selected(i as usize) {
                return doc.layers[0].pixels[i as usize];
            }
            if (op == Op::Median && v[0] == 0.)
                || (op == Op::ReduceNoise && (v[0] == 0. || v[1] == 0.))
            {
                result = center;
            }
            result.map(|c| (c.clamp(0., 1.) * 255.).round() as u8)
        })
        .collect()
}
fn compare(actual: &[Pixel], expected: &[Pixel], op: Op, v: [f32; 8]) {
    for (i, (a, b)) in actual.iter().zip(expected).enumerate() {
        for c in 0..4 {
            assert!(
                (i32::from(a[c]) - i32::from(b[c])).abs() <= 1,
                "{op:?} {v:?}, pixel {i}: {a:?} vs {b:?}"
            );
        }
    }
}
#[test]
#[ignore = "Requires native GPU; recovered neighborhood equations and alpha rules"]
fn hardware_neighborhood_effects() {
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert_ne!(
        gpu.info.device_type,
        eframe::egui_wgpu::wgpu::DeviceType::Cpu
    );
    eprintln!(
        "Neighborhood filters: {} / {:?}",
        gpu.info.name, gpu.info.backend
    );
    let mut doc = fixture();
    check_stochastic(&gpu, &doc);
    for (op, configs) in [
        (
            Op::Median,
            vec![
                [2., 0., 8., 0.],
                [2., 100., 8., 0.],
                [2., 50., 1., 0.],
                [3., 25., 6., 0.],
                [3., 75., 7., 0.],
                [3., 50., 8., 0.],
                [3., 50., 9., 0.],
            ],
        ),
        (
            Op::Outline,
            vec![
                [2., 0., 8., 0.],
                [2., 100., 8., 0.],
                [3., 50., 7., 0.],
                [3., 50., 8., 0.],
            ],
        ),
        (
            Op::OilPainting,
            vec![
                [1., 0.01, 0., 0.],
                [2., 0.2, 0., 0.],
                [3., 0.2, 1., 0.],
                [2., 1., 0., 0.],
            ],
        ),
        (
            Op::ReduceNoise,
            vec![
                [1., 0.4, 0., 0.],
                [3., 1., 0., 0.],
                [0., 1., 0., 0.],
                [5., 0., 0., 0.],
            ],
        ),
    ] {
        for config in configs {
            let mut v = op.parameters();
            v[..4].copy_from_slice(&config);
            let actual = gpu.adjust_parameters(&doc, op, v).unwrap();
            compare(&actual, &oracle(&doc, op, v), op, v);
        }
    }
    doc.select_rect((3, 2), (7, 5), false);
    for op in [Op::Median, Op::Outline, Op::OilPainting, Op::ReduceNoise] {
        let mut v = op.parameters();
        v[0] = 3.;
        let actual = gpu.adjust_parameters(&doc, op, v).unwrap();
        compare(&actual, &oracle(&doc, op, v), op, v);
        for (i, pixel) in actual.iter().enumerate() {
            if !doc.selected(i) {
                assert_eq!(*pixel, doc.layers[0].pixels[i]);
            }
        }
        v[0] = op.controls()[0].2 + 1.;
        assert!(gpu.adjust_parameters(&doc, op, v).is_err());
        v = op.parameters();
        v[0] = 1.5;
        assert!(gpu.adjust_parameters(&doc, op, v).is_err());
        v = op.parameters();
        v[1] = f32::NAN;
        assert!(gpu.adjust_parameters(&doc, op, v).is_err());
    }
    // Half-float interval search has observable finite quality even on a flat
    // image; transparent RGB must survive the independent-channel operations.
    let flat = Document::new(1, 1, [72, 114, 189, 0]).unwrap();
    for op in [Op::Median, Op::Outline, Op::OilPainting, Op::ReduceNoise] {
        let mut v = op.parameters();
        v[0] = op.controls()[0].2;
        compare(
            &gpu.adjust_parameters(&flat, op, v).unwrap(),
            &oracle(&flat, op, v),
            op,
            v,
        );
    }
    let a = gpu
        .adjust_configured(&doc, Op::Outline, [50., 50., 8., 0.])
        .unwrap();
    assert_eq!(
        a,
        gpu.adjust_configured(&doc, Op::Outline, [70., 50., 8., 0.])
            .unwrap()
    );
    let large = Document::new(800, 600, [72, 114, 189, 255]).unwrap();
    for op in [Op::Median, Op::Outline, Op::OilPainting, Op::ReduceNoise] {
        let start = std::time::Instant::now();
        gpu.adjust_parameters(&large, op, op.parameters()).unwrap();
        eprintln!("{op:?} 800x600 default: {:?}", start.elapsed());
    }
}

fn check_stochastic(gpu: &Gpu, doc: &Document) {
    for op in [Op::AddNoise, Op::FrostedGlass] {
        let configs = if op == Op::AddNoise {
            vec![
                [64., 100., 100., 0., 0., 0., 0., 0.],
                [100., 400., 100., 255., 0., 0., 48879., 57005.],
                [43., 0., 37.5, 17., 0., 0., 12345., 56789.],
                [100., 100., 0., 0., 0., 0., 0., 0.],
                [0., 400., 100., 0., 0., 0., 0., 0.],
            ]
        } else {
            vec![
                [3., 0., 1., 1., 0., 0., 0., 0.],
                [5.5, 2.25, 1.7, 4., 255., 0., 48879., 57005.],
                [500., 350., 0.01, 8., 77., 0., 12345., 56789.],
                [0., 0., 1., 2., 0., 0., 0., 0.],
            ]
        };
        for config in configs {
            let mut full = fixture();
            let result = gpu.adjust_parameters(&full, op, config).unwrap();
            compare(&result, &stochastic_oracle(&full, op, config), op, config);
            assert_eq!(
                result,
                gpu.adjust_parameters(&full, op, config).unwrap(),
                "random preview must retain its seed"
            );
            full.select_rect((3, 2), (7, 5), false);
            compare(
                &gpu.adjust_parameters(&full, op, config).unwrap(),
                &stochastic_oracle(&full, op, config),
                op,
                config,
            );
        }
        let mut a = op.parameters();
        let first = gpu.adjust_parameters(doc, op, a).unwrap();
        a[if op == Op::AddNoise { 3 } else { 4 }] = 1.;
        assert_ne!(first, gpu.adjust_parameters(doc, op, a).unwrap());
        a[6] = 65536.;
        assert!(gpu.adjust_parameters(doc, op, a).is_err());
    }
    assert!(
        gpu.adjust_configured(doc, Op::FrostedGlass, [2., 3., 1., 2.])
            .is_err()
    );
}
