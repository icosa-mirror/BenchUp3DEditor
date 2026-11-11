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
locks and host-aware shared precision input landed in `1de74a5`. Current work
adds destination-folder creation to the in-headset file chooser.
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
combined executable is frozen under this milestone commit in
`.local/openxr/preview-builds/`. No replacement headset session was launched.

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
   Close the remaining UI/input dependencies: camera/view commands, desktop-only
   selection gestures and any remaining handle/tool routes identified by the audit. Known limits include
   broader off-face/global references, fast erase-sweep interpolation, evaluated-only paint/edit
   guards and broader nested/scoped cap acceptance. A retained cap hierarchy now
   supplements original-face occlusion queries using the rendered triangles. Guide intersections are locally bounded
   to 16 candidates/eight exact targets; dense clusters can omit a target.
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
