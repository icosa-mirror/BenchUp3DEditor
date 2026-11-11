# BenchUp OpenXR development handoff

Updated: 2026-11-11. Status follows the current `feat/openxr-phase1` worktree.
Read [AGENTS.md](AGENTS.md) and [OPENXR_PLAN.md](OPENXR_PLAN.md) before continuing.
The plan is the current scope/status/backlog; this file is the operational handoff.
Git history retains the earlier milestones and historical handoff revisions.

## Active continuation; user busy

The user resumed the full goal on 2026-11-11 and asked for independent development
until they return. Do not replace the running preview or keep asking for tests.
The current tested executable is `dfab0e6`, preserved in
`.local/openxr/preview-builds/dfab0e6/openxr_editor.exe`; the manifest is
`.local/openxr/keyboard-focus-fix/build.json`. The clean-example launch on
2026-11-11 used session `20261111-091524-788-54dc645e`, PID 77372 at launch, and
reached FOCUSED with stereo and quad frames. Inspect liveness/ownership before
any later requested replacement; never assume that PID still exists.

Recorded human feedback:

1. `ceae626`: no label holes or flicker; Save As chooser appears in the headset.
2. `ceae626`: keyboard focus failed on each key click. This prompted the scoped
   focus-policy guard in `dfab0e6`.
3. `dfab0e6`: the user confirms continuous File-box typing now works without
   clicking back into the field.
4. Exact value movement/undo remains pending. The last question was whether the
   user could enable Exact value, but they became busy before answering. Do not
   mark that flow accepted or failed.
5. Label occlusion is an open design decision: the user wants discussion of full
   hiding versus translucency behind geometry. Do not change that policy before
   discussion. Ask verification questions one at a time when testing resumes.

Section-cap ownership and controller picking landed in `92b7f8a`. Axis/plane
locks and host-aware shared precision input landed in `1de74a5`. Destination-folder
creation landed in `25b514f`; shared immersive View controls landed in `25fd106`.
Bounded captured erase-stroke interpolation landed in `6ca8676`; shared stale-input
guards landed in `466b9f4`; controller joint icons landed in `d7c05f3`. Current work
has completed C13, controller enclosing-window selection, and C14's direct
selection/context gestures. C15 is the next inference/reference audit. The plan now has stable coding
milestone IDs and checkboxes, separate from verification. Do not derive an effort
percentage by counting unequal checklist rows; keep audit-discovered work visible.
Generated caps remain read-only surfaces with optional editable group ownership.
No full phase or visual acceptance has been recorded.

The XR tool controls now expose X, Y, Z and Unlock for the same tools as the
desktop lock toolbar. Selecting two axes locks their plane; selecting an active
axis removes it, and a third axis is disabled. These use the existing persisted
tool preferences and reset-after-action preference. Immersive lock refresh clears
the old snap for the next controller ray instead of projecting panel pixels
through the desktop camera. Shared line/shape/modeling Enter paths also honor
the frame's world-input host. Panel-only and desktop input retain their routes.

The focused `axis-lock-tests.log` passes both real-button preference restoration
and constrained world-target/legacy-Enter cases. `axis-lock-precision-regressions.log`
passes five cases (including those two); `axis-lock-desktop-regressions.log`
passes eleven shared desktop lock cases. Human controller-lock acceptance remains
pending, alongside the previously pending Exact value workflow.
The native example build passes without warnings in `axis-lock-build.log`.
`axis-lock-controls.png` was rendered on RTX 4090 Vulkan and inspected: the new
large lock buttons fit alongside the existing controls, with the inspector and
shared value field retained. This offscreen check started no OpenXR session.
The executable is frozen under `1de74a5` in
`.local/openxr/preview-builds/`; it has not replaced the user's last preview.

The in-panel file chooser now has New folder. Its shared modal focuses a text
field usable by the controller keyboard; Cancel returns to the chooser. Create
runs on a worker, reports failures in place, and navigates into the created
directory while retaining the save filename. Directory creation rejects paths,
reserved characters and dot names, and never overwrites an existing directory.
The existing directory preference updates after a successful listing; no new
preference or model edit is introduced. Controller-only create/save/reopen
acceptance remains pending.
`new-folder-tests.log` passes all five chooser cases, including asynchronous
folder navigation with the save filename retained, rejection of invalid names
and existing folders without overwriting contents, existing overwrite-confirmation
behavior, filtering/directory preferences and keyboard-safe chooser footer layout.
`axis-lock-final-regressions.log` also passes both XR lock cases against the
current source, including the final egui ID constructor update.
`new-folder-build.log` passes the native example build without warnings. The
combined executable is frozen under `25b514f` in
`.local/openxr/preview-builds/`. No replacement headset session was launched.

Shared View controls now issue XR placement commands while immersive mode is
active. The Camera section describes headset-controlled projection/FOV and offers
Fit model, Fit selection, Center origin and Reset view. The seven existing
standard-view cards turn the scene toward the user, with larger captions in XR;
their desktop selection behavior remains unchanged. Existing section keys and
preference restoration are retained. These are transient placement actions like
two-grip navigation, not new saved option states.

