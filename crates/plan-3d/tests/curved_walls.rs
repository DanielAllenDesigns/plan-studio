//! Curved walls in 3D: openings cut through the arc, door and window units
//! standing square to the local tangent, the wall classes along the arc,
//! mitred joins to straight and curved neighbours, and the roof cutting a
//! curved wall facet by facet (docs/parity/walls.md, W-64..W-68).

use plan_3d::wall_kinds::{build_class_cut, EndCuts};
use plan_3d::{build_scene, build_scene_with_types, Material, Mesh, Scene, SceneOptions};
use plan_core::defaults::RoofWallKind;
use plan_core::geometry::{point_in_polygon, polygon_area};
use plan_core::joins::{curved_end_miters, curved_wall_polygon, wall_outlines};
use plan_core::{
    Id, OpeningKind, PlanDefaults, Point, Project, Wall, WallClass, WallCurve, WallKind,
};
use plan_roof::{build_roof, EdgeKind, EdgeRoof, Roof, RoofPlane};
use serde_json::{json, Value};

const RADIUS: f64 = 120.0; // a 10' radius
const THICK: f64 = 7.625;
const HEIGHT: f64 = 109.125;

/// One exterior wall on a 10' radius arc over a 200" chord, of `class`.
fn arc_project(class: WallClass) -> (Project, Id) {
    let mut p = Project::new("t");
    let id = p.add_wall(
        0,
        Point::ZERO,
        Point::new(200.0, 0.0),
        THICK,
        HEIGHT,
        WallKind::Exterior,
    );
    let w = p.floors[0].wall_mut(id).unwrap();
    w.set_class(class);
    w.curve = WallCurve::from_radius(200.0, RADIUS, true);
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

fn wall_of(p: &Project, id: Id) -> Wall {
    p.floors[0].wall(id).unwrap().clone()
}

type V = [f64; 3];

fn tri_points(m: &Mesh, t: &[u32]) -> [V; 3] {
    let p = |i: u32| m.vertices[i as usize].position.map(f64::from);
    [p(t[0]), p(t[1]), p(t[2])]
}

/// Number of times the line from `origin` along `dir` crosses the triangles
/// of `meshes` (a closed wall shell is crossed twice, a hole not at all).
fn crossings(meshes: &[&Mesh], origin: V, dir: V) -> usize {
    let sub = |a: V, b: V| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: V, b: V| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: V, b: V| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut hits = 0;
    for m in meshes {
        for t in m.indices.chunks(3) {
            let [a, b, c] = tri_points(m, t);
            let (e1, e2) = (sub(b, a), sub(c, a));
            let pv = cross(dir, e2);
            let det = dot(e1, pv);
            if det.abs() < 1e-12 {
                continue;
            }
            let tv = sub(origin, a);
            let u = dot(tv, pv) / det;
            let qv = cross(tv, e1);
            let v = dot(dir, qv) / det;
            let s = dot(e2, qv) / det;
            if u >= 0.0 && v >= 0.0 && u + v <= 1.0 && s > 0.0 {
                hits += 1;
            }
        }
    }
    hits
}

/// A horizontal probe across the wall at arc length `s` and height `h`:
/// starts 40" to the right of the wall and looks left through it.
fn probe(scene: &Scene, wall: &Wall, id: Id, s: f64, h: f64) -> usize {
    let (at, tangent) = wall.frame_at(s);
    let normal = tangent.perp();
    let from = at.sub(normal.scale(40.0));
    let origin = [from.x, h, -from.y];
    let dir = [normal.x, 0.0, -normal.y];
    crossings(&of(scene, id), origin, dir)
}

#[test]
fn a_door_on_a_ten_foot_radius_wall_leaves_a_hole() {
    let (mut p, id) = arc_project(WallClass::Standard);
    let plain = wall_of(&p, id);
    let solid = scene(&p);
    let door = p.add_opening(0, id, 118.0, OpeningKind::Door).unwrap();
    let centre = p.floors[0]
        .openings
        .iter()
        .find(|o| o.id == door)
        .unwrap()
        .center_offset;
    let cut = scene(&p);
    let w = wall_of(&p, id);
    assert!((w.curve.unwrap().bulge - plain.curve.unwrap().bulge).abs() < 1e-9);
    // The wall is a closed shell: a probe through it crosses two faces.
    for s in [centre, centre - 14.0, centre + 14.0, centre + 60.0] {
        assert_eq!(probe(&solid, &w, id, s, 40.0), 2, "solid wall at {s}");
    }
    // The door is 36" wide and 80" tall: nothing in the way inside it ...
    for ds in [0.0, -14.0, 14.0] {
        assert_eq!(probe(&cut, &w, id, centre + ds, 40.0), 0, "hole at {ds}");
    }
    // ... jambs either side, the head above and the wall beyond are there.
    for ds in [-22.0, 22.0, 60.0] {
        assert_eq!(probe(&cut, &w, id, centre + ds, 40.0), 2, "jamb at {ds}");
    }
    assert_eq!(probe(&cut, &w, id, centre, 95.0), 2, "head above the door");
    // The reveals line the hole: a jamb face at each side, a head under it.
    let tris: usize = of(&cut, id).iter().map(|m| m.indices.len() / 3).sum();
    let plain_tris: usize = of(&solid, id).iter().map(|m| m.indices.len() / 3).sum();
    assert!(tris > plain_tris, "the hole adds faces");
}

#[test]
fn a_window_on_a_curved_wall_is_cut_between_sill_and_head() {
    let (mut p, id) = arc_project(WallClass::Standard);
    let win = p.add_opening(0, id, 100.0, OpeningKind::Window).unwrap();
    let o = p.floors[0]
        .openings
        .iter()
        .find(|o| o.id == win)
        .unwrap()
        .clone();
    let s = scene(&p);
    let w = wall_of(&p, id);
    let mid = o.sill_height + o.height * 0.5;
    assert_eq!(probe(&s, &w, id, o.center_offset, mid), 0);
    assert_eq!(probe(&s, &w, id, o.center_offset, o.sill_height - 4.0), 2);
    assert_eq!(
        probe(&s, &w, id, o.center_offset, o.sill_height + o.height + 4.0),
        2
    );
}

/// Principal horizontal direction of a mesh's vertices (unit, scene x/z).
fn long_axis(meshes: &[&Mesh]) -> (f64, f64) {
    let pts: Vec<(f64, f64)> = meshes
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| (v.position[0], v.position[2])))
        .map(|(x, z)| (f64::from(x), f64::from(z)))
        .collect();
    let n = pts.len() as f64;
    let (mx, mz) = (
        pts.iter().map(|p| p.0).sum::<f64>() / n,
        pts.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let (mut sxx, mut sxz, mut szz) = (0.0, 0.0, 0.0);
    for (x, z) in &pts {
        let (dx, dz) = (x - mx, z - mz);
        sxx += dx * dx;
        sxz += dx * dz;
        szz += dz * dz;
    }
    let theta = 0.5 * (2.0 * sxz).atan2(sxx - szz);
    (theta.cos(), theta.sin())
}

