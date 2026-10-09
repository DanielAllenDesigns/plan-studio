//! Scenario 90: the moldings system (Reference Manual ch. 29, brief 31).
//!
//! A crown profile is drawn as a closed CAD polyline and added to the
//! library; a room takes it through its Moldings table (the hand-off the Room
//! Specification uses); Make Room Molding Polyline turns the generated
//! moldings into an editable polyline, one edge comes off, Replace From
//! Library swaps the profile and Reverse Direction flips it; the Materials
//! List take-off reports the linear lengths under Interior Trim. The Molding
//! Polyline tool draws a closed rectangle with a drag (clockwise, so the
//! profile lies inside), Replace Moldings swaps a profile with a click, and
//! Auto Place Corner Boards and Quoins take inside corners when asked. Every
//! action is exactly one undo step.

use super::{draw_shell, Sim};
use crate::editor::details_view as dv;
use crate::editor::ObjectRef;
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::toolbar::Action;
use crate::tools::details::DetailsVariant;
use crate::tools::molding as mold;
use crate::tools::ToolId;
use plan_3d::{Material, Scene};
use plan_core::cad::{CadItem, CadObject};
use plan_core::details::{DetailRef, MoldingSide};
use plan_core::geometry::Point;
use plan_core::moldings::{
    builtin_profiles, path_length, trim_takeoff, MoldingTable, MoldingType, EXTERIOR_TRIM,
    INTERIOR_TRIM,
};
use plan_core::{Id, RoomName, WallKind};

const W: f64 = 240.0;
const H: f64 = 192.0;
const ANCHOR: Point = Point::new(120.0, 96.0);

fn house() -> Sim {
    mold::reset_active_profile();
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.cx()
        .floor_mut()
        .room_names
        .push(RoomName::new(ANCHOR, "Den", "Den"));
    sim.app.cx.refresh();
    sim
}

fn scene(sim: &Sim) -> Scene {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
}

fn trim_triangles(scene: &Scene) -> usize {
    scene
        .meshes
        .iter()
        .filter(|m| m.material == Material::Trim)
        .map(plan_3d::Mesh::triangle_count)
        .sum()
}

fn layer(sim: &Sim) -> plan_core::details::DetailsLayer {
    dv::load(&sim.app.cx)
}

/// The linear length reported for `category`, inches.
fn trim_length(sim: &Sim, category: &str) -> f64 {
    trim_takeoff(&sim.app.cx.project)
        .iter()
        .filter(|l| l.category == category)
        .map(|l| l.length)
        .sum()
}

fn inner_perimeter(sim: &mut Sim) -> f64 {
    let rooms = sim.cx().rooms_now().to_vec();
    assert_eq!(rooms.len(), 1);
    let mut ring = rooms[0].inner_polygon.clone();
    let first = ring[0];
    ring.push(first);
    path_length(&ring)
}

/// Adds a closed crown-like polyline beside the house (a 4 x 4 in section
/// is too small for the CAD Polyline tool's click spacing at plan zoom, so
/// the points go in directly) and selects it.
fn draw_profile_polyline(sim: &mut Sim) -> Id {
    let points = [
        (400.0, 20.0),
        (404.0, 20.0),
        (404.0, 22.0),
        (402.0, 24.0),
        (400.0, 24.0),
    ]
    .iter()
    .map(|&(x, y)| Point::new(x, y))
    .collect();
    let cx = sim.cx();
    let id = cx.project.alloc_id();
    let fl = cx.floor;
    cx.project.floors[fl].cad.push(CadObject {
        id,
        layer: plan_core::cad::DEFAULT_CAD_LAYER.to_string(),
        item: CadItem::Polyline {
            points,
            closed: true,
        },
    });
    cx.mark_dirty();
    cx.selection.set(ObjectRef::Cad(id));
    id
}

fn custom(sim: &mut Sim, id: &'static str) {
    sim.action(Action::Custom(id));
}

