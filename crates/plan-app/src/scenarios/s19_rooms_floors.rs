//! Scenario 19: rooms and floors. A garage room that drops in 3D, Open Below,
//! Floor Defaults and the heights of what is built after them, Build New
//! Floor deriving only the exterior walls, Reference Display and snapping to
//! the floor below, a closet nested in a room, a dragged room label and the
//! Structure Define thickness (R-11, R-23, R-26, R-28, R-40, R-44, R-56..R-59,
//! R-65).

use super::{draw_shell, Sim};
use crate::dialogs::floor::FloorDialog;
use crate::dialogs::floor_defaults::{FloorDefaultsDialog, FloorDefaultsTarget};
use crate::dialogs::reference_display::{self, ReferenceFloor, ReferenceSettings};
use crate::dialogs::room::{Define, RoomDialog};
use crate::editor::snap::SnapKind;
use crate::editor::{rooms_edit, ObjectRef};
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::toolbar::{Action, ViewFlag};
use crate::tools::ToolId;
use plan_3d::{Material, Mesh};
use plan_core::extras::StructureLayer;
use plan_core::geometry::Point;
use plan_core::{Id, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn interior() -> ToolId {
    ToolId::Wall {
        kind: WallKind::Interior,
    }
}

/// Shell plus one full-depth partition at x = 240: a west and an east room.
fn two_room_house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(interior());
    sim.drag((240.0, 0.0), (240.0, H + 1.0));
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    assert_eq!(sim.app.cx.rooms.len(), 2);
    sim
}

/// Opens the Room Specification of the room at `p`, lets `f` edit the draft
/// and applies it, the way OK does.
fn edit_room(sim: &mut Sim, p: Point, f: impl FnOnce(&mut RoomDialog)) {
    sim.app.cx.refresh();
    let idx = rooms_edit::room_index_at(&sim.app.cx, p).expect("a room there");
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    let mut d = RoomDialog::new(init);
    f(&mut d);
    assert!(!d.has_error());
    let draft = d.room_name().clone();
    assert!(rooms_edit::apply_room_spec(
        &mut sim.app.cx,
        d.room_index(),
        &draft,
        d.extras()
    ));
    assert_eq!(sim.app.cx.undo_label(), Some("Room Specification"));
}

fn scene(sim: &Sim) -> plan_3d::Scene {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
}

/// Heights of the `material` vertices with plan x in `[x0, x1]`.
fn heights(sim: &Sim, material: Material, x0: f32, x1: f32) -> Vec<f32> {
    scene(sim)
        .meshes
        .into_iter()
        .filter(|m| m.material == material)
        .flat_map(|m| m.vertices)
        .filter(|v| v.position[0] >= x0 && v.position[0] <= x1)
        .map(|v| v.position[1])
        .collect()
}

/// Plan x ranges of the west and east rooms of `two_room_house` (platform
/// vertices lie on the room outlines, so a range must include an outline).
const WEST: (f32, f32) = (-5.0, 10.0);
const EAST: (f32, f32) = (300.0, 500.0);
const ALL: (f32, f32) = (-5.0, 500.0);

fn top(sim: &Sim, m: Material, x0: f32, x1: f32) -> f32 {
    heights(sim, m, x0, x1).into_iter().fold(f32::MIN, f32::max)
}

fn bottom(sim: &Sim, m: Material, x0: f32, x1: f32) -> f32 {
    heights(sim, m, x0, x1).into_iter().fold(f32::MAX, f32::min)
}

/// Area of the upward-facing triangles of `material` meshes at heights in
/// `[y0, y1]`.
fn top_area(sim: &Sim, material: Material, y0: f32, y1: f32) -> f64 {
    let mut a = 0.0;
    for m in scene(sim).meshes.iter().filter(|m| m.material == material) {
        for t in m.indices.chunks(3) {
            let v = |i: u32| m.vertices[i as usize];
            let (p, q, r) = (v(t[0]), v(t[1]), v(t[2]));
            if p.normal[1] < 0.5 || p.position[1] < y0 || p.position[1] > y1 {
                continue;
            }
            let (ux, uz) = (q.position[0] - p.position[0], q.position[2] - p.position[2]);
            let (vx, vz) = (r.position[0] - p.position[0], r.position[2] - p.position[2]);
            a += f64::from((ux * vz - uz * vx).abs()) * 0.5;
        }
    }
    a
}

