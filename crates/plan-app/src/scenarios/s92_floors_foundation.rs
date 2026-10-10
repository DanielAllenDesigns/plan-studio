//! Scenario 92: floor-level options, Foundation Defaults and build behavior,
//! the fireplace foundation (Round 16 brief 17; manual pp. 737-773).
//!
//! Build New Floor steps its elevations to match the floor below; Insert New
//! Floor goes below the current floor, derived from its walls; Floor 0 stays
//! while Auto Rebuild Foundation is on; a foundation built under rooms of
//! different floor heights steps, marks the steps with an "S" in the plan and
//! sits under a terrain 6 in below its stem wall tops; a garage gets its own
//! slab and a cutout sized from its door; a masonry fireplace on Floor 1 gets
//! a foundation block and one placed in an exterior wall faces the room; a
//! copied fireplace brings its specification; the Attic floor warns. Each
//! user action is one undo step.

use super::{draw_shell, Sim};
use crate::dialogs::floor::FloorDialog;
use crate::editor::fireplace_view as fv;
use crate::editor::foundation_view;
use crate::editor::rooms_edit::{self, FoundationSpec, FoundationType};
use crate::editor::site_view;
use crate::editor::ObjectRef;
use crate::tools::fireplace::FireplaceMode as M;
use crate::tools::ToolId;
use plan_core::floors::{DeriveFrom, FloorPlacement, FLOOR_PLATFORM_THICKNESS};
use plan_core::foundation::{step_markers, FoundationLayer};
use plan_core::geometry::Point;
use plan_core::{FloorKind, OpeningKind, OpeningStyle, RoomName, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;
const LEFT: Point = Point::new(120.0, 180.0);
const RIGHT: Point = Point::new(360.0, 180.0);

/// A shell with a partition down the middle and a named room on each side.
fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    let cx = sim.cx();
    cx.project.add_wall(
        0,
        Point::new(240.0, 0.0),
        Point::new(240.0, H + 1.0),
        4.5,
        109.125,
        WallKind::Interior,
    );
    cx.floor_mut()
        .room_names
        .push(RoomName::new(LEFT, "Den", "Den"));
    cx.floor_mut()
        .room_names
        .push(RoomName::new(RIGHT, "Garage", "Garage"));
    cx.mark_dirty();
    cx.refresh();
    sim
}

/// Runs `act` and checks it made exactly one undo step: undo goes back to
/// the step before it and redo brings it back.
fn one_step(sim: &mut Sim, act: impl FnOnce(&mut Sim)) {
    let before = sim.app.cx.undo_label().map(str::to_string);
    act(sim);
    assert_ne!(
        sim.app.cx.undo_label().map(str::to_string),
        before,
        "the action made no undo step"
    );
    sim.undo();
    assert_eq!(
        sim.app.cx.undo_label().map(str::to_string),
        before,
        "the action made more than one undo step"
    );
    sim.redo();
}

// ----- floor options -----

#[test]
fn build_new_floor_steps_its_elevations_to_the_floor_below_in_one_undo_step() {
    let mut sim = house();
    // A vaulted den on the first floor.
    sim.cx().floor_mut().room_names[0].ceiling_height = Some(109.125 + 24.0);
    let mut d = FloorDialog::new_floor(&sim.app.cx.project, 0, &sim.app.cx.defaults);
    if let FloorDialog::NewFloor { spec, .. } = &mut d {
        spec.derive = DeriveFrom::AllWalls;
        spec.step_elevations = true;
        spec.heights_from_defaults = false;
    }
    one_step(&mut sim, |sim| d.apply(sim.cx()));
    let cx = &sim.app.cx;
    assert_eq!(cx.project.floors.len(), 2, "the 1st and the new 2nd floor");
    let up = &cx.project.floors[1];
    assert_eq!(up.name, "2nd Floor");
    let stepped: Vec<_> = up
        .room_names
        .iter()
        .filter(|n| n.floor_height_offset > 1.0)
        .collect();
    assert_eq!(stepped.len(), 1, "{:?}", up.room_names);
    assert_eq!(stepped[0].floor_height_offset, 24.0);
}

#[test]
fn insert_new_floor_goes_below_the_current_floor_derived_from_its_walls() {
    let mut sim = house();
    let walls = sim.app.cx.floor().walls.len();
    let mut d = FloorDialog::insert_floor(&sim.app.cx.project, 0, &sim.app.cx.defaults);
    match &mut d {
        FloorDialog::NewFloor { spec, insert, .. } => {
            assert!(*insert);
            assert_eq!(spec.place, FloorPlacement::Below);
            spec.derive = DeriveFrom::AllWalls;
        }
        _ => panic!("the Insert New Floor dialog"),
    }
    one_step(&mut sim, |sim| d.apply(sim.cx()));
    let p = &sim.app.cx.project;
    assert_eq!(
        p.floors.len(),
        2,
        "the new 1st floor and the old one, now 2nd"
    );
    assert_eq!(
        p.floors[0].walls.len(),
        walls,
        "derived from the current floor's walls"
    );
    assert_eq!(p.floors[0].name, "1st Floor");
    assert_eq!(p.floors[1].name, "2nd Floor");
    assert_eq!(sim.app.cx.floor, 0, "the new floor is current");
}

