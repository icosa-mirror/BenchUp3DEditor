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
        })
    }
}
struct Grab {
    hands: HandFrame,
    scene: Transform,
    previous_up: DVec3,
}

#[derive(Default)]
pub(super) struct Navigation {
    scene: Transform,
    grab: Option<Grab>,
    armed: bool,
    available: bool,
}
impl Navigation {
    pub fn update(&mut self, input: Input) {
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
            });
            return;
        };
        let rotation = (hands.rotation * grab.hands.rotation.conjugate()).normalize();
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
            "Scene grabbed. Move, turn or spread your hands; release a grip to place it."
        } else if !self.armed {
            "Release both grips before grabbing the scene again."
        } else {
            "Hold both grips to move, rotate and scale the scene. Fit 3D resets the view."
        }
    }
}
