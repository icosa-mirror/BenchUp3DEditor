//! Model-space controller tools. Document operations and previews remain shared.
use super::*;
use benchup_core::Ray;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) struct Hit {
    pub entity: u64,
    pub face: Option<u64>,
    pub edge: Option<u64>,
    pub guide: Option<(DVec3, DVec3)>,
    pub guide_points: [Option<DVec3>; 8],
    pub flip: Option<Axis>,
    pub annotation: Option<u64>,
    pub edge_targets: [Option<DVec3>; 3],
    pub point: DVec3,
    pub normal: DVec3,
    pub center: Option<DVec3>,
}

impl Hit {
    fn contains(&self, model: &Model, point: DVec3) -> bool {
        if let Some((a, b)) = self.guide {
            let segment = b - a;
            let t = if segment.length_squared() > 1e-24 {
                (point - a).dot(segment) / segment.length_squared()
            } else { 0. };
            point.distance(a + segment * t.clamp(0., 1.)) < 1e-7
        } else if let Some(edge) = self.edge.and_then(|edge| model.edges.get(&edge)) {
            let (a, b) = model.points(edge);
            let direction = b - a;
            let t = (point - a).dot(direction) / direction.length_squared();
            point.distance(a + direction * t.clamp(0., 1.)) < 1e-7
        } else {
            (point - self.point).dot(self.normal).abs() < 1e-6
        }
    }
    fn inference(&self) -> Inference {
        if let Some((a,b)) = self.guide {
            if a == b { Inference::GuidePoint } else { Inference::OnGuide }
        } else if self.edge.is_some() {
            Inference::OnEdge
        } else {
            Inference::OnFace
        }
    }
}

pub(crate) struct Pointer {
    pub ray: Ray,
    pub model: Arc<Model>,
    pub hit: Option<Hit>,
    pub activate: bool,
    pub cancel: bool,
    pub primary_down: bool,
    pub visible_reference: Option<DVec3>,
}

#[derive(Default)]
pub(crate) struct State {
    active: bool,
    tool: Option<Tool>,
    line_normal: Option<DVec3>,
    stroke: bool,
    grab_depth: Option<f64>,
    section: Option<SectionGrab>,
    pointer_target: Option<DVec3>,
    reference: Option<drawing_inference::DrawingReference>,
    erasing: bool,
}

struct SectionGrab {
    depth: f64,
    start_tip: DVec3,
}

impl State {
    pub(crate) fn cancel_erasing(&mut self, app: &mut BenchUpApp) {
        self.erasing = false;
        app.xr_erasing.clear();
    }

    pub(crate) fn pointer_target(&self) -> Option<DVec3> {
        self.pointer_target
    }
}

pub(crate) fn hint(tool: Tool) -> &'static str {
    match tool {
        Tool::Line => "Trigger: start / next endpoint. Cancel ends the chain.",
        Tool::Rectangle => "Trigger: first corner, then opposite corner. Cancel clears.",
        Tool::Circle | Tool::Polygon => "Trigger: centre, then radius. Cancel clears.",
        Tool::Freehand => "Hold trigger to draw; release to finish. Cancel discards the stroke.",
        Tool::Move => {
            "Hold trigger on the selection and move it; release to commit. Cancel discards."
        }
        Tool::PushPull => "Hold trigger on a face and pull it; release to commit. Cancel discards.",
        Tool::Scale => "Hold trigger away from the selection centre, move to resize, then release.",
        Tool::Rotate => {
            "Hold trigger away from the selection centre, move around it, then release."
        }
        Tool::Offset => "Hold trigger near a face edge, move inward or outward, then release.",
        Tool::TapeMeasure => "Trigger: first point, then second point. Cancel clears.",
        Tool::Protractor => "Trigger: centre, baseline, then angle. Cancel clears.",
        Tool::Dimensions => "Trigger: two endpoints, then annotation position.",
        Tool::Text => "Trigger: attach, then label position. Edit text on the panel.",
        Tool::GrainDirection => {
            "Trigger: choose a part and starting point, then the grain direction."
        }
        Tool::Select => "Trigger selects. Cancel clears selection.",
        Tool::Paint => "Trigger paints a face or group with the chosen colour. Undo restores it.",
        Tool::Erase => "Hold trigger over entities; release to erase. Cancel discards the stroke.",
        Tool::Trim => "Trigger: choose the cutter, then the target body. Cancel clears operands.",
        Tool::OuterShell | Tool::Union | Tool::Intersect | Tool::Split => {
            "Trigger: choose the first body, then the second. Cancel clears operands."
        }
        Tool::Flip => {
            "Trigger: choose a body, then one of its coloured flip planes. Undo restores it."
        }
        _ => "This tool uses the editor panel.",
    }
}

fn plane_point(ray: Ray, origin: DVec3, normal: DVec3) -> Option<DVec3> {
    let denominator = ray.direction.dot(normal);
    if denominator.abs() < 1e-8 {
        return None;
    }
    let t = (origin - ray.origin).dot(normal) / denominator;
    (t > 0. && t < 1e7).then(|| ray.origin + ray.direction * t)
}

