//! The Edit toolbar of a selected room and of the Exterior Room (R-107,
//! R-110; manual pp. 452, 455 and 463 to 464): Turn Ceiling Off/On, Make Room
//! Polyline, Make Standard Area Polyline, Make Living Area Polyline, Expand
//! Room Polyline, Create Schedule from Room, Create Room Elevation Views and
//! Auto Room Dimensions. Calculate Materials in Room comes from the Materials
//! List (`dialogs::materials_list::edit_buttons`).
//!
//! A polyline made from a room is a static CAD polyline on the current CAD
//! layer, not linked to the room afterwards. Each command is one undo step.

use super::exterior::{selected_exterior, structures};
use super::*;
use crate::editor::{EditAction, EditActionKind};
use plan_core::dimension::{AutoGroup, Dimension, DimensionKind};
use plan_core::Id;

/// Command ids of the room Edit toolbar.
pub mod id {
    pub const OPEN: &str = "room.open";
    pub const CEILING_OFF: &str = "room.ceiling_off";
    pub const CEILING_ON: &str = "room.ceiling_on";
    pub const ROOM_POLYLINE: &str = "room.polyline";
    pub const STANDARD_POLYLINE: &str = "room.standard_polyline";
    pub const LIVING_POLYLINE: &str = "room.living_polyline";
    pub const EXPAND: &str = "room.expand";
    pub const SCHEDULE: &str = "room.schedule";
    pub const ELEVATIONS: &str = "room.elevations";
    pub const DIMENSIONS: &str = "room.dimensions";
    pub const EXTERIOR_OPEN: &str = "room.exterior_open";
}

fn button(id: &'static str, label: &'static str, enabled: bool) -> EditAction {
    EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled,
    }
}

/// An expanded room (Expand Room Polyline) the polyline commands work on
/// instead of the selected room: remembered with the floor it belongs to.
#[derive(Clone)]
pub struct Expanded {
    pub floor: usize,
    pub room: Room,
}

/// The room the polyline commands use: the expanded one, else the selected.
fn target_room(cx: &EditorContext) -> Option<Room> {
    if let Some(e) = with(|s| s.expanded.clone()) {
        if e.floor == cx.floor {
            return Some(e.room);
        }
    }
    selected_room(cx).and_then(|i| cx.rooms.get(i).cloned())
}

/// The temporarily enlarged room, when Expand Room Polyline made one.
pub fn expanded_room() -> Option<Expanded> {
    with(|s| s.expanded.clone())
}

/// The buttons of the Edit toolbar for the selected room or Exterior Room.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    if let Some(r) = selected_room(cx) {
        let flat = cx
            .rooms
            .get(r)
            .and_then(|room| name_entry(cx, room))
            .is_none_or(|n| n.flat_ceiling);
        v.push(button(id::OPEN, "Open Object", true));
        v.extend(crate::dialogs::materials_list::edit_buttons(cx));
        v.push(if flat {
            button(id::CEILING_OFF, "Turn Ceiling Off", true)
        } else {
            button(id::CEILING_ON, "Turn Ceiling On", true)
        });
        v.push(button(id::ROOM_POLYLINE, "Make Room Polyline", true));
        v.push(button(
            id::STANDARD_POLYLINE,
            "Make Standard Area Polyline",
            true,
        ));
        let can_expand = cx.rooms.get(r).is_some_and(|room| {
            plan_core::living::expand_room(&cx.floor().walls, 0.5, room_anchor(room)).is_some()
        });
        v.push(button(id::EXPAND, "Expand Room Polyline", can_expand));
        v.push(button(id::SCHEDULE, "Create Schedule from Room", true));
        v.push(button(id::ELEVATIONS, "Create Room Elevation Views", true));
        v.push(button(id::DIMENSIONS, "Auto Room Dimensions", true));
    } else if selected_exterior(cx).is_some() {
        v.push(button(id::EXTERIOR_OPEN, "Open Object", true));
        v.push(button(id::ROOM_POLYLINE, "Make Room Polyline", true));
        v.push(button(
            id::LIVING_POLYLINE,
            "Make Living Area Polyline",
            true,
        ));
    }
    v
}

