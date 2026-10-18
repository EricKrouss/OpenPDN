//! Keyboard details recovered from Tool.OnKeyPress and ToolConfigStrip in 5.1.12.
use super::*;
use crate::content::ShapeKind;
use crate::line::{LineCapStyle, LineDash};

#[derive(Default)]
pub(super) struct ToolInput {
    last_switch: Option<f64>,
    space_pan: bool,
    pan_drag: Option<(egui::PointerButton, egui::Pos2)>,
    pan_press_handled: bool,
}
impl Tool {
    fn hot_key(self) -> Key {
        match self {
            Self::RectangleSelect | Self::LassoSelect | Self::EllipseSelect | Self::MagicWand => {
                Key::S
            }
            Self::MovePixels | Self::MoveSelection => Key::M,
            Self::Line | Self::Shapes => Key::O,
            Self::Brush => Key::B,
            Self::Pencil => Key::P,
            Self::Eraser => Key::E,
            Self::Bucket => Key::F,
            Self::Picker => Key::K,
            Self::Pan => Key::H,
            Self::Zoom => Key::Z,
            Self::Text => Key::T,
            Self::Gradient => Key::G,
            Self::CloneStamp => Key::L,
            Self::Recolor => Key::R,
        }
    }
}
impl ToolInput {
    pub(super) fn temporary_pan(&self) -> bool {
        self.space_pan || self.pan_drag.is_some() || self.pan_press_handled
    }
    pub(super) fn pan_canvas(
        &mut self,
        ctx: &egui::Context,
        enabled: bool,
        hovered: bool,
        drawing: bool,
    ) -> Option<Vec2> {
        self.pan_press_handled = false;
        if !enabled {
            self.space_pan = false;
            self.pan_drag = None;
            return None;
        }
        ctx.input(|input| {
            let mut delta = Vec2::ZERO;
            let mut moved = false;
            // Use each native event's position, including press and release.
            // The aggregate pointer position can already be past the press or
            // release when several native messages arrive in the same frame.
            for event in &input.events {
                match event {
                    egui::Event::Key {
                        key: Key::Space,
                        pressed: false,
                        ..
                    } => {
                        self.space_pan = false;
                        if self
                            .pan_drag
                            .is_some_and(|(button, _)| button == egui::PointerButton::Primary)
                        {
                            self.pan_drag = None;
                        }
                    }
                    egui::Event::WindowFocused(false) => {
                        self.space_pan = false;
                        self.pan_drag = None;
                    }
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed: true,
                        ..
                    } if hovered
                        && !drawing
                        && (*button == egui::PointerButton::Middle
                            || self.space_pan && *button == egui::PointerButton::Primary) =>
                    {
                        self.pan_drag = Some((*button, *pos));
                        self.pan_press_handled = true;
                    }
                    egui::Event::PointerMoved(pos) => {
                        if let Some((_, previous)) = &mut self.pan_drag {
                            delta += *pos - *previous;
                            *previous = *pos;
                            moved = true;
                        }
                    }
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed: false,
                        ..
                    } => {
                        if let Some((tracked, previous)) = self.pan_drag
                            && tracked == *button
                        {
                            delta += *pos - previous;
                            moved |= *pos != previous;
                            self.pan_drag = None;
                        }
                    }
                    _ => {}
                }
            }
            if self
                .pan_drag
                .is_some_and(|(button, _)| !input.pointer.button_down(button))
            {
                self.pan_drag = None;
            }
            moved.then_some(delta)
        })
    }

    fn switch(
        &mut self,
        current: Tool,
        key: Key,
        reverse: bool,
        now: f64,
        pointer_pressed: bool,
    ) -> Option<Tool> {
        let mut tools = Tool::ALL;
        if reverse {
            tools.reverse();
        }
        let recent =
            current.hot_key() == key && self.last_switch.is_some_and(|last| now - last <= 2.);
        let start = if recent {
            tools.iter().position(|&tool| tool == current).unwrap() + 1
        } else {
            0
        };
        let next = (0..tools.len())
            .map(|offset| tools[(start + offset) % tools.len()])
            .find(|&tool| {
                tool.hot_key() == key
                    && (tool != current
                        || matches!(
                            tool,
                            Tool::RectangleSelect
                                | Tool::LassoSelect
                                | Tool::EllipseSelect
                                | Tool::MagicWand
                                | Tool::MovePixels
                                | Tool::MoveSelection
                                | Tool::Line
                                | Tool::Shapes
                                | Tool::Text
                                | Tool::Pan
                                | Tool::Zoom
                                | Tool::Bucket
                                | Tool::Gradient
                        ))
            })?;
        self.last_switch = Some(now);
        (!pointer_pressed).then_some(next)
    }
}

fn cycle<T: Copy + PartialEq>(value: T, choices: &[T], reverse: bool) -> T {
    let index = choices.iter().position(|&choice| choice == value).unwrap();
    choices[(index + if reverse { choices.len() - 1 } else { 1 }) % choices.len()]
}
fn change_width(value: f32, increment: bool, coarse: bool) -> f32 {
    let step = if coarse { 5. } else { 1. };
    (value + if increment { step } else { -step }).clamp(1., 2000.)
}

