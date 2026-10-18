use openpdn::{
    document::{Document, Pixel},
    gpu::{Adjustment, Gpu, gaussian::GaussianPlan},
};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Deserialize)]
struct NativePlan {
    quality: u32,
    sigma: f64,
    pre: f32,
    post: f32,
    matrices: [[f32; 6]; 2],
    taps: Vec<[f32; 4]>,
}
fn fixtures() -> Vec<NativePlan> {
    serde_json::from_str(include_str!("fixtures/native-gaussian.json")).unwrap()
}
#[test]
fn native_gaussian_plans_and_paired_kernels() {
    let cases = fixtures();
    assert_eq!(cases.len(), 45);
    for case in cases {
        let plan = GaussianPlan::new(case.sigma as f32, case.quality).unwrap();
        assert_eq!((plan.pre, plan.post), (case.pre, case.post));
        assert_eq!(plan.matrices(), case.matrices);
        assert_eq!(plan.taps.len(), case.taps.len());
        for (actual, expected) in plan.taps.iter().zip(&case.taps) {
            for (a, b) in actual.iter().zip(expected) {
                assert!(
                    (a - b).abs() <= 0.000001,
                    "Quality {} sigma {}: {actual:?} != {expected:?}",
                    case.quality,
                    case.sigma
                );
            }
        }
    }
    for (sigma, quality) in [(f32::NAN, 1), (-1., 1), (100.1, 1), (2., 0), (2., 4)] {
        assert!(GaussianPlan::new(sigma, quality).is_err());
    }
}
type Color = [f64; 4];
fn mix(a: Color, b: Color, t: f64) -> Color {
    std::array::from_fn(|i| a[i] * (1. - t) + b[i] * t)
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
fn mirror(x: i32, n: u32) -> usize {
    let n = n as i32;
    let r = x.rem_euclid(n * 2);
    if r >= n {
        (n * 2 - 1 - r) as usize
    } else {
        r as usize
    }
}
fn cubic(x: f64) -> f64 {
    let x = x.abs();
    if x < 1. {
        1.5 * x * x * x - 2.5 * x * x + 1.
    } else if x < 2. {
        -0.5 * x * x * x + 2.5 * x * x - 4. * x + 2.
    } else {
        0.
    }
}
/// Independent double-precision graph, evaluated lazily without finite buffer
/// boundaries. Parameters/taps come from the native numeric fixture.
struct Oracle<'a> {
    doc: &'a Document,
    plan: NativePlan,
    gamma: f64,
    pre: HashMap<(i32, i32), Color>,
    horizontal: HashMap<(i32, i32), Color>,
    vertical: HashMap<(i32, i32), Color>,
}
impl Oracle<'_> {
    fn original(&self, x: i32, y: i32) -> Color {
        let c = self.doc.layers[0].pixels
            [mirror(y, self.doc.height) * self.doc.width as usize + mirror(x, self.doc.width)];
        let a = f64::from(c[3]) / 255.;
        [
            linear(f64::from(c[0]) / 255.).powf(self.gamma) * a,
            linear(f64::from(c[1]) / 255.).powf(self.gamma) * a,
            linear(f64::from(c[2]) / 255.).powf(self.gamma) * a,
            a,
        ]
    }
    fn value(&mut self, stage: u8, x: i32, y: i32) -> Color {
        let cached = match stage {
            0 => self.pre.get(&(x, y)),
            1 => self.horizontal.get(&(x, y)),
            _ => self.vertical.get(&(x, y)),
        };
        if let Some(&value) = cached {
            return value;
        }
        let result = if stage == 0 {
            let sx = (f64::from(x) + 0.5) / f64::from(self.plan.pre) - 0.5;
            let sy = (f64::from(y) + 0.5) / f64::from(self.plan.pre) - 0.5;
            let ix = sx.floor() as i32;
            let iy = sy.floor() as i32;
            mix(
                mix(
                    self.original(ix, iy),
                    self.original(ix + 1, iy),
                    sx - sx.floor(),
                ),
                mix(
                    self.original(ix, iy + 1),
                    self.original(ix + 1, iy + 1),
                    sx - sx.floor(),
                ),
                sy - sy.floor(),
            )
        } else {
            let center = if stage == 1 {
                [
                    f64::from(x) / f64::from(self.plan.post) + 0.5,
                    f64::from(y) + 0.5,
                ]
            } else {
                [
                    f64::from(x) + 0.5,
                    f64::from(y) / f64::from(self.plan.post) + 0.5,
                ]
            };
            let mut sum = [0.; 4];
            for n in 0..self.plan.taps.len() {
                let tap = self.plan.taps[n];
                for sign in [-1., 1.] {
                    let mut point = center;
                    point[usize::from(stage == 2)] += sign * f64::from(tap[0]);
                    let value = self.sample(stage - 1, point);
                    for i in 0..4 {
                        sum[i] += value[i] * f64::from(tap[2]);
                    }
                }
            }
            sum
        };
        match stage {
            0 => self.pre.insert((x, y), result),
            1 => self.horizontal.insert((x, y), result),
            _ => self.vertical.insert((x, y), result),
        };
        result
    }
    fn sample(&mut self, stage: u8, point: [f64; 2]) -> Color {
        let xy = point.map(|x| x - 0.5);
        let base = xy.map(|x| x.floor() as i32);
        let t = xy.map(|x| x - x.floor());
        let a = self.value(stage, base[0], base[1]);
        let b = self.value(stage, base[0] + 1, base[1]);
        let c = self.value(stage, base[0], base[1] + 1);
        let d = self.value(stage, base[0] + 1, base[1] + 1);
        mix(mix(a, b, t[0]), mix(c, d, t[0]), t[1])
    }
    fn output(&mut self, x: u32, y: u32) -> Pixel {
        let index = (y * self.doc.width + x) as usize;
        if !self.doc.selected(index) {
            return self.doc.layers[0].pixels[index];
        }
        let point = [x, y].map(|n| {
            (f64::from(n) + 0.5 - f64::from(self.plan.matrices[1][4]))
                / f64::from(self.plan.matrices[1][0])
        });
        let value = if self.plan.quality == 1 {
            self.sample(2, point)
        } else {
            let xy = point.map(|x| x - 0.5);
            let base = xy.map(|x| x.floor() as i32);
            let t = xy.map(|x| x - x.floor());
            let mut sum = [0.; 4];
            for y in -1..=2 {
                for x in -1..=2 {
                    let weight = cubic(f64::from(x) - t[0]) * cubic(f64::from(y) - t[1]);
                    let c = self.value(2, base[0] + x, base[1] + y);
                    for i in 0..4 {
                        sum[i] += c[i] * weight;
                    }
                }
            }
            sum
        };
        std::array::from_fn(|i| {
            let v = if i == 3 {
                value[3]
            } else if value[3] > 1e-6 {
                srgb((value[i] / value[3]).max(0.).powf(1. / self.gamma))
            } else {
                0.
            };
            (v.clamp(0., 1.) * 255.).round() as u8
        })
    }
}
#[test]
#[ignore = "Requires a hardware Vulkan, D3D12 or Metal adapter"]
fn gaussian_all_quality_modes_on_hardware() {
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert_ne!(gpu.info.device_type, eframe::wgpu::DeviceType::Cpu);
    let mut doc = Document::new(17, 13, [0; 4]).unwrap();
    doc.layers[0].pixels = Arc::new(
        (0..221)
            .map(|i| {
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
            })
            .collect(),
    );
    doc.selection = Some(Arc::new(
        (0..221)
            .map(|i| if i % 11 == 0 { 0 } else { 255 })
            .collect(),
    ));
    for plan in fixtures()
        .into_iter()
        .filter(|p| [1., 3., 10., 100.].contains(&p.sigma))
    {
        for gamma in [1., 1.7] {
            let mut values = Adjustment::GaussianBlur.parameters();
            values[0] = plan.sigma as f32 * 3.;
            values[1] = gamma as f32 - 1.;
            values[2] = plan.quality as f32;
            let actual = gpu
                .adjust_parameters(&doc, Adjustment::GaussianBlur, values)
                .unwrap();
            let mut oracle = Oracle {
                doc: &doc,
                plan: plan.clone(),
                gamma,
                pre: HashMap::new(),
                horizontal: HashMap::new(),
                vertical: HashMap::new(),
            };
            for y in 0..doc.height {
                for x in 0..doc.width {
                    let expected = oracle.output(x, y);
                    let index = (y * doc.width + x) as usize;
                    for channel in 0..4 {
                        assert!(
                            actual[index][channel].abs_diff(expected[channel]) <= 1,
                            "quality {} sigma {} gamma {gamma}, {x},{y}: {:?} != {expected:?}",
                            plan.quality,
                            plan.sigma,
                            actual[index]
                        );
                    }
                }
            }
        }
    }
    for quality in 1..=4 {
        let flat = Document::new(1, 1, [40, 90, 170, 128]).unwrap();
        let mut values = Adjustment::GaussianBlur.parameters();
        values[2] = quality as f32;
        for radius in [0., 0.01, 4.5, 6., 30., 300.] {
            values[0] = radius;
            assert_eq!(
                gpu.adjust_parameters(&flat, Adjustment::GaussianBlur, values)
                    .unwrap(),
                *flat.layers[0].pixels
            );
        }
    }
    let mut outputs = Vec::new();
    for quality in 1..=4 {
        let mut values = Adjustment::GaussianBlur.parameters();
        values[0] = 9.;
        values[2] = quality as f32;
        outputs.push(
            gpu.adjust_parameters(&doc, Adjustment::GaussianBlur, values)
                .unwrap(),
        );
    }
    for pair in outputs.windows(2) {
        assert_ne!(
            pair[0], pair[1],
            "Quality choices must execute their respective paths"
        );
    }
}
