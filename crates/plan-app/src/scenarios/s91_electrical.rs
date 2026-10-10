//! Scenario 91 (round 16, brief 25): Electrical Defaults, outlet tools,
//! connection splines and rope lights (E-19..E-22, E-30..E-33; manual
//! pp. 691-707).
//!
//! Draw outlets, switches and lights inside and outside the house and see the
//! weatherproof types appear; edit the Electrical Defaults and see the next
//! devices follow them; place outlets over a counter, over a sink and on the
//! side of a cabinet; connect a switch to two lights with a curved spline,
//! bend, reset and detach it; draw a rope light under a wall cabinet. Every
//! action is one undo step.

use super::{draw_shell, Sim};
use crate::dialogs::default_pages::electrical as defaults_page;
use crate::dialogs::materials_list as ml;
use crate::editor::placed::{add_cabinet, load_cabinets};
use crate::editor::site_view::{load_electrical, save_electrical};
use crate::editor::ObjectRef;
use crate::toolbar::Action;
use crate::tools::cabinet::default_cabinet;
use crate::tools::electrical::{self as et, cmd, ElecVariant};
use crate::tools::ToolId;
use plan_cabinets::CabinetKind;
use plan_core::geometry::Point;
use plan_electrical::{
    Arrow, ConnEnd, Device, DeviceKind, ElectricalDefaults, ElectricalLayer, HeightContext, Mount,
};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    // The shell's exact corners are not the point: no snapping.
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    draw_shell(&mut sim, W, H);
    sim
}

fn elec(sim: &mut Sim, v: ElecVariant) {
    sim.tool(ToolId::ElectricalVariant(v));
}

fn layer(sim: &Sim) -> ElectricalLayer {
    load_electrical(sim.app.cx.floor())
}

fn devices(sim: &Sim) -> Vec<Device> {
    layer(sim).devices
}

fn last(sim: &Sim) -> Device {
    devices(sim).last().cloned().expect("a device")
}

fn label(sim: &Sim) -> Option<String> {
    sim.app.cx.undo_label().map(String::from)
}

/// Places a device with the tool `v` at (`x`, `y`) and returns it.
fn place(sim: &mut Sim, v: ElecVariant, x: f64, y: f64) -> Device {
    elec(sim, v);
    let n = devices(sim).len();
    sim.click(x, y);
    assert_eq!(
        devices(sim).len(),
        n + 1,
        "{v:?} at ({x}, {y}): {}",
        sim.app.cx.status
    );
    last(sim)
}

#[test]
fn the_outlet_switch_and_light_tools_read_where_they_are_clicked() {
    let mut sim = house();
    // Inside the south wall: plain devices at the Electrical Defaults heights.
    let a = place(&mut sim, ElecVariant::Outlet110, 100.0, 8.0);
    let b = place(&mut sim, ElecVariant::Gfci, 140.0, 8.0);
    let c = place(&mut sim, ElecVariant::Outlet220, 180.0, 8.0);
    assert_eq!(
        (a.kind, b.kind, c.kind),
        (
            DeviceKind::Outlet110,
            DeviceKind::Gfci,
            DeviceKind::Outlet220
        )
    );
    assert!([&a, &b, &c]
        .iter()
        .all(|d| d.height == 12.0 && d.wall_id.is_some()));
    assert!(a.position.y > 0.0, "on the room side");
    // Outside the same wall the 110V and GFCI tools place weatherproof outlets.
    let d = place(&mut sim, ElecVariant::Outlet110, 120.0, -8.0);
    let e = place(&mut sim, ElecVariant::Gfci, 160.0, -8.0);
    let f = place(&mut sim, ElecVariant::Outlet220, 200.0, -8.0);
    assert_eq!(
        (d.kind, e.kind),
        (DeviceKind::OutletWp, DeviceKind::OutletWp)
    );
    assert_eq!(f.kind, DeviceKind::Outlet220, "there is no WP 220V outlet");
    assert!(d.position.y < 0.0, "on the outside face");
    assert_eq!(label(&sim).as_deref(), Some("Place 220V Outlet"));
    // Switches and wall lights likewise.
    let s_in = place(&mut sim, ElecVariant::Switch, 300.0, 8.0);
    let s_out = place(&mut sim, ElecVariant::Switch, 330.0, -8.0);
    assert_eq!((s_in.kind, s_in.height), (DeviceKind::Switch, 48.0));
    assert_eq!(s_out.kind, DeviceKind::SwitchWp);
    let l_in = place(&mut sim, ElecVariant::Light, 400.0, 8.0);
    let l_out = place(&mut sim, ElecVariant::Light, 430.0, -8.0);
    assert_eq!(
        l_in.kind,
        DeviceKind::WallSconce,
        "near a wall the Light tool hangs a wall light"
    );
    assert_eq!(l_out.kind, DeviceKind::WallLightExterior);
    assert!(l_in.wall_id.is_some() && l_out.wall_id.is_some());
    // The status bar shows the height the next click would use.
    elec(&mut sim, ElecVariant::Switch);
    sim.move_to(340.0, 8.0);
    assert_eq!(sim.app.cx.readout.as_deref(), Some("Height: 48\""));
    // Away from the walls: a floor outlet, a ceiling light, a path light.
    let floor = place(&mut sim, ElecVariant::Outlet110, 200.0, 120.0);
    assert_eq!(floor.kind, DeviceKind::OutletFloor);
    assert_eq!((floor.height, floor.wall_id), (0.0, None));
    assert_eq!(layer(&sim).options_of(floor.id).mount, Mount::Floor);
    let ceiling = place(&mut sim, ElecVariant::Light, 244.0, 176.0);
    assert_eq!(ceiling.kind, DeviceKind::CeilingLight);
    assert!(
        ceiling.position.dist(Point::new(W / 2.0, H / 2.0)) < 2.0,
        "snaps to the room's center: {:?}",
        ceiling.position
    );
    assert_eq!(layer(&sim).options_of(ceiling.id).mount, Mount::Ceiling);
    let path = place(&mut sim, ElecVariant::Light, 700.0, 500.0);
    assert_eq!(path.kind, DeviceKind::PathLight, "outside every room");
    // A switch needs a wall.
    elec(&mut sim, ElecVariant::Switch);
    let n = devices(&sim).len();
    sim.click(200.0, 200.0);
    assert_eq!(devices(&sim).len(), n);
    assert!(sim.app.cx.status.contains("wall"), "{}", sim.app.cx.status);
    // One undo step per click, each named for what was placed.
    assert_eq!(sim.undo().as_deref(), Some("Place Path Light"));
    assert_eq!(sim.undo().as_deref(), Some("Place Ceiling Light"));
    assert_eq!(sim.undo().as_deref(), Some("Place Floor Outlet"));
}

