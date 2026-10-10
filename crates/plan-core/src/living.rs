//! Standard Area, Living Area, structures, the Exterior Room and room
//! polylines (R-106, R-108, R-109, R-110; manual pp. 449 to 455, 463 to 464).
//!
//! A *structure* is a group of rooms that touch each other (a house, a
//! detached garage); each structure on each floor has an Exterior Room whose
//! label reports the living area inside it. The Living Area is measured like
//! the Standard Area: to the center of the interior walls and to the outside
//! surface of the exterior walls (or the outside of their Main Layer, the
//! `Living Area to` setting of General Plan Defaults), rounded to the nearest
//! square foot, without bay, box and bow windows. The Interior Area, in
//! contrast, takes the inner wall surfaces and includes those windows.

use crate::clip::{boolean_all, BoolOp, Ring};
use crate::defaults::{RoomTypeDef, WallTypeDef};
use crate::extras::StructureLayer;
use crate::geometry::{point_in_polygon, polygon_area, Point};
use crate::model::{Floor, Id, RoomName, Wall, WallKind};
use crate::openings::OpeningStyle;
use crate::rooms::{edge_owner, expand_curves, offset_polygon, Room, WallsNear};
use crate::units::sq_in_to_sq_ft;
use crate::walls::spec_tabs::SideCovering;
use crate::walls::Side;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A rough ceiling lower than this keeps a room out of the Living Area by
/// default (manual p. 454; a basement of this height or more counts).
pub const MIN_LIVING_CEILING: f64 = 48.0;

/// Key under which General Plan Defaults keeps the `Living Area to` choice.
pub const LIVING_TO_KEY: &str = "general.living_area_to";
/// Key of the `Show Living Area Label` switch of General Plan Defaults.
pub const SHOW_LIVING_LABEL_KEY: &str = "general.show_living_label";

/// How far below a structure its Living Area label sits, inches.
pub const LIVING_LABEL_DROP: f64 = 30.0;

/// The `Living Area to` setting of General Plan Defaults: where the Standard
/// and Living Areas end at an exterior wall (manual p. 453).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LivingTo {
    /// The outside surface of the wall.
    #[default]
    OutsideSurface,
    /// The outer surface of the wall's Main Layer.
    MainLayer,
}

impl LivingTo {
    pub const ALL: [LivingTo; 2] = [LivingTo::OutsideSurface, LivingTo::MainLayer];

    pub fn name(self) -> &'static str {
        match self {
            LivingTo::OutsideSurface => "Outside Surface",
            LivingTo::MainLayer => "Outside of Main Layer",
        }
    }

    pub fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|l| l.name() == name)
            .unwrap_or_default()
    }
}

/// Rounds square inches to the nearest whole square foot (Standard Area and
/// Living Area, manual p. 453).
pub fn round_sq_ft(sq_in: f64) -> f64 {
    sq_in_to_sq_ft(sq_in).round()
}

/// The walls room detection sees: those that define rooms, curved walls
/// faceted. Edge owners of a room's polygon are found among them.
pub fn defining_walls(walls: &[Wall]) -> Vec<Wall> {
    expand_curves(walls)
        .into_iter()
        .filter(|w| w.defines_rooms())
        .collect()
}

/// How far `wall` lies outside its centerline at the outer boundary of the
/// Standard Area: the outside surface, or the outer face of its Main Layer
/// when the wall's type has one and the wall's exterior side faces away from
/// the room (`edge` runs counter-clockwise around the room).
fn outward_reach(
    wall: &Wall,
    edge: (Point, Point),
    wall_types: &[WallTypeDef],
    to: LivingTo,
) -> f64 {
    let half = wall.thickness * 0.5;
    if to == LivingTo::OutsideSurface {
        return half;
    }
    let Some(ty) = wall
        .wall_type
        .as_ref()
        .and_then(|n| wall_types.iter().find(|t| &t.name == n))
    else {
        return half;
    };
    if !ty.layers.iter().any(|l| l.is_main) {
        return half;
    }
    // The room is to the left of the edge; the wall's exterior faces out of
    // the room when it is the right side of the edge's direction.
    let along = edge.1.sub(edge.0);
    let same = wall.direction().dot(along) >= 0.0;
    let exterior_is_right_of_edge = match (wall.exterior_side, same) {
        (Side::Left, true) | (Side::Right, false) => false,
        (Side::Right, true) | (Side::Left, false) => true,
    };
    if !exterior_is_right_of_edge {
        return half;
    }
    // Layers run from the exterior face; the types' thickness may differ
    // from the wall's, so scale the offset to the wall.
    let total = ty.thickness();
    let scale = if total > 1e-9 {
        wall.thickness / total
    } else {
        1.0
    };
    (half - ty.main_layer_offset() * scale).max(0.0)
}

/// The distance each edge of `room`'s polygon lies outside the centerline at
/// the Standard Area boundary: the outward reach of an exterior wall, zero
/// at an interior wall (the center) or where no wall owns the edge.
pub fn standard_edge_offsets(
    room: &Room,
    defining: &[Wall],
    wall_types: &[WallTypeDef],
    to: LivingTo,
) -> Vec<f64> {
    let n = room.polygon.len();
    let near = WallsNear::new(defining, 0.5, true);
    (0..n)
        .map(|i| {
            let (a, b) = (room.polygon[i], room.polygon[(i + 1) % n]);
            match edge_owner(defining, &near, a, b, 0.5) {
                Some(w) if w.kind == WallKind::Exterior => outward_reach(w, (a, b), wall_types, to),
                _ => 0.0,
            }
        })
        .collect()
}

