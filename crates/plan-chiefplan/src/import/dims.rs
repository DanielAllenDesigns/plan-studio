//! Linear dimension strings (class 24).
//!
//! A dimension object stores its measured points as records of 909 bytes in
//! X18 files and 554 bytes in X17 files (checked on one X17 job: the 106
//! dimension objects have the sizes 1,962, 2,516 and 3,070 bytes, two, three and
//! four points, a difference of 554 per point). The first point appears twice,
//! 217 bytes apart (at +481 and +698 in the X18 projects, +733 or +735 and +950
//! or +952 in the X17 ones), then one record per further point at
//! `first + 217 + stride * k`. Dimension points sit exactly on wall corners: 159
//! of the point pairs of one project's first-floor dimensions coincide with
//! corners in its PDF.
//!
//! What was **not** found: the position of the dimension line (Chief seems
//! to place strings automatically at a distance the file does not repeat),
//! the text override, the dimension set and the arrow style. The importer
//! therefore creates one Plan Studio dimension per consecutive pair of
//! points with an *assumed* offset on the side away from the floor's walls
//! (see `assumed_offset`). Only about half of the class 24 objects have this
//! shape; the rest (angular, radial, elevation and leader dimensions) are
//! skipped and counted.

use super::tree::{fin, ObjectTree};

/// Dimension class id.
pub const DIMENSION: u8 = 24;
/// Bytes per point record in X18 files.
const RECORD: usize = 909;
/// Bytes per point record in X17 files.
const RECORD_X17: usize = 554;
/// Bytes per point record in the older `01 CA E2 0E` files (X16): the sizes
/// 1,905, 2,439 and 2,973 differ by 534 per point.
const RECORD_X16: usize = 534;
const DUP_GAP: usize = 217;
/// Offset assumed for the dimension line, inches.
pub const ASSUMED_OFFSET: f64 = 36.0;

/// A string of measured points.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefDimension {
    pub node: usize,
    pub points: Vec<(f64, f64)>,
}

fn plausible(x: f64, y: f64) -> bool {
    x.abs() > 1.0 && y.abs() > 1.0 && x.abs() < 1.0e5 && y.abs() < 1.0e5
}