#[test]
fn the_door_leaf_and_window_unit_stand_square_to_the_tangent() {
    for (kind, material) in [
        (OpeningKind::Door, Material::DoorPanel),
        (OpeningKind::Window, Material::WindowGlass),
    ] {
        let (mut p, id) = arc_project(WallClass::Standard);
        let op = p.add_opening(0, id, 60.0, kind).unwrap();
        let centre = p.floors[0].openings[0].center_offset;
        let s = scene(&p);
        let w = wall_of(&p, id);
        let leaf: Vec<&Mesh> = s
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(op) && m.material == material)
            .collect();
        assert!(!leaf.is_empty(), "{kind:?} has a {material:?} mesh");
        // The tangent at the middle of the opening, in scene x/z.
        let (centre_pt, tangent) = w.frame_at(centre);
        let axis = long_axis(&leaf);
        let along = (axis.0 * tangent.x + axis.1 * -tangent.y).abs();
        assert!(along > 0.999, "{kind:?} is {along} along the tangent");
        // And it is not along the chord (the wall bows away from it).
        let chord = w.direction();
        assert!((axis.0 * chord.x + axis.1 * -chord.y).abs() < 0.98);
        // The unit sits inside the wall's thickness all the way across.
        let (c, r) = w.arc_center_radius().unwrap();
        for m in &leaf {
            for v in &m.vertices {
                let q = Point::new(f64::from(v.position[0]), -f64::from(v.position[2]));
                let d = q.dist(c);
                assert!(
                    d > r - THICK * 0.5 - 0.01 && d < r + THICK * 0.5 + 0.01,
                    "{kind:?} vertex {d} from the center, wall spans {}..{}",
                    r - THICK * 0.5,
                    r + THICK * 0.5
                );
            }
        }
        // It stands where the opening is: its middle is at the arc point.
        let mid = leaf
            .iter()
            .flat_map(|m| m.vertices.iter())
            .fold((0.0, 0.0, 0.0), |a, v| {
                (
                    a.0 + f64::from(v.position[0]),
                    a.1 + f64::from(v.position[2]),
                    a.2 + 1.0,
                )
            });
        let mid = Point::new(mid.0 / mid.2, -mid.1 / mid.2);
        assert!(mid.dist(centre_pt) < 3.0, "{kind:?} is {mid:?}");
    }
}

