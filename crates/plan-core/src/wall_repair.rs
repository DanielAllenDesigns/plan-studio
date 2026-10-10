//! Wall connection repair (W-132, W-133, W-135, W-136; manual pp. 386-390):
//! which walls are a little off an allowed angle, which wall ends connect to
//! nothing, the Fix Off Angle Wall geometry, and the Ignore / Reset
//! Notification Icons switches. The plan view marks these walls; the editor
//! (`editor/wall_edit.rs`, `dialogs/fix_connections.rs`) offers the fixes.

use crate::defaults::WallTypeDef;
use crate::geometry::Point;
use crate::joins::main_layer_lines;
use crate::model::{Id, Project, Wall, WallEnd};

/// A wall at least this far from every allowed angle is drawn on purpose and
/// carries no off-angle icon (judgment: DECISIONS WR1).
pub const OFF_ANGLE_MAX_DEG: f64 = 5.0;
/// A wall closer than this to an allowed angle is on it.
pub const OFF_ANGLE_MIN_DEG: f64 = 0.05;
/// A wall end this close to another wall's end or centerline is connected.
pub const TOUCH_TOL: f64 = 0.5;
/// Shorter walls are not judged.
const MIN_LEN: f64 = 1.0;

/// Which part of the wall stays put when it is rotated to a new angle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixLock {
    Start,
    Center,
    End,
}

/// The direction of a straight wall, degrees counter-clockwise from east in
/// `[0, 360)`.
pub fn wall_angle_deg(w: &Wall) -> f64 {
    (w.end - w.start).angle().to_degrees().rem_euclid(360.0)
}

/// The allowed angle nearest to `angle` (degrees). `allowed` lists angles
/// each of which also allows its opposite; empty means every multiple of
/// `increment` (15 when that is not positive).
pub fn nearest_allowed(angle: f64, allowed: &[f64], increment: f64) -> f64 {
    let mut best = angle;
    let mut best_d = f64::MAX;
    let mut consider = |c: f64| {
        let d = (angle - c + 180.0).rem_euclid(360.0) - 180.0;
        if d.abs() < best_d {
            best_d = d.abs();
            best = c.rem_euclid(360.0);
        }
    };
    if allowed.is_empty() {
        let inc = if increment > 0.0 { increment } else { 15.0 };
        consider((angle / inc).round() * inc);
    } else {
        for a in allowed {
            consider(*a);
            consider(*a + 180.0);
        }
    }
    best
}

/// The angle a straight wall should have when it is a little off an allowed
/// one; `None` when it is on an allowed angle, far from every one, curved or
/// too short.
pub fn off_angle(w: &Wall, allowed: &[f64], increment: f64) -> Option<f64> {
    if w.is_curved() || w.length() < MIN_LEN {
        return None;
    }
    let a = wall_angle_deg(w);
    let target = nearest_allowed(a, allowed, increment);
    let d = ((a - target + 180.0).rem_euclid(360.0) - 180.0).abs();
    (OFF_ANGLE_MIN_DEG..=OFF_ANGLE_MAX_DEG)
        .contains(&d)
        .then_some(target)
}

/// The ends of `w` after it turns to `new_angle_deg` (length kept) around the
/// part named by `lock`.
pub fn rotated_ends(w: &Wall, new_angle_deg: f64, lock: FixLock) -> (Point, Point) {
    let len = w.length();
    let dir = Point::new(
        new_angle_deg.to_radians().cos(),
        new_angle_deg.to_radians().sin(),
    );
    match lock {
        FixLock::Start => (w.start, w.start + dir * len),
        FixLock::End => (w.end - dir * len, w.end),
        FixLock::Center => {
            let c = Point::lerp(w.start, w.end, 0.5);
            (c - dir * (len * 0.5), c + dir * (len * 0.5))
        }
    }
}

/// Align With Wall Above / Below only looks at walls whose centerlines are
/// this close (inches) and whose directions differ by under 5 degrees.
pub const ALIGN_REACH: f64 = 24.0;

fn main_edges(w: &Wall, ty: Option<&WallTypeDef>) -> (Point, Point) {
    match ty {
        Some(t) => {
            let (a, _, _, d) = main_layer_lines(w, t);
            (a, d)
        }
        None => {
            let n = w.normal() * (w.thickness * 0.5);
            (w.start + n, w.start - n)
        }
    }
}

