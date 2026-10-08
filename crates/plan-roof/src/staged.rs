//! Roofs with a break: an upper pitch (gambrel, mansard; RF-25) and the Dutch
//! gable (RF-21).
//!
//! Both are built in two stages. The lower roof is the plain weighted-skeleton
//! roof at the lower pitches. It is cut by the horizontal plane at the break
//! (`break_rise` above the eave); the lower planes keep the part below the cut.
//! The section of the lower roof at that height is a smaller polygon, and the
//! upper roof is a second roof built on it, with each edge at its upper pitch
//! (a Dutch gable edge is a vertical gable end there, so the hip below it ends
//! in a short gable). Where an edge keeps its pitch across the break the lower
//! and upper pieces are one plane again and are merged.
//!
//! Limits: the break is one height for the whole roof (the smallest `break_rise`
//! of the edges that give one). The vertical face of a Dutch gable (the
//! triangle that stands on the cut of the end hip, under the short gable) is
//! returned beside the roof as a plain polygon, see [`build_roof_with_faces`]
//! (`crate::build_roof_with_faces`); it is not one of the roof's planes.

use crate::geom::{self, V3};
use crate::{build_roof, EdgeKind, EdgeRoof, EdgeRoofSpec, Roof, RoofPlane};
use plan_core::geometry::polygon_area;
use plan_core::Point;

/// Break height as a fraction of the plain roof's height when no edge gives
/// one.
pub const DEFAULT_BREAK_FRACTION: f64 = 0.6;

/// Positions closer than this are the same (inches).
const EPS: f64 = 1e-4;

/// Does any edge ask for a break?
pub(crate) fn needs_break(specs: &[EdgeRoofSpec]) -> bool {
    specs
        .iter()
        .any(|s| s.dutch_gable || s.upper_pitch.is_some())
}

