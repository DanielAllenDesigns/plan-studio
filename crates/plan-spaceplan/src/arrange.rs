//! Automatic arrangement of freshly generated room boxes.
//!
//! # Heuristic: greedy affinity packing
//!
//! Each floor is packed independently, one box at a time, in a fixed priority
//! order that mirrors how a house is organised around its entry:
//!
//! 1. the **entry** (seed, placed at the origin) and the **hall** that carries
//!    the private rooms;
//! 2. the **living room**, then the **master suite** and **bedrooms/baths**
//!    which are pulled towards the hall;
//! 3. the remaining **public and service rooms**: dining, kitchen, pantry,
//!    mud room, garage, laundry, powder room, office, porch and deck.
//!
//! A box is only ever placed *abutting* an already placed box (east, west,
//! north or south of it, aligned to either end or centered), in its natural
//! orientation or turned 90 degrees. That keeps the layout connected, on the
//! grid and free of overlaps by construction. Among all such candidate
//! positions the cheapest wins, where cost is
//!
//! * minus the affinity of the new room for each room it would touch (kitchen
//!   next to dining and pantry, master bath next to master bedroom, bedrooms
//!   and baths along the hall, mud room between garage and house, ...; bedrooms
//!   next to the garage or kitchen are penalised),
//! * minus a small bonus per foot of shared wall (compactness),
//! * plus a penalty for dead space inside the bounding box (keeps the blob
//!   rectilinear and filled in) and for a bounding box more elongated than
//!   about 1.7:1,
//! * plus a small penalty for turning the box.
//!
//! Ties are broken by candidate order, so the output is deterministic.

use crate::boxes::{overlaps, rect_h, rect_w, shared_edge_len, snap_to_grid, Rect, RoomBox};
use plan_core::Point;

/// Contact below this length (inches) cannot hold a door, so earns no affinity.
const MIN_AFFINITY_CONTACT: f64 = 36.0;
const CONTACT_WEIGHT: f64 = 0.15;
const DEAD_SPACE_WEIGHT: f64 = 0.08;
const ELONGATION_WEIGHT: f64 = 8.0;
const MAX_ASPECT: f64 = 1.7;
const TURN_PENALTY: f64 = 1.5;

fn is_bedroom(t: &str) -> bool {
    matches!(t, "Bedroom" | "Master Bedroom")
}

/// Placement order of a room type (lower is placed earlier).
fn priority(room_type: &str) -> usize {
    const ORDER: [&str; 17] = [
        "Entry",
        "Hall",
        "Living",
        "Master Bedroom",
        "Master Bath",
        "Bedroom",
        "Bath",
        "Dining",
        "Kitchen",
        "Pantry",
        "Mud Room",
        "Garage",
        "Laundry",
        "Powder Room",
        "Office",
        "Porch",
        "Deck",
    ];
    ORDER
        .iter()
        .position(|t| *t == room_type)
        .unwrap_or(ORDER.len())
}

/// One direction of the affinity table (`a` newly placed, `b` already there).
fn affinity_one(a: &str, b: &str) -> Option<f64> {
    Some(match (a, b) {
        ("Entry", "Living") => 6.0,
        ("Entry", "Hall") => 7.0,
        ("Entry", "Porch") => 9.0,
        ("Entry", "Office") => 4.0,
        ("Entry", "Garage") => -2.0,
        ("Entry", t) if is_bedroom(t) => -3.0,
        ("Living", "Dining") => 5.0,
        ("Living", "Kitchen") => 3.0,
        ("Living", "Deck") => 4.0,
        ("Living", "Porch") => 3.0,
        ("Living", "Hall") => 3.0,
        ("Living", "Garage") => -3.0,
        ("Living", t) if is_bedroom(t) => -3.0,
        ("Dining", "Kitchen") => 7.0,
        ("Dining", "Deck") => 4.0,
        ("Dining", "Garage") => -3.0,
        ("Kitchen", "Pantry") => 9.0,
        ("Kitchen", "Mud Room") => 2.0,
        ("Kitchen", "Garage") => 3.0,
        ("Kitchen", "Deck") => 2.0,
        ("Kitchen", t) if is_bedroom(t) => -4.0,
        ("Mud Room", "Garage") => 10.0,
        ("Mud Room", "Laundry") => 4.0,
        ("Laundry", "Garage") => 3.0,
        ("Laundry", "Kitchen") => 2.0,
        ("Laundry", "Hall") => 3.0,
        ("Garage", t) if is_bedroom(t) => -6.0,
        ("Hall", t) if is_bedroom(t) => 6.0,
        ("Hall", "Bath") => 6.0,
        ("Hall", "Powder Room") => 5.0,
        ("Master Bedroom", "Master Bath") => 14.0,
        ("Master Bath", "Bedroom") => -6.0,
        ("Bath", "Bedroom") => 4.0,
        ("Bath", "Master Bedroom") => -2.0,
        ("Bedroom", "Bedroom") => 1.0,
        ("Office", "Living") => 2.0,
        _ => return None,
    })
}

/// Symmetric affinity between two room types.
fn affinity(a: &str, b: &str) -> f64 {
    affinity_one(a, b)
        .or_else(|| affinity_one(b, a))
        .unwrap_or(0.0)
}

