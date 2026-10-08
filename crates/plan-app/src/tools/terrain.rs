//! Terrain tools (CB-51..CB-53 in `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! One tool object with a flavor per Terrain menu entry ([`TerrainVariant`]):
//!
//! * Terrain Perimeter: click, click, ... ; Enter (or a double-click, or a
//!   click on the first point) closes it. There is one per project (CB-51);
//! * Elevation Point: click, then type the elevation in the inline field;
//! * Elevation Line, Road, Driveway, Sidewalk: click a polyline, Enter or
//!   double-click ends it, then the elevation or width is typed;
//! * Elevation Region, Hill, Valley, Raised, Lowered and Flat Region, Terrain
//!   Hole: click a polygon, Enter closes it, then the elevation or height is
//!   typed (Flat Region and Terrain Hole need no value);
//! * Build Terrain: one click builds the surface and shows the contours (CB-53);
//! * Elevation Spline, Spline Road/Driveway/Sidewalk: like their polyline
//!   versions, with the clicks as control points of a Catmull-Rom curve;
//! * Terrain Break: a polyline, then the elevation it is held at;
//! * Straight Terrain Wall/Curb: a polyline; Curved Terrain Wall/Curb: start,
//!   end, then a click that sets the bulge of the arc;
//! * Rectangular Feature: drag a rectangle (or click two corners); Kidney
//!   Shaped Feature, Kidney Garden Bed, Kidney Grass Region: click both ends
//!   of the long axis, then a third click that sets the width; Spline Feature
//!   and the Spline Garden Bed, Grass Region and Water Feature: closed splines;
//! * Garden Bed, Grass Region, Water Feature: polygons; Stepping Stone, Plant
//!   and Sprinkler: polylines or splines the objects are laid along. They take
//!   their sizes from defaults; the Specification (double-click the object)
//!   edits them (`dialogs::terrain::ObjectDialog`, one undo step).
//!
//! The typed value goes through `cx.temp.editing` (the shell forwards typed
//! text while it is set); Enter accepts, an empty field takes the default
//! shown, Esc cancels. Delete removes the element under the pointer, the arrow
//! keys nudge it; a double-click on an object with a specification opens it,
//! one elsewhere (outside a drawing) opens the Terrain Specification.
//!
//! The terrain lives in `Project.terrain` through `editor::site_view`;
//! `site_view::draw_site` draws it.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::terrain::{ObjectDialog, TerrainDialog};
use crate::dialogs::Outcome;
use crate::editor::site_view::{
    draw_polyline, edit_terrain, ensure_landscape_layers, hit_terrain, load_terrain,
    move_terrain_element, object_at, remove_terrain_element, replace_object, terrain_view,
    TerrainHit, TerrainObject, TerrainRecord,
};
use crate::editor::tempdim::EditField;
use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Color32, FontId, Key, Pos2, Rect};
use plan_core::geometry::Point;
use plan_core::units::{fmt_ft_in, parse_ft_in};
use plan_core::WallCurve;
use plan_terrain::{
    arc_polyline, flatten_spline, kidney_outline, rectangle_outline, ElevationLine, ElevationPoint,
    ElevationRegion, Feature, FeatureKind, Landscape, LandscapeKind, Modifier, ModifierKind,
    RoadKind, RoadStrip, ShapeKind, TerrainBreak, TerrainWall, WallKind,
};
use std::cell::RefCell;

mod scape;
pub use scape::{apply_plant, default_plant, plant_choices};

/// Default Hill/Valley height, inches.
const DEFAULT_HILL: f64 = 72.0;
/// Default Raised/Lowered Region height, inches.
const DEFAULT_REGION_SHIFT: f64 = 24.0;
const DEFAULT_ROAD_WIDTH: f64 = 240.0;
const DEFAULT_DRIVEWAY_WIDTH: f64 = 144.0;
const DEFAULT_SIDEWALK_WIDTH: f64 = 48.0;
/// Default height of a new terrain feature slab, inches.
const DEFAULT_FEATURE_HEIGHT: f64 = 4.0;
/// Points per span of a flattened spline.
const SPLINE_SAMPLES: usize = 8;
/// The `EditField` index that marks "the terrain tool's inline value".
const ENTRY_INDEX: usize = usize::MAX;

/// The flavors of the terrain tool, one per menu entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainVariant {
    Perimeter,
    ElevationPoint,
    ElevationLine,
    ElevationRegion,
    Hill,
    Valley,
    Raised,
    Lowered,
    Flat,
    Road,
    Driveway,
    Sidewalk,
    Hole,
    Build,
    ElevationSpline,
    Break,
    StraightWall,
    StraightCurb,
    CurvedWall,
    CurvedCurb,
    RectFeature,
    KidneyFeature,
    SplineFeature,
    BedPolyline,
    BedKidney,
    BedSpline,
    GrassPolyline,
    GrassKidney,
    GrassSpline,
    WaterPolyline,
    WaterSpline,
    StonePolyline,
    StoneSpline,
    SplineRoad,
    SplineDriveway,
    SplineSidewalk,
    PlantPolyline,
    PlantSpline,
    SprinklerPolyline,
    SprinklerSpline,
}

/// What the clicks of a flavor draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Draw {
    Spot,
    Polyline,
    Polygon,
    Command,
    /// Drag (or click two corners) for a rectangle.
    Rect,
    /// Three clicks: both ends of the long axis, then the width.
    Kidney,
    /// Three clicks: start, end, then the bulge of the arc.
    Arc,
}

impl TerrainVariant {
    /// Every flavor, in menu order.
    pub const ALL: [TerrainVariant; 40] = [
        TerrainVariant::Perimeter,
        TerrainVariant::ElevationPoint,
        TerrainVariant::ElevationLine,
        TerrainVariant::ElevationRegion,
        TerrainVariant::ElevationSpline,
        TerrainVariant::Break,
        TerrainVariant::Build,
        TerrainVariant::Hill,
        TerrainVariant::Valley,
        TerrainVariant::Raised,
        TerrainVariant::Lowered,
        TerrainVariant::Flat,
        TerrainVariant::RectFeature,
        TerrainVariant::KidneyFeature,
        TerrainVariant::SplineFeature,
        TerrainVariant::Hole,
        TerrainVariant::BedPolyline,
        TerrainVariant::BedKidney,
        TerrainVariant::BedSpline,
        TerrainVariant::GrassPolyline,
        TerrainVariant::GrassKidney,
        TerrainVariant::GrassSpline,
        TerrainVariant::WaterPolyline,
        TerrainVariant::WaterSpline,
        TerrainVariant::StonePolyline,
        TerrainVariant::StoneSpline,
        TerrainVariant::StraightWall,
        TerrainVariant::StraightCurb,
        TerrainVariant::CurvedWall,
        TerrainVariant::CurvedCurb,
        TerrainVariant::Road,
        TerrainVariant::SplineRoad,
        TerrainVariant::Driveway,
        TerrainVariant::SplineDriveway,
        TerrainVariant::Sidewalk,
        TerrainVariant::SplineSidewalk,
        TerrainVariant::PlantPolyline,
        TerrainVariant::PlantSpline,
        TerrainVariant::SprinklerPolyline,
        TerrainVariant::SprinklerSpline,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TerrainVariant::Perimeter => "Terrain Perimeter",
            TerrainVariant::ElevationPoint => "Elevation Point",
            TerrainVariant::ElevationLine => "Elevation Line",
            TerrainVariant::ElevationRegion => "Elevation Region",
            TerrainVariant::Hill => "Hill",
            TerrainVariant::Valley => "Valley",
            TerrainVariant::Raised => "Raised Region",
            TerrainVariant::Lowered => "Lowered Region",
            TerrainVariant::Flat => "Flat Region (Cut/Fill)",
            TerrainVariant::Road => "Road",
            TerrainVariant::Driveway => "Driveway",
            TerrainVariant::Sidewalk => "Sidewalk",
            TerrainVariant::Hole => "Terrain Hole",
            TerrainVariant::Build => "Build Terrain",
            TerrainVariant::ElevationSpline => "Elevation Spline",
            TerrainVariant::Break => "Terrain Break",
            TerrainVariant::StraightWall => "Straight Terrain Wall",
            TerrainVariant::StraightCurb => "Straight Terrain Curb",
            TerrainVariant::CurvedWall => "Curved Terrain Wall",
            TerrainVariant::CurvedCurb => "Curved Terrain Curb",
            TerrainVariant::RectFeature => "Rectangular Feature",
            TerrainVariant::KidneyFeature => "Kidney Shaped Feature",
            TerrainVariant::SplineFeature => "Spline Feature",
            TerrainVariant::BedPolyline => "Polyline Garden Bed",
            TerrainVariant::BedKidney => "Kidney Garden Bed",
            TerrainVariant::BedSpline => "Spline Garden Bed",
            TerrainVariant::GrassPolyline => "Polyline Grass Region",
            TerrainVariant::GrassKidney => "Kidney Grass Region",
            TerrainVariant::GrassSpline => "Spline Grass Region",
            TerrainVariant::WaterPolyline => "Polyline Water Feature",
            TerrainVariant::WaterSpline => "Spline Water Feature",
            TerrainVariant::StonePolyline => "Polyline Stepping Stone",
            TerrainVariant::StoneSpline => "Spline Stepping Stone",
            TerrainVariant::SplineRoad => "Spline Road",
            TerrainVariant::SplineDriveway => "Spline Driveway",
            TerrainVariant::SplineSidewalk => "Spline Sidewalk",
            TerrainVariant::PlantPolyline => "Polyline Plant",
            TerrainVariant::PlantSpline => "Spline Plant",
            TerrainVariant::SprinklerPolyline => "Polyline Sprinkler",
            TerrainVariant::SprinklerSpline => "Spline Sprinkler",
        }
    }

    fn draw(self) -> Draw {
        use TerrainVariant as V;
        match self {
            V::ElevationPoint => Draw::Spot,
            V::ElevationLine
            | V::ElevationSpline
            | V::Break
            | V::StraightWall
            | V::StraightCurb
            | V::Road
            | V::Driveway
            | V::Sidewalk
            | V::SplineRoad
            | V::SplineDriveway
            | V::SplineSidewalk
            | V::StonePolyline
            | V::StoneSpline
            | V::PlantPolyline
            | V::PlantSpline
            | V::SprinklerPolyline
            | V::SprinklerSpline => Draw::Polyline,
            V::Build => Draw::Command,
            V::CurvedWall | V::CurvedCurb => Draw::Arc,
            V::RectFeature => Draw::Rect,
            V::KidneyFeature | V::BedKidney | V::GrassKidney => Draw::Kidney,
            _ => Draw::Polygon,
        }
    }

    /// The clicks are control points of a Catmull-Rom curve.
    fn spline(self) -> bool {
        use TerrainVariant as V;
        matches!(
            self,
            V::ElevationSpline
                | V::SplineFeature
                | V::BedSpline
                | V::GrassSpline
                | V::WaterSpline
                | V::StoneSpline
                | V::SplineRoad
                | V::SplineDriveway
                | V::SplineSidewalk
                | V::PlantSpline
                | V::SprinklerSpline
        )
    }

    fn feature(self) -> Option<FeatureKind> {
        match self {
            TerrainVariant::RectFeature => Some(FeatureKind::Rectangular),
            TerrainVariant::KidneyFeature => Some(FeatureKind::Kidney),
            TerrainVariant::SplineFeature => Some(FeatureKind::Spline),
            _ => None,
        }
    }

    fn wall(self) -> Option<WallKind> {
        match self {
            TerrainVariant::StraightWall | TerrainVariant::CurvedWall => Some(WallKind::Wall),
            TerrainVariant::StraightCurb | TerrainVariant::CurvedCurb => Some(WallKind::Curb),
            _ => None,
        }
    }

    /// The landscape object a flavor draws and how its outline is made.
    fn scape(self) -> Option<(LandscapeKind, ShapeKind)> {
        use LandscapeKind as K;
        use ShapeKind as S;
        use TerrainVariant as V;
        Some(match self {
            V::BedPolyline => (K::GardenBed, S::Polyline),
            V::BedKidney => (K::GardenBed, S::Kidney),
            V::BedSpline => (K::GardenBed, S::Spline),
            V::GrassPolyline => (K::GrassRegion, S::Polyline),
            V::GrassKidney => (K::GrassRegion, S::Kidney),
            V::GrassSpline => (K::GrassRegion, S::Spline),
            V::WaterPolyline => (K::WaterFeature, S::Polyline),
            V::WaterSpline => (K::WaterFeature, S::Spline),
            V::StonePolyline => (K::SteppingStones, S::Polyline),
            V::StoneSpline => (K::SteppingStones, S::Spline),
            V::PlantPolyline => (K::Plants, S::Polyline),
            V::PlantSpline => (K::Plants, S::Spline),
            V::SprinklerPolyline => (K::Sprinklers, S::Polyline),
            V::SprinklerSpline => (K::Sprinklers, S::Spline),
            _ => return None,
        })
    }

    fn modifier(self) -> Option<ModifierKind> {
        match self {
            TerrainVariant::Hill => Some(ModifierKind::Hill),
            TerrainVariant::Valley => Some(ModifierKind::Valley),
            TerrainVariant::Raised => Some(ModifierKind::RaisedRegion),
            TerrainVariant::Lowered => Some(ModifierKind::LoweredRegion),
            TerrainVariant::Flat => Some(ModifierKind::FlatRegion),
            _ => None,
        }
    }

    fn road(self) -> Option<RoadKind> {
        match self {
            TerrainVariant::Road | TerrainVariant::SplineRoad => Some(RoadKind::Road),
            TerrainVariant::Driveway | TerrainVariant::SplineDriveway => Some(RoadKind::Driveway),
            TerrainVariant::Sidewalk | TerrainVariant::SplineSidewalk => Some(RoadKind::Sidewalk),
            _ => None,
        }
    }

    /// Fewest points a finished shape needs.
    fn min_points(self) -> usize {
        match self.draw() {
            Draw::Polygon | Draw::Kidney | Draw::Arc => 3,
            Draw::Polyline | Draw::Rect => 2,
            _ => 1,
        }
    }
}

