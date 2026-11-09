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

### Normal desktop lifetime and manual exit

The interactive `openxr_editor` entry point now opens an empty native desktop
window titled "BenchUp VR — close this window to exit". It appears in the taskbar
and accepts normal Close/Alt+F4 actions. Its default launch has no deadline;
`--seconds N` remains an explicit diagnostic option only. Do not use that option
for normal interactive user previews.

The desktop event loop stays on the main thread while the XR session runs on its
own thread, so a headset/runtime frame wait cannot freeze desktop Close. Closing
removes the window and signals orderly session shutdown. Worker completion/errors
close the desktop loop too. An eight-second shutdown guard handles a stalled
runtime/driver; this is not a session/test deadline. Desktop mirroring is deferred.

An isolated Windows lifecycle check using the exact desktop module and a simulated
XR worker confirmed a visible native window, Close delivery, worker notification
and clean exit code 0. The native OpenXR editor build also passes. Live OpenXR
teardown remains to be checked in the next interactive launch.

The desktop window also forwards keyboard/text/IME and modifier events through
egui-winit to the XR editor's shared context. Focus this desktop window to type
into a field selected on the headset panel or use physical-keyboard shortcuts.
Platform output is returned to the native thread for clipboard/IME handling;
the Windows arboard backend is enabled. No desktop mouse coordinates are routed
into the headset panel. Native-window focus loss releases held keys/modifiers
without changing controller focus; inactive XR frames discard queued keyboard
input. The original lifetime check predates this bridge, and physical typing,
clipboard and IME behavior still require an interactive check.

Cancellation now reaches unfinished world tools even on skipped render frames
with no panel events. This prevents a pending stroke or grab from being committed
as an apparent release when XR tracking/focus returns.

The panel editor's Close button and File > Quit now forward the root viewport's
Close command to the native thread and share desktop Close's shutdown path.
Repeated close requests do not reset the shutdown deadline. The native build
passes; live session teardown is still untested. In-panel minimize/maximize
commands and desktop mirroring remain future host work.

### Direct controller-ray drawing and measurements

Scene clicks now honor the active tool instead of switching to Select. Line,
Tape Measure, Protractor and Dimensions accept model-space controller rays, using
the inverse of the same placement/navigation transform as stereo rendering.
Line commits and measurement/dimension commits share the desktop operations,
undo history, guide creation and GPU preview producers. No document camera is
temporarily changed to synthesize desktop pointer events.

The large XR control bar includes Select, Line, Tape, Angle, Dimension, Cancel,
Undo and Redo. Tool selection uses the existing immediately saved preferences.
Trigger picks the next point; the secondary button cancels the unfinished action.
Two-grip navigation reserves the controllers and suspends the tool cursor without
committing a point. Tracking/focus loss retains the existing release-to-rearm
rules. Repeated egui layout passes cannot repeat the same click operation.

Initial world inference supports the hit face's endpoints, midpoints, edges and
cached interior centre, origin/start closure, axis/plane locks, axis alignment,
grid snapping, face hits and a drawing-plane fallback. Boundary work is local to
the hit face, not a scan/project of the complete model. The ray cursor follows
the snapped point, including when drawing on the empty model ground plane.
Dimension placement uses a stable controller-facing plane containing its span;
an invalid/parallel placement ray cannot commit an old offset.

Rectangle, Circle and Polygon also accept controller rays. Pick a first corner
or centre, then the opposite corner or radius. Their initialization and commits
share the desktop Drawing state and document operations, with model-space
degeneracy checks instead of the unrelated desktop camera's pixel threshold.
Surface planes, explicit shape-plane locks and target-driven plane alignment
use the existing geometry routines. Large shape buttons and polygon Sides -/+
controls use the existing immediately persisted tool preferences.

