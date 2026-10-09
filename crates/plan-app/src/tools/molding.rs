//! Moldings in the editor: the profile library (Add to Library as a molding
//! profile, Place Molding Profile, Edit Molding Profile), Replace Moldings
//! and Replace From Library, the edit buttons of molding polylines (Make Room
//! Molding Polyline, Make Cabinet Molding Polyline, Reverse Direction,
//! Remove / Add Molding to Selected Edge) and the hand-off functions the
//! dialogs with a Moldings panel (room, Floor Defaults, cabinet, wall cap)
//! call to store their table. The model is `plan_core::moldings`, the panel
//! `dialogs/molding.rs` and the sweep `plan_3d::molding`.
//!
//! Every command here is one undo step.
//!
//! # State between uses
//!
//! The profile the Molding Polyline tool draws with and Replace Moldings
//! swaps in is remembered ([`active_profile`]); picking a profile in the
//! Moldings panel's library list, adding a polyline to the library and
//! placing a profile all make it the active one. The edge Remove / Add
//! Molding to Selected Edge and the Selected Line panel act on is
//! [`selected_edge_of`].

use crate::editor::actions::{EditAction, EditActionKind};
use crate::editor::{details_view as dv, EditorContext, ObjectRef};
use plan_cabinets::{Cabinet, CabinetKind, MoldingKind as CabMoldingKind};
use plan_core::cad::{CadItem, CadObject};
use plan_core::details::{DetailsLayer, MoldingLine, MoldingSide, MoldingSource};
use plan_core::geometry::Point;
use plan_core::moldings::{
    builtin_profiles, generated_room_lines, square_profile, MoldingTable, MoldingType, ProfileDef,
    ProfileError, TableSource,
};
use plan_core::{Id, Project};
use std::cell::RefCell;

/// Add the selected closed CAD polyline to the library as a molding profile.
pub const ADD_PROFILE: &str = "molding.add_profile";
/// Add the selected closed CAD polylines as one stacked molding profile.
pub const ADD_STACKED_PROFILE: &str = "molding.add_stacked_profile";
/// Place the active molding profile in the plan as a closed polyline.
pub const PLACE_PROFILE: &str = "molding.place_profile";
/// Edit the active molding profile: it is placed as a polyline to reshape.
pub const EDIT_PROFILE: &str = "molding.edit_profile";
/// Replace the selected moldings with the active profile.
pub const REPLACE_FROM_LIBRARY: &str = "molding.replace_from_library";
/// Make Room Molding Polyline.
pub const MAKE_ROOM_POLYLINE: &str = "molding.make_room_polyline";
/// Make Cabinet Molding Polyline.
pub const MAKE_CABINET_POLYLINE: &str = "molding.make_cabinet_polyline";
/// Reverse Direction of the selected molding polylines.
pub const REVERSE_DIRECTION: &str = "molding.reverse_direction";
/// Remove Molding from Selected Edge.
pub const REMOVE_EDGE: &str = "molding.remove_edge";
/// Add Molding to Selected Edge.
pub const ADD_EDGE: &str = "molding.add_edge";
/// Next edge of the selected molding becomes the selected edge.
pub const NEXT_EDGE: &str = "molding.next_edge";
/// Include Inside Corners for Auto Place Corner Boards and Auto Place Quoins.
pub const INCLUDE_INSIDE: &str = "molding.include_inside_corners";
/// Make Room Molding Polyline from the Exterior Room of the floor.
pub const MAKE_EXTERIOR_POLYLINE: &str = "molding.make_exterior_polyline";

/// Every command id of this module.
pub const COMMANDS: [&str; 13] = [
    ADD_PROFILE,
    ADD_STACKED_PROFILE,
    PLACE_PROFILE,
    EDIT_PROFILE,
    REPLACE_FROM_LIBRARY,
    MAKE_ROOM_POLYLINE,
    MAKE_CABINET_POLYLINE,
    REVERSE_DIRECTION,
    REMOVE_EDGE,
    ADD_EDGE,
    NEXT_EDGE,
    INCLUDE_INSIDE,
    MAKE_EXTERIOR_POLYLINE,
];

thread_local! {
    static ACTIVE: RefCell<Option<ProfileDef>> = const { RefCell::new(None) };
    static EDGE: RefCell<Option<(Id, usize)>> = const { RefCell::new(None) };
}

/// The profile the Molding Polyline tool draws with (the default square
/// profile until another is picked or added to the library).
pub fn active_profile() -> ProfileDef {
    ACTIVE.with(|a| a.borrow().clone().unwrap_or_else(square_profile))
}

/// Makes `p` the profile the tools use.
pub fn set_active_profile(p: ProfileDef) {
    ACTIVE.with(|a| *a.borrow_mut() = Some(p));
}