/// The Standard Area outline of `room`: to the center of interior walls and
/// to the outside surface (or the outside of the Main Layer) of exterior
/// walls.
pub fn standard_polygon(
    room: &Room,
    defining: &[Wall],
    wall_types: &[WallTypeDef],
    to: LivingTo,
) -> Vec<Point> {
    let d: Vec<f64> = standard_edge_offsets(room, defining, wall_types, to)
        .into_iter()
        .map(|o| -o)
        .collect();
    offset_polygon(&room.polygon, &d).unwrap_or_else(|| room.polygon.clone())
}

/// The Standard Area of `room`, square inches, less what its nested rooms
/// take out (their outer wall surfaces).
pub fn standard_area_sq_in(
    room: &Room,
    defining: &[Wall],
    wall_types: &[WallTypeDef],
    to: LivingTo,
) -> f64 {
    let outline = standard_polygon(room, defining, wall_types, to);
    let own = polygon_area(&outline);
    // A nested room (a closet pod) is a room of its own: the area around
    // it gives up what its centerline polygon covers (as detection does).
    let taken: f64 = room.holes.iter().map(|h| polygon_area(h)).sum();
    (own - taken).max(0.0)
}

/// The area of the bay, box and bow windows that project from `room`, square
/// inches: the Interior Area includes the space inside them, the Standard
/// Area and Interior Dimensions do not (manual p. 453).
pub fn bay_area_sq_in(floor: &Floor, room: &Room, defining: &[Wall]) -> f64 {
    let n = room.polygon.len();
    if n < 3 {
        return 0.0;
    }
    let near = WallsNear::new(defining, 0.5, true);
    let mut total = 0.0;
    for o in &floor.openings {
        if !matches!(
            o.style,
            OpeningStyle::BayWindow | OpeningStyle::BowWindow | OpeningStyle::BoxWindow
        ) {
            continue;
        }
        let Some(wall) = floor.walls.iter().find(|w| w.id == o.wall_id) else {
            continue;
        };
        let at = wall.point_at(o.center_offset);
        // The edge of the room this window sits on, when its projection
        // leaves the room.
        for i in 0..n {
            let (a, b) = (room.polygon[i], room.polygon[(i + 1) % n]);
            if crate::geometry::dist_to_segment(at, a, b) > wall.thickness.max(1.0) {
                continue;
            }
            if edge_owner(defining, &near, a, b, 0.5).is_none_or(|w| w.id != wall.id) {
                continue;
            }
            let exterior_sign = if wall.exterior_side == Side::Left {
                1.0
            } else {
                -1.0
            };
            let sign = if o.swing_flipped {
                -exterior_sign
            } else {
                exterior_sign
            };
            let proj = wall.normal().scale(sign);
            let out_of_room = b.sub(a).normalized().perp().scale(-1.0);
            if proj.dot(out_of_room) <= 0.0 {
                continue;
            }
            let outline =
                crate::opening_symbol::projection_footprint(o.style, 0.0, o.width, 0.0, 1.0);
            let pts: Vec<Point> = outline.iter().map(|&(s, t)| Point::new(s, t)).collect();
            total += polygon_area(&pts).abs() + o.width * wall.thickness;
            break;
        }
    }
    total
}

/// The Interior Area of `room` including the bay, box and bow windows that
/// project from it, square inches.
pub fn interior_area_with_bays_sq_in(floor: &Floor, room: &Room, defining: &[Wall]) -> f64 {
    room.interior_area_sq_in + bay_area_sq_in(floor, room, defining)
}

// ----- structures -----

/// A group of rooms that touch (share a wall edge or a corner): one building.
#[derive(Debug, Clone, PartialEq)]
pub struct Structure {
    /// Indexes into the floor's detected rooms.
    pub rooms: Vec<usize>,
    /// The outer boundary along the wall centerlines, counter-clockwise.
    pub outline: Vec<Point>,
    /// A point inside the structure's largest room: what stands for the
    /// structure (an Exterior Room finds its structure by it).
    pub anchor: Point,
}

fn key(p: Point) -> (i64, i64) {
    ((p.x * 10.0).round() as i64, (p.y * 10.0).round() as i64)
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// A point inside `room` proper (not in a nested room): the centroid, else
/// the centre of an interior triangle of the outline.
pub fn room_inside_point(room: &Room) -> Point {
    if room.contains(room.centroid) {
        return room.centroid;
    }
    let poly = &room.polygon;
    let n = poly.len();
    for i in 0..n {
        let (a, b, c) = (poly[i], poly[(i + 1) % n], poly[(i + 2) % n]);
        let t = Point::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0);
        if room.contains(t) {
            return t;
        }
    }
    room.centroid
}

