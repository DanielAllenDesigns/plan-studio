//! The Specification tabs of round 15 in 3D: window shapes, treatments, the
//! Materials tab, Size Excludes Frame, a recessed leaf and the double-door
//! swing choices (docs/parity/doors-windows.md, DW-35, DW-56, DW-57, DW-82,
//! DW-117, DW-121, DW-123).

use plan_3d::{build_scene_with, Material, Mesh, Scene, SceneOptions};
use plan_core::openings::spec::{
    BlindStyle, CurtainStyle, DoorSwing, InteriorShutterStyle, MillworkStyle, ShapeKind,
    WindowShape,
};
use plan_core::{Id, OpeningKind, OpeningStyle, Point, Project, WallKind};

/// One 10' exterior wall along +x, 4 1/2" thick and 100" tall, with one
/// opening in its middle.
fn project(kind: OpeningKind, style: OpeningStyle, width: f64) -> (Project, Id) {
    let mut p = Project::new("t");
    let w = p.add_wall(
        0,
        Point::ZERO,
        Point::new(120.0, 0.0),
        4.5,
        100.0,
        WallKind::Exterior,
    );
    let id = p.add_opening(0, w, 60.0, kind).unwrap();
    let o = &mut p.floors[0].openings[0];
    o.style = style;
    o.width = width;
    (p, id)
}

fn window() -> (Project, Id) {
    project(OpeningKind::Window, OpeningStyle::Window, 48.0)
}

fn door() -> (Project, Id) {
    project(OpeningKind::Door, OpeningStyle::DoubleDoor, 60.0)
}

fn spec(p: &mut Project) -> &mut plan_core::openings::OpeningSpec {
    &mut p.floors[0].openings[0].extras.spec
}

fn scene(p: &Project) -> Scene {
    build_scene_with(
        p,
        &SceneOptions {
            show_casing: true,
            ..Default::default()
        },
    )
}

fn meshes<'a>(
    s: &'a Scene,
    id: Id,
    pick: impl Fn(&Mesh) -> bool + 'a,
) -> impl Iterator<Item = &'a Mesh> {
    s.meshes
        .iter()
        .filter(move |m| m.object_id == Some(id) && pick(m))
}

fn extent<'a>(it: impl Iterator<Item = &'a Mesh>, axis: usize) -> (f32, f32) {
    it.flat_map(|m| m.vertices.iter().map(move |v| v.position[axis]))
        .fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
}

fn tris<'a>(it: impl Iterator<Item = &'a Mesh>) -> usize {
    it.map(Mesh::triangle_count).sum()
}

/// Area of the glass seen from the front: the sum of the triangles facing
/// the wall's normal, projected on x-y.
fn glass_area(s: &Scene, id: Id) -> f64 {
    let mut a = 0.0;
    for m in meshes(s, id, |m| m.material == Material::WindowGlass) {
        for t in m.indices.chunks(3) {
            let p: Vec<[f32; 3]> = t.iter().map(|i| m.vertices[*i as usize].position).collect();
            let n = m.vertices[t[0] as usize].normal;
            // Front faces: along -z in scene space (the wall's +y is -z).
            if n[2] < -0.9 {
                a += 0.5
                    * f64::from(
                        ((p[1][0] - p[0][0]) * (p[2][1] - p[0][1])
                            - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]))
                            .abs(),
                    );
            }
        }
    }
    a
}

