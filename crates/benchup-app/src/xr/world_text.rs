//! Retained model-space labels using the editor's font atlas. No eye-space CPU layout.
use super::immersive::Source;
use crate::{renderer, theme::Palette};
use eframe::{egui, egui_wgpu, wgpu};
use glam::{DMat4, DVec3};
use wgpu::util::DeviceExt as _;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    anchor: [f32; 3],
    offset: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    owners: [u32; 2],
}

#[derive(PartialEq)]
struct Key {
    revision: u64,
    display: benchup_core::DisplaySettings,
    selected: std::collections::BTreeSet<u64>,
    hover: Option<u64>,
    pixels_per_point: f32,
    dark: bool,
    atlas: [usize; 2],
    tool_labels: Vec<crate::app::viewport_overlays::Label>,
    tool_billboards: Vec<crate::app::viewport_overlays::Billboard>,
    references: usize,
}

pub(super) struct Text {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    key: Option<Key>,
    markers: renderer::GroupMarkerCache,
    entities: renderer::EntityCache,
    meshes: Vec<(DVec3, egui::epaint::Mesh, Vec<u64>)>,
    owner_slots: wgpu::Buffer,
    vertices: Option<wgpu::Buffer>,
    indices: Option<wgpu::Buffer>,
    count: u32,
    origin: Option<DVec3>,
    dirty: bool,
    pub lines: Vec<(DVec3, DVec3, [f32; 4])>,
    pub ticks: Vec<(DVec3, DVec3, [f32; 4])>,
    pub revision: u64,
}