/// The shape waiting for its typed value.
enum Pending {
    Point(Point),
    Line(Vec<Point>),
    Region(Vec<Point>),
    Modifier(ModifierKind, Vec<Point>),
    Road(RoadKind, Vec<Point>),
    Break(Vec<Point>),
}

struct Entry {
    prompt: &'static str,
    default: f64,
    at: Point,
    what: Pending,
}

pub struct TerrainTool {
    variant: TerrainVariant,
    points: Vec<Point>,
    hover: Option<Point>,
    entry: Option<Entry>,
    /// The elevation typed last (the default for the next one).
    last_elevation: f64,
    dialog: RefCell<Option<TerrainDialog>>,
    applied: RefCell<Option<TerrainRecord>>,
    /// The specification of one object, with the element it edits.
    object_dialog: RefCell<Option<(TerrainHit, ObjectDialog)>>,
    applied_object: RefCell<Option<(TerrainHit, TerrainObject)>>,
    /// The first corner of a rectangle was pressed: releasing elsewhere ends it.
    pressed: bool,
}

impl Default for TerrainTool {
    fn default() -> Self {
        Self {
            variant: TerrainVariant::Perimeter,
            points: Vec::new(),
            hover: None,
            entry: None,
            last_elevation: 0.0,
            dialog: RefCell::new(None),
            applied: RefCell::new(None),
            object_dialog: RefCell::new(None),
            applied_object: RefCell::new(None),
            pressed: false,
        }
    }
}

/// The arc from `start` to `end` through the side of the chord `click` is on,
/// bulging as far as the click is from the chord (rounded to `unit`).
fn arc_through(start: Point, end: Point, click: Point, unit: f64) -> Vec<Point> {
    let normal = (end - start).normalized().perp();
    let mid = Point::lerp(start, end, 0.5);
    let bulge = ((click - mid).dot(normal) / unit).round() * unit;
    if bulge.abs() < 0.5 {
        return vec![start, end];
    }
    let n = WallCurve { bulge }.facet_count(start, end).max(8);
    arc_polyline(start, end, bulge, n)
}

/// Parses a typed length, allowing a leading minus (`-6"`, `-2'`).
pub fn parse_value(text: &str) -> Option<f64> {
    let t = text.trim();
    match t.strip_prefix('-') {
        Some(rest) => parse_ft_in(rest.trim()).map(|v| -v),
        None => parse_ft_in(t),
    }
}

/// Build Terrain: marks the terrain built so the surface and contours show.
/// Returns the number of contour levels.
pub fn build_terrain_now(cx: &mut EditorContext) -> Result<usize, &'static str> {
    let rec = load_terrain(&cx.project).unwrap_or_default();
    if !rec.has_perimeter() {
        return Err("Draw a Terrain Perimeter first");
    }
    edit_terrain(cx, "Build Terrain", |r| r.built = true);
    Ok(terrain_view(&cx.project).map_or(0, |v| v.contours.len()))
}

impl TerrainTool {
    pub fn variant(&self) -> TerrainVariant {
        self.variant
    }

    /// The vertices of the shape being drawn.
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    pub fn is_typing(&self) -> bool {
        self.entry.is_some()
    }

    fn reset(&mut self, cx: &mut EditorContext) {
        self.points.clear();
        self.entry = None;
        cx.temp.editing = None;
        cx.readout = None;
    }

    fn dialog_open(&self) -> bool {
        self.dialog.borrow().is_some() || self.object_dialog.borrow().is_some()
    }

    fn flush(&mut self, cx: &mut EditorContext) -> Option<ToolResult> {
        if let Some((hit, obj)) = self.applied_object.borrow_mut().take() {
            let label = obj.title().to_string();
            edit_terrain(cx, &label, |r| {
                replace_object(&mut r.terrain, hit, obj);
            });
            return Some(ToolResult::committed(&label));
        }
        let draft = self.applied.borrow_mut().take()?;
        edit_terrain(cx, "Terrain Specification", |rec| {
            rec.contour_interval = draft.contour_interval;
            rec.layer = draft.layer.clone();
            let t = &mut rec.terrain;
            t.subfloor_height_above_terrain = draft.terrain.subfloor_height_above_terrain;
            t.building_pad_elevation = draft.terrain.building_pad_elevation;
            t.smoothing = draft.terrain.smoothing;
            t.grid_spacing = draft.terrain.grid_spacing;
        });
        Some(ToolResult::committed("Terrain Specification"))
    }

    fn begin_entry(&mut self, cx: &mut EditorContext, what: Pending, at: Point) {
        let (prompt, default) = match &what {
            Pending::Point(_) | Pending::Line(_) | Pending::Region(_) => {
                ("Elevation", self.last_elevation)
            }
            Pending::Modifier(ModifierKind::Hill, _) => ("Hill height", DEFAULT_HILL),
            Pending::Modifier(ModifierKind::Valley, _) => ("Valley depth", DEFAULT_HILL),
            Pending::Modifier(ModifierKind::RaisedRegion, _) => ("Raise by", DEFAULT_REGION_SHIFT),
            Pending::Modifier(_, _) => ("Lower by", DEFAULT_REGION_SHIFT),
            Pending::Road(RoadKind::Road, _) => ("Road width", DEFAULT_ROAD_WIDTH),
            Pending::Road(RoadKind::Driveway, _) => ("Driveway width", DEFAULT_DRIVEWAY_WIDTH),
            Pending::Road(RoadKind::Sidewalk, _) => ("Sidewalk width", DEFAULT_SIDEWALK_WIDTH),
            Pending::Break(_) => ("Break elevation", self.last_elevation),
        };
        self.entry = Some(Entry {
            prompt,
            default,
            at,
            what,
        });
        cx.temp.editing = Some(EditField {
            index: ENTRY_INDEX,
            text: String::new(),
        });
        cx.readout = Some(format!(
            "{prompt}: type a value, Enter for {}",
            fmt_ft_in(default)
        ));
    }

