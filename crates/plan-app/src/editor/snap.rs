//! The snap engine (S-68..S-74, W-11).
//!
//! Priority when several apply within the snap distance: Endpoint,
//! Intersection, Midpoint, Perpendicular, On Object, then Angle (from the
//! pending start) and finally Grid. Alt suspends the angle snap only, as it
//! did before the engine existed.

use super::selection::ObjectRef;
use plan_core::geometry::{dist_to_segment, project_on_segment, segment_intersection, Point};
use plan_core::{Floor, Id, LayerSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapKind {
    Endpoint,
    Intersection,
    Midpoint,
    /// Foot of the perpendicular from the pending start onto a wall.
    Perpendicular,
    /// Anywhere on a wall centerline.
    OnObject,
    Angle,
    Grid,
    /// Nothing applied; the raw point.
    Free,
}

impl SnapKind {
    pub fn label(self) -> &'static str {
        match self {
            SnapKind::Endpoint => "Endpoint",
            SnapKind::Intersection => "Intersection",
            SnapKind::Midpoint => "Midpoint",
            SnapKind::Perpendicular => "Perpendicular",
            SnapKind::OnObject => "On Object",
            SnapKind::Angle => "Angle",
            SnapKind::Grid => "Grid",
            SnapKind::Free => "Free",
        }
    }

    /// Object snaps (the ones that attach to existing geometry).
    pub fn is_object_snap(self) -> bool {
        matches!(
            self,
            SnapKind::Endpoint
                | SnapKind::Intersection
                | SnapKind::Midpoint
                | SnapKind::Perpendicular
                | SnapKind::OnObject
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapResult {
    pub point: Point,
    pub kind: SnapKind,
    pub source: Option<ObjectRef>,
}

impl SnapResult {
    pub fn free(point: Point) -> Self {
        Self {
            point,
            kind: SnapKind::Free,
            source: None,
        }
    }
}

/// Which snaps are on (Edit > Snap Settings).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapSettings {
    /// Master switch for the object snaps.
    pub object_snaps: bool,
    pub endpoint: bool,
    pub midpoint: bool,
    pub intersection: bool,
    pub perpendicular: bool,
    pub on_object: bool,
    pub angle: bool,
    pub grid: bool,
    /// Screen-space snap distance.
    pub tolerance_px: f64,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            object_snaps: true,
            endpoint: true,
            midpoint: true,
            intersection: true,
            perpendicular: true,
            on_object: true,
            angle: true,
            grid: true,
            tolerance_px: 10.0,
        }
    }
}

/// Everything one snap needs besides the raw point.
pub struct SnapQuery<'a> {
    pub floor: &'a Floor,
    pub layers: &'a LayerSet,
    /// Snap distance in inches.
    pub tol: f64,
    pub grid_step: f64,
    pub angle_deg: f64,
    /// The pending start point (angle and perpendicular snaps measure from it).
    pub origin: Option<Point>,
    /// Alt held: no angle snap.
    pub suspend_angle: bool,
    /// Walls to ignore (the ones being dragged).
    pub exclude: &'a [Id],
}

pub fn snap_to_grid(p: Point, step: f64) -> Point {
    if step <= 0.0 {
        return p;
    }
    Point::new((p.x / step).round() * step, (p.y / step).round() * step)
}

/// Snap `p` to a multiple of `increment_deg` from `start`, then re-snap the length to `step`.
pub fn angle_snap(start: Point, p: Point, step: f64, increment_deg: f64) -> Option<Point> {
    let v = p.sub(start);
    let len = v.length();
    if len < 1e-6 {
        return None;
    }
    let inc = increment_deg.max(1.0).to_radians();
    let a = (v.angle() / inc).round() * inc;
    let len = if step > 0.0 {
        (len / step).round() * step
    } else {
        len
    };
    Some(start.add(Point::new(a.cos(), a.sin()).scale(len)))
}

fn nearest(cands: impl Iterator<Item = (Point, Id)>, raw: Point, tol: f64) -> Option<(Point, Id)> {
    cands
        .map(|(p, id)| (p, id, p.dist(raw)))
        .filter(|(_, _, d)| *d <= tol)
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(p, id, _)| (p, id))
}

