# BenchUp OpenXR implementation plan

Date: 2026-11-09

## Outcomes and initial target

1. Run the complete existing editor as an interactive 2D panel in a headset.
2. Separate the editor UI from the model viewport: retain readable floating UI
   panels while rendering the model as head-tracked, stereoscopic VR content.

Recommended first target: a Windows PC with a SteamVR-connected headset, using
OpenXR and Vulkan. This is an engineering recommendation for the existing desktop
application: it avoids the Android packaging, lifecycle, file access and native
service adaptations needed for standalone Quest. Hardware availability must be
confirmed before the runtime milestone. A Windows PC with SteamVR and a connected
headset has now been confirmed available; test access is still to be arranged.
Steam Frame and standalone Quest are
subsequent platform targets, with their own device validation.

A headset's virtual desktop can provide an immediate 2D usability baseline with
the existing executable. That does not constitute native OpenXR support and does
not satisfy the immersive milestone. The browser client remains supported;
browser XR would be a separate WebXR integration, not a native OpenXR build.

## Current architecture

- `crates/benchup-app/src/main.rs` starts an eframe window or web runner.
- The application uses eframe/egui 0.37 and wgpu 30; the native entry point selects
  wgpu and a throughput-oriented desktop surface configuration.
- `renderer.rs` and `resident_scene.rs` install resources and render through egui
  callbacks. `renderer::install_resources` is an existing seam for a custom host.
- `viewports.rs` already shares one document, selection and history across cameras,
  but temporarily installs each camera on the document while painting. XR views
  should instead receive explicit render parameters.
- Some viewport annotations are still egui painter operations. They require an
  explicit panel-versus-world classification before immersive rendering.

Preserve the retained GPU scene, document operations, undo history and existing
desktop/web behavior. Do not duplicate the editor or read textures back through
the CPU to get them into the headset.

## Phase 1 — Complete editor on a headset panel

### 1A. Panel and input groundwork — started

Implement panel dimensions in metres, logical UI dimensions in egui points, and
controller-ray intersection in panel-local space. Reject invalid tracking rays,
backside interactions and misses. Map the top-left corner to UI origin and retain
metric hit distance for input arbitration. Add regression coverage for orientation,
scale, boundaries, angled rays and invalid inputs.

Initial implementation: `crates/benchup-app/src/xr.rs`, exposed only on native
targets. It contains panel geometry and four regression tests. It is not yet
connected to an OpenXR runtime or the app's input loop. No user-facing control or
saved preference is introduced in this preparatory change.

### 1B. Prove runtime and graphics interoperability — runtime preflight implemented

Introduce an optional native OpenXR host/dependency behind a Cargo feature. Keep
the default desktop and WASM builds independent of the OpenXR loader. Begin with
runtime diagnostics and a coloured panel, before integrating the full editor.

Use runtime graphics requirements and adapter selection to create a compatible
Vulkan device and queue. Prototype importing runtime-owned swapchain images into
wgpu, with explicit ownership, image-layout transitions, queue synchronization,
format negotiation and teardown. wgpu's unsafe HAL APIs are an integration seam,
not proof of compatibility. Encapsulate them in a small reviewed module and pin
compatible dependency versions. If the bridge fails, resolve that decision before
large renderer refactoring; do not substitute CPU image copies.

Drive XR frames with `xrWaitFrame`, `xrBeginFrame`, image acquire/wait, GPU rendering,
image release and `xrEndFrame`. Respect predicted display time, `shouldRender`,
session visibility/focus, stopping, loss and shutdown. Do not inherit the desktop
throughput-oriented queueing policy blindly; XR requires latency-aware pacing.

**Gate:** a stable panel in the target headset, correct colors/orientation, clean
session restart and missing-runtime behavior, and no graphics validation errors.

### 1C. Render the existing editor into the panel

Extract the application frame/UI entry point from the eframe-specific host without
forking document state. Audit dependencies on `eframe::Frame`, creation context,
storage, dialogs, clipboard, file opening, updates and window commands. Supply
host services where needed.

Render egui, including the current 3D viewport callbacks, into a GPU texture and
submit it as an OpenXR quad composition layer. Start with one flat panel placed in
front of the user in a stable reference space. Keep panel physical size, texture
pixel resolution and logical UI size distinct. Provide a desktop mirror for
debugging without letting its refresh cadence govern the headset.

### 1D. Interaction, preferences and usability

Bind OpenXR actions for aim pose, select, secondary action, scrolling and recenter.
Convert the controller aim ray into panel-local coordinates and use the 1A mapper
to feed egui pointer events. Preserve pointer capture during dragging; synthesize
release/cancel on focus or tracking loss and arbitrate mouse/controller ownership.
Support physical keyboard and mouse first, then an explicit in-headset text-entry
path for dimensions, project names and search. Ensure file dialogs and modals are
reachable from the headset.

