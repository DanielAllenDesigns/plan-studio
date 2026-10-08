//! Casing, window sill and door threshold boards (Chief "Casing" / "Jamb").

use crate::builder::MeshSet;
use crate::mesh::Material;
use crate::opening::Ctx;
use plan_core::openings::spec::SHUTTER_THICKNESS;
use plan_core::openings::CasingProfile;
use plan_core::openings::{LintelStyle, ShutterStyle};
use plan_core::{Casing, OpeningKind, OpeningStyle, WallKind};

/// Depth of a window's own frame through the wall (see `windows.rs`).
const WINDOW_FRAME_DEPTH: f64 = 3.5;
/// Sill (stool) thickness.
const SILL_THICKNESS: f64 = 0.75;
/// How far the sill projects past the interior face.
const SILL_PROJECTION: f64 = 1.25;
/// Threshold height.
const THRESHOLD_HEIGHT: f64 = 0.75;

/// Cap board over a casing head: how far it stands beyond the boards, and its
/// thickness.
const CAP_OVERHANG: f64 = 0.75;
const CAP_HEIGHT: f64 = 1.0;
/// Plinth block height and how much wider than a leg it is.
const PLINTH_HEIGHT: f64 = 6.0;
const PLINTH_EXTRA: f64 = 0.75;
/// Narrowest jamb board drawn.
const JAMB_MIN: f64 = 0.5;

