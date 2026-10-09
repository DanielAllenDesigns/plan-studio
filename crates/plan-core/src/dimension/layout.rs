//! Where a dimension's label goes: beside its line, moved, turned, with a
//! leader line and a second-format line, for the editor, the layout window
//! and the sheet PDF alike (manual pp. 478 to 480, 489 to 491 and 502 to
//! 503, in our own words).
//!
//! The painters give the size of a character and a way to measure a string;
//! everything is worked out in plan inches in the plan's own frame (x east,
//! y north) and each painter only converts the points and the angle.

use super::seg::LeaderStyle;
use super::settings::TextPos;
use super::{DimFormat, Dimension};
use crate::geometry::Point;
use std::f64::consts::{FRAC_PI_2, PI};

/// What the painter knows.
pub struct LabelParams<'a> {
    /// Character height of the number, plan inches.
    pub text_h: f64,
    /// The width of a string at `text_h`, plan inches.
    pub width: &'a dyn Fn(&str) -> f64,
    /// The plan view's rotation (counter-clockwise radians; `0` on paper):
    /// text is kept upright as the reader sees it.
    pub view_rotation: f64,
    /// The leader style the Dimension Defaults give.
    pub leader: LeaderStyle,
}

/// The leader settings the Dimension Defaults give (General panel: Include
/// Second Segment, Second Segment Length, Include Arrow); a segment's own
/// settings win.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeaderDefaults {
    pub second_segment: bool,
    /// Plan inches.
    pub second_length: f64,
    pub arrow: bool,
}

impl Default for LeaderDefaults {
    fn default() -> Self {
        Self {
            second_segment: false,
            second_length: 12.0,
            arrow: false,
        }
    }
}

/// One line of the label.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelLine {
    pub text: String,
    /// The middle of the line of text.
    pub center: Point,
    /// Its width.
    pub width: f64,
}

/// The label laid out.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelLayout {
    pub lines: Vec<LabelLine>,
    /// The direction the text reads along, counter-clockwise radians in
    /// plan frame.
    pub angle: f64,
    /// The area of the dimension line the text covers when it sits on the
    /// line (centered); the line breaks behind it.
    pub knockout: Option<[Point; 4]>,
    /// The leader line from the label to the dimension line (polyline).
    pub leader: Vec<Point>,
    /// The leader's arrowhead: its tip (on the dimension line) and the point
    /// the head points back toward.
    pub leader_arrow: Option<(Point, Point)>,
    /// A short stub joining a label that stands beside the end of the line.
    pub stub: Option<(Point, Point)>,
    /// The label stands outside the extension lines (it did not fit).
    pub outside: bool,
}

/// `angle` (plan radians) turned so the text reads upright for a view
/// rotated by `rot`: left to right, and vertical text bottom to top.
pub fn upright(angle: f64, rot: f64) -> f64 {
    let mut a = (angle + rot).rem_euclid(2.0 * PI);
    if a > PI {
        a -= 2.0 * PI;
    }
    if a > FRAC_PI_2 + 1e-4 {
        a -= PI;
    } else if a <= -FRAC_PI_2 + 1e-4 {
        a += PI;
    }
    a - rot
}

impl Dimension {
    /// Lays out the label of this dimension. `anchor` is the middle of the
    /// dimension line (or of its arc), `dir` the unit direction of the line
    /// there, `run` the two ends of a straight line and `line_len` the
    /// length of line between the extension lines.
    pub fn label_layout(
        &self,
        fmt: &DimFormat,
        anchor: Point,
        dir: Point,
        run: Option<(Point, Point)>,
        line_len: f64,
        p: &LabelParams,
    ) -> LabelLayout {
        self.label_layout_with(fmt, anchor, dir, run, line_len, p, &LeaderDefaults::default())
    }

