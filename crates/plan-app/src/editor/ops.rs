//! Geometry edits on the model that more than one tool needs: splitting
//! walls, moving walls with their connected neighbours, sliding openings,
//! transforming CAD items and deleting objects. All of them are plain
//! functions on a `Project`; undo bookkeeping is the caller's job.

use super::selection::ObjectRef;
use plan_core::cad::CadItem;
use plan_core::geometry::{dist_to_segment, project_on_segment, Point};
use plan_core::{Id, Opening, Project, Wall, WallEnd, WallKind};

/// Two wall ends closer than this are connected (same as room detection).
pub const JOIN_TOL: f64 = 0.5;
/// Minimum clear distance between an opening jamb and a wall end or another opening.
pub const OPENING_MARGIN: f64 = 2.0;

/// A wall value that is not in any project (ghosts, templates).
pub fn make_wall(
    id: Id,
    start: Point,
    end: Point,
    thickness: f64,
    height: f64,
    kind: WallKind,
) -> Wall {
    Wall {
        id,
        ..Wall::new(start, end, thickness, height, kind)
    }
}

fn end_pos(w: &Wall, e: WallEnd) -> Point {
    match e {
        WallEnd::Start => w.start,
        WallEnd::End => w.end,
    }
}

fn other_end(e: WallEnd) -> WallEnd {
    match e {
        WallEnd::Start => WallEnd::End,
        WallEnd::End => WallEnd::Start,
    }
}

/// Wall ends within `tol` of `p`, skipping wall `exclude`.
pub fn walls_at(
    project: &Project,
    floor: usize,
    p: Point,
    tol: f64,
    exclude: Option<Id>,
) -> Vec<(Id, WallEnd)> {
    let mut out = Vec::new();
    for w in &project.floors[floor].walls {
        if Some(w.id) == exclude {
            continue;
        }
        for e in [WallEnd::Start, WallEnd::End] {
            if end_pos(w, e).dist(p) <= tol {
                out.push((w.id, e));
            }
        }
    }
    out
}

/// Clamps the openings of `wall_id` into the wall (keeping the jamb margin)
/// and removes the ones that can no longer fit (W-85). Returns the removed ids.
pub fn revalidate_openings(project: &mut Project, floor: usize, wall_id: Id) -> Vec<Id> {
    let f = &mut project.floors[floor];
    let Some(len) = f.wall(wall_id).map(Wall::length) else {
        return Vec::new();
    };
    let mut removed = Vec::new();
    f.openings.retain(|o| {
        let fits = o.wall_id != wall_id || len >= o.width + 2.0 * OPENING_MARGIN;
        if !fits {
            removed.push(o.id);
        }
        fits
    });
    for o in f.openings.iter_mut().filter(|o| o.wall_id == wall_id) {
        let half = o.width * 0.5;
        o.center_offset = o
            .center_offset
            .clamp(half + OPENING_MARGIN, len - half - OPENING_MARGIN);
    }
    removed
}

/// Sets one end of a wall without touching neighbours. Openings keep their
/// distance from the end that stays put. When the wall is stretched past its
/// other end (its direction reverses) the exterior side is flipped with it, so
/// the layers stay on the same physical face (Chief stretches, never
/// detaches).
pub(crate) fn set_end(project: &mut Project, floor: usize, id: Id, end: WallEnd, to: Point) {
    let f = &mut project.floors[floor];
    let Some(w) = f.wall_mut(id) else { return };
    let old_len = w.length();
    let old_dir = w.direction();
    match end {
        WallEnd::Start => w.start = to,
        WallEnd::End => w.end = to,
    }
    let new_len = w.length();
    if new_len > 1e-9 && old_len > 1e-9 && w.direction().dot(old_dir) < 0.0 {
        w.exterior_side = w.exterior_side.opposite();
    }
    if end == WallEnd::Start {
        for o in f.openings.iter_mut().filter(|o| o.wall_id == id) {
            o.center_offset += new_len - old_len;
        }
    }
    revalidate_openings(project, floor, id);
}

