//! Boolean operations on closed polylines (Union, Subtract, Intersect), with
//! arcs (CAD-55).
//!
//! A shape is a [`Ring`]: its points, and for each edge the circle it is a
//! piece of when it belongs to an arc (a polyline arc edge, or a circle).
//! The operation works on the flattened edges:
//!
//! 1. every edge of one region is cut at the points where the other region's
//!    edges cross or touch it (a crossing that involves an arc is moved onto
//!    the exact circle, so the cut lies on the arc and not on its chord);
//!    shared and overlapping edges are cut at the ends of the overlap;
//! 2. each piece is classified against the other region: inside, outside, or
//!    lying on its boundary running the same way or the opposite way;
//! 3. the pieces the operation keeps (Union: outside, Intersect: inside,
//!    Subtract: A outside and B inside reversed) are chained into rings that
//!    keep the interior on their left, so outer rings run counter-clockwise
//!    and holes clockwise;
//! 4. collinear vertices are merged and the runs of pieces of one circle are
//!    fitted back to arcs: [`ring_to_poly`] writes the control vertices with
//!    the bulge of each arc edge and resamples the arc exactly like the
//!    polyline arc edges of the CAD tool (every 7.5 degrees), so the result
//!    is an ordinary polyline with `CadAttrs::arc_edges`.
//!
//! Edges that touch only along a stretch (two squares sharing a side)
//! dissolve in a Union, vanish in an Intersect and stay in a Subtract.
//! Self-intersecting outlines are not supported, and two arcs of the same
//! circle sampled at different points are treated as crossing chords, not as
//! a shared edge.

use crate::cad::{CadItem, PolyArc};
use crate::geometry::{dist_to_segment, point_in_polygon, polygon_area, project_on_segment, Point};
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

/// Points closer than this are one point.
const SNAP: f64 = 1e-6;
/// How far a point may be from a segment and still lie on it.
const ON_TOL: f64 = 1e-5;
/// Flattening step of a circle given as a circle, degrees.
const FLAT_DEG: f64 = 5.0;
/// Angle between the samples of a fitted arc edge, degrees (as in the CAD
/// tool's polyline arc edges).
const SAMPLE_DEG: f64 = 7.5;
/// Largest bulge of one arc edge (a sweep of about 345 degrees).
const MAX_BULGE: f64 = 8.0;
/// A ring smaller than this (square inches) is dropped.
const MIN_AREA: f64 = 1e-6;
/// How far a sample of a polyline arc edge may stray from its circle.
const ARC_TOL: f64 = 0.5;

/// Which Boolean operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    /// Everything either shape covers.
    Union,
    /// The first shape without the others.
    Subtract,
    /// What all shapes cover.
    Intersect,
}

impl BoolOp {
    pub fn name(self) -> &'static str {
        match self {
            BoolOp::Union => "Union",
            BoolOp::Subtract => "Subtract",
            BoolOp::Intersect => "Intersect",
        }
    }
}

/// A circle an edge is a piece of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub center: Point,
    pub radius: f64,
}

impl Circle {
    fn same(&self, o: &Circle) -> bool {
        self.center.dist(o.center) <= 1e-4 * (1.0 + self.radius)
            && (self.radius - o.radius).abs() <= 1e-4 * (1.0 + self.radius)
    }
}

/// A closed outline: `arcs[i]` is the circle of the edge from `pts[i]` to
/// `pts[(i + 1) % n]`, `None` for a straight edge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ring {
    pub pts: Vec<Point>,
    pub arcs: Vec<Option<Circle>>,
}

/// Center and radius of the arc edge from `a` to `b` with `bulge`
/// (`tan(sweep / 4)`, positive counter-clockwise).
pub fn arc_center(a: Point, b: Point, bulge: f64) -> Option<(Point, f64)> {
    let chord = b.sub(a);
    let l = chord.length();
    let theta = 4.0 * bulge.atan();
    if l < 1e-9 || theta.abs() < 1e-9 {
        return None;
    }
    let left = chord.normalized().perp();
    let center = Point::lerp(a, b, 0.5).add(left.scale(l * 0.5 / (theta * 0.5).tan()));
    Some((center, l / (2.0 * (theta.abs() * 0.5).sin())))
}

/// `n + 1` points along the arc edge `a` to `b` with `bulge`, ends included
/// (the same samples as the CAD tool's polyline arc edges).
pub fn arc_samples(a: Point, b: Point, bulge: f64, n: usize) -> Vec<Point> {
    let (Some((c, r)), true) = (arc_center(a, b, bulge), n >= 2) else {
        return vec![a, b];
    };
    let theta = 4.0 * bulge.atan();
    let a0 = a.sub(c).angle();
    (0..=n)
        .map(|k| match k {
            0 => a,
            k if k == n => b,
            k => {
                let ang = a0 + theta * k as f64 / n as f64;
                Point::new(c.x + r * ang.cos(), c.y + r * ang.sin())
            }
        })
        .collect()
}

/// How many segments an arc edge with `bulge` is cut into.
pub fn sample_count(bulge: f64) -> usize {
    ((4.0 * bulge.atan()).abs().to_degrees() / SAMPLE_DEG)
        .ceil()
        .clamp(4.0, 96.0) as usize
}

impl Ring {
    /// A ring of straight edges.
    pub fn polygon(pts: Vec<Point>) -> Ring {
        let n = pts.len();
        Ring {
            pts,
            arcs: vec![None; n],
        }
    }

