//! Scenario 20: roofs in the 3D scene. Gable walls that reach the ridge, hips
//! that clip taller walls, attic walls above a lower roof, soffit and fascia,
//! the baseline at the plate, the eave cut of Roof Defaults and Roof Cuts Wall
//! at Bottom (RF-13..RF-16, RF-20, RF-28, RF-31).

use super::{draw_shell, Sim};
use crate::editor::roof_view::{self, RoofSettings};
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;
use plan_3d::{Material, Mesh, Scene};
use plan_core::defaults::{EaveCut, RoofDetailDefaults, RoofWallKind};
use plan_core::geometry::Point;
use plan_core::{Floor, Id, Project, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn scene_of(p: &Project) -> Scene {
    build_view_scene(p, &ViewScope::default())
}

fn build_roof(sim: &mut Sim) {
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
}

fn of(scene: &Scene, id: Id) -> Vec<&Mesh> {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .collect()
}

fn max_y(ms: &[&Mesh]) -> f32 {
    ms.iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .fold(f32::MIN, f32::max)
}

fn min_y(ms: &[&Mesh]) -> f32 {
    ms.iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .fold(f32::MAX, f32::min)
}

fn tris(scene: &Scene, material: Material) -> usize {
    scene
        .meshes
        .iter()
        .filter(|m| m.material == material)
        .map(Mesh::triangle_count)
        .sum()
}

fn plane_ids(sim: &Sim) -> Vec<Id> {
    roof_view::load(sim.app.cx.floor())
        .planes
        .iter()
        .map(|p| p.id)
        .collect()
}

/// Trim triangles tagged with a roof plane (fascia, soffit, rake boards, caps).
fn roof_trim(sim: &Sim) -> usize {
    let ids = plane_ids(sim);
    scene_of(&sim.app.cx.project)
        .meshes
        .iter()
        .filter(|m| m.material == Material::Trim && m.object_id.is_some_and(|i| ids.contains(&i)))
        .map(Mesh::triangle_count)
        .sum()
}

fn walls_at(sim: &Sim, x: f64, y: f64) -> Id {
    sim.app
        .cx
        .floor()
        .walls
        .iter()
        .min_by(|a, b| {
            plan_core::geometry::dist_to_segment(Point::new(x, y), a.start, a.end).total_cmp(
                &plan_core::geometry::dist_to_segment(Point::new(x, y), b.start, b.end),
            )
        })
        .unwrap()
        .id
}

/// Gable/Roof Line on the two short walls, the way the tool is used.
fn gable_both_ends(sim: &mut Sim) {
    sim.tool(ToolId::RoofVariant(RoofMode::GableLine));
    sim.click(W, 180.0);
    sim.click(0.0, 180.0);
    sim.tool(ToolId::Select);
}

#[test]
fn gable_end_walls_rise_to_the_ridge_and_eave_walls_stop_at_the_plate() {
    let mut sim = house();
    build_roof(&mut sim);
    let east = walls_at(&sim, W, 180.0);
    let south = walls_at(&sim, 240.0, 0.0);
    let hip = scene_of(&sim.app.cx.project);
    let plate = f32::from(1u8) * sim.app.cx.floor().walls[0].height as f32;
    let east_hip = max_y(&of(&hip, east));
    assert!(
        east_hip <= plate + 1.0,
        "a hip roof keeps the wall at the plate"
    );

    gable_both_ends(&mut sim);
    assert_eq!(roof_view::load(sim.app.cx.floor()).planes.len(), 2);
    assert_eq!(
        sim.app.cx.floor().wall(east).unwrap().roof.kind,
        RoofWallKind::FullGable
    );
    let gable = scene_of(&sim.app.cx.project);
    let ridge = tris_top(&gable, &plane_ids(&sim));
    let east_top = max_y(&of(&gable, east));
    assert!(
        east_top > plate + 50.0,
        "the gable wall rises {east_top} above the plate {plate}"
    );
    assert!(
        east_top <= ridge + 0.5,
        "never through the roof: {east_top} vs {ridge}"
    );
    // The eave wall still stops at the plate.
    assert!(max_y(&of(&gable, south)) <= plate + 1.0);
    // Undo steps back to the hip: the wall drops to the plate again.
    assert_eq!(sim.undo().as_deref(), Some("Gable/Roof Line"));
    assert_eq!(sim.undo().as_deref(), Some("Gable/Roof Line"));
    let back = scene_of(&sim.app.cx.project);
    assert!(max_y(&of(&back, east)) <= plate + 1.0);
}

fn tris_top(scene: &Scene, ids: &[Id]) -> f32 {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id.is_some_and(|i| ids.contains(&i)) && m.material == Material::Roof)
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .fold(f32::MIN, f32::max)
}

