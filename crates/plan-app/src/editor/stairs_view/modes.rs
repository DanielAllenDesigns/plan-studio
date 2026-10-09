//! Edit modes and the extra handles of the stair, and the section commands
//! (CB-108, CB-133, CB-144..CB-147, CB-163 in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! * **Flare/Curve Stairs** and **Starter Tread** are edit modes of a
//!   selected straight-family stair: the command switches the stair's
//!   handles to the mode's own and runs again to leave it. The mode is a
//!   property of the editing session, not of the plan, so it lives here and
//!   not in the file.
//! * **Inner / Outer Radius** handles resize a curved stair or ramp from
//!   either edge of its first riser.
//! * **Complete Break** and **Disconnect Selected Subsection** turn one stair
//!   object into separate sections (flights and landings), joined as the
//!   Landing tool joins them.

use super::*;
use std::cell::Cell;

/// What the handles of the selected stair do.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EditMode {
    /// Move, rotate, run and width.
    #[default]
    Normal,
    /// Corner flares, their start and softening, and the tread curve.
    FlareCurve,
    /// The starter treads of the bottom steps.
    StarterTread,
}

thread_local! {
    static MODE: Cell<Option<(Id, EditMode)>> = const { Cell::new(None) };
}

/// The mode stair `id` is being edited in.
pub fn edit_mode(id: Id) -> EditMode {
    MODE.with(|m| match m.get() {
        Some((i, mode)) if i == id => mode,
        _ => EditMode::Normal,
    })
}

/// Puts stair `id` in `mode` (another stair leaves its own mode).
pub fn set_edit_mode(id: Id, mode: EditMode) {
    MODE.with(|m| {
        m.set((mode != EditMode::Normal).then_some((id, mode)));
    });
}

/// Flights of straight steps: the shapes the flare and starter modes edit.
pub fn straight_family(o: &StairObj) -> bool {
    !o.is_landing()
        && !o.is_ramp()
        && !o.is_curved()
        && !matches!(o.stair.params.shape, StairShape::Landing { .. })
}

/// A curved stair or a curved ramp.
pub fn has_arc(o: &StairObj) -> bool {
    o.is_curved() || (o.is_ramp() && o.stair.params.ramp_curve.is_some())
}

fn handle(kind: StairHandleKind, pos: Point, cursor: CursorIcon) -> StairHandle {
    StairHandle { kind, pos, cursor }
}

/// The handles of the stair in a mode: the Move handle and the mode's own.
pub(super) fn mode_handles(obj: &StairObj, mode: EditMode) -> Vec<StairHandle> {
    let o = obj.stair.origin;
    let (along, right) = (obj.along(), obj.right());
    let p = &obj.stair.params;
    let w = p.width;
    let mut out = vec![handle(
        StairHandleKind::Move,
        polygon_centroid(&obj.footprint()),
        CursorIcon::Move,
    )];
    match mode {
        EditMode::Normal => {}
        EditMode::FlareCurve => {
            let fl = &p.flare_shape;
            let len = obj.first_flight_len();
            out.push(handle(
                StairHandleKind::Flare(0),
                o - right * fl.corners[0],
                CursorIcon::Crosshair,
            ));
            out.push(handle(
                StairHandleKind::Flare(1),
                o + right * (w + fl.corners[1]),
                CursorIcon::Crosshair,
            ));
            if p.shape == StairShape::Straight {
                let top = o + along * len;
                out.push(handle(
                    StairHandleKind::Flare(2),
                    top - right * fl.corners[2],
                    CursorIcon::Crosshair,
                ));
                out.push(handle(
                    StairHandleKind::Flare(3),
                    top + right * (w + fl.corners[3]),
                    CursorIcon::Crosshair,
                ));
            }
            let zone = flare_zone(obj);
            out.push(handle(
                StairHandleKind::FlareStart,
                o + along * zone - right * 0.0,
                CursorIcon::ResizeVertical,
            ));
            out.push(handle(
                StairHandleKind::FlareSoften,
                o + along * (zone * fl.soften.clamp(0.0, 1.0)) + right * 6.0,
                CursorIcon::ResizeVertical,
            ));
            out.push(handle(
                StairHandleKind::CurveAll,
                o - along * fl.curve_all + right * (w * 0.5),
                CursorIcon::ResizeVertical,
            ));
        }
        EditMode::StarterTread => {
            out.push(handle(
                StairHandleKind::Starter,
                o - along * p.nosing + right * (w * 0.5),
                CursorIcon::PointingHand,
            ));
        }
    }
    out
}

