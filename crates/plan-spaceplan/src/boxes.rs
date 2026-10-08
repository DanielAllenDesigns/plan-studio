//! Room boxes: the colored rectangles the Space Planning Assistant arranges.
//!
//! A [`RoomBox`] is an axis-aligned rectangle on a 6" grid with a name, a Chief
//! room type, a floor and a color. This module also holds the interactive
//! behaviour that works on a set of boxes: [`bump`] (snap/slide while dragging)
//! and [`validate`] (sanity checks before "Build House").

use plan_core::{Id, Point};
use serde::{Deserialize, Serialize};

/// Grid the boxes live on, inches.
pub const GRID: f64 = 6.0;
/// Geometric comparison tolerance, inches.
pub(crate) const EPS: f64 = 1e-6;
/// Two boxes only count as touching when they share at least this much edge.
const TOUCH_MIN: f64 = 1.0;
/// Smallest box side, inches.
const MIN_SIDE: f64 = 2.0 * GRID;

/// Axis-aligned rectangle as `(min corner, max corner)`, inches.
pub type Rect = (Point, Point);

/// Round `v` to the nearest [`GRID`] multiple.
pub fn snap_to_grid(v: f64) -> f64 {
    (v / GRID).round() * GRID
}

/// Width of a rectangle.
pub(crate) fn rect_w(r: &Rect) -> f64 {
    r.1.x - r.0.x
}

/// Height of a rectangle.
pub(crate) fn rect_h(r: &Rect) -> f64 {
    r.1.y - r.0.y
}

/// Order the corners so `.0` is the min and `.1` the max.
pub(crate) fn normalized(r: Rect) -> Rect {
    (
        Point::new(r.0.x.min(r.1.x), r.0.y.min(r.1.y)),
        Point::new(r.0.x.max(r.1.x), r.0.y.max(r.1.y)),
    )
}

/// True when the interiors of `a` and `b` intersect (touching edges do not).
pub(crate) fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.0.x < b.1.x - EPS && b.0.x < a.1.x - EPS && a.0.y < b.1.y - EPS && b.0.y < a.1.y - EPS
}

fn overlap_len(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

/// Length of the edge two rectangles share (0 when they do not abut).
pub(crate) fn shared_edge_len(a: &Rect, b: &Rect) -> f64 {
    let mut best: f64 = 0.0;
    if (a.1.x - b.0.x).abs() < EPS || (b.1.x - a.0.x).abs() < EPS {
        best = best.max(overlap_len(a.0.y, a.1.y, b.0.y, b.1.y));
    }
    if (a.1.y - b.0.y).abs() < EPS || (b.1.y - a.0.y).abs() < EPS {
        best = best.max(overlap_len(a.0.x, a.1.x, b.0.x, b.1.x));
    }
    best
}

/// Chief-like space-planning colors by room type.
pub fn room_color(room_type: &str) -> [u8; 3] {
    match room_type {
        "Master Bedroom" => [96, 150, 224],
        "Bedroom" => [136, 184, 238],
        "Bath" | "Master Bath" | "Powder Room" => [92, 196, 192],
        "Living" => [255, 224, 112],
        "Dining" => [250, 204, 120],
        "Kitchen" => [247, 168, 90],
        "Pantry" => [244, 190, 130],
        "Garage" => [168, 168, 168],
        "Entry" => [232, 214, 170],
        "Hall" => [214, 212, 200],
        "Laundry" => [190, 172, 226],
        "Mud Room" => [200, 184, 160],
        "Office" => [150, 160, 214],
        "Porch" | "Deck" => [146, 206, 136],
        _ => [200, 200, 200],
    }
}

/// One room box on the space-planning canvas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomBox {
    pub id: Id,
    /// Display name such as "Master Bedroom" or "Bedroom 2".
    pub name: String,
    /// Chief room type ("Bedroom", "Bath", "Living", "Garage", ...).
    pub room_type: String,
    /// `(min, max)` corners, inches, kept on the 6" grid.
    pub rect: Rect,
    /// Floor index (0 = first floor).
    pub floor: usize,
    pub color: [u8; 3],
    /// A locked box ignores drags: [`bump`] returns its rectangle unchanged.
    pub locked: bool,
}

impl RoomBox {
    /// New box with its min corner at `min` and a size of `w` x `h` inches.
    /// The color comes from [`room_color`].
    pub fn new(
        id: Id,
        name: impl Into<String>,
        room_type: impl Into<String>,
        min: Point,
        (w, h): (f64, f64),
        floor: usize,
    ) -> Self {
        let room_type = room_type.into();
        let color = room_color(&room_type);
        Self {
            id,
            name: name.into(),
            room_type,
            rect: (min, Point::new(min.x + w, min.y + h)),
            floor,
            color,
            locked: false,
        }
    }

