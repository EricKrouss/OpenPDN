//! Rectangle and ellipse construction recovered from the supplied 5.1.12 tools.
use crate::{document::Document, resolution::MeasurementUnit};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionDrawMode {
    #[default]
    Normal,
    FixedRatio,
    FixedSize,
}
impl SelectionDrawMode {
    pub const ALL: [Self; 3] = [Self::Normal, Self::FixedRatio, Self::FixedSize];
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::FixedRatio => "Fixed Ratio",
            Self::FixedSize => "Fixed Size",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectionOptions {
    pub mode: SelectionDrawMode,
    pub width: f64,
    pub height: f64,
    pub units: MeasurementUnit,
}
impl Default for SelectionOptions {
    fn default() -> Self {
        Self {
            mode: SelectionDrawMode::Normal,
            width: 400.,
            height: 300.,
            units: MeasurementUnit::Pixel,
        }
    }
}
impl SelectionOptions {
    pub fn sanitized(mut self) -> Self {
        for value in [&mut self.width, &mut self.height] {
            *value = if value.is_finite() {
                value.abs().clamp(0.001, 1e6)
            } else {
                1.
            };
        }
        self
    }
}
/// Returns x/y/width/height before the tool's final intersection and Int32Bound.
pub fn rectangle_bounds(
    a: [f64; 2],
    b: [f64; 2],
    canvas: [f64; 2],
    mode: SelectionDrawMode,
    size: [f64; 2],
    shift: bool,
) -> [f64; 4] {
    if mode == SelectionDrawMode::FixedSize {
        let [w, h] = size.map(f64::abs);
        return [
            b[0].clamp(0., (canvas[0] - w).max(0.)),
            b[1].clamp(0., (canvas[1] - h).max(0.)),
            w,
            h,
        ];
    }
    let left = a[0].min(b[0]).clamp(0., canvas[0]);
    let top = a[1].min(b[1]).clamp(0., canvas[1]);
    let right = (1. + a[0].max(b[0])).clamp(0., canvas[0]);
    let bottom = (1. + a[1].max(b[1])).clamp(0., canvas[1]);
    let (mut w, mut h) = (right - left, bottom - top);
    if mode == SelectionDrawMode::FixedRatio {
        let [sw, sh] = size.map(f64::abs);
        if sw <= 0. || sh <= 0. || !sw.is_finite() || !sh.is_finite() {
            return [a[0], a[1], 0., 0.];
        }
        let ratio = sw / sh;
        if w / sw < h / sh {
            h = (w / ratio).abs();
        } else {
            w = (h * ratio).abs();
        }
    } else if shift {
        w = w.min(h);
        h = w;
    }
    [
        if a[0] <= b[0] { left } else { right - w },
        if a[1] <= b[1] { top } else { bottom - h },
        w,
        h,
    ]
}
pub fn ellipse_bounds(a: [f64; 2], b: [f64; 2], shift: bool) -> [f64; 4] {
    if shift {
        let diameter = (b[0] - a[0]).hypot(b[1] - a[1]);
        [
            (a[0] + b[0] - diameter) * 0.5,
            (a[1] + b[1] - diameter) * 0.5,
            diameter,
            diameter,
        ]
    } else {
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            1. + (a[0] - b[0]).abs(),
            1. + (a[1] - b[1]).abs(),
        ]
    }
}
impl Document {
    pub fn selection_shape_mask(
        &self,
        a: (i32, i32),
        b: (i32, i32),
        ellipse: bool,
        options: SelectionOptions,
        shift: bool,
    ) -> Vec<u8> {
        let a = [f64::from(a.0), f64::from(a.1)];
        let b = [f64::from(b.0), f64::from(b.1)];
        let options = options.sanitized();
        let size = if options.mode == SelectionDrawMode::FixedSize {
            self.resolution()
                .to_pixels([options.width, options.height], options.units)
        } else {
            [options.width, options.height]
        };
        let [mut x, mut y, mut w, mut h] = if ellipse {
            ellipse_bounds(a, b, shift)
        } else {
            rectangle_bounds(
                a,
                b,
                [f64::from(self.width), f64::from(self.height)],
                options.mode,
                size,
                shift,
            )
        };
        if !ellipse {
            let right = (x + w).clamp(0., f64::from(self.width)).ceil();
            let bottom = (y + h).clamp(0., f64::from(self.height)).ceil();
            x = x.clamp(0., f64::from(self.width)).floor();
            y = y.clamp(0., f64::from(self.height)).floor();
            w = right - x;
            h = bottom - y;
        }
        let mut mask = vec![0; (self.width * self.height) as usize];
        if w <= 0. || h <= 0. {
            return mask;
        }
        let x0 = x.floor().clamp(0., f64::from(self.width)) as u32;
        let y0 = y.floor().clamp(0., f64::from(self.height)) as u32;
        let x1 = (x + w).ceil().clamp(0., f64::from(self.width)) as u32;
        let y1 = (y + h).ceil().clamp(0., f64::from(self.height)) as u32;
        for py in y0..y1 {
            for px in x0..x1 {
                if !ellipse
                    || ((f64::from(px) + 0.5 - x - w * 0.5) / (w * 0.5)).powi(2)
                        + ((f64::from(py) + 0.5 - y - h * 0.5) / (h * 0.5)).powi(2)
                        <= 1.
                {
                    mask[(py * self.width + px) as usize] = 255;
                }
            }
        }
        mask
    }
}
