//! Software hidden-line removal: triangles in, visible weighted segments out.
//!
//! Pipeline: transform to view space (clipping at the section plane when
//! cutting) -> weld vertices and split T-junctions -> classify edges ->
//! rasterize a depth buffer -> sample every candidate edge against it.

use crate::drawing::{Drawing, EdgeKind, Line2, LineWeight, RegionKind};
use crate::projection::Projection;
use crate::regions::Labels;
use crate::shadow::Surface;
use crate::styles::ObjectWeights;
use crate::Options;
use plan_3d::{Material, Scene};
use plan_core::{Id, Point};
use std::collections::HashMap;

/// A view-space vector: `[u, v, depth]`, depth growing toward the viewer.
pub(crate) type V3 = [f64; 3];

/// Material and source object of a triangle: a change across an edge draws a seam.
type Group = (Material, Option<Id>);

/// Vertices closer than this (inches) are welded.
const WELD: f64 = 0.01;
/// A vertex within this distance of an edge splits it (T-junction repair).
const SPLIT_TOL: f64 = 0.02;
/// Points this close (inches) to the cut plane count as on it.
const PLANE_EPS: f64 = 1e-4;
/// Smallest normalised facing component that counts as front-facing.
const FRONT_EPS: f64 = 1e-4;
/// Runs shorter than this many pixels are dropped or bridged.
const MIN_RUN_PX: f64 = 2.0;
/// Empty border around the depth buffer, pixels.
pub(crate) const MARGIN_PX: f64 = 2.0;
/// A cut face wins depth ties within this many inches (f32 rounding of coplanar faces).
const CUT_TIE_EPS: f32 = 1e-3;
/// Grid cells per axis for the T-junction vertex index.
const GRID_CELLS: f64 = 24.0;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn lerp(a: V3, b: V3, t: f64) -> V3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn xy(p: V3) -> Point {
    Point::new(p[0], p[1])
}

/// A view-space triangle with its facing.
pub(crate) struct Tri {
    pub p: [V3; 3],
    /// Unit normal from the winding (outward for the solids plan-3d builds).
    n: V3,
    front: bool,
    group: Group,
}

/// Kept triangles plus, for a section, the oriented cut segments (closed
/// loops around each solid, consistently wound so they can be filled), each
/// tagged with the group of the mesh it cuts.
struct Clipped {
    tris: Vec<Tri>,
    cut: Vec<(Point, Point, Group)>,
}

fn push_tri(tris: &mut Vec<Tri>, p: [V3; 3], group: Group) {
    let c = cross(sub(p[1], p[0]), sub(p[2], p[0]));
    let len = dot(c, c).sqrt();
    if len < 1e-6 {
        return;
    }
    let n = [c[0] / len, c[1] / len, c[2] / len];
    tris.push(Tri {
        p,
        n,
        front: n[2] > FRONT_EPS,
        group,
    });
}

/// Transform the scene into view space, keeping depth `<= cut_depth` when
/// cutting and, with `section_depth`, depth `>= cut_depth - section_depth`.
fn collect(
    scene: &Scene,
    proj: &Projection,
    cut_depth: Option<f64>,
    section_depth: Option<f64>,
) -> Clipped {
    let mut out = Clipped {
        tris: Vec::new(),
        cut: Vec::new(),
    };
    let far = cut_depth
        .zip(section_depth)
        .map(|(dc, sd)| dc - sd.max(0.0));
    for mesh in &scene.meshes {
        let group = (mesh.material, mesh.object_id);
        let view = |i: u32| {
            let (pt, d) = proj.project(mesh.vertices[i as usize].position.map(f64::from));
            [pt.x, pt.y, d]
        };
        for t in mesh.indices.chunks(3).filter(|t| t.len() == 3) {
            let p = [view(t[0]), view(t[1]), view(t[2])];
            match cut_depth {
                None => push_tri(&mut out.tris, p, group),
                Some(dc) => clip_tri(p, dc, far, group, &mut out),
            }
        }
    }
    out
}