fn pony() -> WallClass {
    WallClass::Pony {
        upper_type: "Stucco-6".into(),
        lower_type: "Foundation-8".into(),
        split_height: 36.0,
        upper_sets_plan_display: false,
    }
}

/// Heights of the horizontal caps (normal up or down), deduplicated.
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

#[test]
fn a_curved_pony_wall_is_two_boxes_along_the_arc() {
    let (p, id) = arc_project(pony());
    let s = scene(&p);
    let (tops, bottoms) = (cap_heights(&s, id, true), cap_heights(&s, id, false));
    assert_eq!(bottoms.len(), 2, "{bottoms:?}");
    assert!(bottoms[0].abs() < 1e-3 && (bottoms[1] - 36.0).abs() < 1e-3);
    assert_eq!(tops.len(), 2, "{tops:?}");
    assert!((tops[0] - 36.0).abs() < 1e-3 && (tops[1] - 109.125).abs() < 1e-3);
    let mats: Vec<Material> = of(&s, id).iter().map(|m| m.material).collect();
    assert!(mats.contains(&Material::Concrete) && mats.contains(&Material::Stucco));
    // Both boxes follow the arc: every vertex lies at one of the face radii
    // (lower 8" thick, upper Stucco-6), not on a straight chord.
    let w = wall_of(&p, id);
    let (c, r) = w.arc_center_radius().unwrap();
    let turn = w.curve.unwrap().sweep(w.start, w.end).signum();
    let lower = 8.0 * 0.5;
    let upper = 7.625 * 0.5;
    for m in of(&s, id) {
        for v in &m.vertices {
            let q = Point::new(f64::from(v.position[0]), -f64::from(v.position[2]));
            let d = q.dist(c);
            // Only the mid-station vertices are on the faces' radii; the
            // miter-free ends are too, so every vertex must be.
            let ok = [lower, -lower, upper, -upper]
                .iter()
                .any(|h| (d - (r - turn * h)).abs() < 0.02);
            assert!(ok, "vertex {d} from the center is on no face of radius {r}");
        }
    }
}

