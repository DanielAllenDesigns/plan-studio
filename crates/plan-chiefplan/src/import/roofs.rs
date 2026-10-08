//! Roof planes (class 50 version 0, directly on a floor).
//!
//! (Stage 1 listed class 18 as "roof plane"; class 18 is the molding polyline:
//! ridge caps, cornices and fascia runs with the profile strings `Ogee`,
//! `CA-001`, `Default Ridge Cap`.)
//!
//! A roof plane is a large class 50 object (31 to 56 KB) holding
//!
//! 1. an **outline**: a closed chain of 349 to 365 byte line records
//!    (`04 20 x y dx dy length ...`, see [`lines`](super::lines)), the
//!    plane's outline in plan including its overhang, with zero-length edges
//!    between some edges;
//! 2. a **list of object ids** (the wall and plane neighbours, 8 bytes each);
//! 3. one more line record that does not continue the outline: the
//!    **baseline** (the eave line the pitch is measured from). Its record
//!    carries, counted from the start of that record,
//!
//! ```text
//! +0    f64 x, +8 f64 y, +16 dx, +24 dy, +32 length   the baseline
//! +55   f64  baseline height, inches above the first-floor datum
//! +76   f64  pitch angle, radians (atan(rise / 12)); +75 in X17 files
//! ```
//!
//! and then the child class 50 objects (the overhang ring and the footprint
//! polygon of the same plane; not read). Evidence on one job: 40 planes on three
//! floors; angles `0.7854` (12/12), `0.5880` (8/12), `0.6947` (10/12), `0.32175`
//! (4/12), `0.0416` (0.5/12) convert to the round pitches a designer types, the
//! baseline heights `254.58` (plate 247 + 7.58 heel) and `129.84` (first-floor
//! ceiling 121.125 + 8.7) sit just above the wall tops, the planes of the
//! attic floor have the bounding box `x 341 to 1313, y 94 to 1226` against
//! the second-floor walls' `353 to 1301, 118 to 1220` (12" overhang), and
//! the sum of their plan areas is 6,872 sq ft against a 4,200 sq ft footprint
//! (hips, valleys and dormers overlap).
//!
//! Plan Studio's record is the one `roof_view.rs` stores in `Floor.roofs`:
//! `{"kind":"plane","id","polygon3d":[[x,elevation,-y]...],"pitch" (rise per
//! 12),"baseline":[p0,p1],"auto":false,"holes":[],"source":null,"overhang",
//! "label","material","layer","ridge_caps","gutters"}`. The vertex heights
//! follow the plane through the baseline: `h = base + d * tan(angle)` where
//! `d` is the distance from the baseline line toward the up-slope side (the
//! side of the polygon's centroid), so the overhang beyond the baseline hangs
//! below it, as in Chief.
//!
//! Not decoded: hips and gables per edge (the flag bytes), the overhang and
//! fascia sizes, holes and skylights, the plane material. Planes with a
//! duplicate outline are dropped and counted. Other class 50 objects have a
//! chain of line records but no baseline record (the bytes where the pitch
//! would be read as the denormal `4.2e-314`, and no molding children): job A
//! has 3 of them, 22 to 79 KB each, probably ceiling or deck surfaces; they are
//! left out and counted.

use super::lines::{chain, find_edges, is_closed, polygon_of, signed_area, Edge};
use super::tree::{f64_at, ObjectTree};
use serde_json::{json, Value};

/// Roof plane class id.
pub const ROOF_PLANE: u8 = 50;
const BASELINE_HEIGHT: usize = 55;
/// X18 files; X17 files store the angle one byte earlier.
const BASELINE_ANGLE: usize = 76;
const BASELINE_ANGLE_X17: usize = 75;

/// One decoded roof plane.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefRoofPlane {
    pub node: usize,
    /// Closed outline in plan, counter-clockwise.
    pub outline: Vec<(f64, f64)>,
    /// The baseline segment.
    pub baseline: ((f64, f64), (f64, f64)),
    /// Height of the baseline above the first-floor datum, inches.
    pub base_height: f64,
    /// Pitch angle, radians.
    pub angle: f64,
}

