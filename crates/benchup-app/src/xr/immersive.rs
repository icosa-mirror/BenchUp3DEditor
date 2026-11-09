//! Head-tracked stereo scene views. Both eyes share retained world geometry.
use crate::{app_host::XrPanelPreferences, renderer};
use benchup_core::{Camera, DisplaySettings, Model};
use eframe::{egui, egui_wgpu, wgpu};
use glam::{DMat4, DQuat, DVec2, DVec3, DVec4};
use openxr as xr;
use std::{collections::BTreeSet, sync::Arc};

pub(crate) struct Source {
    pub model: Arc<Model>,
    pub revision: u64,
    pub camera: Camera,
    pub display: DisplaySettings,
    pub selected: BTreeSet<u64>,
}

struct Placement {
    center: DVec3,
    fit_scale: f64,
    anchor: xr::Posef,
}

pub(super) struct Scene {
    ctx: egui::Context,
    cache: renderer::SceneCache,
    faces: renderer::FaceCache,
    entities: renderer::EntityCache,
    source: Option<Source>,
    placement: Option<Placement>,
    depth: Option<([u32; 2], wgpu::TextureView)>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            ctx: egui::Context::default(),
            cache: Default::default(),
            faces: Default::default(),
            entities: Default::default(),
            source: None,
            placement: None,
            depth: None,
        }
    }

    pub fn begin_frame(&mut self, source: Source, anchor: xr::Posef, fit: bool) {
        self.faces
            .update_for_revision(&source.model, source.revision);
        self.entities
            .update_for_revision(&source.model, source.revision);
        if self.placement.is_none() || fit {
            if let Some((min, max)) = source.model.bounds() {
                let extent = (max - min).max_element().max(0.001);
                self.placement = Some(Placement {
                    center: (min + max) * 0.5,
                    fit_scale: 1.6 / extent,
                    anchor,
                });
                println!(
                    "[BenchUpXR] Immersive scene fitted: {} faces, scale {:.5} metres per model metre",
                    source.model.faces.len(),
                    1.6 / extent
                );
            }
        }
        self.source = Some(source);
    }

    pub fn paint_eye(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut egui_wgpu::Renderer,
        target: &wgpu::TextureView,
        size: [u32; 2],
        eye: xr::View,
        preferences: XrPanelPreferences,
    ) -> wgpu::SubmissionIndex {
        if self
            .depth
            .as_ref()
            .is_none_or(|(previous, _)| *previous != size)
        {
            self.depth = Some((
                size,
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("BenchUpXR eye depth"),
                        size: wgpu::Extent3d {
                            width: size[0],
                            height: size[1],
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Depth32Float,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&Default::default()),
            ));
        }
        let rect =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0] as f32, size[1] as f32));
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                focused: false,
                ..Default::default()
            },
            |ui| {
                let (Some(source), Some(placement)) = (&self.source, &self.placement) else {
                    return;
                };
                let transform = placement.model_to_tracking(preferences.scene_scale as f64);
                let eye_to_tracking = pose_matrix(eye.pose);
                let model_from_eye = transform.inverse() * eye_to_tracking;
                let view = renderer::RenderView {
                    view_projection: projection(eye.fov, 0.03, 200.0)
                        * eye_to_tracking.inverse()
                        * transform,
                    eye: model_from_eye.transform_point3(DVec3::ZERO),
                    toward_eye: model_from_eye.transform_vector3(DVec3::Z).normalize(),
                };
                let mut display = source.display.clone();
                if !matches!(
                    display.face_style,
                    benchup_core::FaceStyle::Materials | benchup_core::FaceStyle::Shaded
                ) {
                    display.face_style = benchup_core::FaceStyle::Materials;
                }
                // The first immersive path is opaque world geometry. UI annotations,
                // section previews and other special display modes remain on the panel.
                display.x_ray = false;
                display.fog = false;
                display.depth_cue = false;
                display.profiles = false;
                display.back_edges = false;
                display.endpoints = false;
                display.extensions = false;
                display.grain_direction = false;
                display.hidden_geometry = false;
                display.hidden_objects = false;
                display.section_planes = false;
                display.section_cuts = false;
                display.section_fill = false;
                display.shadows = false;
                let mut camera = source.camera.clone();
                camera.projection = benchup_core::Projection::Perspective;
                let key = renderer::SceneKey {
                    joinery_members: None,
                    joint_preview: None,
                    model_revision: source.revision,
                    edit_context: source.model.edit_context,
                    camera,
                    size: DVec2::new(size[0] as f64, size[1] as f64),
                    pixels_per_point: 1.0,
                    display,
                    dark: false,
                    tool: benchup_core::Tool::Orbit,
                    selected: source.selected.clone(),
                    hover: None,
                    outliner_hover: BTreeSet::new(),
                    erasing: BTreeSet::new(),
                };
                self.cache.paint_resident_view(
                    &key,
                    &source.model,
                    &self.faces,
                    &self.entities,
                    ui,
                    rect,
                    renderer::Mesh::new(&key.camera, key.size),
                    Some(&view),
                );
            },
        );
        let jobs = self
            .ctx
            .tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: 1.0,
        };
        // Eye output contains only a GPU callback. Its private egui context must
        // not replace the editor's managed font atlas in the shared renderer.
        output.textures_delta.clear();
        let mut encoder = device.create_command_encoder(&Default::default());
        let commands = renderer.update_buffers(device, queue, &mut encoder, &jobs, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BenchUpXR stereoscopic scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.025,
                            g: 0.035,
                            b: 0.05,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.as_ref().unwrap().1,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            renderer.render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        let submission = queue.submit(commands.into_iter().chain([encoder.finish()]));
        submission
    }
}

impl Placement {
    fn model_to_tracking(&self, scale: f64) -> DMat4 {
        // Model Z is up; OpenXR Y is up. Put the fitted model to the right of
        // the editor so the opaque UI panel does not cover the 3D scene.
        pose_matrix(self.anchor)
            * DMat4::from_translation(DVec3::new(2.05, -0.2, -1.43))
            * DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2)
            * DMat4::from_scale(DVec3::splat(self.fit_scale * scale))
            * DMat4::from_translation(-self.center)
    }
}

fn pose_matrix(pose: xr::Posef) -> DMat4 {
    DMat4::from_rotation_translation(
        DQuat::from_xyzw(
            pose.orientation.x as f64,
            pose.orientation.y as f64,
            pose.orientation.z as f64,
            pose.orientation.w as f64,
        )
        .normalize(),
        DVec3::new(
            pose.position.x as f64,
            pose.position.y as f64,
            pose.position.z as f64,
        ),
    )
}

fn projection(fov: xr::Fovf, near: f64, far: f64) -> DMat4 {
    let (left, right, up, down) = (
        (fov.angle_left as f64).tan(),
        (fov.angle_right as f64).tan(),
        (fov.angle_up as f64).tan(),
        (fov.angle_down as f64).tan(),
    );
    let (width, height) = (right - left, up - down);
    DMat4::from_cols(
        DVec4::new(2.0 / width, 0.0, 0.0, 0.0),
        DVec4::new(0.0, 2.0 / height, 0.0, 0.0),
        DVec4::new(
            (right + left) / width,
            (up + down) / height,
            -far / (far - near),
            -1.0,
        ),
        DVec4::new(0.0, 0.0, -far * near / (far - near), 0.0),
    )
}
