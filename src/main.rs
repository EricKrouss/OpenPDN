use anyhow::{Context, Result};
use openpdn::{app::PaintApp, gpu::Gpu, project};
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--gpu-info") {
        let gpu = pollster::block_on(Gpu::headless())?;
        println!(
            "{} | {:?} | {:?}",
            gpu.info.name, gpu.info.backend, gpu.info.device_type
        );
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "--render") {
        anyhow::ensure!(args.len() == 3, "Usage: openpdn --render INPUT OUTPUT.png");
        let doc = project::open(&PathBuf::from(&args[1]))?;
        let gpu = pollster::block_on(Gpu::headless())?;
        let rendered = gpu.compose(&doc)?;
        project::export(
            &PathBuf::from(&args[2]),
            doc.width,
            doc.height,
            &gpu.read_pixels(&rendered.pixels)?,
        )?;
        return Ok(());
    }
    let smoke_frames = if args.first().is_some_and(|s| s == "--smoke-frames") {
        Some(args.get(1).context("Missing frame count")?.parse::<u32>()?)
    } else {
        None
    };
    if args.first().is_some_and(|s| s == "--help") {
        println!("OpenPDN [IMAGE]\n  --gpu-info\n  --render INPUT OUTPUT\n  --smoke-frames COUNT");
        return Ok(());
    }
    let initial = if smoke_frames.is_none() {
        args.first().map(PathBuf::from)
    } else {
        None
    };
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        persistence_path: std::env::var_os("OPENPDN_STATE_PATH")
            .or_else(|| std::env::var_os("OPENDOTNET_STATE_PATH"))
            .map(PathBuf::from)
            .or_else(|| {
                let previous = eframe::storage_dir("OpenDotNet")?.join("app.ron");
                let current = eframe::storage_dir("OpenPDN")?.join("app.ron");
                (previous.is_file() && !current.exists()).then_some(previous)
            }),
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1280.0, 850.0]),
        ..Default::default()
    };
    eframe::run_native(
        "OpenPDN",
        options,
        Box::new(move |cc| Ok(Box::new(PaintApp::new(cc, initial, smoke_frames)?))),
    )
    .map_err(|e| anyhow::anyhow!("Editor startup failed: {e}"))
}
