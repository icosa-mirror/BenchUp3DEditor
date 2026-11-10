//! Retained annotation hit rectangles from the same font layout as GPU labels.
use super::world_picking::{Index, quad_point};
use benchup_core::{DVec3, Ray};

#[derive(Clone, PartialEq)]
pub(super) struct Label {
    pub id: u64,
    pub anchor: DVec3,
    /// Bounds in egui points: min x/y, max x/y, matching world_text.wgsl.
    pub bounds: [f64; 4],
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> View {
        View {
            head: DVec3::ZERO,
            right: DVec3::X,
            up: DVec3::Y,
            units: 1.,
            font_scale: 1.,
        }
    }
    fn ray(x: f64, y: f64) -> Ray {
        Ray {
            origin: DVec3::new(x, y, 0.),
            direction: DVec3::Z,
        }
    }
    fn label(id: u64, z: f64) -> Label {
        Label {
            id,
            anchor: DVec3::new(0., 0., z),
            bounds: [-100., -10., 100., 10.],
        }
    }
    #[test]
    fn annotation_rectangles_capture_off_center_rays_and_respect_visibility() {
        let mut labels = Labels::default();
        labels.update(vec![label(7, 2.), label(8, 3.)]);
        assert_eq!(
            labels
                .pick(ray(0.15, 0.), view(), |_, _, _| true)
                .unwrap()
                .0,
            7
        );
        assert_eq!(
            labels
                .pick(ray(0.15, 0.), view(), |id, _, _| id != 7)
                .unwrap()
                .0,
            8
        );
        assert!(
            labels
                .pick(ray(0.15, 0.), view(), |_, _, _| false)
                .is_none()
        );
        assert!(labels.pick(ray(0., 0.1), view(), |_, _, _| true).is_none());
        labels.update(vec![]);
        assert!(labels.pick(ray(0., 0.), view(), |_, _, _| true).is_none());
    }
    #[test]
    fn label_hits_follow_gpu_distance_clamps_basis_and_scene_scale() {
        let mut labels = Labels::default();
        labels.update(vec![label(7, 0.1)]);
        assert!(labels.pick(ray(0.03, 0.), view(), |_, _, _| true).is_some());
        assert!(labels.pick(ray(0.04, 0.), view(), |_, _, _| true).is_none());
        labels.update(vec![label(7, 20.)]);
        assert!(labels.pick(ray(0.8, 0.), view(), |_, _, _| true).is_some());
        assert!(labels.pick(ray(0.9, 0.), view(), |_, _, _| true).is_none());
        labels.update(vec![label(7, 2.)]);
        let rotated = || View {
            right: DVec3::Y,
            up: -DVec3::X,
            ..view()
        };
        assert!(
            labels
                .pick(ray(0., 0.15), rotated(), |_, _, _| true)
                .is_some()
        );
        assert!(
            labels
                .pick(ray(0.15, 0.), rotated(), |_, _, _| true)
                .is_none()
        );
        let scaled = || View {
            units: 10.,
            ..view()
        };
        assert!(
            labels
                .pick(ray(0.08, 0.), scaled(), |_, _, _| true)
                .is_some()
        );
        assert!(
            labels
                .pick(ray(0.1, 0.), scaled(), |_, _, _| true)
                .is_none()
        );
    }
}

#[derive(Default)]
pub(super) struct Labels {
    labels: Vec<Label>,
    index: Index,
    radius_points: f64,
}

/// The billboard basis in model coordinates; units is tracking metres per model unit.
pub(super) struct View {
    pub head: DVec3,
    pub right: DVec3,
    pub up: DVec3,
    pub units: f64,
    pub font_scale: f64,
}

impl Labels {
    pub fn update(&mut self, labels: Vec<Label>) {
        if self.labels == labels {
            return;
        }
        self.radius_points = labels
            .iter()
            .map(|label| {
                let [x0, y0, x1, y1] = label.bounds;
                x0.abs().max(x1.abs()).hypot(y0.abs().max(y1.abs()))
            })
            .fold(0., f64::max);
        self.index.replace_points(
            labels
                .iter()
                .enumerate()
                .map(|(i, label)| (i as u64, label.anchor)),
        );
        self.labels = labels;
    }

    pub fn pick(
        &self,
        ray: Ray,
        view: View,
        visible: impl Fn(u64, DVec3, DVec3) -> bool,
    ) -> Option<(u64, DVec3)> {
        if !view.units.is_finite()
            || view.units <= 0.
            || !view.font_scale.is_finite()
            || view.font_scale <= 0.
        {
            return None;
        }
        // The shader clamps the tracking-space distance to eight metres.
        let radius = self.radius_points * 8.0 * 0.0011 * view.font_scale / view.units;
        let hit = self.index.query_padded(ray, radius, |i| {
            let label = &self.labels[i as usize];
            let scale = (label.anchor.distance(view.head) * view.units).clamp(0.3, 8.0)
                * 0.0011
                * view.font_scale
                / view.units;
            let [x0, y0, x1, y1] = label.bounds;
            let corner = |x, y| label.anchor + (view.right * x - view.up * y) * scale;
            let quad = [
                corner(x0, y0),
                corner(x1, y0),
                corner(x1, y1),
                corner(x0, y1),
            ];
            let (point, _) = quad_point(ray, quad)?;
            visible(label.id, label.anchor, point).then_some(point)
        })?;
        Some((self.labels[hit.id as usize].id, hit.point))
    }
}
