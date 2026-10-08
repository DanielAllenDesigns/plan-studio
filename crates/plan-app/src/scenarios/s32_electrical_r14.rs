//! Scenario 32 (round 14): electrical parity. Connect Electrical turns the
//! switches of a light into S3 / S4 switches, Auto Place Outlets follows the
//! NEC rules and adds the exterior weatherproof receptacles, device heights
//! come from the plan's Electrical Defaults, and the 110V / 220V / GFCI / WP /
//! Dedicated flags are drawn in the plan (CB-62..CB-68).

use super::{draw_shell, Sim};
use crate::editor::site_view::{load_electrical, save_electrical};
use crate::tools::electrical::ElecVariant;
use crate::tools::ToolId;
use plan_electrical::{Device, DeviceKind, ElectricalDefaults};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn devices(sim: &Sim) -> Vec<Device> {
    load_electrical(sim.app.cx.floor()).devices
}

fn elec(sim: &mut Sim, v: ElecVariant) {
    sim.tool(ToolId::ElectricalVariant(v));
}

fn kinds_of(sim: &Sim, ids: &[u64]) -> Vec<DeviceKind> {
    let layer = load_electrical(sim.app.cx.floor());
    ids.iter().map(|i| layer.device(*i).unwrap().kind).collect()
}

#[test]
fn connecting_a_second_and_third_switch_to_a_light_makes_s3_and_s4_switches() {
    let mut sim = house();
    elec(&mut sim, ElecVariant::Light);
    sim.click(244.0, 176.0);
    let light = devices(&sim)[0].clone();
    let mut sw = Vec::new();
    elec(&mut sim, ElecVariant::Switch);
    for x in [30.0, 60.0, 90.0] {
        sim.click(x, 8.0);
        sw.push(devices(&sim).last().unwrap().id);
    }
    elec(&mut sim, ElecVariant::Connection);
    let wire = |sim: &mut Sim, id: u64| {
        let p = load_electrical(sim.app.cx.floor())
            .device(id)
            .unwrap()
            .position;
        sim.click(p.x, p.y);
        sim.click(light.position.x, light.position.y);
    };
    wire(&mut sim, sw[0]);
    assert_eq!(
        kinds_of(&sim, &sw),
        [DeviceKind::Switch; 3],
        "one switch is S"
    );
    wire(&mut sim, sw[1]);
    assert_eq!(
        kinds_of(&sim, &sw),
        [
            DeviceKind::Switch3Way,
            DeviceKind::Switch3Way,
            DeviceKind::Switch
        ],
        "{}",
        sim.app.cx.status
    );
    assert!(sim.app.cx.status.contains("3-way"), "{}", sim.app.cx.status);
    wire(&mut sim, sw[2]);
    assert_eq!(
        kinds_of(&sim, &sw),
        [
            DeviceKind::Switch3Way,
            DeviceKind::Switch4Way,
            DeviceKind::Switch3Way
        ]
    );
    let layer = load_electrical(sim.app.cx.floor());
    assert_eq!(layer.device(light.id).unwrap().switched_by, sw);
    // The wiring is one undo step each, and the symbols follow.
    sim.undo();
    assert_eq!(
        kinds_of(&sim, &sw),
        [
            DeviceKind::Switch3Way,
            DeviceKind::Switch3Way,
            DeviceKind::Switch
        ]
    );
    sim.undo();
    assert_eq!(kinds_of(&sim, &sw), [DeviceKind::Switch; 3]);
}

#[test]
fn auto_place_outlets_adds_the_exterior_weatherproof_receptacles_in_one_step() {
    let mut sim = house();
    elec(&mut sim, ElecVariant::AutoOutlets);
    sim.click(240.0, 180.0);
    let all = devices(&sim);
    let wp: Vec<&Device> = all.iter().filter(|d| d.kind.is_weatherproof()).collect();
    assert_eq!(wp.len(), 2, "front and back: {}", sim.app.cx.status);
    assert!(
        sim.app.cx.status.contains("weatherproof"),
        "{}",
        sim.app.cx.status
    );
    // Outside the shell: below the south wall and above the north wall.
    assert!(wp.iter().any(|d| d.position.y < 0.0));
    assert!(wp.iter().any(|d| d.position.y > H));
    assert!(wp.iter().all(|d| d.height == 18.0));
    // The rest are 110V general receptacles at 12" inside the house.
    for d in all.iter().filter(|d| !d.kind.is_weatherproof()) {
        assert_eq!(d.kind, DeviceKind::Outlet110);
        assert_eq!(d.height, 12.0);
        assert!(d.position.y > 0.0 && d.position.y < H, "{:?}", d.position);
    }
    // One undo step takes all of them away; running twice adds nothing.
    let n = all.len();
    sim.click(240.0, 180.0);
    assert_eq!(devices(&sim).len(), n);
    assert_eq!(sim.undo().as_deref(), Some("Auto Place Outlets"));
    assert!(devices(&sim).is_empty());
}

