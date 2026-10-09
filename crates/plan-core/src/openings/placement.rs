//! Where a door or window may stand on its wall, and what its center snaps to
//! (DW-4, DW-10, DW-73..DW-75, DW-87). Shared by the opening tools, the Select
//! tool's drag, the Specification dialog and [`Project::slide_opening`].
//!
//! Everything here is a pure function of the plan: the tool feeds it the
//! pointer's offset along the host wall and gets back the center the opening
//! would take, or `None` when the pointer is over something the opening may not
//! cover (another opening, the face of a wall that meets the host).
//!
//! The pieces:
//!
//! * [`zones`]: the stretches of the host wall no part of the opening may
//!   overlap: neighbouring openings (touching is fine for two windows, the
//!   others keep the end clearance between them), the bodies of walls that
//!   meet or cross the host plus [`JUNCTION_CLEARANCE`] (a jamb never enters
//!   the through wall, DW-87), and the ends of the host.
//! * [`candidates`]: the alignment snaps (DW-10, DW-75): the middle of the
//!   wall, the middle of a free span, equal spacing between neighbours, flush
//!   against a neighbour or a junction, and the centers and jambs of openings
//!   on other walls and other floors.
//! * [`resolve`]: the snap, then the nearest center where the opening fits.

use super::vertically_apart;
use crate::geometry::Point;
use crate::model::{Floor, Id, Opening, OpeningKind, Project, Wall};

/// Clear distance kept between a jamb and the face of a wall that meets the
/// host, and between a jamb and a wall end (Chief's 2 in junction rule; verify
/// in Chief).
pub const JUNCTION_CLEARANCE: f64 = 2.0;
/// Clear distance between two openings (touching windows excepted) and from
/// the wall ends.
pub const MARGIN: f64 = 2.0;

/// What an alignment snap lines the opening up with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    /// The middle of the wall.
    Midpoint,
    /// The middle of the free span between neighbours.
    SpanMiddle,
    /// The same spacing as between two neighbouring openings.
    EqualSpacing,
    /// Flush against a neighbour (windows touch).
    Flush,
    /// Against the face of a wall that meets the host, plus clearance.
    Junction,
    /// A center or jamb of an opening on another wall.
    OtherWall,
    /// A center or jamb of an opening on another floor.
    OtherFloor,
}

impl Align {
    /// Short name for the status line.
    pub fn name(self) -> &'static str {
        match self {
            Align::Midpoint => "wall midpoint",
            Align::SpanMiddle => "middle of the span",
            Align::EqualSpacing => "equal spacing",
            Align::Flush => "flush with the neighbour",
            Align::Junction => "wall junction",
            Align::OtherWall => "opening on another wall",
            Align::OtherFloor => "opening on another floor",
        }
    }
}

/// A stretch `lo..hi` of the host wall that no part of the opening may
/// overlap. The `core` (`core_lo..core_hi`) is what is really there: a
/// pointer inside it refuses the placement instead of sliding the opening
/// out of the way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub lo: f64,
    pub hi: f64,
    pub core_lo: f64,
    pub core_hi: f64,
}

impl Zone {
    fn new(core_lo: f64, core_hi: f64, gap: f64) -> Self {
        Zone {
            lo: core_lo - gap,
            hi: core_hi + gap,
            core_lo,
            core_hi,
        }
    }
}

/// Distance along a straight `host` and across it of `p`, not clamped to the
/// wall (a curved host measures along the arc).
fn along(host: &Wall, p: Point) -> (f64, f64) {
    if host.is_curved() {
        host.locate(p)
    } else {
        let rel = p.sub(host.start);
        (rel.dot(host.direction()), rel.dot(host.normal()))
    }
}

/// How far the body of `other`, as it crosses `host`, reaches along the host
/// to each side of the crossing: half its thickness, longer when the walls
/// meet at a slant.
fn reach(host: &Wall, other: &Wall) -> f64 {
    let sin = host
        .direction()
        .cross(other.direction())
        .abs()
        .clamp(0.2, 1.0);
    other.thickness * 0.5 / sin
}

/// Whether two walls run side by side (or end to end): a join between them
/// closes a corner but crosses no one's body.
fn parallel(a: &Wall, b: &Wall) -> bool {
    a.direction().cross(b.direction()).abs() < 0.05
}

