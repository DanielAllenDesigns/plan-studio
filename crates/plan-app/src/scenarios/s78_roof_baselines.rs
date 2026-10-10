//! Scenario 78: Roof Baseline Polylines, Roof Groups, the Build Roof retain
//! switches and curved roof planes (manual pp. 826 to 829, 847 to 857;
//! RF-9, RF-37, RF-61, RF-62, RF-66..RF-72, RF-125..RF-127, R-114).
//!
//! Make baseline polylines from the walls, draw a porch polyline by hand,
//! give its wall side Against Wall and build the roof from the baselines;
//! put the garage in its own Roof Group; rebuild while keeping a manual plane
//! and an edited one; turn a plane into a barrel. Every action is one undo
//! step.

use super::{draw_shell, Sim};
use crate::dialogs::roof_baseline as dlg;
use crate::editor::roof_view::{self, RoofPlaneRecord};
use crate::editor::rooms_edit::{self, RoomExtras};
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::tools::roof::RoofMode;
use crate::tools::roof_baseline as rb;
use crate::tools::{KeyEvent, ToolId};
use plan_core::geometry::Point;
use plan_core::{RoomName, WallKind};
use plan_roof::{CurvedSpec, JoinLock};

const W: f64 = 480.0;
const H: f64 = 288.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    draw_shell(&mut sim, W, H);
    sim
}

fn planes(sim: &Sim) -> Vec<RoofPlaneRecord> {
    roof_view::load(sim.app.cx.floor()).planes
}

fn label(sim: &Sim) -> Option<String> {
    sim.app.cx.undo_label().map(String::from)
}

/// Build Roof the way a user does: pick the tool, click, OK the dialog.
fn build_roof(sim: &mut Sim) {
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(W / 2.0, H / 2.0);
    sim.ok();
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
}

fn top(planes: &[RoofPlaneRecord]) -> f64 {
    planes
        .iter()
        .flat_map(|p| p.polygon3d.iter())
        .fold(f64::MIN, |m, v| m.max(v[1]))
}

/// Edits the stored Build Roof settings (what the dialog opens with).
fn settings(sim: &mut Sim, f: impl FnOnce(&mut roof_view::RoofSettings)) {
    let mut set = roof_view::load(sim.app.cx.floor());
    let mut s = set.settings.clone().expect("a roof was built");
    f(&mut s);
    set.settings = Some(s);
    let fl = sim.app.cx.floor;
    roof_view::store(&mut sim.app.cx.project, fl, &mut set);
}

// ---------------------------------------------------------------------------
// Roof Baseline Polylines
// ---------------------------------------------------------------------------

