//! Doors (class 9) and windows (class 10).
//!
//! An opening is a child object of the wall that hosts it. Inside it sits a
//! run of five `f64`s whose position moves between files and between object
//! variants (+0x312 and +0x3F5 in two X18 variants of one project, +0x291 in
//! an X17 variant), so the run is found by shape:
//!
//! ```text
//! A+0x00  f64  distance from the wall start to the opening CENTRE, inches
//! A+0x08  f64  width (the window schedule's WDTH: 32 for a 2846, 36 for a 3080)
//! A+0x12  f64  depth, 0.625 in most objects, 0 in some
//! A+0x1A  f64  height (HGHT: 54 for a 2846, 96 for a 3080)
//! A+0x22  f64  head height above the floor (84 for a window with a 30" sill)
//! ```
//!
//! Verified on one project: all 34 windows of its window schedule decode
//! with the scheduled width and height (two 48x60 windows read 46 and 45.5),
//! 43 of 46 door objects decode, and with the position taken as the centre
//! every opening lies in a gap of the wall outlines of the project's PDF
//! (taking it as an edge puts 0.6 to 1.0 of the opening over the wall).
//!
//! In schematic-design files the sizes are not on a grid (32.0851" wide
//! windows), so only the depth is checked against a grid.
//!
//! Doors also carry two flag bytes at fixed distances from `A`:
//!
//! ```text
//! A+0x2A4  u8  swing side: 0 = to the left of the wall direction, 1 = right
//! A+0x2FD  u8  hinge: 0 = at the wall-start jamb, 1 = at the wall-end jamb
//! ```
//!
//! found by comparing the byte values of 25 doors with the swing arcs drawn
//! in one project's PDF (every value separates the 25 doors perfectly, all
//! four hinge/swing combinations occur: 6 start-left, 6 start-right, 5
//! end-left, 8 end-right). Not checked on windows (casement swings) or on
//! X17 files; a byte outside 0 and 1 leaves the defaults.
//!
//! Not decoded: door style (a doorway and a hinged door both carry the
//! style string `Door P04`; `Garage` is recognised by its style string), the
//! window kind (fixed, casement), and a few percent of door objects whose
//! variant keeps the numbers elsewhere.

use super::tree::{fin, on_grid, ObjectTree};
use plan_core::model::OpeningKind;
use plan_core::openings::OpeningStyle;

/// Door class id.
pub const DOOR: u8 = 9;
/// Window class id.
pub const WINDOW: u8 = 10;
/// First offset searched for the run.
const SEARCH_FROM: usize = 0x100;
/// Last offset searched.
const SEARCH_TO: usize = 0x800;
/// Objects smaller than this are not openings (a stray class 9 of 1.6 KB).
const MIN_SIZE: usize = 3000;

/// One decoded opening, in Chief's terms.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefOpening {
    pub node: usize,
    /// Tree index of the host wall.
    pub wall_node: usize,
    pub kind: OpeningKind,
    /// Centre distance from the wall start.
    pub center: f64,
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    /// Head height above the floor.
    pub top: f64,
    /// The object carries the style string `Garage`.
    pub garage: bool,
    /// Doors only: hinge at the wall-end jamb (`A+0x2FD`), when the byte is 0/1.
    pub hinge_at_end: Option<bool>,
    /// Doors only: the leaf swings to the right of the wall direction
    /// (`A+0x2A4`), when the byte is 0/1.
    pub swing_right: Option<bool>,
}

fn flag(bytes: &[u8], at: usize) -> Option<bool> {
    match bytes.get(at) {
        Some(0) => Some(false),
        Some(1) => Some(true),
        _ => None,
    }
}

/// Finds the five-value run in an object, searching from `from`.
pub fn find_run(bytes: &[u8], marker: usize, len: usize) -> Option<(usize, [f64; 5])> {
    let to = SEARCH_TO.min(len.saturating_sub(0x30));
    (SEARCH_FROM..to).find_map(|a| {
        let at = marker + a;
        let w = fin(bytes, at + 8)?;
        if !(6.0..=400.0).contains(&w) {
            return None;
        }
        let d = fin(bytes, at + 0x12)?;
        let h = fin(bytes, at + 0x1A)?;
        let t = fin(bytes, at + 0x22)?;
        let pos = fin(bytes, at)?;
        // Widths and heights are free (a window dragged to 32.0851"); the
        // depth is 0 or a sixteenth multiple (0.625).
        let depth_ok = d == 0.0 || ((0.05..=30.0).contains(&d) && on_grid(d, 16.0));
        let ok = depth_ok
            && (6.0..=400.0).contains(&h)
            && (h - 1.0..=h + 400.0).contains(&t)
            && (0.0..=20_000.0).contains(&pos);
        ok.then_some((a, [pos, w, d, h, t]))
    })
}

