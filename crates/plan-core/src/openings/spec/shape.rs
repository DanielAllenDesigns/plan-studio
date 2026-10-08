//! The Shape tab of a window (DW-121): half round, quarter round, trapezoid,
//! triangle and custom outlines (raked sides, angled top and bottom
//! corners), and the lite pattern that follows them.
//!
//! The outline is a convex polygon in the window's own `(u, v)` frame, `u`
//! across `0..width` from the start jamb and `v` up `0..height` from the
//! sill, counter-clockwise from the bottom-left corner. The wall hole stays
//! rectangular; the plan fills the rest with wall, and 3D glazes the polygon.

use super::{Arch, ArchType, LiteStyle, OpeningSpec};
use serde::{Deserialize, Serialize};

/// Segments in a quarter-round curve.
const QUARTER_SEGMENTS: usize = 16;
const EPS: f64 = 1e-6;

/// The shape of a window (Shape tab, preset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShapeKind {
    /// The plain rectangle.
    #[default]
    Rectangle,
    /// A semicircle (or the segment the height allows) on a rectangle.
    HalfRound,
    /// A quarter ellipse with its square corner at the start jamb sill.
    QuarterRoundLeft,
    /// A quarter ellipse with its square corner at the end jamb sill.
    QuarterRoundRight,
    /// Narrower at the head, sides slanting in.
    Trapezoid,
    /// Narrowing to a point over the middle (a gable window).
    Triangle,
    /// The corner and side values typed on the tab.
    Custom,
}

impl ShapeKind {
    pub const ALL: [ShapeKind; 7] = [
        ShapeKind::Rectangle,
        ShapeKind::HalfRound,
        ShapeKind::QuarterRoundLeft,
        ShapeKind::QuarterRoundRight,
        ShapeKind::Trapezoid,
        ShapeKind::Triangle,
        ShapeKind::Custom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::HalfRound => "Half Round",
            ShapeKind::QuarterRoundLeft => "Quarter Round (Left)",
            ShapeKind::QuarterRoundRight => "Quarter Round (Right)",
            ShapeKind::Trapezoid => "Trapezoid",
            ShapeKind::Triangle => "Triangle",
            ShapeKind::Custom => "Custom",
        }
    }
}

/// An angled corner: the straight cut across it. For a top corner `height`
/// is how far the side comes down, for a bottom corner how far it goes up;
/// `offset` is the run along the head or sill.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CornerCut {
    pub on: bool,
    pub height: f64,
    pub offset: f64,
}

impl Default for CornerCut {
    fn default() -> Self {
        Self {
            on: false,
            height: 12.0,
            offset: 12.0,
        }
    }
}

/// The Shape tab values of a window.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowShape {
    pub kind: ShapeKind,
    /// Custom: height of the start and end side (`None` is the full height);
    /// different values rake the head.
    pub left_height: Option<f64>,
    pub right_height: Option<f64>,
    pub top_left: CornerCut,
    pub top_right: CornerCut,
    pub bottom_left: CornerCut,
    pub bottom_right: CornerCut,
}

fn push(poly: &mut Vec<(f64, f64)>, p: (f64, f64)) {
    if poly
        .last()
        .is_none_or(|q| (q.0 - p.0).abs() > EPS || (q.1 - p.1).abs() > EPS)
    {
        poly.push(p);
    }
}

impl WindowShape {
    /// Whether the window is anything but a plain rectangle.
    pub fn is_shaped(&self) -> bool {
        match self.kind {
            ShapeKind::Rectangle => false,
            ShapeKind::Custom => {
                self.left_height.is_some()
                    || self.right_height.is_some()
                    || self.top_left.on
                    || self.top_right.on
                    || self.bottom_left.on
                    || self.bottom_right.on
            }
            _ => true,
        }
    }

