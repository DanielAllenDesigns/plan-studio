//! Below Grade line overrides (manual pp. 1190, 1197; C-153): the lines of a
//! Vector View that lie under the terrain or under a height get their own
//! colour, style and weight.
//!
//! A drawing line carries a weight class and a kind only, so the weight and
//! the dashes are applied to the line itself (a Medium or Light line, or the
//! hidden-line dashes) and a colour is added as a coloured copy in
//! [`Drawing::styled`], which the 3D view paints over it.

use crate::drawing::{Drawing, EdgeKind, Line2, LineWeight, StyledLine};
use plan_core::geometry::Point;

/// What to change on the lines below grade. `None` keeps the line's own.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BelowGradeStyle {
    pub color: Option<[u8; 3]>,
    /// Draw the lines dashed.
    pub dashed: bool,
    /// Line weight in points.
    pub weight: Option<f64>,
}

/// The weight class a line weight in points draws as.
pub fn weight_class(points: f64) -> LineWeight {
    if points >= 1.0 {
        LineWeight::Heavy
    } else if points >= 0.5 {
        LineWeight::Medium
    } else {
        LineWeight::Light
    }
}

/// Restyles what lies below the height `limit` (drawing Y, inches). Lines
/// that cross the height are split there.
pub fn override_below(drawing: &mut Drawing, limit: f64, style: &BelowGradeStyle) {
    if style.color.is_none() && !style.dashed && style.weight.is_none() {
        return;
    }
    let mut out = Vec::with_capacity(drawing.lines.len());
    let mut styled = Vec::new();
    for l in drawing.lines.drain(..) {
        let (lo, hi) = if l.a.y <= l.b.y { (l.a, l.b) } else { (l.b, l.a) };
        if lo.y >= limit - 1e-9 {
            out.push(l);
            continue;
        }
        // The piece below and, when the line crosses the height, the piece above.
        let (below, above) = if hi.y <= limit + 1e-9 {
            ((l.a, l.b), None)
        } else {
            let t = (limit - lo.y) / (hi.y - lo.y);
            let mid = Point::lerp(lo, hi, t);
            ((lo, mid), Some((mid, hi)))
        };
        if let Some((a, b)) = above {
            out.push(Line2 { a, b, ..l });
        }
        let mut piece = Line2 {
            a: below.0,
            b: below.1,
            ..l
        };
        if let Some(w) = style.weight {
            piece.weight = weight_class(w);
        }
        if style.dashed && piece.kind != EdgeKind::Hidden {
            piece.kind = EdgeKind::Hidden;
        }
        if let Some(color) = style.color {
            styled.push(StyledLine {
                a: piece.a,
                b: piece.b,
                width: style.weight.unwrap_or(match piece.weight {
                    LineWeight::Heavy => 1.8,
                    LineWeight::Medium => 1.1,
                    LineWeight::Light => 0.6,
                }),
                color,
                dashed: style.dashed,
            });
        }
        out.push(piece);
    }
    drawing.lines = out;
    drawing.styled.extend(styled);
    drawing.update_bounds();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(y0: f64, y1: f64) -> Line2 {
        Line2 {
            a: Point::new(0.0, y0),
            b: Point::new(0.0, y1),
            weight: LineWeight::Heavy,
            kind: EdgeKind::Silhouette,
        }
    }

    #[test]
    fn a_line_across_grade_is_split_and_only_the_lower_part_changes() {
        let mut d = Drawing::new(vec![line(-48.0, 48.0), line(10.0, 90.0), line(-30.0, -10.0)]);
        override_below(
            &mut d,
            0.0,
            &BelowGradeStyle {
                color: Some([200, 0, 0]),
                dashed: true,
                weight: Some(0.4),
            },
        );
        // The crossing line became two, the upper line is untouched, the lower
        // line is restyled.
        assert_eq!(d.lines.len(), 4);
        let above: Vec<_> = d.lines.iter().filter(|l| l.a.y.min(l.b.y) >= 0.0).collect();
        assert_eq!(above.len(), 2);
        assert!(above
            .iter()
            .all(|l| l.weight == LineWeight::Heavy && l.kind == EdgeKind::Silhouette));
        let below: Vec<_> = d.lines.iter().filter(|l| l.a.y.max(l.b.y) <= 0.0).collect();
        assert_eq!(below.len(), 2);
        assert!(below
            .iter()
            .all(|l| l.weight == LineWeight::Light && l.kind == EdgeKind::Hidden));
        assert_eq!(d.styled.len(), 2);
        assert!(d.styled.iter().all(|s| s.color == [200, 0, 0] && s.dashed));
    }

    #[test]
    fn no_override_leaves_the_drawing_alone() {
        let mut d = Drawing::new(vec![line(-48.0, 48.0)]);
        let before = d.clone();
        override_below(&mut d, 0.0, &BelowGradeStyle::default());
        assert_eq!(d, before);
    }

    #[test]
    fn weights_in_points_pick_a_class() {
        assert_eq!(weight_class(1.4), LineWeight::Heavy);
        assert_eq!(weight_class(0.7), LineWeight::Medium);
        assert_eq!(weight_class(0.25), LineWeight::Light);
    }
}