/// How far (along the shared length) two straight walls overlap, inches.
pub fn overlap_length(a: &Wall, b: &Wall) -> f64 {
    let d = b.direction();
    let (a0, a1) = ((a.start - b.start).dot(d), (a.end - b.start).dot(d));
    let (lo, hi) = (a0.min(a1), a0.max(a1));
    (hi.min(b.length()) - lo.max(0.0)).max(0.0)
}

/// The sideways move that puts the main-layer outer edge of `w` on the
/// nearest main-layer edge of `other` (W-145): `None` when they are not
/// parallel and close, do not overlap, or already line up.
pub fn align_shift(
    w: &Wall,
    wt: Option<&WallTypeDef>,
    other: &Wall,
    ot: Option<&WallTypeDef>,
) -> Option<Point> {
    if w.is_curved() || other.is_curved() || w.length() < MIN_LEN || other.length() < MIN_LEN {
        return None;
    }
    let n = other.normal();
    if w.direction().cross(other.direction()).abs() > 5f64.to_radians().sin() {
        return None;
    }
    if overlap_length(w, other) < MIN_LEN {
        return None;
    }
    let lat = |p: Point| (p - other.start).dot(n);
    let (w1, w2) = main_edges(w, wt);
    let (o1, o2) = main_edges(other, ot);
    let mut best: Option<f64> = None;
    for mine in [lat(w1), lat(w2)] {
        for theirs in [lat(o1), lat(o2)] {
            let d = theirs - mine;
            if best.is_none_or(|b| d.abs() < b.abs()) {
                best = Some(d);
            }
        }
    }
    let d = best?;
    (d.abs() > 1e-6 && d.abs() <= ALIGN_REACH).then(|| n * d)
}

/// Walls that never carry notification icons: drawn-in helpers and railings.
fn is_judged(w: &Wall) -> bool {
    !(w.flags.invisible
        || w.flags.auto_generated
        || w.flags.railing
        || w.flags.room_divider
        || w.is_curved()
        || w.class.is_railing())
}

fn end_point(w: &Wall, e: WallEnd) -> Point {
    match e {
        WallEnd::Start => w.start,
        WallEnd::End => w.end,
    }
}

impl Project {
    /// Walls of `floor` a little off an allowed angle, with the angle each
    /// should have; walls marked "ignore" are left out.
    pub fn off_angle_walls(&self, floor: usize, allowed: &[f64], increment: f64) -> Vec<(Id, f64)> {
        let Some(f) = self.floors.get(floor) else {
            return Vec::new();
        };
        f.walls
            .iter()
            .filter(|w| is_judged(w) && !w.flags.ignore_off_angle)
            .filter_map(|w| off_angle(w, allowed, increment).map(|t| (w.id, t)))
            .collect()
    }

