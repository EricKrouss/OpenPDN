//! Selection construction and Paint.NET's reference-derived flood color distance.
use crate::document::{BlendMode, Document, Pixel, TRANSPARENT, blend};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SelectionMode {
    #[default]
    Replace,
    Add,
    Subtract,
    Intersect,
    Xor,
}
impl SelectionMode {
    pub fn from_pointer(self, secondary: bool, control: bool, alt: bool) -> Self {
        match (control, alt, secondary) {
            (true, _, false) => Self::Add,
            (true, _, true) => Self::Xor,
            (false, true, false) => Self::Subtract,
            (false, true, true) => Self::Intersect,
            _ => self,
        }
    }
    pub const ALL: [Self; 5] = [
        Self::Replace,
        Self::Add,
        Self::Subtract,
        Self::Intersect,
        Self::Xor,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Add => "Add (union)",
            Self::Subtract => "Subtract",
            Self::Intersect => "Intersect",
            Self::Xor => "Invert (xor)",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlphaMode {
    #[default]
    Premultiplied,
    Straight,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FloodMode {
    #[default]
    Contiguous,
    Global,
}
/// FloodFillAlgorithm.GetDistancePremultiplied/GetDistanceStraight in 5.1.12.
pub fn color_distance(a: Pixel, b: Pixel, alpha: AlphaMode) -> u8 {
    if a == b || (alpha == AlphaMode::Premultiplied && a[3] == 0 && b[3] == 0) {
        return 0;
    }
    let convert = |p: Pixel| {
        let mut v = p.map(|v| v as f32 * (1.0f32 / 255.));
        if alpha == AlphaMode::Premultiplied {
            for c in 0..3 {
                v[c] = v[c] * v[3] + (1. - v[3]) * 0.5;
            }
        }
        v
    };
    let (a, b) = (convert(a), convert(b));
    let delta = [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]];
    // Vector4.Dot uses this left-to-right accumulation in the supplied oracle.
    let distance =
        (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2] + delta[3] * delta[3])
            .sqrt();
    (distance * 0.5 * 255.).round().clamp(1., 255.) as u8
}
pub fn flood_mask(
    pixels: &[Pixel],
    width: u32,
    point: (i32, i32),
    tolerance: u8,
    flood: FloodMode,
    alpha: AlphaMode,
) -> Vec<u8> {
    let mut mask = vec![0; pixels.len()];
    let w = width as usize;
    if w == 0
        || !pixels.len().is_multiple_of(w)
        || point.0 < 0
        || point.1 < 0
        || point.0 as usize >= w
    {
        return mask;
    }
    let Some(start) = (point.1 as usize)
        .checked_mul(w)
        .and_then(|i| i.checked_add(point.0 as usize))
        .filter(|i| *i < pixels.len())
    else {
        return mask;
    };
    let basis = pixels[start];
    if flood == FloodMode::Global {
        for (out, &p) in mask.iter_mut().zip(pixels) {
            if color_distance(basis, p, alpha) <= tolerance {
                *out = 255;
            }
        }
        return mask;
    }
    // Fill horizontal spans and enqueue one seed per adjoining run. The output
    // mask also tracks completed spans, avoiding a second full-image visited
    // buffer and a per-pixel frontier on large uniform areas. All comparisons
    // remain relative to the original seed, never the previous pixel.
    let mut queue = vec![start];
    while let Some(i) = queue.pop() {
        if mask[i] != 0 {
            continue;
        }
        let row_start = i / w * w;
        let row_end = row_start + w;
        let mut left = i;
        let mut right = i + 1;
        while left > row_start
            && mask[left - 1] == 0
            && color_distance(basis, pixels[left - 1], alpha) <= tolerance
        {
            left -= 1;
        }
        while right < row_end
            && mask[right] == 0
            && color_distance(basis, pixels[right], alpha) <= tolerance
        {
            right += 1;
        }
        mask[left..right].fill(255);
        for neighbor_row in [row_start.checked_sub(w), Some(row_end)]
            .into_iter()
            .flatten()
            .filter(|&row| row < pixels.len())
        {
            let mut in_run = false;
            for x in left - row_start..right - row_start {
                let j = neighbor_row + x;
                let matches = mask[j] == 0 && color_distance(basis, pixels[j], alpha) <= tolerance;
                if matches && !in_run {
                    queue.push(j);
                }
                in_run = matches;
            }
        }
    }
    mask
}
impl Document {
    pub fn combine_selection(&mut self, mask: Vec<u8>, mode: SelectionMode) {
        assert_eq!(mask.len(), self.layers[0].pixels.len());
        self.selection = Some(Arc::new(
            mask.into_iter()
                .enumerate()
                .map(|(i, b)| {
                    let a = self.selection.as_ref().map_or(0, |m| m[i]);
                    match mode {
                        SelectionMode::Replace => b,
                        SelectionMode::Add => a.max(b),
                        SelectionMode::Subtract => a.saturating_sub(b),
                        SelectionMode::Intersect => a.min(b),
                        SelectionMode::Xor => a.abs_diff(b),
                    }
                })
                .collect(),
        ));
    }
    pub fn magic_wand(
        &mut self,
        point: (i32, i32),
        tolerance: u8,
        flood: FloodMode,
        alpha: AlphaMode,
        composite: bool,
        mode: SelectionMode,
    ) {
        let sample = if composite {
            Arc::new(self.composite())
        } else {
            self.layers[self.active].pixels.clone()
        };
        let mask = flood_mask(&sample, self.width, point, tolerance, flood, alpha);
        self.combine_selection(mask, mode);
    }
    pub fn select_polygon(&mut self, points: &[(i32, i32)], mode: SelectionMode) {
        let mut mask = vec![0; self.layers[0].pixels.len()];
        if points.len() >= 3 {
            let y0 = points.iter().map(|p| p.1).min().unwrap().max(0);
            let y1 = points
                .iter()
                .map(|p| p.1)
                .max()
                .unwrap()
                .min(self.height as i32 - 1);
            for y in y0..=y1 {
                let mut intersections = Vec::new();
                let sample_y = y as f64 + 0.5;
                for (a, b) in points
                    .iter()
                    .zip(points.iter().cycle().skip(1))
                    .take(points.len())
                {
                    let (ay, by) = (a.1 as f64, b.1 as f64);
                    if (ay <= sample_y && by > sample_y) || (by <= sample_y && ay > sample_y) {
                        intersections.push(
                            a.0 as f64 + (sample_y - ay) * (b.0 as f64 - a.0 as f64) / (by - ay),
                        );
                    }
                }
                intersections.sort_by(f64::total_cmp);
                for pair in intersections.as_chunks::<2>().0 {
                    let x0 = (pair[0] - 0.5).ceil().clamp(0., self.width as f64) as usize;
                    let x1 = (pair[1] - 0.5).ceil().clamp(0., self.width as f64) as usize;
                    mask[y as usize * self.width as usize + x0
                        ..y as usize * self.width as usize + x1]
                        .fill(255);
                }
            }
        }
        self.combine_selection(mask, mode);
    }
    pub fn fill_with_options(
        &mut self,
        point: (i32, i32),
        color: Pixel,
        tolerance: u8,
        flood: FloodMode,
        alpha: AlphaMode,
        composite: bool,
    ) {
        self.fill_blended(
            point,
            color,
            tolerance,
            flood,
            alpha,
            composite,
            crate::tool_blend::ToolBlend::default(),
        );
    }
    #[allow(clippy::too_many_arguments)]
    pub fn fill_blended(
        &mut self,
        point: (i32, i32),
        source: impl Into<crate::pattern::FillSource>,
        tolerance: u8,
        flood: FloodMode,
        alpha: AlphaMode,
        composite: bool,
        blender: crate::tool_blend::ToolBlend,
    ) {
        let source = source.into();
        let sample = if composite {
            Arc::new(self.composite())
        } else {
            self.layers[self.active].pixels.clone()
        };
        let mask = flood_mask(&sample, self.width, point, tolerance, flood, alpha);
        let selection = self.selection.clone();
        for (i, p) in Arc::make_mut(&mut self.layers[self.active].pixels)
            .iter_mut()
            .enumerate()
        {
            if mask[i] != 0 && selection.as_ref().is_none_or(|m| m[i] != 0) {
                *p = blender.apply(
                    *p,
                    source.sample(
                        (i % self.width as usize) as i32,
                        (i / self.width as usize) as i32,
                    ),
                    selection.as_ref().map_or(255, |m| m[i]),
                );
            }
        }
    }
    /// Basic integer translation; scale, rotate and subpixel resampling remain separate gates.
    pub fn translate_selection(&mut self, dx: i32, dy: i32, pixels: bool) {
        if dx == 0 && dy == 0 {
            return;
        }
        let w = self.width as usize;
        let h = self.height as usize;
        let old_mask = self
            .selection
            .clone()
            .unwrap_or_else(|| Arc::new(vec![255; w * h]));
        let old = self.layers[self.active].pixels.clone();
        let mut mask = vec![0; w * h];
        let mut moved = if pixels {
            Some(old.as_ref().clone())
        } else {
            None
        };
        if let Some(out) = &mut moved {
            for (p, &m) in out.iter_mut().zip(old_mask.iter()) {
                if m != 0 {
                    *p = TRANSPARENT;
                }
            }
        }
        for (i, &m) in old_mask.iter().enumerate().filter(|(_, m)| **m != 0) {
            let (x, y) = (
                (i % w) as i64 + i64::from(dx),
                (i / w) as i64 + i64::from(dy),
            );
            if x >= 0 && y >= 0 && x < w as i64 && y < h as i64 {
                let dest = y as usize * w + x as usize;
                mask[dest] = m;
                if let Some(out) = &mut moved {
                    out[dest] = blend(out[dest], old[i], 255, BlendMode::Normal);
                }
            }
        }
        if let Some(pixels) = moved {
            self.layers[self.active].pixels = Arc::new(pixels);
        }
        self.selection = Some(Arc::new(mask));
    }
}
