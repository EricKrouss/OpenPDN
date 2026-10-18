use crate::document::{Document, Pixel};
use anyhow::{Context, Result, ensure};
use eframe::egui_wgpu::wgpu::{self, util::DeviceExt};
use std::{sync::mpsc, time::Duration};
mod effect_data;
mod filters;
pub mod gaussian;
mod kernels;
mod palette;
mod redeye;
mod resize;
mod transform;
pub use effect_data::MIX_MODES;
pub use resize::Resampler;
mod noise;
mod temperature;

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
    compositor: wgpu::ComputePipeline,
    adjustment: wgpu::ComputePipeline,
    transfer: wgpu::ComputePipeline,
    resizer: wgpu::ComputePipeline,
    transformer: wgpu::ComputePipeline,
    gaussian: wgpu::ComputePipeline,
    filter_module: wgpu::ShaderModule,
    filter_layout: wgpu::BindGroupLayout,
    filter_pipelines:
        std::sync::Mutex<std::collections::HashMap<u32, std::sync::Arc<wgpu::ComputePipeline>>>,
}
pub struct RenderedImage {
    pub texture: wgpu::Texture,
    pub pixels: wgpu::Buffer,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ImageOperation {
    InvertColors,
    InvertAlpha,
    BlackWhite,
    Sepia,
    Posterize,
    BrightnessContrast,
    Exposure,
    HueSaturation,
    Vignette,
    Relief,
    Emboss = 11,
    Morphology = 12,
    Levels = 13,
    Curves = 14,
    AutoLevel = 15,
    TemperatureTint = 16,
    HighlightsShadows = 17,
    SquareBlur = 18,
    GaussianBlur = 19,
    Fragment = 20,
    RadialBlur = 21,
    ZoomBlur = 22,
    Bulge = 23,
    Twist = 24,
    PolarInversion = 25,
    TileReflection = 26,
    Glow = 27,
    PencilSketch = 28,
    MotionBlur = 29,
    InkSketch = 30,
    SurfaceBlur = 31,
    SoftenPortrait = 32,
    DropShadow = 33,
    Median = 34,
    Outline = 35,
    OilPainting = 36,
    ReduceNoise = 37,
    AddNoise = 38,
    FrostedGlass = 39,
    Bokeh = 40,
    SketchBlur = 41,
    Quantize = 42,
    Crystalize = 43,
    Dents = 44,
    Pixelate = 45,
    RedEye = 46,
    Sharpen = 47,
    Straighten = 48,
    Clouds = 49,
    Julia = 50,
    Mandelbrot = 51,
    Turbulence = 52,
    EdgeDetect = 53,
}
pub use ImageOperation as Adjustment;
impl ImageOperation {
    pub fn name(self) -> &'static str {
        match self {
            Self::InvertColors => "Invert Colors",
            Self::InvertAlpha => "Invert Alpha",
            Self::BlackWhite => "Black and White",
            Self::Sepia => "Sepia",
            Self::Posterize => "Posterize",
            Self::BrightnessContrast => "Brightness / Contrast",
            Self::Exposure => "Exposure",
            Self::HueSaturation => "Hue / Saturation",
            Self::Vignette => "Vignette",
            Self::Relief => "Relief",
            Self::Emboss => "Emboss",
            Self::Morphology => "Morphology",
            Self::Levels => "Levels",
            Self::Curves => "Curves",
            Self::AutoLevel => "Auto-Level",
            Self::TemperatureTint => "Temperature and Tint",
            Self::HighlightsShadows => "Highlights / Shadows",
            Self::SquareBlur => "Square Blur",
            Self::GaussianBlur => "Gaussian Blur",
            Self::Fragment => "Fragment",
            Self::RadialBlur => "Radial Blur",
            Self::ZoomBlur => "Zoom Blur",
            Self::Bulge => "Bulge",
            Self::Twist => "Twist",
            Self::PolarInversion => "Polar Inversion",
            Self::TileReflection => "Tile Reflection",
            Self::Glow => "Glow",
            Self::PencilSketch => "Pencil Sketch",
            Self::MotionBlur => "Motion Blur",
            Self::InkSketch => "Ink Sketch",
            Self::SurfaceBlur => "Surface Blur",
            Self::SoftenPortrait => "Soften Portrait",
            Self::DropShadow => "Drop Shadow",
            Self::Median => "Median",
            Self::Outline => "Outline",
            Self::OilPainting => "Oil Painting",
            Self::ReduceNoise => "Reduce Noise",
            Self::AddNoise => "Add Noise",
            Self::FrostedGlass => "Frosted Glass",
            Self::Bokeh => "Bokeh",
            Self::SketchBlur => "Sketch Blur",
            Self::Quantize => "Quantize",
            Self::Crystalize => "Crystalize",
            Self::Dents => "Dents",
            Self::Pixelate => "Pixelate",
            Self::RedEye => "Red Eye Removal",
            Self::Sharpen => "Sharpen",
            Self::Straighten => "Straighten",
            Self::Clouds => "Clouds",
            Self::Julia => "Julia Fractal",
            Self::Mandelbrot => "Mandelbrot Fractal",
            Self::Turbulence => "Turbulence",
            Self::EdgeDetect => "Edge Detect",
        }
    }
    pub fn defaults(self) -> [f32; 4] {
        match self {
            Self::Posterize => [16.; 4],
            Self::Sepia => [50., 0., 0., 0.],
            Self::HueSaturation => [0., 100., 0., 0.],
            Self::Vignette => [0., 0., 0.5, 1.],
            Self::Relief => [45., 0., 0., 0.],
            Self::Morphology => [5., 5., 0., 0.],
            Self::HighlightsShadows => [0., 0., 0., 1.25],
            Self::SquareBlur => [6., 0., 0., 0.],
            Self::GaussianBlur => [2., 0., 4., 0.],
            Self::Fragment => [4., 8., 0., 0.],
            Self::RadialBlur => [4., 0., 0., 2.],
            Self::ZoomBlur => [1.25, 2., 0., 0.],
            Self::Bulge => [0.45, 0., 0., 0.],
            Self::Twist => [30., 1., 0., 0.],
            Self::PolarInversion => [1., 0., 0., 2.],
            Self::TileReflection => [30., 40., 8., 2.],
            Self::Glow => [6., 10., 10., 0.],
            Self::PencilSketch => [2., 0., 0., 0.],
            Self::MotionBlur => [25., 10., 1., 0.],
            Self::InkSketch => [50., 50., 0., 0.],
            Self::SurfaceBlur => [6., 15., 0., 0.],
            Self::SoftenPortrait => [5., 0., 10., 0.],
            Self::DropShadow => [10., 10., -45., 0.75],
            Self::Median => [10., 50., 8., 0.],
            Self::Outline => [3., 50., 8., 0.],
            Self::OilPainting => [10., 0.2, 0., 0.],
            Self::ReduceNoise => [10., 0.4, 0., 0.],
            Self::AddNoise => [64., 100., 100., 0.],
            Self::FrostedGlass => [3., 0., 1., 2.],
            Self::Bokeh => [25., 0., 3., 0.],
            Self::SketchBlur => [25., 50., 3., 0.],
            Self::Quantize => [1., 256., 7., 128.],
            Self::Crystalize => [8., 2., 0., 0.],
            Self::Dents => [25., 50., 10., 10.],
            Self::Pixelate => [2., 0., 0., 2.],
            Self::RedEye => [3., 0., 0., 0.],
            Self::Sharpen => [2., 0., 0., 0.],
            Self::Straighten => [0., 0., 0., 0.],
            Self::Clouds => [250., 0.5, 0., 0.],
            Self::Julia => [4., 1., 0., 2.],
            Self::Mandelbrot => [1., 10., 0., 2.],
            Self::Turbulence => [4., 100., 4096., 0.],
            Self::EdgeDetect => [0.5, 0., 0., 0.],
            _ => [0.; 4],
        }
    }
    pub fn parameters(self) -> [f32; 8] {
        let mut values = [0.; 8];
        values[..4].copy_from_slice(&self.defaults());
        match self {
            Self::Dents => values[5] = 2.,
            Self::Pixelate | Self::Julia => values[4] = 2.,
            Self::Mandelbrot => values[5] = 2.,
            Self::Turbulence => values[4] = 1.,
            Self::Clouds => {
                values[5] = 255.;
                values[6] = 16777215.;
                values[7] = 255.;
            }
            _ => {}
        }
        if self == Self::ZoomBlur {
            values[4] = 2.;
        }
        if matches!(
            self,
            Self::Bulge | Self::Twist | Self::PolarInversion | Self::TileReflection
        ) {
            values[4] = 2.;
        }
        values
    }
    pub fn is_effect(self) -> bool {
        matches!(
            self,
            Self::Vignette
                | Self::Relief
                | Self::Emboss
                | Self::Morphology
                | Self::SquareBlur
                | Self::GaussianBlur
                | Self::Fragment
                | Self::RadialBlur
                | Self::ZoomBlur
                | Self::Bulge
                | Self::Twist
                | Self::PolarInversion
                | Self::TileReflection
                | Self::Glow
                | Self::PencilSketch
                | Self::MotionBlur
                | Self::InkSketch
                | Self::SurfaceBlur
                | Self::SoftenPortrait
                | Self::DropShadow
                | Self::Median
                | Self::Outline
                | Self::OilPainting
                | Self::ReduceNoise
                | Self::AddNoise
                | Self::FrostedGlass
                | Self::Bokeh
                | Self::SketchBlur
                | Self::Quantize
                | Self::Crystalize
                | Self::Dents
                | Self::Pixelate
                | Self::RedEye
                | Self::Sharpen
                | Self::Straighten
                | Self::Clouds
                | Self::Julia
                | Self::Mandelbrot
                | Self::Turbulence
                | Self::EdgeDetect
        )
    }
    /// Frozen 5.1.12 property order, limits and integral controls.
    pub fn controls(self) -> &'static [(&'static str, f32, f32, bool)] {
        match self {
            Self::Sepia => &[("Intensity", 0., 100., true)],
            Self::BrightnessContrast => &[
                ("Brightness", -100., 100., true),
                ("Contrast", -100., 100., true),
            ],
            Self::Exposure => &[("Exposure", -200., 200., true)],
            Self::HueSaturation => &[
                ("Hue", -180., 180., true),
                ("Saturation", 0., 200., true),
                ("Lightness", -100., 100., true),
            ],
            Self::Vignette => &[
                ("Center X", -1., 1., false),
                ("Center Y", -1., 1., false),
                ("Radius", 0.1, 4., false),
                ("Strength", 0., 1., false),
            ],
            Self::Relief => &[("Angle", -180., 180., false)],
            Self::Emboss => &[("Angle", 0., 360., false)],
            Self::Morphology => &[
                ("Width", 1., 100., true),
                ("Height", 1., 100., true),
                ("Erode", 0., 1., true),
            ],
            Self::TemperatureTint => &[
                ("Temperature", -100., 100., true),
                ("Tint", -100., 100., true),
            ],
            Self::HighlightsShadows => &[
                ("Highlights", -100., 100., true),
                ("Shadows", -100., 100., true),
                ("Clarity", -100., 100., true),
                ("Radius", 0., 10., false),
            ],
            Self::SquareBlur => &[
                ("Radius", 0., 300., false),
                ("Gamma Boost", -0.99, 2., false),
            ],
            Self::GaussianBlur => &[
                ("Radius", 0., 300., false),
                ("Gamma Boost", -0.99, 2., false),
                ("Quality", 1., 4., true),
            ],
            Self::Fragment => &[
                ("Fragments", 2., 200., true),
                ("Distance", 0., 400., false),
                ("Rotation", 0., 360., false),
            ],
            Self::RadialBlur => &[
                ("Angle", 0., 360., false),
                ("Center X", -2., 2., false),
                ("Center Y", -2., 2., false),
                ("Quality", 1., 8., false),
            ],
            Self::ZoomBlur => &[
                ("Distance", 0.25, 4., false),
                ("Focus", 1., 4., false),
                ("Center X", -2., 2., false),
                ("Center Y", -2., 2., false),
                ("Quality", 1., 8., false),
            ],
            Self::Bulge => &[
                ("Strength", -3., 1., false),
                ("Center X", -1., 1., false),
                ("Center Y", -1., 1., false),
                ("Edge Mode", 0., 3., true),
                ("Quality", 1., 8., true),
            ],
            Self::Twist => &[
                ("Amount", -200., 200., true),
                ("Size", 0.01, 2., false),
                ("Center X", -2., 2., false),
                ("Center Y", -2., 2., false),
                ("Quality", 1., 8., true),
            ],
            Self::PolarInversion => &[
                ("Scale", -8., 8., false),
                ("Center X", -2., 2., false),
                ("Center Y", -2., 2., false),
                ("Edge Mode", 0., 3., true),
                ("Quality", 1., 8., true),
            ],
            Self::TileReflection => &[
                ("Angle", -180., 180., false),
                ("Tile Size", 1., 1600., false),
                ("Curvature", -200., 200., false),
                ("Edge Mode", 0., 3., true),
                ("Quality", 1., 8., true),
            ],
            Self::Glow => &[
                ("Radius", 1., 20., false),
                ("Brightness", -100., 100., true),
                ("Contrast", -100., 100., true),
            ],
            Self::PencilSketch => &[
                ("Pencil Tip Size", 1., 20., false),
                ("Color Range", -20., 20., false),
            ],
            Self::MotionBlur => &[
                ("Angle", -180., 180., false),
                ("Distance", 1., 500., false),
                ("Centered", 0., 1., true),
                ("Edge Mode", 0., 3., true),
            ],
            Self::InkSketch => &[("Ink Outline", 0., 99., true), ("Coloring", 0., 100., true)],
            Self::SurfaceBlur => &[("Radius", 1., 50., true), ("Threshold", 1., 100., true)],
            Self::SoftenPortrait => &[
                ("Softness", 0., 10., false),
                ("Lighting", -20., 20., true),
                ("Warmth", 0., 20., true),
            ],
            Self::DropShadow => &[
                ("Blur Radius", 0., 100., false),
                ("Distance", 0., 100., false),
                ("Angle", -180., 180., false),
                ("Opacity", 0., 1., false),
                ("Color", 0., 16777215., true),
                ("Shadow Only", 0., 1., true),
            ],
            Self::Median => &[
                ("Radius", 0., 100., true),
                ("Percentile", 0., 100., true),
                ("Quality", 1., 9., true),
            ],
            Self::Outline => &[
                ("Thickness", 1., 70., true),
                ("Intensity", 0., 100., true),
                ("Quality", 1., 9., true),
            ],
            Self::OilPainting => &[
                ("Brush Size", 1., 50., true),
                ("Granularity", 0.01, 1., false),
                ("Kernel Shape", 0., 1., true),
            ],
            Self::ReduceNoise => &[("Radius", 0., 50., true), ("Strength", 0., 1., false)],
            Self::AddNoise => &[
                ("Intensity", 0., 100., true),
                ("Saturation", 0., 400., true),
                ("Coverage", 0., 100., false),
                ("Seed", 0., 255., true),
            ],
            Self::FrostedGlass => &[
                ("Maximum Scatter Radius", 0., 500., false),
                ("Minimum Scatter Radius", 0., 500., false),
                ("Diffusion", 0.01, 3., false),
                ("Smoothness", 1., 8., true),
                ("Seed", 0., 255., true),
            ],
            Self::Bokeh => &[
                ("Radius", 0., 300., false),
                ("Gamma Boost", -0.99, 2., false),
                ("Quality", 1., 10., true),
            ],
            Self::SketchBlur => &[
                ("Radius", 0., 100., false),
                ("Percentile", 0., 100., true),
                ("Smoothness", 1., 20., true),
            ],
            Self::Quantize => &[
                ("Algorithm", 0., 1., true),
                ("Colors", 2., 256., true),
                ("Dithering", 0., 8., true),
                ("Alpha Threshold", 0., 255., true),
            ],
            Self::Crystalize => &[
                ("Cell Size", 2., 250., true),
                ("Quality", 1., 5., true),
                ("Seed", 0., 255., true),
            ],
            Self::Dents => &[
                ("Scale", 0., 200., false),
                ("Refraction", 0., 200., false),
                ("Detail", 0., 100., false),
                ("Turbulence", 0., 100., false),
                ("Angle", -180., 180., false),
                ("Quality", 1., 8., true),
                ("Seed", 0., 255., true),
            ],
            Self::Pixelate => &[
                ("Cell Size", 1., 256., true),
                ("Center X", -1., 1., false),
                ("Center Y", -1., 1., false),
                ("Scale Down", 0., 5., true),
                ("Scale Up", 0., 2., true),
            ],
            Self::RedEye => &[("Strength", 1., 5., true)],
            Self::Sharpen => &[("Amount", 0., 10., false), ("Threshold", 0., 1., false)],
            Self::Straighten => &[("Angle", -45., 45., false), ("Sampling", 0., 2., true)],
            Self::Clouds => &[
                ("Scale", 2., 1000., true),
                ("Roughness", 0., 1., false),
                ("Blend Mode", 0., 45., true),
                ("Seed", 0., 255., true),
                ("Color 1", 0., 16777215., true),
                ("Alpha 1", 0., 255., true),
                ("Color 2", 0., 16777215., true),
                ("Alpha 2", 0., 255., true),
            ],
            Self::Julia => &[
                ("Factor", 1., 10., false),
                ("Zoom", 0.1, 50., false),
                ("Angle", -180., 180., false),
                ("Quality", 1., 8., true),
                ("Blend Mode", 0., 45., true),
            ],
            Self::Mandelbrot => &[
                ("Factor", 1., 10., false),
                ("Zoom", 0., 100., false),
                ("Angle", -180., 180., false),
                ("Quality", 1., 8., true),
                ("Invert Colors", 0., 1., true),
                ("Blend Mode", 0., 45., true),
            ],
            Self::Turbulence => &[
                ("Octaves", 1., 15., true),
                ("Period", 0.1, 1024., false),
                ("Tile Size", 1., 4096., true),
                ("Noise", 0., 1., true),
                ("Seed", 0., 255., true),
                ("Blend Mode", 0., 45., true),
            ],
            Self::EdgeDetect => &[
                ("Strength", 0., 1., false),
                ("Blur Radius", 0., 10., false),
                ("Algorithm", 0., 1., true),
                ("Overlay Edges", 0., 1., true),
            ],
            _ => &[],
        }
    }
}

