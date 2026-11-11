//! Controller-relative panel placement, independent of the world scene transform.
use crate::app_host::{XrHand, XrPanelPreferences};
use glam::{DMat3, DQuat, DVec3};
use openxr as xr;

pub(super) struct Placement {
    pub pose: xr::Posef,
    pub width: f32,
    pub pointer_hand: Option<usize>,
    pub attached: bool,
    pub changed: bool,
}

#[derive(Default)]
pub(super) struct Anchor {
    pose: Option<xr::Posef>,
    policy: Option<(bool, XrHand)>,
}

impl Anchor {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(
        &mut self,
        preferences: XrPanelPreferences,
        head: Option<xr::Posef>,
        hand: Option<super::navigation::Grip>,
        navigating: bool,
        world: xr::Posef,
    ) -> Placement {
        let policy = (preferences.controller_panel, preferences.dominant_hand);
        let changed_policy = self.policy.is_some_and(|previous| previous != policy);
        let candidate = preferences
            .controller_panel
            .then(|| controller_pose(head?, hand?))
            .flatten();
        let pose = if navigating {
            // Keep the tablet out of the moving hands during a scene grab. A
            // newly tracked hand waits for release instead of moving the UI.
            candidate.and(self.pose.filter(|_| !changed_policy))
        } else {
            candidate
        };
        let changed = changed_policy || pose.is_some() != self.pose.is_some();
        self.pose = pose;
        self.policy = Some(policy);
        Placement {
            pose: pose.unwrap_or(world),
            width: if pose.is_some() {
                preferences.controller_width_metres
            } else {
                preferences.width_metres
            },
            pointer_hand: preferences
                .controller_panel
                .then(|| preferences.dominant_hand.index()),
            attached: pose.is_some(),
            changed,
        }
    }
}

fn controller_pose(head: xr::Posef, hand: super::navigation::Grip) -> Option<xr::Posef> {
    if !hand.position.is_finite()
        || !hand.orientation.is_finite()
        || hand.orientation.length_squared() < 1e-12
    {
        return None;
    }
    let center = hand.position + hand.orientation.normalize() * DVec3::new(0., 0.12, -0.08);
    let head = DVec3::new(
        head.position.x as f64,
        head.position.y as f64,
        head.position.z as f64,
    );
    let direction = head - center;
    if !direction.is_finite() || direction.length_squared() < 0.2_f64.powi(2) {
        return None;
    }
    // Position follows the grip; the front faces the user with gravity-aligned
    // text rather than requiring a particular controller wrist orientation.
    let normal = direction.normalize();
    let right = DVec3::Y.cross(normal).try_normalize().unwrap_or(DVec3::X);
    let up = normal.cross(right).normalize();
    let q = DQuat::from_mat3(&DMat3::from_cols(right, up, normal)).normalize();
    Some(xr::Posef {
        orientation: xr::Quaternionf {
            x: q.x as f32,
            y: q.y as f32,
            z: q.z as f32,
            w: q.w as f32,
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

    fn grip(x: f64) -> Grip {
        Grip {
            position: DVec3::new(x, 1., -0.5),
            orientation: DQuat::IDENTITY,
        }
    }
    fn head() -> xr::Posef {
        xr::Posef {
            position: xr::Vector3f {
                x: 0.,
                y: 1.6,
                z: 0.,
            },
            ..xr::Posef::IDENTITY
        }
    }

    #[test]
    fn attachment_faces_head_and_filters_the_host_hand_for_both_dominances() {
        for dominant in [XrHand::Left, XrHand::Right] {
            let preferences = XrPanelPreferences {
                controller_panel: true,
                dominant_hand: dominant,
                ..Default::default()
            };
            let mut anchor = Anchor::default();
            let hand = grip(if dominant == XrHand::Right { -0.3 } else { 0.3 });
            let frame = anchor.update(
                preferences,
                Some(head()),
                Some(hand),
                false,
                xr::Posef::IDENTITY,
            );
            assert!(frame.changed);
            assert_eq!(frame.pointer_hand, Some(dominant.index()));
            assert_eq!(frame.width, 0.45);
            let p = frame.pose.position;
            let q = frame.pose.orientation;
            let orientation = DQuat::from_xyzw(q.x as f64, q.y as f64, q.z as f64, q.w as f64);
            let toward_head = (DVec3::new(0., 1.6, 0.)
                - DVec3::new(p.x as f64, p.y as f64, p.z as f64))
            .normalize();
            assert!((orientation * DVec3::Z).distance(toward_head) < 1e-6);
            assert!((orientation * DVec3::Y).y > 0.);
        }
    }

    #[test]
    fn grabs_freeze_panel_loss_falls_back_and_reacquisition_or_handedness_changes_cancel_capture() {
        let mut anchor = Anchor::default();
        let mut preferences = XrPanelPreferences {
            controller_panel: true,
            ..Default::default()
        };
        let first = anchor.update(
            preferences,
            Some(head()),
            Some(grip(-0.3)),
            false,
            xr::Posef::IDENTITY,
        );
        let frozen = anchor.update(
            preferences,
            Some(head()),
            Some(grip(-0.7)),
            true,
            xr::Posef::IDENTITY,
        );
        assert_eq!(frozen.pose, first.pose);
        assert!(!frozen.changed);
        let fallback = anchor.update(preferences, Some(head()), None, true, xr::Posef::IDENTITY);
        assert_eq!(fallback.pose, xr::Posef::IDENTITY);
        assert_eq!(fallback.width, preferences.width_metres);
        assert!(fallback.changed);
        let held = anchor.update(
            preferences,
            Some(head()),
            Some(grip(-0.7)),
            true,
            xr::Posef::IDENTITY,
        );
        assert_eq!(held.pose, xr::Posef::IDENTITY);
        let resumed = anchor.update(
            preferences,
            Some(head()),
            Some(grip(-0.7)),
            false,
            xr::Posef::IDENTITY,
        );
        assert!(resumed.changed);
        preferences.dominant_hand = XrHand::Left;
        assert!(
            anchor
                .update(
                    preferences,
                    Some(head()),
                    Some(grip(0.3)),
                    false,
                    xr::Posef::IDENTITY
                )
                .changed
        );
        anchor.reset();
        assert_eq!(
            anchor
                .update(preferences, None, None, false, xr::Posef::IDENTITY)
                .pose,
            xr::Posef::IDENTITY
        );
        preferences.controller_panel = false;
        assert_eq!(
            anchor
                .update(
                    preferences,
                    Some(head()),
                    Some(grip(0.3)),
                    false,
                    xr::Posef::IDENTITY
                )
                .pointer_hand,
            None
        );
    }
}
