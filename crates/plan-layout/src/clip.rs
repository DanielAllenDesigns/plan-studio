//! Software clipping against an axis-aligned rectangle (`PdfDoc` has no clip
//! operator), so a clipped layout box can never draw outside its frame.

/// A clip rectangle `[x_min, y_min, x_max, y_max]` in PDF points.
pub(crate) type Rect = [f64; 4];

pub(crate) type Pt = (f64, f64);

/// Whether `p` is inside `r` (edges inclusive).
pub(crate) fn contains(r: Rect, p: Pt) -> bool {
    p.0 >= r[0] && p.0 <= r[2] && p.1 >= r[1] && p.1 <= r[3]
}

/// Liang-Barsky segment clip. Returns the visible part of `a`-`b`, if any.
pub(crate) fn clip_segment(r: Rect, a: Pt, b: Pt) -> Option<(Pt, Pt)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    let checks = [
        (-dx, a.0 - r[0]),
        (dx, r[2] - a.0),
        (-dy, a.1 - r[1]),
        (dy, r[3] - a.1),
    ];
    for (p, q) in checks {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                if t > t1 {
                    return None;
                }
                t0 = t0.max(t);
            } else {
                if t < t0 {
                    return None;
                }
                t1 = t1.min(t);
            }
        }
    }
    let clamp = |p: Pt| (p.0.clamp(r[0], r[2]), p.1.clamp(r[1], r[3]));
    Some((
        clamp((a.0 + t0 * dx, a.1 + t0 * dy)),
        clamp((a.0 + t1 * dx, a.1 + t1 * dy)),
    ))
}

/// Sutherland-Hodgman polygon clip. Returns an empty vector when nothing remains.
pub(crate) fn clip_polygon(r: Rect, poly: &[Pt]) -> Vec<Pt> {
    // Each edge: (inside test, intersection with the edge line).
    type Edge = (fn(Rect, Pt) -> bool, fn(Rect, Pt, Pt) -> Pt);
    fn at_x(x: f64, a: Pt, b: Pt) -> Pt {
        let t = (x - a.0) / (b.0 - a.0);
        (x, a.1 + t * (b.1 - a.1))
    }
    fn at_y(y: f64, a: Pt, b: Pt) -> Pt {
        let t = (y - a.1) / (b.1 - a.1);
        (a.0 + t * (b.0 - a.0), y)
    }
    let edges: [Edge; 4] = [
        (|r, p| p.0 >= r[0], |r, a, b| at_x(r[0], a, b)),
        (|r, p| p.0 <= r[2], |r, a, b| at_x(r[2], a, b)),
        (|r, p| p.1 >= r[1], |r, a, b| at_y(r[1], a, b)),
        (|r, p| p.1 <= r[3], |r, a, b| at_y(r[3], a, b)),
    ];
    let mut out: Vec<Pt> = poly.to_vec();
    for (inside, cross) in edges {
        let input = std::mem::take(&mut out);
        for (i, &cur) in input.iter().enumerate() {
            let prev = input[(i + input.len() - 1) % input.len()];
            match (inside(r, prev), inside(r, cur)) {
                (true, true) => out.push(cur),
                (true, false) => out.push(cross(r, prev, cur)),
                (false, true) => {
                    out.push(cross(r, prev, cur));
                    out.push(cur);
                }
                (false, false) => {}
            }
        }
        if out.is_empty() {
            break;
        }
    }
    for p in &mut out {
        p.0 = p.0.clamp(r[0], r[2]);
        p.1 = p.1.clamp(r[1], r[3]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = [0.0, 0.0, 10.0, 10.0];

    #[test]
    fn segment_inside_outside_and_crossing() {
        assert_eq!(
            clip_segment(R, (1.0, 1.0), (2.0, 2.0)),
            Some(((1.0, 1.0), (2.0, 2.0)))
        );
        assert_eq!(clip_segment(R, (-5.0, 11.0), (-1.0, 20.0)), None);
        let (a, b) = clip_segment(R, (-5.0, 5.0), (15.0, 5.0)).unwrap();
        assert_eq!((a, b), ((0.0, 5.0), (10.0, 5.0)));
        // Parallel to an edge and outside.
        assert_eq!(clip_segment(R, (-1.0, -5.0), (-1.0, 5.0)), None);
    }

    #[test]
    fn polygon_clip_keeps_inside_part() {
        let big = [(-5.0, -5.0), (15.0, -5.0), (15.0, 15.0), (-5.0, 15.0)];
        let out = clip_polygon(R, &big);
        assert_eq!(out.len(), 4);
        assert!(out.iter().all(|&p| contains(R, p)));
        let away = [(20.0, 20.0), (30.0, 20.0), (30.0, 30.0)];
        assert!(clip_polygon(R, &away).is_empty());
    }
}
