//! Head-tracked stereo scene views. Both eyes share retained world geometry.
use crate::{app_host::XrPanelPreferences, renderer};
use benchup_core::{Camera, DisplaySettings, Model};
use eframe::{egui, egui_wgpu, wgpu};
use glam::{DMat4, DQuat, DVec2, DVec3, DVec4};
use openxr as xr;
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone)]
pub(crate) struct Source {
    pub model: Arc<Model>,
    pub revision: u64,
    pub camera: Camera,
    pub display: DisplaySettings,
    pub selected: BTreeSet<u64>,
    pub overlays: crate::app::viewport_overlays::Overlays,
    pub references: Arc<crate::app::viewport_overlays::Overlays>,
    pub tool: benchup_core::Tool,
    pub hover: Option<u64>,
    pub outliner_hover: BTreeSet<u64>,
    pub erasing: BTreeSet<u64>,
    pub joinery_members: Option<[u64; 2]>,
    pub editing_text: Option<u64>,
    pub grain_preview: std::collections::BTreeMap<u64, DVec3>,
    pub paste: Option<PasteSource>,
    pub status: String,
}

#[derive(Clone)]
pub(crate) struct PasteSource {
    pub model: Arc<Model>,
    pub offset: DVec3,
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
    picking: crate::picking::PickingCache,
    input: Option<super::input::SceneInput>,
    navigation: super::navigation::Navigation,
    lines: super::world_lines::Lines,
    text: super::world_text::Text,
    head: DMat4,
    dark: bool,
    sections: crate::app::display_scene::SectionCache,
    surfaces: super::world_surfaces::Surfaces,
    backdrop: super::backdrop::Backdrop,
    grid: super::floor_grid::Grid,
    paste_cache: renderer::SceneCache,
    paste_faces: renderer::FaceCache,
    paste_entities: renderer::EntityCache,
    paste_source: Option<Arc<Model>>,
    paste_revision: u64,
    paste_origin: DVec3,
    paste_text: super::world_text::Text,
    paste_lines: super::world_lines::Lines,
    empty_references: Arc<crate::app::viewport_overlays::Overlays>,
}