/// Casing boards on both faces of the wall around the opening, plus the sill
/// (windows) and apron below it. A mulled unit has one casing around the
/// whole unit: each member draws the leg on its free end, the first member
/// the head, sill and apron across all of them (DW-52); a window standing
/// over a door (a transom) draws none of its own. The interior face takes
/// the interior casing (width, depth, reveal), the outside face of an
/// exterior wall the exterior casing; the profile (Casing tab) adds a head
/// cap or plinth blocks.
pub fn add_casing(ctx: &Ctx, set: &mut MeshSet) {
    if ctx.unit.covered {
        return;
    }
    let interior_c = ctx.opening.casing.unwrap_or_default();
    let spec = &ctx.opening.extras.spec;
    let exterior_c = if ctx.wall.kind == WallKind::Exterior {
        spec.casing_exterior_size.unwrap_or(interior_c)
    } else {
        interior_c
    };
    let (len, height) = (ctx.wall.length(), ctx.wall.height);
    let half = ctx.half();
    let unit = ctx.unit;
    let window = !unit.door && ctx.opening.kind == OpeningKind::Window;
    let (u0, u1) = unit.span;
    let bottom = unit.h.0;
    let top = unit.h.1;
    let trim = set.material(Material::Trim);
    for side in [1.0, -1.0] {
        let is_interior = side == ctx.interior;
        if (is_interior && !spec.casing_interior) || (!is_interior && !spec.casing_exterior) {
            continue;
        }
        let Casing {
            width,
            depth,
            reveal,
        } = if is_interior { interior_c } else { exterior_c };
        let s_lo = (u0 - reveal - width).max(0.0);
        let s_hi = (u1 + reveal + width).min(len);
        let leg_bottom = if window { bottom } else { bottom.max(0.0) };
        let head = (top + reveal, (top + reveal + width).min(height));
        let t = if side > 0.0 {
            (half, half + depth)
        } else {
            (-half - depth, -half)
        };
        let mut legs = Vec::new();
        if !unit.left {
            legs.push(((s_lo, (u0 - reveal).max(0.0)), (leg_bottom, head.0)));
        }
        if !unit.right {
            legs.push((((u1 + reveal).min(len), s_hi), (leg_bottom, head.0)));
        }
        for (s, hh) in &legs {
            if s.1 - s.0 > 1e-6 && hh.1 - hh.0 > 1e-6 {
                ctx.frame.cuboid(trim, *s, t, *hh);
            }
        }
        if !unit.draws_head() {
            continue;
        }
        if head.1 - head.0 > 1e-6 {
            ctx.frame.cuboid(trim, (s_lo, s_hi), t, head);
        }
        let proud = if side > 0.0 {
            (half, half + depth + CAP_OVERHANG)
        } else {
            (-half - depth - CAP_OVERHANG, -half)
        };
        match spec.casing_profile {
            CasingProfile::Flat => {}
            CasingProfile::Cap => {
                let cap = (head.1, (head.1 + CAP_HEIGHT).min(height));
                let wide = (
                    (s_lo - CAP_OVERHANG).max(0.0),
                    (s_hi + CAP_OVERHANG).min(len),
                );
                if cap.1 - cap.0 > 1e-6 {
                    ctx.frame.cuboid(trim, wide, proud, cap);
                }
            }
            CasingProfile::Plinth => {
                // A block at the foot of each leg that reaches the floor and
                // one at each head corner.
                for (s, hh) in &legs {
                    let wide = (
                        (s.0 - PLINTH_EXTRA * 0.5).max(0.0),
                        (s.1 + PLINTH_EXTRA * 0.5).min(len),
                    );
                    if hh.0 <= 1e-6 {
                        ctx.frame
                            .cuboid(trim, wide, proud, (0.0, PLINTH_HEIGHT.min(hh.1)));
                    }
                    if head.1 - head.0 > 1e-6 {
                        ctx.frame.cuboid(trim, wide, proud, head);
                    }
                }
            }
        }
        if window {
            let apron = (
                bottom - SILL_THICKNESS - width.min(bottom - SILL_THICKNESS),
                bottom - SILL_THICKNESS,
            );
            if apron.1 - apron.0 > 1e-6 && ctx.opening.style != OpeningStyle::PassThrough {
                ctx.frame
                    .cuboid(trim, (s_lo, s_hi), t, (apron.0.max(0.0), apron.1));
            }
            // The stool: on the room side it projects into the room; the
            // outside of an exterior wall gets one standing off its casing.
            if bottom >= SILL_THICKNESS && (is_interior || ctx.wall.kind == WallKind::Exterior) {
                let reach = if is_interior {
                    SILL_PROJECTION
                } else {
                    depth + SILL_PROJECTION
                };
                let ts = if side > 0.0 {
                    (-half, half + reach)
                } else {
                    (-half - reach, half)
                };
                ctx.frame
                    .cuboid(trim, (s_lo, s_hi), ts, (bottom - SILL_THICKNESS, bottom));
            }
        }
    }
}

/// The half-thickness of the leaf a door's jamb boards stand clear of.
fn leaf_half(style: OpeningStyle) -> f64 {
    match style {
        OpeningStyle::Garage => 1.0,
        OpeningStyle::Sliding => 1.5,
        OpeningStyle::Bifold => 0.5,
        _ => crate::doors::DOOR_THICKNESS * 0.5,
    }
}