/// Clip one triangle to the slab `far <= depth <= dc` (Sutherland-Hodgman).
/// Only the near plane produces cut segments.
fn clip_tri(p: [V3; 3], dc: f64, far: Option<f64>, group: Group, out: &mut Clipped) {
    let f = p.map(|v| v[2] - dc);
    let mut poly: Vec<V3> = Vec::with_capacity(5);
    if f.iter().all(|&x| x <= PLANE_EPS) {
        poly.extend_from_slice(&p);
    } else if f.iter().all(|&x| x > PLANE_EPS) {
        return;
    } else {
        let (mut exit, mut entry) = (None, None);
        for i in 0..3 {
            let (a, b) = (p[i], p[(i + 1) % 3]);
            let (fa, fb) = (f[i], f[(i + 1) % 3]);
            let (in_a, in_b) = (fa <= PLANE_EPS, fb <= PLANE_EPS);
            if in_a {
                poly.push(a);
            }
            if in_a != in_b {
                let mut x = lerp(a, b, fa / (fa - fb));
                x[2] = dc;
                poly.push(x);
                if in_a {
                    exit = Some(x);
                } else {
                    entry = Some(x);
                }
            }
        }
        if let (Some(x0), Some(x1)) = (exit, entry) {
            if xy(x0).dist(xy(x1)) > 1e-6 {
                out.cut.push((xy(x0), xy(x1), group));
            }
        }
    }
    if let Some(df) = far {
        poly = clip_far(&poly, df);
    }
    for k in 1..poly.len().saturating_sub(1) {
        push_tri(&mut out.tris, [poly[0], poly[k], poly[k + 1]], group);
    }
}

/// Keep the part of a convex polygon with `depth >= df`.
fn clip_far(poly: &[V3], df: f64) -> Vec<V3> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (fa, fb) = (a[2] - df, b[2] - df);
        let (in_a, in_b) = (fa >= -PLANE_EPS, fb >= -PLANE_EPS);
        if in_a {
            out.push(a);
        }
        if in_a != in_b {
            let mut x = lerp(a, b, fa / (fa - fb));
            x[2] = df;
            out.push(x);
        }
    }
    out
}

/// An edge that may be drawn, with how it should look.
struct Candidate {
    a: V3,
    b: V3,
    weight: LineWeight,
    kind: EdgeKind,
    /// The wall or opening the edge belongs to (a front-facing neighbour's
    /// source object), for the per-object pen weights.
    object: Option<Id>,
}

/// Uniform grid over welded vertices, for T-junction queries.
struct VertexGrid {
    cell: f64,
    origin: V3,
    cells: HashMap<[i32; 3], Vec<u32>>,
}

impl VertexGrid {
    fn new(pts: &[V3]) -> VertexGrid {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in pts {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let span = (0..3).map(|k| hi[k] - lo[k]).fold(1.0, f64::max);
        let mut grid = VertexGrid {
            cell: (span / GRID_CELLS).max(1.0),
            origin: lo,
            cells: HashMap::new(),
        };
        for (i, p) in pts.iter().enumerate() {
            grid.cells.entry(grid.key(*p)).or_default().push(i as u32);
        }
        grid
    }

    fn key(&self, p: V3) -> [i32; 3] {
        [0, 1, 2].map(|k| ((p[k] - self.origin[k]) / self.cell).floor() as i32)
    }

    /// Vertices in cells overlapping the box around segment `a`-`b`.
    fn near(&self, a: V3, b: V3) -> Vec<u32> {
        let lo = self.key([0, 1, 2].map(|k| a[k].min(b[k]) - SPLIT_TOL));
        let hi = self.key([0, 1, 2].map(|k| a[k].max(b[k]) + SPLIT_TOL));
        let mut found = Vec::new();
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    if let Some(v) = self.cells.get(&[x, y, z]) {
                        found.extend_from_slice(v);
                    }
                }
            }
        }
        found
    }
}

/// Weld vertices by position; returns the welded points and per-triangle ids.
fn weld(tris: &[Tri]) -> (Vec<V3>, Vec<Option<[u32; 3]>>) {
    let mut map: HashMap<[i64; 3], u32> = HashMap::new();
    let mut pts: Vec<V3> = Vec::new();
    let ids = tris
        .iter()
        .map(|t| {
            let id = t.p.map(|p| {
                let key = p.map(|c| (c / WELD).round() as i64);
                *map.entry(key).or_insert_with(|| {
                    pts.push(p);
                    (pts.len() - 1) as u32
                })
            });
            (id[0] != id[1] && id[1] != id[2] && id[0] != id[2]).then_some(id)
        })
        .collect();
    (pts, ids)
}