/// Decodes the dimension at tree index `node`, if it has the string shape. The
/// record sizes of X18, X17 and X16 are tried in that order; the longest string wins.
pub fn decode_dimension(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefDimension> {
    let mut best: Option<ChiefDimension> = None;
    for record in [RECORD, RECORD_X17, RECORD_X16] {
        if let Some(d) = decode_with(bytes, tree, node, record) {
            if best
                .as_ref()
                .is_none_or(|b| d.points.len() > b.points.len())
            {
                best = Some(d);
            }
        }
    }
    best
}

fn decode_with(
    bytes: &[u8],
    tree: &ObjectTree,
    node: usize,
    record: usize,
) -> Option<ChiefDimension> {
    let n = tree.node(node);
    let len = n.len();
    if len < 0x100 + DUP_GAP + 16 {
        return None;
    }
    let first = (0x100..len.saturating_sub(DUP_GAP + 16).min(0x400)).find(|&o| {
        let (a, b) = (fin(bytes, n.marker + o), fin(bytes, n.marker + o + 8));
        let (c, d) = (
            fin(bytes, n.marker + o + DUP_GAP),
            fin(bytes, n.marker + o + DUP_GAP + 8),
        );
        matches!((a, b, c, d), (Some(a), Some(b), Some(c), Some(d)) if a == c && b == d && plausible(a, b))
    })?;
    let mut points = vec![(
        fin(bytes, n.marker + first)?,
        fin(bytes, n.marker + first + 8)?,
    )];
    let mut o = first + DUP_GAP + record;
    while o + 16 <= len {
        match (fin(bytes, n.marker + o), fin(bytes, n.marker + o + 8)) {
            (Some(x), Some(y)) if plausible(x, y) => points.push((x, y)),
            _ => break,
        }
        o += record;
    }
    (points.len() >= 2).then_some(ChiefDimension { node, points })
}

/// The side (+1 left, -1 right of start-to-end) and size of the assumed
/// offset: away from `centre` (the middle of the floor's walls).
pub fn assumed_offset(start: (f64, f64), end: (f64, f64), centre: (f64, f64)) -> f64 {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        return ASSUMED_OFFSET;
    }
    let (nx, ny) = (-dy / len, dx / len);
    let mid = ((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0);
    let side = (mid.0 - centre.0) * nx + (mid.1 - centre.1) * ny;
    if side >= 0.0 {
        ASSUMED_OFFSET
    } else {
        -ASSUMED_OFFSET
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::tree::testutil::*;

    /// A dimension with the points in its records.
    pub fn dim_obj(points: &[(f64, f64)]) -> Vec<u8> {
        let total = 0x1e1 + DUP_GAP + RECORD * points.len() + 40;
        sized(DIMENSION, 0, total, |b| {
            let first = 0x1e1;
            put_f64(b, first, points[0].0);
            put_f64(b, first + 8, points[0].1);
            put_f64(b, first + DUP_GAP, points[0].0);
            put_f64(b, first + DUP_GAP + 8, points[0].1);
            for (k, p) in points.iter().enumerate().skip(1) {
                let o = first + DUP_GAP + RECORD * k;
                put_f64(b, o, p.0);
                put_f64(b, o + 8, p.1);
            }
        })
    }

    #[test]
    fn decodes_a_string_of_points() {
        let pts = [(459.47, 773.61), (581.97, 773.61), (699.47, 773.61)];
        let body = dim_obj(&pts);
        let tree = ObjectTree::build(&body);
        let i = tree.of_kind(DIMENSION, 0).next().unwrap();
        let d = decode_dimension(&body, &tree, i).unwrap();
        assert_eq!(d.points.len(), 3);
        assert_eq!(d.points[1], (581.97, 773.61));
        assert_eq!(d.points[2], (699.47, 773.61));
    }

    /// A dimension in the X17 layout: 554 bytes per point record.
    fn dim_obj_x17(points: &[(f64, f64)]) -> Vec<u8> {
        let total = 735 + DUP_GAP + RECORD_X17 * points.len() + 40;
        sized(DIMENSION, 0, total, |b| {
            let first = 735;
            put_f64(b, first, points[0].0);
            put_f64(b, first + 8, points[0].1);
            put_f64(b, first + DUP_GAP, points[0].0);
            put_f64(b, first + DUP_GAP + 8, points[0].1);
            for (k, p) in points.iter().enumerate().skip(1) {
                let o = first + DUP_GAP + RECORD_X17 * k;
                put_f64(b, o, p.0);
                put_f64(b, o + 8, p.1);
            }
        })
    }

    #[test]
    fn decodes_the_x17_record_size() {
        let pts = [(896.39, 504.17), (896.39, 514.17), (900.0, 600.5)];
        let body = dim_obj_x17(&pts);
        let tree = ObjectTree::build(&body);
        let i = tree.of_kind(DIMENSION, 0).next().unwrap();
        let d = decode_dimension(&body, &tree, i).unwrap();
        assert_eq!(d.points, pts.to_vec());
        // The X18 stride finds only the first point there, so the X17 one wins.
        assert_eq!(
            decode_with(&body, &tree, i, RECORD).map(|d| d.points.len()),
            None
        );
    }

    #[test]
    fn decodes_the_x16_record_size() {
        let pts = [(100.0, 200.0), (100.0, 260.5), (140.0, 300.0)];
        let total = 735 + DUP_GAP + RECORD_X16 * pts.len() + 40;
        let body = sized(DIMENSION, 0, total, |b| {
            let first = 735;
            put_f64(b, first, pts[0].0);
            put_f64(b, first + 8, pts[0].1);
            put_f64(b, first + DUP_GAP, pts[0].0);
            put_f64(b, first + DUP_GAP + 8, pts[0].1);
            for (k, p) in pts.iter().enumerate().skip(1) {
                let o = first + DUP_GAP + RECORD_X16 * k;
                put_f64(b, o, p.0);
                put_f64(b, o + 8, p.1);
            }
        });
        let tree = ObjectTree::build(&body);
        let i = tree.of_kind(DIMENSION, 0).next().unwrap();
        assert_eq!(
            decode_dimension(&body, &tree, i).unwrap().points,
            pts.to_vec()
        );
    }

    #[test]
    fn rejects_objects_without_the_duplicate_pair() {
        let body = sized(DIMENSION, 0, 0x600, |_| {});
        let tree = ObjectTree::build(&body);
        let i = tree.of_kind(DIMENSION, 0).next().unwrap();
        assert!(decode_dimension(&body, &tree, i).is_none());
        let tiny = sized(DIMENSION, 0, 0x80, |_| {});
        let tree = ObjectTree::build(&tiny);
        let i = tree.of_kind(DIMENSION, 0).next().unwrap();
        assert!(decode_dimension(&tiny, &tree, i).is_none());
    }

    #[test]
    fn offset_points_away_from_the_centre() {
        // Left of start->end (+x) is +y. A string below the centre goes right.
        assert_eq!(
            assumed_offset((0.0, 0.0), (100.0, 0.0), (50.0, 200.0)),
            -36.0
        );
        assert_eq!(
            assumed_offset((0.0, 300.0), (100.0, 300.0), (50.0, 200.0)),
            36.0
        );
        assert_eq!(assumed_offset((5.0, 5.0), (5.0, 5.0), (0.0, 0.0)), 36.0);
    }
}
