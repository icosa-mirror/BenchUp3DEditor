//! Phase 1B only: borrowed Vulkan images, wgpu rendering, and an OpenXR quad.
//! No desktop host, document, preferences, or controller actions are created.

use ash::vk::{self, Handle};
use openxr as xr;
use std::{
    error::Error,
    time::{Duration, Instant},
};
use wgpu::hal::{self, vulkan};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

// Field order releases the queue/device before the adapter/instance. XR objects
// are local to run() and are destroyed before this owner, including on errors.
struct Graphics {
    queue: wgpu::Queue,
    device: wgpu::Device,
    _adapter: wgpu::Adapter,
    _instance: wgpu::Instance,
    binding: xr::vulkan::SessionCreateInfo,
}

// xrEndFrame can enqueue compositor work on the bound queue after wgpu's
// submission fence. Drain that work before destroying XR images/session objects.
// Declare this guard after the XR resources so it also covers early returns.
struct DrainGpu<'a>(&'a Graphics);

impl Drop for DrainGpu<'_> {
    fn drop(&mut self) {
        // SAFETY: the device is alive; this host is the sole application queue
        // submitter, and the frame loop has stopped before guard destruction.
        unsafe {
            if let Some(device) = self.0.device.as_hal::<hal::api::Vulkan>() {
                if let Err(error) = device.raw_device().device_wait_idle() {
                    eprintln!("[BenchUpXR] GPU drain during teardown failed: {error}");
                }
            }
        }
    }
}

