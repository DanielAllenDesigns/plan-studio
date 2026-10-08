//! Automatic wall connections (W-31..W-45, W-12, W-40).
//!
//! Chief joins walls as they are drawn or edited: an end near another wall's
//! end becomes a corner, an end near another wall's centerline becomes a T,
//! crossing walls are cut at the crossing and duplicate collinear walls are
//! merged. [`auto_connect`] does that for one wall after it was created or an
//! end was moved/released; [`fix_all_connections`] runs it over every wall
//! (Fix Wall Connections, W-41/W-42).
//!
//! All the work is on a [`Project`] ([`auto_connect_project`]) so it is
//! testable without an editor; the `EditorContext` wrappers then mark the
//! derived data (rooms, outlines) for recomputing. None of these functions
//! open an undo step: the caller's `begin_change` covers the whole edit
//! (W-40), except [`fix_wall_connections_action`], which owns its own.
//!
//! Every function is idempotent: running it again on its own result changes
//! nothing and reports 0.
//!
//! Splitting through walls at a T (and at a crossing) is controlled by
//! [`ConnectOptions::split_on_tee`] (default true, `SPLIT_ON_TEE_DEFAULT`).
//! Chief keeps the through wall whole (W-35, W-36) but our room detection and
//! layer assignment need the pieces, so splitting is the default. The model's
//! `PlanDefaults` has no field for the flag yet; until it does it is an
//! option of the functions here, not a persisted default.

use super::ops::{self, JOIN_TOL};
use super::EditorContext;
use plan_core::geometry::{dist_to_segment, segment_intersection, Point};
use plan_core::{Id, Project, Wall, WallEnd};

/// Smallest connect distance, inches (Chief uses the wall thickness, min 6").
pub const MIN_CONNECT_DISTANCE: f64 = 6.0;
/// Walls shorter than this are removed (W-7, W-42).
pub const MIN_WALL_LENGTH: f64 = 1.0;
/// `split_on_tee` unless the caller says otherwise.
pub const SPLIT_ON_TEE_DEFAULT: bool = true;

/// Corners of walls meeting at less than this sine (about 6 degrees) are not
/// solved by line intersection; the end just snaps onto the other end.
const MIN_SIN: f64 = 0.1;
/// Positions closer than this are the same point.
const SAME: f64 = 1e-9;
/// Passes of the fix-up loop before giving up (a pass can enable the next).
const MAX_PASSES: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectOptions {
    /// Split the through wall where another wall ends on it (T) and both
    /// walls where they cross (X). When false the T is only snapped onto the
    /// centerline and crossings are left alone (Chief's W-35/W-36 behavior).
    pub split_on_tee: bool,
    /// Smallest connect distance in inches (Chief's "Connect Distance
    /// Minimum" in Default Settings ▸ Walls). Thicker walls use their own
    /// thickness instead.
    pub connect_distance_min: f64,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            split_on_tee: SPLIT_ON_TEE_DEFAULT,
            connect_distance_min: MIN_CONNECT_DISTANCE,
        }
    }
}

impl ConnectOptions {
    /// The options Daniel's plan defaults ask for.
    pub fn from_defaults(d: &plan_core::defaults::PlanDefaults) -> Self {
        Self {
            split_on_tee: d.walls_connect.split_on_tee,
            connect_distance_min: d.walls_connect.connect_distance_min,
        }
    }
}

/// How close an end must be to another wall to be connected to it: the
/// wall's thickness, at least 6".
pub fn connect_distance(w: &Wall) -> f64 {
    connect_distance_with(w, MIN_CONNECT_DISTANCE)
}

