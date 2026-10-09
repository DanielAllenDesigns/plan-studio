//! Floor platform framing: joists, rim joists and mid-span blocking.

use crate::build::{BlockingStyle, Connection, DetailOptions};
use crate::defaults::FramingDefaults;
use crate::lumber::{Lumber, TWO_BY_THICKNESS};
use crate::member::{add, scale, Member, MemberKind, Transform3, Vec3};
use plan_core::{Point, Room};
use serde::{Deserialize, Serialize};

/// Subfloor thickness between the joist tops and the finished platform.
const SUBFLOOR: f64 = 0.75;
/// Longest unblocked joist span, inches.
const MAX_UNBLOCKED_SPAN: f64 = 96.0;
const EPS: f64 = 1e-6;

/// Which way the joists run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoistDirection {
    /// Joists parallel to plan X (spanning the room's width).
    AlongX,
    /// Joists parallel to plan Y (spanning the room's depth).
    AlongY,
    /// Span the shorter bounding-box dimension.
    Auto,
}

/// One joist line: its constant coordinate and the (entry, exit) intervals
/// along the span axis where it lies inside the room.
struct Line {
    across: f64,
    spans: Vec<(f64, f64)>,
}

/// Frame a floor platform over `room`'s polygon.
///
/// Joists are spaced on centre across the bounding box starting with a joist
/// flush with the first edge and one flush with the last. Each joist line is
/// clipped to the polygon with an even-odd scan, so concave rooms get a joist
/// per interior interval. With `rim_joist`, a rim joist runs along every
/// polygon edge perpendicular to the joists (axis-aligned edges only) and
/// joists are shortened to butt against it; other edges get no rim. The joist
/// top sits 3/4" below `floor_elevation` (the subfloor). The polygon is the
/// room's wall-centreline outline, so no wall thickness is deducted.
pub fn frame_floor(
    room: &Room,
    floor_elevation: f64,
    d: &FramingDefaults,
    direction: JoistDirection,
) -> Vec<Member> {
    frame_floor_holes(room, floor_elevation, d, direction, &[])
}

/// Frame a ceiling over `room`: ceiling joists (`d.ceiling_joist_size` at
/// `d.ceiling_joist_spacing`, running `d.ceiling_direction`) that rest on the
/// top plates, so their bottoms sit at `plate_top`. No rim joists or blocking.
/// The members are [`MemberKind::CeilingJoist`].
pub fn frame_ceiling(room: &Room, plate_top: f64, d: &FramingDefaults) -> Vec<Member> {
    frame_ceiling_ref(room, plate_top, d, &DetailOptions::default(), None)
}

/// [`frame_ceiling`] with the detail options and the joists laid out from a
/// Framing Reference Marker.
pub fn frame_ceiling_ref(
    room: &Room,
    plate_top: f64,
    d: &FramingDefaults,
    opts: &DetailOptions,
    reference: Option<Point>,
) -> Vec<Member> {
    let as_floor = FramingDefaults {
        joist_size: d.ceiling_joist_size,
        joist_spacing: d.ceiling_joist_spacing,
        rim_joist: false,
        blocking: false,
        ..d.clone()
    };
    // `frame_floor` puts the joist tops one subfloor thickness below the
    // elevation it is given; the ceiling joists stand on the plate instead.
    let elevation = plate_top + SUBFLOOR + d.ceiling_joist_size.depth;
    frame_floor_ref(
        room,
        elevation,
        &as_floor,
        d.ceiling_direction,
        &[],
        opts,
        true,
        reference,
    )
    .into_iter()
    .map(|mut m| {
        m.kind = MemberKind::CeilingJoist;
        m
    })
    .collect()
}

/// A hole in the platform (a stairwell) in the frame of the joists: `s` along
/// the joists, `c` across them.
struct HoleBox {
    s0: f64,
    s1: f64,
    c0: f64,
    c1: f64,
}

/// [`frame_floor`] with holes (stairwells, from the platform's Floor Hole
/// outlines). Each hole's bounding box is framed the way a stairwell is:
///
/// * `d.hole_plies` **trimmer joists** run the full joist span on each side of
///   the hole, replacing the common joists they stand on;
/// * `d.hole_plies` **header joists** run across the joists at each end of the
///   hole, between the trimmers;
/// * common joists inside the hole's width are cut short and butt the headers.
///
/// Holes outside the room (or not inside a single joist span) are ignored.
pub fn frame_floor_holes(
    room: &Room,
    floor_elevation: f64,
    d: &FramingDefaults,
    direction: JoistDirection,
    holes: &[Vec<Point>],
) -> Vec<Member> {
    frame_floor_opts(
        room,
        floor_elevation,
        d,
        direction,
        holes,
        &DetailOptions::default(),
        false,
    )
}

/// [`frame_floor_holes`] with the framing detail options: the Rim Joist
/// Width, the Stagger or Flush connection of double rim joists, a maximum rim
/// board length and the blocking style (In Line, Stagger or Cross/Bridging).
/// `ceiling` takes the ceiling's blocking style.
pub fn frame_floor_opts(
    room: &Room,
    floor_elevation: f64,
    d: &FramingDefaults,
    direction: JoistDirection,
    holes: &[Vec<Point>],
    opts: &DetailOptions,
    ceiling: bool,
) -> Vec<Member> {
    frame_floor_ref(room, floor_elevation, d, direction, holes, opts, ceiling, None)
}