#[test]
fn curved_half_foundation_and_glass_walls_follow_the_arc() {
    // Half wall: capped at its height; foundation: below the floor.
    let (p, id) = arc_project(WallClass::HalfWall { height: 36.0 });
    let s = scene(&p);
    assert!((s.bounds().unwrap().1[1] - 36.0).abs() < 1e-3);
    assert!(s.bounds().unwrap().0[2] < -30.0, "bows to +y plan");
    assert!(!of(&s, id).is_empty());

    let (mut p, id) = arc_project(WallClass::Foundation);
    p.floors[0].wall_mut(id).unwrap().foundation_height = 40.0;
    let s = scene(&p);
    let (lo, hi) = s.bounds().unwrap();
    assert!((lo[1] + 40.0).abs() < 1e-3 && hi[1].abs() < 1e-3);
    assert!(of(&s, id).iter().all(|m| m.material == Material::Concrete));

    // Glass is built a facet at a time; it spans the arc, not the chord.
    let (p, id) = arc_project(WallClass::Glass);
    let s = scene(&p);
    let mats: Vec<Material> = of(&s, id).iter().map(|m| m.material).collect();
    assert!(mats.contains(&Material::Glass) && mats.contains(&Material::Trim));
    assert!(s.bounds().unwrap().0[2] < -30.0);
    // A door in a curved glass wall still leaves a hole in the glass.
    let (mut p, id) = arc_project(WallClass::Glass);
    let plain = scene(&p).triangle_count();
    p.add_opening(0, id, 118.0, OpeningKind::Window).unwrap();
    let w = wall_of(&p, id);
    let centre = p.floors[0].openings[0].center_offset;
    let cut = scene(&p);
    assert_eq!(probe(&cut, &w, id, centre, 50.0), 0);
    assert!(cut.triangle_count() > plain);
}

/// The wall's top faces (normal up) as 2-D triangles in plan coordinates.
fn top_triangles(meshes: &[&Mesh]) -> Vec<[Point; 3]> {
    let mut out = Vec::new();
    for m in meshes {
        for t in m.indices.chunks(3) {
            if t.iter().all(|i| m.vertices[*i as usize].normal[1] > 0.99) {
                let [a, b, c] = tri_points(m, t);
                // Only the top at the highest level.
                if (a[1] - b[1]).abs() < 1e-4 && (a[1] - c[1]).abs() < 1e-4 && a[1] > 100.0 {
                    out.push([a, b, c].map(|v| Point::new(v[0], -v[2])));
                }
            }
        }
    }
    out
}

fn in_triangle(p: Point, t: &[Point; 3]) -> bool {
    let d = |a: Point, b: Point| b.sub(a).cross(p.sub(a));
    let (d1, d2, d3) = (d(t[0], t[1]), d(t[1], t[2]), d(t[2], t[0]));
    let neg = d1 < -1e-9 || d2 < -1e-9 || d3 < -1e-9;
    let pos = d1 > 1e-9 || d2 > 1e-9 || d3 > 1e-9;
    !(neg && pos)
}

fn tri_area(t: &[Point; 3]) -> f64 {
    (t[1].sub(t[0]).cross(t[2].sub(t[0]))).abs() * 0.5
}