Add persistent controls for panel size/distance, UI scale and handedness using
`InspectorPreferences` capture/restore and `persist_inspector_changes`, with
backward-compatible defaults. Store display choices in `DisplaySettings` where
appropriate. Persist placement preferences relative to a recentered frame, not
stale tracking-space coordinates. Preserve modal action ordering and English UI.

**Phase 1 acceptance:** open, edit, undo, save and reopen a real project entirely
through the panel; accurate controller clicks and drags; readable menus; usable
text input; preferences survive immediate restart without a document edit; headset
disconnect/reconnect does not corrupt the document. Profile Rooftop with hardware
GPU rendering and measure CPU/GPU timing and missed compositor frames at the
selected headset refresh rate. A runtime panel demo alone does not complete phase 1.

## Phase 2 — Immersive model with separate UI

### 2A. Separate scene preparation, views and editor UI

Introduce explicit render-view inputs: view/projection matrices, eye position,
target size, clipping and render origin. Separate document/scene updates from
per-view rendering, with one authoritative document and one set of retained GPU
geometry buffers. Desktop viewports use the same interface.

Classify every overlay: menus, inspector and dialogs stay in UI panels; selection,
guides, gizmos, measurements, handles and editing previews become world-space GPU
content. Preserve visibility, occlusion and interaction semantics. Do not rebuild
CPU scene meshes or duplicate geometry for each eye.

### 2B. Head-tracked stereo rendering

Submit the model through an OpenXR projection layer, with runtime-provided eye
poses and asymmetric fields of view at predicted display time. Render two views
using shared resources first; optimize with multiview only after measuring.
Keep the UI in a separate quad layer, with intentional overlay/depth behavior.

Define and test model-units-to-metres conversion, axes/handedness, depth range and
origin precision. Maintain a separate XR navigation transform: moving the user or
scaling the viewing experience must not alter model geometry or the desktop camera.
Offer tabletop inspection and true-scale walkthrough, with recenter, teleport and
snap-turn before optional smooth locomotion. Persist comfort/display preferences;
keep project-specific visibility and assignments in the document.

**Gate:** correct stereo scale and clipping, stable tracking, readable separate UI,
and representative large models meeting the measured headset frame budget.

### 2C. Editing in immersive space

Start with inspection and ray selection, then move/rotate/scale, snapping,
measurements and existing modeling tools. Route world-space rays through shared
picking and commands; avoid converting everything back into desktop pixels.
Define UI capture versus scene interaction, drag constraints, numeric entry,
cancel/undo and controller feedback. Keep GPU-rendered previews and highlights.

**Phase 2 acceptance:** edit the same document in desktop, panel and immersive
modes without divergence; validate controller selection and tools on Rooftop;
cover focus loss, tracking loss, reference-space changes, scale extremes and
immediate preference restoration. Verify existing desktop and browser rendering
on hardware GPUs as well as the headset.

### Controller-input progress

The editor preview now binds aim, primary/secondary clicks and vertical scrolling
for KHR simple, HTC Vive, Valve Index and Oculus Touch profiles. A retained GPU
cursor shows the logical UI hit. Drag capture retains its controller outside the
panel; focus/tracking loss releases outside controls, cancels a captured gesture
and suppresses held-trigger presses on re-entry. Eight focused XR tests pass.
The user confirmed live cursor, primary clicks and wheel input on the main view;
the three-minute session stopped cleanly after 12,191 frames. Drag verification
was blocked by small text and the fixed panel distance. Add persistent distance
and UI-size controls before asking the user to navigate unfamiliar editor UI.

## Validation and rollout

- Unit tests: panel hit mapping, input capture, coordinate transforms, preference
  migration/restoration, and session state transitions.
- GPU integration: imported-image lifetime/synchronization, color space, stereo
  projection and offscreen callbacks, followed by actual headset validation.
- Stage releases as opt-in panel preview, panel beta, immersive inspection preview,
  then immersive editing. Keep release gates separate from implementation progress.
- Use existing process/compiler logs for the repository's final compilation check;
  do not launch a build solely for that check. Any later deploy uses the existing
  GitHub Actions release workflow, including its prohibition on local release builds.
- No schedule is committed before the graphics bridge and target hardware have
  been proven. Those are the largest early uncertainties.

## References

