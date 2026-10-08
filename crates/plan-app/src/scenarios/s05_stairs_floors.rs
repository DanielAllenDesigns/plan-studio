//! Scenario 5: a stair, Build New Floor, the stairwell and the floor
//! up / down commands (CB-22..CB-30, R-55, R-59, R-67).

use super::{draw_shell, Sim};
use crate::editor::stairs_view::{self, StairCommand, StairKind};
use crate::editor::{EditorRequest, ObjectRef};
use crate::toolbar::Action;
use crate::tools::ToolId;
use plan_core::geometry::{point_in_polygon, Point};
use plan_core::WallKind;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

/// Build New Floor through its dialog: the menu action, then OK.
fn build_new_floor(sim: &mut Sim) {
    sim.action(Action::BuildNewFloor);
    sim.ok();
}

fn stairs(sim: &Sim, fl: usize) -> Vec<stairs_view::StairObj> {
    stairs_view::load(&sim.app.cx.project.floors[fl])
}

#[test]
fn build_new_floor_derives_the_exterior_walls_through_its_dialog() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    assert_eq!(sim.app.cx.project.floors.len(), 1);
    sim.action(Action::BuildNewFloor);
    // The dialog is up but nothing has happened yet.
    assert_eq!(sim.app.cx.project.floors.len(), 1);
    sim.ok();
    let cx = &sim.app.cx;
    assert_eq!(cx.project.floors.len(), 2);
    // R-59: the new floor is active, named like Chief names it, and stacked
    // on the first floor's platform.
    assert_eq!(cx.floor, 1);
    assert_eq!(cx.floor().name, "2nd Floor");
    assert!(
        cx.floor().elevation > cx.project.floors[0].elevation + cx.project.floors[0].ceiling_height
    );
    // Derive: the exterior walls (and their doors) came along, 4 of them.
    assert_eq!(cx.floor().walls.len(), 4);
    assert!(cx
        .floor()
        .walls
        .iter()
        .all(|w| w.kind == WallKind::Exterior));
    assert_eq!(cx.floor().openings.len(), 1);
    // New ids: the copies are not the first floor's walls.
    let first_ids: Vec<_> = cx.project.floors[0].walls.iter().map(|w| w.id).collect();
    assert!(cx.floor().walls.iter().all(|w| !first_ids.contains(&w.id)));
    // And the second floor has a room of its own.
    assert_eq!(sim.app.cx.rooms.len(), 1);
    // One undo step removes the floor and goes back to the first.
    assert_eq!(sim.undo().as_deref(), Some("Build New Floor"));
    assert_eq!(sim.app.cx.project.floors.len(), 1);
    assert_eq!(sim.app.cx.floor, 0);
    sim.redo();
    assert_eq!(sim.app.cx.project.floors.len(), 2);
}

#[test]
fn floor_up_and_down_switch_the_floor_and_keep_the_active_tool() {
    let mut sim = house();
    build_new_floor(&mut sim);
    assert_eq!(sim.app.cx.floor, 1);
    sim.tool(ToolId::Window);
    sim.action(Action::FloorDown);
    assert_eq!(sim.app.cx.floor, 0);
    // R-67: switching floors keeps the tool.
    assert_eq!(sim.app.tools.active_id(), ToolId::Window);
    sim.action(Action::FloorDown);
    assert_eq!(sim.app.cx.floor, 0, "no floor below the first");
    sim.action(Action::FloorUp);
    assert_eq!(sim.app.cx.floor, 1);
    sim.action(Action::FloorUp);
    assert_eq!(sim.app.cx.floor, 1, "no floor above the second");
    // Each floor's own walls show: the 2nd floor is not the first.
    assert_eq!(sim.app.cx.floor().name, "2nd Floor");
    sim.action(Action::FloorDown);
    assert_eq!(sim.app.cx.floor().name, "1st Floor");
}