/// Moves one end of wall `id` to `to`; every other wall end connected to the
/// old position follows (their far ends stay fixed). Walls that butt into the
/// side of wall `id` (T-junctions) are stretched along their own direction so
/// they stay on the wall's new centerline (S-19, W-105). Returns false if the
/// wall does not exist.
pub fn move_wall_end_joined(
    project: &mut Project,
    floor: usize,
    id: Id,
    end: WallEnd,
    to: Point,
) -> bool {
    let Some(w) = project.floors[floor].wall(id).cloned() else {
        return false;
    };
    let old = end_pos(&w, end);
    let far = end_pos(&w, other_end(end));
    let joined = walls_at(project, floor, old, JOIN_TOL, Some(id));
    // Ends resting on this wall's side, away from both of its ends.
    let mut butting: Vec<(Id, WallEnd, Point)> = Vec::new();
    for o in &project.floors[floor].walls {
        if o.id == id || joined.iter().any(|(j, _)| *j == o.id) {
            continue;
        }
        for e in [WallEnd::Start, WallEnd::End] {
            let p = end_pos(o, e);
            if p.dist(w.start) > JOIN_TOL
                && p.dist(w.end) > JOIN_TOL
                && dist_to_segment(p, w.start, w.end) <= JOIN_TOL
            {
                butting.push((o.id, e, p));
            }
        }
    }
    set_end(project, floor, id, end, to);
    for (jid, jend) in joined {
        set_end(project, floor, jid, jend, to);
    }
    let nd = (to - far).normalized();
    if to.dist(far) > 1e-6 {
        for (oid, e, p) in butting {
            let Some(o) = project.floors[floor].wall(oid) else {
                continue;
            };
            let far_o = end_pos(o, other_end(e));
            let v = (p - far_o).normalized();
            let target = line_intersection(far, nd, far_o, v)
                .unwrap_or_else(|| project_on_segment(p, far, to).1);
            set_end(project, floor, oid, e, target);
        }
    }
    true
}

/// Intersection of the line through `p` along `u` with the line through `q`
/// along `v`.
pub(crate) fn line_intersection(p: Point, u: Point, q: Point, v: Point) -> Option<Point> {
    let denom = u.cross(v);
    if denom.abs() < 0.02 {
        return None;
    }
    let k = (q - p).cross(v) / denom;
    Some(p + u * k)
}

/// Moves a wall perpendicular to its length by `s` inches (positive toward
/// its +normal). Walls connected at its ends, and walls butting into its
/// side, keep their directions and stretch so the corners stay joined; where
/// that is impossible (collinear continuation) their end simply follows.
pub fn move_wall_perpendicular(project: &mut Project, floor: usize, id: Id, s: f64) -> bool {
    let Some(w) = project.floors[floor].wall(id).cloned() else {
        return false;
    };
    let n = w.normal();
    let d = n * s;
    let u = w.direction();
    let (a2, b2) = (w.start + d, w.end + d);

    // (wall, which end, shared point on the moving wall's line)
    let mut followers: Vec<(Id, WallEnd, Point)> = Vec::new();
    for o in &project.floors[floor].walls {
        if o.id == id {
            continue;
        }
        for e in [WallEnd::Start, WallEnd::End] {
            let p = end_pos(o, e);
            let at_end = p.dist(w.start) <= JOIN_TOL || p.dist(w.end) <= JOIN_TOL;
            let on_side = !at_end && dist_to_segment(p, w.start, w.end) <= JOIN_TOL;
            if at_end || on_side {
                followers.push((o.id, e, p));
            }
        }
    }
    for (oid, e, p) in followers {
        let Some(o) = project.floors[floor].wall(oid) else {
            continue;
        };
        let far = end_pos(o, other_end(e));
        let v = (p - far).normalized();
        // Where the follower meets the moved wall's line.
        let moved_line_point = if p.dist(w.start) <= JOIN_TOL { a2 } else { b2 };
        let target = if p.dist(w.start) <= JOIN_TOL || p.dist(w.end) <= JOIN_TOL {
            line_intersection(moved_line_point, u, far, v)
        } else {
            line_intersection(a2, u, far, v)
        };
        let to = target.unwrap_or(p + d);
        set_end(project, floor, oid, e, to);
    }
    if let Some(wm) = project.floors[floor].wall_mut(id) {
        wm.start = a2;
        wm.end = b2;
    }
    true
}

