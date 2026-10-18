//! Reference-derived adjustment controls and transactional GPU previews.
use super::*;
use crate::icons::{self, Icon};
use egui::Color32;
pub(super) struct EffectDialog {
    before: Document,
    op: Adjustment,
    values: [f32; 8],
    defaults: [f32; 8],
    enabled: [bool; 8],
    linked: bool,
    preview: bool,
    tonal: super::tonal::TonalControls,
    center_image: Option<egui::TextureHandle>,
    pending: Option<u64>,
    valid: bool,
    accept_requested: bool,
}
fn feature_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        let font = egui::TextStyle::Button.resolve(ui.style());
        let galley = ui
            .painter()
            .layout_no_wrap(label.to_owned(), font, ui.visuals().text_color());
        let size = egui::vec2(ui.available_width().max(galley.size().x + 26.), 18.);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        if response.hovered() || response.has_focus() {
            ui.painter()
                .rect_filled(rect, 2., ui.visuals().widgets.hovered.bg_fill);
        }
        let icon_rect = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 9., rect.center().y),
            egui::vec2(17., 17.),
        );
        icons::paint(ui.painter(), icon_rect, icon, enabled);
        ui.painter().galley(
            egui::pos2(rect.left() + 24., rect.center().y - galley.size().y * 0.5),
            galley,
            ui.visuals().text_color(),
        );
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        response
    })
    .inner
}
impl PaintApp {
    pub(super) fn adjustments_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(210.);
        for (label, icon, op) in [
            ("Auto-Level", Icon::AutoLevel, Some(Adjustment::AutoLevel)),
            (
                "Black and White",
                Icon::BlackWhite,
                Some(Adjustment::BlackWhite),
            ),
            (
                "Brightness / Contrast…",
                Icon::BrightnessContrast,
                Some(Adjustment::BrightnessContrast),
            ),
            ("Curves…", Icon::Curves, Some(Adjustment::Curves)),
            ("Exposure…", Icon::Exposure, Some(Adjustment::Exposure)),
            (
                "Highlights / Shadows…",
                Icon::HighlightsShadows,
                Some(Adjustment::HighlightsShadows),
            ),
            (
                "Hue / Saturation…",
                Icon::HueSaturation,
                Some(Adjustment::HueSaturation),
            ),
            (
                "Invert Alpha",
                Icon::InvertAlpha,
                Some(Adjustment::InvertAlpha),
            ),
            (
                "Invert Colors",
                Icon::InvertColors,
                Some(Adjustment::InvertColors),
            ),
            ("Levels…", Icon::Levels, Some(Adjustment::Levels)),
            ("Posterize…", Icon::Posterize, Some(Adjustment::Posterize)),
            ("Sepia…", Icon::Sepia, Some(Adjustment::Sepia)),
            (
                "Temperature and Tint…",
                Icon::TemperatureTint,
                Some(Adjustment::TemperatureTint),
            ),
        ] {
            if feature_button(ui, icon, label, op.is_some()).clicked() {
                if let Some(op) = op {
                    if matches!(
                        op,
                        Adjustment::Posterize | Adjustment::Levels | Adjustment::Curves
                    ) || !op.controls().is_empty()
                    {
                        self.begin_effect(op);
                    } else {
                        self.adjustment(op);
                    }
                }
                ui.close();
            }
        }
    }
    pub(super) fn effects_menu(&mut self, ui: &mut egui::Ui) {
        use Icon::*;
        if ui
            .add_enabled(
                self.last_effect.is_some(),
                egui::Button::new("Repeat Last Effect     Ctrl+F"),
            )
            .clicked()
        {
            self.repeat_effect();
            ui.close();
        }
        ui.separator();
        for (category, items) in [
            (
                "Artistic",
                &[
                    ("Ink Sketch", InkSketch),
                    ("Oil Painting", OilPainting),
                    ("Pencil Sketch", PencilSketch),
                ][..],
            ),
            (
                "Blurs",
                &[
                    ("Bokeh", Bokeh),
                    ("Fragment", Fragment),
                    ("Gaussian Blur", GaussianBlur),
                    ("Median", Median),
                    ("Motion Blur", MotionBlur),
                    ("Radial Blur", RadialBlur),
                    ("Sketch Blur", SketchBlur),
                    ("Square Blur", SquareBlur),
                    ("Surface Blur", SurfaceBlur),
                    ("Zoom Blur", ZoomBlur),
                ][..],
            ),
            ("Color", &[("Quantize", Quantize)][..]),
            (
                "Distort",
                &[
                    ("Bulge", Bulge),
                    ("Crystalize", Crystalize),
                    ("Dents", Dents),
                    ("Frosted Glass", FrostedGlass),
                    ("Morphology", Morphology),
                    ("Pixelate", Pixelate),
                    ("Polar Inversion", PolarInversion),
                    ("Tile Reflection", TileReflection),
                    ("Twist", Twist),
                ][..],
            ),
            (
                "Noise",
                &[("Add Noise", AddNoise), ("Reduce Noise", ReduceNoise)][..],
            ),
            ("Object", &[("Drop Shadow", DropShadow)][..]),
            (
                "Photo",
                &[
                    ("Glow", Glow),
                    ("Red Eye Removal", RedEye),
                    ("Sharpen", Sharpen),
                    ("Soften Portrait", SoftenPortrait),
                    ("Straighten", Straighten),
                    ("Vignette", Vignette),
                ][..],
            ),
            (
                "Render",
                &[
                    ("Clouds", Clouds),
                    ("Julia Fractal", Julia),
                    ("Mandelbrot Fractal", Mandelbrot),
                    ("Turbulence", Turbulence),
                ][..],
            ),
            (
                "Stylize",
                &[
                    ("Edge Detect", EdgeDetect),
                    ("Emboss", Emboss),
                    ("Outline", Outline),
                    ("Relief", Relief),
                ][..],
            ),
        ] {
            ui.menu_button(category, |ui| {
                let font = egui::TextStyle::Button.resolve(ui.style());
                let width = items
                    .iter()
                    .map(|(label, _)| {
                        ui.painter()
                            .layout_no_wrap(
                                (*label).to_owned(),
                                font.clone(),
                                ui.visuals().text_color(),
                            )
                            .size()
                            .x
                            + 26.
                    })
                    .fold(0., f32::max);
                ui.set_min_width(width);
                for &(label, icon) in items {
                    let operation = match icon {
                        Vignette => Some(Adjustment::Vignette),
                        Relief => Some(Adjustment::Relief),
                        Emboss => Some(Adjustment::Emboss),
                        Morphology => Some(Adjustment::Morphology),
                        SquareBlur => Some(Adjustment::SquareBlur),
                        GaussianBlur => Some(Adjustment::GaussianBlur),
                        Fragment => Some(Adjustment::Fragment),
                        RadialBlur => Some(Adjustment::RadialBlur),
                        ZoomBlur => Some(Adjustment::ZoomBlur),
                        Bulge => Some(Adjustment::Bulge),
                        Twist => Some(Adjustment::Twist),
                        PolarInversion => Some(Adjustment::PolarInversion),
                        TileReflection => Some(Adjustment::TileReflection),
                        Glow => Some(Adjustment::Glow),
                        PencilSketch => Some(Adjustment::PencilSketch),
                        MotionBlur => Some(Adjustment::MotionBlur),
                        InkSketch => Some(Adjustment::InkSketch),
                        SurfaceBlur => Some(Adjustment::SurfaceBlur),
                        SoftenPortrait => Some(Adjustment::SoftenPortrait),
                        DropShadow => Some(Adjustment::DropShadow),
                        Median => Some(Adjustment::Median),
                        Outline => Some(Adjustment::Outline),
                        OilPainting => Some(Adjustment::OilPainting),
                        ReduceNoise => Some(Adjustment::ReduceNoise),
                        AddNoise => Some(Adjustment::AddNoise),
                        FrostedGlass => Some(Adjustment::FrostedGlass),
                        Bokeh => Some(Adjustment::Bokeh),
                        SketchBlur => Some(Adjustment::SketchBlur),
                        Quantize => Some(Adjustment::Quantize),
                        Crystalize => Some(Adjustment::Crystalize),
                        Dents => Some(Adjustment::Dents),
                        Pixelate => Some(Adjustment::Pixelate),
                        RedEye => Some(Adjustment::RedEye),
                        Sharpen => Some(Adjustment::Sharpen),
                        Straighten => Some(Adjustment::Straighten),
                        Clouds => Some(Adjustment::Clouds),
                        Julia => Some(Adjustment::Julia),
                        Mandelbrot => Some(Adjustment::Mandelbrot),
                        Turbulence => Some(Adjustment::Turbulence),
                        EdgeDetect => Some(Adjustment::EdgeDetect),
                        _ => None,
                    };
                    let response = feature_button(ui, icon, label, operation.is_some());
                    if let Some(op) = operation {
                        if response.clicked() {
                            self.begin_effect(op);
                            ui.close();
                        }
                    } else {
                        response.on_hover_text("Implementation and reference validation pending");
                    }
                }
            });
        }
    }
    pub(super) fn cancel_effect(&mut self) {
        if let Some(effect) = self.effect.take() {
            self.processing.cancel();
            self.editor_mut().document = effect.before;
            self.refresh = true;
        }
    }
    pub(super) fn repeat_effect(&mut self) {
        if let Some((op, values)) = self.last_effect {
            self.finish_stroke();
            match self
                .gpu
                .adjust_parameters(&self.editor().document, op, values)
            {
                Ok(pixels) => {
                    let before = self.editor().document.clone();
                    self.editor_mut().document.layers[before.active].pixels = Arc::new(pixels);
                    self.editor_mut().record(op.name(), before);
                    self.refresh = true;
                }
                Err(e) => self.error = Some(format!("Repeat failed: {e:#}")),
            }
        }
    }
    fn begin_effect(&mut self, op: Adjustment) {
        self.finish_stroke();
        let mut values = op.parameters();
        if matches!(op, Adjustment::AddNoise | Adjustment::FrostedGlass) {
            let seed = self.random_effect_seeds[usize::from(op == Adjustment::FrostedGlass)];
            values[6] = (seed & 65535) as f32;
            values[7] = (seed >> 16) as f32;
        }
        if matches!(op, Adjustment::Crystalize | Adjustment::Dents) {
            values[7] = (self.random_effect_seeds[0] & 255) as f32;
        }
        if op == Adjustment::Clouds {
            for (index, color) in [(4, self.primary), (6, self.secondary)] {
                values[index] = u32::from_be_bytes([0, color[0], color[1], color[2]]) as f32;
                values[index + 1] = f32::from(color[3]);
            }
        }
        self.effect = Some(EffectDialog {
            before: self.editor().document.clone(),
            op,
            values,
            defaults: values,
            enabled: [true; 8],
            linked: true,
            preview: true,
            tonal: Default::default(),
            center_image: None,
            pending: None,
            valid: false,
            accept_requested: false,
        });
        self.preview_effect();
    }
    fn preview_effect(&mut self) {
        let Some(mut effect) = self.effect.take() else {
            return;
        };
        if effect.preview {
            let values = std::array::from_fn(|i| {
                if effect.enabled[i] {
                    effect.values[i]
                } else {
                    0.
                }
            });
            let table = match effect.op {
                Adjustment::Levels => effect.tonal.levels.table().map(|t| Some((t, false))),
                Adjustment::Curves => effect
                    .tonal
                    .curves
                    .table()
                    .map(|t| Some((t, effect.tonal.curves.luminosity))),
                _ => Ok(None),
            };
            effect.valid = false;
            match table {
                Ok(table) => {
                    let gpu = Arc::clone(&self.gpu);
                    let before = effect.before.clone();
                    let op = effect.op;
                    effect.pending = Some(self.processing.submit(move || match table {
                        Some((table, luminosity)) => gpu.transfer(&before, &table, luminosity),
                        None => gpu.adjust_parameters(&before, op, values),
                    }));
                }
                Err(error) => {
                    self.processing.cancel();
                    effect.pending = None;
                    effect.accept_requested = false;
                    self.error = Some(format!("GPU preview failed: {error:#}"));
                }
            }
        } else {
            self.processing.cancel();
            effect.pending = None;
            effect.valid = false;
            self.editor_mut().document = effect.before.clone();
        }
        self.refresh = true;
        self.effect = Some(effect);
    }
    pub(super) fn poll_effect(&mut self) {
        let Some(generation) = self.effect.as_ref().and_then(|e| e.pending) else {
            return;
        };
        let Some(result) = self.processing.poll(generation) else {
            return;
        };
        let mut effect = self.effect.take().unwrap();
        effect.pending = None;
        match result {
            Ok(pixels) => {
                self.editor_mut().document = effect.before.clone();
                self.editor_mut().document.layers[effect.before.active].pixels = Arc::new(pixels);
                effect.valid = true;
                self.refresh = true;
            }
            Err(error) => {
                effect.valid = false;
                effect.accept_requested = false;
                self.error = Some(format!("GPU preview failed: {error:#}"));
            }
        }
        self.effect = Some(effect);
        self.finish_effect_if_ready();
    }
    fn finish_effect_if_ready(&mut self) {
        if !self
            .effect
            .as_ref()
            .is_some_and(|e| e.accept_requested && e.valid && e.pending.is_none())
        {
            return;
        }
        let effect = self.effect.take().unwrap();
        if effect.op.is_effect() {
            self.last_effect = Some((effect.op, effect.values));
        }
        self.editor_mut().record(effect.op.name(), effect.before);
        self.refresh = true;
    }
    pub(super) fn effect_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut effect) = self.effect.take() else {
            return;
        };
        let mut changed = false;
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("image_adjustment")).show(ctx, |ui| {
            ui.set_min_width(300.);
            ui.heading(effect.op.name());
            if matches!(effect.op, Adjustment::Levels | Adjustment::Curves) {
                changed |= effect.tonal.ui(ui, effect.op, &effect.before);
            } else if effect.op == Adjustment::Morphology {
                for (i, label) in ["Width", "Height"].into_iter().enumerate() {
                    if ui
                        .add(
                            egui::Slider::new(&mut effect.values[i], 1. ..=100.)
                                .text(label)
                                .integer(),
                        )
                        .changed()
                    {
                        changed = true;
                        if effect.linked {
                            effect.values[1 - i] = effect.values[i];
                        }
                    }
                }
                if ui.checkbox(&mut effect.linked, "Linked").changed() && effect.linked {
                    effect.values[1] = effect.values[0];
                    changed = true;
                }
                ui.horizontal(|ui| {
                    changed |= ui
                        .selectable_value(&mut effect.values[2], 0., "Dilate")
                        .changed();
                    changed |= ui
                        .selectable_value(&mut effect.values[2], 1., "Erode")
                        .changed();
                });
            } else if effect.op != Adjustment::Posterize {
                for (i, &(label, min, max, integer)) in effect.op.controls().iter().enumerate() {
                    if label == "Seed" {
                        if ui.button("Randomize").clicked() {
                            effect.values[i] = (effect.values[i] + 1.) % 256.;
                            changed = true;
                        }
                        continue;
                    }
                    if label == "Blend Mode" {
                        egui::ComboBox::from_id_salt(("mix", i))
                            .selected_text(crate::gpu::MIX_MODES[effect.values[i] as usize].0)
                            .show_ui(ui, |ui| {
                                for (index, (name, _)) in crate::gpu::MIX_MODES.iter().enumerate() {
                                    changed |= ui
                                        .selectable_value(
                                            &mut effect.values[i],
                                            index as f32,
                                            *name,
                                        )
                                        .changed();
                                }
                            });
                        continue;
                    }
                    let choices: Option<&[&str]> = match label {
                        "Algorithm" => Some(if effect.op == Adjustment::Quantize {
                            &["Median Cut", "Octree"]
                        } else {
                            &["Sobel", "Prewitt"]
                        }),
                        "Sampling" => Some(&["Cubic", "Linear", "Nearest Neighbor"]),
                        "Scale Down" => Some(&[
                            "Anisotropic",
                            "High Quality Cubic",
                            "Multisample Linear",
                            "Cubic",
                            "Linear",
                            "Nearest Neighbor",
                        ]),
                        "Scale Up" => Some(&["Cubic", "Linear", "Nearest Neighbor"]),
                        "Noise" => Some(&["Turbulence", "Fractal Sum"]),
                        _ => None,
                    };
                    if let Some(choices) = choices {
                        ui.horizontal(|ui| {
                            ui.label(label);
                            egui::ComboBox::from_id_salt((label, i))
                                .selected_text(choices[effect.values[i] as usize])
                                .show_ui(ui, |ui| {
                                    for (index, name) in choices.iter().enumerate() {
                                        changed |= ui
                                            .selectable_value(
                                                &mut effect.values[i],
                                                index as f32,
                                                *name,
                                            )
                                            .changed();
                                    }
                                });
                        });
                        continue;
                    }
                    if effect.op == Adjustment::Clouds && label.starts_with("Alpha ") {
                        continue;
                    }
                    if effect.op == Adjustment::Clouds && label.starts_with("Color ") {
                        let rgb = effect.values[i] as u32;
                        let mut color = [
                            (rgb >> 16) as u8,
                            (rgb >> 8) as u8,
                            rgb as u8,
                            effect.values[i + 1] as u8,
                        ];
                        ui.horizontal(|ui| {
                            ui.label(label);
                            if ui
                                .color_edit_button_srgba_unmultiplied(&mut color)
                                .changed()
                            {
                                effect.values[i] =
                                    u32::from_be_bytes([0, color[0], color[1], color[2]]) as f32;
                                effect.values[i + 1] = f32::from(color[3]);
                                changed = true;
                            }
                        });
                        continue;
                    }
                    if matches!(label, "Edge Mode" | "Kernel Shape") {
                        let modes: &[&str] = if label == "Kernel Shape" {
                            &["Circle", "Square"]
                        } else {
                            &["Clamp", "Wrap", "Mirror", "Transparent"]
                        };
                        egui::ComboBox::from_id_salt(label)
                            .selected_text(modes[effect.values[i] as usize])
                            .show_ui(ui, |ui| {
                                for (index, mode) in modes.iter().enumerate() {
                                    changed |= ui
                                        .selectable_value(
                                            &mut effect.values[i],
                                            index as f32,
                                            *mode,
                                        )
                                        .changed();
                                }
                            });
                        continue;
                    }
                    if label == "Color" && effect.op == Adjustment::DropShadow {
                        let color = effect.values[i] as u32;
                        let mut rgb = [(color >> 16) as u8, (color >> 8) as u8, color as u8];
                        ui.horizontal(|ui| {
                            ui.label("Color");
                            if ui.color_edit_button_srgb(&mut rgb).changed() {
                                effect.values[i] =
                                    u32::from_be_bytes([0, rgb[0], rgb[1], rgb[2]]) as f32;
                                changed = true;
                            }
                        });
                        continue;
                    }
                    if matches!(
                        label,
                        "Centered" | "Shadow Only" | "Invert Colors" | "Overlay Edges"
                    ) {
                        let mut centered = effect.values[i] != 0.;
                        if ui.checkbox(&mut centered, label).changed() {
                            effect.values[i] = centered as u8 as f32;
                            changed = true;
                        }
                        continue;
                    }
                    if min == max {
                        ui.label("Quality: High quality");
                        continue;
                    }
                    let mut slider =
                        egui::Slider::new(&mut effect.values[i], min..=max).text(label);
                    if label == "Granularity" {
                        slider = slider.logarithmic(true);
                    }
                    if integer {
                        slider = slider.integer();
                    } else {
                        slider = slider.max_decimals(2);
                    }
                    let edited = ui.add(slider).changed();
                    if edited && effect.op == Adjustment::FrostedGlass && i < 2 {
                        if i == 0 {
                            effect.values[1] = effect.values[1].min(effect.values[0]);
                        } else {
                            effect.values[0] = effect.values[0].max(effect.values[1]);
                        }
                    }
                    changed |= edited;
                }
                if let Some(index) = effect
                    .op
                    .controls()
                    .iter()
                    .position(|control| control.0 == "Center X")
                {
                    if effect.center_image.is_none() {
                        let doc = &effect.before;
                        let bitmap = image::RgbaImage::from_raw(
                            doc.width,
                            doc.height,
                            doc.layers[doc.active]
                                .pixels
                                .iter()
                                .flatten()
                                .copied()
                                .collect(),
                        )
                        .unwrap();
                        let use_selection =
                            !matches!(effect.op, Adjustment::RadialBlur | Adjustment::ZoomBlur);
                        let bounds = if use_selection {
                            doc.selection_bounds()
                                .unwrap_or([0, 0, doc.width, doc.height])
                        } else {
                            [0, 0, doc.width, doc.height]
                        };
                        let clipped = image::imageops::crop_imm(
                            &bitmap, bounds[0], bounds[1], bounds[2], bounds[3],
                        );
                        let thumbnail = image::imageops::thumbnail(&*clipped, 128, 96);
                        let pixels = egui::ColorImage::from_rgba_unmultiplied(
                            [thumbnail.width() as usize, thumbnail.height() as usize],
                            thumbnail.as_raw(),
                        );
                        effect.center_image = Some(ctx.load_texture(
                            "effect_center_preview",
                            pixels,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                    let texture = effect.center_image.as_ref().unwrap();
                    ui.horizontal(|ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(texture.size_vec2(), Sense::click_and_drag());
                        ui.painter().rect_filled(rect, 0., Color32::GRAY);
                        ui.painter().image(
                            texture.id(),
                            rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                            Color32::WHITE,
                        );
                        if (response.clicked() || response.dragged())
                            && let Some(pos) = response.interact_pointer_pos()
                        {
                            let relative = (pos - rect.min) / rect.size() * 2. - egui::vec2(1., 1.);
                            effect.values[index] = relative.x.clamp(-1., 1.);
                            effect.values[index + 1] = relative.y.clamp(-1., 1.);
                            changed = true;
                        }
                        let marker = rect.min
                            + rect.size()
                                * egui::vec2(
                                    (effect.values[index] + 1.) * 0.5,
                                    (effect.values[index + 1] + 1.) * 0.5,
                                );
                        let painter = ui.painter().with_clip_rect(rect);
                        painter.circle_stroke(marker, 5., egui::Stroke::new(2., Color32::WHITE));
                        painter.line_segment(
                            [marker - egui::vec2(8., 0.), marker + egui::vec2(8., 0.)],
                            egui::Stroke::new(1., Color32::BLACK),
                        );
                        painter.line_segment(
                            [marker - egui::vec2(0., 8.), marker + egui::vec2(0., 8.)],
                            egui::Stroke::new(1., Color32::BLACK),
                        );
                        ui.label("Center");
                    });
                }
            } else {
                for (i, label) in ["Red", "Green", "Blue", "Alpha"].into_iter().enumerate() {
                    ui.horizontal(|ui| {
                        changed |= ui.checkbox(&mut effect.enabled[i], label).changed();
                        let response = ui.add_enabled(
                            effect.enabled[i],
                            egui::Slider::new(&mut effect.values[i], 2.0..=64.0).integer(),
                        );
                        if response.changed() {
                            changed = true;
                            if effect.linked {
                                let value = effect.values[i];
                                effect.values[..4].fill(value);
                            }
                        }
                    });
                }
                if ui.checkbox(&mut effect.linked, "Linked").changed() && effect.linked {
                    let value = effect.values[0];
                    effect.values[..4].fill(value);
                    changed = true;
                }
            }
            if effect.op == Adjustment::RedEye {
                ui.label("Select the eyes before applying this effect.");
            }
            changed |= ui
                .checkbox(
                    &mut effect.preview,
                    if effect.pending.is_some() {
                        "Preview (rendering…)"
                    } else {
                        "Preview"
                    },
                )
                .changed();
            ui.horizontal(|ui| {
                if ui.button("Reset").clicked() {
                    effect.values = effect.defaults;
                    effect.enabled = [true; 8];
                    effect.linked = true;
                    effect.tonal = Default::default();
                    changed = true;
                }
                accept = ui
                    .add_enabled(!effect.accept_requested, egui::Button::new("OK"))
                    .clicked();
                cancel = ui.button("Cancel").clicked();
            });
            cancel |= ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
            accept |= ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        });
        if cancel {
            self.processing.cancel();
            self.editor_mut().document = effect.before;
            self.refresh = true;
        } else if accept {
            let needs_preview =
                changed || !effect.preview || (!effect.valid && effect.pending.is_none());
            effect.preview = true;
            effect.accept_requested = true;
            self.effect = Some(effect);
            if needs_preview {
                self.preview_effect();
            }
            self.finish_effect_if_ready();
        } else {
            if changed {
                effect.accept_requested = false;
            }
            self.effect = Some(effect);
            if changed {
                self.preview_effect();
            }
        }
    }
}
