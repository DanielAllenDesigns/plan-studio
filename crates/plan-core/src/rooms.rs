//! Automatic room detection, the heart of Chief's "draw walls, get rooms".
//!
//! Walls are reduced to their centerlines, split at every intersection and
//! T-junction, snapped into a planar graph, and each bounded face of that
//! graph becomes a room. Faces are traced with the standard half-edge walk:
//! leaving a vertex, take the first edge clockwise from the one we arrived on,
//! which yields counter-clockwise (positive area) loops for interior faces and
//! one clockwise loop for the unbounded outside, which is discarded.

use crate::geometry::{
    polygon_area, polygon_centroid, project_on_segment, segment_intersection, Point,
};
use crate::model::Wall;
use crate::units::sq_in_to_sq_ft;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Room {
    /// Counter-clockwise centerline polygon, inches.
    pub polygon: Vec<Point>,
    pub area_sq_in: f64,
    pub centroid: Point,
    pub label: String,
}

impl Room {
    pub fn area_sq_ft(&self) -> f64 {
        sq_in_to_sq_ft(self.area_sq_in)
    }
}

/// Ignore faces smaller than this (slivers from near-coincident walls). 1 sq ft.
const MIN_ROOM_AREA_SQ_IN: f64 = 144.0;

pub fn detect_rooms(walls: &[Wall], tol: f64) -> Vec<Room> {
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
    let mut rooms = Vec::new();
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
                let idx = list
                    .iter()
                    .position(|&n| n == u)
                    .expect("edge must be in adjacency");
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
            if area > MIN_ROOM_AREA_SQ_IN {
                let centroid = polygon_centroid(&polygon);
                rooms.push(Room {
                    polygon,
                    area_sq_in: area,
                    centroid,
                    label: String::new(),
                });
            }
        }
    }

    // Stable, readable ordering: top-left rooms first.
    rooms.sort_by(|r, s| {
        (-r.centroid.y, r.centroid.x)
            .partial_cmp(&(-s.centroid.y, s.centroid.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (i, r) in rooms.iter_mut().enumerate() {
        r.label = format!("Room {}", i + 1);
    }
    rooms
}

fn node_index(nodes: &mut Vec<Point>, p: Point, tol: f64) -> usize {
    if let Some(i) = nodes.iter().position(|n| n.dist(p) <= tol) {
        return i;
    }
    nodes.push(p);
    nodes.len() - 1
}

/// Break every wall centerline at the points where other walls end on it
/// (T-junctions) or cross it, so the graph is planar.
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
    use crate::model::WallKind;

    fn wall(id: u64, x0: f64, y0: f64, x1: f64, y1: f64) -> Wall {
        Wall {
            id,
            start: Point::new(x0, y0),
            end: Point::new(x1, y1),
            thickness: 4.5,
            height: 109.125,
            kind: WallKind::Interior,
        }
    }

    #[test]
    fn four_walls_make_one_room() {
        // 20' x 10' box.
        let walls = vec![
            wall(1, 0.0, 0.0, 240.0, 0.0),
            wall(2, 240.0, 0.0, 240.0, 120.0),
            wall(3, 240.0, 120.0, 0.0, 120.0),
            wall(4, 0.0, 120.0, 0.0, 0.0),
        ];
        let rooms = detect_rooms(&walls, 0.5);
        assert_eq!(rooms.len(), 1);
        assert!((rooms[0].area_sq_ft() - 200.0).abs() < 1e-6);
    }

    #[test]
    fn t_junction_splits_into_two_rooms() {
        let walls = vec![
            wall(1, 0.0, 0.0, 240.0, 0.0),
            wall(2, 240.0, 0.0, 240.0, 120.0),
            wall(3, 240.0, 120.0, 0.0, 120.0),
            wall(4, 0.0, 120.0, 0.0, 0.0),
            // Partition ending on the top and bottom walls (two T-junctions).
            wall(5, 120.0, 0.0, 120.0, 120.0),
        ];
        let rooms = detect_rooms(&walls, 0.5);
        assert_eq!(rooms.len(), 2);
        for r in &rooms {
            assert!(
                (r.area_sq_ft() - 100.0).abs() < 1e-6,
                "got {}",
                r.area_sq_ft()
            );
        }
    }

    #[test]
    fn dangling_wall_makes_no_room() {
        let walls = vec![
            wall(1, 0.0, 0.0, 100.0, 0.0),
            wall(2, 100.0, 0.0, 100.0, 80.0),
        ];
        assert!(detect_rooms(&walls, 0.5).is_empty());
    }

    #[test]
    fn crossing_walls_make_four_rooms() {
        let walls = vec![
            wall(1, 0.0, 0.0, 240.0, 0.0),
            wall(2, 240.0, 0.0, 240.0, 240.0),
            wall(3, 240.0, 240.0, 0.0, 240.0),
            wall(4, 0.0, 240.0, 0.0, 0.0),
            wall(5, 120.0, 0.0, 120.0, 240.0),
            wall(6, 0.0, 120.0, 240.0, 120.0),
        ];
        let rooms = detect_rooms(&walls, 0.5);
        assert_eq!(rooms.len(), 4);
    }
}
