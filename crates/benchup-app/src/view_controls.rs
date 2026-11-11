use benchup_core::tr;
use super::*;
use benchup_core::StandardView;

impl BenchUpApp {
    pub(super) fn view_contents(&mut self, ui: &mut egui::Ui) {
        let before = self.doc.camera.clone();
        let immersive = crate::app_host::world_input(ui.ctx());
        inspector_section(ui, tr!("view_controls.camera"), "view_camera", false, |ui| {
            if immersive {
                ui.label("Projection and field of view follow the headset. Hold both grips to move, turn and scale the scene.");
                ui.horizontal_wrapped(|ui| {
                    for (label, action) in [
                        ("Fit model", crate::app_host::WorldView::Scene),
                        ("Fit selection", crate::app_host::WorldView::Selection),
                        ("Center origin", crate::app_host::WorldView::Origin),
                        ("Reset view", crate::app_host::WorldView::Reset),
                    ] {
                        let enabled = action != crate::app_host::WorldView::Selection || !self.selected.is_empty();
                        if ui.add_enabled(enabled, egui::Button::new(egui::RichText::new(label).size(18.))
                            .min_size(egui::vec2(120., 42.))).clicked()
                        {
                            crate::app_host::request_world_view(ui.ctx(), action);
                        }
                    }
                });
                return;
            }
            ui.text_label(tr!("view_controls.projection"));
            let button_width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.;
            ui.horizontal(|ui| {
                for (projection, label) in [
                    (Projection::Perspective, tr!("view_controls.perspective")),
                    (Projection::Parallel, tr!("view_controls.parallel")),
                ] {
                    let response = ui.add_sized(
                        [button_width, ui.spacing().interact_size.y],
                        egui::Button::new(label).selected(self.doc.camera.projection == projection),
                    );
                    if response.clicked() {
                        self.doc.camera.projection = projection;
                    }
                    if projection == Projection::Parallel {
                        response
                            .on_hover_text(tr!("view_controls.orthographic_projection_parallel_lines_stay_parallel"));
                    }
                }
            });
            ui.add_enabled_ui(
                self.doc.camera.projection == Projection::Perspective,
                |ui| {
                    ui.text_label(tr!("view_controls.field_of_view"));
                    let mut degrees = self.doc.camera.fov.to_degrees();
                    if ui
                        .add(egui::Slider::new(&mut degrees, 15. ..=100.).suffix(tr!("view_controls.degree_symbol")))
                        .changed()
                    {
                        self.doc.camera.fov = degrees.to_radians();
                    }
                },
            );
            let button_width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.;
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [button_width, ui.spacing().interact_size.y],
                        egui::Button::new(tr!("view_controls.fit_model")),
                    )
                    .clicked()
                {
                    self.doc.camera.fit(
                        self.doc.model.bounds(),
                        dv(self.viewport.size().max(Vec2::splat(1.))),
                    );
                }
                ui.add_enabled_ui(!self.selected.is_empty(), |ui| {
                    if ui
                        .add_sized(
                            [button_width, ui.spacing().interact_size.y],
                            egui::Button::new(tr!("view_controls.fit_selection")),
                        )
                        .clicked()
                    {
                        self.fit_selection(ui.ctx());
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [button_width, ui.spacing().interact_size.y],
                        egui::Button::new(tr!("view_controls.reset_camera")),
                    )
                    .clicked()
                {
                    self.doc.camera = Camera::default();
                    self.doc.camera.fit(
                        self.doc.model.bounds(),
                        dv(self.viewport.size().max(Vec2::splat(1.))),
                    );
                }
            });
        });
        inspector_section(ui, tr!("view_controls.standard_views"), "view_standard_views", false, |ui| {
            if immersive {
                ui.label("Turn the scene to view it from one of these directions. Reset view restores the upright starting orientation.");
                if let Some(view) = standard_view_cards(ui, None, true) {
                    crate::app_host::request_world_view(ui.ctx(), crate::app_host::WorldView::Standard(view));
                }
            } else {
                standard_view_picker(ui, &mut self.doc.camera);
            }
        });
        if before != self.doc.camera {
            self.changed(ui.ctx());
        }
    }
}

// Match the face-style cards: responsive columns, cube preview, caption,
// selected background, and a radio-button accessibility description.
fn standard_view_picker(ui: &mut egui::Ui, camera: &mut Camera) {
    if let Some(view) = standard_view_cards(ui, camera.standard_view(), false) {
        camera.set_standard_view(view);
    }
}

