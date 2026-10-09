//! Meshes for corner boards, quoins, moldings, material regions, polygon
//! decks and 3D solids (`plan_core::details`).
//!
//! * [`detail_meshes`] builds everything stored on every floor of a project;
//!   the app adds it to the 3D scene: `scene.meshes.extend(detail_meshes(&project))`.
//! * Corner boards are three boxes (a board on each outer wall face and the
//!   square where they meet) standing off the siding; quoins are the same
//!   "L" stacked course by course, with long and short blocks swapping faces
//!   on alternate courses.
//! * A molding is its cross section swept along every segment of the line.
//! * A material region is a thin plate: on the floor it lies on the finished
//!   floor (or is flush with it when it cuts the finish layers), on a wall it
//!   stands off the chosen face.
//! * A deck is a slab of decking boards with, optionally, posts and a top
//!   rail; solids are boxes, 24-sided cylinders and cones, a 24 x 12 sphere,
//!   prisms, pyramids and flat faces.
//!
//! All lengths are inches; scene space is the usual one (X = plan x, Y up,
//! Z = -plan y).

use crate::builder::{MeshBuilder, V3};
use crate::frame::{to_scene, Frame};
use crate::mesh::{Material, Mesh, Vertex};
use crate::slab::FLOOR_FINISH;
use crate::triangulate::ear_clip;
use plan_core::details::{
    CornerBoard, DeckPolygon, DetailsLayer, MaterialRegion, MoldingLine, Quoin, RegionKind,
    Solid3d, SolidKind, DECK_RAILING_HEIGHT, SOLID_SEGMENTS,
};
use plan_core::geometry::{polygon_area, polygon_centroid, Point};
use plan_core::walls::Side;
use plan_core::{Floor, Project};
use std::f64::consts::{PI, TAU};

const IN_PER_FT: f64 = 12.0;

/// Latitude bands of a sphere (half the segments around it).
pub const SPHERE_STACKS: usize = SOLID_SEGMENTS / 2;
/// A cut floor region sits this far above the finished floor so the plate
/// does not fight the floor slab.
const FLUSH_LIFT: f64 = 0.02;
/// Deck railing posts are at most this far apart, inches.
const POST_SPACING: f64 = 72.0;
/// Deck railing post size, inches.
const POST_SIZE: f64 = 3.5;
/// Deck top rail width and depth, inches.
const RAIL_WIDTH: f64 = 2.5;
const RAIL_DEPTH: f64 = 1.5;

/// Maps a material name from a dialog to a surface material; `default` when
/// the name is empty or not recognised.
pub fn material_of(name: &str, default: Material) -> Material {
    if let Some(m) = Material::from_layer_name(name) {
        return m;
    }
    let n = name.to_ascii_lowercase();
    const TABLE: [(&str, Material); 24] = [
        ("trim", Material::Trim),
        ("paint", Material::Trim),
        ("white", Material::Trim),
        ("metal", Material::Metal),
        ("steel", Material::Metal),
        ("aluminum", Material::Metal),
        ("iron", Material::Metal),
        ("glass", Material::Glass),
        ("roof", Material::Roof),
        ("shingle", Material::Roof),
        ("tile", Material::Stone),
        ("ceramic", Material::Stone),
        ("marble", Material::Stone),
        ("granite", Material::Stone),
        ("wood", Material::Floor),
        ("oak", Material::Floor),
        ("maple", Material::Floor),
        ("deck", Material::Floor),
        ("plank", Material::Floor),
        ("cedar", Material::Floor),
        ("frame", Material::Framing),
        ("lumber", Material::Framing),
        ("fir", Material::Framing),
        ("pine", Material::Framing),
    ];
    TABLE
        .iter()
        .find(|(k, _)| n.contains(k))
        .map_or(default, |&(_, m)| m)
}

fn uv(p: Point) -> [f32; 2] {
    [(p.x / IN_PER_FT) as f32, (p.y / IN_PER_FT) as f32]
}

fn oriented_ccw(ring: &[Point]) -> Vec<Point> {
    let mut v = ring.to_vec();
    if polygon_area(&v) < 0.0 {
        v.reverse();
    }
    v
}

/// Adds a vertical prism over a simple `ring` (any winding) between two scene
/// elevations: top and bottom caps and the side walls.
fn add_prism(mesh: &mut MeshBuilder, ring: &[Point], y0: f64, y1: f64) {
    if ring.len() < 3 || y1 - y0 <= 1e-9 || polygon_area(ring).abs() < 1e-9 {
        return;
    }
    let ring = oriented_ccw(ring);
    for [a, b, c] in ear_clip(&ring) {
        let tri = [ring[a], ring[b], ring[c]];
        if polygon_area(&tri).abs() < 1e-9 {
            continue;
        }
        let uvs = tri.map(uv);
        mesh.tri(tri.map(|p| to_scene(p, y1)), uvs, [0.0, 1.0, 0.0]);
        mesh.tri(tri.map(|p| to_scene(p, y0)), uvs, [0.0, -1.0, 0.0]);
    }
    add_sides(mesh, &ring, y0, y1);
}