#[test]
fn a_crown_profile_goes_from_a_polyline_to_a_room_and_to_an_editable_polyline() {
    let mut sim = house();
    let before_trim = trim_triangles(&scene(&sim));

    // 1. A closed polyline becomes a profile of the plan's library.
    draw_profile_polyline(&mut sim);
    custom(&mut sim, mold::ADD_PROFILE);
    let profile = mold::active_profile();
    assert_eq!(profile.name, "Molding Profile");
    assert!(layer(&sim).profile("Molding Profile").is_some());
    assert!(
        (profile.width() - 4.0).abs() < 1e-6 && (profile.height() - 4.0).abs() < 1e-6,
        "{:?} {:?}",
        profile.bounds(),
        profile.parts
    );
    // One undo step takes it out of the library again.
    assert_eq!(sim.undo().as_deref(), Some("Add to Library"));
    assert!(layer(&sim).profile("Molding Profile").is_none());
    sim.redo();
    assert!(layer(&sim).profile("Molding Profile").is_some());

    // 2. The room takes it as a crown through its Moldings table.
    let mut table = MoldingTable::single(profile.clone());
    table.rows[0].kind = MoldingType::Crown;
    mold::set_room_molding_table(sim.cx(), ANCHOR, table.clone());
    sim.app.cx.refresh();
    let after_room = trim_triangles(&scene(&sim));
    assert!(after_room > before_trim, "{after_room} {before_trim}");
    let perimeter = inner_perimeter(&mut sim);
    let reported = trim_length(&sim, INTERIOR_TRIM);
    assert!(
        (reported - perimeter).abs() < 1.0,
        "{reported} vs {perimeter}"
    );
    // The crown hangs from the ceiling.
    let crown_top = scene(&sim)
        .meshes
        .iter()
        .filter(|m| m.material == Material::Trim)
        .flat_map(|m| m.vertices.iter().map(|v| f64::from(v.position[1])))
        .fold(f64::MIN, f64::max);
    assert!(crown_top > 100.0, "{crown_top}");

    // 3. Make Room Molding Polyline: the moldings are editable objects now
    // and the room stops generating them.
    sim.cx().cursor_world = Some(ANCHOR);
    custom(&mut sim, mold::MAKE_ROOM_POLYLINE);
    let l = layer(&sim);
    assert_eq!(l.moldings.len(), 1, "one closed run around the room");
    let id = l.moldings[0].id;
    assert!(l.moldings[0].is_closed());
    assert!(!l.moldings[0].automatic);
    assert!(l.room_moldings_at(ANCHOR).unwrap().table.rows.is_empty());
    sim.app.cx.refresh();
    assert_eq!(
        trim_triangles(&scene(&sim)),
        after_room,
        "same moldings, now objects"
    );
    assert!((trim_length(&sim, INTERIOR_TRIM) - perimeter).abs() < 1.0);
    assert_eq!(sim.undo().as_deref(), Some("Make Room Molding Polyline"));
    assert!(layer(&sim).moldings.is_empty());
    assert_eq!(
        layer(&sim)
            .room_moldings_at(ANCHOR)
            .unwrap()
            .table
            .rows
            .len(),
        1
    );
    sim.redo();
    assert_eq!(layer(&sim).moldings.len(), 1);

    // 4. Remove Molding from Selected Edge: one edge comes off.
    sim.cx().selection.set(ObjectRef::Detail(id));
    mold::select_edge(id, 1);
    let edge_len = layer(&sim).molding(id).unwrap().edge_length_3d(1);
    // (The Edit toolbar hook is in the integration queue; the buttons are
    // built by `tools::molding::edit_actions`.)
    let buttons: Vec<&str> = mold::edit_actions(&sim.app.cx)
        .iter()
        .map(|a| a.label)
        .collect();
    assert!(
        buttons.contains(&"Remove Molding from Selected Edge"),
        "{buttons:?}"
    );
    custom(&mut sim, mold::REMOVE_EDGE);
    assert!(!layer(&sim).molding(id).unwrap().edge_on(1));
    assert!((trim_length(&sim, INTERIOR_TRIM) - (perimeter - edge_len)).abs() < 1.0);
    // The removed edge has no molding in 3D: fewer triangles than before.
    sim.app.cx.refresh();
    assert!(trim_triangles(&scene(&sim)) < after_room);
    assert_eq!(
        sim.undo().as_deref(),
        Some("Remove Molding from Selected Edge")
    );
    assert!(layer(&sim).molding(id).unwrap().edge_on(1));

    // 5. Replace From Library: the polyline takes another profile.
    let base = builtin_profiles()
        .into_iter()
        .find(|p| p.kind == MoldingType::Base)
        .unwrap();
    mold::set_active_profile(base.clone());
    custom(&mut sim, mold::REPLACE_FROM_LIBRARY);
    assert_eq!(
        layer(&sim).molding(id).unwrap().table.rows[0].profile.name,
        base.name
    );
    assert_eq!(sim.undo().as_deref(), Some("Replace Moldings"));
    assert_eq!(
        layer(&sim).molding(id).unwrap().table.rows[0].profile.name,
        "Molding Profile"
    );

    // 6. Reverse Direction flips the side the profile lies on.
    let first = layer(&sim).molding(id).unwrap().polyline[0];
    custom(&mut sim, mold::REVERSE_DIRECTION);
    let flipped = layer(&sim).molding(id).unwrap().polyline.clone();
    assert!(flipped.len() > 2);
    assert!(flipped[flipped.len() - 1].dist(first) < 1e-6);
    assert_eq!(sim.undo().as_deref(), Some("Reverse Direction"));
}