/// Runs the engine for the raw cursor point `raw`.
pub fn snap(raw: Point, q: &SnapQuery, s: &SnapSettings) -> SnapResult {
    let walls: Vec<_> = q
        .floor
        .walls
        .iter()
        .filter(|w| !q.exclude.contains(&w.id) && q.layers.is_visible(&w.layer))
        .collect();
    let hit = |kind: SnapKind, found: Option<(Point, Id)>| {
        found.map(|(point, id)| SnapResult {
            point,
            kind,
            source: Some(ObjectRef::Wall(id)),
        })
    };

    if s.object_snaps {
        if s.endpoint {
            let ends = walls.iter().flat_map(|w| [(w.start, w.id), (w.end, w.id)]);
            if let Some(r) = hit(SnapKind::Endpoint, nearest(ends, raw, q.tol)) {
                return r;
            }
        }
        if s.intersection {
            let near: Vec<_> = walls
                .iter()
                .filter(|w| dist_to_segment(raw, w.start, w.end) <= q.tol * 2.0)
                .collect();
            let mut pts = Vec::new();
            for (i, a) in near.iter().enumerate() {
                for b in &near[i + 1..] {
                    if let Some((t, _)) = segment_intersection(a.start, a.end, b.start, b.end) {
                        pts.push((Point::lerp(a.start, a.end, t), a.id));
                    }
                }
            }
            if let Some(r) = hit(SnapKind::Intersection, nearest(pts.into_iter(), raw, q.tol)) {
                return r;
            }
        }
        if s.midpoint {
            let mids = walls
                .iter()
                .map(|w| (Point::lerp(w.start, w.end, 0.5), w.id));
            if let Some(r) = hit(SnapKind::Midpoint, nearest(mids, raw, q.tol)) {
                return r;
            }
        }
        if s.perpendicular {
            if let Some(o) = q.origin {
                let feet = walls.iter().filter_map(|w| {
                    let (t, foot) = project_on_segment(o, w.start, w.end);
                    ((0.0..=1.0).contains(&t) && foot.dist(o) > 1e-6).then_some((foot, w.id))
                });
                if let Some(r) = hit(SnapKind::Perpendicular, nearest(feet, raw, q.tol)) {
                    return r;
                }
            }
        }
        if s.on_object {
            let best = walls
                .iter()
                .map(|w| {
                    let (_, p) = project_on_segment(raw, w.start, w.end);
                    (p, w.id)
                })
                .filter(|(p, _)| p.dist(raw) <= q.tol)
                .min_by(|a, b| a.0.dist(raw).total_cmp(&b.0.dist(raw)));
            if let Some(r) = hit(SnapKind::OnObject, best) {
                return r;
            }
        }
    }

    if s.angle && !q.suspend_angle {
        if let Some(start) = q.origin {
            if let Some(point) = angle_snap(start, raw, q.grid_step, q.angle_deg) {
                return SnapResult {
                    point,
                    kind: SnapKind::Angle,
                    source: None,
                };
            }
        }
    }
    if s.grid && q.grid_step > 0.0 {
        return SnapResult {
            point: snap_to_grid(raw, q.grid_step),
            kind: SnapKind::Grid,
            source: None,
        };
    }
    SnapResult::free(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{Project, WallKind};

    fn project() -> Project {
        let mut p = Project::new("t");
        for (a, b) in [((0.0, 0.0), (120.0, 0.0)), ((60.0, -50.0), (60.0, 50.0))] {
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                4.5,
                100.0,
                WallKind::Interior,
            );
        }
        p
    }

    fn q<'a>(p: &'a Project, origin: Option<Point>) -> SnapQuery<'a> {
        SnapQuery {
            floor: &p.floors[0],
            layers: &p.layers,
            tol: 5.0,
            grid_step: 1.0,
            angle_deg: 15.0,
            origin,
            suspend_angle: false,
            exclude: &[],
        }
    }

    #[test]
    fn endpoint_beats_grid() {
        let p = project();
        let r = snap(
            Point::new(121.3, 2.2),
            &q(&p, None),
            &SnapSettings::default(),
        );
        assert_eq!(r.kind, SnapKind::Endpoint);
        assert_eq!(r.point, Point::new(120.0, 0.0));
        assert!(matches!(r.source, Some(ObjectRef::Wall(_))));
        let far = snap(
            Point::new(40.4, 30.6),
            &q(&p, None),
            &SnapSettings::default(),
        );
        assert_eq!(
            (far.kind, far.point),
            (SnapKind::Grid, Point::new(40.0, 31.0))
        );
    }

    #[test]
    fn intersection_midpoint_and_on_object() {
        let p = project();
        let s = SnapSettings::default();
        let r = snap(Point::new(61.0, 1.0), &q(&p, None), &s);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Intersection, Point::new(60.0, 0.0))
        );
        let r = snap(Point::new(30.0, 2.0), &q(&p, None), &s);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::OnObject, Point::new(30.0, 0.0))
        );
        let r = snap(Point::new(60.0, 47.0), &q(&p, None), &s);
        assert_eq!(r.kind, SnapKind::Endpoint);
        let r = snap(Point::new(60.0, 24.0), &q(&p, None), &s);
        assert_eq!(r.kind, SnapKind::OnObject);
    }

    #[test]
    fn perpendicular_from_the_pending_start() {
        let p = project();
        let origin = Some(Point::new(30.0, 40.0));
        let r = snap(
            Point::new(31.0, 2.0),
            &q(&p, origin),
            &SnapSettings::default(),
        );
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Perpendicular, Point::new(30.0, 0.0))
        );
    }

    #[test]
    fn angle_snap_unless_suspended() {
        let p = project();
        let origin = Some(Point::new(0.0, 100.0));
        let raw = Point::new(100.0, 105.0);
        let r = snap(raw, &q(&p, origin), &SnapSettings::default());
        assert_eq!(r.kind, SnapKind::Angle);
        assert!((r.point.y - 100.0).abs() < 1e-6);
        let mut query = q(&p, origin);
        query.suspend_angle = true;
        assert_eq!(
            snap(raw, &query, &SnapSettings::default()).kind,
            SnapKind::Grid
        );
    }

    #[test]
    fn hidden_walls_and_excluded_walls_do_not_snap() {
        let mut p = project();
        let id = p.floors[0].walls[0].id;
        let ids = [id];
        let mut query = q(&p, None);
        query.exclude = &ids;
        let r = snap(Point::new(121.0, 1.0), &query, &SnapSettings::default());
        assert_eq!(r.kind, SnapKind::Grid);
        p.layers.set_display("Walls, Normal", false);
        let r = snap(
            Point::new(121.0, 1.0),
            &q(&p, None),
            &SnapSettings::default(),
        );
        assert_eq!(r.kind, SnapKind::Grid);
    }
}
