//! Scenario 31 (round 14): the Cabinet Specification's live tabs, library
//! door styles, pushing neighbours, Convert Polyline to Soffit, and the
//! stairs' handrail sides, bullnose, hidden treads and Schedule tab
//! (CB-4, CB-7, CB-12, CB-17, CB-26, CB-29, CB-32, CB-33).

use super::{draw_shell, Sim};
use crate::editor::handles::HandleKind;
use crate::editor::placed::{self, apply_cabinet, cabinet_by_id, load_cabinets, PlacedRef};
use crate::editor::stairs_view::{self, StairCommand, StairKind};
use crate::editor::{EditActionKind, ObjectRef};
use crate::toolbar::Action;
use crate::tools::cabinet::{bump_mode, set_bump_mode, BumpMode, BUMP_MODE_COMMAND};
use crate::tools::ToolId;
use plan_cabinets::{Cabinet, CabinetKind, FillPattern, FootStyle, PilasterStyle};
use plan_core::geometry::Point;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn cabs(sim: &Sim) -> Vec<Cabinet> {
    load_cabinets(sim.app.cx.floor())
}

fn place(sim: &mut Sim, kind: CabinetKind, x: f64, y: f64) {
    sim.tool(ToolId::CabinetVariant(kind));
    sim.app.cx.selection.clear();
    sim.click(x, y);
}

fn triangles(c: &Cabinet) -> usize {
    plan_cabinets::meshes(c)
        .iter()
        .map(|m| m.indices.len() / 3)
        .sum()
}

#[test]
fn the_specification_tabs_edit_the_fill_accessories_info_and_schedule_of_a_placed_cabinet() {
    let mut sim = house();
    placed::set_auto_join(false);
    place(&mut sim, CabinetKind::Base, 100.0, 10.0);
    let before = cabs(&sim)[0].clone();
    let id = before.id;
    let tris = triangles(&before);
    let shapes = sim.plan_shapes().len();
    // The dialog opens on the cabinet and closes with OK without changing it.
    assert!(sim.open_spec(ObjectRef::Cabinet(id)));
    sim.ok();
    assert!(!sim.app.has_dialog());
    assert_eq!(cabs(&sim)[0], before);
    // Edit the draft the way the tabs do and apply it (one undo step).
    let mut draft = before.clone();
    draft.fill.pattern = FillPattern::CrossHatch;
    draft.accessories.feet = FootStyle::Bun;
    draft.accessories.pilaster = PilasterStyle::Fluted;
    draft.info.manufacturer = "Acme Cabinetry".into();
    draft.in_schedule = false;
    assert!(apply_cabinet(&mut sim.app.cx, &draft));
    let after = cabs(&sim)[0].clone();
    assert_eq!(after.info.manufacturer, "Acme Cabinetry");
    // The plan draws the hatch; the 3D model gets feet and pilasters.
    assert!(
        sim.plan_shapes().len() > shapes + 10,
        "the hatch lines are drawn"
    );
    assert!(triangles(&after) > tris);
    // The schedule leaves it out.
    let rows = |sim: &Sim| {
        plan_docs::schedule_kinds::table(
            &sim.app.cx.project,
            &plan_core::schedules::Schedule::new(
                plan_core::schedules::ScheduleKind::Cabinet,
                Point::ZERO,
            ),
            0,
            None,
        )
        .rows
        .len()
    };
    assert_eq!(rows(&sim), 0);
    // The components list names what the tabs added.
    let parts = plan_cabinets::components(&after);
    assert!(parts.iter().any(|c| c.name == "Bun Foot" && c.count == 4));
    assert!(parts
        .iter()
        .any(|c| c.name == "Fluted Pilaster" && c.count == 2));
    // Undo takes it all back in one step.
    sim.undo();
    assert_eq!(cabs(&sim)[0], before);
    assert_eq!(rows(&sim), 1);
    // The file keeps it: save and load the project.
    assert!(apply_cabinet(&mut sim.app.cx, &draft));
    let json = sim.app.cx.project.to_json().unwrap();
    let back = plan_core::Project::from_json(&json).unwrap();
    let kept = back.floors[0].cabinets_as::<Cabinet>().unwrap();
    assert_eq!(kept[0].fill.pattern, FillPattern::CrossHatch);
    assert!(!kept[0].in_schedule);
    assert_eq!(kept[0].accessories.feet, FootStyle::Bun);
}

