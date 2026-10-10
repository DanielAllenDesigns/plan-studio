//! Scenario 77: roof eave alignment (Round 16 brief 18; manual pp. 829,
//! 830, 844; RF-76, RF-112). A mixed-pitch roof built with Same Roof Height
//! at Exterior Walls, Same Height Eaves and neither, and the settings kept
//! with the roof. Pivot locks and birdsmouth arithmetic are unit-tested in
//! `plan-roof` (`tests.rs`).

use super::{draw_shell, Sim};
use crate::editor::roof_view::{self, RoofPlaneRecord, RoofSettings};
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;
use plan_roof::{HeightSettings, RoofFraming};

const W: f64 = 480.0;
const H: f64 = 360.0;

/// A rectangular house whose long walls rise at 12:12 and short walls at 6:12.
fn mixed_pitch_house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    for w in &mut sim.app.cx.project.floors[0].walls {
        let long = (w.start.y - w.end.y).abs() < 1e-6;
        w.roof.pitch_in_12 = Some(if long { 12.0 } else { 6.0 });
    }
    sim
}

fn build(sim: &mut Sim, heights: HeightSettings) -> Vec<RoofPlaneRecord> {
    let fl = sim.app.cx.floor;
    let mut s = RoofSettings::from_defaults(&sim.app.cx.defaults);
    s.heights = heights;
    roof_view::rebuild(&mut sim.app.cx.project, fl, s, false).expect("roof built");
    roof_view::load(sim.app.cx.floor()).planes
}

/// Elevation of a plane over the outside face of its wall: the eave tip
/// plus the overhang climbed at the pitch.
fn wall_height(r: &RoofPlaneRecord) -> f64 {
    eave_height(r) + r.overhang * r.pitch / 12.0
}

fn eave_height(r: &RoofPlaneRecord) -> f64 {
    r.polygon3d[0][1]
}

fn spread(v: impl Iterator<Item = f64>) -> f64 {
    let v: Vec<f64> = v.collect();
    v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min)
}

#[test]
fn same_roof_height_changes_overhangs_so_mixed_pitches_meet_at_the_wall_and_the_eave() {
    let mut sim = mixed_pitch_house();
    let planes = build(&mut sim, HeightSettings::default());
    assert_eq!(planes.len(), 4);
    assert!(
        spread(planes.iter().map(wall_height)) < 1e-6,
        "walls bear at one height"
    );
    assert!(
        spread(planes.iter().map(eave_height)) < 1e-6,
        "eaves meet at one height"
    );
    // Default plane: 8:12 with a 16" overhang. The 12:12 planes overhang
    // less, the 6:12 planes more, so the fascia drop is the same.
    for p in &planes {
        let want = 16.0 * 8.0 / p.pitch;
        assert!(
            (p.overhang - want).abs() < 1e-6,
            "{}:12 overhang {}",
            p.pitch,
            p.overhang
        );
    }
}

#[test]
fn same_height_eaves_keeps_the_wall_overhangs_and_moves_the_planes() {
    let mut sim = mixed_pitch_house();
    let planes = build(
        &mut sim,
        HeightSettings {
            same_roof_height: false,
            same_height_eaves: true,
            ..HeightSettings::default()
        },
    );
    assert!(planes.iter().all(|p| (p.overhang - 16.0).abs() < 1e-6));
    assert!(
        spread(planes.iter().map(eave_height)) < 1e-6,
        "one eave height"
    );
    // Equal overhangs at unequal pitches: the steeper plane bears higher.
    assert!(spread(planes.iter().map(wall_height)) > 7.9);
    let steep = planes.iter().map(wall_height).fold(f64::MIN, f64::max);
    let flat = planes.iter().map(wall_height).fold(f64::MAX, f64::min);
    assert!((steep - flat - (16.0 * 12.0 / 12.0 - 16.0 * 6.0 / 12.0)).abs() < 1e-6);
}

#[test]
fn a_single_pitch_roof_is_independent_and_keeps_a_wall_overhang() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    for w in &mut sim.app.cx.project.floors[0].walls {
        w.roof.pitch_in_12 = Some(12.0);
        w.roof.overhang = Some(24.0);
    }
    let planes = build(&mut sim, HeightSettings::default());
    assert!(planes.iter().all(|p| (p.overhang - 24.0).abs() < 1e-6));
    // Both switches on adjust it to the default plane's fascia drop.
    let planes = build(
        &mut sim,
        HeightSettings {
            same_height_eaves: true,
            ..HeightSettings::default()
        },
    );
    assert!(planes
        .iter()
        .all(|p| (p.overhang - 16.0 * 8.0 / 12.0).abs() < 1e-6));
}

#[test]
fn heel_height_lifts_a_truss_roof_and_the_cut_a_rafter_roof() {
    let mut sim = mixed_pitch_house();
    let base = build(&mut sim, HeightSettings::default());
    let top = |v: &[RoofPlaneRecord]| v.iter().map(wall_height).fold(f64::MIN, f64::max);
    let heel = build(
        &mut sim,
        HeightSettings {
            framing: RoofFraming::Trusses,
            heel_height: 5.0,
            ..HeightSettings::default()
        },
    );
    assert!((top(&heel) - top(&base) - 5.0).abs() < 1e-6);
    // A birdsmouth cut of 3" with the automatic cut off sinks the roof.
    let cut = build(
        &mut sim,
        HeightSettings {
            auto_birdsmouth: false,
            birdsmouth_cut: -3.0,
            ..HeightSettings::default()
        },
    );
    assert!((top(&base) - top(&cut) - 3.0).abs() < 1e-6);
}

