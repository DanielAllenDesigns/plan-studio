//! Scenario 75: tray and coffered ceilings, ceiling planes and cathedral
//! behavior (manual pp. 457-462, 857-860; R-111, R-112, R-32, R-146).
//!
//! Make a tray in a living room, nest one, add rope lights through the
//! Tray Ceiling Specification, then switch the room to a cathedral ceiling
//! and see the heights change and the trays show their Caution; make a
//! coffered ceiling, explode a tray, convert a polyline. Every action is one
//! undo step.

use super::{draw_shell, Sim};
use crate::dialogs::tray_ceiling as dlg;
use crate::editor::roof_view;
use crate::editor::rooms_edit;
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::tools::roof::RoofMode;
use crate::tools::tray_ceiling::{self as tc, cmd};
use crate::tools::ToolId;
use plan_core::geometry::Point;
use plan_core::tray::{ceiling_height_at, Caution, RopeLight, TrayCeiling};
use plan_core::Id;

const W: f64 = 240.0;
const H: f64 = 180.0;
const CENTER: (f64, f64) = (W / 2.0, H / 2.0);

fn house() -> Sim {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    draw_shell(&mut sim, W, H);
    sim
}

fn ceiling(sim: &Sim) -> f64 {
    sim.app.cx.floor().ceiling_height
}

fn tray_ids(sim: &Sim) -> Vec<Id> {
    sim.app.cx.floor().tray_ids()
}

fn label(sim: &Sim) -> Option<String> {
    sim.app.cx.undo_label().map(String::from)
}

/// The finished ceiling height over plan point `p` with the trays in place.
fn height_at(sim: &Sim, p: (f64, f64)) -> f64 {
    ceiling_height_at(&tc::geoms(&sim.app.cx), Point::new(p.0, p.1), ceiling(sim))
}

fn select_room(sim: &mut Sim) {
    sim.app.cx.refresh();
    assert!(!sim.app.cx.rooms.is_empty(), "the shell makes a room");
    rooms_edit::select_room(&mut sim.app.cx, 0);
}

fn tray_meshes(sim: &Sim, id: Id) -> usize {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .count()
}

fn build_roof(sim: &mut Sim) {
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(CENTER.0, CENTER.1);
    sim.ok();
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
}

/// Runs an Edit toolbar command the way the button does and lets the
/// dialogs it opens run.
fn command(sim: &mut Sim, id: &str) {
    assert!(tc::run_command(&mut sim.app.cx, id), "{id}");
}

#[test]
fn clicking_in_a_room_makes_a_tray_that_follows_it_in_one_undo_step() {
    let mut sim = house();
    let h = ceiling(&sim);
    sim.tool(ToolId::TrayCeiling);
    assert_eq!(sim.app.tools.active().name(), "Tray Ceiling Polyline");
    let r = sim.click(CENTER.0, CENTER.1);
    assert!(r.consumed);
    assert_eq!(tray_ids(&sim).len(), 1);
    assert_eq!(label(&sim).as_deref(), Some("Make Tray Ceiling in Room"));
    let id = tray_ids(&sim)[0];
    // The polyline is a CAD polyline on the Ceiling Planes layer.
    let obj = sim.app.cx.floor().cad.iter().find(|c| c.id == id).unwrap();
    assert_eq!(obj.layer, plan_core::tray::LAYER);
    // The default tray: a 24" ring dropped 8" below the ceiling.
    let g = tc::geoms(&sim.app.cx);
    assert!(g[0].ok());
    assert_eq!((g[0].h_inner, g[0].h_outer), (h, h - 8.0));
    assert_eq!(height_at(&sim, CENTER), h);
    assert_eq!(height_at(&sim, (12.0, 12.0)), h - 8.0);
    // One undo step takes it all away; redo brings it back.
    sim.undo();
    assert!(tray_ids(&sim).is_empty());
    assert!(!sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .any(|c| c.layer == plan_core::tray::LAYER));
    sim.redo();
    assert_eq!(tray_ids(&sim), vec![id]);
}

