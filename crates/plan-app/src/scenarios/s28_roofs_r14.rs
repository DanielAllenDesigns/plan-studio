//! Scenario 28: roofs, round 14. Roof Styles written to the walls, a wall's
//! roof directive applied from the selection, wings at their own plate
//! heights, pitch and rotate handles under Select, Edit All Roof Planes,
//! polygon roof holes and dormers dragged across planes (RF-3, RF-4, RF-9,
//! RF-24, RF-34, RF-36, RF-38, RF-39, RF-42, RF-50, RF-51).

use super::{draw_shell, Sim};
use crate::editor::handles::{handles_for, HandleKind};
use crate::editor::roof_view::{self, RoofSettings, RoofStyle};
use crate::editor::{EditActionKind, ObjectRef};
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;
use plan_3d::{Material, Scene};
use plan_core::defaults::RoofWallKind;
use plan_core::geometry::Point;
use plan_core::{Floor, Id, Project, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn build_roof(sim: &mut Sim) {
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
}

fn planes(sim: &Sim) -> Vec<roof_view::RoofPlaneRecord> {
    roof_view::load(sim.app.cx.floor()).planes
}

fn scene_of(p: &Project) -> Scene {
    build_view_scene(p, &ViewScope::default())
}

fn east_wall(sim: &Sim) -> Id {
    sim.app
        .cx
        .floor()
        .walls
        .iter()
        .max_by(|a, b| (a.start.x + a.end.x).total_cmp(&(b.start.x + b.end.x)))
        .unwrap()
        .id
}

fn roof_top(scene: &Scene) -> f32 {
    scene
        .meshes
        .iter()
        .filter(|m| m.material == Material::Roof)
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .fold(f32::MIN, f32::max)
}

#[test]
fn one_click_on_a_wall_sets_its_roof_directive_and_the_roof_follows() {
    let mut sim = house();
    build_roof(&mut sim);
    assert_eq!(planes(&sim).len(), 4);
    let east = east_wall(&sim);
    sim.app.cx.selection.set(ObjectRef::Wall(east));
    let labels: Vec<&str> = sim
        .app
        .cx
        .extra_edit_actions()
        .iter()
        .filter_map(|a| match a.kind {
            EditActionKind::Custom { id, label, .. } if id.starts_with("roof.wall.") => Some(label),
            _ => None,
        })
        .collect();
    assert_eq!(
        labels,
        vec![
            "Hip Wall",
            "Full Gable Wall",
            "High Shed/Gable Wall",
            "Knee Wall",
            "Dutch Gable Wall"
        ]
    );
    sim.app.cx.run_custom("roof.wall.gable");
    assert_eq!(
        sim.app.cx.floor().wall(east).unwrap().roof.kind,
        RoofWallKind::FullGable
    );
    // The roof was rebuilt with the east end a gable.
    assert_eq!(planes(&sim).len(), 3);
    // One undo step takes back the wall and the roof together.
    assert_eq!(sim.undo().as_deref(), Some("Full Gable Wall"));
    assert_eq!(planes(&sim).len(), 4);
    assert_eq!(
        sim.app.cx.floor().wall(east).unwrap().roof.kind,
        RoofWallKind::Hip
    );
}

#[test]
fn the_roof_directives_are_offered_for_a_selection_of_walls_not_for_nothing() {
    let mut sim = house();
    sim.app.cx.selection.clear();
    let none = sim.app.cx.extra_edit_actions().iter().any(
        |a| matches!(a.kind, EditActionKind::Custom { id, .. } if id.starts_with("roof.wall.")),
    );
    assert!(!none, "nothing selected, no roof buttons");
    let mut interior = Sim::new();
    interior.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    interior.drag((0.0, 0.0), (200.0, 1.0));
    let id = interior.app.cx.floor().walls[0].id;
    interior.app.cx.selection.set(ObjectRef::Wall(id));
    // R17-07 (DECISIONS RH6): Half Walls, Railings and interior Knee Walls
    // take a directive too.
    assert!(interior.app.cx.extra_edit_actions().iter().any(
        |a| matches!(a.kind, EditActionKind::Custom { id, .. } if id.starts_with("roof.wall."))
    ));
}

#[test]
fn every_roof_style_builds_a_different_roof_from_the_same_walls() {
    let mut counts = Vec::new();
    for style in RoofStyle::ALL {
        let mut sim = house();
        let fl = sim.app.cx.floor;
        let s = RoofSettings::from_defaults(&sim.app.cx.defaults);
        roof_view::apply_style(&mut sim.app.cx.project, fl, style, s.pitch).unwrap();
        roof_view::rebuild(&mut sim.app.cx.project, fl, s, false).unwrap();
        counts.push((style, planes(&sim).len()));
    }
    let n = |st: RoofStyle| counts.iter().find(|(s, _)| *s == st).unwrap().1;
    assert_eq!(n(RoofStyle::Hip), 4);
    assert_eq!(n(RoofStyle::Gable), 2);
    assert_eq!(n(RoofStyle::Shed), 1);
    assert_eq!(n(RoofStyle::Gambrel), 4);
    assert_eq!(n(RoofStyle::HalfHip), 4);
    assert!(n(RoofStyle::DutchGable) >= 4);
}

#[test]
fn a_half_hip_roof_has_a_gable_wall_that_stops_under_the_small_hip() {
    let mut sim = house();
    let fl = sim.app.cx.floor;
    let s = RoofSettings::from_defaults(&sim.app.cx.defaults);
    roof_view::apply_style(&mut sim.app.cx.project, fl, RoofStyle::Gable, s.pitch).unwrap();
    roof_view::rebuild(&mut sim.app.cx.project, fl, s.clone(), false).unwrap();
    let gable_top = roof_top(&scene_of(&sim.app.cx.project));
    roof_view::apply_style(&mut sim.app.cx.project, fl, RoofStyle::HalfHip, s.pitch).unwrap();
    roof_view::rebuild(&mut sim.app.cx.project, fl, s, false).unwrap();
    let scene = scene_of(&sim.app.cx.project);
    // The ridge is as high as the gable's and the east wall rises to the
    // clip, no higher.
    assert!((roof_top(&scene) - gable_top).abs() < 1.0);
    let east = east_wall(&sim);
    let wall_top = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(east))
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .fold(f32::MIN, f32::max);
    assert!(wall_top < gable_top - 5.0, "{wall_top} vs {gable_top}");
    assert!(wall_top > sim.app.cx.floor().walls[0].height as f32 + 10.0);
}

