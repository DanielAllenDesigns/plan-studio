//! Stair flights (class 46) and landings (class 47).
//!
//! A straight stair flight is a class 46 object directly on a floor; a U or L
//! shaped stair is several of them plus class 47 landings. Evidence: the first
//! floor of one job has a class 46 whose line starts at `(378.9, 1022.8)`,
//! runs `+y` for 149.19" and is 41.9" wide, and the room named `STAIR WELL`
//! has its centre at `x = 379.7` and a 43.5" x 236" box covering `y = 936 to
//! 1172`: the flight fills the north end of the room. The run divided by the
//! tread depth is a whole number in all 11 flights read (5, 7, 6, 3, 14, 2
//! treads), and the rise divided by the riser height is a whole number
//! (17.94 risers... exactly 18 for 137.875 / 7.6597).
//!
//! ```text
//! flight (class 46)
//! +671 (first line record)  x, y, dx, dy, length
//!        x, y     the middle of the bottom riser, plan inches
//!        dx, dy   the direction of travel
//!        length   the run: treads x tread depth
//! rise anchor R   (offset varies; 8,940 + 137 = 9,077 in the three jobs read)
//! R-137  f64      stair width
//! R      f64      floor-to-floor rise, inches
//! R+8    f64      tread depth
//! R+18   f64      riser height (rise / risers, e.g. 7.6597 = 137.875 / 18)
//!
//! landing (class 47)
//! first line record and its three followers: the four edges of a rectangle
//! ```
//!
//! The rise anchor is found by shape (a rise, a tread between 6" and 16", a riser
//! 4" to 10" that divides the rise into a whole number of risers). The landing
//! height is not stored plainly; the importer gives a landing half of the
//! rise of the flights on its floor (Low). Railings are not read: the stair
//! sides import as open.

use super::lines::{chain, find_edges, is_closed, polygon_of};
use super::tree::{f64_at, ObjectTree};
use serde_json::{json, Value};

/// Stair flight class id.
pub const FLIGHT: u8 = 46;
/// Landing class id.
pub const LANDING: u8 = 47;
/// Distance from the rise field back to the width field.
const WIDTH_BEFORE_RISE: usize = 137;
/// Where the search for the rise field starts, from the `CD` byte.
const RISE_SEARCH_FROM: usize = 0x400;

/// One decoded flight.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefFlight {
    pub node: usize,
    /// Middle of the bottom riser.
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    /// Run of the flight.
    pub run: f64,
    pub width: f64,
    pub tread: f64,
    /// Floor-to-floor rise of the whole stair.
    pub rise: f64,
    pub riser: f64,
}

impl ChiefFlight {
    /// Number of treads in this flight.
    pub fn treads(&self) -> usize {
        (self.run / self.tread).round().max(1.0) as usize
    }
}

/// Decodes the class 46 object at tree index `node`.
pub fn decode_flight(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefFlight> {
    let n = tree.node(node);
    let edges = find_edges(
        bytes,
        n.marker,
        n.marker + 0x100,
        n.end.min(n.marker + 0x400),
    );
    let line = edges.first()?;
    if line.len < 1.0 {
        return None;
    }
    let hi = n.len().saturating_sub(40);
    let r = (RISE_SEARCH_FROM..hi).find(|&o| {
        let v = |k: usize| f64_at(bytes, n.marker + o + k).filter(|v| v.is_finite());
        let (Some(rise), Some(tread), Some(riser)) = (v(0), v(8), v(18)) else {
            return false;
        };
        (6.0..=16.0).contains(&tread)
            && (4.0..=10.0).contains(&riser)
            && (10.0..=400.0).contains(&rise)
            && {
                let k = rise / riser;
                (k - k.round()).abs() < 0.02 && k.round() >= 2.0
            }
    })?;
    let at = |k: isize| f64_at(bytes, (n.marker + r).wrapping_add_signed(k));
    let rise = at(0)?;
    let tread = at(8)?;
    let riser = at(18)?;
    let width = r
        .checked_sub(WIDTH_BEFORE_RISE)
        .and_then(|w| f64_at(bytes, n.marker + w))
        .filter(|w| (20.0..=140.0).contains(w))
        .unwrap_or(36.0);
    let runs = line.len / tread;
    if (runs - runs.round()).abs() > 0.06 {
        return None;
    }
    Some(ChiefFlight {
        node,
        x: line.x,
        y: line.y,
        dx: line.dx,
        dy: line.dy,
        run: line.len,
        width,
        tread,
        rise,
        riser,
    })
}

/// The `Floor.stairs` entry of a flight, in the serialized shape of
/// `plan_stairs::Stair` with a straight `StairParams`. `floor_elevation` is the
/// elevation of the floor the flight is on.
pub fn flight_json(f: &ChiefFlight, id: u64, floor_elevation: f64) -> Value {
    // Plan Studio's origin is the bottom riser's left corner (facing the travel).
    let (ox, oy) = (f.x - f.dy * f.width / 2.0, f.y + f.dx * f.width / 2.0);
    let risers = f.treads() + 1;
    json!({
        "id": id,
        "origin": {"x": ox, "y": oy},
        "direction": f.dy.atan2(f.dx),
        "params": {
            "total_rise": risers as f64 * f.riser,
            "width": f.width,
            "tread_depth": f.tread,
            "riser_height_target": f.riser,
            "shape": "Straight",
        },
        "floor_elevation": floor_elevation,
        "base": 0.0,
    })
}

/// One decoded landing: a closed outline.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefLanding {
    pub node: usize,
    pub outline: Vec<(f64, f64)>,
}

