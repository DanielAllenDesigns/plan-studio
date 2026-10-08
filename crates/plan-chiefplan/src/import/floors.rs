//! Floors (class 30) of a plan file.
//!
//! A plan holds its floors as top-level class 30 objects, in file order
//! foundation, first floor, second floor ... (Daniel's template has four,
//! every project read so far has four). Each carries three `f64`s, each one
//! preceded by a flag byte:
//!
//! ```text
//! +0x278  f64  elevation of the floor relative to the first floor, inches
//!              (-132.25 foundation, 0 first floor, 137.875 second floor)
//! +0x281  f64  ceiling height as Plan Studio's `Floor::ceiling_height`
//!              (115.5, 121.125, 109.125, 97.125: ceiling + 1 1/8")
//! +0x28a  f64  not identified (118.0, 118.0, 114.5, 23.25)
//! ```
//!
//! The same triple sits at `+0x1fc/+0x205/+0x20e` in files saved by X17
//! (header `01 CA 7F 0F`; X18 files start `01 CA 1A 10`). When neither fixed
//! position validates, the triple is searched for in the first 0x400 bytes.

use super::tree::{fin, on_grid, ObjectTree};
use crate::decode::class;

/// Floor class id.
pub const FLOOR: u8 = 30;

/// Decoded header of one floor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChiefFloor {
    /// Tree index of the class 30 object.
    pub node: usize,
    /// Elevation relative to the first floor, inches.
    pub elevation: f64,
    /// Ceiling height value, inches.
    pub ceiling: f64,
    /// The third value (unidentified).
    pub third: f64,
    /// Whether the header triple was found (otherwise defaults are used).
    pub found: bool,
}

/// File generation by header magic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generation {
    X17,
    X18,
    Unknown,
}

impl Generation {
    pub fn of(bytes: &[u8]) -> Generation {
        match bytes.get(2..4) {
            Some([0x1A, 0x10]) => Generation::X18,
            Some([0x7F, 0x0F]) => Generation::X17,
            _ => Generation::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Generation::X17 => "X17 (01 CA 7F 0F)",
            Generation::X18 => "X18 (01 CA 1A 10)",
            Generation::Unknown => "unknown generation",
        }
    }

    /// Offset of the elevation `f64` in a floor object.
    fn floor_offset(self) -> Option<usize> {
        match self {
            Generation::X18 => Some(0x278),
            Generation::X17 => Some(0x1FC),
            Generation::Unknown => None,
        }
    }
}

/// Top-level floor nodes (class 30 not nested in another class 30).
pub fn floor_nodes(tree: &ObjectTree) -> Vec<usize> {
    tree.of_kind(FLOOR, 0)
        .filter(|&i| tree.ancestor_of_class(i, FLOOR).is_none())
        .collect()
}

/// Whether the triple at `o` (relative to the marker) looks right. `min_ceil`
/// is 24" when searching blindly and 4" at the known offsets (a project with
/// a slab floor stores -8, 8, 4 there).
fn triple_at(b: &[u8], base: usize, o: usize, min_ceil: f64) -> Option<(f64, f64, f64)> {
    let elev = fin(b, base + o)?;
    let ceil = fin(b, base + o + 9)?;
    let third = fin(b, base + o + 18)?;
    let flags_ok =
        matches!(b.get(base + o + 8), Some(0 | 1)) && matches!(b.get(base + o + 17), Some(0 | 1));
    let nice = |v: f64| v == 0.0 || v.abs() >= 1e-3;
    let ok = flags_ok
        && nice(elev)
        && nice(third)
        && (min_ceil..=300.0).contains(&ceil)
        && on_grid(ceil, 8.0)
        && (-600.0..=3000.0).contains(&elev)
        && (elev == 0.0 || on_grid(elev, 8.0))
        && (-1.0..=600.0).contains(&third);
    ok.then_some((elev, ceil, third))
}

