//! Decks (CB-86, R-40): the Deck Specification of a room and the plan
//! geometry that planking, framing and the plan symbol share.
//!
//! A deck is a room whose [`RoomName::deck`] is set: its outline is the
//! room's interior polygon, the walls around it are Deck Edge or Deck
//! Railing walls, and a side that rests on the house (a wall of any other
//! class) carries the ledger. This module holds
//!
//! * [`DeckSpec`] with its three groups of options (planking, framing and
//!   stairs to grade), serde-default so older plans load,
//! * [`plank_layout`]: the decking boards, field and picture-frame border,
//! * [`joist_lines`], [`clip_line`]: the lines of joists clipped to the
//!   outline, used by `plan-framing`,
//! * [`ledger_edges`]: which edges of the outline lie against the house,
//! * [`deck_rooms`]: the deck rooms of a floor with their outlines.
//!
//! Lengths are inches; angles of the planking and joists are degrees
//! counter-clockwise from plan +x.

use crate::geometry::{point_in_polygon, polygon_area, Point};
use crate::model::{Floor, Id, RoomName, Wall};
use crate::rooms::{detect_rooms, Room};
use crate::walls::WallClass;
use serde::{Deserialize, Serialize};

/// Layer the decking boards are drawn on.
pub const DECK_LAYER: &str = "Decks";
/// Room type name that makes a room a deck.
pub const DECK_ROOM_TYPE: &str = "Deck";
/// Tolerance when matching walls to outline edges, inches.
const EDGE_TOL: f64 = 1.5;

/// Decking boards (Planking tab of the Deck Specification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeckPlanking {
    /// Build the boards. Off keeps the room's platform as a solid slab.
    pub enabled: bool,
    /// Board width, inches (5/4x6 decking is 5 1/2").
    pub board_width: f64,
    /// Board thickness, inches.
    pub board_thickness: f64,
    /// Gap between boards, inches.
    pub gap: f64,
    /// Direction the boards run, degrees from plan +x.
    pub angle: f64,
    /// Picture frame border: boards that run round the edge, mitred.
    pub border: bool,
    /// Rows of border boards.
    pub border_boards: u32,
    /// Boards overhang the rim by this much, inches.
    pub overhang: f64,
    /// Surface material name ("" = the Deck default).
    pub material: String,
    /// Border material ("" = same as the field).
    pub border_material: String,
}

impl Default for DeckPlanking {
    fn default() -> Self {
        Self {
            enabled: true,
            board_width: 5.5,
            board_thickness: 1.5,
            gap: 0.25,
            angle: 0.0,
            border: false,
            border_boards: 1,
            overhang: 0.0,
            material: "Decking".into(),
            border_material: String::new(),
        }
    }
}

/// Deck framing (Framing tab): joists, beams, posts with footings, ledger
/// and rim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeckFramingSpec {
    /// Build framing when the deck is framed (Build Framing > Deck).
    pub enabled: bool,
    /// Set by the build command: the deck members exist, so the 3D view
    /// does not draw its own skirt.
    pub built: bool,
    /// Joist section, e.g. `"2x8"`.
    pub joist_size: String,
    /// On-centre spacing of the joists, inches.
    pub joist_spacing: f64,
    /// Direction the joists run, degrees; `None` runs them away from the
    /// ledger (or along the shorter side without one).
    pub joist_angle: Option<f64>,
    /// Beam section and plies.
    pub beam_size: String,
    pub beam_plies: u32,
    /// Distance from the outer rim to the beam, inches (the cantilever).
    pub beam_setback: f64,
    /// Post section.
    pub post_size: String,
    /// Greatest distance between posts, inches.
    pub post_spacing: f64,
    /// Square footing under each post.
    pub footing_size: f64,
    pub footing_thickness: f64,
    /// Ledger on the sides that lie against the house.
    pub ledger: bool,
    /// Rim joists on the other sides.
    pub rim_joists: bool,
    /// Distance from the top of the deck to the ground, inches.
    pub height_above_grade: f64,
}

