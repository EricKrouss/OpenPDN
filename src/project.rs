use crate::document::{BlendMode, Document, Layer, Metadata, Pixel, pixel_count};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::Arc,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    width: u32,
    height: u32,
    layers: Vec<LayerInfo>,
    #[serde(default)]
    metadata: Metadata,
    #[serde(default)]
    pdn_header: Option<String>,
    #[serde(default = "default_version")]
    saved_with: [i32; 4],
}
#[derive(Serialize, Deserialize)]
struct LayerInfo {
    name: String,
    visible: bool,
    opacity: u8,
    blend_mode: BlendMode,
    #[serde(default)]
    metadata: Metadata,
    #[serde(default)]
    is_background: bool,
}

fn default_version() -> [i32; 4] {
    [0, 1, 0, 0]
}

// Development format, deliberately distinct from Paint.NET's PDN3 serialization.
pub fn save_project(document: &Document, path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdn"))
    {
        crate::pdn::write(document, &mut temporary)?;
    } else {
        write_project(document, &mut temporary)?;
    }
    if let Ok(metadata) = std::fs::metadata(path) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
    }
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

pub fn write_project(document: &Document, output: impl Write + std::io::Seek) -> Result<()> {
    document.validate()?;
    let manifest = Manifest {
        format: "OpenDotNet".into(),
        version: 2,
        metadata: document.metadata.clone(),
        pdn_header: document.pdn_header.clone(),
        saved_with: document.saved_with,
        width: document.width,
        height: document.height,
        layers: document
            .layers
            .iter()
            .map(|l| LayerInfo {
                name: l.name.clone(),
                visible: l.visible,
                opacity: l.opacity,
                blend_mode: l.blend_mode,
                metadata: l.metadata.clone(),
                is_background: l.is_background,
            })
            .collect(),
    };
    let mut zip = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", options)?;
    zip.write_all(&serde_json::to_vec(&manifest)?)?;
    for (i, layer) in document.layers.iter().enumerate() {
        zip.start_file(format!("layers/{i}.rgba"), options)?;
        zip.write_all(bytemuck::cast_slice(layer.pixels.as_slice()))?;
    }
    zip.finish()?;
    Ok(())
}

pub fn read_project(input: impl Read + std::io::Seek) -> Result<Document> {
    let mut zip = ZipArchive::new(input)?;
    let mut text = Vec::new();
    zip.by_name("manifest.json")?
        .take(1_048_577)
        .read_to_end(&mut text)?;
    ensure!(text.len() <= 1_048_576, "Project manifest is too large");
    let manifest: Manifest = serde_json::from_slice(&text)?;
    ensure!(
        manifest.format == "OpenDotNet" && (1..=2).contains(&manifest.version),
        "Unsupported project version"
    );
    let count = pixel_count(manifest.width, manifest.height)?;
    ensure!(
        !manifest.layers.is_empty() && manifest.layers.len() <= 256,
        "Invalid layer count"
    );
    ensure!(
        count * manifest.layers.len() * 4 <= 512 * 1024 * 1024,
        "Project exceeds development memory limit"
    );
    let mut layers = Vec::new();
    for (i, info) in manifest.layers.into_iter().enumerate() {
        let mut data = Vec::new();
        zip.by_name(&format!("layers/{i}.rgba"))?
            .take((count * 4 + 1) as u64)
            .read_to_end(&mut data)?;
        ensure!(data.len() == count * 4, "Invalid pixel data for layer {i}");
        layers.push(Layer {
            name: info.name,
            visible: info.visible,
            opacity: info.opacity,
            blend_mode: info.blend_mode,
            metadata: info.metadata,
            is_background: info.is_background,
            pixels: Arc::new(data.as_chunks::<4>().0.to_vec()),
        });
    }
    let document = Document {
        width: manifest.width,
        height: manifest.height,
        layers,
        active: 0,
        selection: None,
        metadata: manifest.metadata,
        pdn_header: manifest.pdn_header,
        saved_with: manifest.saved_with,
    };
    document.validate()?;
    Ok(document)
}

pub fn open(path: &Path) -> Result<Document> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("opdn") => read_project(File::open(path)?),
        Some("pdn") => crate::pdn::read(File::open(path)?),
        _ => {
            let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(256 * 1024 * 1024);
            reader.limits(limits);
            let img = reader.decode()?.into_rgba8();
            pixel_count(img.width(), img.height())?;
            let mut doc = Document::new(img.width(), img.height(), [0; 4])?;
            doc.layers[0].pixels = Arc::new(img.pixels().map(|p| p.0).collect());
            Ok(doc)
        }
    }
    .with_context(|| format!("Could not open {}", path.display()))
}

pub fn export(path: &Path, width: u32, height: u32, pixels: &[Pixel]) -> Result<()> {
    ensure!(
        pixels.len() == pixel_count(width, height)?,
        "Invalid export pixel dimensions"
    );
    let format = image::ImageFormat::from_path(path)?;
    let img = image::RgbaImage::from_raw(width, height, bytemuck::cast_slice(pixels).to_vec())
        .context("Invalid image buffer")?;
    if format == image::ImageFormat::Jpeg {
        let mut rgb = image::RgbImage::new(width, height);
        for (dest, p) in rgb.pixels_mut().zip(pixels) {
            for c in 0..3 {
                dest[c] = (crate::document::scale(p[c] as u32, p[3] as u32) + 255 - p[3] as u32)
                    .min(255) as u8;
            }
        }
        rgb.save_with_format(path, format)?;
    } else {
        img.save_with_format(path, format)?;
    }
    Ok(())
}
