//! Cut and fill: graded pads, wall cuts and the cut/fill volume report.
//!
//! A *pad* is a flat area at one elevation. Inside its outline the terrain is
//! the pad's top; outside, the pad's sides slope back to the existing ground at
//! `ratio` (horizontal run per unit of rise, so 2 is a 1:2 slope). Where the
//! existing ground is above the pad the side is a cut slope rising from the pad
//! edge, where it is below the side is a fill slope falling from it, and each
//! stops (the "daylight line") where it meets the existing ground.
//!
//! Pads come from the features with `pad` set (flat top at the mean existing
//! ground under the outline plus the feature's height, negative to cut in) and
//! from the building pad (the first floor less the terrain-to-first-floor
//! distance, under the footprint plus a margin).
//!
//! A *wall cut* lowers the surface on the cut (right) side of a terrain wall
//! or curb by its `retain` height at the wall, sloping back up at 1:4 within
//! the wall's reach and fading out beyond its ends.

use plan_core::geometry::{point_in_polygon, polygon_area};
use plan_core::Point;

use crate::elevation::ElevationModel;
use crate::geom::{bounds, dedup_points, densify, dist_to_boundary, offset_polygon};
use crate::landscape::TerrainWall;
use crate::model::{Feature, FeatureKind, Terrain, DEFAULT_SLOPE_RATIO};

/// Steepest side slope allowed: 1:0.25, nearly vertical.
const MIN_RATIO: f64 = 0.25;
/// Farthest a pad's side slope is followed, inches.
const MAX_REACH: f64 = 1800.0;
/// Cubic inches in a cubic yard.
const CUBIC_INCHES_PER_YARD: f64 = 46_656.0;
/// The volume integration uses at most this many cells per pad.
const MAX_CELLS: f64 = 40_000.0;

/// Where a pad comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadSource {
    /// `Terrain::features[i]`.
    Feature(usize),
    /// The building pad.
    Building,
}

/// A graded pad.
#[derive(Debug, Clone)]
pub(crate) struct Pad {
    pub(crate) source: PadSource,
    pub(crate) polygon: Vec<Point>,
    /// Elevation of the flat top, inches.
    pub(crate) top: f64,
    /// Run per unit of rise of the sides.
    pub(crate) ratio: f64,
    /// Distance from the pad at which its sides meet the existing ground, inches.
    pub(crate) reach: f64,
}

impl Pad {
    /// The graded elevation at `p` where the existing ground is `z`.
    pub(crate) fn apply(&self, z: f64, p: Point) -> f64 {
        slope_height(&self.polygon, self.top, self.ratio, z, p)
    }

    /// Distances from the pad outline to the rings the surface is sampled on.
    pub(crate) fn ring_offsets(&self) -> Vec<f64> {
        ring_series(self.reach + 1e-6).collect()
    }
}

/// Distances of the sample rings around a pad: every foot near the pad, then
/// ever farther apart (a quarter of the distance).
fn ring_series(limit: f64) -> impl Iterator<Item = f64> {
    let mut d = 0.0_f64;
    std::iter::from_fn(move || {
        d += (d / 4.0).max(12.0);
        (d <= limit).then_some(d)
    })
}

/// The side-slope rule of a pad: `top` inside `polygon`, otherwise the ground
/// `z` held back by a slope of `ratio` from the pad edge (cut above the pad,
/// fill below it).
fn slope_height(polygon: &[Point], top: f64, ratio: f64, z: f64, p: Point) -> f64 {
    if point_in_polygon(p, polygon) {
        return top;
    }
    let run = dist_to_boundary(p, polygon) / ratio;
    if z >= top {
        z.min(top + run)
    } else {
        z.max(top - run)
    }
}

/// Spacing of the sample rings around a pad.
pub(crate) fn ring_step(spacing: f64) -> f64 {
    (spacing / 2.0).clamp(12.0, 60.0)
}

