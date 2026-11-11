//! Keep application coordinates stable when the runtime changes LOCAL's origin.
//! The event's pose maps the new natural origin into the previous natural frame;
//! the application offset becomes inverse(change) * previous_offset at changeTime.
use glam::{DMat4, DQuat, DVec3};
use openxr as xr;
use std::collections::VecDeque;

struct Pending {
    time: xr::Time,
    previous_from_new: Option<DMat4>,
}

pub(super) struct Rebase {
    pub offset: xr::Posef,
    pub reset_world: bool,
}

pub(super) struct Changes {
    natural_from_app: DMat4,
    pending: VecDeque<Pending>,
}

impl Default for Changes {
    fn default() -> Self {
        Self {
            natural_from_app: DMat4::IDENTITY,
            pending: VecDeque::new(),
        }
    }
}

impl Changes {
    pub fn schedule(&mut self, time: xr::Time, pose: Option<xr::Posef>) {
        let previous_from_new = pose.and_then(|pose| {
            let p = pose.position;
            let q = pose.orientation;
            let position = DVec3::new(p.x as f64, p.y as f64, p.z as f64);
            let orientation = DQuat::from_xyzw(q.x as f64, q.y as f64, q.z as f64, q.w as f64);
            (position.is_finite()
                && orientation.is_finite()
                && orientation.length_squared() > 1e-12)
                .then(|| DMat4::from_rotation_translation(orientation.normalize(), position))
        });
        let index = self
            .pending
            .iter()
            .position(|event| event.time.as_nanos() > time.as_nanos())
            .unwrap_or(self.pending.len());
        self.pending.insert(
            index,
            Pending {
                time,
                previous_from_new,
            },
        );
    }

    pub fn take_due(&mut self, time: xr::Time) -> Option<Rebase> {
        let mut changed = false;
        let mut reset_world = false;
        while self
            .pending
            .front()
            .is_some_and(|event| event.time.as_nanos() <= time.as_nanos())
        {
            let event = self.pending.pop_front().unwrap();
            changed = true;
            if let Some(previous_from_new) = event.previous_from_new {
                self.natural_from_app = previous_from_new.inverse() * self.natural_from_app;
            } else {
                // No physical-space continuity can be inferred. Start a new
                // application frame and refit the scene beside the user's head.
                self.natural_from_app = DMat4::IDENTITY;
                reset_world = true;
            }
        }
        changed.then(|| {
            let (_, rotation, position) = self.natural_from_app.to_scale_rotation_translation();
            let rotation = rotation.normalize();
            Rebase {
                offset: xr::Posef {
                    orientation: xr::Quaternionf {
                        x: rotation.x as f32,
                        y: rotation.y as f32,
                        z: rotation.z as f32,
                        w: rotation.w as f32,
                    },
                    position: xr::Vector3f {
                        x: position.x as f32,
                        y: position.y as f32,
                        z: position.z as f32,
                    },
                },
                reset_world,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(position: DVec3, yaw: f64) -> xr::Posef {
        let q = DQuat::from_rotation_y(yaw);
        xr::Posef {
            orientation: xr::Quaternionf {
                x: q.x as f32,
                y: q.y as f32,
                z: q.z as f32,
                w: q.w as f32,
            },
            position: xr::Vector3f {
                x: position.x as f32,
                y: position.y as f32,
                z: position.z as f32,
            },
        }
    }

    fn matrix(pose: xr::Posef) -> DMat4 {
        let p = pose.position;
        let q = pose.orientation;
        DMat4::from_rotation_translation(
            DQuat::from_xyzw(q.x as f64, q.y as f64, q.z as f64, q.w as f64).normalize(),
            DVec3::new(p.x as f64, p.y as f64, p.z as f64),
        )
    }

    #[test]
    fn changes_apply_at_predicted_time_and_preserve_physical_placement_in_order() {
        let first = pose(DVec3::new(2., 0.3, -1.), 0.7);
        let second = pose(DVec3::new(-0.8, 0., 1.4), -1.2);
        let mut changes = Changes::default();
        changes.schedule(xr::Time::from_nanos(20), Some(second));
        changes.schedule(xr::Time::from_nanos(10), Some(first));
        assert!(changes.take_due(xr::Time::from_nanos(9)).is_none());
        let rebase = changes.take_due(xr::Time::from_nanos(10)).unwrap();
        assert!(!rebase.reset_world);
        assert!((matrix(first) * matrix(rebase.offset)).abs_diff_eq(DMat4::IDENTITY, 1e-6));
        assert!(changes.take_due(xr::Time::from_nanos(19)).is_none());
        let rebase = changes.take_due(xr::Time::from_nanos(21)).unwrap();
        assert!(
            (matrix(first) * matrix(second) * matrix(rebase.offset))
                .abs_diff_eq(DMat4::IDENTITY, 1e-6)
        );
        assert!(changes.take_due(xr::Time::from_nanos(22)).is_none());
    }

    #[test]
    fn unknown_or_malformed_changes_request_refit_and_compose_later_valid_changes() {
        for invalid in [
            None,
            Some(pose(DVec3::splat(f64::NAN), 0.)),
            Some(xr::Posef {
                orientation: xr::Quaternionf {
                    x: 0.,
                    y: 0.,
                    z: 0.,
                    w: 0.,
                },
                position: xr::Vector3f {
                    x: 0.,
                    y: 0.,
                    z: 0.,
                },
            }),
        ] {
            let mut changes = Changes::default();
            changes.schedule(xr::Time::from_nanos(1), Some(pose(DVec3::X, 0.5)));
            changes.take_due(xr::Time::from_nanos(1)).unwrap();
            changes.schedule(xr::Time::from_nanos(2), invalid);
            let next = pose(DVec3::Z, -0.4);
            changes.schedule(xr::Time::from_nanos(3), Some(next));
            let rebase = changes.take_due(xr::Time::from_nanos(3)).unwrap();
            assert!(rebase.reset_world);
            assert!((matrix(next) * matrix(rebase.offset)).abs_diff_eq(DMat4::IDENTITY, 1e-6));
        }
    }
}
