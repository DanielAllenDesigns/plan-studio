//! Wall layers join in 3D (brief 40, docs/parity/walls.md W-137): every layer
//! of a wall is its own slab built from the plan's layer outline, so the
//! layers of two walls meet edge to edge at an L corner and a T, and no slab
//! pokes past the outer corner.

use plan_3d::{build_scene_with_types, Mesh, SceneOptions};
use plan_core::{Id, PlanDefaults, Point, Project, WallKind};

type Plan = (f64, f64);

fn project(walls: &[(Plan, Plan)]) -> (Project, Vec<Id>, usize, f64) {
    let defaults = PlanDefaults::chief_x18_daniel();
    let ty = defaults.wall_type("Stucco-6").unwrap().clone();
    let mut p = Project::new("t");
    let mut ids = Vec::new();
    for (a, b) in walls {
        let id = p.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            ty.thickness(),
            96.0,
            WallKind::Exterior,
        );
        p.floors[0].wall_mut(id).unwrap().wall_type = Some(ty.name.clone());
        ids.push(id);
    }
    (p, ids, ty.layers.len(), ty.thickness())
}

fn scene(p: &Project) -> Vec<Mesh> {
    let defaults = PlanDefaults::chief_x18_daniel();
    build_scene_with_types(p, &SceneOptions::default(), &defaults.wall_types).meshes
}

/// Plan positions `(x, y)` of the vertices of `m`, with their heights.
fn plan_points(m: &Mesh) -> Vec<(f64, f64, f64)> {
    m.vertices
        .iter()
        .map(|v| {
            (
                v.position[0] as f64,
                -(v.position[2] as f64),
                v.position[1] as f64,
            )
        })
        .collect()
}

/// Whether two slabs share a vertical edge: two plan positions in common, at
/// two different heights each.
fn share_vertical_edge(a: &Mesh, b: &Mesh) -> bool {
    let (pa, pb) = (plan_points(a), plan_points(b));
    let mut shared: Vec<(f64, f64)> = Vec::new();
    for p in &pa {
        let hit = |q: &(f64, f64, f64)| (p.0 - q.0).abs() < 1e-3 && (p.1 - q.1).abs() < 1e-3;
        let tall = pa.iter().any(|q| hit(q) && (q.2 - p.2).abs() > 1.0);
        if tall && pb.iter().any(hit) && !shared.iter().any(|s| s.0 == p.0 && s.1 == p.1) {
            shared.push((p.0, p.1));
        }
    }
    !shared.is_empty()
}

fn slabs(meshes: &[Mesh], id: Id) -> Vec<&Mesh> {
    meshes.iter().filter(|m| m.object_id == Some(id)).collect()
}

#[test]
fn an_l_corner_builds_one_slab_per_layer_and_the_layers_share_the_corner_edge() {
    let (p, ids, layers, thick) =
        project(&[((0.0, 0.0), (120.0, 0.0)), ((120.0, 0.0), (120.0, 100.0))]);
    let meshes = scene(&p);
    let (a, b) = (slabs(&meshes, ids[0]), slabs(&meshes, ids[1]));
    assert_eq!(a.len(), layers, "one slab per layer of the first wall");
    assert_eq!(b.len(), layers, "one slab per layer of the second wall");
    // Every layer of the first wall meets some layer of the second along a
    // vertical edge at the corner.
    for (k, s) in a.iter().enumerate() {
        assert!(
            b.iter().any(|t| share_vertical_edge(s, t)),
            "layer {k} shares no corner edge"
        );
    }
    // Nothing protrudes past the outer corner of the wall pair.
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for m in a.iter().chain(b.iter()) {
        for (x, y, _) in plan_points(m) {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
    }
    let h = thick * 0.5;
    assert!(
        min_x >= -1e-3 && max_x <= 120.0 + h + 1e-3,
        "x {min_x}..{max_x}"
    );
    assert!(
        min_y >= -h - 1e-3 && max_y <= 100.0 + 1e-3,
        "y {min_y}..{max_y}"
    );
    // The outer corner is filled by the mitred layers.
    assert!(max_x > 120.0 + h - 1e-3, "outer corner is open: {max_x}");
    assert!(min_y < -h + 1e-3, "outer corner is open: {min_y}");
}

#[test]
fn the_layers_of_a_tee_stop_on_the_through_wall() {
    let (p, ids, layers, _) =
        project(&[((0.0, 0.0), (240.0, 0.0)), ((120.0, 0.0), (120.0, 100.0))]);
    let meshes = scene(&p);
    let (through, branch) = (slabs(&meshes, ids[0]), slabs(&meshes, ids[1]));
    assert_eq!(through.len(), layers);
    assert_eq!(branch.len(), layers);
    // The through wall is uncut: its layers span the whole length.
    let (lo, hi) = through
        .iter()
        .flat_map(|m| plan_points(m))
        .fold((f64::MAX, f64::MIN), |(lo, hi), (x, _, _)| {
            (lo.min(x), hi.max(x))
        });
    assert!(lo.abs() < 1e-3 && (hi - 240.0).abs() < 1e-3, "{lo}..{hi}");
    // No branch slab reaches into the through wall's far side: every branch
    // vertex lies on one side of the through wall's centerline or on its face.
    let side = branch
        .iter()
        .flat_map(|m| plan_points(m))
        .map(|(_, y, _)| y)
        .fold(f64::MAX, f64::min);
    assert!(side > -3.1, "branch pokes through: {side}");
}

#[test]
fn a_forty_five_degree_corner_has_no_slab_past_the_outer_corner() {
    let (p, ids, layers, _) = project(&[((0.0, 0.0), (100.0, 0.0)), ((100.0, 0.0), (170.7, 70.7))]);
    let meshes = scene(&p);
    let (a, b) = (slabs(&meshes, ids[0]), slabs(&meshes, ids[1]));
    assert_eq!((a.len(), b.len()), (layers, layers));
    for (k, s) in a.iter().enumerate() {
        assert!(
            b.iter().any(|t| share_vertical_edge(s, t)),
            "layer {k} shares no corner edge"
        );
    }
}
