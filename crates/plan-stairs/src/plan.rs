//! The 2D plan symbol of a stair: outline, risers, direction arrow, UP label
//! and the break line Chief draws where the floor above cuts the flight.
//!
//! The Plan Display and Arrow panels ([`crate::PlanOptions`]) shape the break
//! line, the arrowhead, the tread numbers, the walkline and which parts of a
//! railing show.

use crate::layout::{arc_band, Curve, Flight, Layout, RampArc, Uv};
use crate::railing::{
    landing_guards, landing_rail_paths, plan_symbol_railing, stair_railing_geometry,
};
use crate::{ArrowStyle, BreakStyle, RailSide, RailStyle, SideKind, Stair, StairParams};
use plan_core::Point;

/// Radius of the circle at the foot of the direction arrow.
const CIRCLE_RADIUS: f64 = 2.5;
/// Distance of that circle's centre from the first riser.
const CIRCLE_ALONG: f64 = 4.0;
/// Plan text height for the UP label.
const TEXT_HEIGHT: f64 = 6.0;
/// Height of a tread number.
const NUMBER_HEIGHT: f64 = 4.0;

/// A plan-view drawing primitive (inches, plan Y-up).
#[derive(Debug, Clone, PartialEq)]
pub enum Stroke {
    /// A straight segment.
    Line(Point, Point),
    /// A polyline; the flag closes it into a polygon.
    Polyline(Vec<Point>, bool),
    /// A circular arc, counter-clockwise from `start_deg` to `end_deg`.
    Arc {
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    /// Text anchored at its baseline start, rotated by `angle` degrees.
    Text {
        pos: Point,
        text: String,
        height: f64,
        angle: f64,
    },
}

/// Draw the stair in plan.
///
/// `cut_at` is the fraction (0..1) of the total flight length, landings
/// excluded, at which the floor above cuts the stair. At that point the stair
/// is truncated and a zigzag break line is drawn; `Some(2.0 / 3.0)` matches
/// Chief's default. `None` draws the full run.
pub fn plan_symbol(stair: &Stair, cut_at: Option<f64>) -> Vec<Stroke> {
    let layout = Layout::build(stair);
    let to_plan = |p: Uv| layout.frame.uv(p);
    if layout.is_landing {
        return landing_symbol(stair, &layout);
    }
    if let Some(arc) = &layout.ramp_arc {
        return ramp_arc_symbol(stair, &layout, arc);
    }
    if let Some(c) = &layout.curve {
        return curved_symbol(stair, &layout, c, cut_at);
    }
    let p = &stair.params;
    let flights = &layout.flights;
    let cut = locate_cut(flights, cut_at);
    let last = cut.map_or(flights.len() - 1, |(i, _)| i);
    let mut out = Vec::new();
    let flared = !p.flare_shape.is_none();

    for (i, f) in flights.iter().enumerate().take(last + 1) {
        let clipped = cut.filter(|&(ci, _)| ci == i).map(|(_, s)| s);
        let len = clipped.unwrap_or(f.len);
        for j in 0..f.risers {
            let s = f64::from(j) * layout.tread_depth;
            let keep = if clipped.is_some() {
                s < len - 1e-9
            } else {
                true
            };
            if keep {
                // The line across the riser; the first riser of a flight has
                // no tread below it, so it is never curved.
                let bulge = if j == 0 { 0.0 } else { layout.bulge(p, i, j) };
                for w in layout.across(p, i, s, bulge).windows(2) {
                    out.push(Stroke::Line(to_plan(w[0]), to_plan(w[1])));
                }
            }
        }
        if flared {
            let left = layout.edge_chain(p, i, len, false);
            let right = layout.edge_chain(p, i, len, true);
            if clipped.is_some() {
                // From the cut back down the left edge, across the bottom,
                // then up the right edge.
                let mut open: Vec<Uv> = left.iter().rev().copied().collect();
                open.extend(right.iter().copied());
                out.push(Stroke::Polyline(
                    open.into_iter().map(to_plan).collect(),
                    false,
                ));
                out.push(break_line(f, len, &to_plan, p));
            } else if f.len > 1e-9 {
                let mut ring = left;
                ring.extend(right.into_iter().rev());
                out.push(Stroke::Polyline(
                    ring.into_iter().map(to_plan).collect(),
                    true,
                ));
            }
            continue;
        }
        let corners = [
            f.at(0.0, 0.0),
            f.at(len, 0.0),
            f.at(len, f.width),
            f.at(0.0, f.width),
        ];
        if clipped.is_some() {
            let open = [corners[1], corners[0], corners[3], corners[2]];
            out.push(Stroke::Polyline(open.map(to_plan).to_vec(), false));
            out.push(break_line(f, len, &to_plan, p));
        } else if f.len > 1e-9 {
            out.push(Stroke::Polyline(corners.map(to_plan).to_vec(), true));
        }
    }

    if last >= 1 {
        for outline in &layout.outlines {
            out.push(Stroke::Polyline(
                outline.iter().map(|&p| to_plan(p)).collect(),
                true,
            ));
        }
        for r in &layout.turn_risers {
            out.push(Stroke::Line(to_plan(r.a), to_plan(r.b)));
        }
    }

    for (_, apron) in layout.aprons(p) {
        out.push(Stroke::Polyline(
            apron.into_iter().map(to_plan).collect(),
            true,
        ));
    }
    if p.walkline.show {
        out.extend(straight_walkline(stair, &layout, last, cut));
    }
    out.extend(side_strokes(stair, &layout, last, cut));
    if p.handrail && layout.is_ramp {
        out.extend(ramp_handrails(stair, &layout, last, cut));
    }
    if p.plan.number_treads && !layout.is_ramp {
        out.extend(tread_numbers(stair, &layout, last, cut));
    }
    out.extend(direction_arrow(stair, &layout, last, cut.map(|(_, s)| s)));
    out
}

/// A landing: its outline and, for a rectangle, the crossed diagonals, and a
/// railing symbol along each open side that has one.
fn landing_symbol(stair: &Stair, layout: &Layout) -> Vec<Stroke> {
    let poly: Vec<Point> = layout
        .footprint
        .iter()
        .map(|&p| layout.frame.uv(p))
        .collect();
    let mut out = vec![Stroke::Polyline(poly.clone(), true)];
    if poly.len() == 4 {
        out.push(Stroke::Line(poly[0], poly[2]));
        out.push(Stroke::Line(poly[1], poly[3]));
    }
    let p = &stair.params;
    for g in landing_guards(stair) {
        let (a, b, kind) = (g.a, g.b, g.kind);
        if kind == SideKind::Railing {
            out.extend(plan_symbol_railing(a, b, &p.railing_for(g.side)));
        } else {
            // A wall or half wall: a closed band on the outside.
            let t = if kind == SideKind::Wall { 4.5 } else { 5.5 };
            let out_n = (b - a).normalized();
            let n = Point::new(out_n.y, -out_n.x) * t;
            out.push(Stroke::Polyline(vec![a, b, b + n, a + n], true));
        }
    }
    out
}

/// Half the line gap of a railing in plan (the top rail is this wide).
const RAIL_PLAN_WIDTH: f64 = 3.5;
/// A wall handrail's line sits this far inside the stair's edge in plan.
const HANDRAIL_PLAN_INSET: f64 = 1.5;

/// The walkline of a straight stair, `distance` from the right edge, along
/// each flight (or to the cut).
fn straight_walkline(
    stair: &Stair,
    layout: &Layout,
    last: usize,
    cut: Option<(usize, f64)>,
) -> Vec<Stroke> {
    let p = &stair.params;
    let mut out = Vec::new();
    for (i, f) in layout.flights.iter().enumerate().take(last + 1) {
        if f.len < 1e-9 {
            continue;
        }
        let len = cut.filter(|&(ci, _)| ci == i).map_or(f.len, |(_, s)| s);
        let lat = (f.width - p.walkline.distance.clamp(0.0, f.width)).max(0.0);
        out.push(Stroke::Polyline(
            vec![
                layout.frame.uv(f.at(0.0, lat)),
                layout.frame.uv(f.at(len, lat)),
            ],
            false,
        ));
    }
    out
}

/// The number of each tread, 1 at the bottom, on the left of the centre of
/// every tread of the flights that show.
fn tread_numbers(
    stair: &Stair,
    layout: &Layout,
    last: usize,
    cut: Option<(usize, f64)>,
) -> Vec<Stroke> {
    let mut out = Vec::new();
    let mut n = 0u32;
    let angle = stair.direction.to_degrees().rem_euclid(360.0);
    for (i, f) in layout.flights.iter().enumerate().take(last + 1) {
        let len = cut.filter(|&(ci, _)| ci == i).map_or(f.len, |(_, s)| s);
        for j in 1..=f.treads {
            n += 1;
            let s = (f64::from(j) - 0.5) * layout.tread_depth;
            if s > len + 1e-9 {
                continue;
            }
            let height = NUMBER_HEIGHT.min(layout.tread_depth * 0.45);
            let at = layout.frame.uv(f.at(s - height * 0.5, f.width * 0.12));
            out.push(Stroke::Text {
                pos: at,
                text: n.to_string(),
                height,
                angle,
            });
        }
    }
    out
}

/// A ramp's handrails: a thin line a little inside each edge.
fn ramp_handrails(
    stair: &Stair,
    layout: &Layout,
    last: usize,
    cut: Option<(usize, f64)>,
) -> Vec<Stroke> {
    let _ = stair;
    let mut out = Vec::new();
    for (i, f) in layout.flights.iter().enumerate().take(last + 1) {
        if f.len < 1e-9 {
            continue;
        }
        let len = cut.filter(|&(ci, _)| ci == i).map_or(f.len, |(_, s)| s);
        for lat in [HANDRAIL_PLAN_INSET, f.width - HANDRAIL_PLAN_INSET] {
            out.push(Stroke::Line(
                layout.frame.uv(f.at(0.0, lat)),
                layout.frame.uv(f.at(len, lat)),
            ));
        }
    }
    out
}

/// A small square about `c`.
fn square(c: Point, half: f64) -> Stroke {
    Stroke::Polyline(
        vec![
            c + Point::new(half, half),
            c + Point::new(-half, half),
            c + Point::new(-half, -half),
            c + Point::new(half, -half),
        ],
        true,
    )
}

/// Railing, wall and half-wall symbols along the straight flights: a double
/// line with newel squares for a railing, a closed band for a wall.
fn side_strokes(
    stair: &Stair,
    layout: &Layout,
    last: usize,
    cut: Option<(usize, f64)>,
) -> Vec<Stroke> {
    let p = &stair.params;
    let opts = &p.plan;
    let mut out = Vec::new();
    for (kind, right_side) in [(p.left_side, false), (p.right_side, true)] {
        if kind == SideKind::None {
            continue;
        }
        let rail_side = if right_side {
            RailSide::Right
        } else {
            RailSide::Left
        };
        let railing = p.railing_for(rail_side);
        for (i, f) in layout.flights.iter().enumerate().take(last + 1) {
            if f.len < 1e-9 {
                continue;
            }
            let clipped = cut.filter(|&(ci, _)| ci == i).map(|(_, s)| s);
            let len = clipped.unwrap_or(f.len);
            let lat = if right_side { f.width } else { 0.0 };
            let r = f.right();
            // Outward is away from the stair's middle.
            let out_sign = if right_side { 1.0 } else { -1.0 };
            let edge = |s: f64, off: f64| {
                let q = f.at(s, lat);
                layout
                    .frame
                    .uv((q.0 + r.0 * off * out_sign, q.1 + r.1 * off * out_sign))
            };
            match kind {
                SideKind::None => {}
                SideKind::Handrail => {
                    // One thin line a little inside the edge: a handrail on
                    // the wall, with no newels.
                    out.push(Stroke::Line(
                        edge(0.0, -HANDRAIL_PLAN_INSET),
                        edge(len, -HANDRAIL_PLAN_INSET),
                    ));
                }
                SideKind::Railing => {
                    let half = railing.top_rail.0.max(RAIL_PLAN_WIDTH) / 2.0;
                    if opts.draw_rails {
                        for off in [-half, half] {
                            out.push(Stroke::Line(edge(0.0, off), edge(len, off)));
                        }
                    }
                    let n = railing.newel.size / 2.0;
                    let ends: &[f64] = if clipped.is_some() {
                        &[0.0]
                    } else {
                        &[0.0, f.len]
                    };
                    if opts.draw_newels {
                        for &s in ends {
                            out.push(Stroke::Polyline(
                                vec![
                                    edge(s - n, -n),
                                    edge(s + n, -n),
                                    edge(s + n, n),
                                    edge(s - n, n),
                                ],
                                true,
                            ));
                        }
                    }
                }
                SideKind::Wall | SideKind::HalfWall => {
                    let t = if kind == SideKind::Wall { 4.5 } else { 5.5 };
                    out.push(Stroke::Polyline(
                        vec![edge(0.0, 0.0), edge(len, 0.0), edge(len, t), edge(0.0, t)],
                        true,
                    ));
                }
            }
        }
        if kind == SideKind::Railing && last > 0 {
            let half = railing.top_rail.0.max(RAIL_PLAN_WIDTH) / 2.0;
            let n = railing.newel.size / 2.0;
            // The rail that carries across each landing or turn: a double
            // line along the path and a newel square at each corner.
            for path in landing_rail_paths(stair, rail_side).iter().take(last) {
                if opts.draw_rails {
                    for w in path.windows(2) {
                        let d = (w[1] - w[0]).normalized();
                        let off = d.perp() * half;
                        out.push(Stroke::Line(w[0] + off, w[1] + off));
                        out.push(Stroke::Line(w[0] - off, w[1] - off));
                    }
                }
                if opts.draw_newels {
                    for &c in &path[1..path.len() - 1] {
                        out.push(square(c, n));
                    }
                }
            }
        }
        // Balusters: a small square at each one (switched on in the
        // Newels/Balusters panel; off by default).
        if kind == SideKind::Railing && opts.draw_balusters {
            if let RailStyle::Balusters { size, .. } = railing.style {
                let g = stair_railing_geometry(stair, rail_side, &railing);
                for (at, _, _) in g.balusters {
                    out.push(square(at, size / 2.0));
                }
            }
        }
    }
    out
}

/// Sampled arc of a curved stair at lateral `lat` from angle `a0` to `a1`.
fn curve_arc(c: &Curve, lat: f64, a0: f64, a1: f64) -> Vec<Uv> {
    let n = (((a1 - a0).abs().to_degrees() / 5.0).ceil() as usize).max(1);
    (0..=n)
        .map(|i| c.at_lat(a0 + (a1 - a0) * i as f64 / n as f64, lat))
        .collect()
}

/// The arrowhead at `b` pointing along `dir`, in the stair's arrow style;
/// `seg` is the length of the shaft it ends, which caps the head.
fn arrow_head(style: ArrowStyle, size: f64, b: Point, dir: Point, seg: f64) -> Option<Stroke> {
    let head = size.min(seg);
    let back = b - dir * head;
    let side = dir.perp() * (head * 0.45);
    match style {
        ArrowStyle::Closed => Some(Stroke::Polyline(vec![b, back + side, back - side], true)),
        ArrowStyle::Open => Some(Stroke::Polyline(vec![back + side, b, back - side], false)),
        ArrowStyle::Line | ArrowStyle::None => None,
    }
}

/// The arrow of a curved stair or ramp along the walking line from angle
/// `start` to `end`, with its circle and UP label.
fn curve_arrow(
    stair: &Stair,
    layout: &Layout,
    c: &Curve,
    circle_at: f64,
    end: f64,
) -> Vec<Stroke> {
    let to_plan = |p: Uv| layout.frame.uv(p);
    let opts = &stair.params.plan;
    let walk = c.walk();
    let da = |len: f64| len / walk.max(1e-9);
    let mut out = Vec::new();
    if opts.arrow != ArrowStyle::None {
        out.push(Stroke::Arc {
            center: to_plan(c.at(circle_at, walk)),
            radius: CIRCLE_RADIUS,
            start_deg: 0.0,
            end_deg: 360.0,
        });
        let start = (circle_at + da(CIRCLE_RADIUS)).min(end);
        if end - start > 1e-6 {
            let n = (((end - start).to_degrees() / 5.0).ceil() as usize).max(1);
            let plan_pts: Vec<Point> = (0..=n)
                .map(|i| to_plan(c.at(start + (end - start) * i as f64 / n as f64, walk)))
                .collect();
            out.push(Stroke::Polyline(plan_pts.clone(), false));
            if let [.., a, b] = plan_pts[..] {
                let seg = a.dist(b);
                if seg > 1e-9 {
                    let dir = (b - a) * (1.0 / seg);
                    out.extend(arrow_head(opts.arrow, opts.arrow_size, b, dir, seg * 4.0));
                }
            }
        }
    }
    out.push(Stroke::Text {
        pos: to_plan(c.at_lat(
            circle_at + da(CIRCLE_RADIUS + 3.0),
            (c.width / 2.0 + CIRCLE_RADIUS + 0.5 * TEXT_HEIGHT + 1.0).min(c.width),
        )),
        text: "UP".into(),
        height: TEXT_HEIGHT,
        angle: stair.direction.to_degrees().rem_euclid(360.0),
    });
    out
}

/// The plan symbol of a curved stair: radial riser lines, the two edge arcs,
/// the break line, the direction arrow along the walking line and "UP".
fn curved_symbol(stair: &Stair, layout: &Layout, c: &Curve, cut_at: Option<f64>) -> Vec<Stroke> {
    let to_plan = |p: Uv| layout.frame.uv(p);
    let poly = |pts: Vec<Uv>, closed: bool| {
        Stroke::Polyline(pts.into_iter().map(to_plan).collect(), closed)
    };
    let p = &stair.params;
    let sweep = c.sweep();
    let cut = cut_at
        .filter(|f| f.is_finite() && *f < 1.0)
        .map(|f| f.clamp(0.0, 1.0) * sweep);
    let end = cut.unwrap_or(sweep);
    let mut out = Vec::new();

    for k in 0..c.risers {
        let a = c.step * f64::from(k);
        if cut.is_some() && a >= end - 1e-9 {
            continue;
        }
        out.push(Stroke::Line(
            to_plan(c.at_lat(a, 0.0)),
            to_plan(c.at_lat(a, c.width)),
        ));
    }
    if cut.is_some() {
        let mut pts = curve_arc(c, 0.0, end, 0.0);
        pts.extend(curve_arc(c, c.width, 0.0, end));
        out.push(poly(pts, false));
        // The break line across the width at the cut angle.
        let size = p.plan.break_size;
        let eps = 1e-3;
        let (p0, p1) = (c.at(end - eps, c.walk()), c.at(end + eps, c.walk()));
        let tl = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2))
            .sqrt()
            .max(1e-12);
        let t = ((p1.0 - p0.0) / tl, (p1.1 - p0.1) / tl);
        let w = c.width;
        let pts: Vec<Uv> = break_profile(w, p.plan.break_style, p.plan.break_angle, size)
            .into_iter()
            .map(|(lat, off)| {
                let q = c.at_lat(end, lat);
                (q.0 + t.0 * off, q.1 + t.1 * off)
            })
            .collect();
        out.push(poly(pts, false));
    } else {
        out.push(Stroke::Polyline(
            layout.footprint.iter().map(|&p| to_plan(p)).collect(),
            true,
        ));
    }

    // The centre pole of a spiral stair.
    if p.spiral {
        out.push(Stroke::Arc {
            center: to_plan(c.center),
            radius: c.inner.max(1.0),
            start_deg: 0.0,
            end_deg: 360.0,
        });
    }

    // The walkline: an arc at the walking radius.
    if p.walkline.show {
        let pts = curve_arc(c, if c.left { c.walk_off } else { c.width - c.walk_off }, 0.0, end);
        out.push(poly(pts, false));
    }

    if p.plan.number_treads {
        let angle = stair.direction.to_degrees().rem_euclid(360.0);
        for j in 1..=c.treads {
            let a = c.step * (f64::from(j) - 0.5);
            if a > end + 1e-9 {
                continue;
            }
            out.push(Stroke::Text {
                pos: to_plan(c.at_lat(a, c.width * 0.18)),
                text: j.to_string(),
                height: NUMBER_HEIGHT,
                angle,
            });
        }
    }

    let circle_at = (CIRCLE_ALONG / c.walk().max(1e-9)).min(end / 2.0);
    out.extend(curve_arrow(stair, layout, c, circle_at, end));
    out
}