Standard directions preserve the target's physical position and size, including
after two-grip navigation, and use the desktop camera's analytic right/up basis
for stable Top and Bottom directions. Fit model/selection retain world orientation;
Center origin retains physical scale and orientation; Reset view restores the
upright initial orientation. All routes leave document geometry, undo history and
the saved desktop camera unchanged. Shared fit commands also request XR framing
when called by menus/keyboard/project consumers during immersive UI frames.
`world-view-tests.log` passes three focused cases: actual XR View controls queue
commands without modifying the desktop camera or project; all seven standard
directions match desktop right/up bases and remain invertible for picking; and
framing preserves orientation/scale/target through navigation, with an upright
reset matching the original placement. `world-view-desktop-regression.log` passes
the existing shared projection/all-standard-views/no-resize UI case. Live
controller acceptance remains pending.
`world-view-build.log` passes the native example build without warnings.
`world-view-eye.png` was rendered on RTX 4090 Vulkan without an OpenXR session and
inspected. Compared with the prior checked `section-cap-eye.png`, the 1440 x 900
default-view image differs in only two pixels, by at most one color level. This
checks default rendering, not live standard-view/controller acceptance. The
combined executable is preserved under `25fd106` in
`.local/openxr/preview-builds/`.

The held-trigger eraser now samples the interval between consecutive tracked rays
using the existing retained scene picker, including its face/edge/guide/cap
visibility checks. Sampling handles controller rotation and translation, with
0.008-radian target spacing and at most 63 extra queries per interval. The release
interval is included; all targets join the existing one-operation/one-undo stroke.
Invalid or locked document IDs are rejected before they join the transaction.
The pending interval carries the validated document revision and editing context;
the whole collected stroke cancels if the document or context changes before release.
The collector reuses the existing snapshot revision without another scene scan.

Interpolation requires the same controller, model snapshot/revision, display and
model-to-tracking transform. Loss, cancellation, a gap over 150 ms, more than
45 degrees of direction change or 0.5 metres of origin travel drops the interval
and requires a fresh press to rearm interpolation. Existing point-per-frame input
and capture semantics remain in place. Capture loss, owner changes and cancellation
clear the pending erase on input updates even if XR skips rendering the UI; normal
trigger release is retained as a commit. This bounded sampler can still miss very
thin/near targets during an interval that reaches the sample cap; controller
acceptance and large-scene costs remain unverified. No new preference is added.

`erase-sweep-tests.log` passes eight focused cases: retained thin-edge queries
between missed endpoints for translation/rotation and release, visibility rejection,
all interpolation reset conditions, swept-target release/undo, document-change
cancellation, capture-loss handling without a rendered UI frame, existing held-erase/
loss/cancel behavior and the desktop stroke's intermediate-motion regression.
These are software checks, not headset acceptance.
`erase-sweep-input-regressions.log` passes the three existing pointer-capture,
focus-loss and held-button rearm cases.
`erase-sweep-build.log` passes the final native example build without warnings.
The combined executable is frozen under `6ca8676` in
`.local/openxr/preview-builds/`; no replacement headset session was launched.

Native world pointers now carry the committed-document revision, project generation,
editing context, tool and display settings used to pick them. Every world-tool
callback validates that stamp before processing the ray; a mismatch follows the
existing lost-pointer cleanup instead of applying the old target to changed state.
Production pointers without a stamp are rejected. Direct model-space unit fixtures
may omit it, while the new guard cases use real scene-source stamps.

Project replacement increments a native XR generation even when the loaded model
is identical; the erase interpolator checks that generation too. The guard uses
the cached committed-model revision rather than the evaluated preview revision,
so continuous modeling previews and ordinary selection remain usable. Snapshot
capture validates committed data when needed, and normal UI-frame comparisons
reuse that cached revision. No user preference or serialized project field is added.

`input-revision-tests.log` passes three cases covering stale first clicks after
model/context/tool/display changes and identical-project reload, rejection before
an old erase can commit, fresh-input recovery, and valid selection/line/continuous
Move preview input. `input-revision-source-regressions.log` passes two retained
snapshot/context cases. `input-revision-erase-regressions.log` passes all eight
erase cases, including project-generation reset. The native example build passes
without warnings in `input-revision-build.log`. The executable is preserved under
`466b9f4` in `.local/openxr/preview-builds/`; no headset session was
launched. Full live transition/capture acceptance remains pending.

Joint edit/warning icons now have typed controller hit targets alongside their
existing GPU billboards. Cached reference overlays own their model-space anchors,
28-point rectangles and owner sets; a retained spatial index uses the shared
billboard basis, distance sizing and UI scale. Same-depth overlaps follow draw
order, and action metadata updates even when rectangle geometry is unchanged.
Face/cap hit visibility follows the current first-surface owner rule; no text
shader or label occlusion policy was changed.

An idle primary click opens the existing joint editor through the desktop marker
operation, including nested context navigation, pair selection and lock/sharing
guards. Inspector preference changes flush immediately. Hover supplies a world
instruction hint. Typed handles do not become document faces or erase targets;
active captured gestures retain their release transaction when crossing an icon.
Automatic Fasteners is intentionally disabled by `fasteners::AVAILABLE = false`.
Keep that existing gate. Its entry-face XR route is conditional future work;
controller window selection now uses shared enclosing-window semantics.