Paint and Erase also accept trigger clicks on the immersive model. Paint reuses
the desktop face/group operation; the large XR controls expose its existing saved
colour and target preferences. Erase uses the same scoped/lock-aware removal and
history operation as desktop selection/sweep erasing, with one target per click.
The shared source now supplies hover to GPU rendering and annotations, preserving
face-only paint hover instead of replacing it with the controller's whole group.
Generated evaluated faces that lack an original document face cannot be painted
directly; those hits report the existing paint instructions. Continuous erase
sweeps and standalone-edge targeting remain future work.

Attached Text accepts an anchor on a hit face followed by a label position in
model space. Placement uses a controller-facing plane captured at the first
click, with geometry/axis/grid snapping, the shared transient leader preview and
the existing text dialog/commit/undo path. Enter or accept the contents on the
editor panel; no XR virtual keyboard is implemented. Editing an existing label
by directly ray-picking its billboard remains future work. The shared grid helper
accepts an explicit working-plane normal for XR while keeping desktop defaults.

Freehand uses trigger-hold sampling and a tracked release to commit through the
shared drawing operation as one undo action. Samples use angular spacing in
model space. The active stroke is capped at 2048 samples; reaching the cap finishes
it and requires a trigger release before another starts. Secondary Cancel,
focus/tracking loss, panel capture or grip navigation discards unfinished samples
without changing the document. Repeated layout passes cannot sample or commit
twice. This controller behaviour still awaits headset validation.

Move and Push/Pull use trigger grabs. The picked anchor stays at its initial
distance along the controller aim ray; controller translation and ray rotation
move that anchor, or project its displacement onto the face's extrusion direction.
This also works when the ray is parallel to that direction. Start/preview/commit
share the desktop Modeling state, MovePreview plan and document/history operations.
Existing explicit axis/plane locks and grid snapping apply. The large XR controls
expose Move, Push/Pull and the already saved Keep starting face option.

Tracked release commits once. Cancel, loss of pointer ownership/focus/tracking or
grip navigation discards the uncommitted preview. Zero-distance release does not
create an edit. Locked or generated-only targets cannot start a grab. The beam
follows the modeling preview marker even when the ray no longer hits geometry.
Scale and Offset use the same trigger-grab lifecycle: Scale compares the grabbed point's radius
with its original radius about the selection centre (respecting axis/plane
constraints); Offset uses the same nearest-boundary direction as desktop. Scale
is positive and bounded, and a reference on a locked axis cancels with a message.
Grid snapping uses radial distance for Scale and inset distance for Offset.
Their previews, authoritative operations and undo commits are shared with desktop.
Rotate also uses a trigger grab around the selection centre: the initial picked
point supplies the reference and the controller supplies its new direction.
It shares desktop's default/locked rotation normal, angle calculation, arc
preview and commit operation. References on the rotation axis are rejected.
World-driven Modeling previews ignore desktop pointer refreshes, including those
requested by keyboard axis or modifier changes. Geometry/reference snapping
during grabs and standalone-edge grabs remain future work. Controller behavior
awaits headset validation.

Copied geometry can be placed with the controller ray. Large Copy/Paste buttons
use the existing clipboard and paste setup; keyboard/menu paste enters the same
world placement path. The retained GPU ghost follows geometry or the drawing
plane with grid snapping. Trigger commits through the shared paste/history
operation, and Cancel discards the ghost. This reuses the immutable paste model
and updates its transform rather than cloning geometry each frame. XR controls
hide while shared modal dialogs are open, so text/name dialogs stay accessible.

The controls are now a docked bottom panel that reserves space for the shared
editor and inspector instead of covering them with a foreground overlay. One
large tool picker opens a four-column palette; Copy/Paste/Cancel/Undo/Redo remain
visible, and extra parameters appear for the active tool only. Tool selection and
parameters keep the same persisted preferences; no separate palette/category
setting was added. The native build and hardware-GPU panel snapshot pass, with
the viewport/sidebar visibly laid out above the controls. Headset testing remains
deferred until the user is free.

