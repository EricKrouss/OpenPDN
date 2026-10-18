//! Document geometry and bitmap clipboard operations. Layer properties and file
//! metadata survive geometry edits; callers commit one history transaction.
use crate::document::{Document, Pixel, TRANSPARENT, blend, pixel_count, scale};
use anyhow::{Result, ensure};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}
impl Anchor {
    pub const ALL: [Self; 9] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Left,
        Self::Center,
        Self::Right,
        Self::BottomLeft,
        Self::Bottom,
        Self::BottomRight,
    ];
    pub fn offset(self, old: [u32; 2], new: [u32; 2]) -> [i32; 2] {
        let slot = Self::ALL.iter().position(|a| *a == self).unwrap();
        let delta = [new[0] as i32 - old[0] as i32, new[1] as i32 - old[1] as i32];
        // C# integer division truncates towards zero, including odd shrink sizes.
        [
            match slot % 3 {
                0 => 0,
                1 => delta[0] / 2,
                _ => delta[0],
            },
            match slot / 3 {
                0 => 0,
                1 => delta[1] / 2,
                _ => delta[1],
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClipboardImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Pixel>,
    pub origin: [i32; 2],
}
impl ClipboardImage {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.pixels.len() == pixel_count(self.width, self.height)?,
            "Invalid clipboard image dimensions"
        );
        Ok(())
    }
}

impl Document {
    pub fn canvas_size(
        &mut self,
        width: u32,
        height: u32,
        anchor: Anchor,
        fill: Pixel,
    ) -> Result<()> {
        let count = pixel_count(width, height)?;
        if (width, height) == (self.width, self.height) {
            return Ok(());
        }
        let [dx, dy] = anchor.offset([self.width, self.height], [width, height]);
        for layer in &mut self.layers {
            let mut pixels = vec![fill; count];
            for y in 0..self.height {
                let ny = y as i32 + dy;
                if !(0..height as i32).contains(&ny) {
                    continue;
                }
                let x0 = 0.max(-dx) as u32;
                let x1 = self.width.min((width as i32 - dx).max(0) as u32);
                if x1 > x0 {
                    let src = (y * self.width + x0) as usize;
                    let dst = (ny as u32 * width + (x0 as i32 + dx) as u32) as usize;
                    pixels[dst..dst + (x1 - x0) as usize]
                        .copy_from_slice(&layer.pixels[src..src + (x1 - x0) as usize]);
                }
            }
            layer.pixels = Arc::new(pixels);
        }
        self.width = width;
        self.height = height;
        self.selection = None;
        Ok(())
    }
    pub fn crop_to_selection(&mut self) {
        if self.selection.is_none() {
            return;
        }
        let Some([x0, y0, width, height]) = self.selection_bounds() else {
            return;
        };
        let selection = self.selection.as_ref().unwrap();
        for layer in &mut self.layers {
            let mut pixels = Vec::with_capacity((width * height) as usize);
            for y in y0..y0 + height {
                for x in x0..x0 + width {
                    let i = (y * self.width + x) as usize;
                    let mut pixel = layer.pixels[i];
                    pixel[3] = scale(u32::from(pixel[3]), u32::from(selection[i])) as u8;
                    pixels.push(pixel);
                }
            }
            layer.pixels = Arc::new(pixels);
        }
        self.width = width;
        self.height = height;
        self.selection = None;
    }
    pub fn merge_down(&mut self) {
        if self.active == 0 {
            return;
        }
        let upper = self.layers.remove(self.active);
        self.active -= 1;
        let lower = &mut self.layers[self.active];
        // Original MergeLayerDownFunction ignores visibility and keeps lower properties.
        for (dst, src) in Arc::make_mut(&mut lower.pixels)
            .iter_mut()
            .zip(upper.pixels.iter())
        {
            *dst = blend(*dst, *src, upper.opacity, upper.blend_mode);
        }
    }
    pub fn copy_selection(&self, merged: bool) -> Option<ClipboardImage> {
        let [x0, y0, width, height] = self.selection_bounds()?;
        let source = if merged {
            Arc::new(self.composite())
        } else {
            self.layers[self.active].pixels.clone()
        };
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for y in y0..y0 + height {
            for x in x0..x0 + width {
                let i = (y * self.width + x) as usize;
                let mut pixel = source[i];
                if let Some(mask) = &self.selection {
                    pixel[3] = scale(u32::from(pixel[3]), u32::from(mask[i])) as u8;
                }
                pixels.push(pixel);
            }
        }
        Some(ClipboardImage {
            width,
            height,
            pixels,
            origin: [x0 as i32, y0 as i32],
        })
    }
    pub fn paste_image(
        &mut self,
        image: &ClipboardImage,
        origin: [i32; 2],
        new_layer: bool,
    ) -> Result<()> {
        image.validate()?;
        if new_layer {
            ensure!(self.layers.len() < 256, "Layer limit reached");
            self.add_layer();
        }
        let mut mask = vec![0; pixel_count(self.width, self.height)?];
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        for y in 0..image.height {
            let ny = y as i64 + i64::from(origin[1]);
            if !(0..i64::from(self.height)).contains(&ny) {
                continue;
            }
            for x in 0..image.width {
                let nx = x as i64 + i64::from(origin[0]);
                if !(0..i64::from(self.width)).contains(&nx) {
                    continue;
                }
                let i = ny as usize * self.width as usize + nx as usize;
                let src = image.pixels[(y * image.width + x) as usize];
                pixels[i] = blend(pixels[i], src, 255, crate::document::BlendMode::Normal);
                mask[i] = 255;
            }
        }
        self.selection = Some(Arc::new(mask));
        Ok(())
    }
    pub fn rotate_ccw(&mut self) {
        for _ in 0..3 {
            self.rotate_cw();
        }
    }
    pub fn rotate_180(&mut self) {
        self.flip(true);
        self.flip(false);
    }
}

impl ClipboardImage {
    pub fn into_document(self) -> Result<Document> {
        self.validate()?;
        let mut doc = Document::new(self.width, self.height, TRANSPARENT)?;
        doc.layers[0].pixels = Arc::new(self.pixels);
        Ok(doc)
    }
}
