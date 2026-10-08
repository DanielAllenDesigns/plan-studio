//! Plan-view symbols: perimeter, contours with elevation labels, features and road edges.

use plan_core::units::fmt_ft_in_frac;
use plan_core::Point;

use crate::contour::Contour;
use crate::geom::strip_edges;
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
    },
}

const LABEL_HEIGHT: f64 = 8.0;

/// Strokes for the terrain in plan: the perimeter, contours (major ones heavier) each
/// labeled with its elevation in feet-inches, feature outlines and road edges.
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
        for line in c.polylines.iter().filter(|l| l.len() >= 2) {
            let closed = line.first() == line.last() && line.len() > 3;
            out.push(Stroke::Polyline {
                points: line.clone(),
                closed,
                weight,
                kind,
            });
            out.push(Stroke::Text {
                at: line[line.len() / 2],
                text: label.clone(),
                height: LABEL_HEIGHT,
            });
        }
    }
    for f in t.features.iter().filter(|f| f.polygon.len() >= 3) {
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
