//! Small CPU-side sampling kernels; image convolution stays on the GPU.
use super::effect_data::BOKEH;

pub(super) fn bokeh(radius: f64, quality: usize) -> Vec<[f32; 4]> {
    let n = radius.ceil() as i32;
    let size = (2 * n + 1) as usize;
    let components = BOKEH[quality - 1];
    let scale = if quality == 1 { 1.4 } else { 1.2 };
    let mut kernels = Vec::new();
    let mut total = 0.;
    for &[a, b, z, w] in components {
        let mut values = vec![[0_f64; 2]; size];
        for step in -n * 1024..=n * 1024 {
            let position = f64::from(step) / 1024.;
            if position.abs() > radius {
                continue;
            }
            let x = position * scale / radius;
            let decay = (-a * x * x).exp();
            let re = decay * (b * x * x).cos();
            let im = decay * (b * x * x).sin();
            let lo = position.floor() as i32;
            let fraction = position - f64::from(lo);
            values[(lo + n) as usize][0] += re * (1. - fraction);
            values[(lo + n) as usize][1] += im * (1. - fraction);
            if fraction != 0. {
                values[(lo + n + 1) as usize][0] += re * fraction;
                values[(lo + n + 1) as usize][1] += im * fraction;
            }
        }
        let re: f64 = values.iter().map(|v| v[0]).sum();
        let im: f64 = values.iter().map(|v| v[1]).sum();
        total += z * (re * re - im * im) + w * (2. * re * im);
        kernels.push(values);
    }
    let norm = total.sqrt().recip();
    kernels
        .iter()
        .enumerate()
        .flat_map(|(c, values)| {
            values.iter().map(move |v| {
                [
                    (v[0] * norm) as f32,
                    (v[1] * norm) as f32,
                    components[c][2] as f32,
                    components[c][3] as f32,
                ]
            })
        })
        .collect()
}

// Seeded System.Random's subtractive stream (the original uses seed=iteration).
struct LegacyRandom {
    state: [i32; 56],
    next: usize,
    next_p: usize,
}
impl LegacyRandom {
    fn new(seed: i32) -> Self {
        let mut state = [0; 56];
        let mut mj = 161803398 - seed;
        state[55] = mj;
        let mut mk = 1;
        for i in 1..55 {
            let ii = 21 * i % 55;
            state[ii] = mk;
            mk = mj - mk;
            if mk < 0 {
                mk += i32::MAX;
            }
            mj = state[ii];
        }
        for _ in 0..4 {
            for i in 1..56 {
                state[i] -= state[1 + (i + 30) % 55];
                if state[i] < 0 {
                    state[i] += i32::MAX;
                }
            }
        }
        Self {
            state,
            next: 0,
            next_p: 21,
        }
    }
    fn next(&mut self) -> f64 {
        self.next = self.next % 55 + 1;
        self.next_p = self.next_p % 55 + 1;
        let mut v = self.state[self.next] - self.state[self.next_p];
        if v == i32::MAX {
            v -= 1;
        }
        if v < 0 {
            v += i32::MAX;
        }
        self.state[self.next] = v;
        f64::from(v) / f64::from(i32::MAX)
    }
}
pub(super) fn sketch(radius: f32, smoothness: usize) -> Vec<[f32; 4]> {
    let area = (std::f64::consts::PI * f64::from(radius * radius))
        .round_ties_even()
        .max(5.) as usize;
    let count = ((area as f32 * 0.01).round_ties_even() as usize).clamp(5, area);
    (0..smoothness)
        .flat_map(|iteration| {
            let mut random = LegacyRandom::new(iteration as i32);
            (0..count).map(move |_| {
                let r = random.next().sqrt();
                let angle = random.next() * std::f64::consts::TAU;
                [
                    (r * angle.cos() * f64::from(radius)) as f32,
                    (r * angle.sin() * f64::from(radius)) as f32,
                    0.,
                    0.,
                ]
            })
        })
        .collect()
}

