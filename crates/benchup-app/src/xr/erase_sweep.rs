//! Bounded interpolation between adjacent, captured eraser rays.
use super::input::{SceneHit, SceneInput};
use benchup_core::{Model, Ray};
use glam::DMat4;
use std::{collections::BTreeSet, sync::Arc};

pub(super) struct Key {
    pub model: Arc<Model>,
    pub revision: u64,
    pub project: u64,
    pub transform: DMat4,
    pub display: benchup_core::DisplaySettings,
}

impl Key {
    fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.model, &other.model) && self.revision == other.revision && self.project == other.project
            && self.transform == other.transform && self.display == other.display
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn key(model: &Arc<Model>) -> Key {
        Key { model: Arc::clone(model), revision: 1, project: 0, transform: DMat4::IDENTITY, display: Default::default() }
    }
    fn input(ray: Ray, select: bool, down: bool) -> SceneInput {
        SceneInput { owner: 0, ray, hit: None, select, clear: false, pressed: down, primary_down: down }
    }
    fn hit(id: u64, point: DVec3, ray: Ray) -> SceneHit {
        SceneHit { handle: None, entity: Some(id), section_normal: None, face: None, edge: Some(id), guide: None,
            guide_points: [None; 8], flip: None, annotation: None, point, distance: point.distance(ray.origin) }
    }

    #[test]
    fn erase_sweep_finds_thin_retained_edges_between_missed_endpoints_and_on_release() {
        let mut model = Model::default();
        let edge = model.add_edge(DVec3::new(0., -0.15, -2.), DVec3::new(0., 0.15, -2.)).unwrap();
        let model = Arc::new(model);
        let mut index = super::super::world_picking::Index::default();
        index.prepare(&model);
        for rotation in [false, true] {
            let ray = |x| if rotation {
                Ray { origin: DVec3::ZERO, direction: DVec3::new(x, 0., -2.).normalize() }
            } else { Ray { origin: DVec3::new(x, 0., 0.), direction: -DVec3::Z } };
            let query = |ray, visible| index.query(ray, 0.01, |_, a, b| Some((a, b)), |_, _| visible)
                .map(|found| hit(found.id, found.point, ray));
            assert!(query(ray(-0.08), true).is_none());
            assert!(query(ray(0.08), true).is_none());
            for visible in [true, false] {
                let mut sweep = EraseSweep::default();
                sweep.update(Some(key(&model)), Some(input(ray(-0.08), true, true)), 1., |_| panic!("no interpolation before press"));
                let mut count = 0;
                let targets = sweep.update(Some(key(&model)), Some(input(ray(0.08), false, false)), 1.01, |ray| {
                    count += 1;
                    query(ray, visible)
                });
                assert!(count > 0 && count <= EraseSweep::MAX_STEPS - 1);
                assert_eq!(targets, if visible { BTreeSet::from([edge]) } else { BTreeSet::new() });
                assert!(sweep.previous.is_none(), "release ends the interpolation interval");
            }
        }
    }

    #[test]
    fn erase_sweep_never_bridges_loss_controller_document_view_or_tracking_jumps() {
        let model = Arc::new(Model::default());
        let first = input(Ray { origin: DVec3::ZERO, direction: -DVec3::Z }, true, true);
        for cause in 0..12 {
            let mut sweep = EraseSweep::default();
            sweep.update(Some(key(&model)), Some(first), 1., |_| unreachable!());
            let mut next_key = key(&model);
            let mut next = SceneInput { select: false, ray: Ray { origin: DVec3::X * 0.1, ..first.ray }, ..first };
            let mut time = 1.01;
            match cause {
                0 => next.owner = 1,
                1 => time = 2.,
                2 => next_key.model = Arc::new(Model::default()),
                3 => next_key.revision += 1,
                4 => next_key.transform = DMat4::from_scale(DVec3::splat(2.)),
                5 => next_key.display.section_cuts = !next_key.display.section_cuts,
                6 => next.clear = true,
                7 => next.ray.origin = DVec3::X,
                8 => next.ray.direction = DVec3::X,
                10 => next.pressed = false,
                11 => next_key.project += 1,
                _ => {}
            }
            let next = (cause != 9).then_some(next);
            assert!(sweep.update(Some(next_key), next, time, |_| panic!("invalid sweep bridge")).is_empty());
            let held = SceneInput { select: false, ..first };
            sweep.update(Some(key(&model)), Some(held), time + 0.01, |_| panic!("must rearm with a fresh press"));
        }
    }

    #[test]
    fn erase_sweep_capture_loss_cancels_without_waiting_for_a_rendered_ui_frame() {
        use super::super::input::erase_capture_cancelled;
        let ray = Ray { origin: DVec3::ZERO, direction: -DVec3::Z };
        let held = input(ray, false, true);
        assert!(!erase_capture_cancelled(Some(held), Some(held)));
        assert!(!erase_capture_cancelled(Some(held), Some(input(ray, false, false))));
        assert!(!erase_capture_cancelled(None, Some(input(ray, true, true))));
        for next in [None, Some(SceneInput { owner: 1, ..held }),
            Some(SceneInput { pressed: false, ..held }), Some(SceneInput { clear: true, ..held })]
        {
            assert!(erase_capture_cancelled(Some(held), next));
        }
    }
}

