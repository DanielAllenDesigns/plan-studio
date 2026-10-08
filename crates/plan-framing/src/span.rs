//! Span checks for joists, ceiling joists and rafters.
//!
//! Build Framing sizes nothing by itself: it frames what the defaults say.
//! This module answers "does that lumber carry that span?" with a plain beam
//! calculation (bending and live-load deflection of a simple span), so the
//! Build Framing dialog and the build report can warn about a 2x6 floor joist
//! over 14 feet. It is a planning aid, not a code check: the values are for
//! Douglas Fir-Larch No. 2 with the repetitive-member and size factors, and
//! ignore shear, bearing, notches and holes. Chief's own structural
//! calculators (CB-42) are a separate, larger feature.

use crate::defaults::FramingDefaults;
use crate::lumber::{
    format_inches, Lumber, TWO_BY_EIGHT, TWO_BY_FOUR, TWO_BY_SIX, TWO_BY_TEN, TWO_BY_TWELVE,
};
use crate::member::{Member, MemberKind};
use crate::roof::RoofFramingDefaults;

/// Reference bending stress of DF-L No. 2, psi.
const FB: f64 = 900.0;
/// Modulus of elasticity of DF-L No. 2, psi.
const E: f64 = 1_600_000.0;
/// Repetitive-member factor for members at 24" or less on centre.
const REPETITIVE: f64 = 1.15;

/// What a member carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpanUse {
    /// Floor joists: 40 psf live, 10 psf dead, L/360.
    Floor,
    /// Ceiling joists without storage: 10 psf live, 5 psf dead, L/240.
    Ceiling,
    /// Rafters: 20 psf live, 10 psf dead, L/180, on the horizontal run.
    Rafter,
}

impl SpanUse {
    pub fn name(self) -> &'static str {
        match self {
            SpanUse::Floor => "floor joist",
            SpanUse::Ceiling => "ceiling joist",
            SpanUse::Rafter => "rafter",
        }
    }

    /// (live psf, dead psf, deflection limit denominator).
    fn load(self) -> (f64, f64, f64) {
        match self {
            SpanUse::Floor => (40.0, 10.0, 360.0),
            SpanUse::Ceiling => (10.0, 5.0, 240.0),
            SpanUse::Rafter => (20.0, 10.0, 180.0),
        }
    }
}

/// Size factor Cf of dimension lumber by depth.
fn size_factor(depth: f64) -> f64 {
    match depth {
        d if d <= 3.5 => 1.5,
        d if d <= 5.5 => 1.3,
        d if d <= 7.25 => 1.2,
        d if d <= 9.25 => 1.1,
        _ => 1.0,
    }
}

/// The longest clear span, inches, of `lumber` at `spacing` inches on centre
/// carrying `use_`: the lesser of the bending limit and the deflection limit.
pub fn allowable_span(use_: SpanUse, lumber: Lumber, spacing: f64) -> f64 {
    let (live, dead, denom) = use_.load();
    let (b, d) = (lumber.thickness, lumber.depth);
    let section = b * d * d / 6.0;
    let inertia = b * d * d * d / 12.0;
    let strip = spacing.max(1.0) / 12.0 / 12.0;
    let w_total = (live + dead) * strip;
    let w_live = live * strip;
    let fb = FB * size_factor(d) * if spacing <= 24.0 { REPETITIVE } else { 1.0 };
    let bending = (8.0 * fb * section / w_total).sqrt();
    let deflection = (384.0 * E * inertia / (5.0 * w_live * denom)).cbrt();
    bending.min(deflection)
}

/// The smallest 2x lumber from 2x4 to 2x12 whose allowable span reaches
/// `span` at `spacing`; `None` when even a 2x12 falls short.
pub fn smallest_for(use_: SpanUse, span: f64, spacing: f64) -> Option<Lumber> {
    [
        TWO_BY_FOUR,
        TWO_BY_SIX,
        TWO_BY_EIGHT,
        TWO_BY_TEN,
        TWO_BY_TWELVE,
    ]
    .into_iter()
    .find(|l| allowable_span(use_, *l, spacing) >= span)
}