/// The structures of a floor's rooms, in the order of their first room.
pub fn structures(rooms: &[Room]) -> Vec<Structure> {
    let n = rooms.len();
    let mut parent: Vec<usize> = (0..n).collect();
    // Rooms that share a corner (and so an edge or a point of a wall).
    let mut at_vertex: HashMap<(i64, i64), usize> = HashMap::new();
    for (i, r) in rooms.iter().enumerate() {
        for p in &r.polygon {
            match at_vertex.get(&key(*p)) {
                Some(&j) => {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    parent[a] = b;
                }
                None => {
                    at_vertex.insert(key(*p), i);
                }
            }
        }
    }
    // A room inside another's hole belongs to the structure around it.
    for i in 0..n {
        for j in 0..n {
            if i != j
                && rooms[i]
                    .holes
                    .iter()
                    .any(|h| point_in_polygon(rooms[j].centroid, h))
            {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        match groups.iter_mut().find(|(r, _)| *r == root) {
            Some((_, v)) => v.push(i),
            None => groups.push((root, vec![i])),
        }
    }
    groups
        .into_iter()
        .map(|(_, members)| {
            let outline = outer_loop(rooms, &members);
            let biggest = members
                .iter()
                .copied()
                .max_by(|&a, &b| rooms[a].area_sq_in.total_cmp(&rooms[b].area_sq_in))
                .unwrap_or(members[0]);
            Structure {
                anchor: room_inside_point(&rooms[biggest]),
                rooms: members,
                outline,
            }
        })
        .collect()
}

/// The outer boundary loop of the rooms `members`: edges shared by two rooms
/// run both ways and cancel; the rest chain into loops, of which the largest
/// is the outline.
fn outer_loop(rooms: &[Room], members: &[usize]) -> Vec<Point> {
    type K = (i64, i64);
    let k = |p: Point| -> K { ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64) };
    let mut edges: HashMap<(K, K), (Point, Point)> = HashMap::new();
    for &m in members {
        let poly = &rooms[m].polygon;
        let n = poly.len();
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            if k(a) != k(b) {
                edges.insert((k(a), k(b)), (a, b));
            }
        }
    }
    let outer: Vec<(K, (Point, Point))> = edges
        .iter()
        .filter(|((a, b), _)| !edges.contains_key(&(*b, *a)))
        .map(|(key, v)| (key.0, *v))
        .collect();
    let mut next: HashMap<K, Vec<Point>> = HashMap::new();
    let mut origin: HashMap<K, Point> = HashMap::new();
    let mut starts: Vec<K> = Vec::new();
    for (a, (pa, pb)) in &outer {
        next.entry(*a).or_default().push(*pb);
        origin.insert(*a, *pa);
        starts.push(*a);
    }
    starts.sort_unstable();
    starts.dedup();
    let mut best: Vec<Point> = Vec::new();
    let mut best_area = 0.0;
    for s in starts {
        while let Some(first) = next.get_mut(&s).and_then(Vec::pop) {
            let mut pts = vec![origin[&s]];
            let mut cur = first;
            let mut guard = 0;
            while k(cur) != s && guard < 100_000 {
                pts.push(cur);
                match next.get_mut(&k(cur)).and_then(Vec::pop) {
                    Some(p) => cur = p,
                    None => break,
                }
                guard += 1;
            }
            let a = polygon_area(&pts).abs();
            if a > best_area {
                best_area = a;
                best = pts;
            }
        }
    }
    if polygon_area(&best) < 0.0 {
        best.reverse();
    }
    drop_collinear(best)
}

fn drop_collinear(pts: Vec<Point>) -> Vec<Point> {
    let n = pts.len();
    if n < 3 {
        return pts;
    }
    (0..n)
        .filter(|&i| {
            let (p, c, q) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            c.sub(p).cross(q.sub(c)).abs() > 1e-6 * c.sub(p).length().max(1.0)
        })
        .map(|i| pts[i])
        .collect()
}

/// Is `p` just outside an exterior wall of the structure: within `reach`
/// inches of its outline and not in a room? (Click outside an exterior wall
/// to select its Exterior Room, manual p. 449.)
pub fn near_outline(outline: &[Point], p: Point, reach: f64) -> bool {
    let n = outline.len();
    n >= 3
        && (0..n)
            .any(|i| crate::geometry::dist_to_segment(p, outline[i], outline[(i + 1) % n]) <= reach)
}

// ----- the Living Area -----

/// The rough ceiling of a room: its own, else the ceiling height it
/// resolves to plus the finish on it. A room on a foundation floor with a
/// platform above it has the clear height under that platform.
pub fn rough_ceiling(floor: &Floor, entry: Option<&RoomName>) -> f64 {
    if let Some(r) = entry.and_then(|n| n.rough_ceiling) {
        return r;
    }
    let offset = entry.map_or(0.0, |n| n.floor_height_offset);
    let height = entry.and_then(|n| n.ceiling_height).unwrap_or_else(|| {
        if floor.kind == crate::floors::FloorKind::Foundation
            && floor.settings.ceiling_structure_thickness > 0.0
        {
            (floor.ceiling_height - floor.settings.ceiling_structure_thickness - offset).max(1.0)
        } else {
            floor.ceiling_height
        }
    });
    let finish = entry
        .and_then(|n| n.misc.as_ref())
        .map_or(floor.settings.ceiling_finish_thickness, |m| {
            m.ceiling_finish_thickness
        });
    height + finish
}

/// Does `room` count toward the Living Area (manual pp. 454 and 455)? Its
/// own Living Area setting (Include / Exclude) wins; else its Room Type's
/// (unknown types count as interior); and a rough ceiling lower than
/// [`MIN_LIVING_CEILING`] keeps it out by default, while a basement of that
/// height or more counts.
pub fn room_in_living_area(floor: &Floor, room: &Room, room_types: &[RoomTypeDef]) -> bool {
    let entry = room.name_entry(&floor.room_names);
    if let Some(over) = entry.and_then(|n| n.include_in_living_area) {
        return over;
    }
    let type_default = entry
        .and_then(|n| room_types.iter().find(|t| t.name == n.room_type))
        .is_none_or(|t| t.include_in_living_area);
    type_default && rough_ceiling(floor, entry) >= MIN_LIVING_CEILING - 1e-9
}

