use crate::{
    brush::{BrushOptions, BrushPoint, BrushShape, BrushTip},
    document::{BlendMode, Document, Editor, Pixel},
    gpu::{Adjustment, Gpu, RenderedImage},
    painting::{GradientOptions, GradientRepeat, GradientShape},
    pattern::{FillSource, FillStyle, HatchStyle},
    picker::{PickerBehavior, SampleSize},
    project,
    resolution::ImageSize,
    selection::{AlphaMode, FloodMode, SelectionMode},
    selection_geometry::{SelectionDrawMode, SelectionOptions},
    tool_blend::ToolBlend,
};
use anyhow::Result;
use eframe::{
    egui::{self, Key, Modifiers, Sense, Vec2},
    egui_wgpu::wgpu,
};
use std::{path::PathBuf, sync::Arc};
mod brush;
mod color_wheel;
mod colors;
mod content;
mod editing;
mod effects;
mod input;
mod picker;
mod selection;
mod text;
mod tonal;
mod transform;
mod workspace;
use effects::EffectDialog;
use workspace::{LayerProperties, Workspace};
fn pointer_press(ctx: &egui::Context) -> Option<(egui::Pos2, egui::PointerButton)> {
    ctx.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::PointerButton {
                pos,
                button,
                pressed: true,
                ..
            } if matches!(
                button,
                egui::PointerButton::Primary | egui::PointerButton::Secondary
            ) =>
            {
                Some((*pos, *button))
            }
            _ => None,
        })
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Brush,
    Pencil,
    Eraser,
    Bucket,
    Picker,
    RectangleSelect,
    EllipseSelect,
    LassoSelect,
    MagicWand,
    MovePixels,
    MoveSelection,
    Zoom,
    Line,
    Pan,
    Gradient,
    CloneStamp,
    Recolor,
    Text,
    Shapes,
}
impl Tool {
    const ALL: [Self; 19] = [
        Self::RectangleSelect,
        Self::MovePixels,
        Self::LassoSelect,
        Self::MoveSelection,
        Self::EllipseSelect,
        Self::Zoom,
        Self::MagicWand,
        Self::Pan,
        Self::Bucket,
        Self::Gradient,
        Self::Brush,
        Self::Eraser,
        Self::Pencil,
        Self::Picker,
        Self::CloneStamp,
        Self::Recolor,
        Self::Text,
        Self::Line,
        Self::Shapes,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Brush => "Paintbrush",
            Self::Pencil => "Pencil",
            Self::Eraser => "Eraser",
            Self::Bucket => "Paint Bucket",
            Self::Picker => "Color Picker",
            Self::RectangleSelect => "Rectangle Select",
            Self::EllipseSelect => "Ellipse Select",
            Self::LassoSelect => "Lasso Select",
            Self::MagicWand => "Magic Wand",
            Self::MovePixels => "Move Selected Pixels",
            Self::MoveSelection => "Move Selection",
            Self::Zoom => "Zoom",
            Self::Line => "Line / Curve",
            Self::Pan => "Pan",
            Self::Gradient => "Gradient",
            Self::CloneStamp => "Clone Stamp",
            Self::Recolor => "Recolor",
            Self::Text => "Text",
            Self::Shapes => "Shapes",
        }
    }
}
struct Tab {
    editor: Editor,
    path: Option<PathBuf>,
    name: String,
    thumbnail: Option<egui::TextureHandle>,
    thumbnail_revision: u64,
}
struct CloneSample {
    pixels: Arc<Vec<Pixel>>,
    offset: (i32, i32),
}
struct Stroke {
    before: Document,
    start: (i32, i32),
    last: (i32, i32),
    color: Pixel,
    tool: Tool,
    points: Vec<(i32, i32)>,
    brush_points: Vec<BrushPoint>,
    brush_tip: Option<BrushTip>,
    selection_mode: SelectionMode,
    selection_options: SelectionOptions,
    origin_drag: Option<(i32, i32)>,
    began: std::time::Instant,
    basis: Pixel,
    clone_source: Option<CloneSample>,
    secondary: bool,
    drawing_modifiers: (bool, bool),
}
#[derive(Clone, Copy)]
enum CloseAction {
    Tab(usize),
    Exit,
}

