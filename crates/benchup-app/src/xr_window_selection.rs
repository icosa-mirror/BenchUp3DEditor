//! A tracked selection window; geometry is tested only when the gesture ends.
use benchup_core::{ConstructionGuide, Ray, selection::BoxSelectionView};
use glam::{DMat4, DVec2, DVec3};

#[derive(Clone, Copy)]
pub(crate) struct View {
    pub head: DVec3,
    pub right: DVec3,
    pub up: DVec3,
    pub units: f64,
    pub font_scale: f64,
    pub transform: DMat4,
}

pub(super) struct Window {
    origin: DVec3,
    right: DVec3,
    up: DVec3,
    normal: DVec3,
    unit: f64,
    transform: DMat4,
    view: View,
    pub end: DVec2,
}

impl Window {
    pub fn new(ray: Ray, point: Option<DVec3>, view: View) -> Option<Self> {
        if !view.units.is_finite() || view.units <= 0. || !view.font_scale.is_finite() || view.font_scale <= 0. { return None; }
        let origin = point.unwrap_or(ray.origin + ray.direction * (1.5 / view.units));
        let normal = (origin - view.head).try_normalize()?;
        let mut right = (view.right - normal * view.right.dot(normal)).try_normalize()
            .or_else(|| view.up.cross(normal).try_normalize())?;
        let mut up = right.cross(normal).normalize();
        if up.dot(view.up) < 0. { right = -right; up = -up; }
        let unit = (origin.distance(view.head) * view.units).clamp(0.3, 8.) * 0.0011 * view.font_scale / view.units;
        Some(Self { origin, right, up, normal, unit, transform: view.transform, view, end: DVec2::ZERO })
    }

    pub fn update(&mut self, ray: Ray, view: View) -> Option<DVec3> {
        if view.transform != self.transform || !view.head.is_finite() { return None; }
        self.view = view;
        let denominator = ray.direction.dot(self.normal);
        if denominator.abs() < 1e-10 { return None; }
        let distance = (self.origin - ray.origin).dot(self.normal) / denominator;
        if distance <= 0. || !distance.is_finite() { return None; }
        let point = ray.origin + ray.direction * distance;
        self.end = DVec2::new((point - self.origin).dot(self.right), (point - self.origin).dot(self.up)) / self.unit;
        self.end.is_finite().then_some(point)
    }

    fn corner(&self, point: DVec2) -> DVec3 { self.origin + (self.right * point.x + self.up * point.y) * self.unit }
    pub fn quad(&self) -> [DVec3; 4] {
        [self.origin, self.corner(DVec2::new(self.end.x, 0.)), self.corner(self.end), self.corner(DVec2::new(0., self.end.y))]
    }
}

impl BoxSelectionView for Window {
    fn project(&self, point: DVec3) -> Option<DVec2> {
        let delta = point - self.view.head;
        let depth = delta.dot(self.normal);
        let plane = (self.origin - self.view.head).dot(self.normal);
        if depth.abs() < 1e-10 || plane.abs() < 1e-10 || delta.length() * self.view.units < 0.005 { return None; }
        let t = plane / depth;
        if t <= 0. { return None; }
        let on_plane = self.view.head + delta * t - self.origin;
        Some(DVec2::new(on_plane.dot(self.right), on_plane.dot(self.up)) / self.unit)
    }

    fn label_bounds(&self, point: DVec3, size: DVec2) -> Option<(DVec2, DVec2)> {
        let unit = (point.distance(self.view.head) * self.view.units).clamp(0.3, 8.)
            * 0.0011 * self.view.font_scale / self.view.units;
        let mut lo = DVec2::splat(f64::INFINITY);
        let mut hi = DVec2::splat(f64::NEG_INFINITY);
        for (x, y) in [(-1., -1.), (-1., 1.), (1., -1.), (1., 1.)] {
            let corner = self.project(point + (self.view.right * size.x * x + self.view.up * size.y * y) * (unit * 0.5))?;
            lo = lo.min(corner); hi = hi.max(corner);
        }
        Some((lo, hi))
    }

