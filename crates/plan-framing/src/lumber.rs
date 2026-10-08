//! Dimensional lumber: actual (dressed) sizes and their nominal names.

use serde::{Deserialize, Serialize};

/// A piece of dimensional lumber, in actual (dressed) inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Lumber {
    /// Actual thickness, 1.5" for every "2x" size.
    pub thickness: f64,
    /// Actual depth: 3.5, 5.5, 7.25, 9.25 or 11.25 for the standard sizes.
    pub depth: f64,
}

/// Actual thickness of a "2x" board.
pub const TWO_BY_THICKNESS: f64 = 1.5;

/// 2x4 (1 1/2" x 3 1/2").
pub const TWO_BY_FOUR: Lumber = Lumber::two_by(3.5);
/// 2x6 (1 1/2" x 5 1/2").
pub const TWO_BY_SIX: Lumber = Lumber::two_by(5.5);
/// 2x8 (1 1/2" x 7 1/4").
pub const TWO_BY_EIGHT: Lumber = Lumber::two_by(7.25);
/// 2x10 (1 1/2" x 9 1/4").
pub const TWO_BY_TEN: Lumber = Lumber::two_by(9.25);
/// 2x12 (1 1/2" x 11 1/4").
pub const TWO_BY_TWELVE: Lumber = Lumber::two_by(11.25);

/// Nominal size for an actual dimension (3.5 -> 4, 5.5 -> 6, 7.25 -> 8, ...).
fn nominal(actual: f64) -> u32 {
    (actual + 0.5).round().max(0.0) as u32
}

impl Lumber {
    /// A 1 1/2" thick board of the given actual depth.
    pub const fn two_by(depth: f64) -> Self {
        Self {
            thickness: TWO_BY_THICKNESS,
            depth,
        }
    }

    /// Nominal thickness in inches (1.5 -> 2).
    pub fn nominal_thickness(&self) -> u32 {
        nominal(self.thickness)
    }

    /// Nominal depth in inches (5.5 -> 6).
    pub fn nominal_depth(&self) -> u32 {
        nominal(self.depth)
    }

    /// Nominal name such as `"2x6"`.
    pub fn nominal_name(&self) -> String {
        format!("{}x{}", self.nominal_thickness(), self.nominal_depth())
    }
}

/// Format a length in inches as whole inches plus a reduced fraction rounded to
/// 1/16", e.g. `104 5/8`, `39`, `3/4`.
pub fn format_inches(inches: f64) -> String {
    let sixteenths = (inches.abs() * 16.0).round() as i64;
    let sign = if inches < 0.0 && sixteenths != 0 {
        "-"
    } else {
        ""
    };
    let (whole, mut n, mut d) = (sixteenths / 16, sixteenths % 16, 16);
    while n != 0 && n % 2 == 0 {
        n /= 2;
        d /= 2;
    }
    match (whole, n) {
        (w, 0) => format!("{sign}{w}"),
        (0, _) => format!("{sign}{n}/{d}"),
        (w, _) => format!("{sign}{w} {n}/{d}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nominal_names() {
        assert_eq!(TWO_BY_FOUR.nominal_name(), "2x4");
        assert_eq!(TWO_BY_SIX.nominal_name(), "2x6");
        assert_eq!(TWO_BY_EIGHT.nominal_name(), "2x8");
        assert_eq!(TWO_BY_TEN.nominal_name(), "2x10");
        assert_eq!(TWO_BY_TWELVE.nominal_name(), "2x12");
    }

    #[test]
    fn inch_formatting() {
        assert_eq!(format_inches(104.625), "104 5/8");
        assert_eq!(format_inches(39.0), "39");
        assert_eq!(format_inches(0.75), "3/4");
        assert_eq!(format_inches(92.625), "92 5/8");
    }
}
