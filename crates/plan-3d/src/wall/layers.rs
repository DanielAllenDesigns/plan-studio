//! Wall layers joined at corners and intersections (brief 40).
//!
//! A wall whose type has several layers is built as one slab per layer, each
//! extruded from the layer's own plan outline
//! ([`plan_core::walls::intersect::outline_of`]): the same mitred, butted or
//! slid end lines the plan draws, so the material of a layer runs on round a
//! corner and no slab pokes past it. An end face is dropped where it lies
//! against another wall's layer (a mitre shares one edge, a butt stops on the
//! surface it meets), so no two faces are left coincident.

use super::{build_wall_shaped_cut, EndCuts, InteriorSign, Shape, WallLook, EPS};
use crate::mesh::{Material, Mesh};
use plan_core::joins::{wall_layer_bands, WallLayerOutline};
use plan_core::walls::intersect::{end_edge_is_shared, outline_of};
use plan_core::{Point, Wall, WallKind, WallTypeDef};

/// Drops cut lines that coincide with the square end of the plain box, so a
/// wall nothing joins keeps its plain frame.
pub fn natural_cuts(wall: &Wall, polygon: &[Point]) -> EndCuts {
    let [sl, el, er, sr] = polygon else {
        return EndCuts::NONE;
    };
    let (dir, length) = (wall.direction(), wall.length());
    let square = |a: Point, b: Point, at: f64| {
        ((a - wall.start).dot(dir) - at).abs() < 1e-4
            && ((b - wall.start).dot(dir) - at).abs() < 1e-4
    };
    EndCuts {
        start: (!square(*sl, *sr, 0.0)).then_some((*sl, *sr)),
        end: (!square(*el, *er, length)).then_some((*el, *er)),
    }
}

/// The layers of `body` as slabs. `outlines` are the layer outlines of the
/// whole floor; `ty` is the wall's type. Falls back to the whole-wall box
/// (cut by the layer-0 outline when the wall has one layer) when the outlines
/// are not there for this wall.
#[allow(clippy::too_many_arguments)]
pub fn build_layers(
    body: &Wall,
    elevation: f64,
    holes: &[super::Hole],
    interior: InteriorSign,
    look: WallLook,
    shape: &Shape,
    outlines: &[WallLayerOutline],
    ty: Option<&WallTypeDef>,
    cuts: &EndCuts,
) -> Vec<Mesh> {
    let bands = wall_layer_bands(body, ty);
    let polys: Vec<Option<[Point; 4]>> = (0..bands.len())
        .map(|k| outline_of(outlines, body.id, k))
        .collect();
    if bands.len() <= 1 || polys.iter().any(Option::is_none) {
        let c = match polys.first().copied().flatten() {
            Some(p) if cuts.is_none() => natural_cuts(body, &p),
            _ => *cuts,
        };
        let open = match polys.first().copied().flatten() {
            Some(p) => (
                end_edge_is_shared(outlines, body.id, p[0], p[3]),
                end_edge_is_shared(outlines, body.id, p[1], p[2]),
            ),
            None => (false, false),
        };
        let shape = Shape {
            open_start: open.0 && !c.is_none(),
            open_end: open.1 && !c.is_none(),
            ..*shape
        };
        return super::build_wall_shaped_cut(body, elevation, holes, interior, look, &shape, &c);
    }
    let n = body.normal();
    let centers: Vec<f64> = bands.iter().map(|b| (b.outer + b.inner) * 0.5).collect();
    // The exterior-most layer lies on the side away from the room; with no
    // room the type's own order says.
    let last = bands.len() - 1;
    let (exterior_k, interior_k) = if interior > 0.0 {
        extremes(&centers)
    } else if interior < 0.0 {
        let (lo, hi) = extremes(&centers);
        (hi, lo)
    } else {
        (0, last)
    };
    let mut out = Vec::new();
    for (k, band) in bands.iter().enumerate() {
        let thickness = (band.outer - band.inner).abs();
        if thickness <= EPS {
            continue;
        }
        let Some(p) = polys[k] else { continue };
        let mut slab = body.clone();
        slab.start = body.start + n * centers[k];
        slab.end = body.end + n * centers[k];
        slab.thickness = thickness;
        let material = if k == exterior_k && body.kind != WallKind::Interior {
            look.exterior
        } else if k == exterior_k || k == interior_k {
            Material::WallInterior
        } else {
            layer_material(ty, k, &band.name)
        };
        let c = natural_cuts(body, &p);
        let slab_shape = Shape {
            solid: Some(material),
            open_start: end_edge_is_shared(outlines, body.id, p[0], p[3]),
            open_end: end_edge_is_shared(outlines, body.id, p[1], p[2]),
            // Only the exterior surface follows the roof split.
            split: shape.split.filter(|_| k == exterior_k),
            ..*shape
        };
        // Niches are cut into the interior face only.
        let kept: Vec<super::Hole>;
        let slab_holes = if k == interior_k {
            holes
        } else {
            kept = holes
                .iter()
                .filter(|h| h.niche_depth.is_none())
                .copied()
                .collect();
            &kept
        };
        let slab_look = WallLook { exterior: material };
        out.extend(build_wall_shaped_cut(
            &slab,
            elevation,
            slab_holes,
            interior,
            slab_look,
            &slab_shape,
            &c,
        ));
    }
    out
}

/// Indices of the layers lying furthest to the minus and plus side.
fn extremes(centers: &[f64]) -> (usize, usize) {
    let mut lo = 0;
    let mut hi = 0;
    for (i, c) in centers.iter().enumerate() {
        if *c < centers[lo] {
            lo = i;
        }
        if *c > centers[hi] {
            hi = i;
        }
    }
    (lo, hi)
}

fn layer_material(ty: Option<&WallTypeDef>, k: usize, name: &str) -> Material {
    let from_def = ty
        .and_then(|t| t.layers.get(k))
        .and_then(|l| Material::from_layer_name(&l.material));
    Material::from_layer_name(name)
        .or(from_def)
        .unwrap_or(Material::Framing)
}