    /// A circle as a ring of chords (counter-clockwise).
    pub fn circle(center: Point, radius: f64) -> Ring {
        let n = ((360.0 / FLAT_DEG).round() as usize).max(8);
        let pts = (0..n)
            .map(|k| {
                let a = TAU * k as f64 / n as f64;
                Point::new(center.x + radius * a.cos(), center.y + radius * a.sin())
            })
            .collect();
        Ring {
            pts,
            arcs: vec![Some(Circle { center, radius }); n],
        }
    }

    /// The ring of a closed polyline. `arcs` are its arc edges: the points
    /// between the two ends of an edge are samples along the arc (a span
    /// whose samples stray from its circle is read as straight chords).
    /// Counter-clockwise; `None` for fewer than three points or no area.
    pub fn from_polyline(points: &[Point], arcs: &[PolyArc]) -> Option<Ring> {
        let n = points.len();
        if n < 3 {
            return None;
        }
        let mut tags: Vec<Option<Circle>> = vec![None; n];
        for s in arcs {
            if s.from + 2 > s.to || s.to > n || s.from >= n {
                continue;
            }
            let (a, b) = (points[s.from], points[s.to % n]);
            let Some((center, radius)) = arc_center(a, b, s.bulge) else {
                continue;
            };
            let on_circle =
                (s.from..=s.to).all(|k| (points[k % n].dist(center) - radius).abs() <= ARC_TOL);
            if on_circle {
                for tag in &mut tags[s.from..s.to] {
                    *tag = Some(Circle { center, radius });
                }
            }
        }
        let ring = Ring {
            pts: points.to_vec(),
            arcs: tags,
        }
        .cleaned();
        ring.oriented_ccw()
    }

    /// The ring of a closed CAD item: a closed polyline (with its arc edges)
    /// or a circle.
    pub fn from_item(item: &CadItem, arcs: &[PolyArc]) -> Option<Ring> {
        match item {
            CadItem::Polyline {
                points,
                closed: true,
            } => Ring::from_polyline(points, arcs),
            CadItem::Circle { center, radius } if *radius > SNAP => {
                Some(Ring::circle(*center, *radius))
            }
            _ => None,
        }
    }

    /// Signed area (positive counter-clockwise).
    pub fn area(&self) -> f64 {
        polygon_area(&self.pts)
    }

    fn cleaned(self) -> Ring {
        let mut pts: Vec<Point> = Vec::with_capacity(self.pts.len());
        let mut arcs: Vec<Option<Circle>> = Vec::with_capacity(self.pts.len());
        for (p, t) in self.pts.iter().zip(self.arcs.iter()) {
            match pts.last() {
                Some(last) if last.dist(*p) <= SNAP => {
                    if let Some(l) = arcs.last_mut() {
                        *l = *t;
                    }
                }
                _ => {
                    pts.push(*p);
                    arcs.push(*t);
                }
            }
        }
        while pts.len() > 1 && pts[0].dist(pts[pts.len() - 1]) <= SNAP {
            pts.pop();
            arcs.pop();
        }
        Ring { pts, arcs }
    }

    fn reversed(&self) -> Ring {
        let n = self.pts.len();
        let pts: Vec<Point> = self.pts.iter().rev().copied().collect();
        // Reversed edge j runs from the old vertex n-1-j to n-2-j.
        let arcs = (0..n).map(|j| self.arcs[(2 * n - 2 - j) % n]).collect();
        Ring { pts, arcs }
    }

    fn oriented_ccw(self) -> Option<Ring> {
        let a = self.area();
        if self.pts.len() < 3 || a.abs() < MIN_AREA {
            return None;
        }
        Some(if a < 0.0 { self.reversed() } else { self })
    }
}

// ----- edges -----

#[derive(Debug, Clone, Copy)]
struct Edge {
    a: Point,
    b: Point,
    arc: Option<Circle>,
}

fn edges_of(region: &[Ring]) -> Vec<Edge> {
    let mut out = Vec::new();
    for r in region {
        let n = r.pts.len();
        for i in 0..n {
            out.push(Edge {
                a: r.pts[i],
                b: r.pts[(i + 1) % n],
                arc: r.arcs[i],
            });
        }
    }
    out
}

/// Merges points closer than [`SNAP`] and numbers the distinct ones.
#[derive(Default)]
struct Snap {
    cells: HashMap<(i64, i64), Vec<usize>>,
    pts: Vec<Point>,
}

impl Snap {
    const CELL: f64 = 1e-3;

    fn id(&mut self, p: Point) -> usize {
        let cx = (p.x / Self::CELL).floor() as i64;
        let cy = (p.y / Self::CELL).floor() as i64;
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = self.cells.get(&(cx + dx, cy + dy)) {
                    for &i in v {
                        if self.pts[i].dist(p) <= SNAP {
                            return i;
                        }
                    }
                }
            }
        }
        self.pts.push(p);
        self.cells
            .entry((cx, cy))
            .or_default()
            .push(self.pts.len() - 1);
        self.pts.len() - 1
    }

    fn point(&mut self, p: Point) -> Point {
        let i = self.id(p);
        self.pts[i]
    }
}

/// Intersections of the line through `a`, `b` with `c`.
fn line_circle(a: Point, b: Point, c: &Circle) -> Vec<Point> {
    let d = b.sub(a);
    let len2 = d.dot(d);
    if len2 < 1e-18 {
        return Vec::new();
    }
    let f = a.sub(c.center);
    let (qa, qb, qc) = (len2, 2.0 * f.dot(d), f.dot(f) - c.radius * c.radius);
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 {
        return Vec::new();
    }
    let sq = disc.sqrt();
    [(-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa)]
        .iter()
        .map(|t| a.add(d.scale(*t)))
        .collect()
}