Outer Shell, Union, Intersect, Trim, Split and Flip accept controller clicks.
Large tool buttons reuse desktop solid-tool setup and saved active-tool choice;
world hits enter the shared operand validation and boolean/history operations.
Trim preserves cutter-then-target order. Flip clicks query the same three quads
as the shared GPU overlays and apply the existing flip operation. Quad queries
run only on a click, not as a per-frame scan of selected geometry; plane hover
feedback remains to be added. Cancel clears pending operands and paste previews,
and cancelling a paste keeps the original selection. Headset tests are pending.

Grain Direction uses two controller picks on an unlocked group/component and a
direction endpoint. It shares desktop's anchor validation, multi-part targets,
linked-instance reference frames, retained GPU preview and atomic document/undo
assignment. The first point remains on hit geometry with snapping disabled, and
the second supports face-plane/axis/grid inference. A changed selection cancels
the pending assignment before it can edit stale targets. The tool is included
in the same persisted picker. Headset interaction remains untested.

Standalone edges now participate in controller picking through a retained,
model-space spatial hierarchy. Angular hit tolerance stays consistent under
scene scaling. Queries apply editing scope, hidden state, section clipping and
exact-point face occlusion, then fall back to face hits. Select/Erase/transforms
can target loose lines; drawing and measurements get visible endpoint/midpoint
and on-edge targets. Paint/PushPull/Offset retain face-only targeting.

The index is prepared lazily on a world query and reused by model Arc identity;
head motion, navigation and UI-only work do not rebuild it. Temporary modeling
previews skip edge preparation/query and use the existing grabbed anchor. Snap
targets are local to the picked edge/face, with no full-model CPU projection.
Four isolated checks against the exact index source pass (cone bounds,
sloping/crossing angular minima, clipping/occlusion filtering, and hierarchy
traversal). The native build passes; live edge picking still awaits the headset.

Section handles now accept controller trigger grabs. They reuse the desktop
SectionDrag state, shared handle anchors/highlighting and section-offset setter,
with model-space controller movement constrained to the chosen axis. A Sections
menu exposes the existing view controls. Each change persists through the shared
preference path; tracked release keeps the value. Cancel, grip navigation,
focus/tracking loss or changing tools restores the starting offset. Desktop
pointer events do not update a world-driven section drag.

The last user-launched executable was started from the frozen ignored
`.local/openxr/preview-builds/section-preview/` directory after the user requested
it. It has no session deadline. Startup logs confirm the native window, keyboard
bridge, focused OpenXR session, stereo and editor-quad frame submission, and
controller grip events. New modeling and section behavior await user feedback;
The user subsequently closed it; check current process ownership before any launch.

Construction guides now participate in controller selection, erasure and
transforms, and drawing/measurement snapping. A separate retained spatial index
uses the exact finite line spans drawn in XR and includes point guides. It is
rebuilt only when the model or fitted span changes. Guide visibility and editing
scope are respected; guides remain overlays through surfaces and section cuts,
matching their display. Geometry edge targets take precedence over guides.
Constrained snapping intersects the picked guide with the active axis/plane;
finite display ends do not become false endpoint targets. Attachment tools and
face/solid operations retain geometry targets. Five isolated spatial-index checks
pass, including guide points and span changes. Live interaction remains untested.

Remaining: guide-guide intersection inference and off-face
nearby geometry; desktop reference-acquisition parity; controller paths for
direct annotation picking and flip-plane hover feedback. Legacy desktop-only navigation modes
remain available through the panel and no longer silently switch to Select.
The native OpenXR example build passes. Desktop-panel and immersive-eye startup
snapshots pass on the RTX 4090 Vulkan backend without starting an OpenXR session;
the large control bar was visually checked for fit/readability. Controller-driven
editing still awaits its first headset check; broad suites remain deferred.

Suggested next headset sequence: select Line in the large control bar, click two
points on the scene or ground, Cancel, Undo and Redo; use Tape for two points,
Angle for three and Dimension for two endpoints plus an annotation position.
Also check that both grips still scale/translate/yaw with Keep upright enabled.

### Two-grip scene navigation