/// The Living Area of one structure of a floor.
#[derive(Debug, Clone, PartialEq)]
pub struct LivingArea {
    /// Index into [`structures`]' result.
    pub structure: usize,
    /// The rooms counted (indexes into the floor's rooms).
    pub rooms: Vec<usize>,
    /// Square inches, from the Standard Areas of the counted rooms.
    pub area_sq_in: f64,
    /// Where the label sits: centred under the structure.
    pub label_at: Point,
}

impl LivingArea {
    /// The area to the nearest square foot.
    pub fn sq_ft(&self) -> f64 {
        round_sq_ft(self.area_sq_in)
    }

    /// The automatic label text.
    pub fn text(&self) -> String {
        format!(
            "Living Area: {} sq ft",
            group_thousands(self.sq_ft() as i64)
        )
    }
}

/// 1820 as "1,820".
pub fn group_thousands(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// The Living Area of each structure of `floor` that has a counted room
/// (a structure with none has no label). `rooms` are the floor's detected
/// rooms.
pub fn living_areas(
    floor: &Floor,
    rooms: &[Room],
    room_types: &[RoomTypeDef],
    wall_types: &[WallTypeDef],
    to: LivingTo,
) -> Vec<LivingArea> {
    let defining = defining_walls(&floor.walls);
    structures(rooms)
        .into_iter()
        .enumerate()
        .filter_map(|(si, s)| {
            let counted: Vec<usize> = s
                .rooms
                .iter()
                .copied()
                .filter(|&i| room_in_living_area(floor, &rooms[i], room_types))
                .collect();
            if counted.is_empty() {
                return None;
            }
            let area: f64 = counted
                .iter()
                .map(|&i| standard_area_sq_in(&rooms[i], &defining, wall_types, to))
                .sum();
            // A structure whose boundary does not close has no place for a label.
            let first = s.outline.first().copied()?;
            let (mut lo, mut hi) = (first, first);
            for p in &s.outline {
                lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
            }
            Some(LivingArea {
                structure: si,
                rooms: counted,
                area_sq_in: area,
                label_at: Point::new((lo.x + hi.x) * 0.5, lo.y - LIVING_LABEL_DROP),
            })
        })
        .collect()
}

/// The Living Area of `floor` in square feet, rounded per structure and
/// summed.
pub fn floor_living_sq_ft(
    floor: &Floor,
    rooms: &[Room],
    room_types: &[RoomTypeDef],
    wall_types: &[WallTypeDef],
    to: LivingTo,
) -> f64 {
    living_areas(floor, rooms, room_types, wall_types, to)
        .iter()
        .map(LivingArea::sq_ft)
        .sum()
}

// ----- heights of the Structure panel (R-115) -----

/// The absolute and relative heights the Structure panel reports for a room
/// (manual pp. 467 and 468): measured from zero, the top of the subfloor of
/// Floor 1, or from surfaces in the room or the room below.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoomHeights {
    /// Top of this room's subfloor.
    pub floor_abs: f64,
    /// Bottom of this room's ceiling framing.
    pub ceiling_abs: f64,
    /// Subfloor height of the room directly above (`None`: no floor above).
    pub floor_above: Option<f64>,
    /// Several rooms with different floor heights lie above ("No Change").
    pub mixed_above: bool,
    /// Subfloor height of the room directly below.
    pub floor_below: Option<f64>,
    /// Top of the stem walls around a room below that has them.
    pub swt_below: Option<f64>,
    /// Subfloor to the bottom of the ceiling framing.
    pub rough: f64,
    /// Finished floor surface to the finished ceiling surface.
    pub finished: f64,
    /// The ceiling of the room below, subfloor to rough ceiling.
    pub ceiling_below: Option<f64>,
    /// Stem Wall Top to Ceiling (when the foundation supplies the floor).
    pub stem_wall_top_to_ceiling: Option<f64>,
    /// Floor to Stem Wall Top (when the foundation supplies the floor).
    pub floor_to_stem_wall_top: Option<f64>,
}

/// The heights of the room of `floor` holding `at` whose name entry is
/// `entry` (a room without one has the floor's defaults).
pub fn room_heights(
    project: &crate::model::Project,
    floor: usize,
    entry: Option<&RoomName>,
    at: Point,
) -> RoomHeights {
    let f = &project.floors[floor];
    let offset = entry.map_or(0.0, |n| n.floor_height_offset);
    let rough = rough_ceiling(f, entry);
    let finished = entry
        .and_then(|n| n.ceiling_height)
        .unwrap_or(rough - f.settings.ceiling_finish_thickness)
        .max(0.0);
    let floor_abs = f.elevation + offset;
    let mut h = RoomHeights {
        floor_abs,
        ceiling_abs: floor_abs + rough,
        rough,
        finished,
        ..RoomHeights::default()
    };
    // The room above and below: where `at` falls on the neighbouring floors.
    let near = |g: usize| -> Vec<(f64, Option<&RoomName>, usize)> {
        let rooms = crate::rooms::detect_rooms(&project.floors[g].walls, 0.5);
        rooms
            .iter()
            .filter(|r| r.contains(at))
            .map(|r| {
                let e = r.name_entry(&project.floors[g].room_names);
                (
                    project.floors[g].elevation + e.map_or(0.0, |n| n.floor_height_offset),
                    e,
                    g,
                )
            })
            .collect()
    };
    if floor + 1 < project.floors.len() {
        let above = near(floor + 1);
        let mut levels: Vec<f64> = above.iter().map(|a| a.0).collect();
        levels.sort_by(f64::total_cmp);
        levels.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        match levels.as_slice() {
            [] => h.floor_above = Some(project.floors[floor + 1].elevation),
            [one] => h.floor_above = Some(*one),
            _ => h.mixed_above = true,
        }
    }
    if floor > 0 {
        if let Some((level, e, g)) = near(floor - 1).into_iter().next() {
            h.floor_below = Some(level);
            h.ceiling_below = Some(rough_ceiling(&project.floors[g], e));
            if let Some(w) = e.and_then(|n| n.stem_wall_height) {
                h.swt_below = Some(project.floors[g].elevation + w);
            }
        }
    }
    if entry.is_some_and(|n| n.options.floor_from_foundation) {
        if let Some(swt) = h.swt_below.or(h.floor_below) {
            h.floor_to_stem_wall_top = Some(swt - floor_abs);
            h.stem_wall_top_to_ceiling = Some(h.ceiling_abs - swt);
        }
    }
    h
}