/// The plan symbol of a curved ramp: the outline, the edges of the landings,
/// handrails and the arrow along the walking line.
fn ramp_arc_symbol(stair: &Stair, layout: &Layout, arc: &RampArc) -> Vec<Stroke> {
    let to_plan = |p: Uv| layout.frame.uv(p);
    let c = &arc.curve;
    let p = &stair.params;
    let sweep = c.sweep();
    let mut out = vec![Stroke::Polyline(
        layout.footprint.iter().map(|&q| to_plan(q)).collect(),
        true,
    )];
    for &(a0, a1, _, rise) in &arc.segs {
        if rise.abs() < 1e-9 {
            out.push(Stroke::Polyline(
                arc_band(c, a0, a1).into_iter().map(to_plan).collect(),
                true,
            ));
        }
    }
    if p.handrail {
        for lat in [HANDRAIL_PLAN_INSET, c.width - HANDRAIL_PLAN_INSET] {
            out.push(Stroke::Polyline(
                curve_arc(c, lat, 0.0, sweep).into_iter().map(to_plan).collect(),
                false,
            ));
        }
    }
    if p.walkline.show {
        out.push(Stroke::Polyline(
            curve_arc(c, c.width / 2.0, 0.0, sweep)
                .into_iter()
                .map(to_plan)
                .collect(),
            false,
        ));
    }
    let circle_at = (CIRCLE_ALONG / c.walk().max(1e-9)).min(sweep / 2.0);
    out.extend(curve_arrow(stair, layout, c, circle_at, sweep));
    out
}