/// Decodes the class 47 object at tree index `node`.
pub fn decode_landing(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefLanding> {
    let n = tree.node(node);
    let edges = find_edges(
        bytes,
        n.marker,
        n.marker + 0x100,
        n.end.min(n.marker + 0x600),
    );
    if edges.is_empty() {
        return None;
    }
    let c = chain(&edges, 0);
    if !is_closed(&c) {
        return None;
    }
    let mut outline = polygon_of(&c);
    if outline.len() < 3 {
        return None;
    }
    if super::lines::signed_area(&outline) < 0.0 {
        outline.reverse();
    }
    Some(ChiefLanding { node, outline })
}

/// The `Floor.stairs` entry of a landing: a `Landing` shape over its outline,
/// with its top `rise` above the floor.
pub fn landing_json(l: &ChiefLanding, id: u64, floor_elevation: f64, rise: f64) -> Value {
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for &(x, y) in &l.outline {
        lo = (lo.0.min(x), lo.1.min(y));
        hi = (hi.0.max(x), hi.1.max(y));
    }
    let (w, d) = (hi.0 - lo.0, hi.1 - lo.1);
    let outline: Vec<Value> = l
        .outline
        .iter()
        .map(|&(x, y)| json!({"x": x, "y": y}))
        .collect();
    json!({
        "id": id,
        "origin": {"x": lo.0, "y": lo.1},
        "direction": 0.0,
        "params": {
            "total_rise": rise,
            "width": w.min(d),
            "shape": {"Landing": {"depth": w.max(d)}},
            "outline": outline,
        },
        "floor_elevation": floor_elevation,
        "base": 0.0,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::lines::tests::{put_edge, put_rect};
    use crate::import::tree::testutil::*;

    /// A flight object: the line record at 671 and the spec run at 9077-137.
    pub fn flight_obj(
        line: (f64, f64, f64, f64, f64),
        width: f64,
        tread: f64,
        rise: f64,
        riser: f64,
    ) -> Vec<u8> {
        sized(FLIGHT, 0, 10000, |b| {
            put_edge(b, 671, line);
            put_f64(b, 8940, width);
            put_f64(b, 9077, rise);
            put_f64(b, 9085, tread);
            put_f64(b, 9095, riser);
        })
    }

    #[test]
    fn decodes_a_flight_and_builds_the_stair() {
        // 6 treads of 10.5" going +y, 48" wide, 18 risers of 7.65972".
        let obj = flight_obj(
            (591.0, 234.93, 0.0, 1.0, 63.0),
            48.0,
            10.5,
            137.875,
            137.875 / 18.0,
        );
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(FLIGHT, 0).next().unwrap();
        let f = decode_flight(&obj, &tree, i).unwrap();
        assert_eq!(
            (f.x, f.y, f.run, f.width, f.tread),
            (591.0, 234.93, 63.0, 48.0, 10.5)
        );
        assert_eq!(f.treads(), 6);
        let j = flight_json(&f, 9, 0.0);
        // Facing +y, the left corner is at -x of the middle.
        assert!((j["origin"]["x"].as_f64().unwrap() - (591.0 - 24.0)).abs() < 1e-9);
        assert!((j["origin"]["y"].as_f64().unwrap() - 234.93).abs() < 1e-9);
        assert!((j["direction"].as_f64().unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(j["params"]["shape"], "Straight");
        assert!((j["params"]["total_rise"].as_f64().unwrap() - 7.0 * 137.875 / 18.0).abs() < 1e-9);
        assert_eq!(j["params"]["tread_depth"], 10.5);
    }

    #[test]
    fn rejects_runs_that_are_not_whole_treads_and_missing_specs() {
        let obj = flight_obj((0.0, 0.0, 1.0, 0.0, 64.0), 42.0, 10.5, 126.75, 7.5);
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(FLIGHT, 0).next().unwrap();
        assert!(decode_flight(&obj, &tree, i).is_none());
        let blank = sized(FLIGHT, 0, 10000, |b| {
            put_edge(b, 671, (0.0, 0.0, 1.0, 0.0, 63.0))
        });
        let tree = ObjectTree::build(&blank);
        let i = tree.of_kind(FLIGHT, 0).next().unwrap();
        assert!(decode_flight(&blank, &tree, i).is_none());
    }

    #[test]
    fn landing_outline_becomes_a_landing_stair() {
        let obj = sized(LANDING, 0, 2000, |b| {
            put_rect(b, 671, 644.49, 194.52, 45.84, 48.0)
        });
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(LANDING, 0).next().unwrap();
        let l = decode_landing(&obj, &tree, i).unwrap();
        assert_eq!(l.outline.len(), 4);
        let j = landing_json(&l, 3, -125.875, 63.0);
        assert_eq!(j["params"]["shape"], json!({"Landing": {"depth": 48.0}}));
        assert!((j["params"]["width"].as_f64().unwrap() - 45.84).abs() < 1e-9);
        assert_eq!(j["params"]["outline"].as_array().unwrap().len(), 4);
        assert_eq!(j["floor_elevation"], -125.875);
        let open = sized(LANDING, 0, 2000, |b| {
            put_edge(b, 671, (0.0, 0.0, 1.0, 0.0, 10.0))
        });
        let tree = ObjectTree::build(&open);
        let i = tree.of_kind(LANDING, 0).next().unwrap();
        assert!(decode_landing(&open, &tree, i).is_none());
    }
}
