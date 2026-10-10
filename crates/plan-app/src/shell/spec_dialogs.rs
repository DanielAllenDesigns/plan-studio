//! Hosts the specification dialogs of every object kind except a single wall
//! and openings (those keep their extras in `main.rs`; several walls open one
//! dialog here, W-83): stairs, cabinets,
//! symbols, roof planes, ceiling planes and dormers, framing members, slabs and other foundation objects,
//! corner trim, moldings, material regions, hatching, decks and 3D solids,
//! electrical devices, terrain, dimensions, text and CAD. [`SpecDialogs::open`] is the one place that maps an [`ObjectRef`] to
//! its dialog; OK applies the draft as one undo step.

use crate::dialogs::cabinet::CabinetDialog;
use crate::dialogs::cabinet_multi::CabinetMultiDialog;
use crate::dialogs::cad::CadDialog;
use crate::dialogs::details::DetailsDialog;
use crate::dialogs::dimension::{self, DimensionDialog};
use crate::dialogs::electrical::ElectricalDialog;
use crate::dialogs::foundation::FoundationDialog;
use crate::dialogs::framing::FramingMemberDialog;
use crate::dialogs::object_info::{self, InfoSession, SharedInfo};
use crate::dialogs::property_manager::{self, PropSession, SharedSession};
use crate::dialogs::roof::{CeilingDialog, DormerDialog, RoofPlaneDialog};
use crate::dialogs::stairs::StairDialog;
use crate::dialogs::symbol::SymbolDialog;
use crate::dialogs::terrain::{ObjectDialog, TerrainDialog};
use crate::dialogs::text::annot::AnnotDialog;
use crate::dialogs::text::TextDialog;
use crate::dialogs::WallDialog;
use crate::dialogs::{cad, text, Outcome};
use crate::editor::{
    details_view, foundation_view, framing_view, placed, roof_view, rooms_edit, schedule_view,
    site_view, stairs_view,
};
use crate::editor::{EditorContext, ObjectRef};
use crate::shell::view3d_panel::{Outbox, ViewRequest};
use eframe::egui;
use plan_core::Id;

enum Active {
    Stair(Box<StairDialog>),
    Cabinet(Box<CabinetDialog>),
    /// The Cabinet Specification over several selected cabinets (CB-631).
    Cabinets(Box<CabinetMultiDialog>),
    Symbol(Box<SymbolDialog>),
    RoofPlane(Box<RoofPlaneDialog>),
    /// A dormer: its id, the plane it stands on and the dialog.
    Dormer(Id, Id, Box<DormerDialog>),
    Foundation(Box<FoundationDialog>),
    Details(Box<DetailsDialog>),
    Ceiling(Box<CeilingDialog>),
    Framing(Box<FramingMemberDialog>),
    Device(Id, Box<ElectricalDialog>),
    Terrain(Box<TerrainDialog>),
    /// The specification of one terrain element (wall, curb, break, feature,
    /// road, landscape object, elevation line).
    TerrainObject(site_view::TerrainHit, Box<ObjectDialog>),
    Dimension(Box<DimensionDialog>),
    Text(Box<TextDialog>),
    /// A callout, marker or note (Callout, Marker or Note Specification).
    Annot(Box<AnnotDialog>),
    Cad(Box<CadDialog>),
    /// The Wall Specification over several selected walls (W-83).
    Walls(Box<WallDialog>),
}

/// The open specification dialog, if any (one at a time).
#[derive(Default)]
pub struct SpecDialogs {
    active: Option<Active>,
    /// The Properties tab of the open dialog (`property_manager`).
    props: Option<SharedSession>,
    /// The Properties tab of the wall or opening dialog `main.rs` hosts.
    main_props: Option<SharedSession>,
    /// The Components and Object Information tabs of the open dialog.
    info: Option<SharedInfo>,
    /// The same for the wall or opening dialog `main.rs` hosts.
    main_info: Option<SharedInfo>,
}