#[test]
fn a_garage_takes_ceiling_outlets_and_a_deck_weatherproof_ones() {
    let mut sim = house();
    let anchor = Point::new(W / 2.0, H / 2.0);
    sim.app.cx.project.floors[0]
        .room_names
        .push(plan_core::model::RoomName::new(anchor, "Garage", "Garage"));
    sim.app.cx.refresh();
    let o = place(&mut sim, ElecVariant::Outlet110, 200.0, 120.0);
    assert_eq!(
        o.kind,
        DeviceKind::Outlet110,
        "a 110V outlet in a garage goes on the ceiling"
    );
    assert_eq!(layer(&sim).options_of(o.id).mount, Mount::Ceiling);
    assert_eq!(o.height, DeviceKind::CeilingLight.default_height());
    // The GFCI tool puts its outlet on the floor.
    let g = place(&mut sim, ElecVariant::Gfci, 260.0, 120.0);
    assert_eq!(g.kind, DeviceKind::OutletFloor);
    // The same room as a deck: everything in it is weatherproof.
    let mut deck = house();
    deck.app.cx.project.floors[0]
        .room_names
        .push(plan_core::model::RoomName::new(anchor, "Deck", "Deck"));
    deck.app.cx.refresh();
    let wp = place(&mut deck, ElecVariant::Outlet110, 200.0, 120.0);
    assert_eq!((wp.kind, wp.height), (DeviceKind::OutletWp, 0.0));
    let on_wall = place(&mut deck, ElecVariant::Outlet110, 100.0, 8.0);
    assert_eq!(
        on_wall.kind,
        DeviceKind::OutletWp,
        "the wall of an exterior room too"
    );
    let sw = place(&mut deck, ElecVariant::Switch, 300.0, 8.0);
    assert_eq!(sw.kind, DeviceKind::SwitchWp);
}

/// Auto Place Outlets in a house whose only room is named `name` of type
/// `ty`: the plain (not weatherproof) outlets it places, and the undo label.
fn auto_outlets_in(name: &str, ty: &str) -> (usize, Option<String>, Sim) {
    let mut sim = house();
    let anchor = Point::new(W / 2.0, H / 2.0);
    sim.app.cx.project.floors[0]
        .room_names
        .push(plan_core::model::RoomName::new(anchor, name, ty));
    sim.app.cx.refresh();
    et::auto_place_floor_outlets(&mut sim.app.cx);
    let inside = devices(&sim)
        .iter()
        .filter(|d| !d.kind.is_weatherproof())
        .count();
    let l = label(&sim);
    (inside, l, sim)
}

