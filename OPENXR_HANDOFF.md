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

### Current work: immersive visual parity

The next user headset check is gated on visual equivalence to the desktop model
viewport. Do not request incremental headset tests. Validation suites, backend
benchmarking and Vulkan validation remain deferred while functionality advances.

World text now uses retained glyph buffers and the editor's shared font atlas:
saved text entities and leaders, group names, saved dimension values and extension
lines. Units, evaluated model coordinates, visibility settings and selection colors
come from the existing model/editor state. Both eyes share one head-facing layout;
projection and readable angular sizing run on the GPU. Glyphs are staged before
the panel atlas upload, and reshaped only when content/settings/font scale change.

This is not a claim of visual equivalence: exact owner-aware label occlusion,
inference glyphs, tool previews, sections, specialized display
modes and joinery/fastener feedback still need completion. No new headset visual
confirmation has been requested for these overlays.

Construction guide lines and points, active edit-context bounds and dimension end
ticks now use retained GPU overlay instances. Dash spacing, point-marker sizing
and projected tick direction are shader operations, shared by both eye draws.
Axes use the existing saved axis colors; the immersive palette follows the editor
theme. Guide lines are currently bounded to the fitted scene neighbourhood, and
guide points use a screen-facing cross. Exact infinite-grid/guide clipping and
owner-aware annotation occlusion remain parity work.

Live tool overlays now have model-space descriptors in `viewport_overlays.rs`.
Desktop and XR share the drawing-preview segment producer. XR also consumes
current inference/lock labels, drawing axes/projections, tape/protractor arcs and
values, dimension-placement previews, and modeling directions/value/error labels.
The descriptors use existing precision, measurement and snap state; they do not
implement a separate model-operation pipeline. Tool line/marker buffers are
retained separately from the grid and saved-reference buffer. World labels are
reshaped only when the corresponding content changes.

Controller scene clicks still select entities; world-ray drawing/manipulation is
not yet wired. Current transient overlays reflect operations performed through the
editor panel. Exact inference glyph shapes, rotation handles/arcs, section planes
and caps, joinery/fastener markers, display-mode parity, label layout/ownership and
direct tool interaction remain open. The duplicated 2D viewport remains until
these parity gaps are closed.

- Root `OPENXR_PLAN.md`: architecture, phased deliverables and acceptance gates.
- Native `crates/benchup-app/src/xr.rs`: physical panel dimensions and validated
  controller-ray-to-logical-UI mapping, with four regression tests.
- Optional native-only `openxr` Cargo feature, pinned openxr 0.22.0, dynamically
  loaded; updated `Cargo.lock`. Default startup does not load OpenXR.
- `src/xr/runtime.rs`: explicit runtime preflight for loader, required Vulkan
  extension, runtime/headset identity, tracking, stereo views and Vulkan API range.
- `examples/openxr_diagnostics.rs`: terminal command with nonzero failure status
  and actionable diagnostics. No session, GPU device or swapchain is created.

At the original macOS handoff no headset mode was functional. The Windows
continuation below supersedes that implementation status. No XR UI settings
have been introduced.

## Windows continuation, 2026-11-09

The preceding implemented/validation sections describe the original macOS
handoff. Windows preflight now passes using SteamVR's installed loader through
the process-scoped `scripts/openxr_probe.ps1` helper. See `OPENXR_WINDOWS.md` for
commands, machine evidence, validation setup, and current limits.

`openxr_panel` now implements the runtime-selected Vulkan/wgpu bridge, borrowed
swapchain images, an orientation-pattern quad, frame pacing, and session shutdown.
Successive unattended runs submitted quad frames on the RTX 4090 and stopped
cleanly. A later active-headset editor run reached VISIBLE/FOCUSED and submitted
4,094 editor quad frames before a clean stop. The user confirmed that the editor
was visible, readable, upright, and stable in the world during head movement.
A second session reached VISIBLE, submitted 4,258 editor frames and stopped
cleanly without restarting SteamVR.
Khronos validation still reports runtime-side shader/image/copy errors, including
the runtime's BlankEyeBuffer; the clean-validation gate has not passed. Editor
hosting is now implemented as a render-only preview using the shared editor UI
and GPU viewport callbacks. An independent hardware Vulkan snapshot of the
bundled Example was inspected on the RTX 4090. Full-editor headset submission
is now verified after the user reconnected the headset.
The same SPIR-V layout errors also
reproduce without an XR session and match wgpu/Naga issue #7696. Controller input
is now implemented with GPU cursor rendering, clicks, scrolling, capture across
panel edges and focus/tracking cancellation. Eight focused XR tests pass; the user
confirmed live cursor and primary clicks using the runtime's Oculus Touch
profile, plus wheel input affecting the main viewport. The run stopped cleanly
after 12,191 frames. A drag test was not completed: the user found the text too
small and could not bring the fixed-distance panel closer. Persistent panel
distance, width, UI scale and recenter controls are now implemented; the user
confirmed they are readable and work. Ten focused checks cover XR behavior and
preference defaults/immediate-save/restoration. A restart stopped cleanly after
582 frames and retained the chosen settings on disk; human visual confirmation
of that restart is still pending. Keyboard/text entry, desktop
mirroring, native window services and persistent XR controls remain.

## Immersive 3D continuation

World input/overlay foundation: controller rays now reuse model-space picking;
selection is shared with the inspector/document selection, and stereo hover,
GPU axes/grid references and hit rays/markers are implemented. Panel capture and
modal dialogs take precedence. The native build and ten-second startup passed
(86 frames, clean stop); live interaction/overlay confirmation is deferred.
Next functionality is world labels/dimensions, then further previews/handles.

Stereo decision: retain the current two-eye-pass renderer while implementing
functionality. The user agreed to defer multiview/single-pass instancing. Keep
shared geometry and view-neutral overlay data; future work is frame submission
batching, later timing-based prioritization, then layered resources/view-indexed
shaders and a two-pass fallback. See the decision section in OPENXR_PLAN.md.

The user has clarified the target: the immersive view must replace the 2D model
viewport, including spatial hints, overlays and text labels, rather than remain a
secondary inspection view. `OPENXR_PLAN.md` now inventories the existing paths
and orders the shared world-overlay/input work. Keep tool/document logic shared;
world text, guides, previews and handles need GPU rendering and controller-ray
interaction. Inspectors/toolbars/dialogs may remain floating panels. Further
validation is still deferred; prioritize functionality.

The user asked to prioritize 3D rendering and defer validation. A native
stereoscopic projection layer is now implemented alongside the editor quad.
Both eyes use runtime poses/FOVs and share retained GPU world geometry from the
editor's evaluated render-model snapshot. A separate navigation transform handles
Z-up/Y-up conversion, fitting and scale; it does not modify model coordinates or
the document camera. The initial opaque materials/edges view appears to the right
of the panel. Annotation/special display previews remain on the editor panel.

The XR bar has persistent 3D-view/scale options and a Fit 3D action. Native build
and ten-second runtime startup succeeded: 404 faces were fitted, stereo frames
were submitted and the session stopped cleanly after 650 frames. The user's
preview exited successfully after 16,257 editor frames through STOPPING/IDLE/
EXITING; stereo scene submission was active at startup. Human visual confirmation
of the 3D scene is not yet recorded. No additional test suites or validation-layer
runs were performed, as requested. Logs are `.local/openxr/immersive-build.log`,
`immersive-startup.log` and `immersive-live.log`.

## Historical macOS validation

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
