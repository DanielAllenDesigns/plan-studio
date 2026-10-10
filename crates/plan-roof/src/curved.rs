//! Curved (barrel) roof planes (manual pp. 847, 854 to 857; RF-61).
//!
//! Any roof plane can be made a curved plane: its section across the slope
//! (the eave to the ridge) becomes a circular arc instead of a straight line.
//! The plane keeps its eave and its highest edge exactly where they were, so
//! a curved plane still tiles the plan; only the surface between them bows.
//!
//! # The three numbers
//!
//! Along the slope the arc leaves the eave at the *Angle at Eave* above
//! horizontal and arrives at the ridge at the *Angle at Ridge*. The chord
//! from the eave to the ridge slopes at the plane's pitch, which is the mean
//! of the two angles: a plane of pitch 0 with 45 degrees at the eave has
//! -45 degrees at the ridge (a barrel), and a 12 in 12 plane (45 degrees) whose
//! ridge angle is 1 degree has 89 degrees at the eave (nearly vertical at the
//! eave, nearly flat at the peak). The arc turns by the difference of the two
//! angles, and its *Radius to Roof Surface* follows from that turn and the
//! chord. The three are tied: change one and the others follow
//! ([`CurvedSpec::with_eave_angle`], [`with_ridge_angle`], [`with_radius`]);
//! the pitch stays.
//!
//! # Facets
//!
//! A renderer draws the arc as flat facets. [`curved_facets`] cuts the plane
//! into strips parallel to the eave, one per step of the *facet angle* (7.5
//! degrees unless set; it must divide 360 evenly, so the nearest value that
//! does is used). Each strip is a planar [`RoofPlane`] whose vertices lie on
//! the arc.
//!
//! [`with_ridge_angle`]: CurvedSpec::with_ridge_angle
//! [`with_radius`]: CurvedSpec::with_radius

use crate::geom::{self, V3};
use crate::RoofPlane;
use plan_core::geometry::polygon_area;
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// Facet Angle a curved plane starts with, degrees (manual p. 847).
pub const DEFAULT_FACET_ANGLE: f64 = 7.5;
/// Angles are kept inside this magnitude so the surface stays a function of
/// the run (a plane cannot overhang itself), degrees.
const MAX_ANGLE: f64 = 89.0;
/// Fewest and most facets of one plane.
const MAX_FACETS: usize = 720;

/// The curve of one roof plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CurvedSpec {
    /// Angle of the surface at the eave, degrees above horizontal.
    pub angle_at_eave: f64,
    /// Angle of the surface at the ridge edge, degrees above horizontal.
    pub angle_at_ridge: f64,
    /// Automatic Facet Angle: [`DEFAULT_FACET_ANGLE`].
    pub auto_facet: bool,
    /// The Facet Angle used when [`auto_facet`](Self::auto_facet) is off,
    /// degrees.
    pub facet_angle: f64,
}

impl Default for CurvedSpec {
    fn default() -> Self {
        Self {
            angle_at_eave: 0.0,
            angle_at_ridge: 0.0,
            auto_facet: true,
            facet_angle: DEFAULT_FACET_ANGLE,
        }
    }
}

/// What Join Curved Roof Plane keeps when the ridge edge of a curved plane
/// moves to meet another plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinLock {
    /// Lock the Radius to Roof Surface: the curvature stays and the angle at
    /// the ridge changes.
    Radius,
    /// Lock the Angle at Ridge: the angle at the ridge stays and the
    /// curvature changes.
    AngleAtRidge,
}

fn pitch_deg(rise_in_12: f64) -> f64 {
    (rise_in_12 / 12.0).atan().to_degrees()
}

fn limit(angle: f64) -> f64 {
    angle.clamp(-MAX_ANGLE, MAX_ANGLE)
}

impl CurvedSpec {
    /// A curve that is not curved yet: both angles are the plane's pitch.
    pub fn straight(pitch_in_12: f64) -> Self {
        let a = pitch_deg(pitch_in_12);
        Self {
            angle_at_eave: a,
            angle_at_ridge: a,
            ..Self::default()
        }
    }

    /// Slope of the chord from eave to ridge, degrees: the plane's pitch.
    pub fn chord_angle(&self) -> f64 {
        (self.angle_at_eave + self.angle_at_ridge) * 0.5
    }

    /// Turn of the surface from eave to ridge, degrees. Positive bows the
    /// plane upward (a barrel), negative into a bowl.
    pub fn sweep(&self) -> f64 {
        self.angle_at_eave - self.angle_at_ridge
    }

