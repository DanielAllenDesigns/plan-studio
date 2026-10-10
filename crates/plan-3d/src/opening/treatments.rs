//! Window treatments (Treatments tab): curtains, blinds and interior shutters
//! on the room side, exterior millwork above and below the casing on the
//! outside. They are 3D only; the plan does not draw them.

use crate::builder::MeshSet;
use crate::mesh::{Material, Mesh};
use crate::opening::Ctx;
use crate::windows::FRAME_DEPTH;
use plan_core::openings::spec::{
    BlindStyle, CurtainStyle, InteriorShutterStyle, MillworkStyle, Treatments,
};
use plan_core::{Casing, OpeningKind, OpeningStyle};

/// Curtain rod thickness and how far it stands off the wall face.
const ROD: f64 = 0.75;
const ROD_STANDOFF: f64 = 3.0;
/// Thickness of a flat curtain panel.
const CLOTH: f64 = 0.6;
/// Depth of a fold of pleated cloth.
const FOLD: f64 = 1.6;
/// Folds in a pleated panel.
const FOLDS: usize = 6;
/// Slat spacing and thickness of horizontal blinds.
const SLAT_PITCH: f64 = 1.0;
const SLAT: f64 = 0.15;
/// Millwork projection past the casing.
const MILLWORK_DEPTH: f64 = 1.5;
const CORNICE_CAP: f64 = 1.0;
/// Tallest a treatment can be, a bound on the slat count.
const MAX_SLATS: usize = 80;

/// Whether the opening takes window treatments.
fn applies(ctx: &Ctx) -> bool {
    ctx.opening.kind == OpeningKind::Window
        && !matches!(
            ctx.opening.style,
            OpeningStyle::PassThrough
                | OpeningStyle::WallNiche
                | OpeningStyle::BayWindow
                | OpeningStyle::BowWindow
                | OpeningStyle::BoxWindow
        )
}

/// The meshes of the opening's treatments: `(interior colored meshes,
/// exterior millwork meshes)`; the first carry their colors, the millwork is
/// plain trim for the Casing paint to take.
pub fn build(ctx: &Ctx) -> (Vec<Mesh>, Vec<Mesh>) {
    let t = ctx.opening.extras.spec.treatments;
    if !applies(ctx) || !t.any() {
        return (Vec::new(), Vec::new());
    }
    let mut inside = Vec::new();
    if t.any_interior() {
        let id = Some(ctx.opening.id);
        if t.curtain != CurtainStyle::None {
            let mut set = MeshSet::default();
            curtains(ctx, &t, &mut set);
            inside.extend(colored(set.finish(id), t.curtain_color));
        }
        if t.blind != BlindStyle::None {
            let mut set = MeshSet::default();
            blinds(ctx, &t, &mut set);
            inside.extend(colored(set.finish(id), t.blind_color));
        }
        if t.shutter != InteriorShutterStyle::None {
            let mut set = MeshSet::default();
            shutters(ctx, &t, &mut set);
            inside.extend(colored(set.finish(id), t.shutter_color));
        }
    }
    let mut set = MeshSet::default();
    millwork(ctx, &t, &mut set);
    (inside, set.finish(Some(ctx.opening.id)))
}

fn colored(meshes: Vec<Mesh>, rgb: [u8; 3]) -> impl Iterator<Item = Mesh> {
    meshes.into_iter().map(move |mut m| {
        m.color = Some(rgb);
        m
    })
}

/// Wall-local `t` range from the room-side face out to `depth` past it,
/// starting `from` in front of the face.
fn room_t(ctx: &Ctx, from: f64, depth: f64) -> (f64, f64) {
    let face = ctx.half();
    let (a, b) = (face + from, face + from + depth);
    if ctx.interior > 0.0 {
        (a, b)
    } else {
        (-b, -a)
    }
}

/// The casing width plus reveal on the room side (zero without casing).
fn casing_reach(ctx: &Ctx) -> f64 {
    if ctx.opening.extras.spec.casing_interior {
        let c = ctx.opening.casing.unwrap_or_default();
        c.width + c.reveal
    } else {
        0.0
    }
}

