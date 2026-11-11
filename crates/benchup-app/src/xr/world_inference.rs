//! Bounded model-space reference inference, independent of a desktop camera.
use benchup_core::{Axis, AxisGuide, DVec3, Inference, Ray, Snap, inference::AxisConstraint};
use std::sync::Arc;

pub(crate) trait ReferenceQuery {
    fn alignment_candidates(&self, ray: Ray, plane: (DVec3, DVec3), constraint: Option<(DVec3, AxisConstraint)>) -> [Vec<AxisGuide>; 3];
    fn contains_face(&self, _origin: DVec3, _face: u64, _point: DVec3) -> Option<bool> { None }
}

/// Only a small per-axis result set reaches the shared tool callback. Combine
/// intersecting lines there so their world coordinates, not screen projections,
/// define the endpoint and its retained guide overlays.
#[cfg(test)]
pub(crate) fn align_guides(ray: Ray, plane: (DVec3, DVec3), constraint: Option<(DVec3, AxisConstraint)>,
    guides: &[AxisGuide]) -> Option<(Snap, Vec<AxisGuide>)>
{
    align_guides_filtered(ray, plane, constraint, guides, |_| true)
}

pub(crate) fn align_guides_filtered(ray: Ray, plane: (DVec3, DVec3), constraint: Option<(DVec3, AxisConstraint)>,
    guides: &[AxisGuide], allowed: impl Fn(DVec3) -> bool) -> Option<(Snap, Vec<AxisGuide>)>
{
    let near = |point: DVec3| {
        let delta = point - ray.origin;
        let depth = delta.dot(ray.direction);
        let score = (delta - ray.direction * depth).length() / depth;
        (depth > 0. && score <= 0.01 && (point - plane.0).dot(plane.1).abs() < 1e-6
            && constraint.is_none_or(|(start, c)| c.contains(point - start))).then_some(score)
    };
    let mut unique: Vec<(AxisGuide, f64, usize)> = Vec::with_capacity(72);
    for (index, guide) in guides.iter().enumerate() {
        if unique.iter().any(|(old, _, _)| old.axis == guide.axis
            && (old.origin - guide.origin).cross(guide.axis.vector()).length() < 1e-7) { continue; }
        let score = axis_point(ray, guide.origin, guide.axis, constraint, Some(plane)).and_then(near).unwrap_or(f64::INFINITY);
        unique.push((*guide, score, index));
    }
    unique.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.cmp(&b.2)));
    let mut counts = [0; 3];
    let guides: Vec<_> = unique.into_iter().filter_map(|(guide, _, _)| {
        let index = match guide.axis { Axis::X => 0, Axis::Y => 1, Axis::Z => 2 };
        if counts[index] >= benchup_core::inference::MAX_ALIGNMENT_REFERENCES_PER_AXIS { return None; }
        counts[index] += 1; Some(guide)
    }).collect();
    let mut candidates = Vec::new();
    let mut consider = |point: DVec3, first: usize, second: Option<usize>| {
        let Some(score) = near(point) else { return; };
        candidates.push((point, first, second, score));
    };
    for (i, guide) in guides.iter().enumerate() {
        if let Some(point) = axis_point(ray, guide.origin, guide.axis, constraint, Some(plane)) {
            consider(point, i, None);
        }
        for (j, second) in guides.iter().enumerate().skip(i + 1) {
            if guide.axis == second.axis { continue; }
            let Some(point) = (Ray { origin: guide.origin, direction: guide.axis.vector() })
                .axis(second.origin, second.axis.vector()) else { continue; };
            let on_line = |g: &AxisGuide| {
                let delta = point - g.origin;
                (delta - g.axis.vector() * delta.dot(g.axis.vector())).length() < 1e-6
            };
            if !on_line(guide) || !on_line(second) { continue; }
            consider(point, i, Some(j));
        }
    }
    candidates.sort_by(|a, b| b.2.is_some().cmp(&a.2.is_some()).then(a.3.total_cmp(&b.3))
        .then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    for (point, first, second, _) in candidates {
        if !allowed(point) { continue; }
        let mut active = vec![guides[first]];
        if let Some(second) = second {
            active.push(guides[second]);
            if let Some(third) = guides.iter().find(|g| g.axis != guides[first].axis && g.axis != guides[second].axis
                && (point - g.origin).cross(g.axis.vector()).length() < 1e-7 && point.distance(g.origin) > 1e-7) {
                active.push(*third);
            }
        }
        return Some((Snap { point, kind: Inference::Aligned }, active));
    }
    None
}

