//! Building and drawing the curved dimensions (radius, arc length and angle),
//! and moving or turning a dimension, with its string, as one object.
//!
//! The editor, the layout renderer and the sheet PDF all draw a curved
//! dimension from [`CurveGeom`]: polylines and one label spot, so the three
//! painters need no arc maths of their own.

use super::seg::{CurveKind, DimCurve};
use super::{DimFormat, Dimension};
use crate::geometry::Point;
use crate::model::{Floor, Id, Wall};
use std::f64::consts::{PI, TAU};

/// Where a curved dimension's parts lie.
#[derive(Debug, Clone, PartialEq)]
pub struct CurveGeom {
    /// Extension lines from a measured point out to the dimension line.
    pub extensions: Vec<(Point, Point)>,
    /// The dimension line as a polyline (a straight line has two points).
    pub line: Vec<Point>,
    /// Unit directions at the two ends of `line`, each pointing along the
    /// line away from its end (the way an end mark's body lies).
    pub inward: [Point; 2],
    /// The middle of the label and the direction the label reads along.
    pub label_at: Point,
    pub label_dir: Point,
}

impl DimCurve {
    /// The radius or arc length (`kind`) of the curved wall `w`, measured at
    /// the surface `lateral` inches to the left of its centerline. `None` for
    /// a straight wall or an angle kind.
    pub fn of_wall(w: &Wall, kind: CurveKind, lateral: f64) -> Option<DimCurve> {
        if kind == CurveKind::Angle {
            return None;
        }
        let (center, _) = w.arc_center_radius()?;
        let sweep = w.curve?.sweep(w.start, w.end);
        let at = w.point_offset(w.path_length() * 0.5, lateral);
        Some(DimCurve {
            kind,
            center,
            radius: center.dist(at).max(1e-6),
            start: w.start.sub(center).angle(),
            sweep,
            walls: [Some(w.id), None],
            toward_end: [true; 2],
            lateral,
        })
    }

    /// The angle at `vertex` between the arms through `p1` and `p2`, drawn
    /// as an arc of `radius`: the short way round, as Chief reads it.
    pub fn from_points(vertex: Point, p1: Point, p2: Point, radius: f64) -> Option<DimCurve> {
        if vertex.dist(p1) < 1e-6 || vertex.dist(p2) < 1e-6 || radius < 1e-6 {
            return None;
        }
        let (a1, a2) = (p1.sub(vertex).angle(), p2.sub(vertex).angle());
        let mut sweep = (a2 - a1).rem_euclid(TAU);
        let mut start = a1;
        if sweep > PI {
            start = a2;
            sweep = TAU - sweep;
        }
        Some(DimCurve {
            kind: CurveKind::Angle,
            center: vertex,
            radius,
            start,
            sweep,
            walls: [None, None],
            toward_end: [true; 2],
            lateral: 0.0,
        })
    }

    /// The angle between the lines of two walls, on the side of the clicks
    /// `p1` (on the first wall) and `p2` (on the second). The dimension
    /// follows both walls when they move.
    pub fn between_walls(
        a: &Wall,
        b: &Wall,
        p1: Point,
        p2: Point,
        radius: f64,
    ) -> Option<DimCurve> {
        let (da, db) = (
            a.end.sub(a.start).normalized(),
            b.end.sub(b.start).normalized(),
        );
        let denom = da.cross(db);
        if denom.abs() < 1e-9 {
            return None;
        }
        let t = b.start.sub(a.start).cross(db) / denom;
        let vertex = a.start.add(da.scale(t));
        let toward_end = [p1.sub(vertex).dot(da) >= 0.0, p2.sub(vertex).dot(db) >= 0.0];
        let ua = if toward_end[0] { da } else { da.scale(-1.0) };
        let ub = if toward_end[1] { db } else { db.scale(-1.0) };
        let mut c = DimCurve::from_points(vertex, vertex.add(ua), vertex.add(ub), radius)?;
        // Keep the arm order the walls give (from the first wall's arm).
        let start = ua.angle();
        let mut sweep = (ub.angle() - start).rem_euclid(TAU);
        if sweep > PI {
            sweep -= TAU;
        }
        c.start = start;
        c.sweep = sweep;
        c.walls = [Some(a.id), Some(b.id)];
        c.toward_end = toward_end;
        Some(c)
    }
}