#[test]
fn a_room_typed_garage_drops_its_floor_24_inches_in_3d_and_gets_a_slab() {
    let mut sim = two_room_house();
    let house_floor = top(&sim, Material::Floor, WEST.0, WEST.1);
    let east_before = top(&sim, Material::Floor, EAST.0, EAST.1);
    assert!((house_floor - east_before).abs() < 0.01);

    edit_room(&mut sim, Point::new(360.0, 180.0), |d| {
        d.set_room_type("Garage")
    });
    // The dialog brought the Garage function's platform along.
    let name = sim.app.cx.floor().room_names.last().unwrap().clone();
    assert_eq!(name.room_type, "Garage");
    assert_eq!(name.floor_height_offset, -24.0);

    let dropped = top(&sim, Material::Floor, EAST.0, EAST.1);
    assert!(
        (dropped - (house_floor - 24.0)).abs() < 1.0,
        "garage floor {dropped} vs house {house_floor}"
    );
    // A slab under it, 4" thick: it is concrete, not framing.
    let low = bottom(&sim, Material::Floor, EAST.0, EAST.1);
    assert!((dropped - low - 4.0).abs() < 0.5, "{dropped} - {low}");
    // The house side did not move.
    assert!((top(&sim, Material::Floor, WEST.0, WEST.1) - house_floor).abs() < 0.01);
    // R-26: concrete stem walls along the dropped room (the 3D view rebuilds).
    assert!(
        scene(&sim)
            .meshes
            .iter()
            .any(|m| m.material == Material::Concrete),
        "no stem walls under the garage"
    );
    // One undo and the floor is level again.
    sim.undo();
    assert!((top(&sim, Material::Floor, EAST.0, EAST.1) - house_floor).abs() < 0.01);
}

#[test]
fn an_open_below_room_has_no_floor_and_opens_the_ceiling_under_it() {
    let mut sim = two_room_house();
    // Build New Floor with every wall so the second floor has two rooms too.
    sim.action(Action::BuildNewFloor);
    sim.ok();
    assert_eq!(sim.app.cx.floor, 1);
    sim.tool(interior());
    sim.drag((240.0, 0.0), (240.0, H + 1.0));
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    assert_eq!(sim.app.cx.rooms.len(), 2);
    let e = sim.app.cx.project.floors[1].elevation as f32;
    let floor1 = |sim: &Sim| top_area(sim, Material::Floor, e - 1.0, e + 2.0);
    let ceiling0 = |sim: &Sim| top_area(sim, Material::Ceiling, 100.0, 130.0);
    let (f1, c0) = (floor1(&sim), ceiling0(&sim));
    assert!(f1 > 100_000.0 && c0 > 100_000.0, "{f1} {c0}");

    edit_room(&mut sim, Point::new(120.0, 180.0), |d| {
        d.set_room_type("Open Below")
    });
    let room_area = sim.app.cx.rooms
        [rooms_edit::room_index_at(&sim.app.cx, Point::new(120.0, 180.0)).unwrap()]
    .area_sq_in;
    let f1_after = floor1(&sim);
    assert!(
        (f1 - f1_after - room_area).abs() < room_area * 0.1,
        "floor platform loses the room: {f1} -> {f1_after} (room {room_area})"
    );
    let c0_after = ceiling0(&sim);
    assert!(
        (c0 - c0_after - room_area).abs() < room_area * 0.1,
        "the ceiling below opens: {c0} -> {c0_after}"
    );
}