impl Scene {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self {
            ctx: egui::Context::default(),
            cache: Default::default(),
            faces: Default::default(),
            entities: Default::default(),
            source: None,
            placement: None,
            depth: None,
            picking: Default::default(),
            input: None,
            navigation: Default::default(),
            lines: super::world_lines::Lines::new(device, format),
            text: super::world_text::Text::new(device, format),
            head: DMat4::IDENTITY,
            dark: false,
            sections: Default::default(),
            surfaces: super::world_surfaces::Surfaces::new(device, format),
            backdrop: super::backdrop::Backdrop::new(device, format),
            grid: super::floor_grid::Grid::new(device, format),
            paste_cache: Default::default(),
            paste_faces: Default::default(),
            paste_entities: Default::default(),
            paste_source: None,
            paste_revision: 0,
            paste_origin: DVec3::ZERO,
            paste_text: super::world_text::Text::new(device, format),
            paste_lines: super::world_lines::Lines::new(device, format),
            empty_references: Default::default(),
        }
    }

    pub fn begin_frame(&mut self, source: Source, anchor: xr::Posef, fit: bool) {
        if fit {
            self.navigation.reset();
        }
        self.head = pose_matrix(anchor);
        self.faces
            .update_for_revision(&source.model, source.revision);
        self.entities
            .update_for_revision(&source.model, source.revision);
        let display =
            crate::app::display_scene::effective_display(&source.display, &source.model, self.dark);
        self.picking.set_display(&display);
        self.sections
            .geometry(&self.faces, &self.entities, &display);
        if let Some(paste) = &source.paste {
            if self
                .paste_source
                .as_ref()
                .is_none_or(|old| !Arc::ptr_eq(old, &paste.model))
            {
                self.paste_revision = self.paste_revision.wrapping_add(1);
                self.paste_source = Some(Arc::clone(&paste.model));
                self.paste_origin = paste
                    .model
                    .bounds()
                    .map_or(DVec3::ZERO, |(a, b)| (a + b) * 0.5);
                self.paste_faces
                    .update_for_revision(&paste.model, self.paste_revision);
                self.paste_entities
                    .update_for_revision(&paste.model, self.paste_revision);
            }
        } else if self.paste_source.take().is_some() {
            self.paste_cache = Default::default();
            self.paste_faces = Default::default();
            self.paste_entities = Default::default();
        }
        self.picking.update(&self.faces);
        if self.placement.is_none() || fit {
            if let Some((min, max)) = source.model.bounds().or_else(|| {
                source
                    .paste
                    .as_ref()
                    .and_then(|p| p.model.bounds().map(|(a, b)| (a + p.offset, b + p.offset)))
            }) {
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

    pub fn set_input(&mut self, input: Option<super::input::SceneInput>) {
        self.input = input;
    }

    pub fn navigate(&mut self, input: super::navigation::Input, preferences: XrPanelPreferences) {
        let previous = self.navigation.grabbing();
        let pivot = self.placement.as_ref().map_or(DVec3::ZERO, |p| {
            p.model_to_tracking(preferences.scene_scale as f64)
                .transform_point3(p.center)
        });
        self.navigation
            .update(input, preferences.keep_upright, pivot);
        if previous != self.navigation.grabbing() {
            println!(
                "[BenchUpXR] Scene grab {} (relative scale {:.3})",
                if self.navigation.grabbing() {
                    "started"
                } else {
                    "released"
                },
                self.navigation.scale()
            );
        }
    }

    pub fn diagnostic_eye(&mut self, scale: f32, size: [u32; 2]) -> Option<xr::View> {
        let source = self.source.as_ref()?;
        let transform =
            self.navigation.matrix() * self.placement.as_ref()?.model_to_tracking(scale as f64);
        let camera = &source.camera;
        let position = transform.transform_point3(camera.eye()).as_vec3();
        let right = transform.transform_vector3(camera.right()).normalize();
        let up = transform.transform_vector3(camera.up()).normalize();
        let back = transform
            .transform_vector3(camera.eye() - camera.target)
            .normalize();
        let rotation = DQuat::from_mat3(&glam::DMat3::from_cols(right, up, back)).as_quat();
        let half_y = camera.fov as f32 * 0.5;
        let half_x = (half_y.tan() * size[0] as f32 / size[1] as f32).atan();
        let eye = xr::View {
            pose: xr::Posef {
                position: xr::Vector3f {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                },
                orientation: xr::Quaternionf {
                    x: rotation.x,
                    y: rotation.y,
                    z: rotation.z,
                    w: rotation.w,
                },
            },
            fov: xr::Fovf {
                angle_left: -half_x,
                angle_right: half_x,
                angle_up: half_y,
                angle_down: -half_y,
            },
        };
        self.head = pose_matrix(eye.pose);
        Some(eye)
    }

    pub fn stage_labels(&mut self, ctx: &egui::Context, source: &Source) {
        let mut main = source.clone();
        if let Some(placement) = &self.placement {
            use crate::app::viewport_overlays::{Billboard, Glyph};
            let colors = crate::theme::Palette::for_painter(&ctx.debug_painter());
            main.overlays.billboards.push(Billboard {
                point: placement.center + DVec3::new(0.0, 0.0, 1.05 / placement.fit_scale),
                owners: Default::default(),
                depth: false,
                shapes: vec![Glyph::Paragraph {
                    text: format!("{}\n{}", source.status, self.navigation.hint()),
                    size: 12.0,
                    width: 320.0,
                    color: colors.ink,
                    background: colors.paper,
                }],
            });
        }
        let source = &main;
        let dark = ctx.global_style().visuals.dark_mode;
        if self.dark != dark || self.ctx.global_style().visuals.dark_mode != dark {
            self.dark = dark;
            self.ctx.set_visuals(ctx.global_style().visuals.clone());
        }
        self.ctx.set_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        self.text.stage(
            ctx,
            source,
            self.input
                .and_then(|input| input.hit)
                .map(|hit| hit.entity)
                .or(source.hover),
            None,
        );
        if let Some(paste) = &source.paste {
            let mut secondary = source.clone();
            secondary.model = Arc::clone(&paste.model);
            secondary.revision = Arc::as_ptr(&paste.model) as u64;
            secondary.display = paste_display(&source.display, paste.offset);
            secondary.display.group_names = false;
            secondary.selected.clear();
            secondary.hover = None;
            secondary.outliner_hover.clear();
            secondary.erasing.clear();
            secondary.editing_text = None;
            secondary.joinery_members = None;
            secondary.grain_preview.clear();
            secondary.overlays = Default::default();
            secondary.references = Arc::clone(&self.empty_references);
            secondary.paste = None;
            let colors = crate::theme::Palette::for_painter(&ctx.debug_painter());
            self.paste_text
                .stage(ctx, &secondary, None, Some(colors.green));
            // Paste text may grow the shared atlas after main labels were shaped.
            self.text.stage(
                ctx,
                source,
                self.input
                    .and_then(|input| input.hit)
                    .map(|hit| hit.entity)
                    .or(source.hover),
                None,
            );
        } else {
            self.paste_text.clear();
        }
    }

    pub fn pick_ray(&self, ray: benchup_core::Ray, scale: f32) -> Option<super::input::SceneHit> {
        let (Some(source), Some(placement)) = (&self.source, &self.placement) else {
            return None;
        };
        let transform = self.navigation.matrix() * placement.model_to_tracking(scale as f64);
        let inverse = transform.inverse();
        let model_ray = benchup_core::Ray {
            origin: inverse.transform_point3(ray.origin),
            direction: inverse.transform_vector3(ray.direction).normalize(),
        };
        let (face, _, point) =
            self.picking
                .nearest(&self.faces, &self.entities, model_ray, false)?;
        let entity = self.entities.get(face).context?;
        if !source.model.contains_entity(&entity) {
            return None;
        }
        let tracking_point = transform.transform_point3(point);
        Some(super::input::SceneHit {
            entity,
            point,
            tracking_point,
            distance: tracking_point.distance(ray.origin),
        })
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
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
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
                let transform = self.navigation.matrix()
                    * placement.model_to_tracking(preferences.scene_scale as f64);
                let eye_to_tracking = pose_matrix(eye.pose);
                let model_from_eye = transform.inverse() * eye_to_tracking;
                let view = renderer::RenderView {
                    view_projection: projection(eye.fov, 0.03, 200.0)
                        * eye_to_tracking.inverse()
                        * transform,
                    eye: model_from_eye.transform_point3(DVec3::ZERO),
                    toward_eye: model_from_eye.transform_vector3(DVec3::Z).normalize(),
                    colors: None,
                };
                let mut display = crate::app::display_scene::effective_display(
                    &source.display,
                    &source.model,
                    self.dark,
                );
                if source.model.faces.is_empty() {
                    display.shadows = false;
                }
                let mut camera = source.camera.clone();
                camera.projection = benchup_core::Projection::Perspective;
                let key = renderer::SceneKey {
                    joinery_members: source.joinery_members,
                    joint_preview: None,
                    grain_preview: source.grain_preview.clone(),
                    model_revision: source.revision,
                    edit_context: source.model.edit_context,
                    camera,
                    size: DVec2::new(size[0] as f64, size[1] as f64),
                    pixels_per_point: 1.0,
                    display,
                    dark: self.dark,
                    tool: source.tool,
                    selected: source.selected.clone(),
                    hover: self
                        .input
                        .and_then(|input| input.hit)
                        .map(|hit| hit.entity)
                        .or(source.hover),
                    outliner_hover: source.outliner_hover.clone(),
                    erasing: source.erasing.clone(),
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
                if let Some(paste) = &source.paste {
                    let paste_display = paste_display(&source.display, paste.offset);
                    let colors = crate::theme::Palette::for_ui(ui);
                    let paste_view = renderer::RenderView {
                        view_projection: view.view_projection
                            * DMat4::from_translation(paste.offset),
                        eye: view.eye - paste.offset,
                        toward_eye: view.toward_eye,
                        colors: Some([crate::theme::FacePalette::for_ui(ui).paste, colors.green]),
                    };
                    let mut paste_key = key.clone();
                    paste_key.display = paste_display;
                    paste_key.model_revision = self.paste_revision;
                    paste_key.edit_context = None;
                    paste_key.selected.clear();
                    paste_key.hover = None;
                    paste_key.outliner_hover.clear();
                    paste_key.erasing.clear();
                    paste_key.joinery_members = None;
                    paste_key.joint_preview = None;
                    paste_key.grain_preview.clear();
                    self.paste_cache.paint_resident_view(
                        &paste_key,
                        &paste.model,
                        &self.paste_faces,
                        &self.paste_entities,
                        ui,
                        rect,
                        renderer::Mesh::new(&paste_key.camera, paste_key.size),
                        Some(&paste_view),
                    );
                }
            },
        );
        let jobs = self
            .ctx
            .tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: 1.0,
        };
        if let (Some(source), Some(placement)) = (&self.source, &self.placement) {
            use super::world_lines::Line;
            let spacing = crate::app::grid::configured_spacing(&source.camera, &source.display);
            let half = (1.6 / placement.fit_scale).max(0.1);
            let origin = placement.center;
            let mut tools = Vec::new();
            self.lines.owned_tools(
                device,
                (source.revision, Arc::as_ptr(&source.references) as usize),
                &source.references.owned_segments,
                |id| self.cache.resident_face_slot(id),
            );
            for (index, owned) in source.references.owned_segments.iter().enumerate() {
                let line = &owned.line;
                let color = line.color.to_normalized_gamma_f32();
                tools.push(
                    (if line.dashed {
                        Line::dashed(line.a - origin, line.b - origin, color, line.width)
                    } else {
                        Line::new(line.a - origin, line.b - origin, color, line.width)
                    })
                    .owned(self.lines.owner_range(index)),
                );
            }
            for line in source
                .overlays
                .segments
                .iter()
                .chain(&source.references.segments)
            {
                let color = line.color.to_normalized_gamma_f32();
                tools.push(if line.dashed {
                    Line::dashed(line.a - origin, line.b - origin, color, line.width)
                } else {
                    Line::new(line.a - origin, line.b - origin, color, line.width)
                });
            }
            tools.extend(
                source
                    .overlays
                    .markers
                    .iter()
                    .chain(&source.references.markers)
                    .map(|(point, color)| {
                        Line::marker(*point - origin, color.to_normalized_gamma_f32(), 4.0)
                    }),
            );
            tools.extend(source.overlays.ticks.iter().map(|(point, toward, color)| {
                Line::tick(
                    *point - origin,
                    *toward - origin,
                    color.to_normalized_gamma_f32(),
                )
            }));
            for line in source
                .overlays
                .overlay_segments
                .iter()
                .chain(&source.references.overlay_segments)
            {
                tools.push(
                    Line::new(
                        line.a - origin,
                        line.b - origin,
                        line.color.to_normalized_gamma_f32(),
                        line.width,
                    )
                    .overlay(),
                );
            }
            for &(a, b, color) in source
                .overlays
                .arrows
                .iter()
                .chain(&source.references.arrows)
            {
                tools.push(Line::arrow(
                    a - origin,
                    b - origin,
                    color.to_normalized_gamma_f32(),
                ));
            }
            self.lines.tools(device, tools);
            let reference_key = vec![
                origin.x.to_bits(),
                origin.y.to_bits(),
                origin.z.to_bits(),
                half.to_bits(),
                spacing.to_bits(),
                u64::from(source.display.grid),
                u64::from(source.display.axes),
                source.display.grid_max_length.to_bits(),
                self.text.revision,
                self.sections.generation,
            ];
            self.lines.references(device, reference_key, || {
                let mut lines = Vec::new();

                if source.display.axes {
                    for (axis, rgb) in [DVec3::X, DVec3::Y, DVec3::Z]
                        .into_iter()
                        .zip(source.display.axis_colors)
                    {
                        let color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
                            .to_normalized_gamma_f32();
                        lines.push(Line::new(
                            -axis * half * 10000.0 - origin,
                            axis * half * 10000.0 - origin,
                            color,
                            2.0,
                        ));
                    }
                }
                let colors = crate::theme::Palette::for_painter(&self.ctx.debug_painter());
                if source.display.guides {
                    for (id, guide) in source
                        .model
                        .guides
                        .iter()
                        .filter(|(id, _)| !source.model.is_hidden(**id))
                    {
                        let color = if self.entities.selected(&source.model, *id, &source.selected)
                        {
                            colors.accent
                        } else if self.input.and_then(|input| input.hit).is_some_and(|hit| {
                            source.model.entity_in_context(*id) == Some(hit.entity)
                        }) {
                            colors.green
                        } else {
                            egui::Color32::from_rgb(0, 174, 214)
                        }
                        .to_normalized_gamma_f32();
                        if let Some(direction) = guide.direction {
                            let center =
                                guide.origin + direction * (origin - guide.origin).dot(direction);
                            lines.push(Line::dashed(
                                center - direction * half * 10.0 - origin,
                                center + direction * half * 10.0 - origin,
                                color,
                                1.5,
                            ));
                        } else {
                            lines.push(Line::marker(guide.origin - origin, color, 4.0));
                        }
                    }
                }
                if let Some((lo, hi)) = self.faces.context_bounds(&source.model, &self.entities) {
                    let padding = ((hi - lo).max_element() * 0.04).max(0.05);
                    let lo = lo - DVec3::splat(padding);
                    let hi = hi + DVec3::splat(padding);
                    let corner = |i: usize| {
                        DVec3::new(
                            if i & 1 == 0 { lo.x } else { hi.x },
                            if i & 2 == 0 { lo.y } else { hi.y },
                            if i & 4 == 0 { lo.z } else { hi.z },
                        )
                    };
                    for i in 0..8 {
                        for axis in 0..3 {
                            if i & (1 << axis) == 0 {
                                lines.push(Line::dashed(
                                    corner(i) - origin,
                                    corner(i | (1 << axis)) - origin,
                                    colors.muted.to_normalized_gamma_f32(),
                                    1.0,
                                ));
                            }
                        }
                    }
                }
                for &(a, b, color) in &self.text.lines {
                    lines.push(Line::new(a - origin, b - origin, color, 1.5));
                }
                for &(a, b, color) in &self.text.ticks {
                    lines.push(Line::tick(a - origin, b - origin, color));
                }
                for &(a, b) in &self.sections.segments {
                    lines.push(Line::new(
                        a - origin,
                        b - origin,
                        crate::theme::ACCENT.to_normalized_gamma_f32(),
                        3.0,
                    ));
                }
                for quad in
                    crate::app::display_scene::section_planes(&source.model, &source.display)
                {
                    for i in 0..4 {
                        lines.push(Line::dashed(
                            quad[i] - origin,
                            quad[(i + 1) % 4] - origin,
                            crate::theme::ACCENT.to_normalized_gamma_f32(),
                            1.5,
                        ));
                    }
                }
                lines
            });
            let transform = self.navigation.matrix()
                * placement.model_to_tracking(preferences.scene_scale as f64);
            let tracking_view = projection(eye.fov, 0.03, 200.0) * pose_matrix(eye.pose).inverse();
            let display = crate::app::display_scene::effective_display(
                &source.display,
                &source.model,
                self.dark,
            );
            let model_eye = transform
                .inverse()
                .transform_point3(pose_matrix(eye.pose).transform_point3(DVec3::ZERO));
            let forward = transform
                .inverse()
                .transform_vector3(pose_matrix(eye.pose).transform_vector3(DVec3::Z))
                .normalize();
            self.grid.prepare(
                queue,
                tracking_view * transform * DMat4::from_translation(origin),
                model_eye,
                forward,
                origin,
                spacing,
                &display,
                self.dark,
            );
            self.backdrop.prepare(
                queue,
                tracking_view * transform * DMat4::from_translation(origin),
                model_eye - origin,
                &display,
                self.dark,
                source.display.shadows,
            );
            let surface_key = super::world_surfaces::Key {
                revision: source.revision,
                sections: self.sections.generation,
                display,
                dark: self.dark,
                origin,
                previews: source.overlays.surfaces.clone(),
            };
            self.surfaces.prepare(
                device,
                queue,
                surface_key,
                tracking_view * transform * DMat4::from_translation(origin),
                || {
                    let color = crate::theme::FacePalette::new(self.dark).section;
                    self.sections
                        .triangles
                        .iter()
                        .map(|points| super::world_surfaces::Triangle {
                            points: *points,
                            color,
                        })
                        .chain(source.overlays.surfaces.iter().cloned())
                        .collect()
                },
            );
            self.text.prepare(
                device,
                queue,
                origin,
                tracking_view,
                transform,
                self.head,
                |id| self.cache.resident_face_slot(id),
                source.display.section_limits(),
                preferences.ui_scale / 1.3,
            );
            if let Some(paste) = &source.paste {
                let paste_origin = self.paste_origin;
                let paste_transform = transform * DMat4::from_translation(paste.offset);
                let limits = paste_display(&source.display, paste.offset).section_limits();
                self.paste_text.prepare(
                    device,
                    queue,
                    paste_origin,
                    tracking_view,
                    paste_transform,
                    self.head,
                    |_| 0,
                    limits,
                    preferences.ui_scale / 1.3,
                );
                self.paste_lines.references(
                    device,
                    vec![self.paste_revision, self.paste_text.revision],
                    || {
                        let mut lines = Vec::new();
                        for &(a, b, color) in &self.paste_text.lines {
                            lines.push(Line::new(a - paste_origin, b - paste_origin, color, 1.5));
                        }
                        for &(a, b, color) in &self.paste_text.ticks {
                            lines.push(Line::tick(a - paste_origin, b - paste_origin, color));
                        }
                        lines
                    },
                );
                self.paste_lines.prepare(
                    queue,
                    tracking_view * paste_transform * DMat4::from_translation(paste_origin),
                    tracking_view,
                    size,
                    &[],
                    limits,
                    paste_origin,
                );
            }
            let mut pointer = Vec::new();
            if let Some(input) = self.input {
                if let Some(hit) = input.hit {
                    let color = if input.pressed {
                        [1.0, 0.65, 0.1, 1.0]
                    } else {
                        [0.1, 0.9, 1.0, 1.0]
                    };
                    let target = transform.transform_point3(hit.point);
                    let point = target - input.ray.direction * 0.002;
                    let radius =
                        (hit.tracking_point.distance(input.ray.origin) * 0.004).clamp(0.004, 0.03);
                    pointer.push(Line::new(input.ray.origin, point, color, 1.8));
                    for axis in [DVec3::X, DVec3::Y, DVec3::Z] {
                        pointer.push(Line::new(
                            point - axis * radius,
                            point + axis * radius,
                            color,
                            2.2,
                        ));
                    }
                }
            }
            self.lines.prepare(
                queue,
                tracking_view * transform * DMat4::from_translation(origin),
                tracking_view,
                size,
                &pointer,
                source.display.section_limits(),
                origin,
            );
        }
        // Eye output contains only a GPU callback. Its private egui context must
        // not replace the editor's managed font atlas in the shared renderer.
        output.textures_delta.clear();
        let mut encoder = device.create_command_encoder(&Default::default());
        let commands = renderer.update_buffers(device, queue, &mut encoder, &jobs, &screen);
        let visibility = self.cache.resident_visibility_targets();
        if let Some((faces, depth)) = &visibility {
            self.surfaces
                .paint_visibility(&mut encoder, size, faces, depth);
        }
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
            let mut pass = pass.forget_lifetime();
            self.backdrop.paint(&mut pass, size);
            renderer.render(&mut pass, &jobs, &screen);
            self.grid.paint(&mut pass, size);
            self.surfaces.paint(&mut pass, size);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BenchUpXR world hints and annotations"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            let depth = &self.depth.as_ref().unwrap().1;
            pass.set_viewport(0.0, 0.0, size[0] as f32, size[1] as f32, 0.0, 1.0);
            pass.set_scissor_rect(0, 0, size[0], size[1]);
            self.lines.paint(
                device,
                &mut pass,
                size,
                visibility.as_ref().map(|v| &v.0),
                depth,
            );
            self.text.paint(
                device,
                renderer,
                &mut pass,
                visibility.as_ref().map(|v| &v.0),
                depth,
            );
            if self
                .source
                .as_ref()
                .is_some_and(|source| source.paste.is_some())
            {
                self.paste_lines.paint(
                    device,
                    &mut pass,
                    size,
                    visibility.as_ref().map(|v| &v.0),
                    depth,
                );
                self.paste_text.paint(
                    device,
                    renderer,
                    &mut pass,
                    visibility.as_ref().map(|v| &v.0),
                    depth,
                );
            }
        }
        let submission = queue.submit(commands.into_iter().chain([encoder.finish()]));
        submission
    }
}

fn paste_display(source: &DisplaySettings, offset: DVec3) -> DisplaySettings {
    let mut display = source.clone();
    display.face_style = benchup_core::FaceStyle::Shaded;
    display.shadows = false;
    display.x_ray = false;
    display.edges = true;
    display.group_emphasis = false;
    display.color_by_tag = false;
    display.endpoints = false;
    display.profiles = false;
    display.depth_cue = false;
    display.extensions = false;
    display.hidden_geometry = false;
    display.hidden_objects = false;
    display.grain_direction = false;
    display.section_fill = false;
    display.section_planes = false;
    if display.section_cuts {
        display.section_height -= offset.z;
        display.section_offsets[0] -= offset.x;
        display.section_offsets[1] -= offset.y;
    }
    display
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
