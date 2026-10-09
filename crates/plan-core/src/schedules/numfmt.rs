//! Number Formatting of a schedule column (Schedule Specification > Number
//! Formatting, manual pp. 724 to 725).
//!
//! A column that holds a number (a width, an area, a count) can carry its own
//! [`NumFormat`]: the units, whether the unit marks show, leading and trailing
//! zeros, a thousands separator, and the accuracy as decimal places or as the
//! smallest fraction of an inch, with the denominator shown or not and the
//! fraction reduced by the greatest common divisor or to the closest simple
//! fraction. A column with no format keeps the plan's own length text
//! ([`crate::units::fmt_ft_in`]); the default [`NumFormat`] reproduces that
//! text exactly, so choosing "Feet-Inches, 1/16" changes nothing.
//!
//! The fraction style (horizontal, diagonal, vertical) and its text size are
//! properties of the whole schedule ([`FractionFormat`]); they change how a
//! drawn fraction is set, not the characters of the cell text.

use serde::{Deserialize, Serialize};

/// What a numeric column measures; sets the raw unit and the formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NumKind {
    /// Inches.
    Length,
    /// Square feet.
    Area,
    /// Cubic feet.
    Volume,
    /// Feet (a perimeter, a linear run).
    Feet,
    /// A count or quantity.
    Count,
    /// Board feet.
    BoardFeet,
}

/// The unit a length column is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NumUnit {
    /// `12'-6 1/2"`.
    #[default]
    FeetInches,
    Feet,
    Inches,
    Millimeters,
    Centimeters,
    Meters,
}

impl NumUnit {
    pub const ALL: [NumUnit; 6] = [
        NumUnit::FeetInches,
        NumUnit::Feet,
        NumUnit::Inches,
        NumUnit::Millimeters,
        NumUnit::Centimeters,
        NumUnit::Meters,
    ];

    pub fn name(self) -> &'static str {
        match self {
            NumUnit::FeetInches => "Feet and Inches",
            NumUnit::Feet => "Feet",
            NumUnit::Inches => "Inches",
            NumUnit::Millimeters => "Millimeters",
            NumUnit::Centimeters => "Centimeters",
            NumUnit::Meters => "Meters",
        }
    }

    fn mark(self) -> &'static str {
        match self {
            NumUnit::FeetInches => "",
            NumUnit::Feet => "'",
            NumUnit::Inches => "\"",
            NumUnit::Millimeters => " mm",
            NumUnit::Centimeters => " cm",
            NumUnit::Meters => " m",
        }
    }

    /// Inches to this unit (not for feet-inches).
    fn from_inches(self, v: f64) -> f64 {
        match self {
            NumUnit::FeetInches | NumUnit::Inches => v,
            NumUnit::Feet => v / 12.0,
            NumUnit::Millimeters => v * 25.4,
            NumUnit::Centimeters => v * 2.54,
            NumUnit::Meters => v * 0.0254,
        }
    }
}

/// How precisely a number is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accuracy {
    /// Decimal Places, 0 to 20.
    Decimal(u8),
    /// Smallest Fraction: the largest denominator, 1 to 128 (1 is whole
    /// numbers).
    Fraction(u16),
}

/// How a fraction is reduced when "Reduce Fractions" is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Reduce {
    /// Divide the numerator and denominator by their greatest common divisor
    /// (best for fractional inches).
    #[default]
    Gcd,
    /// The simplest fraction that is close to the value, without regard to
    /// the largest denominator (0.333 shows as 1/3).
    Closest,
}

/// The digit grouping of values over 999.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Thousands {
    #[default]
    None,
    Comma,
    Dot,
    Space,
    NonBreakingSpace,
}

impl Thousands {
    pub const ALL: [Thousands; 5] = [
        Thousands::None,
        Thousands::Comma,
        Thousands::Dot,
        Thousands::Space,
        Thousands::NonBreakingSpace,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Thousands::None => "None",
            Thousands::Comma => "Use Comma",
            Thousands::Dot => "Use Dot",
            Thousands::Space => "Use Space",
            Thousands::NonBreakingSpace => "Use Non-Breaking Space",
        }
    }

    fn sep(self) -> Option<char> {
        match self {
            Thousands::None => None,
            Thousands::Comma => Some(','),
            Thousands::Dot => Some('.'),
            Thousands::Space => Some(' '),
            Thousands::NonBreakingSpace => Some('\u{a0}'),
        }
    }
}