#[test]
#[ignore = "R16-19 in progress"]
fn make_baselines_draw_a_porch_and_build_the_roof_from_them() {
    let mut sim = house();
    build_roof(&mut sim);
    assert_eq!(planes(&sim).len(), 4, "a hip roof over the shell");

    // Make Roof Baseline Polylines deletes the roof and makes the polyline.
    assert!(rb::make_polylines(&mut sim.app.cx));
    assert_eq!(label(&sim).as_deref(), Some("Make Roof Baseline Polylines"));
    assert!(planes(&sim).is_empty(), "the roof is gone");
    let made = rb::baselines(sim.app.cx.floor());
    assert_eq!(made.len(), 1);
    let house_line = &made[0];
    // The polyline lies on the outside of the exterior walls: it holds the
    // shell and its wall faces.
    let area = plan_core::geometry::polygon_area(&house_line.points);
    assert!(area > W * H, "area {area}");
    assert!(house_line.spec.edges.iter().all(|e| e.rises()));
    assert!(house_line
        .spec
        .edges
        .iter()
        .all(|e| (e.overhang - 16.0).abs() < 1e-6));
    let (south, north) = {
        let ys: Vec<f64> = house_line.points.iter().map(|p| p.y).collect();
        (
            ys.iter().cloned().fold(f64::MAX, f64::min),
            ys.iter().cloned().fold(f64::MIN, f64::max),
        )
    };
    assert!(north - south > H);
    // The polyline is a CAD polyline on the Roofs, Baseline Polylines layer.
    let cad = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == house_line.id)
        .unwrap();
    assert_eq!(cad.layer, rb::LAYER);

    // Draw a porch polyline south of the house with the tool: four corners,
    // then the first corner closes it.
    sim.tool(ToolId::RoofBaseline);
    assert_eq!(sim.app.tools.active().name(), "Roof Baseline Polyline");
    let (x0, x1, y1, y0) = (120.0, 360.0, south, south - 96.0);
    for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
        sim.click(x, y);
    }
    let r = sim.click(x0, y0);
    assert_eq!(r.commit.as_deref(), Some("Roof Baseline Polyline"));
    assert_eq!(label(&sim).as_deref(), Some("Roof Baseline Polyline"));
    let porch_id = rb::baselines(sim.app.cx.floor())
        .iter()
        .map(|b| b.id)
        .find(|id| *id != house_line.id)
        .expect("the porch polyline");

    // Its wall side (the north edge) is Against Wall; the dialog edits it.
    sim.tool(ToolId::Select);
    let b = rb::baseline(sim.app.cx.floor(), porch_id).unwrap();
    let north_edge = (0..b.edge_count())
        .find(|&i| {
            let (p, q) = b.edge(i);
            (p.y - y1).abs() < 1e-6 && (q.y - y1).abs() < 1e-6
        })
        .unwrap_or_else(|| panic!("the north edge {y1}: {:?}", b.points));
    assert!(dlg::open_spec(&sim.app.cx, porch_id, Some(north_edge)));
    dlg::with_dialog(|d| {
        assert_eq!(d.selected_edge(), north_edge);
        d.spec_mut().edges[north_edge].against_wall = true;
        d.spec_mut().height = 90.0;
    })
    .unwrap();
    assert!(dlg::accept_dialog(&mut sim.app.cx));
    assert_eq!(label(&sim).as_deref(), Some("Roof Baseline Specification"));
    let b = rb::baseline(sim.app.cx.floor(), porch_id).unwrap();
    assert!(b.spec.edges[north_edge].against_wall && b.spec.height == 90.0);
    assert_eq!(
        plan_roof::directive_text(&b.spec.edges[north_edge]),
        "V (vert)"
    );

    // Build Roof Planes with Use Existing Roof Baselines: the house hips and
    // the porch's three free sides.
    assert!(rb::build_from_baselines(&mut sim.app.cx));
    assert_eq!(label(&sim).as_deref(), Some("Build Roof"));
    let built = planes(&sim);
    assert_eq!(
        built.len(),
        4 + 3,
        "{:?}",
        built.iter().map(|p| p.source).collect::<Vec<_>>()
    );
    assert!(built.iter().all(|p| p.auto));
    // The porch roof is lower than the house roof.
    let porch_top = top(&built
        .iter()
        .filter(|p| p.plan_polygon().iter().all(|q| q.y < south - 1.0))
        .cloned()
        .collect::<Vec<_>>());
    assert!(porch_top < top(&built) - 1.0);

    // One undo step each: the build, the specification, the porch, the make.
    assert_eq!(sim.undo().as_deref(), Some("Build Roof"));
    assert!(planes(&sim).is_empty());
    assert_eq!(sim.undo().as_deref(), Some("Roof Baseline Specification"));
    assert!(
        !rb::baseline(sim.app.cx.floor(), porch_id)
            .unwrap()
            .spec
            .edges[north_edge]
            .against_wall
    );
    assert_eq!(sim.undo().as_deref(), Some("Roof Baseline Polyline"));
    assert_eq!(rb::baselines(sim.app.cx.floor()).len(), 1);
    assert_eq!(sim.undo().as_deref(), Some("Make Roof Baseline Polylines"));
    assert_eq!(planes(&sim).len(), 4, "the walls' own roof is back");
    assert!(rb::baselines(sim.app.cx.floor()).is_empty());
}