    /// Is the plane flat after all?
    pub fn is_straight(&self) -> bool {
        self.sweep().abs() < 1e-6
    }

    /// The Angle at Eave set to `eave` degrees with the pitch kept: the
    /// Angle at Ridge follows (`2 x pitch - eave`).
    pub fn with_eave_angle(&self, pitch_in_12: f64, eave: f64) -> Self {
        let a = pitch_deg(pitch_in_12);
        let (eave, ridge) = fit_pair(a, limit(eave), true);
        Self {
            angle_at_eave: eave,
            angle_at_ridge: ridge,
            ..*self
        }
    }

    /// The Angle at Ridge set to `ridge` degrees with the pitch kept: the
    /// Angle at Eave follows.
    pub fn with_ridge_angle(&self, pitch_in_12: f64, ridge: f64) -> Self {
        let a = pitch_deg(pitch_in_12);
        let (ridge, eave) = fit_pair(a, limit(ridge), false);
        Self {
            angle_at_eave: eave,
            angle_at_ridge: ridge,
            ..*self
        }
    }

    /// The curve of a plane whose chord has the given `pitch_in_12` over a
    /// horizontal `run` (eave to ridge) and whose surface lies `radius` from
    /// its centre. A bow, not a bowl. `None` when the radius is too short to
    /// span the chord.
    pub fn with_radius(&self, pitch_in_12: f64, run: f64, radius: f64) -> Option<Self> {
        let a = pitch_deg(pitch_in_12);
        let chord = run / a.to_radians().cos();
        if radius <= 0.0 || chord <= 0.0 || radius < chord * 0.5 {
            return None;
        }
        let half_sweep = (chord / (2.0 * radius)).asin().to_degrees();
        let (eave, ridge) = fit_pair(a, limit(a + half_sweep), true);
        Some(Self {
            angle_at_eave: eave,
            angle_at_ridge: ridge,
            ..*self
        })
    }

    /// Radius to Roof Surface for a plane whose eave-to-ridge run is `run`
    /// inches horizontally; 0 when the plane is straight.
    pub fn radius(&self, run: f64) -> f64 {
        if self.is_straight() || run <= 0.0 {
            return 0.0;
        }
        let a = self.chord_angle().to_radians();
        let chord = run / a.cos();
        chord / (2.0 * (self.sweep().to_radians() * 0.5).sin().abs())
    }

    /// The facet angle in use: [`DEFAULT_FACET_ANGLE`] when automatic, else
    /// the set one, moved to the nearest value that divides 360 degrees
    /// evenly.
    pub fn facet_angle_used(&self) -> f64 {
        let want = if self.auto_facet || self.facet_angle <= 0.0 {
            DEFAULT_FACET_ANGLE
        } else {
            self.facet_angle
        };
        let n = (360.0 / want).round().clamp(4.0, 3600.0);
        360.0 / n
    }

    /// How many facets the plane is cut into.
    pub fn facet_count(&self) -> usize {
        if self.is_straight() {
            return 1;
        }
        let n = (self.sweep().abs() / self.facet_angle_used() - 1e-9).ceil() as usize;
        n.clamp(1, MAX_FACETS)
    }

    /// The same curve on a plane whose pitch is now `pitch_in_12`: a straight
    /// spec stays straight, a bowed one keeps its Angle at Eave and takes the
    /// Angle at Ridge that gives the new pitch.
    pub fn retarget(&self, pitch_in_12: f64) -> Self {
        if self.is_straight() {
            Self {
                angle_at_eave: pitch_deg(pitch_in_12),
                angle_at_ridge: pitch_deg(pitch_in_12),
                ..*self
            }
        } else {
            self.with_eave_angle(pitch_in_12, self.angle_at_eave)
        }
    }