fn standard_view_cards(ui: &mut egui::Ui, current: Option<StandardView>, actions: bool) -> Option<StandardView> {
    let mut chosen = None;
    let mut current = current;
    let colors = Palette::for_ui(ui);
    let faces = FacePalette::for_ui(ui);
    let spacing = ui.spacing().item_spacing.x;
    let columns = ((ui.available_width() + spacing) / (96. + spacing))
        .floor()
        .clamp(1., StandardView::ALL.len() as f32) as usize;
    ui.push_id("standard_views", |ui| {
        for row in StandardView::ALL.chunks(columns) {
            let width =
                (ui.available_width() - spacing * (row.len() - 1) as f32) / row.len() as f32;
            ui.horizontal(|ui| {
                for &view in row {
                    let selected = current == Some(view);
                    let mut response = ui
                        .add_sized(
                            egui::vec2(width, 100.),
                            egui::Button::new("").selected(selected).corner_radius(5),
                        )
                        .on_hover_text(tr!("view_controls.view", view.label()));
                    if response.clicked() && !selected {
                        chosen = Some(view);
                        if !actions { current = Some(view); }
                        response.mark_changed();
                    }
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            if actions { egui::Role::Button } else { egui::Role::RadioButton },
                            ui.is_enabled(),
                            !actions && current == Some(view),
                            view.label(),
                        )
                    });
                    if ui.is_rect_visible(response.rect) {
                        let rect = response.rect;
                        standard_view_preview(
                            ui.painter(),
                            Rect::from_center_size(
                                egui::pos2(rect.center().x, rect.top() + 33.),
                                egui::vec2(width - 16., 50.),
                            ),
                            view,
                            colors,
                            faces,
                        );
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.bottom() - 21.),
                            egui::Align2::CENTER_CENTER,
                            view.label(),
                            FontId::proportional(if actions { 18. } else { 11. }),
                            colors.ink,
                        );
                    }
                }
            });
        }
    });
    chosen
}

#[cfg(all(test, feature = "openxr", not(target_arch = "wasm32")))]
mod xr_view_tests {
    use super::*;
    use crate::{app_host::{self, WorldView}, test_ui::ContextExt};

    fn frame(app: &mut BenchUpApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run_test_ui(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1400., 1000.))),
            focused: true, events, ..Default::default()
        }, |ui| {
            app_host::set_world_input(ctx, true);
            app.view_contents(ui);
        })
    }

    fn position(output: &egui::FullOutput, label: &str) -> Pos2 {
        fn find(shape: &egui::Shape, label: &str) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.rect.center().to_vec2()),
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
                _ => None,
            }
        }
        output.shapes.iter().find_map(|shape| find(&shape.shape, label)).expect("visible view action")
    }

    #[test]
    fn xr_view_controls_route_actions_without_changing_the_desktop_camera() {
        let ctx = egui::Context::default();
        let mut app = BenchUpApp::with_document(Document::default(), None, true);
        set_section_open(&ctx, "view_camera", true);
        set_section_open(&ctx, "view_standard_views", true);
        let camera = app.doc.camera.clone();
        let mut output = frame(&mut app, &ctx, Vec::new());
        for (label, expected) in [
            ("Fit selection", None),
            ("Fit model", Some(WorldView::Scene)),
            ("Center origin", Some(WorldView::Origin)),
            ("Reset view", Some(WorldView::Reset)),
            (StandardView::Top.label(), Some(WorldView::Standard(StandardView::Top))),
        ] {
            let pos = position(&output, label);
            for pressed in [true, false] {
                output = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                    pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
                }]);
            }
            assert_eq!(app_host::take_world_view(&ctx), expected);
            assert_eq!(app_host::take_world_view(&ctx), None, "commands must not replay");
            assert_eq!(app.doc.camera, camera);
            assert!(!app.project.modified);
        }
        // Shared commands invoked by keyboard/menu consumers use the same route.
        let _ = ctx.run_test_ui(egui::RawInput::default(), |_| {
            app_host::set_world_input(&ctx, true);
            app.selected.insert(1);
            app.fit_selection(&ctx);
        });
        assert_eq!(app_host::take_world_view(&ctx), Some(WorldView::Selection));
        assert_eq!(app.doc.camera, camera);
        // Outside the immersive host these remain normal desktop commands.
        app.doc.camera.target = DVec3::ONE;
        app.fit_origin(&ctx);
        assert_eq!(app.doc.camera.target, DVec3::ZERO);
        assert_eq!(app_host::take_world_view(&ctx), None);
    }
}