fn edge_key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// Chain of vertex ids from `a` to `b` through every welded vertex lying on the segment.
fn split_chain(pts: &[V3], grid: &VertexGrid, a: u32, b: u32) -> Vec<u32> {
    let (pa, pb) = (pts[a as usize], pts[b as usize]);
    let ab = sub(pb, pa);
    let len = dot(ab, ab).sqrt();
    let mut mids: Vec<(f64, u32)> = Vec::new();
    for c in grid.near(pa, pb) {
        if c == a || c == b {
            continue;
        }
        let ac = sub(pts[c as usize], pa);
        let s = dot(ac, ab) / len;
        if s <= SPLIT_TOL || s >= len - SPLIT_TOL {
            continue;
        }
        let perp = [0, 1, 2].map(|k| ac[k] - ab[k] / len * s);
        if dot(perp, perp).sqrt() <= SPLIT_TOL {
            mids.push((s, c));
        }
    }
    mids.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut chain = vec![a];
    chain.extend(mids.into_iter().map(|m| m.1));
    chain.push(b);
    chain
}

/// Classify the triangles meeting at one edge.
fn classify(edge_tris: &[u32], tris: &[Tri], crease_cos: f64) -> Option<(EdgeKind, LineWeight)> {
    let adj: Vec<&Tri> = edge_tris.iter().map(|&i| &tris[i as usize]).collect();
    let fronts = adj.iter().filter(|t| t.front).count();
    if adj.len() == 1 || (fronts > 0 && fronts < adj.len()) {
        return Some((EdgeKind::Silhouette, LineWeight::Heavy));
    }
    let mut seam = false;
    for (i, p) in adj.iter().enumerate() {
        for q in &adj[i + 1..] {
            if dot(p.n, q.n) < crease_cos {
                return Some((EdgeKind::Crease, LineWeight::Medium));
            }
            seam |= p.group != q.group;
        }
    }
    seam.then_some((EdgeKind::Material, LineWeight::Light))
}

/// The source object of an edge: that of the first front-facing neighbour
/// that has one, else of any neighbour that has one.
fn edge_object(edge_tris: &[u32], tris: &[Tri]) -> Option<Id> {
    let adj = || edge_tris.iter().map(|&i| &tris[i as usize]);
    adj()
        .filter(|t| t.front)
        .find_map(|t| t.group.1)
        .or_else(|| adj().find_map(|t| t.group.1))
}

/// Candidate edges of the kept triangles: silhouettes, creases and seams.
fn candidates(
    tris: &[Tri],
    cut_depth: Option<f64>,
    far: Option<f64>,
    opts: &Options,
) -> Vec<Candidate> {
    let (pts, ids) = weld(tris);
    let grid = VertexGrid::new(&pts);
    let crease_cos = opts.crease_angle_deg.to_radians().cos();

    let mut whole: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for (ti, id) in ids.iter().enumerate() {
        if let Some(id) = id {
            for k in 0..3 {
                whole
                    .entry(edge_key(id[k], id[(k + 1) % 3]))
                    .or_default()
                    .push(ti as u32);
            }
        }
    }
    let mut whole: Vec<_> = whole.into_iter().collect();
    whole.sort_unstable_by_key(|e| e.0);

    let mut parts: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for ((a, b), adj) in &whole {
        let chain = split_chain(&pts, &grid, *a, *b);
        for w in chain.windows(2) {
            let list = parts.entry(edge_key(w[0], w[1])).or_default();
            for &t in adj {
                if !list.contains(&t) {
                    list.push(t);
                }
            }
        }
    }
    let mut parts: Vec<_> = parts.into_iter().collect();
    parts.sort_unstable_by_key(|e| e.0);

    parts
        .into_iter()
        .filter_map(|((a, b), adj)| {
            let (pa, pb) = (pts[a as usize], pts[b as usize]);
            if let Some(dc) = cut_depth {
                if pa[2] >= dc - 1e-3 && pb[2] >= dc - 1e-3 {
                    return None; // lies on the cut plane: drawn as a cut line instead
                }
            }
            if let Some(df) = far {
                if pa[2] <= df + 1e-3 && pb[2] <= df + 1e-3 {
                    return None; // lies on the back clip plane of a limited-depth section
                }
            }
            let (kind, weight) = classify(&adj, tris, crease_cos)?;
            Some(Candidate {
                a: pa,
                b: pb,
                weight,
                kind,
                object: edge_object(&adj, tris),
            })
        })
        .collect()
}

/// Depth buffer over the drawing extent. Larger depth is nearer the viewer.
pub(crate) struct DepthBuffer {
    pub w: usize,
    pub h: usize,
    pub scale: f64,
    pub origin: Point,
    pub data: Vec<f32>,
    /// Label of the surface that owns each pixel's depth; 0 is background.
    pub ids: Vec<u32>,
}

