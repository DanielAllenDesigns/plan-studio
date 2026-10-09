//! 2D drawing patterns (hatches) for plans and elevations.

use plan_core::Point;
use serde::{Deserialize, Serialize};

use crate::noise::{hash64, unit_f64};

/// Upper bound on segments returned by [`pattern_strokes`].
pub const MAX_STROKES: usize = 20_000;

/// A 2D hatch description. All lengths are real-world inches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    /// No hatch.
    None,
    /// Parallel lines at `angle_deg`, `spacing` apart.
    Lines { angle_deg: f64, spacing: f64 },
    /// Two perpendicular line sets.
    CrossHatch { angle_deg: f64, spacing: f64 },
    /// Running-bond brick; `height` is course height including the joint.
    Brick { length: f64, height: f64 },
    /// Running-bond masonry block.
    Block { length: f64, height: f64 },
    /// Staggered shingle courses; `exposure` is course height.
    Shingle { exposure: f64, width: f64 },
    /// Horizontal lap boards, `exposure` inches apart.
    LapSiding { exposure: f64 },
    /// Vertical boards with battens over the joints every `spacing`.
    BoardAndBatten { spacing: f64 },
    /// Stack-bond grid of `w` x `h` tiles.
    Tile { w: f64, h: f64 },
    /// 90-degree herringbone of `length` x `width` planks.
    Herringbone { length: f64, width: f64 },
    /// Batt insulation zig-zag.
    Insulation,
    /// Random stipple (stable per absolute position).
    Concrete,
    /// Earth ticks.
    Earth,
    /// Grass tufts.
    Grass,
}

impl Pattern {
    /// Modular brick: 8" x 2 1/4" per course including joints.
    pub fn brick() -> Self {
        Pattern::Brick {
            length: 8.0,
            height: 2.25,
        }
    }
    /// Standard 16" x 8" concrete block.
    pub fn block() -> Self {
        Pattern::Block {
            length: 16.0,
            height: 8.0,
        }
    }
    /// 5" exposure, 12" wide shingles.
    pub fn shingle() -> Self {
        Pattern::Shingle {
            exposure: 5.0,
            width: 12.0,
        }
    }
    /// 6" exposure lap siding.
    pub fn lap_siding() -> Self {
        Pattern::LapSiding { exposure: 6.0 }
    }
    /// Board and batten at 12" centres.
    pub fn board_and_batten() -> Self {
        Pattern::BoardAndBatten { spacing: 12.0 }
    }
}

impl Pattern {
    /// This pattern `k` times its size (course heights, tile sizes, line
    /// spacing); the stipple and symbol patterns have no size and are
    /// unchanged. A non-positive or non-finite `k` changes nothing.
    pub fn scaled(&self, k: f64) -> Pattern {
        if !(k.is_finite() && k > 0.0) || (k - 1.0).abs() < 1e-12 {
            return self.clone();
        }
        match *self {
            Pattern::Lines { angle_deg, spacing } => Pattern::Lines {
                angle_deg,
                spacing: spacing * k,
            },
            Pattern::CrossHatch { angle_deg, spacing } => Pattern::CrossHatch {
                angle_deg,
                spacing: spacing * k,
            },
            Pattern::Brick { length, height } => Pattern::Brick {
                length: length * k,
                height: height * k,
            },
            Pattern::Block { length, height } => Pattern::Block {
                length: length * k,
                height: height * k,
            },
            Pattern::Shingle { exposure, width } => Pattern::Shingle {
                exposure: exposure * k,
                width: width * k,
            },
            Pattern::LapSiding { exposure } => Pattern::LapSiding {
                exposure: exposure * k,
            },
            Pattern::BoardAndBatten { spacing } => Pattern::BoardAndBatten {
                spacing: spacing * k,
            },
            Pattern::Tile { w, h } => Pattern::Tile { w: w * k, h: h * k },
            Pattern::Herringbone { length, width } => Pattern::Herringbone {
                length: length * k,
                width: width * k,
            },
            Pattern::None
            | Pattern::Insulation
            | Pattern::Concrete
            | Pattern::Earth
            | Pattern::Grass => self.clone(),
        }
    }