fn unit(a: f64) -> Point {
    Point::new(a.cos(), a.sin())
}

impl Dimension {
    /// The parts of a radius, arc length or angle dimension, or `None` for a
    /// linear one. `gap` and `past` are the extension line settings.
    pub fn curve_geom(&self, gap: f64, past: f64) -> Option<CurveGeom> {
        let c = *self.curve()?;
        let (a0, a1) = c.angles();
        let sgn = if c.sweep >= 0.0 { 1.0 } else { -1.0 };
        Some(match c.kind {
            CurveKind::Radius => {
                // From the center out through the arc, and `offset` beyond.
                let mid = c.start + c.sweep * 0.5;
                let u = unit(mid);
                let tip = c.center.add(u.scale(c.radius + self.offset.max(0.0)));
                CurveGeom {
                    extensions: Vec::new(),
                    line: vec![c.center, tip],
                    inward: [u, u.scale(-1.0)],
                    label_at: Point::lerp(c.center, tip, 0.5),
                    label_dir: u,
                }
            }
            CurveKind::ArcLength => {
                let r = (c.radius + self.offset).max(1e-6);
                let n = ((c.sweep.abs() / 0.12).ceil() as usize).clamp(4, 96);
                let line = c.sample(self.offset, n);
                let ext = |a: f64| {
                    let u = unit(a);
                    let out = self.offset.signum();
                    let from = c.center.add(u.scale(c.radius + gap * out));
                    let to = c.center.add(u.scale(r + past * out));
                    (from, to)
                };
                let mid = c.start + c.sweep * 0.5;
                CurveGeom {
                    extensions: vec![ext(a0), ext(a1)],
                    inward: [
                        unit(a0 + sgn * FRAC_PI_2).scale(1.0),
                        unit(a1 + sgn * FRAC_PI_2).scale(-1.0),
                    ],
                    line,
                    label_at: c.center.add(unit(mid).scale(r)),
                    label_dir: unit(mid + FRAC_PI_2),
                }
            }
            CurveKind::Angle => {
                let r = c.radius;
                let n = ((c.sweep.abs() / 0.12).ceil() as usize).clamp(4, 96);
                let line = c.sample(0.0, n);
                let ext = |a: f64| {
                    let u = unit(a);
                    (
                        c.center.add(u.scale(gap)),
                        c.center.add(u.scale(r + past.max(6.0))),
                    )
                };
                let mid = c.start + c.sweep * 0.5;
                CurveGeom {
                    extensions: vec![ext(a0), ext(a1)],
                    inward: [
                        unit(a0 + sgn * FRAC_PI_2),
                        unit(a1 + sgn * FRAC_PI_2).scale(-1.0),
                    ],
                    line,
                    label_at: c.center.add(unit(mid).scale(r)),
                    label_dir: unit(mid + FRAC_PI_2),
                }
            }
        })
    }

    /// Moves the dimension, its curve and its label (a moved label rides
    /// along: it is kept relative to the line).
    pub fn translate(&mut self, d: Point) {
        self.start = self.start.add(d);
        self.end = self.end.add(d);
        if let Some(c) = self.look.seg.curve.as_mut() {
            c.center = c.center.add(d);
        }
    }

