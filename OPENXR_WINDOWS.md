# Windows OpenXR probes

Run these commands from this checkout in a host PowerShell terminal. The default
desktop application remains independent of the OpenXR loader.

## Current validation status (2026-11-11)

An isolated RTX 4090 Vulkan snapshot of frozen build `138d7f4` loaded the
standalone Khronos layer (advertised API 1.4.368) and the clean 404-face Example.
Synchronization validation was requested through the existing layer settings.
It wrote the image but reported **10** validation errors, all
`VUID-StandaloneSpirv-None-10684`, at the layer's default repeated-message cap;
the total occurrence count is not established. No other VUID category appeared. Validation is
not clean. Artifacts are in `.local/openxr/offscreen-validation-138d7f4-b90cda7c/`,
including `validation.log` and `validation-summary.json`. Exit success/image
creation alone is not a passed validation gate.

The lockfile pins wgpu/Naga 30.0.1. The local HAL selects SPIR-V 1.6 for Vulkan
1.3+ devices. The current [upstream issue #7696](https://github.com/gfx-rs/wgpu/issues/7696)
still records the same Function-layout failure on Naga 30.0.1. A
[candidate fork patch](https://github.com/sargunv/wgpu/commit/30580e4fa17a195d4746e8525028a04f90c2d3cc)
addresses layout-free types. An isolated app build with that patch renders the
same 404-face Example with zero validation errors and a visually unchanged image
(3 of 1,296,000 pixels differ by at most 1/255).
The layer is confirmed loaded; the only two warnings identify the explicitly
forced layer. Evidence: `.local/openxr/naga-layout-experiment/full-render/`.
The stock/fixed shader-module probes also reproduce/remove the error for the
world-line and edge shaders. No package-cache edits or diagnostic suppression
were used. These checks created no OpenXR session.

The workspace now pins `naga` and `naga-types` to the immutable candidate commit;
wgpu/HAL remain on the registry release. Both shader crates must share a source
to preserve type identity. The fork is three commits ahead of the release base;
only four SPIR-V backend files differ within Naga, and its unrelated DX12/HAL
changes are not selected. This is a deliberate compiler compatibility pin, not a
local path override. Remove the pin when an upstream release includes the fix.
The final remote-pinned editor example builds with `--locked --offline` and its
independent immersive and editor-panel snapshots both report zero errors/no
VUIDs with the layer loaded. See
`.local/openxr/naga-layout-experiment/pinned-render-summary.json` for hashes and
counts. Original transitive lockfile resolutions are preserved. Desktop/web
backend compatibility and OpenXR runtime validation are still separate gates.

This check created no OpenXR session. The historical runtime image/copy/lifetime
findings below therefore remain separate and unverified on the current build.
The current application has its queue drain before normal XR-resource teardown;
source inspection alone does not clear runtime validation. Ordinary previews
remain validation-disabled. Earlier probe sections below retain their historical
evidence; current implementation status is in the plan and handoff.

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
   `Vive OpenXR: Vive SRanipal`; two recommended 2112 Ã— 2304 views;
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

The quad uses a 1440 Ã— 900 sRGB image and measures 1.2 Ã— 0.75 metres, positioned
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

## Shared-editor render preview

`openxr_editor` uses the existing BenchUp UI, document, history and retained GPU
viewport callbacks on the same Vulkan device and directly into the acquired quad
image. The eframe entry point calls the same UI function through a small host
service adapter. Controller input and panel controls are described below. A
desktop mirror, physical keyboard/mouse input and native window-command handling
are not supplied. This does not satisfy phase 1's editing/saving acceptance gate.

```powershell
./scripts/openxr_probe.ps1 -Probe Editor -ValidationDirectory .local/openxr/validation-layers
```

The editor loads the usual workspace unless `BENCHUP_DATA_DIR` overrides it.
For automated checks, use an isolated directory and copy the bundled
`assets/examples/Example.bup` to its `model.json`, as the README capture helper does.
Preferences use the existing RON key/value format and shared capture/restore
logic, with immediate flushing even without a document edit. Malformed files and
intervening external changes detected before a save produce errors rather than
silently replacing the file.

Two explicit PNG diagnostics are available:

```powershell
# Runtime-selected Vulkan device; requires successful OpenXR instance creation.
./scripts/openxr_probe.ps1 -Probe Editor -Snapshot .local/openxr/editor-preview.png

# Independent hardware Vulkan rendering, without an OpenXR instance or session.
cargo run -p benchup-app --locked --offline --features openxr --example openxr_editor -- --desktop-snapshot .local/openxr/editor-desktop-preview.png
```

PNG readback belongs only to these diagnostic commands. The headset path has no
CPU image-copy/readback step. A desktop snapshot proves neither runtime adapter
selection nor XR image interoperability. It is useful for checking the shared
renderer when the headset is unavailable.

### Windows continuation evidence

The editor example compiled successfully after the workspace drive returned.
An independent Vulkan snapshot rendered the bundled Example project through the
shared UI and GPU viewport on the RTX 4090; the resulting 1440 Ã— 900 PNG was
visually inspected. It contains the model, toolbars, inspector and startup release
notes. This is a diagnostic image, not a release/README screenshot.
A second run restored the saved preferences/read state and produced
`.local/openxr/editor-desktop-preview-restored.png` without that modal; this image
was inspected as well. Five focused XR tests passed, including immediate
preference restoration without a document edit and rejection of a detected
intervening preference-file change.
Default desktop `cargo check -p benchup-app --locked --offline` passed. The
default WASM check also passed with `--target wasm32-unknown-unknown`, after
fetching missing locked dependencies. These are compilation checks, not browser
GPU or interactive desktop validation.

Core and synchronization validation were enabled. This run reproduced
`VUID-StandaloneSpirv-None-10684` layout-decoration errors with existing renderer
shaders even without an OpenXR instance/session. The messages match the open
[wgpu/Naga issue #7696](https://github.com/gfx-rs/wgpu/issues/7696).
No package-cache changes, dependency replacement or validation-message suppression
were applied. The independent renderer run therefore also does not pass the
clean-validation gate. Its validation error categories contained only this
SPIR-V error; no synchronization or resource-lifetime errors were reported in
the independent snapshot run.

After the drive interruption, SteamVR/OpenXR instance creation failed. The current
SteamVR startup log reported no connected headset (including
`VRInitError_Driver_WirelessHmdNotConnectedAndNoSteam`). Steam/SteamVR startup was
requested without changing the active runtime. Full-editor XR session submission
still requires a connected headset; the successful desktop PNG cannot substitute
for that result. Logs are `.local/openxr/editor-desktop-snapshot.log`,
`editor-snapshot-resume.log` and `preflight-resume.log`.

### Active-headset editor verification

After reconnection, preflight passed again with SteamVR/OpenXR 2.18.4 and the
Vive headset identity. The `openxr_editor` example selected the RTX 4090, created
the 1440 Ã— 900 three-image sRGB swapchain and submitted the complete editor quad.
The session reached VISIBLE and FOCUSED. The user confirmed that the editor was
visible, readable, upright and stable in the world during head movement.

The first active-headset run submitted 4,094 quad frames and stopped cleanly at
the timed exit. This frame count is not a performance benchmark: focus/visibility
changed during the run, and GPU timing and missed compositor frames were not
measured. The known runtime shader/image/copy/teardown diagnostics and Naga
SPIR-V errors remain; the clean-validation gate is still open. Evidence is in
`.local/openxr/editor-headset-ready.log`. Controller editing, native input/window
services, desktop mirroring and phase 1 end-to-end acceptance are still pending.

A second session started successfully on the same active runtime, reached
VISIBLE, submitted 4,258 editor quad frames and stopped cleanly. Its log is
`.local/openxr/editor-headset-restart.log`. Both processes exited successfully;
no SteamVR restart or system runtime change was needed between them.

## Controller-input preview

The editor preview now creates OpenXR aim, primary/secondary click and scroll
actions for KHR simple controllers, HTC Vive, Valve Index and Oculus Touch
profiles. Trigger/select is the primary button; Vive/simple menu and Index/Touch
B (Touch left menu) provide the secondary button. A touched trackpad/thumbstick
scrolls vertically. The runtime chooses the active interaction profile; diagnostic
profile/tracking changes use the `[BenchUpXR]` prefix.

A retained GPU cursor marks the current panel hit and changes color while a
button is held. Pointer coordinates follow egui's current logical scale. A fresh
press on the panel claims a controller; captured drags retain that controller
and continue outside the panel bounds. Tracking/focus loss releases outside all
widgets and cancels a captured editor operation. A held trigger cannot generate
a new press merely by re-entering the panel or regaining focus. Cancellation is
processed even if XR skips rendering, without queuing unbounded hover events.

For a longer manual test:

```powershell
cargo run -p benchup-app --locked --offline --features openxr --example openxr_editor -- --seconds 180
```

Eight focused XR tests passed, including drag capture, tracking/focus loss and
held-trigger suppression. A five-second runtime startup check created the action
bindings, reached VISIBLE, submitted 264 editor frames and stopped cleanly. In the
three-minute live test, the user confirmed the cursor and primary clicks work.
SteamVR selected `/interaction_profiles/oculus/touch_controller` for both hands,
and both aim spaces reported valid/tracked position and orientation (flags 15).
The user also confirmed wheel input affects the main viewport. A scrollbar/slider
drag test could not be completed because the user was unfamiliar with the app and
found the text too small at the fixed panel distance. The run stopped cleanly
after 12,191 editor frames; evidence is `.local/openxr/controller-live.log`.
Scrollbar dragging and focus/tracking-loss behavior remain unverified on hardware.
Panel distance and readability controls became the next usability requirement.
No user-facing XR settings were added in that initial controller milestone.
Physical keyboard/mouse input, text entry, desktop mirroring and native window
services are also pending.

## Persistent panel distance and readability

The editor now starts at 1.0 m with 130% text, and an always-visible, large
`XR panel` bar supplies Closer/Farther, Text -/+, Panel -/+ and Recenter controls.
The user confirmed that the controls are readable and work, and that Recenter
places the panel correctly. The chosen settings were written immediately during
the live run (0.8 m distance, 1.1 m width and 130% text at the observed point).
The live run stopped cleanly after 12,710 editor frames.

Distance, width and UI scale are serialized in `InspectorPreferences.xr_panel`
and use shared capture/restore plus `persist_inspector_changes`. The fields exist
in desktop/web builds without OpenXR too, so those hosts retain the preferences.
Older data uses defaults; restored values are sanitized. No document geometry or
history is changed by these options. The toolbar has a stable anchored layout.

Placement uses a transient, gravity-aligned anchor from the current VIEW pose at
startup/recenter. Only tracking-relative distance/size/scale are saved, never an
absolute headset pose. The controller mapper uses the same current panel pose,
dimensions and logical egui scale as the compositor/cursor.

Nine focused XR tests and the preference regression passed, including old-data
defaults, immediate saves without a model edit, restoration and invalid values.
The executable, default desktop check and default WASM check passed. A hardware
Vulkan PNG of the larger toolbar was inspected; a five-second XR startup check
submitted 331 frames and stopped cleanly. A subsequent 60-second restart reached
VISIBLE, submitted 582 frames and stopped cleanly. Its preference roundtrip
retained 0.8 m distance, 1.1 m width and 130% text instead of resetting to defaults;
human visual confirmation of that restart has not been received. The log is
`.local/openxr/readability-restoration.log`. Other evidence uses
`.local/openxr/readability-*.log` and
`.local/openxr/editor-readability.png`. Known Vulkan/Naga diagnostics remain.

## Immersive 3D functionality preview

The user requested prioritizing functionality and deferring validation. The
editor now submits a stereoscopic OpenXR projection layer as well as its UI quad.
Each eye uses its predicted display-time pose and asymmetric runtime FOV. Shared
retained GPU geometry renders both eyes; the scene comes from the same evaluated
model snapshot used by the editor. No CPU image transfer or per-eye model copy is
introduced.

The model initially appears to the right of the editor as a fitted, world-locked
inspection view. The XR bar supplies a persistent 3D-view toggle, 3D -/+ scale
controls and Fit 3D. These alter the navigation transform, not document geometry
or the desktop camera. Model coordinates are metres with Z up; XR coordinates use
Y up. The first path renders opaque materials/edges and selection appearance;
annotations, special display previews and editing tools remain on the panel.

The native executable built. A ten-second run selected the RTX 4090, created
2112 Ã— 2304 targets per eye, fitted the 404-face model, submitted head-tracked
stereo frames, reached VISIBLE/FOCUSED and stopped cleanly after 650 frames. The
user's preview reached STOPPING/IDLE/EXITING and exited successfully after 16,257
editor frames, with stereo scene submission active at startup. Human visual
confirmation of the 3D scene is not yet recorded. Validation layers, test suites and broad
backend checks are deferred as requested; existing validation issues are not
claimed resolved. Evidence is `.local/openxr/immersive-build.log`,
`immersive-startup.log` and `immersive-live.log`.
