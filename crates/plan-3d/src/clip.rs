//! 2D clipping and the top profile of a wall that follows the roof.
//!
//! Wall faces are drawn in wall-local `(s, h)` coordinates: `s` along the wall
//! from its start, `h` up from the wall's bottom. A [`TopProfile`] is the
//! height of the wall's top along `s` as a run of straight pieces (a gable
//! triangle is two pieces, a hip-clipped wall is one sloped piece per
//! stretch under a plane, a step where a plane ends is a jump between two
//! pieces). Wall faces are rectangles (with openings cut) clipped to the
//! region under the profile.

/// A point in `(s, h)` or plan space.
pub type P2 = (f64, f64);

/// Tolerance, inches.
const EPS: f64 = 1e-6;

/// Signed area of a polygon (counter-clockwise positive).
pub fn area(poly: &[P2]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        * 0.5
}

/// The part of the polygon where `a*x + b*y + c >= 0` (Sutherland-Hodgman;
/// exact for a convex polygon, which every caller passes).
pub fn clip_half_plane(poly: &[P2], a: f64, b: f64, c: f64) -> Vec<P2> {
    let f = |p: P2| a * p.0 + b * p.1 + c;
    let n = poly.len();
    let mut out = Vec::with_capacity(n + 2);
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let (fp, fq) = (f(p), f(q));
        if fp >= 0.0 {
            out.push(p);
        }
        if (fp >= 0.0) != (fq >= 0.0) {
            let t = fp / (fp - fq);
            out.push((p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t));
        }
    }
    out
}

/// One straight stretch of a wall top: height `h0` at `s0`, `h1` at `s1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub s0: f64,
    pub s1: f64,
    pub h0: f64,
    pub h1: f64,
}

impl Piece {
    /// Height of the piece at `s` (extrapolated outside it).
    pub fn at(&self, s: f64) -> f64 {
        if self.s1 - self.s0 <= EPS {
            return self.h0;
        }
        self.h0 + (self.h1 - self.h0) * (s - self.s0) / (self.s1 - self.s0)
    }

    fn slope(&self) -> f64 {
        if self.s1 - self.s0 <= EPS {
            0.0
        } else {
            (self.h1 - self.h0) / (self.s1 - self.s0)
        }
    }
}

/// Which side of a jump [`TopProfile::at`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Before,
    After,
}

/// The top of a wall along its length: contiguous [`Piece`]s from `s = 0` to
/// the wall length, heights above the wall's bottom.
#[derive(Debug, Clone, PartialEq)]
pub struct TopProfile {
    pub pieces: Vec<Piece>,
}

impl TopProfile {
    /// A level top at `h` over `length`.
    pub fn flat(length: f64, h: f64) -> Self {
        Self {
            pieces: vec![Piece {
                s0: 0.0,
                s1: length,
                h0: h,
                h1: h,
            }],
        }
    }

    /// Highest point of the top.
    pub fn max_height(&self) -> f64 {
        self.pieces
            .iter()
            .flat_map(|p| [p.h0, p.h1])
            .fold(0.0, f64::max)
    }

    /// Lowest point of the top.
    pub fn min_height(&self) -> f64 {
        self.pieces
            .iter()
            .flat_map(|p| [p.h0, p.h1])
            .fold(f64::INFINITY, f64::min)
    }

    /// Is the top level at `h` everywhere?
    pub fn is_flat_at(&self, h: f64) -> bool {
        self.pieces
            .iter()
            .all(|p| (p.h0 - h).abs() < 1e-4 && (p.h1 - h).abs() < 1e-4)
    }

    /// Height of the top at `s`; at a jump, the value on `side`.
    pub fn at(&self, s: f64, side: Side) -> f64 {
        let hit = |p: &&Piece| s >= p.s0 - EPS && s <= p.s1 + EPS;
        let found = match side {
            Side::Before => self
                .pieces
                .iter()
                .find(|p| p.s0 < s - EPS && s <= p.s1 + EPS)
                .or_else(|| self.pieces.iter().find(hit)),
            Side::After => self
                .pieces
                .iter()
                .find(|p| s >= p.s0 - EPS && s < p.s1 - EPS)
                .or_else(|| self.pieces.iter().rev().find(hit)),
        };
        match (found, self.pieces.first(), self.pieces.last()) {
            (Some(p), _, _) => p.at(s),
            (None, Some(f), _) if s < f.s0 => f.h0,
            (None, _, Some(l)) => l.h1,
            _ => 0.0,
        }
    }