impl Default for DeckFramingSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            built: false,
            joist_size: "2x8".into(),
            joist_spacing: 16.0,
            joist_angle: None,
            beam_size: "2x10".into(),
            beam_plies: 2,
            beam_setback: 12.0,
            post_size: "6x6".into(),
            post_spacing: 96.0,
            footing_size: 18.0,
            footing_thickness: 12.0,
            ledger: true,
            rim_joists: true,
            height_above_grade: 36.0,
        }
    }
}

/// Stairs to grade (Stairs tab).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeckStairs {
    pub to_grade: bool,
    /// Index of the outline edge the stairs leave from (edge `i` runs from
    /// outline point `i` to `i + 1`).
    pub edge: usize,
    pub width: f64,
    /// Tread depth, inches.
    pub tread: f64,
    /// The stair object the last Build Deck Framing made, so a rebuild
    /// replaces it.
    pub stair_id: Option<Id>,
}

impl Default for DeckStairs {
    fn default() -> Self {
        Self {
            to_grade: false,
            edge: 0,
            width: 36.0,
            tread: 10.5,
            stair_id: None,
        }
    }
}

/// The Deck Specification of a room.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DeckSpec {
    pub planking: DeckPlanking,
    pub framing: DeckFramingSpec,
    pub stairs: DeckStairs,
}

impl DeckSpec {
    /// Total height of the decking: what sits on top of the joists.
    pub fn board_thickness(&self) -> f64 {
        self.planking.board_thickness.max(0.1)
    }

    /// Does the room draw its own decking in place of the platform slab?
    pub fn draws_boards(&self) -> bool {
        self.planking.enabled
    }
}

/// Is the room a deck: its type is Deck, or it has a Deck Specification?
pub fn is_deck(name: &RoomName) -> bool {
    name.deck.is_some() || name.room_type == DECK_ROOM_TYPE
}

/// A deck room of a floor: its outline and specification.
#[derive(Debug, Clone)]
pub struct DeckRoom {
    /// The room's name entry (anchor, floor height offset, ...).
    pub name: RoomName,
    pub spec: DeckSpec,
    /// Interior outline, counter-clockwise.
    pub outline: Vec<Point>,
    /// Centerline outline, counter-clockwise.
    pub centerline: Vec<Point>,
}

/// The deck rooms of `floor` that carry a Deck Specification, with their
/// outlines (the interior polygon of the detected room).
pub fn deck_rooms(floor: &Floor, rooms: &[Room]) -> Vec<DeckRoom> {
    let mut out = Vec::new();
    for room in rooms {
        let Some(name) = room.name_entry(&floor.room_names) else {
            continue;
        };
        let Some(spec) = name.deck.clone() else {
            continue;
        };
        let outline = if room.inner_polygon.len() >= 3 {
            room.inner_polygon.clone()
        } else {
            room.polygon.clone()
        };
        out.push(DeckRoom {
            name: name.clone(),
            spec,
            outline: ccw(&outline),
            centerline: ccw(&room.polygon),
        });
    }
    out
}

/// [`deck_rooms`] after detecting the rooms of `floor`.
pub fn deck_rooms_of(floor: &Floor) -> Vec<DeckRoom> {
    if floor.room_names.iter().all(|n| n.deck.is_none()) {
        return Vec::new();
    }
    deck_rooms(floor, &detect_rooms(&floor.walls, 0.5))
}

/// The polygon with counter-clockwise winding.
pub fn ccw(poly: &[Point]) -> Vec<Point> {
    let mut v = poly.to_vec();
    if polygon_area(&v) < 0.0 {
        v.reverse();
    }
    v
}

/// The wall classes that make the edge of a deck rather than a house wall.
fn is_deck_boundary(w: &Wall) -> bool {
    matches!(
        w.class,
        WallClass::DeckEdge | WallClass::DeckRailing | WallClass::Railing
    ) || w.is_deck_edge
        || w.flags.railing
        || w.flags.room_divider
        || w.flags.invisible
        || matches!(w.class, WallClass::RoomDivider)
}