#[test]
fn the_build_roof_dialog_carries_the_new_switches() {
    let mut sim = house();
    build_roof(&mut sim);
    settings(&mut sim, |s| {
        s.switches.retain_manual = false;
        s.switches.segment_angle = 30.0;
        s.switches.pitch_in_degrees = true;
        s.switches.show_all_ridges = false;
    });
    // They survive a trip through the stored roof.
    let s = roof_view::load(sim.app.cx.floor()).settings.unwrap();
    assert!(!s.switches.retain_manual && s.switches.retain_edited);
    assert_eq!(s.switches.segment_angle, 30.0);
    assert!(s.switches.pitch_in_degrees && !s.switches.show_all_ridges);
    // Loading the roof sets what dialogs and labels show: degrees.
    assert_eq!(roof_view::pitch_label(12.0), "45\u{b0}");
    // A rebuild keeps the switches.
    build_roof(&mut sim);
    let s = roof_view::load(sim.app.cx.floor()).settings.unwrap();
    assert_eq!(s.switches.segment_angle, 30.0);
    plan_roof::set_pitch_display_degrees(false);
}

// ---------------------------------------------------------------------------
// Roof Groups
// ---------------------------------------------------------------------------

/// A house and a garage of one height sharing a wall, drawn as an L with a
/// partition: two rooms.
fn house_and_garage() -> Sim {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    let cx = &mut sim.app.cx;
    let c = [
        (0.0, 0.0),
        (540.0, 0.0),
        (540.0, 240.0),
        (300.0, 240.0),
        (300.0, 288.0),
        (0.0, 288.0),
    ];
    for i in 0..c.len() {
        let (a, b) = (c[i], c[(i + 1) % c.len()]);
        cx.project.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            6.0,
            96.0,
            WallKind::Exterior,
        );
    }
    cx.project.add_wall(
        0,
        Point::new(300.0, 0.0),
        Point::new(300.0, 240.0),
        4.0,
        96.0,
        WallKind::Interior,
    );
    cx.refresh();
    sim
}

fn room_index_at(sim: &Sim, p: Point) -> usize {
    sim.app
        .cx
        .rooms
        .iter()
        .position(|r| r.contains(p))
        .expect("a room there")
}

#[test]
fn a_roof_group_roofs_the_garage_as_its_own_building() {
    let mut sim = house_and_garage();
    assert_eq!(sim.app.cx.rooms.len(), 2);
    build_roof(&mut sim);
    // One integrated system over the L: one plane per exterior edge.
    let joined = planes(&sim);
    assert_eq!(joined.len(), 6);

    // Put the garage (the east room) in Roof Group 1 through the Room
    // Specification's apply step: one undo step.
    let garage = room_index_at(&sim, Point::new(420.0, 120.0));
    let anchor = rooms_edit::room_anchor(&sim.app.cx.rooms[garage]);
    let mut draft = RoomName::new(anchor, "Garage", "Garage");
    draft.roof_group = 1;
    let extras = RoomExtras::from_defaults(&sim.app.cx.defaults);
    assert!(rooms_edit::apply_room_spec(
        &mut sim.app.cx,
        garage,
        &draft,
        &extras
    ));
    assert_eq!(label(&sim).as_deref(), Some("Room Specification"));
    let stored = sim
        .app
        .cx
        .floor()
        .room_names
        .iter()
        .find(|n| n.anchor == anchor)
        .unwrap();
    assert_eq!(stored.roof_group, 1);

    // Auto Rebuild notices the group (it is part of the signature); the
    // rebuilt roof is two buildings: a hip roof over each rectangle.
    assert!(roof_view::auto_rebuild(&mut sim.app.cx));
    let split = planes(&sim);
    assert_eq!(split.len(), 4 + 4, "two separate hip roofs");
    let east: Vec<RoofPlaneRecord> = split
        .iter()
        .filter(|p| p.centroid().x > 306.0)
        .cloned()
        .collect();
    let west: Vec<RoofPlaneRecord> = split
        .iter()
        .filter(|p| p.centroid().x <= 306.0)
        .cloned()
        .collect();
    assert_eq!(east.len(), 4);
    assert_eq!(west.len(), 4);
    // The garage is 240" deep and the house 288": each has its own ridge.
    assert!(top(&west) > top(&east) + 1.0);

    // Undoing the room step brings back the one integrated roof.
    assert_eq!(sim.undo().as_deref(), Some("Room Specification"));
    assert_eq!(
        sim.app
            .cx
            .floor()
            .room_names
            .iter()
            .filter(|n| n.roof_group != 0)
            .count(),
        0
    );
}

// ---------------------------------------------------------------------------
// Retain switches
// ---------------------------------------------------------------------------

