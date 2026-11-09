//! A small retained GPU overlay; only its position uniform changes each frame.
use super::input::FrameInput;

pub(super) struct Cursor {
    pipeline: wgpu::RenderPipeline,
    binding: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    visible: bool,
    size: [u32; 2],
}

impl Cursor {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BenchUpXR cursor uniform"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BenchUpXR cursor"),
            source: wgpu::ShaderSource::Wgsl(include_str!("cursor.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BenchUpXR cursor"),
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
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BenchUpXR cursor binding"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            pipeline,
            binding,
            uniform,
            visible: false,
            size: [1, 1],
        }
    }

    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        input: &FrameInput,
        size: [u32; 2],
        pixels_per_point: f32,
    ) {
        self.visible = false;
        self.size = size;
        if let Some(position) = input.cursor {
            let position = egui_position_pixels(position, pixels_per_point);
            if position.x >= 0.0
                && position.y >= 0.0
                && position.x <= size[0] as f32
                && position.y <= size[1] as f32
            {
                let data = [
                    position.x,
                    position.y,
                    size[0] as f32,
                    size[1] as f32,
                    if input.pressed { 1.0 } else { 0.0 },
                    0.0,
                    0.0,
                    0.0,
                ];
                queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&data));
                self.visible = true;
            }
        }
    }

    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.visible {
            pass.set_viewport(0.0, 0.0, self.size[0] as f32, self.size[1] as f32, 0.0, 1.0);
            pass.set_scissor_rect(0, 0, self.size[0], self.size[1]);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.binding, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

fn egui_position_pixels(position: eframe::egui::Pos2, pixels_per_point: f32) -> eframe::egui::Pos2 {
    eframe::egui::pos2(position.x * pixels_per_point, position.y * pixels_per_point)
}
