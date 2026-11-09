//! A normal desktop lifetime window, independent of the XR frame wait.
use eframe::egui;
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

enum Event {
    Finished(Result<(), String>),
    Ready(egui::Context),
    PlatformOutput(egui::PlatformOutput),
}

pub(super) struct KeyboardInput {
    pub events: Vec<egui::Event>,
}

#[derive(Clone)]
pub(super) struct NativeOutput(EventLoopProxy<Event>);

impl NativeOutput {
    pub fn ready(&self, ctx: egui::Context) {
        let _ = self.0.send_event(Event::Ready(ctx));
    }
    pub fn platform_output(&self, output: egui::PlatformOutput) {
        let _ = self.0.send_event(Event::PlatformOutput(output));
    }
}

pub(super) struct NativeInput {
    pub output: NativeOutput,
    pub events: Receiver<KeyboardInput>,
}

struct Desktop {
    proxy: EventLoopProxy<Event>,
    window: Option<Window>,
    worker: Option<JoinHandle<()>>,
    close: Arc<AtomicBool>,
    closing: Option<Instant>,
    duration: Option<Duration>,
    result: Option<Result<(), String>>,
    finished: bool,
    input: Option<egui_winit::State>,
    keys: Vec<egui::Key>,
    keyboard_tx: Sender<KeyboardInput>,
    keyboard_rx: Option<Receiver<KeyboardInput>>,
}

impl ApplicationHandler<Event> for Desktop {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.worker.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("BenchUp VR — close this window to exit")
            .with_inner_size(winit::dpi::LogicalSize::new(640., 360.));
        match event_loop.create_window(attributes) {
            Ok(window) => self.window = Some(window),
            Err(error) => {
                self.result = Some(Err(format!("Could not create the desktop window: {error}")));
                event_loop.exit();
                return;
            }
        }
        println!("[BenchUpXR] Desktop window opened; Close ends the VR session.");
        let proxy = self.proxy.clone();
        let close = self.close.clone();
        let duration = self.duration;
        let native = NativeInput {
            output: NativeOutput(proxy.clone()),
            events: self.keyboard_rx.take().expect("One XR worker"),
        };
        match std::thread::Builder::new()
            .name("BenchUp OpenXR".into())
            .spawn(move || {
                // Report a worker panic too, so the desktop never waits forever for
                // a completion notification that will not arrive.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    super::panel::run_editor_session(duration, &close, native)
                        .map_err(|error| error.to_string())
                }))
                .unwrap_or_else(|_| {
                    Err("The OpenXR worker panicked; see the application log.".into())
                });
                let _ = proxy.send_event(Event::Finished(result));
            }) {
            Ok(worker) => self.worker = Some(worker),
            Err(error) => {
                self.result = Some(Err(format!("Could not start OpenXR: {error}")));
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self
            .window
            .as_ref()
            .is_none_or(|window| window.id() != window_id)
        {
            return;
        }
        if matches!(
            event,
            WindowEvent::KeyboardInput { .. }
                | WindowEvent::ModifiersChanged(_)
                | WindowEvent::Ime(_)
                | WindowEvent::Focused(_)
        ) {
            if let (Some(window), Some(input)) = (&self.window, &mut self.input) {
                let _ = input.on_window_event(window, &event);
                let mut raw = input.take_egui_input(window);
                // Native keyboard focus is independent of headset/pointer
                // focus. Do not inject desktop mouse or global-focus events.
                raw.events.retain(|event| {
                    matches!(
                        event,
                        egui::Event::Key { .. }
                            | egui::Event::ModifiersChanged(_)
                            | egui::Event::Text(_)
                            | egui::Event::Ime(_)
                            | egui::Event::Copy
                            | egui::Event::Cut
                            | egui::Event::Paste(_)
                    )
                });
                for event in &raw.events {
                    if let egui::Event::Key { key, pressed, .. } = event {
                        if *pressed && !self.keys.contains(key) {
                            self.keys.push(*key);
                        }
                        if !pressed {
                            self.keys.retain(|held| held != key);
                        }
                    }
                }
                if matches!(event, WindowEvent::Focused(false)) {
                    raw.events
                        .push(egui::Event::ModifiersChanged(Default::default()));
                    raw.events
                        .extend(self.keys.drain(..).map(|key| egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: false,
                            repeat: false,
                            modifiers: Default::default(),
                        }));
                }
                let _ = self.keyboard_tx.send(KeyboardInput { events: raw.events });
            }
        }
        if let WindowEvent::CloseRequested = event {
            println!("[BenchUpXR] Desktop Close requested.");
            self.close.store(true, Ordering::Relaxed);
            self.closing = Some(Instant::now());
            // Remove the window/taskbar entry immediately while XR releases its
            // swapchains and device on its own thread.
            self.window = None;
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(50),
            ));
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        match event {
            Event::Finished(result) => {
                self.finished = true;
                self.result = Some(result);
                event_loop.exit();
            }
            Event::Ready(ctx) => {
                if let Some(window) = &self.window {
                    self.input = Some(egui_winit::State::new(
                        ctx,
                        egui::ViewportId::ROOT,
                        event_loop,
                        Some(window.scale_factor() as f32),
                        window.theme(),
                        None,
                    ));
                    println!("[BenchUpXR] Desktop keyboard connected to the XR editor.");
                }
            }
            Event::PlatformOutput(output) => {
                if let (Some(window), Some(input)) = (&self.window, &mut self.input) {
                    input.handle_platform_output(window, output);
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(closing) = self.closing {
            // Keep desktop Close usable even if the runtime/driver blocks in
            // wait_frame or teardown. This bounds shutdown, not the VR session.
            if closing.elapsed() >= Duration::from_secs(8) {
                self.result = Some(Err(
                    "The OpenXR runtime did not finish shutdown after desktop Close.".into(),
                ));
                event_loop.exit();
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(50),
                ));
            }
        }
    }
}

pub(super) fn run(duration: Option<Duration>) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::<Event>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let (keyboard_tx, keyboard_rx) = mpsc::channel();
    let mut desktop = Desktop {
        proxy: event_loop.create_proxy(),
        window: None,
        worker: None,
        close: Arc::new(AtomicBool::new(false)),
        closing: None,
        duration,
        result: None,
        finished: false,
        input: None,
        keys: Vec::new(),
        keyboard_tx,
        keyboard_rx: Some(keyboard_rx),
    };
    let result = event_loop.run_app(&mut desktop);
    desktop.close.store(true, Ordering::Relaxed);
    if desktop.finished
        && let Some(worker) = desktop.worker.take()
    {
        // All XR resources are already dropped before Finished is sent.
        let _ = worker.join();
    }
    result?;
    desktop
        .result
        .unwrap_or_else(|| Err("Desktop event loop ended before OpenXR shutdown completed.".into()))
        .map_err(Into::into)
}
