//! Dimensions: manual, automatic exterior strings and temporary (transient) ones.

use crate::geometry::Point;
use crate::model::{Id, Wall, WallKind};
use crate::units::{fmt_ft_in_frac, format_length, LengthFormat};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimensionKind {
    Manual,
    AutoExterior,
    Temporary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    pub id: Id,
    pub kind: DimensionKind,
    /// First measured point.
    pub start: Point,
    /// Second measured point.
    pub end: Point,
    /// Signed perpendicular offset of the dimension line from the start-end
    /// line. Positive is the left side when walking start to end.
    pub offset: f64,
    #[serde(default)]
    pub text_override: Option<String>,
}

/// How dimension text is formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DimFormat {
    /// Smallest fraction denominator shown: 8 or 16.
    pub smallest_fraction: u32,
    /// Show `'` and `"` unit indicators.
    pub unit_indicators: bool,
    /// Full length format (imperial or metric). When set it takes over
    /// from `smallest_fraction` / `unit_indicators`.
    #[serde(default)]
    pub length: Option<LengthFormat>,
}

impl Default for DimFormat {
    fn default() -> Self {
        Self {
            smallest_fraction: 16,
            unit_indicators: true,
            length: None,
        }
    }
}

impl DimFormat {
    /// Whole-millimetre dimension text with no unit marks, e.g. `3048`.
    pub fn metric_mm() -> Self {
        Self {
            length: Some(LengthFormat::metric_mm()),
            ..Self::default()
        }
    }

    /// Feet-inches text rounded to the chosen fraction, e.g. `12'-6 1/2"`
    /// (or `12-6 1/2` without unit indicators).
    pub fn fmt_len(&self, inches: f64) -> String {
        if let Some(l) = &self.length {
            return format_length(inches, l);
        }
        let s = fmt_ft_in_frac(inches, self.smallest_fraction);
        if self.unit_indicators {
            s
        } else {
            s.replace(['\'', '"'], "")
        }
    }
}

impl Dimension {
    pub fn new(id: Id, kind: DimensionKind, start: Point, end: Point, offset: f64) -> Self {
        Self {
            id,
            kind,
            start,
            end,
            offset,
            text_override: None,
        }
    }

    pub fn length(&self) -> f64 {
        self.start.dist(self.end)
    }

    fn offset_vec(&self) -> Point {
        self.end
            .sub(self.start)
            .normalized()
            .perp()
            .scale(self.offset)
    }

    /// Endpoints of the dimension line (the measured line pushed out by `offset`).
    pub fn line_points(&self) -> (Point, Point) {
        let o = self.offset_vec();
        (self.start.add(o), self.end.add(o))
    }

    /// Extension lines from each measured point out to the dimension line.
    pub fn extension_lines(&self) -> [(Point, Point); 2] {
        let o = self.offset_vec();
        [(self.start, self.start.add(o)), (self.end, self.end.add(o))]
    }

    pub fn label(&self, fmt: &DimFormat) -> String {
        match &self.text_override {
            Some(t) => t.clone(),
            None => fmt.fmt_len(self.length()),
        }
    }
}

/// Which axis-aligned side of the building a string runs along.
#[derive(Clone, Copy)]
enum Side {
    Bottom,
    Right,
    Top,
    Left,
}

const AXIS_TOL: f64 = 1e-3;
const SIDE_TOL: f64 = 1.0;
const BREAK_TOL: f64 = 0.5;

fn is_horizontal(w: &Wall) -> bool {
    w.direction().y.abs() < AXIS_TOL && w.length() > 0.0
}

fn is_vertical(w: &Wall) -> bool {
    w.direction().x.abs() < AXIS_TOL && w.length() > 0.0
}