/// `14' 2 1/2"` for 170.5 inches.
fn feet_inches(inches: f64) -> String {
    let whole = (inches / 12.0).floor();
    let rest = inches - whole * 12.0;
    if rest < 1.0 / 32.0 {
        format!("{whole}'")
    } else {
        format!("{whole}' {}\"", format_inches(rest))
    }
}

/// Members that carry more than their size allows, grouped by use, size and
/// spacing.
#[derive(Debug, Clone, PartialEq)]
pub struct SpanIssue {
    pub use_: SpanUse,
    pub lumber: Lumber,
    pub spacing: f64,
    /// How many members are over.
    pub count: usize,
    /// The longest span among them, inches.
    pub longest: f64,
    /// What that lumber allows at that spacing, inches.
    pub allowed: f64,
}

impl SpanIssue {
    /// One sentence for the build report, e.g. `3 floor joists 2x6 span up to
    /// 14' (2x6 at 16" o.c. carries 9' 2")`.
    pub fn message(&self) -> String {
        format!(
            "{} {}{} {} span up to {} ({} at {}\" o.c. carries {})",
            self.count,
            self.use_.name(),
            if self.count == 1 { "" } else { "s" },
            self.lumber.nominal_name(),
            feet_inches(self.longest),
            self.lumber.nominal_name(),
            format_inches(self.spacing),
            feet_inches(self.allowed),
        )
    }

    /// The lumber that would carry the longest span, if a 2x does.
    pub fn suggestion(&self) -> Option<Lumber> {
        smallest_for(self.use_, self.longest, self.spacing)
    }
}

