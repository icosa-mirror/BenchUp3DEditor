//! Explicit developer preflight; does not open the editor or change preferences.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    match benchup_app::xr::runtime::diagnose(std::io::stdout().lock()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("OpenXR preflight failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // The browser app does not link or load the native OpenXR dependency.
}