impl Text {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BenchUpXR world text"),
            source: wgpu::ShaderSource::Wgsl(include_str!("world_text.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BenchUpXR world text"), layout: None,
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vertex"),
                buffers: &[Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as _, step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32x2,3=>Float32x4,4=>Uint32x2] })], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fragment"),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: Default::default(), depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false), depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(), bias: wgpu::DepthBiasState { constant: -1, ..Default::default() } }),
            multisample: Default::default(), multiview_mask: None, cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BenchUpXR text view"),
            size: 176,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BenchUpXR text view"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            pipeline,
            uniform,
            binding,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            key: None,
            markers: Default::default(),
            entities: Default::default(),
            meshes: Vec::new(),
            owner_slots: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("BenchUpXR label owners"),
                contents: bytemuck::cast_slice(&[0u32]),
                usage: wgpu::BufferUsages::STORAGE,
            }),
            vertices: None,
            indices: None,
            count: 0,
            origin: None,
            dirty: false,
            lines: Vec::new(),
            ticks: Vec::new(),
            revision: 0,
        }
    }

    // Must run inside the editor's egui pass, before its atlas delta is uploaded.
    pub fn stage(&mut self, ctx: &egui::Context, source: &Source, hover: Option<u64>) {
        let mut key = Key {
            revision: source.revision,
            display: source.display.clone(),
            selected: source.selected.clone(),
            hover,
            pixels_per_point: ctx.pixels_per_point(),
            dark: ctx.global_style().visuals.dark_mode,
            atlas: ctx.fonts(|fonts| fonts.font_image_size()),
            tool_labels: source.overlays.labels.clone(),
            tool_billboards: source.overlays.billboards.clone(),
            references: std::sync::Arc::as_ptr(&source.references) as usize,
        };
        if self.key.as_ref() == Some(&key) {
            return;
        }
        self.entities
            .update_for_revision(&source.model, source.revision);
        let colors = Palette::for_painter(&ctx.debug_painter());
        let model = &source.model;
        let color = |id| {
            if self.entities.selected(model, id, &source.selected) {
                colors.accent
            } else if hover.is_some() && hover == model.entity_in_context(id) {
                colors.green
            } else if model.entity_in_context(id).is_none() {
                colors.muted
            } else {
                colors.ink
            }
        };
        let mut labels = Vec::new();
        let mut owners = std::collections::BTreeMap::<usize, Vec<u64>>::new();
        for label in source
            .overlays
            .labels
            .iter()
            .chain(&source.references.labels)
        {
            labels.push((
                label.point,
                label.text.clone(),
                egui::FontId::monospace(12.0),
                label.color,
                320.0,
            ));
        }
        let mut lines = Vec::new();
        let mut ticks = Vec::new();
        if source.display.group_names {
            for marker in self.markers.update(model, source.revision).iter() {
                if let Some(object) = model.objects.get(&marker.id).filter(|object| {
                    matches!(object.kind, benchup_core::ObjectKind::Group)
                        && self.entities.get(object.id).context == Some(object.id)
                        && !object.name.trim().is_empty()
                        && self.entities.get(object.id).displayed(&source.display)
                }) {
                    owners.insert(labels.len(), marker.owners.iter().copied().collect());
                    labels.push((
                        marker.point,
                        object.name.clone(),
                        egui::FontId::proportional(13.0),
                        color(object.id),
                        f32::INFINITY,
                    ));
                }
            }
        }
        if source.display.dimensions {
            for dimension in model
                .dimensions
                .values()
                .filter(|d| self.entities.get(d.id).displayed(&source.display))
            {
                let points = dimension.layout(model);
                let ink = color(dimension.id);
                for (a, b) in [(0, 2), (1, 3), (2, 3)] {
                    lines.push((points[a], points[b], ink.to_normalized_gamma_f32()));
                }
                for (a, b) in [(2, 3), (3, 2)] {
                    ticks.push((points[a], points[b], ink.to_normalized_gamma_f32()));
                }
                labels.push((
                    (points[2] + points[3]) * 0.5,
                    source.display.units.format_length(dimension.length(model)),
                    egui::FontId::monospace(12.0),
                    ink,
                    f32::INFINITY,
                ));
            }
        }
        for text in model
            .texts
            .values()
            .filter(|t| self.entities.get(t.id).displayed(&source.display))
        {
            let points = text.layout(model);
            let ink = color(text.id);
            lines.push((points[0], points[1], ink.to_normalized_gamma_f32()));
            labels.push((
                points[1],
                text.text.clone(),
                egui::FontId::monospace(12.0),
                ink,
                320.0,
            ));
        }
        // Allocate every glyph before tessellating: atlas growth must not leave
        // earlier labels with UVs normalized against an obsolete atlas size.
        let mut shapes: Vec<_> = labels
            .into_iter()
            .enumerate()
            .map(|(index, (anchor, text, font, ink, wrap))| {
                let galley = ctx.fonts_mut(|fonts| fonts.layout(text, font, ink, wrap));
                let bounds = egui::Rect::from_center_size(
                    egui::Pos2::ZERO,
                    galley.size() + egui::vec2(12.0, 8.0),
                );
                (
                    anchor,
                    vec![
                        egui::Shape::rect_filled(bounds, 4, colors.paper),
                        egui::Shape::galley(bounds.min + egui::vec2(6.0, 4.0), galley, ink),
                    ],
                    owners.remove(&index).unwrap_or_default(),
                )
            })
            .collect();
        for billboard in source
            .overlays
            .billboards
            .iter()
            .chain(&source.references.billboards)
        {
            use crate::app::viewport_overlays::Glyph;
            let mut glyphs = Vec::new();
            for glyph in &billboard.shapes {
                glyphs.push(match glyph {
                    Glyph::Circle {
                        center,
                        radius,
                        fill,
                        color,
                        width,
                    } => egui::Shape::Circle(egui::epaint::CircleShape {
                        center: egui::pos2(center[0], center[1]),
                        radius: *radius,
                        fill: *fill,
                        stroke: egui::Stroke::new(*width, *color),
                    }),
                    Glyph::Rect {
                        min,
                        max,
                        fill,
                        color,
                        width,
                    } => {
                        let rect = egui::Rect::from_min_max(
                            egui::pos2(min[0], min[1]),
                            egui::pos2(max[0], max[1]),
                        );
                        egui::Shape::Vec(vec![
                            egui::Shape::rect_filled(rect, 1, *fill),
                            egui::Shape::rect_stroke(
                                rect,
                                1,
                                egui::Stroke::new(*width, *color),
                                egui::StrokeKind::Inside,
                            ),
                        ])
                    }
                    Glyph::Polygon {
                        points,
                        fill,
                        color,
                        width,
                    } => egui::Shape::convex_polygon(
                        points.iter().map(|p| egui::pos2(p[0], p[1])).collect(),
                        *fill,
                        egui::Stroke::new(*width, *color),
                    ),
                    Glyph::Path {
                        points,
                        color,
                        width,
                    } => egui::Shape::line(
                        points.iter().map(|p| egui::pos2(p[0], p[1])).collect(),
                        egui::Stroke::new(*width, *color),
                    ),
                    Glyph::Text {
                        at,
                        text,
                        size,
                        color,
                    } => {
                        let galley = ctx.fonts_mut(|fonts| {
                            fonts.layout_no_wrap(
                                text.clone(),
                                egui::FontId::proportional(*size),
                                *color,
                            )
                        });
                        egui::Shape::galley(
                            egui::pos2(at[0], at[1]) - galley.size() * 0.5,
                            galley,
                            *color,
                        )
                    }
                    Glyph::Hint {
                        at,
                        text,
                        size,
                        color,
                        background,
                    } => {
                        let galley = ctx.fonts_mut(|fonts| {
                            fonts.layout_no_wrap(
                                text.clone(),
                                egui::FontId::proportional(*size),
                                *color,
                            )
                        });
                        let at = egui::pos2(at[0], at[1]);
                        let bounds = egui::Rect::from_min_size(
                            at - egui::vec2(6.0, 4.0),
                            galley.size() + egui::vec2(12.0, 8.0),
                        );
                        egui::Shape::Vec(vec![
                            egui::Shape::rect_filled(bounds, 4, *background),
                            egui::Shape::galley(at, galley, *color),
                        ])
                    }
                });
            }
            shapes.push((
                billboard.point,
                glyphs,
                if billboard.depth {
                    billboard.owners.iter().copied().collect()
                } else {
                    vec![0]
                },
            ));
        }
        self.meshes.clear();
        for (anchor, shapes, owners) in shapes {
            let jobs = ctx.tessellate(
                shapes
                    .into_iter()
                    .map(|shape| egui::epaint::ClippedShape::new(egui::Rect::EVERYTHING, shape))
                    .collect(),
                key.pixels_per_point,
            );
            for job in jobs {
                if let egui::epaint::Primitive::Mesh(mesh) = job.primitive {
                    self.meshes.push((anchor, mesh, owners.clone()));
                }
            }
        }
        key.atlas = ctx.fonts(|fonts| fonts.font_image_size());
        self.lines = lines;
        self.ticks = ticks;
        self.key = Some(key);
        self.dirty = true;
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        origin: DVec3,
        projection: DMat4,
        model: DMat4,
        head: DMat4,
        slot: impl Fn(u64) -> u32,
    ) {
        if self.dirty || self.origin != Some(origin) {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            let mut owner_slots = Vec::new();
            for (anchor, mesh, owners) in &self.meshes {
                let begin = owner_slots.len() as u32;
                let mut slots: Vec<_> = owners
                    .iter()
                    .copied()
                    .map(&slot)
                    .filter(|slot| *slot != 0)
                    .collect();
                slots.sort_unstable();
                slots.dedup();
                owner_slots.extend(slots);
                let range = if owners.as_slice() == [0] {
                    [u32::MAX, u32::MAX]
                } else {
                    [begin, owner_slots.len() as u32]
                };
                let base = vertices.len() as u32;
                vertices.extend(mesh.vertices.iter().map(|v| Vertex {
                    anchor: (*anchor - origin).as_vec3().to_array(),
                    offset: [v.pos.x, v.pos.y],
                    uv: [v.uv.x, v.uv.y],
                    color: v.color.to_normalized_gamma_f32(),
                    owners: range,
                }));
                indices.extend(mesh.indices.iter().map(|i| base + i));
            }
            self.count = indices.len() as u32;
            if owner_slots.is_empty() {
                owner_slots.push(0);
            }
            self.owner_slots = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("BenchUpXR retained label ownership"),
                contents: bytemuck::cast_slice(&owner_slots),
                usage: wgpu::BufferUsages::STORAGE,
            });
            if self.count > 0 {
                self.vertices = Some(device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("BenchUpXR retained glyph vertices"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ));
                self.indices = Some(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("BenchUpXR retained glyph indices"),
                        contents: bytemuck::cast_slice(&indices),
                        usage: wgpu::BufferUsages::INDEX,
                    }),
                );
            }
            self.origin = Some(origin);
            self.dirty = false;
        }
        let mut data = [0.0f32; 44];
        data[..16].copy_from_slice(&projection.as_mat4().to_cols_array());
        data[16..32].copy_from_slice(
            &(model * DMat4::from_translation(origin))
                .as_mat4()
                .to_cols_array(),
        );
        data[32..36].copy_from_slice(&head.x_axis.as_vec4().to_array());
        data[36..40].copy_from_slice(&head.y_axis.as_vec4().to_array());
        data[40..44].copy_from_slice(&head.w_axis.as_vec4().to_array());
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&data));
    }

    pub fn paint(
        &self,
        device: &wgpu::Device,
        renderer: &egui_wgpu::Renderer,
        pass: &mut wgpu::RenderPass<'_>,
        visibility: Option<&wgpu::TextureView>,
    ) {
        if self.count == 0 {
            return;
        }
        let Some(visibility) = visibility else {
            return;
        };
        let Some(texture) = renderer
            .texture(&egui::TextureId::Managed(0))
            .and_then(|t| t.texture.as_ref())
        else {
            return;
        };
        let view = texture.create_view(&Default::default());
        let atlas = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BenchUpXR shared editor atlas"),
            layout: &self.pipeline.get_bind_group_layout(1),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.binding, &[]);
        pass.set_bind_group(1, &atlas, &[]);
        let ownership = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BenchUpXR annotation visibility"),
            layout: &self.pipeline.get_bind_group_layout(2),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.owner_slots.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(visibility),
                },
            ],
        });
        pass.set_bind_group(2, &ownership, &[]);
        pass.set_vertex_buffer(0, self.vertices.as_ref().unwrap().slice(..));
        pass.set_index_buffer(
            self.indices.as_ref().unwrap().slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..self.count, 0, 0..1);
    }
}