    /// `WindowShape` of a kind with the values a preset stands for on a window
    /// `w` x `h` (so the Custom fields show where the preset began).
    pub fn preset(kind: ShapeKind, w: f64, h: f64) -> Self {
        let mut s = Self {
            kind,
            ..Self::default()
        };
        match kind {
            ShapeKind::Trapezoid => {
                s.top_left = CornerCut {
                    on: true,
                    height: h,
                    offset: w * 0.2,
                };
                s.top_right = s.top_left;
            }
            ShapeKind::Triangle => {
                s.top_left = CornerCut {
                    on: true,
                    height: h,
                    offset: w * 0.5,
                };
                s.top_right = s.top_left;
            }
            _ => {}
        }
        s
    }

    /// The convex outline of a window `w` wide and `h` tall (see the module
    /// docs). A plain rectangle gives its four corners.
    pub fn outline(&self, w: f64, h: f64) -> Vec<(f64, f64)> {
        let (w, h) = (w.max(1.0), h.max(1.0));
        let mut out = Vec::new();
        match self.kind {
            ShapeKind::Rectangle => {
                out.extend([(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]);
            }
            ShapeKind::HalfRound => {
                let arch = Arch {
                    kind: ArchType::RoundTop,
                    height: 0.0,
                };
                let rise = arch.rise(w, h);
                let spring = h - rise;
                push(&mut out, (0.0, 0.0));
                push(&mut out, (w, 0.0));
                push(&mut out, (w, spring));
                for p in arch.profile(w, rise).iter().rev() {
                    push(&mut out, (p.0, spring + p.1));
                }
                push(&mut out, (0.0, spring));
            }
            ShapeKind::QuarterRoundLeft | ShapeKind::QuarterRoundRight => {
                // The square corner, then the arc from the far end of the sill
                // up to the top of the near side.
                let mut pts = vec![(0.0, 0.0)];
                for i in 0..=QUARTER_SEGMENTS {
                    let a = std::f64::consts::FRAC_PI_2 * i as f64 / QUARTER_SEGMENTS as f64;
                    pts.push((w * a.cos(), h * a.sin()));
                }
                if self.kind == ShapeKind::QuarterRoundLeft {
                    for p in pts {
                        push(&mut out, p);
                    }
                } else {
                    // Mirrored, and reversed to stay counter-clockwise.
                    for p in pts.iter().rev() {
                        push(&mut out, (w - p.0, p.1));
                    }
                }
            }
            ShapeKind::Trapezoid | ShapeKind::Triangle | ShapeKind::Custom => {
                let preset;
                let s = if self.kind == ShapeKind::Custom {
                    self
                } else {
                    preset = Self::preset(self.kind, w, h);
                    &preset
                };
                custom_outline(s, w, h, &mut out);
            }
        }
        if out.len() > 1
            && (out[0].0 - out[out.len() - 1].0).abs() < EPS
            && (out[0].1 - out[out.len() - 1].1).abs() < EPS
        {
            out.pop();
        }
        out
    }

    /// The pieces of the `w` x `h` rectangle that lie outside the outline:
    /// convex quads cut by vertical slices, for the wall fill over a shape.
    pub fn spandrels(&self, w: f64, h: f64) -> Vec<Vec<(f64, f64)>> {
        let poly = self.outline(w, h);
        if poly.len() < 3 {
            return Vec::new();
        }
        let mut us: Vec<f64> = poly.iter().map(|p| p.0.clamp(0.0, w)).collect();
        us.push(0.0);
        us.push(w);
        us.sort_by(f64::total_cmp);
        us.dedup_by(|a, b| (*a - *b).abs() < EPS);
        let mut out = Vec::new();
        for pair in us.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (Some(sa), Some(sb)) = (section(&poly, a), section(&poly, b)) else {
                // Outside the polygon entirely: the whole slice is wall.
                out.push(vec![(a, 0.0), (b, 0.0), (b, h), (a, h)]);
                continue;
            };
            let upper = [(a, sa.1), (b, sb.1), (b, h), (a, h)];
            let lower = [(a, 0.0), (b, 0.0), (b, sb.0), (a, sa.0)];
            for quad in [upper, lower] {
                if quad_area(&quad) > 1e-4 {
                    out.push(quad.to_vec());
                }
            }
        }
        out
    }