/// The house plus a lower garage on its east side.
fn with_garage(sim: &mut Sim, garage_h: f64) {
    // The garage hangs on the east wall's actual ends (drawn a few inches off).
    let east = sim
        .app
        .cx
        .floor()
        .wall(east_wall(sim))
        .cloned()
        .expect("east wall");
    let (a, b) = if east.start.y < east.end.y {
        (east.start, east.end)
    } else {
        (east.end, east.start)
    };
    let pts = [
        a,
        Point::new(a.x + 240.0, a.y),
        Point::new(a.x + 240.0, b.y),
        b,
    ];
    let p = &mut sim.app.cx.project;
    for i in 0..3 {
        p.add_wall(0, pts[i], pts[i + 1], 6.0, garage_h, WallKind::Exterior);
    }
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
}

#[test]
fn a_garage_with_lower_walls_gets_a_lower_roof_beside_the_main_roof() {
    let mut sim = house();
    with_garage(&mut sim, 96.0);
    build_roof(&mut sim);
    let set = planes(&sim);
    let eaves: Vec<f64> = set.iter().map(|r| r.baseline_height()).collect();
    let lo = eaves.iter().copied().fold(f64::MAX, f64::min);
    let hi = eaves.iter().copied().fold(f64::MIN, f64::max);
    let main_h = sim.app.cx.floor().walls[0].height;
    // The garage eaves sit as far below the main eaves as its walls are lower.
    assert!(
        (hi - lo - (main_h - 96.0)).abs() < 1.5,
        "{lo} {hi} {main_h}"
    );
    // In 3D the garage roof stays below the main ridge and the shared wall
    // rises to the main plate beside it.
    let scene = scene_of(&sim.app.cx.project);
    let garage_top = set
        .iter()
        .filter(|r| r.baseline_height() < lo + 1.0)
        .flat_map(|r| r.polygon3d.iter().map(|v| v[1]))
        .fold(f64::MIN, f64::max);
    assert!((garage_top as f32) < roof_top(&scene));
    // One undo takes the whole build back.
    assert_eq!(sim.undo().as_deref(), Some("Build Roof"));
    assert!(planes(&sim).is_empty());
}

