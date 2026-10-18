use super::workspace::icon;
use super::*;
use egui::{Color32, Rect, vec2};

pub(super) struct SavePaletteDialog {
    name: String,
    names: Vec<String>,
    focus: bool,
}
pub(super) fn default_library() -> Option<crate::palette::Library> {
    let user_dir = std::env::var_os("OPENPDN_PALETTES_PATH")
        .or_else(|| std::env::var_os("OPENDOTNET_PALETTES_PATH"))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("OPENPDN_STATE_PATH")
                .or_else(|| std::env::var_os("OPENDOTNET_STATE_PATH"))
                .and_then(|p| PathBuf::from(p).parent().map(|p| p.join("Palettes")))
        })
        .or_else(|| {
            let current = eframe::storage_dir("OpenPDN")?.join("Palettes");
            let previous = eframe::storage_dir("OpenDotNet").map(|p| p.join("Palettes"));
            Some(if !current.exists() {
                previous.filter(|p| p.is_dir()).unwrap_or(current)
            } else {
                current
            })
        })?;
    let app_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("Palettes")));
    Some(crate::palette::Library { user_dir, app_dir })
}

fn palette_action(
    ui: &mut egui::Ui,
    label: &str,
    glyph: crate::icons::Icon,
    checked: bool,
    enabled: bool,
) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(format!("     {label}")).min_size(vec2(210., 22.)),
    );
    let rect = Rect::from_center_size(response.rect.left_center() + vec2(12., 0.), vec2(16., 16.));
    if checked {
        let c = rect.center();
        ui.painter().line_segment(
            [c + vec2(-5., 0.), c + vec2(-1., 4.)],
            (1.5, ui.visuals().text_color()),
        );
        ui.painter().line_segment(
            [c + vec2(-1., 4.), c + vec2(6., -4.)],
            (1.5, ui.visuals().text_color()),
        );
    } else {
        crate::icons::paint(ui.painter(), rect, glyph, enabled);
    }
    response
}

