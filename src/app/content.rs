use super::*;
use crate::content::{ShapeKind, ShapeMode, TextAlign};
use crate::line::{CurveMode, LineCapStyle, LineDash};

pub(super) enum ContentKind {
    Text(super::text::TextState),
    Shape {
        start: (f64, f64),
        end: (f64, f64),
        secondary: bool,
    },
    Line {
        points: [[f32; 2]; 4],
        secondary: bool,
    },
}
pub(super) struct LiveContent {
    pub(super) before: Document,
    pub(super) kind: ContentKind,
    drag: Option<ContentDrag>,
    pub(super) press_frame: u64,
    line_drag: Option<LineDrag>,
}
#[derive(Clone, Copy)]
enum LineDragMode {
    Point(usize),
    Move,
    Rotate,
}
#[derive(Clone, Copy)]
struct LineDrag {
    mode: LineDragMode,
    press: [f32; 2],
    points: [[f32; 2]; 4],
    center: [f32; 2],
    secondary: bool,
}
struct ContentDrag {
    press: (f64, f64),
    start: (f64, f64),
    end: (f64, f64),
    resize: bool,
}
impl LiveContent {
    pub(super) fn shape(stroke: Stroke, kind: ShapeKind) -> Self {
        Self {
            before: stroke.before,
            kind: if stroke.tool == Tool::Line {
                ContentKind::Line {
                    points: crate::line::initial_points_with_modifiers(
                        stroke.start,
                        stroke.last,
                        stroke.drawing_modifiers.0,
                        stroke.drawing_modifiers.1,
                    ),
                    secondary: stroke.secondary,
                }
            } else {
                let (start, end) = crate::content::shape_drawing_points(
                    stroke.start,
                    stroke.last,
                    kind,
                    stroke.drawing_modifiers.0,
                    stroke.drawing_modifiers.1,
                );
                ContentKind::Shape {
                    start,
                    end,
                    secondary: stroke.secondary,
                }
            },
            drag: None,
            line_drag: None,
            press_frame: u64::MAX,
        }
    }
}
impl PaintApp {
    pub(super) fn finish_content(&mut self) {
        if let Some(content) = self.content.take() {
            let label = match content.kind {
                ContentKind::Text(_) => "Text",
                ContentKind::Shape { .. } => "Shapes",
                ContentKind::Line { .. } => "Line / Curve",
            };
            if self.editor().document.layers[content.before.active].pixels
                != content.before.layers[content.before.active].pixels
            {
                self.editor_mut().record(label, content.before);
            }
            self.refresh = true;
        }
    }
    pub(super) fn cancel_content(&mut self) {
        if let Some(content) = self.content.take() {
            self.editor_mut().document = content.before;
            self.refresh = true;
        }
    }
    pub(super) fn preview_content(&mut self) {
        let Some(mut content) = self.content.take() else {
            return;
        };
        let mut doc = content.before.clone();
        let result = match &mut content.kind {
            ContentKind::Text(state) => {
                state.layout = self
                    .text_renderer
                    .layout_at_resolution(
                        state.editor.text(),
                        &self.text_options,
                        content.before.resolution(),
                    )
                    .ok();
                self.text_renderer.render(
                    &mut doc,
                    state.editor.text(),
                    (state.position.0, state.render_top()),
                    if state.secondary {
                        self.secondary
                    } else {
                        self.primary
                    },
                    &self.text_options,
                )
            }
            ContentKind::Shape {
                start,
                end,
                secondary,
            } => {
                let (a, b) = if *secondary {
                    (self.secondary, self.primary)
                } else {
                    (self.primary, self.secondary)
                };
                doc.draw_shape_at(*start, *end, a, b, &self.shape_options)
            }
            ContentKind::Line { points, secondary } => doc.draw_curve(
                *points,
                FillSource {
                    style: self.fill_style,
                    foreground: if *secondary {
                        self.secondary
                    } else {
                        self.primary
                    },
                    background: if *secondary {
                        self.primary
                    } else {
                        self.secondary
                    },
                },
                &self.line_options,
            ),
        };
        match result {
            Ok(()) => self.editor_mut().document = doc,
            Err(e) => self.error = Some(format!("Content preview failed: {e:#}")),
        }
        self.content = Some(content);
        self.refresh = true;
    }
    pub(super) fn begin_text(&mut self, position: (i32, i32), secondary: bool) {
        self.finish_stroke();
        self.finish_content();
        self.content = Some(LiveContent {
            before: self.editor().document.clone(),
            kind: ContentKind::Text(super::text::TextState::new(position, secondary)),
            drag: None,
            line_drag: None,
            press_frame: u64::MAX,
        });
        self.preview_content();
    }
    pub(super) fn sync_tool_blend(&mut self) {
        let overwrite = self.tool_blend == ToolBlend::Overwrite;
        let mode = self.tool_blend.mode();
        self.text_options.overwrite = overwrite;
        self.text_options.blend_mode = mode;
        self.shape_options.overwrite = overwrite;
        self.shape_options.blend_mode = mode;
        self.line_options.overwrite = overwrite;
        self.line_options.blend_mode = mode;
        self.gradient_options.overwrite = overwrite;
        self.gradient_options.blend_mode = mode;
    }
    pub(super) fn pattern_options(&mut self, ui: &mut egui::Ui) {
        if !matches!(
            self.tool,
            Tool::Brush | Tool::Bucket | Tool::Line | Tool::Shapes
        ) {
            return;
        }
        let old = self.fill_style;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(18., 18.), Sense::hover());
        crate::icons::paint(ui.painter(), rect, crate::icons::Icon::HatchFill, true);
        response.on_hover_text("Fill style");
        egui::ComboBox::from_id_salt("tool_fill")
            .width(130.)
            .selected_text(match self.fill_style {
                FillStyle::Solid => "Solid color",
                FillStyle::Hatch(h) => h.name(),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.fill_style, FillStyle::Solid, "Solid color");
                for style in HatchStyle::ALL {
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(16., 16.), Sense::hover());
                        for y in 0..8 {
                            for x in 0..8 {
                                let shade = 255 - style.coverage(x, y);
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        rect.min + egui::vec2(x as f32 * 2., y as f32 * 2.),
                                        egui::vec2(2., 2.),
                                    ),
                                    0.,
                                    egui::Color32::from_gray(shade),
                                );
                            }
                        }
                        ui.selectable_value(
                            &mut self.fill_style,
                            FillStyle::Hatch(style),
                            style.name(),
                        );
                    });
                }
            });
        if old != self.fill_style {
            self.line_options.fill_style = self.fill_style;
            self.shape_options.fill_style = self.fill_style;
            self.preview_content();
        }
    }
    pub(super) fn tool_blend_options(&mut self, ui: &mut egui::Ui) {
        if !matches!(
            self.tool,
            Tool::Brush
                | Tool::Pencil
                | Tool::Bucket
                | Tool::CloneStamp
                | Tool::Text
                | Tool::Line
                | Tool::Shapes
                | Tool::Gradient
        ) {
            return;
        }
        let old = self.tool_blend;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(18., 18.), Sense::hover());
        crate::icons::paint(ui.painter(), rect, crate::icons::Icon::ToolBlend, true);
        response.on_hover_text("Blending mode");
        egui::ComboBox::from_id_salt("tool_blend")
            .width(85.)
            .selected_text(self.tool_blend.name())
            .show_ui(ui, |ui| {
                for mode in ToolBlend::choices() {
                    ui.selectable_value(&mut self.tool_blend, mode, mode.name());
                }
            });
        if old != self.tool_blend {
            self.sync_tool_blend();
            self.preview_content();
            if let Some(LiveContent {
                kind: ContentKind::Text(state),
                ..
            }) = &mut self.content
            {
                state.focus = true;
            }
        }
    }
    pub(super) fn content_options(&mut self, ui: &mut egui::Ui) {
        let old_text = self.text_options.clone();
        let old_shape = self.shape_options.clone();
        let old_line = self.line_options.clone();
        if self.tool == Tool::Text {
            egui::ComboBox::from_id_salt("font_family")
                .width(130.)
                .selected_text(&self.text_options.family)
                .show_ui(ui, |ui| {
                    for family in &self.text_renderer.families {
                        ui.selectable_value(&mut self.text_options.family, family.clone(), family);
                    }
                });
            ui.add(
                egui::DragValue::new(&mut self.text_options.size)
                    .range(1. ..=2000.)
                    .suffix(
                        if self.text_options.size_metric == crate::content::FontSizeMetric::Points {
                            " pt"
                        } else {
                            ""
                        },
                    )
                    .speed(0.5),
            );
            for (label, value) in [
                ("B", &mut self.text_options.bold),
                ("I", &mut self.text_options.italic),
                ("U", &mut self.text_options.underline),
                ("S̶", &mut self.text_options.strikeout),
            ] {
                ui.toggle_value(value, label);
            }
            egui::ComboBox::from_id_salt("text_align")
                .width(70.)
                .selected_text(format!("{:?}", self.text_options.align))
                .show_ui(ui, |ui| {
                    for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
                        ui.selectable_value(
                            &mut self.text_options.align,
                            align,
                            format!("{align:?}"),
                        );
                    }
                });
            ui.checkbox(&mut self.text_options.antialias, "Antialias");
            egui::ComboBox::from_id_salt("font_size_metric")
                .selected_text(self.text_options.size_metric.name())
                .show_ui(ui, |ui| {
                    for metric in [
                        crate::content::FontSizeMetric::Points,
                        crate::content::FontSizeMetric::Fixed96Dpi,
                    ] {
                        ui.selectable_value(
                            &mut self.text_options.size_metric,
                            metric,
                            metric.name(),
                        );
                    }
                });
        } else if self.tool == Tool::Line {
            ui.label("Width");
            ui.add(
                egui::DragValue::new(&mut self.line_options.width)
                    .range(1.0..=2000.)
                    .speed(0.1),
            );
            egui::ComboBox::from_id_salt("curve_mode")
                .width(75.)
                .selected_text(format!("{:?}", self.line_options.mode))
                .show_ui(ui, |ui| {
                    for mode in [CurveMode::Spline, CurveMode::Bezier, CurveMode::Straight] {
                        ui.selectable_value(&mut self.line_options.mode, mode, format!("{mode:?}"));
                    }
                });
            for (id, label, value) in [
                ("line_start_cap", "Start", &mut self.line_options.start_cap),
                ("line_end_cap", "End", &mut self.line_options.end_cap),
            ] {
                ui.label(label);
                egui::ComboBox::from_id_salt(id)
                    .width(90.)
                    .selected_text(value.name())
                    .show_ui(ui, |ui| {
                        for cap in LineCapStyle::ALL {
                            ui.selectable_value(value, cap, cap.name());
                        }
                    });
            }
            egui::ComboBox::from_id_salt("line_dash")
                .width(85.)
                .selected_text(self.line_options.dash.name())
                .show_ui(ui, |ui| {
                    for dash in LineDash::ALL {
                        ui.selectable_value(&mut self.line_options.dash, dash, dash.name());
                    }
                });
            ui.checkbox(&mut self.line_options.antialias, "Antialias");
        } else if self.tool == Tool::Shapes {
            egui::ComboBox::from_id_salt("shape_kind")
                .width(155.)
                .selected_text(self.shape_options.kind.name())
                .show_ui(ui, |ui| {
                    for kind in ShapeKind::ALL {
                        ui.selectable_value(&mut self.shape_options.kind, kind, kind.name());
                    }
                });
            egui::ComboBox::from_id_salt("shape_mode")
                .width(95.)
                .selected_text(format!("{:?}", self.shape_options.mode))
                .show_ui(ui, |ui| {
                    for mode in [ShapeMode::Outline, ShapeMode::Fill, ShapeMode::Both] {
                        ui.selectable_value(
                            &mut self.shape_options.mode,
                            mode,
                            format!("{mode:?}"),
                        );
                    }
                });
            ui.label("Width");
            ui.add(
                egui::DragValue::new(&mut self.shape_options.width)
                    .range(1.0..=2000.)
                    .speed(0.1),
            );
            if self.shape_options.kind == ShapeKind::RoundedRectangle {
                ui.label("Radius");
                ui.add(egui::DragValue::new(&mut self.shape_options.radius).range(0. ..=500.));
            }
            ui.label("Angle");
            ui.add(
                egui::DragValue::new(&mut self.shape_options.angle)
                    .range(-180. ..=180.)
                    .suffix("°"),
            );
            ui.checkbox(&mut self.shape_options.dashed, "Dashes");
            ui.checkbox(&mut self.shape_options.antialias, "Antialias");
        }
        if old_line.width != self.line_options.width {
            self.set_brush_width(self.line_options.width);
        } else if old_shape.width != self.shape_options.width {
            self.set_brush_width(self.shape_options.width);
        }
        if old_text != self.text_options
            || old_shape != self.shape_options
            || old_line != self.line_options
        {
            self.preview_content();
            if let Some(LiveContent {
                kind: ContentKind::Text(state),
                ..
            }) = &mut self.content
            {
                state.focus = true;
            }
        }
        if self.content.is_some() && matches!(self.tool, Tool::Text | Tool::Shapes | Tool::Line) {
            if ui.button("Finish").clicked() {
                self.finish_content();
            }
            if ui.button("Cancel").clicked() {
                self.cancel_content();
            }
        }
    }
    /// Drag the center to move, or a shape corner to resize. Events remain captured
    /// outside the image until release, and never begin a second paint stroke.
    pub(super) fn content_handles(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        editing: bool,
    ) -> bool {
        if matches!(
            self.content.as_ref().map(|c| &c.kind),
            Some(ContentKind::Text(_))
        ) {
            return self.text_canvas(ui, rect, editing);
        }
        let Some(mut content) = self.content.take() else {
            return false;
        };
        if let ContentKind::Line { points, .. } = &mut content.kind {
            let zoom = self.zoom;
            let screen = |p: [f32; 2]| rect.min + egui::vec2(p[0], p[1]) * zoom;
            let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
            for pair in points.windows(2) {
                painter.line_segment(
                    [screen(pair[0]), screen(pair[1])],
                    (1., egui::Color32::from_rgba_unmultiplied(20, 135, 235, 120)),
                );
            }
            let mut captured = content.line_drag.is_some();
            let press = pointer_press(ui.ctx());
            let original = *points;
            let bounds = egui::Rect::from_points(&original.map(|p| egui::pos2(p[0], p[1])));
            let center = [bounds.center().x, bounds.center().y];
            let rotation = [center[0], bounds.min.y - 28. / zoom];
            painter.line_segment(
                [screen([center[0], bounds.min.y]), screen(rotation)],
                (1., egui::Color32::from_rgb(20, 135, 235)),
            );
            let handles = original
                .into_iter()
                .enumerate()
                .map(|(i, p)| (p, LineDragMode::Point(i)))
                .chain([
                    (center, LineDragMode::Move),
                    (rotation, LineDragMode::Rotate),
                ]);
            for (p, mode) in handles {
                let handle = egui::Rect::from_center_size(screen(p), egui::vec2(10., 10.));
                if matches!(mode, LineDragMode::Point(_)) {
                    painter.rect_filled(handle, 1., egui::Color32::WHITE);
                    painter.rect_stroke(
                        handle,
                        1.,
                        (1., egui::Color32::BLACK),
                        egui::StrokeKind::Inside,
                    );
                } else {
                    painter.circle_filled(handle.center(), 5., egui::Color32::WHITE);
                    painter.circle_stroke(handle.center(), 5., (1., egui::Color32::BLACK));
                    if matches!(mode, LineDragMode::Move) {
                        for v in [egui::vec2(3., 0.), egui::vec2(0., 3.)] {
                            painter.line_segment(
                                [handle.center() - v, handle.center() + v],
                                (1., egui::Color32::BLACK),
                            );
                        }
                    }
                }
                if editing
                    && !egui::Popup::is_any_open(ui.ctx())
                    && content.press_frame != ui.ctx().cumulative_frame_nr()
                    && press.is_some_and(|(p, _)| handle.expand(3.).contains(p))
                {
                    content.press_frame = ui.ctx().cumulative_frame_nr();
                    ui.ctx().memory_mut(|m| {
                        if let Some(id) = m.focused() {
                            m.surrender_focus(id);
                        }
                    });
                    let (p, button) = press.unwrap();
                    content.line_drag = Some(LineDrag {
                        mode: if button == egui::PointerButton::Secondary {
                            LineDragMode::Rotate
                        } else {
                            mode
                        },
                        press: [(p.x - rect.min.x) / zoom, (p.y - rect.min.y) / zoom],
                        points: original,
                        center,
                        secondary: button == egui::PointerButton::Secondary,
                    });
                    captured = true;
                }
            }
            let mut changed = false;
            if let Some(drag) = content.line_drag {
                if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                    let at = [(p.x - rect.min.x) / zoom, (p.y - rect.min.y) / zoom];
                    let delta = [at[0] - drag.press[0], at[1] - drag.press[1]];
                    let mut next = drag.points;
                    match drag.mode {
                        LineDragMode::Point(index) => {
                            next[index] = [next[index][0] + delta[0], next[index][1] + delta[1]];
                            if ui.input(|i| i.modifiers.shift) && (index == 0 || index == 3) {
                                let anchor = drag.points[3 - index].map(f64::from);
                                next[index] = crate::line::constrain_endpoint(
                                    anchor,
                                    next[index].map(f64::from),
                                )
                                .map(|v| v as f32);
                            }
                        }
                        LineDragMode::Move => {
                            for p in &mut next {
                                p[0] += delta[0];
                                p[1] += delta[1];
                            }
                        }
                        LineDragMode::Rotate => {
                            let mut angle = (at[1] - drag.center[1]).atan2(at[0] - drag.center[0])
                                - (drag.press[1] - drag.center[1])
                                    .atan2(drag.press[0] - drag.center[0]);
                            if ui.input(|i| i.modifiers.shift) {
                                let step = std::f32::consts::PI / 12.;
                                angle = (angle / step).round() * step;
                            }
                            let (sin, cos) = angle.sin_cos();
                            for p in &mut next {
                                let x = p[0] - drag.center[0];
                                let y = p[1] - drag.center[1];
                                *p = [
                                    drag.center[0] + x * cos - y * sin,
                                    drag.center[1] + x * sin + y * cos,
                                ];
                            }
                        }
                    }
                    changed = *points != next;
                    *points = next;
                }
                if !ui.input(|i| {
                    if drag.secondary {
                        i.pointer.secondary_down()
                    } else {
                        i.pointer.primary_down()
                    }
                }) {
                    content.line_drag = None;
                }
            }
            self.content = Some(content);
            if changed {
                self.preview_content();
            }
            return captured;
        }
        let (start, end) = match &content.kind {
            ContentKind::Shape { start, end, .. } => (*start, *end),
            ContentKind::Line { .. } | ContentKind::Text(_) => unreachable!(),
        };
        let screen = |p: (f64, f64)| rect.min + egui::vec2(p.0 as f32, p.1 as f32) * self.zoom;
        let center = ((start.0 + end.0) / 2., (start.1 + end.1) / 2.);
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let press = pointer_press(ui.ctx())
            .filter(|(_, b)| *b == egui::PointerButton::Primary)
            .map(|p| p.0);
        let mut captured = content.drag.is_some();
        let mut changed = false;
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        if start != end {
            painter.rect_stroke(
                egui::Rect::from_two_pos(screen(start), screen(end)),
                0.,
                (1., egui::Color32::from_rgb(20, 135, 235)),
                egui::StrokeKind::Outside,
            );
        }
        for (point, resize) in [(center, false), (end, true)] {
            if resize && start == end {
                continue;
            }
            let handle = egui::Rect::from_center_size(screen(point), egui::vec2(10., 10.));
            painter.rect_filled(handle, 1., egui::Color32::WHITE);
            painter.rect_stroke(
                handle,
                1.,
                (1., egui::Color32::BLACK),
                egui::StrokeKind::Inside,
            );
            if editing
                && !egui::Popup::is_any_open(ui.ctx())
                && press.is_some_and(|p| handle.expand(3.).contains(p))
                && ui.input(|i| i.pointer.primary_pressed())
                && content.press_frame != ui.ctx().cumulative_frame_nr()
            {
                content.press_frame = ui.ctx().cumulative_frame_nr();
                let p = press.unwrap();
                let p = (
                    f64::from((p.x - rect.min.x) / self.zoom),
                    f64::from((p.y - rect.min.y) / self.zoom),
                );
                content.drag = Some(ContentDrag {
                    press: p,
                    start,
                    end,
                    resize,
                });
                captured = true;
            }
        }
        if let Some(ContentDrag {
            press,
            start: a,
            end: b,
            resize,
        }) = content.drag
        {
            if let Some(p) = pointer {
                let dx = f64::from((p.x - rect.min.x) / self.zoom) - press.0;
                let dy = f64::from((p.y - rect.min.y) / self.zoom) - press.1;
                match &mut content.kind {
                    ContentKind::Shape { start, end, .. } => {
                        *start = if resize { a } else { (a.0 + dx, a.1 + dy) };
                        *end = (b.0 + dx, b.1 + dy);
                        if resize && ui.input(|i| i.modifiers.shift) {
                            *end = crate::content::constrain_shape_endpoint(
                                *start,
                                *end,
                                self.shape_options.kind,
                            );
                        }
                    }
                    ContentKind::Line { .. } | ContentKind::Text(_) => unreachable!(),
                }
                changed = true;
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                content.drag = None;
            }
        }
        self.content = Some(content);
        if changed {
            self.preview_content();
        }
        captured
    }
}