    /// The lite dividers inside the outline as segments `((u0, v0), (u1, v1))`
    /// in the window's frame, clipped to the outline inset by `margin`, for
    /// the pattern of `spec` (lites across and vertical, the grid style and
    /// custom dividers; the muntin width is the caller's).
    pub fn lite_lines(
        &self,
        spec: &OpeningSpec,
        lites: (u32, u32),
        w: f64,
        h: f64,
        margin: f64,
    ) -> Vec<((f64, f64), (f64, f64))> {
        let poly = inset_convex(&self.outline(w, h), margin);
        if poly.len() < 3 {
            return Vec::new();
        }
        let (cols, rows) = (lites.0.clamp(1, 16), lites.1.clamp(1, 16));
        let even = |n: u32| -> Vec<f64> { (1..n).map(|i| f64::from(i) / f64::from(n)).collect() };
        let (xs, ys) = match spec.lite_style {
            LiteStyle::Standard | LiteStyle::Diamond => (even(cols), even(rows)),
            LiteStyle::Prairie => {
                let m = (w.min(h) * 0.2).clamp(2.0, 8.0);
                let cut = |extent: f64| {
                    if extent > 4.0 * m {
                        vec![m / extent, 1.0 - m / extent]
                    } else {
                        Vec::new()
                    }
                };
                (cut(w), cut(h))
            }
            LiteStyle::Custom => (spec.custom_across.clone(), spec.custom_up.clone()),
        };
        let mut lines = Vec::new();
        if spec.lite_style == LiteStyle::Diamond {
            // Diagonals through the lattice of cols x rows cells.
            let (a, b) = (w / f64::from(cols), h / f64::from(rows));
            for mirror in [false, true] {
                for k in -(i64::from(rows) - 1)..=(i64::from(cols) - 1) {
                    let k = k as f64;
                    // x = a (y / b + k)
                    let (y0, y1) = (-5.0, h + 5.0);
                    let at = |y: f64| {
                        let x = a * (y / b + k);
                        if mirror {
                            w - x
                        } else {
                            x
                        }
                    };
                    lines.push(((at(y0), y0), (at(y1), y1)));
                }
            }
        } else {
            for f in xs.iter().filter(|f| **f > 0.0 && **f < 1.0) {
                lines.push(((f * w, -1.0), (f * w, h + 1.0)));
            }
            for f in ys.iter().filter(|f| **f > 0.0 && **f < 1.0) {
                lines.push(((-1.0, f * h), (w + 1.0, f * h)));
            }
        }
        lines
            .into_iter()
            .filter_map(|(p, q)| clip_segment_convex(&poly, p, q))
            .filter(|(p, q)| (q.0 - p.0).hypot(q.1 - p.1) > 0.5)
            .collect()
    }
}

fn custom_outline(s: &WindowShape, w: f64, h: f64, out: &mut Vec<(f64, f64)>) {
    let lh = s.left_height.unwrap_or(h).clamp(0.0, h);
    let rh = s.right_height.unwrap_or(h).clamp(0.0, h);
    let top = |u: f64| lh + (rh - lh) * (u / w);
    let clamp_cut =
        |c: &CornerCut, side_h: f64| (c.height.clamp(0.0, side_h), c.offset.clamp(0.0, w * 0.5));
    // Bottom edge, left to right.
    if s.bottom_left.on {
        let (_, off) = clamp_cut(&s.bottom_left, lh);
        push(out, (off, 0.0));
    } else {
        push(out, (0.0, 0.0));
    }
    if s.bottom_right.on {
        let (_, off) = clamp_cut(&s.bottom_right, rh);
        push(out, (w - off, 0.0));
        let (up, _) = clamp_cut(&s.bottom_right, rh);
        push(out, (w, up));
    } else {
        push(out, (w, 0.0));
    }
    // Up the end side, along the head, down the start side.
    if s.top_right.on {
        let (drop, off) = clamp_cut(&s.top_right, rh);
        push(out, (w, rh - drop));
        push(out, (w - off, top(w - off)));
    } else {
        push(out, (w, rh));
    }
    if s.top_left.on {
        let (drop, off) = clamp_cut(&s.top_left, lh);
        push(out, (off, top(off)));
        push(out, (0.0, lh - drop));
    } else {
        push(out, (0.0, lh));
    }
    if s.bottom_left.on {
        let (up, _) = clamp_cut(&s.bottom_left, lh);
        push(out, (0.0, up));
    }
}