#[test]
fn a_stair_is_drawn_by_dragging_and_solves_its_risers() {
    let mut sim = house();
    build_new_floor(&mut sim);
    sim.action(Action::FloorDown);
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    assert_eq!(sim.app.tools.active().name(), "Draw Stairs");
    // Drag from the bottom riser along +x for 150".
    let r = sim.drag((100.0, 100.0), (250.0, 100.0));
    assert_eq!(r.commit.as_deref(), Some("Draw Stairs"));
    let all = stairs(&sim, 0);
    assert_eq!(all.len(), 1);
    let o = &all[0];
    let sol = o.solution();
    // The rise is the floor-to-floor height up to the 2nd floor (CB-24).
    let rise = sim.app.cx.project.floors[1].elevation - sim.app.cx.project.floors[0].elevation;
    assert!(
        (o.stair.params.total_rise - rise).abs() < 1e-6,
        "{} vs {rise}",
        o.stair.params.total_rise
    );
    assert!(
        sol.risers >= 14 && sol.risers <= 18,
        "{} risers",
        sol.risers
    );
    let riser_h = sol.riser_height;
    assert!((6.0..=8.0).contains(&riser_h), "riser height {riser_h}");
    assert_eq!(o.stair.params.width, 36.0);
    assert!(
        o.stair.direction.abs() < 1e-9,
        "drag direction is the travel direction"
    );
    assert_eq!(
        sim.app.cx.selection.single(),
        Some(ObjectRef::Stair(o.id()))
    );
    // CB-29: the status offers the stairwell when the top lands in a room above.
    assert!(
        sim.app.cx.status.starts_with("Stairs:"),
        "{}",
        sim.app.cx.status
    );
    assert!(
        sim.app.cx.status.contains("Auto Stairwell"),
        "{}",
        sim.app.cx.status
    );
    // The stair goes with undo.
    assert_eq!(sim.undo().as_deref(), Some("Draw Stairs"));
    assert!(stairs(&sim, 0).is_empty());
}

#[test]
fn every_stair_flyout_entry_places_a_stair_or_landing() {
    let mut sim = house();
    sim.tool(ToolId::Select);
    for (i, kind) in StairKind::ALL.iter().enumerate() {
        sim.tool(ToolId::StairsVariant(*kind));
        let y = 60.0 + i as f64 * 40.0;
        sim.drag((60.0, y), (200.0, y));
        sim.app.cx.selection.clear();
    }
    assert_eq!(stairs(&sim, 0).len(), StairKind::ALL.len());
}

#[test]
fn the_stair_specification_dialog_opens_and_adjusts_the_stair() {
    let mut sim = house();
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((100.0, 100.0), (250.0, 100.0));
    let id = stairs(&sim, 0)[0].id();
    sim.tool(ToolId::Select);
    sim.requests.clear();
    sim.double_click(175.0, 100.0);
    assert!(
        sim.requests
            .contains(&EditorRequest::OpenSpec(ObjectRef::Stair(id))),
        "{:?}",
        sim.requests
    );
    assert!(sim.app.spec.is_open());
    sim.ok();
    assert!(!sim.app.spec.is_open());
    assert_eq!(sim.app.cx.undo_label(), Some("Stair Specification"));
    // Widen it through the same dialog type and apply (one undo step).
    let mut d = crate::dialogs::stairs::StairDialog::new(stairs(&sim, 0)[0].clone());
    d.draft_mut().stair.params.width = 48.0;
    assert!(stairs_view::apply_edit(&mut sim.app.cx, d.draft()));
    assert_eq!(stairs(&sim, 0)[0].stair.params.width, 48.0);
    assert_eq!(sim.undo().as_deref(), Some("Stair Specification"));
    assert_eq!(stairs(&sim, 0)[0].stair.params.width, 36.0);
}

