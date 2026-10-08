//! Plan-view symbols: perimeter, contours with elevation labels, features and road edges.

use std::f64::consts::{FRAC_PI_2, PI};

use plan_core::units::fmt_ft_in_frac;
use plan_core::Point;

use crate::contour::Contour;
use crate::geom::strip_edges;
use crate::landscape::path_length;
use crate::model::Terrain;

/// What a plan stroke represents, so renderers can pick layers and colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeKind {
    Perimeter,
    Contour,
    MajorContour,
    Feature,
    RoadEdge,
}

/// A 2D drawing primitive for the Terrain layer in plan view.
#[derive(Debug, Clone, PartialEq)]
pub enum Stroke {
    Polyline {
        points: Vec<Point>,
        closed: bool,
        /// Line weight, points.
        weight: f64,
        kind: StrokeKind,
    },
    Text {
        at: Point,
        text: String,
        /// Text height, inches.
        height: f64,
        /// Rotation, radians counter-clockwise (plan y up). Contour labels
        /// follow the line but are kept readable: between -90 and 90 degrees.
        angle: f64,
    },
}

const LABEL_HEIGHT: f64 = 8.0;

/// Where the elevation labels of one contour line go: `(point, readable angle)`.
///
/// With `spacing` > 0 a line gets one label per `spacing` inches of its length,
/// each centered in its stretch (a line shorter than the spacing still gets one
/// when it is long enough to hold the text); with `spacing` of 0 it gets one at
/// its middle. `text_width` is the length of the label, inches.
pub fn contour_label_spots(line: &[Point], spacing: f64, text_width: f64) -> Vec<(Point, f64)> {
    let len = path_length(line);
    if line.len() < 2 || len <= 0.0 {
        return Vec::new();
    }
    let count = if spacing > 0.0 {
        let n = (len / spacing + 1e-9).floor() as usize;
        if n == 0 && len >= text_width * 2.0 {
            1
        } else {
            n
        }
    } else {
        1
    };
    let cell = len / count.max(1) as f64;
    (0..count)
        .filter_map(|k| point_and_angle(line, (k as f64 + 0.5) * cell))
        .collect()
}

/// The point `dist` along the polyline and the readable direction there.
fn point_and_angle(line: &[Point], dist: f64) -> Option<(Point, f64)> {
    let mut left = dist;
    for w in line.windows(2) {
        let seg = w[0].dist(w[1]);
        if seg < 1e-9 {
            continue;
        }
        if left <= seg {
            let mut a = w[1].sub(w[0]).angle();
            if a > FRAC_PI_2 + 1e-9 {
                a -= PI;
            } else if a <= -FRAC_PI_2 + 1e-9 {
                a += PI;
            }
            return Some((Point::lerp(w[0], w[1], left / seg), a));
        }
        left -= seg;
    }
    None
}

/// Strokes for the terrain in plan: the perimeter, contours (major ones heavier) with
/// their elevations in feet-inches written along the lines (see
/// [`Terrain::contour_label_spacing`] and [`Terrain::contour_label_major_only`]),
/// feature outlines and road edges.
pub fn plan_symbols(t: &Terrain, contours: &[Contour]) -> Vec<Stroke> {
    let mut out = Vec::new();
    if t.perimeter.len() >= 2 {
        out.push(Stroke::Polyline {
            points: t.perimeter.clone(),
            closed: true,
            weight: 1.5,
            kind: StrokeKind::Perimeter,
        });
    }
    for c in contours {
        let (weight, kind) = if c.major {
            (1.0, StrokeKind::MajorContour)
        } else {
            (0.35, StrokeKind::Contour)
        };
        let label = fmt_ft_in_frac(c.z, 2);
        let labeled = c.major || !t.contour_label_major_only;
        let width = label.chars().count() as f64 * LABEL_HEIGHT * 0.6;
        for line in c.polylines.iter().filter(|l| l.len() >= 2) {
            let closed = line.first() == line.last() && line.len() > 3;
            out.push(Stroke::Polyline {
                points: line.clone(),
                closed,
                weight,
                kind,
            });
            if !labeled {
                continue;
            }
            for (at, angle) in contour_label_spots(line, t.contour_label_spacing, width) {
                out.push(Stroke::Text {
                    at,
                    text: label.clone(),
                    height: LABEL_HEIGHT,
                    angle,
                });
            }
        }
    }
    // Terrain holes only: the other features draw through `landscape_plan`.
    for f in t
        .features
        .iter()
        .filter(|f| f.polygon.len() >= 3 && f.kind == crate::model::FeatureKind::Hole)
    {
        out.push(Stroke::Polyline {
            points: f.polygon.clone(),
            closed: true,
            weight: 0.5,
            kind: StrokeKind::Feature,
        });
    }
    for road in t.roads.iter().filter(|r| r.width > 0.0) {
        let edges = strip_edges(&road.centerline, road.width / 2.0);
        if edges.center.len() < 2 {
            continue;
        }
        for side in [edges.left, edges.right] {
            out.push(Stroke::Polyline {
                points: side,
                closed: false,
                weight: if road.curb { 0.7 } else { 0.5 },
                kind: StrokeKind::RoadEdge,
            });
        }
    }
    out
}
