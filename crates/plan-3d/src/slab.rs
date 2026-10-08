//! Floor and ceiling slabs triangulated from detected room polygons.

use crate::builder::MeshBuilder;
use crate::mesh::{Material, Mesh};
use crate::triangulate::ear_clip;
use plan_core::Point;

/// Slab thickness, inches.
pub const SLAB_THICKNESS: f64 = 1.0;
/// Finish thickness added above the floor elevation, inches.
pub const FLOOR_FINISH: f64 = 0.75;

const IN_PER_FT: f64 = 12.0;

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

/// Per-room vertical settings that reach the 3D platforms (R-23, R-24, R-33):
/// the room name entry whose anchor lies in the room supplies a floor height
/// offset and a ceiling height override; rooms without a named entry (or
/// without an override) keep the floor's defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RoomLevels {
    /// Raise of the room's floor above the floor datum, inches.
    pub floor_offset: f64,
    /// Ceiling height measured from the room's own floor, inches.
    pub ceiling_height: f64,
}

/// The levels of `room` on `floor`.
pub(crate) fn room_levels(floor: &plan_core::Floor, room: &plan_core::Room) -> RoomLevels {
    let named = floor
        .room_names
        .iter()
        .find(|n| plan_core::geometry::point_in_polygon(n.anchor, &room.polygon));
    RoomLevels {
        floor_offset: named.map_or(0.0, |n| n.floor_height_offset),
        ceiling_height: named
            .and_then(|n| n.ceiling_height)
            .unwrap_or(floor.ceiling_height),
    }
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
