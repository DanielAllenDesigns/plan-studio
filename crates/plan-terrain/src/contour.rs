//! Contour lines by marching triangles.

use std::collections::{BTreeMap, HashMap, HashSet};

use plan_core::Point;

use crate::model::TerrainSurface;

/// Endpoints closer than this (inches) are treated as the same point when chaining.
const CHAIN_TOLERANCE: f64 = 0.01;
/// Every this-many'th contour is a major contour.
const MAJOR_EVERY: i64 = 5;

/// All contour lines at one elevation.
#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    /// Elevation, inches.
    pub z: f64,
    /// Chained polylines in plan coordinates; closed loops repeat their first point.
    pub polylines: Vec<Vec<Point>>,
    /// True for every fifth contour (multiples of `5 * interval`, including 0).
    pub major: bool,
}

/// Contours of `surface` every `interval` inches (non-positive falls back to 12"),
/// sorted by elevation. Levels the surface never crosses are omitted.
///
/// A vertex exactly on a level counts as above it, so a perfectly flat surface at a
/// level yields no contour.
pub fn contours(surface: &TerrainSurface, interval: f64) -> Vec<Contour> {
    contours_with(surface, interval, MAJOR_EVERY as u32)
}

/// [`contours`] with every `major_every`'th level a major contour (`0` falls
/// back to every fifth).
pub fn contours_with(surface: &TerrainSurface, interval: f64, major_every: u32) -> Vec<Contour> {
    contours_opts(
        surface,
        &ContourOptions {
            interval,
            major_every,
            ..ContourOptions::default()
        },
    )
}

/// How the contour lines are cut (Terrain Specification > Contours).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourOptions {
    /// Distance between contours, inches (non-positive falls back to 12").
    pub interval: f64,
    /// Every this-many'th contour is a major one (0 = every fifth).
    pub major_every: u32,
    /// Shifts which elevation gets a contour: levels fall at `offset` plus
    /// whole intervals, inches.
    pub offset: f64,
    /// Passes of 2D corner cutting that smooth the lines (0 = none).
    pub smooth_passes: u32,
}

impl Default for ContourOptions {
    fn default() -> Self {
        ContourOptions {
            interval: 12.0,
            major_every: MAJOR_EVERY as u32,
            offset: 0.0,
            smooth_passes: 0,
        }
    }
}

/// [`contours`] with all the Contours panel options.
pub fn contours_opts(surface: &TerrainSurface, opts: &ContourOptions) -> Vec<Contour> {
    let (interval, major_every, offset) = (opts.interval, opts.major_every, opts.offset);
    let offset = if offset.is_finite() { offset } else { 0.0 };
    let major_every = if major_every == 0 {
        MAJOR_EVERY
    } else {
        i64::from(major_every)
    };
    let interval = if interval.is_finite() && interval > 0.0 {
        interval
    } else {
        12.0
    };
    let z = |i: u32| surface.vertices[i as usize][1];
    let mut segments: BTreeMap<i64, Vec<(Point, Point)>> = BTreeMap::new();
    for tri in &surface.triangles {
        let zs = tri.map(z);
        let (zmin, zmax) = (
            zs.iter().copied().fold(f64::INFINITY, f64::min),
            zs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        );
        let k0 = ((zmin - offset) / interval).ceil() as i64;
        let k1 = ((zmax - offset) / interval).floor() as i64;
        for k in k0..=k1 {
            let level = offset + k as f64 * interval;
            let above = zs.map(|v| v >= level);
            let crossings: Vec<Point> = (0..3)
                .filter(|&e| above[e] != above[(e + 1) % 3])
                .map(|e| crossing(surface, tri[e], tri[(e + 1) % 3], level))
                .collect();
            if let [a, b] = crossings[..] {
                segments.entry(k).or_default().push((a, b));
            }
        }
    }
    segments
        .into_iter()
        .filter_map(|(k, segs)| {
            let mut polylines = chain_segments(&segs);
            if opts.smooth_passes > 0 {
                polylines = polylines
                    .iter()
                    .map(|l| smooth_line(l, opts.smooth_passes.min(6)))
                    .collect();
            }
            (!polylines.is_empty()).then(|| Contour {
                z: offset + k as f64 * interval,
                polylines,
                major: k.rem_euclid(major_every) == 0,
            })
        })
        .collect()
}