#[test]
fn a_shaped_window_is_glazed_to_its_outline_and_the_wall_fills_the_rest() {
    let (mut p, id) = window();
    let rect_area = glass_area(&scene(&p), id);
    assert!(rect_area > 1000.0);
    let walls_before = tris(meshes(&scene(&p), id, |m| {
        matches!(m.material, Material::WallExterior | Material::WallInterior)
    }));
    assert_eq!(walls_before, 0, "a rectangle needs no corner fill");
    for kind in [
        ShapeKind::HalfRound,
        ShapeKind::QuarterRoundLeft,
        ShapeKind::QuarterRoundRight,
        ShapeKind::Trapezoid,
        ShapeKind::Triangle,
    ] {
        let o = &mut p.floors[0].openings[0];
        let (w, h) = (o.width, o.height);
        o.extras.spec.shape = WindowShape::preset(kind, w, h);
        let s = scene(&p);
        let area = glass_area(&s, id);
        assert!(area > 100.0, "{kind:?} has glass");
        assert!(area < rect_area * 0.99, "{kind:?}: {area} vs {rect_area}");
        // Glass stays inside the hole.
        let (x0, x1) = extent(meshes(&s, id, |m| m.material == Material::WindowGlass), 0);
        assert!(x0 >= 36.0 - 0.01 && x1 <= 84.0 + 0.01, "{kind:?} {x0} {x1}");
        // Wall fills the corners the outline leaves.
        assert!(
            tris(meshes(&s, id, |m| m.material == Material::WallExterior)) > 0,
            "{kind:?}"
        );
    }
}

#[test]
fn a_triangle_window_peaks_over_the_middle() {
    let (mut p, id) = window();
    let o = &mut p.floors[0].openings[0];
    let (w, h) = (o.width, o.height);
    o.extras.spec.shape = WindowShape::preset(ShapeKind::Triangle, w, h);
    let s = scene(&p);
    let top = 24.0 + 60.0;
    let (_, y1) = extent(meshes(&s, id, |m| m.material == Material::WindowGlass), 1);
    // The glass peak sits near the head (a narrow apex insets further than a
    // flat head would), over x = 60.
    assert!(y1 > (top - 10.0) as f32 && y1 < top as f32, "{y1}");
    let peak_x: Vec<f32> = meshes(&s, id, |m| m.material == Material::WindowGlass)
        .flat_map(|m| m.vertices.iter())
        .filter(|v| v.position[1] > y1 - 0.5)
        .map(|v| v.position[0])
        .collect();
    assert!(peak_x.iter().all(|x| (x - 60.0).abs() < 2.5), "{peak_x:?}");
}

#[test]
fn the_lite_pattern_follows_the_shape() {
    let (mut p, id) = window();
    {
        let o = &mut p.floors[0].openings[0];
        let (w, h) = (o.width, o.height);
        o.extras.spec.shape = WindowShape::preset(ShapeKind::HalfRound, w, h);
    }
    let plain = tris(meshes(&scene(&p), id, |m| {
        m.material == Material::WindowFrame
    }));
    p.floors[0].openings[0].lites = (3, 3);
    let gridded = tris(meshes(&scene(&p), id, |m| {
        m.material == Material::WindowFrame
    }));
    assert!(
        gridded > plain,
        "dividers add muntins: {plain} -> {gridded}"
    );
    // All the pieces stay inside the hole.
    let (x0, x1) = extent(
        meshes(&scene(&p), id, |m| m.material == Material::WindowFrame),
        0,
    );
    assert!(x0 >= 36.0 - 0.01 && x1 <= 84.0 + 0.01);
}

#[test]
fn a_size_without_the_frame_cuts_the_wall_hole_wider() {
    let (mut p, id) = window();
    p.floors[0].openings[0].extras.frame_width = Some(2.0);
    let (a0, a1) = extent(
        meshes(&scene(&p), id, |m| m.material == Material::WindowFrame),
        0,
    );
    assert!(
        (a0 - 36.0).abs() < 0.01 && (a1 - 84.0).abs() < 0.01,
        "{a0} {a1}"
    );
    spec(&mut p).size_includes_frame = false;
    let (b0, b1) = extent(
        meshes(&scene(&p), id, |m| m.material == Material::WindowFrame),
        0,
    );
    assert!(
        (b0 - 34.0).abs() < 0.01 && (b1 - 86.0).abs() < 0.01,
        "{b0} {b1}"
    );
}

