//! Road geometry beyond the plain strip (manual pp. 1349-1355): flares where a
//! road meets another, polyline roads and other outline shapes, medians,
//! cul-de-sacs and the Auto Generate Sidewalk command.

use plan_core::geometry::polygon_area;
use plan_core::Point;

use crate::geom::{dedup_points, strip_edges, StripEdges};
use crate::landscape::path_length;
use crate::landscape_plan::circle_points;
use crate::model::{RoadKind, RoadStrip, Terrain};

/// Corners of a cul-de-sac's circle.
pub const CUL_DE_SAC_CORNERS: usize = 36;
/// Samples across a flare's fillet.
const FLARE_STEPS: usize = 8;
/// A road end this close to a click is "the end of the road", inches.
const END_REACH: f64 = 36.0;
/// Road ends closer than this are one junction, inches.
const JUNCTION: f64 = 2.0;
/// Default flare radius, inches.
pub const DEFAULT_FLARE: f64 = 24.0;

/// Extra offset of a flared edge at `d` inches from the flared end: a quarter
/// circle of radius `r` that is tangent to the road edge `r` back from the
/// end and meets the crossing road square on at the end.
pub fn flare_offset(r: f64, d: f64) -> f64 {
    if r <= 0.0 || d >= r {
        return 0.0;
    }
    let d = d.max(0.0);
    r - (r * r - (r - d) * (r - d)).max(0.0).sqrt()
}

/// The strip edges of `road` with its flares: both ends can widen into a
/// fillet of the radius named in `flare_start` / `flare_end`.
pub(crate) fn flared_edges(road: &RoadStrip) -> StripEdges {
    let base = strip_edges(&road.centerline, road.width / 2.0);
    let n = base.center.len();
    let total = path_length(&base.center);
    let r0 = road.flare_start.filter(|r| *r > 0.5);
    let r1 = road.flare_end.filter(|r| *r > 0.5);
    if n < 2 || total < 1.0 || (r0.is_none() && r1.is_none()) {
        return base;
    }
    let cap = total / 2.0;
    let (r0, r1) = (r0.map(|r| r.min(cap)), r1.map(|r| r.min(cap)));
    // Stations (distance along the path) of the base points, plus the flare steps.
    let mut along = vec![0.0];
    for w in base.center.windows(2) {
        along.push(along[along.len() - 1] + w[0].dist(w[1]));
    }
    let mut stations: Vec<f64> = along.clone();
    for (r, from_end) in [(r0, false), (r1, true)] {
        if let Some(r) = r {
            for k in 1..FLARE_STEPS {
                let d = r * k as f64 / FLARE_STEPS as f64;
                stations.push(if from_end { total - d } else { d });
            }
        }
    }
    stations.sort_by(f64::total_cmp);
    stations.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let at = |pts: &[Point], s: f64| -> Point {
        let i = along
            .partition_point(|a| *a <= s)
            .saturating_sub(1)
            .min(n - 2);
        let span = (along[i + 1] - along[i]).max(1e-9);
        Point::lerp(pts[i], pts[i + 1], ((s - along[i]) / span).clamp(0.0, 1.0))
    };
    let mut out = StripEdges {
        center: Vec::new(),
        left: Vec::new(),
        right: Vec::new(),
    };
    for s in stations {
        let (c, l, r) = (at(&base.center, s), at(&base.left, s), at(&base.right, s));
        let mut extra = 0.0;
        if let Some(r0) = r0 {
            extra += flare_offset(r0, s);
        }
        if let Some(r1) = r1 {
            extra += flare_offset(r1, total - s);
        }
        let push_out = |p: Point| {
            let v = p.sub(c);
            let len = v.length();
            if len < 1e-9 {
                p
            } else {
                p.add(v.scale(extra / len))
            }
        };
        out.center.push(c);
        out.left.push(push_out(l));
        out.right.push(push_out(r));
    }
    out
}

/// The closed outline a road covers: its own shape for outline kinds, else
/// the left edge out and the right edge back.
pub fn road_polygon(road: &RoadStrip) -> Vec<Point> {
    if road.outline.len() >= 3 {
        return dedup_points(&road.outline, true);
    }
    if road.kind == RoadKind::CulDeSac && road.radius > 0.0 {
        return circle_points(road.center, road.radius, CUL_DE_SAC_CORNERS, 0.0);
    }
    if road.width <= 0.0 || road.centerline.len() < 2 {
        return Vec::new();
    }
    let e = flared_edges(road);
    let mut poly = e.left;
    poly.extend(e.right.into_iter().rev());
    poly
}

/// Length of the road's path or outline and the area it covers (square inches).
pub fn road_length_and_area(road: &RoadStrip) -> (f64, f64) {
    if road.outline.len() >= 3 || road.kind == RoadKind::CulDeSac {
        let poly = road_polygon(road);
        let closed = if poly.len() >= 2 {
            path_length(&poly) + poly[poly.len() - 1].dist(poly[0])
        } else {
            0.0
        };
        return (closed, polygon_area(&poly).abs());
    }
    let len = path_length(&road.centerline);
    (len, len * road.width.max(0.0))
}