/// Translates the walls `ids` by `delta` (openings travel with their wall).
/// Wall ends of other walls connected to the moved ends follow, so corners
/// stay joined (free move, S-22 and S-37).
pub fn translate_walls_with_followers(
    project: &mut Project,
    floor: usize,
    ids: &[Id],
    delta: Point,
) {
    let mut followers: Vec<(Id, WallEnd, Point)> = Vec::new();
    for id in ids {
        let Some(w) = project.floors[floor].wall(*id).cloned() else {
            continue;
        };
        for p in [w.start, w.end] {
            for (jid, e) in walls_at(project, floor, p, JOIN_TOL, None) {
                let known = followers.iter().any(|(i, en, _)| *i == jid && *en == e);
                if !ids.contains(&jid) && !known {
                    followers.push((jid, e, p + delta));
                }
            }
        }
    }
    for (jid, e, to) in followers {
        set_end(project, floor, jid, e, to);
    }
    for id in ids {
        project.translate_wall(floor, *id, delta);
    }
}

/// Splits wall `id` at the point of its centerline nearest `at` (see
/// `Project::split_wall_at`). The first half keeps the id; returns the id of
/// the second half. Refuses (None) near the ends or when an opening straddles
/// the split.
pub fn split_wall_at(project: &mut Project, floor: usize, id: Id, at: Point) -> Option<Id> {
    let w = project.floors[floor].wall(id)?;
    let (t, _) = project_on_segment(at, w.start, w.end);
    let d = t * w.length();
    if d <= JOIN_TOL || d >= w.length() - JOIN_TOL {
        return None;
    }
    project.split_wall_at(floor, id, at).map(|(_, new)| new)
}

/// Splits every wall whose centerline passes through `p` away from its ends
/// (T-junctions). Walls in `exclude` are left alone. Returns the new ids.
pub fn split_walls_at_point(
    project: &mut Project,
    floor: usize,
    p: Point,
    exclude: &[Id],
) -> Vec<Id> {
    let hits: Vec<Id> = project.floors[floor]
        .walls
        .iter()
        .filter(|w| {
            !exclude.contains(&w.id)
                && dist_to_segment(p, w.start, w.end) <= 0.05
                && p.dist(w.start) > JOIN_TOL
                && p.dist(w.end) > JOIN_TOL
        })
        .map(|w| w.id)
        .collect();
    hits.into_iter()
        .filter_map(|id| split_wall_at(project, floor, id, p))
        .collect()
}

/// Moves an opening to `center` on `wall_id` (its host or another wall),
/// clamped to the jamb margin. Returns false (and changes nothing) when it
/// would overlap another opening or the wall is too short.
pub fn place_opening_at(
    project: &mut Project,
    floor: usize,
    opening_id: Id,
    wall_id: Id,
    center: f64,
) -> bool {
    let f = &project.floors[floor];
    let Some(mut o) = f.openings.iter().find(|o| o.id == opening_id).cloned() else {
        return false;
    };
    let Some(len) = f.wall(wall_id).map(Wall::path_length) else {
        return false;
    };
    let half = o.width * 0.5;
    if len < o.width + 2.0 * OPENING_MARGIN {
        return false;
    }
    o.wall_id = wall_id;
    o.center_offset = center.clamp(half + OPENING_MARGIN, len - half - OPENING_MARGIN);
    // The shared placement rules: neighbour clearance (windows may touch)
    // and the bodies of the walls that meet the host.
    if !f
        .wall(wall_id)
        .is_some_and(|host| plan_core::openings::placement::fits_at(f, host, &o, &[opening_id]))
    {
        return false;
    }
    if let Some(slot) = project.floors[floor]
        .openings
        .iter_mut()
        .find(|x| x.id == opening_id)
    {
        *slot = o;
    }
    true
}

