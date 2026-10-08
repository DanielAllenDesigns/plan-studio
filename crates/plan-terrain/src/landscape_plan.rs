//! Plan-view drawing of the landscape objects: outlines, fills, hatching, stone,
//! plant and sprinkler symbols. Every item names its Chief layer so the plan
//! renderer can honor the layer's display switch.

use std::f64::consts::PI;

use plan_3d::triangulate::ear_clip;
use plan_core::units::fmt_ft_in_frac;
use plan_core::Point;

use crate::geom::{dedup_points, offset_polygon, strip_edges};
use crate::landscape::{
    FillStyle, Landscape, LandscapeKind, ObjectStyle, TerrainBreak, TerrainWall, WallKind,
    LAYER_BREAKS, LAYER_FEATURES,
};
use crate::model::{Feature, FeatureKind, Terrain};

/// A 2D drawing primitive.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanShape {
    Polyline {
        points: Vec<Point>,
        closed: bool,
        color: [u8; 3],
        /// Line weight, points.
        weight: f64,
        dashed: bool,
    },
    /// A filled region, already cut into triangles.
    Fill {
        triangles: Vec<[Point; 3]>,
        /// RGBA.
        color: [u8; 4],
    },
    Text {
        at: Point,
        text: String,
        /// Text height, inches.
        height: f64,
        color: [u8; 3],
    },
}

/// A shape and the layer it is drawn on.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanItem {
    pub layer: String,
    pub shape: PlanShape,
}

const FEATURE_COLOR: [u8; 3] = [0x3C, 0x7F, 0xA8];
const WALL_COLOR: [u8; 3] = [0x60, 0x60, 0x66];
const BREAK_COLOR: [u8; 3] = [0xB0, 0x40, 0x30];
const BED_COLOR: [u8; 3] = [0x8B, 0x5A, 0x2B];
const GRASS_COLOR: [u8; 3] = [0x4F, 0x8F, 0x3A];
const WATER_COLOR: [u8; 3] = [0x2F, 0x7F, 0xC0];
const STONE_COLOR: [u8; 3] = [0x7A, 0x77, 0x70];
const PLANT_COLOR: [u8; 3] = [0x2E, 0x7D, 0x32];
const SPRINKLER_COLOR: [u8; 3] = [0x1E, 0x88, 0xE5];
const LABEL_HEIGHT: f64 = 8.0;
const SOLID_ALPHA: u8 = 90;

