//! Chief-style elevation detail: regions, hatches, poche, shadows, depth weights, labels.

use plan_3d::{build_scene, Material, Mesh, Scene, Vertex};
use plan_core::geometry::{dist_to_segment, point_in_polygon};
use plan_core::{OpeningKind, Point, Project, WallKind};
use plan_elevation::{
    annotate, elevation, elevation_with_labels, section, Drawing, EdgeKind, LineWeight, Options,
    Region, RegionKind, SectionCut, SunDir, ViewDir,
};

const WALL_H: f64 = 109.125;

fn opts() -> Options {
    Options {
        raster_px: 256,
        ..Options::default()
    }
}

fn ft(f: f64) -> f64 {
    f * 12.0
}

fn wall_project(with_door: bool) -> Project {
    let mut p = Project::new("wall");
    let id = p.add_wall(
        0,
        Point::ZERO,
        Point::new(ft(10.0), 0.0),
        4.5,
        WALL_H,
        WallKind::Exterior,
    );
    if with_door {
        p.add_opening(0, id, ft(5.0), OpeningKind::Door)
            .expect("door fits");
    }
    p
}

fn room_project() -> Project {
    let mut p = Project::new("room");
    let (w, h) = (ft(20.0), ft(10.0));
    let c = [
        Point::new(0.0, 0.0),
        Point::new(w, 0.0),
        Point::new(w, h),
        Point::new(0.0, h),
    ];
    for i in 0..4 {
        p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, WALL_H, WallKind::Exterior);
    }
    p
}

/// A mesh from corner positions and CCW-from-outside quads.
fn quads(pts: &[[f32; 3]], quads: &[[usize; 4]], material: Material, id: Option<u64>) -> Mesh {
    let vertices = pts
        .iter()
        .map(|&position| Vertex {
            position,
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        })
        .collect();
    let mut indices = Vec::new();
    for q in quads {
        indices.extend([q[0], q[1], q[2], q[0], q[2], q[3]].map(|i| i as u32));
    }
    Mesh {
        vertices,
        indices,
        material,
        object_id: id,
        color: None,
    }
}

fn cuboid(x: (f32, f32), y: (f32, f32), z: (f32, f32), material: Material, id: u64) -> Mesh {
    let p = |a: usize, b: usize, c: usize| {
        [
            if a == 0 { x.0 } else { x.1 },
            if b == 0 { y.0 } else { y.1 },
            if c == 0 { z.0 } else { z.1 },
        ]
    };
    let pts = [
        p(0, 0, 0),
        p(1, 0, 0),
        p(1, 1, 0),
        p(0, 1, 0),
        p(0, 0, 1),
        p(1, 0, 1),
        p(1, 1, 1),
        p(0, 1, 1),
    ];
    quads(
        &pts,
        &[
            [4, 5, 6, 7], // +z
            [1, 0, 3, 2], // -z
            [5, 1, 2, 6], // +x
            [0, 4, 7, 3], // -x
            [7, 6, 2, 3], // +y
            [0, 1, 5, 4], // -y
        ],
        material,
        Some(id),
    )
}

fn scene_of(meshes: Vec<Mesh>) -> Scene {
    Scene { meshes }
}

fn regions_of(d: &Drawing, kind: RegionKind) -> Vec<&Region> {
    d.regions.iter().filter(|r| r.kind == kind).collect()
}

fn near_boundary(poly: &[Point], p: Point, tol: f64) -> bool {
    (0..poly.len()).any(|i| dist_to_segment(p, poly[i], poly[(i + 1) % poly.len()]) <= tol)
}

#[test]
fn regions_cover_the_visible_wall() {
    for with_door in [false, true] {
        let scene = build_scene(&wall_project(with_door));
        let d = elevation(&scene, ViewDir::Front, &opts());
        let faces = regions_of(&d, RegionKind::Face);
        assert!(!faces.is_empty());
        let (w, h) = d.size();
        let covered: f64 = faces.iter().map(|r| r.area()).sum();
        let ratio = covered / (w * h);
        assert!(
            (0.95..=1.02).contains(&ratio),
            "door={with_door}: coverage {ratio:.3}"
        );
        assert!(regions_of(&d, RegionKind::Cut).is_empty());
    }
}

