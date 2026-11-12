# BenchUp OpenXR implementation plan

Updated: 2026-11-12. Status follows the current `feat/openxr-phase1` worktree;
completed milestones and build evidence are recorded in the handoff.
This is the current plan, not a chronological implementation log. Operational
commands and the latest continuation state belong in [OPENXR_HANDOFF.md](OPENXR_HANDOFF.md).

## Active continuation

Latest user direction: prioritize actual bugs and missing functionality, not
speculative edge cases. The core coding checklist C01-C25 is implemented. A requested test confirmed
the earlier controller panel faced the head and froze when a grip was held.
The new C25 rigid-parent build corrects that gap and is ready for a physical check;
its source/path are recorded in `.local/openxr/rigid-controller-panel-build.json`.
Remaining acceptance is not complete; continue implementation only for concrete
gaps demonstrated by normal use, requested testing or existing evidence. Do not
create extra milestones or validation work merely to remain active.

The user resumed the goal and is currently busy. Continue independent development
without more test questions or replacing their running preview. Their `dfab0e6`
clean-example session confirmed that virtual-keyboard typing now retains focus.
They also confirmed no label holes/flicker and an in-headset Save As chooser.
Exact value movement/undo verification remains pending. Ask future test questions
one at a time when the user returns and requests testing.

Label occlusion policy (hide versus fade behind geometry) explicitly awaits
user discussion; do not change it in the meantime. Current implementation work
now includes retained section-cap ownership/picking: grouped caps select, paint
and transform their owner; cap surfaces anchor drawing without becoming document
faces, and filled caps block hidden face/edge/annotation targets. Controller and
nested/scoped cap acceptance remains pending.
The XR panel also exposes persisted axis/plane locks and destination-folder
creation. Shared View controls now target immersive placement: Fit model/selection,
Center origin, Reset view and all seven standard directions. Headset projection
remains runtime-controlled; those commands do not modify the desktop camera.
Held-trigger erasing now interpolates between adjacent captured rays through the
retained picker, including the release interval, and keeps one undo transaction.
Interpolation is bounded and resets on input/scene discontinuities; thin/near
targets at the sample limit and hardware interaction/performance still need checking.
All native world-tool callbacks now reject input picked from a different committed
model, project generation, editing context, tool or display state. Identical reloads
are distinguished from the old project; continuous evaluated previews use the
committed-model revision so valid dragging remains active.
Joint edit/warning billboards now have retained controller hit rectangles and
world hints, using the shared desktop edit action, context navigation and permission
checks. Captured gestures keep priority over icons. Fastener entry-face controls
remain conditional on the existing disabled feature gate. Controller enclosing
window selection is implemented with a saved mode, retained GPU outline and
release-time shared selection query. Direct double-trigger group/text editing
and empty-click context exit share desktop operations, including tiny window-mode
clicks. C15's global/off-face and modeling inference routes are implemented.
C16 has closed its recorded A01-A14 coding gaps, including retained overlay
markers, axes styling/fade, mode/target instructions, native paste seeds and copied
guide previews. C16's eight-row coding audit is complete. Subsequent C17-C23
milestones implement reference-space recovery, compact controls, optional
controller attachment/recall and modal/keyboard cancellation fixes. The latest
prepared executable is `591c117`; its corrected immersive launch has submitted
a head-tracked stereo frame, with human confirmation pending.
The acceptance gates below remain open.
Full phase/visual acceptance remains open.

## Outcome and working rules

1. Replace the XR panel's duplicate 2D model viewport with the immersive,
   head-tracked stereo viewport. Spatial hints, previews, overlays and labels
   belong with the 3D model. Menus, inspectors and dialogs may remain floating UI.
2. Keep one authoritative document, selection and undo history. Share model
   operations and tool state with desktop/web; do not translate desktop pixels
   into 3D operations or fork the editor.
3. Continue authorized implementation autonomously. Do not pause for human
   verification unless the user requests it. Pending human confirmation is a
   reporting limit, not a reason to stop development or ask repeated readiness
   questions. Have a working executable before offering or starting a requested test.
4. Prioritize functionality. Broad suites, Vulkan validation, large-scene
   benchmarking and multiview remain deferred. Run narrow checks appropriate to
   changes; do not describe builds or submitted frames as verified visual parity.
5. Persist user-facing controls through the existing shared preferences. Preserve
   the app's existing autosave behavior. Test launches start from a fresh bundled
   example, carrying forward control/readability preferences rather than scene edits.
6. Commit coherent milestones and push within the existing authorization. Do not
   deploy, create release tags or create a PR. Work in the current checkout and
   preserve unrelated changes and other agents' processes.
7. Interactive previews have no deadline. Use the normal desktop/taskbar window
   for manual Close. Inspect ownership before replacing an existing session.

## Phase status

