//! Door styles: hinged, double, sliding, pocket, bifold, garage, barn, shower,
//! fixed.

use crate::builder::MeshSet;
use crate::frame::Frame;
use crate::leaf::{Grid, Leaf};
use crate::mesh::Material;
use crate::opening::{ArchGeom, Ctx};
use crate::wall::Hole;
use plan_core::opening_symbol::{bifold_panels, sliding_panels};
use plan_core::openings::{HandleStyle, LiteStyle};
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
/// or a lite style of its own, and lites are shown (a plain `(1, 1)` door
/// stays a solid slab).
fn glazing(ctx: &Ctx) -> Option<(u32, u32)> {
    let (c, r) = ctx.opening.lites;
    let styled = ctx.opening.extras.spec.lite_style != LiteStyle::Standard;
    (ctx.opts.show_lites && c >= 1 && r >= 1 && (styled || u64::from(c) * u64::from(r) > 1))
        .then_some((c, r))
}

/// The lite grid of the Lites tab over `u` x `h`.
fn lite_grid(ctx: &Ctx, lites: (u32, u32), u: (f64, f64), h: (f64, f64)) -> Grid {
    Grid::for_spec(&ctx.opening.extras.spec, lites, u, h)
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
        grid: lite_grid(
            ctx,
            (cols, rows),
            (stile, width - stile),
            (h0 + height * 0.3, h1 - (STILE + 2.0).min(height * 0.1)),
        ),
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

/// Two to four overlapping glazed panels on two tracks (one panel per 4' of
/// width, see [`plan_core::opening_symbol::sliding_panels`]). Panels
/// alternate tracks; every second one is movable and slides over its
/// neighbour when the door is open. With the hinge at the end the layout is
/// mirrored.
fn sliding(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    let w = h.s1 - h.s0;
    let n = sliding_panels(w);
    let step = w / n as f64;
    let pw = step + 1.0;
    let off = (ctx.half() - THIN_PANEL * 0.5).clamp(0.25, 0.75);
    let height = h.h1 - h.h0;
    let lite = Lite {
        grid: Grid::plain((2.0, pw - 2.0), (h.h0 + height * 0.5, h.h1 - 2.0), 1, 1),
        glass: Material::Glass,
        bars: Material::DoorPanel,
    };
    let tt = (-THIN_PANEL * 0.5, THIN_PANEL * 0.5);
    let mirror = ctx.opening.hinge_at_end;
    for k in 0..n {
        // Logical position from the fixed side: 0 is the fixed panel.
        let kk = if mirror { n - 1 - k } else { k };
        let movable = kk % 2 == 1;
        let track = if movable { off } else { -off };
        let at = h.s0 + k as f64 * step - if k > 0 { 0.5 } else { 0.0 };
        let shift = if movable && ctx.opts.doors_open {
            let dir = if mirror { 1.0 } else { -1.0 };
            dir * (step - 1.0)
        } else {
            0.0
        };
        let leaf = Leaf::new((at + shift, track), (1.0, 0.0));
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

/// One pair of panels hinged together: the first hinged to the jamb at `hs`
/// (running toward `dsign` along the wall), the second hinged to its tip.
fn bifold_pair(ctx: &Ctx, set: &mut MeshSet, hs: f64, dsign: f64, panel_w: f64) {
    let h = ctx.hole;
    let a = open_angle(ctx).clamp(0.0, 80.0).to_radians();
    let swing = ctx.swing_sign();
    let first = Leaf::new((hs, 0.0), (dsign * a.cos(), swing * a.sin()));
    let tip = first.st(panel_w, 0.0);
    let second = Leaf::new(tip, (dsign * a.cos(), -swing * a.sin()));
    let t = THIN_PANEL * 0.5;
    for leaf in [first, second] {
        panel(
            &ctx.frame,
            set,
            &leaf,
            (0.0, panel_w),
            (h.h0, h.h1),
            (-t, t),
            None,
        );
    }
}

/// Two panels hinged together (one pair), or four (a pair from each jamb);
/// folded back when the door is open.
fn bifold(ctx: &Ctx, set: &mut MeshSet) {
    let h = ctx.hole;
    let w = h.s1 - h.s0;
    if bifold_panels(w) == 2 {
        let (hs, dsign) = ctx.hinge();
        bifold_pair(ctx, set, hs, dsign, w * 0.5);
    } else {
        bifold_pair(ctx, set, h.s0, 1.0, w * 0.25);
        bifold_pair(ctx, set, h.s1, -1.0, w * 0.25);
    }
}

/// Two hinged leaves, each half the opening, meeting in the middle.
fn double(ctx: &Ctx, set: &mut MeshSet, angle: f64) {
    let h = ctx.hole;
    let half_w = (h.s1 - h.s0) * 0.5;
    let a = angle.to_radians();
    let t = DOOR_THICKNESS * 0.5;
    for (hs, dsign) in [(h.s0, 1.0), (h.s1, -1.0)] {
        let leaf = Leaf::new((hs, 0.0), (dsign * a.cos(), ctx.swing_sign() * a.sin()));
        let lite = door_lite(ctx, half_w);
        panel(
            &ctx.frame,
            set,
            &leaf,
            (0.0, half_w),
            (h.h0, h.h1),
            (-t, t),
            lite.as_ref(),
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
                grid: lite_grid(ctx, (cols, rows), (3.0, w - 3.0), (a + 4.0, b - 3.0)),
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

/// Handle and hinges on a hinged leaf `w` wide (Hardware tab), as simple
/// metal shapes on both faces.
fn hardware_on_leaf(ctx: &Ctx, set: &mut MeshSet, leaf: &Leaf, w: f64, hinges: bool) {
    let hw = ctx.opening.extras.spec.hardware;
    let h = ctx.hole;
    let f = &ctx.frame;
    let door_t = DOOR_THICKNESS * 0.5;
    let metal = set.material(Material::Metal);
    // A `tt` range standing `a..b` off the face on `side`.
    let off = |side: f64, a: f64, b: f64| {
        let (x, y) = (side * (door_t + a), side * (door_t + b));
        (x.min(y), x.max(y))
    };
    if hw.handle != HandleStyle::None {
        let u = w - hw.in_from_edge.clamp(0.5, w * 0.5);
        let hh = (h.h0 + hw.handle_height).clamp(h.h0 + 1.0, (h.h1 - 3.0).max(h.h0 + 1.0));
        for side in [1.0, -1.0] {
            match hw.handle {
                HandleStyle::None => {}
                HandleStyle::Knob => {
                    leaf.boxed(
                        f,
                        metal,
                        (u - 0.4, u + 0.4),
                        off(side, 0.0, 1.5),
                        (hh - 0.4, hh + 0.4),
                    );
                    leaf.boxed(
                        f,
                        metal,
                        (u - 1.0, u + 1.0),
                        off(side, 1.5, 3.5),
                        (hh - 1.0, hh + 1.0),
                    );
                }
                HandleStyle::Lever => {
                    leaf.boxed(
                        f,
                        metal,
                        (u - 1.0, u + 1.0),
                        off(side, 0.0, 0.5),
                        (hh - 1.0, hh + 1.0),
                    );
                    leaf.boxed(
                        f,
                        metal,
                        (u - 5.0, u + 0.5),
                        off(side, 0.5, 1.25),
                        (hh - 0.4, hh + 0.4),
                    );
                }
                HandleStyle::Pull => {
                    leaf.boxed(
                        f,
                        metal,
                        (u - 0.5, u + 0.5),
                        off(side, 2.0, 3.0),
                        (hh - 6.0, hh + 6.0),
                    );
                    for dh in [-5.0, 5.0] {
                        leaf.boxed(
                            f,
                            metal,
                            (u - 0.4, u + 0.4),
                            off(side, 0.0, 2.0),
                            (hh + dh - 0.4, hh + dh + 0.4),
                        );
                    }
                }
            }
        }
    }
    if hinges && hw.hinges > 0 {
        let n = hw.hinges as usize;
        let (lo, hi) = (h.h0 + hw.hinge_inset, h.h1 - hw.hinge_inset);
        for k in 0..n {
            let hh = if n == 1 {
                (h.h0 + h.h1) * 0.5
            } else {
                lo + (hi - lo) * k as f64 / (n - 1) as f64
            };
            leaf.boxed(
                f,
                metal,
                (0.0, 0.6),
                (-door_t - 0.1, door_t + 0.1),
                (hh - 2.0, hh + 2.0),
            );
        }
    }
}

/// Handle and hinges of the door styles that have a leaf to hang them on.
fn hardware(ctx: &Ctx, set: &mut MeshSet, style: OpeningStyle) {
    if !ctx.opening.extras.spec.hardware.enabled {
        return;
    }
    let h = ctx.hole;
    let w = h.s1 - h.s0;
    match style {
        OpeningStyle::Hinged | OpeningStyle::Fixed => {
            let angle = if style == OpeningStyle::Fixed {
                0.0
            } else {
                open_angle(ctx)
            };
            hardware_on_leaf(ctx, set, &hinge_leaf(ctx, angle), w, true);
        }
        OpeningStyle::DoubleDoor => {
            let a = open_angle(ctx).to_radians();
            for (hs, dsign) in [(h.s0, 1.0), (h.s1, -1.0)] {
                let leaf = Leaf::new((hs, 0.0), (dsign * a.cos(), ctx.swing_sign() * a.sin()));
                hardware_on_leaf(ctx, set, &leaf, w * 0.5, true);
            }
        }
        OpeningStyle::Pocket => {
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
            // The latch edge is the one that leads into the pocket.
            let leaf = if toward_end {
                Leaf::new((h.s1 + dir * shift, 0.0), (-1.0, 0.0))
            } else {
                Leaf::new((h.s0 + dir * shift, 0.0), (1.0, 0.0))
            };
            hardware_on_leaf(ctx, set, &leaf, w, false);
        }
        _ => {}
    }
}

/// The glazed arch over the leaves of an arched door (Arch tab): a curved
/// frame band and, unless it is a plain cased doorway, a pane.
fn arch_cap(ctx: &Ctx, set: &mut MeshSet, arch: &ArchGeom, glazed: bool) {
    let f = &ctx.frame;
    let leaf = Leaf::new((0.0, 0.0), (1.0, 0.0));
    let t = (-DOOR_THICKNESS * 0.5, DOOR_THICKNESS * 0.5);
    let band = set.material(Material::DoorPanel);
    for (o, i) in arch.outer.windows(2).zip(arch.inner.windows(2)) {
        leaf.slab(f, band, &[o[0], o[1], i[1], i[0]], t);
    }
    if glazed {
        let g = crate::leaf::GLASS_THICKNESS * 0.5;
        leaf.slab(f, set.material(Material::WindowGlass), &arch.inner, (-g, g));
    }
}

fn build_style(ctx: &Ctx, set: &mut MeshSet, style: OpeningStyle) {
    match style {
        OpeningStyle::Hinged => hinged(ctx, set, open_angle(ctx)),
        OpeningStyle::DoubleDoor => double(ctx, set, open_angle(ctx)),
        OpeningStyle::Fixed => hinged(ctx, set, 0.0),
        OpeningStyle::Sliding => sliding(ctx, set),
        OpeningStyle::Pocket => pocket(ctx, set),
        OpeningStyle::Bifold => bifold(ctx, set),
        OpeningStyle::Garage => garage(ctx, set),
        OpeningStyle::Barn => barn(ctx, set),
        OpeningStyle::Shower => shower(ctx, set, open_angle(ctx)),
        _ => {}
    }
    hardware(ctx, set, style);
}

/// Add the door meshes for the opening's style (a cased doorway adds nothing
/// unless its head is arched). An arched head stops the leaves at the
/// springline and closes the arch above them (a glazed transom).
pub fn build(ctx: &Ctx, set: &mut MeshSet) {
    let style = ctx.style();
    let jamb = ctx.opening.frame_width().max(1.5);
    if let Some(arch) = ctx.arch(jamb) {
        let low = Ctx {
            frame: ctx.frame,
            wall: ctx.wall,
            opening: ctx.opening,
            hole: Hole {
                h1: arch.spring,
                ..ctx.hole
            },
            interior: ctx.interior,
            opts: ctx.opts,
            unit: ctx.unit,
        };
        build_style(&low, set, style);
        arch_cap(ctx, set, &arch, style != OpeningStyle::Doorway);
        return;
    }
    build_style(ctx, set, style);
}