Controller Select has a persisted Window selection toggle (default off). Hold
the trigger to place and drag a world-fixed window, then release to select fully
enclosed entities/groups; Add/remove selection toggles the result. Only four GPU
line segments update during dragging. The shared core selection query runs once
on release, through a tracked-head projection without changing the document
camera. Label bounds use the current head basis/scale; infinite guides are
clipped against the window portal. A tiny gesture retains ordinary point picking.
Cancel/Escape, missing input, focus/tracking/grip capture loss, tool/mode changes,
document/project/context changes and scene-transform changes discard the pending
window. Cancelling a pending window preserves the previous selection. Headset
comfort, enclosure behavior and cancellation acceptance remain pending.

`window-core-tests.log` passes the nine filtered core selection cases.
`window-selection-tests.log` passes four focused XR cases covering shared group
enclosure/toggle/history behavior, cancellation, tracked projection and infinite
guide clipping. `window-preferences-tests.log` passes immediate preference
saving/restoration and legacy defaults, including the new default-off mode.
The native `openxr_editor` example build passes without warnings in
`window-selection-build.log`. Its executable is frozen under this implementation
commit in `.local/openxr/preview-builds/`; no replacement session was launched.

Controller Select now shares desktop click shortcuts: two trigger clicks on the
same group/component enter it, two on saved text open its shared editor, and an
empty non-additive click leaves one editing level. Tiny clicks in Window selection
mode use that same route; actual enclosure drags do not open objects or close
context. Add/remove selection keeps its toggle semantics. Double-click timing
uses egui's existing input option. Click continuity requires the same committed
document/project/context/tool/display stamp and scene transform, and resets on
pointer loss, keyboard actions, cancellation, focus/capture loss and tool changes.
Object locks/visibility/context restrictions remain in the shared enter action.
The panel's shared Edit menu already exposes the desktop entity actions, and the
immersive panel has Edit group/component and Close context buttons. Keep secondary
trigger as Cancel; no separate world context popup or conflicting button mapping
was introduced. Controller timing and end-to-end nested editing remain unverified.

`selection-context-final-tests.log` passes the two controller selection cases:
group entry/context exit in ordinary and Window modes, saved-text editor opening,
and interruption by missing pointers, project changes, additive selection,
elapsed time and display changes. `selection-context-window-regressions.log`
passes the four enclosure/projection/guide/cancellation cases after this change.
`selection-context-build.log` passes the native editor example build without
warnings. The executable is frozen under this milestone's commit in
`.local/openxr/preview-builds/`. No headset session was launched.

C15 now adds a retained global endpoint/midpoint reference index, rebuilt only
when the evaluated source model changes. Each world-axis query affinely maps
BVH node bounds onto the tool's current working plane/lock before testing leaf
anchors. The native scene callback supplies editing-context, hidden-state,
section and face/cap visibility checks using its existing caches. It returns up
to one reference per axis to the shared drawing/measurement callback. Compatible
reference/start/origin guides combine at a world intersection and use the existing
GPU guide overlays. Exact geometry targets and acquired-reference projections
retain priority. No desktop camera projection or new persistent option is involved.
C15 remains open: audit manipulation inference, geometry-constrained refinements
and cases requiring more than the nearest visible reference for each axis.
Move now shares the immutable stationary geometry from `MoveSnapCache` with its
world target index. Targets exclude the edited geometry and inherited hidden
state, retain editing scope and section clipping, and match desktop Move's
stationary X-ray targeting. Controller Move now supports edge/end/midpoint and
face/centre targets, axis/plane intersections and global reference guides. Shared
grid refinement preserves exact geometry targets; no qualifying snap retains the
existing controller-depth/constraint/grid fallback. The existing model preview,
commit and single-undo routes remain authoritative. Target indexes persist for
the gesture instead of following the changing preview. C15 still includes the
remaining manipulation and geometry-constrained drawing inference audit.

Scale now reuses that stationary target service with visible-target semantics:
opaque faces and generated section caps block hidden edge/reference targets,
unless the existing X-ray preference is enabled. Move keeps its desktop X-ray
target behavior. Scale stores its snapped point and retained guides on the shared
modeling state, applies shared grid refinement to the target, then computes the
world-space ratio from the original centre/reference. An exact target is not
rounded a second time as a grid radius. Free controller-depth scaling retains
its existing radius/grid fallback. Axis/plane locks, exact-value entry and the
existing preview/commit/undo routes remain authoritative. Remaining C15 work
includes Rotate, Push/Pull, Offset and geometry-constrained drawing refinements.

`scale-inference-final-tests.log` passes three focused cases: controller scaling
to an exact target with grid enabled, in free and X-locked modes, with one undo;
stationary opaque-face occlusion/X-ray preference; and filled-section-cap
occlusion/X-ray preference. `scale-inference-final-modeling.log` passes all five
current precise-modeling cases. `scale-inference-move-regressions.log` passes
the three stationary Move target/constraint/guide cases. These are software
checks; controller comfort, dense-scene performance and full visual parity remain
unverified.
`scale-inference-build.log` passes the native editor example build without
warnings. The executable is frozen under this milestone's commit in
`.local/openxr/preview-builds/`; no headset session was launched.