#[test]
fn auto_stairwell_adds_a_stairwell_room_on_the_floor_above() {
    let mut sim = house();
    build_new_floor(&mut sim);
    sim.action(Action::FloorDown);
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((100.0, 100.0), (250.0, 100.0));
    let id = stairs(&sim, 0)[0].id();
    // The Edit toolbar offers it.
    let cmds = stairs_view::edit_commands(&sim.app.cx);
    assert!(
        cmds.iter()
            .any(|(c, on)| *c == StairCommand::AutoStairwell && *on),
        "{cmds:?}"
    );
    let before = sim.app.cx.project.floors[1].walls.len();
    assert!(stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::AutoStairwell
    ));
    sim.app.cx.refresh();
    assert!(sim.app.cx.project.floors[1].walls.len() > before);
    assert_eq!(sim.app.cx.undo_label(), Some("Auto Stairwell"));
    // Running it twice is refused.
    assert!(!stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::AutoStairwell
    ));
    // On the floor above there is now a room called Stairwell (CB-30).
    sim.action(Action::FloorUp);
    sim.app.cx.refresh();
    let names: Vec<String> = sim
        .app
        .cx
        .rooms
        .clone()
        .iter()
        .map(|r| sim.app.cx.room_name(r))
        .collect();
    assert!(names.iter().any(|n| n == "Stairwell"), "{names:?}");
    let _ = id;
    // Undo removes the walls again.
    sim.action(Action::FloorDown);
    assert_eq!(sim.undo().as_deref(), Some("Auto Stairwell"));
    assert_eq!(sim.app.cx.project.floors[1].walls.len(), before);
}

/// CB-29: the stairwell is a hole in the upper floor platform: nothing of the
/// 2nd floor's floor slab covers the stair's footprint.
#[test]
fn the_stairwell_cuts_a_hole_in_the_upper_floor_slab_in_3d() {
    use crate::shell::view3d_panel::{build_view_scene, ViewScope};
    use plan_3d::Material;
    let mut sim = house();
    build_new_floor(&mut sim);
    sim.action(Action::FloorDown);
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((100.0, 100.0), (250.0, 100.0));
    let o = stairs(&sim, 0)[0].clone();
    assert!(stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::AutoStairwell
    ));
    let footprint = o.footprint();
    let centre = plan_core::geometry::polygon_centroid(&footprint);
    let second = sim.app.cx.project.floors[1].elevation;
    let scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    let mut covering = 0;
    for m in scene
        .meshes
        .iter()
        .filter(|m| m.material == Material::Floor)
    {
        for tri in m.indices.chunks(3) {
            let p: Vec<_> = tri
                .iter()
                .map(|i| m.vertices[*i as usize].position)
                .collect();
            let y = (p[0][1] + p[1][1] + p[2][1]) / 3.0;
            if (y as f64) < second {
                continue;
            }
            let cx = (p[0][0] + p[1][0] + p[2][0]) / 3.0;
            let cz = (p[0][2] + p[1][2] + p[2][2]) / 3.0;
            if point_in_polygon(Point::new(cx as f64, -cz as f64), &footprint) {
                covering += 1;
            }
        }
    }
    assert_eq!(
        covering, 0,
        "{covering} floor triangles of the 2nd floor cover the stairwell at {centre:?}"
    );
}

#[test]
fn deleting_the_floor_goes_through_a_confirmation_and_undoes() {
    let mut sim = house();
    build_new_floor(&mut sim);
    sim.action(Action::DeleteFloor);
    // Nothing is deleted until OK.
    assert_eq!(sim.app.cx.project.floors.len(), 2);
    sim.ok();
    assert_eq!(sim.app.cx.project.floors.len(), 1);
    assert_eq!(sim.undo().as_deref(), Some("Delete Current Floor"));
    assert_eq!(sim.app.cx.project.floors.len(), 2);
    // The only floor cannot be deleted.
    let mut solo = Sim::new();
    solo.action(Action::DeleteFloor);
    assert!(
        solo.app.cx.status.contains("only floor"),
        "{}",
        solo.app.cx.status
    );
}
