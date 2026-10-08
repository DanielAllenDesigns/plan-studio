//! Free-angle sections and elevations, automatic dimensions, material labels,
//! per-object pen weights and DXF output.

use plan_3d::{build_scene, Material, Mesh, Scene, Vertex};
use plan_core::floors::FLOOR_PLATFORM_THICKNESS;
use plan_core::model::DEFAULT_CEILING_HEIGHT;
use plan_core::{OpeningKind, Point, Project, WallKind};
use plan_elevation::{
    annotate_view, elevation_free, section_free, AnnotateOptions, DimKind, DimOptions, EdgeKind,
    FreeView, LineWeight, ObjectWeights, Options, RegionKind,
};

const WALL_H: f64 = DEFAULT_CEILING_HEIGHT;

fn opts() -> Options {
    Options {
        raster_px: 512,
        ..Options::default()
    }
}

/// A wall of `len` at `deg` from the origin; the view that faces its left face
/// from just in front of it, as the Wall Elevation tool builds it.
fn skewed_wall(deg: f64, len: f64) -> (Project, FreeView, f64, u64) {
    let mut p = Project::new("skew");
    let a = Point::ZERO;
    let u = Point::new(deg.to_radians().cos(), deg.to_radians().sin());
    let id = p.add_wall(0, a, a + u * len, 4.5, WALL_H, WallKind::Exterior);
    let n = u.perp();
    let origin = a + u * (len * 0.5) + n * (2.25 + 0.5);
    let view = FreeView::new(origin, (n * -1.0).angle().to_degrees()).with_half_width(len * 0.5);
    (p, view, 4.5 + 0.5 + 2.0, id)
}

#[test]
fn a_30_degree_wall_elevation_matches_the_wall_face() {
    let (p, view, depth, wall) = skewed_wall(30.0, 144.0);
    let scene = build_scene(&p);
    let d = section_free(
        &scene,
        &view,
        &Options {
            section_depth: Some(depth),
            ..opts()
        },
    );
    let faces: Vec<_> = d
        .regions
        .iter()
        .filter(|r| r.kind == RegionKind::Face && r.object_id == Some(wall))
        .collect();
    assert!(!faces.is_empty(), "the wall face shows");
    let (mut lo, mut hi) = (Point::new(1e9, 1e9), Point::new(-1e9, -1e9));
    for r in &faces {
        for q in &r.polygon {
            lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
            hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
        }
    }
    let tol = 1.5;
    assert!(
        (lo.x + 72.0).abs() < tol && (hi.x - 72.0).abs() < tol,
        "{lo:?} {hi:?}"
    );
    assert!(lo.y.abs() < tol && (hi.y - WALL_H).abs() < tol);
    // The drawing stays inside the camera line.
    assert!(d.bounds.0.x >= -72.0 - 1e-6 && d.bounds.1.x <= 72.0 + 1e-6);
}

#[test]
fn the_same_wall_square_to_an_axis_gives_the_same_face() {
    let (p0, v0, depth, wall) = skewed_wall(0.0, 144.0);
    let (p30, v30, _, wall30) = skewed_wall(30.0, 144.0);
    let o = Options {
        section_depth: Some(depth),
        ..opts()
    };
    let size = |p: &Project, v: &FreeView, id: u64| {
        let d = section_free(&build_scene(p), v, &o);
        let r = d
            .regions
            .iter()
            .filter(|r| r.object_id == Some(id) && r.kind == RegionKind::Face)
            .map(|r| r.area())
            .sum::<f64>();
        r
    };
    let (a0, a30) = (size(&p0, &v0, wall), size(&p30, &v30, wall30));
    assert!((a0 - a30).abs() / a0 < 0.02, "{a0} vs {a30}");
    assert!(v0.axis().is_some() && v30.axis().is_none());
}

#[test]
fn the_back_clip_and_the_cut_plane_follow_the_camera_line() {
    // A second wall 60" behind the first (parallel, at 30 degrees) shows only without a back clip.
    let (mut p, view, depth, _) = skewed_wall(30.0, 144.0);
    let u = Point::new(30f64.to_radians().cos(), 30f64.to_radians().sin());
    let n = u.perp();
    let far = p.add_wall(
        0,
        n * -60.0,
        n * -60.0 + u * 144.0,
        4.5,
        WALL_H,
        WallKind::Exterior,
    );
    let scene = build_scene(&p);
    let sees_far = |d: &plan_elevation::Drawing| d.regions.iter().any(|r| r.object_id == Some(far));
    let clipped = section_free(
        &scene,
        &view,
        &Options {
            section_depth: Some(depth),
            ..opts()
        },
    );
    assert!(!sees_far(&clipped));
    // Unclipped the front wall hides the far one (same extent), so move the
    // camera line past the front wall: the cut removes it and the far wall shows.
    let past = FreeView::new(view.origin + view.dir() * 12.0, view.view_deg).with_half_width(72.0);
    let beyond = section_free(&scene, &past, &opts());
    assert!(sees_far(&beyond));
    assert!(
        beyond.cut_regions().count() == 0,
        "the line misses both walls"
    );
}