fn add_manual_plane(sim: &mut Sim) -> u64 {
    let fl = sim.app.cx.floor;
    sim.app.cx.begin_change("Roof Plane");
    let mut set = roof_view::load(sim.app.cx.floor());
    let id = sim.app.cx.project.alloc_id();
    // A small shed plane over the north-west corner, high above the roof so
    // it never coincides with an automatic plane.
    let poly = vec![
        [20.0, 300.0, -250.0],
        [100.0, 300.0, -250.0],
        [100.0, 330.0, -270.0],
        [20.0, 330.0, -270.0],
    ];
    set.planes.push(RoofPlaneRecord::new(
        id,
        poly,
        18.0,
        (Point::new(20.0, 250.0), Point::new(100.0, 250.0)),
    ));
    roof_view::store(&mut sim.app.cx.project, fl, &mut set);
    sim.app.cx.mark_dirty();
    id
}

#[test]
fn rebuild_keeps_a_manual_plane_only_while_retain_manual_is_on() {
    let mut sim = house();
    build_roof(&mut sim);
    let manual = add_manual_plane(&mut sim);
    assert_eq!(planes(&sim).len(), 5);

    // Retain Manually Drawn Roof Planes (the default): the plane stays and
    // the four automatic planes are replaced.
    build_roof(&mut sim);
    assert_eq!(label(&sim).as_deref(), Some("Build Roof"));
    let after = planes(&sim);
    assert_eq!(after.len(), 5);
    assert!(after.iter().any(|p| p.id == manual && !p.auto));
    // One undo step puts the previous roof back.
    assert_eq!(sim.undo().as_deref(), Some("Build Roof"));
    assert!(planes(&sim).iter().any(|p| p.id == manual));

    // Off: the rebuild replaces the manual plane too.
    settings(&mut sim, |s| s.switches.retain_manual = false);
    build_roof(&mut sim);
    let after = planes(&sim);
    assert_eq!(after.len(), 4);
    assert!(after.iter().all(|p| p.id != manual && p.auto));
}

#[test]
fn an_edited_plane_is_kept_and_its_rebuilt_twin_is_dropped() {
    let mut sim = house();
    build_roof(&mut sim);
    let before = planes(&sim);
    // Stretch one plane's eave end along the eave: the plane is edited
    // (no longer automatic) but is still the same surface.
    let target = before
        .iter()
        .find(|p| (p.baseline.0.y - p.baseline.1.y).abs() < 1e-6 && p.baseline.0.y < 0.0)
        .cloned()
        .expect("the south plane");
    let fl = sim.app.cx.floor;
    sim.app.cx.begin_change("Edit Roof Plane");
    let mut set = roof_view::load(sim.app.cx.floor());
    let moved_to = Point::new(target.baseline.1.x - 40.0, target.baseline.1.y);
    set.plane_mut(target.id).unwrap().move_vertex(1, moved_to);
    roof_view::store(&mut sim.app.cx.project, fl, &mut set);
    let edited = planes(&sim)
        .into_iter()
        .find(|p| p.id == target.id)
        .unwrap();
    assert!(!edited.auto && edited.source.is_some());

    // Rebuild with Retain Edited Automatic Roof Planes: the edited plane
    // stays and the new south plane, coplanar with it, is not made.
    build_roof(&mut sim);
    let after = planes(&sim);
    assert_eq!(after.len(), 4, "the rebuilt twin is dropped");
    let kept = after.iter().find(|p| p.id == target.id).expect("kept");
    assert!(!kept.auto);
    assert_eq!(kept.baseline.1, moved_to, "the edit survives");

    // Off: the rebuild replaces it with an automatic plane.
    settings(&mut sim, |s| s.switches.retain_edited = false);
    build_roof(&mut sim);
    let after = planes(&sim);
    assert_eq!(after.len(), 4);
    assert!(after.iter().all(|p| p.auto));
}

// ---------------------------------------------------------------------------
// Curved roof planes
// ---------------------------------------------------------------------------

fn plane_meshes(sim: &Sim, id: u64) -> usize {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .count()
}