/// Jamb boards lining the hole through the whole thickness of the wall, so the
/// frame is as deep as the wall and its layers (Chief "Jamb" / "Frame"): legs
/// and head for a door, standing clear of its leaf; legs, head and sill for a
/// window, outside the depth of the window frame itself. Not between mulled
/// members, not over an arched head, not for a barn or shower door.
pub fn add_jambs(ctx: &Ctx, set: &mut MeshSet) {
    let o = ctx.opening;
    let style = ctx.style();
    if matches!(
        style,
        OpeningStyle::PassThrough
            | OpeningStyle::WallNiche
            | OpeningStyle::Barn
            | OpeningStyle::Shower
    ) {
        return;
    }
    let h = ctx.hole;
    let half = ctx.half();
    let jw = o
        .frame_width()
        .max(JAMB_MIN)
        .min((h.s1 - h.s0) * 0.25)
        .min((h.h1 - h.h0) * 0.25);
    // The depth the opening's own frame or leaf occupies at the middle.
    // A projecting window has no frame in the wall: its liner is one board
    // through the whole thickness.
    let projecting = matches!(
        style,
        OpeningStyle::BayWindow | OpeningStyle::BowWindow | OpeningStyle::BoxWindow
    );
    let inner = match o.kind {
        _ if projecting => 0.0,
        OpeningKind::Door => leaf_half(style),
        OpeningKind::Window => WINDOW_FRAME_DEPTH.min(ctx.wall.thickness) * 0.5,
    };
    if half - inner < 1e-3 {
        return;
    }
    let faces: &[(f64, f64)] = if projecting {
        &[(-half, half)]
    } else {
        &[(-half, -inner), (inner, half)]
    };
    let trim = set.material(Material::Trim);
    let arched = ctx.arch(jw).is_some();
    let unit = ctx.unit;
    for &t in faces {
        if !unit.left {
            ctx.frame.cuboid(trim, (h.s0, h.s0 + jw), t, (h.h0, h.h1));
        }
        if !unit.right {
            ctx.frame.cuboid(trim, (h.s1 - jw, h.s1), t, (h.h0, h.h1));
        }
        if !arched && !unit.capped {
            ctx.frame
                .cuboid(trim, (h.s0 + jw, h.s1 - jw), t, (h.h1 - jw, h.h1));
        }
        if o.kind == OpeningKind::Window && h.h0 > 1e-6 && !unit.covered {
            ctx.frame
                .cuboid(trim, (h.s0 + jw, h.s1 - jw), t, (h.h0, h.h0 + jw));
        }
    }
}

/// Lintel boards over the head on the faces that ask for one (Lintel tab).
pub fn add_lintel(ctx: &Ctx, set: &mut MeshSet) {
    let l = ctx.opening.extras.spec.lintel;
    if !l.any() || ctx.opening.style == OpeningStyle::WallNiche {
        return;
    }
    let c = ctx.opening.casing.unwrap_or_default();
    let (len, height) = (ctx.wall.length(), ctx.wall.height);
    let half = ctx.half();
    let h = ctx.hole;
    // On top of the casing head when casing is drawn, else on the hole.
    let base = h.h1 + c.reveal + if ctx.opts.show_casing { c.width } else { 0.0 };
    let reach = c.width + c.reveal + l.extend;
    let s = ((h.s0 - reach).max(0.0), (h.s1 + reach).min(len));
    let trim = set.material(Material::Trim);
    let exterior = -ctx.interior;
    for (side, wanted) in [(exterior, l.exterior), (ctx.interior, l.interior)] {
        if !wanted {
            continue;
        }
        let depth = |extra: f64| {
            if side > 0.0 {
                (half, half + l.depth + extra)
            } else {
                (-half - l.depth - extra, -half)
            }
        };
        let board = (base, (base + l.height).min(height));
        if board.1 - board.0 <= 1e-6 {
            continue;
        }
        ctx.frame.cuboid(trim, s, depth(0.0), board);
        match l.style {
            LintelStyle::Flat => {}
            LintelStyle::Cap => {
                let cap = (board.1, (board.1 + 1.0).min(height));
                let wide = ((s.0 - 0.75).max(0.0), (s.1 + 0.75).min(len));
                if cap.1 - cap.0 > 1e-6 {
                    ctx.frame.cuboid(trim, wide, depth(0.75), cap);
                }
            }
            LintelStyle::Keystone => {
                let mid = (h.s0 + h.s1) * 0.5;
                let kw = (l.height * 0.8).max(1.0);
                let key = (board.0, (board.1 + l.height * 0.4).min(height));
                ctx.frame
                    .cuboid(trim, (mid - kw * 0.5, mid + kw * 0.5), depth(0.5), key);
            }
        }
    }
}

