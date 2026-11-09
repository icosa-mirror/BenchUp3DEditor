//! Services needed by the shared editor UI, independent of its rendering host.
use eframe::Frame;

pub(crate) enum UiHost<'a> {
    Desktop(&'a mut Frame),
    #[cfg(all(feature = "openxr", not(target_arch = "wasm32")))]
    OpenXr(&'a mut (dyn eframe::Storage + 'static)),
}

impl UiHost<'_> {
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