/// Decodes the header of the floor at tree index `node`.
pub fn decode_floor(
    bytes: &[u8],
    tree: &ObjectTree,
    node: usize,
    generation: Generation,
) -> ChiefFloor {
    let n = tree.node(node);
    let base = n.marker;
    let limit = n.len().min(0x420);
    let fixed = generation
        .floor_offset()
        .filter(|&o| o + 26 <= limit)
        .and_then(|o| triple_at(bytes, base, o, 4.0));
    let hit = fixed.or_else(|| {
        // Several offsets can validate (a shifted triple reads the elevation
        // as the ceiling); prefer the most non-zero values, then the earliest.
        (0x150..limit.saturating_sub(26))
            .filter_map(|o| triple_at(bytes, base, o, 24.0).map(|t| (o, t)))
            .max_by_key(|&(o, (a, b, c))| {
                let score = [a, b, c].iter().filter(|v| **v != 0.0).count();
                (score, std::cmp::Reverse(o))
            })
            .map(|(_, t)| t)
    });
    match hit {
        Some((elevation, ceiling, third)) => ChiefFloor {
            node,
            elevation,
            ceiling,
            third,
            found: true,
        },
        None => ChiefFloor {
            node,
            elevation: 0.0,
            ceiling: crate::import::DEFAULT_CEILING,
            third: 0.0,
            found: false,
        },
    }
}

/// Decodes every top-level floor, in file order.
pub fn decode_floors(bytes: &[u8], tree: &ObjectTree, generation: Generation) -> Vec<ChiefFloor> {
    floor_nodes(tree)
        .into_iter()
        .map(|i| decode_floor(bytes, tree, i, generation))
        .collect()
}

/// Index (into `floors`) of the floor that contains tree node `i`.
pub fn floor_of(tree: &ObjectTree, floors: &[ChiefFloor], i: usize) -> Option<usize> {
    floors
        .iter()
        .position(|f| f.node != i && tree.contains(f.node, i))
}

/// Raw class label used in reports for the floor-level classes this crate
/// does not turn into Plan Studio objects, with the confidence of the guess.
/// `(class, version) -> (label, confidence)`.
pub fn class_label(class_id: u8, version: u8) -> Option<(&'static str, &'static str)> {
    Some(match (class_id, version) {
        (6, 0) => ("wall", "High"),
        (9, 0) => ("door", "High"),
        (10, 0) => ("window", "High"),
        (23, 0) => ("room", "High"),
        (24, 0) => ("dimension", "Medium"),
        (25, 0) => ("text", "Medium"),
        (30, 0) => ("floor", "High"),
        (15, 0) => ("cabinet / box (cabinet, shelf, soffit)", "High"),
        (16, 0) => ("furniture group", "Medium"),
        (21, 0) => ("electrical device", "High"),
        (46, 0) => ("stair flight", "Medium"),
        (47, 0) => ("stair landing", "Medium"),
        (50, 0) => ("roof plane", "Medium"),
        (50, 1) => ("roof plane part", "Low"),
        (18, 1) => ("molding polyline (ridge cap, cornice)", "Medium"),
        (79, 0) => ("wall framing assembly", "Medium"),
        (68, 0) => ("framing member", "High"),
        (48, 0) => ("text label (room name / area macro)", "High"),
        (40, 0) => ("angled line", "Medium"),
        (31, 0) => ("line", "High"),
        (4, 0) => ("point", "High"),
        (120, 1) => ("material list component", "High"),
        (93, 0) | (122, 0) => ("molding / trim set", "Low"),
        (54, 0) => ("wall tag", "Low"),
        (81, 1) => ("wall connection record", "Medium"),
        (109, 0) => ("countertop (island) / countertop part", "Medium"),
        (123, 0) => ("library object (fixture, appliance, furniture)", "High"),
        (114, 0) => ("library object entry (name, tags)", "Medium"),
        (209, 0) => ("area / region object", "Low"),
        (52, 0) => ("floor/ceiling/roof surface", "Low"),
        (34, 1) | (36, 1) => ("unidentified plan object", "None"),
        _ => return None,
    })
}

