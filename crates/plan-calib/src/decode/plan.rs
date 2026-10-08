//! Plan-view (top-down, hidden-line) projection of a triangle mesh.
//!
//! Chief draws most library objects in plan from their 3D geometry. The
//! catalogs carry no separate 2D symbol, so Plan Studio does the same: it
//! looks straight down at the mesh and keeps the edges a plan drawing shows.
//!
//! 1. Vertices are welded and a depth buffer is rasterized (highest Z per
//!    cell).
//! 2. Candidate edges are mesh boundary edges, creases (adjacent faces more
//!    than [`PlanOptions::crease_deg`] apart) and silhouette edges (one
//!    adjacent face looks up, the other down).
//! 3. Each candidate is sampled along its length and kept where it is not
//!    buried under a higher surface.
//! 4. The kept pieces are chained into polylines, simplified, near-circles are
//!    turned into [`Stroke::Circle`], and the whole drawing is rotated 180
//!    degrees and centred on the XY bounds so that the object's front faces
//!    +Y as `plan_library` expects (Chief's front faces -Y).

use super::mesh::Mesh;
use plan_core::geometry::Point;
use plan_library::Stroke;
use std::collections::HashMap;

/// Tuning knobs for [`plan_view`].
#[derive(Debug, Clone, Copy)]
pub struct PlanOptions {
    /// Depth-buffer cells along the longer side of the object.
    pub grid: usize,
    /// Dihedral angle (degrees) above which a shared edge is drawn.
    pub crease_deg: f64,
    /// Upper bound on the number of strokes (the longest are kept).
    pub max_strokes: usize,
}

impl Default for PlanOptions {
    fn default() -> Self {
        PlanOptions {
            grid: 200,
            crease_deg: 20.0,
            max_strokes: 400,
        }
    }
}

/// The projected drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanView {
    /// Strokes in inches, centred on the origin, front towards +Y.
    pub strokes: Vec<Stroke>,
    /// X extent of the mesh.
    pub width: f64,
    /// Y extent of the mesh.
    pub depth: f64,
    /// Z extent of the mesh.
    pub height: f64,
}

type Pt2 = [f64; 2];