/// The connect distance with a plan-specific minimum.
pub fn connect_distance_with(w: &Wall, min: f64) -> f64 {
    w.thickness.max(min)
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

fn straight(w: &Wall) -> bool {
    !w.is_curved() && w.length() >= MIN_WALL_LENGTH
}

fn set_exact(project: &mut Project, floor: usize, id: Id, e: WallEnd, to: Point) {
    if let Some(w) = project.floors[floor].wall_mut(id) {
        match e {
            WallEnd::Start => w.start = to,
            WallEnd::End => w.end = to,
        }
    }
}

// ----- the pieces -----

/// Removes walls shorter than [`MIN_WALL_LENGTH`]. Returns how many.
fn remove_degenerate(project: &mut Project, floor: usize) -> usize {
    let ids: Vec<Id> = project.floors[floor]
        .walls
        .iter()
        .filter(|w| w.path_length() < MIN_WALL_LENGTH)
        .map(|w| w.id)
        .collect();
    for id in &ids {
        project.remove_wall(floor, *id);
    }
    ids.len()
}

/// The conditions `Project::join_collinear_walls` requires, so a merge is
/// never started that it would refuse.
fn can_merge(a: &Wall, b: &Wall) -> bool {
    !a.is_curved()
        && !b.is_curved()
        && (a.thickness - b.thickness).abs() <= 1e-9
        && (a.height - b.height).abs() <= 1e-9
        && a.kind == b.kind
        && a.class == b.class
        && a.layer == b.layer
        && a.wall_type == b.wall_type
        && a.flags == b.flags
        && a.exterior_side == b.exterior_side
}

/// Merges wall `id` with walls of the same type that lie on top of it
/// (collinear and overlapping, W-42). Walls that merely touch end to end are
/// left alone (W-33). Returns the number of merges.
fn merge_overlaps(project: &mut Project, floor: usize, id: Id) -> usize {
    let mut n = 0;
    for _ in 0..MAX_PASSES {
        let Some(w) = project.floors[floor].wall(id).cloned() else {
            return n;
        };
        if !straight(&w) {
            return n;
        }
        let wd = w.direction();
        // (other, lo, hi, other length, w.start is the low end)
        let found = project.floors[floor].walls.iter().find_map(|o| {
            if o.id == id || !straight(o) || !can_merge(&w, o) {
                return None;
            }
            let od = o.direction();
            if od.cross(wd).abs() > 1e-3 {
                return None;
            }
            let off = |p: Point| (p - o.start).cross(od).abs();
            if off(w.start) > JOIN_TOL || off(w.end) > JOIN_TOL {
                return None;
            }
            let a = (w.start - o.start).dot(od);
            let b = (w.end - o.start).dot(od);
            let (lo, hi, l) = (a.min(b), a.max(b), o.length());
            let overlap = hi.min(l) - lo.max(0.0);
            (overlap > JOIN_TOL).then_some((o.id, o.start, o.end, lo, hi, l, a < b))
        });
        let Some((oid, o_start, o_end, lo, hi, l, w_start_low)) = found else {
            return n;
        };
        let has_openings = |pid: Id| {
            project.floors[floor]
                .openings
                .iter()
                .any(|x| x.wall_id == pid)
        };
        if lo >= -JOIN_TOL && hi <= l + JOIN_TOL {
            // `id` lies inside the other wall: it is a duplicate.
            if has_openings(id) {
                return n;
            }
            project.remove_wall(floor, id);
            return n + 1;
        }
        if lo <= JOIN_TOL && hi >= l - JOIN_TOL {
            // The other wall lies inside `id`.
            if has_openings(oid) {
                return n;
            }
            project.remove_wall(floor, oid);
            n += 1;
            continue;
        }
        // Partial overlap: pull the overlapping end of `id` back to the end of
        // the other wall so the two just touch, then join them.
        let (trim_end, to) = if lo < 0.0 {
            (
                if w_start_low {
                    WallEnd::End
                } else {
                    WallEnd::Start
                },
                o_start,
            )
        } else {
            (
                if w_start_low {
                    WallEnd::Start
                } else {
                    WallEnd::End
                },
                o_end,
            )
        };
        ops::set_end(project, floor, id, trim_end, to);
        if project.join_collinear_walls(floor, id, oid).is_some() {
            n += 1;
        }
        return n;
    }
    n
}

/// Corner of wall `id` (end at `p`, other end `far`) and wall `o` (end at `q`,
/// other end `far_o`): the intersection of the two centerlines, if it is a
/// sensible place to move both ends to (neither wall flips or collapses).
fn corner_point(p: Point, far: Point, q: Point, far_o: Point, d: f64) -> Option<Point> {
    let u = (p - far).normalized();
    let v = (q - far_o).normalized();
    if u.cross(v).abs() < MIN_SIN {
        return None;
    }
    let x = ops::line_intersection(p, u, q, v)?;
    let sane =
        |from: Point, dir: Point| (x - from).dot(dir) > 0.0 && x.dist(from) >= MIN_WALL_LENGTH;
    (x.dist(p) <= 6.0 * d && x.dist(q) <= 6.0 * d && sane(far, u) && sane(far_o, v)).then_some(x)
}

/// Connects one end of wall `id` to its neighbours. Returns the number of
/// edits made.
fn connect_end(
    project: &mut Project,
    floor: usize,
    id: Id,
    end: WallEnd,
    opts: &ConnectOptions,
) -> usize {
    let Some(w) = project.floors[floor].wall(id).cloned() else {
        return 0;
    };
    if !straight(&w) {
        return 0;
    }
    let p = end_pos(&w, end);
    let far = end_pos(&w, other_end(end));
    let d = connect_distance_with(&w, opts.connect_distance_min);

    // Nearest other end, and nearest other centerline (away from its ends).
    let mut best_end: Option<(f64, Id, WallEnd, Point)> = None;
    let mut best_int: Option<(f64, Id, Point)> = None;
    for o in &project.floors[floor].walls {
        // A curved wall is a neighbour too: its ends and its arc.
        if o.id == id || o.path_length() < MIN_WALL_LENGTH {
            continue;
        }
        for oe in [WallEnd::Start, WallEnd::End] {
            let q = end_pos(o, oe);
            let dist = p.dist(q);
            if dist <= d && best_end.is_none_or(|b| dist < b.0) {
                best_end = Some((dist, o.id, oe, q));
            }
        }
        let (foot, _) = o.closest_point(p);
        let dist = foot.dist(p);
        if dist <= d
            && foot.dist(o.start) > JOIN_TOL
            && foot.dist(o.end) > JOIN_TOL
            && best_int.is_none_or(|b| dist < b.0)
        {
            best_int = Some((dist, o.id, foot));
        }
    }

    // Already a corner: ends coincide.
    if best_end.is_some_and(|b| b.0 <= SAME) {
        return 0;
    }
    let on_line = best_int.is_some_and(|b| b.0 <= 1e-6);

    if let (Some((_, oid, oe, q)), false) = (best_end, on_line) {
        // (a) Corner: both ends go to the intersection of the centerlines.
        let Some(o) = project.floors[floor].wall(oid).cloned() else {
            return 0;
        };
        let joined_elsewhere = ops::walls_at(project, floor, q, JOIN_TOL, Some(oid))
            .iter()
            .any(|(j, _)| *j != id);
        // A curved neighbour keeps its end: only this wall's end moves to it.
        let x = if joined_elsewhere || o.is_curved() {
            None
        } else {
            corner_point(p, far, q, end_pos(&o, other_end(oe)), d)
        };
        let mut n = 0;
        let target = x.unwrap_or(q);
        if target.dist(p) > SAME || target != p {
            ops::set_end(project, floor, id, end, target);
            set_exact(project, floor, id, end, target);
            n += 1;
        }
        if x.is_some() && target != q {
            ops::set_end(project, floor, oid, oe, target);
            set_exact(project, floor, oid, oe, target);
            n += 1;
        }
        return n;
    }

    let Some((dist, oid, foot)) = best_int else {
        return 0;
    };
    // (b) T: the end goes onto the other wall's centerline, along its own
    // direction when that is sensible, else straight across.
    let Some(o) = project.floors[floor].wall(oid).cloned() else {
        return 0;
    };
    let mut target = foot;
    let u = (p - far).normalized();
    if dist > 1e-6 && !o.is_curved() && u.cross(o.direction()).abs() >= MIN_SIN {
        if let Some(x) = ops::line_intersection(p, u, o.start, o.direction()) {
            let inside = dist_to_segment(x, o.start, o.end) <= 1e-6
                && x.dist(o.start) > JOIN_TOL
                && x.dist(o.end) > JOIN_TOL;
            if inside
                && x.dist(p) <= 3.0 * d
                && (x - far).dot(u) > 0.0
                && x.dist(far) >= MIN_WALL_LENGTH
            {
                target = x;
            }
        }
    }
    let mut n = 0;
    if target != p {
        ops::set_end(project, floor, id, end, target);
        set_exact(project, floor, id, end, target);
        n += 1;
    }
    if opts.split_on_tee {
        if let Some(new) = ops::split_wall_at(project, floor, oid, target) {
            set_exact(project, floor, oid, WallEnd::End, target);
            set_exact(project, floor, new, WallEnd::Start, target);
            n += 1;
        }
    }
    n
}

/// Curved walls: the end of curved wall `id` snaps onto the nearest end of
/// another wall within the connect distance (no miter, no corner solving: the
/// bulge stays and only the chord end moves). Returns the number of edits.
fn snap_curved_end(
    project: &mut Project,
    floor: usize,
    id: Id,
    end: WallEnd,
    opts: &ConnectOptions,
) -> usize {
    let Some(w) = project.floors[floor].wall(id).cloned() else {
        return 0;
    };
    if !w.is_curved() {
        return 0;
    }
    let p = end_pos(&w, end);
    let far = end_pos(&w, other_end(end));
    let d = connect_distance_with(&w, opts.connect_distance_min);
    let best = project.floors[floor]
        .walls
        .iter()
        .filter(|o| o.id != id)
        .flat_map(|o| [end_pos(o, WallEnd::Start), end_pos(o, WallEnd::End)])
        .map(|q| (p.dist(q), q))
        // Never collapse the chord.
        .filter(|(dist, q)| *dist <= d && q.dist(far) >= MIN_WALL_LENGTH)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    match best {
        Some((dist, q)) if dist > SAME => {
            set_exact(project, floor, id, end, q);
            1
        }
        Some(_) => 0,
        None => snap_curved_tee(project, floor, id, end, &w, opts),
    }
}

/// A curved wall's end that is near no wall end but near another wall's
/// centerline (a straight wall's line or another arc) goes onto that
/// centerline and, with `split_on_tee`, cuts the wall there: a T. The miter
/// is the join's (`plan_core::joins`): the arc meets the host's face along
/// its tangent. Returns the number of edits.
fn snap_curved_tee(
    project: &mut Project,
    floor: usize,
    id: Id,
    end: WallEnd,
    w: &Wall,
    opts: &ConnectOptions,
) -> usize {
    let p = end_pos(w, end);
    let far = end_pos(w, other_end(end));
    let d = connect_distance_with(w, opts.connect_distance_min);
    let best = project.floors[floor]
        .walls
        .iter()
        .filter(|o| o.id != id && o.path_length() >= MIN_WALL_LENGTH)
        .filter_map(|o| {
            let (foot, _) = o.closest_point(p);
            let dist = foot.dist(p);
            (dist <= d
                && foot.dist(o.start) > JOIN_TOL
                && foot.dist(o.end) > JOIN_TOL
                && foot.dist(far) >= MIN_WALL_LENGTH)
                .then_some((dist, o.id, foot))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((dist, oid, foot)) = best else {
        return 0;
    };
    let mut n = 0;
    if dist > SAME {
        set_exact(project, floor, id, end, foot);
        n += 1;
    }
    if opts.split_on_tee {
        if let Some(new) = ops::split_wall_at(project, floor, oid, foot) {
            set_exact(project, floor, oid, WallEnd::End, foot);
            set_exact(project, floor, new, WallEnd::Start, foot);
            n += 1;
        }
    }
    n
}

/// (c) Cuts wall `id` and every wall it crosses at the crossings (X
/// junctions, W-36). Returns the number of crossings cut.
fn split_crossings(project: &mut Project, floor: usize, id: Id) -> usize {
    let mut n = 0;
    let mut work = vec![id];
    let mut guard = 0;
    while let Some(cur) = work.pop() {
        guard += 1;
        if guard > 64 {
            break;
        }
        let Some(w) = project.floors[floor].wall(cur).cloned() else {
            continue;
        };
        if !straight(&w) {
            continue;
        }
        let hit = project.floors[floor].walls.iter().find_map(|o| {
            if o.id == cur || !straight(o) {
                return None;
            }
            let (t, _) = segment_intersection(w.start, w.end, o.start, o.end)?;
            let x = Point::lerp(w.start, w.end, t);
            let interior = [w.start, w.end, o.start, o.end]
                .iter()
                .all(|e| e.dist(x) > JOIN_TOL);
            interior.then_some((o.id, x))
        });
        let Some((oid, x)) = hit else { continue };
        let split_o = ops::split_wall_at(project, floor, oid, x);
        let split_c = ops::split_wall_at(project, floor, cur, x);
        if split_o.is_none() && split_c.is_none() {
            continue;
        }
        if let Some(new) = split_o {
            set_exact(project, floor, oid, WallEnd::End, x);
            set_exact(project, floor, new, WallEnd::Start, x);
        }
        if let Some(new) = split_c {
            set_exact(project, floor, cur, WallEnd::End, x);
            set_exact(project, floor, new, WallEnd::Start, x);
            work.push(new);
        }
        work.push(cur);
        n += 1;
    }
    n
}

// ----- entry points -----

/// Connects wall `id` to the plan: corners, Ts, crossings, collinear
/// duplicates and zero-length walls (see the module docs). Returns the number
/// of edits made; 0 means the wall was already connected.
pub fn auto_connect_project(
    project: &mut Project,
    floor: usize,
    id: Id,
    opts: &ConnectOptions,
) -> usize {
    let mut total = remove_degenerate(project, floor);
    for _ in 0..MAX_PASSES {
        let mut n = merge_overlaps(project, floor, id);
        if project.floors[floor].wall(id).is_none() {
            total += n;
            break;
        }
        for end in [WallEnd::Start, WallEnd::End] {
            n += connect_end(project, floor, id, end, opts);
            n += snap_curved_end(project, floor, id, end, opts);
        }
        if opts.split_on_tee {
            n += split_crossings(project, floor, id);
        }
        n += remove_degenerate(project, floor);
        total += n;
        if n == 0 {
            break;
        }
    }
    total
}

/// Runs [`auto_connect_project`] on every wall of the floor until nothing
/// changes (W-41, W-42). Returns the number of repairs.
pub fn fix_all_connections_project(
    project: &mut Project,
    floor: usize,
    opts: &ConnectOptions,
) -> usize {
    let mut total = remove_degenerate(project, floor);
    for _ in 0..MAX_PASSES {
        let ids: Vec<Id> = project.floors[floor].walls.iter().map(|w| w.id).collect();
        let mut n = 0;
        for id in ids {
            if project.floors[floor].wall(id).is_some() {
                n += auto_connect_project(project, floor, id, opts);
            }
        }
        total += n;
        if n == 0 {
            break;
        }
    }
    total
}

/// Connects wall `wall_id` on the active floor and recomputes rooms and
/// outlines. Call it after a wall was created or one of its ends was moved or
/// released, inside the caller's undo step. Returns the number of edits.
pub fn auto_connect(cx: &mut EditorContext, wall_id: Id) -> usize {
    auto_connect_with(cx, wall_id, &ConnectOptions::from_defaults(&cx.defaults))
}

/// [`auto_connect`] with explicit options.
pub fn auto_connect_with(cx: &mut EditorContext, wall_id: Id, opts: &ConnectOptions) -> usize {
    let n = auto_connect_project(&mut cx.project, cx.floor, wall_id, opts);
    cx.mark_dirty();
    cx.refresh();
    n
}

/// Fix Wall Connections over every wall of the active floor, then recomputes
/// rooms and outlines. Does not open an undo step.
pub fn fix_all_connections(cx: &mut EditorContext) -> usize {
    let n = fix_all_connections_project(
        &mut cx.project,
        cx.floor,
        &ConnectOptions::from_defaults(&cx.defaults),
    );
    cx.mark_dirty();
    cx.refresh();
    n
}

/// The Fix Wall Connections button of the Edit toolbar (W-41): repairs the
/// selected walls, or every wall when none is selected, as one undo step.
/// The dispatch in `EditorContext::apply_edit_action` calls this.
pub fn fix_wall_connections_action(cx: &mut EditorContext) {
    let selected: Vec<Id> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            super::ObjectRef::Wall(id) => Some(*id),
            _ => None,
        })
        .collect();
    cx.begin_change("Fix Wall Connections");
    let opts = ConnectOptions::from_defaults(&cx.defaults);
    let n = if selected.is_empty() {
        fix_all_connections_project(&mut cx.project, cx.floor, &opts)
    } else {
        let mut n = 0;
        for _ in 0..MAX_PASSES {
            let mut pass = 0;
            for id in &selected {
                if cx.project.floors[cx.floor].wall(*id).is_some() {
                    pass += auto_connect_project(&mut cx.project, cx.floor, *id, &opts);
                }
            }
            n += pass;
            if pass == 0 {
                break;
            }
        }
        n
    };
    if n == 0 {
        cx.cancel_change();
        cx.status = "Fix Wall Connections: all connections are already clean".into();
    } else {
        cx.mark_dirty();
        cx.refresh();
        cx.status = format!(
            "Fix Wall Connections: {n} repair{}",
            if n == 1 { "" } else { "s" }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{detect_rooms, OpeningKind, WallKind};

    fn wall(p: &mut Project, a: (f64, f64), b: (f64, f64)) -> Id {
        p.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            7.625,
            100.0,
            WallKind::Exterior,
        )
    }

    fn opts() -> ConnectOptions {
        ConnectOptions::default()
    }

    fn at(p: &Project, id: Id) -> (Point, Point) {
        let w = p.floors[0].wall(id).unwrap();
        (w.start, w.end)
    }

    #[test]
    fn sloppy_rectangle_closes_to_exact_corners_and_one_room() {
        let mut p = Project::new("t");
        // Every end 3" short of or beyond its corner.
        let w1 = wall(&mut p, (0.0, 0.0), (237.0, 0.0));
        let w2 = wall(&mut p, (240.0, -3.0), (240.0, 147.0));
        let w3 = wall(&mut p, (243.0, 144.0), (-3.0, 144.0));
        let w4 = wall(&mut p, (0.0, 147.0), (0.0, 3.0));
        for id in [w1, w2, w3, w4] {
            auto_connect_project(&mut p, 0, id, &opts());
        }
        assert_eq!(p.floors[0].walls.len(), 4);
        let c = |x: f64, y: f64| Point::new(x, y);
        assert_eq!(at(&p, w1), (c(0.0, 0.0), c(240.0, 0.0)));
        assert_eq!(at(&p, w2), (c(240.0, 0.0), c(240.0, 144.0)));
        assert_eq!(at(&p, w3), (c(240.0, 144.0), c(0.0, 144.0)));
        assert_eq!(at(&p, w4), (c(0.0, 144.0), c(0.0, 0.0)));
        assert_eq!(detect_rooms(&p.floors[0].walls, 0.5).len(), 1);
        // Idempotent.
        for id in [w1, w2, w3, w4] {
            assert_eq!(auto_connect_project(&mut p, 0, id, &opts()), 0);
        }
    }

    #[test]
    fn end_near_a_centerline_becomes_a_clean_tee() {
        let mut p = Project::new("t");
        let through = wall(&mut p, (0.0, 0.0), (240.0, 0.0));
        let stem = wall(&mut p, (100.0, 120.0), (100.0, 4.0));
        assert!(auto_connect_project(&mut p, 0, stem, &opts()) > 0);
        let f = &p.floors[0];
        assert_eq!(f.walls.len(), 3);
        assert_eq!(at(&p, stem).1, Point::new(100.0, 0.0));
        assert_eq!(at(&p, through).1, Point::new(100.0, 0.0));
        let tail = f
            .walls
            .iter()
            .find(|w| w.id != through && w.id != stem)
            .unwrap();
        assert_eq!(
            (tail.start, tail.end),
            (Point::new(100.0, 0.0), Point::new(240.0, 0.0))
        );
        assert_eq!(auto_connect_project(&mut p, 0, stem, &opts()), 0);
    }

    #[test]
    fn tee_without_split_keeps_the_through_wall_whole() {
        let mut p = Project::new("t");
        wall(&mut p, (0.0, 0.0), (240.0, 0.0));
        let stem = wall(&mut p, (100.0, 120.0), (100.0, 4.0));
        let o = ConnectOptions {
            split_on_tee: false,
            ..ConnectOptions::default()
        };
        assert!(auto_connect_project(&mut p, 0, stem, &o) > 0);
        assert_eq!(p.floors[0].walls.len(), 2);
        assert_eq!(at(&p, stem).1, Point::new(100.0, 0.0));
        assert_eq!(auto_connect_project(&mut p, 0, stem, &o), 0);
    }

    #[test]
    fn crossing_walls_split_into_four_sharing_one_point() {
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (200.0, 0.0));
        let b = wall(&mut p, (100.0, -50.0), (100.0, 50.0));
        assert!(auto_connect_project(&mut p, 0, b, &opts()) > 0);
        let f = &p.floors[0];
        assert_eq!(f.walls.len(), 4);
        let x = Point::new(100.0, 0.0);
        let at_x = f
            .walls
            .iter()
            .flat_map(|w| [w.start, w.end])
            .filter(|e| *e == x)
            .count();
        assert_eq!(at_x, 4);
        assert!(f.wall(a).is_some());
        assert_eq!(auto_connect_project(&mut p, 0, b, &opts()), 0);
    }

    #[test]
    fn overlapping_collinear_walls_merge_but_touching_ones_do_not() {
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (100.0, 0.0));
        let b = wall(&mut p, (60.0, 0.0), (200.0, 0.0));
        assert!(auto_connect_project(&mut p, 0, b, &opts()) > 0);
        assert_eq!(p.floors[0].walls.len(), 1);
        assert_eq!(at(&p, b), (Point::new(0.0, 0.0), Point::new(200.0, 0.0)));
        assert!(p.floors[0].wall(a).is_none());
        // End to end stays two walls (W-33).
        let mut q = Project::new("t");
        wall(&mut q, (0.0, 0.0), (100.0, 0.0));
        let n = wall(&mut q, (100.0, 0.0), (200.0, 0.0));
        assert_eq!(auto_connect_project(&mut q, 0, n, &opts()), 0);
        assert_eq!(q.floors[0].walls.len(), 2);
    }

    #[test]
    fn duplicate_wall_inside_another_is_dropped() {
        let mut p = Project::new("t");
        wall(&mut p, (0.0, 0.0), (200.0, 0.0));
        let d = wall(&mut p, (50.0, 0.0), (150.0, 0.0));
        auto_connect_project(&mut p, 0, d, &opts());
        assert_eq!(p.floors[0].walls.len(), 1);
    }

    #[test]
    fn collinear_merge_moves_openings() {
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (100.0, 0.0));
        let b = wall(&mut p, (80.0, 0.0), (200.0, 0.0));
        let door = p.add_opening(0, b, 80.0, OpeningKind::Door).unwrap();
        auto_connect_project(&mut p, 0, b, &opts());
        let o = p.floors[0].openings.iter().find(|o| o.id == door).unwrap();
        assert_eq!(o.wall_id, b);
        assert!(p.floors[0].wall(a).is_none());
    }

    #[test]
    fn no_wall_shorter_than_an_inch_survives() {
        let mut p = Project::new("t");
        wall(&mut p, (0.0, 0.0), (100.0, 0.0));
        let tiny = wall(&mut p, (50.0, 20.0), (50.5, 20.0));
        wall(&mut p, (0.0, 50.0), (100.0, 50.0));
        auto_connect_project(&mut p, 0, tiny, &opts());
        assert_eq!(p.floors[0].walls.len(), 2);
        assert!(p.floors[0]
            .walls
            .iter()
            .all(|w| w.length() >= MIN_WALL_LENGTH));
    }

    #[test]
    fn fix_all_closes_three_near_misses_and_is_idempotent() {
        let mut p = Project::new("t");
        // A U of walls plus a free wall, each gap 2-5".
        let a = wall(&mut p, (0.0, 0.0), (120.0, 0.0));
        let b = wall(&mut p, (123.0, 4.0), (123.0, 120.0)); // corner miss
        let c = wall(&mut p, (120.0, 124.0), (0.0, 124.0)); // corner miss
        let d = wall(&mut p, (0.0, 120.0), (0.0, 5.0)); // corner miss
        let n = fix_all_connections_project(&mut p, 0, &opts());
        assert!(n >= 3);
        let f = &p.floors[0];
        assert_eq!(f.walls.len(), 4);
        // All four corners are now exactly shared.
        let ends: Vec<Point> = f.walls.iter().flat_map(|w| [w.start, w.end]).collect();
        for e in &ends {
            assert_eq!(ends.iter().filter(|o| *o == e).count(), 2);
        }
        let _ = (a, b, c, d);
        assert_eq!(detect_rooms(&f.walls, 0.5).len(), 1);
        assert_eq!(fix_all_connections_project(&mut p, 0, &opts()), 0);
    }

    #[test]
    fn corner_end_joined_elsewhere_just_snaps() {
        // A third wall ending near an existing corner joins it (3-way).
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (100.0, 0.0));
        let b = wall(&mut p, (100.0, 0.0), (100.0, 80.0));
        let c = wall(&mut p, (100.0, -50.0), (103.0, -3.0));
        let _ = (a, b);
        auto_connect_project(&mut p, 0, c, &opts());
        assert_eq!(at(&p, c).1, Point::new(100.0, 0.0));
        assert_eq!(at(&p, a).1, Point::new(100.0, 0.0));
        assert_eq!(at(&p, b).0, Point::new(100.0, 0.0));
    }

    #[test]
    fn tee_split_refuses_and_still_snaps_when_a_door_straddles() {
        let mut p = Project::new("t");
        let through = wall(&mut p, (0.0, 0.0), (240.0, 0.0));
        p.add_opening(0, through, 100.0, OpeningKind::Door).unwrap();
        let stem = wall(&mut p, (100.0, 120.0), (100.0, 3.0));
        auto_connect_project(&mut p, 0, stem, &opts());
        assert_eq!(at(&p, stem).1, Point::new(100.0, 0.0));
        assert_eq!(p.floors[0].walls.len(), 2);
    }

    #[test]
    fn editor_wrappers_refresh_rooms_and_fix_action_is_one_undo_step() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let t = cx.wall_thickness(WallKind::Exterior);
        let ids: Vec<Id> = [
            ((0.0, 0.0), (117.0, 0.0)),
            ((120.0, -2.0), (120.0, 96.0)),
            ((120.0, 96.0), (0.0, 96.0)),
            ((0.0, 96.0), (0.0, 0.0)),
        ]
        .iter()
        .map(|(a, b)| {
            cx.project.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                t,
                100.0,
                WallKind::Exterior,
            )
        })
        .collect();
        cx.refresh();
        assert!(cx.rooms.is_empty());
        cx.selection.clear();
        fix_wall_connections_action(&mut cx);
        assert_eq!(cx.rooms.len(), 1);
        assert_eq!(cx.undo_label(), Some("Fix Wall Connections"));
        assert_eq!(cx.undo().as_deref(), Some("Fix Wall Connections"));
        assert!(cx.rooms.is_empty());
        // Clean plan: no undo step is left behind.
        cx.redo();
        let before = cx.undo_label().map(str::to_string);
        fix_wall_connections_action(&mut cx);
        assert_eq!(cx.undo_label().map(str::to_string), before);
        assert!(cx.status.contains("already clean"));
        let _ = ids;
    }

    #[test]
    fn walls_of_different_class_never_merge() {
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (200.0, 0.0));
        let b = wall(&mut p, (100.0, 0.0), (300.0, 0.0));
        p.floors[0].wall_mut(b).unwrap().class = plan_core::WallClass::Foundation;
        let (wa, wb) = (
            p.floors[0].wall(a).unwrap().clone(),
            p.floors[0].wall(b).unwrap().clone(),
        );
        assert!(!can_merge(&wa, &wb));
        assert_eq!(merge_overlaps(&mut p, 0, a), 0);
        assert_eq!(p.floors[0].walls.len(), 2);
        // The same class still merges.
        p.floors[0].wall_mut(b).unwrap().class = plan_core::WallClass::Standard;
        let (wa, wb) = (
            p.floors[0].wall(a).unwrap().clone(),
            p.floors[0].wall(b).unwrap().clone(),
        );
        assert!(can_merge(&wa, &wb));
    }

    #[test]
    fn curved_wall_ends_snap_to_the_nearest_wall_end() {
        let mut p = Project::new("t");
        let straight = wall(&mut p, (0.0, 0.0), (200.0, 0.0));
        let other = wall(&mut p, (400.0, 0.0), (400.0, 200.0));
        // A curve whose ends are 4" short of both walls' ends.
        let curved = wall(&mut p, (204.0, 0.0), (396.0, 0.0));
        p.floors[0].wall_mut(curved).unwrap().curve = Some(plan_core::WallCurve { bulge: 40.0 });
        assert!(auto_connect_project(&mut p, 0, curved, &opts()) > 0);
        let (s, e) = at(&p, curved);
        assert_eq!((s, e), (Point::new(200.0, 0.0), Point::new(400.0, 0.0)));
        // No miter: the neighbours did not move, and the wall stays curved.
        assert_eq!(
            at(&p, straight),
            (Point::new(0.0, 0.0), Point::new(200.0, 0.0))
        );
        assert_eq!(
            at(&p, other),
            (Point::new(400.0, 0.0), Point::new(400.0, 200.0))
        );
        assert!(p.floors[0].wall(curved).unwrap().is_curved());
        // Idempotent, and an end farther than the connect distance stays put.
        assert_eq!(auto_connect_project(&mut p, 0, curved, &opts()), 0);
        let far = wall(&mut p, (600.0, 0.0), (800.0, 0.0));
        p.floors[0].wall_mut(far).unwrap().curve = Some(plan_core::WallCurve { bulge: 30.0 });
        assert_eq!(auto_connect_project(&mut p, 0, far, &opts()), 0);
        assert_eq!(
            at(&p, far),
            (Point::new(600.0, 0.0), Point::new(800.0, 0.0))
        );
    }

    #[test]
    fn a_straight_wall_snaps_onto_the_end_of_a_curved_one() {
        let mut p = Project::new("t");
        let arc = wall(&mut p, (0.0, 0.0), (200.0, 0.0));
        p.floors[0].wall_mut(arc).unwrap().curve = Some(plan_core::WallCurve { bulge: 40.0 });
        // Drawn 4" off the arc's end: the straight wall's start goes to it.
        let line = wall(&mut p, (204.0, 2.0), (204.0, 150.0));
        assert!(auto_connect_project(&mut p, 0, line, &opts()) > 0);
        assert_eq!(at(&p, line).0, Point::new(200.0, 0.0));
        assert_eq!(at(&p, arc), (Point::new(0.0, 0.0), Point::new(200.0, 0.0)));
        assert!(p.floors[0].wall(arc).unwrap().is_curved());
        assert_eq!(auto_connect_project(&mut p, 0, line, &opts()), 0);
    }

    #[test]
    fn a_wall_ending_near_an_arc_makes_a_tee_on_it_and_splits_it() {
        let mut p = Project::new("t");
        let arc = wall(&mut p, (0.0, 0.0), (200.0, 0.0));
        p.floors[0].wall_mut(arc).unwrap().curve = Some(plan_core::WallCurve { bulge: 40.0 });
        // 3" short of the arc's apex (100, 40).
        let line = wall(&mut p, (100.0, 150.0), (100.0, 43.0));
        assert!(auto_connect_project(&mut p, 0, line, &opts()) > 0);
        let end = at(&p, line).1;
        assert!(end.dist(Point::new(100.0, 40.0)) < 1e-6, "{end:?}");
        let walls = &p.floors[0].walls;
        assert_eq!(walls.len(), 3, "the arc is cut where the wall ends on it");
        assert_eq!(walls.iter().filter(|w| w.is_curved()).count(), 2);
        assert_eq!(auto_connect_project(&mut p, 0, line, &opts()), 0);
        // Without splitting it only snaps onto the arc.
        let mut q = Project::new("t");
        let arc = wall(&mut q, (0.0, 0.0), (200.0, 0.0));
        q.floors[0].wall_mut(arc).unwrap().curve = Some(plan_core::WallCurve { bulge: 40.0 });
        let line = wall(&mut q, (100.0, 150.0), (100.0, 43.0));
        let keep = ConnectOptions {
            split_on_tee: false,
            ..opts()
        };
        assert!(auto_connect_project(&mut q, 0, line, &keep) > 0);
        assert!(at(&q, line).1.dist(Point::new(100.0, 40.0)) < 1e-6);
        assert_eq!(q.floors[0].walls.len(), 2);
    }

    #[test]
    fn a_curved_wall_ending_near_a_straight_wall_makes_a_tee() {
        let mut p = Project::new("t");
        let host = wall(&mut p, (0.0, 0.0), (300.0, 0.0));
        let arc = wall(&mut p, (150.0, 100.0), (150.0, 4.0));
        p.floors[0].wall_mut(arc).unwrap().curve = Some(plan_core::WallCurve { bulge: 20.0 });
        assert!(auto_connect_project(&mut p, 0, arc, &opts()) > 0);
        assert_eq!(at(&p, arc).1, Point::new(150.0, 0.0));
        assert!(p.floors[0].wall(arc).unwrap().is_curved());
        assert_eq!(p.floors[0].walls.len(), 3, "the host is cut at the tee");
        assert!(p.floors[0].wall(host).is_some());
        assert_eq!(auto_connect_project(&mut p, 0, arc, &opts()), 0);
    }

    #[test]
    fn arcs_that_meet_tangent_join_through_and_a_kink_is_a_corner() {
        use plan_core::joins::{wall_end_joins, ConnectionKind};
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (200.0, 0.0));
        p.floors[0].wall_mut(a).unwrap().curve = Some(plan_core::WallCurve { bulge: 30.0 });
        let wa = p.floors[0].wall(a).unwrap().clone();
        let tangent = wa.curve.unwrap().tangent_at_end(wa.start, wa.end);
        let end = Point::new(400.0, -60.0);
        let b = wall(&mut p, (200.0, 0.0), (end.x, end.y));
        let curve = plan_core::WallCurve::tangent_to(Point::new(200.0, 0.0), end, tangent);
        p.floors[0].wall_mut(b).unwrap().curve = curve;
        // Already joined: nothing to do, and the join is a through join.
        assert_eq!(auto_connect_project(&mut p, 0, b, &opts()), 0);
        let walls = &p.floors[0].walls;
        let i = walls.iter().position(|w| w.id == a).unwrap();
        assert_eq!(
            wall_end_joins(walls, i, WallEnd::End, 0.5),
            vec![(1, ConnectionKind::Through)]
        );
        // Bend the second arc off the tangent: the same joint is a corner.
        p.floors[0].wall_mut(b).unwrap().curve = Some(plan_core::WallCurve { bulge: 25.0 });
        let walls = &p.floors[0].walls;
        assert_eq!(
            wall_end_joins(walls, i, WallEnd::End, 0.5),
            vec![(1, ConnectionKind::Corner)]
        );
        // Drawn 4" off, the second arc's start snaps on and keeps its curve.
        let c = wall(&mut p, (204.0, 0.0), (400.0, 80.0));
        p.floors[0].wall_mut(c).unwrap().curve = Some(plan_core::WallCurve { bulge: 15.0 });
        assert!(auto_connect_project(&mut p, 0, c, &opts()) > 0);
        assert_eq!(at(&p, c).0, Point::new(200.0, 0.0));
        assert!(p.floors[0].wall(c).unwrap().is_curved());
    }

    // ----- three-way and four-way junctions, crossings (W-34, W-36, W-103) -----

    fn outline_of(p: &Project, id: Id) -> Vec<Point> {
        plan_core::wall_outlines(&p.floors[0].walls, 0.5)
            .into_iter()
            .find(|o| o.wall_id == id)
            .unwrap()
            .polygon
    }

    #[test]
    fn a_wall_ended_on_a_joined_corner_makes_a_three_way_and_keeps_the_pair() {
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (200.0, 0.0));
        let b = wall(&mut p, (200.0, 0.0), (200.0, 150.0));
        // Drawn from the east, ending 3" short of the corner of a and b.
        let c = wall(&mut p, (400.0, 0.0), (203.0, 0.0));
        assert!(auto_connect_project(&mut p, 0, c, &opts()) > 0);
        assert_eq!(at(&p, c).1, Point::new(200.0, 0.0), "snaps onto the corner");
        // The pair that was joined stays joined and untouched.
        assert_eq!(at(&p, a), (Point::new(0.0, 0.0), Point::new(200.0, 0.0)));
        assert_eq!(
            at(&p, b),
            (Point::new(200.0, 0.0), Point::new(200.0, 150.0))
        );
        assert_eq!(p.floors[0].walls.len(), 3);
        // a and c are one line: they run through and b butts their face.
        let ob = outline_of(&p, b);
        assert!((ob[0].y - 3.8125).abs() < 1e-9, "{ob:?}");
        assert!((ob[3].y - 3.8125).abs() < 1e-9, "{ob:?}");
        for id in [a, b, c] {
            assert_eq!(auto_connect_project(&mut p, 0, id, &opts()), 0);
        }
    }

    #[test]
    fn a_tee_in_the_middle_of_a_wall_makes_a_three_way_with_the_split_halves() {
        let mut p = Project::new("t");
        let through = wall(&mut p, (0.0, 0.0), (240.0, 0.0));
        let stem = wall(&mut p, (120.0, 100.0), (120.0, 2.0));
        auto_connect_project(&mut p, 0, stem, &opts());
        assert_eq!(p.floors[0].walls.len(), 3);
        // The halves run through with square ends; the stem butts the face.
        let halves: Vec<Id> = p.floors[0]
            .walls
            .iter()
            .filter(|w| w.id != stem)
            .map(|w| w.id)
            .collect();
        assert!(halves.contains(&through));
        let os = outline_of(&p, stem);
        // The stem's end is its start-or-end point at the junction, now on the face.
        let on_face = os.iter().filter(|q| (q.y - 3.8125).abs() < 1e-9).count();
        assert_eq!(on_face, 2, "{os:?}");
    }

    #[test]
    fn crossing_walls_are_cut_into_a_four_way_where_the_exterior_wall_runs_through() {
        let mut p = Project::new("t");
        let a = wall(&mut p, (0.0, 0.0), (200.0, 0.0));
        let b = wall(&mut p, (100.0, -80.0), (100.0, 80.0));
        // a is exterior, b interior; b is thicker than a would be otherwise.
        p.floors[0].wall_mut(a).unwrap().kind = WallKind::Exterior;
        {
            let w = p.floors[0].wall_mut(b).unwrap();
            w.kind = WallKind::Interior;
            w.thickness = 4.5;
        }
        assert!(auto_connect_project(&mut p, 0, b, &opts()) > 0);
        let f = &p.floors[0];
        assert_eq!(f.walls.len(), 4);
        let at_x = |w: &Wall| w.start == Point::new(100.0, 0.0) || w.end == Point::new(100.0, 0.0);
        assert!(f.walls.iter().all(at_x));
        // The exterior halves keep square ends at the crossing...
        let outlines = plan_core::wall_outlines(&f.walls, 0.5);
        for (w, o) in f.walls.iter().zip(&outlines) {
            if w.kind == WallKind::Exterior {
                for (q, r) in o.polygon.iter().zip(w.footprint().iter()) {
                    assert!(q.dist(*r) < 1e-9, "{:?}", o.polygon);
                }
            } else {
                // ...and the interior halves stop on its faces (3.8125 = half of 7 5/8).
                let near: Vec<f64> = o
                    .polygon
                    .iter()
                    .filter(|q| q.dist(Point::new(100.0, 0.0)) < 6.0)
                    .map(|q| q.y.abs())
                    .collect();
                assert_eq!(near.len(), 2, "{:?}", o.polygon);
                assert!(near.iter().all(|y| (y - 3.8125).abs() < 1e-9), "{near:?}");
            }
        }
    }

    #[test]
    fn crossing_walls_stay_whole_without_split_on_tee() {
        let mut p = Project::new("t");
        wall(&mut p, (0.0, 0.0), (200.0, 0.0));
        let b = wall(&mut p, (100.0, -50.0), (100.0, 50.0));
        let o = ConnectOptions {
            split_on_tee: false,
            ..ConnectOptions::default()
        };
        assert_eq!(auto_connect_project(&mut p, 0, b, &o), 0);
        assert_eq!(p.floors[0].walls.len(), 2);
        // Turning the option on cuts them, and the outlines are a clean four-way.
        assert!(auto_connect_project(&mut p, 0, b, &opts()) > 0);
        assert_eq!(p.floors[0].walls.len(), 4);
        let outlines = plan_core::wall_outlines(&p.floors[0].walls, 0.5);
        let areas: f64 = outlines
            .iter()
            .map(|o| plan_core::geometry::polygon_area(&o.polygon).abs())
            .sum();
        // 200 x 7.625 and 100 x 7.625 less the doubly covered middle square.
        let cross = 200.0 * 7.625 + 100.0 * 7.625 - 7.625 * 7.625;
        assert!((areas - cross).abs() < 1e-6, "{areas} vs {cross}");
    }

    #[test]
    fn a_five_way_junction_still_connects_and_is_stable() {
        let mut p = Project::new("t");
        let ids: Vec<Id> = (0..5)
            .map(|k| {
                let t = (k as f64) * std::f64::consts::TAU / 5.0;
                wall(&mut p, (0.0, 0.0), (150.0 * t.cos(), 150.0 * t.sin()))
            })
            .collect();
        for id in &ids {
            assert_eq!(auto_connect_project(&mut p, 0, *id, &opts()), 0);
        }
        let outlines = plan_core::wall_outlines(&p.floors[0].walls, 0.5);
        assert_eq!(outlines.len(), 5);
        assert!(outlines
            .iter()
            .all(|o| o.polygon.iter().all(|q| q.x.is_finite() && q.y.is_finite())));
    }
}