/// The edges of `outline` (edge `i` runs from point `i` to `i + 1`) that lie
/// against a house wall: a wall that is neither a deck edge, a railing nor a
/// room divider within half its thickness of the edge.
pub fn ledger_edges(outline: &[Point], walls: &[Wall]) -> Vec<usize> {
    let n = outline.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let len = a.dist(b);
        if len < 6.0 {
            continue;
        }
        let dir = b.sub(a).normalized();
        let mid = Point::lerp(a, b, 0.5);
        let against = walls.iter().any(|w| {
            if is_deck_boundary(w) || w.length() < 6.0 {
                return false;
            }
            let wd = w.direction();
            if wd.cross(dir).abs() > 0.03 {
                return false;
            }
            let reach = w.thickness * 0.5 + EDGE_TOL;
            // The edge's middle lies within the wall's half thickness, and
            // along the wall's length.
            let rel = mid.sub(w.start);
            let along = rel.dot(wd);
            let across = rel.dot(wd.perp()).abs();
            across <= reach && along >= -EDGE_TOL && along <= w.length() + EDGE_TOL
        });
        if against {
            out.push(i);
        }
    }
    out
}

// ----- clipping lines to an outline -----

/// The parameter intervals `(t0, t1)` of the infinite line `p + t * dir`
/// (unit `dir`) that lie inside `outline` (any simple polygon).
/// A line that runs exactly along an edge is found from one side of the
/// polygon only, so callers keep their lines clear of the outline.
pub fn clip_line(outline: &[Point], p: Point, dir: Point) -> Vec<(f64, f64)> {
    let n = outline.len();
    if n < 3 {
        return Vec::new();
    }
    let normal = dir.perp();
    let mut ts: Vec<f64> = Vec::new();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let da = a.sub(p).dot(normal);
        let db = b.sub(p).dot(normal);
        // A crossing where the signed distances change sign; an edge lying
        // on the line contributes nothing (its ends do via the neighbours).
        if (da < 0.0) != (db < 0.0) && (da - db).abs() > 1e-12 {
            let s = da / (da - db);
            let q = Point::lerp(a, b, s);
            ts.push(q.sub(p).dot(dir));
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.as_chunks::<2>().0.iter().map(|c| (c[0], c[1])).collect()
}

/// A straight run inside the outline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run {
    pub a: Point,
    pub b: Point,
}

impl Run {
    pub fn length(&self) -> f64 {
        self.a.dist(self.b)
    }
}

/// Lines at `spacing` apart running along `angle` (degrees), clipped to the
/// outline: the first line is `first` from the outline's low side (across
/// the run), then every `spacing`. Each run is `(line index, run)`.
pub fn joist_lines(outline: &[Point], angle: f64, first: f64, spacing: f64) -> Vec<(usize, Run)> {
    let spacing = spacing.max(1.0);
    let (dir, across) = axes(angle);
    let Some((lo, hi)) = extent(outline, across) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut k = 0usize;
    let mut v = lo + first;
    while v <= hi + 1e-9 {
        let origin = across * v;
        for (t0, t1) in clip_line(outline, origin, dir) {
            if t1 - t0 > 1e-6 {
                out.push((
                    k,
                    Run {
                        a: origin + dir * t0,
                        b: origin + dir * t1,
                    },
                ));
            }
        }
        v += spacing;
        k += 1;
    }
    out
}

/// Unit run direction and its across direction for an angle in degrees.
pub fn axes(angle: f64) -> (Point, Point) {
    let a = angle.to_radians();
    let dir = Point::new(a.cos(), a.sin());
    (dir, dir.perp())
}