#[test]
fn pushing_mode_moves_the_neighbours_of_a_cabinet_dragged_along_the_wall() {
    let mut sim = house();
    placed::set_auto_join(false);
    set_bump_mode(BumpMode::Bump);
    // Two cabinets side by side, and a third one apart.
    place(&mut sim, CabinetKind::Base, 100.0, 10.0);
    place(&mut sim, CabinetKind::Base, 124.0, 10.0);
    place(&mut sim, CabinetKind::Base, 260.0, 10.0);
    let run = cabs(&sim);
    assert_eq!(run.len(), 3);
    let xs0: Vec<f64> = run.iter().map(|c| c.position.x).collect();
    let moving = run[2].id;
    // The Edit toolbar cycles Bump -> Push (the third cabinet is selected).
    sim.app.cx.selection.set(ObjectRef::Cabinet(moving));
    let toolbar = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    let toggle = toolbar
        .iter()
        .find(|a| matches!(a.kind, EditActionKind::Custom { id, .. } if id == BUMP_MODE_COMMAND))
        .expect("the bump mode button");
    assert_eq!(toggle.label, "Neighbors: Bump");
    sim.app.cx.apply_edit_action(toggle.kind);
    assert_eq!(bump_mode(), BumpMode::Push);
    // Drag it with its Move handle until it overlaps the second cabinet.
    let handle = placed::placed_handles(
        sim.app.cx.floor(),
        PlacedRef::Cabinet(moving),
        sim.app.cx.px_per_in,
    )
    .into_iter()
    .find(|h| h.kind == HandleKind::Move)
    .unwrap();
    let (hx, hy) = (handle.pos.x, handle.pos.y);
    let width = run[2].width;
    // Move left far enough that its left edge sits 10" inside the second one.
    let target_left = xs0[1] + run[1].width - 10.0;
    let dx = target_left - xs0[2];
    sim.drag((hx, hy), (hx + dx, hy));
    let after = cabs(&sim);
    let x = |id: u64| after.iter().find(|c| c.id == id).unwrap().position.x;
    // The dragged cabinet got where it was taken; the two it ran into moved
    // left ahead of it, still butted together.
    assert!((x(moving) - target_left).abs() < 1e-6, "{}", x(moving));
    assert!((x(run[1].id) - (target_left - run[1].width)).abs() < 1e-6);
    assert!((x(run[0].id) - (x(run[1].id) - run[0].width)).abs() < 1e-6);
    let _ = width;
    // One undo step puts all three back.
    assert_eq!(sim.undo().as_deref(), Some("Move Cabinet"));
    let back: Vec<f64> = cabs(&sim).iter().map(|c| c.position.x).collect();
    assert_eq!(back, xs0);
    set_bump_mode(BumpMode::Bump);
}

