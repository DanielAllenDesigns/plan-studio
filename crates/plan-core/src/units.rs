//! Unit helpers. The model stores inches; these convert for display/input,
//! in imperial (feet-inches) and metric (mm / cm / m) alike.

use serde::{Deserialize, Serialize};

/// Millimetres per inch (exact by definition).
pub const MM_PER_INCH: f64 = 25.4;
/// Centimetres per inch.
pub const CM_PER_INCH: f64 = 2.54;
/// Metres per inch.
pub const M_PER_INCH: f64 = 0.0254;
/// Inches per foot.
pub const INCHES_PER_FOOT: f64 = 12.0;
/// Square inches per square foot.
pub const SQ_IN_PER_SQ_FT: f64 = 144.0;
/// Square metres per square inch (0.0254^2).
pub const SQ_M_PER_SQ_IN: f64 = 0.000_645_16;

pub fn feet(f: f64) -> f64 {
    f * 12.0
}

pub fn sq_in_to_sq_ft(a: f64) -> f64 {
    a / 144.0
}

/// Format inches as architectural feet-inches, e.g. `12'-6 1/2"`, rounded to 1/16".
pub fn fmt_ft_in(inches: f64) -> String {
    fmt_ft_in_frac(inches, 16)
}

/// Like [`fmt_ft_in`] but rounded to `1/denom"` (e.g. 8 or 16). A `denom` of 0 is treated as 1.
pub fn fmt_ft_in_frac(inches: f64, denom: u32) -> String {
    let denom = i64::from(denom.max(1));
    let sign = if inches < 0.0 { "-" } else { "" };
    let total = (inches.abs() * denom as f64).round() as i64;
    let feet = total / (12 * denom);
    let rem = total % (12 * denom);
    let whole_in = rem / denom;
    let frac = rem % denom;
    let mut s = format!("{sign}{feet}'-{whole_in}");
    if frac != 0 {
        let mut n = frac;
        let mut d = denom;
        while n % 2 == 0 && d % 2 == 0 {
            n /= 2;
            d /= 2;
        }
        s.push_str(&format!(" {n}/{d}"));
    }
    s.push('"');
    s
}

/// Parse `12'`, `12' 6"`, `12'-6 1/2"`, `6"`, `1/2"`, `12.5'` or a bare number (inches).
pub fn parse_ft_in(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (feet_part, inch_part) = match s.find('\'') {
        Some(i) => (Some(&s[..i]), &s[i + 1..]),
        None => (None, s),
    };
    let mut total = 0.0;
    if let Some(f) = feet_part {
        total += f.trim().parse::<f64>().ok()? * 12.0;
    }
    let inch_part = inch_part
        .trim()
        .trim_start_matches('-')
        .trim()
        .trim_end_matches('"')
        .trim();
    if inch_part.is_empty() {
        return Some(total);
    }
    let mut inches = 0.0;
    for tok in inch_part.split_whitespace() {
        if let Some((n, d)) = tok.split_once('/') {
            let n: f64 = n.parse().ok()?;
            let d: f64 = d.parse().ok()?;
            if d == 0.0 {
                return None;
            }
            inches += n / d;
        } else {
            inches += tok.parse::<f64>().ok()?;
        }
    }
    Some(total + inches)
}

/// Square inches to square metres.
pub fn sq_in_to_sq_m(a: f64) -> f64 {
    a * SQ_M_PER_SQ_IN
}

/// Which family of units a plan is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UnitSystem {
    #[default]
    Imperial,
    Metric,
}

/// The unit a length is displayed or entered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LengthUnit {
    /// Architectural feet-inches with fractions, e.g. `12'-6 1/2"`.
    #[default]
    FeetInches,
    /// Inches with fractions, e.g. `150 1/2"`.
    Inches,
    /// Decimal feet, e.g. `10.25'`.
    DecimalFeet,
    Millimeters,
    Centimeters,
    Meters,
}

impl LengthUnit {
    pub fn system(self) -> UnitSystem {
        match self {
            LengthUnit::FeetInches | LengthUnit::Inches | LengthUnit::DecimalFeet => {
                UnitSystem::Imperial
            }
            LengthUnit::Millimeters | LengthUnit::Centimeters | LengthUnit::Meters => {
                UnitSystem::Metric
            }
        }
    }
}

/// How a length is turned into text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LengthFormat {
    pub unit: LengthUnit,
    /// Smallest fraction denominator for feet-inches / inches (8, 16, ...).
    pub fraction_denominator: u32,
    /// Decimal places for metric and decimal-feet output.
    pub decimals: u32,
    /// Show unit marks: `'` `"` for imperial, ` mm` / ` cm` / ` m` for metric.
    pub unit_indicators: bool,
    /// Keep trailing zeroes in decimals (`3.050` rather than `3.05`).
    pub trailing_zeroes: bool,
}