/// The stretches of `host` occupied by the bodies of the other walls of the
/// floor that meet or cross it, with [`JUNCTION_CLEARANCE`] on each side. A
/// wall that meets the host's end covers that end.
pub fn junction_zones(floor: &Floor, host: &Wall) -> Vec<Zone> {
    let len = host.path_length();
    let half = host.thickness * 0.5;
    let mut out = Vec::new();
    for other in &floor.walls {
        if other.id == host.id || other.flags.invisible || parallel(host, other) {
            continue;
        }
        let r = reach(host, other);
        // An end of the other wall inside the host's body: a T or a corner.
        let mut crossing = None;
        for e in [other.start, other.end] {
            let (s, t) = along(host, e);
            if t.abs() <= half + 1e-6 && s > -r - 1e-6 && s < len + r + 1e-6 {
                crossing = Some(s);
                break;
            }
        }
        // An end of the host inside the other wall's body: the host is the
        // stem of a T.
        let mut stem = None;
        if crossing.is_none() {
            for (end_s, host_end) in [(0.0, host.start), (len, host.end)] {
                let (s2, t2) = along(other, host_end);
                if t2.abs() <= other.thickness * 0.5 + 1e-6
                    && s2 >= -1e-6
                    && s2 <= other.path_length() + 1e-6
                {
                    stem = Some(end_s);
                    break;
                }
            }
        }
        // Crossing walls: the other wall passes right through the host.
        let mut through = None;
        if crossing.is_none() && stem.is_none() && !host.is_curved() && !other.is_curved() {
            let d = host.direction();
            let od = other.direction();
            let denom = d.cross(od);
            let rel = other.start.sub(host.start);
            let s = rel.cross(od) / denom;
            let s2 = rel.cross(d) / denom;
            if s > 0.0 && s < len && s2 > 0.0 && s2 < other.path_length() {
                through = Some(s);
            }
        }
        if let Some(s) = crossing.or(stem).or(through) {
            out.push(Zone::new(s - r, s + r, JUNCTION_CLEARANCE));
        }
    }
    out
}

/// Every stretch of `host` the opening `o` (the template: kind, width, sill
/// and height decide what it may touch) must stay out of, other than the
/// wall ends. `skip` is an opening to leave out (the one being dragged).
pub fn zones(floor: &Floor, host: &Wall, o: &Opening, skip: Option<Id>) -> Vec<Zone> {
    zones_skipping(floor, host, o, skip.as_slice())
}

/// [`zones`] leaving out several openings (a whole mulled unit being moved).
pub fn zones_skipping(floor: &Floor, host: &Wall, o: &Opening, skip: &[Id]) -> Vec<Zone> {
    let mut out = junction_zones(floor, host);
    for n in floor.openings_on(host.id) {
        if skip.contains(&n.id) || vertically_apart(o, n) {
            continue;
        }
        let gap = if o.kind == OpeningKind::Window && n.kind == OpeningKind::Window {
            0.0
        } else {
            MARGIN
        };
        out.push(Zone::new(n.start_offset(), n.end_offset(), gap));
    }
    out
}

/// The clear distance two openings keep between their jambs: none between
/// two windows (they may touch, DW-4), [`MARGIN`] otherwise.
pub fn clearance_between(a: &Opening, b: &Opening) -> f64 {
    if a.kind == OpeningKind::Window && b.kind == OpeningKind::Window {
        0.0
    } else {
        MARGIN
    }
}

/// Whether `a` and `b` (on the same wall) get in each other's way under the
/// placement rules: closer than [`clearance_between`] while sharing wall
/// height. A hair of slack keeps a position the rules worked out to the exact
/// clearance from failing on rounding.
pub fn conflict(a: &Opening, b: &Opening) -> bool {
    super::openings_conflict(a, b, clearance_between(a, b) - 1e-6)
}

/// Whether `o`, standing where its `center_offset` says on `host`, keeps clear
/// of the neighbouring openings (all but `skip`) and of the bodies of walls
/// that meet or cross the host.
pub fn fits_at(floor: &Floor, host: &Wall, o: &Opening, skip: &[Id]) -> bool {
    let (a, b) = (o.start_offset(), o.end_offset());
    !zones_skipping(floor, host, o, skip)
        .iter()
        .any(|z| b > z.lo + 1e-6 && a < z.hi - 1e-6)
}

