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
//! # Per-edge flags and overhangs (stage 3)
//!
//! Each outline record carries, counted from its `x` field,
//!
//! ```text
//! +46  3 bytes  join: FF FF FF = a free edge; n 00 00 = the edge is joined to
//!                another roof plane (n is a small index, 0 to 4)
//! +49  1 byte   0/1: the edge overhangs the wall (an eave or rake with a
//!                projection; 0 = the edge stands on the wall)
//! +51  1 byte   1 on every joined edge (and on a few free ones)
//! ```
//!
//! Evidence, 181 planes of 10 saved files, 1,073 edges of 6" or more: 343 of the
//! 347 joined edges coincide with an edge of another plane of the floor in
//! plan, and 642 of the 726 free ones have no neighbour, so a joined edge is a
//! ridge, hip or valley and a free edge not parallel to the baseline is a
//! gable end (a rake). The overhang byte separates edges that stand on a wall
//! from edges that project past it: of 289 free edges with the byte at 0, 217
//! lie within 4 inches of a wall line; of 386 with the byte at 1, 290 are 4
//! inches or more off it. Confidence: Medium for both.
//!
//! The overhang size is not stored as a number (no `f64` of the plane equals
//! it): it is the distance from the baseline to the outline's eave edge, which
//! is where the projection ends. The planes whose eave edge lies past the
//! baseline cluster at 12" (30 planes), 16" (19), 15" (10), then 9, 18 and
//! 12.5 (Medium). The baseline lies on the wall (within 3" of a wall line in
//! most planes) and the outline already includes the overhang.
//!
//! The outline is rotated so that its first edge is the eave edge, which is
//! what Plan Studio's `plan_roof::classify_edges` takes as the eave
//! (`polygon3d[0] -> [1]`); without that the fascia, soffit and gutters of an
//! imported plane would run along an arbitrary edge.
//!
//! Not decoded: the fascia sizes, holes and skylights, the plane material.
//! Planes with a duplicate outline are dropped and counted. Other class 50
//! objects have a chain of line records but no baseline record (the bytes where the pitch
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

/// Offsets of the per-edge flags, from the `x` field of an outline record.
const EDGE_JOIN: usize = 46;
const EDGE_OVERHANG: usize = 49;
const EDGE_JOIN_MARK: usize = 51;

/// What an outline edge of a plane is, in `plan_roof::EdgeRole` terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeRole {
    /// The baseline edge: the first edge of the outline.
    Eave,
    /// Parallel to the baseline and joined to another plane.
    Ridge,
    /// Not parallel to the baseline and joined to another plane (hip or
    /// valley).
    HipOrValley,
    /// A sloped edge no plane shares: the gable end.
    Rake,
    /// A level edge nothing shares (the high edge of a shed plane).
    Top,
}

impl EdgeRole {
    /// The word stored in the plane record.
    pub fn name(self) -> &'static str {
        match self {
            EdgeRole::Eave => "eave",
            EdgeRole::Ridge => "ridge",
            EdgeRole::HipOrValley => "hip_or_valley",
            EdgeRole::Rake => "rake",
            EdgeRole::Top => "top",
        }
    }
}

/// One edge of a plane's outline with the flags of its record.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefRoofEdge {
    pub start: (f64, f64),
    pub end: (f64, f64),
    /// Index of the neighbour the edge is joined to (`None`: a free edge).
    pub link: Option<u32>,
    /// The edge projects past the wall (the overhang byte).
    pub overhangs: bool,
    pub role: EdgeRole,
}

