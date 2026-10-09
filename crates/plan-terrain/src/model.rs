//! Data model: the editable [`Terrain`] and the built [`TerrainSurface`].

use std::sync::OnceLock;

use plan_core::Point;
use serde::{Deserialize, Serialize};

use crate::landscape::{Landscape, ObjectStyle, TerrainBreak, TerrainWall};
use crate::query::TriIndex;
use crate::spec::{
    AbsoluteElevation, BuildStats, LabelUnits, ObjectExtras, Skirt, SmoothingLevel, TriangleDetail,
    DEFAULT_CONTOUR_SMOOTH_PASSES, DEFAULT_CUSTOM_TRIANGLES, DEFAULT_SURFACE_OFFSET,
};

/// A spot height (Elevation Point tool).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElevationPoint {
    pub pos: Point,
    /// Elevation, inches.
    pub z: f64,
}

/// Tension of a cardinal spline: 0.5 is Catmull-Rom, 0 a polyline.
pub const DEFAULT_TENSION: f64 = 0.5;

/// A polyline at one height (Elevation Line / Elevation Spline once flattened).
///
/// An Elevation Spline keeps its clicked `control` points and the `tension` it
/// was flattened with, so the curve can be re-flattened when the tension is
/// edited ([`ElevationLine::reflatten`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ElevationLine {
    pub points: Vec<Point>,
    pub z: f64,
    /// Control points of a spline line (empty for a plain polyline).
    pub control: Vec<Point>,
    /// Spline tension, 0 (straight segments) to 1 (loose); 0.5 is Catmull-Rom.
    pub tension: f64,
    /// Label, schedule category and object information.
    pub extras: ObjectExtras,
}

impl Default for ElevationLine {
    fn default() -> Self {
        ElevationLine {
            points: Vec::new(),
            z: 0.0,
            control: Vec::new(),
            tension: DEFAULT_TENSION,
            extras: ObjectExtras::default(),
        }
    }
}

impl ElevationLine {
    /// A straight line at `z`.
    pub fn polyline(points: Vec<Point>, z: f64) -> Self {
        ElevationLine {
            points,
            z,
            ..ElevationLine::default()
        }
    }

    /// A spline through `control` at `z` with `tension` (`samples` points per span).
    pub fn spline(control: Vec<Point>, z: f64, tension: f64, samples: usize) -> Self {
        let points = crate::geom::flatten_spline_tension(&control, false, samples, tension);
        ElevationLine {
            points,
            z,
            control,
            tension,
            extras: ObjectExtras::default(),
        }
    }

    /// Rebuilds `points` from the control points with the current tension
    /// (does nothing for a plain polyline).
    pub fn reflatten(&mut self, samples: usize) {
        if self.control.len() >= 3 {
            self.points =
                crate::geom::flatten_spline_tension(&self.control, false, samples, self.tension);
        }
    }
}

/// A polygon held at one height (Elevation Region).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElevationRegion {
    pub polygon: Vec<Point>,
    pub z: f64,
}

/// The terrain modifier tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModifierKind {
    /// Smooth bump added to the surface.
    Hill,
    /// Smooth dip subtracted from the surface.
    Valley,
    /// Shifts the surface up inside the polygon.
    RaisedRegion,
    /// Shifts the surface down inside the polygon.
    LoweredRegion,
    /// Flat Region (Cut/Fill): levels the polygon to its mean elevation.
    FlatRegion,
}

/// A polygonal modifier. `height` is a magnitude in inches (ignored by `FlatRegion`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Modifier {
    pub kind: ModifierKind,
    pub polygon: Vec<Point>,
    pub height: f64,
}

/// The terrain feature tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeatureKind {
    Rectangular,
    Kidney,
    Spline,
    /// Cuts the surface away (Terrain Hole, Make Terrain Hole Around Building).
    Hole,
    /// Polyline Terrain Feature: a clicked polygon.
    Polyline,
    /// Round Terrain Feature: a circle from its center and radius.
    Round,
}

/// Default slope ratio of the sloped sides of a graded pad: 2 horizontal per
/// 1 vertical (1:2).
pub const DEFAULT_SLOPE_RATIO: f64 = 2.0;

