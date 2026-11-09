//! Infinite/finite perspective grid. Lattice, LOD, clipping and fade are GPU work.
use eframe::wgpu;
use glam::{DMat4, DVec3};
pub(super) struct Grid {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
}
impl Grid {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BenchUpXR floor grid"),
            source: wgpu::ShaderSource::Wgsl(include_str!("floor_grid.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BenchUpXR analytic grid"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BenchUpXR grid view"),
            size: 240,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BenchUpXR grid view"),
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
        }
    }
    pub fn prepare(
        &self,
        queue: &wgpu::Queue,
        matrix: DMat4,
        eye: DVec3,
        forward: DVec3,
        origin: DVec3,
        spacing: f64,
        display: &benchup_core::DisplaySettings,
        dark: bool,
    ) {
        let mut data = [0.0f32; 60];
        data[..16].copy_from_slice(&matrix.as_mat4().to_cols_array());
        data[16..32].copy_from_slice(&matrix.inverse().as_mat4().to_cols_array());
        data[32..36].copy_from_slice(&(eye - origin).extend(0.0).as_vec4().to_array());
        data[36..40].copy_from_slice(&[
            spacing as f32,
            display.grid_max_length as f32,
            u32::from(display.grid_infinite) as f32,
            eye.dot(forward).abs().max(0.005) as f32,
        ]);
        data[40..44].copy_from_slice(
            &origin
                .extend(u32::from(display.grid) as f64)
                .as_vec4()
                .to_array(),
        );
        let (minor, major) = if dark {
            ([61, 72, 61], [83, 96, 83])
        } else {
            ([188, 194, 175], [161, 168, 148])
        };
        for (offset, rgb) in [(44, minor), (48, major)] {
            for i in 0..3 {
                data[offset + i] = rgb[i] as f32 / 255.0;
            }
        }
        let limits = display.section_limits();
        for i in 0..3 {
            data[52 + i] = limits[i].map_or(f32::MAX, |v| (v - origin[i]) as f32);
        }
        data[56..60].copy_from_slice(&forward.extend(0.0).as_vec4().to_array());
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&data));
    }
    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>, size: [u32; 2]) {
        pass.set_viewport(0.0, 0.0, size[0] as f32, size[1] as f32, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, size[0], size[1]);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.binding, &[]);
        pass.draw(0..3, 0..1);
    }
}
