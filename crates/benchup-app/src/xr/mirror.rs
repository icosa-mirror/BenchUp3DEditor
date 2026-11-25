//! GPU-only desktop mirror. XR-owned images remain render attachments only.
use eframe::wgpu;
use glam::{DMat4, DVec3};
use openxr as xr;
use std::{error::Error, sync::Arc};
use wgpu::util::DeviceExt as _;
use winit::window::Window;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Binding {
    group: wgpu::BindGroup,
    transform: wgpu::Buffer,
}

struct Painter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Painter {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BenchUpXR mirror"),
            source: wgpu::ShaderSource::Wgsl(include_str!("mirror.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BenchUpXR mirror image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BenchUpXR mirror"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BenchUpXR mirror"),
            layout: Some(&pipeline_layout),
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
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("BenchUpXR mirror sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            sampler,
        }
    }

    fn bind(&self, device: &wgpu::Device, image: &wgpu::TextureView) -> Binding {
        let transform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BenchUpXR mirror transform"),
            contents: bytemuck::cast_slice(&glam::Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BenchUpXR mirror image"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(image),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: transform.as_entire_binding(),
                },
            ],
        });
        Binding { group, transform }
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, binding: &Binding) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &binding.group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Retained application-owned source. Never sample or copy from an XR image.
pub(super) struct Capture {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    size: [u32; 2],
    painter: Painter,
    binding: Binding,
}

impl Capture {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, size: [u32; 2]) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BenchUpXR mirror capture"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let painter = Painter::new(device, format);
        let binding = painter.bind(device, &view);
        Self {
            texture,
            view,
            size,
            painter,
            binding,
        }
    }

    pub fn copy_to(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
    ) -> wgpu::SubmissionIndex {
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = render_pass(&mut encoder, target);
            self.painter.draw(&mut pass, &self.binding);
        }
        queue.submit([encoder.finish()])
    }
}

pub(super) struct Mirror {
    // The surface retains the window; drop it before the explicit window handle.
    surface: wgpu::Surface<'static>,
    window: Arc<Window>,
    instance: wgpu::Instance,
    output: super::desktop::NativeOutput,
    config: wgpu::SurfaceConfiguration,
    configured: bool,
    painter: Painter,
    pub panel: Capture,
    pub eye: Option<Capture>,
    panel_binding: Binding,
    eye_binding: Option<Binding>,
}

impl Mirror {
    pub fn new(
        instance: &wgpu::Instance,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        window: Arc<Window>,
        output: super::desktop::NativeOutput,
        format: wgpu::TextureFormat,
        panel_size: [u32; 2],
        eye_size: Option<[u32; 2]>,
    ) -> Result<Self> {
        let surface = output.create_surface(instance)?;
        let caps = surface.get_capabilities(adapter);
        let mut config = surface
            .get_default_config(adapter, 1, 1)
            .ok_or("The runtime-selected GPU cannot present to this window")?;
        config.format = caps
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::AutoNoVsync;
        config.desired_maximum_frame_latency = 1;
        let painter = Painter::new(device, config.format);
        let panel = Capture::new(device, format, panel_size);
        let eye = eye_size.map(|size| Capture::new(device, format, size));
        let panel_binding = painter.bind(device, &panel.view);
        let eye_binding = eye.as_ref().map(|image| painter.bind(device, &image.view));
        println!(
            "[BenchUpXRMirror] GPU mirror ready: {:?}; left eye plus editor quad",
            config.format
        );
        Ok(Self {
            surface,
            window,
            instance: instance.clone(),
            output,
            config,
            configured: false,
            painter,
            panel,
            eye,
            panel_binding,
            eye_binding,
        })
    }

    fn render_to(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        size: [u32; 2],
        scene: Option<(xr::View, xr::Posef, xr::Extent2Df)>,
    ) -> wgpu::SubmissionIndex {
        let source = if scene.is_some() {
            self.eye.as_ref().unwrap_or(&self.panel)
        } else {
            &self.panel
        };
        if let Some((eye, panel, extent)) = scene {
            let matrix = super::immersive::projection(eye.fov, 0.03, 200.)
                * super::immersive::pose_matrix(eye.pose).inverse()
                * super::immersive::pose_matrix(panel)
                * DMat4::from_scale(DVec3::new(
                    extent.width as f64 * 0.5,
                    extent.height as f64 * 0.5,
                    1.,
                ));
            queue.write_buffer(
                &self.panel_binding.transform,
                0,
                bytemuck::cast_slice(&matrix.as_mat4().to_cols_array()),
            );
        } else {
            queue.write_buffer(
                &self.panel_binding.transform,
                0,
                bytemuck::cast_slice(&glam::Mat4::IDENTITY.to_cols_array()),
            );
        }
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = render_pass(&mut encoder, target);
            let [x, y, width, height] = letterbox(source.size, size);
            pass.set_viewport(x, y, width, height, 0., 1.);
            if scene.is_some()
                && let Some(binding) = &self.eye_binding
            {
                self.painter.draw(&mut pass, binding);
            }
            // The OpenXR quad is submitted after the projection layer, without
            // scene-depth occlusion. Match that ordering on the desktop.
            self.painter.draw(&mut pass, &self.panel_binding);
        }
        queue.submit([encoder.finish()])
    }

    pub fn visible(&self) -> bool {
        let size = self.window.inner_size();
        size.width > 0 && size.height > 0 && self.window.is_minimized() != Some(true)
    }

    pub fn present(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: Option<(xr::View, xr::Posef, xr::Extent2Df)>,
    ) -> Result<bool> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 || self.window.is_minimized() == Some(true) {
            return Ok(false);
        }
        if !self.configured || self.config.width != size.width || self.config.height != size.height
        {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(device, &self.config);
            self.configured = true;
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.configured = false;
                frame
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.configured = false;
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.output.create_surface(&self.instance)?;
                self.configured = false;
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Desktop surface validation failed".into());
            }
        };
        let target = frame.texture.create_view(&Default::default());
        self.render_to(device, queue, &target, [size.width, size.height], scene);
        self.window.pre_present_notify();
        queue.present(frame);
        Ok(true)
    }
}

