//! Landing rules: automatic height and thickness, adjacent landings and the
//! short edge of a landing between two sections.

use plan_core::Point;

/// Edges of two landings this close (inches) belong together.
pub const ADJACENT_TOLERANCE: f64 = 1.0;
/// Thickness of a landing that is not attached to a floor structure (a
/// free-standing landing), inches.
pub const FREE_STANDING_THICKNESS: f64 = 6.75;
/// The shortest edge a landing between two sections at less than 90 degrees
/// is created with, inches.
pub const MIN_SHORT_EDGE: f64 = 6.0;

/// Auto Adjust Thickness: one riser plus the floor finish; a free-standing
/// landing is [`FREE_STANDING_THICKNESS`].
pub fn auto_thickness(riser: f64, floor_finish: f64, free_standing: bool) -> f64 {
    if free_standing {
        FREE_STANDING_THICKNESS
    } else {
        (riser + floor_finish).max(0.0)
    }
}

/// Auto Adjust Height: the landing's top is the top of the section that
/// arrives on it; a landing the sections only leave from is as high as the
/// stair that starts there already assumed. With no arriving section the
/// height stays as it is.
pub fn auto_height(arriving_tops: &[f64], current: f64) -> f64 {
    arriving_tops
        .iter()
        .copied()
        .fold(None, |m: Option<f64>, t| Some(m.map_or(t, |m| m.max(t))))
        .unwrap_or(current)
}

/// The top of a landing placed next to an earlier one: one riser higher.
pub fn adjacent_height(earlier_top: f64, riser: f64) -> f64 {
    earlier_top + riser
}

/// The edges where two landing outlines meet: `(i, j)` when edge `i` of `a`
/// (from `a[i]` to `a[i+1]`) and edge `j` of `b` run alongside each other
/// within `tolerance` and overlap by more than the tolerance. Both edges
/// are to lose their railing.
pub fn adjacent_edges(a: &[Point], b: &[Point], tolerance: f64) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let (n, m) = (a.len(), b.len());
    if n < 3 || m < 3 {
        return out;
    }
    for i in 0..n {
        let (p, q) = (a[i], a[(i + 1) % n]);
        let len = p.dist(q);
        if len < 1e-6 {
            continue;
        }
        let d = (q - p) * (1.0 / len);
        for j in 0..m {
            let (r, s) = (b[j], b[(j + 1) % m]);
            let off = |x: Point| (x - p).cross(d).abs();
            if off(r) > tolerance || off(s) > tolerance {
                continue;
            }
            let t = |x: Point| (x - p).x * d.x + (x - p).y * d.y;
            let (lo, hi) = (t(r).min(t(s)), t(r).max(t(s)));
            if hi.min(len) - lo.max(0.0) > tolerance {
                out.push((i, j));
            }
        }
    }
    out
}

/// Do the two outlines have an edge in common (within the 1 inch)?
pub fn are_adjacent(a: &[Point], b: &[Point]) -> bool {
    !adjacent_edges(a, b, ADJACENT_TOLERANCE).is_empty()
}

/// The short edge of a landing between two sections whose directions turn
/// by less than 90 degrees: the natural edge, but never under 6 inches.
pub fn short_edge(natural: f64, turn_radians: f64) -> f64 {
    if turn_radians.abs() < std::f64::consts::FRAC_PI_2 {
        natural.max(MIN_SHORT_EDGE)
    } else {
        natural
    }
}