/// Back to the default square profile.
pub fn reset_active_profile() {
    ACTIVE.with(|a| *a.borrow_mut() = None);
}

/// The selected edge of molding `id` (0 when none was picked).
pub fn selected_edge_of(id: Id) -> usize {
    EDGE.with(|e| e.borrow().filter(|(m, _)| *m == id).map_or(0, |(_, i)| i))
}

/// Selects edge `i` of molding `id`.
pub fn select_edge(id: Id, i: usize) {
    EDGE.with(|e| *e.borrow_mut() = Some((id, i)));
}

/// The index of the edge of `m` nearest to `p`.
pub fn edge_near(m: &MoldingLine, p: Point) -> Option<usize> {
    (0..m.edge_count())
        .map(|i| {
            (
                i,
                plan_core::geometry::dist_to_segment(p, m.polyline[i], m.polyline[i + 1]),
            )
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

// ===================================================================
// The library
// ===================================================================

/// The built-in profiles and the plan's own, the latter first by name.
pub fn all_profiles(project: &Project) -> Vec<ProfileDef> {
    let mut out = builtin_profiles();
    for f in &project.floors {
        for p in DetailsLayer::load(f).profiles {
            match out.iter_mut().find(|q| q.name.eq_ignore_ascii_case(&p.name)) {
                Some(q) => *q = p,
                None => out.push(p),
            }
        }
    }
    out
}

/// The profile named `name` in the built-in library or the plan's.
pub fn find_profile(project: &Project, name: &str) -> Option<ProfileDef> {
    all_profiles(project)
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name.trim()))
}

/// The closed polylines among the selected CAD objects.
fn selected_closed_polylines(cx: &EditorContext) -> Vec<(Id, Vec<Point>)> {
    let cad = &cx.floor().cad;
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) => cad.iter().find(|c| c.id == *id),
            _ => None,
        })
        .filter_map(|c| match &c.item {
            CadItem::Polyline { points, closed } if *closed || is_ring(points) => {
                Some((c.id, points.clone()))
            }
            _ => None,
        })
        .collect()
}

fn is_ring(pts: &[Point]) -> bool {
    pts.len() > 3 && pts[0].dist(pts[pts.len() - 1]) < 1e-6
}

/// A name for a new profile: `base`, or `base 2`, `base 3` ... when taken.
pub fn unique_profile_name(project: &Project, base: &str) -> String {
    let taken = |n: &str| find_profile(project, n).is_some();
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|k| format!("{base} {k}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| base.to_string())
}

/// Add to Library on the selected closed polyline(s): a molding profile at
/// actual size in the plan's library (the back of the molding is the left
/// edge of the polyline). One closed polyline is a profile; with `stacked`
/// several are one stacked profile that keeps their positions relative to
/// each other. Adding a name the library has replaces that profile. Returns
/// the name.
pub fn add_profile_from_selection(
    cx: &mut EditorContext,
    name: Option<&str>,
    kind: MoldingType,
    stacked: bool,
) -> Result<String, String> {
    let polys = selected_closed_polylines(cx);
    if polys.is_empty() {
        return Err("Select a closed polyline to add as a molding profile".into());
    }
    if polys.len() > 1 && !stacked {
        return Err(
            "Select one closed polyline, or use Add to Library as Stacked Molding".into(),
        );
    }
    let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => unique_profile_name(&cx.project, "Molding Profile"),
    };
    let parts: Vec<(Vec<Point>, String)> = polys
        .into_iter()
        .map(|(_, pts)| (pts, String::new()))
        .collect();
    let def = ProfileDef::stacked(name.clone(), kind, parts).map_err(|e: ProfileError| e.to_string())?;
    let stored = def.clone();
    dv::edit(cx, "Add to Library", |l| l.add_profile(stored));
    set_active_profile(def);
    cx.status = format!("{name} added to the library as a molding profile");
    Ok(name)
}

/// Place Molding Profile: the profile `name` as closed polylines at actual
/// size, its back-bottom corner at `at`, on the CAD layer; they are selected.
/// Returns the ids of the polylines.
pub fn place_profile(cx: &mut EditorContext, name: &str, at: Point) -> Vec<Id> {
    let Some(def) = find_profile(&cx.project, name) else {
        cx.status = format!("No molding profile named {name}");
        return Vec::new();
    };
    cx.begin_change("Place Molding Profile");
    let mut ids = Vec::new();
    let fl = cx.floor;
    for part in &def.parts {
        let id = cx.project.alloc_id();
        ids.push(id);
        cx.project.floors[fl].cad.push(CadObject {
            id,
            layer: plan_core::cad::DEFAULT_CAD_LAYER.to_string(),
            item: CadItem::Polyline {
                points: part.section.iter().map(|p| *p + at).collect(),
                closed: true,
            },
        });
    }
    cx.mark_dirty();
    cx.selection.clear();
    for id in &ids {
        cx.selection.add(ObjectRef::Cad(*id));
    }
    set_active_profile(def);
    cx.status = format!("{name} placed; reshape it, then add it to the library again");
    ids
}

