//! Bounding-volume hierarchy over triangles (binned SAH, median fallback).

use crate::vec3::V3;

/// Triangles per leaf the builder aims for.
const LEAF_SIZE: usize = 4;
/// SAH bins along the split axis.
const BINS: usize = 12;
/// Hard depth cap so the traversal stack can never overflow.
const MAX_DEPTH: usize = 60;
/// Traversal stack size (comfortably above [`MAX_DEPTH`]).
const STACK: usize = 96;
/// Nearest accepted hit distance, inches.
const T_MIN: f32 = 1e-4;

/// A triangle stored as a corner plus two edges, tagged with its material.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tri {
    pub v0: V3,
    pub e1: V3,
    pub e2: V3,
    /// Index into [`plan_3d::Material::ALL`].
    pub material: u32,
}

impl Tri {
    /// `None` for degenerate (zero-area or non-finite) triangles.
    pub fn new(a: V3, b: V3, c: V3, material: u32) -> Option<Tri> {
        let tri = Tri {
            v0: a,
            e1: b - a,
            e2: c - a,
            material,
        };
        let area2 = tri.e1.cross(tri.e2).length();
        (area2.is_finite() && area2 > 1e-9).then_some(tri)
    }

    /// Unit geometric normal (winding order).
    pub fn normal(&self) -> V3 {
        self.e1.cross(self.e2).normalized()
    }

    fn bounds(&self) -> (V3, V3) {
        let b = self.v0 + self.e1;
        let c = self.v0 + self.e2;
        let lo = V3::new(
            self.v0.x.min(b.x).min(c.x),
            self.v0.y.min(b.y).min(c.y),
            self.v0.z.min(b.z).min(c.z),
        );
        let hi = V3::new(
            self.v0.x.max(b.x).max(c.x),
            self.v0.y.max(b.y).max(c.y),
            self.v0.z.max(b.z).max(c.z),
        );
        (lo, hi)
    }

    /// Moller-Trumbore; returns the hit distance within `(T_MIN, tmax)`.
    pub fn intersect(&self, o: V3, d: V3, tmax: f32) -> Option<f32> {
        let p = d.cross(self.e2);
        let det = self.e1.dot(p);
        if det.abs() < 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        let s = o - self.v0;
        let u = s.dot(p) * inv;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let q = s.cross(self.e1);
        let v = d.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let t = self.e2.dot(q) * inv;
        (t > T_MIN && t < tmax).then_some(t)
    }
}

/// Closest-hit record.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hit {
    pub t: f32,
    /// Index into [`Bvh::tris`].
    pub tri: usize,
}

#[derive(Clone, Copy, Debug)]
struct Node {
    lo: V3,
    hi: V3,
    /// Leaf: first triangle. Interior: right child (left child is `self + 1`).
    offset: u32,
    /// Triangle count; zero for interior nodes.
    count: u32,
    axis: u32,
}

impl Node {
    fn hit_box(&self, o: V3, inv: V3, tmax: f32) -> bool {
        let mut near = 0.0_f32;
        let mut far = tmax;
        for k in 0..3 {
            let a = (self.lo[k] - o[k]) * inv[k];
            let b = (self.hi[k] - o[k]) * inv[k];
            let (a, b) = if a < b { (a, b) } else { (b, a) };
            // `max`/`min` ignore NaN, so a ray lying in a slab plane still passes.
            near = near.max(a);
            far = far.min(b * (1.0 + 1e-6) + 1e-6);
        }
        near <= far
    }
}

/// Build-time primitive: bounds, centroid and original index.
#[derive(Clone, Copy)]
struct Prim {
    lo: V3,
    hi: V3,
    c: V3,
    idx: u32,
}

/// A static BVH over a reordered triangle list.
#[derive(Debug, Default)]
pub(crate) struct Bvh {
    nodes: Vec<Node>,
    /// Triangles in leaf order; [`Hit::tri`] indexes this.
    pub tris: Vec<Tri>,
}

