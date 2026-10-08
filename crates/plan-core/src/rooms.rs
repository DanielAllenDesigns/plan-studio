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
    segment_intersection, BoxGrid, Point,
};
use crate::model::{Project, RoomName, Wall, WallKind};
use crate::units::sq_in_to_sq_ft;
use std::collections::{HashMap, HashSet};

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
    /// Nested rooms (R-11): the centerline polygons of free-standing loops
    /// wholly inside this room (a closet pod, a chimney box). The areas above
    /// already exclude them and the floor and ceiling platforms have a hole
    /// under each.
    pub holes: Vec<Vec<Point>>,
    /// Area inside the outer surfaces of this room's walls, square inches:
    /// what this room takes out of an enclosing room's interior area when it
    /// is nested in one.
    pub outer_area_sq_in: f64,
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
    /// Is `p` in the room proper: inside its polygon and not inside one of
    /// its nested rooms (R-11)?
    pub fn contains(&self, p: Point) -> bool {
        point_in_polygon(p, &self.polygon) && !self.holes.iter().any(|h| point_in_polygon(p, h))
    }
    /// The first name entry anchored in this room proper (an anchor inside a
    /// nested room belongs to that room, not to this one).
    pub fn name_entry<'a>(&self, names: &'a [RoomName]) -> Option<&'a RoomName> {
        names.iter().find(|n| self.contains(n.anchor))
    }
}

/// How far a Garage floor sits below the house floor by default, inches
/// (R-40, R-26).
pub const GARAGE_FLOOR_DROP: f64 = 24.0;
/// Thickness of the concrete slab under a garage or porch, inches.
pub const SLAB_FLOOR_THICKNESS: f64 = 4.0;

/// What a room function (or a few named room types) sets on a room's
/// platforms (R-40, R-41): the defaults of the Structure switches and the
/// floor height offset, which stay editable per room.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDefaults {
    /// A floor platform under the room (off for Open Below, Attic, Courtyard).
    pub has_floor: bool,
    /// A ceiling platform over the room (off for Deck, Porch and Courtyard).
    pub has_ceiling: bool,
    /// Floor height offset from the floor datum, inches (a Garage drops it).
    pub floor_height_offset: f64,
    /// Floor finish thickness, inches; `None` keeps the floor's default.
    pub floor_finish_thickness: Option<f64>,
    /// Floor Structure layers; empty keeps the floor's default platform.
    pub floor_structure: Vec<crate::extras::StructureLayer>,
}

impl Default for FunctionDefaults {
    fn default() -> Self {
        Self {
            has_floor: true,
            has_ceiling: true,
            floor_height_offset: 0.0,
            floor_finish_thickness: None,
            floor_structure: Vec::new(),
        }
    }
}

/// The platform defaults of a room with function `function` (a room type's
/// function: Standard, Utility, Garage, Deck, Porch, Open Below) and room
/// type `type_name` (an Attic or Courtyard has no floor platform whatever its
/// function, and a Courtyard, open to the sky, has no ceiling either).
pub fn function_defaults(function: &str, type_name: &str) -> FunctionDefaults {
    use crate::extras::StructureLayer as L;
    let mut d = FunctionDefaults::default();
    match function {
        "Garage" => {
            d.floor_height_offset = -GARAGE_FLOOR_DROP;
            d.floor_finish_thickness = Some(0.0);
            d.floor_structure = vec![L::new("Concrete", SLAB_FLOOR_THICKNESS)];
        }
        "Deck" => {
            d.has_ceiling = false;
            d.floor_finish_thickness = Some(0.0);
            d.floor_structure = vec![L::new("Decking", 1.5), L::new("Joist", 7.25)];
        }
        "Porch" => {
            d.has_ceiling = false;
            d.floor_finish_thickness = Some(0.0);
            d.floor_structure = vec![L::new("Concrete", SLAB_FLOOR_THICKNESS)];
        }
        "Open Below" => d.has_floor = false,
        // A roof platform: a membrane deck with no ceiling under it.
        "Flat Roof" => {
            d.has_ceiling = false;
            d.floor_finish_thickness = Some(0.0);
            d.floor_structure = vec![L::new("Membrane", 0.5), L::new("Joist", 7.25)];
        }
        _ => {}
    }
    if matches!(type_name, "Attic" | "Courtyard") {
        d.has_floor = false;
    }
    if type_name == "Courtyard" {
        d.has_ceiling = false;
    }
    d
}