/// Edit Molding Profile: places the profile beside the plan to be reshaped
/// with the CAD tools. Adding the polyline to the library under the same
/// name replaces the profile (Chief opens a CAD detail window for this).
pub fn edit_profile(cx: &mut EditorContext, name: &str) -> Vec<Id> {
    let f = cx.floor();
    let right = f
        .walls
        .iter()
        .flat_map(|w| [w.start.x, w.end.x])
        .fold(f64::MIN, f64::max);
    let low = f
        .walls
        .iter()
        .flat_map(|w| [w.start.y, w.end.y])
        .fold(f64::MAX, f64::min);
    let at = if right > f64::MIN / 2.0 {
        Point::new(right + 48.0, low)
    } else {
        Point::new(48.0, 48.0)
    };
    place_profile(cx, name, at)
}

// ===================================================================
// Moldings on the plan
// ===================================================================

/// The selected molding polylines.
pub fn selected_moldings(cx: &EditorContext) -> Vec<Id> {
    let layer = dv::load(cx);
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Detail(id) => layer.molding(*id).map(|m| m.id),
            _ => None,
        })
        .collect()
}

/// Replace Moldings / Replace From Library on moldings: each takes `profile`
/// in place of its main profile (the first row of its table), keeping the
/// offsets and everything else. Returns how many were replaced.
pub fn replace_moldings(cx: &mut EditorContext, ids: &[Id], profile: &ProfileDef) -> usize {
    let layer = dv::load(cx);
    let todo: Vec<Id> = ids
        .iter()
        .copied()
        .filter(|i| layer.molding(*i).is_some())
        .collect();
    if todo.is_empty() || !profile.is_valid() {
        return 0;
    }
    dv::edit(cx, "Replace Moldings", |l| {
        for id in &todo {
            if let Some(m) = l.molding_mut(*id) {
                m.ensure_table();
                m.table.replace(0, profile.clone());
                if let Some(r) = m.table.rows.first() {
                    m.width = r.width;
                    m.height = r.height;
                }
                m.profile = plan_core::details::MoldingProfile::Custom(profile.section().to_vec());
                m.automatic = false;
            }
        }
    });
    todo.len()
}

/// Replace Moldings with a click at `p`: the molding under it takes the
/// active profile. Returns the id replaced.
pub fn replace_at(cx: &mut EditorContext, p: Point) -> Option<Id> {
    let tol = cx.pick_tol();
    let layer = dv::load(cx);
    let hit = layer
        .moldings
        .iter()
        .filter_map(|m| {
            edge_near(m, p).map(|i| {
                (
                    m.id,
                    plan_core::geometry::dist_to_segment(p, m.polyline[i], m.polyline[i + 1]),
                )
            })
        })
        .filter(|(_, d)| *d <= tol.max(m_reach(&layer, p)))
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    let profile = active_profile();
    if replace_moldings(cx, &[hit.0], &profile) == 1 {
        Some(hit.0)
    } else {
        None
    }
}

/// How far from a molding's path a click still counts, inches: the molding's
/// own projection.
fn m_reach(layer: &DetailsLayer, p: Point) -> f64 {
    layer
        .moldings
        .iter()
        .filter_map(|m| edge_near(m, p).map(|_| m.width))
        .fold(0.0, f64::max)
}

/// Reverse Direction on the molding polylines `ids`.
pub fn reverse_direction(cx: &mut EditorContext, ids: &[Id]) -> usize {
    let layer = dv::load(cx);
    let todo: Vec<Id> = ids
        .iter()
        .copied()
        .filter(|i| layer.molding(*i).is_some())
        .collect();
    if todo.is_empty() {
        return 0;
    }
    dv::edit(cx, "Reverse Direction", |l| {
        for id in &todo {
            if let Some(m) = l.molding_mut(*id) {
                m.reverse_direction();
            }
        }
    });
    todo.len()
}

/// Remove Molding from Selected Edge / Add Molding to Selected Edge on edge
/// `edge` of molding `id`. Returns whether anything changed.
pub fn set_edge(cx: &mut EditorContext, id: Id, edge: usize, on: bool) -> bool {
    let layer = dv::load(cx);
    let Some(m) = layer.molding(id) else {
        return false;
    };
    if edge >= m.edge_count() || m.edge_on(edge) == on {
        return false;
    }
    let label = if on {
        "Add Molding to Selected Edge"
    } else {
        "Remove Molding from Selected Edge"
    };
    dv::edit(cx, label, |l| {
        if let Some(m) = l.molding_mut(id) {
            m.set_edge_on(edge, on);
        }
    });
    true
}