/// [`frame_floor_opts`] with the joist layout anchored at a Framing Reference
/// Marker: a joist centre falls on the marker's position plus whole
/// spacings, and edge joists close the gaps the grid leaves (manual p. 919).
#[allow(clippy::too_many_arguments)]
pub fn frame_floor_ref(
    room: &Room,
    floor_elevation: f64,
    d: &FramingDefaults,
    direction: JoistDirection,
    holes: &[Vec<Point>],
    opts: &DetailOptions,
    ceiling: bool,
    reference: Option<Point>,
) -> Vec<Member> {
    let poly = &room.polygon;
    if poly.len() < 3 {
        return Vec::new();
    }
    let (min, max) = bounds(poly);
    let along_x = match direction {
        JoistDirection::AlongX => true,
        JoistDirection::AlongY => false,
        JoistDirection::Auto => max.x - min.x <= max.y - min.y,
    };
    let lumber = d.joist_size;
    let t = lumber.thickness;
    let y_mid = floor_elevation - SUBFLOOR - lumber.depth / 2.0;
    // Map (span, across) back to the 3D frame.
    let to3 = |span: f64, across: f64, y: f64| -> Vec3 {
        if along_x {
            [span, y, -across]
        } else {
            [across, y, -span]
        }
    };
    let span_dir: Vec3 = if along_x {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 0.0, -1.0]
    };
    let across_dir: Vec3 = if along_x {
        [0.0, 0.0, -1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let up: Vec3 = [0.0, 1.0, 0.0];
    let rim_plies = d.rim_plies.clamp(1, 3);
    let rim_width = if opts.rim_width > 0.1 {
        opts.rim_width
    } else {
        TWO_BY_THICKNESS
    };
    let rim = if d.rim_joist {
        rim_width * f64::from(rim_plies)
    } else {
        0.0
    };

    let (lo, hi) = if along_x {
        (min.y, max.y)
    } else {
        (min.x, max.x)
    };
    let origin = reference.map(|r| if along_x { r.y } else { r.x });
    let lines: Vec<Line> = joist_positions(lo, hi, d.joist_spacing.max(t), t, origin)
        .into_iter()
        .map(|across| Line {
            across,
            spans: clip(poly, along_x, across)
                .into_iter()
                .map(|(a, b)| (a + rim, b - rim))
                .filter(|&(a, b)| b - a > 1.0)
                .collect(),
        })
        .collect();

    // Holes: cut or replace the common joists, add trimmers and headers.
    let rim_in = |across: f64| -> Vec<(f64, f64)> {
        clip(poly, along_x, across)
            .into_iter()
            .map(|(a, b)| (a + rim, b - rim))
            .filter(|&(a, b)| b - a > 1.0)
            .collect()
    };
    let to_hole = |outline: &Vec<Point>| -> Option<HoleBox> {
        let (lo, hi) = bounds(outline);
        let (s0, s1, c0, c1) = if along_x {
            (lo.x, hi.x, lo.y, hi.y)
        } else {
            (lo.y, hi.y, lo.x, hi.x)
        };
        (outline.len() >= 3 && s1 - s0 > 1.0 && c1 - c0 > 1.0).then_some(HoleBox { s0, s1, c0, c1 })
    };
    let plies = d.hole_plies.max(1);
    let zone = f64::from(plies) * t;
    let mut hole_members: Vec<Member> = Vec::new();
    let mut lines = lines;
    for h in holes.iter().filter_map(to_hole) {
        // The span the hole sits in; skip a hole the joists do not cross.
        let mid = (h.c0 + h.c1) / 2.0;
        let Some(&(a, b)) = rim_in(mid)
            .iter()
            .find(|&&(a, b)| a <= h.s0 + EPS && b >= h.s1 - EPS)
        else {
            continue;
        };
        for line in &mut lines {
            let (lo, hi) = (line.across - t / 2.0, line.across + t / 2.0);
            if hi <= h.c0 - zone + EPS || lo >= h.c1 + zone - EPS {
                continue;
            }
            let inside = lo >= h.c0 - EPS && hi <= h.c1 + EPS;
            if inside {
                // Cut short: the tails butt the headers.
                let mut cut = Vec::new();
                for &(sa, sb) in &line.spans {
                    if sa <= h.s0 + EPS && sb >= h.s1 - EPS {
                        cut.push((sa, h.s0 - zone));
                        cut.push((h.s1 + zone, sb));
                    } else {
                        cut.push((sa, sb));
                    }
                }
                cut.retain(|&(x, y)| y - x > 1.0);
                line.spans = cut;
            } else {
                // A trimmer takes its place.
                line.spans
                    .retain(|&(sa, sb)| !(sa <= h.s0 + EPS && sb >= h.s1 - EPS));
            }
        }
        for k in 0..plies {
            let off = (f64::from(k) + 0.5) * t;
            for across in [h.c0 - off, h.c1 + off] {
                if let Some((sa, sb)) = rim_in(across)
                    .into_iter()
                    .find(|&(x, y)| x <= h.s0 + EPS && y >= h.s1 - EPS)
                {
                    let tf = Transform3 {
                        origin: to3(sa, across, y_mid),
                        axis_x: span_dir,
                        axis_y: up,
                    };
                    hole_members.push(Member::new(
                        MemberKind::TrimmerJoist,
                        lumber,
                        sb - sa,
                        tf,
                        None,
                    ));
                }
            }
            for span_at in [h.s0 - off, h.s1 + off] {
                if span_at < a || span_at > b {
                    continue;
                }
                let tf = Transform3 {
                    origin: to3(span_at, h.c0, y_mid),
                    axis_x: across_dir,
                    axis_y: up,
                };
                hole_members.push(Member::new(
                    MemberKind::HeaderJoist,
                    lumber,
                    h.c1 - h.c0,
                    tf,
                    None,
                ));
            }
        }
    }

    let mut out = Vec::new();
    for line in &lines {
        for &(a, b) in &line.spans {
            let tf = Transform3 {
                origin: to3(a, line.across, y_mid),
                axis_x: span_dir,
                axis_y: up,
            };
            out.push(Member::new(MemberKind::Joist, lumber, b - a, tf, None));
        }
    }

    out.extend(hole_members);

    if d.rim_joist {
        let ccw = signed_area(poly) >= 0.0;
        for (i, &p) in poly.iter().enumerate() {
            let q = poly[(i + 1) % poly.len()];
            let e = q - p;
            let perpendicular = if along_x { e.x.abs() } else { e.y.abs() } <= EPS;
            if !perpendicular || e.length() <= 1.0 {
                continue;
            }
            let inward = if ccw { e.perp() } else { -e.perp() }.normalized();
            let dir = e.normalized();
            let rim_lumber = Lumber {
                thickness: rim_width,
                depth: lumber.depth,
            };
            // Boards no longer than the maximum, cut evenly along the edge.
            let boards = if opts.max_rim_length > 1.0 {
                (e.length() / opts.max_rim_length).ceil().max(1.0) as usize
            } else {
                1
            };
            for ply in 0..rim_plies {
                // Stagger: every ply after the first stops one board
                // thickness short at alternating ends of alternating edges;
                // Flush runs every ply to the platform edge.
                let (cut_start, cut_end) = match opts.rim_connection {
                    Connection::Flush => (0.0, 0.0),
                    Connection::Stagger if ply == 0 => (0.0, 0.0),
                    Connection::Stagger => {
                        if (i + ply as usize) % 2 == 0 {
                            (rim_width, 0.0)
                        } else {
                            (0.0, rim_width)
                        }
                    }
                };
                let base = p + inward * (rim_width * (f64::from(ply) + 0.5));
                let run = e.length() - cut_start - cut_end;
                let piece = run / boards as f64;
                for k in 0..boards {
                    let start = base + dir * (cut_start + piece * k as f64);
                    let tf = Transform3 {
                        origin: [start.x, y_mid, -start.y],
                        axis_x: [dir.x, 0.0, -dir.y],
                        axis_y: up,
                    };
                    out.push(Member::new(MemberKind::RimJoist, rim_lumber, piece, tf, None));
                }
            }
        }
    }

    if d.blocking {
        let style = if ceiling {
            opts.ceiling_blocking_style
        } else {
            opts.blocking_style
        };
        for pair in lines.windows(2) {
            let (l0, l1) = (&pair[0], &pair[1]);
            let gap = l1.across - l0.across - t;
            if gap < 1.0 {
                continue;
            }
            if l0.spans.len() != l1.spans.len() {
                continue;
            }
            for (bay, (&(a0, b0), &(a1, b1))) in l0.spans.iter().zip(&l1.spans).enumerate() {
                let (a, b) = (a0.max(a1), b0.min(b1));
                let span = b - a;
                if span <= MAX_UNBLOCKED_SPAN {
                    continue;
                }
                let rows = (span / MAX_UNBLOCKED_SPAN).ceil() as u32 - 1;
                let pair_index = (l0.across / d.joist_spacing.max(1.0)).round() as i64 + bay as i64;
                for r in 1..=rows {
                    let mut at = a + span * f64::from(r) / f64::from(rows + 1);
                    // Stagger: alternate bays sit either side of the row.
                    if style == BlockingStyle::Stagger {
                        at += if pair_index % 2 == 0 { t } else { -t };
                    }
                    if style == BlockingStyle::Cross {
                        // Two crossed bridging boards in the bay.
                        let h = (lumber.depth * 0.9).max(1.0);
                        let len = gap.hypot(h);
                        let bridge = Lumber {
                            thickness: 1.0,
                            depth: 3.0,
                        };
                        for flip in [false, true] {
                            let (y0, y1) = if flip {
                                (y_mid + h / 2.0, y_mid - h / 2.0)
                            } else {
                                (y_mid - h / 2.0, y_mid + h / 2.0)
                            };
                            let from = add(to3(at, l0.across, y0), scale(across_dir, t / 2.0));
                            let dirv = [
                                across_dir[0] * gap / len,
                                (y1 - y0) / len,
                                across_dir[2] * gap / len,
                            ];
                            let tf = Transform3 {
                                origin: from,
                                axis_x: dirv,
                                axis_y: span_dir,
                            };
                            let mut m = Member::new(MemberKind::Blocking, bridge, len, tf, None);
                            m.label = format!("cross bridging x {}", crate::lumber::format_inches(len));
                            out.push(m);
                        }
                        continue;
                    }
                    let start = add(to3(at, l0.across, y_mid), scale(across_dir, t / 2.0));
                    let tf = Transform3 {
                        origin: start,
                        axis_x: across_dir,
                        axis_y: up,
                    };
                    out.push(Member::new(MemberKind::Blocking, lumber, gap, tf, None));
                }
            }
        }
    }
    out
}

/// Framing for a tray ceiling (manual p. 458, "Build Framing for Selected
/// Object"): the side walls of the step and the ceiling joists of the
/// ceilings it makes.
///
/// * Vertical sides: a wall along each edge of the hole, just outside it - a
///   bottom plate at the lower ceiling, a top plate under the upper one and
///   studs between them at `d.stud_spacing` (the last stud at the corner) -
///   as [`MemberKind::BottomPlate`], [`MemberKind::TopPlate`] and
///   [`MemberKind::Stud`].
/// * Sloped sides: one [`MemberKind::Rafter`] per `rec.rafter_spacing` along
///   each edge, running up the slope from the outer ceiling to the hole.
/// * Ceiling joists ([`MemberKind::CeilingJoist`], `d.ceiling_joist_size` at
///   `rec.rafter_spacing`): over the outer ceiling (the ring around the hole)
///   when it hangs below the surface the tray sits in, and over the inner
///   ceiling when a Recess into Ceiling raises it.
///
/// Heights in `geom` are above the floor datum; `floor_elevation` is added.
/// A tray with a Caution or with Retain Framing set makes no members.
pub fn frame_tray_ceiling(
    geom: &plan_core::tray::TrayGeom,
    rec: &plan_core::tray::TrayCeiling,
    floor_elevation: f64,
    d: &FramingDefaults,
) -> Vec<Member> {
    if !geom.ok() || rec.retain_framing || geom.inner.len() < 3 {
        return Vec::new();
    }
    let up: Vec3 = [0.0, 1.0, 0.0];
    let to3 = |p: Point, y: f64| -> Vec3 { [p.x, y, -p.y] };
    let dir3 = |v: Point| -> Vec3 { [v.x, 0.0, -v.y] };
    let (low, high) = (floor_elevation + geom.low(), floor_elevation + geom.high());
    let lumber = d.stud_size;
    let t = lumber.thickness;
    let inner = plan_core::tray::ccw(&geom.inner);
    let n = inner.len();
    let mut out = Vec::new();

    for i in 0..n {
        let (a, b) = (inner[i], inner[(i + 1) % n]);
        let len = a.dist(b);
        if len < 1.0 {
            continue;
        }
        let dir = (b - a).normalized();
        // Outward from a counter-clockwise hole is to the right of the edge.
        let outward = -dir.perp();
        let step = |dist: f64| a + dir * dist + outward * (lumber.depth / 2.0);
        if geom.run > 1e-6 {
            // A rafter up the slope: its low end on the outer ceiling's edge
            // `run` out from the hole, its high end at the hole.
            let spacing = rec.rafter_spacing.max(t);
            let slope = (geom.run * geom.run + (geom.h_inner - geom.h_outer).powi(2)).sqrt();
            let mut at: f64 = 0.0;
            loop {
                let along = at.min(len);
                let foot = a + dir * along + outward * geom.run;
                let top = a + dir * along;
                let (y0, y1) = (
                    floor_elevation + geom.h_outer,
                    floor_elevation + geom.h_inner,
                );
                let axis: Vec3 = [
                    (top.x - foot.x) / slope,
                    (y1 - y0) / slope,
                    -(top.y - foot.y) / slope,
                ];
                let tf = Transform3 {
                    origin: to3(foot, y0),
                    axis_x: axis,
                    axis_y: dir3(dir),
                };
                out.push(Member::new(
                    MemberKind::Rafter,
                    d.ceiling_joist_size,
                    slope,
                    tf,
                    None,
                ));
                if along >= len {
                    break;
                }
                at += spacing;
            }
            continue;
        }
        // Plates lie flat along the edge: depth across the wall, thickness up.
        let flat = |kind: MemberKind, y: f64| {
            let tf = Transform3 {
                origin: to3(step(0.0), y),
                axis_x: dir3(dir),
                axis_y: dir3(outward),
            };
            Member::new(kind, lumber, len, tf, None)
        };
        out.push(flat(MemberKind::BottomPlate, low + t / 2.0));
        out.push(flat(MemberKind::TopPlate, high - t / 2.0));
        let stud_len = (high - low - 2.0 * t).max(0.0);
        if stud_len < 0.5 {
            continue;
        }
        let spacing = d.stud_spacing.max(t);
        let mut at: f64 = 0.0;
        loop {
            // Studs are centred on `at`; the first and last sit flush.
            let centre = at.clamp(t / 2.0, (len - t / 2.0).max(t / 2.0));
            let tf = Transform3 {
                origin: to3(step(centre), low + t),
                axis_x: up,
                axis_y: dir3(outward),
            };
            out.push(Member::new(MemberKind::Stud, lumber, stud_len, tf, None));
            if at >= len {
                break;
            }
            at += spacing;
        }
    }

    // Ceiling joists for the ceilings the tray makes.
    let joist_defaults = FramingDefaults {
        ceiling_joist_spacing: rec.rafter_spacing.max(t),
        ..d.clone()
    };
    let joists = |poly: &[Point], holes: &[Vec<Point>], level: f64| -> Vec<Member> {
        let room = Room {
            polygon: poly.to_vec(),
            ..Room::default()
        };
        let as_floor = FramingDefaults {
            joist_size: joist_defaults.ceiling_joist_size,
            joist_spacing: joist_defaults.ceiling_joist_spacing,
            rim_joist: false,
            blocking: false,
            ..joist_defaults.clone()
        };
        let elevation = level + SUBFLOOR + joist_defaults.ceiling_joist_size.depth;
        frame_floor_holes(
            &room,
            elevation,
            &as_floor,
            joist_defaults.ceiling_direction,
            holes,
        )
        .into_iter()
        .map(|mut m| {
            m.kind = MemberKind::CeilingJoist;
            m
        })
        .collect()
    };
    if geom.h_outer < geom.base - 1e-6 {
        let edge = if geom.run > 1e-6 {
            plan_core::tray::outset_outline(&geom.inner, geom.run)
        } else {
            geom.inner.clone()
        };
        out.extend(joists(&geom.outer, &[edge], floor_elevation + geom.h_outer));
    }
    if geom.h_inner > geom.base + 1e-6 {
        out.extend(joists(&geom.inner, &[], floor_elevation + geom.h_inner));
    }
    out
}

fn bounds(poly: &[Point]) -> (Point, Point) {
    poly.iter().fold(
        (
            Point::new(f64::INFINITY, f64::INFINITY),
            Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
        ),
        |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        },
    )
}