#[test]
fn a_one_story_garage_beside_a_second_floor_is_roofed_on_the_first_floor() {
    let mut sim = house();
    with_garage(&mut sim, 96.0);
    let h = sim.app.cx.floor().walls[0].height;
    sim.app.cx.project.floors.push(Floor::new("2nd Floor", h));
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(W, 0.0),
        Point::new(W, H),
        Point::new(0.0, H),
    ];
    for i in 0..4 {
        sim.app
            .cx
            .project
            .add_wall(1, pts[i], pts[(i + 1) % 4], 6.0, h, WallKind::Exterior);
    }
    // The shared wall closes the main house's first floor.
    sim.app
        .cx
        .project
        .add_wall(0, pts[1], pts[2], 6.0, h, WallKind::Exterior);
    sim.app.cx.floor = 1;
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    build_roof(&mut sim);
    let top = roof_view::load(&sim.app.cx.project.floors[1]);
    let low = roof_view::load(&sim.app.cx.project.floors[0]);
    assert_eq!(top.planes.len(), 4);
    assert!(!low.planes.is_empty(), "the garage has a roof on floor 1");
    let eaves: Vec<f64> = low.planes.iter().map(|r| r.baseline_height()).collect();
    let (lo, hi) = (
        eaves.iter().copied().fold(f64::MAX, f64::min),
        eaves.iter().copied().fold(f64::MIN, f64::max),
    );
    assert!(hi - lo < 1.0, "one plate height: {eaves:?}");
    assert!(lo < top.planes[0].baseline_height() - 100.0);
}

#[test]
fn a_selected_roof_plane_shows_pitch_edge_and_rotate_handles_under_select() {
    let mut sim = house();
    build_roof(&mut sim);
    let id = planes(&sim)[0].id;
    sim.app.cx.selection.set(ObjectRef::RoofPlane(id));
    let hs = handles_for(&sim.app.cx, 1.0);
    let has = |f: &dyn Fn(HandleKind) -> bool| hs.iter().any(|h| f(h.kind));
    assert!(has(&|k| k == HandleKind::Move));
    assert!(has(&|k| matches!(k, HandleKind::Reshape(_))));
    assert!(has(&|k| matches!(k, HandleKind::EdgeMove(_))));
    assert!(has(&|k| k == HandleKind::Pitch));
    assert!(has(&|k| k == HandleKind::Rotate));
    // The pitch arrow sits up the slope from the centre.
    let rec = &planes(&sim)[0];
    let arrow = hs.iter().find(|h| h.kind == HandleKind::Pitch).unwrap();
    assert!(arrow.pos.sub(rec.centroid()).dot(rec.up_slope()) > 1.0);
    // Dragging it steepens the plane.
    let p = roof_view::apply_handle_drag(
        &mut sim.app.cx.project,
        0,
        id,
        crate::editor::handles::roof_plane_handle(HandleKind::Pitch).unwrap(),
        arrow.pos,
        arrow.pos.add(rec.up_slope().scale(12.0)),
    )
    .unwrap();
    assert!((p.record.pitch - (rec.pitch + 3.0)).abs() < 1e-9);
}