/// Intersections of two circles.
fn circle_circle(c1: &Circle, c2: &Circle) -> Vec<Point> {
    let d = c2.center.sub(c1.center);
    let dist = d.length();
    if dist < 1e-9 || dist > c1.radius + c2.radius || dist < (c1.radius - c2.radius).abs() {
        return Vec::new();
    }
    let a = (c1.radius * c1.radius - c2.radius * c2.radius + dist * dist) / (2.0 * dist);
    let h2 = c1.radius * c1.radius - a * a;
    let h = h2.max(0.0).sqrt();
    let u = d.scale(1.0 / dist);
    let mid = c1.center.add(u.scale(a));
    vec![mid.add(u.perp().scale(h)), mid.sub(u.perp().scale(h))]
}

/// `p`, a crossing of the chords `e` and `f`, moved onto the exact circle of
/// whichever of them is an arc (the nearest exact intersection).
fn refine(p: Point, e: &Edge, f: &Edge) -> Point {
    let cands = match (e.arc, f.arc) {
        (None, None) => return p,
        (Some(c), None) => line_circle(f.a, f.b, &c),
        (None, Some(c)) => line_circle(e.a, e.b, &c),
        (Some(c1), Some(c2)) => circle_circle(&c1, &c2),
    };
    let reach = 0.01
        * e.arc
            .iter()
            .chain(f.arc.iter())
            .map(|c| c.radius)
            .fold(0.0, f64::max)
        + 0.01;
    cands
        .into_iter()
        .map(|q| (q.dist(p), q))
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(p, |(_, q)| q)
}

/// The pieces of the edges of `x` after cutting them at the crossings and
/// touches with the edges of `y`.
fn split(x: &[Edge], y: &[Edge], snap: &mut Snap) -> Vec<Edge> {
    let mut out = Vec::new();
    for e in x {
        let d = e.b.sub(e.a);
        let elen = d.length();
        let mut cuts: Vec<(f64, Point)> = Vec::new();
        for f in y {
            for q in [f.a, f.b] {
                let (t, pr) = project_on_segment(q, e.a, e.b);
                if pr.dist(q) <= ON_TOL && q.dist(e.a) > ON_TOL && q.dist(e.b) > ON_TOL {
                    cuts.push((t, q));
                }
            }
            let fd = f.b.sub(f.a);
            let flen = fd.length();
            let denom = d.cross(fd);
            if elen < 1e-12 || flen < 1e-12 || denom.abs() <= 1e-12 * elen * flen {
                continue;
            }
            let qp = f.a.sub(e.a);
            let t = qp.cross(fd) / denom;
            let u = qp.cross(d) / denom;
            if t <= 0.0 || t >= 1.0 || u <= 0.0 || u >= 1.0 {
                continue;
            }
            let p = e.a.add(d.scale(t));
            if [e.a, e.b, f.a, f.b].iter().any(|v| v.dist(p) <= ON_TOL) {
                continue;
            }
            let p = refine(p, e, f);
            let (t2, _) = project_on_segment(p, e.a, e.b);
            if t2 > 0.0 && t2 < 1.0 {
                cuts.push((t2, p));
            } else {
                cuts.push((t, e.a.add(d.scale(t))));
            }
        }
        cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut chain: Vec<Point> = vec![snap.point(e.a)];
        for (_, p) in cuts {
            chain.push(snap.point(p));
        }
        chain.push(snap.point(e.b));
        for w in chain.windows(2) {
            if w[0].dist(w[1]) > SNAP * 0.5 {
                out.push(Edge {
                    a: w[0],
                    b: w[1],
                    arc: e.arc,
                });
            }
        }
    }
    out
}

/// Where a piece lies against the other region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rel {
    Inside,
    Outside,
    /// On the boundary, running the same way.
    Same,
    /// On the boundary, running the other way.
    Opposite,
}

fn rings_contain(rings: &[Vec<Point>], p: Point) -> bool {
    rings.iter().filter(|r| point_in_polygon(p, r)).count() % 2 == 1
}

fn classify(s: &Edge, other: &[Edge], other_rings: &[Vec<Point>]) -> Rel {
    let dir = s.b.sub(s.a);
    for f in other {
        if dist_to_segment(s.a, f.a, f.b) <= ON_TOL && dist_to_segment(s.b, f.a, f.b) <= ON_TOL {
            return if dir.dot(f.b.sub(f.a)) > 0.0 {
                Rel::Same
            } else {
                Rel::Opposite
            };
        }
    }
    if rings_contain(other_rings, Point::lerp(s.a, s.b, 0.5)) {
        Rel::Inside
    } else {
        Rel::Outside
    }
}

fn polygons(region: &[Ring]) -> Vec<Vec<Point>> {
    region.iter().map(|r| r.pts.clone()).collect()
}