The user requested Open Brush-style navigation: hold both grip buttons to scale,
rotate and translate the scene. Grip poses and squeeze actions are bound for
Touch, Vive and Index controllers (simple controllers have no squeeze binding).
Both grips establish a two-hand frame; subsequent midpoint, span and frame changes
apply a uniform similarity transform to the XR scene root. Rendering, annotations,
environment and scene-ray picking use that same transform. The editor quad stays
in place, and document/model coordinates and undo history are unchanged.

The user confirmed two-grip navigation worked, then requested an upright lock.
Keep upright now defaults on, limiting grab rotation to yaw around tracking Y;
vertical translation and full hand-distance scaling remain available. The saved
XR checkbox can restore free rotation. Older preference data defaults to upright,
and checkbox changes persist immediately without model edits. Enabling upright
on a tilted scene levels it around the scene center, preserving that point.
Yaw freezes when hand separation becomes nearly vertical and rebases on recovery.

Release either grip to leave the scene in place. Any held grip reserves pointing
input, cancels captured UI/scene clicks and disarms held triggers until released.
Focus/tracking loss or nearly coincident hands freezes navigation and requires
both grips to be released before another grab. Grip press/release thresholds have
hysteresis. A continuous hand-frame up direction avoids wrist-cancellation flips.
Scale is bounded, and Fit 3D resets the transient grab transform and saved 3D zoom.
The HUD describes the current grab/release state. Tracking-space navigation is
session-local; physical room poses are not written into saved document/settings.

The executable builds and the offscreen eye-render startup passes. The original
live grip run records grab/release and scale transitions, matching the user's
confirmation. Three isolated checks against the actual navigation module pass:
tilt rejection with vertical movement, yaw/scaling with a fixed grab point, and
leveling without a pivot jump. The new upright behavior awaits a headset check;
broad backend/persistence suites remain deferred.

### Visual parity implementation

The user explicitly asked to continue until a useful parity test is ready, rather
than stop after each milestone. Keep committing implementation milestones and
continue independently; do not end the work merely to report a completed commit.

The immersive renderer now uses the shared effective display settings instead of
forcing an opaque materials-only view. The retained GPU path supports face styles,
X-Ray, hidden geometry/objects, profile/depth/endpoint/extension cues, fog and the
existing cached shadow atlas. Shared section-cache cap triangles and contours are
rendered in both eyes; cap visibility participates in edge and annotation masking.
Sky, ground, horizon and sun are GPU-rendered from saved environment settings.

Group text uses GPU face/depth visibility and retained sorted owner-slot lists:
own-group faces do not hide their names, while intervening groups clip the glyphs
and background. Other annotations retain surface-depth occlusion. This replaces
the initial naive depth-only text path. Theme selection is explicitly synchronized
with the shared editor context, including its first frame.

`openxr_editor --immersive-snapshot output.png` exercises the actual eye renderer
using a synthetic eye derived from the saved document camera, on the hardware GPU
without an OpenXR session. This is for narrow functional iteration; it is not a
substitute for the final headset visual comparison. Broad validation and profiling
remain deferred.

The next user headset check is gated on visual equivalence to the desktop model
viewport. Do not request incremental headset tests. Validation suites, backend
benchmarking and Vulkan validation remain deferred while functionality advances.

World text now uses retained glyph buffers and the editor's shared font atlas:
saved text entities and leaders, group names, saved dimension values and extension
lines. Units, evaluated model coordinates, visibility settings and selection colors
come from the existing model/editor state. Both eyes share one head-facing layout;
projection and readable angular sizing run on the GPU. Glyphs are staged before
the panel atlas upload, and reshaped only when content/settings/font scale change.

This is not yet a claim of complete visual equivalence. No new headset visual
confirmation has been requested for these overlays.

Construction guide lines and points, active edit-context bounds and dimension end
ticks now use retained GPU overlay instances. Dash spacing, point-marker sizing
and projected tick direction are shader operations, shared by both eye draws.
Axes use the existing saved axis colors; the immersive palette follows the editor
theme. The floor grid now uses an analytic GPU ray/plane lattice, with saved
finite/infinite extent, spacing, major/minor colors, distance fade and adaptive
distant spacing. It replaces the initial bounded fitted-scene grid mesh.