/// Side walls of a counter-clockwise ring.
fn add_sides(mesh: &mut MeshBuilder, ring: &[Point], y0: f64, y1: f64) {
    let (v0, v1) = ((y0 / IN_PER_FT) as f32, (y1 / IN_PER_FT) as f32);
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let len = a.dist(b);
        if len <= 1e-9 {
            continue;
        }
        let d = (b - a).normalized();
        let out = [d.y as f32, 0.0, d.x as f32];
        let quad = [
            to_scene(a, y0),
            to_scene(b, y0),
            to_scene(b, y1),
            to_scene(a, y1),
        ];
        let u = (len / IN_PER_FT) as f32;
        mesh.quad(quad, [[0.0, v0], [u, v0], [u, v1], [0.0, v1]], out);
    }
}

/// A beam of `width` along `a`-`b` between two scene elevations.
fn add_beam(mesh: &mut MeshBuilder, a: Point, b: Point, width: f64, y0: f64, y1: f64) {
    let d = (b - a).normalized();
    if d.length() < 0.5 {
        return;
    }
    let n = d.perp() * (width * 0.5);
    add_prism(mesh, &[a - n, b - n, b + n, a + n], y0, y1);
}

fn finished(mesh: MeshBuilder, id: u64) -> Option<Mesh> {
    (!mesh.is_empty()).then(|| mesh.finish(Some(id)))
}

// ===================================================================
// Corner boards and quoins
// ===================================================================

/// The corner board at an exterior corner: three boxes in an "L" standing off
/// the outer wall faces.
pub fn corner_board_mesh(b: &CornerBoard, floor_elev: f64) -> Option<Mesh> {
    if b.width <= 0.0 || b.thickness <= 0.0 || b.height <= 0.0 {
        return None;
    }
    let mut mesh = MeshBuilder::new(material_of(&b.material, Material::Trim));
    let (y0, y1) = (floor_elev + b.base, floor_elev + b.base + b.height);
    for part in b.parts() {
        add_prism(&mut mesh, &part, y0, y1);
    }
    finished(mesh, b.id)
}

/// The stack of quoin blocks at an exterior corner, one "L" per course.
pub fn quoin_mesh(q: &Quoin, floor_elev: f64) -> Option<Mesh> {
    if q.width <= 0.0 || q.height <= 0.0 || q.depth <= 0.0 || q.total_height <= 0.0 {
        return None;
    }
    let mut mesh = MeshBuilder::new(material_of(&q.material, Material::Stone));
    for i in 0..q.courses() {
        let (la, lb) = q.course_lengths(i);
        let ring = q.axes.l_polygon(q.corner, la, lb, q.depth);
        let y0 = floor_elev + q.course_base(i);
        add_prism(&mut mesh, &ring, y0, y0 + q.height);
    }
    finished(mesh, q.id)
}

// ===================================================================
// Moldings
// ===================================================================