    /// The curve after the ridge edge of the plane moved so the chord now
    /// climbs `rise` over `run` (inches, both horizontal-vertical from the
    /// eave), keeping what `lock` says. The plane's eave does not move.
    /// `old_run` is the run the curve had. `None` when the new chord cannot
    /// be spanned (a locked radius too short for it).
    pub fn after_join(&self, old_run: f64, run: f64, rise: f64, lock: JoinLock) -> Option<Self> {
        if run <= 0.0 {
            return None;
        }
        let pitch = rise / run * 12.0;
        match lock {
            JoinLock::AngleAtRidge => Some(self.with_ridge_angle(pitch, self.angle_at_ridge)),
            JoinLock::Radius => {
                let radius = self.radius(old_run);
                if radius <= 0.0 {
                    return Some(Self::straight(pitch));
                }
                // The chord is longer or shorter now: a radius too short
                // for it cannot be kept.
                let chord = run.hypot(rise);
                if radius < chord * 0.5 {
                    return None;
                }
                let half = (chord / (2.0 * radius)).asin().to_degrees();
                let a = rise.atan2(run).to_degrees();
                let (eave, ridge) = fit_pair(a, limit(a + half), true);
                Some(Self {
                    angle_at_eave: eave,
                    angle_at_ridge: ridge,
                    ..*self
                })
            }
        }
    }
}

/// With the mean of the pair fixed at `chord` degrees and one member set to
/// `given`, the pair `(given, other)`; the other is `2 x chord - given`. When
/// the other would leave +-89 degrees, `given` moves so it does not.
/// `given_is_eave` only tells the caller which is which and is unused here.
fn fit_pair(chord: f64, given: f64, _given_is_eave: bool) -> (f64, f64) {
    let mut other = 2.0 * chord - given;
    let mut given = given;
    if other > MAX_ANGLE {
        other = MAX_ANGLE;
        given = 2.0 * chord - other;
    } else if other < -MAX_ANGLE {
        other = -MAX_ANGLE;
        given = 2.0 * chord - other;
    }
    (given, other)
}

/// Height of the curved surface above its eave at horizontal distance `u`
/// from the eave line, inches. `run` is the plane's eave-to-ridge run.
pub fn curve_height(spec: &CurvedSpec, run: f64, u: f64) -> f64 {
    let u = u.clamp(0.0, run);
    let chord_angle = spec.chord_angle().to_radians();
    if spec.is_straight() || run <= 0.0 {
        return u * chord_angle.tan();
    }
    let (te, sweep) = (spec.angle_at_eave.to_radians(), spec.sweep().to_radians());
    let chord = run / chord_angle.cos();
    let k = 2.0 * (sweep * 0.5).sin() / chord;
    // sin(theta) = sin(theta_e) - k u.
    let s = (te.sin() - k * u).clamp(-1.0, 1.0);
    (s.asin().cos() - te.cos()) / k
}

/// The cross section of the curved surface at the facet breaks: `(u, height)`
/// pairs from the eave (`u = 0`) to the ridge (`u = run`), both in inches.
pub fn section_points(spec: &CurvedSpec, run: f64) -> Vec<(f64, f64)> {
    let n = spec.facet_count();
    if spec.is_straight() || run <= 0.0 || n == 1 {
        let h = curve_height(spec, run, run);
        return vec![(0.0, 0.0), (run, h)];
    }
    let (te, sweep) = (spec.angle_at_eave.to_radians(), spec.sweep().to_radians());
    let chord = run / spec.chord_angle().to_radians().cos();
    let k = 2.0 * (sweep * 0.5).sin() / chord;
    (0..=n)
        .map(|j| {
            let th = te - sweep * j as f64 / n as f64;
            ((te.sin() - th.sin()) / k, (th.cos() - te.cos()) / k)
        })
        .collect()
}

/// The eave-to-ridge run of `plane`: how far its highest edge stands from the
/// eave line in plan, inches (0 for a degenerate plane).
pub fn plane_run(plane: &RoofPlane) -> f64 {
    let Some((o, up)) = frame(plane) else {
        return 0.0;
    };
    plane
        .plan_polygon()
        .iter()
        .map(|p| p.sub(o).dot(up))
        .fold(0.0, f64::max)
}

/// The eave's start in plan and the horizontal unit vector up the slope.
fn frame(plane: &RoofPlane) -> Option<(Point, Point)> {
    let poly = plane.plan_polygon();
    if poly.len() < 3 {
        return None;
    }
    let (a, b) = (poly[0], poly[1]);
    let e = b.sub(a);
    if e.length() < 1e-9 {
        return None;
    }
    let mut up = e.normalized().perp();
    let mid = Point::new(
        poly.iter().map(|p| p.x).sum::<f64>() / poly.len() as f64,
        poly.iter().map(|p| p.y).sum::<f64>() / poly.len() as f64,
    );
    if mid.sub(a).dot(up) < 0.0 {
        up = up.scale(-1.0);
    }
    Some((a, up))
}

