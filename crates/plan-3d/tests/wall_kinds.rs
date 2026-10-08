//! The flyout wall classes in 3D: pony, half, glass, foundation, room
//! divider, deck edge, fencing and curved walls.

use plan_3d::{build_scene_with_types, Material, Mesh, Scene, SceneOptions};
use plan_core::{
    FenceStyle, Id, OpeningKind, PlanDefaults, Point, Project, WallClass, WallCurve, WallKind,
};

fn project(class: WallClass) -> (Project, Id) {
    let mut p = Project::new("t");
    let id = p.add_wall(
        0,
        Point::ZERO,
        Point::new(120.0, 0.0),
        7.625,
        109.125,
        WallKind::Exterior,
    );
    p.floors[0].wall_mut(id).unwrap().set_class(class);
    (p, id)
}

fn scene(p: &Project) -> Scene {
    let d = PlanDefaults::chief_x18_daniel();
    build_scene_with_types(p, &SceneOptions::default(), &d.wall_types)
}

fn of(scene: &Scene, id: Id) -> Vec<&Mesh> {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .collect()
}

/// Y of every vertex of the wall's meshes, and the top/bottom caps (normal +-Y).
fn cap_heights(scene: &Scene, id: Id, up: bool) -> Vec<f32> {
    let mut ys = Vec::new();
    for m in of(scene, id) {
        for v in &m.vertices {
            if (v.normal[1] > 0.99 && up) || (v.normal[1] < -0.99 && !up) {
                ys.push(v.position[1]);
            }
        }
    }
    ys.sort_by(f32::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    ys
}

fn pony() -> WallClass {
    WallClass::Pony {
        upper_type: "Stucco-6".into(),
        lower_type: "Foundation-8".into(),
        split_height: 36.0,
        upper_sets_plan_display: false,
    }
}

#[test]
fn pony_wall_is_two_boxes_split_at_36_inches() {
    let (p, id) = project(pony());
    let s = scene(&p);
    let tops = cap_heights(&s, id, true);
    let bottoms = cap_heights(&s, id, false);
    // Lower box 0..36, upper box 36..109.125.
    assert_eq!(bottoms.len(), 2, "{bottoms:?}");
    assert!(bottoms[0].abs() < 1e-3 && (bottoms[1] - 36.0).abs() < 1e-3);
    assert_eq!(tops.len(), 2, "{tops:?}");
    assert!((tops[0] - 36.0).abs() < 1e-3 && (tops[1] - 109.125).abs() < 1e-3);
    // The lower box is concrete (lower type), the upper stucco (upper type).
    let mats: Vec<Material> = of(&s, id).iter().map(|m| m.material).collect();
    assert!(mats.contains(&Material::Concrete));
    assert!(mats.contains(&Material::Stucco));
    let (lo, hi) = s.bounds().unwrap();
    assert!(lo[1].abs() < 1e-3 && (hi[1] - 109.125).abs() < 1e-3);
}

#[test]
fn half_wall_is_capped_at_its_height() {
    let (p, id) = project(WallClass::HalfWall { height: 36.0 });
    let s = scene(&p);
    assert!(!of(&s, id).is_empty());
    assert!((s.bounds().unwrap().1[1] - 36.0).abs() < 1e-3);
}

#[test]
fn room_divider_has_no_mesh() {
    let (p, id) = project(WallClass::RoomDivider);
    assert!(of(&scene(&p), id).is_empty());
    // Even without the invisible flag.
    let (mut p, id) = project(WallClass::RoomDivider);
    p.floors[0].wall_mut(id).unwrap().flags.invisible = false;
    assert!(of(&scene(&p), id).is_empty());
}

#[test]
fn glass_wall_is_glass_in_a_trim_frame() {
    let (p, id) = project(WallClass::Glass);
    let s = scene(&p);
    let mats: Vec<Material> = of(&s, id).iter().map(|m| m.material).collect();
    assert!(mats.contains(&Material::Glass) && mats.contains(&Material::Trim));
    assert!(!mats.contains(&Material::WallInterior));
    assert!((s.bounds().unwrap().1[1] - 109.125).abs() < 1e-3);
}

#[test]
fn glass_pony_wall_is_solid_below_and_glass_above() {
    let (p, id) = project(WallClass::GlassPony {
        lower_type: "Brick-6".into(),
        split_height: 36.0,
    });
    let s = scene(&p);
    let mats: Vec<Material> = of(&s, id).iter().map(|m| m.material).collect();
    assert!(mats.contains(&Material::Brick) && mats.contains(&Material::Glass));
    assert!(cap_heights(&s, id, true)
        .iter()
        .any(|y| (y - 36.0).abs() < 1e-3));
}

#[test]
fn foundation_wall_extends_below_the_floor() {
    let (mut p, id) = project(WallClass::Foundation);
    p.floors[0].wall_mut(id).unwrap().foundation_height = 40.0;
    let s = scene(&p);
    let (lo, hi) = s.bounds().unwrap();
    assert!((lo[1] + 40.0).abs() < 1e-3, "{lo:?}");
    assert!(hi[1].abs() < 1e-3, "{hi:?}");
    assert!(of(&s, id).iter().all(|m| m.material == Material::Concrete));
}

#[test]
fn deck_edge_is_a_rim_board_without_railing() {
    let (p, id) = project(WallClass::DeckEdge);
    let s = scene(&p);
    let mats: Vec<Material> = of(&s, id).iter().map(|m| m.material).collect();
    assert_eq!(mats, vec![Material::Trim]);
    assert!(s.bounds().unwrap().1[1].abs() < 1e-3);
}

#[test]
fn deck_railing_and_railing_build_posts_and_rails() {
    for class in [WallClass::DeckRailing, WallClass::Railing] {
        let (p, id) = project(class);
        let s = scene(&p);
        let mats: Vec<Material> = of(&s, id).iter().map(|m| m.material).collect();
        assert!(mats.contains(&Material::Trim) && mats.contains(&Material::Metal));
        assert!((s.bounds().unwrap().1[1] - 36.0).abs() < 1e-3);
    }
}

#[test]
fn fencing_has_posts_and_pickets() {
    for style in [FenceStyle::Picket, FenceStyle::Privacy, FenceStyle::Rail] {
        let (mut p, id) = project(WallClass::Fencing { style });
        p.floors[0].wall_mut(id).unwrap().height = 72.0;
        let s = scene(&p);
        assert!(!of(&s, id).is_empty());
        assert!((s.bounds().unwrap().1[1] - 72.0).abs() < 1e-3);
    }
    let (mut p, id) = project(WallClass::Fencing {
        style: FenceStyle::Picket,
    });
    p.floors[0].wall_mut(id).unwrap().height = 72.0;
    let picket = scene(&p).triangle_count();
    p.floors[0].wall_mut(id).unwrap().class = WallClass::Fencing {
        style: FenceStyle::Rail,
    };
    assert!(picket > scene(&p).triangle_count());
}

#[test]
fn doors_still_cut_pony_and_half_walls() {
    for class in [
        pony(),
        WallClass::HalfWall { height: 36.0 },
        WallClass::Glass,
    ] {
        let (mut p, id) = project(class.clone());
        let plain = scene(&p).triangle_count();
        p.add_opening(0, id, 60.0, OpeningKind::Window).unwrap();
        assert!(scene(&p).triangle_count() > plain, "{class:?}");
    }
}

#[test]
fn curved_walls_are_faceted() {
    let (mut p, id) = project(WallClass::Standard);
    let straight = scene(&p).triangle_count();
    p.floors[0].wall_mut(id).unwrap().curve = Some(WallCurve { bulge: 30.0 });
    let s = scene(&p);
    assert!(s.triangle_count() > straight * 2);
    // The arc bulges to the left (+y plan = -z scene).
    assert!(s.bounds().unwrap().0[2] < -25.0);
}
