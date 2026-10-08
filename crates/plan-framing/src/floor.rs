//! Floor platform framing: joists, rim joists and mid-span blocking.

use crate::defaults::FramingDefaults;
use crate::lumber::TWO_BY_THICKNESS;
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
    let rim = if d.rim_joist { TWO_BY_THICKNESS } else { 0.0 };

    let (lo, hi) = if along_x {
        (min.y, max.y)
    } else {
        (min.x, max.x)
    };
    let lines: Vec<Line> = joist_positions(lo, hi, d.joist_spacing.max(t), t)
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
            let start = p + inward * (t / 2.0);
            let dir = e.normalized();
            let tf = Transform3 {
                origin: [start.x, y_mid, -start.y],
                axis_x: [dir.x, 0.0, -dir.y],
                axis_y: up,
            };
            out.push(Member::new(
                MemberKind::RimJoist,
                lumber,
                e.length(),
                tf,
                None,
            ));
        }
    }

    if d.blocking {
        for pair in lines.windows(2) {
            let (l0, l1) = (&pair[0], &pair[1]);
            let gap = l1.across - l0.across - t;
            if gap < 1.0 {
                continue;
            }
            for (&(a0, b0), &(a1, b1)) in l0.spans.iter().zip(&l1.spans) {
                let (a, b) = (a0.max(a1), b0.min(b1));
                let span = b - a;
                if span <= MAX_UNBLOCKED_SPAN {
                    continue;
                }
                let rows = (span / MAX_UNBLOCKED_SPAN).ceil() as u32 - 1;
                for r in 1..=rows {
                    let at = a + span * f64::from(r) / f64::from(rows + 1);
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
fn joist_positions(lo: f64, hi: f64, step: f64, t: f64) -> Vec<f64> {
    if hi - lo < t {
        return Vec::new();
    }
    let (first, last) = (lo + t / 2.0, hi - t / 2.0);
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
}
