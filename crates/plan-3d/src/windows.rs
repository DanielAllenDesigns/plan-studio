//! Window styles: hung, casement, sliding, awning and hopper sashes, fixed,
//! and projecting bay / box / bow units.

use crate::builder::MeshSet;
use crate::leaf::{Grid, Leaf};
use crate::mesh::Material;
use crate::opening::{ArchGeom, Ctx};
use plan_core::opening_symbol::projection_footprint;
use plan_core::OpeningStyle;
/// Sash thickness.
const SASH_THICKNESS: f64 = 1.75;
/// Frame depth through the wall.
const FRAME_DEPTH: f64 = 3.5;
/// Thickness of a projected window panel.
const PANEL_THICKNESS: f64 = 2.0;
/// Seat board and roof slab thickness.
const SEAT_THICKNESS: f64 = 1.5;
const ROOF_THICKNESS: f64 = 2.0;
/// Width from which a casement window has two sashes.
const DOUBLE_CASEMENT_FROM: f64 = 48.0;

fn lites(ctx: &Ctx) -> (u32, u32) {
    if ctx.opts.show_lites {
        let (c, r) = ctx.opening.lites;
        (c.max(1), r.max(1))
    } else {
        (1, 1)
    }
}

/// The lite grid of the Lites tab over `u` x `h`; a plain pane when lites are
/// switched off.
fn grid(ctx: &Ctx, u: (f64, f64), h: (f64, f64)) -> Grid {
    if ctx.opts.show_lites {
        Grid::for_spec(&ctx.opening.extras.spec, lites(ctx), u, h)
    } else {
        Grid::plain(u, h, 1, 1)
    }
}

/// Width of a frame leg on one side of the hole: half the mullion where a
/// mulled neighbour stands against it, else the frame width.
fn leg(ctx: &Ctx, neighbour: bool) -> f64 {
    if neighbour {
        ctx.opening.extras.spec.mullion_width * 0.5
    } else {
        ctx.opening.frame_width()
    }
}

/// Flat frame, optional sash, glass panes and muntins across the whole hole.
fn flat(ctx: &Ctx, set: &mut MeshSet, sash: bool) {
    let legs = (leg(ctx, ctx.unit.left), leg(ctx, ctx.unit.right));
    flat_between(ctx, set, ctx.hole.s0, ctx.hole.s1, sash, legs);
}

/// [`flat`] over the part `s0..s1` of the hole; `legs` are the widths of the
/// frame on the start and end sides.
fn flat_between(ctx: &Ctx, set: &mut MeshSet, s0: f64, s1: f64, sash: bool, legs: (f64, f64)) {
    let h = ctx.hole;
    let (w, ht) = (s1 - s0, h.h1 - h.h0);
    let fw = ctx.opening.frame_width().min(w / 4.0).min(ht / 4.0);
    let (lw, rw) = (legs.0.min(w / 4.0), legs.1.min(w / 4.0));
    let depth = FRAME_DEPTH.min(ctx.wall.thickness) * 0.5;
    let t = (-depth, depth);
    let f = &ctx.frame;

    let ring = set.material(Material::WindowFrame);
    f.cuboid(ring, (s0, s1), t, (h.h0, h.h0 + fw));
    f.cuboid(ring, (s0, s1), t, (h.h1 - fw, h.h1));
    f.cuboid(ring, (s0, s0 + lw), t, (h.h0 + fw, h.h1 - fw));
    f.cuboid(ring, (s1 - rw, s1), t, (h.h0 + fw, h.h1 - fw));

    let (i0, i1) = (s0 + lw, s1 - rw);
    let (j0, j1) = (h.h0 + fw, h.h1 - fw);
    let spec = &ctx.opening.extras.spec;
    let has_sash = sash && spec.has_sash;
    let side = ctx.opening.sash_side();
    let (sw, st, sb) = if has_sash {
        (
            side.min((i1 - i0) / 4.0),
            spec.sash_top.min((j1 - j0) / 4.0),
            spec.sash_bottom.min((j1 - j0) / 4.0),
        )
    } else {
        (0.0, 0.0, 0.0)
    };
    if has_sash {
        let sd = SASH_THICKNESS.min(depth * 2.0) * 0.5;
        let t = (-sd, sd);
        let ring = set.material(Material::WindowFrame);
        f.cuboid(ring, (i0, i1), t, (j0, j0 + sb));
        f.cuboid(ring, (i0, i1), t, (j1 - st, j1));
        f.cuboid(ring, (i0, i0 + sw), t, (j0 + sb, j1 - st));
        f.cuboid(ring, (i1 - sw, i1), t, (j0 + sb, j1 - st));
    }
    let g = grid(ctx, (i0 + sw, i1 - sw), (j0 + sb, j1 - st));
    let leaf = Leaf::new((0.0, 0.0), (1.0, 0.0));
    g.panes(f, &leaf, set.material(Material::WindowGlass), 0.0);
    g.muntins(
        f,
        &leaf,
        set.material(Material::WindowFrame),
        0.0,
        g.muntin * 0.5,
    );
}