`move-inference-final-tests.log` passes four focused cases: shared stationary
snapshot/exclusion and exact targets, world-space edge/face constraint
intersections, face centres/global-guide toggles, and controller endpoint snapping
with grid enabled followed by a single undoable release commit.
`move-inference-modeling-regressions.log` passes all four current precise-modeling
cases, including exact numeric values for all five tools, default release commit
and cancellation. Controller snapping comfort and broader tool parity remain open.
`move-inference-build.log` passes the native editor example build without
warnings. Its executable is frozen under this milestone's commit in
`.local/openxr/preview-builds/`; no headset session was started.

`global-inference-checked-tests.log` passes all seven focused reference cases,
including retained-versus-exhaustive comparisons on oblique/grazing planes,
endpoint/midpoint eligibility, visibility/lock rejection, combined-guide/skew-line
behavior and the earlier acquired-reference regressions.
`global-inference-preview-test.log` passes the shared controller drawing-preview
route without a document/history edit. These checks do not establish headset
snapping comfort, dense-scene GPU performance or complete inference parity.
`global-inference-build.log` passes the native editor example build without
warnings. The executable is frozen under this milestone's commit in
`.local/openxr/preview-builds/`. No replacement headset session was launched.

`joint-handle-tests.log` contains passing controller scope/permission/preference,
hit-rectangle/scaling/action-cache and face/cap visibility cases (plus unrelated
matches from the initial filter). `joint-handle-capture-tests.log` passes captured
erase release over a joint control. `joint-handle-joint-regressions.log` passes all
20 joint-related cases, including desktop marker navigation and the new XR route.
Two pre-existing fixture assumptions were corrected separately: `1716044` gives
the navigation-only test a saved baseline; `5ee922d` scopes outside-body volume
queries to their parent assembly. Production behavior was unchanged by those fixes.
The native example build passes without warnings in `joint-handle-build.log`.
The executable is frozen under `d7c05f3` in `.local/openxr/preview-builds/`.
No replacement headset session was launched; controller/visual acceptance remains open.

## User direction

1. Continue immersive viewport replacement autonomously, updating both documents
   with reality as milestones land. Only pause for human verification when the
   user requests it; do not stop merely because a milestone was committed or an
   overlay lacks human confirmation. Report that distinction honestly.
2. Functionality comes first. Broad suites, graphics validation, Rooftop/performance
   work and multiview are deferred. Use narrow checks appropriate to changes.
3. Have an executable ready before interrupting the user for a requested preview.
   Previews run until normal Close, with no automatic five-minute/session deadline.
4. Keep the app's existing autosave behavior. Each test launch must start from
   the clean bundled example, preserving control/readability preferences rather
   than restoring the previous edited scene. Do not discard an existing workspace
   or user edits to reset a test.
5. Commit focused milestones and push under the existing authorization. No deploy,
   release tags or PR. Work in this checkout and preserve unrelated/concurrent work.

## Repository and latest milestones

1. Checkout: `F:\Unity Projects\BenchUp3DEditor`, branch `feat/openxr-phase1`.
   Origin is `https://github.com/icosa-mirror/BenchUp3DEditor.git`;
   upstream is `https://github.com/BenchUp3D/editor.git`.
2. `634384c`: saved annotation label picking, existing-text editing and a shared
   predicted head sample for label input/rendering.
3. `7559d0c`: text-leader and dimension-stroke picking.
4. `27ae6c1`: retained original-edge references and model-space axis inference.
5. `d1b2335`: clean-example preview launcher, preserving preferences and app autosave.
6. `c535cad`: reference-coordinate projection onto locked axes.
7. `404a8f8`: direct-hit XR grid refinement and quantized reference acquisition.
8. XR multi-selection now has a saved Add/remove selection control and honors
   native Shift using desktop toggle semantics. Empty misses retain selection in
   that mode. The shared Edit menu already exposes context entry/exit and entity
   actions without requiring the 2D model view. Selection/gesture parity beyond
   ray toggle still needs auditing.
9. Shared scene preparation now updates the evaluated model without painting a
   2D viewport. Scoped XR snapshots propagate editing context while retaining
   geometry snapshots; connection overlays use that scope and labels/eyes share
   one staged frame source.
10. Frozen executables are under `.local/openxr/preview-builds/<commit>/`.
   The last requested manual launch used `dfab0e6`; it must not be confused with
   the latest built executable. Inspect current process ownership/path before
   closing, replacing or claiming that a preview runs the newest code.

## Implemented architecture

1. Optional native `openxr` feature; pinned OpenXR 0.22.0 and wgpu 30.0.1 bridge.
   `runtime.rs` handles preflight; `panel.rs` handles sessions, borrowed Vulkan
   swapchain images, frame pacing and quad/stereo submission.
2. `editor.rs` runs the existing application/egui UI offscreen, sharing its document,
   operations, history, GPU callbacks and persistent preferences through `app_host.rs`.
   Evaluated-model preparation is now shared through `prepare_render_model`,
   so XR source refresh no longer relies on `paint_evaluated_scene`. Scoped XR
   snapshots propagate editing context; stage and eye rendering share one frame
   source rather than preparing the scene again outside the UI pass.
   Fit selection now frames the selected rendered bounds in immersive space,
   reusing placement/navigation reset and saved zoom handling; empty/degenerate
   bounds retain the existing fallback. The desktop camera is not changed.