fn standard_view_preview(
    painter: &egui::Painter,
    rect: Rect,
    view: StandardView,
    colors: Palette,
    faces: FacePalette,
) {
    // Directional icons share one orientation, including the three faces on
    // the far side. Only Iso uses its own shaded, isometric preview.
    let mut camera = Camera::default();
    if view == StandardView::Iso {
        camera.set_standard_view(view);
    }
    let toward_eye = (camera.eye() - camera.target).normalize();
    let scale = rect.width().min(rect.height()) * 0.30;
    let project = |p: DVec3| {
        rect.center() + egui::vec2(p.dot(camera.right()) as f32, -p.dot(camera.up()) as f32) * scale
    };
    let axes = [
        (DVec3::X, DVec3::Y, DVec3::Z),
        (DVec3::Y, DVec3::X, DVec3::Z),
        (DVec3::Z, DVec3::X, DVec3::Y),
    ];
    let face_points = |normal: DVec3, u: DVec3, v: DVec3| {
        [
            normal - u - v,
            normal + u - v,
            normal + u + v,
            normal - u + v,
        ]
        .map(project)
        .to_vec()
    };
    if view == StandardView::Iso {
        for (axis, u, v) in axes {
            for sign in [-1., 1.] {
                let normal = axis * sign;
                if normal.dot(toward_eye) <= 0. {
                    continue;
                }
                let fill = if axis == DVec3::Z {
                    faces.front
                } else if axis == DVec3::X {
                    faces.side
                } else {
                    faces.shaded_side
                };
                painter.add(egui::Shape::convex_polygon(
                    face_points(normal, u, v),
                    fill,
                    Stroke::new(1.2, colors.ink),
                ));
            }
        }
        return;
    }
    let (normal, u, v) = match view {
        StandardView::Top => (DVec3::Z, DVec3::X, DVec3::Y),
        StandardView::Front => (-DVec3::Y, DVec3::X, DVec3::Z),
        StandardView::Right => (DVec3::X, DVec3::Y, DVec3::Z),
        StandardView::Back => (DVec3::Y, DVec3::X, DVec3::Z),
        StandardView::Left => (-DVec3::X, DVec3::Y, DVec3::Z),
        StandardView::Bottom => (-DVec3::Z, DVec3::X, DVec3::Y),
        StandardView::Iso => unreachable!(),
    };
    let highlighted = face_points(normal, u, v);
    painter.add(egui::Shape::convex_polygon(
        highlighted.clone(),
        Color32::from_rgba_unmultiplied(colors.green.r(), colors.green.g(), colors.green.b(), 80),
        Stroke::NONE,
    ));
    // Draw all twelve edges so back, left, and bottom remain readable through
    // the empty cube. Recede the hidden edges without hiding the chosen face.
    for hidden in [true, false] {
        for (axis, u, v) in axes {
            for a in [-1., 1.] {
                for b in [-1., 1.] {
                    let is_hidden = (u * a).dot(toward_eye) <= 0. && (v * b).dot(toward_eye) <= 0.;
                    if is_hidden != hidden {
                        continue;
                    }
                    let center = u * a + v * b;
                    painter.line_segment(
                        [project(center - axis), project(center + axis)],
                        if hidden {
                            Stroke::new(1., colors.muted)
                        } else {
                            Stroke::new(1.2, colors.ink)
                        },
                    );
                }
            }
        }
    }
    painter.add(egui::Shape::closed_line(
        highlighted,
        Stroke::new(1.8, colors.green),
    ));
}