#[test]
fn auto_place_outlets_follows_the_room_functions() {
    let (bedroom, l, mut sim) = auto_outlets_in("Bedroom", "Bedroom");
    assert!(bedroom > 4, "a bedroom gets outlets all around: {bedroom}");
    assert_eq!(l.as_deref(), Some("Auto Place Outlets"));
    // One undo step takes them all away.
    assert_eq!(sim.undo().as_deref(), Some("Auto Place Outlets"));
    assert!(devices(&sim).is_empty());
    // An exterior room gets none; a hybrid room fewer than a bedroom.
    let (deck, _, _) = auto_outlets_in("Deck", "Deck");
    assert_eq!(deck, 0, "no Auto Place Outlets in an exterior room");
    let (garage, _, _) = auto_outlets_in("Garage", "Garage");
    assert!(
        garage > 0 && garage < bedroom,
        "garage {garage} vs {bedroom}"
    );
}

/// Opens the Electrical Defaults page on `tool`'s tab, edits it with `edit`
/// and presses OK.
fn edit_defaults(
    sim: &mut Sim,
    tool: ElecVariant,
    edit: impl FnOnce(&mut defaults_page::ElectricalPage),
) {
    defaults_page::request_open_for(tool);
    sim.dialog_frame(false);
    assert!(defaults_page::is_open());
    defaults_page::with_page(edit).expect("the page is open");
    sim.ok();
    assert!(!defaults_page::is_open(), "OK closes the page");
}

#[test]
fn editing_the_electrical_defaults_moves_the_next_devices() {
    let mut sim = house();
    let before = place(&mut sim, ElecVariant::Outlet110, 100.0, 8.0);
    assert_eq!(before.height, 12.0);
    // Double-clicking an Electrical Tools button opens the page on its tab.
    for (tool, tab) in [
        (ElecVariant::Gfci, et::DefaultsTab::Electrical),
        (ElecVariant::Connection, et::DefaultsTab::Connection),
        (ElecVariant::RopeLight, et::DefaultsTab::RopeLight),
    ] {
        defaults_page::request_open_for(tool);
        sim.dialog_frame(false);
        assert_eq!(defaults_page::with_page(|p| p.tab()), Some(tab), "{tool:?}");
        sim.cancel();
    }
    let undo_before = label(&sim);
    edit_defaults(&mut sim, ElecVariant::Outlet110, |p| {
        let d = p.defaults_mut();
        d.outlet_height = 18.0;
        d.switch_height = 42.0;
        d.above_base_cabinet = 6.0;
        d.on_cabinet_side = 30.0;
        assert!(p.choose_object("Light", DeviceKind::RecessedCan));
        assert!(
            !p.choose_object("Light", DeviceKind::Gfci),
            "not in its family"
        );
    });
    assert_eq!(label(&sim).as_deref(), Some("Electrical Defaults"));
    let stored = ElectricalDefaults::load(&sim.app.cx.project);
    assert_eq!(stored.outlet_height, 18.0);
    assert_eq!(stored.counter_height(), 42.0);
    assert!(label(&sim) != undo_before || undo_before.is_none());
    // One undo step takes the dialog's OK back, and redo brings it again.
    assert_eq!(sim.undo().as_deref(), Some("Electrical Defaults"));
    assert_eq!(
        ElectricalDefaults::load(&sim.app.cx.project),
        ElectricalDefaults::default()
    );
    assert_eq!(sim.redo().as_deref(), Some("Electrical Defaults"));
    assert_eq!(ElectricalDefaults::load(&sim.app.cx.project), stored);
    // The devices already placed keep their heights; the next ones follow.
    assert_eq!(devices(&sim)[0].height, 12.0);
    let o = place(&mut sim, ElecVariant::Outlet110, 140.0, 8.0);
    let g = place(&mut sim, ElecVariant::Gfci, 180.0, 8.0);
    let s = place(&mut sim, ElecVariant::Switch, 300.0, 8.0);
    assert_eq!((o.height, g.height, s.height), (18.0, 18.0, 42.0));
    let phone = place(&mut sim, ElecVariant::PhoneJack, 220.0, 8.0);
    assert_eq!(phone.height, 18.0, "jacks follow the Outlet height");
    let t = place(&mut sim, ElecVariant::Thermostat, 260.0, 8.0);
    assert_eq!(t.height, 42.0, "thermostats follow the Switch height");
    // The Light tool now places the chosen Default Library Object.
    let l = place(&mut sim, ElecVariant::Light, 244.0, 176.0);
    assert_eq!(l.kind, DeviceKind::RecessedCan);
    // Use Default Heights off: the height saved with each symbol.
    edit_defaults(&mut sim, ElecVariant::Outlet110, |p| {
        p.defaults_mut().use_default_heights = false;
    });
    let o2 = place(&mut sim, ElecVariant::Outlet110, 340.0, 8.0);
    assert_eq!(o2.height, DeviceKind::Outlet110.default_height());
}

