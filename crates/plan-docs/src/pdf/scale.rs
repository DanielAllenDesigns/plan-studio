//! Drawing scales and standard sheet sizes.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

/// Standard sheet sizes, always reported landscape (width >= height).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SheetSize {
    /// Architectural D, 24 x 36 in.
    ArchD,
    /// Architectural C, 18 x 24 in.
    ArchC,
    /// US Letter, 8.5 x 11 in.
    Letter,
    /// Tabloid / ledger, 11 x 17 in.
    Tabloid,
    /// Architectural A, 9 x 12 in.
    ArchA,
    /// Architectural B, 12 x 18 in.
    ArchB,
    /// Architectural E, 36 x 48 in.
    ArchE,
    /// Architectural E1, 30 x 42 in.
    ArchE1,
    /// ANSI A, 8.5 x 11 in.
    AnsiA,
    /// ANSI B, 11 x 17 in.
    AnsiB,
    /// ANSI C, 17 x 22 in.
    AnsiC,
    /// ANSI D, 22 x 34 in.
    AnsiD,
    /// ANSI E, 34 x 44 in.
    AnsiE,
    /// ISO A0, 841 x 1189 mm.
    IsoA0,
    /// ISO A1, 594 x 841 mm.
    IsoA1,
    /// ISO A2, 420 x 594 mm.
    IsoA2,
    /// ISO A3, 297 x 420 mm.
    IsoA3,
    /// ISO A4, 210 x 297 mm.
    IsoA4,
}

const MM_PER_IN: f64 = 25.4;

impl SheetSize {
    /// Every sheet size.
    pub const ALL: [SheetSize; 18] = [
        SheetSize::ArchD,
        SheetSize::ArchC,
        SheetSize::Letter,
        SheetSize::Tabloid,
        SheetSize::ArchA,
        SheetSize::ArchB,
        SheetSize::ArchE,
        SheetSize::ArchE1,
        SheetSize::AnsiA,
        SheetSize::AnsiB,
        SheetSize::AnsiC,
        SheetSize::AnsiD,
        SheetSize::AnsiE,
        SheetSize::IsoA0,
        SheetSize::IsoA1,
        SheetSize::IsoA2,
        SheetSize::IsoA3,
        SheetSize::IsoA4,
    ];

    /// Landscape `(width, height)` in inches.
    pub fn inches(self) -> (f64, f64) {
        let mm = |w: f64, h: f64| (w / MM_PER_IN, h / MM_PER_IN);
        match self {
            SheetSize::ArchD => (36.0, 24.0),
            SheetSize::ArchC => (24.0, 18.0),
            SheetSize::Letter => (11.0, 8.5),
            SheetSize::Tabloid => (17.0, 11.0),
            SheetSize::ArchA => (12.0, 9.0),
            SheetSize::ArchB => (18.0, 12.0),
            SheetSize::ArchE => (48.0, 36.0),
            SheetSize::ArchE1 => (42.0, 30.0),
            SheetSize::AnsiA => (11.0, 8.5),
            SheetSize::AnsiB => (17.0, 11.0),
            SheetSize::AnsiC => (22.0, 17.0),
            SheetSize::AnsiD => (34.0, 22.0),
            SheetSize::AnsiE => (44.0, 34.0),
            SheetSize::IsoA0 => mm(1189.0, 841.0),
            SheetSize::IsoA1 => mm(841.0, 594.0),
            SheetSize::IsoA2 => mm(594.0, 420.0),
            SheetSize::IsoA3 => mm(420.0, 297.0),
            SheetSize::IsoA4 => mm(297.0, 210.0),
        }
    }

    /// Landscape `(width, height)` in PDF points.
    pub fn points(self) -> (f64, f64) {
        let (w, h) = self.inches();
        (w * 72.0, h * 72.0)
    }

    /// Human-readable name, e.g. `ARCH D (24 x 36)`.
    pub fn label(self) -> &'static str {
        match self {
            SheetSize::ArchD => "ARCH D (24 x 36)",
            SheetSize::ArchC => "ARCH C (18 x 24)",
            SheetSize::Letter => "Letter (8.5 x 11)",
            SheetSize::Tabloid => "Tabloid (11 x 17)",
            SheetSize::ArchA => "ARCH A (9 x 12)",
            SheetSize::ArchB => "ARCH B (12 x 18)",
            SheetSize::ArchE => "ARCH E (36 x 48)",
            SheetSize::ArchE1 => "ARCH E1 (30 x 42)",
            SheetSize::AnsiA => "ANSI A (8.5 x 11)",
            SheetSize::AnsiB => "ANSI B (11 x 17)",
            SheetSize::AnsiC => "ANSI C (17 x 22)",
            SheetSize::AnsiD => "ANSI D (22 x 34)",
            SheetSize::AnsiE => "ANSI E (34 x 44)",
            SheetSize::IsoA0 => "ISO A0",
            SheetSize::IsoA1 => "ISO A1",
            SheetSize::IsoA2 => "ISO A2",
            SheetSize::IsoA3 => "ISO A3",
            SheetSize::IsoA4 => "ISO A4",
        }
    }
}