Phase 2 advanced before formal Phase 1 acceptance at the user's explicit request
to reach immersive functionality quickly. Neither phase is accepted as complete.

| Phase | Implemented now | Outstanding acceptance or functionality |
|---|---|---|
| 1A: panel/input foundation | Metric panel geometry; ray mapping; OpenXR actions; capture and focus/tracking cancellation; LOCAL origin-change compensation | Complete drag/loss/reference-change/restart coverage on the headset |
| 1B: Vulkan/wgpu interoperability | Runtime preflight; runtime-selected Vulkan GPU/device; borrowed swapchain images; GPU quad/stereo rendering; XR frame lifecycle; pinned SPIR-V compiler fix | OpenXR-runtime core/synchronization validation and the full restart/loss matrix remain open |
| 1C: complete editor panel | Shared editor UI and GPU viewport callbacks rendered into an OpenXR quad; native desktop lifetime window | Complete host-service workflow acceptance; desktop mirroring stays explicitly deferred |
| 1D: interaction/usability | Controller cursor/click/scroll; comfort/recenter controls; physical keyboard/IME/clipboard bridge; controller keyboard with saved visibility/caps/symbols; in-panel file chooser with destination-folder creation; native lifetime window; shared Close/Quit path; saved preferences; compact controls and optional controller panel/recall | Headset keyboard/file-chooser/controller-panel acceptance and end-to-end project workflow verification; additional native window commands remain explicitly deferred |
| 2A: shared scene/overlay preparation | Evaluated model/overlays, scoped snapshots and an immersive inspector/tool panel without a duplicate 2D canvas; eight-row source audit closed | Rendered/controller workflow acceptance across the recorded parity matrix |
| 2B: stereo scene | Explicit eye poses/projections; shared geometry across two eye passes; GPU environment, grid, shadows and section rendering | Visual equivalence not established; validation/performance gates remain deferred |
| 2C: immersive editing | World-ray selection, audited drawing/modeling/annotation/handle routes, guides/reference inference and upright two-grip navigation; no duplicate 2D model view | Controller acceptance of inference/tool/handle and workflow/gesture parity; fix newly demonstrated gaps |

## Coding milestone checklist

These stable IDs track committed implementation, separately from the acceptance
gates below. They are not equal units of effort; their count is not a completion
percentage. Keep newly discovered work under the relevant open milestone and
record scope changes rather than silently changing an estimate.

1. [x] C01: Runtime/Vulkan interoperability and the complete editor quad panel.
2. [x] C02: Independent stereo scene and retained GPU overlay/rendering baseline.
3. [x] C03: Controller ray selection and the shared drawing/modeling tool baseline.
4. [x] C04: Two-grip navigation, upright lock, comfort controls and recentering.
5. [x] C05: Physical and controller keyboards, text focus and readable panel controls.
6. [x] C06: Native lifetime window and in-headset file chooser with New folder.
7. [x] C07: World numeric entry, Exact value modeling, saved axis/plane controls.
8. [x] C08: Retained section-cap ownership and picking (`92b7f8a`).
9. [x] C09: Shared View commands target immersive placement (`25fd106`).
10. [x] C10: Captured erase interpolation and cancellation (`6ca8676`).
11. [x] C11: Current-ray source validation across UI changes (`466b9f4`).
12. [x] C12: Controller joint edit/warning icons (`d7c05f3`).
13. [x] C13: Controller enclosing-window selection, saved mode and safe cancellation.
14. [x] C14: Direct Select double-trigger text/object editing and empty-click
    context exit, including tiny clicks in Window selection mode. Shared Edit
    commands and panel context controls provide entity actions; secondary trigger
    retains Cancel rather than opening a competing world context popup.
15. [x] C15: Global/off-face references, bounded candidate pairs, edge/face containment, evaluated-edge acquisition and all five modeling inference routes.
16. [x] C16: Reconcile the eight tool/overlay/command source routes and implement
    the fourteen confirmed gaps A01-A14. Rendered/controller acceptance remains
    separate from this coding milestone.
17. [x] C17: Handle runtime LOCAL reference-space changes at their specified
    frame time, preserve physical placement with a valid origin transform and
    cancel/refit safely when that transform is unknown. Two focused checks and
    the locked native build pass. Runtime recenter acceptance remains open.
18. [x] C18: Start compacting XR controls toward the requested controller-panel layout.
    Keep Recenter and everyday tool actions accessible, wrap rows at narrow widths
    and persist a collapsed-by-default View and comfort section. The focused
    persistence check, locked build and inspected open/closed hardware snapshots
    pass. Further layout reduction and headset/controller acceptance remain open.
19. [x] C19: Optional non-dominant-controller panel, saved handedness/width,
    dominant-hand pointing and tracking/focus fallbacks (rigid parenting corrected in C25). The
    implementation passes two placement checks, immediate preference restoration,
    the locked native build and inspected settings-layout snapshots.
    World placement remains the default pending headset readability acceptance.
