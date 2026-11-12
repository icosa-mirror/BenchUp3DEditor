//! Controller-relative panel placement, independent of the world scene transform.
use crate::app_host::{XrHand, XrPanelPreferences};
use glam::{DQuat, DVec3};
use openxr as xr;

const LOCAL_OFFSET: DVec3 = DVec3::new(0., 0.15, 0.);
// 270 degrees around the controller-local X axis.
const LOCAL_ROTATION: DQuat = DQuat::from_xyzw(
    std::f64::consts::FRAC_1_SQRT_2,
    0.,
    0.,
    -std::f64::consts::FRAC_1_SQRT_2,
);

pub(super) fn local_pose() -> xr::Posef {
    let [x, y, z, w] = LOCAL_ROTATION.to_array();
    xr::Posef {
        orientation: xr::Quaternionf {
            x: x as f32,
            y: y as f32,
            z: z as f32,
            w: w as f32,
        },
        position: xr::Vector3f {
            x: 0.,
            y: LOCAL_OFFSET.y as f32,
            z: 0.,
        },
    }
}

pub(super) struct Placement {
    pub pose: xr::Posef,
    pub width: f32,
    pub pointer_hand: Option<usize>,
    pub attached: bool,
    pub changed: bool,
}

#[derive(Default)]
pub(super) struct Anchor {
    attached: bool,
    policy: Option<(bool, XrHand)>,
}

impl Anchor {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(
        &mut self,
        preferences: XrPanelPreferences,
        hand: Option<super::navigation::Grip>,
        world: xr::Posef,
    ) -> Placement {
        let policy = (preferences.controller_panel, preferences.dominant_hand);
        let pose = preferences
            .controller_panel
            .then(|| controller_pose(hand?))
            .flatten();
        let attached = pose.is_some();
        let changed = self.policy != Some(policy) || attached != self.attached;
        self.attached = attached;
        self.policy = Some(policy);
        Placement {
            pose: pose.unwrap_or(world),
            width: if attached {
                preferences.controller_width_metres
            } else {
                preferences.width_metres
            },
            pointer_hand: preferences
                .controller_panel
                .then(|| preferences.dominant_hand.index()),
            attached,
            changed,
        }
    }
}

fn controller_pose(hand: super::navigation::Grip) -> Option<xr::Posef> {
    if !hand.position.is_finite()
        || !hand.orientation.is_finite()
        || hand.orientation.length_squared() < 1e-12
    {
        return None;
    }
    // A rigid child of the grip: no head-facing rotation or navigation freeze.
    // Keep the offset grip-local; flip the display about its local X axis.
    let grip_rotation = hand.orientation.normalize();
    let center = hand.position + grip_rotation * LOCAL_OFFSET;
    let rotation = grip_rotation * LOCAL_ROTATION;
    Some(xr::Posef {
        orientation: xr::Quaternionf {
            x: rotation.x as f32,
            y: rotation.y as f32,
            z: rotation.z as f32,
            w: rotation.w as f32,
        },
        position: xr::Vector3f {
            x: center.x as f32,
            y: center.y as f32,
            z: center.z as f32,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::super::navigation::Grip;
    use super::*;
    use glam::{DMat4, DQuat};

    fn matrix(pose: xr::Posef) -> DMat4 {
        let p = pose.position;
        let q = pose.orientation;
        DMat4::from_rotation_translation(
            DQuat::from_xyzw(q.x as f64, q.y as f64, q.z as f64, q.w as f64).normalize(),
            DVec3::new(p.x as f64, p.y as f64, p.z as f64),
        )
    }

    #[test]
    fn panel_is_a_rigid_child_of_translated_and_rotated_grip() {
        let mut anchor = Anchor::default();
        let preferences = XrPanelPreferences {
            controller_panel: true,
            ..Default::default()
        };
        for hand in [
            Grip {
                position: DVec3::new(-0.3, 1., -0.5),
                orientation: DQuat::IDENTITY,
            },
            Grip {
                position: DVec3::new(0.5, 1.7, -0.2),
                orientation: DQuat::from_euler(glam::EulerRot::XYZ, 0.7, -1.1, 0.4),
            },
        ] {
            let frame = anchor.update(preferences, Some(hand), xr::Posef::IDENTITY);
            let parent = DMat4::from_rotation_translation(hand.orientation, hand.position);
            assert!((parent.inverse() * matrix(frame.pose)).abs_diff_eq(
                DMat4::from_rotation_translation(LOCAL_ROTATION, LOCAL_OFFSET),
                1e-6
            ));
            assert!((parent * matrix(local_pose())).abs_diff_eq(matrix(frame.pose), 1e-6));
            assert!(frame.attached);
            assert_eq!(frame.pointer_hand, Some(1));
        }
    }

    #[test]
    fn loss_falls_back_and_role_changes_cancel_without_freezing_a_tracked_pose() {
        let mut anchor = Anchor::default();
        let mut preferences = XrPanelPreferences {
            controller_panel: true,
            ..Default::default()
        };
        let mut hand = Grip {
            position: DVec3::new(-0.3, 1., -0.5),
            orientation: DQuat::IDENTITY,
        };
        let first = anchor.update(preferences, Some(hand), xr::Posef::IDENTITY);
        assert!(first.changed);
        hand.position.x -= 0.4;
        hand.orientation = DQuat::from_rotation_z(0.8);
        let moved = anchor.update(preferences, Some(hand), xr::Posef::IDENTITY);
        assert_ne!(moved.pose, first.pose);
        assert!(!moved.changed);
        let lost = anchor.update(preferences, None, xr::Posef::IDENTITY);
        assert!(!lost.attached);
        assert!(lost.changed);
        assert_eq!(lost.pose, xr::Posef::IDENTITY);
        assert!(
            anchor
                .update(preferences, Some(hand), xr::Posef::IDENTITY)
                .changed
        );
        preferences.dominant_hand = XrHand::Left;
        let reversed = anchor.update(preferences, Some(hand), xr::Posef::IDENTITY);
        assert_eq!(reversed.pointer_hand, Some(0));
        assert!(reversed.changed);
        preferences.controller_panel = false;
        let detached = anchor.update(preferences, Some(hand), xr::Posef::IDENTITY);
        assert!(!detached.attached);
        assert_eq!(detached.pointer_hand, None);
    }
}