impl Graphics {
    fn new(xr: &xr::Instance, system: xr::SystemId) -> Result<Self> {
        let requirements = xr.graphics_requirements::<xr::Vulkan>(system)?;
        // Use wgpu 30's maximum supported API version for this first probe.
        let api = vk::API_VERSION_1_3;
        if requirements.min_api_version_supported > xr::Version::new(1, 3, 0)
            || requirements.max_api_version_supported.major() < 1
        {
            return Err(
                "Runtime Vulkan requirements are incompatible with this Vulkan 1.3 probe".into(),
            );
        }
        // SAFETY: the system Vulkan loader supplies ABI-compatible entry points;
        // OpenXR adds its required extensions to wgpu's requested extension set.
        unsafe {
            let entry = ash::Entry::load()?;
            if entry
                .try_enumerate_instance_version()?
                .unwrap_or(vk::API_VERSION_1_0)
                < api
            {
                return Err("Vulkan 1.3 loader required; update the GPU driver".into());
            }
            let flags = wgpu::InstanceFlags::DEBUG;
            let extensions = vulkan::Instance::desired_extensions(&entry, api, flags)?;
            let names: Vec<_> = extensions.iter().map(|name| name.as_ptr()).collect();
            let app = vk::ApplicationInfo::default()
                .api_version(api)
                .application_name(c"BenchUpXR");
            let validation = std::env::var_os("BENCHUP_XR_VALIDATION").is_some();
            let layers = if validation {
                vec![c"VK_LAYER_KHRONOS_validation".as_ptr()]
            } else {
                vec![]
            };
            if validation {
                let available = entry.enumerate_instance_layer_properties()?;
                if !available
                    .iter()
                    .any(|layer| layer.layer_name_as_c_str() == Ok(c"VK_LAYER_KHRONOS_validation"))
                {
                    return Err(
                        "Khronos validation requested but unavailable; check -ValidationDirectory"
                            .into(),
                    );
                }
                println!(
                    "[BenchUpXR] Enabling Khronos validation on the application Vulkan instance."
                );
            }
            let info = vk::InstanceCreateInfo::default()
                .application_info(&app)
                .enabled_layer_names(&layers)
                .enabled_extension_names(&names);
            let raw = xr
                .create_vulkan_instance(
                    system,
                    std::mem::transmute(entry.static_fn().get_instance_proc_addr),
                    (&info as *const vk::InstanceCreateInfo<'_>).cast(),
                )?
                .map_err(vk::Result::from_raw)?;
            let instance = ash::Instance::load(entry.static_fn(), vk::Instance::from_raw(raw as _));
            let hal_instance = match vulkan::Instance::from_raw(
                entry.clone(),
                instance.clone(),
                api,
                0,
                None,
                extensions,
                flags,
                Default::default(),
                false,
                None,
            ) {
                Ok(value) => value,
                Err(error) => {
                    instance.destroy_instance(None);
                    return Err(error.into());
                }
            };
            // HAL owns the instance from here onward. Do not destroy the ash clone.
            let physical =
                vk::PhysicalDevice::from_raw(xr.vulkan_graphics_device(system, raw)? as _);
            if instance
                .get_physical_device_properties(physical)
                .api_version
                < api
            {
                return Err("Runtime-selected GPU requires a Vulkan 1.3 capable driver".into());
            }
            let exposed = hal_instance
                .expose_adapter(physical)
                .ok_or("Runtime GPU is unsupported by wgpu")?;
            println!("[BenchUpXR] Runtime-selected GPU: {}", exposed.info.name);
            let family = instance
                .get_physical_device_queue_family_properties(physical)
                .iter()
                .position(|q| q.queue_count > 0 && q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .ok_or("Runtime GPU has no graphics queue")? as u32;
            let descriptor = wgpu::DeviceDescriptor::default();
            let extensions = exposed
                .adapter
                .required_device_extensions(descriptor.required_features);
            let names: Vec<_> = extensions.iter().map(|name| name.as_ptr()).collect();
            let mut features = exposed
                .adapter
                .physical_device_features(&extensions, descriptor.required_features);
            let queues = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(family)
                .queue_priorities(&[1.0])];
            let info = features.add_to_device_create(
                vk::DeviceCreateInfo::default()
                    .queue_create_infos(&queues)
                    .enabled_extension_names(&names),
            );
            let raw_device = xr
                .create_vulkan_device(
                    system,
                    std::mem::transmute(entry.static_fn().get_instance_proc_addr),
                    physical.as_raw() as _,
                    (&info as *const vk::DeviceCreateInfo<'_>).cast(),
                )?
                .map_err(vk::Result::from_raw)?;
            let device =
                ash::Device::load(instance.fp_v1_0(), vk::Device::from_raw(raw_device as _));
            let opened = match exposed.adapter.device_from_raw(
                device.clone(),
                None,
                &extensions,
                descriptor.required_features,
                &descriptor.required_limits,
                &descriptor.memory_hints,
                family,
                0,
            ) {
                Ok(value) => value,
                Err(error) => {
                    device.destroy_device(None);
                    return Err(error.into());
                }
            };
            // HAL owns the device. The high-level handles retain their parents.
            let instance = wgpu::Instance::from_hal::<hal::api::Vulkan>(hal_instance);
            let adapter = instance.create_adapter_from_hal(exposed);
            let (device, queue) = adapter.create_device_from_hal(opened, &descriptor)?;
            println!(
                "[BenchUpXR] Application Vulkan instance=0x{:x}, device=0x{:x}, queue family={family}",
                raw as usize, raw_device as usize
            );
            Ok(Self {
                queue,
                device,
                _adapter: adapter,
                _instance: instance,
                binding: xr::vulkan::SessionCreateInfo {
                    instance: raw,
                    physical_device: physical.as_raw() as _,
                    device: raw_device,
                    queue_family_index: family,
                    queue_index: 0,
                },
            })
        }
    }

    fn paint(
        &self,
        image: xr::sys::platform::VkImage,
        format: wgpu::TextureFormat,
        size: wgpu::Extent3d,
        pipeline: &wgpu::RenderPipeline,
        editor: Option<&mut super::editor::Editor>,
    ) -> Result<()> {
        let descriptor = wgpu::TextureDescriptor {
            label: Some("BenchUpXR borrowed image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };
        // SAFETY: run() acquired and waited for this image. It matches the
        // negotiated format/extent and same Vulkan device. External memory plus
        // a no-op drop callback prevents wgpu from destroying runtime resources.
        // XR hands us COLOR_ATTACHMENT_OPTIMAL; pass the matching initial usage
        // to wgpu. The sole render pass leaves that same layout, as XR requires.
        // GPU completion and wrapper destruction precede xrReleaseSwapchainImage.
        let texture = unsafe {
            let device = self
                .device
                .as_hal::<hal::api::Vulkan>()
                .ok_or("Missing Vulkan device")?;
            let texture = device.texture_from_raw(
                vk::Image::from_raw(image as _),
                &hal::TextureDescriptor {
                    label: descriptor.label,
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: descriptor.dimension,
                    format,
                    usage: wgpu::TextureUses::COLOR_TARGET,
                    memory_flags: hal::MemoryFlags::empty(),
                    view_formats: vec![],
                },
                Some(Box::new(|| {})),
                vulkan::TextureMemory::External,
            );
            self.device.create_texture_from_hal::<hal::api::Vulkan>(
                texture,
                &descriptor,
                wgpu::TextureUses::COLOR_TARGET,
            )
        };
        let view = texture.create_view(&Default::default());
        let submission = if let Some(editor) = editor {
            editor.paint(&self.device, &self.queue, &view)?
        } else {
            let mut encoder = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("BenchUpXR orientation quad"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.02,
                                g: 0.55,
                                b: 0.8,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(pipeline);
                pass.draw(0..3, 0..1);
            }
            self.queue.submit([encoder.finish()])
        };
        // Conservative single-frame pacing for the interoperability gate, not a
        // throughput policy. No CPU readback or separate graphics queue is used.
        self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })?;
        // Drain deferred image-view destruction while the XR image is alive,
        // including on the final frame before swapchain teardown.
        drop(view);
        drop(texture);
        self.device.poll(wgpu::PollType::Poll)?;
        Ok(())
    }
}

/// Display an orientation panel for `duration`, then request an orderly session stop.
/// The timeout includes waiting for the headset/session to become ready.
pub fn run(duration: Duration) -> Result<()> {
    run_content(duration, false)
}

/// Render-only phase 1C preview using the shared editor and preference storage.
/// Controller input, native window services and the desktop mirror are pending.
pub fn run_editor(duration: Duration) -> Result<()> {
    run_content(duration, true)
}

/// Diagnose the shared editor renderer on the runtime-selected GPU without a
/// session. Readback is only for this PNG artifact, never the headset path.
pub fn snapshot_editor(path: &std::path::Path) -> Result<()> {
    // SAFETY: same explicit loader and enable2 initialization as the quad probe.
    let entry = unsafe { xr::Entry::load(&()) }?;
    let mut extensions = xr::ExtensionSet::default();
    extensions.khr_vulkan_enable2 = true;
    let instance = entry.create_instance(
        &xr::ApplicationInfo {
            application_name: "BenchUpXR editor GPU snapshot",
            ..Default::default()
        },
        &extensions,
        &[],
        &(),
    )?;
    let system = instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;
    let graphics = Graphics::new(&instance, system)?;
    snapshot_editor_on_device(&graphics.device, &graphics.queue, path)
}

/// Independent renderer check when no headset/runtime is available. This does
/// not establish runtime adapter selection or XR interoperability.
pub async fn snapshot_editor_without_runtime(path: &std::path::Path) -> Result<()> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        })
        .await?;
    let info = adapter.get_info();
    if info.device_type == wgpu::DeviceType::Cpu {
        return Err("The editor snapshot requires a hardware Vulkan GPU".into());
    }
    println!(
        "[BenchUpXR] Independent renderer check: {} ({:?}); no OpenXR session or adapter selection",
        info.name, info.backend
    );
    let (device, queue) = adapter.request_device(&Default::default()).await?;
    snapshot_editor_on_device(&device, &queue, path)
}

