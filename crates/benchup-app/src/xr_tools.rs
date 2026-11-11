//! Model-space controller tools. Document operations and previews remain shared.
use super::*;
use benchup_core::Ray;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) struct Hit {
    pub entity: Option<u64>,
    pub section: bool,
    pub face: Option<u64>,
    pub edge: Option<u64>,
    pub guide: Option<(DVec3, DVec3)>,
    pub guide_points: [Option<DVec3>; 8],
    pub flip: Option<Axis>,
    pub annotation: Option<u64>,
    pub edge_targets: [Option<DVec3>; 3],
    pub point: DVec3,
    pub normal: DVec3,
    pub center: Option<DVec3>,
}

impl Hit {
    fn contains(&self, model: &Model, point: DVec3) -> bool {
        if let Some((a, b)) = self.guide {
            let segment = b - a;
            let t = if segment.length_squared() > 1e-24 {
                (point - a).dot(segment) / segment.length_squared()
            } else { 0. };
            point.distance(a + segment * t.clamp(0., 1.)) < 1e-7
        } else if let Some(edge) = self.edge.and_then(|edge| model.edges.get(&edge)) {
            let (a, b) = model.points(edge);
            let direction = b - a;
            let t = (point - a).dot(direction) / direction.length_squared();
            point.distance(a + direction * t.clamp(0., 1.)) < 1e-7
        } else {
            (point - self.point).dot(self.normal).abs() < 1e-6
        }
    }
    fn inference(&self) -> Inference {
        if self.section {
            Inference::Plane
        } else if let Some((a,b)) = self.guide {
            if a == b { Inference::GuidePoint } else { Inference::OnGuide }
        } else if self.edge.is_some() {
            Inference::OnEdge
        } else {
            Inference::OnFace
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InputStamp {
    document: u64,
    pub(crate) project: u64,
    context: Option<u64>,
    tool: Tool,
    display: benchup_core::DisplaySettings,
}

pub(crate) struct Pointer {
    // Production pointers must carry a source stamp. Unit fixtures may omit it
    // when exercising model-space math independently of the rendering host.
    pub stamp: Option<InputStamp>,
    pub handle: Option<(super::viewport_overlays::HandleAction, DVec3)>,
    pub view: Option<super::xr_window_selection::View>,
    pub ray: Ray,
    pub model: Arc<Model>,
    pub hit: Option<Hit>,
    pub activate: bool,
    pub cancel: bool,
    pub primary_down: bool,
    pub visible_reference: Option<DVec3>,
}

#[derive(Default)]
pub(crate) struct State {
    active: bool,
    tool: Option<Tool>,
    line_normal: Option<DVec3>,
    stroke: bool,
    grab_depth: Option<f64>,
    section: Option<SectionGrab>,
    pointer_target: Option<DVec3>,
    handle_hint: Option<&'static str>,
    window: Option<WindowGesture>,
    selection_click: Option<SelectionClick>,
    move_targets: Option<crate::xr::world_move::Targets>,
    model_plane: Option<DVec3>,
    reference: Option<drawing_inference::DrawingReference>,
    erasing: bool,
    erase_revision: Option<(u64, Option<u64>)>,
    erase_sweep: Option<EraseTargets>,
    panel_hover: bool,
    precision_target: Option<PrecisionTarget>,
    model_phase: Option<ModelPhase>,
    request_precision_input: bool,
}

struct EraseTargets {
    revision: (u64, Option<u64>),
    ids: BTreeSet<u64>,
}

struct WindowGesture {
    frame: super::xr_window_selection::Window,
    revision: (u64, u64, Option<u64>),
}

const WINDOW_HINT: &str = "Hold trigger and drag a window. Release selects enclosed entities. Cancel discards the window.";

struct SelectionClick {
    entity: u64,
    time: f64,
    stamp: InputStamp,
    transform: Option<glam::DMat4>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ModelPhase { Aiming, Typing }

// A panel may retain only a target acquired for this exact pending operation.
// Geometry/context edits and advancing a chained tool invalidate it immediately.
#[derive(PartialEq)]
struct PrecisionTarget {
    revision: u64,
    context: Option<u64>,
    tool: Tool,
    lock: Option<Axis>,
    second_lock: Option<Axis>,
    line_start: Option<DVec3>,
    drawing: Option<(DVec3, DVec3, usize, Option<DVec3>)>,
    measurement: Option<(Vec<DVec3>, bool, Option<u64>)>,
    modeling: Option<ModelingTarget>,
}

#[derive(PartialEq)]
struct ModelingTarget {
    anchor: DVec3,
    direction: DVec3,
    center: DVec3,
    face: Option<u64>,
    value: f64,
    copy: bool,
    keep_starting_face: bool,
    rotation: Option<(DVec3, Option<DVec3>)>,
}

struct SectionGrab {
    depth: f64,
    start_tip: DVec3,
}

impl State {
    pub(crate) fn capturing(&self) -> bool {
        self.erasing || self.stroke || self.grab_depth.is_some() || self.section.is_some() || self.window.is_some()
    }
    pub(crate) fn handle_hint(&self) -> Option<&'static str> { self.handle_hint }
    pub(crate) fn cancel_window(&mut self) -> bool {
        let cancelled = self.window.take().is_some();
        if cancelled { self.cancel_selection_click(); }
        cancelled
    }
    pub(crate) fn cancel_selection_click(&mut self) { self.selection_click = None; }
    pub(crate) fn append_window_overlay(&self, overlays: &mut super::viewport_overlays::Overlays) {
        if let Some(window) = &self.window {
            let quad = window.frame.quad();
            for i in 0..4 {
                overlays.overlay_segments.push(super::viewport_overlays::Segment {
                    a: quad[i], b: quad[(i + 1) % 4], color: Color32::from_rgb(65, 160, 245), width: 2., dashed: true,
                });
            }
        }
    }
    pub(crate) fn take_precision_input_request(&mut self) -> bool {
        std::mem::take(&mut self.request_precision_input)
    }

    fn cancel_precision_modeling(&mut self, app: &mut BenchUpApp) {
        if self.model_phase.take().is_some() {
            app.modeling = None;
            app.length_input.clear();
            self.grab_depth = None;
        }
        self.request_precision_input = false;
    }
    pub(crate) fn set_panel_hover(&mut self, hover: bool) {
        self.panel_hover = hover;
    }
    pub(crate) fn cancel_erasing(&mut self, app: &mut BenchUpApp) {
        self.erasing = false;
        self.erase_revision = None;
        self.erase_sweep = None;
        app.xr_erasing.clear();
    }

    pub(crate) fn pointer_target(&self) -> Option<DVec3> {
        self.pointer_target
    }
}

pub(crate) fn hint(tool: Tool, precise_modeling: bool) -> &'static str {
    if precise_modeling && matches!(tool, Tool::Move | Tool::Rotate | Tool::Scale | Tool::PushPull | Tool::Offset) {
        return "Trigger: anchor, then target. Type an exact value and press Enter. Cancel discards.";
    }
    match tool {
        Tool::Line => "Trigger: start / next endpoint. Cancel ends the chain.",
        Tool::Rectangle => "Trigger: first corner, then opposite corner. Cancel clears.",
        Tool::Circle | Tool::Polygon => "Trigger: centre, then radius. Cancel clears.",
        Tool::Freehand => "Hold trigger to draw; release to finish. Cancel discards the stroke.",
        Tool::Move => {
            "Hold trigger on the selection and move it; release to commit. Cancel discards."
        }
        Tool::PushPull => "Hold trigger on a face and pull it; release to commit. Cancel discards.",
        Tool::Scale => "Hold trigger away from the selection centre, move to resize, then release.",
        Tool::Rotate => {
            "Hold trigger away from the selection centre, move around it, then release."
        }
        Tool::Offset => "Hold trigger near a face edge, move inward or outward, then release.",
        Tool::TapeMeasure => "Trigger: first point, then second point. Cancel clears.",
        Tool::Protractor => "Trigger: centre, baseline, then angle. Cancel clears.",
        Tool::Dimensions => "Trigger: two endpoints, then annotation position.",
        Tool::Text => "Trigger: attach, then label position. Edit text on the panel.",
        Tool::GrainDirection => {
            "Trigger: choose a part and starting point, then the grain direction."
        }
        Tool::Select => "Trigger selects. Double trigger opens a group or text. Empty space leaves the editing context. Add/remove selection toggles targets. Cancel clears selection.",
        Tool::Paint => "Trigger paints a face or group with the chosen colour. Undo restores it.",
        Tool::Erase => "Hold trigger over entities; release to erase. Cancel discards the stroke.",
        Tool::Trim => "Trigger: choose the cutter, then the target body. Cancel clears operands.",
        Tool::OuterShell | Tool::Union | Tool::Intersect | Tool::Split => {
            "Trigger: choose the first body, then the second. Cancel clears operands."
        }
        Tool::Flip => {
            "Trigger: choose a body, then one of its coloured flip planes. Undo restores it."
        }
        Tool::Orbit | Tool::Walk => "Hold both grips to move, turn and scale the scene. Choose an editing tool on the panel.",
    }
}

fn plane_point(ray: Ray, origin: DVec3, normal: DVec3) -> Option<DVec3> {
    let denominator = ray.direction.dot(normal);
    if denominator.abs() < 1e-8 {
        return None;
    }
    let t = (origin - ray.origin).dot(normal) / denominator;
    (t > 0. && t < 1e7).then(|| ray.origin + ray.direction * t)
}

impl BenchUpApp {
    pub(super) fn xr_tool_hint(&self) -> &'static str {
        if self.tool == Tool::Select && self.xr_panel.window_selection { WINDOW_HINT }
        else { hint(self.tool, self.xr_panel.precise_modeling) }
    }

    fn xr_window_select(&mut self, pointer: &Pointer, state: &mut State, clicked: bool,
        activate: bool, ctx: &egui::Context)
    {
        self.snap = None;
        self.hover = None;
        let revision = (self.document_revision(ctx), self.xr_project_epoch, self.doc.model.edit_context);
        if clicked && pointer.primary_down && let Some(view) = pointer.view {
            state.window = super::xr_window_selection::Window::new(pointer.ray, pointer.hit.map(|hit| hit.point), view)
                .map(|frame| WindowGesture { frame, revision });
        }
        let Some(mut window) = state.window.take() else { return; };
        if window.revision != revision { return; }
        let Some(point) = pointer.view.and_then(|view| window.frame.update(pointer.ray, view)) else {
            return;
        };
        state.pointer_target = Some(point);
        state.handle_hint = Some(WINDOW_HINT);
        if pointer.primary_down || !activate { state.window = Some(window); return; }
        if window.frame.end.length() <= 4. {
            self.xr_select_click(pointer, state, ctx);
            return;
        }
        state.cancel_selection_click();
        let hits = {
            self.entity_cache.update_for_revision(&self.doc.model, revision.0);
            selection::box_select_projected_with_state(&self.doc.model, &window.frame, glam::DVec2::ZERO,
                window.frame.end, self.doc.display.guides, |id| {
                    let state = self.entity_cache.get(id);
                    (state.hidden, state.context)
                })
        };
        if self.xr_panel.extend_selection || ctx.input(|input| input.modifiers.shift) {
            for id in hits { if !self.selected.remove(&id) { self.selected.insert(id); } }
        } else { self.selected = hits; }
        ctx.request_repaint();
    }
    fn xr_select_click(&mut self, pointer: &Pointer, state: &mut State, ctx: &egui::Context) {
        let previous = state.selection_click.take();
        let hit = pointer.hit.and_then(|hit| hit.entity).filter(|id| self.doc.model.contains_entity(id));
        if self.xr_panel.extend_selection || ctx.input(|input| input.modifiers.shift) {
            if let Some(id) = hit && !self.selected.remove(&id) { self.selected.insert(id); }
        } else if let Some(id) = hit {
            let time = ctx.input(|input| input.time);
            let stamp = self.xr_input_stamp(ctx);
            let transform = pointer.view.map(|view| view.transform);
            let delay = ctx.options(|options| options.input_options.max_double_click_delay);
            let repeated = previous.is_some_and(|last| last.entity == id && last.stamp == stamp
                && last.transform == transform && time >= last.time && time - last.time < delay);
            self.selected = BTreeSet::from([id]);
            if repeated {
                if self.doc.model.texts.contains_key(&id) { self.begin_text_edit(id, ctx); }
                else if self.doc.model.objects.contains_key(&id) { self.enter_object(id, ctx); }
            } else {
                state.selection_click = Some(SelectionClick { entity: id, time, stamp, transform });
            }
        } else if self.doc.model.edit_context.is_some() {
            self.close_edit_context(ctx);
        } else { self.selected.clear(); }
        ctx.request_repaint();
    }

    pub(super) fn xr_input_stamp(&mut self, ctx: &egui::Context) -> InputStamp {
        InputStamp { document: self.document_revision(ctx), project: self.xr_project_epoch,
            context: self.doc.model.edit_context, tool: self.tool, display: self.doc.display.clone() }
    }

    fn xr_pointer_is_current(&mut self, pointer: &Pointer, ctx: &egui::Context) -> bool {
        let Some(stamp) = &pointer.stamp else { return cfg!(test); };
        stamp.project == self.xr_project_epoch && stamp.context == self.doc.model.edit_context
            && stamp.tool == self.tool && stamp.display == self.doc.display
            && stamp.document == self.document_revision(ctx)
    }

    pub(crate) fn xr_set_erase_sweep(&self, state: &mut State, ids: BTreeSet<u64>) {
        // Reuse the already validated rendered-document revision here. This is
        // outside the UI frame; validating the whole model again would be wasteful.
        state.erase_sweep = (!ids.is_empty() && !self.document_model_dirty).then(|| EraseTargets {
            revision: (self.document_model.revision(), self.doc.model.edit_context), ids,
        });
    }

    fn xr_precision_target(&mut self, ctx: &egui::Context, include_modeling: bool) -> Option<PrecisionTarget> {
        let modeling = self.modeling.as_ref().filter(|edit| include_modeling && edit.world_input).map(|edit| ModelingTarget {
            anchor: edit.anchor, direction: edit.direction, center: edit.center, face: edit.face,
            value: edit.value, copy: edit.copy, keep_starting_face: edit.keep_starting_face,
            rotation: edit.rotation.as_ref().map(|rotation| (rotation.normal, rotation.reference)),
        });
        if modeling.is_none() && (self.snap.is_none() || self.tool == Tool::Freehand || self.modeling.is_some()
            || !(self.line_start.is_some() || self.drawing.is_some() || self.measurement_precision_available()))
        {
            return None;
        }
        Some(PrecisionTarget {
            revision: self.document_revision(ctx),
            context: self.doc.model.edit_context,
            tool: self.tool,
            lock: self.lock,
            second_lock: self.second_lock,
            line_start: self.line_start,
            drawing: self.drawing.as_ref().map(|drawing| (drawing.origin, drawing.normal, drawing.points.len(), drawing.points.last().copied())),
            measurement: self.measurement.as_ref().map(|measurement| (measurement.points.clone(), measurement.complete, measurement.guide_id)),
            modeling,
        })
    }

    pub(crate) fn xr_tool_controls(
        &mut self,
        ui: &mut egui::Ui,
        storage: &mut dyn eframe::Storage,
        state: &mut State,
        preferences: &mut crate::app_host::XrPanelPreferences,
    ) {
        let ctx = ui.ctx().clone();
        crate::app_host::set_world_input(&ctx, preferences.immersive_scene);
        ui.add_enabled_ui(self.xr_scene_interaction_enabled(&ctx), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().interact_size.y = 42.;
                egui::ComboBox::from_id_salt("benchup.xr.tool_picker")
                    .width(240.)
                    .height(340.)
                    .selected_text(
                        egui::RichText::new(format!("Tool: {}", self.tool.label())).size(18.),
                    )
                    .show_ui(ui, |ui| {
                        let tools = [
                            Tool::Select,
                            Tool::Line,
                            Tool::Rectangle,
                            Tool::Circle,
                            Tool::Polygon,
                            Tool::Freehand,
                            Tool::TapeMeasure,
                            Tool::Protractor,
                            Tool::Dimensions,
                            Tool::Text,
                            Tool::Move,
                            Tool::Rotate,
                            Tool::Scale,
                            Tool::PushPull,
                            Tool::Offset,
                            Tool::Paint,
                            Tool::Erase,
                            Tool::OuterShell,
                            Tool::Union,
                            Tool::Intersect,
                            Tool::Trim,
                            Tool::Split,
                            Tool::Flip,
                            Tool::GrainDirection,
                        ];
                        for row in tools.chunks(4) {
                            ui.horizontal(|ui| {
                                for &tool in row {
                                    if ui
                                        .add_sized(
                                            [132., 42.],
                                            egui::Button::new(
                                                egui::RichText::new(tool.label()).size(18.),
                                            )
                                            .selected(self.tool == tool),
                                        )
                                        .clicked()
                                    {
                                        if tool.is_solid() {
                                            self.start_solid_tool(tool, &ctx);
                                        } else {
                                            self.set_tool(tool);
                                        }
                                        self.persist_inspector_changes(&ctx, Some(storage), false);
                                        ui.close();
                                    }
                                }
                            });
                        }
                    });
                if ui
                    .add_enabled(
                        !self.selected.is_empty(),
                        egui::Button::new(egui::RichText::new("Copy").size(18.))
                            .min_size(egui::vec2(84., 42.)),
                    )
                    .clicked()
                {
                    self.copy_selection(&ctx);
                }
                if ui
                    .add_enabled(
                        self.clipboard.is_some(),
                        egui::Button::new(egui::RichText::new("Paste").size(18.))
                            .min_size(egui::vec2(84., 42.)),
                    )
                    .clicked()
                {
                    self.context_position = None;
                    self.context_action(&ctx, ContextAction::Paste);
                }
                for (label, action) in [("Cancel", 0), ("Undo", 1), ("Redo", 2)] {
                    if ui
                        .add_sized(
                            [84., 42.],
                            egui::Button::new(egui::RichText::new(label).size(18.)),
                        )
                        .clicked()
                    {
                        let had_window = state.cancel_window();
                        state.cancel_selection_click();
                        self.xr_cancel_action(action == 0 && !had_window, &ctx, storage);
                        state.reference = None;
                        state.erasing = false;
                        if action != 0 {
                            self.undo(&ctx, action == 2);
                        }
                    }
                }
                ui.menu_button(egui::RichText::new("Sections").size(18.), |ui| {
                    self.sections_menu(ui)
                });
            });
            let mut changed = false;
            if self.supports_axis_locks() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Locks").size(18.));
                    for (axis, label) in [(Axis::X, "X"), (Axis::Y, "Y"), (Axis::Z, "Z")] {
                        let active = self.lock == Some(axis) || self.second_lock == Some(axis);
                        let available = active || self.second_lock.is_none();
                        if ui.add_enabled(available, egui::Button::new(egui::RichText::new(label).size(18.))
                            .min_size(egui::vec2(62., 42.)).selected(active))
                            .on_hover_text("Select one axis or combine two for a plane lock.")
                            .clicked()
                        {
                            self.toggle_axis_lock(axis, &ctx);
                            changed = true;
                        }
                    }
                    if ui.add_enabled(self.lock.is_some(), egui::Button::new(egui::RichText::new("Unlock").size(18.))
                        .min_size(egui::vec2(90., 42.))).clicked()
                    {
                        self.set_axis_lock(None, &ctx);
                        changed = true;
                    }
                });
            }
            if matches!(self.tool, Tool::Move | Tool::Rotate | Tool::Scale | Tool::PushPull | Tool::Offset) {
                if ui.checkbox(&mut preferences.precise_modeling, egui::RichText::new("Exact value").size(18.))
                    .on_hover_text("Choose an anchor, then a target with the trigger. The keyboard opens to enter a length, angle or scale factor. Leaving Exact value off uses held-trigger dragging.")
                    .changed()
                {
                    state.cancel_precision_modeling(self);
                    if state.grab_depth.take().is_some() { self.modeling = None; self.length_input.clear(); }
                    state.precision_target = None;
                }
            }
            match self.tool {
                Tool::Select => {
                    if ui.checkbox(&mut preferences.window_selection, egui::RichText::new("Window selection").size(18.)).changed() {
                        state.cancel_window();
                        state.cancel_selection_click();
                    }
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut preferences.extend_selection,
                            egui::RichText::new("Add/remove selection").size(18.));
                        ui.label(egui::RichText::new(format!("{} selected",self.selected.len())).size(18.));
                    });
                }
                Tool::TapeMeasure | Tool::Protractor => {
                    changed |= ui
                        .checkbox(
                            &mut self.create_guides,
                            egui::RichText::new("Create construction guides").size(18.),
                        )
                        .changed();
                }
                Tool::Polygon => {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{} sides", self.polygon_sides)).size(18.),
                        );
                        for (label, increase) in [("Sides -", false), ("Sides +", true)] {
                            if ui
                                .add_sized(
                                    [100., 42.],
                                    egui::Button::new(egui::RichText::new(label).size(18.)),
                                )
                                .clicked()
                            {
                                self.polygon_sides = if increase {
                                    self.polygon_sides + 1
                                } else {
                                    self.polygon_sides.saturating_sub(1)
                                }
                                .clamp(3, 64);
                                changed = true;
                            }
                        }
                    });
                }
                Tool::Paint => {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Colour").size(18.));
                        changed |= ui
                            .scope(|ui| {
                                ui.spacing_mut().interact_size = egui::vec2(48., 42.);
                                ui.color_edit_button_srgb(&mut self.paint_color).changed()
                            })
                            .inner;
                        for (label, target) in
                            [("Face", PaintTarget::Face), ("Group", PaintTarget::Group)]
                        {
                            if ui
                                .add_sized(
                                    [100., 42.],
                                    egui::Button::new(egui::RichText::new(label).size(18.))
                                        .selected(self.paint_target == target),
                                )
                                .clicked()
                            {
                                self.paint_target = target;
                                changed = true;
                            }
                        }
                    });
                }
                Tool::PushPull => {
                    changed |= ui
                        .checkbox(
                            &mut self.push_pull_new_faces,
                            egui::RichText::new("Keep starting face").size(18.),
                        )
                        .changed();
                }
                _ => {}
            }
            if changed {
                self.persist_inspector_changes(&ctx, Some(storage), false);
            }
            let hint = if self.paste_preview.is_some() {
                "Trigger places the copy. Cancel discards the preview."
            } else if self.tool == Tool::Select && preferences.window_selection {
                WINDOW_HINT
            } else {
                hint(self.tool, preferences.precise_modeling)
            };
            ui.label(egui::RichText::new(hint).size(16.));
        });
    }

    fn xr_cancel_action(
        &mut self,
        clear_selection: bool,
        ctx: &egui::Context,
        storage: &mut dyn eframe::Storage,
    ) {
        self.xr_erasing.clear();
        if self.world_section_info().is_some() {
            self.cancel_section_drag(ctx);
            self.persist_inspector_changes(ctx, Some(storage), false);
            return;
        }
        let placing_copy = self.paste_preview.is_some();
        self.stop_drawing();
        self.measurement = None;
        self.modeling = None;
        self.paste_preview = None;
        self.solid_first = None;
        self.flip_hover = None;
        if clear_selection && self.tool == Tool::Select && !placing_copy {
            self.selected.clear();
        }
    }

    pub(crate) fn xr_world_pointer(
        &mut self,
        pointer: Option<&Pointer>,
        state: &mut State,
        activate: bool,
        ctx: &egui::Context,
        storage: &mut dyn eframe::Storage,
    ) {
        self.xr_world_pointer_with_references(pointer, state, activate, ctx, storage, None);
    }

    pub(crate) fn xr_world_pointer_with_references(
        &mut self, pointer: Option<&Pointer>, state: &mut State, activate: bool,
        ctx: &egui::Context, storage: &mut dyn eframe::Storage,
        references: Option<&dyn crate::xr::world_inference::ReferenceQuery>,
    ) {
        state.pointer_target = None;
        state.handle_hint = None;
        if self.modeling.is_none() { state.move_targets = None; state.model_plane = None; }
        if state.model_phase == Some(ModelPhase::Typing)
            && (state.precision_target.is_none() || state.precision_target != self.xr_precision_target(ctx, true))
        {
            state.cancel_precision_modeling(self);
            state.precision_target = None;
        }
        let Some(pointer) = pointer.filter(|_| self.xr_scene_interaction_enabled(ctx))
            .filter(|pointer| self.xr_pointer_is_current(pointer, ctx)) else {
            state.cancel_window();
            state.cancel_selection_click();
            let retain_precision = state.panel_hover && self.xr_scene_interaction_enabled(ctx)
                && state.precision_target.is_some()
                && state.precision_target == self.xr_precision_target(ctx, state.model_phase == Some(ModelPhase::Typing));
            state.cancel_erasing(self);
            if state.section.take().is_some() && self.world_section_info().is_some() {
                self.cancel_section_drag(ctx);
                self.persist_inspector_changes(ctx, Some(storage), false);
            }
            if state.grab_depth.take().is_some() {
                self.modeling = None;
                self.length_input.clear();
            }
            if state.stroke {
                self.stop_drawing();
                state.stroke = false;
            }
            if !retain_precision && (state.active || state.precision_target.is_some()) {
                self.drawing_reference = None;
                self.flip_hover = None;
                self.snap = None;
                self.hover = None;
                self.drawing_guide = None;
                self.drawing_axis_guides.clear();
                self.drawing_projection = None;
            }
            state.active = false;
            if !retain_precision {
                state.cancel_precision_modeling(self);
                state.reference = None;
                state.precision_target = None;
            }
            return;
        };
        state.precision_target = None;
        state.active = true;
        self.flip_hover = None;
        if state.tool != Some(self.tool) {
            state.cancel_precision_modeling(self);
            if state.section.take().is_some() && self.world_section_info().is_some() {
                self.cancel_section_drag(ctx);
                self.persist_inspector_changes(ctx, Some(storage), false);
            }
            state.tool = Some(self.tool);
            state.cancel_selection_click();
            state.cancel_window();
            state.cancel_erasing(self);
            state.line_normal = None;
            state.stroke = false;
            state.grab_depth = None;
            state.reference = None;
        }
        self.hover = pointer.hit.and_then(|hit| hit.entity);
        // World references survive desktop UI passes without adopting its cursor.
        self.drawing_reference = state.reference;
        self.drawing_guide = None;
        self.drawing_axis_guides.clear();
        self.drawing_projection = None;
        if self.xr_section_pointer(pointer, state, activate, ctx, storage) {
            return;
        }
        if activate && pointer.cancel {
            state.cancel_selection_click();
            if state.cancel_window() { self.snap = None; self.hover = None; ctx.request_repaint(); return; }
            state.cancel_precision_modeling(self);
            self.xr_cancel_action(true, ctx, storage);
            state.line_normal = None;
            state.stroke = false;
            state.grab_depth = None;
            state.reference = None;
            state.cancel_erasing(self);
            ctx.request_repaint();
            return;
        }
        let clicked = activate && pointer.activate;
        if let Some((action, point)) = pointer.handle.filter(|_| !state.capturing()) {
            use super::viewport_overlays::HandleAction;
            self.snap = None;
            self.hover = None;
            let available = match action {
                HandleAction::EditJoint(id) => !self.joinery.open && self.doc.model.connections.get(&id)
                    .is_some_and(|connection| matches!(connection.operation,
                        benchup_core::connections::ConnectionOperation::Joinery(_))),
            };
            if !available { return; }
            state.pointer_target = Some(point);
            state.handle_hint = Some(match action { HandleAction::EditJoint(_) => "Trigger edits this wood joint." });
            if clicked {
                self.xr_cancel_action(false, ctx, storage);
                state.cancel_erasing(self);
                state.reference = None;
                state.precision_target = None;
                state.handle_hint = None;
                match action { HandleAction::EditJoint(id) => { self.edit_joint_from_marker(id, ctx); } }
                self.persist_inspector_changes(ctx, Some(storage), false);
            }
            return;
        }
        if self.paste_preview.is_none() && self.modeling.is_none()
            && pointer.hit.is_some_and(|hit| hit.section)
            && (matches!(self.tool, Tool::PushPull | Tool::Offset)
                || self.tool == Tool::Paint && self.paint_target == PaintTarget::Face)
        {
            self.snap = None;
            self.hover = None;
            if clicked { self.say(ctx, "Section fills cannot be edited as faces. Choose an original model face."); }
            return;
        }
        if self.paste_preview.is_some() {
            state.erasing = false;
            self.xr_erasing.clear();
            state.reference = None;
            self.drawing_reference = None;
            let normal = self.plane_lock().map_or(DVec3::Z, Axis::vector);
            self.snap = self.xr_ray_snap(pointer, None, normal);
            self.apply_world_grid_snap(normal,&pointer.model,pointer.hit.and_then(|hit|hit.edge),pointer.hit.and_then(|hit|hit.face));
            if let Some(snap) = self.snap {
                self.paste_preview.as_mut().unwrap().point = snap.point;
                if clicked {
                    let preview = self.paste_preview.take().unwrap();
                    self.paste_at(
                        ctx,
                        &preview.clipboard,
                        preview.point - preview.clipboard.anchor,
                    );
                    self.snap = None;
                }
            }
            ctx.request_repaint();
            return;
        }
        if self.tool.is_solid() {
            self.snap = None;
            let flip = (self.tool == Tool::Flip).then_some(pointer.hit).flatten()
                .and_then(|hit|hit.flip.map(|axis|(axis,hit.point)));
            if let Some((axis,point)) = flip {
                self.flip_hover = Some(axis);
                self.snap = Some(Snap { point, kind: Inference::Plane });
                state.pointer_target = Some(point);
            }
            if clicked {
                if let Some((axis, point)) = flip {
                    self.apply_flip(axis, ctx);
                    self.snap = Some(Snap {
                        point,
                        kind: Inference::Plane,
                    });
                } else {
                    let target = pointer
                        .hit
                        .and_then(|hit| hit.entity)
                        .filter(|id| self.doc.model.contains_entity(id));
                    self.solid_click_at_target(target, ctx);
                }
            }
            return;
        }
        if matches!(
            self.tool,
            Tool::Move | Tool::PushPull | Tool::Scale | Tool::Offset | Tool::Rotate
        ) {
            self.xr_model_grab(pointer, state, clicked, activate, ctx);
            return;
        }
        if self.tool == Tool::Paint {
            self.snap = None;
            if self.paint_target == PaintTarget::Face {
                self.hover = pointer
                    .hit
                    .and_then(|hit| hit.face)
                    .filter(|id| self.doc.model.faces.contains_key(id));
            }
            if clicked {
                if self.paint_target == PaintTarget::Group && pointer.hit.is_some_and(|hit| hit.section) {
                    if let Some(target) = pointer.hit.and_then(|hit|hit.entity).filter(|id|self.doc.model.objects.contains_key(id)) {
                        self.paint_model_target(target,ctx);
                    } else { self.say(ctx, "Choose a group to paint, or an original model face."); }
                } else {
                    self.paint_model_face(pointer.hit.and_then(|hit| hit.face), ctx);
                }
            }
            return;
        }
        if self.tool == Tool::Erase {
            self.snap = None;
            self.hover = self.hover.filter(|id|self.doc.model.contains_entity(id) && !self.doc.model.is_locked(*id));
            let revision = (self.document_revision(ctx), self.doc.model.edit_context);
            if clicked {
                self.xr_erasing.clear();
                state.erasing = true;
                state.erase_revision = Some(revision);
            }
            if state.erasing && state.erase_revision != Some(revision) {
                state.cancel_erasing(self);
                ctx.request_repaint();
                return;
            }
            if state.erasing {
                if activate && let Some(sweep) = state.erase_sweep.take() && sweep.revision == revision {
                    self.xr_erasing.extend(sweep.ids.into_iter().filter(|id|
                        self.doc.model.contains_entity(id) && !self.doc.model.is_locked(*id)));
                }
                if let Some(id) = self.hover { self.xr_erasing.insert(id); }
                if activate && !pointer.primary_down {
                    state.erasing = false;
                    state.erase_revision = None;
                    let mut ids=std::mem::take(&mut self.xr_erasing);
                    ids.retain(|id|self.doc.model.contains_entity(id) && !self.doc.model.is_locked(*id));
                    if !ids.is_empty() { self.erase_entities(&ids,tr!("app.erase_geometry"),ctx); }
                }
            }
            return;
        }
        if self.tool == Tool::Select {
            if self.xr_panel.window_selection { self.xr_window_select(pointer, state, clicked, activate, ctx); return; }
            state.cancel_window();
            if clicked {
                self.xr_select_click(pointer, state, ctx);
            }
            return;
        }
        if self.tool == Tool::Text && self.measurement.is_none()
            && let Some(id) = pointer.hit.and_then(|hit|hit.annotation)
                .filter(|id|self.doc.model.texts.contains_key(id))
        {
            self.snap = None;
            if clicked {
                self.selected = BTreeSet::from([id]);
                self.begin_text_edit(id, ctx);
            }
            return;
        }
        if !matches!(
            self.tool,
            Tool::Line
                | Tool::Rectangle
                | Tool::Circle
                | Tool::Polygon
                | Tool::Freehand
                | Tool::TapeMeasure
                | Tool::Protractor
                | Tool::Dimensions
                | Tool::Text
                | Tool::GrainDirection
        ) {
            self.snap = None;
            if clicked {
                self.say(ctx, "Use both grips to navigate the VR scene, or choose an editing tool from the XR picker.");
            }
            return;
        }
        if clicked && self.measurement.as_ref().is_some_and(|m| m.complete) {
            self.measurement = None;
            self.length_input.clear();
        }
        let start = if self.tool == Tool::Line {
            self.line_start
        } else if self.tool == Tool::GrainDirection {
            self.grain_anchor.as_ref().map(|anchor| anchor.point)
        } else if self.tool.is_drawing() {
            self.drawing.as_ref().map(|drawing| drawing.origin)
        } else {
            self.measurement
                .as_ref()
                .filter(|m| !m.complete)
                .and_then(|m| m.points.first().copied())
        };
        if self.tool == Tool::Line && start.is_none() {
            state.line_normal = None;
        }
        if self.lock.is_none()
            && let Some(drawing) = &mut self.drawing
        {
            drawing.set_normal(drawing.surface_normal);
        }
        let normal = (self.tool == Tool::Text)
            .then(|| {
                self.measurement
                    .as_ref()
                    .and_then(|m| m.normal.try_normalize())
            })
            .flatten()
            .or_else(|| self.grain_plane())
            .or_else(|| self.measurement_plane().map(|p| p.1))
            .or_else(|| self.drawing.as_ref().map(|drawing| drawing.normal))
            .or(state.line_normal)
            .or_else(|| pointer.hit.filter(|hit| hit.section).map(|hit| hit.normal))
            .unwrap_or(DVec3::Z);
        self.snap = self.xr_ray_snap(pointer, start, normal);
        self.xr_reference_inference(pointer,state,start,normal,references);
        self.xr_shape_plane(pointer.ray);
        if matches!(self.tool, Tool::Text | Tool::GrainDirection) && start.is_none() {
            // A new leader must attach to geometry, even with snapping disabled.
            self.snap = pointer.hit.map(|hit| {
                self.snap
                    .filter(|snap| {
                        snap.kind != Inference::Plane && hit.contains(&pointer.model, snap.point)
                    })
                    .unwrap_or(Snap {
                        point: hit.point,
                        kind: hit.inference(),
                    })
            });
            if self.tool == Tool::GrainDirection {
                self.hover = pointer.hit.and_then(|hit| hit.entity).filter(|id| {
                    self.doc.model.objects.contains_key(id)
                        && !self.doc.model.is_locked(*id)
                        && !self.doc.model.is_hidden(*id)
                });
                if self.hover.is_none() {
                    self.snap = None;
                }
            }
        }
        if self.tool == Tool::GrainDirection
            && let Some(anchor) = &self.grain_anchor
        {
            self.hover = Some(anchor.part);
        }
        let plane = self
            .drawing
            .as_ref()
            .map(|drawing| drawing.normal)
            .or_else(|| {
                (self.tool != Tool::Dimensions)
                    .then(|| self.plane_lock().map(Axis::vector))
                    .flatten()
            })
            .unwrap_or(normal);
        self.apply_world_grid_snap(plane,&pointer.model,pointer.hit.and_then(|hit|hit.edge),pointer.hit.and_then(|hit|hit.face));
        if self.tool == Tool::Text
            && let (Some(m), Some(snap)) = (&mut self.measurement, self.snap)
        {
            m.offset = snap.point - m.points[0];
        }
        let valid_offset = self.xr_measurement_offset(pointer.ray);
        if self.tool == Tool::Freehand {
            self.xr_freehand(pointer, state, clicked, activate, ctx);
            return;
        }
        state.precision_target = self.xr_precision_target(ctx, state.model_phase == Some(ModelPhase::Typing));
        if !clicked || !valid_offset {
            return;
        }
        if self.tool == Tool::Line {
            if self.line_start.is_some() {
                self.commit_line_snap(ctx, !self.length_input.trim().is_empty());
            } else if let Some(snap) = self.snap {
                self.line_start = Some(snap.point);
                state.line_normal = pointer.hit.map(|hit| hit.normal).or(Some(DVec3::Z));
            }
        } else if self.tool.is_drawing() {
            if self.drawing.is_some() {
                self.commit_drawing_at_snap(ctx, false);
            } else {
                self.xr_begin_shape(pointer);
            }
        } else if self.tool == Tool::GrainDirection {
            let hit = if self.grain_anchor.is_none() {
                let anchor = pointer.hit.zip(self.snap).and_then(|(hit, snap)| {
                    hit.entity.and_then(|entity| self.grain_anchor_at_hit(entity, snap.point, (hit.face.is_some() || hit.section).then_some(hit.normal)))
                });
                if anchor.is_none() {
                    self.say(
                        ctx,
                        "Choose an unlocked group or component for the grain direction.",
                    );
                    return;
                }
                anchor
            } else {
                None
            };
            self.grain_click_at_anchor(hit, ctx);
        } else if self.tool == Tool::Text {
            if self.snap.is_none() {
                self.say(
                    ctx,
                    "Point at geometry to attach text, or at the label plane to place it.",
                );
                return;
            }
            self.text_click_at_snap(pointer.hit.and_then(|hit| hit.entity), ctx);
            if let Some(m) = &mut self.measurement {
                m.normal = pointer.ray.direction;
            }
        } else if self.measurement_precision_available() && !self.length_input.trim().is_empty() {
            self.commit_measurement_value(ctx);
        } else {
            if self.tool == Tool::Dimensions
                && let Some(snap) = self.snap
            {
                if let Some(m) = self.measurement.as_mut().filter(|m| m.points.len() == 1) {
                    if let Some(direction) = (snap.point - m.points[0]).try_normalize() {
                        // The annotation plane contains the measured span and
                        // faces the controller at the second endpoint.
                        m.normal = (pointer.ray.direction
                            - direction * pointer.ray.direction.dot(direction))
                        .try_normalize()
                        .unwrap_or_else(|| {
                            direction
                                .cross(DVec3::X)
                                .try_normalize()
                                .unwrap_or(DVec3::Y)
                        });
                    }
                }
            }
            self.measurement_click_snap(
                pointer.hit.map_or(normal, |hit| hit.normal),
                pointer.ray.origin,
                ctx,
            );
        }
        state.precision_target = self.xr_precision_target(ctx, state.model_phase == Some(ModelPhase::Typing));
        ctx.request_repaint();
    }

    fn xr_section_pointer(
        &mut self,
        pointer: &Pointer,
        state: &mut State,
        activate: bool,
        ctx: &egui::Context,
        storage: &mut dyn eframe::Storage,
    ) -> bool {
        if let Some(grab) = &state.section {
            let Some((axis, offset, _)) = self.world_section_info() else {
                state.section = None;
                return true;
            };
            self.snap = None;
            self.hover = None;
            self.flip_hover = None;
            if !self.doc.display.section_planes
                || !self.doc.display.section_axes[axis]
                || activate && pointer.cancel
            {
                self.cancel_section_drag(ctx);
                self.persist_inspector_changes(ctx, Some(storage), false);
                state.section = None;
                return true;
            }
            let tip = pointer.ray.origin + pointer.ray.direction * grab.depth;
            let value = (offset + (tip - grab.start_tip)[axis]).clamp(-1e8, 1e8);
            if activate && value.is_finite() {
                self.move_section_plane(ctx, axis, value);
                self.persist_inspector_changes(ctx, Some(storage), false);
            }
            state.pointer_target = self.section_world_target();
            if activate && !pointer.primary_down {
                self.finish_world_section();
                state.section = None;
            }
            return true;
        }
        if self.modeling.is_some()
            || self.gesture.is_some()
            || state.stroke
            || state.grab_depth.is_some()
            || pointer.cancel
        {
            return false;
        }
        let handle = self
            .section_handle_anchors()
            .into_iter()
            .filter_map(|(axis, point)| {
                let delta = point - pointer.ray.origin;
                let depth = delta.dot(pointer.ray.direction);
                if depth <= 0. {
                    return None;
                }
                let error = (delta - pointer.ray.direction * depth).length() / depth;
                (error <= 0.02).then_some((axis, point, depth, error))
            })
            .min_by(|a, b| a.3.total_cmp(&b.3).then(a.2.total_cmp(&b.2)));
        let Some((axis, point, depth, _)) = handle else {
            return false;
        };
        self.snap = None;
        self.hover = None;
        self.flip_hover = None;
        state.pointer_target = Some(point);
        if activate && pointer.activate {
            self.begin_world_section(axis, point);
            state.section = Some(SectionGrab {
                depth,
                start_tip: pointer.ray.origin + pointer.ray.direction * depth,
            });
        }
        true
    }

    fn xr_model_grab(
        &mut self,
        pointer: &Pointer,
        state: &mut State,
        clicked: bool,
        activate: bool,
        ctx: &egui::Context,
    ) {
        self.snap = None;
        if state.model_phase == Some(ModelPhase::Typing) {
            state.precision_target = self.xr_precision_target(ctx, true);
            return;
        }
        if matches!(self.tool, Tool::PushPull | Tool::Offset) {
            self.hover = pointer
                .hit
                .and_then(|hit| hit.face)
                .filter(|id| self.doc.model.faces.contains_key(id));
        }
        if !activate {
            return;
        }
        let confirming = clicked && state.model_phase == Some(ModelPhase::Aiming) && self.modeling.is_some();
        if clicked && !confirming {
            let Some(hit) = pointer.hit else {
                return;
            };
            let (face, direction) = if matches!(self.tool, Tool::PushPull | Tool::Offset) {
                let Some(face) = hit.face.and_then(|face| self.doc.model.faces.get(&face)) else {
                    self.say(ctx, "Choose an original document face to edit.");
                    return;
                };
                if self.doc.model.is_locked(face.id) {
                    self.say(ctx, "Unlock the face before editing it.");
                    return;
                }
                let Some(plane) = benchup_core::faces::plane(&self.doc.model, &face.vertices)
                else {
                    return;
                };
                if (hit.point - plane.origin).dot(plane.normal).abs() > 1e-6 {
                    self.say(ctx, "Choose an original document face to edit.");
                    return;
                }
                state.model_plane = Some(plane.normal);
                let direction = if self.tool == Tool::Offset {
                    self.offset_direction(face, hit.point, plane)
                } else {
                    plane.normal
                        * if plane.normal.dot(pointer.ray.origin - hit.point) >= 0. {
                            1.
                        } else {
                            -1.
                        }
                };
                if self.selected_modeling_face() != hit.face {
                    self.selected = BTreeSet::from([face.id]);
                }
                (hit.face, direction)
            } else {
                let Some(entity) = hit.entity.filter(|id| self.doc.model.contains_entity(id)) else {
                    self.say(ctx, "Choose an original document entity to transform.");
                    return;
                };
                self.prepare_picking();
                if !self
                    .entity_cache
                    .selected(&self.doc.model, entity, &self.selected)
                {
                    self.selected = BTreeSet::from([entity]);
                }
                if self.doc.model.selection_locked(&self.selected) {
                    self.say(ctx, "Unlock the selection before transforming it.");
                    return;
                }
                (None, DVec3::ZERO)
            };
            let anchor = if self.tool == Tool::Rotate {
                self.doc
                    .model
                    .selection_center(&self.selected)
                    .unwrap_or(hit.point)
            } else {
                hit.point
            };
            self.begin_modeling_at_anchor(anchor, direction, face, Pos2::ZERO, pointer.ray);
            let edit = self.modeling.as_mut().unwrap();
            edit.world_input = true;
            // Linear face operations may target original rings/surfaces, but
            // never their changing evaluated preview.
            if edit.move_snap.is_none() {
                edit.move_snap = Some(benchup_core::inference::MoveSnapCache::new(&edit.base));
            }
            if self.tool == Tool::Rotate
                && let Some(edit) = &mut self.modeling
            {
                let rotation = edit.rotation.as_mut().unwrap();
                let reference = hit.point - edit.anchor;
                let reference = reference - rotation.normal * reference.dot(rotation.normal);
                if reference.length_squared() < 1e-12 {
                    self.modeling = None;
                    self.say(
                        ctx,
                        "Grab a point away from the selection centre and rotation axis.",
                    );
                    return;
                }
                rotation.reference = Some(reference);
                edit.direction = reference;
            }
            if self.tool == Tool::Scale
                && let Some(edit) = &self.modeling
            {
                let reference = edit.anchor - edit.center;
                let reference = edit
                    .scale_constraint
                    .map_or(reference, |constraint| constraint.project(reference));
                if reference.length_squared() < 1e-12 {
                    self.modeling = None;
                    self.say(
                        ctx,
                        "Grab a point away from the selection centre and any locked scale axis.",
                    );
                    return;
                }
            }
            state.grab_depth = Some(
                (hit.point - pointer.ray.origin)
                    .dot(pointer.ray.direction)
                    .max(1e-6),
            );
            state.model_phase = self.xr_panel.precise_modeling.then_some(ModelPhase::Aiming);
        }
        let (Some(depth), Some(edit)) = (state.grab_depth, self.modeling.as_ref()) else {
            state.grab_depth = None;
            return;
        };
        // Keep the grabbed anchor at its original distance along the aim ray.
        // This supports controller translation and swinging the ray naturally,
        // including pulls where the ray is parallel to the extrusion axis.
        let mut delta = pointer.ray.origin + pointer.ray.direction * depth - edit.anchor;
        let constraint = self.axis_constraint();
        let rotation_lock = self.shape_lock();
        let direction = if self.tool == Tool::PushPull {
            self.pull_direction(edit.unlocked_direction)
        } else {
            edit.unlocked_direction
        };
        let spacing = grid::configured_spacing(&self.doc.camera, &self.doc.display);
        let model_target = if matches!(self.tool, Tool::Move | Tool::Scale | Tool::Rotate | Tool::PushPull | Tool::Offset) {
            let geometry = edit.move_snap.as_ref().map(|cache| cache.shared_geometry());
            geometry.and_then(|geometry| {
                let targets = state.move_targets.get_or_insert_with(Default::default);
                targets.prepare(Arc::clone(&geometry), &self.doc.display);
                let point = edit.anchor + if self.tool == Tool::Rotate { delta } else { constraint.map_or(delta, |c| c.project(delta)) };
                let options = crate::xr::world_move::Options { geometry: self.doc.snap_to_geometry,
                        targets: benchup_core::inference::SnapTargets { edges: self.snap_to_edge,
                            faces: self.snap_to_face, face_centers: self.snap_to_face_center },
                        axes: self.doc.snap_to_axes, x_ray: self.tool == Tool::Move || self.doc.display.x_ray };
                let target = if self.tool == Tool::Rotate {
                    let normal = edit.rotation.as_ref().unwrap().normal_for_lock(rotation_lock);
                    targets.rotation(pointer.ray, edit.anchor, normal, point, options)
                } else if self.tool == Tool::Offset {
                    targets.offset(pointer.ray, edit.anchor, state.model_plane.unwrap_or(DVec3::Z), point, constraint, options)
                } else if self.tool == Tool::PushPull {
                    targets.infer_with_options(pointer.ray, edit.anchor, edit.anchor + delta, None, options)
                        .filter(|target| {
                            let starting_surface = target.face == edit.face && edit.face.is_some()
                                || target.edge.is_some_and(|edge| edit.snap_excluded.contains(&edge));
                            (target.snap.point - edit.anchor).dot(direction).abs() > 1e-7
                                || target.snap.kind != Inference::OnFace && !starting_surface
                        })
                } else { targets.infer_with_options(pointer.ray, edit.anchor, point, constraint, options) };
                target.map(|target| (target, geometry))
            })
        } else { None };
        let edit = self.modeling.as_mut().unwrap();
        edit.snap = None;
        edit.guides.clear();
        if self.tool == Tool::Move {
            if let Some((mut target, geometry)) = model_target {
                if self.snap_to_grid {
                    let plane = constraint.and_then(|c| match c { AxisConstraint::Plane(axis) => Some(axis.vector()), _ => None });
                    let axis = constraint.and_then(|c| match c { AxisConstraint::Axis(axis) => Some(axis), _ => None });
                    target.snap = grid::snap_with_geometry_hint(target.snap, spacing, plane, axis, &geometry,
                        None, Some((target.edge, target.face)));
                }
                delta = target.snap.point - edit.anchor;
                edit.snap = Some(target.snap);
                edit.guides = target.guides;
            } else {
                if let Some(constraint) = constraint {
                    delta = constraint.project(delta);
                }
                if self.snap_to_grid {
                    delta = (delta / spacing).round() * spacing;
                }
            }
            edit.direction = delta;
            edit.value = delta.length();
        } else if self.tool == Tool::Rotate {
            let rotation = edit.rotation.as_mut().unwrap();
            rotation.set_lock(rotation_lock);
            let normal = rotation.normal;
            let (mut snap, edge, face, guides) = if let Some((target, _)) = model_target {
                (target.snap, target.edge, target.face, target.guides)
            } else {
                let denominator = pointer.ray.direction.dot(normal);
                if denominator.abs() > 1e-8 {
                    let depth = (edit.anchor - pointer.ray.origin).dot(normal) / denominator;
                    if depth > 0. { delta = pointer.ray.origin + pointer.ray.direction * depth - edit.anchor; }
                }
                let point = edit.anchor + delta - normal * delta.dot(normal);
                (Snap { point, kind: Inference::Plane }, None, None, Vec::new())
            };
            if self.snap_to_grid {
                let plane_axis = [Axis::X, Axis::Y, Axis::Z].into_iter()
                    .find(|axis| normal.cross(axis.vector()).length() < 1e-7).map(AxisConstraint::Plane);
                let axis = grid::movement_axis(snap.point, Some(edit.anchor), plane_axis, self.doc.snap_to_axes);
                snap = grid::snap_with_geometry_hint(snap, spacing, Some(normal), axis, &edit.base,
                    Some(&edit.snap_excluded), Some((edge, face)));
            }
            delta = snap.point - edit.anchor;
            edit.snap = Some(snap);
            edit.guides = guides;
            let reference = rotation.reference.unwrap();
            let reference = reference - normal * reference.dot(normal);
            let target = delta - normal * delta.dot(normal);
            if reference.length_squared() < 1e-12 || target.length_squared() < 1e-12 {
                state.grab_depth = None;
                self.modeling = None;
                self.say(ctx, "The rotation reference lies on its axis. Grab another point or change the axis lock.");
                return;
            }
            edit.direction = target;
            edit.value = normal
                .dot(reference.cross(target))
                .atan2(reference.dot(target))
                .to_degrees();
        } else if self.tool == Tool::Scale {
            edit.scale_constraint = constraint;
            let snapped = if let Some((mut target, geometry)) = model_target {
                if self.snap_to_grid {
                    let plane = constraint.and_then(|c| match c { AxisConstraint::Plane(axis) => Some(axis.vector()), _ => None });
                    let axis = grid::movement_axis(target.snap.point, Some(edit.anchor), constraint, self.doc.snap_to_axes);
                    target.snap = grid::snap_with_geometry_hint(target.snap, spacing, plane, axis, &geometry,
                        None, Some((target.edge, target.face)));
                }
                delta = target.snap.point - edit.anchor;
                edit.snap = Some(target.snap);
                edit.guides = target.guides;
                true
            } else { false };
            edit.direction = delta;
            let reference = edit.anchor - edit.center;
            let target = edit.anchor + delta - edit.center;
            let (reference, target) = constraint.map_or((reference, target), |constraint| {
                (constraint.project(reference), constraint.project(target))
            });
            if reference.length_squared() < 1e-12 {
                state.grab_depth = None;
                self.modeling = None;
                self.say(ctx, "The scale reference lies on the locked axis. Choose another point or unlock the axis.");
                return;
            }
            let mut radius = target.length();
            if self.snap_to_grid && !snapped {
                radius = (radius / spacing).round() * spacing;
            }
            edit.value = (radius / reference.length()).clamp(0.001, 1000.);
        } else {
            edit.direction = direction;
            let mut target = model_target.map(|(target, _)| target);
            let plane = if self.tool == Tool::Offset {
                match constraint {
                    Some(AxisConstraint::Plane(axis)) => Some(axis.vector()),
                    Some(AxisConstraint::Axis(_)) => None,
                    None => Some(state.model_plane.unwrap_or(DVec3::Z)),
                }
            } else { None };
            if target.is_none() && self.tool == Tool::Offset {
                let constrained = if let Some(AxisConstraint::Axis(axis)) = constraint {
                    pointer.ray.axis(edit.anchor, axis.vector())
                } else {
                    let normal = plane.unwrap();
                    let denominator = pointer.ray.direction.dot(normal);
                    (denominator.abs() > 1e-8).then(|| {
                        pointer.ray.origin + pointer.ray.direction * (edit.anchor - pointer.ray.origin).dot(normal) / denominator
                    })
                }.filter(|point| (*point - pointer.ray.origin).dot(pointer.ray.direction) > 0.);
                let fallback = edit.anchor + constraint.map_or(delta, |c| c.project(delta));
                let fallback = if constraint.is_none() {
                    let normal = plane.unwrap();
                    fallback - normal * (fallback - edit.anchor).dot(normal)
                } else { fallback };
                let point = constrained.unwrap_or(fallback);
                target = Some(crate::xr::world_move::Target { snap: Snap { point, kind: Inference::Plane },
                    guides: Vec::new(), edge: None, face: None });
            }
            let snapped = if let Some(mut target) = target {
                if self.snap_to_grid {
                    let axis = if self.tool == Tool::Offset {
                        grid::movement_axis(target.snap.point, Some(edit.anchor), constraint, self.doc.snap_to_axes)
                    } else { None };
                    target.snap = grid::snap_with_geometry_hint(target.snap, spacing, plane, axis, &edit.base,
                        Some(&edit.snap_excluded), Some((target.edge, target.face)));
                }
                delta = target.snap.point - edit.anchor;
                edit.snap = Some(target.snap);
                edit.guides = target.guides;
                true
            } else { false };
            if self.tool == Tool::Offset
                && let Some(constraint) = constraint
            {
                delta = constraint.project(delta);
            }
            edit.value = delta.dot(direction);
            if self.snap_to_grid && !snapped {
                edit.value = (edit.value / spacing).round() * spacing;
            }
        }
        self.refresh_modeling_preview();
        if confirming {
            if self.tool == Tool::Move && self.modeling.as_ref().is_none_or(|edit| edit.direction.length_squared() < 1e-12) {
                self.say(ctx, "Point away from the anchor to choose a move direction.");
                return;
            }
            state.model_phase = Some(ModelPhase::Typing);
            state.grab_depth = None;
            state.precision_target = self.xr_precision_target(ctx, true);
            state.request_precision_input = true;
            ctx.request_repaint();
            return;
        }
        if !pointer.primary_down && state.model_phase != Some(ModelPhase::Aiming) {
            state.grab_depth = None;
            if self.modeling.as_ref().is_some_and(|edit| {
                (edit.value - if self.tool == Tool::Scale { 1. } else { 0. }).abs() >= 1e-7
            }) || !self.length_input.trim().is_empty()
            {
                self.commit_modeling_at_value(ctx);
            }
            // Errors leave the authoritative model untouched; do not retain a
            // released grab or commit a frozen preview after tracking loss.
            self.modeling = None;
            self.length_input.clear();
        }
        ctx.request_repaint();
    }

    fn xr_begin_shape(&mut self, pointer: &Pointer) {
        let surface = pointer.hit.filter(|hit| {
            (hit.face.is_some() || hit.section)
                && self
                    .snap
                    .is_some_and(|snap| (snap.point - hit.point).dot(hit.normal).abs() < 1e-6)
        });
        self.begin_drawing_at_snap(surface.map(|hit| hit.normal));
    }

    fn xr_freehand(
        &mut self,
        pointer: &Pointer,
        state: &mut State,
        clicked: bool,
        activate: bool,
        ctx: &egui::Context,
    ) {
        if !activate {
            return;
        }
        if clicked {
            self.xr_begin_shape(pointer);
            state.stroke = self.drawing.is_some();
        }
        if !state.stroke {
            return;
        }
        if self.drawing.is_none() {
            state.stroke = false;
            return;
        }
        if let (Some(drawing), Some(snap)) = (&mut self.drawing, self.snap) {
            let last = *drawing.points.last().unwrap();
            // Angular spacing remains consistent when navigating/scaling the
            // model. Only the active stroke receives incremental samples.
            let spacing = (last.distance(pointer.ray.origin) * 0.003).max(1e-6);
            if last.distance(snap.point) >= spacing {
                drawing.points.push(snap.point);
            }
        }
        let full = self
            .drawing
            .as_ref()
            .is_some_and(|drawing| drawing.points.len() >= 2048);
        if !pointer.primary_down || full {
            state.stroke = false;
            // A valid tracked release can finish at the last sample when its
            // ray is parallel to the plane. Missing tracking/focus takes the
            // cancellation path above instead of committing.
            if self.snap.is_none() {
                self.snap = self
                    .drawing
                    .as_ref()
                    .and_then(|drawing| drawing.points.last())
                    .map(|&point| Snap {
                        point,
                        kind: Inference::Plane,
                    });
            }
            self.commit_drawing_at_snap(ctx, false);
            if full {
                self.say(
                    ctx,
                    "Stroke limit reached. Release the trigger to start another stroke.",
                );
            }
        }
        ctx.request_repaint();
    }

    fn xr_reference_inference(&mut self, pointer: &Pointer, state: &mut State, start: Option<DVec3>, normal: DVec3,
        references: Option<&dyn crate::xr::world_inference::ReferenceQuery>) {
        let eligible = (self.tool.is_drawing() || self.tool.is_measuring() || self.tool == Tool::GrainDirection)
            && self.doc.snap_to_axes && self.doc.snap_to_geometry && self.snap_to_edge;
        let contains = |reference:drawing_inference::DrawingReference| {
            let Some(edge) = self.doc.model.edges.get(&reference.edge)
                .filter(|edge|!self.doc.model.is_hidden(edge.id)) else { return false; };
            let (a,b) = self.doc.model.points(edge);
            let segment=b-a;
            if segment.length_squared()<1e-24 { return false; }
            let t=(reference.point-a).dot(segment)/segment.length_squared();
            (-1e-7..=1.0000001).contains(&t) && reference.point.distance(a+segment*t)<1e-6
        };
        state.reference = state.reference.filter(|reference|eligible && contains(*reference)
            && pointer.visible_reference.is_some_and(|point|point.distance(reference.point)<1e-7));
        let mut acquired = false;
        if eligible && let Some(hit) = pointer.hit.filter(|hit|hit.edge.is_some()) {
            let source = self.snap.filter(|snap| matches!(snap.kind,Inference::Endpoint
                | Inference::Midpoint | Inference::FaceCenter | Inference::Origin | Inference::OnEdge)
                && hit.contains(&pointer.model,snap.point))
                .unwrap_or(Snap {point:hit.point,kind:Inference::OnEdge});
            let source = if self.snap_to_grid {
                grid::snap_with_geometry_hint(source,
                    grid::configured_spacing(&self.doc.camera,&self.doc.display),
                    None,None,&self.doc.model,None,Some((hit.edge,None)))
            } else { source };
            let reference = drawing_inference::DrawingReference {edge:hit.edge.unwrap(),point:source.point};
            if contains(reference) { state.reference=Some(reference); acquired=true; }
        }
        self.drawing_reference=state.reference;
        let Some(base) = self.snap else { return; };
        if matches!(base.kind,Inference::Endpoint | Inference::Midpoint | Inference::FaceCenter
            | Inference::Origin | Inference::OnEdge | Inference::OnGuide | Inference::GuidePoint) { return; }
        let constraint = if self.tool.is_drawing() && self.tool != Tool::Line {
            self.shape_lock().map(AxisConstraint::Plane)
        } else if self.tool == Tool::Dimensions { None } else { self.axis_constraint() };
        let constraint = start.zip(constraint.or_else(||match base.kind {
            Inference::Axis(axis) => Some(AxisConstraint::Axis(axis)), _ => None,
        }));
        let plane = pointer.hit.filter(|hit|base.kind == Inference::OnFace && hit.face.is_some())
            .map_or((start.unwrap_or(DVec3::ZERO),normal),|hit|(hit.point,hit.normal));
        let mut guides = Vec::new();
        if self.doc.snap_to_axes {
            for origin in start.into_iter().chain([DVec3::ZERO]) {
                guides.extend([Axis::X, Axis::Y, Axis::Z].map(|axis| AxisGuide { origin, axis }));
            }
            if eligible && let Some(references) = references {
                guides.extend(references.alignment(pointer.ray, plane, constraint).into_iter().flatten());
            }
        }
        let global = crate::xr::world_inference::align_guides(pointer.ray, plane, constraint, &guides);
        let Some(reference) = state.reference else {
            if let Some((snap, guides)) = global { self.snap = Some(snap); self.drawing_axis_guides = guides; }
            return;
        };
        if self.tool != Tool::Dimensions
            && let Some((start,axis)) = start.zip(self.single_axis_lock())
            && let Some(snap) = crate::xr::world_inference::reference_projection(
                pointer.ray,reference.point,start,axis,Some(plane),acquired)
        {
            self.snap=Some(snap);
            self.drawing_projection=Some((reference.point,snap.point));
            return;
        }
        if let Some((snap,guide)) = crate::xr::world_inference::reference_axis(pointer.ray,reference.point,constraint,Some(plane)) {
            self.snap=Some(snap);
            self.drawing_guide=Some(guide);
        } else if let Some((snap, guides)) = global {
            self.snap = Some(snap); self.drawing_axis_guides = guides;
        }
    }

    fn xr_ray_snap(&self, pointer: &Pointer, start: Option<DVec3>, normal: DVec3) -> Option<Snap> {
        let ray = pointer.ray;
        let constraint = if self.tool.is_drawing() && self.tool != Tool::Line {
            self.shape_lock().map(AxisConstraint::Plane)
        } else {
            (self.tool != Tool::Dimensions)
                .then(|| self.axis_constraint())
                .flatten()
        };
        let allowed = |point: DVec3| {
            start
                .zip(constraint)
                .is_none_or(|(s, c)| c.contains(point - s))
        };
        // Angular tolerance remains usable as the user scales or moves the scene.
        // Targets come from retained spatial queries, never a full scene scan.
        let near = |point: DVec3| -> Option<f64> {
            let delta = point - ray.origin;
            let depth = delta.dot(ray.direction);
            if depth <= 0. || !allowed(point) {
                return None;
            }
            let tolerance = depth * 0.01;
            if pointer.hit.is_some_and(|hit| {
                hit.edge.is_none() && hit.guide.is_none()
                    && depth > (hit.point - ray.origin).dot(ray.direction) + tolerance
            }) {
                return None;
            }
            let distance = (delta - ray.direction * depth).length();
            (distance <= tolerance).then_some(distance / depth)
        };
        let mut points = vec![(DVec3::ZERO, Inference::Origin)];
        if let Some(start) = start {
            points.push((start, Inference::Endpoint));
        }
        if self.doc.snap_to_geometry
            && let Some(hit) = pointer.hit
        {
            points.extend(hit.guide_points.into_iter().flatten()
                .map(|point| (point, Inference::GuidePoint)));
            if self.snap_to_edge {
                points.extend(hit.edge_targets.into_iter().enumerate().filter_map(
                    |(index, point)| {
                        point.map(|point| {
                            (
                                point,
                                if index == 2 {
                                    Inference::Midpoint
                                } else {
                                    Inference::Endpoint
                                },
                            )
                        })
                    },
                ));
            }
            if self.snap_to_face_center
                && let Some(center) = hit.center
            {
                points.push((center, Inference::FaceCenter));
            }
        }
        if let Some((point, kind)) = points
            .into_iter()
            .filter_map(|(p, k)| near(p).map(|distance| (p, k, distance)))
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(p, k, _)| (p, k))
        {
            return Some(Snap { point, kind });
        }
        if self.doc.snap_to_geometry
            && self.snap_to_edge
            && let Some(hit) = pointer
                .hit
                .filter(|hit| hit.edge.is_some() && allowed(hit.point))
        {
            return Some(Snap {
                point: hit.point,
                kind: Inference::OnEdge,
            });
        }
        if self.doc.snap_to_geometry
            && let Some(hit) = pointer.hit.filter(|hit| hit.guide.is_some())
        {
            let (a, b) = hit.guide.unwrap();
            let direction = (b - a).try_normalize();
            let point = match (direction, start.zip(constraint)) {
                (None, _) => Some(a),
                (Some(direction), Some((start, AxisConstraint::Axis(axis)))) => {
                    (Ray { origin: start, direction: axis.vector() }).axis(a, direction)
                        .filter(|point| hit.contains(&pointer.model, *point))
                }
                (Some(direction), Some((start, AxisConstraint::Plane(axis))))
                    if direction.dot(axis.vector()).abs() > 1e-9 => {
                        Some(a + direction * ((start - a).dot(axis.vector())
                            / direction.dot(axis.vector())))
                            .filter(|point| hit.contains(&pointer.model, *point))
                    }
                _ => Some(hit.point),
            };
            if let Some(point) = point.filter(|point| near(*point).is_some()) {
                return Some(Snap { point, kind: hit.inference() });
            }
        }
        if let Some(start) = start {
            if let Some(constraint) = constraint {
                let (point, kind) = match constraint {
                    AxisConstraint::Axis(axis) => {
                        (ray.axis(start, axis.vector())?, Inference::Axis(axis))
                    }
                    AxisConstraint::Plane(axis) => {
                        (plane_point(ray, start, axis.vector())?, Inference::Plane)
                    }
                };
                return ((point - ray.origin).dot(ray.direction) > 0.)
                    .then_some(Snap { point, kind });
            }
            if self.doc.snap_to_axes {
                if let Some((point, axis, _)) = [Axis::X, Axis::Y, Axis::Z]
                    .into_iter()
                    .filter_map(|axis| {
                        let point = ray.axis(start, axis.vector())?;
                        if point.distance_squared(start) < 1e-12 {
                            return None;
                        }
                        near(point).map(|distance| (point, axis, distance))
                    })
                    .min_by(|a, b| a.2.total_cmp(&b.2))
                {
                    return Some(Snap {
                        point,
                        kind: Inference::Axis(axis),
                    });
                }
            }
        }
        if self.snap_to_face
            && self.doc.snap_to_geometry
            && let Some(hit) = pointer.hit.filter(|hit| hit.face.is_some() || hit.section)
        {
            return Some(Snap {
                point: hit.point,
                kind: hit.inference(),
            });
        }
        plane_point(ray, start.unwrap_or(DVec3::ZERO), normal).map(|point| Snap {
            point,
            kind: Inference::Plane,
        })
    }

    fn xr_shape_plane(&mut self, ray: Ray) {
        let (Some(drawing), Some(snap)) = (&self.drawing, self.snap) else {
            return;
        };
        if self.lock.is_none()
            && drawing.on_surface
            && matches!(snap.kind, Inference::Axis(_))
            && (snap.point - drawing.origin)
                .dot(drawing.surface_normal)
                .abs()
                > 1e-6
        {
            self.snap = plane_point(ray, drawing.origin, drawing.normal).map(|point| Snap {
                point,
                kind: Inference::Plane,
            });
        }
        if self.lock.is_none()
            && let Some(snap) = self.snap.filter(|snap| snap.kind != Inference::Plane)
        {
            self.drawing
                .as_mut()
                .unwrap()
                .align_to(snap.point, ray.origin);
        }
    }

    fn xr_measurement_offset(&mut self, ray: Ray) -> bool {
        if self.tool != Tool::Dimensions {
            return true;
        }
        let Some(m) = self
            .measurement
            .as_ref()
            .filter(|m| !m.complete && m.points.len() == 2)
        else {
            return true;
        };
        let (a, b) = (m.points[0], m.points[1]);
        let center = (a + b) * 0.5;
        let direction = self
            .dimension_preview()
            .unwrap()
            .line_direction(&self.doc.model);
        let normal = self.plane_lock().map(Axis::vector).unwrap_or(m.normal);
        let offset = if let Some(axis) = self
            .single_axis_lock()
            .filter(|axis| (b - a).dot(axis.vector()).abs() < 1e-7)
        {
            ray.axis(center, axis.vector()).map(|point| point - center)
        } else {
            plane_point(ray, center, normal).map(|point| {
                let delta = point - center;
                delta - direction * delta.dot(direction)
            })
        };
        self.snap = None;
        if let Some(offset) = offset {
            self.measurement.as_mut().unwrap().offset = offset;
        }
        offset.is_some()
    }
}