#[test]
fn the_molding_polyline_tool_drags_a_closed_clockwise_path_with_the_active_profile() {
    let mut sim = house();
    // The default profile is the square one.
    sim.tool(ToolId::DetailsVariant(DetailsVariant::MoldingPolyline));
    let r = sim.drag((20.0, 20.0), (100.0, 80.0));
    assert_eq!(r.commit.as_deref(), Some("Molding Polyline"));
    let l = layer(&sim);
    assert_eq!(l.moldings.len(), 1);
    let m = &l.moldings[0];
    assert!(m.is_closed());
    assert_eq!(m.side, MoldingSide::Right);
    assert_eq!(
        m.table.rows[0].profile.name,
        plan_core::moldings::SQUARE_PROFILE
    );
    // Clockwise: the profile is inside the rectangle in 3D.
    sim.app.cx.refresh();
    let s = scene(&sim);
    let (lo, hi) = bounds_of(&s, m.id);
    assert!(lo[0] >= 19.99 && hi[0] <= 100.01, "{lo:?} {hi:?}");
    // The profile picked last is remembered between uses.
    let crown = builtin_profiles()
        .into_iter()
        .find(|p| p.kind == MoldingType::Crown)
        .unwrap();
    mold::set_active_profile(crown.clone());
    sim.drag((120.0, 20.0), (200.0, 80.0));
    let l = layer(&sim);
    assert_eq!(l.moldings.len(), 2);
    assert_eq!(l.moldings[1].table.rows[0].profile.name, crown.name);
    // A crown starts at the ceiling.
    assert!(
        l.moldings[1].elevation > 80.0,
        "{}",
        l.moldings[1].elevation
    );
    assert_eq!(sim.undo().as_deref(), Some("Molding Polyline"));
    assert_eq!(layer(&sim).moldings.len(), 1);
    assert_eq!(sim.undo().as_deref(), Some("Molding Polyline"));
    assert!(layer(&sim).moldings.is_empty());
    mold::reset_active_profile();
}

fn bounds_of(scene: &Scene, id: Id) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for m in scene.meshes.iter().filter(|m| m.object_id == Some(id)) {
        for v in &m.vertices {
            for k in 0..3 {
                lo[k] = lo[k].min(v.position[k]);
                hi[k] = hi[k].max(v.position[k]);
            }
        }
    }
    (lo, hi)
}

#[test]
fn the_molding_line_has_its_own_start_and_end_heights() {
    let mut sim = house();
    sim.tool(ToolId::DetailsVariant(DetailsVariant::MoldingLine));
    let r = sim.click(30.0, 40.0);
    assert!(r.commit.is_none());
    let r = sim.click(130.0, 40.0);
    assert_eq!(r.commit.as_deref(), Some("Molding Line"));
    let id = layer(&sim).moldings[0].id;
    // Heights come from the Selected Line panel; here straight on the model.
    dv::edit(sim.cx(), "Molding Line", |l| {
        let m = l.molding_mut(id).unwrap();
        m.set_vertex_bottom(0, 10.0);
        m.set_vertex_bottom(1, 50.0);
    });
    let m = layer(&sim).molding(id).unwrap().clone();
    assert!(m.is_sloped());
    assert!((m.edge_length_3d(0) - (100.0f64.powi(2) + 40.0f64.powi(2)).sqrt()).abs() < 1e-6);
    let s = scene(&sim);
    let (lo, hi) = bounds_of(&s, id);
    assert!(lo[1] < 11.0 && hi[1] > 50.0, "{lo:?} {hi:?}");
    // The 3D length is what the Materials List reports.
    let len = trim_length(&sim, INTERIOR_TRIM);
    assert!((len - m.edge_length_3d(0)).abs() < 1e-6);
    assert_eq!(sim.undo().as_deref(), Some("Molding Line"));
}

