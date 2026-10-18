//! Raster content for live Text and Shapes transactions.
use crate::document::{BlendMode, Document, Pixel, blend};
use anyhow::{Result, ensure};
use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};
use std::sync::Arc;
use tiny_skia::{FillRule, Paint, Path, PathBuilder, Pixmap, Stroke, StrokeDash, Transform};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TextOptions {
    pub family: String,
    pub size: f32,
    pub size_metric: FontSizeMetric,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub align: TextAlign,
    pub antialias: bool,
    pub overwrite: bool,
    pub blend_mode: BlendMode,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FontSizeMetric {
    #[default]
    Points,
    Fixed96Dpi,
}
impl FontSizeMetric {
    pub fn name(self) -> &'static str {
        match self {
            Self::Points => "Points",
            Self::Fixed96Dpi => "Fixed (96 DPI)",
        }
    }
}
/// TextTool.GetSizedFontProperties and AppSettingsToolsSectionExtensions.
pub fn font_size_dips(
    size: f32,
    metric: FontSizeMetric,
    resolution: crate::resolution::Resolution,
) -> f32 {
    use crate::resolution::MeasurementUnit;
    let dpi = match metric {
        FontSizeMetric::Fixed96Dpi => 96.,
        FontSizeMetric::Points => match resolution.unit {
            MeasurementUnit::Inch => resolution.x,
            MeasurementUnit::Centimeter => resolution.x * 2.54,
            MeasurementUnit::Pixel => resolution.x / 96.,
        },
    };
    (f64::from(size) * dpi / 72.) as f32
}
impl Default for TextOptions {
    fn default() -> Self {
        Self {
            family: "sans-serif".into(),
            size: 12.,
            size_metric: FontSizeMetric::Points,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            align: TextAlign::Left,
            antialias: true,
            overwrite: false,
            blend_mode: BlendMode::Normal,
        }
    }
}
pub struct TextRenderer {
    fonts: FontSystem,
    cache: SwashCache,
    pub families: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct TextLineLayout {
    pub top: f32,
    pub height: f32,
    pub left: f32,
    pub width: f32,
    pub cursors: Vec<(usize, f32)>,
}
#[derive(Clone, Debug)]
pub struct CanvasTextLayout {
    pub lines: Vec<TextLineLayout>,
}
impl CanvasTextLayout {
    pub fn caret(&self, line: usize, offset: usize) -> (f32, f32, f32) {
        let run = &self.lines[line.min(self.lines.len() - 1)];
        let x = run
            .cursors
            .iter()
            .rev()
            .find(|(i, _)| *i <= offset)
            .map_or(run.left, |(_, x)| *x);
        (x, run.top, run.height)
    }
    /// TextTool's first boundary at or beyond X, rather than nearest glyph.
    pub fn hit(&self, x: f32, y: f32) -> (usize, usize) {
        let line = self
            .lines
            .iter()
            .position(|run| y < run.top + run.height)
            .unwrap_or(self.lines.len() - 1);
        let run = &self.lines[line];
        let index = run
            .cursors
            .iter()
            .find(|(_, cx)| *cx >= x)
            .or_else(|| run.cursors.last())
            .map_or(0, |(i, _)| *i);
        (line, index)
    }
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let left = self.lines.iter().map(|r| r.left).fold(0., f32::min);
        let right = self
            .lines
            .iter()
            .map(|r| r.left + r.width)
            .fold(0., f32::max);
        let last = self.lines.last().unwrap();
        (left, 0., right, last.top + last.height)
    }
}
impl Default for TextRenderer {
    fn default() -> Self {
        Self::with_font_system(FontSystem::new())
    }
}
impl TextRenderer {
    /// Keep native fonts, with the already-bundled egui fonts as a last resort.
    /// Minimal installations and Wine may have no usable system font at all.
    pub fn with_font_system(mut fonts: FontSystem) -> Self {
        let empty = fonts.db().faces().next().is_none();
        for font in eframe::egui::FontDefinitions::default().font_data.values() {
            fonts.db_mut().load_font_data(font.font.to_vec());
        }
        if empty {
            fonts.db_mut().set_sans_serif_family("Ubuntu");
            fonts.db_mut().set_serif_family("Ubuntu");
            fonts.db_mut().set_monospace_family("Hack");
        }
        let mut families: Vec<_> = fonts
            .db()
            .faces()
            .flat_map(|face| face.families.iter().map(|f| f.0.clone()))
            .collect();
        families.sort();
        families.dedup();
        families.insert(0, "sans-serif".into());
        Self {
            fonts,
            cache: SwashCache::new(),
            families,
        }
    }
}
impl TextRenderer {
    fn buffer(&mut self, text: &str, options: &TextOptions, size: f32) -> Result<Buffer> {
        ensure!(text.len() <= 65536, "Text is limited to 64 KiB per edit");
        ensure!(
            options.size.is_finite() && (1. ..=2000.).contains(&options.size),
            "Font size must be 1..2000"
        );
        ensure!(
            size.is_finite() && size > 0. && size <= 4096.,
            "Resolved font size exceeds the current 4096-pixel glyph limit; reduce size or use Fixed (96 DPI)"
        );
        let family = if options.family == "sans-serif" || !self.families.contains(&options.family) {
            Family::SansSerif
        } else {
            Family::Name(&options.family)
        };
        let attrs = Attrs::new()
            .family(family)
            .weight(if options.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            })
            .style(if options.italic {
                Style::Italic
            } else {
                Style::Normal
            });
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size * 1.2));
        buffer.set_wrap(Wrap::None);
        buffer.set_size(None, None);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);
        Ok(buffer)
    }
    /// Caret, hit testing, and raster output share this shaped font layout.
    pub fn layout(&mut self, text: &str, options: &TextOptions) -> Result<CanvasTextLayout> {
        self.layout_at_resolution(text, options, crate::resolution::Resolution::default())
    }
    pub fn layout_at_resolution(
        &mut self,
        text: &str,
        options: &TextOptions,
        resolution: crate::resolution::Resolution,
    ) -> Result<CanvasTextLayout> {
        let buffer = self.buffer(
            text,
            options,
            font_size_dips(options.size, options.size_metric, resolution),
        )?;
        let mut lines = Vec::new();
        for run in buffer.layout_runs() {
            let left = match options.align {
                TextAlign::Left => 0.,
                TextAlign::Center => -run.line_w / 2.,
                TextAlign::Right => -run.line_w,
            };
            let cursors = run
                .text
                .char_indices()
                .map(|(i, _)| i)
                .chain([run.text.len()])
                .map(|i| {
                    let cursor = cosmic_text::Cursor::new(run.line_i, i);
                    (i, left + run.cursor_position(&cursor).unwrap_or(0.))
                })
                .collect();
            lines.push(TextLineLayout {
                top: run.line_top,
                height: run.line_height,
                left,
                width: run.line_w,
                cursors,
            });
        }
        // Buffer always provides the empty final line, including an empty edit.
        ensure!(!lines.is_empty(), "Font layout returned no text lines");
        Ok(CanvasTextLayout { lines })
    }
    /// Position is the top of the first line, with per-line alignment around its X anchor.
    pub fn render(
        &mut self,
        doc: &mut Document,
        text: &str,
        position: (i32, i32),
        color: Pixel,
        options: &TextOptions,
    ) -> Result<()> {
        doc.validate()?;
        let size = font_size_dips(options.size, options.size_metric, doc.resolution());
        let buffer = self.buffer(text, options, size)?;
        let offsets: Vec<_> = buffer
            .layout_runs()
            .map(|run| {
                (
                    run.line_top,
                    run.line_top + run.line_height,
                    match options.align {
                        TextAlign::Left => 0.,
                        TextAlign::Center => -run.line_w / 2.,
                        TextAlign::Right => -run.line_w,
                    },
                    run.line_w,
                    run.line_y,
                )
            })
            .collect();
        let width = doc.width as i32;
        let height = doc.height as i32;
        let mut raster = vec![[0; 4]; doc.width as usize * doc.height as usize];
        let mut put = |x: i32, y: i32, c: Pixel| {
            if x < 0 || y < 0 || x >= width || y >= height {
                return;
            }
            let i = (y * width + x) as usize;
            if c[3] == 0 {
                return;
            }
            raster[i] = blend(raster[i], c, 255, BlendMode::Normal);
        };
        for run in buffer.layout_runs() {
            let offset = match options.align {
                TextAlign::Left => 0.,
                TextAlign::Center => -run.line_w / 2.,
                TextAlign::Right => -run.line_w,
            };
            for glyph in run.glyphs {
                let physical = glyph.physical((offset, run.line_y), 1.);
                self.cache.with_pixels(
                    &mut self.fonts,
                    physical.cache_key,
                    Color::rgba(color[0], color[1], color[2], 255),
                    |x, y, c| {
                        let coverage = if options.antialias {
                            c.a()
                        } else if c.a() >= 128 {
                            255
                        } else {
                            0
                        };
                        put(
                            position.0.saturating_add(physical.x).saturating_add(x),
                            position.1.saturating_add(physical.y).saturating_add(y),
                            [c.r(), c.g(), c.b(), coverage],
                        );
                    },
                );
            }
        }
        if options.underline || options.strikeout {
            for (_, _, offset, w, baseline) in offsets {
                for yy in 0..(size / 16.).round().max(1.) as i32 {
                    for xx in 0..(w.ceil() as i32).min(width.max(0)) {
                        for (enabled, y) in [
                            (options.underline, baseline + size * 0.1),
                            (options.strikeout, baseline - size * 0.3),
                        ] {
                            if enabled {
                                put(
                                    position
                                        .0
                                        .saturating_add(offset.round() as i32)
                                        .saturating_add(xx),
                                    position
                                        .1
                                        .saturating_add(y.round() as i32)
                                        .saturating_add(yy),
                                    [color[0], color[1], color[2], 255],
                                );
                            }
                        }
                    }
                }
            }
        }
        let mask = doc.selection.clone();
        let pixels = Arc::make_mut(&mut doc.layers[doc.active].pixels);
        let blender = if options.overwrite {
            crate::tool_blend::ToolBlend::Overwrite
        } else {
            crate::tool_blend::ToolBlend::composite(options.blend_mode)
        };
        for (i, c) in raster.into_iter().enumerate() {
            let coverage = crate::document::scale(
                u32::from(c[3]),
                u32::from(mask.as_ref().map_or(255, |m| m[i])),
            ) as u8;
            let src = [c[0], c[1], c[2], color[3]];
            pixels[i] = blender.apply(pixels[i], src, coverage);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Diamond,
    Trapezoid,
    Parallelogram,
    Triangle,
    RightTriangle,
    Pentagon,
    Hexagon,
    Heptagon,
    Octagon,
    Star3,
    Star4,
    Star5,
    Star6,
    Arrow,
    NotchedArrow,
    PentagonArrow,
    ChevronArrow,
    RectangleCallout,
    RoundedCallout,
    EllipseCallout,
    CloudCallout,
    Lightning,
    Check,
    Multiply,
    Gear,
    Heart,
}
impl ShapeKind {
    /// The reference's constrained aspect ratio is independent of the user's
    /// drawing bounds, including regular polygons and stars.
    pub fn aspect_ratio(self) -> f64 {
        let sides = match self {
            Self::Pentagon => Some(5),
            Self::Hexagon => Some(6),
            Self::Heptagon => Some(7),
            Self::Octagon => Some(8),
            _ => None,
        };
        if let Some(sides) = sides {
            if sides % 2 == 0 {
                return (std::f64::consts::TAU * (sides / 4) as f64 / sides as f64).sin();
            }
            let width = (1..sides / 2)
                .map(|i| (i as f64 * std::f64::consts::TAU / sides as f64).sin())
                .fold(0., f64::max)
                * 2.;
            let height = 1.
                - (std::f64::consts::TAU * ((sides as f64 / 2.).ceil() - 1.) / sides as f64).cos();
            return width / height;
        }
        if self == Self::Triangle {
            return 1. / 0.75f64.sqrt();
        }
        let (sides, ratio) = match self {
            Self::Star3 => (3, 0.2),
            Self::Star4 => (4, 0.25),
            Self::Star5 => (5, 0.381966),
            Self::Star6 => (6, 0.57735),
            _ => return 1.,
        };
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for i in 0..sides * 2 {
            let angle =
                i as f64 * std::f64::consts::PI / sides as f64 - std::f64::consts::FRAC_PI_2;
            let radius = if i % 2 == 0 { 1. } else { ratio };
            for (axis, v) in [radius * angle.cos(), radius * angle.sin()]
                .into_iter()
                .enumerate()
            {
                min[axis] = min[axis].min(v);
                max[axis] = max[axis].max(v);
            }
        }
        (max[0] - min[0]) / (max[1] - min[1])
    }
    pub const ALL: [Self; 29] = [
        Self::Rectangle,
        Self::RoundedRectangle,
        Self::Ellipse,
        Self::Diamond,
        Self::Trapezoid,
        Self::Parallelogram,
        Self::Triangle,
        Self::RightTriangle,
        Self::Pentagon,
        Self::Hexagon,
        Self::Heptagon,
        Self::Octagon,
        Self::Star3,
        Self::Star4,
        Self::Star5,
        Self::Star6,
        Self::Arrow,
        Self::NotchedArrow,
        Self::PentagonArrow,
        Self::ChevronArrow,
        Self::RectangleCallout,
        Self::RoundedCallout,
        Self::EllipseCallout,
        Self::CloudCallout,
        Self::Lightning,
        Self::Check,
        Self::Multiply,
        Self::Gear,
        Self::Heart,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Rectangle => "Rectangle",
            Self::RoundedRectangle => "Rounded Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Diamond => "Diamond",
            Self::Trapezoid => "Trapezoid",
            Self::Parallelogram => "Parallelogram",
            Self::Triangle => "Triangle",
            Self::RightTriangle => "Right Triangle",
            Self::Pentagon => "Pentagon",
            Self::Hexagon => "Hexagon",
            Self::Heptagon => "Heptagon",
            Self::Octagon => "Octagon",
            Self::Star3 => "3-Point Star",
            Self::Star4 => "4-Point Star",
            Self::Star5 => "5-Point Star",
            Self::Star6 => "6-Point Star",
            Self::Arrow => "Arrow",
            Self::NotchedArrow => "Notched Arrow",
            Self::PentagonArrow => "Pentagon Arrow",
            Self::ChevronArrow => "Chevron Arrow",
            Self::RectangleCallout => "Rectangular Callout",
            Self::RoundedCallout => "Rounded Rectangular Callout",
            Self::EllipseCallout => "Ellipse Callout",
            Self::CloudCallout => "Cloud Callout",
            Self::Lightning => "Lightning Bolt",
            Self::Check => "Check Mark",
            Self::Multiply => "Multiply",
            Self::Gear => "Gear",
            Self::Heart => "Heart",
        }
    }
}
pub fn constrain_shape_endpoint(start: (f64, f64), end: (f64, f64), kind: ShapeKind) -> (f64, f64) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let ratio = kind.aspect_ratio();
    let width = dx.abs().min(dy.abs() * ratio);
    (
        start.0 + width * dx.signum(),
        start.1 + width / ratio * dy.signum(),
    )
}
/// Integer pointer coordinates are snapped to opposite rectangle corners, then
/// Shift constrains the aspect ratio and Alt reflects the start around the press.
pub fn shape_drawing_points(
    start: (i32, i32),
    end: (i32, i32),
    kind: ShapeKind,
    constrain: bool,
    centered: bool,
) -> ((f64, f64), (f64, f64)) {
    let mut start = (start.0 as f64, start.1 as f64);
    let mut end = (end.0 as f64, end.1 as f64);
    if start.0 == end.0 {
        end.0 += 1.;
    }
    if start.1 == end.1 {
        end.1 += 1.;
    }
    if constrain {
        end = constrain_shape_endpoint(start, end, kind);
    }
    if centered {
        start = (2. * start.0 - end.0, 2. * start.1 - end.1);
    }
    (start, end)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeMode {
    Outline,
    Fill,
    Both,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeOptions {
    pub kind: ShapeKind,
    pub mode: ShapeMode,
    pub width: f32,
    pub radius: f32,
    pub angle: f32,
    pub antialias: bool,
    pub overwrite: bool,
    pub blend_mode: BlendMode,
    pub fill_style: crate::pattern::FillStyle,
    pub dashed: bool,
}
impl Default for ShapeOptions {
    fn default() -> Self {
        Self {
            kind: ShapeKind::Rectangle,
            mode: ShapeMode::Outline,
            width: 2.,
            radius: 10.,
            angle: 0.,
            antialias: true,
            overwrite: false,
            blend_mode: BlendMode::Normal,
            fill_style: crate::pattern::FillStyle::Solid,
            dashed: false,
        }
    }
}

fn polygon(points: &[(f32, f32)]) -> Path {
    let mut p = PathBuilder::new();
    p.move_to(points[0].0, points[0].1);
    for &(x, y) in &points[1..] {
        p.line_to(x, y);
    }
    p.close();
    p.finish().unwrap()
}
fn regular(n: usize, ratio: Option<f32>, rotation: f32) -> Path {
    let count = if ratio.is_some() { n * 2 } else { n };
    let points: Vec<_> = (0..count)
        .map(|i| {
            let r = if i % 2 == 1 { ratio.unwrap_or(1.) } else { 1. };
            let a = i as f32 * std::f32::consts::TAU / count as f32 - std::f32::consts::FRAC_PI_2
                + rotation.to_radians();
            (r * a.cos(), r * a.sin())
        })
        .collect();
    polygon(&points)
}
// Reference path data has only M/L/C/Z commands. It is compiled to a native path,
// never interpreted as executable XAML or loaded from untrusted files.
fn reference_path(data: &str) -> Path {
    let tokens: Vec<_> = data.split_whitespace().collect();
    let mut i = 0;
    let mut p = PathBuilder::new();
    while i < tokens.len() {
        let command = tokens[i];
        i += 1;
        let mut number = || {
            let v = tokens[i].parse::<f32>().unwrap();
            i += 1;
            v
        };
        match command {
            "M" => p.move_to(number(), number()),
            "L" => p.line_to(number(), number()),
            "C" => p.cubic_to(number(), number(), number(), number(), number(), number()),
            "Z" => p.close(),
            _ => unreachable!(),
        }
    }
    p.finish().unwrap()
}
include!("shape_data.rs");
pub fn shape_path(kind: ShapeKind) -> Path {
    use ShapeKind::*;
    match kind {
        Rectangle | RoundedRectangle => {
            PathBuilder::from_rect(tiny_skia::Rect::from_xywh(0., 0., 1., 1.).unwrap())
        }
        Ellipse => {
            let mut p = PathBuilder::new();
            p.push_oval(tiny_skia::Rect::from_xywh(0., 0., 1., 1.).unwrap());
            p.finish().unwrap()
        }
        Diamond => polygon(&[(0.5, 0.), (1., 0.5), (0.5, 1.), (0., 0.5)]),
        Trapezoid => polygon(&[(0.25, 0.), (0.75, 0.), (1., 1.), (0., 1.)]),
        Parallelogram => polygon(&[(0.25, 0.), (1., 0.), (0.75, 1.), (0., 1.)]),
        Triangle => polygon(&[(0.5, 0.), (1., 1.), (0., 1.)]),
        RightTriangle => polygon(&[(0., 0.), (1., 1.), (0., 1.)]),
        Pentagon => regular(5, None, 0.),
        Hexagon => regular(6, None, 0.),
        Heptagon => regular(7, None, 0.),
        Octagon => regular(8, None, 22.5),
        Star3 => regular(3, Some(0.2), 0.),
        Star4 => regular(4, Some(0.25), 0.),
        Star5 => regular(5, Some(0.381966), 0.),
        Star6 => regular(6, Some(0.57735), 0.),
        Arrow => polygon(&[
            (1., 0.5),
            (0.5, 0.),
            (0.5, 0.25),
            (0., 0.25),
            (0., 0.75),
            (0.5, 0.75),
            (0.5, 1.),
        ]),
        NotchedArrow => polygon(&[
            (1., 0.5),
            (0.5, 0.),
            (0.5, 0.25),
            (0., 0.25),
            (0.25, 0.5),
            (0., 0.75),
            (0.5, 0.75),
            (0.5, 1.),
        ]),
        PentagonArrow => polygon(&[(1., 0.5), (0.5, 0.), (0., 0.), (0., 1.), (0.5, 1.)]),
        ChevronArrow => polygon(&[
            (0., 0.),
            (1. / 3., 0.5),
            (0., 1.),
            (2. / 3., 1.),
            (1., 0.5),
            (2. / 3., 0.),
        ]),
        RectangleCallout => polygon(&[
            (0., 0.),
            (1., 0.),
            (1., 0.75),
            (0.25, 0.75),
            (0., 1.),
            (0.125, 0.75),
            (0., 0.75),
        ]),
        Check => polygon(&[
            (0.85, 0.),
            (1., 0.15),
            (0.375, 1.),
            (0., 0.625),
            (0.15, 0.475),
            (0.352, 0.677),
        ]),
        Multiply => polygon(&[
            (0.15, 0.),
            (0.5, 0.35),
            (0.85, 0.),
            (1., 0.15),
            (0.65, 0.5),
            (1., 0.85),
            (0.85, 1.),
            (0.5, 0.65),
            (0.15, 1.),
            (0., 0.85),
            (0.35, 0.5),
            (0., 0.15),
        ]),
        RoundedCallout => reference_path(ROUNDED_CALLOUT),
        EllipseCallout => reference_path(ELLIPSE_CALLOUT),
        CloudCallout => reference_path(CLOUD_CALLOUT),
        Lightning => reference_path(LIGHTNING),
        Gear => reference_path(GEAR),
        Heart => reference_path(HEART),
    }
}
fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let r = r.min(w / 2.).min(h / 2.).max(0.);
    let k = 0.5522848 * r;
    let mut p = PathBuilder::new();
    p.move_to(x + r, y);
    p.line_to(x + w - r, y);
    p.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    p.line_to(x + w, y + h - r);
    p.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    p.line_to(x + r, y + h);
    p.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    p.line_to(x, y + r);
    p.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    p.close();
    p.finish().unwrap()
}
impl Document {
    pub fn draw_shape(
        &mut self,
        start: (i32, i32),
        end: (i32, i32),
        primary: Pixel,
        secondary: Pixel,
        options: &ShapeOptions,
    ) -> Result<()> {
        self.draw_shape_at(
            (start.0 as f64, start.1 as f64),
            (end.0 as f64, end.1 as f64),
            primary,
            secondary,
            options,
        )
    }
    pub fn draw_shape_at(
        &mut self,
        start: (f64, f64),
        end: (f64, f64),
        primary: Pixel,
        secondary: Pixel,
        options: &ShapeOptions,
    ) -> Result<()> {
        self.validate()?;
        ensure!(
            [start.0, start.1, end.0, end.1]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.),
            "Invalid shape coordinates"
        );
        ensure!(
            [options.width, options.radius, options.angle]
                .iter()
                .all(|v| v.is_finite())
                && (0.1..=100.).contains(&options.width)
                && (0. ..=500.).contains(&options.radius),
            "Invalid shape parameters"
        );
        let x = start.0.min(end.0) as f32;
        let y = start.1.min(end.1) as f32;
        let w = (start.0 - end.0).abs() as f32;
        let h = (start.1 - end.1).abs() as f32;
        if w <= 0. || h <= 0. {
            return Ok(());
        }
        let base = shape_path(options.kind);
        let bounds = base.compute_tight_bounds().unwrap();
        let transform = Transform::from_translate(-bounds.x(), -bounds.y())
            .post_scale(w / bounds.width(), h / bounds.height())
            .post_translate(x, y);
        let path = if options.kind == ShapeKind::RoundedRectangle {
            rounded_rect(x, y, w, h, options.radius)
        } else {
            base.transform(transform).unwrap()
        };
        let rotation = Transform::from_rotate_at(options.angle, x + w / 2., y + h / 2.);
        let path = path.transform(rotation).unwrap();
        let rectangle = matches!(
            options.kind,
            ShapeKind::Rectangle | ShapeKind::RoundedRectangle
        );
        let inset_path = |inset: f32| -> Option<Path> {
            let (ww, hh) = (w - 2. * inset, h - 2. * inset);
            if ww <= 0. || hh <= 0. {
                return None;
            }
            let path = if options.kind == ShapeKind::RoundedRectangle {
                rounded_rect(x + inset, y + inset, ww, hh, options.radius)
            } else {
                PathBuilder::from_rect(tiny_skia::Rect::from_xywh(x + inset, y + inset, ww, hh)?)
            };
            path.transform(rotation)
        };
        let outline = if rectangle {
            inset_path(options.width / 2.)
        } else {
            Some(path.clone())
        };
        let fill = if options.mode == ShapeMode::Both && rectangle {
            inset_path(if options.kind == ShapeKind::Rectangle {
                options.width
            } else {
                options.width / 2.
            })
        } else {
            Some(path.clone())
        };
        let foreground_tile = crate::pattern::FillSource {
            style: options.fill_style,
            foreground: primary,
            background: secondary,
        }
        .tile();
        let background_tile = crate::pattern::FillSource {
            style: options.fill_style,
            foreground: secondary,
            background: primary,
        }
        .tile();
        let render = |pixmap: &mut Pixmap, primary: Pixel, secondary: Pixel, patterned: bool| {
            let mut paint = Paint {
                anti_alias: options.antialias,
                ..Default::default()
            };
            if options.mode != ShapeMode::Outline
                && let Some(fill) = &fill
            {
                let c = if options.mode == ShapeMode::Both {
                    secondary
                } else {
                    primary
                };
                paint.set_color_rgba8(c[0], c[1], c[2], c[3]);
                let tile = if options.mode == ShapeMode::Both {
                    &background_tile
                } else {
                    &foreground_tile
                };
                if patterned && let Some(tile) = tile {
                    paint.shader = tiny_skia::Pattern::new(
                        tile.as_ref(),
                        tiny_skia::SpreadMode::Repeat,
                        tiny_skia::FilterQuality::Nearest,
                        1.,
                        Transform::identity(),
                    );
                }

                pixmap.fill_path(fill, &paint, FillRule::EvenOdd, Transform::identity(), None);
            }
            if options.mode != ShapeMode::Fill {
                paint.set_color_rgba8(primary[0], primary[1], primary[2], primary[3]);
                if patterned && let Some(tile) = &foreground_tile {
                    paint.shader = tiny_skia::Pattern::new(
                        tile.as_ref(),
                        tiny_skia::SpreadMode::Repeat,
                        tiny_skia::FilterQuality::Nearest,
                        1.,
                        Transform::identity(),
                    );
                }

                let mut stroke = Stroke {
                    width: options.width,
                    ..Default::default()
                };
                if options.dashed {
                    stroke.dash = StrokeDash::new(vec![options.width * 3., options.width], 0.);
                }
                if let Some(outline) = &outline {
                    pixmap.stroke_path(outline, &paint, &stroke, Transform::identity(), None);
                } else {
                    pixmap.fill_path(
                        &path,
                        &paint,
                        FillRule::EvenOdd,
                        Transform::identity(),
                        None,
                    );
                }
            }
        };
        let mut pixmap = Pixmap::new(self.width, self.height)
            .ok_or_else(|| anyhow::anyhow!("Shape allocation failed"))?;
        render(&mut pixmap, primary, secondary, true);
        let mut coverage_map = Pixmap::new(self.width, self.height)
            .ok_or_else(|| anyhow::anyhow!("Shape mask allocation failed"))?;
        render(&mut coverage_map, [255; 4], [255; 4], false);
        let mask = self.selection.clone();
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        let blender = if options.overwrite {
            crate::tool_blend::ToolBlend::Overwrite
        } else {
            crate::tool_blend::ToolBlend::composite(options.blend_mode)
        };
        for (i, p) in pixmap.pixels().iter().enumerate() {
            let coverage = coverage_map.pixels()[i].alpha();
            if coverage == 0 {
                continue;
            }
            let a = p.alpha();
            let straight = |c: u8| {
                if a == 0 {
                    0
                } else {
                    ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8
                }
            };
            let src = [
                straight(p.red()),
                straight(p.green()),
                straight(p.blue()),
                if options.overwrite {
                    ((u32::from(a) * 255 + u32::from(coverage) / 2) / u32::from(coverage)).min(255)
                        as u8
                } else {
                    a
                },
            ];
            let coverage = if options.overwrite {
                crate::document::scale(
                    u32::from(coverage),
                    u32::from(mask.as_ref().map_or(255, |m| m[i])),
                ) as u8
            } else {
                mask.as_ref().map_or(255, |m| m[i])
            };
            pixels[i] = blender.apply(pixels[i], src, coverage);
        }
        Ok(())
    }
}