impl DepthBuffer {
    pub fn new(extent: (Point, Point), raster_px: usize) -> DepthBuffer {
        let (lo, hi) = extent;
        let longest = (hi.x - lo.x).max(hi.y - lo.y).max(1e-6);
        let scale = raster_px.max(8) as f64 / longest;
        let dim = |span: f64| (span * scale).ceil() as usize + 2 * MARGIN_PX as usize + 1;
        let (w, h) = (dim(hi.x - lo.x), dim(hi.y - lo.y));
        DepthBuffer {
            w,
            h,
            scale,
            origin: lo,
            data: vec![f32::NEG_INFINITY; w * h],
            ids: vec![0; w * h],
        }
    }

    /// Continuous pixel coordinates of a drawing point.
    pub fn to_px(&self, p: Point) -> (f64, f64) {
        (
            (p.x - self.origin.x) * self.scale + MARGIN_PX,
            (p.y - self.origin.y) * self.scale + MARGIN_PX,
        )
    }

    /// Size of one pixel in drawing units.
    pub fn pixel_size(&self) -> f64 {
        1.0 / self.scale
    }

    pub fn pixel_index(&self, x: f64, y: f64) -> usize {
        let cx = (x.floor().max(0.0) as usize).min(self.w - 1);
        let cy = (y.floor().max(0.0) as usize).min(self.h - 1);
        cy * self.w + cx
    }

    /// Edge-function rasterizer sampling pixel centres.
    pub fn rasterize(&mut self, tri: &[V3; 3], label: u32) {
        let q = tri.map(|p| {
            let (x, y) = self.to_px(xy(p));
            [x, y, p[2]]
        });
        let area =
            (q[1][0] - q[0][0]) * (q[2][1] - q[0][1]) - (q[2][0] - q[0][0]) * (q[1][1] - q[0][1]);
        if area.abs() < 1e-12 {
            return;
        }
        let bound = |k: usize, f: fn(f64, f64) -> f64| q.iter().map(|p| p[k]).fold(q[0][k], f);
        let x0 = ((bound(0, f64::min) - 0.5).floor().max(0.0)) as usize;
        let y0 = ((bound(1, f64::min) - 0.5).floor().max(0.0)) as usize;
        let x1 = (((bound(0, f64::max) - 0.5).ceil().max(0.0)) as usize).min(self.w - 1);
        let y1 = (((bound(1, f64::max) - 0.5).ceil().max(0.0)) as usize).min(self.h - 1);
        let edge = |a: &[f64; 3], b: &[f64; 3], px: f64, py: f64| {
            (b[0] - a[0]) * (py - a[1]) - (b[1] - a[1]) * (px - a[0])
        };
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let l0 = edge(&q[1], &q[2], px, py) / area;
                let l1 = edge(&q[2], &q[0], px, py) / area;
                let l2 = 1.0 - l0 - l1;
                if l0 < -1e-7 || l1 < -1e-7 || l2 < -1e-7 {
                    continue;
                }
                let d = (l0 * q[0][2] + l1 * q[1][2] + l2 * q[2][2]) as f32;
                let i = y * self.w + x;
                if d > self.data[i] {
                    self.data[i] = d;
                    self.ids[i] = label;
                }
            }
        }
    }

    /// Fill one solid's cut face at `depth` using a non-zero winding scanline
    /// fill, labelling the pixels it wins (ties go to the cut face).
    fn fill_cut(&mut self, segments: &[(Point, Point)], depth: f64, label: u32) {
        let mut rows: Vec<Vec<(f64, i32)>> = vec![Vec::new(); self.h];
        for (a, b) in segments {
            let ((x0, y0), (x1, y1)) = (self.to_px(*a), self.to_px(*b));
            if (y1 - y0).abs() < 1e-12 {
                continue;
            }
            let (lo, hi, dir) = if y1 > y0 { (y0, y1, 1) } else { (y1, y0, -1) };
            let first = (lo - 0.5).ceil().max(0.0) as usize;
            let end = ((hi - 0.5).ceil().max(0.0) as usize).min(self.h);
            for (row, cell) in rows.iter_mut().enumerate().take(end).skip(first) {
                let yc = row as f64 + 0.5;
                cell.push((x0 + (yc - y0) / (y1 - y0) * (x1 - x0), dir));
            }
        }
        for (row, crossings) in rows.iter_mut().enumerate() {
            crossings.sort_by(|p, q| p.0.total_cmp(&q.0));
            let mut winding = 0;
            for pair in crossings.windows(2) {
                winding += pair[0].1;
                if winding == 0 {
                    continue;
                }
                let from = (pair[0].0 - 0.5).ceil().max(0.0) as usize;
                let end = ((pair[1].0 - 0.5).ceil().max(0.0) as usize).min(self.w);
                for x in from..end {
                    let i = row * self.w + x;
                    if depth as f32 + CUT_TIE_EPS >= self.data[i] {
                        self.data[i] = self.data[i].max(depth as f32);
                        self.ids[i] = label;
                    }
                }
            }
        }
    }
}