20. [x] C20: Remove repeated immersive-panel instructions when the shared status
    bar already shows the same text; preserve distinct notices/value messages and
    panel-only controller help. Expose full truncated XR status text on hover.
    The locked native build and paired before/after hardware layout snapshots
    pass. Headset hover/readability acceptance remains open.
21. [x] C21: Preserve open dialog drafts on controller capture loss. Distinguish
    synthetic drag-cancel Escape from deliberate Escape, while still releasing
    the pointer outside widgets. The reproducing shared-Modal regression and
    locked native build pass. Hardware modal/loss acceptance remains open.
22. [x] C22: Ray-free controller-panel recall using the panel hand's existing
    secondary button. Detach/persist world placement, cancel captures and wait
    for a valid head pose when recentering. Release guards prevent startup,
    held-button, inactive-action and handedness-change commands. The guard
    regression, locked native build and setup-layout snapshot pass. Physical
    Menu/B delivery and modal/keyboard recovery acceptance remain open.
23. [x] C23: Treat live panel-placement cancellation separately from keyboard
    focus loss. Retain the text target and completed queued virtual keys while
    focused/head-tracked; keep clearing them on actual focus/tracking loss. The
    editor/modal field regression and locked native build pass. Physical
    placement/recall keyboard-focus acceptance remains open.
24. [x] C24: Correct the immersive preview launch after a real test restored
    `immersive_scene:false`. The clean-scene helper now starts in 3D while keeping
    readability preferences; `-PreserveViewMode` explicitly retains panel-only
    test settings when needed. Parser and actual-profile normalization checks pass.
    The corrected `591c117` session has submitted a stereo scene; human
    confirmation of that relaunch is pending.
25. [x] C25: Rigidly parent the quad to the non-dominant controller's OpenXR grip
    space, using one fixed local translation and fixed local rotation (corrected to 270 degrees about X in C26). Remove
    head-facing orientation and grip freezing. Only both grips suspend pointer
    input for scene navigation. Two rigid-transform/fallback checks pass; the
    locked native build passes. Physical rigid-following acceptance is pending.
26. [x] C26: Apply the user-confirmed 270-degree local X rotation to the
    controller panel, in both its submitted quad pose and ray/pointer pose.
    The locked native build passes; the existing rigid-transform assertion is
    updated for the corrected local rotation. Physical facing/click acceptance
    is pending.
27. [ ] C27: Visible tracked controller representations. The user reported their
    absence in the headset; this is a genuine missing feature, not yet implemented.
28. [x] C28: Fix ignored first panel clicks after pointing away. Release arming
    must use valid controller tracking, not whether its ray projects onto the
    panel plane. The real-button first-click regression and all six focused input checks pass,
    as do the two transform checks and combined locked native build. Hardware
    first-click acceptance failed: intermittent missed clicks also occur while
    staying on the panel and moving between widgets; this arming fix is insufficient.
29. [ ] C29: Resolve intermittent panel clicks across widgets. Add bounded
    `[BenchUpXRClick]` action-to-UI tracing and reproduce in the headset before
    choosing a fix. The diagnostic build preserves existing input thresholds.
    The user confirmed the 270-degree controller-panel orientation is comfortable.

Automatic Fasteners is intentionally disabled by `fasteners::AVAILABLE = false`.
Its entry-face XR route is conditional future work, not a reason to enable that
feature or a current required gap against the active desktop editor. Label
occlusion policy remains a separate discussion-dependent item. Hardware/browser
acceptance, validation and performance work remain in the verification gates;
acceptance can still reveal additional implementation work.

## C16 source audit

C16's coding audit is closed: the eight source routes below are reconciled and
the fourteen confirmed findings are implemented in the built native preview.
This is source/build evidence, not proof of headset/graphics acceptance or full
goal completion. New findings from the acceptance matrix remain implementation
work and must be recorded explicitly.

