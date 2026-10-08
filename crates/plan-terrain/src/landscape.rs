//! Landscape objects of the Terrain menu: break lines, terrain walls and curbs,
//! garden beds, grass regions, water features, stepping stones, plant runs and
//! sprinklers (CB-44, CB-46, CB-48..CB-50), plus the outline helpers the drawing
//! tools use (rectangle, kidney, arc).
//!
//! Everything is stored flattened: a spline or a curved wall is kept as the
//! polyline the tool made from its control points, `shape` only remembers how it
//! was drawn.

use std::f64::consts::PI;

use plan_core::{Point, WallCurve};
use serde::{Deserialize, Serialize};

use crate::geom::flatten_spline;

/// Default layer of the terrain features (rectangular, kidney, spline).
pub const LAYER_FEATURES: &str = "Terrain, Features";
pub const LAYER_WALLS: &str = "Terrain, Walls";
pub const LAYER_BREAKS: &str = "Terrain, Breaks";
pub const LAYER_BEDS: &str = "Landscaping, Garden Beds";
pub const LAYER_GRASS: &str = "Landscaping, Grass Regions";
pub const LAYER_WATER: &str = "Landscaping, Water Features";
pub const LAYER_STONES: &str = "Landscaping, Stepping Stones";
pub const LAYER_PLANTS: &str = "Plants";
pub const LAYER_SPRINKLERS: &str = "Sprinklers";

/// How a closed region is filled in plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FillStyle {
    /// The object kind's own look.
    #[default]
    Default,
    None,
    Solid,
    Hatch,
    /// A tint with rows of wavy lines, the water look.
    Ripple,
}

/// Layer, line style and fill style shared by every terrain object.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectStyle {
    /// Empty means the kind's default layer.
    pub layer: String,
    /// `None` takes the kind's color.
    pub line_color: Option<[u8; 3]>,
    /// Line weight in points; `0` takes the kind's weight.
    pub line_weight: f64,
    pub dashed: bool,
    pub fill: FillStyle,
    /// `None` takes the kind's color.
    pub fill_color: Option<[u8; 3]>,
}

impl ObjectStyle {
    /// The explicit layer, or `default`.
    pub fn layer_or<'a>(&'a self, default: &'a str) -> &'a str {
        if self.layer.trim().is_empty() {
            default
        } else {
            &self.layer
        }
    }
}

/// A Terrain Break: a line held at one elevation whose vertices stay fixed when
/// the surface is smoothed, so the crease survives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TerrainBreak {
    pub points: Vec<Point>,
    /// Elevation of the break line, inches.
    pub z: f64,
    pub style: ObjectStyle,
}

impl Default for TerrainBreak {
    fn default() -> Self {
        TerrainBreak {
            points: Vec::new(),
            z: 0.0,
            style: ObjectStyle::default(),
        }
    }
}

/// Terrain wall or terrain curb.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WallKind {
    #[default]
    Wall,
    Curb,
}

/// A retaining wall or curb that follows the ground: its top is `height` above
/// the terrain along the whole path and its bottom `depth` below it.
///
/// A wall cuts the terrain surface: when the terrain is built the strip under
/// the wall is a gap with vertical faces, the surface on the left of the path
/// (the retained side) keeps its grade and the surface on the right (the cut
/// side) is lowered by `retain` at the wall, sloping back up to the existing
/// ground at 1:4. Contours stop at the wall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TerrainWall {
    pub kind: WallKind,
    /// Centerline, flattened (a curved wall is its arc).
    pub points: Vec<Point>,
    pub curved: bool,
    /// Top above the terrain, inches.
    pub height: f64,
    /// Bottom below the terrain (footing), inches.
    pub depth: f64,
    pub thickness: f64,
    /// "Concrete", "Stone" or "Brick".
    pub material: String,
    pub style: ObjectStyle,
    /// How much lower the surface on the right of the wall is than on its
    /// left, inches (0 = the same grade both sides; negative cuts the left).
    pub retain: f64,
    /// Cut the surface along the wall (a gap with the wall's vertical faces).
    pub cut: bool,
    /// A stepped retaining wall: the top holds level over a stretch and drops
    /// in `step` inch courses as the ground falls, instead of following every
    /// bump of the ground.
    pub stepped: bool,
    /// Height of one course of a stepped wall, inches.
    pub step: f64,
}

/// Height of one course of a stepped wall until the specification changes it
/// (a standard 8" block).
pub const DEFAULT_WALL_STEP: f64 = 8.0;

/// Slope ratio of the graded ground on the cut side of a wall (4 = 1:4).
pub const WALL_SLOPE_RATIO: f64 = 4.0;

