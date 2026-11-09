//! Retained model-space caps and translucent tool planes.
use eframe::wgpu;
use glam::{DMat4, DVec3};
use wgpu::util::DeviceExt as _;

pub(crate) use crate::app::viewport_overlays::Triangle;

#[derive(PartialEq)]
pub(super) struct Key {
    pub revision: u64,
    pub sections: u64,
    pub display: benchup_core::DisplaySettings,
    pub dark: bool,
    pub origin: DVec3,
    pub previews: Vec<Triangle>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    point: [f32; 3],
    color: [f32; 4],
}

pub(super) struct Surfaces {
    pipelines: [wgpu::RenderPipeline; 2],
    visibility: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
    key: Option<Key>,
    buffers: [Option<wgpu::Buffer>; 2],
    counts: [u32; 2],
}

impl Surfaces {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BenchUpXR world surfaces"),
            source: wgpu::ShaderSource::Wgsl(include_str!("world_surfaces.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BenchUpXR surface view"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BenchUpXR surfaces"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipelines = std::array::from_fn(|i| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("BenchUpXR retained surfaces"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex"),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x4],
                    })],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(i == 0),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: -1,
                        ..Default::default()
                    },
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BenchUpXR surface view"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let visibility = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BenchUpXR section cap visibility"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as _,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x4],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("visibility"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rg32Uint,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: -1,
                    ..Default::default()
                },
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BenchUpXR surface view"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            pipelines,
            visibility,
            uniform,
            binding,
            key: None,
            buffers: [None, None],
            counts: [0, 0],
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: Key,
        matrix: DMat4,
        build: impl FnOnce() -> Vec<Triangle>,
    ) {
        if self.key.as_ref() != Some(&key) {
            let mut vertices: [Vec<Vertex>; 2] = [Vec::new(), Vec::new()];
            for triangle in build() {
                let batch = usize::from(triangle.color.a() < 255);
                vertices[batch].extend(triangle.points.map(|point| Vertex {
                    point: (point - key.origin).as_vec3().to_array(),
                    color: triangle.color.to_normalized_gamma_f32(),
                }));
            }
            for (i, vertices) in vertices.iter().enumerate() {
                self.counts[i] = vertices.len() as u32;
                if !vertices.is_empty() {
                    self.buffers[i] = Some(device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("BenchUpXR cached surface geometry"),
                            contents: bytemuck::cast_slice(vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        },
                    ));
                }
            }
            self.key = Some(key);
        }
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&matrix.as_mat4().to_cols_array()),
        );
    }

    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>, size: [u32; 2]) {
        pass.set_viewport(0.0, 0.0, size[0] as f32, size[1] as f32, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, size[0], size[1]);
        for i in 0..2 {
            if self.counts[i] > 0 {
                pass.set_pipeline(&self.pipelines[i]);
                pass.set_bind_group(0, &self.binding, &[]);
                pass.set_vertex_buffer(0, self.buffers[i].as_ref().unwrap().slice(..));
                pass.draw(0..self.counts[i], 0..1);
            }
        }
    }

    pub fn paint_visibility(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        size: [u32; 2],
        faces: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) {
        if self.counts[0] == 0 {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BenchUpXR cap occlusion"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: faces,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_viewport(0.0, 0.0, size[0] as f32, size[1] as f32, 0.0, 1.0);
        pass.set_pipeline(&self.visibility);
        pass.set_bind_group(0, &self.binding, &[]);
        pass.set_vertex_buffer(0, self.buffers[0].as_ref().unwrap().slice(..));
        pass.draw(0..self.counts[0], 0..1);
    }
}