    /// The wall of floor `floor + dir` (`dir` is 1 above, -1 below) that wall
    /// `id` of `floor` overlaps most, with the move that aligns the two
    /// (Align With Wall Above / Below, W-145).
    pub fn align_candidate(
        &self,
        floor: usize,
        id: Id,
        dir: isize,
        types: &[WallTypeDef],
    ) -> Option<(Id, Point)> {
        let other_floor = floor.checked_add_signed(dir)?;
        let w = self.floors.get(floor)?.wall(id)?;
        let ty = |w: &Wall| {
            w.wall_type
                .as_deref()
                .and_then(|n| types.iter().find(|t| t.name == n))
        };
        self.floors
            .get(other_floor)?
            .walls
            .iter()
            .filter(|o| !o.flags.auto_generated && !o.flags.invisible)
            .filter(|o| overlap_length(w, o) >= MIN_LEN)
            .filter_map(|o| {
                align_shift(w, ty(w), o, ty(o)).map(|s| (o.id, s, overlap_length(w, o)))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(i, s, _)| (i, s))
    }

    /// Whether end `e` of wall `id` touches another wall's end or centerline.
    pub fn end_is_connected(&self, floor: usize, id: Id, e: WallEnd) -> bool {
        let Some(f) = self.floors.get(floor) else {
            return true;
        };
        let Some(w) = f.wall(id) else { return true };
        let p = end_point(w, e);
        f.walls.iter().any(|o| {
            o.id != id
                && !o.flags.auto_generated
                && o.path_length() >= MIN_LEN
                && o.closest_point(p).0.dist(p) <= TOUCH_TOL
        })
    }

    /// The loose ends of wall `id`: ends touching nothing, that are not
    /// locked against connecting (Auto Connect) and are not ignored.
    pub fn unconnected_ends(&self, floor: usize, id: Id) -> Vec<WallEnd> {
        let Some(w) = self.floors.get(floor).and_then(|f| f.wall(id)) else {
            return Vec::new();
        };
        if !is_judged(w) || w.flags.ignore_unconnected || w.length() < MIN_LEN {
            return Vec::new();
        }
        [WallEnd::Start, WallEnd::End]
            .into_iter()
            .filter(|e| {
                let locked = match e {
                    WallEnd::Start => w.flags.lock_start,
                    WallEnd::End => w.flags.lock_end,
                };
                !locked && !self.end_is_connected(floor, id, *e)
            })
            .collect()
    }

    /// Every loose wall end of the floor.
    pub fn unconnected_walls(&self, floor: usize) -> Vec<(Id, WallEnd)> {
        let Some(f) = self.floors.get(floor) else {
            return Vec::new();
        };
        f.walls
            .iter()
            .flat_map(|w| {
                self.unconnected_ends(floor, w.id)
                    .into_iter()
                    .map(|e| (w.id, e))
            })
            .collect()
    }

    /// Fix Off Angle Wall (W-133): turns wall `id` to `new_angle_deg` around
    /// `lock`. Returns the old and new ends; the caller moves the walls that
    /// were joined to the old ends.
    pub fn fix_off_angle(
        &mut self,
        floor: usize,
        id: Id,
        new_angle_deg: f64,
        lock: FixLock,
    ) -> Option<[(Point, Point); 2]> {
        let w = self.floors.get_mut(floor)?.wall_mut(id)?;
        if w.is_curved() || w.length() < MIN_LEN {
            return None;
        }
        let old = (w.start, w.end);
        let new = rotated_ends(w, new_angle_deg, lock);
        w.start = new.0;
        w.end = new.1;
        Some([old, new])
    }

    /// Ignore / Ignore All / Ignore Unconnected Wall: stops the icons of the
    /// given walls (all walls of the floor when `ids` is `None`). Returns how
    /// many walls changed.
    pub fn ignore_wall_icons(
        &mut self,
        floor: usize,
        ids: Option<&[Id]>,
        off_angle: bool,
        unconnected: bool,
    ) -> usize {
        let Some(f) = self.floors.get_mut(floor) else {
            return 0;
        };
        let mut n = 0;
        for w in &mut f.walls {
            if ids.is_some_and(|l| !l.contains(&w.id)) {
                continue;
            }
            let before = (w.flags.ignore_off_angle, w.flags.ignore_unconnected);
            w.flags.ignore_off_angle |= off_angle;
            w.flags.ignore_unconnected |= unconnected;
            n += usize::from(before != (w.flags.ignore_off_angle, w.flags.ignore_unconnected));
        }
        n
    }

    /// Reset Notification Icons: every ignored icon of the floor comes back.
    /// Returns how many walls changed.
    pub fn reset_notification_icons(&mut self, floor: usize) -> usize {
        let Some(f) = self.floors.get_mut(floor) else {
            return 0;
        };
        let mut n = 0;
        for w in &mut f.walls {
            if w.flags.ignore_off_angle || w.flags.ignore_unconnected {
                w.flags.ignore_off_angle = false;
                w.flags.ignore_unconnected = false;
                n += 1;
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WallKind;

    fn wall(p: &mut Project, a: (f64, f64), b: (f64, f64)) -> Id {
        p.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            4.5,
            96.0,
            WallKind::Interior,
        )
    }

    fn free(a: (f64, f64), b: (f64, f64)) -> Wall {
        Wall::new(
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            4.5,
            96.0,
            WallKind::Interior,
        )
    }

    const DEG: [f64; 0] = [];

    #[test]
    fn thresholds_of_the_off_angle_icon() {
        // 2 degrees off east is off angle; on east, 20 degrees off and 45 are not.
        let at = |deg: f64| {
            let r = deg.to_radians();
            free((0.0, 0.0), (120.0 * r.cos(), 120.0 * r.sin()))
        };
        assert_eq!(off_angle(&at(2.0), &DEG, 15.0), Some(0.0));
        assert_eq!(off_angle(&at(0.0), &DEG, 15.0), None);
        assert_eq!(off_angle(&at(0.02), &DEG, 15.0), None);
        assert_eq!(off_angle(&at(20.0), &[0.0, 90.0], 15.0), None);
        assert_eq!(off_angle(&at(92.0), &[0.0, 90.0], 15.0), Some(90.0));
        assert_eq!(off_angle(&at(182.0), &[0.0, 90.0], 15.0), Some(180.0));
        assert_eq!(off_angle(&at(359.0), &[0.0, 90.0], 15.0), Some(0.0));
    }

    #[test]
    fn rotating_keeps_the_length_and_the_locked_part() {
        let w = free((0.0, 0.0), (120.0, 4.0));
        let len = w.length();
        for lock in [FixLock::Start, FixLock::Center, FixLock::End] {
            let (s, e) = rotated_ends(&w, 0.0, lock);
            assert!((s.dist(e) - len).abs() < 1e-9);
            assert!((s.y - e.y).abs() < 1e-9);
            match lock {
                FixLock::Start => assert_eq!(s, w.start),
                FixLock::End => assert_eq!(e, w.end),
                FixLock::Center => {
                    assert!(Point::lerp(s, e, 0.5).dist(Point::lerp(w.start, w.end, 0.5)) < 1e-9)
                }
            }
        }
    }

    #[test]
    fn loose_ends_ignores_and_reset() {
        let mut p = Project::new("t");
        wall(&mut p, (0.0, 0.0), (120.0, 0.0));
        wall(&mut p, (120.0, 0.0), (120.0, 96.0));
        // A corner: the shared end is connected, the two far ends are loose.
        assert_eq!(p.unconnected_walls(0).len(), 2);
        // A wall ending on the other wall's centerline is connected there.
        let t = wall(&mut p, (60.0, 0.0), (60.0, 80.0));
        assert!(p.end_is_connected(0, t, WallEnd::Start));
        assert!(!p.end_is_connected(0, t, WallEnd::End));
        let before = p.unconnected_walls(0).len();
        // Ignore one wall, then Ignore All, then reset.
        assert_eq!(p.ignore_wall_icons(0, Some(&[t]), false, true), 1);
        assert_eq!(p.unconnected_walls(0).len(), before - 1);
        assert_eq!(p.ignore_wall_icons(0, None, true, true), 3);
        assert!(p.unconnected_walls(0).is_empty());
        assert_eq!(p.reset_notification_icons(0), 3);
        assert_eq!(p.unconnected_walls(0).len(), before);
        // A locked end (Auto Connect) is not reported.
        p.floors[0].walls[2].flags.lock_end = true;
        assert_eq!(p.unconnected_walls(0).len(), before - 1);
    }

    #[test]
    fn a_wall_aligns_with_the_wall_below_by_its_outer_edge() {
        let mut p = Project::new("t");
        p.floors.push(crate::model::Floor::new("Second", 108.0));
        let low = wall(&mut p, (0.0, 0.0), (240.0, 0.0));
        p.floors[0].wall_mut(low).unwrap().thickness = 6.0;
        let up = p.add_wall(
            1,
            Point::new(0.0, 4.0),
            Point::new(240.0, 4.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        // Edges below: -3 and +3; edges above: 1.75 and 6.25; nearest pair is
        // +3 against 1.75, so the upper wall moves up 1.25.
        let (o, shift) = p.align_candidate(1, up, -1, &[]).unwrap();
        assert_eq!(o, low);
        assert!(
            (shift.y - 1.25).abs() < 1e-9 && shift.x.abs() < 1e-9,
            "{shift:?}"
        );
        // A perpendicular wall, or one far away, does not align.
        let far = p.add_wall(
            1,
            Point::new(0.0, 90.0),
            Point::new(240.0, 90.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        assert!(p.align_candidate(1, far, -1, &[]).is_none());
        let cross = p.add_wall(
            1,
            Point::new(60.0, -50.0),
            Point::new(60.0, 50.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        assert!(p.align_candidate(1, cross, -1, &[]).is_none());
    }

    #[test]
    fn fix_off_angle_turns_the_wall() {
        let mut p = Project::new("t");
        let id = wall(&mut p, (0.0, 0.0), (120.0, 3.0));
        assert_eq!(p.off_angle_walls(0, &DEG, 15.0).len(), 1);
        let [_, new] = p.fix_off_angle(0, id, 0.0, FixLock::Start).unwrap();
        assert_eq!(new.0, Point::new(0.0, 0.0));
        assert!((new.1.y).abs() < 1e-9);
        assert!(p.off_angle_walls(0, &DEG, 15.0).is_empty());
    }
}
