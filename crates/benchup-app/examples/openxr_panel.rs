//! Explicit, time-limited graphics interoperability probe; does not open a document.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    match benchup_app::xr::panel::run(std::time::Duration::from_secs(60)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[BenchUpXR] Panel failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
