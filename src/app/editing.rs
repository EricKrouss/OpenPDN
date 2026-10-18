use super::*;
use crate::{
    editing::{Anchor, ClipboardImage},
    gpu::Resampler,
    resolution::{ImageSize, MeasurementUnit},
};
use std::borrow::Cow;

pub(super) fn image_size_fields(ui: &mut egui::Ui, size: &mut ImageSize) -> bool {
    let mut changed = false;
    let mut aspect = size.aspect();
    if ui.checkbox(&mut aspect, "Maintain aspect ratio").changed() {
        size.set_aspect(aspect);
        changed = true;
    }
    egui::Grid::new("pixel_size_fields")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Width:");
            let mut width = size.pixels()[0];
            if ui
                .add(
                    egui::DragValue::new(&mut width)
                        .range(1..=16384)
                        .suffix(" px"),
                )
                .changed()
            {
                size.set_pixel_width(f64::from(width));
                changed = true;
            }
            ui.end_row();
            ui.label("Height:");
            let mut height = size.pixels()[1];
            if ui
                .add(
                    egui::DragValue::new(&mut height)
                        .range(1..=16384)
                        .suffix(" px"),
                )
                .changed()
            {
                size.set_pixel_height(f64::from(height));
                changed = true;
            }
            ui.end_row();
        });
    ui.separator();
    ui.label("Print size");
    egui::Grid::new("print_size_fields")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Width:");
            let mut width = size.width();
            if ui
                .add(
                    egui::DragValue::new(&mut width)
                        .range(0.001..=1e6)
                        .speed(0.01)
                        .max_decimals(4),
                )
                .changed()
            {
                size.set_width(width);
                changed = true;
            }
            ui.end_row();
            ui.label("Height:");
            let mut height = size.height();
            if ui
                .add(
                    egui::DragValue::new(&mut height)
                        .range(0.001..=1e6)
                        .speed(0.01)
                        .max_decimals(4),
                )
                .changed()
            {
                size.set_height(height);
                changed = true;
            }
            ui.end_row();
            ui.label("Resolution:");
            let mut resolution = size.resolution();
            if ui
                .add(
                    egui::DragValue::new(&mut resolution)
                        .range(0.01..=65536.)
                        .max_decimals(4),
                )
                .changed()
            {
                size.set_resolution(resolution);
                changed = true;
            }
            ui.end_row();
            ui.label("Units:");
            let mut units = size.units();
            egui::ComboBox::from_id_salt("print_units")
                .selected_text(match units {
                    MeasurementUnit::Centimeter => "Pixels / centimeter",
                    _ => "Pixels / inch",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut units, MeasurementUnit::Inch, "Pixels / inch");
                    ui.selectable_value(
                        &mut units,
                        MeasurementUnit::Centimeter,
                        "Pixels / centimeter",
                    );
                });
            if units != size.units() {
                size.set_units(units);
                changed = true;
            }
            ui.end_row();
        });
    changed
}

