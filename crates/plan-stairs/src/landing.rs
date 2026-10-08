//! Polygon slabs: landings of any outline (convex or not) as one mesh.
//!
//! The outline is triangulated by ear clipping in plan; the mesh has a top
//! face, a bottom face and one quad per edge. Scene space is X right, Y up,
//! Z = -plan y (see `plan-3d`).

use plan_3d::{Material, Mesh, Vertex};
use plan_core::{Id, Point};

/// Twice the signed area of the outline (positive when counter-clockwise in plan).
fn signed_area2(poly: &[Point]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum()
}

fn in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let d1 = (b - a).cross(p - a);
    let d2 = (c - b).cross(p - b);
    let d3 = (a - c).cross(p - c);
    d1 >= -1e-9 && d2 >= -1e-9 && d3 >= -1e-9
}

/// Triangles (indices into `poly`) covering a simple polygon, each
/// counter-clockwise in plan. Empty for fewer than three corners.
pub(crate) fn triangulate(poly: &[Point]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let mut idx: Vec<usize> = (0..n).collect();
    if signed_area2(poly) < 0.0 {
        idx.reverse();
    }
    let mut out = Vec::with_capacity(n - 2);
    while idx.len() > 3 {
        let m = idx.len();
        let ear = (0..m).find(|&i| {
            let (a, b, c) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (pa, pb, pc) = (poly[a], poly[b], poly[c]);
            if (pb - pa).cross(pc - pb) <= 1e-9 {
                return false;
            }
            !idx.iter()
                .filter(|&&k| k != a && k != b && k != c)
                .any(|&k| in_triangle(poly[k], pa, pb, pc))
        });
        // A degenerate outline has no ear: clip the first corner anyway.
        let i = ear.unwrap_or(0);
        out.push([idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]]);
        idx.remove(i);
    }
    out.push([idx[0], idx[1], idx[2]]);
    out
}

fn scene(p: Point, y: f64) -> [f32; 3] {
    [p.x as f32, y as f32, -p.y as f32]
}

/// Appends a triangle facing `hint` (a scene-space normal).
fn push_tri(
    mesh: &mut Mesh,
    pts: [[f32; 3]; 3],
    hint: [f32; 3],
    uv: impl Fn([f32; 3]) -> [f32; 2],
) {
    let e1 = [
        pts[1][0] - pts[0][0],
        pts[1][1] - pts[0][1],
        pts[1][2] - pts[0][2],
    ];
    let e2 = [
        pts[2][0] - pts[0][0],
        pts[2][1] - pts[0][1],
        pts[2][2] - pts[0][2],
    ];
    let n = [
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ];
    if n[0] * n[0] + n[1] * n[1] + n[2] * n[2] < 1e-12 {
        return;
    }
    let flip = n[0] * hint[0] + n[1] * hint[1] + n[2] * hint[2] < 0.0;
    let order: [usize; 3] = if flip { [0, 2, 1] } else { [0, 1, 2] };
    let base = mesh.vertices.len() as u32;
    for k in order {
        mesh.vertices.push(Vertex {
            position: pts[k],
            normal: hint,
            uv: uv(pts[k]),
        });
    }
    mesh.indices.extend([base, base + 1, base + 2]);
}