/// Checks the joists, ceiling joists and rafters among `members` against the
/// spacings in the defaults. A rafter's span is its horizontal run. Trusses,
/// beams and headers are not checked.
pub fn check_spans(
    members: &[Member],
    d: &FramingDefaults,
    roof: &RoofFramingDefaults,
) -> Vec<SpanIssue> {
    let mut out: Vec<SpanIssue> = Vec::new();
    for m in members {
        let (use_, spacing, span) = match m.kind {
            MemberKind::Joist => (SpanUse::Floor, d.joist_spacing, m.length),
            MemberKind::CeilingJoist => (SpanUse::Ceiling, d.ceiling_joist_spacing, m.length),
            MemberKind::Rafter => {
                let pitch = m.cuts.pitch_in_12;
                let run = m.length * (12.0 / (144.0 + pitch * pitch).sqrt());
                (SpanUse::Rafter, roof.spacing, run)
            }
            _ => continue,
        };
        let allowed = allowable_span(use_, m.lumber, spacing);
        if span <= allowed + 1e-6 {
            continue;
        }
        match out
            .iter_mut()
            .find(|i| i.use_ == use_ && i.lumber == m.lumber && i.spacing == spacing)
        {
            Some(i) => {
                i.count += 1;
                i.longest = i.longest.max(span);
            }
            None => out.push(SpanIssue {
                use_,
                lumber: m.lumber,
                spacing,
                count: 1,
                longest: span,
                allowed,
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::member::Transform3;

    #[test]
    fn spans_grow_with_depth_and_shrink_with_spacing() {
        for use_ in [SpanUse::Floor, SpanUse::Ceiling, SpanUse::Rafter] {
            let spans: Vec<f64> = [TWO_BY_SIX, TWO_BY_EIGHT, TWO_BY_TEN, TWO_BY_TWELVE]
                .iter()
                .map(|l| allowable_span(use_, *l, 16.0))
                .collect();
            assert!(spans.windows(2).all(|w| w[0] < w[1]), "{use_:?} {spans:?}");
            assert!(
                allowable_span(use_, TWO_BY_EIGHT, 12.0) > allowable_span(use_, TWO_BY_EIGHT, 16.0)
            );
            assert!(
                allowable_span(use_, TWO_BY_EIGHT, 16.0) > allowable_span(use_, TWO_BY_EIGHT, 24.0)
            );
        }
    }

    #[test]
    fn floor_joist_spans_are_in_the_range_the_span_tables_give() {
        // The IRC tables for DF-L No. 2 at 16" and 40/10 psf run from about
        // 10' (2x6) to 21' (2x12); the beam calculation (no shear or bearing
        // adjustments) lands a little under them.
        let ft = |l| allowable_span(SpanUse::Floor, l, 16.0) / 12.0;
        assert!((8.5..11.0).contains(&ft(TWO_BY_SIX)), "{}", ft(TWO_BY_SIX));
        assert!(
            (12.0..14.5).contains(&ft(TWO_BY_EIGHT)),
            "{}",
            ft(TWO_BY_EIGHT)
        );
        assert!((15.0..18.5).contains(&ft(TWO_BY_TEN)), "{}", ft(TWO_BY_TEN));
        assert!(
            (17.0..22.0).contains(&ft(TWO_BY_TWELVE)),
            "{}",
            ft(TWO_BY_TWELVE)
        );
        // Ceiling joists carry far less load, so reach further.
        assert!(
            allowable_span(SpanUse::Ceiling, TWO_BY_SIX, 16.0)
                > allowable_span(SpanUse::Floor, TWO_BY_SIX, 16.0)
        );
    }

    #[test]
    fn the_smallest_lumber_for_a_span_is_found() {
        let l = smallest_for(SpanUse::Floor, 12.0 * 12.0, 16.0).unwrap();
        assert_eq!(l, TWO_BY_EIGHT);
        assert!(smallest_for(SpanUse::Floor, 40.0 * 12.0, 16.0).is_none());
        assert_eq!(
            smallest_for(SpanUse::Floor, 5.0 * 12.0, 16.0).map(|l| l.depth),
            Some(3.5)
        );
    }

    fn member(kind: MemberKind, lumber: Lumber, length: f64) -> Member {
        Member::new(
            kind,
            lumber,
            length,
            Transform3 {
                origin: [0.0; 3],
                axis_x: [1.0, 0.0, 0.0],
                axis_y: [0.0, 1.0, 0.0],
            },
            None,
        )
    }

    #[test]
    fn long_joists_and_rafters_are_reported_once_per_size() {
        let d = FramingDefaults::default();
        let roof = RoofFramingDefaults::default();
        let members = vec![
            member(MemberKind::Joist, TWO_BY_SIX, 168.0),
            member(MemberKind::Joist, TWO_BY_SIX, 180.0),
            member(MemberKind::Joist, TWO_BY_SIX, 96.0),
            member(MemberKind::Joist, TWO_BY_TEN, 168.0),
            member(MemberKind::Rafter, TWO_BY_SIX, 300.0),
            member(MemberKind::Stud, TWO_BY_SIX, 300.0),
        ];
        let issues = check_spans(&members, &d, &roof);
        let floor: Vec<_> = issues.iter().filter(|i| i.use_ == SpanUse::Floor).collect();
        assert_eq!(floor.len(), 1);
        assert_eq!(floor[0].count, 2);
        assert_eq!(floor[0].longest, 180.0);
        assert!(floor[0]
            .message()
            .starts_with("2 floor joists 2x6 span up to"));
        assert!(floor[0]
            .suggestion()
            .is_some_and(|l| l.depth > TWO_BY_SIX.depth));
        assert_eq!(
            issues.iter().filter(|i| i.use_ == SpanUse::Rafter).count(),
            1
        );
        // Nothing else (studs, the 2x10 within its span, the short joist).
        assert_eq!(issues.len(), 2);
        assert!(check_spans(&members[2..4], &d, &roof).is_empty());
    }
}
