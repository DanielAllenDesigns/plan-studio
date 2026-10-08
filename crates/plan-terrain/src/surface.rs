//! Build Terrain: sample, interpolate, triangulate, clip and smooth.

use std::collections::HashMap;

use plan_core::geometry::{dist_to_segment, point_in_polygon, polygon_area, polygon_centroid};
use plan_core::Point;

use crate::delaunay::triangulate;
use crate::elevation::{break_step, ElevationModel, FEATHER};
use crate::geom::{bounds, dedup_points, densify, dist_to_boundary, offset_polygon, strip_edges};
use crate::grading::ring_step;
use crate::model::{Feature, FeatureKind, HeightGrid, ModifierKind, Terrain, TerrainSurface};
use crate::query::elevation_at;

/// Default grid spacing when the terrain's value is unusable, inches.
const DEFAULT_SPACING: f64 = 120.0;
/// The sample grid is coarsened until it has at most this many nodes.
const MAX_GRID_NODES: usize = 10_000;
/// Sample points closer than this (inches) are merged.
const MERGE_TOLERANCE: f64 = 1.0;
/// How far a terrain hole sits outside the building footprint, inches.
const HOLE_MARGIN: f64 = 12.0;

/// A point set that rejects points within [`MERGE_TOLERANCE`] of an existing one.
#[derive(Default)]
struct PointSet {
    pts: Vec<Point>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl PointSet {
    fn cell(p: Point) -> (i64, i64) {
        (
            (p.x / MERGE_TOLERANCE).floor() as i64,
            (p.y / MERGE_TOLERANCE).floor() as i64,
        )
    }

    fn insert(&mut self, p: Point) {
        let (cx, cy) = Self::cell(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                let near = self
                    .cells
                    .get(&(cx + dx, cy + dy))
                    .is_some_and(|ids| ids.iter().any(|&i| self.pts[i].dist(p) < MERGE_TOLERANCE));
                if near {
                    return;
                }
            }
        }
        self.cells.entry((cx, cy)).or_default().push(self.pts.len());
        self.pts.push(p);
    }
}

fn sanitize_spacing(requested: f64, width: f64, height: f64) -> f64 {
    let mut s = if requested.is_finite() && requested > 0.0 {
        requested
    } else {
        DEFAULT_SPACING
    };
    let nodes = |s: f64| ((width / s).ceil() as usize + 1) * ((height / s).ceil() as usize + 1);
    while nodes(s) > MAX_GRID_NODES {
        s *= 1.25;
    }
    s
}

/// The steps of Build Terrain, for a progress display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStage {
    /// Sampling the elevation data, the pads and the walls.
    Sampling,
    /// Delaunay triangulation.
    Triangulating,
    /// Clipping to the perimeter, holes and walls.
    Clipping,
    /// Smoothing the surface.
    Smoothing,
    /// The surface is ready.
    Done,
}

impl BuildStage {
    pub fn label(self) -> &'static str {
        match self {
            BuildStage::Sampling => "Sampling elevation data",
            BuildStage::Triangulating => "Triangulating",
            BuildStage::Clipping => "Clipping to the perimeter",
            BuildStage::Smoothing => "Smoothing",
            BuildStage::Done => "Done",
        }
    }
}

/// Chief's Build Terrain: a triangulated surface clipped to the perimeter.
///
/// 1. Sample a regular grid over the perimeter bounding box (coarsened to at most
///    10,000 nodes) plus every elevation datum, modifier and hole vertex.
/// 2. Elevation is inverse-distance interpolation of points, lines (sampled every
///    `grid_spacing / 2`) and regions (which fix z inside their polygon).
/// 3. Modifiers are applied in order: hill/valley bumps, raised/lowered shifts with a
///    24" feathered edge, flat regions levelled to their mean.
/// 4. Delaunay triangulation (Bowyer-Watson), dropping triangles whose centroid is
///    outside the perimeter or inside a `Hole` feature.
/// 5. `smoothing` Laplacian iterations on interior vertices (boundary vertices and
///    vertices on a Terrain Break line stay put).
///
/// Break lines are sampled finely along their length at their own elevation, so the
/// surface holds that elevation along the line and smoothing leaves the crease sharp.
///
/// Features with `pad` set and the building pad are graded: the terrain under
/// them is levelled and their sides slope back to the existing ground (see the
/// `grading` module). Terrain walls and curbs cut the surface: the strip under
/// a wall is left out, the surface either side takes its own grade and
/// contours stop at the wall.
///
/// An invalid perimeter (fewer than three distinct points or zero area) gives an empty surface.
pub fn build_terrain(t: &Terrain) -> TerrainSurface {
    build_terrain_with_progress(t, &mut |_, _| {})
}

