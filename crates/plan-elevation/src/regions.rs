//! Visible-region extraction from a label (id) buffer.
//!
//! Every raster pixel carries a label: background, or a (material, object,
//! kind) triple. The boundary of each label is traced as closed loops along
//! the pixel lattice. Staircases are then smoothed the way marching squares
//! does (the contour passes through the midpoints of the lattice edges, so a
//! diagonal becomes a straight line), except at corners between two straight
//! runs of two or more edges, which keep their exact lattice vertex so
//! rectangles stay square. Loops are simplified with Douglas-Peucker at
//! [`SIMPLIFY_PX`] and holes are joined to their outer ring by a zero-width
//! slit, giving one polygon per connected area.

use crate::drawing::{Region, RegionKind};
use plan_3d::Material;
use plan_core::geometry::{dist_to_segment, point_in_polygon, polygon_area};
use plan_core::{Id, Point};
use std::collections::HashMap;

/// Douglas-Peucker tolerance, pixels.
pub(crate) const SIMPLIFY_PX: f64 = 0.5;
/// Rings with this many vertices or fewer are not simplified.
const MIN_SIMPLIFY_VERTICES: usize = 8;
/// Loops enclosing less than this many square pixels are dropped.
const MIN_AREA_PX: f64 = 2.0;

/// What a non-zero label stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct LabelKey {
    pub material: Material,
    pub object_id: Option<Id>,
    pub kind: RegionKind,
}

/// Interning table: label `n` (1-based) is `keys[n - 1]`; label 0 is background.
#[derive(Debug, Default)]
pub(crate) struct Labels {
    keys: Vec<LabelKey>,
    index: HashMap<LabelKey, u32>,
}

impl Labels {
    pub fn get(&mut self, material: Material, object_id: Option<Id>, kind: RegionKind) -> u32 {
        let key = LabelKey {
            material,
            object_id,
            kind,
        };
        if let Some(&l) = self.index.get(&key) {
            return l;
        }
        self.keys.push(key);
        let label = self.keys.len() as u32;
        self.index.insert(key, label);
        label
    }

    /// The key of a non-zero label.
    pub fn key(&self, label: u32) -> Option<LabelKey> {
        label
            .checked_sub(1)
            .and_then(|i| self.keys.get(i as usize))
            .copied()
    }
}

/// One directed lattice edge with its label on the left (counter-clockwise
/// around the label's pixels, Y up).
#[derive(Clone, Copy)]
struct Edge {
    label: u32,
    from: u32,
    to: u32,
}

impl Edge {
    fn key(&self) -> u64 {
        (u64::from(self.label) << 32) | u64::from(self.from)
    }
}

/// Boundary edges between differing labels; background has none.
fn boundary_edges(ids: &[u32], w: usize, h: usize) -> Vec<Edge> {
    let vw = (w + 1) as u32;
    let vid = |x: usize, y: usize| y as u32 * vw + x as u32;
    let at = |x: isize, y: isize| -> u32 {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
            0
        } else {
            ids[y as usize * w + x as usize]
        }
    };
    let mut edges = Vec::new();
    for y in 0..h {
        for x in 0..=w {
            let (l, r) = (at(x as isize - 1, y as isize), at(x as isize, y as isize));
            if l == r {
                continue;
            }
            if l != 0 {
                edges.push(Edge {
                    label: l,
                    from: vid(x, y),
                    to: vid(x, y + 1),
                });
            }
            if r != 0 {
                edges.push(Edge {
                    label: r,
                    from: vid(x, y + 1),
                    to: vid(x, y),
                });
            }
        }
    }
    for y in 0..=h {
        for x in 0..w {
            let (b, t) = (at(x as isize, y as isize - 1), at(x as isize, y as isize));
            if b == t {
                continue;
            }
            if b != 0 {
                edges.push(Edge {
                    label: b,
                    from: vid(x + 1, y),
                    to: vid(x, y),
                });
            }
            if t != 0 {
                edges.push(Edge {
                    label: t,
                    from: vid(x, y),
                    to: vid(x + 1, y),
                });
            }
        }
    }
    edges
}