3. `desktop.rs` owns the normal taskbar window on the main thread. XR runs on a
   worker. Physical keyboard/text/IME and Windows clipboard are bridged; controller text entry is added in the current milestone; desktop
   mouse/focus do not replace headset pointing/focus. Close/Alt+F4 and shared
   Close/File Quit request shutdown. The eight-second stalled-shutdown guard is
   not a session deadline. Desktop mirroring is not implemented.
4. `immersive.rs` uses explicit eye transforms and shared retained geometry and
   display state. GPU environment, analytic grid, shadows, styles/X-Ray, section
   caps/contours, labels and overlays are implemented. Immersive mode hosts the
   shared inspector/context UI instead of a duplicate model canvas. The XR tool
   picker remains; legacy camera/tool rails remain in panel-only mode. Controller
   instructions retain transient notices and modeling values. Full visual
   equivalence has not been established.
5. `viewport_overlays.rs` shares model-space previews/hints. `world_lines.rs`,
   `world_text.rs`, `world_surfaces.rs`, `floor_grid.rs` and `backdrop.rs` render
   spatial feedback. Glyph/annotation layout is reused across head motion; both
   eyes share the billboard basis. Group labels use owner-aware GPU visibility.
6. `xr_tools.rs` routes controller input into shared drawing, measurements, modeling,
   paint/erase, text, grain, solids, clipboard and history commands. Held-trigger
   erase accumulates sampled targets for one release-time shared command/undo,
   with geometry/guide/annotation preview colors. Cancel, lost input, tool changes,
   paste and keyboard commands discard the pending erase. Fast-motion ray sweep
   interpolation is not implemented yet. Section grabs
   use shared drag/settings/persistence machinery. See the plan's tool table.
7. `world_picking.rs` retains edge/guide hierarchies and finite guide spans;
   `world_labels.rs` retains label rectangles and annotation-stroke indices;
   `world_inference.rs` handles acquired references/locked-axis projections.
   `grid.rs` accepts exact XR hit IDs and the evaluated model, avoiding a fresh
   model-wide search during grid refinement. Desktop's unhinted lookup is retained.
8. `navigation.rs` implements two-grip translation/rotation/scale. Keep upright
   defaults on; yaw-only rotation retains vertical translation and scale. Navigation,
   focus/tracking loss and capture cancellation require release-to-rearm. Absolute
   room poses and the transient navigation transform are not saved as preferences.

## Controller keyboard milestone

1. `xr/keyboard.rs` supplies the always-reachable Keyboard strip, alphanumeric
   and symbol pages, persistent Caps, Space, Backspace, arrows and Enter.
   Opening it replaces the larger XR tool controls; hiding it restores them.
   Select a text/numeric field first. Keys retain that field and feed ordinary
   egui text/key events on the next frame. Modal layering keeps the keyboard
   interactive above the backdrop; the shared modal shell reserves its height.
2. Visibility, Caps and Symbols use `XrPanelPreferences` and the existing immediate
   inspector preference save/restore path, with defaults for older saved data.
   Focus targets, queued text and geometry are not saved as keyboard preferences.
   Focus/tracking cancellation discards queued virtual input.
3. `xr_tools::State` now retains a pending line/shape/measurement target while
   the focused controller points at the panel. The key includes document revision,
   editing scope, tool/locks and operation stage. Losing panel tracking, changing
   the operation or editing geometry clears it. Validate before the shared field
   can consume Enter and again after UI actions. Held modeling/section/freehand
   gestures keep their cancellation behavior.
4. The status field routes immersive Enter through the supplied world snap/value,
   without desktop pointer refresh. This covers line/shape and measurement input;
   Exact value modeling now supplies an explicit frozen target while ordinary held
   grabs retain their cancellation behavior. XR uses the in-panel chooser below.

## Keyboard focus correction for the requested test

1. Egui's click-elsewhere focus policy surrenders TextEdit focus on a keyboard
   click even when the key itself is non-focusable. Earlier keyboard coverage
   checked final text, so it missed focus loss during the click/release frames.
2. The XR host now temporarily suppresses click-elsewhere focus surrender only
   while a valid text target is being used over the virtual keyboard. A scoped
   guard restores the original policy after UI layout, leaving normal clicks
   elsewhere unchanged. Keeping active focus also preserves the text field's
   event filter; caret arrows must not become global focus-navigation keys.
3. The updated test checks focus on every press/release, successive characters,
   caret arrows, Backspace, outside-click behavior, editor/modal contexts and
   queued-input cancellation after tracking loss. The unmodified behavior fails
   the new focus assertion; the fixed production keyboard module passes both UI
   cases when linked against this checkout's cached egui dependency. Evidence is
   in `.local/openxr/keyboard-focus-fix/before.log` and `after.log`. These are
   isolated UI tests, not complete app/headset acceptance. The native build passes
   without warnings in `keyboard-focus-fix/build.log`. The user confirmed the
   corrected File-box workflow on 2026-11-11.

## Section-cap interaction