/// One decoded roof plane.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefRoofPlane {
    pub node: usize,
    /// Closed outline in plan, counter-clockwise, starting with the eave
    /// edge (`outline[0] -> outline[1]`).
    pub outline: Vec<(f64, f64)>,
    /// The baseline segment.
    pub baseline: ((f64, f64), (f64, f64)),
    /// Height of the baseline above the first-floor datum, inches.
    pub base_height: f64,
    /// Pitch angle, radians.
    pub angle: f64,
    /// `edges[k]` runs from `outline[k]` to `outline[k + 1]`.
    pub edges: Vec<ChiefRoofEdge>,
    /// Horizontal projection of the eave past the baseline, inches (0 when
    /// the eave edge is on the baseline).
    pub overhang: f64,
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
    let baseline_seg = (baseline.start(), baseline.end());
    let (outline, edges, overhang) =
        eave_first(bytes, n.marker, &outline, &outline_chain, baseline_seg);
    Some(ChiefRoofPlane {
        node,
        outline,
        baseline: baseline_seg,
        base_height,
        angle,
        edges,
        overhang,
    })
}

/// Reads the join and overhang bytes of the outline record `e`: the join
/// index (`None` for a free edge) and whether the edge overhangs. `None` when
/// the bytes are not in the known pattern (an unknown layout).
fn edge_flags(bytes: &[u8], base: usize, e: &Edge) -> Option<(Option<u32>, bool)> {
    let at = base + e.offset;
    let join = bytes.get(at + EDGE_JOIN..at + EDGE_JOIN + 3)?;
    // A joined edge also has the byte at +51 set; a record of zeros (an
    // unknown layout) is not read as joined.
    let link = match join {
        [0xFF, 0xFF, 0xFF] => None,
        [n, 0, 0] if *n < 64 && *bytes.get(at + EDGE_JOIN_MARK)? == 1 => Some(u32::from(*n)),
        _ => return None,
    };
    let over = *bytes.get(at + EDGE_OVERHANG)?;
    (over <= 1).then_some((link, over == 1))
}

