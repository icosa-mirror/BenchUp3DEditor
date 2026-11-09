//! OpenXR actions and panel pointer capture. No document or tool state is copied.
use super::EditorPanel;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2};
use glam::{DQuat, DVec3};
use openxr as xr;

#[derive(Clone, Copy, Default)]
struct Hand {
    position: Option<Pos2>,
    hit_distance: Option<f64>,
    buttons: [bool; 2],
    scroll: f32,
}

#[derive(Default)]
pub(super) struct FrameInput {
    pub events: Vec<Event>,
    pub cursor: Option<Pos2>,
    pub pressed: bool,
    pub focused: bool,
}

#[derive(Default)]
struct Pointer {
    owner: Option<usize>,
    down: [bool; 2],
    previous: [[bool; 2]; 2],
    armed: [bool; 2],
    position: Option<Pos2>,
}

impl Pointer {
    fn cancel(&mut self, output: &mut FrameInput) {
        if self.position.is_some() {
            // Release outside every widget so focus/tracking loss cannot click
            // the control under the last ray. Escape cancels a captured tool drag.
            let pos = egui::pos2(-1_000_000.0, -1_000_000.0);
            output.events.push(Event::PointerMoved(pos));
            if self.down.iter().any(|down| *down) {
                for pressed in [true, false] {
                    output.events.push(Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    });
                }
            }
            for (index, button) in [PointerButton::Primary, PointerButton::Secondary]
                .into_iter()
                .enumerate()
            {
                if self.down[index] {
                    output.events.push(Event::PointerButton {
                        pos,
                        button,
                        pressed: false,
                        modifiers: Modifiers::NONE,
                    });
                }
            }
            output.events.push(Event::PointerGone);
        }
        self.down = [false; 2];
        self.position = None;
        self.owner = None;
    }

    fn update(&mut self, hands: [Hand; 2], focused: bool, seconds: f32) -> FrameInput {
        let mut output = FrameInput {
            focused,
            ..Default::default()
        };
        if !focused {
            self.cancel(&mut output);
            self.armed = [false; 2];
            self.previous = [[false; 2]; 2];
            return output;
        }
        for (index, hand) in hands.iter().enumerate() {
            if hand.position.is_none() {
                self.armed[index] = false;
            } else if !hand.buttons.iter().any(|down| *down) {
                self.armed[index] = true;
            }
        }
        let captured = self.down.iter().any(|down| *down);
        if captured
            && self
                .owner
                .is_some_and(|index| hands[index].position.is_none())
        {
            self.cancel(&mut output);
            self.previous = hands.map(|hand| hand.buttons);
            return output;
        }
        if !captured {
            let pressed = (0..2).find(|&index| {
                hands[index].hit_distance.is_some()
                    && self.armed[index]
                    && (0..2)
                        .any(|button| hands[index].buttons[button] && !self.previous[index][button])
            });
            self.owner = pressed
                .or_else(|| {
                    self.owner
                        .filter(|&index| hands[index].hit_distance.is_some())
                })
                .or_else(|| {
                    (0..2)
                        .filter(|&index| hands[index].hit_distance.is_some())
                        .min_by(|&a, &b| {
                            hands[a]
                                .hit_distance
                                .unwrap()
                                .total_cmp(&hands[b].hit_distance.unwrap())
                        })
                });
        }
        if let Some(index) = self.owner {
            if let Some(pos) = hands[index].position {
                self.position = Some(pos);
                output.events.push(Event::PointerMoved(pos));
                for (button_index, button) in [PointerButton::Primary, PointerButton::Secondary]
                    .into_iter()
                    .enumerate()
                {
                    let physical = hands[index].buttons[button_index];
                    let down = physical
                        && (self.down[button_index]
                            || (self.armed[index]
                                && !self.previous[index][button_index]
                                && hands[index].hit_distance.is_some()));
                    if self.down[button_index] != down {
                        output.events.push(Event::PointerButton {
                            pos,
                            button,
                            pressed: down,
                            modifiers: Modifiers::NONE,
                        });
                    }
                    self.down[button_index] = down;
                }
                let axis = hands[index].scroll;
                if axis.is_finite() && axis.abs() > 0.2 && hands[index].hit_distance.is_some() {
                    output.events.push(Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Unknown,
                        delta: egui::vec2(0.0, axis * 600.0 * seconds.clamp(0.0, 0.05)),
                        modifiers: Modifiers::NONE,
                    });
                }
                output.cursor = Some(pos);
                output.pressed = self.down.iter().any(|down| *down);
            }
        } else {
            self.cancel(&mut output);
        }
        self.previous = hands.map(|hand| hand.buttons);
        output
    }
}

