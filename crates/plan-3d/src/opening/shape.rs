//! Shaped windows (Shape tab): half round, quarter round, trapezoid, triangle
//! and custom outlines, glazed to the outline with a frame band, an optional
//! sash band and the lite pattern clipped to the shape. The wall hole stays
//! rectangular; wall fills the corners the outline leaves.

use crate::builder::MeshSet;
use crate::leaf::Leaf;
use crate::mesh::Material;
use crate::opening::Ctx;
use crate::windows::{FRAME_DEPTH, SASH_THICKNESS};
use plan_core::openings::spec::inset_convex;

/// Frame band width used when the opening has no frame of its own.
const MIN_FRAME: f64 = 0.5;

/// Wall-local `(s, h)` points of the outline of `ctx`'s window.
pub fn outline(ctx: &Ctx) -> Vec<(f64, f64)> {
    let h = ctx.hole;
    ctx.opening
        .extras
        .spec
        .shape
        .outline(h.s1 - h.s0, h.h1 - h.h0)
        .into_iter()
        .map(|p| (h.s0 + p.0, h.h0 + p.1))
        .collect()
}

/// The wall that fills the rectangle of the hole outside the outline, in the
/// exterior and interior wall materials (the way an arch's corners are
/// filled).
pub fn spandrel(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    let half = ctx.half();
    let leaf = Leaf::new((0.0, 0.0), (1.0, 0.0));
    let exterior = -ctx.interior;
    let pieces = ctx
        .opening
        .extras
        .spec
        .shape
        .spandrels(h.s1 - h.s0, h.h1 - h.h0);
    for (side, material) in [
        (exterior, Material::WallExterior),
        (ctx.interior, Material::WallInterior),
    ] {
        let t = if side > 0.0 {
            (0.0, half)
        } else {
            (-half, 0.0)
        };
        let mesh = set.material(material);
        for piece in &pieces {
            let pts: Vec<(f64, f64)> = piece.iter().map(|p| (h.s0 + p.0, h.h0 + p.1)).collect();
            leaf.slab(&ctx.frame, mesh, &pts, t);
        }
    }
}

/// A band between `outer` and `inner` (the same polygon moved in), as one
/// quad per edge when the two have matching vertices, else per-edge strips
/// of the given width.
fn band(
    ctx: &Ctx,
    leaf: &Leaf,
    mesh: &mut crate::builder::MeshBuilder,
    outer: &[(f64, f64)],
    inner: &[(f64, f64)],
    width: f64,
    t: (f64, f64),
) {
    let n = outer.len();
    if inner.len() == n {
        for i in 0..n {
            let j = (i + 1) % n;
            leaf.slab(
                &ctx.frame,
                mesh,
                &[outer[i], outer[j], inner[j], inner[i]],
                t,
            );
        }
        return;
    }
    for i in 0..n {
        let (a, b) = (outer[i], outer[(i + 1) % n]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx.hypot(dy).max(1e-9);
        // Inward normal of a counter-clockwise polygon.
        let (nx, ny) = (-dy / len * width, dx / len * width);
        leaf.slab(
            &ctx.frame,
            mesh,
            &[a, b, (b.0 + nx, b.1 + ny), (a.0 + nx, a.1 + ny)],
            t,
        );
    }
}

/// Builds a shaped window: frame, sash band, glass and muntins.
pub fn shaped(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    let (w, ht) = (h.s1 - h.s0, h.h1 - h.h0);
    let spec = &ctx.opening.extras.spec;
    let out = outline(ctx);
    if out.len() < 3 {
        return;
    }
    let fw = ctx
        .opening
        .frame_width()
        .max(MIN_FRAME)
        .min(w * 0.25)
        .min(ht * 0.25);
    let depth = FRAME_DEPTH.min(ctx.wall.thickness) * 0.5;
    let leaf = Leaf::new((0.0, 0.0), (1.0, 0.0));
    let inner = inset_convex(&out, fw);
    if inner.len() < 3 {
        return;
    }
    band(
        ctx,
        &leaf,
        set.material(Material::WindowFrame),
        &out,
        &inner,
        fw,
        (-depth, depth),
    );
    // The sash band inside the frame.
    let side = ctx.opening.sash_side().min(w * 0.2).min(ht * 0.2);
    let mut glass = inner.clone();
    if spec.has_sash && side > 0.05 && ctx.style() != plan_core::OpeningStyle::Fixed {
        let sd = SASH_THICKNESS.min(depth * 2.0) * 0.5;
        let in2 = inset_convex(&inner, side);
        if in2.len() >= 3 {
            band(
                ctx,
                &leaf,
                ctx.sash.borrow_mut().material(Material::WindowFrame),
                &inner,
                &in2,
                side,
                (-sd, sd),
            );
            glass = in2;
        }
    }
    let g = crate::leaf::GLASS_THICKNESS * 0.5;
    leaf.slab(
        &ctx.frame,
        set.material(Material::WindowGlass),
        &glass,
        (-g, g),
    );
    // Lite dividers clipped to the glass.
    if ctx.opts.show_lites {
        let (cols, rows) = ctx.opening.lites;
        let muntin = spec.muntin_width.clamp(0.125, 4.0);
        let local_glass: Vec<(f64, f64)> = glass.iter().map(|p| (p.0 - h.s0, p.1 - h.h0)).collect();
        let mut sash = ctx.sash.borrow_mut();
        let mesh = sash.material(Material::WindowFrame);
        for (p, q) in clip_lines(ctx, (cols.max(1), rows.max(1)), w, ht, &local_glass) {
            let len = (q.0 - p.0).hypot(q.1 - p.1);
            if len < 0.5 {
                continue;
            }
            let (dx, dy) = ((q.0 - p.0) / len, (q.1 - p.1) / len);
            let off = (-dy * muntin * 0.5, dx * muntin * 0.5);
            let (a, b) = ((h.s0 + p.0, h.h0 + p.1), (h.s0 + q.0, h.h0 + q.1));
            leaf.slab(
                &ctx.frame,
                mesh,
                &[
                    (a.0 + off.0, a.1 + off.1),
                    (b.0 + off.0, b.1 + off.1),
                    (b.0 - off.0, b.1 - off.1),
                    (a.0 - off.0, a.1 - off.1),
                ],
                (-muntin * 0.25, muntin * 0.25),
            );
        }
    }
}

/// The lite lines of the pattern clipped to the glass polygon (window frame
/// coordinates).
fn clip_lines(
    ctx: &Ctx,
    lites: (u32, u32),
    w: f64,
    ht: f64,
    glass: &[(f64, f64)],
) -> Vec<((f64, f64), (f64, f64))> {
    let shape = &ctx.opening.extras.spec.shape;
    // The pattern's own lines run over the whole outline; clip them again to
    // the glass (the sash band may be inside the frame band).
    shape
        .lite_lines(&ctx.opening.extras.spec, lites, w, ht, 0.0)
        .into_iter()
        .filter_map(|(p, q)| plan_core::openings::spec::clip_segment_convex(glass, p, q))
        .collect()
}