/// Runs a room command; false when `cmd` is not one of them.
pub fn run_command(cx: &mut EditorContext, cmd: &str) -> bool {
    match cmd {
        id::OPEN => {
            if let Some(r) = selected_room(cx) {
                request_room_dialog(cx, r);
            }
        }
        id::EXTERIOR_OPEN => {
            if let Some(i) = selected_exterior(cx) {
                request_exterior_dialog(cx, i);
            }
        }
        id::CEILING_OFF | id::CEILING_ON => {
            if let Some(r) = selected_room(cx) {
                crate::tools::tray_ceiling::set_flat_ceiling(cx, r, cmd == id::CEILING_ON);
            }
        }
        id::ROOM_POLYLINE => {
            if selected_room(cx).is_some() {
                make_room_polyline(cx);
            } else {
                make_exterior_polyline(cx);
            }
        }
        id::STANDARD_POLYLINE => {
            make_standard_area_polyline(cx);
        }
        id::LIVING_POLYLINE => {
            make_living_area_polyline(cx);
        }
        id::EXPAND => {
            expand_room_polyline(cx);
        }
        id::SCHEDULE => {
            if let Some(r) = selected_room(cx) {
                crate::dialogs::schedule_spec::ask_schedule_type(r);
            }
        }
        id::ELEVATIONS => {
            create_room_elevation_views(cx);
        }
        id::DIMENSIONS => {
            auto_room_dimensions(cx);
        }
        _ => return false,
    }
    true
}

// ----- polylines -----

/// Adds closed polylines on the current CAD layer as one undo step.
fn add_polylines(cx: &mut EditorContext, label: &str, rings: Vec<Vec<Point>>) -> Vec<Id> {
    let rings: Vec<Vec<Point>> = rings.into_iter().filter(|r| r.len() >= 3).collect();
    if rings.is_empty() {
        cx.status = format!("{label}: there is nothing to outline");
        return Vec::new();
    }
    cx.begin_change(label);
    let fl = cx.floor;
    let layer = cx.project.layers.current_cad_layer();
    let ids: Vec<Id> = rings
        .into_iter()
        .map(|points| {
            cx.project.add_cad(
                fl,
                layer.clone(),
                CadItem::Polyline {
                    points,
                    closed: true,
                },
            )
        })
        .collect();
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!(
        "{label}: {} polyline(s) on the \"{layer}\" layer",
        ids.len()
    );
    ids
}

/// Make Room Polyline: a CAD polyline along the surfaces of the selected room
/// (the expanded room after Expand Room Polyline).
pub fn make_room_polyline(cx: &mut EditorContext) -> Vec<Id> {
    let Some(room) = target_room(cx) else {
        cx.status = "Select a room first".into();
        return Vec::new();
    };
    add_polylines(cx, "Make Room Polyline", vec![living::room_polyline(&room)])
}

/// Make Standard Area Polyline: along the extent of the room's Standard Area.
pub fn make_standard_area_polyline(cx: &mut EditorContext) -> Vec<Id> {
    let Some(room) = target_room(cx) else {
        cx.status = "Select a room first".into();
        return Vec::new();
    };
    let def = living::defining_walls(&cx.floor().walls);
    let ring =
        living::standard_area_polyline(&room, &def, cx.wall_types(), living_basis(&cx.defaults));
    add_polylines(cx, "Make Standard Area Polyline", vec![ring])
}

/// Make Room Polyline on the Exterior Room: a polyline around the exterior
/// walls, the footprint of the structure (p. 455).
pub fn make_exterior_polyline(cx: &mut EditorContext) -> Vec<Id> {
    let Some(i) = selected_exterior(cx) else {
        cx.status = "Select the Exterior Room first".into();
        return Vec::new();
    };
    let s = structures(cx).swap_remove(i);
    let rings = living::structure_polylines(
        cx.floor(),
        &cx.rooms,
        &s,
        cx.wall_types(),
        living_basis(&cx.defaults),
    );
    add_polylines(cx, "Make Room Polyline", rings)
}

/// Make Living Area Polyline: the exact extent of the structure's Living Area
/// on this floor. It stays as drawn when the model changes.
pub fn make_living_area_polyline(cx: &mut EditorContext) -> Vec<Id> {
    let Some(i) = selected_exterior(cx) else {
        cx.status = "Select the Exterior Room first".into();
        return Vec::new();
    };
    let Some(area) = living_labels(cx).into_iter().find(|a| a.structure == i) else {
        cx.status = "None of the rooms of this structure is in the Living Area".into();
        return Vec::new();
    };
    let rings = living::living_area_polylines(
        cx.floor(),
        &cx.rooms,
        &area,
        cx.wall_types(),
        living_basis(&cx.defaults),
    );
    add_polylines(cx, "Make Living Area Polyline", rings)
}