/// Include Inside Corners for the auto-placed corner trim.
pub fn set_include_inside(cx: &mut EditorContext, on: bool) -> bool {
    if dv::load(cx).include_inside_corners == on {
        return false;
    }
    dv::edit(cx, "Include Inside Corners", |l| l.include_inside_corners = on);
    true
}

// ===================================================================
// Make Room / Cabinet Molding Polyline
// ===================================================================

/// Make Room Molding Polyline: the moldings of the room at `at` (its
/// Moldings table, the Floor Defaults it uses, or the older Moldings tab)
/// become editable molding polylines and the room stops generating them.
/// Returns how many polylines were made.
pub fn make_room_molding_polylines(cx: &mut EditorContext, at: Point) -> usize {
    let rooms = cx.rooms_now().to_vec();
    let Some(room) = rooms.iter().find(|r| r.contains(at)).cloned() else {
        cx.status = "Click inside a room first".into();
        return 0;
    };
    let layer = dv::load(cx);
    let floor = cx.floor().clone();
    let (datum, ceiling) = plan_3d::molding::room_datum_and_ceiling(&floor, &room);
    let lines = generated_room_lines(&floor, &layer, &room, ceiling, datum);
    if lines.is_empty() {
        cx.status = "That room has no moldings to convert".into();
        return 0;
    }
    let anchor = room
        .name_entry(&floor.room_names)
        .map_or_else(|| room.interior_point(), |n| n.anchor);
    cx.begin_change("Make Room Molding Polyline");
    let mut layer = layer;
    let n = lines.len();
    for mut line in lines {
        line.id = cx.project.alloc_id();
        line.automatic = false;
        layer.moldings.push(line);
    }
    // The room no longer generates them.
    let rec = layer.room_moldings_mut(anchor);
    rec.table.source = TableSource::Own;
    rec.table.rows.clear();
    let fl = cx.floor;
    dv::save(&mut cx.project, fl, &layer);
    if let Some(named) = cx.project.floors[fl]
        .room_names
        .iter_mut()
        .find(|r| r.anchor.dist(anchor) < 1.0)
    {
        named.moldings.clear();
    }
    cx.mark_dirty();
    cx.status = format!("{n} molding polyline{} made", if n == 1 { "" } else { "s" });
    n
}

/// The lines of one cabinet's moldings: a front run and both returns on a
/// rectangular cabinet, a ring around the outline on a corner cabinet.
fn cabinet_lines(c: &Cabinet, elevation: f64) -> Vec<MoldingLine> {
    let mut out = Vec::new();
    for m in &c.moldings {
        let (z0, z1) = match m.kind {
            CabMoldingKind::Crown => (c.height, c.height + m.height),
            CabMoldingKind::LightRail => (-m.height, 0.0),
        };
        let _ = z1;
        let (kind, name) = match m.kind {
            CabMoldingKind::Crown => (MoldingType::Crown, "Cabinet Crown"),
            CabMoldingKind::LightRail => (MoldingType::Rail, "Cabinet Light Rail"),
        };
        let Ok(def) = ProfileDef::from_polyline(
            name,
            kind,
            &[
                Point::new(0.0, 0.0),
                Point::new(m.projection, 0.0),
                Point::new(m.projection, m.height),
                Point::new(0.0, m.height),
            ],
        ) else {
            continue;
        };
        let bottom = elevation + c.elevation + z0;
        let (path, side) = if matches!(c.kind, CabinetKind::CornerBase | CabinetKind::CornerWall) {
            let mut ring = c.footprint();
            let first = ring[0];
            ring.push(first);
            (ring, MoldingSide::Right)
        } else {
            let (w, d) = (c.width, c.depth);
            (
                vec![
                    c.to_plan(Point::new(0.0, 0.0)),
                    c.to_plan(Point::new(0.0, d)),
                    c.to_plan(Point::new(w, d)),
                    c.to_plan(Point::new(w, 0.0)),
                ],
                MoldingSide::Left,
            )
        };
        let mut line = MoldingLine::with_profile(0, path, def, bottom);
        line.side = side;
        line.source = MoldingSource::Cabinet(c.id);
        out.push(line);
    }
    out
}

