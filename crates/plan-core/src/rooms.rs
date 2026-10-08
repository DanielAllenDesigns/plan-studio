//! Automatic room detection, the heart of Chief's "draw walls, get rooms".
//!
//! Walls are reduced to their centerlines, split at every intersection and
//! T-junction, snapped into a planar graph, and each bounded face of that
//! graph becomes a room. Faces are traced with the standard half-edge walk:
//! leaving a vertex, take the first edge clockwise from the one we arrived on,
//! which yields counter-clockwise (positive area) loops for interior faces and
//! one clockwise loop for the unbounded outside, which is discarded.

use crate::defaults::RoomTypeDef;
use crate::geometry::{
    dist_to_segment, point_in_polygon, polygon_area, polygon_centroid, project_on_segment,
    segment_intersection, Point,
};
use crate::model::{Project, Wall, WallKind};
use crate::units::sq_in_to_sq_ft;
use std::collections::HashSet;

/// A detected room (R-1..R-18).
///
/// Three areas are carried:
/// * `area_sq_in`: area of the **centerline** polygon (`polygon`); kept for
///   compatibility with older callers.
/// * `interior_area_sq_in`: area of the **interior-surface** polygon
///   (`inner_polygon`), Chief's Interior Area (R-2, R-49).
/// * `standard_area_sq_in`: Chief's Standard Area (R-49): to the outside of
///   exterior walls and the centre of interior (shared) walls.
#[derive(Debug, Clone, Default)]
pub struct Room {
    /// Counter-clockwise centerline polygon, inches.
    pub polygon: Vec<Point>,
    /// Centerline area, square inches (see the type docs).
    pub area_sq_in: f64,
    pub centroid: Point,
    pub label: String,
    /// Counter-clockwise polygon along the interior wall surfaces (R-2).
    pub inner_polygon: Vec<Point>,
    /// Area of `inner_polygon`, square inches.
    pub interior_area_sq_in: f64,
    /// Standard Area, square inches (R-49).
    pub standard_area_sq_in: f64,
}

impl Room {
    /// Centerline area in square feet.
    pub fn area_sq_ft(&self) -> f64 {
        sq_in_to_sq_ft(self.area_sq_in)
    }
    /// The centerline polygon (same as the `polygon` field).
    pub fn centerline_polygon(&self) -> &[Point] {
        &self.polygon
    }
    /// Interior Area in square feet (R-49).
    pub fn interior_area_sq_ft(&self) -> f64 {
        sq_in_to_sq_ft(self.interior_area_sq_in)
    }
    /// Alias of [`Room::interior_area_sq_ft`].
    pub fn interior_area(&self) -> f64 {
        self.interior_area_sq_ft()
    }
    /// Standard Area in square feet (R-49).
    pub fn standard_area_sq_ft(&self) -> f64 {
        sq_in_to_sq_ft(self.standard_area_sq_in)
    }
}

/// Ignore faces smaller than this (slivers from near-coincident walls). 1 sq ft.
const MIN_ROOM_AREA_SQ_IN: f64 = 144.0;

/// Detect rooms. Same as [`detect_rooms_inner`]: `polygon`/`area_sq_in` stay
/// the centerline values and the interior/standard fields are filled in too.
pub fn detect_rooms(walls: &[Wall], tol: f64) -> Vec<Room> {
    detect_rooms_inner(walls, tol)
}

/// Detect rooms and compute their interior-surface polygons (R-2): each
/// centerline polygon edge is offset inward by half the thickness of the wall
/// that owns it and adjacent offset edges are intersected. Walls whose flags
/// say they do not define rooms are skipped ([`crate::walls::WallFlags::defines_rooms`],
/// R-3..R-5). Curved walls bound rooms as faceted arcs (R-12).
pub fn detect_rooms_inner(walls: &[Wall], tol: f64) -> Vec<Room> {
    let defining: Vec<Wall> = expand_curves(walls)
        .into_iter()
        .filter(|w| w.flags.defines_rooms())
        .collect();
    let mut rooms = detect_centerline_rooms(&defining, tol);
    for r in rooms.iter_mut() {
        fill_surface_areas(r, &defining, tol);
    }
    rooms
}