#[test]
fn dragging_draws_a_rectangle_polyline_nested_in_the_tray() {
    let mut sim = house();
    sim.tool(ToolId::TrayCeiling);
    sim.click(CENTER.0, CENTER.1);
    let outer = tray_ids(&sim)[0];
    let r = sim.drag((70.0, 50.0), (170.0, 130.0));
    assert_eq!(r.commit.as_deref(), Some("Tray Ceiling Polyline"));
    assert_eq!(tray_ids(&sim).len(), 2);
    assert_eq!(label(&sim).as_deref(), Some("Tray Ceiling Polyline"));
    let g = tc::geoms(&sim.app.cx);
    let inner = g.iter().find(|g| g.id != outer).unwrap();
    assert_eq!(inner.parent, Some(outer));
    assert_eq!(inner.level, 1);
    // A stray click outside any room makes nothing, and a tiny drag nothing.
    let before = tray_ids(&sim).len();
    sim.drag((400.0, 400.0), (405.0, 405.0));
    sim.click(900.0, 900.0);
    assert_eq!(tray_ids(&sim).len(), before);
    // Esc drops a pending drag.
    sim.move_to(10.0, 10.0);
    sim.down(10.0, 10.0);
    assert!(sim.esc().consumed);
    sim.up(80.0, 80.0);
    assert_eq!(tray_ids(&sim).len(), before);
}

#[test]
fn make_a_tray_nest_one_add_rope_lights_then_go_cathedral() {
    let mut sim = house();
    let h = ceiling(&sim);
    build_roof(&mut sim);

    // 1. Make Tray Ceiling in Room opens the specification with a Width.
    select_room(&mut sim);
    let buttons = tc::edit_actions(&sim.app.cx);
    let names: Vec<&str> = buttons.iter().map(|b| b.label).collect();
    assert!(names.contains(&"Make Tray Ceiling in Room"), "{names:?}");
    assert!(names.contains(&"Turn Off Ceiling"));
    command(&mut sim, cmd::MAKE_IN_ROOM);
    assert!(dlg::dialog_open());
    assert_eq!(dlg::with_dialog(|d| d.shows_width()), Some(true));
    dlg::with_dialog(|d| {
        d.draft_mut().width = 24.0;
        d.draft_mut().depth = 8.0;
    });
    let parent = dlg::accept_dialog(&mut sim.app.cx).expect("the tray is made");
    assert_eq!(label(&sim).as_deref(), Some("Make Tray Ceiling in Room"));
    assert_eq!(tray_ids(&sim), vec![parent]);
    assert_eq!(height_at(&sim, (12.0, 12.0)), h - 8.0);

    // 2. Make Nested Tray Ceiling: 12" in from the first, 4" deep.
    assert_eq!(
        sim.app.cx.selection.single(),
        Some(crate::editor::ObjectRef::Cad(parent))
    );
    command(&mut sim, cmd::NESTED);
    dlg::with_dialog(|d| {
        d.draft_mut().width = 12.0;
        d.draft_mut().depth = 4.0;
    });
    let nested = dlg::accept_dialog(&mut sim.app.cx).expect("the nested tray is made");
    assert_eq!(label(&sim).as_deref(), Some("Make Nested Tray Ceiling"));
    let g = tc::geoms(&sim.app.cx);
    assert_eq!(g.len(), 2);
    let gn = g.iter().find(|g| g.id == nested).unwrap();
    assert_eq!(gn.parent, Some(parent));
    assert_eq!(gn.h_outer, h - 4.0);
    // The nested ring is the parent's hole inset: 36" from the room's wall.
    assert_eq!(height_at(&sim, (12.0, 12.0)), h - 8.0);
    assert_eq!(height_at(&sim, CENTER), h);
    let inside_nested_ring = gn.inner[0] + Point::new(-4.0, -4.0);
    assert_eq!(
        ceiling_height_at(&g, inside_nested_ring, h),
        h - 4.0,
        "{inside_nested_ring:?}"
    );

    // 3. Rope lights on the nested tray (the Rope Lights panel).
    sim.app
        .cx
        .selection
        .set(crate::editor::ObjectRef::Cad(nested));
    command(&mut sim, cmd::SPEC);
    dlg::with_dialog(|d| d.draft_mut().rope_lights.push(RopeLight::default())).unwrap();
    dlg::accept_dialog(&mut sim.app.cx);
    assert_eq!(label(&sim).as_deref(), Some("Tray Ceiling Specification"));
    let rec = sim.app.cx.floor().tray(nested).unwrap().clone();
    assert_eq!(rec.rope_lights.len(), 1);
    let g = tc::geoms(&sim.app.cx);
    let gn = g.iter().find(|g| g.id == nested).unwrap();
    let ropes = plan_core::tray::rope_light_paths(gn, &rec);
    assert_eq!(ropes.len(), 1);
    // 2" above the nested ring (at h - 4).
    assert_eq!(ropes[0].elevation, h - 4.0 + 2.0);
    // Undo takes back just the rope light.
    sim.undo();
    assert!(sim
        .app
        .cx
        .floor()
        .tray(nested)
        .unwrap()
        .rope_lights
        .is_empty());
    sim.redo();
    assert_eq!(
        sim.app.cx.floor().tray(nested).unwrap().rope_lights.len(),
        1
    );

    // 4. Both trays are in the 3D scene.
    assert!(tray_meshes(&sim, parent) > 0);
    assert!(tray_meshes(&sim, nested) > 0);

    // 5. Turn Off Ceiling: the room becomes a cathedral. Trays show their
    // Caution and leave the scene; the ceiling follows the roof.
    select_room(&mut sim);
    let flat_height = height_at(&sim, CENTER);
    command(&mut sim, cmd::CEILING_OFF);
    assert_eq!(label(&sim).as_deref(), Some("Turn Off Ceiling"));
    let g = tc::geoms(&sim.app.cx);
    assert!(g.iter().all(|t| t.caution == Some(Caution::RoomNotFlat)));
    assert_eq!(tray_meshes(&sim, parent), 0);
    assert_eq!(tray_meshes(&sim, nested), 0);
    let floor = sim.app.cx.floor();
    let planes = plan_3d::tray::cathedral_planes(floor);
    assert!(!planes.is_empty(), "the roof gives the room ceiling planes");
    let vault = plan_roof::ceiling_height_at(&planes, Point::new(CENTER.0, CENTER.1))
        .expect("a plane over the room's middle")
        - floor.elevation;
    assert!(vault > flat_height + 10.0, "{vault} vs {flat_height}");
    // The menu offers Turn On Ceiling now, and a tray cannot be exploded.
    select_room(&mut sim);
    let names: Vec<&str> = tc::edit_actions(&sim.app.cx)
        .iter()
        .map(|b| b.label)
        .collect();
    assert!(names.contains(&"Turn On Ceiling"), "{names:?}");
    assert!(!tc::explode(&mut sim.app.cx, parent));

    // 6. Turn On Ceiling brings the trays back.
    command(&mut sim, cmd::CEILING_ON);
    assert_eq!(label(&sim).as_deref(), Some("Turn On Ceiling"));
    assert!(tc::geoms(&sim.app.cx).iter().all(|t| t.ok()));
    assert!(tray_meshes(&sim, parent) > 0);
    assert!(plan_3d::tray::cathedral_planes(sim.app.cx.floor()).is_empty());
    assert_eq!(height_at(&sim, (12.0, 12.0)), h - 8.0);
}

