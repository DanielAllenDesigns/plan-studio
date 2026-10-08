//! Half hip (jerkinhead) gable ends (RF-3, Roof Styles > Half Hip).
//!
//! A half hip is a gable whose peak is clipped by a small hip: the end wall
//! rises vertically to the *clip height*, and above it a short hip plane
//! slopes up and back to the ridge. The roof is built as a plain gable roof
//! first (the neighbouring planes reach the rake line of the end edge); each
//! of those planes is then cut by the hip plane that starts on the rake line
//! at the clip height, and the hip plane's face is added as a plane of its
//! own.

use crate::geom::{self, V3};
use crate::{Roof, RoofPlane};
use plan_core::Point;

/// Where two points count as the same, inches.
const SAME: f64 = 0.05;
/// A vertex this close to the rake line is on it, inches.
const ON_LINE: f64 = 0.5;
/// Share of the gable's height the wall keeps when no clip height is given.
pub const DEFAULT_CLIP_FRACTION: f64 = 0.6;

/// Clips the gable end `edge` (`a -> b`, counter-clockwise footprint edge,
/// rake line `overhang` inches outside it) of `roof` with a hip of `pitch`
/// (rise per 12). `clip_rise` is the height of the clip above the lowest eave
/// tip on the rake line; `None` keeps [`DEFAULT_CLIP_FRACTION`] of the gable.
/// Returns whether the roof was changed.
pub(crate) fn clip_gable_end(
    roof: &mut Roof,
    edge: (Point, Point),
    overhang: f64,
    pitch: f64,
    clip_rise: Option<f64>,
    source_edge: usize,
) -> bool {
    let d = edge.1.sub(edge.0);
    if d.length() < 1e-9 || pitch <= 0.0 {
        return false;
    }
    let along = d.normalized();
    // Counter-clockwise footprint: the building is on the left, so outside is
    // on the right of the edge.
    let out = Point::new(along.y, -along.x);
    let e0 = edge.0.add(out.scale(overhang));
    let s_of = |p: Point| p.sub(e0).dot(out);
    // The gable's base and apex on the rake line.
    let mut on_line: Vec<f64> = Vec::new();
    for pl in &roof.planes {
        for v in &pl.polygon3d {
            if s_of(geom::to_plan(*v)).abs() <= ON_LINE {
                on_line.push(v[1]);
            }
        }
    }
    let (Some(lo), Some(hi)) = (
        on_line.iter().copied().reduce(f64::min),
        on_line.iter().copied().reduce(f64::max),
    ) else {
        return false;
    };
    if hi - lo < 2.0 {
        return false;
    }
    let clip = match clip_rise {
        Some(r) => lo + r,
        None => lo + DEFAULT_CLIP_FRACTION * (hi - lo),
    }
    .clamp(lo + 1.0, hi - 1.0);
    let k = pitch / 12.0;
    // Height of the hip plane above plan point `p`.
    let hip_y = |p: Point| clip - k * s_of(p);
    let outside = |v: V3| v[1] - hip_y(geom::to_plan(v));

    let mut planes: Vec<RoofPlane> = Vec::with_capacity(roof.planes.len() + 1);
    let mut cuts: Vec<(V3, V3)> = Vec::new();
    for pl in &roof.planes {
        let touches = pl
            .polygon3d
            .iter()
            .any(|v| s_of(geom::to_plan(*v)).abs() <= ON_LINE);
        if !touches || !pl.polygon3d.iter().any(|v| outside(*v) > SAME) {
            planes.push(pl.clone());
            continue;
        }
        let Some((poly, cut)) = clip_polygon(&pl.polygon3d, &outside) else {
            // Nothing of the plane stays.
            continue;
        };
        if let Some(c) = cut {
            cuts.push(c);
        }
        planes.push(RoofPlane {
            polygon3d: poly,
            ..pl.clone()
        });
    }
    if cuts.is_empty() {
        return false;
    }
    let Some(face) = hip_face(&cuts, &s_of, along) else {
        return false;
    };
    let (p0, p1) = (geom::to_plan(face[0]), geom::to_plan(face[1]));
    planes.push(RoofPlane {
        polygon3d: face,
        pitch_in_12: pitch,
        baseline: (p0, p1),
        source_edge,
    });
    roof.planes = planes;
    true
}

/// `poly` with the part where `outside` is positive removed; the first vertex
/// stays first when it survives. The second value is the new edge on the cut
/// (its two end points), if the polygon was cut at all.
/// A polygon cut by the hip plane and the new edge it got on the cut.
type Clipped = (Vec<V3>, Option<(V3, V3)>);

fn clip_polygon(poly: &[V3], outside: &dyn Fn(V3) -> f64) -> Option<Clipped> {
    let n = poly.len();
    let mut out: Vec<V3> = Vec::new();
    let mut crossings: Vec<V3> = Vec::new();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (fa, fb) = (outside(a), outside(b));
        let (ina, inb) = (fa <= SAME, fb <= SAME);
        if ina {
            out.push(a);
        }
        if ina != inb {
            let t = fa / (fa - fb);
            let x = geom::add3(a, geom::scale3(geom::sub3(b, a), t));
            out.push(x);
            crossings.push(x);
        }
    }
    if out.len() < 3 {
        return None;
    }
    // Drop repeats that the SAME tolerance let through.
    out.dedup_by(|a, b| geom::dot3(geom::sub3(*a, *b), geom::sub3(*a, *b)) < SAME * SAME);
    if out.len() < 3 {
        return None;
    }
    // Keep the original first vertex first.
    if let Some(k) = out.iter().position(|v| *v == poly[0]) {
        out.rotate_left(k);
    }
    let cut = (crossings.len() == 2).then(|| (crossings[0], crossings[1]));
    Some((out, cut))
}

