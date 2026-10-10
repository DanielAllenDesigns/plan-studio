//! Survey angles: quadrant and azimuth bearings, degrees/minutes/seconds, and
//! the Number Style (Round 16 brief 06, CAD-108, CAD-109, CAD-134).
//!
//! The plan keeps directions as degrees counter-clockwise from east (the
//! "math angle"). A bearing is read from north: an azimuth runs clockwise from
//! north through 360, and a quadrant bearing is written as a north or south
//! letter, an angle up to 90 and an east or west letter (`N 61 25 10 E`).
//! North is the plan's +Y direction ([`NORTH_DEG`]).

use crate::geometry::Point;
use crate::units::{LengthFormat, LengthUnit};
use serde::{Deserialize, Serialize};

/// The math angle of plan north.
pub const NORTH_DEG: f64 = 90.0;

/// How an angle is shown and typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AngleStyle {
    /// Decimal degrees counter-clockwise from east: `45.5°`.
    #[default]
    Degrees,
    /// Degrees, minutes and seconds counter-clockwise from east: `45°30'0"`.
    Dms,
    /// Quadrant bearing: `N 61°25'10" E`.
    Quadrant,
    /// Azimuth, clockwise from north: `121°25'10"`.
    Azimuth,
}

impl AngleStyle {
    pub const ALL: [AngleStyle; 4] = [
        AngleStyle::Degrees,
        AngleStyle::Dms,
        AngleStyle::Quadrant,
        AngleStyle::Azimuth,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AngleStyle::Degrees => "Degrees",
            AngleStyle::Dms => "Degrees, Minutes, Seconds",
            AngleStyle::Quadrant => "Quadrant Bearing",
            AngleStyle::Azimuth => "Azimuth Bearing",
        }
    }
}

/// An angle style with its precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AngleFormat {
    pub style: AngleStyle,
    /// Decimals of a degree (Degrees) or of a second (the others).
    pub decimals: u32,
}

impl Default for AngleFormat {
    fn default() -> Self {
        Self {
            style: AngleStyle::Degrees,
            decimals: 2,
        }
    }
}

/// Math angle (degrees counter-clockwise from east) to azimuth in `[0, 360)`.
pub fn math_to_azimuth(math: f64) -> f64 {
    (NORTH_DEG - math).rem_euclid(360.0)
}

/// Azimuth (degrees clockwise from north) to a math angle in `[0, 360)`.
pub fn azimuth_to_math(az: f64) -> f64 {
    (NORTH_DEG - az).rem_euclid(360.0)
}

/// The unit direction of an azimuth.
pub fn azimuth_dir(az: f64) -> Point {
    let m = azimuth_to_math(az).to_radians();
    Point::new(m.cos(), m.sin())
}

/// The azimuth of the direction `from` to `to`.
pub fn azimuth_of(from: Point, to: Point) -> f64 {
    math_to_azimuth(to.sub(from).angle().to_degrees())
}

/// Splits decimal degrees into whole degrees, whole minutes and seconds,
/// rounded to `sec_decimals` places with the carry applied.
fn split_dms(deg: f64, sec_decimals: u32) -> (i64, i64, f64) {
    let scale = 10f64.powi(sec_decimals.min(6) as i32);
    let total = (deg.abs() * 3600.0 * scale).round() / scale;
    let d = (total / 3600.0).floor();
    let rest = total - d * 3600.0;
    let m = (rest / 60.0).floor();
    let s = rest - m * 60.0;
    (d as i64, m as i64, s)
}

fn fmt_seconds(s: f64, decimals: u32) -> String {
    let t = format!("{:.*}", decimals as usize, s);
    if decimals > 0 {
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        t
    }
}

fn fmt_dms(deg: f64, decimals: u32) -> String {
    let (d, m, s) = split_dms(deg, decimals);
    let neg = if deg < 0.0 && (d, m) != (0, 0) || deg < 0.0 && s > 0.0 {
        "-"
    } else {
        ""
    };
    format!("{neg}{d}\u{b0}{m}'{}\"", fmt_seconds(s, decimals))
}

/// Quadrant bearing text of an azimuth: `N 61°25'10" E`; the four cardinal
/// directions are the bare letters `N`, `E`, `S`, `W`.
pub fn format_quadrant(az: f64, sec_decimals: u32) -> String {
    let az = az.rem_euclid(360.0);
    let cardinal = |t: f64| (az - t).abs() < 1e-9;
    if cardinal(0.0) {
        return "N".into();
    }
    if cardinal(90.0) {
        return "E".into();
    }
    if cardinal(180.0) {
        return "S".into();
    }
    if cardinal(270.0) {
        return "W".into();
    }
    let (ns, ew, a) = if az < 90.0 {
        ('N', 'E', az)
    } else if az < 180.0 {
        ('S', 'E', 180.0 - az)
    } else if az < 270.0 {
        ('S', 'W', az - 180.0)
    } else {
        ('N', 'W', 360.0 - az)
    };
    format!("{ns} {} {ew}", fmt_dms(a, sec_decimals))
}

