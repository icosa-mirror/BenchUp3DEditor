//! Project-owned inspection views, shared by desktop and the XR panel.
use super::*;
use crate::inspector_table::{Column, InspectorTable};
use benchup_core::{InspectionView, InspectionViewId, InspectionViewpoint};

enum Action {
    Save,
    Recall(InspectionViewId),
    Update(InspectionViewId),
    Delete(InspectionViewId),
}

impl BenchUpApp {
    fn current_inspection_viewpoint(&mut self, ctx: &egui::Context) -> Option<InspectionViewpoint> {
        if crate::app_host::world_input(ctx) {
            #[cfg(all(feature = "openxr", not(target_arch = "wasm32")))]
            {
                let (stamp, viewpoint) = crate::app_host::inspection_snapshot(ctx)?;
                return (stamp == self.xr_input_stamp(ctx)).then_some(viewpoint);
            }
            #[cfg(not(all(feature = "openxr", not(target_arch = "wasm32"))))]
            return None;
        }
        Some(InspectionViewpoint::from_camera(self.doc.camera.clone()))
    }

    fn inspection_views_changed(&mut self, ctx: &egui::Context) {
        // View metadata does not invalidate retained geometry or geometry undo.
        self.project.modified = true;
        self.persist_edit();
        ctx.request_repaint();
    }

    fn store_inspection_view(
        &mut self,
        ctx: &egui::Context,
        viewpoint: InspectionViewpoint,
        update: Option<InspectionViewId>,
    ) -> Option<InspectionViewId> {
        if !viewpoint.valid() {
            return None;
        }
        let id = if let Some(id) = update {
            self.doc
                .inspection_views
                .iter_mut()
                .find(|view| view.id == id)?
                .viewpoint = viewpoint;
            id
        } else {
            let name = (1..)
                .map(|index| format!("View {index}"))
                .find(|name| {
                    !self
                        .doc
                        .inspection_views
                        .iter()
                        .any(|view| view.name == *name)
                })
                .unwrap();
            let view = InspectionView::new(name, viewpoint);
            let id = view.id;
            self.doc.inspection_views.push(view);
            id
        };
        self.inspection_views_changed(ctx);
        Some(id)
    }

    fn recall_inspection_view(&mut self, ctx: &egui::Context, id: InspectionViewId) {
        let Some(view) = self.doc.inspection_views.iter().find(|view| view.id == id) else {
            return;
        };
        let viewpoint = view.viewpoint.clone();
        if !crate::app_host::request_inspection_recall(ctx, viewpoint.clone()) {
            self.doc.camera = viewpoint.camera;
            self.inspection_views_changed(ctx);
        }
    }

    fn inspection_action(
        &mut self,
        ctx: &egui::Context,
        action: Action,
        current: Option<InspectionViewpoint>,
    ) {
        match action {
            Action::Save => {
                if let Some(point) = current {
                    self.store_inspection_view(ctx, point, None);
                }
            }
            Action::Update(id) => {
                if let Some(point) = current {
                    self.store_inspection_view(ctx, point, Some(id));
                }
            }
            Action::Recall(id) => self.recall_inspection_view(ctx, id),
            Action::Delete(id) => {
                self.doc.inspection_views.retain(|view| view.id != id);
                self.inspection_views_changed(ctx);
            }
        }
    }

    pub(super) fn inspection_views_menu(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let current = self.current_inspection_viewpoint(&ctx);
        if ui
            .add_enabled(current.is_some(), egui::Button::new("Save current view"))
            .clicked()
        {
            self.inspection_action(&ctx, Action::Save, current);
            ui.close();
        }
        ui.separator();
        let entries: Vec<_> = self
            .doc
            .inspection_views
            .iter()
            .map(|view| {
                (
                    view.id,
                    if view.name.trim().is_empty() {
                        "Unnamed view".into()
                    } else {
                        view.name.clone()
                    },
                )
            })
            .collect();
        if entries.is_empty() {
            ui.label("No saved inspection views");
        }
        for (id, name) in entries {
            if ui.push_id(id, |ui| ui.button(name)).inner.clicked() {
                self.recall_inspection_view(&ctx, id);
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Manage saved views").clicked() {
            self.inspector = true;
            sidebar::set_section_open(&ctx, "view", true);
            sidebar::set_section_open(&ctx, "view_inspection_views", true);
            ui.close();
        }
    }

    pub(super) fn inspection_views_controls(&mut self, ui: &mut egui::Ui) {
        sidebar::inspector_section(
            ui,
            "Saved inspection views",
            "view_inspection_views",
            false,
            |ui| {
                let ctx = ui.ctx().clone();
                let current = self.current_inspection_viewpoint(&ctx);
                let mut action = None;
                if ui
                    .add_enabled(current.is_some(), egui::Button::new("Save current view"))
                    .clicked()
                {
                    action = Some(Action::Save);
                }
                ui.label("Stored with this project. Edit a name to rename it.");
                let mut renamed = false;
                sidebar::restored_scroll_area(
                    ui,
                    "inspection_views.table",
                    egui::ScrollArea::horizontal(),
                )
                .show(ui, |ui| {
                    let mut table = InspectorTable::new(
                        ui.available_width(),
                        [
                            Column::at_least(80.),
                            Column::fixed(64.),
                            Column::fixed(48.),
                        ],
                    );
                    table.header(ui, ["Name", "Recall", "More"]);
                    for view in &mut self.doc.inspection_views {
                        table.row(
                            ui,
                            ("inspection_view", view.id),
                            |column, cell| match column {
                                0 => {
                                    renamed |= cell
                                        .add(
                                            egui::TextEdit::singleline(&mut view.name)
                                                .hint_text("View name")
                                                .desired_width(f32::INFINITY),
                                        )
                                        .changed();
                                }
                                1 => {
                                    if cell.button("Recall").clicked() {
                                        action = Some(Action::Recall(view.id));
                                    }
                                }
                                _ => {
                                    cell.menu_button("...", |ui| {
                                        if ui
                                            .add_enabled(
                                                current.is_some(),
                                                egui::Button::new("Update"),
                                            )
                                            .on_hover_text(
                                                "Replace this view with the current viewpoint",
                                            )
                                            .clicked()
                                        {
                                            action = Some(Action::Update(view.id));
                                            ui.close();
                                        }
                                        if ui.button("Delete").clicked() {
                                            action = Some(Action::Delete(view.id));
                                            ui.close();
                                        }
                                    })
                                    .response
                                    .on_hover_text("Update or delete this view");
                                }
                            },
                        );
                    }
                });
                if renamed {
                    self.inspection_views_changed(&ctx);
                }
                if let Some(action) = action {
                    self.inspection_action(&ctx, action, current);
                }
            },
        );
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::test_ui::ContextExt as _;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Preferences(BTreeMap<String, String>);
    impl eframe::Storage for Preferences {
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

    fn frame(
        ctx: &egui::Context,
        app: &mut BenchUpApp,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run_test_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(720., 640.))),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| app.inspection_views_controls(ui),
        )
    }

