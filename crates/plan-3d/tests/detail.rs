//! Chief-level detail: door/window styles, casing, wall types, railings.

use plan_3d::{
    build_scene, build_scene_with, build_scene_with_types, railing_post_count, Material, Mesh,
    Scene, SceneOptions,
};
use plan_core::{
    Casing, Id, OpeningKind, OpeningStyle, PlanDefaults, Point, PonyWall, Project, WallKind,
};

fn ft(f: f64) -> f64 {
    f * 12.0
}

/// One 10' wall along +x (interior side on the left, so units project to +z).
fn wall_project(kind: WallKind) -> (Project, Id) {
    let mut p = Project::new("t");
    let id = p.add_wall(0, Point::ZERO, Point::new(ft(10.0), 0.0), 4.5, 100.0, kind);
    (p, id)
}

fn with_opening(kind: OpeningKind, style: OpeningStyle, width: f64) -> (Project, Id, Id) {
    let (mut p, wall) = wall_project(WallKind::Interior);
    let id = p.add_opening(0, wall, ft(5.0), kind).unwrap();
    let o = p.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
        .unwrap();
    o.style = style;
    o.width = width;
    (p, wall, id)
}

fn materials_of(scene: &Scene, id: Id) -> Vec<Material> {
    let mut m: Vec<Material> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .map(|m| m.material)
        .collect();
    m.sort_by_key(Material::index);
    m.dedup();
    m
}

fn sorted(mut m: Vec<Material>) -> Vec<Material> {
    m.sort_by_key(Material::index);
    m
}

fn tris(scene: &Scene, id: Id, material: Material) -> usize {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == material)
        .map(Mesh::triangle_count)
        .sum()
}

fn casing_on() -> SceneOptions {
    SceneOptions {
        show_casing: true,
        ..Default::default()
    }
}

#[test]
fn every_door_style_builds_its_expected_materials() {
    use Material::*;
    let cases = [
        (OpeningStyle::Hinged, vec![DoorPanel]),
        (OpeningStyle::Sliding, vec![DoorPanel, Glass]),
        (OpeningStyle::Pocket, vec![DoorPanel]),
        (OpeningStyle::Bifold, vec![DoorPanel]),
        (OpeningStyle::Garage, vec![DoorPanel, Metal]),
        (OpeningStyle::Doorway, vec![]),
        (OpeningStyle::Barn, vec![DoorPanel, Metal]),
        (OpeningStyle::Shower, vec![Glass, Metal]),
        (OpeningStyle::Fixed, vec![DoorPanel]),
    ];
    for (style, base) in cases {
        let (p, _, id) = with_opening(OpeningKind::Door, style, 36.0);
        let scene = build_scene(&p);
        assert_eq!(materials_of(&scene, id), sorted(base.clone()), "{style:?}");

        let cased = build_scene_with(&p, &casing_on());
        let mut with_trim = base;
        with_trim.push(Trim);
        assert_eq!(
            materials_of(&cased, id),
            sorted(with_trim),
            "{style:?} + casing"
        );
        assert!(!materials_of(&cased, id).is_empty());
        for m in &cased.meshes {
            assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
        }
    }
}

#[test]
fn window_and_niche_styles_build() {
    use Material::*;
    let (p, _, id) = with_opening(OpeningKind::Window, OpeningStyle::Window, 36.0);
    assert_eq!(
        materials_of(&build_scene(&p), id),
        sorted(vec![WindowFrame, WindowGlass])
    );
    let (p, _, id) = with_opening(OpeningKind::Window, OpeningStyle::Fixed, 36.0);
    assert_eq!(
        materials_of(&build_scene(&p), id),
        sorted(vec![WindowFrame, WindowGlass])
    );
    let (p, _, id) = with_opening(OpeningKind::Window, OpeningStyle::PassThrough, 36.0);
    assert!(materials_of(&build_scene(&p), id).is_empty());
    assert_eq!(
        materials_of(&build_scene_with(&p, &casing_on()), id),
        vec![Trim]
    );

    // A niche is a recess, not a hole: left face has 4 strips, right 1.
    let (p, wall2, id) = with_opening(OpeningKind::Window, OpeningStyle::WallNiche, 36.0);
    let scene = build_scene(&p);
    assert!(materials_of(&scene, id).is_empty());
    let wall_tris: usize = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(wall2))
        .map(Mesh::triangle_count)
        .sum();
    assert_eq!(wall_tris, (4 + 1 + 4 + 5) * 2);
}

#[test]
fn three_by_two_lites_make_six_glass_panes() {
    let (mut p, _, id) = with_opening(OpeningKind::Window, OpeningStyle::Window, 48.0);
    p.floors[0].openings[0].lites = (3, 2);
    let scene = build_scene(&p);
    assert_eq!(tris(&scene, id, Material::WindowGlass), 6 * 12);
    // show_lites off collapses to one pane.
    let off = build_scene_with(
        &p,
        &SceneOptions {
            show_lites: false,
            ..Default::default()
        },
    );
    assert_eq!(tris(&off, id, Material::WindowGlass), 12);
}