/// A molding: the cross section swept along the line (the molding projects
/// to the left of the drawing direction). Segments meet at mitered joints:
/// the section is offset along the bisector of the two segments' normals, so
/// the faces of neighbouring segments meet exactly at each corner. A closed
/// line (last point on the first) miters at its start too; an open one is
/// capped at its two ends.
pub fn molding_mesh(m: &MoldingLine, floor_elev: f64) -> Option<Mesh> {
    let section = m.section();
    if m.polyline.len() < 2 || section.len() < 3 || m.height <= 0.0 || m.width <= 0.0 {
        return None;
    }
    // The line without repeated points.
    let mut pts: Vec<Point> = Vec::with_capacity(m.polyline.len());
    for p in &m.polyline {
        if pts.last().is_none_or(|q| q.dist(*p) > 1e-9) {
            pts.push(*p);
        }
    }
    if pts.len() < 2 {
        return None;
    }
    let n = pts.len();
    let closed = n > 2 && pts[0].dist(pts[n - 1]) < 1e-6;
    let seg_normal = |i: usize| (pts[i + 1] - pts[i]).normalized().perp();
    // Where the section's offset goes at vertex `j`: the direction to offset
    // along and the factor that keeps the faces of both segments in line.
    let lateral = |j: usize| -> (Point, f64) {
        let prev = if j > 0 {
            Some(seg_normal(j - 1))
        } else if closed {
            Some(seg_normal(n - 2))
        } else {
            None
        };
        let next = if j + 1 < n {
            Some(seg_normal(j))
        } else if closed {
            Some(seg_normal(0))
        } else {
            None
        };
        match (prev, next) {
            (Some(p), Some(q)) => {
                let sum = p + q;
                if sum.length() < 1e-9 {
                    (q, 1.0)
                } else {
                    let bisector = sum.normalized();
                    (bisector, 1.0 / bisector.dot(q).max(MITER_LIMIT))
                }
            }
            (Some(p), None) => (p, 1.0),
            (None, Some(q)) => (q, 1.0),
            (None, None) => (Point::ZERO, 1.0),
        }
    };
    let mut mesh = MeshBuilder::new(material_of(&m.material, Material::Trim));
    let base = floor_elev + m.elevation;
    let centre = polygon_centroid(&section);
    for i in 0..n - 1 {
        let (a, b) = (pts[i], pts[i + 1]);
        let len = a.dist(b);
        let d = (b - a).normalized();
        let nrm = d.perp();
        let (la, ka) = lateral(i);
        let (lb, kb) = lateral(i + 1);
        let at_a = |s: Point| to_scene(a + la * (s.x * ka), base + s.y);
        let at_b = |s: Point| to_scene(b + lb * (s.x * kb), base + s.y);
        for j in 0..section.len() {
            let (s0, s1) = (section[j], section[(j + 1) % section.len()]);
            let e = s1 - s0;
            if e.length() <= 1e-9 {
                continue;
            }
            // Outward normal of a counter-clockwise section edge.
            let (nu, nv) = (e.y, -e.x);
            let l = nu.hypot(nv);
            let (nu, nv) = (nu / l, nv / l);
            let normal = [(nrm.x * nu) as f32, nv as f32, (-(nrm.y * nu)) as f32];
            let u = (len / IN_PER_FT) as f32;
            let (v0, v1) = ((s0.y / IN_PER_FT) as f32, (s1.y / IN_PER_FT) as f32);
            mesh.quad(
                [at_a(s0), at_b(s0), at_b(s1), at_a(s1)],
                [[0.0, v0], [u, v0], [u, v1], [0.0, v1]],
                normal,
            );
        }
        // End caps: a fan from the section's centre, at the free ends of an
        // open line only (the joints are closed by the neighbour).
        for (end, sign) in [(0, -1.0_f64), (1, 1.0)] {
            if closed || (end == 0 && i > 0) || (end == 1 && i + 2 < n) {
                continue;
            }
            let normal = [(d.x * sign) as f32, 0.0, (-(d.y * sign)) as f32];
            let at = |s: Point| if end == 0 { at_a(s) } else { at_b(s) };
            for j in 0..section.len() {
                let (s0, s1) = (section[j], section[(j + 1) % section.len()]);
                mesh.tri([at(centre), at(s0), at(s1)], [[0.0, 0.0]; 3], normal);
            }
        }
    }
    finished(mesh, m.id)
}

/// The least cosine between a joint's bisector and a segment normal the miter
/// follows; sharper corners are cut off at 4 times the section's projection.
const MITER_LIMIT: f64 = 0.25;

// ===================================================================
// Material regions
// ===================================================================

/// The plate of a material region on `floor`, or `None` when its wall is
/// gone (or curved) or the region is degenerate.
pub fn region_mesh(r: &MaterialRegion, floor: &Floor) -> Option<Mesh> {
    if r.thickness <= 0.0 || r.outline.len() < 3 {
        return None;
    }
    let material = material_of(&r.material, Material::Floor);
    let mut mesh = MeshBuilder::new(material);
    match r.kind {
        RegionKind::Floor => {
            let top = floor.elevation + FLOOR_FINISH;
            let (y0, y1) = if r.cut_finish_layers {
                (top - r.thickness, top + FLUSH_LIFT)
            } else {
                (top, top + r.thickness)
            };
            add_prism(&mut mesh, &r.outline, y0, y1);
        }
        RegionKind::Wall(id) => {
            let wall = floor.wall(id).filter(|w| !w.is_curved())?;
            let (u0, u1, v0, v1) = r.uv_bounds()?;
            let (u0, u1) = (u0.max(0.0), u1.min(wall.length()));
            let (v0, v1) = (v0.max(0.0), v1.min(wall.height));
            if u1 - u0 <= 1e-9 || v1 - v0 <= 1e-9 {
                return None;
            }
            let half = wall.thickness * 0.5;
            let t = match r.side {
                Side::Left => (half, half + r.thickness),
                Side::Right => (-half - r.thickness, -half),
            };
            Frame::new(wall, floor.elevation).cuboid(&mut mesh, (u0, u1), t, (v0, v1));
        }
    }
    finished(mesh, r.id)
}

// ===================================================================
// Decks
// ===================================================================

