//! Weighted straight skeleton of a simple polygon.
//!
//! The polygon boundary is treated as a *wavefront* that moves inward as
//! "height" `t` increases. Edge `i` moves with horizontal speed `s_i`
//! (inches of inset per inch of rise), so its roof plane climbs at pitch
//! `12 / s_i` in 12. A speed of zero means the edge does not move at all: a
//! vertical wall (gable end) whose neighbouring vertices simply slide along it.
//!
//! The wavefront is a set of cyclic *loops* of vertices. Each vertex moves with
//! the unique velocity that keeps it on the offset lines of both its incident
//! edges. The simulation repeatedly finds the earliest event and applies it:
//!
//! * **edge event**: an edge shrinks to zero length; its two vertices merge;
//! * **split event**: a reflex vertex runs into a wavefront edge and splits the
//!   loop in two.
//!
//! A loop with two vertices left is degenerate (its two edges coincide), which
//! terminates it with one final skeleton segment (the ridge). The traced
//! vertex paths are the skeleton arcs; every arc is recorded on the faces of
//! both original edges it separates, so adjacent faces share their nodes
//! exactly and the resulting roof is watertight by construction.

use plan_core::Point;
use std::collections::{HashMap, HashSet};

/// Below this a velocity/length derivative counts as zero.
const TINY: f64 = 1e-9;

/// A skeleton node: plan position at height `t` above the eave.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Node {
    pub pos: Point,
    pub t: f64,
}

/// A fixed wavefront edge line: `n . p = c + s * t`.
struct Line {
    /// Unit inward normal.
    n: Point,
    /// Unit direction (start to end of the original edge).
    d: Point,
    /// Horizontal speed (inset per unit height).
    s: f64,
    /// `n . P0` at `t = 0`.
    c: f64,
}

/// A moving wavefront vertex between two edges.
#[derive(Clone)]
struct Vtx {
    /// Position at `t0`.
    p: Point,
    t0: f64,
    vel: Point,
    /// Node where this vertex started.
    node: usize,
    /// Edge arriving at the vertex and edge leaving it.
    prev: usize,
    next: usize,
    reflex: bool,
}

impl Vtx {
    fn at(&self, t: f64) -> Point {
        self.p + self.vel * (t - self.t0)
    }
}

enum Ev {
    Edge { k: usize },
    Split { i: usize, k: usize },
}

struct Cand {
    t: f64,
    ev: Ev,
}

/// The finished skeleton: nodes plus, per original edge, the segments that
/// bound its roof face.
pub(crate) struct Skeleton {
    pub nodes: Vec<Node>,
    faces: Vec<Vec<(usize, usize)>>,
    n: usize,
}

struct Sim {
    lines: Vec<Line>,
    nodes: Vec<Node>,
    faces: Vec<Vec<(usize, usize)>>,
    eps: f64,
}

/// Velocity of the vertex shared by edges `a` and `b`.
fn velocity(a: &Line, b: &Line) -> Point {
    let det = a.n.cross(b.n);
    if det.abs() < 1e-9 {
        // Collinear (same side) or coincident antiparallel edges.
        return if a.n.dot(b.n) > 0.0 {
            a.n * ((a.s + b.s) * 0.5)
        } else {
            Point::ZERO
        };
    }
    Point::new(
        (a.s * b.n.y - a.n.y * b.s) / det,
        (a.n.x * b.s - a.s * b.n.x) / det,
    )
}