/// Drawing scales: architectural (paper inches per plan foot) and metric
/// ratios (`Ratio(50)` is 1:50).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scale {
    /// 1/2" = 1'-0"
    HalfInch,
    /// 1/4" = 1'-0"
    QuarterInch,
    /// 3/16" = 1'-0"
    ThreeSixteenths,
    /// 1/8" = 1'-0"
    EighthInch,
    /// 1" = 10'-0"
    OneInchEq10Ft,
    /// 1" = 20'-0"
    OneInchEq20Ft,
    /// 3/4" = 1'-0"
    ThreeQuarterInch,
    /// 1" = 1'-0"
    OneInch,
    /// 1-1/2" = 1'-0"
    OneAndHalfInch,
    /// 3" = 1'-0"
    ThreeInch,
    /// 1" = 30'-0" (site plans; listed by [`Scale::SITE`])
    OneInchEq30Ft,
    /// 1" = 40'-0"
    OneInchEq40Ft,
    /// 1" = 50'-0"
    OneInchEq50Ft,
    /// 1" = 60'-0"
    OneInchEq60Ft,
    /// 1" = 100'-0"
    OneInchEq100Ft,
    /// Metric ratio 1:n (`Ratio(0)` is treated as 1:1).
    Ratio(u32),
}

/// Ratios offered when stepping a metric scale up or down.
const STANDARD_RATIOS: [u32; 13] = [1, 2, 5, 10, 20, 25, 50, 75, 100, 200, 250, 500, 1000];

impl Scale {
    /// The four scales plan sheets step down through when a plan does not
    /// fit, largest drawing first. [`Scale::smaller`] stops at 1/8".
    pub const DESCENDING: [Scale; 4] = [
        Scale::HalfInch,
        Scale::QuarterInch,
        Scale::ThreeSixteenths,
        Scale::EighthInch,
    ];

    /// Every architectural (non-ratio) scale, largest drawing first.
    pub const ALL: [Scale; 10] = [
        Scale::ThreeInch,
        Scale::OneAndHalfInch,
        Scale::OneInch,
        Scale::ThreeQuarterInch,
        Scale::HalfInch,
        Scale::QuarterInch,
        Scale::ThreeSixteenths,
        Scale::EighthInch,
        Scale::OneInchEq10Ft,
        Scale::OneInchEq20Ft,
    ];

    /// The site-plan scales below 1" = 20' that Send to Layout and a layout
    /// box offer too (kept out of [`Scale::ALL`], which sheets step through).
    pub const SITE: [Scale; 5] = [
        Scale::OneInchEq30Ft,
        Scale::OneInchEq40Ft,
        Scale::OneInchEq50Ft,
        Scale::OneInchEq60Ft,
        Scale::OneInchEq100Ft,
    ];

    /// The metric ratios the scale lists offer, largest drawing first.
    pub const METRIC: [Scale; 10] = [
        Scale::Ratio(10),
        Scale::Ratio(20),
        Scale::Ratio(25),
        Scale::Ratio(50),
        Scale::Ratio(75),
        Scale::Ratio(100),
        Scale::Ratio(200),
        Scale::Ratio(250),
        Scale::Ratio(500),
        Scale::Ratio(1000),
    ];

    /// Every scale a drop-down offers: [`Scale::ALL`], then [`Scale::SITE`],
    /// then [`Scale::METRIC`].
    pub fn choices() -> Vec<Scale> {
        Scale::ALL
            .into_iter()
            .chain(Scale::SITE)
            .chain(Scale::METRIC)
            .collect()
    }