/// Joins the kept pieces into rings with the interior on the left.
fn chain(edges: Vec<Edge>, snap: &mut Snap) -> Vec<Ring> {
    struct E {
        from: usize,
        to: usize,
        e: Edge,
    }
    let es: Vec<E> = edges
        .into_iter()
        .map(|e| {
            let (from, to) = (snap.id(e.a), snap.id(e.b));
            E { from, to, e }
        })
        .filter(|x| x.from != x.to)
        .collect();
    let mut out_of: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, x) in es.iter().enumerate() {
        out_of.entry(x.from).or_default().push(i);
    }
    let mut used = vec![false; es.len()];
    let mut rings = Vec::new();
    for start in 0..es.len() {
        if used[start] {
            continue;
        }
        let origin = es[start].from;
        let mut cur = start;
        let mut pts: Vec<Point> = Vec::new();
        let mut arcs: Vec<Option<Circle>> = Vec::new();
        let closed = loop {
            used[cur] = true;
            pts.push(es[cur].e.a);
            arcs.push(es[cur].e.arc);
            let at = es[cur].to;
            if at == origin {
                break true;
            }
            let din = es[cur].e.b.sub(es[cur].e.a).normalized();
            let next = out_of
                .get(&at)
                .into_iter()
                .flatten()
                .copied()
                .filter(|j| !used[*j])
                .max_by(|a, b| {
                    let turn = |j: usize| {
                        let dout = es[j].e.b.sub(es[j].e.a).normalized();
                        let ang = din.cross(dout).atan2(din.dot(dout));
                        // A U-turn is the last resort.
                        if ang.abs() > PI - 1e-9 {
                            -10.0
                        } else {
                            ang
                        }
                    };
                    turn(*a).total_cmp(&turn(*b))
                });
            match next {
                Some(j) => cur = j,
                None => break false,
            }
        };
        if closed && pts.len() >= 3 {
            rings.push(Ring { pts, arcs });
        }
    }
    rings
}

/// Removes the vertices that sit on a straight run, and the rings with no
/// area.
fn simplify(rings: Vec<Ring>) -> Vec<Ring> {
    let mut out = Vec::new();
    for mut r in rings {
        loop {
            let n = r.pts.len();
            if n < 3 {
                break;
            }
            let mut removed = false;
            for i in 0..n {
                let prev = (i + n - 1) % n;
                let next = (i + 1) % n;
                if r.arcs[prev].is_none() && r.arcs[i].is_none() {
                    let (a, b, c) = (r.pts[prev], r.pts[i], r.pts[next]);
                    let straight =
                        dist_to_segment(b, a, c) <= ON_TOL && b.sub(a).dot(c.sub(b)) > 0.0;
                    if straight {
                        r.pts.remove(i);
                        r.arcs.remove(i);
                        removed = true;
                        break;
                    }
                }
            }
            if !removed {
                break;
            }
        }
        if r.pts.len() >= 3 && r.area().abs() > MIN_AREA {
            out.push(r);
        }
    }
    out
}

/// Which pieces of `a` and `b` (regions: outer rings counter-clockwise, holes
/// clockwise) the operation keeps, as the rings of the result.
pub fn boolean(op: BoolOp, a: &[Ring], b: &[Ring]) -> Vec<Ring> {
    if a.is_empty() {
        return match op {
            BoolOp::Union => b.to_vec(),
            _ => Vec::new(),
        };
    }
    if b.is_empty() {
        return match op {
            BoolOp::Intersect => Vec::new(),
            _ => a.to_vec(),
        };
    }
    let mut snap = Snap::default();
    let (ea, eb) = (edges_of(a), edges_of(b));
    let (pa, pb) = (split(&ea, &eb, &mut snap), split(&eb, &ea, &mut snap));
    let (ra, rb) = (polygons(a), polygons(b));
    let mut kept: Vec<Edge> = Vec::new();
    for s in &pa {
        let rel = classify(s, &eb, &rb);
        let keep = matches!(
            (op, rel),
            (BoolOp::Union, Rel::Outside | Rel::Same)
                | (BoolOp::Intersect, Rel::Inside | Rel::Same)
                | (BoolOp::Subtract, Rel::Outside | Rel::Opposite)
        );
        if keep {
            kept.push(*s);
        }
    }
    for s in &pb {
        let rel = classify(s, &ea, &ra);
        match (op, rel) {
            (BoolOp::Union, Rel::Outside) | (BoolOp::Intersect, Rel::Inside) => kept.push(*s),
            (BoolOp::Subtract, Rel::Inside) => kept.push(Edge {
                a: s.b,
                b: s.a,
                arc: s.arc,
            }),
            _ => {}
        }
    }
    simplify(chain(kept, &mut snap))
}

/// Folds the operation over several shapes: Union and Intersect over all of
/// them, Subtract the first minus all the others.
pub fn boolean_all(op: BoolOp, shapes: &[Ring]) -> Vec<Ring> {
    let mut it = shapes.iter();
    let Some(first) = it.next() else {
        return Vec::new();
    };
    let mut acc: Vec<Ring> = vec![first.clone()];
    for s in it {
        acc = boolean(op, &acc, std::slice::from_ref(s));
        if acc.is_empty() && op != BoolOp::Union {
            break;
        }
    }
    acc
}

/// Net area of a region (outer rings minus holes).
pub fn region_area(region: &[Ring]) -> f64 {
    region.iter().map(Ring::area).sum()
}

// ----- back to polylines -----

/// One polyline of a result.
#[derive(Debug, Clone, PartialEq)]
pub struct OutPoly {
    /// The points, with the samples of every arc edge.
    pub points: Vec<Point>,
    /// The arc edges (`CadAttrs::arc_edges`).
    pub arcs: Vec<PolyArc>,
    /// A hole of the result (it runs clockwise).
    pub hole: bool,
    /// The whole ring is one circle: a Circle item replaces the polyline.
    pub circle: Option<(Point, f64)>,
}

impl OutPoly {
    /// The CAD item: a closed polyline, or a circle when the ring is one.
    pub fn item(&self) -> CadItem {
        match self.circle {
            Some((center, radius)) => CadItem::Circle { center, radius },
            None => CadItem::Polyline {
                points: self.points.clone(),
                closed: true,
            },
        }
    }
}

