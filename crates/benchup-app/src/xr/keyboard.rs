//! Controller-operated input for the shared editor's text widgets.
use crate::app_host::XrPanelPreferences;
use eframe::egui::{self, Event, Key};

#[derive(Default)]
pub(super) struct Keyboard {
    target: Option<egui::Id>,
    target_modal: Option<egui::LayerId>,
    pending: Vec<Event>,
    rect: Option<egui::Rect>,
}

pub(super) struct FocusGuard {
    ctx: egui::Context,
    previous: egui::SurrenderFocusOn,
}

impl Drop for FocusGuard {
    fn drop(&mut self) {
        self.ctx
            .options_mut(|options| options.input_options.surrender_focus_on = self.previous);
    }
}

impl Keyboard {
    // Must run before editor widgets inspect this frame's pointer events. A key
    // with non-focusable Sense still counts as a click elsewhere for TextEdit.
    // Preserve focus only over the keyboard; normal clicks elsewhere keep their
    // normal focus behavior. The guard restores the host policy after UI layout.
    pub(super) fn protect_focus(&self, ctx: &egui::Context) -> Option<FocusGuard> {
        let over_keyboard = ctx.input(|input| {
            input.focused
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|pos| self.rect.is_some_and(|rect| rect.contains(pos)))
        });
        if self.target.is_none()
            || !over_keyboard
            || self.target_modal != ctx.memory(|memory| memory.top_modal_layer())
        {
            return None;
        }
        let previous = ctx.options_mut(|options| {
            std::mem::replace(
                &mut options.input_options.surrender_focus_on,
                egui::SurrenderFocusOn::Never,
            )
        });
        Some(FocusGuard {
            ctx: ctx.clone(),
            previous,
        })
    }

    pub(super) fn height(visible: bool) -> f32 {
        if visible { 290.0 } else { 44.0 }
    }

    // A pointer/placement cancellation is not a keyboard-focus transition.
    // Keep a completed virtual key queued while the host remains focused.
    pub(super) fn cancel_input(&mut self, ctx: &egui::Context, focused: bool) {
        if !focused {
            self.take_events(ctx, false);
        }
    }

    pub(super) fn take_events(&mut self, ctx: &egui::Context, focused: bool) -> Vec<Event> {
        if !focused {
            self.target = None;
            self.pending.clear();
        }
        if !self.pending.is_empty() {
            if let Some(target) = self.target {
                ctx.memory_mut(|memory| {
                    // Re-requesting active focus resets TextEdit's event filter,
                    // turning caret arrows into global focus navigation.
                    if memory.focused() != Some(target) {
                        memory.request_focus(target);
                    }
                });
            }
        }
        std::mem::take(&mut self.pending)
    }

    // Called before editor widgets process this frame's controller click. TextEdit
    // may surrender focus when a key is clicked; retain only that active field,
    // never an arbitrary previously focused toolbar or a dismissed dialog.
    pub(super) fn remember_target(&mut self, ctx: &egui::Context) {
        let modal = ctx.memory(|memory| memory.top_modal_layer());
        if self.target_modal != modal {
            self.target = None;
        }
        if ctx.output(|output| output.ime.is_some()) {
            self.target = ctx.memory(|memory| memory.focused());
            self.target_modal = modal;
        } else if !ctx.input(|input| {
            input
                .pointer
                .interact_pos()
                .is_some_and(|pos| self.rect.is_some_and(|rect| rect.contains(pos)))
        }) {
            self.target = None;
        }
    }

    fn text(&mut self, text: &str) {
        self.pending.push(Event::Text(text.to_owned()));
    }

    fn key(&mut self, key: Key) {
        for pressed in [true, false] {
            self.pending.push(Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }

    pub(super) fn show(&mut self, ctx: &egui::Context, preferences: &mut XrPanelPreferences) {
        let id = egui::Id::unique("benchup.xr.keyboard");
        let layer = egui::LayerId::new(egui::Order::Foreground, id);
        if let Some(modal) = ctx.memory(|memory| memory.top_modal_layer()) {
            ctx.set_sublayer(modal, layer);
            if self.target_modal != Some(modal) {
                self.target = None;
            }
        }
        ctx.move_to_top(layer);
        let height = Self::height(preferences.keyboard_visible);
        let screen = ctx.content_rect();
        let response = egui::Area::new(id)
            .order(egui::Order::Foreground)
            .movable(false)
            .fixed_pos(egui::pos2(screen.left(), screen.bottom() - height))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .inner_margin(6)
                    .show(ui, |ui| {
                        ui.set_width((screen.width() - 12.0).max(200.0));
                        ui.horizontal(|ui| {
                            if button(
                                ui,
                                if preferences.keyboard_visible {
                                    "Hide keyboard"
                                } else {
                                    "Keyboard"
                                },
                                150.0,
                                30.0,
                            ) {
                                preferences.keyboard_visible = !preferences.keyboard_visible;
                            }
                            if preferences.keyboard_visible {
                                ui.label(if self.target.is_some() {
                                    "Typing into selected field"
                                } else {
                                    "Select a text or numeric field"
                                });
                            }
                        });
                        if !preferences.keyboard_visible {
                            return;
                        }
                        ui.add_enabled_ui(self.target.is_some(), |ui| {
                            let rows = if preferences.keyboard_symbols {
                                ["1234567890", "!@#$%^&*()", "[]{}<>/\\", "_+=:;?\"'"]
                            } else {
                                ["1234567890", "qwertyuiop", "asdfghjkl", "zxcvbnm,.-"]
                            };
                            let width =
                                ((ui.available_width() - 9.0 * ui.spacing().item_spacing.x) / 10.0)
                                    .max(24.0);
                            for row in rows {
                                ui.horizontal(|ui| {
                                    for character in row.chars() {
                                        let character = if preferences.keyboard_caps {
                                            character.to_ascii_uppercase()
                                        } else {
                                            character
                                        };
                                        let label = character.to_string();
                                        if button(ui, &label, width, 42.0) {
                                            self.text(&label);
                                        }
                                    }
                                });
                            }
                            ui.horizontal(|ui| {
                                if button(
                                    ui,
                                    if preferences.keyboard_caps {
                                        "CAPS"
                                    } else {
                                        "Caps"
                                    },
                                    width,
                                    42.0,
                                ) {
                                    preferences.keyboard_caps = !preferences.keyboard_caps;
                                }
                                if button(
                                    ui,
                                    if preferences.keyboard_symbols {
                                        "ABC"
                                    } else {
                                        "Symbols"
                                    },
                                    width * 1.3,
                                    42.0,
                                ) {
                                    preferences.keyboard_symbols = !preferences.keyboard_symbols;
                                }
                                if button(ui, "Space", width * 2.0, 42.0) {
                                    self.text(" ");
                                }
                                for (label, key, factor) in [
                                    ("Left", Key::ArrowLeft, 0.8),
                                    ("Right", Key::ArrowRight, 0.8),
                                    ("Backspace", Key::Backspace, 1.5),
                                    ("Enter", Key::Enter, 1.2),
                                ] {
                                    if button(ui, label, width * factor, 42.0) {
                                        self.key(key);
                                    }
                                }
                            });
                        });
                    });
            });
        self.rect = Some(response.response.rect);
        if !self.pending.is_empty() {
            ctx.request_repaint();
        }
    }
}