/// The range of `outline` along the `across` axis.
pub fn extent(outline: &[Point], across: Point) -> Option<(f64, f64)> {
    if outline.is_empty() {
        return None;
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in outline {
        let v = p.dot(across);
        lo = lo.min(v);
        hi = hi.max(v);
    }
    Some((lo, hi))
}

/// The polygon moved inward by `d` (outward when negative): each edge moves
/// along its normal and neighbouring edges meet at their intersection.
/// `None` when the result collapses (zero area or turned inside out).
pub fn offset_polygon(outline: &[Point], d: f64) -> Option<Vec<Point>> {
    let poly = ccw(outline);
    let n = poly.len();
    if n < 3 {
        return None;
    }
    if d.abs() < 1e-9 {
        return (polygon_area(&poly) > 1e-6).then_some(poly);
    }
    // Inward normal of a counter-clockwise edge is its left side.
    let lines: Vec<(Point, Point)> = (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let dir = b.sub(a).normalized();
            let off = dir.perp() * d;
            (a + off, dir)
        })
        .collect();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (p0, d0) = lines[(i + n - 1) % n];
        let (p1, d1) = lines[i];
        let denom = d0.cross(d1);
        if denom.abs() < 1e-9 {
            out.push(p1);
            continue;
        }
        let t = p1.sub(p0).cross(d1) / denom;
        out.push(p0 + d0 * t);
    }
    let area0 = polygon_area(&poly);
    let area1 = polygon_area(&out);
    let ok = area1 > 1e-6 && ((d >= 0.0 && area1 < area0) || (d < 0.0 && area1 > area0));
    ok.then_some(out)
}

// ----- planking -----

/// One decking board: a quadrilateral (a mitred border board is not a
/// rectangle).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plank {
    pub corners: [Point; 4],
    /// Part of the picture frame border.
    pub border: bool,
}

impl Plank {
    /// Length of the board along its long side.
    pub fn length(&self) -> f64 {
        let c = &self.corners;
        c[0].dist(c[1]).max(c[1].dist(c[2]))
    }

    /// Area, square inches.
    pub fn area(&self) -> f64 {
        polygon_area(&self.corners).abs()
    }
}

/// The boards of a deck.
#[derive(Debug, Clone, Default)]
pub struct PlankLayout {
    pub planks: Vec<Plank>,
}

impl PlankLayout {
    pub fn border(&self) -> impl Iterator<Item = &Plank> {
        self.planks.iter().filter(|p| p.border)
    }

    pub fn field(&self) -> impl Iterator<Item = &Plank> {
        self.planks.iter().filter(|p| !p.border)
    }

    /// Board feet of decking: area times thickness over 144.
    pub fn board_feet(&self, thickness: f64) -> f64 {
        self.planks.iter().map(Plank::area).sum::<f64>() * thickness / 12.0 / 12.0
    }
}