#[cfg(test)]
mod precision_panel_tests {
    use super::*;
    use crate::test_ui::ContextExt;

    #[derive(Default)]
    struct SavedStorage(std::collections::BTreeMap<String, String>, bool);
    impl eframe::Storage for SavedStorage {
        fn get_string(&self, key: &str) -> Option<String> { self.0.get(key).cloned() }
        fn set_string(&mut self, key: &str, value: String) { self.0.insert(key.into(), value); }
        fn remove_string(&mut self, key: &str) { self.0.remove(key); }
        fn flush(&mut self) { self.1 = true; }
    }

    #[test]
    fn xr_axis_lock_buttons_save_plane_and_unlock_without_model_edits() {
        let (mut app, mut state, ctx) = fixture(false);
        let mut storage = SavedStorage::default();
        let mut preferences = app.xr_panel;
        let mut frame = |events, storage: &mut SavedStorage| ctx.run_test_ui(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000., 800.))),
            focused: true, events, ..Default::default()
        }, |ui| app.xr_tool_controls(ui, storage, &mut state, &mut preferences));
        fn position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
            fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
                match shape {
                    egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.rect.center().to_vec2()),
                    egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
                    _ => None,
                }
            }
            output.shapes.iter().find_map(|shape| find(&shape.shape, label)).expect("visible lock button")
        }
        let mut output = frame(Vec::new(), &mut storage);
        // The third axis is unavailable while two axes define a plane.
        for label in ["X", "Y", "Z", "Unlock"] {
            let pos = position(&output, label);
            for pressed in [true, false] {
                output = frame(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                    pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
                }], &mut storage);
            }
            let mut restored = BenchUpApp::with_document(Document::default(), None, true);
            restored.restore_inspector_preferences(&egui::Context::default(), Some(&storage));
            assert_eq!((restored.lock, restored.second_lock), match label {
                "X" => (Some(Axis::X), None),
                "Y" | "Z" => (Some(Axis::X), Some(Axis::Y)),
                _ => (None, None),
            });
        }
        assert!(storage.1);
        assert!(!app.project.modified);
        assert!(app.dirty_since.is_none());
    }

    #[test]
    fn xr_axis_lock_refresh_and_legacy_enter_keep_world_targets() {
        let (mut app, mut state, ctx) = fixture(false);
        app.viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000., 800.));
        let _ = ctx.run_test_ui(egui::RawInput {
            events: vec![egui::Event::PointerMoved(egui::pos2(400., 300.))], ..Default::default()
        }, |_| {
            crate::app_host::set_world_input(&ctx, true);
            app.toggle_axis_lock(Axis::X, &ctx);
            assert!(app.snap.is_none(), "panel cursor must not become a scene target");
            let pointer = Pointer { stamp: None, handle: None, view: None, ray: Ray { origin: DVec3::new(2., 1., 1.), direction: -DVec3::Z },
                model: Arc::new(app.doc.model.clone()), hit: None, activate: false, cancel: false,
                primary_down: false, visible_reference: None };
            state.set_panel_hover(false);
            app.xr_world_pointer(Some(&pointer), &mut state, false, &ctx, &mut Storage);
            assert!(app.snap.unwrap().point.distance(DVec3::new(2., 0., 0.)) < 1e-8);
            app.length_input = "50cm".into();
            app.commit_line(&ctx, true);
            assert!(app.line_start.unwrap().distance(DVec3::new(0.5, 0., 0.)) < 1e-8);
        });
        assert!(!crate::app_host::world_input(&ctx), "host marker expires after its frame");
        app.undo(&ctx, false);
        assert!(app.doc.model.edges.is_empty());
    }

    struct Storage;
    impl eframe::Storage for Storage {
        fn get_string(&self, _: &str) -> Option<String> { None }
        fn set_string(&mut self, _: &str, _: String) {}
        fn remove_string(&mut self, _: &str) {}
        fn flush(&mut self) {}
    }

    fn fixture(rectangle: bool) -> (BenchUpApp, State, egui::Context) {
        let ctx = egui::Context::default();
        let mut app = BenchUpApp::with_document(Document::default(), None, true);
        app.tool = if rectangle { Tool::Rectangle } else { Tool::Line };
        if rectangle {
            app.drawing = Some(Drawing { origin: DVec3::ZERO, surface_normal: DVec3::Z,
                on_surface: false, normal: DVec3::Z, u: DVec3::X, v: DVec3::Y, points: Vec::new() });
        } else {
            app.line_start = Some(DVec3::ZERO);
        }
        let pointer = Pointer { stamp: None, handle: None, view: None, ray: Ray { origin: DVec3::new(2.,1.,1.), direction: -DVec3::Z },
            model: Arc::new(app.doc.model.clone()), hit: None, activate: false, cancel: false,
            primary_down: false, visible_reference: None };
        let mut state = State::default();
        app.xr_world_pointer(Some(&pointer), &mut state, false, &ctx, &mut Storage);
        assert!(app.snap.is_some());
        assert!(state.precision_target.is_some());
        state.set_panel_hover(true);
        (app, state, ctx)
    }

    #[test]
    fn panel_retains_line_direction_for_numeric_commit_then_invalidates_old_target() {
        let (mut app, mut state, ctx) = fixture(false);
        let target = app.snap.unwrap().point;
        for _ in 0..3 { app.xr_world_pointer(None, &mut state, false, &ctx, &mut Storage); }
        assert_eq!(app.snap.unwrap().point, target);
        app.length_input = "50cm".into();
        app.commit_precision_input(&ctx, true);
        let endpoint = app.line_start.unwrap();
        assert!((endpoint.length() - 0.5).abs() < 1e-8);
        assert!(endpoint.normalize().distance(target.normalize()) < 1e-8);
        assert!(!app.doc.model.edges.is_empty());
        app.xr_world_pointer(None, &mut state, false, &ctx, &mut Storage);
        assert!(app.snap.is_none());
        assert!(state.precision_target.is_none());
        app.undo(&ctx, false);
        assert!(app.doc.model.edges.is_empty());
    }

    #[test]
    fn rectangle_numeric_commit_uses_world_plane_without_desktop_refresh() {
        let (mut app, mut state, ctx) = fixture(true);
        app.xr_world_pointer(None, &mut state, false, &ctx, &mut Storage);
        app.length_input = "40cm,20cm".into();
        app.commit_precision_input(&ctx, true);
        assert!(app.drawing.is_none());
        assert!(!app.doc.model.faces.is_empty());
        let (lo, hi) = app.doc.model.bounds().unwrap();
        assert!((hi.x - lo.x - 0.4).abs() < 1e-8);
        assert!((hi.y - lo.y - 0.2).abs() < 1e-8);
    }

    #[test]
    fn panel_target_is_discarded_after_loss_tool_scope_stage_or_geometry_change() {
        for change in 0..5 {
            let (mut app, mut state, ctx) = fixture(false);
            match change {
                0 => state.set_panel_hover(false),
                1 => app.tool = Tool::Rectangle,
                2 => app.doc.model.edit_context = Some(99),
                3 => app.line_start = Some(DVec3::X),
                _ => { app.doc.model.add_edge(DVec3::ZERO, DVec3::Z).unwrap(); app.changed(&ctx); }
            }
            app.xr_world_pointer(None, &mut state, false, &ctx, &mut Storage);
            assert!(app.snap.is_none(), "change={change}");
            state.set_panel_hover(true);
            app.xr_world_pointer(None, &mut state, false, &ctx, &mut Storage);
            assert!(app.snap.is_none(), "no stale restoration, change={change}");
        }
    }
}