fn snapshot_editor_on_device(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &std::path::Path,
) -> Result<()> {
    let size = wgpu::Extent3d {
        width: 1440,
        height: 900,
        depth_or_array_layers: 1,
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("BenchUpXR diagnostic snapshot"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut editor = super::editor::Editor::new(device, queue, format, size)?;
    // A second frame lets egui settle its first-frame layout and font uploads.
    for _ in 0..2 {
        editor.paint(device, queue, &view)?;
    }
    let stride = (size.width * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("BenchUpXR diagnostic PNG readback"),
        size: u64::from(stride) * u64::from(size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
    device.poll(wgpu::PollType::wait_indefinitely())?;
    receive.recv()??;
    let pixels = buffer.slice(..).get_mapped_range()?;
    let mut rgba = Vec::with_capacity((size.width * size.height * 4) as usize);
    for row in pixels.chunks(stride as usize) {
        rgba.extend_from_slice(&row[..size.width as usize * 4]);
    }
    image::save_buffer(
        path,
        &rgba,
        size.width,
        size.height,
        image::ColorType::Rgba8,
    )?;
    drop(pixels);
    buffer.unmap();
    println!(
        "[BenchUpXR] Shared editor GPU snapshot saved: {}",
        path.display()
    );
    Ok(())
}

fn run_content(duration: Duration, show_editor: bool) -> Result<()> {
    println!(
        "[BenchUpXR] Vulkan/wgpu quad probe; duration {}s",
        duration.as_secs()
    );
    // SAFETY: explicit developer entry point loading the installed OpenXR loader.
    let entry = unsafe { xr::Entry::load(&()) }?;
    let available = entry.enumerate_extensions()?;
    if !available.khr_vulkan_enable2 {
        return Err("XR_KHR_vulkan_enable2 unavailable".into());
    }
    let mut extensions = xr::ExtensionSet::default();
    extensions.khr_vulkan_enable2 = true;
    let instance = entry.create_instance(
        &xr::ApplicationInfo {
            application_name: "BenchUpXR quad probe",
            ..Default::default()
        },
        &extensions,
        &[],
        &(),
    )?;
    let system = instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;
    let graphics = Graphics::new(&instance, system)?;
    println!("[BenchUpXR] Creating OpenXR session on the application Vulkan device.");
    // SAFETY: binding identifies the live device/queue created for this XR system.
    // graphics outlives all session, space and swapchain handles below.
    let (session, mut waiter, mut stream) =
        unsafe { instance.create_session::<xr::Vulkan>(system, &graphics.binding) }?;
    let formats = session.enumerate_swapchain_formats()?;
    let (vk_format, format) = [
        (
            vk::Format::R8G8B8A8_SRGB,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ),
        (
            vk::Format::B8G8R8A8_SRGB,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ),
    ]
    .into_iter()
    .find(|(vk, _)| formats.contains(&(vk.as_raw() as _)))
    .ok_or("Runtime offers no supported sRGB panel format")?;
    let size = wgpu::Extent3d {
        width: 1440,
        height: 900,
        depth_or_array_layers: 1,
    };
    let mut swapchain = session.create_swapchain(&xr::SwapchainCreateInfo {
        create_flags: xr::SwapchainCreateFlags::EMPTY,
        usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT,
        format: vk_format.as_raw() as _,
        sample_count: 1,
        width: size.width,
        height: size.height,
        face_count: 1,
        array_size: 1,
        mip_count: 1,
    })?;
    let images = swapchain.enumerate_images()?;
    println!("[BenchUpXR] Creating wgpu orientation pipeline.");
    let shader = graphics
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BenchUpXR orientation pattern"),
            source: wgpu::ShaderSource::Wgsl(include_str!("panel.wgsl").into()),
        });
    let pipeline = graphics
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BenchUpXR orientation pattern"),
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
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
    let space =
        session.create_reference_space(xr::ReferenceSpaceType::LOCAL, xr::Posef::IDENTITY)?;
    let mut editor = if show_editor {
        println!("[BenchUpXR] Installing the shared editor and GPU viewport callbacks.");
        Some(super::editor::Editor::new(
            &graphics.device,
            &graphics.queue,
            format,
            size,
        )?)
    } else {
        None
    };
    let blend = instance
        .enumerate_environment_blend_modes(system, xr::ViewConfigurationType::PRIMARY_STEREO)?
        .into_iter()
        .find(|mode| *mode == xr::EnvironmentBlendMode::OPAQUE)
        .ok_or("Opaque environment blend mode unavailable")?;
    println!(
        "[BenchUpXR] Swapchain: {} images, {vk_format:?}, {}x{}",
        images.len(),
        size.width,
        size.height
    );
    let _drain_gpu = DrainGpu(&graphics);
    let start = Instant::now();
    let mut events = xr::EventDataBuffer::new();
    let mut running = false;
    let mut stopping = false;
    let mut submitted = 0u64;
    loop {
        while let Some(event) = instance.poll_event(&mut events)? {
            match event {
                xr::Event::SessionStateChanged(event) => {
                    println!("[BenchUpXR] Session: {:?}", event.state());
                    match event.state() {
                        xr::SessionState::READY => {
                            session.begin(xr::ViewConfigurationType::PRIMARY_STEREO)?;
                            running = true;
                        }
                        xr::SessionState::STOPPING => {
                            session.end()?;
                            running = false;
                            if stopping {
                                break;
                            }
                        }
                        xr::SessionState::EXITING => {
                            if submitted == 0 {
                                return Err("Session exited without rendering a quad".into());
                            }
                            println!(
                                "[BenchUpXR] Runtime exit; {submitted} quad frames submitted."
                            );
                            return Ok(());
                        }
                        xr::SessionState::LOSS_PENDING => {
                            return Err(
                                "OpenXR session lost; rerun the probe after reconnecting".into()
                            );
                        }
                        _ => {}
                    }
                }
                xr::Event::InstanceLossPending(_) => return Err("OpenXR instance lost".into()),
                _ => {}
            }
        }
        if start.elapsed() >= duration && !stopping {
            if running {
                session.request_exit()?;
                stopping = true;
            } else {
                return Err("Headset did not start a renderable session before timeout".into());
            }
        }
        if stopping && !running {
            break;
        }
        if stopping && start.elapsed() > duration + Duration::from_secs(5) {
            return Err("Runtime did not stop the session within five seconds".into());
        }
        if !running {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let frame = waiter.wait()?;
        stream.begin()?;
        if !frame.should_render {
            stream.end(frame.predicted_display_time, blend, &[])?;
            continue;
        }
        let index = swapchain.acquire_image()? as usize;
        swapchain.wait_image(xr::Duration::INFINITE)?;
        // On graphics failure, never hand a possibly in-flight image back to the
        // compositor: unwind session resources before destroying the GPU owner.
        graphics.paint(images[index], format, size, &pipeline, editor.as_mut())?;
        swapchain.release_image()?;
        let quad = xr::CompositionLayerQuad::new()
            .space(&space)
            .eye_visibility(xr::EyeVisibility::BOTH)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&swapchain)
                    .image_array_index(0)
                    .image_rect(xr::Rect2Di {
                        offset: xr::Offset2Di { x: 0, y: 0 },
                        extent: xr::Extent2Di {
                            width: size.width as _,
                            height: size.height as _,
                        },
                    }),
            )
            .pose(xr::Posef {
                orientation: xr::Quaternionf::IDENTITY,
                position: xr::Vector3f {
                    x: 0.0,
                    y: 0.0,
                    z: -1.5,
                },
            })
            .size(xr::Extent2Df {
                width: 1.2,
                height: 0.75,
            });
        stream.end(frame.predicted_display_time, blend, &[&quad])?;
        submitted += 1;
        if submitted == 1 {
            if show_editor {
                println!(
                    "[BenchUpXR] First complete editor frame submitted to the quad (render-only preview)."
                );
            } else {
                println!(
                    "[BenchUpXR] First wgpu quad submitted; red top-left, green top-right, blue bottom-left, white bottom-right; 1.5m ahead of the initial local origin."
                );
            }
        }
    }
    println!("[BenchUpXR] Clean stop; {submitted} quad frames submitted.");
    if submitted == 0 {
        return Err("Session ended without rendering a quad".into());
    }
    Ok(())
}
