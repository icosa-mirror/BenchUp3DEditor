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

## Quad graphics probe

```powershell
./scripts/openxr_probe.ps1 -Probe Panel -ValidationDirectory .local/openxr/validation-layers
```

Without validation, omit `-ValidationDirectory`. The executable is also available
as `cargo run -p benchup-app --locked --features openxr --example openxr_panel`
when the loader is already discoverable. It runs for 60 seconds, requests session
exit, and waits for the runtime's STOPPING state before ending the session.

The quad uses a 1440 × 900 sRGB image and measures 1.2 × 0.75 metres, positioned
1.5 metres along negative Z in LOCAL space. Expected quadrants are red/green on
top and blue/white below. This is an isolated graphics probe, without a document,
controller actions, editor UI, or saved settings.

The OpenXR Vulkan enable2 entry points create the instance and logical device
using wgpu's extension/feature requirements and the runtime-selected physical GPU.
The bridge pins ash 0.38.0 and wgpu 30.0.1. It borrows each acquired image with
external memory and a non-owning drop callback. Its declared initial state and
final render usage both correspond to COLOR_ATTACHMENT_OPTIMAL. It waits for GPU
completion before releasing the image, drains deferred image-view destruction,
and drains the bound Vulkan device before XR-resource teardown. There is no CPU
texture readback or copy path.

The probe currently selects Vulkan 1.3, wgpu 30's maximum backend API version.
OpenXR's maximum graphics version check uses the supported **major** version,
matching the openxrs Vulkan example. This probe rejects loaders/GPUs below 1.3;
it is not a claim that wgpu itself requires 1.3 on every platform.

Two successive Windows runs created the session and swapchain on the RTX 4090,
submitted two quad frames each, reached SYNCHRONIZED, and stopped cleanly. Neither
reached VISIBLE or FOCUSED while the headset was unattended. No user observation
of the panel has been obtained.

Validation is **not clean**. Initial validation exposed a Function-array SPIR-V
decoration in the pattern shader; replacing that array with arithmetic removed
that error. Remaining diagnostics include shaders created inside xrCreateSession
(before the pattern pipeline), external D3D11 image initialization, and copy/layout
hazards involving the runtime's named `BlankEyeBuffer`. The probe issues no image
copy commands. Explicitly enabling validation only on the application instance
still observes runtime commands using the bound device. Do not suppress these
messages or treat them as a passed graphics-validation gate. With an explicit
device drain before XR-resource destruction, the third run also submitted two
frames and stopped cleanly; the earlier in-use-memory teardown error disappeared.
Validation still reported 14 runtime-created objects at device destruction
(command buffers, semaphores, fences and memory). The teardown gate remains open.