/// Chaikin corner cutting: each pass replaces every corner by two points a
/// quarter of the way along its neighbors. Open lines keep their end points; a
/// closed loop (first point repeated last) stays closed.
pub fn smooth_line(line: &[Point], passes: u32) -> Vec<Point> {
    let mut pts = line.to_vec();
    for _ in 0..passes {
        if pts.len() < 3 {
            break;
        }
        let closed = pts.first() == pts.last();
        let n = pts.len();
        let mut next = Vec::with_capacity(n * 2);
        if !closed {
            next.push(pts[0]);
        }
        let segs = n - 1;
        for i in 0..segs {
            let (a, b) = (pts[i], pts[i + 1]);
            if !closed && i == 0 {
                next.push(Point::lerp(a, b, 0.75));
            } else if !closed && i == segs - 1 {
                next.push(Point::lerp(a, b, 0.25));
            } else {
                next.push(Point::lerp(a, b, 0.25));
                next.push(Point::lerp(a, b, 0.75));
            }
        }
        if closed {
            let first = next[0];
            next.push(first);
        } else {
            next.push(pts[n - 1]);
        }
        pts = next;
    }
    pts
}

/// Where the edge `a`-`b` crosses `level`, computed identically from either triangle.
fn crossing(surface: &TerrainSurface, a: u32, b: u32, level: f64) -> Point {
    let (a, b) = (a.min(b), a.max(b));
    let (va, vb) = (surface.vertices[a as usize], surface.vertices[b as usize]);
    let t = (level - va[1]) / (vb[1] - va[1]);
    Point::new(va[0] + (vb[0] - va[0]) * t, va[2] + (vb[2] - va[2]) * t)
}

/// Merges points within [`CHAIN_TOLERANCE`] into numbered nodes.
#[derive(Default)]
struct NodeTable {
    pts: Vec<Point>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl NodeTable {
    fn node(&mut self, p: Point) -> usize {
        let cell = |v: f64| (v / CHAIN_TOLERANCE).floor() as i64;
        let (cx, cy) = (cell(p.x), cell(p.y));
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(ids) = self.cells.get(&(cx + dx, cy + dy)) {
                    if let Some(&id) = ids
                        .iter()
                        .find(|&&i| self.pts[i].dist(p) <= CHAIN_TOLERANCE)
                    {
                        return id;
                    }
                }
            }
        }
        self.cells.entry((cx, cy)).or_default().push(self.pts.len());
        self.pts.push(p);
        self.pts.len() - 1
    }
}

/// Chain unordered segments into polylines by matching endpoints.
fn chain_segments(segments: &[(Point, Point)]) -> Vec<Vec<Point>> {
    let mut nodes = NodeTable::default();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for &(p, q) in segments {
        let (a, b) = (nodes.node(p), nodes.node(q));
        if a != b && seen.insert((a.min(b), a.max(b))) {
            edges.push((a, b));
        }
    }
    let mut adjacent: Vec<Vec<usize>> = vec![Vec::new(); nodes.pts.len()];
    for (e, &(a, b)) in edges.iter().enumerate() {
        adjacent[a].push(e);
        adjacent[b].push(e);
    }
    let mut used = vec![false; edges.len()];
    // Open chains start at their ends (odd degree); whatever remains are closed loops.
    let order = (0..adjacent.len())
        .filter(|&n| adjacent[n].len() % 2 == 1)
        .chain(0..adjacent.len());
    let mut polylines = Vec::new();
    for start in order {
        while let Some(&first) = adjacent[start].iter().find(|&&e| !used[e]) {
            let mut line = vec![nodes.pts[start]];
            let (mut cur, mut edge) = (start, first);
            loop {
                used[edge] = true;
                let (a, b) = edges[edge];
                cur = if a == cur { b } else { a };
                line.push(nodes.pts[cur]);
                match adjacent[cur].iter().find(|&&e| !used[e]) {
                    Some(&next) => edge = next,
                    None => break,
                }
            }
            polylines.push(line);
        }
    }
    polylines
}
