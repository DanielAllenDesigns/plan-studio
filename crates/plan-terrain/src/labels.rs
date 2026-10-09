//! Labels of terrain objects (the "Terrain Labels" layer, manual p. 1321): a
//! custom or automatic text on any terrain object, the note beside an
//! Elevation Point, and the anchor points the plan and the Terrain Labels tool
//! use.

use plan_core::geometry::polygon_centroid;
use plan_core::Point;

use crate::landscape::path_length;
use crate::model::{ModifierKind, RoadKind, Terrain};
use crate::spec::{ObjectKey, DEFAULT_MARKER_RADIUS};
use crate::symbols::{Stroke, StrokeKind};

/// Layer of the labels of terrain objects.
pub const LAYER_TERRAIN_LABELS: &str = "Terrain Labels";
/// Layer of the primary (major) contour lines and their labels.
pub const LAYER_PRIMARY_CONTOURS: &str = "Terrain, Primary Contours";
/// Layer of the secondary (minor) contour lines and their labels.
pub const LAYER_SECONDARY_CONTOURS: &str = "Terrain, Secondary Contours";

/// Height of the label text, inches.
const LABEL_HEIGHT: f64 = 8.0;

/// A label placed on the plan.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelSpot {
    pub key: ObjectKey,
    pub at: Point,
    pub text: String,
    /// The note beside an Elevation Point rather than a Label panel label.
    pub note: bool,
}

fn midpoint(path: &[Point]) -> Option<Point> {
    let total = path_length(path);
    if path.is_empty() {
        return None;
    }
    let mut left = total / 2.0;
    for w in path.windows(2) {
        let len = w[0].dist(w[1]);
        if left <= len && len > 0.0 {
            return Some(Point::lerp(w[0], w[1], left / len));
        }
        left -= len;
    }
    path.last().copied()
}

fn centroid(poly: &[Point]) -> Option<Point> {
    (poly.len() >= 3).then(|| polygon_centroid(poly))
}

/// Where the label of `key` is anchored (before its own offset).
pub fn anchor_of(t: &Terrain, key: ObjectKey) -> Option<Point> {
    match key {
        ObjectKey::Perimeter => centroid(&t.perimeter),
        ObjectKey::Point(i) => t.elevation_points.get(i).map(|e| e.pos),
        ObjectKey::Line(i) => t.elevation_lines.get(i).and_then(|l| midpoint(&l.points)),
        ObjectKey::Region(i) => t
            .elevation_regions
            .get(i)
            .and_then(|r| centroid(&r.polygon)),
        ObjectKey::Modifier(i) => t.modifiers.get(i).and_then(|m| centroid(&m.polygon)),
        ObjectKey::Feature(i) => t.features.get(i).and_then(|f| centroid(&f.polygon)),
        ObjectKey::Break(i) => t.breaks.get(i).and_then(|b| midpoint(&b.points)),
        ObjectKey::Wall(i) => t.walls.get(i).and_then(|w| midpoint(&w.points)),
        ObjectKey::Landscape(i) => t.landscape.get(i).and_then(|l| {
            if l.is_region() {
                centroid(&l.points)
            } else {
                midpoint(&l.points)
            }
        }),
        ObjectKey::Road(i) => t.roads.get(i).and_then(|r| {
            if r.kind == RoadKind::CulDeSac {
                Some(r.center)
            } else if r.outline.len() >= 3 {
                centroid(&r.outline)
            } else {
                midpoint(&r.centerline)
            }
        }),
    }
}