/// The plane cut into flat facets that follow the curve `spec`, from the
/// eave up. A plane whose spec is straight comes back as it is. Each facet's
/// first edge is its lower edge; heights are relative to the plane's eave
/// elevation, so the eave and the ridge edge stay put.
pub fn curved_facets(plane: &RoofPlane, spec: &CurvedSpec) -> Vec<RoofPlane> {
    let run = plane_run(plane);
    let Some((origin, up)) = frame(plane) else {
        return vec![plane.clone()];
    };
    if spec.is_straight() || run <= 1e-6 {
        return vec![plane.clone()];
    }
    let section = section_points(spec, run);
    let eave_y = plane.polygon3d.first().map_or(0.0, |v| v[1]);
    let along = up.perp().scale(-1.0);
    // The strip between two breaks, long enough to hold any plane.
    let reach = plane
        .plan_polygon()
        .iter()
        .map(|p| p.sub(origin).length())
        .fold(0.0, f64::max)
        + 10.0;
    let subject = geom::ccw(&plane.plan_polygon());
    let mut out = Vec::new();
    for w in section.windows(2) {
        let ((u0, z0), (u1, z1)) = (w[0], w[1]);
        if u1 - u0 < 1e-9 {
            continue;
        }
        let corner = |u: f64, s: f64| origin.add(up.scale(u)).add(along.scale(s));
        let strip = vec![
            corner(u0, -reach),
            corner(u1, -reach),
            corner(u1, reach),
            corner(u0, reach),
        ];
        let strip = geom::ccw(&strip);
        let piece = geom::clip_convex(&subject, &strip);
        if piece.len() < 3 || polygon_area(&piece).abs() < 1e-6 {
            continue;
        }
        let height = |p: Point| {
            let u = p.sub(origin).dot(up);
            eave_y + z0 + (z1 - z0) * ((u - u0) / (u1 - u0)).clamp(0.0, 1.0)
        };
        let mut ring: Vec<V3> = piece.iter().map(|p| [p.x, height(*p), -p.y]).collect();
        // The lower edge first: the longest edge lying on u = u0.
        let n = ring.len();
        let on_low = |i: usize| {
            let (p, q) = (piece[i], piece[(i + 1) % n]);
            (p.sub(origin).dot(up) - u0).abs() < 1e-6 && (q.sub(origin).dot(up) - u0).abs() < 1e-6
        };
        let best = (0..n)
            .filter(|&i| on_low(i))
            .max_by(|&i, &j| {
                let l = |k: usize| piece[k].dist(piece[(k + 1) % n]);
                l(i).total_cmp(&l(j))
            })
            .unwrap_or(0);
        ring.rotate_left(best);
        let slope = (z1 - z0) / (u1 - u0);
        let (a, b) = (geom::to_plan(ring[0]), geom::to_plan(ring[1]));
        out.push(RoofPlane {
            polygon3d: ring,
            pitch_in_12: slope * 12.0,
            baseline: (a, b),
            source_edge: plane.source_edge,
        });
    }
    if out.is_empty() {
        vec![plane.clone()]
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A level plane: 20 ft along the eave, a 12 ft run, 100" eave.
    fn flat_plane() -> RoofPlane {
        // Plan (x, y) with the eave along x at y = 0 and the plane rising
        // toward +y; vertices [x, elevation, -y].
        let poly = vec![
            [0.0, 100.0, 0.0],
            [240.0, 100.0, 0.0],
            [240.0, 100.0, -144.0],
            [0.0, 100.0, -144.0],
        ];
        RoofPlane {
            polygon3d: poly,
            pitch_in_12: 0.0,
            baseline: (Point::new(0.0, 0.0), Point::new(240.0, 0.0)),
            source_edge: 0,
        }
    }

    fn sloped_plane(pitch: f64) -> RoofPlane {
        let rise = 144.0 * pitch / 12.0;
        let mut p = flat_plane();
        p.polygon3d[2][1] += rise;
        p.polygon3d[3][1] += rise;
        p.pitch_in_12 = pitch;
        p
    }

    #[test]
    fn the_pitch_is_the_mean_of_the_two_angles() {
        let s = CurvedSpec::straight(0.0).with_eave_angle(0.0, 45.0);
        assert!((s.angle_at_ridge + 45.0).abs() < 1e-9, "barrel: 45 and -45");
        assert!(s.chord_angle().abs() < 1e-9);
        // 12 in 12: ridge angle 1 degree leaves 89 at the eave.
        let t = CurvedSpec::straight(12.0).with_ridge_angle(12.0, 1.0);
        assert!((t.angle_at_eave - 89.0).abs() < 1e-9);
        assert!((t.chord_angle() - 45.0).abs() < 1e-9);
        // A straight spec starts with equal angles.
        let st = CurvedSpec::straight(12.0);
        assert_eq!((st.angle_at_eave, st.angle_at_ridge), (45.0, 45.0));
        assert!(st.is_straight());
        assert_eq!(st.radius(144.0), 0.0);
    }

    #[test]
    fn the_radius_is_tied_to_the_angles() {
        // Flat plane, 144" run, 45 and -45 degrees: chord 144, the arc turns
        // 90 degrees, radius = 144 / (2 sin 45) = 101.82".
        let s = CurvedSpec::straight(0.0).with_eave_angle(0.0, 45.0);
        let r = s.radius(144.0);
        assert!((r - 144.0 / (2.0 * 45f64.to_radians().sin())).abs() < 1e-9);
        // Setting the radius back gives the same angles.
        let back = CurvedSpec::straight(0.0)
            .with_radius(0.0, 144.0, r)
            .unwrap();
        assert!((back.angle_at_eave - 45.0).abs() < 1e-6);
        assert!((back.angle_at_ridge + 45.0).abs() < 1e-6);
        // A radius shorter than half the chord cannot span it.
        assert!(CurvedSpec::default()
            .with_radius(0.0, 144.0, 50.0)
            .is_none());
    }

    #[test]
    fn the_facet_angle_divides_360_and_sets_the_facet_count() {
        let s = CurvedSpec::straight(0.0).with_eave_angle(0.0, 45.0);
        assert_eq!(s.facet_angle_used(), 7.5);
        assert_eq!(s.facet_count(), 12, "90 degrees in 7.5 degree facets");
        // 8 degrees does not divide 360; 45 facets of 8 would, 360/8 = 45.
        let mut t = s;
        t.auto_facet = false;
        t.facet_angle = 7.0;
        // 360 / 7 = 51.4, so the nearest divisor is 360 / 51 = 7.0588.
        assert!((t.facet_angle_used() - 360.0 / 51.0).abs() < 1e-9);
        assert_eq!(t.facet_count(), 13);
        // A coarse angle gives few facets.
        t.facet_angle = 45.0;
        assert_eq!(t.facet_count(), 2);
    }

    #[test]
    fn the_barrel_section_lies_on_a_circle_and_keeps_its_ends() {
        let plane = flat_plane();
        let spec = CurvedSpec::straight(0.0).with_eave_angle(0.0, 45.0);
        let run = plane_run(&plane);
        assert!((run - 144.0).abs() < 1e-9);
        let r = spec.radius(run);
        let pts = section_points(&spec, run);
        assert_eq!(pts.len(), 13);
        // The ends: eave at the eave, ridge back at the eave's height (pitch 0).
        assert_eq!(pts[0], (0.0, 0.0));
        assert!((pts[12].0 - 144.0).abs() < 1e-6 && pts[12].1.abs() < 1e-6);
        // Every break lies on one circle of radius r.
        let te = 45f64.to_radians();
        let (cx, cz) = (r * te.sin(), -r * te.cos());
        for &(u, z) in &pts {
            let d = ((u - cx).powi(2) + (z - cz).powi(2)).sqrt();
            assert!(
                (d - r).abs() < 1e-6,
                "({u}, {z}) is {d} from the centre, not {r}"
            );
        }
        // The crown is r (1 - cos 45) above the ends.
        let crown = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max);
        assert!((crown - r * (1.0 - te.cos())).abs() < 1e-6);
        // curve_height agrees at the middle.
        assert!((curve_height(&spec, run, 72.0) - crown).abs() < 1e-6);
    }

    #[test]
    fn facets_tile_the_plane_and_follow_the_arc() {
        let plane = flat_plane();
        let spec = CurvedSpec::straight(0.0).with_eave_angle(0.0, 45.0);
        let facets = curved_facets(&plane, &spec);
        assert_eq!(facets.len(), 12);
        let area: f64 = facets.iter().map(RoofPlane::projected_area).sum();
        assert!(
            (area - plane.projected_area()).abs() < 1e-3,
            "tiles the plan"
        );
        // Neighbouring facets share their edge heights (no gaps).
        let r = spec.radius(144.0);
        let te = 45f64.to_radians();
        let (cx, cz) = (r * te.sin(), -r * te.cos());
        for f in &facets {
            assert!(f.normal()[1] > 0.0, "faces up");
            for v in &f.polygon3d {
                let (u, z) = (v[2].abs(), v[1] - 100.0);
                let d = ((u - cx).powi(2) + (z - cz).powi(2)).sqrt();
                assert!((d - r).abs() < 1e-6, "vertex off the arc by {}", d - r);
            }
            // The first edge of a facet is its lower, level edge.
            assert!((f.polygon3d[0][1] - f.polygon3d[1][1]).abs() < 1e-9);
        }
        // Facets of the first half climb, the second half falls.
        assert!(facets[0].pitch_in_12 > 0.0 && facets[11].pitch_in_12 < 0.0);
    }

    #[test]
    fn a_curved_pitched_plane_keeps_its_eave_and_ridge_heights() {
        let plane = sloped_plane(12.0);
        // 12:12, ridge angle 1 degree: nearly vertical at the eave.
        let spec = CurvedSpec::straight(12.0).with_ridge_angle(12.0, 1.0);
        let facets = curved_facets(&plane, &spec);
        assert!(facets.len() > 6);
        let top = facets
            .iter()
            .flat_map(|f| f.polygon3d.iter())
            .fold(f64::MIN, |m, v| m.max(v[1]));
        assert!((top - (100.0 + 144.0)).abs() < 1e-6, "ridge height kept");
        let low = facets
            .iter()
            .flat_map(|f| f.polygon3d.iter())
            .fold(f64::MAX, |m, v| m.min(v[1]));
        assert!((low - 100.0).abs() < 1e-6, "eave kept");
        // The steep lower facet is near vertical, the top one nearly flat.
        assert!(facets[0].pitch_in_12 > 100.0);
        assert!(facets[facets.len() - 1].pitch_in_12 < 2.0);
    }

    #[test]
    fn a_hip_plane_is_cut_to_its_triangle_in_every_strip() {
        // A triangular hip plane: eave 240" along x, apex 120" up the slope.
        let tri = RoofPlane {
            polygon3d: vec![
                [0.0, 100.0, 0.0],
                [240.0, 100.0, 0.0],
                [120.0, 160.0, -120.0],
            ],
            pitch_in_12: 6.0,
            baseline: (Point::new(0.0, 0.0), Point::new(240.0, 0.0)),
            source_edge: 3,
        };
        let spec = CurvedSpec::straight(6.0).with_ridge_angle(6.0, 10.0);
        let facets = curved_facets(&tri, &spec);
        assert!(facets.len() > 3);
        let area: f64 = facets.iter().map(RoofPlane::projected_area).sum();
        assert!((area - tri.projected_area()).abs() < 1e-3);
        assert!(facets.iter().all(|f| f.source_edge == 3));
    }

    #[test]
    fn joining_keeps_the_radius_or_the_ridge_angle() {
        let spec = CurvedSpec::straight(0.0).with_eave_angle(0.0, 45.0);
        let old_run = 144.0;
        let radius = spec.radius(old_run);
        // The ridge edge now meets a plane 36" higher.
        let by_radius = spec
            .after_join(old_run, 144.0, 36.0, JoinLock::Radius)
            .unwrap();
        assert!(
            (by_radius.radius(144.0) - radius).abs() < 1e-6,
            "radius locked"
        );
        assert!(
            (by_radius.chord_angle() - 36f64.atan2(144.0).to_degrees()).abs() < 1e-6,
            "the chord reaches the new height"
        );
        let by_ridge = spec
            .after_join(old_run, 144.0, 36.0, JoinLock::AngleAtRidge)
            .unwrap();
        assert!(
            (by_ridge.angle_at_ridge - spec.angle_at_ridge).abs() < 1e-9,
            "ridge angle locked"
        );
        assert!((by_ridge.chord_angle() - 36f64.atan2(144.0).to_degrees()).abs() < 1e-6);
        // The curvature changed.
        assert!((by_ridge.radius(144.0) - radius).abs() > 1.0);
        // A locked radius too short for a much longer chord is refused.
        assert!(spec
            .after_join(old_run, 144.0, 1000.0, JoinLock::Radius)
            .is_none());
    }

    #[test]
    fn a_straight_spec_is_left_alone() {
        let plane = sloped_plane(6.0);
        let facets = curved_facets(&plane, &CurvedSpec::straight(6.0));
        assert_eq!(facets, vec![plane]);
    }
}
