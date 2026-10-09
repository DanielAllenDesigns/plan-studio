//! Building footprint (outer boundary) from wall centerlines.
//!
//! The graph construction mirrors `plan_core::rooms`: walls are split at every
//! intersection and T-junction and snapped into a planar graph. Instead of the
//! bounded faces (rooms) we want the single unbounded face, which the half-edge
//! walk traces clockwise; reversing it gives the counter-clockwise outline.

use plan_core::geometry::{polygon_area, project_on_segment, segment_intersection};
use plan_core::{Point, Wall};
use std::collections::HashSet;

/// Outer boundary of the walls as a counter-clockwise polygon (inches), with
/// collinear intermediate vertices (T-junctions, end-to-end walls) removed.
///
/// Returns `None` when the walls enclose no area. If the walls form several
/// disconnected buildings the one with the largest outline is returned.
pub fn footprint_from_walls(walls: &[Wall], tol: f64) -> Option<Vec<Point>> {
    let tol = tol.max(1e-9);
    let segs = split_segments(walls, tol);

    let mut nodes: Vec<Point> = Vec::new();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (a, b) in segs {
        let ia = node_index(&mut nodes, a, tol);
        let ib = node_index(&mut nodes, b, tol);
        if ia == ib {
            continue;
        }
        let key = (ia.min(ib), ia.max(ib));
        if !edges.contains(&key) {
            edges.push(key);
        }
    }
    prune_dangling(nodes.len(), &mut edges);
    if edges.len() < 3 {
        return None;
    }

    // Adjacency sorted by outgoing angle, ascending (counter-clockwise order).
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for &(a, b) in &edges {
        adj[a].push(b);
        adj[b].push(a);
    }
    for (i, list) in adj.iter_mut().enumerate() {
        let o = nodes[i];
        list.sort_by(|&p, &q| {
            let ap = nodes[p].sub(o).angle();
            let aq = nodes[q].sub(o).angle();
            ap.partial_cmp(&aq).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let mut visited: HashSet<(usize, usize)> = HashSet::new();
    let mut outer: Option<(f64, Vec<Point>)> = None;
    let max_steps = edges.len() * 2 + 2;
    for &(a, b) in &edges {
        for (u0, v0) in [(a, b), (b, a)] {
            if visited.contains(&(u0, v0)) {
                continue;
            }
            let mut loop_nodes = Vec::new();
            let (mut u, mut v) = (u0, v0);
            let mut steps = 0;
            loop {
                visited.insert((u, v));
                loop_nodes.push(u);
                let list = &adj[v];
                let idx = list.iter().position(|&n| n == u)?;
                // First edge clockwise from the reversed incoming direction.
                let next = list[(idx + list.len() - 1) % list.len()];
                u = v;
                v = next;
                steps += 1;
                if (u, v) == (u0, v0) || steps > max_steps {
                    break;
                }
            }
            let polygon: Vec<Point> = loop_nodes.iter().map(|&i| nodes[i]).collect();
            let area = polygon_area(&polygon);
            // The unbounded face is the clockwise (negative) one.
            if area < 0.0 && outer.as_ref().is_none_or(|(best, _)| area < *best) {
                outer = Some((area, polygon));
            }
        }
    }

    let (_, mut poly) = outer?;
    poly.reverse();
    let poly = drop_collinear(poly);
    (poly.len() >= 3 && polygon_area(&poly) > 0.0).then_some(poly)
}

/// The walls with every curved wall replaced by the straight sections an
/// automatic roof is built over (manual p. 826; RF-66, RF-67).
///
/// A curved wall becomes sections of at most `segment_angle` degrees of arc
/// (6 to 90; the lower the angle, the more sections), each a straight wall
/// with the curved wall's thickness, height and roof directive. A *concave*
/// curved wall, one that bows into the building, gets sections only when their
/// baselines are longer than `min_alcove` inches (Minimum Alcove Size); a
/// smaller alcove is spanned by a straight baseline between its ends instead,
/// as if the wall were straight. Straight walls pass through unchanged.
pub fn flatten_curved_walls(walls: &[Wall], segment_angle: f64, min_alcove: f64) -> Vec<Wall> {
    if walls
        .iter()
        .all(|w| w.curve.is_none_or(|c| c.is_straight()))
    {
        return walls.to_vec();
    }
    let step = crate::switches::clamp_segment_angle(segment_angle);
    // The outline over the chords tells which side of a curved wall is inside.
    let chords: Vec<Wall> = walls
        .iter()
        .map(|w| {
            let mut c = w.clone();
            c.curve = None;
            c
        })
        .collect();
    let outline = footprint_from_walls(&chords, 0.5);
    let mut out = Vec::new();
    for w in walls {
        let Some(curve) = w.curve.filter(|c| !c.is_straight()) else {
            out.push(w.clone());
            continue;
        };
        let sweep = curve.sweep(w.start, w.end).abs().to_degrees();
        let n = ((sweep / step - 1e-9).ceil() as usize).max(1);
        let pts = curve.sample_points(w.start, w.end, n);
        let concave = outline
            .as_ref()
            .is_some_and(|fp| bows_inward(w, curve.bulge, fp));
        let section = pts[0].dist(pts[1]);
        if concave && section <= min_alcove {
            let mut straight = w.clone();
            straight.curve = None;
            out.push(straight);
            continue;
        }
        for pair in pts.windows(2) {
            let mut seg = w.clone();
            seg.start = pair[0];
            seg.end = pair[1];
            seg.curve = None;
            out.push(seg);
        }
    }
    out
}

/// Does the curved wall `w` (bulge `bulge`) bow toward the inside of the
/// counter-clockwise outline `fp`? A wall whose chord is not on the outline
/// counts as convex.
fn bows_inward(w: &Wall, bulge: f64, fp: &[Point]) -> bool {
    let mid = Point::lerp(w.start, w.end, 0.5);
    let n = fp.len();
    let tol = w.thickness.max(1.0);
    let Some(k) = (0..n).find(|&k| {
        plan_core::geometry::dist_to_segment(mid, fp[k], fp[(k + 1) % n]) <= tol
            && w.direction()
                .cross(fp[(k + 1) % n].sub(fp[k]).normalized())
                .abs()
                < 0.2
    }) else {
        return false;
    };
    let along = fp[(k + 1) % n].sub(fp[k]);
    // The inside of a counter-clockwise outline is left of its edges.
    let aligned = w.direction().dot(along) > 0.0;
    (aligned && bulge > 0.0) || (!aligned && bulge < 0.0)
}

/// Repeatedly delete edges that end at a degree-1 node (free-standing walls).
fn prune_dangling(node_count: usize, edges: &mut Vec<(usize, usize)>) {
    loop {
        let mut degree = vec![0usize; node_count];
        for &(a, b) in edges.iter() {
            degree[a] += 1;
            degree[b] += 1;
        }
        let before = edges.len();
        edges.retain(|&(a, b)| degree[a] > 1 && degree[b] > 1);
        if edges.len() == before {
            return;
        }
    }
}

/// Remove vertices lying on the straight line between their neighbours.
fn drop_collinear(mut pts: Vec<Point>) -> Vec<Point> {
    loop {
        let n = pts.len();
        if n < 4 {
            return pts;
        }
        let straight = (0..n).find(|&k| {
            let (p, c, q) = (pts[(k + n - 1) % n], pts[k], pts[(k + 1) % n]);
            let (a, b) = (c.sub(p), q.sub(c));
            let scale = a.length() * b.length();
            scale <= f64::EPSILON || (a.cross(b).abs() <= 1e-9 * scale && a.dot(b) > 0.0)
        });
        match straight {
            Some(k) => {
                pts.remove(k);
            }
            None => return pts,
        }
    }
}

fn node_index(nodes: &mut Vec<Point>, p: Point, tol: f64) -> usize {
    if let Some(i) = nodes.iter().position(|n| n.dist(p) <= tol) {
        return i;
    }
    nodes.push(p);
    nodes.len() - 1
}

/// Break every wall centerline where other walls end on it or cross it.
fn split_segments(walls: &[Wall], tol: f64) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    for (i, w) in walls.iter().enumerate() {
        let (a, b) = (w.start, w.end);
        let len = a.dist(b);
        if len < tol {
            continue;
        }
        let mut ts = vec![0.0, 1.0];
        for (j, o) in walls.iter().enumerate() {
            if i == j {
                continue;
            }
            for p in [o.start, o.end] {
                let (t, q) = project_on_segment(p, a, b);
                if q.dist(p) <= tol && t > 0.0 && t < 1.0 {
                    ts.push(t);
                }
            }
            if let Some((t, u)) = segment_intersection(a, b, o.start, o.end) {
                if t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0 {
                    ts.push(t);
                }
            }
        }
        ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        ts.dedup_by(|x, y| (*x - *y).abs() * len < tol);
        for k in 0..ts.len() - 1 {
            let p = Point::lerp(a, b, ts[k]);
            let q = Point::lerp(a, b, ts[k + 1]);
            if p.dist(q) > tol {
                out.push((p, q));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{WallCurve, WallKind};

    /// A 40 x 24 ft house whose south wall (0,0 -> 480,0) bows by `bulge`
    /// (negative bows outward).
    fn house(bulge: f64) -> Vec<Wall> {
        let c = [
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 288.0),
            Point::new(0.0, 288.0),
        ];
        (0..4)
            .map(|i| {
                let mut w = Wall::new(c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
                if i == 0 {
                    w.curve = Some(WallCurve { bulge });
                }
                w
            })
            .collect()
    }

    #[test]
    fn a_convex_curved_wall_is_cut_at_the_segment_angle() {
        // Bulge 60 over a 480 chord sweeps 56.1 degrees.
        let at = |angle: f64| flatten_curved_walls(&house(-60.0), angle, 24.0).len() - 3;
        assert_eq!(at(30.0), 2);
        assert_eq!(at(15.0), 4);
        assert_eq!(at(6.0), 10);
        // Out of range angles are limited to 6 .. 90 degrees.
        assert_eq!(at(1.0), 10);
        assert_eq!(at(180.0), 1);
        // The sections form a footprint that bows past the chord.
        let flat = flatten_curved_walls(&house(-60.0), 15.0, 24.0);
        let fp = footprint_from_walls(&flat, 0.5).unwrap();
        assert!(fp.len() > 4);
        assert!(
            fp.iter().any(|p| p.y < -50.0),
            "the outline reaches the bow"
        );
        assert!(polygon_area(&fp) > 480.0 * 288.0);
    }

    #[test]
    fn straight_walls_pass_through_unchanged() {
        let mut walls = house(0.0);
        walls[0].curve = None;
        let out = flatten_curved_walls(&walls, 15.0, 24.0);
        assert_eq!(out.len(), walls.len());
        for (a, b) in out.iter().zip(&walls) {
            assert_eq!((a.start, a.end), (b.start, b.end));
        }
    }

    #[test]
    fn a_small_concave_alcove_is_spanned_by_a_straight_baseline() {
        // The south wall bows into the house: a concave wall.
        let big = flatten_curved_walls(&house(60.0), 15.0, 24.0);
        assert_eq!(big.len(), 3 + 4, "sections of 120\" are longer than 24\"");
        let fp = footprint_from_walls(&big, 0.5).unwrap();
        assert!(
            polygon_area(&fp) < 480.0 * 288.0,
            "the alcove cuts into the house"
        );
        // With a Minimum Alcove Size longer than the sections the wall is a
        // straight baseline between its ends.
        let simple = flatten_curved_walls(&house(60.0), 15.0, 200.0);
        assert_eq!(simple.len(), 4);
        assert!(simple.iter().all(|w| w.curve.is_none()));
        let fp = footprint_from_walls(&simple, 0.5).unwrap();
        assert!((polygon_area(&fp) - 480.0 * 288.0).abs() < 1e-6);
        // A convex wall is never simplified by the alcove size.
        assert_eq!(
            flatten_curved_walls(&house(-60.0), 15.0, 200.0).len(),
            3 + 4
        );
    }
}