/// Make Cabinet Molding Polyline on the selected cabinets: their crown and
/// light rail become editable molding polylines and leave the cabinet.
/// Returns how many polylines were made.
pub fn make_cabinet_molding_polylines(cx: &mut EditorContext) -> usize {
    let ids: Vec<Id> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cabinet(id) => Some(*id),
            _ => None,
        })
        .collect();
    let fl = cx.floor;
    let Ok(mut cabs) = cx.project.floors[fl].cabinets_as::<Cabinet>() else {
        return 0;
    };
    let mut lines: Vec<MoldingLine> = Vec::new();
    for c in cabs.iter().filter(|c| ids.contains(&c.id)) {
        lines.extend(cabinet_lines(c, 0.0));
    }
    if lines.is_empty() {
        cx.status = "The selected cabinets have no moldings".into();
        return 0;
    }
    cx.begin_change("Make Cabinet Molding Polyline");
    for c in cabs.iter_mut().filter(|c| ids.contains(&c.id)) {
        c.moldings.clear();
    }
    let _ = cx.project.floors[fl].set_cabinets(&cabs);
    let mut layer = dv::load(cx);
    let n = lines.len();
    for mut line in lines {
        line.id = cx.project.alloc_id();
        layer.moldings.push(line);
    }
    dv::save(&mut cx.project, fl, &layer);
    cx.mark_dirty();
    cx.status = format!("{n} molding polyline{} made", if n == 1 { "" } else { "s" });
    n
}

// ===================================================================
// Tables of rooms and floors
// ===================================================================

/// The molding table of the Floor Defaults.
pub fn floor_molding_table(cx: &EditorContext) -> MoldingTable {
    dv::load(cx).floor_moldings
}

/// Stores the Floor Defaults molding table (one undo step).
pub fn set_floor_molding_table(cx: &mut EditorContext, table: MoldingTable) {
    dv::edit(cx, "Floor Defaults Moldings", |l| l.floor_moldings = table);
}

/// The molding table of the room whose label anchor is `anchor`, if it has a
/// record of its own.
pub fn room_molding_table(cx: &EditorContext, anchor: Point) -> Option<MoldingTable> {
    dv::load(cx).room_moldings_at(anchor).map(|r| r.table.clone())
}

/// Stores the molding table of a room. The older Moldings tab of the room
/// stops drawing (its list is cleared) so the room's moldings are only built
/// once. One undo step.
pub fn set_room_molding_table(cx: &mut EditorContext, anchor: Point, table: MoldingTable) {
    cx.begin_change("Room Moldings");
    let mut layer = dv::load(cx);
    layer.room_moldings_mut(anchor).table = table;
    let fl = cx.floor;
    dv::save(&mut cx.project, fl, &layer);
    if let Some(named) = cx.project.floors[fl]
        .room_names
        .iter_mut()
        .find(|r| r.anchor.dist(anchor) < 1.0)
    {
        named.moldings.clear();
    }
    cx.mark_dirty();
}

/// Per-edge molding of a room: turns the molding on or off along edge `edge`
/// of the room's interior outline. One undo step.
pub fn set_room_edge(cx: &mut EditorContext, anchor: Point, edge: usize, on: bool) {
    dv::edit(cx, "Room Molding Edge", |l| {
        let rec = l.room_moldings_mut(anchor);
        rec.off_edges.retain(|e| *e != edge);
        if !on {
            rec.off_edges.push(edge);
            rec.off_edges.sort_unstable();
        }
    });
}

/// The molding table of a room type's defaults (Room Type Defaults,
/// Moldings), if the type has one.
pub fn type_molding_table(cx: &EditorContext, room_type: &str) -> Option<MoldingTable> {
    dv::load(cx)
        .type_moldings
        .iter()
        .find(|t| t.room_type.eq_ignore_ascii_case(room_type.trim()))
        .map(|t| t.table.clone())
}

/// Stores the molding table of a room type's defaults (one undo step).
pub fn set_type_molding_table(cx: &mut EditorContext, room_type: &str, table: MoldingTable) {
    let name = room_type.trim().to_string();
    dv::edit(cx, "Room Type Moldings", |l| {
        match l
            .type_moldings
            .iter_mut()
            .find(|t| t.room_type.eq_ignore_ascii_case(&name))
        {
            Some(t) => t.table = table,
            None => l.type_moldings.push(plan_core::moldings::TypeMoldings {
                room_type: name,
                table,
            }),
        }
    });
}

/// Make Room Molding Polyline from the Exterior Room: a closed molding
/// polyline round the outer faces of the exterior walls of the floor,
/// `height` above the floor (the Convert Molding / Height dialog; "Blank
/// Molding" is the square profile). Returns its id.
pub fn make_exterior_molding_polyline(
    cx: &mut EditorContext,
    profile: ProfileDef,
    height: f64,
) -> Option<Id> {
    let (rings, _) = crate::tools::cad_ops::footprint_rings(cx)?;
    let ring = rings
        .into_iter()
        .max_by(|a, b| {
            plan_core::geometry::polygon_area(&a.pts)
                .abs()
                .total_cmp(&plan_core::geometry::polygon_area(&b.pts).abs())
        })?;
    let mut pts = ring.pts;
    if pts.len() < 3 {
        return None;
    }
    if plan_core::geometry::polygon_area(&pts) < 0.0 {
        pts.reverse();
    }
    let first = pts[0];
    pts.push(first);
    let id = cx.project.alloc_id();
    // Counter-clockwise, projecting to the right: outward.
    dv::edit(cx, "Make Room Molding Polyline", |l| {
        let mut line = MoldingLine::with_profile(id, pts, profile, height);
        line.source = MoldingSource::Manual;
        l.moldings.push(line);
    });
    dv::select(cx, plan_core::details::DetailRef::Molding(id));
    cx.status = "Exterior room molding polyline made".into();
    Some(id)
}