pub(super) struct Controllers {
    spaces: [xr::Space; 2],
    aim: xr::Action<xr::Posef>,
    select: xr::Action<bool>,
    secondary: xr::Action<bool>,
    scroll: xr::Action<xr::Vector2f>,
    scroll_touch: xr::Action<bool>,
    paths: [xr::Path; 2],
    set: xr::ActionSet,
    pointer: Pointer,
    previous_flags: [xr::SpaceLocationFlags; 2],
}

impl Controllers {
    pub fn new(instance: &xr::Instance, session: &xr::Session<xr::Vulkan>) -> xr::Result<Self> {
        let paths = [
            instance.string_to_path("/user/hand/left")?,
            instance.string_to_path("/user/hand/right")?,
        ];
        let set = instance.create_action_set("editor_panel", "Editor panel", 0)?;
        let aim = set.create_action::<xr::Posef>("aim", "Aim at panel", &paths)?;
        let select = set.create_action::<bool>("select", "Primary click", &paths)?;
        let secondary = set.create_action::<bool>("secondary", "Secondary click", &paths)?;
        let scroll = set.create_action::<xr::Vector2f>("scroll", "Scroll panel", &paths)?;
        let scroll_touch = set.create_action::<bool>("scroll_touch", "Scroll contact", &paths)?;
        for (profile, primary, secondary_left, secondary_right, axis) in [
            (
                "khr/simple_controller",
                "select/click",
                "menu/click",
                "menu/click",
                None,
            ),
            (
                "htc/vive_controller",
                "trigger/click",
                "menu/click",
                "menu/click",
                Some("trackpad"),
            ),
            (
                "valve/index_controller",
                "trigger/click",
                "b/click",
                "b/click",
                Some("thumbstick"),
            ),
            (
                "oculus/touch_controller",
                "trigger/value",
                "menu/click",
                "b/click",
                Some("thumbstick"),
            ),
        ] {
            let mut bindings = Vec::new();
            for (index, side) in ["left", "right"].into_iter().enumerate() {
                let path = |component: &str| {
                    instance.string_to_path(&format!("/user/hand/{side}/input/{component}"))
                };
                bindings.push(xr::Binding::new(&aim, path("aim/pose")?));
                bindings.push(xr::Binding::new(&select, path(primary)?));
                bindings.push(xr::Binding::new(
                    &secondary,
                    path(if index == 0 {
                        secondary_left
                    } else {
                        secondary_right
                    })?,
                ));
                if let Some(axis) = axis {
                    bindings.push(xr::Binding::new(&scroll, path(axis)?));
                    bindings.push(xr::Binding::new(
                        &scroll_touch,
                        path(&format!("{axis}/touch"))?,
                    ));
                }
            }
            instance.suggest_interaction_profile_bindings(
                instance.string_to_path(&format!("/interaction_profiles/{profile}"))?,
                &bindings,
            )?;
        }
        session.attach_action_sets(&[&set])?;
        let spaces = [
            aim.create_space(session, paths[0], xr::Posef::IDENTITY)?,
            aim.create_space(session, paths[1], xr::Posef::IDENTITY)?,
        ];
        Ok(Self {
            spaces,
            aim,
            select,
            secondary,
            scroll,
            scroll_touch,
            paths,
            set,
            pointer: Pointer::default(),
            previous_flags: [xr::SpaceLocationFlags::EMPTY; 2],
        })
    }

    pub fn sample(
        &mut self,
        session: &xr::Session<xr::Vulkan>,
        space: &xr::Space,
        time: xr::Time,
        panel: EditorPanel,
        panel_pose: xr::Posef,
        focused: bool,
        seconds: f32,
    ) -> xr::Result<FrameInput> {
        if !focused {
            return Ok(self.pointer.update([Hand::default(); 2], false, seconds));
        }
        session.sync_actions(&[(&self.set).into()])?;
        let panel_rotation = rotation(panel_pose.orientation);
        let mut hands = [Hand::default(); 2];
        for (index, hand) in hands.iter_mut().enumerate() {
            let path = self.paths[index];
            if !self.aim.is_active(session, path)? {
                continue;
            }
            let location = self.spaces[index].locate(space, time)?;
            if self.previous_flags[index] != location.location_flags {
                println!(
                    "[BenchUpXR] Controller {index} tracking: {:?}",
                    location.location_flags
                );
                self.previous_flags[index] = location.location_flags;
            }
            let required = xr::SpaceLocationFlags::POSITION_VALID
                | xr::SpaceLocationFlags::ORIENTATION_VALID
                | xr::SpaceLocationFlags::POSITION_TRACKED
                | xr::SpaceLocationFlags::ORIENTATION_TRACKED;
            if !location.location_flags.contains(required) {
                continue;
            }
            let Some(panel_rotation) = panel_rotation else {
                continue;
            };
            let Some(aim_rotation) = rotation(location.pose.orientation) else {
                continue;
            };
            let origin = panel_rotation.conjugate()
                * (position(location.pose.position) - position(panel_pose.position));
            let direction = panel_rotation.conjugate() * (aim_rotation * -DVec3::Z);
            let Some(projected) = panel.project_ray(origin, direction) else {
                continue;
            };
            let select = self.select.state(session, path)?;
            if !select.is_active {
                continue;
            }
            let secondary = self.secondary.state(session, path)?;
            let scroll = self.scroll.state(session, path)?;
            let touch = self.scroll_touch.state(session, path)?;
            *hand = Hand {
                position: Some(egui::pos2(
                    projected.position_points.x as f32,
                    projected.position_points.y as f32,
                )),
                hit_distance: panel
                    .hit_test(origin, direction)
                    .map(|hit| hit.distance_metres),
                buttons: [
                    select.is_active && select.current_state,
                    secondary.is_active && secondary.current_state,
                ],
                scroll: if scroll.is_active && touch.is_active && touch.current_state {
                    scroll.current_state.y
                } else {
                    0.0
                },
            };
        }
        Ok(self.pointer.update(hands, true, seconds))
    }