#[test]
fn a_coffered_ceiling_is_a_grid_of_recessed_trays_in_one_step() {
    let mut sim = house();
    let h = ceiling(&sim);
    select_room(&mut sim);
    command(&mut sim, cmd::COFFERED);
    assert_eq!(label(&sim).as_deref(), Some("Make Coffered Ceiling"));
    let n = tray_ids(&sim).len();
    // A 234" x 174" room in 36" cells with 8" beams: 5 by 3.
    assert_eq!(n, 15, "coffers");
    let g = tc::geoms(&sim.app.cx);
    assert!(g
        .iter()
        .all(|t| t.ok() && t.level == 0 && t.h_inner == h + 6.0));
    // A coffer is raised, a beam is at the ceiling.
    let c = g[0].inner[0] + Point::new(20.0, 20.0);
    assert_eq!(ceiling_height_at(&g, c, h), h + 6.0);
    // The recessed trays cut the ceiling platform.
    assert_eq!(plan_3d::tray::recess_holes(sim.app.cx.floor()).len(), n);
    sim.undo();
    assert!(tray_ids(&sim).is_empty());
}

#[test]
fn exploding_a_tray_leaves_ceiling_planes_and_a_plain_polyline() {
    let mut sim = house();
    sim.tool(ToolId::TrayCeiling);
    sim.click(CENTER.0, CENTER.1);
    let id = tray_ids(&sim)[0];
    // Rope lights and a molding become polylines of their own.
    sim.app.cx.selection.set(crate::editor::ObjectRef::Cad(id));
    command(&mut sim, cmd::SPEC);
    dlg::with_dialog(|d| {
        d.draft_mut().rope_lights.push(RopeLight::default());
        d.draft_mut().moldings.push(Default::default());
    });
    dlg::accept_dialog(&mut sim.app.cx);
    let cad_before = sim.app.cx.floor().cad.len();
    assert!(roof_view::load(sim.app.cx.floor()).ceilings.is_empty());
    assert!(tc::explode(&mut sim.app.cx, id));
    assert_eq!(label(&sim).as_deref(), Some("Explode Tray Ceiling"));
    assert!(tray_ids(&sim).is_empty());
    let set = roof_view::load(sim.app.cx.floor());
    assert!(!set.ceilings.is_empty());
    // The ring is flat at the dropped height (4 pieces around the hole).
    assert!(set.ceilings.iter().all(|c| c.pitch == 0.0));
    let h = sim.app.cx.floor().elevation + ceiling(&sim);
    assert!(set
        .ceilings
        .iter()
        .all(|c| (c.height_at_baseline - (h - 8.0)).abs() < 1e-6));
    // The polyline, the molding run and the rope light run remain.
    assert_eq!(sim.app.cx.floor().cad.len(), cad_before + 2);
    // One undo puts the tray back whole.
    sim.undo();
    assert_eq!(tray_ids(&sim), vec![id]);
    assert!(roof_view::load(sim.app.cx.floor()).ceilings.is_empty());
    assert_eq!(sim.app.cx.floor().cad.len(), cad_before);
}

