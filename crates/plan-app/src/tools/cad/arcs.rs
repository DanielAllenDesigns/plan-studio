//! Polyline arc edges, Delete Break and Make Arc Tangent (CAD-21, CAD-22,
//! CAD-24).
//!
//! A polyline edge can be an arc. The model keeps it as plain points: the
//! points between the two ends of the edge are samples along the arc (every
//! 7.5 degrees), and `CadAttrs::arc_edges` remembers which spans are arcs and
//! their bulge, so drawing, DXF and layout see a smooth curve with no help.
//! A span whose samples no longer lie on its arc (a sample vertex was dragged,
//! the polyline was mirrored) is forgotten when next read: the curve stays as
//! drawn, it just cannot be toggled back to a single edge.
//!
//! [`Logical`] is the polyline seen as its control vertices, each with the
//! bulge of the edge leaving it; every edit works on that and rebuilds the
//! sampled points.
//!
//! * Change Line/Arc toggles the edge nearest the click (a bulge toward the
//!   click when it becomes an arc); a plain Line becomes an Arc object and
//!   back (CAD-22);
//! * dragging the diamond handle on an arc edge of a selected polyline sets
//!   its bulge (the Select tool does that, with [`Logical::set_bulge`]);
//! * Delete Break removes the vertex nearest the click, the two edges around
//!   it becoming one straight edge (CAD-21);
//! * Make Arc Tangent turns an arc edge (or an Arc object) so it leaves its
//!   neighbour at the shared end without a corner (CAD-24).

use super::*;
use plan_core::cad::{CadAttrs, PolyArc};

/// Bulge of a quarter circle: `tan(90 deg / 4)`.
pub const DEFAULT_BULGE: f64 = 0.414_213_562_373_095;
/// Largest bulge a drag may set (a sweep of about 345 degrees).
pub const MAX_BULGE: f64 = 8.0;
/// Angle between the samples of an arc edge, degrees.
const SAMPLE_DEG: f64 = 7.5;
/// How far a sample may stray from its arc before the span is forgotten.
const SAMPLE_TOL: f64 = 0.5;

/// The sweep of an edge with `bulge`, radians, counter-clockwise positive.
pub fn sweep_of(bulge: f64) -> f64 {
    4.0 * bulge.atan()
}

/// How many segments an arc edge with `bulge` is cut into.
pub fn sample_count(bulge: f64) -> usize {
    ((sweep_of(bulge).abs().to_degrees() / SAMPLE_DEG).ceil() as usize).clamp(4, 96)
}

/// Center and radius of the arc edge from `a` to `b` with `bulge`.
pub fn arc_center(a: Point, b: Point, bulge: f64) -> Option<(Point, f64)> {
    let chord = b.sub(a);
    let l = chord.length();
    let theta = sweep_of(bulge);
    if l < 1e-9 || theta.abs() < 1e-9 {
        return None;
    }
    let left = chord.normalized().perp();
    let center = Point::lerp(a, b, 0.5).add(left.scale(l * 0.5 / (theta * 0.5).tan()));
    Some((center, l / (2.0 * (theta.abs() * 0.5).sin())))
}