/// Distance from `p` to the segment `a -> b`.
fn dist_seg(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// Rotates the counter-clockwise `outline` so its first edge is the eave
/// (the edge parallel to the baseline that lies furthest out), gives every
/// edge its flags and role, and measures the overhang. Returns the rotated
/// outline, the edges and the overhang in inches.
fn eave_first(
    bytes: &[u8],
    base: usize,
    outline: &[(f64, f64)],
    chain: &[Edge],
    baseline: ((f64, f64), (f64, f64)),
) -> (Vec<(f64, f64)>, Vec<ChiefRoofEdge>, f64) {
    let n = outline.len();
    let (a, b) = baseline;
    let (bx, by) = (b.0 - a.0, b.1 - a.1);
    let blen = bx.hypot(by).max(1e-9);
    let (ux, uy) = (bx / blen, by / blen);
    // The inward normal of the baseline points at the outline's centroid.
    let (mut nx, mut ny) = (-uy, ux);
    let (cx, cy) = outline.iter().fold((0.0, 0.0), |s, p| {
        (s.0 + p.0 / n as f64, s.1 + p.1 / n as f64)
    });
    if (cx - a.0) * nx + (cy - a.1) * ny < 0.0 {
        nx = -nx;
        ny = -ny;
    }
    let parallel = |p: (f64, f64), q: (f64, f64)| {
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        let l = dx.hypot(dy);
        l >= 1.0 && (dx * uy - dy * ux).abs() / l < 1e-3
    };
    // Eave candidates: parallel to the baseline; the one furthest out (least
    // distance along the inward normal), the longest on a tie.
    let mut eave: Option<(usize, f64, f64)> = None;
    for k in 0..n {
        let (p, q) = (outline[k], outline[(k + 1) % n]);
        if !parallel(p, q) {
            continue;
        }
        let d = ((p.0 + q.0) / 2.0 - a.0) * nx + ((p.1 + q.1) / 2.0 - a.1) * ny;
        let len = (q.0 - p.0).hypot(q.1 - p.1);
        let better = match eave {
            None => true,
            Some((_, bd, bl)) => d < bd - 0.01 || ((d - bd).abs() <= 0.01 && len > bl),
        };
        if better {
            eave = Some((k, d, len));
        }
    }
    let (first, dist) = eave.map_or((0, 0.0), |(k, d, _)| (k, d));
    let overhang = if (-120.0..-0.5).contains(&dist) {
        -dist
    } else {
        0.0
    };
    let rotated: Vec<(f64, f64)> = (0..n).map(|k| outline[(first + k) % n]).collect();
    let mut edges = Vec::with_capacity(n);
    for k in 0..n {
        let (p, q) = (rotated[k], rotated[(k + 1) % n]);
        let mid = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
        // The record chain edge that covers this outline edge.
        let rec = chain
            .iter()
            .filter(|e| e.len > 0.01)
            .filter(|e| dist_seg(mid, e.start(), e.end()) < 0.25)
            .max_by(|x, y| x.len.total_cmp(&y.len));
        let flags = rec.and_then(|e| edge_flags(bytes, base, e));
        let (link, overhangs) = flags.unwrap_or((None, false));
        let par = parallel(p, q);
        let role = if k == 0 {
            EdgeRole::Eave
        } else {
            match (par, link.is_some()) {
                (true, true) => EdgeRole::Ridge,
                (true, false) => EdgeRole::Top,
                (false, true) => EdgeRole::HipOrValley,
                (false, false) => EdgeRole::Rake,
            }
        };
        edges.push(ChiefRoofEdge {
            start: p,
            end: q,
            link,
            overhangs,
            role,
        });
    }
    (rotated, edges, overhang)
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
        "overhang": p.overhang,
        "label": "",
        "material": "Asphalt Shingles",
        "layer": layer,
        "ridge_caps": false,
        "gutters": false,
        "chief_edges": p.edges.iter().map(|e| json!({
            "start": {"x": e.start.0, "y": e.start.1},
            "end": {"x": e.end.0, "y": e.end.1},
            "role": e.role.name(),
            "joined": e.link.is_some(),
            "overhangs": e.overhangs,
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::lines::tests::put_edge;
    use crate::import::tree::testutil::*;

    /// Per-edge flags of a test plane: the join index (`None` = free) and the
    /// overhang byte.
    pub type Flags = (Option<u8>, bool);

    /// A plane: closed outline of `pts` (edge records 350 bytes apart from
    /// 671), a baseline record, then a child class 50 object. Every edge is
    /// free and stands on the wall.
    pub fn plane_obj(
        pts: &[(f64, f64)],
        baseline: (f64, f64, f64, f64, f64),
        height: f64,
        angle: f64,
    ) -> Vec<u8> {
        plane_obj_flags(
            pts,
            baseline,
            height,
            angle,
            &vec![(None, false); pts.len()],
        )
    }

    /// [`plane_obj`] with the join and overhang flags of each edge.
    pub fn plane_obj_flags(
        pts: &[(f64, f64)],
        baseline: (f64, f64, f64, f64, f64),
        height: f64,
        angle: f64,
        flags: &[Flags],
    ) -> Vec<u8> {
        let child = sized(ROOF_PLANE, 0, 400, |_| {});
        let total = 9000 + child.len();
        sized(ROOF_PLANE, 0, total, |b| {
            let n = pts.len();
            for k in 0..n {
                let (p, q) = (pts[k], pts[(k + 1) % n]);
                let len = (q.0 - p.0).hypot(q.1 - p.1);
                let at = 671 + 350 * k;
                put_edge(b, at, (p.0, p.1, (q.0 - p.0) / len, (q.1 - p.1) / len, len));
                match flags[k].0 {
                    None => b[at + EDGE_JOIN..at + EDGE_JOIN + 3].copy_from_slice(&[0xFF; 3]),
                    Some(i) => {
                        b[at + EDGE_JOIN] = i;
                        b[at + EDGE_JOIN_MARK] = 1;
                    }
                }
                b[at + EDGE_OVERHANG] = u8::from(flags[k].1);
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

    #[test]
    fn the_eave_edge_comes_first_with_flags_and_overhang() {
        // A hip-like plane: outline listed starting at a side edge. The
        // baseline y = 0 is 12" inside the south edge (y = -12), so the eave
        // projects 12"; the ridge (north, y = 60) and one side are joined; the
        // other side is a gable end; the eave and the gable overhang.
        let angle = (8.0f64 / 12.0).atan();
        let pts = [
            (100.0, -12.0), // east side starts here: edge 0 = east side (gable)
            (100.0, 60.0),
            (0.0, 60.0), // ridge
            (0.0, -12.0), // west side (joined)
                         // closes with the south edge back to (100, -12)
        ];
        let flags = [
            (None, true),     // east side: free, overhangs (a rake)
            (Some(2), false), // ridge: joined
            (Some(1), false), // west side: joined
            (None, true),     // south edge: the eave, overhangs
        ];
        let obj = plane_obj_flags(&pts, (0.0, 0.0, 1.0, 0.0, 100.0), 129.84, angle, &flags);
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(ROOF_PLANE, 0).next().unwrap();
        let p = decode_plane(&obj, &tree, i).unwrap();
        // The eave (south) edge is first: outline[0] -> outline[1] runs along y = -12.
        assert_eq!(p.outline.len(), 4);
        assert!((p.outline[0].1 + 12.0).abs() < 1e-9 && (p.outline[1].1 + 12.0).abs() < 1e-9);
        assert!((p.overhang - 12.0).abs() < 1e-9, "{}", p.overhang);
        assert_eq!(p.edges.len(), 4);
        assert_eq!(p.edges[0].role, EdgeRole::Eave);
        assert!(p.edges[0].overhangs && p.edges[0].link.is_none());
        let roles: Vec<EdgeRole> = p.edges.iter().map(|e| e.role).collect();
        // Counter-clockwise from the eave: east side (rake), ridge, west side (joined).
        assert_eq!(
            roles,
            vec![
                EdgeRole::Eave,
                EdgeRole::Rake,
                EdgeRole::Ridge,
                EdgeRole::HipOrValley
            ]
        );
        assert_eq!(p.edges[2].link, Some(2));
        assert!(p.edges[1].overhangs);
        // The vertex heights are unchanged by the rotation: the overhang hangs below.
        let poly = p.polygon3d();
        assert!(poly[0][1] < 129.84 && (poly[0][1] - poly[1][1]).abs() < 1e-9);
        assert!(poly[2][1] > 129.84 + 39.0);
        let j = plane_json(&p, 3, "Roof Planes");
        assert_eq!(j["overhang"], 12.0);
        assert_eq!(j["chief_edges"][0]["role"], "eave");
        assert_eq!(j["chief_edges"][1]["role"], "rake");
        assert_eq!(j["chief_edges"][2]["joined"], true);
    }

    #[test]
    fn a_zero_record_has_no_joined_edges_and_no_overhang_without_the_baseline_gap() {
        // The baseline on the eave edge: no overhang. A record whose flag
        // bytes are all zero (an unknown layout) is never read as joined.
        let angle = (4.0f64 / 12.0).atan();
        let pts = [(0.0, 0.0), (100.0, 0.0), (100.0, 60.0), (0.0, 60.0)];
        let mut obj = plane_obj(&pts, (0.0, 0.0, 1.0, 0.0, 100.0), 100.0, angle);
        let m = obj.iter().position(|&c| c == 0xCD).unwrap();
        for k in 0..4 {
            let at = m + 671 + 350 * k;
            obj[at + EDGE_JOIN..at + EDGE_JOIN + 3].fill(0);
        }
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(ROOF_PLANE, 0).next().unwrap();
        let p = decode_plane(&obj, &tree, i).unwrap();
        assert_eq!(p.overhang, 0.0);
        assert!(p.edges.iter().all(|e| e.link.is_none() && !e.overhangs));
        assert_eq!(p.edges[0].role, EdgeRole::Eave);
        // The two sides are not parallel to the baseline and free: gable ends;
        // the far edge is a level free edge.
        assert_eq!(p.edges[1].role, EdgeRole::Rake);
        assert_eq!(p.edges[2].role, EdgeRole::Top);
    }
}