impl TerrainWall {
    pub fn new(kind: WallKind, points: Vec<Point>, curved: bool) -> Self {
        let (height, depth, thickness) = match kind {
            WallKind::Wall => (36.0, 12.0, 8.0),
            WallKind::Curb => (6.0, 4.0, 6.0),
        };
        TerrainWall {
            kind,
            points,
            curved,
            height,
            depth,
            thickness,
            material: "Concrete".into(),
            style: ObjectStyle::default(),
            retain: 0.0,
            cut: true,
            stepped: false,
            step: DEFAULT_WALL_STEP,
        }
    }

    pub fn default_layer(&self) -> &str {
        LAYER_WALLS
    }

    /// Horizontal reach of the graded ground on the cut side, inches.
    pub fn reach(&self) -> f64 {
        (self.retain.abs() * WALL_SLOPE_RATIO).max(12.0)
    }
}

impl Default for TerrainWall {
    fn default() -> Self {
        TerrainWall::new(WallKind::Wall, Vec::new(), false)
    }
}

/// The landscaping objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LandscapeKind {
    #[default]
    GardenBed,
    GrassRegion,
    WaterFeature,
    SteppingStones,
    Plants,
    Sprinklers,
}

/// How the outline was drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShapeKind {
    #[default]
    Polyline,
    Kidney,
    Spline,
}

/// How a plant is built in the 3D view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlantForm {
    /// From the plant: conifers (spruce, pine, cedar, arborvitae, ...) are
    /// cones, other tall plants trees on a trunk, the rest round shrubs.
    #[default]
    Auto,
    /// A round canopy (a shrub, or a tree on a trunk when tall).
    Round,
    /// A cone: an evergreen.
    Cone,
    /// Two crossed upright planes: a flat cut-out of a plant, cheap to draw.
    Billboard,
}

impl PlantForm {
    pub const ALL: [PlantForm; 4] = [
        PlantForm::Auto,
        PlantForm::Round,
        PlantForm::Cone,
        PlantForm::Billboard,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PlantForm::Auto => "Automatic",
            PlantForm::Round => "Round canopy",
            PlantForm::Cone => "Cone (evergreen)",
            PlantForm::Billboard => "Billboard",
        }
    }
}

/// Words in a plant's name or catalog id that make it a conifer.
const CONIFER_WORDS: [&str; 9] = [
    "spruce",
    "pine",
    "cedar",
    "fir",
    "arborvitae",
    "juniper",
    "cypress",
    "evergreen",
    "conifer",
];

/// Whether `text` (a plant name or catalog id) names a conifer.
pub fn is_conifer(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.split(|c: char| !c.is_ascii_alphabetic())
        .any(|w| CONIFER_WORDS.contains(&w))
}

/// One landscaping object. The numeric fields mean (inches unless noted):
///
/// | kind | `height` | `size` | `spacing` | `depth` | `edging` |
/// |---|---|---|---|---|---|
/// | garden bed | mulch depth above grade | edging height | - | - | edging on |
/// | grass region | lift above grade | - | - | - | - |
/// | water feature | water level below grade | edge (coping) width | - | basin depth below the water | edge on |
/// | stepping stones | stone thickness | stone size | center to center | - | - |
/// | plants | plant height | canopy width | center to center | - | - |
/// | sprinklers | riser height | spray radius | head to head | - | - |
///
/// `arc` is the spray angle of a sprinkler head in degrees, `plant` the catalog
/// id of the plant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Landscape {
    pub kind: LandscapeKind,
    pub shape: ShapeKind,
    /// Region outline (closed implicitly) or path.
    pub points: Vec<Point>,
    pub material: String,
    pub height: f64,
    pub size: f64,
    pub spacing: f64,
    pub depth: f64,
    pub edging: bool,
    pub plant: String,
    pub arc: f64,
    pub style: ObjectStyle,
    /// Control points of a kidney or spline outline: `points` is the spline
    /// through them (closed for regions). Empty for a clicked polyline.
    pub control: Vec<Point>,
    /// How a plant is built in 3D.
    pub form: PlantForm,
}

impl Landscape {
    /// The form a plant is built with: its own, or for `Auto` a cone for a
    /// conifer and a round canopy for the rest.
    pub fn plant_form(&self) -> PlantForm {
        match self.form {
            PlantForm::Auto if is_conifer(&self.plant) => PlantForm::Cone,
            PlantForm::Auto => PlantForm::Round,
            f => f,
        }
    }
}

