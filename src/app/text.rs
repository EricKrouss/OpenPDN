use super::{content::ContentKind, *};
use crate::{
    content::CanvasTextLayout,
    text::{CanvasText, TextMotion},
};

pub(super) struct TextState {
    pub editor: CanvasText,
    pub position: (i32, i32),
    pub secondary: bool,
    pub focus: bool,
    pub layout: Option<CanvasTextLayout>,
    drag: Option<(egui::Pos2, (i32, i32), egui::PointerButton)>,
    blink: f64,
    input_frame: u64,
}
impl TextState {
    // TextTool anchors the click at the vertical center of the first line.
    pub fn render_top(&self) -> i32 {
        (self.position.1 as f64
            - self
                .layout
                .as_ref()
                .map_or(0., |l| f64::from(l.lines[0].height) / 2.))
        .floor() as i32
    }
    pub fn new(position: (i32, i32), secondary: bool) -> Self {
        Self {
            editor: CanvasText::default(),
            position,
            secondary,
            focus: true,
            layout: None,
            drag: None,
            blink: 0.,
            input_frame: u64::MAX,
        }
    }
}
impl PaintApp {
    pub(super) fn cancel_text_composition(&mut self) -> bool {
        let Some(content::LiveContent {
            kind: ContentKind::Text(state),
            ..
        }) = &mut self.content
        else {
            return false;
        };
        if !state.editor.composing() {
            return false;
        }
        state.editor.cancel_composition();
        self.preview_content();
        true
    }
    pub(super) fn text_canvas(&mut self, ui: &egui::Ui, rect: egui::Rect, editing: bool) -> bool {
        let Some(mut content) = self.content.take() else {
            return false;
        };
        let ContentKind::Text(state) = &mut content.kind else {
            unreachable!()
        };
        let ctx = ui.ctx();
        let id = egui::Id::new("canvas_text_input");
        let now = ui.input(|i| i.time);
        let frame = ctx.cumulative_frame_nr();
        let focused = ctx.memory(|m| m.has_focus(id));
        let mut changed = false;
        let mut interacted = false;
        let request_focus = state.focus
            && editing
            && !egui::Popup::is_any_open(ctx)
            && !ui.input(|i| i.pointer.any_down());
        if request_focus {
            ctx.memory_mut(|m| m.request_focus(id));
            state.focus = false;
            state.blink = now;
        }
        let focused = focused || ctx.memory(|m| m.has_focus(id));
        if editing && focused {
            ctx.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                )
            });
            if state.input_frame != frame {
                state.input_frame = frame;
                let events = ctx.input_mut(|i| {
                    let mut events = Vec::new();
                    i.events.retain(|event| {
                        let take = match event {
                            egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Ime(_) => {
                                true
                            }
                            egui::Event::Key {
                                key, pressed: true, ..
                            } => matches!(
                                key,
                                Key::ArrowLeft
                                    | Key::ArrowRight
                                    | Key::ArrowUp
                                    | Key::ArrowDown
                                    | Key::Home
                                    | Key::End
                                    | Key::Backspace
                                    | Key::Delete
                                    | Key::Enter
                                    | Key::Tab
                            ),
                            _ => false,
                        };
                        if take {
                            events.push(event.clone());
                        }
                        !take
                    });
                    events
                });
                for event in events {
                    let before = state.editor.text().to_owned();
                    match event {
                        egui::Event::Text(text) if !state.editor.composing() => {
                            state.editor.insert(&text)
                        }
                        egui::Event::Paste(text) => state.editor.insert(&text),
                        egui::Event::Ime(egui::ImeEvent::Preedit {
                            text,
                            active_range_chars,
                        }) => state.editor.preedit(&text, active_range_chars),
                        egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                            state.editor.commit(&text)
                        }
                        egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
                            before_chars,
                            after_chars,
                        }) => state.editor.delete_surrounding(before_chars, after_chars),
                        egui::Event::Key { key, modifiers, .. }
                            if !state.editor.composing() && !modifiers.alt =>
                        {
                            let word = modifiers.command || modifiers.ctrl;
                            match key {
                                Key::Enter => state.editor.insert("\n"),
                                Key::Backspace => state.editor.delete(true, word),
                                Key::Delete => state.editor.delete(false, word),
                                Key::ArrowLeft => state.editor.motion(if word {
                                    TextMotion::WordLeft
                                } else {
                                    TextMotion::Left
                                }),
                                Key::ArrowRight => state.editor.motion(if word {
                                    TextMotion::WordRight
                                } else {
                                    TextMotion::Right
                                }),
                                Key::Home => state.editor.motion(if word {
                                    TextMotion::Start
                                } else {
                                    TextMotion::Home
                                }),
                                Key::End => state.editor.motion(if word {
                                    TextMotion::Finish
                                } else {
                                    TextMotion::End
                                }),
                                Key::ArrowUp | Key::ArrowDown => {
                                    if let Some(layout) = &state.layout {
                                        let (line, index) = state.editor.line_cursor();
                                        let (x, y, h) = layout.caret(line, index);
                                        let (line, index) = layout.hit(
                                            x,
                                            y + h * if key == Key::ArrowUp { -0.75 } else { 1.25 },
                                        );
                                        state.editor.set_cursor(line, index);
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                    changed |= before != state.editor.text();
                    interacted = true;
                    if changed {
                        state.layout = self
                            .text_renderer
                            .layout_at_resolution(
                                state.editor.text(),
                                &self.text_options,
                                content.before.resolution(),
                            )
                            .ok();
                    }
                }
            }
        } else if state.editor.composing() {
            state.editor.cancel_composition();
            changed = true;
        }
        let mut captured = state.drag.is_some();
        if let Some(layout) = &state.layout {
            let origin = rect.min
                + egui::vec2(state.position.0 as f32, state.render_top() as f32) * self.zoom;
            let (left, top, right, bottom) = layout.bounds();
            let bounds = egui::Rect::from_min_max(
                origin + egui::vec2(left, top) * self.zoom,
                origin + egui::vec2(right, bottom) * self.zoom,
            );
            let height = layout.lines[0].height;
            let last = layout.lines.last().unwrap();
            let margin = (rect.size() * 0.5).min(egui::vec2(12., 12.));
            let handle_pos = (origin
                + egui::vec2(last.left + last.width, last.top + last.height) * self.zoom
                + egui::vec2(10., 10.))
            .clamp(rect.min + margin, rect.max - margin);
            let handle = egui::Rect::from_center_size(handle_pos, egui::vec2(14., 14.));
            let control = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
            let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
            if !control && state.drag.is_none() {
                painter.circle_filled(handle_pos, 7., egui::Color32::WHITE);
                painter.circle_stroke(handle_pos, 7., (1., egui::Color32::BLACK));
                for axis in [egui::vec2(5., 0.), egui::vec2(0., 5.)] {
                    painter.line_segment(
                        [handle_pos - axis, handle_pos + axis],
                        (1., egui::Color32::BLACK),
                    );
                }
            }
            if editing
                && ui.rect_contains_pointer(rect)
                && !egui::Popup::is_any_open(ctx)
                && content.press_frame != frame
                && let Some((p, button)) = pointer_press(ctx)
            {
                if button == egui::PointerButton::Secondary
                    || (!control && handle.expand(3.).contains(p))
                {
                    state.drag = Some((p, state.position, button));
                    captured = true;
                    content.press_frame = frame;
                } else if bounds.expand(height * self.zoom).contains(p) {
                    let local = (p - origin) / self.zoom;
                    let (line, index) = layout.hit(local.x, local.y);
                    state.editor.set_cursor(line, index);
                    state.focus = true;
                    content.press_frame = frame;
                    captured = true;
                    interacted = true;
                }
            }
            if let Some((press, start, button)) = state.drag {
                if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                    let delta = (p - rect.min) / self.zoom;
                    let press = (press - rect.min) / self.zoom;
                    // TextTool floors both pointer positions before translating.
                    let next = (
                        start
                            .0
                            .saturating_add(delta.x.floor() as i32 - press.x.floor() as i32),
                        start
                            .1
                            .saturating_add(delta.y.floor() as i32 - press.y.floor() as i32),
                    );
                    changed |= next != state.position;
                    state.position = next;
                }
                if !ui.input(|i| i.pointer.button_down(button)) {
                    state.drag = None;
                    state.focus = true;
                }
            }
            let (line, index) = state.editor.line_cursor();
            let (x, y, h) = layout.caret(line, index);
            let cursor_rect = egui::Rect::from_min_size(
                origin + egui::vec2(x, y) * self.zoom,
                egui::vec2(1.5, h * self.zoom),
            );
            // Register an actual focusable widget without an input window.
            ui.interact(cursor_rect, id, Sense::focusable_noninteractive())
                .widget_info(|| {
                    egui::WidgetInfo::text_edit(
                        editing,
                        state.editor.text(),
                        state.editor.text(),
                        "Canvas text",
                    )
                });
            // Interact can surrender focus on the release that started the edit
            // because the canvas, rather than the thin caret, received the click.
            if request_focus {
                ctx.memory_mut(|m| m.request_focus(id));
            }
            if focused && editing {
                if interacted {
                    state.blink = now;
                }
                if !control
                    && state.drag.is_none()
                    && (((now - state.blink) / 0.3) as u64).is_multiple_of(2)
                {
                    painter.rect_filled(cursor_rect, 0., egui::Color32::BLACK);
                }
                ctx.request_repaint_after(std::time::Duration::from_millis(150));
                ui.output_mut(|o| {
                    o.ime = Some(egui::output::IMEOutput {
                        purpose: egui::IMEPurpose::Normal,
                        rect: bounds,
                        cursor_rect,
                        should_interrupt_composition: !editing,
                    })
                });
            }
        }
        self.content = Some(content);
        if changed {
            self.preview_content();
        }
        captured
    }
}
