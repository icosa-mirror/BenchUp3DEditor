//! Model-space controller tools. Document operations and previews remain shared.
use super::*;
use benchup_core::Ray;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) struct Hit {
    pub entity: u64,
    pub face: u64,
    pub point: DVec3,
    pub normal: DVec3,
    pub center: Option<DVec3>,
}

pub(crate) struct Pointer {
    pub ray: Ray,
    pub model: Arc<Model>,
    pub hit: Option<Hit>,
    pub activate: bool,
    pub cancel: bool,
    pub primary_down: bool,
}

#[derive(Default)]
pub(crate) struct State {
    active: bool,
    tool: Option<Tool>,
    line_normal: Option<DVec3>,
    stroke: bool,
    grab_depth: Option<f64>,
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
        Tool::Select => "Trigger selects. Cancel clears selection.",
        Tool::Paint => "Trigger paints a face or group with the chosen colour. Undo restores it.",
        Tool::Erase => "Trigger erases the highlighted entity. Undo restores it.",
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
                        self.xr_cancel_action(action == 0);
                        if action != 0 {
                            self.undo(&ctx, action == 2);
                        }
                    }
                }
            });
            let mut changed = false;
            match self.tool {
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

    fn xr_cancel_action(&mut self, clear_selection: bool) {
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
    ) {
        let Some(pointer) = pointer.filter(|_| self.xr_scene_interaction_enabled(ctx)) else {
            if state.grab_depth.take().is_some() {
                self.modeling = None;
                self.length_input.clear();
            }
            if state.stroke {
                self.stop_drawing();
                state.stroke = false;
            }
            if state.active {
                self.snap = None;
                self.hover = None;
                self.drawing_guide = None;
                self.drawing_axis_guides.clear();
                self.drawing_projection = None;
            }
            state.active = false;
            return;
        };
        state.active = true;
        if state.tool != Some(self.tool) {
            state.tool = Some(self.tool);
            state.line_normal = None;
            state.stroke = false;
            state.grab_depth = None;
        }
        self.hover = pointer.hit.map(|hit| hit.entity);
        // Desktop reference acquisition uses screen coordinates; don't retain
        // those guides when ownership moves to a model-space controller ray.
        self.drawing_reference = None;
        self.drawing_guide = None;
        self.drawing_axis_guides.clear();
        self.drawing_projection = None;
        if activate && pointer.cancel {
            self.xr_cancel_action(true);
            state.line_normal = None;
            state.stroke = false;
            state.grab_depth = None;
            ctx.request_repaint();
            return;
        }
        let clicked = activate && pointer.activate;
        if self.paste_preview.is_some() {
            let normal = self.plane_lock().map_or(DVec3::Z, Axis::vector);
            self.snap = self.xr_ray_snap(pointer, None, normal);
            self.apply_grid_snap_in_plane(Some(normal));
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
            if clicked {
                if self.tool == Tool::Flip
                    && let Some((axis, point)) = self.xr_flip_plane(pointer.ray)
                {
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
                    .map(|hit| hit.face)
                    .filter(|id| self.doc.model.faces.contains_key(id));
            }
            if clicked {
                self.paint_model_face(pointer.hit.map(|hit| hit.face), ctx);
            }
            return;
        }
        if self.tool == Tool::Erase {
            self.snap = None;
            self.hover = self.hover.filter(|id| !self.doc.model.is_locked(*id));
            if clicked && let Some(id) = self.hover {
                self.erase_entities(&BTreeSet::from([id]), tr!("app.erase_geometry"), ctx);
            }
            return;
        }
        if self.tool == Tool::Select {
            if clicked {
                self.selected = pointer
                    .hit
                    .map(|hit| hit.entity)
                    .filter(|id| self.doc.model.contains_entity(id))
                    .into_iter()
                    .collect();
                ctx.request_repaint();
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
        ) {
            self.snap = None;
            if clicked {
                self.say(ctx, "This tool currently uses the editor panel. Choose a drawing or measurement tool from the XR controls to edit in 3D.");
            }
            return;
        }
        if clicked && self.measurement.as_ref().is_some_and(|m| m.complete) {
            self.measurement = None;
            self.length_input.clear();
        }
        let start = if self.tool == Tool::Line {
            self.line_start
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
            .or_else(|| self.measurement_plane().map(|p| p.1))
            .or_else(|| self.drawing.as_ref().map(|drawing| drawing.normal))
            .or(state.line_normal)
            .unwrap_or(DVec3::Z);
        self.snap = self.xr_ray_snap(pointer, start, normal);
        self.xr_shape_plane(pointer.ray);
        if self.tool == Tool::Text && start.is_none() {
            // A new leader must attach to geometry, even with snapping disabled.
            self.snap = pointer.hit.map(|hit| {
                self.snap
                    .filter(|snap| {
                        snap.kind != Inference::Plane
                            && (snap.point - hit.point).dot(hit.normal).abs() < 1e-6
                    })
                    .unwrap_or(Snap {
                        point: hit.point,
                        kind: Inference::OnFace,
                    })
            });
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
        self.apply_grid_snap_in_plane(Some(plane));
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

    fn xr_flip_plane(&self, ray: Ray) -> Option<(Axis, DVec3)> {
        // Only query the three shared flip quads on a click; no per-frame scan
        // of selected geometry or separate plane geometry is introduced.
        self.flip_planes()
            .into_iter()
            .filter_map(|(axis, quad)| {
                let u = quad[1] - quad[0];
                let v = quad[3] - quad[0];
                let normal = u.cross(v).try_normalize()?;
                let point = plane_point(ray, quad[0], normal)?;
                let delta = point - quad[0];
                let (a, b) = (
                    delta.dot(u) / u.length_squared(),
                    delta.dot(v) / v.length_squared(),
                );
                ((0. ..=1.).contains(&a) && (0. ..=1.).contains(&b)).then_some((
                    axis,
                    point,
                    (point - ray.origin).dot(ray.direction),
                ))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(axis, point, _)| (axis, point))
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
                .map(|hit| hit.face)
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
                let Some(face) = self.doc.model.faces.get(&hit.face) else {
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
                if self.selected_modeling_face() != Some(hit.face) {
                    self.selected = BTreeSet::from([hit.face]);
                }
                (Some(hit.face), direction)
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
            self.snap
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
        // Only the hit face's boundary is visited, never the full scene per frame.
        let near = |point: DVec3| -> Option<f64> {
            let delta = point - ray.origin;
            let depth = delta.dot(ray.direction);
            if depth <= 0. || !allowed(point) {
                return None;
            }
            let tolerance = depth * 0.01;
            if pointer
                .hit
                .is_some_and(|hit| depth > (hit.point - ray.origin).dot(ray.direction) + tolerance)
            {
                return None;
            }
            let distance = (delta - ray.direction * depth).length();
            (distance <= tolerance).then_some(distance / depth)
        };
        let mut points = vec![(DVec3::ZERO, Inference::Origin)];
        if let Some(start) = start {
            points.push((start, Inference::Endpoint));
        }
        let face = pointer
            .hit
            .and_then(|hit| pointer.model.faces.get(&hit.face));
        if self.doc.snap_to_geometry {
            if let Some(face) = face {
                if self.snap_to_edge {
                    for ring in std::iter::once(&face.vertices).chain(face.holes.iter()) {
                        for (&a, &b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
                            let a = pointer.model.vertices[&a].position;
                            let b = pointer.model.vertices[&b].position;
                            points.push((a, Inference::Endpoint));
                            points.push(((a + b) * 0.5, Inference::Midpoint));
                        }
                    }
                }
                if self.snap_to_face_center {
                    if let Some(center) = pointer.hit.and_then(|hit| hit.center) {
                        points.push((center, Inference::FaceCenter));
                    }
                }
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
        if self.doc.snap_to_geometry && self.snap_to_edge {
            let mut nearest: Option<(DVec3, f64)> = None;
            if let Some(face) = face {
                for ring in std::iter::once(&face.vertices).chain(face.holes.iter()) {
                    for (&a, &b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
                        let a = pointer.model.vertices[&a].position;
                        let b = pointer.model.vertices[&b].position;
                        if a.distance_squared(b) < 1e-16 {
                            continue;
                        }
                        let point = ray.segment_point(a, b);
                        if let Some(distance) = near(point) {
                            if nearest.is_none_or(|(_, old)| distance < old) {
                                nearest = Some((point, distance));
                            }
                        }
                    }
                }
            }
            if let Some((point, _)) = nearest {
                return Some(Snap {
                    point,
                    kind: Inference::OnEdge,
                });
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
            && let Some(hit) = pointer.hit
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