fn clean_ratio(ratio: f64) -> f64 {
    if ratio.is_finite() && ratio > 0.0 {
        ratio.max(MIN_RATIO)
    } else {
        DEFAULT_SLOPE_RATIO
    }
}

fn make_pad(
    model: &ElevationModel,
    source: PadSource,
    polygon: Vec<Point>,
    top: f64,
    ratio: f64,
) -> Pad {
    let ratio = clean_ratio(ratio);
    let step = ring_step(model.spacing());
    let mut reach = MAX_REACH;
    for d in ring_series(MAX_REACH) {
        let ring = densify(&offset_polygon(&polygon, d), step, true);
        let active = ring.iter().any(|&q| {
            let z = model.height_ungraded(q);
            z > top + d / ratio + 0.01 || z < top - d / ratio - 0.01
        });
        if !active {
            reach = d;
            break;
        }
    }
    Pad {
        source,
        polygon,
        top,
        ratio,
        reach,
    }
}

fn usable(polygon: &[Point]) -> bool {
    polygon.len() >= 3 && polygon_area(polygon).abs() >= 1.0
}

/// The pads of `t`, the building pad first, then the pad features in order.
pub(crate) fn build_pads(t: &Terrain, model: &ElevationModel) -> Vec<Pad> {
    let mut pads = Vec::new();
    if t.flatten_pad {
        if let Some(b) = &t.building_pad {
            let footprint = dedup_points(&b.footprint, true);
            if usable(&footprint) {
                let polygon = offset_polygon(&footprint, b.margin.max(0.0));
                let top = match b.first_floor {
                    Some(ff) => ff - t.subfloor_height_above_terrain,
                    None => model.mean_ground(&polygon),
                };
                pads.push(make_pad(
                    model,
                    PadSource::Building,
                    polygon,
                    top,
                    b.slope_ratio,
                ));
            }
        }
    }
    for (i, f) in t.features.iter().enumerate() {
        if !f.pad || f.kind == FeatureKind::Hole {
            continue;
        }
        let polygon = dedup_points(&f.polygon, true);
        if usable(&polygon) {
            let top = model.mean_ground(&polygon) + f.height;
            pads.push(make_pad(
                model,
                PadSource::Feature(i),
                polygon,
                top,
                f.slope_ratio,
            ));
        }
    }
    pads
}

// ----- wall cuts -----

/// The surface shift either side of a terrain wall or curb.
#[derive(Debug, Clone)]
pub(crate) struct WallCut {
    pub(crate) points: Vec<Point>,
    /// Half the wall's thickness, inches.
    pub(crate) half: f64,
    pub(crate) retain: f64,
    pub(crate) reach: f64,
}

impl WallCut {
    fn new(w: &TerrainWall) -> Option<Self> {
        let points = dedup_points(&w.points, false);
        (w.cut && points.len() >= 2 && w.thickness > 0.0).then(|| WallCut {
            points,
            half: (w.thickness / 2.0).max(0.5),
            retain: if w.retain.is_finite() { w.retain } else { 0.0 },
            reach: w.reach(),
        })
    }

    /// Lateral offset (positive on the right of travel) and overshoot beyond the
    /// ends of the segment nearest to `p`.
    fn locate(&self, p: Point) -> (f64, f64) {
        let mut best = (f64::INFINITY, 0.0, 0.0);
        for w in self.points.windows(2) {
            let dir = w[1].sub(w[0]);
            let len = dir.length();
            if len < 1e-9 {
                continue;
            }
            let u = dir.scale(1.0 / len);
            let rel = p.sub(w[0]);
            let along = rel.dot(u);
            let s = rel.cross(u);
            let o = if along < 0.0 {
                -along
            } else if along > len {
                along - len
            } else {
                0.0
            };
            let dist = s.hypot(o);
            if dist < best.0 {
                best = (dist, s, o);
            }
        }
        (best.1, best.2)
    }