/// Replace curved walls by their faceted chords (same id and properties).
fn expand_curves(walls: &[Wall]) -> Vec<Wall> {
    let mut out = Vec::with_capacity(walls.len());
    for w in walls {
        match w.curve {
            Some(c) if !c.is_straight() => {
                let n = c.facet_count(w.start, w.end);
                let pts = c.sample_points(w.start, w.end, n);
                for pair in pts.windows(2) {
                    let mut f = w.clone();
                    f.curve = None;
                    f.start = pair[0];
                    f.end = pair[1];
                    out.push(f);
                }
            }
            _ => out.push(w.clone()),
        }
    }
    out
}

/// Offset a counter-clockwise polygon: edge `i` (from vertex `i` to `i+1`)
/// moves left (inward) by `d[i]`; negative values move it outward. Adjacent
/// offset edges are intersected; parallel neighbours with different offsets
/// get a step. Returns `None` for a degenerate result.
fn offset_polygon(poly: &[Point], d: &[f64]) -> Option<Vec<Point>> {
    let n = poly.len();
    if n < 3 {
        return None;
    }
    let dirs: Vec<Point> = (0..n)
        .map(|i| poly[(i + 1) % n].sub(poly[i]).normalized())
        .collect();
    let mut out = Vec::with_capacity(n + 2);
    for j in 0..n {
        let prev = (j + n - 1) % n;
        let (dp, dc) = (dirs[prev], dirs[j]);
        let a = poly[j] + dp.perp() * d[prev];
        let b = poly[j] + dc.perp() * d[j];
        let cross = dp.cross(dc);
        if cross.abs() < 1e-6 {
            if (d[prev] - d[j]).abs() > 1e-9 {
                out.push(a);
            }
            out.push(b);
        } else {
            let t = (b - a).cross(dc) / cross;
            out.push(a + dp * t);
        }
    }
    let area = polygon_area(&out);
    (area.is_finite() && area > 0.0).then_some(out)
}

/// The wall that owns the polygon edge `a -> b`: nearest parallel wall to the
/// edge midpoint (the thicker one on ties).
fn edge_owner(walls: &[Wall], a: Point, b: Point, tol: f64) -> Option<&Wall> {
    let mid = Point::lerp(a, b, 0.5);
    let dir = b.sub(a).normalized();
    let mut best: Option<(f64, &Wall)> = None;
    for w in walls {
        if w.length() <= tol || w.direction().cross(dir).abs() > 1e-3 {
            continue;
        }
        let d = dist_to_segment(mid, w.start, w.end);
        if d > tol.max(0.5) {
            continue;
        }
        let better = match best {
            None => true,
            Some((bd, bw)) => {
                d < bd - 1e-9 || ((d - bd).abs() <= 1e-9 && w.thickness > bw.thickness)
            }
        };
        if better {
            best = Some((d, w));
        }
    }
    best.map(|(_, w)| w)
}

fn fill_surface_areas(room: &mut Room, walls: &[Wall], tol: f64) {
    let n = room.polygon.len();
    let owners: Vec<Option<&Wall>> = (0..n)
        .map(|i| edge_owner(walls, room.polygon[i], room.polygon[(i + 1) % n], tol))
        .collect();
    let inner_d: Vec<f64> = owners
        .iter()
        .map(|o| o.map_or(0.0, |w| w.thickness * 0.5))
        .collect();
    let std_d: Vec<f64> = owners
        .iter()
        .map(|o| match o {
            Some(w) if w.kind == WallKind::Exterior => -w.thickness * 0.5,
            _ => 0.0,
        })
        .collect();
    room.inner_polygon =
        offset_polygon(&room.polygon, &inner_d).unwrap_or_else(|| room.polygon.clone());
    room.interior_area_sq_in = polygon_area(&room.inner_polygon);
    room.standard_area_sq_in = offset_polygon(&room.polygon, &std_d)
        .map(|p| polygon_area(&p))
        .unwrap_or(room.area_sq_in);
}