/// The staged roof and the vertical faces of its Dutch gables (outward-facing
/// roof-space polygons), or `None` when the plain roof should be used (the
/// skeleton had to approximate, or the break is at or above the peak).
pub(crate) fn build_staged(
    footprint: &[Point],
    specs: &[EdgeRoofSpec],
    baseline: f64,
) -> Option<(Roof, Vec<Vec<V3>>)> {
    let n = footprint.len();
    if n < 3 || polygon_area(footprint) <= 0.0 {
        return None; // counter-clockwise footprints only
    }
    let spec_of = |i: usize| specs.get(i).copied().unwrap_or_default();
    let lower_edges: Vec<EdgeRoof> = (0..n).map(|i| spec_of(i).to_edge_roof()).collect();
    let lower = build_roof(footprint, &lower_edges, baseline);
    if lower.approximate || lower.planes.is_empty() {
        return None;
    }
    let peak = lower.bounds().map(|(_, hi)| hi[1] - baseline)?;
    let rise = specs
        .iter()
        .filter(|s| s.dutch_gable || s.upper_pitch.is_some())
        .filter_map(|s| s.break_rise)
        .filter(|r| *r > 0.0)
        .fold(f64::INFINITY, f64::min);
    let rise = if rise.is_finite() {
        rise
    } else {
        peak * DEFAULT_BREAK_FRACTION
    };
    if rise <= EPS || rise >= peak - 1e-3 {
        return None;
    }
    let cut_y = baseline + rise;

    // The lower planes below the cut, with the cut segment of each.
    struct Piece {
        plane: RoofPlane,
        /// Cut segment, left to right along the eave direction.
        cut: Option<(Point, Point)>,
    }
    let mut pieces: Vec<Piece> = Vec::new();
    for pl in &lower.planes {
        let Some((poly, cut)) = clip_below(&pl.polygon3d, cut_y) else {
            continue;
        };
        pieces.push(Piece {
            plane: RoofPlane {
                polygon3d: poly,
                ..pl.clone()
            },
            cut,
        });
    }

    // The section polygon of the lower roof at the cut, edge by edge.
    let mut verts: Vec<Point> = Vec::new();
    let mut edge_of_vertex: Vec<usize> = Vec::new();
    let cut_of = |e: usize| {
        pieces
            .iter()
            .find(|p| p.plane.source_edge == e)
            .and_then(|p| p.cut)
    };
    for (e, lower_edge) in lower_edges.iter().enumerate() {
        match cut_of(e) {
            Some((a, _)) => {
                verts.push(a);
                edge_of_vertex.push(e);
            }
            None if lower_edge.kind != EdgeKind::Hip => {
                // A gable or shed edge runs from the end of the previous
                // segment to the start of the next one.
                let prev = (1..=n).find_map(|k| cut_of((e + n - k) % n).map(|(_, b)| b));
                let next = (1..=n).find_map(|k| cut_of((e + k) % n).map(|(a, _)| a));
                if let (Some(s), Some(t)) = (prev, next) {
                    if s.dist(t) > EPS {
                        verts.push(s);
                        edge_of_vertex.push(e);
                    }
                }
            }
            None => {}
        }
    }
    if verts.len() < 3 || polygon_area(&verts) < 1.0 {
        return None;
    }

    // The upper roof on the section.
    let upper_edges: Vec<EdgeRoof> = edge_of_vertex
        .iter()
        .map(|&e| {
            let s = spec_of(e);
            let mut er = lower_edges[e];
            er.overhang = 0.0;
            if er.kind == EdgeKind::Hip {
                if s.dutch_gable {
                    er.kind = EdgeKind::Gable;
                } else if let Some(p) = s.upper_pitch {
                    er.pitch_in_12 = p;
                }
            }
            er
        })
        .collect();
    let upper = build_roof(&verts, &upper_edges, cut_y);
    if upper.approximate {
        return None;
    }

    // The face under each Dutch gable: the cut of the end hip is its sill,
    // the upper roof's rake edges over that line its sides.
    let mut faces: Vec<Vec<V3>> = Vec::new();
    for e in 0..n {
        if !(spec_of(e).dutch_gable && lower_edges[e].kind == EdgeKind::Hip) {
            continue;
        }
        let Some((a, b)) = cut_of(e) else { continue };
        let len = a.dist(b);
        if len <= EPS {
            continue;
        }
        let dir = b.sub(a).normalized();
        let mut tops: Vec<(f64, V3)> = Vec::new();
        for v in upper.planes.iter().flat_map(|u| u.polygon3d.iter()) {
            let q = geom::to_plan(*v);
            let along = q.sub(a).dot(dir);
            let off = q.sub(a).cross(dir).abs();
            if v[1] > cut_y + 1e-3
                && off < 1e-3
                && (-1e-3..=len + 1e-3).contains(&along)
                && !tops.iter().any(|(t, _)| (t - along).abs() < 1e-3)
            {
                tops.push((along, *v));
            }
        }
        if tops.is_empty() {
            continue;
        }
        // From b's side back to a's, above the sill a -> b.
        tops.sort_by(|x, y| y.0.total_cmp(&x.0));
        let mut face = vec![geom::lift(a, cut_y), geom::lift(b, cut_y)];
        face.extend(tops.into_iter().map(|(_, v)| v));
        let d = footprint[(e + 1) % n].sub(footprint[e]).normalized();
        faces.push(geom::orient_toward(face, [d.y, 0.0, d.x]));
    }

    let mut planes: Vec<RoofPlane> = Vec::new();
    for piece in pieces {
        let e = piece.plane.source_edge;
        let s = spec_of(e);
        let up = upper.planes.iter().find(|u| {
            edge_of_vertex
                .get(u.source_edge)
                .is_some_and(|&mapped| mapped == e)
        });
        let same_pitch = s
            .upper_pitch
            .is_none_or(|p| (p - piece.plane.pitch_in_12).abs() < 1e-9);
        match up {
            Some(u) if same_pitch && !s.dutch_gable => {
                match merge_pieces(&piece.plane.polygon3d, &u.polygon3d) {
                    Some(poly) => planes.push(RoofPlane {
                        polygon3d: poly,
                        ..piece.plane
                    }),
                    None => {
                        planes.push(piece.plane);
                        planes.push(upper_plane(u, e));
                    }
                }
            }
            Some(u) => {
                planes.push(piece.plane);
                planes.push(upper_plane(u, e));
            }
            None => planes.push(piece.plane),
        }
    }
    Some((
        Roof {
            planes,
            fascia_height: lower.fascia_height,
            baseline_elevation: baseline,
            approximate: false,
        },
        faces,
    ))
}

