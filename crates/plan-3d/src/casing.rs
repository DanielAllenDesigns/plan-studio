//! Casing, window sill and door threshold boards (Chief "Casing" / "Jamb").

use crate::builder::MeshSet;
use crate::mesh::Material;
use crate::opening::Ctx;
use plan_core::{Casing, OpeningKind, OpeningStyle};

/// Sill (stool) thickness.
const SILL_THICKNESS: f64 = 0.75;
/// How far the sill projects past the interior face.
const SILL_PROJECTION: f64 = 1.25;
/// Threshold height.
const THRESHOLD_HEIGHT: f64 = 0.75;

/// Casing boards on both faces of the wall around the opening, plus the sill
/// (windows) and apron below it.
pub fn add_casing(ctx: &Ctx, set: &mut MeshSet) {
    let c = ctx.opening.casing.unwrap_or_default();
    let Casing {
        width,
        depth,
        reveal,
    } = c;
    let (len, height) = (ctx.wall.length(), ctx.wall.height);
    let half = ctx.half();
    let h = ctx.hole;
    let window = ctx.opening.kind == OpeningKind::Window;
    let s_lo = (h.s0 - reveal - width).max(0.0);
    let s_hi = (h.s1 + reveal + width).min(len);
    let leg_bottom = if window { h.h0 } else { h.h0.max(0.0) };
    let head = (h.h1 + reveal, (h.h1 + reveal + width).min(height));
    let trim = set.material(Material::Trim);
    for side in [1.0, -1.0] {
        let t = if side > 0.0 {
            (half, half + depth)
        } else {
            (-half - depth, -half)
        };
        let legs = [
            ((s_lo, (h.s0 - reveal).max(0.0)), (leg_bottom, head.0)),
            (((h.s1 + reveal).min(len), s_hi), (leg_bottom, head.0)),
        ];
        for (s, hh) in legs {
            if s.1 - s.0 > 1e-6 && hh.1 - hh.0 > 1e-6 {
                ctx.frame.cuboid(trim, s, t, hh);
            }
        }
        if head.1 - head.0 > 1e-6 {
            ctx.frame.cuboid(trim, (s_lo, s_hi), t, head);
        }
        if window {
            let apron = (
                h.h0 - SILL_THICKNESS - width.min(h.h0 - SILL_THICKNESS),
                h.h0 - SILL_THICKNESS,
            );
            if apron.1 - apron.0 > 1e-6 && ctx.opening.style != OpeningStyle::PassThrough {
                ctx.frame
                    .cuboid(trim, (s_lo, s_hi), t, (apron.0.max(0.0), apron.1));
            }
        }
    }
    if window && h.h0 >= SILL_THICKNESS {
        // The stool projects into the room (interior side).
        let t = if ctx.interior >= 0.0 {
            (-half, half + SILL_PROJECTION)
        } else {
            (-half - SILL_PROJECTION, half)
        };
        ctx.frame
            .cuboid(trim, (s_lo, s_hi), t, (h.h0 - SILL_THICKNESS, h.h0));
    }
}

/// Threshold across the bottom of an exterior door.
pub fn add_threshold(ctx: &Ctx, set: &mut MeshSet) {
    let style = ctx.opening.style;
    if ctx.opening.kind != OpeningKind::Door
        || !style.is_door_style()
        || matches!(style, OpeningStyle::Shower | OpeningStyle::Doorway)
        || ctx.hole.h0 > 1e-6
    {
        return;
    }
    let half = ctx.half();
    ctx.frame.cuboid(
        set.material(Material::Trim),
        (ctx.hole.s0, ctx.hole.s1),
        (-half, half),
        (0.0, THRESHOLD_HEIGHT.min(ctx.wall.height)),
    );
}