    /// [`Dimension::label_layout`] with the leader defaults of the
    /// Dimension Defaults (second segment, its length, the arrowhead).
    #[allow(clippy::too_many_arguments)]
    pub fn label_layout_with(
        &self,
        fmt: &DimFormat,
        anchor: Point,
        dir: Point,
        run: Option<(Point, Point)>,
        line_len: f64,
        p: &LabelParams,
        lead: &LeaderDefaults,
    ) -> LabelLayout {
        let parts = self.label_parts(fmt);
        let texts = parts.lines();
        if texts.iter().all(String::is_empty) {
            return LabelLayout::default();
        }
        let opts = self.look.label_options(fmt);
        let seg = &self.look.seg;
        let h = p.text_h.max(1e-6);
        let base = dir.angle();
        let fixed = seg.label_angle.or(opts.angle).map(f64::to_radians);
        let angle = match fixed {
            Some(a) => a,
            None => upright(base, p.view_rotation),
        };
        let (s, c) = angle.sin_cos();
        let along = Point::new(c, s);
        let up = Point::new(-s, c);
        let widths: Vec<f64> = texts.iter().map(|t| (p.width)(t)).collect();
        let wide = widths.iter().copied().fold(0.0, f64::max);
        let gap = h * 0.25;
        let lift = h * 0.5 + gap;
        // Where the primary number sits against the line, and the second
        // format on the far side from it.
        let (first, second) = match opts.position {
            TextPos::Above => (lift, -lift),
            TextPos::Below => (-lift, lift),
            TextPos::Centered => (0.0, -lift),
        };
        let mut center = anchor;
        let mut outside = false;
        let mut stub = None;
        // A number that does not fit between the extension lines moves
        // beside the end of the line (DIM-9).
        if let Some((a, b)) = run {
            if wide + h * 0.5 > line_len && seg.label_move.is_none() {
                let end = if b.sub(a).dot(along) >= 0.0 { b } else { a };
                let dir_out = if end == b { b.sub(a) } else { a.sub(b) }.normalized();
                let out = if dir_out.dot(along) >= 0.0 {
                    along
                } else {
                    along.scale(-1.0)
                };
                stub = Some((end, end.add(out.scale(h * 0.4))));
                center = end.add(out.scale(h * 0.4 + wide * 0.5 + gap));
                outside = true;
            }
        }
        let mut leader = Vec::new();
        let mut leader_arrow = None;
        if let Some(m) = seg.label_move {
            let moved = along.scale(m.x).add(up.scale(m.y));
            center = center.add(moved);
            let style = seg.leader.unwrap_or(p.leader);
            if style != LeaderStyle::None && moved.dist(Point::ZERO) > h {
                let edge = |toward: f64| along.scale(toward.signum() * wide * 0.5);
                let at = anchor;
                let corner = at.add(up.scale(m.y));
                let end = center.sub(edge(m.x));
                let two = seg.leader_second_segment.unwrap_or(lead.second_segment);
                let second = seg.leader_second_length.unwrap_or(lead.second_length).max(0.0);
                let side = if m.x >= 0.0 { 1.0 } else { -1.0 };
                leader = match style {
                    LeaderStyle::None => Vec::new(),
                    LeaderStyle::SquareCorner => vec![at, corner, end],
                    LeaderStyle::Diagonal if two => {
                        // The bend stands `second` short of the label.
                        vec![at, end.sub(along.scale(side * second)), end]
                    }
                    LeaderStyle::Diagonal => vec![at, end],
                    LeaderStyle::RoundCorner => {
                        // A quarter-round bend between the rise and the run.
                        let r = (m.x.abs().min(m.y.abs()) * 0.5).min(h * 2.0).max(1e-6);
                        let from = corner.sub(up.scale(m.y.signum() * r));
                        let to = corner.add(along.scale(side * r));
                        let mut v = vec![at, from];
                        for k in 1..=6 {
                            let t = f64::from(k) / 6.0;
                            let a = Point::lerp(from, corner, t);
                            let b = Point::lerp(corner, to, t);
                            v.push(Point::lerp(a, b, t));
                        }
                        v.push(end);
                        if two {
                            v.push(end.add(along.scale(side * second.min(h * 4.0))));
                        }
                        v
                    }
                };
                if lead_arrow(seg.leader_arrow, lead.arrow) && leader.len() >= 2 {
                    leader_arrow = Some((leader[0], leader[1]));
                }
            }
        }
        let lines = texts
            .iter()
            .zip(&widths)
            .enumerate()
            .map(|(i, (t, w))| LabelLine {
                text: t.clone(),
                center: center.add(up.scale(if i == 0 { first } else { second })),
                width: *w,
            })
            .collect();
        let knockout = (opts.position == TextPos::Centered && !outside && seg.label_move.is_none())
            .then(|| {
                let (hx, hy) = (along.scale(wide * 0.5 + gap), up.scale(h * 0.5 + gap * 0.5));
                [
                    center.sub(hx).sub(hy),
                    center.add(hx).sub(hy),
                    center.add(hx).add(hy),
                    center.sub(hx).add(hy),
                ]
            });
        LabelLayout {
            lines,
            angle,
            knockout,
            leader,
            leader_arrow,
            stub,
            outside,
        }
    }
}

