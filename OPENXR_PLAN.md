# BenchUp OpenXR implementation plan

Updated: 2026-11-10. Implementation baseline: `404a8f8` on `feat/openxr-phase1`.
This is the current plan, not a chronological implementation log. Operational
commands and the latest continuation state belong in [OPENXR_HANDOFF.md](OPENXR_HANDOFF.md).

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
| 1A: panel/input foundation | Metric panel geometry; ray mapping; OpenXR actions; capture and focus/tracking cancellation | Complete drag/loss/restart coverage on the headset |
| 1B: Vulkan/wgpu interoperability | Runtime preflight; runtime-selected Vulkan GPU/device; borrowed swapchain images; GPU quad/stereo rendering; XR frame lifecycle | Clean graphics validation and the full restart/loss matrix remain open |
| 1C: complete editor panel | Shared editor UI and GPU viewport callbacks rendered into an OpenXR quad | Desktop mirroring and complete host-service verification remain open |
| 1D: interaction/usability | Controller cursor/click/scroll; comfort/recenter controls; physical keyboard/IME/clipboard bridge; native lifetime window; shared Close/Quit path; saved preferences | In-headset virtual keyboard, native file-dialog access, other window commands and end-to-end project workflow verification |
| 2A: shared scene/overlay preparation | Evaluated-model GPU scene; shared model-space overlay descriptions; retained references, glyphs and previews | Finish auditing desktop-only dependencies; duplicate 2D viewport still present |
| 2B: stereo scene | Explicit eye poses/projections; shared geometry across two eye passes; GPU environment, grid, shadows and section rendering | Visual equivalence not established; validation/performance gates remain deferred |
| 2C: immersive editing | World-ray selection, broad tool coverage, annotation picking, guides/reference inference, upright two-grip navigation | Remaining inference/tool/handle parity and final replacement of the 2D model view |

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
   installed on the document to implement world-ray modeling.
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

## Spatial parity inventory

Implemented means present in source and the built native preview. It does not
mean every display mode or interaction has been visually confirmed in the headset.
The numbered rows are the checklist for the replacement milestone.

| # | Area | Implemented | Remaining audit/gap |
|---|---|---|---|
| 1 | Selection and context | Shared selection/hover, eraser/outliner/joinery colors, context bounds, endpoint/edge cues; world entity picking | Audit editing-context entry/exit, context actions and all remaining selection gestures without the 2D viewport |
| 2 | Axes, grid and inference | Saved axis colors; analytic finite/infinite GPU grid; construction lines/points; endpoint/midpoint/face-center/axis/plane targets; guide intersections; acquired edge references and locked-axis projection | Broader off-face/global alignment; references to evaluated-only edges; prove every desktop lock/precision workflow has an XR route |
| 3 | Drawing/modeling feedback | Shared drawing previews; move/scale directions; rotation/protractor graphics; numeric values/errors; retained GPU paste ghost; flip-plane hover | Audit every desktop handle/preview and precision/cancel path against its XR route |
| 4 | Dimensions and measurements | Saved dimensions, values/units, leaders/extension lines, ticks; Tape/Protractor/Dimensions previews; direct label/stroke picking | Compare placement, occlusion, formatting and editing workflows across hosts |
| 5 | Names and attached text | Group labels with owner-aware GPU visibility; saved text and leaders; readable shared-basis glyphs; world text creation and existing-text editing | Compare wrapping/appearance and visibility; virtual keyboard remains absent |
| 6 | Sections and display/environment | Shared effective display, face styles/X-Ray, hidden/profile/depth/endpoint/extension cues, fog, cached shadows; GPU sky/ground/horizon/sun; clipped faces, caps/contours and controller section handles | Original-face ray hierarchy does not cover cap-only picking/occlusion; audit settings/handle parity and display combinations |
| 7 | Joinery, fasteners and solids | Evaluated geometry, connection axes/markers/arrows, joint edit/warning glyphs, owner-aware guides, solid operand feedback and shared boolean/flip commands | Audit all desktop-supported target/edit gestures in immersive space; evaluated-only paint targets remain restricted |
| 8 | Instructions and tool UI | World HUD/status/errors, inference labels/vector glyphs; docked readable controls; shared inspectors/modals; native keyboard input | Audit reachable commands, numeric/lock input and native dialogs; complete removal of the duplicate 2D model view |

The object-length axis diagram in `entity_gizmo.rs` is inspector UI, not a spatial
manipulation gizmo. Classify remaining painter paths by function rather than filename.

## Implemented interaction coverage and limits

