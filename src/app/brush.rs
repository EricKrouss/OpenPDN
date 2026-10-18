use super::*;
fn glyph(ui: &mut egui::Ui, icon: crate::icons::Icon) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(16., 16.), egui::Sense::hover());
    crate::icons::paint(ui.painter(), rect, icon, true);
}
impl PaintApp {
    pub(super) fn brush_controls(&mut self, ui: &mut egui::Ui) {
        if !matches!(
            self.tool,
            Tool::Brush | Tool::Eraser | Tool::CloneStamp | Tool::Recolor
        ) {
            return;
        }
        ui.separator();
        glyph(
            ui,
            match self.brush_options.shape {
                BrushShape::Circle => crate::icons::Icon::BrushCircle,
                BrushShape::Square => crate::icons::Icon::BrushSquare,
            },
        );
        egui::ComboBox::from_id_salt("brush-tip")
            .selected_text(match self.brush_options.shape {
                BrushShape::Circle => "Circle",
                BrushShape::Square => "Square",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.brush_options.shape, BrushShape::Circle, "Circle");
                ui.selectable_value(&mut self.brush_options.shape, BrushShape::Square, "Square");
            });
        glyph(ui, crate::icons::Icon::BrushHardness);
        ui.label("Hardness:");
        ui.add_enabled(
            self.brush_options.antialiased,
            egui::DragValue::new(&mut self.brush_options.hardness)
                .range(0. ..=1.)
                .speed(0.01)
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.))
                .custom_parser(|s| {
                    s.trim_end_matches('%')
                        .parse::<f64>()
                        .ok()
                        .map(|v| v / 100.)
                }),
        );
        glyph(ui, crate::icons::Icon::BrushSpacing);
        ui.label("Spacing:");
        ui.add(
            egui::DragValue::new(&mut self.brush_options.spacing)
                .range(0.01..=5.)
                .speed(0.01)
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.))
                .custom_parser(|s| {
                    s.trim_end_matches('%')
                        .parse::<f64>()
                        .ok()
                        .map(|v| v / 100.)
                }),
        );
        glyph(ui, crate::icons::Icon::BrushAntialias);
        ui.checkbox(&mut self.brush_options.antialiased, "Antialiasing");
        glyph(ui, crate::icons::Icon::BrushSmoothing);
        ui.checkbox(&mut self.brush_options.smoothing, "Smoothing");
        glyph(ui, crate::icons::Icon::BrushPressure);
        ui.checkbox(&mut self.brush_options.pressure, "Pressure")
            .on_hover_text("Use pressure when supplied by the input device");
    }
}