| ID | Confirmed finding | State |
|---|---|---|
| A01 | Desktop Move fades unselected faces and overlays edges; the retained XR renderer previously used ordinary opaque styling | GPU uniform/selection-mask route implemented; visual acceptance open |
| A02 | XR previously used document selection while desktop highlighted preview copies/caps | Operation-returned preview selection is shared by desktop/XR; evaluated-ID fallback cached per rendered snapshot; visual acceptance open |
| A03 | World HUD repeated default instructions and advertised direct Select actions during window selection or target-handle actions | Shared mode-specific hint and target override composition implemented; status/errors preserved; headset acceptance open |
| A04 | Measurements/text only advertised their full sequence; section handles had no action-specific prompts | Measurement/text phase prompts shared by HUD and XR controls; section hover/drag instructions implemented; controller acceptance open |
| A05 | Desktop attached text has an open arrowhead at its attachment; XR retained text previously drew only the leader | Retained GPU arrowheads implemented for saved, edited/new and clipboard text; shader/native checks pass; independent saved-text fixture renders three readable labels/open heads in three directions; live/clipboard/headset acceptance remains open |
| A06 | Desktop joint marker tooltips identify the joint and its warning; XR only advertised editing, and its global error list is limited to four | Target-specific joint identity/error prompt implemented independently of that limit; multiline HUD composition updated; six focused checks and native build passed; controller acceptance open |
| A07 | Desktop Flip planes use a 1-point idle / 2-point hover outline; XR previously used a fixed 1.5-point outline | XR retained segments now use the desktop idle/hover widths; native build passed; visual acceptance open |
| A08 | Desktop marks the fixed text attachment while positioning its label; XR omitted that marker | Text measurement anchor now supplies the existing retained billboard circle; native build passed; visual acceptance open |
| A09 | Desktop projection/Move/Scale inference guides mark their source points, and modeling-start hints use the wider cursor offset; XR omitted those circles and used the normal hint offset before a grab | Existing retained billboard circles and pre-grab hint offset now match the desktop routes; combined marker build passed; visual acceptance open |
| A10 | Desktop axes include a small depth-tested origin dot; XR previously drew the three axes without that dot | Retained GPU filled-circle instance implemented under the existing axes setting with scene depth/section clipping; shared shader check and native build passed; visual acceptance open |
| A11 | Desktop axes distinguish positive/negative halves with width/opacity and fade with forward depth; XR previously drew full-colour 2-point axes without that fade | Directional retained instances and GPU reciprocal-depth fade implemented from the live eye matrix; shader/math checks and native build passed; visual acceptance open |
| A12 | Shared Paste seeds its preview with desktop camera inference, and library insertion uses the desktop camera target, even in immersive mode while the ray is still over the panel | Native initial previews now use the clipboard anchor and skip desktop picking/inference; shared world-ray placement/commit and one-step undo checks passed; desktop library/Paste in Place checks and native build passed; controller acceptance open |
| A13 | Desktop construction guides increase stroke width on selection/hover, including point crosses; XR previously changed colour but kept a fixed 1.5-point stroke | Retained lines and point crosses now use 2.5-point selected / 2-point hovered / 1.5-point idle strokes with existing colour/erasing logic; combined guide build passed; visual acceptance open |
| A14 | Desktop clipboard ghosts include copied construction lines and guide points; native paste reference instances previously contained text leaders/ticks only | Clipboard line/point instances implemented with shared hidden/Guides rules; GPU spans include paste translation; source/rebasing/span, shader checks and native build passed, including remote diagonal anchors; visual acceptance open |

The source reconciliation follows actual producers and consumers, rather than
counting passing tests as proof of the whole viewport:

| Row | Desktop/shared producer | Native consumer and reconciliation |
|---|---|---|
| 1 | `viewport_selection.rs`, shared selection/context operations and resident appearance | XR source uses prepared selection; `xr_tools.rs` routes ray/window selection and context actions; retained masks/handles draw the selection. A01/A02 close the confirmed display gaps. |
| 2 | Shared display, inference state, guide definitions and `ReferenceFade` semantics | `floor_grid`, `world_lines`, `world_inference` and retained picking supply grid/axes/guide/reference routes. A09-A11/A13 restore marker and styling cues; dynamic guide spans use GPU uniforms. |
| 3 | `drawing_points`, shared modeling previews/selection, precision and rotation geometry | `viewport_overlays.rs` prepares native tool feedback; world-ray tools keep authoritative commits. A02/A07-A09 restore preview selection, Flip and anchor/reference cues; A14 supplies copied guides. |
| 4 | `measurement_tools.rs`, `Dimension::layout`, shared units/guide/exact-value commits | Native ray/offset adapters feed shared measurement operations; retained overlays and `world_text` consume the same layout/value data. A04 supplies current-step prompts. |
| 5 | `text_tools.rs`, group marker data and shared text editor | `world_text` consumes saved labels/layout and editor state; native annotation picking routes to shared edits. A05/A08 restore arrowheads and placement anchors. Saved text retains monospace 12 / 320-point wrapping. |
| 6 | `display_scene` effective display/section cache and shared resident renderer | Native eyes use explicit runtime views, shared cap triangles/owners, retained cap picking and GPU environment/shadows. Section handles use shared anchors/offset operations; A01/A04 cover confirmed display/action cues. |
| 7 | Evaluated joinery previews, cut regions/marker anchors, grain cache and solid commands | Native snapshots include evaluated geometry; reference overlays and controller handles consume shared joint data/permissions. A06/A07 restore targeted warnings and Flip cues. Automatic Fasteners stays disabled. |
| 8 | `ui_with_host`, shared menus/inspectors/modals, file workers and clipboard commands | Immersive workspace replaces the duplicate model canvas; frame-scoped host routing supplies native View/Paste actions, keyboard and chooser bridge. A03/A04/A12 remove misleading instructions and desktop-camera paste seeds. |