/// Visible (`true`) and hidden (`false`) runs of one edge, as `(visible, t0, t1)`.
fn edge_runs(buf: &DepthBuffer, a: V3, b: V3, bias: f64) -> Vec<(bool, f64, f64)> {
    let len_px = xy(a).dist(xy(b)) * buf.scale;
    if len_px < 1e-6 {
        return Vec::new();
    }
    let n = len_px.ceil().max(1.0) as usize;
    let mut flags: Vec<bool> = (0..n)
        .map(|i| {
            let p = lerp(a, b, (i as f64 + 0.5) / n as f64);
            let (x, y) = buf.to_px(xy(p));
            p[2] >= f64::from(buf.data[buf.pixel_index(x, y)]) - bias
        })
        .collect();
    let px_per_sample = len_px / n as f64;
    let min_samples = (MIN_RUN_PX / px_per_sample).ceil() as usize;

    // Bridge short hidden gaps between visible runs, then collect runs.
    let mut runs = collect_runs(&flags);
    for &(vis, s, e) in runs.iter().take(runs.len().saturating_sub(1)).skip(1) {
        if !vis && e - s < min_samples {
            flags[s..e].iter_mut().for_each(|f| *f = true);
        }
    }
    runs = collect_runs(&flags);
    runs.into_iter()
        .filter(|&(_, s, e)| e - s >= min_samples)
        .map(|(vis, s, e)| (vis, s as f64 / n as f64, e as f64 / n as f64))
        .collect()
}

fn collect_runs(flags: &[bool]) -> Vec<(bool, usize, usize)> {
    let mut runs: Vec<(bool, usize, usize)> = Vec::new();
    for (i, &f) in flags.iter().enumerate() {
        match runs.last_mut() {
            Some(r) if r.0 == f => r.2 = i + 1,
            _ => runs.push((f, i, i + 1)),
        }
    }
    runs
}

/// With `depth_weights`, lines more than this far (inches) behind the nearest drawn line step down a weight.
const DEPTH_BAND: f64 = 12.0;

fn step_down(w: LineWeight) -> LineWeight {
    match w {
        LineWeight::Heavy => LineWeight::Medium,
        _ => LineWeight::Light,
    }
}

