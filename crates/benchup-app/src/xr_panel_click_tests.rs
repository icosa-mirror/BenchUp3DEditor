//! Replay controller-style panel events through the real shared editor UI.
use super::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct Storage(BTreeMap<String, String>);
impl eframe::Storage for Storage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }
    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }
    fn flush(&mut self) {}
}

struct Replay {
    h: Harness,
    storage: Storage,
    retry: bool,
    clicked: Option<egui::Id>,
    text: Option<String>,
}
impl Replay {
    fn new(retry: bool) -> Self {
        let mut h = Harness::new();
        h.size = egui::vec2(960., 640.);
        Self {
            h,
            storage: Storage::default(),
            retry,
            clicked: None,
            text: None,
        }
    }

    fn frame(&mut self, mut events: Vec<egui::Event>) -> egui::FullOutput {
        self.h.time += 0.01;
        self.clicked = None;
        // This matches Editor::update_ui, including the explicit modifier event.
        events.insert(0, egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        let ctx = self.h.ctx.clone();
        ctx.run_test_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.h.size)),
                focused: true,
                time: Some(self.h.time),
                events,
                ..Default::default()
            },
            |ui| {
                self.h
                    .app
                    .ui_with_host(ui, &mut crate::app_host::UiHost::OpenXr(&mut self.storage));
                if let Some(text) = &mut self.text {
                    egui::Window::new("Replay text input")
                        .fixed_pos(egui::pos2(300., 300.))
                        .show(&ctx, |ui| {
                            ui.add(
                                egui::TextEdit::singleline(text)
                                    .id(egui::Id::unique("xr.click_replay.field")),
                            );
                        });
                }
                if let Some(id) = ctx.interaction_snapshot(|snapshot| snapshot.clicked) {
                    self.clicked = Some(id);
                }
                if self.retry && ctx.current_pass_index() == 0 {
                    ctx.request_discard("XR click replay layout retry");
                }
            },
        )
    }

    fn button(&mut self, pos: Pos2, pressed: bool) -> egui::FullOutput {
        self.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ])
    }

    fn click(&mut self, pos: Pos2) {
        self.button(pos, true);
        self.button(pos, false);
    }
}

#[test]
fn xr_panel_click_switches_real_dropdowns_with_and_without_layout_retry() {
    for retry in [false, true] {
        let mut replay = Replay::new(retry);
        replay.frame(vec![]);
        let output = replay.frame(vec![]);
        let targets =
            ["File", "Edit", "View", "Tools", "Help"].map(|label| text_position(&output, label));
        for (label, pos) in ["File", "Edit", "View", "Tools", "Help"]
            .into_iter()
            .zip(targets)
        {
            // Deliberately move and press together, without a separate hover pass.
            replay.click(pos);
            let id = replay
                .clicked
                .unwrap_or_else(|| panic!("No click on {label}; retry={retry}"));
            let response = replay.h.ctx.read_response(id).unwrap();
            let popup = egui::Popup::menu(&response).get_id();
            assert!(
                egui::Popup::is_id_open(&replay.h.ctx, popup),
                "{label} did not open on one click; retry={retry}"
            );
        }
    }
}

#[test]
fn xr_panel_click_after_text_focus_opens_real_dropdown_on_first_click() {
    let mut replay = Replay::new(false);
    replay.text = Some("Draft".into());
    replay.frame(vec![]);
    let output = replay.frame(vec![]);
    let file = text_position(&output, "File");
    // Use an existing text widget in a floating egui window; no native input or GPU.
    let field_id = egui::Id::unique("xr.click_replay.field");
    let ctx = replay.h.ctx.clone();
    ctx.memory_mut(|memory| memory.request_focus(field_id));
    replay.frame(vec![]);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(field_id));
    replay.click(file);
    assert!(replay.clicked.is_some());
    assert!(egui::Popup::is_any_open(&ctx));
}
