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
fn schedule_from_room_lists_the_lighting_of_one_room() {
    use crate::editor::schedule_view as sv;
    let mut sim = lit();
    assert!(!sim.app.cx.rooms.is_empty());
    let id = assert_one_undo_step(&mut sim, "schedule", |s| {
        sv::create_from_room(
            &mut s.app.cx,
            plan_core::schedules::ScheduleKind::Electrical,
            0,
            plan_core::geometry::Point::new(0.0, 700.0),
        )
    })
    .expect("created");
    assert_eq!(sv::find(&sim.app.cx, id).unwrap().rooms.len(), 1);
}

#[test]
fn a_leader_reads_the_door_it_points_at() {
    use plan_core::geometry::Point;
    let mut sim = lit();
    let (a, b) = {
        let w = &sim.app.cx.floor().walls[0];
        (w.start, w.end)
    };
    let wid = sim.app.cx.floor().walls[0].id;
    let door = sim
        .app
        .cx
        .project
        .add_opening(0, wid, 60.0, plan_core::OpeningKind::Door)
        .unwrap();
    sim.app.cx.refresh();
    let o = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == door)
        .unwrap();
    let mid = (o.start_offset() + o.end_offset()) / 2.0;
    let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    let tip = Point::new(a.x + (b.x - a.x) / len * mid, a.y + (b.y - a.y) / len * mid);
    let facts = plan_core::macros::reference_facts(&sim.app.cx.project, 0, tip, &[]);
    assert!(
        facts.iter().any(|(k, _)| k == "width"),
        "the arrow tip finds the door: {facts:?}"
    );
}

#[test]
fn a_light_outside_the_wall_is_the_exterior_wall_light() {
    let mut sim = lit();
    put(&mut sim, ElecVariant::Light, 200.0, -8.0);
    let d = devices(&sim);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].kind, plan_electrical::DeviceKind::WallLightExterior);
}