    /// Does the hatch have a direction of its own that an angle can turn
    /// exactly (line sets turn in place)?
    fn is_line_set(&self) -> bool {
        matches!(self, Pattern::Lines { .. } | Pattern::CrossHatch { .. })
    }
}

/// [`pattern_strokes`] for a pattern drawn `scale` times its size and turned
/// `angle_deg` degrees counter-clockwise (the Pattern tab of the Material
/// Specification). Line sets take the angle directly; the courses, tiles and
/// boards of the other patterns are drawn over a square that covers the
/// rectangle's circumscribed circle and turned about the rectangle's centre,
/// so callers still clip the result to their polygon. The result is capped at
/// [`MAX_STROKES`].
pub fn pattern_strokes_turned(
    p: &Pattern,
    rect: (Point, Point),
    scale_in_per_ft: f64,
    scale: f64,
    angle_deg: f64,
) -> Vec<(Point, Point)> {
    let p = p.scaled(scale);
    let angle = if angle_deg.is_finite() {
        angle_deg
    } else {
        0.0
    };
    if angle.rem_euclid(360.0) < 1e-9 {
        return pattern_strokes(&p, rect, scale_in_per_ft);
    }
    if p.is_line_set() {
        let turned = match p {
            Pattern::Lines { angle_deg, spacing } => Pattern::Lines {
                angle_deg: angle_deg + angle,
                spacing,
            },
            Pattern::CrossHatch { angle_deg, spacing } => Pattern::CrossHatch {
                angle_deg: angle_deg + angle,
                spacing,
            },
            other => other,
        };
        return pattern_strokes(&turned, rect, scale_in_per_ft);
    }
    let (a, b) = rect;
    let c = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let r = 0.5 * ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    let square = (Point::new(c.x - r, c.y - r), Point::new(c.x + r, c.y + r));
    let (sin, cos) = angle.to_radians().sin_cos();
    let turn = |q: Point| {
        let (dx, dy) = (q.x - c.x, q.y - c.y);
        Point::new(c.x + dx * cos - dy * sin, c.y + dx * sin + dy * cos)
    };
    pattern_strokes(&p, square, scale_in_per_ft)
        .into_iter()
        .map(|(q, w)| (turn(q), turn(w)))
        .collect()
}

type Seg = (Point, Point);

/// Axis-aligned clip window with a bounded segment sink.
struct Sink {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    out: Vec<Seg>,
}

impl Sink {
    fn full(&self) -> bool {
        self.out.len() >= MAX_STROKES
    }

    /// Clips `a`-`b` to the window and stores the visible part.
    fn push(&mut self, a: Point, b: Point) {
        if self.full() {
            return;
        }
        if let Some((c, d)) = clip_to_rect(a, b, self.x0, self.y0, self.x1, self.y1) {
            if c.dist(d) > 1e-9 {
                self.out.push((c, d));
            }
        }
    }

    fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    fn height(&self) -> f64 {
        self.y1 - self.y0
    }
}

/// Liang-Barsky clip of a segment to a rectangle.
fn clip_to_rect(a: Point, b: Point, x0: f64, y0: f64, x1: f64, y1: f64) -> Option<Seg> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, a.x - x0),
        (dx, x1 - a.x),
        (-dy, a.y - y0),
        (dy, y1 - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                if t > t1 {
                    return None;
                }
                t0 = t0.max(t);
            } else {
                if t < t0 {
                    return None;
                }
                t1 = t1.min(t);
            }
        }
    }
    let at = |t: f64| Point::new((a.x + dx * t).clamp(x0, x1), (a.y + dy * t).clamp(y0, y1));
    Some((at(t0), at(t1)))
}

/// Grows `spacing` until a drawing at `scale_in_per_ft` (paper inches per
/// foot) would not put lines closer than 1/32" apart on paper.
fn thin(spacing: f64, scale_in_per_ft: f64) -> f64 {
    const MIN_PAPER_IN: f64 = 1.0 / 32.0;
    if !(scale_in_per_ft.is_finite() && scale_in_per_ft > 0.0) {
        return spacing;
    }
    let paper = spacing * scale_in_per_ft / 12.0;
    if paper >= MIN_PAPER_IN {
        spacing
    } else {
        spacing * (MIN_PAPER_IN / paper).ceil()
    }
}

