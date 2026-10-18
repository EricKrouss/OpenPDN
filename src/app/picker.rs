use super::*;
impl PaintApp {
    pub(super) fn change_tool(&mut self, tool: Tool) {
        if tool == self.tool {
            return;
        }
        self.finish_stroke();
        self.previous_tool = self.tool;
        self.tool = tool;
        self.picker_press = None;
    }
    pub(super) fn pick_color(&mut self, point: (i32, i32), secondary: bool, control: bool) {
        if let Some(color) = self.editor().document.sample_color(
            point,
            self.picker_size,
            self.sample_composite || control,
        ) {
            if secondary {
                self.secondary = color;
            } else {
                self.primary = color;
            }
        }
    }
    pub(super) fn picker_options(&mut self, ui: &mut egui::Ui) {
        if self.tool != Tool::Picker {
            return;
        }
        egui::ComboBox::from_id_salt("picker_size")
            .width(90.)
            .selected_text(self.picker_size.name())
            .show_ui(ui, |ui| {
                for size in SampleSize::ALL {
                    ui.selectable_value(&mut self.picker_size, size, size.name());
                }
            });
        ui.checkbox(&mut self.sample_composite, "Sample all layers");
        egui::ComboBox::from_id_salt("picker_behavior")
            .width(170.)
            .selected_text(self.picker_behavior.name())
            .show_ui(ui, |ui| {
                for behavior in PickerBehavior::ALL {
                    ui.selectable_value(&mut self.picker_behavior, behavior, behavior.name());
                }
            });
    }
    pub(super) fn picker_canvas(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        editing: bool,
        point: Option<(i32, i32)>,
        hovered: bool,
    ) {
        if self.tool != Tool::Picker {
            return;
        }
        let control = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        if editing
            && hovered
            && let Some(point) = point
        {
            let radius = (self.picker_size.pixels() - 1) / 2;
            let a = rect.min
                + egui::vec2((point.0 - radius) as f32, (point.1 - radius) as f32) * self.zoom;
            let sample = egui::Rect::from_min_size(
                a,
                egui::Vec2::splat(self.picker_size.pixels() as f32 * self.zoom),
            );
            let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
            painter.rect_stroke(
                sample.expand(0.5),
                0.,
                (1., egui::Color32::WHITE),
                egui::StrokeKind::Outside,
            );
            painter.rect_stroke(
                sample,
                0.,
                (1., egui::Color32::BLACK),
                egui::StrokeKind::Outside,
            );
        }
        if let Some(button) = self.picker_press {
            if editing && let Some(point) = point {
                self.pick_color(point, button == egui::PointerButton::Secondary, control);
            }
            if !ui.input(|i| i.pointer.button_down(button)) {
                self.picker_press = None;
                match self.picker_behavior {
                    PickerBehavior::Stay => {}
                    PickerBehavior::Previous => self.change_tool(self.previous_tool),
                    PickerBehavior::Pencil => self.change_tool(Tool::Pencil),
                }
            }
        }
    }
}
