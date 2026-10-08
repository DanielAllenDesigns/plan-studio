//! Material hatches for elevation and section boxes.
//!
//! Instead of rebuilding wall-face polygons from the elevation's line work
//! (outline loops are unreliable once windows, doors and trim split them),
//! the hatch is derived from the 3D scene itself: every triangle of a
//! hatchable mesh that faces the camera and lies in a plane parallel to the
//! view is projected, its mesh's `plan_materials` pattern is generated over the
//! bounding rectangle of the coplanar triangles, and the pattern strokes are
//! clipped to each triangle. The triangles tile the wall face (openings are
//! holes in the wall mesh), so the union of the clipped strokes fills exactly
//! the visible wall-face region.

use plan_3d::{Material, Scene};
use plan_core::Point;
use plan_elevation::{Projection, ViewDir};
use plan_materials::{clip_strokes_to_polygon, pattern_strokes, Pattern};
use std::collections::BTreeMap;

/// Upper bound on hatch strokes per view, to keep PDFs small.
pub const MAX_HATCH_STROKES: usize = 40_000;

/// Depth spread (inches) below which a triangle counts as parallel to the view.
const PLANE_TOL_IN: f64 = 0.05;
/// Smallest projected triangle area (square inches) worth hatching.
const MIN_AREA_SQ_IN: f64 = 0.5;

/// One hatch segment in drawing space (inches of the building).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HatchStroke {
    pub a: Point,
    pub b: Point,
    /// The scene material whose pattern produced the stroke.
    pub material: Material,
}

/// The elevation hatch pattern of a scene material (`None` = not hatched).
pub fn pattern_for(material: Material) -> Option<Pattern> {
    match material {
        Material::WallExterior | Material::Siding => Some(Pattern::lap_siding()),
        Material::Brick => Some(Pattern::brick()),
        Material::Stone => Some(Pattern::block()),
        Material::Stucco | Material::Concrete => Some(Pattern::Concrete),
        _ => None,
    }
}

fn area2(p: &[Point; 3]) -> f64 {
    (p[1] - p[0]).cross(p[2] - p[0])
}

/// Material hatch strokes for the wall faces seen from `dir`.
///
/// `cut_offset` is a section plane (scene coordinate along the view axis, as
/// in [`plan_elevation::SectionCut`]): triangles in front of the plane, which
/// the section removes, are skipped. `scale_in_per_ft` is the drawing scale
/// (paper inches per foot); it coarsens patterns that would be denser than
/// 1/32" on paper.
///
/// Limits: only camera-facing triangles parallel to the view plane are
/// hatched (sloped roofs and gables are not); occlusion by nearer geometry is
/// not tested, so a hatched face hidden behind a porch or wing still shows
/// its hatch; cut faces of a section get no poche; pattern origin is the
/// lower-left of each face, so courses do not line up between walls.
pub fn wall_face_hatch(
    scene: &Scene,
    dir: ViewDir,
    cut_offset: Option<f64>,
    scale_in_per_ft: f64,
) -> Vec<HatchStroke> {
    let Some(bounds) = scene.bounds() else {
        return Vec::new();
    };
    let proj = Projection::for_view(dir, bounds);
    let cut_depth = cut_offset.map(|o| proj.depth_of_offset(o));
    let mut out = Vec::new();
    for mesh in &scene.meshes {
        let Some(pattern) = pattern_for(mesh.material) else {
            continue;
        };
        // Coplanar camera-facing triangles, grouped by depth (0.1" buckets).
        let mut groups: BTreeMap<i64, Vec<[Point; 3]>> = BTreeMap::new();
        for tri in mesh.indices.as_chunks::<3>().0 {
            let mut pts = [Point::ZERO; 3];
            let mut depths = [0.0; 3];
            let mut ok = true;
            for (k, &i) in tri.iter().enumerate() {
                let Some(v) = mesh.vertices.get(i as usize) else {
                    ok = false;
                    break;
                };
                (pts[k], depths[k]) = proj.project(v.position.map(f64::from));
            }
            if !ok {
                continue;
            }
            let (dmin, dmax) = depths
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &d| {
                    (lo.min(d), hi.max(d))
                });
            if dmax - dmin > PLANE_TOL_IN || area2(&pts) < MIN_AREA_SQ_IN * 2.0 {
                continue;
            }
            if cut_depth.is_some_and(|c| dmax > c + 1e-3) {
                continue;
            }
            groups
                .entry((dmax * 10.0).round() as i64)
                .or_default()
                .push(pts);
        }
        for tris in groups.values() {
            let (mut lo, mut hi) = (tris[0][0], tris[0][0]);
            for p in tris.iter().flatten() {
                lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
            }
            let strokes = pattern_strokes(&pattern, (lo, hi), scale_in_per_ft);
            for tri in tris {
                let (tlo, thi) = tri.iter().fold((tri[0], tri[0]), |(l, h), p| {
                    (
                        Point::new(l.x.min(p.x), l.y.min(p.y)),
                        Point::new(h.x.max(p.x), h.y.max(p.y)),
                    )
                });
                let near: Vec<(Point, Point)> = strokes
                    .iter()
                    .copied()
                    .filter(|(a, b)| {
                        a.x.max(b.x) >= tlo.x
                            && a.x.min(b.x) <= thi.x
                            && a.y.max(b.y) >= tlo.y
                            && a.y.min(b.y) <= thi.y
                    })
                    .collect();
                for (a, b) in clip_strokes_to_polygon(&near, tri) {
                    out.push(HatchStroke {
                        a,
                        b,
                        material: mesh.material,
                    });
                }
                if out.len() >= MAX_HATCH_STROKES {
                    out.truncate(MAX_HATCH_STROKES);
                    return out;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_3d::{Mesh, Vertex};

    fn quad_mesh(material: Material, z: f32) -> Mesh {
        // A 100" x 100" wall face at depth z, wound counter-clockwise seen
        // from +Z (the Front camera).
        let v = |x: f32, y: f32| Vertex {
            position: [x, y, z],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
        };
        Mesh {
            vertices: vec![v(0.0, 0.0), v(100.0, 0.0), v(100.0, 100.0), v(0.0, 100.0)],
            indices: vec![0, 1, 2, 0, 2, 3],
            material,
            object_id: None,
            color: None,
        }
    }

    #[test]
    fn front_face_is_hatched_back_face_is_not() {
        let scene = Scene {
            meshes: vec![quad_mesh(Material::Siding, 0.0)],
        };
        let s = wall_face_hatch(&scene, ViewDir::Front, None, 0.25);
        assert!(s.len() > 5, "{}", s.len());
        for h in &s {
            for p in [h.a, h.b] {
                assert!((-1e-6..=100.0 + 1e-6).contains(&p.x));
                assert!((-1e-6..=100.0 + 1e-6).contains(&p.y));
            }
        }
        // Seen from behind the face is back-facing.
        assert!(wall_face_hatch(&scene, ViewDir::Back, None, 0.25).is_empty());
        // Unhatched materials give nothing.
        let plain = Scene {
            meshes: vec![quad_mesh(Material::WallInterior, 0.0)],
        };
        assert!(wall_face_hatch(&plain, ViewDir::Front, None, 0.25).is_empty());
    }

    #[test]
    fn faces_in_front_of_a_section_plane_are_skipped() {
        let scene = Scene {
            meshes: vec![quad_mesh(Material::Brick, 10.0)],
        };
        assert!(!wall_face_hatch(&scene, ViewDir::Front, Some(20.0), 0.25).is_empty());
        assert!(wall_face_hatch(&scene, ViewDir::Front, Some(5.0), 0.25).is_empty());
    }
}