/// A window with an arched head (Arch tab): a frame of bottom rail, legs and
/// a curved band, the rectangle under the springline glazed with the lite
/// grid and the arch above it one pane.
fn arched(ctx: &Ctx, set: &mut MeshSet, arch: &ArchGeom) {
    let h = ctx.hole;
    let fw = ctx.opening.frame_width().max(0.5).min((h.s1 - h.s0) * 0.25);
    let depth = FRAME_DEPTH.min(ctx.wall.thickness) * 0.5;
    let t = (-depth, depth);
    let f = &ctx.frame;
    let spring = arch.spring;
    let ring = set.material(Material::WindowFrame);
    f.cuboid(ring, (h.s0, h.s1), t, (h.h0, h.h0 + fw));
    if spring > h.h0 + fw {
        f.cuboid(ring, (h.s0, h.s0 + fw), t, (h.h0 + fw, spring));
        f.cuboid(ring, (h.s1 - fw, h.s1), t, (h.h0 + fw, spring));
    }
    let leaf = Leaf::new((0.0, 0.0), (1.0, 0.0));
    for (o, i) in arch.outer.windows(2).zip(arch.inner.windows(2)) {
        leaf.slab(f, ring, &[o[0], o[1], i[1], i[0]], t);
    }
    // The rectangle under the curve: the lite grid.
    let g = grid(ctx, (h.s0 + fw, h.s1 - fw), (h.h0 + fw, spring));
    if spring - (h.h0 + fw) >= 2.0 {
        g.panes(f, &leaf, set.material(Material::WindowGlass), 0.0);
        g.muntins(
            f,
            &leaf,
            set.material(Material::WindowFrame),
            0.0,
            g.muntin * 0.5,
        );
    }
    // The arch above it is one pane.
    let gt = crate::leaf::GLASS_THICKNESS * 0.5;
    leaf.slab(
        f,
        set.material(Material::WindowGlass),
        &arch.inner,
        (-gt, gt),
    );
}

/// A casement: one sash, or two meeting at a post from 4' wide.
fn casement(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    if h.s1 - h.s0 >= DOUBLE_CASEMENT_FROM {
        let mid = (h.s0 + h.s1) * 0.5;
        let post = ctx.opening.extras.spec.mullion_width * 0.5;
        flat_between(ctx, set, h.s0, mid, true, (leg(ctx, ctx.unit.left), post));
        flat_between(ctx, set, mid, h.s1, true, (post, leg(ctx, ctx.unit.right)));
    } else {
        flat(ctx, set, true);
    }
}

/// A frame ring with two overlapping glazed sashes on separate tracks; the
/// movable one slides over the fixed one when the unit is open.
fn sliding_window(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    let (w, ht) = (h.s1 - h.s0, h.h1 - h.h0);
    let fw = ctx.opening.frame_width().min(w / 4.0).min(ht / 4.0);
    let depth = FRAME_DEPTH.min(ctx.wall.thickness) * 0.5;
    let t = (-depth, depth);
    let f = &ctx.frame;
    let ring = set.material(Material::WindowFrame);
    f.cuboid(ring, (h.s0, h.s1), t, (h.h0, h.h0 + fw));
    f.cuboid(ring, (h.s0, h.s1), t, (h.h1 - fw, h.h1));
    f.cuboid(ring, (h.s0, h.s0 + fw), t, (h.h0 + fw, h.h1 - fw));
    f.cuboid(ring, (h.s1 - fw, h.s1), t, (h.h0 + fw, h.h1 - fw));

    let inner = w - 2.0 * fw;
    let len = inner * 0.5 + 0.5;
    let track = (depth * 0.4).clamp(0.2, 0.75);
    let thick = 0.5;
    let mirror = ctx.opening.hinge_at_end;
    let rows = lites(ctx).1;
    for k in 0..2 {
        let kk = if mirror { 1 - k } else { k };
        let movable = kk == 1;
        let at = h.s0 + fw + if k == 0 { 0.0 } else { inner * 0.5 - 0.5 };
        let shift = if movable {
            (if mirror { 1.0 } else { -1.0 }) * (inner * 0.5 - 0.5) * ctx.open_fraction()
        } else {
            0.0
        };
        let c = if movable { track } else { -track };
        let leaf = Leaf::new((at + shift, 0.0), (1.0, 0.0));
        panel(ctx, set, &leaf, len, (c - thick, c + thick), rows);
    }
}