    fn guide_segment(&self, guide: &ConstructionGuide) -> Option<(DVec2, DVec2)> {
        let direction = guide.direction?;
        let lo = DVec2::ZERO.min(self.end);
        let hi = DVec2::ZERO.max(self.end);
        let corners = [self.corner(lo), self.corner(DVec2::new(hi.x, lo.y)), self.corner(hi), self.corner(DVec2::new(lo.x, hi.y))];
        let center = self.corner((lo + hi) * 0.5) - self.view.head;
        let mut planes = Vec::with_capacity(5);
        for i in 0..4 {
            let mut n = (corners[i] - self.view.head).cross(corners[(i + 1) % 4] - self.view.head).try_normalize()?;
            if n.dot(center) < 0. { n = -n; }
            planes.push((n, n.dot(self.view.head)));
        }
        let n = self.normal * (self.origin - self.view.head).dot(self.normal).signum();
        planes.push((n, n.dot(self.view.head) + 0.005 / self.view.units));
        let (mut a, mut b) = (f64::NEG_INFINITY, f64::INFINITY);
        for (n, d) in planes {
            let value = n.dot(guide.origin) - d;
            let slope = n.dot(direction);
            if slope.abs() < 1e-12 { if value < -1e-9 { return None; } }
            else if slope > 0. { a = a.max(-value / slope); }
            else { b = b.min(-value / slope); }
        }
        if a > b { return None; }
        let first = if a.is_finite() { a } else if b.is_finite() { b - 1. } else { 0. };
        let last = if b.is_finite() { b } else { first + first.abs().max(1.) };
        Some((self.project(guide.origin + direction * first)?.clamp(lo, hi),
            self.project(guide.origin + direction * last)?.clamp(lo, hi)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> View { View { head: DVec3::ZERO, right: DVec3::X, up: DVec3::Y,
        units: 1., font_scale: 1., transform: DMat4::IDENTITY } }
    fn ray(point: DVec3) -> Ray { Ray { origin: DVec3::ZERO, direction: point.normalize() } }

    #[test]
    fn window_selection_projection_tracks_the_head_without_moving_the_world_frame() {
        let mut window = Window::new(ray(DVec3::Z), Some(DVec3::Z), view()).unwrap();
        let end = DVec3::new(0.4, 0.4, 1.);
        window.update(ray(end), view()).unwrap();
        let quad = window.quad();
        let projected = window.project(DVec3::new(0.4, 0.4, 2.)).unwrap();
        let bounds = window.label_bounds(DVec3::Z * 2., DVec2::new(20., 10.)).unwrap();
        assert!((bounds.1 - bounds.0).distance(DVec2::new(20., 10.)) < 1e-8);
        let moved = View { head: DVec3::X * 0.1, ..view() };
        window.update(ray(end), moved).unwrap();
        assert_eq!(window.quad(), quad);
        assert!(window.project(DVec3::new(0.4, 0.4, 2.)).unwrap().distance(projected) > 1.);
        assert!(window.project(-DVec3::Z).is_none());
        assert!(window.update(ray(end), View { transform: DMat4::from_scale(DVec3::splat(2.)), ..moved }).is_none());
    }

    #[test]
    fn window_selection_clips_infinite_guides_to_the_tracked_window() {
        let mut window = Window::new(ray(DVec3::Z), Some(DVec3::Z), view()).unwrap();
        window.update(ray(DVec3::new(0.4, 0.4, 1.)), view()).unwrap();
        let line = |y, z| ConstructionGuide { origin: DVec3::new(-10., y, z), direction: Some(DVec3::X) };
        assert!(window.guide_segment(&line(0.2, 2.)).is_some());
        assert!(window.guide_segment(&line(2., 2.)).is_none());
        assert!(window.guide_segment(&line(0.2, -2.)).is_none());
    }
}
