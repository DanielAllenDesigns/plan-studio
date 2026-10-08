//! Material hatches for visible faces.
//!
//! Each [`RegionKind::Face`] region gets the elevation pattern of its
//! material, generated over the polygon's bounding box with
//! `plan_materials::pattern_strokes` and clipped to the polygon (holes
//! excluded) as Light [`EdgeKind::Hatch`] lines.
//!
//! | Material | Pattern |
//! |---|---|
//! | Brick | running-bond brick |
//! | Siding | lap siding, 6" exposure |
//! | Stucco | sparse concrete stipple (every third dash) |
//! | Stone | irregular coursed stone, see [`stone_strokes`] |
//! | Roof | shingle courses |
//! | Concrete | concrete stipple |
//! | Glass, WindowGlass | 45 degree lines, 6" apart |
//! | everything else | none |
//!
//! Courses and stones start at the region's lower-left corner; stipple and
//! line patterns use absolute coordinates, so neighbouring regions line up.

use crate::drawing::{EdgeKind, Line2, LineWeight, Region, RegionKind};
use plan_3d::Material;
use plan_core::Point;
use plan_materials::{clip_strokes_to_polygon, pattern_strokes, Pattern, MAX_STROKES};

/// Drawing scale used to coarsen dense patterns: 1/4" = 1'-0" paper inches per foot.
const HATCH_SCALE: f64 = 0.25;

/// The 2D pattern for a material, `None` when it is not hatched.
fn pattern_for(m: Material) -> Option<Pattern> {
    match m {
        Material::Brick => Some(Pattern::brick()),
        Material::Siding => Some(Pattern::LapSiding { exposure: 6.0 }),
        Material::Stucco | Material::Concrete => Some(Pattern::Concrete),
        Material::Roof => Some(Pattern::shingle()),
        Material::Glass | Material::WindowGlass => Some(Pattern::Lines {
            angle_deg: 45.0,
            spacing: 6.0,
        }),
        _ => None,
    }
}

/// Splitmix64 finaliser.
fn hash(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform value in `[0, 1)` for two integer keys and a salt.
fn unit(a: i64, b: i64, salt: u64) -> f64 {
    let h = hash(hash(a as u64 ^ salt) ^ (b as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93));
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Irregular coursed stone over `rect`: rows 6-10" high, stones 10-22" wide,
/// joints wobbling by up to 1" and leaning slightly. Deterministic.
pub(crate) fn stone_strokes(rect: (Point, Point)) -> Vec<(Point, Point)> {
    let (a, b) = rect;
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    if !(x1 > x0 && y1 > y0) || ![x0, x1, y0, y1].iter().all(|v| v.is_finite()) {
        return Vec::new();
    }
    // Keep the stroke count bounded on huge faces by growing the stones.
    let grow = (((x1 - x0) * (y1 - y0)) / (14.0 * 8.0 * (MAX_STROKES as f64 / 6.0)))
        .sqrt()
        .max(1.0);
    let wobble = |row: i64, x: f64| -> f64 {
        let k = (x / 12.0).floor();
        let t = x / 12.0 - k;
        let (w0, w1) = (
            unit(row, k as i64, 0x51) - 0.5,
            unit(row, k as i64 + 1, 0x51) - 0.5,
        );
        (w0 + (w1 - w0) * t) * 2.0
    };
    let mut out: Vec<(Point, Point)> = Vec::new();
    // Row boundaries, bottom up.
    let mut row_y = vec![y0];
    let mut r = 0i64;
    while *row_y.last().unwrap_or(&y1) < y1 && out.len() < MAX_STROKES {
        let height = (6.0 + 4.0 * unit(r, 0, 0x52)) * grow;
        row_y.push((row_y[row_y.len() - 1] + height).min(y1));
        r += 1;
    }
    let y_at = |row: usize, x: f64| -> f64 {
        let y = row_y[row];
        if row == 0 || row + 1 == row_y.len() {
            y
        } else {
            (y + wobble(row as i64, x)).clamp(y0, y1)
        }
    };
    for row in 1..row_y.len() - 1 {
        let mut x = x0;
        while x < x1 {
            let nx = (x + 12.0 * grow).min(x1);
            out.push((Point::new(x, y_at(row, x)), Point::new(nx, y_at(row, nx))));
            x = nx;
        }
    }
    for row in 0..row_y.len() - 1 {
        let mut x = x0 - 22.0 * unit(row as i64, 1, 0x53) * grow;
        let mut j = 0i64;
        while x < x1 && out.len() < MAX_STROKES {
            if x > x0 {
                let lean = (unit(row as i64, j, 0x54) - 0.5) * 1.5;
                out.push((
                    Point::new(x, y_at(row, x)),
                    Point::new(x + lean, y_at(row + 1, x + lean)),
                ));
            }
            x += (10.0 + 12.0 * unit(row as i64, j + 1, 0x55)) * grow;
            j += 1;
        }
    }
    out.truncate(MAX_STROKES);
    out
}

fn bbox(poly: &[Point]) -> (Point, Point) {
    poly.iter().fold(
        (
            Point::new(f64::INFINITY, f64::INFINITY),
            Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
        ),
        |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        },
    )
}

/// Hatch strokes of one region (empty for unhatched materials and non-face regions).
pub(crate) fn region_strokes(region: &Region) -> Vec<(Point, Point)> {
    if region.kind != RegionKind::Face || region.polygon.len() < 3 {
        return Vec::new();
    }
    let rect = bbox(&region.polygon);
    let raw = if region.material == Material::Stone {
        stone_strokes(rect)
    } else if let Some(p) = pattern_for(region.material) {
        let mut s = pattern_strokes(&p, rect, HATCH_SCALE);
        if region.material == Material::Stucco {
            s = s.into_iter().step_by(3).collect();
        }
        s
    } else {
        return Vec::new();
    };
    clip_strokes_to_polygon(&raw, &region.polygon)
}

/// Light hatch lines for every face region.
pub(crate) fn hatch_lines(regions: &[Region]) -> Vec<Line2> {
    regions
        .iter()
        .flat_map(region_strokes)
        .map(|(a, b)| Line2 {
            a,
            b,
            weight: LineWeight::Light,
            kind: EdgeKind::Hatch,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stone_pattern_fills_the_rect_deterministically() {
        let rect = (Point::new(0.0, 0.0), Point::new(120.0, 96.0));
        let a = stone_strokes(rect);
        assert!(a.len() > 20);
        assert_eq!(a, stone_strokes(rect));
        for (p, q) in &a {
            for v in [p, q] {
                assert!((-2.0..=122.0).contains(&v.x) && (-0.01..=96.01).contains(&v.y));
            }
        }
    }

    #[test]
    fn unmapped_materials_have_no_hatch() {
        for m in [Material::WallInterior, Material::Trim, Material::Metal] {
            assert!(pattern_for(m).is_none());
        }
    }
}