    /// Turns the dimension about `pivot` by `angle` radians
    /// (counter-clockwise). A dimension that is turned lets go of the
    /// objects it was tied to: its points are no longer on them.
    pub fn rotate_about(&mut self, pivot: Point, angle: f64) {
        let (s, c) = angle.sin_cos();
        let turn = |p: Point| {
            let v = p.sub(pivot);
            pivot.add(Point::new(v.x * c - v.y * s, v.x * s + v.y * c))
        };
        self.start = turn(self.start);
        self.end = turn(self.end);
        self.anchors = [None, None];
        if let Some(cv) = self.look.seg.curve.as_mut() {
            cv.center = turn(cv.center);
            cv.start += angle;
            cv.walls = [None, None];
        }
    }
}

use std::f64::consts::FRAC_PI_2;

impl Floor {
    /// The dimension ids to act on when the user picks `id`: the whole string
    /// it belongs to (Chief selects, moves, copies and deletes a dimension
    /// line as one object).
    pub fn pick_dimension_string(&self, id: Id) -> Vec<Id> {
        self.string_members(id)
    }

    /// Moves every segment of the string of `id` by `d`; returns how many
    /// moved.
    pub fn move_string(&mut self, id: Id, d: Point) -> usize {
        let members = self.string_members(id);
        let mut n = 0;
        for m in members {
            if let Some(dim) = self.dimensions.iter_mut().find(|x| x.id == m) {
                dim.translate(d);
                n += 1;
            }
        }
        n
    }

    /// Turns every segment of the string of `id` about `pivot`.
    pub fn rotate_string(&mut self, id: Id, pivot: Point, angle: f64) -> usize {
        let members = self.string_members(id);
        let mut n = 0;
        for m in members {
            if let Some(dim) = self.dimensions.iter_mut().find(|x| x.id == m) {
                dim.rotate_about(pivot, angle);
                n += 1;
            }
        }
        n
    }

    /// After a paste: the copies (`pairs` of old and new ids) get strings of
    /// their own among themselves, and a curved dimension is tied to the
    /// copy of its wall (`wall_copy`: old wall id to new) or, when the wall
    /// was not copied, to nothing.
    pub fn repair_pasted_dimensions(
        &mut self,
        pairs: &[(Id, Id)],
        wall_copy: &dyn Fn(Id) -> Option<Id>,
    ) {
        let mut groups: Vec<(Id, Vec<Id>)> = Vec::new();
        for (old, new) in pairs {
            let Some(d) = self.dimensions.iter_mut().find(|d| d.id == *new) else {
                continue;
            };
            let was = d.look.seg.string.take();
            if let Some(c) = d.look.seg.curve.as_mut() {
                for w in c.walls.iter_mut() {
                    *w = w.and_then(wall_copy);
                }
            }
            let _ = old;
            if let Some(s) = was {
                match groups.iter_mut().find(|(g, _)| *g == s) {
                    Some((_, v)) => v.push(*new),
                    None => groups.push((s, vec![*new])),
                }
            }
        }
        for (_, ids) in groups {
            self.join_string(&ids);
        }
    }

    /// Drags the dimension line of `id` to the pointer at `to` (`snap` is
    /// the grid to round the distance to). A straight dimension moves its
    /// whole string's line; an arc length or radius sets how far its line
    /// stands from the arc; an angle sets the radius of its arc. Returns
    /// whether the dimension exists. A line with a Fixed Proximity
    /// extension cannot be moved by hand (manual p. 505).
    pub fn drag_dimension_line(&mut self, id: Id, to: Point, snap: Option<f64>) -> bool {
        if self.line_is_fixed(id) {
            return self.dimensions.iter().any(|d| d.id == id);
        }
        let round = |v: f64| snap.map_or(v, |u| (v / u.max(1e-9)).round() * u.max(1e-9));
        let Some(d) = self.dimensions.iter_mut().find(|d| d.id == id) else {
            return false;
        };
        if let Some(c) = d.look.seg.curve {
            let dist = round(to.dist(c.center));
            match c.kind {
                CurveKind::Angle => {
                    let nc = DimCurve {
                        radius: dist.max(6.0),
                        ..c
                    };
                    let nd = Dimension::curved(d.kind, nc, d.offset);
                    d.start = nd.start;
                    d.end = nd.end;
                    d.look.seg.curve = Some(nc);
                }
                CurveKind::ArcLength => d.offset = dist - c.radius,
                CurveKind::Radius => d.offset = (dist - c.radius).max(0.0),
            }
            return true;
        }
        let dir = d.end.sub(d.start).normalized();
        let off = round(to.sub(d.start).dot(dir.perp()));
        let through = d.start.add(dir.perp().scale(off));
        d.offset = off;
        self.set_string_line(id, through);
        true
    }