/// Generates the segments of `p` covering `rect` (two opposite corners, in
/// inches), clipped to the rectangle and capped at [`MAX_STROKES`].
///
/// `scale_in_per_ft` is the drawing scale in paper inches per real foot
/// (1/4" = 1'-0" is `0.25`); features that would be denser than 1/32" on
/// paper are coarsened so the hatch stays legible. Line, cross-hatch and
/// stipple patterns use absolute coordinates (neighbouring regions line up);
/// courses, tiles and boards start at the rectangle's lower-left corner.
/// Degenerate rectangles or non-positive dimensions give no strokes.
pub fn pattern_strokes(
    p: &Pattern,
    rect: (Point, Point),
    scale_in_per_ft: f64,
) -> Vec<(Point, Point)> {
    let (a, b) = rect;
    let mut s = Sink {
        x0: a.x.min(b.x),
        y0: a.y.min(b.y),
        x1: a.x.max(b.x),
        y1: a.y.max(b.y),
        out: Vec::new(),
    };
    let finite = [s.x0, s.y0, s.x1, s.y1].iter().all(|v| v.is_finite());
    if !finite || s.width() <= 0.0 || s.height() <= 0.0 {
        return Vec::new();
    }
    let sc = scale_in_per_ft;
    match *p {
        Pattern::None => {}
        Pattern::Lines { angle_deg, spacing } => hatch(&mut s, angle_deg, thin(spacing, sc)),
        Pattern::CrossHatch { angle_deg, spacing } => {
            let sp = thin(spacing, sc);
            hatch(&mut s, angle_deg, sp);
            hatch(&mut s, angle_deg + 90.0, sp);
        }
        Pattern::Brick { length, height } | Pattern::Block { length, height } => {
            let (l, h) = (thin(length, sc), thin(height, sc));
            courses(&mut s, l, h, |k| if k % 2 == 1 { l / 2.0 } else { 0.0 });
        }
        Pattern::Shingle { exposure, width } => {
            let (e, w) = (thin(exposure, sc), thin(width, sc));
            courses(&mut s, w, e, |k| unit_f64(hash64(k as u64 ^ 0x5348)) * w);
        }
        Pattern::LapSiding { exposure } => lap(&mut s, thin(exposure, sc)),
        Pattern::BoardAndBatten { spacing } => board_and_batten(&mut s, thin(spacing, sc)),
        Pattern::Tile { w, h } => courses(&mut s, thin(w, sc), thin(h, sc), |_| 0.0),
        Pattern::Herringbone { length, width } => {
            herringbone(&mut s, thin(length, sc), thin(width, sc))
        }
        Pattern::Insulation => insulation(&mut s, sc),
        Pattern::Concrete => concrete(&mut s, thin(3.0, sc)),
        Pattern::Earth => earth(&mut s, thin(6.0, sc)),
        Pattern::Grass => grass(&mut s, thin(6.0, sc)),
    }
    s.out
}

/// Parallel lines through the window, anchored at the origin.
fn hatch(s: &mut Sink, angle_deg: f64, spacing: f64) {
    if !(spacing.is_finite() && spacing > 0.0) {
        return;
    }
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    let d = Point::new(cos, sin);
    let n = Point::new(-sin, cos);
    let proj =
        [(s.x0, s.y0), (s.x1, s.y0), (s.x0, s.y1), (s.x1, s.y1)].map(|(x, y)| x * n.x + y * n.y);
    let lo = proj.iter().copied().fold(f64::MAX, f64::min);
    let hi = proj.iter().copied().fold(f64::MIN, f64::max);
    // Keep the line count within the budget by coarsening the spacing.
    let spacing = spacing.max((hi - lo) / (MAX_STROKES as f64 / 2.0));
    let reach = 2.0 * s.x0.abs().max(s.x1.abs()).hypot(s.y0.abs().max(s.y1.abs())) + spacing;
    let (k0, k1) = ((lo / spacing).ceil() as i64, (hi / spacing).floor() as i64);
    for k in k0..=k1 {
        let c = n * (k as f64 * spacing);
        s.push(c - d * reach, c + d * reach);
    }
}