// ----- room polylines (R-110) -----

/// Make Room Polyline: the surfaces of the room (its inner wall surfaces).
pub fn room_polyline(room: &Room) -> Vec<Point> {
    if room.inner_polygon.len() >= 3 {
        room.inner_polygon.clone()
    } else {
        room.polygon.clone()
    }
}

/// Make Standard Area Polyline: the extent of the room's Standard Area.
pub fn standard_area_polyline(
    room: &Room,
    defining: &[Wall],
    wall_types: &[WallTypeDef],
    to: LivingTo,
) -> Vec<Point> {
    standard_polygon(room, defining, wall_types, to)
}

/// Make Living Area Polyline: the outline of the counted rooms' Standard
/// Areas of one structure (the exact extent of the Living Area). One ring per
/// separate piece, outer rings first.
pub fn living_area_polylines(
    floor: &Floor,
    rooms: &[Room],
    area: &LivingArea,
    wall_types: &[WallTypeDef],
    to: LivingTo,
) -> Vec<Vec<Point>> {
    union_standard_polygons(floor, rooms, &area.rooms, wall_types, to)
}

/// Make Room Polyline on the Exterior Room: the outline around the exterior
/// walls of `structure` (every room of it, counted in the Living Area or not),
/// to the outside surface or the outside of the Main Layer.
pub fn structure_polylines(
    floor: &Floor,
    rooms: &[Room],
    structure: &Structure,
    wall_types: &[WallTypeDef],
    to: LivingTo,
) -> Vec<Vec<Point>> {
    union_standard_polygons(floor, rooms, &structure.rooms, wall_types, to)
}

fn union_standard_polygons(
    floor: &Floor,
    rooms: &[Room],
    members: &[usize],
    wall_types: &[WallTypeDef],
    to: LivingTo,
) -> Vec<Vec<Point>> {
    let defining = defining_walls(&floor.walls);
    let shapes: Vec<Ring> = members
        .iter()
        .map(|&i| Ring::polygon(standard_polygon(&rooms[i], &defining, wall_types, to)))
        .collect();
    let mut rings = boolean_all(BoolOp::Union, &shapes);
    rings.sort_by(|a, b| b.area().total_cmp(&a.area()));
    rings.into_iter().map(|r| r.pts).collect()
}

/// Expand Room Polyline: the room with the invisible walls and railings
/// around it ignored, so it spans the rooms they divide. `None` when the
/// room has no such wall around it (the expanded room would be the same).
pub fn expand_room(walls: &[Wall], tol: f64, inside: Point) -> Option<Room> {
    let solid: Vec<Wall> = walls
        .iter()
        .filter(|w| {
            !(w.is_room_divider() || w.class.is_railing() || w.flags.railing || w.flags.invisible)
        })
        .cloned()
        .collect();
    if solid.len() == walls.len() {
        return None;
    }
    let expanded = crate::rooms::detect_rooms(&solid, tol);
    let original = crate::rooms::detect_rooms(walls, tol);
    let small = original.iter().find(|r| r.contains(inside))?;
    let big = expanded
        .into_iter()
        .filter(|r| r.contains(inside))
        .min_by(|a, b| a.area_sq_in.total_cmp(&b.area_sq_in))?;
    (big.area_sq_in > small.area_sq_in + 1.0).then_some(big)
}

// ----- the Exterior Room (R-106) -----

/// The Exterior Room Specification of one structure on a floor: the covering
/// and surface material of its exterior walls (manual pp. 449 and 450).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExteriorRoom {
    /// A point inside the structure (its largest room's anchor when the
    /// specification was made); the structure that holds it is this one.
    pub anchor: Point,
    /// The exterior face covering put on every exterior wall of the structure.
    pub covering: SideCovering,
    /// Material of the Exterior Wall Surface ("" = leave the walls').
    pub surface_material: String,
    pub surface_rgb: [u8; 3],
}

impl Default for ExteriorRoom {
    fn default() -> Self {
        Self {
            anchor: Point::ZERO,
            covering: SideCovering::default(),
            surface_material: String::new(),
            surface_rgb: [200, 200, 200],
        }
    }
}

impl ExteriorRoom {
    /// Is the specification empty (it changes nothing)?
    pub fn is_blank(&self) -> bool {
        self.covering.is_empty() && self.surface_material.is_empty()
    }
}