#[test]
fn the_height_group_is_kept_with_the_roof_and_a_build_is_one_undo_step() {
    let mut sim = mixed_pitch_house();
    let wanted = HeightSettings {
        same_height_eaves: true,
        allow_low_planes: false,
        framing: RoofFraming::Trusses,
        heel_height: 4.0,
        ..HeightSettings::default()
    };
    build(&mut sim, wanted.clone());
    let kept = roof_view::load(sim.app.cx.floor())
        .settings
        .expect("settings kept");
    assert_eq!(kept.heights, wanted);
    // Build Roof through the tool: one undo step takes the roof away.
    let mut fresh = mixed_pitch_house();
    fresh.tool(ToolId::RoofVariant(RoofMode::Build));
    fresh.click(240.0, 180.0);
    fresh.ok();
    assert_eq!(roof_view::load(fresh.app.cx.floor()).planes.len(), 4);
    fresh.undo();
    assert!(roof_view::load(fresh.app.cx.floor()).planes.is_empty());
}

fn top_of(r: &RoofPlaneRecord) -> f64 {
    r.polygon3d.iter().map(|v| v[1]).fold(f64::MIN, f64::max)
}

#[test]
fn locking_the_ridge_and_changing_the_pitch_keeps_the_ridge_where_it_was() {
    let mut sim = mixed_pitch_house();
    let planes = build(&mut sim, HeightSettings::default());
    let old = planes[0].clone();
    let thickness = roof_view::RoofStructure::default().thickness();
    let mut edited = old.clone();
    let h = old.plane_heights(thickness);
    let ridge = h.ridge_top();
    // The General panel with the Ridge Top radio set: 12:12 becomes 8:12.
    let steeper = h.with_pitch(old.pitch + 4.0, plan_roof::HeightLock::RidgeTop, true);
    edited.apply_heights(&steeper);
    let fl = sim.app.cx.floor;
    assert!(roof_view::apply_plane_edit(
        &mut sim.app.cx.project,
        fl,
        &edited
    ));
    let now = roof_view::load(sim.app.cx.floor())
        .planes
        .into_iter()
        .find(|r| r.id == old.id)
        .unwrap();
    assert_eq!(now.pitch, old.pitch + 4.0);
    assert!((now.plane_heights(thickness).ridge_top() - ridge).abs() < 1e-6);
    assert!((top_of(&now) - ridge).abs() < 1e-6, "ridge moved");
    // The eave dropped to make room for the steeper slope.
    assert!(eave_height(&now) < eave_height(&old) - 1.0);
    assert!(!now.auto, "an edited plane is kept");
}

#[test]
fn a_roof_directive_typed_over_several_walls_builds_a_gambrel_in_one_undo_step() {
    use crate::tools::ToolId as T;
    use plan_core::defaults::RoofWallKind;
    use crate::editor::selection::ObjectRef;
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    let long: Vec<_> = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .filter(|w| (w.start.y - w.end.y).abs() < 1e-6)
        .map(|w| w.id)
        .collect();
    let short: Vec<_> = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .filter(|w| (w.start.y - w.end.y).abs() >= 1e-6)
        .map(|w| w.id)
        .collect();
    assert_eq!((long.len(), short.len()), (2, 2));
    // Gable ends first, a one-wall edit each.
    for id in &short {
        sim.app.cx.floor_mut().wall_mut(*id).unwrap().roof.kind = RoofWallKind::FullGable;
    }
    // Shift-select the two long walls and fill in their Roof panel once.
    sim.tool(T::Select);
    for id in &long {
        sim.cx().selection.add(ObjectRef::Wall(*id));
    }
    sim.cx()
        .apply_edit_action(crate::editor::actions::EditActionKind::OpenObject);
    sim.app.process_requests();
    {
        let d = sim.app.spec.walls_dialog_mut().expect("multi-wall dialog");
        d.edit_field("roof", |w| {
            w.roof.kind = RoofWallKind::Hip;
            w.roof.pitch_in_12 = Some(18.0);
            w.roof.upper_pitch = Some((6.0, 150.0));
        });
    }
    sim.ok();
    for id in &long {
        let r = &sim.app.cx.floor().wall(*id).unwrap().roof;
        assert_eq!(r.pitch_in_12, Some(18.0));
        assert_eq!(r.upper_pitch, Some((6.0, 150.0)));
    }
    sim.undo();
    assert!(long.iter().all(|id| {
        sim.app
            .cx
            .floor()
            .wall(*id)
            .unwrap()
            .roof
            .upper_pitch
            .is_none()
    }));
    sim.redo();
    let fl = sim.app.cx.floor;
    let s = RoofSettings::from_defaults(&sim.app.cx.defaults);
    roof_view::rebuild(&mut sim.app.cx.project, fl, s, false).expect("roof built");
    let planes = roof_view::load(sim.app.cx.floor()).planes;
    let pitches: Vec<f64> = planes.iter().map(|p| p.pitch).collect();
    assert!(
        pitches.iter().filter(|p| **p == 18.0).count() >= 2,
        "{pitches:?}"
    );
    assert!(
        pitches.iter().filter(|p| **p == 6.0).count() >= 2,
        "{pitches:?}"
    );
}