/// A circle as a closed polyline, `n` corners, starting at angle `start`.
pub fn circle_points(c: Point, r: f64, n: usize, start: f64) -> Vec<Point> {
    (0..n)
        .map(|i| {
            let a = start + 2.0 * PI * i as f64 / n as f64;
            Point::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

/// Parallel hatch segments clipped to `poly`: lines `spacing` apart at `angle`
/// radians (even-odd across concave outlines).
pub fn hatch_segments(poly: &[Point], spacing: f64, angle: f64) -> Vec<[Point; 2]> {
    let pts = dedup_points(poly, true);
    if pts.len() < 3 || spacing <= 0.0 {
        return Vec::new();
    }
    let (s, c) = (angle.sin(), angle.cos());
    // Into the hatch frame (rotate by -angle) and back.
    let to = |p: Point| Point::new(p.x * c + p.y * s, -p.x * s + p.y * c);
    let from = |p: Point| Point::new(p.x * c - p.y * s, p.x * s + p.y * c);
    let rot: Vec<Point> = pts.iter().copied().map(to).collect();
    let (lo, hi) = rot
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        });
    let mut out = Vec::new();
    let mut y = (lo / spacing).floor() * spacing + spacing;
    while y < hi {
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..rot.len() {
            let (a, b) = (rot[i], rot[(i + 1) % rot.len()]);
            if (a.y <= y) != (b.y <= y) {
                xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.as_chunks::<2>().0 {
            out.push([from(Point::new(pair[0], y)), from(Point::new(pair[1], y))]);
        }
        y += spacing;
    }
    out
}

struct Look {
    line: [u8; 3],
    weight: f64,
    dashed: bool,
}

fn look(style: &ObjectStyle, color: [u8; 3], weight: f64, dashed: bool) -> Look {
    Look {
        line: style.line_color.unwrap_or(color),
        weight: if style.line_weight > 0.0 {
            style.line_weight
        } else {
            weight
        },
        dashed: style.dashed || dashed,
    }
}

struct Sink<'a> {
    layer: &'a str,
    items: &'a mut Vec<PlanItem>,
}

impl Sink<'_> {
    fn push(&mut self, shape: PlanShape) {
        self.items.push(PlanItem {
            layer: self.layer.to_string(),
            shape,
        });
    }

    fn line(&mut self, points: Vec<Point>, closed: bool, l: &Look) {
        if points.len() >= 2 {
            self.push(PlanShape::Polyline {
                points,
                closed,
                color: l.line,
                weight: l.weight,
                dashed: l.dashed,
            });
        }
    }

    fn text(&mut self, at: Point, text: String, color: [u8; 3]) {
        self.push(PlanShape::Text {
            at,
            text,
            height: LABEL_HEIGHT,
            color,
        });
    }

    /// The fill of a closed region: solid color or hatch lines, per `style`
    /// (`default` when the style says Default).
    fn fill(&mut self, poly: &[Point], style: &ObjectStyle, default: FillStyle, color: [u8; 3]) {
        let kind = match style.fill {
            FillStyle::Default => default,
            other => other,
        };
        let color = style.fill_color.unwrap_or(color);
        match kind {
            FillStyle::Solid => {
                let pts = dedup_points(poly, true);
                let triangles: Vec<[Point; 3]> = ear_clip(&pts)
                    .into_iter()
                    .map(|[a, b, c]| [pts[a], pts[b], pts[c]])
                    .collect();
                if !triangles.is_empty() {
                    self.push(PlanShape::Fill {
                        triangles,
                        color: [color[0], color[1], color[2], SOLID_ALPHA],
                    });
                }
            }
            FillStyle::Hatch => {
                for [a, b] in hatch_segments(poly, 12.0, PI / 4.0) {
                    self.push(PlanShape::Polyline {
                        points: vec![a, b],
                        closed: false,
                        color,
                        weight: 0.4,
                        dashed: false,
                    });
                }
            }
            FillStyle::None | FillStyle::Default => {}
        }
    }
}

/// Everything the landscape objects of `t` draw in plan, except the outlines of
/// terrain holes (those are part of [`crate::plan_symbols`]).
pub fn landscape_plan(t: &Terrain) -> Vec<PlanItem> {
    let mut items = Vec::new();
    for f in t.features.iter().filter(|f| f.kind != FeatureKind::Hole) {
        feature_plan(f, &mut items);
    }
    for b in &t.breaks {
        break_plan(b, &mut items);
    }
    for w in &t.walls {
        wall_plan(w, &mut items);
    }
    for l in &t.landscape {
        object_plan(l, &mut items);
    }
    items
}

fn centroid(pts: &[Point]) -> Point {
    let n = pts.len().max(1) as f64;
    let sum = pts.iter().fold(Point::ZERO, |a, p| a + *p);
    Point::new(sum.x / n, sum.y / n)
}

fn feature_plan(f: &Feature, items: &mut Vec<PlanItem>) {
    if f.polygon.len() < 3 {
        return;
    }
    let mut s = Sink {
        layer: f.style.layer_or(LAYER_FEATURES),
        items,
    };
    let l = look(&f.style, FEATURE_COLOR, 0.8, false);
    s.fill(&f.polygon, &f.style, FillStyle::None, FEATURE_COLOR);
    s.line(f.polygon.clone(), true, &l);
    let label = match (f.material.trim().is_empty(), f.height > 0.0) {
        (false, true) => format!("{} {}", f.material, fmt_ft_in_frac(f.height, 2)),
        (false, false) => f.material.clone(),
        (true, true) => fmt_ft_in_frac(f.height, 2),
        (true, false) => String::new(),
    };
    if !label.is_empty() {
        s.text(centroid(&f.polygon), label, l.line);
    }
}

fn break_plan(b: &TerrainBreak, items: &mut Vec<PlanItem>) {
    if b.points.len() < 2 {
        return;
    }
    let mut s = Sink {
        layer: b.style.layer_or(LAYER_BREAKS),
        items,
    };
    let l = look(&b.style, BREAK_COLOR, 1.2, true);
    s.line(b.points.clone(), false, &l);
    let mid = b.points[b.points.len() / 2];
    s.text(mid, format!("Break {}", fmt_ft_in_frac(b.z, 2)), l.line);
}

/// The closed outline of a wall: left edge out, right edge back.
pub fn wall_outline(w: &TerrainWall) -> Vec<Point> {
    let e = strip_edges(&w.points, w.thickness.max(0.1) / 2.0);
    if e.center.len() < 2 {
        return Vec::new();
    }
    e.left
        .into_iter()
        .chain(e.right.into_iter().rev())
        .collect()
}

fn wall_plan(w: &TerrainWall, items: &mut Vec<PlanItem>) {
    let outline = wall_outline(w);
    if outline.len() < 4 {
        return;
    }
    let mut s = Sink {
        layer: w.style.layer_or(w.default_layer()),
        items,
    };
    let weight = if w.kind == WallKind::Curb { 0.7 } else { 1.1 };
    let l = look(&w.style, WALL_COLOR, weight, false);
    s.fill(&outline, &w.style, FillStyle::Solid, WALL_COLOR);
    s.line(outline, true, &l);
}

fn object_plan(o: &Landscape, items: &mut Vec<PlanItem>) {
    let mut s = Sink {
        layer: o.layer(),
        items,
    };
    match o.kind {
        LandscapeKind::GardenBed => {
            let l = look(&o.style, BED_COLOR, 1.0, false);
            s.fill(&o.points, &o.style, FillStyle::Hatch, BED_COLOR);
            s.line(o.points.clone(), true, &l);
            if o.edging {
                s.line(offset_polygon(&o.points, 2.0), true, &l);
            }
            if o.points.len() >= 3 && !o.material.is_empty() {
                s.text(centroid(&o.points), o.material.clone(), l.line);
            }
        }
        LandscapeKind::GrassRegion => {
            let l = look(&o.style, GRASS_COLOR, 0.8, true);
            s.fill(&o.points, &o.style, FillStyle::Hatch, GRASS_COLOR);
            s.line(o.points.clone(), true, &l);
        }
        LandscapeKind::WaterFeature => {
            let l = look(&o.style, WATER_COLOR, 1.0, false);
            s.fill(&o.points, &o.style, FillStyle::Solid, WATER_COLOR);
            s.line(o.points.clone(), true, &l);
            if o.edging && o.size > 0.0 {
                s.line(offset_polygon(&o.points, o.size), true, &l);
            }
        }
        LandscapeKind::SteppingStones => {
            let l = look(&o.style, STONE_COLOR, 1.0, false);
            for (c, dir) in o.stones() {
                s.line(circle_points(c, o.size / 2.0, 8, dir + PI / 8.0), true, &l);
            }
        }
        LandscapeKind::Plants => {
            let l = look(&o.style, PLANT_COLOR, 0.9, false);
            for p in o.plant_positions() {
                s.line(circle_points(p, o.size / 2.0, 20, 0.0), true, &l);
                s.line(circle_points(p, o.size / 8.0, 8, 0.0), true, &l);
            }
        }
        LandscapeKind::Sprinklers => {
            let l = look(&o.style, SPRINKLER_COLOR, 0.8, false);
            for (c, facing) in o.heads() {
                s.line(circle_points(c, 3.0, 8, 0.0), true, &l);
                if o.size <= 0.0 {
                    continue;
                }
                if o.arc >= 359.5 {
                    s.line(circle_points(c, o.size, 32, 0.0), true, &l);
                } else {
                    let half = o.arc.to_radians() / 2.0;
                    let n = ((o.arc / 10.0).ceil() as usize).max(2);
                    let mut sector = vec![c];
                    sector.extend((0..=n).map(|i| {
                        let a = facing - half + 2.0 * half * i as f64 / n as f64;
                        Point::new(c.x + o.size * a.cos(), c.y + o.size * a.sin())
                    }));
                    s.line(sector, true, &l);
                }
            }
        }
    }
}
