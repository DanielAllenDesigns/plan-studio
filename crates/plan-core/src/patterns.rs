//! Custom hatch patterns (CAD-71, CAD-79..CAD-81; manual pp. 222, 227-229).
//!
//! A [`CustomPattern`] is one or more [`PatternTileGroup`]s. Each group has a
//! tile drawn from CAD lines, a Repeat Box (width and height of one cell, with
//! a horizontal shift that offsets the rows and a vertical shift that offsets
//! the columns) and any number of Infinite Pattern Lines, which repeat
//! independently of the box. `fill_styles::fill_geometry` lays a pattern out
//! over an area; this module builds and edits the patterns:
//!
//! * [`CustomPattern::from_cad`]: Create Pattern from drawn CAD objects.
//! * group editing: add, delete, repeat box, shifts, infinite lines.
//! * [`parse_pat`]: File > Import > Import Patterns (.pat).

use crate::cad::CadItem;
use crate::geometry::Point;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

/// A line that repeats on its own: the family of parallel lines `spacing`
/// apart through (`x`, `y`) of the group's frame, each shifted `shift` along
/// itself from the one before (the Infinite Pattern Line Specification and
/// the `.pat` format's delta-x and delta-y).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InfiniteLine {
    pub x: f64,
    pub y: f64,
    pub angle_deg: f64,
    /// Distance Between Lines.
    pub spacing: f64,
    pub shift: f64,
    /// Positive dash, negative gap, zero dot; empty is continuous.
    pub dashes: Vec<f64>,
}

impl Default for InfiniteLine {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            angle_deg: 0.0,
            spacing: 12.0,
            shift: 0.0,
            dashes: Vec::new(),
        }
    }
}

/// One tile with its own Repeat Box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PatternTileGroup {
    /// The tile's lines in the Repeat Box's frame (lower left is 0, 0).
    pub tile: Vec<(Point, Point)>,
    pub dots: Vec<Point>,
    /// Repeat Box size, plan inches.
    pub width: f64,
    pub height: f64,
    /// Horizontal Shift: offsets the rows.
    pub h_shift: f64,
    /// Vertical Shift: offsets the columns.
    pub v_shift: f64,
    pub infinite: Vec<InfiniteLine>,
}

impl Default for PatternTileGroup {
    fn default() -> Self {
        Self {
            tile: Vec::new(),
            dots: Vec::new(),
            width: 12.0,
            height: 12.0,
            h_shift: 0.0,
            v_shift: 0.0,
            infinite: Vec::new(),
        }
    }
}

/// A custom pattern: at least one group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomPattern {
    pub name: String,
    pub groups: Vec<PatternTileGroup>,
}

impl Default for CustomPattern {
    fn default() -> Self {
        Self::new("New Pattern")
    }
}

/// Segments of a circle or arc (an arc's `sweep` radians from `start`).
fn arc_points(center: Point, r: f64, start: f64, sweep: f64) -> Vec<Point> {
    let n = ((sweep.abs() / TAU * 32.0).ceil() as usize).max(4);
    (0..=n)
        .map(|i| {
            let a = start + sweep * i as f64 / n as f64;
            Point::new(center.x + r * a.cos(), center.y + r * a.sin())
        })
        .collect()
}

/// The line segments a CAD item contributes to a tile (text has none).
pub fn item_segments(item: &CadItem) -> Vec<(Point, Point)> {
    let chain = |pts: &[Point], closed: bool| -> Vec<(Point, Point)> {
        let mut v: Vec<(Point, Point)> = pts.windows(2).map(|w| (w[0], w[1])).collect();
        if closed && pts.len() > 2 {
            v.push((pts[pts.len() - 1], pts[0]));
        }
        v
    };
    match item {
        CadItem::Line { a, b } => vec![(*a, *b)],
        CadItem::Polyline { points, closed } => chain(points, *closed),
        CadItem::Circle { center, radius } => chain(&arc_points(*center, *radius, 0.0, TAU), false),
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(TAU);
            chain(&arc_points(*center, *radius, *start_angle, sweep), false)
        }
        CadItem::Text { .. } => Vec::new(),
    }
}