/// A feature overlay. A `Hole` cuts the surface away. A `pad` feature is true
/// cut and fill: the terrain under the outline is levelled to a flat pad and
/// its sides slope back to the existing ground at `slope_ratio`. Any other
/// feature is a flat slab over the ground ([`crate::landscape_meshes`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Feature {
    pub kind: FeatureKind,
    pub polygon: Vec<Point>,
    pub material: String,
    /// Height of the flat top above the mean ground under the outline, inches
    /// (negative cuts the pad into the ground).
    pub height: f64,
    /// Layer, line style and fill style.
    pub style: ObjectStyle,
    /// Grade the terrain to this feature (cut/fill pad with sloped sides).
    pub pad: bool,
    /// Run per unit of rise of the pad's sloped sides (2 = 1:2).
    pub slope_ratio: f64,
    /// Control points of a kidney or spline outline (the polygon is the
    /// closed spline through them); empty when the outline was clicked point
    /// by point.
    pub control: Vec<Point>,
    /// Center of a round feature (the polygon is the circle around it).
    pub center: Point,
    /// Radius of a round feature, inches (0 for every other kind).
    pub radius: f64,
    /// Thickness of the feature, inches. 0 is solid down to the ground; a
    /// positive value makes a shell of that thickness under the top (a
    /// planter or a pool).
    pub thickness: f64,
    /// Hide the part of this feature that a lower feature cuts (planters and pools).
    pub clip_overlap: bool,
    /// Label, schedule category and object information.
    pub extras: ObjectExtras,
}

/// Corners of the polygon a round feature is drawn with.
pub const ROUND_CORNERS: usize = 36;

impl Default for Feature {
    fn default() -> Self {
        Feature {
            kind: FeatureKind::Rectangular,
            polygon: Vec::new(),
            material: String::new(),
            height: 0.0,
            style: ObjectStyle::default(),
            pad: false,
            slope_ratio: DEFAULT_SLOPE_RATIO,
            control: Vec::new(),
            center: Point::new(0.0, 0.0),
            radius: 0.0,
            thickness: 0.0,
            clip_overlap: false,
            extras: ObjectExtras::default(),
        }
    }
}

impl Feature {
    /// A round feature of `radius` inches around `center`.
    pub fn round(center: Point, radius: f64) -> Self {
        let mut f = Feature {
            kind: FeatureKind::Round,
            center,
            radius,
            ..Feature::default()
        };
        f.reflatten();
        f
    }

    /// Resizes a round feature so its edge passes through `edge` (a no-op for
    /// the other kinds).
    pub fn set_radius_through(&mut self, edge: Point) {
        if self.kind == FeatureKind::Round {
            self.radius = self.center.dist(edge).max(1.0);
            self.reflatten();
        }
    }

    /// Rebuilds the outline from the control points (the closed spline through
    /// them). Does nothing for an outline without control points.
    pub fn reflatten(&mut self) {
        if self.kind == FeatureKind::Round && self.radius > 0.0 {
            self.polygon =
                crate::landscape_plan::circle_points(self.center, self.radius, ROUND_CORNERS, 0.0);
        } else if self.control.len() >= 3 {
            self.polygon = crate::landscape::closed_spline(&self.control);
        }
    }

    /// The name the feature kind goes by in the specification and reports.
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            FeatureKind::Rectangular => "Rectangular",
            FeatureKind::Kidney => "Kidney",
            FeatureKind::Spline => "Spline",
            FeatureKind::Hole => "Hole",
            FeatureKind::Polyline => "Polyline",
            FeatureKind::Round => "Round",
        }
    }
}

/// Road-like strip types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoadKind {
    Road,
    Driveway,
    Sidewalk,
    /// Road Marking / Stripe: a thin painted line laid on the ground or on a
    /// road (centerline stripe, crosswalk bar, parking line).
    Marking,
    /// A polyline island inside a road: it cuts a hole in the road that
    /// shows the terrain material, with its own curb when the road has one.
    Median,
    /// A round road end placed on the end of a road.
    CulDeSac,
}

