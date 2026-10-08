//! Window styles: single casement, fixed, and projecting bay / box / bow units.

use crate::builder::MeshSet;
use crate::leaf::{Grid, Leaf, MUNTIN};
use crate::mesh::Material;
use crate::opening::Ctx;
use plan_core::OpeningStyle;

/// Window frame (jamb) width, 3/4" (Chief default).
pub const FRAME_WIDTH: f64 = 0.75;
/// Sash rail/stile width, 1 1/2" (Chief default).
pub const SASH_WIDTH: f64 = 1.5;
/// Sash thickness.
const SASH_THICKNESS: f64 = 1.75;
/// Frame depth through the wall.
const FRAME_DEPTH: f64 = 3.5;
/// How far bay, box and bow windows project from the exterior face.
pub const PROJECTION: f64 = 18.0;
/// Thickness of a projected window panel.
const PANEL_THICKNESS: f64 = 2.0;
/// Seat board and roof slab thickness.
const SEAT_THICKNESS: f64 = 1.5;
const ROOF_THICKNESS: f64 = 2.0;
/// Number of straight segments approximating a bow.
const BOW_SEGMENTS: usize = 5;

fn lites(ctx: &Ctx) -> (u32, u32) {
    if ctx.opts.show_lites {
        let (c, r) = ctx.opening.lites;
        (c.max(1), r.max(1))
    } else {
        (1, 1)
    }
}

/// Flat frame, optional sash, glass panes and muntins.
fn flat(ctx: &Ctx, set: &mut MeshSet, sash: bool) {
    let h = ctx.hole;
    let (w, ht) = (h.s1 - h.s0, h.h1 - h.h0);
    let fw = FRAME_WIDTH.min(w / 4.0).min(ht / 4.0);
    let depth = FRAME_DEPTH.min(ctx.wall.thickness) * 0.5;
    let t = (-depth, depth);
    let f = &ctx.frame;

    let ring = set.material(Material::WindowFrame);
    f.cuboid(ring, (h.s0, h.s1), t, (h.h0, h.h0 + fw));
    f.cuboid(ring, (h.s0, h.s1), t, (h.h1 - fw, h.h1));
    f.cuboid(ring, (h.s0, h.s0 + fw), t, (h.h0 + fw, h.h1 - fw));
    f.cuboid(ring, (h.s1 - fw, h.s1), t, (h.h0 + fw, h.h1 - fw));

    let (i0, i1) = (h.s0 + fw, h.s1 - fw);
    let (j0, j1) = (h.h0 + fw, h.h1 - fw);
    let sw = if sash {
        SASH_WIDTH.min((i1 - i0) / 4.0).min((j1 - j0) / 4.0)
    } else {
        0.0
    };
    if sash {
        let st = SASH_THICKNESS.min(depth * 2.0) * 0.5;
        let t = (-st, st);
        let ring = set.material(Material::WindowFrame);
        f.cuboid(ring, (i0, i1), t, (j0, j0 + sw));
        f.cuboid(ring, (i0, i1), t, (j1 - sw, j1));
        f.cuboid(ring, (i0, i0 + sw), t, (j0 + sw, j1 - sw));
        f.cuboid(ring, (i1 - sw, i1), t, (j0 + sw, j1 - sw));
    }
    let (cols, rows) = lites(ctx);
    let grid = Grid {
        u: (i0 + sw, i1 - sw),
        h: (j0 + sw, j1 - sw),
        cols,
        rows,
    };
    let leaf = Leaf::new((0.0, 0.0), (1.0, 0.0));
    grid.panes(f, &leaf, set.material(Material::WindowGlass), 0.0);
    grid.muntins(
        f,
        &leaf,
        set.material(Material::WindowFrame),
        0.0,
        MUNTIN * 0.5,
    );
}

/// One panel of a projecting unit along `leaf` (u in `0..len`), inset toward
/// the wall by `tt`.
fn panel(ctx: &Ctx, set: &mut MeshSet, leaf: &Leaf, len: f64, tt: (f64, f64), rows: u32) {
    let h = ctx.hole;
    let ht = h.h1 - h.h0;
    let fw = (SASH_WIDTH).min(len / 4.0).min(ht / 4.0);
    let f = &ctx.frame;
    let ring = set.material(Material::WindowFrame);
    leaf.boxed(f, ring, (0.0, len), tt, (h.h0, h.h0 + fw));
    leaf.boxed(f, ring, (0.0, len), tt, (h.h1 - fw, h.h1));
    leaf.boxed(f, ring, (0.0, fw), tt, (h.h0 + fw, h.h1 - fw));
    leaf.boxed(f, ring, (len - fw, len), tt, (h.h0 + fw, h.h1 - fw));
    let grid = Grid {
        u: (fw, len - fw),
        h: (h.h0 + fw, h.h1 - fw),
        cols: 1,
        rows,
    };
    let mid = (tt.0 + tt.1) * 0.5;
    grid.panes(f, leaf, set.material(Material::WindowGlass), mid);
    grid.muntins(
        f,
        leaf,
        set.material(Material::WindowFrame),
        mid,
        MUNTIN * 0.5,
    );
}