/// Run the whole pipeline for one projection, optionally cutting at `cut_depth`.
pub(crate) fn render(
    scene: &Scene,
    proj: &Projection,
    cut_depth: Option<f64>,
    opts: &Options,
    weights: Option<&ObjectWeights>,
) -> Drawing {
    let clipped = collect(scene, proj, cut_depth, opts.section_depth);
    let far = cut_depth
        .zip(opts.section_depth)
        .map(|(dc, sd)| dc - sd.max(0.0));
    let mut buf = DepthBuffer::new(proj.extent(), opts.raster_px);
    let mut labels = Labels::default();
    for t in &clipped.tris {
        if t.front || !opts.cull_backfaces {
            let label = labels.get(t.group.0, t.group.1, RegionKind::Face);
            buf.rasterize(&t.p, label);
        }
    }
    if let Some(dc) = cut_depth {
        // A solid's cut loop is only closed across all its meshes (a wall has
        // one mesh per material), so fill per source object; meshes without
        // an object are separate solids per material.
        type SolidKey = (Option<Id>, Option<usize>);
        type Solid = (SolidKey, Vec<(Point, Point, Material)>);
        let mut solids: Vec<Solid> = Vec::new();
        for &(a, b, (material, object)) in &clipped.cut {
            let key = (object, object.is_none().then(|| material.index()));
            match solids.iter_mut().find(|(k, _)| *k == key) {
                Some((_, segs)) => segs.push((a, b, material)),
                None => solids.push((key, vec![(a, b, material)])),
            }
        }
        for ((object, _), segs) in &solids {
            // Name the cut face after the material with the most cut length.
            let mut length = [0.0_f64; Material::ALL.len()];
            for (a, b, m) in segs {
                length[m.index()] += a.dist(*b);
            }
            let best = (0..length.len()).fold(0, |best, i| {
                if length[i] > length[best] + 1e-6 {
                    i
                } else {
                    best
                }
            });
            let label = labels.get(Material::ALL[best], *object, RegionKind::Cut);
            let outline: Vec<(Point, Point)> = segs.iter().map(|&(a, b, _)| (a, b)).collect();
            buf.fill_cut(&outline, dc, label);
        }
    }
    let bias = 1.5 * buf.pixel_size() + 0.02;

    // Each line carries its mean depth for the optional depth weighting and
    // its source object for the per-object pen weights.
    let mut lines: Vec<(Line2, f64, Option<Id>)> = clipped
        .cut
        .iter()
        .map(|&(a, b, _)| {
            (
                Line2 {
                    a,
                    b,
                    weight: LineWeight::Heavy,
                    kind: EdgeKind::Cut,
                },
                f64::INFINITY,
                None,
            )
        })
        .collect();
    for c in candidates(&clipped.tris, cut_depth, far, opts) {
        for (visible, t0, t1) in edge_runs(&buf, c.a, c.b, bias) {
            let end = |t: f64| match t {
                t if t <= 0.0 => xy(c.a),
                t if t >= 1.0 => xy(c.b),
                t => xy(lerp(c.a, c.b, t)),
            };
            let depth = lerp(c.a, c.b, 0.5 * (t0 + t1))[2];
            if visible {
                lines.push((
                    Line2 {
                        a: end(t0),
                        b: end(t1),
                        weight: c.weight,
                        kind: c.kind,
                    },
                    depth,
                    c.object,
                ));
            } else if opts.include_hidden_dashed {
                lines.push((
                    Line2 {
                        a: end(t0),
                        b: end(t1),
                        weight: LineWeight::Light,
                        kind: EdgeKind::Hidden,
                    },
                    depth,
                    c.object,
                ));
            }
        }
    }
    if opts.depth_weights {
        let nearest = lines
            .iter()
            .filter(|(l, _, _)| l.kind != EdgeKind::Hidden)
            .map(|(_, d, _)| *d)
            .filter(|d| d.is_finite())
            .fold(f64::NEG_INFINITY, f64::max);
        for (l, d, _) in &mut lines {
            if l.kind != EdgeKind::Hidden && *d < nearest - DEPTH_BAND {
                l.weight = step_down(l.weight);
            }
        }
    }
    if let Some(w) = weights {
        for (l, _, object) in &mut lines {
            if let Some(class) = object.and_then(|id| w.class_of(id)) {
                l.weight = w.restyle(l.kind, l.weight, class);
            }
        }
    }
    let mut drawing = Drawing::new(lines.into_iter().map(|(l, _, _)| l).collect());
    drawing.merge_collinear();

    if opts.regions || opts.hatch || opts.shadows.is_some() {
        let to_point = |p: Point| {
            Point::new(
                (p.x - MARGIN_PX) / buf.scale + buf.origin.x,
                (p.y - MARGIN_PX) / buf.scale + buf.origin.y,
            )
        };
        drawing.regions = crate::regions::extract(&buf.ids, buf.w, buf.h, &labels, &to_point);
        if let Some(sun) = opts.shadows {
            let surface = |i: usize| match labels.key(buf.ids[i]) {
                None => Surface::Background,
                Some(k) if k.kind == RegionKind::Cut => Surface::Skip,
                Some(k) => Surface::Material(k.material),
            };
            let shadowed = crate::shadow::shadowed_pixels(
                &clipped.tris,
                &buf,
                proj,
                sun,
                opts.raster_px,
                &surface,
            );
            let mut shadow_labels = Labels::default();
            let mut shadow_ids = vec![0u32; buf.ids.len()];
            for s in shadowed {
                let material = s.material.unwrap_or(Material::Floor);
                shadow_ids[s.index] = shadow_labels.get(material, None, RegionKind::Shadow);
            }
            drawing.regions.extend(crate::regions::extract(
                &shadow_ids,
                buf.w,
                buf.h,
                &shadow_labels,
                &to_point,
            ));
        }
    }
    if opts.hatch {
        drawing.lines.extend(crate::hatch::hatch_lines(
            &drawing.regions,
            opts.hatch_scale,
        ));
    }
    drawing
}