impl RoadKind {
    /// Chief-style name of the kind.
    pub fn name(self) -> &'static str {
        match self {
            RoadKind::Road => "Road",
            RoadKind::Driveway => "Driveway",
            RoadKind::Sidewalk => "Sidewalk",
            RoadKind::Marking => "Road Marking",
            RoadKind::Median => "Median",
            RoadKind::CulDeSac => "Cul-de-sac",
        }
    }

    /// Material a strip of this kind is built of until it names its own.
    pub fn default_material(self) -> &'static str {
        match self {
            RoadKind::Road | RoadKind::Driveway | RoadKind::CulDeSac => "Asphalt",
            RoadKind::Sidewalk => "Concrete",
            RoadKind::Marking => "Paint",
            RoadKind::Median => "Grass",
        }
    }

    /// Kinds drawn as a closed outline rather than along a centerline.
    pub fn is_outline_kind(self) -> bool {
        matches!(self, RoadKind::Median | RoadKind::CulDeSac)
    }
}

/// A road, driveway or sidewalk: a polyline strip draped on the terrain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoadStrip {
    pub kind: RoadKind,
    pub centerline: Vec<Point>,
    /// Full strip width, inches.
    pub width: f64,
    pub curb: bool,
    /// How much higher the centerline sits than the edges (a crowned road
    /// sheds water to both curbs), inches.
    pub crown: f64,
    /// Height of the curb blocks, inches.
    pub curb_height: f64,
    /// Material by name ("Asphalt", "Concrete", "Gravel", "Stone", "Brick",
    /// "Paint"); empty takes the kind's own ([`RoadKind::default_material`]).
    pub material: String,
    /// A dashed marking (a stripe drawn as dashes).
    pub dashed: bool,
    /// Color of a marking in plan, RGB; `None` is a traffic yellow.
    pub color: Option<[u8; 3]>,
    /// Layer the strip is drawn on; empty draws it on the terrain's own layer.
    pub layer: String,
    /// A polyline road, driveway or sidewalk (and a median or cul-de-sac) is
    /// a closed outline instead of a centerline strip: when this has three
    /// or more points it is the shape and `centerline` is ignored.
    pub outline: Vec<Point>,
    /// Center of a cul-de-sac.
    pub center: Point,
    /// Radius of a cul-de-sac, inches (0 for every other kind).
    pub radius: f64,
    /// How far the top stands above the terrain, inches (negative sinks it).
    pub to_top: f64,
    /// Thickness of the slab, inches (0 draws a thin sheet on the ground).
    pub thickness: f64,
    /// Flare at the start of the strip: radius of the fillet where it meets
    /// another road, inches (`None` has no flare).
    pub flare_start: Option<f64>,
    /// Flare at the end of the strip.
    pub flare_end: Option<f64>,
    /// Width of the curb blocks, inches.
    pub curb_width: f64,
    /// Leave a gap in the curb where a driveway or sidewalk crosses it.
    pub cut_curb: bool,
    /// Label, schedule category and object information.
    pub extras: ObjectExtras,
}

/// Default width of a road marking, inches.
pub const MARKING_WIDTH: f64 = 4.0;
/// Dash and gap of a dashed marking, inches.
pub const MARKING_DASH: f64 = 120.0;
pub const MARKING_GAP: f64 = 240.0;

impl RoadStrip {
    /// The material name the strip is built of.
    pub fn material_name(&self) -> &str {
        if self.material.trim().is_empty() {
            self.kind.default_material()
        } else {
            self.material.trim()
        }
    }

    /// The layer the strip names for itself, if any.
    pub fn own_layer(&self) -> Option<&str> {
        Some(self.layer.trim()).filter(|l| !l.is_empty())
    }

    /// A road marking along `centerline`, `width` inches wide.
    pub fn marking(centerline: Vec<Point>, width: f64, dashed: bool) -> Self {
        RoadStrip {
            kind: RoadKind::Marking,
            centerline,
            width,
            dashed,
            ..RoadStrip::default()
        }
    }
}