fn contains_ascii(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Door and window nodes below `wall_node` (children, grandchildren ...),
/// not descending into another wall.
fn opening_nodes(tree: &ObjectTree, wall_node: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack: Vec<usize> = tree.children(wall_node).iter().rev().copied().collect();
    while let Some(c) = stack.pop() {
        let n = tree.node(c);
        match (n.class, n.version) {
            (DOOR, 0) | (WINDOW, 0) => out.push(c),
            (6, _) => {}
            _ => stack.extend(tree.children(c).iter().rev().copied()),
        }
    }
    out
}

/// Decodes the openings (doors and windows) hosted by the wall `wall_node`.
pub fn decode_openings(bytes: &[u8], tree: &ObjectTree, wall_node: usize) -> Vec<ChiefOpening> {
    let mut out = Vec::new();
    for c in opening_nodes(tree, wall_node) {
        let n = tree.node(c);
        let kind = if n.class == DOOR {
            OpeningKind::Door
        } else {
            OpeningKind::Window
        };
        if n.len() < MIN_SIZE {
            continue;
        }
        let Some((a, [pos, w, d, h, t])) = find_run(bytes, n.marker, n.len()) else {
            continue;
        };
        let door = kind == OpeningKind::Door;
        out.push(ChiefOpening {
            hinge_at_end: door.then(|| flag(bytes, n.marker + a + 0x2FD)).flatten(),
            swing_right: door.then(|| flag(bytes, n.marker + a + 0x2A4)).flatten(),
            node: c,
            wall_node,
            kind,
            center: pos,
            width: w,
            depth: d,
            height: h,
            top: t,
            garage: contains_ascii(&bytes[n.marker..n.end], b"Garage"),
        });
    }
    out
}

/// Counts door and window objects (any size) hosted by walls but not decoded.
pub fn undecoded(bytes: &[u8], tree: &ObjectTree, wall_node: usize) -> usize {
    opening_nodes(tree, wall_node)
        .into_iter()
        .filter(|&c| {
            let n = tree.node(c);
            n.len() >= MIN_SIZE && find_run(bytes, n.marker, n.len()).is_none()
        })
        .count()
}

/// Plan Studio style of a decoded opening (Low confidence for doors: width
/// and the `Garage` string only).
pub fn style_of(o: &ChiefOpening) -> OpeningStyle {
    match o.kind {
        OpeningKind::Window => OpeningStyle::Window,
        OpeningKind::Door => {
            if o.garage {
                OpeningStyle::Garage
            } else if o.top - o.height > 0.5 {
                OpeningStyle::PassThrough
            } else if o.width >= 150.0 {
                OpeningStyle::Sliding
            } else if o.width > 100.0 {
                OpeningStyle::Doorway
            } else if o.width >= 54.0 {
                OpeningStyle::DoubleDoor
            } else {
                OpeningStyle::Hinged
            }
        }
    }
}

/// Sill height: head height minus height, never negative.
pub fn sill_of(o: &ChiefOpening) -> f64 {
    (o.top - o.height).max(0.0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::tree::testutil::*;
    use crate::import::walls::tests::{line_obj, wall_obj};

    /// A door (class 9) or window (class 10) with the run at `a`.
    pub fn opening_obj(class: u8, a: usize, run: [f64; 5], extra: &[u8]) -> Vec<u8> {
        opening_obj_flags(class, a, run, extra, (0, 0))
    }

    /// Like [`opening_obj`] with the swing-side and hinge bytes set.
    pub fn opening_obj_flags(
        class: u8,
        a: usize,
        run: [f64; 5],
        extra: &[u8],
        (side, hinge): (u8, u8),
    ) -> Vec<u8> {
        sized(class, 0, 0xC00, |b| {
            b[a + 0x2A4] = side;
            b[a + 0x2FD] = hinge;
            // A decoy before the run: width 1.25 fails the range check.
            put_f64(b, 0x120, 1.25);
            put_f64(b, 0x128, 6.75);
            for (k, off) in [0usize, 8, 0x12, 0x1A, 0x22].into_iter().enumerate() {
                put_f64(b, a + off, run[k]);
            }
            b[0x800..0x800 + extra.len()].copy_from_slice(extra);
        })
    }

    #[test]
    fn windows_and_doors_in_two_variants() {
        let win = opening_obj(WINDOW, 0x312, [62.0, 32.0, 0.625, 54.0, 84.0], b"");
        let door = opening_obj(DOOR, 0x3F5, [96.0, 192.0, 0.0, 96.0, 96.0], b"");
        let garage = opening_obj(
            DOOR,
            0x312,
            [300.0, 108.0, 0.625, 96.0, 96.0],
            b"Garage Door CHD05",
        );
        let wall = wall_obj(
            line_obj(0.0, 0.0, 1.0, 0.0, 400.0),
            None,
            None,
            &[win, door, garage],
        );
        let tree = ObjectTree::build(&wall);
        let w = tree.of_kind(6, 0).next().unwrap();
        let ops = decode_openings(&wall, &tree, w);
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].kind, OpeningKind::Window);
        assert_eq!(
            (ops[0].center, ops[0].width, ops[0].height, ops[0].top),
            (62.0, 32.0, 54.0, 84.0)
        );
        assert_eq!(sill_of(&ops[0]), 30.0);
        assert_eq!(style_of(&ops[0]), OpeningStyle::Window);
        assert_eq!(ops[1].kind, OpeningKind::Door);
        assert_eq!(ops[1].width, 192.0);
        assert_eq!(style_of(&ops[1]), OpeningStyle::Sliding);
        assert_eq!(sill_of(&ops[1]), 0.0);
        assert!(ops[2].garage);
        assert_eq!(style_of(&ops[2]), OpeningStyle::Garage);
        assert_eq!(undecoded(&wall, &tree, w), 0);
    }

    #[test]
    fn door_hinge_and_swing_flags() {
        let right_end =
            opening_obj_flags(DOOR, 0x312, [60.0, 32.0, 0.625, 96.0, 96.0], b"", (1, 1));
        let left_start =
            opening_obj_flags(DOOR, 0x312, [120.0, 32.0, 0.625, 96.0, 96.0], b"", (0, 0));
        let odd = opening_obj_flags(DOOR, 0x312, [180.0, 32.0, 0.625, 96.0, 96.0], b"", (7, 9));
        let win = opening_obj_flags(WINDOW, 0x312, [240.0, 32.0, 0.625, 54.0, 84.0], b"", (1, 1));
        let wall = wall_obj(
            line_obj(0.0, 0.0, 1.0, 0.0, 400.0),
            None,
            None,
            &[right_end, left_start, odd, win],
        );
        let tree = ObjectTree::build(&wall);
        let w = tree.of_kind(6, 0).next().unwrap();
        let ops = decode_openings(&wall, &tree, w);
        assert_eq!(ops.len(), 4);
        assert_eq!(
            (ops[0].swing_right, ops[0].hinge_at_end),
            (Some(true), Some(true))
        );
        assert_eq!(
            (ops[1].swing_right, ops[1].hinge_at_end),
            (Some(false), Some(false))
        );
        // Bytes other than 0/1 leave the defaults; windows are not read.
        assert_eq!((ops[2].swing_right, ops[2].hinge_at_end), (None, None));
        assert_eq!((ops[3].swing_right, ops[3].hinge_at_end), (None, None));
    }

    #[test]
    fn style_heuristics_by_width() {
        let mk = |kind, width: f64, h: f64, top: f64| ChiefOpening {
            hinge_at_end: None,
            swing_right: None,
            node: 0,
            wall_node: 0,
            kind,
            center: 0.0,
            width,
            depth: 0.0,
            height: h,
            top,
            garage: false,
        };
        assert_eq!(
            style_of(&mk(OpeningKind::Door, 32.0, 96.0, 96.0)),
            OpeningStyle::Hinged
        );
        assert_eq!(
            style_of(&mk(OpeningKind::Door, 72.0, 96.0, 96.0)),
            OpeningStyle::DoubleDoor
        );
        assert_eq!(
            style_of(&mk(OpeningKind::Door, 108.0, 96.0, 96.0)),
            OpeningStyle::Doorway
        );
        assert_eq!(
            style_of(&mk(OpeningKind::Door, 60.0, 24.0, 40.0)),
            OpeningStyle::PassThrough
        );
        assert_eq!(sill_of(&mk(OpeningKind::Door, 60.0, 24.0, 40.0)), 16.0);
        assert_eq!(sill_of(&mk(OpeningKind::Window, 30.0, 54.0, 50.0)), 0.0);
    }

    #[test]
    fn rejects_small_objects_and_missing_runs() {
        let tiny = sized(DOOR, 0, 1660, |_| {});
        let blank = sized(WINDOW, 0, 0xC00, |_| {});
        let wall = wall_obj(
            line_obj(0.0, 0.0, 1.0, 0.0, 400.0),
            None,
            None,
            &[tiny, blank],
        );
        let tree = ObjectTree::build(&wall);
        let w = tree.of_kind(6, 0).next().unwrap();
        assert!(decode_openings(&wall, &tree, w).is_empty());
        // The tiny object is ignored altogether, the blank one is counted.
        assert_eq!(undecoded(&wall, &tree, w), 1);
    }
}