    pub fn width(&self) -> f64 {
        rect_w(&self.rect)
    }

    pub fn height(&self) -> f64 {
        rect_h(&self.rect)
    }

    pub fn center(&self) -> Point {
        Point::lerp(self.rect.0, self.rect.1, 0.5)
    }

    pub fn area_sq_ft(&self) -> f64 {
        self.width() * self.height() / 144.0
    }

    /// Resize to about `sq_ft`, keeping the width:height ratio and the min
    /// corner. Sides are rounded to the grid (never below 12"), so the result
    /// is within a few percent of the request rather than exact.
    pub fn set_area_keep_aspect(&mut self, sq_ft: f64) {
        let now = self.width() * self.height();
        if now <= EPS || sq_ft <= 0.0 {
            return;
        }
        let k = (sq_ft * 144.0 / now).sqrt();
        let w = snap_to_grid(self.width() * k).max(MIN_SIDE);
        let h = snap_to_grid(self.height() * k).max(MIN_SIDE);
        self.rect.1 = Point::new(self.rect.0.x + w, self.rect.0.y + h);
    }

    /// Move the box so its min corner is at `min` (snapped to the grid).
    pub fn move_to(&mut self, min: Point) {
        let (w, h) = (self.width(), self.height());
        let min = Point::new(snap_to_grid(min.x), snap_to_grid(min.y));
        self.rect = (min, Point::new(min.x + w, min.y + h));
    }
}

/// How far (in grid steps) [`bump`] searches for free space as a last resort.
const MAX_SEARCH_RINGS: i32 = 400;

/// Chief-style bumping for a box being dragged.
///
/// `proposed` is where the user dragged box `moving` to. The result is the
/// rectangle to actually use:
/// 1. the position is rounded to the grid;
/// 2. if it overlaps a neighbor on the same floor it is pushed out along the
///    axis of smaller penetration;
/// 3. each axis then snaps to a neighbor edge within `snap` inches: abutting
///    (edge against the facing edge) or flush (same-side edges aligned). Doing
///    x then y makes corner-to-corner alignment fall out naturally;
/// 4. if snapping would cause an overlap it is dropped, and as a last resort
///    the nearest free grid position is used, so the result never overlaps.
///
/// A locked (or unknown) box is returned as given.
pub fn bump(boxes: &[RoomBox], moving: Id, proposed: Rect, snap: f64) -> Rect {
    let me = boxes.iter().find(|b| b.id == moving);
    if let Some(m) = me {
        if m.locked {
            return m.rect;
        }
    }
    let floor = me.map_or(0, |m| m.floor);
    let others: Vec<Rect> = boxes
        .iter()
        .filter(|b| b.id != moving && b.floor == floor)
        .map(|b| b.rect)
        .collect();

    let r = normalized(proposed);
    let size = (rect_w(&r), rect_h(&r));
    let rect_at = |min: Point| -> Rect { (min, Point::new(min.x + size.0, min.y + size.1)) };
    let hits = |min: Point| -> bool {
        let r = rect_at(min);
        others.iter().any(|o| overlaps(&r, o))
    };

    let mut min = push_out(
        Point::new(snap_to_grid(r.0.x), snap_to_grid(r.0.y)),
        size,
        &others,
    );

    // Snap x then y; fall back to a single axis, then to no snap, if the
    // snapped position would overlap something.
    let snapped_x = Point::new(min.x + snap_delta(min, size, &others, snap, true), min.y);
    let snapped_xy = Point::new(
        snapped_x.x,
        snapped_x.y + snap_delta(snapped_x, size, &others, snap, false),
    );
    let snapped_y = Point::new(min.x, min.y + snap_delta(min, size, &others, snap, false));
    for cand in [snapped_xy, snapped_x, snapped_y] {
        if !hits(cand) {
            min = cand;
            break;
        }
    }

    if hits(min) {
        min = nearest_free(min, &hits);
    }
    rect_at(min)
}

/// Push a `size` rectangle at `min` out of every overlapping rectangle along
/// the axis with the smaller penetration.
fn push_out(mut min: Point, size: (f64, f64), others: &[Rect]) -> Point {
    for _ in 0..32 {
        let r = (min, Point::new(min.x + size.0, min.y + size.1));
        let Some(o) = others.iter().find(|o| overlaps(&r, o)) else {
            break;
        };
        let pen_left = r.1.x - o.0.x; // moving left by this clears it
        let pen_right = o.1.x - r.0.x;
        let pen_down = r.1.y - o.0.y;
        let pen_up = o.1.y - r.0.y;
        let (px, dx) = if pen_left <= pen_right {
            (pen_left, -pen_left)
        } else {
            (pen_right, pen_right)
        };
        let (py, dy) = if pen_down <= pen_up {
            (pen_down, -pen_down)
        } else {
            (pen_up, pen_up)
        };
        if px <= py {
            min.x += dx;
        } else {
            min.y += dy;
        }
    }
    min
}

