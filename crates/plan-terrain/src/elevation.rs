//! The elevation field: inverse-distance interpolation of the data plus modifiers.

use std::f64::consts::PI;

use plan_core::geometry::{point_in_polygon, polygon_centroid};
use plan_core::Point;

use crate::geom::{bounds, densify, dist_to_boundary};
use crate::grading::{build_pads, wall_cuts, Pad, WallCut};
use crate::geom::offset_polygon;
use crate::model::{ElevationRegion, Modifier, ModifierKind, Terrain};
use crate::spec::{AbsoluteElevation, ObjectKey};

/// Distances inside an open-interior region (tangent to the edge) at which
/// the surface is held at the region's elevation, inches.
pub(crate) const TANGENT_RINGS: [f64; 2] = [24.0, 60.0];

/// Width of the soft edge on raised, lowered and flat regions, inches.
pub(crate) const FEATHER: f64 = 24.0;

/// Sample spacing along a break line: finer than the grid so the crease is sharp.
pub(crate) fn break_step(spacing: f64) -> f64 {
    (spacing / 4.0).clamp(6.0, 24.0)
}

/// Evaluates terrain elevation at any plan point.
pub(crate) struct ElevationModel<'a> {
    /// Position, elevation and the distance the datum reaches (0 = no limit).
    datums: Vec<(Point, f64, f64)>,
    regions: &'a [ElevationRegion],
    /// Per region: the interior is held flat (false = only the outline).
    interior_flat: Vec<bool>,
    /// Vertical shift of the whole surface (Absolute Elevation retained modes).
    shift: f64,
    modifiers: &'a [Modifier],
    centroids: Vec<Point>,
    /// Mean elevation inside each flat region (unused for other kinds).
    flat_means: Vec<f64>,
    spacing: f64,
    /// Graded pads (cut/fill features and the building pad).
    pads: Vec<Pad>,
    /// Wall cuts: the surface either side of a terrain wall or curb.
    cuts: Vec<WallCut>,
}

impl<'a> ElevationModel<'a> {
    /// Gather the data: points, lines sampled every `spacing / 2`, and region outlines.
    pub(crate) fn new(t: &'a Terrain, spacing: f64) -> Self {
        let half = spacing / 2.0;
        let mut datums: Vec<(Point, f64, f64)> = t
            .elevation_points
            .iter()
            .map(|e| (e.pos, e.z, 0.0))
            .collect();
        for line in &t.elevation_lines {
            datums.extend(
                densify(&line.points, half, false)
                    .into_iter()
                    .map(|p| (p, line.z, 0.0)),
            );
        }
        let mut interior_flat = Vec::with_capacity(t.elevation_regions.len());
        for (i, region) in t.elevation_regions.iter().enumerate() {
            datums.extend(
                densify(&region.polygon, half, true)
                    .into_iter()
                    .map(|p| (p, region.z, 0.0)),
            );
            let ex = t.extras(ObjectKey::Region(i));
            interior_flat.push(!ex.interior_open);
            if ex.interior_open && ex.tangent_to_edge {
                for d in TANGENT_RINGS {
                    datums.extend(
                        densify(&offset_polygon(&region.polygon, -d), half, true)
                            .into_iter()
                            .map(|p| (p, region.z, 0.0)),
                    );
                }
            }
        }
        for brk in t.breaks.iter().filter(|b| !b.follow_ground) {
            datums.extend(
                densify(&brk.points, break_step(spacing), false)
                    .into_iter()
                    .map(|p| (p, brk.z, brk.transition.max(0.0))),
            );
        }
        let mut model = ElevationModel {
            datums,
            interior_flat,
            shift: 0.0,
            regions: &t.elevation_regions,
            modifiers: &t.modifiers,
            centroids: t
                .modifiers
                .iter()
                .map(|m| polygon_centroid(&m.polygon))
                .collect(),
            flat_means: vec![0.0; t.modifiers.len()],
            spacing,
            pads: Vec::new(),
            cuts: Vec::new(),
        };
        // Flat regions level to the mean of the surface as modified by earlier modifiers.
        for i in 0..model.modifiers.len() {
            if model.modifiers[i].kind == ModifierKind::FlatRegion {
                model.flat_means[i] = model.region_mean(i);
            }
        }
        // Absolute Elevation (retained modes): the whole surface moves so the
        // surface at the Reference Point or at contour 0 sits where it was said.
        model.shift = match t.absolute_elevation {
            AbsoluteElevation::Automatic => 0.0,
            AbsoluteElevation::ContourZero => t.floor_one_elevation + t.surface_offset,
            AbsoluteElevation::ReferencePoint => t
                .effective_reference_point()
                .map_or(0.0, |p| {
                    t.floor_one_elevation + t.surface_offset
                        - model.height_upto(p, model.modifiers.len())
                }),
        };
        model.pads = build_pads(t, &model);
        model.cuts = wall_cuts(t);
        model
    }