/// Lays the decking boards of `outline`: the picture-frame border first
/// (when set), then field boards running along `planking.angle` inside what
/// the border leaves, each `board_width` wide with `gap` between.
pub fn plank_layout(outline: &[Point], planking: &DeckPlanking) -> PlankLayout {
    let mut layout = PlankLayout::default();
    if !planking.enabled || outline.len() < 3 {
        return layout;
    }
    let width = planking.board_width.max(0.5);
    let pitch = width + planking.gap.max(0.0);
    let base = ccw(outline);
    // Boards overhang the rim.
    let base = if planking.overhang > 0.0 {
        offset_polygon(&base, -planking.overhang).unwrap_or(base)
    } else {
        base
    };
    let mut field = base.clone();
    if planking.border {
        let rows = planking.border_boards.clamp(1, 4);
        for r in 0..rows {
            let d0 = f64::from(r) * pitch;
            let (Some(outer), Some(inner)) =
                (offset_polygon(&base, d0), offset_polygon(&base, d0 + width))
            else {
                break;
            };
            let n = outer.len().min(inner.len());
            for i in 0..n {
                let j = (i + 1) % n;
                layout.planks.push(Plank {
                    corners: [outer[i], outer[j], inner[j], inner[i]],
                    border: true,
                });
            }
            // The field starts one gap inside the last row.
            if r + 1 == rows {
                match offset_polygon(&base, d0 + pitch) {
                    Some(f) => field = f,
                    None => return layout,
                }
            }
        }
    }
    let (dir, across) = axes(planking.angle);
    let Some((lo, hi)) = extent(&field, across) else {
        return layout;
    };
    let mut v = lo;
    while v + 1e-6 < hi {
        let top = (v + width).min(hi);
        // Intersect the intervals of the strip's two edges.
        let first = clip_line(&field, across * v, dir);
        let second = clip_line(&field, across * top, dir);
        let mid = clip_line(&field, across * ((v + top) * 0.5), dir);
        for (m0, m1) in mid {
            // Trim to the interval of each edge that overlaps the middle one.
            let mut t0 = m0;
            let mut t1 = m1;
            for edge in [&first, &second] {
                if let Some(&(e0, e1)) = edge.iter().find(|(e0, e1)| *e0 < m1 && *e1 > m0) {
                    t0 = t0.max(e0);
                    t1 = t1.min(e1);
                }
            }
            if t1 - t0 < 2.0 || top - v < 0.5 {
                continue;
            }
            let a = across * v + dir * t0;
            let b = across * v + dir * t1;
            let c = across * top + dir * t1;
            let d = across * top + dir * t0;
            layout.planks.push(Plank {
                corners: [a, b, c, d],
                border: false,
            });
        }
        v += pitch;
    }
    layout
}

/// The direction the joists run: the spec's, else away from the longest
/// ledger edge (perpendicular to it), else across the shorter side.
pub fn joist_angle(outline: &[Point], ledger: &[usize], spec: &DeckFramingSpec) -> f64 {
    if let Some(a) = spec.joist_angle {
        return a;
    }
    let n = outline.len();
    if n < 3 {
        return 0.0;
    }
    let longest = ledger.iter().copied().filter(|i| *i < n).max_by(|a, b| {
        let la = outline[*a].dist(outline[(*a + 1) % n]);
        let lb = outline[*b].dist(outline[(*b + 1) % n]);
        la.total_cmp(&lb)
    });
    if let Some(i) = longest {
        let d = outline[(i + 1) % n].sub(outline[i]).normalized();
        return (d.perp().angle().to_degrees() + 360.0) % 180.0;
    }
    // Along the side with less extent: joists span the shorter way.
    let w = extent(outline, Point::new(1.0, 0.0)).map_or(0.0, |(l, h)| h - l);
    let h = extent(outline, Point::new(0.0, 1.0)).map_or(0.0, |(l, h)| h - l);
    if w <= h {
        0.0
    } else {
        90.0
    }
}

/// Actual size of a nominal lumber dimension, inches (2 is 1 1/2, 8 is 7 1/4).
pub fn actual_size(nominal: u32) -> f64 {
    match nominal {
        1 => 0.75,
        2 => 1.5,
        3 => 2.5,
        4 => 3.5,
        5 => 4.5,
        6 => 5.5,
        8 => 7.25,
        10 => 9.25,
        12 => 11.25,
        14 => 13.25,
        16 => 15.25,
        n => f64::from(n) - 0.5,
    }
}

/// `(thickness, depth)` of a nominal size such as `"2x8"` or `"4 x 10"`.
pub fn parse_size(size: &str) -> Option<(u32, u32)> {
    let mut it = size.split(['x', 'X']).map(|t| t.trim().parse::<u32>().ok());
    let (a, b) = (it.next()??, it.next()??);
    (it.next().is_none() && a > 0 && b > 0).then_some((a, b))
}

/// Actual depth of the joists of `spec`, inches (7 1/4" when the size does
/// not parse).
pub fn joist_depth(spec: &DeckFramingSpec) -> f64 {
    parse_size(&spec.joist_size).map_or(7.25, |(_, d)| actual_size(d))
}

