//! Operation-specific fixed modeling snapshots keep the changing preview out
//! of targets; their world indexes remain unchanged while dragging.
use benchup_core::{Axis, AxisGuide, DVec3, DisplaySettings, Inference, Model, Ray, Snap, inference::AxisConstraint};
use std::sync::Arc;
use crate::renderer;

#[derive(Clone, Copy)]
pub(crate) struct Options {
    pub geometry: bool,
    pub targets: benchup_core::inference::SnapTargets,
    pub axes: bool,
    pub x_ray: bool,
}

pub(crate) struct Target {
    pub snap: Snap,
    pub guides: Vec<AxisGuide>,
    pub edge: Option<u64>,
    pub face: Option<u64>,
}

#[derive(Default)]
pub(crate) struct Targets {
    source: Option<Arc<Model>>,
    revision: u64,
    faces: renderer::FaceCache,
    entities: renderer::EntityCache,
    picking: crate::picking::PickingCache,
    edges: super::world_picking::Index,
    references: super::world_inference::References,
    display: DisplaySettings,
    sections: crate::app::display_scene::SectionCache,
    caps: super::section_picking::Index,
}

impl Targets {
    pub fn prepare(&mut self, model: Arc<Model>, display: &DisplaySettings) {
        if self.source.as_ref().is_none_or(|old| !Arc::ptr_eq(old, &model)) {
            self.revision = self.revision.wrapping_add(1);
            self.faces.update_for_revision(&model, self.revision);
            self.entities.update_for_revision(&model, self.revision);
            self.edges.prepare(&model);
            self.references.prepare(&model);
            self.picking.update(&self.faces);
            self.source = Some(model);
        }
        self.display = display.clone();
        self.picking.set_display(display);
        self.sections.geometry(&self.faces, &self.entities, display);
        self.caps.prepare(self.sections.generation, &self.sections.triangles, &self.sections.triangle_owners);
    }

    #[cfg(test)]
    pub fn infer(&self, ray: Ray, anchor: DVec3, point: DVec3, constraint: Option<AxisConstraint>,
        geometry: bool, edges: bool, axes: bool) -> Option<Target>
    {
        self.infer_with_visibility(ray, anchor, point, constraint, geometry, edges, axes, true)
    }

    #[cfg(test)]
    pub fn infer_with_visibility(&self, ray: Ray, anchor: DVec3, point: DVec3, constraint: Option<AxisConstraint>,
        geometry: bool, edges: bool, axes: bool, x_ray: bool) -> Option<Target>
    {
        self.infer_with_options(ray, anchor, point, constraint, Options { geometry, axes, x_ray,
            targets: benchup_core::inference::SnapTargets { edges, faces: true, face_centers: true } })
    }

    pub fn infer_with_options(&self, ray: Ray, anchor: DVec3, point: DVec3, constraint: Option<AxisConstraint>,
        options: Options) -> Option<Target>
    {
        self.infer_on_plane(ray, anchor, point, constraint, None, options)
    }

    pub fn rotation(&self, ray: Ray, pivot: DVec3, normal: DVec3, point: DVec3, options: Options) -> Option<Target> {
        let in_plane = self.infer_on_plane(ray, pivot, point, None, Some((pivot, normal)), options)
            .filter(|target| target.snap.point.distance_squared(pivot) > 1e-12);
        if in_plane.is_some() { return in_plane; }
        if !options.geometry { return None; }
        let mut projected = self.infer_with_options(ray, pivot, point, None, Options { axes: false, ..options })?;
        projected.snap.point -= normal * (projected.snap.point - pivot).dot(normal);
        if projected.snap.point.distance_squared(pivot) < 1e-12 { return None; }
        // The projected point is not on the original edge/face. Grid refinement
        // must not pull it back out of the rotation plane through that geometry.
        projected.edge = None;
        projected.face = None;
        Some(projected)
    }

    pub fn offset(&self, ray: Ray, anchor: DVec3, normal: DVec3, point: DVec3,
        constraint: Option<AxisConstraint>, options: Options) -> Option<Target>
    {
        self.infer_on_plane(ray, anchor, point, constraint, constraint.is_none().then_some((anchor, normal)), options)
    }