#[test]
fn the_attic_floor_warns_when_walls_are_drawn_on_it_and_has_no_rooms() {
    let mut sim = house();
    let a = rooms_edit::build_attic_floor(sim.cx()).unwrap();
    assert!(foundation_view::attic_warning(sim.cx()).is_none());
    assert!(sim.app.cx.project.floors[a].room_names.is_empty());
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((60.0, 60.0), (200.0, 60.0));
    let w = foundation_view::attic_warning(sim.cx());
    assert!(w.is_some_and(|t| t.contains("not meant to be a living area")));
    assert!(sim.app.cx.status.contains("living area"));
    // It says so once per visit.
    assert!(foundation_view::attic_warning(sim.cx()).is_none());
}

// ----- foundations -----

fn build(sim: &mut Sim, f: impl FnOnce(&mut FoundationSpec)) {
    let mut spec = FoundationSpec::from_defaults(&sim.app.cx.defaults);
    spec.stem_height = 36.0;
    f(&mut spec);
    FloorDialog::foundation(spec).apply(sim.cx());
}

#[test]
fn a_foundation_on_a_sloped_terrain_steps_with_s_markers_and_sits_under_the_terrain() {
    let mut sim = house();
    // The den's floor is raised 24 in: a stepped foundation.
    sim.cx().floor_mut().room_names[0].floor_height_offset = 24.0;
    one_step(&mut sim, |sim| build(sim, |s| s.settings.s_markers = true));
    let cx = &sim.app.cx;
    assert_eq!(cx.project.floors[0].kind, FloorKind::Foundation);
    let marks = step_markers(&cx.project.floors[0]);
    assert!(marks.len() >= 2, "{marks:?}");
    assert!(marks.iter().all(|m| (m.high - m.low - 24.0).abs() < 1e-9));
    // The S shows in the plan of Floor 0 and not with the markers off.
    sim.cx().floor = 0;
    let has_s = |sim: &mut Sim| {
        sim.plan_shapes().iter().any(|s| match s {
            eframe::egui::Shape::Text(t) => t.galley.text() == "S",
            _ => false,
        })
    };
    assert!(has_s(&mut sim), "an S where the stem walls step");
    sim.cx().floor = 1;
    build(&mut sim, |s| s.settings.s_markers = false);
    sim.cx().floor = 0;
    assert!(!has_s(&mut sim));
    // A sloped terrain: the building pad sits 6 in under the stem wall tops.
    sim.cx().floor = 1;
    let mut rec = site_view::load_terrain(&sim.app.cx.project).unwrap_or_default();
    for (x, y, z) in [
        (-300.0, -300.0, 0.0),
        (780.0, -300.0, 60.0),
        (780.0, 660.0, 60.0),
        (-300.0, 660.0, 0.0),
    ] {
        rec.terrain
            .elevation_points
            .push(plan_terrain::ElevationPoint {
                pos: Point::new(x, y),
                z,
            });
    }
    site_view::save_terrain(&mut sim.app.cx.project, &rec);
    assert!(site_view::auto_building_pad(sim.cx()));
    let rec = site_view::load_terrain(&sim.app.cx.project).unwrap();
    let platform = FLOOR_PLATFORM_THICKNESS;
    assert!(
        (rec.terrain.building_pad_elevation + platform + 6.0).abs() < 1e-9,
        "{}",
        rec.terrain.building_pad_elevation
    );
}