#[test]
fn a_closed_polyline_converts_to_a_tray_and_sloped_sides_drop_the_rest() {
    let mut sim = house();
    // A CAD rectangle.
    sim.tool(ToolId::Select);
    let pts = vec![
        Point::new(60.0, 40.0),
        Point::new(180.0, 40.0),
        Point::new(180.0, 140.0),
        Point::new(60.0, 140.0),
    ];
    let id = sim.app.cx.project.alloc_id();
    sim.app.cx.project.floors[0].cad.push(plan_core::CadObject {
        id,
        layer: plan_core::cad::DEFAULT_CAD_LAYER.into(),
        item: plan_core::CadItem::Polyline {
            points: pts,
            closed: true,
        },
    });
    sim.app.cx.refresh();
    sim.app.cx.selection.set(crate::editor::ObjectRef::Cad(id));
    let names: Vec<&str> = tc::edit_actions(&sim.app.cx)
        .iter()
        .map(|b| b.label)
        .collect();
    assert!(
        names.contains(&"Convert Polyline to Tray Ceiling"),
        "{names:?}"
    );
    command(&mut sim, cmd::CONVERT);
    assert_eq!(
        label(&sim).as_deref(),
        Some("Convert Polyline to Tray Ceiling")
    );
    assert_eq!(tray_ids(&sim), vec![id]);
    // Sloped sides: pitch 12 in 12, 8" deep; moldings and ropes are off.
    sim.app.cx.selection.set(crate::editor::ObjectRef::Cad(id));
    command(&mut sim, cmd::SPEC);
    dlg::with_dialog(|d| {
        d.draft_mut().pitch = Some(12.0);
        d.draft_mut().width = 24.0;
    });
    dlg::accept_dialog(&mut sim.app.cx);
    let g = tc::geoms(&sim.app.cx);
    assert!((g[0].run - 8.0).abs() < 1e-9);
    // Halfway up the 8" run (4" out from the hole) is 4" below the ceiling.
    let h = ceiling(&sim);
    let p = Point::new(60.0 - 4.0, 90.0);
    assert!((g[0].height_at(p).unwrap() - (h - 4.0)).abs() < 1e-9);
    // Too wide a slope for the ring is a Caution and changes nothing.
    sim.app.cx.selection.set(crate::editor::ObjectRef::Cad(id));
    command(&mut sim, cmd::SPEC);
    dlg::with_dialog(|d| d.draft_mut().width = 4.0);
    dlg::accept_dialog(&mut sim.app.cx);
    let g = tc::geoms(&sim.app.cx);
    assert_eq!(g[0].caution, Some(Caution::SlopeTooWide));
    assert_eq!(height_at(&sim, (62.0, 90.0)), h);
}

#[test]
fn the_plan_draws_trays_and_the_cautions_without_trouble() {
    let mut sim = house();
    sim.tool(ToolId::TrayCeiling);
    sim.click(CENTER.0, CENTER.1);
    sim.dialog_frame(false);
    // Cathedral: the Caution symbol is drawn instead.
    select_room(&mut sim);
    command(&mut sim, cmd::CEILING_OFF);
    sim.dialog_frame(false);
    let g = tc::geoms(&sim.app.cx);
    assert_eq!(g[0].caution, Some(Caution::RoomNotFlat));
    assert!(!g[0].ok());
    let _ = TrayCeiling::default();
}

#[test]
fn trays_survive_a_save_and_load() {
    let mut sim = house();
    sim.tool(ToolId::TrayCeiling);
    sim.click(CENTER.0, CENTER.1);
    let id = tray_ids(&sim)[0];
    let json = sim.app.cx.project.to_json().unwrap();
    let back = plan_core::Project::from_json(&json).unwrap();
    assert!(back.floors[0].is_tray(id));
    assert_eq!(
        back.floors[0].trays.get(id),
        sim.app.cx.floor().trays.get(id)
    );
}