/// A rectangle as a road outline: the strip of `width` inches along `a`-`b`.
pub fn rectangle_along(a: Point, b: Point, width: f64) -> Vec<Point> {
    let dir = b.sub(a).normalized();
    let n = dir.perp().scale(width / 2.0);
    vec![a.add(n), b.add(n), b.sub(n), a.sub(n)]
}

/// A cul-de-sac of `radius` inches around `center`.
pub fn cul_de_sac(center: Point, radius: f64) -> RoadStrip {
    RoadStrip {
        kind: RoadKind::CulDeSac,
        center,
        radius,
        outline: circle_points(center, radius, CUL_DE_SAC_CORNERS, 0.0),
        curb: true,
        curb_height: 6.0,
        ..RoadStrip::default()
    }
}

/// A road end near `click` (strip roads only: a cul-de-sac cannot attach to a
/// polyline road): the end point and the width of the road that ends there.
pub fn road_end_near(t: &Terrain, click: Point, reach: f64) -> Option<(Point, f64)> {
    let reach = reach.max(END_REACH);
    t.roads
        .iter()
        .filter(|r| r.kind == RoadKind::Road && r.outline.len() < 3 && r.centerline.len() >= 2)
        .flat_map(|r| {
            [
                (r.centerline[0], r.width),
                (r.centerline[r.centerline.len() - 1], r.width),
            ]
        })
        .map(|(p, w)| (p.dist(click), p, w))
        .filter(|(d, _, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p, w)| (p, w))
}

/// A cul-de-sac on the end of the road nearest `click`, sized from the road
/// (twice its width, at least 20 ft across).
pub fn cul_de_sac_at(t: &Terrain, click: Point, reach: f64) -> Option<RoadStrip> {
    let (end, width) = road_end_near(t, click, reach)?;
    let radius = (width * 1.5).max(120.0);
    let mut road = cul_de_sac(end, radius);
    road.material = t
        .roads
        .iter()
        .find(|r| r.kind == RoadKind::Road && !r.material.trim().is_empty())
        .map(|r| r.material.clone())
        .unwrap_or_default();
    Some(road)
}

/// The Auto Generate Sidewalk options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoSidewalk {
    pub left: bool,
    pub right: bool,
    /// Also the roads joined end to end to the chosen one.
    pub all_connected: bool,
    /// Gap between the road's edge and the sidewalk, inches.
    pub offset: f64,
    /// Width of the sidewalks, inches.
    pub width: f64,
}

impl Default for AutoSidewalk {
    fn default() -> Self {
        AutoSidewalk {
            left: true,
            right: true,
            all_connected: true,
            offset: 0.0,
            width: 48.0,
        }
    }
}

fn ends(r: &RoadStrip) -> [Point; 2] {
    [r.centerline[0], r.centerline[r.centerline.len() - 1]]
}

/// The strip roads joined end to end to road `start` (including it).
pub fn connected_roads(t: &Terrain, start: usize) -> Vec<usize> {
    let usable =
        |r: &RoadStrip| r.kind == RoadKind::Road && r.outline.len() < 3 && r.centerline.len() >= 2;
    if !t.roads.get(start).is_some_and(usable) {
        return Vec::new();
    }
    let mut seen = vec![start];
    let mut i = 0;
    while i < seen.len() {
        let here = ends(&t.roads[seen[i]]);
        for (j, r) in t.roads.iter().enumerate() {
            if seen.contains(&j) || !usable(r) {
                continue;
            }
            if ends(r)
                .iter()
                .any(|e| here.iter().any(|h| h.dist(*e) <= JUNCTION))
            {
                seen.push(j);
            }
        }
        i += 1;
    }
    seen.sort_unstable();
    seen
}

/// Auto Generate Sidewalk: sidewalk strips along the chosen sides of road
/// `road` (and, with `all_connected`, every road joined to it). The sidewalk
/// runs parallel to the road, `offset` inches off its edge.
pub fn auto_sidewalks(t: &Terrain, road: usize, o: &AutoSidewalk) -> Vec<RoadStrip> {
    let set = if o.all_connected {
        connected_roads(t, road)
    } else {
        connected_roads(t, road)
            .into_iter()
            .filter(|i| *i == road)
            .collect()
    };
    let mut out = Vec::new();
    for i in set {
        let r = &t.roads[i];
        let d = r.width / 2.0 + o.offset.max(0.0) + o.width / 2.0;
        let edges = strip_edges(&r.centerline, d);
        for (wanted, line) in [(o.left, edges.left), (o.right, edges.right)] {
            if wanted && line.len() >= 2 {
                out.push(RoadStrip {
                    kind: RoadKind::Sidewalk,
                    centerline: line,
                    width: o.width,
                    ..RoadStrip::default()
                });
            }
        }
    }
    out
}