    /// The outline the clicks make: the corners of a rectangle or kidney, the
    /// arc of a curved wall, the flattened curve of a spline, or the polyline
    /// itself. `None` (with a status line) when the clicks make no valid shape.
    fn shape_from(&self, cx: &mut EditorContext, ctrl: &[Point]) -> Option<Vec<Point>> {
        let v = self.variant;
        match v.draw() {
            Draw::Rect => {
                let (a, b) = (ctrl[0], ctrl[1]);
                if (a.x - b.x).abs() < 1.0 || (a.y - b.y).abs() < 1.0 {
                    cx.status = "The rectangle has no area".into();
                    return None;
                }
                Some(rectangle_outline(a, b))
            }
            Draw::Kidney => {
                let blob = kidney_outline(ctrl[0], ctrl[1], ctrl[2]);
                if blob.is_none() {
                    cx.status =
                        "Click the two ends, then a point off the axis for the width".into();
                }
                blob
            }
            Draw::Arc => Some(arc_through(ctrl[0], ctrl[1], ctrl[2], cx.snap_unit())),
            Draw::Polygon if v.spline() => Some(flatten_spline(ctrl, true, SPLINE_SAMPLES)),
            Draw::Polyline if v.spline() => Some(flatten_spline(ctrl, false, SPLINE_SAMPLES)),
            _ => Some(ctrl.to_vec()),
        }
    }

    /// Ends the shape: closes a perimeter, adds a hole, flat region, wall,
    /// break or landscape object, or asks for the value the shape needs.
    fn finish(&mut self, cx: &mut EditorContext) -> ToolResult {
        let v = self.variant;
        if self.points.len() < v.min_points() {
            cx.status = format!("{} needs at least {} points", v.name(), v.min_points());
            return ToolResult::consumed();
        }
        let ctrl = std::mem::take(&mut self.points);
        self.pressed = false;
        let Some(pts) = self.shape_from(cx, &ctrl) else {
            // Keep the clicks of a polygon so the user can add more; the
            // fixed-count shapes start over.
            if matches!(v.draw(), Draw::Polygon | Draw::Polyline) {
                self.points = ctrl;
            }
            return ToolResult::consumed();
        };
        let at = pts[pts.len() - 1];
        if let Some((kind, shape)) = v.scape() {
            let mut obj = Landscape::new(kind, shape, pts);
            if kind == LandscapeKind::Plants {
                scape::apply_plant(&mut obj, &default_plant());
            }
            edit_terrain(cx, v.name(), |r| r.terrain.landscape.push(obj));
            ensure_landscape_layers(&mut cx.project);
            self.after_commit(cx);
            return ToolResult::committed(v.name());
        }
        if let Some(kind) = v.feature() {
            edit_terrain(cx, v.name(), |r| {
                r.terrain.features.push(Feature {
                    kind,
                    polygon: pts,
                    material: "Concrete".into(),
                    height: DEFAULT_FEATURE_HEIGHT,
                    ..Feature::default()
                });
            });
            ensure_landscape_layers(&mut cx.project);
            self.after_commit(cx);
            return ToolResult::committed(v.name());
        }
        if let Some(kind) = v.wall() {
            let curved = v.draw() == Draw::Arc;
            edit_terrain(cx, v.name(), |r| {
                r.terrain.walls.push(TerrainWall::new(kind, pts, curved));
            });
            ensure_landscape_layers(&mut cx.project);
            self.after_commit(cx);
            return ToolResult::committed(v.name());
        }
        match v {
            TerrainVariant::Perimeter => {
                edit_terrain(cx, "Terrain Perimeter", |r| r.terrain.perimeter = pts);
                self.after_commit(cx);
                ToolResult::committed("Terrain Perimeter")
            }
            TerrainVariant::Hole => {
                edit_terrain(cx, "Terrain Hole", |r| {
                    r.terrain.features.push(Feature {
                        kind: FeatureKind::Hole,
                        polygon: pts,
                        ..Feature::default()
                    });
                });
                self.after_commit(cx);
                ToolResult::committed("Terrain Hole")
            }
            TerrainVariant::Flat => {
                edit_terrain(cx, "Flat Region", |r| {
                    r.terrain.modifiers.push(Modifier {
                        kind: ModifierKind::FlatRegion,
                        polygon: pts,
                        height: 0.0,
                    });
                });
                self.after_commit(cx);
                ToolResult::committed("Flat Region")
            }
            TerrainVariant::ElevationLine | TerrainVariant::ElevationSpline => {
                self.begin_entry(cx, Pending::Line(pts), at);
                ToolResult::consumed()
            }
            TerrainVariant::Break => {
                self.begin_entry(cx, Pending::Break(pts), at);
                ToolResult::consumed()
            }
            TerrainVariant::ElevationRegion => {
                self.begin_entry(cx, Pending::Region(pts), at);
                ToolResult::consumed()
            }
            other => {
                let what = match (other.modifier(), other.road()) {
                    (Some(kind), _) => Pending::Modifier(kind, pts),
                    (_, Some(kind)) => Pending::Road(kind, pts),
                    _ => return ToolResult::consumed(),
                };
                self.begin_entry(cx, what, at);
                ToolResult::consumed()
            }
        }
    }

    fn after_commit(&mut self, cx: &mut EditorContext) {
        self.points.clear();
        self.entry = None;
        cx.temp.editing = None;
        cx.readout = None;
        cx.status.clear();
    }

    /// Enter in the inline field: stores the shape with the typed value.
    fn commit_entry(&mut self, cx: &mut EditorContext) -> ToolResult {
        let text = cx
            .temp
            .editing
            .as_ref()
            .map(|e| e.text.clone())
            .unwrap_or_default();
        let Some(default) = self.entry.as_ref().map(|e| e.default) else {
            return ToolResult::ignored();
        };
        let value = if text.trim().is_empty() {
            default
        } else if let Some(v) = parse_value(&text) {
            v
        } else {
            cx.status = format!("\"{text}\" is not a length");
            return ToolResult::consumed();
        };
        let Some(entry) = self.entry.take() else {
            return ToolResult::ignored();
        };
        let label = match entry.what {
            Pending::Point(p) => {
                self.last_elevation = value;
                edit_terrain(cx, "Elevation Point", |r| {
                    r.terrain
                        .elevation_points
                        .push(ElevationPoint { pos: p, z: value });
                });
                "Elevation Point"
            }
            Pending::Line(points) => {
                self.last_elevation = value;
                edit_terrain(cx, "Elevation Line", |r| {
                    r.terrain
                        .elevation_lines
                        .push(ElevationLine { points, z: value });
                });
                "Elevation Line"
            }
            Pending::Region(polygon) => {
                self.last_elevation = value;
                edit_terrain(cx, "Elevation Region", |r| {
                    r.terrain
                        .elevation_regions
                        .push(ElevationRegion { polygon, z: value });
                });
                "Elevation Region"
            }
            Pending::Break(points) => {
                self.last_elevation = value;
                edit_terrain(cx, "Terrain Break", |r| {
                    r.terrain.breaks.push(TerrainBreak {
                        points,
                        z: value,
                        ..TerrainBreak::default()
                    });
                });
                ensure_landscape_layers(&mut cx.project);
                "Terrain Break"
            }
            Pending::Modifier(kind, polygon) => {
                edit_terrain(cx, "Terrain Modifier", |r| {
                    r.terrain.modifiers.push(Modifier {
                        kind,
                        polygon,
                        height: value.abs(),
                    });
                });
                "Terrain Modifier"
            }
            Pending::Road(kind, centerline) => {
                if value <= 0.0 {
                    cx.status = "The width must be greater than zero".into();
                    self.entry = Some(Entry {
                        what: Pending::Road(kind, centerline),
                        ..entry
                    });
                    return ToolResult::consumed();
                }
                edit_terrain(cx, "Terrain Road", |r| {
                    r.terrain.roads.push(RoadStrip {
                        kind,
                        centerline,
                        width: value,
                        curb: kind == RoadKind::Road,
                    });
                });
                "Terrain Road"
            }
        };
        self.after_commit(cx);
        ToolResult::committed(label)
    }

    /// Delete: removes the element under the pointer.
    fn delete_hovered(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(h) = self.hover else {
            return ToolResult::ignored();
        };
        let Some(rec) = load_terrain(&cx.project) else {
            return ToolResult::ignored();
        };
        let Some(hit) = hit_terrain(&rec.terrain, h, cx.pick_tol()) else {
            return ToolResult::ignored();
        };
        edit_terrain(cx, "Delete Terrain Element", |r| {
            remove_terrain_element(&mut r.terrain, hit);
        });
        ToolResult::committed("Delete Terrain Element")
    }

    /// The outline the clicks so far (and the pointer) would make.
    fn preview(&self, cx: &EditorContext, ctrl: Vec<Point>) -> (Vec<Point>, bool) {
        let v = self.variant;
        let enough = ctrl.len() >= v.min_points();
        match v.draw() {
            Draw::Rect if ctrl.len() >= 2 => (rectangle_outline(ctrl[0], ctrl[1]), true),
            Draw::Kidney if enough => match kidney_outline(ctrl[0], ctrl[1], ctrl[2]) {
                Some(blob) => (blob, true),
                None => (ctrl, false),
            },
            Draw::Arc if enough => (
                arc_through(ctrl[0], ctrl[1], ctrl[2], cx.snap_unit()),
                false,
            ),
            Draw::Polygon if v.spline() && ctrl.len() >= 3 => {
                (flatten_spline(&ctrl, true, SPLINE_SAMPLES), true)
            }
            Draw::Polyline if v.spline() && ctrl.len() >= 3 => {
                (flatten_spline(&ctrl, false, SPLINE_SAMPLES), false)
            }
            _ => (ctrl, self.entry.is_some() && v.draw() == Draw::Polygon),
        }
    }

