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
