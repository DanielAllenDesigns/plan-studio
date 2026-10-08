//! Meshes of the door and window variants added with the Door and Window
//! flyouts: double doors, sliding doors of 2 to 4 panels, bifold pairs,
//! casement, sliding, awning and hopper windows, and the side bay, bow and box
//! windows project to.

use plan_3d::{build_scene, build_scene_with, Material, Mesh, Scene, SceneOptions};
use plan_core::{Id, OpeningKind, OpeningStyle, Point, Project, WallKind};

fn ft(f: f64) -> f64 {
    f * 12.0
}

/// A 20' interior wall with one opening of `style` and `width` at its middle.
fn with_opening(kind: OpeningKind, style: OpeningStyle, width: f64) -> (Project, Id) {
    let mut p = Project::new("t");
    let wall = p.add_wall(
        0,
        Point::ZERO,
        Point::new(ft(20.0), 0.0),
        4.5,
        100.0,
        WallKind::Interior,
    );
    let id = p.add_opening(0, wall, ft(10.0), kind).unwrap();
    let o = p.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
        .unwrap();
    o.style = style;
    o.width = width;
    (p, id)
}

fn tris(scene: &Scene, id: Id, material: Material) -> usize {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == material)
        .map(Mesh::triangle_count)
        .sum()
}

/// Mean of the coordinate along the wall's normal axis for one material.
fn mean_normal_axis(scene: &Scene, id: Id, material: Material) -> f32 {
    let v: Vec<f32> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == material)
        .flat_map(|m| m.vertices.iter().map(|v| v.position[2]))
        .collect();
    v.iter().sum::<f32>() / v.len().max(1) as f32
}

/// A box is 12 triangles.
const BOX: usize = 12;

#[test]
fn double_door_is_two_slabs() {
    let (p, single) = with_opening(OpeningKind::Door, OpeningStyle::Hinged, 60.0);
    let (q, double) = with_opening(OpeningKind::Door, OpeningStyle::DoubleDoor, 60.0);
    let one = tris(&build_scene(&p), single, Material::DoorPanel);
    let two = tris(&build_scene(&q), double, Material::DoorPanel);
    assert_eq!(one, BOX);
    assert_eq!(two, 2 * BOX);
}

#[test]
fn double_door_leaves_swing_open_from_both_jambs() {
    let (p, id) = with_opening(OpeningKind::Door, OpeningStyle::DoubleDoor, 60.0);
    let closed = build_scene(&p);
    let open = build_scene_with(
        &p,
        &SceneOptions {
            doors_open: true,
            ..Default::default()
        },
    );
    // Closed leaves stay in the wall plane; open ones swing out of it.
    let reach = |s: &Scene| {
        s.meshes
            .iter()
            .filter(|m| m.object_id == Some(id))
            .flat_map(|m| m.vertices.iter().map(|v| v.position[2].abs()))
            .fold(0.0_f32, f32::max)
    };
    assert!(reach(&closed) < 2.0);
    assert!(reach(&open) > 25.0, "{}", reach(&open));
}

#[test]
fn sliding_doors_have_two_to_four_panes() {
    for (w, n) in [(36.0, 2), (72.0, 2), (120.0, 3), (192.0, 4)] {
        let (p, id) = with_opening(OpeningKind::Door, OpeningStyle::Sliding, w);
        let scene = build_scene(&p);
        assert_eq!(tris(&scene, id, Material::Glass), n * BOX, "{w}\" wide");
    }
}

#[test]
fn sliding_panels_alternate_between_two_tracks() {
    let (p, id) = with_opening(OpeningKind::Door, OpeningStyle::Sliding, 144.0);
    let scene = build_scene(&p);
    // Three panes on two tracks: the panes' faces fall on four distinct planes
    // (two per track), straddling the wall centerline.
    let mut planes: Vec<i64> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == Material::Glass)
        .flat_map(|m| {
            m.vertices
                .iter()
                .map(|v| (v.position[2] * 100.0).round() as i64)
        })
        .collect();
    planes.sort_unstable();
    planes.dedup();
    assert_eq!(planes.len(), 4, "{planes:?}");
    assert!(planes[0] < 0 && planes[3] > 0);
}

#[test]
fn bifold_is_a_pair_or_two_pairs() {
    let (p, narrow) = with_opening(OpeningKind::Door, OpeningStyle::Bifold, 36.0);
    let (q, wide) = with_opening(OpeningKind::Door, OpeningStyle::Bifold, 72.0);
    assert_eq!(tris(&build_scene(&p), narrow, Material::DoorPanel), 2 * BOX);
    assert_eq!(tris(&build_scene(&q), wide, Material::DoorPanel), 4 * BOX);
}

#[test]
fn casement_gets_a_second_sash_from_four_feet() {
    let (p, one) = with_opening(OpeningKind::Window, OpeningStyle::Casement, 30.0);
    let (q, two) = with_opening(OpeningKind::Window, OpeningStyle::Casement, 60.0);
    let g1 = tris(&build_scene(&p), one, Material::WindowGlass);
    let g2 = tris(&build_scene(&q), two, Material::WindowGlass);
    assert_eq!(g1, BOX);
    assert_eq!(g2, 2 * BOX);
}

#[test]
fn sliding_window_has_two_glazed_sashes() {
    let (p, id) = with_opening(OpeningKind::Window, OpeningStyle::SlidingWindow, 60.0);
    let scene = build_scene(&p);
    assert_eq!(tris(&scene, id, Material::WindowGlass), 2 * BOX);
    // A sliding style on a window is the sliding window, not the door.
    let (q, win) = with_opening(OpeningKind::Window, OpeningStyle::Sliding, 60.0);
    let sq = build_scene(&q);
    assert_eq!(tris(&sq, win, Material::WindowGlass), 2 * BOX);
    assert_eq!(tris(&sq, win, Material::DoorPanel), 0);
}

#[test]
fn awning_and_hopper_carry_a_hinge_bar_at_head_and_sill() {
    let (p, awning) = with_opening(OpeningKind::Window, OpeningStyle::Awning, 36.0);
    let (q, hopper) = with_opening(OpeningKind::Window, OpeningStyle::Hopper, 36.0);
    let top = |s: &Scene, id: Id| {
        s.meshes
            .iter()
            .filter(|m| m.object_id == Some(id) && m.material == Material::Metal)
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .fold(f32::MIN, f32::max)
    };
    let (sa, sh) = (build_scene(&p), build_scene(&q));
    assert_eq!(tris(&sa, awning, Material::Metal), BOX);
    assert_eq!(tris(&sh, hopper, Material::Metal), BOX);
    assert!(top(&sa, awning) > top(&sh, hopper) + 20.0);
}

#[test]
fn projecting_windows_follow_the_swing_flip() {
    for style in [
        OpeningStyle::BayWindow,
        OpeningStyle::BoxWindow,
        OpeningStyle::BowWindow,
    ] {
        let (mut p, id) = with_opening(OpeningKind::Window, style, 72.0);
        let before = mean_normal_axis(&build_scene(&p), id, Material::Roof);
        p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap()
            .swing_flipped = true;
        let after = mean_normal_axis(&build_scene(&p), id, Material::Roof);
        assert!(before * after < 0.0, "{style:?}: {before} vs {after}");
    }
}