/// Expand Room Polyline: selects a temporarily enlarged room that ignores the
/// invisible walls and railings around the selected room; the polyline
/// buttons then use it. Not a change to the plan (no undo step).
pub fn expand_room_polyline(cx: &mut EditorContext) -> bool {
    let Some(room) = selected_room(cx).and_then(|i| cx.rooms.get(i)) else {
        cx.status = "Select a room first".into();
        return false;
    };
    match plan_core::living::expand_room(&cx.floor().walls, 0.5, room_anchor(room)) {
        Some(big) => {
            let area = big.interior_area_sq_ft();
            with(|s| {
                s.expanded = Some(Expanded {
                    floor: cx.floor,
                    room: big,
                })
            });
            cx.status = format!("Expanded room: {} sq ft", area.round());
            true
        }
        None => {
            cx.status = "This room has no invisible walls or railings to ignore".into();
            false
        }
    }
}

// ----- elevations, dimensions, schedule -----

/// Create Room Elevation Views: an interior elevation view of each wall of the
/// selected room (the four views Auto Interior Elevations makes). One undo
/// step; views of the same name are updated, not repeated.
pub fn create_room_elevation_views(cx: &mut EditorContext) -> Vec<Id> {
    let Some(room) = selected_room(cx).and_then(|i| cx.rooms.get(i)).cloned() else {
        cx.status = "Select a room first".into();
        return Vec::new();
    };
    cx.begin_change("Create Room Elevation Views");
    let fl = cx.floor;
    let before = (cx.project.cameras.clone(), cx.project.layers.layers.len());
    let ids =
        crate::tools::camera::add_interior_elevations(&mut cx.project, fl, room_anchor(&room));
    if ids.is_empty() {
        cx.cancel_change();
        cx.status = "Create Room Elevation Views: the room has no walls to view".into();
        return ids;
    }
    // Views of the same names are updated; when they are already right there
    // is nothing to undo.
    if before == (cx.project.cameras.clone(), cx.project.layers.layers.len()) {
        cx.cancel_change();
        cx.status = format!("The {} interior elevation views are up to date", ids.len());
        return ids;
    }
    cx.mark_dirty();
    cx.status = format!("Created {} interior elevation views", ids.len());
    ids
}

/// Auto Room Dimensions: interior dimensions that measure each wall defining
/// the selected room, a string just inside its walls. A second run replaces
/// the strings of the room instead of repeating them. One undo step.
pub fn auto_room_dimensions(cx: &mut EditorContext) -> usize {
    let Some(room) = selected_room(cx).and_then(|i| cx.rooms.get(i)).cloned() else {
        cx.status = "Select a room first".into();
        return 0;
    };
    let poly = if room.inner_polygon.len() >= 3 {
        room.inner_polygon.clone()
    } else {
        room.polygon.clone()
    };
    let sep = {
        let s = cx.defaults.dimensions.auto_line_separation;
        if s > 0.0 {
            s
        } else {
            18.0
        }
    };
    let n = poly.len();
    let dims: Vec<Dimension> = (0..n)
        .filter_map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            (a.dist(b) >= 6.0).then(|| {
                let mut d = Dimension::new(0, DimensionKind::AutoExterior, a, b, sep);
                d.auto_group = AutoGroup::Interior;
                d
            })
        })
        .collect();
    if dims.is_empty() {
        cx.status = "Auto Room Dimensions: the room has no walls to measure".into();
        return 0;
    }
    cx.begin_change("Auto Room Dimensions");
    let fl = cx.floor;
    // The strings a former run made for this room go first.
    cx.project.floors[fl].dimensions.retain(|d| {
        if d.kind != DimensionKind::AutoExterior || d.auto_group != AutoGroup::Interior {
            return true;
        }
        let (p, q) = d.line_points();
        !point_in_polygon_xy(Point::lerp(p, q, 0.5), &poly)
    });
    let style = cx.defaults.dimensions.text_style.clone();
    let count = dims.len();
    for mut d in dims {
        if d.text_style.is_none() && !style.is_empty() {
            d.text_style = Some(style.clone());
        }
        cx.project.add_dimension(fl, d);
    }
    cx.mark_dirty();
    cx.status = format!("Added {count} room dimensions");
    count
}

/// Is `p` inside the room outline or within a dimension offset of it?
fn point_in_polygon_xy(p: Point, poly: &[Point]) -> bool {
    plan_core::geometry::point_in_polygon(p, poly)
        || (0..poly.len()).any(|i| {
            plan_core::geometry::dist_to_segment(p, poly[i], poly[(i + 1) % poly.len()]) < 30.0
        })
}

#[cfg(test)]
/// Create Schedule from Room: a schedule of `kind` limited to the selected
/// room, placed at `at` (the chooser dialog and the click that places it are
/// the Schedule tool's; this is the same step it ends with).
pub fn create_schedule_from_room(
    cx: &mut EditorContext,
    kind: plan_core::schedules::ScheduleKind,
    at: Point,
) -> Option<Id> {
    let r = selected_room(cx)?;
    crate::editor::schedule_view::create_from_room(cx, kind, r, at)
}