/// `n + 1` points along the arc edge from `a` to `b`, ends included.
pub fn arc_samples(a: Point, b: Point, bulge: f64, n: usize) -> Vec<Point> {
    let (Some((c, r)), true) = (arc_center(a, b, bulge), n >= 2) else {
        return vec![a, b];
    };
    let theta = sweep_of(bulge);
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

/// The direction of travel at the start (`at_end` false) or the end of the arc
/// edge `a` to `b` with `bulge`.
pub fn edge_tangent(a: Point, b: Point, bulge: f64, at_end: bool) -> Point {
    let c = b.sub(a).normalized();
    let half = sweep_of(bulge) * 0.5;
    let ang = if at_end { half } else { -half };
    let (s, co) = ang.sin_cos();
    Point::new(c.x * co - c.y * s, c.x * s + c.y * co)
}

/// Does `points[from..=to]` follow the arc edge with `bulge`?
fn span_matches(points: &[Point], span: &PolyArc) -> bool {
    let n = points.len();
    let (f, t) = (span.from, span.to);
    if f + 2 > t || t > n || f >= n {
        return false;
    }
    let want = arc_samples(points[f], points[t % n], span.bulge, t - f);
    (f + 1..t).all(|k| want[k - f].dist(points[k % n]) <= SAMPLE_TOL)
}

/// The spans of `arcs` that still describe `points`.
pub fn valid_arcs(points: &[Point], closed: bool, arcs: &[PolyArc]) -> Vec<PolyArc> {
    let n = points.len();
    let mut spans: Vec<PolyArc> = arcs
        .iter()
        .copied()
        .filter(|a| (a.to < n || (a.to == n && closed)) && span_matches(points, a))
        .collect();
    spans.sort_by_key(|a| a.from);
    let mut out: Vec<PolyArc> = Vec::new();
    for s in spans {
        if out.last().is_none_or(|l| l.to <= s.from) {
            out.push(s);
        }
    }
    out
}

/// A polyline as its control vertices; `bulge[i]` belongs to the edge from
/// vertex `i` to the next (the last one only on a closed polyline).
#[derive(Clone, Debug, PartialEq)]
pub struct Logical {
    pub pts: Vec<Point>,
    pub bulge: Vec<Option<f64>>,
    pub closed: bool,
}

impl Logical {
    /// The control vertices of `points`, with the arc edges `arcs` found.
    pub fn decompose(points: &[Point], closed: bool, arcs: &[PolyArc]) -> Logical {
        let spans = valid_arcs(points, closed, arcs);
        let n = points.len();
        let (mut pts, mut bulge) = (Vec::new(), Vec::new());
        let mut i = 0;
        while i < n {
            pts.push(points[i]);
            match spans.iter().find(|s| s.from == i) {
                Some(s) => {
                    bulge.push(Some(s.bulge));
                    i = s.to;
                }
                None => {
                    bulge.push(None);
                    i += 1;
                }
            }
        }
        Logical { pts, bulge, closed }
    }

    /// The sampled points and the arc spans.
    pub fn rebuild(&self) -> (Vec<Point>, Vec<PolyArc>) {
        let nv = self.pts.len();
        let (mut out, mut arcs) = (Vec::new(), Vec::new());
        for i in 0..nv {
            let from = out.len();
            out.push(self.pts[i]);
            let has_edge = self.closed || i + 1 < nv;
            if let (true, Some(b)) = (has_edge, self.bulge[i]) {
                let next = self.pts[(i + 1) % nv];
                let n = sample_count(b);
                let samples = arc_samples(self.pts[i], next, b, n);
                out.extend(samples[1..n].iter().copied());
                arcs.push(PolyArc {
                    from,
                    to: out.len(),
                    bulge: b,
                });
            }
        }
        (out, arcs)
    }

    pub fn edge_count(&self) -> usize {
        if self.closed {
            self.pts.len()
        } else {
            self.pts.len().saturating_sub(1)
        }
    }

    fn end_of(&self, edge: usize) -> usize {
        (edge + 1) % self.pts.len()
    }

    /// The points that draw edge `i`.
    pub fn edge_path(&self, i: usize) -> Vec<Point> {
        let (a, b) = (self.pts[i], self.pts[self.end_of(i)]);
        match self.bulge[i] {
            Some(bu) => arc_samples(a, b, bu, sample_count(bu)),
            None => vec![a, b],
        }
    }

    /// The edge nearest `p` and its distance.
    pub fn nearest_edge(&self, p: Point) -> Option<(usize, f64)> {
        (0..self.edge_count())
            .map(|i| {
                let path = self.edge_path(i);
                let d = path
                    .windows(2)
                    .map(|w| {
                        plan_core::geometry::project_on_segment(p, w[0], w[1])
                            .1
                            .dist(p)
                    })
                    .fold(f64::MAX, f64::min);
                (i, d)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// The vertex nearest `p` and its distance.
    pub fn nearest_vertex(&self, p: Point) -> Option<(usize, f64)> {
        self.pts
            .iter()
            .enumerate()
            .map(|(i, v)| (i, v.dist(p)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Edge `i` becomes straight, or an arc bulging toward `toward`.
    pub fn toggle_edge(&mut self, i: usize, toward: Point) -> Result<(), &'static str> {
        if i >= self.edge_count() {
            return Err("There is no edge there");
        }
        self.bulge[i] = match self.bulge[i] {
            Some(_) => None,
            None => {
                let (a, b) = (self.pts[i], self.pts[self.end_of(i)]);
                if a.dist(b) < 1.0 {
                    return Err("That edge is too short to curve");
                }
                // The arc bulges to the click's side; a counter-clockwise
                // edge bulges to the right of its chord.
                let left = b.sub(a).cross(toward.sub(a)) > 0.0;
                Some(if left { -DEFAULT_BULGE } else { DEFAULT_BULGE })
            }
        };
        Ok(())
    }

    /// Sets the bulge of arc edge `i` so its apex is `apex` away from the
    /// chord, on the side `side` (a point) is on.
    pub fn set_bulge_through(&mut self, i: usize, through: Point) -> Result<(), &'static str> {
        if i >= self.edge_count() || self.bulge[i].is_none() {
            return Err("That edge is not an arc");
        }
        let (a, b) = (self.pts[i], self.pts[self.end_of(i)]);
        let l = a.dist(b);
        if l < 1e-6 {
            return Err("That edge is too short");
        }
        let right = b.sub(a).normalized().perp().scale(-1.0);
        let sagitta = through.sub(Point::lerp(a, b, 0.5)).dot(right);
        let bu = (2.0 * sagitta / l).clamp(-MAX_BULGE, MAX_BULGE);
        // A bulge near zero would be a straight edge: keep a visible arc.
        self.bulge[i] = Some(if bu.abs() < 0.02 {
            0.02f64.copysign(bu)
        } else {
            bu
        });
        Ok(())
    }

    /// Removes vertex `i`; the edges around it become one straight edge.
    pub fn delete_vertex(&mut self, i: usize) -> Result<(), &'static str> {
        let min = if self.closed { 3 } else { 2 };
        if self.pts.len() <= min || i >= self.pts.len() {
            return Err("A polyline needs at least two vertices (a closed one three)");
        }
        let nv = self.pts.len();
        if self.closed || (i > 0 && i + 1 < nv) {
            let prev = (i + nv - 1) % nv;
            self.bulge[prev] = None;
        }
        self.pts.remove(i);
        self.bulge.remove(i);
        if !self.closed {
            // The last vertex of an open polyline has no edge.
            if let Some(last) = self.bulge.last_mut() {
                *last = None;
            }
        }
        Ok(())
    }

    /// Turns arc edge `i` so it leaves the edge before it (or, at the start of
    /// an open polyline, meets the edge after it) with no corner.
    pub fn make_tangent(&mut self, i: usize) -> Result<(), &'static str> {
        if i >= self.edge_count() || self.bulge[i].is_none() {
            return Err("That edge is not an arc");
        }
        let nv = self.pts.len();
        let (a, b) = (self.pts[i], self.pts[self.end_of(i)]);
        let chord = b.sub(a);
        if chord.length() < 1e-6 {
            return Err("That edge is too short");
        }
        let signed = |from: Point, to: Point| from.cross(to).atan2(from.dot(to));
        // The tangent-chord angle `phi` is half the sweep.
        let phi = if i > 0 || self.closed {
            // From the direction of travel into the shared start vertex to
            // the chord.
            let prev = (i + nv - 1) % nv;
            let pa = self.pts[prev];
            let t = match self.bulge[prev] {
                Some(pb) => edge_tangent(pa, a, pb, true),
                None => a.sub(pa).normalized(),
            };
            signed(t, chord)
        } else if i + 1 < self.edge_count() {
            // The first edge of an open polyline: from the chord to the
            // direction of travel out of the end vertex.
            let nxt = i + 1;
            let nb = self.pts[nxt + 1];
            let t = match self.bulge[nxt] {
                Some(nbu) => edge_tangent(b, nb, nbu, false),
                None => nb.sub(b).normalized(),
            };
            signed(chord, t)
        } else {
            return Err("There is no neighbouring edge to be tangent to");
        };
        if phi.abs() < 0.01 || phi.abs() > 3.0 {
            return Err("That arc cannot be tangent to its neighbour");
        }
        self.bulge[i] = Some((phi * 0.5).tan());
        Ok(())
    }
}

/// An Arc object turned so it leaves `from` (one of its ends) in direction
/// `dir`, keeping its other end. `from_is_start`: `from` is the arc's
/// start-angle end.
pub fn arc_tangent_at(arc: &CadItem, from_is_start: bool, dir: Point) -> Option<CadItem> {
    let CadItem::Arc {
        center,
        radius,
        start_angle,
        end_angle,
    } = arc
    else {
        return None;
    };
    let at = |a: f64| Point::new(center.x + radius * a.cos(), center.y + radius * a.sin());
    let (shared, other) = if from_is_start {
        (at(*start_angle), at(*end_angle))
    } else {
        (at(*end_angle), at(*start_angle))
    };
    arc_start_end_tangent(shared, other, shared.add(dir.normalized().scale(10.0)))
}

// ----- the tool's click modes -----

/// The sampled points, closed flag and arc spans of a polyline object.
fn polyline_parts(cx: &EditorContext, id: Id) -> Option<(Vec<Point>, bool, Vec<PolyArc>)> {
    let c = cad_by_id(cx.floor(), id)?;
    let CadItem::Polyline { points, closed } = &c.item else {
        return None;
    };
    let arcs = cx
        .floor()
        .cad_attrs(id)
        .map(|a| a.arc_edges)
        .unwrap_or_default();
    Some((points.clone(), *closed, arcs))
}

/// Writes a rebuilt polyline back, with its arc spans.
fn store_polyline(cx: &mut EditorContext, id: Id, l: &Logical) {
    let (points, arcs) = l.rebuild();
    let fl = cx.floor;
    if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
        c.item = CadItem::Polyline {
            points,
            closed: l.closed,
        };
    }
    let edges = l.edge_count();
    cx.project.edit_cad_attrs(fl, id, |a: &mut CadAttrs| {
        a.arc_edges = arcs;
        // Edges that no longer exist cannot stay hidden.
        a.hidden_edges.retain(|e| *e < edges);
    });
}

/// Replaces the polyline `id` by `l` as one undo step; used by the Select
/// tool's arc handle too.
pub fn replace_polyline(cx: &mut EditorContext, id: Id, l: &Logical) {
    store_polyline(cx, id, l);
    cx.mark_dirty();
}

/// The control vertices of polyline `id` (with its remembered arcs).
pub fn logical_of(cx: &EditorContext, id: Id) -> Option<Logical> {
    let (points, closed, arcs) = polyline_parts(cx, id)?;
    Some(Logical::decompose(&points, closed, &arcs))
}

/// The arc handles of polyline `id`: `(edge, apex)`.
pub fn arc_handles(cx: &EditorContext, id: Id) -> Vec<(usize, Point)> {
    let Some(l) = logical_of(cx, id) else {
        return Vec::new();
    };
    (0..l.edge_count())
        .filter(|i| l.bulge[*i].is_some())
        .map(|i| {
            let path = l.edge_path(i);
            (i, path[path.len() / 2])
        })
        .collect()
}

impl CadTool {
    /// The drawn CAD object (not text) under the pointer.
    fn arc_target(&self, cx: &EditorContext, p: &PointerEvent) -> Option<plan_core::CadObject> {
        let tol = cx.pick_tol();
        hit_test(cx.floor(), cx.layers(), p.world, tol)
            .into_iter()
            .find_map(|o| match o {
                ObjectRef::Cad(id) => cad_by_id(cx.floor(), id)
                    .filter(|c| !matches!(c.item, CadItem::Text { .. }))
                    .cloned(),
                _ => None,
            })
    }

    fn arc_done(cx: &mut EditorContext, label: &str) -> ToolResult {
        cx.mark_dirty();
        cx.status.clear();
        ToolResult::committed(label)
    }

    /// Change Line/Arc (CAD-22).
    pub(super) fn change_arc_click(
        &mut self,
        cx: &mut EditorContext,
        p: &PointerEvent,
    ) -> ToolResult {
        let label = "Change Line/Arc";
        let Some(obj) = self.arc_target(cx, p) else {
            cx.status = "Change Line/Arc: click a line, an arc or a polyline edge".into();
            return ToolResult::consumed();
        };
        if !cx.check_unlocked(ObjectRef::Cad(obj.id)) {
            return ToolResult::consumed();
        }
        let fl = cx.floor;
        match &obj.item {
            CadItem::Polyline { .. } => {
                let Some(mut l) = logical_of(cx, obj.id) else {
                    return ToolResult::consumed();
                };
                let Some((edge, _)) = l.nearest_edge(p.world) else {
                    return ToolResult::consumed();
                };
                if let Err(e) = l.toggle_edge(edge, p.world) {
                    cx.status = format!("{label}: {e}");
                    return ToolResult::consumed();
                }
                cx.begin_change(label);
                store_polyline(cx, obj.id, &l);
            }
            CadItem::Line { a, b } => {
                let (a, b) = (*a, *b);
                if a.dist(b) < 1.0 {
                    return ToolResult::consumed();
                }
                let left = b.sub(a).cross(p.world.sub(a)) > 0.0;
                let bulge = if left { -DEFAULT_BULGE } else { DEFAULT_BULGE };
                let Some((center, radius)) = arc_center(a, b, bulge) else {
                    return ToolResult::consumed();
                };
                let (sa, ea) = (a.sub(center).angle(), b.sub(center).angle());
                let (start_angle, end_angle) = if bulge > 0.0 { (sa, ea) } else { (ea, sa) };
                cx.begin_change(label);
                if let Some(c) = cx.project.floors[fl]
                    .cad
                    .iter_mut()
                    .find(|c| c.id == obj.id)
                {
                    c.item = CadItem::Arc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                    };
                }
            }
            CadItem::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let at =
                    |a: f64| Point::new(center.x + radius * a.cos(), center.y + radius * a.sin());
                cx.begin_change(label);
                if let Some(c) = cx.project.floors[fl]
                    .cad
                    .iter_mut()
                    .find(|c| c.id == obj.id)
                {
                    c.item = CadItem::Line {
                        a: at(*start_angle),
                        b: at(*end_angle),
                    };
                }
            }
            _ => {
                cx.status = format!("{label}: a circle has no edge to change");
                return ToolResult::consumed();
            }
        }
        Self::arc_done(cx, label)
    }

    /// Delete Break (CAD-21): the vertex nearest the click goes.
    pub(super) fn delete_break_click(
        &mut self,
        cx: &mut EditorContext,
        p: &PointerEvent,
    ) -> ToolResult {
        let label = "Delete Break";
        let Some(obj) = self.arc_target(cx, p) else {
            cx.status = "Delete Break: click a vertex of a polyline".into();
            return ToolResult::consumed();
        };
        let Some(mut l) = logical_of(cx, obj.id) else {
            cx.status = "Delete Break: click a vertex of a polyline".into();
            return ToolResult::consumed();
        };
        let tol = cx.pick_tol() * 3.0;
        let Some((v, d)) = l.nearest_vertex(p.world).filter(|(_, d)| *d <= tol) else {
            cx.status = "Delete Break: click closer to a vertex".into();
            return ToolResult::consumed();
        };
        let _ = d;
        if !cx.check_unlocked(ObjectRef::Cad(obj.id)) {
            return ToolResult::consumed();
        }
        if let Err(e) = l.delete_vertex(v) {
            cx.status = format!("{label}: {e}");
            return ToolResult::consumed();
        }
        cx.begin_change(label);
        store_polyline(cx, obj.id, &l);
        Self::arc_done(cx, label)
    }

    /// Make Arc Tangent (CAD-24).
    pub(super) fn arc_tangent_click(
        &mut self,
        cx: &mut EditorContext,
        p: &PointerEvent,
    ) -> ToolResult {
        let label = "Make Arc Tangent";
        let Some(obj) = self.arc_target(cx, p) else {
            cx.status = "Make Arc Tangent: click an arc".into();
            return ToolResult::consumed();
        };
        if !cx.check_unlocked(ObjectRef::Cad(obj.id)) {
            return ToolResult::consumed();
        }
        let fl = cx.floor;
        match &obj.item {
            CadItem::Polyline { .. } => {
                let Some(mut l) = logical_of(cx, obj.id) else {
                    return ToolResult::consumed();
                };
                let Some(edge) = (0..l.edge_count())
                    .filter(|i| l.bulge[*i].is_some())
                    .min_by(|a, b| {
                        let da = l
                            .edge_path(*a)
                            .windows(2)
                            .map(|w| {
                                plan_core::geometry::project_on_segment(p.world, w[0], w[1])
                                    .1
                                    .dist(p.world)
                            })
                            .fold(f64::MAX, f64::min);
                        let db = l
                            .edge_path(*b)
                            .windows(2)
                            .map(|w| {
                                plan_core::geometry::project_on_segment(p.world, w[0], w[1])
                                    .1
                                    .dist(p.world)
                            })
                            .fold(f64::MAX, f64::min);
                        da.total_cmp(&db)
                    })
                else {
                    cx.status = "Make Arc Tangent: that polyline has no arc edge".into();
                    return ToolResult::consumed();
                };
                if let Err(e) = l.make_tangent(edge) {
                    cx.status = format!("{label}: {e}");
                    return ToolResult::consumed();
                }
                cx.begin_change(label);
                store_polyline(cx, obj.id, &l);
            }
            CadItem::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let at =
                    |a: f64| Point::new(center.x + radius * a.cos(), center.y + radius * a.sin());
                let ends = [(true, at(*start_angle)), (false, at(*end_angle))];
                // Each end of the arc against the free ends of the other CAD
                // objects: a line, or the first and last segment of a polyline.
                let mut found: Vec<(bool, Point, Point, f64)> = Vec::new();
                for other in cx.floor().cad.iter().filter(|c| c.id != obj.id) {
                    let ends_of: Vec<(Point, Point)> = match &other.item {
                        CadItem::Line { a, b } => vec![(*a, *b), (*b, *a)],
                        CadItem::Polyline {
                            points,
                            closed: false,
                        } if points.len() >= 2 => {
                            let n = points.len();
                            vec![(points[0], points[1]), (points[n - 1], points[n - 2])]
                        }
                        _ => Vec::new(),
                    };
                    for (shared_end, inner) in ends_of {
                        for (is_start, e) in ends {
                            if e.dist(shared_end) <= 1.5 {
                                // Travel continues through the shared point.
                                let dir = shared_end.sub(inner);
                                found.push((is_start, e, dir, e.dist(p.world)));
                            }
                        }
                    }
                }
                found.sort_by(|a, b| a.3.total_cmp(&b.3));
                let Some((is_start, _, dir, _)) = found.first().copied() else {
                    cx.status = "Make Arc Tangent: no line meets this arc at either end".into();
                    return ToolResult::consumed();
                };
                let Some(new) = arc_tangent_at(&obj.item, is_start, dir) else {
                    cx.status = "Make Arc Tangent: that arc cannot be tangent there".into();
                    return ToolResult::consumed();
                };
                cx.begin_change(label);
                if let Some(c) = cx.project.floors[fl]
                    .cad
                    .iter_mut()
                    .find(|c| c.id == obj.id)
                {
                    c.item = new;
                }
            }
            _ => {
                cx.status = format!("{label}: click an arc, or a polyline with an arc edge");
                return ToolResult::consumed();
            }
        }
        Self::arc_done(cx, label)
    }
}