/// The Exterior Room record of the structure of `floor` holding `anchor`.
pub fn exterior_room_of<'a>(
    floor: &'a Floor,
    rooms: &[Room],
    structure: &Structure,
) -> Option<&'a ExteriorRoom> {
    floor
        .exterior_rooms
        .iter()
        .find(|e| structure.rooms.iter().any(|&i| rooms[i].contains(e.anchor)))
}

/// The ids of the exterior walls around `structure`: the exterior-kind walls
/// whose centerline lies along its outline.
pub fn structure_exterior_walls(floor: &Floor, structure: &Structure) -> Vec<Id> {
    let out = &structure.outline;
    let n = out.len();
    if n < 3 {
        return Vec::new();
    }
    floor
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior)
        .filter(|w| {
            let mid = Point::lerp(w.start, w.end, 0.5);
            (0..n).any(|i| {
                let (a, b) = (out[i], out[(i + 1) % n]);
                crate::geometry::dist_to_segment(mid, a, b) < 0.5
                    && crate::geometry::dist_to_segment(w.start, a, b) < 0.5
                    && crate::geometry::dist_to_segment(w.end, a, b) < 0.5
            })
        })
        .map(|w| w.id)
        .collect()
}

/// Writes `spec` onto the exterior walls `ids` of `floor`: the covering of
/// the exterior face and, when a surface material is named, the Exterior
/// Wall Surface. Returns how many walls changed.
pub fn apply_exterior_room(floor: &mut Floor, ids: &[Id], spec: &ExteriorRoom) -> usize {
    let mut n = 0;
    for w in floor.walls.iter_mut().filter(|w| ids.contains(&w.id)) {
        w.spec.covering.exterior = spec.covering.clone();
        if !spec.surface_material.is_empty() {
            w.spec.materials.set(
                "Exterior Wall Surface",
                Some((&spec.surface_material, spec.surface_rgb)),
            );
        }
        n += 1;
    }
    n
}

/// Raising or lowering the default heights of a level by dragging the
/// Exterior Room's edges (manual p. 451): the top edge sets the default
/// ceiling height, the bottom edge the default floor height (the level's
/// elevation, not available on Floor 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExteriorEdge {
    Top,
    Bottom,
}