fn curtains(ctx: &Ctx, t: &Treatments, set: &mut MeshSet) {
    let h = ctx.hole;
    let len = ctx.wall.length();
    let reach = casing_reach(ctx);
    let (lo, hi) = (h.s0 - reach, h.s1 + reach);
    let head = h.h1 + reach;
    let rod_h = head + t.curtain_above_casing.max(0.0);
    let bottom = t.curtain_off_floor.clamp(0.0, rod_h - 6.0);
    let f = &ctx.frame;
    let trim = set.material(Material::Trim);
    // The rod runs past the panels by a finial's length.
    let (rs0, rs1) = ((lo - 6.0).max(0.0), (hi + 6.0).min(len));
    f.cuboid(
        trim,
        (rs0, rs1),
        room_t(ctx, ROD_STANDOFF, ROD),
        (rod_h - ROD, rod_h),
    );
    match t.curtain {
        CurtainStyle::None => {}
        CurtainStyle::Valance => {
            let drop = 8.0_f64.min(rod_h - bottom);
            f.cuboid(
                trim,
                (rs0 + 2.0, rs1 - 2.0),
                room_t(ctx, ROD_STANDOFF - 0.5, 2.0),
                (rod_h - ROD - drop, rod_h),
            );
        }
        CurtainStyle::Panels | CurtainStyle::Pleated => {
            let width = ((h.s1 - h.s0) * 0.4).clamp(12.0, 36.0);
            let panels = [(lo - width, lo + 1.0), (hi - 1.0, hi + width)];
            for (a, b) in panels {
                let (a, b) = (a.max(0.0), b.min(len));
                if b - a < 2.0 {
                    continue;
                }
                let top = rod_h - ROD;
                if t.curtain == CurtainStyle::Panels {
                    f.cuboid(
                        trim,
                        (a, b),
                        room_t(ctx, ROD_STANDOFF - 0.3, CLOTH),
                        (bottom, top),
                    );
                } else {
                    let step = (b - a) / FOLDS as f64;
                    for k in 0..FOLDS {
                        let x = a + step * k as f64;
                        let back = if k % 2 == 0 { 0.0 } else { FOLD };
                        f.cuboid(
                            trim,
                            (x, x + step),
                            room_t(ctx, ROD_STANDOFF - 0.3 + back, CLOTH),
                            (bottom, top),
                        );
                    }
                }
            }
        }
    }
}

fn blinds(ctx: &Ctx, t: &Treatments, set: &mut MeshSet) {
    let h = ctx.hole;
    let fw = ctx.opening.frame_width().max(0.5);
    let (s0, s1) = (h.s0 + fw, h.s1 - fw);
    let (lo, hi) = (h.h0 + fw, h.h1 - fw);
    if s1 - s0 < 3.0 || hi - lo < 3.0 {
        return;
    }
    // Inside the recess, just in front of the window frame's room side.
    let frame_in = (FRAME_DEPTH * 0.5).min(ctx.half());
    let from = frame_in - ctx.half() + 0.1;
    let f = &ctx.frame;
    let trim = set.material(Material::Trim);
    let drop = ((hi - lo) * t.blind_lowered.clamp(0.0, 1.0)).max(1.5);
    match t.blind {
        BlindStyle::None => {}
        BlindStyle::Horizontal => {
            f.cuboid(trim, (s0, s1), room_t(ctx, from, 1.2), (hi - 1.5, hi));
            let n = (((drop - 1.5) / SLAT_PITCH).floor() as usize).min(MAX_SLATS);
            for k in 0..n {
                let y = hi - 1.5 - (k as f64 + 0.5) * SLAT_PITCH;
                f.cuboid(
                    trim,
                    (s0 + 0.2, s1 - 0.2),
                    room_t(ctx, from, 1.0),
                    (y - SLAT * 0.5, y + SLAT * 0.5),
                );
            }
            // The bottom rail.
            let y = hi - 1.5 - n as f64 * SLAT_PITCH;
            f.cuboid(
                trim,
                (s0 + 0.2, s1 - 0.2),
                room_t(ctx, from, 1.0),
                (y - 0.5, y),
            );
        }
        BlindStyle::Vertical => {
            f.cuboid(trim, (s0, s1), room_t(ctx, from, 1.2), (hi - 1.5, hi));
            let pitch = 3.2;
            let n = (((s1 - s0) / pitch).floor() as usize).clamp(1, MAX_SLATS);
            let step = (s1 - s0) / n as f64;
            for k in 0..n {
                let x = s0 + step * k as f64;
                f.cuboid(
                    trim,
                    (x + 0.1, x + step - 0.1),
                    room_t(ctx, from, 0.3),
                    (lo, hi - 1.5),
                );
            }
        }
        BlindStyle::Roller => {
            // The roller tube, and the shade hanging from it.
            f.cuboid(trim, (s0, s1), room_t(ctx, from, 2.0), (hi - 2.0, hi));
            f.cuboid(
                trim,
                (s0 + 0.1, s1 - 0.1),
                room_t(ctx, from + 0.2, 0.1),
                (hi - drop, hi - 2.0),
            );
        }
    }
}