// ----- Disconnect Edges and Hide / Show Edge (CAD-117) -----

/// One part of a polyline after an edge was disconnected.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    /// A single edge: a line or an arc object.
    Edge(CadItem),
    /// Two or more edges that stay one polyline.
    Chain(Logical),
}

/// The object an edge becomes on its own: a line, or an arc object.
pub fn edge_item(l: &Logical, i: usize) -> CadItem {
    let n = l.pts.len();
    let (a, b) = (l.pts[i], l.pts[(i + 1) % n]);
    if let Some(bu) = l.bulge[i] {
        if let Some((center, radius)) = arc_center(a, b, bu) {
            let (sa, ea) = (a.sub(center).angle(), b.sub(center).angle());
            let (start_angle, end_angle) = if bu > 0.0 { (sa, ea) } else { (ea, sa) };
            return CadItem::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            };
        }
    }
    CadItem::Line { a, b }
}

/// `count` edges of `l` from edge `first` on (wrapping on a closed
/// polyline) as one piece.
fn chain(l: &Logical, first: usize, count: usize) -> Piece {
    let n = l.pts.len();
    if count == 1 {
        return Piece::Edge(edge_item(l, first));
    }
    let mut pts = Vec::with_capacity(count + 1);
    let mut bulge = Vec::with_capacity(count + 1);
    for j in 0..count {
        pts.push(l.pts[(first + j) % n]);
        bulge.push(l.bulge[(first + j) % n]);
    }
    pts.push(l.pts[(first + count) % n]);
    bulge.push(None);
    Piece::Chain(Logical {
        pts,
        bulge,
        closed: false,
    })
}