/// [`build_terrain`] reporting its progress: `progress(stage, fraction)` is
/// called as each step starts (`fraction` runs from 0 to 1).
pub fn build_terrain_with_progress(
    t: &Terrain,
    progress: &mut dyn FnMut(BuildStage, f32),
) -> TerrainSurface {
    progress(BuildStage::Sampling, 0.0);
    let perimeter = dedup_points(&t.perimeter, true);
    if perimeter.len() < 3 || polygon_area(&perimeter).abs() < 1e-6 {
        return TerrainSurface::default();
    }
    let (lo, hi) = bounds(&perimeter).expect("perimeter is non-empty");
    let requested = t.grid_spacing / f64::from(t.subdivision.max(1));
    let spacing = sanitize_spacing(requested, hi.x - lo.x, hi.y - lo.y);
    let model = ElevationModel::new(t, spacing);

    let nx = ((hi.x - lo.x) / spacing - 1e-9).ceil().max(0.0) as usize + 1;
    let ny = ((hi.y - lo.y) / spacing - 1e-9).ceil().max(0.0) as usize + 1;
    let mut grid = HeightGrid {
        origin: lo,
        spacing,
        nx,
        ny,
        z: Vec::with_capacity(nx * ny),
    };
    for j in 0..ny {
        for i in 0..nx {
            grid.z.push(model.height(grid.node(i, j)));
        }
    }

    let samples = collect_samples(t, &perimeter, &grid, &model);
    progress(BuildStage::Triangulating, 0.35);
    let triangles = triangulate(&samples);
    progress(BuildStage::Clipping, 0.7);

    let holes: Vec<&[Point]> = t
        .features
        .iter()
        .filter(|f| f.kind == FeatureKind::Hole)
        .map(|f| f.polygon.as_slice())
        .collect();
    let kept: Vec<[usize; 3]> = triangles
        .into_iter()
        .filter(|tri| {
            let [a, b, c] = tri.map(|i| samples[i]);
            if b.sub(a).cross(c.sub(a)).abs() < 1e-9 {
                return false;
            }
            let centroid = a.add(b).add(c).scale(1.0 / 3.0);
            point_in_polygon(centroid, &perimeter)
                && !holes.iter().any(|h| point_in_polygon(centroid, h))
                && !model.cuts().iter().any(|w| w.contains(centroid))
        })
        .collect();

    // Keep only vertices that triangles use.
    let mut remap = vec![u32::MAX; samples.len()];
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(kept.len());
    for tri in &kept {
        tris.push(tri.map(|i| {
            if remap[i] == u32::MAX {
                remap[i] = vertices.len() as u32;
                vertices.push([samples[i].x, model.height(samples[i]), samples[i].y]);
            }
            remap[i]
        }));
    }

    // Vertices on a break line, along a wall or under a pad stay put so the
    // crease, the cut and the flat pad survive smoothing.
    let on_break: Vec<bool> = vertices
        .iter()
        .map(|v| {
            let p = Point::new(v[0], v[2]);
            t.breaks.iter().any(|b| {
                b.points
                    .windows(2)
                    .any(|w| dist_to_segment(p, w[0], w[1]) <= MERGE_TOLERANCE)
            }) || model
                .cuts()
                .iter()
                .any(|w| (w.distance(p) - w.half).abs() <= MERGE_TOLERANCE)
                || model.pads().iter().any(|pad| {
                    point_in_polygon(p, &pad.polygon)
                        || dist_to_boundary(p, &pad.polygon) <= MERGE_TOLERANCE
                })
        })
        .collect();
    progress(BuildStage::Smoothing, 0.9);
    smooth(&mut vertices, &tris, t.smoothing, &on_break);
    let mut surface = TerrainSurface::new(vertices, tris, grid);
    if t.smoothing > 0 {
        // Keep the exported grid consistent with the smoothed surface where it has data.
        let mut grid = std::mem::take(&mut surface.grid);
        for j in 0..grid.ny {
            for i in 0..grid.nx {
                if let Some(z) = elevation_at(&surface, grid.node(i, j)) {
                    grid.z[j * grid.nx + i] = z;
                }
            }
        }
        surface.grid = grid;
    }
    progress(BuildStage::Done, 1.0);
    surface
}