fn quad_area(q: &[(f64, f64); 4]) -> f64 {
    let mut a = 0.0;
    for i in 0..4 {
        let (p, r) = (q[i], q[(i + 1) % 4]);
        a += p.0 * r.1 - r.0 * p.1;
    }
    a.abs() * 0.5
}

/// The `(lowest, highest)` `v` of the convex polygon at `u`, or `None` when
/// the vertical line misses it.
pub fn section(poly: &[(f64, f64)], u: f64) -> Option<(f64, f64)> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let n = poly.len();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let (a, b) = (p.0 - u, q.0 - u);
        if a * b > 0.0 && a.abs() > EPS && b.abs() > EPS {
            continue;
        }
        if (p.0 - q.0).abs() < EPS {
            if (p.0 - u).abs() < EPS {
                lo = lo.min(p.1.min(q.1));
                hi = hi.max(p.1.max(q.1));
            }
            continue;
        }
        let t = ((u - p.0) / (q.0 - p.0)).clamp(0.0, 1.0);
        let v = p.1 + (q.1 - p.1) * t;
        lo = lo.min(v);
        hi = hi.max(v);
    }
    (lo <= hi).then_some((lo, hi))
}

/// A convex counter-clockwise polygon moved in by `d` on every edge. Returns
/// an empty polygon when it vanishes.
pub fn inset_convex(poly: &[(f64, f64)], d: f64) -> Vec<(f64, f64)> {
    let n = poly.len();
    if n < 3 || d <= 0.0 {
        return poly.to_vec();
    }
    // Each edge as a half-plane `n . x >= c`, then clip the polygon by all.
    let mut out = poly.to_vec();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        let len = dx.hypot(dy);
        if len < EPS {
            continue;
        }
        // Inward normal of a counter-clockwise polygon.
        let (nx, ny) = (-dy / len, dx / len);
        let c = nx * p.0 + ny * p.1 + d;
        out = clip_half_plane(&out, (nx, ny), c);
        if out.len() < 3 {
            return Vec::new();
        }
    }
    out
}

/// Keeps the part of `poly` with `nrm . x >= c` (Sutherland-Hodgman).
fn clip_half_plane(poly: &[(f64, f64)], nrm: (f64, f64), c: f64) -> Vec<(f64, f64)> {
    let side = |p: (f64, f64)| nrm.0 * p.0 + nrm.1 * p.1 - c;
    let mut out = Vec::new();
    let n = poly.len();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            push(&mut out, p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            let t = sp / (sp - sq);
            push(&mut out, (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t));
        }
    }
    if out.len() > 1 {
        let (f, l) = (out[0], out[out.len() - 1]);
        if (f.0 - l.0).abs() < EPS && (f.1 - l.1).abs() < EPS {
            out.pop();
        }
    }
    out
}