/// Give `name` the platform defaults `d` (R-41): the Structure switches, the
/// floor height offset, the Floor Structure and the floor finish
/// (`default_finish` when the function sets none). Run when a room's type
/// changes; every value stays editable afterwards.
pub fn apply_function_defaults(name: &mut RoomName, d: &FunctionDefaults, default_finish: f64) {
    name.has_floor = d.has_floor;
    name.has_ceiling = d.has_ceiling;
    name.floor_height_offset = d.floor_height_offset;
    let mut misc = name.misc.take().unwrap_or_default();
    misc.floor_structure = d.floor_structure.clone();
    misc.floor_finish_thickness = d.floor_finish_thickness.unwrap_or(default_finish);
    name.misc = Some(misc);
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
/// say they do not define rooms, or that stand raised above the floor, are skipped ([`crate::walls::Wall::defines_rooms`],
/// R-3..R-5). Curved walls bound rooms as faceted arcs (R-12).
pub fn detect_rooms_inner(walls: &[Wall], tol: f64) -> Vec<Room> {
    detect_rooms_with(walls, tol, true)
}

/// [`detect_rooms_inner`], with the position index switched on or off (off is
/// the plain scan the index must agree with).
fn detect_rooms_with(walls: &[Wall], tol: f64, indexed: bool) -> Vec<Room> {
    let defining: Vec<Wall> = expand_curves(walls)
        .into_iter()
        .filter(|w| w.defines_rooms())
        .collect();
    let mut rooms = detect_centerline_rooms(&defining, tol, indexed);
    // Each room edge looks for its owning wall among the walls near it.
    let reach = tol.max(0.5);
    let near = WallsNear::new(&defining, reach, indexed);
    for r in rooms.iter_mut() {
        fill_surface_areas(r, &defining, &near, tol);
    }
    nest_rooms(&mut rooms);
    rooms
}

/// Nested rooms (R-11): a room whose polygon lies wholly inside another
/// room's becomes a hole of the smallest such room, and that room's areas
/// give up what the island covers. Rooms that share wall edges (neighbours)
/// are never nested: every vertex of an island must lie strictly inside the
/// enclosing polygon.
fn nest_rooms(rooms: &mut [Room]) {
    let n = rooms.len();
    if n < 2 {
        return;
    }
    let bounds: Vec<(Point, Point)> = rooms
        .iter()
        .map(|r| crate::foundation::bounds(&r.polygon))
        .collect();
    // Index of the smallest enclosing room of each room.
    let mut parent: Vec<Option<usize>> = vec![None; n];
    for j in 0..n {
        let mut best: Option<usize> = None;
        for i in 0..n {
            if i == j || rooms[i].area_sq_in <= rooms[j].area_sq_in {
                continue;
            }
            let (ilo, ihi) = bounds[i];
            let (jlo, jhi) = bounds[j];
            if jlo.x < ilo.x || jlo.y < ilo.y || jhi.x > ihi.x || jhi.y > ihi.y {
                continue;
            }
            let inside = rooms[j].polygon.iter().all(|&p| {
                point_in_polygon(p, &rooms[i].polygon) && !on_boundary(p, &rooms[i].polygon)
            });
            if inside && best.is_none_or(|b| rooms[i].area_sq_in < rooms[b].area_sq_in) {
                best = Some(i);
            }
        }
        parent[j] = best;
    }
    for j in 0..n {
        let Some(i) = parent[j] else { continue };
        let hole = rooms[j].polygon.clone();
        let (area, outer) = (
            rooms[j].area_sq_in,
            rooms[j].outer_area_sq_in.max(rooms[j].area_sq_in),
        );
        let r = &mut rooms[i];
        r.holes.push(hole);
        r.area_sq_in = (r.area_sq_in - area).max(0.0);
        r.interior_area_sq_in = (r.interior_area_sq_in - outer).max(0.0);
        r.standard_area_sq_in = (r.standard_area_sq_in - area).max(0.0);
    }
}

/// Is `p` on the outline of `poly` (within a hair)?
fn on_boundary(p: Point, poly: &[Point]) -> bool {
    (0..poly.len()).any(|i| dist_to_segment(p, poly[i], poly[(i + 1) % poly.len()]) < 1e-6)
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

/// Walls found by position: a broad-phase over the walls' bounding boxes
/// (small lists are scanned instead). Candidates come back in wall order.
struct WallsNear {
    grid: Option<BoxGrid>,
    len: usize,
    reach: f64,
}

/// Below this many walls a scan beats building a grid.
const GRID_MIN_WALLS: usize = 24;

impl WallsNear {
    /// `reach` is how far from a wall's own box a query point still counts.
    fn new(walls: &[Wall], reach: f64, indexed: bool) -> Self {
        let grid = (indexed && walls.len() >= GRID_MIN_WALLS).then(|| {
            let boxes: Vec<(Point, Point)> = walls
                .iter()
                .map(|w| {
                    (
                        Point::new(
                            w.start.x.min(w.end.x) - reach,
                            w.start.y.min(w.end.y) - reach,
                        ),
                        Point::new(
                            w.start.x.max(w.end.x) + reach,
                            w.start.y.max(w.end.y) + reach,
                        ),
                    )
                })
                .collect();
            BoxGrid::new(&boxes)
        });
        WallsNear {
            grid,
            len: walls.len(),
            reach,
        }
    }

    /// The walls that may lie within `reach` of `p`, ascending.
    fn around(&self, p: Point, out: &mut Vec<usize>) {
        match &self.grid {
            Some(g) => g.query(p, p, out),
            None => {
                out.clear();
                out.extend(0..self.len);
            }
        }
    }
}

/// The wall that owns the polygon edge `a -> b`: nearest parallel wall to the
/// edge midpoint (the thicker one on ties).
fn edge_owner<'a>(
    walls: &'a [Wall],
    near: &WallsNear,
    a: Point,
    b: Point,
    tol: f64,
) -> Option<&'a Wall> {
    let mid = Point::lerp(a, b, 0.5);
    let dir = b.sub(a).normalized();
    let mut best: Option<(f64, &Wall)> = None;
    let mut candidates = Vec::new();
    near.around(mid, &mut candidates);
    debug_assert!(near.reach >= tol.max(0.5));
    for &i in &candidates {
        let w = &walls[i];
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

fn fill_surface_areas(room: &mut Room, walls: &[Wall], near: &WallsNear, tol: f64) {
    let n = room.polygon.len();
    let owners: Vec<Option<&Wall>> = (0..n)
        .map(|i| edge_owner(walls, near, room.polygon[i], room.polygon[(i + 1) % n], tol))
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
    let outer_d: Vec<f64> = inner_d.iter().map(|d| -d).collect();
    room.outer_area_sq_in = offset_polygon(&room.polygon, &outer_d)
        .map(|p| polygon_area(&p))
        .unwrap_or(room.area_sq_in);
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
                let Some(n) = r.name_entry(names) else {
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

fn detect_centerline_rooms(walls: &[Wall], tol: f64, indexed: bool) -> Vec<Room> {
    let segs = split_segments(walls, tol, indexed);

    let mut nodes = NodeIndex::new(tol, indexed);
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut seen_edges: HashSet<(usize, usize)> = HashSet::new();
    for (a, b) in segs {
        let ia = nodes.index(a);
        let ib = nodes.index(b);
        if ia == ib {
            continue;
        }
        let key = (ia.min(ib), ia.max(ib));
        if seen_edges.insert(key) {
            edges.push(key);
        }
    }
    let nodes = nodes.points;

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

/// The graph's nodes: points merged within `tol`, found through a hash of
/// `tol`-sized cells instead of a scan. `index` answers with the lowest node
/// index within `tol`, the same as scanning the list from the start.
struct NodeIndex {
    points: Vec<Point>,
    tol: f64,
    /// Off: every lookup scans the list (what the hash must agree with).
    hashed: bool,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl NodeIndex {
    fn new(tol: f64, hashed: bool) -> Self {
        NodeIndex {
            points: Vec::new(),
            tol,
            hashed,
            cells: HashMap::new(),
        }
    }

    fn cell(&self, p: Point) -> (i64, i64) {
        (
            (p.x / self.tol).floor() as i64,
            (p.y / self.tol).floor() as i64,
        )
    }

    fn index(&mut self, p: Point) -> usize {
        if self.hashed && self.tol > 0.0 && self.tol.is_finite() {
            let (cx, cy) = self.cell(p);
            let mut best: Option<usize> = None;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(list) = self.cells.get(&(cx + dx, cy + dy)) {
                        for &i in list {
                            if best.is_none_or(|b| i < b) && self.points[i].dist(p) <= self.tol {
                                best = Some(i);
                            }
                        }
                    }
                }
            }
            if let Some(i) = best {
                return i;
            }
            self.points.push(p);
            let i = self.points.len() - 1;
            self.cells.entry((cx, cy)).or_default().push(i);
            return i;
        }
        // A zero or odd tolerance: the plain scan.
        if let Some(i) = self.points.iter().position(|n| n.dist(p) <= self.tol) {
            return i;
        }
        self.points.push(p);
        self.points.len() - 1
    }
}

/// Break every wall centerline at the points where other walls end on it
/// (T-junctions) or cross it, so the graph is planar.
fn split_segments(walls: &[Wall], tol: f64, indexed: bool) -> Vec<(Point, Point)> {
    let near = WallsNear::new(walls, tol, indexed);
    let mut candidates = Vec::new();
    let mut out = Vec::new();
    for (i, w) in walls.iter().enumerate() {
        let (a, b) = (w.start, w.end);
        let len = a.dist(b);
        if len < tol {
            continue;
        }
        let mut ts = vec![0.0, 1.0];
        // Only walls whose box meets this wall's box (grown by `tol`) can end
        // on it or cross it.
        match &near.grid {
            Some(g) => g.query(
                Point::new(a.x.min(b.x) - tol, a.y.min(b.y) - tol),
                Point::new(a.x.max(b.x) + tol, a.y.max(b.y) + tol),
                &mut candidates,
            ),
            None => {
                candidates.clear();
                candidates.extend(0..walls.len());
            }
        }
        for &j in &candidates {
            if i == j {
                continue;
            }
            let o = &walls[j];
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

    /// A 20' x 10' room with a free-standing 4' x 3' closet loop inside it.
    fn room_with_closet(t: f64) -> Vec<Wall> {
        let k = WallKind::Interior;
        let mut walls = box_walls(t);
        walls.extend([
            wall_t(5, 60.0, 40.0, 108.0, 40.0, t, k),
            wall_t(6, 108.0, 40.0, 108.0, 76.0, t, k),
            wall_t(7, 108.0, 76.0, 60.0, 76.0, t, k),
            wall_t(8, 60.0, 76.0, 60.0, 40.0, t, k),
        ]);
        walls
    }

    #[test]
    fn a_closet_loop_inside_a_room_is_nested() {
        let rooms = detect_rooms(&room_with_closet(4.5), 0.5);
        assert_eq!(rooms.len(), 2, "outer room and closet");
        let (outer, closet) = if rooms[0].holes.is_empty() {
            (&rooms[1], &rooms[0])
        } else {
            (&rooms[0], &rooms[1])
        };
        assert_eq!(outer.holes.len(), 1);
        assert!(closet.holes.is_empty());
        let island = 48.0 * 36.0;
        assert!((outer.area_sq_in - (240.0 * 120.0 - island)).abs() < 1e-6);
        assert!((closet.area_sq_in - island).abs() < 1e-6);
        // The interior area gives up the closet's outside surfaces, not just
        // its centerline area.
        let outside = (48.0 + 4.5) * (36.0 + 4.5);
        let box_inner = (240.0 - 4.5) * (120.0 - 4.5);
        assert!((outer.interior_area_sq_in - (box_inner - outside)).abs() < 1e-6);
        // Points in the closet are in the closet, not the room around it.
        let inside = Point::new(80.0, 58.0);
        assert!(closet.contains(inside) && !outer.contains(inside));
        assert!(outer.contains(Point::new(30.0, 30.0)));
        // A name anchored in the closet belongs to the closet.
        let names = vec![RoomName::new(inside, "Closet", "Closet")];
        assert!(closet.name_entry(&names).is_some());
        assert!(outer.name_entry(&names).is_none());
    }

    #[test]
    fn neighbouring_rooms_are_not_nested() {
        let mut walls = box_walls(4.5);
        walls.push(wall(5, 120.0, 0.0, 120.0, 120.0));
        let rooms = detect_rooms(&walls, 0.5);
        assert_eq!(rooms.len(), 2);
        assert!(rooms.iter().all(|r| r.holes.is_empty()));
        assert!((rooms.iter().map(|r| r.area_sq_in).sum::<f64>() - 240.0 * 120.0).abs() < 1e-6);
    }

    #[test]
    fn function_defaults_follow_chief() {
        let g = function_defaults("Garage", "Garage");
        assert_eq!(g.floor_height_offset, -GARAGE_FLOOR_DROP);
        assert_eq!(g.floor_finish_thickness, Some(0.0));
        assert!(g.has_floor && g.has_ceiling);
        for f in ["Deck", "Porch"] {
            let d = function_defaults(f, f);
            assert!(d.has_floor && !d.has_ceiling, "{f}");
            assert!(!d.floor_structure.is_empty());
        }
        assert!(!function_defaults("Open Below", "Open Below").has_floor);
        let roof = function_defaults("Flat Roof", "Flat Roof");
        assert!(
            roof.has_floor && !roof.has_ceiling,
            "a roof deck has no ceiling"
        );
        assert!(!roof.floor_structure.is_empty());
        assert!(!function_defaults("Utility", "Attic").has_floor);
        let court = function_defaults("Standard", "Courtyard");
        assert!(!court.has_floor && !court.has_ceiling, "open to the sky");
        assert!(function_defaults("Utility", "Attic").has_ceiling);
        assert_eq!(
            function_defaults("Standard", "Bath"),
            FunctionDefaults::default()
        );
        let mut name = RoomName::new(Point::ZERO, "Garage", "Garage");
        apply_function_defaults(&mut name, &g, 0.75);
        assert_eq!(name.floor_height_offset, -24.0);
        let misc = name.misc.clone().unwrap();
        assert_eq!(misc.floor_finish_thickness, 0.0);
        assert_eq!(
            crate::extras::structure_thickness(&misc.floor_structure),
            4.0
        );
        // Back to a plain room: the floor's own finish returns.
        apply_function_defaults(&mut name, &FunctionDefaults::default(), 0.75);
        assert_eq!(name.floor_height_offset, 0.0);
        assert_eq!(name.misc.unwrap().floor_finish_thickness, 0.75);
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

    /// A messy grid: gaps, short walls, diagonals, Ts, mixed thickness.
    fn messy_plan(n: usize, seed: u64) -> Vec<Wall> {
        let mut s = seed;
        let mut rnd = move || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut walls = Vec::new();
        let mut id = 0;
        let mut add = |walls: &mut Vec<Wall>, a: (f64, f64), b: (f64, f64), t: f64, k| {
            id += 1;
            walls.push(wall_t(id, a.0, a.1, b.0, b.1, t, k));
        };
        for r in 0..=n {
            for c in 0..n {
                if rnd() < 0.15 {
                    continue;
                }
                let (x, y) = (c as f64 * 120.0, r as f64 * 120.0);
                let len = if rnd() < 0.1 { 60.0 } else { 120.0 };
                let k = if r == 0 || r == n {
                    WallKind::Exterior
                } else {
                    WallKind::Interior
                };
                add(
                    &mut walls,
                    (x, y),
                    (x + len, y),
                    [4.5, 6.0, 7.625][(r + c) % 3],
                    k,
                );
            }
        }
        for c in 0..=n {
            for r in 0..n {
                if rnd() < 0.15 {
                    continue;
                }
                let (x, y) = (c as f64 * 120.0, r as f64 * 120.0);
                add(&mut walls, (x, y), (x, y + 120.0), 4.5, WallKind::Interior);
            }
        }
        for _ in 0..n {
            let x = (rnd() * n as f64).floor() * 120.0;
            let y = (rnd() * n as f64).floor() * 120.0;
            add(
                &mut walls,
                (x, y),
                (x + 120.0, y + 120.0),
                4.5,
                WallKind::Interior,
            );
            add(
                &mut walls,
                (x + 60.0, y),
                (x + 60.0, y + 120.0),
                4.5,
                WallKind::Interior,
            );
        }
        walls
    }

    #[test]
    fn the_position_index_gives_the_rooms_a_scan_gives() {
        for (n, seed) in [(5, 7), (9, 8), (13, 9)] {
            let walls = messy_plan(n, seed);
            assert!(walls.len() > GRID_MIN_WALLS, "{} walls", walls.len());
            let fast = format!("{:?}", detect_rooms_with(&walls, 0.5, true));
            let scan = format!("{:?}", detect_rooms_with(&walls, 0.5, false));
            assert_eq!(fast, scan, "plan {n}");
            assert!(fast.len() > 100);
        }
    }

    #[test]
    fn node_index_returns_the_first_node_within_tolerance() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(0.3, 0.0),
            Point::new(0.6, 0.0),
            Point::new(-0.4, 0.4),
            Point::new(10.0, 10.0),
            Point::new(10.2, 10.1),
            Point::new(0.5, 0.0),
        ];
        let (mut a, mut b) = (NodeIndex::new(0.5, true), NodeIndex::new(0.5, false));
        for p in pts {
            assert_eq!(a.index(p), b.index(p), "{p:?}");
        }
        assert_eq!(a.points, b.points);
    }
}