#[test]
fn edit_all_roof_planes_opens_for_a_roof_and_cancel_changes_nothing() {
    let mut sim = house();
    build_roof(&mut sim);
    let before = sim.app.cx.project.floors[0].roofs.clone();
    sim.tool(ToolId::RoofVariant(RoofMode::EditAll));
    sim.dialog_frame(false);
    sim.cancel();
    assert_eq!(sim.app.cx.project.floors[0].roofs, before);
    // Without a roof it explains itself instead of opening.
    let mut empty = house();
    empty.tool(ToolId::RoofVariant(RoofMode::EditAll));
    empty.dialog_frame(false);
    assert!(empty.app.cx.status.contains("no roof planes"));
}

#[test]
fn a_polygon_hole_is_drawn_with_clicks_and_is_open_in_3d() {
    let mut sim = house();
    build_roof(&mut sim);
    sim.tool(ToolId::RoofVariant(RoofMode::Hole));
    let corners = [
        (200.0, 30.0),
        (280.0, 30.0),
        (280.0, 70.0),
        (240.0, 70.0),
        (240.0, 100.0),
        (200.0, 100.0),
    ];
    for (x, y) in corners {
        sim.click(x, y);
    }
    let res = sim.double_click(200.0, 100.0);
    assert_eq!(res.commit.as_deref(), Some("Roof Hole"));
    let holes: Vec<_> = planes(&sim).into_iter().flat_map(|r| r.holes).collect();
    assert_eq!(holes.len(), 1);
    assert_eq!(holes[0].outline.len(), 6);
    assert_eq!(sim.undo().as_deref(), Some("Roof Hole"));
}

#[test]
fn the_dormer_dialog_places_a_dormer_and_dragging_it_moves_it_on_the_roof() {
    let mut sim = house();
    let fl = sim.app.cx.floor;
    // A gable roof makes two big planes.
    let s = RoofSettings::from_defaults(&sim.app.cx.defaults);
    roof_view::apply_style(&mut sim.app.cx.project, fl, RoofStyle::Gable, s.pitch).unwrap();
    roof_view::rebuild(&mut sim.app.cx.project, fl, s, false).unwrap();
    sim.tool(ToolId::RoofVariant(RoofMode::Dormer));
    sim.click(240.0, 20.0);
    sim.ok();
    sim.dialog_frame(false);
    let set = roof_view::load(sim.app.cx.floor());
    assert_eq!(set.dormers.len(), 1, "the dormer dialog's OK placed one");
    let before = set.dormers[0].spec.position_along_eave;
    // Drag it sideways in Edit mode.
    sim.tool(ToolId::RoofVariant(RoofMode::Edit));
    sim.drag((240.0, 20.0), (300.0, 20.0));
    let after = roof_view::load(sim.app.cx.floor()).dormers[0]
        .spec
        .position_along_eave;
    assert!((after - before - 60.0).abs() < 1.0, "{before} -> {after}");
}

#[test]
fn edit_all_roof_planes_is_a_tool_of_its_own_in_the_roof_flyout() {
    let mut sim = Sim::new();
    sim.tool(ToolId::RoofVariant(RoofMode::EditAll));
    assert_eq!(sim.app.tools.active().name(), "Edit All Roof Planes");
    // The flyout's Edit All entry starts that mode and Edit Roof Planes
    // keeps its own entry.
    let flyout = crate::toolbar::roof();
    let modes: Vec<RoofMode> = flyout
        .entries
        .iter()
        .filter_map(|i| match i.action {
            crate::toolbar::Action::SetTool(ToolId::RoofVariant(m)) => Some(m),
            _ => None,
        })
        .collect();
    assert!(modes.contains(&RoofMode::EditAll));
    assert!(modes.contains(&RoofMode::Edit));
}
