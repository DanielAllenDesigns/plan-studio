//! Placing roof planes against one another and entering their edges
//! (manual pp. 836, 839, 841, 843, 844; RF-94, RF-107, RF-108, RF-111,
//! RF-113, RF-114, RF-165; Round 16 brief 18b). Pure arithmetic on
//! [`RoofPlane`]s and plan points; the editor owns the commands.
//!
//! Lengths are inches, pitch is rise per 12 of run.

use crate::RoofPlane;
use plan_core::Point;
use std::f64::consts::{FRAC_PI_2, PI};

/// Two baselines count as parallel when the sine of the angle between them
/// is below this.
const PARALLEL_SIN: f64 = 0.02;
/// Pitches closer than this (rise per 12) are the same.
const SAME_PITCH: f64 = 0.01;
/// A plane edge snaps to a wall surface this close at most, inches.
pub const WALL_SNAP_DISTANCE: f64 = 12.0;
/// ... when the two lines are within this angle (radians, about 3 degrees).
pub const WALL_SNAP_ANGLE: f64 = 0.05;

/// Why two planes cannot be placed against each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementError {
    /// The baselines are not parallel.
    NotParallel,
    /// The planes rise toward opposite sides.
    FacingApart,
    /// The pitches differ, so no move makes them one plane.
    DifferentPitch,
    /// A vertical or empty plane has no height.
    Degenerate,
}

impl std::fmt::Display for PlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PlacementError::NotParallel => "the baselines are not parallel",
            PlacementError::FacingApart => "the planes rise toward opposite sides",
            PlacementError::DifferentPitch => "the planes have different pitches",
            PlacementError::Degenerate => "a plane has no slope to match",
        })
    }
}

/// The unit plan direction in which `plane` rises (square to its baseline,
/// toward the rest of the outline).
pub fn up_slope(plane: &RoofPlane) -> Point {
    let (a, b) = plane.baseline;
    let d = b.sub(a);
    if d.length() < 1e-9 {
        return Point::new(0.0, 1.0);
    }
    let n = d.normalized().perp();
    let poly = plane.plan_polygon();
    let c = if poly.is_empty() {
        a
    } else {
        let s = poly.iter().fold(Point::ZERO, |s, p| s.add(*p));
        s.scale(1.0 / poly.len() as f64)
    };
    if c.sub(a).dot(n) < 0.0 {
        n.scale(-1.0)
    } else {
        n
    }
}

/// How far `moving` must be raised (negative: lowered) for its baseline to
/// lie in the plane of `target` (Move to be Coplanar, manual p. 844). The
/// baselines must be parallel, both planes rise toward the same side and
/// their pitches must match; the move is vertical.
pub fn coplanar_shift(moving: &RoofPlane, target: &RoofPlane) -> Result<f64, PlacementError> {
    let (a0, a1) = moving.baseline;
    let (b0, b1) = target.baseline;
    let (da, db) = (a1.sub(a0), b1.sub(b0));
    if da.length() < 1e-9 || db.length() < 1e-9 {
        return Err(PlacementError::Degenerate);
    }
    if da.normalized().cross(db.normalized()).abs() > PARALLEL_SIN {
        return Err(PlacementError::NotParallel);
    }
    if up_slope(moving).dot(up_slope(target)) <= 0.0 {
        return Err(PlacementError::FacingApart);
    }
    if (moving.pitch_in_12 - target.pitch_in_12).abs() > SAME_PITCH {
        return Err(PlacementError::DifferentPitch);
    }
    let here = moving.polygon3d.first().ok_or(PlacementError::Degenerate)?;
    let want = target.height_at(a0).ok_or(PlacementError::Degenerate)?;
    Ok(want - here[1])
}

/// Where the line through edge `edge` of `plane` (vertex `edge` to the next)
/// meets the plane of `other`: `[x, elevation, -plan y]`, anywhere along the
/// extended line (Place Roof Plane Intersection Point, manual p. 843).
/// `None` when the line runs parallel to the other plane.
pub fn edge_plane_point(plane: &RoofPlane, edge: usize, other: &RoofPlane) -> Option<[f64; 3]> {
    let n = plane.polygon3d.len();
    if n < 2 || edge >= n {
        return None;
    }
    let p0 = plane.polygon3d[edge];
    let p1 = plane.polygon3d[(edge + 1) % n];
    let nm = other.normal();
    let o = *other.polygon3d.first()?;
    let dir = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let denom = nm[0] * dir[0] + nm[1] * dir[1] + nm[2] * dir[2];
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = (nm[0] * (o[0] - p0[0]) + nm[1] * (o[1] - p0[1]) + nm[2] * (o[2] - p0[2])) / denom;
    Some([p0[0] + t * dir[0], p0[1] + t * dir[1], p0[2] + t * dir[2]])
}

/// Which baseline height a new plane drawn over an existing one takes (Set
/// Baseline Height, manual p. 844).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BaselineOver {
    /// The top of the wall below: a full-height dormer.
    #[default]
    WallTop,
    /// The surface of the existing roof plane at the baseline start: a dormer
    /// vent or a cricket.
    ExistingPlane,
}

/// The elevation a new baseline starting at `start` takes: the wall top, or
/// the height of `under` there (falling back to the wall top when `under`
/// has no height at that point).
pub fn baseline_height_over(
    choice: BaselineOver,
    wall_top: f64,
    under: &RoofPlane,
    start: Point,
) -> f64 {
    match choice {
        BaselineOver::WallTop => wall_top,
        BaselineOver::ExistingPlane => under.height_at(start).unwrap_or(wall_top),
    }
}