#[test]
fn a_curved_wall_is_mitred_to_a_straight_and_a_curved_neighbour_without_gap_or_overlap() {
    // Straight A, then arc B, then arc D, then straight C, all one chain.
    let mut p = Project::new("chain");
    let wall = |p: &mut Project, a: (f64, f64), b: (f64, f64)| {
        p.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            10.0,
            HEIGHT,
            WallKind::Exterior,
        )
    };
    wall(&mut p, (0.0, 0.0), (100.0, 0.0));
    let b = wall(&mut p, (100.0, 0.0), (170.0, 60.0));
    let d = wall(&mut p, (170.0, 60.0), (170.0, 150.0));
    wall(&mut p, (170.0, 150.0), (270.0, 150.0));
    p.floors[0].wall_mut(b).unwrap().curve = Some(WallCurve { bulge: 14.0 });
    p.floors[0].wall_mut(d).unwrap().curve = Some(WallCurve { bulge: -12.0 });
    let walls = p.floors[0].walls.clone();
    let none = |_: &str| None;
    let outlines = wall_outlines(&walls, 0.5);

    // Build each wall with its joined ends: curved walls from the joins, the
    // straight ones from their outlines.
    let mut meshes: Vec<(Id, Vec<Mesh>)> = Vec::new();
    for w in &walls {
        let cuts = if w.is_curved() {
            EndCuts::of(&walls, w)
        } else {
            let o = outlines.iter().find(|o| o.wall_id == w.id).unwrap();
            EndCuts::from_outline(&o.polygon)
        };
        meshes.push((w.id, build_class_cut(w, 0.0, &[], 1.0, &none, None, &cuts)));
    }
    let of_wall = |id: Id| -> Vec<&Mesh> {
        meshes
            .iter()
            .filter(|(i, _)| *i == id)
            .flat_map(|(_, m)| m.iter())
            .collect()
    };

    // The cut line at each curved end: its caps lie on it.
    for id in [b, d] {
        let miters = curved_end_miters(&walls, id, 0.5).unwrap();
        let tops = top_triangles(&of_wall(id));
        assert!(!tops.is_empty());
        for (k, cut) in miters.iter().enumerate() {
            let (l, r) = cut.expect("both ends of the chain are joined");
            let line = r.sub(l).normalized();
            // Distance of the wall's own vertices from the cut line: none on
            // the far side, and the end points exactly on it.
            let side_of = |q: Point| line.cross(q.sub(l));
            let verts: Vec<Point> = tops.iter().flatten().copied().collect();
            let mid = verts
                .iter()
                .fold(Point::ZERO, |a, v| a.add(*v))
                .scale(1.0 / verts.len() as f64);
            let inside = side_of(mid).signum();
            assert!(
                verts.iter().all(|q| side_of(*q) * inside > -1e-3),
                "wall {id} end {k} pokes past its cut"
            );
            let on_cut = verts.iter().filter(|q| side_of(**q).abs() < 1e-3).count();
            assert!(on_cut >= 2, "wall {id} end {k}: {on_cut} points on the cut");
        }
    }

    // Sample the plan: every point inside the joined outlines is under exactly
    // one wall's top face (no gap, no overlap).
    let polys: Vec<(Id, Vec<Point>)> = walls
        .iter()
        .map(|w| {
            let poly = if w.is_curved() {
                curved_wall_polygon(&walls, w.id, 0.5).unwrap()
            } else {
                outlines
                    .iter()
                    .find(|o| o.wall_id == w.id)
                    .unwrap()
                    .polygon
                    .clone()
            };
            (w.id, poly)
        })
        .collect();
    let tops: Vec<(Id, Vec<[Point; 3]>)> = walls
        .iter()
        .map(|w| (w.id, top_triangles(&of_wall(w.id))))
        .collect();
    let (mut inside, mut bad, mut overlap) = (0, 0, 0);
    let mut area_3d = 0.0;
    for (_, ts) in &tops {
        area_3d += ts.iter().map(tri_area).sum::<f64>();
    }
    let mut y = -10.0;
    while y < 170.0 {
        let mut x = -10.0;
        while x < 290.0 {
            let q = Point::new(x + 0.37, y + 0.41);
            if polys.iter().any(|(_, poly)| point_in_polygon(q, poly)) {
                inside += 1;
                let covering = tops
                    .iter()
                    .filter(|(_, ts)| ts.iter().any(|t| in_triangle(q, t)))
                    .count();
                if covering == 0 {
                    bad += 1;
                }
                if covering > 1 {
                    overlap += 1;
                }
            }
            x += 1.5;
        }
        y += 1.5;
    }
    assert!(inside > 1000, "{inside} samples");
    // The plan polygons and the 3-D facets differ by a sliver along the arcs
    // (the plan miter points sit on the tangent offset, 3-D on the arc).
    assert!(
        (bad as f64) < inside as f64 * 0.01,
        "{bad} of {inside} plan points have no top face"
    );
    assert!(
        (overlap as f64) < inside as f64 * 0.01,
        "{overlap} of {inside} plan points lie under two walls"
    );
    let area_2d: f64 = polys.iter().map(|(_, poly)| polygon_area(poly).abs()).sum();
    assert!(
        (area_3d - area_2d).abs() / area_2d < 0.01,
        "top faces {area_3d} vs outlines {area_2d}"
    );
}

#[test]
fn an_unjoined_curved_wall_keeps_square_ends() {
    let (p, id) = arc_project(WallClass::Standard);
    let w = wall_of(&p, id);
    assert_eq!(EndCuts::of(&p.floors[0].walls, &w), EndCuts::NONE);
    // Square ends: the cap is radial, so both cap points share one angle.
    let s = scene(&p);
    let (c, _) = w.arc_center_radius().unwrap();
    let start = w.start.sub(c).normalized();
    let caps: Vec<Point> = of(&s, id)
        .iter()
        .flat_map(|m| m.vertices.iter())
        .map(|v| Point::new(f64::from(v.position[0]), -f64::from(v.position[2])))
        .filter(|q| q.dist(w.start) < THICK)
        .collect();
    assert!(!caps.is_empty());
    for q in caps {
        assert!(q.sub(c).normalized().dist(start) < 1e-6, "{q:?}");
    }
}