/// Compute the weighted straight skeleton of a CCW simple polygon.
///
/// `speeds[i]` is the horizontal inset speed of edge `(poly[i], poly[i+1])`.
/// Returns `None` if the simulation cannot complete (self-intersecting input,
/// numerically hopeless degeneracy); callers fall back to an approximation.
pub(crate) fn compute(poly: &[Point], speeds: &[f64]) -> Option<Skeleton> {
    let n = poly.len();
    if n < 3 || speeds.len() != n {
        return None;
    }
    let (mut lo, mut hi) = (poly[0], poly[0]);
    for p in poly {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let extent = (hi.x - lo.x).max(hi.y - lo.y).max(1.0);
    let eps = 1e-6 * extent;

    let lines: Vec<Line> = (0..n)
        .map(|k| {
            let d = poly[(k + 1) % n].sub(poly[k]).normalized();
            let nrm = d.perp();
            Line {
                n: nrm,
                d,
                s: speeds[k].max(0.0),
                c: nrm.dot(poly[k]),
            }
        })
        .collect();
    if lines.iter().any(|l| l.d == Point::ZERO) {
        return None;
    }

    let nodes: Vec<Node> = poly.iter().map(|&pos| Node { pos, t: 0.0 }).collect();
    let mut sim = Sim {
        lines,
        nodes,
        faces: vec![Vec::new(); n],
        eps,
    };
    let initial: Vec<Vtx> = (0..n)
        .map(|k| sim.make_vertex(poly[k], 0.0, k, (k + n - 1) % n, k))
        .collect();
    if !sim.run(initial) {
        return None;
    }
    Some(Skeleton {
        nodes: sim.nodes,
        faces: sim.faces,
        n,
    })
}

impl Sim {
    fn make_vertex(&self, p: Point, t: f64, node: usize, prev: usize, next: usize) -> Vtx {
        let (a, b) = (&self.lines[prev], &self.lines[next]);
        Vtx {
            p,
            t0: t,
            vel: velocity(a, b),
            node,
            prev,
            next,
            reflex: a.d.cross(b.d) < -1e-9,
        }
    }

    /// Find or create the node at (`p`, `t`), merging coincident nodes.
    fn node_at(&mut self, p: Point, t: f64) -> usize {
        let eps = self.eps;
        if let Some(i) = self
            .nodes
            .iter()
            .position(|n| (n.t - t).abs() <= eps && n.pos.dist(p) <= eps)
        {
            return i;
        }
        self.nodes.push(Node { pos: p, t });
        self.nodes.len() - 1
    }

    /// Record the arc travelled by `v` (ending at `end`) on both its faces.
    fn add_arc(&mut self, v: &Vtx, end: usize) {
        if v.node != end {
            self.faces[v.prev].push((v.node, end));
            self.faces[v.next].push((v.node, end));
        }
    }

    fn run(&mut self, initial: Vec<Vtx>) -> bool {
        let n = initial.len();
        let max_iter = 64 * (n + 8) * (n + 8);
        let mut iter = 0usize;
        let mut stack = vec![(initial, 0.0f64)];
        while let Some((mut lp, mut t)) = stack.pop() {
            loop {
                iter += 1;
                if iter > max_iter {
                    return false;
                }
                if lp.len() < 2 {
                    break;
                }
                if self.is_flat(&lp, t) {
                    self.terminate(&lp, t);
                    break;
                }
                let Some(c) = self.next_event(&lp, t) else {
                    return false;
                };
                t = c.t.max(t);
                match c.ev {
                    Ev::Edge { k } => self.edge_event(&mut lp, k, t),
                    Ev::Split { i, k } => {
                        let (l1, l2) = self.split_event(&lp, i, k, t);
                        stack.push((l2, t));
                        lp = l1;
                    }
                }
            }
        }
        true
    }

    /// Earliest event of a loop at or after time `t`.
    fn next_event(&self, lp: &[Vtx], t: f64) -> Option<Cand> {
        let n = lp.len();
        let eps = self.eps;
        let mut best: Option<Cand> = None;
        let better = |c: &Cand, best: &Option<Cand>| match best {
            None => true,
            Some(b) => {
                c.t < b.t - eps
                    || ((c.t - b.t).abs() <= eps
                        && matches!(c.ev, Ev::Edge { .. })
                        && matches!(b.ev, Ev::Split { .. }))
            }
        };

        for k in 0..n {
            let (a, b) = (&lp[k], &lp[(k + 1) % n]);
            let d = self.lines[a.next].d;
            let len = b.at(t).sub(a.at(t)).dot(d);
            let dl = b.vel.sub(a.vel).dot(d);
            let dt = if dl < -TINY {
                len.max(0.0) / -dl
            } else if len <= eps {
                0.0
            } else {
                continue;
            };
            let c = Cand {
                t: t + dt,
                ev: Ev::Edge { k },
            };
            if better(&c, &best) {
                best = Some(c);
            }
        }

        for (i, v) in lp.iter().enumerate() {
            if !v.reflex {
                continue;
            }
            let pv = v.at(t);
            for k in 0..n {
                if k == i || (k + 1) % n == i {
                    continue;
                }
                let (a, b) = (&lp[k], &lp[(k + 1) % n]);
                let ln = &self.lines[a.next];
                let dist = ln.n.dot(pv) - (ln.c + ln.s * t);
                if dist < -eps {
                    continue;
                }
                let denom = ln.n.dot(v.vel) - ln.s;
                if denom > -TINY {
                    continue;
                }
                let th = t + dist.max(0.0) / -denom;
                let h = v.at(th);
                let (pa, pb) = (a.at(th), b.at(th));
                let u = h.sub(pa).dot(ln.d);
                let len = pb.sub(pa).dot(ln.d);
                if len < -eps || u < -eps || u > len + eps {
                    continue;
                }
                let c = Cand {
                    t: th,
                    ev: Ev::Split { i, k },
                };
                if better(&c, &best) {
                    best = Some(c);
                }
            }
        }
        best
    }

    /// Edge `k` (between `lp[k]` and `lp[k+1]`) collapses.
    fn edge_event(&mut self, lp: &mut Vec<Vtx>, k: usize, t: f64) {
        lp.rotate_left(k);
        let (a, b) = (lp[0].clone(), lp[1].clone());
        let p = Point::lerp(a.at(t), b.at(t), 0.5);
        let node = self.node_at(p, t);
        self.add_arc(&a, node);
        self.add_arc(&b, node);
        let v = self.make_vertex(p, t, node, a.prev, b.next);
        lp.remove(1);
        lp[0] = v;
    }

    /// Reflex vertex `lp[i]` hits edge `k`; the loop splits in two.
    fn split_event(&mut self, lp: &[Vtx], i: usize, k: usize, t: f64) -> (Vec<Vtx>, Vec<Vtx>) {
        let n = lp.len();
        let v = lp[i].clone();
        let e = lp[k].next;
        let p = v.at(t);
        let node = self.node_at(p, t);
        self.add_arc(&v, node);

        let cyc = |from: usize, to: usize| {
            let mut out = Vec::new();
            let mut j = from;
            loop {
                out.push(lp[j % n].clone());
                if j % n == to {
                    break;
                }
                j += 1;
            }
            out
        };
        // Loop 1: e -> N' -(b)-> V.next ... -> lp[k] -(e)-> N'.
        let mut l1 = vec![self.make_vertex(p, t, node, e, v.next)];
        l1.extend(cyc((i + 1) % n, k));
        // Loop 2: N'' -(e)-> lp[k+1] ... -> V.prev -(a)-> N''.
        let mut l2 = vec![self.make_vertex(p, t, node, v.prev, e)];
        l2.extend(cyc((k + 1) % n, (i + n - 1) % n));
        (l1, l2)
    }

    /// `true` when every vertex of the loop lies on one line (zero area): the
    /// wavefront has collapsed. Always true for a two-vertex loop.
    fn is_flat(&self, lp: &[Vtx], t: f64) -> bool {
        let pos: Vec<Point> = lp.iter().map(|v| v.at(t)).collect();
        let (mut a, mut b, mut far) = (pos[0], pos[0], 0.0);
        for p in &pos {
            for q in &pos {
                if p.dist(*q) > far {
                    (a, b, far) = (*p, *q, p.dist(*q));
                }
            }
        }
        if far <= self.eps {
            return true;
        }
        let axis = b.sub(a).normalized();
        pos.iter().all(|p| p.sub(a).cross(axis).abs() <= self.eps)
    }

    /// Close a collapsed (flat) loop. Its edges now coincide pairwise; the
    /// vertices are joined by ridge segments, each added to the faces of the
    /// loop edges that cover that stretch of the line.
    fn terminate(&mut self, lp: &[Vtx], t: f64) {
        let n = lp.len();
        let ids: Vec<usize> = lp
            .iter()
            .map(|v| {
                let id = self.node_at(v.at(t), t);
                self.add_arc(v, id);
                id
            })
            .collect();
        // Parameter of every vertex along the line through the farthest pair.
        let pos: Vec<Point> = ids.iter().map(|&i| self.nodes[i].pos).collect();
        let (mut a, mut b, mut far) = (pos[0], pos[0], 0.0);
        for p in &pos {
            for q in &pos {
                if p.dist(*q) > far {
                    (a, b, far) = (*p, *q, p.dist(*q));
                }
            }
        }
        if far <= self.eps {
            return;
        }
        let axis = b.sub(a).normalized();
        let s: Vec<f64> = pos.iter().map(|p| p.sub(a).dot(axis)).collect();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&i, &j| s[i].partial_cmp(&s[j]).unwrap_or(std::cmp::Ordering::Equal));
        order.dedup_by(|j, i| ids[*i] == ids[*j]);
        for w in order.windows(2) {
            let (lo, hi) = (s[w[0]], s[w[1]]);
            let seg = (ids[w[0]], ids[w[1]]);
            for m in 0..n {
                let (u, v) = (s[m], s[(m + 1) % n]);
                if (u - v).abs() > self.eps
                    && u.min(v) <= lo + self.eps
                    && u.max(v) >= hi - self.eps
                {
                    self.faces[lp[m].next].push(seg);
                }
            }
        }
    }
}