/// Is plan point `p` inside `plane`'s outline and the plane not flat? A new
/// baseline starting here lies on the plane (manual p. 836).
pub fn baseline_lies_on(plane: &RoofPlane, p: Point) -> bool {
    plane_contains(&plane.plan_polygon(), p)
}

fn plane_contains(poly: &[Point], p: Point) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ----- edge entry: projected vs actual (RF-97) -----

/// How a typed edge length is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LengthEntry {
    /// Measured in plan.
    #[default]
    Projected,
    /// Measured along the slope of the edge.
    Actual,
}

impl LengthEntry {
    pub fn label(self) -> &'static str {
        match self {
            LengthEntry::Projected => "Projected",
            LengthEntry::Actual => "Actual (along the slope)",
        }
    }
}

/// Plan (projected) and true (actual) length of the edge `a -> b`.
pub fn edge_length(a: [f64; 3], b: [f64; 3]) -> (f64, f64) {
    let plan = ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
    let rise = b[1] - a[1];
    (plan, (plan * plan + rise * rise).sqrt())
}

/// Plan length of the edge `a -> b` once its length is typed as `typed` under
/// `entry`; the edge keeps its slope.
pub fn plan_length_for_entry(a: [f64; 3], b: [f64; 3], typed: f64, entry: LengthEntry) -> f64 {
    let (plan, actual) = edge_length(a, b);
    match entry {
        LengthEntry::Projected => typed,
        LengthEntry::Actual if actual > 1e-9 => typed * plan / actual,
        LengthEntry::Actual => typed,
    }
}

/// Perimeter of `polygon3d`, projected and actual.
pub fn perimeter(polygon3d: &[[f64; 3]]) -> (f64, f64) {
    let n = polygon3d.len();
    (0..n).fold((0.0, 0.0), |(p, a), i| {
        let (ep, ea) = edge_length(polygon3d[i], polygon3d[(i + 1) % n]);
        (p + ep, a + ea)
    })
}

// ----- In From Baseline (RF-165) -----

/// The height above the floor at which a second pitch starts when it begins
/// `in_from` inches (in plan) from the baseline of a plane of `pitch` that
/// leaves the top of a wall `wall_height` high.
pub fn start_height_for_in_from_baseline(wall_height: f64, pitch: f64, in_from: f64) -> f64 {
    wall_height + in_from * pitch / 12.0
}

/// The distance in from the baseline (plan) at which a plane of `pitch`
/// reaches `start_height`; zero at or below the wall top, or for a flat plane.
pub fn in_from_baseline_for_start_height(wall_height: f64, pitch: f64, start_height: f64) -> f64 {
    if pitch.abs() < 1e-9 {
        return 0.0;
    }
    ((start_height - wall_height) * 12.0 / pitch).max(0.0)
}

// ----- parallel / perpendicular and wall snapping (RF-107, RF-108) -----

/// The turn (radians, within a quarter turn either way) that makes direction
/// `edge` parallel to `reference`, or square to it with `perpendicular`.
pub fn turn_to_align(edge: Point, reference: Point, perpendicular: bool) -> f64 {
    let (e, r) = (edge.angle(), reference.angle());
    let mut want = r - e + if perpendicular { FRAC_PI_2 } else { 0.0 };
    // Fold into (-pi/2, pi/2]: an edge has no head or tail.
    want = (want + FRAC_PI_2).rem_euclid(PI) - FRAC_PI_2;
    if want <= -FRAC_PI_2 + 1e-12 {
        want += PI;
    }
    want
}

/// A wall's outside surface line for snapping: a point on it and its unit
/// direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallSurface {
    pub a: Point,
    pub b: Point,
}

/// Snaps the baseline `a -> b` onto the nearest wall surface that is nearly
/// parallel and within [`WALL_SNAP_DISTANCE`] (plane edges snap to the outside
/// of a parallel wall, manual p. 841). Both ends land on the surface line,
/// keeping their positions along it. Returns the baseline unchanged when no
/// surface qualifies.
pub fn snap_to_wall_surface(a: Point, b: Point, walls: &[WallSurface]) -> (Point, Point) {
    let d = b.sub(a);
    if d.length() < 1e-9 {
        return (a, b);
    }
    let dir = d.normalized();
    let mid = Point::lerp(a, b, 0.5);
    let mut best: Option<(f64, WallSurface)> = None;
    for w in walls {
        let wd = w.b.sub(w.a);
        if wd.length() < 1e-9 {
            continue;
        }
        let u = wd.normalized();
        if u.cross(dir).abs() > WALL_SNAP_ANGLE.sin() {
            continue;
        }
        let dist = mid.sub(w.a).dot(u.perp()).abs();
        if dist > WALL_SNAP_DISTANCE {
            continue;
        }
        if best.is_none_or(|(d0, _)| dist < d0) {
            best = Some((dist, *w));
        }
    }
    let Some((_, w)) = best else {
        return (a, b);
    };
    let u = w.b.sub(w.a).normalized();
    let onto = |p: Point| {
        let off = p.sub(w.a).dot(u.perp());
        p.sub(u.perp().scale(off))
    };
    (onto(a), onto(b))
}