/// The automatic label of `key`: its elevation for elevation data, its kind
/// and size for the rest.
pub fn auto_label(t: &Terrain, key: ObjectKey) -> String {
    let z = |v: f64| t.contour_label_units.format(v);
    match key {
        ObjectKey::Perimeter => "Terrain Perimeter".into(),
        ObjectKey::Point(i) => t.elevation_points.get(i).map_or_else(String::new, |e| z(e.z)),
        ObjectKey::Line(i) => t.elevation_lines.get(i).map_or_else(String::new, |l| z(l.z)),
        ObjectKey::Region(i) => t
            .elevation_regions
            .get(i)
            .map_or_else(String::new, |r| z(r.z)),
        ObjectKey::Break(i) => t.breaks.get(i).map_or_else(String::new, |b| z(b.z)),
        ObjectKey::Modifier(i) => t.modifiers.get(i).map_or_else(String::new, |m| match m.kind {
            ModifierKind::Hill => format!("Hill {}", z(m.height)),
            ModifierKind::Valley => format!("Valley {}", z(m.height)),
            ModifierKind::RaisedRegion => format!("Raised {}", z(m.height)),
            ModifierKind::LoweredRegion => format!("Lowered {}", z(m.height)),
            ModifierKind::FlatRegion => "Flat Region".into(),
        }),
        ObjectKey::Feature(i) => t.features.get(i).map_or_else(String::new, |f| {
            if f.kind == crate::model::FeatureKind::Hole {
                "Terrain Hole".into()
            } else if f.material.trim().is_empty() {
                format!("{} Feature", f.kind_name())
            } else {
                format!("{} Feature, {}", f.kind_name(), f.material.trim())
            }
        }),
        ObjectKey::Wall(i) => t.walls.get(i).map_or_else(String::new, |w| {
            let name = match w.kind {
                crate::landscape::WallKind::Wall => "Terrain Wall",
                crate::landscape::WallKind::Curb => "Terrain Curb",
            };
            format!("{name} {}", z(w.height))
        }),
        ObjectKey::Landscape(i) => t.landscape.get(i).map_or_else(String::new, |l| {
            if l.plant.is_empty() {
                l.name().to_string()
            } else {
                l.plant.clone()
            }
        }),
        ObjectKey::Road(i) => t.roads.get(i).map_or_else(String::new, |r| {
            if r.kind.is_outline_kind() {
                r.kind.name().to_string()
            } else {
                format!("{} {}", r.kind.name(), z(r.width))
            }
        }),
    }
}

/// The label text of `key`: the custom one when set, else the automatic one.
pub fn label_text(t: &Terrain, key: ObjectKey) -> String {
    let ex = t.extras(key);
    if ex.label.text.trim().is_empty() {
        auto_label(t, key)
    } else {
        ex.label.text.trim().to_string()
    }
}

/// Every label on the plan: the labels switched on in the Label panel and the
/// notes beside elevation points.
pub fn label_spots(t: &Terrain) -> Vec<LabelSpot> {
    let mut out = Vec::new();
    for key in t.object_keys() {
        let ex = t.extras(key);
        let Some(anchor) = anchor_of(t, key) else {
            continue;
        };
        if ex.label.shown {
            out.push(LabelSpot {
                key,
                at: anchor.add(ex.label.offset),
                text: label_text(t, key),
                note: false,
            });
        }
        if let (ObjectKey::Point(i), false) = (key, ex.note.trim().is_empty()) {
            let z = t.elevation_points.get(i).map_or(0.0, |e| e.z);
            let r = if ex.marker_radius > 0.0 {
                ex.marker_radius
            } else {
                DEFAULT_MARKER_RADIUS
            };
            out.push(LabelSpot {
                key,
                at: anchor.add(Point::new(r + 2.0, r + 2.0)),
                text: ex.note.replace("%elevation%", &t.contour_label_units.format(z)),
                note: true,
            });
        }
    }
    out
}

/// The labels as plan strokes.
pub fn label_strokes(t: &Terrain) -> Vec<Stroke> {
    label_spots(t)
        .into_iter()
        .map(|s| Stroke::Text {
            at: s.at,
            text: s.text,
            height: LABEL_HEIGHT,
            angle: 0.0,
            kind: StrokeKind::Label,
            negative: false,
        })
        .collect()
}
