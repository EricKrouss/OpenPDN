use anyhow::{Context, Result};
use openpdn::{pdn, project};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        args.len() == 2,
        "Usage: cargo run --example pdn_roundtrip -- INPUT.pdn OUTPUT.pdn"
    );
    let document = if args[0] == "--new-resolution" {
        let mut document = openpdn::document::Document::new(3, 2, [12, 34, 56, 78])?;
        document.set_resolution(openpdn::resolution::Resolution {
            unit: openpdn::resolution::MeasurementUnit::Centimeter,
            x: 37.79527559055118,
            y: 42.25,
        })?;
        document
    } else if args[0] == "--new" {
        openpdn::document::Document::new(3, 2, [12, 34, 56, 78])?
    } else {
        project::open(std::path::Path::new(&args[0]))?
    };
    project::save_project(&document, std::path::Path::new(&args[1]))?;
    let reloaded = pdn::read(std::fs::File::open(&args[1])?).context("Reading Rust PDN output")?;
    anyhow::ensure!(
        document.layers == reloaded.layers && document.metadata == reloaded.metadata,
        "PDN round trip changed layers or metadata"
    );
    println!(
        "{}x{}, {} layers; pixels, attributes and metadata preserved",
        document.width,
        document.height,
        document.layers.len()
    );
    Ok(())
}