    /// The graded pads, in the order they are applied.
    pub(crate) fn pads(&self) -> &[Pad] {
        &self.pads
    }

    /// The wall cuts.
    pub(crate) fn cuts(&self) -> &[WallCut] {
        &self.cuts
    }

    pub(crate) fn spacing(&self) -> f64 {
        self.spacing
    }

    /// Inverse-distance-weighted (power 2) elevation of the data, then region overrides.
    /// With no data at all the surface is flat at 0.
    pub(crate) fn base(&self, p: Point) -> f64 {
        let mut z = self.idw(p);
        for (i, region) in self.regions.iter().enumerate() {
            if self.interior_flat.get(i).copied().unwrap_or(true)
                && point_in_polygon(p, &region.polygon)
            {
                z = region.z;
            }
        }
        z
    }

    fn idw(&self, p: Point) -> f64 {
        let (mut num, mut den) = (0.0, 0.0);
        let (mut num_all, mut den_all) = (0.0, 0.0);
        for &(q, z, reach) in &self.datums {
            let d = p.sub(q);
            let d2 = d.dot(d);
            if d2 < 1e-6 {
                return z;
            }
            let w = 1.0 / d2;
            num_all += w * z;
            den_all += w;
            // A datum with a reach (a break's transition distance) fades out
            // over that distance.
            let fade = if reach > 0.0 {
                let s = (d2.sqrt() / reach).clamp(0.0, 1.0);
                1.0 - s * s * (3.0 - 2.0 * s)
            } else {
                1.0
            };
            num += w * fade * z;
            den += w * fade;
        }
        if den > 1e-12 {
            num / den
        } else if den_all > 0.0 {
            num_all / den_all
        } else {
            0.0
        }
    }

    /// Elevation of the existing ground: base data with every modifier applied
    /// in order, before any pad or wall grades it.
    pub(crate) fn height_ungraded(&self, p: Point) -> f64 {
        self.height_upto(p, self.modifiers.len()) + self.shift
    }

    /// Final elevation: the existing ground graded by the pads (in order) and
    /// shifted across the walls.
    pub(crate) fn height(&self, p: Point) -> f64 {
        let mut z = self.height_ungraded(p);
        for pad in &self.pads {
            z = pad.apply(z, p);
        }
        for cut in &self.cuts {
            z += cut.shift(p);
        }
        z
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
        self.mean_over(&self.modifiers[i].polygon, |p| self.height_upto(p, i))
    }

    /// Mean of the existing ground under `poly`.
    pub(crate) fn mean_ground(&self, poly: &[Point]) -> f64 {
        self.mean_over(poly, |p| self.height_ungraded(p))
    }

    /// Mean of `f` over the polygon's vertices and a lattice of interior points.
    fn mean_over(&self, poly: &[Point], f: impl Fn(Point) -> f64) -> f64 {
        let Some((lo, hi)) = bounds(poly) else {
            return 0.0;
        };
        let step = (self.spacing / 2.0).max((hi.x - lo.x).max(hi.y - lo.y) / 64.0);
        let mut samples: Vec<Point> = poly.to_vec();
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
        let total: f64 = samples.iter().map(|&p| f(p)).sum();
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