impl Landscape {
    /// A new object with Chief-like defaults for its kind.
    pub fn new(kind: LandscapeKind, shape: ShapeKind, points: Vec<Point>) -> Self {
        let base = Landscape {
            kind,
            shape,
            points,
            material: String::new(),
            height: 0.0,
            size: 0.0,
            spacing: 0.0,
            depth: 0.0,
            edging: false,
            plant: String::new(),
            arc: 360.0,
            style: ObjectStyle::default(),
            control: Vec::new(),
            form: PlantForm::Auto,
        };
        match kind {
            LandscapeKind::GardenBed => Landscape {
                material: "Mulch".into(),
                height: 3.0,
                size: 4.0,
                edging: true,
                ..base
            },
            LandscapeKind::GrassRegion => Landscape {
                material: "Grass".into(),
                height: 0.5,
                ..base
            },
            LandscapeKind::WaterFeature => Landscape {
                material: "Water".into(),
                height: 6.0,
                size: 6.0,
                depth: 24.0,
                edging: true,
                ..base
            },
            LandscapeKind::SteppingStones => Landscape {
                material: "Stone".into(),
                height: 3.0,
                size: 18.0,
                spacing: 30.0,
                ..base
            },
            LandscapeKind::Plants => Landscape {
                height: 36.0,
                size: 36.0,
                spacing: 36.0,
                ..base
            },
            LandscapeKind::Sprinklers => Landscape {
                height: 4.0,
                size: 144.0,
                spacing: 144.0,
                ..base
            },
        }
    }

    /// Garden beds, grass and water are closed regions; the rest are paths.
    pub fn is_region(&self) -> bool {
        matches!(
            self.kind,
            LandscapeKind::GardenBed | LandscapeKind::GrassRegion | LandscapeKind::WaterFeature
        )
    }

    pub fn default_layer(&self) -> &'static str {
        match self.kind {
            LandscapeKind::GardenBed => LAYER_BEDS,
            LandscapeKind::GrassRegion => LAYER_GRASS,
            LandscapeKind::WaterFeature => LAYER_WATER,
            LandscapeKind::SteppingStones => LAYER_STONES,
            LandscapeKind::Plants => LAYER_PLANTS,
            LandscapeKind::Sprinklers => LAYER_SPRINKLERS,
        }
    }

    pub fn layer(&self) -> &str {
        self.style.layer_or(self.default_layer())
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            LandscapeKind::GardenBed => "Garden Bed",
            LandscapeKind::GrassRegion => "Grass Region",
            LandscapeKind::WaterFeature => "Water Feature",
            LandscapeKind::SteppingStones => "Stepping Stones",
            LandscapeKind::Plants => "Plants",
            LandscapeKind::Sprinklers => "Sprinklers",
        }
    }

    /// Rebuilds `points` from the control points (a closed spline for regions,
    /// an open one for paths). Does nothing without control points.
    pub fn reflatten(&mut self) {
        if self.control.len() >= 3 {
            self.points = if self.is_region() {
                closed_spline(&self.control)
            } else {
                open_spline(&self.control)
            };
        }
    }

    /// Stone centers with the direction of travel.
    pub fn stones(&self) -> Vec<(Point, f64)> {
        stepping_stones(&self.points, self.spacing)
    }

    /// Plant centers along the path.
    pub fn plant_positions(&self) -> Vec<Point> {
        distribute_along(&self.points, self.spacing)
            .into_iter()
            .map(|(p, _)| p)
            .collect()
    }

    /// Sprinkler heads with the direction their spray faces.
    pub fn heads(&self) -> Vec<(Point, f64)> {
        sprinkler_heads(&self.points, self.spacing)
    }
}

impl Default for Landscape {
    fn default() -> Self {
        Landscape::new(LandscapeKind::GardenBed, ShapeKind::Polyline, Vec::new())
    }
}

// ----- geometry helpers -----

/// Length of an open polyline, inches.
pub fn path_length(pts: &[Point]) -> f64 {
    pts.windows(2).map(|w| w[0].dist(w[1])).sum()
}

/// The point at `dist` along the path and the direction there (radians).
fn point_at(pts: &[Point], dist: f64) -> Option<(Point, f64)> {
    let mut left = dist.max(0.0);
    let mut last = None;
    for w in pts.windows(2) {
        let len = w[0].dist(w[1]);
        if len < 1e-9 {
            continue;
        }
        let dir = w[1].sub(w[0]).angle();
        if left <= len {
            return Some((Point::lerp(w[0], w[1], left / len), dir));
        }
        left -= len;
        last = Some((w[1], dir));
    }
    last
}

/// Points every `spacing` inches along the path, starting at its first point:
/// `floor(length / spacing) + 1` of them, each with the direction of travel.
pub fn distribute_along(pts: &[Point], spacing: f64) -> Vec<(Point, f64)> {
    let len = path_length(pts);
    if spacing <= 0.0 || len <= 0.0 {
        return pts.first().map(|p| vec![(*p, 0.0)]).unwrap_or_default();
    }
    let n = (len / spacing + 1e-9).floor() as usize + 1;
    (0..n)
        .filter_map(|i| point_at(pts, i as f64 * spacing))
        .collect()
}