#[test]
fn a_recessed_door_leaf_stands_in_from_the_outside_face() {
    let (mut p, id) = project(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
    let (z0, z1) = extent(
        meshes(&scene(&p), id, |m| m.material == Material::DoorPanel),
        2,
    );
    assert!((z0 + 0.6875).abs() < 0.01 && (z1 - 0.6875).abs() < 0.01);
    spec(&mut p).recess_depth = Some(0.5);
    let (z0, z1) = extent(
        meshes(&scene(&p), id, |m| m.material == Material::DoorPanel),
        2,
    );
    // The leaf's middle is 1 3/4" (half the 4 1/2" wall minus the 1/2")
    // from the centerline, toward the outside.
    let mid = (z0 + z1) * 0.5;
    assert!((mid.abs() - 1.75).abs() < 0.01, "{mid}");
}

#[test]
fn double_doors_swing_one_leaf_or_from_the_center() {
    let (mut p, id) = door();
    spec(&mut p).show_open_in_3d = true;
    let reach = |p: &Project| {
        let s = scene(p);
        let (z0, z1) = extent(meshes(&s, id, |m| m.material == Material::DoorPanel), 2);
        z0.abs().max(z1.abs())
    };
    // Both leaves open: each is 30" long.
    assert!((reach(&p) - 30.0).abs() < 0.1);
    // Left only: the leaf on the start side opens, the other stays shut.
    spec(&mut p).door_swing = DoorSwing::LeftOnly;
    let s = scene(&p);
    let open_xs = |s: &Scene, pick: fn(f32) -> bool| {
        meshes(s, id, |m| m.material == Material::DoorPanel)
            .flat_map(|m| m.vertices.iter())
            .filter(|v| v.position[2].abs() > 5.0 && pick(v.position[0]))
            .count()
    };
    assert!(open_xs(&s, |x| x < 90.0) > 0);
    assert_eq!(open_xs(&s, |x| x > 91.0), 0, "the end leaf stays shut");
    spec(&mut p).door_swing = DoorSwing::RightOnly;
    let s = scene(&p);
    assert!(open_xs(&s, |x| x > 30.0 && x < 90.0 || x >= 90.0) > 0);
    assert_eq!(open_xs(&s, |x| x < 29.0), 0, "the start leaf stays shut");
    // From the center both leaves pivot at x = 60.
    spec(&mut p).door_swing = DoorSwing::Both;
    spec(&mut p).swings_from_center = true;
    let s = scene(&p);
    let near_hinge = meshes(&s, id, |m| m.material == Material::DoorPanel)
        .flat_map(|m| m.vertices.iter())
        .filter(|v| v.position[2].abs() > 25.0)
        .all(|v| (v.position[0] - 60.0).abs() < 1.5);
    assert!(near_hinge, "open leaf tips stand over the middle");
}

#[test]
fn curtains_blinds_shutters_and_millwork_are_built_on_their_sides() {
    let (mut p, id) = window();
    let base = tris(meshes(&scene(&p), id, |_| true));
    let colored = |s: &Scene, rgb: [u8; 3]| -> Vec<Mesh> {
        meshes(s, id, |m| m.color == Some(rgb)).cloned().collect()
    };
    let t = &mut spec(&mut p).treatments;
    t.curtain = CurtainStyle::Pleated;
    t.curtain_color = [200, 10, 10];
    t.blind = BlindStyle::Horizontal;
    t.blind_color = [10, 200, 10];
    t.shutter = InteriorShutterStyle::Plantation;
    t.shutter_color = [10, 10, 200];
    t.millwork_above = MillworkStyle::Cornice;
    t.millwork_below = MillworkStyle::Apron;
    let s = scene(&p);
    assert!(tris(meshes(&s, id, |_| true)) > base + 100);
    let curtain = colored(&s, [200, 10, 10]);
    let blind = colored(&s, [10, 200, 10]);
    let shutter = colored(&s, [10, 10, 200]);
    assert!(!curtain.is_empty() && !blind.is_empty() && !shutter.is_empty());
    // All three stand on the room side, off the wall face.
    let side = |ms: &[Mesh]| {
        let (lo, hi) = extent(ms.iter(), 2);
        (lo, hi)
    };
    let (cl, ch) = side(&curtain);
    assert!(cl.signum() == ch.signum() || cl.abs() < 0.01 || ch.abs() < 0.01);
    let room_sign = if ch.abs() > cl.abs() { 1.0 } else { -1.0 };
    for ms in [&curtain, &blind, &shutter] {
        let (lo, hi) = side(ms);
        let far = if room_sign > 0.0 { hi } else { -lo };
        assert!(far > 1.0, "stands in the room or its recess: {lo} {hi}");
        let wrong = if room_sign > 0.0 { lo } else { -hi };
        assert!(wrong >= -0.01, "none on the outside: {lo} {hi}");
    }
    // The curtains reach beyond the casing; the millwork is on the other side.
    let (x0, x1) = extent(curtain.iter(), 0);
    assert!(x0 < 36.0 && x1 > 84.0, "panels flank the window: {x0} {x1}");
    let millwork: Vec<&Mesh> = meshes(&s, id, |m| {
        m.material == Material::Trim && m.color.is_none()
    })
    .collect();
    let (mz0, mz1) = extent(millwork.iter().copied(), 2);
    let outside = if room_sign > 0.0 { mz0 } else { -mz1 };
    assert!(
        outside < -2.25,
        "millwork reaches past the outside face: {mz0} {mz1}"
    );
    // Turning them off restores the plain window.
    *spec(&mut p) = plan_core::openings::OpeningSpec::default();
    assert_eq!(tris(meshes(&scene(&p), id, |_| true)), base);
}

#[test]
fn treatments_are_for_windows_only() {
    let (mut p, id) = door();
    spec(&mut p).treatments.curtain = CurtainStyle::Panels;
    let with = tris(meshes(&scene(&p), id, |_| true));
    *spec(&mut p) = plan_core::openings::OpeningSpec::default();
    assert_eq!(with, tris(meshes(&scene(&p), id, |_| true)));
}

#[test]
fn the_materials_tab_paints_each_component() {
    let (mut p, id) = window();
    {
        let m = &mut spec(&mut p).materials;
        m.set("Frame", Some(("Oak", [160, 110, 60])));
        m.set("Sash", Some(("Painted", [20, 30, 40])));
        m.set("Glass", Some(("Tinted", [10, 60, 20])));
        m.set("Casing", Some(("Pine", [200, 180, 120])));
        m.set("Sill", Some(("Stone", [90, 90, 95])));
    }
    spec(&mut p).sill.enabled = true;
    let s = scene(&p);
    let has = |rgb: [u8; 3]| meshes(&s, id, |m| m.color == Some(rgb)).count() > 0;
    for rgb in [
        [160, 110, 60],
        [20, 30, 40],
        [10, 60, 20],
        [200, 180, 120],
        [90, 90, 95],
    ] {
        assert!(has(rgb), "{rgb:?}");
    }
    // The glass paint sits on glass, the casing paint on trim.
    assert!(meshes(&s, id, |m| m.color == Some([10, 60, 20]))
        .all(|m| m.material == Material::WindowGlass));
    assert!(
        meshes(&s, id, |m| m.color == Some([200, 180, 120])).all(|m| m.material == Material::Trim)
    );
    // An unpainted component keeps its material's own color.
    spec(&mut p).materials.set("Frame", None);
    let s = scene(&p);
    assert!(
        meshes(&s, id, |m| m.material == Material::WindowFrame
            && m.color.is_none())
        .count()
            > 0
    );
    // A door has its own components.
    let (mut p, id) = project(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
    spec(&mut p)
        .materials
        .set("Door Panel", Some(("Walnut", [90, 50, 30])));
    spec(&mut p)
        .materials
        .set("Threshold", Some(("Brass", [190, 150, 50])));
    spec(&mut p)
        .materials
        .set("Jamb", Some(("Oak", [160, 110, 60])));
    let s = scene(&p);
    for rgb in [[90, 50, 30], [190, 150, 50], [160, 110, 60]] {
        assert!(
            meshes(&s, id, |m| m.color == Some(rgb)).count() > 0,
            "{rgb:?}"
        );
    }
}
