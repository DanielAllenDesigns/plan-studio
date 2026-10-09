//! Scenario 63 (integration pass 2, area C): the Select tool honours the
//! cabinet Bump / Push / Pass Through mode and offers Convert Polyline to
//! Soffit; the Library Browser reads its preferences.

use super::{draw_shell, Sim};
use crate::editor::handles::HandleKind;
use crate::editor::placed::{self, load_cabinets, PlacedRef};
use crate::editor::ObjectRef;
use crate::tools::cabinet::{bump_mode, set_bump_mode, BumpMode};
use crate::tools::ToolId;
use plan_cabinets::{Cabinet, CabinetKind};
use plan_core::geometry::Point;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    sim
}

fn cabs(sim: &Sim) -> Vec<Cabinet> {
    load_cabinets(sim.app.cx.floor())
}

fn place(sim: &mut Sim, x: f64, y: f64) {
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    sim.app.cx.selection.clear();
    sim.click(x, y);
}

#[test]
fn the_select_tool_pushes_neighbours_in_push_mode_and_undoes_in_one_step() {
    let mut sim = house();
    placed::set_auto_join(false);
    set_bump_mode(BumpMode::Push);
    place(&mut sim, 100.0, 10.0);
    place(&mut sim, 124.0, 10.0);
    place(&mut sim, 260.0, 10.0);
    let run = cabs(&sim);
    assert_eq!(run.len(), 3);
    let xs0: Vec<f64> = run.iter().map(|c| c.position.x).collect();
    let moving = run[2].id;
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Cabinet(moving));
    let handle = placed::placed_handles(
        sim.app.cx.floor(),
        PlacedRef::Cabinet(moving),
        sim.app.cx.px_per_in,
    )
    .into_iter()
    .find(|h| h.kind == HandleKind::Move)
    .unwrap();
    let (hx, hy) = (handle.pos.x, handle.pos.y);
    let target_left = xs0[1] + run[1].width - 10.0;
    let dx = target_left - xs0[2];
    sim.drag((hx, hy), (hx + dx, hy));
    let after = cabs(&sim);
    let x = |id: u64| after.iter().find(|c| c.id == id).unwrap().position.x;
    assert!((x(moving) - target_left).abs() < 1e-6, "{}", x(moving));
    assert!((x(run[1].id) - (target_left - run[1].width)).abs() < 1e-6);
    assert!((x(run[0].id) - (x(run[1].id) - run[0].width)).abs() < 1e-6);
    assert!(sim.undo().is_some());
    let back: Vec<f64> = cabs(&sim).iter().map(|c| c.position.x).collect();
    assert_eq!(back, xs0, "one undo step puts the whole run back");
    set_bump_mode(BumpMode::Bump);
    assert_eq!(bump_mode(), BumpMode::Bump);
}

#[test]
fn the_select_tool_edit_toolbar_offers_convert_polyline_to_soffit_for_a_closed_polyline() {
    let mut sim = house();
    let ring = vec![
        Point::new(60.0, 60.0),
        Point::new(180.0, 60.0),
        Point::new(180.0, 120.0),
        Point::new(60.0, 120.0),
    ];
    let closed = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        plan_core::CadItem::Polyline {
            points: ring.clone(),
            closed: true,
        },
    );
    let open = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        plan_core::CadItem::Polyline {
            points: ring,
            closed: false,
        },
    );
    sim.tool(ToolId::Select);
    let label = "Convert Polyline to Soffit";
    sim.app.cx.selection.set(ObjectRef::Cad(open));
    let bar = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    assert!(!bar.iter().any(|a| a.label == label));
    sim.app.cx.selection.set(ObjectRef::Cad(closed));
    let bar = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    let button = bar
        .iter()
        .find(|a| a.label == label)
        .expect("offered for a closed polyline");
    sim.app.cx.apply_edit_action(button.kind);
    let soffits: Vec<Cabinet> = cabs(&sim)
        .into_iter()
        .filter(|c| c.kind == CabinetKind::Soffit)
        .collect();
    assert_eq!(soffits.len(), 1);
    assert!(sim.app.cx.floor().cad.iter().all(|c| c.id != closed));
}

#[test]
fn clear_terrain_removes_only_what_build_terrain_made() {
    use crate::editor::site_view::{edit_terrain, load_terrain};
    use crate::toolbar::{Action, TerrainCommand};
    let mut sim = house();
    edit_terrain(&mut sim.app.cx, "Terrain", |rec| {
        rec.terrain.perimeter = vec![
            Point::new(0.0, 0.0),
            Point::new(600.0, 0.0),
            Point::new(600.0, 400.0),
            Point::new(0.0, 400.0),
        ];
        for (x, z) in [(0.0, 0.0), (600.0, 60.0)] {
            rec.terrain
                .elevation_points
                .push(plan_terrain::ElevationPoint {
                    pos: Point::new(x, 100.0),
                    z,
                });
        }
        rec.built = true;
    });
    sim.action(Action::Terrain(TerrainCommand::Clear));
    let rec = load_terrain(&sim.app.cx.project).expect("the terrain record stays");
    assert!(!rec.built, "the generated surface is gone");
    assert_eq!(rec.terrain.perimeter.len(), 4, "the perimeter stays");
    assert_eq!(rec.terrain.elevation_points.len(), 2, "the data stays");
    assert_eq!(sim.undo().as_deref(), Some("Clear Terrain"));
    assert!(load_terrain(&sim.app.cx.project).unwrap().built);
}
