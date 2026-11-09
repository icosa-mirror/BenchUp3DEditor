//! Distant sky, ground and sun. Reconstruct world directions on the GPU.
use eframe::wgpu;
use glam::{DMat4, DVec3};

pub(super) struct Backdrop {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
}
impl Backdrop {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BenchUpXR environment"),
            source: wgpu::ShaderSource::Wgsl(include_str!("backdrop.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BenchUpXR world environment"),
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
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BenchUpXR environment view"),
            size: 176,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BenchUpXR environment view"),
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
        display: &benchup_core::DisplaySettings,
        dark: bool,
        sun: bool,
    ) {
        let mut data = [0.0f32; 44];
        data[..16].copy_from_slice(&matrix.inverse().as_mat4().to_cols_array());
        data[16..20].copy_from_slice(&eye.extend(0.0).as_vec4().to_array());
        for (offset, rgb) in [
            (20, display.background),
            (24, display.sky_color),
            (28, display.ground_color),
            (32, display.horizon_color),
        ] {
            for i in 0..3 {
                data[offset + i] = rgb[i] as f32 / 255.0;
            }
        }
        let angle = display.sun_angle.to_radians();
        let elevation = display.sun_elevation.to_radians();
        data[36..40].copy_from_slice(&[
            (angle.cos() * elevation.cos()) as f32,
            (angle.sin() * elevation.cos()) as f32,
            elevation.sin() as f32,
            0.0,
        ]);
        data[40..44].copy_from_slice(&[
            u32::from(display.sky) as f32,
            u32::from(display.ground) as f32,
            u32::from(dark) as f32,
            u32::from(sun) as f32,
        ]);
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
