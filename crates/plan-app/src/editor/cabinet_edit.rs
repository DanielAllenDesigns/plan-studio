//! Edit toolbar commands of a selected cabinet (reference manual p. 660):
//! Open Cabinet Doors/Drawers, Close Cabinet Doors/Drawers, Generate Custom
//! Countertop and Make Cabinet Molding Polyline.
//!
//! Open and Close set the cabinet's Show Open options (doors, drawers and
//! rollouts together) on every selected cabinet; each command is one undo
//! step like any Edit command. Make Cabinet Molding Polyline takes a
//! cabinet's molding off the cabinet and draws its outline as a closed CAD
//! polyline on a layer of its own (CB-486).

use super::actions::{EditAction, EditActionKind};
use super::placed::{self, cabinet_by_id, replace_cabinet};
use super::{EditorContext, ObjectRef};
use plan_cabinets::{Cabinet, ShowOpen};
use plan_core::cad::CadItem;
use plan_core::layers::Layer;

pub const OPEN_DOORS: &str = "cabinet.open_doors";
pub const CLOSE_DOORS: &str = "cabinet.close_doors";
pub const MOLDING_POLYLINE: &str = "cabinet.molding_polyline";

/// The layer a cabinet molding polyline lands on.
pub const MOLDING_LAYER: &str = "Cabinets, Moldings";

fn selected_cabinets(cx: &EditorContext) -> Vec<Cabinet> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cabinet(id) => cabinet_by_id(cx.floor(), *id),
            _ => None,
        })
        .collect()
}

/// The commands the Edit toolbar offers for the selection.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let cabs = selected_cabinets(cx);
    if cabs.is_empty() {
        return Vec::new();
    }
    let custom = |id, label, enabled| EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled,
    };
    let any_open = cabs.iter().any(|c| c.show_open.any());
    let all_open = cabs.iter().all(|c| c.show_open == ShowOpen::all());
    let mut v = vec![
        custom(OPEN_DOORS, "Open Cabinet Doors/Drawers", !all_open),
        custom(CLOSE_DOORS, "Close Cabinet Doors/Drawers", any_open),
        custom(
            placed::GENERATE_COUNTERTOP,
            "Generate Custom Countertop",
            cabs.iter().any(|c| c.countertop.is_some()),
        ),
    ];
    if cabs.iter().any(|c| !c.moldings.is_empty()) {
        v.push(custom(
            MOLDING_POLYLINE,
            "Make Cabinet Molding Polyline",
            true,
        ));
    }
    v
}

/// Runs one of this module's commands. Returns whether `id` was one.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        OPEN_DOORS => {
            set_show_open(cx, ShowOpen::all(), "Open Cabinet Doors/Drawers");
            true
        }
        CLOSE_DOORS => {
            set_show_open(cx, ShowOpen::default(), "Close Cabinet Doors/Drawers");
            true
        }
        MOLDING_POLYLINE => {
            make_molding_polylines(cx);
            true
        }
        _ => false,
    }
}

/// Sets Show Open on the selected cabinets; returns how many changed.
pub fn set_show_open(cx: &mut EditorContext, to: ShowOpen, label: &str) -> usize {
    let todo: Vec<Cabinet> = selected_cabinets(cx)
        .into_iter()
        .filter(|c| c.show_open != to)
        .collect();
    if todo.is_empty() {
        cx.status = format!("{label}: nothing to change");
        return 0;
    }
    cx.begin_change(label);
    let fl = cx.floor;
    let mut n = 0;
    for mut c in todo {
        c.show_open = to;
        if replace_cabinet(&mut cx.project, fl, &c) {
            n += 1;
        }
    }
    cx.mark_dirty();
    n
}

/// Make Cabinet Molding Polyline: every molding of the selected cabinets
/// becomes a closed polyline of its own on [`MOLDING_LAYER`]. Returns how
/// many polylines were made.
pub fn make_molding_polylines(cx: &mut EditorContext) -> usize {
    let mut rings: Vec<(Cabinet, Vec<Vec<plan_core::geometry::Point>>)> = Vec::new();
    for c in selected_cabinets(cx) {
        let paths: Vec<_> = (0..c.moldings.len())
            .filter_map(|i| c.molding_path(i))
            .collect();
        if !paths.is_empty() {
            rings.push((c, paths));
        }
    }
    if rings.is_empty() {
        cx.status = "The selected cabinets have no moldings".into();
        return 0;
    }
    cx.begin_change("Make Cabinet Molding Polyline");
    if cx.project.layers.get(MOLDING_LAYER).is_none() {
        let color = cx
            .project
            .layers
            .get("Cabinets, Wall")
            .map_or([0, 0, 0], |l| l.color);
        cx.project.layers.add(Layer::new(MOLDING_LAYER, color, 18));
    }
    let fl = cx.floor;
    let mut made = 0;
    for (mut cab, paths) in rings {
        for pts in paths {
            cx.project.add_cad(
                fl,
                MOLDING_LAYER,
                CadItem::Polyline {
                    points: pts,
                    closed: true,
                },
            );
            made += 1;
        }
        while cab.take_molding(0).is_some() {}
        replace_cabinet(&mut cx.project, fl, &cab);
    }
    cx.mark_dirty();
    made
}

/// Brings the program-made parts of the cabinets up to date after an edit:
/// the blind ends of cabinets that meet in a corner and which ends of each
/// cabinet are exposed or mated (overhangs, closed toe, feet on the sides).
/// Part of the undo step the caller has begun; returns how many cabinets
/// were rewritten.
pub fn sync_special(cx: &mut EditorContext) -> usize {
    let fl = cx.floor;
    let mut cabs = placed::load_cabinets(cx.floor());
    if cabs.is_empty() {
        return 0;
    }
    let walls = crate::tools::cabinet::wall_polys(cx);
    let before = cabs.clone();
    plan_cabinets::apply_blind_corners(&mut cabs);
    plan_cabinets::apply_exposures(&mut cabs, &walls);
    let mut n = 0;
    for (a, b) in before.iter().zip(&cabs) {
        if a != b && replace_cabinet(&mut cx.project, fl, b) {
            n += 1;
        }
    }
    if n > 0 {
        cx.mark_dirty();
    }
    n
}