/// The polyline `l` with edge `e` disconnected: the edge itself first, then
/// what is left of the polyline, in order (nothing if there is nothing
/// else, so a lone line cannot be disconnected).
pub fn disconnect_pieces(l: &Logical, e: usize) -> Option<Vec<Piece>> {
    let (n, edges) = (l.pts.len(), l.edge_count());
    if e >= edges || edges < 2 {
        return None;
    }
    let mut out = vec![Piece::Edge(edge_item(l, e))];
    if l.closed {
        out.push(chain(l, (e + 1) % n, n - 1));
    } else {
        if e >= 1 {
            out.push(chain(l, 0, e));
        }
        let after = edges - 1 - e;
        if after >= 1 {
            out.push(chain(l, e + 1, after));
        }
    }
    Some(out)
}

impl CadTool {
    /// Disconnect Edges (CAD-117): the clicked edge of a polyline becomes an
    /// object of its own; the rest stays together.
    pub(super) fn disconnect_click(
        &mut self,
        cx: &mut EditorContext,
        p: &PointerEvent,
    ) -> ToolResult {
        let label = "Disconnect Edges";
        let Some(obj) = self.arc_target(cx, p) else {
            cx.status = format!("{label}: click an edge of a polyline");
            return ToolResult::consumed();
        };
        let Some(l) = logical_of(cx, obj.id) else {
            cx.status = format!("{label}: click an edge of a polyline");
            return ToolResult::consumed();
        };
        let Some((e, _)) = l
            .nearest_edge(p.world)
            .filter(|(_, d)| *d <= cx.pick_tol() * 3.0)
        else {
            cx.status = format!("{label}: click closer to an edge");
            return ToolResult::consumed();
        };
        let Some(pieces) = disconnect_pieces(&l, e) else {
            cx.status = format!("{label}: a single edge has nothing to disconnect from");
            return ToolResult::consumed();
        };
        if !cx.check_unlocked(ObjectRef::Cad(obj.id)) {
            return ToolResult::consumed();
        }
        cx.begin_change(label);
        let fl = cx.floor;
        let look = cx.floor().cad_attrs(obj.id).unwrap_or_default();
        let mut edge_id = obj.id;
        for (k, piece) in pieces.into_iter().enumerate() {
            // The first piece (the edge) keeps the object's identity.
            let (item, arcs) = match piece {
                Piece::Edge(item) => (item, Vec::new()),
                Piece::Chain(c) => {
                    let (points, arcs) = c.rebuild();
                    (
                        CadItem::Polyline {
                            points,
                            closed: false,
                        },
                        arcs,
                    )
                }
            };
            let id = if k == 0 {
                if let Some(slot) = cx.project.floors[fl]
                    .cad
                    .iter_mut()
                    .find(|c| c.id == obj.id)
                {
                    slot.item = item;
                }
                obj.id
            } else {
                cx.project.add_cad(fl, &obj.layer, item)
            };
            let mut a = look.clone();
            a.target = id;
            a.arc_edges = arcs;
            a.hidden_edges.clear();
            cx.project.set_cad_attrs(fl, a);
            if k == 0 {
                edge_id = id;
            }
        }
        cx.selection.set(ObjectRef::Cad(edge_id));
        Self::arc_done(cx, label)
    }

    /// Hide / Show Selected Edge (CAD-117): the clicked edge of a polyline
    /// stops drawing, or draws again.
    pub(super) fn edge_visibility_click(
        &mut self,
        cx: &mut EditorContext,
        p: &PointerEvent,
    ) -> ToolResult {
        let label = "Hide/Show Edge";
        let Some(obj) = self.arc_target(cx, p) else {
            cx.status = format!("{label}: click an edge of a polyline");
            return ToolResult::consumed();
        };
        let Some(l) = logical_of(cx, obj.id) else {
            cx.status = format!("{label}: click an edge of a polyline");
            return ToolResult::consumed();
        };
        let Some((e, _)) = l
            .nearest_edge(p.world)
            .filter(|(_, d)| *d <= cx.pick_tol() * 3.0)
        else {
            cx.status = format!("{label}: click closer to an edge");
            return ToolResult::consumed();
        };
        if !cx.check_unlocked(ObjectRef::Cad(obj.id)) {
            return ToolResult::consumed();
        }
        cx.begin_change(label);
        let fl = cx.floor;
        let mut now_hidden = false;
        cx.project.edit_cad_attrs(fl, obj.id, |a: &mut CadAttrs| {
            if let Some(i) = a.hidden_edges.iter().position(|h| *h == e) {
                a.hidden_edges.remove(i);
            } else {
                a.hidden_edges.push(e);
                a.hidden_edges.sort_unstable();
                now_hidden = true;
            }
        });
        let result = Self::arc_done(cx, label);
        cx.status = if now_hidden {
            "Edge hidden; click it again to show it".into()
        } else {
            "Edge shown".into()
        };
        result
    }
}

