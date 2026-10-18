//! Line/Curve geometry follows the recovered four control points and natural
//! cubic spline. Raster coverage is provided by the portable tiny-skia renderer.
use crate::document::{BlendMode, Document, scale};
use anyhow::{Result, ensure};
use std::sync::Arc;
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveMode {
    Spline,
    Bezier,
    Straight,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineCapStyle {
    Flat,
    Rounded,
    Arrow,
    ArrowFilled,
}
impl LineCapStyle {
    pub const ALL: [Self; 4] = [Self::Flat, Self::Arrow, Self::ArrowFilled, Self::Rounded];
    pub fn name(self) -> &'static str {
        match self {
            Self::Flat => "Flat",
            Self::Rounded => "Rounded",
            Self::Arrow => "Arrow",
            Self::ArrowFilled => "Filled Arrow",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineDash {
    Solid,
    Dash,
    Dot,
    DashDot,
    DashDotDot,
}
impl LineDash {
    pub const ALL: [Self; 5] = [
        Self::Solid,
        Self::Dash,
        Self::Dot,
        Self::DashDot,
        Self::DashDotDot,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::Dash => "Dash",
            Self::Dot => "Dot",
            Self::DashDot => "Dash Dot",
            Self::DashDotDot => "Dash Dot Dot",
        }
    }
    fn pattern(self, width: f32) -> Option<StrokeDash> {
        let (pattern, offset) = match self {
            Self::Solid => return None,
            Self::Dash => (vec![2., 2.], 1.),
            Self::Dot => (vec![0., 2.], 0.),
            Self::DashDot => (vec![2., 2., 0., 2.], 1.),
            Self::DashDotDot => (vec![2., 2., 0., 2., 0., 2.], 1.),
        };
        StrokeDash::new(
            pattern.into_iter().map(|v| v * width).collect(),
            offset * width,
        )
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct LineOptions {
    pub mode: CurveMode,
    pub width: f32,
    pub start_cap: LineCapStyle,
    pub end_cap: LineCapStyle,
    pub dash: LineDash,
    pub antialias: bool,
    pub overwrite: bool,
    pub blend_mode: BlendMode,
    pub fill_style: crate::pattern::FillStyle,
}
impl Default for LineOptions {
    fn default() -> Self {
        Self {
            mode: CurveMode::Spline,
            width: 2.,
            start_cap: LineCapStyle::Flat,
            end_cap: LineCapStyle::Flat,
            dash: LineDash::Solid,
            antialias: true,
            overwrite: false,
            blend_mode: BlendMode::Normal,
            fill_style: crate::pattern::FillStyle::Solid,
        }
    }
}
pub fn initial_points(start: (i32, i32), end: (i32, i32)) -> [[f32; 2]; 4] {
    initial_points_with_modifiers(start, end, false, false)
}
/// Original ShapesToolChanges 15-degree constraint, including pixel-aligned
/// lengths for horizontal/vertical and diagonal lines. Ties round away from zero.
pub fn constrain_endpoint(start: [f64; 2], end: [f64; 2]) -> [f64; 2] {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let round = f64::round;
    // OS atan2 implementations disagree by one ULP at a 7.5-degree boundary,
    // which can change the entire snapped line. Use one portable implementation.
    let step = round(12. * libm::atan2(delta[1], delta[0]) / std::f64::consts::PI);
    let angle = step * std::f64::consts::PI / 12.;
    let mut length = (delta[0] * delta[0] + delta[1] * delta[1]).sqrt();
    if step % 6. == 0. {
        length = round(length);
    } else if step % 3. == 0. {
        length = round(length / std::f64::consts::SQRT_2) * std::f64::consts::SQRT_2;
    }
    [
        start[0] + length * angle.cos(),
        start[1] + length * angle.sin(),
    ]
}
pub fn initial_points_with_modifiers(
    start: (i32, i32),
    end: (i32, i32),
    constrain: bool,
    centered: bool,
) -> [[f32; 2]; 4] {
    let mut start = [start.0 as f64 + 0.5, start.1 as f64 + 0.5];
    let mut end = [end.0 as f64 + 0.5, end.1 as f64 + 0.5];
    if constrain {
        end = constrain_endpoint(start, end);
    }
    if centered {
        start = [2. * start[0] - end[0], 2. * start[1] - end[1]];
    }
    std::array::from_fn(|i| {
        let t = i as f64 / 3.;
        [
            (start[0] * (1. - t) + end[0] * t) as f32,
            (start[1] * (1. - t) + end[1] * t) as f32,
        ]
    })
}
/// Three cubic segments of the original four-knot natural spline, in f64.
pub fn spline_segments(knots: [[f64; 2]; 4]) -> [[[f64; 2]; 3]; 3] {
    let mut first = [[0.; 2]; 3];
    for axis in 0..2 {
        let rhs = [
            knots[0][axis] + 2. * knots[1][axis],
            4. * knots[1][axis] + 2. * knots[2][axis],
            (8. * knots[2][axis] + knots[3][axis]) / 2.,
        ];
        let mut buffer = [0.; 3];
        let mut result = [0.; 3];
        let mut divisor = 2.;
        result[0] = rhs[0] / divisor;
        for i in 1..3 {
            buffer[i] = 1. / divisor;
            divisor = if i < 2 { 4. } else { 3.5 } - buffer[i];
            result[i] = (rhs[i] - result[i - 1]) / divisor;
        }
        for i in (0..2).rev() {
            result[i] -= buffer[i + 1] * result[i + 1];
        }
        for i in 0..3 {
            first[i][axis] = result[i];
        }
    }
    std::array::from_fn(|i| {
        [
            first[i],
            std::array::from_fn(|axis| {
                if i < 2 {
                    2. * knots[i + 1][axis] - first[i + 1][axis]
                } else {
                    (knots[3][axis] + first[2][axis]) / 2.
                }
            }),
            knots[i + 1],
        ]
    })
}
fn mid(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}
fn flatten(points: [[f32; 2]; 4], depth: u32, out: &mut Vec<[f32; 2]>) {
    let [a, b, c, d] = points;
    let dx = d[0] - a[0];
    let dy = d[1] - a[1];
    let cross = |p: [f32; 2]| ((p[0] - a[0]) * dy - (p[1] - a[1]) * dx).abs();
    let flat = if dx * dx + dy * dy < 0.0001 {
        [b, c]
            .iter()
            .all(|p| (p[0] - a[0]).hypot(p[1] - a[1]) <= 0.1)
    } else {
        cross(b).max(cross(c)).powi(2) <= 0.01 * (dx * dx + dy * dy)
    };
    // Also subdivide collinear backtracking control polygons.
    let polygon = (b[0] - a[0]).hypot(b[1] - a[1])
        + (c[0] - b[0]).hypot(c[1] - b[1])
        + (d[0] - c[0]).hypot(d[1] - c[1]);
    if depth == 16 || (flat && polygon - (d[0] - a[0]).hypot(d[1] - a[1]) <= 0.1) {
        out.push(d);
        return;
    }
    let ab = mid(a, b);
    let bc = mid(b, c);
    let cd = mid(c, d);
    let abc = mid(ab, bc);
    let bcd = mid(bc, cd);
    let center = mid(abc, bcd);
    flatten([a, ab, abc, center], depth + 1, out);
    flatten([center, bcd, cd, d], depth + 1, out);
}
fn unit(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let v = [b[0] - a[0], b[1] - a[1]];
    let length = v[0].hypot(v[1]);
    if length == 0. {
        [0.; 2]
    } else {
        [v[0] / length, v[1] / length]
    }
}
fn trimmed(points: &[[f32; 2]], start: f32, end: f32) -> Vec<[f32; 2]> {
    let mut out = Vec::new();
    let mut distance = 0.;
    for pair in points.windows(2) {
        let length = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
        if length > 0. && distance + length >= start && distance <= end {
            let lo = ((start - distance) / length).clamp(0., 1.);
            let hi = ((end - distance) / length).clamp(0., 1.);
            if hi >= lo {
                for t in [lo, hi] {
                    let p = [
                        pair[0][0] + (pair[1][0] - pair[0][0]) * t,
                        pair[0][1] + (pair[1][1] - pair[0][1]) * t,
                    ];
                    if out.last() != Some(&p) {
                        out.push(p);
                    }
                }
            }
        }
        distance += length;
    }
    out
}
impl Document {
    pub fn draw_curve(
        &mut self,
        points: [[f32; 2]; 4],
        source: impl Into<crate::pattern::FillSource>,
        options: &LineOptions,
    ) -> Result<()> {
        let mut source = source.into();
        source.style = options.fill_style;
        self.validate()?;
        ensure!(
            points
                .iter()
                .flatten()
                .all(|p| p.is_finite() && p.abs() <= 100000.)
                && options.width.is_finite()
                && (0.1..=100.).contains(&options.width),
            "Invalid Line/Curve geometry"
        );
        let mut line = vec![points[0]];
        match options.mode {
            CurveMode::Straight => line.extend_from_slice(&points[1..]),
            CurveMode::Bezier => flatten(points, 0, &mut line),
            CurveMode::Spline => {
                let segments = spline_segments(points.map(|p| p.map(f64::from)));
                let mut start = points[0];
                for segment in segments {
                    let [a, b, end] = segment.map(|p| p.map(|v| v as f32));
                    flatten([start, a, b, end], 0, &mut line);
                    start = end;
                }
            }
        }
        line.dedup();
        if line.len() < 2 {
            return Ok(());
        }
        let first_tangent = unit(line[0], line[1]);
        let last = line.len() - 1;
        let last_tangent = unit(line[last - 1], line[last]);
        for axis in 0..2 {
            line[0][axis] -= first_tangent[axis] * 0.4999;
            line[last][axis] += last_tangent[axis] * 0.4999;
        }
        let length: f32 = line
            .windows(2)
            .map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]))
            .sum();
        let cap_trim = |cap| match cap {
            LineCapStyle::Arrow => 0.5 * options.width,
            LineCapStyle::ArrowFilled => 1.5 * options.width,
            _ => 0.,
        };
        let stroke_points = trimmed(
            &line,
            cap_trim(options.start_cap),
            length - cap_trim(options.end_cap),
        );
        let mut pixmap = Pixmap::new(self.width, self.height)
            .ok_or_else(|| anyhow::anyhow!("Line allocation failed"))?;
        let mut paint = Paint {
            anti_alias: options.antialias,
            ..Default::default()
        };
        paint.set_color_rgba8(255, 255, 255, 255);
        if stroke_points.len() >= 2 {
            let mut path = PathBuilder::new();
            path.move_to(stroke_points[0][0], stroke_points[0][1]);
            for p in &stroke_points[1..] {
                path.line_to(p[0], p[1]);
            }
            if let Some(path) = path.finish() {
                pixmap.stroke_path(
                    &path,
                    &paint,
                    &Stroke {
                        width: options.width,
                        line_join: LineJoin::Round,
                        line_cap: if options.dash == LineDash::Solid {
                            LineCap::Butt
                        } else {
                            LineCap::Square
                        },
                        dash: options.dash.pattern(options.width),
                        ..Default::default()
                    },
                    Transform::identity(),
                    None,
                );
            }
        }
        for (cap, p, tangent, reverse) in [
            (options.start_cap, line[0], first_tangent, -1.),
            (options.end_cap, line[last], last_tangent, 1.),
        ] {
            if cap == LineCapStyle::Rounded {
                let path = PathBuilder::from_circle(p[0], p[1], options.width * 0.5);
                if let Some(path) = path {
                    pixmap.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            } else if matches!(cap, LineCapStyle::Arrow | LineCapStyle::ArrowFilled) {
                let height = options.width * if cap == LineCapStyle::Arrow { 2.5 } else { 5. };
                let half = options.width * 2.5;
                let at = |x: f32, y: f32| {
                    [
                        p[0] + reverse * (x * tangent[0] - y * tangent[1]),
                        p[1] + reverse * (x * tangent[1] + y * tangent[0]),
                    ]
                };
                let a = at(-height, half);
                let b = at(-height, -half);
                let mut arrow = PathBuilder::new();
                arrow.move_to(a[0], a[1]);
                arrow.line_to(p[0], p[1]);
                arrow.line_to(b[0], b[1]);
                if cap == LineCapStyle::ArrowFilled {
                    arrow.close();
                }
                if let Some(path) = arrow.finish() {
                    if cap == LineCapStyle::ArrowFilled {
                        pixmap.fill_path(
                            &path,
                            &paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    } else {
                        pixmap.stroke_path(
                            &path,
                            &paint,
                            &Stroke {
                                width: options.width,
                                ..Default::default()
                            },
                            Transform::identity(),
                            None,
                        );
                    }
                }
            }
        }
        let mask = self.selection.clone();
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        let blender = if options.overwrite {
            crate::tool_blend::ToolBlend::Overwrite
        } else {
            crate::tool_blend::ToolBlend::composite(options.blend_mode)
        };
        for (i, p) in pixmap.pixels().iter().enumerate() {
            let coverage = scale(
                u32::from(p.alpha()),
                u32::from(mask.as_ref().map_or(255, |m| m[i])),
            ) as u8;
            pixels[i] = blender.apply(
                pixels[i],
                source.sample(
                    (i % self.width as usize) as i32,
                    (i / self.width as usize) as i32,
                ),
                coverage,
            );
        }
        Ok(())
    }
}