/// The deck slab (decking boards) and, when it has a railing, a second mesh
/// with its posts and top rail.
pub fn deck_meshes(d: &DeckPolygon, floor_elev: f64) -> Vec<Mesh> {
    if d.outline.len() < 3 || d.board_thickness <= 0.0 || d.area() < 1e-6 {
        return Vec::new();
    }
    let top = floor_elev + d.elevation;
    let mut boards = MeshBuilder::new(material_of(&d.material, Material::Floor));
    add_prism(&mut boards, &d.outline, top - d.board_thickness, top);
    let mut out: Vec<Mesh> = finished(boards, d.id).into_iter().collect();
    if d.railing {
        let mut rail = MeshBuilder::new(Material::Trim);
        let ring = oriented_ccw(&d.outline);
        let n = ring.len();
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            let len = a.dist(b);
            if len <= 1e-9 {
                continue;
            }
            let spans = (len / POST_SPACING).ceil().max(1.0) as usize;
            for k in 0..spans {
                let p = Point::lerp(a, b, k as f64 / spans as f64);
                add_beam(
                    &mut rail,
                    p,
                    p + (b - a).normalized() * POST_SIZE,
                    POST_SIZE,
                    top,
                    top + DECK_RAILING_HEIGHT,
                );
            }
            add_beam(
                &mut rail,
                a,
                b,
                RAIL_WIDTH,
                top + DECK_RAILING_HEIGHT - RAIL_DEPTH,
                top + DECK_RAILING_HEIGHT,
            );
        }
        out.extend(finished(rail, d.id));
    }
    out
}

// ===================================================================
// Solids
// ===================================================================