/// Does `p` lie on the deck?
pub fn on_deck(outline: &[Point], p: Point) -> bool {
    point_in_polygon(p, outline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WallKind;

    fn rect(w: f64, h: f64) -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, h),
            Point::new(0.0, h),
        ]
    }

    #[test]
    fn a_line_is_clipped_to_a_rectangle() {
        let r = rect(120.0, 96.0);
        let runs = clip_line(&r, Point::new(0.0, 40.0), Point::new(1.0, 0.0));
        assert_eq!(runs.len(), 1);
        assert!((runs[0].0 - 0.0).abs() < 1e-9 && (runs[0].1 - 120.0).abs() < 1e-9);
        assert!(clip_line(&r, Point::new(0.0, 200.0), Point::new(1.0, 0.0)).is_empty());
    }

    #[test]
    fn a_line_through_an_l_shape_gives_two_runs() {
        let l = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 40.0),
            Point::new(60.0, 40.0),
            Point::new(60.0, 100.0),
            Point::new(0.0, 100.0),
        ];
        // Across the notch at y = 70: only the left leg.
        let runs = clip_line(&l, Point::new(0.0, 70.0), Point::new(1.0, 0.0));
        assert_eq!(runs.len(), 1);
        assert!((runs[0].1 - 60.0).abs() < 1e-9);
        // A vertical line at x = 80 crosses only the bottom arm.
        let v = clip_line(&l, Point::new(80.0, 0.0), Point::new(0.0, 1.0));
        assert_eq!(v.len(), 1);
        assert!((v[0].1 - 40.0).abs() < 1e-9);
    }

    #[test]
    fn joists_are_sixteen_inches_on_centre() {
        let r = rect(192.0, 144.0);
        let lines = joist_lines(&r, 90.0, 8.0, 16.0);
        // 8 in from the low side, then every 16 in: 8, 24, ... 184.
        assert_eq!(lines.len(), 12);
        assert!(lines
            .iter()
            .all(|(_, run)| (run.length() - 144.0).abs() < 1e-6));
        let mut xs: Vec<f64> = lines.iter().map(|(_, r)| r.a.x).collect();
        xs.sort_by(f64::total_cmp);
        assert!((xs[0] - 8.0).abs() < 1e-6);
        assert!(xs.windows(2).all(|w| (w[1] - w[0] - 16.0).abs() < 1e-6));
    }

    #[test]
    fn planks_cover_the_deck_with_gaps() {
        let r = rect(120.0, 96.0);
        let p = DeckPlanking::default();
        let l = plank_layout(&r, &p);
        // 96 / (5.5 + 0.25) = 16.7: 17 boards, the last one trimmed.
        assert_eq!(l.field().count(), 17);
        assert_eq!(l.border().count(), 0);
        let covered: f64 = l.planks.iter().map(Plank::area).sum();
        assert!(covered < 120.0 * 96.0 && covered > 120.0 * 96.0 * 0.93);
        assert!(l.planks.iter().all(|pl| (pl.length() - 120.0).abs() < 1e-6));
    }

    #[test]
    fn boards_turn_with_the_angle() {
        let r = rect(120.0, 96.0);
        let p = DeckPlanking {
            angle: 90.0,
            ..DeckPlanking::default()
        };
        let l = plank_layout(&r, &p);
        assert_eq!(l.field().count(), 21);
        assert!(l.planks.iter().all(|pl| (pl.length() - 96.0).abs() < 1e-6));
    }

    #[test]
    fn a_picture_frame_border_is_four_mitred_boards() {
        let r = rect(120.0, 96.0);
        let p = DeckPlanking {
            border: true,
            ..DeckPlanking::default()
        };
        let l = plank_layout(&r, &p);
        assert_eq!(l.border().count(), 4);
        // The first border board runs the whole bottom edge, mitred: its outer
        // edge is the deck edge, 120" long, and its inner edge is 109" long.
        let b = l.border().next().unwrap();
        assert!((b.corners[0].dist(b.corners[1]) - 120.0).abs() < 1e-6);
        assert!((b.corners[3].dist(b.corners[2]) - 109.0).abs() < 1e-6);
        // The field is smaller than without the border.
        let plain = plank_layout(&r, &DeckPlanking::default());
        assert!(l.field().count() < plain.field().count());
        let field_area: f64 = l.field().map(Plank::area).sum();
        assert!(field_area < 109.0 * 85.0);
    }

    #[test]
    fn offsetting_a_polygon_shrinks_it_and_collapses() {
        let r = rect(100.0, 60.0);
        let o = offset_polygon(&r, 10.0).unwrap();
        assert!((polygon_area(&o) - 80.0 * 40.0).abs() < 1e-6);
        assert!(offset_polygon(&r, 40.0).is_none());
        let out = offset_polygon(&r, -5.0).unwrap();
        assert!((polygon_area(&out) - 110.0 * 70.0).abs() < 1e-6);
    }

    #[test]
    fn the_ledger_is_the_edge_against_a_house_wall() {
        let outline = rect(144.0, 96.0);
        let mut house = Wall::new(
            Point::new(-20.0, 0.0),
            Point::new(180.0, 0.0),
            6.5,
            109.0,
            WallKind::Exterior,
        );
        house.id = 1;
        let mut edge = Wall::new(
            Point::new(144.0, 0.0),
            Point::new(144.0, 96.0),
            9.25,
            36.0,
            WallKind::Exterior,
        );
        edge.id = 2;
        edge.class = WallClass::DeckEdge;
        edge.is_deck_edge = true;
        // The house wall's centerline lies half a thickness outside the deck.
        house.start = Point::new(-20.0, -3.25);
        house.end = Point::new(180.0, -3.25);
        let ledger = ledger_edges(&outline, &[house, edge]);
        assert_eq!(ledger, vec![0]);
    }

    #[test]
    fn joists_run_away_from_the_ledger() {
        let outline = rect(144.0, 96.0);
        let spec = DeckFramingSpec::default();
        // Ledger on the bottom edge (runs along x): joists run along y.
        assert!((joist_angle(&outline, &[0], &spec) - 90.0).abs() < 1e-9);
        // No ledger: across the shorter side (the 96" y side), so along y.
        assert!((joist_angle(&outline, &[], &spec) - 90.0).abs() < 1e-9);
        let fixed = DeckFramingSpec {
            joist_angle: Some(0.0),
            ..DeckFramingSpec::default()
        };
        assert_eq!(joist_angle(&outline, &[0], &fixed), 0.0);
    }

    #[test]
    fn lumber_sizes_parse_to_actual_dimensions() {
        assert_eq!(parse_size("2x8"), Some((2, 8)));
        assert_eq!(parse_size(" 4 x 10 "), Some((4, 10)));
        assert_eq!(parse_size("2x"), None);
        assert_eq!(parse_size("2x8x1"), None);
        assert_eq!(parse_size("garbage"), None);
        assert_eq!(actual_size(8), 7.25);
        assert_eq!(joist_depth(&DeckFramingSpec::default()), 7.25);
        let odd = DeckFramingSpec {
            joist_size: "??".into(),
            ..DeckFramingSpec::default()
        };
        assert_eq!(joist_depth(&odd), 7.25);
    }

    #[test]
    fn a_spec_round_trips_and_old_files_load_with_defaults() {
        let spec = DeckSpec {
            planking: DeckPlanking {
                angle: 45.0,
                border: true,
                ..DeckPlanking::default()
            },
            ..DeckSpec::default()
        };
        let json = serde_json::to_string(&spec).unwrap();
        let back: DeckSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
        let sparse: DeckSpec = serde_json::from_str(r#"{"planking":{"gap":0.5}}"#).unwrap();
        assert_eq!(sparse.planking.gap, 0.5);
        assert_eq!(sparse.framing.joist_spacing, 16.0);
    }
}