/// Automatic exterior dimensions.
///
/// Uses the Exterior walls (or all walls when none are Exterior). For each of
/// the four axis-aligned sides it emits one overall dimension, `offset` inches
/// outside the outer wall face and measuring wall centerline to centerline,
/// and, when walls break the side, a string of dimensions between consecutive
/// centerline breakpoints half-way between the wall and the overall line.
/// The four overall dimensions come first (bottom, right, top, left), then
/// the strings. Ids are `0`; the caller assigns real ids when adding them.
pub fn auto_exterior_dimensions(walls: &[Wall], offset: f64) -> Vec<Dimension> {
    let mut outer: Vec<&Wall> = walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior)
        .collect();
    if outer.is_empty() {
        outer = walls.iter().collect();
    }
    outer.retain(|w| is_horizontal(w) || is_vertical(w));
    if outer.is_empty() {
        return Vec::new();
    }

    // Extreme centerline coordinates of the outer boundary.
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for w in &outer {
        for p in [w.start, w.end] {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
    }

    let mut overall = Vec::new();
    let mut strings = Vec::new();
    for side in [Side::Bottom, Side::Right, Side::Top, Side::Left] {
        // Walls lying along this side: (along-axis interval, thickness).
        let (horizontal, line) = match side {
            Side::Bottom => (true, min_y),
            Side::Top => (true, max_y),
            Side::Left => (false, min_x),
            Side::Right => (false, max_x),
        };
        let on_side: Vec<&&Wall> = outer
            .iter()
            .filter(|w| {
                if horizontal {
                    is_horizontal(w) && (w.start.y - line).abs() <= SIDE_TOL
                } else {
                    is_vertical(w) && (w.start.x - line).abs() <= SIDE_TOL
                }
            })
            .collect();
        if on_side.is_empty() {
            continue;
        }
        let along = |p: Point| if horizontal { p.x } else { p.y };
        let half_t = on_side.iter().map(|w| w.thickness).fold(0.0_f64, f64::max) * 0.5;
        let lo = on_side
            .iter()
            .flat_map(|w| [along(w.start), along(w.end)])
            .fold(f64::INFINITY, f64::min);
        let hi = on_side
            .iter()
            .flat_map(|w| [along(w.start), along(w.end)])
            .fold(f64::NEG_INFINITY, f64::max);

        // Outer face coordinate on the perpendicular axis.
        let face = match side {
            Side::Bottom | Side::Left => line - half_t,
            Side::Top | Side::Right => line + half_t,
        };
        // Direction chosen so the left-hand perpendicular points outward.
        let (s, e) = match side {
            Side::Bottom => (Point::new(hi, face), Point::new(lo, face)),
            Side::Top => (Point::new(lo, face), Point::new(hi, face)),
            Side::Right => (Point::new(face, hi), Point::new(face, lo)),
            Side::Left => (Point::new(face, lo), Point::new(face, hi)),
        };
        overall.push(Dimension::new(0, DimensionKind::AutoExterior, s, e, offset));

        // Breakpoints: side wall ends plus walls butting into the side line.
        let mut breaks: Vec<f64> = on_side
            .iter()
            .flat_map(|w| [along(w.start), along(w.end)])
            .collect();
        for w in walls {
            let perpendicular = if horizontal {
                is_vertical(w)
            } else {
                is_horizontal(w)
            };
            if !perpendicular {
                continue;
            }
            for p in [w.start, w.end] {
                let across = if horizontal { p.y } else { p.x };
                if (across - line).abs() <= half_t + SIDE_TOL {
                    let a = along(p);
                    if a > lo - BREAK_TOL && a < hi + BREAK_TOL {
                        breaks.push(a);
                    }
                }
            }
        }
        breaks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        breaks.dedup_by(|a, b| (*a - *b).abs() <= BREAK_TOL);
        if breaks.len() > 2 {
            for pair in breaks.windows(2) {
                let pt = |a: f64| {
                    if horizontal {
                        Point::new(a, face)
                    } else {
                        Point::new(face, a)
                    }
                };
                // Same direction convention as the overall dimension.
                let (a, b) = if (e.sub(s)).dot(if horizontal {
                    Point::new(1.0, 0.0)
                } else {
                    Point::new(0.0, 1.0)
                }) >= 0.0
                {
                    (pair[0], pair[1])
                } else {
                    (pair[1], pair[0])
                };
                strings.push(Dimension::new(
                    0,
                    DimensionKind::AutoExterior,
                    pt(a),
                    pt(b),
                    offset * 0.5,
                ));
            }
        }
    }
    overall.extend(strings);
    overall
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(id: u64, x0: f64, y0: f64, x1: f64, y1: f64, kind: WallKind) -> Wall {
        Wall {
            id,
            ..Wall::new(Point::new(x0, y0), Point::new(x1, y1), 6.0, 109.125, kind)
        }
    }

    fn rect() -> Vec<Wall> {
        let e = WallKind::Exterior;
        vec![
            wall(1, 0.0, 0.0, 240.0, 0.0, e),
            wall(2, 240.0, 0.0, 240.0, 120.0, e),
            wall(3, 240.0, 120.0, 0.0, 120.0, e),
            wall(4, 0.0, 120.0, 0.0, 0.0, e),
        ]
    }

    #[test]
    fn label_formatting() {
        let f = DimFormat::default();
        assert_eq!(f.fmt_len(150.5), "12'-6 1/2\"");
        assert_eq!(f.fmt_len(150.07), "12'-6 1/16\"");
        let eighths = DimFormat {
            smallest_fraction: 8,
            unit_indicators: true,
            length: None,
        };
        assert_eq!(eighths.fmt_len(150.07), "12'-6 1/8\"");
        assert_eq!(eighths.fmt_len(150.0), "12'-6\"");
        let bare = DimFormat {
            smallest_fraction: 16,
            unit_indicators: false,
            length: None,
        };
        assert_eq!(bare.fmt_len(150.5), "12-6 1/2");
        let mm = DimFormat::metric_mm();
        assert_eq!(mm.fmt_len(120.0), "3048");
        assert_eq!(mm.fmt_len(1234.5 / 25.4), "1235");
        // Old JSON without the field still loads and stays imperial.
        let old: DimFormat =
            serde_json::from_str(r#"{"smallest_fraction":8,"unit_indicators":true}"#).unwrap();
        assert_eq!(old.length, None);
        assert_eq!(old.fmt_len(150.0), "12'-6\"");
        let mut d = Dimension::new(
            1,
            DimensionKind::Manual,
            Point::ZERO,
            Point::new(150.5, 0.0),
            12.0,
        );
        assert_eq!(d.label(&f), "12'-6 1/2\"");
        d.text_override = Some("EQ".into());
        assert_eq!(d.label(&f), "EQ");
    }

    #[test]
    fn line_and_extension_geometry() {
        let d = Dimension::new(
            1,
            DimensionKind::Manual,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            24.0,
        );
        assert!((d.length() - 100.0).abs() < 1e-9);
        let (a, b) = d.line_points();
        assert!(a.dist(Point::new(0.0, 24.0)) < 1e-9 && b.dist(Point::new(100.0, 24.0)) < 1e-9);
        let ext = d.extension_lines();
        assert!(ext[0].0.dist(d.start) < 1e-9 && ext[0].1.dist(a) < 1e-9);
        assert!(ext[1].0.dist(d.end) < 1e-9 && ext[1].1.dist(b) < 1e-9);
    }

    #[test]
    fn auto_exterior_on_rectangle() {
        let dims = auto_exterior_dimensions(&rect(), 36.0);
        assert_eq!(dims.len(), 4);
        let lens: Vec<f64> = dims.iter().map(|d| d.length()).collect();
        assert_eq!(
            lens.iter().filter(|l| (**l - 240.0).abs() < 1e-9).count(),
            2
        );
        assert_eq!(
            lens.iter().filter(|l| (**l - 120.0).abs() < 1e-9).count(),
            2
        );
        // Dimension lines sit outside the building (outer face + offset).
        let top = dims
            .iter()
            .find(|d| (d.start.y - 123.0).abs() < 1e-9)
            .unwrap();
        assert!((top.line_points().0.y - 159.0).abs() < 1e-9);
        let bottom = dims
            .iter()
            .find(|d| (d.start.y + 3.0).abs() < 1e-9)
            .unwrap();
        assert!((bottom.line_points().0.y + 39.0).abs() < 1e-9);
    }

    #[test]
    fn auto_exterior_adds_strings_at_breakpoints() {
        let mut walls = rect();
        // Interior wall meeting the bottom wall at x = 100.
        walls.push(wall(5, 100.0, 0.0, 100.0, 120.0, WallKind::Interior));
        let dims = auto_exterior_dimensions(&walls, 36.0);
        // 4 overall + 2 segments on the bottom + 2 on the top.
        assert_eq!(dims.len(), 4 + 2 + 2);
        let mut seg: Vec<f64> = dims[4..]
            .iter()
            .filter(|d| (d.offset - 18.0).abs() < 1e-9)
            .map(|d| d.length())
            .collect();
        seg.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(seg, vec![100.0, 100.0, 140.0, 140.0]);
    }
}