#[test]
fn regions_can_be_switched_off() {
    let scene = build_scene(&wall_project(false));
    let off = Options {
        regions: false,
        ..opts()
    };
    let d = elevation(&scene, ViewDir::Front, &off);
    assert!(d.regions.is_empty());
    let on = elevation(&scene, ViewDir::Front, &opts());
    assert_eq!(d.lines, on.lines, "regions never change the line work");
}

#[test]
fn window_hole_is_excluded_from_the_wall_region() {
    let scene = scene_of(vec![
        cuboid((0.0, 40.0), (0.0, 100.0), (-6.0, 0.0), Material::Brick, 1),
        cuboid((80.0, 120.0), (0.0, 100.0), (-6.0, 0.0), Material::Brick, 1),
        cuboid((40.0, 80.0), (70.0, 100.0), (-6.0, 0.0), Material::Brick, 1),
        cuboid((40.0, 80.0), (0.0, 30.0), (-6.0, 0.0), Material::Brick, 1),
        cuboid((40.0, 80.0), (30.0, 70.0), (-6.0, -4.0), Material::Glass, 2),
    ]);
    let d = elevation(&scene, ViewDir::Front, &opts());
    let brick: Vec<_> = d
        .regions
        .iter()
        .filter(|r| r.material == Material::Brick && r.kind == RegionKind::Face)
        .collect();
    assert_eq!(brick.len(), 1);
    let total = 120.0 * 100.0;
    assert!((brick[0].area() - (total - 40.0 * 40.0)).abs() / total < 0.03);
    assert!(!point_in_polygon(Point::new(60.0, 50.0), &brick[0].polygon));
    let glass: Vec<_> = d
        .regions
        .iter()
        .filter(|r| r.material == Material::Glass)
        .collect();
    assert_eq!(glass.len(), 1);
    assert_eq!(glass[0].object_id, Some(2));
}

#[test]
fn hatch_strokes_lie_inside_their_regions() {
    let mats = [
        Material::Brick,
        Material::Siding,
        Material::Stucco,
        Material::Stone,
        Material::Roof,
        Material::Concrete,
        Material::Glass,
    ];
    for m in mats {
        let scene = scene_of(vec![
            cuboid((0.0, 160.0), (0.0, 100.0), (-6.0, 0.0), m, 1),
            cuboid((0.0, 60.0), (20.0, 60.0), (0.0, 2.0), Material::Trim, 2),
        ]);
        let o = Options {
            hatch: true,
            ..opts()
        };
        let d = elevation(&scene, ViewDir::Front, &o);
        let hatch: Vec<_> = d
            .lines
            .iter()
            .filter(|l| l.kind == EdgeKind::Hatch)
            .collect();
        assert!(hatch.len() > 10, "{m:?}: {} hatch strokes", hatch.len());
        assert!(hatch.iter().all(|l| l.weight == LineWeight::Light));
        let faces = regions_of(&d, RegionKind::Face);
        for l in &hatch {
            let mid = Point::lerp(l.a, l.b, 0.5);
            let ok = faces
                .iter()
                .any(|r| point_in_polygon(mid, &r.polygon) || near_boundary(&r.polygon, mid, 0.05));
            assert!(ok, "{m:?}: stroke midpoint {mid:?} outside every region");
            assert!(
                !faces
                    .iter()
                    .any(|r| r.material == Material::Trim && point_in_polygon(mid, &r.polygon)),
                "hatch leaked into the unhatched trim region"
            );
        }
        let plain = elevation(&scene, ViewDir::Front, &opts());
        assert!(plain.lines.iter().all(|l| l.kind != EdgeKind::Hatch));
    }
}

#[test]
fn siding_hatch_is_horizontal_and_brick_has_joints() {
    let mk = |m| scene_of(vec![cuboid((0.0, 160.0), (0.0, 100.0), (-6.0, 0.0), m, 1)]);
    let o = Options {
        hatch: true,
        ..opts()
    };
    let siding = elevation(&mk(Material::Siding), ViewDir::Front, &o);
    let courses: Vec<_> = siding
        .lines
        .iter()
        .filter(|l| l.kind == EdgeKind::Hatch)
        .collect();
    assert!(
        (14..=17).contains(&courses.len()),
        "{} courses",
        courses.len()
    );
    assert!(courses.iter().all(|l| (l.a.y - l.b.y).abs() < 1e-6));
    let brick = elevation(&mk(Material::Brick), ViewDir::Front, &o);
    let vertical = brick
        .lines
        .iter()
        .filter(|l| l.kind == EdgeKind::Hatch && (l.a.x - l.b.x).abs() < 1e-6)
        .count();
    assert!(vertical > 100);
}