fn lead_arrow(own: Option<bool>, default: bool) -> bool {
    own.unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::super::{DimensionKind, TextPos};
    use super::*;

    fn dim(len: f64) -> Dimension {
        Dimension::new(
            0,
            DimensionKind::Manual,
            Point::ZERO,
            Point::new(len, 0.0),
            0.0,
        )
    }

    fn params(w: &dyn Fn(&str) -> f64) -> LabelParams<'_> {
        LabelParams {
            text_h: 4.0,
            width: w,
            view_rotation: 0.0,
            leader: LeaderStyle::SquareCorner,
        }
    }

    #[test]
    fn text_stays_upright_and_vertical_reads_bottom_to_top() {
        assert!(upright(0.0, 0.0).abs() < 1e-9);
        assert!(
            (upright(PI, 0.0)).abs() < 1e-9,
            "a leftward line reads left to right"
        );
        assert!((upright(-FRAC_PI_2, 0.0) - FRAC_PI_2).abs() < 1e-9);
        assert!((upright(FRAC_PI_2, 0.0) - FRAC_PI_2).abs() < 1e-9);
        // A view turned a quarter turn reads the same way on screen.
        assert!(upright(0.0, FRAC_PI_2).abs() < 1e-9);
        assert!(upright(PI, FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn the_label_sits_above_centered_or_below_and_a_second_format_opposite() {
        let w = |s: &str| s.chars().count() as f64 * 2.0;
        let fmt = DimFormat::default();
        let d = dim(120.0);
        let anchor = Point::new(60.0, 0.0);
        let l = d.label_layout(
            &fmt,
            anchor,
            Point::new(1.0, 0.0),
            Some((Point::ZERO, Point::new(120.0, 0.0))),
            120.0,
            &params(&w),
        );
        assert_eq!(l.lines.len(), 1);
        assert!(l.lines[0].center.y > 0.0, "above by default");
        assert!(l.knockout.is_none());
        let mut f2 = fmt;
        f2.label.position = TextPos::Centered;
        let l = d.label_layout(
            &f2,
            anchor,
            Point::new(1.0, 0.0),
            Some((Point::ZERO, Point::new(120.0, 0.0))),
            120.0,
            &params(&w),
        );
        assert!(l.lines[0].center.y.abs() < 1e-9);
        assert!(
            l.knockout.is_some(),
            "the line breaks behind a centered number"
        );
        f2.label.position = TextPos::Below;
        f2.label.second.include = true;
        let l = d.label_layout(
            &f2,
            anchor,
            Point::new(1.0, 0.0),
            Some((Point::ZERO, Point::new(120.0, 0.0))),
            120.0,
            &params(&w),
        );
        assert_eq!(l.lines.len(), 2);
        assert!(l.lines[0].center.y < 0.0 && l.lines[1].center.y > 0.0);
    }

    #[test]
    fn a_number_too_wide_for_the_line_moves_outside_with_a_stub() {
        let w = |s: &str| s.chars().count() as f64 * 2.0;
        let fmt = DimFormat::default();
        let d = dim(10.0);
        let l = d.label_layout(
            &fmt,
            Point::new(5.0, 0.0),
            Point::new(1.0, 0.0),
            Some((Point::ZERO, Point::new(10.0, 0.0))),
            10.0,
            &params(&w),
        );
        assert!(l.outside);
        assert!(l.stub.is_some());
        assert!(l.lines[0].center.x > 10.0);
    }

    #[test]
    fn a_moved_label_gets_a_leader_and_a_turned_one_keeps_its_angle() {
        let w = |s: &str| s.chars().count() as f64 * 2.0;
        let fmt = DimFormat::default();
        let mut d = dim(120.0);
        d.look.seg.label_move = Some(Point::new(30.0, 20.0));
        let a = Point::new(60.0, 0.0);
        let l = d.label_layout(
            &fmt,
            a,
            Point::new(1.0, 0.0),
            Some((Point::ZERO, Point::new(120.0, 0.0))),
            120.0,
            &params(&w),
        );
        assert_eq!(l.leader.len(), 3, "square corner: line, corner, label");
        assert!(l.lines[0].center.x > 85.0);
        d.look.seg.leader = Some(LeaderStyle::None);
        let l = d.label_layout(
            &fmt,
            a,
            Point::new(1.0, 0.0),
            Some((Point::ZERO, Point::new(120.0, 0.0))),
            120.0,
            &params(&w),
        );
        assert!(l.leader.is_empty());
        d.look.seg.label_angle = Some(90.0);
        let l = d.label_layout(&fmt, a, Point::new(1.0, 0.0), None, 120.0, &params(&w));
        assert!((l.angle - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn leaders_can_be_a_line_or_an_arc_with_two_segments_and_an_arrow() {
        let w = |s: &str| s.chars().count() as f64 * 2.0;
        let fmt = DimFormat::default();
        let mut d = dim(120.0);
        d.look.seg.label_move = Some(Point::new(40.0, 20.0));
        let a = Point::new(60.0, 0.0);
        let run = Some((Point::ZERO, Point::new(120.0, 0.0)));
        let lay = |d: &Dimension, lead: &LeaderDefaults| {
            d.label_layout_with(&fmt, a, Point::new(1.0, 0.0), run, 120.0, &params(&w), lead)
        };
        // A diagonal line is one segment; a second segment bends it.
        d.look.seg.leader = Some(LeaderStyle::Diagonal);
        let l = lay(&d, &LeaderDefaults::default());
        assert_eq!(l.leader.len(), 2);
        assert!(l.leader_arrow.is_none());
        let two = LeaderDefaults {
            second_segment: true,
            second_length: 10.0,
            arrow: true,
        };
        let l = lay(&d, &two);
        assert_eq!(l.leader.len(), 3, "line, bend, label");
        assert!((l.leader[1].dist(l.leader[2]) - 10.0).abs() < 1e-9);
        let (tip, toward) = l.leader_arrow.expect("the defaults ask for an arrow");
        assert_eq!(tip, a);
        assert_eq!(toward, l.leader[1]);
        // The segment's own settings win over the defaults.
        d.look.seg.leader_second_segment = Some(false);
        d.look.seg.leader_arrow = Some(false);
        let l = lay(&d, &two);
        assert_eq!(l.leader.len(), 2);
        assert!(l.leader_arrow.is_none());
        // A round corner is an arc: many points, ending at the label.
        d.look.seg.leader = Some(LeaderStyle::RoundCorner);
        let l = lay(&d, &LeaderDefaults::default());
        assert!(l.leader.len() > 5, "{}", l.leader.len());
        assert_eq!(l.leader[0], a);
    }

    #[test]
    fn a_blank_label_lays_out_nothing() {
        let w = |s: &str| s.len() as f64;
        let mut d = dim(120.0);
        d.look.seg.blank = true;
        let l = d.label_layout(
            &DimFormat::default(),
            Point::ZERO,
            Point::new(1.0, 0.0),
            None,
            120.0,
            &params(&w),
        );
        assert!(l.lines.is_empty());
    }
}