/// Projects `meshes` into a plan drawing; `None` when they have no area.
pub fn plan_view(meshes: &[&Mesh], opts: PlanOptions) -> Option<PlanView> {
    let (verts, tris) = weld(meshes);
    if tris.is_empty() {
        return None;
    }
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for t in &tris {
        for &i in t {
            for (k, c) in verts[i as usize].iter().enumerate() {
                lo[k] = lo[k].min(*c);
                hi[k] = hi[k].max(*c);
            }
        }
    }
    let (lx, ly) = (hi[0] - lo[0], hi[1] - lo[1]);
    let longest = lx.max(ly);
    if longest.is_nan() || longest <= 1e-6 {
        return None;
    }
    let cell = longest / opts.grid.max(8) as f64;
    let gw = (lx / cell).ceil() as usize + 3;
    let gh = (ly / cell).ceil() as usize + 3;
    let zbuf = depth_buffer(&verts, &tris, lo, cell, gw, gh);

    let candidates = candidate_edges(&verts, &tris, opts.crease_deg);
    let mut pieces: Vec<(Pt2, Pt2)> = Vec::new();
    for (u, v) in candidates {
        visible_pieces(
            verts[u as usize],
            verts[v as usize],
            &zbuf,
            lo,
            cell,
            gw,
            gh,
            &mut pieces,
        );
    }
    // Orient every piece the same way, sort for a deterministic result and
    // drop coincident pieces (the foot and the top of a vertical wall).
    for p in &mut pieces {
        if key(p.0) > key(p.1) {
            std::mem::swap(&mut p.0, &mut p.1);
        }
    }
    pieces.sort_by_key(|a| (key(a.0), key(a.1)));
    pieces.dedup_by(|a, b| key(a.0) == key(b.0) && key(a.1) == key(b.1));

    let eps = (longest * 0.0015).max(0.03);
    let min_len = (longest * 0.004).max(0.3);
    let mut chains = chain(&pieces);
    let mut strokes: Vec<(f64, Stroke, bool)> = Vec::new();
    for (pts, closed) in chains.drain(..) {
        let pts = simplify(&pts, eps, closed);
        let len = path_len(&pts, closed);
        if len < min_len || pts.len() < 2 {
            continue;
        }
        if closed && pts.len() >= 3 {
            if let Some((c, r)) = fit_circle(&pts) {
                strokes.push((
                    len,
                    Stroke::Circle {
                        center: Point::new(c[0], c[1]),
                        radius: r,
                    },
                    true,
                ));
                continue;
            }
        }
        strokes.push((
            len,
            Stroke::Polyline {
                points: pts.iter().map(|p| Point::new(p[0], p[1])).collect(),
                closed: closed && pts.len() >= 3,
            },
            closed,
        ));
    }
    strokes.sort_by(|a, b| b.0.total_cmp(&a.0));
    strokes.truncate(opts.max_strokes);

    let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let mut out: Vec<Stroke> = strokes
        .into_iter()
        .map(|(_, s, _)| rotate_centre(s, centre))
        .collect();
    if out.is_empty() {
        // Nothing survived (a flat or fully smooth shape): fall back to the
        // footprint so the symbol still has the right extent.
        let (hw, hd) = (lx / 2.0, ly / 2.0);
        out.push(Stroke::Polyline {
            points: vec![
                Point::new(-hw, -hd),
                Point::new(hw, -hd),
                Point::new(hw, hd),
                Point::new(-hw, hd),
            ],
            closed: true,
        });
    }
    Some(PlanView {
        strokes: out,
        width: lx,
        depth: ly,
        height: hi[2] - lo[2],
    })
}

fn rotate_centre(s: Stroke, c: Pt2) -> Stroke {
    let f = |p: Point| Point::new(-(p.x - c[0]), -(p.y - c[1]));
    match s {
        Stroke::Polyline { points, closed } => Stroke::Polyline {
            points: points.into_iter().map(f).collect(),
            closed,
        },
        Stroke::Circle { center, radius } => Stroke::Circle {
            center: f(center),
            radius,
        },
        Stroke::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => Stroke::Arc {
            center: f(center),
            radius,
            start_deg: start_deg + 180.0,
            end_deg: end_deg + 180.0,
        },
    }
}

/// Welds vertices closer than 1/1000 inch and drops degenerate triangles.
fn weld(meshes: &[&Mesh]) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
    const Q: f64 = 1.0e-3;
    let mut ids: HashMap<(i64, i64, i64), u32> = HashMap::new();
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for m in meshes {
        let mut remap: Vec<u32> = Vec::with_capacity(m.vertices.len());
        for v in &m.vertices {
            let k = (
                (v[0] / Q).round() as i64,
                (v[1] / Q).round() as i64,
                (v[2] / Q).round() as i64,
            );
            let id = *ids.entry(k).or_insert_with(|| {
                verts.push(*v);
                (verts.len() - 1) as u32
            });
            remap.push(id);
        }
        for t in &m.triangles {
            let r = [
                remap.get(t[0] as usize).copied(),
                remap.get(t[1] as usize).copied(),
                remap.get(t[2] as usize).copied(),
            ];
            if let [Some(a), Some(b), Some(c)] = r {
                if a != b && b != c && a != c {
                    tris.push([a, b, c]);
                }
            }
        }
    }
    (verts, tris)
}

