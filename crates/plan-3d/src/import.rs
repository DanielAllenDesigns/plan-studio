//! Importing external triangle meshes (for example Chief library objects).
//!
//! Three steps, all pure functions on [`Mesh`]:
//!
//! 1. [`mesh_from_triangles`] validates raw buffers and builds a [`Mesh`].
//! 2. [`fit_mesh_to_box`] scales the mesh to a placed size and moves it into
//!    the symbol-local frame described below.
//! 3. [`transform_mesh`] rotates, mirrors and moves it into the scene.
//!
//! # Frames
//!
//! Scene space is X = plan x, Y = up, Z = -plan y (see the crate docs).
//! Imported buffers are expected in the **natural import frame**: a Chief
//! object (X right, Y towards the back, Z up, front towards -Y) mapped with
//! `(x, y, z) -> (x, z, -y)`. That is a proper rotation, so winding is
//! preserved, and the object's **front faces +Z** and its back faces -Z.
//!
//! [`fit_mesh_to_box`] outputs the **symbol-local frame**: bounding-box center
//! at `x = 0`, bottom at `y = 0` and back face at `z = 0`, so the footprint is
//! `x` in `[-width/2, width/2]` and `z` in `[0, depth]` with the front at
//! `z = depth`. The origin is the symbol's back-center, matching
//! `plan_core::PlacedSymbol::position`.
//!
//! A `PlacedSymbol` has its front towards plan +Y at angle 0, which is scene
//! -Z. To place a symbol-local mesh, [`transform_mesh`] it with
//! `yaw_rad = (angle_deg + 180).to_radians()`, `mirror_x = flip` and
//! `origin = [position.x, floor_elevation + elevation, -position.y]`.

use crate::mesh::{Material, Mesh, Vertex};
use plan_core::Id;

/// Triangles with an area below this (square inches) are dropped.
const MIN_AREA: f32 = 1e-9;

type V3 = [f32; 3];

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