#[test]
fn a_garage_door_leaves_a_curb_cutout_as_wide_as_its_rough_opening_and_concrete_cutout() {
    let mut sim = house();
    let right_wall = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .find(|w| {
            w.kind == WallKind::Exterior
                && (w.start.y - w.end.y).abs() < 10.0
                && w.start.x.min(w.end.x) < 5.0
        })
        .map(|w| w.id)
        .expect("the bottom wall");
    let door = sim
        .cx()
        .project
        .add_opening(0, right_wall, 360.0, OpeningKind::Door)
        .unwrap();
    {
        let o = sim
            .cx()
            .floor_mut()
            .openings
            .iter_mut()
            .find(|o| o.id == door)
            .unwrap();
        o.style = OpeningStyle::Garage;
        o.width = 108.0;
        o.extras.spec.rough.add_width = 4.0;
        o.extras.spec.rough.concrete_each_side = 6.0;
    }
    // Monolithic Slab: curbs; Walls with Footings: stem walls.
    for (kind, label) in [
        (FoundationType::MonolithicSlab, "curb"),
        (FoundationType::WallsWithFootings, "stem wall"),
    ] {
        build(&mut sim, |s| s.kind = kind);
        let f0 = &sim.app.cx.project.floors[0];
        assert_eq!(f0.openings.len(), 1, "a cutout in the {label}");
        assert_eq!(f0.openings[0].width, 112.0 + 12.0, "{label}");
        assert!(f0.walls.iter().any(|w| w.id == f0.openings[0].wall_id));
        // The garage floor was lowered and takes its floor from below.
        let g = &sim.app.cx.project.floors[1].room_names[1];
        assert!(g.floor_height_offset < 0.0 && g.options.floor_from_foundation);
        assert_eq!(
            FoundationLayer::load(f0).slabs.len(),
            if kind == FoundationType::MonolithicSlab {
                2
            } else {
                1
            }
        );
    }
}

#[test]
fn floor_zero_cannot_be_deleted_while_auto_rebuild_foundation_is_on() {
    let mut sim = house();
    build(&mut sim, |s| s.settings.auto_rebuild = true);
    assert!(
        !foundation_view::auto_rebuild(sim.cx()),
        "nothing changed since the build"
    );
    assert!(!rooms_edit::delete_foundation(sim.cx()));
    assert!(sim.app.cx.status.contains("Auto Rebuild Foundation"));
    assert_eq!(sim.app.cx.project.floors.len(), 2);
    // Floor 0 is closed to hand editing too.
    sim.cx().floor = 0;
    assert!(foundation_view::locked(&sim.app.cx));
    sim.tool(ToolId::FoundationVariant(
        crate::tools::foundation::FoundationVariant::Slab,
    ));
    let before = FoundationLayer::load(sim.app.cx.floor()).slabs.len();
    sim.drag((10.0, 10.0), (100.0, 100.0));
    assert_eq!(
        FoundationLayer::load(sim.app.cx.floor()).slabs.len(),
        before
    );
    // Floor 1 changes: the foundation follows, with no step of its own.
    sim.cx().floor = 1;
    let walls = sim.app.cx.project.floors[0].walls.len();
    let id = sim.app.cx.project.floors[1].walls[0].id;
    sim.cx().project.floors[1].wall_mut(id).unwrap().start = Point::new(-60.0, 0.0);
    assert!(foundation_view::auto_rebuild(sim.cx()));
    assert_eq!(sim.app.cx.project.floors[0].walls.len(), walls);
    assert!(sim.app.cx.project.floors[0]
        .walls
        .iter()
        .any(|w| w.start.x == -60.0));
}

// ----- fireplaces -----

#[test]
fn a_fireplace_in_an_exterior_wall_faces_the_interior_whichever_side_is_clicked() {
    let mut sim = house();
    sim.tool(ToolId::FireplaceVariant(M::InWall));
    // The pointer is outside the bottom wall (y < 0).
    let r = sim.click(100.0, -10.0);
    assert!(r.commit.is_some(), "{r:?}");
    let ObjectRef::Symbol(id) = sim.app.cx.selection.single().unwrap() else {
        panic!("a fireplace");
    };
    let (sym, fp) = fv::load(sim.app.cx.floor(), id).unwrap();
    assert!(fp.in_wall);
    // The front faces +y, into the room, although the click was outside.
    assert!(sym.angle.abs() < 2.0, "{}", sym.angle);
    sim.undo();
    assert!(sim.app.cx.floor().fireplaces.is_empty());
    sim.redo();
    assert_eq!(sim.app.cx.floor().fireplaces.len(), 1);
    // On the partition (an interior wall) it faces the edge clicked.
    sim.tool(ToolId::FireplaceVariant(M::InWall));
    let r = sim.click(246.0, 200.0);
    assert!(r.commit.is_some());
    let ObjectRef::Symbol(id) = sim.app.cx.selection.single().unwrap() else {
        panic!("a fireplace");
    };
    let (sym, _) = fv::load(sim.app.cx.floor(), id).unwrap();
    // The front of a fireplace at angle a points along (-sin a, cos a): +x,
    // into the right-hand room.
    assert!(-sym.angle.to_radians().sin() > 0.99, "{}", sym.angle);
}