impl Bvh {
    pub fn build(tris: Vec<Tri>) -> Bvh {
        let mut prims: Vec<Prim> = tris
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let (lo, hi) = t.bounds();
                Prim {
                    lo,
                    hi,
                    c: (lo + hi) * 0.5,
                    idx: i as u32,
                }
            })
            .collect();
        let mut nodes = Vec::with_capacity(tris.len() / 2 + 1);
        if !prims.is_empty() {
            build_node(&mut prims, 0, 0, &mut nodes);
        }
        let ordered = prims.iter().map(|p| tris[p.idx as usize]).collect();
        Bvh {
            nodes,
            tris: ordered,
        }
    }

    /// Nearest hit with distance below `tmax`.
    pub fn closest(&self, o: V3, d: V3, tmax: f32) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = V3::new(1.0 / d.x, 1.0 / d.y, 1.0 / d.z);
        let mut stack = [0_u32; STACK];
        let mut sp = 0;
        let mut cur = 0_usize;
        let mut best = tmax;
        let mut hit = None;
        loop {
            let node = &self.nodes[cur];
            if node.hit_box(o, inv, best) {
                if node.count > 0 {
                    let first = node.offset as usize;
                    for i in first..first + node.count as usize {
                        if let Some(t) = self.tris[i].intersect(o, d, best) {
                            best = t;
                            hit = Some(Hit { t, tri: i });
                        }
                    }
                } else {
                    let (near, far) = if d[node.axis as usize] < 0.0 {
                        (node.offset as usize, cur + 1)
                    } else {
                        (cur + 1, node.offset as usize)
                    };
                    stack[sp] = far as u32;
                    sp += 1;
                    cur = near;
                    continue;
                }
            }
            if sp == 0 {
                return hit;
            }
            sp -= 1;
            cur = stack[sp] as usize;
        }
    }
}

fn surface_area(lo: V3, hi: V3) -> f32 {
    let e = hi - lo;
    2.0 * (e.x * e.y + e.y * e.z + e.z * e.x)
}

fn grow(lo: &mut V3, hi: &mut V3, plo: V3, phi: V3) {
    *lo = V3::new(lo.x.min(plo.x), lo.y.min(plo.y), lo.z.min(plo.z));
    *hi = V3::new(hi.x.max(phi.x), hi.y.max(phi.y), hi.z.max(phi.z));
}

const EMPTY_LO: V3 = V3::splat(f32::INFINITY);
const EMPTY_HI: V3 = V3::splat(f32::NEG_INFINITY);

