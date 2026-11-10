# BenchUp OpenXR development handoff

Updated: 2026-11-10. Status follows the current `feat/openxr-phase1` worktree.
Read [AGENTS.md](AGENTS.md) and [OPENXR_PLAN.md](OPENXR_PLAN.md) before continuing.
The plan is the current scope/status/backlog; this file is the operational handoff.
Git history retains the earlier milestones and historical handoff revisions.

## Paused, test-ready stop

The requested bounded milestone is ready: a controller-operated keyboard for
editor text fields and modal dialogs, using shared egui text/key input and
persistent XR keyboard preferences. The immersive-first panel remains in place.
The native executable was built at 2026-11-10 12:16:03 local time. Preserve it in
`.local/openxr/preview-builds/<this-commit>/openxr_editor.exe`. Stop here and keep
the goal paused; do not replace the existing preview without a user request.
This does not complete the overall OpenXR plan.

## User direction when resumed

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
   The last requested manual launch used `27ae6c1`; it must not be confused with
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
3. This adds editor/dialog text entry, not native Windows file-dialog control.
   Precise world drawing while aiming at the keyboard remains open: the current
   scene-pointer cancellation path can clear the tool's last snap. Do not claim
   complete numeric/lock parity from successful inspector field entry.

## Next work after the user resumes

1. Follow the numbered spatial parity inventory in `OPENXR_PLAN.md`. Audit concrete
   desktop consumers and immersive input routes: editing context/context actions,
   remaining handles, precision/locks, joinery/fastener targeting and display modes.
2. Evaluated-model preparation has been extracted from `paint_evaluated_scene`.
   Close the remaining UI/input dependencies: camera/view commands, desktop-only
   selection gestures and any remaining handle/tool routes identified by the audit. Known limits include
   broader off-face/global references, fast erase-sweep interpolation, evaluated-only paint/edit
   guards and cap-only picking/occlusion. Original-face occlusion queries do not
   fully represent rendered section caps. Guide intersections are locally bounded
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

1. Keyboard milestone: `.local/openxr/virtual-keyboard-tests.log` passes two cases:
   actual rendered-key clicks update a shared text field in both editor and modal
   contexts (including Backspace), and focus loss discards queued input.
   `.local/openxr/keyboard-preference-tests.log` passes immediate persistence,
   restoration and legacy defaults without model edits. The final native build
   passes without warnings in `virtual-keyboard-final-build.log`. The RTX 4090
   Vulkan snapshot `virtual-keyboard-panel.png` was inspected for keyboard fit
   below the inspector. These are software/GPU checks, not headset confirmation.


2. Human feedback confirmed the quad's visibility/readability/upright orientation
   and world stability, basic controller cursor/click/scroll, comfort controls and
   Recenter. The user viewed the immersive scene and gave positive two-grip feedback.
   There is no recorded complete overlay/display/tool parity comparison.
3. Latest native build: `.local/openxr/immersive-primary-final-build.log`.
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

4. Earlier Vulkan panel/eye snapshots exercised the RTX 4090 rendering paths.
   `openxr_editor --desktop-snapshot <png>` and `--immersive-snapshot <png>` support
   narrow hardware rendering checks without a headset session. They do not prove
   human visual equivalence or all recent controller interactions.
5. Clean graphics validation, end-to-end project/save/dialog workflows, full loss/
   restart coverage, large-scene timings and current desktop/browser compatibility
   remain open. `OPENXR_WINDOWS.md` retains historical runtime/wgpu validation
   findings; validation-disabled preview success does not resolve those findings.
6. Preserve the two-eye-pass decision. Batch/profile later, then assess multiview
   across geometry, visibility, overlays and text with a two-pass fallback. No
   multiview, validation-layer or benchmark detour during this functionality pass.
