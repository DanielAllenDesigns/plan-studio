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
//! [`landscape`] adds the rest of the Terrain menu: break lines, terrain walls and
//! curbs, flat feature slabs, garden beds, grass, water, stepping stones, plant
//! runs and sprinklers, with their plan drawing ([`landscape_plan`], every item on
//! its Chief layer) and 3D meshes draped on the surface ([`landscape_meshes`]).
//!
//! Units are inches. Surface vertices are stored as `[plan x, elevation, plan y]`
//! (elevation up); the 3D meshes use plan-3d's frame (X right, Y up, Z = -plan y).

mod contour;
pub mod delaunay;
mod elevation;
mod geom;
pub mod landscape;
mod landscape_mesh;
mod landscape_plan;
mod mesh;
mod model;
mod query;
mod surface;
mod symbols;

#[cfg(test)]
mod landscape_tests;
#[cfg(test)]
mod tests;

pub use contour::{contours, Contour};
pub use geom::flatten_spline;
pub use landscape::{
    arc_polyline, closed_spline, distribute_along, kidney_outline, open_spline, path_length,
    rectangle_outline, sprinkler_heads, stepping_stones, FillStyle, Landscape, LandscapeKind,
    ObjectStyle, ShapeKind, TerrainBreak, TerrainWall, WallKind,
};
pub use landscape_mesh::{landscape_meshes, wall_meshes};
pub use landscape_plan::{
    circle_points, hatch_segments, landscape_plan, wall_outline, PlanItem, PlanShape,
};
pub use mesh::{
    road_meshes, terrain_mesh, terrain_object_id, terrain_object_of, TerrainPart, TERRAIN_ID_BASE,
};
pub use model::{
    ElevationLine, ElevationPoint, ElevationRegion, Feature, FeatureKind, HeightGrid, Modifier,
    ModifierKind, RoadKind, RoadStrip, Terrain, TerrainSurface,
};
pub use query::elevation_at;
pub use surface::{auto_hole_for_building, build_terrain};
pub use symbols::{plan_symbols, Stroke, StrokeKind};