#[test]
fn floor_defaults_set_the_height_of_everything_built_after_them() {
    let mut sim = two_room_house();
    let first_ceiling = sim.app.cx.floor().ceiling_height;
    // Edit > Default Settings > Floors and Rooms > Floor Defaults opens.
    sim.action(Action::PlanFloorDefaults);
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.cancel();
    // The floor's own Floor Defaults: ceiling 10', then OK is one undo step.
    let mut settings = sim.app.cx.floor().settings.clone();
    settings.floor_structure_thickness = 12.0;
    let mut d = FloorDefaultsDialog::new(
        FloorDefaultsTarget::ThisFloor("1st Floor".into()),
        120.0,
        settings.clone(),
        vec![],
    );
    d.set_ceiling_height(120.0);
    FloorDialog::Defaults(Box::new(d)).apply(&mut sim.app.cx);
    assert_eq!(sim.app.cx.undo_label(), Some("Floor Defaults"));
    assert_eq!(sim.app.cx.floor().ceiling_height, 120.0);
    // Walls at the old ceiling height followed.
    assert!(sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .all(|w| (w.height - 120.0).abs() < 1e-9));
    let ceil = bottom(&sim, Material::Ceiling, ALL.0, ALL.1);
    assert!((ceil - 120.0).abs() < 0.5, "ceiling platform at {ceil}");

    // As the plan default too: the next floor starts with 120" and a 12" platform.
    let mut plan = FloorDefaultsDialog::new(
        FloorDefaultsTarget::PlanDefaults,
        120.0,
        settings.clone(),
        vec![],
    );
    plan.set_ceiling_height(120.0);
    FloorDialog::Defaults(Box::new(plan)).apply(&mut sim.app.cx);
    sim.action(Action::BuildNewFloor);
    sim.ok();
    assert_eq!(sim.app.cx.floor, 1);
    assert_eq!(sim.app.cx.floor().ceiling_height, 120.0);
    let want = sim.app.cx.project.floors[0].ceiling_height + 12.0;
    assert!(
        (sim.app.cx.floor().elevation - want).abs() < 1e-6,
        "{}",
        sim.app.cx.floor().elevation
    );
    // A wall drawn there starts with its own tool default (W-6), the floor
    // setting is what rooms and the ceiling platform use.
    sim.tool(interior());
    sim.drag((100.0, 100.0), (300.0, 100.0));
    let w = sim.app.cx.floor().walls.last().unwrap();
    assert_eq!(w.height, sim.app.cx.defaults.interior_wall.height);
    let _ = first_ceiling;
    // The room on the new floor has the floor's ceiling height in the dialog.
    sim.app.cx.refresh();
    let idx = rooms_edit::room_index_at(&sim.app.cx, Point::new(240.0, 300.0)).unwrap();
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    assert_eq!(init.floor_ceiling_height, 120.0);
}

#[test]
fn build_new_floor_derive_exterior_only_leaves_the_partition_downstairs() {
    let mut sim = two_room_house();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Door);
    sim.click(240.0, 180.0);
    let doors = sim.app.cx.floor().openings.len();
    assert!(doors >= 2, "an exterior and an interior door, got {doors}");
    sim.tool(ToolId::Select);
    sim.action(Action::BuildNewFloor);
    sim.ok();
    let cx = &sim.app.cx;
    assert_eq!(cx.floor, 1);
    // R-59: Derive from exterior walls: every exterior wall of the floor
    // below (the partition split two of them at its tees), no interior wall,
    // and only the exterior door.
    let below_exterior = cx.project.floors[0]
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior)
        .count();
    assert_eq!(cx.floor().walls.len(), below_exterior);
    assert!(cx
        .floor()
        .walls
        .iter()
        .all(|w| w.kind == WallKind::Exterior));
    assert_eq!(cx.floor().openings.len(), 1);
    // One room upstairs, two downstairs.
    assert_eq!(cx.rooms.len(), 1);
    assert!(cx.project.floors[0]
        .walls
        .iter()
        .any(|w| w.kind == WallKind::Interior));
    assert_eq!(cx.project.floors[0].openings.len(), doors);
    sim.undo();
    assert_eq!(sim.app.cx.project.floors.len(), 1);
}