fn render_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("BenchUpXR mirror image"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    })
}

fn letterbox(source: [u32; 2], target: [u32; 2]) -> [f32; 4] {
    let scale = (target[0] as f32 / source[0] as f32).min(target[1] as f32 / source[1] as f32);
    let width = source[0] as f32 * scale;
    let height = source[1] as f32 * scale;
    [
        (target[0] as f32 - width) * 0.5,
        (target[1] as f32 - height) * 0.5,
        width,
        height,
    ]
}

/// Native presentation, resize and GPU-composition diagnostic. No XR runtime.
pub(super) fn smoke(
    window: Arc<Window>,
    output: super::desktop::NativeOutput,
    close: &std::sync::atomic::AtomicBool,
    path: &std::path::Path,
) -> Result<()> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    println!(
        "[BenchUpXRMirror] Diagnostic GPU: {}",
        adapter.get_info().name
    );
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    let mut mirror = Mirror::new(
        &instance,
        &adapter,
        &device,
        window.clone(),
        output,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        [2, 2],
        Some([2, 2]),
    )?;
    for (image, pixels) in [
        (
            &mirror.panel,
            [
                255_u8, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
            ],
        ),
        (
            mirror.eye.as_ref().unwrap(),
            [
                0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 128, 255, 0, 0, 128, 255,
            ],
        ),
    ] {
        queue.write_texture(
            image.texture.as_image_copy(),
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
        );
    }
    let eye = xr::View {
        pose: xr::Posef::IDENTITY,
        fov: xr::Fovf {
            angle_left: -0.7853982,
            angle_right: 0.7853982,
            angle_up: 0.7853982,
            angle_down: -0.7853982,
        },
    };
    let panel = xr::Posef {
        orientation: xr::Quaternionf::IDENTITY,
        position: xr::Vector3f {
            x: 0.,
            y: 0.,
            z: -2.,
        },
    };
    let extent = xr::Extent2Df {
        width: 1.,
        height: 0.65,
    };
    let scene = Some((eye, panel, extent));
    let mut presented = 0;
    let mut sizes = std::collections::BTreeSet::new();
    for frame in 0..18 {
        if close.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Mirror diagnostic interrupted".into());
        }
        if frame == 4 {
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(800, 450));
        }
        if frame == 11 {
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(480, 480));
        }
        if mirror.present(&device, &queue, if frame % 2 == 0 { scene } else { None })? {
            presented += 1;
            sizes.insert((mirror.config.width, mirror.config.height));
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    if presented == 0 || sizes.len() < 2 {
        return Err(format!(
            "Mirror presentation/resize incomplete: {presented} frames, {sizes:?}"
        )
        .into());
    }
    let size = wgpu::Extent3d {
        width: 640,
        height: 360,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("BenchUpXR mirror diagnostic readback"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: mirror.config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    mirror.render_to(
        &device,
        &queue,
        &target.create_view(&Default::default()),
        [640, 360],
        scene,
    );
    super::panel::save_texture(&device, &queue, &target, size, path)?;
    // Exercise the exact capture-to-render-attachment pass used for XR images,
    // with an owned attachment so this check needs no runtime or headset.
    let copy_size = wgpu::Extent3d {
        width: 2,
        height: 2,
        depth_or_array_layers: 1,
    };
    let copy = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("BenchUpXR mirror copy check"),
        size: copy_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    mirror
        .panel
        .copy_to(&device, &queue, &copy.create_view(&Default::default()));
    let copy_path = path.with_extension("copy.png");
    super::panel::save_texture(&device, &queue, &copy, copy_size, &copy_path)?;
    let pixels = image::open(&copy_path)?.to_rgba8();
    if pixels.get_pixel(0, 0).0 != [255, 0, 0, 255] || pixels.get_pixel(1, 1).0 != [0, 255, 0, 255]
    {
        return Err("Mirror copy changed image orientation or color channels".into());
    }
    println!("[BenchUpXRMirror] Smoke passed: {presented} frames; sizes {sizes:?}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mirror_preserves_source_aspect_when_resized() {
        assert_eq!(letterbox([100, 100], [640, 360]), [140., 0., 360., 360.]);
        assert_eq!(letterbox([1440, 900], [640, 360]), [32., 0., 576., 360.]);
        assert_eq!(letterbox([640, 360], [360, 640]), [0., 218.75, 360., 202.5]);
    }
}
