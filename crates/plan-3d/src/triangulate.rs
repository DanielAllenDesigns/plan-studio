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
}
