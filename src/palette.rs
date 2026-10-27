//! Paint.NET palette text interchange and reference default colors.
use crate::document::Pixel;
use anyhow::{Context, Result, ensure};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub const COLOR_COUNT: usize = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    User,
    App,
}
#[derive(Clone, Debug)]
pub struct NamedPalette {
    pub name: String,
    pub location: Location,
    pub colors: Vec<Pixel>,
}
/// A named library uses 96 colors, unlike the raw text parser, which preserves
/// short and empty inputs. PalettesService.EnsureValidPaletteSize pads with white.
pub fn library_colors(colors: &[Pixel]) -> Vec<Pixel> {
    (0..COLOR_COUNT)
        .map(|i| colors.get(i).copied().unwrap_or([255; 4]))
        .collect()
}
/// ChangeExtension(name, ".txt") followed by Windows filename validation. Use
/// the same portable filename restrictions on every platform.
pub fn library_filename(name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let filename = stem.to_owned() + ".txt";
    (!filename
        .chars()
        .any(|c| c < ' ' || "<>:\"/\\|?*".contains(c)))
    .then_some(filename)
}
pub struct Library {
    pub user_dir: PathBuf,
    pub app_dir: Option<PathBuf>,
}
impl Library {
    pub fn entries(&self) -> Result<Vec<NamedPalette>> {
        let mut entries = Vec::new();
        for (location, directory) in [
            (Location::User, Some(&self.user_dir)),
            (Location::App, self.app_dir.as_ref()),
        ] {
            let Some(directory) = directory else { continue };
            let files = match std::fs::read_dir(directory) {
                Ok(files) => files,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e).with_context(|| directory.display().to_string()),
            };
            let start = entries.len();
            for file in files {
                let file = file?;
                let path = file.path();
                if !path.is_file()
                    || !path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
                {
                    continue;
                }
                let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let colors = library_colors(
                    &load(&path).with_context(|| format!("Loading {}", path.display()))?,
                );
                entries.push(NamedPalette {
                    name: name.to_owned(),
                    location,
                    colors,
                });
            }
            entries[start..].sort_by_key(|entry| entry.name.to_lowercase());
            for pair in entries[start..].windows(2) {
                ensure!(
                    pair[0].name.to_lowercase() != pair[1].name.to_lowercase(),
                    "Duplicate palette name: {}",
                    pair[0].name
                );
            }
        }
        Ok(entries)
    }
    pub fn save_named(&self, name: &str, colors: &[Pixel]) -> Result<PathBuf> {
        let filename = library_filename(name).context("Invalid palette name")?;
        let mut path = self.user_dir.join(&filename);
        std::fs::create_dir_all(&self.user_dir)?;
        // Paint.NET palette IDs compare names without case. Reuse the existing
        // filename on case-sensitive hosts instead of creating a duplicate.
        for entry in std::fs::read_dir(&self.user_dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().to_lowercase() == filename.to_lowercase() {
                path = entry.path();
                break;
            }
        }
        save(&path, &library_colors(colors))?;
        Ok(path)
    }
}
pub fn parse_rgb_hex(text: &str) -> Option<[u8; 3]> {
    let text = text.strip_prefix('#').unwrap_or(text).trim();
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(text, 16).ok()?;
    (value <= 0xFFFFFF).then_some([(value >> 16) as u8, (value >> 8) as u8, value as u8])
}
pub fn rgb_to_hsv(color: Pixel) -> [u16; 3] {
    let [r, g, b] = [color[0], color[1], color[2]].map(|v| f64::from(v) / 255.);
    let lo = r.min(g).min(b);
    let hi = r.max(g).max(b);
    let delta = hi - lo;
    let (s, mut h) = if hi == 0. || delta == 0. {
        (0., 0.)
    } else {
        (
            delta / hi,
            if r == hi {
                (g - b) / delta
            } else if g == hi {
                2. + (b - r) / delta
            } else {
                4. + (r - g) / delta
            },
        )
    };
    h *= 60.;
    if h < 0. {
        h += 360.;
    }
    [h as u16, (s * 100.) as u16, (hi * 100.) as u16]
}
pub fn hsv_to_rgb(hsv: [u16; 3]) -> [u8; 3] {
    let h = f64::from(hsv[0] % 360) / 60.;
    let s = f64::from(hsv[1].min(100)) / 100.;
    let v = f64::from(hsv[2].min(100)) / 100.;
    let sector = h.floor() as u32;
    let f = h - f64::from(sector);
    let p = v * (1. - s);
    let q = v * (1. - s * f);
    let t = v * (1. - s * (1. - f));
    let rgb = match sector {
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        5 => [v, p, q],
        _ => [v, t, p],
    };
    rgb.map(|c| (c * 255.).clamp(0., 255.) as u8)
}
pub const DEFAULT: [Pixel; COLOR_COUNT] = [
    [0, 0, 0, 255],
    [64, 64, 64, 255],
    [255, 0, 0, 255],
    [255, 106, 0, 255],
    [255, 216, 0, 255],
    [182, 255, 0, 255],
    [76, 255, 0, 255],
    [0, 255, 33, 255],
    [0, 255, 144, 255],
    [0, 255, 255, 255],
    [0, 148, 255, 255],
    [0, 38, 255, 255],
    [72, 0, 255, 255],
    [178, 0, 255, 255],
    [255, 0, 220, 255],
    [255, 0, 110, 255],
    [255, 255, 255, 255],
    [128, 128, 128, 255],
    [127, 0, 0, 255],
    [127, 51, 0, 255],
    [127, 106, 0, 255],
    [91, 127, 0, 255],
    [38, 127, 0, 255],
    [0, 127, 14, 255],
    [0, 127, 70, 255],
    [0, 127, 127, 255],
    [0, 74, 127, 255],
    [0, 19, 127, 255],
    [33, 0, 127, 255],
    [87, 0, 127, 255],
    [127, 0, 110, 255],
    [127, 0, 55, 255],
    [160, 160, 160, 255],
    [48, 48, 48, 255],
    [255, 127, 127, 255],
    [255, 178, 127, 255],
    [255, 233, 127, 255],
    [218, 255, 127, 255],
    [165, 255, 127, 255],
    [127, 255, 142, 255],
    [127, 255, 197, 255],
    [127, 255, 255, 255],
    [127, 201, 255, 255],
    [127, 146, 255, 255],
    [161, 127, 255, 255],
    [214, 127, 255, 255],
    [255, 127, 237, 255],
    [255, 127, 182, 255],
    [192, 192, 192, 255],
    [96, 96, 96, 255],
    [127, 63, 63, 255],
    [127, 89, 63, 255],
    [127, 116, 63, 255],
    [109, 127, 63, 255],
    [82, 127, 63, 255],
    [63, 127, 71, 255],
    [63, 127, 98, 255],
    [63, 127, 127, 255],
    [63, 100, 127, 255],
    [63, 73, 127, 255],
    [80, 63, 127, 255],
    [107, 63, 127, 255],
    [127, 63, 118, 255],
    [127, 63, 91, 255],
    [0, 0, 0, 128],
    [64, 64, 64, 128],
    [255, 0, 0, 128],
    [255, 106, 0, 128],
    [255, 216, 0, 128],
    [182, 255, 0, 128],
    [76, 255, 0, 128],
    [0, 255, 33, 128],
    [0, 255, 144, 128],
    [0, 255, 255, 128],
    [0, 148, 255, 128],
    [0, 38, 255, 128],
    [72, 0, 255, 128],
    [178, 0, 255, 128],
    [255, 0, 220, 128],
    [255, 0, 110, 128],
    [255, 255, 255, 128],
    [128, 128, 128, 128],
    [127, 0, 0, 128],
    [127, 51, 0, 128],
    [127, 106, 0, 128],
    [91, 127, 0, 128],
    [38, 127, 0, 128],
    [0, 127, 14, 128],
    [0, 127, 70, 128],
    [0, 127, 127, 128],
    [0, 74, 127, 128],
    [0, 19, 127, 128],
    [33, 0, 127, 128],
    [87, 0, 127, 128],
    [127, 0, 110, 128],
    [127, 0, 55, 128],
];