pub struct PaintApp {
    gpu: Arc<Gpu>,
    processing: crate::processing::LatestWorker<Vec<Pixel>>,
    render_state: eframe::egui_wgpu::RenderState,
    rendered: Option<RenderedImage>,
    texture: Option<egui::TextureId>,
    tabs: Vec<Tab>,
    active: usize,
    tool: Tool,
    previous_tool: Tool,
    input: input::ToolInput,
    picker_size: SampleSize,
    picker_behavior: PickerBehavior,
    picker_press: Option<egui::PointerButton>,
    primary: Pixel,
    secondary: Pixel,
    palette: Vec<Pixel>,
    palette_add: bool,
    palette_library: Option<crate::palette::Library>,
    named_palettes: Vec<crate::palette::NamedPalette>,
    palette_menu_open: bool,
    palette_save: Option<colors::SavePaletteDialog>,
    color_hex: String,
    color_hex_basis: Pixel,
    color_hsv: [u16; 3],
    color_hsv_basis: Pixel,
    brush_size: f32,
    brush_options: BrushOptions,
    brush_pressure: f64,
    tolerance: u8,
    flood_mode: FloodMode,
    alpha_mode: AlphaMode,
    sample_composite: bool,
    selection_mode: SelectionMode,
    selection_options: SelectionOptions,
    dropped_files: Option<Vec<PathBuf>>,
    zoom: f32,
    refresh: bool,
    stroke: Option<Stroke>,
    error: Option<String>,
    new_dialog: bool,
    new_size: ImageSize,
    transparent: bool,
    pending_close: Option<CloseAction>,
    allow_exit: bool,
    frame_count: u32,
    smoke_frames: Option<u32>,
    workspace: Workspace,
    layer_properties: Option<LayerProperties>,
    effect: Option<EffectDialog>,
    last_effect: Option<(Adjustment, [f32; 8])>,
    random_effect_seeds: [u32; 2],
    gradient_options: GradientOptions,
    tool_blend: ToolBlend,
    fill_style: FillStyle,
    clone_anchor: Option<((i32, i32), usize)>,
    clone_offset: Option<((i32, i32), usize)>,
    recolor_sample_start: bool,
    content: Option<content::LiveContent>,
    text_options: crate::content::TextOptions,
    shape_options: crate::content::ShapeOptions,
    line_options: crate::line::LineOptions,
    text_renderer: crate::content::TextRenderer,
    size_dialog: Option<editing::SizeDialog>,
    paste_dialog: Option<editing::PasteDialog>,
    clipboard: Option<arboard::Clipboard>,
    copied_image: Option<crate::editing::ClipboardImage>,
    transform: Option<transform::LiveTransform>,
    transform_bilinear: bool,
    transform_linear: bool,
}
impl PaintApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        initial: Option<PathBuf>,
        smoke_frames: Option<u32>,
    ) -> Result<Self> {
        let state = cc
            .wgpu_render_state
            .clone()
            .ok_or_else(|| anyhow::anyhow!("A wgpu renderer is required"))?;
        let gpu = Arc::new(Gpu::new(
            state.device.clone(),
            state.queue.clone(),
            state.adapter.get_info(),
        ));
        let repaint = cc.egui_ctx.clone();
        let processing = crate::processing::LatestWorker::new(move || repaint.request_repaint())?;
        eprintln!(
            "OpenPDN renderer: {} ({:?}, {:?})",
            gpu.info.name, gpu.info.backend, gpu.info.device_type
        );
        let document = Document::new(800, 600, [255; 4])?;
        workspace::configure_style(&cc.egui_ctx);
        let mut app = Self {
            size_dialog: None,
            paste_dialog: None,
            clipboard: None,
            copied_image: None,
            transform: None,
            transform_bilinear: false,
            transform_linear: true,
            gpu,
            processing,
            render_state: state,
            rendered: None,
            texture: None,
            tabs: vec![Tab {
                editor: Editor::new(document),
                path: None,
                name: "Untitled".into(),
                thumbnail: None,
                thumbnail_revision: u64::MAX,
            }],
            active: 0,
            tool: Tool::Brush,
            previous_tool: Tool::Brush,
            picker_size: SampleSize::default(),
            picker_behavior: PickerBehavior::default(),
            picker_press: None,
            primary: [0, 0, 0, 255],
            secondary: [255; 4],
            palette: crate::palette::DEFAULT.to_vec(),
            palette_add: false,
            palette_library: colors::default_library(),
            named_palettes: Vec::new(),
            palette_menu_open: false,
            palette_save: None,
            color_hex: String::from("000000"),
            color_hex_basis: [0, 0, 0, 255],
            color_hsv: [0; 3],
            color_hsv_basis: [0, 0, 0, 255],
            brush_size: 2.0,
            brush_options: BrushOptions::default(),
            brush_pressure: 1.,
            tolerance: 0,
            flood_mode: FloodMode::Contiguous,
            alpha_mode: AlphaMode::Premultiplied,
            sample_composite: false,
            selection_mode: SelectionMode::Replace,
            selection_options: SelectionOptions::default(),
            dropped_files: None,
            zoom: 1.0,
            refresh: true,
            stroke: None,
            error: None,
            new_dialog: false,
            new_size: ImageSize::new([800, 600]),
            transparent: false,
            pending_close: None,
            allow_exit: false,
            frame_count: 0,
            smoke_frames,
            workspace: Workspace::default(),
            input: input::ToolInput::default(),
            layer_properties: None,
            effect: None,
            last_effect: None,
            random_effect_seeds: {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                [
                    stamp as u32,
                    (stamp >> 32) as u32 ^ (stamp as u32).rotate_left(13),
                ]
            },
            gradient_options: GradientOptions::default(),
            tool_blend: ToolBlend::default(),
            fill_style: FillStyle::Solid,
            clone_anchor: None,
            clone_offset: None,
            recolor_sample_start: false,
            content: None,
            text_options: Default::default(),
            shape_options: Default::default(),
            line_options: Default::default(),
            text_renderer: Default::default(),
        };
        app.restore_preferences(cc.storage);
        if std::env::var_os("OPENPDN_FLOAT_UTILITIES")
            .or_else(|| std::env::var_os("OPENDOTNET_FLOAT_UTILITIES"))
            .is_some_and(|v| v == "1")
        {
            app.workspace.native = [true; 4];
            app.workspace.visible = [true; 4];
        }
        if let Some(path) = initial {
            app.open(path);
            if app.tabs.len() > 1 {
                app.tabs.remove(0);
                app.active = 0;
            }
        }
        Ok(app)
    }
    fn editor(&self) -> &Editor {
        &self.tabs[self.active].editor
    }
    fn editor_mut(&mut self) -> &mut Editor {
        &mut self.tabs[self.active].editor
    }
    fn edit(&mut self, label: &str, f: impl FnOnce(&mut Document)) {
        self.finish_stroke();
        let before = self.editor().document.clone();
        f(&mut self.editor_mut().document);
        if let Err(error) = self.gpu.check_document(&self.editor().document) {
            self.editor_mut().document = before;
            self.error = Some(format!("{error:#}"));
            return;
        }
        self.editor_mut().record(label, before);
        self.refresh = true;
    }
    fn finish_stroke(&mut self) {
        self.finish_transform();
        self.finish_content();
        if let Some(stroke) = self.stroke.take() {
            if matches!(
                stroke.tool,
                Tool::RectangleSelect | Tool::EllipseSelect | Tool::LassoSelect
            ) {
                self.finish_selection(&stroke);
            }
            self.editor_mut().record(stroke.tool.name(), stroke.before);
            self.refresh = true;
        }
    }
    fn open(&mut self, path: PathBuf) {
        self.finish_stroke();
        match project::open(&path).and_then(|doc| {
            self.gpu.check_document(&doc)?;
            Ok(doc)
        }) {
            Ok(doc) => {
                self.finish_stroke();
                let native = path.extension().and_then(|s| s.to_str()).is_some_and(|e| {
                    e.eq_ignore_ascii_case("opdn") || e.eq_ignore_ascii_case("pdn")
                });
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.tabs.push(Tab {
                    editor: Editor::new(doc),
                    path: native.then_some(path),
                    name,
                    thumbnail: None,
                    thumbnail_revision: u64::MAX,
                });
                self.active = self.tabs.len() - 1;
                self.clone_anchor = None;
                self.clone_offset = None;
                self.refresh = true;
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }
    fn open_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter(
                "Images and projects",
                &[
                    "pdn", "opdn", "png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp",
                    "ico", "qoi",
                ],
            )
            .pick_files()
        {
            for path in paths {
                self.open(path);
            }
        }
    }
    fn save(&mut self, save_as: bool) -> bool {
        self.finish_stroke();
        let path = if save_as || self.tabs[self.active].path.is_none() {
            rfd::FileDialog::new()
                .add_filter("Paint.NET document", &["pdn"])
                .add_filter("OpenPDN project", &["opdn"])
                .set_file_name("Untitled.pdn")
                .save_file()
                .map(|mut p| {
                    if p.extension().is_none() {
                        p.set_extension("pdn");
                    }
                    p
                })
        } else {
            self.tabs[self.active].path.clone()
        };
        let Some(path) = path else {
            return false;
        };
        if !path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("opdn") || e.eq_ignore_ascii_case("pdn"))
        {
            self.error = Some(
                "Save projects with the .pdn or .opdn extension. Use Export image for raster files.".into(),
            );
            return false;
        }
        match project::save_project(&self.editor().document, &path) {
            Ok(()) => {
                let tab = &mut self.tabs[self.active];
                tab.name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                tab.path = Some(path);
                tab.editor.mark_saved();
                true
            }
            Err(e) => {
                self.error = Some(format!("Save failed: {e:#}"));
                false
            }
        }
    }
    fn export(&mut self) {
        self.finish_stroke();
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .add_filter("JPEG", &["jpg"])
            .add_filter("BMP", &["bmp"])
            .add_filter("TIFF", &["tiff"])
            .add_filter("WebP", &["webp"])
            .set_file_name("Untitled.png")
            .save_file()
        {
            let result = (|| -> Result<()> {
                let doc = &self.editor().document;
                let rendered = self.gpu.compose(doc)?;
                project::export(
                    &path,
                    doc.width,
                    doc.height,
                    &self.gpu.read_pixels(&rendered.pixels)?,
                )
            })();
            if let Err(e) = result {
                self.error = Some(format!("Export failed: {e:#}"));
            }
        }
    }
    fn adjustment(&mut self, op: Adjustment) {
        self.finish_stroke();
        match self.gpu.adjust(&self.editor().document, op) {
            Ok(pixels) => self.edit(op.name(), |d| d.layers[d.active].pixels = Arc::new(pixels)),
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }
    fn request_close(&mut self, action: CloseAction, ctx: &egui::Context) {
        self.cancel_effect();
        self.finish_stroke();
        let dirty = match action {
            CloseAction::Tab(i) => self.tabs[i].editor.dirty(),
            CloseAction::Exit => self.tabs.iter().any(|t| t.editor.dirty()),
        };
        if dirty {
            self.pending_close = Some(action);
        } else {
            self.close(action, ctx);
        }
    }
    fn close(&mut self, action: CloseAction, ctx: &egui::Context) {
        self.pending_close = None;
        match action {
            CloseAction::Exit => {
                self.allow_exit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            CloseAction::Tab(i) => {
                self.tabs.remove(i);
                if self.active > i {
                    self.active -= 1;
                }
                if self.tabs.is_empty() {
                    self.tabs.push(Tab {
                        editor: Editor::new(Document::new(800, 600, [255; 4]).unwrap()),
                        path: None,
                        name: "Untitled".into(),
                        thumbnail: None,
                        thumbnail_revision: u64::MAX,
                    });
                }
                self.active = self.active.min(self.tabs.len() - 1);
                self.refresh = true;
            }
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.transform_shortcuts(ctx) {
            return;
        }
        if self.content.is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                if self.tool == Tool::Text {
                    if !self.cancel_text_composition() {
                        self.finish_content();
                    }
                } else {
                    self.cancel_content();
                }
                return;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter)) {
                self.finish_content();
                return;
            }
            if self.tool == Tool::Line
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
            {
                self.finish_content();
                return;
            }
            if self.tool == Tool::Text {
                if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
                    self.save(false);
                }
                return;
            }
        }
        self.workspace_shortcuts(ctx);
        self.editing_shortcuts(ctx);
        let command = |key| egui::KeyboardShortcut::new(Modifiers::COMMAND, key);
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::F))) {
            self.repeat_effect();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::O))) {
            self.open_dialog();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::N))) {
            self.open_new_dialog();
        }
        if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::S,
            ))
        }) {
            self.save(true);
        } else if ctx.input_mut(|i| i.consume_shortcut(&command(Key::S))) {
            self.save(false);
        }
        if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::Z,
            ))
        }) {
            self.finish_stroke();
            self.editor_mut().redo();
            self.refresh = true;
        } else if ctx.input_mut(|i| i.consume_shortcut(&command(Key::Z))) {
            self.undo_action();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::Y))) {
            self.finish_stroke();
            self.editor_mut().redo();
            self.refresh = true;
        }
        if !ctx.egui_wants_keyboard_input() {
            if self.content.is_none()
                && !ctx.input(|i| i.pointer.any_down())
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
            {
                self.edit("Deselect", |d| d.selection = None);
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete)) {
                self.edit("Erase Selection", Document::erase_selection);
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                if let Some(stroke) = self.stroke.take() {
                    self.editor_mut().document = stroke.before;
                    self.refresh = true;
                } else {
                    self.edit("Deselect", |d| d.selection = None);
                }
            }
        }
    }
    fn menu(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New…        Ctrl+N").clicked() {
                    self.open_new_dialog();
                    ui.close();
                }
                if ui.button("Open…       Ctrl+O").clicked() {
                    self.open_dialog();
                    ui.close();
                }
                if ui.button("Save project   Ctrl+S").clicked() {
                    self.save(false);
                    ui.close();
                }
                if ui.button("Save project as…").clicked() {
                    self.save(true);
                    ui.close();
                }
                if ui.button("Export image…").clicked() {
                    self.export();
                    ui.close();
                }
                ui.separator();
                if ui.button("Close image").clicked() {
                    self.request_close(CloseAction::Tab(self.active), &ctx);
                    ui.close();
                }
                if ui.button("Exit").clicked() {
                    self.request_close(CloseAction::Exit, &ctx);
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(
                        self.editor().can_undo() || self.transform.is_some(),
                        egui::Button::new("Undo   Ctrl+Z"),
                    )
                    .clicked()
                {
                    self.undo_action();
                    ui.close();
                }
                if ui
                    .add_enabled(self.editor().can_redo(), egui::Button::new("Redo   Ctrl+Y"))
                    .clicked()
                {
                    self.finish_stroke();
                    self.editor_mut().redo();
                    self.refresh = true;
                    ui.close();
                }
                ui.separator();
                for (label, action) in [
                    ("Cut   Ctrl+X", 0),
                    ("Copy   Ctrl+C", 1),
                    ("Copy Merged   Ctrl+Shift+C", 2),
                    ("Paste   Ctrl+V", 3),
                    ("Paste into New Layer   Ctrl+Shift+V", 4),
                    ("Paste into New Image   Ctrl+Alt+V", 5),
                ] {
                    if ui.button(label).clicked() {
                        match action {
                            0 => self.copy_image(true, false),
                            1 => self.copy_image(false, false),
                            2 => self.copy_image(false, true),
                            3 => self.paste(false, false),
                            4 => self.paste(true, false),
                            _ => self.paste(false, true),
                        }
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Select all").clicked() {
                    self.edit("Select All", |d| {
                        d.selection = Some(Arc::new(vec![255; (d.width * d.height) as usize]))
                    });
                    ui.close();
                }
                if ui.button("Deselect").clicked() {
                    self.edit("Deselect", |d| d.selection = None);
                    ui.close();
                }
                if ui.button("Invert selection").clicked() {
                    self.edit("Invert Selection", Document::invert_selection);
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.editor().document.selection.is_some(),
                        egui::Button::new("Erase selection"),
                    )
                    .clicked()
                {
                    self.edit("Erase Selection", Document::erase_selection);
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Actual size").clicked() {
                    self.zoom = 1.0;
                    ui.close();
                }
                if ui.button("Zoom in").clicked() {
                    self.zoom = (self.zoom * 1.25).min(16.0);
                    ui.close();
                }
                if ui.button("Zoom out").clicked() {
                    self.zoom = (self.zoom / 1.25).max(0.05);
                    ui.close();
                }
                ui.separator();
                ui.checkbox(&mut self.workspace.grid, "Pixel Grid");
                ui.checkbox(&mut self.workspace.rulers, "Rulers");
                ui.separator();
                ui.menu_button("Floating Tool Windows", |ui| {
                    for (i, name) in ["Tools", "History", "Layers", "Colors"]
                        .into_iter()
                        .enumerate()
                    {
                        ui.checkbox(&mut self.workspace.native[i], name);
                    }
                    ui.separator();
                    if workspace::window_action(
                        ui,
                        "Float All Outside Main Window",
                        crate::icons::Icon::FloatWindow,
                    )
                    .clicked()
                    {
                        self.workspace.native = [true; 4];
                        self.workspace.visible = [true; 4];
                        ui.close();
                    }
                    if workspace::window_action(
                        ui,
                        "Dock All in Main Window",
                        crate::icons::Icon::DockWindow,
                    )
                    .clicked()
                    {
                        self.workspace.native = [false; 4];
                        ui.close();
                    }
                });
            });
            ui.menu_button("Image", |ui| {
                if ui
                    .add_enabled(
                        self.editor().document.selection.is_some()
                            && self.editor().document.selection_bounds().is_some(),
                        egui::Button::new("Crop to Selection   Ctrl+Shift+X"),
                    )
                    .clicked()
                {
                    self.crop();
                    ui.close();
                }
                if ui.button("Resize…   Ctrl+R").clicked() {
                    self.open_size_dialog(false);
                    ui.close();
                }
                if ui.button("Canvas Size…   Ctrl+Shift+R").clicked() {
                    self.open_size_dialog(true);
                    ui.close();
                }
                ui.separator();
                if ui.button("Flip horizontal").clicked() {
                    self.edit("Flip Horizontal", |d| d.flip(true));
                    ui.close();
                }
                if ui.button("Flip vertical").clicked() {
                    self.edit("Flip Vertical", |d| d.flip(false));
                    ui.close();
                }
                if ui.button("Rotate 90° clockwise").clicked() {
                    self.edit("Rotate 90°", Document::rotate_cw);
                    ui.close();
                }
                if ui.button("Rotate 90° counterclockwise   Ctrl+G").clicked() {
                    self.edit("Rotate 90° Counterclockwise", Document::rotate_ccw);
                    ui.close();
                }
                if ui.button("Rotate 180°").clicked() {
                    self.edit("Rotate 180°", Document::rotate_180);
                    ui.close();
                }
                if ui.button("Flatten").clicked() {
                    self.edit("Flatten", Document::flatten);
                    ui.close();
                }
            });
            ui.menu_button("Layers", |ui| {
                if ui.button("Add layer").clicked() {
                    self.edit("Add Layer", Document::add_layer);
                    ui.close();
                }
                if ui.button("Duplicate layer").clicked() {
                    self.edit("Duplicate Layer", Document::duplicate_layer);
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.editor().document.layers.len() > 1,
                        egui::Button::new("Delete layer"),
                    )
                    .clicked()
                {
                    self.edit("Delete Layer", Document::delete_layer);
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.editor().document.active > 0,
                        egui::Button::new("Merge Layer Down   Ctrl+M"),
                    )
                    .clicked()
                {
                    self.merge_down();
                    ui.close();
                }
            });
            ui.menu_button("Adjustments", |ui| self.adjustments_menu(ui));
            ui.menu_button("Effects", |ui| self.effects_menu(ui));
        });
    }
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let editing = self.pending_close.is_none()
            && self.palette_save.is_none()
            && self.size_dialog.is_none()
            && self.paste_dialog.is_none()
            && !self.new_dialog
            && self.error.is_none()
            && self.layer_properties.is_none()
            && self.effect.is_none()
            && self.dropped_files.is_none();
        if !editing {
            self.finish_stroke();
        }
        if self.refresh {
            match self.gpu.compose_canvas(&self.editor().document) {
                Ok(rendered) => {
                    let view = rendered.texture.create_view(&Default::default());
                    let mut renderer = self.render_state.renderer.write();
                    if let Some(id) = self.texture.take() {
                        renderer.free_texture(&id);
                    }
                    self.texture = Some(renderer.register_native_texture(
                        &self.gpu.device,
                        &view,
                        wgpu::FilterMode::Nearest,
                    ));
                    self.rendered = Some(rendered);
                    self.workspace.render_generation =
                        self.workspace.render_generation.wrapping_add(1);
                }
                Err(e) => self.error = Some(format!("{e:#}")),
            }
            self.refresh = false;
        }
        let Some(texture) = self.texture else {
            return;
        };
        let doc = &self.editor().document;
        let size = Vec2::new(doc.width as f32, doc.height as f32) * self.zoom;
        let canvas_hovered = self
            .workspace
            .canvas
            .is_some_and(|rect| ui.rect_contains_pointer(rect.intersect(ui.clip_rect())));
        let pan_delta =
            self.input
                .pan_canvas(ui.ctx(), editing, canvas_hovered, self.stroke.is_some());
        let temporary_pan = self.input.temporary_pan();
        let panning = temporary_pan || self.tool == Tool::Pan;
        let editing = editing && !temporary_pan;
        let mut scroll = egui::ScrollArea::both()
            .id_salt("document-canvas")
            .auto_shrink([false, false])
            .scroll_source(egui::scroll_area::ScrollSource {
                scroll_bar: editing,
                mouse_wheel: editing,
                drag: if editing && self.tool == Tool::Pan && !temporary_pan {
                    egui::scroll_area::DragScroll::Always
                } else {
                    egui::scroll_area::DragScroll::Never
                },
            });
        if let Some(delta) = pan_delta {
            let offset = egui::scroll_area::State::load(
                ui.ctx(),
                ui.make_persistent_id(egui::IdSalt::new("document-canvas")),
            )
            .unwrap_or_default()
            .offset
                - delta;
            scroll = scroll.scroll_offset(offset);
        }
        scroll.show(ui, |ui| {
            let sense = if !editing || panning {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            };
            let (area, _) = ui.allocate_exact_size(size.max(ui.available_size()), Sense::hover());
            let rect = egui::Rect::from_min_size(
                area.min + (area.size() - size).max(Vec2::ZERO) * 0.5,
                size,
            );
            let response = ui.interact(rect, ui.id().with("canvas_image"), sense);
            if panning && (response.hovered() || temporary_pan) {
                ui.ctx()
                    .set_cursor_icon(if ui.input(|input| input.pointer.any_down()) {
                        egui::CursorIcon::Grabbing
                    } else {
                        egui::CursorIcon::Grab
                    });
            }
            ui.painter().rect_stroke(
                rect.expand(1.0),
                0.0,
                (1.0, egui::Color32::GRAY),
                egui::StrokeKind::Outside,
            );
            ui.painter().image(
                texture,
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            self.workspace.canvas = Some(rect);
            self.canvas_overlays(ui, rect);
            let point = ui.ctx().input(|i| i.pointer.interact_pos()).map(|p| {
                (
                    ((p.x - response.rect.min.x) / self.zoom).floor() as i32,
                    ((p.y - response.rect.min.y) / self.zoom).floor() as i32,
                )
            });
            if let Some(force) = ui.input(|i| {
                i.events.iter().rev().find_map(|e| match e {
                    egui::Event::Touch {
                        force: Some(f),
                        phase: egui::TouchPhase::Start | egui::TouchPhase::Move,
                        ..
                    } if f.is_finite() => Some(*f),
                    _ => None,
                })
            }) {
                self.brush_pressure = f64::from(force.clamp(0., 1.));
            } else if ui.input(|i| i.pointer.any_pressed()) {
                self.brush_pressure = 1.;
            }
            let brush_point = ui.input(|i| i.pointer.interact_pos()).map(|p| {
                BrushPoint::new(
                    f64::from((p.x - rect.min.x) / self.zoom),
                    f64::from((p.y - rect.min.y) / self.zoom),
                    self.brush_pressure,
                )
            });
            self.workspace.cursor = if response.hovered() { point } else { None };
            let content_capture = self.content_handles(ui, rect, editing)
                | self.transform_handles(ui, rect, editing && response.contains_pointer());
            if self.tool == Tool::CloneStamp {
                let source = self.clone_anchor.map(|(p, _)| p).or_else(|| {
                    self.clone_offset.zip(point).map(|((offset, _), p)| {
                        (p.0.saturating_add(offset.0), p.1.saturating_add(offset.1))
                    })
                });
                if let Some(source) = source {
                    let center = rect.min
                        + Vec2::new(source.0 as f32 + 0.5, source.1 as f32 + 0.5) * self.zoom;
                    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
                    painter.circle_stroke(
                        center,
                        (self.brush_size * self.zoom / 2.).max(4.),
                        (1.5, egui::Color32::from_rgb(20, 135, 235)),
                    );
                    painter.line_segment(
                        [center - Vec2::new(5., 0.), center + Vec2::new(5., 0.)],
                        (1., egui::Color32::BLACK),
                    );
                    painter.line_segment(
                        [center - Vec2::new(0., 5.), center + Vec2::new(0., 5.)],
                        (1., egui::Color32::BLACK),
                    );
                }
            }
            if let Some(stroke) = &self.stroke
                && stroke.tool == Tool::Gradient
            {
                let a =
                    rect.min + Vec2::new(stroke.start.0 as f32, stroke.start.1 as f32) * self.zoom;
                let b =
                    rect.min + Vec2::new(stroke.last.0 as f32, stroke.last.1 as f32) * self.zoom;
                let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
                painter.line_segment([a, b], (1.5, egui::Color32::from_rgb(20, 135, 235)));
                for p in [a, b] {
                    painter.circle_filled(p, 4., egui::Color32::WHITE);
                    painter.circle_stroke(p, 4., (1., egui::Color32::BLACK));
                }
            }
            if editing
                && !content_capture
                && self.tool != Tool::Pan
                && response.hovered()
                && ui.ctx().input(|i| i.pointer.any_pressed())
            {
                if let Some((press, button)) = pointer_press(ui.ctx()) {
                    let point = (
                        ((press.x - rect.min.x) / self.zoom).floor() as i32,
                        ((press.y - rect.min.y) / self.zoom).floor() as i32,
                    );
                    // The canvas owns editing shortcuts after a pointer press.
                    // egui otherwise keeps a toolbar/control focus and eats Delete/tool keys.
                    ui.ctx().memory_mut(|memory| {
                        if let Some(id) = memory.focused() {
                            memory.surrender_focus(id);
                        }
                    });
                    if let Some(stroke) = &mut self.stroke
                        && matches!(
                            stroke.tool,
                            Tool::RectangleSelect | Tool::EllipseSelect | Tool::LassoSelect
                        )
                    {
                        stroke.origin_drag = Some(point);
                        stroke.last = point;
                        return;
                    }
                    let secondary = button == egui::PointerButton::Secondary;
                    let color = if secondary {
                        self.secondary
                    } else {
                        self.primary
                    };
                    if self.tool == Tool::Text {
                        self.begin_text(point, secondary);
                        return;
                    }
                    self.finish_content();
                    let before = self.editor().document.clone();
                    if self.tool == Tool::CloneStamp
                        && ui.input(|i| i.modifiers.ctrl || i.modifiers.command)
                    {
                        if point.0 >= 0
                            && point.1 >= 0
                            && point.0 < before.width as i32
                            && point.1 < before.height as i32
                        {
                            self.clone_anchor = Some((point, before.active));
                            self.clone_offset = None;
                        }
                    } else if self.tool == Tool::CloneStamp
                        && self.clone_anchor.is_none()
                        && self.clone_offset.is_none()
                    {
                        // A source must be chosen before the first clone stroke.
                    } else if self.tool == Tool::Zoom {
                        self.finish_stroke();
                        self.zoom =
                            (self.zoom * if secondary { 0.5 } else { 2.0 }).clamp(0.1, 16.0);
                    } else if self.tool == Tool::Picker {
                        self.picker_press = Some(button);
                        self.pick_color(
                            point,
                            secondary,
                            ui.input(|i| i.modifiers.ctrl || i.modifiers.command),
                        );
                    } else {
                        let clone_source = if self.tool == Tool::CloneStamp {
                            if let Some((source, layer)) = self.clone_anchor.take() {
                                self.clone_offset = Some((
                                    (
                                        source.0.saturating_sub(point.0),
                                        source.1.saturating_sub(point.1),
                                    ),
                                    layer,
                                ));
                            }
                            self.clone_offset.map(|(offset, layer)| CloneSample {
                                pixels: before
                                    .layers
                                    .get(layer)
                                    .unwrap_or(&before.layers[before.active])
                                    .pixels
                                    .clone(),
                                offset,
                            })
                        } else {
                            None
                        };
                        let basis = if self.recolor_sample_start {
                            if point.0 >= 0
                                && point.1 >= 0
                                && point.0 < before.width as i32
                                && point.1 < before.height as i32
                            {
                                before.layers[before.active].pixels
                                    [(point.1 as u32 * before.width + point.0 as u32) as usize]
                            } else {
                                [0; 4]
                            }
                        } else if secondary {
                            self.primary
                        } else {
                            self.secondary
                        };
                        self.stroke = Some(Stroke {
                            before,
                            start: point,
                            last: point,
                            color,
                            tool: self.tool,
                            points: vec![point],
                            brush_points: Vec::new(),
                            brush_tip: matches!(
                                self.tool,
                                Tool::Brush | Tool::Eraser | Tool::CloneStamp | Tool::Recolor
                            )
                            .then(|| BrushTip::new(f64::from(self.brush_size), self.brush_options)),
                            selection_mode: ui.input(|i| {
                                self.selection_mode.from_pointer(
                                    secondary,
                                    i.modifiers.ctrl || i.modifiers.command,
                                    i.modifiers.alt,
                                )
                            }),
                            selection_options: self.selection_options,
                            origin_drag: None,
                            began: std::time::Instant::now(),
                            basis,
                            clone_source,
                            secondary,
                            drawing_modifiers: (false, false),
                        });
                        self.continue_stroke(
                            point,
                            ui.input(|i| (i.modifiers.shift, i.modifiers.alt)),
                            Some(BrushPoint::new(
                                f64::from((press.x - rect.min.x) / self.zoom),
                                f64::from((press.y - rect.min.y) / self.zoom),
                                self.brush_pressure,
                            )),
                        );
                    }
                }
            } else if self.stroke.is_some()
                && ui.ctx().input(|i| i.pointer.any_down())
                && let Some(point) = point
                && self.stroke.as_ref().is_some_and(|s| {
                    s.last != point
                        || s.brush_tip.is_some() && s.brush_points.last().copied() != brush_point
                        || matches!(
                            s.tool,
                            Tool::Line | Tool::Shapes | Tool::RectangleSelect | Tool::EllipseSelect
                        ) && s.drawing_modifiers
                            != ui.input(|i| (i.modifiers.shift, i.modifiers.alt))
                })
            {
                self.continue_stroke(
                    point,
                    ui.input(|i| (i.modifiers.shift, i.modifiers.alt)),
                    brush_point,
                );
            }
            if ui.input(|i| i.pointer.any_released() && i.pointer.any_down())
                && let Some(stroke) = &mut self.stroke
            {
                stroke.origin_drag = None;
            }
            self.picker_canvas(
                ui,
                rect,
                editing && !content_capture,
                point,
                response.hovered(),
            );
            if self.stroke.is_some() && !ui.ctx().input(|i| i.pointer.any_down()) {
                // Motion and release can arrive in the same native frame.
                // Include the final captured point before recording the transaction.
                if let Some(point) = point
                    && self.stroke.as_ref().is_some_and(|s| {
                        s.last != point
                            || s.brush_tip.is_some()
                                && s.brush_points.last().copied() != brush_point
                    })
                {
                    self.continue_stroke(
                        point,
                        ui.input(|i| (i.modifiers.shift, i.modifiers.alt)),
                        brush_point,
                    );
                }
                if self
                    .stroke
                    .as_ref()
                    .is_some_and(|s| matches!(s.tool, Tool::Shapes | Tool::Line))
                {
                    self.content = self
                        .stroke
                        .take()
                        .map(|s| content::LiveContent::shape(s, self.shape_options.kind));
                } else {
                    self.finish_stroke();
                }
            }
        });
    }
    fn continue_stroke(
        &mut self,
        point: (i32, i32),
        modifiers: (bool, bool),
        brush_point: Option<BrushPoint>,
    ) {
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        stroke.drawing_modifiers = modifiers;
        let cursor = point;
        let point = if let Some(origin) = stroke.origin_drag {
            let delta = (
                point.0.saturating_sub(origin.0),
                point.1.saturating_sub(origin.1),
            );
            for p in &mut stroke.points {
                p.0 = p.0.saturating_add(delta.0);
                p.1 = p.1.saturating_add(delta.1);
            }
            stroke.start = stroke.points[0];
            stroke.origin_drag = Some(point);
            *stroke.points.last().unwrap()
        } else {
            if stroke.points.last() != Some(&point) {
                stroke.points.push(point);
            }
            point
        };
        if stroke.brush_tip.is_some()
            && let Some(p) = brush_point
            && stroke.brush_points.last() != Some(&p)
        {
            stroke.brush_points.push(p);
        }
        if stroke.tool == Tool::Gradient {
            let mut options = self.gradient_options;
            options.reverse ^= stroke.secondary;
            match self.gpu.gradient(
                &stroke.before,
                stroke.start,
                point,
                self.primary,
                self.secondary,
                options,
            ) {
                Ok(pixels) => {
                    self.editor_mut().document = stroke.before.clone();
                    self.editor_mut().document.layers[stroke.before.active].pixels =
                        Arc::new(pixels);
                }
                Err(e) => {
                    self.error = Some(format!("Gradient failed: {e:#}"));
                }
            }
            stroke.last = point;
            self.stroke = Some(stroke);
            self.refresh = true;
            return;
        }
        let size = if stroke.tool == Tool::Pencil {
            1.0
        } else {
            self.brush_size
        };
        let tolerance = self.tolerance;
        let flood = self.flood_mode;
        let alpha = self.alpha_mode;
        let composite = self.sample_composite;
        let blender = self.tool_blend;
        let fill_style = self.fill_style;
        let shape_options = self.shape_options.clone();
        let line_options = self.line_options.clone();
        let (primary, secondary) = if stroke.secondary {
            (self.secondary, self.primary)
        } else {
            (self.primary, self.secondary)
        };
        let fill = FillSource {
            style: if stroke.tool == Tool::Pencil {
                FillStyle::Solid
            } else {
                fill_style
            },
            foreground: primary,
            background: secondary,
        };
        let doc = &mut self.editor_mut().document;
        match stroke.tool {
            Tool::Brush | Tool::Pencil | Tool::Eraser => {
                *doc = stroke.before.clone();
                if let Some(tip) = &stroke.brush_tip {
                    doc.paint_sampled_stroke(
                        &stroke.brush_points,
                        tip,
                        fill,
                        stroke.tool == Tool::Eraser,
                        blender,
                    );
                } else {
                    doc.paint_stroke(
                        &stroke.points,
                        size,
                        fill,
                        stroke.tool == Tool::Eraser,
                        blender,
                    );
                }
            }
            Tool::Bucket => {
                *doc = stroke.before.clone();
                doc.fill_blended(
                    stroke.start,
                    fill,
                    tolerance,
                    flood,
                    alpha,
                    composite,
                    blender,
                );
            }
            Tool::RectangleSelect | Tool::EllipseSelect => {
                *doc = stroke.before.clone();
                let mask = doc.selection_shape_mask(
                    stroke.start,
                    point,
                    stroke.tool == Tool::EllipseSelect,
                    stroke.selection_options,
                    modifiers.0,
                );
                doc.combine_selection(mask, stroke.selection_mode);
            }
            Tool::LassoSelect => {
                *doc = stroke.before.clone();
                doc.select_polygon(&stroke.points, stroke.selection_mode);
            }
            Tool::MagicWand => {
                *doc = stroke.before.clone();
                doc.magic_wand(
                    stroke.start,
                    tolerance,
                    flood,
                    alpha,
                    composite,
                    stroke.selection_mode,
                );
            }
            Tool::MovePixels | Tool::MoveSelection => {
                *doc = stroke.before.clone();
                doc.translate_selection(
                    point.0.saturating_sub(stroke.start.0),
                    point.1.saturating_sub(stroke.start.1),
                    stroke.tool == Tool::MovePixels,
                );
            }
            Tool::Line => {
                *doc = stroke.before.clone();
                if let Err(e) = doc.draw_curve(
                    crate::line::initial_points_with_modifiers(
                        stroke.start,
                        point,
                        modifiers.0,
                        modifiers.1,
                    ),
                    fill,
                    &line_options,
                ) {
                    self.error = Some(format!("Line failed: {e:#}"));
                }
            }
            Tool::Shapes => {
                *doc = stroke.before.clone();
                let (start, end) = crate::content::shape_drawing_points(
                    stroke.start,
                    point,
                    shape_options.kind,
                    modifiers.0,
                    modifiers.1,
                );
                if let Err(e) = doc.draw_shape_at(start, end, primary, secondary, &shape_options) {
                    self.error = Some(format!("Shape failed: {e:#}"));
                }
            }
            Tool::CloneStamp => {
                *doc = stroke.before.clone();
                if let (Some(source), Some(tip)) = (&stroke.clone_source, &stroke.brush_tip) {
                    doc.clone_sampled_stroke(
                        &source.pixels,
                        source.offset,
                        &stroke.brush_points,
                        tip,
                        stroke.color[3],
                        blender,
                    );
                }
            }
            Tool::Recolor => {
                *doc = stroke.before.clone();
                if let Some(tip) = &stroke.brush_tip {
                    doc.recolor_sampled_stroke(
                        &stroke.brush_points,
                        tip,
                        stroke.basis,
                        stroke.color,
                        tolerance,
                        alpha,
                    );
                }
            }
            _ => {}
        }
        self.stroke = Some(Stroke {
            last: cursor,
            ..stroke
        });
        self.refresh = true;
    }
    fn drop_dialog(&mut self, ctx: &egui::Context) {
        if self.new_dialog
            || self.palette_save.is_some()
            || self.size_dialog.is_some()
            || self.paste_dialog.is_some()
            || self.pending_close.is_some()
            || self.error.is_some()
            || self.layer_properties.is_some()
            || self.effect.is_some()
        {
            return;
        }
        let Some(paths) = self.dropped_files.clone() else {
            return;
        };
        egui::Modal::new(egui::Id::new("drop_images")).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(32., 32.), Sense::hover());
                crate::icons::paint(ui.painter(), rect, crate::icons::Icon::DragDrop, true);
                ui.heading("Dropped images");
            });
            ui.label(format!("{} image file(s)", paths.len()));
            if ui.button("Open as new images").clicked() {
                self.dropped_files = None;
                for path in &paths {
                    self.open(path.clone());
                }
            }
            if ui.button("Add as layers").clicked() {
                let imports: Result<Vec<_>> = paths
                    .iter()
                    .map(|p| project::open(p).map(|d| (p, d)))
                    .collect();
                let result = imports.and_then(|imports| {
                    let mut candidate = self.editor().document.clone();
                    for (path, imported) in imports {
                        anyhow::ensure!(
                            (candidate.width, candidate.height)
                                == (imported.width, imported.height),
                            "Imported layers currently require matching canvas dimensions"
                        );
                        candidate.add_layer();
                        let layer = &mut candidate.layers[candidate.active];
                        layer.name = path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        layer.pixels = Arc::new(imported.composite());
                    }
                    self.gpu.check_document(&candidate)?;
                    Ok(candidate)
                });
                self.dropped_files = None;
                match result {
                    Ok(candidate) => self.edit("Import Layers", |d| *d = candidate),
                    Err(e) => self.error = Some(format!("{e:#}")),
                }
            }
            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                self.dropped_files = None;
            }
        });
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.new_dialog {
            egui::Modal::new(egui::Id::new("new_image")).show(ctx, |ui| {
                ui.heading("New");
                editing::image_size_fields(ui, &mut self.new_size);
                let [width, height] = self.new_size.pixels();
                let valid = crate::document::pixel_count(width, height).is_ok();
                if !valid {
                    ui.colored_label(
                        egui::Color32::DARK_RED,
                        "Image exceeds the supported pixel count.",
                    );
                }
                ui.checkbox(&mut self.transparent, "Transparent background");
                ui.horizontal(|ui| {
                    if ui.add_enabled(valid, egui::Button::new("OK")).clicked()
                        || valid && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
                    {
                        let color = if self.transparent { [0; 4] } else { [255; 4] };
                        match Document::new(width, height, color).and_then(|mut d| {
                            d.set_resolution(self.new_size.document_resolution())?;
                            self.gpu.check_document(&d)?;
                            Ok(d)
                        }) {
                            Ok(document) => {
                                self.finish_stroke();
                                self.tabs.push(Tab {
                                    editor: Editor::new(document),
                                    path: None,
                                    name: "Untitled".into(),
                                    thumbnail: None,
                                    thumbnail_revision: u64::MAX,
                                });
                                self.active = self.tabs.len() - 1;
                                self.clone_anchor = None;
                                self.clone_offset = None;
                                self.refresh = true;
                                self.new_dialog = false;
                            }
                            Err(e) => self.error = Some(format!("{e:#}")),
                        }
                    }
                    if ui.button("Cancel").clicked()
                        || ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
                    {
                        self.new_dialog = false;
                    }
                });
            });
        }
        if let Some(action) = self.pending_close {
            egui::Modal::new(egui::Id::new("unsaved_changes")).show(ctx, |ui| {
                ui.heading("Unsaved changes");
                ui.label("Save your changes before closing?");
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        let original = self.active;
                        let indices: Vec<usize> = match action {
                            CloseAction::Tab(i) => vec![i],
                            CloseAction::Exit => self
                                .tabs
                                .iter()
                                .enumerate()
                                .filter_map(|(i, t)| t.editor.dirty().then_some(i))
                                .collect(),
                        };
                        let mut success = true;
                        for i in indices {
                            self.active = i;
                            if !self.save(false) {
                                success = false;
                                break;
                            }
                        }
                        self.active = original;
                        if success {
                            self.close(action, ctx);
                        }
                    }
                    if ui.button("Discard").clicked() {
                        self.close(action, ctx);
                    }
                    if ui.button("Cancel").clicked()
                        || ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
                    {
                        self.pending_close = None;
                    }
                });
            });
        }
        if let Some(message) = self.error.clone() {
            egui::Modal::new(egui::Id::new("error")).show(ctx, |ui| {
                ui.heading("OpenPDN");
                ui.label(message);
                if ui.button("OK").clicked()
                    || ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
                {
                    self.error = None;
                }
            });
        }
    }
}
impl eframe::App for PaintApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.save_preferences(storage);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_effect();
        let content_colors = (self.primary, self.secondary);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_close(CloseAction::Exit, &ctx);
        }
        if self.pending_close.is_none()
            && self.palette_save.is_none()
            && self.size_dialog.is_none()
            && self.paste_dialog.is_none()
            && !self.new_dialog
            && self.error.is_none()
            && self.layer_properties.is_none()
            && self.dropped_files.is_none()
            && self.effect.is_none()
        {
            self.shortcuts(&ctx);
        }
        let dropped: Vec<_> = ctx
            .input_mut(|i| std::mem::take(&mut i.raw.dropped_files))
            .into_iter()
            .map(|f| f.path().to_path_buf())
            .collect();
        if !dropped.is_empty() {
            self.finish_stroke();
            self.dropped_files
                .get_or_insert_with(Vec::new)
                .extend(dropped);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}{} — OpenPDN",
            if self.editor().dirty() { "*" } else { "" },
            self.tabs[self.active].name
        )));
        egui::Panel::top("menus").show(ui, |ui| self.workspace_header(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.workspace_status(ui));
        let central = egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_gray(207))
                    .inner_margin(if self.workspace.rulers {
                        egui::Margin {
                            left: 18,
                            top: 18,
                            right: 0,
                            bottom: 0,
                        }
                    } else {
                        egui::Margin::ZERO
                    }),
            )
            .show(ui, |ui| self.canvas(ui));
        self.draw_rulers(&ctx, central.response.rect);
        self.utility_windows(&ctx, central.response.rect);
        if content_colors != (self.primary, self.secondary) {
            self.preview_content();
        }
        self.size_dialog(&ctx);
        self.paste_dialog(&ctx);
        self.layer_properties_dialog(&ctx);
        self.effect_dialog(&ctx);
        self.save_palette_dialog(&ctx);
        self.dialogs(&ctx);
        self.drop_dialog(&ctx);
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("file_drop_overlay"),
            ));
            let rect = ctx.content_rect();
            painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(105));
            crate::icons::paint(
                &painter,
                egui::Rect::from_center_size(
                    rect.center() - egui::vec2(0., 25.),
                    egui::vec2(64., 64.),
                ),
                crate::icons::Icon::DragDrop,
                true,
            );
            painter.text(
                rect.center() + egui::vec2(0., 35.),
                egui::Align2::CENTER_CENTER,
                "Drop images to open or import as layers",
                egui::FontId::proportional(18.),
                egui::Color32::WHITE,
            );
        }
        if let Some(frames) = self.smoke_frames {
            self.frame_count += 1;
            ctx.request_repaint();
            if self.frame_count >= frames {
                self.allow_exit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.refresh {
            ctx.request_repaint();
        }
    }
}