1. The shared `display_scene::SectionCache` retains an optional selectable body
   owner for every rendered cap triangle. It builds cut contours separately per
   editable context owner, preserving touching groups instead of merging their
   cap ownership. Ungrouped/out-of-scope fills have no document entity or face ID.
2. `xr/section_picking.rs` builds a retained triangle hierarchy when the cap-cache
   generation changes. Controller selection, edge targets and annotation queries
   respect the filled triangles; empty cuts/holes remain open. Cap normals are
   the outward positive model axis. Disabling fill clears cap hits. Flip-plane
   hit tests now respect visible face/cap occlusion rather than hidden quads.
3. A grouped cap can select, transform, erase or Group-paint its original group
   through shared operations. Face paint, Push/Pull and Offset require original
   document faces and reject cap-face edits. Ungrouped cap points can anchor
   drawing/measurement planes. Pointer hits carry optional entity ownership and
   explicit section normals, never fabricated document IDs. Group painting uses
   the same target helper as the existing desktop face/group paint entry point.
4. Section geometry and its GPU rendering remain shared; no new label hiding or
   fading policy is introduced. Cap selection/normal/occlusion controller behavior,
   nested/scoped contexts and complex overlapping/hollow bodies still need wider
   acceptance checks. Global origin/reference inference rules are unchanged.

## World text visibility

1. XR text now uses the retained GPU opaque-face/section-cap visibility texture
   and sorted owner slots. It no longer samples the final scene depth attachment,
   which includes expanded opaque edge cores and was cutting holes through labels
   on their own objects. This follows desktop markers' face-only visibility rule.
   Wire strokes no longer occlude text; foreign opaque faces and caps still do.
   World guide-line depth handling, retained text buffers and shared head basis
   are unchanged. Overlay/HUD text retains its existing bypass policy.
2. The final code embeds the WGSL shader normally. The temporary runtime shader
   override used to isolate the rejection paths was removed before the final
   build. Its local probe executable/shaders under `text-depth-probes/` are
   diagnostics only and must not be confused with a test-ready milestone build.
3. GPU evidence fixes the observed glyph cutouts, not every label-parity case.
   Compare wrapping/angular size and layered foreign occluders behind owner
   surfaces against desktop's owner-excluding query. Full headset readability
   and visual equivalence remain unconfirmed.

## Exact value modeling

1. Enable Exact value in the XR controls for Move, Rotate, Scale, Push/Pull or
   Offset. It is saved in `XrPanelPreferences`, defaults off for older preferences,
   and uses the shared immediate preference save/restore path. Default held-trigger
   dragging remains available when it is off.
2. Trigger once on an original editable target to choose the anchor, aim to choose
   a direction/target, then trigger again. The preview freezes and the keyboard
   opens with the shared value field focused. Enter commits through the shared
   world-value operation and undo history. Invalid input leaves the preview for
   correction. Move requires a nonzero direction before confirmation.
3. Transient aiming/typing state and input-focus requests are not saved. Only a
   confirmed typing target survives a focused panel hover. Tool/context/lock or
   document changes, Cancel, navigation/tracking loss and modal interaction discard
   it. Aiming still cancels on leaving the scene. Pending previews cannot commit
   on a later release. XR panel and spatial HUD hints describe the exact-value flow.

## In-panel file choosing

1. `native_dialog.rs` supplies the host chooser used by the existing native file
   workers in `project_files.rs`. The XR host installs a request/response channel;
   ordinary desktop hosts continue using rfd dialogs with the same titles,
   filters and filenames. Web file dialogs are unchanged. Project codecs, atomic
   writes, import/export logic, results and error handling remain shared.
2. `xr/file_dialog.rs` uses the shared Modal shell for Folder and File entry,
   Up/Home/Refresh navigation, a virtualized folder/file list, hidden-file and
   file-type filters, Open/Save/Cancel, and replacement confirmation. Folder
   enumeration runs on a worker. Relative folder input resolves against the
   displayed folder and is retained as an absolute path. The keyboard remains
   above the modal backdrop and the footer stays above its reserved space.
3. Default extensions are applied before checking for an existing file. Confirm
   replacement against that final path; Cancel preserves the file. The last
   successfully read directory and both filters use `XrFilePreferences` in the
   shared immediate-save capture/restore path, without requiring a model edit.
4. Full controller open/edit/save/reopen acceptance remains open. Creating folders
   from the chooser and previewing exported PDFs inside the headset are not yet
   implemented; existing PDF exports still invoke the desktop PDF viewer.

## Next implementation work

1. Follow the numbered spatial parity inventory in `OPENXR_PLAN.md`. Audit concrete
   desktop consumers and immersive input routes: editing context/context actions,
   remaining handles, precision/locks, joinery/fastener targeting and display modes.
2. Evaluated-model preparation has been extracted from `paint_evaluated_scene`.
   Close the remaining UI/input dependencies: host/window commands, desktop-only
   selection gestures and any remaining handle/tool routes identified by the audit. Known limits include
   broader off-face/global references, near-field/fast sweep sampling limits, evaluated-only paint/edit
   guards and broader nested/scoped cap acceptance. A retained cap hierarchy now
   supplements original-face occlusion queries using the rendered triangles. Guide intersections are locally bounded
   to 16 candidates/eight exact targets; dense clusters can omit a target.
   Current-ray source validation is now shared by all world tools. Exercise the
   full background/project/undo/context transition matrix in the headset later;
   software checks cover representative transitions rather than that whole matrix.