#[cfg(test)]
mod section_cap_tests {
    use super::*;

    struct Storage;
    impl eframe::Storage for Storage {
        fn get_string(&self, _: &str) -> Option<String> { None }
        fn set_string(&mut self, _: &str, _: String) {}
        fn remove_string(&mut self, _: &str) {}
        fn flush(&mut self) {}
    }

    fn fixture(grouped:bool, axis:usize) -> (BenchUpApp,Pointer,Option<u64>) {
        let mut doc=Document::default();
        doc.model.draw_polyline(&[DVec3::ZERO,DVec3::X*2.,DVec3::new(2.,2.,0.),DVec3::Y*2.,DVec3::ZERO],false).unwrap();
        let face=*doc.model.faces.keys().next().unwrap();
        doc.model.push_pull(face,2.).unwrap();
        let owner=if grouped { let selected=doc.model.selectable_entities(); Some(doc.model.make_object(&selected,false,"Box").unwrap()) } else { None };
        doc.display.section_cuts=true;
        doc.display.section_fill=true;
        doc.display.section_axes=[axis==0,axis==1,axis==2];
        doc.display.section_height=0.75;
        doc.display.section_offsets=[0.75;2];
        let mut faces=renderer::FaceCache::default();
        let mut entities=renderer::EntityCache::default();
        faces.update(&doc.model);entities.update(&doc.model);
        let mut section=display_scene::SectionCache::default();
        section.geometry(&faces,&entities,&doc.display);
        let mut index=crate::xr::section_picking::Index::default();
        index.prepare(section.generation,&section.triangles,&section.triangle_owners);
        let normal=[DVec3::X,DVec3::Y,DVec3::Z][axis];
        let mut point=DVec3::splat(0.5);point[axis]=0.75;
        let ray=Ray { origin:point+normal*3.,direction:-normal };
        let hit=index.nearest(ray,f64::INFINITY).unwrap();
        assert_eq!(hit.owner,owner);
        assert_eq!(hit.normal,normal);
        let pointer=Pointer { stamp: None, handle: None, view: None, ray,model:Arc::new(doc.model.clone()),hit:Some(Hit {
            entity:hit.owner,section:true,face:None,edge:None,guide:None,guide_points:[None;8],
            flip:None,annotation:None,edge_targets:[None;3],point:hit.point,normal:hit.normal,center:None,
        }), activate:true,cancel:false,primary_down:true,visible_reference:None };
        (BenchUpApp::with_document(doc,None,true),pointer,owner)
    }