/// The ranges of centers where an opening `width` wide stays clear of every
/// zone and `MARGIN` from the wall ends.
pub fn allowed(len: f64, width: f64, zones: &[Zone]) -> Vec<(f64, f64)> {
    let half = width * 0.5;
    let mut spans = vec![(half + MARGIN, len - half - MARGIN)];
    spans.retain(|s| s.1 >= s.0 - 1e-9);
    for z in zones {
        // Centers in (z.lo - half, z.hi + half) put a jamb inside the zone.
        let (a, b) = (z.lo - half, z.hi + half);
        let mut next = Vec::new();
        for (s0, s1) in spans {
            if b <= s0 + 1e-9 || a >= s1 - 1e-9 {
                next.push((s0, s1));
                continue;
            }
            if a > s0 + 1e-9 {
                next.push((s0, a));
            }
            if b < s1 - 1e-9 {
                next.push((b, s1));
            }
        }
        spans = next;
    }
    spans
}

/// The nearest center to `raw` inside `spans`.
pub fn nearest(spans: &[(f64, f64)], raw: f64) -> Option<f64> {
    spans
        .iter()
        .map(|&(a, b)| raw.clamp(a, b))
        .min_by(|x, y| (x - raw).abs().total_cmp(&(y - raw).abs()))
}

fn within(spans: &[(f64, f64)], c: f64) -> bool {
    spans.iter().any(|&(a, b)| c >= a - 1e-6 && c <= b + 1e-6)
}

/// The alignment snaps for an opening `width` wide on `host` (DW-10, DW-75):
/// centers worth snapping to, each with what it lines up with.
pub fn candidates(
    project: &Project,
    floor_ix: usize,
    host: &Wall,
    o: &Opening,
    zones: &[Zone],
    skip: Option<Id>,
) -> Vec<(f64, Align)> {
    let floor = &project.floors[floor_ix];
    let len = host.path_length();
    let w = o.width;
    let half = w * 0.5;
    let mut out = vec![(len * 0.5, Align::Midpoint)];

    // Free spans between zones and wall ends: their middles.
    let mut sorted: Vec<&Zone> = zones.iter().collect();
    sorted.sort_by(|a, b| a.lo.total_cmp(&b.lo));
    let mut edge = MARGIN;
    for z in sorted.iter() {
        if z.lo - edge >= w - 1e-9 {
            out.push(((edge + z.lo) * 0.5, Align::SpanMiddle));
        }
        edge = edge.max(z.hi);
    }
    if len - MARGIN - edge >= w - 1e-9 {
        out.push(((edge + len - MARGIN) * 0.5, Align::SpanMiddle));
    }

    // Flush against zones: touching windows, or the junction's clearance.
    for z in &sorted {
        out.push((z.hi + half, Align::Flush));
        out.push((z.lo - half, Align::Flush));
    }
    // A junction's clearance is the same position; name it for the status.
    for z in junction_zones(floor, host) {
        out.push((z.hi + half, Align::Junction));
        out.push((z.lo - half, Align::Junction));
    }

    // Equal spacing: the pitch between two neighbours repeated beyond them.
    let mut cs: Vec<f64> = floor
        .openings_on(host.id)
        .filter(|n| Some(n.id) != skip && !vertically_apart(o, n))
        .map(|n| n.center_offset)
        .collect();
    cs.sort_by(f64::total_cmp);
    for pair in cs.windows(2) {
        let pitch = pair[1] - pair[0];
        out.push((pair[1] + pitch, Align::EqualSpacing));
        out.push((pair[0] - pitch, Align::EqualSpacing));
    }

    // Openings on other walls and floors: same center or jamb, so heads and
    // sills run across the house and window stacks line up (DW-75).
    if !host.is_curved() {
        let d = host.direction();
        for (fi, f) in project.floors.iter().enumerate() {
            for n in &f.openings {
                if fi == floor_ix && (n.wall_id == host.id || Some(n.id) == skip) {
                    continue;
                }
                let Some(w2) = f.wall(n.wall_id) else {
                    continue;
                };
                if w2.is_curved() || !parallel(host, w2) {
                    continue;
                }
                let (c, a, b) = (
                    w2.point_along(n.center_offset),
                    w2.point_along(n.start_offset()),
                    w2.point_along(n.end_offset()),
                );
                // Another floor's wall must stand where this one does.
                let across = host.normal().dot(c.sub(host.start)).abs();
                if fi != floor_ix && across > host.thickness + 12.0 {
                    continue;
                }
                let kind = if fi == floor_ix {
                    Align::OtherWall
                } else {
                    Align::OtherFloor
                };
                let proj = |p: Point| p.sub(host.start).dot(d);
                let (sc, sa, sb) = (proj(c), proj(a), proj(b));
                let (lo, hi) = (sa.min(sb), sa.max(sb));
                out.push((sc, kind));
                out.push((lo + half, kind));
                out.push((hi - half, kind));
            }
        }
    }
    out
}

