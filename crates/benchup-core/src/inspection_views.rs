//! Portable inspection viewpoints, including model placement relative to the user.
use crate::{Camera, Projection};
use glam::{DMat4, DVec3};
use serde::{Deserialize, Serialize};

pub type InspectionViewId = uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InspectionView {
    pub id: InspectionViewId,
    pub name: String,
    pub viewpoint: InspectionViewpoint,
}

impl InspectionView {
    pub fn new(name: String, viewpoint: InspectionViewpoint) -> Self {
        Self {
            id: InspectionViewId::new_v4(),
            name,
            viewpoint,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InspectionViewpoint {
    pub camera: Camera,
    /// Metres per model unit and placement in an upright, user-relative frame.
    /// No absolute tracking origin or physical-room coordinates are stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immersive: Option<DMat4>,
}

impl InspectionViewpoint {
    pub fn from_camera(camera: Camera) -> Self {
        Self {
            camera,
            immersive: None,
        }
    }

    pub fn valid(&self) -> bool {
        self.camera.valid() && self.immersive.is_none_or(valid_placement)
    }

    /// Capture the observed model-space camera as well as its physical placement.
    pub fn from_immersive(
        camera: &Camera,
        head_from_model: DMat4,
        upright_head_from_model: DMat4,
        focal_target: DVec3,
    ) -> Option<Self> {
        if !valid_placement(head_from_model) || !valid_placement(upright_head_from_model) {
            return None;
        }
        let model_from_head = head_from_model.inverse();
        let eye = model_from_head.transform_point3(DVec3::ZERO);
        let back = model_from_head.transform_vector3(DVec3::Z).normalize();
        let distance = eye.distance(focal_target).clamp(0.05, 1e6);
        let captured = Camera {
            target: eye - back * distance,
            distance,
            yaw: back.y.atan2(back.x),
            pitch: back.z.clamp(-1., 1.).asin(),
            projection: Projection::Perspective,
            fov: camera.fov,
        };
        let viewpoint = Self {
            camera: captured,
            immersive: Some(upright_head_from_model),
        };
        viewpoint.valid().then_some(viewpoint)
    }

    /// Desktop views become a perspective inspection 1.5 metres ahead. The
    /// headset's projection/FOV remains controlled by the runtime.
    pub fn immersive_placement(&self) -> DMat4 {
        self.immersive.unwrap_or_else(|| {
            let back = (self.camera.eye() - self.camera.target).normalize();
            let model_from_camera = DMat4::from_cols(
                self.camera.right().extend(0.),
                self.camera.up().extend(0.),
                back.extend(0.),
                self.camera.eye().extend(1.),
            );
            DMat4::from_scale(DVec3::splat(1.5 / self.camera.distance))
                * model_from_camera.inverse()
        })
    }
}

fn valid_placement(matrix: DMat4) -> bool {
    if !matrix.is_finite()
        || (matrix.w_axis.w - 1.).abs() > 1e-9
        || matrix.x_axis.w.abs() > 1e-9
        || matrix.y_axis.w.abs() > 1e-9
        || matrix.z_axis.w.abs() > 1e-9
    {
        return false;
    }
    let [x, y, z] = [
        matrix.x_axis.truncate(),
        matrix.y_axis.truncate(),
        matrix.z_axis.truncate(),
    ];
    let scale = x.length();
    if !(1e-12..=1e12).contains(&scale) {
        return false;
    }
    let [x, y, z] = [x / scale, y / scale, z / scale];
    (y.length_squared() - 1.).abs() < 1e-6
        && (z.length_squared() - 1.).abs() < 1e-6
        && x.dot(y).abs() < 1e-6
        && x.dot(z).abs() < 1e-6
        && y.dot(z).abs() < 1e-6
        && x.cross(y).dot(z) > 0.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;
    use glam::DQuat;

    #[test]
    fn saved_views_survive_project_and_autosave_checkpoint_roundtrips() {
        let mut doc = Document::default();
        let legacy = serde_json::to_string(&doc).unwrap();
        assert!(
            Document::from_json(&legacy)
                .unwrap()
                .inspection_views
                .is_empty()
        );
        let camera = Camera {
            target: DVec3::new(2., 3., 4.),
            distance: 3.,
            ..Camera::default()
        };
        let mut point = InspectionViewpoint::from_camera(camera);
        point.immersive = Some(DMat4::from_scale_rotation_translation(
            DVec3::splat(0.5),
            DQuat::from_rotation_z(0.4),
            DVec3::new(0.3, -0.2, -1.5),
        ));
        doc.inspection_views
            .push(InspectionView::new("Front joint".into(), point));
        let loaded = Document::from_json(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(loaded.inspection_views, doc.inspection_views);
        let base = Document::default();
        let restored = doc
            .checkpoint(&base.model, &base.history)
            .restore(&base.model, &base.history)
            .unwrap();
        assert_eq!(restored.inspection_views, doc.inspection_views);
        assert!(Document::default().inspection_views.is_empty());
    }

    #[test]
    fn desktop_and_immersive_viewpoints_share_the_observed_model_camera() {
        let camera = Camera {
            target: DVec3::new(2., 3., 4.),
            distance: 3.,
            ..Camera::default()
        };
        let view = InspectionViewpoint::from_camera(camera.clone());
        let placement = view.immersive_placement();
        assert!(placement.transform_point3(camera.eye()).length() < 1e-10);
        assert!(
            placement
                .transform_point3(camera.target)
                .distance(DVec3::new(0., 0., -1.5))
                < 1e-10
        );
        let captured =
            InspectionViewpoint::from_immersive(&camera, placement, placement, camera.target)
                .unwrap();
        assert!(captured.camera.eye().distance(camera.eye()) < 1e-10);
        assert!((captured.camera.yaw - camera.yaw).abs() < 1e-10);
        assert!((captured.camera.pitch - camera.pitch).abs() < 1e-10);
        assert_eq!(captured.immersive_placement(), placement);
    }
}