/// Closed loops (label, lattice vertices in order; edge `i` runs `v[i]` to `v[i + 1]`).
fn trace_loops(edges: &mut [Edge], w: usize) -> Vec<(u32, Vec<Point>)> {
    edges.sort_unstable_by_key(Edge::key);
    let vw = (w + 1) as u32;
    let vertex = |id: u32| Point::new(f64::from(id % vw), f64::from(id / vw));
    let mut used = vec![false; edges.len()];
    let mut loops = Vec::new();
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        let first = edges[start];
        let mut ring: Vec<Point> = Vec::new();
        let mut cur = start;
        loop {
            used[cur] = true;
            let e = edges[cur];
            ring.push(vertex(e.from));
            if e.to == first.from {
                break;
            }
            let want = (u64::from(e.label) << 32) | u64::from(e.to);
            let lo = edges.partition_point(|c| c.key() < want);
            match (lo..edges.len())
                .take_while(|&j| edges[j].key() == want)
                .find(|&j| !used[j])
            {
                Some(j) => cur = j,
                None => break,
            }
        }
        loops.push((first.label, ring));
    }
    loops
}

/// Smooth a lattice loop: edge midpoints, except exact vertices where two
/// runs of at least two collinear edges meet.
fn smooth_ring(v: &[Point]) -> Vec<Point> {
    let n = v.len();
    if n < 4 {
        return v.to_vec();
    }
    let dir = |i: usize| {
        let (a, b) = (v[i % n], v[(i + 1) % n]);
        ((b.x - a.x) as i32, (b.y - a.y) as i32)
    };
    let Some(s) = (0..n).find(|&i| dir(i + n - 1) != dir(i)) else {
        return v.to_vec();
    };
    // Runs of equal direction as (first edge, edge count), starting at a turn.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < n {
        let mut c = 1;
        while i + c < n && dir(s + i + c) == dir(s + i) {
            c += 1;
        }
        runs.push(((s + i) % n, c));
        i += c;
    }
    let mid = |e: usize| (v[e % n] + v[(e + 1) % n]) * 0.5;
    let m = runs.len();
    let mut out: Vec<Point> = Vec::with_capacity(2 * m);
    let mut push = |p: Point| {
        if out.last() != Some(&p) {
            out.push(p);
        }
    };
    for r in 0..m {
        let (first, count) = runs[r];
        let (prev, next) = (runs[(r + m - 1) % m].1, runs[(r + 1) % m].1);
        let last = first + count - 1;
        if count == 1 {
            push(mid(first));
        } else {
            push(if prev >= 2 { v[first % n] } else { mid(first) });
            push(if next >= 2 {
                v[(last + 1) % n]
            } else {
                mid(last)
            });
        }
    }
    if out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    out
}

/// Douglas-Peucker on an open polyline given as a slice; marks kept indices.
fn dp_mark(pts: &[Point], tol: f64, keep: &mut [bool]) {
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((lo, hi)) = stack.pop() {
        if hi <= lo + 1 {
            continue;
        }
        let (a, b) = (pts[lo], pts[hi]);
        let (mut far, mut far_d) = (lo, -1.0);
        for (i, &p) in pts.iter().enumerate().take(hi).skip(lo + 1) {
            let d = dist_to_segment(p, a, b);
            if d > far_d {
                far_d = d;
                far = i;
            }
        }
        if far_d > tol {
            keep[far] = true;
            stack.push((lo, far));
            stack.push((far, hi));
        }
    }
}

/// Douglas-Peucker for a closed ring, anchored at vertex 0 and the vertex farthest from it.
fn simplify_ring(ring: &[Point], tol: f64) -> Vec<Point> {
    let n = ring.len();
    if n <= 3 {
        return ring.to_vec();
    }
    let far = (1..n)
        .max_by(|&i, &j| ring[0].dist(ring[i]).total_cmp(&ring[0].dist(ring[j])))
        .unwrap_or(1);
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[far] = true;
    dp_mark(&ring[..=far], tol, &mut keep[..=far]);
    let back: Vec<Point> = ring[far..].iter().copied().chain([ring[0]]).collect();
    let mut back_keep = vec![false; back.len()];
    back_keep[0] = true;
    back_keep[back.len() - 1] = true;
    dp_mark(&back, tol, &mut back_keep);
    for (k, &kept) in back_keep.iter().enumerate().take(n - far).skip(1) {
        keep[far + k] |= kept;
    }
    ring.iter()
        .zip(&keep)
        .filter(|(_, &k)| k)
        .map(|(p, _)| *p)
        .collect()
}

