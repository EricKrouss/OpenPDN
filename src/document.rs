use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type Pixel = [u8; 4];
/// Original opaque metadata entries, including EXIF resolution and ICC profiles.
pub type Metadata = Vec<(String, String)>;
pub const TRANSPARENT: Pixel = [0; 4];
pub const MAX_PIXELS: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Additive,
    ColorBurn,
    ColorDodge,
    Reflect,
    Glow,
    Overlay,
    Difference,
    Negation,
    Lighten,
    Darken,
    Screen,
    Xor,
}

impl BlendMode {
    pub const ALL: [Self; 14] = [
        Self::Normal,
        Self::Multiply,
        Self::Additive,
        Self::ColorBurn,
        Self::ColorDodge,
        Self::Reflect,
        Self::Glow,
        Self::Overlay,
        Self::Difference,
        Self::Negation,
        Self::Lighten,
        Self::Darken,
        Self::Screen,
        Self::Xor,
    ];
    pub fn name(self) -> &'static str {
        [
            "Normal",
            "Multiply",
            "Additive",
            "Color Burn",
            "Color Dodge",
            "Reflect",
            "Glow",
            "Overlay",
            "Difference",
            "Negation",
            "Lighten",
            "Darken",
            "Screen",
            "Xor",
        ][self as usize]
    }
}

// ByteUtil.FastScale / CompositionOps in the supplied 5.1.12 assemblies.
pub fn scale(a: u32, b: u32) -> u32 {
    let t = a * b + 128;
    (t + (t >> 8)) >> 8
}

pub fn blend_component(d: u32, s: u32, mode: BlendMode) -> u32 {
    match mode {
        BlendMode::Normal => s,
        BlendMode::Multiply => scale(d, s),
        BlendMode::Additive => (d + s).min(255),
        BlendMode::ColorBurn => ((255 - d) * 255)
            .checked_div(s)
            .map_or(0, |v| 255u32.saturating_sub(v)),
        BlendMode::ColorDodge => {
            if s == 255 {
                255
            } else {
                (d * 255 / (255 - s)).min(255)
            }
        }
        BlendMode::Reflect => {
            if s == 255 {
                255
            } else {
                (d * d / (255 - s)).min(255)
            }
        }
        BlendMode::Glow => blend_component(s, d, BlendMode::Reflect),
        BlendMode::Overlay => {
            if d < 128 {
                scale(d * 2, s)
            } else {
                255 - scale((255 - d) * 2, 255 - s)
            }
        }
        BlendMode::Difference => d.abs_diff(s),
        BlendMode::Negation => 255 - (255i32 - d as i32 - s as i32).unsigned_abs(),
        BlendMode::Lighten => d.max(s),
        BlendMode::Darken => d.min(s),
        BlendMode::Screen => 255 - scale(255 - d, 255 - s),
        BlendMode::Xor => d ^ s,
    }
}

pub fn blend(dst: Pixel, src: Pixel, opacity: u8, mode: BlendMode) -> Pixel {
    let sa = scale(src[3] as u32, opacity as u32);
    if sa == 0 {
        return dst;
    }
    let da = dst[3] as u32;
    let remaining = scale(da, 255 - sa);
    let alpha = remaining + sa;
    let overlap = scale(da, sa);
    let mut out = [0, 0, 0, alpha as u8];
    for c in 0..3 {
        let d = dst[c] as u32;
        let s = src[c] as u32;
        out[c] = ((d * remaining + s * (sa - overlap) + blend_component(d, s, mode) * overlap)
            / alpha) as u8;
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
    pub blend_mode: BlendMode,
    pub pixels: Arc<Vec<Pixel>>,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(default)]
    pub is_background: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub width: u32,
    pub height: u32,
    // Bottom to top, independent of panel order.
    pub layers: Vec<Layer>,
    pub active: usize,
    pub selection: Option<Arc<Vec<u8>>>,
    pub metadata: Metadata,
    pub pdn_header: Option<String>,
    pub saved_with: [i32; 4],
}

pub fn pixel_count(width: u32, height: u32) -> Result<usize> {
    ensure!(width > 0 && height > 0, "Image dimensions must be positive");
    let count = u64::from(width) * u64::from(height);
    ensure!(
        count <= MAX_PIXELS as u64,
        "This development build supports at most {MAX_PIXELS} pixels"
    );
    Ok(count as usize)
}