impl SpecDialogs {
    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }

    /// Arms the Properties tab for the wall or opening dialog that `main.rs`
    /// hosts (none when the object's kind has no custom properties).
    pub fn arm_main_props(&mut self, cx: &EditorContext, o: ObjectRef) {
        self.main_props = PropSession::for_object(cx, o);
        self.main_info = InfoSession::for_object(cx, o);
    }

    /// Test access to the shared panels (Components, Object Information,
    /// Label, Schedule, Manufacturer, Elevation) of the open dialog.
    #[cfg(test)]
    pub fn info_session(&self) -> Option<&SharedInfo> {
        self.info.as_ref()
    }

    /// The Materials List tabs armed for the hosted wall / opening dialog.
    pub fn main_info(&self) -> Option<&SharedInfo> {
        self.main_info.as_ref()
    }

    /// Takes the armed Materials List tabs (OK) or drops them (Cancel).
    pub fn take_main_info(&mut self) -> Option<SharedInfo> {
        self.main_info.take()
    }

    /// The armed session of the hosted wall / opening dialog.
    pub fn main_props(&self) -> Option<&SharedSession> {
        self.main_props.as_ref()
    }

    /// Takes the armed session (OK) or drops it (Cancel).
    pub fn take_main_props(&mut self) -> Option<SharedSession> {
        // A default dialog or a cancelled one has no Materials List tabs
        // either; OK takes them first with `take_main_info`.
        self.main_info = None;
        self.main_props.take()
    }

    /// The Properties session of the open dialog.
    #[cfg(test)]
    pub fn props_mut(&mut self) -> Option<&SharedSession> {
        self.props.as_ref()
    }

    /// Test access to the open multi-wall Wall Specification.
    #[cfg(test)]
    pub fn walls_dialog_mut(&mut self) -> Option<&mut WallDialog> {
        match self.active.as_mut()? {
            Active::Walls(d) => Some(d),
            _ => None,
        }
    }

    /// Test access to the open Dimension Specification's draft.
    #[cfg(test)]
    pub fn dimension_draft_mut(&mut self) -> Option<&mut plan_core::Dimension> {
        match self.active.as_mut()? {
            Active::Dimension(d) => Some(d.draft_mut()),
            _ => None,
        }
    }

    /// Test access to the open Text Specification's draft.
    #[cfg(test)]
    pub fn text_draft_mut(&mut self) -> Option<&mut plan_core::CadObject> {
        match self.active.as_mut()? {
            Active::Text(d) => Some(d.draft_mut()),
            _ => None,
        }
    }

    /// Test access to the open Callout, Marker or Note Specification.
    #[cfg(test)]
    pub fn annot_dialog_mut(&mut self) -> Option<&mut AnnotDialog> {
        match self.active.as_mut()? {
            Active::Annot(d) => Some(d),
            _ => None,
        }
    }

    /// Test access to the open CAD Specification's draft.
    #[cfg(test)]
    pub fn cad_draft_mut(&mut self) -> Option<&mut plan_core::CadObject> {
        match self.active.as_mut()? {
            Active::Cad(d) => Some(d.draft_mut()),
            _ => None,
        }
    }

    /// Test access to the open Staircase, Ramp or Landing Specification's draft.
    #[cfg(test)]
    pub fn stair_draft_mut(&mut self) -> Option<&mut stairs_view::StairObj> {
        match self.active.as_mut()? {
            Active::Stair(d) => Some(d.draft_mut()),
            _ => None,
        }
    }

    /// Test access to the open foundation object's draft.
    #[cfg(test)]
    pub fn foundation_draft_mut(&mut self) -> Option<&mut crate::dialogs::foundation::Draft> {
        match self.active.as_mut()? {
            Active::Foundation(d) => Some(d.draft_mut()),
            _ => None,
        }
    }

    /// Test access to the open detail object's draft.
    #[cfg(test)]
    pub fn details_draft_mut(&mut self) -> Option<&mut crate::dialogs::details::Draft> {
        match self.active.as_mut()? {
            Active::Details(d) => Some(d.draft_mut()),
            _ => None,
        }
    }

    /// Test access to the open terrain element's draft.
    #[cfg(test)]
    pub fn terrain_object_draft_mut(&mut self) -> Option<&mut site_view::TerrainObject> {
        match self.active.as_mut()? {
            Active::TerrainObject(_, d) => Some(d.draft_mut()),
            _ => None,
        }
    }

    /// Test access to the open Dormer Specification's dimensions.
    #[cfg(test)]
    pub fn dormer_spec_mut(&mut self) -> Option<&mut plan_roof::DormerSpec> {
        match self.active.as_mut()? {
            Active::Dormer(_, _, d) => Some(d.spec_mut()),
            _ => None,
        }
    }

    /// Opens one Wall Specification over the walls `ids` of the active floor
    /// (Open Object with several walls selected, W-83). Fields whose values
    /// differ show the mixed state; only the fields edited are written to all
    /// the walls, as one undo step. Returns false for fewer than two walls.
    pub fn open_walls(&mut self, cx: &mut EditorContext, ids: &[Id]) -> bool {
        let walls: Vec<_> = ids
            .iter()
            .filter_map(|id| cx.floor().wall(*id).cloned())
            .collect();
        if walls.len() < 2 {
            return false;
        }
        let heights = [
            cx.wall_height(plan_core::WallKind::Exterior),
            cx.wall_height(plan_core::WallKind::Interior),
        ];
        let retained: Vec<bool> = walls
            .iter()
            .map(|w| framing_view::wall_retained(&cx.project, w.id))
            .collect();
        let mut dialog = WallDialog::multi(walls, heights, cx.wall_types().to_vec());
        dialog.set_framing_retained(&retained);
        self.active = Some(Active::Walls(Box::new(dialog)));
        true
    }

    /// The cabinets of the active floor that are all there is in the
    /// selection, when it holds two or more cabinets and nothing else.
    pub fn selected_cabinets(cx: &EditorContext) -> Option<Vec<Id>> {
        let ids: Vec<Id> = cx
            .selection
            .items
            .iter()
            .map(|o| match o {
                ObjectRef::Cabinet(id) => Some(*id),
                _ => None,
            })
            .collect::<Option<_>>()?;
        (ids.len() >= 2).then_some(ids)
    }

    /// Test access to the open one-cabinet Cabinet Specification.
    #[cfg(test)]
    pub fn cabinet_dialog_mut(&mut self) -> Option<&mut CabinetDialog> {
        match self.active.as_mut()? {
            Active::Cabinet(d) => Some(d),
            _ => None,
        }
    }

    /// Test access to the open multi-cabinet Cabinet Specification.
    #[cfg(test)]
    pub fn cabinets_dialog_mut(&mut self) -> Option<&mut CabinetMultiDialog> {
        match self.active.as_mut()? {
            Active::Cabinets(d) => Some(d),
            _ => None,
        }
    }

    /// The walls of the active floor that are all there is in the selection,
    /// when it holds two or more walls and nothing else.
    pub fn selected_walls(cx: &EditorContext) -> Option<Vec<Id>> {
        let ids: Vec<Id> = cx
            .selection
            .items
            .iter()
            .map(|o| match o {
                ObjectRef::Wall(id) => Some(*id),
                _ => None,
            })
            .collect::<Option<_>>()?;
        (ids.len() >= 2).then_some(ids)
    }

    /// Opens the dialog of `o`. Returns false when the object has none (or
    /// no longer exists). Rooms and cameras are handed to the owners of
    /// their dialogs (`dialogs::build_tools`, the 3D panel).
    pub fn open(&mut self, cx: &mut EditorContext, o: ObjectRef) -> bool {
        // A Materials List Polyline is a CAD polyline with a specification of
        // its own (the Included Floors/Categories grid).
        if let ObjectRef::Cad(id) = o {
            if crate::dialogs::materials_list::open_polyline_spec(cx, id) {
                return true;
            }
        }
        // A fireplace symbol opens the Fireplace Specification: the Fireplace
        // tool hosts that dialog and goes back to Select Objects when it closes.
        if let ObjectRef::Symbol(id) = o {
            if crate::editor::fireplace_view::is_fireplace(cx.floor(), id) {
                crate::editor::fireplace_view::request_open(id);
                cx.requests.push(crate::editor::EditorRequest::SetTool(
                    crate::tools::ToolId::Fireplace,
                ));
                return true;
            }
        }
        let layer_names = |cx: &EditorContext| -> Vec<String> {
            cx.layers().layers.iter().map(|l| l.name.clone()).collect()
        };
        let dialog = match o {
            ObjectRef::Stair(id) => stairs_view::find(cx.floor(), id)
                .map(|s| Active::Stair(Box::new(StairDialog::new(s)))),
            ObjectRef::Cabinet(id) => match Self::selected_cabinets(cx).filter(|v| v.contains(&id))
            {
                // Open Object over several cabinets: one dialog, No Change
                // where the values differ (CB-631).
                Some(ids) => {
                    let cabs: Vec<_> = ids
                        .iter()
                        .filter_map(|i| placed::cabinet_by_id(cx.floor(), *i))
                        .collect();
                    if cabs.len() >= 2 {
                        Some(Active::Cabinets(Box::new(CabinetMultiDialog::new(cabs))))
                    } else {
                        placed::cabinet_by_id(cx.floor(), id)
                            .map(|c| Active::Cabinet(Box::new(CabinetDialog::new(c))))
                    }
                }
                None => placed::cabinet_by_id(cx.floor(), id)
                    .map(|c| Active::Cabinet(Box::new(CabinetDialog::new(c)))),
            },
            ObjectRef::Symbol(id) => cx.floor().symbol(id).cloned().map(|s| {
                let paint = crate::tools::materials::symbol_material(&cx.project, s.id);
                Active::Symbol(Box::new(
                    SymbolDialog::new(s, layer_names(cx)).with_material(paint),
                ))
            }),
            ObjectRef::RoofPlane(id) => {
                let set = roof_view::load(cx.floor());
                match (set.plane(id), set.dormer(id)) {
                    (Some(r), _) => Some(Active::RoofPlane(Box::new(RoofPlaneDialog::new(
                        r.clone(),
                        layer_names(cx),
                    )))),
                    (None, Some(d)) => Some(Active::Dormer(
                        d.id,
                        d.main,
                        Box::new(DormerDialog::new(d.spec)),
                    )),
                    (None, None) => set
                        .ceiling(id)
                        .cloned()
                        .map(|c| Active::Ceiling(Box::new(CeilingDialog::new(c, layer_names(cx))))),
                }
            }
            ObjectRef::Foundation(id) => {
                let layer = foundation_view::load(cx);
                layer.find(id).and_then(|r| {
                    FoundationDialog::new(&layer, r, layer_names(cx)).map(|d| {
                        let datums = crate::dialogs::foundation::datums_for(cx, &layer, r);
                        Active::Foundation(Box::new(d.with_datums(datums)))
                    })
                })
            }
            ObjectRef::Detail(id) => {
                let layer = details_view::load(cx);
                layer.find(id).and_then(|r| {
                    let mut names = layer_names(cx);
                    if let Some(own) = layer.layer_of(r).filter(|l| !names.contains(l)) {
                        names.push(own);
                    }
                    DetailsDialog::new(&layer, r, names).map(|d| Active::Details(Box::new(d)))
                })
            }
            ObjectRef::Framing(id) => match framing_view::find(cx.floor(), id) {
                Some(framing_view::Record::Manual(m)) => Some(Active::Framing(Box::new(
                    FramingMemberDialog::new(&m, layer_names(cx)),
                ))),
                Some(framing_view::Record::Built(m)) => Some(Active::Framing(Box::new(
                    FramingMemberDialog::new(&m, layer_names(cx)).for_built(),
                ))),
                // The Joist and Roof Truss Direction Lines have a Specification;
                // markers, Bearing Lines and Truss Bases have none.
                Some(r) => {
                    FramingMemberDialog::for_direction(&r).map(|d| Active::Framing(Box::new(d)))
                }
                None => None,
            },
            ObjectRef::Device(id) => {
                let layer = site_view::load_electrical(cx.floor());
                layer.device(id).map(|d| {
                    let dialog = ElectricalDialog::for_device(d, &layer)
                        .with_defaults(&plan_electrical::ElectricalDefaults::load(&cx.project));
                    Active::Device(id, Box::new(dialog))
                })
            }
            ObjectRef::Terrain => {
                let rec = site_view::load_terrain(&cx.project).unwrap_or_default();
                Some(Active::Terrain(Box::new(TerrainDialog::new(&rec))))
            }
            ObjectRef::TerrainObject(hit) => {
                let Some(rec) = site_view::load_terrain(&cx.project) else {
                    return false;
                };
                if !site_view::hit_exists(&rec.terrain, hit) {
                    return false;
                }
                // Points, regions and modifiers have no dialog of their own:
                // they open the Terrain Specification.
                match site_view::object_at(&rec.terrain, hit) {
                    Some(obj) => Some(Active::TerrainObject(hit, Box::new(ObjectDialog::new(obj)))),
                    None => Some(Active::Terrain(Box::new(TerrainDialog::new(&rec)))),
                }
            }
            ObjectRef::Dimension(_) => {
                dimension::open_for(cx, o).map(|d| Active::Dimension(Box::new(d)))
            }
            ObjectRef::Cad(_) | ObjectRef::Text(_) => text::annot::open_for(cx, o)
                .map(|d| Active::Annot(Box::new(d)))
                .or_else(|| text::open_for(cx, o).map(|d| Active::Text(Box::new(d))))
                .or_else(|| cad::open_for(cx, o).map(|d| Active::Cad(Box::new(d)))),
            ObjectRef::Room(idx) => {
                rooms_edit::request_room_dialog(cx, idx);
                return cx.rooms.get(idx).is_some();
            }
            ObjectRef::Camera(id) => {
                Outbox::global().post(ViewRequest::OpenCameraSpec(id));
                return cx.project.camera(id).is_some();
            }
            ObjectRef::Schedule(id) => {
                // The Schedule Specification window is hosted by build_tools.
                crate::dialogs::build_tools::open_schedule_spec(cx.floor, id);
                return schedule_view::exists(cx.floor(), id);
            }
            ObjectRef::Block(id) => {
                crate::dialogs::arch_block::open(cx, id);
                return cx.floor().blocks.get(id).is_some();
            }
            ObjectRef::Solid(id) => {
                crate::dialogs::solids::open_compound(cx, id);
                return cx.floor().solid_layer.compound(id).is_some();
            }
            ObjectRef::Wall(_) | ObjectRef::Opening(_) => None,
        };
        let opened = dialog.is_some();
        if opened {
            self.active = dialog;
            self.props = PropSession::for_object(cx, o);
            self.info = InfoSession::for_object(cx, o);
        }
        opened
    }

    /// Shows the open dialog and applies an OK.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        // A click of the Callout, Marker or Note tool posts its dialog.
        if self.active.is_none() {
            if let Some(d) = text::annot::take_posted() {
                self.active = Some(Active::Annot(Box::new(d)));
            }
        }
        let Some(mut a) = self.active.take() else {
            return;
        };
        let prev_info = object_info::set_current(self.info.clone());
        let outcome = property_manager::with_current(self.props.as_ref(), || match &mut a {
            Active::Stair(d) => d.show(ctx),
            Active::Cabinet(d) => d.show(ctx),
            Active::Cabinets(d) => d.show(ctx),
            Active::Symbol(d) => d.show(ctx),
            Active::RoofPlane(d) => d.show(ctx),
            Active::Dormer(_, _, d) => d.show(ctx),
            Active::Foundation(d) => d.show(ctx),
            Active::Details(d) => d.show(ctx),
            Active::Ceiling(d) => d.show(ctx),
            Active::Framing(d) => d.show(ctx),
            Active::Device(_, d) => d.show(ctx),
            Active::Terrain(d) => d.show(ctx),
            Active::TerrainObject(_, d) => d.show(ctx),
            Active::Dimension(d) => d.show(ctx),
            Active::Text(d) => d.show(ctx),
            Active::Annot(d) => d.show(ctx),
            Active::Cad(d) => d.show(ctx),
            Active::Walls(d) => d.show(ctx),
        });
        object_info::set_current(prev_info);
        match outcome {
            Outcome::Open => self.active = Some(a),
            Outcome::Cancel => {
                self.props = None;
                self.info = None;
            }
            Outcome::Ok => {
                // The dialog and its Properties and Materials List tabs are
                // one undo step.
                let depth = property_manager::before_apply(cx);
                // One step, and none when OK changed nothing (QA-26).
                cx.undo_group(|cx| apply(cx, &a));
                property_manager::after_apply(cx, self.props.as_ref(), depth);
                object_info::after_apply(cx, self.info.as_ref(), depth);
                self.props = None;
                self.info = None;
                cx.mark_dirty();
            }
        }
    }
}

