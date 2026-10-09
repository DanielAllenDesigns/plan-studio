//! The Options tab's Bay Roof (RF-29, DW-48): kind, pitch and overhang of the
//! roof over a bay, box or bow window.

use plan_3d::{build_scene, Material, Mesh, Scene};
use plan_core::openings::{BayRoof, BayRoofKind};
use plan_core::{Id, OpeningKind, OpeningStyle, Point, Project, WallKind};

fn unit(style: OpeningStyle, roof: BayRoof) -> (Scene, Id) {
    let mut p = Project::new("t");
    let wall = p.add_wall(
        0,
        Point::ZERO,
        Point::new(120.0, 0.0),
        4.5,
        100.0,
        WallKind::Interior,
    );
    let id = p
        .add_opening(0, wall, 60.0, OpeningKind::Window)
        .expect("opening");
    let o = p.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
        .unwrap();
    o.style = style;
    o.width = 72.0;
    o.extras.spec.bay_roof = roof;
    (build_scene(&p), id)
}

fn roof_of(scene: &Scene, id: Id) -> Vec<&Mesh> {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == Material::Roof)
        .collect()
}

/// Largest tilt of an upward face from level, rise per 12 of run.
fn steepest(ms: &[&Mesh]) -> f32 {
    ms.iter()
        .flat_map(|m| m.vertices.iter())
        .filter(|v| v.normal[1] > 0.5)
        .map(|v| (1.0 - v.normal[1] * v.normal[1]).max(0.0).sqrt() / v.normal[1] * 12.0)
        .fold(0.0, f32::max)
}

fn zmax(ms: &[&Mesh]) -> f32 {
    ms.iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[2]))
        .fold(f32::MIN, f32::max)
}

fn with(kind: BayRoofKind, pitch: f64, overhang: f64) -> BayRoof {
    BayRoof {
        kind,
        pitch,
        overhang,
    }
}

#[test]
fn the_default_is_what_the_style_always_had() {
    for style in [
        OpeningStyle::BayWindow,
        OpeningStyle::BowWindow,
        OpeningStyle::BoxWindow,
    ] {
        let (scene, id) = unit(style, BayRoof::default());
        let pitch = steepest(&roof_of(&scene, id));
        assert!((pitch - 6.0).abs() < 0.2, "{style:?} {pitch}");
    }
}

#[test]
fn none_builds_no_roof_and_flat_builds_a_level_slab() {
    for style in [OpeningStyle::BayWindow, OpeningStyle::BoxWindow] {
        let (scene, id) = unit(style, with(BayRoofKind::None, 6.0, 0.0));
        assert!(roof_of(&scene, id).is_empty(), "{style:?}");
        let (scene, id) = unit(style, with(BayRoofKind::Flat, 6.0, 0.0));
        let flat = roof_of(&scene, id);
        assert!(!flat.is_empty(), "{style:?}");
        assert!(steepest(&flat) < 0.05, "{style:?}");
    }
}

#[test]
fn pitch_and_overhang_change_the_roof() {
    let (scene, id) = unit(OpeningStyle::BayWindow, with(BayRoofKind::Hip, 12.0, 0.0));
    let steep = steepest(&roof_of(&scene, id));
    assert!((steep - 12.0).abs() < 0.3, "{steep}");
    let (scene, id) = unit(OpeningStyle::BayWindow, with(BayRoofKind::Hip, 6.0, 0.0));
    let plain = zmax(&roof_of(&scene, id));
    let (scene, id) = unit(OpeningStyle::BayWindow, with(BayRoofKind::Hip, 6.0, 6.0));
    let wide = zmax(&roof_of(&scene, id));
    assert!(wide > plain + 4.0, "{plain} -> {wide}");
}

#[test]
fn a_shed_roof_over_a_bay_slopes_from_the_wall_and_a_hip_over_a_box_hips() {
    let (scene, id) = unit(OpeningStyle::BayWindow, with(BayRoofKind::Shed, 6.0, 0.0));
    let shed = roof_of(&scene, id);
    assert!(!shed.is_empty());
    let leaning = shed
        .iter()
        .flat_map(|m| m.vertices.iter())
        .filter(|v| v.normal[1] > 0.8 && v.normal[1] < 0.99)
        .all(|v| v.normal[2] > 0.0);
    assert!(leaning);
    let (scene, id) = unit(OpeningStyle::BoxWindow, with(BayRoofKind::Hip, 6.0, 0.0));
    let hip = roof_of(&scene, id);
    assert!(!hip.is_empty());
    // A hip has faces leaning to the sides too.
    assert!(hip
        .iter()
        .flat_map(|m| m.vertices.iter())
        .any(|v| v.normal[0].abs() > 0.2 && v.normal[1] > 0.5));
}

#[test]
fn the_roof_options_survive_json() {
    let spec = plan_core::openings::OpeningSpec {
        bay_roof: with(BayRoofKind::Flat, 9.0, 3.0),
        ..Default::default()
    };
    let back: plan_core::openings::OpeningSpec =
        serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
    assert_eq!(back.bay_roof, spec.bay_roof);
    // An old plan has no record: the default.
    let old: plan_core::openings::OpeningSpec = serde_json::from_str("{}").unwrap();
    assert_eq!(old.bay_roof, BayRoof::default());
}
