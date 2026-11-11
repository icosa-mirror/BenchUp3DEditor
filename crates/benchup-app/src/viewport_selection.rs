//! Preview selection is visual state, independent of document selection/undo.
use super::*;
use std::cell::RefCell;

type RemapKey = (u64, usize, Option<u64>, Tool, bool);

pub(super) struct PreparedSelection {
    roots: BTreeSet<u64>,
    evaluated: RefCell<Option<(RemapKey, BTreeSet<u64>)>>,
}

impl PreparedSelection {
    pub(super) fn new(roots: BTreeSet<u64>) -> Self { Self { roots, evaluated: RefCell::new(None) } }
    pub(super) fn update(slot: &mut Option<Self>, roots: BTreeSet<u64>) {
        if !slot.as_ref().is_some_and(|old| old.roots == roots) { *slot = Some(Self::new(roots)); }
    }
}

impl BenchUpApp {
    pub(super) fn viewport_selection(&self, model: &Model, revision: u64) -> BTreeSet<u64> {
        if self.history_preview.is_some() { return BTreeSet::new(); }
        let Some(edit) = self.modeling.as_ref().filter(|edit| edit.preview.is_some()) else { return self.selected.clone(); };
        let Some(prepared) = &edit.preview_selection else { return self.selected.clone(); };
        if prepared.roots.iter().all(|id| model.contains_entity(id)) { return prepared.roots.clone(); }

        // Evaluation may replace primitive IDs. Preserve the desktop selection
        // policy for that backend case, once per rendered snapshot rather than
        // once per eye/head pose. Ordinary previews use operation-returned IDs.
        let key = (revision, model as *const Model as usize, model.edit_context, self.tool, edit.copy);
        if let Some((old, selected)) = prepared.evaluated.borrow().as_ref() && *old == key { return selected.clone(); }
        let selected: BTreeSet<_> = if self.tool == Tool::PushPull {
            model.faces.keys().filter(|id| !edit.base.faces.contains_key(id) && model.parent(**id) == model.edit_context)
                .next_back().copied().or(edit.face).filter(|id| model.contains_entity(id)).into_iter().collect()
        } else if edit.copy {
            model.selectable_entities().into_iter().filter(|id| !edit.base.contains_entity(id)).collect()
        } else {
            prepared.roots.iter().filter(|id| model.contains_entity(id)).copied().collect()
        };
        *prepared.evaluated.borrow_mut() = Some((key, selected.clone()));
        selected
    }
}