#[test]
fn door_lites_cut_glass_only_when_asked() {
    let (mut p, _, id) = with_opening(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
    assert_eq!(tris(&build_scene(&p), id, Material::WindowGlass), 0);
    p.floors[0].openings[0].lites = (2, 3);
    let scene = build_scene(&p);
    assert_eq!(tris(&scene, id, Material::WindowGlass), 6 * 12);
    assert!(tris(&scene, id, Material::DoorPanel) > 12 * 4);
}

#[test]
fn casing_adds_trim_on_both_faces() {
    let (p, _, id) = with_opening(OpeningKind::Window, OpeningStyle::Window, 36.0);
    assert_eq!(tris(&build_scene(&p), id, Material::Trim), 0);
    let scene = build_scene_with(&p, &casing_on());
    let trim: Vec<&Mesh> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == Material::Trim)
        .collect();
    assert!(!trim.is_empty());
    // Wall is 4.5" thick on z = +/-2.25; default casing depth is 3/4".
    let zs: Vec<f32> = trim
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[2]))
        .collect();
    let (zmin, zmax) = (
        zs.iter().copied().fold(f32::MAX, f32::min),
        zs.iter().copied().fold(f32::MIN, f32::max),
    );
    assert!(zmin < -2.25 - 0.7, "{zmin}");
    assert!(zmax > 2.25 + 0.7, "{zmax}");
    // The room side (left, scene -z) also carries the sill projection.
    assert!(zmin < -3.4, "{zmin}");
    // Custom casing depth is honoured.
    let mut p = p;
    p.floors[0].openings[0].casing = Some(Casing {
        width: 5.5,
        depth: 1.0,
        reveal: 0.25,
    });
    let scene = build_scene_with(&p, &casing_on());
    let zmax = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == Material::Trim)
        .flat_map(|m| m.vertices.iter().map(|v| v.position[2]))
        .fold(f32::MIN, f32::max);
    assert!((zmax - 3.25).abs() < 1e-4, "{zmax}");
}

#[test]
fn exterior_doors_get_a_threshold_only_with_casing() {
    let (mut p, wall) = wall_project(WallKind::Exterior);
    let id = p.add_opening(0, wall, ft(5.0), OpeningKind::Door).unwrap();
    let without = tris(&build_scene(&p), id, Material::Trim);
    let with = tris(&build_scene_with(&p, &casing_on()), id, Material::Trim);
    assert_eq!(without, 0);
    // Two legs + head on two faces (6 boxes), the jamb boards beside the
    // leaf (legs and head, on two sides: 6 more) and the threshold.
    assert_eq!(with, 13 * 12);
}

#[test]
fn bay_box_and_bow_project_eighteen_inches() {
    for style in [
        OpeningStyle::BayWindow,
        OpeningStyle::BoxWindow,
        OpeningStyle::BowWindow,
    ] {
        let (p, _, id) = with_opening(OpeningKind::Window, style, 72.0);
        let scene = build_scene(&p);
        let zmax = scene
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(id))
            .flat_map(|m| m.vertices.iter().map(|v| v.position[2]))
            .fold(f32::MIN, f32::max);
        assert!(
            (zmax - (2.25 + 18.0)).abs() < 1e-3,
            "{style:?} projects {}",
            zmax - 2.25
        );
        let mats = materials_of(&scene, id);
        for m in [
            Material::WindowFrame,
            Material::WindowGlass,
            Material::Trim,
            Material::Roof,
        ] {
            assert!(mats.contains(&m), "{style:?} missing {m:?}");
        }
    }
    // Panes: bay and box have 3, bow has 5.
    let panes = |style| {
        let (p, _, id) = with_opening(OpeningKind::Window, style, 72.0);
        tris(&build_scene(&p), id, Material::WindowGlass) / 12
    };
    assert_eq!(panes(OpeningStyle::BayWindow), 3);
    assert_eq!(panes(OpeningStyle::BoxWindow), 3);
    assert_eq!(panes(OpeningStyle::BowWindow), 5);
}

#[test]
fn open_hinged_door_swings_out_of_the_wall_plane() {
    let (p, _, id) = with_opening(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
    let extent = |scene: &Scene| {
        scene
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(id))
            .flat_map(|m| m.vertices.iter().map(|v| v.position[2].abs()))
            .fold(0.0f32, f32::max)
    };
    assert!(extent(&build_scene(&p)) < 1.0);
    let open = build_scene_with(
        &p,
        &SceneOptions {
            doors_open: true,
            ..Default::default()
        },
    );
    assert!(extent(&open) > 35.0, "{}", extent(&open));
}