/// Text of a math angle in the given style.
pub fn format_angle(math: f64, f: &AngleFormat) -> String {
    match f.style {
        AngleStyle::Degrees => {
            let s = format!("{:.*}", f.decimals as usize, math);
            let s = if f.decimals > 0 {
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            } else {
                s
            };
            let s = if s == "-0" { "0".into() } else { s };
            format!("{s}\u{b0}")
        }
        AngleStyle::Dms => fmt_dms(math.rem_euclid(360.0), f.decimals),
        AngleStyle::Quadrant => format_quadrant(math_to_azimuth(math), f.decimals),
        AngleStyle::Azimuth => fmt_dms(math_to_azimuth(math), f.decimals),
    }
}

/// The numbers in `s` (degrees, minutes, seconds), ignoring the marks
/// `° ' "`, commas and spaces between them; `None` if anything else is there.
fn dms_numbers(s: &str) -> Option<Vec<f64>> {
    let cleaned: String = s
        .chars()
        .map(|c| match c {
            '\u{b0}' | '\'' | '"' | '\u{2032}' | '\u{2033}' | ',' | '\u{ba}' => ' ',
            c => c,
        })
        .collect();
    let mut out = Vec::new();
    for tok in cleaned.split_whitespace() {
        let v: f64 = tok.parse().ok()?;
        if !v.is_finite() {
            return None;
        }
        out.push(v);
    }
    if out.is_empty() || out.len() > 3 {
        return None;
    }
    Some(out)
}

/// Degrees from up to three numbers: degrees, minutes, seconds. Minutes and
/// seconds must be below 60 and not negative.
fn dms_degrees(nums: &[f64]) -> Option<f64> {
    let d = nums[0];
    let m = nums.get(1).copied().unwrap_or(0.0);
    let s = nums.get(2).copied().unwrap_or(0.0);
    if !(0.0..60.0).contains(&m) || !(0.0..60.0).contains(&s) {
        return None;
    }
    if (m != 0.0 || s != 0.0) && d.fract() != 0.0 {
        return None;
    }
    let mag = d.abs() + m / 60.0 + s / 3600.0;
    Some(if d < 0.0 || d.to_string().starts_with('-') {
        -mag
    } else {
        mag
    })
}

/// Reads a quadrant bearing (`N 45 E`, `S 12 30 W`, `n45e`, `N`, `W`) as an
/// azimuth.
pub fn parse_quadrant(s: &str) -> Option<f64> {
    let t = s.trim().to_ascii_uppercase();
    let first = t.chars().next()?;
    let last = t.chars().last()?;
    match (t.len(), first) {
        (1, 'N') => return Some(0.0),
        (1, 'E') => return Some(90.0),
        (1, 'S') => return Some(180.0),
        (1, 'W') => return Some(270.0),
        _ => {}
    }
    if !matches!(first, 'N' | 'S') || !matches!(last, 'E' | 'W') || t.len() < 3 {
        return None;
    }
    let body = &t[1..t.len() - 1];
    let nums = dms_numbers(body)?;
    let a = dms_degrees(&nums)?;
    if !(0.0..=90.0).contains(&a) {
        return None;
    }
    Some(match (first, last) {
        ('N', 'E') => a,
        ('S', 'E') => 180.0 - a,
        ('S', 'W') => 180.0 + a,
        _ => (360.0 - a) % 360.0,
    })
}

/// Reads an angle typed in any way Chief accepts and returns a math angle
/// (degrees counter-clockwise from east). A quadrant bearing is recognised by
/// its letters and an azimuth by an `Az` prefix; a bare number or DMS is read
/// in `style` (an azimuth under [`AngleStyle::Azimuth`], otherwise counter-
/// clockwise from east).
pub fn parse_angle(s: &str, style: AngleStyle) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if t.chars().any(|c| c.is_ascii_alphabetic()) {
        let lower = t.to_ascii_lowercase();
        if let Some(rest) = lower
            .strip_prefix("azimuth")
            .or_else(|| lower.strip_prefix("az"))
        {
            let a = dms_degrees(&dms_numbers(rest.trim_start_matches([' ', ':']))?)?;
            return Some(azimuth_to_math(a));
        }
        return parse_quadrant(t).map(azimuth_to_math);
    }
    let a = dms_degrees(&dms_numbers(t)?)?;
    Some(match style {
        AngleStyle::Azimuth => azimuth_to_math(a),
        _ => a,
    })
}