| # | Interaction | Current behavior and limits |
|---|---|---|
| 1 | Selection and navigation | World hover/select; model-space loose-edge picking; two-grip translation/yaw/scale with optional full rotation; Fit 3D and recenter; UI/modal/grip ownership and cancellation |
| 2 | Drawing | Line, Rectangle, Circle, Polygon and trigger-held Freehand share previews, document commits and undo; bounded Freehand sampling; empty projects have a usable initial drawing space |
| 3 | Measurement and construction | Tape Measure, Protractor and Dimensions; saved guide-mode toggle and live guide preview; finite rendered guide spans used for picking; guide points and crossings; reference axes/locked-coordinate projections |
| 4 | Manipulation | Move, Push/Pull, Scale, Offset and Rotate trigger grabs use shared modeling state and commits; original-document guards reject unsupported evaluated-only targets |
| 5 | Paint and erase | Face/group paint and entity erase clicks; continuous erase sweeps and painting evaluated-only generated faces remain unimplemented |
| 6 | Text and annotations | Text anchor/leader placement and shared editor modal; saved text/dimension labels and strokes can be selected, erased or transformed; Text opens an editable saved text; section-clipped and face-occluded targets are filtered |
| 7 | Solids and grain | Shared solid validation/operand order and boolean history; Flip uses the displayed quads for hover/click; Grain Direction uses shared part/reference validation and assignment |
| 8 | Clipboard/history | Copy/Paste ghost and placement; trigger commit and Cancel; shared Undo/Redo. Paste snapping does not inherit restrictions from the previous tool and flip handles do not capture it |

Guide intersection inference is bounded to 16 nearby candidates and eight exact
point targets (at most 120 pairs); dense clusters may omit a candidate. World
references currently acquire the hit original edge, not an unrestricted global
reference search. XR grid refinement no longer scans all edges/faces; containment
still tests the picked face's topology. These are explicit limits, not performance
or complete-parity claims.

## Next work, in order

1. Reconcile the eight parity rows against actual desktop consumers and XR input
   routes. Record concrete missing cases, especially editing context/context
   actions, remaining handles, precision/locks, joinery/fastener targeting and
   display combinations. Do not substitute repeated minor snap refinements for
   this replacement-viewport checklist.
2. Close the identified dependencies on the 2D model view. Known work includes
   broader reference/alignment behavior, continuous erase, safe handling of
   unsupported evaluated targets and cap-only interaction/occlusion semantics.
   Preserve shared operations, bounded queries and retained GPU feedback.
3. Make the immersive view the XR model viewport once the audit shows the
   supported workflows no longer depend on the duplicate view. Remove that
   duplicate from the XR panel; retain toolbars, inspectors and dialogs and keep
   normal desktop/web viewports supported. This milestone is not implemented yet.
4. Keep current builds and documents synchronized as milestones land. Use source
   review, appropriate narrow math/regression checks and existing hardware snapshot
   paths when needed. Continue implementation while human confirmation is pending;
   only pause for a human test when the user requests one.
5. Resume the broader acceptance work below when requested. Only then claim
   verified parity or phase completion based on the recorded evidence.

## Evidence and acceptance gates

1. The user has confirmed the editor quad is visible, upright/readable and stable
   under head movement; controller cursor/clicks and scrolling work; Closer/Text +
   and Recenter work. Basic immersive viewing and two-grip navigation received
   positive feedback. This is not a complete overlay/tool/display comparison.
2. Native builds and previous RTX 4090 Vulkan panel/eye snapshots establish working
   rendering paths. The latest native OpenXR editor build passes for `404a8f8`.
   Fifteen isolated picking/reference checks and a separate focused grid regression
   pass. These do not constitute a full application suite or headset validation
   of recent annotations, references, sections, flip and grid changes.
3. Phase 1 acceptance remains open: complete open/edit/undo/save/reopen workflow,
   usable text and file-dialog access, capture/loss/restart/disconnect recovery,
   immediate preference restoration and clean graphics validation. Historical
   validation findings are in `OPENXR_WINDOWS.md`; they have not been cleared by
   the current validation-disabled previews.
4. Phase 2 acceptance remains open: immersive/desktop/panel document and tool
   equivalence; all eight spatial parity rows; no required 2D model-view fallback;
   focus/tracking/reference-space changes and scale extremes. Latest desktop/web
   hardware compatibility has not been revalidated in this functionality pass.
5. Large-scene timing on Rooftop, CPU/GPU/memory/compositor measurements and broad
   backend checks remain deferred. No confirmed Rooftop fixture path is recorded;
   obtain a suitable fixture before that gate rather than substituting a small
   example or claiming measured performance from code inspection.

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
4. Desktop mirroring, additional native window commands, virtual keyboard input,
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
