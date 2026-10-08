//! Door styles: hinged, sliding, pocket, bifold, garage, barn, shower, fixed.

use crate::builder::MeshSet;
use crate::frame::Frame;
use crate::leaf::{Grid, Leaf};
use crate::mesh::Material;
use crate::opening::Ctx;
use plan_core::OpeningStyle;

/// Door slab thickness, 1 3/8".
pub const DOOR_THICKNESS: f64 = 1.375;
/// Glass door-light stile width (rail/stile border around a lite).
const STILE: f64 = 6.0;
/// Sliding/bifold panel thickness.
const THIN_PANEL: f64 = 1.0;
/// Garage section count.
const GARAGE_SECTIONS: usize = 4;
/// Garage door panel thickness.
const GARAGE_THICKNESS: f64 = 2.0;

/// A rectangular lite grid cut into a panel, in leaf-local `u`/`h`.
struct Lite {
    grid: Grid,
    glass: Material,
    bars: Material,
}

/// Lites to cut into doors: only when the opening asks for more than one lite
/// and lites are shown (a plain `(1, 1)` door stays a solid slab).
fn glazing(ctx: &Ctx) -> Option<(u32, u32)> {
    let (c, r) = ctx.opening.lites;
    (ctx.opts.show_lites && c >= 1 && r >= 1 && u64::from(c) * u64::from(r) > 1).then_some((c, r))
}

/// A slab over `u` x `h`, `tt` thick, with an optional lite cut out of it.
fn panel(
    frame: &Frame,
    set: &mut MeshSet,
    leaf: &Leaf,
    u: (f64, f64),
    h: (f64, f64),
    tt: (f64, f64),
    lite: Option<&Lite>,
) {
    let Some(lite) = lite else {
        leaf.boxed(frame, set.material(Material::DoorPanel), u, tt, h);
        return;
    };
    let (lu, lh) = (lite.grid.u, lite.grid.h);
    let slab = set.material(Material::DoorPanel);
    leaf.boxed(frame, slab, u, tt, (h.0, lh.0));
    leaf.boxed(frame, slab, u, tt, (lh.1, h.1));
    leaf.boxed(frame, slab, (u.0, lu.0), tt, lh);
    leaf.boxed(frame, slab, (lu.1, u.1), tt, lh);
    let mid = (tt.0 + tt.1) * 0.5;
    lite.grid.panes(frame, leaf, set.material(lite.glass), mid);
    lite.grid
        .muntins(frame, leaf, set.material(lite.bars), mid, tt.1 - tt.0);
}

/// Hinged-leaf frame for a door `width` wide rotated `angle_deg` about its hinge.
fn hinge_leaf(ctx: &Ctx, angle_deg: f64) -> Leaf {
    let (hs, dsign) = ctx.hinge();
    let a = angle_deg.to_radians();
    Leaf::new((hs, 0.0), (dsign * a.cos(), ctx.swing_sign() * a.sin()))
}

/// Door slab lite: upper two thirds, `STILE` wide borders.
fn door_lite(ctx: &Ctx, width: f64) -> Option<Lite> {
    let (cols, rows) = glazing(ctx)?;
    let (h0, h1) = (ctx.hole.h0, ctx.hole.h1);
    let height = h1 - h0;
    let stile = STILE.min(width / 4.0);
    Some(Lite {
        grid: Grid {
            u: (stile, width - stile),
            h: (h0 + height * 0.3, h1 - (STILE + 2.0).min(height * 0.1)),
            cols,
            rows,
        },
        glass: Material::WindowGlass,
        bars: Material::DoorPanel,
    })
}

fn open_angle(ctx: &Ctx) -> f64 {
    ctx.opts.display().open_angle_deg
}