/// The original parser skips malformed lines, accepts variable-length unsigned
/// hexadecimal values, and keeps alpha zero for six-digit input.
pub fn parse(text: &str) -> Vec<Pixel> {
    text.split(['\r', '\n'])
        .filter_map(|line| {
            let text = line.split(';').next()?.trim();
            let text = text.strip_prefix('+').unwrap_or(text);
            let text = text
                .strip_prefix("0x")
                .or_else(|| text.strip_prefix("0X"))
                .unwrap_or(text);
            if text.is_empty() || !text.bytes().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            let argb = u32::from_str_radix(text, 16).ok()?;
            Some([
                (argb >> 16) as u8,
                (argb >> 8) as u8,
                argb as u8,
                (argb >> 24) as u8,
            ])
        })
        .take(COLOR_COUNT)
        .collect()
}
pub fn serialize(colors: &[Pixel]) -> String {
    let mut text = String::from(
        "; Paint.NET Palette File\n; Colors are hexadecimal AARRGGBB, one per line.\n",
    );
    for &[r, g, b, a] in colors.iter().take(COLOR_COUNT) {
        use std::fmt::Write;
        writeln!(text, "{a:02X}{r:02X}{g:02X}{b:02X}").unwrap();
    }
    text
}
pub fn load(path: &Path) -> Result<Vec<Pixel>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .context("Reading palette")?;
    ensure!(bytes.len() <= 1024 * 1024, "Palette exceeds 1 MiB");
    let text = decode_file(&bytes);
    Ok(parse(&text))
}
fn decode_file(bytes: &[u8]) -> String {
    // StreamReader defaults to UTF-8 and recognizes UTF-16/32 BOMs. Invalid
    // encoded sequences become replacement characters rather than aborting
    // the palette; the line parser then skips malformed color entries.
    if let Some(data) = bytes
        .strip_prefix(&[255, 254, 0, 0])
        .or_else(|| bytes.strip_prefix(&[0, 0, 254, 255]))
    {
        let little = bytes[0] == 255;
        let mut text = String::new();
        for group in data.chunks(4) {
            let c = if let Ok(value) = <[u8; 4]>::try_from(group) {
                char::from_u32(if little {
                    u32::from_le_bytes(value)
                } else {
                    u32::from_be_bytes(value)
                })
                .unwrap_or('\u{fffd}')
            } else {
                '\u{fffd}'
            };
            text.push(c);
        }
        text
    } else if let Some(data) = bytes
        .strip_prefix(&[255, 254])
        .or_else(|| bytes.strip_prefix(&[254, 255]))
    {
        let little = bytes[0] == 255;
        let units: Vec<_> = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| {
                if little {
                    u16::from_le_bytes(*v)
                } else {
                    u16::from_be_bytes(*v)
                }
            })
            .collect();
        let mut text = String::from_utf16_lossy(&units);
        if data.len() % 2 != 0 {
            text.push('\u{fffd}');
        }
        text
    } else {
        String::from_utf8_lossy(bytes.strip_prefix(&[239, 187, 191]).unwrap_or(bytes)).into_owned()
    }
}
pub fn save(path: &Path, colors: &[Pixel]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&[239, 187, 191])?;
    file.write_all(serialize(colors).as_bytes())?;
    if let Ok(metadata) = std::fs::metadata(path) {
        file.as_file().set_permissions(metadata.permissions())?;
    }
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