/// A sash hinged at the head (awning) or the sill (hopper): the frame with a
/// metal hinge bar along the hinged edge.
fn hinged_sash(ctx: &Ctx, set: &mut MeshSet, at_head: bool) {
    flat(ctx, set, true);
    let h = ctx.hole;
    let (w, ht) = (h.s1 - h.s0, h.h1 - h.h0);
    let fw = ctx.opening.frame_width().min(w / 4.0).min(ht / 4.0);
    let range = if at_head {
        (h.h1 - fw - 1.0, h.h1 - fw)
    } else {
        (h.h0 + fw, h.h0 + fw + 1.0)
    };
    let metal = set.material(Material::Metal);
    ctx.frame
        .cuboid(metal, (h.s0 + fw, h.s1 - fw), (-0.5, 0.5), range);
}

/// One panel of a projecting unit along `leaf` (u in `0..len`), inset toward
/// the wall by `tt`.
fn panel(ctx: &Ctx, set: &mut MeshSet, leaf: &Leaf, len: f64, tt: (f64, f64), rows: u32) {
    let h = ctx.hole;
    let ht = h.h1 - h.h0;
    let fw = ctx.opening.sash_side().min(len / 4.0).min(ht / 4.0);
    let f = &ctx.frame;
    let ring = set.material(Material::WindowFrame);
    leaf.boxed(f, ring, (0.0, len), tt, (h.h0, h.h0 + fw));
    leaf.boxed(f, ring, (0.0, len), tt, (h.h1 - fw, h.h1));
    leaf.boxed(f, ring, (0.0, fw), tt, (h.h0 + fw, h.h1 - fw));
    leaf.boxed(f, ring, (len - fw, len), tt, (h.h0 + fw, h.h1 - fw));
    let g = grid(ctx, (fw, len - fw), (h.h0 + fw, h.h1 - fw));
    // The panel grids keep their own column count of one.
    let g = if g.diamond.is_none() && g.cuts.is_none() {
        Grid { cols: 1, rows, ..g }
    } else {
        g
    };
    let mid = (tt.0 + tt.1) * 0.5;
    g.panes(f, leaf, set.material(Material::WindowGlass), mid);
    g.muntins(
        f,
        leaf,
        set.material(Material::WindowFrame),
        mid,
        g.muntin * 0.5,
    );
}

/// Bay (3 panels at 45 degrees), box (3 panels at 90 degrees) or bow (5
/// segments on an arc), with a seat board and a small roof slab.
fn projecting(ctx: &Ctx, set: &mut MeshSet) {
    // Projects toward the exterior: away from the room-facing side.
    let sign = -ctx.interior.signum() * ctx.swing_sign();
    let (h0, h1) = (ctx.hole.s0, ctx.hole.s1);
    let poly = projection_footprint(ctx.opening.style, h0, h1, sign * ctx.half(), sign);
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
    let style = ctx.style();
    if matches!(
        style,
        OpeningStyle::Window | OpeningStyle::Fixed | OpeningStyle::Casement
    ) {
        if let Some(arch) = ctx.arch(ctx.opening.frame_width()) {
            arched(ctx, set, &arch);
            return;
        }
    }
    match style {
        OpeningStyle::Fixed => flat(ctx, set, false),
        OpeningStyle::Casement => casement(ctx, set),
        OpeningStyle::SlidingWindow | OpeningStyle::Sliding => sliding_window(ctx, set),
        OpeningStyle::Awning => hinged_sash(ctx, set, true),
        OpeningStyle::Hopper => hinged_sash(ctx, set, false),
        OpeningStyle::BayWindow | OpeningStyle::BoxWindow | OpeningStyle::BowWindow => {
            projecting(ctx, set)
        }
        _ => flat(ctx, set, true),
    }
}
