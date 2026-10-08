//! Hosts the specification dialogs of every object kind except walls and
//! openings (those two keep their extras in `main.rs`): stairs, cabinets,
//! symbols, roof planes, electrical devices, terrain, dimensions, text and
//! CAD. [`SpecDialogs::open`] is the one place that maps an [`ObjectRef`] to
//! its dialog; OK applies the draft as one undo step.

use crate::dialogs::cabinet::CabinetDialog;
use crate::dialogs::cad::CadDialog;
use crate::dialogs::dimension::{self, DimensionDialog};
use crate::dialogs::electrical::ElectricalDialog;
use crate::dialogs::roof::RoofPlaneDialog;
use crate::dialogs::stairs::StairDialog;
use crate::dialogs::symbol::SymbolDialog;
use crate::dialogs::terrain::TerrainDialog;
use crate::dialogs::text::TextDialog;
use crate::dialogs::{cad, text, Outcome};
use crate::editor::{placed, roof_view, rooms_edit, site_view, stairs_view};
use crate::editor::{EditorContext, ObjectRef};
use crate::shell::view3d_panel::{Outbox, ViewRequest};
use eframe::egui;
use plan_core::Id;

enum Active {
    Stair(Box<StairDialog>),
    Cabinet(Box<CabinetDialog>),
    Symbol(Box<SymbolDialog>),
    RoofPlane(Box<RoofPlaneDialog>),
    Device(Id, Box<ElectricalDialog>),
    Terrain(Box<TerrainDialog>),
    Dimension(Box<DimensionDialog>),
    Text(Box<TextDialog>),
    Cad(Box<CadDialog>),
}

/// The open specification dialog, if any (one at a time).
#[derive(Default)]
pub struct SpecDialogs {
    active: Option<Active>,
}

impl SpecDialogs {
    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }

    /// Opens the dialog of `o`. Returns false when the object has none (or
    /// no longer exists). Rooms and cameras are handed to the owners of
    /// their dialogs (`dialogs::build_tools`, the 3D panel).
    pub fn open(&mut self, cx: &mut EditorContext, o: ObjectRef) -> bool {
        let layer_names = |cx: &EditorContext| -> Vec<String> {
            cx.layers().layers.iter().map(|l| l.name.clone()).collect()
        };
        let dialog = match o {
            ObjectRef::Stair(id) => stairs_view::find(cx.floor(), id)
                .map(|s| Active::Stair(Box::new(StairDialog::new(s)))),
            ObjectRef::Cabinet(id) => placed::cabinet_by_id(cx.floor(), id)
                .map(|c| Active::Cabinet(Box::new(CabinetDialog::new(c)))),
            ObjectRef::Symbol(id) => cx
                .floor()
                .symbol(id)
                .cloned()
                .map(|s| Active::Symbol(Box::new(SymbolDialog::new(s, layer_names(cx))))),
            ObjectRef::RoofPlane(id) => roof_view::load(cx.floor())
                .plane(id)
                .cloned()
                .map(|r| Active::RoofPlane(Box::new(RoofPlaneDialog::new(r, layer_names(cx))))),
            ObjectRef::Device(id) => {
                let layer = site_view::load_electrical(cx.floor());
                layer
                    .device(id)
                    .map(|d| Active::Device(id, Box::new(ElectricalDialog::for_device(d, &layer))))
            }
            ObjectRef::Terrain => {
                let rec = site_view::load_terrain(&cx.project).unwrap_or_default();
                Some(Active::Terrain(Box::new(TerrainDialog::new(&rec))))
            }
            ObjectRef::Dimension(_) => {
                dimension::open_for(cx, o).map(|d| Active::Dimension(Box::new(d)))
            }
            ObjectRef::Cad(_) | ObjectRef::Text(_) => text::open_for(cx, o)
                .map(|d| Active::Text(Box::new(d)))
                .or_else(|| cad::open_for(cx, o).map(|d| Active::Cad(Box::new(d)))),
            ObjectRef::Room(idx) => {
                rooms_edit::request_room_dialog(cx, idx);
                return cx.rooms.get(idx).is_some();
            }
            ObjectRef::Camera(id) => {
                Outbox::global().post(ViewRequest::OpenCameraSpec(id));
                return cx.project.camera(id).is_some();
            }
            ObjectRef::Wall(_) | ObjectRef::Opening(_) => None,
        };
        let opened = dialog.is_some();
        if opened {
            self.active = dialog;
        }
        opened
    }

    /// Shows the open dialog and applies an OK.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        let Some(mut a) = self.active.take() else {
            return;
        };
        let outcome = match &mut a {
            Active::Stair(d) => d.show(ctx),
            Active::Cabinet(d) => d.show(ctx),
            Active::Symbol(d) => d.show(ctx),
            Active::RoofPlane(d) => d.show(ctx),
            Active::Device(_, d) => d.show(ctx),
            Active::Terrain(d) => d.show(ctx),
            Active::Dimension(d) => d.show(ctx),
            Active::Text(d) => d.show(ctx),
            Active::Cad(d) => d.show(ctx),
        };
        match outcome {
            Outcome::Open => self.active = Some(a),
            Outcome::Cancel => {}
            Outcome::Ok => {
                apply(cx, &a);
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
        Active::Symbol(d) => {
            placed::apply_symbol(cx, d.draft());
        }
        Active::RoofPlane(d) => {
            let (fl, id) = (cx.floor, d.draft().id);
            let mut set = roof_view::load(&cx.project.floors[fl]);
            if set.plane(id).is_some() {
                cx.begin_change("Roof Plane Specification");
                if let Some(old) = set.plane_mut(id) {
                    roof_view::apply_edits(old, d.draft());
                }
                roof_view::store(&mut cx.project, fl, &mut set);
            }
        }
        Active::Device(id, d) => {
            let draft = d.draft().clone();
            site_view::edit_electrical(cx, "Electrical Service Specification", |layer, _| {
                if let Some(dev) = layer.device_mut(*id) {
                    draft.apply(dev);
                }
            });
        }
        Active::Terrain(d) => {
            let draft = d.draft().clone();
            site_view::edit_terrain(cx, "Terrain Specification", |rec| {
                rec.contour_interval = draft.contour_interval;
                rec.layer = draft.layer.clone();
                let t = &mut rec.terrain;
                t.subfloor_height_above_terrain = draft.terrain.subfloor_height_above_terrain;
                t.building_pad_elevation = draft.terrain.building_pad_elevation;
                t.smoothing = draft.terrain.smoothing;
                t.grid_spacing = draft.terrain.grid_spacing;
            });
        }
        Active::Dimension(d) => {
            d.apply(cx);
        }
        Active::Text(d) => {
            d.apply(cx);
        }
        Active::Cad(d) => {
            d.apply(cx);
        }
    }
}