/// The sweep of the edge `a` to `b` about `c`, signed (counter-clockwise
/// positive), at most half a turn.
fn edge_sweep(c: Point, a: Point, b: Point) -> f64 {
    let (u, v) = (a.sub(c), b.sub(c));
    u.cross(v).atan2(u.dot(v))
}

/// A ring as a polyline with its arc edges fitted.
pub fn ring_to_poly(r: &Ring) -> OutPoly {
    let n = r.pts.len();
    let hole = r.area() < 0.0;
    // A ring that is all one circle is a circle.
    if let Some(c0) = r.arcs.first().copied().flatten() {
        if r.arcs.iter().all(|t| t.is_some_and(|c| c.same(&c0))) {
            let total: f64 = (0..n)
                .map(|i| edge_sweep(c0.center, r.pts[i], r.pts[(i + 1) % n]))
                .sum();
            if (total.abs() - TAU).abs() < 1e-3 {
                return OutPoly {
                    points: r.pts.clone(),
                    arcs: Vec::new(),
                    hole,
                    circle: Some((c0.center, c0.radius)),
                };
            }
        }
    }
    // Start at an edge that does not continue the run before it.
    let starts_run = |i: usize| {
        let prev = (i + n - 1) % n;
        match (r.arcs[prev], r.arcs[i]) {
            (Some(p), Some(c)) => !p.same(&c),
            _ => true,
        }
    };
    let first = (0..n).find(|i| starts_run(*i)).unwrap_or(0);
    // Control vertices: (point, bulge of the edge leaving it).
    let mut ctrl: Vec<(Point, Option<f64>)> = Vec::new();
    let mut i = 0;
    while i < n {
        let k = (first + i) % n;
        match r.arcs[k] {
            None => {
                ctrl.push((r.pts[k], None));
                i += 1;
            }
            Some(c) => {
                // The run of edges on this circle, with the sweep so far.
                let mut j = i;
                let mut sweeps: Vec<f64> = Vec::new();
                while j < n {
                    let kk = (first + j) % n;
                    if !r.arcs[kk].is_some_and(|t| t.same(&c)) {
                        break;
                    }
                    sweeps.push(edge_sweep(c.center, r.pts[kk], r.pts[(kk + 1) % n]));
                    j += 1;
                }
                let total: f64 = sweeps.iter().sum();
                let limit = 4.0 * MAX_BULGE.atan() * 0.98;
                if total.abs() < 1e-6 {
                    ctrl.push((r.pts[k], None));
                } else if total.abs() <= limit {
                    ctrl.push((r.pts[k], Some((total / 4.0).tan())));
                } else {
                    // Too long for one edge: split at the vertex nearest half.
                    let mut acc = 0.0;
                    let mut split_at = 1;
                    let mut best = f64::MAX;
                    for (m, s) in sweeps.iter().enumerate().take(sweeps.len() - 1) {
                        acc += s;
                        let off = (acc - total / 2.0).abs();
                        if off < best {
                            best = off;
                            split_at = m + 1;
                        }
                    }
                    let first_half: f64 = sweeps[..split_at].iter().sum();
                    ctrl.push((r.pts[k], Some((first_half / 4.0).tan())));
                    ctrl.push((
                        r.pts[(first + i + split_at) % n],
                        Some(((total - first_half) / 4.0).tan()),
                    ));
                }
                i = j;
            }
        }
    }
    // The points: each arc edge is resampled between its control vertices.
    let nc = ctrl.len();
    let mut points: Vec<Point> = Vec::new();
    let mut arcs: Vec<PolyArc> = Vec::new();
    for (idx, (p, bulge)) in ctrl.iter().enumerate() {
        let from = points.len();
        points.push(*p);
        if let Some(b) = bulge {
            let next = ctrl[(idx + 1) % nc].0;
            let cnt = sample_count(*b);
            let samples = arc_samples(*p, next, *b, cnt);
            points.extend(samples[1..cnt].iter().copied());
            arcs.push(PolyArc {
                from,
                to: points.len(),
                bulge: *b,
            });
        }
    }
    OutPoly {
        points,
        arcs,
        hole,
        circle: None,
    }
}

