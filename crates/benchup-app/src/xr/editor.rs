//! Shared editor rendering on the XR device. No alternate document or viewport.
use crate::{BenchUpApp, app_host::UiHost};
use eframe::{App as _, Storage as _, egui, egui_wgpu};
use std::{collections::BTreeMap, path::PathBuf, time::Instant};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// The desktop host uses the same RON key/value file and shared preference keys.
// Fail rather than overwriting a malformed file or another process's changes.
struct Preferences {
    path: PathBuf,
    values: BTreeMap<String, String>,
    previous: Option<Vec<u8>>,
    dirty: bool,
    error: Option<String>,
}

impl Preferences {
    fn open(path: PathBuf) -> Result<Self> {
        let previous = match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let values = previous
            .as_ref()
            .map(|bytes| ron::de::from_bytes(bytes))
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            path,
            values,
            previous,
            dirty: false,
            error: None,
        })
    }

    fn write(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let current = match std::fs::read(&self.path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if current != self.previous {
            return Err(
                "Preferences changed in another process; close the other editor before retrying"
                    .into(),
            );
        }
        let bytes = ron::to_string(&self.values)?.into_bytes();
        let parent = self
            .path
            .parent()
            .ok_or("Preferences path has no parent directory")?;
        std::fs::create_dir_all(parent)?;
        let mut staged = tempfile::NamedTempFile::new_in(parent)?;
        std::io::Write::write_all(&mut staged, &bytes)?;
        staged.as_file().sync_all()?;
        staged.persist(&self.path)?;
        self.previous = Some(bytes);
        self.dirty = false;
        Ok(())
    }
}

impl eframe::Storage for Preferences {
    fn get_string(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        if self.values.get(key) != Some(&value) {
            self.values.insert(key.into(), value);
            self.dirty = true;
        }
    }
    fn remove_string(&mut self, key: &str) {
        self.dirty |= self.values.remove(key).is_some();
    }
    fn flush(&mut self) {
        if let Err(error) = self.write() {
            self.error = Some(error.to_string());
        }
    }
}

pub(super) struct Editor {
    app: BenchUpApp,
    ctx: egui::Context,
    preferences: Preferences,
    renderer: egui_wgpu::Renderer,
    depth: wgpu::TextureView,
    size: [u32; 2],
    start: Instant,
    input: super::input::FrameInput,
    cursor: super::cursor::Cursor,
    recenter_requested: bool,
    scene: super::immersive::Scene,
    fit_scene_requested: bool,
    world_tools: crate::app::xr_tools::State,
    native_output: Option<super::desktop::NativeOutput>,
    keyboard_modifiers: egui::Modifiers,
}