#[test]
fn the_depth_handle_slides_a_built_in_fireplace_out_until_its_front_meets_the_inside_edge() {
    let mut sim = house();
    sim.tool(ToolId::FireplaceVariant(M::InWall));
    sim.click(100.0, 20.0);
    let ObjectRef::Symbol(id) = sim.app.cx.selection.single().unwrap() else {
        panic!("a fireplace");
    };
    let (sym, _) = fv::load(sim.app.cx.floor(), id).unwrap();
    // Back flush with the outside face of the 6 in wall; the front stands out
    // into the room.
    let front0 = sym.position.y + sym.depth;
    assert!(front0 > 3.0 + 10.0, "{front0}");
    // Dragging far toward the outside stops at the inside edge of the wall.
    let slid = fv::slide_in_wall(
        sim.app.cx.floor(),
        &sym,
        Point::new(100.0, front0),
        Point::new(100.0, -400.0),
    )
    .unwrap();
    let wall = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .find(|w| w.start.y.abs() < 5.0 && w.end.y.abs() < 5.0)
        .unwrap();
    let inside_edge = wall.thickness * 0.5 + (wall.start.y + wall.end.y) * 0.5;
    assert!(
        (slid.position.y + slid.depth - inside_edge).abs() < 3.0,
        "front {} vs inside edge {inside_edge}",
        slid.position.y + slid.depth
    );
    // A free fireplace is not slid: it resizes like any symbol.
    sim.tool(ToolId::FireplaceVariant(M::Masonry));
    sim.click(300.0, 250.0);
    let ObjectRef::Symbol(free) = sim.app.cx.selection.single().unwrap() else {
        panic!("a fireplace");
    };
    let sym = sim.app.cx.floor().symbol(free).unwrap().clone();
    assert!(
        fv::slide_in_wall(sim.app.cx.floor(), &sym, Point::ZERO, Point::new(0.0, 10.0)).is_none()
    );
}

#[test]
fn a_masonry_fireplace_on_floor_one_gets_a_foundation_block_in_one_undo_step() {
    let mut sim = house();
    sim.tool(ToolId::FireplaceVariant(M::Masonry));
    sim.click(100.0, 20.0);
    let ObjectRef::Symbol(id) = sim.app.cx.selection.single().unwrap() else {
        panic!("a fireplace");
    };
    one_step(&mut sim, |sim| build(sim, |_| {}));
    let f0 = &sim.app.cx.project.floors[0];
    assert_eq!(f0.fireplaces.len(), 1);
    assert_eq!(f0.fireplaces[0].base_of, Some(id));
    assert!(f0.fireplaces[0].no_firebox);
    sim.undo();
    assert_eq!(sim.app.cx.project.floors.len(), 1);
}

#[test]
fn a_copied_fireplace_brings_its_specification() {
    let mut sim = house();
    sim.tool(ToolId::FireplaceVariant(M::Masonry));
    sim.click(100.0, 20.0);
    let ObjectRef::Symbol(id) = sim.app.cx.selection.single().unwrap() else {
        panic!("a fireplace");
    };
    // A specification the default would not give.
    let (sym, mut fp) = fv::load(sim.app.cx.floor(), id).unwrap();
    fp.name = "Great Room Hearth".into();
    fp.firebox.width = 30.0;
    fp.suppress_dimensions = true;
    assert!(fv::apply(sim.cx(), &sym, &fp));
    let clip = crate::editor::Clipboard::capture(sim.cx());
    assert!(!clip.is_empty());
    sim.cx().begin_change("Paste");
    let pasted = clip.paste(sim.cx(), Point::new(200.0, 0.0), false);
    sim.cx().mark_dirty();
    assert_eq!(pasted.len(), 1);
    let f = sim.app.cx.floor();
    assert_eq!(f.fireplaces.len(), 2);
    let copy = f.fireplaces.iter().find(|c| c.id != id).unwrap();
    assert_eq!(copy.name, "Great Room Hearth");
    assert_eq!(copy.firebox.width, 30.0);
    assert!(copy.suppress_dimensions);
    assert!(f.symbol(copy.id).is_some());
}

#[test]
fn the_fireplace_draws_over_the_wall_and_shows_its_width_dimensions() {
    use eframe::egui::Shape;
    let mut sim = house();
    sim.tool(ToolId::FireplaceVariant(M::Masonry));
    sim.click(100.0, 20.0);
    let texts = |sim: &mut Sim| -> Vec<String> {
        sim.plan_shapes()
            .iter()
            .filter_map(|s| match s {
                Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    };
    let with = texts(&mut sim);
    // The overall width (6') and the firebox width (3').
    assert!(with.iter().any(|t| t == "6'-0\""), "{with:?}");
    assert!(with.iter().any(|t| t == "3'-0\""), "{with:?}");
    let ObjectRef::Symbol(id) = sim.app.cx.selection.single().unwrap() else {
        panic!("a fireplace");
    };
    let (sym, mut fp) = fv::load(sim.app.cx.floor(), id).unwrap();
    fp.suppress_dimensions = true;
    assert!(fv::apply(sim.cx(), &sym, &fp));
    let without = texts(&mut sim);
    assert!(!without.iter().any(|t| t == "6'-0\""), "{without:?}");
}