impl Default for LengthFormat {
    fn default() -> Self {
        Self {
            unit: LengthUnit::FeetInches,
            fraction_denominator: 16,
            decimals: 2,
            unit_indicators: true,
            trailing_zeroes: false,
        }
    }
}

impl LengthFormat {
    /// Feet-inches rounded to 1/8", as used at quarter-inch scale.
    pub fn imperial_quarter_scale() -> Self {
        Self {
            fraction_denominator: 8,
            ..Self::default()
        }
    }

    /// Whole millimetres with no unit text, e.g. `3048`.
    pub fn metric_mm() -> Self {
        Self {
            unit: LengthUnit::Millimeters,
            decimals: 0,
            unit_indicators: false,
            ..Self::default()
        }
    }
}

/// Snap away float noise (e.g. 1234.4999999999998) before rounding for display.
fn snap(v: f64) -> f64 {
    (v * 1e9).round() / 1e9
}

/// Fixed-decimal text, optionally trimming trailing zeroes; never `-0`.
fn fmt_decimal(v: f64, decimals: u32, trailing_zeroes: bool) -> String {
    let d = decimals.min(12);
    // Round half away from zero (format! alone rounds half to even).
    let scale = 10f64.powi(d as i32);
    let mut s = format!("{:.*}", d as usize, (snap(v) * scale).round() / scale);
    if !trailing_zeroes && s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// Whole inches plus a reduced fraction, rounded to `1/denom"`, e.g. `150 1/2`.
fn fmt_inches_frac(inches: f64, denom: u32) -> String {
    let denom = i64::from(denom.max(1));
    let total = (snap(inches.abs()) * denom as f64).round() as i64;
    let sign = if inches < 0.0 && total != 0 { "-" } else { "" };
    let whole = total / denom;
    let frac = total % denom;
    if frac == 0 {
        return format!("{sign}{whole}");
    }
    let (mut n, mut d) = (frac, denom);
    while n % 2 == 0 && d % 2 == 0 {
        n /= 2;
        d /= 2;
    }
    if whole == 0 {
        format!("{sign}{n}/{d}")
    } else {
        format!("{sign}{whole} {n}/{d}")
    }
}

/// Format a length given in inches according to `f`.
pub fn format_length(inches: f64, f: &LengthFormat) -> String {
    let ind = f.unit_indicators;
    match f.unit {
        LengthUnit::FeetInches => {
            let s = fmt_ft_in_frac(inches, f.fraction_denominator);
            if ind {
                s
            } else {
                s.replace(['\'', '"'], "")
            }
        }
        LengthUnit::Inches => {
            let s = fmt_inches_frac(inches, f.fraction_denominator);
            if ind {
                format!("{s}\"")
            } else {
                s
            }
        }
        LengthUnit::DecimalFeet => {
            let s = fmt_decimal(inches / INCHES_PER_FOOT, f.decimals, f.trailing_zeroes);
            if ind {
                format!("{s}'")
            } else {
                s
            }
        }
        LengthUnit::Millimeters => metric(inches * MM_PER_INCH, f, " mm"),
        LengthUnit::Centimeters => metric(inches * CM_PER_INCH, f, " cm"),
        LengthUnit::Meters => metric(inches * M_PER_INCH, f, " m"),
    }
}

fn metric(v: f64, f: &LengthFormat, suffix: &str) -> String {
    let s = fmt_decimal(v, f.decimals, f.trailing_zeroes);
    if f.unit_indicators {
        format!("{s}{suffix}")
    } else {
        s
    }
}

/// Parse a length to inches. A unit suffix (`mm`, `cm`, `m`, `ft`, `in`, `'`,
/// `"`) overrides `default_unit`; otherwise a bare number is read in
/// `default_unit` (bare feet-inches / inches input is inches).
pub fn parse_length(s: &str, default_unit: LengthUnit) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if t.contains('\'') || t.contains('"') {
        return parse_ft_in(t);
    }
    let lower = t.to_ascii_lowercase();
    let metric_num = |rest: &str, per_inch: f64| -> Option<f64> {
        let v: f64 = rest.trim().parse().ok()?;
        v.is_finite().then(|| v / per_inch)
    };
    if let Some(r) = lower.strip_suffix("mm") {
        return metric_num(r, MM_PER_INCH);
    }
    if let Some(r) = lower.strip_suffix("cm") {
        return metric_num(r, CM_PER_INCH);
    }
    if let Some(r) = lower.strip_suffix('m') {
        return metric_num(r, M_PER_INCH);
    }
    if let Some(r) = lower.strip_suffix("ft") {
        return parse_ft_in(&format!("{}'", r.trim()));
    }
    if let Some(r) = lower.strip_suffix("in") {
        return parse_ft_in(r);
    }
    match default_unit {
        LengthUnit::FeetInches => parse_bare_ft_in(t),
        LengthUnit::Inches => parse_ft_in(t),
        LengthUnit::DecimalFeet => parse_ft_in(&format!("{t}'")),
        LengthUnit::Millimeters => metric_num(t, MM_PER_INCH),
        LengthUnit::Centimeters => metric_num(t, CM_PER_INCH),
        LengthUnit::Meters => metric_num(t, M_PER_INCH),
    }
}