    /// Puts the dimension line of the string of `id` through `through`:
    /// every member's offset is set so its line passes the point (the
    /// members share one line, so they stay one string). Returns how many
    /// segments changed.
    pub fn set_string_line(&mut self, id: Id, through: Point) -> usize {
        let members = self.string_members(id);
        let mut n = 0;
        for m in members {
            if let Some(d) = self.dimensions.iter_mut().find(|x| x.id == m) {
                if d.curve().is_some() || d.length() < 1e-9 {
                    continue;
                }
                let perp = d.end.sub(d.start).normalized().perp();
                let off = through.sub(d.start).dot(perp);
                if (off - d.offset).abs() > 1e-9 {
                    d.offset = off;
                    n += 1;
                }
            }
        }
        n
    }

    /// Makes a string of every run of segments among `ids` that continue
    /// each other on one dimension line and are not in a string yet (the
    /// strings an automatic run or a continued manual dimension draws).
    /// Returns how many strings were made.
    pub fn join_chains(&mut self, ids: &[Id]) -> usize {
        let chains: Vec<Vec<Id>> = self
            .dimension_chains()
            .into_iter()
            .map(|c| {
                c.into_iter()
                    .map(|i| self.dimensions[i].id)
                    .collect::<Vec<_>>()
            })
            .filter(|c: &Vec<Id>| {
                c.iter().all(|id| {
                    ids.contains(id)
                        && self
                            .dimensions
                            .iter()
                            .find(|d| d.id == *id)
                            .is_some_and(|d| d.string_id().is_none())
                })
            })
            .collect();
        chains
            .iter()
            .filter(|c| self.join_string(c).is_some())
            .count()
    }

    /// Brings every dimension up to date with the objects it measures: tied
    /// ends, curved dimensions that follow walls and Grid Rounding. The
    /// editor runs it whenever the plan changes (Auto Refresh). Returns
    /// whether anything changed.
    pub fn refresh_dimensions(&mut self, fmt: &DimFormat) -> bool {
        let mut changed = self.sync_dimension_anchors();
        changed |= self.enforce_fixed_proximity();
        changed |= self.sync_dimension_curves();
        changed |= self.regrid_dimensions(fmt);
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::super::DimensionKind;
    use super::*;
    use crate::model::{Project, WallKind};
    use crate::walls::WallCurve;

    fn wall(p: &mut Project, a: (f64, f64), b: (f64, f64)) -> Id {
        p.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            6.0,
            96.0,
            WallKind::Exterior,
        )
    }