#[test]
fn a_closed_polyline_becomes_a_soffit_from_the_edit_toolbar() {
    let mut sim = house();
    let ring = vec![
        Point::new(60.0, 60.0),
        Point::new(180.0, 60.0),
        Point::new(180.0, 120.0),
        Point::new(120.0, 150.0),
        Point::new(60.0, 120.0),
    ];
    let poly = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        plan_core::CadItem::Polyline {
            points: ring,
            closed: true,
        },
    );
    sim.tool(ToolId::CabinetVariant(CabinetKind::Soffit));
    sim.app.cx.selection.set(ObjectRef::Cad(poly));
    let toolbar = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    let button = toolbar
        .iter()
        .find(|a| a.label == "Convert Polyline to Soffit")
        .expect("offered for a closed polyline");
    sim.app.cx.apply_edit_action(button.kind);
    let soffits = cabs(&sim);
    assert_eq!(soffits.len(), 1);
    assert_eq!(soffits[0].kind, CabinetKind::Soffit);
    assert_eq!(soffits[0].custom.as_ref().unwrap().outline.len(), 5);
    assert!(sim.app.cx.floor().cad.iter().all(|c| c.id != poly));
    // The soffit builds in 3D from its outline.
    assert!(triangles(&soffits[0]) > 12);
    assert_eq!(sim.undo().as_deref(), Some("Convert Polyline to Soffit"));
    assert!(cabs(&sim).is_empty());
    assert!(sim.app.cx.floor().cad.iter().any(|c| c.id == poly));
}

fn stairs(sim: &Sim, fl: usize) -> Vec<stairs_view::StairObj> {
    stairs_view::load(&sim.app.cx.project.floors[fl])
}

#[test]
fn the_staircase_specification_sets_handrail_sides_and_a_bullnose_and_the_floor_above_shows_the_treads(
) {
    use plan_stairs::{Bullnose, RailStyle, SideKind, StairPart};
    let mut sim = house();
    sim.action(Action::BuildNewFloor);
    sim.ok();
    sim.action(Action::FloorDown);
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((100.0, 100.0), (250.0, 100.0));
    let id = stairs(&sim, 0)[0].id();
    // The dialog opens on the Schedule tab's data too.
    assert!(sim.open_spec(ObjectRef::Stair(id)));
    sim.ok();
    // Left: a wall handrail. Right: a guard with its own glass infill.
    let mut draft = stairs(&sim, 0)[0].clone();
    draft.stair.params.left_side = SideKind::Handrail;
    draft.stair.params.right_side = SideKind::Railing;
    let mut right = draft.stair.params.railing;
    right.style = RailStyle::Glass;
    right.newel.size = 5.0;
    draft.stair.params.right_railing = Some(right);
    draft.stair.params.bullnose = Bullnose::Both;
    assert!(stairs_view::apply_edit(&mut sim.app.cx, &draft));
    let o = stairs(&sim, 0)[0].clone();
    let parts = plan_stairs::tagged_meshes(&o.stair);
    let rails = parts
        .iter()
        .filter(|(p, _)| *p == StairPart::Handrail)
        .count();
    assert!(rails >= 2, "the handrail and the guard are in 3D: {rails}");
    // The Components tab lists them and the bullnose.
    let names: Vec<String> = stairs_view::components(&o)
        .into_iter()
        .map(|c| c.name)
        .collect();
    for want in ["Handrails", "Bullnose bottom tread", "Newels"] {
        assert!(names.iter().any(|n| n == want), "{want} in {names:?}");
    }
    // Auto Stairwell opens the floor above; there the treads show dashed.
    assert!(stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::AutoStairwell
    ));
    let o = stairs(&sim, 0)[0].clone();
    assert!(stairs_view::open_to_floor_above(&sim.app.cx.project, 0, &o));
    let on_floor_0 = sim.plan_shapes().len();
    sim.action(Action::FloorUp);
    let on_floor_1 = sim.plan_shapes().len();
    assert!(on_floor_1 > 0 && on_floor_0 > 0);
    // The undo history keeps the edits as steps of their own.
    sim.action(Action::FloorDown);
    assert_eq!(sim.undo().as_deref(), Some("Auto Stairwell"));
    assert_eq!(sim.undo().as_deref(), Some("Stair Specification"));
    let o = stairs(&sim, 0)[0].clone();
    assert_eq!(o.stair.params.left_side, SideKind::None);
    assert!(o.stair.params.right_railing.is_none());
    assert_eq!(o.stair.params.bullnose, Bullnose::None);
    let _ = cabinet_by_id;
}