fn upper_plane(u: &RoofPlane, source_edge: usize) -> RoofPlane {
    RoofPlane {
        source_edge,
        ..u.clone()
    }
}

/// A face clipped below a height: the polygon and its cut segment.
type Clipped = (Vec<V3>, Option<(Point, Point)>);

/// `poly` (an upward face whose first edge is the eave) clipped to `y <= cut`,
/// starting at the original first vertex again, and the segment it leaves on
/// the cut (left to right along the eave direction). `None` when nothing
/// stays below the cut.
fn clip_below(poly: &[V3], cut: f64) -> Option<Clipped> {
    let n = poly.len();
    if n < 3 {
        return None;
    }
    if poly.iter().all(|p| p[1] <= cut + EPS) {
        return Some((poly.to_vec(), None));
    }
    let inside = |p: V3| p[1] <= cut + EPS;
    let mut out: Vec<V3> = Vec::new();
    let mut exit: Option<V3> = None;
    let mut entry: Option<V3> = None;
    for i in 0..n {
        let (cur, prev) = (poly[i], poly[(i + n - 1) % n]);
        let cross_at = |a: V3, b: V3| {
            let t = (cut - a[1]) / (b[1] - a[1]);
            [a[0] + (b[0] - a[0]) * t, cut, a[2] + (b[2] - a[2]) * t]
        };
        match (inside(prev), inside(cur)) {
            (true, true) => out.push(cur),
            (true, false) => {
                let x = cross_at(prev, cur);
                exit = Some(x);
                out.push(x);
            }
            (false, true) => {
                let x = cross_at(prev, cur);
                entry = Some(x);
                out.push(x);
                out.push(cur);
            }
            (false, false) => {}
        }
    }
    out.dedup_by(|a, b| geom::sub3(*a, *b).iter().all(|c| c.abs() < EPS));
    if out.len() < 3 {
        return None;
    }
    // Start at the original eave vertex again.
    if let Some(k) = out.iter().position(|p| *p == poly[0]) {
        out.rotate_left(k);
    }
    let cut_seg = match (entry, exit) {
        (Some(a), Some(b)) if a != b => Some((geom::to_plan(a), geom::to_plan(b))),
        _ => None,
    };
    let cut_seg = cut_seg.filter(|(a, b)| a.dist(*b) > EPS);
    Some((out, cut_seg))
}

