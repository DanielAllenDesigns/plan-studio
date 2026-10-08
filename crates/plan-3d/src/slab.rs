//! Floor and ceiling slabs triangulated from detected room polygons.

use crate::builder::MeshBuilder;
use crate::frame::to_scene;
use crate::mesh::{Material, Mesh};
use crate::triangulate::ear_clip;
use plan_core::geometry::polygon_area;
use plan_core::{Point, Room};

/// Slab thickness, inches.
pub const SLAB_THICKNESS: f64 = 1.0;
/// Finish thickness added above the floor elevation, inches.
pub const FLOOR_FINISH: f64 = 0.75;

const IN_PER_FT: f64 = 12.0;

fn uv(p: Point) -> [f32; 2] {
    [(p.x / IN_PER_FT) as f32, (p.y / IN_PER_FT) as f32]
}

/// Room polygon forced counter-clockwise.
fn ccw(polygon: &[Point]) -> Vec<Point> {
    let mut pts = polygon.to_vec();
    if polygon_area(&pts) < 0.0 {
        pts.reverse();
    }
    pts
}

/// Top (facing up) and bottom (facing down) faces of one polygon.
fn add_caps(mesh: &mut MeshBuilder, pts: &[Point], y_bottom: f64, y_top: f64) {
    for [a, b, c] in ear_clip(pts) {
        let tri = [pts[a], pts[b], pts[c]];
        let uvs = tri.map(uv);
        mesh.tri(tri.map(|p| to_scene(p, y_top)), uvs, [0.0, 1.0, 0.0]);
        mesh.tri(tri.map(|p| to_scene(p, y_bottom)), uvs, [0.0, -1.0, 0.0]);
    }
}

/// Outward-facing side walls of a CCW polygon's slab edge.
fn add_sides(mesh: &mut MeshBuilder, pts: &[Point], y_bottom: f64, y_top: f64) {
    let (v0, v1) = ((y_bottom / IN_PER_FT) as f32, (y_top / IN_PER_FT) as f32);
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let len = a.dist(b);
        if len <= 1e-9 {
            continue;
        }
        let d = (b - a).normalized();
        let out = [d.y as f32, 0.0, d.x as f32]; // plan (dy, -dx) -> scene (x, _, -y)
        let quad = [
            to_scene(a, y_bottom),
            to_scene(b, y_bottom),
            to_scene(b, y_top),
            to_scene(a, y_top),
        ];
        let u = (len / IN_PER_FT) as f32;
        mesh.quad(quad, [[0.0, v0], [u, v0], [u, v1], [0.0, v1]], out);
    }
}

/// One slab mesh covering every room, between `y_bottom` and `y_top` (scene inches).
pub fn build_slab(material: Material, rooms: &[Room], y_bottom: f64, y_top: f64) -> Option<Mesh> {
    let mut mesh = MeshBuilder::new(material);
    for room in rooms {
        let pts = ccw(&room.polygon);
        add_caps(&mut mesh, &pts, y_bottom, y_top);
        add_sides(&mut mesh, &pts, y_bottom, y_top);
    }
    (!mesh.is_empty()).then(|| mesh.finish(None))
}
