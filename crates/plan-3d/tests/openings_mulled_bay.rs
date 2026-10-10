//! Round 16, brief 22: bow windows built from their Bay/Box and Bow
//! Specification (sections, depth, floor and ceiling, foundation), and windows
//! whose casings touch sharing one casing (DW-48, DW-158, DW-167).

use plan_3d::{build_scene, build_scene_with, Material, Mesh, Scene, SceneOptions};
use plan_core::openings::bay::{BayUnit, LoweredCeiling, RaisedFloor};
use plan_core::{Id, OpeningKind, OpeningStyle, Point, Project, WallKind};

fn wall_project() -> (Project, Id) {
    let mut p = Project::new("t");
    let wall = p.add_wall(
        0,
        Point::ZERO,
        Point::new(240.0, 0.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    (p, wall)
}

fn unit(style: OpeningStyle, edit: impl FnOnce(&mut BayUnit)) -> (Scene, Id) {
    let (mut p, wall) = wall_project();
    let id = p
        .add_opening(0, wall, 120.0, OpeningKind::Window)
        .expect("opening");
    let o = p.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
        .unwrap();
    o.style = style;
    o.width = 72.0;
    o.extras.spec.bay = BayUnit::for_style(style);
    edit(&mut o.extras.spec.bay);
    (build_scene(&p), id)
}

fn of(scene: &Scene, id: Id, m: Material) -> Vec<&Mesh> {
    scene
        .meshes
        .iter()
        .filter(|x| x.object_id == Some(id) && x.material == m)
        .collect()
}

fn tris(ms: &[&Mesh]) -> usize {
    ms.iter().map(|m| m.triangle_count()).sum()
}

fn extent(ms: &[&Mesh], axis: usize) -> (f32, f32) {
    ms.iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[axis]))
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
}

#[test]
fn a_bow_window_has_a_pane_per_section() {
    // The glass is proportional to the number of sections: every component
    // window has the same single-pane grid.
    let glass = |n: u32| {
        let (scene, id) = unit(OpeningStyle::BowWindow, |b| b.segments = n);
        tris(&of(&scene, id, Material::WindowGlass))
    };
    let (g3, g5, g7) = (glass(3), glass(5), glass(7));
    assert!(g3 > 0);
    assert_eq!(g5 * 3, g3 * 5, "{g3} {g5}");
    assert_eq!(g7 * 3, g3 * 7, "{g3} {g7}");
}

#[test]
fn the_unit_projects_the_depth_that_was_set() {
    for depth in [12.0_f64, 20.0, 30.0] {
        let (scene, id) = unit(OpeningStyle::BoxWindow, |b| b.depth = depth);
        let panels: Vec<&Mesh> = of(&scene, id, Material::WindowFrame);
        // Scene z is the plan y negated; the wall's face is at 3 in.
        let (lo, hi) = extent(&panels, 2);
        let reach = lo.abs().max(hi.abs()) as f64 - 3.0;
        assert!(
            (reach - depth).abs() < 0.6,
            "depth {depth} projects {reach}"
        );
    }
}

#[test]
fn a_raised_floor_lifts_the_seat_and_a_lowered_ceiling_the_roof_comes_down() {
    let (scene, id) = unit(OpeningStyle::BayWindow, |_| {});
    let roof_top = extent(&of(&scene, id, Material::Roof), 1).1;
    let (scene2, id2) = unit(OpeningStyle::BayWindow, |b| {
        b.lowered_ceiling = Some(LoweredCeiling {
            height: 12.0,
            ..LoweredCeiling::default()
        });
    });
    let lowered = extent(&of(&scene2, id2, Material::Roof), 1).1;
    assert!(
        (roof_top - lowered - 12.0).abs() < 0.3,
        "{roof_top} -> {lowered}"
    );
    // The floor of the unit sits at the raised height.
    let (scene3, id3) = unit(OpeningStyle::BayWindow, |b| {
        b.raised_floor = Some(RaisedFloor {
            height: 18.0,
            ..RaisedFloor::default()
        });
    });
    let floor_top = extent(&of(&scene3, id3, Material::Floor), 1).1;
    assert!((floor_top - 18.0).abs() < 0.3, "{floor_top}");
    let normal = extent(&of(&scene, id, Material::Floor), 1).1;
    assert!(normal.abs() < 0.3, "{normal}");
}

