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
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                self.logical_size(),
            )),
            time: Some(self.start.elapsed().as_secs_f64()),
            focused: self.input.focused,
            events: std::mem::take(&mut self.input.events),
            ..Default::default()
        };
        let output = self.ctx.run_ui(raw, |ui| {
            self.app
                .ui_with_host(ui, &mut UiHost::OpenXr(&mut self.preferences))
        });
        // Immediate preference persistence uses the same shared capture/restore
        // path as desktop, even without document edits or framework autosave.
        self.preferences.flush();
        if let Some(error) = self.preferences.error.take() {
            return Err(error.into());
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
        // Retain releases/cancellation accumulated while XR skipped rendering.
        self.input.events.append(&mut input.events);
        self.input.cursor = input.cursor;
        self.input.pressed = input.pressed;
        self.input.focused = input.focused;
    }

    pub(super) fn logical_size(&self) -> egui::Vec2 {
        egui::vec2(self.size[0] as f32, self.size[1] as f32) / self.ctx.pixels_per_point()
    }

    pub(super) fn cancel_input(
        &mut self,
        input: super::input::FrameInput,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        self.set_input(input);
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
