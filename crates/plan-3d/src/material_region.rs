//! Layered Floor and Wall Material Regions in 3D (`plan_core::material_region`).
//!
//! A region with a layer table is drawn as one slab per solid layer, each in
//! its own material. A floor region cuts into the finish (the exposed layer
//! is flush with the floor finish, the rest under it) or sits on top of it
//! (exposed layer on top). A wall region does the same on a face of its wall
//! and leaves the openings out. Air gaps are not drawn.

use crate::builder::MeshBuilder;
use crate::details::{material_of, region_mesh};
use crate::frame::Frame;
use crate::mesh::{Material, Mesh};
use crate::slab::FLOOR_FINISH;
use crate::solids::tris_mesh;
use plan_core::details::{MaterialRegion, RegionKind};
use plan_core::material_region::{layer_bands, wall_region_rects, LayerRole};
use plan_core::solids::prism;
use plan_core::walls::Side;
use plan_core::Floor;

/// How far the exposed layer of a cutting region stands above the surface it
/// replaces, so the two do not fight (the single plate uses the same lift).
const FLUSH_LIFT: f64 = 0.02;

/// The meshes of one layered region, or `None` when it has no layer table
/// (the caller draws the single plate then).
pub fn layered_region_meshes(r: &MaterialRegion, floor: &Floor) -> Option<Vec<Mesh>> {
    if !floor.has_region_structure(r.id) {
        return None;
    }
    let layers = floor.region_layers_of(r);
    let bands = layer_bands(&layers, r.cut_finish_layers);
    let mut out = Vec::new();
    match r.kind {
        RegionKind::Floor => {
            if r.outline.len() < 3 {
                return Some(out);
            }
            let top = floor.elevation + FLOOR_FINISH;
            for b in &bands {
                let l = &layers[b.layer];
                if !l.role.is_solid() || l.thickness <= 0.0 {
                    continue;
                }
                let lift = if r.cut_finish_layers && b.layer == 0 {
                    FLUSH_LIFT
                } else {
                    0.0
                };
                // `prism` works relative to a floor at height 0.
                let tris = prism(&r.outline, top - floor.elevation + b.lo, top - floor.elevation + b.hi + lift);
                out.extend(tris_mesh(
                    &tris,
                    floor.elevation,
                    material_of(&l.material, Material::Floor),
                    r.id,
                ));
            }
        }
        RegionKind::Wall(id) => {
            let wall = floor.wall(id).filter(|w| !w.is_curved())?;
            let half = wall.thickness * 0.5;
            let frame = Frame::new(wall, floor.elevation);
            let rects = wall_region_rects(floor, r);
            for b in &bands {
                let l = &layers[b.layer];
                if !l.role.is_solid() || l.thickness <= 0.0 {
                    continue;
                }
                let lift = if r.cut_finish_layers && b.layer == 0 {
                    FLUSH_LIFT
                } else {
                    0.0
                };
                let t = match r.side {
                    Side::Left => (half + b.lo, half + b.hi + lift),
                    Side::Right => (-half - b.hi - lift, -half - b.lo),
                };
                let mut mesh = MeshBuilder::new(material_of(&l.material, Material::Floor));
                for q in &rects {
                    frame.cuboid(&mut mesh, (q[0], q[1]), t, (q[2], q[3]));
                }
                if !mesh.is_empty() {
                    out.push(mesh.finish(Some(r.id)));
                }
            }
        }
    }
    Some(out)
}

/// Every region mesh of a floor: layered regions as slabs, the others as the
/// single plate.
pub fn region_meshes(floor: &Floor, regions: &[MaterialRegion]) -> Vec<Mesh> {
    let mut out = Vec::new();
    for r in regions {
        match layered_region_meshes(r, floor) {
            Some(m) => out.extend(m),
            None => out.extend(region_mesh(r, floor)),
        }
    }
    out
}