fn swatch(ui: &egui::Ui, rect: Rect, color: Pixel) {
    let painter = ui.painter().with_clip_rect(rect);
    for y in 0..((rect.height() / 4.).ceil() as usize) {
        for x in 0..((rect.width() / 4.).ceil() as usize) {
            painter.rect_filled(
                Rect::from_min_size(rect.min + vec2(x as f32 * 4., y as f32 * 4.), vec2(4., 4.)),
                0.,
                Color32::from_gray(if (x + y) % 2 == 0 { 210 } else { 255 }),
            );
        }
    }
    painter.rect_filled(
        rect,
        0.,
        Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]),
    );
    ui.painter().rect_stroke(
        rect,
        0.,
        (1., Color32::from_gray(70)),
        egui::StrokeKind::Inside,
    );
}
impl PaintApp {
    pub(super) fn colors_window(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.workspace.secondary_selected, false, "Primary");
            ui.selectable_value(&mut self.workspace.secondary_selected, true, "Secondary");
            if ui
                .small_button(if self.workspace.more_colors {
                    "Less «"
                } else {
                    "More »"
                })
                .clicked()
            {
                self.workspace.more_colors = !self.workspace.more_colors;
            }
        });
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(196.);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 1.;
                    ui.vertical(|ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(44., 52.), Sense::click());
                        let front = Rect::from_min_size(rect.min, vec2(29., 29.));
                        let back = Rect::from_min_size(rect.min + vec2(13., 13.), vec2(29., 29.));
                        swatch(ui, back, self.secondary);
                        swatch(ui, front, self.primary);
                        if response.clicked() {
                            self.workspace.secondary_selected = response
                                .interact_pointer_pos()
                                .is_some_and(|p| !front.contains(p));
                        }
                        if icon(ui, "swap-colors", "Swap Colors (X)", false, true).clicked() {
                            std::mem::swap(&mut self.primary, &mut self.secondary);
                        }
                        if icon(ui, "reset-colors", "Reset to black and white", false, true)
                            .clicked()
                        {
                            self.primary = [0, 0, 0, 255];
                            self.secondary = [255; 4];
                        }
                    });
                    self.native_color_wheel(ui);
                });
                ui.separator();
                egui::Grid::new("color_swatches")
                    .min_col_width(0.)
                    .min_row_height(0.)
                    .spacing(egui::Vec2::ZERO)
                    .show(ui, |ui| {
                        for n in 0..self.palette.len() {
                            let color = self.palette[n];
                            let (rect, response) =
                                ui.allocate_exact_size(vec2(12., 12.), Sense::click());
                            swatch(ui, rect, color);
                            if self.palette_add && response.hovered() {
                                ui.painter().rect_stroke(
                                    rect.shrink(1.),
                                    0.,
                                    (1., Color32::YELLOW),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            if response.clicked() || response.secondary_clicked() {
                                if self.palette_add {
                                    self.palette[n] = if self.workspace.secondary_selected {
                                        self.secondary
                                    } else {
                                        self.primary
                                    };
                                    self.palette_add = false;
                                } else if self.workspace.secondary_selected
                                    ^ response.secondary_clicked()
                                {
                                    self.secondary = color;
                                } else {
                                    self.primary = color;
                                }
                            }
                            response.on_hover_text(format!(
                                "#{:02X}{:02X}{:02X} · Alpha {}",
                                color[0], color[1], color[2], color[3]
                            ));
                            if (n + 1) % 16 == 0 {
                                ui.end_row();
                            }
                        }
                    });
                ui.horizontal(|ui| {
                    if icon(
                        ui,
                        "add-color",
                        "Add Color: click a palette swatch to replace it",
                        self.palette_add,
                        !self.palette.is_empty(),
                    )
                    .clicked()
                    {
                        self.palette_add = !self.palette_add;
                    }
                    let response = ui.menu_button("     Palettes", |ui| self.palettes_menu(ui));
                    if response.inner.is_none() {
                        self.palette_menu_open = false;
                    }
                    let rect = Rect::from_center_size(
                        response.response.rect.left_center() + vec2(11., 0.),
                        vec2(16., 16.),
                    );
                    crate::icons::paint(ui.painter(), rect, crate::icons::Icon::Palette, true);
                    if self.palette_add {
                        ui.label("Choose a swatch");
                    }
                });
            });
            if self.workspace.more_colors {
                self.numeric_colors(ui);
            }
        });
    }
    fn palettes_menu(&mut self, ui: &mut egui::Ui) {
        use crate::palette::Location;
        if !self.palette_menu_open {
            self.palette_menu_open = true;
            if let Some(library) = &self.palette_library {
                match library.entries() {
                    Ok(entries) => self.named_palettes = entries,
                    Err(e) => self.error = Some(format!("Loading palette library: {e:#}")),
                }
            }
        }
        let first_match = self
            .named_palettes
            .iter()
            .position(|p| p.colors == self.palette);
        self.named_palette_rows(ui, Location::User, first_match);
        if self
            .named_palettes
            .iter()
            .any(|p| p.location == Location::User)
        {
            ui.separator();
        }
        if palette_action(
            ui,
            "Save Current Palette As…",
            crate::icons::Icon::Save,
            false,
            self.palette_library.is_some(),
        )
        .clicked()
        {
            self.finish_stroke();
            self.finish_content();
            self.palette_save = Some(SavePaletteDialog {
                name: String::new(),
                names: self
                    .named_palettes
                    .iter()
                    .filter(|p| p.location == Location::User)
                    .map(|p| p.name.clone())
                    .collect(),
                focus: true,
            });
            ui.close();
        }
        if palette_action(
            ui,
            "Open Palettes Folder",
            crate::icons::Icon::Open,
            false,
            self.palette_library.is_some(),
        )
        .clicked()
        {
            ui.close();
            let result = (|| -> Result<()> {
                let library = self.palette_library.as_ref().unwrap();
                std::fs::create_dir_all(&library.user_dir)?;
                let program = if cfg!(target_os = "windows") {
                    "explorer"
                } else if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                let mut child = std::process::Command::new(program)
                    .arg(&library.user_dir)
                    .spawn()?;
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(())
            })();
            if let Err(e) = result {
                self.error = Some(format!("Opening palettes folder: {e:#}"));
            }
        }
        ui.separator();
        if palette_action(
            ui,
            "Reset to Default Palette",
            crate::icons::Icon::ResetColors,
            false,
            true,
        )
        .clicked()
        {
            self.palette = crate::palette::DEFAULT.to_vec();
            self.palette_add = false;
            ui.close();
        }
        if self
            .named_palettes
            .iter()
            .any(|p| p.location == Location::App)
        {
            ui.separator();
            self.named_palette_rows(ui, Location::App, first_match);
        }
        ui.separator();
        if palette_action(
            ui,
            "Import Palette File…",
            crate::icons::Icon::Open,
            false,
            true,
        )
        .clicked()
        {
            ui.close();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Paint.NET palette", &["txt"])
                .pick_file()
            {
                match crate::palette::load(&path) {
                    Ok(colors) => {
                        self.palette = colors;
                        self.palette_add = false;
                    }
                    Err(e) => self.error = Some(format!("Loading palette: {e:#}")),
                }
            }
        }
        if palette_action(
            ui,
            "Export Palette File…",
            crate::icons::Icon::Save,
            false,
            true,
        )
        .clicked()
        {
            ui.close();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Paint.NET palette", &["txt"])
                .set_file_name("Palette.txt")
                .save_file()
                && let Err(e) = crate::palette::save(&path, &self.palette)
            {
                self.error = Some(format!("Saving palette: {e:#}"));
            }
        }
    }
    fn named_palette_rows(
        &mut self,
        ui: &mut egui::Ui,
        location: crate::palette::Location,
        first_match: Option<usize>,
    ) {
        for (i, entry) in self
            .named_palettes
            .iter()
            .enumerate()
            .filter(|(_, p)| p.location == location)
        {
            if palette_action(
                ui,
                &entry.name,
                crate::icons::Icon::Palette,
                first_match == Some(i),
                true,
            )
            .clicked()
            {
                self.palette.clone_from(&entry.colors);
                self.palette_add = false;
                ui.close();
            }
        }
    }
    pub(super) fn save_palette_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.palette_save.take() else {
            return;
        };
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("save_palette")).show(ctx, |ui| {
            ui.set_width(310.);
            ui.heading("Save Current Palette");
            ui.label("Type a name for the palette:");
            let invalid =
                !draft.name.is_empty() && crate::palette::library_filename(&draft.name).is_none();
            let mut edit = egui::TextEdit::singleline(&mut draft.name).desired_width(f32::INFINITY);
            if invalid {
                edit = edit
                    .background_color(Color32::RED)
                    .text_color(Color32::WHITE);
            }
            let response = ui.add(edit);
            if draft.focus {
                response.request_focus();
                draft.focus = false;
            }
            ui.label("Palettes that have already been saved:");
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_size(vec2(295., 110.));
                egui::ScrollArea::vertical()
                    .max_height(140.)
                    .show(ui, |ui| {
                        for name in &draft.names {
                            if ui.selectable_label(false, name).clicked() {
                                draft.name.clone_from(name);
                                response.request_focus();
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                let valid = crate::palette::library_filename(&draft.name).is_some();
                accept = ui.add_enabled(valid, egui::Button::new("Save")).clicked()
                    || valid && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                cancel = ui.button("Cancel").clicked()
                    || ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
            });
        });
        if accept {
            let result = self
                .palette_library
                .as_ref()
                .unwrap()
                .save_named(&draft.name, &self.palette);
            if let Err(e) = result {
                self.error = Some(format!("Saving named palette: {e:#}"));
                self.palette_save = Some(draft);
            }
        } else if !cancel {
            self.palette_save = Some(draft);
        }
    }
    fn numeric_colors(&mut self, ui: &mut egui::Ui) {
        let color = if self.workspace.secondary_selected {
            &mut self.secondary
        } else {
            &mut self.primary
        };
        ui.vertical(|ui| {
            ui.set_max_width(225.);
            ui.spacing_mut().slider_width = 125.;
            ui.label("RGB");
            for (channel, label) in ["Red", "Green", "Blue"].into_iter().enumerate() {
                ui.add(egui::Slider::new(&mut color[channel], 0..=255).text(label));
            }
            if *color != self.color_hex_basis {
                self.color_hex = format!("{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
                self.color_hex_basis = *color;
            }
            ui.horizontal(|ui| {
                ui.label("Hex:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.color_hex)
                        .desired_width(90.)
                        .char_limit(16),
                );
                if response.changed()
                    && let Some(rgb) = crate::palette::parse_rgb_hex(&self.color_hex)
                {
                    color[..3].copy_from_slice(&rgb);
                    self.color_hex_basis = *color;
                }
                if response.lost_focus() {
                    self.color_hex = format!("{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
                }
            });
            ui.label("HSV");
            if color[..3] != self.color_hsv_basis[..3] {
                self.color_hsv = crate::palette::rgb_to_hsv(*color);
                self.color_hsv_basis = *color;
            }
            let hsv = &mut self.color_hsv;
            let changed = ui
                .add(egui::Slider::new(&mut hsv[0], 0..=360).text("Hue"))
                .changed()
                | ui.add(egui::Slider::new(&mut hsv[1], 0..=100).text("Saturation"))
                    .changed()
                | ui.add(egui::Slider::new(&mut hsv[2], 0..=100).text("Value"))
                    .changed();
            if changed {
                color[..3].copy_from_slice(&crate::palette::hsv_to_rgb(*hsv));
                self.color_hsv_basis = *color;
            }
            ui.label("Transparency — Alpha");
            ui.add(egui::Slider::new(&mut color[3], 0..=255).text("Alpha"));
        });
    }
}