    /// Distance from `p` to the wall's centerline.
    pub(crate) fn distance(&self, p: Point) -> f64 {
        let (s, o) = self.locate(p);
        s.hypot(o)
    }

    /// Is `p` under the wall itself (no surface there)?
    pub(crate) fn contains(&self, p: Point) -> bool {
        self.distance(p) < self.half - 0.25
    }

    /// How much the surface at `p` moves: `-retain` on the cut side at the
    /// wall, tapering to nothing at the wall's reach, 0 on the retained side.
    pub(crate) fn shift(&self, p: Point) -> f64 {
        if self.retain == 0.0 {
            return 0.0;
        }
        let (s, o) = self.locate(p);
        let half = self.half;
        let lateral = if s <= -half {
            0.0
        } else if s < half {
            (s + half) / (2.0 * half)
        } else if s < half + self.reach {
            1.0 - (s - half) / self.reach
        } else {
            0.0
        };
        let along = (1.0 - o / self.reach).clamp(0.0, 1.0);
        -self.retain * lateral * along
    }
}

/// The wall cuts of `t`.
pub(crate) fn wall_cuts(t: &Terrain) -> Vec<WallCut> {
    t.walls.iter().filter_map(WallCut::new).collect()
}

// ----- the cut/fill report -----

/// Cut and fill of one pad, in cubic yards.
#[derive(Debug, Clone, PartialEq)]
pub struct CutFillItem {
    pub name: String,
    pub source: PadSource,
    /// Elevation of the pad's flat top, inches.
    pub top: f64,
    /// Area of the pad's flat top, square feet.
    pub area_sq_ft: f64,
    /// Cut under the pad.
    pub pad_cut_cy: f64,
    /// Fill under the pad.
    pub pad_fill_cy: f64,
    /// Cut in the sloped sides.
    pub slope_cut_cy: f64,
    /// Fill in the sloped sides.
    pub slope_fill_cy: f64,
}

impl CutFillItem {
    pub fn cut_cy(&self) -> f64 {
        self.pad_cut_cy + self.slope_cut_cy
    }

    pub fn fill_cy(&self) -> f64 {
        self.pad_fill_cy + self.slope_fill_cy
    }
}

/// Cut and fill volumes of every graded pad of a terrain.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CutFillReport {
    pub items: Vec<CutFillItem>,
}

impl CutFillReport {
    pub fn cut_cy(&self) -> f64 {
        self.items.iter().map(CutFillItem::cut_cy).sum()
    }

    pub fn fill_cy(&self) -> f64 {
        self.items.iter().map(CutFillItem::fill_cy).sum()
    }