/// The format of one column.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NumFormat {
    pub units: NumUnit,
    pub unit_indicators: bool,
    pub leading_zeros: bool,
    pub trailing_zeros: bool,
    pub thousands: Thousands,
    pub accuracy: Accuracy,
    pub show_denominator: bool,
    pub reduce_fractions: bool,
    pub reduce_mode: Reduce,
}

impl Default for NumFormat {
    /// Feet-inches to the sixteenth: the plan's own length text.
    fn default() -> Self {
        Self {
            units: NumUnit::FeetInches,
            unit_indicators: true,
            leading_zeros: true,
            trailing_zeros: true,
            thousands: Thousands::None,
            accuracy: Accuracy::Fraction(16),
            show_denominator: true,
            reduce_fractions: true,
            reduce_mode: Reduce::Gcd,
        }
    }
}

impl NumFormat {
    /// The format a column of `kind` starts with when the user turns number
    /// formatting on.
    pub fn default_for(kind: NumKind) -> Self {
        match kind {
            NumKind::Length => Self::default(),
            NumKind::Area | NumKind::Volume | NumKind::Feet | NumKind::BoardFeet => Self {
                unit_indicators: false,
                trailing_zeros: true,
                accuracy: Accuracy::Decimal(1),
                ..Self::default()
            },
            NumKind::Count => Self {
                unit_indicators: false,
                accuracy: Accuracy::Decimal(0),
                ..Self::default()
            },
        }
    }
}

/// How fractions are set (Fraction Format).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FractionStyle {
    /// `1/2` on the line.
    #[default]
    Horizontal,
    Diagonal,
    Vertical,
}

impl FractionStyle {
    pub const ALL: [FractionStyle; 3] = [
        FractionStyle::Horizontal,
        FractionStyle::Diagonal,
        FractionStyle::Vertical,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FractionStyle::Horizontal => "Horizontal",
            FractionStyle::Diagonal => "Diagonal",
            FractionStyle::Vertical => "Vertical",
        }
    }
}

/// The Fraction Format of a schedule.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FractionFormat {
    pub style: FractionStyle,
    /// Fraction text size, percent of the number height (Diagonal and
    /// Vertical only).
    pub text_pct: f64,
}

impl Default for FractionFormat {
    fn default() -> Self {
        Self {
            style: FractionStyle::Horizontal,
            text_pct: 70.0,
        }
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// The simplest `n/d` (denominator up to 128) within 0.002 of `f` (0 to 1).
fn closest_fraction(f: f64) -> (u64, u64) {
    for d in 1..=128u64 {
        let n = (f * d as f64).round();
        if (f - n / d as f64).abs() <= 0.002 {
            return (n as u64, d);
        }
    }
    let n = (f * 128.0).round() as u64;
    (n, 128)
}

/// `n/d` of a fraction already rounded to `1/den`, shown per the format;
/// `None` when it is zero.
fn fraction_text(num: u64, den: u64, real: f64, f: &NumFormat) -> Option<String> {
    if num == 0 {
        return None;
    }
    let (mut n, mut d) = (num, den);
    if f.reduce_fractions {
        match f.reduce_mode {
            Reduce::Gcd => {
                let g = gcd(n, d);
                n /= g;
                d /= g;
            }
            Reduce::Closest => {
                let (cn, cd) = closest_fraction(real);
                n = cn;
                d = cd;
            }
        }
    }
    Some(if f.show_denominator {
        format!("{n}/{d}")
    } else {
        n.to_string()
    })
}

/// `1234567` as `1,234,567`.
pub fn group_thousands(int_digits: &str, sep: Option<char>) -> String {
    let Some(sep) = sep else {
        return int_digits.to_string();
    };
    let digits: Vec<char> = int_digits.chars().collect();
    let mut out = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(sep);
        }
        out.push(*c);
    }
    out
}