/// The length along the first flight over which a flare tapers off.
fn flare_zone(o: &StairObj) -> f64 {
    let len = o.first_flight_len().max(1.0);
    let s = o.stair.params.flare_shape.start;
    if s > 1e-9 {
        len * s.min(1.0)
    } else {
        len
    }
}

/// The inner and outer edge handles of a curved stair or ramp, at its first
/// riser.
pub(super) fn radius_handles(obj: &StairObj) -> Vec<StairHandle> {
    let o = obj.stair.origin;
    let (along, right) = (obj.along(), obj.right());
    let w = obj.stair.params.width;
    let left_edge = o + along * 6.0;
    let right_edge = o + along * 6.0 + right * w;
    let (inner, outer) = if obj.stair.params.turn == Turn::Left {
        (left_edge, right_edge)
    } else {
        (right_edge, left_edge)
    };
    let mut out = Vec::new();
    // A spiral's inside edge is its pole: only the outside radius moves.
    if !obj.stair.params.spiral {
        out.push(handle(
            StairHandleKind::InnerRadius,
            inner,
            CursorIcon::ResizeHorizontal,
        ));
    }
    out.push(handle(
        StairHandleKind::OuterRadius,
        outer,
        CursorIcon::ResizeHorizontal,
    ));
    out
}

/// The inside radius of a curved stair or ramp.
fn inner_radius(o: &StairObj) -> f64 {
    match (o.stair.params.shape, o.stair.params.ramp_curve) {
        (StairShape::Curved { inner_radius }, _) => inner_radius,
        (_, Some(r)) => r,
        _ => 0.0,
    }
}

fn put_inner_radius(o: &mut StairObj, r: f64) {
    match &mut o.stair.params.shape {
        StairShape::Curved { inner_radius } => *inner_radius = r,
        _ => o.stair.params.ramp_curve = Some(r),
    }
}

/// Resizes the arc so that its inside (`inner`) or outside edge is `r` from
/// the centre `c`, keeping the other edge and the centre where they are.
fn set_radius(o: &mut StairObj, c: Point, inner: bool, r: f64) {
    let (inner0, w0) = (inner_radius(o), o.stair.params.width);
    let outer0 = inner0 + w0;
    let (inner1, w1) = if inner {
        let i = r.clamp(0.0, (outer0 - MIN_SIZE).max(0.0));
        (i, outer0 - i)
    } else {
        (inner0, (r - inner0).max(MIN_SIZE))
    };
    put_inner_radius(o, inner1);
    o.stair.params.width = w1;
    let right = o.right();
    o.stair.origin = match o.stair.params.turn {
        Turn::Left => c + right * inner1,
        Turn::Right => c - right * (w1 + inner1),
    };
}

/// The undo label of dragging a handle of a mode or an arc.
pub(super) fn label(kind: StairHandleKind) -> Option<&'static str> {
    Some(match kind {
        StairHandleKind::InnerRadius => "Resize Inside Radius",
        StairHandleKind::OuterRadius => "Resize Outside Radius",
        StairHandleKind::Flare(_) => "Flare Stairs",
        StairHandleKind::FlareStart => "Move Flare Start",
        StairHandleKind::FlareSoften => "Soften Flare",
        StairHandleKind::CurveAll => "Curve Treads",
        StairHandleKind::Starter => "Starter Treads",
        _ => return None,
    })
}

