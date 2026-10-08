//! plan-terrain: Chief-style terrain for Plan Studio.
//!
//! A [`Terrain`] holds the Terrain Perimeter, elevation data (points, lines,
//! regions), modifiers (hill, valley, raised/lowered/flat regions), features
//! (including terrain holes under buildings) and road/driveway/sidewalk strips.
//! [`build_terrain`] is Chief's "Build Terrain": it samples a regular grid,
//! interpolates elevations, applies the modifiers, triangulates with a
//! Bowyer-Watson Delaunay, clips to the perimeter and smooths. From the
//! resulting [`TerrainSurface`] you can query [`elevation_at`], extract
//! [`contours`], build 3D meshes ([`terrain_mesh`], [`road_meshes`]) and draw
//! plan symbols ([`plan_symbols`]).
//!
//! Units are inches. Surface vertices are stored as `[plan x, elevation, plan y]`
//! (elevation up); the 3D meshes use plan-3d's frame (X right, Y up, Z = -plan y).

mod contour;
pub mod delaunay;
mod elevation;
mod geom;
mod mesh;
mod model;
mod query;
mod surface;
mod symbols;

#[cfg(test)]
mod tests;

pub use contour::{contours, Contour};
pub use geom::flatten_spline;
pub use mesh::{road_meshes, terrain_mesh};
pub use model::{
    ElevationLine, ElevationPoint, ElevationRegion, Feature, FeatureKind, HeightGrid, Modifier,
    ModifierKind, RoadKind, RoadStrip, Terrain, TerrainSurface,
};
pub use query::elevation_at;
pub use surface::{auto_hole_for_building, build_terrain};
pub use symbols::{plan_symbols, Stroke, StrokeKind};