#[test]
fn outlets_go_over_the_counter_not_over_the_sink_and_up_the_side_of_a_cabinet() {
    let mut sim = house();
    let anchor = Point::new(W / 2.0, H / 2.0);
    sim.app.cx.project.floors[0]
        .room_names
        .push(plan_core::model::RoomName::new(
            anchor, "Kitchen", "Kitchen",
        ));
    // A base cabinet and a sink base against the south wall (inner face y = 3).
    let mut base = default_cabinet(&sim.app.cx, CabinetKind::Base);
    base.position = Point::new(100.0, 3.0);
    base.angle = 0.0;
    base.width = 60.0;
    let base_top = base.elevation + base.height;
    add_cabinet(&mut sim.app.cx.project, 0, base).unwrap();
    let mut sink = default_cabinet(&sim.app.cx, CabinetKind::Base);
    sink.position = Point::new(200.0, 3.0);
    sink.face = plan_cabinets::FaceLayout::sink_base();
    add_cabinet(&mut sim.app.cx.project, 0, sink).unwrap();
    // A free-standing island for the cabinet side.
    let mut island = default_cabinet(&sim.app.cx, CabinetKind::Base);
    island.position = Point::new(200.0, 200.0);
    island.width = 36.0;
    let island_bottom = island.elevation;
    let island_id = add_cabinet(&mut sim.app.cx.project, 0, island).unwrap();
    sim.app.cx.refresh();
    assert_eq!(load_cabinets(sim.app.cx.floor()).len(), 3);
    let defaults = ElectricalDefaults::default();
    // The wall behind a base cabinet: above the counter, GFCI in a kitchen.
    let over = place(&mut sim, ElecVariant::Outlet110, 108.0, 8.0);
    assert_eq!(
        over.kind,
        DeviceKind::Gfci,
        "a kitchen counter is GFCI protected"
    );
    assert_eq!(over.height, base_top + defaults.above_base_cabinet);
    // Behind the sink the plain Outlet height.
    let behind_sink = place(&mut sim, ElecVariant::Outlet110, 212.0, 8.0);
    assert_eq!(behind_sink.height, defaults.outlet_height);
    assert_eq!(behind_sink.kind, DeviceKind::Outlet110);
    // A switch over the counter measures up from it too.
    let sw = place(&mut sim, ElecVariant::Switch, 150.0, 8.0);
    assert_eq!(sw.height, base_top + defaults.above_base_cabinet);
    // A plain wall away from the cabinets: the Outlet height.
    let plain = place(&mut sim, ElecVariant::Outlet110, 380.0, 8.0);
    assert_eq!(plain.height, 12.0);
    // The side of the island: 32 in up from its bottom, on the box.
    let side = place(&mut sim, ElecVariant::Outlet110, 238.0, 215.0);
    assert_eq!(side.wall_id, None);
    assert_eq!(side.height, island_bottom + defaults.on_cabinet_side);
    let o = layer(&sim).options_of(side.id);
    assert_eq!((o.mount, o.host), (Mount::CabinetSide, Some(island_id)));
    assert!(
        side.position.x > 230.0,
        "on the island's right side: {:?}",
        side.position
    );
    assert!(side.angle.cos() > 0.99, "facing out of that side");
    // Set as Default reads the same places back.
    let ctx = et::device_context(&sim.app.cx, &over);
    assert_eq!(
        ctx,
        HeightContext::AboveCounter {
            counter_top: base_top
        }
    );
    let ctx_side = et::device_context(&sim.app.cx, &side);
    assert_eq!(
        ctx_side,
        HeightContext::CabinetSide {
            bottom: island_bottom,
            top: island_bottom + sim_cabinet_height(&sim, island_id)
        }
    );
}

fn sim_cabinet_height(sim: &Sim, id: u64) -> f64 {
    load_cabinets(sim.app.cx.floor())
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.height)
        .unwrap()
}

/// Drags with the Electrical Connection tool from `a` to `b`.
fn drag(sim: &mut Sim, a: Point, b: Point) {
    sim.drag((a.x, a.y), (b.x, b.y));
}