/// `v` with `places` decimals, zeros trimmed unless `trailing`.
fn decimal_text(v: f64, places: u8, trailing: bool, leading: bool, sep: Option<char>) -> String {
    let places = places.min(20) as usize;
    let s = format!("{:.*}", places, v.abs());
    let (int, frac) = s.split_once('.').map_or((s.as_str(), ""), |(a, b)| (a, b));
    let frac = if trailing {
        frac.to_string()
    } else {
        frac.trim_end_matches('0').to_string()
    };
    let int = if int == "0" && !leading && !frac.is_empty() {
        String::new()
    } else {
        group_thousands(int, sep)
    };
    let sign = if v < 0.0 && (int != "0" || !frac.is_empty()) {
        "-"
    } else {
        ""
    };
    if frac.is_empty() {
        format!("{sign}{int}")
    } else {
        format!("{sign}{int}.{frac}")
    }
}

/// A value split into whole units and a fraction, per the accuracy.
fn fractional_text(v: f64, f: &NumFormat, den: u16) -> String {
    let den = u64::from(den.clamp(1, 128));
    let neg = v < 0.0;
    let total = (v.abs() * den as f64).round() as u64;
    let whole = total / den;
    let rem = total % den;
    let frac = fraction_text(rem, den, v.abs().fract(), f);
    let whole_s = group_thousands(&whole.to_string(), f.thousands.sep());
    let sign = if neg && total > 0 { "-" } else { "" };
    match frac {
        // A bare fraction needs no leading zero.
        Some(fr) if whole == 0 => format!("{sign}{fr}"),
        Some(fr) => format!("{sign}{whole_s} {fr}"),
        None => format!("{sign}{whole_s}"),
    }
}

/// Feet and inches (`12'-6 1/2"`).
fn feet_inches_text(inches: f64, f: &NumFormat) -> String {
    let neg = inches < 0.0;
    let (feet, inch_s, inch_zero) = match f.accuracy {
        Accuracy::Fraction(d) => {
            let den = u64::from(d.clamp(1, 128));
            let total = (inches.abs() * den as f64).round() as u64;
            let feet = total / (12 * den);
            let rem = total % (12 * den);
            let whole_in = rem / den;
            let frac = rem % den;
            let mut s = whole_in.to_string();
            if let Some(fr) = fraction_text(frac, den, inches.abs().fract(), f) {
                s.push(' ');
                s.push_str(&fr);
            }
            (feet, s, rem == 0)
        }
        Accuracy::Decimal(n) => {
            let n = n.min(20);
            let scale = 10f64.powi(i32::from(n));
            let total = (inches.abs() * scale).round() / scale;
            let feet = (total / 12.0).floor();
            let mut rem = total - feet * 12.0;
            if rem < 0.0 {
                rem = 0.0;
            }
            let text = decimal_text(rem, n, f.trailing_zeros, true, None);
            (feet as u64, text, rem == 0.0)
        }
    };
    let marks = f.unit_indicators;
    let feet_s = group_thousands(&feet.to_string(), f.thousands.sep());
    let sign = if neg && (feet > 0 || !inch_zero) { "-" } else { "" };
    let mut out = String::from(sign);
    let show_feet = feet > 0 || f.leading_zeros;
    let show_inches = !inch_zero || f.trailing_zeros;
    if show_feet {
        out.push_str(&feet_s);
        if marks {
            out.push('\'');
        }
    }
    if show_inches {
        if show_feet {
            out.push('-');
        }
        out.push_str(&inch_s);
        if marks {
            out.push('"');
        }
    } else if !show_feet {
        out.push('0');
        if marks {
            out.push('"');
        }
    }
    out
}