    #[test]
    fn grouped_section_cap_can_be_painted_as_a_group_and_undone() {
        let (mut app,pointer,_)=fixture(true,2);
        let original=app.doc.model.clone();
        let ctx=egui::Context::default();
        app.tool=Tool::Paint;app.paint_target=PaintTarget::Group;app.paint_color=[12,34,56];
        app.xr_world_pointer(Some(&pointer),&mut State::default(),true,&ctx,&mut Storage);
        assert_ne!(app.doc.model,original);
        app.undo(&ctx,false);
        assert_eq!(app.doc.model,original);
    }

    #[test]
    fn grouped_section_caps_select_and_move_the_group_with_one_undo() {
        let (mut app,mut pointer,owner)=fixture(true,2);
        let ctx=egui::Context::default();let mut state=State::default();
        let original=app.doc.model.clone();
        app.tool=Tool::Select;
        app.xr_world_pointer(Some(&pointer),&mut state,true,&ctx,&mut Storage);
        assert_eq!(app.selected,owner.into_iter().collect());
        app.tool=Tool::Move;
        app.xr_world_pointer(Some(&pointer),&mut state,true,&ctx,&mut Storage);
        assert!(app.modeling.is_some());
        pointer.activate=false;pointer.primary_down=false;pointer.hit=None;pointer.ray.origin+=DVec3::X*0.25;
        app.xr_world_pointer(Some(&pointer),&mut state,true,&ctx,&mut Storage);
        assert_ne!(app.doc.model,original);
        app.undo(&ctx,false);
        assert_eq!(app.doc.model,original);
    }