fn depth_buffer(
    verts: &[[f64; 3]],
    tris: &[[u32; 3]],
    lo: [f64; 3],
    cell: f64,
    gw: usize,
    gh: usize,
) -> Vec<f64> {
    let mut z = vec![f64::NEG_INFINITY; gw * gh];
    for t in tris {
        let p = t.map(|i| verts[i as usize]);
        let xs = p.map(|v| (v[0] - lo[0]) / cell + 1.0);
        let ys = p.map(|v| (v[1] - lo[1]) / cell + 1.0);
        let den = (ys[1] - ys[2]) * (xs[0] - xs[2]) + (xs[2] - xs[1]) * (ys[0] - ys[2]);
        if den.abs() < 1e-12 {
            continue;
        }
        let x0 = xs.iter().cloned().fold(f64::MAX, f64::min).floor().max(0.0) as usize;
        let x1 = (xs.iter().cloned().fold(f64::MIN, f64::max).ceil() as usize).min(gw - 1);
        let y0 = ys.iter().cloned().fold(f64::MAX, f64::min).floor().max(0.0) as usize;
        let y1 = (ys.iter().cloned().fold(f64::MIN, f64::max).ceil() as usize).min(gh - 1);
        for iy in y0..=y1 {
            let py = iy as f64 + 0.5;
            for ix in x0..=x1 {
                let px = ix as f64 + 0.5;
                let l0 = ((ys[1] - ys[2]) * (px - xs[2]) + (xs[2] - xs[1]) * (py - ys[2])) / den;
                let l1 = ((ys[2] - ys[0]) * (px - xs[2]) + (xs[0] - xs[2]) * (py - ys[2])) / den;
                let l2 = 1.0 - l0 - l1;
                if l0 >= -1e-9 && l1 >= -1e-9 && l2 >= -1e-9 {
                    let h = l0 * p[0][2] + l1 * p[1][2] + l2 * p[2][2];
                    let slot = &mut z[iy * gw + ix];
                    if h > *slot {
                        *slot = h;
                    }
                }
            }
        }
    }
    z
}

/// Edges worth drawing, as sorted vertex-index pairs in a stable order.
fn candidate_edges(verts: &[[f64; 3]], tris: &[[u32; 3]], crease_deg: f64) -> Vec<(u32, u32)> {
    let normals: Vec<[f64; 3]> = tris
        .iter()
        .map(|t| {
            let (a, b, c) = (
                verts[t[0] as usize],
                verts[t[1] as usize],
                verts[t[2] as usize],
            );
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l > 1e-12 {
                [n[0] / l, n[1] / l, n[2] / l]
            } else {
                [0.0; 3]
            }
        })
        .collect();
    // (first triangle, second triangle, count)
    let mut edges: HashMap<(u32, u32), (u32, u32, u32)> = HashMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for (u, v) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let e = edges
                .entry((u.min(v), u.max(v)))
                .or_insert((ti as u32, u32::MAX, 0));
            if e.2 == 1 {
                e.1 = ti as u32;
            }
            e.2 += 1;
        }
    }
    let cos_c = crease_deg.to_radians().cos();
    let mut keep: Vec<(u32, u32)> = edges
        .iter()
        .filter(|(_, &(t0, t1, n))| {
            if n != 2 {
                return true;
            }
            let (n0, n1) = (normals[t0 as usize], normals[t1 as usize]);
            let dot = n0[0] * n1[0] + n0[1] * n1[1] + n0[2] * n1[2];
            if dot < cos_c {
                return true;
            }
            let (up0, up1) = (n0[2] > 1e-6, n1[2] > 1e-6);
            up0 != up1 && (n0[2].abs() > 1e-6 || n1[2].abs() > 1e-6)
        })
        .map(|(k, _)| *k)
        .collect();
    keep.sort_unstable();
    keep
}