/// Stepping stones: one per `spacing` of path, each centered in its cell, so a
/// path of `n * spacing` carries `n` stones (a shorter, non-empty path gets one).
pub fn stepping_stones(pts: &[Point], spacing: f64) -> Vec<(Point, f64)> {
    let len = path_length(pts);
    if spacing <= 0.0 || len <= 0.0 {
        return Vec::new();
    }
    let n = ((len / spacing + 1e-9).floor() as usize).max(1);
    // Spread the cells over the real length so the stones stay centered.
    let cell = len / n as f64;
    (0..n)
        .filter_map(|i| point_at(pts, (i as f64 + 0.5) * cell))
        .collect()
}

/// Sprinkler heads every `spacing` inches, both ends included: the heads of
/// [`distribute_along`] with their spray facing the left of the path.
pub fn sprinkler_heads(pts: &[Point], spacing: f64) -> Vec<(Point, f64)> {
    distribute_along(pts, spacing)
        .into_iter()
        .map(|(p, dir)| (p, dir + PI / 2.0))
        .collect()
}

/// The dashes of a dashed line along `pts`: pieces `dash` inches long with
/// `gap` inches between them, the last one cut short at the end of the path.
pub fn dash_path(pts: &[Point], dash: f64, gap: f64) -> Vec<Vec<Point>> {
    let len = path_length(pts);
    if dash <= 0.0 || len <= 0.0 {
        return Vec::new();
    }
    let period = dash + gap.max(0.0);
    let mut out = Vec::new();
    let mut start = 0.0;
    while start < len - 1e-6 {
        let end = (start + dash).min(len);
        let mut piece = Vec::new();
        if let Some((p, _)) = point_at(pts, start) {
            piece.push(p);
        }
        // Original vertices strictly inside the dash keep its corners.
        let mut run = 0.0;
        for w in pts.windows(2) {
            run += w[0].dist(w[1]);
            if run > start + 1e-6 && run < end - 1e-6 {
                piece.push(w[1]);
            }
        }
        if let Some((p, _)) = point_at(pts, end) {
            piece.push(p);
        }
        if piece.len() >= 2 {
            out.push(piece);
        }
        start += period;
    }
    out
}

/// The four corners of the rectangle with opposite corners `a` and `b`.
pub fn rectangle_outline(a: Point, b: Point) -> Vec<Point> {
    vec![
        Point::new(a.x, a.y),
        Point::new(b.x, a.y),
        Point::new(b.x, b.y),
        Point::new(a.x, b.y),
    ]
}

/// A kidney-shaped blob from three clicks: `a` and `b` are the two ends of its
/// long axis and `c` sets the width on its rounded side; the notch is on the
/// opposite side. `None` when the axis or the width is degenerate.
pub fn kidney_outline(a: Point, b: Point, c: Point) -> Option<Vec<Point>> {
    const STEPS: usize = 40;
    const NOTCH: f64 = 0.55;
    let axis = b.sub(a);
    let len = axis.length();
    if len < 1.0 {
        return None;
    }
    let u = axis.scale(1.0 / len);
    let n = u.perp();
    let mid = Point::lerp(a, b, 0.5);
    let side = c.sub(mid).dot(n);
    let half_w = side.abs();
    if half_w < 1.0 {
        return None;
    }
    // The rounded side is where the click is; the notch faces away from it.
    let sign = side.signum();
    let a_len = len / 2.0;
    let pts = (0..STEPS)
        .map(|i| {
            let t = 2.0 * PI * i as f64 / STEPS as f64;
            let (x, mut y) = (a_len * t.cos(), half_w * t.sin());
            if y > 0.0 {
                y -= NOTCH * half_w * t.sin() * t.sin();
            }
            mid.add(u.scale(x)).add(n.scale(-sign * y))
        })
        .collect();
    Some(pts)
}

/// Control points of a kidney: every fourth point of [`kidney_outline`]. The
/// closed spline through them ([`closed_spline`]) is the editable outline.
pub fn kidney_control_points(a: Point, b: Point, c: Point) -> Option<Vec<Point>> {
    let outline = kidney_outline(a, b, c)?;
    Some(outline.into_iter().step_by(4).collect())
}

/// The arc from `start` to `end` that bulges `bulge` inches at its middle (the
/// curved wall's rule), as a polyline with `segments` pieces.
pub fn arc_polyline(start: Point, end: Point, bulge: f64, segments: usize) -> Vec<Point> {
    WallCurve { bulge }.sample_points(start, end, segments)
}

/// A closed Catmull-Rom blob through `control`.
pub fn closed_spline(control: &[Point]) -> Vec<Point> {
    flatten_spline(control, true, 8)
}

/// An open Catmull-Rom curve through `control`.
pub fn open_spline(control: &[Point]) -> Vec<Point> {
    flatten_spline(control, false, 8)
}
