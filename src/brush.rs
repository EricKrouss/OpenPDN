//! Sampled brush settings and path math recovered from Paint.NET 5.1.12.
//! The portable tip rasterizer uses tiny-skia and a linear-light Gaussian;
//! its pixels are not certified against the native Direct2D sprite pipeline.
use crate::document::scale;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BrushShape {
    #[default]
    Circle,
    Square,
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BrushOptions {
    pub shape: BrushShape,
    pub hardness: f64,
    pub spacing: f64,
    pub antialiased: bool,
    pub smoothing: bool,
    pub pressure: bool,
}
impl Default for BrushOptions {
    fn default() -> Self {
        Self {
            shape: BrushShape::Circle,
            hardness: 0.75,
            spacing: 0.15,
            antialiased: true,
            smoothing: true,
            pressure: true,
        }
    }
}
impl BrushOptions {
    pub fn sanitized(self) -> Self {
        Self {
            hardness: if self.hardness.is_finite() {
                self.hardness.clamp(0., 1.)
            } else {
                0.75
            },
            spacing: if self.spacing.is_finite() {
                self.spacing.clamp(0.01, 5.)
            } else {
                0.15
            },
            ..self
        }
    }
    pub fn spacing_pixels(self, size: f64, scale: f64) -> f64 {
        let diameter = size * scale;
        if self.antialiased {
            (diameter.max(1.) * self.spacing).max(1.)
        } else {
            ((diameter * 2.).round_ties_even() / 2. * self.spacing * 2.)
                .round_ties_even()
                .mul_add(0.5, 0.)
                .max(1.)
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushPoint {
    pub x: f64,
    pub y: f64,
    pub pressure: f64,
}
impl BrushPoint {
    pub fn new(x: f64, y: f64, pressure: f64) -> Self {
        Self { x, y, pressure }
    }
    fn components(self) -> [f64; 3] {
        [self.x, self.y, self.pressure]
    }
    fn from_components(v: [f64; 3]) -> Self {
        Self::new(v[0], v[1], v[2])
    }
    fn lerp(self, other: Self, t: f64) -> Self {
        let a = self.components();
        let b = other.components();
        Self::from_components(std::array::from_fn(|i| a[i] * (1. - t) + b[i] * t))
    }
}
pub fn pressure_scale(pressure: f64) -> f64 {
    (pressure * (1. - 1. / 128.) + 1. / 128.).clamp(0., 1.)
}
pub fn flow_opacity(flow: f64) -> f64 {
    flow * flow * flow
}
pub fn soft_diameter(diameter: f64, hardness: f64) -> f64 {
    let start = diameter / 2. * 1.18875;
    start + (diameter - start) * hardness
}
pub fn aliased_bounds(center: [f64; 2], size: f64, scale: f64) -> [f64; 4] {
    let diameter = (size * scale * 2.).round_ties_even() / 2.;
    let diameter = if diameter < 1.5 { 1. } else { diameter };
    [
        (center[0] - diameter / 2.).round(),
        (center[1] - diameter / 2.).round(),
        diameter,
        diameter,
    ]
}

/// Centripetal Catmull-Rom uses pressure as its third coordinate, as does the reference.
pub fn smooth_point(points: [BrushPoint; 4], t: f32) -> BrushPoint {
    if t == 0. {
        return points[1];
    }
    let [a, b, c, d] = points.map(BrushPoint::components);
    let distance = |x: [f64; 3], y: [f64; 3]| {
        (0..3)
            .map(|i| (x[i] - y[i]).powi(2))
            .sum::<f64>()
            .sqrt()
            .sqrt()
    };
    let n = distance(a, b);
    let m = n + distance(b, c);
    let k = m + distance(c, d);
    // Dedupe normally prevents zero intervals. Repeated control points must not
    // introduce NaNs through public, imported or interrupted input paths.
    if n == 0. || m == n || k == m {
        return points[1].lerp(points[2], f64::from(t));
    }
    let t = f64::from(t);
    BrushPoint::from_components(std::array::from_fn(|i| {
        let u = (m - n) * ((a[i] - b[i]) / -n - (a[i] - c[i]) / -m + (b[i] - c[i]) / (n - m));
        let v =
            (m - n) * ((b[i] - c[i]) / (n - m) - (b[i] - d[i]) / (n - k) + (c[i] - d[i]) / (m - k));
        let p = 2. * b[i] - 2. * c[i] + u + v;
        let q = -3. * b[i] + 3. * c[i] - 2. * u - v;
        p * (t * t * t) + q * (t * t) + u * t + b[i]
    }))
}

/// Keep fractional sample positions across input-event boundaries. Clipping is
/// performed after tracking arc length, so entering the canvas preserves phase.
pub fn samples(
    points: &[BrushPoint],
    size: f64,
    options: BrushOptions,
    bounds: [f64; 4],
) -> Vec<BrushPoint> {
    let options = options.sanitized();
    let mut input: Vec<_> = points
        .iter()
        .copied()
        .filter(|p| p.x.is_finite() && p.y.is_finite() && p.pressure.is_finite())
        .map(|p| BrushPoint {
            pressure: if options.pressure { p.pressure } else { 1. },
            ..p
        })
        .collect();
    input.dedup();
    if input.is_empty() || !size.is_finite() || size <= 0. {
        return Vec::new();
    }
    if options.smoothing && input.len() > 2 {
        let mut smoothed = vec![input[0]];
        let extrapolate = |a: BrushPoint, b: BrushPoint| {
            BrushPoint::from_components(std::array::from_fn(|i| {
                2. * a.components()[i] - b.components()[i]
            }))
        };
        for i in 0..input.len() - 1 {
            let controls = [
                if i == 0 {
                    extrapolate(input[0], input[1])
                } else {
                    input[i - 1]
                },
                input[i],
                input[i + 1],
                input
                    .get(i + 2)
                    .copied()
                    .unwrap_or_else(|| extrapolate(input[i + 1], input[i])),
            ];
            let steps = ((input[i + 1].x - input[i].x)
                .hypot(input[i + 1].y - input[i].y)
                .ceil() as usize)
                .clamp(1, 65_536);
            for j in 1..=steps {
                smoothed.push(smooth_point(controls, (j as f64 / steps as f64) as f32));
            }
        }
        input = smoothed;
    }
    let inside = |p: BrushPoint| {
        p.x >= bounds[0] && p.y >= bounds[1] && p.x <= bounds[2] && p.y <= bounds[3]
    };
    let mut output = Vec::new();
    if inside(input[0]) {
        output.push(input[0]);
    }
    let mut last_length = 0.;
    let mut spacing = options.spacing_pixels(size, pressure_scale(input[0].pressure));
    let mut length = 0.;
    for pair in input.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let segment = if options.antialiased {
            dx.hypot(dy)
        } else {
            dx.abs().max(dy.abs())
        };
        if segment == 0. {
            continue;
        }
        let end = length + segment;
        // Skip fully off-canvas spans in constant time when pressure is constant.
        // Variable-pressure input is bounded to avoid hostile imported coordinates.
        let mut low = 0f64;
        let mut high = 1f64;
        for (origin, delta, min, max) in [
            (a.x, dx, bounds[0], bounds[2]),
            (a.y, dy, bounds[1], bounds[3]),
        ] {
            if delta == 0. {
                if origin < min || origin > max {
                    high = -1.;
                }
            } else {
                let t0 = (min - origin) / delta;
                let t1 = (max - origin) / delta;
                low = low.max(t0.min(t1));
                high = high.min(t0.max(t1));
            }
        }
        if a.pressure == b.pressure {
            let limit = if low <= high {
                length + segment * low
            } else {
                end
            };
            let skip = ((limit - last_length) / spacing).ceil().max(1.) - 1.;
            last_length += skip * spacing;
        }
        let mut iterations = 0usize;
        while last_length + spacing <= end {
            last_length += spacing;
            let t = (last_length - length) / segment;
            let p = a.lerp(b, t);
            spacing = options.spacing_pixels(size, pressure_scale(p.pressure));
            if low <= high && t >= low && t <= high {
                // SampleInfo stores its interpolation parameter as float32.
                output.push(a.lerp(b, f64::from(t as f32)));
            }
            iterations += 1;
            if iterations >= 1_000_000 {
                break;
            }
        }
        length = end;
    }
    output
}

/// Prepared once per stroke. Source color opacity is applied after the mask
/// has accumulated, preventing repeated color blending at overlapping stamps.
pub struct BrushTip {
    pub size: f64,
    pub options: BrushOptions,
    edge: usize,
    unit: f64,
    pixels: Vec<f32>,
}
impl BrushTip {
    pub fn new(size: f64, options: BrushOptions) -> Self {
        let size = if size.is_finite() {
            size.clamp(1., 2000.)
        } else {
            2.
        };
        let options = options.sanitized();
        if !options.antialiased {
            return Self {
                size,
                options,
                edge: 0,
                unit: 1.,
                pixels: Vec::new(),
            };
        }
        let diameter = size.max(32.);
        let sigma = diameter * (1. - options.hardness) / 6.;
        // Recovered Direct2D Quality pre-scale bounds the working convolution.
        let pre = (12. / (sigma + 6.)).clamp(0.1, 1.);
        let unit = 1. / pre;
        let soft = soft_diameter(diameter, options.hardness);
        let edge = ((soft + 6. * sigma) * pre).ceil() as usize + 5;
        let edge = edge | 1;
        let mut pixmap =
            tiny_skia::Pixmap::new(edge as u32, edge as u32).expect("bounded brush sprite");
        let center = edge as f32 / 2.;
        let radius = (soft * pre / 2.) as f32;
        let mut path = tiny_skia::PathBuilder::new();
        match options.shape {
            BrushShape::Circle => path.push_circle(center, center, radius),
            BrushShape::Square => path.push_rect(
                tiny_skia::Rect::from_xywh(
                    center - radius,
                    center - radius,
                    2. * radius,
                    2. * radius,
                )
                .unwrap(),
            ),
        }
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path.finish().unwrap(),
            &paint,
            tiny_skia::FillRule::Winding,
            tiny_skia::Transform::identity(),
            None,
        );
        let mut pixels: Vec<f32> = pixmap
            .pixels()
            .iter()
            .map(|p| {
                let c = 1. - f32::from(p.alpha()) / 255.;
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            })
            .collect();
        if sigma > 0. {
            let s = (sigma * pre) as f32;
            let radius = (3. * s).ceil().max(1.) as i32;
            let mut kernel: Vec<f32> = (-radius..=radius)
                .map(|i| (-(i * i) as f32 / (2. * s * s)).exp())
                .collect();
            let sum: f32 = kernel.iter().sum();
            for w in &mut kernel {
                *w /= sum;
            }
            let mut tmp = vec![0.; pixels.len()];
            for vertical in [false, true] {
                for y in 0..edge {
                    for x in 0..edge {
                        let mut value = 0.;
                        for (k, w) in (-radius..=radius).zip(&kernel) {
                            let (sx, sy) = if vertical {
                                (x as i32, y as i32 + k)
                            } else {
                                (x as i32 + k, y as i32)
                            };
                            let c = if sx < 0 || sy < 0 || sx >= edge as i32 || sy >= edge as i32 {
                                1.
                            } else {
                                pixels[sy as usize * edge + sx as usize]
                            };
                            value += c * w;
                        }
                        tmp[y * edge + x] = value;
                    }
                }
                std::mem::swap(&mut pixels, &mut tmp);
            }
        }
        for p in &mut pixels {
            let c = if *p <= 0.0031308 {
                *p * 12.92
            } else {
                1.055 * p.powf(1. / 2.4) - 0.055
            };
            *p = (1. - c).clamp(0., 1.);
        }
        Self {
            size,
            options,
            edge,
            unit,
            pixels,
        }
    }
    pub fn mask(&self, width: u32, height: u32, points: &[BrushPoint]) -> Vec<u8> {
        let mut mask = vec![0; width as usize * height as usize];
        if width == 0 || height == 0 {
            return mask;
        }
        let extent = if self.options.antialiased {
            self.edge as f64 * self.unit * self.size / self.size.max(32.) / 2. + 1.
        } else {
            self.size / 2. + 1.
        };
        let stamps = samples(
            points,
            self.size,
            self.options,
            [
                -extent,
                -extent,
                width as f64 + extent,
                height as f64 + extent,
            ],
        );
        for p in stamps {
            let diameter = self.size * pressure_scale(p.pressure);
            let render = if self.options.antialiased {
                diameter.max(1.)
            } else {
                (diameter * 2.).round_ties_even() / 2.
            };
            let opacity = if self.options.antialiased {
                diameter * diameter / (render * render)
            } else {
                1.
            };
            let half = if self.options.antialiased {
                self.edge as f64 * self.unit * render / self.size.max(32.) / 2.
            } else {
                render.max(1.) / 2.
            };
            let alias = aliased_bounds([p.x, p.y], self.size, pressure_scale(p.pressure));
            let left = if self.options.antialiased {
                p.x - half
            } else {
                alias[0]
            };
            let top = if self.options.antialiased {
                p.y - half
            } else {
                alias[1]
            };
            for y in (top.floor().max(0.) as u32)
                ..=((top + 2. * half).ceil().max(0.) as u32).min(height.saturating_sub(1))
            {
                for x in (left.floor().max(0.) as u32)
                    ..=((left + 2. * half).ceil().max(0.) as u32).min(width.saturating_sub(1))
                {
                    let (dx, dy) = (x as f64 + 0.5 - left - half, y as f64 + 0.5 - top - half);
                    let coverage = if self.options.antialiased {
                        let factor = self.size.max(32.) / (render * self.unit);
                        self.sample(
                            dx * factor + self.edge as f64 / 2. - 0.5,
                            dy * factor + self.edge as f64 / 2. - 0.5,
                        ) * opacity
                    } else if render < 1.5 {
                        f64::from(dx.abs() < 0.5 && dy.abs() < 0.5)
                    } else {
                        match self.options.shape {
                            BrushShape::Circle => f64::from(dx * dx + dy * dy <= half * half),
                            BrushShape::Square => f64::from(dx.abs() < half && dy.abs() < half),
                        }
                    };
                    let value = (coverage * 255.).round().clamp(0., 255.) as u32;
                    let i = (y * width + x) as usize;
                    mask[i] = (value + scale(u32::from(mask[i]), 255 - value)).min(255) as u8;
                }
            }
        }
        mask
    }
    fn sample(&self, x: f64, y: f64) -> f64 {
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let at = |x: i32, y: i32| {
            if x < 0 || y < 0 || x >= self.edge as i32 || y >= self.edge as i32 {
                0.
            } else {
                f64::from(self.pixels[y as usize * self.edge + x as usize])
            }
        };
        let fx = x - f64::from(ix);
        let fy = y - f64::from(iy);
        (at(ix, iy) * (1. - fx) + at(ix + 1, iy) * fx) * (1. - fy)
            + (at(ix, iy + 1) * (1. - fx) + at(ix + 1, iy + 1) * fx) * fy
    }
}
