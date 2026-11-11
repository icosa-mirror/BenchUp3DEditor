//! Retained hit rectangles for the GPU-rendered viewport controls.
use crate::app::viewport_overlays::{Handle, HandleAction, Overlays};
use super::world_labels::{Label, Labels, View};
use benchup_core::{DVec3, Ray};
use std::{collections::BTreeSet, sync::Arc};

pub(super) fn surface_visible(distance: f64, face: Option<(Option<u64>, f64)>,
    cap: Option<(Option<u64>, f64)>, owners: &BTreeSet<u64>) -> bool
{
    let nearest = match (face, cap) {
        (Some(face), Some(cap)) => Some(if cap.1 < face.1 { cap } else { face }),
        (face, cap) => face.or(cap),
    };
    nearest.is_none_or(|(owner, depth)| depth + 1e-5 >= distance
        || owner.is_some_and(|id| owners.contains(&id)))
}

#[derive(Default)]
pub(super) struct Handles {
    source: Option<Arc<Overlays>>,
    labels: Labels,
}

impl Handles {
    pub fn prepare(&mut self, source: &Arc<Overlays>) {
        if self.source.as_ref().is_some_and(|old| Arc::ptr_eq(old, source)) { return; }
        // Later billboards cover earlier ones at the same depth.
        self.labels.update(source.handles.iter().enumerate().rev().map(|(index, handle)| Label {
            id: index as u64, anchor: handle.point, bounds: handle.bounds,
        }).collect());
        self.source = Some(Arc::clone(source));
    }

    pub fn pick(&self, ray: Ray, view: View, visible: impl Fn(&Handle, DVec3) -> bool) -> Option<(HandleAction, DVec3)> {
        let source = self.source.as_ref()?;
        let (slot, point) = self.labels.pick(ray, view, |slot, _, point|
            visible(&source.handles[slot as usize], point))?;
        Some((source.handles[slot as usize].action, point))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(scale: f64) -> View {
        View { head: DVec3::ZERO, right: DVec3::X, up: DVec3::Y, units: 1., font_scale: scale }
    }
    fn ray(x: f64) -> Ray { Ray { origin: DVec3::new(x, 0., 0.), direction: DVec3::Z } }
    fn handle(id: u64) -> Handle {
        Handle { action: HandleAction::EditJoint(id), point: DVec3::Z, bounds: [-14., -14., 14., 14.],
            owners: Default::default(), depth: true }
    }

    #[test]
    fn world_handles_use_billboard_bounds_scaling_visibility_and_current_actions() {
        let mut overlay = Overlays::default();
        overlay.handles = vec![handle(7), handle(9)];
        let overlay = Arc::new(overlay);
        let mut handles = Handles::default();
        handles.prepare(&overlay);
        assert_eq!(handles.pick(ray(0.), view(1.), |_, _| true).unwrap().0, HandleAction::EditJoint(9));
        assert!(handles.pick(ray(0.02), view(1.), |_, _| true).is_none());
        assert!(handles.pick(ray(0.02), view(2.), |_, _| true).is_some());
        assert!(handles.pick(ray(0.), view(1.), |_, _| false).is_none());
        handles.prepare(&overlay);
        let mut changed = (*overlay).clone();
        changed.handles[1].action = HandleAction::EditJoint(11);
        handles.prepare(&Arc::new(changed));
        assert_eq!(handles.pick(ray(0.), view(1.), |_, _| true).unwrap().0, HandleAction::EditJoint(11));
        handles.prepare(&Arc::new(Overlays::default()));
        assert!(handles.pick(ray(0.), view(1.), |_, _| true).is_none());
    }

    #[test]
    fn world_handles_follow_the_existing_first_face_or_cap_owner_visibility_rule() {
        let owners = BTreeSet::from([7]);
        assert!(surface_visible(1., None, None, &owners));
        assert!(surface_visible(1., Some((Some(7), 0.5)), Some((Some(9), 0.8)), &owners));
        assert!(!surface_visible(1., Some((Some(9), 0.5)), Some((Some(7), 0.8)), &owners));
        assert!(surface_visible(1., Some((Some(9), 0.8)), Some((Some(7), 0.5)), &owners));
        assert!(!surface_visible(1., None, Some((None, 0.5)), &owners));
        assert!(surface_visible(1., None, Some((None, 2.)), &owners));
    }
}
