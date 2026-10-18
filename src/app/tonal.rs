use super::*;
use crate::tonal::{Curves, Levels, histogram, spline};

#[derive(Default)]
pub(super) struct TonalControls {
    pub levels: Levels,
    pub curves: Curves,
    channel: usize,
    dragging: Option<usize>,
    histogram: Option<Box<[[u64; 256]; 3]>>,
}
impl TonalControls {
    pub(super) fn ui(&mut self, ui: &mut egui::Ui, op: Adjustment, before: &Document) -> bool {
        let mut changed = false;
        if op == Adjustment::Curves {
            ui.horizontal(|ui| {
                changed |= ui
                    .selectable_value(&mut self.curves.luminosity, true, "Luminosity")
                    .changed();
                changed |= ui
                    .selectable_value(&mut self.curves.luminosity, false, "RGB")
                    .changed();
            });
        }
        let luminosity = op == Adjustment::Curves && self.curves.luminosity;
        if !luminosity {
            egui::ComboBox::from_id_salt("tonal_channel")
                .selected_text(["Red", "Green", "Blue", "RGB"][self.channel])
                .show_ui(ui, |ui| {
                    for (i, label) in ["Red", "Green", "Blue", "RGB"].into_iter().enumerate() {
                        ui.selectable_value(&mut self.channel, i, label);
                    }
                });
        }
        let channel = if luminosity { 0 } else { self.channel.min(2) };
        let hist = self
            .histogram
            .get_or_insert_with(|| Box::new(histogram(before)));
        let size = if op == Adjustment::Curves {
            egui::vec2(256., 256.)
        } else {
            egui::vec2(256., 90.)
        };
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter();
        painter.rect_filled(rect, 0., egui::Color32::from_gray(245));
        let max = hist[channel].iter().copied().max().unwrap_or(1).max(1) as f32;
        for (i, &v) in hist[channel].iter().enumerate() {
            let x = rect.left() + i as f32;
            let h = v as f32 / max * rect.height();
            painter.line_segment(
                [
                    egui::pos2(x, rect.bottom()),
                    egui::pos2(x, rect.bottom() - h),
                ],
                (1., egui::Color32::from_gray(210)),
            );
        }
        for i in 0..=4 {
            let t = i as f32 / 4.;
            let x = rect.left() + rect.width() * t;
            let y = rect.top() + rect.height() * t;
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                (0.5, egui::Color32::GRAY),
            );
            painter.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                (0.5, egui::Color32::GRAY),
            );
        }
        if op == Adjustment::Levels {
            ui.label("Input");
            for (label, values, range) in [
                ("Black", &mut self.levels.input_black, 0. ..=1.),
                ("White", &mut self.levels.input_white, 0. ..=1.),
                ("Gamma", &mut self.levels.gamma, 0.1..=10.),
                ("Output black", &mut self.levels.output_black, 0. ..=1.),
                ("Output white", &mut self.levels.output_white, 0. ..=1.),
            ] {
                let mut value = values[channel];
                let slider = egui::Slider::new(&mut value, range)
                    .text(label)
                    .max_decimals(3);
                if ui.add(slider).changed() {
                    if self.channel == 3 {
                        *values = [value; 3];
                    } else {
                        values[channel] = value;
                    }
                    changed = true;
                }
            }
            if ui.button("Auto").clicked() {
                self.levels = Levels::auto(before);
                changed = true;
            }
        } else {
            let points = &mut self.curves.points[channel];
            let to_screen = |p: (u8, u8)| {
                egui::pos2(
                    rect.left() + p.0 as f32 / 255. * rect.width(),
                    rect.bottom() - p.1 as f32 / 255. * rect.height(),
                )
            };
            if let Ok(table) = spline(points, 256) {
                let line: Vec<_> = table
                    .iter()
                    .enumerate()
                    .map(|(x, y)| {
                        egui::pos2(
                            rect.left() + x as f32,
                            rect.bottom() - y.clamp(0., 1.) * rect.height(),
                        )
                    })
                    .collect();
                painter.add(egui::Shape::line(
                    line,
                    (1.5, egui::Color32::from_rgb(30, 95, 190)),
                ));
            }
            for &point in points.iter() {
                painter.circle_filled(to_screen(point), 4., egui::Color32::WHITE);
                painter.circle_stroke(to_screen(point), 4., (1., egui::Color32::BLACK));
            }
            if let Some(pos) = response.interact_pointer_pos() {
                let nearest = points
                    .iter()
                    .enumerate()
                    .find(|(_, p)| to_screen(**p).distance(pos) < 10.)
                    .map(|(i, _)| i);
                if response.secondary_clicked()
                    && let Some(i) = nearest
                    && points.len() > 2
                {
                    points.remove(i);
                    changed = true;
                }
                if response.clicked() || response.drag_started() {
                    self.dragging = nearest;
                    if self.dragging.is_none() {
                        let x = ((pos.x - rect.left()) / rect.width() * 255.)
                            .round()
                            .clamp(0., 255.) as u8;
                        let y = ((rect.bottom() - pos.y) / rect.height() * 255.)
                            .round()
                            .clamp(0., 255.) as u8;
                        if !points.iter().any(|p| p.0 == x) {
                            points.push((x, y));
                            points.sort_by_key(|p| p.0);
                            self.dragging = points.iter().position(|p| p.0 == x);
                            changed = true;
                        }
                    }
                }
                if (response.dragged() || response.clicked())
                    && let Some(i) = self.dragging
                {
                    let x = ((pos.x - rect.left()) / rect.width() * 255.)
                        .round()
                        .clamp(0., 255.) as u8;
                    let y = ((rect.bottom() - pos.y) / rect.height() * 255.)
                        .round()
                        .clamp(0., 255.) as u8;
                    let lo = if i == 0 { 0 } else { points[i - 1].0 + 1 };
                    let hi = if i + 1 == points.len() {
                        255
                    } else {
                        points[i + 1].0 - 1
                    };
                    points[i] = (x.clamp(lo, hi), y);
                    changed = true;
                }
            }
            if response.drag_stopped() {
                self.dragging = None;
            }
            if self.channel == 3 && !luminosity && changed {
                let points = points.clone();
                self.curves.points = [points.clone(), points.clone(), points];
            }
            ui.label("Click to add; drag to edit; right-click to remove.");
        }
        changed
    }
}