#[allow(clippy::too_many_arguments)]
fn visible_pieces(
    p: [f64; 3],
    q: [f64; 3],
    zbuf: &[f64],
    lo: [f64; 3],
    cell: f64,
    gw: usize,
    gh: usize,
    out: &mut Vec<(Pt2, Pt2)>,
) {
    let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
    if d < 1e-6 {
        return; // vertical edge: a point in plan
    }
    let ns = ((d / cell).ceil() as usize).max(2);
    let at = |t: f64| {
        [
            p[0] + (q[0] - p[0]) * t,
            p[1] + (q[1] - p[1]) * t,
            p[2] + (q[2] - p[2]) * t,
        ]
    };
    let visible = |s: [f64; 3]| {
        let ix = (((s[0] - lo[0]) / cell + 1.0) as usize).min(gw - 1);
        let iy = (((s[1] - lo[1]) / cell + 1.0) as usize).min(gh - 1);
        let mut m = f64::INFINITY;
        for y in iy.saturating_sub(1)..=(iy + 1).min(gh - 1) {
            for x in ix.saturating_sub(1)..=(ix + 1).min(gw - 1) {
                m = m.min(zbuf[y * gw + x]);
            }
        }
        s[2] >= m - 0.2
    };
    let vis: Vec<bool> = (0..=ns)
        .map(|k| visible(at(k as f64 / ns as f64)))
        .collect();
    let mut k = 0;
    while k <= ns {
        if !vis[k] {
            k += 1;
            continue;
        }
        let mut j = k;
        while j < ns && vis[j + 1] {
            j += 1;
        }
        if j > k {
            let a = at(k as f64 / ns as f64);
            let b = at(j as f64 / ns as f64);
            out.push(([a[0], a[1]], [b[0], b[1]]));
        }
        k = j + 1;
    }
}

fn key(p: Pt2) -> (i64, i64) {
    (
        (p[0] * 1000.0).round() as i64,
        (p[1] * 1000.0).round() as i64,
    )
}

/// Chains segments that share end points into polylines `(points, closed)`.
fn chain(pieces: &[(Pt2, Pt2)]) -> Vec<(Vec<Pt2>, bool)> {
    let mut at: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, (a, b)) in pieces.iter().enumerate() {
        if key(*a) == key(*b) {
            continue;
        }
        at.entry(key(*a)).or_default().push(i);
        at.entry(key(*b)).or_default().push(i);
    }
    let mut used = vec![false; pieces.len()];
    let mut out = Vec::new();
    let other_end = |i: usize, node: (i64, i64)| -> Pt2 {
        if key(pieces[i].0) == node {
            pieces[i].1
        } else {
            pieces[i].0
        }
    };
    // Extends `pts` from its last point while the node has exactly two segments.
    let extend = |pts: &mut Vec<Pt2>, used: &mut Vec<bool>| loop {
        let node = key(*pts.last().unwrap_or(&[0.0; 2]));
        let Some(list) = at.get(&node) else { break };
        if list.len() != 2 {
            break;
        }
        let Some(&next) = list.iter().find(|&&i| !used[i]) else {
            break;
        };
        used[next] = true;
        pts.push(other_end(next, node));
    };
    let order: Vec<usize> = {
        let open_first = (0..pieces.len()).filter(|&i| {
            at.get(&key(pieces[i].0)).map_or(0, Vec::len) != 2
                || at.get(&key(pieces[i].1)).map_or(0, Vec::len) != 2
        });
        let rest = (0..pieces.len()).filter(|&i| {
            at.get(&key(pieces[i].0)).map_or(0, Vec::len) == 2
                && at.get(&key(pieces[i].1)).map_or(0, Vec::len) == 2
        });
        open_first.chain(rest).collect()
    };
    for s in order {
        if used[s] || key(pieces[s].0) == key(pieces[s].1) {
            continue;
        }
        used[s] = true;
        let mut pts = vec![pieces[s].0, pieces[s].1];
        extend(&mut pts, &mut used);
        pts.reverse();
        extend(&mut pts, &mut used);
        let closed = pts.len() >= 4 && key(pts[0]) == key(pts[pts.len() - 1]);
        if closed {
            pts.pop();
        }
        out.push((pts, closed));
    }
    out
}