impl BenchUpApp {
    pub(crate) fn xr_tool_controls(
        &mut self,
        ui: &mut egui::Ui,
        storage: &mut dyn eframe::Storage,
        state: &mut State,
        preferences: &mut crate::app_host::XrPanelPreferences,
    ) {
        let ctx = ui.ctx().clone();
        ui.add_enabled_ui(self.xr_scene_interaction_enabled(&ctx), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().interact_size.y = 42.;
                egui::ComboBox::from_id_salt("benchup.xr.tool_picker")
                    .width(240.)
                    .height(340.)
                    .selected_text(
                        egui::RichText::new(format!("Tool: {}", self.tool.label())).size(18.),
                    )
                    .show_ui(ui, |ui| {
                        let tools = [
                            Tool::Select,
                            Tool::Line,
                            Tool::Rectangle,
                            Tool::Circle,
                            Tool::Polygon,
                            Tool::Freehand,
                            Tool::TapeMeasure,
                            Tool::Protractor,
                            Tool::Dimensions,
                            Tool::Text,
                            Tool::Move,
                            Tool::Rotate,
                            Tool::Scale,
                            Tool::PushPull,
                            Tool::Offset,
                            Tool::Paint,
                            Tool::Erase,
                            Tool::OuterShell,
                            Tool::Union,
                            Tool::Intersect,
                            Tool::Trim,
                            Tool::Split,
                            Tool::Flip,
                            Tool::GrainDirection,
                        ];
                        for row in tools.chunks(4) {
                            ui.horizontal(|ui| {
                                for &tool in row {
                                    if ui
                                        .add_sized(
                                            [132., 42.],
                                            egui::Button::new(
                                                egui::RichText::new(tool.label()).size(18.),
                                            )
                                            .selected(self.tool == tool),
                                        )
                                        .clicked()
                                    {
                                        if tool.is_solid() {
                                            self.start_solid_tool(tool, &ctx);
                                        } else {
                                            self.set_tool(tool);
                                        }
                                        self.persist_inspector_changes(&ctx, Some(storage), false);
                                        ui.close();
                                    }
                                }
                            });
                        }
                    });
                if ui
                    .add_enabled(
                        !self.selected.is_empty(),
                        egui::Button::new(egui::RichText::new("Copy").size(18.))
                            .min_size(egui::vec2(84., 42.)),
                    )
                    .clicked()
                {
                    self.copy_selection(&ctx);
                }
                if ui
                    .add_enabled(
                        self.clipboard.is_some(),
                        egui::Button::new(egui::RichText::new("Paste").size(18.))
                            .min_size(egui::vec2(84., 42.)),
                    )
                    .clicked()
                {
                    self.context_position = None;
                    self.context_action(&ctx, ContextAction::Paste);
                }
                for (label, action) in [("Cancel", 0), ("Undo", 1), ("Redo", 2)] {
                    if ui
                        .add_sized(
                            [84., 42.],
                            egui::Button::new(egui::RichText::new(label).size(18.)),
                        )
                        .clicked()
                    {
                        self.xr_cancel_action(action == 0, &ctx, storage);
                        state.reference = None;
                        state.erasing = false;
                        if action != 0 {
                            self.undo(&ctx, action == 2);
                        }
                    }
                }
                ui.menu_button(egui::RichText::new("Sections").size(18.), |ui| {
                    self.sections_menu(ui)
                });
            });
            let mut changed = false;
            match self.tool {
                Tool::Select => {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut preferences.extend_selection,
                            egui::RichText::new("Add/remove selection").size(18.));
                        ui.label(egui::RichText::new(format!("{} selected",self.selected.len())).size(18.));
                    });
                }
                Tool::TapeMeasure | Tool::Protractor => {
                    changed |= ui
                        .checkbox(
                            &mut self.create_guides,
                            egui::RichText::new("Create construction guides").size(18.),
                        )
                        .changed();
                }
                Tool::Polygon => {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{} sides", self.polygon_sides)).size(18.),
                        );
                        for (label, increase) in [("Sides -", false), ("Sides +", true)] {
                            if ui
                                .add_sized(
                                    [100., 42.],
                                    egui::Button::new(egui::RichText::new(label).size(18.)),
                                )
                                .clicked()
                            {
                                self.polygon_sides = if increase {
                                    self.polygon_sides + 1
                                } else {
                                    self.polygon_sides.saturating_sub(1)
                                }
                                .clamp(3, 64);
                                changed = true;
                            }
                        }
                    });
                }
                Tool::Paint => {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Colour").size(18.));
                        changed |= ui
                            .scope(|ui| {
                                ui.spacing_mut().interact_size = egui::vec2(48., 42.);
                                ui.color_edit_button_srgb(&mut self.paint_color).changed()
                            })
                            .inner;
                        for (label, target) in
                            [("Face", PaintTarget::Face), ("Group", PaintTarget::Group)]
                        {
                            if ui
                                .add_sized(
                                    [100., 42.],
                                    egui::Button::new(egui::RichText::new(label).size(18.))
                                        .selected(self.paint_target == target),
                                )
                                .clicked()
                            {
                                self.paint_target = target;
                                changed = true;
                            }
                        }
                    });
                }
                Tool::PushPull => {
                    changed |= ui
                        .checkbox(
                            &mut self.push_pull_new_faces,
                            egui::RichText::new("Keep starting face").size(18.),
                        )
                        .changed();
                }
                _ => {}
            }
            if changed {
                self.persist_inspector_changes(&ctx, Some(storage), false);
            }
            let hint = if self.paste_preview.is_some() {
                "Trigger places the copy. Cancel discards the preview."
            } else {
                hint(self.tool)
            };
            ui.label(egui::RichText::new(hint).size(16.));
        });
    }

    fn xr_cancel_action(
        &mut self,
        clear_selection: bool,
        ctx: &egui::Context,
        storage: &mut dyn eframe::Storage,
    ) {
        self.xr_erasing.clear();
        if self.world_section_info().is_some() {
            self.cancel_section_drag(ctx);
            self.persist_inspector_changes(ctx, Some(storage), false);
            return;
        }
        let placing_copy = self.paste_preview.is_some();
        self.stop_drawing();
        self.measurement = None;
        self.modeling = None;
        self.paste_preview = None;
        self.solid_first = None;
        self.flip_hover = None;
        if clear_selection && self.tool == Tool::Select && !placing_copy {
            self.selected.clear();
        }
    }

    pub(crate) fn xr_world_pointer(
        &mut self,
        pointer: Option<&Pointer>,
        state: &mut State,
        activate: bool,
        ctx: &egui::Context,
        storage: &mut dyn eframe::Storage,
    ) {
        state.pointer_target = None;
        let Some(pointer) = pointer.filter(|_| self.xr_scene_interaction_enabled(ctx)) else {
            state.erasing = false;
            self.xr_erasing.clear();
            if state.section.take().is_some() && self.world_section_info().is_some() {
                self.cancel_section_drag(ctx);
                self.persist_inspector_changes(ctx, Some(storage), false);
            }
            if state.grab_depth.take().is_some() {
                self.modeling = None;
                self.length_input.clear();
            }
            if state.stroke {
                self.stop_drawing();
                state.stroke = false;
            }
            if state.active {
                self.drawing_reference = None;
                self.flip_hover = None;
                self.snap = None;
                self.hover = None;
                self.drawing_guide = None;
                self.drawing_axis_guides.clear();
                self.drawing_projection = None;
            }
            state.active = false;
            state.reference = None;
            return;
        };
        state.active = true;
        self.flip_hover = None;
        if state.tool != Some(self.tool) {
            if state.section.take().is_some() && self.world_section_info().is_some() {
                self.cancel_section_drag(ctx);
                self.persist_inspector_changes(ctx, Some(storage), false);
            }
            state.tool = Some(self.tool);
            state.erasing = false;
            self.xr_erasing.clear();
            state.line_normal = None;
            state.stroke = false;
            state.grab_depth = None;
            state.reference = None;
        }
        self.hover = pointer.hit.map(|hit| hit.entity);
        // World references survive desktop UI passes without adopting its cursor.
        self.drawing_reference = state.reference;
        self.drawing_guide = None;
        self.drawing_axis_guides.clear();
        self.drawing_projection = None;
        if self.xr_section_pointer(pointer, state, activate, ctx, storage) {
            return;
        }
        if activate && pointer.cancel {
            self.xr_cancel_action(true, ctx, storage);
            state.line_normal = None;
            state.stroke = false;
            state.grab_depth = None;
            state.reference = None;
            state.erasing = false;
            ctx.request_repaint();
            return;
        }
        let clicked = activate && pointer.activate;
        if self.paste_preview.is_some() {
            state.erasing = false;
            self.xr_erasing.clear();
            state.reference = None;
            self.drawing_reference = None;
            let normal = self.plane_lock().map_or(DVec3::Z, Axis::vector);
            self.snap = self.xr_ray_snap(pointer, None, normal);
            self.apply_world_grid_snap(normal,&pointer.model,pointer.hit.and_then(|hit|hit.edge),pointer.hit.and_then(|hit|hit.face));
            if let Some(snap) = self.snap {
                self.paste_preview.as_mut().unwrap().point = snap.point;
                if clicked {
                    let preview = self.paste_preview.take().unwrap();
                    self.paste_at(
                        ctx,
                        &preview.clipboard,
                        preview.point - preview.clipboard.anchor,
                    );
                    self.snap = None;
                }
            }
            ctx.request_repaint();
            return;
        }
        if self.tool.is_solid() {
            self.snap = None;
            let flip = (self.tool == Tool::Flip).then_some(pointer.hit).flatten()
                .and_then(|hit|hit.flip.map(|axis|(axis,hit.point)));
            if let Some((axis,point)) = flip {
                self.flip_hover = Some(axis);
                self.snap = Some(Snap { point, kind: Inference::Plane });
                state.pointer_target = Some(point);
            }
            if clicked {
                if let Some((axis, point)) = flip {
                    self.apply_flip(axis, ctx);
                    self.snap = Some(Snap {
                        point,
                        kind: Inference::Plane,
                    });
                } else {
                    let target = pointer
                        .hit
                        .map(|hit| hit.entity)
                        .filter(|id| self.doc.model.contains_entity(id));
                    self.solid_click_at_target(target, ctx);
                }
            }
            return;
        }
        if matches!(
            self.tool,
            Tool::Move | Tool::PushPull | Tool::Scale | Tool::Offset | Tool::Rotate
        ) {
            self.xr_model_grab(pointer, state, clicked, activate, ctx);
            return;
        }
        if self.tool == Tool::Paint {
            self.snap = None;
            if self.paint_target == PaintTarget::Face {
                self.hover = pointer
                    .hit
                    .and_then(|hit| hit.face)
                    .filter(|id| self.doc.model.faces.contains_key(id));
            }
            if clicked {
                self.paint_model_face(pointer.hit.and_then(|hit| hit.face), ctx);
            }
            return;
        }
        if self.tool == Tool::Erase {
            self.snap = None;
            self.hover = self.hover.filter(|id|self.doc.model.contains_entity(id) && !self.doc.model.is_locked(*id));
            if clicked {
                self.xr_erasing.clear();
                state.erasing = true;
            }
            if state.erasing {
                if let Some(id) = self.hover { self.xr_erasing.insert(id); }
                if activate && !pointer.primary_down {
                    state.erasing = false;
                    let mut ids=std::mem::take(&mut self.xr_erasing);
                    ids.retain(|id|self.doc.model.contains_entity(id) && !self.doc.model.is_locked(*id));
                    if !ids.is_empty() { self.erase_entities(&ids,tr!("app.erase_geometry"),ctx); }
                }
            }
            return;
        }
        if self.tool == Tool::Select {
            if clicked {
                let hit = pointer
                    .hit
                    .map(|hit| hit.entity)
                    .filter(|id| self.doc.model.contains_entity(id));
                if self.xr_panel.extend_selection || ctx.input(|input|input.modifiers.shift) {
                    if let Some(id) = hit && !self.selected.remove(&id) {
                        self.selected.insert(id);
                    }
                } else {
                    self.selected = hit.into_iter().collect();
                }
                ctx.request_repaint();
            }
            return;
        }
        if self.tool == Tool::Text && self.measurement.is_none()
            && let Some(id) = pointer.hit.and_then(|hit|hit.annotation)
                .filter(|id|self.doc.model.texts.contains_key(id))
        {
            self.snap = None;
            if clicked {
                self.selected = BTreeSet::from([id]);
                self.begin_text_edit(id, ctx);
            }
            return;
        }
        if !matches!(
            self.tool,
            Tool::Line
                | Tool::Rectangle
                | Tool::Circle
                | Tool::Polygon
                | Tool::Freehand
                | Tool::TapeMeasure
                | Tool::Protractor
                | Tool::Dimensions
                | Tool::Text
                | Tool::GrainDirection
        ) {
            self.snap = None;
            if clicked {
                self.say(ctx, "Use both grips to navigate the VR scene, or choose an editing tool from the XR picker.");
            }
            return;
        }
        if clicked && self.measurement.as_ref().is_some_and(|m| m.complete) {
            self.measurement = None;
            self.length_input.clear();
        }
        let start = if self.tool == Tool::Line {
            self.line_start
        } else if self.tool == Tool::GrainDirection {
            self.grain_anchor.as_ref().map(|anchor| anchor.point)
        } else if self.tool.is_drawing() {
            self.drawing.as_ref().map(|drawing| drawing.origin)
        } else {
            self.measurement
                .as_ref()
                .filter(|m| !m.complete)
                .and_then(|m| m.points.first().copied())
        };
        if self.tool == Tool::Line && start.is_none() {
            state.line_normal = None;
        }
        if self.lock.is_none()
            && let Some(drawing) = &mut self.drawing
        {
            drawing.set_normal(drawing.surface_normal);
        }
        let normal = (self.tool == Tool::Text)
            .then(|| {
                self.measurement
                    .as_ref()
                    .and_then(|m| m.normal.try_normalize())
            })
            .flatten()
            .or_else(|| self.grain_plane())
            .or_else(|| self.measurement_plane().map(|p| p.1))
            .or_else(|| self.drawing.as_ref().map(|drawing| drawing.normal))
            .or(state.line_normal)
            .unwrap_or(DVec3::Z);
        self.snap = self.xr_ray_snap(pointer, start, normal);
        self.xr_reference_inference(pointer,state,start,normal);
        self.xr_shape_plane(pointer.ray);
        if matches!(self.tool, Tool::Text | Tool::GrainDirection) && start.is_none() {
            // A new leader must attach to geometry, even with snapping disabled.
            self.snap = pointer.hit.map(|hit| {
                self.snap
                    .filter(|snap| {
                        snap.kind != Inference::Plane && hit.contains(&pointer.model, snap.point)
                    })
                    .unwrap_or(Snap {
                        point: hit.point,
                        kind: hit.inference(),
                    })
            });
            if self.tool == Tool::GrainDirection {
                self.hover = pointer.hit.map(|hit| hit.entity).filter(|id| {
                    self.doc.model.objects.contains_key(id)
                        && !self.doc.model.is_locked(*id)
                        && !self.doc.model.is_hidden(*id)
                });
                if self.hover.is_none() {
                    self.snap = None;
                }
            }
        }
        if self.tool == Tool::GrainDirection
            && let Some(anchor) = &self.grain_anchor
        {
            self.hover = Some(anchor.part);
        }
        let plane = self
            .drawing
            .as_ref()
            .map(|drawing| drawing.normal)
            .or_else(|| {
                (self.tool != Tool::Dimensions)
                    .then(|| self.plane_lock().map(Axis::vector))
                    .flatten()
            })
            .unwrap_or(normal);
        self.apply_world_grid_snap(plane,&pointer.model,pointer.hit.and_then(|hit|hit.edge),pointer.hit.and_then(|hit|hit.face));
        if self.tool == Tool::Text
            && let (Some(m), Some(snap)) = (&mut self.measurement, self.snap)
        {
            m.offset = snap.point - m.points[0];
        }
        let valid_offset = self.xr_measurement_offset(pointer.ray);
        if self.tool == Tool::Freehand {
            self.xr_freehand(pointer, state, clicked, activate, ctx);
            return;
        }
        if !clicked || !valid_offset {
            return;
        }
        if self.tool == Tool::Line {
            if self.line_start.is_some() {
                self.commit_line_snap(ctx, !self.length_input.trim().is_empty());
            } else if let Some(snap) = self.snap {
                self.line_start = Some(snap.point);
                state.line_normal = pointer.hit.map(|hit| hit.normal).or(Some(DVec3::Z));
            }
        } else if self.tool.is_drawing() {
            if self.drawing.is_some() {
                self.commit_drawing_at_snap(ctx, false);
            } else {
                self.xr_begin_shape(pointer);
            }
        } else if self.tool == Tool::GrainDirection {
            let hit = if self.grain_anchor.is_none() {
                let anchor = pointer.hit.zip(self.snap).and_then(|(hit, snap)| {
                    self.grain_anchor_at_hit(hit.entity, snap.point, hit.face.map(|_| hit.normal))
                });
                if anchor.is_none() {
                    self.say(
                        ctx,
                        "Choose an unlocked group or component for the grain direction.",
                    );
                    return;
                }
                anchor
            } else {
                None
            };
            self.grain_click_at_anchor(hit, ctx);
        } else if self.tool == Tool::Text {
            if self.snap.is_none() {
                self.say(
                    ctx,
                    "Point at geometry to attach text, or at the label plane to place it.",
                );
                return;
            }
            self.text_click_at_snap(pointer.hit.map(|hit| hit.entity), ctx);
            if let Some(m) = &mut self.measurement {
                m.normal = pointer.ray.direction;
            }
        } else if self.measurement_precision_available() && !self.length_input.trim().is_empty() {
            self.commit_measurement_value(ctx);
        } else {
            if self.tool == Tool::Dimensions
                && let Some(snap) = self.snap
            {
                if let Some(m) = self.measurement.as_mut().filter(|m| m.points.len() == 1) {
                    if let Some(direction) = (snap.point - m.points[0]).try_normalize() {
                        // The annotation plane contains the measured span and
                        // faces the controller at the second endpoint.
                        m.normal = (pointer.ray.direction
                            - direction * pointer.ray.direction.dot(direction))
                        .try_normalize()
                        .unwrap_or_else(|| {
                            direction
                                .cross(DVec3::X)
                                .try_normalize()
                                .unwrap_or(DVec3::Y)
                        });
                    }
                }
            }
            self.measurement_click_snap(
                pointer.hit.map_or(normal, |hit| hit.normal),
                pointer.ray.origin,
                ctx,
            );
        }
        ctx.request_repaint();
    }

    fn xr_section_pointer(
        &mut self,
        pointer: &Pointer,
        state: &mut State,
        activate: bool,
        ctx: &egui::Context,
        storage: &mut dyn eframe::Storage,
    ) -> bool {
        if let Some(grab) = &state.section {
            let Some((axis, offset, _)) = self.world_section_info() else {
                state.section = None;
                return true;
            };
            self.snap = None;
            self.hover = None;
            self.flip_hover = None;
            if !self.doc.display.section_planes
                || !self.doc.display.section_axes[axis]
                || activate && pointer.cancel
            {
                self.cancel_section_drag(ctx);
                self.persist_inspector_changes(ctx, Some(storage), false);
                state.section = None;
                return true;
            }
            let tip = pointer.ray.origin + pointer.ray.direction * grab.depth;
            let value = (offset + (tip - grab.start_tip)[axis]).clamp(-1e8, 1e8);
            if activate && value.is_finite() {
                self.move_section_plane(ctx, axis, value);
                self.persist_inspector_changes(ctx, Some(storage), false);
            }
            state.pointer_target = self.section_world_target();
            if activate && !pointer.primary_down {
                self.finish_world_section();
                state.section = None;
            }
            return true;
        }
        if self.modeling.is_some()
            || self.gesture.is_some()
            || state.stroke
            || state.grab_depth.is_some()
            || pointer.cancel
        {
            return false;
        }
        let handle = self
            .section_handle_anchors()
            .into_iter()
            .filter_map(|(axis, point)| {
                let delta = point - pointer.ray.origin;
                let depth = delta.dot(pointer.ray.direction);
                if depth <= 0. {
                    return None;
                }
                let error = (delta - pointer.ray.direction * depth).length() / depth;
                (error <= 0.02).then_some((axis, point, depth, error))
            })
            .min_by(|a, b| a.3.total_cmp(&b.3).then(a.2.total_cmp(&b.2)));
        let Some((axis, point, depth, _)) = handle else {
            return false;
        };
        self.snap = None;
        self.hover = None;
        self.flip_hover = None;
        state.pointer_target = Some(point);
        if activate && pointer.activate {
            self.begin_world_section(axis, point);
            state.section = Some(SectionGrab {
                depth,
                start_tip: pointer.ray.origin + pointer.ray.direction * depth,
            });
        }
        true
    }

    fn xr_model_grab(
        &mut self,
        pointer: &Pointer,
        state: &mut State,
        clicked: bool,
        activate: bool,
        ctx: &egui::Context,
    ) {
        self.snap = None;
        if matches!(self.tool, Tool::PushPull | Tool::Offset) {
            self.hover = pointer
                .hit
                .and_then(|hit| hit.face)
                .filter(|id| self.doc.model.faces.contains_key(id));
        }
        if !activate {
            return;
        }
        if clicked {
            let Some(hit) = pointer.hit else {
                return;
            };
            let (face, direction) = if matches!(self.tool, Tool::PushPull | Tool::Offset) {
                let Some(face) = hit.face.and_then(|face| self.doc.model.faces.get(&face)) else {
                    self.say(ctx, "Choose an original document face to edit.");
                    return;
                };
                if self.doc.model.is_locked(face.id) {
                    self.say(ctx, "Unlock the face before editing it.");
                    return;
                }
                let Some(plane) = benchup_core::faces::plane(&self.doc.model, &face.vertices)
                else {
                    return;
                };
                if (hit.point - plane.origin).dot(plane.normal).abs() > 1e-6 {
                    self.say(ctx, "Choose an original document face to edit.");
                    return;
                }
                let direction = if self.tool == Tool::Offset {
                    self.offset_direction(face, hit.point, plane)
                } else {
                    plane.normal
                        * if plane.normal.dot(pointer.ray.origin - hit.point) >= 0. {
                            1.
                        } else {
                            -1.
                        }
                };
                if self.selected_modeling_face() != hit.face {
                    self.selected = BTreeSet::from([face.id]);
                }
                (hit.face, direction)
            } else {
                if !self.doc.model.contains_entity(&hit.entity) {
                    self.say(ctx, "Choose an original document entity to transform.");
                    return;
                }
                self.prepare_picking();
                if !self
                    .entity_cache
                    .selected(&self.doc.model, hit.entity, &self.selected)
                {
                    self.selected = BTreeSet::from([hit.entity]);
                }
                if self.doc.model.selection_locked(&self.selected) {
                    self.say(ctx, "Unlock the selection before transforming it.");
                    return;
                }
                (None, DVec3::ZERO)
            };
            let anchor = if self.tool == Tool::Rotate {
                self.doc
                    .model
                    .selection_center(&self.selected)
                    .unwrap_or(hit.point)
            } else {
                hit.point
            };
            self.begin_modeling_at_anchor(anchor, direction, face, Pos2::ZERO, pointer.ray);
            self.modeling.as_mut().unwrap().world_input = true;
            if self.tool == Tool::Rotate
                && let Some(edit) = &mut self.modeling
            {
                let rotation = edit.rotation.as_mut().unwrap();
                let reference = hit.point - edit.anchor;
                let reference = reference - rotation.normal * reference.dot(rotation.normal);
                if reference.length_squared() < 1e-12 {
                    self.modeling = None;
                    self.say(
                        ctx,
                        "Grab a point away from the selection centre and rotation axis.",
                    );
                    return;
                }
                rotation.reference = Some(reference);
                edit.direction = reference;
            }
            if self.tool == Tool::Scale
                && let Some(edit) = &self.modeling
            {
                let reference = edit.anchor - edit.center;
                let reference = edit
                    .scale_constraint
                    .map_or(reference, |constraint| constraint.project(reference));
                if reference.length_squared() < 1e-12 {
                    self.modeling = None;
                    self.say(
                        ctx,
                        "Grab a point away from the selection centre and any locked scale axis.",
                    );
                    return;
                }
            }
            state.grab_depth = Some(
                (hit.point - pointer.ray.origin)
                    .dot(pointer.ray.direction)
                    .max(1e-6),
            );
        }
        let (Some(depth), Some(edit)) = (state.grab_depth, self.modeling.as_ref()) else {
            state.grab_depth = None;
            return;
        };
        // Keep the grabbed anchor at its original distance along the aim ray.
        // This supports controller translation and swinging the ray naturally,
        // including pulls where the ray is parallel to the extrusion axis.
        let mut delta = pointer.ray.origin + pointer.ray.direction * depth - edit.anchor;
        let constraint = self.axis_constraint();
        let rotation_lock = self.shape_lock();
        let direction = if self.tool == Tool::PushPull {
            self.pull_direction(edit.unlocked_direction)
        } else {
            edit.unlocked_direction
        };
        let spacing = grid::configured_spacing(&self.doc.camera, &self.doc.display);
        let edit = self.modeling.as_mut().unwrap();
        edit.snap = None;
        edit.guides.clear();
        if self.tool == Tool::Move {
            if let Some(constraint) = constraint {
                delta = constraint.project(delta);
            }
            if self.snap_to_grid {
                delta = (delta / spacing).round() * spacing;
            }
            edit.direction = delta;
            edit.value = delta.length();
        } else if self.tool == Tool::Rotate {
            let rotation = edit.rotation.as_mut().unwrap();
            rotation.set_lock(rotation_lock);
            let normal = rotation.normal;
            let reference = rotation.reference.unwrap();
            let reference = reference - normal * reference.dot(normal);
            let target = delta - normal * delta.dot(normal);
            if reference.length_squared() < 1e-12 || target.length_squared() < 1e-12 {
                state.grab_depth = None;
                self.modeling = None;
                self.say(ctx, "The rotation reference lies on its axis. Grab another point or change the axis lock.");
                return;
            }
            edit.direction = target;
            edit.value = normal
                .dot(reference.cross(target))
                .atan2(reference.dot(target))
                .to_degrees();
        } else if self.tool == Tool::Scale {
            edit.scale_constraint = constraint;
            let reference = edit.anchor - edit.center;
            let target = edit.anchor + delta - edit.center;
            let (reference, target) = constraint.map_or((reference, target), |constraint| {
                (constraint.project(reference), constraint.project(target))
            });
            if reference.length_squared() < 1e-12 {
                state.grab_depth = None;
                self.modeling = None;
                self.say(ctx, "The scale reference lies on the locked axis. Choose another point or unlock the axis.");
                return;
            }
            let mut radius = target.length();
            if self.snap_to_grid {
                radius = (radius / spacing).round() * spacing;
            }
            edit.value = (radius / reference.length()).clamp(0.001, 1000.);
        } else {
            edit.direction = direction;
            if self.tool == Tool::Offset
                && let Some(constraint) = constraint
            {
                delta = constraint.project(delta);
            }
            edit.value = delta.dot(direction);
            if self.snap_to_grid {
                edit.value = (edit.value / spacing).round() * spacing;
            }
        }
        self.refresh_modeling_preview();
        if !pointer.primary_down {
            state.grab_depth = None;
            if self.modeling.as_ref().is_some_and(|edit| {
                (edit.value - if self.tool == Tool::Scale { 1. } else { 0. }).abs() >= 1e-7
            }) || !self.length_input.trim().is_empty()
            {
                self.commit_modeling_at_value(ctx);
            }
            // Errors leave the authoritative model untouched; do not retain a
            // released grab or commit a frozen preview after tracking loss.
            self.modeling = None;
            self.length_input.clear();
        }
        ctx.request_repaint();
    }

    fn xr_begin_shape(&mut self, pointer: &Pointer) {
        let surface = pointer.hit.filter(|hit| {
            hit.face.is_some()
                && self
                    .snap
                    .is_some_and(|snap| (snap.point - hit.point).dot(hit.normal).abs() < 1e-6)
        });
        self.begin_drawing_at_snap(surface.map(|hit| hit.normal));
    }

    fn xr_freehand(
        &mut self,
        pointer: &Pointer,
        state: &mut State,
        clicked: bool,
        activate: bool,
        ctx: &egui::Context,
    ) {
        if !activate {
            return;
        }
        if clicked {
            self.xr_begin_shape(pointer);
            state.stroke = self.drawing.is_some();
        }
        if !state.stroke {
            return;
        }
        if self.drawing.is_none() {
            state.stroke = false;
            return;
        }
        if let (Some(drawing), Some(snap)) = (&mut self.drawing, self.snap) {
            let last = *drawing.points.last().unwrap();
            // Angular spacing remains consistent when navigating/scaling the
            // model. Only the active stroke receives incremental samples.
            let spacing = (last.distance(pointer.ray.origin) * 0.003).max(1e-6);
            if last.distance(snap.point) >= spacing {
                drawing.points.push(snap.point);
            }
        }
        let full = self
            .drawing
            .as_ref()
            .is_some_and(|drawing| drawing.points.len() >= 2048);
        if !pointer.primary_down || full {
            state.stroke = false;
            // A valid tracked release can finish at the last sample when its
            // ray is parallel to the plane. Missing tracking/focus takes the
            // cancellation path above instead of committing.
            if self.snap.is_none() {
                self.snap = self
                    .drawing
                    .as_ref()
                    .and_then(|drawing| drawing.points.last())
                    .map(|&point| Snap {
                        point,
                        kind: Inference::Plane,
                    });
            }
            self.commit_drawing_at_snap(ctx, false);
            if full {
                self.say(
                    ctx,
                    "Stroke limit reached. Release the trigger to start another stroke.",
                );
            }
        }
        ctx.request_repaint();
    }

    fn xr_reference_inference(&mut self, pointer: &Pointer, state: &mut State, start: Option<DVec3>, normal: DVec3) {
        let eligible = (self.tool.is_drawing() || self.tool.is_measuring() || self.tool == Tool::GrainDirection)
            && self.doc.snap_to_axes && self.doc.snap_to_geometry && self.snap_to_edge;
        let contains = |reference:drawing_inference::DrawingReference| {
            let Some(edge) = self.doc.model.edges.get(&reference.edge)
                .filter(|edge|!self.doc.model.is_hidden(edge.id)) else { return false; };
            let (a,b) = self.doc.model.points(edge);
            let segment=b-a;
            if segment.length_squared()<1e-24 { return false; }
            let t=(reference.point-a).dot(segment)/segment.length_squared();
            (-1e-7..=1.0000001).contains(&t) && reference.point.distance(a+segment*t)<1e-6
        };
        state.reference = state.reference.filter(|reference|eligible && contains(*reference)
            && pointer.visible_reference.is_some_and(|point|point.distance(reference.point)<1e-7));
        let mut acquired = false;
        if eligible && let Some(hit) = pointer.hit.filter(|hit|hit.edge.is_some()) {
            let source = self.snap.filter(|snap| matches!(snap.kind,Inference::Endpoint
                | Inference::Midpoint | Inference::FaceCenter | Inference::Origin | Inference::OnEdge)
                && hit.contains(&pointer.model,snap.point))
                .unwrap_or(Snap {point:hit.point,kind:Inference::OnEdge});
            let source = if self.snap_to_grid {
                grid::snap_with_geometry_hint(source,
                    grid::configured_spacing(&self.doc.camera,&self.doc.display),
                    None,None,&self.doc.model,None,Some((hit.edge,None)))
            } else { source };
            let reference = drawing_inference::DrawingReference {edge:hit.edge.unwrap(),point:source.point};
            if contains(reference) { state.reference=Some(reference); acquired=true; }
        }
        self.drawing_reference=state.reference;
        let (Some(reference),Some(base)) = (state.reference,self.snap) else { return; };
        if matches!(base.kind,Inference::Endpoint | Inference::Midpoint | Inference::FaceCenter
            | Inference::Origin | Inference::OnEdge | Inference::OnGuide | Inference::GuidePoint) { return; }
        let constraint = if self.tool.is_drawing() && self.tool != Tool::Line {
            self.shape_lock().map(AxisConstraint::Plane)
        } else if self.tool == Tool::Dimensions { None } else { self.axis_constraint() };
        let constraint = start.zip(constraint.or_else(||match base.kind {
            Inference::Axis(axis) => Some(AxisConstraint::Axis(axis)), _ => None,
        }));
        let plane = pointer.hit.filter(|hit|base.kind == Inference::OnFace && hit.face.is_some())
            .map_or((start.unwrap_or(DVec3::ZERO),normal),|hit|(hit.point,hit.normal));
        if self.tool != Tool::Dimensions
            && let Some((start,axis)) = start.zip(self.single_axis_lock())
            && let Some(snap) = crate::xr::world_inference::reference_projection(
                pointer.ray,reference.point,start,axis,Some(plane),acquired)
        {
            self.snap=Some(snap);
            self.drawing_projection=Some((reference.point,snap.point));
            return;
        }
        if let Some((snap,guide)) = crate::xr::world_inference::reference_axis(pointer.ray,reference.point,constraint,Some(plane)) {
            self.snap=Some(snap);
            self.drawing_guide=Some(guide);
        }
    }

    fn xr_ray_snap(&self, pointer: &Pointer, start: Option<DVec3>, normal: DVec3) -> Option<Snap> {
        let ray = pointer.ray;
        let constraint = if self.tool.is_drawing() && self.tool != Tool::Line {
            self.shape_lock().map(AxisConstraint::Plane)
        } else {
            (self.tool != Tool::Dimensions)
                .then(|| self.axis_constraint())
                .flatten()
        };
        let allowed = |point: DVec3| {
            start
                .zip(constraint)
                .is_none_or(|(s, c)| c.contains(point - s))
        };
        // Angular tolerance remains usable as the user scales or moves the scene.
        // Targets come from retained spatial queries, never a full scene scan.
        let near = |point: DVec3| -> Option<f64> {
            let delta = point - ray.origin;
            let depth = delta.dot(ray.direction);
            if depth <= 0. || !allowed(point) {
                return None;
            }
            let tolerance = depth * 0.01;
            if pointer.hit.is_some_and(|hit| {
                hit.edge.is_none() && hit.guide.is_none()
                    && depth > (hit.point - ray.origin).dot(ray.direction) + tolerance
            }) {
                return None;
            }
            let distance = (delta - ray.direction * depth).length();
            (distance <= tolerance).then_some(distance / depth)
        };
        let mut points = vec![(DVec3::ZERO, Inference::Origin)];
        if let Some(start) = start {
            points.push((start, Inference::Endpoint));
        }
        if self.doc.snap_to_geometry
            && let Some(hit) = pointer.hit
        {
            points.extend(hit.guide_points.into_iter().flatten()
                .map(|point| (point, Inference::GuidePoint)));
            if self.snap_to_edge {
                points.extend(hit.edge_targets.into_iter().enumerate().filter_map(
                    |(index, point)| {
                        point.map(|point| {
                            (
                                point,
                                if index == 2 {
                                    Inference::Midpoint
                                } else {
                                    Inference::Endpoint
                                },
                            )
                        })
                    },
                ));
            }
            if self.snap_to_face_center
                && let Some(center) = hit.center
            {
                points.push((center, Inference::FaceCenter));
            }
        }
        if let Some((point, kind)) = points
            .into_iter()
            .filter_map(|(p, k)| near(p).map(|distance| (p, k, distance)))
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(p, k, _)| (p, k))
        {
            return Some(Snap { point, kind });
        }
        if self.doc.snap_to_geometry
            && self.snap_to_edge
            && let Some(hit) = pointer
                .hit
                .filter(|hit| hit.edge.is_some() && allowed(hit.point))
        {
            return Some(Snap {
                point: hit.point,
                kind: Inference::OnEdge,
            });
        }
        if self.doc.snap_to_geometry
            && let Some(hit) = pointer.hit.filter(|hit| hit.guide.is_some())
        {
            let (a, b) = hit.guide.unwrap();
            let direction = (b - a).try_normalize();
            let point = match (direction, start.zip(constraint)) {
                (None, _) => Some(a),
                (Some(direction), Some((start, AxisConstraint::Axis(axis)))) => {
                    (Ray { origin: start, direction: axis.vector() }).axis(a, direction)
                        .filter(|point| hit.contains(&pointer.model, *point))
                }
                (Some(direction), Some((start, AxisConstraint::Plane(axis))))
                    if direction.dot(axis.vector()).abs() > 1e-9 => {
                        Some(a + direction * ((start - a).dot(axis.vector())
                            / direction.dot(axis.vector())))
                            .filter(|point| hit.contains(&pointer.model, *point))
                    }
                _ => Some(hit.point),
            };
            if let Some(point) = point.filter(|point| near(*point).is_some()) {
                return Some(Snap { point, kind: hit.inference() });
            }
        }
        if let Some(start) = start {
            if let Some(constraint) = constraint {
                let (point, kind) = match constraint {
                    AxisConstraint::Axis(axis) => {
                        (ray.axis(start, axis.vector())?, Inference::Axis(axis))
                    }
                    AxisConstraint::Plane(axis) => {
                        (plane_point(ray, start, axis.vector())?, Inference::Plane)
                    }
                };
                return ((point - ray.origin).dot(ray.direction) > 0.)
                    .then_some(Snap { point, kind });
            }
            if self.doc.snap_to_axes {
                if let Some((point, axis, _)) = [Axis::X, Axis::Y, Axis::Z]
                    .into_iter()
                    .filter_map(|axis| {
                        let point = ray.axis(start, axis.vector())?;
                        if point.distance_squared(start) < 1e-12 {
                            return None;
                        }
                        near(point).map(|distance| (point, axis, distance))
                    })
                    .min_by(|a, b| a.2.total_cmp(&b.2))
                {
                    return Some(Snap {
                        point,
                        kind: Inference::Axis(axis),
                    });
                }
            }
        }
        if self.snap_to_face
            && self.doc.snap_to_geometry
            && let Some(hit) = pointer.hit.filter(|hit| hit.face.is_some())
        {
            return Some(Snap {
                point: hit.point,
                kind: Inference::OnFace,
            });
        }
        plane_point(ray, start.unwrap_or(DVec3::ZERO), normal).map(|point| Snap {
            point,
            kind: Inference::Plane,
        })
    }

    fn xr_shape_plane(&mut self, ray: Ray) {
        let (Some(drawing), Some(snap)) = (&self.drawing, self.snap) else {
            return;
        };
        if self.lock.is_none()
            && drawing.on_surface
            && matches!(snap.kind, Inference::Axis(_))
            && (snap.point - drawing.origin)
                .dot(drawing.surface_normal)
                .abs()
                > 1e-6
        {
            self.snap = plane_point(ray, drawing.origin, drawing.normal).map(|point| Snap {
                point,
                kind: Inference::Plane,
            });
        }
        if self.lock.is_none()
            && let Some(snap) = self.snap.filter(|snap| snap.kind != Inference::Plane)
        {
            self.drawing
                .as_mut()
                .unwrap()
                .align_to(snap.point, ray.origin);
        }
    }

    fn xr_measurement_offset(&mut self, ray: Ray) -> bool {
        if self.tool != Tool::Dimensions {
            return true;
        }
        let Some(m) = self
            .measurement
            .as_ref()
            .filter(|m| !m.complete && m.points.len() == 2)
        else {
            return true;
        };
        let (a, b) = (m.points[0], m.points[1]);
        let center = (a + b) * 0.5;
        let direction = self
            .dimension_preview()
            .unwrap()
            .line_direction(&self.doc.model);
        let normal = self.plane_lock().map(Axis::vector).unwrap_or(m.normal);
        let offset = if let Some(axis) = self
            .single_axis_lock()
            .filter(|axis| (b - a).dot(axis.vector()).abs() < 1e-7)
        {
            ray.axis(center, axis.vector()).map(|point| point - center)
        } else {
            plane_point(ray, center, normal).map(|point| {
                let delta = point - center;
                delta - direction * delta.dot(direction)
            })
        };
        self.snap = None;
        if let Some(offset) = offset {
            self.measurement.as_mut().unwrap().offset = offset;
        }
        offset.is_some()
    }
}