    fn infer_on_plane(&self, ray: Ray, anchor: DVec3, point: DVec3, constraint: Option<AxisConstraint>,
        plane_override: Option<(DVec3, DVec3)>, options: Options) -> Option<Target>
    {
        let Options { geometry, targets, axes, x_ray } = options;
        let edges = targets.edges;
        let model = self.source.as_ref()?;
        let allowed = |point: DVec3| self.display.section_contains(point)
            && constraint.is_none_or(|c| c.contains(point - anchor))
            && plane_override.is_none_or(|(origin, normal)| (point - origin).dot(normal).abs() < 1e-7);
        let near = |point: DVec3| {
            let delta = point - ray.origin;
            let depth = delta.dot(ray.direction);
            let score = (delta - ray.direction * depth).length() / depth;
            (depth > 0. && score <= 0.01 && allowed(point) && self.visible(ray.origin, point, x_ray)).then_some(score)
        };
        // Desktop Move uses X-ray targets; Scale uses visible targets. Hidden
        // state and editing scope were resolved by MoveSnapCache, and section
        // clipping still applies; the moving preview is absent from this set.
        if geometry && edges && let Some(hit) = self.edges.query(ray, 0.01, |id, a, b| {
            let state = self.entities.get(id);
            if state.hidden || state.context.is_none() { return None; }
            let (a, b) = self.display.section_segment(a, b)?;
            let (a, b) = constrained_segment(a, b, anchor, constraint)?;
            if let Some((origin, normal)) = plane_override { plane_segment(a, b, origin, normal) } else { Some((a, b)) }
        }, |_, point| allowed(point) && self.visible(ray.origin, point, x_ray)) {
            let (a, b) = model.points(&model.edges[&hit.id]);
            let exact = [(a, Inference::Endpoint), (b, Inference::Endpoint), ((a + b) * 0.5, Inference::Midpoint)]
                .into_iter().filter_map(|(point, kind)| near(point).map(|score| (point, kind, score)))
                .min_by(|a, b| a.2.total_cmp(&b.2));
            let snap = exact.map_or(Snap { point: hit.point, kind: Inference::OnEdge },
                |(point, kind, _)| Snap { point, kind });
            return Some(Target { snap, guides: vec![], edge: Some(hit.id), face: None });
        }
        let mut face_target = None;
        if geometry && (targets.faces || targets.face_centers)
            && let Some((id, _, hit)) = self.picking.nearest(&self.faces, &self.entities, ray, true) {
            let face = self.faces.get(id);
            let point = if let Some((origin, normal)) = plane_override {
                plane_face(ray, origin, normal, face.normal, hit)
            } else { constrained_face(ray, anchor, constraint, face.normal, hit) };
            if let Some(point) = point
                && face.contains_point(point) && near(point).is_some() {
                let center = targets.face_centers.then(|| self.faces.snap_center(model, id)).flatten()
                    .filter(|&center| near(center).is_some());
                let target = Target { snap: center.map_or(Snap { point, kind: Inference::OnFace },
                    |point| Snap { point, kind: Inference::FaceCenter }), guides: vec![], edge: None, face: Some(id) };
                if center.is_some() { return Some(target); }
                if targets.faces { face_target = Some(target); }
            }
        }
        if !axes { return face_target; }
        let plane = if let Some(plane) = plane_override { plane } else if let Some(target) = &face_target {
            (target.snap.point, self.faces.get(target.face.unwrap()).normal)
        } else { match constraint {
            Some(AxisConstraint::Plane(normal)) => (anchor, normal.vector()),
            _ => (point, ray.direction),
        } };
        let mut guides: Vec<_> = [anchor, DVec3::ZERO].into_iter().flat_map(|origin|
            [Axis::X, Axis::Y, Axis::Z].map(|axis| AxisGuide { origin, axis })).collect();
        if geometry && edges {
            guides.extend(self.references.candidates(ray, plane, constraint.map(|c| (anchor, c)), |id, reference| {
                let state = self.entities.get(id);
                !state.hidden && state.context.is_some() && self.visible(ray.origin, reference, x_ray)
            }).into_iter().flatten());
        }
        if let Some((snap, guides)) = super::world_inference::align_guides_filtered(ray, plane, constraint.map(|c| (anchor, c)), &guides,
            |point| allowed(point) && self.visible(ray.origin, point, x_ray)
                && face_target.as_ref().is_none_or(|target| self.faces.get(target.face.unwrap()).contains_point(point)))
            && allowed(snap.point) && self.visible(ray.origin, snap.point, x_ray) {
            return Some(Target { snap, guides, edge: None, face: None });
        }
        face_target
    }