    /// Lowest height of the top over `s0..s1`.
    pub fn min_over(&self, s0: f64, s1: f64) -> f64 {
        let mut lo = self.at(s0, Side::After).min(self.at(s1, Side::Before));
        for p in &self.pieces {
            for (s, h) in [(p.s0, p.h0), (p.s1, p.h1)] {
                if s > s0 + EPS && s < s1 - EPS {
                    lo = lo.min(h);
                }
            }
        }
        lo
    }

    /// The rectangle `s0..s1` x `h0..h1` cut to the region under the top, as
    /// convex polygons (one per piece it touches).
    pub fn clip_rect(&self, s0: f64, s1: f64, h0: f64, h1: f64) -> Vec<Vec<P2>> {
        let mut out = Vec::new();
        for p in &self.pieces {
            let (x0, x1) = (s0.max(p.s0), s1.min(p.s1));
            if x1 - x0 <= EPS {
                continue;
            }
            let rect = [(x0, h0), (x1, h0), (x1, h1), (x0, h1)];
            // Keep h <= piece(s):  slope*s - h + (h0 - slope*s0) >= 0.
            let k = p.slope();
            let clipped = clip_half_plane(&rect, k, -1.0, p.h0 - k * p.s0);
            let clipped = dedup(&clipped);
            if clipped.len() >= 3 && area(&clipped).abs() > 1e-6 {
                out.push(clipped);
            }
        }
        out
    }
}

impl TopProfile {
    /// The convex polygon `poly` (in `(s, h)`) cut to the region under the
    /// profile, one polygon per piece it touches.
    pub fn clip_below(&self, poly: &[P2]) -> Vec<Vec<P2>> {
        self.clip_side(poly, true)
    }

    /// The convex polygon `poly` cut to the region on or over the profile.
    pub fn clip_above(&self, poly: &[P2]) -> Vec<Vec<P2>> {
        self.clip_side(poly, false)
    }

    fn clip_side(&self, poly: &[P2], below: bool) -> Vec<Vec<P2>> {
        let mut out = Vec::new();
        for p in &self.pieces {
            if p.s1 - p.s0 <= EPS {
                continue;
            }
            let strip = clip_half_plane(poly, 1.0, 0.0, -p.s0);
            let strip = clip_half_plane(&strip, -1.0, 0.0, p.s1);
            // h <= line:  k*s - h + (h0 - k*s0) >= 0; h >= line is its negation.
            let k = p.slope();
            let c = p.h0 - k * p.s0;
            let clipped = if below {
                clip_half_plane(&strip, k, -1.0, c)
            } else {
                clip_half_plane(&strip, -k, 1.0, -c)
            };
            let clipped = dedup(&clipped);
            if clipped.len() >= 3 && area(&clipped).abs() > 1e-6 {
                out.push(clipped);
            }
        }
        out
    }

    /// Every height raised by `by`.
    pub fn raised(&self, by: f64) -> TopProfile {
        TopProfile {
            pieces: self
                .pieces
                .iter()
                .map(|p| Piece {
                    h0: p.h0 + by,
                    h1: p.h1 + by,
                    ..*p
                })
                .collect(),
        }
    }

    /// No height above `max`.
    pub fn clamped(&self, max: f64) -> TopProfile {
        let pieces = self
            .pieces
            .iter()
            .flat_map(|p| lower_envelope(p.s0, p.s1, &[(p.h0, p.h1), (max, max)]))
            .collect();
        TopProfile {
            pieces: merge(pieces),
        }
    }

    /// Every height lowered by `by`, none below zero.
    pub fn lowered(&self, by: f64) -> TopProfile {
        let pieces = self
            .pieces
            .iter()
            .flat_map(|p| upper_envelope(p.s0, p.s1, &[(p.h0 - by, p.h1 - by), (0.0, 0.0)]))
            .collect();
        TopProfile {
            pieces: merge(pieces),
        }
    }
}

fn dedup(poly: &[P2]) -> Vec<P2> {
    let mut out: Vec<P2> = Vec::with_capacity(poly.len());
    for &p in poly {
        if out
            .last()
            .is_none_or(|q| (q.0 - p.0).abs() > 1e-7 || (q.1 - p.1).abs() > 1e-7)
        {
            out.push(p);
        }
    }
    while out.len() > 1 {
        let (f, l) = (out[0], out[out.len() - 1]);
        if (f.0 - l.0).abs() <= 1e-7 && (f.1 - l.1).abs() <= 1e-7 {
            out.pop();
        } else {
            break;
        }
    }
    out
}