/// Fix Wall Connections (W-41, W-42) on a whole floor: pairs ends within the
/// connect distance, closes corners, joins Ts and crossings, merges
/// overlapping collinear walls and removes zero-length walls. Returns the
/// number of repairs made (0 when the plan was already clean). The Edit
/// toolbar button calls `connect::fix_wall_connections_action`.
pub fn fix_wall_connections(project: &mut Project, floor: usize) -> usize {
    super::connect::fix_all_connections_project(
        project,
        floor,
        &super::connect::ConnectOptions::default(),
    )
}

// ----- CAD transforms -----

fn rot(p: Point, c: Point, a: f64) -> Point {
    let (s, co) = a.sin_cos();
    let v = p - c;
    c + Point::new(v.x * co - v.y * s, v.x * s + v.y * co)
}

pub fn translate_cad(item: &mut CadItem, d: Point) {
    match item {
        CadItem::Line { a, b } => {
            *a = *a + d;
            *b = *b + d;
        }
        CadItem::Arc { center, .. } | CadItem::Circle { center, .. } => *center = *center + d,
        CadItem::Polyline { points, .. } => points.iter_mut().for_each(|p| *p = *p + d),
        CadItem::Text { pos, .. } => *pos = *pos + d,
    }
}

pub fn rotate_cad(item: &mut CadItem, c: Point, a: f64) {
    match item {
        CadItem::Line { a: p, b } => {
            *p = rot(*p, c, a);
            *b = rot(*b, c, a);
        }
        CadItem::Arc {
            center,
            start_angle,
            end_angle,
            ..
        } => {
            *center = rot(*center, c, a);
            *start_angle += a;
            *end_angle += a;
        }
        CadItem::Circle { center, .. } => *center = rot(*center, c, a),
        CadItem::Polyline { points, .. } => points.iter_mut().for_each(|p| *p = rot(*p, c, a)),
        CadItem::Text { pos, angle, .. } => {
            *pos = rot(*pos, c, a);
            *angle += a;
        }
    }
}

/// Center of a CAD item's bounds.
pub fn cad_center(item: &CadItem) -> Point {
    let (lo, hi) = item.bounds();
    Point::lerp(lo, hi, 0.5)
}

// ----- deleting -----