    fn visible(&self, origin: DVec3, point: DVec3, x_ray: bool) -> bool {
        if !self.display.section_contains(point) { return false; }
        if x_ray { return true; }
        let distance = point.distance(origin);
        if distance < 1e-12 { return false; }
        let ray = Ray { origin, direction: (point - origin) / distance };
        self.caps.unoccluded(origin, point) && self.picking.nearest(&self.faces, &self.entities, ray, false)
            .is_none_or(|(_, depth, _)| depth + 1e-5 >= distance)
    }
}

fn constrained_segment(a: DVec3, b: DVec3, anchor: DVec3, constraint: Option<AxisConstraint>) -> Option<(DVec3, DVec3)> {
    let Some(c) = constraint else { return Some((a, b)); };
    let residual = |point: DVec3| point - anchor - c.project(point - anchor);
    let first = residual(a);
    let last = residual(b);
    if first.length() < 1e-7 && last.length() < 1e-7 { return Some((a, b)); }
    let delta = last - first;
    if delta.length_squared() < 1e-24 { return None; }
    let t = -first.dot(delta) / delta.length_squared();
    let point = a.lerp(b, t);
    ((0.0..=1.0).contains(&t) && residual(point).length() < 1e-7).then_some((point, point))
}

fn constrained_face(ray: Ray, anchor: DVec3, constraint: Option<AxisConstraint>, normal: DVec3, hit: DVec3) -> Option<DVec3> {
    match constraint {
        None => Some(hit),
        Some(AxisConstraint::Axis(axis)) => {
            let denominator = axis.vector().dot(normal);
            if denominator.abs() < 1e-9 {
                ((anchor - hit).dot(normal).abs() < 1e-7).then(|| ray.axis(anchor, axis.vector())).flatten()
            } else { Some(anchor + axis.vector() * (hit - anchor).dot(normal) / denominator) }
        }
        Some(AxisConstraint::Plane(axis)) => {
            plane_face(ray, anchor, axis.vector(), normal, hit)
        }
    }
}

fn plane_segment(a: DVec3, b: DVec3, origin: DVec3, normal: DVec3) -> Option<(DVec3, DVec3)> {
    let first = (a - origin).dot(normal);
    let last = (b - origin).dot(normal);
    if first.abs() < 1e-7 && last.abs() < 1e-7 { return Some((a, b)); }
    if (last - first).abs() < 1e-12 { return None; }
    let t = -first / (last - first);
    (0.0..=1.0).contains(&t).then(|| { let point = a.lerp(b, t); (point, point) })
}