    /// Arrow keys: moves the element under the pointer.
    fn nudge_hovered(&mut self, cx: &mut EditorContext, delta: Point) -> ToolResult {
        let Some(h) = self.hover else {
            return ToolResult::ignored();
        };
        let Some(rec) = load_terrain(&cx.project) else {
            return ToolResult::ignored();
        };
        let Some(hit) = hit_terrain(&rec.terrain, h, cx.pick_tol()) else {
            return ToolResult::ignored();
        };
        edit_terrain(cx, "Move Terrain Element", |r| {
            move_terrain_element(&mut r.terrain, hit, delta);
        });
        self.hover = Some(h + delta);
        ToolResult::committed("Move Terrain Element")
    }

    /// The snapped point for the pointer: from the last point's angle, except
    /// for the first point and the click that sets a width or a bulge.
    fn point_for(&self, cx: &EditorContext, p: &PointerEvent) -> Point {
        let sets_width =
            matches!(self.variant.draw(), Draw::Arc | Draw::Kidney) && self.points.len() == 2;
        if self.points.is_empty() || sets_width {
            p.snapped
        } else {
            self.snapped(cx, p)
        }
    }

    fn snapped(&self, cx: &EditorContext, p: &PointerEvent) -> Point {
        cx.snap_at(p.world, self.points.last().copied(), p.modifiers.alt, &[])
            .point
    }

    fn update_readout(&self, cx: &mut EditorContext) {
        if self.entry.is_some() {
            return;
        }
        cx.readout = match (self.points.last(), self.hover) {
            (Some(a), Some(h)) => Some(format!("Length: {}", cx.fmt_dim(a.dist(h)))),
            _ => None,
        };
    }
}

impl Tool for TerrainTool {
    fn id(&self) -> ToolId {
        ToolId::TerrainVariant(self.variant)
    }

