//! Brush sampling and gradient settings recovered from the frozen reference.
use crate::{
    document::{BlendMode, Document, Pixel, blend},
    selection::{AlphaMode, color_distance},
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum GradientShape {
    #[default]
    Linear,
    Reflected,
    Diamond,
    Radial,
    Conical,
    Spiral,
    SpiralCounterClockwise,
}
impl GradientShape {
    pub const ALL: [Self; 7] = [
        Self::Linear,
        Self::Reflected,
        Self::Diamond,
        Self::Radial,
        Self::Conical,
        Self::Spiral,
        Self::SpiralCounterClockwise,
    ];
    pub fn name(self) -> &'static str {
        [
            "Linear",
            "Linear (reflected)",
            "Diamond",
            "Radial",
            "Conical",
            "Spiral",
            "Spiral (counter-clockwise)",
        ][self as usize]
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum GradientRepeat {
    #[default]
    None,
    Wrapped,
    Reflected,
}
impl GradientRepeat {
    pub const ALL: [Self; 3] = [Self::None, Self::Wrapped, Self::Reflected];
    pub fn name(self) -> &'static str {
        ["No repeat", "Repeat", "Repeat (reflected)"][self as usize]
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct GradientOptions {
    pub shape: GradientShape,
    pub repeat: GradientRepeat,
    pub alpha_only: bool,
    pub overwrite: bool,
    pub blend_mode: BlendMode,
    pub reverse: bool,
}

/// Union coverage avoids applying clone opacity or recolor shifts repeatedly at joins.
fn brush_mask(width: u32, height: u32, points: &[(i32, i32)], size: f32) -> Vec<u8> {
    let mut mask = vec![0; width as usize * height as usize];
    if !size.is_finite() || size <= 0. {
        return mask;
    }
    let radius = (size.min(100.) / 2.).max(0.5);
    for (a, b) in points
        .iter()
        .zip(points.iter().skip(1).chain(points.last()))
    {
        // Native pointer coordinates can be far outside the captured image.
        // Clip the segment before stepping to keep work bounded by the canvas.
        let delta = (b.0 as f64 - a.0 as f64, b.1 as f64 - a.1 as f64);
        let mut low = 0f64;
        let mut high = 1f64;
        for (origin, direction, min, max) in [
            (
                a.0 as f64,
                delta.0,
                -radius as f64,
                width as f64 + radius as f64,
            ),
            (
                a.1 as f64,
                delta.1,
                -radius as f64,
                height as f64 + radius as f64,
            ),
        ] {
            if direction == 0. {
                if origin < min || origin > max {
                    high = -1.;
                }
            } else {
                let t0 = (min - origin) / direction;
                let t1 = (max - origin) / direction;
                low = low.max(t0.min(t1));
                high = high.min(t0.max(t1));
            }
        }
        if low > high {
            continue;
        }
        let steps = ((delta.0.abs().max(delta.1.abs()) * (high - low)).ceil() as usize).max(1);
        for step in 0..=steps {
            let t = low + (high - low) * step as f64 / steps as f64;
            let cx = (a.0 as f64 + delta.0 * t) as f32 + 0.5;
            let cy = (a.1 as f64 + delta.1 * t) as f32 + 0.5;
            for y in ((cy - radius).floor() as i32).max(0)
                ..=((cy + radius).ceil() as i32).min(height as i32 - 1)
            {
                for x in ((cx - radius).floor() as i32).max(0)
                    ..=((cx + radius).ceil() as i32).min(width as i32 - 1)
                {
                    if (x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)
                        <= radius * radius
                    {
                        mask[(y as u32 * width + x as u32) as usize] = 255;
                    }
                }
            }
        }
    }
    mask
}
impl Document {
    pub fn paint_stroke(
        &mut self,
        points: &[(i32, i32)],
        size: f32,
        source: impl Into<crate::pattern::FillSource>,
        erase: bool,
        blender: crate::tool_blend::ToolBlend,
    ) {
        let source = source.into();
        let mask = brush_mask(self.width, self.height, points, size);
        self.paint_mask(mask, source, erase, blender);
    }
    pub fn paint_sampled_stroke(
        &mut self,
        points: &[crate::brush::BrushPoint],
        tip: &crate::brush::BrushTip,
        source: impl Into<crate::pattern::FillSource>,
        erase: bool,
        blender: crate::tool_blend::ToolBlend,
    ) {
        self.paint_mask(
            tip.mask(self.width, self.height, points),
            source.into(),
            erase,
            blender,
        );
    }
    fn paint_mask(
        &mut self,
        mask: Vec<u8>,
        source: crate::pattern::FillSource,
        erase: bool,
        blender: crate::tool_blend::ToolBlend,
    ) {
        let color = source.foreground;
        let selection = self.selection.clone();
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        for (i, coverage) in mask.into_iter().enumerate() {
            let coverage = crate::document::scale(
                u32::from(coverage),
                u32::from(selection.as_ref().map_or(255, |m| m[i])),
            ) as u8;
            if erase {
                pixels[i][3] = crate::document::scale(
                    u32::from(pixels[i][3]),
                    255 - crate::document::scale(u32::from(color[3]), u32::from(coverage)),
                ) as u8;
                if pixels[i][3] == 0 {
                    pixels[i] = crate::document::TRANSPARENT;
                }
            } else {
                pixels[i] = blender.apply(
                    pixels[i],
                    source.sample(
                        (i % self.width as usize) as i32,
                        (i / self.width as usize) as i32,
                    ),
                    coverage,
                );
            }
        }
    }
    pub fn clone_stroke(
        &mut self,
        source: &[Pixel],
        offset: (i32, i32),
        points: &[(i32, i32)],
        size: f32,
        opacity: u8,
    ) {
        self.clone_stroke_blended(
            source,
            offset,
            points,
            size,
            opacity,
            crate::tool_blend::ToolBlend::default(),
        );
    }
    #[allow(clippy::too_many_arguments)]
    pub fn clone_stroke_blended(
        &mut self,
        source: &[Pixel],
        offset: (i32, i32),
        points: &[(i32, i32)],
        size: f32,
        opacity: u8,
        blender: crate::tool_blend::ToolBlend,
    ) {
        if source.len() != self.width as usize * self.height as usize {
            return;
        }
        let mask = brush_mask(self.width, self.height, points, size);
        self.clone_mask(source, offset, mask, opacity, blender, false);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn clone_sampled_stroke(
        &mut self,
        source: &[Pixel],
        offset: (i32, i32),
        points: &[crate::brush::BrushPoint],
        tip: &crate::brush::BrushTip,
        opacity: u8,
        blender: crate::tool_blend::ToolBlend,
    ) {
        if source.len() != self.width as usize * self.height as usize {
            return;
        }
        self.clone_mask(
            source,
            offset,
            tip.mask(self.width, self.height, points),
            opacity,
            blender,
            true,
        );
    }
    fn clone_mask(
        &mut self,
        source: &[Pixel],
        offset: (i32, i32),
        mask: Vec<u8>,
        opacity: u8,
        blender: crate::tool_blend::ToolBlend,
        mask_opacity: bool,
    ) {
        let selection = self.selection.clone();
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        for (i, coverage) in mask.into_iter().enumerate() {
            if coverage == 0 || selection.as_ref().is_some_and(|m| m[i] == 0) {
                continue;
            }
            let x = (i % self.width as usize) as i64 + offset.0 as i64;
            let y = (i / self.width as usize) as i64 + offset.1 as i64;
            if x >= 0 && y >= 0 && x < self.width as i64 && y < self.height as i64 {
                let mut src = source[(y * self.width as i64 + x) as usize];
                let coverage = if mask_opacity {
                    crate::document::scale(u32::from(coverage), u32::from(opacity)) as u8
                } else {
                    src[3] = crate::document::scale(u32::from(src[3]), u32::from(opacity)) as u8;
                    coverage
                };
                let coverage = crate::document::scale(
                    u32::from(coverage),
                    u32::from(selection.as_ref().map_or(255, |m| m[i])),
                ) as u8;
                pixels[i] = blender.apply(pixels[i], src, coverage);
            }
        }
    }
    pub fn recolor_stroke(
        &mut self,
        points: &[(i32, i32)],
        size: f32,
        basis: Pixel,
        fill: Pixel,
        tolerance: u8,
        alpha: AlphaMode,
    ) {
        let mask = brush_mask(self.width, self.height, points, size);
        self.recolor_mask(mask, basis, fill, tolerance, alpha);
    }
    pub fn recolor_sampled_stroke(
        &mut self,
        points: &[crate::brush::BrushPoint],
        tip: &crate::brush::BrushTip,
        basis: Pixel,
        fill: Pixel,
        tolerance: u8,
        alpha: AlphaMode,
    ) {
        self.recolor_mask(
            tip.mask(self.width, self.height, points),
            basis,
            fill,
            tolerance,
            alpha,
        );
    }
    fn recolor_mask(
        &mut self,
        mask: Vec<u8>,
        basis: Pixel,
        fill: Pixel,
        tolerance: u8,
        alpha: AlphaMode,
    ) {
        let selection = self.selection.clone();
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        for (i, covered) in mask.into_iter().enumerate() {
            if covered == 0
                || selection.as_ref().is_some_and(|m| m[i] == 0)
                || color_distance(pixels[i], basis, alpha) > tolerance
            {
                continue;
            }
            let replacement = std::array::from_fn(|c| {
                (pixels[i][c] as i32 + fill[c] as i32 - basis[c] as i32).clamp(0, 255) as u8
            });
            let coverage = crate::document::scale(
                u32::from(covered),
                u32::from(selection.as_ref().map_or(255, |m| m[i])),
            ) as u8;
            pixels[i] = blend(pixels[i], replacement, coverage, BlendMode::Normal);
        }
    }
}