impl CustomPattern {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            groups: vec![PatternTileGroup::default()],
        }
    }

    /// Create Pattern: one tile drawn from `items`. The Repeat Box starts as
    /// the tile's bounding box; the tile is moved so the box's lower left
    /// corner is 0, 0. `None` when the items draw no lines.
    pub fn from_cad(name: &str, items: &[CadItem]) -> Option<CustomPattern> {
        let segs: Vec<(Point, Point)> = items.iter().flat_map(item_segments).collect();
        if segs.is_empty() {
            return None;
        }
        let (mut lo, mut hi) = (segs[0].0, segs[0].0);
        for p in segs.iter().flat_map(|(a, b)| [*a, *b]) {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        let mut g = PatternTileGroup {
            tile: segs.iter().map(|(a, b)| (*a - lo, *b - lo)).collect(),
            width: (hi.x - lo.x).max(0.25),
            height: (hi.y - lo.y).max(0.25),
            ..PatternTileGroup::default()
        };
        // A tile of one horizontal or vertical line gets a box that makes it
        // a visible rule.
        if hi.x - lo.x < 0.25 {
            g.width = 12.0;
        }
        if hi.y - lo.y < 0.25 {
            g.height = 12.0;
        }
        Some(CustomPattern {
            name: name.to_string(),
            groups: vec![g],
        })
    }

    /// Add Pattern Tile Group: a new empty group after the last; returns its
    /// index.
    pub fn add_group(&mut self) -> usize {
        self.groups.push(PatternTileGroup::default());
        self.groups.len() - 1
    }

    /// Delete Pattern Tile Group: refused for the last one.
    pub fn delete_group(&mut self, i: usize) -> bool {
        if self.groups.len() > 1 && i < self.groups.len() {
            self.groups.remove(i);
            true
        } else {
            false
        }
    }

    /// Next Pattern Tile Group (wraps no further than the last).
    pub fn next_group(&self, i: usize) -> usize {
        (i + 1).min(self.groups.len().saturating_sub(1))
    }

    /// Previous Pattern Tile Group.
    pub fn previous_group(&self, i: usize) -> usize {
        i.saturating_sub(1)
    }

    /// Edit Pattern Repeat Dimensions.
    pub fn set_repeat(
        &mut self,
        group: usize,
        width: f64,
        height: f64,
        h_shift: f64,
        v_shift: f64,
    ) -> Result<(), String> {
        if !(width > 0.0 && height > 0.0) {
            return Err("The Repeat Box needs a width and a height".into());
        }
        let g = self.groups.get_mut(group).ok_or("No such tile group")?;
        g.width = width;
        g.height = height;
        g.h_shift = h_shift;
        g.v_shift = v_shift;
        Ok(())
    }

    /// Infinite Pattern Line: adds one to `group`.
    pub fn add_infinite_line(&mut self, group: usize, line: InfiniteLine) -> Result<usize, String> {
        if line.spacing.is_nan() || line.spacing.abs() <= 1e-3 {
            return Err("Distance Between Lines must be more than zero".into());
        }
        let g = self.groups.get_mut(group).ok_or("No such tile group")?;
        g.infinite.push(line);
        Ok(g.infinite.len() - 1)
    }

    /// The pattern draws nothing.
    pub fn is_empty(&self) -> bool {
        self.groups
            .iter()
            .all(|g| g.tile.is_empty() && g.dots.is_empty() && g.infinite.is_empty())
    }

    /// The tile lines of `group` in the Pattern window, and the repeat box
    /// as a closed ring.
    pub fn repeat_box(&self, group: usize) -> Vec<Point> {
        let Some(g) = self.groups.get(group) else {
            return Vec::new();
        };
        vec![
            Point::ZERO,
            Point::new(g.width, 0.0),
            Point::new(g.width, g.height),
            Point::new(0.0, g.height),
        ]
    }
}

/// What a `.pat` import found.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PatImport {
    pub patterns: Vec<CustomPattern>,
    pub skipped: Vec<String>,
}