/// Deletes the objects (a wall takes its openings with it). Returns the
/// number of objects removed.
pub fn delete_objects(project: &mut Project, floor: usize, objects: &[ObjectRef]) -> usize {
    let mut n = 0;
    for o in objects {
        let before = {
            let f = &project.floors[floor];
            f.walls.len() + f.openings.len() + f.dimensions.len() + f.cad.len()
        };
        match *o {
            ObjectRef::Wall(id) => project.remove_wall(floor, id),
            ObjectRef::Opening(id) => project.remove_opening(floor, id),
            ObjectRef::Dimension(id) => project.remove_dimension(floor, id),
            ObjectRef::Cad(id) | ObjectRef::Text(id) => project.remove_cad(floor, id),
            _ => {}
        }
        let f = &project.floors[floor];
        let after = f.walls.len() + f.openings.len() + f.dimensions.len() + f.cad.len();
        n += usize::from(after < before);
    }
    // A deleted CAD object takes its style extras with it.
    project.prune_cad_data(floor);
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::OpeningKind;

    fn room() -> (Project, [Id; 4]) {
        let mut p = Project::new("t");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(120.0, 96.0),
            Point::new(0.0, 96.0),
        ];
        let mut ids = [0; 4];
        for i in 0..4 {
            ids[i] = p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, 100.0, WallKind::Exterior);
        }
        (p, ids)
    }

    #[test]
    fn perpendicular_move_keeps_corners_joined() {
        let (mut p, ids) = room();
        // Move the top wall (index 2, runs right-to-left) down by 20".
        let top = p.floors[0].wall(ids[2]).unwrap().clone();
        let s = (Point::new(0.0, -20.0)).dot(top.normal());
        assert!(move_wall_perpendicular(&mut p, 0, ids[2], s));
        let f = &p.floors[0];
        assert_eq!(f.wall(ids[2]).unwrap().start, Point::new(120.0, 76.0));
        assert_eq!(f.wall(ids[2]).unwrap().end, Point::new(0.0, 76.0));
        // Right wall shortened, left wall shortened, both still attached.
        assert_eq!(f.wall(ids[1]).unwrap().end, Point::new(120.0, 76.0));
        assert_eq!(f.wall(ids[3]).unwrap().start, Point::new(0.0, 76.0));
        // Directions unchanged.
        assert!(f.wall(ids[1]).unwrap().direction().x.abs() < 1e-9);
    }

    #[test]
    fn end_drag_moves_joined_ends() {
        let (mut p, ids) = room();
        assert!(move_wall_end_joined(
            &mut p,
            0,
            ids[0],
            WallEnd::End,
            Point::new(140.0, 10.0)
        ));
        let f = &p.floors[0];
        assert_eq!(f.wall(ids[0]).unwrap().end, Point::new(140.0, 10.0));
        assert_eq!(f.wall(ids[1]).unwrap().start, Point::new(140.0, 10.0));
        assert_eq!(f.wall(ids[3]).unwrap().end, Point::ZERO);
    }

    #[test]
    fn splitting_distributes_openings_and_refuses_straddlers() {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::ZERO,
            Point::new(200.0, 0.0),
            6.0,
            100.0,
            WallKind::Interior,
        );
        let d1 = p.add_opening(0, w, 30.0, OpeningKind::Door).unwrap();
        let d2 = p.add_opening(0, w, 160.0, OpeningKind::Door).unwrap();
        assert!(split_wall_at(&mut p, 0, w, Point::new(30.0, 0.0)).is_none());
        let n = split_wall_at(&mut p, 0, w, Point::new(100.0, 0.0)).unwrap();
        let f = &p.floors[0];
        assert_eq!(f.wall(w).unwrap().end, Point::new(100.0, 0.0));
        assert_eq!(f.wall(n).unwrap().start, Point::new(100.0, 0.0));
        let o1 = f.openings.iter().find(|o| o.id == d1).unwrap();
        let o2 = f.openings.iter().find(|o| o.id == d2).unwrap();
        assert_eq!((o1.wall_id, o2.wall_id), (w, n));
        assert!((o2.center_offset - 60.0).abs() < 1e-9);
    }

    fn corner_pair() -> (Project, Id, Id) {
        // A runs (0,0)-(100,0); B runs (100,0)-(100,60); joined at (100,0).
        let mut p = Project::new("t");
        let a = p.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let b = p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(100.0, 60.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        (p, a, b)
    }

    #[test]
    fn l_corner_wall_moved_perpendicular_stretches_the_neighbour() {
        let (mut p, a, b) = corner_pair();
        // Push B 30" to the right (toward +x).
        let s = Point::new(30.0, 0.0).dot(p.floors[0].wall(b).unwrap().normal());
        assert!(move_wall_perpendicular(&mut p, 0, b, s));
        let f = &p.floors[0];
        assert_eq!(f.wall(b).unwrap().start, Point::new(130.0, 0.0));
        assert_eq!(f.wall(a).unwrap().end, Point::new(130.0, 0.0));
        assert_eq!(f.wall(a).unwrap().start, Point::ZERO);
        assert!(f.wall(a).unwrap().direction().y.abs() < 1e-9);
    }

    #[test]
    fn t_wall_moved_perpendicular_keeps_butting_walls_attached() {
        // Through wall along y=0; a stem rises from its middle.
        let mut p = Project::new("t");
        let through = p.add_wall(
            0,
            Point::new(-100.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let stem = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(0.0, 80.0),
            4.5,
            100.0,
            WallKind::Interior,
        );
        // Move the through wall up 25".
        let s = Point::new(0.0, 25.0).dot(p.floors[0].wall(through).unwrap().normal());
        assert!(move_wall_perpendicular(&mut p, 0, through, s));
        let f = &p.floors[0];
        assert_eq!(f.wall(through).unwrap().start.y, 25.0);
        assert_eq!(f.wall(stem).unwrap().start, Point::new(0.0, 25.0));
        assert_eq!(f.wall(stem).unwrap().end, Point::new(0.0, 80.0));
    }

    #[test]
    fn x_junction_pieces_follow_a_moved_wall() {
        // Two crossing walls, both already split at (0,0).
        let mut p = Project::new("t");
        let l = p.add_wall(
            0,
            Point::new(-80.0, 0.0),
            Point::ZERO,
            6.0,
            100.0,
            WallKind::Interior,
        );
        let r = p.add_wall(
            0,
            Point::ZERO,
            Point::new(80.0, 0.0),
            6.0,
            100.0,
            WallKind::Interior,
        );
        let d = p.add_wall(
            0,
            Point::new(0.0, -80.0),
            Point::ZERO,
            6.0,
            100.0,
            WallKind::Interior,
        );
        let u = p.add_wall(
            0,
            Point::ZERO,
            Point::new(0.0, 80.0),
            6.0,
            100.0,
            WallKind::Interior,
        );
        // Move the right-hand piece 20" up: the vertical pieces stretch to it.
        let s = Point::new(0.0, 20.0).dot(p.floors[0].wall(r).unwrap().normal());
        assert!(move_wall_perpendicular(&mut p, 0, r, s));
        let f = &p.floors[0];
        let joint = f.wall(r).unwrap().start;
        assert_eq!(joint, Point::new(0.0, 20.0));
        assert_eq!(f.wall(d).unwrap().end, joint);
        assert_eq!(f.wall(u).unwrap().start, joint);
        // The collinear neighbour simply follows (no detach).
        assert_eq!(f.wall(l).unwrap().end, joint);
    }

    #[test]
    fn wall_moved_past_a_connected_wall_end_stretches_and_stays_attached() {
        let (mut p, a, b) = corner_pair();
        // Push B 150" left, beyond A's start at x=0.
        let s = Point::new(-150.0, 0.0).dot(p.floors[0].wall(b).unwrap().normal());
        assert!(move_wall_perpendicular(&mut p, 0, b, s));
        let f = &p.floors[0];
        assert_eq!(f.wall(b).unwrap().start, Point::new(-50.0, 0.0));
        assert_eq!(f.wall(a).unwrap().end, Point::new(-50.0, 0.0));
        // A now runs backwards; its layers stay on the same physical face.
        assert!(f.wall(a).unwrap().direction().x < 0.0);
        assert_eq!(
            f.wall(a).unwrap().exterior_side,
            plan_core::Side::Right,
            "exterior side flips with the direction"
        );
    }

    #[test]
    fn dragging_a_through_wall_end_keeps_the_stem_on_it() {
        let mut p = Project::new("t");
        let through = p.add_wall(
            0,
            Point::ZERO,
            Point::new(200.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let stem = p.add_wall(
            0,
            Point::new(100.0, 70.0),
            Point::new(100.0, 0.0),
            4.5,
            100.0,
            WallKind::Interior,
        );
        // Lift the far end of the through wall by 20": the stem end rides the
        // new centerline (y = 10 at x = 100).
        assert!(move_wall_end_joined(
            &mut p,
            0,
            through,
            WallEnd::End,
            Point::new(200.0, 20.0)
        ));
        let f = &p.floors[0];
        assert_eq!(f.wall(stem).unwrap().start, Point::new(100.0, 70.0));
        let e = f.wall(stem).unwrap().end;
        assert!((e.x - 100.0).abs() < 1e-9 && (e.y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn make_wall_sets_the_id() {
        let w = make_wall(
            9,
            Point::ZERO,
            Point::new(10.0, 0.0),
            4.0,
            90.0,
            WallKind::Interior,
        );
        assert_eq!((w.id, w.thickness), (9, 4.0));
    }
}