/// The lower piece `p` and the upper piece `u` of one plane as a single
/// polygon: they share the cut edge (`p` runs it right to left, `u` left to
/// right). The eave edge stays first; vertices that became collinear go.
fn merge_pieces(p: &[V3], u: &[V3]) -> Option<Vec<V3>> {
    if p.len() < 3 || u.len() < 3 {
        return None;
    }
    let same = |a: V3, b: V3| geom::sub3(a, b).iter().all(|c| c.abs() < 1e-3);
    // The cut edge of `p`: (u[1], u[0]).
    let k = (0..p.len()).find(|&k| same(p[k], u[1]) && same(p[(k + 1) % p.len()], u[0]))?;
    let mut merged: Vec<V3> = (0..p.len()).map(|j| p[(k + 1 + j) % p.len()]).collect();
    merged.extend(u[2..].iter().copied());
    // Back to the eave start.
    let first = merged.iter().position(|v| same(*v, p[0]))?;
    merged.rotate_left(first);
    // Drop collinear vertices (never the two eave vertices).
    let mut i = 2;
    while i < merged.len() && merged.len() > 3 {
        let (a, b, c) = (merged[i - 1], merged[i], merged[(i + 1) % merged.len()]);
        let (ab, bc) = (geom::sub3(b, a), geom::sub3(c, b));
        let cr = geom::cross3(ab, bc);
        let scale = geom::dot3(ab, ab).sqrt() * geom::dot3(bc, bc).sqrt();
        if geom::dot3(cr, cr).sqrt() <= 1e-6 * scale.max(1e-12) {
            merged.remove(i);
        } else {
            i += 1;
        }
    }
    Some(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_roof_with_specs;

    fn rect() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 288.0),
            Point::new(0.0, 288.0),
        ]
    }

    fn hip(pitch: f64) -> EdgeRoofSpec {
        EdgeRoofSpec {
            pitch,
            overhang: 0.0,
            ..EdgeRoofSpec::default()
        }
    }

    fn gable() -> EdgeRoofSpec {
        EdgeRoofSpec {
            gable: true,
            overhang: 0.0,
            ..EdgeRoofSpec::default()
        }
    }

    fn peak(roof: &Roof) -> f64 {
        roof.bounds().unwrap().1[1]
    }

    #[test]
    fn upper_pitch_makes_a_gambrel() {
        // Gable ends east and west; both long sides 6:12 up to 36" over the
        // eave, 24:12 above.
        let side = EdgeRoofSpec {
            upper_pitch: Some(24.0),
            break_rise: Some(36.0),
            ..hip(6.0)
        };
        let specs = vec![side, gable(), side, gable()];
        let roof = build_roof_with_specs(&rect(), &specs, 100.0);
        assert!(!roof.approximate);
        // 36" of rise take 72" of run; the remaining 72" of run rise 144".
        assert!((peak(&roof) - (100.0 + 36.0 + 144.0)).abs() < 1e-6);
        // Two planes per side: the lower slope and the steeper upper one.
        assert_eq!(roof.planes.len(), 4);
        let pitches: Vec<f64> = roof.planes.iter().map(|p| p.pitch_in_12).collect();
        assert_eq!(pitches.iter().filter(|&&p| p == 24.0).count(), 2);
        assert_eq!(pitches.iter().filter(|&&p| p == 6.0).count(), 2);
        // The planes tile the footprint.
        let area: f64 = roof.planes.iter().map(RoofPlane::projected_area).sum();
        assert!((area - 480.0 * 288.0).abs() < 1e-3, "{area}");
        for pl in &roof.planes {
            assert!(pl.normal()[1] > 0.0);
        }
    }

    #[test]
    fn an_upper_pitch_equal_to_the_lower_changes_nothing() {
        let plain = vec![hip(8.0), gable(), hip(8.0), gable()];
        let side = EdgeRoofSpec {
            upper_pitch: Some(8.0),
            break_rise: Some(40.0),
            ..hip(8.0)
        };
        let split = vec![side, gable(), side, gable()];
        let a = build_roof_with_specs(&rect(), &plain, 0.0);
        let b = build_roof_with_specs(&rect(), &split, 0.0);
        assert_eq!(a.planes.len(), b.planes.len());
        assert!((peak(&a) - peak(&b)).abs() < 1e-6);
        let area = |r: &Roof| r.planes.iter().map(RoofPlane::area).sum::<f64>();
        assert!((area(&a) - area(&b)).abs() < 1e-3);
    }

    #[test]
    fn dutch_gable_ends_a_hip_in_a_short_gable() {
        // Hip roof 8:12 with Dutch gables on the east and west ends.
        let long = hip(8.0);
        let dutch = EdgeRoofSpec {
            dutch_gable: true,
            break_rise: Some(60.0),
            ..hip(8.0)
        };
        let specs = vec![long, dutch, long, dutch];
        let roof = build_roof_with_specs(&rect(), &specs, 0.0);
        assert!(!roof.approximate);
        // The peak is the plain hip roof's (144 run at 8:12).
        assert!((peak(&roof) - 96.0).abs() < 1e-6);
        // The hip ridge was 192" long; with the gables it runs the full
        // length of the cut: the two long planes now reach 60" up the ends.
        let hip_roof = build_roof_with_specs(&rect(), &vec![long; 4], 0.0);
        let ridge_len = |r: &Roof| {
            let hi = peak(r);
            let xs: Vec<f64> = r
                .planes
                .iter()
                .flat_map(|p| p.polygon3d.iter())
                .filter(|v| (v[1] - hi).abs() < 1e-6)
                .map(|v| v[0])
                .collect();
            xs.iter().cloned().fold(f64::MIN, f64::max)
                - xs.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert!((ridge_len(&hip_roof) - 192.0).abs() < 1e-6);
        assert!(ridge_len(&roof) > 192.0 + 100.0, "{}", ridge_len(&roof));
        // Four planes of the hip still exist (the end hips are cut short).
        let ends: Vec<_> = roof.planes.iter().filter(|p| p.source_edge == 1).collect();
        assert_eq!(ends.len(), 1);
        assert!(ends[0].ridge_height() <= 60.0 + 1e-6);
        // Nothing sticks out of the footprint.
        let (lo, hi) = roof.bounds().unwrap();
        assert!(lo[0] >= -1e-6 && hi[0] <= 480.0 + 1e-6);
    }

    #[test]
    fn a_dutch_gable_gets_a_vertical_face_standing_on_the_cut_of_the_hip() {
        let long = hip(8.0);
        let dutch = EdgeRoofSpec {
            dutch_gable: true,
            break_rise: Some(60.0),
            ..hip(8.0)
        };
        let specs = vec![long, dutch, long, dutch];
        let (roof, faces) = crate::build_roof_with_faces(&rect(), &specs, 0.0);
        let plain = build_roof_with_specs(&rect(), &specs, 0.0);
        assert_eq!(roof, plain, "the roof is the same either way");
        // One face for each Dutch end.
        assert_eq!(faces.len(), 2);
        let mut sides = [0, 0];
        for face in &faces {
            let n = geom::unit3(geom::newell(face)).unwrap();
            assert!(n[1].abs() < 1e-9, "vertical: {n:?}");
            // Faces out of the building: the east end looks +x, the west -x.
            let east = n[0] > 0.9;
            assert!(east || n[0] < -0.9, "{n:?}");
            sides[usize::from(east)] += 1;
            let x = face[0][0];
            assert!(
                face.iter().all(|v| (v[0] - x).abs() < 1e-6),
                "one vertical plane"
            );
            assert!(
                x > 1.0 && x < 479.0,
                "inside the footprint, at the cut: {x}"
            );
            // The sill is the cut of the end hip at the break (60").
            assert!((face[0][1] - 60.0).abs() < 1e-6 && (face[1][1] - 60.0).abs() < 1e-6);
            // The top is the ridge, level with the roof's peak.
            let top = face.iter().fold(f64::MIN, |m, v| m.max(v[1]));
            assert!((top - peak(&roof)).abs() < 1e-6);
            // A triangle: the 108" cut sill and 36" of height.
            let area = geom::dot3(geom::newell(face), geom::newell(face)).sqrt() * 0.5;
            assert!((area - 0.5 * 108.0 * 36.0).abs() < 1e-3, "{area}");
            // The face meets the hip below it exactly along the sill.
            let end = roof
                .planes
                .iter()
                .find(|p| p.source_edge == if east { 1 } else { 3 })
                .unwrap();
            let on_end = |v: V3| {
                end.polygon3d
                    .iter()
                    .any(|w| geom::sub3(v, *w).iter().all(|c| c.abs() < 1e-6))
            };
            assert!(on_end(face[0]) || on_end(face[1]));
        }
        assert_eq!(sides, [1, 1]);
        // No Dutch gable, no faces; the plate variant carries them too.
        let (_, none) = crate::build_roof_with_faces(&rect(), &[hip(8.0); 4], 0.0);
        assert!(none.is_empty());
        let (_, at_plate) = crate::build_roof_at_plate_with_faces(&rect(), &specs, 100.0, 6.0);
        assert_eq!(at_plate.len(), 2);
    }

    #[test]
    fn a_break_above_the_peak_is_ignored() {
        let side = EdgeRoofSpec {
            upper_pitch: Some(20.0),
            break_rise: Some(500.0),
            ..hip(8.0)
        };
        let roof = build_roof_with_specs(&rect(), &[side, gable(), side, gable()], 0.0);
        assert_eq!(roof.planes.len(), 2);
        assert!((peak(&roof) - 96.0).abs() < 1e-6);
    }
}