fn plane_face(ray: Ray, origin: DVec3, n: DVec3, normal: DVec3, hit: DVec3) -> Option<DVec3> {
    let direction = n.cross(normal);
    if direction.length_squared() < 1e-18 {
        ((hit - origin).dot(n).abs() < 1e-7).then_some(hit)
    } else {
        let tangent = normal - n * normal.dot(n);
        let origin = origin + tangent * (hit - origin).dot(normal) / tangent.length_squared();
        ray.axis(origin, direction.normalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn world_rotate_targets_support_arbitrary_world_planes_and_preferences() {
        let normal = DVec3::new(1., 0., 1.).normalize();
        let point = DVec3::new(1., 1., -1.);
        let mut model = Model::default();
        let edge = model.add_edge(point, point + DVec3::Y).unwrap();
        let mut targets = Targets::default();
        targets.prepare(Arc::new(model), &DisplaySettings::default());
        let ray = Ray { origin: point + normal * 2., direction: -normal };
        let options = Options { geometry: true, targets: benchup_core::inference::SnapTargets::default(), axes: false, x_ray: false };
        let target = targets.rotation(ray, DVec3::ZERO, normal, point, options).unwrap();
        assert_eq!(target.edge, Some(edge));
        assert_eq!(target.snap.kind, Inference::Endpoint);
        assert!(target.snap.point.distance(point) < 1e-8);
        assert!(target.snap.point.dot(normal).abs() < 1e-8);
        let disabled = Options { targets: benchup_core::inference::SnapTargets { edges: false, faces: false, face_centers: false }, ..options };
        assert!(targets.rotation(ray, DVec3::ZERO, normal, point, disabled).is_none());
        let off_plane = point + normal;
        let ray = Ray { origin: off_plane + normal * 2., direction: -normal };
        assert!(targets.rotation(ray, point, normal, point, options).is_none());
    }
    #[test]
    fn modeling_target_preferences_keep_faces_and_face_centres_independent() {
        let mut model = Model::default();
        model.draw_polyline(&[DVec3::ZERO, DVec3::X * 2., DVec3::new(2., 2., 0.), DVec3::Y * 2., DVec3::ZERO], false).unwrap();
        let mut service = Targets::default();
        service.prepare(Arc::new(model), &DisplaySettings::default());
        let ray = Ray { origin: DVec3::new(1., 1., 2.), direction: -DVec3::Z };
        for (faces, centres, expected) in [(false, false, None), (true, false, Some(Inference::OnFace)),
            (false, true, Some(Inference::FaceCenter)), (true, true, Some(Inference::FaceCenter))] {
            let options = Options { geometry: true, axes: false, x_ray: false,
                targets: benchup_core::inference::SnapTargets { edges: false, faces, face_centers: centres } };
            let result = service.infer_with_options(ray, DVec3::ZERO, DVec3::ONE, None, options);
            assert_eq!(result.map(|target| target.snap.kind), expected);
        }
        let off_centre = Ray { origin: DVec3::new(0.4, 0.5, 2.), ..ray };
        assert!(service.infer_with_options(off_centre, DVec3::ZERO, DVec3::ONE, None,
            Options { geometry: true, axes: false, x_ray: false,
                targets: benchup_core::inference::SnapTargets { edges: false, faces: false, face_centers: true } }).is_none());
    }
    #[test]
    fn world_scale_targets_respect_filled_section_caps() {
        let mut model = Model::default();
        model.draw_polyline(&[DVec3::ZERO, DVec3::X * 2., DVec3::new(2., 2., 0.), DVec3::Y * 2., DVec3::ZERO], false).unwrap();
        let face = *model.faces.keys().next().unwrap();
        model.push_pull(face, 2.).unwrap();
        let edge = model.add_edge(DVec3::new(1., 1., 0.25), DVec3::new(1., 1.5, 0.25)).unwrap();
        let display = DisplaySettings { section_cuts: true, section_fill: true, section_axes: [false, false, true],
            section_height: 0.75, ..Default::default() };
        let mut targets = Targets::default();
        targets.prepare(Arc::new(model), &display);
        let ray = Ray { origin: DVec3::new(1., 1., 3.), direction: -DVec3::Z };
        assert!(targets.caps.nearest(ray, f64::INFINITY).is_some());
        assert!(targets.infer_with_visibility(ray, DVec3::ZERO, DVec3::ONE, None, true, true, false, false).is_none());
        let xray = targets.infer_with_visibility(ray, DVec3::ZERO, DVec3::ONE, None, true, true, false, true).unwrap();
        assert_eq!(xray.edge, Some(edge));
    }

    #[test]
    fn world_scale_targets_respect_stationary_face_occlusion_and_xray_preference() {
        let mut model = Model::default();
        let edge = model.add_edge(DVec3::new(1., 1., 0.), DVec3::new(1., 2., 0.)).unwrap();
        model.draw_polyline(&[DVec3::Z, DVec3::new(3., 0., 1.), DVec3::new(3., 3., 1.),
            DVec3::new(0., 3., 1.), DVec3::Z], false).unwrap();
        let mut targets = Targets::default();
        targets.prepare(Arc::new(model), &DisplaySettings::default());
        let ray = Ray { origin: DVec3::new(1., 1., 2.), direction: -DVec3::Z };
        let visible = targets.infer_with_visibility(ray, DVec3::ZERO, DVec3::new(1., 1., 0.), None, true, true, false, false).unwrap();
        assert!(visible.edge.is_none());
        assert!(visible.face.is_some());
        assert!((visible.snap.point.z - 1.).abs() < 1e-8);
        let xray = targets.infer_with_visibility(ray, DVec3::ZERO, DVec3::new(1., 1., 0.), None, true, true, false, true).unwrap();
        assert_eq!(xray.edge, Some(edge));
        assert_eq!(xray.snap.kind, Inference::Endpoint);
        assert!(xray.snap.point.z.abs() < 1e-8);
    }
    #[test]
    fn world_move_stationary_snapshot_excludes_preview_and_keeps_exact_targets() {
        let mut model = Model::default();
        let moving = model.add_edge(DVec3::ZERO, DVec3::Y).unwrap();
        let target = model.add_edge(DVec3::X * 2., DVec3::new(2., 2., 0.)).unwrap();
        let cache = benchup_core::inference::MoveSnapCache::excluding(&model, &std::collections::BTreeSet::from([moving]));
        let geometry = cache.shared_geometry();
        assert!(!geometry.edges.contains_key(&moving));
        assert!(Arc::ptr_eq(&geometry, &cache.shared_geometry()));
        let mut targets = Targets::default();
        targets.prepare(Arc::clone(&geometry), &DisplaySettings::default());
        let revision = targets.revision;
        targets.prepare(geometry, &DisplaySettings::default());
        assert_eq!(targets.revision, revision);
        for (y, kind) in [(0., Inference::Endpoint), (1., Inference::Midpoint), (0.4, Inference::OnEdge)] {
            let ray = Ray { origin: DVec3::new(2., y, 2.), direction: -DVec3::Z };
            let hit = targets.infer(ray, DVec3::ZERO, DVec3::new(2., y, 0.), None, true, true, false).unwrap();
            assert_eq!(hit.edge, Some(target));
            assert_eq!(hit.snap.kind, kind);
            assert!(hit.snap.point.distance(DVec3::new(2., y, 0.)) < 1e-8);
            assert!(targets.infer(ray, DVec3::ZERO, DVec3::ZERO, None, false, false, false).is_none());
        }
    }

    #[test]
    fn world_move_constraints_intersect_edges_and_faces_in_world_space() {
        let x = Some(AxisConstraint::Axis(Axis::X));
        let (a, b) = constrained_segment(DVec3::new(2., -1., 0.), DVec3::new(2., 1., 0.), DVec3::ZERO, x).unwrap();
        assert_eq!(a, DVec3::X * 2.); assert_eq!(a, b);
        assert!(constrained_segment(DVec3::new(2., -1., 1.), DVec3::new(2., 1., 1.), DVec3::ZERO, x).is_none());
        let ray = Ray { origin: DVec3::new(2., 0.004, 2.), direction: -DVec3::Z };
        assert_eq!(constrained_face(ray, DVec3::ZERO, x, DVec3::X, DVec3::X * 2.), Some(DVec3::X * 2.));
        let plane = Some(AxisConstraint::Plane(Axis::Y));
        assert_eq!(constrained_face(ray, DVec3::ZERO, plane, DVec3::Z, DVec3::ZERO), Some(DVec3::X * 2.));
        assert!(constrained_face(ray, DVec3::ZERO, plane, DVec3::Y, DVec3::Y).is_none());
    }

    #[test]
    fn world_move_face_centres_and_global_guides_respect_target_toggles() {
        let mut model = Model::default();
        model.draw_polyline(&[DVec3::ZERO, DVec3::X * 2., DVec3::new(2., 2., 0.), DVec3::Y * 2., DVec3::ZERO], false).unwrap();
        let mut targets = Targets::default();
        targets.prepare(Arc::new(model), &DisplaySettings::default());
        let ray = Ray { origin: DVec3::new(1., 1., 2.), direction: -DVec3::Z };
        let hit = targets.infer(ray, DVec3::ZERO, DVec3::ONE, None, true, true, true).unwrap();
        assert_eq!(hit.snap.kind, Inference::FaceCenter);
        assert_eq!(hit.snap.point, DVec3::new(1., 1., 0.));
        let ray = Ray { origin: DVec3::new(3., 0.004, 2.), direction: -DVec3::Z };
        let hit = targets.infer(ray, DVec3::ZERO, DVec3::X * 3., None, false, false, true).unwrap();
        assert_eq!(hit.snap.kind, Inference::Aligned);
        assert!(!hit.guides.is_empty());
        assert!(hit.snap.point.y.abs() < 1e-8);
    }
}
