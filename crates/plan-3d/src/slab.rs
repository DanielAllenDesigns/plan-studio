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

type V3d = [f64; 3];

fn sub3(a: V3d, b: V3d) -> V3d {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: V3d, b: V3d) -> V3d {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: V3d) -> Option<V3d> {
    let len = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (len > 1e-9).then(|| [a[0] / len, a[1] / len, a[2] / len])
}

fn f32v(a: V3d) -> [f32; 3] {
    a.map(|c| c as f32)
}

/// A slab of `thickness` inches below a planar polygon in scene space
/// (`x`, `y` up, `z`; inches), for roof planes and ceilings.
///
/// `polygon3d` is the upper surface, in either winding; the slab extends
/// against the plane normal that points up (or the Newell normal when the
/// plane is vertical). Returns an empty mesh for degenerate input.
pub fn slab_from_polygon(polygon3d: &[[f64; 3]], thickness: f64, material: Material) -> Mesh {
    let mut mesh = MeshBuilder::new(material);
    let n = polygon3d.len();
    // Newell normal: CCW orientation when viewed from its tip.
    let mut nn = [0.0; 3];
    for i in 0..n {
        let (a, b) = (polygon3d[i], polygon3d[(i + 1) % n]);
        nn[0] += (a[1] - b[1]) * (a[2] + b[2]);
        nn[1] += (a[2] - b[2]) * (a[0] + b[0]);
        nn[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let Some(nn) = (n >= 3).then(|| unit3(nn)).flatten() else {
        return mesh.finish(None);
    };
    let up = if nn[1] < -1e-9 { nn.map(|c| -c) } else { nn };
    let drop = thickness.max(0.0);
    let bottom = |p: V3d| {
        [
            p[0] - up[0] * drop,
            p[1] - up[1] * drop,
            p[2] - up[2] * drop,
        ]
    };

    // Triangulate in the plane projected along its dominant axis.
    let axis = (0..3)
        .max_by(|&a, &b| nn[a].abs().total_cmp(&nn[b].abs()))
        .unwrap_or(1);
    let (ia, ib) = match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let flat: Vec<Point> = polygon3d.iter().map(|p| Point::new(p[ia], p[ib])).collect();
    let uv = |p: V3d| [(p[ia] / IN_PER_FT) as f32, (p[ib] / IN_PER_FT) as f32];
    for [a, b, c] in ear_clip(&flat) {
        let tri = [polygon3d[a], polygon3d[b], polygon3d[c]];
        let uvs = tri.map(uv);
        mesh.tri(tri.map(f32v), uvs, f32v(up));
        if drop > 0.0 {
            mesh.tri(tri.map(|p| f32v(bottom(p))), uvs, f32v(up.map(|c| -c)));
        }
    }
    if drop > 0.0 {
        for i in 0..n {
            let (a, b) = (polygon3d[i], polygon3d[(i + 1) % n]);
            let Some(out) = unit3(cross3(sub3(b, a), nn)) else {
                continue;
            };
            let len =
                ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            let u = (len / IN_PER_FT) as f32;
            let quad = [a, b, bottom(b), bottom(a)].map(f32v);
            mesh.quad(
                quad,
                [[0.0, 0.0], [u, 0.0], [u, 1.0], [0.0, 1.0]],
                f32v(out),
            );
        }
    }
    mesh.finish(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_square_slab_is_a_closed_box() {
        let sq = [
            [0.0, 100.0, 0.0],
            [120.0, 100.0, 0.0],
            [120.0, 100.0, -120.0],
            [0.0, 100.0, -120.0],
        ];
        for poly in [sq.to_vec(), sq.iter().rev().copied().collect::<Vec<_>>()] {
            let m = slab_from_polygon(&poly, 6.0, Material::Roof);
            assert_eq!(m.triangle_count(), 12);
            let (lo, hi) = m.bounds().unwrap();
            assert!((hi[1] - 100.0).abs() < 1e-4 && (lo[1] - 94.0).abs() < 1e-4);
            // Normals agree with winding and the box faces outward.
            let center = [60.0, 97.0, -60.0];
            for t in m.indices.chunks(3) {
                let p = |i: u32| m.vertices[i as usize].position.map(f64::from);
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                let g = cross3(sub3(b, a), sub3(c, a));
                let out = sub3(a, center);
                assert!(g[0] * out[0] + g[1] * out[1] + g[2] * out[2] > 0.0);
            }
        }
    }

    #[test]
    fn sloped_slab_hangs_below_the_plane_and_degenerate_is_empty() {
        let tilted = [
            [0.0, 0.0, 0.0],
            [120.0, 0.0, 0.0],
            [120.0, 60.0, -120.0],
            [0.0, 60.0, -120.0],
        ];
        let m = slab_from_polygon(&tilted, 4.0, Material::Roof);
        assert_eq!(m.triangle_count(), 12);
        let (lo, _) = m.bounds().unwrap();
        assert!(lo[1] < 0.0);
        assert_eq!(
            slab_from_polygon(&tilted[..2], 4.0, Material::Roof).triangle_count(),
            0
        );
    }
}