impl Project {
    /// Total living area of `floor` in square feet (R-51, R-42): the interior
    /// area of every room whose Living Area setting resolves to included. A
    /// room's setting is its name entry's `include_in_living_area` override,
    /// else the `include_in_living_area` of its room type in `room_types`
    /// (unnamed or unknown types count as included).
    pub fn living_area_sq_ft(
        &self,
        floor: usize,
        rooms: &[Room],
        room_types: &[RoomTypeDef],
    ) -> f64 {
        let names = &self.floors[floor].room_names;
        let total: f64 = rooms
            .iter()
            .filter(|r| {
                let Some(n) = names
                    .iter()
                    .find(|n| point_in_polygon(n.anchor, &r.polygon))
                else {
                    return true;
                };
                n.include_in_living_area.unwrap_or_else(|| {
                    room_types
                        .iter()
                        .find(|t| t.name == n.room_type)
                        .is_none_or(|t| t.include_in_living_area)
                })
            })
            .map(|r| {
                if r.inner_polygon.is_empty() {
                    r.area_sq_in
                } else {
                    r.interior_area_sq_in
                }
            })
            .sum();
        sq_in_to_sq_ft(total)
    }
}

fn detect_centerline_rooms(walls: &[Wall], tol: f64) -> Vec<Room> {
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
                    ..Room::default()
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
        wall_t(id, x0, y0, x1, y1, 4.5, WallKind::Interior)
    }

    fn wall_t(id: u64, x0: f64, y0: f64, x1: f64, y1: f64, t: f64, kind: WallKind) -> Wall {
        Wall {
            id,
            ..Wall::new(Point::new(x0, y0), Point::new(x1, y1), t, 109.125, kind)
        }
    }

    fn box_walls(t: f64) -> Vec<Wall> {
        let k = WallKind::Exterior;
        vec![
            wall_t(1, 0.0, 0.0, 240.0, 0.0, t, k),
            wall_t(2, 240.0, 0.0, 240.0, 120.0, t, k),
            wall_t(3, 240.0, 120.0, 0.0, 120.0, t, k),
            wall_t(4, 0.0, 120.0, 0.0, 0.0, t, k),
        ]
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

    #[test]
    fn inner_polygon_is_offset_by_half_thickness() {
        let rooms = detect_rooms_inner(&box_walls(6.0), 0.5);
        assert_eq!(rooms.len(), 1);
        let r = &rooms[0];
        // Centerline values are unchanged.
        assert!((r.area_sq_ft() - 200.0).abs() < 1e-6);
        assert_eq!(r.centerline_polygon().len(), r.polygon.len());
        let expect = (240.0 - 6.0) * (120.0 - 6.0) / 144.0;
        assert!(
            (r.interior_area_sq_ft() - expect).abs() < 1e-6,
            "{}",
            r.interior_area_sq_ft()
        );
        assert!((r.interior_area() - expect).abs() < 1e-6);
        assert!(r
            .inner_polygon
            .iter()
            .all(|p| p.x >= 3.0 - 1e-9 && p.x <= 237.0 + 1e-9));
        // Standard area: outside of the exterior walls.
        let std = (240.0 + 6.0) * (120.0 + 6.0) / 144.0;
        assert!((r.standard_area_sq_ft() - std).abs() < 1e-6);
        // detect_rooms fills the same fields.
        let r2 = &detect_rooms(&box_walls(6.0), 0.5)[0];
        assert!((r2.interior_area_sq_ft() - expect).abs() < 1e-6);
    }

    #[test]
    fn inner_polygon_with_partition_and_mixed_thickness() {
        let mut walls = box_walls(6.0);
        walls.push(wall_t(5, 120.0, 0.0, 120.0, 120.0, 4.0, WallKind::Interior));
        let rooms = detect_rooms_inner(&walls, 0.5);
        assert_eq!(rooms.len(), 2);
        for r in &rooms {
            // 120 wide centerline: 3 (outer) + 2 (partition) off, 120 tall: 3 + 3 off.
            let expect = (120.0 - 3.0 - 2.0) * (120.0 - 6.0) / 144.0;
            assert!(
                (r.interior_area_sq_ft() - expect).abs() < 1e-6,
                "{}",
                r.interior_area_sq_ft()
            );
            // Standard: out 3 on three sides, centre of the partition.
            let std = (120.0 + 3.0) * (120.0 + 6.0) / 144.0;
            assert!((r.standard_area_sq_ft() - std).abs() < 1e-6);
        }
    }

    #[test]
    fn flags_control_room_definition() {
        let mut walls = box_walls(6.0);
        let mut div = wall_t(5, 120.0, 0.0, 120.0, 120.0, 4.0, WallKind::Interior);
        div.flags.room_divider = true;
        div.flags.invisible = true;
        walls.push(div);
        assert_eq!(detect_rooms_inner(&walls, 0.5).len(), 2);
        // An invisible wall that is not a divider does not split rooms.
        walls[4].flags.room_divider = false;
        assert_eq!(detect_rooms_inner(&walls, 0.5).len(), 1);
        walls[4].flags.invisible = false;
        walls[4].flags.no_room_definition = true;
        assert_eq!(detect_rooms_inner(&walls, 0.5).len(), 1);
        // A divider overrides no_room_definition.
        walls[4].flags.room_divider = true;
        assert_eq!(detect_rooms_inner(&walls, 0.5).len(), 2);
    }

    #[test]
    fn curved_wall_bounds_a_room() {
        // A D-shaped room: straight chord closed by a semicircular wall.
        let mut walls = vec![wall(1, 0.0, 0.0, 100.0, 0.0)];
        let mut arc = wall(2, 100.0, 0.0, 0.0, 0.0);
        arc.curve = Some(crate::walls::WallCurve { bulge: -50.0 });
        walls.push(arc);
        let rooms = detect_rooms_inner(&walls, 0.5);
        assert_eq!(rooms.len(), 1);
        let half_disc = std::f64::consts::PI * 50.0 * 50.0 / 2.0;
        assert!((rooms[0].area_sq_in - half_disc).abs() / half_disc < 0.02);
        assert!(rooms[0].interior_area_sq_in < rooms[0].area_sq_in);
    }

    #[test]
    fn living_area_uses_inner_area_and_type_flags() {
        let walls = {
            let mut w = box_walls(6.0);
            w.push(wall_t(5, 120.0, 0.0, 120.0, 120.0, 6.0, WallKind::Interior));
            w
        };
        let rooms = detect_rooms_inner(&walls, 0.5);
        let d = crate::defaults::PlanDefaults::chief_x18_daniel();
        let mut p = Project::new("l");
        let both = p.living_area_sq_ft(0, &rooms, &d.rooms.room_types);
        let one = rooms[0].interior_area_sq_ft();
        assert!((both - 2.0 * one).abs() < 1e-6);
        // Name the left room Garage: excluded.
        let left = rooms.iter().find(|r| r.centroid.x < 120.0).unwrap();
        p.set_room_name(0, left.centroid, "Garage", "Garage", &rooms);
        let after = p.living_area_sq_ft(0, &rooms, &d.rooms.room_types);
        assert!((after - one).abs() < 1e-6, "{after} vs {one}");
        // Per-room override wins over the type default.
        p.floors[0].room_names[0].include_in_living_area = Some(true);
        let over = p.living_area_sq_ft(0, &rooms, &d.rooms.room_types);
        assert!((over - 2.0 * one).abs() < 1e-6);
    }
}