/// `v` (in the raw unit of `kind`: inches, square feet, ...) as column text.
pub fn format_value(v: f64, kind: NumKind, f: &NumFormat) -> String {
    match kind {
        NumKind::Length => match f.units {
            NumUnit::FeetInches => feet_inches_text(v, f),
            u => {
                let x = u.from_inches(v);
                let body = match f.accuracy {
                    Accuracy::Decimal(n) => {
                        decimal_text(x, n, f.trailing_zeros, f.leading_zeros, f.thousands.sep())
                    }
                    Accuracy::Fraction(d) => fractional_text(x, f, d),
                };
                if f.unit_indicators {
                    format!("{body}{}", u.mark())
                } else {
                    body
                }
            }
        },
        NumKind::Count => {
            let n = match f.accuracy {
                Accuracy::Decimal(n) => n,
                Accuracy::Fraction(_) => 0,
            };
            decimal_text(v, n, f.trailing_zeros, f.leading_zeros, f.thousands.sep())
        }
        NumKind::Area | NumKind::Volume | NumKind::Feet | NumKind::BoardFeet => {
            let places = match f.accuracy {
                Accuracy::Decimal(n) => n,
                Accuracy::Fraction(_) => 1,
            };
            let body = decimal_text(v, places, f.trailing_zeros, f.leading_zeros, f.thousands.sep());
            if !f.unit_indicators {
                return body;
            }
            let mark = match kind {
                NumKind::Area => " sq ft",
                NumKind::Volume => " cu ft",
                NumKind::Feet => " ft",
                _ => " bf",
            };
            format!("{body}{mark}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::fmt_ft_in;

    #[test]
    fn the_default_format_is_the_plan_text() {
        let f = NumFormat::default();
        for v in [0.0, 0.5, 36.0, 80.0, 109.125, 150.5, 5.3333, 144.0, 12.0, 119.9] {
            assert_eq!(format_value(v, NumKind::Length, &f), fmt_ft_in(v), "{v}");
        }
    }

    #[test]
    fn smallest_fraction_and_denominator_control() {
        let mut f = NumFormat::default();
        f.accuracy = Accuracy::Fraction(8);
        // 3/16 rounds to the nearest eighth.
        assert_eq!(format_value(75.1875, NumKind::Length, &f), "6'-3 1/4\"");
        f.accuracy = Accuracy::Fraction(1);
        assert_eq!(format_value(75.4, NumKind::Length, &f), "6'-3\"");
        // Without reducing, the chosen denominator stays.
        f.accuracy = Accuracy::Fraction(16);
        f.reduce_fractions = false;
        assert_eq!(format_value(75.5, NumKind::Length, &f), "6'-3 8/16\"");
        // Denominator hidden: eighths as a bare numerator.
        f.reduce_fractions = true;
        f.accuracy = Accuracy::Fraction(8);
        f.show_denominator = false;
        assert_eq!(format_value(75.375, NumKind::Length, &f), "6'-3 3\"");
    }

    #[test]
    fn gcd_and_closest_fractions_differ() {
        let mut f = NumFormat::default();
        f.units = NumUnit::Inches;
        f.unit_indicators = false;
        f.accuracy = Accuracy::Fraction(16);
        assert_eq!(format_value(0.333, NumKind::Length, &f), "5/16");
        f.reduce_mode = Reduce::Closest;
        assert_eq!(format_value(0.333, NumKind::Length, &f), "1/3");
    }

    #[test]
    fn zeros_decimals_and_thousands() {
        let mut f = NumFormat::default();
        f.trailing_zeros = false;
        assert_eq!(format_value(36.0, NumKind::Length, &f), "3'");
        assert_eq!(format_value(39.5, NumKind::Length, &f), "3'-3 1/2\"");
        f.leading_zeros = false;
        assert_eq!(format_value(6.5, NumKind::Length, &f), "6 1/2\"");
        let d = NumFormat {
            units: NumUnit::Inches,
            accuracy: Accuracy::Decimal(2),
            ..NumFormat::default()
        };
        assert_eq!(format_value(6.5, NumKind::Length, &d), "6.50\"");
        let mut t = NumFormat::default_for(NumKind::Area);
        t.thousands = Thousands::Comma;
        assert_eq!(format_value(12345.67, NumKind::Area, &t), "12,345.7");
        t.accuracy = Accuracy::Decimal(0);
        t.unit_indicators = true;
        assert_eq!(format_value(1999.6, NumKind::Area, &t), "2,000 sq ft");
        let c = NumFormat::default_for(NumKind::Count);
        assert_eq!(format_value(7.0, NumKind::Count, &c), "7");
    }

    #[test]
    fn metric_units() {
        let f = NumFormat {
            units: NumUnit::Millimeters,
            accuracy: Accuracy::Decimal(0),
            ..NumFormat::default()
        };
        assert_eq!(format_value(36.0, NumKind::Length, &f), "914 mm");
        let m = NumFormat {
            units: NumUnit::Meters,
            accuracy: Accuracy::Decimal(2),
            ..NumFormat::default()
        };
        assert_eq!(format_value(80.0, NumKind::Length, &m), "2.03 m");
    }
}