/// Suppress Adjacent Room Moldings for a wall: room moldings stop at it.
pub fn set_wall_suppresses_moldings(cx: &mut EditorContext, wall: Id, on: bool) {
    dv::edit(cx, "Suppress Room Moldings", |l| {
        l.molding_free_walls.retain(|w| *w != wall);
        if on {
            l.molding_free_walls.push(wall);
        }
    });
}

// ===================================================================
// Edit buttons and commands
// ===================================================================

fn button(id: &'static str, label: &'static str) -> EditAction {
    EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled: true,
    }
}

/// The Edit toolbar buttons for the selection: the molding polyline
/// buttons, Make Cabinet Molding Polyline on cabinets with moldings, and
/// Add to Library on a closed polyline.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    let layer = dv::load(cx);
    let ms = selected_moldings(cx);
    if !ms.is_empty() {
        v.push(button(REVERSE_DIRECTION, "Reverse Direction"));
        if ms.len() == 1 {
            if let Some(m) = layer.molding(ms[0]) {
                let e = selected_edge_of(m.id).min(m.edge_count().saturating_sub(1));
                if m.edge_on(e) {
                    v.push(button(REMOVE_EDGE, "Remove Molding from Selected Edge"));
                } else {
                    v.push(button(ADD_EDGE, "Add Molding to Selected Edge"));
                }
                if m.edge_count() > 1 {
                    v.push(button(NEXT_EDGE, "Select Next Edge"));
                }
            }
        }
        v.push(button(REPLACE_FROM_LIBRARY, "Replace Moldings"));
    }
    let polys = selected_closed_polylines(cx);
    if polys.len() == 1 {
        v.push(button(ADD_PROFILE, "Add to Library as Molding Profile"));
    } else if polys.len() > 1 {
        v.push(button(
            ADD_STACKED_PROFILE,
            "Add to Library as Stacked Molding",
        ));
    }
    let has_cabinet_molding = cx.selection.items.iter().any(|o| match o {
        ObjectRef::Cabinet(id) => cx
            .floor()
            .cabinets_as::<Cabinet>()
            .is_ok_and(|cs| cs.iter().any(|c| c.id == *id && !c.moldings.is_empty())),
        _ => false,
    });
    if has_cabinet_molding {
        v.push(button(MAKE_CABINET_POLYLINE, "Make Cabinet Molding Polyline"));
    }
    v
}

/// Runs a command id of this module; false when it is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        ADD_PROFILE | ADD_STACKED_PROFILE => {
            let stacked = id == ADD_STACKED_PROFILE;
            if let Err(e) = add_profile_from_selection(cx, None, MoldingType::Other, stacked) {
                cx.status = e;
            }
        }
        PLACE_PROFILE => {
            let at = cx.cursor_world.unwrap_or(Point::ZERO);
            let name = active_profile().name;
            place_profile(cx, &name, at);
        }
        EDIT_PROFILE => {
            let name = active_profile().name;
            edit_profile(cx, &name);
        }
        REPLACE_FROM_LIBRARY => {
            let ids = selected_moldings(cx);
            let p = active_profile();
            let n = replace_moldings(cx, &ids, &p);
            cx.status = if n == 0 {
                "Select a molding polyline first".into()
            } else {
                format!("{n} molding{} replaced with {}", if n == 1 { "" } else { "s" }, p.name)
            };
        }
        MAKE_ROOM_POLYLINE => {
            let at = cx.cursor_world.or_else(|| {
                let rooms = cx.rooms_now();
                (rooms.len() == 1).then(|| rooms[0].interior_point())
            });
            match at {
                Some(p) => {
                    make_room_molding_polylines(cx, p);
                }
                None => cx.status = "Click inside a room first".into(),
            }
        }
        MAKE_CABINET_POLYLINE => {
            make_cabinet_molding_polylines(cx);
        }
        REVERSE_DIRECTION => {
            let ids = selected_moldings(cx);
            reverse_direction(cx, &ids);
        }
        REMOVE_EDGE | ADD_EDGE => {
            let ids = selected_moldings(cx);
            if let [one] = ids.as_slice() {
                let e = selected_edge_of(*one);
                set_edge(cx, *one, e, id == ADD_EDGE);
            }
        }
        NEXT_EDGE => {
            let ids = selected_moldings(cx);
            if let [one] = ids.as_slice() {
                let n = dv::load(cx).molding(*one).map_or(0, MoldingLine::edge_count);
                if n > 0 {
                    select_edge(*one, (selected_edge_of(*one) + 1) % n);
                }
            }
        }
        INCLUDE_INSIDE => {
            let now = dv::load(cx).include_inside_corners;
            set_include_inside(cx, !now);
        }
        MAKE_EXTERIOR_POLYLINE => {
            let height = cx.floor().ceiling_height;
            if make_exterior_molding_polyline(cx, active_profile(), height).is_none() {
                cx.status = "The floor has no exterior walls".into();
            }
        }
        _ => return false,
    }
    true
}