    #[test]
    fn section_caps_are_read_only_faces_but_can_anchor_drawing_on_each_axis() {
        for tool in [Tool::PushPull,Tool::Offset,Tool::Paint] {
            let (mut app,pointer,_)=fixture(true,2);
            let original=app.doc.model.clone();app.tool=tool;app.paint_target=PaintTarget::Face;
            app.xr_world_pointer(Some(&pointer),&mut State::default(),true,&egui::Context::default(),&mut Storage);
            assert_eq!(app.doc.model,original);assert!(app.modeling.is_none());
        }
        for axis in 0..3 {
            for tool in [Tool::Line,Tool::Rectangle] {
                let (mut app,pointer,_)=fixture(false,axis);
                app.tool=tool;
                app.xr_world_pointer(Some(&pointer),&mut State::default(),true,&egui::Context::default(),&mut Storage);
                let origin=if tool==Tool::Line { app.line_start.unwrap() } else {
                    let drawing=app.drawing.as_ref().unwrap();
                    assert!(drawing.on_surface);assert_eq!(drawing.normal,pointer.hit.unwrap().normal);drawing.origin
                };
                assert!((origin[axis]-0.75).abs()<1e-8,"axis={axis}, tool={tool:?}, point={origin:?}");
            }
        }
    }
}