#[derive(Default)]
pub(super) struct References {
    source: Option<Arc<benchup_core::Model>>,
    anchors: Vec<(u64, DVec3)>,
    index: super::world_picking::Index,
}

impl References {
    pub fn matches(&self, model: &Arc<benchup_core::Model>) -> bool {
        self.source.as_ref().is_some_and(|source| Arc::ptr_eq(source, model))
    }
    pub fn prepare(&mut self, model: &Arc<benchup_core::Model>) {
        if self.matches(model) { return; }
        self.anchors.clear();
        for edge in model.edges.values() {
            let (a, b) = model.points(edge);
            self.anchors.extend([a, b, (a + b) * 0.5].map(|point| (edge.id, point)));
        }
        self.index.replace_points(self.anchors.iter().enumerate().map(|(slot, &(_, point))| (slot as u64, point)));
        self.source = Some(Arc::clone(model));
    }

    #[cfg(test)]
    pub fn alignment(&self, ray: Ray, plane: (DVec3, DVec3), constraint: Option<(DVec3, AxisConstraint)>,
        visible: impl Fn(u64, DVec3) -> bool) -> [Option<AxisGuide>; 3]
    {
        self.candidates(ray, plane, constraint, visible).map(|guides| guides.into_iter().next())
    }

    pub fn candidates(&self, ray: Ray, plane: (DVec3, DVec3), constraint: Option<(DVec3, AxisConstraint)>,
        visible: impl Fn(u64, DVec3) -> bool) -> [Vec<AxisGuide>; 3]
    {
        [Axis::X, Axis::Y, Axis::Z].map(|axis| {
            let Some(transform) = axis_projection(ray, axis, plane, constraint) else { return Vec::new(); };
            self.index.query_mapped_candidates(ray, 0.01, transform, benchup_core::inference::MAX_ALIGNMENT_REFERENCES_PER_AXIS, |slot, reference, point| {
                let expected = axis_point(ray, reference, axis, constraint, Some(plane));
                expected.is_some_and(|expected| expected.distance(point) < 1e-6)
                    && visible(self.anchors[slot as usize].0, reference)
            }, |a, b| (self.anchors[a as usize].1 - self.anchors[b as usize].1).cross(axis.vector()).length() < 1e-7)
                .into_iter().map(|hit| AxisGuide { origin: self.anchors[hit.id as usize].1, axis }).collect()
        })
    }
}