Live tool overlays now have model-space descriptors in `viewport_overlays.rs`.
Desktop and XR share the drawing-preview segment producer. XR also consumes
current inference/lock labels, drawing axes/projections, tape/protractor arcs and
values, dimension-placement previews, and modeling directions/value/error labels.
The descriptors use existing precision, measurement and snap state; they do not
implement a separate model-operation pipeline. Tool line/marker buffers are
retained separately from the grid and saved-reference buffer. World labels are
reshaped only when the corresponding content changes.

The retained billboard path now also draws vector glyphs, including inference
endpoint/midpoint shapes and the shared joint edit/warning icon outlines.
Joinery cut guides preserve owner-aware visibility in the GPU face/depth mask.
Fastener insertion axes, head/interface markers, arrowheads and invalid-reference
labels use existing connection evaluation, cached by source/settings changes;
head motion does not repeat connection evaluation. Painter-style hints retain
their overlay behavior, while depth-tested model annotations retain occlusion.
Active tool, eraser preview, outliner hover and joinery member coloring are shared
with the editor rather than synthesized independently by XR.

Controller scene clicks still select entities; world-ray drawing/manipulation is
not yet wired. Current transient overlays reflect operations performed through the
editor panel. Rotation graphics now share world-space arc/reference geometry with
desktop. Modeling confirmation/value/error messages and transient text annotations
also share their original producers. Incomplete measurement snap markers and hints
use the retained billboard path. Grain now uses GPU dash/end-grain patterns with
the existing physical spacing, deterministic jitter and face-interior anchors.
Compact per-object axis textures support transformed instances and live previews;
preview motion does not rebuild face meshes or CPU mark lists. The desktop path
shares the same anchor producer. An isolated assigned-grain eye snapshot renders
the patterns on the RTX 4090. Paste previews now use a separate retained model
layer, initialized once per preview and moved by a transform uniform. Copied text
and dimensions use retained annotation buffers with the existing green preview
colors. Empty-scene paste can establish the initial scene placement. Section
limits are translated into the preview's local coordinates.

Annotations render after the geometry pass and sample the final scene depth in
addition to the owner-aware face mask. This accounts for temporary preview geometry
without breaking own-group label visibility or painter-style overlay hints. The
hardware eye-render startup passes with the final-depth pass; interactive paste
and headset comparison remain unconfirmed, with broad validation still deferred.
The model-side HUD shares the existing status producer, including tool instructions,
modeling confirmation/errors, temporary notices and suspended-joint warnings.
Section-plane handles share model-space anchors with desktop; their direction
arrows rotate on the GPU using the common head basis. Single-axis and combined
inference guides share their original producer. Modeling anchor markers, locked
colors, selected group-label appearance, outliner annotation hover and history
preview selection behavior now follow the desktop state. Text +/- also scales
world labels/HUD through the existing saved XR text preference. Overlay strokes
use antialiased premultiplied coverage. Section caps/contours and saved labels
render in an isolated hardware eye snapshot without a headset session.

The duplicated 2D viewport
remains for comparison and existing tool interaction while these gaps are closed.

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

An empty project now initializes a one-to-one model-space XR workspace instead
of waiting for geometry bounds. Fit 3D uses the same fallback for an empty or
zero-extent project, so the first controller drawing ray can reach its drawing
plane. Nonzero scene bounds retain the existing fit behavior. The native OpenXR
editor build passes; first-stroke headset confirmation remains deferred.

The XR Tape Measure and Protractor controls now expose Create construction
guides directly, using the existing saved preference and immediate persistence.
Their live construction-line preview is submitted as a small GPU-line overlay
using the same model-space span as saved guides. It follows the shared
measurement/precision state and respects guide visibility and history preview.
The existing preference regression coverage already exercises both guide-mode
values and restoration; no new preference schema or defaults were introduced.
The guide-workflow native OpenXR editor build passes. Headset confirmation of
these controls and previews remains pending; broad validation stays deferred.