    pub fn cancel(&mut self) -> FrameInput {
        self.pointer.update([Hand::default(); 2], false, 0.0)
    }

    pub fn log_profiles(&self, session: &xr::Session<xr::Vulkan>) -> xr::Result<()> {
        for (index, path) in self.paths.iter().enumerate() {
            let profile = session.current_interaction_profile(*path)?;
            let name = if profile == xr::Path::NULL {
                "unbound".into()
            } else {
                session.instance().path_to_string(profile)?
            };
            println!("[BenchUpXR] Controller {index} profile: {name}");
        }
        Ok(())
    }
}

fn position(value: xr::Vector3f) -> DVec3 {
    DVec3::new(value.x as f64, value.y as f64, value.z as f64)
}
fn rotation(value: xr::Quaternionf) -> Option<DQuat> {
    let rotation = DQuat::from_xyzw(
        value.x as f64,
        value.y as f64,
        value.z as f64,
        value.w as f64,
    );
    (rotation.is_finite() && rotation.length_squared() > 1e-12).then(|| rotation.normalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hand(x: f32, pressed: bool) -> Hand {
        Hand {
            position: Some(egui::pos2(x, 30.0)),
            hit_distance: Some(1.0),
            buttons: [pressed, false],
            ..Default::default()
        }
    }
    fn clicks(input: &FrameInput, pressed: bool) -> usize {
        input.events.iter().filter(|event| matches!(event, Event::PointerButton { pressed: state, .. } if *state == pressed)).count()
    }
    #[test]
    fn capture_retains_hand_and_unbounded_drag_until_release() {
        let mut pointer = Pointer::default();
        pointer.update([hand(10.0, false), Hand::default()], true, 0.01);
        assert_eq!(
            clicks(
                &pointer.update([hand(10.0, true), hand(20.0, true)], true, 0.01),
                true
            ),
            1
        );
        let outside = Hand {
            hit_distance: None,
            ..hand(-50.0, true)
        };
        let drag = pointer.update([outside, hand(20.0, true)], true, 0.01);
        assert_eq!(drag.cursor, outside.position);
        assert_eq!(clicks(&drag, false), 0);
        assert_eq!(
            clicks(
                &pointer.update([hand(-50.0, false), hand(20.0, true)], true, 0.01),
                false
            ),
            1
        );
    }
    #[test]
    fn loss_releases_once_and_held_button_cannot_repress_after_focus_return() {
        let mut pointer = Pointer::default();
        pointer.update([hand(10.0, false), Hand::default()], true, 0.01);
        pointer.update([hand(10.0, true), Hand::default()], true, 0.01);
        assert_eq!(
            clicks(&pointer.update([Hand::default(); 2], false, 0.01), false),
            1
        );
        assert_eq!(
            clicks(
                &pointer.update([hand(10.0, true), Hand::default()], true, 0.01),
                true
            ),
            0
        );
        pointer.update([hand(10.0, false), Hand::default()], true, 0.01);
        assert_eq!(
            clicks(
                &pointer.update([hand(10.0, true), Hand::default()], true, 0.01),
                true
            ),
            1
        );
        assert_eq!(
            clicks(&pointer.update([Hand::default(); 2], true, 0.01), false),
            1
        );
        assert_eq!(
            clicks(&pointer.update([Hand::default(); 2], true, 0.01), false),
            0
        );
    }
    #[test]
    fn entering_panel_with_held_trigger_does_not_click() {
        let mut pointer = Pointer::default();
        pointer.update(
            [
                Hand {
                    hit_distance: None,
                    ..hand(100.0, true)
                },
                Hand::default(),
            ],
            true,
            0.01,
        );
        assert_eq!(
            clicks(
                &pointer.update([hand(10.0, true), Hand::default()], true, 0.01),
                true
            ),
            0
        );
    }
}