fn hinged(ctx: &Ctx, set: &mut MeshSet, angle: f64) {
    let w = ctx.hole.s1 - ctx.hole.s0;
    let leaf = hinge_leaf(ctx, angle);
    let t = DOOR_THICKNESS * 0.5;
    let lite = door_lite(ctx, w);
    panel(
        &ctx.frame,
        set,
        &leaf,
        (0.0, w),
        (ctx.hole.h0, ctx.hole.h1),
        (-t, t),
        lite.as_ref(),
    );
}

/// Two overlapping glazed panels on separate tracks.
fn sliding(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    let w = h.s1 - h.s0;
    let pw = w * 0.5 + 1.0;
    let off = (ctx.half() - THIN_PANEL * 0.5).clamp(0.25, 0.75);
    let shift = if ctx.opts.doors_open {
        w * 0.5 - 1.0
    } else {
        0.0
    };
    let height = h.h1 - h.h0;
    let lite = Lite {
        grid: Grid {
            u: (2.0, pw - 2.0),
            h: (h.h0 + height * 0.5, h.h1 - 2.0),
            cols: 1,
            rows: 1,
        },
        glass: Material::Glass,
        bars: Material::DoorPanel,
    };
    let tt = (-THIN_PANEL * 0.5, THIN_PANEL * 0.5);
    let fixed = Leaf::new((h.s0, -off), (1.0, 0.0));
    let moving = Leaf::new((h.s1 - pw - shift, off), (1.0, 0.0));
    for leaf in [fixed, moving] {
        panel(
            &ctx.frame,
            set,
            &leaf,
            (0.0, pw),
            (h.h0, h.h1),
            tt,
            Some(&lite),
        );
    }
}

/// A slab that slides into the wall beside the opening when open.
fn pocket(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    let w = h.s1 - h.s0;
    let toward_end = ctx.opening.hinge_at_end;
    let room = if toward_end {
        ctx.wall.length() - h.s1
    } else {
        h.s0
    };
    let shift = if ctx.opts.doors_open {
        (w - 1.0).min(room - 0.5).max(0.0)
    } else {
        0.0
    };
    let dir = if toward_end { 1.0 } else { -1.0 };
    let leaf = Leaf::new((h.s0 + dir * shift, 0.0), (1.0, 0.0));
    let t = DOOR_THICKNESS * 0.5;
    let lite = door_lite(ctx, w);
    panel(
        &ctx.frame,
        set,
        &leaf,
        (0.0, w),
        (h.h0, h.h1),
        (-t, t),
        lite.as_ref(),
    );
}

/// Two panels hinged together; folded back when the door is open.
fn bifold(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    let half_w = (h.s1 - h.s0) * 0.5;
    let angle = open_angle(ctx).clamp(0.0, 80.0);
    let a = angle.to_radians();
    let (hs, dsign) = ctx.hinge();
    let swing = ctx.swing_sign();
    let first = Leaf::new((hs, 0.0), (dsign * a.cos(), swing * a.sin()));
    let tip = first.st(half_w, 0.0);
    let second = Leaf::new(tip, (dsign * a.cos(), -swing * a.sin()));
    let t = THIN_PANEL * 0.5;
    for leaf in [first, second] {
        panel(
            &ctx.frame,
            set,
            &leaf,
            (0.0, half_w),
            (h.h0, h.h1),
            (-t, t),
            None,
        );
    }
}