impl Default for RoadStrip {
    fn default() -> Self {
        RoadStrip {
            kind: RoadKind::Road,
            centerline: Vec::new(),
            width: 240.0,
            curb: false,
            crown: 0.0,
            curb_height: 6.0,
            material: String::new(),
            dashed: false,
            color: None,
            layer: String::new(),
            outline: Vec::new(),
            center: Point::new(0.0, 0.0),
            radius: 0.0,
            to_top: 0.0,
            thickness: 0.0,
            flare_start: None,
            flare_end: None,
            curb_width: DEFAULT_CURB_WIDTH,
            cut_curb: true,
            extras: ObjectExtras::default(),
        }
    }
}

/// Default width of the curb blocks, inches.
pub const DEFAULT_CURB_WIDTH: f64 = 6.0;

/// The building pad: the terrain under the house is levelled to the first
/// floor less the plan's "terrain to first floor" distance
/// ([`Terrain::subfloor_height_above_terrain`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildingPad {
    /// The building outline (foundation or first floor walls).
    pub footprint: Vec<Point>,
    /// How far the level pad reaches beyond the footprint, inches.
    pub margin: f64,
    /// Elevation of the first floor, inches. `None` levels the pad at the mean
    /// existing ground under it (a balanced cut and fill).
    pub first_floor: Option<f64>,
    /// Run per unit of rise of the pad's sloped sides.
    pub slope_ratio: f64,
}

impl Default for BuildingPad {
    fn default() -> Self {
        BuildingPad {
            footprint: Vec::new(),
            margin: 24.0,
            first_floor: None,
            slope_ratio: DEFAULT_SLOPE_RATIO,
        }
    }
}

/// How a family of contour lines is drawn in plan (Terrain Specification >
/// Contours: primary lines are the majors, secondary the lines between them).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ContourStyle {
    /// `None` takes the plan's terrain color.
    pub color: Option<[u8; 3]>,
    /// Line weight in points; `0` takes the family's own weight.
    pub weight: f64,
    pub dashed: bool,
}

/// Default weight of the primary (major) contours, points.
pub const PRIMARY_CONTOUR_WEIGHT: f64 = 1.0;
/// Default weight of the secondary (minor) contours, points.
pub const SECONDARY_CONTOUR_WEIGHT: f64 = 0.35;

impl ContourStyle {
    /// The weight to draw with: the style's, or `default` when it takes the family's own.
    pub fn weight_or(&self, default: f64) -> f64 {
        if self.weight > 0.0 {
            self.weight
        } else {
            default
        }
    }
}

/// Every this-many'th contour is a major contour.
pub const DEFAULT_MAJOR_EVERY: u32 = 5;
/// Default distance between elevation labels along a major contour, inches.
pub const DEFAULT_LABEL_SPACING: f64 = 480.0;

/// Material of the ground surface until the specification names another.
pub const DEFAULT_GROUND_MATERIAL: &str = "Grass";
/// Material of the bare ground until the specification names another.
pub const DEFAULT_DIRT_MATERIAL: &str = "Dirt";