/// Outer-surface footprint of the projecting unit in wall `(s, t)`, starting
/// and ending on the exterior wall face.
fn footprint(ctx: &Ctx, style: OpeningStyle, sign: f64) -> Vec<(f64, f64)> {
    let h = ctx.hole;
    let w = h.s1 - h.s0;
    let t0 = sign * ctx.half();
    let t1 = t0 + sign * PROJECTION;
    match style {
        OpeningStyle::BayWindow => {
            let ds = PROJECTION.min((w - 6.0).max(0.0) * 0.5);
            vec![(h.s0, t0), (h.s0 + ds, t1), (h.s1 - ds, t1), (h.s1, t0)]
        }
        OpeningStyle::BoxWindow => vec![(h.s0, t0), (h.s0, t1), (h.s1, t1), (h.s1, t0)],
        _ => {
            // Circular arc through both jambs with sagitta PROJECTION.
            let c = w * 0.5;
            let r = (c * c + PROJECTION * PROJECTION) / (2.0 * PROJECTION);
            let (sc, tc) = ((h.s0 + h.s1) * 0.5, t0 - sign * (r - PROJECTION));
            let phi0 = c.atan2(r - PROJECTION);
            let arc: Vec<(f64, f64)> = (0..=BOW_SEGMENTS)
                .map(|k| {
                    let phi = -phi0 + 2.0 * phi0 * k as f64 / BOW_SEGMENTS as f64;
                    (sc + r * phi.sin(), sign * r * phi.cos() + tc)
                })
                .collect();
            // No segment vertex sits at the arc apex; rescale so the unit
            // projects exactly PROJECTION from the wall face.
            let depth = |p: &(f64, f64)| (p.1 - t0) * sign;
            let max = arc.iter().map(depth).fold(f64::MIN, f64::max).max(1e-9);
            let k = PROJECTION / max;
            arc.iter().map(|p| (p.0, t0 + (p.1 - t0) * k)).collect()
        }
    }
}

/// Bay (3 panels at 45 degrees), box (3 panels at 90 degrees) or bow (5
/// segments on an arc), with a seat board and a small roof slab.
fn projecting(ctx: &Ctx, set: &mut MeshSet) {
    // Projects toward the exterior: away from the room-facing side.
    let sign = -ctx.interior.signum();
    let poly = footprint(ctx, ctx.opening.style, sign);
    let n = poly.len() as f64;
    let centroid = (
        poly.iter().map(|p| p.0).sum::<f64>() / n,
        poly.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let rows = lites(ctx).1;
    for pair in poly.windows(2) {
        let (p, q) = (pair[0], pair[1]);
        let len = (q.0 - p.0).hypot(q.1 - p.1);
        if len < 1e-6 {
            continue;
        }
        let leaf = Leaf::new(p, (q.0 - p.0, q.1 - p.1));
        let mid = ((p.0 + q.0) * 0.5, (p.1 + q.1) * 0.5);
        let nrm = leaf.normal();
        let outward = nrm.0 * (mid.0 - centroid.0) + nrm.1 * (mid.1 - centroid.1) > 0.0;
        let tt = if outward {
            (-PANEL_THICKNESS, 0.0)
        } else {
            (0.0, PANEL_THICKNESS)
        };
        panel(ctx, set, &leaf, len, tt, rows);
    }
    let h = ctx.hole;
    let trim = set.material(Material::Trim);
    ctx.frame.prism(trim, &poly, (h.h0 - SEAT_THICKNESS, h.h0));
    let roof = set.material(Material::Roof);
    ctx.frame.prism(roof, &poly, (h.h1, h.h1 + ROOF_THICKNESS));
}

/// Add the window meshes for `ctx.opening.style`.
pub fn build(ctx: &Ctx, set: &mut MeshSet) {
    match ctx.opening.style {
        OpeningStyle::Fixed => flat(ctx, set, false),
        OpeningStyle::BayWindow | OpeningStyle::BoxWindow | OpeningStyle::BowWindow => {
            projecting(ctx, set)
        }
        _ => flat(ctx, set, true),
    }
}