impl ChiefRoofPlane {
    /// Rise per 12 of run.
    pub fn pitch(&self) -> f64 {
        12.0 * self.angle.tan()
    }

    /// Plan area, square inches.
    pub fn area(&self) -> f64 {
        signed_area(&self.outline).abs()
    }

    /// The outline vertices with their heights `[x, elevation, -y]` (Plan
    /// Studio's roof frame).
    pub fn polygon3d(&self) -> Vec<[f64; 3]> {
        let (a, b) = self.baseline;
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx.hypot(dy).max(1e-9);
        let (mut nx, mut ny) = (-dy / len, dx / len);
        let n = self.outline.len() as f64;
        let (cx, cy) = self
            .outline
            .iter()
            .fold((0.0, 0.0), |s, p| (s.0 + p.0 / n, s.1 + p.1 / n));
        if (cx - a.0) * nx + (cy - a.1) * ny < 0.0 {
            nx = -nx;
            ny = -ny;
        }
        let k = self.angle.tan();
        self.outline
            .iter()
            .map(|&(x, y)| {
                let d = (x - a.0) * nx + (y - a.1) * ny;
                [x, self.base_height + d * k, -y]
            })
            .collect()
    }
}

/// Decodes the class 50 object at tree index `node` (a floor-level plane).
pub fn decode_plane(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefRoofPlane> {
    let n = tree.node(node);
    if n.len() < 8000 {
        return None;
    }
    // Only the object's own records: stop at its first child.
    let own_end = tree
        .children(node)
        .first()
        .map_or(n.end, |&c| tree.node(c).marker);
    let edges = find_edges(bytes, n.marker, n.marker + 0x40, own_end);
    if edges.len() < 4 {
        return None;
    }
    let outline_chain = chain(&edges, 0);
    if !is_closed(&outline_chain) {
        return None;
    }
    // The baseline is the first record after the outline chain.
    let baseline: &Edge = edges.get(outline_chain.len())?;
    let rec = n.marker + baseline.offset;
    let angle = pitch_angle(bytes, rec)?;
    let base_height =
        f64_at(bytes, rec + BASELINE_HEIGHT).filter(|h| (-400.0..2000.0).contains(h))?;
    if baseline.len < 1.0 {
        return None;
    }
    let mut outline = polygon_of(&outline_chain);
    if outline.len() < 3 {
        return None;
    }
    if signed_area(&outline) < 0.0 {
        outline.reverse();
    }
    Some(ChiefRoofPlane {
        node,
        outline,
        baseline: (baseline.start(), baseline.end()),
        base_height,
        angle,
    })
}

/// The pitch angle of a baseline record starting at `rec`: the candidate offset
/// (76 for X18, 75 for X17) whose rise per 12 is a multiple of a quarter inch
/// (every pitch of 80 planes of two jobs is); else the X18 offset when it is a
/// plausible angle.
fn pitch_angle(bytes: &[u8], rec: usize) -> Option<f64> {
    let read = |o: usize| f64_at(bytes, rec + o).filter(|a| (0.0..1.5).contains(a));
    let round = |a: f64| {
        let p = 12.0 * a.tan() * 4.0;
        (p - p.round()).abs() < 0.01
    };
    let found = [BASELINE_ANGLE, BASELINE_ANGLE_X17]
        .iter()
        .filter_map(|&o| read(o))
        .find(|&a| a >= 1e-3 && round(a));
    // A flat roof stores exactly 0.0; denormal residue (4e-314) means the
    // record is not a roof baseline at all.
    found.or_else(|| {
        (read(BASELINE_ANGLE) == Some(0.0) || read(BASELINE_ANGLE_X17) == Some(0.0)).then_some(0.0)
    })
}

/// Whether two planes have the same outline (corner by corner, within 0.1").
pub fn same_outline(a: &ChiefRoofPlane, b: &ChiefRoofPlane) -> bool {
    if (a.area() - b.area()).abs() > 1.0 {
        return false;
    }
    a.outline.iter().all(|p| {
        b.outline
            .iter()
            .any(|q| (p.0 - q.0).abs() < 0.1 && (p.1 - q.1).abs() < 0.1)
    }) && b.outline.iter().all(|q| {
        a.outline
            .iter()
            .any(|p| (p.0 - q.0).abs() < 0.1 && (p.1 - q.1).abs() < 0.1)
    })
}

/// The `Floor.roofs` entry of a plane.
pub fn plane_json(p: &ChiefRoofPlane, id: u64, layer: &str) -> Value {
    json!({
        "kind": "plane",
        "id": id,
        "polygon3d": p.polygon3d(),
        "pitch": p.pitch(),
        "baseline": [
            {"x": p.baseline.0 .0, "y": p.baseline.0 .1},
            {"x": p.baseline.1 .0, "y": p.baseline.1 .1},
        ],
        "auto": false,
        "holes": [],
        "source": null,
        "overhang": 0.0,
        "label": "",
        "material": "Asphalt Shingles",
        "layer": layer,
        "ridge_caps": false,
        "gutters": false,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::lines::tests::put_edge;
    use crate::import::tree::testutil::*;

    /// A plane: closed outline of `pts` (edge records 350 bytes apart from
    /// 671), a baseline record, then a child class 50 object.
    pub fn plane_obj(
        pts: &[(f64, f64)],
        baseline: (f64, f64, f64, f64, f64),
        height: f64,
        angle: f64,
    ) -> Vec<u8> {
        let child = sized(ROOF_PLANE, 0, 400, |_| {});
        let total = 9000 + child.len();
        sized(ROOF_PLANE, 0, total, |b| {
            let n = pts.len();
            for k in 0..n {
                let (p, q) = (pts[k], pts[(k + 1) % n]);
                let len = (q.0 - p.0).hypot(q.1 - p.1);
                put_edge(
                    b,
                    671 + 350 * k,
                    (p.0, p.1, (q.0 - p.0) / len, (q.1 - p.1) / len, len),
                );
            }
            let at = 671 + 350 * n + 200;
            put_edge(b, at, baseline);
            put_f64(b, at + 55, height);
            put_f64(b, at + 76, angle);
            let c = 8900;
            b[c..c + child.len() - 1].copy_from_slice(&child[1..]);
        })
    }

    #[test]
    fn decodes_outline_baseline_pitch_and_heights() {
        // A 100 x 60 rectangle; eave baseline along y = 0 (the south edge),
        // 8/12 pitch, baseline at 129.84.
        let angle = (8.0f64 / 12.0).atan();
        let pts = [(0.0, 0.0), (100.0, 0.0), (100.0, 60.0), (0.0, 60.0)];
        let obj = plane_obj(&pts, (0.0, 0.0, 1.0, 0.0, 100.0), 129.84, angle);
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(ROOF_PLANE, 0).next().unwrap();
        assert_eq!(tree.children(i).len(), 1);
        let p = decode_plane(&obj, &tree, i).unwrap();
        assert_eq!(p.outline.len(), 4);
        assert!((p.pitch() - 8.0).abs() < 1e-9);
        assert!((p.area() - 6000.0).abs() < 1e-6);
        let poly = p.polygon3d();
        assert_eq!(poly.len(), 4);
        // South vertices at the baseline height, north ones 60 * 8/12 = 40 higher.
        assert!((poly[0][1] - 129.84).abs() < 1e-9);
        assert!((poly[2][1] - (129.84 + 40.0)).abs() < 1e-9);
        // Frame [x, elevation, -y].
        assert_eq!(poly[2][0], 100.0);
        assert_eq!(poly[2][2], -60.0);
        let j = plane_json(&p, 12, "Roof Planes");
        assert_eq!(j["kind"], "plane");
        assert!((j["pitch"].as_f64().unwrap() - 8.0).abs() < 1e-9);
        assert_eq!(j["polygon3d"].as_array().unwrap().len(), 4);
        assert_eq!(j["baseline"][1]["x"], 100.0);
    }

    #[test]
    fn overhang_below_the_baseline_hangs_lower() {
        // The baseline is 12" inside the south edge: that edge hangs below.
        let angle = std::f64::consts::FRAC_PI_4; // 12/12
        let pts = [(0.0, -12.0), (100.0, -12.0), (100.0, 60.0), (0.0, 60.0)];
        let obj = plane_obj(&pts, (0.0, 0.0, 1.0, 0.0, 100.0), 200.0, angle);
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(ROOF_PLANE, 0).next().unwrap();
        let p = decode_plane(&obj, &tree, i).unwrap();
        let poly = p.polygon3d();
        let south = poly.iter().find(|v| v[2] == 12.0).unwrap();
        assert!((south[1] - 188.0).abs() < 1e-9);
    }

    #[test]
    fn flat_roofs_are_exact_zero_and_denormal_residue_is_not_a_roof() {
        let pts = [(0.0, 0.0), (100.0, 0.0), (100.0, 60.0), (0.0, 60.0)];
        let flat = plane_obj(&pts, (0.0, 0.0, 1.0, 0.0, 100.0), 175.25, 0.0);
        let tree = ObjectTree::build(&flat);
        let i = tree.of_kind(ROOF_PLANE, 0).next().unwrap();
        let p = decode_plane(&flat, &tree, i).unwrap();
        assert_eq!(p.pitch(), 0.0);
        assert!(p.polygon3d().iter().all(|v| v[1] == 175.25));
        // An outline whose "pitch" bytes read 4.2e-314 and whose height is 0.
        let residue = plane_obj(&pts, (0.0, 0.0, 1.0, 0.0, 100.0), 0.0, 4.243991582e-314);
        let tree = ObjectTree::build(&residue);
        let i = tree.of_kind(ROOF_PLANE, 0).next().unwrap();
        assert!(decode_plane(&residue, &tree, i).is_none());
        // X17 stores the angle one byte earlier.
        let angle = (4.0f64 / 12.0).atan();
        let mut x17 = plane_obj(&pts, (0.0, 0.0, 1.0, 0.0, 100.0), 150.0, angle);
        let m = x17.iter().position(|&c| c == 0xCD).unwrap();
        let at = m + 671 + 350 * 4 + 200 + 76;
        let raw: [u8; 8] = x17[at..at + 8].try_into().unwrap();
        x17[at..at + 8].fill(0);
        x17[at - 1..at + 7].copy_from_slice(&raw);
        let tree = ObjectTree::build(&x17);
        let i = tree.of_kind(ROOF_PLANE, 0).next().unwrap();
        let p = decode_plane(&x17, &tree, i).unwrap();
        assert!((p.pitch() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_open_outlines_small_objects_and_duplicates() {
        let small = sized(ROOF_PLANE, 0, 3000, |_| {});
        let tree = ObjectTree::build(&small);
        let i = tree.of_kind(ROOF_PLANE, 0).next().unwrap();
        assert!(decode_plane(&small, &tree, i).is_none());
        let angle = 0.5f64.atan();
        let pts = [(0.0, 0.0), (100.0, 0.0), (100.0, 60.0), (0.0, 60.0)];
        let a_obj = plane_obj(&pts, (0.0, 0.0, 1.0, 0.0, 100.0), 100.0, angle);
        let b_obj = plane_obj(&pts, (0.0, 0.0, 1.0, 0.0, 100.0), 100.0, angle);
        let ta = ObjectTree::build(&a_obj);
        let tb = ObjectTree::build(&b_obj);
        let a = decode_plane(&a_obj, &ta, ta.of_kind(ROOF_PLANE, 0).next().unwrap()).unwrap();
        let b = decode_plane(&b_obj, &tb, tb.of_kind(ROOF_PLANE, 0).next().unwrap()).unwrap();
        assert!(same_outline(&a, &b));
        let c_obj = plane_obj(
            &[(0.0, 0.0), (50.0, 0.0), (50.0, 60.0), (0.0, 60.0)],
            (0.0, 0.0, 1.0, 0.0, 50.0),
            100.0,
            angle,
        );
        let tc = ObjectTree::build(&c_obj);
        let c = decode_plane(&c_obj, &tc, tc.of_kind(ROOF_PLANE, 0).next().unwrap()).unwrap();
        assert!(!same_outline(&a, &c));
    }
}