#[cfg(test)]
mod precise_modeling_tests {
    use super::*;

    struct Storage;
    impl eframe::Storage for Storage {
        fn get_string(&self, _: &str) -> Option<String> { None }
        fn set_string(&mut self, _: &str, _: String) {}
        fn remove_string(&mut self, _: &str) {}
        fn flush(&mut self) {}
    }

    fn fixture(tool: Tool, precise: bool) -> (BenchUpApp, State, egui::Context, u64) {
        let mut document = Document::default();
        document.model.draw_polyline(&[DVec3::ZERO, DVec3::X * 2., DVec3::new(2.,1.,0.), DVec3::Y, DVec3::ZERO], false).unwrap();
        let face = *document.model.faces.keys().next().unwrap();
        let mut app = BenchUpApp::with_document(document, None, true);
        app.tool = tool;
        app.xr_panel.precise_modeling = precise;
        (app, State::default(), egui::Context::default(), face)
    }

    #[test]
    fn world_linear_controller_uses_exact_geometry_height_and_offset_with_grid() {
        for tool in [Tool::PushPull, Tool::Offset] {
            let (mut app, mut state, ctx, face) = fixture(tool, false);
            let (anchor, target) = if tool == Tool::PushPull {
                (DVec3::new(1., 0.5, 0.), DVec3::new(1., 0.5, 0.37))
            } else { (DVec3::new(1., 0., 0.), DVec3::new(1., 0.37, 0.)) };
            app.doc.model.add_edge(target, target + DVec3::X * 0.4).unwrap();
            app.doc.snap_to_geometry = true; app.snap_to_edge = true; app.snap_to_face = true;
            app.snap_to_face_center = true; app.doc.snap_to_axes = false; app.snap_to_grid = true;
            let original = app.doc.model.clone();
            let start = point(&app, anchor, Some(face), true, true);
            app.xr_world_pointer(Some(&start), &mut state, true, &ctx, &mut Storage);
            let held = point(&app, target, None, false, true);
            app.xr_world_pointer(Some(&held), &mut state, true, &ctx, &mut Storage);
            let edit = app.modeling.as_ref().unwrap();
            assert_eq!(edit.snap.unwrap().kind, Inference::Endpoint, "{tool:?}");
            assert!((edit.value - 0.37).abs() < 1e-8, "{tool:?}: {}", edit.value);
            assert_eq!(app.doc.model, original);
            let released = point(&app, target, None, false, false);
            app.xr_world_pointer(Some(&released), &mut state, true, &ctx, &mut Storage);
            assert!(app.modeling.is_none());
            assert_ne!(app.doc.model, original, "{tool:?}");
            app.undo(&ctx, false);
            assert_eq!(app.doc.model, original);
            assert!(app.doc.history.undo_label().is_none());
        }
    }

    #[test]
    fn world_linear_controller_free_pull_is_not_captured_by_the_starting_face_centre() {
        let (mut app, mut state, ctx, face) = fixture(Tool::PushPull, false);
        app.doc.snap_to_geometry = true; app.snap_to_edge = true; app.snap_to_face = true;
        app.snap_to_face_center = true; app.doc.snap_to_axes = false; app.snap_to_grid = false;
        let anchor = DVec3::new(1., 0.5, 0.);
        let start = point(&app, anchor, Some(face), true, true);
        app.xr_world_pointer(Some(&start), &mut state, true, &ctx, &mut Storage);
        let held = point(&app, anchor + DVec3::Z * 0.37, Some(face), false, true);
        app.xr_world_pointer(Some(&held), &mut state, true, &ctx, &mut Storage);
        let edit = app.modeling.as_ref().unwrap();
        assert!(edit.snap.is_none());
        assert!((edit.value - 0.37).abs() < 1e-8);
    }

    #[test]
    fn world_linear_controller_offset_follows_a_tilted_face_with_geometry_snaps_disabled() {
        let mut doc = Document::default();
        let normal = DVec3::new(0., -1., 1.).normalize();
        let v = DVec3::new(0., 1., 1.).normalize();
        doc.model.draw_polyline(&[DVec3::ZERO, DVec3::X * 2., DVec3::X * 2. + v, v, DVec3::ZERO], false).unwrap();
        let face = *doc.model.faces.keys().next().unwrap();
        let mut app = BenchUpApp::with_document(doc, None, true);
        app.tool = Tool::Offset; app.doc.snap_to_geometry = false; app.doc.snap_to_axes = false; app.snap_to_grid = false;
        let ctx = egui::Context::default(); let mut state = State::default();
        let anchor = DVec3::X;
        let mut start = point(&app, anchor, Some(face), true, true);
        start.ray = Ray { origin: anchor + normal * 2., direction: -normal };
        start.hit.as_mut().unwrap().normal = normal;
        app.xr_world_pointer(Some(&start), &mut state, true, &ctx, &mut Storage);
        let target = anchor + v * 0.2;
        let mut held = point(&app, target, None, false, true);
        held.ray = Ray { origin: target + normal * 2., direction: -normal };
        app.xr_world_pointer(Some(&held), &mut state, true, &ctx, &mut Storage);
        let edit = app.modeling.as_ref().unwrap();
        assert!((edit.value - 0.2).abs() < 1e-8, "{}", edit.value);
        assert!(edit.snap.unwrap().point.distance(target) < 1e-8);
        assert!(edit.snap.unwrap().point.dot(normal).abs() < 1e-8);
    }

    #[test]
    fn world_rotate_controller_projects_off_plane_targets_without_changing_axis() {
        for snapping in [false, true] {
            let (mut app, mut state, ctx, face) = fixture(Tool::Rotate, false);
            let target = DVec3::new(2., 1.5, 1.);
            app.doc.model.add_edge(target, target + DVec3::Y).unwrap();
            app.lock = Some(Axis::Z);
            app.doc.snap_to_geometry = true; app.snap_to_edge = snapping;
            app.snap_to_face = false; app.snap_to_face_center = false; app.doc.snap_to_axes = false;
            app.snap_to_grid = snapping;
            let original = app.doc.model.clone();
            let start = point(&app, DVec3::new(2., 0.5, 0.), Some(face), true, true);
            app.xr_world_pointer(Some(&start), &mut state, true, &ctx, &mut Storage);
            let held = point(&app, target, None, false, true);
            app.xr_world_pointer(Some(&held), &mut state, true, &ctx, &mut Storage);
            let edit = app.modeling.as_ref().unwrap();
            assert_eq!(edit.rotation.as_ref().unwrap().normal, DVec3::Z);
            assert_eq!(edit.snap.unwrap().kind, if snapping { Inference::Endpoint } else { Inference::Plane });
            assert!(edit.snap.unwrap().point.z.abs() < 1e-8);
            assert!((edit.value - 45.).abs() < 1e-8, "snapping {snapping}: {}", edit.value);
            assert_eq!(app.doc.model, original);
            let released = point(&app, target, None, false, false);
            app.xr_world_pointer(Some(&released), &mut state, true, &ctx, &mut Storage);
            assert!(app.modeling.is_none());
            assert_ne!(app.doc.model, original);
            app.undo(&ctx, false);
            assert_eq!(app.doc.model, original);
            assert!(app.doc.history.undo_label().is_none());
        }
    }

    #[test]
    fn world_scale_controller_preserves_exact_target_with_grid_and_one_undo() {
        for lock in [None, Some(Axis::X)] {
            let (mut app, mut state, ctx, face) = fixture(Tool::Scale, false);
            let anchor = DVec3::new(2., 1., 0.);
            let target = if lock.is_some() { DVec3::new(3., 1., 0.) } else { DVec3::new(3., 1.5, 0.) };
            app.doc.model.add_edge(target, target + DVec3::Y).unwrap();
            app.lock = lock;
            app.doc.snap_to_geometry = true; app.snap_to_edge = true; app.doc.snap_to_axes = false;
            app.snap_to_grid = true;
            let original = app.doc.model.clone();
            let start = point(&app, anchor, Some(face), true, true);
            app.xr_world_pointer(Some(&start), &mut state, true, &ctx, &mut Storage);
            let held = point(&app, target, None, false, true);
            app.xr_world_pointer(Some(&held), &mut state, true, &ctx, &mut Storage);
            let edit = app.modeling.as_ref().unwrap();
            assert_eq!(edit.snap.unwrap().kind, Inference::Endpoint);
            assert!(edit.snap.unwrap().point.distance(target) < 1e-8);
            assert!((edit.value - 2.).abs() < 1e-8, "lock {lock:?}: {}", edit.value);
            assert_eq!(app.doc.model, original);
            let released = point(&app, target, None, false, false);
            app.xr_world_pointer(Some(&released), &mut state, true, &ctx, &mut Storage);
            assert!(app.modeling.is_none());
            assert_ne!(app.doc.model, original);
            app.undo(&ctx, false);
            assert_eq!(app.doc.model, original);
            assert!(app.doc.history.undo_label().is_none());
        }
    }