/// Whether open segments `ab` and `cd` properly cross (shared endpoints do not count).
fn cross_properly(a: Point, b: Point, c: Point, d: Point) -> bool {
    let side = |p: Point, q: Point, r: Point| (q - p).cross(r - p);
    let (d1, d2) = (side(c, d, a), side(c, d, b));
    let (d3, d4) = (side(a, b, c), side(a, b, d));
    d1 * d2 < -1e-12 && d3 * d4 < -1e-12
}

/// Whether the slit `m`-`t` crosses an edge of any ring in `rings`.
fn slit_blocked(m: Point, t: Point, rings: &[&[Point]]) -> bool {
    rings.iter().any(|ring| {
        (0..ring.len()).any(|i| cross_properly(m, t, ring[i], ring[(i + 1) % ring.len()]))
    })
}

/// Join `holes` to `outer` with zero-width slits into a single ring.
fn bridge_holes(outer: Vec<Point>, mut holes: Vec<Vec<Point>>) -> Vec<Point> {
    let rightmost = |h: &[Point]| {
        h.iter()
            .enumerate()
            .max_by(|a, b| a.1.x.total_cmp(&b.1.x))
            .map(|(i, p)| (i, *p))
            .unwrap_or((0, Point::ZERO))
    };
    holes.sort_by(|a, b| rightmost(b).1.x.total_cmp(&rightmost(a).1.x));
    let mut poly = outer;
    while !holes.is_empty() {
        let hole = holes.remove(0);
        let (m, mp) = rightmost(&hole);
        let mut order: Vec<usize> = (0..poly.len()).collect();
        order.sort_by(|&i, &j| mp.dist(poly[i]).total_cmp(&mp.dist(poly[j])));
        let mut rings: Vec<&[Point]> = vec![&poly];
        rings.extend(holes.iter().map(Vec::as_slice));
        rings.push(&hole);
        let k = order
            .iter()
            .copied()
            .find(|&i| !slit_blocked(mp, poly[i], &rings))
            .or(order.first().copied())
            .unwrap_or(0);
        let mut joined = Vec::with_capacity(poly.len() + hole.len() + 2);
        joined.extend_from_slice(&poly[..=k]);
        joined.extend((0..=hole.len()).map(|s| hole[(m + s) % hole.len()]));
        joined.extend_from_slice(&poly[k..]);
        poly = joined;
    }
    poly
}