/// The wall type class, re-exported for the wall decoder.
pub(crate) const WALL_TYPE: u8 = class::WALL_TYPE;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::tree::testutil::*;

    /// A floor object with the elevation triple at the X18 offsets.
    pub fn floor_obj(elev: f64, ceiling: f64, children: &[Vec<u8>]) -> Vec<u8> {
        let kids: usize = children.iter().map(|c| c.len() - 1).sum();
        let total = 0x2c0 + kids;
        let mut obj = sized(FLOOR, 0, total, |buf| {
            put_f64(buf, 0x278, elev);
            put_f64(buf, 0x281, ceiling);
            put_f64(buf, 0x28a, 118.0);
            let mut at = 0x2c0;
            for c in children {
                buf[at..at + c.len() - 1].copy_from_slice(&c[1..]);
                at += c.len() - 1;
            }
        });
        // `sized` computed the size from `total`, which already counts the
        // children.
        obj.shrink_to_fit();
        obj
    }

    #[test]
    fn reads_the_triple_at_the_x18_offsets() {
        let mut body = Vec::new();
        body.extend(floor_obj(-132.25, 115.5, &[]));
        body.extend(floor_obj(0.0, 121.125, &[]));
        body.extend(floor_obj(137.875, 109.125, &[]));
        let tree = ObjectTree::build(&body);
        let mut file = vec![0u8; 4];
        file[2] = 0x1A;
        file[3] = 0x10;
        let floors = decode_floors(&body, &tree, Generation::of(&file));
        assert_eq!(floors.len(), 3);
        assert_eq!(floors[0].elevation, -132.25);
        assert_eq!(floors[0].ceiling, 115.5);
        assert_eq!(floors[1].elevation, 0.0);
        assert_eq!(floors[2].ceiling, 109.125);
        assert!(floors.iter().all(|f| f.found));
    }

    #[test]
    fn finds_the_triple_when_the_offset_is_unknown() {
        // X17 layout, generation not recognised: found by search.
        let obj = sized(FLOOR, 0, 0x400, |buf| {
            put_f64(buf, 0x1fc, 137.875);
            put_f64(buf, 0x205, 109.125);
            put_f64(buf, 0x20e, 114.5);
            // A decoy that fails validation: two adjacent 128.0.
            put_f64(buf, 0x17f, 128.0);
            put_f64(buf, 0x187, 128.0);
        });
        let tree = ObjectTree::build(&obj);
        let f = decode_floors(&obj, &tree, Generation::Unknown);
        assert_eq!(f.len(), 1);
        assert!(f[0].found);
        assert_eq!((f[0].elevation, f[0].ceiling), (137.875, 109.125));
    }

    #[test]
    fn missing_triple_falls_back_to_defaults() {
        let obj = sized(FLOOR, 0, 0x300, |_| {});
        let tree = ObjectTree::build(&obj);
        let f = decode_floors(&obj, &tree, Generation::X18);
        assert!(!f[0].found);
        assert_eq!(f[0].ceiling, crate::import::DEFAULT_CEILING);
    }

    #[test]
    fn nested_floors_are_not_floors_and_labels_resolve() {
        let inner = floor_obj(0.0, 108.0, &[]);
        let outer = floor_obj(0.0, 120.0, &[inner]);
        let tree = ObjectTree::build(&outer);
        assert_eq!(floor_nodes(&tree).len(), 1);
        assert_eq!(class_label(6, 0).unwrap().0, "wall");
        assert!(class_label(200, 0).is_none());
        assert_eq!(Generation::of(&[1, 0xCA, 0x7F, 0x0F]), Generation::X17);
        assert_eq!(Generation::of(&[]), Generation::Unknown);
    }
}