impl PaintApp {
    pub(super) fn set_brush_width(&mut self, value: f32) {
        self.brush_size = value.clamp(1., 2000.);
        self.line_options.width = self.brush_size;
        self.shape_options.width = self.brush_size;
    }
    pub(super) fn tool_input_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Space))
            && self.stroke.is_none()
            && !ctx.input(|input| input.pointer.any_down())
        {
            self.input.space_pan = true;
        }
        // Keep native input order and repeat events. A fast burst of S/S/S in
        // one frame must cycle three times, including an exact two-second gap.
        let events: Vec<_> = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        for (key, modifiers) in events {
            if modifiers.alt {
                continue;
            }
            let command = modifiers.command || modifiers.ctrl;
            let drawing = self.stroke.is_some() || ctx.input(|input| input.pointer.any_down());
            let mut preview = false;
            let handled = if drawing
                && matches!(self.tool, Tool::Line | Tool::Shapes)
                && matches!(
                    key,
                    Key::C | Key::X | Key::A | Key::Comma | Key::Period | Key::Slash
                )
                && !command
            {
                true
            } else if !command && matches!(key, Key::C | Key::X) {
                if key == Key::C {
                    self.workspace.secondary_selected = !self.workspace.secondary_selected;
                } else {
                    std::mem::swap(&mut self.primary, &mut self.secondary);
                    preview = true;
                }
                true
            } else if !modifiers.shift
                && matches!(key, Key::OpenBracket | Key::CloseBracket)
                && matches!(
                    self.tool,
                    Tool::Brush
                        | Tool::Eraser
                        | Tool::CloneStamp
                        | Tool::Recolor
                        | Tool::Line
                        | Tool::Shapes
                )
            {
                if !drawing {
                    self.set_brush_width(change_width(
                        self.brush_size,
                        key == Key::CloseBracket,
                        command,
                    ));
                    preview = true;
                }
                true
            } else if !command && !drawing && self.tool == Tool::Shapes && key == Key::A {
                self.shape_options.kind =
                    cycle(self.shape_options.kind, &ShapeKind::ALL, modifiers.shift);
                preview = true;
                true
            } else if !command
                && !modifiers.shift
                && !drawing
                && self.tool == Tool::Line
                && matches!(key, Key::Comma | Key::Period | Key::Slash)
            {
                match key {
                    Key::Comma => {
                        self.line_options.start_cap =
                            cycle(self.line_options.start_cap, &LineCapStyle::ALL, false)
                    }
                    Key::Period => {
                        self.line_options.dash =
                            cycle(self.line_options.dash, &LineDash::ALL, false)
                    }
                    _ => {
                        self.line_options.end_cap =
                            cycle(self.line_options.end_cap, &LineCapStyle::ALL, false)
                    }
                }
                preview = true;
                true
            } else if !command && Tool::ALL.iter().any(|tool| tool.hot_key() == key) {
                if let Some(next) = self.input.switch(
                    self.tool,
                    key,
                    modifiers.shift,
                    ctx.input(|input| input.time),
                    drawing,
                ) {
                    self.change_tool(next);
                }
                true
            } else {
                false
            };
            if handled {
                ctx.input_mut(|input| {
                    input.consume_key(modifiers, key);
                });
            }
            if preview {
                self.preview_content();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timed_tool_keys_reset_and_reverse_in_reference_order() {
        let mut input = ToolInput::default();
        assert_eq!(
            input.switch(Tool::Brush, Key::S, false, 0., false),
            Some(Tool::RectangleSelect)
        );
        assert_eq!(
            input.switch(Tool::RectangleSelect, Key::S, false, 0.5, false),
            Some(Tool::LassoSelect)
        );
        assert_eq!(
            input.switch(Tool::LassoSelect, Key::S, false, 2.5, false),
            Some(Tool::EllipseSelect)
        );
        assert_eq!(
            input.switch(Tool::EllipseSelect, Key::S, false, 4.50001, false),
            Some(Tool::RectangleSelect)
        );
        assert_eq!(
            input.switch(Tool::RectangleSelect, Key::S, true, 5., false),
            Some(Tool::MagicWand)
        );
        assert_eq!(
            input.switch(Tool::Brush, Key::S, true, 5.5, false),
            Some(Tool::MagicWand)
        );
        assert_eq!(
            input.switch(Tool::Shapes, Key::O, false, 10., false),
            Some(Tool::Line)
        );
        assert_eq!(
            input.switch(Tool::Line, Key::O, true, 11., false),
            Some(Tool::Shapes)
        );
        assert_eq!(
            input.switch(Tool::MoveSelection, Key::M, true, 20., false),
            Some(Tool::MoveSelection)
        );
        assert_eq!(
            input.switch(Tool::MoveSelection, Key::M, true, 21., false),
            Some(Tool::MovePixels)
        );
        assert_eq!(input.switch(Tool::Brush, Key::B, false, 22., false), None);
        assert_eq!(
            input.switch(Tool::Text, Key::T, false, 23., false),
            Some(Tool::Text)
        );
        assert_eq!(
            input.switch(Tool::Pan, Key::H, false, 24., false),
            Some(Tool::Pan)
        );
        assert_eq!(
            input.switch(Tool::Zoom, Key::Z, false, 25., false),
            Some(Tool::Zoom)
        );
        assert_eq!(
            input.switch(Tool::Bucket, Key::F, false, 26., false),
            Some(Tool::Bucket)
        );
        assert_eq!(
            input.switch(Tool::Gradient, Key::G, false, 27., false),
            Some(Tool::Gradient)
        );
    }
    #[test]
    fn ignored_tool_key_during_a_stroke_still_refreshes_the_cycle_timer() {
        let mut input = ToolInput::default();
        assert_eq!(
            input.switch(Tool::RectangleSelect, Key::S, false, 0., false),
            Some(Tool::RectangleSelect)
        );
        assert_eq!(
            input.switch(Tool::RectangleSelect, Key::S, false, 1., true),
            None
        );
        assert_eq!(
            input.switch(Tool::RectangleSelect, Key::S, false, 2.5, false),
            Some(Tool::LassoSelect)
        );
    }
    #[test]
    fn tool_option_cycles_follow_original_numeric_enum_order() {
        assert_eq!(
            cycle(LineCapStyle::Flat, &LineCapStyle::ALL, false),
            LineCapStyle::Arrow
        );
        assert_eq!(
            cycle(LineCapStyle::ArrowFilled, &LineCapStyle::ALL, false),
            LineCapStyle::Rounded
        );
        assert_eq!(
            cycle(LineCapStyle::Rounded, &LineCapStyle::ALL, false),
            LineCapStyle::Flat
        );
        assert_eq!(
            cycle(LineDash::DashDotDot, &LineDash::ALL, false),
            LineDash::Solid
        );
        assert_eq!(
            cycle(ShapeKind::Heart, &ShapeKind::ALL, false),
            ShapeKind::Rectangle
        );
        assert_eq!(
            cycle(ShapeKind::Rectangle, &ShapeKind::ALL, true),
            ShapeKind::Heart
        );
    }
    #[test]
    fn width_shortcuts_preserve_fractions_and_clamp_like_float_setting() {
        assert_eq!(change_width(2.5, true, false), 3.5);
        assert_eq!(change_width(2.5, true, true), 7.5);
        assert_eq!(change_width(2.5, false, true), 1.);
        assert_eq!(change_width(1997.5, true, true), 2000.);
    }
    #[test]
    fn temporary_pan_uses_pointer_motion_and_stops_on_space_or_button_release() {
        let ctx = egui::Context::default();
        let mut input = ToolInput {
            space_pan: true,
            ..Default::default()
        };
        let space = |pressed| egui::Event::Key {
            key: Key::Space,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let button = |button, pressed, x, y| egui::Event::PointerButton {
            pos: egui::pos2(x, y),
            button,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let mut frame = |events, enabled, drawing| {
            let mut delta = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    delta = input.pan_canvas(ui.ctx(), enabled, true, drawing);
                },
            );
            output.textures_delta.clear();
            (delta, input.temporary_pan())
        };
        assert_eq!(
            frame(
                vec![
                    space(true),
                    button(egui::PointerButton::Primary, true, 10., 10.)
                ],
                true,
                false
            ),
            (None, true)
        );
        assert_eq!(
            frame(
                vec![egui::Event::PointerMoved(egui::pos2(30., 25.))],
                true,
                false
            ),
            (Some(egui::vec2(20., 15.)), true)
        );
        assert_eq!(frame(vec![space(false)], true, false), (None, false));
        frame(
            vec![button(egui::PointerButton::Primary, false, 30., 25.)],
            true,
            false,
        );
        assert_eq!(
            frame(
                vec![button(egui::PointerButton::Middle, true, 40., 40.)],
                true,
                false
            ),
            (None, true)
        );
        assert_eq!(
            frame(
                vec![egui::Event::PointerMoved(egui::pos2(25., 20.))],
                true,
                false
            ),
            (Some(egui::vec2(-15., -20.)), true)
        );
        assert_eq!(
            frame(
                vec![button(egui::PointerButton::Middle, false, 25., 20.)],
                true,
                false
            ),
            (None, false)
        );
        assert_eq!(
            frame(
                vec![button(egui::PointerButton::Middle, true, 40., 40.)],
                true,
                true
            ),
            (None, false)
        );
        assert_eq!(frame(vec![], false, false), (None, false));
        frame(
            vec![button(egui::PointerButton::Middle, false, 40., 40.)],
            true,
            false,
        );
        assert_eq!(
            frame(
                vec![
                    button(egui::PointerButton::Middle, true, 10., 10.),
                    egui::Event::PointerMoved(egui::pos2(20., 25.)),
                    button(egui::PointerButton::Middle, false, 30., 40.),
                ],
                true,
                false
            ),
            (Some(egui::vec2(20., 30.)), true)
        );
        assert_eq!(frame(vec![], true, false), (None, false));
    }
}
