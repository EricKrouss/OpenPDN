//! Reference LevelsParameters and natural cubic spline transfer tables.
use crate::document::Document;
use anyhow::{Result, ensure};

#[derive(Clone, Debug)]
pub struct Levels {
    pub input_black: [f32; 3],
    pub input_white: [f32; 3],
    pub gamma: [f32; 3],
    pub output_black: [f32; 3],
    pub output_white: [f32; 3],
}
impl Default for Levels {
    fn default() -> Self {
        Self {
            input_black: [0.; 3],
            input_white: [1.; 3],
            gamma: [1.; 3],
            output_black: [0.; 3],
            output_white: [1.; 3],
        }
    }
}
impl Levels {
    pub fn table(&self) -> Result<Vec<[f32; 4]>> {
        for c in 0..3 {
            ensure!(
                [
                    self.input_black[c],
                    self.input_white[c],
                    self.output_black[c],
                    self.output_white[c]
                ]
                .iter()
                .all(|v| v.is_finite() && (0. ..=1.).contains(v))
                    && self.gamma[c].is_finite()
                    && (0.1..=10.).contains(&self.gamma[c]),
                "Invalid levels parameter"
            );
        }
        Ok((0..256)
            .map(|i| {
                let x = i as f32 / 255.;
                let rgb = std::array::from_fn::<_, 3, _>(|c| {
                    let v = x - self.input_black[c];
                    if v < 0. {
                        self.output_black[c]
                    } else if v + self.input_black[c] >= self.input_white[c] {
                        self.output_white[c]
                    } else {
                        (self.output_black[c] as f64
                            + (self.output_white[c] as f64 - self.output_black[c] as f64)
                                * (v as f64 / (self.input_white[c] - self.input_black[c]) as f64)
                                    .powf(self.gamma[c] as f64))
                        .clamp(0., 1.) as f32
                    }
                });
                [rgb[0], rgb[1], rgb[2], x]
            })
            .collect())
    }
    pub fn auto(doc: &Document) -> Self {
        let histogram = histogram(doc);
        let mut result = Self::default();
        for (c, bins) in histogram.iter().enumerate() {
            let total: u64 = bins.iter().sum();
            if total == 0 {
                continue;
            }
            let percentile = |fraction: f64| {
                let threshold = total as f64 * fraction;
                let mut sum = 0u64;
                for (i, v) in bins.iter().enumerate() {
                    sum += v;
                    if sum as f64 >= threshold {
                        return i as f32 / 255.;
                    }
                }
                1.
            };
            let black = percentile(0.005f32 as f64);
            let white = percentile(0.995f32 as f64);
            let mean = (bins
                .iter()
                .enumerate()
                .map(|(i, &v)| i as f64 * v as f64)
                .sum::<f64>()
                / total as f64
                / 255.) as f32;
            result.input_black[c] = black;
            result.input_white[c] = white;
            if black < mean && mean < white {
                result.gamma[c] = (0.5f64.ln()
                    / (((mean - black) as f64) / (white - black) as f64).ln())
                .clamp(0.1, 10.) as f32;
            }
        }
        result
    }
}
pub fn histogram(doc: &Document) -> [[u64; 256]; 3] {
    let mut bins = [[0; 256]; 3];
    for (i, p) in doc.layers[doc.active].pixels.iter().enumerate() {
        if !doc.selected(i) {
            continue;
        }
        for c in 0..3 {
            bins[c][p[c] as usize] += 1;
        }
    }
    bins
}
#[derive(Clone, Debug)]
pub struct Curves {
    pub points: [Vec<(u8, u8)>; 3],
    pub luminosity: bool,
}
impl Default for Curves {
    fn default() -> Self {
        Self {
            points: std::array::from_fn(|_| vec![(0, 0), (255, 255)]),
            luminosity: true,
        }
    }
}
impl Curves {
    pub fn table(&self) -> Result<Vec<[f32; 4]>> {
        let samples: Result<Vec<_>> = self.points.iter().map(|p| spline(p, 1024)).collect();
        let samples = samples?;
        Ok((0..1024)
            .map(|i| {
                [
                    samples[0][i],
                    samples[1][i],
                    samples[2][i],
                    i as f32 / 1023.,
                ]
            })
            .collect())
    }
}
pub fn spline(points: &[(u8, u8)], count: usize) -> Result<Vec<f32>> {
    ensure!(
        points.len() >= 2 && points.len() <= 256 && count >= 2,
        "Curve needs at least two points"
    );
    let mut points = points.to_vec();
    points.sort_by_key(|p| p.0);
    ensure!(
        points.windows(2).all(|p| p[0].0 != p[1].0),
        "Curve X positions must be unique"
    );
    let n = points.len();
    let x: Vec<_> = points.iter().map(|p| p.0 as f64 / 255.).collect();
    let y: Vec<_> = points.iter().map(|p| p.1 as f64 / 255.).collect();
    let mut second = vec![0.; n];
    let mut work = vec![0.; n];
    for i in 1..n - 1 {
        let span = x[i + 1] - x[i - 1];
        let ratio = (x[i] - x[i - 1]) / span;
        let divisor = ratio * second[i - 1] + 2.;
        second[i] = (ratio - 1.) / divisor;
        let delta = (y[i + 1] - y[i]) / (x[i + 1] - x[i]) - (y[i] - y[i - 1]) / (x[i] - x[i - 1]);
        work[i] = (6. * delta / span - ratio * work[i - 1]) / divisor;
    }
    for i in (0..n - 1).rev() {
        second[i] = second[i] * second[i + 1] + work[i];
    }
    Ok((0..count)
        .map(|i| {
            let v = i as f64 / (count - 1) as f64;
            let hi = x.partition_point(|p| *p <= v).clamp(1, n - 1);
            let lo = hi - 1;
            let h = x[hi] - x[lo];
            let a = (x[hi] - v) / h;
            let b = (v - x[lo]) / h;
            (a * y[lo]
                + b * y[hi]
                + ((a * a * a - a) * second[lo] + (b * b * b - b) * second[hi]) * h * h / 6.)
                as f32
        })
        .collect())
}
