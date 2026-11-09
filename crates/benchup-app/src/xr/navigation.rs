//! Two-grip similarity transform in tracking space. Document coordinates stay fixed.
use glam::{DMat3, DMat4, DQuat, DVec3};

#[derive(Clone, Copy)]
pub(super) struct Grip {
    pub position: DVec3,
    pub orientation: DQuat,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Input {
    pub enabled: bool,
    pub buttons: [Option<bool>; 2],
    pub hands: [Option<Grip>; 2],
}

#[derive(Clone, Copy)]
struct Transform {
    translation: DVec3,
    rotation: DQuat,
    scale: f64,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: DVec3::ZERO,
            rotation: DQuat::IDENTITY,
            scale: 1.0,
        }
    }
}
#[derive(Clone, Copy)]
struct HandFrame {
    center: DVec3,
    rotation: DQuat,
    span: f64,
    up: DVec3,
    heading: Option<DVec3>,
}
impl HandFrame {
    fn new(hands: [Option<Grip>; 2], previous_up: Option<DVec3>) -> Option<Self> {
        let [Some(left), Some(right)] = hands else {
            return None;
        };
        if !left.position.is_finite()
            || !right.position.is_finite()
            || !left.orientation.is_finite()
            || !right.orientation.is_finite()
        {
            return None;
        }
        let delta = right.position - left.position;
        let span = delta.length();
        if !span.is_finite() || span < 0.08 {
            return None;
        }
        let x = delta / span;
        let horizontal = DVec3::new(delta.x, 0.0, delta.z);
        let heading = (horizontal.length() >= 0.05).then(|| horizontal.normalize());
        let projected = |v: DVec3| v - x * v.dot(x);
        let hint = projected(left.orientation * DVec3::Y + right.orientation * DVec3::Y);
        let previous = previous_up.and_then(|up| projected(up).try_normalize());
        let mut up = if hint.length_squared() > 0.04 {
            hint.normalize()
        } else {
            previous.unwrap_or_else(|| {
                projected(if x.y.abs() < 0.9 { DVec3::Y } else { DVec3::Z }).normalize()
            })
        };
        // Opposed wrists can cancel the up hint. Choose its continuous sign
        // when it returns instead of rolling the scene by 180 degrees.
        if previous.is_some_and(|previous| up.dot(previous) < 0.0) {
            up = -up;
        }
        let rotation = DQuat::from_mat3(&DMat3::from_cols(x, up, x.cross(up))).normalize();
        Some(Self {
            center: (left.position + right.position) * 0.5,
            rotation,
            span,
            up,
            heading,
        })
    }
}
struct Grab {
    hands: HandFrame,
    scene: Transform,
    previous_up: DVec3,
    heading: Option<DVec3>,
    yaw: f64,
}