/// The outcome of [`resolve`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    pub center: f64,
    pub align: Option<Align>,
}

/// The center an opening of `o`'s size takes when the pointer is `raw` inches
/// along `host`: the alignment snap within `tol` if there is one, else the
/// grid (`unit`, skipped when `snap` is false), then the nearest center where
/// the opening fits. `None` when the pointer is over a neighbour or a through
/// wall (inside a zone's core), or the opening fits nowhere.
#[allow(clippy::too_many_arguments)]
pub fn resolve(
    project: &Project,
    floor_ix: usize,
    host: &Wall,
    o: &Opening,
    raw: f64,
    unit: f64,
    tol: f64,
    snap: bool,
    skip: Option<Id>,
) -> Option<Resolved> {
    let floor = &project.floors[floor_ix];
    let len = host.path_length();
    if len < o.width + 2.0 * MARGIN {
        return None;
    }
    let zs = zones(floor, host, o, skip);
    if zs
        .iter()
        .any(|z| raw > z.core_lo + 1e-6 && raw < z.core_hi - 1e-6)
    {
        return None;
    }
    let spans = allowed(len, o.width, &zs);
    if spans.is_empty() {
        return None;
    }
    let mut center = raw;
    let mut align = None;
    if snap {
        center = (raw / unit).round() * unit;
        let best = candidates(project, floor_ix, host, o, &zs, skip)
            .into_iter()
            .filter(|(c, _)| within(&spans, *c) && (c - raw).abs() <= tol)
            .min_by(|a, b| (a.0 - raw).abs().total_cmp(&(b.0 - raw).abs()));
        if let Some((c, a)) = best {
            center = c;
            align = Some(a);
        }
    }
    let center = nearest(&spans, center)?;
    Some(Resolved { center, align })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WallKind;
    use crate::openings::OpeningStyle;

    fn project_with_wall(len: f64) -> (Project, Id) {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(len, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        (p, w)
    }

    fn door(width: f64) -> Opening {
        let mut o = Opening::default_door(0, 0, 0.0);
        o.width = width;
        o
    }

    fn window(width: f64) -> Opening {
        let mut o = Opening::default_window(0, 0, 0.0);
        o.width = width;
        o
    }

    fn at(p: &Project, w: Id, o: &Opening, raw: f64) -> Option<Resolved> {
        let host = p.floors[0].wall(w).unwrap();
        resolve(p, 0, host, o, raw, 1.0, 6.0, true, None)
    }

    #[test]
    fn the_middle_of_the_wall_wins_near_the_middle() {
        let (p, w) = project_with_wall(240.0);
        let r = at(&p, w, &door(32.0), 123.0).unwrap();
        assert_eq!((r.center, r.align), (120.0, Some(Align::Midpoint)));
        // Far from it, the grid.
        let r = at(&p, w, &door(32.0), 60.4).unwrap();
        assert_eq!((r.center, r.align), (60.0, None));
    }

    #[test]
    fn a_wall_that_meets_the_host_keeps_the_jamb_off_its_face() {
        let (mut p, w) = project_with_wall(240.0);
        // A 4 1/2" partition meeting the wall at 100".
        p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(100.0, 120.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let host = p.floors[0].wall(w).unwrap();
        let zs = junction_zones(&p.floors[0], host);
        assert_eq!(zs.len(), 1);
        // Its body is 97.75..102.25, and the clearance adds 2" each side.
        assert!((zs[0].core_lo - 97.75).abs() < 1e-9 && (zs[0].core_hi - 102.25).abs() < 1e-9);
        assert!((zs[0].lo - 95.75).abs() < 1e-9 && (zs[0].hi - 104.25).abs() < 1e-9);
        // A 32" door clicked beside the junction slides to 104.25 + 16.
        let r = at(&p, w, &door(32.0), 108.0).unwrap();
        assert_eq!(r.center, 120.25);
        // Clicked on the partition's footprint, it is refused.
        assert!(at(&p, w, &door(32.0), 100.0).is_none());
        // And on the other side it stops short of the face.
        let r = at(&p, w, &door(32.0), 90.0).unwrap();
        assert!(r.center + 16.0 <= 95.75 + 1e-9, "{}", r.center);
    }

    #[test]
    fn a_corner_covers_the_end_of_the_wall() {
        let (mut p, w) = project_with_wall(240.0);
        p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(0.0, 120.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        // The side wall's body (to 3") plus 2" clearance: a 36" door starts at 5".
        let r = at(&p, w, &door(36.0), 10.0).unwrap();
        assert_eq!(r.center, 23.0);
    }

    #[test]
    fn windows_touch_but_a_door_keeps_its_clearance() {
        let (mut p, w) = project_with_wall(300.0);
        let mut first = window(36.0);
        first.center_offset = 100.0;
        first.wall_id = w;
        first.id = p.alloc_id();
        p.floors[0].openings.push(first);
        // A window next to it snaps flush: its start at 118, center 136.
        let r = at(&p, w, &window(36.0), 140.0).unwrap();
        assert_eq!((r.center, r.align), (136.0, Some(Align::Flush)));
        // A door keeps 2" clear of it (a door is also not vertically apart).
        let r = at(&p, w, &door(36.0), 140.0).unwrap();
        assert_eq!(r.center, 138.0);
        // On top of it, nothing is placed.
        assert!(at(&p, w, &window(36.0), 105.0).is_none());
    }

    #[test]
    fn the_middle_of_a_free_span_and_equal_spacing_snap() {
        let (mut p, w) = project_with_wall(400.0);
        for c in [50.0, 150.0] {
            let mut o = window(30.0);
            o.center_offset = c;
            o.wall_id = w;
            o.id = p.alloc_id();
            p.floors[0].openings.push(o);
        }
        let host = p.floors[0].wall(w).unwrap().clone();
        // Pitch 100: the next one goes at 250.
        let r = resolve(&p, 0, &host, &window(30.0), 253.0, 1.0, 6.0, true, None).unwrap();
        assert_eq!((r.center, r.align), (250.0, Some(Align::EqualSpacing)));
        // Between two far-apart windows the middle of the span.
        let mut far = window(30.0);
        far.center_offset = 350.0;
        far.wall_id = w;
        far.id = p.alloc_id();
        p.floors[0].openings.push(far);
        let host = p.floors[0].wall(w).unwrap().clone();
        // Free span between 165 and 335: middle 250 (also the spacing); use a
        // door so only the span middle (not the window rhythm) is in play.
        let r = resolve(&p, 0, &host, &door(30.0), 247.0, 1.0, 6.0, true, None).unwrap();
        assert_eq!(r.center, 250.0);
        assert!(matches!(
            r.align,
            Some(Align::SpanMiddle | Align::EqualSpacing | Align::Midpoint)
        ));
    }

    #[test]
    fn alt_places_exactly_where_the_pointer_is() {
        let (p, w) = project_with_wall(240.0);
        let host = p.floors[0].wall(w).unwrap();
        let r = resolve(&p, 0, host, &door(32.0), 123.4, 1.0, 6.0, false, None).unwrap();
        assert_eq!((r.center, r.align), (123.4, None));
    }

    #[test]
    fn openings_on_other_walls_and_floors_are_alignment_candidates() {
        let (mut p, w) = project_with_wall(400.0);
        // A parallel wall 120" away with a window centered at 250".
        let back = p.add_wall(
            0,
            Point::new(0.0, 120.0),
            Point::new(400.0, 120.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let mut across = window(36.0);
        across.center_offset = 250.0;
        across.wall_id = back;
        across.id = p.alloc_id();
        p.floors[0].openings.push(across);
        let r = at(&p, w, &window(36.0), 253.0).unwrap();
        assert_eq!((r.center, r.align), (250.0, Some(Align::OtherWall)));
        // The same on the floor above, where the wall stands over this one.
        let (mut p2, w2) = project_with_wall(400.0);
        let up = p2.insert_floor_above(0).unwrap();
        let upper = p2.add_wall(
            up,
            Point::new(0.0, 0.0),
            Point::new(400.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let mut above = window(36.0);
        above.center_offset = 90.0;
        above.wall_id = upper;
        above.id = p2.alloc_id();
        p2.floors[up].openings.push(above);
        let host = p2.floors[0].wall(w2).unwrap();
        let r = resolve(&p2, 0, host, &window(36.0), 93.0, 1.0, 6.0, true, None).unwrap();
        assert_eq!((r.center, r.align), (90.0, Some(Align::OtherFloor)));
        // A stacked door is not in the way of a window above: skip logic.
        let _ = OpeningStyle::Hinged;
    }

    #[test]
    fn a_crossing_wall_blocks_the_host_where_it_passes() {
        let (mut p, w) = project_with_wall(240.0);
        p.add_wall(
            0,
            Point::new(80.0, -60.0),
            Point::new(80.0, 60.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let host = p.floors[0].wall(w).unwrap();
        let zs = junction_zones(&p.floors[0], host);
        assert_eq!(zs.len(), 1);
        assert!((zs[0].core_lo - 77.75).abs() < 1e-9);
    }

    #[test]
    fn a_wall_too_short_for_the_opening_has_no_place() {
        let (p, w) = project_with_wall(30.0);
        assert!(at(&p, w, &door(32.0), 15.0).is_none());
        assert!(at(&p, w, &door(24.0), 15.0).is_some());
    }

    #[test]
    fn sliding_follows_the_shared_rules() {
        let (mut p, host) = project_with_wall(240.0);
        // A wall meeting the host at 120 in, 6 in thick: its body plus the
        // 2 in clearance covers 115..125 plus the clearance, 113..127.
        p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        let a = p.add_opening(0, host, 40.0, OpeningKind::Window).unwrap();
        let b = p.add_opening(0, host, 200.0, OpeningKind::Window).unwrap();
        let width = p.floors[0]
            .openings
            .iter()
            .find(|o| o.id == a)
            .unwrap()
            .width;
        // The jamb may not enter the clearance of the meeting wall.
        assert!(!p.slide_opening(0, a, 120.0 - 3.0 - 2.0 - width * 0.5 + 1.0));
        assert!(p.slide_opening(0, a, 120.0 - 3.0 - 2.0 - width * 0.5));
        // Two windows may touch, as in the tool.
        let wb = p.floors[0]
            .openings
            .iter()
            .find(|o| o.id == b)
            .unwrap()
            .width;
        let ca = p.floors[0]
            .openings
            .iter()
            .find(|o| o.id == a)
            .unwrap()
            .center_offset;
        assert!(p.slide_opening(0, b, ca + width * 0.5 + wb * 0.5 + 10.0));
        assert!(p.slide_opening(0, b, 120.0 + 3.0 + 2.0 + wb * 0.5));
        // A door keeps its clearance from a window.
        let d = p.add_opening(0, host, 200.0, OpeningKind::Door).unwrap();
        let host_ref = p.floors[0].wall(host).unwrap();
        let mut probe = p.floors[0]
            .openings
            .iter()
            .find(|o| o.id == d)
            .unwrap()
            .clone();
        let wb_ = p.floors[0]
            .openings
            .iter()
            .find(|o| o.id == b)
            .unwrap()
            .clone();
        probe.center_offset = wb_.end_offset() + probe.width * 0.5 + 1.0;
        assert!(!fits_at(&p.floors[0], host_ref, &probe, &[d]));
        probe.center_offset = wb_.end_offset() + probe.width * 0.5 + 2.0;
        assert!(fits_at(&p.floors[0], host_ref, &probe, &[d]));
        let wa = p.floors[0]
            .openings
            .iter()
            .find(|o| o.id == a)
            .unwrap()
            .clone();
        assert!(conflict(&wa, &{
            let mut w2 = wa.clone();
            w2.center_offset += width - 1.0;
            w2
        }));
    }
}
