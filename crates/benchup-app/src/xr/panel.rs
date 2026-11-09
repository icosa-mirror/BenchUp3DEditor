//! Borrowed Vulkan images and an OpenXR quad, with a pattern probe or shared
//! editor host. The editor path adds preferences and controller input; native
//! window services and desktop mirroring are still pending.

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

struct Stereo {
    chains: [xr::Swapchain<xr::Vulkan>; 2],
    images: [Vec<xr::sys::platform::VkImage>; 2],
    size: wgpu::Extent3d,
}

impl Stereo {
    fn new(
        instance: &xr::Instance,
        session: &xr::Session<xr::Vulkan>,
        system: xr::SystemId,
        format: vk::Format,
    ) -> Result<Self> {
        let views = instance.enumerate_view_configuration_views(
            system,
            xr::ViewConfigurationType::PRIMARY_STEREO,
        )?;
        if views.len() != 2 {
            return Err("Stereo rendering requires exactly two OpenXR views".into());
        }
        let size = wgpu::Extent3d {
            width: views
                .iter()
                .map(|view| view.recommended_image_rect_width)
                .max()
                .unwrap(),
            height: views
                .iter()
                .map(|view| view.recommended_image_rect_height)
                .max()
                .unwrap(),
            depth_or_array_layers: 1,
        };
        let create = || {
            session.create_swapchain(&xr::SwapchainCreateInfo {
                create_flags: xr::SwapchainCreateFlags::EMPTY,
                usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT,
                format: format.as_raw() as _,
                sample_count: 1,
                width: size.width,
                height: size.height,
                face_count: 1,
                array_size: 1,
                mip_count: 1,
            })
        };
        let chains = [create()?, create()?];
        let images = [chains[0].enumerate_images()?, chains[1].enumerate_images()?];
        println!(
            "[BenchUpXR] Stereo eye targets: {} x {} per eye",
            size.width, size.height
        );
        Ok(Self {
            chains,
            images,
            size,
        })
    }
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
        eye: Option<xr::View>,
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
            if let Some(eye) = eye {
                editor.paint_eye(
                    &self.device,
                    &self.queue,
                    &view,
                    [size.width, size.height],
                    eye,
                )
            } else {
                editor.paint(&self.device, &self.queue, &view)?
            }
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

/// Shared-editor panel preview with controller input and shared preferences.
/// Native window services and the desktop mirror are pending.
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
    snapshot_editor_on_device(&graphics.device, &graphics.queue, path, false)
}

/// Independent renderer check when no headset/runtime is available. This does
/// not establish runtime adapter selection or XR interoperability.
pub async fn snapshot_editor_without_runtime(path: &std::path::Path) -> Result<()> {
    snapshot_without_runtime(path,false).await
}

/// Exercise the actual eye render path on the hardware GPU without a headset.
pub async fn snapshot_immersive_without_runtime(path:&std::path::Path)->Result<()> {
    snapshot_without_runtime(path,true).await
}

async fn snapshot_without_runtime(path: &std::path::Path, immersive:bool) -> Result<()> {
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
    snapshot_editor_on_device(&device, &queue, path, immersive)
}

