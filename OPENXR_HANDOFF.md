# OpenXR handoff to the Windows agent

## User intent

Continue implementing `OPENXR_PLAN.md` in phases:
1. The full BenchUp editor as an interactive 2D OpenXR panel.
2. Separate floating UI from an immersive stereoscopic model viewport.

The user confirmed a Windows PC with SteamVR and a connected headset is available.
They want work committed as coherent milestones are completed. They requested this
handoff because the current development machine is macOS. Use the Windows machine
for actual runtime/GPU validation, and ask directly if a prerequisite needs fixing.

## Branch and repository

Handoff branch: `feat/openxr-phase1` on `https://github.com/icosa-mirror/BenchUp3DEditor.git`.
The existing `upstream` points to `https://github.com/BenchUp3D/editor.git`; this work
is being pushed to the fork, not upstream. Read `AGENTS.md` before making changes.
The initial plan/panel milestone is commit `e21b8c9`.

## Implemented

- Root `OPENXR_PLAN.md`: architecture, phased deliverables and acceptance gates.
- Native `crates/benchup-app/src/xr.rs`: physical panel dimensions and validated
  controller-ray-to-logical-UI mapping, with four regression tests.
- Optional native-only `openxr` Cargo feature, pinned openxr 0.22.0, dynamically
  loaded; updated `Cargo.lock`. Default startup does not load OpenXR.
- `src/xr/runtime.rs`: explicit runtime preflight for loader, required Vulkan
  extension, runtime/headset identity, tracking, stereo views and Vulkan API range.
- `examples/openxr_diagnostics.rs`: terminal command with nonzero failure status
  and actionable diagnostics. No session, GPU device or swapchain is created.

No headset mode is functional yet. No new UI settings have been introduced.
The rendering host, runtime-selected Vulkan device, swapchain interoperability,
frame loop, egui input integration and persistent XR controls remain to implement.

## Validation already completed on macOS

`cargo test -p benchup-app --features openxr --lib xr::tests` passed all four tests,
compiling the optional runtime module as well. Cargo metadata confirms the native
dependency is optional and the diagnostic example is feature-gated. Earlier missing
cached dependencies were resolved by fetching crates; that is no longer a blocker.
No Windows headset or GPU interoperability results have been obtained.
The diagnostic example also compiled and ran on macOS, returning exit status 1
with the expected actionable missing-loader error. This validates that failure
path, not successful initialization on a headset runtime.

## First actions on Windows

1. Inspect the checkout and preserve any local changes before switching branches.
2. Read `AGENTS.md`, `OPENXR_PLAN.md`, this file and the XR source.
3. Verify Rust/MSVC build prerequisites, SteamVR, the active OpenXR runtime and the
   OpenXR loader. Do not silently install software or change system runtime choices.
   Ask the user to fix missing prerequisites rather than devising long workarounds.
4. Run from a terminal:

   ```powershell
   cargo run -p benchup-app --locked --features openxr --example openxr_diagnostics
   ```

   A conformant `openxr_loader.dll` must be discoverable by the executable. Start
   SteamVR and connect/wake the headset. Keep the diagnostic output as evidence.
5. Fix any platform-specific preflight failures, then implement phase 1B's graphics
   prototype: runtime-selected Vulkan adapter/device, wgpu interoperability with
   runtime-owned images, session lifecycle/frame pacing and a colored quad layer.
   Validate ownership, layout transitions and synchronization on the actual GPU.
6. Once the quad works, continue phase 1C: host the existing editor offscreen and
   submit it as a panel. Reuse `renderer::install_resources` and retain GPU rendering.

## Constraints and completion gates

- Keep desktop and WASM behavior intact and the XR path optional.
- Successful diagnostics do not establish rendering support. Do not report phase 1
  complete until the editor is usable in the headset, including editing and saving.
- Preserve a single document and undo history. Do not CPU-readback the viewport to
  get it into XR. Phase 2 must use explicit eye poses/projections and shared GPU data.
- Persist all future user-facing controls through the shared preference machinery;
  respect the existing modal/table conventions and English UI requirement.
- Validate large scenes such as Rooftop on actual hardware.
- Commit coherent milestones with messages explaining why. This is development,
  not deployment: do not create release tags or change the release version.
- At the end of each change follow the existing-process/compiler-log check in
  `AGENTS.md`; report when no existing app/log is available.