/// The exterior sill under a window (Lintel tab, Exterior Sill).
pub fn add_exterior_sill(ctx: &Ctx, set: &mut MeshSet) {
    let sill = ctx.opening.extras.spec.sill;
    if !sill.enabled || ctx.opening.kind != OpeningKind::Window || ctx.hole.h0 < 0.5 {
        return;
    }
    let c = ctx.opening.casing.unwrap_or_default();
    let half = ctx.half();
    let h = ctx.hole;
    let reach = c.width + c.reveal + sill.extend;
    let s = (
        (h.s0 - reach).max(0.0),
        (h.s1 + reach).min(ctx.wall.length()),
    );
    let exterior = -ctx.interior;
    let t = if exterior > 0.0 {
        (half - 0.5, half + sill.depth)
    } else {
        (-half - sill.depth, -half + 0.5)
    };
    let lo = (h.h0 - sill.height).max(0.0);
    ctx.frame
        .cuboid(set.material(Material::Trim), s, t, (lo, h.h0));
}

/// Exterior shutters beside or over the opening (Shutters tab): panel
/// shutters as a board with raised panels, louvered ones as a frame with
/// slats. Only on exterior walls, on the outside face.
pub fn add_shutters(ctx: &Ctx, set: &mut MeshSet) {
    let sh = ctx.opening.extras.spec.shutters;
    if !sh.present() || ctx.wall.kind != WallKind::Exterior {
        return;
    }
    let half = ctx.half();
    let h = ctx.hole;
    let c = ctx.opening.casing.unwrap_or_default();
    let len = ctx.wall.length();
    let exterior = -ctx.interior;
    let thick = SHUTTER_THICKNESS;
    let (t_back, t_front) = if exterior > 0.0 {
        (
            (half, half + thick * 0.5),
            (half + thick * 0.5, half + thick),
        )
    } else {
        (
            (-half - thick * 0.5, -half),
            (-half - thick, -half - thick * 0.5),
        )
    };
    let trim = set.material(Material::Trim);
    for (a, b) in sh.spans(h.s0, h.s1, c.width + c.reveal) {
        let (a, b) = (a.max(0.0), b.min(len));
        if b - a < 1.0 {
            continue;
        }
        let (lo, hi) = (h.h0, h.h1);
        match sh.style {
            ShutterStyle::None => {}
            ShutterStyle::Panel => {
                ctx.frame.cuboid(trim, (a, b), t_back, (lo, hi));
                // Two raised panels, upper and lower, inside a 2 1/2" border.
                let border = 2.5_f64.min((b - a) * 0.25);
                let mid = (lo + hi) * 0.5;
                for (p0, p1) in [(lo + 3.0, mid - 1.0), (mid + 1.0, hi - 3.0)] {
                    if p1 - p0 > 1.0 {
                        ctx.frame
                            .cuboid(trim, (a + border, b - border), t_front, (p0, p1));
                    }
                }
            }
            ShutterStyle::Louver => {
                let border = 2.5_f64.min((b - a) * 0.25);
                ctx.frame.cuboid(trim, (a, a + border), t_back, (lo, hi));
                ctx.frame.cuboid(trim, (b - border, b), t_back, (lo, hi));
                ctx.frame
                    .cuboid(trim, (a + border, b - border), t_back, (lo, lo + 3.0));
                ctx.frame
                    .cuboid(trim, (a + border, b - border), t_back, (hi - 3.0, hi));
                let pitch = (sh.louver_size * 2.0).max(0.5);
                let room = (hi - lo) - 6.0;
                let n = (room / pitch).floor().max(0.0) as usize;
                for k in 0..n {
                    let y = lo + 3.0 + (k as f64 + 0.5) * room / n as f64;
                    ctx.frame.cuboid(
                        trim,
                        (a + border, b - border),
                        (t_back.0, t_front.1),
                        (y - sh.louver_size * 0.25, y + sh.louver_size * 0.25),
                    );
                }
            }
        }
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
