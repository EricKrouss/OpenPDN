//! A floating selection keeps its original pixels through successive edits,
//! including edits that temporarily place it outside the canvas.
use crate::{
    document::{Document, Editor, Pixel, scale},
    editing::{Anchor, ClipboardImage},
};
use anyhow::{Result, ensure};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub center: [f32; 2],
    pub scale: [f32; 2],
    /// Clockwise degrees in image coordinates.
    pub angle: f32,
}
impl Transform {
    pub fn point(self, point: [f32; 2], size: [u32; 2]) -> [f32; 2] {
        let x = (point[0] - size[0] as f32 * 0.5) * self.scale[0];
        let y = (point[1] - size[1] as f32 * 0.5) * self.scale[1];
        let (sin, cos) = self.angle.to_radians().sin_cos();
        [
            self.center[0] + cos * x - sin * y,
            self.center[1] + sin * x + cos * y,
        ]
    }
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.center
                .iter()
                .chain(&self.scale)
                .chain([&self.angle])
                .all(|v| v.is_finite()),
            "Nonfinite transform"
        );
        ensure!(
            self.center.iter().all(|v| v.abs() <= 1_000_000.) && self.angle.abs() <= 360_000.,
            "Transform exceeds coordinate limits"
        );
        ensure!(
            self.scale
                .iter()
                .all(|v| (0.001..=1000.).contains(&v.abs())),
            "Transform scale is outside 0.001–1000"
        );
        Ok(())
    }
}

pub struct FloatingSelection {
    pub before: Document,
    pub base: Document,
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<Vec<Pixel>>,
    pub mask: Arc<Vec<u8>>,
    pub transform: Transform,
    pub original_transform: Transform,
    pub include_pixels: bool,
    pub pasted: bool,
    pub leave_copy: bool,
}

/// The original PasteInToNewLayerAction records the blank layer first;
/// PasteImageAction then records any canvas expansion before MoveTool's paste.
/// Prepare and validate the complete result before changing the live history.
pub struct PastePlan {
    pub floating: FloatingSelection,
    preparations: Vec<(&'static str, Document)>,
}
impl PastePlan {
    pub fn new(
        document: &Document,
        image: ClipboardImage,
        new_layer: bool,
        expand: bool,
    ) -> Result<Self> {
        document.validate()?;
        image.validate()?;
        let layers = document.layers.len() + usize::from(new_layer);
        let size = if expand {
            [
                document.width.max(image.width),
                document.height.max(image.height),
            ]
        } else {
            [document.width, document.height]
        };
        let count = crate::document::pixel_count(size[0], size[1])?;
        ensure!(layers <= 256, "Layer limit reached");
        ensure!(
            count as u64 * layers as u64 * 4 <= 512 * 1024 * 1024,
            "Paste exceeds development memory limit"
        );
        let mut base = document.clone();
        let mut preparations = Vec::new();
        if new_layer {
            base.add_layer();
            preparations.push(("Add New Layer", base.clone()));
        }
        if size != [base.width, base.height] {
            base.canvas_size(size[0], size[1], Anchor::TopLeft, [0; 4])?;
            preparations.push(("Canvas Size", base.clone()));
        }
        Ok(Self {
            floating: FloatingSelection::from_paste(&base, image, false, false)?,
            preparations,
        })
    }
    pub fn begin(self, editor: &mut Editor) -> FloatingSelection {
        for (label, document) in self.preparations {
            let before = std::mem::replace(&mut editor.document, document);
            editor.record(label, before);
        }
        self.floating
    }
}
impl FloatingSelection {
    pub fn from_selection(
        document: &Document,
        include_pixels: bool,
        leave_copy: bool,
    ) -> Result<Option<Self>> {
        document.validate()?;
        let bounds = if document.selection.is_some() {
            let Some(bounds) = document.selection_bounds() else {
                return Ok(None);
            };
            bounds
        } else {
            [0, 0, document.width, document.height]
        };
        let [x, y, width, height] = bounds;
        let mut pixels = Vec::with_capacity((width * height) as usize);
        let mut mask = Vec::with_capacity(pixels.capacity());
        let mut base = document.clone();
        let layer = &document.layers[document.active].pixels;
        for cy in y..y + height {
            for cx in x..x + width {
                let i = (cy * document.width + cx) as usize;
                let coverage = document.selection.as_ref().map_or(255, |m| m[i]);
                pixels.push(layer[i]);
                mask.push(coverage);
                if include_pixels && !leave_copy && coverage != 0 {
                    let p = &mut Arc::make_mut(&mut base.layers[document.active].pixels)[i];
                    p[3] = scale(u32::from(p[3]), u32::from(255 - coverage)) as u8;
                }
            }
        }
        let transform = Transform {
            center: [
                x as f32 + width as f32 * 0.5,
                y as f32 + height as f32 * 0.5,
            ],
            scale: [1.; 2],
            angle: 0.,
        };
        Ok(Some(Self {
            before: document.clone(),
            base,
            width,
            height,
            pixels: Arc::new(pixels),
            mask: Arc::new(mask),
            transform,
            original_transform: transform,
            include_pixels,
            pasted: false,
            leave_copy,
        }))
    }
    pub fn from_paste(
        document: &Document,
        image: ClipboardImage,
        new_layer: bool,
        expand: bool,
    ) -> Result<Self> {
        image.validate()?;
        let mut base = document.clone();
        if expand {
            base.canvas_size(
                base.width.max(image.width),
                base.height.max(image.height),
                Anchor::TopLeft,
                [0; 4],
            )?;
        }
        if new_layer {
            ensure!(base.layers.len() < 256, "Too many layers");
            base.add_layer();
        }
        let x = image.origin[0].clamp(0, base.width.saturating_sub(image.width) as i32);
        let y = image.origin[1].clamp(0, base.height.saturating_sub(image.height) as i32);
        let transform = Transform {
            center: [
                x as f32 + image.width as f32 * 0.5,
                y as f32 + image.height as f32 * 0.5,
            ],
            scale: [1.; 2],
            angle: 0.,
        };
        Ok(Self {
            before: document.clone(),
            base,
            width: image.width,
            height: image.height,
            mask: Arc::new(vec![255; image.pixels.len()]),
            pixels: Arc::new(image.pixels),
            transform,
            original_transform: transform,
            include_pixels: true,
            pasted: true,
            leave_copy: false,
        })
    }
    pub fn label(&self) -> &'static str {
        if self.pasted {
            "Paste"
        } else if self.include_pixels {
            "Move Selected Pixels"
        } else {
            "Move Selection"
        }
    }
    pub fn corners(&self) -> [[f32; 2]; 4] {
        [
            [0., 0.],
            [self.width as f32, 0.],
            [self.width as f32, self.height as f32],
            [0., self.height as f32],
        ]
        .map(|p| self.transform.point(p, [self.width, self.height]))
    }
}
