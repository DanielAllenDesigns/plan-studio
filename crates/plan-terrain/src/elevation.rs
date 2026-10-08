//! The elevation field: inverse-distance interpolation of the data plus modifiers.

use std::f64::consts::PI;

use plan_core::geometry::{point_in_polygon, polygon_centroid};
use plan_core::Point;

use crate::geom::{bounds, densify, dist_to_boundary};
use crate::model::{ElevationRegion, Modifier, ModifierKind, Terrain};

/// Width of the soft edge on raised, lowered and flat regions, inches.
pub(crate) const FEATHER: f64 = 24.0;

/// Sample spacing along a break line: finer than the grid so the crease is sharp.
pub(crate) fn break_step(spacing: f64) -> f64 {
    (spacing / 4.0).clamp(6.0, 24.0)
}

/// Evaluates terrain elevation at any plan point.
pub(crate) struct ElevationModel<'a> {
    datums: Vec<(Point, f64)>,
    regions: &'a [ElevationRegion],
    modifiers: &'a [Modifier],
    centroids: Vec<Point>,
    /// Mean elevation inside each flat region (unused for other kinds).
    flat_means: Vec<f64>,
    spacing: f64,
}

impl<'a> ElevationModel<'a> {
    /// Gather the data: points, lines sampled every `spacing / 2`, and region outlines.
    pub(crate) fn new(t: &'a Terrain, spacing: f64) -> Self {
        let half = spacing / 2.0;
        let mut datums: Vec<(Point, f64)> =
            t.elevation_points.iter().map(|e| (e.pos, e.z)).collect();
        for line in &t.elevation_lines {
            datums.extend(
                densify(&line.points, half, false)
                    .into_iter()
                    .map(|p| (p, line.z)),
            );
        }
        for region in &t.elevation_regions {
            datums.extend(
                densify(&region.polygon, half, true)
                    .into_iter()
                    .map(|p| (p, region.z)),
            );
        }
        for brk in &t.breaks {
            datums.extend(
                densify(&brk.points, break_step(spacing), false)
                    .into_iter()
                    .map(|p| (p, brk.z)),
            );
        }
        let mut model = ElevationModel {
            datums,
            regions: &t.elevation_regions,
            modifiers: &t.modifiers,
            centroids: t
                .modifiers
                .iter()
                .map(|m| polygon_centroid(&m.polygon))
                .collect(),
            flat_means: vec![0.0; t.modifiers.len()],
            spacing,
        };
        // Flat regions level to the mean of the surface as modified by earlier modifiers.
        for i in 0..model.modifiers.len() {
            if model.modifiers[i].kind == ModifierKind::FlatRegion {
                model.flat_means[i] = model.region_mean(i);
            }
        }
        model
    }

    /// Inverse-distance-weighted (power 2) elevation of the data, then region overrides.
    /// With no data at all the surface is flat at 0.
    pub(crate) fn base(&self, p: Point) -> f64 {
        let mut z = self.idw(p);
        for region in self.regions {
            if point_in_polygon(p, &region.polygon) {
                z = region.z;
            }
        }
        z
    }

    fn idw(&self, p: Point) -> f64 {
        let (mut num, mut den) = (0.0, 0.0);
        for &(q, z) in &self.datums {
            let d = p.sub(q);
            let d2 = d.dot(d);
            if d2 < 1e-6 {
                return z;
            }
            let w = 1.0 / d2;
            num += w * z;
            den += w;
        }
        if den > 0.0 {
            num / den
        } else {
            0.0
        }
    }

    /// Final elevation: base data with every modifier applied in order.
    pub(crate) fn height(&self, p: Point) -> f64 {
        self.height_upto(p, self.modifiers.len())
    }

    fn height_upto(&self, p: Point, count: usize) -> f64 {
        let mut z = self.base(p);
        for i in 0..count {
            z = self.apply(i, p, z);
        }
        z
    }

    fn apply(&self, i: usize, p: Point, z: f64) -> f64 {
        let m = &self.modifiers[i];
        match m.kind {
            ModifierKind::Hill => z + m.height * bump(p, &m.polygon, self.centroids[i]),
            ModifierKind::Valley => z - m.height * bump(p, &m.polygon, self.centroids[i]),
            ModifierKind::RaisedRegion => z + m.height * feather(p, &m.polygon),
            ModifierKind::LoweredRegion => z - m.height * feather(p, &m.polygon),
            ModifierKind::FlatRegion => {
                let w = feather(p, &m.polygon);
                z + (self.flat_means[i] - z) * w
            }
        }
    }

    /// Mean elevation of modifier `i`'s polygon (vertices plus a lattice of interior points)
    /// before that modifier is applied.
    fn region_mean(&self, i: usize) -> f64 {
        let poly = &self.modifiers[i].polygon;
        let Some((lo, hi)) = bounds(poly) else {
            return 0.0;
        };
        let step = (self.spacing / 2.0).max((hi.x - lo.x).max(hi.y - lo.y) / 64.0);
        let mut samples: Vec<Point> = poly.clone();
        let mut y = lo.y + step / 2.0;
        while y < hi.y {
            let mut x = lo.x + step / 2.0;
            while x < hi.x {
                let p = Point::new(x, y);
                if point_in_polygon(p, poly) {
                    samples.push(p);
                }
                x += step;
            }
            y += step;
        }
        if samples.is_empty() {
            return 0.0;
        }
        let total: f64 = samples.iter().map(|&p| self.height_upto(p, i)).sum();
        total / samples.len() as f64
    }
}

/// Smooth bump weight: 1 at the polygon centroid falling to 0 at the boundary (cosine falloff).
fn bump(p: Point, poly: &[Point], centroid: Point) -> f64 {
    if !point_in_polygon(p, poly) {
        return 0.0;
    }
    let to_centroid = p.dist(centroid);
    let to_edge = dist_to_boundary(p, poly);
    let span = to_centroid + to_edge;
    if span <= 1e-9 {
        return 0.0;
    }
    0.5 * (1.0 + (PI * to_centroid / span).cos())
}

/// Weight 0 on/outside the polygon edge rising smoothly to 1 at [`FEATHER`] inches inside.
fn feather(p: Point, poly: &[Point]) -> f64 {
    if !point_in_polygon(p, poly) {
        return 0.0;
    }
    let s = (dist_to_boundary(p, poly) / FEATHER).clamp(0.0, 1.0);
    s * s * (3.0 - 2.0 * s)
}
