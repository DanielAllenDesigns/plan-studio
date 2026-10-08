//! Layout markers and the generators they steer: Joist Direction, Roof Truss
//! Direction, Bearing Line, Framing Reference Marker and Truss Base.
//!
//! A direction line gives the **spacing axis**: members are spaced along it
//! and run perpendicular to it. A reference marker anchors the layout grid
//! (the first stud, joist or truss sits on the marker); a bearing line splits
//! joists where it crosses them and adds a beam.

use crate::defaults::FramingDefaults;
use crate::manual::{FramingMember, LumberSize, MemberKind, SUBFLOOR};
use crate::member::Member;
use crate::truss::TrussSpec;
use crate::wall::frame_wall_grid;
use plan_core::{Id, Opening, Point, Room, Wall};
use serde::{Deserialize, Serialize};

const EPS: f64 = 1e-6;
/// Shortest member kept, inches.
const MIN_LEN: f64 = 1.0;

fn line_angle_deg(line: (Point, Point)) -> f64 {
    (line.1 - line.0).angle().to_degrees()
}

fn unit_deg(deg: f64) -> Point {
    let r = deg.to_radians();
    Point::new(r.cos(), r.sin())
}

macro_rules! direction_marker {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
        pub struct $name {
            /// The line as drawn.
            pub line: (Point, Point),
            /// On-centre spacing along the line, inches. `0.0` uses the
            /// default spacing.
            pub spacing: f64,
            /// Direction of the spacing axis in degrees (counter-clockwise
            /// from plan +X). Authoritative: rotating the marker edits this.
            pub angle: f64,
        }

        impl $name {
            /// A marker whose `angle` follows `line`.
            pub fn new(line: (Point, Point), spacing: f64) -> Self {
                Self {
                    line,
                    spacing,
                    angle: line_angle_deg(line),
                }
            }

            /// Unit vector members are spaced along.
            pub fn axis(&self) -> Point {
                unit_deg(self.angle)
            }

            /// Unit vector members run along: perpendicular to the line.
            pub fn run(&self) -> Point {
                self.axis().perp()
            }
        }
    };
}

direction_marker!(
    /// Joist Direction: joists run perpendicular to the line.
    JoistDirection
);
direction_marker!(
    /// Roof Truss Direction: trusses span perpendicular to the line and are
    /// spaced along it.
    RoofTrussDirection
);

/// A bearing line: joists bear on it, so it splits their spans and carries a
/// beam.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BearingLine {
    pub line: (Point, Point),
}

/// A Framing Reference Marker. Layout grids are measured from `point`;
/// `angle` (degrees) orients the marker symbol and does not change the grid,
/// which always follows the wall or the direction line it applies to.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReferenceMarker {
    pub point: Point,
    pub angle: f64,
}

/// Truss Base: the closed polyline that bounds where roof trusses are laid
/// out (usually the bearing outline), at a bearing `elevation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrussBase {
    pub points: Vec<Point>,
    pub elevation: f64,
}

impl TrussBase {
    pub fn new(points: Vec<Point>, elevation: f64) -> Self {
        Self { points, elevation }
    }

    /// Enclosed area, square inches.
    pub fn area(&self) -> f64 {
        let n = self.points.len();
        ((0..n)
            .map(|i| self.points[i].cross(self.points[(i + 1) % n]))
            .sum::<f64>()
            / 2.0)
            .abs()
    }
}

/// Frame a wall with the stud grid anchored at a Framing Reference Marker.
///
/// The marker's position along the wall (its projection on the wall
/// direction) is where a stud's near edge sits; studs repeat at the spacing
/// both ways, with the wall's first and last studs kept. `None` frames like
/// [`crate::frame_wall`].
pub fn frame_wall_with_marker(
    wall: &Wall,
    openings: &[&Opening],
    floor_elevation: f64,
    d: &FramingDefaults,
    marker: Option<&ReferenceMarker>,
) -> Vec<Member> {
    let origin = marker.map(|m| (m.point - wall.start).dot(wall.direction()));
    frame_wall_grid(wall, openings, floor_elevation, d, origin)
}