/// Direct2D CTurbulence::InitializeGridLevel10's Park-Miller gradients.
pub(super) fn turbulence(seed: u32, period: f32, tile: f32) -> Vec<[f32; 4]> {
    let mut random = if seed == 0 { 1_u64 } else { u64::from(seed) };
    let mut next = || {
        random = random * 16807 % 2147483647;
        random as u32
    };
    let mut permutation: Vec<_> = (0..256).collect();
    let mut gradients = vec![[0.; 4]; 512];
    // Channel-major draw order is part of the native random stream.
    for channel in [0, 1, 2, 3] {
        for index in 0..256 {
            let x = (next() % 512) as f32 / 256. - 1.;
            let y = (next() % 512) as f32 / 256. - 1.;
            let length = (x * x + y * y).sqrt();
            gradients[index * 2][channel] = x / length;
            gradients[index * 2 + 1][channel] = y / length;
        }
    }
    for i in (1..256).rev() {
        permutation.swap(i, next() as usize % 256);
    }
    let f = 1. / period;
    let lo = (f * tile).floor() / tile;
    let hi = (f * tile).ceil() / tile;
    let frequency = if lo > 0. && f / lo < hi / f { lo } else { hi };
    let stitch = (frequency * tile + 0.5) as u32;
    let mut table = vec![
        [frequency, frequency, 0., 0.],
        [
            stitch as f32,
            stitch as f32,
            (4096 + stitch) as f32,
            (4096 + stitch) as f32,
        ],
    ];
    table.extend(permutation.into_iter().map(|v| [v as f32, 0., 0., 0.]));
    table.extend(gradients);
    table
}

#[cfg(test)]
mod tests {
    #[test]
    fn original_bokeh_kernels() {
        #[derive(serde::Deserialize)]
        struct Kernel {
            real: Vec<f32>,
            imaginary: Vec<f32>,
        }
        #[derive(serde::Deserialize)]
        struct Case {
            radius: f64,
            quality: usize,
            kernels: Vec<Kernel>,
        }
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../tests/fixtures/paintnet-5.1.12-bokeh.json"
        ))
        .unwrap();
        for case in cases {
            let actual = super::bokeh(case.radius, case.quality);
            let size = case.kernels[0].real.len();
            assert_eq!(actual.len(), size * case.quality);
            for (component, k) in case.kernels.iter().enumerate() {
                for j in 0..size {
                    let a = actual[component * size + j];
                    assert!(
                        (a[0] - k.real[j]).abs() < 1e-6 && (a[1] - k.imaginary[j]).abs() < 1e-6,
                        "radius {} quality {} sample {}: {:?} vs {:?}",
                        case.radius,
                        case.quality,
                        j,
                        a,
                        [k.real[j], k.imaginary[j]]
                    );
                }
            }
        }
    }
    #[test]
    fn original_sketch_offsets() {
        #[derive(serde::Deserialize)]
        struct Case {
            radius: f32,
            iteration: usize,
            offsets: Vec<[f32; 2]>,
        }
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../tests/fixtures/paintnet-5.1.12-sketch.json"
        ))
        .unwrap();
        for case in cases {
            let all = super::sketch(case.radius, case.iteration + 1);
            let actual = &all[case.iteration * case.offsets.len()..];
            for (a, b) in actual.iter().zip(case.offsets) {
                assert!(
                    (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6,
                    "{a:?} vs {b:?}"
                );
            }
        }
    }
    #[test]
    fn original_native_turbulence_grids() {
        #[derive(serde::Deserialize)]
        struct Case {
            seed: u32,
            permutation: Vec<u32>,
            gradients: Vec<[f32; 4]>,
        }
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../tests/fixtures/native-turbulence.json"))
                .unwrap();
        for case in cases {
            let actual = super::turbulence(case.seed, 100., 4096.);
            for (a, b) in actual[2..258].iter().zip(case.permutation) {
                assert_eq!(a[0] as u32, b);
            }
            for (a, b) in actual[258..].iter().zip(case.gradients) {
                for c in 0..4 {
                    assert!(
                        (a[c] - b[c]).abs() < 1e-7,
                        "seed {} gradient {a:?} vs {b:?}",
                        case.seed
                    );
                }
            }
        }
    }
}