// ----- Input Arc (CAD-8, Round 16 brief 06) -----

/// Which side of its direction of travel an Input Arc curves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    /// Counter-clockwise: the center is to the left of the direction.
    Left,
    /// Clockwise: the center is to the right.
    Right,
}

/// How far an Input Arc runs from its start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Extent {
    /// The arc angle, in degrees (above 0, up to 360).
    Angle(f64),
    /// The length measured along the arc (inches).
    ArcLength(f64),
    /// The straight distance from start to end (inches); the shorter arc
    /// (up to a half circle) has that chord.
    ChordLength(f64),
}

/// The arc an Input Arc dialog describes.
#[derive(Debug, Clone, PartialEq)]
pub struct InputArc {
    /// The arc object (stored counter-clockwise, whichever way it curves).
    pub item: CadItem,
    pub start: Point,
    pub end: Point,
    /// The direction of travel at the end, degrees counter-clockwise from
    /// east: where a following arc or line continues.
    pub end_dir: f64,
    /// The arc angle in degrees.
    pub sweep: f64,
}

/// Builds the arc that leaves `start` heading `dir_deg` (degrees
/// counter-clockwise from east), curves to `curve` with `radius` and runs
/// for `extent`.
pub fn input_arc(
    start: Point,
    dir_deg: f64,
    radius: f64,
    curve: Curve,
    extent: Extent,
) -> Result<InputArc, &'static str> {
    use std::f64::consts::TAU;
    if radius.is_nan() || radius <= 1e-6 {
        return Err("The radius needs a distance above zero");
    }
    let sweep = match extent {
        Extent::Angle(deg) => deg.to_radians(),
        Extent::ArcLength(len) => len / radius,
        Extent::ChordLength(chord) => {
            if chord.is_nan() || chord <= 1e-9 {
                return Err("The chord length needs a distance above zero");
            }
            if chord > 2.0 * radius + 1e-9 {
                return Err("The chord is longer than the diameter");
            }
            2.0 * (chord / (2.0 * radius)).min(1.0).asin()
        }
    };
    if sweep.is_nan() || sweep <= 1e-9 {
        return Err("The arc needs a length or angle above zero");
    }
    if sweep > TAU + 1e-9 {
        return Err("The arc runs more than a full circle");
    }
    let sweep = sweep.min(TAU);
    let dir = dir_deg.to_radians();
    let heading = Point::new(dir.cos(), dir.sin());
    let (center, a_start, a_end, end_dir) = match curve {
        Curve::Left => {
            let center = start.add(heading.perp().scale(radius));
            let a0 = dir - std::f64::consts::FRAC_PI_2;
            (center, a0, a0 + sweep, dir + sweep)
        }
        Curve::Right => {
            let center = start.sub(heading.perp().scale(radius));
            let a0 = dir + std::f64::consts::FRAC_PI_2;
            (center, a0, a0 - sweep, dir - sweep)
        }
    };
    let at = |a: f64| center.add(Point::new(a.cos(), a.sin()).scale(radius));
    let end = at(a_end);
    // The object turns counter-clockwise from its start angle to its end
    // angle, so a right-curving arc is stored from its end back to its start.
    let (start_angle, end_angle) = match curve {
        Curve::Left => (a_start, a_end),
        Curve::Right => (a_end, a_start),
    };
    Ok(InputArc {
        item: CadItem::Arc {
            center,
            radius,
            start_angle: start_angle.rem_euclid(TAU),
            end_angle: end_angle.rem_euclid(TAU),
        },
        start,
        end,
        end_dir: end_dir.to_degrees().rem_euclid(360.0),
        sweep: sweep.to_degrees(),
    })
}

/// Like [`input_arc`], but the direction is that of the arc's chord (the
/// straight line from the start to the end) rather than the tangent at the
/// start (New CAD Arc: Chord Direction).
pub fn input_arc_chord(
    start: Point,
    chord_dir_deg: f64,
    radius: f64,
    curve: Curve,
    extent: Extent,
) -> Result<InputArc, &'static str> {
    let probe = input_arc(start, chord_dir_deg, radius, curve, extent)?;
    // The chord lies half the arc angle off the tangent, toward the center.
    let half = probe.sweep / 2.0;
    let tangent = match curve {
        Curve::Left => chord_dir_deg - half,
        Curve::Right => chord_dir_deg + half,
    };
    input_arc(start, tangent, radius, curve, extent)
}

/// Free Form arc: the arc through the ends of a dragged path and the point
/// of the path farthest from the chord (Free Form mode, CAD-7).
pub fn free_form_arc(path: &[Point]) -> Option<CadItem> {
    let (a, b) = (*path.first()?, *path.last()?);
    let chord = b.sub(a);
    if chord.length() < 1e-6 {
        return None;
    }
    let n = chord.normalized().perp();
    let m = path
        .iter()
        .copied()
        .max_by(|p, q| n.dot(p.sub(a)).abs().total_cmp(&n.dot(q.sub(a)).abs()))?;
    arc_three_point(a, m, b)
}