/// Everything the user draws for the terrain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Terrain {
    /// Terrain Perimeter, a closed polygon (the closing point is implicit).
    pub perimeter: Vec<Point>,
    pub elevation_points: Vec<ElevationPoint>,
    pub elevation_lines: Vec<ElevationLine>,
    pub elevation_regions: Vec<ElevationRegion>,
    pub modifiers: Vec<Modifier>,
    pub features: Vec<Feature>,
    pub roads: Vec<RoadStrip>,
    /// Terrain Break lines: flat creases that stay sharp in the surface.
    pub breaks: Vec<TerrainBreak>,
    /// Terrain walls and curbs (retaining walls that follow the ground).
    pub walls: Vec<TerrainWall>,
    /// Garden beds, grass, water, stepping stones, plant runs and sprinklers.
    pub landscape: Vec<Landscape>,
    /// Chief default 6": how far the subfloor sits above the terrain at the building.
    pub subfloor_height_above_terrain: f64,
    /// Elevation of the building pad, inches.
    pub building_pad_elevation: f64,
    /// Laplacian smoothing iterations applied to interior vertices (default 0).
    pub smoothing: u32,
    /// Spacing of the sample grid, inches (default 120").
    pub grid_spacing: f64,
    /// The sample grid is divided this many times finer (1 = as is).
    pub subdivision: u32,
    /// Every this-many'th contour is a major contour.
    pub contour_major_every: u32,
    /// Distance between elevation labels along a contour, inches (0 = one
    /// label per contour line).
    pub contour_label_spacing: f64,
    /// Label only the major contours.
    pub contour_label_major_only: bool,
    /// Label the primary (major) contours at all.
    pub label_primary: bool,
    /// The building pad, levelled under the house.
    pub building_pad: Option<BuildingPad>,
    /// Level the building pad (when one is defined).
    pub flatten_pad: bool,
    /// Compass rotation of the plan: degrees clockwise from the top of the
    /// plan to true north (the North Pointer). The sun angle and the compass
    /// labels use it.
    pub north_angle: f64,
    /// The placed North Pointer (its CAD objects on the Site Plan layer).
    pub north_pointer: Option<crate::site_symbols::SiteMark>,
    /// Material of the ground surface (Terrain Specification > Materials):
    /// "Grass", "Dirt", "Gravel", "Stone", "Concrete", "Mulch" or "Asphalt".
    pub ground_material: String,
    /// Material of the bare ground: the cut slopes of graded pads and the strip under walls.
    pub dirt_material: String,
    /// Line style of the primary (major) contours.
    pub contour_primary: ContourStyle,
    /// Line style of the secondary (minor) contours.
    pub contour_secondary: ContourStyle,
    /// Absolute Elevation: how the terrain meets Floor 1.
    pub absolute_elevation: AbsoluteElevation,
    /// The placed Terrain Elevation Reference Point (`None` uses the middle of
    /// the perimeter's box).
    pub reference_point: Option<Point>,
    /// Vertical distance from the Floor 1 subfloor to the surface at the
    /// Reference Point or at contour 0 (normally negative), inches.
    pub surface_offset: f64,
    /// Elevation of the Floor 1 subfloor the retained modes measure from, inches.
    pub floor_one_elevation: f64,
    /// The 3D skirt around the terrain edge.
    pub skirt: Skirt,
    /// Leave the terrain out inside the building (Hide Terrain Intersected by
    /// Building): no surface or contours under the house.
    pub hide_under_building: bool,
    /// Terrain Surface Smoothing.
    pub smoothing_level: SmoothingLevel,
    /// Surface triangle detail.
    pub triangle_detail: TriangleDetail,
    /// Triangle count of the Custom detail.
    pub custom_triangles: u32,
    /// Largest triangle edge, inches (0 = not used).
    pub max_triangle_size: f64,
    /// Contour offset: contours fall at the offset plus whole intervals, inches.
    pub contour_offset: f64,
    /// Units of the contour labels.
    pub contour_label_units: LabelUnits,
    /// Label the contours below elevation 0 in red.
    pub highlight_negative: bool,
    /// Smooth the contour lines in 2D.
    pub contour_smoothing: bool,
    /// Passes of the 2D contour smoothing.
    pub contour_smooth_passes: u32,
    /// What the last Build Terrain made (`None` before the first build and
    /// after Clear Terrain).
    pub last_build: Option<BuildStats>,
    /// Label, schedule category and object information of the perimeter.
    pub perimeter_extras: ObjectExtras,
    /// The season plant images are shown in.
    pub season: crate::plants::Season,
    /// Extras of the objects that do not carry them (points, regions, modifiers).
    pub side_extras: crate::spec::SideExtras,
}