#[cfg(test)]
mod erase_stroke_tests {
    use super::*;

    #[derive(Default)]
    struct Storage;
    impl eframe::Storage for Storage {
        fn get_string(&self, _: &str) -> Option<String> { None }
        fn set_string(&mut self, _: &str, _: String) {}
        fn remove_string(&mut self, _: &str) {}
        fn flush(&mut self) {}
    }

    fn fixture() -> (BenchUpApp, Arc<Model>, [u64; 2]) {
        let mut doc=Document::default();
        let first=doc.model.add_edge(DVec3::ZERO,DVec3::X).unwrap();
        let second=doc.model.add_edge(DVec3::Y,DVec3::X+DVec3::Y).unwrap();
        let model=Arc::new(doc.model.clone());
        let mut app=BenchUpApp::with_document(doc,None,true);
        app.set_tool(Tool::Erase);
        (app,model,[first,second])
    }

    fn pointer(model: &Arc<Model>, entity: Option<u64>, down: bool, activate: bool) -> Pointer {
        Pointer {
            ray:Ray {origin:DVec3::new(0.5,0.,1.),direction:-DVec3::Z},
            model:Arc::clone(model),
            hit:entity.map(|entity|Hit {
                entity,face:None,edge:Some(entity),guide:None,guide_points:[None;8],
                flip:None,annotation:None,edge_targets:[None;3],
                point:DVec3::new(0.5,0.,0.),normal:DVec3::Z,center:None,
            }),
            activate,cancel:false,primary_down:down,visible_reference:None,
        }
    }

