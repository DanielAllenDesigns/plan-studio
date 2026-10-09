//! The text of a dimension: the primary number, the secondary format, the
//! tolerance, the rounding method and the rounded-value indicators (manual
//! pp. 477, 479 to 480, 489 to 491, in our own words).

use crate::units::{
    format_length, LengthFormat, LengthUnit, CM_PER_INCH, INCHES_PER_FOOT, MM_PER_INCH,
    M_PER_INCH,
};
use serde::{Deserialize, Serialize};

use super::settings::{RoundMethod, TextPos};

/// The Secondary Format panel: a second number for the same distance in
/// other units or another accuracy. Chief shows it on the far side of the
/// line from the primary number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SecondFormat {
    /// "Include Second Format".
    pub include: bool,
    pub format: LengthFormat,
}

impl Default for SecondFormat {
    fn default() -> Self {
        Self {
            include: false,
            // The usual reason for a second number: the other unit system.
            format: LengthFormat {
                unit: LengthUnit::Millimeters,
                decimals: 0,
                unit_indicators: true,
                ..LengthFormat::default()
            },
        }
    }
}

/// How a plus-minus tolerance is written after the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TolMode {
    #[default]
    None,
    /// `12'-0" \u{b1}1/4"`.
    Symmetric,
    /// `12'-0" +1/4"/-1/8"`.
    Deviation,
    /// The two limits: `12'-0 1/4" / 11'-11 7/8"`.
    Limits,
}

impl TolMode {
    pub const ALL: [TolMode; 4] = [
        TolMode::None,
        TolMode::Symmetric,
        TolMode::Deviation,
        TolMode::Limits,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TolMode::None => "None",
            TolMode::Symmetric => "Plus or Minus",
            TolMode::Deviation => "Separate Plus and Minus",
            TolMode::Limits => "Limits",
        }
    }
}

/// A tolerance written after the number, in plan inches.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Tolerance {
    pub mode: TolMode,
    pub plus: f64,
    pub minus: f64,
}

impl Tolerance {
    /// `primary` (the number as written) with the tolerance for `value`
    /// (inches), the tolerance values written in `fmt`.
    pub fn apply(&self, primary: &str, value: f64, fmt: &LengthFormat) -> String {
        let w = |v: f64| format_length(v.abs(), fmt);
        match self.mode {
            TolMode::None => primary.to_string(),
            TolMode::Symmetric => format!("{primary} \u{b1}{}", w(self.plus)),
            TolMode::Deviation => format!("{primary} +{}/-{}", w(self.plus), w(self.minus)),
            TolMode::Limits => format!(
                "{} / {}",
                format_length(value + self.plus, fmt),
                format_length(value - self.minus, fmt)
            ),
        }
    }
}

/// What the Dimension Defaults decide about every dimension's number beyond
/// the primary format; carried by [`super::DimFormat`], so everything that
/// writes a dimension's text sees it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DimLabelOptions {
    pub second: SecondFormat,
    pub tolerance: Tolerance,
    pub rounding: RoundMethod,
    /// "+ or - after number".
    pub plus_minus_after: bool,
    /// "~ before number".
    pub tilde_before: bool,
    pub position: TextPos,
    /// A fixed label angle, degrees; `None` is automatic.
    pub angle: Option<f64>,
    /// Angular dimensions: decimals of a degree, or degrees-minutes-seconds.
    pub angle_decimals: u8,
    pub angle_dms: bool,
}

impl Default for DimLabelOptions {
    fn default() -> Self {
        Self {
            second: SecondFormat::default(),
            tolerance: Tolerance::default(),
            rounding: RoundMethod::Grid,
            plus_minus_after: false,
            tilde_before: false,
            position: TextPos::Above,
            angle: None,
            angle_decimals: 1,
            angle_dms: false,
        }
    }
}

/// The smallest distance `f` can show, plan inches.
pub fn step_inches(f: &LengthFormat) -> f64 {
    let dec = 10f64.powi(-(f.decimals.min(12) as i32));
    match f.unit {
        LengthUnit::FeetInches | LengthUnit::Inches => 1.0 / f64::from(f.fraction_denominator.max(1)),
        LengthUnit::DecimalFeet => dec * INCHES_PER_FOOT,
        LengthUnit::Millimeters => dec / MM_PER_INCH,
        LengthUnit::Centimeters => dec / CM_PER_INCH,
        LengthUnit::Meters => dec / M_PER_INCH,
    }
}

/// `v` rounded to a whole number of `step`s.
pub fn round_to_step(v: f64, step: f64) -> f64 {
    if step <= 1e-12 {
        return v;
    }
    (v / step).round() * step
}

/// Grid Rounding for a run of parts: every part is the difference between
/// two rounded positions along the run, so the parts always add up to the
/// whole (manual p. 477). `values` are the true lengths in order.
pub fn grid_round(values: &[f64], step: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity(values.len());
    let (mut pos, mut shown_pos) = (0.0_f64, 0.0_f64);
    for v in values {
        pos += v;
        let r = round_to_step(pos, step);
        out.push(r - shown_pos);
        shown_pos = r;
    }
    out
}

/// The rounded-value indicators for a number whose true value is `truth` and
/// whose shown value is `shown`: `(prefix, suffix)`. A `+` means the true
/// value is higher than the shown one, a `-` lower, and a `~` before the
/// number that it is not exact.
pub fn indicators(truth: f64, shown: f64, step: f64, after: bool, tilde: bool) -> (String, String) {
    let eps = (step * 0.01).max(1e-9);
    let diff = truth - shown;
    if diff.abs() <= eps {
        return (String::new(), String::new());
    }
    let prefix = if tilde { "~" } else { "" }.to_string();
    let suffix = if after {
        if diff > 0.0 { "+" } else { "-" }
    } else {
        ""
    }
    .to_string();
    (prefix, suffix)
}