Clipboard-specific reconciliation includes the desktop's explicit dimension,
text and construction-guide ghost loops. Native paste text/guide instances use
the same visibility/units and a shared paste transform. Ghost group names remain
suppressed because desktop group markers describe the main render model only.
Screenshots/reports retain their explicit output-camera sizing; additional native
window commands remain the separately recorded future work.

## Current architecture and boundaries

1. `src/xr/runtime.rs` and the `openxr_diagnostics` example inspect the loader,
   runtime/headset, extensions, tracking, stereo views and Vulkan requirements.
   Default desktop/web startup remains independent of the optional OpenXR loader.
2. `src/xr/panel.rs` owns sessions, actions/frame integration, borrowed Vulkan/wgpu
   swapchain images and quad/stereo submission. Ownership/layout/synchronization
   must remain explicit; no CPU readback is used to get the editor into XR.
3. `src/xr/editor.rs` hosts the existing application and egui context offscreen.
   `app_host.rs` supplies shared host services and persistent XR preferences.
4. `src/xr/desktop.rs` keeps a responsive native window on the main thread and XR
   work on a worker. It forwards native keyboard/text/IME/clipboard services and
   shared Close/Quit. The empty window is a lifetime surface, not a scene mirror.
5. `src/xr/immersive.rs` renders explicit eye views from the evaluated model,
   retained renderer caches and shared display/tool state. Desktop cameras are not
   installed on the document to implement world-ray modeling. Shared
   `prepare_render_model` supplies both paint and XR directly; XR no longer needs
   a preceding 2D paint to refresh evaluated geometry. Scoped snapshots carry
   editing context, and labels/eye geometry consume the same staged frame source.
6. `viewport_overlays.rs`, `world_lines.rs`, `world_text.rs`, `world_surfaces.rs`,
   `floor_grid.rs` and `backdrop.rs` describe/render spatial feedback. Both eyes
   share font layout and a head basis. GPU shaders handle projection, clipping,
   line/marker sizing and readable label sizing. Head motion does not rebuild
   the scene or evaluate every connection.
7. `xr_tools.rs` routes world rays into shared document/modeling/measurement
   operations. `world_picking.rs` retains edge/guide hierarchies;
   `world_labels.rs` retains annotation hit layouts;
   `world_inference.rs` implements bounded model-space reference inference.
   CPU work is authoritative interaction and incremental preparation, not dense
   per-eye projection. XR grid refinement uses the exact picked edge/face.
8. `navigation.rs` applies a session-local model-to-tracking transform. Both grips
   translate, rotate and scale the scene; Keep upright defaults on and restricts
   rotation to yaw while retaining vertical translation and scaling. Navigation
   reserves pointing input and requires release-to-rearm after loss/cancellation.
   Fit 3D and Fit selection reframe the immersive placement from scene/selected
   rendered bounds; they do not move the desktop camera. Shared View controls
   now route to the same placement system. Standard views turn the scene using
   desktop right/up bases without resizing or moving its target; Center origin
   preserves physical scale, and Reset view restores the upright starting pose.

## Spatial parity inventory

Implemented means present in source and the built native preview. It does not
mean every display mode or interaction has been visually confirmed in the headset.
The numbered rows are the checklist for the replacement milestone.