#[derive(Default)]
pub(super) struct Navigation {
    scene: Transform,
    grab: Option<Grab>,
    armed: bool,
    available: bool,
    upright: Option<bool>,
}
impl Navigation {
    pub fn update(&mut self, input: Input, keep_upright: bool, pivot: DVec3) {
        if self.upright != Some(keep_upright) {
            if keep_upright {
                let point =
                    self.scene.translation + self.scene.rotation * (pivot * self.scene.scale);
                let forward = self.scene.rotation * -DVec3::Z;
                let right = self.scene.rotation * DVec3::X;
                let yaw = if forward.x * forward.x + forward.z * forward.z > 0.001 {
                    (-forward.x).atan2(-forward.z)
                } else {
                    (-right.z).atan2(right.x)
                };
                self.scene.rotation = DQuat::from_rotation_y(yaw);
                self.scene.translation = point - self.scene.rotation * (pivot * self.scene.scale);
            }
            self.upright = Some(keep_upright);
            self.cancel();
        }
        if !input.enabled {
            self.cancel();
            return;
        }
        self.available = input.buttons.iter().all(Option::is_some);
        if input.buttons == [Some(false), Some(false)] {
            self.grab = None;
            self.armed = true;
            return;
        }
        if input.buttons.iter().any(Option::is_none) {
            self.cancel();
            return;
        }
        if input.buttons != [Some(true), Some(true)] {
            self.grab = None;
            return;
        }
        if !self.armed {
            return;
        }
        let previous = self.grab.as_ref().map(|grab| grab.previous_up);
        let Some(hands) = HandFrame::new(input.hands, previous) else {
            self.cancel();
            return;
        };
        let Some(grab) = &mut self.grab else {
            self.grab = Some(Grab {
                hands,
                scene: self.scene,
                previous_up: hands.up,
                heading: hands.heading,
                yaw: 0.0,
            });
            return;
        };
        let rotation = if keep_upright {
            if let (Some(previous), Some(current)) = (grab.heading, hands.heading) {
                let angle = previous.cross(current).y.atan2(previous.dot(current));
                grab.yaw = (grab.yaw + angle).rem_euclid(std::f64::consts::TAU);
            }
            // Near-vertical hand separation has no reliable heading. Freeze
            // yaw there, then rebase it when a horizontal heading returns.
            grab.heading = hands.heading;
            DQuat::from_rotation_y(grab.yaw)
        } else {
            (hands.rotation * grab.hands.rotation.conjugate()).normalize()
        };
        let scale = (grab.scene.scale * hands.span / grab.hands.span).clamp(0.01, 1000.0);
        let ratio = scale / grab.scene.scale;
        let translation =
            hands.center + rotation * ((grab.scene.translation - grab.hands.center) * ratio);
        let orientation = (rotation * grab.scene.rotation).normalize();
        if translation.is_finite() && orientation.is_finite() {
            self.scene = Transform {
                translation,
                rotation: orientation,
                scale,
            };
            grab.previous_up = hands.up;
        } else {
            self.cancel();
        }
    }
    fn cancel(&mut self) {
        self.grab = None;
        self.armed = false;
    }
    pub fn reset(&mut self) {
        self.scene = Transform::default();
        self.cancel();
    }
    pub fn matrix(&self) -> DMat4 {
        DMat4::from_scale_rotation_translation(
            DVec3::splat(self.scene.scale),
            self.scene.rotation,
            self.scene.translation,
        )
    }
    pub fn grabbing(&self) -> bool {
        self.grab.is_some()
    }
    pub fn scale(&self) -> f64 {
        self.scene.scale
    }
    pub fn hint(&self) -> &'static str {
        if !self.available {
            "Fit 3D resets the scene view."
        } else if self.grabbing() {
            if self.upright == Some(true) {
                "Scene grabbed; up stays upright. Move, turn or spread your hands; release a grip to place it."
            } else {
                "Scene grabbed. Move, turn or spread your hands; release a grip to place it."
            }
        } else if !self.armed {
            "Release both grips before grabbing the scene again."
        } else {
            "Hold both grips to move, rotate and scale the scene. Fit 3D resets the view."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn released()->Input {Input {enabled:true,buttons:[Some(false);2],..Default::default()}}
    fn held(center:DVec3,span:f64,rotation:DQuat)->Input {
        let delta=rotation*DVec3::X*span*0.5;
        Input {enabled:true,buttons:[Some(true);2],hands:[Some(Grip {position:center-delta,orientation:rotation}),Some(Grip {position:center+delta,orientation:rotation})]}
    }
    fn close(a:DVec3,b:DVec3) {assert!(a.distance(b)<1e-10,"{a:?} != {b:?}");}

    #[test]
    fn upright_ignores_hand_tilt_but_allows_vertical_movement() {
        let mut nav=Navigation::default();let center=DVec3::new(0.0,1.0,-0.5);
        nav.update(released(),true,DVec3::ZERO);
        nav.update(held(center,0.4,DQuat::IDENTITY),true,DVec3::ZERO);
        nav.update(held(center+DVec3::Y,0.4,DQuat::from_rotation_z(0.6)),true,DVec3::ZERO);
        close(nav.matrix().transform_vector3(DVec3::Y),DVec3::Y);
        close(nav.matrix().transform_point3(DVec3::ZERO),DVec3::Y);
    }

    #[test]
    fn upright_yaw_and_scale_keep_the_grab_point_fixed() {
        let mut nav=Navigation::default();let center=DVec3::new(0.2,1.0,-0.5);
        nav.update(released(),true,DVec3::ZERO);
        nav.update(held(center,0.4,DQuat::IDENTITY),true,DVec3::ZERO);
        nav.update(held(center,0.8,DQuat::from_rotation_y(0.7)),true,DVec3::ZERO);
        assert!((nav.scale()-2.0).abs()<1e-10);
        close(nav.matrix().transform_point3(center),center);
        let direction=nav.matrix().transform_vector3(DVec3::X).normalize();
        close(direction,DQuat::from_rotation_y(0.7)*DVec3::X);
        nav.update(held(center,0.8,DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2)),true,DVec3::ZERO);
        nav.update(held(center,0.8,DQuat::from_rotation_y(1.2)),true,DVec3::ZERO);
        close(nav.matrix().transform_vector3(DVec3::X).normalize(),direction);
    }

    #[test]
    fn enabling_upright_levels_the_scene_without_moving_its_pivot() {
        let mut nav=Navigation::default();let center=DVec3::new(0.0,1.0,-0.5);let pivot=DVec3::new(2.0,-1.0,0.3);
        nav.update(released(),false,pivot);
        nav.update(held(center,0.4,DQuat::IDENTITY),false,pivot);
        nav.update(held(center,0.4,DQuat::from_rotation_z(0.6)),false,pivot);
        let before=nav.matrix().transform_point3(pivot);
        nav.update(released(),true,pivot);
        close(nav.matrix().transform_vector3(DVec3::Y).normalize(),DVec3::Y);
        close(nav.matrix().transform_point3(pivot),before);
    }
}