// The valid target on an axis is affine in its reference. Invalid plane/lock
// combinations are rejected exactly in the leaf callback, after bounds pruning.
fn axis_projection(ray: Ray, axis: Axis, plane: (DVec3, DVec3), constraint: Option<(DVec3, AxisConstraint)>) -> Option<glam::DMat4> {
    let direction = axis.vector();
    let lock_plane = constraint.and_then(|(start, c)| match c {
        AxisConstraint::Plane(normal) => Some((start, normal.vector())), _ => None,
    });
    let intersect = [Some(plane), lock_plane].into_iter().flatten()
        .find(|(_, normal)| direction.dot(*normal).abs() > 1e-9);
    let fixed = constraint.and_then(|(start, c)| match c {
        AxisConstraint::Axis(lock) if lock != axis => Some(start), _ => None,
    });
    let perpendicular = ray.direction - direction * ray.direction.dot(direction);
    if fixed.is_none() && intersect.is_none() && perpendicular.length_squared() < 1e-18 { return None; }
    let map = |reference: DVec3| {
        if let Some(start) = fixed { reference + direction * (start - reference).dot(direction) }
        else if let Some((origin, normal)) = intersect {
            reference + direction * (origin - reference).dot(normal) / direction.dot(normal)
        } else {
            let depth = (reference - ray.origin).dot(perpendicular) / perpendicular.length_squared();
            reference + direction * (ray.origin + ray.direction * depth - reference).dot(direction)
        }
    };
    let origin = map(DVec3::ZERO);
    Some(glam::DMat4::from_cols((map(DVec3::X) - origin).extend(0.), (map(DVec3::Y) - origin).extend(0.),
        (map(DVec3::Z) - origin).extend(0.), origin.extend(1.)))
}

/// A hovered reference supplies a coordinate even if the locked endpoint is
/// away from the ray. After acquisition, the ray must approach that endpoint.
pub(crate) fn reference_projection(
    ray: Ray,
    reference: DVec3,
    start: DVec3,
    axis: Axis,
    plane: Option<(DVec3, DVec3)>,
    acquiring: bool,
) -> Option<Snap> {
    let direction = axis.vector();
    let point = start + direction * (reference - start).dot(direction);
    if point.distance(start) < 1e-7
        || plane.is_some_and(|(origin, normal)| (point - origin).dot(normal).abs() > 1e-6)
    {
        return None;
    }
    let delta = point - ray.origin;
    let depth = delta.dot(ray.direction);
    if depth <= 0. || !depth.is_finite() {
        return None;
    }
    if !acquiring && (delta - ray.direction * depth).length() / depth > 0.01 {
        return None;
    }
    Some(Snap {
        point,
        kind: Inference::Aligned,
    })
}