fn shutters(ctx: &Ctx, t: &Treatments, set: &mut MeshSet) {
    let h = ctx.hole;
    let fw = ctx.opening.frame_width().max(0.5);
    let (s0, s1) = (h.s0 + fw, h.s1 - fw);
    let (glass_lo, glass_hi) = (h.h0 + fw, h.h1 - fw);
    // Cafe leaves cover the lower half of the glass.
    let (lo, hi) = match t.shutter {
        InteriorShutterStyle::Cafe => (glass_lo, glass_lo + (glass_hi - glass_lo) * 0.5),
        _ => (glass_lo, glass_hi),
    };
    if s1 - s0 < 6.0 || hi - lo < 6.0 {
        return;
    }
    let frame_in = (FRAME_DEPTH * 0.5).min(ctx.half());
    let from = frame_in - ctx.half() + 0.05;
    let thick = 1.25;
    let f = &ctx.frame;
    let trim = set.material(Material::Trim);
    let mid = (s0 + s1) * 0.5;
    let leaf_w = (mid - s0).max(1.0);
    for k in 0..2 {
        let (a, b) = if k == 0 { (s0, mid) } else { (mid, s1) };
        if t.shutter_closed {
            let stile = 2.0_f64.min(leaf_w * 0.3);
            let tt = room_t(ctx, from, thick);
            f.cuboid(trim, (a, a + stile), tt, (lo, hi));
            f.cuboid(trim, (b - stile, b), tt, (lo, hi));
            f.cuboid(trim, (a + stile, b - stile), tt, (lo, lo + 2.5));
            f.cuboid(trim, (a + stile, b - stile), tt, (hi - 2.5, hi));
            // Louvers across the middle of the leaf.
            let room = (hi - lo) - 5.0;
            let n = ((room / 1.5).floor() as usize).min(MAX_SLATS);
            for j in 0..n {
                let y = lo + 2.5 + (j as f64 + 0.5) * room / n.max(1) as f64;
                f.cuboid(
                    trim,
                    (a + stile, b - stile),
                    room_t(ctx, from + 0.1, thick * 0.6),
                    (y - 0.2, y + 0.2),
                );
            }
        } else {
            // Folded back against the jamb, standing out into the room.
            let (x0, x1) = if k == 0 {
                (a, a + thick)
            } else {
                (b - thick, b)
            };
            f.cuboid(
                trim,
                (x0, x1),
                room_t(ctx, from, leaf_w.min(24.0)),
                (lo, hi),
            );
        }
    }
}

/// The exterior casing of the opening, for the millwork to sit on.
fn outside_casing(ctx: &Ctx) -> Casing {
    let c = ctx.opening.casing.unwrap_or_default();
    if ctx.wall.kind == plan_core::WallKind::Exterior {
        ctx.opening.extras.spec.casing_exterior_size.unwrap_or(c)
    } else {
        c
    }
}

fn millwork(ctx: &Ctx, t: &Treatments, set: &mut MeshSet) {
    if t.millwork_above == MillworkStyle::None && t.millwork_below == MillworkStyle::None {
        return;
    }
    let h = ctx.hole;
    let len = ctx.wall.length();
    let half = ctx.half();
    let c = outside_casing(ctx);
    let reach = if ctx.opening.extras.spec.casing_exterior {
        c.width + c.reveal
    } else {
        0.0
    };
    let exterior = -ctx.interior;
    let depth = if ctx.opening.extras.spec.casing_exterior {
        c.depth
    } else {
        0.0
    };
    let tr = |extra: f64| {
        if exterior > 0.0 {
            (half, half + depth + MILLWORK_DEPTH + extra)
        } else {
            (-half - depth - MILLWORK_DEPTH - extra, -half)
        }
    };
    let mid = (h.s0 + h.s1) * 0.5;
    let f = &ctx.frame;
    let trim = set.material(Material::Trim);
    match t.millwork_above {
        MillworkStyle::None | MillworkStyle::Apron => {}
        style => {
            let width = if t.millwork_above_width > 0.0 {
                t.millwork_above_width
            } else {
                (h.s1 - h.s0) + 2.0 * reach
            };
            let s = ((mid - width * 0.5).max(0.0), (mid + width * 0.5).min(len));
            let base = h.h1 + reach;
            let board = (
                base,
                (base + t.millwork_above_height.max(1.0)).min(ctx.wall.height),
            );
            if board.1 - board.0 > 1e-6 && s.1 - s.0 > 1e-6 {
                f.cuboid(trim, s, tr(0.0), board);
                if style == MillworkStyle::Cornice {
                    let cap = (board.1, (board.1 + CORNICE_CAP).min(ctx.wall.height));
                    let wide = ((s.0 - 1.0).max(0.0), (s.1 + 1.0).min(len));
                    if cap.1 - cap.0 > 1e-6 {
                        f.cuboid(trim, wide, tr(CORNICE_CAP), cap);
                    }
                }
            }
        }
    }
    if t.millwork_below == MillworkStyle::Apron && h.h0 > 3.0 {
        let width = (h.s1 - h.s0) + 2.0 * (reach + t.millwork_below_extend.max(0.0));
        let s = ((mid - width * 0.5).max(0.0), (mid + width * 0.5).min(len));
        let top = (h.h0 - 0.75 - reach).max(0.0);
        let board = ((top - t.millwork_below_height.max(1.0)).max(0.0), top);
        if board.1 - board.0 > 1e-6 && s.1 - s.0 > 1e-6 {
            f.cuboid(trim, s, tr(0.0), board);
        }
    }
}
