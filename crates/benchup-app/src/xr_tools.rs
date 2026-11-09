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
}

#[derive(Default)]
pub(crate) struct State {
    active: bool,
    tool: Option<Tool>,
    line_normal: Option<DVec3>,
}

pub(crate) fn hint(tool: Tool) -> &'static str {
    match tool {
        Tool::Line => "Trigger: start / next endpoint. Cancel ends the chain.",
        Tool::TapeMeasure => "Trigger: first point, then second point. Cancel clears.",
        Tool::Protractor => "Trigger: centre, baseline, then angle. Cancel clears.",
        Tool::Dimensions => "Trigger: two endpoints, then annotation position.",
        Tool::Select => "Trigger selects. Cancel clears selection.",
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
                for (label, tool) in [
                    ("Select", Tool::Select),
                    ("Line", Tool::Line),
                    ("Tape", Tool::TapeMeasure),
                    ("Angle", Tool::Protractor),
                    ("Dimension", Tool::Dimensions),
                ] {
                    if ui
                        .add_sized(
                            [100., 42.],
                            egui::Button::new(egui::RichText::new(label).size(18.))
                                .selected(self.tool == tool),
                        )
                        .clicked()
                    {
                        self.set_tool(tool);
                        self.persist_inspector_changes(&ctx, Some(storage), false);
                    }
                }
                for (label, action) in [("Cancel", 0), ("Undo", 1), ("Redo", 2)] {
                    if ui
                        .add_sized(
                            [84., 42.],
                            egui::Button::new(egui::RichText::new(label).size(18.)),
                        )
                        .clicked()
                    {
                        self.stop_drawing();
                        self.measurement = None;
                        if action == 0 && self.tool == Tool::Select {
                            self.selected.clear();
                        }
                        if action != 0 {
                            self.undo(&ctx, action == 2);
                        }
                    }
                }
            });
            ui.label(egui::RichText::new(hint(self.tool)).size(16.));
        });
    }

    pub(crate) fn xr_world_pointer(
        &mut self,
        pointer: Option<&Pointer>,
        state: &mut State,
        activate: bool,
        ctx: &egui::Context,
    ) {
        let Some(pointer) = pointer.filter(|_| self.xr_scene_interaction_enabled(ctx)) else {
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
        }
        self.hover = pointer.hit.map(|hit| hit.entity);
        // Desktop reference acquisition uses screen coordinates; don't retain
        // those guides when ownership moves to a model-space controller ray.
        self.drawing_reference = None;
        self.drawing_guide = None;
        self.drawing_axis_guides.clear();
        self.drawing_projection = None;
        if activate && pointer.cancel {
            self.stop_drawing();
            self.measurement = None;
            state.line_normal = None;
            if self.tool == Tool::Select {
                self.selected.clear();
            }
            ctx.request_repaint();
            return;
        }
        let clicked = activate && pointer.activate;
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
            Tool::Line | Tool::TapeMeasure | Tool::Protractor | Tool::Dimensions
        ) {
            self.snap = None;
            if clicked {
                self.say(ctx, "This tool currently uses the editor panel. Choose Line, Tape Measure, Protractor or Dimensions to edit in 3D.");
            }
            return;
        }
        if clicked && self.measurement.as_ref().is_some_and(|m| m.complete) {
            self.measurement = None;
            self.length_input.clear();
        }
        let start = if self.tool == Tool::Line {
            self.line_start
        } else {
            self.measurement
                .as_ref()
                .filter(|m| !m.complete)
                .and_then(|m| m.points.first().copied())
        };
        if self.tool == Tool::Line && start.is_none() {
            state.line_normal = None;
        }
        let normal = self
            .measurement_plane()
            .map(|p| p.1)
            .or(state.line_normal)
            .unwrap_or(DVec3::Z);
        self.snap = self.xr_ray_snap(pointer, start, normal);
        self.apply_grid_snap();
        let valid_offset = self.xr_measurement_offset(pointer.ray);
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

    fn xr_ray_snap(&self, pointer: &Pointer, start: Option<DVec3>, normal: DVec3) -> Option<Snap> {
        let ray = pointer.ray;
        let constraint = (self.tool != Tool::Dimensions)
            .then(|| self.axis_constraint())
            .flatten();
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