#[test]
fn reference_display_shows_the_floor_below_and_the_pointer_snaps_to_its_walls() {
    reference_display::reset_settings();
    // A free-standing wall on the first floor, with ends nothing else has.
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(interior());
    sim.drag((150.0, 100.0), (330.0, 100.0));
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    sim.action(Action::BuildNewFloor);
    sim.ok();
    assert_eq!(sim.app.cx.floor, 1);
    let below = sim.app.cx.project.floors[0]
        .walls
        .iter()
        .find(|w| w.kind == WallKind::Interior)
        .unwrap()
        .clone();
    let raw = Point::new(below.end.x + 3.0, below.end.y + 4.0);
    // Reference Display off: nothing of the floor below is there to find.
    sim.app.cx.view_flags.remove(&ViewFlag::ReferenceDisplay);
    assert!(reference_display::reference_walls(&sim.app.cx).is_empty());
    let off = sim.app.cx.snap_at(raw, None, false, &[]);
    assert!(off.point.dist(below.end) > 0.5, "{off:?}");
    // On, the floor below: its walls draw dimmed and their ends snap.
    sim.app.cx.view_flags.insert(ViewFlag::ReferenceDisplay);
    let walls = reference_display::reference_walls(&sim.app.cx);
    assert_eq!(walls.len(), 5, "all of floor 1's walls are the reference");
    let on = sim.app.cx.snap_at(raw, None, false, &[]);
    assert!(on.point.dist(below.end) < 1e-6, "{on:?}");
    assert_eq!(on.kind, SnapKind::Endpoint);
    // The Wall tool starts exactly on it, and Alt turns the snap off.
    sim.tool(interior());
    sim.drag(
        (below.start.x + 2.0, below.start.y - 3.0),
        (below.start.x + 2.0, below.start.y + 120.0),
    );
    let w = sim.app.cx.floor().walls.last().unwrap().clone();
    assert!(
        w.start.dist(below.start) < 1e-6,
        "{:?} vs {:?}",
        w.start,
        below.start
    );
    let alt = sim.app.cx.snap_at(raw, None, true, &[]);
    assert_eq!(alt.point, raw);

    // Pointing the reference at the floor above shows nothing from here.
    reference_display::set_settings(ReferenceSettings {
        floor: ReferenceFloor::Above,
        ..ReferenceSettings::default()
    });
    assert!(reference_display::reference_walls(&sim.app.cx).is_empty());
    reference_display::reset_settings();
}

#[test]
fn a_closet_drawn_inside_a_room_is_its_own_room_and_leaves_the_area_around_it() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.app.cx.refresh();
    let whole = sim.app.cx.rooms[0].clone();
    // A 60 x 48 closet in the middle of the room, four interior walls.
    sim.tool(interior());
    sim.drag((200.0, 150.0), (260.0, 150.0));
    sim.drag((260.0, 150.0), (260.0, 198.0));
    sim.drag((260.0, 198.0), (200.0, 198.0));
    sim.drag((200.0, 198.0), (200.0, 150.0));
    sim.app.cx.refresh();
    assert_eq!(sim.app.cx.rooms.len(), 2, "the shell and the closet");
    let outer = sim
        .app
        .cx
        .rooms
        .iter()
        .max_by(|a, b| a.area_sq_in.total_cmp(&b.area_sq_in))
        .unwrap()
        .clone();
    let closet = sim
        .app
        .cx
        .rooms
        .iter()
        .min_by(|a, b| a.area_sq_in.total_cmp(&b.area_sq_in))
        .unwrap()
        .clone();
    assert_eq!(
        outer.holes.len(),
        1,
        "the closet is a hole in the room around it"
    );
    let t = sim.app.cx.wall_thickness(WallKind::Interior);
    // Closet interior: (60 - t) x (48 - t).
    let want = (60.0 - t) * (48.0 - t) / 144.0;
    assert!(
        (closet.interior_area_sq_ft() - want).abs() < 0.5,
        "closet {} vs {want}",
        closet.interior_area_sq_ft()
    );
    // The big room lost the closet's outer footprint (walls included).
    let lost = whole.interior_area_sq_ft() - outer.interior_area_sq_ft();
    let footprint = (60.0 + t) * (48.0 + t) / 144.0;
    assert!((lost - footprint).abs() < 1.0, "lost {lost} vs {footprint}");
    // A click in the closet selects the closet, in the hall the hall.
    sim.tool(ToolId::Select);
    sim.click(230.0, 174.0);
    let sel = rooms_edit::selected_room(&sim.app.cx).unwrap();
    assert!(sim.app.cx.rooms[sel].area_sq_in < outer.area_sq_in);
    sim.click(100.0, 100.0);
    let sel = rooms_edit::selected_room(&sim.app.cx).unwrap();
    assert!(sim.app.cx.rooms[sel].area_sq_in > 100_000.0);
    // Undoing the last closet wall merges everything back into one room.
    sim.undo();
    assert_eq!(sim.app.cx.rooms.len(), 1);
}