/// A cone of `r` on the polygon `base` (flat shaded).
fn add_pyramid(mesh: &mut MeshBuilder, base: &[Point], y0: f64, apex: V3) {
    let ring = oriented_ccw(base);
    for [a, b, c] in ear_clip(&ring) {
        let tri = [ring[a], ring[b], ring[c]];
        if polygon_area(&tri).abs() < 1e-9 {
            continue;
        }
        mesh.tri(tri.map(|p| to_scene(p, y0)), tri.map(uv), [0.0, -1.0, 0.0]);
    }
    for i in 0..ring.len() {
        let (a, b) = (
            to_scene(ring[i], y0),
            to_scene(ring[(i + 1) % ring.len()], y0),
        );
        let (e1, e2) = (sub(b, a), sub(apex, a));
        let mut nrm = cross(e1, e2);
        let l = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        if l <= 1e-9 {
            continue;
        }
        nrm = [nrm[0] / l, nrm[1] / l, nrm[2] / l];
        mesh.tri([a, b, apex], [[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]], nrm);
    }
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A smooth sphere of [`SOLID_SEGMENTS`] segments around and
/// [`SPHERE_STACKS`] bands from pole to pole, centred on `center` (scene
/// space). `2 * segments + 2 * (stacks - 2) * segments` triangles.
pub fn sphere_mesh(center: V3, r: f64, material: Material, object_id: Option<u64>) -> Mesh {
    let (segs, stacks) = (SOLID_SEGMENTS, SPHERE_STACKS);
    let mut vertices: Vec<Vertex> = Vec::new();
    for i in 0..=stacks {
        let phi = PI * i as f64 / stacks as f64;
        for j in 0..=segs {
            let theta = TAU * j as f64 / segs as f64;
            let n = [
                (phi.sin() * theta.cos()) as f32,
                phi.cos() as f32,
                (phi.sin() * theta.sin()) as f32,
            ];
            vertices.push(Vertex {
                position: [
                    center[0] + n[0] * r as f32,
                    center[1] + n[1] * r as f32,
                    center[2] + n[2] * r as f32,
                ],
                normal: n,
                uv: [j as f32 / segs as f32, 1.0 - i as f32 / stacks as f32],
            });
        }
    }
    let at = |i: usize, j: usize| (i * (segs + 1) + j) as u32;
    let mut indices: Vec<u32> = Vec::new();
    for i in 0..stacks {
        for j in 0..segs {
            let (a, b) = (at(i, j), at(i, j + 1));
            let (c, d) = (at(i + 1, j), at(i + 1, j + 1));
            // Counter-clockwise seen from outside (y up, theta toward +z).
            if i != 0 {
                indices.extend([a, b, c]);
            }
            if i != stacks - 1 {
                indices.extend([b, d, c]);
            }
        }
    }
    Mesh {
        vertices,
        indices,
        material,
        object_id,
        color: None,
    }
}

/// The mesh of a 3D solid on a floor at `floor_elev`.
pub fn solid_mesh(s: &Solid3d, floor_elev: f64) -> Option<Mesh> {
    let material = material_of(&s.material, Material::Concrete);
    let base = floor_elev + s.elevation;
    let foot = s.footprint();
    let mut mesh = MeshBuilder::new(material);
    match &s.kind {
        SolidKind::Box { w, d, h } => {
            if *w <= 0.0 || *d <= 0.0 || *h <= 0.0 {
                return None;
            }
            add_prism(&mut mesh, &foot, base, base + h);
        }
        SolidKind::Cylinder { r, h } => {
            if *r <= 0.0 || *h <= 0.0 {
                return None;
            }
            add_prism(&mut mesh, &foot, base, base + h);
        }
        SolidKind::PolylineSolid { h, .. } => {
            if *h <= 0.0 {
                return None;
            }
            add_prism(&mut mesh, &foot, base, base + h);
        }
        SolidKind::Cone { r, h } => {
            if *r <= 0.0 || *h <= 0.0 {
                return None;
            }
            add_pyramid(&mut mesh, &foot, base, to_scene(s.position, base + h));
        }
        SolidKind::Pyramid { h, .. } => {
            if *h <= 0.0 || foot.len() < 3 {
                return None;
            }
            let apex = to_scene(polygon_centroid(&foot), base + h);
            add_pyramid(&mut mesh, &foot, base, apex);
        }
        SolidKind::Sphere { r } => {
            if *r <= 0.0 {
                return None;
            }
            let c = to_scene(s.position, base + r);
            return Some(sphere_mesh(c, *r, material, Some(s.id)));
        }
        SolidKind::Face { .. } => {
            let ring = oriented_ccw(&foot);
            if ring.len() < 3 {
                return None;
            }
            for [a, b, c] in ear_clip(&ring) {
                let tri = [ring[a], ring[b], ring[c]];
                if polygon_area(&tri).abs() < 1e-9 {
                    continue;
                }
                mesh.tri(tri.map(|p| to_scene(p, base)), tri.map(uv), [0.0, 1.0, 0.0]);
                mesh.tri(
                    tri.map(|p| to_scene(p, base)),
                    tri.map(uv),
                    [0.0, -1.0, 0.0],
                );
            }
        }
    }
    finished(mesh, s.id)
}

// ===================================================================
// Floors and projects
// ===================================================================

/// Every mesh of one floor's [`DetailsLayer`].
pub fn floor_detail_meshes(floor: &Floor) -> Vec<Mesh> {
    let layer = DetailsLayer::load(floor);
    if layer.is_empty() && floor.solid_layer.compounds.is_empty() {
        return Vec::new();
    }
    let e = floor.elevation;
    let mut out = Vec::new();
    out.extend(
        layer
            .corner_boards
            .iter()
            .filter_map(|b| corner_board_mesh(b, e)),
    );
    out.extend(layer.quoins.iter().filter_map(|q| quoin_mesh(q, e)));
    out.extend(layer.moldings.iter().filter_map(|m| molding_mesh(m, e)));
    // Layered regions, solids with extra spec fields and compound solids
    // (Boolean results) have their own passes.
    out.extend(crate::material_region::region_meshes(floor, &layer.regions));
    out.extend(layer.decks.iter().flat_map(|d| deck_meshes(d, e)));
    out.extend(crate::solids::solid_meshes(floor, &layer.solids, e));
    out
}

/// Every detail mesh of the project, ready to extend a scene:
/// `scene.meshes.extend(detail_meshes(&project))`.
pub fn detail_meshes(project: &Project) -> Vec<Mesh> {
    project
        .floors
        .iter()
        .flat_map(floor_detail_meshes)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::details::{wall_rect, MoldingProfile};
    use plan_core::{detect_rooms, WallKind};

    fn square() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(96.0, 0.0),
            Point::new(96.0, 96.0),
            Point::new(0.0, 96.0),
        ]
    }

    fn box_project() -> Project {
        let mut p = Project::new("t");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, 108.0, WallKind::Exterior);
        }
        p
    }

    fn volume(m: &Mesh) -> f64 {
        // Signed volume from the triangles (closed meshes only).
        let p = |i: u32| m.vertices[i as usize].position;
        m.indices
            .chunks(3)
            .map(|t| {
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                let cr = cross(b, c);
                f64::from(a[0] * cr[0] + a[1] * cr[1] + a[2] * cr[2]) / 6.0
            })
            .sum()
    }

    #[test]
    fn auto_corner_boards_become_four_meshes_outside_the_walls() {
        let mut p = box_project();
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let floor = p.floors[0].clone();
        let mut layer = DetailsLayer::default();
        let mut ids = 100..;
        layer.auto_corner_boards(&floor, &rooms, &mut || ids.next().unwrap());
        layer.store(&mut p.floors[0]);
        let meshes = detail_meshes(&p);
        assert_eq!(meshes.len(), 4);
        for m in &meshes {
            assert_eq!(m.material, Material::Trim);
            assert!(m.triangle_count() > 0);
            let (lo, hi) = m.bounds().unwrap();
            // Floor to the top plate, standing off the outer face (3.25").
            assert!((lo[1]).abs() < 1e-4 && (hi[1] - 108.0).abs() < 1e-4);
            let outside_x = lo[0] < -3.0 || hi[0] > 243.0;
            assert!(outside_x, "{lo:?} {hi:?}");
        }
    }

    #[test]
    fn quoins_stack_in_courses() {
        let q = Quoin {
            id: 1,
            total_height: 40.0,
            ..Quoin::default()
        };
        assert_eq!(q.courses(), 5);
        let m = quoin_mesh(&q, 0.0).unwrap();
        let (lo, hi) = m.bounds().unwrap();
        assert!(lo[1].abs() < 1e-4 && (hi[1] - 40.0).abs() < 1e-4);
        assert_eq!(m.material, Material::Stone);
        // An L prism is 6 sides (12 triangles) + 4 cap triangles at each end.
        assert_eq!(m.triangle_count(), 5 * (12 + 8));
        let flat = Quoin {
            total_height: 7.0,
            ..q
        };
        assert_eq!(flat.courses(), 1);
    }

    /// Scene vertices of `mesh` that lie at plan point `p` (scene z = -y).
    fn vertices_at(mesh: &Mesh, p: Point, y: f32) -> usize {
        mesh.vertices
            .iter()
            .filter(|v| {
                (v.position[0] as f64 - p.x).abs() < 1e-3
                    && (v.position[2] as f64 + p.y).abs() < 1e-3
                    && (v.position[1] - y).abs() < 1e-3
            })
            .count()
    }

    #[test]
    fn moldings_miter_at_polyline_corners() {
        let mut m = MoldingLine::new(
            1,
            vec![
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(100.0, 100.0),
            ],
            MoldingProfile::Base,
            109.125,
        );
        m.width = 4.0;
        m.height = 6.0;
        let mesh = molding_mesh(&m, 0.0).unwrap();
        // The wall edge of the section stays on the path at the corner; the
        // far edge meets at the mitre point (96, 4), not at (100, 4) / (96, 0).
        assert!(vertices_at(&mesh, Point::new(100.0, 0.0), 0.0) > 0);
        assert!(vertices_at(&mesh, Point::new(96.0, 4.0), 0.0) > 0);
        assert_eq!(vertices_at(&mesh, Point::new(100.0, 4.0), 0.0), 0);
        assert_eq!(vertices_at(&mesh, Point::new(96.0, 0.0), 0.0), 0);

        // Turning the other way puts the molding on the outside of the corner:
        // the mitre point is outside the path.
        let mut out = m.clone();
        out.polyline.reverse();
        out.polyline = vec![
            Point::new(100.0, 100.0),
            Point::new(100.0, 0.0),
            Point::new(0.0, 0.0),
        ];
        let mesh = molding_mesh(&out, 0.0).unwrap();
        assert!(
            vertices_at(&mesh, Point::new(104.0, -4.0), 0.0) > 0,
            "outer mitre point"
        );

        // A closed line has no end caps at all and one mitre per corner.
        let mut closed = m.clone();
        closed.polyline = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
            Point::new(0.0, 0.0),
        ];
        let mesh = molding_mesh(&closed, 0.0).unwrap();
        assert_eq!(mesh.triangle_count(), 4 * 8, "four segments, no caps");
        assert!(vertices_at(&mesh, Point::new(4.0, 4.0), 0.0) > 0);
        assert!(vertices_at(&mesh, Point::new(96.0, 4.0), 0.0) > 0);
        let want = 6.0 * (4.0 * 400.0 - 4.0 * 16.0);
        assert!(
            (volume(&mesh).abs() - want).abs() / want < 1e-3,
            "{}",
            volume(&mesh)
        );
    }

    #[test]
    fn a_molding_line_extrudes_a_closed_profile() {
        let m = MoldingLine::new(
            1,
            vec![
                Point::new(0.0, 0.0),
                Point::new(96.0, 0.0),
                Point::new(96.0, 48.0),
            ],
            MoldingProfile::Crown,
            109.125,
        );
        let mesh = molding_mesh(&m, 0.0).expect("a molding mesh");
        assert_eq!(mesh.material, Material::Trim);
        // 4 section edges x 2 triangles per segment, and a fan of 4 at each
        // of the two free ends (the joint in the middle is mitred, not capped).
        assert_eq!(mesh.triangle_count(), 2 * 8 + 2 * 4);
        let (lo, hi) = mesh.bounds().unwrap();
        assert!((lo[1] - (109.125 - 4.5) as f32).abs() < 1e-3);
        assert!((hi[1] - 109.125).abs() < 1e-3);
        // The molding turns left, so it lies on the inside of the corner:
        // the two 3.5"-wide strips of 144" overlap in a 3.5" square, and the
        // mitre shares it between them (4.5" high).
        let want = 4.5 * (3.5 * 144.0 - 3.5 * 3.5);
        assert!(
            (volume(&mesh).abs() - want).abs() / want < 1e-3,
            "{}",
            volume(&mesh)
        );
        assert!(molding_mesh(&MoldingLine::default(), 0.0).is_none());
    }

    #[test]
    fn the_molding_projects_to_the_left_of_the_line() {
        let m = MoldingLine::new(
            1,
            vec![Point::new(0.0, 0.0), Point::new(96.0, 0.0)],
            MoldingProfile::Base,
            109.0,
        );
        let mesh = molding_mesh(&m, 0.0).unwrap();
        let (lo, hi) = mesh.bounds().unwrap();
        // Left of +x is +plan y, which is -z in the scene.
        assert!(
            (lo[2] + 0.75).abs() < 1e-4 && hi[2].abs() < 1e-4,
            "{lo:?} {hi:?}"
        );
    }

    #[test]
    fn floor_regions_are_plates_on_or_flush_with_the_finished_floor() {
        let floor = Floor::new("1", 0.0);
        let r = MaterialRegion::floor(1, square());
        let m = region_mesh(&r, &floor).unwrap();
        let (lo, hi) = m.bounds().unwrap();
        assert!((lo[1] - FLOOR_FINISH as f32).abs() < 1e-4);
        assert!((hi[1] - (FLOOR_FINISH + 0.25) as f32).abs() < 1e-4);
        let cut = MaterialRegion {
            cut_finish_layers: true,
            ..r
        };
        let m = region_mesh(&cut, &floor).unwrap();
        let (lo, hi) = m.bounds().unwrap();
        assert!((lo[1] - (FLOOR_FINISH - 0.25) as f32).abs() < 1e-4);
        assert!(hi[1] < (FLOOR_FINISH + 0.05) as f32);
    }

    #[test]
    fn wall_regions_stand_off_the_chosen_face() {
        let p = box_project();
        let floor = &p.floors[0];
        let w = &floor.walls[0]; // along +x from the origin, normal +y
        let mut r = MaterialRegion::wall(1, w.id, Side::Left, 24.0, 72.0, 12.0, 60.0);
        r.thickness = 0.5;
        let m = region_mesh(&r, floor).unwrap();
        let (lo, hi) = m.bounds().unwrap();
        // Left of +x is +plan y = -z in the scene; face at 3.25".
        assert!(
            (lo[2] + 3.75).abs() < 1e-4 && (hi[2] + 3.25).abs() < 1e-4,
            "{lo:?} {hi:?}"
        );
        assert!((lo[0] - 24.0).abs() < 1e-4 && (hi[0] - 72.0).abs() < 1e-4);
        assert!((lo[1] - 12.0).abs() < 1e-4 && (hi[1] - 60.0).abs() < 1e-4);
        r.side = Side::Right;
        let m = region_mesh(&r, floor).unwrap();
        let (lo, hi) = m.bounds().unwrap();
        assert!((lo[2] - 3.25).abs() < 1e-4 && (hi[2] - 3.75).abs() < 1e-4);
        // A missing wall gives nothing; the region clamps to the wall.
        let gone = MaterialRegion {
            kind: RegionKind::Wall(9999),
            ..r.clone()
        };
        assert!(region_mesh(&gone, floor).is_none());
        let big = MaterialRegion {
            outline: wall_rect(0.0, 9999.0, 0.0, 9999.0),
            ..r
        };
        let (lo, hi) = region_mesh(&big, floor).unwrap().bounds().unwrap();
        assert!(lo[0] >= -1e-4 && hi[0] <= 240.0 + 1e-4 && hi[1] <= 108.0 + 1e-4);
    }

    #[test]
    fn a_deck_is_a_one_and_a_half_inch_slab_with_an_optional_railing() {
        let mut d = DeckPolygon::new(1, square());
        d.elevation = 30.0;
        let meshes = deck_meshes(&d, 0.0);
        assert_eq!(meshes.len(), 1);
        let (lo, hi) = meshes[0].bounds().unwrap();
        assert!((lo[1] - 28.5).abs() < 1e-4 && (hi[1] - 30.0).abs() < 1e-4);
        let v = volume(&meshes[0]).abs();
        assert!(
            (v - d.volume()).abs() / d.volume() < 1e-3,
            "{v} vs {}",
            d.volume()
        );
        d.railing = true;
        let meshes = deck_meshes(&d, 0.0);
        assert_eq!(meshes.len(), 2);
        assert_eq!(meshes[1].material, Material::Trim);
        let (_, hi) = meshes[1].bounds().unwrap();
        assert!((hi[1] - 66.0).abs() < 1e-4);
        assert!(deck_meshes(&DeckPolygon::default(), 0.0).is_empty());
    }

    #[test]
    fn a_sphere_has_the_expected_triangle_count_and_outward_faces() {
        let s = Solid3d::new(1, SolidKind::Sphere { r: 12.0 }, Point::new(10.0, 20.0));
        let m = solid_mesh(&s, 0.0).unwrap();
        // 24 segments, 12 stacks: 2 pole fans + 10 bands of 2 triangles.
        assert_eq!(m.triangle_count(), 2 * 24 + 10 * 24 * 2);
        let (lo, hi) = m.bounds().unwrap();
        assert!(lo[1].abs() < 1e-4 && (hi[1] - 24.0).abs() < 1e-4);
        let centre = [10.0_f32, 12.0, -20.0];
        for t in m.indices.chunks(3) {
            let p = |i: u32| m.vertices[i as usize].position;
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let n = cross(sub(b, a), sub(c, a));
            let mid = [
                (a[0] + b[0] + c[0]) / 3.0 - centre[0],
                (a[1] + b[1] + c[1]) / 3.0 - centre[1],
                (a[2] + b[2] + c[2]) / 3.0 - centre[2],
            ];
            let d = n[0] * mid[0] + n[1] * mid[1] + n[2] * mid[2];
            assert!(d >= -1e-3, "a triangle faces inward");
        }
        // Volume of a 24 x 12 sphere is within 4% of the true one.
        let want = 4.0 / 3.0 * PI * 1728.0;
        assert!((volume(&m).abs() - want).abs() / want < 0.04);
    }

    #[test]
    fn cylinders_cones_boxes_prisms_pyramids_and_faces_all_mesh() {
        let at = Point::new(50.0, 50.0);
        let cyl = Solid3d::new(1, SolidKind::Cylinder { r: 6.0, h: 40.0 }, at);
        let m = solid_mesh(&cyl, 0.0).unwrap();
        // 24 sides x 2 + caps of 22 triangles x 2.
        assert_eq!(m.triangle_count(), 24 * 2 + 2 * 22);
        let want = PI * 36.0 * 40.0;
        assert!((volume(&m).abs() - want).abs() / want < 0.05);
        let cone = Solid3d::new(2, SolidKind::Cone { r: 6.0, h: 30.0 }, at);
        let m = solid_mesh(&cone, 0.0).unwrap();
        assert_eq!(m.triangle_count(), 24 + 22);
        let (_, hi) = m.bounds().unwrap();
        assert!((hi[1] - 30.0).abs() < 1e-4);
        let b = Solid3d::new(
            3,
            SolidKind::Box {
                w: 10.0,
                d: 20.0,
                h: 30.0,
            },
            at,
        );
        let m = solid_mesh(&b, 0.0).unwrap();
        assert_eq!(m.triangle_count(), 12);
        assert!((volume(&m).abs() - 6000.0).abs() < 1.0);
        let tri_outline = vec![
            Point::new(-10.0, -10.0),
            Point::new(10.0, -10.0),
            Point::new(0.0, 10.0),
        ];
        let prism = Solid3d::new(
            4,
            SolidKind::PolylineSolid {
                outline: tri_outline.clone(),
                h: 12.0,
            },
            at,
        );
        assert_eq!(solid_mesh(&prism, 0.0).unwrap().triangle_count(), 3 * 2 + 2);
        let pyr = Solid3d::new(
            5,
            SolidKind::Pyramid {
                outline: tri_outline.clone(),
                h: 12.0,
            },
            at,
        );
        assert_eq!(solid_mesh(&pyr, 0.0).unwrap().triangle_count(), 3 + 1);
        let face = Solid3d::new(
            6,
            SolidKind::Face {
                polygon: tri_outline,
            },
            at,
        );
        assert_eq!(solid_mesh(&face, 0.0).unwrap().triangle_count(), 2);
        assert!(solid_mesh(&Solid3d::new(7, SolidKind::Sphere { r: 0.0 }, at), 0.0).is_none());
        // Elevation lifts the base; rotation turns the footprint.
        let lifted = Solid3d {
            elevation: 36.0,
            rotation: 90.0,
            ..b
        };
        let (lo, hi) = solid_mesh(&lifted, 0.0).unwrap().bounds().unwrap();
        assert!((lo[1] - 36.0).abs() < 1e-4 && (hi[1] - 66.0).abs() < 1e-4);
        assert!((hi[0] - lo[0] - 20.0).abs() < 1e-3);
    }

    #[test]
    fn material_names_map_to_surface_materials() {
        assert_eq!(material_of("Brick – Red", Material::Trim), Material::Brick);
        assert_eq!(material_of("Oak Floor", Material::Trim), Material::Floor);
        assert_eq!(material_of("Zinc Panel", Material::Metal), Material::Metal);
        assert_eq!(material_of("", Material::Concrete), Material::Concrete);
    }

    #[test]
    fn an_empty_project_has_no_detail_meshes() {
        assert!(detail_meshes(&Project::new("e")).is_empty());
    }
}
