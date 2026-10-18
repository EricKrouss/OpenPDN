use super::*;
use crate::resolution::MeasurementUnit;

impl PaintApp {
    pub(super) fn selection_options_ui(&mut self, ui: &mut egui::Ui) {
        if self.tool != Tool::RectangleSelect {
            return;
        }
        let response = egui::ComboBox::from_id_salt("selection_draw_mode")
            .width(112.)
            .selected_text(format!("     {}", self.selection_options.mode.name()))
            .show_ui(ui, |ui| {
                for mode in SelectionDrawMode::ALL {
                    ui.selectable_value(&mut self.selection_options.mode, mode, mode.name());
                }
            });
        let rect = egui::Rect::from_center_size(
            response.response.rect.left_center() + egui::vec2(11., 0.),
            egui::vec2(16., 16.),
        );
        crate::icons::paint(
            ui.painter(),
            rect,
            match self.selection_options.mode {
                SelectionDrawMode::Normal => crate::icons::Icon::RectangleSelect,
                SelectionDrawMode::FixedRatio => crate::icons::Icon::SelectionRatio,
                SelectionDrawMode::FixedSize => crate::icons::Icon::SelectionSize,
            },
            true,
        );
        if self.selection_options.mode != SelectionDrawMode::Normal {
            ui.label("Width:");
            ui.add(
                egui::DragValue::new(&mut self.selection_options.width)
                    .range(0.001..=1e6)
                    .speed(1.),
            );
            ui.label("Height:");
            ui.add(
                egui::DragValue::new(&mut self.selection_options.height)
                    .range(0.001..=1e6)
                    .speed(1.),
            );
            if super::workspace::icon(ui, "swap-colors", "Swap width and height", false, true)
                .clicked()
            {
                std::mem::swap(
                    &mut self.selection_options.width,
                    &mut self.selection_options.height,
                );
            }
            if self.selection_options.mode == SelectionDrawMode::FixedSize {
                egui::ComboBox::from_id_salt("selection_units")
                    .selected_text(self.selection_options.units.name())
                    .show_ui(ui, |ui| {
                        for unit in MeasurementUnit::ALL {
                            ui.selectable_value(
                                &mut self.selection_options.units,
                                unit,
                                unit.name(),
                            );
                        }
                    });
            }
        }
    }
    pub(super) fn finish_selection(&mut self, stroke: &Stroke) {
        let fixed = stroke.tool == Tool::RectangleSelect
            && stroke.selection_options.mode == SelectionDrawMode::FixedSize;
        let moved = stroke.points.len() > 1;
        let quick = stroke.began.elapsed().as_secs_f64() <= 0.050;
        let empty = self
            .editor()
            .document
            .selection
            .as_ref()
            .is_none_or(|m| m.iter().all(|&p| p == 0));
        let replace = stroke.selection_mode == SelectionMode::Replace;
        if replace
            && ((!fixed && (!moved || quick))
                || empty
                || fixed && self.editor().document.selection == stroke.before.selection)
        {
            self.editor_mut().document.selection = None;
        } else if !replace && !fixed && !moved {
            self.editor_mut().document.selection =
                if stroke.selection_mode == SelectionMode::Intersect && empty {
                    None
                } else {
                    stroke.before.selection.clone()
                };
        } else if empty {
            self.editor_mut().document.selection = None;
        }
    }
}
