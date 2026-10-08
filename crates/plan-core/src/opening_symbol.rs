//! Plan symbols of doors and windows (DW-38..DW-49, DW-83).
//!
//! [`plan_symbol`] turns an opening and its host wall into a list of
//! [`SymbolPart`]s in world inches: jamb lines, the leaf, the swing arc,
//! glazing lines, dashed hidden lines (pocket, garage path, awning arms), the
//! projecting outline of a bay, bow or box window. It is a pure function so
//! the plan view, the PDF sheets and the tests all read the same geometry; the
//! caller only picks a pen for each [`PartKind`].
//!
//! Positions are worked out in wall-local `(s, t)`: `s` along the wall from
//! its start, `t` across it along the wall's left normal. A leaf swings toward
//! `+t` unless [`Opening::swing_flipped`], and the hinge sits on the start
//! jamb unless [`Opening::hinge_at_end`].

use crate::geometry::Point;
use crate::model::{Opening, OpeningKind, Wall};
use crate::openings::spec::CurvedCasing;
use crate::openings::OpeningStyle;

/// How far bay, box and bow windows project from the exterior wall face.
pub const PROJECTION: f64 = 18.0;
/// Thickness of one panel of a projecting window.
pub const PROJECTED_PANEL: f64 = 2.0;
/// Number of straight segments approximating a bow.
pub const BOW_SEGMENTS: usize = 5;
/// Panels overlap by this much on a sliding door or window.
const SLIDING_OVERLAP: f64 = 2.0;
/// Segments of a drawn swing arc.
const ARC_SEGMENTS: usize = 16;
/// Width from which a casement window has two sashes.
const DOUBLE_CASEMENT_FROM: f64 = 48.0;

/// What a part of a symbol is, so the caller can choose its pen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
    /// A line across the wall at a jamb.
    Jamb,
    /// A frame line: a wall face across a window, a projecting unit's outline.
    Frame,
    /// A glazing line or pane.
    Glass,
    /// A door leaf, sash or sliding panel.
    Leaf,
    /// A swing arc.
    Swing,
    /// A dashed line for what is hidden or only suggested (pocket, garage
    /// path, awning arms, the pass-through head).
    Hidden,
    /// A barn-door track.
    Track,
    /// A sliding direction arrow.
    Arrow,
    /// The thin threshold line across an exterior door (DW-81).
    Threshold,
    /// The exterior sill of a window projecting past the wall face (DW-85).
    Sill,
    /// An Opening Indicator mark: the "X" of a fixed unit, the arrowhead of
    /// a swing arc, the way an awning opens (DW-83).
    Indicator,
}

/// One polyline (or closed polygon) of a symbol, in world inches.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolPart {
    pub kind: PartKind,
    pub points: Vec<Point>,
    pub closed: bool,
}

/// The plan symbol of one opening.
#[derive(Debug, Clone, PartialEq)]
pub struct OpeningSymbol {
    /// Wall-local `t` range cleared of wall fill across the opening:
    /// `(-half, half)` for a full-depth opening, a shallow band for a niche.
    pub cut: (f64, f64),
    /// Wall-local `s` range cleared of wall fill: the opening's jambs, wider
    /// than the unit when its size does not include the frame (DW-82).
    pub span: (f64, f64),
    pub parts: Vec<SymbolPart>,
}

impl OpeningSymbol {
    /// The parts of one kind.
    pub fn of(&self, kind: PartKind) -> impl Iterator<Item = &SymbolPart> {
        self.parts.iter().filter(move |p| p.kind == kind)
    }

    pub fn count(&self, kind: PartKind) -> usize {
        self.of(kind).count()
    }

    /// Farthest distance of any point from the wall centerline, inches.
    pub fn max_reach(&self, wall: &Wall) -> f64 {
        self.parts
            .iter()
            .flat_map(|p| p.points.iter())
            .map(|q| wall.locate(*q).1.abs())
            .fold(0.0, f64::max)
    }
}

/// Number of panels of a sliding door: one per 4' of width, 2 to 4.
pub fn sliding_panels(width: f64) -> usize {
    ((width / 48.0).ceil() as usize).clamp(2, 4)
}

/// Number of panels of a bifold door: a pair up to 4', two pairs beyond.
pub fn bifold_panels(width: f64) -> usize {
    if width <= 48.0 {
        2
    } else {
        4
    }
}

/// Number of leaves of a door style: two for a double door, one otherwise.
pub fn door_leaves(style: OpeningStyle) -> usize {
    if style == OpeningStyle::DoubleDoor {
        2
    } else {
        1
    }
}

/// Wall-local frame: `p(s, t)` is the world point. On a curved wall `s` is
/// the arc length and `t` the radial offset, so a line along the wall
/// follows the arc (DW-88).
struct Axes<'a> {
    start: Point,
    d: Point,
    n: Point,
    curve: Option<&'a Wall>,
}

impl<'a> Axes<'a> {
    fn new(wall: &'a Wall) -> Self {
        Self {
            start: wall.start,
            d: wall.direction(),
            n: wall.normal(),
            curve: wall.is_curved().then_some(wall),
        }
    }

    fn p(&self, s: f64, t: f64) -> Point {
        match self.curve {
            Some(w) => w.point_offset(s, t),
            None => self.start.add(self.d.scale(s)).add(self.n.scale(t)),
        }
    }

    fn pts(&self, pts: &[(f64, f64)]) -> Vec<Point> {
        pts.iter().map(|&(s, t)| self.p(s, t)).collect()
    }
}

fn part(kind: PartKind, points: Vec<Point>) -> SymbolPart {
    SymbolPart {
        kind,
        points,
        closed: false,
    }
}

fn closed(kind: PartKind, points: Vec<Point>) -> SymbolPart {
    SymbolPart {
        kind,
        points,
        closed: true,
    }
}

/// A rectangle over `s0..s1` and `t0..t1`.
fn rect(ax: &Axes, kind: PartKind, s: (f64, f64), t: (f64, f64)) -> SymbolPart {
    closed(
        kind,
        ax.pts(&[(s.0, t.0), (s.1, t.0), (s.1, t.1), (s.0, t.1)]),
    )
}

/// A thin leaf lying across the opening, `th` thick, on the centerline.
fn flat_leaf(ax: &Axes, s: (f64, f64), th: f64) -> SymbolPart {
    rect(ax, PartKind::Leaf, s, (-th * 0.5, th * 0.5))
}

/// A flat leaf over `s` and `th` thick, standing at `lt` across the wall
/// (the part `flat_leaf` made, replaced when the leaf is recessed).
fn shift_t(flat: SymbolPart, ax: &Axes, s: (f64, f64), th: f64, lt: f64) -> SymbolPart {
    if lt == 0.0 {
        flat
    } else {
        rect(ax, PartKind::Leaf, s, (lt - th * 0.5, lt + th * 0.5))
    }
}

/// A swing arc about `(hs, 0)`: from along the wall (toward `dsign`) up to
/// `angle` toward `side`.
fn arc(ax: &Axes, hs: f64, dsign: f64, side: f64, radius: f64, angle: f64) -> SymbolPart {
    arc_t(ax, hs, 0.0, dsign, side, radius, angle)
}

/// [`arc`] about `(hs, t0)`: a leaf recessed into the wall (DW-57).
fn arc_t(
    ax: &Axes,
    hs: f64,
    t0: f64,
    dsign: f64,
    side: f64,
    radius: f64,
    angle: f64,
) -> SymbolPart {
    let pts = (0..=ARC_SEGMENTS)
        .map(|i| {
            let a = angle * i as f64 / ARC_SEGMENTS as f64;
            ax.p(hs + dsign * radius * a.cos(), t0 + side * radius * a.sin())
        })
        .collect();
    part(PartKind::Swing, pts)
}

/// The tip of a leaf of length `len` hinged at `(hs, 0)`, opened `angle`.
fn leaf_tip(ax: &Axes, hs: f64, dsign: f64, side: f64, len: f64, angle: f64) -> Point {
    leaf_tip_t(ax, hs, 0.0, dsign, side, len, angle)
}

/// [`leaf_tip`] for a leaf hinged at `(hs, t0)`.
fn leaf_tip_t(ax: &Axes, hs: f64, t0: f64, dsign: f64, side: f64, len: f64, angle: f64) -> Point {
    ax.p(
        hs + dsign * len * angle.cos(),
        t0 + side * len * angle.sin(),
    )
}

/// An arrow along the wall from `s_from` to `s_to` at `t`.
fn arrow(ax: &Axes, s_from: f64, s_to: f64, t: f64, head: f64) -> SymbolPart {
    let dir = if s_to >= s_from { 1.0 } else { -1.0 };
    part(
        PartKind::Arrow,
        vec![
            ax.p(s_from, t),
            ax.p(s_to, t),
            ax.p(s_to - dir * head, t + head * 0.5),
            ax.p(s_to, t),
            ax.p(s_to - dir * head, t - head * 0.5),
        ],
    )
}