/// The face of the hip: base on the rake line (counter-clockwise, first
/// edge), then back through the cuts of the planes.
fn hip_face(cuts: &[(V3, V3)], s_of: &dyn Fn(Point) -> f64, along: Point) -> Option<Vec<V3>> {
    let same = |a: V3, b: V3| {
        let d = geom::sub3(a, b);
        geom::dot3(d, d) < SAME * SAME * 100.0
    };
    // Chain the cuts into one polyline.
    let mut segs: Vec<(V3, V3)> = cuts.to_vec();
    let first = segs.remove(0);
    let mut chain: Vec<V3> = vec![first.0, first.1];
    while !segs.is_empty() {
        let tail = *chain.last()?;
        let head = chain[0];
        let at = |p: V3| segs.iter().position(|s| same(s.0, p) || same(s.1, p));
        let (i, at_tail) = at(tail)
            .map(|i| (i, true))
            .or_else(|| at(head).map(|i| (i, false)))?;
        let s = segs.remove(i);
        let from = if at_tail { tail } else { head };
        let other = if same(s.0, from) { s.1 } else { s.0 };
        if at_tail {
            chain.push(other);
        } else {
            chain.insert(0, other);
        }
    }
    if chain.len() < 3 {
        return None;
    }
    // The two ends of the chain lie on the rake line.
    let (a, b) = (chain[0], *chain.last()?);
    if s_of(geom::to_plan(a)).abs() > ON_LINE || s_of(geom::to_plan(b)).abs() > ON_LINE {
        return None;
    }
    // Base from the end that comes first along the edge to the other.
    let ta = geom::to_plan(a).dot(along);
    let tb = geom::to_plan(b).dot(along);
    let mut face = chain;
    if ta > tb {
        face.reverse();
    }
    // Eave first: [start, end, then the cut back to the start].
    let end = face.pop()?;
    let start = face.remove(0);
    let mut poly: Vec<V3> = vec![start, end];
    // Interior points on the baseline itself add nothing.
    poly.extend(
        face.into_iter()
            .rev()
            .filter(|v| s_of(geom::to_plan(*v)).abs() > ON_LINE),
    );
    if poly.len() < 3 {
        return None;
    }
    // Counter-clockwise from above: the Newell normal must point up.
    if geom::newell(&poly)[1] < 0.0 {
        return None;
    }
    Some(poly)
}

#[cfg(test)]
mod tests {
    use crate::{build_roof_with_specs, EdgeRoofSpec, Roof, RoofPlane};
    use plan_core::Point;

    fn rect() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 288.0),
            Point::new(0.0, 288.0),
        ]
    }

    /// Hip sides (edges 0 and 2) and gable ends (1 and 3) with optional half
    /// hips.
    fn specs(half: Option<f64>, rise: Option<f64>) -> Vec<EdgeRoofSpec> {
        let hip = EdgeRoofSpec {
            overhang: 0.0,
            ..EdgeRoofSpec::default()
        };
        let gable = EdgeRoofSpec {
            full_gable_wall: true,
            overhang: 0.0,
            half_hip_pitch: half,
            half_hip_rise: rise,
            ..EdgeRoofSpec::default()
        };
        vec![hip, gable, hip, gable]
    }

    fn area(roof: &Roof) -> f64 {
        roof.planes.iter().map(RoofPlane::projected_area).sum()
    }

    #[test]
    fn a_half_hip_clips_the_gable_peak_with_a_small_hip() {
        let plain = build_roof_with_specs(&rect(), &specs(None, None), 100.0);
        assert_eq!(plain.planes.len(), 2);
        let half = build_roof_with_specs(&rect(), &specs(Some(8.0), None), 100.0);
        assert!(!half.approximate);
        // Two side planes and a hip face at each end.
        assert_eq!(half.planes.len(), 4);
        // The roof is still watertight in plan: the faces tile the footprint.
        assert!(
            (area(&half) - 480.0 * 288.0).abs() < 1e-3,
            "{}",
            area(&half)
        );
        // Every plane faces up and the ridge height is unchanged.
        for p in &half.planes {
            assert!(p.normal()[1] > 0.0);
        }
        let (_, hi) = half.bounds().unwrap();
        let (_, hi_plain) = plain.bounds().unwrap();
        assert!((hi[1] - hi_plain[1]).abs() < 1e-6);
        // The hip face starts on the end line at the clip height (60% of the
        // 96" gable) and climbs at its own pitch.
        let faces: Vec<&RoofPlane> = half
            .planes
            .iter()
            .filter(|p| p.source_edge % 2 == 1)
            .collect();
        assert_eq!(faces.len(), 2);
        for f in faces {
            assert!((f.pitch_in_12 - 8.0).abs() < 1e-9);
            let base = f.polygon3d[0][1];
            assert!((base - (100.0 + 0.6 * 96.0)).abs() < 1e-6, "base {base}");
            assert!((f.polygon3d[0][1] - f.polygon3d[1][1]).abs() < 1e-9);
        }
    }

    #[test]
    fn the_clip_height_can_be_given() {
        let half = build_roof_with_specs(&rect(), &specs(Some(12.0), Some(48.0)), 100.0);
        let face = half.planes.iter().find(|p| p.source_edge == 1).unwrap();
        assert!((face.polygon3d[0][1] - 148.0).abs() < 1e-6);
        assert!((area(&half) - 480.0 * 288.0).abs() < 1e-3);
    }

    #[test]
    fn a_plain_gable_end_is_unchanged_without_a_half_hip() {
        let a = build_roof_with_specs(&rect(), &specs(None, None), 100.0);
        let b = build_roof_with_specs(&rect(), &specs(None, Some(10.0)), 100.0);
        assert_eq!(a, b);
    }
}
