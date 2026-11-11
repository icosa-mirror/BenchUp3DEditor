use super::*;
use benchup_core::StandardView;

#[test]
fn xr_standard_views_match_desktop_bases_and_keep_picking_invertible() {
    let bounds = Some((DVec3::new(-2., 1., -1.), DVec3::new(4., 5., 7.)));
    let anchor = xr::Posef {
        orientation: xr::Quaternionf { x: 0., y: 0.5, z: 0., w: 3_f32.sqrt() / 2. },
        position: xr::Vector3f { x: 3., y: 1.6, z: -2. },
    };
    let anchor_matrix = pose_matrix(anchor);
    let facing = DVec3::new(-Placement::OFFSET.x, 0., -Placement::OFFSET.z).normalize();
    for view in StandardView::ALL {
        let placement = Placement::fitted(bounds, anchor, WorldView::Standard(view), None);
        let matrix = placement.model_to_tracking(1.);
        let mut camera = Camera::default();
        camera.set_standard_view(view);
        for (model, tracking) in [
            (camera.right(), DVec3::Y.cross(facing)),
            (camera.up(), DVec3::Y),
            ((camera.eye() - camera.target).normalize(), facing),
        ] {
            assert!(matrix.transform_vector3(model).normalize().distance(
                anchor_matrix.transform_vector3(tracking).normalize()) < 1e-7, "{view:?}");
        }
        assert!(matrix.transform_point3(placement.center).distance(
            anchor_matrix.transform_point3(Placement::OFFSET)) < 1e-7);
        let point = DVec3::new(0.3, -0.7, 2.1);
        assert!(matrix.inverse().transform_point3(matrix.transform_point3(point)).distance(point) < 1e-8);
    }
}

#[test]
fn xr_framing_preserves_navigation_orientation_and_origin_scale_then_resets_upright() {
    let bounds = Some((DVec3::ZERO, DVec3::new(8., 4., 2.)));
    let initial = Placement::fitted(bounds, xr::Posef::IDENTITY, WorldView::Reset, None);
    let navigation = DMat4::from_scale_rotation_translation(DVec3::splat(2.5),
        DQuat::from_rotation_y(0.8), DVec3::new(4., 2., -1.));
    let previous = navigation * initial.model_to_tracking(1.);
    let anchor = xr::Posef { position: xr::Vector3f { x: 2., y: 1., z: 3. }, ..xr::Posef::IDENTITY };
    let origin = Placement::fitted(bounds, anchor, WorldView::Origin, Some((initial.center, previous)));
    assert_eq!(origin.center, DVec3::ZERO);
    let origin_matrix = origin.model_to_tracking(1.);
    for axis in [DVec3::X, DVec3::Y, DVec3::Z] {
        assert!(origin_matrix.transform_vector3(axis).distance(previous.transform_vector3(axis)) < 1e-8);
    }
    let selected = Some((DVec3::ONE, DVec3::splat(3.)));
    let fit = Placement::fitted(selected, anchor, WorldView::Selection, Some((initial.center, previous)));
    assert_eq!(fit.center, DVec3::splat(2.));
    assert!((fit.fit_scale - 0.8).abs() < 1e-8);
    assert!(fit.model_to_tracking(1.).transform_vector3(DVec3::X).normalize().distance(
        previous.transform_vector3(DVec3::X).normalize()) < 1e-8);
    let standard = Placement::fitted(bounds, anchor, WorldView::Standard(StandardView::Top), Some((initial.center, previous)));
    assert_eq!(standard.center, initial.center);
    assert!((standard.fit_scale - origin.fit_scale).abs() < 1e-8);
    assert!(standard.model_to_tracking(1.).transform_point3(standard.center).distance(
        previous.transform_point3(initial.center)) < 1e-8);
    let reset = Placement::fitted(bounds, anchor, WorldView::Reset, Some((initial.center, previous)));
    assert!(reset.model_to_tracking(1.).transform_vector3(DVec3::Z).normalize().distance(DVec3::Y) < 1e-8);
    let legacy = pose_matrix(anchor) * DMat4::from_translation(Placement::OFFSET)
        * DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2)
        * DMat4::from_scale(DVec3::splat(0.2)) * DMat4::from_translation(-DVec3::new(4., 2., 1.));
    assert!(legacy.abs_diff_eq(reset.model_to_tracking(1.), 1e-8));
}