/// Outline of a projecting (bay, box or bow) window in wall `(s, t)`:
/// starts and ends on the exterior face at `t0`, `sign` points away from the
/// wall. `s0..s1` are the jambs.
pub fn projection_footprint(
    style: OpeningStyle,
    s0: f64,
    s1: f64,
    t0: f64,
    sign: f64,
) -> Vec<(f64, f64)> {
    let w = s1 - s0;
    let t1 = t0 + sign * PROJECTION;
    match style {
        OpeningStyle::BayWindow => {
            let ds = PROJECTION.min((w - 6.0).max(0.0) * 0.5);
            vec![(s0, t0), (s0 + ds, t1), (s1 - ds, t1), (s1, t0)]
        }
        OpeningStyle::BoxWindow => vec![(s0, t0), (s0, t1), (s1, t1), (s1, t0)],
        _ => {
            // Circular arc through both jambs with sagitta PROJECTION.
            let c = w * 0.5;
            let r = (c * c + PROJECTION * PROJECTION) / (2.0 * PROJECTION);
            let (sc, tc) = ((s0 + s1) * 0.5, t0 - sign * (r - PROJECTION));
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

/// `poly` moved `d` toward the side its centroid is on (the inner face of the
/// panels of a projecting window). End points slide along their own segment's
/// normal; inner corners are the intersection of the two offset lines.
pub fn inset_polyline(poly: &[(f64, f64)], d: f64) -> Vec<(f64, f64)> {
    let n = poly.len();
    if n < 2 {
        return poly.to_vec();
    }
    let centroid = (
        poly.iter().map(|p| p.0).sum::<f64>() / n as f64,
        poly.iter().map(|p| p.1).sum::<f64>() / n as f64,
    );
    // Offset line of each segment: (point on it, unit direction).
    let lines: Vec<((f64, f64), (f64, f64))> = poly
        .windows(2)
        .map(|w| {
            let (p, q) = (w[0], w[1]);
            let len = (q.0 - p.0).hypot(q.1 - p.1).max(1e-12);
            let dir = ((q.0 - p.0) / len, (q.1 - p.1) / len);
            let mut nrm = (-dir.1, dir.0);
            let mid = ((p.0 + q.0) * 0.5, (p.1 + q.1) * 0.5);
            if nrm.0 * (centroid.0 - mid.0) + nrm.1 * (centroid.1 - mid.1) < 0.0 {
                nrm = (-nrm.0, -nrm.1);
            }
            ((p.0 + nrm.0 * d, p.1 + nrm.1 * d), dir)
        })
        .collect();
    let mut out = Vec::with_capacity(n);
    out.push(lines[0].0);
    for i in 1..n - 1 {
        let (p0, d0) = lines[i - 1];
        let (p1, d1) = lines[i];
        let det = d0.0 * d1.1 - d0.1 * d1.0;
        if det.abs() < 1e-9 {
            out.push(p1);
        } else {
            let k = ((p1.0 - p0.0) * d1.1 - (p1.1 - p0.1) * d1.0) / det;
            out.push((p0.0 + d0.0 * k, p0.1 + d0.1 * k));
        }
    }
    let (p, dir) = lines[n - 2];
    let seg = (poly[n - 1].0 - poly[n - 2].0, poly[n - 1].1 - poly[n - 2].1);
    let len = seg.0.hypot(seg.1);
    out.push((p.0 + dir.0 * len, p.1 + dir.1 * len));
    out
}

/// The plan symbol of `o` in its host `wall`. `exterior` is the wall-local
/// side of the outside (`1.0` left, `-1.0` right; see
/// [`crate::openings::exterior_sign`]): bay, bow and box windows project
/// there (reversed by [`Opening::swing_flipped`]), awnings open outward and
/// hoppers inward, and a niche is cut from the other, room side.
///
/// On a curved wall the lines along the wall (jambs, frame and glazing
/// lines, sliding panels, the projecting outline) follow the arc, and a
/// hinged leaf with its swing arc stands in the frame of the chord between
/// the jambs, the way the 3D view builds it (DW-88).
pub fn plan_symbol(wall: &Wall, o: &Opening, exterior: f64) -> OpeningSymbol {
    plan_symbol_in(wall, o, exterior, false)
}

/// [`plan_symbol`] for an opening that may stand over another one on its
/// wall (`over`, a transom over a door): drawn dashed, as what is above the
/// plan's cut plane.
pub fn plan_symbol_in(wall: &Wall, o: &Opening, exterior: f64, over: bool) -> OpeningSymbol {
    let mut sym = if wall.is_curved() {
        curved_symbol(wall, o, exterior)
    } else {
        straight_symbol(wall, o, exterior)
    };
    if over {
        for p in &mut sym.parts {
            p.kind = PartKind::Hidden;
        }
    }
    sym
}

/// The style a symbol is drawn for: one that does not suit the kind falls
/// back to the plain one.
fn symbol_style(o: &Opening) -> OpeningStyle {
    match (o.kind, o.effective_style()) {
        (OpeningKind::Door, s) if !s.is_door_style() && s != OpeningStyle::Fixed => {
            OpeningStyle::Hinged
        }
        (OpeningKind::Window, OpeningStyle::Sliding) => OpeningStyle::SlidingWindow,
        (OpeningKind::Window, s) if s.is_door_style() && s != OpeningStyle::Doorway => {
            OpeningStyle::Window
        }
        (_, s) => s,
    }
}

/// Segments of a drawn curve are at most this many degrees of arc.
const CURVE_STEP_DEG: f64 = 3.0;

/// The symbol of an opening on a curved wall: worked out against the
/// straight wall of the chord between its jambs, then the parts along the
/// wall are laid on the arc.
fn curved_symbol(wall: &Wall, o: &Opening, exterior: f64) -> OpeningSymbol {
    let (s0, s1) = (o.start_offset(), o.end_offset());
    let (a, b) = (wall.point_along(s0), wall.point_along(s1));
    let chord = a.dist(b);
    let Some((_, radius)) = wall.arc_center_radius().filter(|_| chord > 1e-6) else {
        let mut flat = wall.clone();
        flat.curve = None;
        return straight_symbol(&flat, o, exterior);
    };
    let u = b.sub(a).scale(1.0 / chord);
    let nv = u.perp();
    let mut vw = wall.clone();
    vw.curve = None;
    vw.start = a.sub(u.scale(s0));
    vw.end = vw.start.add(u.scale(wall.path_length()));
    let mut vo = o.clone();
    vo.width = chord;
    vo.center_offset = s0 + chord * 0.5;
    let mut sym = straight_symbol(&vw, &vo, exterior);
    // The cleared span is in chord terms: its end follows the arc length.
    sym.span.1 += o.width - chord;
    let style = symbol_style(o);
    let rigid_leaf = matches!(
        style,
        OpeningStyle::Hinged
            | OpeningStyle::Shower
            | OpeningStyle::DoubleDoor
            | OpeningStyle::Casement
    );
    let scale = o.width / chord;
    let step = (radius * CURVE_STEP_DEG.to_radians()).max(0.5);
    // Virtual (s, t) of a point, and its place on the arc.
    let local = |p: Point| {
        let rel = p.sub(vw.start);
        (s0 + (rel.dot(u) - s0) * scale, rel.dot(nv))
    };
    for part in &mut sym.parts {
        if matches!(part.kind, PartKind::Swing | PartKind::Indicator)
            || (part.kind == PartKind::Leaf && rigid_leaf)
        {
            continue;
        }
        let pts: Vec<(f64, f64)> = part.points.iter().map(|&p| local(p)).collect();
        let n = pts.len();
        let edges = if part.closed { n } else { n.saturating_sub(1) };
        let mut out: Vec<Point> = Vec::with_capacity(n);
        for i in 0..n {
            let (sa, ta) = pts[i];
            out.push(wall.point_offset(sa, ta));
            if i >= edges {
                continue;
            }
            let (sb, tb) = pts[(i + 1) % n];
            // Only a line running along the wall bends with it.
            if (tb - ta).abs() < 1e-6 {
                let k = ((sb - sa).abs() / step).ceil() as usize;
                for j in 1..k.clamp(1, 64) {
                    out.push(wall.point_offset(sa + (sb - sa) * j as f64 / k as f64, ta));
                }
            }
        }
        part.points = out;
    }
    sym
}

/// How far the wall opening runs past each side of the unit: the frame or
/// jamb width when the size does not include it (DW-82), else nothing.
fn frame_reach(o: &Opening, style: OpeningStyle) -> f64 {
    if o.extras.spec.size_includes_frame || !has_frame_blocks(o, style) {
        return 0.0;
    }
    o.frame_width().min(o.width * 0.25)
}

/// How far the cleared opening in the wall runs past each side of the unit
/// `o` (the Jamb and Frame tabs' "Size Excludes Jamb / Frame"): the plan cuts
/// the wall that much wider, and 3D cuts its hole the same.
pub fn cleared_reach(o: &Opening) -> f64 {
    frame_reach(o, symbol_style(o))
}

/// Whether the plan draws jamb or frame blocks beside the jambs of `o`.
fn has_frame_blocks(o: &Opening, style: OpeningStyle) -> bool {
    match o.kind {
        OpeningKind::Window => {
            o.extras.frame_width.is_some_and(|f| f > 0.0)
                && matches!(
                    style,
                    OpeningStyle::Window
                        | OpeningStyle::Fixed
                        | OpeningStyle::Casement
                        | OpeningStyle::SlidingWindow
                        | OpeningStyle::Awning
                        | OpeningStyle::Hopper
                )
        }
        OpeningKind::Door => {
            o.extras.spec.jamb_in_plan
                && o.frame_width() > 0.0
                && matches!(
                    style,
                    OpeningStyle::Hinged
                        | OpeningStyle::DoubleDoor
                        | OpeningStyle::Doorway
                        | OpeningStyle::Sliding
                        | OpeningStyle::Fixed
                        | OpeningStyle::Shower
                        | OpeningStyle::Bifold
                )
        }
    }
}

fn straight_symbol(wall: &Wall, o: &Opening, exterior: f64) -> OpeningSymbol {
    // A unit whose size leaves out its frame sits in a wider opening: draw
    // the opening at that width, the frame blocks filling the difference.
    let reach = frame_reach(o, symbol_style(o));
    if reach > 0.0 {
        let mut wide = o.clone();
        wide.width += 2.0 * reach;
        wide.extras.spec.size_includes_frame = true;
        return straight_symbol(wall, &wide, exterior);
    }
    let ax = Axes::new(wall);
    let half = wall.thickness * 0.5;
    let (s0, s1) = (o.start_offset(), o.end_offset());
    let w = o.width;
    let show_open = o.extras.show_open_in_plan;
    let angle = o
        .extras
        .swing_angle_deg
        .unwrap_or(90.0)
        .clamp(1.0, 180.0)
        .to_radians();
    let side = if o.swing_flipped { -1.0 } else { 1.0 };
    let (hs, dsign) = if o.hinge_at_end {
        (s1, -1.0)
    } else {
        (s0, 1.0)
    };
    let leaf_th = o
        .extras
        .thickness
        .unwrap_or(1.375)
        .clamp(0.25, half.max(0.5) * 2.0);
    // Where the leaf stands across the wall: on the centerline, or recessed
    // to a depth from the exterior face (DW-57).
    let lt = o
        .extras
        .spec
        .recess_depth
        .map_or(0.0, |d| exterior * (half - d.clamp(0.0, wall.thickness)));
    let both = o.extras.spec.swings_both;
    let mut sym = OpeningSymbol {
        cut: (-half, half),
        span: (s0, s1),
        parts: Vec::new(),
    };
    let jambs = |sym: &mut OpeningSymbol| {
        for s in [s0, s1] {
            sym.parts
                .push(part(PartKind::Jamb, vec![ax.p(s, half), ax.p(s, -half)]));
        }
    };
    // The two wall-face lines and a glazing line between the jambs.
    let window_base = |sym: &mut OpeningSymbol, glass: bool| {
        sym.parts
            .push(part(PartKind::Frame, vec![ax.p(s0, half), ax.p(s1, half)]));
        if glass {
            sym.parts
                .push(part(PartKind::Glass, vec![ax.p(s0, 0.0), ax.p(s1, 0.0)]));
        }
        sym.parts.push(part(
            PartKind::Frame,
            vec![ax.p(s0, -half), ax.p(s1, -half)],
        ));
    };

    let style = symbol_style(o);

    match style {
        OpeningStyle::Hinged | OpeningStyle::Shower => {
            jambs(&mut sym);
            let glass = style == OpeningStyle::Shower;
            if show_open {
                let base = ax.p(hs, lt);
                let tip = leaf_tip_t(&ax, hs, lt, dsign, side, w, angle);
                if glass {
                    // A glass leaf: a long thin rectangle about the leaf line.
                    let perp = tip.sub(base).normalized().perp().scale(0.19);
                    sym.parts.push(closed(
                        PartKind::Leaf,
                        vec![base.add(perp), tip.add(perp), tip.sub(perp), base.sub(perp)],
                    ));
                } else {
                    sym.parts.push(part(PartKind::Leaf, vec![base, tip]));
                }
                sym.parts.push(arc_t(&ax, hs, lt, dsign, side, w, angle));
                if both {
                    sym.parts.push(arc_t(&ax, hs, lt, dsign, -side, w, angle));
                }
            } else {
                sym.parts.push(shift_t(
                    flat_leaf(&ax, (s0, s1), if glass { 0.375 } else { leaf_th }),
                    &ax,
                    (s0, s1),
                    if glass { 0.375 } else { leaf_th },
                    lt,
                ));
            }
        }
        OpeningStyle::DoubleDoor => {
            jambs(&mut sym);
            let half_w = w * 0.5;
            let mid = s0 + half_w;
            let spec = &o.extras.spec;
            // Each leaf: the hinge (at the jamb, or both in the middle for
            // "Swings from Center"), the way it runs from the hinge, and the
            // stretch of the opening it fills when shut.
            let leaves = if spec.swings_from_center {
                [(mid, -1.0, (s0, mid)), (mid, 1.0, (mid, s1))]
            } else {
                [(s0, 1.0, (s0, mid)), (s1, -1.0, (mid, s1))]
            };
            for (k, (h, dir, span)) in leaves.into_iter().enumerate() {
                if show_open && spec.door_swing.swings(k == 1) {
                    sym.parts.push(part(
                        PartKind::Leaf,
                        vec![
                            ax.p(h, lt),
                            leaf_tip_t(&ax, h, lt, dir, side, half_w, angle),
                        ],
                    ));
                    sym.parts.push(arc_t(&ax, h, lt, dir, side, half_w, angle));
                    if both {
                        sym.parts.push(arc_t(&ax, h, lt, dir, -side, half_w, angle));
                    }
                } else {
                    sym.parts.push(shift_t(
                        flat_leaf(&ax, span, leaf_th),
                        &ax,
                        span,
                        leaf_th,
                        lt,
                    ));
                }
            }
        }
        OpeningStyle::Doorway => jambs(&mut sym),
        OpeningStyle::Sliding => {
            jambs(&mut sym);
            let n = sliding_panels(w);
            let off = (half * 0.45).clamp(0.6, 1.5);
            let th = (half * 0.3).clamp(0.4, 1.0);
            let step = w / n as f64;
            for k in 0..n {
                let a = s0 + k as f64 * step - if k > 0 { SLIDING_OVERLAP * 0.5 } else { 0.0 };
                let b = s0
                    + (k + 1) as f64 * step
                    + if k + 1 < n {
                        SLIDING_OVERLAP * 0.5
                    } else {
                        0.0
                    };
                // The panel nearest the fixed end is fixed; every second one
                // after it is movable and runs on the other track.
                let kk = if o.hinge_at_end { n - 1 - k } else { k };
                let t = if kk % 2 == 0 { -off } else { off };
                sym.parts.push(rect(
                    &ax,
                    PartKind::Leaf,
                    (a, b),
                    (t - th * 0.5, t + th * 0.5),
                ));
            }
            let (from, to) = if o.hinge_at_end {
                (s0 + w * 0.2, s1 - w * 0.2)
            } else {
                (s1 - w * 0.2, s0 + w * 0.2)
            };
            sym.parts
                .push(arrow(&ax, from, to, 0.0, (w * 0.06).clamp(1.5, 5.0)));
        }
        OpeningStyle::Pocket => {
            jambs(&mut sym);
            let toward_end = o.hinge_at_end;
            // The pocket is the span past the pocket-side jamb, inside the wall.
            let (p0, p1) = if toward_end {
                (s1, s1 + w)
            } else {
                (s0 - w, s0)
            };
            sym.parts.push(SymbolPart {
                kind: PartKind::Hidden,
                points: ax.pts(&[
                    (p0, half * 0.8),
                    (p1, half * 0.8),
                    (p1, -half * 0.8),
                    (p0, -half * 0.8),
                ]),
                closed: true,
            });
            if show_open {
                // The leaf slid into the pocket, a little left out at the jamb.
                let (l0, l1) = if toward_end {
                    (s1 - 2.0, s1 + w - 2.0)
                } else {
                    (s0 - w + 2.0, s0 + 2.0)
                };
                sym.parts.push(flat_leaf(&ax, (l0, l1), leaf_th));
            } else {
                sym.parts.push(flat_leaf(&ax, (s0, s1), leaf_th));
            }
        }
        OpeningStyle::Bifold => {
            jambs(&mut sym);
            let n = bifold_panels(w);
            if show_open {
                let depth = (w / n as f64 * 0.5).max(1.0);
                if n == 2 {
                    let (j, dir) = (hs, dsign);
                    sym.parts.push(part(
                        PartKind::Leaf,
                        vec![
                            ax.p(j, 0.0),
                            ax.p(j + dir * w * 0.5, side * depth),
                            ax.p(j + dir * w, 0.0),
                        ],
                    ));
                } else {
                    for (j, dir) in [(s0, 1.0), (s1, -1.0)] {
                        sym.parts.push(part(
                            PartKind::Leaf,
                            vec![
                                ax.p(j, 0.0),
                                ax.p(j + dir * w * 0.25, side * depth),
                                ax.p(j + dir * w * 0.5, 0.0),
                            ],
                        ));
                    }
                }
            } else {
                let step = w / n as f64;
                for k in 0..n {
                    sym.parts.push(flat_leaf(
                        &ax,
                        (s0 + k as f64 * step, s0 + (k + 1) as f64 * step),
                        1.0,
                    ));
                }
            }
        }
        OpeningStyle::Garage => {
            jambs(&mut sym);
            sym.parts.push(flat_leaf(&ax, (s0, s1), 2.0));
            // The overhead path: rails running back into the garage.
            let reach = (w * 0.5).min(96.0);
            sym.parts.push(part(
                PartKind::Hidden,
                vec![
                    ax.p(s0, side * half),
                    ax.p(s0, side * (half + reach)),
                    ax.p(s1, side * (half + reach)),
                    ax.p(s1, side * half),
                ],
            ));
        }
        OpeningStyle::Barn => {
            jambs(&mut sym);
            let t0 = side * (half + 0.5);
            let t1 = side * (half + 0.5 + leaf_th);
            let dir = if o.hinge_at_end { 1.0 } else { -1.0 };
            let closed_s = (s0 - 1.0, s1 + 1.0);
            let slide = if show_open { dir * w } else { 0.0 };
            sym.parts.push(rect(
                &ax,
                PartKind::Leaf,
                (closed_s.0 + slide, closed_s.1 + slide),
                (t0.min(t1), t0.max(t1)),
            ));
            if show_open {
                sym.parts.push(SymbolPart {
                    kind: PartKind::Hidden,
                    points: ax.pts(&[
                        (closed_s.0, t0.min(t1)),
                        (closed_s.1, t0.min(t1)),
                        (closed_s.1, t0.max(t1)),
                        (closed_s.0, t0.max(t1)),
                    ]),
                    closed: true,
                });
            }
            let tm = (t0 + t1) * 0.5;
            sym.parts.push(part(
                PartKind::Track,
                vec![
                    ax.p((s0 - w).max(0.0), tm),
                    ax.p((s1 + w).min(wall.path_length()), tm),
                ],
            ));
        }
        OpeningStyle::Fixed => {
            jambs(&mut sym);
            window_base(&mut sym, false);
            sym.parts
                .push(rect(&ax, PartKind::Glass, (s0, s1), (-0.25, 0.25)));
        }
        OpeningStyle::Window => {
            jambs(&mut sym);
            window_base(&mut sym, true);
        }
        OpeningStyle::Casement => {
            jambs(&mut sym);
            window_base(&mut sym, true);
            if show_open {
                if w >= DOUBLE_CASEMENT_FROM {
                    let hw = w * 0.5;
                    for (h, dir) in [(s0, 1.0), (s1, -1.0)] {
                        sym.parts.push(part(
                            PartKind::Leaf,
                            vec![ax.p(h, 0.0), leaf_tip(&ax, h, dir, side, hw, angle)],
                        ));
                        sym.parts.push(arc(&ax, h, dir, side, hw, angle));
                    }
                } else {
                    sym.parts.push(part(
                        PartKind::Leaf,
                        vec![ax.p(hs, 0.0), leaf_tip(&ax, hs, dsign, side, w, angle)],
                    ));
                    sym.parts.push(arc(&ax, hs, dsign, side, w, angle));
                }
            }
        }
        OpeningStyle::SlidingWindow => {
            jambs(&mut sym);
            window_base(&mut sym, false);
            let off = (half * 0.25).clamp(0.3, 1.0);
            let mid = (s0 + s1) * 0.5;
            let ov = SLIDING_OVERLAP * 0.5;
            sym.parts.push(rect(
                &ax,
                PartKind::Leaf,
                (s0, mid + ov),
                (off - 0.25, off + 0.25),
            ));
            sym.parts.push(rect(
                &ax,
                PartKind::Leaf,
                (mid - ov, s1),
                (-off - 0.25, -off + 0.25),
            ));
            let (from, to) = if o.hinge_at_end {
                (s0 + w * 0.15, s1 - w * 0.15)
            } else {
                (s1 - w * 0.15, s0 + w * 0.15)
            };
            sym.parts
                .push(arrow(&ax, from, to, 0.0, (w * 0.05).clamp(1.0, 4.0)));
        }
        OpeningStyle::Awning | OpeningStyle::Hopper => {
            jambs(&mut sym);
            window_base(&mut sym, true);
            // An awning opens outward, a hopper inward; the dashed arms show
            // the projecting sash.
            let toward = if style == OpeningStyle::Awning {
                exterior
            } else {
                -exterior
            };
            let toward = if o.swing_flipped { -toward } else { toward };
            let arm = 6.0;
            sym.parts.push(part(
                PartKind::Hidden,
                vec![
                    ax.p(s0 + 1.0, toward * half),
                    ax.p(s0 + 1.0, toward * (half + arm)),
                    ax.p(s1 - 1.0, toward * (half + arm)),
                    ax.p(s1 - 1.0, toward * half),
                ],
            ));
        }
        OpeningStyle::BayWindow | OpeningStyle::BowWindow | OpeningStyle::BoxWindow => {
            let sign = if o.swing_flipped { -exterior } else { exterior };
            let outer = projection_footprint(style, s0, s1, sign * half, sign);
            let inner = inset_polyline(&outer, PROJECTED_PANEL);
            sym.parts.push(part(PartKind::Frame, ax.pts(&outer)));
            sym.parts.push(part(PartKind::Frame, ax.pts(&inner)));
            // The sill line across the opening on the room side.
            sym.parts.push(part(
                PartKind::Glass,
                vec![ax.p(s0, -sign * half), ax.p(s1, -sign * half)],
            ));
            jambs(&mut sym);
        }
        OpeningStyle::PassThrough => {
            jambs(&mut sym);
            window_base(&mut sym, false);
            sym.parts
                .push(part(PartKind::Hidden, vec![ax.p(s0, 0.0), ax.p(s1, 0.0)]));
        }
        OpeningStyle::WallNiche => {
            // A recess from the room side, not through the wall.
            let ns = -exterior;
            let depth = o.niche_depth(wall.thickness).min(wall.thickness);
            let back = ns * (half - depth);
            let face = ns * half;
            sym.cut = (back.min(face), back.max(face));
            for s in [s0, s1] {
                sym.parts
                    .push(part(PartKind::Jamb, vec![ax.p(s, face), ax.p(s, back)]));
            }
            sym.parts
                .push(part(PartKind::Frame, vec![ax.p(s0, back), ax.p(s1, back)]));
        }
    }
    add_frame_blocks(&mut sym, &ax, wall, o, style);
    add_threshold_and_sill(&mut sym, &ax, wall, o, style, exterior, lt);
    add_arch_marks(&mut sym, &ax, wall, o, style);
    add_rough_opening(&mut sym, &ax, wall, o);
    add_shutters(&mut sym, &ax, wall, o, exterior);
    add_indicators(&mut sym, &ax, wall, o, style, exterior);
    sym
}

/// The jamb blocks of a window frame, as wide as the Frame tab says (DW-82):
/// drawn once the window has a frame width of its own. A door's jamb blocks
/// (DW-82) are drawn beside each jamb line unless its Jamb tab turns them off.
fn add_frame_blocks(
    sym: &mut OpeningSymbol,
    ax: &Axes,
    wall: &Wall,
    o: &Opening,
    style: OpeningStyle,
) {
    if !has_frame_blocks(o, style) {
        return;
    }
    let half = wall.thickness * 0.5;
    let fw = o.frame_width().min(o.width * 0.25);
    let (s0, s1) = (o.start_offset(), o.end_offset());
    for s in [(s0, s0 + fw), (s1 - fw, s1)] {
        sym.parts.push(rect(ax, PartKind::Frame, s, (-half, half)));
    }
}

/// The thin threshold line across an exterior door (DW-81) and the exterior
/// sill of a window (DW-85) that the Lintel tab turned on.
fn add_threshold_and_sill(
    sym: &mut OpeningSymbol,
    ax: &Axes,
    wall: &Wall,
    o: &Opening,
    style: OpeningStyle,
    exterior: f64,
    lt: f64,
) {
    let half = wall.thickness * 0.5;
    let (s0, s1) = (o.start_offset(), o.end_offset());
    let exterior_wall = wall.kind == crate::model::WallKind::Exterior;
    match o.kind {
        OpeningKind::Door => {
            let thresholded = matches!(
                style,
                OpeningStyle::Hinged
                    | OpeningStyle::DoubleDoor
                    | OpeningStyle::Sliding
                    | OpeningStyle::Fixed
            );
            if exterior_wall && thresholded && o.extras.spec.threshold {
                let leaf_th = o.extras.thickness.unwrap_or(1.375);
                let t = (lt + exterior * (leaf_th * 0.5 + 0.4)).clamp(-half, half);
                sym.parts
                    .push(part(PartKind::Threshold, vec![ax.p(s0, t), ax.p(s1, t)]));
            }
        }
        OpeningKind::Window => {
            let sill = &o.extras.spec.sill;
            let projects = !matches!(
                style,
                OpeningStyle::BayWindow
                    | OpeningStyle::BowWindow
                    | OpeningStyle::BoxWindow
                    | OpeningStyle::WallNiche
                    | OpeningStyle::PassThrough
            );
            if exterior_wall && sill.enabled && projects && sill.depth > 0.0 {
                let a = (s0 - sill.extend).max(0.0);
                let b = (s1 + sill.extend).min(wall.path_length());
                let (t0, t1) = (exterior * half, exterior * (half + sill.depth));
                sym.parts
                    .push(rect(ax, PartKind::Sill, (a, b), (t0.min(t1), t0.max(t1))));
            }
        }
    }
}

/// Opening Indicators (DW-83): the "X" of a fixed unit, the arrow of an
/// awning or hopper, and an arrowhead at the free end of every swing arc.
fn add_indicators(
    sym: &mut OpeningSymbol,
    ax: &Axes,
    wall: &Wall,
    o: &Opening,
    style: OpeningStyle,
    exterior: f64,
) {
    let ind = o.extras.spec.indicators;
    if !ind.show_in_plan && !ind.swing_arrows {
        return;
    }
    let half = wall.thickness * 0.5;
    let (s0, s1) = (o.start_offset(), o.end_offset());
    if ind.show_in_plan {
        match style {
            OpeningStyle::Fixed => {
                for (a, b) in [((s0, half), (s1, -half)), ((s0, -half), (s1, half))] {
                    sym.parts.push(part(
                        PartKind::Indicator,
                        vec![ax.p(a.0, a.1), ax.p(b.0, b.1)],
                    ));
                }
            }
            OpeningStyle::Awning | OpeningStyle::Hopper => {
                let toward = if style == OpeningStyle::Awning {
                    exterior
                } else {
                    -exterior
                };
                let toward = if o.swing_flipped { -toward } else { toward };
                let mid = (s0 + s1) * 0.5;
                let head = (o.width * 0.08).clamp(1.0, 3.0);
                let reach = half + 4.0;
                sym.parts.push(part(
                    PartKind::Indicator,
                    vec![
                        ax.p(mid, 0.0),
                        ax.p(mid, toward * reach),
                        ax.p(mid - head, toward * (reach - head)),
                        ax.p(mid, toward * reach),
                        ax.p(mid + head, toward * (reach - head)),
                    ],
                ));
            }
            _ => {}
        }
    }
    if ind.swing_arrows {
        let heads: Vec<SymbolPart> = sym
            .of(PartKind::Swing)
            .filter(|a| a.points.len() >= 2)
            .map(|a| {
                let n = a.points.len();
                let (tip, prev) = (a.points[n - 1], a.points[n - 2]);
                let dir = tip.sub(prev).normalized();
                let len = (o.width * 0.08).clamp(1.5, 4.0);
                let back = tip.sub(dir.scale(len));
                let wing = dir.perp().scale(len * 0.45);
                part(
                    PartKind::Indicator,
                    vec![back.add(wing), tip, back.sub(wing)],
                )
            })
            .collect();
        sym.parts.extend(heads);
    }
}

/// Dashed head lines over the opening when its head is arched: the arch
/// is above the cut plane, so it is only suggested in plan.
fn add_arch_marks(
    sym: &mut OpeningSymbol,
    ax: &Axes,
    wall: &Wall,
    o: &Opening,
    style: OpeningStyle,
) {
    if ((!o.is_arched() || o.extras.spec.arch.rise(o.width, o.height) <= 0.0) && !o.is_shaped())
        || matches!(
            style,
            OpeningStyle::WallNiche
                | OpeningStyle::BayWindow
                | OpeningStyle::BowWindow
                | OpeningStyle::BoxWindow
        )
    {
        return;
    }
    let q = wall.thickness * 0.25;
    let (s0, s1) = (o.start_offset(), o.end_offset());
    for t in [q, -q] {
        sym.parts
            .push(part(PartKind::Hidden, vec![ax.p(s0, t), ax.p(s1, t)]));
    }
}

/// The rough opening as two dashed lines across the wall, past each jamb by
/// the Rough Opening tab's extra (DW-56). Nothing unless the tab asks for it
/// and the rough opening is wider than the unit.
fn add_rough_opening(sym: &mut OpeningSymbol, ax: &Axes, wall: &Wall, o: &Opening) {
    let rough = &o.extras.spec.rough;
    if !rough.show_in_plan || matches!(o.style, OpeningStyle::WallNiche | OpeningStyle::PassThrough)
    {
        return;
    }
    let b = rough.extents(o.kind);
    let half = wall.thickness * 0.5;
    let (lo, hi) = (
        (o.start_offset() - b.left).max(0.0),
        (o.end_offset() + b.right).min(wall.path_length()),
    );
    if b.left + b.right <= 1e-9 {
        return;
    }
    for s in [lo, hi] {
        sym.parts
            .push(part(PartKind::Hidden, vec![ax.p(s, half), ax.p(s, -half)]));
    }
}

/// Exterior shutters as small rectangles outside the wall (DW-84).
fn add_shutters(sym: &mut OpeningSymbol, ax: &Axes, wall: &Wall, o: &Opening, exterior: f64) {
    let sh = &o.extras.spec.shutters;
    if !sh.present() || wall.kind != crate::model::WallKind::Exterior {
        return;
    }
    let half = wall.thickness * 0.5;
    let t = (
        exterior * half,
        exterior * (half + crate::openings::spec::SHUTTER_THICKNESS),
    );
    let t = (t.0.min(t.1), t.0.max(t.1));
    for (a, b) in sh.spans(o.start_offset(), o.end_offset(), o.casing_reach()) {
        sym.parts.push(rect(ax, PartKind::Frame, (a, b), t));
    }
}

/// One casing board over `s` along the wall and `t` across it. On a curved
/// wall `mode` lays it radial (square to the wall where it stands: the plain
/// rectangle of the wall's frame), straight (square to the wall at the middle
/// `mid` of the opening) or parallel (bent to follow the curve).
fn casing_rect(
    wall: &Wall,
    ax: &Axes,
    mode: CurvedCasing,
    s: (f64, f64),
    t: (f64, f64),
    mid: f64,
) -> SymbolPart {
    if !wall.is_curved() || mode == CurvedCasing::Radial {
        return rect(ax, PartKind::Frame, s, t);
    }
    match mode {
        CurvedCasing::Parallel => {
            let n = ((s.1 - s.0).abs().ceil() as usize).clamp(2, 24);
            let mut pts: Vec<(f64, f64)> = (0..=n)
                .map(|i| (s.0 + (s.1 - s.0) * i as f64 / n as f64, t.0))
                .collect();
            pts.extend(
                (0..=n)
                    .rev()
                    .map(|i| (s.0 + (s.1 - s.0) * i as f64 / n as f64, t.1)),
            );
            closed(PartKind::Frame, ax.pts(&pts))
        }
        _ => {
            // The board's frame is the wall's tangent at the middle of the
            // opening: straight, the same on both sides.
            let (at, tangent) = wall.frame_at(mid);
            let nrm = tangent.perp();
            let p = |s_: f64, t_: f64| at.add(tangent.scale(s_ - mid)).add(nrm.scale(t_));
            closed(
                PartKind::Frame,
                vec![p(s.0, t.0), p(s.1, t.0), p(s.1, t.1), p(s.0, t.1)],
            )
        }
    }
}

/// Casing as small rectangles on both wall faces beside the jambs of the
/// unit `o` belongs to (DW-52, DW-79): one rectangle at each end of a mulled
/// unit, none between its members. `unit` is the span of the whole unit
/// (`None` for an opening on its own); `exterior` is the wall-local side of
/// the outside. Empty unless the opening asks for casing in plan.
pub fn casing_parts(
    wall: &Wall,
    o: &Opening,
    unit: Option<(f64, f64)>,
    exterior: f64,
) -> Vec<SymbolPart> {
    let spec = &o.extras.spec;
    if !spec.casing_in_plan || o.style == OpeningStyle::WallNiche {
        return Vec::new();
    }
    let ax = Axes::new(wall);
    let half = wall.thickness * 0.5;
    let c = o.casing.unwrap_or_default();
    let (lo, hi) = unit.unwrap_or((o.start_offset(), o.end_offset()));
    let mut out = Vec::new();
    for side in [1.0, -1.0] {
        let is_exterior = side == exterior;
        if (is_exterior && !spec.casing_exterior) || (!is_exterior && !spec.casing_interior) {
            continue;
        }
        let t = (side * half, side * (half + c.depth));
        let t = (t.0.min(t.1), t.0.max(t.1));
        if o.start_offset() <= lo + 1e-9 {
            let end = lo - c.reveal;
            out.push(casing_rect(
                wall,
                &ax,
                spec.curved_casing,
                ((end - c.width).max(0.0), end),
                t,
                (lo + hi) * 0.5,
            ));
        }
        if o.end_offset() >= hi - 1e-9 {
            let start = hi + c.reveal;
            out.push(casing_rect(
                wall,
                &ax,
                spec.curved_casing,
                (start, (start + c.width).min(wall.path_length())),
                t,
                (lo + hi) * 0.5,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{OpeningKind, WallKind};

    fn wall() -> Wall {
        Wall::new(
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        )
    }

    fn open(kind: OpeningKind, style: OpeningStyle, width: f64) -> Opening {
        let mut o = Opening::new(1, 120.0, kind, width, 80.0, 0.0);
        o.style = style;
        o
    }

    fn sym(o: &Opening) -> OpeningSymbol {
        // Left normal of a +x wall is +y; the outside is the left side.
        plan_symbol(&wall(), o, 1.0)
    }

    #[test]
    fn hinged_has_two_jambs_a_leaf_and_a_quarter_arc() {
        let s = sym(&open(OpeningKind::Door, OpeningStyle::Hinged, 36.0));
        assert_eq!(s.count(PartKind::Jamb), 2);
        assert_eq!(s.count(PartKind::Leaf), 1);
        let arc = s.of(PartKind::Swing).next().unwrap();
        assert_eq!(arc.points.len(), ARC_SEGMENTS + 1);
        // Hinge at the start jamb (s = 102), leaf perpendicular, 36" long.
        let leaf = s.of(PartKind::Leaf).next().unwrap();
        assert!((leaf.points[0].x - 102.0).abs() < 1e-9 && leaf.points[0].y.abs() < 1e-9);
        assert!((leaf.points[1].x - 102.0).abs() < 1e-6 && (leaf.points[1].y - 36.0).abs() < 1e-6);
        // Flipped hinge and swing mirror it.
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        d.swing_flipped = true;
        d.hinge_at_end = true;
        let s = sym(&d);
        let leaf = s.of(PartKind::Leaf).next().unwrap();
        assert!((leaf.points[0].x - 138.0).abs() < 1e-9);
        assert!((leaf.points[1].y + 36.0).abs() < 1e-6);
    }

    #[test]
    fn closed_hinged_door_is_a_flat_leaf_without_an_arc() {
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        d.extras.show_open_in_plan = false;
        let s = sym(&d);
        assert_eq!(s.count(PartKind::Swing), 0);
        let leaf = s.of(PartKind::Leaf).next().unwrap();
        assert!(leaf.closed && leaf.points.len() == 4);
        assert!(s.max_reach(&wall()) <= 3.0 + 1e-9);
    }

    #[test]
    fn swing_angle_sets_the_open_leaf() {
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        d.extras.swing_angle_deg = Some(45.0);
        let s = sym(&d);
        let leaf = s.of(PartKind::Leaf).next().unwrap();
        let tip = leaf.points[1];
        assert!((tip.x - (102.0 + 36.0 * 45f64.to_radians().cos())).abs() < 1e-6);
        assert!((tip.y - 36.0 * 45f64.to_radians().sin()).abs() < 1e-6);
    }

    #[test]
    fn double_door_has_two_leaves_and_two_arcs() {
        let s = sym(&open(OpeningKind::Door, OpeningStyle::DoubleDoor, 60.0));
        assert_eq!(s.count(PartKind::Leaf), 2);
        assert_eq!(s.count(PartKind::Swing), 2);
        // Each leaf is half the width.
        for l in s.of(PartKind::Leaf) {
            assert!((l.points[0].dist(l.points[1]) - 30.0).abs() < 1e-6);
        }
        assert_eq!(door_leaves(OpeningStyle::DoubleDoor), 2);
        assert_eq!(door_leaves(OpeningStyle::Hinged), 1);
    }

    #[test]
    fn doorway_is_jambs_only() {
        let s = sym(&open(OpeningKind::Door, OpeningStyle::Doorway, 36.0));
        // Two jamb lines and the two jamb blocks beside them (DW-82).
        assert_eq!(s.parts.len(), 4);
        assert_eq!(s.count(PartKind::Jamb), 2);
        assert_eq!(s.count(PartKind::Frame), 2);
    }

    #[test]
    fn sliding_door_panels_follow_the_width() {
        for (w, n) in [
            (36.0, 2),
            (72.0, 2),
            (96.0, 2),
            (120.0, 3),
            (192.0, 4),
            (300.0, 4),
        ] {
            assert_eq!(sliding_panels(w), n, "{w}");
        }
        let s = sym(&open(OpeningKind::Door, OpeningStyle::Sliding, 72.0));
        assert_eq!(s.count(PartKind::Leaf), 2);
        assert_eq!(s.count(PartKind::Arrow), 1);
        assert_eq!(s.count(PartKind::Swing), 0);
        // Panels sit on alternating sides of the centerline.
        let ts: Vec<f64> = s
            .of(PartKind::Leaf)
            .map(|p| p.points.iter().map(|q| q.y).sum::<f64>() / 4.0)
            .collect();
        assert!(ts[0] < 0.0 && ts[1] > 0.0);
        let s = sym(&open(OpeningKind::Door, OpeningStyle::Sliding, 144.0));
        assert_eq!(s.count(PartKind::Leaf), 3);
    }

    #[test]
    fn pocket_door_draws_a_dashed_pocket_in_the_wall() {
        let mut d = open(OpeningKind::Door, OpeningStyle::Pocket, 30.0);
        let s = sym(&d);
        let pocket = s.of(PartKind::Hidden).next().unwrap();
        assert!(pocket.closed);
        // Past the start jamb (s = 105), inside the wall thickness.
        assert!(pocket.points.iter().all(|p| p.x <= 105.0 + 1e-9));
        assert!(pocket.points.iter().all(|p| p.y.abs() < 3.0));
        d.hinge_at_end = true;
        let s = sym(&d);
        let pocket = s.of(PartKind::Hidden).next().unwrap();
        assert!(pocket.points.iter().all(|p| p.x >= 135.0 - 1e-9));
        assert_eq!(s.count(PartKind::Leaf), 1);
    }

    #[test]
    fn bifold_draws_a_v_per_pair() {
        let s = sym(&open(OpeningKind::Door, OpeningStyle::Bifold, 36.0));
        let legs: Vec<_> = s.of(PartKind::Leaf).collect();
        assert_eq!(legs.len(), 1);
        assert_eq!(legs[0].points.len(), 3);
        assert_eq!(bifold_panels(36.0), 2);
        let s = sym(&open(OpeningKind::Door, OpeningStyle::Bifold, 72.0));
        assert_eq!(s.count(PartKind::Leaf), 2);
        assert_eq!(bifold_panels(72.0), 4);
    }

    #[test]
    fn garage_has_a_panel_and_a_dashed_overhead_path() {
        let s = sym(&open(OpeningKind::Door, OpeningStyle::Garage, 192.0));
        assert_eq!(s.count(PartKind::Leaf), 1);
        let path = s.of(PartKind::Hidden).next().unwrap();
        assert_eq!(path.points.len(), 4);
        assert!((path.points[1].y - (3.0 + 96.0)).abs() < 1e-6);
    }

    #[test]
    fn barn_door_hangs_outside_the_wall_on_a_track() {
        let mut d = open(OpeningKind::Door, OpeningStyle::Barn, 36.0);
        d.extras.show_open_in_plan = false;
        let s = sym(&d);
        let panel = s.of(PartKind::Leaf).next().unwrap();
        assert!(panel.points.iter().all(|p| p.y >= 3.0 + 0.5 - 1e-9));
        assert_eq!(s.count(PartKind::Track), 1);
        // Open: the panel has slid one width along the wall (toward the start).
        d.extras.show_open_in_plan = true;
        let s = sym(&d);
        let panel = s.of(PartKind::Leaf).next().unwrap();
        assert!(panel.points.iter().all(|p| p.x <= 120.0 - 17.0 + 1e-9));
        assert_eq!(s.count(PartKind::Hidden), 1);
    }

    #[test]
    fn shower_door_is_a_glass_leaf_with_an_arc() {
        let s = sym(&open(OpeningKind::Door, OpeningStyle::Shower, 28.0));
        assert_eq!(s.count(PartKind::Swing), 1);
        let leaf = s.of(PartKind::Leaf).next().unwrap();
        assert!(leaf.closed && leaf.points.len() == 4);
    }

    #[test]
    fn fixed_door_has_glass_and_no_swing() {
        let s = sym(&open(OpeningKind::Door, OpeningStyle::Fixed, 36.0));
        assert_eq!(s.count(PartKind::Swing), 0);
        assert_eq!(s.count(PartKind::Glass), 1);
        // The two wall-face lines of the glazing, plus the two jamb blocks.
        assert_eq!(s.count(PartKind::Frame), 4);
    }

    #[test]
    fn plain_window_is_three_lines_between_the_jambs() {
        let s = sym(&open(OpeningKind::Window, OpeningStyle::Window, 36.0));
        assert_eq!(s.count(PartKind::Frame), 2);
        assert_eq!(s.count(PartKind::Glass), 1);
        assert_eq!(s.count(PartKind::Swing), 0);
    }

    #[test]
    fn casement_draws_swing_arcs() {
        let one = sym(&open(OpeningKind::Window, OpeningStyle::Casement, 30.0));
        assert_eq!(one.count(PartKind::Swing), 1);
        assert_eq!(one.count(PartKind::Leaf), 1);
        let two = sym(&open(OpeningKind::Window, OpeningStyle::Casement, 60.0));
        assert_eq!(two.count(PartKind::Swing), 2);
        assert_eq!(two.count(PartKind::Leaf), 2);
    }

    #[test]
    fn sliding_window_has_two_offset_panels_and_an_arrow() {
        let s = sym(&open(
            OpeningKind::Window,
            OpeningStyle::SlidingWindow,
            60.0,
        ));
        assert_eq!(s.count(PartKind::Leaf), 2);
        assert_eq!(s.count(PartKind::Arrow), 1);
    }

    #[test]
    fn awning_and_hopper_dash_their_arms_on_opposite_sides() {
        let a = sym(&open(OpeningKind::Window, OpeningStyle::Awning, 36.0));
        let h = sym(&open(OpeningKind::Window, OpeningStyle::Hopper, 36.0));
        let ay = a.of(PartKind::Hidden).next().unwrap().points[1].y;
        let hy = h.of(PartKind::Hidden).next().unwrap().points[1].y;
        assert!(ay > 0.0 && hy < 0.0, "{ay} {hy}");
    }

    #[test]
    fn projecting_windows_reach_eighteen_inches_past_the_face() {
        for (style, n) in [
            (OpeningStyle::BayWindow, 4),
            (OpeningStyle::BoxWindow, 4),
            (OpeningStyle::BowWindow, BOW_SEGMENTS + 1),
        ] {
            let s = sym(&open(OpeningKind::Window, style, 72.0));
            let outer = s.of(PartKind::Frame).next().unwrap();
            assert_eq!(outer.points.len(), n, "{style:?}");
            let reach = s.max_reach(&wall());
            assert!(
                (reach - (3.0 + PROJECTION)).abs() < 1e-6,
                "{style:?} {reach}"
            );
            assert_eq!(s.count(PartKind::Frame), 2);
            // Mirrored to the other side by Reverse Swing.
            let mut w = open(OpeningKind::Window, style, 72.0);
            w.swing_flipped = true;
            let f = sym(&w);
            let below = f
                .of(PartKind::Frame)
                .flat_map(|p| p.points.iter())
                .map(|p| p.y)
                .fold(f64::MAX, f64::min);
            assert!((below + 3.0 + PROJECTION).abs() < 1e-6);
        }
    }

    #[test]
    fn inset_keeps_the_end_points_on_their_offset_lines() {
        let outer = projection_footprint(OpeningStyle::BoxWindow, 0.0, 60.0, 3.0, 1.0);
        let inner = inset_polyline(&outer, 2.0);
        assert_eq!(inner.len(), 4);
        // (0,3)->(0,21) moves to s = 2; the far side to t = 19.
        assert!((inner[0].0 - 2.0).abs() < 1e-9);
        assert!((inner[1].0 - 2.0).abs() < 1e-9 && (inner[1].1 - 19.0).abs() < 1e-9);
        assert!((inner[2].0 - 58.0).abs() < 1e-9 && (inner[2].1 - 19.0).abs() < 1e-9);
    }

    #[test]
    fn pass_through_is_dashed_and_niche_only_cuts_partway() {
        let p = sym(&open(OpeningKind::Window, OpeningStyle::PassThrough, 36.0));
        assert_eq!(p.count(PartKind::Hidden), 1);
        assert_eq!(p.cut, (-3.0, 3.0));
        let n = sym(&open(OpeningKind::Window, OpeningStyle::WallNiche, 24.0));
        // Exterior is the left (+t) side, so the niche is cut from the right
        // (room) face, 3.5" deep: it stops short of the far face.
        assert!((n.cut.0 + 3.0).abs() < 1e-9);
        assert!((n.cut.1 - 0.5).abs() < 1e-9);
        assert!(n.cut.1 < 3.0);
        assert_eq!(n.count(PartKind::Jamb), 2);
        assert_eq!(n.count(PartKind::Frame), 1);
    }

    #[test]
    fn every_style_produces_a_symbol_with_jambs_or_a_projection() {
        use OpeningStyle as S;
        let all = [
            S::Hinged,
            S::Sliding,
            S::Pocket,
            S::Bifold,
            S::Garage,
            S::Doorway,
            S::Barn,
            S::Shower,
            S::Fixed,
            S::Window,
            S::BayWindow,
            S::BowWindow,
            S::BoxWindow,
            S::PassThrough,
            S::WallNiche,
            S::DoubleDoor,
            S::Casement,
            S::SlidingWindow,
            S::Awning,
            S::Hopper,
        ];
        for style in all {
            let kind = if style.is_door_style() {
                OpeningKind::Door
            } else {
                OpeningKind::Window
            };
            let s = sym(&open(kind, style, 48.0));
            assert!(!s.parts.is_empty(), "{style:?}");
            assert!(s.count(PartKind::Jamb) >= 2, "{style:?}");
            for p in &s.parts {
                assert!(p.points.iter().all(|q| q.x.is_finite() && q.y.is_finite()));
            }
        }
    }

    #[test]
    fn an_arched_head_adds_dashed_head_lines_and_a_square_one_does_not() {
        use crate::openings::{Arch, ArchType};
        let mut w = open(OpeningKind::Window, OpeningStyle::Window, 36.0);
        let plain = sym(&w).count(PartKind::Hidden);
        w.extras.spec.arch = Arch {
            kind: ArchType::RoundTop,
            height: 0.0,
        };
        let s = sym(&w);
        assert_eq!(s.count(PartKind::Hidden), plain + 2);
        // The lines run jamb to jamb on either side of the centerline.
        let dashed: Vec<_> = s.of(PartKind::Hidden).collect();
        assert!(dashed
            .iter()
            .all(|d| (d.points[0].x - 102.0).abs() < 1e-9 && (d.points[1].x - 138.0).abs() < 1e-9));
        assert!(dashed[0].points[0].y * dashed[1].points[0].y < 0.0);
        // A door arches too; a niche never does.
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        d.extras.spec.arch.kind = ArchType::Gothic;
        assert_eq!(sym(&d).count(PartKind::Hidden), 2);
        let mut n = open(OpeningKind::Window, OpeningStyle::WallNiche, 24.0);
        n.extras.spec.arch.kind = ArchType::RoundTop;
        assert_eq!(sym(&n).count(PartKind::Hidden), 0);
    }

    #[test]
    fn the_frame_tab_draws_jamb_blocks_of_its_width() {
        let mut w = open(OpeningKind::Window, OpeningStyle::Window, 36.0);
        let before = sym(&w).count(PartKind::Frame);
        w.extras.frame_width = Some(2.5);
        let s = sym(&w);
        assert_eq!(s.count(PartKind::Frame), before + 2);
        let block = s.of(PartKind::Frame).find(|p| p.closed).unwrap();
        let xs: Vec<f64> = block.points.iter().map(|q| q.x).collect();
        let (lo, hi) = (
            xs.iter().copied().fold(f64::MAX, f64::min),
            xs.iter().copied().fold(f64::MIN, f64::max),
        );
        assert!((hi - lo - 2.5).abs() < 1e-9, "{lo} {hi}");
        // A door draws its jamb blocks as wide as its Jamb tab says (DW-82)
        // and none once the tab turns them off.
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        d.extras.jamb_width = Some(2.5);
        let blocks = sym(&d);
        assert_eq!(blocks.count(PartKind::Frame), 2);
        let b = blocks.of(PartKind::Frame).next().unwrap();
        let xs: Vec<f64> = b.points.iter().map(|q| q.x).collect();
        let width = xs.iter().copied().fold(f64::MIN, f64::max)
            - xs.iter().copied().fold(f64::MAX, f64::min);
        assert!((width - 2.5).abs() < 1e-9, "{width}");
        d.extras.spec.jamb_in_plan = false;
        assert_eq!(sym(&d).count(PartKind::Frame), 0);
    }

    #[test]
    fn shutters_are_rectangles_outside_the_wall() {
        use crate::openings::{ShutterSides, ShutterStyle};
        let mut w = open(OpeningKind::Window, OpeningStyle::Window, 36.0);
        let before = sym(&w).parts.len();
        w.extras.spec.shutters.style = ShutterStyle::Panel;
        let s = sym(&w);
        assert_eq!(s.parts.len(), before + 2);
        // The wall's outside is +y here: both rectangles lie past the face.
        let rects: Vec<_> = s.of(PartKind::Frame).filter(|p| p.closed).collect();
        assert_eq!(rects.len(), 2);
        for r in rects {
            let ys: Vec<f64> = r.points.iter().map(|q| q.y).collect();
            assert!(
                ys.iter().all(|y| *y >= 3.0 - 1e-9 && *y <= 4.0 + 1e-9),
                "{ys:?}"
            );
        }
        w.extras.spec.shutters.sides = ShutterSides::Right;
        assert_eq!(sym(&w).parts.len(), before + 1);
        // Not on an interior wall.
        let mut iw = wall();
        iw.kind = WallKind::Interior;
        w.extras.spec.shutters.sides = ShutterSides::Both;
        assert_eq!(plan_symbol(&iw, &w, -1.0).parts.len(), before);
    }

    #[test]
    fn a_mulled_unit_has_casing_at_its_two_ends_only() {
        let mut door = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        door.center_offset = 100.0;
        let mut side = open(OpeningKind::Window, OpeningStyle::Fixed, 18.0);
        side.center_offset = 100.0 + 18.0 + 9.0;
        for o in [&mut door, &mut side] {
            o.extras.spec.casing_in_plan = true;
        }
        let unit = Some((door.start_offset(), side.end_offset()));
        let w = wall();
        let a = casing_parts(&w, &door, unit, 1.0);
        let b = casing_parts(&w, &side, unit, 1.0);
        // The door contributes its start side on each face, the sidelite its
        // end side: four rectangles around the unit, none between.
        assert_eq!((a.len(), b.len()), (2, 2));
        let xs: Vec<f64> = a
            .iter()
            .chain(&b)
            .flat_map(|p| p.points.iter().map(|q| q.x))
            .collect();
        assert!(
            xs.iter().all(|x| *x <= 82.0 + 1e-9 || *x >= 127.0 - 1e-9),
            "{xs:?}"
        );
        // On their own, each would have a rectangle at each jamb of each face.
        assert_eq!(casing_parts(&w, &door, None, 1.0).len(), 4);
        // Off unless the opening asks for it.
        door.extras.spec.casing_in_plan = false;
        assert!(casing_parts(&w, &door, unit, 1.0).is_empty());
        // The Casing tab switches faces: no exterior casing leaves one face.
        door.extras.spec.casing_in_plan = true;
        door.extras.spec.casing_exterior = false;
        assert_eq!(casing_parts(&w, &door, None, 1.0).len(), 2);
    }

    #[test]
    fn the_niche_depth_is_editable() {
        let mut n = open(OpeningKind::Window, OpeningStyle::WallNiche, 24.0);
        // A 6" wall, outside on the left: the niche is cut from the right face.
        let default = sym(&n);
        assert!((default.cut.1 - default.cut.0 - 3.5).abs() < 1e-9);
        n.extras.spec.niche_depth = 2.0;
        let s = sym(&n);
        assert!((s.cut.1 - s.cut.0 - 2.0).abs() < 1e-9);
        // Never through the wall: 1" is left behind.
        n.extras.spec.niche_depth = 40.0;
        let s = sym(&n);
        assert!((s.cut.1 - s.cut.0 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn calculated_door_panels_draw_a_double_door_when_wide() {
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 60.0);
        assert_eq!(sym(&d).count(PartKind::Leaf), 1);
        d.extras.spec.calc_panels = true;
        assert_eq!(sym(&d).count(PartKind::Leaf), 2);
        d.width = 30.0;
        assert_eq!(sym(&d).count(PartKind::Leaf), 1);
    }

    /// A 240" chord bowed by 60": radius 150, a 36" opening at arc length 90.
    fn arc_wall() -> Wall {
        let mut w = Wall::new(
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        w.curve = Some(crate::walls::WallCurve { bulge: 60.0 });
        w
    }

    /// Distance of `p` from the arc of `w` offset `t` to its left.
    fn off_arc(w: &Wall, p: Point, t: f64) -> f64 {
        let (c, r) = w.arc_center_radius().unwrap();
        let sweep = w.curve.unwrap().sweep(w.start, w.end);
        // Left of travel is toward the center when turning counter-clockwise.
        let radius = r - sweep.signum() * t;
        (p.sub(c).length() - radius).abs()
    }

    #[test]
    fn a_window_on_a_curved_wall_lies_on_the_offset_arcs() {
        let w = arc_wall();
        let half = 3.0;
        let mut o = open(OpeningKind::Window, OpeningStyle::Window, 60.0);
        o.center_offset = 130.0;
        let s = plan_symbol(&w, &o, 1.0);
        // The frame lines, the glazing line and the jambs all stay on the
        // arcs at the faces and the centerline.
        let mut points = 0;
        for part in s
            .parts
            .iter()
            .filter(|p| matches!(p.kind, PartKind::Frame | PartKind::Glass | PartKind::Jamb))
        {
            for q in &part.points {
                points += 1;
                let d = [half, 0.0, -half]
                    .iter()
                    .map(|t| off_arc(&w, *q, *t))
                    .fold(f64::MAX, f64::min);
                assert!(d < 0.1, "{:?} {q:?} is {d} off the arcs", part.kind);
            }
        }
        // A straight window symbol has 2 points per line; these follow the arc.
        assert!(points > 12, "{points}");
        // Every line runs from the start jamb to the end jamb along the arc:
        // the end points sit at arc length 100 and 160.
        let ends: Vec<Point> = [100.0, 160.0].iter().map(|a| w.point_along(*a)).collect();
        let frame = s.of(PartKind::Frame).next().unwrap();
        let first = frame.points.first().unwrap();
        let last = frame.points.last().unwrap();
        let near = |p: &Point| {
            ends.iter()
                .map(|e| {
                    let (c, _) = w.arc_center_radius().unwrap();
                    // Compare angles about the arc center.
                    (p.sub(c).angle() - e.sub(c).angle()).abs()
                })
                .fold(f64::MAX, f64::min)
        };
        assert!(near(first) < 0.01 && near(last) < 0.01);
    }

    #[test]
    fn a_door_on_a_curved_wall_swings_in_the_frame_of_its_chord() {
        let w = arc_wall();
        let mut o = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        o.center_offset = 130.0;
        let s = plan_symbol(&w, &o, 1.0);
        // The jambs are radial lines across the wall on the arc.
        let jambs: Vec<&SymbolPart> = s.of(PartKind::Jamb).collect();
        assert_eq!(jambs.len(), 2);
        for j in &jambs {
            let (a, b) = (j.points[0], j.points[1]);
            assert!(off_arc(&w, a, 3.0) < 0.1 && off_arc(&w, b, -3.0) < 0.1);
        }
        // The hinge is on the centerline at the start jamb, and the leaf is as
        // long as the chord between the jambs.
        let leaf = s.of(PartKind::Leaf).next().unwrap();
        let (hinge, tip) = (leaf.points[0], leaf.points[1]);
        assert!(off_arc(&w, hinge, 0.0) < 1e-6);
        assert!((hinge.dist(w.point_along(112.0)) - 0.0).abs() < 1e-6);
        let chord = w.point_along(112.0).dist(w.point_along(148.0));
        assert!((hinge.dist(tip) - chord).abs() < 1e-6);
        // The swing arc is a true circle about the hinge.
        let arc = s.of(PartKind::Swing).next().unwrap();
        for q in &arc.points {
            assert!((q.dist(hinge) - chord).abs() < 1e-6);
        }
        // Opened a quarter turn, the tip stands square to the chord.
        let chord_dir = w.point_along(148.0).sub(hinge).normalized();
        assert!(tip.sub(hinge).normalized().dot(chord_dir).abs() < 1e-6);
    }

    #[test]
    fn a_curved_wall_symbol_stays_inside_the_wall_band() {
        let w = arc_wall();
        for style in [
            OpeningStyle::Sliding,
            OpeningStyle::Doorway,
            OpeningStyle::Fixed,
        ] {
            let mut o = open(OpeningKind::Door, style, 48.0);
            o.center_offset = 120.0;
            let s = plan_symbol(&w, &o, 1.0);
            assert!(
                s.max_reach(&w) <= 3.0 + 0.1,
                "{style:?} {}",
                s.max_reach(&w)
            );
        }
    }

    #[test]
    fn a_window_over_a_door_is_drawn_dashed() {
        let w = wall();
        let o = open(OpeningKind::Window, OpeningStyle::Fixed, 36.0);
        let solid = plan_symbol(&w, &o, 1.0);
        let over = plan_symbol_in(&w, &o, 1.0, true);
        assert!(solid.parts.iter().any(|p| p.kind == PartKind::Frame));
        assert_eq!(solid.parts.len(), over.parts.len());
        assert!(over.parts.iter().all(|p| p.kind == PartKind::Hidden));
    }
    // ----- round 14: threshold, sill, both swings, indicators, recess -----

    #[test]
    fn an_exterior_door_has_a_threshold_line_across_the_opening() {
        let d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        let s = sym(&d);
        let t = s.of(PartKind::Threshold).next().expect("threshold");
        // Across the opening, parallel to the wall, on the outside of the leaf.
        assert!((t.points[0].x - 102.0).abs() < 1e-9 && (t.points[1].x - 138.0).abs() < 1e-9);
        assert!(t.points[0].y.abs() > 0.0 && t.points[0].y.abs() < 3.0);
        assert!((t.points[0].y - t.points[1].y).abs() < 1e-9);
        // The line sits on the exterior side (the left face here).
        assert!(t.points[0].y > 0.0);
        // An interior wall, a doorway and a switched-off threshold have none.
        let mut inner = wall();
        inner.kind = WallKind::Interior;
        assert_eq!(plan_symbol(&inner, &d, -1.0).count(PartKind::Threshold), 0);
        assert_eq!(
            sym(&open(OpeningKind::Door, OpeningStyle::Doorway, 36.0)).count(PartKind::Threshold),
            0
        );
        let mut off = d.clone();
        off.extras.spec.threshold = false;
        assert_eq!(sym(&off).count(PartKind::Threshold), 0);
    }

    #[test]
    fn a_window_sill_projects_past_the_exterior_face_when_defined() {
        let mut w = open(OpeningKind::Window, OpeningStyle::Window, 36.0);
        assert_eq!(sym(&w).count(PartKind::Sill), 0);
        w.extras.spec.sill.enabled = true;
        let s = sym(&w);
        let sill = s.of(PartKind::Sill).next().expect("sill");
        let ys: Vec<f64> = sill.points.iter().map(|q| q.y).collect();
        let xs: Vec<f64> = sill.points.iter().map(|q| q.x).collect();
        // From the wall face (3") out by the sill depth (2") on the outside.
        assert!((ys.iter().copied().fold(f64::MIN, f64::max) - 5.0).abs() < 1e-9);
        assert!((ys.iter().copied().fold(f64::MAX, f64::min) - 3.0).abs() < 1e-9);
        // It runs past the jambs by the sill's extend.
        assert!((xs.iter().copied().fold(f64::MAX, f64::min) - 101.0).abs() < 1e-9);
        assert!((xs.iter().copied().fold(f64::MIN, f64::max) - 139.0).abs() < 1e-9);
        // No sill on an interior wall.
        let mut inner = wall();
        inner.kind = WallKind::Interior;
        assert_eq!(plan_symbol(&inner, &w, -1.0).count(PartKind::Sill), 0);
    }

    #[test]
    fn swings_both_directions_draws_the_arc_on_each_side() {
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        assert_eq!(sym(&d).count(PartKind::Swing), 1);
        d.extras.spec.swings_both = true;
        let s = sym(&d);
        assert_eq!(s.count(PartKind::Swing), 2);
        let sides: Vec<f64> = s
            .of(PartKind::Swing)
            .map(|a| a.points.iter().map(|q| q.y).sum::<f64>())
            .collect();
        assert!(sides[0] > 0.0 && sides[1] < 0.0, "{sides:?}");
        // A double door gets two more.
        let mut dd = open(OpeningKind::Door, OpeningStyle::DoubleDoor, 60.0);
        dd.extras.spec.swings_both = true;
        assert_eq!(sym(&dd).count(PartKind::Swing), 4);
    }

    #[test]
    fn indicators_mark_fixed_units_and_the_ends_of_swing_arcs() {
        let mut f = open(OpeningKind::Window, OpeningStyle::Fixed, 36.0);
        assert_eq!(sym(&f).count(PartKind::Indicator), 0);
        f.extras.spec.indicators.show_in_plan = true;
        // An X: two diagonals corner to corner.
        let s = sym(&f);
        assert_eq!(s.count(PartKind::Indicator), 2);
        let d = s.of(PartKind::Indicator).next().unwrap();
        assert!((d.points[0].x - 102.0).abs() < 1e-9 && (d.points[1].x - 138.0).abs() < 1e-9);
        // An arrowhead at the free end of the arc, three points ending on it.
        let mut h = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        h.extras.spec.indicators.swing_arrows = true;
        let s = sym(&h);
        let head = s.of(PartKind::Indicator).next().expect("arrowhead");
        assert_eq!(head.points.len(), 3);
        let arc = s.of(PartKind::Swing).next().unwrap();
        assert!(head.points[1].dist(*arc.points.last().unwrap()) < 1e-9);
        // The arc ends at the open leaf (36" out from the hinge) and the
        // head points along the arc there, parallel to the wall.
        assert!(
            (head.points[1].y - 36.0).abs() < 1e-6,
            "{:?}",
            head.points[1]
        );
        assert!((head.points[0].y - head.points[2].y).abs() > 1e-6);
    }

    #[test]
    fn a_recessed_door_stands_at_its_depth_from_the_exterior_face() {
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        // Recessed 1" in from the outside face (3"): the leaf is at y = 2.
        d.extras.spec.recess_depth = Some(1.0);
        let s = sym(&d);
        let leaf = s.of(PartKind::Leaf).next().unwrap();
        assert!(
            (leaf.points[0].y - 2.0).abs() < 1e-9,
            "{:?}",
            leaf.points[0]
        );
        let arc = s.of(PartKind::Swing).next().unwrap();
        assert!((arc.points[0].y - 2.0).abs() < 1e-9 || (arc.points[0].y - 38.0).abs() < 1e-6);
        // Closed, the flat leaf is centered there.
        d.extras.show_open_in_plan = false;
        let leaf = sym(&d).of(PartKind::Leaf).next().unwrap().clone();
        let ys: Vec<f64> = leaf.points.iter().map(|q| q.y).collect();
        let mid = (ys.iter().copied().fold(f64::MIN, f64::max)
            + ys.iter().copied().fold(f64::MAX, f64::min))
            * 0.5;
        assert!((mid - 2.0).abs() < 1e-9, "{mid}");
    }

    #[test]
    fn a_size_that_leaves_out_the_frame_widens_the_cleared_opening() {
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        d.extras.jamb_width = Some(1.5);
        assert_eq!(sym(&d).span, (102.0, 138.0));
        d.extras.spec.size_includes_frame = false;
        let s = sym(&d);
        assert_eq!(s.span, (100.5, 139.5));
        // The jamb lines and blocks run out to the wider opening.
        let jambs: Vec<f64> = s.of(PartKind::Jamb).map(|j| j.points[0].x).collect();
        assert!(
            jambs.contains(&100.5) && jambs.contains(&139.5),
            "{jambs:?}"
        );
        // Windows with a frame width do the same.
        let mut w = open(OpeningKind::Window, OpeningStyle::Window, 36.0);
        w.extras.frame_width = Some(2.0);
        w.extras.spec.size_includes_frame = false;
        assert_eq!(sym(&w).span, (100.0, 140.0));
    }

    #[test]
    fn the_new_marks_stand_in_the_chord_frame_on_a_curved_wall() {
        let w = arc_wall();
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        d.center_offset = 100.0;
        d.extras.spec.swings_both = true;
        d.extras.spec.indicators.swing_arrows = true;
        let s = plan_symbol(&w, &d, 1.0);
        assert_eq!(s.count(PartKind::Swing), 2);
        assert_eq!(s.count(PartKind::Indicator), 2);
        assert!(s.span.1 - s.span.0 >= 36.0 - 1e-6);
    }

    // ----- round 15: swing choices, rough opening, curved casing, shapes -----

    #[test]
    fn a_double_door_can_swing_one_leaf_or_from_the_center() {
        use crate::openings::spec::DoorSwing;
        let mut d = open(OpeningKind::Door, OpeningStyle::DoubleDoor, 60.0);
        assert_eq!(sym(&d).count(PartKind::Swing), 2);
        // Left only: one arc, and the other leaf shut across its half.
        d.extras.spec.door_swing = DoorSwing::LeftOnly;
        let s = sym(&d);
        assert_eq!(s.count(PartKind::Swing), 1);
        assert_eq!(s.count(PartKind::Leaf), 2);
        let arc = s.of(PartKind::Swing).next().unwrap();
        assert!(arc.points.iter().all(|p| p.x <= 120.0 + 1e-6), "start leaf");
        d.extras.spec.door_swing = DoorSwing::RightOnly;
        let s = sym(&d);
        let arc = s.of(PartKind::Swing).next().unwrap();
        assert!(arc.points.iter().all(|p| p.x >= 120.0 - 1e-6), "end leaf");
        // From the center: both hinges stand in the middle of the opening.
        d.extras.spec.door_swing = DoorSwing::Both;
        d.extras.spec.swings_from_center = true;
        let s = sym(&d);
        assert_eq!(s.count(PartKind::Swing), 2);
        for leaf in s.of(PartKind::Leaf) {
            assert!((leaf.points[0].x - 120.0).abs() < 1e-9, "{:?}", leaf.points);
        }
        // Shut, the leaves still fill their halves.
        d.extras.show_open_in_plan = false;
        assert_eq!(sym(&d).count(PartKind::Leaf), 2);
        assert_eq!(sym(&d).count(PartKind::Swing), 0);
    }

    #[test]
    fn the_rough_opening_draws_dashed_lines_past_the_jambs_only_when_asked() {
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        let before = sym(&d).parts.len();
        d.extras.spec.rough.add_width = 4.0;
        // Not shown in plan until the tab says so.
        assert_eq!(sym(&d).parts.len(), before);
        d.extras.spec.rough.show_in_plan = true;
        let s = sym(&d);
        assert_eq!(s.parts.len(), before + 2);
        let xs: Vec<f64> = s.of(PartKind::Hidden).map(|p| p.points[0].x).collect();
        assert!(xs.contains(&100.0) && xs.contains(&140.0), "{xs:?}");
        // No extra space, nothing to draw.
        d.extras.spec.rough.add_width = 0.0;
        assert_eq!(sym(&d).parts.len(), before);
    }

    #[test]
    fn cleared_reach_matches_the_cleared_span() {
        let mut d = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        d.extras.jamb_width = Some(1.5);
        assert_eq!(cleared_reach(&d), 0.0);
        d.extras.spec.size_includes_frame = false;
        assert_eq!(cleared_reach(&d), 1.5);
        let s = sym(&d).span;
        assert_eq!(s, (d.start_offset() - 1.5, d.end_offset() + 1.5));
    }

    #[test]
    fn curved_wall_casing_is_radial_straight_or_parallel() {
        use crate::openings::spec::CurvedCasing;
        let w = arc_wall();
        let mut win = open(OpeningKind::Window, OpeningStyle::Window, 36.0);
        win.center_offset = 100.0;
        win.extras.spec.casing_in_plan = true;
        let parts = |o: &Opening| casing_parts(&w, o, None, 1.0);
        let radial = parts(&win);
        assert_eq!(radial.len(), 4);
        assert!(radial.iter().all(|p| p.points.len() == 4));
        win.extras.spec.curved_casing = CurvedCasing::Parallel;
        let parallel = parts(&win);
        assert_eq!(parallel.len(), 4);
        assert!(parallel.iter().all(|p| p.points.len() > 4), "bent boards");
        win.extras.spec.curved_casing = CurvedCasing::Straight;
        let straight = parts(&win);
        assert_eq!(straight.len(), 4);
        assert!(straight.iter().all(|p| p.points.len() == 4));
        // Straight boards are square to the middle of the opening: the boards
        // at the two ends of one face share a direction.
        let dir = |p: &SymbolPart| {
            let v = p.points[1].sub(p.points[0]);
            v.scale(1.0 / v.length())
        };
        assert!(dir(&straight[0]).dot(dir(&straight[1])) > 0.999);
        assert!(dir(&radial[0]).dot(dir(&radial[1])) < 0.999);
        // On a straight wall all three are the same rectangle.
        let flat = wall();
        let mut s = open(OpeningKind::Window, OpeningStyle::Window, 36.0);
        s.extras.spec.casing_in_plan = true;
        let a = casing_parts(&flat, &s, None, 1.0);
        s.extras.spec.curved_casing = CurvedCasing::Parallel;
        assert_eq!(casing_parts(&flat, &s, None, 1.0), a);
    }

    #[test]
    fn a_shaped_window_is_marked_at_the_head_in_plan() {
        use crate::openings::spec::{ShapeKind, WindowShape};
        let mut win = open(OpeningKind::Window, OpeningStyle::Window, 36.0);
        let plain = sym(&win).count(PartKind::Hidden);
        win.extras.spec.shape = WindowShape::preset(ShapeKind::Triangle, 36.0, 80.0);
        assert!(win.is_shaped());
        assert_eq!(sym(&win).count(PartKind::Hidden), plain + 2);
        // A door never has a shape.
        let mut door = open(OpeningKind::Door, OpeningStyle::Hinged, 36.0);
        door.extras.spec.shape = WindowShape::preset(ShapeKind::Triangle, 36.0, 80.0);
        assert!(!door.is_shaped());
    }
}
