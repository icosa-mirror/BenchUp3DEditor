//! Render-only shared-editor preview. Controller input is a subsequent milestone.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let result = match arguments.as_slice() {
        [] => benchup_app::xr::panel::run_editor(std::time::Duration::from_secs(60)),
        [flag, path] if flag == "--snapshot" => {
            benchup_app::xr::panel::snapshot_editor(std::path::Path::new(path))
        }
        [flag, path] if flag == "--desktop-snapshot" => pollster::block_on(
            benchup_app::xr::panel::snapshot_editor_without_runtime(std::path::Path::new(path)),
        ),
        _ => {
            eprintln!(
                "Usage: openxr_editor [--snapshot output.png | --desktop-snapshot output.png]"
            );
            return std::process::ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[BenchUpXR] Editor preview failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