#[test]
fn hatch_follows_the_drawing_scale_and_rehatch_matches_a_fresh_drawing() {
    let scene = scene_of(vec![cuboid(
        (0.0, 480.0),
        (0.0, 240.0),
        (-6.0, 0.0),
        Material::Brick,
        1,
    )]);
    let at = |scale: f64| Options {
        hatch: true,
        hatch_scale: scale,
        ..opts()
    };
    let count =
        |d: &plan_elevation::Drawing| d.lines.iter().filter(|l| l.kind == EdgeKind::Hatch).count();
    let quarter = elevation(&scene, ViewDir::Front, &at(0.25));
    let tiny = elevation(&scene, ViewDir::Front, &at(0.02));
    assert!(
        count(&tiny) < count(&quarter) / 2,
        "a small scale coarsens the courses: {} vs {}",
        count(&tiny),
        count(&quarter)
    );
    // Making the hatch again for another scale gives what a fresh drawing at
    // that scale has, and a drawing without hatch is left alone.
    let mut again = quarter.clone();
    assert!(again.rehatch(0.02));
    assert_eq!(count(&again), count(&tiny));
    assert_eq!(again.lines.len(), tiny.lines.len());
    let mut plain = elevation(&scene, ViewDir::Front, &opts());
    assert!(!plain.rehatch(0.02));
}

/// Two parallel walls, no room: no floor or ceiling slabs get cut.
fn two_wall_project() -> Project {
    let mut project = Project::new("two walls");
    for x in [0.0, ft(20.0)] {
        project.add_wall(
            0,
            Point::new(x, 0.0),
            Point::new(x, ft(10.0)),
            6.5,
            WALL_H,
            WallKind::Exterior,
        );
    }
    project
}

#[test]
fn section_through_two_walls_has_two_poche_regions() {
    let scene = build_scene(&two_wall_project());
    let cut = SectionCut {
        plane_normal: ViewDir::Front,
        offset: -60.0,
    };
    let d = section(&scene, cut, &opts());
    let poche: Vec<_> = d.cut_regions().collect();
    assert_eq!(poche.len(), 2, "{poche:?}");
    for r in &poche {
        assert_eq!(r.kind, RegionKind::Cut);
        let expect = 6.5 * WALL_H;
        assert!(
            (r.area() - expect).abs() / expect < 0.1,
            "poche area {} vs {expect}",
            r.area()
        );
    }
    // The cut outline is Heavy.
    assert!(d
        .lines
        .iter()
        .filter(|l| l.kind == EdgeKind::Cut)
        .all(|l| l.weight == LineWeight::Heavy));
    // Cut faces are not hatched.
    let o = Options {
        hatch: true,
        ..opts()
    };
    let hatched = section(&scene, cut, &o);
    assert_eq!(hatched.cut_regions().count(), 2);
}

#[test]
fn section_depth_limits_what_lies_beyond_the_cut() {
    let mut project = room_project();
    let north = project.floors[0].walls[2].id;
    project
        .add_opening(0, north, ft(10.0), OpeningKind::Door)
        .expect("door fits");
    let scene = build_scene(&project);
    let cut = SectionCut {
        plane_normal: ViewDir::Front,
        offset: -60.0,
    };
    let full = section(&scene, cut, &opts());
    let jamb = |d: &Drawing| {
        d.lines.iter().any(|l| {
            l.kind != EdgeKind::Cut && (l.a.x - 102.0).abs() < 1.5 && (l.a.x - l.b.x).abs() < 1e-6
        })
    };
    assert!(jamb(&full), "door jamb of the far wall is drawn");

    let shallow = section(
        &scene,
        cut,
        &Options {
            section_depth: Some(12.0),
            ..opts()
        },
    );
    assert!(!jamb(&shallow), "far wall is beyond the section depth");
    let wall_poche = |d: &Drawing| d.cut_regions().filter(|r| r.object_id.is_some()).count();
    assert_eq!(
        wall_poche(&shallow),
        wall_poche(&full),
        "poche is unaffected"
    );
    assert_eq!(wall_poche(&full), 2);
    assert!(shallow.lines.len() < full.lines.len());
    let deep = section(
        &scene,
        cut,
        &Options {
            section_depth: Some(500.0),
            ..opts()
        },
    );
    assert!(jamb(&deep));
}

