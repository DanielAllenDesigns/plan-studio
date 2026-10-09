//! Scenario 29: rooms, floors and foundations, round 14. Build Foundation
//! with a basement, a crawl space, a monolithic slab and grade beams on
//! piers; the attic floor and its rooms following Build Roof; rough ceiling
//! and finish thickness, moldings from the library, per-surface materials, a
//! monolithic slab room, the label style and position, the Room Types buttons
//! and the Open Below rule (R-18, R-25, R-27, R-31, R-34, R-36, R-38, R-46,
//! R-52, R-61, R-62, R-68).

use super::{draw_shell, Sim};
use crate::dialogs::default_lists::RoomTypesDialog;
use crate::dialogs::floor::FloorDialog;
use crate::dialogs::room::RoomDialog;
use crate::editor::roof_view::{self, RoofSettings};
use crate::editor::rooms_edit::{self, FoundationSpec, FoundationType, NewFloorSpec};
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::tools::ToolId;
use plan_3d::{Material, Mesh};
use plan_core::floors::FoundationRooms;
use plan_core::geometry::Point;
use plan_core::rooms::{LabelPlacement, RoomSlab};
use plan_core::{FloorKind, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    assert_eq!(sim.app.cx.rooms.len(), 1);
    sim
}

fn meshes(sim: &Sim, material: Material) -> Vec<Mesh> {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
        .meshes
        .into_iter()
        .filter(|m| m.material == material)
        .collect()
}

fn ys(ms: &[Mesh]) -> Vec<f32> {
    ms.iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .collect()
}

fn top(ms: &[Mesh]) -> f32 {
    ys(ms).into_iter().fold(f32::MIN, f32::max)
}

