//! In-panel chooser for the existing asynchronous project-file workers.
use crate::{
    app_host::XrFilePreferences,
    modal::Modal,
    native_dialog::{Host, Request},
};
use eframe::egui;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

struct Entry {
    path: PathBuf,
    name: String,
    directory: bool,
    hidden: bool,
}

struct Browser {
    request: Request,
    directory: PathBuf,
    folder_input: String,
    filename: String,
    entries: Vec<Entry>,
    visible: Vec<usize>,
    loading: Option<mpsc::Receiver<Result<Vec<Entry>, String>>>,
    error: Option<String>,
    replace: Option<PathBuf>,
}

pub(super) struct Dialogs {
    requests: mpsc::Receiver<Request>,
    browser: Option<Browser>,
}

impl Dialogs {
    pub(super) fn new(ctx: &egui::Context) -> Self {
        Self {
            requests: Host::install(ctx),
            browser: None,
        }
    }

    pub(super) fn show(&mut self, ctx: &egui::Context, preferences: &mut XrFilePreferences) {
        if self.browser.is_none() {
            if let Ok(request) = self.requests.try_recv() {
                let directory = preferences.directory.clone().unwrap_or_else(home_directory);
                self.browser = Some(Browser::new(request, directory, ctx));
            }
        }
        let Some(browser) = &mut self.browser else {
            return;
        };
        browser.poll(preferences);
        let mut finish: Option<Option<PathBuf>> = None;
        if let Some(path) = browser.replace.clone() {
            let response = Modal::new("benchup.xr.file_replace", 700.).show(ctx, |modal| {
                modal.header(|ui| {
                    ui.heading("Replace existing file?");
                });
                modal.body(|ui| {
                    ui.label(path.display().to_string());
                    ui.label("The existing file will be replaced when the save completes.");
                });
                modal.footer(|ui| {
                    if action(ui, "Cancel") {
                        browser.replace = None;
                    }
                    if action(ui, "Replace") {
                        finish = Some(Some(path.clone()));
                    }
                });
            });
            if response.should_close() {
                browser.replace = None;
            }
        } else {
            let response = Modal::new("benchup.xr.file_chooser", 900.).show(ctx, |modal| {
                modal.header(|ui| {
                    ui.heading(&browser.request.title);
                });
                modal.body(|ui| {
                    ui.horizontal(|ui| {
                        if action(ui, "Up") {
                            if let Some(parent) = browser.directory.parent() {
                                browser.navigate(parent.to_path_buf(), ctx);
                            }
                        }
                        if action(ui, "Home") {
                            browser.navigate(home_directory(), ctx);
                        }
                        if action(ui, "Refresh") {
                            browser.navigate(browser.directory.clone(), ctx);
                        }
                    });
                    let mut navigate = false;
                    ui.horizontal(|ui| {
                        ui.label("Folder");
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut browser.folder_input)
                                .id(egui::Id::unique("benchup.xr.file_folder"))
                                .font(egui::FontId::proportional(18.))
                                .desired_width((ui.available_width() - 80.).max(100.)),
                        );
                        navigate |= response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter));
                        navigate |= action(ui, "Go");
                    });
                    if navigate {
                        browser.navigate(PathBuf::from(browser.folder_input.trim()), ctx);
                    }
                    let mut submit = false;
                    ui.horizontal(|ui| {
                        ui.label("File");
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut browser.filename)
                                .id(egui::Id::unique("benchup.xr.file_name"))
                                .font(egui::FontId::proportional(18.))
                                .desired_width(ui.available_width()),
                        );
                        submit |= response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    });
                    ui.horizontal(|ui| {
                        let changed = ui
                            .checkbox(&mut preferences.show_hidden, "Show hidden files")
                            .changed()
                            | ui.checkbox(&mut preferences.show_all_files, "Show all file types")
                                .changed();
                        if changed {
                            browser.filter(preferences);
                        }
                    });
                    if !browser.request.extensions.is_empty() {
                        ui.label(format!(
                            "File types: {}",
                            browser.request.extensions.join(", ")
                        ));
                    }
                    if let Some(error) = &browser.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    if browser.loading.is_some() {
                        ui.spinner();
                        ui.label("Reading folder…");
                    }
                    let mut selected = None;
                    egui::ScrollArea::vertical()
                        .id_salt("benchup.xr.file_list")
                        .max_height(240.)
                        .show_rows(ui, 42., browser.visible.len(), |ui, rows| {
                            for row in rows {
                                let entry = &browser.entries[browser.visible[row]];
                                let label = if entry.directory {
                                    format!("Folder: {}", entry.name)
                                } else {
                                    entry.name.clone()
                                };
                                let response = ui.add_sized(
                                    [ui.available_width(), 42.],
                                    egui::Button::new(egui::RichText::new(label).size(18.)),
                                );
                                if response.clicked() {
                                    selected = Some((entry.path.clone(), entry.directory));
                                }
                            }
                        });
                    if let Some((path, directory)) = selected {
                        if directory {
                            browser.navigate(path, ctx);
                        } else {
                            browser.filename = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned();
                        }
                    }
                    if submit {
                        finish = browser.submit();
                    }
                });
                modal.footer(|ui| {
                    if action(ui, "Cancel") {
                        finish = Some(None);
                    }
                    ui.add_enabled_ui(
                        browser.loading.is_none() && !browser.filename.trim().is_empty(),
                        |ui| {
                            if action(ui, if browser.request.save { "Save" } else { "Open" }) {
                                finish = browser.submit();
                            }
                        },
                    );
                });
            });
            if response.should_close() {
                finish = Some(None);
            }
        }
        if let Some(path) = finish {
            if let Some(browser) = self.browser.take() {
                browser.request.respond(path);
            }
        }
    }
}