    /// Cut less fill: positive means soil to haul away, negative soil to bring in.
    pub fn net_cy(&self) -> f64 {
        self.cut_cy() - self.fill_cy()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The report as CSV text: one row per pad (name, top elevation in inches,
    /// area in square feet, cut and fill in cubic yards) and a total row.
    pub fn to_csv(&self) -> String {
        fn field(s: &str) -> String {
            if s.contains([',', '"', '\n']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        }
        let mut out =
            String::from("Pad,Top elevation (in),Area (sq ft),Cut (cu yd),Fill (cu yd)\n");
        for i in &self.items {
            out.push_str(&format!(
                "{},{:.1},{:.1},{:.2},{:.2}\n",
                field(&i.name),
                i.top,
                i.area_sq_ft,
                i.cut_cy(),
                i.fill_cy()
            ));
        }
        out.push_str(&format!(
            "Total,,,{:.2},{:.2}\n",
            self.cut_cy(),
            self.fill_cy()
        ));
        out
    }
}

/// Cubic inches moved by one pad, split under the pad and in its sides.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PadVolumes {
    pub pad_cut: f64,
    pub pad_fill: f64,
    pub slope_cut: f64,
    pub slope_fill: f64,
}

/// Integrates the cut and fill of a pad over the existing `ground`: `top` inside
/// `polygon`, sides of `ratio` out to `reach` inches, on a lattice of `cell`
/// inch squares aligned with the outline's bounding box.
pub fn pad_volumes(
    ground: &dyn Fn(Point) -> f64,
    polygon: &[Point],
    top: f64,
    ratio: f64,
    reach: f64,
    cell: f64,
) -> PadVolumes {
    let mut v = PadVolumes::default();
    let Some((lo, hi)) = bounds(polygon) else {
        return v;
    };
    let ratio = clean_ratio(ratio);
    let cell = cell.max(0.5);
    let margin = (reach / cell).ceil().max(0.0);
    let (x0, y0) = (lo.x - margin * cell, lo.y - margin * cell);
    let nx = (((hi.x - x0) / cell).ceil() + margin) as usize;
    let ny = (((hi.y - y0) / cell).ceil() + margin) as usize;
    let area = cell * cell;
    for j in 0..ny {
        for i in 0..nx {
            let q = Point::new(x0 + (i as f64 + 0.5) * cell, y0 + (j as f64 + 0.5) * cell);
            let inside = point_in_polygon(q, polygon);
            if !inside && dist_to_boundary(q, polygon) > reach {
                continue;
            }
            let z = ground(q);
            let delta = slope_height(polygon, top, ratio, z, q) - z;
            let (cut, fill) = if delta < 0.0 {
                (-delta * area, 0.0)
            } else {
                (0.0, delta * area)
            };
            if inside {
                v.pad_cut += cut;
                v.pad_fill += fill;
            } else {
                v.slope_cut += cut;
                v.slope_fill += fill;
            }
        }
    }
    v
}

fn feature_name(i: usize, f: &Feature) -> String {
    let kind = f.kind_name();
    if f.material.trim().is_empty() {
        format!("{kind} Feature {}", i + 1)
    } else {
        format!("{kind} Feature {} ({})", i + 1, f.material.trim())
    }
}

/// Cut and fill of every graded pad of `t` (the building pad and the features
/// with `pad` set), against the existing ground the terrain data describes.
pub fn cut_fill_report(t: &Terrain) -> CutFillReport {
    let perimeter = dedup_points(&t.perimeter, true);
    let spacing = if t.grid_spacing.is_finite() && t.grid_spacing > 0.0 {
        t.grid_spacing / f64::from(t.subdivision.max(1))
    } else {
        120.0
    };
    if perimeter.len() < 3 {
        return CutFillReport::default();
    }
    let model = ElevationModel::new(t, spacing);
    let ground = |q: Point| model.height_ungraded(q);
    let items = model
        .pads()
        .iter()
        .map(|pad| {
            let (lo, hi) = bounds(&pad.polygon).unwrap_or((Point::ZERO, Point::ZERO));
            let (w, h) = (hi.x - lo.x + 2.0 * pad.reach, hi.y - lo.y + 2.0 * pad.reach);
            let cell = (w * h / MAX_CELLS).sqrt().max(6.0);
            let v = pad_volumes(&ground, &pad.polygon, pad.top, pad.ratio, pad.reach, cell);
            let cy = |cubic_inches: f64| cubic_inches / CUBIC_INCHES_PER_YARD;
            let name = match pad.source {
                PadSource::Building => "Building Pad".to_string(),
                PadSource::Feature(i) => feature_name(i, &t.features[i]),
            };
            CutFillItem {
                name,
                source: pad.source,
                top: pad.top,
                area_sq_ft: polygon_area(&pad.polygon).abs() / 144.0,
                pad_cut_cy: cy(v.pad_cut),
                pad_fill_cy: cy(v.pad_fill),
                slope_cut_cy: cy(v.slope_cut),
                slope_fill_cy: cy(v.slope_fill),
            }
        })
        .collect();
    CutFillReport { items }
}