#[test]
fn a_switch_is_connected_to_two_lights_with_curved_splines_that_can_be_edited() {
    let mut sim = house();
    let sw = place(&mut sim, ElecVariant::Switch, 60.0, 8.0);
    let l1 = place(&mut sim, ElecVariant::Light, 120.0, 100.0);
    let l2 = place(&mut sim, ElecVariant::RecessedLight, 300.0, 250.0);
    assert_ne!(l1.id, l2.id);
    elec(&mut sim, ElecVariant::Connection);
    // The diamond handle (or any press) on a device and a drag to the next.
    drag(&mut sim, sw.position, l1.position);
    assert_eq!(label(&sim).as_deref(), Some("Electrical Connection"));
    drag(&mut sim, sw.position, l2.position);
    assert_eq!(label(&sim).as_deref(), Some("Electrical Connection"));
    let lay = layer(&sim);
    assert_eq!(lay.connections.len(), 2);
    assert!(lay
        .connections
        .iter()
        .all(|c| c.is_arc() && c.from == sw.id));
    let ratio = lay.curvature_ratio(&lay.connections[0]).unwrap();
    assert!((ratio - plan_electrical::DEFAULT_CURVATURE).abs() < 1e-9);
    assert_eq!(lay.device(l1.id).unwrap().switched_by, vec![sw.id]);
    assert_eq!(lay.device(l2.id).unwrap().switched_by, vec![sw.id]);
    // The spline drawn last is selected: double-click its curve to add a vertex.
    let i = sim.app.tools.active().name();
    assert_eq!(i, "Electrical Connection");
    let path = lay.connection_path(&lay.connections[1]).unwrap();
    let on_curve = path[path.len() / 4];
    let r = sim.double_click(on_curve.x, on_curve.y);
    assert_eq!(r.commit.as_deref(), Some("Edit Connection"));
    assert_eq!(layer(&sim).connections[1].vertices.len(), 2);
    // Drag a vertex: one undo step, only that vertex moves.
    let before = layer(&sim).connections[1].vertices.clone();
    let target = before[1] + Point::new(40.0, -30.0);
    let r = sim.drag((before[1].x, before[1].y), (target.x, target.y));
    assert_eq!(r.commit.as_deref(), Some("Bend Connection"));
    let after = layer(&sim).connections[1].vertices.clone();
    assert_eq!(after[0], before[0]);
    assert!(after[1].dist(target) < 1e-6, "{:?} {:?}", after[1], target);
    assert_eq!(label(&sim).as_deref(), Some("Bend Connection"));
    // Reset Curvature takes the vertices away and applies the default ratio.
    assert!(et::run_command(&mut sim.app.cx, cmd::RESET_CURVATURE));
    assert_eq!(label(&sim).as_deref(), Some("Reset Curvature"));
    let lay = layer(&sim);
    assert!(lay.connections[1].is_arc());
    assert!(
        (lay.curvature_ratio(&lay.connections[1]).unwrap() - plan_electrical::DEFAULT_CURVATURE)
            .abs()
            < 1e-9
    );
    // Drag the light end off its light: the spline stays, the control goes.
    let end = lay.device(l2.id).unwrap().position;
    let inset = (sim.app.cx.pick_tol() * 1.5).max(6.0);
    let (_, handle) = et::end_handles(&lay, &lay.connections[1], inset).unwrap();
    assert!(
        handle.dist(end) > 3.0,
        "the end handle is off the light itself"
    );
    let r = sim.drag((handle.x, handle.y), (end.x + 60.0, end.y + 40.0));
    assert_eq!(r.commit.as_deref(), Some("Edit Connection"));
    let lay = layer(&sim);
    assert_eq!((lay.connections[1].from, lay.connections[1].to), (sw.id, 0));
    assert!(lay.device(l2.id).unwrap().switched_by.is_empty());
    assert_eq!(lay.connections.len(), 2, "the spline is still there");
    // Drop the end back on the light: wired again.
    let free = lay.connections[1].to_at.unwrap();
    let r = sim.drag((free.x, free.y), (end.x, end.y));
    assert_eq!(r.commit.as_deref(), Some("Edit Connection"));
    let lay = layer(&sim);
    assert_eq!(lay.connections[1].to, l2.id);
    assert_eq!(lay.device(l2.id).unwrap().switched_by, vec![sw.id]);
    // Undo walks back through the edits one at a time.
    assert_eq!(sim.undo().as_deref(), Some("Edit Connection"));
    assert_eq!(sim.undo().as_deref(), Some("Edit Connection"));
    assert_eq!(sim.undo().as_deref(), Some("Reset Curvature"));
    assert_eq!(sim.undo().as_deref(), Some("Bend Connection"));
    assert_eq!(sim.undo().as_deref(), Some("Edit Connection"));
}