/// Courses of height `h` from the bottom; head joints every `len`, with
/// course `k` shifted right by `offset(k)` inches.
fn courses(s: &mut Sink, len: f64, h: f64, offset: impl Fn(i64) -> f64) {
    if !(len > 0.0 && h > 0.0) {
        return;
    }
    // Coarsen uniformly so rows x joints stays inside the stroke budget.
    let budget = MAX_STROKES as f64 / 2.0;
    let est = (s.height() / h) * (s.width() / len);
    let f = if est > budget {
        (est / budget).sqrt()
    } else {
        1.0
    };
    let (h, len) = (h * f, len * f);
    let rows = (s.height() / h).ceil() as i64;
    for k in 0..rows {
        if s.full() {
            return;
        }
        let (y_lo, y_hi) = (s.y0 + k as f64 * h, (s.y0 + (k + 1) as f64 * h).min(s.y1));
        if k > 0 {
            s.push(Point::new(s.x0, y_lo), Point::new(s.x1, y_lo));
        }
        let mut x = s.x0 + offset(k).rem_euclid(len);
        while x < s.x1 && !s.full() {
            if x > s.x0 + 1e-9 {
                s.push(Point::new(x, y_lo), Point::new(x, y_hi));
            }
            x += len;
        }
    }
}

/// Horizontal board lines every `exposure`.
fn lap(s: &mut Sink, exposure: f64) {
    if !(exposure.is_finite() && exposure > 0.0) {
        return;
    }
    let exposure = exposure.max(s.height() / (MAX_STROKES as f64 / 2.0));
    let mut y = s.y0 + exposure;
    while y < s.y1 && !s.full() {
        s.push(Point::new(s.x0, y), Point::new(s.x1, y));
        y += exposure;
    }
}

/// Batten pairs 1 1/2" wide centred on each board joint.
fn board_and_batten(s: &mut Sink, spacing: f64) {
    if !(spacing.is_finite() && spacing > 0.0) {
        return;
    }
    let spacing = spacing.max(s.width() / (MAX_STROKES as f64 / 4.0));
    let half = (1.5_f64).min(spacing * 0.5) / 2.0;
    let mut x = s.x0 + spacing;
    while x < s.x1 + half && !s.full() {
        for dx in [-half, half] {
            s.push(Point::new(x + dx, s.y0), Point::new(x + dx, s.y1));
        }
        x += spacing;
    }
}

/// 90-degree herringbone: each plank is `n` = length / width units long.
fn herringbone(s: &mut Sink, length: f64, width: f64) {
    if !(length > 0.0 && width > 0.0) {
        return;
    }
    let n = (length / width).round().max(1.0) as i64;
    let w = width.max(s.width().max(s.height()) / 60.0);
    let (nf, period) = (n as f64, 2.0 * n as f64);
    let (cols_w, rows_h) = (s.width() / w, s.height() / w);
    let plank = |s: &mut Sink, x: f64, y: f64, rw: f64, rh: f64| {
        let p = |px: f64, py: f64| Point::new(s.x0 + px * w, s.y0 + py * w);
        let (p0, p1, p2, p3) = (p(x, y), p(x + rw, y), p(x + rw, y + rh), p(x, y + rh));
        s.push(p0, p1);
        s.push(p1, p2);
        s.push(p2, p3);
        s.push(p3, p0);
    };
    for i in (-n - 1)..(rows_h.ceil() as i64 + n + 1) {
        let fi = i as f64;
        let m_lo = ((-fi - 2.0 * nf) / period).floor() as i64;
        let m_hi = ((cols_w - fi) / period).ceil() as i64;
        for m in m_lo..=m_hi {
            if s.full() {
                return;
            }
            let shift = m as f64 * period;
            // Horizontal plank in row i, vertical plank right of it going down.
            plank(s, fi + shift, fi, nf, 1.0);
            plank(s, fi + nf + shift, fi - nf + 1.0, 1.0, nf);
        }
    }
}