fn union_bounds(rects: &[Rect], extra: &Rect) -> Rect {
    let mut lo = extra.0;
    let mut hi = extra.1;
    for r in rects {
        lo = Point::new(lo.x.min(r.0.x), lo.y.min(r.0.y));
        hi = Point::new(hi.x.max(r.1.x), hi.y.max(r.1.y));
    }
    (lo, hi)
}

/// Cost of placing a box of `room_type` at `rect` next to `placed`.
fn cost(placed: &[&RoomBox], room_type: &str, rect: &Rect, turned: bool) -> f64 {
    let mut c = if turned { TURN_PENALTY } else { 0.0 };
    let mut placed_area = rect_w(rect) * rect_h(rect);
    for p in placed {
        let shared = shared_edge_len(rect, &p.rect);
        c -= CONTACT_WEIGHT * shared / 12.0;
        let aff = affinity(room_type, &p.room_type);
        // Wanted neighbors need a door-sized contact; unwanted ones count on any contact.
        let counts = if aff > 0.0 {
            shared >= MIN_AFFINITY_CONTACT
        } else {
            shared > 0.0
        };
        if counts {
            c -= aff * (1.0 + shared.min(120.0) / 120.0);
        }
        placed_area += p.width() * p.height();
    }
    let rects: Vec<Rect> = placed.iter().map(|p| p.rect).collect();
    let bounds = union_bounds(&rects, rect);
    let (bw, bh) = (rect_w(&bounds), rect_h(&bounds));
    c += DEAD_SPACE_WEIGHT * (bw * bh - placed_area).max(0.0) / 144.0;
    let aspect = bw.max(bh) / bw.min(bh).max(1.0);
    c += ELONGATION_WEIGHT * (aspect - MAX_ASPECT).max(0.0);
    c
}

/// Candidate min corners abutting `p` for a box of size `w` x `h`.
fn candidates(p: &Rect, w: f64, h: f64) -> Vec<Point> {
    let mid_y = snap_to_grid((p.0.y + p.1.y - h) / 2.0);
    let mid_x = snap_to_grid((p.0.x + p.1.x - w) / 2.0);
    let mut out = Vec::with_capacity(12);
    for y in [p.0.y, p.1.y - h, mid_y] {
        out.push(Point::new(p.1.x, y)); // east
        out.push(Point::new(p.0.x - w, y)); // west
    }
    for x in [p.0.x, p.1.x - w, mid_x] {
        out.push(Point::new(x, p.1.y)); // north
        out.push(Point::new(x, p.0.y - h)); // south
    }
    out
}

/// Best rectangle for a new box of `size` among those abutting `placed`.
fn best_position(placed: &[&RoomBox], room_type: &str, size: (f64, f64)) -> Rect {
    let mut best: Option<(f64, Rect)> = None;
    let orientations: &[(bool, (f64, f64))] = &[(false, size), (true, (size.1, size.0))];
    for &(turned, (w, h)) in orientations {
        if turned && (size.0 - size.1).abs() < 1e-9 {
            continue;
        }
        for p in placed {
            for min in candidates(&p.rect, w, h) {
                let rect: Rect = (min, Point::new(min.x + w, min.y + h));
                if placed.iter().any(|q| overlaps(&rect, &q.rect)) {
                    continue;
                }
                let c = cost(placed, room_type, &rect, turned);
                if best.is_none_or(|(bc, _)| c < bc - 1e-9) {
                    best = Some((c, rect));
                }
            }
        }
    }
    best.map(|(_, r)| r)
        .unwrap_or((Point::ZERO, Point::new(size.0, size.1)))
}

/// Arrange every floor of `boxes` in place (see the module docs).
pub(crate) fn arrange(boxes: &mut [RoomBox]) {
    let floors = boxes.iter().map(|b| b.floor).max().map_or(0, |m| m + 1);
    for floor in 0..floors {
        let mut order: Vec<usize> = (0..boxes.len())
            .filter(|&i| boxes[i].floor == floor)
            .collect();
        order.sort_by_key(|&i| priority(&boxes[i].room_type)); // stable
        let mut placed: Vec<usize> = Vec::with_capacity(order.len());
        for &i in &order {
            let size = (boxes[i].width(), boxes[i].height());
            let rect = if placed.is_empty() {
                (Point::ZERO, Point::new(size.0, size.1))
            } else {
                let refs: Vec<&RoomBox> = placed.iter().map(|&j| &boxes[j]).collect();
                best_position(&refs, &boxes[i].room_type, size)
            };
            boxes[i].rect = rect;
            placed.push(i);
        }
        // Shift the floor so its bounding box starts at the origin.
        let rects: Vec<Rect> = placed.iter().map(|&j| boxes[j].rect).collect();
        if let Some(first) = rects.first() {
            let bounds = union_bounds(&rects, first);
            for &j in &placed {
                let r = boxes[j].rect;
                boxes[j].rect = (
                    Point::new(r.0.x - bounds.0.x, r.0.y - bounds.0.y),
                    Point::new(r.1.x - bounds.0.x, r.1.y - bounds.0.y),
                );
            }
        }
    }
}