/// Feet-inches text written without marks, e.g. `12-6 1/2` (what
/// [`format_length`] emits with indicators off). Anything else is a bare
/// inches value as in [`parse_ft_in`].
fn parse_bare_ft_in(t: &str) -> Option<f64> {
    let (neg, body) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t),
    };
    if let Some((feet, rest)) = body.split_once('-') {
        if !feet.is_empty() && feet.chars().all(|c| c.is_ascii_digit()) {
            let v = parse_ft_in(&format!("{feet}'{rest}"))?;
            return Some(if neg { -v } else { v });
        }
    }
    parse_ft_in(t)
}

/// Format an area given in square inches: `212.5 sq ft` or `19.74 m²`.
pub fn format_area(sq_in: f64, system: UnitSystem) -> String {
    match system {
        UnitSystem::Imperial => format!("{:.1} sq ft", snap(sq_in_to_sq_ft(sq_in))),
        UnitSystem::Metric => format!("{:.2} m\u{b2}", snap(sq_in_to_sq_m(sq_in))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_common_values() {
        assert_eq!(fmt_ft_in(0.0), "0'-0\"");
        assert_eq!(fmt_ft_in(150.0), "12'-6\"");
        assert_eq!(fmt_ft_in(150.5), "12'-6 1/2\"");
        assert_eq!(fmt_ft_in(4.5), "0'-4 1/2\"");
        assert_eq!(fmt_ft_in(109.125), "9'-1 1/8\"");
    }

    #[test]
    fn parses_round_trip() {
        assert_eq!(parse_ft_in("12'-6 1/2\""), Some(150.5));
        assert_eq!(parse_ft_in("12' 6\""), Some(150.0));
        assert_eq!(parse_ft_in("12'"), Some(144.0));
        assert_eq!(parse_ft_in("6\""), Some(6.0));
        assert_eq!(parse_ft_in("36"), Some(36.0));
        assert_eq!(parse_ft_in("12.5'"), Some(150.0));
        assert_eq!(parse_ft_in("abc"), None);
    }

    const ALL_UNITS: [LengthUnit; 6] = [
        LengthUnit::FeetInches,
        LengthUnit::Inches,
        LengthUnit::DecimalFeet,
        LengthUnit::Millimeters,
        LengthUnit::Centimeters,
        LengthUnit::Meters,
    ];

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn default_and_presets() {
        let d = LengthFormat::default();
        assert_eq!(d.unit, LengthUnit::FeetInches);
        assert_eq!(d.fraction_denominator, 16);
        assert_eq!(
            LengthFormat::imperial_quarter_scale().fraction_denominator,
            8
        );
        let m = LengthFormat::metric_mm();
        assert_eq!(m.unit, LengthUnit::Millimeters);
        assert_eq!(m.decimals, 0);
        assert_eq!(LengthUnit::Meters.system(), UnitSystem::Metric);
        assert_eq!(LengthUnit::DecimalFeet.system(), UnitSystem::Imperial);
    }

    #[test]
    fn round_trip_every_unit_with_and_without_indicators() {
        // 120" = 3048 mm = 304.8 cm = 3.048 m = 10 ft.
        for unit in ALL_UNITS {
            for ind in [true, false] {
                let f = LengthFormat {
                    unit,
                    decimals: 6,
                    unit_indicators: ind,
                    ..LengthFormat::default()
                };
                for inches in [120.0, 150.5, 0.0, 9.0] {
                    let text = format_length(inches, &f);
                    let back = parse_length(&text, unit).unwrap_or_else(|| panic!("{text}"));
                    assert!(close(back, inches), "{unit:?} {text} -> {back} != {inches}");
                }
            }
        }
    }

    #[test]
    fn formats_each_unit() {
        let mm = LengthFormat::metric_mm();
        assert_eq!(format_length(120.0, &mm), "3048");
        let cm = LengthFormat {
            unit: LengthUnit::Centimeters,
            decimals: 1,
            ..mm
        };
        assert_eq!(format_length(120.0, &cm), "304.8");
        let m = LengthFormat {
            unit: LengthUnit::Meters,
            decimals: 3,
            unit_indicators: true,
            ..mm
        };
        assert_eq!(format_length(120.0, &m), "3.048 m");
        assert_eq!(
            format_length(
                120.0,
                &LengthFormat {
                    unit_indicators: true,
                    ..mm
                }
            ),
            "3048 mm"
        );
        let df = LengthFormat {
            unit: LengthUnit::DecimalFeet,
            ..LengthFormat::default()
        };
        assert_eq!(format_length(123.0, &df), "10.25'");
        assert_eq!(format_length(120.0, &df), "10'");
        let trailing = LengthFormat {
            trailing_zeroes: true,
            ..df
        };
        assert_eq!(format_length(120.0, &trailing), "10.00'");
        let inches = LengthFormat {
            unit: LengthUnit::Inches,
            ..LengthFormat::default()
        };
        assert_eq!(format_length(150.5, &inches), "150 1/2\"");
        assert_eq!(format_length(0.5, &inches), "1/2\"");
    }

    #[test]
    fn imperial_rounding_eighths_vs_sixteenths() {
        let sixteenths = LengthFormat::default();
        let eighths = LengthFormat::imperial_quarter_scale();
        // 150.07" is 1/16" past 150" after rounding.
        assert_eq!(format_length(150.07, &sixteenths), "12'-6 1/16\"");
        assert_eq!(format_length(150.07, &eighths), "12'-6 1/8\"");
        assert_eq!(format_length(150.02, &eighths), "12'-6\"");
        let bare = LengthFormat {
            unit_indicators: false,
            ..sixteenths
        };
        assert_eq!(format_length(150.5, &bare), "12-6 1/2");
    }

    #[test]
    fn metric_rounding_of_1234_5_mm() {
        let inches = 1234.5 / MM_PER_INCH;
        let zero = LengthFormat::metric_mm();
        assert_eq!(format_length(inches, &zero), "1235");
        let one = LengthFormat {
            decimals: 1,
            ..zero
        };
        assert_eq!(format_length(inches, &one), "1234.5");
        let one_zeroes = LengthFormat {
            decimals: 1,
            trailing_zeroes: true,
            ..zero
        };
        assert_eq!(format_length(1234.0 / MM_PER_INCH, &one_zeroes), "1234.0");
        assert_eq!(format_length(1234.0 / MM_PER_INCH, &one), "1234");
        assert_eq!(format_length(-0.001, &zero), "0");
        assert_eq!(format_length(-120.0, &zero), "-3048");
    }

    #[test]
    fn area_conversions() {
        assert_eq!(
            format_area(212.5 * 144.0, UnitSystem::Imperial),
            "212.5 sq ft"
        );
        assert_eq!(
            format_area(212.5 * 144.0, UnitSystem::Metric),
            "19.74 m\u{b2}"
        );
        assert_eq!(format_area(0.0, UnitSystem::Metric), "0.00 m\u{b2}");
        assert!(close(sq_in_to_sq_m(1550.0031), 1.0));
    }

    #[test]
    fn parses_unit_suffixes() {
        let ft = LengthUnit::FeetInches;
        assert!(close(parse_length("3048mm", ft).unwrap(), 120.0));
        assert!(close(parse_length("3048 mm", ft).unwrap(), 120.0));
        assert!(close(parse_length("3.048m", ft).unwrap(), 120.0));
        assert!(close(parse_length("3.048 M", ft).unwrap(), 120.0));
        assert!(close(parse_length("304.8cm", ft).unwrap(), 120.0));
        assert!(close(
            parse_length("10.25'", LengthUnit::Millimeters).unwrap(),
            123.0
        ));
        assert!(close(
            parse_length("10ft", LengthUnit::Meters).unwrap(),
            120.0
        ));
        assert!(close(parse_length("6in", LengthUnit::Meters).unwrap(), 6.0));
        assert!(close(parse_length("-3048mm", ft).unwrap(), -120.0));
        // Suffix overrides the default; bare numbers use it.
        assert!(close(
            parse_length("3048", LengthUnit::Millimeters).unwrap(),
            120.0
        ));
        assert!(close(
            parse_length("3.048", LengthUnit::Meters).unwrap(),
            120.0
        ));
        assert!(close(
            parse_length("304.8", LengthUnit::Centimeters).unwrap(),
            120.0
        ));
        assert!(close(
            parse_length("10.25", LengthUnit::DecimalFeet).unwrap(),
            123.0
        ));
        assert!(close(parse_length("36", ft).unwrap(), 36.0));
        // Existing feet-inches forms delegate to parse_ft_in.
        assert_eq!(
            parse_length("12'-6 1/2\"", LengthUnit::Millimeters),
            Some(150.5)
        );
        assert_eq!(parse_length("6\"", LengthUnit::Millimeters), Some(6.0));
        assert_eq!(parse_length("", ft), None);
        assert_eq!(parse_length("abc", ft), None);
        assert_eq!(parse_length("mm", ft), None);
        assert_eq!(parse_length("12abcmm", ft), None);
    }
}
