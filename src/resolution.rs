//! Physical measurement conversion and Paint.NET EXIF resolution metadata.
use crate::document::Document;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum MeasurementUnit {
    #[default]
    Pixel = 1,
    Inch = 2,
    Centimeter = 3,
}
impl MeasurementUnit {
    pub const ALL: [Self; 3] = [Self::Pixel, Self::Inch, Self::Centimeter];
    pub fn name(self) -> &'static str {
        match self {
            Self::Pixel => "Pixels",
            Self::Inch => "Inches",
            Self::Centimeter => "Centimeters",
        }
    }
    pub fn default_dpu(self) -> f64 {
        match self {
            Self::Pixel => 1.,
            Self::Inch => 96.,
            Self::Centimeter => 96. / 2.54,
        }
    }
}
pub fn convert_measurement(
    length: f64,
    source: MeasurementUnit,
    basis: MeasurementUnit,
    dpu: f64,
    result: MeasurementUnit,
) -> f64 {
    use MeasurementUnit::*;
    if source == result {
        return length;
    }
    match (source, basis, result) {
        (Inch, _, Centimeter) => length * 2.54,
        (Centimeter, _, Inch) => length / 2.54,
        (Pixel, Pixel, _) => length / result.default_dpu(),
        (_, Pixel, Pixel) => length * source.default_dpu(),
        (_, _, Pixel) if source == basis => length * dpu,
        (Inch, Centimeter, Pixel) => length * (dpu * 2.54),
        (Centimeter, Inch, Pixel) => length * (dpu / 2.54),
        (Pixel, _, _) if basis == result => length / dpu,
        (Pixel, Inch, Centimeter) => length / (dpu / 2.54),
        (Pixel, Centimeter, Inch) => length / (dpu * 2.54),
        _ => unreachable!("All measurement unit combinations are covered"),
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolution {
    pub unit: MeasurementUnit,
    pub x: f64,
    pub y: f64,
}
impl Default for Resolution {
    fn default() -> Self {
        Self {
            unit: MeasurementUnit::Inch,
            x: 96.,
            y: 96.,
        }
    }
}
impl Resolution {
    pub fn to_pixels(self, size: [f64; 2], units: MeasurementUnit) -> [f64; 2] {
        [
            convert_measurement(size[0], units, self.unit, self.x, MeasurementUnit::Pixel),
            convert_measurement(size[1], units, self.unit, self.y, MeasurementUnit::Pixel),
        ]
    }
}
/// The original ImageSizeDialog constrainer keeps fractional physical and pixel
/// dimensions until acceptance. Resolution edits preserve the pixel dimensions.
#[derive(Clone, Debug)]
pub struct ImageSize {
    original: [u32; 2],
    width: f64,
    height: f64,
    units: MeasurementUnit,
    resolution: f64,
    aspect: bool,
}
impl ImageSize {
    /// File > New reads the clipboard afresh and otherwise uses the normal
    /// 800x600 default. Clipboard pixels do not supply a physical DPI value.
    pub fn for_new(clipboard_pixels: Option<[u32; 2]>) -> Self {
        let pixels = clipboard_pixels
            .filter(|[w, h]| *w > 0 && *h > 0)
            .unwrap_or([800, 600]);
        Self::new(pixels)
    }
    pub fn new(original: [u32; 2]) -> Self {
        Self {
            original,
            width: f64::from(original[0]) / 96.,
            height: f64::from(original[1]) / 96.,
            units: MeasurementUnit::Inch,
            resolution: 96.,
            aspect: false,
        }
    }
    pub fn for_document(document: &Document) -> Self {
        let mut size = Self::new([document.width, document.height]);
        let resolution = document.resolution();
        if resolution.unit == MeasurementUnit::Centimeter {
            size.set_units(MeasurementUnit::Centimeter);
        }
        // A pixel-unit document uses the reference's default physical DPI.
        if resolution.unit != MeasurementUnit::Pixel {
            size.set_resolution(resolution.x);
        }
        size
    }
    pub fn width(&self) -> f64 {
        self.width
    }
    pub fn height(&self) -> f64 {
        self.height
    }
    pub fn units(&self) -> MeasurementUnit {
        self.units
    }
    pub fn resolution(&self) -> f64 {
        self.resolution
    }
    pub fn aspect(&self) -> bool {
        self.aspect
    }
    pub fn pixel_width(&self) -> f64 {
        if self.units == MeasurementUnit::Pixel {
            self.width
        } else {
            self.width * self.resolution
        }
    }
    pub fn pixel_height(&self) -> f64 {
        if self.units == MeasurementUnit::Pixel {
            self.height
        } else {
            self.height * self.resolution
        }
    }
    pub fn pixels(&self) -> [u32; 2] {
        [self.pixel_width(), self.pixel_height()]
            .map(|n| n.round().clamp(1., u32::MAX as f64) as u32)
    }
    pub fn set_width(&mut self, value: f64) {
        if self.width == value {
            return;
        }
        self.width = value;
        if self.aspect {
            self.height = value / self.original_aspect();
        }
    }
    pub fn set_height(&mut self, value: f64) {
        if self.height == value {
            return;
        }
        self.height = value;
        if self.aspect {
            self.width = value * self.original_aspect();
        }
    }
    pub fn set_pixel_width(&mut self, value: f64) {
        self.set_width(if self.units == MeasurementUnit::Pixel {
            value
        } else {
            value / self.resolution
        });
    }
    pub fn set_pixel_height(&mut self, value: f64) {
        self.set_height(if self.units == MeasurementUnit::Pixel {
            value
        } else {
            value / self.resolution
        });
    }
    pub fn set_resolution(&mut self, value: f64) {
        if !value.is_finite() || value < 0.01 || self.resolution == value {
            return;
        }
        if self.units != MeasurementUnit::Pixel {
            self.width = self.width * self.resolution / value;
            self.height = self.height * self.resolution / value;
        }
        self.resolution = value;
    }
    pub fn set_units(&mut self, units: MeasurementUnit) {
        if self.units == units {
            return;
        }
        match (self.units, units) {
            (_, MeasurementUnit::Pixel) => {
                self.width *= self.resolution;
                self.height *= self.resolution;
            }
            (MeasurementUnit::Inch, MeasurementUnit::Centimeter) => {
                self.width *= 2.54;
                self.height *= 2.54;
                self.resolution /= 2.54;
            }
            (MeasurementUnit::Centimeter, MeasurementUnit::Inch) => {
                self.width /= 2.54;
                self.height /= 2.54;
                self.resolution *= 2.54;
            }
            // The native dialog only offers physical units here. Allow callers
            // to restore them after Pixel without losing the retained DPI.
            (MeasurementUnit::Pixel, _) => {
                self.width /= self.resolution;
                self.height /= self.resolution;
            }
            _ => unreachable!(),
        }
        self.units = units;
    }
    pub fn set_aspect(&mut self, enabled: bool) {
        if enabled && !self.aspect {
            self.height = self.width / self.original_aspect();
        }
        self.aspect = enabled;
    }
    pub fn set_by_percent(&mut self, scale: f64) {
        let aspect = self.aspect;
        self.aspect = false;
        self.set_pixel_width(f64::from(self.original[0]) * scale);
        self.set_pixel_height(f64::from(self.original[1]) * scale);
        self.aspect = aspect;
    }
    pub fn document_resolution(&self) -> Resolution {
        if self.units == MeasurementUnit::Pixel {
            Resolution {
                unit: self.units,
                x: 1.,
                y: 1.,
            }
        } else {
            Resolution {
                unit: self.units,
                x: self.resolution,
                y: self.resolution,
            }
        }
    }
    fn original_aspect(&self) -> f64 {
        f64::from(self.original[0]) / f64::from(self.original[1])
    }
}
// Resolution values are little-endian SHORT/RATIONAL encoded in the XML's base64.
// Decode at most eight bytes; opaque metadata and profiles are left untouched.
fn decode(value: &str) -> Option<Vec<u8>> {
    let mut result = Vec::new();
    let mut bits = 0u32;
    let mut count = 0;
    for b in value.bytes().filter(|b| !b.is_ascii_whitespace()) {
        if b == b'=' {
            break;
        }
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(v);
        count += 6;
        if count >= 8 {
            count -= 8;
            result.push((bits >> count) as u8);
            if result.len() > 8 {
                return None;
            }
        }
    }
    Some(result)
}
impl Document {
    /// Replace only Image-IFD resolution tags, preserving other opaque metadata.
    /// Rational encoding uses the original integer-or-thousandths rule.
    pub fn set_resolution(&mut self, resolution: Resolution) -> Result<()> {
        ensure!(
            resolution.x.is_finite()
                && resolution.y.is_finite()
                && (0.01..=65536.).contains(&resolution.x)
                && (0.01..=65536.).contains(&resolution.y),
            "Resolution must be between 0.01 and 65536"
        );
        ensure!(
            resolution.unit != MeasurementUnit::Pixel || (resolution.x == 1. && resolution.y == 1.),
            "Pixel resolution must equal one"
        );
        self.metadata.retain(|(key, value)| {
            if !key
                .get(..6)
                .is_some_and(|k| k.eq_ignore_ascii_case("$exif."))
                || value.len() > 1024
                || key.rsplit_once('[').is_some_and(|(_, s)| s != "0]")
            {
                return true;
            }
            let Ok(xml) = roxmltree::Document::parse(value) else {
                return true;
            };
            let root = xml.root_element();
            !(root.tag_name().name() == "exif"
                && root
                    .attribute("id")
                    .is_some_and(|id| matches!(id, "282" | "283" | "296")))
        });
        let rational = |value: f64| {
            let (numerator, denominator) = if value.fract() == 0. {
                (value as u32, 1u32)
            } else {
                ((value * 1000.).floor() as u32, 1000u32)
            };
            [numerator.to_le_bytes(), denominator.to_le_bytes()].concat()
        };
        for (id, kind, bytes) in [
            (296, 3, (resolution.unit as u16).to_le_bytes().to_vec()),
            (282, 5, rational(resolution.x)),
            (283, 5, rational(resolution.y)),
        ] {
            let index = (0u32..)
                .find(|i| {
                    !self
                        .metadata
                        .iter()
                        .any(|(key, _)| key.eq_ignore_ascii_case(&format!("$exif.tag{i}[0]")))
                })
                .unwrap();
            self.metadata.push((
                format!("$exif.tag{index}[0]"),
                format!(
                    "<exif id=\"{id}\" len=\"{}\" type=\"{kind}\" value=\"{}\" />",
                    bytes.len(),
                    encode(&bytes)
                ),
            ));
        }
        Ok(())
    }
    pub fn resolution(&self) -> Resolution {
        let mut unit = None;
        let mut x = None;
        let mut y = None;
        for (key, value) in &self.metadata {
            if !key
                .get(..6)
                .is_some_and(|k| k.eq_ignore_ascii_case("$exif."))
                || value.len() > 1024
            {
                continue;
            }
            // Section zero is the Image IFD. Legacy names without a section are
            // accepted by the original metadata reader as well.
            if let Some((_, section)) = key.rsplit_once('[')
                && section != "0]"
            {
                continue;
            }
            let Ok(xml) = roxmltree::Document::parse(value) else {
                continue;
            };
            let root = xml.root_element();
            if root.tag_name().name() != "exif" {
                continue;
            }
            let Some(id) = root.attribute("id").and_then(|s| s.parse::<u16>().ok()) else {
                continue;
            };
            if ![282, 283, 296].contains(&id) {
                continue;
            }
            let Some(data) = root.attribute("value").and_then(decode) else {
                continue;
            };
            if id == 296 && root.attribute("type") == Some("3") && data.len() == 2 {
                unit = match u16::from_le_bytes([data[0], data[1]]) {
                    1 => Some(MeasurementUnit::Pixel),
                    2 => Some(MeasurementUnit::Inch),
                    3 => Some(MeasurementUnit::Centimeter),
                    _ => None,
                };
            } else if root.attribute("type") == Some("5") && data.len() == 8 {
                let numerator = u32::from_le_bytes(data[..4].try_into().unwrap());
                let denominator = u32::from_le_bytes(data[4..].try_into().unwrap());
                if denominator != 0 {
                    let v = (f64::from(numerator) / f64::from(denominator)).clamp(0.01, 65536.);
                    if id == 282 {
                        x = Some(v);
                    } else if id == 283 {
                        y = Some(v);
                    }
                }
            }
        }
        let unit = unit.unwrap_or(MeasurementUnit::Inch);
        Resolution {
            unit,
            x: x.unwrap_or(unit.default_dpu()),
            y: y.unwrap_or_else(|| x.unwrap_or(unit.default_dpu())),
        }
    }
}
fn encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let word = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for n in 0..4 {
            result.push(if n > chunk.len() {
                '='
            } else {
                ALPHABET[((word >> (18 - n * 6)) & 63) as usize] as char
            });
        }
    }
    result
}
