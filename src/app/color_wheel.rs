use super::*;
use crate::{
    color_wheel::{WheelModifiers, grab_color, wheel_pixel},
    palette::{hsv_to_rgb, rgb_to_hsv},
};
use egui::{Color32, Rect, vec2};
#[derive(Clone, Default)]
struct WheelState {
    other: Pixel,
    hsv: [[u16; 3]; 2],
    locked: Option<[[u16; 3]; 2]>,
    modifiers: WheelModifiers,
    tracking: Option<egui::PointerButton>,
}
impl PaintApp {
    pub(super) fn native_color_wheel(&mut self, ui: &mut egui::Ui) {
        let id = ui.make_persistent_id("reference-color-wheel");
        let (rect, response) = ui.allocate_exact_size(vec2(149., 149.), Sense::click_and_drag());
        let radius = 70.;
        let center = rect.min + vec2(74.5, 74.5);
        let texture_id = egui::Id::new("reference-wheel-texture");
        let texture = ui
            .ctx()
            .data(|d| d.get_temp::<egui::TextureHandle>(texture_id))
            .unwrap_or_else(|| {
                let pixels = (0..149)
                    .flat_map(|y| {
                        (0..149).map(move |x| {
                            let [r, g, b, a] =
                                wheel_pixel([f64::from(x) - 74., f64::from(y) - 74.], radius);
                            Color32::from_rgba_unmultiplied(r, g, b, a)
                        })
                    })
                    .collect();
                let image = egui::ColorImage::new([149, 149], pixels);
                let texture = ui.ctx().load_texture(
                    "Paint.NET wheel math",
                    image,
                    egui::TextureOptions::LINEAR,
                );
                ui.ctx()
                    .data_mut(|d| d.insert_temp(texture_id, texture.clone()));
                texture
            });
        ui.painter().image(
            texture.id(),
            rect,
            Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            Color32::WHITE,
        );
        let mut state = ui
            .ctx()
            .data(|d| d.get_temp::<WheelState>(id))
            .unwrap_or_default();
        let (active, other) = if self.workspace.secondary_selected {
            (self.secondary, self.primary)
        } else {
            (self.primary, self.secondary)
        };
        if active[..3] != self.color_hsv_basis[..3] {
            self.color_hsv = rgb_to_hsv(active);
            self.color_hsv_basis = active;
        }
        state.hsv[0] = self.color_hsv;
        if other[..3] != state.other[..3] {
            state.hsv[1] = rgb_to_hsv(other);
            state.other = other;
        }
        let modifiers = ui.input(|i| WheelModifiers {
            snap: i.modifiers.shift && !i.modifiers.alt,
            lock_hue: i.modifiers.alt,
            lock_radius: (i.modifiers.ctrl || i.modifiers.command) && !i.modifiers.alt,
        });
        if modifiers != state.modifiers {
            if modifiers == WheelModifiers::default() {
                state.locked = None;
            } else {
                state.locked = Some(state.hsv);
            }
            state.modifiers = modifiers;
        }
        if response.hovered() {
            for button in [egui::PointerButton::Primary, egui::PointerButton::Secondary] {
                if ui.input(|i| i.pointer.button_pressed(button)) {
                    state.tracking = Some(button);
                }
            }
        }
        if let Some(button) = state.tracking
            && let Some(position) = ui.input(|i| i.pointer.interact_pos())
        {
            let index = usize::from(button == egui::PointerButton::Secondary);
            let basis = state.locked.unwrap_or(state.hsv)[index];
            let delta = [
                f64::from((position.x - rect.left()).floor()) - 74.,
                f64::from((position.y - rect.top()).floor()) - 74.,
            ];
            state.hsv[index] = grab_color(delta, radius, basis, modifiers);
            let color = if self.workspace.secondary_selected ^ (index == 1) {
                &mut self.secondary
            } else {
                &mut self.primary
            };
            color[..3].copy_from_slice(&hsv_to_rgb(state.hsv[index]));
            if index == 0 {
                self.color_hsv = state.hsv[0];
                self.color_hsv_basis = *color;
            } else {
                state.other = *color;
            }
            if !ui.input(|i| i.pointer.button_down(button)) {
                state.tracking = None;
            }
        }
        if response.hovered() || state.tracking.is_some() {
            let index = usize::from(state.tracking == Some(egui::PointerButton::Secondary));
            let angle = f32::from(state.hsv[index][0]).to_radians();
            let stroke = (1., Color32::from_black_alpha(100));
            if modifiers.lock_hue {
                ui.painter().line_segment(
                    [
                        center,
                        center + vec2(angle.cos(), angle.sin()) * radius as f32,
                    ],
                    stroke,
                );
            }
            if modifiers.lock_radius {
                ui.painter().circle_stroke(
                    center,
                    (f32::from(state.hsv[index][1]) / 100.).powf(2925. / 4095.) * radius as f32,
                    stroke,
                );
            }
            if modifiers.snap {
                for n in 0..24 {
                    let angle = n as f32 * std::f32::consts::TAU / 24.;
                    ui.painter().line_segment(
                        [
                            center + vec2(angle.cos(), angle.sin()) * 65.,
                            center + vec2(angle.cos(), angle.sin()) * 70.,
                        ],
                        stroke,
                    );
                }
            }
        }
        for index in 0..2 {
            if index == 1 && state.tracking != Some(egui::PointerButton::Secondary) {
                continue;
            }
            let [h, s, _] = state.hsv[index];
            let angle = f32::from(h).to_radians();
            let point = center
                + vec2(angle.cos(), angle.sin())
                    * ((f32::from(s) / 100.).powf(2925. / 4095.) * radius as f32);
            let rgb = hsv_to_rgb([h, s, 100]);
            ui.painter()
                .circle_filled(point, 4.5, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
            ui.painter().circle_stroke(point, 3.5, (1., Color32::WHITE));
            ui.painter().circle_stroke(point, 4.5, (1., Color32::BLACK));
        }
        ui.ctx().data_mut(|d| d.insert_temp(id, state));
        response.on_hover_text("Shift: snap hue · Alt: lock hue · Ctrl: lock saturation · Right button: opposite color");
    }
}