/// A molding's profile name for lists and labels.
pub fn label_of(m: &MoldingLine) -> String {
    if !m.label.trim().is_empty() {
        return m.label.clone();
    }
    match m.table.rows.first() {
        Some(r) => r.profile.name.clone(),
        None => m.profile.name().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    fn cx() -> EditorContext {
        reset_active_profile();
        EditorContext::new(plan_defaults::embedded())
    }

    fn add_cad_polyline(cx: &mut EditorContext, pts: Vec<Point>) -> Id {
        let id = cx.project.alloc_id();
        let fl = cx.floor;
        cx.project.floors[fl].cad.push(CadObject {
            id,
            layer: plan_core::cad::DEFAULT_CAD_LAYER.to_string(),
            item: CadItem::Polyline {
                points: pts,
                closed: true,
            },
        });
        id
    }

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_closed_polyline_becomes_a_library_profile_in_one_step() {
        let mut cx = cx();
        let id = add_cad_polyline(
            &mut cx,
            vec![pt(10.0, 10.0), pt(13.0, 10.0), pt(13.0, 14.0), pt(10.0, 14.0)],
        );
        cx.selection.set(ObjectRef::Cad(id));
        let name = add_profile_from_selection(&mut cx, Some("My Base"), MoldingType::Base, false)
            .unwrap();
        assert_eq!(name, "My Base");
        let p = find_profile(&cx.project, "my base").expect("in the library");
        assert!((p.width() - 3.0).abs() < 1e-9 && (p.height() - 4.0).abs() < 1e-9);
        assert_eq!(active_profile().name, "My Base");
        assert_eq!(cx.undo().as_deref(), Some("Add to Library"));
        assert!(find_profile(&cx.project, "My Base").is_none());
    }

    #[test]
    fn open_or_many_polylines_are_refused_unless_stacked() {
        let mut cx = cx();
        let a = add_cad_polyline(&mut cx, vec![pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 1.0)]);
        let b = add_cad_polyline(&mut cx, vec![pt(0.0, 1.0), pt(2.0, 1.0), pt(2.0, 2.0), pt(0.0, 2.0)]);
        cx.selection.set(ObjectRef::Cad(a));
        cx.selection.add(ObjectRef::Cad(b));
        assert!(add_profile_from_selection(&mut cx, None, MoldingType::Crown, false).is_err());
        let name = add_profile_from_selection(&mut cx, Some("Built-up"), MoldingType::Crown, true)
            .unwrap();
        let p = find_profile(&cx.project, &name).unwrap();
        assert!(p.is_stack());
        // Nothing selected: an error, not a change.
        cx.selection.clear();
        assert!(add_profile_from_selection(&mut cx, None, MoldingType::Crown, false).is_err());
    }

    #[test]
    fn a_placed_profile_is_a_closed_polyline_at_actual_size() {
        let mut cx = cx();
        let crown = builtin_profiles()
            .into_iter()
            .find(|p| p.kind == MoldingType::Crown)
            .unwrap();
        let ids = place_profile(&mut cx, &crown.name, pt(100.0, 50.0));
        assert_eq!(ids.len(), 1);
        let obj = cx.floor().cad.iter().find(|c| c.id == ids[0]).unwrap();
        let CadItem::Polyline { points, closed } = &obj.item else {
            panic!("a polyline");
        };
        assert!(*closed);
        let lo = points.iter().fold(pt(f64::MAX, f64::MAX), |a, p| pt(a.x.min(p.x), a.y.min(p.y)));
        assert!((lo.x - 100.0).abs() < 1e-9 && (lo.y - 50.0).abs() < 1e-9);
        // And back: adding it to the library again replaces the profile.
        let n = add_profile_from_selection(&mut cx, Some("Back Again"), MoldingType::Crown, false)
            .unwrap();
        let back = find_profile(&cx.project, &n).unwrap();
        assert!((back.height() - crown.height()).abs() < 1e-9);
        assert_eq!(cx.undo().as_deref(), Some("Add to Library"));
        assert_eq!(cx.undo().as_deref(), Some("Place Molding Profile"));
    }

    fn molding(cx: &mut EditorContext) -> Id {
        dv::add_molding(
            cx,
            vec![pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 100.0)],
            plan_core::details::MoldingProfile::Base,
        )
    }

    #[test]
    fn replace_moldings_swaps_the_profile_keeping_offsets() {
        let mut cx = cx();
        let id = molding(&mut cx);
        dv::edit(&mut cx, "t", |l| {
            let m = l.molding_mut(id).unwrap();
            m.ensure_table();
            m.table.rows[0].v_offset = 1.5;
        });
        let crown = builtin_profiles()
            .into_iter()
            .find(|p| p.kind == MoldingType::Crown)
            .unwrap();
        assert_eq!(replace_moldings(&mut cx, &[id], &crown), 1);
        let l = dv::load(&cx);
        let m = l.molding(id).unwrap();
        assert_eq!(m.table.rows[0].profile.name, crown.name);
        assert_eq!(m.table.rows[0].v_offset, 1.5);
        assert!((m.height - crown.height()).abs() < 1e-9);
        assert_eq!(label_of(m), crown.name);
        assert_eq!(cx.undo().as_deref(), Some("Replace Moldings"));
        assert_eq!(replace_moldings(&mut cx, &[999], &crown), 0);
    }

    #[test]
    fn reverse_direction_and_edges_are_one_step_each() {
        let mut cx = cx();
        let id = molding(&mut cx);
        let before = dv::load(&cx).molding(id).unwrap().polyline.clone();
        assert_eq!(reverse_direction(&mut cx, &[id]), 1);
        let after = dv::load(&cx).molding(id).unwrap().polyline.clone();
        assert_eq!(after[0], before[2]);
        assert_eq!(cx.undo().as_deref(), Some("Reverse Direction"));
        assert!(set_edge(&mut cx, id, 1, false));
        assert!(!dv::load(&cx).molding(id).unwrap().edge_on(1));
        assert!(!set_edge(&mut cx, id, 1, false), "already off");
        assert!(set_edge(&mut cx, id, 1, true));
        assert_eq!(cx.undo().as_deref(), Some("Add Molding to Selected Edge"));
        assert_eq!(cx.undo().as_deref(), Some("Remove Molding from Selected Edge"));
        assert!(!set_edge(&mut cx, id, 9, false));
    }

    #[test]
    fn the_edit_buttons_follow_the_selection() {
        let mut cx = cx();
        let id = molding(&mut cx);
        assert!(edit_actions(&cx).is_empty());
        cx.selection.set(ObjectRef::Detail(id));
        let labels: Vec<&str> = edit_actions(&cx).iter().map(|a| a.label).collect();
        assert!(labels.contains(&"Reverse Direction"));
        assert!(labels.contains(&"Remove Molding from Selected Edge"));
        assert!(labels.contains(&"Replace Moldings"));
        select_edge(id, 1);
        set_edge(&mut cx, id, 1, false);
        let labels: Vec<&str> = edit_actions(&cx).iter().map(|a| a.label).collect();
        assert!(labels.contains(&"Add Molding to Selected Edge"));
        assert!(run_command(&mut cx, NEXT_EDGE));
        assert_eq!(selected_edge_of(id), 0);
        assert!(!run_command(&mut cx, "something.else"));
    }

    #[test]
    fn floor_and_room_tables_are_stored_in_one_step() {
        let mut cx = cx();
        let mut t = MoldingTable::default();
        t.add_new(square_profile());
        set_floor_molding_table(&mut cx, t.clone());
        assert_eq!(floor_molding_table(&cx), t);
        assert_eq!(cx.undo().as_deref(), Some("Floor Defaults Moldings"));
        assert!(floor_molding_table(&cx).is_empty());
        set_room_molding_table(&mut cx, pt(10.0, 10.0), t.clone());
        assert_eq!(room_molding_table(&cx, pt(10.0, 10.5)), Some(t.clone()));
        set_room_edge(&mut cx, pt(10.0, 10.0), 2, false);
        assert_eq!(dv::load(&cx).room_moldings_at(pt(10.0, 10.0)).unwrap().off_edges, vec![2]);
        set_room_edge(&mut cx, pt(10.0, 10.0), 2, true);
        assert!(dv::load(&cx).room_moldings_at(pt(10.0, 10.0)).unwrap().off_edges.is_empty());
        set_type_molding_table(&mut cx, "Den", t.clone());
        assert_eq!(type_molding_table(&cx, "den"), Some(t.clone()));
        assert!(type_molding_table(&cx, "Bath").is_none());
        set_wall_suppresses_moldings(&mut cx, 7, true);
        assert_eq!(dv::load(&cx).molding_free_walls, vec![7]);
        assert!(set_include_inside(&mut cx, true));
        assert!(!set_include_inside(&mut cx, true));
    }
}