    /// Paper inches per plan foot.
    pub fn inches_per_foot(self) -> f64 {
        match self {
            Scale::OneInchEq30Ft => 1.0 / 30.0,
            Scale::OneInchEq40Ft => 1.0 / 40.0,
            Scale::OneInchEq50Ft => 1.0 / 50.0,
            Scale::OneInchEq60Ft => 1.0 / 60.0,
            Scale::OneInchEq100Ft => 1.0 / 100.0,
            Scale::HalfInch => 0.5,
            Scale::QuarterInch => 0.25,
            Scale::ThreeSixteenths => 0.1875,
            Scale::EighthInch => 0.125,
            Scale::OneInchEq10Ft => 0.1,
            Scale::OneInchEq20Ft => 0.05,
            Scale::ThreeQuarterInch => 0.75,
            Scale::OneInch => 1.0,
            Scale::OneAndHalfInch => 1.5,
            Scale::ThreeInch => 3.0,
            Scale::Ratio(n) => 12.0 / f64::from(n.max(1)),
        }
    }

    /// Points of paper per inch of plan: `72 * scale_in_per_ft / 12`.
    pub fn points_per_inch(self) -> f64 {
        72.0 * self.inches_per_foot() / 12.0
    }

    /// Scale note text, e.g. `1/4" = 1'-0"` or `1:50`.
    ///
    /// Ratio labels are interned (one small allocation per distinct ratio,
    /// kept for the life of the process) so the result is always `'static`.
    pub fn label(self) -> &'static str {
        match self {
            Scale::HalfInch => "1/2\" = 1'-0\"",
            Scale::QuarterInch => "1/4\" = 1'-0\"",
            Scale::ThreeSixteenths => "3/16\" = 1'-0\"",
            Scale::EighthInch => "1/8\" = 1'-0\"",
            Scale::OneInchEq10Ft => "1\" = 10'-0\"",
            Scale::OneInchEq20Ft => "1\" = 20'-0\"",
            Scale::ThreeQuarterInch => "3/4\" = 1'-0\"",
            Scale::OneInch => "1\" = 1'-0\"",
            Scale::OneAndHalfInch => "1-1/2\" = 1'-0\"",
            Scale::ThreeInch => "3\" = 1'-0\"",
            Scale::OneInchEq30Ft => "1\" = 30'-0\"",
            Scale::OneInchEq40Ft => "1\" = 40'-0\"",
            Scale::OneInchEq50Ft => "1\" = 50'-0\"",
            Scale::OneInchEq60Ft => "1\" = 60'-0\"",
            Scale::OneInchEq100Ft => "1\" = 100'-0\"",
            Scale::Ratio(n) => ratio_label(n.max(1)),
        }
    }

    /// The next smaller drawing scale, if any.
    ///
    /// Architectural scales follow [`Scale::ALL`], except that 1/8" has no
    /// smaller scale: plan sheets stop stepping down there (use
    /// [`Scale::smaller_any`] to continue to 1" = 10'). Ratios step to the
    /// next larger standard ratio (1:50 to 1:75).
    pub fn smaller(self) -> Option<Scale> {
        match self {
            Scale::EighthInch => None,
            _ => self.smaller_any(),
        }
    }

    /// Like [`Scale::smaller`] but 1/8" continues to 1" = 10'.
    pub fn smaller_any(self) -> Option<Scale> {
        match self {
            Scale::Ratio(n) => STANDARD_RATIOS
                .iter()
                .copied()
                .find(|&r| r > n)
                .map(Scale::Ratio),
            _ => {
                let i = Scale::ALL.iter().position(|s| *s == self)?;
                Scale::ALL.get(i + 1).copied()
            }
        }
    }

    /// The next larger drawing scale, if any (1:50 to 1:20, 1/4" to 3/8"...).
    pub fn larger(self) -> Option<Scale> {
        match self {
            Scale::Ratio(n) => STANDARD_RATIOS
                .iter()
                .copied()
                .rev()
                .find(|&r| r < n)
                .map(Scale::Ratio),
            _ => {
                let i = Scale::ALL.iter().position(|s| *s == self)?;
                i.checked_sub(1).map(|j| Scale::ALL[j])
            }
        }
    }

    /// The scale that draws `ipf` paper inches per plan foot (what the
    /// two-part Drawing Scale of a view comes to): the architectural scale
    /// with that value, else the metric ratio 1:n nearest to it.
    pub fn from_inches_per_foot(ipf: f64) -> Scale {
        if let Some(s) = Scale::ALL
            .into_iter()
            .chain(Scale::SITE)
            .find(|s| (s.inches_per_foot() - ipf).abs() < 1e-9)
        {
            return s;
        }
        Scale::Ratio((12.0 / ipf.max(1e-6)).round().clamp(1.0, 100_000.0) as u32)
    }

    /// Parse a scale note such as `1/4" = 1'`, `1/4"=1'-0"`, `3/16 in = 1 ft`,
    /// `1" = 10'` or `1:50`. Returns `None` for text that is not one of the
    /// supported scales.
    pub fn from_label(label: &str) -> Option<Scale> {
        let t = label.trim();
        if let Some(n) = parse_ratio(t) {
            return Some(Scale::Ratio(n));
        }
        let (lhs, rhs) = t.split_once('=')?;
        let paper_in = parse_inches(lhs)?;
        let plan_ft = parse_feet(rhs)?;
        if plan_ft <= 0.0 {
            return None;
        }
        let ipf = paper_in / plan_ft;
        Scale::ALL
            .into_iter()
            .chain(Scale::SITE)
            .find(|s| (s.inches_per_foot() - ipf).abs() < 1e-9)
    }
}