// ----- traverse -----

/// One course of a traverse: an azimuth and a distance in inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Course {
    pub azimuth: f64,
    pub distance: f64,
}

/// The corners a traverse visits, starting at `start` (`courses.len() + 1`
/// points).
pub fn traverse_points(start: Point, courses: &[Course]) -> Vec<Point> {
    let mut out = vec![start];
    let mut at = start;
    for c in courses {
        at = at.add(azimuth_dir(c.azimuth).scale(c.distance));
        out.push(at);
    }
    out
}

/// How far the last corner of a traverse is from its start (inches).
pub fn closure_error(start: Point, courses: &[Course]) -> f64 {
    traverse_points(start, courses)
        .last()
        .map_or(0.0, |p| p.dist(start))
}

/// The course from `a` to `b`.
pub fn course_between(a: Point, b: Point) -> Course {
    Course {
        azimuth: azimuth_of(a, b),
        distance: a.dist(b),
    }
}

// ----- Number Style (PR-30, PR-31) -----

/// How a roof pitch is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PitchStyle {
    /// Rise in 12: `6 in 12`.
    #[default]
    RiseOver12,
    Degrees,
}

/// What a custom unit measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quantity {
    /// Base unit: inches.
    Length,
    /// Base unit: square inches.
    Area,
    /// Base unit: cubic inches.
    Volume,
}

/// A custom unit: `value_in_unit = base * multiplier`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomUnit {
    pub name: String,
    pub quantity: Quantity,
    pub multiplier: f64,
}

impl CustomUnit {
    pub fn from_base(&self, base: f64) -> f64 {
        base * self.multiplier
    }

    pub fn to_base(&self, v: f64) -> Option<f64> {
        (self.multiplier != 0.0).then(|| v / self.multiplier)
    }

    pub fn format(&self, base: f64, decimals: u32) -> String {
        format!(
            "{:.*} {}",
            decimals as usize,
            self.from_base(base),
            self.name
        )
    }
}

/// The Number Style and Angle Style settings together.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NumberStyle {
    pub length: LengthFormat,
    pub angle: AngleFormat,
    pub pitch: PitchStyle,
    pub custom_units: Vec<CustomUnit>,
}

impl NumberStyle {
    /// Is this the factory style (nothing to save with the plan)?
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Survey style: decimal feet and quadrant bearings.
    pub fn survey() -> Self {
        Self {
            length: LengthFormat {
                unit: LengthUnit::DecimalFeet,
                decimals: 2,
                ..LengthFormat::default()
            },
            angle: AngleFormat {
                style: AngleStyle::Quadrant,
                decimals: 0,
            },
            ..Self::default()
        }
    }

    pub fn format_length(&self, inches: f64) -> String {
        crate::units::format_length(inches, &self.length)
    }

    pub fn format_angle(&self, math: f64) -> String {
        format_angle(math, &self.angle)
    }

    pub fn parse_angle(&self, s: &str) -> Option<f64> {
        parse_angle(s, self.angle.style)
    }

    pub fn parse_length(&self, s: &str) -> Option<f64> {
        crate::units::parse_length(s, self.length.unit)
    }

    /// A pitch given as an angle in degrees.
    pub fn format_pitch(&self, deg: f64) -> String {
        match self.pitch {
            PitchStyle::RiseOver12 => {
                let rise = deg.to_radians().tan() * 12.0;
                let s = format!("{rise:.2}");
                let s = s.trim_end_matches('0').trim_end_matches('.');
                format!("{s} in 12")
            }
            PitchStyle::Degrees => format!("{deg:.1}\u{b0}"),
        }
    }