impl Document {
    pub fn new(width: u32, height: u32, color: Pixel) -> Result<Self> {
        let count = pixel_count(width, height)?;
        Ok(Self {
            width,
            height,
            layers: vec![Layer {
                name: "Background".into(),
                visible: true,
                opacity: 255,
                blend_mode: BlendMode::Normal,
                pixels: Arc::new(vec![color; count]),
                metadata: vec![],
                is_background: true,
            }],
            active: 0,
            selection: None,
            metadata: vec![],
            pdn_header: None,
            saved_with: [0, 1, 0, 0],
        })
    }
    pub fn validate(&self) -> Result<()> {
        let count = pixel_count(self.width, self.height)?;
        ensure!(
            !self.layers.is_empty() && self.active < self.layers.len(),
            "Invalid active layer"
        );
        ensure!(self.layers.len() <= 256, "Layer limit is 256 in this build");
        ensure!(
            self.layers.iter().all(|l| l.pixels.len() == count),
            "Invalid layer dimensions"
        );
        ensure!(
            self.selection.as_ref().is_none_or(|m| m.len() == count),
            "Invalid selection dimensions"
        );
        Ok(())
    }
    pub fn selected(&self, i: usize) -> bool {
        self.selection.as_ref().is_none_or(|s| s[i] != 0)
    }
    /// Selected pixel extent as x/y/width/height; an empty mask has no extent.
    pub fn selection_bounds(&self) -> Option<[u32; 4]> {
        let Some(mask) = &self.selection else {
            return Some([0, 0, self.width, self.height]);
        };
        let mut lo = [self.width, self.height];
        let mut hi = [0, 0];
        for (i, value) in mask.iter().enumerate() {
            if *value != 0 {
                let x = i as u32 % self.width;
                let y = i as u32 / self.width;
                lo[0] = lo[0].min(x);
                lo[1] = lo[1].min(y);
                hi[0] = hi[0].max(x + 1);
                hi[1] = hi[1].max(y + 1);
            }
        }
        (hi[0] > lo[0] && hi[1] > lo[1]).then(|| [lo[0], lo[1], hi[0] - lo[0], hi[1] - lo[1]])
    }
    pub fn composite(&self) -> Vec<Pixel> {
        let mut out = vec![TRANSPARENT; (self.width * self.height) as usize];
        for layer in self.layers.iter().filter(|l| l.visible) {
            for (d, s) in out.iter_mut().zip(layer.pixels.iter()) {
                *d = blend(*d, *s, layer.opacity, layer.blend_mode);
            }
        }
        out
    }
    pub fn add_layer(&mut self) {
        let name = format!("Layer {}", self.layers.len() + 1);
        self.layers.insert(
            self.active + 1,
            Layer {
                name,
                visible: true,
                opacity: 255,
                blend_mode: BlendMode::Normal,
                pixels: Arc::new(vec![TRANSPARENT; (self.width * self.height) as usize]),
                metadata: vec![],
                is_background: false,
            },
        );
        self.active += 1;
    }
    pub fn duplicate_layer(&mut self) {
        let mut copy = self.layers[self.active].clone();
        copy.name.push_str(" copy");
        self.layers.insert(self.active + 1, copy);
        self.active += 1;
    }
    pub fn delete_layer(&mut self) {
        if self.layers.len() > 1 {
            self.layers.remove(self.active);
            self.active = self.active.min(self.layers.len() - 1);
        }
    }
    pub fn flatten(&mut self) {
        self.layers = vec![Layer {
            name: "Background".into(),
            visible: true,
            opacity: 255,
            blend_mode: BlendMode::Normal,
            pixels: Arc::new(self.composite()),
            metadata: vec![],
            is_background: true,
        }];
        self.active = 0;
    }
    pub fn clear_selection_pixels(&mut self) {
        let selection = self.selection.clone();
        for (i, p) in Arc::make_mut(&mut self.layers[self.active].pixels)
            .iter_mut()
            .enumerate()
        {
            let coverage = selection.as_ref().map_or(255, |s| s[i]);
            *p = crate::tool_blend::ToolBlend::Overwrite.apply(*p, TRANSPARENT, coverage);
        }
    }
    /// EraseSelectionFunction requires an explicit selection and then deselects.
    pub fn erase_selection(&mut self) {
        if self.selection.is_some() {
            self.clear_selection_pixels();
            self.selection = None;
        }
    }
    pub fn select_rect(&mut self, a: (i32, i32), b: (i32, i32), ellipse: bool) {
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        let mut mask = vec![0; (self.width * self.height) as usize];
        let rx = (x1 - x0 + 1) as f32 / 2.0;
        let ry = (y1 - y0 + 1) as f32 / 2.0;
        for y in y0.max(0)..=(y1.min(self.height as i32 - 1)) {
            for x in x0.max(0)..=(x1.min(self.width as i32 - 1)) {
                let inside = !ellipse
                    || ((x as f32 + 0.5 - x0 as f32 - rx) / rx).powi(2)
                        + ((y as f32 + 0.5 - y0 as f32 - ry) / ry).powi(2)
                        <= 1.0;
                if inside {
                    mask[(y as u32 * self.width + x as u32) as usize] = 255;
                }
            }
        }
        self.selection = Some(Arc::new(mask));
    }
    pub fn invert_selection(&mut self) {
        let count = (self.width * self.height) as usize;
        self.selection = Some(Arc::new(
            (0..count)
                .map(|i| 255 - self.selection.as_ref().map_or(255, |mask| mask[i]))
                .collect(),
        ));
    }
    pub fn paint_line(
        &mut self,
        a: (i32, i32),
        b: (i32, i32),
        size: f32,
        color: Pixel,
        erase: bool,
    ) {
        self.paint_stroke(
            &[a, b],
            size,
            color,
            erase,
            crate::tool_blend::ToolBlend::default(),
        );
    }
    pub fn fill(&mut self, point: (i32, i32), color: Pixel, tolerance: u8) {
        if point.0 < 0
            || point.1 < 0
            || point.0 >= self.width as i32
            || point.1 >= self.height as i32
        {
            return;
        }
        if !self.selected((point.1 as u32 * self.width + point.0 as u32) as usize) {
            return;
        }
        self.fill_with_options(
            point,
            color,
            tolerance,
            crate::selection::FloodMode::Contiguous,
            crate::selection::AlphaMode::Premultiplied,
            false,
        );
    }
    pub fn flip(&mut self, horizontal: bool) {
        let w = self.width as usize;
        let h = self.height as usize;
        let remap = |old: &[Pixel]| -> Vec<Pixel> {
            (0..old.len())
                .map(|i| {
                    let (x, y) = (i % w, i / w);
                    old[if horizontal {
                        y * w + w - 1 - x
                    } else {
                        (h - 1 - y) * w + x
                    }]
                })
                .collect()
        };
        for l in &mut self.layers {
            l.pixels = Arc::new(remap(&l.pixels));
        }
        if let Some(mask) = &self.selection {
            let pixels: Vec<_> = mask.iter().map(|&a| [a, 0, 0, 0]).collect();
            self.selection = Some(Arc::new(remap(&pixels).iter().map(|p| p[0]).collect()));
        }
    }
    pub fn rotate_cw(&mut self) {
        let w = self.width as usize;
        let h = self.height as usize;
        let rotate = |old: &[Pixel]| -> Vec<Pixel> {
            let mut out = vec![TRANSPARENT; old.len()];
            for y in 0..h {
                for x in 0..w {
                    out[x * h + h - 1 - y] = old[y * w + x];
                }
            }
            out
        };
        for l in &mut self.layers {
            l.pixels = Arc::new(rotate(&l.pixels));
        }
        if let Some(mask) = &self.selection {
            let pixels: Vec<_> = mask.iter().map(|&a| [a, 0, 0, 0]).collect();
            self.selection = Some(Arc::new(rotate(&pixels).iter().map(|p| p[0]).collect()));
        }
        std::mem::swap(&mut self.width, &mut self.height);
    }
}