/// Four stacked sections with metal rails; lites in the top section.
fn garage(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    let w = h.s1 - h.s0;
    let leaf = Leaf::new((h.s0, 0.0), (1.0, 0.0));
    let t = (GARAGE_THICKNESS.min(ctx.wall.thickness) * 0.5).max(0.25);
    let section = (h.h1 - h.h0) / GARAGE_SECTIONS as f64;
    let top = GARAGE_SECTIONS - 1;
    for k in 0..GARAGE_SECTIONS {
        let (a, b) = (h.h0 + k as f64 * section, h.h0 + (k + 1) as f64 * section);
        let lite = (k == top)
            .then(|| glazing(ctx))
            .flatten()
            .map(|(cols, rows)| Lite {
                grid: Grid {
                    u: (3.0, w - 3.0),
                    h: (a + 4.0, b - 3.0),
                    cols,
                    rows,
                },
                glass: Material::WindowGlass,
                bars: Material::DoorPanel,
            });
        panel(
            &ctx.frame,
            set,
            &leaf,
            (0.0, w),
            (a + 0.25, b - 0.25),
            (-t, t),
            lite.as_ref(),
        );
    }
    let rail_t = (-t - 0.125, t + 0.125);
    let metal = set.material(Material::Metal);
    for k in 0..GARAGE_SECTIONS {
        let a = h.h0 + k as f64 * section;
        leaf.boxed(&ctx.frame, metal, (0.0, w), rail_t, (a, a + 1.5));
    }
}

/// A panel hung outside the wall on a track bar; slides along the wall when open.
fn barn(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    let w = h.s1 - h.s0;
    let sign = ctx.swing_sign();
    let center = sign * (ctx.half() + 0.5 + DOOR_THICKNESS * 0.5);
    let pw = w + 2.0;
    let dir = if ctx.opening.hinge_at_end { 1.0 } else { -1.0 };
    let shift = if ctx.opts.doors_open { w } else { 0.0 };
    let start = h.s0 - 1.0 + dir * shift;
    let leaf = Leaf::new((start, center), (1.0, 0.0));
    let t = DOOR_THICKNESS * 0.5;
    let lite = door_lite(ctx, pw);
    panel(
        &ctx.frame,
        set,
        &leaf,
        (0.0, pw),
        (h.h0, h.h1),
        (-t, t),
        lite.as_ref(),
    );

    let len = ctx.wall.length();
    let origin = Leaf::new((0.0, 0.0), (1.0, 0.0));
    let metal = set.material(Material::Metal);
    let bar = ((h.s0 - w).max(0.0), (h.s1 + w).min(len));
    origin.boxed(
        &ctx.frame,
        metal,
        bar,
        (center - 0.5, center + 0.5),
        (h.h1 + 1.0, h.h1 + 3.0),
    );
    for u in [3.0, pw - 3.0] {
        let s = start + u;
        origin.boxed(
            &ctx.frame,
            metal,
            (s - 1.0, s + 1.0),
            (center - 0.25, center + 0.25),
            (h.h1, h.h1 + 3.0),
        );
    }
}

/// A glass leaf with a metal hinge strip and head rail.
fn shower(ctx: &Ctx, set: &mut MeshSet, angle: f64) {
    let h = ctx.hole;
    let w = h.s1 - h.s0;
    let leaf = hinge_leaf(ctx, angle);
    let g = 0.1875;
    leaf.boxed(
        &ctx.frame,
        set.material(Material::Glass),
        (0.0, w),
        (-g, g),
        (h.h0, h.h1),
    );
    let metal = set.material(Material::Metal);
    leaf.boxed(&ctx.frame, metal, (0.0, 1.0), (-0.5, 0.5), (h.h0, h.h1));
    leaf.boxed(&ctx.frame, metal, (0.0, w), (-0.5, 0.5), (h.h1 - 1.0, h.h1));
}

/// Add the door meshes for `ctx.opening.style` (a cased doorway adds nothing).
pub fn build(ctx: &Ctx, set: &mut MeshSet) {
    match ctx.opening.style {
        OpeningStyle::Hinged => hinged(ctx, set, open_angle(ctx)),
        OpeningStyle::Fixed => hinged(ctx, set, 0.0),
        OpeningStyle::Sliding => sliding(ctx, set),
        OpeningStyle::Pocket => pocket(ctx, set),
        OpeningStyle::Bifold => bifold(ctx, set),
        OpeningStyle::Garage => garage(ctx, set),
        OpeningStyle::Barn => barn(ctx, set),
        OpeningStyle::Shower => shower(ctx, set, open_angle(ctx)),
        _ => {}
    }
}