/// Map the cut fraction to `(flight index, distance along that flight)`.
fn locate_cut(flights: &[Flight], cut_at: Option<f64>) -> Option<(usize, f64)> {
    let total: f64 = flights.iter().map(|f| f.len).sum();
    let fraction = cut_at.filter(|f| f.is_finite())?.clamp(0.0, 1.0);
    if total <= 0.0 || fraction >= 1.0 {
        return None;
    }
    let mut remaining = fraction * total;
    for (i, f) in flights.iter().enumerate() {
        if remaining <= f.len {
            return Some((i, remaining));
        }
        remaining -= f.len;
    }
    None
}

/// The break line across a width `w` as `(lateral, offset along the stair)`
/// points: six of them, for any style (the line is straight outside the
/// middle fifth, where the style shows).
fn break_profile(w: f64, style: BreakStyle, angle_deg: f64, size: f64) -> Vec<(f64, f64)> {
    let lats = [0.0, 0.4 * w, 0.45 * w, 0.55 * w, 0.6 * w, w];
    let tilt = angle_deg.to_radians().tan();
    let shape = |k: usize, lat: f64| match style {
        BreakStyle::Zigzag => [0.0, 0.0, size, -size, 0.0, 0.0][k],
        BreakStyle::Straight => 0.0,
        // One slanted line corner to corner.
        BreakStyle::Slash => 2.0 * size * (lat / w.max(1e-9) - 0.5),
    };
    lats.iter()
        .enumerate()
        .map(|(k, &lat)| (lat, shape(k, lat) + tilt * (lat - w / 2.0)))
        .collect()
}