/// A linear height over the cell `[a, b]`: `h(a)` and `h(b)`.
pub type Line = (f64, f64);

/// Lower envelope of `lines` (each linear over `[a, b]`) as pieces.
pub fn lower_envelope(a: f64, b: f64, lines: &[Line]) -> Vec<Piece> {
    let len = b - a;
    if len <= EPS || lines.is_empty() {
        return Vec::new();
    }
    let value = |l: &Line, t: f64| l.0 + (l.1 - l.0) * t;
    // Crossings between pairs of lines, as fractions of the cell.
    let mut cuts = vec![0.0, 1.0];
    for i in 0..lines.len() {
        for j in i + 1..lines.len() {
            let (di, dj) = (lines[i].1 - lines[i].0, lines[j].1 - lines[j].0);
            let denom = di - dj;
            if denom.abs() > 1e-12 {
                let t = (lines[j].0 - lines[i].0) / denom;
                if t > 1e-9 && t < 1.0 - 1e-9 {
                    cuts.push(t);
                }
            }
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    let mut out: Vec<Piece> = Vec::new();
    for w in cuts.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        let mid = (t0 + t1) * 0.5;
        let best = lines
            .iter()
            .min_by(|x, y| value(x, mid).total_cmp(&value(y, mid)))
            .copied()
            .unwrap_or(lines[0]);
        let piece = Piece {
            s0: a + len * t0,
            s1: a + len * t1,
            h0: value(&best, t0),
            h1: value(&best, t1),
        };
        match out.last_mut() {
            Some(last)
                if (last.slope() - piece.slope()).abs() < 1e-9
                    && (last.h1 - piece.h0).abs() < 1e-7 =>
            {
                last.s1 = piece.s1;
                last.h1 = piece.h1;
            }
            _ => out.push(piece),
        }
    }
    out
}

/// Upper envelope of `lines` (each linear over `[a, b]`) as pieces.
pub fn upper_envelope(a: f64, b: f64, lines: &[Line]) -> Vec<Piece> {
    let negated: Vec<Line> = lines.iter().map(|l| (-l.0, -l.1)).collect();
    lower_envelope(a, b, &negated)
        .into_iter()
        .map(|p| Piece {
            h0: -p.h0,
            h1: -p.h1,
            ..p
        })
        .collect()
}

/// A profile over `length` made of the sloped `runs` `(s0, s1, h0, h1)`,
/// level at zero where no run lies (overlaps keep the earlier run).
pub fn profile_from_runs(length: f64, runs: &[(f64, f64, f64, f64)]) -> TopProfile {
    let mut runs: Vec<(f64, f64, f64, f64)> = runs
        .iter()
        .map(|&(a, b, h0, h1)| (a.max(0.0), b.min(length), h0.max(0.0), h1.max(0.0)))
        .filter(|r| r.1 - r.0 > EPS)
        .collect();
    runs.sort_by(|x, y| x.0.total_cmp(&y.0));
    let flat = |s0: f64, s1: f64| Piece {
        s0,
        s1,
        h0: 0.0,
        h1: 0.0,
    };
    let mut pieces: Vec<Piece> = Vec::new();
    let mut cursor = 0.0;
    for (mut s0, s1, mut h0, h1) in runs {
        if s1 <= cursor + EPS {
            continue;
        }
        if s0 < cursor {
            h0 += (h1 - h0) * (cursor - s0) / (s1 - s0);
            s0 = cursor;
        }
        if s0 - cursor > EPS {
            pieces.push(flat(cursor, s0));
        }
        pieces.push(Piece { s0, s1, h0, h1 });
        cursor = s1;
    }
    if length - cursor > EPS {
        pieces.push(flat(cursor, length));
    }
    TopProfile {
        pieces: merge(pieces),
    }
}

/// Merge neighbouring pieces that continue one straight line.
pub fn merge(pieces: Vec<Piece>) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for p in pieces {
        match out.last_mut() {
            Some(last)
                if (last.s1 - p.s0).abs() < 1e-7
                    && (last.slope() - p.slope()).abs() < 1e-9
                    && (last.h1 - p.h0).abs() < 1e-7 =>
            {
                last.s1 = p.s1;
                last.h1 = p.h1;
            }
            _ => out.push(p),
        }
    }
    out
}