fn apply(cx: &mut EditorContext, a: &Active) {
    match a {
        Active::Stair(d) => {
            stairs_view::apply_edit(cx, d.draft());
        }
        Active::Cabinet(d) => {
            placed::apply_cabinet(cx, d.draft());
        }
        Active::Cabinets(d) => {
            if d.apply(cx) == 0 {
                cx.status = "Cabinet Specification: nothing was changed".into();
            }
        }
        Active::Symbol(d) => {
            // The Materials tab's paint joins the Specification's undo step.
            if placed::apply_symbol(cx, d.draft()) {
                if let Some(choice) = d.material_choice() {
                    let id = d.draft().id;
                    if crate::tools::materials::apply_symbol_material(
                        &mut cx.project,
                        id,
                        choice.as_deref(),
                    ) {
                        cx.mark_dirty();
                    }
                }
            }
        }
        Active::RoofPlane(d) => {
            let fl = cx.floor;
            if roof_view::exists(&cx.project.floors[fl], d.draft().id) {
                cx.begin_change("Roof Plane Specification");
                roof_view::apply_plane_edit(&mut cx.project, fl, d.draft());
            }
        }
        Active::Dormer(id, main, d) => {
            let fl = cx.floor;
            cx.begin_change("Dormer Specification");
            if let Err(e) = roof_view::apply_dormer(&mut cx.project, fl, *main, Some(*id), d.spec())
            {
                cx.cancel_change();
                cx.status = format!("Dormer Specification: {e}");
            }
        }
        Active::Foundation(d) => {
            let draft = d.draft().clone();
            let mut found = false;
            foundation_view::edit(cx, draft.title(), |l| found = draft.apply(l));
            if !found {
                cx.cancel_change();
                cx.status = "The object is gone".into();
            }
        }
        Active::Details(d) => {
            let draft = d.draft().clone();
            let mut found = false;
            details_view::edit(cx, draft.title(), |l| found = draft.apply(l));
            if !found {
                cx.cancel_change();
                cx.status = "The object is gone".into();
            }
        }
        Active::Ceiling(d) => {
            let fl = cx.floor;
            cx.begin_change("Ceiling Plane Specification");
            if !roof_view::apply_ceiling_edit(&mut cx.project, fl, d.draft()) {
                cx.cancel_change();
                cx.status = "The ceiling plane is gone".into();
            }
        }
        Active::Framing(d) => {
            d.apply(cx);
        }
        Active::Device(id, d) => {
            let draft = d.draft().clone();
            site_view::edit_electrical(cx, "Electrical Service Specification", |layer, floor| {
                if let Some(dev) = layer.device_mut(*id) {
                    draft.apply(dev);
                }
                // The Switches tab: connected lights and 3-way pairs.
                draft.apply_to_layer(layer, &floor.walls);
            });
            // "Use as default height" and the Default Heights list (same undo step).
            if draft.store_defaults(&mut cx.project) {
                cx.mark_dirty();
            }
        }
        Active::Terrain(d) => {
            let draft = d.draft().clone();
            site_view::edit_terrain(cx, "Terrain Specification", |rec| {
                rec.apply_spec(&draft);
            });
        }
        Active::TerrainObject(hit, d) => {
            let draft = d.draft().clone();
            let mut found = false;
            site_view::edit_terrain(cx, draft.title(), |rec| {
                found = site_view::replace_object(&mut rec.terrain, *hit, draft.clone());
            });
            if !found {
                cx.cancel_change();
                cx.status = "The object is gone".into();
            }
        }
        Active::Dimension(d) => {
            d.apply(cx);
        }
        Active::Text(d) => {
            d.apply(cx);
        }
        Active::Annot(d) => {
            d.apply(cx);
        }
        Active::Cad(d) => {
            d.apply(cx);
        }
        Active::Walls(d) => {
            cx.begin_change("Wall Specification");
            let fl = cx.floor;
            let mut changed = d.apply_multi(&mut cx.project, fl);
            if let (Some(v), Some(ids)) = (d.retain_framing_change(), d.multi_ids()) {
                changed += framing_view::retain_walls_in(&mut cx.project, ids, v);
            }
            if changed == 0 {
                cx.cancel_change();
                cx.status = "Wall Specification: nothing was changed".into();
            } else {
                cx.project.sync_platform_walls();
                cx.refresh();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::foundation::{rect_outline, DATA_LAYER};
    use plan_core::geometry::Point;

    #[test]
    fn a_terrain_element_opens_its_own_dialog_and_ok_is_one_undo_step() {
        use crate::editor::site_view::TerrainHit;
        use plan_terrain::{ElevationPoint, Landscape, LandscapeKind, ShapeKind};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        site_view::edit_terrain(&mut cx, "Draw", |r| {
            r.terrain.landscape.push(Landscape::new(
                LandscapeKind::GardenBed,
                ShapeKind::Polyline,
                vec![
                    Point::new(0.0, 0.0),
                    Point::new(120.0, 0.0),
                    Point::new(120.0, 120.0),
                ],
            ));
            r.terrain.elevation_points.push(ElevationPoint {
                pos: Point::new(300.0, 300.0),
                z: 12.0,
            });
        });
        let mut dialogs = SpecDialogs::default();
        let bed = ObjectRef::TerrainObject(TerrainHit::Landscape(0));
        assert!(dialogs.open(&mut cx, bed));
        assert!(matches!(
            dialogs.active,
            Some(Active::TerrainObject(TerrainHit::Landscape(0), _))
        ));
        if let Some(Active::TerrainObject(_, d)) = dialogs.active.as_mut() {
            match d.draft_mut() {
                site_view::TerrainObject::Landscape(l) => l.height = 7.0,
                other => panic!("{other:?}"),
            }
        }
        let a = dialogs.active.take().unwrap();
        let depth = cx.undo_label().map(str::to_string);
        apply(&mut cx, &a);
        assert_eq!(cx.undo_label(), Some("Garden Bed Specification"));
        let stored = site_view::load_terrain(&cx.project).unwrap();
        assert_eq!(stored.terrain.landscape[0].height, 7.0);
        cx.undo();
        assert_ne!(
            site_view::load_terrain(&cx.project)
                .unwrap()
                .terrain
                .landscape[0]
                .height,
            7.0
        );
        assert_eq!(cx.undo_label().map(str::to_string), depth);

        // An elevation point has a dialog of its own (round 14); the whole
        // terrain opens the Terrain Specification. A vanished element opens
        // nothing.
        let mut dialogs = SpecDialogs::default();
        assert!(dialogs.open(&mut cx, ObjectRef::TerrainObject(TerrainHit::Point(0))));
        assert!(matches!(
            dialogs.active,
            Some(Active::TerrainObject(TerrainHit::Point(0), _))
        ));
        let mut dialogs = SpecDialogs::default();
        assert!(dialogs.open(&mut cx, ObjectRef::Terrain));
        assert!(matches!(dialogs.active, Some(Active::Terrain(_))));
        let mut dialogs = SpecDialogs::default();
        assert!(!dialogs.open(&mut cx, ObjectRef::TerrainObject(TerrainHit::Wall(4))));
        assert!(!dialogs.is_open());
    }

    #[test]
    fn foundation_objects_and_dormers_open_their_dialogs() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut dialogs = SpecDialogs::default();
        let slab = foundation_view::add_slab(
            &mut cx,
            rect_outline(Point::ZERO, Point::new(240.0, 180.0)),
            false,
        );
        assert!(dialogs.open(&mut cx, ObjectRef::Foundation(slab)));
        assert!(dialogs.is_open());
        assert!(!dialogs.open(&mut cx, ObjectRef::Foundation(slab + 99)));
        // OK applies the draft as one undo step.
        if let Some(Active::Foundation(d)) = dialogs.active.as_mut() {
            if let crate::dialogs::foundation::Draft::Slab(s) = d.draft_mut() {
                s.thickness = 6.0;
            }
        }
        let a = dialogs.active.take().unwrap();
        apply(&mut cx, &a);
        assert_eq!(
            foundation_view::load(&cx).slab(slab).unwrap().thickness,
            6.0
        );
        assert_eq!(cx.undo_label(), Some("Slab Specification"));
        assert!(cx.project.layers.get(DATA_LAYER).is_none());

        // A dormer on a manual plane.
        let fl = cx.floor;
        let (base, poly) = roof_view::manual_plane_geometry(
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(120.0, 120.0),
            100.0,
            8.0,
        )
        .unwrap();
        let mut set = roof_view::load(&cx.project.floors[fl]);
        let pid = cx.project.alloc_id();
        set.planes
            .push(roof_view::RoofPlaneRecord::new(pid, poly, 8.0, base));
        roof_view::store(&mut cx.project, fl, &mut set);
        let spec = plan_roof::DormerSpec {
            position_along_eave: 120.0,
            setback_from_eave: 40.0,
            ..Default::default()
        };
        let did = roof_view::apply_dormer(&mut cx.project, fl, pid, None, spec).unwrap();
        assert!(dialogs.open(&mut cx, ObjectRef::RoofPlane(did)));
        assert!(matches!(dialogs.active, Some(Active::Dormer(..))));
        assert!(dialogs.open(&mut cx, ObjectRef::RoofPlane(pid)));
        assert!(matches!(dialogs.active, Some(Active::RoofPlane(_))));
    }

    #[test]
    fn details_open_the_details_dialog_and_ok_applies_with_undo() {
        use plan_core::details::MoldingProfile;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut dialogs = SpecDialogs::default();
        let deck = details_view::add_deck(
            &mut cx,
            vec![
                Point::new(0.0, 0.0),
                Point::new(96.0, 0.0),
                Point::new(96.0, 96.0),
            ],
        );
        let mold = details_view::add_molding(
            &mut cx,
            vec![Point::new(0.0, 200.0), Point::new(80.0, 200.0)],
            MoldingProfile::Crown,
        );
        for id in [deck, mold] {
            assert!(dialogs.open(&mut cx, ObjectRef::Detail(id)), "{id}");
            assert!(matches!(dialogs.active, Some(Active::Details(_))));
        }
        assert!(!dialogs.open(&mut cx, ObjectRef::Detail(deck + 999)));
        assert!(dialogs.open(&mut cx, ObjectRef::Detail(deck)));
        if let Some(Active::Details(d)) = dialogs.active.as_mut() {
            if let crate::dialogs::details::Draft::Deck(x) = d.draft_mut() {
                x.elevation = 30.0;
                x.railing = true;
            }
        }
        let a = dialogs.active.take().unwrap();
        apply(&mut cx, &a);
        let d = details_view::load(&cx).deck(deck).unwrap().clone();
        assert_eq!((d.elevation, d.railing), (30.0, true));
        assert_eq!(cx.undo_label(), Some("Deck Specification"));
        cx.undo();
        assert_eq!(details_view::load(&cx).deck(deck).unwrap().elevation, 0.0);
    }

    #[test]
    fn framing_members_and_ceiling_planes_open_their_dialogs() {
        use crate::editor::framing_view::Record;
        use plan_framing::{FramingMember, ManualMemberKind, ReferenceMarker};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut dialogs = SpecDialogs::default();
        let m = framing_view::new_member(
            cx.floor(),
            ManualMemberKind::Joist,
            Point::new(0.0, 0.0),
            Point::new(192.0, 0.0),
        );
        let id = framing_view::add_record(&mut cx, "Place Joist", |id| {
            Record::Manual(FramingMember { id, ..m })
        });
        let marker = framing_view::add_record(&mut cx, "Place Marker", |id| Record::Marker {
            id,
            marker: ReferenceMarker {
                point: Point::new(10.0, 10.0),
                angle: 0.0,
            },
        });
        assert!(dialogs.open(&mut cx, ObjectRef::Framing(id)));
        assert!(matches!(dialogs.active, Some(Active::Framing(_))));
        // Layout markers have no specification; neither do missing records.
        assert!(!dialogs.open(&mut cx, ObjectRef::Framing(marker)));
        assert!(!dialogs.open(&mut cx, ObjectRef::Framing(id + 500)));
        if let Some(Active::Framing(d)) = dialogs.active.as_mut() {
            d.set_length(240.0);
        }
        let a = dialogs.active.take().unwrap();
        apply(&mut cx, &a);
        let Some(Record::Manual(m)) = framing_view::find(cx.floor(), id) else {
            panic!("the member is gone");
        };
        assert!((m.plan_length() - 240.0).abs() < 1e-6);
        assert_eq!(cx.undo_label(), Some("Framing Member Specification"));

        // A ceiling plane: General, Line Style and Layer.
        let fl = cx.floor;
        let cid = roof_view::add_ceiling(
            &mut cx.project,
            fl,
            (
                Point::new(0.0, 0.0),
                Point::new(240.0, 0.0),
                Point::new(120.0, 90.0),
            ),
            100.0,
            4.0,
        )
        .unwrap();
        assert!(dialogs.open(&mut cx, ObjectRef::RoofPlane(cid)));
        assert!(matches!(dialogs.active, Some(Active::Ceiling(_))));
        if let Some(Active::Ceiling(d)) = dialogs.active.as_mut() {
            let c = d.draft_mut();
            c.pitch = 6.0;
            c.height_at_baseline = 108.0;
            c.thickness = 7.0;
            c.line_style = plan_core::LineStyle::Solid;
        }
        let a = dialogs.active.take().unwrap();
        apply(&mut cx, &a);
        let c = roof_view::load(cx.floor()).ceilings[0].clone();
        assert_eq!(
            (c.pitch, c.height_at_baseline, c.thickness, c.line_style),
            (6.0, 108.0, 7.0, plan_core::LineStyle::Solid)
        );
        assert_eq!(cx.undo_label(), Some("Ceiling Plane Specification"));
    }

    #[test]
    fn the_electrical_dialog_stores_default_heights_in_the_same_undo_step() {
        use plan_electrical::{place_free, DeviceKind, ElectricalDefaults};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut layer = site_view::load_electrical(cx.floor());
        let id = layer.add(place_free(DeviceKind::Switch, Point::new(40.0, 40.0)));
        site_view::save_electrical(&mut cx.project, 0, &layer);
        let before = cx.undo_label().map(str::to_string);
        let mut dialogs = SpecDialogs::default();
        assert!(dialogs.open(&mut cx, ObjectRef::Device(id)));
        let Some(Active::Device(_, d)) = dialogs.active.as_mut() else {
            panic!("the Electrical Service Specification did not open");
        };
        // Opened with the plan's defaults, so the tab can offer them.
        let defaults = d.draft().defaults.clone().expect("opened with defaults");
        assert_eq!(defaults, ElectricalDefaults::load(&cx.project));
        let mut edited = defaults;
        edited.set_height(DeviceKind::Switch, 52.0);
        d.draft_mut().defaults = Some(edited);
        d.draft_mut().label = "Hall".into();
        let a = dialogs.active.take().unwrap();
        apply(&mut cx, &a);
        assert_eq!(cx.undo_label(), Some("Electrical Service Specification"));
        assert_eq!(
            ElectricalDefaults::load(&cx.project).height(DeviceKind::Switch),
            52.0
        );
        let stored = site_view::load_electrical(cx.floor());
        assert_eq!(stored.device(id).unwrap().label, "Hall");
        // One undo step takes back the device edit and the default together.
        cx.undo();
        assert_eq!(
            ElectricalDefaults::load(&cx.project).height(DeviceKind::Switch),
            DeviceKind::Switch.default_height()
        );
        assert_ne!(
            site_view::load_electrical(cx.floor())
                .device(id)
                .unwrap()
                .label,
            "Hall"
        );
        assert_eq!(cx.undo_label().map(str::to_string), before);
    }

    #[test]
    fn the_symbol_dialog_paints_the_symbol_in_the_same_undo_step() {
        use plan_core::PlacedSymbol;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_symbol(
            0,
            PlacedSymbol::new("nope", Point::new(10.0, 10.0), 20.0, 20.0, 20.0),
        );
        let mut dialogs = SpecDialogs::default();
        assert!(dialogs.open(&mut cx, ObjectRef::Symbol(id)));
        let Some(Active::Symbol(d)) = dialogs.active.as_mut() else {
            panic!("the Symbol Specification did not open");
        };
        assert_eq!(d.material_choice(), None, "nothing chosen yet");
        d.choose_material(Some("Drywall".into()));
        let a = dialogs.active.take().unwrap();
        apply(&mut cx, &a);
        assert_eq!(cx.undo_label(), Some("Symbol Specification"));
        assert_eq!(
            crate::tools::materials::symbol_material(&cx.project, id).as_deref(),
            Some("Drywall")
        );
        // Reopening shows the paint; putting the look back clears it.
        let mut dialogs = SpecDialogs::default();
        assert!(dialogs.open(&mut cx, ObjectRef::Symbol(id)));
        let Some(Active::Symbol(d)) = dialogs.active.as_mut() else {
            panic!("the Symbol Specification did not open");
        };
        d.choose_material(None);
        let a = dialogs.active.take().unwrap();
        apply(&mut cx, &a);
        assert_eq!(
            crate::tools::materials::symbol_material(&cx.project, id),
            None
        );
        cx.undo();
        assert_eq!(
            crate::tools::materials::symbol_material(&cx.project, id).as_deref(),
            Some("Drywall")
        );
        cx.undo();
        assert_eq!(
            crate::tools::materials::symbol_material(&cx.project, id),
            None
        );
    }
}
