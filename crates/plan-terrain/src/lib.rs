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
mod grading;
mod import;
mod import_assistant;
mod labels;
pub mod landscape;
mod landscape_mesh;
mod landscape_plan;
mod mesh;
mod model;
mod plants;
mod query;
mod retaining;
mod roads;
mod schedule;
mod site_symbols;
mod spec;
mod surface;
mod symbols;

#[cfg(test)]
mod grading_tests;
#[cfg(test)]
mod landscape_tests;
#[cfg(test)]
mod r14_tests;
#[cfg(test)]
mod r15_tests;
#[cfg(test)]
mod tests;

pub use contour::{contours, contours_opts, contours_with, smooth_line, Contour, ContourOptions};
pub use geom::{flatten_spline, flatten_spline_tension};
pub use grading::{
    cut_fill_report, pad_volumes, CutFillItem, CutFillReport, PadSource, PadVolumes,
};
pub use import::{import_points, ImportFormat, ImportUnit, ImportedPoints};
pub use import_assistant::{
    filter_points, import_gps, import_terrain_text, parse_gpx_points, perimeter_around, ranges_of,
    read_columns, scale_points, thin, ColumnOrder, DataRanges, Delimiter, GpsImportAs, GpsKind,
    GpsPoint, GpsResult, GpsTransform, RangeFilter, RawPoint, ScaleOptions, TerrainImport,
    TextLayout, MANY_POINTS,
};
pub use labels::{
    anchor_of, auto_label, label_spots, label_strokes, label_text, LabelSpot,
    LAYER_PRIMARY_CONTOURS, LAYER_SECONDARY_CONTOURS, LAYER_TERRAIN_LABELS,
};
pub use landscape::{
    arc_polyline, closed_spline, dash_path, distribute_along, is_conifer, kidney_control_points,
    kidney_outline, open_spline, path_length, rectangle_outline, sprinkler_heads, stepping_stones,
    FillStyle, Landscape, LandscapeKind, ObjectStyle, PlantForm, ShapeKind, TerrainBreak,
    TerrainWall, WallKind, DEFAULT_WALL_STEP, WALL_SLOPE_RATIO,
};
pub use landscape_mesh::{landscape_meshes, wall_meshes};
pub use landscape_plan::{
    circle_points, hatch_segments, landscape_plan, ripple_lines, wall_outline, PlanItem, PlanShape,
};
pub use mesh::{
    road_meshes, skirt_mesh, terrain_mesh, terrain_mesh_for, terrain_object_id, terrain_object_of,
    TerrainPart, TERRAIN_ID_BASE,
};
pub use model::{
    BuildingPad, ContourStyle, ElevationLine, ElevationPoint, ElevationRegion, Feature,
    FeatureKind, HeightGrid, Modifier, ModifierKind, RoadKind, RoadStrip, Terrain, TerrainSurface,
    DEFAULT_DIRT_MATERIAL, DEFAULT_GROUND_MATERIAL, DEFAULT_LABEL_SPACING, DEFAULT_MAJOR_EVERY,
    DEFAULT_SLOPE_RATIO, DEFAULT_TENSION, MARKING_DASH, MARKING_GAP, MARKING_WIDTH,
    PRIMARY_CONTOUR_WEIGHT, ROUND_CORNERS, SECONDARY_CONTOUR_WEIGHT,
};
pub use plants::{
    default_age_at_maturity, default_seasons, distribute_in, grow_plants, growth_fraction,
    Distribution, GrassBlades, GrassLook, Mow, PlantImage, Season, SeasonLook,
};
pub use query::elevation_at;
pub use retaining::{retaining_wall, RetainingWall, FOOTING, SIDE_SAMPLE};
pub use roads::{
    auto_sidewalks, connected_roads, cul_de_sac, cul_de_sac_at, flare_offset, rectangle_along,
    road_end_near, road_length_and_area, road_polygon, AutoSidewalk, DEFAULT_FLARE,
};
pub use schedule::{
    category_of, default_category, terrain_schedule, ScheduleCategory, ScheduleRow,
};
pub use site_symbols::{
    azimuth_of_plan_vector, facing_label, north_angle_toward, north_pointer_items, north_vector,
    plan_azimuth, scale_bar_items, true_azimuth, SiteMark, DEFAULT_POINTER_RADIUS, SITE_PLAN_LAYER,
};
pub use spec::{
    clear_generated_only, AbsoluteElevation, BuildStats, LabelUnits, ObjectExtras, ObjectInfo,
    ObjectKey, ObjectLabel, Skirt, SkirtMode, SmoothingLevel, TriangleDetail,
    AUTOMATIC_SUBFLOOR_DISTANCE, DEFAULT_MARKER_RADIUS, DEFAULT_SKIRT_THICKNESS,
};
pub use surface::{auto_hole_for_building, build_terrain, build_terrain_with_progress, BuildStage};
pub use symbols::{contour_label_spots, plan_symbols, Stroke, StrokeKind};