/// The stair after dragging a mode or arc handle `kind` from `start` to `to`;
/// `orig` is the stair as it was when the drag began.
pub(super) fn drag(o: &mut StairObj, orig: &StairObj, kind: StairHandleKind, start: Point, to: Point) {
    let (along, right) = (orig.along(), orig.right());
    let at = to - orig.stair.origin;
    let w = orig.stair.params.width;
    let t = orig.stair.params.tread_depth;
    match kind {
        StairHandleKind::InnerRadius | StairHandleKind::OuterRadius => {
            if let Some(c) = plan_stairs::curve_center(&orig.stair) {
                set_radius(o, c, kind == StairHandleKind::InnerRadius, to.dist(c));
            }
        }
        StairHandleKind::Flare(i) => {
            let lat = at.dot(right);
            let reach = match i {
                0 | 2 => -lat,
                _ => lat - w,
            };
            if let Some(c) = o.stair.params.flare_shape.corners.get_mut(i as usize) {
                *c = reach.clamp(0.0, MAX_FLARE);
            }
        }
        StairHandleKind::FlareStart => {
            let len = orig.first_flight_len().max(1.0);
            let frac = (at.dot(along) / len).clamp(0.1, 1.0);
            o.stair.params.flare_shape.start = if frac > 0.99 { 0.0 } else { frac };
        }
        StairHandleKind::FlareSoften => {
            let zone = flare_zone(orig).max(1.0);
            o.stair.params.flare_shape.soften = (at.dot(along) / zone).clamp(0.0, 1.0);
        }
        StairHandleKind::CurveAll => {
            o.stair.params.flare_shape.curve_all = (-at.dot(along)).clamp(0.0, t.max(0.0));
        }
        StairHandleKind::Starter => {
            let d = (to - start).dot(-along);
            let from = orig.stair.params.starter;
            o.stair.params.starter = if (to - start).length() < 3.0 {
                // A click goes round: none, one, two.
                match from {
                    Starter::None => Starter::One,
                    Starter::One => Starter::Two,
                    Starter::Two => Starter::None,
                }
            } else if d < 0.5 * t {
                Starter::None
            } else if d < 1.5 * t {
                Starter::One
            } else {
                Starter::Two
            };
        }
        _ => {}
    }
}

/// The farthest a flared corner stands out from the stair, inches.
pub const MAX_FLARE: f64 = 36.0;

// ----- sections -----

/// Replaces stair `o` by `pieces` (bottom to top; the first keeps its id) as
/// one undo step named `what`, and joins them.
fn replace_with_sections(cx: &mut EditorContext, o: &StairObj, pieces: Vec<Stair>, what: &str) -> String {
    cx.begin_change(what);
    let fl = cx.floor;
    let n = pieces.len();
    let mut ids = Vec::new();
    for (i, st) in pieces.into_iter().enumerate() {
        let landing = matches!(st.params.shape, StairShape::Landing { .. });
        let mut x = o.x.clone();
        x.stairwell_walls.clear();
        x.stairwell_hole = None;
        x.stairwell_guard = false;
        x.guard_walls.clear();
        if landing {
            x.break_line = false;
            x.show_risers = false;
        } else if i + 1 < n {
            // Only the top flight reaches the floor above.
            x.break_line = false;
        }
        if i == 0 {
            let id = o.id();
            update(&mut cx.project, fl, id, |t| {
                t.stair = st;
                t.x = x;
            });
            ids.push(id);
        } else {
            ids.push(add(&mut cx.project, fl, StairObj { stair: st, x }));
        }
    }
    for id in &ids {
        connect(&mut cx.project, fl, *id);
    }
    cx.selection.set(ObjectRef::Stair(o.id()));
    cx.mark_dirty();
    format!("{what}: {n} sections")
}

/// Why a stair cannot be broken up while it owns a stairwell.
const WELL_IN_THE_WAY: &str =
    "Remove the stairwell first (delete its hole and walls on the floor above): it follows the whole stair";

/// Complete Break: the stair divided in the middle into a lower flight, a
/// landing the stair's width deep and an upper flight.
pub fn complete_break(cx: &mut EditorContext, id: Id) -> Result<String, String> {
    let o = find(cx.floor(), id).ok_or("Select a stair first")?;
    if o.x.stairwell_hole.is_some() {
        return Err(WELL_IN_THE_WAY.into());
    }
    let risers = o.solution().risers;
    let k = (risers / 2).clamp(2, risers.saturating_sub(2).max(2));
    let depth = o.stair.params.width.max(plan_stairs::MIN_BREAK_LANDING);
    let pieces = plan_stairs::complete_break(&o.stair, k, depth)
        .ok_or("Complete Break needs a straight stair of at least four risers")?;
    Ok(replace_with_sections(cx, &o, pieces, "Complete Break"))
}

/// Disconnect Selected Subsection: an L-shaped or U-shaped stair as separate
/// flights and landing(s).
pub fn disconnect(cx: &mut EditorContext, id: Id) -> Result<String, String> {
    let o = find(cx.floor(), id).ok_or("Select a stair first")?;
    if o.x.stairwell_hole.is_some() {
        return Err(WELL_IN_THE_WAY.into());
    }
    let pieces = plan_stairs::disconnect(&o.stair)
        .ok_or("Disconnect Subsection needs an L-shaped or U-shaped stair")?;
    Ok(replace_with_sections(cx, &o, pieces, "Disconnect Subsection"))
}