#[test]
fn railing_wall_has_ceil_length_over_96_plus_one_posts() {
    for inches in [120.0, 96.0, 200.0, 50.0] {
        let mut p = Project::new("r");
        let id = p.add_wall(
            0,
            Point::ZERO,
            Point::new(inches, 0.0),
            4.5,
            100.0,
            WallKind::Interior,
        );
        p.floors[0].walls[0].flags.railing = true;
        let scene = build_scene(&p);
        let posts = tris(&scene, id, Material::Trim) / 12;
        let expected = (inches / 96.0).ceil() as usize + 1;
        assert_eq!(posts, expected, "{inches}");
        assert_eq!(railing_post_count(inches), expected);
        assert!(tris(&scene, id, Material::Metal) > 12);
        // Railing is 36" tall, not a solid wall.
        let (_, hi) = scene.bounds().unwrap();
        assert!((hi[1] - 36.0).abs() < 1e-4);
        assert!(scene
            .meshes
            .iter()
            .all(|m| m.material != Material::WallInterior));
    }
}

#[test]
fn a_railing_wall_stands_on_its_bottom_offset() {
    let mut p = Project::new("r");
    p.add_wall(
        0,
        Point::ZERO,
        Point::new(120.0, 0.0),
        4.5,
        100.0,
        WallKind::Interior,
    );
    p.floors[0].walls[0].flags.railing = true;
    p.floors[0].walls[0].bottom_offset = 12.0;
    let (lo, hi) = build_scene(&p).bounds().unwrap();
    assert!((lo[1] - 12.0).abs() < 1e-4, "{lo:?}");
    assert!((hi[1] - 48.0).abs() < 1e-4, "{hi:?}");
}

#[test]
fn invisible_walls_produce_no_mesh() {
    let (mut p, wall) = wall_project(WallKind::Interior);
    p.add_opening(0, wall, ft(5.0), OpeningKind::Door).unwrap();
    p.floors[0].walls[0].flags.invisible = true;
    assert!(build_scene(&p).meshes.is_empty());
}

#[test]
fn half_and_pony_walls_are_cut_down() {
    let (mut p, _) = wall_project(WallKind::Interior);
    p.floors[0].walls[0].flags.half_wall = true;
    assert!((build_scene(&p).bounds().unwrap().1[1] - 36.0).abs() < 1e-4);
    p.floors[0].walls[0].flags.half_wall = false;
    p.floors[0].walls[0].flags.pony = Some(PonyWall {
        lower_type: "Brick-6".into(),
        lower_height: 42.0,
    });
    assert!((build_scene(&p).bounds().unwrap().1[1] - 42.0).abs() < 1e-4);
}

fn room() -> Project {
    let mut p = Project::new("room");
    let c = [
        Point::new(0.0, 0.0),
        Point::new(ft(20.0), 0.0),
        Point::new(ft(20.0), ft(10.0)),
        Point::new(0.0, ft(10.0)),
    ];
    for i in 0..4 {
        p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior);
    }
    p
}

#[test]
fn wall_types_color_the_exterior_face() {
    let defaults = PlanDefaults::default();
    for (ty, expected) in [
        ("Stucco-6", Material::Stucco),
        ("Siding-6", Material::Siding),
        ("Brick-6", Material::Brick),
        ("stone-6", Material::Stone),
        ("Foundation-8", Material::Concrete),
    ] {
        let mut p = room();
        for w in &mut p.floors[0].walls {
            w.wall_type = Some(ty.into());
        }
        let scene = build_scene_with_types(&p, &SceneOptions::default(), &defaults.wall_types);
        assert!(scene.meshes.iter().any(|m| m.material == expected), "{ty}");
        assert!(scene
            .meshes
            .iter()
            .all(|m| m.material != Material::WallExterior));
        assert!(scene
            .meshes
            .iter()
            .any(|m| m.material == Material::WallInterior));
        // Without the defaults the type is unknown and the old look remains.
        let plain = build_scene(&p);
        assert!(plain
            .meshes
            .iter()
            .any(|m| m.material == Material::WallExterior));
    }
    // A project-registered type wins without needing defaults.
    let mut p = room();
    p.wall_types = defaults.wall_types.clone();
    p.floors[0].walls[0].wall_type = Some("Brick-6".into());
    assert!(build_scene(&p)
        .meshes
        .iter()
        .any(|m| m.material == Material::Brick));
}

#[test]
fn layer_names_map_to_materials() {
    assert_eq!(Material::from_layer_name("Stucco"), Some(Material::Stucco));
    assert_eq!(
        Material::from_layer_name("Lap SIDING"),
        Some(Material::Siding)
    );
    assert_eq!(Material::from_layer_name("Brick"), Some(Material::Brick));
    assert_eq!(
        Material::from_layer_name("Stone Veneer"),
        Some(Material::Stone)
    );
    assert_eq!(
        Material::from_layer_name("Drywall"),
        Some(Material::WallInterior)
    );
    assert_eq!(
        Material::from_layer_name("Concrete"),
        Some(Material::Concrete)
    );
    assert_eq!(Material::from_layer_name("Fir Framing"), None);
    assert_eq!(Material::ALL.len(), 24);
    for (i, m) in Material::ALL.iter().enumerate() {
        assert_eq!(m.index(), i);
    }
}