/// The part of the segment `p`-`q` inside a convex counter-clockwise polygon.
pub fn clip_segment_convex(
    poly: &[(f64, f64)],
    p: (f64, f64),
    q: (f64, f64),
) -> Option<((f64, f64), (f64, f64))> {
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    let d = (q.0 - p.0, q.1 - p.1);
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let len = ex.hypot(ey);
        if len < EPS {
            continue;
        }
        let (nx, ny) = (-ey / len, ex / len);
        let num = nx * (p.0 - a.0) + ny * (p.1 - a.1);
        let den = nx * d.0 + ny * d.1;
        if den.abs() < 1e-12 {
            if num < 0.0 {
                return None;
            }
            continue;
        }
        let t = -num / den;
        if den > 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    Some((
        (p.0 + d.0 * t0, p.1 + d.1 * t0),
        (p.0 + d.0 * t1, p.1 + d.1 * t1),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(poly: &[(f64, f64)]) -> f64 {
        let n = poly.len();
        (0..n)
            .map(|i| {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                a.0 * b.1 - b.0 * a.1
            })
            .sum::<f64>()
            * 0.5
    }

    fn convex_ccw(poly: &[(f64, f64)]) -> bool {
        let n = poly.len();
        (0..n).all(|i| {
            let (a, b, c) = (poly[i], poly[(i + 1) % n], poly[(i + 2) % n]);
            (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0) >= -1e-6
        })
    }

    #[test]
    fn a_rectangle_is_not_shaped_and_has_four_corners() {
        let s = WindowShape::default();
        assert!(!s.is_shaped());
        assert_eq!(s.outline(36.0, 48.0).len(), 4);
        assert!(s.spandrels(36.0, 48.0).is_empty());
    }

    #[test]
    fn every_preset_is_a_convex_counter_clockwise_polygon_inside_the_box() {
        for kind in ShapeKind::ALL {
            let s = WindowShape::preset(kind, 36.0, 48.0);
            let poly = s.outline(36.0, 48.0);
            assert!(poly.len() >= 3, "{kind:?}");
            assert!(area(&poly) > 0.0, "{kind:?} winds clockwise");
            assert!(convex_ccw(&poly), "{kind:?} is not convex: {poly:?}");
            for p in &poly {
                assert!(
                    p.0 >= -1e-6 && p.0 <= 36.0 + 1e-6 && p.1 >= -1e-6 && p.1 <= 48.0 + 1e-6,
                    "{kind:?} leaves the box at {p:?}"
                );
            }
        }
    }

    #[test]
    fn the_triangle_peaks_over_the_middle_and_the_trapezoid_narrows() {
        let tri = WindowShape::preset(ShapeKind::Triangle, 36.0, 48.0).outline(36.0, 48.0);
        assert_eq!(tri.len(), 3);
        assert!(tri
            .iter()
            .any(|p| (p.0 - 18.0).abs() < 1e-6 && (p.1 - 48.0).abs() < 1e-6));
        assert!((area(&tri) - 36.0 * 48.0 * 0.5).abs() < 1e-6);
        let trap = WindowShape::preset(ShapeKind::Trapezoid, 36.0, 48.0).outline(36.0, 48.0);
        assert_eq!(trap.len(), 4);
        let head: Vec<_> = trap.iter().filter(|p| p.1 > 47.0).collect();
        assert_eq!(head.len(), 2);
        assert!((head[0].0 - head[1].0).abs() < 36.0 - 14.0);
    }

    #[test]
    fn half_and_quarter_round_curve_inside_the_box() {
        let half = WindowShape {
            kind: ShapeKind::HalfRound,
            ..Default::default()
        };
        let poly = half.outline(36.0, 48.0);
        // A 36 wide half round has an 18 rise: the head peaks at the top.
        let top = poly.iter().map(|p| p.1).fold(0.0_f64, f64::max);
        assert!((top - 48.0).abs() < 1e-6);
        assert!(poly.len() > 10);
        // The arch cuts less than a rectangle.
        assert!(area(&poly) < 36.0 * 48.0 && area(&poly) > 36.0 * 48.0 * 0.8);
        let q = WindowShape {
            kind: ShapeKind::QuarterRoundLeft,
            ..Default::default()
        };
        let a = area(&q.outline(36.0, 36.0));
        // A quarter of a circle of radius 36.
        assert!(
            (a - std::f64::consts::PI * 36.0 * 36.0 / 4.0).abs() < 25.0,
            "{a}"
        );
        // The right-hand quarter mirrors it: its square corner is at the end.
        let r = WindowShape {
            kind: ShapeKind::QuarterRoundRight,
            ..Default::default()
        }
        .outline(36.0, 36.0);
        assert!(r
            .iter()
            .any(|p| (p.0 - 36.0).abs() < 1e-6 && p.1.abs() < 1e-6));
        assert!(r
            .iter()
            .any(|p| (p.0 - 36.0).abs() < 1e-6 && (p.1 - 36.0).abs() < 1e-6));
        assert!(convex_ccw(&r) && area(&r) > 0.0);
    }

    #[test]
    fn custom_sides_rake_the_head_and_corners_cut() {
        let s = WindowShape {
            kind: ShapeKind::Custom,
            left_height: Some(30.0),
            right_height: Some(48.0),
            ..Default::default()
        };
        assert!(s.is_shaped());
        let poly = s.outline(36.0, 48.0);
        assert_eq!(poly.len(), 4);
        assert!(poly.contains(&(0.0, 30.0)) && poly.contains(&(36.0, 48.0)));
        let cut = WindowShape {
            kind: ShapeKind::Custom,
            top_left: CornerCut {
                on: true,
                height: 10.0,
                offset: 10.0,
            },
            bottom_right: CornerCut {
                on: true,
                height: 6.0,
                offset: 6.0,
            },
            ..Default::default()
        };
        let poly = cut.outline(36.0, 48.0);
        assert_eq!(poly.len(), 6);
        assert!(convex_ccw(&poly) && area(&poly) > 0.0);
        assert!((area(&poly) - (36.0 * 48.0 - 50.0 - 18.0)).abs() < 1e-6);
        // A Custom shape without values is a rectangle.
        assert!(!WindowShape {
            kind: ShapeKind::Custom,
            ..Default::default()
        }
        .is_shaped());
    }

    #[test]
    fn the_spandrels_fill_exactly_what_the_outline_leaves() {
        for kind in [
            ShapeKind::HalfRound,
            ShapeKind::QuarterRoundLeft,
            ShapeKind::QuarterRoundRight,
            ShapeKind::Trapezoid,
            ShapeKind::Triangle,
        ] {
            let s = WindowShape::preset(kind, 36.0, 48.0);
            let outline = area(&s.outline(36.0, 48.0));
            let fill: f64 = s.spandrels(36.0, 48.0).iter().map(|q| area(q).abs()).sum();
            assert!(
                (outline + fill - 36.0 * 48.0).abs() < 0.5,
                "{kind:?}: {outline} + {fill}"
            );
        }
    }

    #[test]
    fn lite_lines_stay_inside_the_shape() {
        let s = WindowShape::preset(ShapeKind::Triangle, 36.0, 48.0);
        let mut spec = OpeningSpec::default();
        let lines = s.lite_lines(&spec, (3, 3), 36.0, 48.0, 1.0);
        assert!(!lines.is_empty());
        let poly = s.outline(36.0, 48.0);
        for (p, q) in &lines {
            for pt in [p, q] {
                // Inside the triangle: under both slanted sides.
                let under = pt.1 <= 48.0 * (1.0 - (pt.0 - 18.0).abs() / 18.0) + 1e-6;
                assert!(
                    under && pt.0 >= -1e-6 && pt.0 <= 36.0 + 1e-6 && pt.1 >= -1e-6,
                    "{pt:?}"
                );
            }
        }
        assert!(poly.len() == 3);
        // One lite: no dividers; diamond: diagonals.
        assert!(s.lite_lines(&spec, (1, 1), 36.0, 48.0, 1.0).is_empty());
        spec.lite_style = LiteStyle::Diamond;
        assert!(!s.lite_lines(&spec, (2, 2), 36.0, 48.0, 1.0).is_empty());
        spec.lite_style = LiteStyle::Custom;
        spec.custom_across = vec![0.5];
        let custom = s.lite_lines(&spec, (1, 1), 36.0, 48.0, 1.0);
        assert_eq!(custom.len(), 1);
        assert!((custom[0].0 .0 - 18.0).abs() < 1e-6);
    }

    #[test]
    fn insetting_shrinks_and_clipping_trims() {
        let sq = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let inner = inset_convex(&sq, 1.0);
        assert!((area(&inner) - 64.0).abs() < 1e-6);
        assert!(inset_convex(&sq, 6.0).is_empty());
        let (a, b) = clip_segment_convex(&sq, (-5.0, 5.0), (15.0, 5.0)).unwrap();
        assert!((a.0).abs() < 1e-9 && (b.0 - 10.0).abs() < 1e-9);
        assert!(clip_segment_convex(&sq, (-5.0, 20.0), (15.0, 20.0)).is_none());
    }
}
