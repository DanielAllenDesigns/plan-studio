//! Plan Footprint: the outer boundary of a floor's rooms.

use std::collections::HashMap;

use plan_core::geometry::polygon_area;
use plan_core::units::sq_in_to_sq_ft;
use plan_core::{Point, Project, Room};
use serde::Serialize;

/// Result of [`plan_footprint`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Footprint {
    /// Outer boundary, counter-clockwise, without collinear vertices.
    pub polygon: Vec<Point>,
    /// Enclosed area in square feet (courtyards are subtracted).
    pub area_sq_ft: f64,
    /// Length of the outer boundary in feet.
    pub perimeter_ft: f64,
}

type Key = (i64, i64);

fn key(p: Point) -> Key {
    ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64)
}

/// The outer boundary of the rooms of `floor`, to wall centerlines.
///
/// Uses the half-edge idea: room polygons run counter-clockwise, so an edge
/// shared by two rooms appears once in each direction and cancels, leaving
/// only edges that belong to one room. Those chain into the outline. An empty
/// footprint is returned when there are no rooms.
///
/// # Panics
/// Panics if `floor` is not a valid floor index.
pub fn plan_footprint(project: &Project, floor: usize, rooms: &[Room]) -> Footprint {
    let _ = &project.floors[floor];
    let mut edges: HashMap<(Key, Key), (Point, Point)> = HashMap::new();
    for r in rooms {
        let n = r.polygon.len();
        for i in 0..n {
            let (a, b) = (r.polygon[i], r.polygon[(i + 1) % n]);
            if key(a) != key(b) {
                edges.insert((key(a), key(b)), (a, b));
            }
        }
    }
    let outer: HashMap<(Key, Key), (Point, Point)> = edges
        .iter()
        .filter(|((a, b), _)| !edges.contains_key(&(*b, *a)))
        .map(|(k, v)| (*k, *v))
        .collect();

    // Chain outer edges into loops, walking from edge start to edge start.
    let mut next: HashMap<Key, Vec<Point>> = HashMap::new();
    let mut starts: Vec<Key> = Vec::new();
    for ((a, _), (_, b)) in &outer {
        next.entry(*a).or_default().push(*b);
        starts.push(*a);
    }
    starts.sort_unstable();
    starts.dedup();
    let origin: HashMap<Key, Point> = outer.iter().map(|((a, _), (pa, _))| (*a, *pa)).collect();

    let mut loops: Vec<Vec<Point>> = Vec::new();
    for s in starts {
        while let Some(first) = next.get_mut(&s).and_then(Vec::pop) {
            let mut pts = vec![origin[&s]];
            let mut cur = first;
            while key(cur) != s {
                pts.push(cur);
                match next.get_mut(&key(cur)).and_then(Vec::pop) {
                    Some(p) => cur = p,
                    None => break,
                }
            }
            loops.push(pts);
        }
    }

    let net_area: f64 = loops.iter().map(|l| polygon_area(l)).sum();
    let mut polygon = loops
        .into_iter()
        .max_by(|a, b| {
            polygon_area(a)
                .abs()
                .partial_cmp(&polygon_area(b).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(drop_collinear)
        .unwrap_or_default();
    if polygon_area(&polygon) < 0.0 {
        polygon.reverse();
    }
    let perimeter: f64 = (0..polygon.len())
        .map(|i| polygon[i].dist(polygon[(i + 1) % polygon.len()]))
        .sum();
    Footprint {
        polygon,
        area_sq_ft: sq_in_to_sq_ft(net_area.abs()),
        perimeter_ft: perimeter / 12.0,
    }
}

/// Remove vertices that lie on the straight line between their neighbours.
fn drop_collinear(pts: Vec<Point>) -> Vec<Point> {
    let n = pts.len();
    if n < 3 {
        return pts;
    }
    (0..n)
        .filter(|&i| {
            let (p, c, q) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            c.sub(p).cross(q.sub(c)).abs() > 1e-6 * c.sub(p).length().max(1.0)
        })
        .map(|i| pts[i])
        .collect()
}
