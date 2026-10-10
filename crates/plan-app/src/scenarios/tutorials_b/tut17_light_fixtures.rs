//! Lesson 17, Light Fixtures (pp. 298-317). Real: ceiling lights placed on
//! a 40 in pitch, a wall light that finds its wall. Open: Enter Coordinates
//! copies, Create Schedule from Room and the leader macro.
use crate::editor::site_view::load_electrical;
use crate::scenarios::tutorials_support::*;
use crate::scenarios::Sim;
use crate::tools::electrical::ElecVariant;
use crate::tools::ToolId;
use plan_electrical::Device;

fn lit() -> Sim {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    chic_cottage(&mut sim);
    sim
}

fn devices(sim: &Sim) -> Vec<Device> {
    load_electrical(sim.app.cx.floor()).devices
}

fn put(sim: &mut Sim, v: ElecVariant, x: f64, y: f64) {
    sim.tool(ToolId::ElectricalVariant(v));
    sim.click(x, y);
}

#[test]
fn ceiling_lights_keep_a_forty_inch_pitch() {
    let mut sim = lit();
    for x in [200.0, 240.0, 280.0] {
        assert_one_undo_step(&mut sim, "light", |s| put(s, ElecVariant::Light, x, 200.0));
    }
    let d = devices(&sim);
    assert_eq!(d.len(), 3);
    let mut xs: Vec<f64> = d.iter().map(|d| d.position.x).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!((xs[1] - xs[0] - 40.0).abs() < 1e-6 && (xs[2] - xs[1] - 40.0).abs() < 1e-6);
}

#[test]
fn a_wall_light_mounts_on_the_wall_it_is_clicked_against() {
    let mut sim = lit();
    assert_one_undo_step(&mut sim, "sconce", |s| {
        put(s, ElecVariant::WallLight, 200.0, 6.0)
    });
    let d = devices(&sim);
    assert_eq!(d.len(), 1);
    assert!(d[0].wall_id.is_some(), "the sconce is hosted by a wall");
}

#[test]
#[ignore = "T7-17: E-33 (Copy + Enter Coordinates 40 in; Multiple Copy intervals 50 / 52)"]
fn copy_by_typed_coordinates() {
    assert_ignored_break("E-33");
}

#[test]
#[ignore = "T7-17: R-105 (Create Schedule from Room: Kitchen Lighting Schedule)"]
fn schedule_from_room() {
    assert_ignored_break("R-105");
}

#[test]
#[ignore = "T7-17: TXT-65 (leader text %description% reads the object it points at)"]
fn leader_description_macro() {
    assert_ignored_break("TXT-65");
}

#[test]
#[ignore = "T7-17: E-31 (exterior wall light default on the deck: Wide Brim Sconce)"]
fn exterior_wall_light_default() {
    assert_ignored_break("E-31");
}
