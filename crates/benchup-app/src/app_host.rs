//! Services needed by the shared editor UI, independent of its rendering host.
use eframe::Frame;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct XrFilePreferences {
    pub directory: Option<std::path::PathBuf>,
    pub show_hidden: bool,
    pub show_all_files: bool,
}

/// Tracking-relative XR preferences are retained by every host, including builds
/// without OpenXR. Never store absolute runtime tracking-space coordinates.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct XrPanelPreferences {
    pub distance_metres: f32,
    pub width_metres: f32,
    pub ui_scale: f32,
    pub immersive_scene: bool,
    pub scene_scale: f32,
    pub keep_upright:bool,
    pub extend_selection: bool,
    pub keyboard_visible: bool,
    pub keyboard_caps: bool,
    pub keyboard_symbols: bool,
    pub precise_modeling: bool,
}

impl Default for XrPanelPreferences {
    fn default() -> Self {
        Self {
            distance_metres: 1.0,
            width_metres: 1.2,
            ui_scale: 1.3,
            immersive_scene: true,
            scene_scale: 1.0,
            keep_upright:true,
            extend_selection: false,
            keyboard_visible: false,
            keyboard_caps: false,
            keyboard_symbols: false,
            precise_modeling: false,
        }
    }
}

impl XrPanelPreferences {
    pub fn sanitized(self) -> Self {
        let defaults = Self::default();
        let clamp = |value: f32, fallback: f32, min: f32, max: f32| {
            if value.is_finite() {
                value.clamp(min, max)
            } else {
                fallback
            }
        };
        Self {
            distance_metres: clamp(self.distance_metres, defaults.distance_metres, 0.5, 3.0),
            width_metres: clamp(self.width_metres, defaults.width_metres, 0.6, 2.4),
            ui_scale: clamp(self.ui_scale, defaults.ui_scale, 1.0, 1.6),
            immersive_scene: self.immersive_scene,
            scene_scale: clamp(self.scene_scale, defaults.scene_scale, 0.05, 20.0),
            keep_upright:self.keep_upright,
            extend_selection: self.extend_selection,
            keyboard_visible: self.keyboard_visible,
            keyboard_caps: self.keyboard_caps,
            keyboard_symbols: self.keyboard_symbols,
            precise_modeling: self.precise_modeling,
        }
    }
}

pub(crate) enum UiHost<'a> {
    Desktop(&'a mut Frame),
    #[cfg(all(feature = "openxr", not(target_arch = "wasm32")))]
    OpenXr(&'a mut (dyn eframe::Storage + 'static)),
}

impl UiHost<'_> {
    pub(crate) fn is_xr(&self) -> bool {
        match self {
            Self::Desktop(_) => false,
            #[cfg(all(feature = "openxr", not(target_arch = "wasm32")))]
            Self::OpenXr(_) => true,
        }
    }

    pub(crate) fn frame_mut(&mut self) -> Option<&mut Frame> {
        match self {
            Self::Desktop(frame) => Some(frame),
            #[cfg(all(feature = "openxr", not(target_arch = "wasm32")))]
            Self::OpenXr(_) => None,
        }
    }

    pub(crate) fn storage_mut(&mut self) -> Option<&mut (dyn eframe::Storage + 'static)> {
        match self {
            Self::Desktop(frame) => frame.storage_mut(),
            #[cfg(all(feature = "openxr", not(target_arch = "wasm32")))]
            Self::OpenXr(storage) => Some(&mut **storage),
        }
    }
}