/// [`frame_wall_with_marker`] that also knows how the wall meets its
/// neighbours (see [`crate::frame_wall_joined`]), for the corner and tee studs
/// and the wall blocking.
pub fn frame_wall_with_marker_joined(
    wall: &Wall,
    openings: &[&Opening],
    floor_elevation: f64,
    d: &FramingDefaults,
    marker: Option<&ReferenceMarker>,
    joints: &crate::WallJoints,
) -> Vec<Member> {
    let origin = marker.map(|m| (m.point - wall.start).dot(wall.direction()));
    crate::wall::frame_wall_joined(wall, openings, floor_elevation, d, origin, joints)
}

/// One run line in the local frame (`u` along the run, `v` across).
struct Run {
    across: f64,
    spans: Vec<(f64, f64)>,
}

/// Lay out run lines over `poly`. `run` is the unit run direction; lines are
/// spaced along `run.perp()` rotated back (the spacing axis). `grid_origin`
/// is a position along the spacing axis to anchor the grid on.
fn runs(
    poly: &[Point],
    axis: Point,
    spacing: f64,
    t: f64,
    grid_origin: Option<f64>,
) -> (Vec<Run>, Point) {
    let run = axis.perp();
    let local: Vec<(f64, f64)> = poly.iter().map(|p| (p.dot(run), p.dot(axis))).collect();
    let (lo, hi) = local
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &(_, v)| {
            (lo.min(v), hi.max(v))
        });
    let step = spacing.max(t);
    let positions = positions(lo, hi, step, t, grid_origin);
    let runs = positions
        .into_iter()
        .map(|across| Run {
            across,
            spans: clip(&local, across)
                .into_iter()
                .filter(|&(a, b)| b - a > MIN_LEN)
                .collect(),
        })
        .collect();
    (runs, run)
}

/// Member centre positions across `[lo, hi]`. Flush at both edges and on
/// `step` between; with an `origin`, on the grid `origin + k*step` instead,
/// plus edge members where the grid leaves a gap wider than `t`.
fn positions(lo: f64, hi: f64, step: f64, t: f64, origin: Option<f64>) -> Vec<f64> {
    if hi - lo < t {
        return Vec::new();
    }
    let (first, last) = (lo + t / 2.0, hi - t / 2.0);
    match origin {
        None => {
            let mut v: Vec<f64> = (0..)
                .map(|k| first + f64::from(k) * step)
                .take_while(|&p| p <= last - t + EPS)
                .collect();
            if last - first > EPS {
                v.push(last);
            }
            v
        }
        Some(o) => {
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
            v
        }
    }
}

