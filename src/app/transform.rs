use super::*;
use crate::transform::{FloatingSelection, Transform};

pub(super) struct LiveTransform {
    pub floating: FloatingSelection,
    drag: Option<TransformDrag>,
    press_frame: u64,
}
#[derive(Clone, Copy)]
enum DragMode {
    Move,
    Scale([f32; 2]),
    Rotate,
}
#[derive(Clone, Copy)]
struct TransformDrag {
    press: [f32; 2],
    transform: Transform,
    mode: DragMode,
    secondary: bool,
}
impl PaintApp {
    pub(super) fn undo_action(&mut self) {
        if self.transform.is_some() {
            self.cancel_transform();
        } else {
            self.finish_stroke();
            self.editor_mut().undo();
            self.refresh = true;
        }
    }
    pub(super) fn finish_transform(&mut self) {
        if let Some(live) = self.transform.take() {
            self.editor_mut()
                .record(live.floating.label(), live.floating.before);
            self.refresh = true;
        }
    }
    pub(super) fn cancel_transform(&mut self) {
        if let Some(live) = self.transform.take() {
            self.editor_mut().document = live.floating.before;
            self.refresh = true;
        }
    }
    pub(super) fn preview_transform(&mut self) {
        let Some(live) = &self.transform else {
            return;
        };
        match self.gpu.transform_selection(
            &live.floating,
            self.transform_bilinear,
            self.transform_linear,
        ) {
            Ok(document) => {
                self.editor_mut().document = document;
                self.refresh = true;
            }
            Err(e) => self.error = Some(format!("Transform failed: {e:#}")),
        }
    }
    pub(super) fn begin_paste(&mut self, floating: FloatingSelection, preview: Document) {
        self.transform = Some(LiveTransform {
            floating,
            drag: None,
            press_frame: u64::MAX,
        });
        self.tool = Tool::MovePixels;
        self.editor_mut().document = preview;
        self.refresh = true;
    }
    pub(super) fn transform_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        if self.transform.is_none() || ctx.egui_wants_keyboard_input() {
            return false;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)) {
            self.cancel_transform();
            return true;
        }
        // The original MoveTool commits and deselects for both Enter and Escape.
        if ctx.input_mut(|i| {
            i.consume_key(Modifiers::NONE, Key::Enter)
                || i.consume_key(Modifiers::NONE, Key::Escape)
        }) {
            self.finish_transform();
            self.edit("Deselect", |d| d.selection = None);
            return true;
        }
        let shift = ctx.input(|i| i.modifiers.shift);
        let mut delta = [0.; 2];
        for (key, axis, direction) in [
            (Key::ArrowLeft, 0, -1.),
            (Key::ArrowRight, 0, 1.),
            (Key::ArrowUp, 1, -1.),
            (Key::ArrowDown, 1, 1.),
        ] {
            if ctx.input_mut(|i| {
                i.consume_key(
                    if shift {
                        Modifiers::SHIFT
                    } else {
                        Modifiers::NONE
                    },
                    key,
                )
            }) {
                delta[axis] += direction * if shift { 10. } else { 1. };
            }
        }
        if delta != [0.; 2] {
            for (p, d) in self
                .transform
                .as_mut()
                .unwrap()
                .floating
                .transform
                .center
                .iter_mut()
                .zip(delta)
            {
                *p += d;
            }
            self.preview_transform();
            return true;
        }
        false
    }
    pub(super) fn transform_options(&mut self, ui: &mut egui::Ui) {
        if !matches!(self.tool, Tool::MovePixels | Tool::MoveSelection) {
            return;
        }
        let mut changed = false;
        if self.tool == Tool::MovePixels {
            egui::ComboBox::from_id_salt("move_sampler")
                .width(90.)
                .selected_text(if self.transform_bilinear {
                    "Bilinear"
                } else {
                    "Nearest Neighbor"
                })
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(&mut self.transform_bilinear, false, "Nearest Neighbor")
                        .changed();
                    changed |= ui
                        .selectable_value(&mut self.transform_bilinear, true, "Bilinear")
                        .changed();
                });
            changed |= ui
                .checkbox(&mut self.transform_linear, "Linear gamma")
                .changed();
        }
        if let Some(live) = &mut self.transform {
            for (label, value) in ["X", "Y"]
                .into_iter()
                .zip(live.floating.transform.center.iter_mut())
            {
                ui.label(label);
                changed |= ui
                    .add(
                        egui::DragValue::new(value)
                            .range(-100000. ..=100000.)
                            .speed(0.5),
                    )
                    .changed();
            }
            for (label, value) in ["W%", "H%"]
                .into_iter()
                .zip(live.floating.transform.scale.iter_mut())
            {
                ui.label(label);
                let mut percent = *value * 100.;
                if ui
                    .add(
                        egui::DragValue::new(&mut percent)
                            .range(-100000. ..=100000.)
                            .speed(1.),
                    )
                    .changed()
                {
                    *value = if percent < 0. {
                        percent.min(-0.1)
                    } else {
                        percent.max(0.1)
                    } / 100.;
                    changed = true;
                }
            }
            ui.label("Angle");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut live.floating.transform.angle)
                        .range(-360. ..=360.)
                        .suffix("°")
                        .speed(0.5),
                )
                .changed();
            if ui.button("Finish").clicked() {
                self.finish_transform();
            }
            if ui.button("Cancel").clicked() {
                self.cancel_transform();
            }
        }
        if changed {
            self.preview_transform();
        }
    }
    pub(super) fn transform_handles(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        editing: bool,
    ) -> bool {
        if !matches!(self.tool, Tool::MovePixels | Tool::MoveSelection) {
            return false;
        }
        let zoom = self.zoom;
        let point = |p: egui::Pos2| [(p.x - rect.min.x) / zoom, (p.y - rect.min.y) / zoom];
        let press = pointer_press(ui.ctx());
        if self.transform.is_none()
            && editing
            && press.is_some()
            && !egui::Popup::is_any_open(ui.ctx())
        {
            let leave_copy = ui.input(|i| i.modifiers.command);
            match FloatingSelection::from_selection(
                &self.editor().document,
                self.tool == Tool::MovePixels,
                leave_copy,
            ) {
                Ok(Some(floating)) => {
                    self.transform = Some(LiveTransform {
                        floating,
                        drag: None,
                        press_frame: u64::MAX,
                    })
                }
                Ok(None) => return true,
                Err(e) => {
                    self.error = Some(format!("{e:#}"));
                    return true;
                }
            }
        }
        let Some(mut live) = self.transform.take() else {
            return false;
        };
        let t = live.floating.transform;
        let size = [live.floating.width, live.floating.height];
        let screen = |p: [f32; 2]| rect.min + egui::vec2(p[0], p[1]) * zoom;
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        let corners = live.floating.corners().map(screen);
        for i in 0..4 {
            painter.line_segment(
                [corners[i], corners[(i + 1) % 4]],
                (1., egui::Color32::from_rgb(20, 135, 235)),
            );
        }
        let mut handles = Vec::new();
        for uv in [
            [0., 0.],
            [0.5, 0.],
            [1., 0.],
            [1., 0.5],
            [1., 1.],
            [0.5, 1.],
            [0., 1.],
            [0., 0.5],
        ] {
            handles.push((
                screen(t.point([uv[0] * size[0] as f32, uv[1] * size[1] as f32], size)),
                DragMode::Scale(uv),
            ));
        }
        let top = (corners[0] + corners[1].to_vec2()) * 0.5;
        let (sin, cos) = t.angle.to_radians().sin_cos();
        let rotate = top + egui::vec2(sin, -cos) * 24.;
        painter.line_segment([top, rotate], (1., egui::Color32::from_rgb(20, 135, 235)));
        handles.push((rotate, DragMode::Rotate));
        let mut hit = None;
        for (p, mode) in handles {
            let r = egui::Rect::from_center_size(p, egui::vec2(8., 8.));
            painter.rect_filled(r, 0., egui::Color32::WHITE);
            painter.rect_stroke(r, 0., (1., egui::Color32::BLACK), egui::StrokeKind::Inside);
            if press.is_some_and(|(p, _)| r.expand(4.).contains(p)) {
                hit = Some(mode);
            }
        }
        let mut changed = false;
        if editing
            && !egui::Popup::is_any_open(ui.ctx())
            && live.press_frame != ui.ctx().cumulative_frame_nr()
            && let Some((p, b)) = press
        {
            live.press_frame = ui.ctx().cumulative_frame_nr();
            ui.ctx().memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
            live.drag = Some(TransformDrag {
                press: point(p),
                transform: t,
                mode: if b == egui::PointerButton::Secondary {
                    DragMode::Rotate
                } else {
                    hit.unwrap_or(DragMode::Move)
                },
                secondary: b == egui::PointerButton::Secondary,
            });
        }
        if let Some(drag) = live.drag {
            if let Some(p) = ui.input(|i| i.pointer.interact_pos()).map(point) {
                let mut next = drag.transform;
                match drag.mode {
                    DragMode::Move => {
                        for (axis, coordinate) in p.into_iter().enumerate() {
                            next.center[axis] += coordinate - drag.press[axis];
                        }
                    }
                    DragMode::Rotate => {
                        let angle =
                            |v: [f32; 2]| (v[1] - next.center[1]).atan2(v[0] - next.center[0]);
                        next.angle += (angle(p) - angle(drag.press)).to_degrees();
                        if ui.input(|i| i.modifiers.shift) {
                            next.angle = (next.angle / 15.).round() * 15.;
                        }
                    }
                    DragMode::Scale(uv) => {
                        let opposite =
                            [(1. - uv[0]) * size[0] as f32, (1. - uv[1]) * size[1] as f32];
                        let anchor = drag.transform.point(opposite, size);
                        let (sin, cos) = next.angle.to_radians().sin_cos();
                        let delta = [p[0] - anchor[0], p[1] - anchor[1]];
                        let local = [
                            cos * delta[0] + sin * delta[1],
                            -sin * delta[0] + cos * delta[1],
                        ];
                        for axis in 0..2 {
                            if uv[axis] != 0.5 {
                                let scale =
                                    local[axis] / ((2. * uv[axis] - 1.) * size[axis] as f32);
                                next.scale[axis] = if scale < 0. {
                                    scale.clamp(-1000., -0.001)
                                } else {
                                    scale.clamp(0.001, 1000.)
                                };
                            }
                        }
                        if ui.input(|i| i.modifiers.shift) {
                            let axis = if uv[0] == 0.5 {
                                1
                            } else if uv[1] == 0.5
                                || (next.scale[0] / drag.transform.scale[0] - 1.).abs()
                                    >= (next.scale[1] / drag.transform.scale[1] - 1.).abs()
                            {
                                0
                            } else {
                                1
                            };
                            next.scale[1 - axis] = drag.transform.scale[1 - axis]
                                * (next.scale[axis] / drag.transform.scale[axis]).abs();
                        }
                        let local = [
                            (opposite[0] - size[0] as f32 * 0.5) * next.scale[0],
                            (opposite[1] - size[1] as f32 * 0.5) * next.scale[1],
                        ];
                        next.center = [
                            anchor[0] - cos * local[0] + sin * local[1],
                            anchor[1] - sin * local[0] - cos * local[1],
                        ];
                    }
                }
                changed = next != live.floating.transform;
                live.floating.transform = next;
            }
            if !ui.input(|i| {
                if drag.secondary {
                    i.pointer.secondary_down()
                } else {
                    i.pointer.primary_down()
                }
            }) {
                live.drag = None;
            }
        }
        let captured = press.is_some() || live.drag.is_some();
        self.transform = Some(live);
        if changed {
            self.preview_transform();
        }
        captured
    }
}
