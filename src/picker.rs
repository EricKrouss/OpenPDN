//! ColorPickerTool's six sample sizes and alpha-weighted float32 averaging.
use crate::document::{Document, Pixel};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleSize {
    #[default]
    Point,
    Three,
    Five,
    Eleven,
    ThirtyOne,
    FiftyOne,
}
impl SampleSize {
    pub const ALL: [Self; 6] = [
        Self::Point,
        Self::Three,
        Self::Five,
        Self::Eleven,
        Self::ThirtyOne,
        Self::FiftyOne,
    ];
    pub fn pixels(self) -> i32 {
        match self {
            Self::Point => 1,
            Self::Three => 3,
            Self::Five => 5,
            Self::Eleven => 11,
            Self::ThirtyOne => 31,
            Self::FiftyOne => 51,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Point => "Single pixel",
            Self::Three => "3 × 3",
            Self::Five => "5 × 5",
            Self::Eleven => "11 × 11",
            Self::ThirtyOne => "31 × 31",
            Self::FiftyOne => "51 × 51",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickerBehavior {
    #[default]
    Stay,
    Previous,
    Pencil,
}
impl PickerBehavior {
    pub const ALL: [Self; 3] = [Self::Stay, Self::Previous, Self::Pencil];
    pub fn name(self) -> &'static str {
        match self {
            Self::Stay => "Do not switch tool",
            Self::Previous => "Switch to previous tool",
            Self::Pencil => "Switch to Pencil",
        }
    }
}
/// Same conversion, accumulation and rounding order as the original managed IL.
pub fn average(colors: impl Iterator<Item = Pixel>) -> Pixel {
    let mut sum = [0f32; 4];
    let mut count = 0usize;
    for color in colors {
        let alpha = f32::from(color[3]) / 255.;
        for c in 0..3 {
            sum[c] += (f32::from(color[c]) / 255.) * alpha;
        }
        sum[3] += alpha;
        count += 1;
    }
    if count == 0 {
        return [0; 4];
    }
    for x in &mut sum {
        *x /= count as f32;
    }
    if sum[3] == 0. {
        return [0; 4];
    }
    let inverse = 1. / sum[3];
    for channel in &mut sum[..3] {
        *channel *= inverse;
    }
    sum.map(|v| (v * 255.).round().clamp(0., 255.) as u8)
}
impl Document {
    pub fn sample_color(
        &self,
        point: (i32, i32),
        size: SampleSize,
        composite: bool,
    ) -> Option<Pixel> {
        if point.0 < 0
            || point.1 < 0
            || point.0 >= self.width as i32
            || point.1 >= self.height as i32
        {
            return None;
        }
        let merged = composite.then(|| self.composite());
        let pixels = merged
            .as_deref()
            .unwrap_or(&self.layers[self.active].pixels);
        if size == SampleSize::Point {
            return Some(pixels[point.1 as usize * self.width as usize + point.0 as usize]);
        }
        let radius = (size.pixels() - 1) / 2;
        let x0 = (point.0 - radius).max(0);
        let x1 = (point.0 + radius).min(self.width as i32 - 1);
        let y0 = (point.1 - radius).max(0);
        let y1 = (point.1 + radius).min(self.height as i32 - 1);
        Some(average((y0..=y1).flat_map(|y| {
            (x0..=x1).map(move |x| pixels[y as usize * self.width as usize + x as usize])
        })))
    }
}