/// Structure layers of a floor platform as one line (for readouts).
pub fn describe_layers(layers: &[StructureLayer]) -> String {
    layers
        .iter()
        .map(|l| format!("{} {:.2}", l.material, l.thickness))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::{WallLayer, WallTypeDef};
    use crate::model::{Project, Wall, WallKind};
    use crate::rooms::detect_rooms;

    fn ring(p: &mut Project, x0: f64, y0: f64, x1: f64, y1: f64, kind: WallKind) {
        let c = [
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, kind);
        }
    }

    fn types() -> Vec<RoomTypeDef> {
        vec![
            RoomTypeDef::new("Bedroom", "Standard", true, true),
            RoomTypeDef::new("Garage", "Garage", false, false),
            RoomTypeDef::new("Basement", "Standard", true, true),
        ]
    }

    fn name(p: &mut Project, at: Point, ty: &str) {
        p.floors[0].room_names.push(RoomName::new(at, ty, ty));
    }

    #[test]
    fn standard_area_runs_to_the_outside_surface_and_the_main_layer() {
        let mut p = Project::new("t");
        ring(&mut p, 0.0, 0.0, 240.0, 120.0, WallKind::Exterior);
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert_eq!(rooms.len(), 1);
        let def = defining_walls(&p.floors[0].walls);
        // 6 in walls: the outside surface is 3 in out from the centerline.
        let a = standard_area_sq_in(&rooms[0], &def, &[], LivingTo::OutsideSurface);
        assert!((a - 246.0 * 126.0).abs() < 1e-6, "{a}");
        // A wall type of siding 1, main 4, drywall 1: the main layer's
        // outside is 1 in from the outside surface, so 2 in from the center.
        let ty = WallTypeDef {
            name: "Frame".into(),
            kind: WallKind::Exterior,
            layers: vec![
                WallLayer::new("Siding", 1.0, false, "Siding"),
                WallLayer::new("Framing", 4.0, true, "Fir"),
                WallLayer::new("Drywall", 1.0, false, "Drywall"),
            ],
        };
        for w in &mut p.floors[0].walls {
            w.wall_type = Some("Frame".into());
            w.exterior_side = Side::Right; // counter-clockwise walls: out is right
        }
        let def = defining_walls(&p.floors[0].walls);
        let m = standard_area_sq_in(&rooms[0], &def, &[ty], LivingTo::MainLayer);
        assert!((m - 244.0 * 124.0).abs() < 1e-6, "{m}");
    }

    #[test]
    fn rooms_rule_the_living_area_at_the_48_inch_boundary() {
        let mut p = Project::new("t");
        ring(&mut p, 0.0, 0.0, 240.0, 120.0, WallKind::Exterior);
        name(&mut p, Point::new(120.0, 60.0), "Bedroom");
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let f = &p.floors[0];
        assert!(room_in_living_area(f, &rooms[0], &types()));
        let mut g = f.clone();
        g.room_names[0].rough_ceiling = Some(47.9);
        assert!(!room_in_living_area(&g, &rooms[0], &types()));
        g.room_names[0].rough_ceiling = Some(48.0);
        assert!(room_in_living_area(&g, &rooms[0], &types()));
        // The room's own setting wins over the rule and the type.
        g.room_names[0].rough_ceiling = Some(10.0);
        g.room_names[0].include_in_living_area = Some(true);
        assert!(room_in_living_area(&g, &rooms[0], &types()));
        g.room_names[0].include_in_living_area = Some(false);
        g.room_names[0].rough_ceiling = Some(96.0);
        assert!(!room_in_living_area(&g, &rooms[0], &types()));
        // A hybrid type is out unless included by hand.
        let mut h = f.clone();
        h.room_names[0].room_type = "Garage".into();
        assert!(!room_in_living_area(&h, &rooms[0], &types()));
        h.room_names[0].include_in_living_area = Some(true);
        assert!(room_in_living_area(&h, &rooms[0], &types()));
    }

    #[test]
    fn a_basement_counts_at_48_inches_or_more() {
        let mut p = Project::new("t");
        ring(&mut p, 0.0, 0.0, 240.0, 120.0, WallKind::Exterior);
        name(&mut p, Point::new(120.0, 60.0), "Basement");
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let mut f = p.floors[0].clone();
        f.room_names[0].ceiling_height = Some(72.0);
        assert!(room_in_living_area(&f, &rooms[0], &types()));
        f.room_names[0].ceiling_height = Some(40.0);
        f.room_names[0].rough_ceiling = None;
        assert!(!room_in_living_area(&f, &rooms[0], &types()));
    }

    #[test]
    fn each_structure_has_its_own_rounded_living_area() {
        let mut p = Project::new("two");
        ring(&mut p, 0.0, 0.0, 240.0, 120.0, WallKind::Exterior);
        ring(&mut p, 600.0, 0.0, 780.0, 100.0, WallKind::Exterior);
        name(&mut p, Point::new(120.0, 60.0), "Bedroom");
        name(&mut p, Point::new(690.0, 50.0), "Bedroom");
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert_eq!(rooms.len(), 2);
        let st = structures(&rooms);
        assert_eq!(st.len(), 2);
        let areas = living_areas(
            &p.floors[0],
            &rooms,
            &types(),
            &[],
            LivingTo::OutsideSurface,
        );
        assert_eq!(areas.len(), 2);
        let sq: Vec<f64> = areas.iter().map(LivingArea::sq_ft).collect();
        // 246 x 126 and 186 x 106 in, to the nearest sq ft.
        let want = [246.0_f64 * 126.0 / 144.0, 186.0_f64 * 106.0 / 144.0];
        for w in want {
            assert!(sq.contains(&w.round()), "{sq:?} vs {want:?}");
        }
        assert!(areas[0].text().starts_with("Living Area: "));
        // Excluding every room of one structure removes its label.
        p.floors[0].room_names[1].include_in_living_area = Some(false);
        let one = living_areas(
            &p.floors[0],
            &rooms,
            &types(),
            &[],
            LivingTo::OutsideSurface,
        );
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn rooms_that_share_a_wall_are_one_structure_and_the_outline_is_the_outside() {
        let mut p = Project::new("l");
        ring(&mut p, 0.0, 0.0, 240.0, 120.0, WallKind::Exterior);
        p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert_eq!(rooms.len(), 2);
        let st = structures(&rooms);
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].rooms.len(), 2);
        assert!((polygon_area(&st[0].outline) - 240.0 * 120.0).abs() < 1e-6);
        assert!(near_outline(&st[0].outline, Point::new(120.0, -5.0), 12.0));
        assert!(!near_outline(&st[0].outline, Point::new(120.0, 60.0), 12.0));
    }

    #[test]
    fn a_bay_window_is_in_the_interior_area_but_not_the_standard_area() {
        let mut p = Project::new("bay");
        ring(&mut p, 0.0, 0.0, 240.0, 120.0, WallKind::Exterior);
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let wall = p.floors[0]
            .walls
            .iter()
            .find(|w| w.start == Point::new(0.0, 0.0) && w.end == Point::new(240.0, 0.0))
            .unwrap()
            .clone();
        let mut o = crate::model::Opening::default_window(1, wall.id, 120.0);
        o.style = OpeningStyle::BoxWindow;
        o.width = 60.0;
        // Counter-clockwise walls: the room is to the left, so outside is to
        // the right of each wall.
        let mut f = p.floors[0].clone();
        for w in &mut f.walls {
            w.exterior_side = Side::Right;
        }
        f.openings.push(o);
        let def = defining_walls(&f.walls);
        let bay = bay_area_sq_in(&f, &rooms[0], &def);
        assert!(bay > 60.0 * 18.0, "{bay}");
        assert!(interior_area_with_bays_sq_in(&f, &rooms[0], &def) > rooms[0].interior_area_sq_in);
        // The Standard Area does not change with the window.
        let a = standard_area_sq_in(&rooms[0], &def, &[], LivingTo::OutsideSurface);
        assert!((a - 246.0 * 126.0).abs() < 1e-6);
    }

    #[test]
    fn the_polylines_follow_the_room_the_standard_area_and_the_living_area() {
        let mut p = Project::new("poly");
        ring(&mut p, 0.0, 0.0, 240.0, 120.0, WallKind::Exterior);
        p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        name(&mut p, Point::new(60.0, 60.0), "Bedroom");
        name(&mut p, Point::new(180.0, 60.0), "Bedroom");
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let def = defining_walls(&p.floors[0].walls);
        let left = rooms.iter().find(|r| r.centroid.x < 120.0).unwrap();
        assert!((polygon_area(&room_polyline(left)) - left.interior_area_sq_in).abs() < 1e-6);
        let std = standard_area_polyline(left, &def, &[], LivingTo::OutsideSurface);
        // To the outside of the exterior walls and the centre of the divider.
        assert!((polygon_area(&std) - 123.0 * 126.0).abs() < 1e-6);
        let areas = living_areas(
            &p.floors[0],
            &rooms,
            &types(),
            &[],
            LivingTo::OutsideSurface,
        );
        let rings = living_area_polylines(
            &p.floors[0],
            &rooms,
            &areas[0],
            &[],
            LivingTo::OutsideSurface,
        );
        assert_eq!(rings.len(), 1, "two rooms form one outline");
        assert!((polygon_area(&rings[0]).abs() - 246.0 * 126.0).abs() < 1.0);
    }

    #[test]
    fn expand_room_ignores_an_invisible_wall() {
        let mut p = Project::new("x");
        ring(&mut p, 0.0, 0.0, 240.0, 120.0, WallKind::Interior);
        let id = p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            0.5,
            96.0,
            WallKind::Interior,
        );
        let w = p.floors[0].walls.iter_mut().find(|w| w.id == id).unwrap();
        w.flags.room_divider = true;
        let walls = p.floors[0].walls.clone();
        assert_eq!(detect_rooms(&walls, 0.5).len(), 2);
        let big = expand_room(&walls, 0.5, Point::new(60.0, 60.0)).expect("spans both rooms");
        assert!((big.area_sq_in - 240.0 * 120.0).abs() < 1e-6);
        // Without an invisible wall there is nothing to expand.
        let mut q = Project::new("y");
        ring(&mut q, 0.0, 0.0, 240.0, 120.0, WallKind::Interior);
        assert!(expand_room(&q.floors[0].walls, 0.5, Point::new(60.0, 60.0)).is_none());
    }

    #[test]
    fn an_exterior_room_spec_covers_the_walls_of_its_structure() {
        let mut p = Project::new("e");
        ring(&mut p, 0.0, 0.0, 240.0, 120.0, WallKind::Exterior);
        ring(&mut p, 600.0, 0.0, 780.0, 100.0, WallKind::Exterior);
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let st = structures(&rooms);
        let near = st.iter().find(|s| s.anchor.x < 300.0).unwrap();
        let ids = structure_exterior_walls(&p.floors[0], near);
        assert_eq!(ids.len(), 4);
        let mut spec = ExteriorRoom::default();
        spec.covering.covering = "Lap Siding".into();
        spec.surface_material = "Cedar Siding".into();
        let n = apply_exterior_room(&mut p.floors[0], &ids, &spec);
        assert_eq!(n, 4);
        let covered = p.floors[0]
            .walls
            .iter()
            .filter(|w| w.spec.covering.exterior.covering == "Lap Siding")
            .count();
        assert_eq!(covered, 4, "the other structure's walls are untouched");
        let _: &Wall = &p.floors[0].walls[0];
    }

    #[test]
    fn the_structure_panel_heights_read_the_floors_above_and_below() {
        let mut p = Project::new("h");
        ring(&mut p, 0.0, 0.0, 240.0, 120.0, WallKind::Exterior);
        let up = p.build_new_floor(true);
        let at = Point::new(120.0, 60.0);
        // Ground floor: nothing below, the second floor's subfloor above.
        let g = room_heights(&p, 0, None, at);
        assert!(g.floor_below.is_none() && g.ceiling_below.is_none());
        assert_eq!(g.floor_above, Some(p.floors[up].elevation));
        assert!((g.ceiling_abs - g.floor_abs - g.rough).abs() < 1e-9);
        // The upper floor: the ground floor below, its rough ceiling.
        let u = room_heights(&p, up, None, at);
        assert_eq!(u.floor_below, Some(p.floors[0].elevation));
        assert!(u
            .ceiling_below
            .is_some_and(|c| c >= p.floors[0].ceiling_height));
        assert!(u.floor_above.is_none());
        // A raised floor shows in the absolute floor; a rough ceiling in both.
        let mut n = RoomName::new(at, "Bedroom", "Bedroom");
        n.floor_height_offset = 6.0;
        n.rough_ceiling = Some(100.0);
        let h = room_heights(&p, 0, Some(&n), at);
        assert_eq!(h.floor_abs, 6.0);
        assert_eq!(h.rough, 100.0);
        assert_eq!(h.ceiling_abs, 106.0);
        // Foundation-supplied floors report the stem wall distances.
        n.options.floor_from_foundation = true;
        let f = room_heights(&p, up, Some(&n), at);
        assert!(f.floor_to_stem_wall_top.is_some() && f.stem_wall_top_to_ceiling.is_some());
    }

    #[test]
    fn areas_round_to_the_nearest_square_foot() {
        assert_eq!(round_sq_ft(144.0 * 12.4), 12.0);
        assert_eq!(round_sq_ft(144.0 * 12.5), 13.0);
        assert_eq!(round_sq_ft(144.0 * 12.6), 13.0);
        assert_eq!(group_thousands(1235), "1,235");
        assert_eq!(group_thousands(95), "95");
        assert_eq!(group_thousands(1_234_567), "1,234,567");
        let a = LivingArea {
            structure: 0,
            rooms: vec![0],
            area_sq_in: 144.0 * 1819.6,
            label_at: Point::ZERO,
        };
        assert_eq!(a.sq_ft(), 1820.0);
        assert_eq!(a.text(), "Living Area: 1,820 sq ft");
        assert_eq!(
            LivingTo::from_name("Outside of Main Layer"),
            LivingTo::MainLayer
        );
        assert_eq!(LivingTo::from_name("whatever"), LivingTo::OutsideSurface);
    }
}