#[test]
fn dragging_a_room_label_moves_it_in_one_undo_step_and_it_survives_saving() {
    let mut sim = two_room_house();
    let p = Point::new(120.0, 180.0);
    let idx = rooms_edit::room_index_at(&sim.app.cx, p).unwrap();
    let before = rooms_edit::label_position(&sim.app.cx, &sim.app.cx.rooms[idx].clone());
    sim.tool(ToolId::Select);
    sim.drag((before.x, before.y), (before.x + 40.0, before.y - 30.0));
    let idx = rooms_edit::room_index_at(&sim.app.cx, p).unwrap();
    let after = rooms_edit::label_position(&sim.app.cx, &sim.app.cx.rooms[idx].clone());
    assert!(
        (after.x - before.x - 40.0).abs() < 2.0 && (after.y - before.y + 30.0).abs() < 2.0,
        "{before:?} -> {after:?}"
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Move Room Label"));
    // The room is the selection after the drag (the press picked it).
    assert_eq!(rooms_edit::selected_room(&sim.app.cx), Some(idx));
    // Saved with the plan.
    let json = sim.app.cx.project.to_json().unwrap();
    let back = plan_core::Project::from_json(&json).unwrap();
    assert!(back.floors[0]
        .room_names
        .iter()
        .any(|n| n.label.offset.dist(Point::new(40.0, -30.0)) < 2.0));
    // Undo puts the label back.
    sim.undo();
    let idx = rooms_edit::room_index_at(&sim.app.cx, p).unwrap();
    let again = rooms_edit::label_position(&sim.app.cx, &sim.app.cx.rooms[idx].clone());
    assert!(again.dist(before) < 1e-6);
    let _ = ObjectRef::Terrain;
}

#[test]
fn the_floor_structure_define_sets_the_platform_thickness_in_3d_and_survives_ok() {
    let mut sim = two_room_house();
    let p = Point::new(360.0, 180.0);
    let thickness = |sim: &Sim| {
        top(sim, Material::Floor, EAST.0, EAST.1) - bottom(sim, Material::Floor, EAST.0, EAST.1)
    };
    let default = thickness(&sim);
    edit_room(&mut sim, p, |d| {
        let stack = d.structure_mut(Define::Floor);
        stack.clear();
        stack.push(StructureLayer::new("Subfloor", 0.75));
        stack.push(StructureLayer::new("Joist", 9.25));
    });
    let thick = thickness(&sim);
    assert!(
        (thick - 10.0).abs() < 0.5,
        "a 10\" Floor Structure builds a 10\" platform, got {thick} (was {default})"
    );
    assert!(thick > default + 5.0);
    // Reopened, the Define stack is still there.
    sim.app.cx.refresh();
    let idx = rooms_edit::room_index_at(&sim.app.cx, p).unwrap();
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    let mut d = RoomDialog::new(init);
    assert_eq!(d.structure_mut(Define::Floor).len(), 2);
    // The ceiling's Define is independent.
    assert!(d.structure_mut(Define::Ceiling).is_empty());
    let _: Option<Id> = None;
    let _ = Mesh::triangle_count;
}