#[test]
fn replace_moldings_gives_the_clicked_molding_the_active_profile() {
    let mut sim = house();
    sim.tool(ToolId::DetailsVariant(DetailsVariant::MoldingLine));
    sim.click(30.0, 40.0);
    sim.click(130.0, 40.0);
    let id = layer(&sim).moldings[0].id;
    let base = builtin_profiles()
        .into_iter()
        .find(|p| p.kind == MoldingType::Base)
        .unwrap();
    mold::set_active_profile(base.clone());
    sim.tool(ToolId::DetailsVariant(DetailsVariant::ReplaceMoldings));
    // A click away from any molding replaces nothing.
    let none = sim.click(200.0, 150.0);
    assert!(none.commit.is_none());
    let r = sim.click(80.0, 40.0);
    assert_eq!(r.commit.as_deref(), Some("Replace Moldings"));
    let m = layer(&sim).molding(id).unwrap().clone();
    assert_eq!(m.table.rows[0].profile.name, base.name);
    assert_eq!(sim.undo().as_deref(), Some("Replace Moldings"));
    mold::reset_active_profile();
}

fn l_house() -> Sim {
    mold::reset_active_profile();
    let mut sim = Sim::new();
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 96.0),
        Point::new(120.0, 96.0),
        Point::new(120.0, 192.0),
        Point::new(0.0, 192.0),
    ];
    for i in 0..pts.len() {
        sim.cx().project.add_wall(
            0,
            pts[i],
            pts[(i + 1) % pts.len()],
            6.0,
            96.0,
            WallKind::Exterior,
        );
    }
    sim.app.cx.refresh();
    sim
}

#[test]
fn auto_placed_corner_trim_takes_inside_corners_when_asked() {
    let mut sim = l_house();
    sim.tool(ToolId::DetailsVariant(DetailsVariant::AutoCornerBoards));
    let r = sim.click(60.0, 60.0);
    assert_eq!(r.commit.as_deref(), Some("Auto Place Corner Boards"));
    // Five outside corners and no inside one.
    assert_eq!(layer(&sim).corner_boards.len(), 5);
    assert!(layer(&sim).corner_boards.iter().all(|b| !b.inside));
    assert_eq!(sim.undo().as_deref(), Some("Auto Place Corner Boards"));
    // Include Inside Corners adds the notch.
    assert!(mold::set_include_inside(sim.cx(), true));
    let r = sim.click(60.0, 60.0);
    assert_eq!(r.commit.as_deref(), Some("Auto Place Corner Boards"));
    let boards = layer(&sim).corner_boards;
    assert_eq!(boards.len(), 6);
    assert_eq!(boards.iter().filter(|b| b.inside).count(), 1);
    // Quoins follow the same rule.
    sim.tool(ToolId::DetailsVariant(DetailsVariant::AutoQuoins));
    sim.click(60.0, 60.0);
    let quoins = layer(&sim).quoins;
    assert_eq!(quoins.len(), 6);
    assert_eq!(quoins.iter().filter(|q| q.inside).count(), 1);
    // The Materials List reports corner boards and quoins as exterior trim.
    let exterior = trim_length(&sim, EXTERIOR_TRIM);
    assert!(exterior > 6.0 * 96.0 * 2.0 - 1.0, "{exterior}");
    assert!(trim_takeoff(&sim.app.cx.project)
        .iter()
        .any(|l| l.item == "Quoin" && l.count > 0));
    assert_eq!(sim.undo().as_deref(), Some("Auto Place Quoins"));
}

#[test]
fn corner_trim_holds_set_top_and_bottom_and_recesses_to_the_sheathing() {
    let mut sim = l_house();
    sim.tool(ToolId::DetailsVariant(DetailsVariant::AutoCornerBoards));
    sim.click(60.0, 60.0);
    let id = layer(&sim).corner_boards[0].id;
    let plain = layer(&sim).corner_boards[0].outline();
    dv::edit(sim.cx(), "Corner Board Specification", |l| {
        let b = l.corner_board_mut(id).unwrap();
        b.recessed = true;
        b.set_top = true;
        b.height = 60.0;
    });
    let b = layer(&sim).corner_boards[0].clone();
    assert!(
        b.outline()[0].dist(plain[0]) > 0.5,
        "recessed boards sit back"
    );
    // The walls grow; the board that holds its top stays, the others follow.
    for w in sim.cx().floor_mut().walls.iter_mut() {
        w.height = 120.0;
    }
    let rooms = sim.cx().rooms_now().to_vec();
    let floor = sim.cx().floor().clone();
    let mut l = layer(&sim);
    assert!(l.refresh_trim_heights(&floor, &rooms));
    assert!((l.corner_board(id).unwrap().height - 60.0).abs() < 1e-9);
    assert!(l
        .corner_boards
        .iter()
        .filter(|b| b.id != id)
        .all(|b| (b.height - 120.0).abs() < 1e-9));
    let _ = DetailRef::CornerBoard(id);
}

