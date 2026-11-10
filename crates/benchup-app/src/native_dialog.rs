//! File workers use the host's chooser; normal desktop hosts retain native dialogs.
use eframe::egui;
use std::{path::PathBuf, sync::mpsc};

#[derive(Clone, Default)]
pub(crate) struct Host {
    sender: Option<mpsc::Sender<Request>>,
}

#[cfg_attr(not(feature = "openxr"), allow(dead_code))]
pub(crate) struct Request {
    pub title: String,
    pub filename: String,
    pub extensions: Vec<String>,
    pub save: bool,
    response: mpsc::Sender<Option<PathBuf>>,
}

#[cfg_attr(not(feature = "openxr"), allow(dead_code))]
impl Request {
    pub fn respond(self, path: Option<PathBuf>) {
        let _ = self.response.send(path);
    }

    pub fn accepts(&self, path: &std::path::Path) -> bool {
        self.extensions.is_empty()
            || path.extension().is_some_and(|extension| {
                extension.to_str().is_some_and(|extension| {
                    self.extensions
                        .iter()
                        .any(|allowed| extension.eq_ignore_ascii_case(allowed))
                })
            })
    }

    // Check replacement against the final filename, including the extension the
    // shared export/project writer would add. Never confirm a different path.
    pub fn save_path(&self, mut path: PathBuf) -> PathBuf {
        if !self.accepts(&path) {
            if let Some(extension) = self.extensions.first() {
                let mut name = path.file_name().unwrap_or_default().to_os_string();
                name.push(".");
                name.push(extension);
                path.set_file_name(name);
            }
        }
        path
    }
}

impl Host {
    pub fn from_context(ctx: &egui::Context) -> Self {
        ctx.data(|data| data.get_temp::<Self>(egui::Id::unique("benchup.file_dialog.host")))
            .unwrap_or_default()
    }

    #[cfg(feature = "openxr")]
    pub fn install(ctx: &egui::Context) -> mpsc::Receiver<Request> {
        let (sender, receiver) = mpsc::channel();
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::unique("benchup.file_dialog.host"),
                Self {
                    sender: Some(sender),
                },
            )
        });
        receiver
    }

    pub fn dialog(self) -> Dialog {
        Dialog {
            native: rfd::FileDialog::new(),
            sender: self.sender,
            title: String::new(),
            filename: String::new(),
            extensions: Vec::new(),
        }
    }
}

pub(crate) struct Dialog {
    native: rfd::FileDialog,
    sender: Option<mpsc::Sender<Request>>,
    title: String,
    filename: String,
    extensions: Vec<String>,
}

impl Dialog {
    pub fn set_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self.native = self.native.set_title(&self.title);
        self
    }

    pub fn set_file_name(mut self, filename: impl Into<String>) -> Self {
        self.filename = filename.into();
        self.native = self.native.set_file_name(&self.filename);
        self
    }

    pub fn add_filter(mut self, name: impl Into<String>, extensions: &[&str]) -> Self {
        self.native = self.native.add_filter(name, extensions);
        self.extensions
            .extend(extensions.iter().map(|extension| (*extension).to_owned()));
        self
    }

    pub fn pick_file(self) -> Option<PathBuf> {
        self.choose(false)
    }
    pub fn save_file(self) -> Option<PathBuf> {
        self.choose(true)
    }

    fn choose(self, save: bool) -> Option<PathBuf> {
        if let Some(sender) = self.sender {
            let (response, receiver) = mpsc::channel();
            sender
                .send(Request {
                    title: self.title,
                    filename: self.filename,
                    extensions: self.extensions,
                    save,
                    response,
                })
                .ok()?;
            receiver.recv().ok().flatten()
        } else if save {
            self.native.save_file()
        } else {
            self.native.pick_file()
        }
    }
}

#[cfg(all(test, feature = "openxr"))]
mod tests {
    use super::*;
    use crate::project_files::{self, Outcome};
    use std::time::Duration;

    #[test]
    fn xr_chooser_uses_existing_project_save_open_and_cancel_workers() {
        let ctx = egui::Context::default();
        let requests = Host::install(&ctx);
        let directory = tempfile::tempdir().unwrap();
        let mut document = benchup_core::Document::default();
        document.name = "XR round trip".into();
        let pending = project_files::save(
            &ctx,
            None,
            document.name.clone(),
            serde_json::to_string(&document).unwrap(),
        );
        let request = requests.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(request.save);
        let path = request.save_path(directory.path().join("scene"));
        assert_eq!(path.extension().unwrap(), "bup");
        request.respond(Some(path.clone()));
        assert!(
            matches!(pending.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(), Outcome::Saved(Some(saved)) if saved == path)
        );
        let decoded = project_files::decode(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(decoded.name, document.name);
        let pending = project_files::open(&ctx);
        let request = requests.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(!request.save);
        assert!(request.accepts(&path));
        request.respond(Some(path.clone()));
        assert!(
            matches!(pending.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(), Outcome::Opened(doc, Some(opened)) if doc.name == document.name && opened == path)
        );
        let pending = project_files::open(&ctx);
        requests
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .respond(None);
        assert!(matches!(
            pending
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap(),
            Outcome::Cancelled
        ));
    }
}