/// Picks one coordinate of a rectangle (an edge position).
type EdgeFn = fn(&Rect) -> f64;

/// Smallest move (|delta| <= `snap`) along one axis that aligns an edge of the
/// moving rectangle with an edge of a neighbor that is near it on the other
/// axis. Returns 0 when nothing is in range.
fn snap_delta(min: Point, size: (f64, f64), others: &[Rect], snap: f64, x_axis: bool) -> f64 {
    let (lo, hi, o_lo, o_hi): (f64, f64, EdgeFn, EdgeFn) = if x_axis {
        (min.x, min.x + size.0, |r| r.0.x, |r| r.1.x)
    } else {
        (min.y, min.y + size.1, |r| r.0.y, |r| r.1.y)
    };
    // Extent on the *other* axis decides whether a neighbor is "near".
    let (c_lo, c_hi): (f64, f64) = if x_axis {
        (min.y, min.y + size.1)
    } else {
        (min.x, min.x + size.0)
    };
    let mut best: Option<f64> = None;
    for o in others {
        let (oc_lo, oc_hi) = if x_axis {
            (o.0.y, o.1.y)
        } else {
            (o.0.x, o.1.x)
        };
        if oc_lo - c_hi > snap + EPS || c_lo - oc_hi > snap + EPS {
            continue;
        }
        // Abut first so it wins ties against flush alignment.
        for d in [
            o_hi(o) - lo, // my low edge against its high edge
            o_lo(o) - hi, // my high edge against its low edge
            o_lo(o) - lo, // flush low edges
            o_hi(o) - hi, // flush high edges
        ] {
            if d.abs() <= snap + EPS && best.is_none_or(|b| d.abs() < b.abs() - EPS) {
                best = Some(d);
            }
        }
    }
    best.unwrap_or(0.0)
}

/// Nearest grid position (searching outward in rings) where `hits` is false.
fn nearest_free(from: Point, hits: &dyn Fn(Point) -> bool) -> Point {
    for ring in 1..=MAX_SEARCH_RINGS {
        let mut best: Option<(f64, Point)> = None;
        for dx in -ring..=ring {
            for dy in -ring..=ring {
                if dx.abs().max(dy.abs()) != ring {
                    continue;
                }
                let p = Point::new(from.x + f64::from(dx) * GRID, from.y + f64::from(dy) * GRID);
                let d = p.dist(from);
                if best.is_none_or(|(bd, _)| d < bd) && !hits(p) {
                    best = Some((d, p));
                }
            }
        }
        if let Some((_, p)) = best {
            return p;
        }
    }
    from
}

/// A problem found by [`validate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub box_id: Id,
    pub message: String,
}

fn is_bedroom(room_type: &str) -> bool {
    matches!(room_type, "Bedroom" | "Master Bedroom")
}