/// The break line across the flight at distance `at` from its first riser.
fn break_line(f: &Flight, at: f64, to_plan: &impl Fn(Uv) -> Point, params: &StairParams) -> Stroke {
    let o = &params.plan;
    let pts = break_profile(f.width, o.break_style, o.break_angle, o.break_size);
    Stroke::Polyline(
        pts.iter().map(|&(l, s)| to_plan(f.at(at + s, l))).collect(),
        false,
    )
}

/// Circle at the bottom, centreline with arrowhead at the top, and the UP label.
fn direction_arrow(stair: &Stair, layout: &Layout, last: usize, cut: Option<f64>) -> Vec<Stroke> {
    let to_plan = |p: Uv| layout.frame.uv(p);
    let opts = &stair.params.plan;
    let flights = &layout.flights;
    let f0 = &flights[0];
    let mid = f0.width / 2.0;
    let circle_along = CIRCLE_ALONG.min(f0.len / 2.0);
    let mut out = Vec::new();
    if opts.arrow != ArrowStyle::None {
        out.push(Stroke::Arc {
            center: to_plan(f0.at(circle_along, mid)),
            radius: CIRCLE_RADIUS,
            start_deg: 0.0,
            end_deg: 360.0,
        });

        let mut path = vec![f0.at(circle_along + CIRCLE_RADIUS, mid)];
        for (i, f) in flights.iter().enumerate().take(last + 1) {
            let end = if i == last {
                cut.unwrap_or(f.len)
            } else {
                f.len
            };
            let exit = f.at(f.len, mid);
            path.push(f.at(end, mid));
            if i < last {
                let next = &flights[i + 1];
                let entry = next.at(0.0, next.width / 2.0);
                let dot = f.dir.0 * next.dir.0 + f.dir.1 * next.dir.1;
                if dot > 0.5 {
                    // Straight on (a ramp landing): the centreline just continues.
                } else if dot.abs() < 1e-9 {
                    // Quarter turn: meet where the two centrelines cross.
                    path.push(if f.dir.1 == 0.0 {
                        (entry.0, exit.1)
                    } else {
                        (exit.0, entry.1)
                    });
                } else {
                    // Half turn: cross the landing at its middle.
                    let half = landing_extent(layout, f.dir) / 2.0;
                    path.push((exit.0 + f.dir.0 * half, exit.1 + f.dir.1 * half));
                    path.push((entry.0 + f.dir.0 * half, entry.1 + f.dir.1 * half));
                }
                path.push(entry);
            }
        }

        if let [.., a, b] = path[..] {
            let seg = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            if seg > 1e-6 && path.len() > 1 {
                out.push(Stroke::Polyline(
                    path.iter().map(|&p| to_plan(p)).collect(),
                    false,
                ));
                let dir = Point::new((b.0 - a.0) / seg, (b.1 - a.1) / seg);
                let dir_plan = layout.frame.vector((dir.x, dir.y));
                let head = arrow_head(opts.arrow, opts.arrow_size, to_plan(b), dir_plan, seg);
                out.extend(head);
            }
        }
    }

    let label_lateral = (mid + CIRCLE_RADIUS + 0.5 * TEXT_HEIGHT + 1.0).min(f0.width);
    out.push(Stroke::Text {
        pos: to_plan(f0.at(circle_along + CIRCLE_RADIUS + 3.0, label_lateral)),
        text: "UP".into(),
        height: TEXT_HEIGHT,
        angle: stair.direction.to_degrees().rem_euclid(360.0),
    });
    out
}

/// Length of the landing outline measured along `dir`.
fn landing_extent(layout: &Layout, dir: Uv) -> f64 {
    let Some(outline) = layout.outlines.first() else {
        return 0.0;
    };
    let proj = |p: &Uv| p.0 * dir.0 + p.1 * dir.1;
    let max = outline.iter().map(proj).fold(f64::MIN, f64::max);
    let min = outline.iter().map(proj).fold(f64::MAX, f64::min);
    max - min
}