/// Does the table draw anything (some solid layer with a thickness)?
pub fn draws_something(floor: &Floor, r: &MaterialRegion) -> bool {
    floor
        .region_layers_of(r)
        .iter()
        .any(|l| l.role != LayerRole::AirGap && l.thickness > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::details::DetailsLayer;
    use plan_core::geometry::Point;
    use plan_core::material_region::{MaterialLayer, RegionStructure};
    use plan_core::{Opening, OpeningKind, Project, WallKind};

    fn extent(ms: &[Mesh], axis: usize) -> (f32, f32) {
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for m in ms {
            for v in &m.vertices {
                lo = lo.min(v.position[axis]);
                hi = hi.max(v.position[axis]);
            }
        }
        (lo, hi)
    }

    #[test]
    fn a_floor_region_stacks_its_layers_cut_or_on_top() {
        let mut p = Project::new("fr");
        let rid = p.alloc_id();
        let mut d = DetailsLayer::default();
        d.regions.push(MaterialRegion::floor(
            rid,
            vec![
                Point::new(0.0, 0.0),
                Point::new(48.0, 0.0),
                Point::new(48.0, 48.0),
                Point::new(0.0, 48.0),
            ],
        ));
        p.floors[0].set_details(&d).unwrap();
        // No table: the single plate.
        let f = &p.floors[0];
        let plain = region_meshes(f, &DetailsLayer::load(f).regions);
        assert_eq!(plain.len(), 1);
        let s = RegionStructure::new(
            rid,
            vec![
                MaterialLayer::new("Ceramic Tile 12x12", 0.5),
                MaterialLayer::new("Concrete", 1.0),
            ],
        );
        p.floors[0].set_region_structure(s);
        let f = &p.floors[0];
        let regions = DetailsLayer::load(f).regions;
        let on_top = region_meshes(f, &regions);
        assert_eq!(on_top.len(), 2, "one slab per layer");
        let (lo, hi) = extent(&on_top, 1);
        assert!((lo - FLOOR_FINISH as f32).abs() < 1e-4);
        assert!((hi - (FLOOR_FINISH as f32 + 1.5)).abs() < 1e-4, "{hi}");
        // Cut: the exposed layer is flush, the stack goes down.
        let mut cut = regions[0].clone();
        cut.cut_finish_layers = true;
        let m = layered_region_meshes(&cut, f).unwrap();
        let (lo, hi) = extent(&m, 1);
        assert!((hi - FLOOR_FINISH as f32).abs() < 0.05, "{hi}");
        assert!((lo - (FLOOR_FINISH as f32 - 1.5)).abs() < 1e-4, "{lo}");
        // The mesh carries the region's id and the layer's material.
        assert!(m.iter().all(|m| m.object_id == Some(rid)));
        assert_ne!(m[0].material, m[1].material);
    }

    #[test]
    fn a_wall_region_leaves_the_doorway_open() {
        let mut p = Project::new("wr");
        let wid = p.add_wall(0, Point::new(0.0, 0.0), Point::new(120.0, 0.0), 6.5, 96.0, WallKind::Exterior);
        let oid = p.alloc_id();
        let mut o = Opening::new(wid, 60.0, OpeningKind::Door, 36.0, 80.0, 0.0);
        o.id = oid;
        p.floors[0].openings.push(o);
        let rid = p.alloc_id();
        let mut d = DetailsLayer::default();
        d.regions.push(MaterialRegion::wall(rid, wid, Side::Left, 0.0, 120.0, 0.0, 96.0));
        p.floors[0].set_details(&d).unwrap();
        p.floors[0].set_region_structure(RegionStructure::new(
            rid,
            vec![MaterialLayer::new("Ceramic Tile 12x12", 0.5), MaterialLayer::new("Concrete", 0.5)],
        ));
        let f = &p.floors[0];
        let regions = DetailsLayer::load(f).regions;
        let m = layered_region_meshes(&regions[0], f).unwrap();
        assert_eq!(m.len(), 2);
        // The faces facing out of the left of the wall sit beyond half its thickness.
        let (lo, hi) = extent(&m, 2);
        assert!(lo.abs() > 3.0 || hi.abs() > 3.0);
        // No vertex of the first slab lies inside the doorway (x 42..78, y < 80) on the wall face.
        for v in &m[0].vertices {
            let (x, y) = (v.position[0], v.position[1]);
            assert!(!(x > 42.1 && x < 77.9 && y < 79.9) || y <= 0.001, "{x} {y}");
        }
        assert!(draws_something(f, &regions[0]));
    }
}