struct Sample {
    key: Key,
    input: SceneInput,
    time: f64,
}

#[derive(Default)]
pub(super) struct EraseSweep {
    previous: Option<Sample>,
}

impl EraseSweep {
    // Edge/guide picks use a 0.01-radian cone. Overlap samples within it;
    // translation is converted to an angular bound at the nearer endpoint.
    const STEP: f64 = 0.008;
    const MAX_STEPS: usize = 64;

    pub fn reset(&mut self) { self.previous = None; }

    pub fn update(&mut self, key: Option<Key>, input: Option<SceneInput>, time: f64,
        mut pick: impl FnMut(Ray) -> Option<SceneHit>) -> BTreeSet<u64>
    {
        let previous = self.previous.take();
        let mut targets = BTreeSet::new();
        let (Some(key), Some(mut input)) = (key, input) else { return targets; };
        if input.clear || input.primary_down && !input.pressed || !time.is_finite() || !key.transform.is_finite()
            || !input.ray.origin.is_finite() || !input.ray.direction.is_finite()
        { return targets; }
        let Some(direction) = input.ray.direction.try_normalize() else { return targets; };
        input.ray.direction = direction;
        if input.select && input.primary_down {
            self.previous = Some(Sample { key, input, time });
            return targets;
        }
        let Some(previous) = previous.filter(|previous|
            previous.input.primary_down && previous.input.owner == input.owner
                && previous.key.matches(&key) && time > previous.time && time - previous.time <= 0.15)
        else { return targets; };
        let angle = previous.input.ray.direction.dot(direction).clamp(-1., 1.).acos();
        let travel = previous.input.ray.origin.distance(input.ray.origin);
        // A tracking jump is not evidence that the controller swept that path.
        if angle > std::f64::consts::FRAC_PI_4 || travel > 0.5 { return targets; }
        let depth = [previous.input.hit, input.hit].into_iter().flatten()
            .map(|hit| hit.distance).filter(|distance| distance.is_finite() && *distance > 0.)
            .reduce(f64::min).unwrap_or(0.25).max(0.05);
        let steps = ((angle + travel / depth) / Self::STEP).ceil().clamp(1., Self::MAX_STEPS as f64) as usize;
        for i in 1..steps {
            let t = i as f64 / steps as f64;
            let ray = Ray {
                origin: previous.input.ray.origin.lerp(input.ray.origin, t),
                direction: previous.input.ray.direction.lerp(direction, t).normalize(),
            };
            if let Some(entity) = pick(ray).and_then(|hit| hit.entity) { targets.insert(entity); }
        }
        if input.primary_down { self.previous = Some(Sample { key, input, time }); }
        targets
    }
}