/// The two points where a trimming line cut a polygon.
pub type Cut = ([f64; 3], [f64; 3]);

/// Plan-space vertical half-plane trimming of a roof plane polygon, in roof
/// space (`X = plan x`, `Y` up, `Z = -plan y`).
///
/// Keeps the vertices with `(p - origin) . normal >= distance` (plan
/// coordinates, `normal` a unit vector). Edges crossing the line are cut with
/// the height interpolated, so a planar polygon stays planar. Returns the new
/// polygon and the cut edge (the two points on the line) when the line cut
/// the polygon.
pub fn trim_polygon3(
    poly: &[[f64; 3]],
    origin: P2,
    normal: P2,
    distance: f64,
) -> (Vec<[f64; 3]>, Option<Cut>) {
    let f = |p: [f64; 3]| (p[0] - origin.0) * normal.0 + (-p[2] - origin.1) * normal.1 - distance;
    let n = poly.len();
    let mut out = Vec::with_capacity(n + 2);
    let mut cuts: Vec<[f64; 3]> = Vec::new();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let (fp, fq) = (f(p), f(q));
        if fp >= 0.0 {
            out.push(p);
        }
        if (fp >= 0.0) != (fq >= 0.0) {
            let t = fp / (fp - fq);
            let c = [
                p[0] + (q[0] - p[0]) * t,
                p[1] + (q[1] - p[1]) * t,
                p[2] + (q[2] - p[2]) * t,
            ];
            out.push(c);
            cuts.push(c);
        }
    }
    let cut = (cuts.len() == 2).then(|| (cuts[0], cuts[1]));
    (out, cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipping_a_rect_under_a_slope_gives_a_trapezoid() {
        let top = TopProfile {
            pieces: vec![Piece {
                s0: 0.0,
                s1: 100.0,
                h0: 50.0,
                h1: 150.0,
            }],
        };
        let polys = top.clip_rect(0.0, 100.0, 0.0, 200.0);
        assert_eq!(polys.len(), 1);
        assert!((area(&polys[0]).abs() - 10_000.0).abs() < 1e-6);
    }

    #[test]
    fn a_rect_wholly_under_the_top_is_unchanged() {
        let top = TopProfile::flat(100.0, 96.0);
        let polys = top.clip_rect(10.0, 40.0, 0.0, 96.0);
        assert_eq!(polys.len(), 1);
        assert_eq!(polys[0].len(), 4);
    }

    #[test]
    fn the_lower_envelope_of_a_roof_and_a_plate_has_a_knee() {
        // Level plate at 96 and a plane rising from 80 to 160 over 100".
        let env = lower_envelope(0.0, 100.0, &[(96.0, 96.0), (80.0, 160.0)]);
        assert_eq!(env.len(), 2);
        assert!((env[0].s1 - 20.0).abs() < 1e-9, "crosses at 20");
        assert_eq!(env[0].h0, 80.0);
        assert_eq!(env[1].h1, 96.0);
    }

    #[test]
    fn trimming_keeps_planarity_and_reports_the_cut() {
        // A plane rising along x, cut at x = 50.
        let poly = [
            [0.0, 0.0, 0.0],
            [100.0, 100.0, 0.0],
            [100.0, 100.0, -100.0],
            [0.0, 0.0, -100.0],
        ];
        let (kept, cut) = trim_polygon3(&poly, (50.0, 0.0), (-1.0, 0.0), 0.0);
        assert_eq!(kept.len(), 4);
        assert!(kept.iter().all(|p| p[0] <= 50.0 + 1e-9));
        let (a, b) = cut.unwrap();
        assert!((a[1] - 50.0).abs() < 1e-9 && (b[1] - 50.0).abs() < 1e-9);
    }

    #[test]
    fn profile_lookup_reads_either_side_of_a_jump() {
        let top = TopProfile {
            pieces: vec![
                Piece {
                    s0: 0.0,
                    s1: 10.0,
                    h0: 5.0,
                    h1: 5.0,
                },
                Piece {
                    s0: 10.0,
                    s1: 20.0,
                    h0: 9.0,
                    h1: 9.0,
                },
            ],
        };
        assert_eq!(top.at(10.0, Side::Before), 5.0);
        assert_eq!(top.at(10.0, Side::After), 9.0);
        assert_eq!(top.min_over(0.0, 20.0), 5.0);
    }
}