#[derive(Clone)]
struct HistoryEntry {
    label: String,
    before: Document,
    after: Document,
}
pub struct Editor {
    pub document: Document,
    entries: Vec<HistoryEntry>,
    cursor: usize,
    saved: Document,
    pub revision: u64,
}
impl Editor {
    pub fn new(document: Document) -> Self {
        Self {
            saved: document.clone(),
            document,
            entries: vec![],
            cursor: 0,
            revision: 0,
        }
    }
    pub fn dirty(&self) -> bool {
        // Selection and active layer are editor state, not file modifications.
        self.document.width != self.saved.width
            || self.document.height != self.saved.height
            || self.document.layers != self.saved.layers
            || self.document.metadata != self.saved.metadata
            || self.document.pdn_header != self.saved.pdn_header
    }
    pub fn mark_saved(&mut self) {
        self.saved = self.document.clone();
    }
    pub fn record(&mut self, label: &str, before: Document) {
        if before == self.document {
            return;
        }
        self.entries.truncate(self.cursor);
        self.entries.push(HistoryEntry {
            label: label.into(),
            before,
            after: self.document.clone(),
        });
        self.cursor += 1;
        self.revision += 1;
    }
    pub fn edit(&mut self, label: &str, f: impl FnOnce(&mut Document)) {
        let before = self.document.clone();
        f(&mut self.document);
        if self.document.validate().is_err() {
            self.document = before;
            return;
        }
        self.record(label, before);
    }
    pub fn undo(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.document = self.entries[self.cursor].before.clone();
            self.revision += 1;
        }
    }
    pub fn redo(&mut self) {
        if self.cursor < self.entries.len() {
            self.document = self.entries[self.cursor].after.clone();
            self.cursor += 1;
            self.revision += 1;
        }
    }
    pub fn history(&self) -> impl Iterator<Item = (&str, bool)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.label.as_str(), i < self.cursor))
    }
    pub fn history_position(&self) -> usize {
        self.cursor
    }
    pub fn seek_history(&mut self, position: usize) {
        let position = position.min(self.entries.len());
        while self.cursor > position {
            self.undo();
        }
        while self.cursor < position {
            self.redo();
        }
    }
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }
    pub fn can_redo(&self) -> bool {
        self.cursor < self.entries.len()
    }
}