#[test]
fn a_plane_becomes_a_barrel_in_one_undo_step() {
    let mut sim = house();
    build_roof(&mut sim);
    let p = planes(&sim)
        .into_iter()
        .find(|p| (p.baseline.0.y - p.baseline.1.y).abs() < 1e-6 && p.baseline.0.y < 0.0)
        .unwrap();
    let flat = plane_meshes(&sim, p.id);
    let spec = CurvedSpec::straight(p.pitch).with_eave_angle(p.pitch, 40.0);
    assert!(rb::set_curved(&mut sim.app.cx, p.id, Some(spec)));
    assert_eq!(label(&sim).as_deref(), Some("Curved Roof Plane"));
    let rec = planes(&sim).into_iter().find(|q| q.id == p.id).unwrap();
    assert!(rec.curved.is_some() && !rec.auto, "curving edits the plane");
    // The eave and the top edge keep their heights.
    let heights = |r: &RoofPlaneRecord| {
        r.polygon3d
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), v| {
                (lo.min(v[1]), hi.max(v[1]))
            })
    };
    assert_eq!(heights(&rec), heights(&p));
    // The 3D view draws it in facets: more meshes than one flat slab.
    assert!(plane_meshes(&sim, p.id) > flat);
    // And back.
    assert_eq!(sim.undo().as_deref(), Some("Curved Roof Plane"));
    assert!(planes(&sim)
        .iter()
        .find(|q| q.id == p.id)
        .unwrap()
        .curved
        .is_none());
    assert_eq!(plane_meshes(&sim, p.id), flat);
}

#[test]
fn joining_a_curved_plane_asks_what_to_keep() {
    let mut sim = house();
    build_roof(&mut sim);
    let south = planes(&sim)
        .into_iter()
        .find(|p| (p.baseline.0.y - p.baseline.1.y).abs() < 1e-6 && p.baseline.0.y < 0.0)
        .unwrap();
    let spec = CurvedSpec::straight(south.pitch).with_eave_angle(south.pitch, 40.0);
    assert!(rb::set_curved(&mut sim.app.cx, south.id, Some(spec)));
    assert!(rb::join_asks(&sim.app.cx, south.id));

    // The east plane is the one to join to; ask through the dialog and keep
    // the angle at the ridge.
    let east = planes(&sim)
        .into_iter()
        .find(|p| (p.baseline.0.x - p.baseline.1.x).abs() < 1e-6 && p.baseline.0.x > W / 2.0)
        .unwrap();
    dlg::open_join_curved(&sim.app.cx, 0, south.id, 1, east.id);
    assert!(dlg::join_dialog_open());
    dlg::with_join_dialog(|d| {
        assert_eq!(
            d.lock(),
            JoinLock::Radius,
            "the first choice is the default"
        );
        d.set_lock(JoinLock::AngleAtRidge);
    })
    .unwrap();
    let ridge_before = planes(&sim)
        .into_iter()
        .find(|q| q.id == south.id)
        .unwrap()
        .curved
        .unwrap()
        .angle_at_ridge;
    assert!(dlg::accept_join(&mut sim.app.cx));
    assert_eq!(label(&sim).as_deref(), Some("Join Roof Planes"));
    let joined = planes(&sim).into_iter().find(|q| q.id == south.id).unwrap();
    let c = joined.curved.unwrap();
    assert!(
        (c.angle_at_ridge - ridge_before).abs() < 1e-6,
        "the angle at the ridge is locked"
    );
    assert_eq!(sim.undo().as_deref(), Some("Join Roof Planes"));
}

#[test]
fn escape_drops_a_pending_polyline_and_a_crossing_one_is_refused() {
    let mut sim = house();
    sim.tool(ToolId::RoofBaseline);
    sim.click(100.0, -200.0);
    sim.click(200.0, -200.0);
    sim.click(200.0, -100.0);
    assert!(sim.esc().consumed);
    assert!(rb::baselines(sim.app.cx.floor()).is_empty());
    // A bow tie crosses itself: Enter refuses it.
    for (x, y) in [
        (100.0, -200.0),
        (220.0, -100.0),
        (220.0, -200.0),
        (100.0, -100.0),
    ] {
        sim.click(x, y);
    }
    let r = sim.key(KeyEvent::key(egui::Key::Enter));
    assert!(r.commit.is_none());
    assert!(rb::baselines(sim.app.cx.floor()).is_empty());
    assert!(sim.app.cx.status.contains("crosses itself"));
}

use eframe::egui;