fn snapshot_editor_on_device(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &std::path::Path,
    immersive:bool,
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
    if immersive {editor.paint_immersive_snapshot(device,queue,&view,[size.width,size.height])?;}
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
        "[BenchUpXR] GPU snapshot saved: {}",
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
    let view_space =
        session.create_reference_space(xr::ReferenceSpaceType::VIEW, xr::Posef::IDENTITY)?;
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
    let mut panel_pose = xr::Posef {
        orientation: xr::Quaternionf::IDENTITY,
        position: xr::Vector3f {
            x: 0.0,
            y: 0.0,
            z: -1.5,
        },
    };
    let mut panel_size = xr::Extent2Df {
        width: 1.2,
        height: 0.75,
    };
    let mut controllers = if show_editor {
        Some(super::input::Controllers::new(&instance, &session)?)
    } else {
        None
    };
    let mut stereo = if show_editor {
        Some(Stereo::new(&instance, &session, system, vk_format)?)
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
    let mut focused = false;
    let mut last_input = Instant::now();
    let mut anchor = xr::Posef::IDENTITY;
    let mut anchored = false;
    let mut stopping = false;
    let mut submitted = 0u64;
    let mut stereo_submitted = 0u64;
    loop {
        while let Some(event) = instance.poll_event(&mut events)? {
            match event {
                xr::Event::SessionStateChanged(event) => {
                    println!("[BenchUpXR] Session: {:?}", event.state());
                    focused = event.state() == xr::SessionState::FOCUSED;
                    if !focused {
                        if let (Some(controllers), Some(editor)) = (&mut controllers, &mut editor) {
                            editor.cancel_input(
                                controllers.cancel(),
                                &graphics.device,
                                &graphics.queue,
                            )?;
                        }
                    }
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
                xr::Event::InteractionProfileChanged(_) => {
                    if let Some(controllers) = &controllers {
                        controllers.log_profiles(&session)?;
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
        if let Some(editor) = &mut editor {
            let preferences = editor.panel_preferences();
            let recenter = editor.take_recenter_request();
            if !anchored || recenter {
                let head = view_space.locate(&space, frame.predicted_display_time)?;
                if head.location_flags.contains(
                    xr::SpaceLocationFlags::POSITION_VALID
                        | xr::SpaceLocationFlags::ORIENTATION_VALID,
                ) {
                    if let Some(pose) = upright_anchor(head.pose) {
                        anchor = pose;
                        anchored = true;
                    }
                }
            }
            panel_pose = panel_from_anchor(anchor, preferences.distance_metres);
            panel_size = xr::Extent2Df {
                width: preferences.width_metres,
                height: preferences.width_metres * size.height as f32 / size.width as f32,
            };
        }
        if let (Some(controllers), Some(editor)) = (&mut controllers, &mut editor) {
            let logical = editor.logical_size();
            let panel = super::EditorPanel::new(
                glam::DVec2::new(panel_size.width as f64, panel_size.height as f64),
                glam::DVec2::new(logical.x as f64, logical.y as f64),
            )
            .ok_or("Invalid editor panel dimensions")?;
            let now = Instant::now();
            if frame.should_render {
                let input = controllers.sample(
                    &session,
                    &space,
                    frame.predicted_display_time,
                    panel,
                    panel_pose,
                    focused,
                    now.duration_since(last_input).as_secs_f32(),
                    editor.scene_interaction_enabled(),
                    editor.navigation_enabled(),
                    |ray| editor.pick_scene_ray(ray),
                )?;
                editor.set_input(input);
            } else {
                editor.cancel_input(controllers.cancel(), &graphics.device, &graphics.queue)?;
            }
            last_input = now;
        }
        if !frame.should_render {
            stream.end(frame.predicted_display_time, blend, &[])?;
            continue;
        }
        let index = swapchain.acquire_image()? as usize;
        swapchain.wait_image(xr::Duration::INFINITE)?;
        // On graphics failure, never hand a possibly in-flight image back to the
        // compositor: unwind session resources before destroying the GPU owner.
        graphics.paint(
            images[index],
            format,
            size,
            &pipeline,
            editor.as_mut(),
            None,
        )?;
        swapchain.release_image()?;
        let mut projection_views = Vec::new();
        if let (Some(stereo), Some(editor)) = (&mut stereo, &mut editor) {
            if editor.panel_preferences().immersive_scene {
                let (state, views) = session.locate_views(
                    xr::ViewConfigurationType::PRIMARY_STEREO,
                    frame.predicted_display_time,
                    &space,
                )?;
                if views.len() == 2
                    && state.contains(
                        xr::ViewStateFlags::POSITION_VALID | xr::ViewStateFlags::ORIENTATION_VALID,
                    )
                {
                    let head = view_space.locate(&space, frame.predicted_display_time)?;
                    let scene_anchor = upright_anchor(head.pose).unwrap_or(anchor);
                    editor.begin_immersive_frame(scene_anchor);
                    for (eye_index, eye) in views.iter().enumerate() {
                        let chain = &mut stereo.chains[eye_index];
                        let image = chain.acquire_image()? as usize;
                        chain.wait_image(xr::Duration::INFINITE)?;
                        graphics.paint(
                            stereo.images[eye_index][image],
                            format,
                            stereo.size,
                            &pipeline,
                            Some(&mut *editor),
                            Some(*eye),
                        )?;
                        chain.release_image()?;
                    }
                    for (eye_index, eye) in views.iter().enumerate() {
                        projection_views.push(
                            xr::CompositionLayerProjectionView::new()
                                .pose(eye.pose)
                                .fov(eye.fov)
                                .sub_image(
                                    xr::SwapchainSubImage::new()
                                        .swapchain(&stereo.chains[eye_index])
                                        .image_array_index(0)
                                        .image_rect(xr::Rect2Di {
                                            offset: xr::Offset2Di { x: 0, y: 0 },
                                            extent: xr::Extent2Di {
                                                width: stereo.size.width as _,
                                                height: stereo.size.height as _,
                                            },
                                        }),
                                ),
                        );
                    }
                }
            }
        }
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
            .pose(panel_pose)
            .size(panel_size);
        if projection_views.is_empty() {
            stream.end(frame.predicted_display_time, blend, &[&quad])?;
        } else {
            let projection = xr::CompositionLayerProjection::new()
                .space(&space)
                .views(&projection_views);
            stream.end(frame.predicted_display_time, blend, &[&projection, &quad])?;
            stereo_submitted += 1;
            if stereo_submitted == 1 {
                println!(
                    "[BenchUpXR] First head-tracked stereoscopic 3D frame submitted. Look right of the editor panel; Fit 3D reframes the model."
                );
            }
        }
        submitted += 1;
        if submitted == 1 {
            if show_editor {
                println!(
                    "[BenchUpXR] First complete editor frame submitted to the quad (controller input enabled)."
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

// Recenter in a transient gravity-aligned frame. Saved preferences contain only
// distance/size/scale, never headset positions from a previous tracking session.
fn upright_anchor(head: xr::Posef) -> Option<xr::Posef> {
    let q = glam::DQuat::from_xyzw(
        head.orientation.x as f64,
        head.orientation.y as f64,
        head.orientation.z as f64,
        head.orientation.w as f64,
    );
    let position = glam::DVec3::new(
        head.position.x as f64,
        head.position.y as f64,
        head.position.z as f64,
    );
    if !q.is_finite() || q.length_squared() < 1e-12 || !position.is_finite() {
        return None;
    }
    let q = q.normalize();
    let forward = q * -glam::DVec3::Z;
    let yaw = if forward.x * forward.x + forward.z * forward.z > 1e-12 {
        (-forward.x).atan2(-forward.z)
    } else {
        let right = q * glam::DVec3::X;
        (-right.z).atan2(right.x)
    };
    let q = glam::DQuat::from_rotation_y(yaw);
    Some(xr::Posef {
        orientation: xr::Quaternionf {
            x: q.x as f32,
            y: q.y as f32,
            z: q.z as f32,
            w: q.w as f32,
        },
        position: head.position,
    })
}

fn panel_from_anchor(anchor: xr::Posef, distance: f32) -> xr::Posef {
    let q = glam::Quat::from_xyzw(
        anchor.orientation.x,
        anchor.orientation.y,
        anchor.orientation.z,
        anchor.orientation.w,
    );
    let offset = q * glam::vec3(0.0, 0.0, -distance);
    xr::Posef {
        orientation: anchor.orientation,
        position: xr::Vector3f {
            x: anchor.position.x + offset.x,
            y: anchor.position.y + offset.y,
            z: anchor.position.z + offset.z,
        },
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;
    #[test]
    fn recenter_places_panel_ahead_of_head_without_persisting_tracking_coordinates() {
        let q = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
            * glam::Quat::from_rotation_x(0.2);
        let anchor = upright_anchor(xr::Posef {
            orientation: xr::Quaternionf {
                x: q.x,
                y: q.y,
                z: q.z,
                w: q.w,
            },
            position: xr::Vector3f {
                x: 2.0,
                y: 1.7,
                z: -3.0,
            },
        })
        .unwrap();
        let panel = panel_from_anchor(anchor, 1.0);
        assert!((panel.position.x - 1.0).abs() < 1e-5);
        assert!((panel.position.y - 1.7).abs() < 1e-5);
        assert!((panel.position.z + 3.0).abs() < 1e-5);
        assert_eq!(panel.orientation.x, 0.0);
        assert_eq!(panel.orientation.z, 0.0);
    }
}