fn num(t: &str) -> Option<f64> {
    t.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Reads AutoCAD `.pat` hatch text: `*NAME, description` then one line per
/// line family, `angle, x-origin, y-origin, delta-x, delta-y, dashes...`.
/// `unit` is plan inches per file unit.
pub fn parse_pat(text: &str, unit: f64) -> PatImport {
    let unit = if unit.is_finite() && unit > 0.0 {
        unit
    } else {
        1.0
    };
    let mut out = PatImport::default();
    let mut cur: Option<(String, Vec<InfiniteLine>, bool)> = None;
    let finish = |c: Option<(String, Vec<InfiniteLine>, bool)>, out: &mut PatImport| {
        if let Some((name, lines, ok)) = c {
            if ok && !lines.is_empty() {
                let mut p = CustomPattern::new(&name);
                p.groups[0].infinite = lines;
                out.patterns.push(p);
            } else {
                out.skipped.push(name);
            }
        }
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(head) = line.strip_prefix('*') {
            finish(cur.take(), &mut out);
            let name = head.split(',').next().unwrap_or("").trim().to_string();
            cur = Some((name, Vec::new(), true));
            continue;
        }
        let Some((_, lines, ok)) = cur.as_mut() else {
            continue;
        };
        let t: Vec<&str> = line.split(',').collect();
        let vals: Vec<Option<f64>> = t.iter().map(|s| num(s)).collect();
        if t.len() < 5 || vals.iter().any(|v| v.is_none()) {
            *ok = false;
            continue;
        }
        let v: Vec<f64> = vals.into_iter().flatten().collect();
        lines.push(InfiniteLine {
            x: v[1] * unit,
            y: v[2] * unit,
            angle_deg: v[0],
            // delta-x runs along the line, delta-y across.
            shift: v[3] * unit,
            spacing: v[4] * unit,
            dashes: v[5..].iter().map(|d| d * unit).collect(),
        });
        if lines
            .last()
            .is_some_and(|l| l.spacing.is_nan() || l.spacing.abs() <= 1e-6)
        {
            *ok = false;
        }
    }
    finish(cur.take(), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill_styles::{fill_geometry, FillStyle};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn create_pattern_from_cad_moves_the_tile_to_its_repeat_box() {
        let items = vec![
            CadItem::Polyline {
                points: vec![p(100.0, 50.0), p(106.0, 50.0), p(106.0, 56.0)],
                closed: true,
            },
            CadItem::Circle {
                center: p(103.0, 52.0),
                radius: 1.0,
            },
            CadItem::Text {
                pos: p(0.0, 0.0),
                text: "x".into(),
                height: 2.0,
                angle: 0.0,
            },
        ];
        let pat = CustomPattern::from_cad("Triangle", &items).unwrap();
        let g = &pat.groups[0];
        assert_eq!((g.width, g.height), (6.0, 6.0));
        // 3 triangle edges + the circle's chords; text adds none.
        assert!(g.tile.len() > 10);
        let lo = g
            .tile
            .iter()
            .flat_map(|(a, b)| [a.x.min(b.x), a.y.min(b.y)])
            .fold(f64::INFINITY, f64::min);
        assert!(lo.abs() < 1e-9, "the box corner is the origin");
        assert!(CustomPattern::from_cad("Empty", &[]).is_none());
        // A single horizontal line gets a usable box height.
        let rule = CustomPattern::from_cad(
            "Rule",
            &[CadItem::Line {
                a: p(0.0, 0.0),
                b: p(8.0, 0.0),
            }],
        )
        .unwrap();
        assert_eq!((rule.groups[0].width, rule.groups[0].height), (8.0, 12.0));
    }

    #[test]
    fn a_pattern_made_from_cad_tiles_a_slab() {
        let pat = CustomPattern::from_cad(
            "Plus",
            &[
                CadItem::Line {
                    a: p(0.0, 3.0),
                    b: p(6.0, 3.0),
                },
                CadItem::Line {
                    a: p(3.0, 0.0),
                    b: p(3.0, 6.0),
                },
            ],
        )
        .unwrap();
        let slab = vec![p(0.0, 0.0), p(60.0, 0.0), p(60.0, 36.0), p(0.0, 36.0)];
        let g = fill_geometry(&FillStyle::library("Plus"), &slab, &[], &[pat]);
        // 10 x 6 boxes, two lines each.
        assert_eq!(g.lines.len(), 120);
        assert!(!g.truncated);
    }

    #[test]
    fn groups_repeat_boxes_shifts_and_infinite_lines_edit_like_the_pattern_window() {
        let mut pat = CustomPattern::new("Two");
        assert!(!pat.delete_group(0), "the last group stays");
        let second = pat.add_group();
        assert_eq!(second, 1);
        assert_eq!(pat.next_group(0), 1);
        assert_eq!(pat.next_group(1), 1);
        assert_eq!(pat.previous_group(0), 0);
        assert!(pat.set_repeat(1, 0.0, 5.0, 0.0, 0.0).is_err());
        pat.set_repeat(1, 8.0, 4.0, 2.0, 0.0).unwrap();
        assert_eq!(pat.groups[1].h_shift, 2.0);
        assert_eq!(pat.repeat_box(1)[2], p(8.0, 4.0));
        assert!(pat
            .add_infinite_line(
                0,
                InfiniteLine {
                    spacing: 0.0,
                    ..InfiniteLine::default()
                }
            )
            .is_err());
        assert_eq!(
            pat.add_infinite_line(
                0,
                InfiniteLine {
                    spacing: 6.0,
                    ..InfiniteLine::default()
                }
            ),
            Ok(0)
        );
        assert!(!pat.is_empty());
        assert!(pat.delete_group(0));
        assert_eq!(pat.groups.len(), 1);
        assert_eq!(pat.groups[0].width, 8.0);
        assert!(CustomPattern::new("E").is_empty());
    }

    #[test]
    fn shifts_offset_the_rows_of_the_repeat_box() {
        let mut pat = CustomPattern::new("Dash");
        pat.groups[0] = PatternTileGroup {
            tile: vec![(p(0.0, 0.0), p(0.0, 4.0))],
            width: 8.0,
            height: 4.0,
            h_shift: 2.0,
            ..PatternTileGroup::default()
        };
        let area = vec![p(0.0, 0.0), p(32.0, 0.0), p(32.0, 12.0), p(0.0, 12.0)];
        let g = fill_geometry(&FillStyle::library("Dash"), &area, &[], &[pat]);
        let xs = |y: f64| -> Vec<f64> {
            let mut v: Vec<f64> = g
                .lines
                .iter()
                .filter(|(a, b)| (a.y.min(b.y) - y).abs() < 1e-9)
                .map(|(a, _)| a.x)
                .collect();
            v.sort_by(f64::total_cmp);
            v
        };
        // Row 1 sits 2 to the right of row 0, row 2 another 2.
        assert_eq!(xs(0.0)[0], 0.0);
        assert_eq!(xs(4.0)[0], 2.0);
        assert_eq!(xs(8.0)[0], 4.0);
    }

    #[test]
    fn infinite_lines_fill_a_pattern_with_dashes_and_phase() {
        let mut pat = CustomPattern::new("Dashed");
        pat.add_infinite_line(
            0,
            InfiniteLine {
                spacing: 6.0,
                dashes: vec![4.0, -2.0],
                ..InfiniteLine::default()
            },
        )
        .unwrap();
        pat.groups[0].tile.clear();
        let area = vec![p(0.0, 0.0), p(24.0, 0.0), p(24.0, 12.0), p(0.0, 12.0)];
        let g = fill_geometry(&FillStyle::library("Dashed"), &area, &[], &[pat]);
        // Lines at y = 0, 6, 12; each 24 long breaks into 4 + 2: 4 dashes.
        let row6: Vec<_> = g
            .lines
            .iter()
            .filter(|(a, _)| (a.y - 6.0).abs() < 1e-9)
            .collect();
        assert_eq!(row6.len(), 4);
        assert!((row6[0].1.x - row6[0].0.x - 4.0).abs() < 1e-9);
        assert!((row6[1].0.x - row6[0].0.x - 6.0).abs() < 1e-9);
    }

    #[test]
    fn the_pat_importer_reads_line_families_and_skips_bad_ones() {
        let text = "\
; a comment
*ANSI31, ANSI Iron, Brick, Stone masonry
45, 0,0, 0,3.175
*DASHEDX, dashed crosses
0, 0,0, 0,6, 3,-1.5
90, 0,0, 3,6, 3,-1.5
*BAD, broken
oops
";
        let imp = parse_pat(text, 1.0);
        assert_eq!(imp.patterns.len(), 2);
        assert_eq!(imp.skipped, vec!["BAD".to_string()]);
        let a = &imp.patterns[0];
        assert_eq!(a.name, "ANSI31");
        assert_eq!(a.groups[0].infinite[0].angle_deg, 45.0);
        assert!((a.groups[0].infinite[0].spacing - 3.175).abs() < 1e-12);
        let b = &imp.patterns[1];
        assert_eq!(b.groups[0].infinite.len(), 2);
        assert_eq!(b.groups[0].infinite[1].shift, 3.0);
        assert_eq!(b.groups[0].infinite[0].dashes, vec![3.0, -1.5]);
        // Millimetre files scale.
        let mm = parse_pat("*M,m\n0, 0,0, 0,25.4", 1.0 / 25.4);
        assert!((mm.patterns[0].groups[0].infinite[0].spacing - 1.0).abs() < 1e-9);
        // An imported pattern tiles at once.
        let area = vec![p(0.0, 0.0), p(12.0, 0.0), p(12.0, 12.0), p(0.0, 12.0)];
        let g = fill_geometry(&FillStyle::library("ANSI31"), &area, &[], &imp.patterns);
        assert!(g.lines.len() >= 3);
    }
}