/// A 120x100 wall with a 40x40 window whose glass is recessed 4" behind the face.
fn reveal_scene() -> Scene {
    scene_of(vec![
        cuboid((0.0, 40.0), (0.0, 100.0), (-6.0, 0.0), Material::Stucco, 1),
        cuboid(
            (80.0, 120.0),
            (0.0, 100.0),
            (-6.0, 0.0),
            Material::Stucco,
            1,
        ),
        cuboid(
            (40.0, 80.0),
            (70.0, 100.0),
            (-6.0, 0.0),
            Material::Stucco,
            1,
        ),
        cuboid((40.0, 80.0), (0.0, 30.0), (-6.0, 0.0), Material::Stucco, 1),
        cuboid((40.0, 80.0), (30.0, 70.0), (-6.0, -4.0), Material::Glass, 2),
    ])
}

#[test]
fn sun_from_the_upper_left_shadows_the_window_reveal() {
    let sun = SunDir {
        azimuth_deg: 225.0,
        altitude_deg: 40.0,
    };
    let o = Options {
        shadows: Some(sun),
        ..opts()
    };
    let d = elevation(&reveal_scene(), ViewDir::Front, &o);
    let shadows = regions_of(&d, RegionKind::Shadow);
    assert!(!shadows.is_empty(), "no shadow regions");
    // The left jamb throws about 4" of shadow onto the glass, the head about 4.7".
    let on_glass = |p: Point| {
        shadows
            .iter()
            .any(|r| r.material == Material::Glass && point_in_polygon(p, &r.polygon))
    };
    assert!(on_glass(Point::new(42.0, 50.0)), "left jamb shadow");
    assert!(on_glass(Point::new(60.0, 68.0)), "head shadow");
    assert!(!on_glass(Point::new(70.0, 40.0)), "lit corner of the glass");
    // Lit wall face stays unshadowed.
    assert!(!shadows
        .iter()
        .any(|r| point_in_polygon(Point::new(10.0, 50.0), &r.polygon)));
    // Shadows are an overlay: the face regions are unchanged.
    let plain = elevation(&reveal_scene(), ViewDir::Front, &opts());
    assert_eq!(
        regions_of(&d, RegionKind::Face).len(),
        regions_of(&plain, RegionKind::Face).len()
    );
}

#[test]
fn sun_behind_the_camera_casts_no_shadow_on_the_face() {
    let behind = SunDir {
        azimuth_deg: 180.0,
        altitude_deg: 0.0,
    };
    let o = Options {
        shadows: Some(behind),
        ..opts()
    };
    let d = elevation(&reveal_scene(), ViewDir::Front, &o);
    assert!(regions_of(&d, RegionKind::Shadow).is_empty());
    // Sun behind the building: every visible face is turned away from it.
    let back = SunDir {
        azimuth_deg: 0.0,
        altitude_deg: 30.0,
    };
    let o = Options {
        shadows: Some(back),
        ..opts()
    };
    let d = elevation(&reveal_scene(), ViewDir::Front, &o);
    let covered: f64 = regions_of(&d, RegionKind::Shadow)
        .iter()
        .map(|r| r.area())
        .sum();
    assert!(
        covered > 0.9 * 120.0 * 100.0,
        "self-shadowed area {covered}"
    );
}

#[test]
fn ground_shadow_appears_in_the_plan_view() {
    let scene = scene_of(vec![
        cuboid((0.0, 200.0), (0.0, 4.0), (-200.0, 0.0), Material::Floor, 1),
        cuboid(
            (60.0, 100.0),
            (4.0, 100.0),
            (-100.0, -60.0),
            Material::Concrete,
            2,
        ),
    ]);
    let o = Options {
        shadows: Some(SunDir {
            azimuth_deg: 225.0,
            altitude_deg: 35.0,
        }),
        ..opts()
    };
    let d = elevation(&scene, ViewDir::Top, &o);
    // The slab is a visible surface everywhere, so the pillar's shadow falls on it.
    assert!(!regions_of(&d, RegionKind::Shadow).is_empty());
}