| # | Area | Implemented | Remaining audit/gap |
|---|---|---|---|
| 1 | Selection and context | Shared selection/hover, replace/toggle ray selection, enclosing windows and direct group/text/context shortcuts; context bounds, endpoint/edge cues; shared Edit-menu context actions; held-trigger erase with bounded ray interpolation and one undo | Controller window/context/cancellation acceptance and extreme/near-field sweep coverage remain open |
| 2 | Axes, grid and inference | Saved axis colors, origin dot, directional styling and GPU depth fade; analytic finite/infinite GPU grid; selected/hovered construction lines/points; endpoint/midpoint/face-center/axis/plane targets; guide intersections; acquired displayed-edge references, 3D edge-constrained guide pairs and locked-axis projection; persisted X/Y/Z and Unlock controls in XR, including two-axis planes | Controller lock/reference, guide-state and display/workflow acceptance remains open |
| 3 | Drawing/modeling feedback | Shared drawing previews; move/scale directions and source markers; rotation/protractor graphics; numeric values/errors; retained GPU paste ghost including copied guides; flip-plane hover; host-aware lock refresh, shared precision commits and prepared copy/cap preview selection | Preview-selection, copied-guide placement and exact-value/controller acceptance remains open |
| 4 | Dimensions and measurements | Saved dimensions, values/units, leaders/extension lines, ticks; Tape/Protractor/Dimensions previews; direct label/stroke picking; shared commit/guide/exact-value routes and current-step prompts | Source routes reconciled; rendered placement/occlusion/formatting and controller editing acceptance remain open |
| 5 | Names and attached text | Group labels with owner-aware GPU face/cap visibility; stroke depth no longer clips glyphs; saved text and leaders with retained attachment arrowheads (A05); shared-basis glyphs; world text creation and editing | Source placement/edit/layout routes reconciled after A05; compare angular readability and layered occluders behind owner surfaces; headset text-entry acceptance pending |
| 6 | Sections and display/environment | Shared effective display, face styles/X-Ray and derived GPU Move transparency (A01), hidden/profile/depth/endpoint/extension cues, fog, cached shadows; GPU sky/ground/horizon/sun; clipped faces, caps/contours, retained cap picking with scoped body ownership and controller section handles | Shared cap/render/picker and section settings/handle routes inspected; nested/scoped, overlapping-cap and display-combination acceptance remains open |
| 7 | Joinery, fasteners and solids | Evaluated geometry, connection axes/markers/arrows, controller joint edit/warning icons with targeted diagnostics (A06) and shared editor action, owner-aware guides, retained grain feedback, solid operand feedback and shared boolean/flip commands with matched outlines (A07) | Controller and display-combination acceptance remains open. Fastener entry-face controls stay deferred under the disabled gate; original-face paint restrictions match the shared desktop operation |
| 8 | Instructions and tool UI | Mode/phase/target-specific HUD/status/errors, inference labels/vector glyphs; docked readable controls; shared inspectors/modals; native keyboard input; shared View framing and standard directions target XR placement; host-aware Paste/library seeds and native chooser routing | End-to-end controller acceptance of recent lock/view/file/clipboard workflows remains pending |

The object-length axis diagram in `entity_gizmo.rs` is inspector UI, not a spatial
manipulation gizmo. Classify remaining painter paths by function rather than filename.

## Implemented interaction coverage and limits

| # | Interaction | Current behavior and limits |
|---|---|---|
| 1 | Selection and navigation | World hover/replace/toggle and enclosing-window selection; double-trigger group/text editing and empty-click context exit; saved Add/remove selection control and native Shift; model-space loose-edge picking; two-grip translation/yaw/scale with optional full rotation; Fit 3D, Fit selection and recenter; UI/modal/grip ownership and cancellation |
| 2 | Drawing | Line, Rectangle, Circle, Polygon and trigger-held Freehand share previews, document commits and undo; bounded Freehand sampling; empty projects have a usable initial drawing space |
| 3 | Measurement and construction | Tape Measure, Protractor and Dimensions; saved guide-mode toggle and live guide preview; finite rendered guide spans used for picking; guide points and crossings; reference axes/locked-coordinate projections |
| 4 | Manipulation | Move, Push/Pull, Scale, Offset and Rotate use shared modeling state and commits, with default trigger grabs or optional two-click Exact value entry; Move, Scale and Rotate have stationary geometry/reference targets and shared grid refinement. Scale preserves visible-target/exact-ratio behavior; Rotate supports in-plane targets and off-plane projection without changing its axis. Original-document guards reject unsupported evaluated-only targets. Push/Pull and Offset preserve exact target distances and face/lock-aware free input; tool/display/workflow acceptance remains open after the C16 source audit |
| 5 | Paint and erase | Face/group paint; held-trigger erase interpolates bounded adjacent rays, previews accumulated targets and commits once on release; cancel/loss/keyboard commands discard it. Thin/near targets at the sample limit and evaluated-only paint remain open |
| 6 | Text and annotations | Text anchor/leader placement and shared editor modal; saved text/dimension labels and strokes can be selected, erased or transformed; Text opens an editable saved text; section-clipped and face-occluded targets are filtered |
| 7 | Solids and grain | Shared solid validation/operand order and boolean history; Flip uses the displayed quads for hover/click; Grain Direction uses shared part/reference validation and assignment |
| 8 | Clipboard/history | Copy/Paste ghost and placement; trigger commit and Cancel; shared Undo/Redo. Paste snapping does not inherit restrictions from the previous tool and flip handles do not capture it |