impl Default for Terrain {
    /// A flat 100' x 80' lot at elevation 0 with no data.
    fn default() -> Self {
        Terrain {
            perimeter: vec![
                Point::new(0.0, 0.0),
                Point::new(1200.0, 0.0),
                Point::new(1200.0, 960.0),
                Point::new(0.0, 960.0),
            ],
            elevation_points: Vec::new(),
            elevation_lines: Vec::new(),
            elevation_regions: Vec::new(),
            modifiers: Vec::new(),
            features: Vec::new(),
            roads: Vec::new(),
            breaks: Vec::new(),
            walls: Vec::new(),
            landscape: Vec::new(),
            subfloor_height_above_terrain: 6.0,
            building_pad_elevation: 0.0,
            smoothing: 0,
            grid_spacing: 120.0,
            subdivision: 1,
            contour_major_every: DEFAULT_MAJOR_EVERY,
            contour_label_spacing: DEFAULT_LABEL_SPACING,
            contour_label_major_only: true,
            label_primary: true,
            building_pad: None,
            flatten_pad: true,
            north_angle: 0.0,
            north_pointer: None,
            ground_material: DEFAULT_GROUND_MATERIAL.into(),
            dirt_material: DEFAULT_DIRT_MATERIAL.into(),
            contour_primary: ContourStyle::default(),
            contour_secondary: ContourStyle::default(),
            absolute_elevation: AbsoluteElevation::Automatic,
            reference_point: None,
            surface_offset: DEFAULT_SURFACE_OFFSET,
            floor_one_elevation: 0.0,
            skirt: Skirt::default(),
            hide_under_building: false,
            smoothing_level: SmoothingLevel::Passes,
            triangle_detail: TriangleDetail::Grid,
            custom_triangles: DEFAULT_CUSTOM_TRIANGLES,
            max_triangle_size: 0.0,
            contour_offset: 0.0,
            contour_label_units: LabelUnits::FeetInches,
            highlight_negative: false,
            contour_smoothing: false,
            contour_smooth_passes: DEFAULT_CONTOUR_SMOOTH_PASSES,
            last_build: None,
            perimeter_extras: ObjectExtras::default(),
            season: crate::plants::Season::Summer,
            side_extras: Default::default(),
        }
    }
}

impl Terrain {
    /// Finished floor elevation: the building pad plus the subfloor height above terrain.
    pub fn finished_floor_elevation(&self) -> f64 {
        self.building_pad_elevation + self.subfloor_height_above_terrain
    }
}

/// The regular sample grid the surface was built from (row-major, `z[j * nx + i]`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HeightGrid {
    /// Plan position of node `(0, 0)`.
    pub origin: Point,
    pub spacing: f64,
    pub nx: usize,
    pub ny: usize,
    /// Elevation (after modifiers) at every node, including nodes outside the perimeter.
    pub z: Vec<f64>,
}

impl HeightGrid {
    /// Elevation at node `(i, j)`, or `None` when out of range.
    pub fn at(&self, i: usize, j: usize) -> Option<f64> {
        (i < self.nx && j < self.ny)
            .then(|| self.z.get(j * self.nx + i).copied())
            .flatten()
    }

    /// Plan position of node `(i, j)`.
    pub fn node(&self, i: usize, j: usize) -> Point {
        Point::new(
            self.origin.x + i as f64 * self.spacing,
            self.origin.y + j as f64 * self.spacing,
        )
    }
}

/// A triangulated terrain surface.
///
/// `vertices` are `[plan x, elevation, plan y]`; `triangles` index them and are
/// counter-clockwise in plan (so they face up). The point-location index used by
/// [`crate::elevation_at`] is built lazily on first query, so do not mutate
/// `vertices`/`triangles` after querying.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerrainSurface {
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub grid: HeightGrid,
    #[serde(skip)]
    index: OnceLock<TriIndex>,
}

impl TerrainSurface {
    /// Assemble a surface from its parts.
    pub fn new(vertices: Vec<[f64; 3]>, triangles: Vec<[u32; 3]>, grid: HeightGrid) -> Self {
        TerrainSurface {
            vertices,
            triangles,
            grid,
            index: OnceLock::new(),
        }
    }

    /// Plan position of vertex `i`.
    pub fn plan_point(&self, i: u32) -> Point {
        let v = self.vertices[i as usize];
        Point::new(v[0], v[2])
    }

    pub(crate) fn tri_index(&self) -> &TriIndex {
        self.index
            .get_or_init(|| TriIndex::build(&self.vertices, &self.triangles))
    }
}
