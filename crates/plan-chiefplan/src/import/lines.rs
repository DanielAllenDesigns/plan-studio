//! Edge records: the 2-D line records that outline polygons.
//!
//! Many plan objects store an outline as a chain of line records: a
//! countertop island, a roof plane, a stair landing, the box a room label
//! frames. Each record is
//!
//! ```text
//! 04 20           two marker bytes
//! +2   f64 x      start point, plan inches
//! +10  f64 y
//! +18  f64 dx     unit direction
//! +26  f64 dy
//! +34  f64 length
//! ...             per-edge flags, ids and the heights of roof edges
//! ```
//!
//! This is the same `x, y, dx, dy, length` field set as a wall's reference
//! line (class 31). The records of one outline sit one after another at a fixed
//! stride per object type (90 bytes for countertops and landings, 349 to 365
//! for roof planes), the end of one is the start of the next, and a zero-length
//! edge may sit between two others.

use super::tree::f64_at;

/// One line record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    /// Offset of `x` from the object's `CD` byte.
    pub offset: usize,
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    pub len: f64,
}

impl Edge {
    pub fn start(&self) -> (f64, f64) {
        (self.x, self.y)
    }

    pub fn end(&self) -> (f64, f64) {
        (self.x + self.dx * self.len, self.y + self.dy * self.len)
    }
}

