//! Generic grip-space controllers: one retained mesh, two small GPU instances.
use super::navigation::{Grip, Input};
use eframe::wgpu;
use glam::{DMat4, Vec3};
use wgpu::util::DeviceExt as _;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    tracking_from_grip: [[f32; 4]; 4],
    tint: [f32; 4],
}

pub(super) struct Controllers {
    pipeline: wgpu::RenderPipeline,
    binding: wgpu::BindGroup,
    view: wgpu::Buffer,
    vertices: wgpu::Buffer,
    instances: wgpu::Buffer,
    vertex_count: u32,
    poses: [Option<Grip>; 2],
    squeezed: [bool; 2],
    count: u32,
}

impl Controllers {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BenchUpXR generic controllers"),
            source: wgpu::ShaderSource::Wgsl(include_str!("controller_visuals.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BenchUpXR controller view"),
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
            label: Some("BenchUpXR controller layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BenchUpXR grip-space controllers"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader, entry_point: Some("vertex"),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as _, step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x4],
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Instance>() as _, step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![3=>Float32x4,4=>Float32x4,5=>Float32x4,6=>Float32x4,7=>Float32x4],
                    }),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader, entry_point: Some("fragment"),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true), depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(), bias: Default::default(),
            }),
            multisample: Default::default(), multiview_mask: None, cache: None,
        });
        let view = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BenchUpXR controller view"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BenchUpXR controller view"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view.as_entire_binding(),
            }],
        });
        let mesh = mesh();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BenchUpXR retained controller mesh"),
            contents: bytemuck::cast_slice(&mesh),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BenchUpXR tracked controller instances"),
            size: (2 * std::mem::size_of::<Instance>()) as _,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            binding,
            view,
            vertices,
            instances,
            vertex_count: mesh.len() as _,
            poses: [None; 2],
            squeezed: [false; 2],
            count: 0,
        }
    }

    pub fn set_tracking(&mut self, input: Input) {
        // Navigation transforms the document, never the tracked hands.
        self.poses = input.hands;
        self.squeezed = input.buttons.map(|down| down == Some(true));
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue, clip_from_tracking: DMat4) {
        let instances = instances(self.poses, self.squeezed);
        self.count = instances.len() as _;
        if self.count == 0 {
            return;
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
        queue.write_buffer(
            &self.view,
            0,
            bytemuck::cast_slice(&clip_from_tracking.as_mat4().to_cols_array()),
        );
    }

    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.binding, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.draw(0..self.vertex_count, 0..self.count);
    }
}

fn instances(poses: [Option<Grip>; 2], squeezed: [bool; 2]) -> Vec<Instance> {
    poses
        .into_iter()
        .enumerate()
        .filter_map(|(hand, pose)| {
            let pose = pose?;
            if !pose.position.is_finite()
                || !pose.orientation.is_finite()
                || pose.orientation.length_squared() < 1e-12
            {
                return None;
            }
            let tint = if squeezed[hand] {
                [0.35, 1., 0.45, 1.]
            } else if hand == 0 {
                [0.15, 0.6, 1., 1.]
            } else {
                [1., 0.55, 0.15, 1.]
            };
            Some(Instance {
                tracking_from_grip: DMat4::from_rotation_translation(
                    pose.orientation.normalize(),
                    pose.position,
                )
                .as_mat4()
                .to_cols_array_2d(),
                tint,
            })
        })
        .collect()
}

fn mesh() -> Vec<Vertex> {
    let mut vertices = Vec::new();
    // Grip origin sits in the palm. -Z is forward, +Y is up. A short grip,
    // broad top and raised trigger give visible orientation without device assets.
    box_mesh(
        &mut vertices,
        Vec3::new(0., -0.02, 0.),
        Vec3::new(0.036, 0.10, 0.038),
        [0.33, 0.36, 0.40, 0.],
    );
    box_mesh(
        &mut vertices,
        Vec3::new(0., 0.035, -0.012),
        Vec3::new(0.060, 0.022, 0.075),
        [0., 0., 0., 1.],
    );
    box_mesh(
        &mut vertices,
        Vec3::new(0., 0.008, -0.026),
        Vec3::new(0.024, 0.024, 0.014),
        [0.8, 0.82, 0.86, 0.],
    );
    box_mesh(
        &mut vertices,
        Vec3::new(-0.014, 0.050, -0.018),
        Vec3::new(0.013, 0.012, 0.013),
        [0.12, 0.14, 0.17, 0.],
    );
    box_mesh(
        &mut vertices,
        Vec3::new(0.015, 0.048, -0.006),
        Vec3::new(0.010, 0.006, 0.010),
        [0.85, 0.86, 0.89, 0.],
    );
    vertices
}

fn box_mesh(vertices: &mut Vec<Vertex>, center: Vec3, size: Vec3, color: [f32; 4]) {
    for (normal, u, v) in [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (-Vec3::X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (-Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::Y, Vec3::X),
    ] {
        let origin = center + normal * size * 0.5;
        let a = u * size * 0.5;
        let b = v * size * 0.5;
        let corners = [
            origin - a - b,
            origin + a - b,
            origin + a + b,
            origin - a + b,
        ];
        for index in [0, 1, 2, 0, 2, 3] {
            vertices.push(Vertex {
                position: corners[index].to_array(),
                normal: normal.to_array(),
                color,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DQuat, DVec3};

    #[test]
    fn instances_follow_grips_and_drop_lost_tracking() {
        let grip = Grip {
            position: DVec3::new(1., 2., 3.),
            orientation: DQuat::from_rotation_x(0.7),
        };
        let data = instances([None, Some(grip)], [false; 2]);
        assert_eq!(data.len(), 1);
        let transform = glam::Mat4::from_cols_array_2d(&data[0].tracking_from_grip);
        assert!(
            transform
                .transform_point3(Vec3::ZERO)
                .distance(grip.position.as_vec3())
                < 1e-6
        );
        assert!(
            transform
                .transform_vector3(-Vec3::Z)
                .distance((grip.orientation * -DVec3::Z).as_vec3())
                < 1e-6
        );
        assert!(instances([None; 2], [false; 2]).is_empty());
        assert_ne!(instances([Some(grip); 2], [false; 2])[0].tint, data[0].tint);
        assert_eq!(
            instances([Some(grip); 2], [true; 2])[0].tint,
            [0.35, 1., 0.45, 1.]
        );
    }

    #[test]
    fn retained_mesh_is_outward_facing_and_hand_sized() {
        let mesh = mesh();
        assert_eq!(mesh.len(), 180);
        for triangle in mesh.chunks_exact(3) {
            let [a, b, c] = std::array::from_fn(|i| Vec3::from_array(triangle[i].position));
            assert!(
                (b - a)
                    .cross(c - a)
                    .dot(Vec3::from_array(triangle[0].normal))
                    > 0.
            );
            assert!(
                triangle
                    .iter()
                    .all(|vertex| Vec3::from_array(vertex.position).length() < 0.1)
            );
        }
    }
}