Guide intersection inference is bounded to 16 nearby candidates and eight exact
point targets (at most 120 pairs); dense clusters may omit a candidate. World
reference acquisition retains the hit original edge. A separate retained global
endpoint/midpoint query now retains up to 24 distinct visible guide lines per
world axis. Start/origin guides compete within that same shared desktop limit
before compatible intersections are combined on the current working plane.
The drawing/measurement callback queries the native scene's visibility caches;
it does not project the desktop camera or scan all references per frame.
edge snaps require a true 3D guide-pair intersection on the picked segment. Face refinements
validate containment/visibility and try secondary candidates when the closest
intersection falls outside the targeted face or into a hole.
Move now uses the same immutable, exclusion-filtered target snapshot as desktop,
with world indexes retained for the gesture rather than rebuilt for its preview.
Scale shares those targets with visible-face/section-cap occlusion and the saved
X-ray preference; exact snapped ratios are preserved with grid snapping enabled.
Stationary modeling targets independently honour the existing edge, face and
face-centre snap preferences; disabling a face target does not disable centres.
Rotate now uses stationary visible targets on its chosen plane, including
arbitrary plane normals. Eligible off-plane geometry can define the angle by
projection without changing the axis. Ray/plane free rotation, shared grid
refinement and a parallel-ray depth fallback preserve the native input route.
Push/Pull now projects fixed original-model targets into its extrusion direction,
with a starting-surface guard and the free depth fallback. Offset queries the
original face plane or explicit lock, retaining tilt/elevation and exact target
distances through shared grid refinement. All five modeling tools now have
geometry/reference input routes; C15 and the C16 source audit are implemented.
XR grid refinement no longer scans all edges/faces; containment
still tests the picked face's topology. These are explicit limits, not performance
or complete-parity claims.

## Remaining implementation and acceptance work

1. Exercise the eight parity rows against the recorded acceptance matrix and fix
   newly demonstrated gaps. The source audit and its fourteen confirmed coding
   findings are closed; source reconciliation alone does not establish full
   rendered/workflow equivalence.
2. Verify that the native routes work without a required 2D model-view fallback.
   Evaluated scene preparation, shared View framing and standard directions,
   selection/context handles and clipboard placement now have immersive routes.
   Remaining checks include display/workflow combinations, near-field/fast sweep
   sampling limits, unsupported evaluated targets and nested/scoped cap interaction.
   Current-ray source guards now cover all native world tools; live project/undo/
   context transition and capture acceptance still needs the broader matrix.
   Preserve shared operations, bounded queries and retained GPU feedback.
3. The immersive-first layout is now implemented: when 3D view is enabled, the
   panel hosts the shared inspector, context controls, menus and numeric status,
   plus the existing XR tool picker. The 2D model canvas and desktop camera/tool
   rails are not drawn or given viewport input. Shared inspector state/keys are
   preserved. Turning 3D view off restores the original panel-only editor; normal
   desktop/web hosts retain their original layout. Full parity is not yet proven.

4. Keep current builds and documents synchronized as milestones land. Use source
   review, appropriate narrow math/regression checks and existing hardware snapshot
   paths when needed. Continue implementation while human confirmation is pending;
   only pause for a human test when the user requests one.
5. Complete the broader acceptance work below as appropriate. Only claim
   verified parity or phase completion based on the recorded evidence.

## Evidence and acceptance gates

1. The user has confirmed the editor quad is visible, upright/readable and stable
   under head movement; controller cursor/clicks and scrolling work; Closer/Text +
   and Recenter work. Basic immersive viewing and two-grip navigation received
   positive feedback. This is not a complete overlay/tool/display comparison.
2. Native builds pass for the immersive-first panel. Earlier isolated RTX 4090
   Vulkan images show its inspector/controls without a duplicate model canvas and
   the clean 404-face model in the eye path. These are layout/rendering checks;
   headset confirmation of the new layout and selected-target framing is not recorded. Two focused application tests prove edit refresh without 2D painting
   and group-scope propagation with stable geometry snapshots. An isolated clean-
   example eye snapshot renders 404 faces on the RTX 4090 Vulkan path and was
   inspected. Two erase regressions and the immediate-save preference regression
   also pass. Fifteen isolated picking/reference checks and the focused grid
   regression remain recorded. These checks do not constitute a full suite or
   headset confirmation of all recent interactions; broad validation is deferred.

   Renderer build `138d7f4` also passed three isolated RTX 4090 Vulkan snapshots
   without OpenXR: clean Example and matched Guides off/on fixtures, each with
   404 faces. The inspected guide image shows the dashed line and point cross;
   the pair differs by 2,216 pixels (guide-colour pixels: 0 off, 852 on). This
   exercises the new GPU uniform/guide branch in the main scene. It does not prove
   OpenXR device/swapchain interoperability or controller clipboard placement.

   Prepared build `591c117` independently renders a three-leader saved-text
   fixture on RTX 4090 Vulkan, with all 404 Example faces loaded and no
   error/warning/panic output. The inspected image shows upright readable labels
   and open arrowheads pointing toward their anchors in three screen directions.
   Evidence: `.local/openxr/annotation-render/front/renderer-check.json` and
   `eye.png`. This uses one diagnostic eye and unattached saved text; it does not
   accept live/clipboard text, stereo, occlusion policy or headset readability.

   A focused application/file-worker integration check also passes Save As,
   edit/undo, save to the existing path, New and Open through the XR dialog host,
   including guide/text persistence and restored undo/redo history. Immediate
   axis/plane preference save/restore without a model edit passes against the
   current test build. These are software workflow checks; controller chooser,
   virtual typing and restart acceptance remain separate.