impl Browser {
    fn new(request: Request, directory: PathBuf, ctx: &egui::Context) -> Self {
        let mut browser = Self {
            filename: request.filename.clone(),
            request,
            directory: directory.clone(),
            folder_input: String::new(),
            entries: Vec::new(),
            visible: Vec::new(),
            loading: None,
            error: None,
            replace: None,
        };
        browser.navigate(directory, ctx);
        browser
    }

    fn navigate(&mut self, path: PathBuf, ctx: &egui::Context) {
        let base = if self.directory.is_absolute() {
            self.directory.clone()
        } else {
            std::env::current_dir().unwrap_or_default()
        };
        let path = if path.is_absolute() {
            path
        } else {
            base.join(path)
        };
        self.folder_input = path.to_string_lossy().into_owned();
        self.directory = path.clone();
        self.entries.clear();
        self.visible.clear();
        self.error = None;
        self.replace = None;
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(read_directory(&path));
            ctx.request_repaint();
        });
        self.loading = Some(receiver);
    }

    fn poll(&mut self, preferences: &mut XrFilePreferences) {
        let result = match self.loading.as_ref().map(|receiver| receiver.try_recv()) {
            Some(Ok(result)) => result,
            Some(Err(mpsc::TryRecvError::Disconnected)) => Err("Folder reader stopped.".into()),
            _ => return,
        };
        self.loading = None;
        match result {
            Ok(entries) => {
                self.entries = entries;
                preferences.directory = Some(self.directory.clone());
                self.filter(preferences);
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn filter(&mut self, preferences: &XrFilePreferences) {
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                ((!entry.hidden || preferences.show_hidden)
                    && (entry.directory
                        || preferences.show_all_files
                        || self.request.accepts(&entry.path)))
                .then_some(index)
            })
            .collect();
    }

    fn submit(&mut self) -> Option<Option<PathBuf>> {
        if self.loading.is_some() || self.filename.trim().is_empty() {
            return None;
        }
        let path = self.directory.join(self.filename.trim());
        if path.is_dir() {
            self.error = Some("Choose a file, or open a folder from the list.".into());
            return None;
        }
        if !self.directory.is_dir() {
            self.error = Some("Choose an existing folder first.".into());
            return None;
        }
        if self.request.save {
            let path = self.request.save_path(path);
            if path.exists() {
                self.replace = Some(path);
                None
            } else {
                Some(Some(path))
            }
        } else if path.is_file() {
            Some(Some(path))
        } else {
            self.error = Some("That file does not exist.".into());
            None
        }
    }
}

fn action(ui: &mut egui::Ui, label: &str) -> bool {
    ui.add(egui::Button::new(egui::RichText::new(label).size(18.)).min_size(egui::vec2(70., 42.)))
        .clicked()
}

fn home_directory() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