/// Extract the polygons of every label in `ids` (`w` x `h` pixels).
///
/// `to_point` maps pixel-lattice coordinates to drawing space. Regions are
/// ordered Face, Cut, Shadow, then by first appearance of their label, largest first.
pub(crate) fn extract(
    ids: &[u32],
    w: usize,
    h: usize,
    labels: &Labels,
    to_point: &dyn Fn(Point) -> Point,
) -> Vec<Region> {
    let mut edges = boundary_edges(ids, w, h);
    let loops = trace_loops(&mut edges, w);
    // Outer rings (counter-clockwise) and hole rings (clockwise) per label.
    type Rings = (Vec<Vec<Point>>, Vec<Vec<Point>>);
    let mut per_label: HashMap<u32, Rings> = HashMap::new();
    for (label, ring) in loops {
        let ring = smooth_ring(&ring);
        let area = polygon_area(&ring);
        if area.abs() < MIN_AREA_PX {
            continue;
        }
        // Short rings are already minimal; simplifying would eat thin features.
        let ring = if ring.len() > MIN_SIMPLIFY_VERTICES {
            simplify_ring(&ring, SIMPLIFY_PX)
        } else {
            ring
        };
        if ring.len() < 3 || polygon_area(&ring).abs() < MIN_AREA_PX * 0.5 {
            continue;
        }
        let entry = per_label.entry(label).or_default();
        if area > 0.0 {
            entry.0.push(ring);
        } else {
            entry.1.push(ring);
        }
    }

    let mut out: Vec<(RegionKind, u32, f64, Region)> = Vec::new();
    for (label, (outers, holes)) in per_label {
        let Some(key) = labels.key(label) else {
            continue;
        };
        let mut owned: Vec<Vec<Vec<Point>>> = vec![Vec::new(); outers.len()];
        for hole in holes {
            let host = outers
                .iter()
                .enumerate()
                .filter(|(_, o)| point_in_polygon(hole[0], o))
                .min_by(|a, b| polygon_area(a.1).total_cmp(&polygon_area(b.1)))
                .map(|(i, _)| i);
            if let Some(i) = host {
                owned[i].push(hole);
            }
        }
        for (outer, holes) in outers.into_iter().zip(owned) {
            let area =
                polygon_area(&outer) - holes.iter().map(|h| polygon_area(h).abs()).sum::<f64>();
            let ring = bridge_holes(outer, holes);
            out.push((
                key.kind,
                label,
                area,
                Region {
                    polygon: ring.into_iter().map(to_point).collect(),
                    material: key.material,
                    object_id: key.object_id,
                    kind: key.kind,
                },
            ));
        }
    }
    out.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(b.2.total_cmp(&a.2)));
    out.into_iter().map(|(_, _, _, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(w: usize, h: usize, rects: &[(usize, usize, usize, usize, u32)]) -> Vec<u32> {
        let mut ids = vec![0; w * h];
        for &(x0, y0, x1, y1, l) in rects {
            for y in y0..y1 {
                for x in x0..x1 {
                    ids[y * w + x] = l;
                }
            }
        }
        ids
    }

    fn labels(n: usize) -> Labels {
        let mut l = Labels::default();
        for i in 0..n {
            l.get(Material::Brick, Some(i as Id), RegionKind::Face);
        }
        l
    }

    #[test]
    fn rectangle_stays_square() {
        let ids = grid(40, 30, &[(5, 5, 35, 25, 1)]);
        let r = extract(&ids, 40, 30, &labels(1), &|p| p);
        assert_eq!(r.len(), 1);
        assert!((r[0].area() - 600.0).abs() < 1e-9);
        assert_eq!(r[0].polygon.len(), 4);
    }

    #[test]
    fn hole_is_bridged_and_subtracted() {
        let ids = grid(40, 30, &[(5, 5, 35, 25, 1), (15, 10, 25, 20, 0)]);
        let r = extract(&ids, 40, 30, &labels(1), &|p| p);
        assert_eq!(r.len(), 1);
        let a = r[0].area();
        assert!((a - 500.0).abs() < 1e-9, "area {a}");
    }

    #[test]
    fn island_in_a_hole_is_its_own_region() {
        let ids = grid(
            60,
            60,
            &[(5, 5, 55, 55, 1), (15, 15, 45, 45, 0), (25, 25, 35, 35, 2)],
        );
        let r = extract(&ids, 60, 60, &labels(2), &|p| p);
        assert_eq!(r.len(), 2);
        assert!((r[0].area() - (2500.0 - 900.0)).abs() < 1e-9);
        assert!((r[1].area() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn diagonal_edges_simplify_to_few_vertices() {
        let mut ids = vec![0u32; 64 * 64];
        for y in 0..60usize {
            for x in 0..64usize {
                if x < y + 2 && y < 60 {
                    ids[y * 64 + x] = 1;
                }
            }
        }
        let r = extract(&ids, 64, 64, &labels(1), &|p| p);
        assert_eq!(r.len(), 1);
        assert!(r[0].polygon.len() <= 10, "{} vertices", r[0].polygon.len());
    }

    #[test]
    fn specks_are_dropped() {
        let ids = grid(20, 20, &[(3, 3, 4, 4, 1)]);
        assert!(extract(&ids, 20, 20, &labels(1), &|p| p).is_empty());
    }
}