#[test]
fn depth_weights_step_far_lines_down_one_class() {
    let scene = scene_of(vec![
        cuboid((0.0, 100.0), (0.0, 100.0), (0.0, 20.0), Material::Stucco, 1),
        cuboid(
            (200.0, 300.0),
            (0.0, 100.0),
            (-300.0, -280.0),
            Material::Stucco,
            2,
        ),
    ]);
    let heavy_x = |d: &Drawing| -> Vec<f64> {
        d.lines
            .iter()
            .filter(|l| l.weight == LineWeight::Heavy && (l.a.x - l.b.x).abs() < 1e-6)
            .map(|l| l.a.x)
            .collect()
    };
    let flat = elevation(&scene, ViewDir::Front, &opts());
    assert!(heavy_x(&flat).iter().any(|&x| x > 190.0));
    let weighted = elevation(
        &scene,
        ViewDir::Front,
        &Options {
            depth_weights: true,
            ..opts()
        },
    );
    assert!(
        heavy_x(&weighted).iter().all(|&x| x < 150.0),
        "far box stays heavy"
    );
    assert!(
        heavy_x(&weighted).iter().any(|&x| x < 10.0),
        "near box stays heavy"
    );
    let far_medium = weighted.lines.iter().any(|l| {
        l.weight == LineWeight::Medium
            && (l.a.x - 200.0).abs() < 1.0
            && (l.a.x - l.b.x).abs() < 1e-6
    });
    assert!(far_medium);
}

#[test]
fn labels_title_levels_and_grade() {
    let d = elevation_with_labels(&wall_project(false), ViewDir::Front, &opts());
    let texts: Vec<&str> = d.texts.iter().map(|(_, t)| t.as_str()).collect();
    assert!(texts.contains(&"FRONT ELEVATION"), "{texts:?}");
    assert!(texts.contains(&"T.O. PLATE 9'-1 1/8\""), "{texts:?}");
    assert!(texts.contains(&"T.O. SUBFLOOR 0'-0\""), "{texts:?}");
    assert!(d.lines.iter().any(|l| l.kind == EdgeKind::Annotation));
    // Callouts sit left of the building, the title below it.
    let (lo, _) = d.bounds;
    for (p, t) in &d.texts {
        if t.starts_with("T.O.") {
            assert!(p.x < lo.x, "{t} at {p:?}");
        }
        if t == "FRONT ELEVATION" {
            assert!(p.y < 0.0);
        }
    }
    let side = elevation_with_labels(&wall_project(false), ViewDir::Left, &opts());
    assert!(side.texts.iter().any(|(_, t)| t == "LEFT ELEVATION"));
}

#[test]
fn roof_pitch_symbols_follow_the_gable_rakes() {
    // Gable end view of an 8:12 roof, ridge along Z.
    let pts = [
        [0.0, 0.0, 0.0],
        [240.0, 0.0, 0.0],
        [120.0, 80.0, 0.0],
        [0.0, 0.0, -240.0],
        [240.0, 0.0, -240.0],
        [120.0, 80.0, -240.0],
    ];
    let roof = quads(
        &pts,
        &[
            [0, 1, 2, 2], // front gable (degenerate quad tail)
            [4, 3, 5, 5], // back gable
            [0, 2, 5, 3], // left slope
            [1, 4, 5, 2], // right slope
            [0, 3, 4, 1], // soffit
        ],
        Material::Roof,
        Some(9),
    );
    let scene = scene_of(vec![roof]);
    let mut d = elevation(&scene, ViewDir::Front, &opts());
    let before = d.lines.len();
    annotate(&mut d, &scene, &wall_project(false), ViewDir::Front);
    let pitches: Vec<_> = d.texts.iter().filter(|(_, t)| t == "8:12").collect();
    assert_eq!(pitches.len(), 2, "{:?}", d.texts);
    let triangles = d
        .lines
        .iter()
        .filter(|l| l.kind == EdgeKind::Annotation)
        .count();
    assert!(triangles >= 6 && d.lines.len() > before);
    // The symbols sit near the two rake edges, above the roof outline.
    for (p, _) in pitches {
        assert!(p.y > 0.0 && p.y < 200.0);
    }
}