/// An angle in degrees as dimension text: decimal degrees, or degrees,
/// minutes and seconds.
pub fn angle_text(deg: f64, decimals: u8, dms: bool) -> String {
    if !dms {
        return format!("{deg:.*}\u{b0}", usize::from(decimals.min(6)));
    }
    let total = (deg.abs() * 3600.0).round();
    let d = (total / 3600.0).floor();
    let m = ((total - d * 3600.0) / 60.0).floor();
    let s = total - d * 3600.0 - m * 60.0;
    let sign = if deg < 0.0 { "-" } else { "" };
    if s > 0.0 {
        format!("{sign}{d:.0}\u{b0}{m:.0}'{s:.0}\"")
    } else if m > 0.0 {
        format!("{sign}{d:.0}\u{b0}{m:.0}'")
    } else {
        format!("{sign}{d:.0}\u{b0}")
    }
}

/// A dimension label: the primary number and, with a second format, the
/// second one (which sits on the other side of the line).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LabelParts {
    pub primary: String,
    pub second: Option<String>,
}

impl LabelParts {
    /// The label on one line: `primary (second)`.
    pub fn one_line(&self) -> String {
        match &self.second {
            Some(s) if !s.is_empty() => format!("{} ({s})", self.primary),
            _ => self.primary.clone(),
        }
    }

    /// The label as the lines it is drawn on, primary first.
    pub fn lines(&self) -> Vec<String> {
        let mut v = vec![self.primary.clone()];
        if let Some(s) = self.second.as_ref().filter(|s| !s.is_empty()) {
            v.push(s.clone());
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imperial(denom: u32) -> LengthFormat {
        LengthFormat {
            fraction_denominator: denom,
            ..LengthFormat::default()
        }
    }

    #[test]
    fn steps_follow_the_format() {
        assert!((step_inches(&imperial(8)) - 0.125).abs() < 1e-12);
        let mm = LengthFormat {
            unit: LengthUnit::Millimeters,
            decimals: 0,
            ..LengthFormat::default()
        };
        assert!((step_inches(&mm) - 1.0 / 25.4).abs() < 1e-12);
        let ft = LengthFormat {
            unit: LengthUnit::DecimalFeet,
            decimals: 2,
            ..LengthFormat::default()
        };
        assert!((step_inches(&ft) - 0.12).abs() < 1e-12);
    }

    #[test]
    fn grid_rounding_makes_the_parts_add_up() {
        // Three parts of 1/3 inch at a 1 inch grid: distance rounding gives
        // 0 + 0 + 0 (a sum of 0 for a whole of 1); grid rounding 0, 1, 0.
        let parts = [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0];
        let g = grid_round(&parts, 1.0);
        assert!((g.iter().sum::<f64>() - 1.0).abs() < 1e-9, "{g:?}");
        let d: f64 = parts.iter().map(|p| round_to_step(*p, 1.0)).sum();
        assert_eq!(d, 0.0);
        // Equal parts of 4.4 and 4.4 and 4.4 at a 1 inch grid.
        let g = grid_round(&[4.4, 4.4, 4.4], 1.0);
        assert_eq!(g, vec![4.0, 5.0, 4.0]);
        assert_eq!(g.iter().sum::<f64>(), round_to_step(13.2, 1.0));
    }

    #[test]
    fn indicators_say_which_way_the_truth_lies() {
        let (p, s) = indicators(10.4, 10.0, 1.0, true, true);
        assert_eq!((p.as_str(), s.as_str()), ("~", "+"));
        let (p, s) = indicators(9.6, 10.0, 1.0, true, false);
        assert_eq!((p.as_str(), s.as_str()), ("", "-"));
        let (p, s) = indicators(10.0, 10.0, 1.0, true, true);
        assert_eq!((p.as_str(), s.as_str()), ("", ""));
        let (p, s) = indicators(10.4, 10.0, 1.0, false, false);
        assert_eq!((p.as_str(), s.as_str()), ("", ""));
    }

    #[test]
    fn tolerances_write_after_the_number() {
        let f = imperial(8);
        let t = Tolerance {
            mode: TolMode::Symmetric,
            plus: 0.25,
            minus: 0.25,
        };
        assert_eq!(t.apply("12'-0\"", 144.0, &f), "12'-0\" \u{b1}0'-0 1/4\"");
        let t = Tolerance {
            mode: TolMode::Limits,
            plus: 0.25,
            minus: 0.125,
        };
        assert_eq!(t.apply("12'-0\"", 144.0, &f), "12'-0 1/4\" / 11'-11 7/8\"");
        let t = Tolerance {
            mode: TolMode::None,
            ..t
        };
        assert_eq!(t.apply("x", 1.0, &f), "x");
    }

    #[test]
    fn angles_in_degrees_or_dms() {
        assert_eq!(angle_text(90.0, 1, false), "90.0\u{b0}");
        assert_eq!(angle_text(33.25, 2, false), "33.25\u{b0}");
        assert_eq!(angle_text(33.5, 1, true), "33\u{b0}30'");
        assert_eq!(angle_text(10.0 + 15.0 / 60.0 + 30.0 / 3600.0, 1, true), "10\u{b0}15'30\"");
    }

    #[test]
    fn parts_join_on_one_line() {
        let p = LabelParts {
            primary: "10'".into(),
            second: Some("3048".into()),
        };
        assert_eq!(p.one_line(), "10' (3048)");
        assert_eq!(p.lines(), vec!["10'", "3048"]);
        assert_eq!(LabelParts::default().lines(), vec![String::new()]);
    }
}