/// Interned `1:n` label.
fn ratio_label(n: u32) -> &'static str {
    static CACHE: Mutex<BTreeMap<u32, &'static str>> = Mutex::new(BTreeMap::new());
    let mut map = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    map.entry(n)
        .or_insert_with(|| Box::leak(format!("1:{n}").into_boxed_str()))
}

/// `1:50` or `1 : 50` to `50`.
fn parse_ratio(s: &str) -> Option<u32> {
    let (l, r) = s.split_once(':')?;
    if l.trim() != "1" {
        return None;
    }
    r.trim().parse::<u32>().ok().filter(|n| *n > 0)
}

/// Paper inches from text such as `1/4"`, `0.5 in`, `1-1/2"` or `1 1/2"`.
fn parse_inches(s: &str) -> Option<f64> {
    let s = s
        .trim()
        .trim_end_matches(['"', '\u{201d}'])
        .trim_end_matches("inches")
        .trim_end_matches("inch")
        .trim_end_matches("in")
        .trim();
    let frac = |t: &str| -> Option<f64> {
        match t.split_once('/') {
            Some((n, d)) => {
                let (n, d) = (n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?);
                (d != 0.0).then_some(n / d)
            }
            None => t.trim().parse().ok(),
        }
    };
    if s.contains('/') {
        if let Some((whole, f)) = s.rsplit_once(['-', ' ']) {
            if f.contains('/') {
                return Some(whole.trim().parse::<f64>().ok()? + frac(f)?);
            }
        }
    }
    frac(s)
}