3. Phase 1 acceptance remains open: complete open/edit/undo/save/reopen workflow,
   controller text/file workflow acceptance, capture/loss/restart/disconnect recovery,
   immediate preference restoration and clean graphics validation. Historical
   validation findings are in `OPENXR_WINDOWS.md`; they have not been cleared by
   the current validation-disabled previews.
   A current isolated validation-enabled snapshot of build `138d7f4` reproduces
   10 reported shader errors (the default duplicate cap), all
   `VUID-StandaloneSpirv-None-10684`, with the Khronos layer
   loaded. An isolated compiler-patch build removes those errors with the layer
   loaded and produces a 404-face Example snapshot differing in only 3 of
   1,296,000 pixels by at most 1/255. The workspace
   now pins Naga and its shared types to the reviewed immutable fix; wgpu/HAL
   stay on the registry release. This clears the reproduced compiler failure in
   that offscreen path, not the separate OpenXR runtime image/resource findings.
4. Phase 2 acceptance remains open: immersive/desktop/panel document and tool
   equivalence; all eight spatial parity rows; no required 2D model-view fallback;
   focus/tracking/reference-space changes and scale extremes. Latest desktop/web
   hardware compatibility has not been revalidated in this functionality pass.
5. Large-scene timing on Rooftop, CPU/GPU/memory/compositor measurements and broad
   backend checks remain deferred. No confirmed Rooftop fixture path is recorded;
   obtain a suitable fixture before that gate rather than substituting a small
   example or claiming measured performance from code inspection.

## Controller-attached UI direction

1. User direction (2026-11-11): once the 2D controls are compact enough, the UI
   becomes a panel attached to the non-dominant controller. The immersive scene
   remains in world space. This is the intended UI destination after compacting
   the current desktop-derived controls, not an accepted capability of this build.
2. Reuse the shared app commands, preferences, modal and keyboard routes while
   reducing the panel layout. Keep the dominant controller available to point at
   the panel and interact with the scene. Handedness and any new placement or
   layout preferences must persist through the existing shared preference path.
   C18 starts this reduction with readable wrapping control rows and a saved
   View and comfort disclosure. C19 adds an explicit controller-attachment option;
   world placement remains the default while the new path awaits acceptance.
3. Before replacing the current world panel, implement tracked non-dominant-hand
   anchoring and explicitly handle tracking loss, focus changes and two-grip
   scene navigation. Verify readability and reachability of menus, modals and
   text entry on the compact panel. The C19 path follows the non-dominant grip,
   inherits its position and rotation directly from the grip space (C25), with no head-facing rotation or grip freeze. Losing
   the host hand returns to a recentered world panel and cancels input captures.
   Recenter explicitly detaches and persists world placement. These routes need
   headset verification before replacing the default interaction surface.

## Stereo rendering decision and later optimization

1. Retain two explicit eye passes now. Both eyes share retained scene geometry,
   authoritative tool state, overlay/font data and a common billboard layout.
   Multiview/single-pass instancing is deliberately deferred, not rejected.
2. After functionality, examine batching panel/eye submissions and completion
   waits while preserving borrowed-image lifetime, layout and ownership guarantees.
   Measure submission, vertex/visibility and fragment costs on representative
   scenes before setting multiview priority.
3. A later multiview implementation needs capability negotiation, layered
   swapchain/depth/visibility resources and view-indexed geometry, overlay and text
   shaders. Keep the two-pass path as a backend fallback.
4. Desktop mirroring, additional native window commands, extended virtual keyboard input,
   standalone Quest packaging, Steam Frame support and broader platform adaptation are
   future work. The current target is Windows + SteamVR + Vulkan on the RTX 4090.

## References

1. [Khronos OpenXR tutorial](https://openxr-tutorial.com/index.html)
2. [OpenXR quad layers](https://registry.khronos.org/OpenXR/specs/1.1/man/html/XrCompositionLayerQuad.html)
3. [Khronos frame submission guide](https://github.com/KhronosGroup/OpenXR-Guide/blob/main/chapters/frame_submission.md)
4. [wgpu native device interoperability](https://docs.rs/wgpu/30.0.1/wgpu/struct.Device.html)
5. [Windows/Vulkan OpenXR setup](https://www.openxr-tutorial.com/windows/vulkan/1-introduction.html)
6. [Khronos multiview guide](https://docs.vulkan.org/tutorial/latest/OpenXR_Vulkan_Spatial_Computing/08_Slang_Spatial_Shaders/04_incorporating_into_the_engine.html)
7. [wgpu MULTIVIEW](https://docs.rs/wgpu/30.0.1/wgpu/struct.Features.html#associatedconstant.MULTIVIEW)
8. [OpenXR reference-space change timing and origin transforms](https://registry.khronos.org/OpenXR/specs/1.1/man/html/XrEventDataReferenceSpaceChangePending.html)