/// The polylines of every ring of a region.
pub fn region_polys(region: &[Ring]) -> Vec<OutPoly> {
    region.iter().map(ring_to_poly).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(x: f64, y: f64, w: f64, h: f64) -> Ring {
        Ring::from_polyline(
            &[
                Point::new(x, y),
                Point::new(x + w, y),
                Point::new(x + w, y + h),
                Point::new(x, y + h),
            ],
            &[],
        )
        .unwrap()
    }

    fn area(region: &[Ring]) -> f64 {
        region_area(region)
    }

    fn run(op: BoolOp, a: &Ring, b: &Ring) -> Vec<Ring> {
        boolean(op, std::slice::from_ref(a), std::slice::from_ref(b))
    }

    #[test]
    fn overlapping_squares_union_intersect_subtract() {
        let (a, b) = (sq(0.0, 0.0, 2.0, 2.0), sq(1.0, 1.0, 2.0, 2.0));
        let u = run(BoolOp::Union, &a, &b);
        assert_eq!(u.len(), 1);
        assert!((area(&u) - 7.0).abs() < 1e-9);
        assert_eq!(u[0].pts.len(), 8);
        let i = run(BoolOp::Intersect, &a, &b);
        assert_eq!(i.len(), 1);
        assert!((area(&i) - 1.0).abs() < 1e-9);
        assert_eq!(i[0].pts.len(), 4);
        let s = run(BoolOp::Subtract, &a, &b);
        assert_eq!(s.len(), 1);
        assert!((area(&s) - 3.0).abs() < 1e-9);
        assert_eq!(s[0].pts.len(), 6);
    }

    #[test]
    fn squares_that_share_a_side_dissolve_the_side() {
        let (a, b) = (sq(0.0, 0.0, 2.0, 2.0), sq(2.0, 0.0, 2.0, 2.0));
        let u = run(BoolOp::Union, &a, &b);
        assert_eq!(u.len(), 1);
        assert!((area(&u) - 8.0).abs() < 1e-9);
        assert_eq!(
            u[0].pts.len(),
            4,
            "the shared side's ends merge into straight runs"
        );
        assert!(run(BoolOp::Intersect, &a, &b).is_empty());
        let s = run(BoolOp::Subtract, &a, &b);
        assert_eq!(s.len(), 1);
        assert!((area(&s) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn partly_shared_sides_and_t_junctions() {
        // B sits on A's right side, shorter than it, so its corner touches A's
        // side in the middle (a T-junction).
        let (a, b) = (sq(0.0, 0.0, 2.0, 4.0), sq(2.0, 1.0, 2.0, 2.0));
        let u = run(BoolOp::Union, &a, &b);
        assert_eq!(u.len(), 1);
        assert!((area(&u) - 12.0).abs() < 1e-9);
        assert_eq!(u[0].pts.len(), 8);
        assert!(run(BoolOp::Intersect, &a, &b).is_empty());
        let s = run(BoolOp::Subtract, &a, &b);
        assert!((area(&s) - 8.0).abs() < 1e-9);
        assert_eq!(s[0].pts.len(), 4);
    }

    #[test]
    fn identical_shapes() {
        let a = sq(0.0, 0.0, 3.0, 3.0);
        let b = a.clone();
        let u = run(BoolOp::Union, &a, &b);
        assert_eq!(u.len(), 1);
        assert!((area(&u) - 9.0).abs() < 1e-9);
        let i = run(BoolOp::Intersect, &a, &b);
        assert!((area(&i) - 9.0).abs() < 1e-9);
        assert!(run(BoolOp::Subtract, &a, &b).is_empty());
    }

    #[test]
    fn a_shape_inside_another_makes_a_hole_or_vanishes() {
        let (big, small) = (sq(0.0, 0.0, 6.0, 6.0), sq(2.0, 2.0, 2.0, 2.0));
        let s = run(BoolOp::Subtract, &big, &small);
        assert_eq!(s.len(), 2);
        assert!((area(&s) - 32.0).abs() < 1e-9);
        let polys = region_polys(&s);
        assert_eq!(polys.iter().filter(|p| p.hole).count(), 1);
        let u = run(BoolOp::Union, &big, &small);
        assert_eq!(u.len(), 1);
        assert!((area(&u) - 36.0).abs() < 1e-9);
        let i = run(BoolOp::Intersect, &big, &small);
        assert!((area(&i) - 4.0).abs() < 1e-9);
        assert!(run(BoolOp::Subtract, &small, &big).is_empty());
    }

    #[test]
    fn a_bar_across_a_shape_splits_it_in_two() {
        let (a, bar) = (sq(0.0, 0.0, 10.0, 4.0), sq(4.0, -1.0, 2.0, 6.0));
        let s = run(BoolOp::Subtract, &a, &bar);
        assert_eq!(s.len(), 2);
        assert!((area(&s) - 32.0).abs() < 1e-9);
    }

    #[test]
    fn squares_that_touch_at_a_corner_stay_two_rings() {
        let (a, b) = (sq(0.0, 0.0, 2.0, 2.0), sq(2.0, 2.0, 2.0, 2.0));
        let u = run(BoolOp::Union, &a, &b);
        assert_eq!(u.len(), 2);
        assert!((area(&u) - 8.0).abs() < 1e-9);
        assert!(run(BoolOp::Intersect, &a, &b).is_empty());
        assert!((area(&run(BoolOp::Subtract, &a, &b)) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn disjoint_shapes() {
        let (a, b) = (sq(0.0, 0.0, 1.0, 1.0), sq(5.0, 5.0, 1.0, 1.0));
        assert_eq!(run(BoolOp::Union, &a, &b).len(), 2);
        assert!(run(BoolOp::Intersect, &a, &b).is_empty());
        assert!((area(&run(BoolOp::Subtract, &a, &b)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn clockwise_input_is_turned_around() {
        let cw = Ring::from_polyline(
            &[
                Point::new(0.0, 0.0),
                Point::new(0.0, 2.0),
                Point::new(2.0, 2.0),
                Point::new(2.0, 0.0),
            ],
            &[],
        )
        .unwrap();
        assert!(cw.area() > 0.0);
        let u = run(BoolOp::Union, &cw, &sq(1.0, 1.0, 2.0, 2.0));
        assert!((area(&u) - 7.0).abs() < 1e-9);
    }

    #[test]
    fn a_concave_shape_against_a_square() {
        // An L shape cut by a square covering its notch.
        let l = Ring::from_polyline(
            &[
                Point::new(0.0, 0.0),
                Point::new(4.0, 0.0),
                Point::new(4.0, 2.0),
                Point::new(2.0, 2.0),
                Point::new(2.0, 4.0),
                Point::new(0.0, 4.0),
            ],
            &[],
        )
        .unwrap();
        let notch = sq(2.0, 2.0, 2.0, 2.0);
        let u = run(BoolOp::Union, &l, &notch);
        assert_eq!(u.len(), 1);
        assert!((area(&u) - 16.0).abs() < 1e-9);
        assert_eq!(
            u[0].pts.len(),
            4,
            "the notch fills in and the corners merge"
        );
        assert!(run(BoolOp::Intersect, &l, &notch).is_empty());
    }

    #[test]
    fn many_shapes_fold() {
        let shapes = [
            sq(0.0, 0.0, 2.0, 2.0),
            sq(1.0, 0.0, 2.0, 2.0),
            sq(2.0, 0.0, 2.0, 2.0),
        ];
        let u = boolean_all(BoolOp::Union, &shapes);
        assert_eq!(u.len(), 1);
        assert!((area(&u) - 8.0).abs() < 1e-9);
        let i = boolean_all(BoolOp::Intersect, &shapes);
        assert!(
            i.is_empty(),
            "the first and last do not meet in the middle one's overlap"
        );
        let s = boolean_all(BoolOp::Subtract, &shapes);
        assert!((area(&s) - 2.0).abs() < 1e-9);
    }

    fn circle_area(r: f64) -> f64 {
        PI * r * r
    }

    /// Does every sample of the arc edges lie on the circle of its ends?
    fn arcs_are_exact(p: &OutPoly) -> bool {
        let n = p.points.len();
        p.arcs.iter().all(|a| {
            let (s, e) = (p.points[a.from], p.points[a.to % n]);
            let Some((c, r)) = arc_center(s, e, a.bulge) else {
                return false;
            };
            (a.from..=a.to).all(|k| (p.points[k % n].dist(c) - r).abs() < 1e-6)
        })
    }

    #[test]
    fn a_circle_cut_by_a_square_keeps_its_arc_exact() {
        let c = Ring::circle(Point::ZERO, 10.0);
        let b = sq(0.0, -20.0, 20.0, 40.0);
        let i = run(BoolOp::Intersect, &c, &b);
        assert_eq!(i.len(), 1);
        let want = circle_area(10.0) / 2.0;
        assert!(
            (area(&i) - want).abs() / want < 0.01,
            "area {} vs {}",
            area(&i),
            want
        );
        let poly = ring_to_poly(&i[0]);
        assert_eq!(poly.arcs.len(), 1, "one arc edge");
        assert!(arcs_are_exact(&poly));
        // The arc's bulge is a half circle: tan(180 / 4 degrees) = 1.
        assert!((poly.arcs[0].bulge.abs() - 1.0).abs() < 1e-6);
        // The straight cut is one edge: the arc's samples plus nothing else.
        assert_eq!(poly.points.len(), poly.arcs[0].to);
    }

    #[test]
    fn circle_and_box_union_and_subtract() {
        let c = Ring::circle(Point::new(0.0, 0.0), 10.0);
        let b = sq(0.0, -5.0, 30.0, 10.0);
        let u = run(BoolOp::Union, &c, &b);
        assert_eq!(u.len(), 1);
        let est = circle_area(10.0) + 300.0 - strip_overlap();
        assert!(
            (area(&u) - est).abs() / est < 0.01,
            "{} vs {}",
            area(&u),
            est
        );
        let poly = ring_to_poly(&u[0]);
        assert!(!poly.arcs.is_empty());
        assert!(arcs_are_exact(&poly));
        let s = run(BoolOp::Subtract, &c, &b);
        assert_eq!(s.len(), 1);
        assert!(
            (area(&s) - (circle_area(10.0) - strip_overlap())).abs() / circle_area(10.0) < 0.01
        );
    }

    /// Area of the circle r=10 inside the strip x>0, |y|<5.
    fn strip_overlap() -> f64 {
        // Numeric integration of the half-height of the circle.
        let n = 20000;
        let mut a = 0.0;
        for k in 0..n {
            let x = (k as f64 + 0.5) * 10.0 / n as f64;
            let h = (100.0 - x * x).sqrt().min(5.0);
            a += 2.0 * h * 10.0 / n as f64;
        }
        a
    }

    #[test]
    fn two_circles_union_intersect_subtract() {
        let (a, b) = (
            Ring::circle(Point::ZERO, 10.0),
            Ring::circle(Point::new(10.0, 0.0), 10.0),
        );
        let lens = 2.0 * 100.0 * (0.5f64).acos() - 0.5 * 10.0 * (300.0f64).sqrt();
        let i = run(BoolOp::Intersect, &a, &b);
        assert_eq!(i.len(), 1);
        assert!(
            (area(&i) - lens).abs() / lens < 0.01,
            "{} vs {}",
            area(&i),
            lens
        );
        let poly = ring_to_poly(&i[0]);
        assert_eq!(poly.arcs.len(), 2, "two arc edges");
        assert!(arcs_are_exact(&poly));
        let u = run(BoolOp::Union, &a, &b);
        assert!((area(&u) - (2.0 * circle_area(10.0) - lens)).abs() / area(&u) < 0.01);
        let s = run(BoolOp::Subtract, &a, &b);
        assert!((area(&s) - (circle_area(10.0) - lens)).abs() / area(&s) < 0.01);
        assert_eq!(region_polys(&s)[0].arcs.len(), 2);
    }

    #[test]
    fn a_circle_inside_a_circle_makes_a_ring_of_two_circles() {
        let (a, b) = (
            Ring::circle(Point::ZERO, 10.0),
            Ring::circle(Point::new(1.0, 0.0), 4.0),
        );
        let s = run(BoolOp::Subtract, &a, &b);
        assert_eq!(s.len(), 2);
        let polys = region_polys(&s);
        assert!(
            polys.iter().all(|p| p.circle.is_some()),
            "both rings are circles again"
        );
        assert_eq!(polys.iter().filter(|p| p.hole).count(), 1);
        let want = circle_area(10.0) - circle_area(4.0);
        assert!((area(&s) - want).abs() / want < 0.01);
    }

    #[test]
    fn a_polyline_with_an_arc_edge_goes_through_a_boolean() {
        // A half disc: the diameter and an arc edge.
        let a = Point::new(-10.0, 0.0);
        let b = Point::new(10.0, 0.0);
        // b -> a counter-clockwise over the top: a half turn, bulge 1.
        let samples = arc_samples(b, a, 1.0, sample_count(1.0));
        let mut pts = vec![b];
        pts.extend(samples[1..samples.len() - 1].iter().copied());
        pts.push(a);
        let n = pts.len();
        let arcs = vec![PolyArc {
            from: 0,
            to: n - 1,
            bulge: 1.0,
        }];
        let half = Ring::from_polyline(&pts, &arcs).unwrap();
        assert!(half.arcs.iter().filter(|t| t.is_some()).count() >= 4);
        let cut = sq(-20.0, 0.0, 40.0, 5.0);
        let i = run(BoolOp::Intersect, &half, &cut);
        assert_eq!(i.len(), 1);
        let poly = ring_to_poly(&i[0]);
        assert_eq!(poly.arcs.len(), 2, "the right and the left end of the band");
        assert!(arcs_are_exact(&poly));
        // Segment of the disc under y = 5: area of the half disc below y=5.
        let want = {
            let r: f64 = 10.0;
            let h: f64 = 5.0;
            // Area of the disc between y=0 and y=h for the upper half.
            0.5 * (h * (r * r - h * h).sqrt() + r * r * (h / r).asin()) * 2.0
        };
        assert!(
            (area(&i) - want).abs() / want < 0.01,
            "{} vs {}",
            area(&i),
            want
        );
    }

    /// A small deterministic generator for the stress tests.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn int(&mut self, n: u64) -> f64 {
            (self.next() % n) as f64
        }
    }

    fn check_identities(a: &Ring, b: &Ring) {
        let (aa, ab) = (a.area(), b.area());
        let u = area(&run(BoolOp::Union, a, b));
        let i = area(&run(BoolOp::Intersect, a, b));
        let s = area(&run(BoolOp::Subtract, a, b));
        let t = area(&run(BoolOp::Subtract, b, a));
        assert!(
            (u + i - aa - ab).abs() < 1e-6,
            "inclusion-exclusion: {a:?} {b:?}"
        );
        assert!((s - (aa - i)).abs() < 1e-6, "A minus B: {a:?} {b:?}");
        assert!((t - (ab - i)).abs() < 1e-6, "B minus A: {a:?} {b:?}");
    }

    #[test]
    fn grid_rectangles_with_every_kind_of_touching_obey_the_area_identities() {
        let mut g = Lcg(7);
        for _ in 0..400 {
            let rect = |g: &mut Lcg| {
                let (x, y) = (g.int(6), g.int(6));
                sq(x, y, 1.0 + g.int(4), 1.0 + g.int(4))
            };
            let (a, b) = (rect(&mut g), rect(&mut g));
            check_identities(&a, &b);
        }
    }

    #[test]
    fn rotated_squares_and_circles_obey_the_area_identities() {
        let mut g = Lcg(11);
        for _ in 0..200 {
            let poly = |g: &mut Lcg| {
                let (cx, cy) = (g.int(8) * 0.7, g.int(8) * 0.7);
                let ang = g.int(90) * 1.0f64.to_radians();
                let r = 2.0 + g.int(4);
                let pts: Vec<Point> = (0..4)
                    .map(|k| {
                        let a = ang + k as f64 * PI / 2.0;
                        Point::new(cx + r * a.cos(), cy + r * a.sin())
                    })
                    .collect();
                Ring::from_polyline(&pts, &[]).unwrap()
            };
            let (a, b) = (poly(&mut g), poly(&mut g));
            check_identities(&a, &b);
        }
    }

    #[test]
    fn polygons_against_circles_obey_the_area_identities_within_the_arc_error() {
        let mut g = Lcg(5);
        for _ in 0..100 {
            let c = Ring::circle(Point::new(g.int(6) * 1.3, g.int(6) * 1.3), 2.0 + g.int(3));
            let b = sq(
                g.int(6) * 1.1,
                g.int(6) * 1.1,
                1.0 + g.int(5),
                1.0 + g.int(5),
            );
            let (ac, ab) = (c.area(), b.area());
            let u = area(&run(BoolOp::Union, &c, &b));
            let i = area(&run(BoolOp::Intersect, &c, &b));
            let s = area(&run(BoolOp::Subtract, &c, &b));
            let tol = 0.02 * ac;
            assert!((u + i - ac - ab).abs() < tol, "union/intersect");
            assert!((s - (ac - i)).abs() < tol, "subtract");
        }
    }

    #[test]
    fn results_are_closed_valid_polylines() {
        let a = Ring::circle(Point::ZERO, 10.0);
        let b = sq(-3.0, -30.0, 6.0, 60.0);
        let s = run(BoolOp::Subtract, &a, &b);
        assert_eq!(s.len(), 2, "the bar splits the disc into two");
        for p in region_polys(&s) {
            assert!(p.points.len() >= 5);
            assert!(arcs_are_exact(&p));
            for ar in &p.arcs {
                assert!(ar.to <= p.points.len());
            }
        }
    }
}