#[test]
fn a_free_spline_and_the_diamond_handle_and_the_connection_defaults() {
    let mut sim = house();
    let sw = place(&mut sim, ElecVariant::Switch, 60.0, 8.0);
    let l = place(&mut sim, ElecVariant::Light, 120.0, 100.0);
    // Electrical Connection Defaults: a straighter, solid spline with an arrow.
    edit_defaults(&mut sim, ElecVariant::Connection, |p| {
        let c = &mut p.defaults_mut().connection;
        c.curvature_ratio = 0.1;
        c.line_style = plan_core::LineStyle::Solid;
        c.arrow = Arrow::End;
        c.label = "14/2".into();
    });
    // The diamond handle below the selected switch starts a spline.
    elec(&mut sim, ElecVariant::Switch);
    sim.click(sw.position.x, sw.position.y);
    let diamond = Point::new(
        sw.position.x,
        sw.position.y - (sim.app.cx.pick_tol() * 2.0).max(8.0),
    );
    sim.drag((diamond.x, diamond.y), (l.position.x, l.position.y));
    let lay = layer(&sim);
    assert_eq!(lay.connections.len(), 1);
    let c = &lay.connections[0];
    assert_eq!((c.from, c.to), (sw.id, l.id));
    assert!((lay.curvature_ratio(c).unwrap() - 0.1).abs() < 1e-9);
    assert_eq!(c.style(), plan_core::LineStyle::Solid);
    assert_eq!((c.arrow, c.label.as_str()), (Arrow::End, "14/2"));
    assert_eq!(label(&sim).as_deref(), Some("Electrical Connection"));
    // The spline draws with an arrow head and in its own layer.
    assert!(sim
        .app
        .cx
        .project
        .layers
        .get(plan_core::layers::ELECTRICAL_CONNECTION_LAYER)
        .is_some());
    let shapes = sim.plan_shapes();
    assert!(shapes.len() > 10);
    // A press on open ground and a drag draws a free spline.
    elec(&mut sim, ElecVariant::Connection);
    sim.drag((300.0, 300.0), (400.0, 300.0));
    let lay = layer(&sim);
    assert_eq!(lay.connections.len(), 2);
    assert_eq!((lay.connections[1].from, lay.connections[1].to), (0, 0));
    assert_eq!(label(&sim).as_deref(), Some("Electrical Connection"));
    sim.undo();
    assert_eq!(layer(&sim).connections.len(), 1);
    // A plain wire between two lights (no switch) is a drawing, not control.
    let l2 = place(&mut sim, ElecVariant::Light, 300.0, 250.0);
    elec(&mut sim, ElecVariant::Connection);
    sim.drag((l.position.x, l.position.y), (l2.position.x, l2.position.y));
    let lay = layer(&sim);
    assert_eq!(lay.connections.len(), 2);
    assert!(lay.device(l2.id).unwrap().switched_by.is_empty());
}

#[test]
fn more_switches_on_one_light_become_three_and_four_way_switches() {
    let mut sim = house();
    let l = place(&mut sim, ElecVariant::Light, 244.0, 176.0);
    let mut sws = Vec::new();
    for x in [60.0, 120.0, 180.0] {
        sws.push(place(&mut sim, ElecVariant::Switch, x, 8.0));
    }
    elec(&mut sim, ElecVariant::Connection);
    for s in &sws {
        drag(&mut sim, s.position, l.position);
    }
    let kinds: Vec<_> = sws
        .iter()
        .map(|s| layer(&sim).device(s.id).unwrap().kind)
        .collect();
    assert_eq!(
        kinds,
        [
            DeviceKind::Switch3Way,
            DeviceKind::Switch4Way,
            DeviceKind::Switch3Way
        ]
    );
    // A switch whose Automatically Change Switch Type When Wiring is off stays.
    let mut lay = layer(&sim);
    let extra = lay.add(plan_electrical::place_free(
        DeviceKind::Switch,
        Point::new(400.0, 40.0),
    ));
    let mut o = lay.options_of(extra);
    o.auto_switch_type = false;
    lay.set_options(extra, o);
    save_electrical(&mut sim.app.cx.project, 0, &lay);
    drag(&mut sim, Point::new(400.0, 40.0), l.position);
    assert_eq!(layer(&sim).device(extra).unwrap().kind, DeviceKind::Switch);
}

