//! Behavioural tests for elevations, sections and the overhead view.

use plan_3d::build_scene;
use plan_core::{OpeningKind, Point, Project, WallKind};
use plan_elevation::{
    elevation, elevation_from_project, plan_overhead, section, Drawing, EdgeKind, Line2,
    LineWeight, Options, SectionCut, ViewDir,
};

const WALL_H: f64 = 109.125;
const TOL: f64 = 1.5;

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

fn is_vertical(l: &Line2) -> bool {
    (l.a.x - l.b.x).abs() < 1e-6 && l.length() > 1.0
}

fn is_horizontal(l: &Line2) -> bool {
    (l.a.y - l.b.y).abs() < 1e-6 && l.length() > 1.0
}

fn distinct_vertical_xs(d: &Drawing) -> Vec<f64> {
    let mut xs: Vec<f64> = d
        .lines
        .iter()
        .filter(|l| is_vertical(l))
        .map(|l| l.a.x)
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    xs
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < TOL
}

#[test]
fn single_wall_front_bounds_and_heavy_outline() {
    let d = elevation_from_project(&wall_project(false), ViewDir::Front, &opts());
    let (w, h) = d.size();
    assert!(near(w, 120.0) && near(h, WALL_H), "size {w} x {h}");

    let heavy: Vec<&Line2> = d
        .lines
        .iter()
        .filter(|l| l.weight == LineWeight::Heavy)
        .collect();
    assert!(
        heavy
            .iter()
            .any(|l| is_horizontal(l) && near(l.a.y, WALL_H) && l.length() > 110.0),
        "missing heavy top edge"
    );
    for x in [0.0, 120.0] {
        assert!(
            heavy
                .iter()
                .any(|l| is_vertical(l) && near(l.a.x, x) && l.length() > 100.0),
            "missing heavy vertical end at x={x}"
        );
    }
}

#[test]
fn door_adds_lines_around_the_opening() {
    let d = elevation_from_project(&wall_project(true), ViewDir::Front, &opts());
    let xs = distinct_vertical_xs(&d);
    assert!(xs.len() >= 4, "vertical lines at {xs:?}");
    // Jambs of the 36" door centred at 60".
    for x in [42.0, 78.0] {
        assert!(xs.iter().any(|&v| near(v, x)), "no jamb near {x}: {xs:?}");
    }
    // Door head at 80".
    assert!(d
        .lines
        .iter()
        .any(|l| is_horizontal(l) && near(l.a.y, 80.0) && l.length() > 30.0));
}

#[test]
fn room_front_shows_only_the_near_wall_outline() {
    let d = elevation_from_project(&room_project(), ViewDir::Front, &opts());
    let (w, h) = d.size();
    // Near wall 240" plus the side walls' end faces poking out 3.25" each side.
    assert!(near(w, 246.5), "width {w}");
    assert!(h < WALL_H + 3.0, "height {h}");
    // Bottom and top outline, the 1" ceiling-slab strip above the wall top, and
    // four verticals: both outer ends and the two near-wall ends.
    let expected = 2.0 * 246.5 + 240.0 + 4.0 * WALL_H;
    let total = d.total_length();
    assert!(
        (total - expected).abs() < 0.1 * expected,
        "total {total}, expected about {expected}"
    );
    // Nothing from the far wall's interior: no vertical line inside the near wall's span
    // except its two ends.
    let inner: Vec<f64> = distinct_vertical_xs(&d)
        .into_iter()
        .filter(|x| *x > 5.0 && *x < 235.0)
        .collect();
    assert!(inner.is_empty(), "stray interior verticals at {inner:?}");
}

#[test]
fn hidden_dashed_option_adds_hidden_lines_for_the_room() {
    let scene = build_scene(&room_project());
    let plain = elevation(&scene, ViewDir::Front, &opts());
    let dashed = elevation(
        &scene,
        ViewDir::Front,
        &Options {
            include_hidden_dashed: true,
            ..opts()
        },
    );
    assert!(plain.lines.iter().all(|l| l.kind != EdgeKind::Hidden));
    assert!(dashed.lines.iter().any(|l| l.kind == EdgeKind::Hidden));
}

#[test]
fn section_through_the_room_cuts_both_side_walls() {
    let mut project = room_project();
    let north = project.floors[0].walls[2].id;
    project
        .add_opening(0, north, ft(10.0), OpeningKind::Door)
        .expect("door fits");
    let scene = build_scene(&project);
    // Plan y = 60" is scene z = -60".
    let cut = SectionCut {
        plane_normal: ViewDir::Front,
        offset: -60.0,
    };
    let d = section(&scene, cut, &opts());
    let cut_lines: Vec<&Line2> = d.lines.iter().filter(|l| l.kind == EdgeKind::Cut).collect();
    assert!(cut_lines.iter().all(|l| l.weight == LineWeight::Heavy));
    assert!(cut_lines.len() >= 2);
    // Both side walls show full-height cut faces: left wall x in [-3.25, 3.25], right in [236.75, 243.25].
    for x in [-3.25, 3.25, 236.75, 243.25] {
        assert!(
            cut_lines
                .iter()
                .any(|l| is_vertical(l) && near(l.a.x, x) && l.length() > 100.0),
            "no cut line at x={x}"
        );
    }
    // The far wall's door is visible beyond the cut: jambs at 102" and 138", not cut lines.
    for x in [102.0, 138.0] {
        assert!(
            d.lines
                .iter()
                .any(|l| l.kind != EdgeKind::Cut && is_vertical(l) && near(l.a.x, x)),
            "no door jamb beyond the cut near x={x}"
        );
    }
}

#[test]
fn plan_overhead_is_the_plan_outline() {
    let scene = build_scene(&room_project());
    let d = plan_overhead(&scene, &opts());
    let (w, h) = d.size();
    assert!(near(w, 246.5) && near(h, 126.5), "size {w} x {h}");
    assert!(d.lines.iter().any(|l| l.weight == LineWeight::Heavy));
}

#[test]
fn back_view_mirrors_front() {
    let scene = build_scene(&room_project());
    let f = elevation(&scene, ViewDir::Front, &opts());
    let b = elevation(&scene, ViewDir::Back, &opts());
    assert!(near(f.size().0, b.size().0) && near(f.size().1, b.size().1));
    assert!(b.bounds.0.x < 0.0 && f.bounds.0.x >= -3.5);
}

#[test]
fn side_views_have_the_depth_of_the_plan() {
    let scene = build_scene(&room_project());
    for dir in [ViewDir::Left, ViewDir::Right] {
        let d = elevation(&scene, dir, &opts());
        assert!(near(d.size().0, 126.5), "{dir:?} width {}", d.size().0);
    }
}

#[test]
fn empty_scene_gives_empty_drawing() {
    let d = elevation(&plan_3d::Scene::default(), ViewDir::Front, &opts());
    assert!(d.lines.is_empty());
    assert_eq!(d.bounds, (Point::ZERO, Point::ZERO));
}

#[test]
fn drawing_exports_to_cad_and_svg() {
    let d = elevation_from_project(&wall_project(true), ViewDir::Front, &opts());
    let cad = d.to_cad("Front");
    assert_eq!(cad.len(), d.lines.len());
    assert!(cad.iter().all(|o| o.layer.starts_with("Front, ")));
    let svg = d.svg();
    assert!(svg.starts_with("<svg"));
    assert_eq!(svg.matches("<line").count(), d.lines.len());
}