pub(super) struct SizeDialog {
    canvas: bool,
    size: ImageSize,
    original: [u32; 2],
    percent: f64,
    sampler: Resampler,
    linear: bool,
    anchor: Anchor,
    fill: usize,
}
pub(super) struct PasteDialog {
    image: ClipboardImage,
    new_layer: bool,
}
impl PaintApp {
    pub(super) fn open_new_dialog(&mut self) {
        self.finish_stroke();
        let clipboard_pixels = self
            .read_clipboard_data()
            .ok()
            .map(|image| [image.width as u32, image.height as u32]);
        let aspect = self.new_size.aspect();
        self.new_size = ImageSize::for_new(clipboard_pixels);
        self.new_size.set_aspect(aspect);
        self.new_dialog = true;
    }
    pub(super) fn open_size_dialog(&mut self, canvas: bool) {
        self.finish_stroke();
        let d = &self.editor().document;
        self.size_dialog = Some(SizeDialog {
            canvas,
            size: {
                let mut size = ImageSize::for_document(d);
                size.set_aspect(!canvas);
                size
            },
            original: [d.width, d.height],
            percent: 100.,
            sampler: Resampler::Cubic,
            linear: true,
            anchor: Anchor::Center,
            fill: 0,
        });
    }
    pub(super) fn crop(&mut self) {
        self.edit("Crop to Selection", Document::crop_to_selection);
    }
    pub(super) fn merge_down(&mut self) {
        self.edit("Merge Layer Down", Document::merge_down);
    }
    pub(super) fn copy_image(&mut self, cut: bool, merged: bool) {
        self.finish_stroke();
        let Some(image) = self.editor().document.copy_selection(merged) else {
            return;
        };
        let result = (|| -> Result<()> {
            if self.clipboard.is_none() {
                self.clipboard = Some(arboard::Clipboard::new()?);
            }
            self.clipboard
                .as_mut()
                .unwrap()
                .set_image(arboard::ImageData {
                    width: image.width as usize,
                    height: image.height as usize,
                    bytes: Cow::Borrowed(bytemuck::cast_slice(&image.pixels)),
                })?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.copied_image = Some(image);
                if cut {
                    self.edit("Cut", |doc| {
                        doc.clear_selection_pixels();
                        doc.selection = None;
                    });
                }
            }
            Err(e) => self.error = Some(format!("Could not copy image: {e:#}")),
        }
    }
    fn read_clipboard_data(&mut self) -> Result<arboard::ImageData<'static>> {
        if self.clipboard.is_none() {
            self.clipboard = Some(arboard::Clipboard::new()?);
        }
        let native = self.clipboard.as_mut().unwrap().get_image()?;
        u32::try_from(native.width)?;
        u32::try_from(native.height)?;
        let bytes = native
            .width
            .checked_mul(native.height)
            .and_then(|count| count.checked_mul(4));
        anyhow::ensure!(
            native.width > 0 && native.height > 0 && bytes == Some(native.bytes.len()),
            "Invalid clipboard image data"
        );
        Ok(native)
    }
    fn read_clipboard(&mut self) -> Result<ClipboardImage> {
        let native = self.read_clipboard_data()?;
        let width = native.width as u32;
        let height = native.height as u32;
        crate::document::pixel_count(width, height)?;
        let pixels = native
            .bytes
            .chunks_exact(4)
            .map(|v| [v[0], v[1], v[2], v[3]])
            .collect();
        let mut image = ClipboardImage {
            width,
            height,
            pixels,
            origin: [0, 0],
        };
        if let Some(saved) = &self.copied_image
            && saved.width == width
            && saved.height == height
            && saved.pixels == image.pixels
        {
            image.origin = saved.origin;
        }
        Ok(image)
    }
    pub(super) fn paste(&mut self, new_layer: bool, new_image: bool) {
        self.finish_stroke();
        match self.read_clipboard() {
            Ok(image) => {
                if new_image {
                    match image.into_document().and_then(|d| {
                        self.gpu.check_document(&d)?;
                        Ok(d)
                    }) {
                        Ok(document) => {
                            self.tabs.push(Tab {
                                editor: Editor::new(document),
                                path: None,
                                name: "Untitled".into(),
                                thumbnail: None,
                                thumbnail_revision: u64::MAX,
                            });
                            self.active = self.tabs.len() - 1;
                            self.refresh = true;
                            self.clone_anchor = None;
                            self.clone_offset = None;
                        }
                        Err(e) => self.error = Some(format!("{e:#}")),
                    }
                } else if image.width > self.editor().document.width
                    || image.height > self.editor().document.height
                {
                    self.paste_dialog = Some(PasteDialog { image, new_layer });
                } else {
                    self.commit_paste(image, new_layer, false);
                }
            }
            Err(e) => self.error = Some(format!("Could not paste image: {e:#}")),
        }
    }
    fn commit_paste(&mut self, image: ClipboardImage, new_layer: bool, expand: bool) {
        let result =
            crate::transform::PastePlan::new(&self.editor().document, image, new_layer, expand)
                .and_then(|plan| {
                    self.gpu.check_document(&plan.floating.base)?;
                    let preview = self.gpu.transform_selection(
                        &plan.floating,
                        self.transform_bilinear,
                        self.transform_linear,
                    )?;
                    Ok((plan, preview))
                });
        match result {
            Ok((plan, preview)) => {
                let floating = plan.begin(self.editor_mut());
                self.begin_paste(floating, preview);
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }
    pub(super) fn editing_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        // egui-winit consumes Ctrl+C/X/V key presses for text clipboard handling.
        // Releases retain their modifiers even when the clipboard contains only an image.
        let commands: Vec<_> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: false,
                        modifiers,
                        ..
                    } if modifiers.command && matches!(key, Key::C | Key::X | Key::V) => {
                        Some((*key, *modifiers))
                    }
                    _ => None,
                })
                .collect()
        });
        if !commands.is_empty() {
            ctx.input_mut(|i|i.events.retain(|event| !matches!(event,egui::Event::Copy|egui::Event::Cut|egui::Event::Paste(_)) && !matches!(event,egui::Event::Key { key:Key::C|Key::X|Key::V,pressed:false,modifiers,.. } if modifiers.command)));
        }
        for (key, m) in commands {
            match key {
                Key::C => self.copy_image(false, m.shift),
                Key::X if !m.shift => self.copy_image(true, false),
                Key::X => self.crop(),
                Key::V => self.paste(m.shift, m.alt),
                _ => {}
            }
        }
        let command = Modifiers::COMMAND;
        for (modifiers, key, action) in [
            (command | Modifiers::SHIFT, Key::R, 0),
            (command, Key::R, 1),
            (command, Key::H, 3),
            (command, Key::G, 4),
            (command | Modifiers::SHIFT, Key::F, 5),
            (command, Key::A, 6),
            (command, Key::I, 7),
            (command, Key::M, 8),
        ] {
            if ctx.input_mut(|i| i.consume_key(modifiers, key)) {
                match action {
                    0 => self.open_size_dialog(true),
                    1 => self.open_size_dialog(false),
                    2 => self.crop(),
                    3 => self.edit("Rotate 90° Clockwise", Document::rotate_cw),
                    4 => self.edit("Rotate 90° Counterclockwise", Document::rotate_ccw),
                    5 => self.edit("Flatten", Document::flatten),
                    6 => self.edit("Select All", |d| {
                        d.selection = Some(Arc::new(vec![255; (d.width * d.height) as usize]))
                    }),
                    7 => self.edit("Invert Selection", Document::invert_selection),
                    8 => self.merge_down(),
                    _ => {}
                }
            }
        }
    }
    pub(super) fn size_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.size_dialog.take() else {
            return;
        };
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("document_size")).show(ctx, |ui| {
            ui.set_min_width(330.);
            ui.heading(if draft.canvas {
                "Canvas Size"
            } else {
                "Resize"
            });
            ui.separator();
            ui.label(format!(
                "Original size: {} × {} pixels",
                draft.original[0], draft.original[1]
            ));
            if !draft.canvas {
                ui.horizontal(|ui| {
                    ui.label("By percentage:");
                    if ui
                        .add(
                            egui::DragValue::new(&mut draft.percent)
                                .range(0.01..=10000.)
                                .suffix("%"),
                        )
                        .changed()
                    {
                        draft.size.set_by_percent(draft.percent / 100.);
                    }
                });
            }
            if image_size_fields(ui, &mut draft.size) {
                draft.percent = draft.size.pixel_width() / f64::from(draft.original[0]) * 100.;
            }
            ui.separator();
            if draft.canvas {
                ui.horizontal(|ui| {
                    ui.label("Anchor:");
                    egui::Grid::new("canvas_anchor")
                        .spacing(egui::vec2(2., 2.))
                        .show(ui, |ui| {
                            for (i, anchor) in Anchor::ALL.into_iter().enumerate() {
                                let response = ui
                                    .add(
                                        egui::Button::new("")
                                            .selected(draft.anchor == anchor)
                                            .min_size(egui::vec2(26., 26.)),
                                    )
                                    .on_hover_text(
                                        [
                                            "Top Left",
                                            "Top",
                                            "Top Right",
                                            "Left",
                                            "Center",
                                            "Right",
                                            "Bottom Left",
                                            "Bottom",
                                            "Bottom Right",
                                        ][i],
                                    );
                                let center = response.rect.center();
                                let color = ui.visuals().text_color();
                                if i == 4 {
                                    ui.painter().rect_filled(
                                        egui::Rect::from_center_size(center, egui::vec2(5., 5.)),
                                        0.,
                                        color,
                                    );
                                } else {
                                    let direction =
                                        egui::vec2((i % 3) as f32 - 1., (i / 3) as f32 - 1.)
                                            .normalized();
                                    let normal = egui::vec2(-direction.y, direction.x);
                                    let tip = center + direction * 6.;
                                    ui.painter()
                                        .line_segment([center - direction * 5., tip], (1.5, color));
                                    ui.painter().line_segment(
                                        [tip - direction * 4. + normal * 3., tip],
                                        (1.5, color),
                                    );
                                    ui.painter().line_segment(
                                        [tip - direction * 4. - normal * 3., tip],
                                        (1.5, color),
                                    );
                                }
                                if response.clicked() {
                                    draft.anchor = anchor;
                                }
                                if i % 3 == 2 {
                                    ui.end_row();
                                }
                            }
                        });
                });
                egui::ComboBox::from_label("Fill")
                    .selected_text(
                        [
                            "Transparent",
                            "Primary Color",
                            "Secondary Color",
                            "White",
                            "Black",
                        ][draft.fill],
                    )
                    .show_ui(ui, |ui| {
                        for (i, label) in [
                            "Transparent",
                            "Primary Color",
                            "Secondary Color",
                            "White",
                            "Black",
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            ui.selectable_value(&mut draft.fill, i, label);
                        }
                    });
            } else {
                egui::ComboBox::from_label("Resampling")
                    .selected_text(draft.sampler.name())
                    .show_ui(ui, |ui| {
                        for sampler in Resampler::ALL {
                            ui.selectable_value(&mut draft.sampler, sampler, sampler.name());
                        }
                    });
                ui.add_enabled(
                    draft.sampler != Resampler::Nearest,
                    egui::Checkbox::new(&mut draft.linear, "Gamma correction"),
                );
            }
            let [width, height] = draft.size.pixels();
            let valid = crate::document::pixel_count(width, height).is_ok();
            if !valid {
                ui.colored_label(
                    egui::Color32::DARK_RED,
                    "Image exceeds the supported pixel count.",
                );
            }
            ui.horizontal(|ui| {
                accept = ui.add_enabled(valid, egui::Button::new("OK")).clicked()
                    || valid && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                cancel = ui.button("Cancel").clicked()
                    || ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
            });
        });
        if accept {
            let before = self.editor().document.clone();
            let [width, height] = draft.size.pixels();
            let original_resolution = before.resolution();
            let resolution = draft.size.document_resolution();
            // Both native actions compare the horizontal DPU only. Accepting an
            // unchanged dialog must preserve metadata (including a distinct Y
            // DPU) and leave the undo history alone.
            if [before.width, before.height] == [width, height]
                && original_resolution.unit == resolution.unit
                && original_resolution.x == resolution.x
            {
                return;
            }
            let result = if draft.canvas {
                let mut d = before.clone();
                d.canvas_size(
                    width,
                    height,
                    draft.anchor,
                    [
                        [0; 4],
                        self.primary,
                        self.secondary,
                        [255; 4],
                        [0, 0, 0, 255],
                    ][draft.fill],
                )
                .map(|()| d)
            } else if [before.width, before.height] == [width, height] {
                Ok(before.clone())
            } else {
                self.gpu
                    .resize_document(&before, width, height, draft.sampler, draft.linear)
            };
            match result.and_then(|mut d| {
                d.set_resolution(resolution)?;
                self.gpu.check_document(&d)?;
                Ok(d)
            }) {
                Ok(document) => {
                    self.editor_mut().document = document;
                    self.editor_mut().record(
                        if draft.canvas {
                            "Canvas Size"
                        } else {
                            "Resize"
                        },
                        before,
                    );
                    self.clone_anchor = None;
                    self.clone_offset = None;
                    self.refresh = true;
                }
                Err(e) => {
                    self.error = Some(format!("{e:#}"));
                    self.size_dialog = Some(draft);
                }
            }
        } else if !cancel {
            self.size_dialog = Some(draft);
        }
    }
    pub(super) fn paste_dialog(&mut self, ctx: &egui::Context) {
        let Some(draft) = self.paste_dialog.take() else {
            return;
        };
        let mut choice = None;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("paste_size")).show(ctx, |ui| {
            ui.set_min_width(340.);
            ui.heading("Pasted image is larger than the canvas");
            ui.label(format!(
                "Pasted image: {} × {} pixels",
                draft.image.width, draft.image.height
            ));
            ui.label(format!(
                "Canvas: {} × {} pixels",
                self.editor().document.width,
                self.editor().document.height
            ));
            let expand = ui.button("Expand canvas");
            if expand.clicked() || ui.input_mut(|i| i.consume_key(Modifiers::ALT, Key::E)) {
                choice = Some(true);
            }
            ui.label("Enlarge the canvas to fit the pasted image.");
            let keep = ui.button("Keep canvas size");
            if keep.clicked() || ui.input_mut(|i| i.consume_key(Modifiers::ALT, Key::K)) {
                choice = Some(false);
            }
            ui.label("Move the pasted image to choose which part stays visible.");
            let cancel_button = ui.button("Cancel");
            if cancel_button.clicked()
                || ui.input_mut(|i| {
                    i.consume_key(Modifiers::NONE, Key::Escape)
                        || i.consume_key(Modifiers::ALT, Key::C)
                })
            {
                cancel = true;
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                if cancel_button.has_focus() {
                    cancel = true;
                } else {
                    choice = Some(!keep.has_focus());
                }
            }
        });
        if !cancel {
            if let Some(expand) = choice {
                self.commit_paste(draft.image, draft.new_layer, expand);
            } else {
                // Keep the captured bitmap through the modal without cloning its
                // full pixel buffer every frame or rereading a changed clipboard.
                self.paste_dialog = Some(draft);
            }
        }
    }
}