#[test]
fn a_rope_light_under_a_wall_cabinet_is_a_path_with_its_own_specification() {
    let mut sim = house();
    // A wall cabinet on the south wall, 54" up.
    let mut cab = default_cabinet(&sim.app.cx, CabinetKind::Wall);
    cab.position = Point::new(100.0, 3.0);
    let bottom = cab.elevation;
    add_cabinet(&mut sim.app.cx.project, 0, cab).unwrap();
    sim.app.cx.refresh();
    assert!(bottom > 12.0);
    // The Rope Light Defaults: 4" between lights, from the start.
    edit_defaults(&mut sim, ElecVariant::RopeLight, |p| {
        let r = &mut p.defaults_mut().rope;
        r.spacing = 4.0;
        r.center_lights = false;
    });
    elec(&mut sim, ElecVariant::RopeLight);
    let r = sim.drag((105.0, 14.0), (145.0, 14.0));
    assert_eq!(r.commit.as_deref(), Some("Place Rope Light"));
    assert_eq!(label(&sim).as_deref(), Some("Place Rope Light"));
    let lay = layer(&sim);
    assert_eq!(lay.ropes.len(), 1);
    let rope = lay.ropes[0].clone();
    assert!(
        devices(&sim).is_empty(),
        "a rope light is a path, not a device"
    );
    assert_eq!(rope.points.len(), 2);
    assert_eq!(rope.length(), 40.0);
    assert_eq!(
        rope.spec.height, bottom,
        "it hangs from the cabinet's bottom"
    );
    assert_eq!(rope.spec.spacing, 4.0);
    assert_eq!(rope.lights().len(), 11, "every 4\" from the start");
    // Edited like an open polyline: a midpoint handle adds a vertex, a vertex drags.
    let mid = Point::lerp(rope.points[0], rope.points[1], 0.5);
    let r = sim.drag((mid.x, mid.y), (mid.x, mid.y + 20.0));
    assert_eq!(r.commit.as_deref(), Some("Edit Rope Light"));
    let rope = layer(&sim).ropes[0].clone();
    assert_eq!(rope.points.len(), 3);
    assert!((rope.points[1].y - (mid.y + 20.0)).abs() < 1e-6);
    let end = rope.points[2];
    sim.drag((end.x, end.y), (end.x + 20.0, end.y));
    assert_eq!(label(&sim).as_deref(), Some("Edit Rope Light"));
    assert_eq!(layer(&sim).ropes[0].points[2].x, end.x + 20.0);
    // Double-click opens the Rope Light Specification; OK applies it.
    let r0 = layer(&sim).ropes[0].clone();
    let p = plan_core::geometry::Point::lerp(r0.points[0], r0.points[1], 0.25);
    let r = sim.double_click(p.x, p.y);
    assert!(r.consumed);
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.dialog_frame(true);
    sim.dialog_frame(false);
    assert_eq!(label(&sim).as_deref(), Some("Rope Light Specification"));
    // It is in the electrical schedule and the Materials List by length.
    let entries = plan_docs::schedule_kinds::entries(
        &sim.app.cx.project,
        plan_core::schedules::ScheduleKind::Electrical,
        None,
    );
    let row = entries
        .iter()
        .find(|e| e.name == "Rope Light")
        .expect("a schedule row");
    assert_eq!(
        row.cells.iter().find(|(k, _)| *k == "type").unwrap().1,
        "Rope Light"
    );
    sim.action(Action::Custom(ml::cmd::ALL));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    let lines = ml::with_state(|st| st.shown.clone());
    let rl = lines
        .iter()
        .find(|l| l.line.item.starts_with("Rope Light"))
        .expect("a Materials List row");
    assert_eq!(
        (rl.line.category.as_str(), rl.line.unit.as_str()),
        ("Electrical", "lf")
    );
    // The list rounds up to whole feet.
    let feet = layer(&sim).ropes[0].length() / 12.0;
    assert!(
        rl.line.quantity >= feet - 0.05 && rl.line.quantity < feet + 1.0,
        "{} for {feet}",
        rl.line.quantity
    );
    // Treat as One Object off: out of the schedule.
    let mut lay = layer(&sim);
    lay.ropes[0].spec.treat_as_object = false;
    save_electrical(&mut sim.app.cx.project, 0, &lay);
    let entries = plan_docs::schedule_kinds::entries(
        &sim.app.cx.project,
        plan_core::schedules::ScheduleKind::Electrical,
        None,
    );
    assert!(!entries.iter().any(|e| e.name == "Rope Light"));
    // Delete removes the selected rope light, in one undo step.
    sim.key(crate::tools::KeyEvent::key(eframe::egui::Key::Delete));
    assert!(layer(&sim).ropes.is_empty());
    assert_eq!(label(&sim).as_deref(), Some("Delete Rope Light"));
    sim.undo();
    assert_eq!(layer(&sim).ropes.len(), 1);
}

#[test]
fn a_tray_ceilings_rope_light_runs_come_back_as_closed_rope_lights() {
    let mut sim = house();
    sim.tool(ToolId::TrayCeiling);
    sim.click(W / 2.0, H / 2.0);
    let id = sim.app.cx.floor().tray_ids()[0];
    sim.app.cx.selection.set(ObjectRef::Cad(id));
    assert!(crate::tools::tray_ceiling::run_command(
        &mut sim.app.cx,
        crate::tools::tray_ceiling::cmd::SPEC
    ));
    crate::dialogs::tray_ceiling::with_dialog(|d| {
        d.draft_mut()
            .rope_lights
            .push(plan_core::tray::RopeLight::default())
    })
    .unwrap();
    crate::dialogs::tray_ceiling::accept_dialog(&mut sim.app.cx);
    let ropes = et::tray_rope_lights(&sim.app.cx);
    assert_eq!(ropes.len(), 1);
    assert!(ropes[0].closed && ropes[0].tray == id);
    assert!(ropes[0].length() > 100.0);
    let ceiling = sim.app.cx.floor().ceiling_height;
    assert!(ropes[0].top_elevation(ceiling) <= ceiling);
    assert!(ropes[0].lights().len() > 10);
}

