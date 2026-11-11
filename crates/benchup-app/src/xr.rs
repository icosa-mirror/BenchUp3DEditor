//! Native XR groundwork. No runtime is loaded by the desktop application yet.
//!
//! Panel coordinates follow OpenXR: metres, +X right, +Y up, front facing +Z.
//! Transform tracked rays into panel-local space before calling `hit_test`.

use glam::{DVec2, DVec3};

#[cfg(feature = "openxr")]
pub mod runtime;

#[cfg(feature = "openxr")]
pub mod panel;

#[cfg(feature = "openxr")]
mod editor;

#[cfg(feature = "openxr")]
mod keyboard;

#[cfg(feature = "openxr")]
mod file_dialog;

#[cfg(feature = "openxr")]
mod input;

#[cfg(feature = "openxr")]
mod erase_sweep;

#[cfg(feature = "openxr")]
mod cursor;

#[cfg(feature = "openxr")]
pub(crate) mod immersive;

#[cfg(feature = "openxr")]
mod world_lines;

#[cfg(feature = "openxr")]
mod world_text;

#[cfg(feature = "openxr")]
mod world_labels;

#[cfg(feature = "openxr")]
pub(crate) mod world_inference;

#[cfg(feature = "openxr")]
mod world_surfaces;

#[cfg(feature = "openxr")]
mod backdrop;

#[cfg(feature = "openxr")]
mod floor_grid;

#[cfg(feature = "openxr")]
mod navigation;

#[cfg(feature = "openxr")]
mod world_picking;

#[cfg(feature = "openxr")]
pub(crate) mod section_picking;

#[cfg(feature = "openxr")]
mod desktop;

/// A flat editor surface. Logical size is in egui points, not texture pixels.
#[derive(Clone, Copy, Debug)]
pub struct EditorPanel {
    size_metres: DVec2,
    size_points: DVec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelHit {
    /// Position from the top-left of the editor, in egui points.
    pub position_points: DVec2,
    pub distance_metres: f64,
}

impl EditorPanel {
    /// Both sizes must be finite and strictly positive. The host should choose
    /// matching aspect ratios to avoid stretching the editor texture.
    pub fn new(size_metres: DVec2, size_points: DVec2) -> Option<Self> {
        let valid = |v: DVec2| v.is_finite() && v.min_element() > 0.0;
        (valid(size_metres) && valid(size_points)).then_some(Self {
            size_metres,
            size_points,
        })
    }

    /// Intersect a controller ray with the front of the panel at local Z=0.
    /// Accepts non-unit directions; rejects invalid, parallel, rear-facing,
    /// behind-origin and out-of-bounds intersections. This only maps input;
    /// surface rendering remains on the GPU.
    pub fn hit_test(self, origin: DVec3, direction: DVec3) -> Option<PanelHit> {
        let hit = self.project_ray(origin, direction)?;
        if hit.position_points.min_element() < 0.0
            || hit.position_points.x > self.size_points.x
            || hit.position_points.y > self.size_points.y
        {
            return None;
        }
        Some(hit)
    }

    /// Unbounded plane projection for a drag already captured by the panel.
    /// Invalid/backside rays still cancel interaction; only edge clipping differs.
    fn project_ray(self, origin: DVec3, direction: DVec3) -> Option<PanelHit> {
        if !origin.is_finite() || !direction.is_finite() {
            return None;
        }
        let direction = direction.try_normalize()?;
        if direction.z >= -1e-9 || origin.z <= 0.0 {
            return None;
        }
        let distance_metres = -origin.z / direction.z;
        let point = origin + direction * distance_metres;
        let uv = DVec2::new(
            point.x / self.size_metres.x + 0.5,
            0.5 - point.y / self.size_metres.y,
        );
        if !distance_metres.is_finite() || !uv.is_finite() {
            return None;
        }
        Some(PanelHit {
            position_points: uv * self.size_points,
            distance_metres,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> EditorPanel {
        EditorPanel::new(DVec2::new(2.0, 1.0), DVec2::new(1440.0, 720.0)).unwrap()
    }

    #[test]
    fn maps_center_and_corners_to_logical_points() {
        for (origin, expected) in [
            (DVec3::new(0.0, 0.0, 2.0), DVec2::new(720.0, 360.0)),
            (DVec3::new(-1.0, 0.5, 2.0), DVec2::ZERO),
            (DVec3::new(1.0, -0.5, 2.0), DVec2::new(1440.0, 720.0)),
        ] {
            let hit = panel().hit_test(origin, -DVec3::Z * 4.0).unwrap();
            assert_eq!(hit.position_points, expected);
            assert_eq!(hit.distance_metres, 2.0);
        }
    }

    #[test]
    fn angled_ray_uses_plane_intersection_and_metric_distance() {
        let hit = panel()
            .hit_test(DVec3::new(-0.5, 0.0, 1.0), DVec3::new(0.5, 0.0, -1.0))
            .unwrap();
        assert!((hit.position_points - DVec2::new(720.0, 360.0)).length() < 1e-9);
        assert!((hit.distance_metres - 1.25_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn rejects_misses_and_invalid_tracking() {
        for (origin, direction) in [
            (DVec3::Z, DVec3::X),
            (DVec3::Z, DVec3::Z),
            (-DVec3::Z, DVec3::Z),
            (DVec3::ZERO, -DVec3::Z),
            (DVec3::new(1.1, 0.0, 1.0), -DVec3::Z),
            (DVec3::Z, DVec3::ZERO),
            (DVec3::splat(f64::NAN), -DVec3::Z),
            (DVec3::Z, DVec3::splat(f64::INFINITY)),
        ] {
            assert!(panel().hit_test(origin, direction).is_none());
        }
    }

    #[test]
    fn rejects_invalid_panel_dimensions() {
        for size in [
            DVec2::ZERO,
            DVec2::new(-1.0, 1.0),
            DVec2::splat(f64::NAN),
            DVec2::splat(f64::INFINITY),
        ] {
            assert!(EditorPanel::new(size, DVec2::ONE).is_none());
            assert!(EditorPanel::new(DVec2::ONE, size).is_none());
        }
    }
}
