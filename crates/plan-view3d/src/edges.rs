//! CPU-side edge extraction for the optional line overlay.

use plan_3d::Vertex;
use std::collections::BTreeMap;

use crate::math::{self, Vec3};

/// Positions closer than 1/1000 inch are treated as the same point.
const WELD_SCALE: f32 = 1000.0;
/// Faces whose normals are closer than this many degrees count as coplanar.
const CREASE_DEGREES: f32 = 30.0;

type Key = [i64; 3];

fn key(p: Vec3) -> Key {
    [
        (p[0] * WELD_SCALE).round() as i64,
        (p[1] * WELD_SCALE).round() as i64,
        (p[2] * WELD_SCALE).round() as i64,
    ]
}

struct Edge {
    a: Vec3,
    b: Vec3,
    /// Number of triangles sharing this edge.
    faces: u32,
    /// Normal of the first triangle seen.
    first_normal: Vec3,
    /// True once a second triangle meets the first at a visible angle.
    crease: bool,
}

/// Extract the visible edges of an indexed triangle mesh.
///
/// Vertices are welded by position (meshes often duplicate vertices per face),
/// and an edge is kept when it lies on the open boundary, is shared by more
/// than two triangles, or joins two triangles whose normals differ by more
/// than 30 degrees. Diagonals inside a flat face are dropped, so a single quad
/// yields exactly its four sides. Output order is deterministic.
pub fn unique_edges(vertices: &[Vertex], indices: &[u32]) -> Vec<(Vec3, Vec3)> {
    let cos_crease = CREASE_DEGREES.to_radians().cos();
    let mut edges: BTreeMap<(Key, Key), Edge> = BTreeMap::new();

    let (triangles, _) = indices.as_chunks::<3>();
    for tri in triangles {
        let Some(corners) = tri
            .iter()
            .map(|&i| vertices.get(i as usize).map(|v| v.position))
            .collect::<Option<Vec<Vec3>>>()
        else {
            continue;
        };
        let normal = math::cross(
            math::sub(corners[1], corners[0]),
            math::sub(corners[2], corners[0]),
        );
        if math::length(normal) < 1e-9 {
            continue; // degenerate triangle
        }
        let normal = math::normalize(normal);
        for k in 0..3 {
            let (a, b) = (corners[k], corners[(k + 1) % 3]);
            let (ka, kb) = (key(a), key(b));
            if ka == kb {
                continue;
            }
            let (lo, hi, pa, pb) = if ka < kb {
                (ka, kb, a, b)
            } else {
                (kb, ka, b, a)
            };
            edges
                .entry((lo, hi))
                .and_modify(|e| {
                    e.faces += 1;
                    if e.faces == 2 && math::dot(e.first_normal, normal) < cos_crease {
                        e.crease = true;
                    }
                })
                .or_insert(Edge {
                    a: pa,
                    b: pb,
                    faces: 1,
                    first_normal: normal,
                    crease: false,
                });
        }
    }

    edges
        .into_values()
        .filter(|e| e.faces == 1 || e.faces > 2 || e.crease)
        .map(|e| (e.a, e.b))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32, z: f32) -> Vertex {
        Vertex {
            position: [x, y, z],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        }
    }

    #[test]
    fn single_quad_has_four_edges() {
        let verts = [
            v(0.0, 0.0, 0.0),
            v(10.0, 0.0, 0.0),
            v(10.0, 0.0, -5.0),
            v(0.0, 0.0, -5.0),
        ];
        let edges = unique_edges(&verts, &[0, 1, 2, 0, 2, 3]);
        assert_eq!(edges.len(), 4, "the flat diagonal must be dropped");
    }

    #[test]
    fn box_with_per_face_vertices_has_twelve_edges() {
        // Build a unit cube from six independent quads (vertices not shared).
        let faces: [[[f32; 3]; 4]; 6] = [
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
            [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
            [[1., 0., 0.], [1., 0., 1.], [1., 1., 1.], [1., 1., 0.]],
            [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
            [[0., 1., 0.], [1., 1., 0.], [1., 1., 1.], [0., 1., 1.]],
        ];
        let mut verts = Vec::new();
        let mut idx = Vec::new();
        for f in faces {
            let base = verts.len() as u32;
            verts.extend(f.iter().map(|p| v(p[0], p[1], p[2])));
            idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        assert_eq!(unique_edges(&verts, &idx).len(), 12);
    }

    #[test]
    fn invalid_indices_are_skipped() {
        let verts = [v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)];
        assert_eq!(unique_edges(&verts, &[0, 1, 9]).len(), 0);
        assert_eq!(unique_edges(&verts, &[0, 1, 2]).len(), 3);
    }
}