/// Plan feet from text such as `1'-0"`, `10'`, `1 ft` or `20 feet`.
fn parse_feet(s: &str) -> Option<f64> {
    let s = s.trim();
    let end = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let value: f64 = s[..end].parse().ok()?;
    let unit = s[end..].trim_start();
    let is_feet = ["'", "\u{2019}", "\u{2032}", "ft", "feet", "foot"]
        .iter()
        .any(|u| unit.starts_with(u));
    is_feet.then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_math() {
        assert!((Scale::QuarterInch.points_per_inch() - 1.5).abs() < 1e-12);
        assert!((Scale::OneInchEq10Ft.points_per_inch() - 0.6).abs() < 1e-12);
        assert!((Scale::OneInch.points_per_inch() - 6.0).abs() < 1e-12);
        assert!((Scale::Ratio(50).points_per_inch() - 1.44).abs() < 1e-12);
        assert!((Scale::Ratio(100).inches_per_foot() - 0.12).abs() < 1e-12);
        assert_eq!(Scale::EighthInch.smaller(), None);
        assert_eq!(Scale::EighthInch.smaller_any(), Some(Scale::OneInchEq10Ft));
        assert_eq!(Scale::HalfInch.smaller(), Some(Scale::QuarterInch));
        assert_eq!(Scale::HalfInch.larger(), Some(Scale::ThreeQuarterInch));
        assert_eq!(Scale::ThreeInch.larger(), None);
        assert_eq!(Scale::OneInchEq20Ft.smaller(), None);
        assert_eq!(Scale::Ratio(50).smaller(), Some(Scale::Ratio(75)));
        assert_eq!(Scale::Ratio(50).larger(), Some(Scale::Ratio(25)));
        assert_eq!(Scale::Ratio(1).larger(), None);
        assert_eq!(Scale::Ratio(1000).smaller(), None);
    }

    #[test]
    fn site_and_metric_scales_are_listed_and_round_trip() {
        let all = Scale::choices();
        assert_eq!(all.len(), 10 + 5 + 10);
        for s in Scale::SITE {
            assert_eq!(Scale::from_label(s.label()), Some(s), "{}", s.label());
            assert_eq!(Scale::from_inches_per_foot(s.inches_per_foot()), s);
        }
        assert_eq!(Scale::OneInchEq30Ft.label(), "1\" = 30'-0\"");
        assert_eq!(Scale::from_label("1\" = 100'"), Some(Scale::OneInchEq100Ft));
        assert!(Scale::METRIC.contains(&Scale::Ratio(1000)));
        // The ones below 1" = 20' draw smaller than it, and keep their order.
        for w in Scale::SITE.windows(2) {
            assert!(w[0].inches_per_foot() > w[1].inches_per_foot());
        }
        assert!(Scale::OneInchEq30Ft.inches_per_foot() < Scale::OneInchEq20Ft.inches_per_foot());
        // Sheets still stop where they did.
        assert_eq!(Scale::OneInchEq20Ft.smaller_any(), None);
    }

    #[test]
    fn all_is_sorted_largest_first() {
        for w in Scale::ALL.windows(2) {
            assert!(w[0].inches_per_foot() > w[1].inches_per_foot());
        }
    }

    #[test]
    fn label_round_trips() {
        for s in Scale::ALL {
            assert_eq!(Scale::from_label(s.label()), Some(s), "{}", s.label());
        }
        for n in [20, 50, 100, 250] {
            let s = Scale::Ratio(n);
            assert_eq!(Scale::from_label(s.label()), Some(s));
        }
        assert_eq!(Scale::Ratio(50).label(), "1:50");
    }

    #[test]
    fn labels_parse_loosely() {
        assert_eq!(Scale::from_label("1/4\" = 1'"), Some(Scale::QuarterInch));
        assert_eq!(Scale::from_label("1/4\"=1'-0\""), Some(Scale::QuarterInch));
        assert_eq!(
            Scale::from_label("3/16 in = 1 ft"),
            Some(Scale::ThreeSixteenths)
        );
        assert_eq!(Scale::from_label("0.5\" = 1'"), Some(Scale::HalfInch));
        assert_eq!(Scale::from_label("1\" = 10'"), Some(Scale::OneInchEq10Ft));
        assert_eq!(Scale::from_label("1\" = 20 ft"), Some(Scale::OneInchEq20Ft));
        assert_eq!(Scale::from_label("1\" = 1'"), Some(Scale::OneInch));
        assert_eq!(
            Scale::from_label("1 1/2\" = 1'"),
            Some(Scale::OneAndHalfInch)
        );
        assert_eq!(Scale::from_label("3\"=1'-0\""), Some(Scale::ThreeInch));
        assert_eq!(Scale::from_label("1 : 100"), Some(Scale::Ratio(100)));
        assert_eq!(Scale::from_label("1\" = 7'"), None);
        assert_eq!(Scale::from_label("2:50"), None);
        assert_eq!(Scale::from_label("1:0"), None);
        assert_eq!(Scale::from_label("1/4"), None);
        assert_eq!(Scale::from_label("1/4\" = 1"), None);
    }

    #[test]
    fn a_view_scale_in_inches_per_foot_finds_its_scale() {
        assert_eq!(Scale::from_inches_per_foot(0.25), Scale::QuarterInch);
        assert_eq!(Scale::from_inches_per_foot(3.0), Scale::ThreeInch);
        assert_eq!(Scale::from_inches_per_foot(0.05), Scale::OneInchEq20Ft);
        // 1 mm = 50 mm draws 0.24 inch per foot: the ratio 1:50.
        assert_eq!(Scale::from_inches_per_foot(0.24), Scale::Ratio(50));
        assert_eq!(Scale::from_inches_per_foot(0.0), Scale::Ratio(100_000));
    }

    #[test]
    fn sheet_sizes() {
        assert_eq!(SheetSize::ArchD.points(), (2592.0, 1728.0));
        assert_eq!(SheetSize::ArchE.inches(), (48.0, 36.0));
        assert_eq!(SheetSize::AnsiA.inches(), SheetSize::Letter.inches());
        let (w, h) = SheetSize::IsoA3.inches();
        assert!((w * 25.4 - 420.0).abs() < 1e-9 && (h * 25.4 - 297.0).abs() < 1e-9);
        for s in SheetSize::ALL {
            let (w, h) = s.points();
            assert!(w > h, "{} is landscape", s.label());
        }
    }

    #[test]
    fn serde_round_trip() {
        let s = serde_json::to_string(&Scale::Ratio(50)).unwrap();
        assert_eq!(serde_json::from_str::<Scale>(&s).unwrap(), Scale::Ratio(50));
        assert_eq!(
            serde_json::to_string(&Scale::QuarterInch).unwrap(),
            "\"QuarterInch\""
        );
        let s = serde_json::to_string(&SheetSize::IsoA1).unwrap();
        assert_eq!(
            serde_json::from_str::<SheetSize>(&s).unwrap(),
            SheetSize::IsoA1
        );
    }
}