/// Every plan point the triangulation is built from, perimeter first.
fn collect_samples(
    t: &Terrain,
    perimeter: &[Point],
    grid: &HeightGrid,
    model: &ElevationModel,
) -> Vec<Point> {
    let spacing = grid.spacing;
    let half = spacing / 2.0;
    let mut set = PointSet::default();
    let on_lot = |p: Point| {
        point_in_polygon(p, perimeter) || dist_to_boundary(p, perimeter) < MERGE_TOLERANCE
    };

    for p in densify(perimeter, spacing, true) {
        set.insert(p);
    }
    let add_if_on_lot = |set: &mut PointSet, pts: Vec<Point>| {
        for p in pts.into_iter().filter(|&p| on_lot(p)) {
            set.insert(p);
        }
    };
    add_if_on_lot(&mut set, t.elevation_points.iter().map(|e| e.pos).collect());
    for line in &t.elevation_lines {
        add_if_on_lot(&mut set, densify(&line.points, half, false));
    }
    for region in &t.elevation_regions {
        add_if_on_lot(&mut set, densify(&region.polygon, half, true));
    }
    for m in &t.modifiers {
        add_if_on_lot(&mut set, densify(&m.polygon, half, true));
        add_if_on_lot(&mut set, vec![polygon_centroid(&m.polygon)]);
        if matches!(
            m.kind,
            ModifierKind::RaisedRegion | ModifierKind::LoweredRegion | ModifierKind::FlatRegion
        ) {
            // An inset ring where the feathered edge reaches full height.
            add_if_on_lot(
                &mut set,
                densify(&offset_polygon(&m.polygon, -FEATHER), half, true),
            );
        }
    }
    for f in t.features.iter().filter(|f| f.kind == FeatureKind::Hole) {
        add_if_on_lot(&mut set, densify(&f.polygon, half, true));
    }
    for brk in &t.breaks {
        add_if_on_lot(&mut set, densify(&brk.points, break_step(spacing), false));
    }
    // Pads: the outline and rings out to where the sides meet the ground.
    let step = ring_step(spacing);
    for pad in model.pads() {
        add_if_on_lot(&mut set, densify(&pad.polygon, half.min(step), true));
        for d in pad.ring_offsets() {
            add_if_on_lot(
                &mut set,
                densify(&offset_polygon(&pad.polygon, d), half.min(step), true),
            );
        }
    }
    // Walls: both edges of the strip densely enough that the strip is a
    // conforming gap, then lines across the graded cut side.
    for cut in model.cuts() {
        let edges = strip_edges(&cut.points, cut.half);
        let edge_step = (cut.half * 1.8).clamp(2.0, 24.0);
        add_if_on_lot(&mut set, densify(&edges.left, edge_step, false));
        add_if_on_lot(&mut set, densify(&edges.right, edge_step, false));
        if cut.retain != 0.0 {
            for k in 1..=4 {
                let offset = cut.half + cut.reach * f64::from(k) / 4.0;
                add_if_on_lot(
                    &mut set,
                    densify(&strip_edges(&cut.points, offset).right, half, false),
                );
            }
        }
    }
    for j in 0..grid.ny {
        for i in 0..grid.nx {
            let p = grid.node(i, j);
            if point_in_polygon(p, perimeter) {
                set.insert(p);
            }
        }
    }
    // Nothing is sampled under a wall.
    set.pts
        .retain(|&p| !model.cuts().iter().any(|w| w.contains(p)));
    set.pts
}

/// Laplacian smoothing of interior vertices; vertices on a boundary edge are fixed.
fn smooth(vertices: &mut [[f64; 3]], triangles: &[[u32; 3]], iterations: u32, pinned: &[bool]) {
    if iterations == 0 || triangles.is_empty() {
        return;
    }
    let mut edge_uses: HashMap<(u32, u32), u32> = HashMap::new();
    let mut neighbors: Vec<Vec<u32>> = vec![Vec::new(); vertices.len()];
    for tri in triangles {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            *edge_uses.entry((a.min(b), a.max(b))).or_default() += 1;
            neighbors[a as usize].push(b);
            neighbors[b as usize].push(a);
        }
    }
    let mut fixed = pinned.to_vec();
    for (&(a, b), &uses) in &edge_uses {
        if uses == 1 {
            fixed[a as usize] = true;
            fixed[b as usize] = true;
        }
    }
    for list in &mut neighbors {
        list.sort_unstable();
        list.dedup();
    }
    for _ in 0..iterations {
        let z: Vec<f64> = vertices.iter().map(|v| v[1]).collect();
        for (i, v) in vertices.iter_mut().enumerate() {
            if fixed[i] || neighbors[i].is_empty() {
                continue;
            }
            let mean = neighbors[i].iter().map(|&n| z[n as usize]).sum::<f64>()
                / neighbors[i].len() as f64;
            v[1] = 0.5 * z[i] + 0.5 * mean;
        }
    }
}

/// Add a `Hole` feature around a building: the footprint offset outward by 12".
///
/// Footprints with fewer than three distinct points are ignored.
pub fn auto_hole_for_building(t: &mut Terrain, footprint: &[Point]) {
    let polygon = offset_polygon(footprint, HOLE_MARGIN);
    if polygon.len() < 3 {
        return;
    }
    t.features.push(Feature {
        kind: FeatureKind::Hole,
        polygon,
        ..Feature::default()
    });
}
