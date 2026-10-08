//! Ear-clipping triangulation for simple (possibly concave) polygons.

use plan_core::Point;

const EPS: f64 = 1e-9;

fn signed_area(pts: &[Point]) -> f64 {
    plan_core::geometry::polygon_area(pts)
}

fn in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let d1 = (b - a).cross(p - a);
    let d2 = (c - b).cross(p - b);
    let d3 = (a - c).cross(p - c);
    d1 >= -EPS && d2 >= -EPS && d3 >= -EPS
}

/// True if clipping `prev, cur, next` yields a convex corner containing no other vertex.
fn is_ear(pts: &[Point], ring: &[usize], i: usize) -> bool {
    let n = ring.len();
    let (ip, ic, inx) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
    let (a, b, c) = (pts[ip], pts[ic], pts[inx]);
    if (b - a).cross(c - b) <= EPS {
        return false;
    }
    !ring.iter().any(|&j| {
        if j == ip || j == ic || j == inx {
            return false;
        }
        let p = pts[j];
        // Vertices coincident with a triangle corner (touching rings) do not block.
        let coincident = [a, b, c].iter().any(|q| q.dist(p) <= EPS);
        !coincident && in_triangle(p, a, b, c)
    })
}

/// Pick the next vertex to clip: a true ear, else a collinear vertex, else vertex 0.
fn pick_clip(pts: &[Point], ring: &[usize]) -> usize {
    let n = ring.len();
    if let Some(i) = (0..n).find(|&i| is_ear(pts, ring, i)) {
        return i;
    }
    let collinear = (0..n).find(|&i| {
        let (a, b, c) = (
            pts[ring[(i + n - 1) % n]],
            pts[ring[i]],
            pts[ring[(i + 1) % n]],
        );
        (b - a).cross(c - b).abs() <= EPS
    });
    collinear.unwrap_or(0)
}

/// Triangulate a simple polygon by ear clipping.
///
/// Returns `n - 2` index triples into `pts`, each counter-clockwise in plan
/// space regardless of the input orientation. Degenerate (collinear) vertices
/// produce zero-area triangles so the `n - 2` count always holds.
pub fn ear_clip(pts: &[Point]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let mut ring: Vec<usize> = (0..n).collect();
    if signed_area(pts) < 0.0 {
        ring.reverse();
    }
    let mut tris = Vec::with_capacity(n - 2);
    while ring.len() > 3 {
        let i = pick_clip(pts, &ring);
        let m = ring.len();
        tris.push([ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]]);
        ring.remove(i);
    }
    tris.push([ring[0], ring[1], ring[2]]);
    tris
}

/// Triangulate a simple polygon with holes.
///
/// `outer` and every ring of `holes` may be given in either winding. Holes are
/// merged into the outer ring through zero-width bridges (rightmost hole
/// first, each bridge to the nearest visible vertex) and the merged ring is ear
/// clipped. The result are index triples into the concatenation
/// `outer ++ holes[0] ++ holes[1] ++ ...`, each counter-clockwise in plan space;
/// no triangle with area covers any part of a hole. A hole that cannot be
/// bridged (it lies outside the outer ring) is ignored. Bridges add zero-area
/// triangles, so the count is `outer + holes + 2 * holes - 2`.
pub fn ear_clip_with_holes(outer: &[Point], holes: &[Vec<Point>]) -> Vec<[usize; 3]> {
    if outer.len() < 3 {
        return Vec::new();
    }
    let mut all: Vec<Point> = outer.to_vec();
    let mut rings: Vec<Vec<usize>> = Vec::new();
    for h in holes {
        let start = all.len();
        all.extend_from_slice(h);
        if h.len() < 3 || signed_area(h).abs() <= EPS {
            continue;
        }
        let mut ring: Vec<usize> = (start..start + h.len()).collect();
        if signed_area(h) > 0.0 {
            ring.reverse(); // holes run clockwise
        }
        rings.push(ring);
    }
    let mut merged: Vec<usize> = (0..outer.len()).collect();
    if signed_area(outer) < 0.0 {
        merged.reverse();
    }
    let max_x = |ring: &Vec<usize>| ring.iter().map(|&i| all[i].x).fold(f64::MIN, f64::max);
    rings.sort_by(|a, b| max_x(b).total_cmp(&max_x(a)));
    for hole in rings {
        let mi = (0..hole.len())
            .max_by(|&a, &b| all[hole[a]].x.total_cmp(&all[hole[b]].x))
            .unwrap_or(0);
        let Some(pos) = find_bridge(&all, &merged, all[hole[mi]]) else {
            continue;
        };
        let mut next = merged[..=pos].to_vec();
        next.extend((0..=hole.len()).map(|k| hole[(mi + k) % hole.len()]));
        next.push(merged[pos]);
        next.extend_from_slice(&merged[pos + 1..]);
        merged = next;
    }
    let pts: Vec<Point> = merged.iter().map(|&i| all[i]).collect();
    ear_clip(&pts)
        .into_iter()
        .map(|t| [merged[t[0]], merged[t[1]], merged[t[2]]])
        .collect()
}