    #[test]
    fn world_move_controller_snaps_to_stationary_endpoint_and_commits_one_undo() {
        let (mut app, mut state, ctx, face) = fixture(Tool::Move, false);
        app.doc.model.add_edge(DVec3::new(3., 0.5, 0.), DVec3::new(3., 1.5, 0.)).unwrap();
        app.doc.snap_to_geometry = true; app.snap_to_edge = true; app.doc.snap_to_axes = false;
        app.snap_to_grid = true;
        let original = app.doc.model.clone();
        let start = point(&app, DVec3::new(1., 0.5, 0.), Some(face), true, true);
        app.xr_world_pointer(Some(&start), &mut state, true, &ctx, &mut Storage);
        let target = DVec3::new(3., 0.5, 0.);
        let held = point(&app, target, None, false, true);
        app.xr_world_pointer(Some(&held), &mut state, true, &ctx, &mut Storage);
        let edit = app.modeling.as_ref().unwrap();
        assert_eq!(edit.snap.unwrap().kind, Inference::Endpoint);
        assert!(edit.snap.unwrap().point.distance(target) < 1e-8);
        assert_eq!(app.doc.model, original);
        let released = point(&app, target, None, false, false);
        app.xr_world_pointer(Some(&released), &mut state, true, &ctx, &mut Storage);
        assert!(app.modeling.is_none());
        assert_ne!(app.doc.model, original);
        app.undo(&ctx, false);
        assert_eq!(app.doc.model, original);
        assert!(app.doc.history.undo_label().is_none());
    }

    fn point(app: &BenchUpApp, position: DVec3, face: Option<u64>, clicked: bool, down: bool) -> Pointer {
        Pointer { stamp: None, handle: None, view: None, ray: Ray { origin: position + DVec3::Z * 2., direction: -DVec3::Z },
            model: Arc::new(app.doc.model.clone()), hit: face.map(|face| Hit {
                entity: Some(face), section: false, face: Some(face), edge: None, guide: None, guide_points: [None;8],
                flip: None, annotation: None, edge_targets: [None;3], point: position,
                normal: DVec3::Z, center: Some(DVec3::new(1.,0.5,0.)),
            }), activate: clicked, cancel: false, primary_down: down, visible_reference: None }
    }

    fn aim_and_confirm(app: &mut BenchUpApp, state: &mut State, ctx: &egui::Context, face: u64, target: DVec3) {
        let anchor = DVec3::new(1.5,0.25,0.);
        let pointer = point(app, anchor, Some(face), true, true);
        app.xr_world_pointer(Some(&pointer), state, true, ctx, &mut Storage);
        assert!(app.modeling.is_some());
        let pointer = point(app, anchor, Some(face), false, false);
        app.xr_world_pointer(Some(&pointer), state, true, ctx, &mut Storage);
        assert_eq!(state.model_phase, Some(ModelPhase::Aiming));
        let pointer = point(app, target, None, true, true);
        app.xr_world_pointer(Some(&pointer), state, true, ctx, &mut Storage);
        assert_eq!(state.model_phase, Some(ModelPhase::Typing));
        assert!(state.take_precision_input_request());
        assert!(!state.take_precision_input_request());
    }

    #[test]
    fn exact_values_for_all_five_modeling_tools_commit_once_and_undo() {
        for (tool, input, target) in [
            (Tool::Move, "25cm", DVec3::new(2.5,0.25,0.)),
            (Tool::Rotate, "45", DVec3::new(1.25,1.,0.)),
            (Tool::Scale, "1.5", DVec3::new(2.,0.,0.)),
            (Tool::PushPull, "30cm", DVec3::new(1.5,0.25,0.2)),
            (Tool::Offset, "5cm", DVec3::new(1.5,0.35,0.)),
        ] {
            let (mut app, mut state, ctx, face) = fixture(tool, true);
            let original = app.doc.model.clone();
            aim_and_confirm(&mut app, &mut state, &ctx, face, target);
            let preview_value = app.modeling.as_ref().unwrap().value;
            let pointer = point(&app, DVec3::splat(100.), None, false, false);
            app.xr_world_pointer(Some(&pointer), &mut state, true, &ctx, &mut Storage);
            assert_eq!(app.modeling.as_ref().unwrap().value, preview_value);
            state.set_panel_hover(true);
            app.xr_world_pointer(None, &mut state, false, &ctx, &mut Storage);
            assert_eq!(app.doc.model, original);
            assert!(app.modeling.is_some());
            app.length_input = input.into();
            app.commit_precision_input(&ctx, true);
            assert!(app.modeling.is_none(), "tool={tool:?}");
            assert_ne!(app.doc.model, original, "tool={tool:?}");
            let (lo, hi) = app.doc.model.bounds().unwrap();
            match tool {
                Tool::Move => assert!((lo.x - 0.25).abs() < 1e-8),
                Tool::Scale => assert!((hi.x - lo.x - 3.).abs() < 1e-8),
                Tool::Rotate => assert!((hi.x - lo.x - 3. / 2f64.sqrt()).abs() < 1e-8),
                Tool::PushPull => assert!((hi.z - lo.z - 0.3).abs() < 1e-8),
                _ => {}
            }
            app.xr_world_pointer(None, &mut state, false, &ctx, &mut Storage);
            assert!(state.model_phase.is_none());
            app.undo(&ctx, false);
            assert_eq!(app.doc.model, original, "one undo, tool={tool:?}");
        }
    }

    #[test]
    fn exact_modeling_cancels_without_edit_on_loss_cancel_tool_or_document_change() {
        for change in 0..4 {
            let (mut app, mut state, ctx, face) = fixture(Tool::Move, true);
            aim_and_confirm(&mut app, &mut state, &ctx, face, DVec3::new(2.5,0.25,0.));
            state.set_panel_hover(true);
            let mut pointer = None;
            match change {
                0 => state.set_panel_hover(false),
                1 => { let mut cancel = point(&app, DVec3::ZERO, None, false, false); cancel.cancel = true; pointer = Some(cancel); }
                2 => app.tool = Tool::Line,
                _ => { app.doc.model.add_edge(DVec3::splat(5.), DVec3::splat(6.)).unwrap(); app.changed(&ctx); }
            }
            let expected = app.doc.model.clone();
            app.xr_world_pointer(pointer.as_ref(), &mut state, true, &ctx, &mut Storage);
            assert!(app.modeling.is_none(), "change={change}");
            assert!(state.model_phase.is_none());
            assert_eq!(app.doc.model, expected);
        }
    }

    #[test]
    fn default_mode_still_commits_on_trigger_release() {
        let (mut app, mut state, ctx, face) = fixture(Tool::Move, false);
        let original = app.doc.model.clone();
        let pointer = point(&app, DVec3::new(1.5,0.25,0.), Some(face), true, true);
        app.xr_world_pointer(Some(&pointer), &mut state, true, &ctx, &mut Storage);
        let pointer = point(&app, DVec3::new(2.5,0.25,0.), None, false, false);
        app.xr_world_pointer(Some(&pointer), &mut state, true, &ctx, &mut Storage);
        assert!(app.modeling.is_none());
        assert!(state.model_phase.is_none());
        assert!(!state.take_precision_input_request());
        assert_ne!(app.doc.model, original);
    }
}

#[cfg(test)]
mod erase_stroke_tests {
    use super::*;

    #[derive(Default)]
    struct Storage;
    impl eframe::Storage for Storage {
        fn get_string(&self, _: &str) -> Option<String> { None }
        fn set_string(&mut self, _: &str, _: String) {}
        fn remove_string(&mut self, _: &str) {}
        fn flush(&mut self) {}
    }

    fn fixture() -> (BenchUpApp, Arc<Model>, [u64; 2]) {
        let mut doc=Document::default();
        let first=doc.model.add_edge(DVec3::ZERO,DVec3::X).unwrap();
        let second=doc.model.add_edge(DVec3::Y,DVec3::X+DVec3::Y).unwrap();
        let model=Arc::new(doc.model.clone());
        let mut app=BenchUpApp::with_document(doc,None,true);
        app.set_tool(Tool::Erase);
        (app,model,[first,second])
    }

    fn pointer(model: &Arc<Model>, entity: Option<u64>, down: bool, activate: bool) -> Pointer {
        Pointer {
            stamp: None,
            handle: None,
            view: None,
            ray:Ray {origin:DVec3::new(0.5,0.,1.),direction:-DVec3::Z},
            model:Arc::clone(model),
            hit:entity.map(|entity|Hit {
                entity:Some(entity),section:false,face:None,edge:Some(entity),guide:None,guide_points:[None;8],
                flip:None,annotation:None,edge_targets:[None;3],
                point:DVec3::new(0.5,0.,0.),normal:DVec3::Z,center:None,
            }),
            activate,cancel:false,primary_down:down,visible_reference:None,
        }
    }

    #[test]
    fn held_erase_commits_all_targets_on_release_as_one_undo() {
        let (mut app,model,ids)=fixture();
        let ctx=egui::Context::default();
        let mut state=State::default();
        let mut storage=Storage;
        for (index,id) in ids.into_iter().enumerate() {
            app.xr_world_pointer(Some(&pointer(&model,Some(id),true,index==0)),&mut state,true,&ctx,&mut storage);
            assert_eq!(app.doc.model.edges.len(),2);
        }
        assert_eq!(app.xr_erasing,BTreeSet::from(ids));
        app.xr_world_pointer(Some(&pointer(&model,None,false,false)),&mut state,true,&ctx,&mut storage);
        assert!(app.doc.model.edges.is_empty());
        assert!(app.xr_erasing.is_empty());
        app.undo(&ctx,false);
        assert_eq!(app.doc.model,*model);
    }

    #[test]
    fn erase_sweep_targets_join_the_release_transaction_and_undo_together() {
        let (mut app, model, ids) = fixture();
        let ctx = egui::Context::default();
        let mut state = State::default();
        app.xr_world_pointer(Some(&pointer(&model, Some(ids[0]), true, true)), &mut state, true, &ctx, &mut Storage);
        app.xr_set_erase_sweep(&mut state, BTreeSet::from([ids[1], u64::MAX]));
        // Repeated egui layout must not consume the pending interval early.
        app.xr_world_pointer(Some(&pointer(&model, None, true, false)), &mut state, false, &ctx, &mut Storage);
        assert_eq!(app.xr_erasing, BTreeSet::from([ids[0]]));
        app.xr_world_pointer(Some(&pointer(&model, None, false, false)), &mut state, true, &ctx, &mut Storage);
        assert!(app.doc.model.edges.is_empty());
        assert!(state.erase_sweep.is_none());
        app.undo(&ctx, false);
        assert_eq!(app.doc.model, *model);
    }

    #[test]
    fn captured_erase_release_over_a_joint_handle_keeps_the_release_transaction() {
        let (mut app, model, ids) = fixture();
        let ctx = egui::Context::default();
        let mut state = State::default();
        app.xr_world_pointer(Some(&pointer(&model, Some(ids[0]), true, true)), &mut state, true, &ctx, &mut Storage);
        let mut release = pointer(&model, None, false, false);
        release.handle = Some((super::viewport_overlays::HandleAction::EditJoint(123), DVec3::ZERO));
        app.xr_world_pointer(Some(&release), &mut state, true, &ctx, &mut Storage);
        assert!(!app.doc.model.edges.contains_key(&ids[0]));
        assert!(app.doc.model.edges.contains_key(&ids[1]));
        assert!(!app.joinery.open);
        app.undo(&ctx, false);
        assert_eq!(app.doc.model, *model);
    }

    #[test]
    fn erase_sweep_and_collected_targets_cancel_when_the_document_changes() {
        let (mut app, model, ids) = fixture();
        let ctx = egui::Context::default();
        let mut state = State::default();
        app.xr_world_pointer(Some(&pointer(&model, Some(ids[0]), true, true)), &mut state, true, &ctx, &mut Storage);
        app.xr_set_erase_sweep(&mut state, BTreeSet::from([ids[1]]));
        app.doc.model.add_edge(DVec3::X * 2., DVec3::X * 3.).unwrap();
        app.changed(&ctx);
        let changed = app.doc.model.clone();
        app.xr_world_pointer(Some(&pointer(&model, None, false, false)), &mut state, true, &ctx, &mut Storage);
        assert_eq!(app.doc.model, changed);
        assert!(app.xr_erasing.is_empty());
        assert!(!state.erasing);
        assert!(state.erase_sweep.is_none());
    }

    #[test]
    fn lost_pointer_and_cancel_discard_erase_without_later_release_commit() {
        for cancel in 0..3 {
            let (mut app,model,ids)=fixture();
            let ctx=egui::Context::default();
            let mut state=State::default();
            let mut storage=Storage;
            app.xr_world_pointer(Some(&pointer(&model,Some(ids[0]),true,true)),&mut state,true,&ctx,&mut storage);
            app.xr_set_erase_sweep(&mut state, BTreeSet::from([ids[1]]));
            if cancel == 1 {
                let mut input=pointer(&model,Some(ids[1]),true,false);
                input.cancel=true;
                app.xr_world_pointer(Some(&input),&mut state,true,&ctx,&mut storage);
            } else if cancel == 0 {
                app.xr_world_pointer(None,&mut state,false,&ctx,&mut storage);
            } else {
                state.cancel_erasing(&mut app);
            }
            app.xr_world_pointer(Some(&pointer(&model,None,false,false)),&mut state,true,&ctx,&mut storage);
            assert_eq!(app.doc.model,*model);
            assert!(app.xr_erasing.is_empty());
            assert!(!state.erasing);
            assert!(state.erase_sweep.is_none());
        }
    }
}

#[cfg(test)]
mod window_selection_tests {
    use super::*;
    struct Storage;
    impl eframe::Storage for Storage {
        fn get_string(&self, _: &str) -> Option<String> { None }
        fn set_string(&mut self, _: &str, _: String) {}
        fn remove_string(&mut self, _: &str) {}
        fn flush(&mut self) {}
    }
    fn fixture() -> (BenchUpApp, egui::Context, u64, u64) {
        let mut doc = Document::default();
        let edge = doc.model.add_edge(DVec3::new(0.1, 0.1, 1.), DVec3::new(0.2, 0.2, 1.)).unwrap();
        let group = doc.model.make_object(&BTreeSet::from([edge]), false, "Inside").unwrap();
        doc.model.add_edge(DVec3::new(0.3, 0.3, 1.), DVec3::new(0.6, 0.6, 1.)).unwrap();
        let outside = doc.model.add_edge(DVec3::new(0.7, 0.7, 1.), DVec3::new(0.9, 0.9, 1.)).unwrap();
        let mut app = BenchUpApp::with_document(doc, None, true);
        app.xr_panel.window_selection = true;
        app.project.modified = false;
        app.selected = BTreeSet::from([outside]);
        (app, egui::Context::default(), group, outside)
    }
    fn pointer(app: &BenchUpApp, end: bool, down: bool, clicked: bool) -> Pointer {
        Pointer { stamp: None, handle: None, view: Some(super::super::xr_window_selection::View {
            head: DVec3::ZERO, right: DVec3::X, up: DVec3::Y, units: 1., font_scale: 1., transform: glam::DMat4::IDENTITY,
        }), model: Arc::new(app.doc.model.clone()), ray: Ray { origin: DVec3::ZERO,
            direction: if end { DVec3::new(0.4, 0.4, 1.).normalize() } else { DVec3::Z } },
            hit: None, activate: clicked, cancel: false, primary_down: down, visible_reference: None }
    }
    fn send(app: &mut BenchUpApp, state: &mut State, ctx: &egui::Context, end: bool, down: bool, clicked: bool) {
        let input = pointer(app, end, down, clicked);
        app.xr_world_pointer(Some(&input), state, true, ctx, &mut Storage);
    }