/// Every plausible edge record in `[from, to)` (absolute offsets), in file
/// order. `base` is the object's `CD` offset.
pub fn find_edges(bytes: &[u8], base: usize, from: usize, to: usize) -> Vec<Edge> {
    let mut out = Vec::new();
    let to = to.min(bytes.len());
    let mut i = from;
    while i + 42 <= to {
        if bytes[i] == 0x04 && bytes[i + 1] == 0x20 {
            let v = |k: usize| f64_at(bytes, i + 2 + 8 * k).filter(|v| v.is_finite());
            if let (Some(x), Some(y), Some(dx), Some(dy), Some(len)) =
                (v(0), v(1), v(2), v(3), v(4))
            {
                if (dx * dx + dy * dy - 1.0).abs() < 1e-6
                    && x.abs() < 1.0e6
                    && y.abs() < 1.0e6
                    && (0.0..1.0e5).contains(&len)
                {
                    out.push(Edge {
                        offset: i + 2 - base,
                        x,
                        y,
                        dx,
                        dy,
                        len,
                    });
                    i += 42;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05
}

/// The chain of edges that starts at `edges[first]`: each one starts where the
/// previous ended. Stops at the first gap or when the chain returns to its
/// start (a closed outline).
pub fn chain(edges: &[Edge], first: usize) -> Vec<Edge> {
    let mut out = vec![edges[first]];
    for e in &edges[first + 1..] {
        let last = out[out.len() - 1];
        if near(last.end(), out[0].start()) && out.len() > 2 {
            break;
        }
        if near(last.end(), e.start()) {
            out.push(*e);
        } else {
            break;
        }
    }
    out
}

/// Whether the chain returns to its start.
pub fn is_closed(chain: &[Edge]) -> bool {
    match (chain.first(), chain.last()) {
        (Some(a), Some(b)) => chain.len() >= 3 && near(b.end(), a.start()),
        _ => false,
    }
}

/// The corner points of a chain: edge starts, without zero-length edges,
/// repeated points and collinear middle points.
pub fn polygon_of(chain: &[Edge]) -> Vec<(f64, f64)> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for e in chain {
        if e.len < 0.01 {
            continue;
        }
        if pts.last().is_none_or(|&p| !near(p, e.start())) {
            pts.push(e.start());
        }
    }
    if pts.len() > 1 && near(pts[0], pts[pts.len() - 1]) {
        pts.pop();
    }
    // Drop collinear middle points.
    let n = pts.len();
    if n < 4 {
        return pts;
    }
    let mut keep: Vec<(f64, f64)> = Vec::with_capacity(n);
    for k in 0..n {
        let (a, b, c) = (pts[(k + n - 1) % n], pts[k], pts[(k + 1) % n]);
        let cross = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
        let scale = (b.0 - a.0).hypot(b.1 - a.1) * (c.0 - b.0).hypot(c.1 - b.1);
        if scale > 0.0 && (cross / scale).abs() < 1e-6 {
            continue;
        }
        keep.push(b);
    }
    keep
}

/// Signed area of a polygon (positive counter-clockwise), square inches.
pub fn signed_area(pts: &[(f64, f64)]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|k| {
            let (a, b) = (pts[k], pts[(k + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::tree::testutil::*;

    /// Writes an edge record `04 20 x y dx dy len` at `at` (relative to the CD
    /// byte).
    pub fn put_edge(b: &mut [u8], at: usize, e: (f64, f64, f64, f64, f64)) {
        b[at - 2] = 0x04;
        b[at - 1] = 0x20;
        for (k, v) in [e.0, e.1, e.2, e.3, e.4].iter().enumerate() {
            put_f64(b, at + 8 * k, *v);
        }
    }

    /// Writes a rectangle `w` x `h` with its lower-left corner at `(x, y)`
    /// as four edges 90 bytes apart starting at `at`.
    pub fn put_rect(b: &mut [u8], at: usize, x: f64, y: f64, w: f64, h: f64) {
        put_edge(b, at, (x, y, 1.0, 0.0, w));
        put_edge(b, at + 90, (x + w, y, 0.0, 1.0, h));
        put_edge(b, at + 180, (x + w, y + h, -1.0, 0.0, w));
        put_edge(b, at + 270, (x, y + h, 0.0, -1.0, h));
    }

    #[test]
    fn chains_a_rectangle_and_stops_at_a_gap() {
        let obj = sized(109, 0, 0x300, |b| {
            put_rect(b, 0x80, 908.375, 705.0, 68.0, 120.0);
            // A separate edge elsewhere does not join the chain.
            put_edge(b, 0x200, (10.0, 10.0, 1.0, 0.0, 5.0));
        });
        let m = obj.iter().position(|&c| c == 0xCD).unwrap();
        let edges = find_edges(&obj, m, m, obj.len());
        assert_eq!(edges.len(), 5);
        let c = chain(&edges, 0);
        assert_eq!(c.len(), 4);
        assert!(is_closed(&c));
        let poly = polygon_of(&c);
        assert_eq!(poly.len(), 4);
        assert_eq!(poly[0], (908.375, 705.0));
        assert!((signed_area(&poly) - 68.0 * 120.0).abs() < 1e-6);
    }

    #[test]
    fn zero_length_and_collinear_points_are_dropped() {
        let obj = sized(50, 0, 0x400, |b| {
            put_edge(b, 0x80, (0.0, 0.0, 1.0, 0.0, 10.0));
            put_edge(b, 0x80 + 90, (10.0, 0.0, 1.0, 0.0, 10.0));
            put_edge(b, 0x80 + 180, (20.0, 0.0, 0.0, 1.0, 0.0));
            put_edge(b, 0x80 + 270, (20.0, 0.0, 0.0, 1.0, 10.0));
            put_edge(b, 0x80 + 360, (20.0, 10.0, -1.0, 0.0, 20.0));
            put_edge(b, 0x80 + 450, (0.0, 10.0, 0.0, -1.0, 10.0));
        });
        let m = obj.iter().position(|&c| c == 0xCD).unwrap();
        let edges = find_edges(&obj, m, m, obj.len());
        let c = chain(&edges, 0);
        assert!(is_closed(&c));
        let poly = polygon_of(&c);
        assert_eq!(
            poly,
            vec![(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (0.0, 10.0)]
        );
    }

    #[test]
    fn rejects_non_unit_directions() {
        let obj = sized(50, 0, 0x200, |b| {
            put_edge(b, 0x80, (0.0, 0.0, 0.5, 0.5, 10.0));
        });
        let m = obj.iter().position(|&c| c == 0xCD).unwrap();
        assert!(find_edges(&obj, m, m, obj.len()).is_empty());
    }
}