3. The duplicate 2D model viewport is now removed from immersive mode. The
   existing 3D-view preference switches between the immersive-first inspector/tool
   panel and the original panel-only editor. Shared inspector visibility, sections,
   docking state and tables are reused, with direct edit/close-context controls.
   Remaining tool/gesture/display acceptance work is still open.

4. Keep implementing while human verification is pending. A requested headset
   session is a deliberate handoff, not an excuse to interrupt early or repeatedly
   ask readiness. Broad acceptance gates remain deferred as listed in the plan.

## Build and clean-scene preview workflow

1. Use host PowerShell 7 for host-state/process/runtime actions. Rust/MSVC,
   SteamVR, a connected headset and RTX 4090 Vulkan rendering have worked here.
   SteamVR's loader directory is typically
   `C:\Program Files (x86)\Steam\steamapps\common\SteamVR\bin\win64`.
   Do not change the system runtime or enable validation for ordinary previews.
2. Build implementation changes when needed, with bounded output:

   ```powershell
   cargo build -p benchup-app --locked --features openxr --example openxr_editor *> .local/openxr/editor-build.log
   Get-Content .local/openxr/editor-build.log -Tail 18
   ```

   Preserve the checked executable in a commit-named ignored directory before
   changing sources again. Record build/launch path and timestamp; do not confuse
   a new build with an old still-running instance.
3. Launch only the intended already-built executable using the clean-scene helper:

   ```powershell
   ./scripts/openxr_preview.ps1 -Binary ".local/openxr/preview-builds/<checked-commit>/openxr_editor.exe"
   ```

   Replace `<checked-commit>` with the milestone whose executable was actually
   built; do not assume the newest Git commit is a built executable. Run via
   host PowerShell 7. The helper never compiles and refuses an existing
   preview instead of stopping it. It seeds a unique directory under
   `.local/openxr/preview-sessions/` with the exact `assets/examples/Example.bup`
   as `model.json`. Only `preferences.ron` is carried forward from the last session;
   first use can migrate the old parity-data preferences. Model/history/selection
   autosaves are not copied. App autosave continues normally within each new
   session; explicit Save/Save As remain available. The launcher records executable
   path/mtime, PID and data directory in `launch.json`, with session-local logs.
4. The clean-scene helper was exercised for commit `8c9c183`: session
   `20261110-114406-794-286ed31e`, PID 90260 at launch. Logs recorded the clean
   404-face model, stereo plus quad frames and FOCUSED state. Inspect current
   ownership/liveness before any replacement; this launch does not validate
   subsequent keyboard changes.
5. `.local/openxr/parity-data` and older `editor-data`, `grain-data`, `section-data`
   contain previous workspace state. Preserve them; do not resume their edited
   models for a fresh test. The user can reopen
   `assets/examples/Example.bup` through File > Open to reset a running workspace.
6. Diagnose prerequisites/runtime failures with `scripts/openxr_probe.ps1`,
   `openxr_diagnostics` and [OPENXR_WINDOWS.md](OPENXR_WINDOWS.md) when relevant;
   do not restart the original preflight sequence after every coding milestone.

## Evidence and limits

1. Section caps: `section-cap-tests.log` passes six targeted cases covering touching
   group ownership, nearest/two-sided hits and fill removal, hierarchy queries with
   open holes, group selection/movement/undo, Group paint/undo, and read-only cap
   faces with X/Y/Z drawing anchors. `section-cap-display-regressions.log` passes
   five shared display/cache cases; `section-cap-tool-regressions.log` passes all
   thirteen current XR tool cases. `section-cap-input-regressions.log` passes three
   controller routing cases; `section-cap-paint-regressions.log` passes the existing
   viewport color/history and face/group-paint undo cases. These checks overlap
   and are not a full suite or live-controller cap acceptance.
   The native build passes without warnings in `section-cap-build.log`. RTX 4090
   Vulkan images `section-cap-eye.png` and `section-cap-cut.png` match the prior
   checked normal/cut images pixel for pixel. The cap image was inspected. No
   replacement OpenXR session was launched; the checked executable is preserved
   under `92b7f8a` in `.local/openxr/preview-builds/`.


2. World text: `text-surface-visibility-build.log` passes the native build without
   warnings. On RTX 4090 Vulkan, the final embedded shader's clean-example image
   `text-surface-visibility-eye.png` matches the successful prototype pixel for
   pixel, restoring 163 incorrectly rejected label pixels in this fixed view.
   The final active-Z-section image `text-surface-visibility-cap.png` likewise
   matches the prototype cap-occlusion result. Diagnostic rejection classification
   in `text-depth-probes/` distinguishes the extra scene-depth rejection from
   foreign-face rejection; the active 1.1 m section case exercises cap rejection.
   The clean scene and final cap image were inspected. No OpenXR/headset session
   was started. The exact executable is preserved under commit `ceae626`.


