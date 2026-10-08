//! Deck framing (CB-86): joists, rim joists, a ledger at the house wall,
//! beams and posts with footings, built from the outline and the Deck
//! Specification of a deck room.
//!
//! The members are [`FramingMember`]s so they draw in the plan, mesh in 3D,
//! export and count in the takeoff like any manual framing. Every member's
//! label starts with [`DECK_PREFIX`], which is how a rebuild finds the ones a
//! deck made before.
//!
//! The layout: joists run away from the longest ledger edge (or across the
//! shorter side of a freestanding deck), `joist_spacing` apart on centre,
//! between the rim joists and the ledger. The support lines are the ledger and
//! a beam `beam_setback` in from the far rim; a freestanding deck has a beam
//! at each end; where the clear span would pass what the joist lumber carries
//! (the span check of [`crate::span`]) beams are added between. Posts stand
//! under each beam at most `post_spacing` apart, each on its footing.

use crate::lumber::Lumber;
use crate::manual::{post_with_footing, FootingSpec, FramingMember, LumberSize, MemberKind as K};
use crate::span::{allowable_span, SpanUse};
use plan_core::deck::{
    axes, clip_line, extent, joist_angle, joist_depth, joist_lines, parse_size, DeckSpec,
};
use plan_core::{Id, Point};

/// Every member of a deck has a label that starts with this.
pub const DECK_PREFIX: &str = "Deck ";

/// Thickness of the rim joists and the ledger, inches.
const BOARD: f64 = 1.5;
/// A post stands this far in from the end of its beam, inches.
const POST_INSET: f64 = 6.0;
/// Posts shorter than this get a warning, inches.
const MIN_POST: f64 = 6.0;

/// What a deck build makes.
#[derive(Debug, Clone, Default)]
pub struct DeckFraming {
    pub members: Vec<FramingMember>,
    /// Direction the joists run, degrees.
    pub joist_angle: f64,
    /// Positions of the supports along the joists (the ledger and the
    /// beams), inches along the joist direction.
    pub supports: Vec<f64>,
    /// Things the build could not do or the user should look at.
    pub warnings: Vec<String>,
}

impl DeckFraming {
    pub fn count(&self, kind: K, what: &str) -> usize {
        self.members
            .iter()
            .filter(|m| m.kind == kind && m.label.contains(what))
            .count()
    }

    pub fn joists(&self) -> usize {
        self.count(K::Joist, "joist ") - self.rims()
    }

    pub fn rims(&self) -> usize {
        self.count(K::Joist, "rim joist")
    }

    pub fn beams(&self) -> usize {
        self.count(K::FloorCeilingBeam, "beam")
    }

    pub fn posts(&self) -> usize {
        self.members
            .iter()
            .filter(|m| m.kind == K::PostWithFooting)
            .count()
    }

    pub fn ledgers(&self) -> usize {
        self.count(K::GeneralFraming, "ledger")
    }
}

/// Is `m` a member some deck's build made?
pub fn is_deck_member(m: &FramingMember) -> bool {
    m.label.starts_with(DECK_PREFIX)
}

/// The section of a size name such as `"2x8"`; 2x8 when it does not parse.
pub fn lumber_of(size: &str) -> LumberSize {
    let (t, d) = parse_size(size).unwrap_or((2, 8));
    LumberSize::dim(t, d)
}

/// What the joists of `spec` carry between supports, inches.
pub fn max_joist_span(spec: &DeckSpec) -> f64 {
    let size = lumber_of(&spec.framing.joist_size);
    let lumber = Lumber {
        thickness: size.width(),
        depth: size.depth(),
    };
    allowable_span(SpanUse::Floor, lumber, spec.framing.joist_spacing)
}

/// The inputs of a build.
pub struct DeckInput<'a> {
    /// Interior outline of the deck room.
    pub outline: &'a [Point],
    /// Edges of the outline that lie against the house.
    pub ledger_edges: &'a [usize],
    pub spec: &'a DeckSpec,
    /// Scene elevation of the top of the decking boards.
    pub deck_top: f64,
}

