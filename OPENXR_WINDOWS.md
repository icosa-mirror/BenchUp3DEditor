# Windows OpenXR probes

Run these commands from this checkout in a host PowerShell terminal. The default
desktop application remains independent of the OpenXR loader.

## Prerequisites and loader discovery

Use Rust/MSVC, a Vulkan-capable GPU driver, SteamVR selected as the active OpenXR
runtime, and a connected, awake headset. These probes do not select a runtime or
install software.

The plain diagnostics command is:

```powershell
cargo run -p benchup-app --locked --features openxr --example openxr_diagnostics
```

SteamVR can be installed and running without `openxr_loader.dll` being on `PATH`.
The helper finds SteamVR in the current user's Steam installation and temporarily
adds its `bin/win64` directory to the probe's environment:

```powershell
./scripts/openxr_probe.ps1 -Probe Diagnostics
```

For SteamVR in another library, pass its actual directory explicitly:

```powershell
./scripts/openxr_probe.ps1 -Probe Diagnostics -LoaderDirectory 'D:/SteamLibrary/steamapps/common/SteamVR/bin/win64'
```

The helper restores environment variables when it finishes. It does not copy DLLs
into the application directory or change the system `PATH` or active runtime.

## Vulkan validation

Khronos publishes a standalone Windows x64 validation package in its
[nightly release](https://github.com/KhronosGroup/Vulkan-ValidationLayers/releases/tag/nightly).
Extract its matching DLL and JSON manifest together into a local directory. No
Vulkan SDK installation or registry change is needed. Keep downloaded binaries
under ignored `.local/`, outside version control.

Pass that directory with `-ValidationDirectory` when running a probe. The helper
requests `VK_LAYER_KHRONOS_validation`, synchronization validation, and console
logging using settings under `.local/openxr/settings`. Confirm the layer's startup
message in the log; the helper's request alone does not prove it was loaded.
Validation results supplement headset observations; they do not establish correct
panel placement, colors, orientation, or usability.

## Initial machine evidence, 2026-11-09

1. Branch `feat/openxr-phase1` was clean at `53c3fb6` and matched its fetched origin.
2. Rust and MSVC were available. Vulkan reported NVIDIA GeForce RTX 4090
   (API 1.4.341) and AMD Radeon Graphics (API 1.4.315).
3. The plain command compiled, then failed to discover the OpenXR loader.
   Adding SteamVR's installed loader directory to the process `PATH` resolved it.
4. Preflight passed: SteamVR/OpenXR in Meta compatibility mode 2.18.4;
   `Vive OpenXR: Vive SRanipal`; two recommended 2112 × 2304 views;
   Vulkan requirements 1.0.0 through 1.2.0. Orientation and position tracking
   were both reported unavailable at that time. This is capability evidence,
   not successful rendering or live tracking evidence.
5. Khronos nightly Windows x64 validation layers were downloaded into
   `.local/openxr/validation-layers`. The downloaded ZIP SHA256 was
   `BC8ED6B0B618B6AE3EDB4F24457A5E546726E54190D0BA8A2CA30CFD6A446496`.
   The nightly URL is mutable; this hash identifies this particular download.

Local evidence logs are under `.local/openxr/`; they are not release artifacts.