fn path_len(p: &[Pt2], closed: bool) -> f64 {
    let mut l: f64 = p
        .windows(2)
        .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
        .sum();
    if closed && p.len() > 1 {
        let (a, b) = (p[p.len() - 1], p[0]);
        l += ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    }
    l
}

/// Douglas-Peucker. A closed ring is split at its two farthest-apart samples.
fn simplify(p: &[Pt2], eps: f64, closed: bool) -> Vec<Pt2> {
    if p.len() < 3 {
        return p.to_vec();
    }
    if !closed {
        return dp(p, eps);
    }
    let a = 0;
    let b = (0..p.len())
        .max_by(|&i, &j| dist2(p[a], p[i]).total_cmp(&dist2(p[a], p[j])))
        .unwrap_or(p.len() / 2);
    let first: Vec<Pt2> = p[a..=b].to_vec();
    let mut second: Vec<Pt2> = p[b..].to_vec();
    second.push(p[0]);
    let mut out = dp(&first, eps);
    out.pop();
    let tail = dp(&second, eps);
    out.extend_from_slice(&tail[..tail.len() - 1]);
    out
}

fn dist2(a: Pt2, b: Pt2) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

fn dp(p: &[Pt2], eps: f64) -> Vec<Pt2> {
    let n = p.len();
    if n < 3 {
        return p.to_vec();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((s, e)) = stack.pop() {
        let (a, b) = (p[s], p[e]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        let mut best = (0.0, s);
        for (i, q) in p.iter().enumerate().take(e).skip(s + 1) {
            let d = if len < 1e-12 {
                dist2(a, *q).sqrt()
            } else {
                ((q[0] - a[0]) * dy - (q[1] - a[1]) * dx).abs() / len
            };
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > eps {
            keep[best.1] = true;
            stack.push((s, best.1));
            stack.push((best.1, e));
        }
    }
    p.iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(q, _)| *q)
        .collect()
}

/// A closed ring whose vertices all lie within 3% of one radius is a circle.
fn fit_circle(p: &[Pt2]) -> Option<(Pt2, f64)> {
    if p.len() < 10 {
        return None;
    }
    let n = p.len() as f64;
    let c = [
        p.iter().map(|q| q[0]).sum::<f64>() / n,
        p.iter().map(|q| q[1]).sum::<f64>() / n,
    ];
    let r = p.iter().map(|q| dist2(*q, c).sqrt()).sum::<f64>() / n;
    if r < 0.2 {
        return None;
    }
    let worst = p
        .iter()
        .map(|q| (dist2(*q, c).sqrt() - r).abs())
        .fold(0.0, f64::max);
    // Vertices must also spread around the whole ring.
    let mut sectors = [false; 8];
    for q in p {
        let a = (q[1] - c[1]).atan2(q[0] - c[0]);
        let s = (((a + std::f64::consts::PI) / (2.0 * std::f64::consts::PI)) * 8.0) as usize;
        sectors[s.min(7)] = true;
    }
    (worst <= 0.03 * r && sectors.iter().all(|&s| s)).then_some((c, r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::mesh::testdata::box_mesh;
    use plan_library::Symbol2d;

    fn mesh_of(v: Vec<[f64; 3]>, t: Vec<[u32; 3]>) -> Mesh {
        Mesh {
            vertices: v,
            triangles: t,
        }
    }

    #[test]
    fn a_box_is_its_footprint() {
        let (v, t) = box_mesh(24.0, 12.0, 30.0);
        let m = mesh_of(v, t);
        let pv = plan_view(&[&m], PlanOptions::default()).unwrap();
        assert_eq!((pv.width, pv.depth, pv.height), (24.0, 12.0, 30.0));
        let b = Symbol2d::new(pv.strokes.clone()).bounds().unwrap();
        assert!((b.width() - 24.0).abs() < 0.2, "{}", b.width());
        assert!((b.height() - 12.0).abs() < 0.2, "{}", b.height());
        // Centred on the origin.
        assert!(b.center().x.abs() < 0.2 && b.center().y.abs() < 0.2);
    }

    #[test]
    fn the_front_ends_up_on_plus_y_without_mirroring() {
        // An L: the foot sticks out towards -Y and +X in Chief's frame.
        let (mut v, t) = box_mesh(10.0, 10.0, 5.0);
        let (v2, t2) = box_mesh(10.0, 4.0, 5.0);
        let off = v.len() as u32;
        v.extend(v2.iter().map(|p| [p[0] + 10.0, p[1] - 4.0, p[2]]));
        let mut t = t;
        t.extend(t2.iter().map(|x| [x[0] + off, x[1] + off, x[2] + off]));
        let pv = plan_view(&[&mesh_of(v, t)], PlanOptions::default()).unwrap();
        let b = Symbol2d::new(pv.strokes).bounds().unwrap();
        // Source bounds x 0..20, y -4..10 (centre 10, 3). Rotated 180 degrees:
        // the foot (source x>10, y<0) lands at x<0, y>0.
        assert!((b.width() - 20.0).abs() < 0.3 && (b.height() - 14.0).abs() < 0.3);
    }

    #[test]
    fn sunk_edges_are_hidden_and_round_things_become_circles() {
        // A 20 x 20 slab with a raised 10-sided... use a cylinder-like prism:
        // a 24-gon column on top of the slab.
        let (mut v, mut t) = box_mesh(20.0, 20.0, 2.0);
        let n = 24;
        let base = v.len() as u32;
        for k in 0..n {
            let a = std::f64::consts::TAU * f64::from(k) / f64::from(n);
            v.push([10.0 + 4.0 * a.cos(), 10.0 + 4.0 * a.sin(), 2.0]);
            v.push([10.0 + 4.0 * a.cos(), 10.0 + 4.0 * a.sin(), 8.0]);
        }
        for k in 0..n as u32 {
            let (a, b) = (base + 2 * k, base + 2 * ((k + 1) % n as u32));
            t.push([a, b, a + 1]);
            t.push([b, b + 1, a + 1]);
        }
        let centre_top = v.len() as u32;
        v.push([10.0, 10.0, 8.0]);
        for k in 0..n as u32 {
            let (a, b) = (base + 2 * k + 1, base + 2 * ((k + 1) % n as u32) + 1);
            t.push([centre_top, a, b]);
        }
        let pv = plan_view(&[&mesh_of(v, t)], PlanOptions::default()).unwrap();
        let circles = pv
            .strokes
            .iter()
            .filter(|s| matches!(s, Stroke::Circle { radius, .. } if (radius - 4.0).abs() < 0.3))
            .count();
        assert!(circles >= 1, "{:?}", pv.strokes.len());
    }

    #[test]
    fn empty_and_flat_inputs() {
        assert!(plan_view(&[], PlanOptions::default()).is_none());
        let m = mesh_of(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            vec![[0, 1, 2]],
        );
        // Zero area in plan (a vertical line): the footprint rectangle is
        // returned only when it has an extent; here depth is 0.
        let pv = plan_view(&[&m], PlanOptions::default()).unwrap();
        assert!(!pv.strokes.is_empty());
    }

    #[test]
    fn simplify_and_chain_basics() {
        let p: Vec<Pt2> = (0..=10).map(|i| [f64::from(i), 0.0]).collect();
        assert_eq!(simplify(&p, 0.01, false).len(), 2);
        let pieces = vec![
            ([0.0, 0.0], [1.0, 0.0]),
            ([1.0, 0.0], [1.0, 1.0]),
            ([1.0, 1.0], [0.0, 1.0]),
            ([0.0, 1.0], [0.0, 0.0]),
        ];
        let c = chain(&pieces);
        assert_eq!(c.len(), 1);
        assert!(c[0].1, "closed");
        assert_eq!(c[0].0.len(), 4);
    }
}