fn two_storey() -> Project {
    let mut p = Project::new("two");
    let c = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 160.0),
        Point::new(0.0, 160.0),
    ];
    let mut front = 0;
    for i in 0..4 {
        let id = p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, WALL_H, WallKind::Exterior);
        if i == 0 {
            front = id;
        }
    }
    p.add_opening(0, front, 60.0, OpeningKind::Window).unwrap();
    p.add_opening(0, front, 150.0, OpeningKind::Door).unwrap();
    let up = p.build_new_floor(true);
    assert_eq!(up, 1);
    p
}

fn south_view() -> FreeView {
    // Standing south of the house, looking north (plan +Y): the front elevation.
    FreeView::new(Point::new(120.0, -24.0), 90.0)
}

#[test]
fn auto_dimensions_give_the_floor_to_floor_string() {
    let p = two_storey();
    let scene = build_scene(&p);
    let view = south_view();
    let mut d = elevation_free(&scene, &view, &opts());
    annotate_view(
        &mut d,
        &scene,
        &p,
        &view,
        None,
        &AnnotateOptions {
            dimensions: Some(DimOptions::default()),
            ..AnnotateOptions::default()
        },
    );
    let structure = FLOOR_PLATFORM_THICKNESS;
    let f2f: Vec<_> = d
        .dims
        .iter()
        .filter(|x| x.kind == DimKind::FloorToFloor)
        .collect();
    assert_eq!(f2f.len(), 1);
    assert!((f2f[0].value() - (109.125 + structure)).abs() < 1e-9);
    assert_eq!(
        f2f[0].text,
        plan_core::units::fmt_ft_in_frac(109.125 + structure, 8)
    );
    // The top floor continues to its plate; the overall string spans both.
    assert!(d.dims.iter().any(|x| x.kind == DimKind::FloorToPlate));
    let overall = d.dims.iter().find(|x| x.kind == DimKind::Overall).unwrap();
    assert!((overall.value() - (109.125 + structure + WALL_H)).abs() < 1e-9);
    // Every dimension is also drawn: a line and its text.
    for x in &d.dims {
        assert!(d.texts.iter().any(|(_, t)| *t == x.text));
        assert!(d.lines.iter().any(|l| l.kind == EdgeKind::Annotation
            && (l.a.x - x.x).abs() < 1e-9
            && (l.b.x - x.x).abs() < 1e-9
            && (l.a.y - x.from).abs() < 1e-9
            && (l.b.y - x.to).abs() < 1e-9));
    }
}

#[test]
fn the_opening_string_has_the_sill_and_head_of_the_windows_in_view() {
    let p = two_storey();
    let scene = build_scene(&p);
    let view = south_view();
    let mut d = elevation_free(&scene, &view, &opts());
    annotate_view(
        &mut d,
        &scene,
        &p,
        &view,
        None,
        &AnnotateOptions {
            dimensions: Some(DimOptions::default()),
            ..AnnotateOptions::default()
        },
    );
    let win = p.floors[0]
        .openings
        .iter()
        .find(|o| o.kind == OpeningKind::Window)
        .unwrap();
    let levels: Vec<f64> = d
        .dims
        .iter()
        .filter(|x| x.kind == DimKind::Opening)
        .flat_map(|x| [x.from, x.to])
        .collect();
    for want in [win.sill_height, win.sill_height + win.height] {
        assert!(
            levels.iter().any(|l| (l - want).abs() < 0.3),
            "level {want} in {levels:?}"
        );
    }
    // Without the strings requested there are none, and the level callouts can be left out.
    let mut plain = elevation_free(&scene, &view, &opts());
    annotate_view(
        &mut plain,
        &scene,
        &p,
        &view,
        None,
        &AnnotateOptions {
            levels: false,
            ..AnnotateOptions::default()
        },
    );
    assert!(plain.dims.is_empty());
    assert!(!plain.texts.iter().any(|(_, t)| t.starts_with("T.O.")));
    assert!(plain.texts.iter().any(|(_, t)| t == "FRONT ELEVATION"));
}

#[test]
fn level_callouts_move_left_of_the_dimension_strings() {
    let p = two_storey();
    let scene = build_scene(&p);
    let view = south_view();
    let make = |dims: bool| {
        let mut d = elevation_free(&scene, &view, &opts());
        annotate_view(
            &mut d,
            &scene,
            &p,
            &view,
            None,
            &AnnotateOptions {
                dimensions: dims.then(DimOptions::default),
                ..AnnotateOptions::default()
            },
        );
        d
    };
    let (with, without) = (make(true), make(false));
    let callout_x = |d: &plan_elevation::Drawing| {
        d.texts
            .iter()
            .filter(|(_, t)| t.starts_with("T.O."))
            .map(|(p, _)| p.x)
            .fold(f64::INFINITY, f64::min)
    };
    let strings_x = with.dims.iter().map(|x| x.x).fold(f64::INFINITY, f64::min);
    assert!(callout_x(&with) < callout_x(&without));
    assert!(callout_x(&with) < strings_x);
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
    let faces: [[usize; 4]; 6] = [
        [4, 5, 6, 7],
        [1, 0, 3, 2],
        [5, 1, 2, 6],
        [0, 4, 7, 3],
        [7, 6, 2, 3],
        [0, 1, 5, 4],
    ];
    let mut indices = Vec::new();
    for q in faces {
        indices.extend([q[0], q[1], q[2], q[0], q[2], q[3]].map(|i| i as u32));
    }
    Mesh {
        vertices: pts
            .iter()
            .map(|&position| Vertex {
                position,
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            })
            .collect(),
        indices,
        material,
        object_id: Some(id),
    }
}