fn len(a: V3) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// Unit vector, or `None` when `a` is zero or not finite.
fn normalized(a: V3) -> Option<V3> {
    let l = len(a);
    (l.is_finite() && l > 1e-20).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

/// Planar UV (feet) from the dominant axis of the face normal.
fn planar_uv(p: V3, n: V3) -> [f32; 2] {
    let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
    let (u, v) = if ay >= ax && ay >= az {
        (p[0], p[2])
    } else if ax >= az {
        (p[2], p[1])
    } else {
        (p[0], p[1])
    };
    [u / 12.0, v / 12.0]
}

/// Builds a [`Mesh`] from raw triangle buffers.
///
/// * Triangles with an out-of-range index, a non-finite position or (almost)
///   zero area are dropped; nothing panics on bad input.
/// * With `normals == None` (or a normals slice of the wrong length) every
///   triangle gets its own three vertices and a flat face normal.
/// * With per-vertex normals, vertices are shared as given; each normal is
///   normalized, and a zero or non-finite one falls back to the face normal.
/// * UVs are a planar projection in feet.
pub fn mesh_from_triangles(
    positions: &[[f32; 3]],
    normals: Option<&[[f32; 3]]>,
    indices: &[u32],
    material: Material,
    object_id: Option<Id>,
) -> Mesh {
    let normals = normals.filter(|n| n.len() == positions.len());
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut out: Vec<u32> = Vec::new();
    // Source vertex -> output vertex, used only with per-vertex normals.
    let mut remap: Vec<u32> = vec![
        u32::MAX;
        if normals.is_some() {
            positions.len()
        } else {
            0
        }
    ];

    for tri in indices.as_chunks::<3>().0 {
        if tri.iter().any(|&i| i as usize >= positions.len()) {
            continue;
        }
        let p = [
            positions[tri[0] as usize],
            positions[tri[1] as usize],
            positions[tri[2] as usize],
        ];
        if p.iter().flatten().any(|c| !c.is_finite()) {
            continue;
        }
        let c = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        if len(c) * 0.5 < MIN_AREA {
            continue;
        }
        let Some(face) = normalized(c) else { continue };
        match normals {
            None => {
                let base = vertices.len() as u32;
                for q in p {
                    vertices.push(Vertex {
                        position: q,
                        normal: face,
                        uv: planar_uv(q, face),
                    });
                }
                out.extend([base, base + 1, base + 2]);
            }
            Some(ns) => {
                for &i in tri {
                    let slot = &mut remap[i as usize];
                    if *slot == u32::MAX {
                        let q = positions[i as usize];
                        let normal = normalized(ns[i as usize]).unwrap_or(face);
                        *slot = vertices.len() as u32;
                        vertices.push(Vertex {
                            position: q,
                            normal,
                            uv: planar_uv(q, normal),
                        });
                    }
                    out.push(*slot);
                }
            }
        }
    }
    Mesh {
        vertices,
        indices: out,
        material,
        object_id,
        color: None,
    }
}

/// Scales, mirrors, rotates and translates `mesh`, in that order.
///
/// * `scale` multiplies each axis about the mesh origin.
/// * `mirror_x` negates X after scaling (a left-right flip).
/// * `yaw_rad` rotates about the +Y axis, right-handed: `x' = x cos + z sin`,
///   `z' = -x sin + z cos`. This is the same as a counter-clockwise plan
///   rotation by `yaw_rad`, because scene Z = -plan y.
/// * `origin` is added last.
///
/// Normals use the inverse-transpose, so they stay perpendicular under
/// non-uniform scale, and are renormalized. The winding is reversed when the
/// transform has a negative determinant (mirror, or an odd number of negative
/// scale components) so faces keep pointing out.
pub fn transform_mesh(
    mesh: &Mesh,
    origin: [f32; 3],
    yaw_rad: f32,
    scale: [f32; 3],
    mirror_x: bool,
) -> Mesh {
    let (s, c) = yaw_rad.sin_cos();
    let rot = |v: V3| [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c];
    let mx = if mirror_x { -1.0 } else { 1.0 };
    let det = scale[0] * scale[1] * scale[2] * mx;
    let inv = |k: f32| if k.abs() > 1e-12 { 1.0 / k } else { 1.0 };
    let vertices = mesh
        .vertices
        .iter()
        .map(|v| {
            let p = rot([
                v.position[0] * scale[0] * mx,
                v.position[1] * scale[1],
                v.position[2] * scale[2],
            ]);
            let n = rot([
                v.normal[0] * inv(scale[0]) * mx,
                v.normal[1] * inv(scale[1]),
                v.normal[2] * inv(scale[2]),
            ]);
            let n = normalized(n).unwrap_or(v.normal);
            Vertex {
                position: [p[0] + origin[0], p[1] + origin[1], p[2] + origin[2]],
                normal: n,
                uv: v.uv,
            }
        })
        .collect();
    let mut indices = mesh.indices.clone();
    if det < 0.0 {
        for t in indices.as_chunks_mut::<3>().0 {
            t.swap(1, 2);
        }
    }
    Mesh {
        vertices,
        indices,
        material: mesh.material,
        object_id: mesh.object_id,
        color: mesh.color,
    }
}

/// Scales `mesh` per axis so its bounds match `width` (X), `height` (Y) and
/// `depth` (Z), then moves it into the symbol-local frame: bounds center at
/// `x = 0`, bottom at `y = 0`, back face at `z = 0` (front at `z = depth`).
/// See the module docs for the frame conventions.
///
/// An axis with no extent (a flat mesh) or a non-positive target keeps scale 1.
/// An empty mesh is returned unchanged.
pub fn fit_mesh_to_box(mesh: &Mesh, width: f32, depth: f32, height: f32) -> Mesh {
    fit_meshes_to_box(std::slice::from_ref(mesh), width, depth, height)
        .pop()
        .unwrap_or_else(|| mesh.clone())
}

/// [`fit_mesh_to_box`] for a multi-part object: the scale and shift come from
/// the **union** bounds of `meshes`, so the parts stay aligned with each other.
pub fn fit_meshes_to_box(meshes: &[Mesh], width: f32, depth: f32, height: f32) -> Vec<Mesh> {
    let mut acc: Option<([f32; 3], [f32; 3])> = None;
    for (l, h) in meshes.iter().filter_map(Mesh::bounds) {
        let (lo, hi) = acc.get_or_insert((l, h));
        for k in 0..3 {
            lo[k] = lo[k].min(l[k]);
            hi[k] = hi[k].max(h[k]);
        }
    }
    let Some((lo, hi)) = acc else {
        return meshes.to_vec();
    };
    let factor = |target: f32, extent: f32| {
        if target > 0.0 && target.is_finite() && extent > 1e-6 {
            target / extent
        } else {
            1.0
        }
    };
    let k = [
        factor(width, hi[0] - lo[0]),
        factor(height, hi[1] - lo[1]),
        factor(depth, hi[2] - lo[2]),
    ];
    let shift = [-(lo[0] + hi[0]) * 0.5 * k[0], -lo[1] * k[1], -lo[2] * k[2]];
    meshes
        .iter()
        .map(|m| transform_mesh(m, shift, 0.0, k, false))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(size: f32) -> (Vec<[f32; 3]>, Vec<u32>) {
        let s = size;
        let p = vec![
            [0.0, 0.0, 0.0],
            [s, 0.0, 0.0],
            [s, s, 0.0],
            [0.0, s, 0.0],
            [0.0, 0.0, s],
            [s, 0.0, s],
            [s, s, s],
            [0.0, s, s],
        ];
        // Outward-facing (counter-clockwise seen from outside).
        let i = vec![
            0, 2, 1, 0, 3, 2, // z = 0
            4, 5, 6, 4, 6, 7, // z = s
            0, 1, 5, 0, 5, 4, // y = 0
            3, 7, 6, 3, 6, 2, // y = s
            0, 4, 7, 0, 7, 3, // x = 0
            1, 2, 6, 1, 6, 5, // x = s
        ];
        (p, i)
    }

    fn mat() -> Material {
        Material::Concrete
    }

    /// Signed volume; positive for outward-facing closed meshes.
    fn volume(m: &Mesh) -> f32 {
        m.indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                let [a, b, c] = [0, 1, 2].map(|k| m.vertices[t[k] as usize].position);
                (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0
            })
            .sum()
    }

    fn assert_valid(m: &Mesh) {
        assert_eq!(m.indices.len() % 3, 0);
        for &i in &m.indices {
            assert!((i as usize) < m.vertices.len());
        }
        for v in &m.vertices {
            assert!((len(v.normal) - 1.0).abs() < 1e-4, "{:?}", v.normal);
        }
    }

    #[test]
    fn flat_normals_roundtrip() {
        let (p, i) = cube(10.0);
        let m = mesh_from_triangles(&p, None, &i, mat(), Some(7));
        assert_eq!(m.triangle_count(), 12);
        assert_eq!(m.vertices.len(), 36);
        assert_eq!(m.object_id, Some(7));
        assert_valid(&m);
        assert!(volume(&m) > 999.0 && volume(&m) < 1001.0);
        // The z = 0 face points down -Z.
        assert_eq!(m.vertices[0].normal, [0.0, 0.0, -1.0]);
        let (lo, hi) = m.bounds().unwrap();
        assert_eq!((lo, hi), ([0.0; 3], [10.0; 3]));
    }

    #[test]
    fn drops_degenerate_and_invalid() {
        let p = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 0.0, 0.0],
            [f32::NAN, 0.0, 0.0],
        ];
        let i = vec![
            0, 1, 2, // good
            0, 1, 3, // collinear
            0, 0, 1, // repeated index
            0, 1, 9, // out of range
            0, 1, 4, // NaN
            0, 1, // dangling
        ];
        let m = mesh_from_triangles(&p, None, &i, mat(), None);
        assert_eq!(m.triangle_count(), 1);
        assert_valid(&m);
        assert!(mesh_from_triangles(&[], None, &[], mat(), None)
            .bounds()
            .is_none());
    }

    #[test]
    fn supplied_normals_are_normalized_and_shared() {
        let (p, i) = cube(10.0);
        let mut n: Vec<[f32; 3]> = p
            .iter()
            .map(|q| [q[0] - 5.0, q[1] - 5.0, q[2] - 5.0])
            .collect();
        n[3] = [0.0; 3]; // falls back to the face normal
        let m = mesh_from_triangles(&p, Some(&n), &i, mat(), None);
        assert_eq!(m.triangle_count(), 12);
        assert_eq!(m.vertices.len(), 8);
        assert_valid(&m);
        // A wrong-length normals slice is ignored.
        let m = mesh_from_triangles(&p, Some(&n[..3]), &i, mat(), None);
        assert_eq!(m.vertices.len(), 36);
    }

    #[test]
    fn transform_rotates_a_point() {
        let m = Mesh {
            vertices: vec![Vertex {
                position: [1.0, 2.0, 0.0],
                normal: [1.0, 0.0, 0.0],
                uv: [0.0; 2],
            }],
            indices: vec![],
            material: mat(),
            object_id: None,
            color: None,
        };
        // +90 degrees yaw (counter-clockwise in plan): +X goes to plan +Y = scene -Z.
        let t = transform_mesh(
            &m,
            [10.0, 20.0, 30.0],
            std::f32::consts::FRAC_PI_2,
            [1.0; 3],
            false,
        );
        let p = t.vertices[0].position;
        assert!((p[0] - 10.0).abs() < 1e-4, "{p:?}");
        assert!((p[1] - 22.0).abs() < 1e-4);
        assert!((p[2] - 29.0).abs() < 1e-4);
        let n = t.vertices[0].normal;
        assert!(n[0].abs() < 1e-4 && (n[2] + 1.0).abs() < 1e-4, "{n:?}");
    }

    #[test]
    fn mirror_flips_winding_and_normals_stay_outward() {
        let (p, i) = cube(10.0);
        let m = mesh_from_triangles(&p, None, &i, mat(), None);
        let t = transform_mesh(&m, [0.0; 3], 0.0, [1.0; 3], true);
        assert_valid(&t);
        assert!(
            volume(&t) > 999.0,
            "winding must stay outward: {}",
            volume(&t)
        );
        let (lo, hi) = t.bounds().unwrap();
        assert_eq!((lo[0], hi[0]), (-10.0, 0.0));
        // Face normals still agree with the geometric winding.
        for tri in t.indices.as_chunks::<3>().0 {
            let v = |k: usize| t.vertices[tri[k] as usize];
            let c = cross(
                sub(v(1).position, v(0).position),
                sub(v(2).position, v(0).position),
            );
            let d: f32 = (0..3).map(|a| c[a] * v(0).normal[a]).sum();
            assert!(d > 0.0);
        }
    }

    #[test]
    fn non_uniform_scale_keeps_normals_perpendicular() {
        let (p, i) = cube(10.0);
        let m = mesh_from_triangles(&p, None, &i, mat(), None);
        let t = transform_mesh(&m, [0.0; 3], 0.3, [2.0, 0.5, 3.0], false);
        assert_valid(&t);
        for tri in t.indices.as_chunks::<3>().0 {
            let v = |k: usize| t.vertices[tri[k] as usize];
            let c = normalized(cross(
                sub(v(1).position, v(0).position),
                sub(v(2).position, v(0).position),
            ))
            .unwrap();
            let d: f32 = (0..3).map(|a| c[a] * v(0).normal[a]).sum();
            assert!(d > 0.999, "{d}");
        }
    }

    #[test]
    fn fit_scales_cube_and_anchors_back_center() {
        let (p, i) = cube(10.0);
        let m = mesh_from_triangles(&p, None, &i, mat(), None);
        let f = fit_mesh_to_box(&m, 24.0, 30.0, 36.0);
        assert_valid(&f);
        let (lo, hi) = f.bounds().unwrap();
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(close(lo[0], -12.0) && close(hi[0], 12.0), "{lo:?} {hi:?}");
        assert!(close(lo[1], 0.0) && close(hi[1], 36.0));
        assert!(close(lo[2], 0.0) && close(hi[2], 30.0));
        assert!(volume(&f) > 0.0);
    }

    #[test]
    fn fit_multi_part_uses_union_bounds() {
        let (p, i) = cube(10.0);
        let a = mesh_from_triangles(&p, None, &i, mat(), None);
        let b = transform_mesh(&a, [10.0, 0.0, 0.0], 0.0, [1.0; 3], false);
        let f = fit_meshes_to_box(&[a, b], 40.0, 20.0, 20.0);
        let (lo0, hi0) = f[0].bounds().unwrap();
        let (lo1, hi1) = f[1].bounds().unwrap();
        assert!(
            (lo0[0] + 20.0).abs() < 1e-4 && hi0[0].abs() < 1e-4,
            "{lo0:?} {hi0:?}"
        );
        assert!(lo1[0].abs() < 1e-4 && (hi1[0] - 20.0).abs() < 1e-4);
        assert!((hi0[1] - 20.0).abs() < 1e-4 && (hi1[2] - 20.0).abs() < 1e-4);
    }

    #[test]
    fn fit_handles_flat_and_empty() {
        let p = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 0.0, 5.0]];
        let m = mesh_from_triangles(&p, None, &[0, 1, 2], mat(), None);
        let f = fit_mesh_to_box(&m, 20.0, 10.0, 99.0);
        let (lo, hi) = f.bounds().unwrap();
        assert_eq!((lo[1], hi[1]), (0.0, 0.0));
        assert!((hi[0] - 10.0).abs() < 1e-4 && (hi[2] - 10.0).abs() < 1e-4);
        let empty = Mesh {
            vertices: vec![],
            indices: vec![],
            material: mat(),
            object_id: None,
            color: None,
        };
        assert!(fit_mesh_to_box(&empty, 1.0, 1.0, 1.0).vertices.is_empty());
    }
}