    fn text_position(output: &egui::FullOutput, label: &str) -> Pos2 {
        fn find(shape: &egui::Shape, label: &str) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                egui::Shape::Vec(shapes) => {
                    shapes.iter().rev().find_map(|shape| find(shape, label))
                }
                _ => None,
            }
        }
        output
            .shapes
            .iter()
            .rev()
            .find_map(|shape| find(&shape.shape, label))
            .unwrap_or_else(|| panic!("Missing {label}"))
    }

    fn click(ctx: &egui::Context, app: &mut BenchUpApp, label: &str) {
        frame(ctx, app, vec![]);
        let output = frame(ctx, app, vec![]);
        let pos = text_position(&output, label);
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }

    #[test]
    fn inspection_views_ui_persists_and_keeps_records_project_owned() {
        const CHILD: &str = "BENCHUP_INSPECTION_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let directory = tempfile::tempdir().unwrap();
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "app::inspection_views::tests::inspection_views_ui_persists_and_keeps_records_project_owned", "--nocapture"])
                .env(CHILD, "1").env("BENCHUP_DATA_DIR",directory.path()).output().unwrap();
            assert!(
                result.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            return;
        }
        let ctx = egui::Context::default();
        apply(&ctx);
        ctx.style_mut_of(ctx.theme(), |style| style.animation_time = 0.);
        sidebar::set_section_open(&ctx, "view_inspection_views", true);
        let mut app = BenchUpApp::with_document(Document::default(), None, true);
        click(&ctx, &mut app, "Save current view");
        assert_eq!(app.doc.inspection_views.len(), 1);
        let id = app.doc.inspection_views[0].id;
        click(&ctx, &mut app, "View 1");
        let command = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(command),
                egui::Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: command,
                },
                egui::Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: command,
                },
            ],
        );
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::NONE),
                egui::Event::Text("Front joint".into()),
            ],
        );
        assert_eq!(
            storage::load().unwrap().unwrap().inspection_views[0].name,
            "Front joint"
        );
        app.doc.camera.target = DVec3::new(3., 2., 1.);
        click(&ctx, &mut app, "...");
        click(&ctx, &mut app, "Update");
        assert_eq!(app.doc.inspection_views[0].id, id);
        assert_eq!(
            app.doc.inspection_views[0].viewpoint.camera.target,
            DVec3::new(3., 2., 1.)
        );
        app.doc.camera = Camera::default();
        click(&ctx, &mut app, "Recall");
        assert_eq!(
            storage::load().unwrap().unwrap().camera.target,
            DVec3::new(3., 2., 1.)
        );

        let mut preferences = Preferences::default();
        app.persist_inspector_changes(&ctx, Some(&mut preferences), false);
        let fresh_ctx = egui::Context::default();
        let mut fresh = BenchUpApp::with_document(Document::default(), None, true);
        fresh.restore_inspector_preferences(&fresh_ctx, Some(&preferences));
        assert!(fresh.doc.inspection_views.is_empty());
        let mut open = None;
        fresh_ctx.run_test_ui(Default::default(), |ui| {
            open = sidebar::restored_header_open(ui, "view_inspection_views", false)
        });
        assert_eq!(open, Some(true));

        #[cfg(feature = "openxr")]
        {
            crate::app_host::set_world_input(&ctx, true);
            let stamp = app.xr_input_stamp(&ctx);
            let point = app.doc.inspection_views[0].viewpoint.clone();
            crate::app_host::publish_inspection_snapshot(&ctx, Some((stamp, point.clone())));
            assert!(app.current_inspection_viewpoint(&ctx).is_some());
            let camera = app.doc.camera.clone();
            app.recall_inspection_view(&ctx, id);
            assert_eq!(app.doc.camera, camera);
            assert_eq!(crate::app_host::take_inspection_recall(&ctx), Some(point));
            app.xr_project_epoch = app.xr_project_epoch.wrapping_add(1);
            assert!(app.current_inspection_viewpoint(&ctx).is_none());
            crate::app_host::set_world_input(&ctx, false);
        }
        click(&ctx, &mut app, "...");
        click(&ctx, &mut app, "Delete");
        assert!(
            storage::load()
                .unwrap()
                .unwrap()
                .inspection_views
                .is_empty()
        );
    }
}