    /// Reads a pitch typed as `6 in 12`, `6/12`, `6:12` or degrees (`26.57°`).
    pub fn parse_pitch(&self, s: &str) -> Option<f64> {
        let t = s.trim().to_ascii_lowercase();
        for sep in [" in ", "/", ":"] {
            if let Some((r, run)) = t.split_once(sep) {
                let rise: f64 = r.trim().parse().ok()?;
                let run: f64 = run.trim().parse().ok()?;
                return (run > 0.0).then(|| (rise / run).atan().to_degrees());
            }
        }
        let n = t.trim_end_matches('\u{b0}').trim();
        n.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn quadrant_round_trips_in_all_four_quadrants() {
        for az in [10.0, 61.419_444, 100.5, 179.0, 200.25, 269.0, 280.0, 359.0] {
            let text = format_quadrant(az, 3);
            let back = parse_quadrant(&text).expect(&text);
            assert!((back - az).abs() < 1e-3, "{az} -> {text} -> {back}");
        }
        assert_eq!(
            format_quadrant(61.0 + 25.0 / 60.0 + 10.0 / 3600.0, 0),
            "N 61\u{b0}25'10\" E"
        );
        assert_eq!(format_quadrant(180.0 - 45.0, 0), "S 45\u{b0}0'0\" E");
        assert_eq!(format_quadrant(225.0, 0), "S 45\u{b0}0'0\" W");
        assert_eq!(format_quadrant(315.0, 0), "N 45\u{b0}0'0\" W");
    }

    #[test]
    fn typed_quadrant_forms() {
        assert!(close(
            parse_quadrant("N 61 25 10 E").unwrap(),
            61.0 + 25.0 / 60.0 + 10.0 / 3600.0
        ));
        assert!(close(parse_quadrant("S 45 W").unwrap(), 225.0));
        assert!(close(parse_quadrant("s45e").unwrap(), 135.0));
        assert!(close(parse_quadrant("N 30.5 W").unwrap(), 329.5));
        assert!(close(parse_quadrant("N 90 E").unwrap(), 90.0));
        assert_eq!(parse_quadrant("N").unwrap(), 0.0);
        assert_eq!(parse_quadrant("W").unwrap(), 270.0);
    }

    #[test]
    fn cardinal_bearings_are_letters() {
        for (az, t) in [(0.0, "N"), (90.0, "E"), (180.0, "S"), (270.0, "W")] {
            assert_eq!(format_quadrant(az, 0), t);
            assert_eq!(parse_quadrant(t), Some(az));
        }
    }

    #[test]
    fn invalid_bearings_are_refused() {
        for bad in [
            "",
            "N 95 E",
            "N 45",
            "45 E",
            "X 45 E",
            "N 45 61 E",
            "N 45 70 E",
            "N 45 30 90 E",
            "N -5 E",
            "N a E",
            "N 10 20 30 40 E",
            "NE",
            "azimuth",
            "Az",
            "12abc",
        ] {
            assert_eq!(parse_angle(bad, AngleStyle::Degrees), None, "{bad:?}");
        }
    }

    #[test]
    fn azimuth_and_math_angles() {
        assert!(close(math_to_azimuth(90.0), 0.0));
        assert!(close(math_to_azimuth(0.0), 90.0));
        assert!(close(math_to_azimuth(-90.0), 180.0));
        assert!(close(azimuth_to_math(270.0), 180.0));
        for az in [0.0, 33.3, 90.0, 215.0, 359.9] {
            assert!(close(math_to_azimuth(azimuth_to_math(az)), az));
        }
        // Prefixed azimuth, or a bare angle under the Azimuth style.
        assert!(close(
            parse_angle("Az 180", AngleStyle::Degrees).unwrap(),
            270.0
        ));
        assert!(close(
            parse_angle("azimuth 90 30", AngleStyle::Degrees).unwrap(),
            azimuth_to_math(90.5)
        ));
        assert!(close(parse_angle("90", AngleStyle::Azimuth).unwrap(), 0.0));
        assert!(close(parse_angle("90", AngleStyle::Degrees).unwrap(), 90.0));
    }

    #[test]
    fn dms_with_seconds() {
        let a = parse_angle("45\u{b0}30'15\"", AngleStyle::Degrees).unwrap();
        assert!(close(a, 45.0 + 30.0 / 60.0 + 15.0 / 3600.0));
        let f = AngleFormat {
            style: AngleStyle::Dms,
            decimals: 0,
        };
        assert_eq!(format_angle(a, &f), "45\u{b0}30'15\"");
        assert_eq!(parse_angle("45 30 15", AngleStyle::Dms), Some(a));
        // Rounding carries: 59.9999 seconds is the next minute.
        assert_eq!(
            format_angle(10.0 + 59.0 / 60.0 + 59.9999 / 3600.0, &f),
            "11\u{b0}0'0\""
        );
        let f2 = AngleFormat {
            style: AngleStyle::Dms,
            decimals: 1,
        };
        assert_eq!(format_angle(0.5, &f2), "0\u{b0}30'0\"");
        assert!(parse_angle("45 60 0", AngleStyle::Dms).is_none());
        assert!(close(
            parse_angle("-30", AngleStyle::Degrees).unwrap(),
            -30.0
        ));
    }

    #[test]
    fn styles_format_the_same_direction() {
        let math = azimuth_to_math(121.0 + 25.0 / 60.0 + 10.0 / 3600.0);
        let mut f = AngleFormat {
            style: AngleStyle::Quadrant,
            decimals: 0,
        };
        assert_eq!(format_angle(math, &f), "S 58\u{b0}34'50\" E");
        f.style = AngleStyle::Azimuth;
        assert_eq!(format_angle(math, &f), "121\u{b0}25'10\"");
        f.style = AngleStyle::Degrees;
        f.decimals = 1;
        assert_eq!(format_angle(45.0, &f), "45\u{b0}");
        assert_eq!(format_angle(-0.01, &f), "0\u{b0}");
        // Every style parses its own output.
        for style in AngleStyle::ALL {
            let f = AngleFormat { style, decimals: 2 };
            for m in [0.0, 12.5, 90.0, 133.25, 270.0, 301.0] {
                let back = parse_angle(&format_angle(m, &f), style).unwrap();
                assert!(
                    (back.rem_euclid(360.0) - m).abs() < 1e-3,
                    "{style:?} {m} -> {back}"
                );
            }
        }
    }

    #[test]
    fn traverse_of_six_courses_closes() {
        // A six-sided lot whose last course is computed to close it.
        let ft = 12.0;
        let mut courses = vec![
            Course {
                azimuth: parse_quadrant("N 0 E").unwrap(),
                distance: 100.0 * ft,
            },
            Course {
                azimuth: parse_quadrant("N 61 25 10 E").unwrap(),
                distance: 80.0 * ft,
            },
            Course {
                azimuth: parse_quadrant("S 45 E").unwrap(),
                distance: 60.0 * ft,
            },
            Course {
                azimuth: parse_quadrant("S 10 W").unwrap(),
                distance: 90.0 * ft,
            },
            Course {
                azimuth: parse_quadrant("S 80 W").unwrap(),
                distance: 70.0 * ft,
            },
        ];
        let start = Point::new(0.0, 0.0);
        let pts = traverse_points(start, &courses);
        courses.push(course_between(*pts.last().unwrap(), start));
        assert_eq!(courses.len(), 6);
        assert!(closure_error(start, &courses) < 1e-3);
        // An unclosed lot reports its miss.
        courses[5].distance += 1.0;
        assert!(close(closure_error(start, &courses), 1.0));
    }

    #[test]
    fn course_between_reads_back_the_bearing() {
        let c = course_between(Point::new(0.0, 0.0), Point::new(100.0, 100.0));
        assert!(close(c.azimuth, 45.0));
        assert!(close(c.distance, 100.0 * 2f64.sqrt()));
        let p = traverse_points(Point::ZERO, &[c]);
        assert!(p[1].dist(Point::new(100.0, 100.0)) < 1e-9);
    }

    #[test]
    fn number_style_formats_and_parses() {
        let s = NumberStyle::survey();
        assert_eq!(s.format_length(125.0 * 12.0 + 6.0), "125.5'");
        assert_eq!(s.format_angle(azimuth_to_math(45.0)), "N 45\u{b0}0'0\" E");
        assert_eq!(s.parse_angle("N 45 E"), Some(45.0));
        let d = NumberStyle::default();
        assert_eq!(d.format_length(150.5), "12'-6 1/2\"");
        assert_eq!(d.format_pitch(26.565_051_177_077_99), "6 in 12");
        assert!(close(
            d.parse_pitch("6 in 12").unwrap(),
            26.565_051_177_077_99
        ));
        assert!(close(d.parse_pitch("6/12").unwrap(), 26.565_051_177_077_99));
        assert!(close(d.parse_pitch("30\u{b0}").unwrap(), 30.0));
        let g = NumberStyle {
            pitch: PitchStyle::Degrees,
            ..NumberStyle::default()
        };
        assert_eq!(g.format_pitch(30.0), "30.0\u{b0}");
    }

    #[test]
    fn custom_unit_conversions() {
        // Chains: 1 chain is 66 ft, so 792 inches per chain.
        let chain = CustomUnit {
            name: "chain".into(),
            quantity: Quantity::Length,
            multiplier: 1.0 / 792.0,
        };
        assert!(close(chain.from_base(1584.0), 2.0));
        assert!(close(chain.to_base(2.0).unwrap(), 1584.0));
        assert_eq!(chain.format(1584.0, 2), "2.00 chain");
        let zero = CustomUnit {
            name: "x".into(),
            quantity: Quantity::Area,
            multiplier: 0.0,
        };
        assert_eq!(zero.to_base(1.0), None);
    }
}
