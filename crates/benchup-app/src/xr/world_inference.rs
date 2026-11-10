//! Bounded model-space reference inference, independent of a desktop camera.
use benchup_core::{Axis, AxisGuide, DVec3, Inference, Ray, Snap, inference::AxisConstraint};

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
            let direction = axis.vector();
            let mut point = constraint.and_then(|(start, constraint)| match constraint {
                AxisConstraint::Axis(lock) => Ray {
                    origin: start,
                    direction: lock.vector(),
                }
                .axis(reference, direction),
                AxisConstraint::Plane(_) => None,
            });
            let constraint_plane = constraint.and_then(|(start, constraint)| match constraint {
                AxisConstraint::Plane(axis) => Some((start, axis.vector())),
                AxisConstraint::Axis(_) => None,
            });
            for (origin, normal) in [plane, constraint_plane].into_iter().flatten() {
                let denominator = direction.dot(normal);
                if denominator.abs() > 1e-9 {
                    let intersection =
                        reference + direction * (origin - reference).dot(normal) / denominator;
                    if point.is_some_and(|p| p.distance(intersection) > 1e-6) {
                        return None;
                    }
                    point = Some(intersection);
                } else if (reference - origin).dot(normal).abs() > 1e-6 {
                    return None;
                }
            }
            let point = point.or_else(|| ray.axis(reference, direction))?;
            if constraint.is_some_and(|(start, c)| !c.contains(point - start)) {
                return None;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
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