impl BenchUpApp {
    pub(in crate::app) fn paint_menu(&mut self, ui: &mut egui::Ui) {
        let colors = Palette::for_ui(ui);
        let color = Color32::from_rgb(
            self.paint_color[0],
            self.paint_color[1],
            self.paint_color[2],
        );
        let icon = egui::Atom::paint(egui::vec2(18., 18.), |ui, args| {
            icons::paint_bucket(ui.painter(), args.rect, color);
        });
        let response = ui
            .add(
                egui::Button::new((icon, icons::dropdown_atom(colors.muted)))
                .min_size(egui::vec2(0., 32.))
                .selected(self.tool == Tool::Paint),
            )
            .on_hover_text(tr!("view_controls.choose_how_to_paint_and_a_paint_color"));
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr!("view_controls.color")));
        egui::Popup::menu(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                Self::menu_style(ui.style_mut());
                ui.set_width(256.);
                let padding = ui.spacing().button_padding.x as i8;
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(padding, 6))
                    .show(ui, |ui| {
                        ui.text_label(tr!("view_controls.paint_by"));
                        let button_width =
                            (ui.available_width() - ui.spacing().item_spacing.x) / 2.;
                        ui.horizontal(|ui| {
                            for (target, label) in
                                [(PaintTarget::Face, tr!("view_controls.face")), (PaintTarget::Group, tr!("view_controls.group"))]
                            {
                                if ui
                                    .add_sized(
                                        [button_width, 30.],
                                        egui::Button::new(label)
                                            .selected(self.paint_target == target),
                                    )
                                    .clicked()
                                {
                                    self.paint_target = target;
                                    self.set_tool(Tool::Paint);
                                }
                            }
                        });
                        ui.add_space(4.);
                        ui.separator();
                        ui.add_space(4.);
                        self.paint_color_controls(ui, true);
                    });
            });
    }

    pub(in crate::app) fn paint_color_controls(&mut self, ui: &mut egui::Ui, activate_paint: bool) {
        crate::inspector_table::inspector_field(ui, "paint_color", tr!("view_controls.choose_color"), |ui| {
            ui.spacing_mut().interact_size.x = ui.available_width();
            let response = ui.color_edit_button_srgb(&mut self.paint_color);
            if response.changed() && activate_paint {
                self.set_tool(Tool::Paint);
            }
            response
        });
        ui.add_space(6.);
        ui.text_label(
            RichText::new(tr!("view_controls.preset_colors"))
                .small()
                .color(Palette::for_ui(ui).muted),
        );
        let presets = [
            // Light hues, from warm to cool.
            [245, 182, 186],
            [250, 205, 160],
            [249, 235, 162],
            [186, 216, 172],
            [175, 204, 238],
            [217, 188, 231],
            // Saturated hues in the same order.
            [210, 48, 48],
            [239, 128, 45],
            [245, 210, 65],
            [60, 153, 88],
            [55, 105, 195],
            [133, 72, 181],
            // Muted hues in the same order.
            [195, 96, 67],
            [194, 132, 75],
            [225, 179, 87],
            [108, 148, 102],
            [95, 143, 177],
            [163, 121, 167],
            // Neutrals and wood tones, from light to dark.
            [255, 255, 255],
            [236, 231, 215],
            [201, 204, 207],
            [139, 145, 151],
            [148, 101, 65],
            [78, 84, 81],
        ];
        use crate::inspector_table::{Column, InspectorTable};
        let mut table = InspectorTable::new(ui.available_width(), [Column::flex(1.); 6]);
        for (row_index, row) in presets.chunks(6).enumerate() {
            if row_index > 0 {
                ui.add_space(4.);
            }
            table.row(ui, ("paint_preset_row", row_index), |column, cell| {
                let color = row[column];
                let rgb = Color32::from_rgb(color[0], color[1], color[2]);
                // Let narrow cells size the swatch instead of the default button minimum.
                cell.spacing_mut().interact_size.x = cell.available_width();
                cell.spacing_mut().button_padding.x = 0.;
                if cell
                    .add(egui::Button::new("").fill(rgb))
                    .on_hover_text(format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]))
                    .clicked()
                {
                    self.paint_color = color;
                    if activate_paint {
                        self.set_tool(Tool::Paint);
                    }
                }
            });
        }
    }

    pub(in crate::app) fn sections_menu(&mut self, ui: &mut egui::Ui) {
        let before = self.doc.display.clone();
        let button = egui::Button::new((Self::menu_item_icon(ui, Some("sections")), tr!("view_controls.sections")))
            .gap(ui.spacing().icon_spacing)
            .right_text(egui::menu::SubMenuButton::arrow_atom(None));
        egui::menu::SubMenuButton::from_button(button)
            .config(
                egui::menu::MenuConfig::new()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
            )
            .ui(ui, |ui| {
                Self::menu_style(ui.style_mut());
                ui.set_width(208.);
                let d = &mut self.doc.display;
                Self::menu_checkbox(ui, &mut d.section_cuts, tr!("view_controls.section_cuts"));
                Self::menu_checkbox(ui, &mut d.section_fill, tr!("view_controls.section_fill"));
                ui.separator();
                let padding = ui.spacing().button_padding.x as i8;
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(padding, 0))
                    .show(ui, |ui| {
                        use crate::inspector_table::{Column, InspectorTable};
                        let mut table = InspectorTable::new(
                            ui.available_width(),
                            [Column::fixed(112.), Column::flex(1.)],
                        );
                        for (axis, label) in [tr!("view_controls.x_vertical"), tr!("view_controls.y_vertical"), tr!("view_controls.z_horizontal")]
                            .into_iter()
                            .enumerate()
                        {
                            table.row(ui, ("section_axis", axis), |column, cell| {
                                if column == 0 {
                                    if cell
                                        .checkbox(&mut d.section_axes[axis], label)
                                        .on_hover_text(
                                            tr!("view_controls.keeps_geometry_on_the_negative_side_of_this"),
                                        )
                                        .changed()
                                    {
                                        d.section_planes =
                                            d.section_axes.iter().any(|&enabled| enabled);
                                    }
                                } else {
                                    let offset = if axis == 2 {
                                        &mut d.section_height
                                    } else {
                                        &mut d.section_offsets[axis]
                                    };
                                    cell.add(
                                        egui::DragValue::new(offset)
                                            .speed(0.05)
                                            .range(-1e8..=1e8)
                                            .suffix(" m"),
                                    );
                                }
                            });
                        }
                    });
            })
            .0
            .on_hover_text(tr!("view_controls.configure_section_planes_cuts_fill_and_axis_positions"));
        if self.doc.display != before {
            self.changed(ui.ctx());
        }
    }
}