/// Even-odd `(entry, exit)` intervals along `u` where the line `v = across`
/// lies inside the polygon.
fn clip(poly: &[(f64, f64)], across: f64) -> Vec<(f64, f64)> {
    let n = poly.len();
    let mut hits: Vec<f64> = (0..n)
        .filter_map(|i| {
            let (s0, c0) = poly[i];
            let (s1, c1) = poly[(i + 1) % n];
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

fn dim_size(l: crate::lumber::Lumber) -> LumberSize {
    LumberSize::dim(l.nominal_thickness(), l.nominal_depth())
}

fn bounds_wh(poly: &[Point]) -> (f64, f64) {
    let (mut lo, mut hi) = (
        Point::new(f64::INFINITY, f64::INFINITY),
        Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
    );
    for p in poly {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    (hi.x - lo.x, hi.y - lo.y)
}

/// Auto-frame a floor into [`FramingMember`]s, steered by markers.
///
/// * Without a direction, joists span the shorter bounding dimension (as
///   [`crate::frame_floor`] `Auto`). A [`JoistDirection`] makes joists run
///   perpendicular to its line and sets the spacing (when non-zero).
/// * A [`ReferenceMarker`] anchors the joist grid on the marker.
/// * Each [`BearingLine`] crossing the joists splits them, leaving half the
///   beam width each side, and adds one 4x10 `FloorCeilingBeam` along the
///   line over the joists it crosses. A bearing line parallel to the joists
///   does nothing.
///
/// Joists are clipped to the room polygon with no rim joist. Their tops sit
/// 3/4" below `floor_elevation`. Member ids count up from `first_id`.
pub fn frame_floor_directed(
    room: &Room,
    floor_elevation: f64,
    d: &FramingDefaults,
    direction: Option<&JoistDirection>,
    bearing: &[BearingLine],
    marker: Option<&ReferenceMarker>,
    first_id: Id,
) -> Vec<FramingMember> {
    let poly = &room.polygon;
    if poly.len() < 3 {
        return Vec::new();
    }
    let (axis, spacing) = match direction {
        Some(dir) => (
            dir.axis(),
            if dir.spacing > 0.0 {
                dir.spacing
            } else {
                d.joist_spacing
            },
        ),
        None => {
            let (w, h) = bounds_wh(poly);
            let run = if w <= h {
                Point::new(1.0, 0.0)
            } else {
                Point::new(0.0, 1.0)
            };
            (Point::new(run.y, -run.x), d.joist_spacing)
        }
    };
    let joist_size = dim_size(d.joist_size);
    let t = joist_size.width();
    let origin = marker.map(|m| m.point.dot(axis));
    let (lines, run) = runs(poly, axis, spacing, t, origin);
    let to_plan = |s: f64, c: f64| run * s + axis * c;
    let joist_z = floor_elevation - SUBFLOOR - joist_size.depth();

    let beam_proto = FramingMember::new(0, MemberKind::FloorCeilingBeam, Point::ZERO, Point::ZERO);
    let half_gap = beam_proto.width / 2.0;
    // Bearing lines in the local frame.
    let local_bearing: Vec<((f64, f64), (f64, f64))> = bearing
        .iter()
        .map(|b| {
            (
                (b.line.0.dot(run), b.line.0.dot(axis)),
                (b.line.1.dot(run), b.line.1.dot(axis)),
            )
        })
        .collect();
    // Across range of joists each bearing line splits.
    let mut crossed: Vec<Option<(f64, f64)>> = vec![None; bearing.len()];

    let mut out = Vec::new();
    let mut next = first_id;
    for line in &lines {
        for &(a, b) in &line.spans {
            let mut cuts: Vec<f64> = Vec::new();
            for (i, &((u0, v0), (u1, v1))) in local_bearing.iter().enumerate() {
                if (v1 - v0).abs() < EPS
                    || (line.across < v0.min(v1) - EPS)
                    || (line.across > v0.max(v1) + EPS)
                {
                    continue;
                }
                let s = u0 + (line.across - v0) / (v1 - v0) * (u1 - u0);
                if s > a + MIN_LEN && s < b - MIN_LEN {
                    cuts.push(s);
                    crossed[i] = Some(match crossed[i] {
                        Some((lo, hi)) => (lo.min(line.across), hi.max(line.across)),
                        None => (line.across, line.across),
                    });
                }
            }
            cuts.sort_by(f64::total_cmp);
            let mut from = a;
            let mut pieces = Vec::new();
            for &s in &cuts {
                pieces.push((from, s - half_gap));
                from = s + half_gap;
            }
            pieces.push((from, b));
            for (p, q) in pieces.into_iter().filter(|&(p, q)| q - p > MIN_LEN) {
                let mut m = FramingMember::new(
                    next,
                    MemberKind::Joist,
                    to_plan(p, line.across),
                    to_plan(q, line.across),
                )
                .with_lumber(joist_size)
                .at_elevation(joist_z);
                m.label = format!("{} joist", joist_size.name());
                out.push(m);
                next += 1;
            }
        }
    }

    for (i, range) in crossed.iter().enumerate() {
        let Some((lo, hi)) = *range else { continue };
        let ((u0, v0), (u1, v1)) = local_bearing[i];
        let at = |c: f64| {
            let tau = ((c - v0) / (v1 - v0)).clamp(0.0, 1.0);
            to_plan(u0 + tau * (u1 - u0), v0 + tau * (v1 - v0))
        };
        let beam = FramingMember::new(
            next,
            MemberKind::FloorCeilingBeam,
            at(lo - t / 2.0),
            at(hi + t / 2.0),
        );
        let z = floor_elevation - SUBFLOOR - beam.depth;
        out.push(beam.at_elevation(z));
        next += 1;
    }
    out
}

/// Lay roof trusses over a [`TrussBase`]: one `RoofTruss` per run, spanning
/// the base perpendicular to the direction line and spaced along it (the
/// direction's spacing, or 24" when zero), flush at both ends. A
/// [`ReferenceMarker`] anchors the grid. Each truss takes `spec` with its span
/// set to the length of its run, and sits at the base elevation.
pub fn layout_trusses(
    base: &TrussBase,
    direction: &RoofTrussDirection,
    spec: &TrussSpec,
    marker: Option<&ReferenceMarker>,
    first_id: Id,
) -> Vec<FramingMember> {
    if base.points.len() < 3 {
        return Vec::new();
    }
    let axis = direction.axis();
    let spacing = if direction.spacing > 0.0 {
        direction.spacing
    } else {
        MemberKind::RoofTruss.default_spacing()
    };
    let origin = marker.map(|m| m.point.dot(axis));
    let (lines, run) = runs(&base.points, axis, spacing, spec.thickness(), origin);
    let mut out = Vec::new();
    let mut next = first_id;
    for line in &lines {
        for &(a, b) in &line.spans {
            let mut s = spec.clone();
            s.span = b - a;
            let mut m = FramingMember::new(
                next,
                MemberKind::RoofTruss,
                run * a + axis * line.across,
                run * b + axis * line.across,
            )
            .at_elevation(base.elevation);
            m.plies = s.plies.max(1);
            m.width = s.thickness();
            m.truss = Some(s);
            out.push(m);
            next += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manual::MemberKind as K;
    use crate::{frame_wall, TrussType};
    use plan_core::geometry::polygon_area;
    use plan_core::WallKind;

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

    /// 20' x 12'.
    fn rect() -> Room {
        room(&[(0.0, 0.0), (240.0, 0.0), (240.0, 144.0), (0.0, 144.0)])
    }

    fn joists(m: &[FramingMember]) -> Vec<&FramingMember> {
        m.iter().filter(|x| x.kind == K::Joist).collect()
    }

    fn run_of(m: &FramingMember) -> Point {
        (m.end - m.start).normalized()
    }

    #[test]
    fn auto_joists_span_the_short_side() {
        let m = frame_floor_directed(
            &rect(),
            96.0,
            &FramingDefaults::default(),
            None,
            &[],
            None,
            1,
        );
        let j = joists(&m);
        assert_eq!(j.len(), 16);
        for x in &j {
            assert!((x.plan_length() - 144.0).abs() < 1e-9);
            assert!(run_of(x).x.abs() < 1e-9 && run_of(x).y.abs() > 0.99);
            assert_eq!(x.lumber.name(), "2x10");
            // Top 3/4" below the finished floor.
            assert!((x.elevation_bottom + x.depth - (96.0 - 0.75)).abs() < 1e-9);
        }
        assert_eq!(j[0].id, 1);
        assert_eq!(j.last().unwrap().id, 16);
    }

    #[test]
    fn joist_direction_rotates_auto_joists_by_ninety_degrees() {
        let d = FramingDefaults::default();
        let auto = frame_floor_directed(&rect(), 0.0, &d, None, &[], None, 1);
        // A vertical direction line: joists are spaced along Y, run along X.
        let dir = JoistDirection::new((Point::new(0.0, 0.0), Point::new(0.0, 50.0)), 0.0);
        assert!((dir.angle - 90.0).abs() < 1e-9);
        let turned = frame_floor_directed(&rect(), 0.0, &d, Some(&dir), &[], None, 1);
        let (a, b) = (run_of(joists(&auto)[0]), run_of(joists(&turned)[0]));
        assert!(a.dot(b).abs() < 1e-9, "perpendicular: {a:?} {b:?}");
        assert!(b.x.abs() > 0.99);
        for x in joists(&turned) {
            assert!((x.plan_length() - 240.0).abs() < 1e-6);
        }
        // 12' deep room spaced at 16": flush ends plus every 16".
        assert_eq!(joists(&turned).len(), 10);
        // The spacing in the marker overrides the default.
        let wide = JoistDirection::new(dir.line, 24.0);
        let w = frame_floor_directed(&rect(), 0.0, &d, Some(&wide), &[], None, 1);
        assert_eq!(joists(&w).len(), 7);
    }

    #[test]
    fn bearing_line_at_midspan_halves_joists_and_adds_a_beam() {
        let d = FramingDefaults::default();
        let dir = JoistDirection::new((Point::new(0.0, 0.0), Point::new(0.0, 50.0)), 0.0);
        let plain = frame_floor_directed(&rect(), 0.0, &d, Some(&dir), &[], None, 1);
        let bl = BearingLine {
            line: (Point::new(120.0, -10.0), Point::new(120.0, 160.0)),
        };
        let split = frame_floor_directed(&rect(), 0.0, &d, Some(&dir), &[bl], None, 1);
        let (p, s) = (joists(&plain), joists(&split));
        assert_eq!(s.len(), 2 * p.len());
        let beam_w = 3.5;
        for j in &s {
            assert!(
                (j.plan_length() - 240.0 / 2.0).abs() <= beam_w,
                "{}",
                j.plan_length()
            );
        }
        let beams: Vec<_> = split
            .iter()
            .filter(|m| m.kind == K::FloorCeilingBeam)
            .collect();
        assert_eq!(beams.len(), 1);
        let b = beams[0];
        assert!((b.start.x - 120.0).abs() < 1e-9 && (b.end.x - 120.0).abs() < 1e-9);
        // Beam covers the first to last joist across the room.
        assert!(
            (b.plan_length() - 144.0).abs() < 1e-6,
            "{}",
            b.plan_length()
        );
        assert_eq!(b.lumber.name(), "4x10");
        // A bearing line parallel to the joists changes nothing.
        let par = BearingLine {
            line: (Point::new(-10.0, 60.0), Point::new(250.0, 60.0)),
        };
        let same = frame_floor_directed(&rect(), 0.0, &d, Some(&dir), &[par], None, 1);
        assert_eq!(same.len(), plain.len());
    }

    #[test]
    fn reference_marker_anchors_the_joist_grid() {
        let d = FramingDefaults::default();
        let marker = ReferenceMarker {
            point: Point::new(100.0, 0.0),
            angle: 0.0,
        };
        let m = frame_floor_directed(&rect(), 0.0, &d, None, &[], Some(&marker), 1);
        let xs: Vec<f64> = joists(&m).iter().map(|j| j.start.x).collect();
        assert!(xs.iter().any(|&x| (x - 100.0).abs() < 1e-9), "{xs:?}");
        assert!(xs.iter().any(|&x| (x - 84.0).abs() < 1e-9));
        // Edge joists close the gaps the grid leaves.
        assert!(xs.iter().any(|&x| (x - 0.75).abs() < 1e-9));
        assert!(xs.iter().any(|&x| (x - 239.25).abs() < 1e-9));
    }

    fn wall() -> Wall {
        Wall {
            id: 1,
            start: Point::new(0.0, 0.0),
            end: Point::new(120.0, 0.0),
            thickness: 6.5,
            height: 109.125,
            kind: WallKind::Exterior,
            layer: "Walls, Normal".into(),
            ..Default::default()
        }
    }

    fn stud_lefts(m: &[Member]) -> Vec<f64> {
        let mut v: Vec<f64> = m
            .iter()
            .filter(|x| x.kind == crate::MemberKind::Stud)
            .map(|x| x.transform.origin[0] - x.lumber.thickness / 2.0)
            .collect();
        v.sort_by(f64::total_cmp);
        v
    }

    #[test]
    fn reference_marker_shifts_the_first_stud() {
        let d = FramingDefaults::default();
        let w = wall();
        let plain = stud_lefts(&frame_wall(&w, &[], 0.0, &d));
        assert!((plain[1] - 16.0).abs() < 1e-9);
        let same = frame_wall_with_marker(&w, &[], 0.0, &d, None);
        assert_eq!(stud_lefts(&same), plain);
        let marker = ReferenceMarker {
            point: Point::new(10.0, 0.0),
            angle: 0.0,
        };
        let shifted = stud_lefts(&frame_wall_with_marker(&w, &[], 0.0, &d, Some(&marker)));
        // Corner stud, then the grid starting on the marker.
        assert!((shifted[0]).abs() < 1e-9);
        assert!((shifted[1] - 10.0).abs() < 1e-9, "{shifted:?}");
        assert!((shifted[2] - 26.0).abs() < 1e-9);
        assert!(!shifted.iter().any(|&l| (l - 16.0).abs() < 1e-9));
        // Grid also continues back toward the start.
        let marker2 = ReferenceMarker {
            point: Point::new(40.0, 0.0),
            angle: 0.0,
        };
        let s2 = stud_lefts(&frame_wall_with_marker(&w, &[], 0.0, &d, Some(&marker2)));
        assert!(s2.iter().any(|&l| (l - 8.0).abs() < 1e-9), "{s2:?}");
        assert!((s2.last().unwrap() - 118.5).abs() < 1e-9);
        // A marker on the wall start leaves the default layout alone.
        let at0 = ReferenceMarker {
            point: Point::ZERO,
            angle: 0.0,
        };
        assert_eq!(
            stud_lefts(&frame_wall_with_marker(&w, &[], 0.0, &d, Some(&at0))),
            plain
        );
    }

    #[test]
    fn trusses_are_spaced_along_the_direction_line_and_span_the_base() {
        let base = TrussBase::new(
            vec![
                Point::new(0.0, 0.0),
                Point::new(288.0, 0.0),
                Point::new(288.0, 240.0),
                Point::new(0.0, 240.0),
            ],
            96.0,
        );
        assert!((base.area() - 288.0 * 240.0).abs() < 1e-6);
        // Direction line along Y: trusses spaced along Y, spanning X.
        let dir = RoofTrussDirection::new((Point::new(0.0, 0.0), Point::new(0.0, 100.0)), 24.0);
        let spec = TrussSpec::new(TrussType::Fink, 100.0, 6.0);
        let t = layout_trusses(&base, &dir, &spec, None, 10);
        // 240" deep, 1.5" trusses on 24": 0.75, 24.75, ... plus the far end.
        assert_eq!(t.len(), 11);
        assert_eq!(t[0].id, 10);
        for m in &t {
            assert_eq!(m.kind, K::RoofTruss);
            assert!((m.plan_length() - 288.0).abs() < 1e-6);
            assert!((m.elevation_bottom - 96.0).abs() < 1e-9);
            assert!((m.truss.as_ref().unwrap().span - 288.0).abs() < 1e-6);
            assert_eq!(m.to_boxes().len(), 9);
        }
        assert!((t[1].start.y - t[0].start.y - 24.0).abs() < 1e-9);
        // Anchor the grid on a marker.
        let marker = ReferenceMarker {
            point: Point::new(0.0, 100.0),
            angle: 0.0,
        };
        let a = layout_trusses(&base, &dir, &spec, Some(&marker), 1);
        assert!(a.iter().any(|m| (m.start.y - 100.0).abs() < 1e-9));
        assert!(layout_trusses(&TrussBase::new(vec![], 0.0), &dir, &spec, None, 1).is_empty());
    }
}
