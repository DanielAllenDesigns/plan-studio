//! The "box block": the placement record shared by cabinets (class 15),
//! library symbols (class 123, and the furniture groups of class 16), and
//! hosted openings (classes 9 and 10).
//!
//! Objects that sit in the plan as a rectangular box (a cabinet, a toilet, a
//! sofa, a soffit) carry one run of eight `f64`s right after a fixed 32-byte
//! anchor:
//!
//! ```text
//! anchor  00 00 00 00 00 00 30 40   (16.0)
//!         00 00 00 00 00 00 18 40   ( 6.0)
//!         00 x 16
//! +0x00   x        plan inches, the BACK edge, centre of the width
//! +0x08   y
//! +0x10   cos      \ unit vector: the direction the FRONT faces
//! +0x18   sin      /  (cos 0 = front toward +x, sin 1 = toward +y)
//! +0x20   depth    front to back
//! +0x28   width    along the back edge
//! +0x30   height
//! +0x38   top      elevation of the top above the floor (top - height = bottom)
//! ```
//!
//! Evidence (job C, 50 cabinets of the first floor): the point lies 3.25" to
//! 3.5" from the reference line of the wall behind it in 43 of 50 cases (half a
//! 6.5" or 7" wall), `(cos, sin)` points away from that wall in the same 43,
//! and the six others are islands and soffits. Consecutive wall cabinets are
//! `(width1 + width2) / 2` apart along the wall (38.9" for 41.8" and 36"), so
//! `(x, y)` is the middle of the width. The eight values are in the plan's own
//! absolute coordinates for objects that sit directly in a floor (or a furniture
//! group); inside a cabinet (a sink, a dishwasher) `(x, y)` is `(0, 0)`: those
//! are in the cabinet's local frame and not placed.
//!
//! The offset of the anchor inside the object varies with the strings in front
//! of it (826, 1992, 2872 ... in cabinets), so it is found by the anchor bytes.

use super::tree::{f64_at, ObjectTree};

/// The 32 anchor bytes in front of the eight values.
const ANCHOR: [u8; 32] = {
    let mut a = [0u8; 32];
    a[6] = 0x30;
    a[7] = 0x40;
    a[14] = 0x18;
    a[15] = 0x40;
    a
};

/// One placement record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxBlock {
    /// Offset of `x` from the object's `CD` byte.
    pub offset: usize,
    pub x: f64,
    pub y: f64,
    /// Unit vector of the direction the front faces.
    pub cos: f64,
    pub sin: f64,
    pub depth: f64,
    pub width: f64,
    pub height: f64,
    /// Elevation of the top.
    pub top: f64,
}

impl BoxBlock {
    /// Elevation of the bottom above the floor (never negative).
    pub fn elevation(&self) -> f64 {
        (self.top - self.height).max(0.0)
    }

    /// Plan Studio's counter-clockwise angle of the box in radians: local `+Y`
    /// (the front) points along `(cos, sin)`, so the angle is `atan2(-cos, sin)`.
    pub fn angle(&self) -> f64 {
        let a = (-self.cos).atan2(self.sin);
        // Tidy -0.0 and tiny noise (the unit vectors carry 1e-17 residue).
        if a.abs() < 1e-9 {
            0.0
        } else {
            a
        }
    }

    /// The back-left corner (Plan Studio's cabinet origin): the back centre
    /// moved half a width against the width axis `(sin, -cos)`.
    pub fn back_left(&self) -> (f64, f64) {
        let h = self.width / 2.0;
        (self.x - self.sin * h, self.y + self.cos * h)
    }

    /// The centre of the footprint.
    pub fn center(&self) -> (f64, f64) {
        let d = self.depth / 2.0;
        (self.x + self.cos * d, self.y + self.sin * d)
    }

    /// The four footprint corners, counter-clockwise from the back-left.
    pub fn corners(&self) -> [(f64, f64); 4] {
        let (bx, by) = self.back_left();
        // Width axis (sin, -cos), depth axis (cos, sin).
        let (wx, wy) = (self.sin * self.width, -self.cos * self.width);
        let (dx, dy) = (self.cos * self.depth, self.sin * self.depth);
        [
            (bx, by),
            (bx + wx, by + wy),
            (bx + wx + dx, by + wy + dy),
            (bx + dx, by + dy),
        ]
    }
}

fn plausible(b: &BoxBlock) -> bool {
    let unit = (b.cos * b.cos + b.sin * b.sin - 1.0).abs() < 1e-6;
    unit && b.x.abs() < 1.0e6
        && b.y.abs() < 1.0e6
        && b.depth > 0.0
        && b.depth < 1000.0
        && b.width > 0.0
        && b.width < 5000.0
        && b.height > 0.0
        && b.height < 1000.0
        && b.top.abs() < 2000.0
}

