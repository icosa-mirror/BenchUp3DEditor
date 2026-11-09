//! Retained instanced world lines; clipping/projection/width are GPU operations.
use eframe::wgpu;
use wgpu::util::DeviceExt as _;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Line {
    a: [f32; 3],
    width: f32,
    b: [f32; 3],
    color: [f32; 4],
    style: [f32; 4],
}

impl Line {
    pub fn new(a: glam::DVec3, b: glam::DVec3, color: [f32; 4], width: f32) -> Self {
        Self {
            a: a.as_vec3().to_array(),
            b: b.as_vec3().to_array(),
            color,
            width,
            style: [0.0; 4],
        }
    }

    pub fn dashed(a: glam::DVec3, b: glam::DVec3, color: [f32; 4], width: f32) -> Self {
        Self {
            style: [0.0, 10.0, 6.0, 0.0],
            ..Self::new(a, b, color, width)
        }
    }

    pub fn marker(point: glam::DVec3, color: [f32; 4], radius: f32) -> Self {
        Self {
            style: [1.0, radius, 0.0, 0.0],
            ..Self::new(point, point, color, 1.5)
        }
    }

    pub fn tick(point: glam::DVec3, toward: glam::DVec3, color: [f32; 4]) -> Self {
        Self {
            style: [2.0, 0.0, 0.0, 0.0],
            ..Self::new(point, toward, color, 1.5)
        }
    }
}

pub(super) struct Lines {
    pipeline: wgpu::RenderPipeline,
    references: wgpu::Buffer,
    reference_count: u32,
    reference_key: Vec<u64>,
    pointer: wgpu::Buffer,
    pointer_count: u32,
    uniforms: [wgpu::Buffer; 2],
    bindings: [wgpu::BindGroup; 2],
}

impl Lines {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BenchUpXR world overlays"),
            source: wgpu::ShaderSource::Wgsl(include_str!("world_lines.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BenchUpXR world overlay lines"), layout: None,
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vertex"),
                buffers: &[Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Line>() as _, step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,3=>Float32,1=>Float32x3,2=>Float32x4,4=>Float32x4] })], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fragment"),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: Default::default(), depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false), depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(), bias: wgpu::DepthBiasState { constant: -1, ..Default::default() } }),
            multisample: Default::default(), multiview_mask: None, cache: None,
        });
        let uniform = || {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("BenchUpXR overlay view"),
                size: 80,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let uniforms = [uniform(), uniform()];
        let binding = |buffer: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("BenchUpXR overlay view"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };
        let bindings = [binding(&uniforms[0]), binding(&uniforms[1])];
        let empty = || {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("BenchUpXR overlay instances"),
                size: (32 * std::mem::size_of::<Line>()) as _,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            pipeline,
            references: empty(),
            reference_count: 0,
            reference_key: Vec::new(),
            pointer: empty(),
            pointer_count: 0,
            uniforms,
            bindings,
        }
    }

    pub fn references(
        &mut self,
        device: &wgpu::Device,
        key: Vec<u64>,
        build: impl FnOnce() -> Vec<Line>,
    ) {
        if self.reference_key != key {
            let lines = build();
            self.reference_count = lines.len() as u32;
            if !lines.is_empty() {
                self.references = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("BenchUpXR retained reference lines"),
                    contents: bytemuck::cast_slice(&lines),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            }
            self.reference_key = key;
        }
    }

    pub fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        model_matrix: glam::DMat4,
        tracking_matrix: glam::DMat4,
        size: [u32; 2],
        pointer: &[Line],
    ) {
        for (index, matrix) in [model_matrix, tracking_matrix].into_iter().enumerate() {
            let mut data = [0.0f32; 20];
            data[..16].copy_from_slice(&matrix.as_mat4().to_cols_array());
            data[16] = size[0] as f32;
            data[17] = size[1] as f32;
            queue.write_buffer(&self.uniforms[index], 0, bytemuck::cast_slice(&data));
        }
        self.pointer_count = pointer.len().min(32) as u32;
        if self.pointer_count != 0 {
            queue.write_buffer(
                &self.pointer,
                0,
                bytemuck::cast_slice(&pointer[..self.pointer_count as usize]),
            );
        }
    }

    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>, size: [u32; 2]) {
        pass.set_viewport(0.0, 0.0, size[0] as f32, size[1] as f32, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, size[0], size[1]);
        pass.set_pipeline(&self.pipeline);
        for (index, buffer, count) in [
            (0, &self.references, self.reference_count),
            (1, &self.pointer, self.pointer_count),
        ] {
            if count != 0 {
                pass.set_bind_group(0, &self.bindings[index], &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..count);
            }
        }
    }
}