    #[test]
    fn held_erase_commits_all_targets_on_release_as_one_undo() {
        let (mut app,model,ids)=fixture();
        let ctx=egui::Context::default();
        let mut state=State::default();
        let mut storage=Storage;
        for (index,id) in ids.into_iter().enumerate() {
            app.xr_world_pointer(Some(&pointer(&model,Some(id),true,index==0)),&mut state,true,&ctx,&mut storage);
            assert_eq!(app.doc.model.edges.len(),2);
        }
        assert_eq!(app.xr_erasing,BTreeSet::from(ids));
        app.xr_world_pointer(Some(&pointer(&model,None,false,false)),&mut state,true,&ctx,&mut storage);
        assert!(app.doc.model.edges.is_empty());
        assert!(app.xr_erasing.is_empty());
        app.undo(&ctx,false);
        assert_eq!(app.doc.model,*model);
    }

    #[test]
    fn lost_pointer_and_cancel_discard_erase_without_later_release_commit() {
        for cancel in 0..3 {
            let (mut app,model,ids)=fixture();
            let ctx=egui::Context::default();
            let mut state=State::default();
            let mut storage=Storage;
            app.xr_world_pointer(Some(&pointer(&model,Some(ids[0]),true,true)),&mut state,true,&ctx,&mut storage);
            if cancel == 1 {
                let mut input=pointer(&model,Some(ids[1]),true,false);
                input.cancel=true;
                app.xr_world_pointer(Some(&input),&mut state,true,&ctx,&mut storage);
            } else if cancel == 0 {
                app.xr_world_pointer(None,&mut state,false,&ctx,&mut storage);
            } else {
                state.cancel_erasing(&mut app);
            }
            app.xr_world_pointer(Some(&pointer(&model,None,false,false)),&mut state,true,&ctx,&mut storage);
            assert_eq!(app.doc.model,*model);
            assert!(app.xr_erasing.is_empty());
            assert!(!state.erasing);
        }
    }
}