/// Every plausible block inside `[from, to)` (absolute byte offsets), in file
/// order. `base` is the object's `CD` offset (for [`BoxBlock::offset`]).
fn blocks_in(bytes: &[u8], base: usize, from: usize, to: usize) -> Vec<BoxBlock> {
    let mut out = Vec::new();
    let to = to.min(bytes.len());
    if to < from + 32 + 64 {
        return out;
    }
    let mut i = from;
    // The anchor's 0x30 byte is the 7th byte.
    while i + 32 + 64 <= to {
        if bytes[i + 6] == 0x30 && bytes[i + 7] == 0x40 && bytes[i..i + 32] == ANCHOR {
            let p = i + 32;
            let v = |k: usize| f64_at(bytes, p + 8 * k).filter(|v| v.is_finite());
            if let (Some(x), Some(y), Some(c), Some(s), Some(d), Some(w), Some(h), Some(t)) =
                (v(0), v(1), v(2), v(3), v(4), v(5), v(6), v(7))
            {
                let b = BoxBlock {
                    offset: p - base,
                    x,
                    y,
                    cos: c,
                    sin: s,
                    depth: d,
                    width: w,
                    height: h,
                    top: t,
                };
                if plausible(&b) {
                    out.push(b);
                }
            }
            i += 32;
        } else {
            i += 1;
        }
    }
    out
}

/// The object's own block: the first plausible one outside the spans of its
/// child objects (children carry blocks of their own).
pub fn own_block(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<BoxBlock> {
    let n = tree.node(node);
    let mut pos = n.marker;
    let mut gaps: Vec<(usize, usize)> = Vec::new();
    for &c in tree.children(node) {
        let k = tree.node(c);
        if k.marker > pos {
            gaps.push((pos, k.marker));
        }
        pos = pos.max(k.end);
    }
    if pos < n.end {
        gaps.push((pos, n.end));
    }
    gaps.into_iter()
        .flat_map(|(a, b)| blocks_in(bytes, n.marker, a, b))
        .next()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::tree::testutil::*;

    /// Writes an anchor and a block at `at` (relative to the `CD` byte).
    pub fn put_block(b: &mut [u8], at: usize, v: [f64; 8]) {
        b[at + 6] = 0x30;
        b[at + 7] = 0x40;
        b[at + 14] = 0x18;
        b[at + 15] = 0x40;
        for (k, x) in v.iter().enumerate() {
            put_f64(b, at + 32 + 8 * k, *x);
        }
    }

    #[test]
    fn finds_the_block_and_converts_the_frame() {
        let obj = sized(15, 0, 0x500, |b| {
            // Front toward -x: the cabinet's back is on the wall at x = 912.58.
            put_block(
                b,
                0x300,
                [912.583, 621.114, -1.0, 0.0, 24.0, 48.0, 96.0, 96.0],
            );
        });
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(15, 0).next().unwrap();
        let blk = own_block(&obj, &tree, i).unwrap();
        assert_eq!(blk.offset, 0x300 + 32);
        assert_eq!(
            (blk.depth, blk.width, blk.height, blk.top),
            (24.0, 48.0, 96.0, 96.0)
        );
        assert_eq!(blk.elevation(), 0.0);
        // Front toward -x is a quarter turn: a = atan2(1, 0) = 90 degrees.
        assert!((blk.angle() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        // The width runs along y; the back-left corner is the lower end.
        let (bx, by) = blk.back_left();
        assert!((bx - 912.583).abs() < 1e-9 && (by - (621.114 - 24.0)).abs() < 1e-9);
        let c = blk.center();
        assert!((c.0 - (912.583 - 12.0)).abs() < 1e-9 && (c.1 - 621.114).abs() < 1e-9);
        let corners = blk.corners();
        assert!((corners[2].0 - (912.583 - 24.0)).abs() < 1e-9);
    }

    #[test]
    fn front_plus_y_is_angle_zero() {
        let obj = sized(15, 0, 0x400, |b| {
            put_block(b, 0x100, [100.0, 50.0, 0.0, 1.0, 12.0, 30.0, 30.0, 84.0]);
        });
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(15, 0).next().unwrap();
        let blk = own_block(&obj, &tree, i).unwrap();
        assert_eq!(blk.angle(), 0.0);
        assert_eq!(blk.elevation(), 54.0);
        assert_eq!(blk.back_left(), (100.0 - 15.0, 50.0));
    }

    #[test]
    fn rejects_non_unit_direction_and_blocks_inside_children() {
        let obj = sized(15, 0, 0x400, |b| {
            put_block(b, 0x100, [100.0, 50.0, 0.5, 0.5, 12.0, 30.0, 30.0, 84.0]);
        });
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(15, 0).next().unwrap();
        assert!(own_block(&obj, &tree, i).is_none());
        // A block inside a child span does not count as the parent's own.
        let child = sized(123, 0, 0x200, |b| {
            put_block(b, 0x40, [0.0, 0.0, 0.0, -1.0, 20.0, 20.0, 12.0, 12.0]);
        });
        let parent = sized(15, 0, 0x400 + child.len(), |b| {
            put_block(b, 0x100, [100.0, 50.0, 0.0, 1.0, 12.0, 30.0, 30.0, 84.0]);
            let at = 0x300;
            b[at..at + child.len() - 1].copy_from_slice(&child[1..]);
        });
        let tree = ObjectTree::build(&parent);
        let i = tree.of_kind(15, 0).next().unwrap();
        let blk = own_block(&parent, &tree, i).unwrap();
        assert_eq!((blk.x, blk.y), (100.0, 50.0));
        assert_eq!(tree.of_kind(123, 0).count(), 1);
    }
}