#[cfg(all(test, feature = "openxr", not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::test_ui::ContextExt as _;
    fn fixture(scoped: bool) -> (BenchUpApp, u64) {
        let mut doc = Document::default();
        doc.model.draw_polyline(&[DVec3::ZERO, DVec3::X * 2., DVec3::new(2., 1., 0.), DVec3::Y, DVec3::ZERO], false).unwrap();
        if scoped {
            let group = doc.model.make_object(&doc.model.selectable_entities(), false, "Part").unwrap();
            assert!(doc.model.enter_object(group));
        }
        let face = *doc.model.faces.keys().find(|id| doc.model.entity_in_context(**id) == Some(**id)).unwrap();
        let mut app = BenchUpApp::with_document(doc, None, true);
        app.selected = BTreeSet::from([face]);
        (app, face)
    }
    fn begin(app: &mut BenchUpApp, tool: Tool, face: u64) {
        app.tool = tool;
        app.begin_modeling_at_anchor(DVec3::new(1., 0.5, 0.), DVec3::Z,
            (tool == Tool::PushPull).then_some(face), Pos2::ZERO,
            benchup_core::Ray { origin: DVec3::new(1., 0.5, 2.), direction: -DVec3::Z });
    }
    fn source(app: &mut BenchUpApp, ctx: &egui::Context) -> crate::xr::immersive::Source {
        let mut output = None;
        let _ = ctx.run_test_ui(Default::default(), |_| { output = Some(app.xr_scene_source(ctx)); });
        output.unwrap()
    }

    #[test]
    fn preview_selection_highlights_copies_and_restores_on_history_mode_change_and_cancel() {
        let (mut app, face) = fixture(false);
        let original = app.doc.model.clone(); let selected = app.selected.clone();
        app.move_copy = true;
        begin(&mut app, Tool::Move, face);
        let edit = app.modeling.as_mut().unwrap(); edit.direction = DVec3::X; edit.value = 2.;
        app.refresh_modeling_preview();
        let ctx = egui::Context::default();
        let preview = source(&mut app, &ctx);
        assert!(!preview.selected.is_empty());
        assert!(preview.selected.iter().all(|id| !original.contains_entity(id) && preview.model.contains_entity(id)));
        let copy_face = *preview.selected.iter().find(|id| preview.model.faces.contains_key(id)).unwrap();
        assert!((preview.model.face_center(&preview.model.faces[&copy_face]).unwrap().x - 3.).abs() < 1e-8);
        assert_eq!(app.selected, selected); assert_eq!(app.doc.model, original);
        app.history_preview = Some(original.clone());
        assert!(source(&mut app, &ctx).selected.is_empty());
        app.history_preview = None;
        let _ = ctx.run_test_ui(egui::RawInput { focused: true, ..Default::default() }, |_| {});
        let mac = ctx.os() == egui::os::OperatingSystem::Mac;
        app.move_modifier(&ctx, egui::Modifiers { ctrl: !mac, alt: mac, ..Default::default() }, false);
        app.refresh_modeling_preview();
        let moved = source(&mut app, &ctx);
        assert_eq!(moved.selected, selected);
        assert!(preview.selected.iter().all(|id| !moved.model.contains_entity(id)));
        app.modeling = None;
        assert_eq!(source(&mut app, &ctx).selected, selected);
        assert_eq!(app.doc.model, original); assert_eq!(app.selected, selected);
        assert!(app.doc.history.undo_label().is_none());
    }

    #[test]
    fn preview_selection_highlights_the_extruded_cap_in_both_modes_and_nested_contexts() {
        for scoped in [false, true] {
            for keep_start in [false, true] {
                let (mut app, face) = fixture(scoped);
                let original = app.doc.model.clone(); let selected = app.selected.clone();
                begin(&mut app, Tool::PushPull, face);
                let edit = app.modeling.as_mut().unwrap(); edit.keep_starting_face = keep_start; edit.value = 1.;
                app.refresh_modeling_preview();
                let preview = source(&mut app, &egui::Context::default());
                assert_eq!(preview.selected.len(), 1);
                let cap = *preview.selected.first().unwrap();
                assert_ne!(cap, face);
                let center = preview.model.face_center(&preview.model.faces[&cap]).unwrap();
                assert!((center.z - 1.).abs() < 1e-8, "scope {scoped}, keep {keep_start}: {center:?}");
                assert_eq!(app.selected, selected); assert_eq!(app.doc.model, original);
                assert!(app.doc.history.undo_label().is_none());
            }
        }
    }

    #[test]
    fn preview_selection_remaps_evaluated_caps_and_invalidates_the_cached_revision() {
        let (mut app, face) = fixture(false);
        begin(&mut app, Tool::PushPull, face);
        app.modeling.as_mut().unwrap().value = 1.; app.refresh_modeling_preview();
        let edit = app.modeling.as_ref().unwrap();
        let cap = *edit.preview_selection.as_ref().unwrap().roots.first().unwrap();
        let mut evaluated = edit.preview.as_ref().unwrap().clone();
        let mut cap_face = evaluated.faces.remove(&cap).unwrap();
        cap_face.id = cap + 1000; evaluated.faces.insert(cap_face.id, cap_face);
        assert_eq!(app.viewport_selection(&evaluated, 50), BTreeSet::from([cap + 1000]));
        assert_eq!(app.viewport_selection(&evaluated, 50), BTreeSet::from([cap + 1000]));
        let mut cap_face = evaluated.faces.remove(&(cap + 1000)).unwrap();
        cap_face.id = cap + 2000; evaluated.faces.insert(cap_face.id, cap_face);
        assert_eq!(app.viewport_selection(&evaluated, 51), BTreeSet::from([cap + 2000]));
    }

    #[test]
    fn preview_selection_discards_stale_cap_highlights_when_the_operation_fails() {
        let (mut app, face) = fixture(false);
        let original = app.doc.model.clone(); let selected = app.selected.clone();
        begin(&mut app, Tool::PushPull, face);
        app.modeling.as_mut().unwrap().value = 1.; app.refresh_modeling_preview();
        let ctx = egui::Context::default();
        assert_ne!(source(&mut app, &ctx).selected, selected);
        app.modeling.as_mut().unwrap().direction = DVec3::X;
        app.refresh_modeling_preview();
        let edit = app.modeling.as_ref().unwrap();
        assert!(edit.error.is_some()); assert!(edit.preview.is_none()); assert!(edit.preview_selection.is_none());
        assert_eq!(source(&mut app, &ctx).selected, selected);
        assert_eq!(app.doc.model, original); assert!(app.doc.history.undo_label().is_none());
    }
}
