//! Retaining Wall tools (manual p. 1319): a Terrain Break plus a terrain wall
//! whose height comes from the ground on both sides. The top matches the high
//! side and the bottom the low side; on flat ground it is a concrete strip.

use plan_core::Point;

use crate::geom::dedup_points;
use crate::landscape::{TerrainBreak, TerrainWall, WallKind};
use crate::model::{Terrain, TerrainSurface};
use crate::query::elevation_at;

/// How far either side of the wall's face the ground is read, inches.
pub const SIDE_SAMPLE: f64 = 36.0;
/// Spacing of the samples along the wall, inches.
const ALONG_STEP: f64 = 36.0;
/// Depth of the footing under the low side, inches.
pub const FOOTING: f64 = 12.0;

/// What the Retaining Wall tool makes.
#[derive(Debug, Clone, PartialEq)]
pub struct RetainingWall {
    /// The Terrain Break along the wall (it follows the ground, so the
    /// contours stay sharp at the wall without holding one elevation).
    pub terrain_break: TerrainBreak,
    /// The wall, drawn so the high side is on its left.
    pub wall: TerrainWall,
    /// Mean ground elevation on the high side, inches.
    pub high: f64,
    /// Mean ground elevation on the low side, inches.
    pub low: f64,
}

impl RetainingWall {
    /// How far the ground drops across the wall, inches.
    pub fn drop(&self) -> f64 {
        self.high - self.low
    }
}

fn samples_along(points: &[Point]) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    for w in points.windows(2) {
        let len = w[0].dist(w[1]);
        if len < 1e-9 {
            continue;
        }
        let normal = w[1].sub(w[0]).normalized().perp();
        let n = (len / ALONG_STEP).ceil().max(1.0) as usize;
        for k in 0..=n {
            out.push((Point::lerp(w[0], w[1], k as f64 / n as f64), normal));
        }
    }
    out
}

/// The Retaining Wall tool: a Terrain Break and a terrain wall along
/// `points`, sized from the existing ground of `surface` on both sides.
///
/// The ground is read [`SIDE_SAMPLE`] inches either side of the wall's face.
/// The path is turned around when needed so the high side is on its left (the
/// retained side of a terrain wall); the wall's top stands at the high side and
/// its bottom [`FOOTING`] below the low side. The grade step across the wall is
/// the drop between the sides. `None` for a path with no length.
pub fn retaining_wall(
    _t: &Terrain,
    surface: &TerrainSurface,
    points: Vec<Point>,
    curved: bool,
) -> Option<RetainingWall> {
    let mut pts = dedup_points(&points, false);
    if pts.len() < 2 {
        return None;
    }
    let thickness = TerrainWall::new(WallKind::Wall, Vec::new(), false).thickness;
    let off = thickness / 2.0 + SIDE_SAMPLE;
    let fallback = if surface.vertices.is_empty() {
        0.0
    } else {
        surface.vertices.iter().map(|v| v[1]).sum::<f64>() / surface.vertices.len() as f64
    };
    let ground = |p: Point| elevation_at(surface, p).unwrap_or(fallback);
    let (mut left_sum, mut right_sum, mut n) = (0.0, 0.0, 0);
    for (p, normal) in samples_along(&pts) {
        left_sum += ground(p.add(normal.scale(off)));
        right_sum += ground(p.sub(normal.scale(off)));
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let (mut left, mut right) = (left_sum / f64::from(n), right_sum / f64::from(n));
    if right > left {
        pts.reverse();
        std::mem::swap(&mut left, &mut right);
    }
    let drop = (left - right).max(0.0);
    let mut wall = TerrainWall::new(WallKind::Wall, pts.clone(), curved);
    wall.height = 0.0;
    wall.depth = FOOTING;
    wall.retain = drop;
    wall.stepped = false;
    let terrain_break = TerrainBreak {
        points: pts,
        z: left,
        follow_ground: true,
        ..TerrainBreak::default()
    };
    Some(RetainingWall {
        terrain_break,
        wall,
        high: left,
        low: right,
    })
}