    #[test]
    fn window_selection_commits_enclosed_groups_on_release_and_preserves_model_history() {
        for toggle in [false, true] {
            let (mut app, ctx, group, outside) = fixture();
            if toggle { app.selected.insert(group); app.xr_panel.extend_selection = true; }
            let before = app.doc.model.clone();
            let selected = app.selected.clone();
            let mut state = State::default();
            send(&mut app, &mut state, &ctx, false, true, true);
            send(&mut app, &mut state, &ctx, true, true, false);
            assert_eq!(app.selected, selected);
            assert!(state.capturing());
            let mut overlays = super::super::viewport_overlays::Overlays::default();
            state.append_window_overlay(&mut overlays);
            assert_eq!(overlays.overlay_segments.len(), 4);
            send(&mut app, &mut state, &ctx, true, false, false);
            assert_eq!(app.selected, if toggle { BTreeSet::from([outside]) } else { BTreeSet::from([group]) });
            assert!(!state.capturing());
            assert_eq!(app.doc.model, before);
            assert!(app.doc.history.undo_label().is_none());
            assert!(!app.project.modified);
        }
    }

    #[test]
    fn window_selection_cancels_without_applying_pending_hits() {
        for cause in 0..6 {
            let (mut app, ctx, _, _) = fixture();
            let mut state = State::default();
            send(&mut app, &mut state, &ctx, false, true, true);
            send(&mut app, &mut state, &ctx, true, true, false);
            let mut release = pointer(&app, true, false, false);
            match cause {
                0 => release.cancel = true,
                1 => { app.xr_world_pointer(None, &mut state, false, &ctx, &mut Storage); }
                2 => { app.doc.model.add_edge(DVec3::X * 2., DVec3::X * 3.).unwrap(); app.changed(&ctx); }
                3 => release.view.as_mut().unwrap().transform = glam::DMat4::from_scale(DVec3::splat(2.)),
                4 => { let doc = app.doc.clone(); app.replace_project(doc, None, &ctx); }
                _ => app.xr_panel.window_selection = false,
            }
            let selected = app.selected.clone();
            app.xr_world_pointer(Some(&release), &mut state, true, &ctx, &mut Storage);
            assert_eq!(app.selected, selected, "cause {cause}");
            assert!(state.window.is_none());
        }
    }
}

#[cfg(test)]
mod input_revision_tests {
    use super::*;
    use crate::test_ui::ContextExt;

    struct Storage;
    impl eframe::Storage for Storage {
        fn get_string(&self, _: &str) -> Option<String> { None }
        fn set_string(&mut self, _: &str, _: String) {}
        fn remove_string(&mut self, _: &str) {}
        fn flush(&mut self) {}
    }

    fn fixture() -> (BenchUpApp, egui::Context, u64, u64) {
        let mut doc = Document::default();
        doc.model.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        let group = doc.model.make_object(&doc.model.selectable_entities(), false, "Part").unwrap();
        let edge = *doc.model.edges.keys().next().unwrap();
        let mut app = BenchUpApp::with_document(doc, None, true);
        app.set_tool(Tool::Select);
        (app, egui::Context::default(), edge, group)
    }

    fn pointer(app: &mut BenchUpApp, ctx: &egui::Context, edge: u64) -> Pointer {
        let mut source = None;
        let _ = ctx.run_test_ui(Default::default(), |_| { source = Some(app.xr_scene_source(ctx)); });
        let source = source.unwrap();
        Pointer {
            stamp: Some(source.input_stamp), handle: None, view: None, model: source.model,
            ray: Ray { origin: DVec3::new(0.5, 0., 1.), direction: -DVec3::Z },
            hit: Some(Hit { entity: app.doc.model.entity_in_context(edge), section: false,
                face: None, edge: Some(edge), guide: None, guide_points: [None; 8], flip: None,
                annotation: None, edge_targets: [None; 3], point: DVec3::X * 0.5,
                normal: DVec3::Z, center: None }),
            activate: true, cancel: false, primary_down: true, visible_reference: None,
        }
    }

    #[test]
    fn global_alignment_references_reach_the_shared_controller_drawing_preview() {
        struct References;
        impl crate::xr::world_inference::ReferenceQuery for References {
            fn alignment(&self, _: Ray, _: (DVec3, DVec3), _: Option<(DVec3, AxisConstraint)>) -> [Option<AxisGuide>; 3] {
                [None, Some(AxisGuide { origin: DVec3::new(2., 3., 0.), axis: Axis::Y }), None]
            }
        }
        let (mut app, ctx, edge, _) = fixture();
        app.set_tool(Tool::Line);
        app.line_start = Some(DVec3::ZERO);
        app.doc.snap_to_axes = true;
        app.doc.snap_to_geometry = true;
        app.snap_to_edge = true;
        app.snap_to_grid = false;
        let before = app.doc.model.clone();
        let mut input = pointer(&mut app, &ctx, edge);
        input.hit = None;
        input.ray = Ray { origin: DVec3::new(2.004, 0.1, 2.), direction: -DVec3::Z };
        input.activate = false;
        input.primary_down = false;
        app.xr_world_pointer_with_references(Some(&input), &mut State::default(), true, &ctx, &mut Storage, Some(&References));
        assert!(app.snap.unwrap().point.distance(DVec3::new(2., 0.1, 0.)) < 1e-8);
        assert_eq!(app.snap.unwrap().kind, Inference::Aligned);
        assert!(app.drawing_axis_guides.iter().any(|guide| guide.origin == DVec3::new(2., 3., 0.) && guide.axis == Axis::Y));
        assert_eq!(app.doc.model, before);
        assert!(app.doc.history.undo_label().is_none());
    }

    #[test]
    fn controller_selection_clicks_open_groups_and_empty_clicks_leave_context() {
        for window in [false, true] {
            let (mut app, ctx, edge, group) = fixture();
            app.xr_panel.window_selection = window;
            app.project.modified = false;
            let mut state = State::default();
            let click = |app: &mut BenchUpApp, state: &mut State, empty: bool| {
                let mut input = pointer(app, &ctx, edge);
                if empty { input.hit = None; }
                input.view = Some(super::super::xr_window_selection::View {
                    head: DVec3::new(0.5, 0., 1.), right: DVec3::X, up: DVec3::Y,
                    units: 1., font_scale: 1., transform: glam::DMat4::IDENTITY,
                });
                app.xr_world_pointer(Some(&input), state, true, &ctx, &mut Storage);
                input.activate = false;
                input.primary_down = false;
                app.xr_world_pointer(Some(&input), state, true, &ctx, &mut Storage);
            };
            click(&mut app, &mut state, false);
            assert_eq!(app.selected, BTreeSet::from([group]));
            assert_eq!(app.doc.model.edit_context, None);
            click(&mut app, &mut state, false);
            assert_eq!(app.doc.model.edit_context, Some(group));
            assert!(app.selected.is_empty());
            click(&mut app, &mut state, true);
            assert_eq!(app.doc.model.edit_context, None);
            assert!(app.selected.is_empty());
            assert!(app.doc.history.undo_label().is_none());
            assert!(!app.project.modified);
        }
    }

    #[test]
    fn controller_selection_double_clicks_edit_text_and_respect_interruptions() {
        for cause in 0..6 {
            let (mut app, ctx, edge, _) = fixture();
            let text = app.doc.model.add_text(DVec3::Y, DVec3::X, "Label").unwrap();
            app.project.modified = false;
            let mut state = State::default();
            let mut input = pointer(&mut app, &ctx, edge);
            input.hit.as_mut().unwrap().entity = Some(text);
            app.xr_world_pointer(Some(&input), &mut state, true, &ctx, &mut Storage);
            assert!(app.text_editor.is_none());
            match cause {
                1 => app.xr_world_pointer(None, &mut state, true, &ctx, &mut Storage),
                2 => app.xr_project_epoch = app.xr_project_epoch.wrapping_add(1),
                3 => app.xr_panel.extend_selection = true,
                4 => { let _ = ctx.run_test_ui(egui::RawInput { time: Some(100.), ..Default::default() }, |_| {}); }
                5 => app.doc.display.guides = !app.doc.display.guides,
                _ => {}
            }
            let mut input = pointer(&mut app, &ctx, edge);
            input.hit.as_mut().unwrap().entity = Some(text);
            app.xr_world_pointer(Some(&input), &mut state, true, &ctx, &mut Storage);
            assert_eq!(app.text_editor.is_some(), cause == 0, "interruption {cause}");
            assert!(app.doc.history.undo_label().is_none());
            assert!(!app.project.modified);
        }
    }

    #[test]
    fn input_revision_rejects_first_click_after_model_context_tool_display_or_project_changes() {
        for cause in 0..5 {
            let (mut app, ctx, edge, group) = fixture();
            let old = pointer(&mut app, &ctx, edge);
            assert!(app.xr_pointer_is_current(&old, &ctx));
            match cause {
                0 => { app.doc.model.add_edge(DVec3::Y * 2., DVec3::X + DVec3::Y * 2.).unwrap(); app.changed(&ctx); }
                1 => app.doc.model.edit_context = Some(group),
                2 => app.set_tool(Tool::Line),
                3 => { app.doc.display.section_cuts = !app.doc.display.section_cuts; app.changed(&ctx); }
                _ => {
                    let same = app.doc.clone();
                    app.replace_project(same, None, &ctx);
                    assert_ne!(old.stamp.as_ref().unwrap().project, app.xr_project_epoch);
                }
            }
            let model = app.doc.model.clone();
            let mut state = State::default();
            assert!(!app.xr_pointer_is_current(&old, &ctx), "cause {cause}");
            app.xr_world_pointer(Some(&old), &mut state, true, &ctx, &mut Storage);
            assert_eq!(app.doc.model, model);
            assert!(app.selected.is_empty());
            assert!(app.line_start.is_none());
            assert!(!state.active);
            let fresh = pointer(&mut app, &ctx, edge);
            assert!(app.xr_pointer_is_current(&fresh, &ctx));
            app.xr_world_pointer(Some(&fresh), &mut state, true, &ctx, &mut Storage);
            assert!(state.active, "fresh source must restore interaction");
            if app.tool == Tool::Select { assert!(app.selected.contains(&app.doc.model.entity_in_context(edge).unwrap())); }
            else { assert!(app.line_start.is_some()); }
        }
    }

    #[test]
    fn input_revision_cancels_an_erase_before_a_same_frame_model_edit_can_commit() {
        let (mut app, ctx, edge, _) = fixture();
        app.set_tool(Tool::Erase);
        let mut old = pointer(&mut app, &ctx, edge);
        let mut state = State::default();
        app.xr_world_pointer(Some(&old), &mut state, true, &ctx, &mut Storage);
        assert!(state.erasing);
        app.doc.model.add_edge(DVec3::Y * 2., DVec3::X + DVec3::Y * 2.).unwrap();
        app.changed(&ctx);
        let changed = app.doc.model.clone();
        old.activate = false;
        old.primary_down = false;
        app.xr_world_pointer(Some(&old), &mut state, true, &ctx, &mut Storage);
        assert_eq!(app.doc.model, changed);
        assert!(!state.erasing);
        assert!(app.xr_erasing.is_empty());
    }

    #[test]
    fn input_revision_allows_selection_and_pending_preview_changes_with_a_current_model() {
        let (mut app, ctx, edge, group) = fixture();
        let mut current = pointer(&mut app, &ctx, edge);
        let mut state = State::default();
        app.xr_world_pointer(Some(&current), &mut state, true, &ctx, &mut Storage);
        assert!(app.selected.contains(&group));
        current.activate = false;
        app.xr_world_pointer(Some(&current), &mut state, true, &ctx, &mut Storage);
        assert!(state.active);
        app.set_tool(Tool::Line);
        let line = pointer(&mut app, &ctx, edge);
        app.xr_world_pointer(Some(&line), &mut state, true, &ctx, &mut Storage);
        assert!(app.line_start.is_some());
        assert!(app.xr_pointer_is_current(&line, &ctx), "pending tool state does not change the source model");
        app.set_tool(Tool::Move);
        app.selected = BTreeSet::from([group]);
        let before = app.doc.model.clone();
        let mut moving = pointer(&mut app, &ctx, edge);
        app.xr_world_pointer(Some(&moving), &mut state, true, &ctx, &mut Storage);
        moving.activate = false;
        moving.ray.origin.x += 0.25;
        app.xr_world_pointer(Some(&moving), &mut state, true, &ctx, &mut Storage);
        assert!(app.modeling.as_ref().is_some_and(|edit| edit.preview.is_some()));
        assert_eq!(app.doc.model, before);
        assert!(app.xr_pointer_is_current(&moving, &ctx));
        let preview = pointer(&mut app, &ctx, edge);
        assert_eq!(preview.stamp, moving.stamp, "rendered preview geometry must not invalidate committed-model input");
    }
}

#[cfg(test)]
mod scene_source_tests {
    use super::*;

    fn source(app:&mut BenchUpApp, ctx:&egui::Context) -> crate::xr::immersive::Source {
        let mut result=None;
        let mut output=ctx.run_ui(Default::default(),|_|result=Some(app.xr_scene_source(ctx)));
        output.textures_delta.clear();
        result.unwrap()
    }

    #[test]
    fn source_refreshes_after_edits_without_painting_a_desktop_viewport() {
        let mut doc=Document::default();
        doc.model.add_edge(DVec3::ZERO,DVec3::X).unwrap();
        let mut app=BenchUpApp::with_document(doc,None,true);
        let ctx=egui::Context::default();
        let initial=source(&mut app,&ctx);
        let before=app.doc.model.clone();
        let edge=app.doc.model.add_edge(DVec3::Y,DVec3::X+DVec3::Y).unwrap();
        app.finish_edit("Add test edge",before,&ctx);
        let changed=source(&mut app,&ctx);
        assert!(changed.model.edges.contains_key(&edge));
        assert_ne!(changed.revision,initial.revision);
        let stable=source(&mut app,&ctx);
        assert!(Arc::ptr_eq(&changed.model,&stable.model));
        assert_eq!(changed.revision,stable.revision);
    }

    #[test]
    fn source_tracks_editing_context_without_revising_geometry_every_frame() {
        let mut doc=Document::default();
        let edge=doc.model.add_edge(DVec3::ZERO,DVec3::X).unwrap();
        let group=doc.model.make_object(&BTreeSet::from([edge]),false,"Test group").unwrap();
        let edge=*doc.model.objects[&group].entities.iter().find(|id|doc.model.edges.contains_key(id)).unwrap();
        let mut app=BenchUpApp::with_document(doc,None,true);
        let ctx=egui::Context::default();
        let root=source(&mut app,&ctx);
        let geometry_revision=app.render_model.revision();
        assert_eq!(root.model.entity_in_context(edge),Some(group));
        app.enter_object(group,&ctx);
        let scoped=source(&mut app,&ctx);
        assert_eq!(scoped.model.edit_context,Some(group));
        assert_eq!(scoped.model.entity_in_context(edge),Some(edge));
        assert_eq!(app.render_model.revision(),geometry_revision);
        let stable=source(&mut app,&ctx);
        assert!(Arc::ptr_eq(&scoped.model,&stable.model));
        app.close_edit_context(&ctx);
        let closed=source(&mut app,&ctx);
        assert_eq!(closed.model.edit_context,None);
        assert_eq!(closed.model.entity_in_context(edge),Some(group));
    }
}