/// Check a box layout before building the house.
///
/// Reports, per box: overlaps with another box on the same floor; boxes that
/// touch no other box on their floor; baths ("Bath", "Master Bath") with no
/// adjacent hall or bedroom; and garages that touch no non-garage room.
pub fn validate(boxes: &[RoomBox]) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut add = |b: &RoomBox, message: String| {
        issues.push(Issue {
            box_id: b.id,
            message,
        });
    };
    for (i, a) in boxes.iter().enumerate() {
        let same_floor: Vec<&RoomBox> = boxes
            .iter()
            .enumerate()
            .filter(|(j, b)| *j != i && b.floor == a.floor)
            .map(|(_, b)| b)
            .collect();
        for b in &same_floor {
            if overlaps(&a.rect, &b.rect) {
                add(a, format!("{} overlaps {}", a.name, b.name));
            }
        }
        let touching: Vec<&&RoomBox> = same_floor
            .iter()
            .filter(|b| shared_edge_len(&a.rect, &b.rect) >= TOUCH_MIN)
            .collect();
        if !same_floor.is_empty() && touching.is_empty() {
            add(a, format!("{} does not touch any other room", a.name));
        }
        if matches!(a.room_type.as_str(), "Bath" | "Master Bath")
            && !touching
                .iter()
                .any(|b| b.room_type == "Hall" || is_bedroom(&b.room_type))
        {
            add(a, format!("{} has no adjacent hall or bedroom", a.name));
        }
        if a.room_type == "Garage" && !touching.iter().any(|b| b.room_type != "Garage") {
            add(a, format!("{} does not touch the house", a.name));
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(id: Id, ty: &str, x: f64, y: f64, w: f64, h: f64) -> RoomBox {
        RoomBox::new(id, format!("R{id}"), ty, Point::new(x, y), (w, h), 0)
    }

    fn no_overlap(r: &Rect, boxes: &[RoomBox], skip: Id) -> bool {
        boxes
            .iter()
            .filter(|b| b.id != skip)
            .all(|b| !overlaps(r, &b.rect))
    }

    #[test]
    fn area_and_resize_keep_aspect() {
        let mut b = bx(1, "Bedroom", 0.0, 0.0, 156.0, 132.0);
        assert!((b.area_sq_ft() - 143.0).abs() < 1e-9);
        b.set_area_keep_aspect(286.0);
        assert!((b.area_sq_ft() - 286.0).abs() / 286.0 < 0.06);
        assert!((b.width() / b.height() - 156.0 / 132.0).abs() < 0.15);
        b.move_to(Point::new(100.0, 13.0));
        assert_eq!(b.rect.0, Point::new(102.0, 12.0));
    }

    #[test]
    fn bump_aligns_edge_within_snap() {
        let boxes = vec![
            bx(1, "Living", 0.0, 0.0, 144.0, 144.0),
            bx(2, "Kitchen", 400.0, 400.0, 120.0, 120.0),
        ];
        // 6" short of abutting on the right of box 1, y flush-ish with it.
        let proposed = (Point::new(150.0, 6.0), Point::new(270.0, 126.0));
        let r = bump(&boxes, 2, proposed, 6.0);
        assert!((r.0.x - 144.0).abs() < 1e-9, "x abuts: {r:?}");
        assert!((r.0.y - 0.0).abs() < 1e-9, "y flush: {r:?}");
        assert!(no_overlap(&r, &boxes, 2));
    }

    #[test]
    fn bump_never_overlaps_even_when_dropped_inside() {
        let boxes = vec![
            bx(1, "Living", 0.0, 0.0, 240.0, 240.0),
            bx(2, "Kitchen", 600.0, 600.0, 120.0, 120.0),
            bx(3, "Dining", 240.0, 0.0, 120.0, 240.0),
        ];
        for (x, y) in [(60.0, 60.0), (200.0, 100.0), (250.0, 30.0), (-30.0, 200.0)] {
            let p = (Point::new(x, y), Point::new(x + 120.0, y + 120.0));
            let r = bump(&boxes, 2, p, 6.0);
            assert!(no_overlap(&r, &boxes, 2), "overlap at {x},{y}: {r:?}");
            assert!((rect_w(&r) - 120.0).abs() < 1e-9);
        }
    }

    #[test]
    fn bump_locked_box_does_not_move() {
        let mut boxes = vec![bx(1, "Living", 0.0, 0.0, 144.0, 144.0)];
        boxes[0].locked = true;
        let r = bump(
            &boxes,
            1,
            (Point::new(500.0, 500.0), Point::new(644.0, 644.0)),
            6.0,
        );
        assert_eq!(r, boxes[0].rect);
    }

    #[test]
    fn validate_flags_problems() {
        let boxes = vec![
            bx(1, "Bedroom", 0.0, 0.0, 144.0, 144.0),
            bx(2, "Bath", 100.0, 0.0, 60.0, 96.0), // overlaps 1
            bx(3, "Garage", 1000.0, 0.0, 264.0, 264.0), // floating
        ];
        let msgs: Vec<String> = validate(&boxes).into_iter().map(|i| i.message).collect();
        assert!(msgs.iter().any(|m| m.contains("overlaps")));
        assert!(msgs.iter().any(|m| m.contains("does not touch any other")));
        assert!(msgs.iter().any(|m| m.contains("does not touch the house")));
        let ok = vec![
            bx(1, "Bedroom", 0.0, 0.0, 144.0, 144.0),
            bx(2, "Bath", 144.0, 0.0, 60.0, 96.0),
        ];
        assert!(validate(&ok).is_empty());
        let lone_bath = vec![
            bx(1, "Living", 0.0, 0.0, 144.0, 144.0),
            bx(2, "Bath", 144.0, 0.0, 60.0, 96.0),
        ];
        assert!(validate(&lone_bath)
            .iter()
            .any(|i| i.message.contains("no adjacent hall or bedroom")));
    }

    #[test]
    fn serde_round_trip() {
        let b = bx(7, "Kitchen", 12.0, 24.0, 144.0, 192.0);
        let json = serde_json::to_string(&b).unwrap();
        let back: RoomBox = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);
    }
}
