//! Read-only runtime preflight for the Vulkan panel prototype.
//!
//! This deliberately creates no graphics device, session or swapchain. A successful
//! probe establishes runtime capabilities, not wgpu interoperability or rendering.

use openxr as xr;
use std::io::{self, Write};

/// Write progress as each step succeeds, so a later failure retains useful context.
/// Loading is explicit: enabling the Cargo feature never loads XR at app startup.
pub fn diagnose(mut output: impl Write) -> Result<(), String> {
    macro_rules! report {
        ($($arg:tt)*) => {
            writeln!(output, $($arg)*).map_err(|error| format!("Cannot write OpenXR diagnostics: {error}"))?
        };
    }

    report!("BenchUp OpenXR preflight (Vulkan; no rendering session)");
    // SAFETY: use the platform loader name supplied by openxrs, relying on the
    // installed OpenXR loader's ABI. No user-provided function pointers are used.
    let entry = unsafe { xr::Entry::load(&()) }.map_err(|error| {
        format!("Cannot load OpenXR: {error}. Install the platform OpenXR loader and configure an active runtime (initial target: Windows with SteamVR).")
    })?;
    report!("OpenXR loader loaded.");
    let supported = entry.enumerate_extensions().map_err(|error| {
        format!("Cannot enumerate runtime extensions: {error}. Check that an OpenXR runtime is installed and selected.")
    })?;
    if !supported.khr_vulkan_enable2 {
        return Err("The active runtime does not support XR_KHR_vulkan_enable2, required by the planned Vulkan renderer.".into());
    }
    report!("XR_KHR_vulkan_enable2 supported.");
    let mut extensions = xr::ExtensionSet::default();
    extensions.khr_vulkan_enable2 = true;
    let instance = entry
        .create_instance(
            &xr::ApplicationInfo {
                application_name: "BenchUp OpenXR diagnostics",
                engine_name: "BenchUp",
                ..Default::default()
            },
            &extensions,
            &[],
            &(),
        )
        .map_err(|error| format!("Cannot create an OpenXR instance: {error}"))?;
    let properties = instance
        .properties()
        .map_err(|error| format!("Cannot read runtime properties: {error}"))?;
    report!(
        "Runtime: {} {}",
        properties.runtime_name,
        properties.runtime_version
    );
    let system = instance
        .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
        .map_err(|error| {
            format!(
                "No usable headset: {error}. Connect and wake the headset, then start its runtime."
            )
        })?;
    let properties = instance
        .system_properties(system)
        .map_err(|error| format!("Cannot read headset properties: {error}"))?;
    report!("Headset: {}", properties.system_name);
    report!(
        "Tracking: orientation={}, position={}",
        properties.tracking_properties.orientation_tracking,
        properties.tracking_properties.position_tracking
    );

    let views = instance
        .enumerate_view_configuration_views(system, xr::ViewConfigurationType::PRIMARY_STEREO)
        .map_err(|error| format!("Cannot query primary stereo views: {error}"))?;
    if views.len() != 2 {
        return Err(format!(
            "Expected two primary stereo views; runtime returned {}.",
            views.len()
        ));
    }
    for (index, view) in views.iter().enumerate() {
        report!(
            "Eye {index}: recommended {} x {}, samples {}; maximum {} x {}",
            view.recommended_image_rect_width,
            view.recommended_image_rect_height,
            view.recommended_swapchain_sample_count,
            view.max_image_rect_width,
            view.max_image_rect_height
        );
    }
    let requirements = instance
        .graphics_requirements::<xr::Vulkan>(system)
        .map_err(|error| format!("Cannot query Vulkan graphics requirements: {error}"))?;
    report!(
        "Vulkan API range: {} through {}",
        requirements.min_api_version_supported,
        requirements.max_api_version_supported
    );
    report!(
        "Preflight passed. GPU sharing, session creation and headset rendering remain unverified."
    );
    output
        .flush()
        .map_err(|error: io::Error| format!("Cannot flush OpenXR diagnostics: {error}"))?;
    Ok(())
}
