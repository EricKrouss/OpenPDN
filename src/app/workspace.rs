//! Workspace structure recovered from 5.1.12 MainForm, PdnToolBar and utility forms.
//! Icons are independent vector drawings; the reference payload is not shipped.
use super::*;
use crate::document::{Layer, TRANSPARENT, blend};
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke as UiStroke, pos2, vec2};

pub(super) struct Workspace {
    pub visible: [bool; 4],
    pub native: [bool; 4],
    native_positions: [Option<[f32; 2]>; 4],
    native_sizes: [Option<[f32; 2]>; 4],
    native_initial_position: [Option<Pos2>; 4],
    native_initial_size: [Option<Vec2>; 4],
    reset: [bool; 4],
    generation: [u64; 4],
    layer_thumbnails: Vec<(u64, egui::TextureHandle)>,
    pub render_generation: u64,
    restore: Option<[bool; 4]>,
    pub grid: bool,
    pub rulers: bool,
    pub(super) more_colors: bool,
    pub(super) secondary_selected: bool,
    pub canvas: Option<Rect>,
    pub cursor: Option<(i32, i32)>,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            visible: [true; 4],
            native: [false; 4],
            native_positions: [None; 4],
            native_sizes: [None; 4],
            native_initial_position: [None; 4],
            native_initial_size: [None; 4],
            reset: [false; 4],
            generation: [0; 4],
            layer_thumbnails: Vec::new(),
            render_generation: 0,
            restore: None,
            grid: false,
            rulers: false,
            more_colors: false,
            secondary_selected: false,
            canvas: None,
            cursor: None,
        }
    }
}
pub(super) struct LayerProperties {
    index: usize,
    name: String,
    visible: bool,
    opacity: u8,
    mode: BlendMode,
}

pub(super) fn configure_style(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.style_mut_of(egui::Theme::Light, |style| {
        style.spacing.item_spacing = vec2(4.0, 4.0);
        style.spacing.button_padding = vec2(5.0, 3.0);
        style.spacing.window_margin = egui::Margin::same(5);
        style.visuals.window_corner_radius = egui::CornerRadius::ZERO;
        style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
        style.visuals.window_fill = Color32::from_rgb(248, 248, 248);
        style.visuals.panel_fill = Color32::from_gray(247);
        style.visuals.selection.bg_fill = Color32::from_rgb(185, 218, 242);
        style.visuals.selection.stroke = UiStroke::new(1.0, Color32::from_rgb(35, 102, 157));
        for text in [egui::TextStyle::Body, egui::TextStyle::Button] {
            style.text_styles.insert(text, FontId::proportional(12.0));
        }
        style
            .text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(13.0));
    });
}

pub(super) fn icon(
    ui: &mut egui::Ui,
    name: &str,
    tip: &str,
    selected: bool,
    enabled: bool,
) -> egui::Response {
    let response = ui
        .add_enabled(
            enabled,
            egui::Button::new("")
                .selected(selected)
                .min_size(vec2(24.0, 24.0)),
        )
        .on_hover_text(tip);
    let rect = response.rect.shrink(4.0);
    let glyph =
        crate::icons::Icon::named(name).expect("Every UI feature has hand-crafted icon artwork");
    crate::icons::paint(ui.painter(), rect, glyph, enabled);
    response
}

pub(super) fn window_action(
    ui: &mut egui::Ui,
    label: &str,
    glyph: crate::icons::Icon,
) -> egui::Response {
    let response = ui.button(format!("     {label}"));
    let rect = Rect::from_center_size(response.rect.left_center() + vec2(11., 0.), vec2(16., 16.));
    crate::icons::paint(ui.painter(), rect, glyph, true);
    response
}

const TOOL_SLOTS: [(&str, &str, Option<Tool>); 19] = [
    ("rect", "Rectangle Select (S)", Some(Tool::RectangleSelect)),
    ("move", "Move Selected Pixels (M)", Some(Tool::MovePixels)),
    ("lasso", "Lasso Select (S)", Some(Tool::LassoSelect)),
    (
        "move-selection",
        "Move Selection (M)",
        Some(Tool::MoveSelection),
    ),
    ("ellipse", "Ellipse Select (S)", Some(Tool::EllipseSelect)),
    ("zoom", "Zoom (Z)", Some(Tool::Zoom)),
    ("wand", "Magic Wand (S)", Some(Tool::MagicWand)),
    ("pan", "Pan (H)", Some(Tool::Pan)),
    ("bucket", "Paint Bucket (F)", Some(Tool::Bucket)),
    ("gradient", "Gradient (G)", Some(Tool::Gradient)),
    ("brush", "Paintbrush (B)", Some(Tool::Brush)),
    ("eraser", "Eraser (E)", Some(Tool::Eraser)),
    ("pencil", "Pencil (P)", Some(Tool::Pencil)),
    ("picker", "Color Picker (K)", Some(Tool::Picker)),
    ("stamp", "Clone Stamp (L)", Some(Tool::CloneStamp)),
    ("recolor", "Recolor (R)", Some(Tool::Recolor)),
    ("text", "Text (T)", Some(Tool::Text)),
    ("line", "Line / Curve (O)", Some(Tool::Line)),
    ("shapes", "Shapes (O)", Some(Tool::Shapes)),
];
const WINDOW_NAMES: [&str; 4] = ["Tools", "History", "Layers", "Colors"];
const WINDOW_ICONS: [&str; 4] = ["tools", "history", "layers", "colors"];