#[test]
fn device_heights_come_from_the_plans_electrical_defaults() {
    let mut sim = house();
    elec(&mut sim, ElecVariant::Outlet110);
    sim.click(100.0, 8.0);
    assert_eq!(devices(&sim)[0].height, 12.0);
    let mut defaults = ElectricalDefaults::default();
    defaults.set_height(DeviceKind::Outlet110, 18.0);
    defaults.set_height(DeviceKind::Switch, 42.0);
    defaults.set_counter_height(40.0);
    defaults.store(&mut sim.app.cx.project);
    // The status bar shows the height the next click will use.
    elec(&mut sim, ElecVariant::Outlet110);
    sim.move_to(150.0, 8.0);
    assert_eq!(sim.app.cx.readout.as_deref(), Some("Height: 18\""));
    sim.click(150.0, 8.0);
    elec(&mut sim, ElecVariant::Switch);
    sim.click(30.0, 8.0);
    let d = devices(&sim);
    assert_eq!(d[1].height, 18.0);
    assert_eq!(d[2].height, 42.0);
    // The other kinds keep their own heights.
    elec(&mut sim, ElecVariant::Gfci);
    sim.click(200.0, 8.0);
    assert_eq!(devices(&sim)[3].height, 12.0);
    // Auto Place Outlets uses them too.
    let mut sim2 = house();
    let mut defaults = ElectricalDefaults::default();
    defaults.set_height(DeviceKind::Outlet110, 16.0);
    defaults.store(&mut sim2.app.cx.project);
    elec(&mut sim2, ElecVariant::AutoOutlets);
    sim2.click(240.0, 180.0);
    assert!(devices(&sim2)
        .iter()
        .filter(|d| d.kind == DeviceKind::Outlet110)
        .all(|d| d.height == 16.0));
}

#[test]
fn the_flag_symbols_are_drawn_in_the_plan() {
    let mut sim = house();
    for (v, x, tag) in [
        (ElecVariant::OutletWp, 60.0, "WP"),
        (ElecVariant::Gfci, 120.0, "GFCI"),
        (ElecVariant::Outlet220, 180.0, "220V"),
        (ElecVariant::OutletDedicated, 240.0, "DED"),
    ] {
        elec(&mut sim, v);
        sim.click(x, 8.0);
        let _ = tag;
    }
    assert_eq!(devices(&sim).len(), 4, "{}", sim.app.cx.status);
    let texts: Vec<String> = sim
        .plan_shapes()
        .iter()
        .filter_map(|s| match s {
            eframe::egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect();
    for tag in ["WP", "GFCI", "220V", "DED"] {
        assert!(texts.iter().any(|t| t == tag), "{tag} in {texts:?}");
    }
    // Chief's dedicated receptacle is a single blade, GFCI and WP are 110V duplexes.
    assert_eq!(DeviceKind::OutletWp.voltage(), Some(110));
    assert_eq!(DeviceKind::Outlet220.voltage(), Some(220));
}

#[test]
fn the_new_flavors_activate_and_stay_on_the_wall() {
    let mut sim = house();
    for (v, x) in [
        (ElecVariant::OutletWp, 300.0),
        (ElecVariant::OutletDedicated, 380.0),
    ] {
        elec(&mut sim, v);
        assert_eq!(sim.app.tools.active().name(), v.name());
        assert!(!sim.app.tools.active().hint().is_empty());
        let n = devices(&sim).len();
        sim.click(240.0, 180.0); // off every wall
        assert_eq!(devices(&sim).len(), n, "{v:?} needs a wall");
        sim.click(x, 8.0);
        assert_eq!(devices(&sim).len(), n + 1);
    }
    let ids: Vec<u64> = devices(&sim).iter().map(|d| d.id).collect();
    let mut layer = load_electrical(sim.app.cx.floor());
    layer.devices.iter_mut().for_each(|d| d.circuit = Some(3));
    save_electrical(&mut sim.app.cx.project, 0, &layer);
    assert_eq!(ids.len(), 2);
}