#[test]
fn svg_draws_regions_hatch_and_text() {
    let o = Options {
        hatch: true,
        ..opts()
    };
    let scene = scene_of(vec![cuboid(
        (0.0, 160.0),
        (0.0, 100.0),
        (-6.0, 0.0),
        Material::Brick,
        1,
    )]);
    let d = elevation(&scene, ViewDir::Front, &o);
    let svg = d.svg();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("<path"));
    assert!(svg.matches("<line").count() > 100);
    assert!(svg.trim_end().ends_with("</svg>"));

    let room = build_scene(&two_wall_project());
    let cut = SectionCut {
        plane_normal: ViewDir::Front,
        offset: -60.0,
    };
    let sec = section(&room, cut, &opts());
    let svg = sec.svg();
    assert!(svg.starts_with("<svg"));
    assert_eq!(svg.matches("fill=\"#8c8c8c\"").count(), 2, "poche gray");

    let labelled = elevation_with_labels(&wall_project(false), ViewDir::Front, &opts());
    let svg = labelled.svg();
    assert!(svg.contains("<text") && svg.contains("FRONT ELEVATION"));

    let shadowed = elevation(
        &reveal_scene(),
        ViewDir::Front,
        &Options {
            shadows: Some(SunDir {
                azimuth_deg: 225.0,
                altitude_deg: 40.0,
            }),
            ..opts()
        },
    );
    assert!(shadowed.svg().contains("fill-opacity"));
}

#[test]
fn drawing_with_regions_round_trips_through_json() {
    let o = Options {
        hatch: true,
        ..opts()
    };
    let mut d = elevation(&reveal_scene(), ViewDir::Front, &o);
    d.texts.push((Point::new(1.0, 2.0), "NOTE".into()));
    let json = serde_json::to_string(&d).expect("serialises");
    let back: Drawing = serde_json::from_str(&json).expect("deserialises");
    // serde_json's default float parsing may differ in the last bit.
    let close = |p: Point, q: Point| p.dist(q) < 1e-9;
    assert_eq!(back.lines.len(), d.lines.len());
    assert!(back
        .lines
        .iter()
        .zip(&d.lines)
        .all(|(a, b)| close(a.a, b.a) && close(a.b, b.b) && a.kind == b.kind));
    assert_eq!(back.regions.len(), d.regions.len());
    for (a, b) in back.regions.iter().zip(&d.regions) {
        assert_eq!(
            (a.material, a.object_id, a.kind),
            (b.material, b.object_id, b.kind)
        );
        assert_eq!(a.polygon.len(), b.polygon.len());
        assert!(a.polygon.iter().zip(&b.polygon).all(|(p, q)| close(*p, *q)));
    }
    assert_eq!(back.texts, d.texts);
    // Old files without the new fields still load.
    let old = r#"{"lines":[],"bounds":[{"x":0.0,"y":0.0},{"x":0.0,"y":0.0}]}"#;
    let d: Drawing = serde_json::from_str(old).expect("old format");
    assert!(d.regions.is_empty() && d.texts.is_empty());
}

#[test]
#[ignore = "timing; run with --release -- --ignored --nocapture"]
fn perf_house_elevation_2048_with_hatch() {
    let mut p = Project::new("house");
    let (w, h) = (ft(40.0), ft(30.0));
    let c = [
        Point::new(0.0, 0.0),
        Point::new(w, 0.0),
        Point::new(w, h),
        Point::new(0.0, h),
    ];
    let mut ids = Vec::new();
    for i in 0..4 {
        ids.push(p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, WALL_H, WallKind::Exterior));
    }
    for k in 0..4 {
        p.add_opening(0, ids[0], ft(5.0 + 8.0 * k as f64), OpeningKind::Window)
            .expect("window fits");
    }
    p.add_opening(0, ids[0], ft(36.0), OpeningKind::Door)
        .expect("door fits");
    let mut scene = build_scene(&p);
    // Dress the walls in brick and add a shingled roof slab so every face hatches.
    for m in &mut scene.meshes {
        if m.material == Material::WallExterior {
            m.material = Material::Brick;
        }
    }
    scene.meshes.push(cuboid(
        (-24.0, 504.0),
        (WALL_H as f32, 190.0),
        (-384.0, 24.0),
        Material::Roof,
        99,
    ));
    let o = Options {
        raster_px: 2048,
        hatch: true,
        ..Options::default()
    };
    let t = std::time::Instant::now();
    let d = elevation(&scene, ViewDir::Front, &o);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let plain = std::time::Instant::now();
    let _ = elevation(
        &scene,
        ViewDir::Front,
        &Options {
            regions: false,
            ..Options::default()
        },
    );
    let plain_ms = plain.elapsed().as_secs_f64() * 1000.0;
    println!(
        "PERF 40x30 house front @2048 hatch on: {ms:.0} ms ({} lines, {} regions); lines only: {plain_ms:.0} ms",
        d.lines.len(),
        d.regions.len()
    );
}