/// Builds the framing of one deck. The outline must be counter-clockwise
/// (`plan_core::deck::deck_rooms` gives it so) and `ledger_edges` index it.
/// `next_id` hands out the ids of the members.
pub fn build_deck_framing(input: &DeckInput, next_id: &mut dyn FnMut() -> Id) -> DeckFraming {
    let mut out = DeckFraming::default();
    let spec = input.spec;
    let f = &spec.framing;
    let outline = input.outline;
    let n = outline.len();
    if n < 3 || !f.enabled {
        return out;
    }
    let ledger: Vec<usize> = if f.ledger {
        input.ledger_edges.to_vec()
    } else {
        Vec::new()
    };
    let angle = joist_angle(outline, &ledger, f);
    out.joist_angle = angle;
    let (dir, across) = axes(angle);

    let joist_size = lumber_of(&f.joist_size);
    let beam_size = lumber_of(&f.beam_size);
    let post_size = lumber_of(&f.post_size);
    let jd = joist_depth(f);
    let joist_top = input.deck_top - spec.board_thickness();
    let joist_bottom = joist_top - jd;
    let beam_depth = beam_size.depth();
    let beam_bottom = joist_bottom - beam_depth;
    let grade = input.deck_top - f.height_above_grade.max(0.0);

    let mk = |id: Id, kind: K, a: Point, b: Point, size: LumberSize, label: String, z: f64| {
        let mut m = FramingMember::new(id, kind, a, b).with_lumber(size);
        m.label = format!("{DECK_PREFIX}{label} {}", size.name());
        m.elevation_bottom = z;
        m
    };

    // Extent of the outline along the joists.
    let Some((_, across_hi)) = extent(outline, across) else {
        return out;
    };
    let Some((t_lo, t_hi)) = extent(outline, dir) else {
        return out;
    };

    // Joists between the rims (and the ledger): a rim or ledger takes the
    // outer joist position, so lines start one spacing in and stop short of
    // the far edge.
    let rim = f.rim_joists;
    let first = if rim { f.joist_spacing } else { BOARD * 0.5 };
    for (_, run) in joist_lines(outline, angle, first, f.joist_spacing) {
        let v = run.a.dot(across);
        if rim && v > across_hi - BOARD - 0.5 {
            continue;
        }
        let d = (run.b - run.a).normalized();
        // End joists butt the rim or ledger.
        let (a, b) = (run.a + d * BOARD, run.b - d * BOARD);
        if a.dist(b) < 6.0 {
            continue;
        }
        out.members.push(mk(
            next_id(),
            K::Joist,
            a,
            b,
            joist_size,
            "joist".into(),
            joist_bottom,
        ));
    }
    // Without rims the last joist closes the far side.
    if !rim {
        let v = across_hi - BOARD * 0.5;
        for (t0, t1) in clip_line(outline, across * v, dir) {
            let (a, b) = (
                across * v + dir * (t0 + BOARD),
                across * v + dir * (t1 - BOARD),
            );
            if a.dist(b) >= 6.0 {
                out.members.push(mk(
                    next_id(),
                    K::Joist,
                    a,
                    b,
                    joist_size,
                    "joist".into(),
                    joist_bottom,
                ));
            }
        }
    }

    // Rim joists on every edge that is not the ledger, the ledger on the rest.
    for i in 0..n {
        let (p, q) = (outline[i], outline[(i + 1) % n]);
        if p.dist(q) < 6.0 {
            continue;
        }
        let along = (q - p).normalized();
        let inward = along.perp(); // outline is counter-clockwise
        let (a, b) = (p + inward * (BOARD * 0.5), q + inward * (BOARD * 0.5));
        if ledger.contains(&i) {
            out.members.push(mk(
                next_id(),
                K::GeneralFraming,
                a,
                b,
                joist_size,
                "ledger".into(),
                joist_bottom,
            ));
        } else if rim {
            out.members.push(mk(
                next_id(),
                K::Joist,
                a,
                b,
                joist_size,
                "rim joist".into(),
                joist_bottom,
            ));
        }
    }

    // Supports along the joists: the ledger side and the beams.
    let mid_t = (t_lo + t_hi) * 0.5;
    let ledger_t: Vec<f64> = ledger
        .iter()
        .map(|i| {
            let (p, q) = (outline[*i], outline[(*i + 1) % n]);
            Point::lerp(p, q, 0.5).dot(dir)
        })
        .filter(|t| (t - t_lo).abs() < 6.0 || (t - t_hi).abs() < 6.0)
        .collect();
    let at_low = ledger_t.iter().any(|t| *t < mid_t);
    let at_high = ledger_t.iter().any(|t| *t >= mid_t);
    let setback = f.beam_setback.max(0.0);
    let mut supports: Vec<f64> = Vec::new();
    match (at_low, at_high) {
        (true, false) => {
            supports.push(t_lo);
            supports.push(t_hi - setback);
        }
        (false, true) => {
            supports.push(t_lo + setback);
            supports.push(t_hi);
        }
        (true, true) => {
            supports.push(t_lo);
            supports.push(t_hi);
        }
        (false, false) => {
            supports.push(t_lo + setback);
            supports.push(t_hi - setback);
        }
    }
    supports.sort_by(f64::total_cmp);
    // Intermediate beams where the clear span passes the allowable one.
    let allowed = max_joist_span(spec).max(24.0);
    let mut with_mid: Vec<f64> = vec![supports[0]];
    for w in supports.windows(2) {
        let gap = w[1] - w[0];
        let extra = (gap / allowed).ceil() as usize;
        for k in 1..extra {
            with_mid.push(w[0] + gap * k as f64 / extra as f64);
        }
        with_mid.push(w[1]);
    }
    let supports = with_mid;
    if supports.len() >= 2 {
        let outer_cant = (supports[0] - t_lo).max(t_hi - supports[supports.len() - 1]);
        if outer_cant > allowed / 4.0 + 0.1 && outer_cant > 12.0 {
            out.warnings.push(format!(
                "The cantilever past the outer beam is {:.0} in; keep it under a quarter of the {:.0} in joist span",
                outer_cant,
                allowed
            ));
        }
    }

    // Beams under the joists, and posts under the beams. The ledger side
    // carries its load on the house, so the support at the ledger gets none.
    let ledger_pos: Vec<f64> = ledger_t.clone();
    let mut post_height_warned = false;
    for &t in &supports {
        if ledger_pos.iter().any(|lt| (lt - t).abs() < 6.0) {
            continue;
        }
        for (s0, s1) in clip_line(outline, dir * t, across) {
            let (a, b) = (
                dir * t + across * (s0 + BOARD),
                dir * t + across * (s1 - BOARD),
            );
            let len = a.dist(b);
            if len < 12.0 {
                continue;
            }
            let mut beam = mk(
                next_id(),
                K::FloorCeilingBeam,
                a,
                b,
                beam_size,
                "beam".into(),
                beam_bottom,
            );
            beam.plies = f.beam_plies.max(1);
            beam.width = beam_size.width() * f64::from(beam.plies);
            beam.label = format!("{DECK_PREFIX}beam ({}) {}", beam.plies, beam_size.name());
            out.members.push(beam);

            // Posts: one at each end, evenly between, at most post_spacing apart.
            let usable = (len - 2.0 * POST_INSET).max(0.0);
            let gaps = (usable / f.post_spacing.max(24.0)).ceil().max(1.0) as usize;
            for k in 0..=gaps {
                let at = POST_INSET + usable * k as f64 / gaps as f64;
                let c = a + (b - a).normalized() * at;
                let height = beam_bottom - grade;
                if height < MIN_POST && !post_height_warned {
                    post_height_warned = true;
                    out.warnings.push(format!(
                        "The posts are only {:.1} in tall: the deck is low to the ground",
                        height.max(0.0)
                    ));
                }
                let footing = FootingSpec {
                    size: f.footing_size.max(6.0),
                    thickness: f.footing_thickness.max(4.0),
                };
                let (mut post, _) =
                    post_with_footing(next_id(), c, grade, height.max(1.0), footing);
                post = post.with_lumber(post_size);
                post.label = format!("{DECK_PREFIX}post {}", post_size.name());
                out.members.push(post);
            }
        }
    }
    out.supports = supports;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::deck::{DeckFramingSpec, DeckSpec};

    fn rect(w: f64, h: f64) -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, h),
            Point::new(0.0, h),
        ]
    }

    fn build(outline: &[Point], ledger: &[usize], spec: &DeckSpec) -> DeckFraming {
        let mut id = 100;
        let mut next = || {
            id += 1;
            id
        };
        build_deck_framing(
            &DeckInput {
                outline,
                ledger_edges: ledger,
                spec,
                deck_top: 36.0,
            },
            &mut next,
        )
    }

    #[test]
    fn a_ledgered_deck_has_joists_a_ledger_a_far_beam_and_posts() {
        let spec = DeckSpec::default();
        let o = rect(192.0, 96.0);
        let r = build(&o, &[0], &spec);
        assert!((r.joist_angle - 90.0).abs() < 1e-9);
        // 192" at 16" OC: joists at 16..176.
        assert_eq!(r.joists(), 11);
        assert_eq!(r.ledgers(), 1);
        // Rims on the three non-ledger edges.
        assert_eq!(r.rims(), 3);
        assert_eq!(r.beams(), 1);
        // 189" of beam, posts at most 96" apart: 3.
        assert_eq!(r.posts(), 3);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        assert!(r.members.iter().all(is_deck_member));
    }

    #[test]
    fn the_members_stack_from_the_decking_down_to_the_footing() {
        let spec = DeckSpec::default();
        let o = rect(192.0, 96.0);
        let r = build(&o, &[0], &spec);
        let joist = r
            .members
            .iter()
            .find(|m| m.label.contains("joist 2x8"))
            .unwrap();
        // Decking top 36, boards 1.5, joists 7.25 deep.
        assert!((joist.elevation_bottom + joist.depth - (36.0 - 1.5)).abs() < 1e-9);
        let beam = r
            .members
            .iter()
            .find(|m| m.kind == K::FloorCeilingBeam)
            .unwrap();
        assert!((beam.elevation_bottom + beam.depth - joist.elevation_bottom).abs() < 1e-9);
        assert_eq!(beam.plies, 2);
        let post = r
            .members
            .iter()
            .find(|m| m.kind == K::PostWithFooting)
            .unwrap();
        // Posts stand on grade (36" below the top) up to the beam.
        assert!((post.elevation_bottom - 0.0).abs() < 1e-9);
        assert!((post.elevation_bottom + post.height - beam.elevation_bottom).abs() < 1e-9);
        let footing = post.footing().unwrap();
        assert_eq!(footing.size, 18.0);
        assert_eq!(footing.thickness, 12.0);
    }

    #[test]
    fn the_far_beam_sits_the_setback_in_from_the_far_rim() {
        let spec = DeckSpec::default();
        let o = rect(192.0, 96.0);
        let r = build(&o, &[0], &spec);
        let beam = r
            .members
            .iter()
            .find(|m| m.kind == K::FloorCeilingBeam)
            .unwrap();
        assert!((beam.start.y - 84.0).abs() < 1e-9 && (beam.end.y - 84.0).abs() < 1e-9);
        assert_eq!(r.supports.len(), 2);
    }

    #[test]
    fn a_freestanding_deck_has_a_beam_at_each_end() {
        let spec = DeckSpec::default();
        let o = rect(192.0, 96.0);
        let r = build(&o, &[], &spec);
        assert_eq!(r.ledgers(), 0);
        assert_eq!(r.beams(), 2);
        assert_eq!(r.rims(), 4);
        assert_eq!(r.posts(), 6);
    }

    #[test]
    fn a_long_span_gets_a_beam_in_the_middle() {
        let spec = DeckSpec::default();
        // 2x8 at 16" carries about 153": a 20 ft run needs another beam.
        let o = rect(144.0, 240.0);
        let r = build(&o, &[0], &spec);
        assert!(
            (max_joist_span(&spec) - 153.0).abs() < 10.0,
            "{}",
            max_joist_span(&spec)
        );
        assert_eq!(r.beams(), 2, "{:?}", r.supports);
        // Two beams: the middle one and the far one.
        assert_eq!(r.supports.len(), 3);
    }

    #[test]
    fn a_bigger_joist_spans_further() {
        let mut spec = DeckSpec::default();
        let small = max_joist_span(&spec);
        spec.framing.joist_size = "2x12".into();
        assert!(max_joist_span(&spec) > small * 1.4);
        spec.framing.joist_spacing = 24.0;
        let wide = max_joist_span(&spec);
        spec.framing.joist_spacing = 12.0;
        assert!(max_joist_span(&spec) > wide);
    }

    #[test]
    fn spacing_changes_the_joist_count_and_the_angle_can_be_set() {
        let mut spec = DeckSpec::default();
        spec.framing.joist_spacing = 24.0;
        let o = rect(192.0, 96.0);
        let r = build(&o, &[0], &spec);
        // 24" OC: 24..168 = 7 joists.
        assert_eq!(r.joists(), 7);
        spec.framing = DeckFramingSpec {
            joist_angle: Some(0.0),
            ..spec.framing
        };
        let r = build(&o, &[0], &spec);
        assert_eq!(r.joist_angle, 0.0);
        // Joists run along x now, 96" spacing across: 24" OC from y = 0.
        assert!(r.joists() >= 3);
    }

    #[test]
    fn a_low_deck_warns_about_short_posts() {
        let mut spec = DeckSpec::default();
        spec.framing.height_above_grade = 14.0;
        let o = rect(192.0, 96.0);
        let r = build(&o, &[0], &spec);
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("posts"));
    }

    #[test]
    fn framing_can_be_switched_off() {
        let mut spec = DeckSpec::default();
        spec.framing.enabled = false;
        assert!(build(&rect(192.0, 96.0), &[0], &spec).members.is_empty());
    }

    #[test]
    fn no_rim_joists_means_joists_at_the_edges() {
        let mut spec = DeckSpec::default();
        spec.framing.rim_joists = false;
        let o = rect(192.0, 96.0);
        let r = build(&o, &[0], &spec);
        assert_eq!(r.rims(), 0);
        // The last joist closes the far side.
        let far = r
            .members
            .iter()
            .filter(|m| m.kind == K::Joist && m.start.x > 190.0)
            .count();
        assert_eq!(far, 1);
    }

    #[test]
    fn the_takeoff_counts_the_deck_lumber() {
        let spec = DeckSpec::default();
        let o = rect(192.0, 96.0);
        let r = build(&o, &[0], &spec);
        let list = crate::MaterialList::from_members(&[], &r.members);
        // 11 joists, 3 rims, a ledger, a 2-ply beam and 3 posts.
        assert!(list.total_pieces() >= 20, "{}", list.total_pieces());
        assert!(list.total_board_feet() > 100.0);
    }
}