#[test]
fn a_foundation_is_built_under_a_unit_on_the_first_floor_unless_it_is_raised() {
    let (scene, id) = unit(OpeningStyle::BayWindow, |_| {});
    let concrete = of(&scene, id, Material::Concrete);
    assert!(!concrete.is_empty());
    assert!(extent(&concrete, 1).0 < -20.0, "it goes below the floor");
    let (scene, id) = unit(OpeningStyle::BayWindow, |b| {
        b.raised_floor = Some(RaisedFloor::default());
    });
    assert!(of(&scene, id, Material::Concrete).is_empty());
    let (scene, id) = unit(OpeningStyle::BayWindow, |b| b.foundation = false);
    assert!(of(&scene, id, Material::Concrete).is_empty());
}

#[test]
fn use_existing_roof_leaves_the_unit_without_a_roof_of_its_own() {
    let (scene, id) = unit(OpeningStyle::BayWindow, |b| b.roof.use_existing = true);
    assert!(of(&scene, id, Material::Roof).is_empty());
    let (scene, id) = unit(OpeningStyle::BayWindow, |_| {});
    assert!(!of(&scene, id, Material::Roof).is_empty());
}

#[test]
fn a_rectangular_roof_covers_the_bounding_rectangle() {
    let reach = |rect: bool| {
        let (scene, id) = unit(OpeningStyle::BayWindow, |b| b.roof.rectangular = rect);
        let roof = of(&scene, id, Material::Roof);
        let (lo, hi) = extent(&roof, 0);
        (hi - lo) as f64
    };
    // A bay's profile narrows toward the front, so a hip following it never
    // reaches the corners of the rectangle at the front.
    assert!(reach(true) >= reach(false) - 1e-3);
}

fn windows(gap: f64) -> (Scene, Id, Id) {
    let (mut p, wall) = wall_project();
    let a = p
        .add_opening(0, wall, 60.0, OpeningKind::Window)
        .expect("a");
    let b = p
        .add_opening(0, wall, 60.0 + 36.0 + gap, OpeningKind::Window)
        .expect("b");
    for id in [a, b] {
        let o = p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap();
        o.width = 36.0;
        o.height = 48.0;
        o.sill_height = 30.0;
    }
    let casing = SceneOptions {
        show_casing: true,
        ..SceneOptions::default()
    };
    (build_scene_with(&p, &casing), a, b)
}

#[test]
fn windows_whose_casings_touch_share_one_casing_filling_the_gap() {
    // 2 in apart: the casings (3 3/4 in each) touch; a casing as wide as the
    // gap stands between the windows, from the first one's jamb to the
    // second's.
    let (scene, a, _) = windows(2.0);
    let at = |scene: &Scene, x: f64| {
        of(scene, a, Material::Trim)
            .iter()
            .flat_map(|m| m.vertices.iter())
            .any(|v| ((v.position[0] as f64) - x).abs() < 1e-3)
    };
    let jamb_edge = 60.0 + 18.0;
    assert!(
        at(&scene, jamb_edge + 2.0),
        "no shared casing between the windows"
    );
    // 20 in apart they are separate openings with a leg of casing each: the
    // first one's trim does not reach across the gap.
    let (scene, _, _) = windows(20.0);
    assert!(!at(&scene, jamb_edge + 20.0));
}

#[test]
fn a_mulled_pair_draws_the_head_casing_once_across_both() {
    let (scene, a, b) = windows(2.0);
    let head_width = |id: Id| {
        let trim = of(&scene, id, Material::Trim);
        let (lo, hi) = extent(&trim, 0);
        (hi - lo) as f64
    };
    // The first window owns the unit's head and apron: it spans both.
    assert!(head_width(a) > 36.0 * 2.0, "{}", head_width(a));
    // The second window draws no head, only its own end leg and jambs.
    assert!(head_width(b) < head_width(a) - 30.0);
}