3. Exact value modeling: `.local/openxr/precise-modeling-tests.log` passes three
   cases covering all five tools, exact transform values, one-step undo, discard
   after tracking/Cancel/tool/document changes, and default trigger-release edits.
   `precise-modeling-tool-regressions.log` passes all ten current XR tool cases;
   `precise-modeling-keyboard-regressions.log` passes two keyboard cases, and
   `precise-modeling-preference-tests.log` passes immediate save/default/restore.
   The native build passes without warnings in `precise-modeling-build.log`.
   `exact-modeling-controls.png` was inspected for control/hint fit; the RTX 4090
   Vulkan `exact-modeling-hud.png` snapshot renders the 404-face example but its
   diagnostic camera does not establish headset HUD readability. Group labels
   in that earlier image showed rough/clipped glyph edges; the world-text fix
   above resolves the observed cutouts. Full visual equivalence remains open. The checked executable is preserved under
   commit `468f1f7` in `.local/openxr/preview-builds/`. No headset session
   was launched for this change.


4. File chooser: `.local/openxr/file-dialog-tests.log` passes three cases covering
   footer placement above the keyboard, folder/type/hidden filtering plus relative
   folder navigation, and final-extension replacement confirmation with Cancel
   preserving the original file. `file-dialog-project-roundtrip.log` exercises the
   existing save/open/cancel workers through the XR host channel and decodes the
   saved project. `file-dialog-preference-tests.log` verifies immediate folder and
   filter persistence, restoration and legacy defaults without model edits.
   The native build passes without warnings in `file-dialog-build.log`; its exact
   executable is preserved under commit `5c99aac` in `preview-builds/`.
   These checks do not prove a full controller open/edit/save/reopen workflow.


5. `.local/openxr/precision-panel-tests.log` passes three targeted cases: retained
   line direction plus numeric length/undo and post-commit invalidation; rectangle
   dimensions without desktop projection; and clearing targets after tracking,
   tool, editing-scope, operation-stage or geometry changes. The latest test binary
   also passes the two existing erase regressions and two virtual-keyboard cases
   (`precision-erase-regressions.log`, `precision-keyboard-regressions.log`).
   The native build passes without warnings in `precision-panel-build.log` and
   is preserved under commit `56f7149` in `.local/openxr/preview-builds/`.
   This is narrow software coverage, not full tool or headset acceptance.


6. Keyboard milestone: `.local/openxr/virtual-keyboard-tests.log` passes two cases:
   actual rendered-key clicks update a shared text field in both editor and modal
   contexts (including Backspace), and focus loss discards queued input.
   `.local/openxr/keyboard-preference-tests.log` passes immediate persistence,
   restoration and legacy defaults without model edits. The final native build
   passes without warnings in `virtual-keyboard-final-build.log`. The RTX 4090
   Vulkan snapshot `virtual-keyboard-panel.png` was inspected for keyboard fit
   below the inspector. These are software/GPU checks, not headset confirmation.


7. Human feedback confirmed the quad's visibility/readability/upright orientation
   and world stability, basic controller cursor/click/scroll, comfort controls and
   Recenter. The user viewed the immersive scene and gave positive two-grip feedback.
   There is no recorded complete overlay/display/tool parity comparison.
8. Latest native build: `.local/openxr/immersive-primary-final-build.log`.
   `.local/openxr/immersive-primary-panel.png` and `immersive-primary-eye.png` were
   inspected: shared inspector/controls without the duplicate model, and the clean
   404-face example in the independent eye renderer. A subsequent requested
   clean-scene headset launch is recorded above; full visual parity is unverified.
   `.local/openxr/fit-selection-controls.png` was checked for control fit in the
   GPU-rendered panel using a fresh example and carried-forward preferences.
   This verifies layout, not live selected-target framing in the headset.
   `.local/openxr/scene-source-tests.log` passes two focused application cases
   without desktop painting: edit refresh and editing-scope propagation with
   stable snapshots. `.local/openxr/shared-scene-preparation-eye.png` was rendered
   on the RTX 4090 Vulkan path from a fresh bundled-example fixture (404 faces)
   and inspected. `.local/openxr/erase-stroke-final-tests.log` passes the two erase
   release/undo/cancellation cases. `.local/openxr/selection-immediate-preference-test.log`
   passes the shared immediate-save/default/restoration case. Earlier isolated
   spatial/reference and grid checks are in `reference-projection-checks.log` and
   `grid-hit-checks.log` under `.local/openxr/`. The full suite was not run.

9. Earlier Vulkan panel/eye snapshots exercised the RTX 4090 rendering paths.
   `openxr_editor --desktop-snapshot <png>` and `--immersive-snapshot <png>` support
   narrow hardware rendering checks without a headset session. They do not prove
   human visual equivalence or all recent controller interactions.
10. Clean graphics validation, end-to-end project/save/dialog workflows, full loss/
   restart coverage, large-scene timings and current desktop/browser compatibility
   remain open. `OPENXR_WINDOWS.md` retains historical runtime/wgpu validation
   findings; validation-disabled preview success does not resolve those findings.
11. Preserve the two-eye-pass decision. Batch/profile later, then assess multiview
   across geometry, visibility, overlays and text with a two-pass fallback. No
   multiview, validation-layer or benchmark detour during this functionality pass.