#[test]
fn a_hip_roof_set_below_the_wall_tops_clips_every_wall_to_the_slope() {
    let mut sim = house();
    // Raise the walls to 150" after setting up a roof 36" under them.
    let mut settings = RoofSettings::from_defaults(&sim.app.cx.defaults);
    settings.raise_off_plate = -36.0;
    settings.detail.baseline_at_plate = false;
    let fl = sim.app.cx.floor;
    roof_view::rebuild(&mut sim.app.cx.project, fl, settings, false).expect("roof built");
    assert_eq!(roof_view::load(sim.app.cx.floor()).planes.len(), 4);
    let ids: Vec<Id> = sim.wall_ids();
    let clipped = scene_of(&sim.app.cx.project);
    // The same plan with no roof: flat tops at the plate.
    let mut bare = sim.app.cx.project.clone();
    bare.floors[0].roofs.clear();
    let flat = scene_of(&bare);
    for id in ids {
        let (c, f) = (max_y(&of(&clipped, id)), max_y(&of(&flat, id)));
        assert!(c < f - 10.0, "wall {id}: clipped top {c} vs flat {f}");
    }
}

fn lean_to() -> Project {
    // A low shed roof on floor 0 and a taller wall on floor 1 beside it.
    let mut sim = Sim::new();
    let p = &mut sim.app.cx.project;
    p.floors.push(Floor::new("Second", 120.0));
    let sq = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 240.0),
        Point::new(0.0, 240.0),
    ];
    let mut e = [plan_roof::EdgeRoof {
        pitch_in_12: 2.0,
        kind: plan_roof::EdgeKind::Shed,
        overhang: 19.0,
    }; 4];
    e[0].kind = plan_roof::EdgeKind::Hip;
    let roof = plan_roof::build_roof(&sq, &e, 60.0);
    p.floors[0].roofs = roof
        .planes
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            serde_json::json!({
                "kind": "plane", "id": 9000 + i as u64,
                "polygon3d": pl.polygon3d, "pitch": pl.pitch_in_12,
                "baseline": [pl.baseline.0, pl.baseline.1],
                "overhang": 16.0, "ridge_caps": false,
            })
        })
        .collect();
    p.add_wall(
        1,
        Point::new(-100.0, 240.0),
        Point::new(340.0, 240.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    // The roof settings record carries the Roof Defaults the scene reads.
    let mut set = roof_view::load(&p.floors[0]);
    set.settings = Some(RoofSettings::fallback());
    roof_view::store(p, 0, &mut set);
    sim.app.cx.project.clone()
}

#[test]
fn an_attic_wall_fills_the_gap_above_a_lower_roof_and_roof_defaults_switch_it() {
    let mut p = lean_to();
    let attic = |p: &Project| -> usize {
        scene_of(p)
            .meshes
            .iter()
            .filter(|m| m.object_id.is_none() && m.material == Material::WallExterior)
            .count()
    };
    assert!(attic(&p) > 0, "no attic wall above the lean-to");
    let mut d = RoofDetailDefaults {
        auto_attic_walls: false,
        ..RoofDetailDefaults::default()
    };
    roof_view::apply_detail(&mut p, &d);
    assert_eq!(attic(&p), 0, "Auto Attic Walls off");
    d.auto_attic_walls = true;
    roof_view::apply_detail(&mut p, &d);
    assert!(attic(&p) > 0);
}

#[test]
fn soffit_and_fascia_follow_the_roof_defaults_and_count_per_eave() {
    let mut sim = house();
    build_roof(&mut sim);
    gable_both_ends(&mut sim);
    let base = roof_trim(&sim);
    assert!(base > 0, "a gable roof has fascia and soffit");
    let apply = |sim: &mut Sim, f: &dyn Fn(&mut RoofDetailDefaults)| {
        let mut d = roof_view::load(sim.app.cx.floor())
            .settings
            .expect("roof settings")
            .detail;
        f(&mut d);
        let fl = sim.app.cx.floor;
        roof_view::apply_detail(&mut sim.app.cx.project, &d);
        let _ = fl;
    };
    apply(&mut sim, &|d| d.soffit = false);
    let no_soffit = roof_trim(&sim);
    assert!(no_soffit < base, "{base} -> {no_soffit}");
    apply(&mut sim, &|d| {
        d.soffit = false;
        d.fascia = false;
    });
    let none = roof_trim(&sim);
    assert!(none < no_soffit);
    apply(&mut sim, &|d| {
        d.soffit = true;
        d.fascia = true;
        d.frieze = true;
    });
    assert!(
        roof_trim(&sim) > base,
        "a frieze board under each eave adds trim"
    );
    apply(&mut sim, &|d| {
        d.soffit = true;
        d.fascia = true;
        d.frieze = false;
    });
    assert_eq!(roof_trim(&sim), base);
}

/// Underside of the south roof plane where it meets the south wall's
/// centerline, and the wall top there.
fn gap_at_south_wall(sim: &Sim) -> f64 {
    let set = roof_view::load(sim.app.cx.floor());
    let detail = set.settings.as_ref().unwrap().detail.clone();
    let south = set
        .planes
        .iter()
        .filter(|r| (r.baseline.0.y - r.baseline.1.y).abs() < 1e-6)
        .min_by(|a, b| a.baseline.0.y.total_cmp(&b.baseline.0.y))
        .unwrap();
    let wall = sim.app.cx.floor().walls[0].clone();
    let under = south
        .to_roof_plane(0)
        .underside_at(Point::new(240.0, wall.start.y), detail.thickness)
        .expect("the plane covers the wall");
    under - wall.height
}

#[test]
fn the_baseline_at_the_plate_closes_the_gap_between_wall_and_roof() {
    let mut on = house();
    on.app.cx.defaults.roof_detail.baseline_at_plate = true;
    build_roof(&mut on);
    let seated = gap_at_south_wall(&on);
    assert!(seated.abs() < 0.5, "seated roof: gap {seated}");

    let mut off = house();
    off.app.cx.defaults.roof_detail.baseline_at_plate = false;
    build_roof(&mut off);
    let tip = gap_at_south_wall(&off);
    assert!(
        tip > seated + 5.0,
        "the eave tip at the plate leaves a gap of {tip}"
    );
    // In 3D the roof slab over the wall is lower with the plate rule.
    let low = |sim: &Sim| {
        let ids = plane_ids(sim);
        let s = scene_of(&sim.app.cx.project);
        let roof: Vec<&Mesh> = s
            .meshes
            .iter()
            .filter(|m| {
                m.material == Material::Roof && m.object_id.is_some_and(|i| ids.contains(&i))
            })
            .collect();
        min_y(&roof)
    };
    assert!(low(&on) < low(&off), "{} vs {}", low(&on), low(&off));
}

#[test]
fn the_eave_cut_in_roof_defaults_changes_the_fascia_geometry() {
    let mut boxes = Vec::new();
    for cut in [EaveCut::Plumb, EaveCut::Square, EaveCut::Level] {
        let mut sim = house();
        sim.app.cx.defaults.roof_detail.eave_cut = cut;
        build_roof(&mut sim);
        let stored = roof_view::load(sim.app.cx.floor())
            .settings
            .unwrap()
            .detail
            .eave_cut;
        assert_eq!(stored, cut, "the cut is kept with the roof");
        let ids = plane_ids(&sim);
        let s = scene_of(&sim.app.cx.project);
        let trim: Vec<&Mesh> = s
            .meshes
            .iter()
            .filter(|m| {
                m.material == Material::Trim && m.object_id.is_some_and(|i| ids.contains(&i))
            })
            .collect();
        let mut verts: Vec<[i32; 3]> = trim
            .iter()
            .flat_map(|m| m.vertices.iter())
            .map(|v| v.position.map(|c| (c * 100.0).round() as i32))
            .collect();
        verts.sort();
        boxes.push((
            cut,
            trim.iter().map(|m| m.triangle_count()).sum::<usize>(),
            verts,
        ));
    }
    // The same boards, cut three different ways.
    assert_eq!(boxes[0].1, boxes[1].1);
    assert_eq!(boxes[1].1, boxes[2].1);
    assert_ne!(boxes[0].2, boxes[1].2, "plumb vs square cut");
    assert_ne!(boxes[1].2, boxes[2].2, "square vs level cut");
}

/// Floor 0 holds a low 6:12 shed roof rising to the north; a floor-1 wall
/// runs north-south over it, starting where the roof is below the wall's
/// bottom and ending where it is above.
fn wall_over_a_rising_roof() -> (Project, Id) {
    let mut sim = Sim::new();
    let p = &mut sim.app.cx.project;
    p.floors.push(Floor::new("Second", 120.0));
    let sq = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 240.0),
        Point::new(0.0, 240.0),
    ];
    let mut e = [plan_roof::EdgeRoof {
        pitch_in_12: 6.0,
        kind: plan_roof::EdgeKind::Shed,
        overhang: 19.0,
    }; 4];
    e[0].kind = plan_roof::EdgeKind::Hip;
    let roof = plan_roof::build_roof(&sq, &e, 60.0);
    assert_eq!(roof.planes.len(), 1);
    p.floors[0].roofs = roof
        .planes
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            serde_json::json!({
                "kind": "plane", "id": 9000 + i as u64,
                "polygon3d": pl.polygon3d, "pitch": pl.pitch_in_12,
                "baseline": [pl.baseline.0, pl.baseline.1],
                "overhang": 16.0, "ridge_caps": false,
            })
        })
        .collect();
    let wall = p.add_wall(
        1,
        Point::new(100.0, 40.0),
        Point::new(100.0, 200.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    let mut set = roof_view::load(&p.floors[0]);
    set.settings = Some(RoofSettings::fallback());
    roof_view::store(p, 0, &mut set);
    (sim.app.cx.project.clone(), wall)
}

/// Lowest scene height of the wall's vertices lying within 1" of plan `y`.
fn lowest_at(scene: &Scene, id: Id, y: f64) -> f64 {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .flat_map(|m| m.vertices.iter())
        .filter(|v| (f64::from(-v.position[2]) - y).abs() < 1.0)
        .map(|v| f64::from(v.position[1]))
        .fold(f64::MAX, f64::min)
}

#[test]
fn roof_cuts_wall_at_bottom_trims_a_wall_standing_on_a_roof_and_the_default_switches_it() {
    let (mut p, wall) = wall_over_a_rising_roof();
    // Roof surface at plan y: 60 + (y + 22) * 6/12; it passes the wall's
    // 120" bottom at y = 98.
    let surface = |y: f64| 60.0 + (y + 22.0) * 0.5;
    let on = scene_of(&p);
    assert!(
        (lowest_at(&on, wall, 40.0) - 120.0).abs() < 0.01,
        "the low end keeps its bottom"
    );
    let at_end = lowest_at(&on, wall, 200.0);
    assert!(
        (at_end - surface(200.0)).abs() < 3.0,
        "stands on the roof: {at_end}"
    );
    assert!(at_end > 120.0 + 40.0);
    // The top of the wall is the same either way.
    assert!((max_y(&of(&on, wall)) - 216.0).abs() < 0.01);

    let d = RoofDetailDefaults {
        roof_cuts_wall_at_bottom: false,
        ..RoofDetailDefaults::default()
    };
    assert_eq!(roof_view::apply_detail(&mut p, &d), 1);
    let off = scene_of(&p);
    assert!(
        (lowest_at(&off, wall, 200.0) - 120.0).abs() < 0.01,
        "Roof Cuts Wall at Bottom off"
    );
    assert!((min_y(&of(&off, wall)) - 120.0).abs() < 0.01);
}