#[test]
fn the_exterior_room_gets_a_molding_polyline_around_the_outer_faces() {
    let mut sim = house();
    custom(&mut sim, mold::MAKE_EXTERIOR_POLYLINE);
    let l = layer(&sim);
    assert_eq!(l.moldings.len(), 1);
    let m = &l.moldings[0];
    assert!(m.is_closed());
    // The ring runs on the outer faces: longer than the room's own outline.
    let ring_len = path_length(&m.polyline);
    let inner = inner_perimeter(&mut sim);
    assert!(ring_len > inner + 8.0, "{ring_len} vs {inner}");
    // The profile projects outward and the molding hangs at the ceiling.
    assert!(m.elevation > 80.0, "{}", m.elevation);
    sim.app.cx.refresh();
    let s = scene(&sim);
    let (lo, hi) = bounds_of(&s, m.id);
    let (ring_lo, ring_hi) = plan_core::details::bounds(&m.polyline);
    assert!(
        lo[0] <= ring_lo.x as f32 + 0.01 && hi[0] >= ring_hi.x as f32 - 0.01,
        "{lo:?} {hi:?} {ring_lo:?} {ring_hi:?}"
    );
    // Outward: the mesh reaches past the outer faces by the profile's width.
    assert!(
        hi[0] > ring_hi.x as f32 + 0.5 || lo[0] < ring_lo.x as f32 - 0.5,
        "{lo:?} {hi:?}"
    );
    assert_eq!(sim.undo().as_deref(), Some("Make Room Molding Polyline"));
    assert!(layer(&sim).moldings.is_empty());
}

#[test]
fn room_types_floors_and_rooms_set_moldings_in_that_order() {
    use plan_core::moldings::default_table;
    let mut sim = house();
    let crown = builtin_profiles()
        .into_iter()
        .find(|p| p.kind == MoldingType::Crown)
        .unwrap();
    let base = builtin_profiles()
        .into_iter()
        .find(|p| p.kind == MoldingType::Base)
        .unwrap();
    // The Floor Defaults give every room a crown...
    mold::set_floor_molding_table(sim.cx(), MoldingTable::single(crown.clone()));
    sim.app.cx.refresh();
    let floor_only = trim_triangles(&scene(&sim));
    assert!(floor_only > 0);
    // ...the room type changes that to a base...
    mold::set_type_molding_table(sim.cx(), "Den", MoldingTable::single(base.clone()));
    let l = layer(&sim);
    assert_eq!(default_table(&l, "Den").rows[0].profile.name, base.name);
    assert_eq!(
        default_table(&l, "Kitchen").rows[0].profile.name,
        crown.name
    );
    // ...and the room itself can take both.
    let mut own = MoldingTable::single(base);
    own.add_new(crown);
    mold::set_room_molding_table(sim.cx(), ANCHOR, own);
    sim.app.cx.refresh();
    assert!(trim_triangles(&scene(&sim)) > floor_only);
    // Per-edge molding: switch the first edge of the room off.
    let perimeter = trim_length(&sim, INTERIOR_TRIM);
    mold::set_room_edge(sim.cx(), ANCHOR, 0, false);
    let after = trim_length(&sim, INTERIOR_TRIM);
    assert!(after < perimeter - 100.0, "{after} vs {perimeter}");
    assert_eq!(sim.undo().as_deref(), Some("Room Molding Edge"));
    // A wall that suppresses room moldings does the same.
    let wall = sim.cx().floor().walls[0].id;
    mold::set_wall_suppresses_moldings(sim.cx(), wall, true);
    assert!(trim_length(&sim, INTERIOR_TRIM) < perimeter - 100.0);
    assert_eq!(sim.undo().as_deref(), Some("Suppress Room Moldings"));
    // Each of the hand-offs was one undo step.
    assert_eq!(sim.undo().as_deref(), Some("Room Moldings"));
    assert_eq!(sim.undo().as_deref(), Some("Room Type Moldings"));
    assert_eq!(sim.undo().as_deref(), Some("Floor Defaults Moldings"));
    assert!(layer(&sim).floor_moldings.is_empty());
}