    fn name(&self) -> &'static str {
        self.variant.name()
    }

    fn hint(&self) -> String {
        match (self.variant, self.entry.is_some()) {
            (_, true) => "Type the value and press Enter (Esc cancels)".into(),
            (TerrainVariant::Build, _) => "Build Terrain: click to build the terrain".into(),
            (TerrainVariant::ElevationPoint, _) => {
                "Elevation Point: click, then type the elevation".into()
            }
            (v, _) => match v.draw() {
                Draw::Polygon if v.spline() => format!(
                    "{}: click the control points, Enter closes the curve",
                    v.name()
                ),
                Draw::Polygon => {
                    format!("{}: click the corners, Enter closes the shape", v.name())
                }
                Draw::Rect => format!("{}: drag a rectangle, or click two corners", v.name()),
                Draw::Kidney => format!(
                    "{}: click both ends of the long axis, then the width",
                    v.name()
                ),
                Draw::Arc => format!(
                    "{}: click the start, the end, then the bulge of the curve",
                    v.name()
                ),
                _ if v.spline() => format!(
                    "{}: click the control points, Enter ends the curve",
                    v.name()
                ),
                _ => format!("{}: click the points, Enter ends the line", v.name()),
            },
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::TerrainVariant(v) = id {
            if v != self.variant {
                self.points.clear();
                self.entry = None;
                self.pressed = false;
            }
            self.variant = v;
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.reset(cx);
        self.hover = None;
        cx.status.clear();
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.reset(cx);
        *self.dialog.borrow_mut() = None;
        *self.applied.borrow_mut() = None;
        *self.object_dialog.borrow_mut() = None;
        *self.applied_object.borrow_mut() = None;
        self.pressed = false;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        self.hover = Some(self.point_for(cx, &p));
        self.update_readout(cx);
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if self.dialog_open() || self.entry.is_some() {
            return ToolResult::consumed();
        }
        let v = self.variant;
        match v.draw() {
            Draw::Command => match build_terrain_now(cx) {
                Ok(n) => {
                    cx.status = if n == 0 {
                        "Terrain built: no contours yet (add elevation data)".into()
                    } else {
                        format!("Terrain built: {n} contour levels")
                    };
                    ToolResult {
                        switch_to: Some(ToolId::Select),
                        ..ToolResult::committed("Build Terrain")
                    }
                }
                Err(msg) => {
                    cx.status = msg.into();
                    ToolResult::consumed()
                }
            },
            Draw::Spot => {
                let at = p.snapped;
                self.begin_entry(cx, Pending::Point(at), at);
                ToolResult::consumed()
            }
            Draw::Polyline | Draw::Polygon | Draw::Rect | Draw::Kidney | Draw::Arc => {
                if self.points.is_empty()
                    && v == TerrainVariant::Perimeter
                    && load_terrain(&cx.project).is_some_and(|r| r.has_perimeter())
                {
                    cx.status =
                        "The plan already has a terrain perimeter (Delete removes it)".into();
                    return ToolResult::consumed();
                }
                let pt = self.point_for(cx, &p);
                let closes = v.draw() == Draw::Polygon
                    && self.points.len() >= 3
                    && p.world.dist(self.points[0]) <= cx.pick_tol() * 1.5;
                if closes {
                    return self.finish(cx);
                }
                if self.points.last().is_none_or(|l| l.dist(pt) > 0.5) {
                    self.points.push(pt);
                }
                self.update_readout(cx);
                match v.draw() {
                    Draw::Rect if self.points.len() == 1 => self.pressed = true,
                    Draw::Rect | Draw::Kidney | Draw::Arc
                        if self.points.len() >= v.min_points() =>
                    {
                        return self.finish(cx);
                    }
                    _ => {}
                }
                ToolResult::consumed()
            }
        }
    }

    /// Releasing a pressed rectangle corner somewhere else ends the drag.
    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let pressed = std::mem::take(&mut self.pressed);
        if pressed
            && self.variant.draw() == Draw::Rect
            && self.points.len() == 1
            && !self.dialog_open()
            && p.snapped.dist(self.points[0]) > cx.pick_tol() * 1.5
        {
            self.points.push(p.snapped);
            return self.finish(cx);
        }
        ToolResult::ignored()
    }

    fn frame(&mut self, cx: &mut EditorContext, _ctx: &egui::Context) {
        // An OK in the Terrain Specification applies right away.
        let _ = self.flush(cx);
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.entry.is_some() {
            return ToolResult::consumed();
        }
        let rec = load_terrain(&cx.project).unwrap_or_default();
        // The first click of a double-click has just started a shape: an
        // object under the pointer gets its specification instead.
        if self.points.len() <= 1 {
            if let Some(hit) = hit_terrain(&rec.terrain, p.world, cx.pick_tol()) {
                if let Some(obj) = object_at(&rec.terrain, hit) {
                    self.reset(cx);
                    *self.object_dialog.borrow_mut() = Some((hit, ObjectDialog::new(obj)));
                    return ToolResult::consumed();
                }
            }
        }
        if !self.points.is_empty() {
            return self.finish(cx);
        }
        *self.dialog.borrow_mut() = Some(TerrainDialog::new(&rec));
        ToolResult::consumed()
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if self.dialog_open() {
            return ToolResult::consumed();
        }
        if self.entry.is_some() {
            if k.is(Key::Escape) {
                self.reset(cx);
                return ToolResult::consumed();
            }
            if k.is(Key::Enter) {
                return self.commit_entry(cx);
            }
            if k.is(Key::Backspace) {
                if let Some(e) = &mut cx.temp.editing {
                    e.text.pop();
                }
                return ToolResult::consumed();
            }
            if let (Some(t), Some(e)) = (&k.text, &mut cx.temp.editing) {
                e.text
                    .extend(t.chars().filter(|c| "0123456789'\"-./ ".contains(*c)));
            }
            return ToolResult::consumed();
        }
        if k.is(Key::Enter) {
            return if self.points.is_empty() {
                ToolResult::ignored()
            } else {
                self.finish(cx)
            };
        }
        if k.is(Key::Escape) {
            if self.points.is_empty() {
                return ToolResult::ignored();
            }
            self.reset(cx);
            return ToolResult::consumed();
        }
        if k.is(Key::Backspace) || k.is(Key::Delete) {
            if self.points.pop().is_some() {
                self.update_readout(cx);
                return ToolResult::consumed();
            }
            return self.delete_hovered(cx);
        }
        let step = [
            (Key::ArrowLeft, (-1.0, 0.0)),
            (Key::ArrowRight, (1.0, 0.0)),
            (Key::ArrowUp, (0.0, 1.0)),
            (Key::ArrowDown, (0.0, -1.0)),
        ]
        .into_iter()
        .find(|(key, _)| k.is(*key));
        if let (Some((_, (dx, dy))), true) = (step, self.points.is_empty()) {
            let unit = cx.snap_unit() * if k.modifiers.shift { 10.0 } else { 1.0 };
            return self.nudge_hovered(cx, Point::new(dx * unit, dy * unit));
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        let ink = pal.ghost_stroke;
        let (width, color) = if self.variant == TerrainVariant::Perimeter {
            (3.0_f32, Color32::from_rgb(0x4F, 0x7F, 0x3A))
        } else {
            (1.5_f32, ink)
        };
        let stroke = egui::Stroke::new(width, color);
        let mut shape = self.points.clone();
        if self.entry.is_none() {
            shape.extend(self.hover.filter(|_| !self.points.is_empty()));
        }
        let (shape, closed) = self.preview(cx, shape);
        draw_polyline(painter, cam, &shape, closed, stroke, true);
        if self.variant.draw() == Draw::Polygon && self.points.len() >= 3 && self.entry.is_none() {
            // The closing edge back to the first point.
            if let Some(h) = self.hover {
                draw_polyline(
                    painter,
                    cam,
                    &[h, self.points[0]],
                    false,
                    egui::Stroke::new(1.0_f32, color.gamma_multiply(0.5)),
                    true,
                );
            }
        }
        for p in &self.points {
            painter.circle_filled(cam.world_to_screen(*p), 3.0, color);
        }
        if let (None, Some(h), Draw::Spot) = (&self.entry, self.hover, self.variant.draw()) {
            let c = cam.world_to_screen(h);
            let s = egui::Stroke::new(1.2_f32, ink);
            painter.line_segment([c + egui::vec2(-6.0, 0.0), c + egui::vec2(6.0, 0.0)], s);
            painter.line_segment([c + egui::vec2(0.0, -6.0), c + egui::vec2(0.0, 6.0)], s);
        }
        if let Some(e) = &self.entry {
            let typed = cx.temp.editing.as_ref().map_or("", |f| f.text.as_str());
            let text = format!(
                "{}: {}|   (Enter = {})",
                e.prompt,
                typed,
                fmt_ft_in(e.default)
            );
            let at = cam.world_to_screen(e.at) + egui::vec2(10.0, -10.0);
            let font = FontId::proportional(13.0);
            let galley = painter.layout_no_wrap(text, font, pal.text);
            let r = Rect::from_min_size(
                Pos2::new(at.x, at.y - galley.size().y - 6.0),
                galley.size() + egui::vec2(12.0, 6.0),
            );
            painter.rect_filled(r, 3.0, pal.background);
            painter.rect_stroke(
                r,
                3.0,
                egui::Stroke::new(1.0_f32, pal.selection),
                egui::StrokeKind::Middle,
            );
            painter.galley(r.min + egui::vec2(6.0, 3.0), galley, pal.text);
            painter.circle_filled(cam.world_to_screen(e.at), 3.0, pal.selection);
        }
        // The specification of one object.
        {
            let mut slot = self.object_dialog.borrow_mut();
            let outcome = slot.as_mut().map(|(_, d)| d.show(painter.ctx()));
            match outcome {
                Some(Outcome::Ok) => {
                    if let Some((hit, d)) = slot.take() {
                        *self.applied_object.borrow_mut() = Some((hit, d.draft().clone()));
                    }
                }
                Some(Outcome::Cancel) => {
                    slot.take();
                }
                _ => {}
            }
        }
        // The Terrain Specification.
        let mut slot = self.dialog.borrow_mut();
        let outcome = slot.as_mut().map(|d| d.show(painter.ctx()));
        match outcome {
            Some(Outcome::Ok) => {
                if let Some(d) = slot.take() {
                    *self.applied.borrow_mut() = Some(d.draft().clone());
                }
            }
            Some(Outcome::Cancel) => {
                slot.take();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::Project;
    use plan_terrain::elevation_at;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn tool(v: TerrainVariant) -> TerrainTool {
        let mut t = TerrainTool::default();
        t.set_variant(ToolId::TerrainVariant(v));
        t
    }

    fn click(t: &mut TerrainTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        let r = t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
        r
    }

    fn rect_perimeter(t: &mut TerrainTool, cx: &mut EditorContext) -> ToolResult {
        for (x, y) in [(0.0, 0.0), (1200.0, 0.0), (1200.0, 960.0), (0.0, 960.0)] {
            click(t, cx, x, y);
        }
        t.key(cx, KeyEvent::key(Key::Enter))
    }

    fn type_value(t: &mut TerrainTool, cx: &mut EditorContext, text: &str) -> ToolResult {
        if !text.is_empty() {
            t.key(cx, KeyEvent::text(text));
        }
        t.key(cx, KeyEvent::key(Key::Enter))
    }

    fn record(cx: &EditorContext) -> TerrainRecord {
        load_terrain(&cx.project).unwrap()
    }

    #[test]
    fn perimeter_closes_with_enter_and_there_is_only_one() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::Perimeter);
        for (x, y) in [(0.0, 0.0), (1200.0, 0.0), (1200.0, 960.0)] {
            click(&mut t, &mut cx, x, y);
        }
        assert_eq!(t.points().len(), 3);
        assert!(cx.readout.as_deref().unwrap().starts_with("Length:"));
        click(&mut t, &mut cx, 0.0, 960.0);
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Terrain Perimeter"));
        let rec = record(&cx);
        assert_eq!(rec.terrain.perimeter.len(), 4);
        assert!(rec.has_perimeter());
        assert!(t.points().is_empty());

        // A second perimeter is refused and leaves no undo step.
        let before = cx.undo_label().map(str::to_string);
        click(&mut t, &mut cx, 50.0, 50.0);
        assert!(cx.status.contains("already"));
        assert!(t.points().is_empty());
        assert_eq!(cx.undo_label().map(str::to_string), before);
        // Undo removes the perimeter.
        assert_eq!(cx.undo().as_deref(), Some("Terrain Perimeter"));
        assert!(load_terrain(&cx.project).is_none_or(|r| !r.has_perimeter()));
    }

    #[test]
    fn a_click_on_the_first_point_or_a_double_click_closes_the_shape() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::Perimeter);
        for (x, y) in [(0.0, 0.0), (600.0, 0.0), (600.0, 400.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let r = click(&mut t, &mut cx, 1.0, 1.0);
        assert_eq!(r.commit.as_deref(), Some("Terrain Perimeter"));
        assert_eq!(record(&cx).terrain.perimeter.len(), 3);

        let mut cx = self::cx();
        let mut t = tool(TerrainVariant::Perimeter);
        for (x, y) in [(0.0, 0.0), (600.0, 0.0), (600.0, 400.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let p = PointerEvent::at(&cx, Point::new(600.0, 400.0));
        assert!(t.double_click(&mut cx, p).commit.is_some());
        assert_eq!(record(&cx).terrain.perimeter.len(), 3);
    }

    #[test]
    fn too_few_points_do_not_close() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::Perimeter);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 100.0, 0.0);
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(r.commit.is_none() && r.consumed);
        assert!(cx.status.contains("at least 3"));
        assert_eq!(t.points().len(), 2);
        // Backspace drops the last point, Esc cancels the rest.
        t.key(&mut cx, KeyEvent::key(Key::Backspace));
        assert_eq!(t.points().len(), 1);
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(t.points().is_empty());
        assert!(
            !t.key(&mut cx, KeyEvent::escape()).consumed,
            "Esc leaves the tool"
        );
    }

    #[test]
    fn an_elevation_point_stores_the_typed_value() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::ElevationPoint);
        click(&mut t, &mut cx, 300.0, 200.0);
        assert!(t.is_typing());
        assert!(cx.temp.editing.is_some(), "the shell forwards typed text");
        assert!(cx.readout.as_deref().unwrap().starts_with("Elevation:"));
        t.key(&mut cx, KeyEvent::text("3'"));
        t.key(&mut cx, KeyEvent::text("x")); // not a length character
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Elevation Point"));
        let rec = record(&cx);
        assert_eq!(rec.terrain.elevation_points.len(), 1);
        assert_eq!(rec.terrain.elevation_points[0].z, 36.0);
        assert_eq!(
            rec.terrain.elevation_points[0].pos,
            Point::new(300.0, 200.0)
        );
        assert!(cx.temp.editing.is_none());

        // The next point defaults to the last value; negatives and backspace work.
        click(&mut t, &mut cx, 400.0, 200.0);
        assert!(type_value(&mut t, &mut cx, "").commit.is_some());
        assert_eq!(record(&cx).terrain.elevation_points[1].z, 36.0);
        click(&mut t, &mut cx, 500.0, 200.0);
        t.key(&mut cx, KeyEvent::text("-6\"9"));
        t.key(&mut cx, KeyEvent::key(Key::Backspace));
        type_value(&mut t, &mut cx, "");
        assert_eq!(record(&cx).terrain.elevation_points[2].z, -6.0);

        // A bad value keeps the field open; Esc cancels.
        click(&mut t, &mut cx, 600.0, 200.0);
        t.key(&mut cx, KeyEvent::text("1/0\""));
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(r.commit.is_none() && t.is_typing());
        assert!(cx.status.contains("not a length"));
        t.key(&mut cx, KeyEvent::escape());
        assert!(!t.is_typing() && cx.temp.editing.is_none());
        assert_eq!(record(&cx).terrain.elevation_points.len(), 3);

        // Each point is one undo step.
        assert_eq!(cx.undo().as_deref(), Some("Elevation Point"));
        assert_eq!(record(&cx).terrain.elevation_points.len(), 2);
    }

    #[test]
    fn lines_regions_and_modifiers_take_their_values() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::ElevationLine);
        click(&mut t, &mut cx, 0.0, 100.0);
        click(&mut t, &mut cx, 600.0, 100.0);
        t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(type_value(&mut t, &mut cx, "10'").commit.is_some());
        let rec = record(&cx);
        assert_eq!(rec.terrain.elevation_lines[0].z, 120.0);
        assert_eq!(rec.terrain.elevation_lines[0].points.len(), 2);

        t.set_variant(ToolId::TerrainVariant(TerrainVariant::Hill));
        for (x, y) in [(300.0, 300.0), (500.0, 300.0), (400.0, 500.0)] {
            click(&mut t, &mut cx, x, y);
        }
        t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(cx.readout.as_deref().unwrap().contains("Hill height"));
        type_value(&mut t, &mut cx, "");
        let m = &record(&cx).terrain.modifiers[0];
        assert_eq!((m.kind, m.height), (ModifierKind::Hill, DEFAULT_HILL));

        t.set_variant(ToolId::TerrainVariant(TerrainVariant::Flat));
        for (x, y) in [(800.0, 300.0), (900.0, 300.0), (900.0, 400.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Flat Region"));

        t.set_variant(ToolId::TerrainVariant(TerrainVariant::ElevationRegion));
        for (x, y) in [(100.0, 600.0), (300.0, 600.0), (300.0, 800.0)] {
            click(&mut t, &mut cx, x, y);
        }
        t.key(&mut cx, KeyEvent::key(Key::Enter));
        type_value(&mut t, &mut cx, "24");
        assert_eq!(record(&cx).terrain.elevation_regions[0].z, 24.0);

        t.set_variant(ToolId::TerrainVariant(TerrainVariant::Hole));
        for (x, y) in [(700.0, 600.0), (800.0, 600.0), (800.0, 700.0)] {
            click(&mut t, &mut cx, x, y);
        }
        t.key(&mut cx, KeyEvent::key(Key::Enter));
        let rec = record(&cx);
        assert_eq!(rec.terrain.features.len(), 1);
        assert_eq!(rec.terrain.features[0].kind, FeatureKind::Hole);
        assert_eq!(rec.terrain.modifiers.len(), 2);
    }

    #[test]
    fn roads_take_a_width_from_the_popup() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::Driveway);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 300.0, 0.0);
        let p = PointerEvent::at(&cx, Point::new(300.0, 0.0));
        t.double_click(&mut cx, p);
        assert!(t.is_typing());
        assert!(cx.readout.as_deref().unwrap().contains("Driveway width"));
        type_value(&mut t, &mut cx, "");
        let r = &record(&cx).terrain.roads[0];
        assert_eq!(
            (r.kind, r.width, r.curb),
            (RoadKind::Driveway, 144.0, false)
        );

        t.set_variant(ToolId::TerrainVariant(TerrainVariant::Road));
        click(&mut t, &mut cx, 0.0, 200.0);
        click(&mut t, &mut cx, 300.0, 200.0);
        t.key(&mut cx, KeyEvent::key(Key::Enter));
        type_value(&mut t, &mut cx, "0");
        assert!(t.is_typing(), "zero width is refused");
        t.key(&mut cx, KeyEvent::key(Key::Backspace));
        type_value(&mut t, &mut cx, "20'");
        let r = &record(&cx).terrain.roads[1];
        assert_eq!((r.kind, r.width, r.curb), (RoadKind::Road, 240.0, true));
    }

    fn sloped_terrain(cx: &mut EditorContext) {
        let mut t = tool(TerrainVariant::Perimeter);
        rect_perimeter(&mut t, cx);
        let mut p = tool(TerrainVariant::ElevationPoint);
        // Rising 10' over the 100' lot, west to east.
        for (x, y, z) in [
            (0.0, 0.0, "0"),
            (0.0, 960.0, "0"),
            (1200.0, 0.0, "10'"),
            (1200.0, 960.0, "10'"),
        ] {
            click(&mut p, cx, x, y);
            type_value(&mut p, cx, z);
        }
    }

    #[test]
    fn build_terrain_yields_contours_for_a_sloped_data_set() {
        let mut cx = cx();
        sloped_terrain(&mut cx);
        assert!(!record(&cx).built);
        assert!(terrain_view(&cx.project).unwrap().contours.is_empty());

        let mut t = tool(TerrainVariant::Build);
        let r = click(&mut t, &mut cx, 10.0, 10.0);
        assert_eq!(r.commit.as_deref(), Some("Build Terrain"));
        assert_eq!(r.switch_to, Some(ToolId::Select));
        let view = terrain_view(&cx.project).unwrap();
        assert!(view.record.built);
        assert!(view.contours.len() >= 5, "{} levels", view.contours.len());
        assert!(cx.status.contains("contour levels"));
        let surface = view.surface.as_ref().unwrap();
        let mid = elevation_at(surface, Point::new(600.0, 480.0)).unwrap();
        assert!((mid - 60.0).abs() < 1.0, "{mid}");
        assert!(view.symbols.len() > 5, "contours and labels are symbols");
        assert_eq!(
            crate::editor::site_view::terrain_elevation_at(&cx.project, Point::new(600.0, 480.0))
                .map(|z| z.round()),
            Some(60.0)
        );

        // Undo takes the contours away again.
        assert_eq!(cx.undo().as_deref(), Some("Build Terrain"));
        assert!(terrain_view(&cx.project).unwrap().contours.is_empty());
    }

    #[test]
    fn build_terrain_needs_a_perimeter() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::Build);
        let r = click(&mut t, &mut cx, 0.0, 0.0);
        assert!(r.commit.is_none());
        assert!(cx.status.contains("Perimeter"));
        assert!(!cx.can_undo());
    }

    #[test]
    fn terrain_saves_and_loads_with_the_project() {
        let mut cx = cx();
        sloped_terrain(&mut cx);
        click(&mut tool(TerrainVariant::Build), &mut cx, 0.0, 0.0);
        let json = cx.project.to_json().unwrap();
        let project = Project::from_json(&json).unwrap();
        let back = load_terrain(&project).unwrap();
        assert_eq!(back, record(&cx));
        assert_eq!(back.terrain.elevation_points.len(), 4);
        assert!(back.built);
        // Stored in the project's typed slot, not as a CAD record.
        assert!(project.terrain.is_some());
        assert!(project.floors[0].cad.is_empty());
        assert!(project
            .layers
            .get(crate::editor::site_view::TERRAIN_DATA_LAYER)
            .is_none());
    }

    #[test]
    fn delete_removes_the_element_under_the_pointer() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::ElevationPoint);
        click(&mut t, &mut cx, 300.0, 200.0);
        type_value(&mut t, &mut cx, "5'");
        let ev = PointerEvent::at(&cx, Point::new(302.0, 201.0));
        t.pointer_move(&mut cx, ev);
        let r = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete Terrain Element"));
        assert!(record(&cx).terrain.elevation_points.is_empty());
        // Nothing under the pointer: Delete is left to the shell.
        assert!(!t.key(&mut cx, KeyEvent::key(Key::Delete)).consumed);
    }

    #[test]
    fn double_click_opens_the_specification_and_ok_stores_it() {
        let mut cx = cx();
        sloped_terrain(&mut cx);
        let mut t = tool(TerrainVariant::Hill);
        let p = PointerEvent::at(&cx, Point::new(10.0, 10.0));
        assert!(t.double_click(&mut cx, p).consumed);
        assert!(t.dialog_open());
        assert!(t.pointer_down(&mut cx, p.with_down(true)).consumed);
        assert!(
            t.points().is_empty(),
            "canvas ignores clicks while it is open"
        );
        let mut draft = t.dialog.borrow().as_ref().unwrap().draft().clone();
        draft.contour_interval = 24.0;
        draft.terrain.subfloor_height_above_terrain = 18.0;
        draft.terrain.elevation_points.clear(); // not a specification field
        *t.applied.borrow_mut() = Some(draft);
        let r = t.pointer_move(&mut cx, p);
        assert_eq!(r.commit.as_deref(), Some("Terrain Specification"));
        let rec = record(&cx);
        assert_eq!(rec.contour_interval, 24.0);
        assert_eq!(rec.terrain.subfloor_height_above_terrain, 18.0);
        assert_eq!(rec.terrain.elevation_points.len(), 4);
    }

    #[test]
    fn variants_switch_through_the_tool_set() {
        use crate::tools::ToolSet;
        let mut cx = cx();
        let mut set = ToolSet::new();
        set.set_active(&mut cx, ToolId::TerrainVariant(TerrainVariant::Road));
        assert_eq!(set.active().name(), "Road");
        set.set_active(&mut cx, ToolId::TerrainVariant(TerrainVariant::Build));
        assert_eq!(
            set.active_id(),
            ToolId::TerrainVariant(TerrainVariant::Build)
        );
        set.set_active(&mut cx, ToolId::Select);
        assert_eq!(set.active_id(), ToolId::Select);
    }

    #[test]
    fn overlay_draws_every_state_without_panicking() {
        let mut cx = cx();
        sloped_terrain(&mut cx);
        let mut t = tool(TerrainVariant::Hill);
        for (x, y) in [(300.0, 300.0), (500.0, 300.0), (400.0, 500.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let ev = PointerEvent::at(&cx, Point::new(350.0, 350.0));
        t.pointer_move(&mut cx, ev);
        let ctx = egui::Context::default();
        let draw = |t: &TerrainTool, cx: &EditorContext| {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, painter) =
                        ui.allocate_painter(egui::Vec2::new(800.0, 600.0), egui::Sense::hover());
                    let mut cam = Camera::default_view();
                    cam.rect = painter.clip_rect();
                    t.draw_overlay(cx, &painter, &cam);
                });
            });
        };
        draw(&t, &cx);
        t.key(&mut cx, KeyEvent::key(Key::Enter));
        t.key(&mut cx, KeyEvent::text("6'"));
        draw(&t, &cx);
        t.key(&mut cx, KeyEvent::escape());
        let p = PointerEvent::at(&cx, Point::new(10.0, 10.0));
        t.double_click(&mut cx, p);
        draw(&t, &cx);
    }

    // ----- landscape objects -----

    fn press_move_release(
        t: &mut TerrainTool,
        cx: &mut EditorContext,
        a: Point,
        b: Point,
    ) -> ToolResult {
        let pa = PointerEvent::at(cx, a);
        t.pointer_move(cx, pa);
        t.pointer_down(cx, pa.with_down(true));
        let pb = PointerEvent::at(cx, b);
        t.pointer_move(cx, pb);
        t.pointer_up(cx, pb)
    }

    fn clicks(t: &mut TerrainTool, cx: &mut EditorContext, pts: &[(f64, f64)]) {
        for (x, y) in pts {
            click(t, cx, *x, *y);
        }
    }

    fn enter(t: &mut TerrainTool, cx: &mut EditorContext) -> ToolResult {
        t.key(cx, KeyEvent::key(Key::Enter))
    }

    const PATH: [(f64, f64); 2] = [(0.0, 100.0), (300.0, 100.0)];
    const CURVE: [(f64, f64); 4] = [(0.0, 100.0), (150.0, 160.0), (300.0, 100.0), (450.0, 160.0)];
    const POLY: [(f64, f64); 4] = [
        (100.0, 100.0),
        (300.0, 100.0),
        (300.0, 300.0),
        (100.0, 300.0),
    ];
    const KIDNEY: [(f64, f64); 3] = [(100.0, 100.0), (400.0, 100.0), (250.0, 200.0)];

    /// Draws a variant with its documented gesture and returns the undo label.
    fn draw(cx: &mut EditorContext, v: TerrainVariant) -> Option<String> {
        use TerrainVariant as V;
        let mut t = tool(v);
        let r = match v {
            V::RectFeature => press_move_release(
                &mut t,
                cx,
                Point::new(100.0, 100.0),
                Point::new(400.0, 300.0),
            ),
            V::CurvedWall | V::CurvedCurb => {
                clicks(&mut t, cx, &[(100.0, 100.0), (600.0, 100.0)]);
                click(&mut t, cx, 350.0, 200.0)
            }
            V::KidneyFeature | V::BedKidney | V::GrassKidney => {
                clicks(&mut t, cx, &KIDNEY[..2]);
                click(&mut t, cx, KIDNEY[2].0, KIDNEY[2].1)
            }
            V::StraightWall
            | V::StraightCurb
            | V::Break
            | V::ElevationSpline
            | V::SplineRoad
            | V::SplineDriveway
            | V::SplineSidewalk
            | V::StonePolyline
            | V::StoneSpline
            | V::PlantPolyline
            | V::PlantSpline
            | V::SprinklerPolyline
            | V::SprinklerSpline => {
                let pts: &[(f64, f64)] = if v.spline() { &CURVE } else { &PATH };
                clicks(&mut t, cx, pts);
                enter(&mut t, cx)
            }
            _ => {
                clicks(&mut t, cx, &POLY);
                enter(&mut t, cx)
            }
        };
        // Shapes that ask for a value take the default.
        let r = if t.is_typing() {
            type_value(&mut t, cx, "")
        } else {
            r
        };
        r.commit
    }

    #[test]
    fn every_new_entry_activates_and_creates_its_object() {
        use TerrainVariant as V;
        for v in [
            V::ElevationSpline,
            V::Break,
            V::StraightWall,
            V::StraightCurb,
            V::CurvedWall,
            V::CurvedCurb,
            V::RectFeature,
            V::KidneyFeature,
            V::SplineFeature,
            V::BedPolyline,
            V::BedKidney,
            V::BedSpline,
            V::GrassPolyline,
            V::GrassKidney,
            V::GrassSpline,
            V::WaterPolyline,
            V::WaterSpline,
            V::StonePolyline,
            V::StoneSpline,
            V::SplineRoad,
            V::SplineDriveway,
            V::SplineSidewalk,
            V::PlantPolyline,
            V::PlantSpline,
            V::SprinklerPolyline,
            V::SprinklerSpline,
        ] {
            let mut cx = cx();
            let label = draw(&mut cx, v);
            let rec = record(&cx);
            let t = &rec.terrain;
            let made = t.elevation_lines.len()
                + t.breaks.len()
                + t.walls.len()
                + t.features.len()
                + t.landscape.len()
                + t.roads.len();
            assert_eq!(made, 1, "{}: {label:?}", v.name());
            assert!(label.is_some(), "{} committed nothing", v.name());
            assert!(!tool(v).hint().is_empty());
            // One undo step takes it away again.
            assert!(cx.undo().is_some());
            let after = load_terrain(&cx.project).unwrap_or_default().terrain;
            assert_eq!(
                after.elevation_lines.len()
                    + after.breaks.len()
                    + after.walls.len()
                    + after.features.len()
                    + after.landscape.len()
                    + after.roads.len(),
                0,
                "{} undo",
                v.name()
            );
        }
    }

    #[test]
    fn the_flyouts_hold_only_live_terrain_variants() {
        use crate::toolbar::{terrain_menu, Action};
        let mut seen = Vec::new();
        for f in terrain_menu() {
            for e in &f.entries {
                match e.action {
                    Action::SetTool(ToolId::TerrainVariant(v)) => seen.push(v),
                    ref other => panic!("{} / {} is {other:?}", f.group, e.name),
                }
            }
        }
        for v in &seen {
            assert!(TerrainVariant::ALL.contains(v), "{v:?}");
        }
        assert!(seen.len() >= 38);
        for (i, a) in TerrainVariant::ALL.iter().enumerate() {
            assert!(!TerrainVariant::ALL[i + 1..].contains(a), "{a:?} twice");
        }
    }

    #[test]
    fn walls_and_curbs_take_their_shape_and_defaults() {
        let mut cx = cx();
        draw(&mut cx, TerrainVariant::StraightWall);
        draw(&mut cx, TerrainVariant::StraightCurb);
        draw(&mut cx, TerrainVariant::CurvedWall);
        let w = &record(&cx).terrain.walls;
        assert_eq!(
            (w[0].kind, w[0].curved, w[0].points.len()),
            (WallKind::Wall, false, 2)
        );
        assert_eq!((w[0].height, w[0].thickness), (36.0, 8.0));
        assert_eq!((w[1].kind, w[1].height), (WallKind::Curb, 6.0));
        assert!(w[2].curved && w[2].points.len() > 4, "an arc, not a chord");
        // The arc bulges 100" towards the click and ends where it began.
        assert_eq!(
            (w[2].points[0], *w[2].points.last().unwrap()),
            (Point::new(100.0, 100.0), Point::new(600.0, 100.0))
        );
        let apex = w[2].points.iter().map(|p| p.y).fold(f64::MIN, f64::max);
        assert!((apex - 200.0).abs() < 3.0, "{apex}");
        // The Chief layers exist after the first object.
        assert!(cx.project.layers.get("Terrain, Walls").is_some());
    }

    #[test]
    fn a_rectangle_is_dragged_or_clicked() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::RectFeature);
        let r = press_move_release(
            &mut t,
            &mut cx,
            Point::new(100.0, 100.0),
            Point::new(400.0, 300.0),
        );
        assert_eq!(r.commit.as_deref(), Some("Rectangular Feature"));
        let f = &record(&cx).terrain.features[0];
        assert_eq!(f.kind, FeatureKind::Rectangular);
        assert_eq!(f.polygon.len(), 4);
        assert_eq!((f.material.as_str(), f.height), ("Concrete", 4.0));

        // Two clicks do as well; a click without a drag keeps the first corner.
        click(&mut t, &mut cx, 500.0, 100.0);
        assert_eq!(t.points().len(), 1);
        let r = click(&mut t, &mut cx, 700.0, 250.0);
        assert!(r.commit.is_some());
        assert_eq!(record(&cx).terrain.features.len(), 2);
        // A flat "rectangle" is refused.
        click(&mut t, &mut cx, 500.0, 400.0);
        click(&mut t, &mut cx, 700.0, 400.0);
        assert!(cx.status.contains("no area"));
        assert_eq!(record(&cx).terrain.features.len(), 2);
    }

    #[test]
    fn a_kidney_takes_three_clicks() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::BedKidney);
        clicks(&mut t, &mut cx, &KIDNEY[..2]);
        assert_eq!(t.points().len(), 2);
        let r = click(&mut t, &mut cx, 250.0, 200.0);
        assert_eq!(r.commit.as_deref(), Some("Kidney Garden Bed"));
        let o = &record(&cx).terrain.landscape[0];
        assert_eq!(
            (o.kind, o.shape),
            (LandscapeKind::GardenBed, ShapeKind::Kidney)
        );
        assert!(o.points.len() >= 20);
        assert!(o.points.iter().all(|p| p.x >= 99.0 && p.x <= 401.0));
        // A third click on the axis makes no blob.
        clicks(&mut t, &mut cx, &KIDNEY[..2]);
        let r = click(&mut t, &mut cx, 250.0, 100.0);
        assert!(r.commit.is_none() && cx.status.contains("width"));
        assert_eq!(record(&cx).terrain.landscape.len(), 1);
    }

    #[test]
    fn splines_are_stored_flattened_through_their_clicks() {
        let mut cx = cx();
        draw(&mut cx, TerrainVariant::SplineFeature);
        draw(&mut cx, TerrainVariant::StoneSpline);
        let t = record(&cx).terrain;
        assert_eq!(t.features[0].polygon.len(), 4 * SPLINE_SAMPLES);
        assert_eq!(t.landscape[0].points.len(), 3 * SPLINE_SAMPLES + 1);
        assert_eq!(t.landscape[0].shape, ShapeKind::Spline);
        assert!(t.features[0].polygon.contains(&Point::new(100.0, 100.0)));
    }

    #[test]
    fn elevation_spline_and_break_ask_for_an_elevation() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::ElevationSpline);
        clicks(&mut t, &mut cx, &CURVE);
        enter(&mut t, &mut cx);
        assert!(cx.readout.as_deref().unwrap().contains("Elevation"));
        type_value(&mut t, &mut cx, "5'");
        let l = &record(&cx).terrain.elevation_lines[0];
        assert_eq!(l.z, 60.0);
        assert_eq!(l.points.len(), 3 * SPLINE_SAMPLES + 1);

        t.set_variant(ToolId::TerrainVariant(TerrainVariant::Break));
        clicks(&mut t, &mut cx, &PATH);
        enter(&mut t, &mut cx);
        assert!(cx.readout.as_deref().unwrap().contains("Break elevation"));
        // The last elevation is the default.
        type_value(&mut t, &mut cx, "");
        let b = &record(&cx).terrain.breaks[0];
        assert_eq!((b.z, b.points.len()), (60.0, 2));
    }

    #[test]
    fn a_break_line_changes_the_built_surface() {
        let mut cx = cx();
        sloped_terrain(&mut cx);
        click(&mut tool(TerrainVariant::Build), &mut cx, 0.0, 0.0);
        let before = terrain_view(&cx.project).unwrap();
        let z0 = elevation_at(before.surface.as_ref().unwrap(), Point::new(600.0, 480.0)).unwrap();

        let mut t = tool(TerrainVariant::Break);
        clicks(&mut t, &mut cx, &[(600.0, 0.0), (600.0, 960.0)]);
        enter(&mut t, &mut cx);
        type_value(&mut t, &mut cx, "20'");
        let after = terrain_view(&cx.project).unwrap();
        let z1 = elevation_at(after.surface.as_ref().unwrap(), Point::new(600.0, 480.0)).unwrap();
        assert!(
            (z0 - 60.0).abs() < 1.0 && (z1 - 240.0).abs() < 1.0,
            "{z0} -> {z1}"
        );
        assert_eq!(cx.undo().as_deref(), Some("Terrain Break"));
    }

    #[test]
    fn splined_roads_ask_for_a_width_like_their_polyline_versions() {
        let mut cx = cx();
        let mut t = tool(TerrainVariant::SplineDriveway);
        clicks(&mut t, &mut cx, &CURVE);
        let p = PointerEvent::at(&cx, Point::new(450.0, 160.0));
        t.double_click(&mut cx, p);
        assert!(cx.readout.as_deref().unwrap().contains("Driveway width"));
        type_value(&mut t, &mut cx, "");
        let r = &record(&cx).terrain.roads[0];
        assert_eq!(
            (r.kind, r.width, r.centerline.len()),
            (RoadKind::Driveway, 144.0, 3 * SPLINE_SAMPLES + 1)
        );
        t.set_variant(ToolId::TerrainVariant(TerrainVariant::SplineRoad));
        clicks(&mut t, &mut cx, &CURVE);
        enter(&mut t, &mut cx);
        assert!(cx.readout.as_deref().unwrap().contains("Road width"));
        t.key(&mut cx, KeyEvent::escape());
        t.set_variant(ToolId::TerrainVariant(TerrainVariant::SplineSidewalk));
        clicks(&mut t, &mut cx, &CURVE);
        enter(&mut t, &mut cx);
        type_value(&mut t, &mut cx, "");
        assert_eq!(record(&cx).terrain.roads[1].kind, RoadKind::Sidewalk);
    }

    #[test]
    fn stepping_stones_plants_and_sprinklers_are_counted_along_the_path() {
        let mut cx = cx();
        draw(&mut cx, TerrainVariant::StonePolyline);
        draw(&mut cx, TerrainVariant::PlantPolyline);
        draw(&mut cx, TerrainVariant::SprinklerPolyline);
        let t = record(&cx).terrain;
        let (stones, plants, sprinklers) = (&t.landscape[0], &t.landscape[1], &t.landscape[2]);
        // 300" of path at 30" per stone.
        assert_eq!(plan_terrain::path_length(&stones.points), 300.0);
        assert_eq!(stones.stones().len(), 10);
        // Plants start with the library's boxwood: one per canopy width.
        assert!(plants.plant.starts_with("core.plants."), "{}", plants.plant);
        assert_eq!(plants.spacing, plants.size);
        assert_eq!(
            plants.plant_positions().len(),
            (300.0 / plants.spacing).floor() as usize + 1
        );
        // 144" between heads: 0, 144, 288.
        assert_eq!(sprinklers.heads().len(), 3);
        assert_eq!(sprinklers.layer(), "Sprinklers");
        assert_eq!(plants.layer(), "Plants");
    }

    #[test]
    fn beds_grass_and_water_are_polygons_on_their_layers() {
        let mut cx = cx();
        draw(&mut cx, TerrainVariant::BedPolyline);
        draw(&mut cx, TerrainVariant::GrassPolyline);
        draw(&mut cx, TerrainVariant::WaterPolyline);
        let t = record(&cx).terrain;
        assert_eq!(t.landscape.len(), 3);
        assert_eq!(t.landscape[0].layer(), "Landscaping, Garden Beds");
        assert_eq!(
            (t.landscape[0].material.as_str(), t.landscape[0].edging),
            ("Mulch", true)
        );
        assert_eq!(t.landscape[1].material, "Grass");
        assert_eq!((t.landscape[2].height, t.landscape[2].depth), (6.0, 24.0));
        assert!(t.landscape.iter().all(|o| o.points.len() == 4));
        for layer in [
            "Landscaping, Garden Beds",
            "Landscaping, Grass Regions",
            "Landscaping, Water Features",
            "Terrain, Features",
            "Plants",
            "Sprinklers",
        ] {
            assert!(cx.project.layers.get(layer).is_some(), "{layer}");
        }
    }

    #[test]
    fn a_double_click_opens_the_object_specification_and_ok_is_one_undo_step() {
        let mut cx = cx();
        draw(&mut cx, TerrainVariant::StonePolyline);
        let mut t = tool(TerrainVariant::Hill);
        // The first click of the double-click has started a shape.
        click(&mut t, &mut cx, 150.0, 100.0);
        let p = PointerEvent::at(&cx, Point::new(150.0, 100.0));
        assert!(t.double_click(&mut cx, p).consumed);
        assert!(
            t.object_dialog.borrow().is_some(),
            "the stones' specification"
        );
        assert!(t.points().is_empty());
        assert!(t.pointer_down(&mut cx, p.with_down(true)).consumed);
        assert!(
            t.points().is_empty(),
            "the canvas ignores clicks while it is open"
        );

        let mut draft = t.object_dialog.borrow().as_ref().unwrap().1.draft().clone();
        let TerrainObject::Landscape(l) = &mut draft else {
            panic!("{draft:?}")
        };
        l.spacing = 60.0;
        l.size = 12.0;
        let hit = t.object_dialog.borrow().as_ref().unwrap().0;
        *t.applied_object.borrow_mut() = Some((hit, draft));
        t.object_dialog.borrow_mut().take();
        let depth = cx.undo_label().map(str::to_string);
        let r = t.pointer_move(&mut cx, p);
        assert_eq!(r.commit.as_deref(), Some("Stepping Stone Specification"));
        let o = &record(&cx).terrain.landscape[0];
        assert_eq!((o.spacing, o.size, o.stones().len()), (60.0, 12.0, 5));
        // One undo step restores the stones.
        assert_eq!(cx.undo().as_deref(), Some("Stepping Stone Specification"));
        assert_eq!(record(&cx).terrain.landscape[0].spacing, 30.0);
        assert_eq!(cx.undo_label().map(str::to_string), depth);
    }

    #[test]
    fn delete_and_the_arrow_keys_work_on_landscape_objects() {
        let mut cx = cx();
        draw(&mut cx, TerrainVariant::StraightWall);
        draw(&mut cx, TerrainVariant::BedPolyline);
        let mut t = tool(TerrainVariant::Hill);
        // Over the wall.
        let ev = PointerEvent::at(&cx, Point::new(150.0, 101.0));
        t.pointer_move(&mut cx, ev);
        let r = t.key(&mut cx, KeyEvent::key(Key::ArrowUp));
        assert_eq!(r.commit.as_deref(), Some("Move Terrain Element"));
        let unit = cx.snap_unit();
        assert_eq!(
            record(&cx).terrain.walls[0].points[0],
            Point::new(0.0, 100.0 + unit)
        );
        let r = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete Terrain Element"));
        assert!(record(&cx).terrain.walls.is_empty());
        // Inside the bed (a region is picked by its inside as well).
        let ev = PointerEvent::at(&cx, Point::new(200.0, 200.0));
        t.pointer_move(&mut cx, ev);
        assert!(t.key(&mut cx, KeyEvent::key(Key::Delete)).commit.is_some());
        assert!(record(&cx).terrain.landscape.is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Delete Terrain Element"));
        assert_eq!(record(&cx).terrain.landscape.len(), 1);
    }

    #[test]
    fn landscape_meshes_appear_for_the_drawn_objects() {
        let mut cx = cx();
        sloped_terrain(&mut cx);
        click(&mut tool(TerrainVariant::Build), &mut cx, 0.0, 0.0);
        assert!(crate::editor::site_view::terrain_feature_meshes(&cx.project).is_empty());
        for v in [
            TerrainVariant::StraightWall,
            TerrainVariant::BedPolyline,
            TerrainVariant::StonePolyline,
            TerrainVariant::PlantPolyline,
        ] {
            let before = crate::editor::site_view::terrain_feature_meshes(&cx.project).len();
            draw(&mut cx, v);
            let after = crate::editor::site_view::terrain_feature_meshes(&cx.project).len();
            assert!(after > before, "{} added no mesh", v.name());
        }
    }

    #[test]
    fn every_flavor_draws_its_overlay_in_every_state() {
        let ctx = egui::Context::default();
        for v in TerrainVariant::ALL {
            let mut cx = cx();
            let mut t = tool(v);
            let ev = PointerEvent::at(&cx, Point::new(300.0, 200.0));
            let mut states = Vec::new();
            for (x, y) in [(100.0, 100.0), (400.0, 100.0), (250.0, 220.0)] {
                click(&mut t, &mut cx, x, y);
                let ev2 = PointerEvent::at(&cx, Point::new(x + 40.0, y + 40.0));
                t.pointer_move(&mut cx, ev2);
                states.push(t.points().len());
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let (_, painter) = ui
                            .allocate_painter(egui::Vec2::new(800.0, 600.0), egui::Sense::hover());
                        let mut cam = Camera::default_view();
                        cam.rect = painter.clip_rect();
                        t.draw_overlay(&cx, &painter, &cam);
                    });
                });
            }
            t.pointer_move(&mut cx, ev);
            assert!(!t.hint().is_empty(), "{}", v.name());
        }
    }
}