impl Skeleton {
    /// Largest node height.
    pub fn height(&self) -> f64 {
        self.nodes.iter().map(|n| n.t).fold(0.0, f64::max)
    }

    /// The roof face of original edge `i` as a cycle of node ids starting with
    /// the edge's own two eave nodes (counter-clockwise in plan).
    pub fn face(&self, i: usize) -> Option<Vec<usize>> {
        let start = i;
        let second = (i + 1) % self.n;
        let mut segs: HashSet<(usize, usize)> = HashSet::new();
        for &(a, b) in &self.faces[i] {
            segs.insert((a.min(b), a.max(b)));
        }
        segs.insert((start.min(second), start.max(second)));

        let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
        for &(a, b) in &segs {
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default().push(a);
        }
        for list in adj.values_mut() {
            list.sort_unstable();
        }

        let mut path = vec![start, second];
        let (mut prev, mut cur) = (start, second);
        loop {
            let next: Vec<usize> = adj[&cur].iter().copied().filter(|&x| x != prev).collect();
            if path.len() >= 3 && next.contains(&start) {
                break;
            }
            let step = next.into_iter().find(|x| !path.contains(x))?;
            path.push(step);
            prev = cur;
            cur = step;
        }
        (path.len() == adj.len()).then_some(path)
    }
}