/// Position in `ring` of the vertex a hole vertex `m` should be bridged to:
/// the end of the nearest ring edge hit by a ray toward +x, or a ring vertex
/// inside the triangle (m, hit, end) that lies closest to the ray direction.
fn find_bridge(all: &[Point], ring: &[usize], m: Point) -> Option<usize> {
    let n = ring.len();
    let mut best: Option<(f64, usize, Point)> = None; // (hit x, edge start, hit)
    for i in 0..n {
        let (a, b) = (all[ring[i]], all[ring[(i + 1) % n]]);
        if (a.y - m.y) * (b.y - m.y) > 0.0 || (a.y - b.y).abs() <= EPS {
            continue;
        }
        let x = a.x + (m.y - a.y) * (b.x - a.x) / (b.y - a.y);
        if x >= m.x - EPS && best.is_none_or(|(bx, _, _)| x < bx) {
            best = Some((x, i, Point::new(x, m.y)));
        }
    }
    let (_, edge, hit) = best?;
    let (ia, ib) = (edge, (edge + 1) % n);
    let mut pos = if all[ring[ia]].x >= all[ring[ib]].x {
        ia
    } else {
        ib
    };
    let p = all[ring[pos]];
    if p.dist(hit) <= EPS {
        return Some(pos);
    }
    // Reflex vertices inside the triangle (m, hit, p) would block the bridge.
    let side = |a: Point, b: Point, q: Point| (b - a).cross(q - a);
    let orient = side(m, hit, p).signum();
    let mut best_angle = f64::MAX;
    for (j, &idx) in ring.iter().enumerate() {
        let q = all[idx];
        if j == pos || q.dist(m) <= EPS || q.dist(p) <= EPS || q.x < m.x {
            continue;
        }
        let inside = side(m, hit, q) * orient >= -EPS
            && side(hit, p, q) * orient >= -EPS
            && side(p, m, q) * orient >= -EPS;
        if !inside {
            continue;
        }
        let v = q - m;
        let angle = (v.y / v.x.max(EPS)).abs();
        let better = angle < best_angle - EPS
            || ((angle - best_angle).abs() <= EPS && q.dist(m) < all[ring[pos]].dist(m));
        if better {
            best_angle = angle;
            pos = j;
        }
    }
    Some(pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l_shape() -> Vec<Point> {
        [
            (0.0, 0.0),
            (120.0, 0.0),
            (120.0, 60.0),
            (60.0, 60.0),
            (60.0, 120.0),
            (0.0, 120.0),
        ]
        .iter()
        .map(|&(x, y)| Point::new(x, y))
        .collect()
    }

    fn total_area(pts: &[Point], tris: &[[usize; 3]]) -> f64 {
        tris.iter()
            .map(|t| signed_area(&[pts[t[0]], pts[t[1]], pts[t[2]]]))
            .sum()
    }

    #[test]
    fn concave_l_shape_gives_n_minus_2_triangles_with_polygon_area() {
        let pts = l_shape();
        let tris = ear_clip(&pts);
        assert_eq!(tris.len(), pts.len() - 2);
        let area = total_area(&pts, &tris);
        assert!(area > 0.0);
        assert!((area - signed_area(&pts)).abs() < 1e-6);
        assert!((area - 10_800.0).abs() < 1e-6);
        // Every triangle is CCW, so none overlaps by cancelling area.
        for t in &tris {
            assert!(signed_area(&[pts[t[0]], pts[t[1]], pts[t[2]]]) > 0.0);
        }
    }

    #[test]
    fn clockwise_input_is_handled() {
        let mut pts = l_shape();
        pts.reverse();
        let tris = ear_clip(&pts);
        assert_eq!(tris.len(), 4);
        assert!((total_area(&pts, &tris) - 10_800.0).abs() < 1e-6);
    }

    #[test]
    fn collinear_vertices_keep_the_triangle_count() {
        let pts: Vec<Point> = [
            (0.0, 0.0),
            (60.0, 0.0),
            (120.0, 0.0),
            (120.0, 50.0),
            (0.0, 50.0),
        ]
        .iter()
        .map(|&(x, y)| Point::new(x, y))
        .collect();
        let tris = ear_clip(&pts);
        assert_eq!(tris.len(), 3);
        assert!((total_area(&pts, &tris) - 6_000.0).abs() < 1e-6);
    }

    #[test]
    fn degenerate_input_is_empty() {
        assert!(ear_clip(&[Point::ZERO, Point::new(1.0, 0.0)]).is_empty());
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    fn tri_area(all: &[Point], t: &[usize; 3]) -> f64 {
        signed_area(&[all[t[0]], all[t[1]], all[t[2]]])
    }

    /// Triangles cover outer minus holes: area adds up, none is clockwise, and
    /// none contains a probe point inside a hole.
    fn check(outer: &[Point], holes: &[Vec<Point>], probes: &[Point]) {
        let mut all = outer.to_vec();
        for h in holes {
            all.extend_from_slice(h);
        }
        let tris = ear_clip_with_holes(outer, holes);
        let want =
            signed_area(outer).abs() - holes.iter().map(|h| signed_area(h).abs()).sum::<f64>();
        let got: f64 = tris.iter().map(|t| tri_area(&all, t)).sum();
        assert!((got - want).abs() < 1e-6, "area {got} vs {want}");
        assert!(tris.iter().all(|t| tri_area(&all, t) >= -1e-9));
        for p in probes {
            for t in &tris {
                if tri_area(&all, t) > 1e-9 {
                    let (a, b, c) = (all[t[0]], all[t[1]], all[t[2]]);
                    assert!(!(in_triangle(*p, a, b, c)), "probe {p:?} covered by {t:?}");
                }
            }
        }
    }

    #[test]
    fn square_with_one_hole() {
        let outer = rect(0.0, 0.0, 100.0, 100.0);
        let hole = rect(40.0, 40.0, 60.0, 60.0);
        check(
            &outer,
            &[hole],
            &[Point::new(50.0, 50.0), Point::new(41.0, 59.0)],
        );
    }

    #[test]
    fn windings_do_not_matter() {
        let mut outer = rect(0.0, 0.0, 100.0, 100.0);
        let mut hole = rect(20.0, 20.0, 50.0, 70.0);
        outer.reverse();
        check(
            &outer,
            std::slice::from_ref(&hole),
            &[Point::new(35.0, 45.0)],
        );
        hole.reverse();
        check(&outer, &[hole], &[Point::new(35.0, 45.0)]);
    }

    #[test]
    fn several_holes_and_a_concave_outer() {
        let outer = l_shape();
        let holes = vec![
            rect(10.0, 10.0, 25.0, 25.0),
            rect(70.0, 10.0, 100.0, 30.0),
            rect(10.0, 70.0, 40.0, 100.0),
        ];
        let probes = [
            Point::new(17.0, 17.0),
            Point::new(85.0, 20.0),
            Point::new(25.0, 85.0),
        ];
        check(&outer, &holes, &probes);
    }

    #[test]
    fn triangular_hole() {
        let outer = rect(0.0, 0.0, 100.0, 60.0);
        let hole = vec![
            Point::new(30.0, 20.0),
            Point::new(60.0, 30.0),
            Point::new(30.0, 40.0),
        ];
        check(&outer, &[hole], &[Point::new(40.0, 30.0)]);
    }

    #[test]
    fn no_holes_matches_ear_clip_and_outside_hole_is_ignored() {
        let outer = rect(0.0, 0.0, 10.0, 10.0);
        assert_eq!(ear_clip_with_holes(&outer, &[]).len(), 2);
        let far = rect(50.0, 50.0, 60.0, 60.0);
        let tris = ear_clip_with_holes(&outer, &[far]);
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn grids_of_holes_aligned_and_jittered() {
        let outer = rect(0.0, 0.0, 400.0, 400.0);
        let mut seed = 12345u64;
        let mut rnd = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as f64 / (1u64 << 31) as f64
        };
        for jitter in [false, true] {
            let mut holes = Vec::new();
            let mut probes = Vec::new();
            for i in 0..4 {
                for j in 0..4 {
                    let (cx, cy) = (50.0 + 100.0 * i as f64, 50.0 + 100.0 * j as f64);
                    let (dx, dy) = if jitter {
                        ((rnd() - 0.5) * 20.0, (rnd() - 0.5) * 20.0)
                    } else {
                        (0.0, 0.0)
                    };
                    let (w, h) = (if jitter { 10.0 + rnd() * 20.0 } else { 20.0 }, 20.0);
                    holes.push(rect(cx + dx - w, cy + dy - h, cx + dx + w, cy + dy + h));
                    probes.push(Point::new(cx + dx, cy + dy));
                }
            }
            check(&outer, &holes, &probes);
        }
    }
}