fn signed_area(poly: &[Point]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| poly[i].cross(poly[(i + 1) % n]))
        .sum::<f64>()
        / 2.0
}

/// Joist centre positions across `[lo, hi]`: flush at both edges, on `step`
/// centres between, never overlapping the last joist.
fn joist_positions(lo: f64, hi: f64, step: f64, t: f64, origin: Option<f64>) -> Vec<f64> {
    if hi - lo < t {
        return Vec::new();
    }
    let (first, last) = (lo + t / 2.0, hi - t / 2.0);
    if let Some(o) = origin {
        let k0 = ((first - o) / step - EPS).ceil() as i64;
        let mut v: Vec<f64> = (k0..)
            .map(|k| o + k as f64 * step)
            .take_while(|&p| p <= last + EPS)
            .collect();
        if v.first().is_none_or(|&p| p - first > t + EPS) {
            v.insert(0, first);
        }
        if v.last().is_none_or(|&p| last - p > t + EPS) {
            v.push(last);
        }
        return v;
    }
    let mut v: Vec<f64> = (0..)
        .map(|k| first + f64::from(k) * step)
        .take_while(|&p| p <= last - t + EPS)
        .collect();
    if last - first > EPS {
        v.push(last);
    }
    v
}

/// Even-odd intervals `(entry, exit)` along the span axis where the line at
/// `across` lies inside `poly`.
fn clip(poly: &[Point], along_x: bool, across: f64) -> Vec<(f64, f64)> {
    let uv = |p: Point| if along_x { (p.x, p.y) } else { (p.y, p.x) };
    let mut hits: Vec<f64> = (0..poly.len())
        .filter_map(|i| {
            let (s0, c0) = uv(poly[i]);
            let (s1, c1) = uv(poly[(i + 1) % poly.len()]);
            // Half-open rule so a vertex on the line counts exactly once.
            ((c0 <= across) != (c1 <= across)).then(|| s0 + (across - c0) / (c1 - c0) * (s1 - s0))
        })
        .collect();
    hits.sort_by(f64::total_cmp);
    hits.as_chunks::<2>()
        .0
        .iter()
        .map(|c| (c[0], c[1]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::polygon_area;

    fn room(pts: &[(f64, f64)]) -> Room {
        let polygon: Vec<Point> = pts.iter().map(|&(x, y)| Point::new(x, y)).collect();
        Room {
            area_sq_in: polygon_area(&polygon).abs(),
            centroid: Point::ZERO,
            polygon,
            label: String::new(),
            ..Room::default()
        }
    }

    fn rect() -> Room {
        room(&[(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)])
    }

    fn of(m: &[Member], k: MemberKind) -> Vec<&Member> {
        m.iter().filter(|x| x.kind == k).collect()
    }

    #[test]
    fn auto_spans_shorter_dimension() {
        let m = frame_floor(
            &rect(),
            0.0,
            &FramingDefaults::default(),
            JoistDirection::Auto,
        );
        let joists = of(&m, MemberKind::Joist);
        // floor((240 - 1.5) / 16) + 2
        assert_eq!(joists.len(), 16);
        // Joists run along plan Y (3D -Z), 120" less two 1 1/2" rims.
        for j in &joists {
            assert_eq!(j.transform.axis_x, [0.0, 0.0, -1.0]);
            assert_eq!(j.length, 117.0);
            assert_eq!(j.label, "2x10 x 117");
        }
        let xs: Vec<f64> = joists.iter().map(|j| j.transform.origin[0]).collect();
        assert_eq!(xs[0], 0.75);
        assert_eq!(xs[1], 16.75);
        assert_eq!(*xs.last().unwrap(), 239.25);
        // Top of joist 3/4" below the platform, depth downward.
        let top = joists[0].transform.origin[1] + joists[0].lumber.depth / 2.0;
        assert_eq!(top, -0.75);
        let rims = of(&m, MemberKind::RimJoist);
        assert_eq!(rims.len(), 2);
        assert!(rims.iter().all(|r| r.length == 240.0));
        let zs: Vec<f64> = rims.iter().map(|r| r.transform.origin[2]).collect();
        assert_eq!(zs, [-0.75, -119.25]);
    }

    #[test]
    fn explicit_direction_and_no_rim() {
        let d = FramingDefaults {
            rim_joist: false,
            ..FramingDefaults::default()
        };
        let m = frame_floor(&rect(), 100.0, &d, JoistDirection::AlongX);
        let joists = of(&m, MemberKind::Joist);
        assert_eq!(joists.len(), 8 + 1); // 0.75 + 16k <= 120 - 2.25, plus the last
        assert!(joists.iter().all(|j| j.length == 240.0));
        assert!(of(&m, MemberKind::RimJoist).is_empty());
    }

    #[test]
    fn blocking_at_mid_span() {
        let d = FramingDefaults {
            blocking: true,
            ..FramingDefaults::default()
        };
        let m = frame_floor(&rect(), 0.0, &d, JoistDirection::Auto);
        let blocks = of(&m, MemberKind::Blocking);
        // One row (117" span > 96") between each of the 15 joist pairs.
        assert_eq!(blocks.len(), 15);
        assert!((blocks[0].length - 14.5).abs() < 1e-9);
        // Mid-span: plan y = 1.5 + 58.5 = 60 -> z = -60.
        assert!((blocks[0].transform.origin[2] + 60.0).abs() < 1e-9);
    }

    #[test]
    fn l_shaped_room_clips_joists() {
        let l = room(&[
            (0.0, 0.0),
            (240.0, 0.0),
            (240.0, 60.0),
            (120.0, 60.0),
            (120.0, 120.0),
            (0.0, 120.0),
        ]);
        let d = FramingDefaults {
            rim_joist: false,
            ..FramingDefaults::default()
        };
        let m = frame_floor(&l, 0.0, &d, JoistDirection::AlongY);
        let lens: Vec<f64> = of(&m, MemberKind::Joist).iter().map(|j| j.length).collect();
        assert!(lens.contains(&120.0) && lens.contains(&60.0));
        assert!(lens.iter().all(|&l| l == 120.0 || l == 60.0));
    }

    /// Plan-space (min, max) of a member's box.
    fn plan_box(m: &Member) -> (Point, Point) {
        let pts: Vec<Point> = m
            .corners()
            .iter()
            .map(|c| Point::new(c[0], -c[2]))
            .collect();
        let lo = pts.iter().fold(Point::new(1e9, 1e9), |a, p| {
            Point::new(a.x.min(p.x), a.y.min(p.y))
        });
        let hi = pts.iter().fold(Point::new(-1e9, -1e9), |a, p| {
            Point::new(a.x.max(p.x), a.y.max(p.y))
        });
        (lo, hi)
    }

    fn square_hole(x0: f64, y0: f64, w: f64, h: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x0 + w, y0),
            Point::new(x0 + w, y0 + h),
            Point::new(x0, y0 + h),
        ]
    }

    #[test]
    fn stair_hole_gets_trimmers_headers_and_cut_joists() {
        let d = FramingDefaults::default();
        let hole = square_hole(100.0, 30.0, 36.0, 48.0);
        let plain = frame_floor(&rect(), 0.0, &d, JoistDirection::Auto);
        let m = frame_floor_holes(
            &rect(),
            0.0,
            &d,
            JoistDirection::Auto,
            std::slice::from_ref(&hole),
        );
        // Two plies each side, two plies each end.
        let trimmers = of(&m, MemberKind::TrimmerJoist);
        let headers = of(&m, MemberKind::HeaderJoist);
        assert_eq!(trimmers.len(), 4);
        assert_eq!(headers.len(), 4);
        // Trimmers run the whole joist span (117"), headers span the hole's width.
        assert!(trimmers.iter().all(|t| (t.length - 117.0).abs() < 1e-9));
        assert!(headers.iter().all(|h| (h.length - 36.0).abs() < 1e-9));
        // Trimmers stand just outside the hole's sides, headers outside its ends.
        let mut xs: Vec<f64> = trimmers.iter().map(|t| plan_box(t).0.x + 0.75).collect();
        xs.sort_by(f64::total_cmp);
        assert_eq!(xs, [97.75, 99.25, 136.75, 138.25]);
        let mut ys: Vec<f64> = headers.iter().map(|h| plan_box(h).0.y + 0.75).collect();
        ys.sort_by(f64::total_cmp);
        assert_eq!(ys, [27.75, 29.25, 78.75, 80.25]);
        // The joists inside the hole's width are cut to tails that butt the headers.
        let (hlo, hhi) = (Point::new(100.0, 30.0), Point::new(136.0, 78.0));
        for j in of(&m, MemberKind::Joist) {
            let (lo, hi) = plan_box(j);
            let clear = hi.x <= hlo.x + 1e-9
                || lo.x >= hhi.x - 1e-9
                || hi.y <= hlo.y + 1e-9
                || lo.y >= hhi.y - 1e-9;
            assert!(clear, "a joist crosses the hole: {lo:?} {hi:?}");
        }
        let tails: Vec<f64> = of(&m, MemberKind::Joist)
            .iter()
            .filter(|j| j.length < 117.0)
            .map(|j| j.length)
            .collect();
        assert_eq!(tails.len(), 4);
        assert!(tails.iter().any(|l| (l - 25.5).abs() < 1e-9));
        assert!(tails.iter().any(|l| (l - 37.5).abs() < 1e-9));
        // 16 plain joists: one swallowed by a trimmer, two cut in two.
        assert_eq!(of(&plain, MemberKind::Joist).len(), 16);
        assert_eq!(of(&m, MemberKind::Joist).len(), 16 - 1 - 2 + 4);
        // Rim joists are untouched; a hole outside the room changes nothing.
        assert_eq!(of(&m, MemberKind::RimJoist).len(), 2);
        let far = square_hole(500.0, 500.0, 36.0, 36.0);
        let same = frame_floor_holes(&rect(), 0.0, &d, JoistDirection::Auto, &[far]);
        assert_eq!(same.len(), plain.len());
    }

    #[test]
    fn rim_joists_run_along_the_edges_the_joists_butt() {
        let m = frame_floor(
            &rect(),
            0.0,
            &FramingDefaults::default(),
            JoistDirection::AlongX,
        );
        // Joists run along X, so the rims are the two 120" ends.
        let rims = of(&m, MemberKind::RimJoist);
        assert_eq!(rims.len(), 2);
        assert!(rims.iter().all(|r| (r.length - 120.0).abs() < 1e-9));
        // Each joist is shortened by both rims.
        assert!(of(&m, MemberKind::Joist)
            .iter()
            .all(|j| (j.length - 237.0).abs() < 1e-9));
    }

    #[test]
    fn a_double_rim_adds_a_second_ply_and_shortens_the_joists() {
        let d = FramingDefaults {
            rim_plies: 2,
            ..FramingDefaults::default()
        };
        let m = frame_floor(&rect(), 0.0, &d, JoistDirection::Auto);
        assert_eq!(of(&m, MemberKind::RimJoist).len(), 4);
        // 120" less two double rims of 3".
        assert!(of(&m, MemberKind::Joist).iter().all(|j| j.length == 114.0));
        let zs: Vec<f64> = of(&m, MemberKind::RimJoist)
            .iter()
            .map(|r| r.transform.origin[2])
            .collect();
        assert_eq!(zs, [-0.75, -2.25, -119.25, -117.75]);
    }

    #[test]
    fn ceiling_joists_stand_on_the_plate_with_their_own_size_and_spacing() {
        let d = FramingDefaults {
            ceiling_joist_size: crate::lumber::TWO_BY_EIGHT,
            ceiling_joist_spacing: 24.0,
            ceiling_direction: JoistDirection::AlongX,
            ..FramingDefaults::default()
        };
        let m = frame_ceiling(&rect(), 109.0, &d);
        assert!(!m.is_empty());
        assert!(m.iter().all(|j| j.kind == MemberKind::CeilingJoist));
        assert!(m.iter().all(|j| j.lumber == crate::lumber::TWO_BY_EIGHT));
        // Running along X: each joist spans the 240" length.
        assert!(m.iter().all(|j| j.transform.axis_x == [1.0, 0.0, 0.0]));
        assert!(m.iter().all(|j| (j.length - 240.0).abs() < 1e-9));
        // Bottom of every joist on the plate.
        for j in &m {
            let bottom = j.transform.origin[1] - j.lumber.depth / 2.0;
            assert!((bottom - 109.0).abs() < 1e-9, "{bottom}");
        }
        // 24" spacing: fewer joists than the 16" floor above.
        let floor = frame_floor(
            &rect(),
            0.0,
            &FramingDefaults::default(),
            JoistDirection::AlongX,
        );
        assert!(m.len() < of(&floor, MemberKind::Joist).len());
    }
    // ----- Round 16: rim joists and blocking styles -----

    #[test]
    fn the_rim_width_and_connection_shape_a_double_rim() {
        let d = FramingDefaults {
            rim_plies: 2,
            ..FramingDefaults::default()
        };
        let wide = DetailOptions {
            rim_width: 2.0,
            ..DetailOptions::default()
        };
        let m = frame_floor_opts(&rect(), 0.0, &d, JoistDirection::Auto, &[], &wide, false);
        // Two doubled rims of 2" boards take 8" of every joist.
        assert!(of(&m, MemberKind::Joist).iter().all(|j| (j.length - 112.0).abs() < 1e-9));
        let rims = of(&m, MemberKind::RimJoist);
        assert_eq!(rims.len(), 4);
        assert!(rims.iter().all(|r| (r.lumber.thickness - 2.0).abs() < 1e-9));
        // Stagger: the second ply of each edge stops one board short at one end.
        let mut lens: Vec<f64> = rims.iter().map(|r| r.length).collect();
        lens.sort_by(f64::total_cmp);
        assert_eq!(lens, [238.0, 238.0, 240.0, 240.0]);
        let flush = DetailOptions {
            rim_connection: Connection::Flush,
            ..wide.clone()
        };
        let m = frame_floor_opts(&rect(), 0.0, &d, JoistDirection::Auto, &[], &flush, false);
        assert!(of(&m, MemberKind::RimJoist).iter().all(|r| r.length == 240.0));
    }

    #[test]
    fn a_maximum_rim_length_cuts_the_rim_into_boards() {
        let opts = DetailOptions {
            max_rim_length: 100.0,
            ..DetailOptions::default()
        };
        let m = frame_floor_opts(
            &rect(),
            0.0,
            &FramingDefaults::default(),
            JoistDirection::Auto,
            &[],
            &opts,
            false,
        );
        let rims = of(&m, MemberKind::RimJoist);
        // Two 240" edges in three 80" boards each.
        assert_eq!(rims.len(), 6);
        assert!(rims.iter().all(|r| (r.length - 80.0).abs() < 1e-9));
    }

    #[test]
    fn blocking_goes_in_line_staggered_or_as_cross_bridging() {
        let d = FramingDefaults {
            blocking: true,
            ..FramingDefaults::default()
        };
        let run = |style| {
            let o = DetailOptions {
                blocking_style: style,
                ..DetailOptions::default()
            };
            frame_floor_opts(&rect(), 0.0, &d, JoistDirection::Auto, &[], &o, false)
        };
        let zs = |m: &[Member]| -> Vec<i64> {
            let mut v: Vec<i64> = of(m, MemberKind::Blocking)
                .iter()
                .map(|b| (b.transform.origin[2] * 100.0).round() as i64)
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let line = run(BlockingStyle::InLine);
        assert_eq!(of(&line, MemberKind::Blocking).len(), 15);
        assert_eq!(zs(&line).len(), 1);
        let stagger = run(BlockingStyle::Stagger);
        assert_eq!(of(&stagger, MemberKind::Blocking).len(), 15);
        assert_eq!(zs(&stagger).len(), 2);
        let cross = run(BlockingStyle::Cross);
        let boards = of(&cross, MemberKind::Blocking);
        // Two crossed boards in each of the 15 bays, named as bridging.
        assert_eq!(boards.len(), 30);
        assert!(boards.iter().all(|b| b.label.starts_with("cross bridging")));
        // In plan the pair covers the same bay as one in-line block.
        assert!(boards.iter().all(|b| b.length > 14.5 && b.length < 20.0));
    }
}

#[cfg(test)]
mod tray_tests {
    use super::*;
    use plan_core::tray::{resolve, RoomCeiling, TrayCeiling};
    use plan_core::Project;

    fn rect(x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(x1, 0.0),
            Point::new(x1, y1),
            Point::new(0.0, y1),
        ]
    }

    fn geom(spec: TrayCeiling) -> (plan_core::tray::TrayGeom, TrayCeiling) {
        let mut p = Project::new("t");
        let id = p.make_tray_in_room(0, &rect(240.0, 180.0), spec).unwrap();
        let room = RoomCeiling {
            outline: rect(240.0, 180.0),
            height: 96.0,
            flat: true,
        };
        let g = resolve(&p.floors[0], &[room]).remove(0);
        (g, p.floors[0].tray(id).unwrap().clone())
    }

    fn count(m: &[Member], k: MemberKind) -> usize {
        m.iter().filter(|x| x.kind == k).count()
    }

    #[test]
    fn a_vertical_step_gets_plates_and_studs_on_every_edge() {
        let (g, rec) = geom(TrayCeiling::default());
        let m = frame_tray_ceiling(&g, &rec, 0.0, &FramingDefaults::default());
        // Four edges: a bottom and a top plate on each.
        assert_eq!(count(&m, MemberKind::BottomPlate), 4);
        assert_eq!(count(&m, MemberKind::TopPlate), 4);
        let studs: Vec<&Member> = m.iter().filter(|x| x.kind == MemberKind::Stud).collect();
        // 192" edges: 0, 16 ... 192 -> 13 studs; 132" edges: 0 ... 128 and the
        // corner -> 10 studs.
        assert_eq!(studs.len(), 2 * 13 + 2 * 10);
        // Stud length is the 8" step less the two plates.
        assert!(studs.iter().all(|s| (s.length - (8.0 - 3.0)).abs() < 1e-9));
        // Plates run the edge, between the dropped (88") and upper (96") ceiling.
        let bottom = m
            .iter()
            .find(|x| x.kind == MemberKind::BottomPlate)
            .unwrap();
        assert!((bottom.transform.origin[1] - (88.0 + 0.75)).abs() < 1e-9);
        // The dropped ring is joisted below the surface it hangs from.
        assert!(count(&m, MemberKind::CeilingJoist) > 0);
        assert_eq!(count(&m, MemberKind::Rafter), 0);
    }

    #[test]
    fn a_sloped_step_gets_rafters_not_walls() {
        let (g, rec) = geom(TrayCeiling {
            pitch: Some(12.0),
            ..Default::default()
        });
        let m = frame_tray_ceiling(&g, &rec, 0.0, &FramingDefaults::default());
        assert_eq!(count(&m, MemberKind::Stud), 0);
        assert_eq!(count(&m, MemberKind::TopPlate), 0);
        let rafters = count(&m, MemberKind::Rafter);
        // 16" spacing along 192" and 132" edges, a rafter at each end.
        assert_eq!(rafters, 2 * 13 + 2 * 10);
        let r = m.iter().find(|x| x.kind == MemberKind::Rafter).unwrap();
        // 8" run and 8" rise: an 11.3" slope.
        assert!((r.length - 128f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn a_recessed_tray_joists_the_raised_ceiling_and_retain_framing_stops_it() {
        let (g, rec) = geom(TrayCeiling {
            recess: true,
            ..Default::default()
        });
        let m = frame_tray_ceiling(&g, &rec, 0.0, &FramingDefaults::default());
        let joists: Vec<&Member> = m
            .iter()
            .filter(|x| x.kind == MemberKind::CeilingJoist)
            .collect();
        assert!(!joists.is_empty());
        // Over the 192" x 132" hole: joists span the short way.
        assert!(joists.iter().all(|j| j.length <= 192.0 + 1e-6));
        let mut kept = rec.clone();
        kept.retain_framing = true;
        assert!(frame_tray_ceiling(&g, &kept, 0.0, &FramingDefaults::default()).is_empty());
    }

    #[test]
    fn a_tray_with_a_caution_is_not_framed() {
        let (mut g, rec) = geom(TrayCeiling::default());
        g.caution = Some(plan_core::tray::Caution::RoomNotFlat);
        assert!(frame_tray_ceiling(&g, &rec, 0.0, &FramingDefaults::default()).is_empty());
    }
}