fn read_directory(path: &Path) -> Result<Vec<Entry>, String> {
    let mut entries = std::fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            #[cfg(windows)]
            let hidden = {
                use std::os::windows::fs::MetadataExt;
                metadata.file_attributes() & 2 != 0
            };
            #[cfg(not(windows))]
            let hidden = false;
            Some(Entry {
                path: entry.path(),
                directory: metadata.is_dir(),
                hidden: hidden || name.starts_with('.'),
                name,
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by_cached_key(|entry| (!entry.directory, entry.name.to_lowercase()));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_ui::ContextExt as _;
    use std::time::Duration;

    #[test]
    fn chooser_footer_stays_above_the_virtual_keyboard() {
        let ctx = egui::Context::default();
        let requests = Host::install(&ctx);
        let host = Host::from_context(&ctx);
        let worker = std::thread::spawn(move || {
            host.dialog()
                .set_title("Save project")
                .add_filter("Project", &["bup"])
                .set_file_name("scene.bup")
                .save_file()
        });
        let request = requests.recv_timeout(Duration::from_secs(5)).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let mut browser = Browser::new(request, directory.path().to_path_buf(), &ctx);
        browser
            .loading
            .take()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        let (_sender, receiver) = mpsc::channel();
        let mut dialogs = Dialogs {
            requests: receiver,
            browser: Some(browser),
        };
        let mut preferences = XrFilePreferences::default();
        let mut keyboard = super::super::keyboard::Keyboard::default();
        let mut panel = crate::app_host::XrPanelPreferences {
            keyboard_visible: true,
            ..Default::default()
        };
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output = ctx.run_test_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000., 800.),
                    )),
                    ..Default::default()
                },
                |_| {
                    crate::modal::set_bottom_inset(
                        &ctx,
                        super::super::keyboard::Keyboard::height(true),
                    );
                    dialogs.show(&ctx, &mut preferences);
                    keyboard.show(&ctx, &mut panel);
                },
            );
        }
        fn bottom(shape: &egui::Shape, label: &str) -> Option<f32> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos.y + text.galley.rect.bottom())
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| bottom(shape, label)),
                _ => None,
            }
        }
        for label in ["Cancel", "Save"] {
            let y = output
                .shapes
                .iter()
                .find_map(|shape| bottom(&shape.shape, label))
                .expect("visible footer action");
            assert!(y < 510., "{label} overlaps the keyboard at {y}");
        }
        dialogs.browser.take().unwrap().request.respond(None);
        assert_eq!(worker.join().unwrap(), None);
    }

    #[test]
    fn save_confirms_the_final_extended_path_and_cancel_preserves_it() {
        let ctx = egui::Context::default();
        let requests = Host::install(&ctx);
        let host = Host::from_context(&ctx);
        let worker =
            std::thread::spawn(move || host.dialog().add_filter("Project", &["bup"]).save_file());
        let request = requests.recv_timeout(Duration::from_secs(5)).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("scene.bup");
        std::fs::write(&existing, b"original").unwrap();
        let mut browser = Browser::new(request, directory.path().to_path_buf(), &ctx);
        let entries = browser
            .loading
            .take()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        browser.entries = entries;
        browser.filename = "scene".into();
        assert!(browser.submit().is_none());
        assert_eq!(browser.replace.as_ref(), Some(&existing));
        browser.request.respond(None);
        assert_eq!(worker.join().unwrap(), None);
        assert_eq!(std::fs::read(existing).unwrap(), b"original");
    }

    #[test]
    fn folders_remain_visible_when_filtering_and_path_changes_persist_after_read() {
        let ctx = egui::Context::default();
        let requests = Host::install(&ctx);
        let host = Host::from_context(&ctx);
        let worker =
            std::thread::spawn(move || host.dialog().add_filter("Project", &["bup"]).pick_file());
        let request = requests.recv_timeout(Duration::from_secs(5)).unwrap();
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("folder")).unwrap();
        for name in ["model.bup", "other.txt", ".hidden.bup"] {
            std::fs::write(directory.path().join(name), b"").unwrap();
        }
        let mut browser = Browser::new(request, directory.path().to_path_buf(), &ctx);
        let receiver = browser.loading.take().unwrap();
        let entries = receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        let (sender, receiver) = mpsc::channel();
        sender.send(Ok(entries)).unwrap();
        browser.loading = Some(receiver);
        let mut preferences = XrFilePreferences::default();
        browser.poll(&mut preferences);
        assert_eq!(preferences.directory.as_deref(), Some(directory.path()));
        let names = |browser: &Browser| {
            browser
                .visible
                .iter()
                .map(|index| browser.entries[*index].name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&browser), ["folder", "model.bup"]);
        preferences.show_all_files = true;
        preferences.show_hidden = true;
        browser.filter(&preferences);
        assert_eq!(browser.visible.len(), 4);
        browser.navigate(PathBuf::from("folder"), &ctx);
        let entries = browser
            .loading
            .take()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert!(entries.is_empty());
        assert_eq!(browser.directory, directory.path().join("folder"));
        assert!(browser.directory.is_absolute());
        browser.request.respond(None);
        assert_eq!(worker.join().unwrap(), None);
    }
}