// ----- the roof cuts a curved wall -----

const W: f64 = 480.0;
const D: f64 = 360.0;
const PLATE: f64 = 96.0;

fn corners() -> [Point; 4] {
    [
        Point::new(0.0, 0.0),
        Point::new(W, 0.0),
        Point::new(W, D),
        Point::new(0.0, D),
    ]
}

fn house() -> (Project, [Id; 4]) {
    let mut p = Project::new("house");
    let c = corners();
    let mut ids = [0; 4];
    for i in 0..4 {
        ids[i] = p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, PLATE, WallKind::Exterior);
    }
    (p, ids)
}

fn edge(kind: EdgeKind) -> EdgeRoof {
    EdgeRoof {
        pitch_in_12: 8.0,
        kind,
        overhang: 16.0 + 3.0,
    }
}

fn plane_json(plane: &RoofPlane, id: Id) -> Value {
    json!({
        "kind": "plane",
        "id": id,
        "polygon3d": plane.polygon3d,
        "pitch": plane.pitch_in_12,
        "baseline": [plane.baseline.0, plane.baseline.1],
        "overhang": 16.0,
        "ridge_caps": false,
    })
}

fn gable_roof() -> Roof {
    let e = [
        edge(EdgeKind::Hip),
        edge(EdgeKind::Gable),
        edge(EdgeKind::Hip),
        edge(EdgeKind::Gable),
    ];
    build_roof(&corners(), &e, PLATE)
}

fn max_y(meshes: &[&Mesh]) -> f32 {
    meshes
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .fold(f32::MIN, f32::max)
}

#[test]
fn a_curved_gable_end_rises_to_the_ridge_facet_by_facet() {
    let roof = gable_roof();
    let (mut p, ids) = house();
    p.floors[0].roofs = roof
        .planes
        .iter()
        .enumerate()
        .map(|(i, pl)| plane_json(pl, 9000 + i as u64))
        .collect();
    // The right-hand gable end bows in (west) into an arc and is a gable wall.
    let w = p.floors[0].wall_mut(ids[1]).unwrap();
    w.curve = Some(WallCurve { bulge: 30.0 });
    w.roof.kind = RoofWallKind::FullGable;
    let s = build_scene(&p);
    let ridge = roof
        .planes
        .iter()
        .flat_map(|pl| pl.polygon3d.iter().map(|v| v[1]))
        .fold(f64::MIN, f64::max);
    let underside = ridge - 6.0 / roof.planes[0].normal()[1];
    let gable = of(&s, ids[1]);
    let top = f64::from(max_y(&gable));
    assert!(
        (top - underside).abs() < 2.0,
        "curved gable peaks at {top}, ridge underside {underside}"
    );
    assert!(top > PLATE + 100.0);
    // A triangle, not a box: vertices along both slopes, low corners.
    let ys: Vec<f32> = gable
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .collect();
    let low = ys
        .iter()
        .copied()
        .filter(|y| *y > PLATE as f32 + 0.5)
        .fold(f32::MAX, f32::min);
    assert!(top - f64::from(low) > 100.0, "corner {low} peak {top}");
    // The same wall without the directive stops at the plate or below.
    let mut q = p.clone();
    q.floors[0].wall_mut(ids[1]).unwrap().roof.kind = RoofWallKind::Hip;
    let flat = build_scene(&q);
    assert!(f64::from(max_y(&of(&flat, ids[1]))) <= PLATE + 1e-3);
    // The straight eave walls still stop at the plate.
    assert!((f64::from(max_y(&of(&s, ids[0]))) - PLATE).abs() < 1e-3);
    // The wall still follows its arc: it bows 30" off its chord.
    let west = gable
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[0]))
        .fold(f32::MAX, f32::min);
    assert!(west < W as f32 - 25.0, "{west}");
}