impl Gpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, info: wgpu::AdapterInfo) -> Self {
        let trace = std::env::var_os("OPENPDN_GPU_TRACE")
            .or_else(|| std::env::var_os("OPENDOTNET_GPU_TRACE"))
            .is_some();
        if trace {
            eprintln!(
                "GPU device: {} / {:?} / {:?}",
                info.name, info.backend, info.device_type
            );
        }
        let pipeline = |name: &'static str, code: &str| {
            if trace {
                eprintln!("Compiling GPU pipeline: {name}");
            }
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(name),
                source: wgpu::ShaderSource::Wgsl(code.into()),
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
            if trace {
                eprintln!("GPU pipeline ready: {name}");
            }
            pipeline
        };
        let compositor = pipeline("Layer composition", include_str!("shaders/composite.wgsl"));
        let adjustment = pipeline("Image adjustment", include_str!("shaders/adjust.wgsl"));
        let transfer = pipeline("Tonal transfer", include_str!("shaders/transfer.wgsl"));
        let resizer = pipeline("Image resize", include_str!("shaders/resize.wgsl"));
        let transformer = pipeline("Floating transform", include_str!("shaders/transform.wgsl"));
        let gaussian = pipeline("Optimized Gaussian", include_str!("shaders/gaussian.wgsl"));
        let filter_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Floating point filters"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}",
                    include_str!("shaders/effect_helpers.wgsl"),
                    include_str!("shaders/filter.wgsl")
                )
                .into(),
            ),
        });
        let entries: Vec<_> = (0..8)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 5 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: !matches!(binding, 3 | 4 | 7),
                        }
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        let filter_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Filter resources"),
            entries: &entries,
        });
        Self {
            device,
            queue,
            info,
            compositor,
            adjustment,
            transfer,
            resizer,
            transformer,
            gaussian,
            filter_module,
            filter_layout,
            filter_pipelines: Default::default(),
        }
    }
    fn filter_pipeline(&self, operation: u32) -> std::sync::Arc<wgpu::ComputePipeline> {
        let mut pipelines = self
            .filter_pipelines
            .lock()
            .expect("Filter pipeline cache poisoned");
        pipelines
            .entry(operation)
            .or_insert_with(|| {
                let layout = self
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("Filter pipeline"),
                        bind_group_layouts: &[Some(&self.filter_layout)],
                        immediate_size: 0,
                    });
                std::sync::Arc::new(self.device.create_compute_pipeline(
                    &wgpu::ComputePipelineDescriptor {
                        label: Some("Specialized filter"),
                        layout: Some(&layout),
                        module: &self.filter_module,
                        entry_point: Some("main"),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants: &[("operation", f64::from(operation))],
                            ..Default::default()
                        },
                        cache: None,
                    },
                ))
            })
            .clone()
    }
    pub async fn headless() -> Result<Self> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                ..Default::default()
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("OpenPDN"),
                ..Default::default()
            })
            .await?;
        Ok(Self::new(device, queue, adapter.get_info()))
    }
    fn buffer(&self, name: &'static str, data: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(name),
                contents: data,
                usage,
            })
    }
    fn output_buffer(&self, size: u64) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output pixels"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }
    pub fn check_document(&self, doc: &Document) -> Result<()> {
        doc.validate()?;
        let limits = self.device.limits();
        ensure!(
            doc.width <= limits.max_texture_dimension_2d
                && doc.height <= limits.max_texture_dimension_2d,
            "Image exceeds GPU texture dimensions"
        );
        let total = u64::from(doc.width) * u64::from(doc.height) * doc.layers.len() as u64 * 4;
        ensure!(
            total <= limits.max_storage_buffer_binding_size,
            "Image layers exceed this GPU's current storage limit ({} MiB). Tiled storage is pending.",
            limits.max_storage_buffer_binding_size / 1024 / 1024
        );
        ensure!(
            doc.width.div_ceil(16) <= limits.max_compute_workgroups_per_dimension
                && doc.height.div_ceil(16) <= limits.max_compute_workgroups_per_dimension,
            "Image exceeds GPU dispatch limits"
        );
        Ok(())
    }
    pub fn compose(&self, doc: &Document) -> Result<RenderedImage> {
        self.compose_display(doc, false)
    }
    pub fn compose_canvas(&self, doc: &Document) -> Result<RenderedImage> {
        self.compose_display(doc, true)
    }
    fn compose_display(&self, doc: &Document, selection_outline: bool) -> Result<RenderedImage> {
        self.check_document(doc)?;
        let packed: Vec<u32> = doc
            .layers
            .iter()
            .flat_map(|l| l.pixels.iter().map(|&p| u32::from_le_bytes(p)))
            .collect();
        let metadata: Vec<[u32; 4]> = doc
            .layers
            .iter()
            .map(|l| [l.opacity as u32, l.blend_mode as u32, l.visible as u32, 0])
            .collect();
        let source = self.buffer(
            "Layer pixels",
            bytemuck::cast_slice(&packed),
            wgpu::BufferUsages::STORAGE,
        );
        let layers = self.buffer(
            "Layer properties",
            bytemuck::cast_slice(&metadata),
            wgpu::BufferUsages::STORAGE,
        );
        let params = self.buffer(
            "Dimensions",
            bytemuck::cast_slice(&[
                doc.width,
                doc.height,
                doc.layers.len() as u32,
                u32::from(selection_outline && doc.selection.is_some()),
            ]),
            wgpu::BufferUsages::UNIFORM,
        );
        let mut selection = vec![
            0_u32;
            if selection_outline && doc.selection.is_some() {
                (doc.width * doc.height).div_ceil(32) as usize
            } else {
                1
            }
        ];
        if selection_outline && let Some(mask) = &doc.selection {
            for (i, &v) in mask.iter().enumerate() {
                if v != 0 {
                    selection[i / 32] |= 1 << (i % 32);
                }
            }
        }
        let selection_buffer = self.buffer(
            "Selection outline",
            bytemuck::cast_slice(&selection),
            wgpu::BufferUsages::STORAGE,
        );
        let output = self.output_buffer(u64::from(doc.width) * u64::from(doc.height) * 4);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Canvas"),
            size: wgpu::Extent3d {
                width: doc.width,
                height: doc.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Composition resources"),
            layout: &self.compositor.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: source.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: layers.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: selection_buffer.as_entire_binding(),
                },
            ],
        });
        self.dispatch(
            &self.compositor,
            &group,
            doc.width.div_ceil(16),
            doc.height.div_ceil(16),
        );
        Ok(RenderedImage {
            texture,
            pixels: output,
            width: doc.width,
            height: doc.height,
        })
    }
    fn dispatch(&self, pipeline: &wgpu::ComputePipeline, group: &wgpu::BindGroup, x: u32, y: u32) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Image compute"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(x, y, 1);
        }
        self.queue.submit([encoder.finish()]);
    }
    pub fn read_pixels(&self, buffer: &wgpu::Buffer) -> Result<Vec<Pixel>> {
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Pixel readback"),
            size: buffer.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, buffer.size());
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = mpsc::channel();
        staging.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })?;
        rx.recv_timeout(Duration::from_secs(10))
            .context("GPU readback timed out")??;
        let bytes = staging.get_mapped_range(..)?;
        let out = bytes
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect();
        drop(bytes);
        staging.unmap();
        Ok(out)
    }
    pub fn adjust(&self, doc: &Document, op: Adjustment) -> Result<Vec<Pixel>> {
        self.adjust_configured(doc, op, op.defaults())
    }
    pub fn transfer(
        &self,
        doc: &Document,
        table: &[[f32; 4]],
        luminosity: bool,
    ) -> Result<Vec<Pixel>> {
        self.check_document(doc)?;
        ensure!(
            (2..=4096).contains(&table.len()) && table.iter().flatten().all(|v| v.is_finite()),
            "Invalid transfer table"
        );
        let count = doc.width * doc.height;
        ensure!(
            count.div_ceil(256) <= self.device.limits().max_compute_workgroups_per_dimension,
            "Transfer exceeds GPU dispatch limits"
        );
        let source = self.buffer(
            "Transfer source",
            bytemuck::cast_slice(doc.layers[doc.active].pixels.as_slice()),
            wgpu::BufferUsages::STORAGE,
        );
        let mask: Vec<u32> = (0..count as usize)
            .map(|i| doc.selected(i) as u32)
            .collect();
        let mask = self.buffer(
            "Transfer selection",
            bytemuck::cast_slice(&mask),
            wgpu::BufferUsages::STORAGE,
        );
        let table_buffer = self.buffer(
            "Transfer table",
            bytemuck::cast_slice(table),
            wgpu::BufferUsages::STORAGE,
        );
        let params = self.buffer(
            "Transfer parameters",
            bytemuck::cast_slice(&[count, table.len() as u32, luminosity as u32, 0]),
            wgpu::BufferUsages::UNIFORM,
        );
        let output = self.output_buffer(count as u64 * 4);
        let buffers = [&source, &mask, &table_buffer, &params, &output];
        let entries: Vec<_> = buffers
            .iter()
            .enumerate()
            .map(|(i, b)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Transfer resources"),
            layout: &self.transfer.get_bind_group_layout(0),
            entries: &entries,
        });
        self.dispatch(&self.transfer, &group, count.div_ceil(256), 1);
        self.read_pixels(&output)
    }
    /// Sepia uses intensity 0..100; posterize uses four channel counts (0 disables a channel).
    pub fn adjust_configured(
        &self,
        doc: &Document,
        op: Adjustment,
        values: [f32; 4],
    ) -> Result<Vec<Pixel>> {
        let mut parameters = op.parameters();
        parameters[..4].copy_from_slice(&values);
        self.adjust_parameters(doc, op, parameters)
    }
    pub fn adjust_parameters(
        &self,
        doc: &Document,
        op: Adjustment,
        parameters: [f32; 8],
    ) -> Result<Vec<Pixel>> {
        self.check_document(doc)?;
        ensure!(
            parameters.iter().all(|v| v.is_finite()),
            "Non-finite image processing parameter"
        );
        for (i, &(_, min, max, integer)) in op.controls().iter().enumerate() {
            ensure!(
                (min..=max).contains(&parameters[i]) && (!integer || parameters[i].fract() == 0.),
                "Invalid {} parameter",
                op.name()
            );
        }
        if matches!(op, Adjustment::AddNoise | Adjustment::FrostedGlass) {
            ensure!(
                parameters[6..]
                    .iter()
                    .all(|v| (0. ..=65535.).contains(v) && v.fract() == 0.),
                "Invalid random stream seed"
            );
        }
        if matches!(op, Adjustment::Crystalize | Adjustment::Dents) {
            ensure!(
                (0. ..=255.).contains(&parameters[7]) && parameters[7].fract() == 0.,
                "Invalid instance seed"
            );
        }
        if op == Adjustment::FrostedGlass {
            ensure!(
                parameters[1] <= parameters[0],
                "Minimum scatter radius exceeds maximum"
            );
        }
        if op == Adjustment::TemperatureTint {
            if parameters[0] == 0. && parameters[1] == 0. {
                return Ok(doc.layers[doc.active].pixels.as_ref().clone());
            }
            return self.process(
                doc,
                16,
                0,
                temperature::gains(parameters[0], parameters[1]),
                [0.; 4],
            );
        }
        if op as u32 >= 17 {
            return self.filter_image(doc, op, parameters);
        }
        self.adjust_legacy(doc, op, parameters[..4].try_into().unwrap())
    }
    fn adjust_legacy(
        &self,
        doc: &Document,
        op: Adjustment,
        values: [f32; 4],
    ) -> Result<Vec<Pixel>> {
        self.check_document(doc)?;
        ensure!(
            values.iter().all(|v| v.is_finite()),
            "Non-finite image processing parameter"
        );
        match op {
            Adjustment::AutoLevel => {
                return self.transfer(doc, &crate::tonal::Levels::auto(doc).table()?, false);
            }
            Adjustment::Levels => {
                return self.transfer(doc, &crate::tonal::Levels::default().table()?, false);
            }
            Adjustment::Curves => {
                return self.transfer(doc, &crate::tonal::Curves::default().table()?, true);
            }
            _ => {}
        }
        if op == Adjustment::Sepia {
            ensure!(
                (0.0..=100.0).contains(&values[0]),
                "Sepia intensity must be 0..100"
            );
        }
        if op == Adjustment::Posterize {
            ensure!(
                values
                    .iter()
                    .all(|v| *v == 0. || ((2.0..=64.0).contains(v) && v.fract() == 0.)),
                "Posterize counts must be 2..64 or disabled"
            );
        }
        for (i, &(_, min, max, integer)) in op.controls().iter().enumerate() {
            ensure!(
                (min..=max).contains(&values[i]) && (!integer || values[i].fract() == 0.),
                "Invalid {} parameter",
                op.name()
            );
        }
        // Rectangular morphology is separable. Do not multiply the two kernel
        // dimensions per pixel; large supported values must remain usable.
        if op == Adjustment::Morphology && values[0] > 1. && values[1] > 1. {
            let mut intermediate = doc.clone();
            intermediate.selection = None;
            let horizontal = self.process(
                &intermediate,
                op as u32,
                0,
                [values[0], 1., values[2], 0.],
                [0.; 4],
            )?;
            intermediate.layers[doc.active].pixels = std::sync::Arc::new(horizontal);
            intermediate.selection = doc.selection.clone();
            let mut result = self.process(
                &intermediate,
                op as u32,
                0,
                [1., values[1], values[2], 0.],
                [0.; 4],
            )?;
            for (i, pixel) in result.iter_mut().enumerate() {
                if !doc.selected(i) {
                    *pixel = doc.layers[doc.active].pixels[i];
                }
            }
            return Ok(result);
        }
        let bounds = doc
            .selection_bounds()
            .unwrap_or([0, 0, doc.width, doc.height])
            .map(|v| v as f32);
        self.process(doc, op as u32, 0, values, bounds)
    }
    pub fn gradient(
        &self,
        doc: &Document,
        start: (i32, i32),
        end: (i32, i32),
        primary: Pixel,
        secondary: Pixel,
        options: crate::painting::GradientOptions,
    ) -> Result<Vec<Pixel>> {
        let (mut a, mut b) = (primary, secondary);
        if options.alpha_only {
            b[3] = 255 - b[3];
        }
        if options.reverse {
            std::mem::swap(&mut a, &mut b);
        }
        let flags = options.shape as u32
            | ((options.repeat as u32) << 3)
            | ((options.alpha_only as u32) << 5)
            | ((options.overwrite as u32) << 6)
            | ((options.blend_mode as u32) << 7);
        let dx = end.0 as f64 - start.0 as f64;
        let dy = end.1 as f64 - start.1 as f64;
        let square = dx * dx + dy * dy;
        let step = if square == 0. {
            [0., 0.]
        } else {
            [(dx / square) as f32, (dy / square) as f32]
        };
        self.process(
            doc,
            10,
            flags,
            [start.0 as f32, start.1 as f32, end.0 as f32, end.1 as f32],
            [
                f32::from_bits(u32::from_le_bytes(a)),
                f32::from_bits(u32::from_le_bytes(b)),
                step[0],
                step[1],
            ],
        )
    }
    fn process(
        &self,
        doc: &Document,
        operation: u32,
        flags: u32,
        values: [f32; 4],
        bounds: [f32; 4],
    ) -> Result<Vec<Pixel>> {
        self.check_document(doc)?;
        let count = doc.width * doc.height;
        ensure!(
            count.div_ceil(256) <= self.device.limits().max_compute_workgroups_per_dimension,
            "Adjustment exceeds GPU dispatch limits"
        );
        let pixels: Vec<u32> = doc.layers[doc.active]
            .pixels
            .iter()
            .map(|&p| u32::from_le_bytes(p))
            .collect();
        let selection: Vec<u32> = (0..count as usize)
            .map(|i| {
                if operation == 10 {
                    u32::from(doc.selection.as_ref().map_or(255, |m| m[i]))
                } else {
                    doc.selected(i) as u32
                }
            })
            .collect();
        let source = self.buffer(
            "Adjustment source",
            bytemuck::cast_slice(&pixels),
            wgpu::BufferUsages::STORAGE,
        );
        let mask = self.buffer(
            "Selection",
            bytemuck::cast_slice(&selection),
            wgpu::BufferUsages::STORAGE,
        );
        let params = self.buffer(
            "Adjustment parameters",
            bytemuck::cast_slice(&[
                doc.width,
                doc.height,
                operation,
                flags,
                values[0].to_bits(),
                values[1].to_bits(),
                values[2].to_bits(),
                values[3].to_bits(),
                bounds[0].to_bits(),
                bounds[1].to_bits(),
                bounds[2].to_bits(),
                bounds[3].to_bits(),
            ]),
            wgpu::BufferUsages::UNIFORM,
        );
        let output = self.output_buffer(u64::from(count) * 4);
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Adjustment resources"),
            layout: &self.adjustment.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: source.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: mask.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        self.dispatch(&self.adjustment, &group, count.div_ceil(256), 1);
        self.read_pixels(&output)
    }
}