/// Zig-zag along the long axis of the window.
fn insulation(s: &mut Sink, scale: f64) {
    let horizontal = s.width() >= s.height();
    let (long, short) = if horizontal {
        (s.width(), s.height())
    } else {
        (s.height(), s.width())
    };
    let pitch = thin((short * 0.5).max(1.0), scale).max(long / (MAX_STROKES as f64 / 2.0));
    let mut t = 0.0;
    let mut flip = false;
    while t < long && !s.full() {
        let t2 = (t + pitch).min(long);
        let (c0, c1) = if flip { (short, 0.0) } else { (0.0, short) };
        let (a, b) = if horizontal {
            (
                Point::new(s.x0 + t, s.y0 + c0),
                Point::new(s.x0 + t2, s.y0 + c1),
            )
        } else {
            (
                Point::new(s.x0 + c0, s.y0 + t),
                Point::new(s.x0 + c1, s.y0 + t2),
            )
        };
        s.push(a, b);
        t = t2;
        flip = !flip;
    }
}

/// Visits every cell of an absolute grid touching the window. The cell size
/// is enlarged if needed to keep at most ~6,500 cells; the closure receives
/// the effective size.
fn for_cells(s: &mut Sink, cell: f64, mut f: impl FnMut(&mut Sink, i64, i64, f64)) {
    if !(cell.is_finite() && cell > 0.0) {
        return;
    }
    let cell = cell.max(s.width().max(s.height()) / 80.0);
    let (i0, i1) = ((s.x0 / cell).floor() as i64, (s.x1 / cell).floor() as i64);
    let (j0, j1) = ((s.y0 / cell).floor() as i64, (s.y1 / cell).floor() as i64);
    for j in j0..=j1 {
        for i in i0..=i1 {
            if s.full() {
                return;
            }
            f(s, i, j, cell);
        }
    }
}

/// Per-cell jitter in `[0, 1)²`, stable for a given absolute cell.
fn jitter(i: i64, j: i64, salt: u64) -> (f64, f64) {
    let h = hash64(hash64(i as u64 ^ salt) ^ (j as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    (unit_f64(h), unit_f64(h.rotate_left(32)))
}

/// Short random dashes, one per cell.
fn concrete(s: &mut Sink, cell: f64) {
    for_cells(s, cell, |s, i, j, cell| {
        let (jx, jy) = jitter(i, j, 0xC0);
        let p = Point::new((i as f64 + jx) * cell, (j as f64 + jy) * cell);
        let ang = jitter(i, j, 0xC1).0 * std::f64::consts::PI;
        s.push(
            p,
            p + Point::new(ang.cos(), ang.sin()) * (cell * 0.12).max(0.1),
        );
    });
}

/// Horizontal rows of diagonal ticks.
fn earth(s: &mut Sink, pitch: f64) {
    for_cells(s, pitch, |s, i, j, pitch| {
        let base = Point::new(i as f64 * pitch, j as f64 * pitch);
        let tick = pitch * 0.5;
        s.push(base, base + Point::new(pitch * 0.5, 0.0));
        s.push(
            base + Point::new(pitch * 0.25, 0.0),
            base + Point::new(pitch * 0.25 - tick, -tick),
        );
    });
}

/// Three-stroke tufts.
fn grass(s: &mut Sink, pitch: f64) {
    for_cells(s, pitch, |s, i, j, pitch| {
        let (jx, jy) = jitter(i, j, 0x6A);
        let base = Point::new((i as f64 + jx) * pitch, (j as f64 + jy) * pitch);
        let h = pitch * 0.5;
        s.push(base, base + Point::new(0.0, h));
        s.push(base, base + Point::new(-h * 0.5, h * 0.9));
        s.push(base, base + Point::new(h * 0.5, h * 0.9));
    });
}

/// Clips segments to a (possibly concave) polygon, keeping the inside parts.
///
/// Each segment is split at every polygon edge crossing and the pieces whose
/// midpoints lie inside (even-odd rule) are returned, so gable walls and
/// other non-rectangular regions can reuse [`pattern_strokes`] output.
pub fn clip_strokes_to_polygon(
    strokes: &[(Point, Point)],
    polygon: &[Point],
) -> Vec<(Point, Point)> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut ts: Vec<f64> = Vec::new();
    for &(a, b) in strokes {
        let d = b - a;
        ts.clear();
        ts.extend([0.0, 1.0]);
        for (i, &e0) in polygon.iter().enumerate() {
            let e = polygon[(i + 1) % polygon.len()] - e0;
            let denom = d.cross(e);
            if denom.abs() < 1e-12 {
                continue;
            }
            let t = (e0 - a).cross(e) / denom;
            let u = (e0 - a).cross(d) / denom;
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                ts.push(t);
            }
        }
        ts.sort_by(f64::total_cmp);
        for w in ts.windows(2) {
            if w[1] - w[0] < 1e-12 {
                continue;
            }
            let (p, q) = (Point::lerp(a, b, w[0]), Point::lerp(a, b, w[1]));
            if p.dist(q) > 1e-9 && point_in_polygon(Point::lerp(p, q, 0.5), polygon) {
                out.push((p, q));
            }
        }
    }
    out
}