#[test]
fn material_labels_name_the_cladding_with_a_leader() {
    let scene = Scene {
        meshes: vec![
            cuboid(
                (0.0, 120.0),
                (0.0, 108.0),
                (-10.0, 0.0),
                Material::Siding,
                1,
            ),
            cuboid(
                (140.0, 260.0),
                (0.0, 108.0),
                (-10.0, 0.0),
                Material::Brick,
                2,
            ),
            cuboid(
                (0.0, 260.0),
                (108.0, 140.0),
                (-10.0, 0.0),
                Material::Roof,
                3,
            ),
            cuboid(
                (60.0, 90.0),
                (20.0, 60.0),
                (-1.0, 0.0),
                Material::WindowGlass,
                4,
            ),
        ],
    };
    let p = Project::new("labels");
    let view = FreeView::new(Point::new(130.0, -50.0), 90.0);
    let make = |materials: bool| {
        let mut d = elevation_free(&scene, &view, &opts());
        annotate_view(
            &mut d,
            &scene,
            &p,
            &view,
            None,
            &AnnotateOptions {
                materials,
                ..AnnotateOptions::default()
            },
        );
        d
    };
    let (on, off) = (make(true), make(false));
    for name in ["SIDING", "BRICK", "ROOFING"] {
        assert!(on.texts.iter().any(|(_, t)| t == name), "{name} labelled");
        assert!(!off.texts.iter().any(|(_, t)| t == name));
    }
    // Glass is not a cladding: no label for it.
    assert!(!on.texts.iter().any(|(_, t)| t.contains("GLASS")));
    // Three leaders (a line and a small cross each) were added, ending at the label column.
    assert_eq!(on.lines.len() - off.lines.len(), 9);
    let label_x = on
        .texts
        .iter()
        .find(|(_, t)| t == "BRICK")
        .map(|(p, _)| p.x)
        .unwrap();
    assert!(label_x > on.bounds.0.x + 200.0);
    // Each label sits at the height of the region it names (the siding wall is lower than the roof).
    let y = |name: &str| {
        on.texts
            .iter()
            .find(|(_, t)| t == name)
            .map(|(p, _)| p.y)
            .unwrap()
    };
    assert!(y("ROOFING") > y("SIDING"));
}

#[test]
fn object_weights_restyle_the_lines_by_layer_pen() {
    let p = two_storey();
    let scene = build_scene(&p);
    let view = south_view();
    let win = p.floors[0]
        .openings
        .iter()
        .find(|o| o.kind == OpeningKind::Window)
        .unwrap()
        .id;
    let mut w = ObjectWeights::new();
    w.set_pen(win, 13);
    for wall in p.floors.iter().flat_map(|f| f.walls.iter()) {
        w.set_pen(wall.id, 50);
    }
    let plain = plan_elevation::render_free(&scene, &view, false, &opts(), None);
    let styled = plan_elevation::render_free(&scene, &view, false, &opts(), Some(&w));
    let heavy = |d: &plan_elevation::Drawing| {
        d.lines
            .iter()
            .filter(|l| l.weight == LineWeight::Heavy)
            .map(|l| l.length())
            .sum::<f64>()
    };
    // The window's outlines went light, so less heavy line is drawn.
    assert!(
        heavy(&styled) < heavy(&plain),
        "{} vs {}",
        heavy(&styled),
        heavy(&plain)
    );
    assert!(styled.lines.iter().any(|l| l.weight == LineWeight::Light));
    assert!(w.class_of(win) == Some(LineWeight::Light));
}

#[test]
fn the_dxf_has_layers_by_weight() {
    let p = two_storey();
    let scene = build_scene(&p);
    let view = south_view();
    let mut d = elevation_free(
        &scene,
        &view,
        &Options {
            include_hidden_dashed: true,
            ..opts()
        },
    );
    annotate_view(&mut d, &scene, &p, &view, None, &AnnotateOptions::default());
    let dxf = d.to_dxf("Front").expect("a drawing");
    for name in ["Front, Heavy", "Front, Annotation", "Front, Text"] {
        assert!(dxf.contains(name), "layer {name}");
    }
    let layers = d.dxf_layers("Front");
    assert!(layers.contains(&"Front, Heavy".to_string()));
    // Hidden lines are dashed and have their own layer.
    assert!(layers.contains(&"Front, Hidden".to_string()));
    assert!(dxf.contains("LINE"));
    assert!(plan_elevation::Drawing::default().to_dxf("x").is_none());
}