/// Recursively build the subtree over `prims`; returns its node index.
fn build_node(prims: &mut [Prim], base: usize, depth: usize, nodes: &mut Vec<Node>) -> usize {
    let me = nodes.len();
    let (mut lo, mut hi) = (EMPTY_LO, EMPTY_HI);
    let (mut clo, mut chi) = (EMPTY_LO, EMPTY_HI);
    for p in prims.iter() {
        grow(&mut lo, &mut hi, p.lo, p.hi);
        grow(&mut clo, &mut chi, p.c, p.c);
    }
    nodes.push(Node {
        lo,
        hi,
        offset: base as u32,
        count: prims.len() as u32,
        axis: 0,
    });
    let n = prims.len();
    let ext = chi - clo;
    let axis = if ext.x >= ext.y && ext.x >= ext.z {
        0
    } else if ext.y >= ext.z {
        1
    } else {
        2
    };
    if n == 1 || ext[axis] <= 0.0 || depth >= MAX_DEPTH {
        return me; // leaf
    }
    let mid = match sah_split(prims, axis, clo[axis], ext[axis], surface_area(lo, hi)) {
        Split::Leaf => return me,
        Split::At(m) if m > 0 && m < n => m,
        _ => {
            prims.select_nth_unstable_by(n / 2, |a, b| {
                a.c[axis]
                    .partial_cmp(&b.c[axis])
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            n / 2
        }
    };
    let (left, right) = prims.split_at_mut(mid);
    build_node(left, base, depth + 1, nodes);
    let right_idx = build_node(right, base + mid, depth + 1, nodes);
    let node = &mut nodes[me];
    node.offset = right_idx as u32;
    node.count = 0;
    node.axis = axis as u32;
    me
}

enum Split {
    Leaf,
    /// Partitioned in place; the right half starts at this index.
    At(usize),
}

/// Binned surface-area-heuristic split, partitioning `prims` in place.
fn sah_split(prims: &mut [Prim], axis: usize, cmin: f32, cext: f32, parent_area: f32) -> Split {
    let n = prims.len();
    let bin_of = |p: &Prim| (((p.c[axis] - cmin) / cext * BINS as f32) as usize).min(BINS - 1);
    let mut count = [0_usize; BINS];
    let mut blo = [EMPTY_LO; BINS];
    let mut bhi = [EMPTY_HI; BINS];
    for p in prims.iter() {
        let b = bin_of(p);
        count[b] += 1;
        grow(&mut blo[b], &mut bhi[b], p.lo, p.hi);
    }
    // Sweep from the right to cache suffix areas, then from the left.
    let mut right_cost = [0.0_f32; BINS];
    let (mut lo, mut hi, mut cnt) = (EMPTY_LO, EMPTY_HI, 0_usize);
    for b in (1..BINS).rev() {
        grow(&mut lo, &mut hi, blo[b], bhi[b]);
        cnt += count[b];
        right_cost[b - 1] = if cnt > 0 {
            cnt as f32 * surface_area(lo, hi)
        } else {
            0.0
        };
    }
    let (mut lo, mut hi, mut cnt) = (EMPTY_LO, EMPTY_HI, 0_usize);
    let mut best = (f32::INFINITY, 0_usize);
    for b in 0..BINS - 1 {
        grow(&mut lo, &mut hi, blo[b], bhi[b]);
        cnt += count[b];
        let left_cost = if cnt > 0 {
            cnt as f32 * surface_area(lo, hi)
        } else {
            0.0
        };
        let cost = 0.125 + (left_cost + right_cost[b]) / parent_area.max(1e-12);
        if cost < best.0 {
            best = (cost, b);
        }
    }
    if n <= LEAF_SIZE && best.0 >= n as f32 {
        return Split::Leaf;
    }
    // In-place partition: bins <= best.1 go left.
    let (mut i, mut j) = (0, n);
    while i < j {
        if bin_of(&prims[i]) <= best.1 {
            i += 1;
        } else {
            j -= 1;
            prims.swap(i, j);
        }
    }
    Split::At(i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    /// Two perpendicular walls, each a `nx` x `ny` grid of quads (two triangles).
    fn two_walls(nx: usize, ny: usize) -> Vec<Tri> {
        let mut tris = Vec::new();
        let mut quad = |a: V3, b: V3, c: V3, d: V3| {
            tris.extend(Tri::new(a, b, c, 0));
            tris.extend(Tri::new(a, c, d, 1));
        };
        for i in 0..nx {
            for j in 0..ny {
                let (x0, x1) = (
                    240.0 * i as f32 / nx as f32,
                    240.0 * (i + 1) as f32 / nx as f32,
                );
                let (y0, y1) = (
                    96.0 * j as f32 / ny as f32,
                    96.0 * (j + 1) as f32 / ny as f32,
                );
                // Wall along X at z = 0.
                quad(
                    V3::new(x0, y0, 0.0),
                    V3::new(x1, y0, 0.0),
                    V3::new(x1, y1, 0.0),
                    V3::new(x0, y1, 0.0),
                );
                // Wall along Z at x = 240.
                quad(
                    V3::new(240.0, y0, x0),
                    V3::new(240.0, y0, x1),
                    V3::new(240.0, y1, x1),
                    V3::new(240.0, y1, x0),
                );
            }
        }
        tris
    }

    #[test]
    fn bvh_matches_brute_force_on_random_rays() {
        let bvh = Bvh::build(two_walls(8, 4));
        assert_eq!(bvh.tris.len(), 8 * 4 * 2 * 2);
        let mut rng = Rng::new(42);
        let mut hits = 0;
        for _ in 0..200 {
            let o = V3::new(
                rng.next_f32() * 240.0,
                rng.next_f32() * 96.0,
                rng.next_f32() * 240.0 + 1.0,
            );
            let d = V3::new(
                rng.next_f32() * 2.0 - 1.0,
                rng.next_f32() * 2.0 - 1.0,
                rng.next_f32() * 2.0 - 1.0,
            )
            .normalized();
            let brute = bvh
                .tris
                .iter()
                .filter_map(|t| t.intersect(o, d, f32::INFINITY))
                .fold(f32::INFINITY, f32::min);
            match bvh.closest(o, d, f32::INFINITY) {
                Some(h) => {
                    hits += 1;
                    assert_eq!(h.t, brute);
                }
                None => assert!(brute.is_infinite(), "BVH missed a hit at t={brute}"),
            }
        }
        assert!(hits > 30, "only {hits} of 200 rays hit anything");
    }

    #[test]
    fn empty_bvh_never_hits() {
        let bvh = Bvh::build(Vec::new());
        assert!(bvh
            .closest(V3::ZERO, V3::new(0.0, 0.0, 1.0), f32::INFINITY)
            .is_none());
    }
}