impl PaintApp {
    pub(super) fn restore_preferences(&mut self, storage: Option<&dyn eframe::Storage>) {
        let Some(value) = storage.and_then(|s| s.get_string("workspace-v1")) else {
            return;
        };
        let Ok(prefs) = serde_json::from_str::<Preferences>(&value) else {
            return;
        };
        self.workspace.visible = prefs.visible;
        self.workspace.native = prefs.native;
        self.workspace.native_positions = prefs
            .native_positions
            .map(|p| p.filter(|p| p.iter().all(|v| v.is_finite())));
        self.workspace.native_sizes = prefs
            .native_sizes
            .map(|p| p.filter(|p| p.iter().all(|v| v.is_finite() && *v >= 50. && *v <= 8192.)));
        self.workspace.generation = prefs.generation;
        self.workspace.grid = prefs.grid;
        self.workspace.rulers = prefs.rulers;
        self.workspace.more_colors = prefs.more_colors;
        self.palette = prefs
            .palette
            .into_iter()
            .take(crate::palette::COLOR_COUNT)
            .collect();
        self.primary = prefs.primary;
        self.secondary = prefs.secondary;
        self.tool = Tool::ALL.get(prefs.tool).copied().unwrap_or(Tool::Brush);
        if prefs.brush_size.is_finite() {
            self.set_brush_width(prefs.brush_size);
        }
        self.brush_options = prefs.brush_options.sanitized();
        self.text_options = prefs.text_options;
        self.text_options.size = self.text_options.size.clamp(1., 2000.);
        self.selection_mode = prefs.selection_mode;
        self.selection_options = prefs.selection_options.sanitized();
        self.tolerance = prefs.tolerance;
        self.picker_size = prefs.picker_size;
        self.picker_behavior = prefs.picker_behavior;
        self.sample_composite = prefs.sample_composite;
        self.transform_bilinear = prefs.transform_bilinear;
        self.transform_linear = prefs.transform_linear;
        self.tool_blend = prefs.tool_blend;
        self.fill_style = prefs.fill_style;
        self.line_options.fill_style = self.fill_style;
        self.shape_options.fill_style = self.fill_style;
        self.sync_tool_blend();
    }
    pub(super) fn save_preferences(&self, storage: &mut dyn eframe::Storage) {
        let prefs = Preferences {
            visible: self.workspace.restore.unwrap_or(self.workspace.visible),
            native: self.workspace.native,
            native_positions: self.workspace.native_positions,
            native_sizes: self.workspace.native_sizes,
            generation: self.workspace.generation,
            grid: self.workspace.grid,
            rulers: self.workspace.rulers,
            more_colors: self.workspace.more_colors,
            primary: self.primary,
            palette: self.palette.clone(),
            secondary: self.secondary,
            tool: Tool::ALL.iter().position(|t| *t == self.tool).unwrap_or(10),
            brush_size: self.brush_size,
            brush_options: self.brush_options,
            selection_mode: self.selection_mode,
            selection_options: self.selection_options,
            text_options: self.text_options.clone(),
            tolerance: self.tolerance,
            picker_size: self.picker_size,
            picker_behavior: self.picker_behavior,
            sample_composite: self.sample_composite,
            tool_blend: self.tool_blend,
            fill_style: self.fill_style,
            transform_bilinear: self.transform_bilinear,
            transform_linear: self.transform_linear,
        };
        if let Ok(value) = serde_json::to_string(&prefs) {
            storage.set_string("workspace-v1", value);
        }
    }
    pub(super) fn workspace_shortcuts(&mut self, ctx: &egui::Context) {
        let layer_shortcut =
            |key| egui::KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, key);
        if ctx.input_mut(|i| i.consume_shortcut(&layer_shortcut(Key::N))) {
            self.edit("Add New Layer", Document::add_layer);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&layer_shortcut(Key::D))) {
            self.edit("Duplicate Layer", Document::duplicate_layer);
        }
        if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::D))
        }) {
            self.edit("Deselect", |d| d.selection = None);
        }
        for (i, key) in [Key::F5, Key::F6, Key::F7, Key::F8].into_iter().enumerate() {
            // A newly positioned window can cause another egui layout pass.
            // Consume each key so a reset is applied only once across those passes.
            if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, key))
            {
                self.workspace.reset[i] = true;
                self.workspace.visible[i] = true;
            } else if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, key)) {
                self.workspace.visible[i] = !self.workspace.visible[i];
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
            ctx.memory_mut(|memory| {
                if let Some(id) = memory.focused() {
                    memory.surrender_focus(id);
                }
            });
            if let Some(visible) = self.workspace.restore.take() {
                self.workspace.visible = visible;
            } else {
                self.workspace.restore = Some(self.workspace.visible);
                self.workspace.visible = [false; 4];
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F4)) {
            self.open_layer_properties();
        }
        self.tool_input_shortcuts(ctx);
    }
    pub(super) fn workspace_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_min_width(340.0);
                ui.set_max_width(370.0);
                self.menu(ui);
                self.common_actions(ui);
            });
            self.document_strip(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                for i in (0..4).rev() {
                    if icon(
                        ui,
                        WINDOW_ICONS[i],
                        &format!("{} (F{})", WINDOW_NAMES[i], i + 5),
                        self.workspace.visible[i],
                        true,
                    )
                    .clicked()
                    {
                        if ui.input(|v| v.modifiers.command && v.modifiers.shift) {
                            self.workspace.reset[i] = true;
                            self.workspace.visible[i] = true;
                        } else {
                            self.workspace.visible[i] = !self.workspace.visible[i];
                        }
                    }
                }
            });
        });
        ui.separator();
        egui::ScrollArea::horizontal()
            .id_salt("tool_options_scroll")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Tool changes must not move the image or floating window origins.
                    ui.set_min_height(24.0);
                    ui.label("Tool:");
                    let previous = self.tool;
                    egui::ComboBox::from_id_salt("tool_chooser")
                        .width(145.0)
                        .selected_text(self.tool.name())
                        .show_ui(ui, |ui| {
                            for tool in Tool::ALL {
                                ui.selectable_value(&mut self.tool, tool, tool.name());
                            }
                        });
                    if self.tool != previous {
                        self.finish_stroke();
                        self.previous_tool = previous;
                        self.picker_press = None;
                    }
                    ui.separator();
                    self.content_options(ui);
                    self.picker_options(ui);
                    self.transform_options(ui);
                    if matches!(
                        self.tool,
                        Tool::Brush | Tool::Eraser | Tool::CloneStamp | Tool::Recolor
                    ) {
                        ui.label("Brush width:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.brush_size)
                                    .range(1.0..=2000.0)
                                    .speed(1.0),
                            )
                            .changed()
                        {
                            self.set_brush_width(self.brush_size);
                        }
                    }
                    if matches!(
                        self.tool,
                        Tool::RectangleSelect
                            | Tool::EllipseSelect
                            | Tool::LassoSelect
                            | Tool::MagicWand
                    ) {
                        egui::ComboBox::from_id_salt("selection_mode")
                            .selected_text(self.selection_mode.name())
                            .show_ui(ui, |ui| {
                                for mode in SelectionMode::ALL {
                                    ui.selectable_value(
                                        &mut self.selection_mode,
                                        mode,
                                        mode.name(),
                                    );
                                }
                            });
                    }
                    self.selection_options_ui(ui);
                    if self.tool == Tool::Gradient {
                        egui::ComboBox::from_id_salt("gradient_shape")
                            .selected_text(self.gradient_options.shape.name())
                            .show_ui(ui, |ui| {
                                for shape in GradientShape::ALL {
                                    ui.selectable_value(
                                        &mut self.gradient_options.shape,
                                        shape,
                                        shape.name(),
                                    );
                                }
                            });
                        egui::ComboBox::from_id_salt("gradient_repeat")
                            .selected_text(self.gradient_options.repeat.name())
                            .show_ui(ui, |ui| {
                                for repeat in GradientRepeat::ALL {
                                    ui.selectable_value(
                                        &mut self.gradient_options.repeat,
                                        repeat,
                                        repeat.name(),
                                    );
                                }
                            });
                        ui.checkbox(&mut self.gradient_options.alpha_only, "Transparency");
                        ui.checkbox(&mut self.gradient_options.reverse, "Reverse");
                    }
                    if self.tool == Tool::CloneStamp {
                        ui.label("Ctrl-click source; drag to clone");
                    }
                    if matches!(self.tool, Tool::Bucket | Tool::MagicWand | Tool::Recolor) {
                        ui.label("Tolerance:");
                        let mut percent = self.tolerance as f32 / 255.0 * 100.0;
                        if ui
                            .add(egui::Slider::new(&mut percent, 0.0..=100.0).suffix("%"))
                            .changed()
                        {
                            self.tolerance = (percent / 100.0 * 255.0).round() as u8;
                        }
                        if self.tool != Tool::Recolor {
                            egui::ComboBox::from_id_salt("flood_mode")
                                .selected_text(if self.flood_mode == FloodMode::Contiguous {
                                    "Contiguous"
                                } else {
                                    "Global"
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.flood_mode,
                                        FloodMode::Contiguous,
                                        "Contiguous",
                                    );
                                    ui.selectable_value(
                                        &mut self.flood_mode,
                                        FloodMode::Global,
                                        "Global",
                                    );
                                });
                        }
                        egui::ComboBox::from_id_salt("alpha_mode")
                            .selected_text(if self.alpha_mode == AlphaMode::Premultiplied {
                                "Premultiplied alpha"
                            } else {
                                "Straight alpha"
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.alpha_mode,
                                    AlphaMode::Premultiplied,
                                    "Premultiplied alpha",
                                );
                                ui.selectable_value(
                                    &mut self.alpha_mode,
                                    AlphaMode::Straight,
                                    "Straight alpha",
                                );
                            });
                        if self.tool == Tool::Recolor {
                            ui.checkbox(&mut self.recolor_sample_start, "Sample stroke start");
                        } else {
                            ui.checkbox(&mut self.sample_composite, "Sample all layers");
                        }
                    }
                    self.tool_blend_options(ui);
                    self.pattern_options(ui);
                    self.brush_controls(ui);
                });
            });
    }
    fn common_actions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            if icon(ui, "new", "New (Ctrl+N)", false, true).clicked() {
                self.open_new_dialog();
            }
            if icon(ui, "open", "Open (Ctrl+O)", false, true).clicked() {
                self.open_dialog();
            }
            if icon(ui, "save", "Save (Ctrl+S)", false, true).clicked() {
                self.save(false);
            }
            icon(ui, "print", "Print", false, false);
            ui.separator();
            for (action, glyph, tip) in [
                (0, "cut", "Cut (Ctrl+X)"),
                (1, "copy", "Copy (Ctrl+C)"),
                (2, "paste", "Paste (Ctrl+V)"),
                (3, "crop", "Crop to Selection (Ctrl+Shift+X)"),
            ] {
                let enabled = action != 3
                    || self.editor().document.selection.is_some()
                        && self.editor().document.selection_bounds().is_some();
                if icon(ui, glyph, tip, false, enabled).clicked() {
                    match action {
                        0 => self.copy_image(true, false),
                        1 => self.copy_image(false, false),
                        2 => self.paste(false, false),
                        _ => self.crop(),
                    }
                }
            }
            if icon(
                ui,
                "deselect",
                "Deselect (Ctrl+D)",
                false,
                self.editor().document.selection.is_some(),
            )
            .clicked()
            {
                self.edit("Deselect", |d| d.selection = None);
            }
            ui.separator();
            if icon(
                ui,
                "undo",
                "Undo (Ctrl+Z)",
                false,
                self.editor().can_undo() || self.transform.is_some(),
            )
            .clicked()
            {
                self.undo_action();
            }
            if icon(ui, "redo", "Redo (Ctrl+Y)", false, self.editor().can_redo()).clicked() {
                self.finish_stroke();
                self.editor_mut().redo();
                self.refresh = true;
            }
            ui.separator();
            if icon(ui, "grid", "Pixel Grid", self.workspace.grid, true).clicked() {
                self.workspace.grid = !self.workspace.grid;
            }
            if icon(ui, "rulers", "Rulers", self.workspace.rulers, true).clicked() {
                self.workspace.rulers = !self.workspace.rulers;
            }
        });
    }
    fn document_strip(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let width = (ui.available_width() - 120.0).clamp(70.0, 480.0);
        let mut switch = None;
        let mut close = None;
        let mut reorder = None;
        let tab_count = self.tabs.len();
        egui::ScrollArea::horizontal()
            .id_salt("document_strip")
            .max_width(width)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, tab) in self.tabs.iter_mut().enumerate() {
                        if tab.thumbnail_revision != tab.editor.revision || tab.thumbnail.is_none()
                        {
                            let image = thumbnail(&tab.editor.document, None);
                            if let Some(texture) = &mut tab.thumbnail {
                                texture.set(image, egui::TextureOptions::LINEAR);
                            } else {
                                tab.thumbnail = Some(ctx.load_texture(
                                    format!("document-{i}"),
                                    image,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                            tab.thumbnail_revision = tab.editor.revision;
                        }
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(68.0, 58.0), Sense::click_and_drag());
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            if i == self.active {
                                Color32::from_rgb(194, 222, 244)
                            } else {
                                Color32::from_gray(235)
                            },
                        );
                        if let Some(texture) = &tab.thumbnail {
                            let size = texture.size_vec2();
                            let scale = (52.0 / size.x).min(44.0 / size.y);
                            let image_rect = Rect::from_center_size(rect.center(), size * scale);
                            ui.painter().image(
                                texture.id(),
                                image_rect,
                                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                        if i == self.active {
                            ui.painter().rect_stroke(
                                rect,
                                0.0,
                                (1.0, Color32::from_rgb(64, 145, 210)),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if tab.editor.dirty() {
                            ui.painter().text(
                                rect.left_top() + vec2(2.0, 0.0),
                                Align2::LEFT_TOP,
                                "*",
                                FontId::proportional(17.0),
                                Color32::BLACK,
                            );
                        }
                        let close_rect = Rect::from_min_size(
                            rect.right_top() - vec2(15.0, 0.0),
                            vec2(15.0, 15.0),
                        );
                        if response.hovered() {
                            ui.painter().text(
                                close_rect.center(),
                                Align2::CENTER_CENTER,
                                "×",
                                FontId::proportional(14.0),
                                Color32::BLACK,
                            );
                        }
                        if response.clicked() {
                            if response
                                .interact_pointer_pos()
                                .is_some_and(|p| close_rect.contains(p))
                            {
                                close = Some(i);
                            } else {
                                switch = Some(i);
                            }
                        }
                        if response.drag_stopped()
                            && let Some(position) = response.interact_pointer_pos()
                        {
                            let offset = ((position.x - rect.center().x) / 72.0).round() as isize;
                            let target =
                                (i as isize + offset).clamp(0, tab_count as isize - 1) as usize;
                            reorder = Some((i, target));
                        }
                        response.on_hover_text(&tab.name);
                    }
                });
            });
        if let Some(i) = switch {
            self.finish_stroke();
            self.active = i;
            self.clone_anchor = None;
            self.clone_offset = None;
            self.refresh = true;
        }
        if let Some(i) = close {
            self.request_close(CloseAction::Tab(i), &ctx);
        }
        if let Some((from, to)) = reorder {
            self.finish_stroke();
            let tab = self.tabs.remove(from);
            self.tabs.insert(to, tab);
            self.active = if self.active == from {
                to
            } else if from < self.active && to >= self.active {
                self.active - 1
            } else if from > self.active && to <= self.active {
                self.active + 1
            } else {
                self.active
            };
        }
    }
    pub(super) fn utility_windows(&mut self, ctx: &egui::Context, editing: Rect) {
        let positions = [
            editing.left_top() + vec2(7.0, 7.0),
            editing.right_top() + vec2(-219.0, 7.0),
            editing.right_bottom() - vec2(219.0, 286.0),
            editing.left_bottom() + vec2(7.0, -282.0),
        ];
        let sizes = [
            vec2(62.0, 280.0),
            vec2(212.0, 260.0),
            vec2(212.0, 279.0),
            vec2(
                if self.workspace.more_colors {
                    390.0
                } else {
                    212.0
                },
                275.0,
            ),
        ];
        for i in 0..4 {
            if !self.workspace.visible[i] || !self.workspace.native[i] {
                self.workspace.native_initial_position[i] = None;
                self.workspace.native_initial_size[i] = None;
            }
            if !self.workspace.visible[i] {
                continue;
            }
            let mut open = true;
            if self.workspace.reset[i] {
                self.workspace.generation[i] += 1;
                self.workspace.reset[i] = false;
                self.workspace.native_positions[i] = None;
                self.workspace.native_sizes[i] = None;
                self.workspace.native_initial_position[i] = None;
                self.workspace.native_initial_size[i] = None;
            }
            if self.workspace.native[i] {
                self.native_utility_window(ctx, i, positions[i], sizes[i]);
                continue;
            }
            let window = egui::Window::new(WINDOW_NAMES[i])
                .id(egui::Id::new(("utility", i, self.workspace.generation[i])))
                .open(&mut open)
                .default_pos(positions[i])
                .default_size(sizes[i])
                .min_width(if i == 0 { 50.0 } else { 180.0 })
                .constrain_to(editing)
                .collapsible(false)
                .resizable(i == 1 || i == 2);
            let response = window.show(ctx, |ui| {
                if i == 1 || i == 2 {
                    ui.set_min_height(sizes[i].y - 35.0);
                }
                self.utility_content(ui, i);
            });
            if let Some(response) = response {
                let title = Rect::from_min_size(
                    response.response.rect.min,
                    vec2(response.response.rect.width(), 23.),
                );
                let title_click = !self.utility_modal_active() && ctx.input(|input| input.events.iter().any(|event|
                    matches!(event, egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed: false, .. } if title.contains(*pos))));
                egui::Popup::context_menu(&response.response)
                    .open_memory(title_click.then_some(egui::SetOpenCommand::Bool(true)))
                    .show(|ui| {
                        if window_action(
                            ui,
                            "Float Outside Main Window",
                            crate::icons::Icon::FloatWindow,
                        )
                        .clicked()
                        {
                            self.workspace.native[i] = true;
                            self.workspace.native_positions[i] =
                                ctx.input(|v| v.viewport().inner_rect).map(|main| {
                                    let p = main.min + response.response.rect.min.to_vec2();
                                    [p.x, p.y]
                                });
                            ui.close();
                        }
                    });
            }
            self.workspace.visible[i] = open;
        }
    }
    fn utility_content(&mut self, ui: &mut egui::Ui, i: usize) {
        match i {
            0 => self.tools_window(ui),
            1 => self.history_window(ui),
            2 => self.layers_window(ui),
            _ => self.colors_window(ui),
        }
    }
    fn utility_modal_active(&self) -> bool {
        self.pending_close.is_some()
            || self.size_dialog.is_some()
            || self.paste_dialog.is_some()
            || self.new_dialog
            || self.error.is_some()
            || self.layer_properties.is_some()
            || self.effect.is_some()
            || self.dropped_files.is_some()
            || self.palette_save.is_some()
    }
    fn native_utility_window(&mut self, ctx: &egui::Context, i: usize, position: Pos2, size: Vec2) {
        let root = ctx.input(|input| input.viewport().clone());
        let size = match i {
            0 => vec2(70., 300.),
            3 => vec2(
                if self.workspace.more_colors {
                    440.
                } else {
                    228.
                },
                350.,
            ),
            _ => *self.workspace.native_initial_size[i].get_or_insert_with(|| {
                self.workspace.native_sizes[i].map_or(size + vec2(0., 26.), Vec2::from)
            }),
        };
        let mut builder = egui::ViewportBuilder::default()
            .with_title(format!("{} — OpenPDN", WINDOW_NAMES[i]))
            .with_inner_size(size)
            .with_min_inner_size(if i == 0 {
                vec2(62., 280.)
            } else {
                vec2(212., 200.)
            })
            .with_resizable(i == 1 || i == 2)
            .with_minimize_button(false)
            .with_maximize_button(false)
            .with_taskbar(false)
            .with_window_type(egui::X11WindowType::Utility)
            .with_visible(root.minimized != Some(true));
        if self.workspace.native_initial_position[i].is_none() {
            self.workspace.native_initial_position[i] = self.workspace.native_positions[i]
                .map(Pos2::from)
                .or_else(|| root.inner_rect.map(|main| main.min + position.to_vec2()));
        }
        if let Some(position) = self.workspace.native_initial_position[i] {
            builder = builder.with_position(position);
        }
        let id =
            egui::ViewportId::from_hash_of(("native-utility", i, self.workspace.generation[i]));
        ctx.show_viewport_immediate(id, builder, |ui, class| {
            if self.smoke_frames.is_some() && self.frame_count == 0 {
                eprintln!(
                    "OpenPDN native utility: {} ({})",
                    WINDOW_NAMES[i],
                    if class == egui::ViewportClass::Immediate {
                        "Immediate"
                    } else {
                        "Embedded"
                    }
                );
            }
            let child = ui.ctx().clone();
            let window_state = (
                self.workspace.visible,
                self.workspace.native,
                self.workspace.more_colors,
            );
            child.input(|input| {
                if let Some(rect) = input.viewport().outer_rect {
                    self.workspace.native_positions[i] = Some([rect.min.x, rect.min.y]);
                }
                if (i == 1 || i == 2)
                    && let Some(rect) = input.viewport().inner_rect
                {
                    self.workspace.native_sizes[i] = Some([rect.width(), rect.height()]);
                }
                if input.viewport().close_requested() {
                    self.workspace.visible[i] = false;
                }
            });
            let modal = self.utility_modal_active();
            if modal {
                // Immediate viewports render inside the root pass, before its
                // dialogs. A window manager can reject a focus request; keep
                // modal keyboard input on the root even in that case.
                let (events, modifiers) = child.input_mut(|input| {
                    let mut events = Vec::new();
                    input.events.retain(|event| {
                        if matches!(event, egui::Event::Key { .. } | egui::Event::Text(_)
                            | egui::Event::Paste(_) | egui::Event::Copy | egui::Event::Cut
                            | egui::Event::Ime(_)) {
                            events.push(event.clone());
                            false
                        } else {
                            true
                        }
                    });
                    (events, input.modifiers)
                });
                if !events.is_empty() {
                    child.input_mut_for(egui::ViewportId::ROOT, |input| {
                        input.events.extend(events);
                        input.modifiers = modifiers;
                    });
                }
            } else {
                self.shortcuts(&child);
            }
            egui::CentralPanel::default().show(ui, |ui| {
                if self.utility_modal_active() {
                    ui.disable();
                }
                ui.horizontal(|ui| {
                    if icon(ui, "dock-window", "Dock in Main Window", false, true).clicked() {
                        self.workspace.native[i] = false;
                        self.workspace.reset[i] = true;
                    }
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 24.), Sense::drag());
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        WINDOW_NAMES[i],
                        FontId::proportional(12.),
                        ui.visuals().text_color(),
                    );
                    let press = ui.is_enabled() && child.input(|input| input.events.iter().any(|event|
                        matches!(event, egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } if rect.contains(*pos))));
                    if press {
                        // Let the window manager retain pointer capture and track
                        // desktop coordinates while the window moves. Rebuilding
                        // them from local pointer positions can cause overshoot.
                        child.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                });
                self.utility_content(ui, i);
            });
            if (!modal && self.utility_modal_active())
                || (modal && child.input(|input| input.viewport().focused == Some(true)))
            {
                child.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
            }
            if child.input(|input| !input.events.is_empty())
                || self.refresh
                || window_state
                != (
                    self.workspace.visible,
                    self.workspace.native,
                    self.workspace.more_colors,
                )
            {
                child.request_repaint_of(egui::ViewportId::ROOT);
            }
        });
    }
    fn tools_window(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
        egui::Grid::new("tools_grid")
            .min_col_width(0.0)
            .spacing(vec2(1.0, 1.0))
            .show(ui, |ui| {
                for (i, (glyph, name, tool)) in TOOL_SLOTS.iter().enumerate() {
                    if icon(ui, glyph, name, *tool == Some(self.tool), tool.is_some()).clicked() {
                        self.change_tool(tool.unwrap());
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    }
    fn history_window(&mut self, ui: &mut egui::Ui) {
        let height = (ui.available_height() - 40.0).max(90.0);
        let entries: Vec<(String, bool)> = self
            .editor()
            .history()
            .map(|(s, b)| (s.to_owned(), b))
            .collect();
        let cursor = self.editor().history_position();
        let mut target = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(height)
            .min_scrolled_height(height)
            .show(ui, |ui| {
                ui.set_min_width(190.0);
                if ui.selectable_label(cursor == 0, "New Image").clicked() {
                    target = Some(0);
                }
                for (i, (label, applied)) in entries.iter().enumerate() {
                    let text = egui::RichText::new(label).color(if *applied {
                        Color32::BLACK
                    } else {
                        Color32::GRAY
                    });
                    if ui.selectable_label(cursor == i + 1, text).clicked() {
                        target = Some(if cursor == i + 1 { i } else { i + 1 });
                    }
                }
            });
        if let Some(target) = target {
            self.finish_stroke();
            self.editor_mut().seek_history(target);
            self.refresh = true;
        }
        ui.separator();
        ui.horizontal(|ui| {
            if icon(
                ui,
                "undo",
                "Undo (Ctrl+Z)",
                false,
                self.editor().can_undo() || self.transform.is_some(),
            )
            .clicked()
            {
                self.undo_action();
            }
            if icon(ui, "redo", "Redo (Ctrl+Y)", false, self.editor().can_redo()).clicked() {
                self.finish_stroke();
                self.editor_mut().redo();
                self.refresh = true;
            }
        });
    }
    fn layers_window(&mut self, ui: &mut egui::Ui) {
        let height = (ui.available_height() - 40.0).max(110.0);
        let count = self.editor().document.layers.len();
        self.workspace.layer_thumbnails.truncate(count);
        egui::ScrollArea::vertical()
            .id_salt("layer_list")
            .auto_shrink([false, false])
            .max_height(height)
            .min_scrolled_height(height)
            .show(ui, |ui| {
                ui.set_min_width(190.0);
                for index in (0..count).rev() {
                    let layer = &self.editor().document.layers[index];
                    let name = layer.name.clone();
                    let mut visible = layer.visible;
                    let generation = self.workspace.render_generation;
                    while self.workspace.layer_thumbnails.len() <= index {
                        let image = thumbnail(&self.editor().document, Some(index));
                        let texture = ui.ctx().load_texture(
                            "layer-preview",
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.workspace.layer_thumbnails.push((u64::MAX, texture));
                    }
                    if self.workspace.layer_thumbnails[index].0 != generation {
                        let image = thumbnail(&self.editor().document, Some(index));
                        let cached = &mut self.workspace.layer_thumbnails[index];
                        cached.1.set(image, egui::TextureOptions::LINEAR);
                        cached.0 = generation;
                    }
                    let texture = self.workspace.layer_thumbnails[index].1.clone();
                    let selected = self.editor().document.active == index;
                    let frame = egui::Frame::new()
                        .fill(if selected {
                            Color32::from_rgb(194, 222, 244)
                        } else {
                            Color32::from_gray(250)
                        })
                        .inner_margin(4);
                    let row = frame
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Image::new((texture.id(), vec2(40.0, 34.0)))
                                        .fit_to_exact_size(vec2(40.0, 34.0)),
                                );
                                let name_response = ui.add_sized(
                                    [ui.available_width() - 22.0, 34.0],
                                    egui::Label::new(name).sense(Sense::click_and_drag()),
                                );
                                let checkbox = ui.checkbox(&mut visible, "");
                                (name_response, checkbox.changed())
                            })
                            .inner
                        })
                        .inner;
                    if row.1 {
                        self.edit("Layer Visibility", |d| d.layers[index].visible = visible);
                    }
                    if row.0.clicked() || row.0.double_clicked() {
                        self.finish_stroke();
                        self.editor_mut().document.active = index;
                    }
                    if row.0.double_clicked() {
                        self.open_layer_properties();
                    }
                    if row.0.drag_stopped()
                        && let Some(position) = row.0.interact_pointer_pos()
                    {
                        let delta = ((row.0.rect.center().y - position.y) / 46.0).round() as isize;
                        let target = (index as isize + delta).clamp(0, count as isize - 1) as usize;
                        self.edit("Move Layer", |d| {
                            let layer = d.layers.remove(index);
                            d.layers.insert(target, layer);
                            d.active = target;
                        });
                    }
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            if icon(ui, "add", "Add New Layer (Ctrl+Shift+N)", false, true).clicked() {
                self.edit("Add New Layer", Document::add_layer);
            }
            if icon(ui, "delete", "Delete Layer", false, count > 1).clicked() {
                self.edit("Delete Layer", Document::delete_layer);
            }
            if icon(
                ui,
                "duplicate",
                "Duplicate Layer (Ctrl+Shift+D)",
                false,
                true,
            )
            .clicked()
            {
                self.edit("Duplicate Layer", Document::duplicate_layer);
            }
            if icon(
                ui,
                "merge",
                "Merge Layer Down (Ctrl+M)",
                false,
                self.editor().document.active > 0,
            )
            .clicked()
            {
                self.merge_down();
            }
            let active = self.editor().document.active;
            if icon(ui, "up", "Move Layer Up", false, active + 1 < count).clicked() {
                self.move_layer(true, ui.input(|i| i.modifiers.command));
            }
            if icon(ui, "down", "Move Layer Down", false, active > 0).clicked() {
                self.move_layer(false, ui.input(|i| i.modifiers.command));
            }
            if icon(ui, "properties", "Layer Properties (F4)", false, true).clicked() {
                self.open_layer_properties();
            }
        });
    }
    fn move_layer(&mut self, up: bool, extreme: bool) {
        self.edit("Move Layer", |d| {
            let from = d.active;
            let to = if up {
                if extreme {
                    d.layers.len() - 1
                } else {
                    (from + 1).min(d.layers.len() - 1)
                }
            } else if extreme {
                0
            } else {
                from.saturating_sub(1)
            };
            let layer = d.layers.remove(from);
            d.layers.insert(to, layer);
            d.active = to;
        });
    }
    fn open_layer_properties(&mut self) {
        self.finish_stroke();
        let index = self.editor().document.active;
        let layer = &self.editor().document.layers[index];
        self.layer_properties = Some(LayerProperties {
            index,
            name: layer.name.clone(),
            visible: layer.visible,
            opacity: layer.opacity,
            mode: layer.blend_mode,
        });
    }
    pub(super) fn layer_properties_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.layer_properties.take() else {
            return;
        };
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("layer_properties")).show(ctx, |ui| {
            ui.set_min_width(300.0);
            ui.heading("Layer Properties");
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut draft.name);
            });
            ui.checkbox(&mut draft.visible, "Visible");
            ui.separator();
            ui.label("Blending");
            ui.horizontal(|ui| {
                ui.label("Mode:");
                egui::ComboBox::from_id_salt("layer_blend_mode")
                    .selected_text(draft.mode.name())
                    .show_ui(ui, |ui| {
                        for mode in BlendMode::ALL {
                            ui.selectable_value(&mut draft.mode, mode, mode.name());
                        }
                    });
            });
            ui.add(egui::Slider::new(&mut draft.opacity, 0..=255).text("Opacity"));
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = ui.button("Cancel").clicked();
                accept = ui.button("OK").clicked();
            });
        });
        if accept {
            self.edit("Layer Properties", |d| {
                let layer = &mut d.layers[draft.index];
                layer.name = draft.name;
                layer.visible = draft.visible;
                layer.opacity = draft.opacity;
                layer.blend_mode = draft.mode;
            });
        } else if !cancel && !ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.layer_properties = Some(draft);
        }
    }
    pub(super) fn workspace_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(self.tool.name()).on_hover_text(format!(
                "{:?} · {}",
                self.gpu.info.backend, self.gpu.info.name
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("+").clicked() {
                    self.zoom = (self.zoom * 1.25).min(16.0);
                }
                ui.add(
                    egui::Slider::new(&mut self.zoom, 0.05..=16.0)
                        .logarithmic(true)
                        .show_value(false),
                );
                if ui.small_button("−").clicked() {
                    self.zoom = (self.zoom / 1.25).max(0.05);
                }
                ui.label(format!("{:.0}%", self.zoom * 100.0));
                ui.separator();
                ui.label("px");
                if let Some((x, y)) = self.workspace.cursor {
                    ui.label(format!("{x}, {y}"));
                }
                let doc = &self.editor().document;
                ui.label(format!("{} × {}", doc.width, doc.height));
            });
        });
    }
    pub(super) fn canvas_overlays(&self, ui: &egui::Ui, rect: Rect) {
        let doc = &self.editor().document;
        if self.workspace.grid && self.zoom >= 2.0 {
            let clip = rect.intersect(ui.clip_rect());
            let painter = ui.painter().with_clip_rect(clip);
            let x0 = ((clip.left() - rect.left()) / self.zoom).floor().max(0.0) as u32;
            let x1 = ((clip.right() - rect.left()) / self.zoom)
                .ceil()
                .min(doc.width as f32) as u32;
            let y0 = ((clip.top() - rect.top()) / self.zoom).floor().max(0.0) as u32;
            let y1 = ((clip.bottom() - rect.top()) / self.zoom)
                .ceil()
                .min(doc.height as f32) as u32;
            for x in x0..=x1 {
                let x = rect.left() + x as f32 * self.zoom;
                painter.line_segment(
                    [pos2(x, clip.top()), pos2(x, clip.bottom())],
                    (0.5, Color32::from_black_alpha(65)),
                );
            }
            for y in y0..=y1 {
                let y = rect.top() + y as f32 * self.zoom;
                painter.line_segment(
                    [pos2(clip.left(), y), pos2(clip.right(), y)],
                    (0.5, Color32::from_black_alpha(65)),
                );
            }
        }
    }
    pub(super) fn draw_rulers(&self, ctx: &egui::Context, editing: Rect) {
        if !self.workspace.rulers {
            return;
        }
        let Some(canvas) = self.workspace.canvas else {
            return;
        };
        let painter = ctx
            .layer_painter(egui::LayerId::background())
            .with_clip_rect(editing);
        let top = Rect::from_min_max(editing.min, pos2(editing.right(), editing.top() + 18.0));
        let left = Rect::from_min_max(editing.min, pos2(editing.left() + 18.0, editing.bottom()));
        painter.rect_filled(top, 0.0, Color32::from_gray(247));
        painter.rect_filled(left, 0.0, Color32::from_gray(247));
        // Keep major labels about 50 screen pixels apart at every zoom.
        let desired = 50.0 / self.zoom;
        let magnitude = 10.0_f32.powf(desired.log10().floor());
        let normalized = desired / magnitude;
        let step = magnitude
            * if normalized <= 1.0 {
                1.0
            } else if normalized <= 2.0 {
                2.0
            } else if normalized <= 5.0 {
                5.0
            } else {
                10.0
            };
        for (horizontal, origin, min, max, extent) in [
            (
                true,
                canvas.left(),
                editing.left() + 18.0,
                editing.right(),
                self.editor().document.width,
            ),
            (
                false,
                canvas.top(),
                editing.top() + 18.0,
                editing.bottom(),
                self.editor().document.height,
            ),
        ] {
            let first = (((min - origin) / self.zoom / step).floor() as i32).max(0);
            let last = (((max - origin) / self.zoom / step).ceil() as i32)
                .min((extent as f32 / step).ceil() as i32);
            for index in first..=last {
                let value = index as f32 * step;
                let coordinate = origin + value * self.zoom;
                if coordinate < min || coordinate > max {
                    continue;
                }
                let start = if horizontal {
                    pos2(coordinate, top.bottom() - 5.0)
                } else {
                    pos2(left.right() - 5.0, coordinate)
                };
                let end = if horizontal {
                    pos2(coordinate, top.bottom())
                } else {
                    pos2(left.right(), coordinate)
                };
                painter.line_segment([start, end], (1.0, Color32::GRAY));
                let label = if horizontal {
                    pos2(coordinate + 2.0, top.top() + 1.0)
                } else {
                    pos2(left.left() + 1.0, coordinate + 1.0)
                };
                painter.text(
                    label,
                    Align2::LEFT_TOP,
                    format!("{value:.0}"),
                    FontId::proportional(9.0),
                    Color32::from_gray(70),
                );
            }
        }
        if let Some((x, y)) = self.workspace.cursor {
            let color = Color32::from_rgb(45, 135, 210);
            let x = canvas.left() + x as f32 * self.zoom;
            let y = canvas.top() + y as f32 * self.zoom;
            painter.line_segment([pos2(x, top.top()), pos2(x, top.bottom())], (1.0, color));
            painter.line_segment([pos2(left.left(), y), pos2(left.right(), y)], (1.0, color));
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Preferences {
    visible: [bool; 4],
    #[serde(default)]
    native: [bool; 4],
    #[serde(default)]
    native_positions: [Option<[f32; 2]>; 4],
    #[serde(default)]
    native_sizes: [Option<[f32; 2]>; 4],
    generation: [u64; 4],
    grid: bool,
    rulers: bool,
    more_colors: bool,
    primary: Pixel,
    #[serde(default = "default_palette")]
    palette: Vec<Pixel>,
    secondary: Pixel,
    tool: usize,
    brush_size: f32,
    #[serde(default)]
    brush_options: BrushOptions,
    #[serde(default)]
    selection_mode: SelectionMode,
    #[serde(default)]
    selection_options: SelectionOptions,
    #[serde(default)]
    text_options: crate::content::TextOptions,
    tolerance: u8,
    #[serde(default)]
    picker_size: SampleSize,
    #[serde(default)]
    picker_behavior: PickerBehavior,
    #[serde(default)]
    sample_composite: bool,
    #[serde(default)]
    tool_blend: ToolBlend,
    #[serde(default)]
    fill_style: FillStyle,
    #[serde(default)]
    transform_bilinear: bool,
    #[serde(default = "default_linear")]
    transform_linear: bool,
}
fn default_palette() -> Vec<Pixel> {
    crate::palette::DEFAULT.to_vec()
}
fn default_linear() -> bool {
    true
}
fn thumbnail(doc: &Document, layer_index: Option<usize>) -> egui::ColorImage {
    let scale = (64.0 / doc.width as f32)
        .min(48.0 / doc.height as f32)
        .min(1.0);
    let width = (doc.width as f32 * scale).round().max(1.0) as usize;
    let height = (doc.height as f32 * scale).round().max(1.0) as usize;
    let layers: Vec<&Layer> = if let Some(index) = layer_index {
        vec![&doc.layers[index]]
    } else {
        doc.layers.iter().filter(|l| l.visible).collect()
    };
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let source = (y * doc.height as usize / height) * doc.width as usize
                + x * doc.width as usize / width;
            let mut pixel = TRANSPARENT;
            for layer in &layers {
                pixel = blend(
                    pixel,
                    layer.pixels[source],
                    if layer_index.is_some() {
                        255
                    } else {
                        layer.opacity
                    },
                    if layer_index.is_some() {
                        BlendMode::Normal
                    } else {
                        layer.blend_mode
                    },
                );
            }
            let background = if (x / 5 + y / 5) % 2 == 0 { 220 } else { 255 };
            let out = blend(
                [background, background, background, 255],
                pixel,
                255,
                BlendMode::Normal,
            );
            pixels.push(Color32::from_rgb(out[0], out[1], out[2]));
        }
    }
    egui::ColorImage::new([width, height], pixels)
}