#[test]
fn set_as_default_copies_a_device_a_spline_and_a_rope_light() {
    let mut sim = house();
    // A recessed can at 90": the Light tool will place cans from now on.
    let can = place(&mut sim, ElecVariant::RecessedLight, 244.0, 176.0);
    sim.app.cx.selection.set(ObjectRef::Device(can.id));
    sim.app.cx.run_custom(cmd::SET_DEFAULT);
    assert_eq!(label(&sim).as_deref(), Some("Set as Default"));
    let d = ElectricalDefaults::load(&sim.app.cx.project);
    assert_eq!(
        d.object("Light", DeviceKind::CeilingLight),
        DeviceKind::RecessedCan
    );
    // A 110V outlet at 20" on a wall: the Outlet height becomes 20".
    let o = place(&mut sim, ElecVariant::Outlet110, 100.0, 8.0);
    let mut lay = layer(&sim);
    lay.device_mut(o.id).unwrap().height = 20.0;
    save_electrical(&mut sim.app.cx.project, 0, &lay);
    sim.app.cx.selection.set(ObjectRef::Device(o.id));
    sim.app.cx.run_custom(cmd::SET_DEFAULT);
    assert_eq!(
        ElectricalDefaults::load(&sim.app.cx.project).outlet_height,
        20.0
    );
    // Change to GFCI Outlet and back.
    sim.app.cx.run_custom(cmd::TO_GFCI);
    assert_eq!(layer(&sim).device(o.id).unwrap().kind, DeviceKind::Gfci);
    sim.app.cx.run_custom(cmd::TO_110);
    assert_eq!(
        layer(&sim).device(o.id).unwrap().kind,
        DeviceKind::Outlet110
    );
    // A spline with a long, solid, arrowed style: the Electrical Connection tool's.
    let l = place(&mut sim, ElecVariant::Light, 120.0, 100.0);
    let sw = place(&mut sim, ElecVariant::Switch, 60.0, 8.0);
    elec(&mut sim, ElecVariant::Connection);
    drag(&mut sim, sw.position, l.position);
    let mut lay = layer(&sim);
    lay.connections[0].line_style = Some(plan_core::LineStyle::DashDot);
    lay.connections[0].arrow = Arrow::Both;
    save_electrical(&mut sim.app.cx.project, 0, &lay);
    assert!(et::run_command(&mut sim.app.cx, cmd::SET_DEFAULT));
    let c = ElectricalDefaults::load(&sim.app.cx.project).connection;
    assert_eq!(
        (c.line_style, c.arrow),
        (plan_core::LineStyle::DashDot, Arrow::Both)
    );
    let _ = ConnEnd::Start;
    // A rope light's specification becomes the Rope Light Defaults.
    elec(&mut sim, ElecVariant::RopeLight);
    sim.drag((300.0, 300.0), (360.0, 300.0));
    let mut lay = layer(&sim);
    lay.ropes[0].spec.spacing = 3.0;
    save_electrical(&mut sim.app.cx.project, 0, &lay);
    assert!(et::run_command(&mut sim.app.cx, cmd::SET_DEFAULT));
    assert_eq!(
        ElectricalDefaults::load(&sim.app.cx.project).rope.spacing,
        3.0
    );
    // Nothing new to set: no undo step.
    let n = label(&sim);
    assert!(et::run_command(&mut sim.app.cx, cmd::SET_DEFAULT));
    assert_eq!(label(&sim), n);
}

#[test]
fn auto_place_outlets_serves_appliances_and_sinks() {
    use plan_core::symbols::PlacedSymbol;
    let mut sim = house();
    let anchor = Point::new(W / 2.0, H / 2.0);
    let floor = &mut sim.app.cx.project.floors[0];
    floor.room_names.push(plan_core::model::RoomName::new(
        anchor, "Kitchen", "Kitchen",
    ));
    // Synthetic library items: a range and a dryer against the north wall,
    // a refrigerator in the corner and a sink against the south wall.
    for (id, x, y) in [
        ("test.appliances.range_30", 100.0, 20.0),
        ("test.appliances.dryer_27", 200.0, 20.0),
        ("test.appliances.refrigerator_36", 440.0, 20.0),
        ("test.plumbing.sink_double", 300.0, 340.0),
    ] {
        floor
            .symbols
            .push(PlacedSymbol::new(id, Point::new(x, y), 30.0, 26.0, 36.0));
    }
    sim.app.cx.refresh();
    et::auto_place_floor_outlets(&mut sim.app.cx);
    let d = devices(&sim);
    let n220 = d.iter().filter(|d| d.kind == DeviceKind::Outlet220).count();
    assert_eq!(n220, 2, "range and dryer get 220 V outlets");
    let behind_range = d
        .iter()
        .find(|d| d.kind == DeviceKind::Outlet220 && (d.position.x - 100.0).abs() < 1.0)
        .expect("an outlet behind the range");
    assert!(behind_range.wall_id.is_some() && behind_range.position.y < 10.0);
    assert!(
        d.iter().any(|d| d.kind == DeviceKind::Outlet110
            && (d.position.x - 440.0).abs() < 1.0
            && d.height == 12.0),
        "110 V behind the refrigerator"
    );
    let lights: Vec<_> = d
        .iter()
        .filter(|d| d.kind == DeviceKind::RecessedCan)
        .collect();
    assert_eq!(lights.len(), 1, "a light above the sink");
    assert_eq!(lights[0].position, Point::new(300.0, 340.0));
    assert_eq!(label(&sim).as_deref(), Some("Auto Place Outlets"));
    // One undo step takes outlets and light away.
    sim.undo();
    assert!(devices(&sim).is_empty());
}