/// Even-odd point-in-polygon test.
fn point_in_polygon(pt: Point, poly: &[Point]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (pi, pj) = (poly[i], poly[j]);
        if (pi.y > pt.y) != (pj.y > pt.y)
            && pt.x < (pj.x - pi.x) * (pt.y - pi.y) / (pj.y - pi.y) + pi.x
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The material pattern a Fill Style stands for, so a plan fill and the
/// hatch of a section or elevation agree: lines and cross hatch keep their
/// spacing and angle, Brick, Grid and Herringbone their cell size, Concrete
/// and Sand become the stipple and earth patterns. `None` for Solid, Use
/// Layer, Library patterns and the system patterns without a material
/// counterpart (Dots, U's).
pub fn pattern_of_fill(style: &plan_core::fill_styles::FillStyle) -> Option<Pattern> {
    use plan_core::fill_styles::{PatternType, SystemPattern};
    let PatternType::System(sp) = &style.pattern else {
        return None;
    };
    Some(match sp {
        SystemPattern::Lines => Pattern::Lines {
            angle_deg: style.angle_deg,
            spacing: style.width,
        },
        SystemPattern::CrossHatch => Pattern::CrossHatch {
            angle_deg: style.angle_deg,
            spacing: style.width,
        },
        SystemPattern::Brick | SystemPattern::GridOffset => Pattern::Brick {
            length: style.width,
            height: style.height,
        },
        SystemPattern::Grid | SystemPattern::GridStep => Pattern::Tile {
            w: style.width,
            h: style.height,
        },
        SystemPattern::Herringbone => Pattern::Herringbone {
            length: style.width,
            width: style.height,
        },
        SystemPattern::Concrete => Pattern::Concrete,
        SystemPattern::Sand => Pattern::Earth,
        SystemPattern::Solid | SystemPattern::Us | SystemPattern::Dots => return None,
    })
}

#[cfg(test)]
mod fill_tests {
    use super::*;
    use plan_core::fill_styles::{FillStyle, SystemPattern};

    #[test]
    fn a_fill_style_maps_to_the_material_pattern_with_its_size() {
        assert_eq!(
            pattern_of_fill(&FillStyle::hatch(30.0, 9.0, [0; 3])),
            Some(Pattern::Lines {
                angle_deg: 30.0,
                spacing: 9.0
            })
        );
        assert_eq!(
            pattern_of_fill(&FillStyle::system(SystemPattern::Brick, 8.0, 2.25)),
            Some(Pattern::brick())
        );
        assert_eq!(
            pattern_of_fill(&FillStyle::system(SystemPattern::Concrete, 6.0, 6.0)),
            Some(Pattern::Concrete)
        );
        assert!(pattern_of_fill(&FillStyle::solid([0; 3])).is_none());
        assert!(pattern_of_fill(&FillStyle::library("x")).is_none());
        // The mapped pattern makes strokes over a rectangle.
        let p = pattern_of_fill(&FillStyle::hatch(0.0, 12.0, [0; 3])).unwrap();
        let rect = (Point::new(0.0, 0.0), Point::new(48.0, 48.0));
        assert!(!pattern_strokes(&p, rect, 0.25).is_empty());
    }
}