- [Khronos OpenXR tutorial and platform matrix](https://openxr-tutorial.com/index.html)
- [OpenXR quad layers](https://registry.khronos.org/OpenXR/specs/1.1/man/html/XrCompositionLayerQuad.html)
- [Khronos frame submission guide](https://github.com/KhronosGroup/OpenXR-Guide/blob/main/chapters/frame_submission.md)
- [wgpu native device interoperability](https://docs.rs/wgpu/30.0.1/wgpu/struct.Device.html)
- [Windows/Vulkan OpenXR setup](https://www.openxr-tutorial.com/windows/vulkan/1-introduction.html)

## Current implementation status

Phase 1A is implemented in source; runtime integration, renderer hosting, persistent
XR controls and headset validation remain pending. BenchUp cannot yet launch in
OpenXR. The next implementation milestone is 1B on a confirmed Windows/SteamVR
test machine and headset.

The first part of 1B now provides the optional `openxr` Cargo feature and an
explicit developer diagnostic command:

```sh
cargo run -p benchup-app --locked --features openxr --example openxr_diagnostics
```

On the initial Windows target, install a conformant OpenXR loader
(`openxr_loader.dll` must be available to the executable), select SteamVR as the
active OpenXR runtime, and connect/wake the headset. Run from a terminal so that
the output and exit status remain visible. This tool does not install software,
select a runtime, change preferences, or launch the editor.

The command reports progress through loader initialization, the required
`XR_KHR_vulkan_enable2` extension, runtime identity, headset identity/tracking,
primary stereo view dimensions and Vulkan API requirements. Failures return a
nonzero status with stage-specific context. Success only proves preflight
capabilities: it does not yet prove device selection, wgpu image sharing, a working
session, frame submission or headset rendering. Normal app startup never loads
OpenXR, even when built with the feature. The dependency is native-only and
dynamically loaded; the WASM application does not link it.

Next: use the runtime-selected Vulkan adapter/device, validate swapchain-image
interoperability with wgpu, then submit the first colored quad on the target
headset. The macOS development host can compile this groundwork and exercise
missing-loader handling, but is not the selected headset validation platform.

### Windows graphics progress, 2026-11-09

The optional `openxr_panel` example now creates Vulkan through the runtime's
enable2 entry points, imports the selected adapter/device and borrowed swapchain
images into wgpu 30.0.1, and submits a GPU-rendered orientation-pattern quad.
The frame loop handles shouldRender, READY/STOPPING, exit and loss; rendering is
single-frame paced with completion before release and a device drain at teardown.
See `OPENXR_WINDOWS.md` for reproducible commands and evidence.

Repeated Windows runs submitted frames on the RTX 4090 and stopped cleanly while
the headset was unattended. They reached SYNCHRONIZED but not VISIBLE/FOCUSED.
The pattern shader's initial validation error was corrected. Other validation
errors remain in runtime-created shaders, external-image setup and runtime copies
involving BlankEyeBuffer. Phase 1B's visual, restart-under-use, and clean-validation
acceptance gate remains open. This is a graphics prototype, not phase 1 completion.

Phase 1C initially added a render-only `openxr_editor` example. It calls the same editor
constructor/UI through a small host-services adapter and installs the existing
GPU viewport callbacks on the XR device. Preferences retain the shared RON
key/value format and immediate capture/restore persistence. The desktop and web
entry points still use eframe through this same UI seam.

An independent RTX 4090 Vulkan snapshot rendered the bundled Example editor and
was visually inspected. After the user reconnected the headset, an editor quad
run reached VISIBLE/FOCUSED and submitted 4,094 frames before stopping cleanly.
The user confirmed the editor was visible, readable, upright, and stable during
head movement. This establishes render-only headset submission.
A second session reached VISIBLE, submitted 4,258 frames and stopped cleanly
without restarting SteamVR. The independent
snapshot also reproduces the known wgpu/Naga #7696 SPIR-V validation
errors without an XR session. Desktop mirroring, input, native window-command
services and phase 1 acceptance remain outstanding.

Persistent distance, width and UI-scale controls are now implemented using shared
InspectorPreferences capture/restore, including builds without the OpenXR
feature. Defaults are 1.0 m and 130% text; a large always-visible XR bar also
provides recentering in a fresh gravity-aligned tracking-relative frame. The user
confirmed the controls are readable and work, including recentering. Nine XR
tests plus one preference regression passed; desktop/WASM compilation and a
hardware snapshot of the larger controls also passed. A 582-frame restart stopped
cleanly and retained the chosen settings on disk; human visual confirmation of
that restart is pending. These controls do not complete the remaining phase 1 gates.

## Priority change: immersive 3D functionality

The user requested deferring further validation work and reaching head-tracked
3D scene rendering promptly. The next implementation adds an OpenXR projection
layer alongside the existing editor quad. Each eye uses its runtime pose and
asymmetric FOV; a shared retained GPU scene supplies geometry for both eyes.
The scene uses the editor's evaluated model snapshot, not a second document.

Initial functionality is opaque materials/edges and selection appearance. Scene
annotations and special display previews remain on the editor panel. The model
is fitted as a world-locked inspection view to the right of the panel so the
opaque UI does not cover it. Model Z-up to XR Y-up, fitting and scale are view
transforms; the source model and desktop camera are unchanged. Persistent
3D-view and scale controls plus a Fit 3D action are included. The executable built
and a ten-second runtime run submitted stereo scene frames, reached VISIBLE/
FOCUSED and stopped cleanly after 650 frames. A five-minute user preview is now
running. Further test suites, graphics validation and broad backend checks are
deferred at the user's request while functionality is prioritized.