/// Arc About Center: the arc about `center` from `start` to the angle of
/// `end`, taking the shorter way round (Arc About Center mode, CAD-7).
pub fn arc_about_center(center: Point, start: Point, end: Point) -> Option<CadItem> {
    let radius = center.dist(start);
    if radius < 1e-6 || center.dist(end) < 1e-6 {
        return None;
    }
    let (a0, a1) = (start.sub(center).angle(), end.sub(center).angle());
    let ccw = (a1 - a0).rem_euclid(std::f64::consts::TAU);
    let (start_angle, end_angle) = if ccw <= std::f64::consts::PI {
        (a0, a1)
    } else {
        (a1, a0)
    };
    Some(CadItem::Arc {
        center,
        radius,
        start_angle,
        end_angle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        a.dist(b) < 1e-6
    }

    #[test]
    fn a_quarter_bulge_samples_a_quarter_circle() {
        let pts = arc_samples(p(0.0, 0.0), p(100.0, 0.0), DEFAULT_BULGE, 12);
        assert_eq!(pts.len(), 13);
        assert!(near(pts[0], p(0.0, 0.0)) && near(pts[12], p(100.0, 0.0)));
        let (c, r) = arc_center(p(0.0, 0.0), p(100.0, 0.0), DEFAULT_BULGE).unwrap();
        assert!((r - 100.0 / 2f64.sqrt()).abs() < 1e-6);
        // A positive bulge is counter-clockwise: the center is left of the
        // chord and the arc bulges to the right (below it).
        assert!(c.y > 0.0, "center {c:?}");
        assert!(pts.iter().skip(1).take(11).all(|q| q.y < 0.0));
        for q in &pts {
            assert!((q.dist(c) - r).abs() < 1e-6);
        }
    }

    #[test]
    fn decompose_and_rebuild_round_trip() {
        let l = Logical {
            pts: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)],
            bulge: vec![None, Some(0.5), None, None],
            closed: true,
        };
        let (points, arcs) = l.rebuild();
        assert_eq!(arcs.len(), 1);
        assert_eq!(arcs[0].from, 1);
        assert!(points.len() > 4);
        let back = Logical::decompose(&points, true, &arcs);
        assert_eq!(back.pts, l.pts);
        assert_eq!(back.bulge, l.bulge);
        // A span whose sample was dragged is forgotten.
        let mut bent = points.clone();
        bent[2] = bent[2] + p(20.0, 20.0);
        let forgotten = Logical::decompose(&bent, true, &arcs);
        assert!(forgotten.bulge.iter().all(Option::is_none));
        assert_eq!(forgotten.pts.len(), bent.len());
    }

    #[test]
    fn closing_edge_arcs_wrap() {
        let l = Logical {
            pts: vec![p(0.0, 0.0), p(100.0, 0.0), p(50.0, 80.0)],
            bulge: vec![None, None, Some(-0.3)],
            closed: true,
        };
        let (points, arcs) = l.rebuild();
        assert_eq!(arcs[0].to, points.len());
        assert_eq!(Logical::decompose(&points, true, &arcs), l);
    }

    #[test]
    fn toggling_an_edge_bulges_toward_the_click() {
        let mut l = Logical {
            pts: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0)],
            bulge: vec![None, None, None],
            closed: false,
        };
        // Click below the first edge: the arc dips below it.
        l.toggle_edge(0, p(50.0, -10.0)).unwrap();
        let path = l.edge_path(0);
        assert!(path[path.len() / 2].y < -5.0);
        // Toggling again straightens it.
        l.toggle_edge(0, p(50.0, -10.0)).unwrap();
        assert_eq!(l.bulge[0], None);
        // Click above: it rises.
        l.toggle_edge(0, p(50.0, 10.0)).unwrap();
        let path = l.edge_path(0);
        assert!(path[path.len() / 2].y > 5.0);
    }

    #[test]
    fn delete_vertex_joins_the_neighbours_with_a_straight_edge() {
        let mut l = Logical {
            pts: vec![p(0.0, 0.0), p(50.0, 10.0), p(100.0, 0.0), p(100.0, 50.0)],
            bulge: vec![Some(0.3), Some(0.3), None, None],
            closed: false,
        };
        l.delete_vertex(1).unwrap();
        assert_eq!(l.pts, vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 50.0)]);
        assert_eq!(l.bulge, vec![None, None, None]);
        // Ends can go too.
        l.delete_vertex(0).unwrap();
        assert_eq!(l.pts.len(), 2);
        assert!(l.delete_vertex(0).is_err(), "two vertices stay");
        // A closed polyline keeps three.
        let mut tri = Logical {
            pts: vec![p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0)],
            bulge: vec![None; 3],
            closed: true,
        };
        assert!(tri.delete_vertex(1).is_err());
    }

    #[test]
    fn make_tangent_removes_the_corner_with_the_previous_edge() {
        // Leaves a horizontal edge to the right, then arcs up to (100, 100).
        let mut l = Logical {
            pts: vec![p(-100.0, 0.0), p(0.0, 0.0), p(100.0, 100.0)],
            bulge: vec![None, Some(0.9), None],
            closed: false,
        };
        l.make_tangent(1).unwrap();
        let b = l.bulge[1].unwrap();
        let t = edge_tangent(p(0.0, 0.0), p(100.0, 100.0), b, false);
        assert!(near(t, p(1.0, 0.0)), "start tangent {t:?}");
        // The arc turns left (counter-clockwise) toward the chord.
        assert!(b > 0.0);
        // The first edge of an open polyline is tangent at its far end.
        let mut first = Logical {
            pts: vec![p(0.0, 0.0), p(100.0, 0.0), p(200.0, 0.0)],
            bulge: vec![Some(0.5), None, None],
            closed: false,
        };
        // The next edge runs along +x, so the arc must arrive heading +x:
        // that is a straight chord, which cannot be an arc.
        assert!(first.make_tangent(0).is_err());
        first.pts[2] = p(100.0, 100.0);
        first.pts[1] = p(100.0, 0.0);
        first.pts[2] = p(100.0, 100.0);
        // The next edge runs up (+y): an arc from the origin ends heading up.
        first.make_tangent(0).unwrap();
        let b = first.bulge[0].unwrap();
        let end = edge_tangent(p(0.0, 0.0), p(100.0, 0.0), b, true);
        assert!(near(end, p(0.0, 1.0)), "end tangent {end:?}");
    }

    #[test]
    fn dragging_the_apex_sets_the_bulge() {
        let mut l = Logical {
            pts: vec![p(0.0, 0.0), p(100.0, 0.0)],
            bulge: vec![Some(DEFAULT_BULGE), None],
            closed: false,
        };
        // An apex 25 below the chord of 100 is a bulge of 0.5.
        l.set_bulge_through(0, p(50.0, -25.0)).unwrap();
        assert!((l.bulge[0].unwrap() - 0.5).abs() < 1e-9);
        l.set_bulge_through(0, p(50.0, 25.0)).unwrap();
        assert!((l.bulge[0].unwrap() + 0.5).abs() < 1e-9);
        // Never collapses to a straight edge.
        l.set_bulge_through(0, p(50.0, 0.0)).unwrap();
        assert!(l.bulge[0].unwrap().abs() >= 0.02);
    }

    #[test]
    fn an_arc_object_turns_tangent_to_a_line_at_its_end() {
        // A line along +x ending at (100, 0); an arc starting there.
        let arc = CadItem::Arc {
            center: p(100.0, 50.0),
            radius: 50.0,
            start_angle: -std::f64::consts::FRAC_PI_2,
            end_angle: std::f64::consts::FRAC_PI_2,
        };
        // It leaves toward +x (tangent). Tilt the requirement: the line
        // arrives heading up-right, so the arc must leave that way (its far
        // end, (100, 100), is not on that line, so a circle exists).
        let dir = p(1.0, 1.0);
        let CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } = arc_tangent_at(&arc, true, dir).unwrap()
        else {
            panic!()
        };
        let start = p(
            center.x + radius * start_angle.cos(),
            center.y + radius * start_angle.sin(),
        );
        let end = p(
            center.x + radius * end_angle.cos(),
            center.y + radius * end_angle.sin(),
        );
        assert!(
            near(start, p(100.0, 0.0)),
            "keeps the shared end: {start:?}"
        );
        assert!(near(end, p(100.0, 100.0)), "keeps the other end: {end:?}");
        // The arc's tangent at the start is along the requested direction.
        let t = p(-start_angle.sin(), start_angle.cos());
        let d = dir.normalized();
        assert!(t.dot(d) > 0.999, "tangent {t:?}");
    }

    fn tool(mode: CadMode) -> CadTool {
        let mut t = CadTool::default();
        t.set_mode(mode);
        t
    }

    fn click(t: &mut CadTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let ev = PointerEvent::at(cx, Point::new(x, y)).with_down(true);
        let down = t.pointer_down(cx, ev);
        let up = t.pointer_up(cx, PointerEvent::at(cx, Point::new(x, y)));
        if down.commit.is_some() {
            down
        } else {
            up
        }
    }

    fn new_cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    #[test]
    fn change_line_arc_toggles_a_polyline_edge_and_undoes() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        let id = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Polyline {
                points: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0)],
                closed: false,
            },
        );
        let mut t = tool(CadMode::ChangeLineArc);
        let res = click(&mut t, &mut cx, 50.0, 0.0);
        assert_eq!(res.commit.as_deref(), Some("Change Line/Arc"));
        let CadItem::Polyline { points, .. } = &cx.floor().cad[0].item else {
            panic!()
        };
        assert!(
            points.len() > 10,
            "the edge is now sampled: {}",
            points.len()
        );
        let attrs = cx.floor().cad_attrs(id).unwrap();
        assert_eq!(attrs.arc_edges.len(), 1);
        assert_eq!(attrs.arc_edges[0].from, 0);
        // Click the arc again: back to one straight edge.
        let mid = points[points.len() / 4 + 1];
        let res = click(&mut t, &mut cx, mid.x, mid.y);
        assert_eq!(res.commit.as_deref(), Some("Change Line/Arc"));
        let CadItem::Polyline { points, .. } = &cx.floor().cad[0].item else {
            panic!()
        };
        assert_eq!(points.len(), 3);
        assert!(cx.floor().cad_attrs(id).is_none(), "no arc record left");
        cx.undo();
        let CadItem::Polyline { points, .. } = &cx.floor().cad[0].item else {
            panic!()
        };
        assert!(points.len() > 10);
        cx.undo();
        let CadItem::Polyline { points, .. } = &cx.floor().cad[0].item else {
            panic!()
        };
        assert_eq!(points.len(), 3);
    }

    #[test]
    fn change_line_arc_turns_a_line_into_an_arc_and_back() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Line {
                a: p(0.0, 0.0),
                b: p(100.0, 0.0),
            },
        );
        let mut t = tool(CadMode::ChangeLineArc);
        click(&mut t, &mut cx, 50.0, 0.0);
        assert!(matches!(cx.floor().cad[0].item, CadItem::Arc { .. }));
        let CadItem::Arc { .. } = &cx.floor().cad[0].item else {
            panic!()
        };
        let (lo, hi) = cx.floor().cad[0].bounds();
        click(&mut t, &mut cx, (lo.x + hi.x) * 0.5, lo.y);
        assert!(matches!(cx.floor().cad[0].item, CadItem::Line { .. }));
    }

    #[test]
    fn delete_break_removes_a_polyline_vertex() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Polyline {
                points: vec![p(0.0, 0.0), p(50.0, 30.0), p(100.0, 0.0)],
                closed: false,
            },
        );
        let mut t = tool(CadMode::DeleteBreak);
        let res = click(&mut t, &mut cx, 50.0, 30.0);
        assert_eq!(res.commit.as_deref(), Some("Delete Break"));
        let CadItem::Polyline { points, .. } = &cx.floor().cad[0].item else {
            panic!()
        };
        assert_eq!(points, &vec![p(0.0, 0.0), p(100.0, 0.0)]);
        // Two vertices stay: the next click only explains.
        let res = click(&mut t, &mut cx, 0.0, 0.0);
        assert!(res.commit.is_none());
        assert!(cx.status.contains("at least two"));
        cx.undo();
        let CadItem::Polyline { points, .. } = &cx.floor().cad[0].item else {
            panic!()
        };
        assert_eq!(points.len(), 3);
    }

    #[test]
    fn make_arc_tangent_adjusts_an_arc_object_to_the_line_beside_it() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        // A line heading up-right ending at (100, 0)... via (0, -100).
        cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Line {
                a: p(0.0, -100.0),
                b: p(100.0, 0.0),
            },
        );
        let arc = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Arc {
                center: p(100.0, 50.0),
                radius: 50.0,
                start_angle: -std::f64::consts::FRAC_PI_2,
                end_angle: std::f64::consts::FRAC_PI_2,
            },
        );
        let mut t = tool(CadMode::MakeArcTangent);
        // Click the arc near its middle.
        let mid = p(
            100.0 + 50.0 * (-0.785f64).cos(),
            50.0 + 50.0 * (-0.785f64).sin(),
        );
        let res = click(&mut t, &mut cx, mid.x, mid.y);
        assert_eq!(res.commit.as_deref(), Some("Make Arc Tangent"));
        let item = cx
            .floor()
            .cad
            .iter()
            .find(|c| c.id == arc)
            .unwrap()
            .item
            .clone();
        let CadItem::Arc {
            center,
            radius,
            start_angle,
            ..
        } = item
        else {
            panic!()
        };
        let start = p(
            center.x + radius * start_angle.cos(),
            center.y + radius * start_angle.sin(),
        );
        assert!(start.dist(p(100.0, 0.0)) < 1e-6);
        let tangent = p(-start_angle.sin(), start_angle.cos());
        assert!(tangent.dot(p(1.0, 1.0).normalized()) > 0.999);
    }

    #[test]
    fn input_arc_left_and_right_quarter_circles() {
        // Heading east, curving left (up) a quarter circle of radius 100.
        let l = input_arc(p(0.0, 0.0), 0.0, 100.0, Curve::Left, Extent::Angle(90.0)).unwrap();
        assert!(near(l.end, p(100.0, 100.0)), "{:?}", l.end);
        assert!((l.end_dir - 90.0).abs() < 1e-9 && (l.sweep - 90.0).abs() < 1e-9);
        let CadItem::Arc { center, radius, .. } = l.item else {
            panic!()
        };
        assert!(near(center, p(0.0, 100.0)) && (radius - 100.0).abs() < 1e-9);
        // Curving right (down).
        let r = input_arc(p(0.0, 0.0), 0.0, 100.0, Curve::Right, Extent::Angle(90.0)).unwrap();
        assert!(near(r.end, p(100.0, -100.0)), "{:?}", r.end);
        assert!((r.end_dir - 270.0).abs() < 1e-9);
        // Stored counter-clockwise: the same points are on the object.
        let CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } = r.item
        else {
            panic!()
        };
        let sweep = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
        assert!((sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        let first = center.add(Point::new(start_angle.cos(), start_angle.sin()).scale(radius));
        assert!(near(first, r.end), "stored from the end back to the start");
    }

    #[test]
    fn input_arc_by_chord_direction_ends_on_the_chord() {
        for curve in [Curve::Left, Curve::Right] {
            let a = input_arc_chord(p(0.0, 0.0), 30.0, 100.0, curve, Extent::Angle(70.0)).unwrap();
            let chord = a.end.sub(a.start).angle().to_degrees();
            assert!((chord - 30.0).abs() < 1e-6, "{curve:?} {chord}");
            assert!((a.sweep - 70.0).abs() < 1e-9);
        }
    }

    #[test]
    fn input_arc_extents_agree() {
        let by_angle =
            input_arc(p(5.0, 5.0), 30.0, 120.0, Curve::Left, Extent::Angle(60.0)).unwrap();
        let len = 120.0 * 60f64.to_radians();
        let by_len = input_arc(
            p(5.0, 5.0),
            30.0,
            120.0,
            Curve::Left,
            Extent::ArcLength(len),
        )
        .unwrap();
        let by_chord = input_arc(
            p(5.0, 5.0),
            30.0,
            120.0,
            Curve::Left,
            Extent::ChordLength(120.0),
        )
        .unwrap();
        // A 60 degree arc has a chord equal to its radius.
        assert!(near(by_angle.end, by_len.end) && near(by_angle.end, by_chord.end));
        assert!((by_chord.sweep - 60.0).abs() < 1e-9);
        assert!(by_angle.start.dist(by_angle.end) - 120.0 < 1e-9);
    }

    #[test]
    fn input_arc_refuses_impossible_input() {
        let go = |r, e| input_arc(p(0.0, 0.0), 0.0, r, Curve::Left, e);
        assert!(go(0.0, Extent::Angle(10.0)).is_err());
        assert!(go(10.0, Extent::ChordLength(21.0)).is_err());
        assert!(go(10.0, Extent::Angle(0.0)).is_err());
        assert!(go(10.0, Extent::Angle(400.0)).is_err());
        assert!(go(10.0, Extent::ArcLength(-1.0)).is_err());
        assert!(go(10.0, Extent::ChordLength(20.0)).is_ok());
    }

    #[test]
    fn free_form_and_about_center_arcs() {
        // A dragged path bulging up from (0,0) to (100,0).
        let path = [
            p(0.0, 0.0),
            p(25.0, 30.0),
            p(50.0, 40.0),
            p(75.0, 30.0),
            p(100.0, 0.0),
        ];
        let CadItem::Arc { center, radius, .. } = free_form_arc(&path).unwrap() else {
            panic!()
        };
        for q in [path[0], path[2], path[4]] {
            assert!((q.dist(center) - radius).abs() < 1e-6);
        }
        assert!(free_form_arc(&[p(1.0, 1.0)]).is_none());
        assert!(free_form_arc(&[p(1.0, 1.0), p(1.0, 1.0)]).is_none());
        // About a center, the short way round whichever way it is dragged.
        let ccw = arc_about_center(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0)).unwrap();
        let cw = arc_about_center(p(0.0, 0.0), p(0.0, 10.0), p(10.0, 0.0)).unwrap();
        assert_eq!(ccw, cw);
        assert!(arc_about_center(p(0.0, 0.0), p(0.0, 0.0), p(1.0, 0.0)).is_none());
    }

    fn square() -> Logical {
        Logical {
            pts: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)],
            bulge: vec![None, Some(0.5), None, None],
            closed: true,
        }
    }

    #[test]
    fn disconnecting_an_edge_of_a_closed_polyline_leaves_an_open_chain() {
        let l = square();
        let pieces = disconnect_pieces(&l, 2).unwrap();
        assert_eq!(pieces.len(), 2);
        assert_eq!(
            pieces[0],
            Piece::Edge(CadItem::Line {
                a: p(100.0, 100.0),
                b: p(0.0, 100.0)
            })
        );
        let Piece::Chain(rest) = &pieces[1] else {
            panic!()
        };
        // Starts where the removed edge ended and runs round to where it began.
        assert_eq!(
            rest.pts,
            vec![p(0.0, 100.0), p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0)]
        );
        assert_eq!(rest.bulge, vec![None, None, Some(0.5), None]);
        assert!(!rest.closed);
        // The arc edge on its own is an arc object.
        let arc = disconnect_pieces(&l, 1).unwrap();
        assert!(matches!(arc[0], Piece::Edge(CadItem::Arc { .. })));
    }

    #[test]
    fn disconnecting_in_an_open_polyline_splits_it_in_up_to_three() {
        let open = Logical {
            pts: vec![
                p(0.0, 0.0),
                p(10.0, 0.0),
                p(10.0, 10.0),
                p(0.0, 10.0),
                p(0.0, 20.0),
            ],
            bulge: vec![None; 5],
            closed: false,
        };
        let mid = disconnect_pieces(&open, 1).unwrap();
        assert_eq!(mid.len(), 3);
        assert!(matches!(&mid[1], Piece::Edge(CadItem::Line { b, .. }) if *b == p(10.0, 0.0)));
        assert!(matches!(&mid[2], Piece::Chain(c) if c.pts.len() == 3));
        assert_eq!(disconnect_pieces(&open, 0).unwrap().len(), 2);
        assert_eq!(disconnect_pieces(&open, 3).unwrap().len(), 2);
        assert!(disconnect_pieces(&open, 4).is_none());
        let lone = Logical {
            pts: vec![p(0.0, 0.0), p(1.0, 0.0)],
            bulge: vec![None, None],
            closed: false,
        };
        assert!(disconnect_pieces(&lone, 0).is_none());
    }

    fn lshape(cx: &mut EditorContext) -> Id {
        cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Polyline {
                points: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)],
                closed: true,
            },
        )
    }

    #[test]
    fn disconnect_edges_makes_the_clicked_edge_its_own_object() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        let id = lshape(&mut cx);
        let mut t = tool(CadMode::DisconnectEdges);
        let res = click(&mut t, &mut cx, 100.0, 50.0);
        assert_eq!(res.commit.as_deref(), Some("Disconnect Edges"));
        let cad = &cx.floor().cad;
        assert_eq!(cad.len(), 2, "the edge and the rest");
        let edge = cad.iter().find(|c| c.id == id).unwrap();
        assert_eq!(
            edge.item,
            CadItem::Line {
                a: p(100.0, 0.0),
                b: p(100.0, 100.0)
            }
        );
        let rest = cad.iter().find(|c| c.id != id).unwrap();
        let CadItem::Polyline { points, closed } = &rest.item else {
            panic!()
        };
        assert!(!closed);
        assert_eq!(
            points,
            &vec![p(100.0, 100.0), p(0.0, 100.0), p(0.0, 0.0), p(100.0, 0.0)]
        );
        // The edge is selected on its own; one undo puts the polyline back.
        assert!(cx.selection.contains(ObjectRef::Cad(id)));
        cx.undo();
        assert_eq!(cx.floor().cad.len(), 1);
        assert!(matches!(
            cx.floor().cad[0].item,
            CadItem::Polyline { closed: true, .. }
        ));
        // A lone line has nothing to disconnect from.
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Line {
                a: p(0.0, 0.0),
                b: p(50.0, 0.0),
            },
        );
        let res = click(&mut tool(CadMode::DisconnectEdges), &mut cx, 25.0, 0.0);
        assert!(res.commit.is_none());
    }

    #[test]
    fn hide_show_edge_toggles_one_edge_and_undoes() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        let id = lshape(&mut cx);
        let mut t = tool(CadMode::HideShowEdge);
        click(&mut t, &mut cx, 50.0, 100.0);
        assert_eq!(cx.floor().cad_attrs(id).unwrap().hidden_edges, vec![2]);
        click(&mut t, &mut cx, 0.0, 50.0);
        assert_eq!(cx.floor().cad_attrs(id).unwrap().hidden_edges, vec![2, 3]);
        // The same edge again shows it.
        click(&mut t, &mut cx, 50.0, 100.0);
        assert_eq!(cx.floor().cad_attrs(id).unwrap().hidden_edges, vec![3]);
        cx.undo();
        assert_eq!(cx.floor().cad_attrs(id).unwrap().hidden_edges, vec![2, 3]);
        // Deleting a vertex forgets a hidden edge that is gone.
        let mut dt = tool(CadMode::DeleteBreak);
        click(&mut dt, &mut cx, 0.0, 100.0);
        let hidden = cx
            .floor()
            .cad_attrs(id)
            .map(|a| a.hidden_edges)
            .unwrap_or_default();
        assert!(hidden.iter().all(|e| *e < 3), "{hidden:?}");
    }
}