/// A slab with the plan outline `poly`, from height `bottom` up to `top`.
///
/// `None` when the outline has fewer than three corners or no area.
pub fn polygon_slab(
    poly: &[Point],
    bottom: f64,
    top: f64,
    material: Material,
    id: Option<Id>,
) -> Option<Mesh> {
    let tris = triangulate(poly);
    if tris.is_empty() || signed_area2(poly).abs() < 1e-9 || top <= bottom {
        return None;
    }
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        material,
        object_id: id,
        color: None,
    };
    let flat = |p: [f32; 3]| [p[0] / 12.0, p[2] / 12.0];
    for [a, b, c] in tris {
        push_tri(
            &mut mesh,
            [
                scene(poly[a], top),
                scene(poly[b], top),
                scene(poly[c], top),
            ],
            [0.0, 1.0, 0.0],
            flat,
        );
        push_tri(
            &mut mesh,
            [
                scene(poly[a], bottom),
                scene(poly[b], bottom),
                scene(poly[c], bottom),
            ],
            [0.0, -1.0, 0.0],
            flat,
        );
    }
    let ccw = signed_area2(poly) > 0.0;
    let n = poly.len();
    let mut run = 0.0f32;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let d = b - a;
        let len = d.length();
        if len < 1e-9 {
            continue;
        }
        // Outward normal in plan: to the right of a counter-clockwise edge.
        let (nx, ny) = if ccw {
            (d.y / len, -d.x / len)
        } else {
            (-d.y / len, d.x / len)
        };
        let hint = [nx as f32, 0.0, -ny as f32];
        let (pa, pb) = ((scene(a, bottom)), scene(b, bottom));
        let (qa, qb) = (scene(a, top), scene(b, top));
        let r0 = run;
        let uv = move |p: [f32; 3]| {
            let along = if (p[0] - pa[0]).abs() + (p[2] - pa[2]).abs() < 1e-6 {
                0.0
            } else {
                len as f32
            };
            [(r0 + along) / 12.0, p[1] / 12.0]
        };
        push_tri(&mut mesh, [pa, pb, qb], hint, uv);
        push_tri(&mut mesh, [pa, qb, qa], hint, uv);
        run += len as f32;
    }
    Some(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area_of(mesh: &Mesh, up: bool) -> f64 {
        let mut a = 0.0;
        for t in mesh.indices.chunks(3) {
            let p: Vec<_> = t.iter().map(|&i| mesh.vertices[i as usize]).collect();
            if (p[0].normal[1] > 0.5) == up && p[0].normal[1].abs() > 0.5 {
                let e1 = [
                    p[1].position[0] - p[0].position[0],
                    p[1].position[2] - p[0].position[2],
                ];
                let e2 = [
                    p[2].position[0] - p[0].position[0],
                    p[2].position[2] - p[0].position[2],
                ];
                a += f64::from((e1[0] * e2[1] - e1[1] * e2[0]).abs()) / 2.0;
            }
        }
        a
    }

    #[test]
    fn an_l_shaped_outline_triangulates_to_its_area() {
        // 60 x 60 square minus a 30 x 30 corner: 2700 sq in.
        let l = [
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(60.0, 30.0),
            Point::new(30.0, 30.0),
            Point::new(30.0, 60.0),
            Point::new(0.0, 60.0),
        ];
        let tris = triangulate(&l);
        assert_eq!(tris.len(), 4);
        let area: f64 = tris
            .iter()
            .map(|t| (l[t[1]] - l[t[0]]).cross(l[t[2]] - l[t[0]]) / 2.0)
            .sum();
        assert!((area - 2700.0).abs() < 1e-9, "{area}");
        // Clockwise gives the same result.
        let mut cw = l;
        cw.reverse();
        let area: f64 = triangulate(&cw)
            .iter()
            .map(|t| (cw[t[1]] - cw[t[0]]).cross(cw[t[2]] - cw[t[0]]) / 2.0)
            .sum();
        assert!((area - 2700.0).abs() < 1e-9);
    }

    #[test]
    fn the_slab_has_matching_top_and_bottom_faces() {
        let l = [
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(60.0, 30.0),
            Point::new(30.0, 30.0),
            Point::new(30.0, 60.0),
            Point::new(0.0, 60.0),
        ];
        let m = polygon_slab(&l, 10.0, 13.5, Material::Floor, Some(3)).unwrap();
        assert!((area_of(&m, true) - 2700.0).abs() < 1e-3);
        assert!((area_of(&m, false) - 2700.0).abs() < 1e-3);
        let (lo, hi) = m.bounds().unwrap();
        assert!((f64::from(lo[1]) - 10.0).abs() < 1e-6 && (f64::from(hi[1]) - 13.5).abs() < 1e-6);
        // Every normal agrees with its triangle's winding.
        for t in m.indices.chunks(3) {
            let p: Vec<_> = t.iter().map(|&i| m.vertices[i as usize]).collect();
            let e1: Vec<f32> = (0..3)
                .map(|k| p[1].position[k] - p[0].position[k])
                .collect();
            let e2: Vec<f32> = (0..3)
                .map(|k| p[2].position[k] - p[0].position[k])
                .collect();
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let d = n[0] * p[0].normal[0] + n[1] * p[0].normal[1] + n[2] * p[0].normal[2];
            assert!(d > 0.0);
        }
        assert!(polygon_slab(&l[..2], 0.0, 1.0, Material::Floor, None).is_none());
    }
}
