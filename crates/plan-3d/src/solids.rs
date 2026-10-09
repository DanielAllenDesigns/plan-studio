//! 3D solids with the extra spec fields, and compound solids
//! (`plan_core::solids`): the meshes the 3D view adds for them.
//!
//! [`solid_meshes`] replaces the plain `details::solid_mesh` pass of a floor:
//! a primitive without an extra spec (or with one that does not change its
//! shape) keeps the smooth plain mesh; one that is tilted about X or Y, is a
//! truncated pyramid, or has its own surface quality is meshed from
//! [`plan_core::solids::solid_tris`]. Compound solids (the result of Union,
//! Subtract and Intersect) follow.

use crate::builder::{MeshBuilder, V3};
use crate::details::{material_of, solid_mesh};
use crate::mesh::{Material, Mesh};
use plan_core::details::Solid3d;
use plan_core::solids::{tri_normal, Tri};
use plan_core::{Floor, Id};

/// UV in feet from the face's dominant axis (box mapping).
fn uv_of(p: V3, n: [f64; 3]) -> [f32; 2] {
    let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
    // `p` is in scene space (x, up, -y); `n` is the plan-space normal.
    let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
    if az >= ax && az >= ay {
        [(x / 12.0) as f32, (-z / 12.0) as f32]
    } else if ax >= ay {
        [(-z / 12.0) as f32, (y / 12.0) as f32]
    } else {
        [(x / 12.0) as f32, (y / 12.0) as f32]
    }
}

/// A flat-shaded mesh of `tris` (plan coordinates, z above the floor) on a
/// floor at `floor_elev`, tagged with `id`.
pub fn tris_mesh(tris: &[Tri], floor_elev: f64, material: Material, id: Id) -> Option<Mesh> {
    let mut b = MeshBuilder::new(material);
    for t in tris {
        let n = tri_normal(t);
        if n == [0.0; 3] {
            continue;
        }
        let p: [V3; 3] = t.map(|v| [v[0] as f32, (floor_elev + v[2]) as f32, (-v[1]) as f32]);
        let normal: V3 = [n[0] as f32, n[2] as f32, (-n[1]) as f32];
        b.tri(p, p.map(|q| uv_of(q, n)), normal);
    }
    (!b.is_empty()).then(|| b.finish(Some(id)))
}

/// The mesh of one primitive with its extra spec.
pub fn solid_ext_mesh(floor: &Floor, s: &Solid3d, floor_elev: f64) -> Option<Mesh> {
    let ext = floor.solid_layer.ext_of(s.id);
    match ext {
        Some(e) if e.changes_mesh() => {
            let tris = plan_core::solids::solid_tris(s, Some(e));
            tris_mesh(
                &tris,
                floor_elev,
                material_of(&s.material, Material::Concrete),
                s.id,
            )
        }
        _ => solid_mesh(s, floor_elev),
    }
}

/// Every solid mesh of a floor: the primitives of its details layer, then
/// its compound solids.
pub fn solid_meshes(floor: &Floor, solids: &[Solid3d], floor_elev: f64) -> Vec<Mesh> {
    let mut out: Vec<Mesh> = solids
        .iter()
        .filter_map(|s| solid_ext_mesh(floor, s, floor_elev))
        .collect();
    for c in &floor.solid_layer.compounds {
        out.extend(tris_mesh(
            &c.tris,
            floor_elev,
            material_of(&c.material, Material::Concrete),
            c.id,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::details::{DetailsLayer, SolidKind};
    use plan_core::geometry::Point;
    use plan_core::solids::{boolean_floor, BoolOp, SolidExt};
    use plan_core::{ObjectRef, Project};

    fn bounds(m: &Mesh) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for v in &m.vertices {
            for k in 0..3 {
                lo[k] = lo[k].min(v.position[k]);
                hi[k] = hi[k].max(v.position[k]);
            }
        }
        (lo, hi)
    }

    #[test]
    fn a_tilted_box_and_a_union_become_meshes() {
        let mut p = Project::new("s");
        let (a, b) = (p.alloc_id(), p.alloc_id());
        let mut d = DetailsLayer::default();
        for (id, x) in [(a, 0.0), (b, 5.0)] {
            d.solids.push(Solid3d {
                id,
                kind: SolidKind::Box {
                    w: 10.0,
                    d: 10.0,
                    h: 30.0,
                },
                position: Point::new(x, 0.0),
                ..Solid3d::default()
            });
        }
        p.floors[0].set_details(&d).unwrap();
        let f = &p.floors[0];
        // Plain box: 30 tall.
        let plain = solid_meshes(f, &DetailsLayer::load(f).solids, 0.0);
        assert_eq!(plain.len(), 2);
        let (lo, hi) = bounds(&plain[0]);
        assert!((hi[1] - lo[1] - 30.0).abs() < 1e-4);
        // Tilt the first about X by 90: now 10 tall and 30 deep.
        let mut e = SolidExt::new(a);
        e.rot_x = 90.0;
        p.floors[0].solid_layer.set_ext(e);
        let f = &p.floors[0];
        let m = solid_meshes(f, &DetailsLayer::load(f).solids, 0.0);
        let first = m.iter().find(|m| m.object_id == Some(a)).unwrap();
        let (lo, hi) = bounds(first);
        assert!((hi[1] - lo[1] - 10.0).abs() < 1e-4, "{lo:?} {hi:?}");
        assert!((hi[2] - lo[2] - 30.0).abs() < 1e-4);
        // Union of the two: one compound mesh replaces both.
        let nid = p.alloc_id();
        p.floors[0].solid_layer.ext.clear();
        boolean_floor(
            &mut p.floors[0],
            BoolOp::Union,
            &[ObjectRef::Detail(a), ObjectRef::Detail(b)],
            nid,
        )
        .unwrap();
        let f = &p.floors[0];
        let m = solid_meshes(f, &DetailsLayer::load(f).solids, 12.0);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].object_id, Some(nid));
        let (lo, hi) = bounds(&m[0]);
        assert!((hi[0] - lo[0] - 15.0).abs() < 1e-4);
        assert!((lo[1] - 12.0).abs() < 1e-4, "stands on the floor");
        // Every triangle has a unit normal.
        assert!(m[0].vertices.iter().all(|v| {
            let n = v.normal;
            ((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() - 1.0).abs() < 1e-3
        }));
    }
}