fn bottom(ms: &[Mesh]) -> f32 {
    ys(ms).into_iter().fold(f32::MAX, f32::min)
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

fn centre() -> Point {
    Point::new(W / 2.0, H / 2.0)
}

fn build(sim: &mut Sim, f: impl FnOnce(&mut FoundationSpec)) {
    let mut spec = FoundationSpec::from_defaults(&sim.app.cx.defaults);
    f(&mut spec);
    FloorDialog::foundation(spec).apply(&mut sim.app.cx);
    sim.app.cx.refresh();
}

#[test]
fn a_basement_foundation_has_walls_footings_a_slab_and_a_room_with_a_ceiling() {
    let mut sim = house();
    build(&mut sim, |s| {
        s.kind = FoundationType::WallsWithFootings;
        s.stem_height = 108.0;
        s.footing_width = 20.0;
        s.footing_depth = 10.0;
    });
    let cx = &sim.app.cx;
    assert_eq!(cx.project.floors[0].kind, FloorKind::Foundation);
    assert_eq!(cx.project.floors[0].room_names[0].room_type, "Basement");
    assert_eq!(cx.floor, 1, "the plan stays on the first floor");
    let platform = cx.project.floors[1].settings.floor_structure_thickness as f32;
    // A 4" slab on the ground.
    let low = |m: &Mesh| ys(std::slice::from_ref(m)).iter().all(|y| *y < -50.0);
    let slab: Vec<Mesh> = meshes(&sim, Material::Floor)
        .into_iter()
        .filter(low)
        .collect();
    assert!((top(&slab) - bottom(&slab) - 4.0).abs() < 0.01);
    // A ceiling from the underside of the first floor's platform.
    let under: Vec<Mesh> = meshes(&sim, Material::Ceiling)
        .into_iter()
        .filter(|m| ys(std::slice::from_ref(m)).iter().all(|y| *y < 0.5))
        .collect();
    assert!(
        (bottom(&under) + platform).abs() < 0.01,
        "{}",
        bottom(&under)
    );
    // Footings 10" under the 108" stem walls (measured from the underside of
    // the platform that bears on them, DECISIONS FF1).
    let concrete = meshes(&sim, Material::Concrete);
    assert!((bottom(&concrete) + 108.0 + platform + 10.0).abs() < 0.01);
    // Its Room Specification shows the type that the build added to the list.
    sim.app.cx.floor = 0;
    sim.app.cx.refresh();
    let init = rooms_edit::room_dialog_init(&sim.app.cx, 0).unwrap();
    assert_eq!(init.name.room_type, "Basement");
    // The dialog's default ceiling height is the clear height under the
    // first floor's platform, measured from the slab.
    assert!(
        (init.floor_ceiling_height - (108.0 - 4.0)).abs() < 0.01,
        "{}",
        init.floor_ceiling_height
    );
    assert!(init.types.iter().any(|t| t.name == "Basement"));
    assert!(
        !init.slab_allowed,
        "no monolithic flag on a foundation floor"
    );
    // One undo step takes the foundation away again.
    sim.app.cx.undo();
    assert_eq!(sim.app.cx.project.floors.len(), 1);
}

#[test]
fn a_crawl_space_foundation_has_no_slab_and_no_ceiling_of_its_own() {
    let mut sim = house();
    build(&mut sim, |s| s.stem_height = 36.0);
    assert_eq!(
        sim.app.cx.project.floors[0].room_names[0].room_type,
        "Crawl Space"
    );
    let low = |m: &Mesh| ys(std::slice::from_ref(m)).iter().all(|y| *y < -20.0);
    assert_eq!(
        meshes(&sim, Material::Floor)
            .into_iter()
            .filter(low)
            .count(),
        0
    );
    assert_eq!(
        meshes(&sim, Material::Ceiling)
            .into_iter()
            .filter(|m| ys(std::slice::from_ref(m)).iter().all(|y| *y < 0.5))
            .count(),
        0
    );
}

#[test]
fn a_monolithic_slab_foundation_builds_a_slab_of_the_chosen_thickness() {
    let mut sim = house();
    build(&mut sim, |s| {
        s.kind = FoundationType::MonolithicSlab;
        s.slab_thickness = 6.0;
        s.slab_stem_height = 18.0;
    });
    let layer = plan_core::foundation::FoundationLayer::load(&sim.app.cx.project.floors[0]);
    assert_eq!(layer.slabs[0].thickness, 6.0);
    assert!(sim.app.cx.project.floors[0]
        .walls
        .iter()
        .all(|w| w.height == 18.0));
    // The slab is in the 3D view and fits inside the stem wall.
    let concrete = meshes(&sim, Material::Concrete);
    assert!(!concrete.is_empty());
    assert!((bottom(&concrete) + 18.0).abs() < 6.0);
}

#[test]
fn grade_beams_on_piers_put_piers_at_the_corners_and_along_the_walls() {
    let mut sim = house();
    build(&mut sim, |s| {
        s.kind = FoundationType::Piers;
        s.pier_spacing = 96.0;
        s.pier_height = 24.0;
        s.beam_height = 18.0;
    });
    let f = &sim.app.cx.project.floors[0];
    let layer = plan_core::foundation::FoundationLayer::load(f);
    // 480" and 360" walls at 96" spacing: about 5 + 4 + 5 + 4 piers.
    assert!(layer.piers.len() >= 16, "{}", layer.piers.len());
    assert!(f
        .walls
        .iter()
        .all(|w| w.bottom_offset == 24.0 && w.height == 18.0));
    // The platform bears on the grade beams, so Floor 0 is a platform taller
    // than pier plus beam (DECISIONS FF1).
    let platform = sim.app.cx.project.floors[1].settings.floor_structure_thickness;
    assert_eq!(f.elevation, -(42.0 + platform));
}

#[test]
fn the_attic_floor_follows_build_roof() {
    let mut sim = house();
    // Second floor with an attic floor above it.
    rooms_edit::build_new_floor_with(
        &mut sim.app.cx,
        &NewFloorSpec {
            attic: true,
            ..NewFloorSpec::new()
        },
    )
    .unwrap();
    let cx = &mut sim.app.cx;
    assert_eq!(cx.project.floors.len(), 3);
    let attic = &cx.project.floors[2];
    assert_eq!(attic.kind, FloorKind::Attic);
    assert_eq!(attic.walls.len(), 4);
    // Rooms cannot be created on the Attic floor (manual p. 773).
    assert!(attic.room_names.is_empty());
    // Stretch the second floor's east wall and rebuild the roof: the attic
    // walls and rooms are made again from the new outline.
    let east = cx.project.floors[1]
        .walls
        .iter()
        .position(|w| w.start.x > W - 5.0 && w.end.x > W - 5.0)
        .expect("east wall");
    for w in cx.project.floors[1].walls.iter_mut() {
        for p in [&mut w.start, &mut w.end] {
            if p.x > W - 5.0 {
                p.x += 60.0;
            }
        }
    }
    let _ = east;
    let settings = RoofSettings::from_defaults(&cx.defaults);
    roof_view::rebuild(&mut cx.project, 1, settings, false).unwrap();
    let attic = &cx.project.floors[2];
    assert!(
        attic.walls.iter().any(|w| w.start.x > W + 50.0),
        "the attic walls moved with the wall below"
    );
    assert!(attic.room_names.is_empty());
    assert!(attic.walls.iter().all(|w| w.flags.attic));
    // Without an attic floor Build Roof makes none.
    let mut plain = house();
    rooms_edit::build_new_floor(&mut plain.app.cx, true);
    let settings = RoofSettings::from_defaults(&plain.app.cx.defaults);
    roof_view::rebuild(&mut plain.app.cx.project, 1, settings, false).unwrap();
    assert!(plain
        .app
        .cx
        .project
        .floors
        .iter()
        .all(|f| f.kind != FloorKind::Attic));
}

#[test]
fn rough_ceiling_and_finish_thickness_shape_the_ceiling_platform() {
    let mut sim = house();
    let plain = meshes(&sim, Material::Ceiling);
    let ceiling = sim.app.cx.floor().ceiling_height as f32;
    edit_room(&mut sim, centre(), |d| {
        d.extras_mut().ceiling_finish_thickness = 2.0;
    });
    let ceil = meshes(&sim, Material::Ceiling);
    assert!((bottom(&ceil) - ceiling).abs() < 0.01);
    assert!(
        top(&ceil) > top(&plain) + 0.9,
        "the finish lifts the framing"
    );
    // A rough ceiling 12" up puts the framing there.
    edit_room(&mut sim, centre(), |d| {
        d.room_name_mut().rough_ceiling = Some(ceiling as f64 + 12.0);
    });
    let ceil = meshes(&sim, Material::Ceiling);
    assert!(
        (top(&ceil) - (ceiling + 12.0 + 1.0)).abs() < 0.01,
        "{}",
        top(&ceil)
    );
}

#[test]
fn moldings_chosen_in_the_dialog_are_built_and_undone() {
    let mut sim = house();
    let before = meshes(&sim, Material::Trim).len();
    edit_room(&mut sim, centre(), |d| {
        d.extras_mut().base_molding = "Base - Colonial 5 1/4".into();
        d.extras_mut().chair_molding = "Chair Rail - Simple 2 1/2".into();
        d.extras_mut().crown_molding = "Crown - Stepped 6".into();
    });
    assert_eq!(meshes(&sim, Material::Trim).len(), before + 3);
    let n = &sim.app.cx.floor().room_names[0];
    assert_eq!(n.moldings.len(), 3);
    sim.app.cx.undo();
    assert_eq!(meshes(&sim, Material::Trim).len(), before);
    sim.app.cx.redo();
    assert_eq!(meshes(&sim, Material::Trim).len(), before + 3);
}

#[test]
fn surface_materials_lay_plates_on_the_floor_ceiling_and_walls() {
    let mut sim = house();
    assert!(meshes(&sim, Material::Brick).is_empty());
    edit_room(&mut sim, centre(), |d| {
        d.extras_mut().wall_covering = "Brick".into();
        d.room_name_mut().floor_finish = Some("Ceramic Tile".into());
    });
    assert_eq!(meshes(&sim, Material::Brick).len(), 1);
    assert_eq!(meshes(&sim, Material::Stone).len(), 1);
    // A floor-wide default covers the other rooms.
    let mut other = house();
    let walls = meshes(&other, Material::Stucco).len();
    other.app.cx.project.floors[0].settings.wall_material = "Stucco".into();
    assert_eq!(meshes(&other, Material::Stucco).len(), walls + 1);
}

#[test]
fn a_monolithic_slab_room_has_a_thick_floor_and_a_thickened_edge() {
    let mut sim = house();
    edit_room(&mut sim, centre(), |d| {
        d.room_name_mut().monolithic_slab = Some(RoomSlab {
            thickness: 6.0,
            stem_height: 16.0,
        });
    });
    let floor = meshes(&sim, Material::Floor);
    assert!((top(&floor) - bottom(&floor) - 6.0).abs() < 0.01);
    let concrete = meshes(&sim, Material::Concrete);
    assert!((bottom(&concrete) + 16.0).abs() < 0.01);
}

#[test]
fn the_label_style_and_position_reach_the_plan() {
    let mut sim = house();
    let room = sim.app.cx.rooms[0].clone();
    let before = rooms_edit::label_position(&sim.app.cx, &room);
    edit_room(&mut sim, centre(), |d| {
        let style = &mut d.room_name_mut().label_style;
        style.placement = LabelPlacement::Top;
        style.text_style = "Schedule Style".into();
    });
    let room = sim.app.cx.rooms[0].clone();
    let after = rooms_edit::label_position(&sim.app.cx, &room);
    assert!(after.y > before.y + 50.0, "{after:?} vs {before:?}");
    assert!(room.contains(after));
    assert_eq!(
        rooms_edit::label_text_style(&sim.app.cx, &room),
        "Schedule Style"
    );
    // The label can still be picked where it sits now.
    assert_eq!(rooms_edit::label_at(&sim.app.cx, after), Some(0));
}

#[test]
fn the_total_living_area_shows_after_a_room_is_specified() {
    let mut sim = house();
    edit_room(&mut sim, centre(), |d| d.set_name("Den"));
    let s = &sim.app.cx.status;
    assert!(s.contains("Total living area"), "{s}");
    assert!(s.contains("sq ft"), "{s}");
}

#[test]
fn the_room_types_dialog_copies_selects_clears_and_deletes() {
    let sim = house();
    let mut d = RoomTypesDialog::new(&sim.app.cx.defaults);
    let n = d.types.types.len();
    let kitchen = d
        .types
        .types
        .iter()
        .position(|(_, t)| t.name == "Kitchen")
        .unwrap();
    d.copy_type(kitchen, "Galley Kitchen").unwrap();
    assert_eq!(d.types.types.len(), n + 1);
    d.select_all();
    assert!(d.checked.iter().all(|c| *c));
    d.clear_all();
    assert!(d.checked.iter().all(|c| !*c));
    let galley = d.types.types.len() - 1;
    d.checked[galley] = true;
    assert_eq!(d.delete_checked(), Ok(1));
    assert_eq!(d.types.types.len(), n);
}

/// A second floor with an Open Below room over the west of the first, and a
/// partition under the Open Below room's edge.
#[test]
fn open_below_opens_only_rooms_wholly_under_it() {
    let mut sim = house();
    // Partition at x = 240 on the first floor: a west and an east room.
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((240.0, 0.0), (240.0, H + 1.0));
    sim.tool(ToolId::Select);
    rooms_edit::build_new_floor(&mut sim.app.cx, true);
    // Second floor: a partition at x = 360 and its west room Open Below.
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((360.0, 0.0), (360.0, H + 1.0));
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    edit_room(&mut sim, Point::new(120.0, 180.0), |d| {
        d.set_room_type("Open Below")
    });
    let ceilings: Vec<Mesh> = meshes(&sim, Material::Ceiling)
        .into_iter()
        .filter(|m| ys(std::slice::from_ref(m)).iter().all(|y| *y < 150.0))
        .collect();
    let xs: Vec<f32> = ceilings
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[0]))
        .collect();
    assert!(!xs.is_empty(), "the east room keeps its ceiling");
    assert!(xs.iter().all(|x| *x > 230.0), "the west room is open");
    assert!(
        !xs.iter().any(|x| (*x - 360.0).abs() < 2.0),
        "the east ceiling is not cut where the Open Below room ends"
    );
}

#[test]
fn foundation_rooms_choices_cover_every_variant() {
    for rooms in [
        FoundationRooms::Auto,
        FoundationRooms::Basement,
        FoundationRooms::CrawlSpace,
        FoundationRooms::None,
    ] {
        let mut sim = house();
        build(&mut sim, |s| {
            s.stem_height = 100.0;
            s.rooms = rooms;
        });
        let names = &sim.app.cx.project.floors[0].room_names;
        match rooms {
            FoundationRooms::None => assert!(names.is_empty()),
            FoundationRooms::CrawlSpace => assert_eq!(names[0].room_type, "Crawl Space"),
            _ => assert_eq!(names[0].room_type, "Basement"),
        }
    }
}