pub(crate) fn reference_axis(
    ray: Ray,
    reference: DVec3,
    constraint: Option<(DVec3, AxisConstraint)>,
    plane: Option<(DVec3, DVec3)>,
) -> Option<(Snap, AxisGuide)> {
    let near = |point: DVec3| {
        let delta = point - ray.origin;
        let depth = delta.dot(ray.direction);
        let score = (delta - ray.direction * depth).length() / depth;
        (depth > 0. && score <= 0.01).then_some(score)
    };
    [Axis::X, Axis::Y, Axis::Z]
        .into_iter()
        .filter_map(|axis| {
            let point = axis_point(ray, reference, axis, constraint, plane)?;
            let score = near(point)?;
            Some((
                Snap {
                    point,
                    kind: Inference::Aligned,
                },
                AxisGuide {
                    origin: reference,
                    axis,
                },
                score,
            ))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(snap, guide, _)| (snap, guide))
}

pub(crate) fn axis_point(ray: Ray, reference: DVec3, axis: Axis,
    constraint: Option<(DVec3, AxisConstraint)>, plane: Option<(DVec3, DVec3)>) -> Option<DVec3>
{
    let direction = axis.vector();
    let mut point = constraint.and_then(|(start, constraint)| match constraint {
        AxisConstraint::Axis(lock) => Ray { origin: start, direction: lock.vector() }.axis(reference, direction),
        AxisConstraint::Plane(_) => None,
    });
    let constraint_plane = constraint.and_then(|(start, constraint)| match constraint {
        AxisConstraint::Plane(axis) => Some((start, axis.vector())),
        AxisConstraint::Axis(_) => None,
    });
    for (origin, normal) in [plane, constraint_plane].into_iter().flatten() {
        let denominator = direction.dot(normal);
        if denominator.abs() > 1e-9 {
            let intersection = reference + direction * (origin - reference).dot(normal) / denominator;
            if point.is_some_and(|p| p.distance(intersection) > 1e-6) { return None; }
            point = Some(intersection);
        } else if (reference - origin).dot(normal).abs() > 1e-6 { return None; }
    }
    let point = point.or_else(|| ray.axis(reference, direction))?;
    if constraint.is_some_and(|(start, c)| !c.contains(point - start)) { return None; }
    Some(point)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alignment_candidates_rank_builtin_anchors_before_applying_the_limit() {
        let limit = benchup_core::inference::MAX_ALIGNMENT_REFERENCES_PER_AXIS;
        let mut guides = vec![AxisGuide { origin: DVec3::X * -100., axis: Axis::Y }];
        guides.extend((1..=limit).map(|i| AxisGuide { origin: DVec3::new(i as f64 * 0.0001, 1., 0.), axis: Axis::Y }));
        let last = limit as f64 * 0.0001;
        let ray = Ray { origin: DVec3::Z * 2., direction: -DVec3::Z };
        let (snap, _) = align_guides_filtered(ray, (DVec3::ZERO, DVec3::Z), None, &guides,
            |point| (point.x - last).abs() < 1e-8).unwrap();
        assert!((snap.point.x - last).abs() < 1e-8);
    }
    #[test]
    fn alignment_candidates_retain_secondary_intersections_after_surface_rejection() {
        let mut model = benchup_core::Model::default();
        for coordinate in [0., 0.01] {
            model.add_edge(DVec3::new(coordinate, 1., 0.), DVec3::new(coordinate, 2., 0.)).unwrap();
            model.add_edge(DVec3::new(1., coordinate, 0.), DVec3::new(2., coordinate, 0.)).unwrap();
        }
        let mut references = References::default();
        references.prepare(&Arc::new(model));
        let ray = Ray { origin: DVec3::new(0.001, 0.001, 2.), direction: -DVec3::Z };
        let plane = (DVec3::ZERO, DVec3::Z);
        let candidates = references.candidates(ray, plane, None, |_, _| true);
        assert_eq!(candidates[0].len(), 2);
        assert_eq!(candidates[1].len(), 2);
        let guides: Vec<_> = candidates.into_iter().flatten().collect();
        let (snap, active) = align_guides_filtered(ray, plane, None, &guides, |p| p.x > 0.005 && p.y > 0.005).unwrap();
        assert!(snap.point.distance(DVec3::new(0.01, 0.01, 0.)) < 1e-8);
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn alignment_candidates_deduplicate_lines_before_the_shared_desktop_limit() {
        let mut model = benchup_core::Model::default();
        for i in 0..100 {
            let x = i as f64 * 0.0001;
            model.add_edge(DVec3::new(x, 1., 0.), DVec3::new(x, 2., 0.)).unwrap();
        }
        let mut references = References::default();
        references.prepare(&Arc::new(model));
        let ray = Ray { origin: DVec3::Z * 2., direction: -DVec3::Z };
        let candidates = references.candidates(ray, (DVec3::ZERO, DVec3::Z), None, |_, _| true);
        assert_eq!(candidates[1].len(), benchup_core::inference::MAX_ALIGNMENT_REFERENCES_PER_AXIS);
        for (i, guide) in candidates[1].iter().enumerate() {
            assert!((guide.origin.x - i as f64 * 0.0001).abs() < 1e-8);
        }
    }

    #[test]
    fn global_reference_index_finds_off_face_endpoints_and_midpoints_with_visibility_and_locks() {
        let mut model = benchup_core::Model::default();
        let edge = model.add_edge(DVec3::new(2., 3., 0.), DVec3::new(4., 3., 0.)).unwrap();
        let model = Arc::new(model);
        let mut references = References::default();
        references.prepare(&model);
        assert!(references.matches(&model));
        let plane = (DVec3::ZERO, DVec3::Z);
        for x in [2., 3., 4.] {
            let ray = Ray { origin: DVec3::new(x + 0.004, 0., 2.), direction: -DVec3::Z };
            let guides = references.alignment(ray, plane, None, |id, _| id == edge);
            let guide = guides[1].unwrap();
            assert_eq!(guide.origin, DVec3::new(x, 3., 0.));
            let (snap, active) = align_guides(ray, plane, None, &guides.into_iter().flatten().collect::<Vec<_>>()).unwrap();
            assert!(snap.point.distance(DVec3::new(x, 0., 0.)) < 1e-8);
            assert_eq!(active.len(), 1);
            assert!(references.alignment(ray, plane, None, |_, _| false).into_iter().all(|g| g.is_none()));
            let lock = Some((DVec3::ZERO, AxisConstraint::Axis(Axis::Y)));
            assert!(references.alignment(ray, plane, lock, |_, _| true).into_iter().all(|g| g.is_none()));
        }
        let count = references.anchors.len();
        references.prepare(&model);
        assert_eq!(references.anchors.len(), count);
        let empty = Arc::new(benchup_core::Model::default());
        references.prepare(&empty);
        assert!(references.anchors.is_empty());
        assert!(references.matches(&empty));
        assert!(!references.matches(&model));
    }

    #[test]
    fn global_reference_guides_combine_world_intersections_without_accepting_skew_lines() {
        let ray = Ray { origin: DVec3::new(2.003, 3.004, 2.), direction: -DVec3::Z };
        let plane = (DVec3::ZERO, DVec3::Z);
        let guides = [AxisGuide { origin: DVec3::new(0., 3., 0.), axis: Axis::X },
            AxisGuide { origin: DVec3::new(2., 0., 0.), axis: Axis::Y }];
        let (snap, active) = align_guides(ray, plane, None, &guides).unwrap();
        assert!(snap.point.distance(DVec3::new(2., 3., 0.)) < 1e-8);
        assert_eq!(active.len(), 2);
        let skew = [guides[0], AxisGuide { origin: guides[1].origin + DVec3::Z, ..guides[1] }];
        assert_eq!(align_guides(ray, plane, None, &skew).unwrap().1.len(), 1);
        let lock = Some((DVec3::ZERO, AxisConstraint::Plane(Axis::X)));
        assert!(align_guides(ray, plane, lock, &guides).is_none());
    }

    #[test]
    fn global_reference_affine_bounds_match_exact_queries_for_oblique_and_grazing_planes() {
        let mut model = benchup_core::Model::default();
        for x in -8..8 {
            for y in -8..8 {
                let p = DVec3::new(x as f64 * 0.4, y as f64 * 0.4, (x + y) as f64 * 0.1);
                model.add_edge(p, p + DVec3::new(0.1, 0.2, 0.3)).unwrap();
            }
        }
        let mut references = References::default();
        references.prepare(&Arc::new(model));
        for normal in [DVec3::Z, DVec3::new(1., 2., 3.).normalize(), DVec3::X] {
            for direction in [-DVec3::Z, DVec3::new(1., 0., -0.001).normalize()] {
                let ray = Ray { origin: DVec3::new(0.1, 0.2, 2.), direction };
                let plane = (DVec3::ZERO, normal);
                let indexed = references.alignment(ray, plane, None, |_, _| true);
                for (axis, result) in [Axis::X, Axis::Y, Axis::Z].into_iter().zip(indexed) {
                    let mut brute = None::<f64>;
                    for &(_, reference) in &references.anchors {
                        let Some(point) = axis_point(ray, reference, axis, None, Some(plane)) else { continue; };
                        let delta = point - ray.origin;
                        let depth = delta.dot(ray.direction);
                        let score = (delta - ray.direction * depth).length() / depth;
                        if depth > 0. && score <= 0.01 { brute = Some(brute.map_or(score, |old| old.min(score))); }
                    }
                    assert_eq!(result.is_some(), brute.is_some());
                    if let Some(guide) = result {
                        let point = axis_point(ray, guide.origin, axis, None, Some(plane)).unwrap();
                        let delta = point - ray.origin;
                        let depth = delta.dot(ray.direction);
                        let score = (delta - ray.direction * depth).length() / depth;
                        assert!((score - brute.unwrap()).abs() < 1e-8);
                    }
                }
            }
        }
    }
    #[test]
    fn locked_projection_follows_acquired_coordinate_then_requires_nearby_aim() {
        let ray = Ray {
            origin: DVec3::new(2., 3., 2.),
            direction: -DVec3::Z,
        };
        let reference = DVec3::new(2., 3., 0.);
        let plane = Some((DVec3::ZERO, DVec3::Z));
        assert_eq!(
            reference_projection(ray, reference, DVec3::ZERO, Axis::X, plane, true)
                .unwrap()
                .point,
            DVec3::new(2., 0., 0.)
        );
        assert!(reference_projection(ray, reference, DVec3::ZERO, Axis::X, plane, false).is_none());
        let ray = Ray {
            origin: DVec3::new(2., 0.005, 2.),
            ..ray
        };
        assert!(reference_projection(ray, reference, DVec3::ZERO, Axis::X, plane, false).is_some());
    }
    #[test]
    fn locked_projection_rejects_zero_extent_behind_and_off_plane_targets() {
        let ray = Ray {
            origin: DVec3::new(2., 3., 2.),
            direction: -DVec3::Z,
        };
        let reference = DVec3::new(2., 3., 0.);
        assert!(reference_projection(ray, reference, DVec3::ZERO, Axis::Z, None, true).is_none());
        assert!(
            reference_projection(
                Ray {
                    direction: DVec3::Z,
                    ..ray
                },
                reference,
                DVec3::ZERO,
                Axis::X,
                None,
                true
            )
            .is_none()
        );
        assert!(
            reference_projection(
                ray,
                reference,
                DVec3::ZERO,
                Axis::X,
                Some((DVec3::Y, DVec3::Y)),
                true
            )
            .is_none()
        );
    }
    #[test]
    fn reference_axes_intersect_working_planes_and_locked_axes() {
        let ray = Ray {
            origin: DVec3::new(1.006, 1.004, 2.),
            direction: -DVec3::Z,
        };
        let (snap, guide) =
            reference_axis(ray, DVec3::ONE, None, Some((DVec3::ZERO, DVec3::Z))).unwrap();
        assert_eq!(snap.point, DVec3::new(1., 1., 0.));
        assert_eq!(guide.axis, Axis::Z);
        let ray = Ray {
            origin: DVec3::new(2., 0.005, 2.),
            direction: -DVec3::Z,
        };
        let (snap, guide) = reference_axis(
            ray,
            DVec3::new(2., 1., 0.),
            Some((DVec3::ZERO, AxisConstraint::Axis(Axis::X))),
            Some((DVec3::ZERO, DVec3::Z)),
        )
        .unwrap();
        assert_eq!(snap.point, DVec3::new(2., 0., 0.));
        assert_eq!(guide.axis, Axis::Y);
    }
    #[test]
    fn reference_inference_rejects_behind_far_and_incompatible_planes() {
        let ray = Ray {
            origin: DVec3::new(1., 1., 2.),
            direction: DVec3::Z,
        };
        assert!(reference_axis(ray, DVec3::ONE, None, Some((DVec3::ZERO, DVec3::Z))).is_none());
        let ray = Ray {
            direction: -DVec3::Z,
            ..ray
        };
        assert!(
            reference_axis(
                ray,
                DVec3::new(100., 100., 1.),
                None,
                Some((DVec3::ZERO, DVec3::Z))
            )
            .is_none()
        );
        assert!(
            reference_axis(
                ray,
                DVec3::ONE,
                Some((DVec3::ZERO, AxisConstraint::Plane(Axis::X))),
                Some((DVec3::ZERO, DVec3::Z))
            )
            .is_none()
        );
    }
}