    #[test]
    fn an_angle_between_walls_reads_the_side_that_was_clicked() {
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (120.0, 0.0));
        let b = wall(&mut p, (0.0, 0.0), (0.0, 120.0));
        let (wa, wb) = (
            p.floors[0].wall(a).unwrap().clone(),
            p.floors[0].wall(b).unwrap().clone(),
        );
        let near =
            DimCurve::between_walls(&wa, &wb, Point::new(60.0, 0.0), Point::new(0.0, 60.0), 30.0)
                .unwrap();
        assert!((near.value() - 90.0).abs() < 1e-6);
        // Clicking the far side of the first wall gives the obtuse angle.
        let wide = DimCurve::between_walls(
            &wa,
            &wb,
            Point::new(-60.0, 0.0),
            Point::new(0.0, 60.0),
            30.0,
        )
        .unwrap();
        assert!(
            (wide.value() - 90.0).abs() < 1e-6,
            "both arms are perpendicular"
        );
        let mut d = Dimension::curved(DimensionKind::Manual, near, 0.0);
        let fmt = DimFormat::default();
        assert_eq!(d.label(&fmt), "90.0\u{b0}");
        let g = d.curve_geom(1.0, 6.0).unwrap();
        assert_eq!(g.extensions.len(), 2);
        assert!(g.line.len() > 4);
        assert!(g
            .line
            .iter()
            .all(|q| (q.dist(Point::ZERO) - 30.0).abs() < 1e-6));
        // Turning it lets go of the walls and carries the arc.
        d.rotate_about(Point::ZERO, FRAC_PI_2);
        let c = d.curve().unwrap();
        assert!((c.start - FRAC_PI_2).abs() < 1e-9);
        assert!(c.walls.iter().all(Option::is_none));
    }

    #[test]
    fn an_obtuse_angle_between_walls() {
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (120.0, 0.0));
        let b = wall(&mut p, (0.0, 0.0), (-85.0, 85.0));
        let (wa, wb) = (
            p.floors[0].wall(a).unwrap().clone(),
            p.floors[0].wall(b).unwrap().clone(),
        );
        let c = DimCurve::between_walls(
            &wa,
            &wb,
            Point::new(60.0, 0.0),
            Point::new(-40.0, 40.0),
            24.0,
        )
        .unwrap();
        assert!((c.value() - 135.0).abs() < 1e-6, "{}", c.value());
    }

    #[test]
    fn radius_and_arc_length_of_a_curved_wall_come_from_the_wall() {
        let mut p = Project::new("t");
        let id = wall(&mut p, (0.0, 0.0), (240.0, 0.0));
        assert!(DimCurve::of_wall(p.floors[0].wall(id).unwrap(), CurveKind::Radius, 0.0).is_none());
        p.floors[0].wall_mut(id).unwrap().curve = Some(WallCurve { bulge: 60.0 });
        let w = p.floors[0].wall(id).unwrap().clone();
        let arc = DimCurve::of_wall(&w, CurveKind::ArcLength, 0.0).unwrap();
        assert!((arc.value() - w.path_length()).abs() < 1e-6);
        // The outer surface (3 inches to the right of an arc that bulges left
        // is farther from the center or nearer, depending on the side).
        let inner = DimCurve::of_wall(&w, CurveKind::Radius, 3.0).unwrap();
        let outer = DimCurve::of_wall(&w, CurveKind::Radius, -3.0).unwrap();
        assert!((inner.radius - outer.radius).abs() > 5.9);
        let d = Dimension::curved(DimensionKind::Manual, arc, 18.0);
        let g = d.curve_geom(1.0, 4.0).unwrap();
        assert!(
            g.line.first().unwrap().dist(w.start) > 15.0,
            "the line stands off the wall"
        );
        let rad = Dimension::curved(
            DimensionKind::Manual,
            DimCurve {
                kind: CurveKind::Radius,
                ..arc
            },
            0.0,
        );
        let g = rad.curve_geom(1.0, 4.0).unwrap();
        assert_eq!(g.line.len(), 2);
        assert!(g.line[0].dist(arc.center) < 1e-9);
        assert!((g.line[1].dist(arc.center) - arc.radius).abs() < 1e-6);
    }

    #[test]
    fn a_string_moves_and_turns_as_one() {
        let mut p = Project::new("t");
        let mk = |a: f64, b: f64| {
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(a, 0.0),
                Point::new(b, 0.0),
                24.0,
            )
        };
        let a = p.add_dimension(0, mk(0.0, 100.0));
        let b = p.add_dimension(0, mk(100.0, 220.0));
        let lone = p.add_dimension(0, mk(0.0, 50.0));
        assert_eq!(p.floors[0].join_string(&[a, b]), Some(a));
        assert_eq!(p.floors[0].pick_dimension_string(b), vec![a, b]);
        assert_eq!(p.floors[0].pick_dimension_string(lone), vec![lone]);
        assert_eq!(p.floors[0].move_string(b, Point::new(0.0, 10.0)), 2);
        let get = |p: &Project, id: Id| {
            p.floors[0]
                .dimensions
                .iter()
                .find(|d| d.id == id)
                .unwrap()
                .clone()
        };
        assert_eq!(get(&p, a).start, Point::new(0.0, 10.0));
        assert_eq!(get(&p, b).end, Point::new(220.0, 10.0));
        assert_eq!(get(&p, lone).start, Point::new(0.0, 0.0));
        assert_eq!(p.floors[0].rotate_string(a, Point::ZERO, FRAC_PI_2), 2);
        let d = get(&p, b);
        assert!(d.end.dist(Point::new(-10.0, 220.0)) < 1e-9, "{:?}", d.end);
    }

    #[test]
    fn dragging_a_string_line_moves_every_segment() {
        let mut p = Project::new("t");
        let mk = |a: f64, b: f64| {
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(a, 0.0),
                Point::new(b, 0.0),
                24.0,
            )
        };
        let a = p.add_dimension(0, mk(0.0, 100.0));
        let b = p.add_dimension(0, mk(100.0, 220.0));
        p.floors[0].join_string(&[a, b]);
        assert_eq!(p.floors[0].set_string_line(b, Point::new(50.0, 40.0)), 2);
        for id in [a, b] {
            let d = p.floors[0].dimensions.iter().find(|d| d.id == id).unwrap();
            assert!((d.offset - 40.0).abs() < 1e-9);
        }
    }

    #[test]
    fn pasted_strings_stay_strings_and_curves_follow_the_copied_wall() {
        let mut p = Project::new("t");
        let w1 = wall(&mut p, (0.0, 0.0), (240.0, 0.0));
        p.floors[0].wall_mut(w1).unwrap().curve = Some(WallCurve { bulge: 60.0 });
        let w = p.floors[0].wall(w1).unwrap().clone();
        let c = DimCurve::of_wall(&w, CurveKind::ArcLength, 0.0).unwrap();
        let curved = p.add_dimension(0, Dimension::curved(DimensionKind::Manual, c, 12.0));
        let mk = |a: f64, b: f64| {
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(a, 0.0),
                Point::new(b, 0.0),
                24.0,
            )
        };
        let a = p.add_dimension(0, mk(0.0, 100.0));
        let b = p.add_dimension(0, mk(100.0, 220.0));
        p.floors[0].join_string(&[a, b]);
        // The copies, as a paste makes them: same strings, the old walls.
        let copy = |p: &mut Project, id: Id| {
            let d = p.floors[0]
                .dimensions
                .iter()
                .find(|d| d.id == id)
                .unwrap()
                .clone();
            p.add_dimension(0, d)
        };
        let (a2, b2, c2) = (copy(&mut p, a), copy(&mut p, b), copy(&mut p, curved));
        let w2 = wall(&mut p, (500.0, 0.0), (740.0, 0.0));
        p.floors[0].repair_pasted_dimensions(&[(a, a2), (b, b2), (curved, c2)], &|w| {
            (w == w1).then_some(w2)
        });
        assert_eq!(p.floors[0].string_members(a2), vec![a2, b2]);
        assert_eq!(
            p.floors[0].string_members(a),
            vec![a, b],
            "the originals keep theirs"
        );
        let cd = p.floors[0].dimensions.iter().find(|d| d.id == c2).unwrap();
        assert_eq!(cd.curve().unwrap().walls[0], Some(w2));
        // A wall that was not copied lets the curve go.
        p.floors[0].repair_pasted_dimensions(&[(curved, c2)], &|_| None);
        let cd = p.floors[0].dimensions.iter().find(|d| d.id == c2).unwrap();
        assert_eq!(cd.curve().unwrap().walls[0], None);
    }

    #[test]
    fn dragging_a_curved_dimension_line_sets_its_distance() {
        let mut p = Project::new("t");
        let id = wall(&mut p, (0.0, 0.0), (240.0, 0.0));
        p.floors[0].wall_mut(id).unwrap().curve = Some(WallCurve { bulge: 60.0 });
        let w = p.floors[0].wall(id).unwrap().clone();
        let c = DimCurve::of_wall(&w, CurveKind::ArcLength, 0.0).unwrap();
        let did = p.add_dimension(0, Dimension::curved(DimensionKind::Manual, c, 0.0));
        let to = c.center.add(Point::new(0.0, c.radius + 30.0));
        assert!(p.floors[0].drag_dimension_line(did, to, None));
        let d = p.floors[0].dimensions.iter().find(|d| d.id == did).unwrap();
        assert!((d.offset - 30.0).abs() < 1e-9);
        // An angle's pull sets its arc radius and keeps its ends on the arc.
        let a = DimCurve::from_points(
            Point::ZERO,
            Point::new(100.0, 0.0),
            Point::new(0.0, 100.0),
            20.0,
        )
        .unwrap();
        let aid = p.add_dimension(0, Dimension::curved(DimensionKind::Manual, a, 0.0));
        assert!(p.floors[0].drag_dimension_line(aid, Point::new(40.0, 0.0), Some(1.0)));
        let d = p.floors[0].dimensions.iter().find(|d| d.id == aid).unwrap();
        assert!((d.curve().unwrap().radius - 40.0).abs() < 1e-9);
        assert!(d.start.dist(Point::new(40.0, 0.0)) < 1e-9);
        assert!(!p.floors[0].drag_dimension_line(999, Point::ZERO, None));
    }

    #[test]
    fn runs_that_continue_each_other_become_strings() {
        let mut p = Project::new("t");
        let mk = |a: f64, b: f64, off: f64| {
            Dimension::new(
                0,
                DimensionKind::AutoExterior,
                Point::new(a, 0.0),
                Point::new(b, 0.0),
                off,
            )
        };
        let a = p.add_dimension(0, mk(0.0, 100.0, 24.0));
        let b = p.add_dimension(0, mk(100.0, 220.0, 24.0));
        let c = p.add_dimension(0, mk(220.0, 300.0, 24.0));
        let other = p.add_dimension(0, mk(0.0, 300.0, 48.0));
        assert_eq!(p.floors[0].join_chains(&[a, b, c, other]), 1);
        assert_eq!(p.floors[0].string_members(c), vec![a, b, c]);
        assert_eq!(p.floors[0].string_members(other), vec![other]);
        // Already in a string: a second pass makes none.
        assert_eq!(p.floors[0].join_chains(&[a, b, c, other]), 0);
    }

    #[test]
    fn refreshing_follows_walls_curves_and_rounding() {
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (120.0, 0.0));
        let b = wall(&mut p, (0.0, 0.0), (0.0, 120.0));
        let (wa, wb) = (
            p.floors[0].wall(a).unwrap().clone(),
            p.floors[0].wall(b).unwrap().clone(),
        );
        let c =
            DimCurve::between_walls(&wa, &wb, Point::new(60.0, 0.0), Point::new(0.0, 60.0), 30.0)
                .unwrap();
        let id = p.add_dimension(0, Dimension::curved(DimensionKind::Manual, c, 0.0));
        let fmt = DimFormat::default();
        assert!(!p.floors[0].refresh_dimensions(&fmt));
        p.floors[0].wall_mut(b).unwrap().end = Point::new(85.0, 85.0);
        assert!(p.floors[0].refresh_dimensions(&fmt));
        let d = p.floors[0].dimensions.iter().find(|d| d.id == id).unwrap();
        assert_eq!(d.label(&fmt), "45.0\u{b0}");
    }
}