fn button(ui: &mut egui::Ui, label: &str, width: f32, height: f32) -> bool {
    // A keyboard key must not take the target TextEdit's keyboard focus.
    ui.add_sized(
        [width, height],
        egui::Button::new(egui::RichText::new(label).size(18.0)).sense(egui::Sense::CLICK),
    )
    .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        keyboard: &mut Keyboard,
        preferences: &mut XrPanelPreferences,
        text: &mut String,
        mut events: Vec<Event>,
        focus: bool,
        modal: bool,
    ) -> egui::FullOutput {
        events.extend(keyboard.take_events(ctx, true));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| {
                let _focus_guard = keyboard.protect_focus(ctx);
                let field = |ui: &mut egui::Ui| {
                    let response =
                        ui.add(egui::TextEdit::singleline(text).id(egui::Id::unique("test.field")));
                    if focus {
                        response.request_focus();
                    }
                };
                if modal {
                    egui::Modal::new(egui::Id::unique("test.modal")).show(ctx, field);
                } else {
                    egui::CentralPanel::default().show(ui, field);
                }
                keyboard.remember_target(ctx);
                keyboard.show(ctx, preferences);
            },
        );
        output.textures_delta.clear();
        output
    }

    fn text_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
                _ => None,
            }
        }
        output
            .shapes
            .iter()
            .find_map(|shape| find(&shape.shape, label))
            .expect("visible keyboard key")
    }

    #[test]
    fn controller_keys_keep_text_field_focus_in_editor_and_modal() {
        for modal in [false, true] {
            let ctx = egui::Context::default();
            let mut keyboard = Keyboard::default();
            let mut preferences = XrPanelPreferences {
                keyboard_visible: true,
                ..Default::default()
            };
            let mut text = String::new();
            frame(
                &ctx,
                &mut keyboard,
                &mut preferences,
                &mut text,
                vec![],
                true,
                modal,
            );
            let output = frame(
                &ctx,
                &mut keyboard,
                &mut preferences,
                &mut text,
                vec![],
                false,
                modal,
            );
            assert!(keyboard.target.is_some());
            for (label, expected) in [
                ("q", "q"),
                ("w", "qw"),
                ("Left", "qw"),
                ("e", "qew"),
                ("Right", "qew"),
                ("Backspace", "qe"),
            ] {
                let output = frame(
                    &ctx,
                    &mut keyboard,
                    &mut preferences,
                    &mut text,
                    vec![],
                    false,
                    modal,
                );
                let pos = text_position(&output, label);
                frame(
                    &ctx,
                    &mut keyboard,
                    &mut preferences,
                    &mut text,
                    vec![Event::PointerMoved(pos)],
                    false,
                    modal,
                );
                for pressed in [true, false] {
                    frame(
                        &ctx,
                        &mut keyboard,
                        &mut preferences,
                        &mut text,
                        vec![
                            Event::PointerMoved(pos),
                            Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        false,
                        modal,
                    );
                    assert_eq!(
                        ctx.memory(|memory| memory.focused()),
                        Some(egui::Id::unique("test.field")),
                        "field lost focus during keyboard click: modal={modal}, key={label}, pressed={pressed}"
                    );
                }
                frame(
                    &ctx,
                    &mut keyboard,
                    &mut preferences,
                    &mut text,
                    vec![],
                    false,
                    modal,
                );
                assert_eq!(text, expected, "modal={modal}, key={label}");
            }
            // Leaving the keyboard must still let an ordinary outside click
            // release field focus; the temporary policy must not escape layout.
            assert_eq!(
                ctx.options(|options| options.input_options.surrender_focus_on),
                egui::SurrenderFocusOn::Clicks
            );
            let pos = egui::pos2(700., 400.);
            for pressed in [true, false] {
                frame(
                    &ctx,
                    &mut keyboard,
                    &mut preferences,
                    &mut text,
                    vec![
                        Event::PointerMoved(pos),
                        Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    false,
                    modal,
                );
            }
            assert_ne!(
                ctx.memory(|memory| memory.focused()),
                Some(egui::Id::unique("test.field"))
            );
            drop(output);
        }
    }

    #[test]
    fn live_placement_cancellation_keeps_completed_text_but_focus_loss_discards_it() {
        for modal in [false, true] {
            for focused in [false, true] {
                let ctx = egui::Context::default();
                let mut keyboard = Keyboard::default();
                let mut preferences = XrPanelPreferences { keyboard_visible: true, ..Default::default() };
                let mut text = String::new();
                frame(&ctx, &mut keyboard, &mut preferences, &mut text, vec![], true, modal);
                frame(&ctx, &mut keyboard, &mut preferences, &mut text, vec![], false, modal);
                assert!(keyboard.target.is_some());
                // A completed key click waits for the next editor UI pass.
                keyboard.text("a");
                keyboard.cancel_input(&ctx, focused);
                assert_eq!(keyboard.target.is_some(), focused);
                frame(&ctx, &mut keyboard, &mut preferences, &mut text, vec![], false, modal);
                assert_eq!(text, if focused { "a" } else { "" }, "modal={modal}, focused={focused}");
                if focused {
                    assert_eq!(ctx.memory(|memory| memory.focused()), Some(egui::Id::unique("test.field")));
                    keyboard.text("b");
                    frame(&ctx, &mut keyboard, &mut preferences, &mut text, vec![], false, modal);
                    assert_eq!(text, "ab", "continued typing must not need refocusing");
                }
            }
        }
    }

    #[test]
    fn tracking_loss_discards_queued_text_and_target() {
        let ctx = egui::Context::default();
        let mut keyboard = Keyboard {
            target: Some(egui::Id::unique("old.field")),
            ..Default::default()
        };
        keyboard.text("x");
        assert!(keyboard.take_events(&ctx, false).is_empty());
        assert!(keyboard.target.is_none());
    }
}