impl Editor {
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        size: wgpu::Extent3d,
    ) -> Result<Self> {
        let preferences = Preferences::open(crate::preferences_path()?)?;
        let ctx = egui::Context::default();
        let mut renderer = egui_wgpu::Renderer::new(
            device,
            format,
            egui_wgpu::RendererOptions {
                depth_stencil_format: Some(wgpu::TextureFormat::Depth32Float),
                dithering: false,
                ..Default::default()
            },
        );
        crate::renderer::install_resources(device, format, &mut renderer);
        let app = BenchUpApp::new_with_host(
            &ctx,
            Some(&preferences),
            true,
            Some((device.clone(), queue.clone())),
        );
        ctx.set_zoom_factor(app.xr_panel_preferences().ui_scale);
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("BenchUpXR editor depth"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Ok(Self {
            app,
            ctx,
            preferences,
            renderer,
            depth,
            size: [size.width, size.height],
            start: Instant::now(),
            input: Default::default(),
            cursor: super::cursor::Cursor::new(device, format),
            recenter_requested: false,
            scene: super::immersive::Scene::new(device, format),
            fit_scene_requested: true,
            world_tools: Default::default(),
            native_output: None,
            keyboard_modifiers: Default::default(),
        })
    }

    pub(super) fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
    ) -> Result<wgpu::SubmissionIndex> {
        self.cursor
            .update(queue, &self.input, self.size, self.ctx.pixels_per_point());
        let mut output = self.update_ui()?;
        let jobs = self
            .ctx
            .tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: self.size,
            pixels_per_point: output.pixels_per_point,
        };
        self.update_textures(device, queue, &mut output);
        let mut encoder = device.create_command_encoder(&Default::default());
        let commands = self
            .renderer
            .update_buffers(device, queue, &mut encoder, &jobs, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BenchUpXR complete editor"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            let mut pass = pass.forget_lifetime();
            self.renderer.render(&mut pass, &jobs, &screen);
            self.cursor.paint(&mut pass);
        }
        let submission = queue.submit(commands.into_iter().chain([encoder.finish()]));
        self.free_textures(&mut output);
        Ok(submission)
    }

    fn update_ui(&mut self) -> Result<egui::FullOutput> {
        let mut events = std::mem::take(&mut self.input.events);
        events.insert(0, egui::Event::ModifiersChanged(self.keyboard_modifiers));
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                self.logical_size(),
            )),
            time: Some(self.start.elapsed().as_secs_f64()),
            focused: self.input.focused,
            events,
            ..Default::default()
        };
        let mut preferences = self.app.xr_panel_preferences();
        let mut recenter = false;
        let mut fit_scene = false;
        let pointer = self
            .input
            .scene
            .and_then(|input| self.scene.tool_pointer(input, preferences.scene_scale));
        // egui may repeat layout; consume a model operation only once.
        let mut activate = true;
        let mut output = self.ctx.run_ui(raw, |ui| {
            // Leave shared text/name/file dialogs fully visible and clickable.
            if self.ctx.memory(|memory| memory.top_modal_layer().is_none()) {
            egui::Panel::bottom("benchup.xr.panel_controls")
                .resizable(false)
                .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(8))
                .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "XR panel   {:.1} m away   {:.1} m wide   Text {:.0}%",
                                    preferences.distance_metres,
                                    preferences.width_metres,
                                    preferences.ui_scale * 100.0
                                ))
                                .size(18.0),
                            );
                            ui.horizontal(|ui| {
                                let mut button = |label: &str| {
                                    ui.add_sized(
                                        [84.0, 42.0],
                                        egui::Button::new(egui::RichText::new(label).size(18.0)),
                                    )
                                    .clicked()
                                };
                                if button("Closer") {
                                    preferences.distance_metres -= 0.1;
                                }
                                if button("Farther") {
                                    preferences.distance_metres += 0.1;
                                }
                                if button("Text -") {
                                    preferences.ui_scale -= 0.1;
                                }
                                if button("Text +") {
                                    preferences.ui_scale += 0.1;
                                }
                                if button("Panel -") {
                                    preferences.width_metres -= 0.1;
                                }
                                if button("Panel +") {
                                    preferences.width_metres += 0.1;
                                }
                                if button("Recenter") {
                                    recenter = true;
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.checkbox(
                                    &mut preferences.immersive_scene,
                                    egui::RichText::new("3D view (look right)").size(18.0),
                                );
                                ui.checkbox(&mut preferences.keep_upright,egui::RichText::new("Keep upright").size(18.0))
                                    .on_hover_text("Keep the scene upright while grabbing. Horizontal turning, vertical movement and scaling remain available.");
                                let mut button = |label: &str| {
                                    ui.add_sized(
                                        [84.0, 42.0],
                                        egui::Button::new(egui::RichText::new(label).size(18.0)),
                                    )
                                    .clicked()
                                };
                                if button("Fit 3D") {
                                    fit_scene = true;
                                    preferences.scene_scale = 1.0;
                                }
                                if button("3D -") {
                                    preferences.scene_scale *= 0.8;
                                }
                                if button("3D +") {
                                    preferences.scene_scale *= 1.25;
                                }
                            });
                            self.app.xr_tool_controls(ui, &mut self.preferences, &mut self.world_tools);
                });
            }
            self.app
                .ui_with_host(ui, &mut UiHost::OpenXr(&mut self.preferences));
            self.app.xr_world_pointer(pointer.as_ref(), &mut self.world_tools, activate, &self.ctx, &mut self.preferences);
            activate = false;
            if preferences.immersive_scene {
                self.scene.stage_labels(&self.ctx, &self.app.xr_scene_source(&self.ctx));
            }
        });
        preferences = preferences.sanitized();
        if preferences != self.app.xr_panel_preferences() {
            self.app
                .update_xr_panel_preferences(&self.ctx, &mut self.preferences, preferences);
            self.ctx.set_zoom_factor(preferences.ui_scale);
        }
        self.recenter_requested |= recenter;
        self.fit_scene_requested |= fit_scene;
        // Immediate preference persistence uses the same shared capture/restore
        // path as desktop, even without document edits or framework autosave.
        self.preferences.flush();
        if let Some(error) = self.preferences.error.take() {
            return Err(error.into());
        }
        if let Some(native) = &self.native_output {
            if output.viewport_output.get(&egui::ViewportId::ROOT)
                .is_some_and(|viewport| viewport.commands.iter().any(|command| matches!(command,egui::ViewportCommand::Close))) {
                native.request_close();
            }
            native.platform_output(std::mem::take(&mut output.platform_output));
        }
        Ok(output)
    }

    fn update_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output: &mut egui::FullOutput,
    ) {
        for (id, deltas) in output.textures_delta.set.drain() {
            for delta in deltas {
                self.renderer.update_texture(device, queue, id, &delta);
            }
        }
    }

    fn free_textures(&mut self, output: &mut egui::FullOutput) {
        for id in output.textures_delta.free.drain() {
            self.renderer.free_texture(&id);
        }
    }

    pub(super) fn set_input(&mut self, mut input: super::input::FrameInput) {
        if self.input.focused && !input.focused {
            self.input.events.push(egui::Event::WindowFocused(false));
        }
        if !input.focused { self.keyboard_modifiers = Default::default(); }
        for event in &mut input.events {
            if let egui::Event::PointerButton {modifiers,..} = event { *modifiers = self.keyboard_modifiers; }
        }
        self.scene.navigate(input.navigation,self.app.xr_panel_preferences());
        self.scene.set_input(input.scene);
        // Retain releases/cancellation accumulated while XR skipped rendering.
        self.input.events.append(&mut input.events);
        self.input.cursor = input.cursor;
        self.input.pressed = input.pressed;
        self.input.focused = input.focused;
        self.input.scene = input.scene;
        self.input.navigation=input.navigation;
    }

    pub(super) fn attach_native(&mut self, output: super::desktop::NativeOutput) {
        output.ready(self.ctx.clone());
        self.native_output = Some(output);
    }

    pub(super) fn native_keyboard(&mut self, mut input: super::desktop::KeyboardInput, active: bool) {
        if active {
            for event in &input.events {
                if let egui::Event::ModifiersChanged(modifiers) = event { self.keyboard_modifiers = *modifiers; }
            }
            self.input.events.append(&mut input.events);
        } else {
            self.keyboard_modifiers = Default::default();
        }
    }

    pub(super) fn logical_size(&self) -> egui::Vec2 {
        egui::vec2(self.size[0] as f32, self.size[1] as f32) / self.ctx.pixels_per_point()
    }

    pub(super) fn panel_preferences(&self) -> crate::app_host::XrPanelPreferences {
        self.app.xr_panel_preferences()
    }

    pub(super) fn scene_interaction_enabled(&self) -> bool {
        self.app.xr_panel_preferences().immersive_scene
            && self.app.xr_scene_interaction_enabled(&self.ctx)
    }

    pub(super) fn navigation_enabled(&self)->bool {
        self.app.xr_panel_preferences().immersive_scene && self.ctx.memory(|memory|memory.top_modal_layer().is_none())
    }

    pub(super) fn pick_scene_ray(&mut self, ray: benchup_core::Ray) -> Option<super::input::SceneHit> {
        self.scene
            .pick_ray(ray, self.app.xr_panel_preferences())
    }

    pub(super) fn scene_input_head(&mut self, pose: openxr::Posef) { self.scene.input_head(pose); }

    pub(super) fn take_recenter_request(&mut self) -> bool {
        std::mem::take(&mut self.recenter_requested)
    }

    pub(super) fn begin_immersive_frame(&mut self, anchor: openxr::Posef) {
        let mut source = self.app.xr_scene_source(&self.ctx);
        source.tool_target = self.world_tools.pointer_target().or(source.tool_target);
        self.scene.begin_frame(
            source,
            anchor,
            std::mem::take(&mut self.fit_scene_requested),
        );
    }

    pub(super) fn paint_eye(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        size: [u32; 2],
        eye: openxr::View,
    ) -> wgpu::SubmissionIndex {
        self.scene.paint_eye(
            device,
            queue,
            &mut self.renderer,
            target,
            size,
            eye,
            self.app.xr_panel_preferences(),
        )
    }

    pub(super) fn paint_immersive_snapshot(&mut self,device:&wgpu::Device,queue:&wgpu::Queue,target:&wgpu::TextureView,size:[u32;2])->Result<()> {
        self.begin_immersive_frame(openxr::Posef::IDENTITY);
        // Placement is now known, so settle model-side HUD anchors and fonts.
        self.paint(device,queue,target)?;
        let eye=self.scene.diagnostic_eye(self.app.xr_panel_preferences().scene_scale,size).ok_or("The immersive snapshot needs a model with bounds")?;
        self.paint_eye(device,queue,target,size,eye);
        Ok(())
    }

    pub(super) fn cancel_input(
        &mut self,
        input: super::input::FrameInput,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        self.set_input(input);
        // Cancellation must reach world tools even when rendering is skipped
        // and no panel pointer events need a UI pass.
        self.app.xr_world_pointer(None, &mut self.world_tools, false, &self.ctx, &mut self.preferences);
        if !self.input.events.is_empty() {
            let mut output = self.update_ui()?;
            self.update_textures(device, queue, &mut output);
            self.free_textures(&mut output);
        }
        Ok(())
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.app.save(&mut self.preferences);
        self.app.on_exit();
        self.preferences.flush();
        if let Some(error) = &self.preferences.error {
            eprintln!("[BenchUpXR] Preference save failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_survive_immediate_restart_without_document_edits_and_reject_concurrent_writes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("app.ron");
        let mut first = Preferences::open(path.clone()).unwrap();
        first.set_string("benchup.test", "saved".into());
        first.flush();
        assert!(first.error.is_none());
        let restored = Preferences::open(path.clone()).unwrap();
        assert_eq!(
            restored.get_string("benchup.test").as_deref(),
            Some("saved")
        );
        std::fs::write(&path, "{}").unwrap();
        first.set_string("benchup.test", "new".into());
        first.flush();
        assert!(first.error.is_some());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{}");
    }
}
