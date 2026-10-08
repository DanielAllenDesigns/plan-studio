//! Site view: rendering, hit-testing and storage of the electrical devices
//! (CB-62..CB-67) and the terrain (CB-51, CB-52) in the plan.
//!
//! # Storage
//!
//! The floor's [`ElectricalLayer`] lives in `Floor.electrical` and the
//! project's [`TerrainRecord`] (the `plan_terrain::Terrain` plus the contour
//! interval and the built flag) in `Project.terrain`, both through the typed
//! `*_as` / `set_*` accessors of `plan-core`.
//!
//! Older files kept both as JSON text records in `floor.cad` on the hidden
//! layers `"Electrical, Data"` and `"Terrain, Data"`;
//! [`migrate_legacy_storage`] moves them into the typed slots when a project
//! is loaded (and does the same for the roof records, see
//! `roof_view::migrate_legacy`). Undo and redo restore the slots with the
//! rest of the project. [`draw_site`] and [`draw_devices`] are called from
//! `render::draw_plan`; the parsed layers and the built terrain surface are
//! cached per thread, keyed by a comparison with the stored value.

use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Shape};
use plan_core::cad::CadItem;
use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::units::fmt_ft_in_frac;
use plan_core::{Floor, Id, Project, Wall};
use plan_electrical::{place_on_wall, Device, ElectricalLayer, Stroke as ElStroke, WallSide};
use plan_terrain::{
    auto_hole_for_building, build_terrain_with_progress, contours_with, landscape_plan,
    plan_symbols, BuildStage, Contour, FeatureKind, ModifierKind, PlanItem,
    Stroke as TerrainStroke, StrokeKind, Terrain, TerrainSurface,
};

mod landscape;
mod site_plan;
#[allow(unused_imports)] // `terrain_feature_meshes` is for the 3D scene
pub use landscape::{
    all_hits, draw_landscape, ensure_landscape_layers, hit_exists, hit_for_mesh_id, hit_is_closed,
    hit_layer, hit_mesh_id, hit_points, hit_type_name, move_terrain_element, move_terrain_elements,
    move_terrain_vertex, object_at, remove_terrain_elements, replace_object,
    terrain_feature_meshes, TerrainObject,
};
use serde_json::{json, Value};
#[allow(unused_imports)] // the sun angle reads `plan_sun_azimuth`
pub use site_plan::{
    ensure_site_plan_layer, north_angle, place_north_pointer, place_scale_bar, plan_sun_azimuth,
};
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// Legacy hidden layer of the electrical record (older files only).
pub const ELECTRICAL_DATA_LAYER: &str = "Electrical, Data";
/// Legacy hidden layer of the terrain record (older files only).
pub const TERRAIN_DATA_LAYER: &str = "Terrain, Data";
/// The visible layer devices are drawn on.
pub const ELECTRICAL_LAYER: &str = "Electrical";
/// Default visible layer of the terrain.
pub const TERRAIN_LAYER: &str = "Terrain";
/// Chief's default contour interval, inches.
pub const DEFAULT_CONTOUR_INTERVAL: f64 = 12.0;
/// How near a click must be to a device symbol to pick it, inches (at least).
const DEVICE_PICK_MIN: f64 = 5.0;

// ----- storage -----

/// The electrical layer of `floor` (empty when it has none).
pub fn load_electrical(floor: &Floor) -> ElectricalLayer {
    floor
        .electrical_as::<ElectricalLayer>()
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Stores `layer` as the electrical data of `floor`. An empty layer on a
/// floor without electrical data writes nothing.
pub fn save_electrical(project: &mut Project, floor: usize, layer: &ElectricalLayer) {
    let f = &mut project.floors[floor];
    if f.electrical.is_none() && layer.devices.is_empty() && layer.connections.is_empty() {
        return;
    }
    // Serializing plain data cannot fail; keep the old value if it ever does.
    let _ = f.set_electrical(layer);
}

/// The project's terrain with the settings the Terrain Specification edits.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainRecord {
    pub terrain: Terrain,
    /// Contour interval, inches.
    pub contour_interval: f64,
    /// Build Terrain was run: the surface and contours are shown.
    pub built: bool,
    /// The visible layer the terrain is drawn on.
    pub layer: String,
    /// Rebuild the surface after every edit of the terrain. When off, the
    /// surface stays as built until Build Terrain runs again (the view is
    /// marked stale).
    pub auto_rebuild: bool,
    /// [`terrain_key`] of the data the last Build Terrain used (0 = unknown);
    /// only read while `auto_rebuild` is off.
    pub built_key: u64,
}

impl TerrainRecord {
    /// A record with no perimeter yet and no data.
    pub fn new() -> Self {
        Self {
            terrain: Terrain {
                perimeter: Vec::new(),
                ..Terrain::default()
            },
            contour_interval: DEFAULT_CONTOUR_INTERVAL,
            built: false,
            layer: TERRAIN_LAYER.to_string(),
            auto_rebuild: true,
            built_key: 0,
        }
    }

    pub fn has_perimeter(&self) -> bool {
        self.terrain.perimeter.len() >= 3
    }

    /// Takes the settings of a Terrain Specification `draft` (OK in the
    /// dialog): everything the dialog edits, nothing it only displays.
    pub fn apply_spec(&mut self, draft: &TerrainRecord) {
        self.contour_interval = draft.contour_interval;
        self.layer = draft.layer.clone();
        let was_auto = self.auto_rebuild;
        self.auto_rebuild = draft.auto_rebuild;
        let (t, d) = (&mut self.terrain, &draft.terrain);
        t.subfloor_height_above_terrain = d.subfloor_height_above_terrain;
        t.building_pad_elevation = d.building_pad_elevation;
        t.smoothing = d.smoothing;
        t.grid_spacing = d.grid_spacing;
        t.subdivision = d.subdivision;
        t.contour_major_every = d.contour_major_every;
        t.contour_label_spacing = d.contour_label_spacing;
        t.contour_label_major_only = d.contour_label_major_only;
        t.flatten_pad = d.flatten_pad;
        t.north_angle = d.north_angle;
        if let (Some(pad), Some(dp)) = (t.building_pad.as_mut(), d.building_pad.as_ref()) {
            pad.margin = dp.margin;
            pad.slope_ratio = dp.slope_ratio;
            pad.first_floor = dp.first_floor;
        }
        if was_auto && !self.auto_rebuild {
            // Turning auto rebuild off: the surface as it is now is the baseline.
            self.built_key = terrain_key(self);
        }
    }

    fn to_value(&self) -> Option<Value> {
        let terrain = serde_json::to_value(&self.terrain).ok()?;
        Some(json!({
            "terrain": terrain,
            "contour_interval": self.contour_interval,
            "built": self.built,
            "layer": self.layer,
            "auto_rebuild": self.auto_rebuild,
            "built_key": self.built_key,
        }))
    }

    fn from_value(v: &Value) -> Option<Self> {
        let terrain: Terrain = serde_json::from_value(v.get("terrain")?.clone()).ok()?;
        let base = Self::new();
        Some(Self {
            terrain,
            contour_interval: v
                .get("contour_interval")
                .and_then(Value::as_f64)
                .filter(|i| *i > 0.0)
                .unwrap_or(base.contour_interval),
            built: v.get("built").and_then(Value::as_bool).unwrap_or(false),
            layer: v
                .get("layer")
                .and_then(Value::as_str)
                .map_or(base.layer, str::to_string),
            auto_rebuild: v
                .get("auto_rebuild")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            built_key: v.get("built_key").and_then(Value::as_u64).unwrap_or(0),
        })
    }
}

impl Default for TerrainRecord {
    fn default() -> Self {
        Self::new()
    }
}

/// The project's terrain record, if one was saved.
pub fn load_terrain(project: &Project) -> Option<TerrainRecord> {
    project.terrain.as_ref().and_then(TerrainRecord::from_value)
}

/// Stores the terrain record in `Project.terrain`.
pub fn save_terrain(project: &mut Project, rec: &TerrainRecord) {
    if let Some(v) = rec.to_value() {
        project.terrain = Some(v);
    }
}

// ----- migration of the old CAD-record storage -----

/// Removes the JSON text records on `layer` from `floor.cad` and returns the
/// first one that parses. Records that do not parse are left in place.
fn take_record(floor: &mut Floor, layer: &str) -> Option<Value> {
    let parsed = floor.cad.iter().find_map(|c| match &c.item {
        CadItem::Text { text, .. } if c.layer == layer => serde_json::from_str::<Value>(text).ok(),
        _ => None,
    })?;
    floor
        .cad
        .retain(|c| !(c.layer == layer && matches!(c.item, CadItem::Text { .. })));
    Some(parsed)
}

/// Drops the hidden record layer `name` once no CAD item uses it.
pub(crate) fn drop_data_layer(project: &mut Project, name: &str) {
    if project
        .floors
        .iter()
        .any(|f| f.cad.iter().any(|c| c.layer == name))
    {
        return;
    }
    project.layers.layers.retain(|l| l.name != name);
}

fn migrate_electrical(project: &mut Project) -> bool {
    let mut changed = false;
    for floor in &mut project.floors {
        if floor.electrical.is_some() {
            continue;
        }
        if let Some(v) = take_record(floor, ELECTRICAL_DATA_LAYER) {
            floor.electrical = Some(v);
            changed = true;
        }
    }
    drop_data_layer(project, ELECTRICAL_DATA_LAYER);
    changed
}

fn migrate_terrain(project: &mut Project) -> bool {
    let mut changed = false;
    if project.terrain.is_none() {
        for floor in &mut project.floors {
            if let Some(v) = take_record(floor, TERRAIN_DATA_LAYER) {
                project.terrain = Some(v);
                changed = true;
                break;
            }
        }
    }
    drop_data_layer(project, TERRAIN_DATA_LAYER);
    changed
}

/// Project-load step ("Migrate roof storage", no undo entry): moves the roof,
/// electrical and terrain records that older files kept as hidden CAD text
/// into `Floor.roofs`, `Floor.electrical` and `Project.terrain`, removes the
/// legacy items and the hidden "..., Data" layers, and returns whether
/// anything moved. Slots that are already filled are left alone.
pub fn migrate_legacy_storage(project: &mut Project) -> bool {
    let mut changed = super::roof_view::migrate_legacy(project);
    changed |= migrate_electrical(project);
    changed |= migrate_terrain(project);
    changed
}

// ----- caches -----

type ElecCache = Option<(usize, Value, Rc<ElectricalLayer>)>;
type TerrainCache = Option<(Value, Rc<TerrainView>)>;

/// The last surface and contours built, with the [`terrain_key`] of their data.
type SurfaceCache = Option<(u64, TerrainSurface, Vec<Contour>)>;

thread_local! {
    static ELEC_CACHE: RefCell<ElecCache> = const { RefCell::new(None) };
    static TERRAIN_CACHE: RefCell<TerrainCache> = const { RefCell::new(None) };
    static SURFACE_CACHE: RefCell<SurfaceCache> = const { RefCell::new(None) };
}

/// The floor's electrical layer, parsed once per change of the stored value.
pub fn electrical_layer(floor_index: usize, floor: &Floor) -> Rc<ElectricalLayer> {
    let Some(value) = floor.electrical.as_ref() else {
        return Rc::new(ElectricalLayer::default());
    };
    ELEC_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((i, v, layer)) = c.as_ref() {
            if *i == floor_index && v == value {
                return layer.clone();
            }
        }
        let layer: Rc<ElectricalLayer> =
            Rc::new(serde_json::from_value(value.clone()).unwrap_or_default());
        *c = Some((floor_index, value.clone(), layer.clone()));
        layer
    })
}

/// The terrain plus what was derived from it (surface and contours once built).
pub struct TerrainView {
    pub record: TerrainRecord,
    pub surface: Option<TerrainSurface>,
    pub contours: Vec<Contour>,
    pub symbols: Vec<TerrainStroke>,
    /// Landscape objects (features, walls, beds, plants, ...) as plan items.
    pub landscape: Vec<PlanItem>,
    /// The surface is older than the terrain data: auto rebuild is off and the
    /// terrain was edited since Build Terrain.
    pub stale: bool,
}

/// Identifies the terrain data (and contour interval) a surface is built from.
pub fn terrain_key(rec: &TerrainRecord) -> u64 {
    let mut h = DefaultHasher::new();
    if let Ok(text) = serde_json::to_string(&rec.terrain) {
        text.hash(&mut h);
    }
    rec.contour_interval.to_bits().hash(&mut h);
    h.finish() | 1
}

/// Builds the surface and contours of `rec` (Build Terrain), calling
/// `progress(stage, fraction)` as each step starts, and remembers them as the
/// surface of that data.
pub fn build_surface_with_progress(
    rec: &TerrainRecord,
    progress: &mut dyn FnMut(BuildStage, f32),
) -> (TerrainSurface, Vec<Contour>) {
    let s = build_terrain_with_progress(&rec.terrain, progress);
    let c = contours_with(&s, rec.contour_interval, rec.terrain.contour_major_every);
    let key = terrain_key(rec);
    SURFACE_CACHE.with(|cache| {
        *cache.borrow_mut() = Some((key, s.clone(), c.clone()));
    });
    (s, c)
}

impl TerrainView {
    fn new(record: TerrainRecord) -> Self {
        let mut stale = false;
        let (surface, cont) = if record.built && record.has_perimeter() {
            let key = terrain_key(&record);
            // With auto rebuild off, keep showing the surface Build Terrain made.
            let held = (!record.auto_rebuild && record.built_key != 0 && record.built_key != key)
                .then(|| {
                    SURFACE_CACHE.with(|c| {
                        c.borrow()
                            .as_ref()
                            .filter(|(k, _, _)| *k == record.built_key)
                            .map(|(_, s, c)| (s.clone(), c.clone()))
                    })
                })
                .flatten();
            let current = || {
                SURFACE_CACHE.with(|c| {
                    c.borrow()
                        .as_ref()
                        .filter(|(k, _, _)| *k == key)
                        .map(|(_, s, c)| (s.clone(), c.clone()))
                })
            };
            if let Some((s, c)) = held {
                stale = true;
                (Some(s), c)
            } else if let Some((s, c)) = current() {
                (Some(s), c)
            } else {
                let (s, c) = build_surface_with_progress(&record, &mut |_, _| {});
                (Some(s), c)
            }
        } else {
            (None, Vec::new())
        };
        let symbols = plan_symbols(&record.terrain, &cont);
        let landscape = landscape_plan(&record.terrain);
        Self {
            record,
            surface,
            contours: cont,
            symbols,
            landscape,
            stale,
        }
    }
}

/// The built view of the project's terrain (cached by the stored value).
pub fn terrain_view(project: &Project) -> Option<Rc<TerrainView>> {
    let value = project.terrain.as_ref()?;
    TERRAIN_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((v, view)) = c.as_ref() {
            if v == value {
                return Some(view.clone());
            }
        }
        let view = Rc::new(TerrainView::new(TerrainRecord::from_value(value)?));
        *c = Some((value.clone(), view.clone()));
        Some(view)
    })
}

/// Ground elevation at `p` from the built terrain (inches), if any.
pub fn terrain_elevation_at(project: &Project, p: Point) -> Option<f64> {
    let view = terrain_view(project)?;
    plan_terrain::elevation_at(view.surface.as_ref()?, p)
}

// ----- editing helpers -----

/// Runs `edit` on the terrain record as one undo step named `label`.
pub fn edit_terrain(cx: &mut EditorContext, label: &str, edit: impl FnOnce(&mut TerrainRecord)) {
    cx.begin_change(label);
    let mut rec = load_terrain(&cx.project).unwrap_or_default();
    edit(&mut rec);
    save_terrain(&mut cx.project, &rec);
    cx.mark_dirty();
}

/// Runs `edit` on the current floor's electrical layer as one undo step.
pub fn edit_electrical(
    cx: &mut EditorContext,
    label: &str,
    edit: impl FnOnce(&mut ElectricalLayer, &Floor),
) {
    cx.begin_change(label);
    let mut layer = load_electrical(cx.floor());
    edit(&mut layer, cx.floor());
    let fl = cx.floor;
    save_electrical(&mut cx.project, fl, &layer);
    cx.mark_dirty();
}

/// Is `a` the same polygon as `b` (same points within a tolerance)?
fn same_polygon(a: &[Point], b: &[Point]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(p, q)| p.dist(*q) < 0.01)
}

/// Convex hull (monotone chain), counter-clockwise.
fn convex_hull(points: &[Point]) -> Vec<Point> {
    let mut pts: Vec<Point> = points.to_vec();
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| a.dist(*b) < 1e-9);
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: Point, a: Point, b: Point| a.sub(o).cross(b.sub(o));
    let mut hull: Vec<Point> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &Point>> = if pass == 0 {
            Box::new(pts.iter())
        } else {
            Box::new(pts.iter().rev())
        };
        for &p in iter {
            while hull.len() >= start + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

/// The footprint of the building on the current floor: the convex hull of the
/// exterior walls (all walls when there are none).
pub fn building_footprint(floor: &Floor) -> Vec<Point> {
    let exterior = floor
        .walls
        .iter()
        .filter(|w| w.kind == plan_core::WallKind::Exterior);
    let mut pts: Vec<Point> = exterior.flat_map(|w| w.footprint()).collect();
    if pts.is_empty() {
        pts = floor.walls.iter().flat_map(|w| w.footprint()).collect();
    }
    convex_hull(&pts)
}

/// Terrain > Make Terrain Hole Around Building: adds a hole 12" outside the
/// building footprint. Returns false (changing nothing) when there is no
/// building or the hole already exists.
pub fn auto_building_hole(cx: &mut EditorContext) -> bool {
    let footprint = building_footprint(cx.floor());
    if footprint.len() < 3 {
        cx.status = "Draw the building walls first".into();
        return false;
    }
    let mut rec = load_terrain(&cx.project).unwrap_or_default();
    let before = rec.terrain.features.len();
    auto_hole_for_building(&mut rec.terrain, &footprint);
    let added = rec.terrain.features.len() > before;
    let duplicate = added && {
        let new = &rec.terrain.features[before];
        rec.terrain.features[..before]
            .iter()
            .any(|f| f.kind == FeatureKind::Hole && same_polygon(&f.polygon, &new.polygon))
    };
    if !added || duplicate {
        cx.status = "The terrain already has a hole around the building".into();
        return false;
    }
    cx.begin_change("Terrain Hole Around Building");
    save_terrain(&mut cx.project, &rec);
    cx.mark_dirty();
    true
}

/// Terrain > Building Pad: levels the terrain under the building. The pad is
/// the footprint of the exterior walls plus a margin, its top the first
/// floor's elevation less the terrain-to-first-floor distance, its sides
/// sloped back to the ground. Returns false (changing nothing) when there is
/// no building or the pad is already as it would be made.
pub fn auto_building_pad(cx: &mut EditorContext) -> bool {
    let footprint = building_footprint(cx.floor());
    if footprint.len() < 3 {
        cx.status = "Draw the building walls first".into();
        return false;
    }
    let first_floor = cx.project.floors.first().map_or(0.0, |f| f.elevation);
    let mut rec = load_terrain(&cx.project).unwrap_or_default();
    let mut pad = rec.terrain.building_pad.take().unwrap_or_default();
    let unchanged = same_polygon(&pad.footprint, &footprint)
        && pad.first_floor == Some(first_floor)
        && rec.terrain.flatten_pad;
    if unchanged {
        rec.terrain.building_pad = Some(pad);
        cx.status = "The terrain already has a building pad".into();
        return false;
    }
    pad.footprint = footprint;
    pad.first_floor = Some(first_floor);
    rec.terrain.building_pad_elevation = first_floor - rec.terrain.subfloor_height_above_terrain;
    rec.terrain.building_pad = Some(pad);
    rec.terrain.flatten_pad = true;
    cx.begin_change("Building Pad");
    save_terrain(&mut cx.project, &rec);
    cx.mark_dirty();
    true
}

// ----- terrain hit-testing -----

/// A terrain element under the pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TerrainHit {
    Perimeter,
    Point(usize),
    Line(usize),
    Region(usize),
    Modifier(usize),
    Feature(usize),
    Road(usize),
    Break(usize),
    Wall(usize),
    Landscape(usize),
}

fn near_polygon(poly: &[Point], p: Point, tol: f64) -> bool {
    let n = poly.len();
    n >= 2 && (0..n).any(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]) <= tol)
}

fn near_polyline(pts: &[Point], p: Point, tol: f64) -> bool {
    pts.windows(2)
        .any(|w| dist_to_segment(p, w[0], w[1]) <= tol)
}

/// The terrain element nearest `p` within `tol` inches (points first, then
/// lines, edges of regions and modifiers, features, roads, the perimeter).
pub fn hit_terrain(t: &Terrain, p: Point, tol: f64) -> Option<TerrainHit> {
    if let Some(i) = t.elevation_points.iter().position(|e| e.pos.dist(p) <= tol) {
        return Some(TerrainHit::Point(i));
    }
    if let Some(i) = t
        .elevation_lines
        .iter()
        .position(|l| near_polyline(&l.points, p, tol))
    {
        return Some(TerrainHit::Line(i));
    }
    if let Some(i) = t
        .modifiers
        .iter()
        .position(|m| near_polygon(&m.polygon, p, tol))
    {
        return Some(TerrainHit::Modifier(i));
    }
    if let Some(i) = t
        .elevation_regions
        .iter()
        .position(|r| near_polygon(&r.polygon, p, tol))
    {
        return Some(TerrainHit::Region(i));
    }
    if let Some(i) = t
        .features
        .iter()
        .position(|f| near_polygon(&f.polygon, p, tol))
    {
        return Some(TerrainHit::Feature(i));
    }
    if let Some(i) = t
        .roads
        .iter()
        .position(|r| near_polyline(&r.centerline, p, tol.max(r.width * 0.5)))
    {
        return Some(TerrainHit::Road(i));
    }
    if let Some(i) = t
        .breaks
        .iter()
        .position(|b| near_polyline(&b.points, p, tol))
    {
        return Some(TerrainHit::Break(i));
    }
    if let Some(i) = t
        .walls
        .iter()
        .position(|w| near_polyline(&w.points, p, tol.max(w.thickness * 0.5)))
    {
        return Some(TerrainHit::Wall(i));
    }
    // Paths and region outlines first, then the inside of regions.
    if let Some(i) = t.landscape.iter().position(|l| {
        if l.is_region() {
            near_polygon(&l.points, p, tol)
        } else {
            near_polyline(&l.points, p, tol.max(l.size * 0.5))
        }
    }) {
        return Some(TerrainHit::Landscape(i));
    }
    if let Some(i) = t
        .landscape
        .iter()
        .position(|l| l.is_region() && point_in_polygon(p, &l.points))
    {
        return Some(TerrainHit::Landscape(i));
    }
    if let Some(i) = t
        .features
        .iter()
        .position(|f| f.kind != FeatureKind::Hole && point_in_polygon(p, &f.polygon))
    {
        return Some(TerrainHit::Feature(i));
    }
    near_polygon(&t.perimeter, p, tol).then_some(TerrainHit::Perimeter)
}

/// Removes the hit element; returns whether anything was removed.
pub fn remove_terrain_element(t: &mut Terrain, hit: TerrainHit) -> bool {
    fn remove<T>(v: &mut Vec<T>, i: usize) -> bool {
        (i < v.len()).then(|| v.remove(i)).is_some()
    }
    match hit {
        TerrainHit::Perimeter => {
            let had = !t.perimeter.is_empty();
            t.perimeter.clear();
            had
        }
        TerrainHit::Point(i) => remove(&mut t.elevation_points, i),
        TerrainHit::Line(i) => remove(&mut t.elevation_lines, i),
        TerrainHit::Region(i) => remove(&mut t.elevation_regions, i),
        TerrainHit::Modifier(i) => remove(&mut t.modifiers, i),
        TerrainHit::Feature(i) => remove(&mut t.features, i),
        TerrainHit::Road(i) => remove(&mut t.roads, i),
        TerrainHit::Break(i) => remove(&mut t.breaks, i),
        TerrainHit::Wall(i) => remove(&mut t.walls, i),
        TerrainHit::Landscape(i) => remove(&mut t.landscape, i),
    }
}

// ----- device geometry -----

/// The device under `p` (nearest within `tol`, but at least the symbol size).
pub fn device_at(layer: &ElectricalLayer, p: Point, tol: f64) -> Option<Id> {
    let reach = tol.max(DEVICE_PICK_MIN);
    layer
        .devices
        .iter()
        .map(|d| (d.id, device_distance(d, p)))
        .filter(|(_, dist)| *dist <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

fn device_distance(d: &Device, p: Point) -> f64 {
    if let plan_electrical::DeviceKind::RopeLight { length } = d.kind {
        // Local +Y is the strip's direction.
        let dir = Point::new(-d.angle.sin(), d.angle.cos());
        dist_to_segment(p, d.position, d.position + dir * length)
    } else {
        d.position.dist(p)
    }
}

/// Which face of `wall` the wall-mounted device `d` sits on.
pub fn device_side(d: &Device, wall: &Wall) -> WallSide {
    let (_, on_wall) = plan_core::geometry::project_on_segment(d.position, wall.start, wall.end);
    if d.position.sub(on_wall).dot(wall.normal()) >= 0.0 {
        WallSide::Left
    } else {
        WallSide::Right
    }
}

/// A copy of `d` moved to `offset` along `wall` on `side` (id, height, label,
/// circuit and switches are kept).
pub fn device_on_wall(d: &Device, wall: &Wall, offset: f64, side: WallSide) -> Device {
    let placed = place_on_wall(d.kind, wall, offset.clamp(0.0, wall.length()), side);
    Device {
        id: d.id,
        height: d.height,
        circuit: d.circuit,
        label: d.label.clone(),
        switched_by: d.switched_by.clone(),
        finish: d.finish.clone(),
        hide_label: d.hide_label,
        ..placed
    }
}

/// Slides the wall device `d` along its `wall` to the projection of `p`
/// (rounded to `unit`), keeping the face it is on.
pub fn slide_on_wall(d: &mut Device, wall: &Wall, p: Point, unit: f64) {
    let (t, _) = plan_core::geometry::project_on_segment(p, wall.start, wall.end);
    let mut offset = t * wall.length();
    if unit > 0.0 {
        offset = (offset / unit).round() * unit;
    }
    *d = device_on_wall(d, wall, offset, device_side(d, wall));
}

/// Flip side: a wall device moves to the other face of its wall; a free
/// device turns around.
pub fn flip_side(d: &mut Device, wall: Option<&Wall>) {
    match (d.wall_id, wall) {
        (Some(_), Some(w)) => {
            let (t, _) = plan_core::geometry::project_on_segment(d.position, w.start, w.end);
            let side = match device_side(d, w) {
                WallSide::Left => WallSide::Right,
                WallSide::Right => WallSide::Left,
            };
            *d = device_on_wall(d, w, t * w.length(), side);
        }
        _ => d.angle = (d.angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU),
    }
}

// ----- drawing -----

const PERIMETER_COLOR: Color32 = Color32::from_rgb(0x4F, 0x7F, 0x3A);
const CONTOUR_COLOR: Color32 = Color32::from_rgb(0x9C, 0x78, 0x4A);
const MAJOR_CONTOUR_COLOR: Color32 = Color32::from_rgb(0x7A, 0x55, 0x2B);
const FEATURE_COLOR: Color32 = Color32::from_rgb(0x3C, 0x7F, 0xA8);
const ROAD_COLOR: Color32 = Color32::from_rgb(0x70, 0x70, 0x78);
const DATA_COLOR: Color32 = Color32::from_rgb(0xB0, 0x40, 0x30);
const MODIFIER_COLOR: Color32 = Color32::from_rgb(0x2E, 0x8B, 0x7A);
/// Smallest on-screen text height worth drawing, pixels.
const MIN_TEXT_PX: f32 = 5.0;

fn sc(cam: &Camera, p: Point) -> Pos2 {
    cam.world_to_screen(p)
}

/// A world polyline in `stroke`, optionally closed and dashed.
pub fn draw_polyline(
    painter: &egui::Painter,
    cam: &Camera,
    pts: &[Point],
    closed: bool,
    stroke: egui::Stroke,
    dashed: bool,
) {
    if pts.len() < 2 {
        return;
    }
    let mut screen: Vec<Pos2> = pts.iter().map(|p| sc(cam, *p)).collect();
    if closed {
        screen.push(screen[0]);
    }
    if dashed {
        painter.extend(Shape::dashed_line(&screen, stroke, 8.0, 5.0));
    } else {
        painter.add(Shape::line(screen, stroke));
    }
}

fn draw_label(
    painter: &egui::Painter,
    cam: &Camera,
    at: Point,
    text: &str,
    height: f64,
    c: Color32,
) {
    let px = (height * cam.px_per_in) as f32;
    if px < MIN_TEXT_PX {
        return;
    }
    painter.text(
        sc(cam, at),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(px.min(40.0)),
        c,
    );
}

/// A label centered on `at`, turned `angle` radians counter-clockwise (plan y up).
fn draw_label_rotated(
    painter: &egui::Painter,
    cam: &Camera,
    at: Point,
    text: &str,
    height: f64,
    angle: f64,
    c: Color32,
) {
    if angle.abs() < 1e-6 {
        draw_label(painter, cam, at, text, height, c);
        return;
    }
    let px = (height * cam.px_per_in) as f32;
    if px < MIN_TEXT_PX {
        return;
    }
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(px.min(40.0)), c);
    // egui turns text clockwise about its top-left corner; the plan turns counter-clockwise.
    let a = -(angle as f32);
    let half = galley.size() / 2.0;
    let rot = egui::emath::Rot2::from_angle(a);
    let top_left = sc(cam, at) - rot * half;
    painter.add(egui::epaint::TextShape::new(top_left, galley, c).with_angle(a));
}

fn centroid(pts: &[Point]) -> Point {
    let n = pts.len().max(1) as f64;
    let sum = pts.iter().fold(Point::ZERO, |a, p| a + *p);
    Point::new(sum.x / n, sum.y / n)
}

fn modifier_name(k: ModifierKind) -> &'static str {
    match k {
        ModifierKind::Hill => "Hill",
        ModifierKind::Valley => "Valley",
        ModifierKind::RaisedRegion => "Raised",
        ModifierKind::LoweredRegion => "Lowered",
        ModifierKind::FlatRegion => "Flat",
    }
}

/// The terrain under everything: perimeter (heavy), contours (major heavier,
/// labeled), road edges, features, elevation data and modifiers (CB-51, CB-52).
pub fn draw_site(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let Some(view) = terrain_view(&cx.project) else {
        return;
    };
    draw_landscape(cx, painter, cam, &view.landscape);
    if !cx.layers().is_visible(&view.record.layer) {
        return;
    }
    let px = |w: f64, min: f32| ((w as f32) * 1.6).max(min);
    if view.stale {
        // Auto rebuild is off and the terrain was edited since Build Terrain.
        if let Some(p) = view.record.terrain.perimeter.first() {
            draw_label(
                painter,
                cam,
                p.add(Point::new(0.0, 24.0)),
                "Terrain out of date: run Build Terrain",
                10.0,
                DATA_COLOR,
            );
        }
    }
    for s in &view.symbols {
        match s {
            TerrainStroke::Polyline {
                points,
                closed,
                weight,
                kind,
            } => {
                let (color, width) = match kind {
                    StrokeKind::Perimeter => (PERIMETER_COLOR, 3.0),
                    StrokeKind::MajorContour => (MAJOR_CONTOUR_COLOR, px(*weight, 1.4)),
                    StrokeKind::Contour => (CONTOUR_COLOR, px(*weight, 0.8)),
                    StrokeKind::Feature => (FEATURE_COLOR, px(*weight, 1.0)),
                    StrokeKind::RoadEdge => (ROAD_COLOR, px(*weight, 1.0)),
                };
                draw_polyline(
                    painter,
                    cam,
                    points,
                    *closed,
                    egui::Stroke::new(width, color),
                    false,
                );
            }
            TerrainStroke::Text {
                at,
                text,
                height,
                angle,
            } => {
                draw_label_rotated(
                    painter,
                    cam,
                    *at,
                    text,
                    *height,
                    *angle,
                    MAJOR_CONTOUR_COLOR,
                );
            }
        }
    }
    let t = &view.record.terrain;
    let data = egui::Stroke::new(1.2_f32, DATA_COLOR);
    for e in &t.elevation_points {
        let c = sc(cam, e.pos);
        painter.line_segment([c + egui::vec2(-6.0, 0.0), c + egui::vec2(6.0, 0.0)], data);
        painter.line_segment([c + egui::vec2(0.0, -6.0), c + egui::vec2(0.0, 6.0)], data);
        painter.text(
            c + egui::vec2(8.0, -8.0),
            Align2::LEFT_BOTTOM,
            fmt_ft_in_frac(e.z, 2),
            FontId::proportional(11.0),
            DATA_COLOR,
        );
    }
    for l in &t.elevation_lines {
        draw_polyline(painter, cam, &l.points, false, data, true);
        if let Some(mid) = l.points.get(l.points.len() / 2) {
            draw_label(painter, cam, *mid, &fmt_ft_in_frac(l.z, 2), 8.0, DATA_COLOR);
        }
    }
    for r in &t.elevation_regions {
        draw_polyline(painter, cam, &r.polygon, true, data, true);
        draw_label(
            painter,
            cam,
            centroid(&r.polygon),
            &fmt_ft_in_frac(r.z, 2),
            8.0,
            DATA_COLOR,
        );
    }
    let modifier = egui::Stroke::new(1.2_f32, MODIFIER_COLOR);
    for m in &t.modifiers {
        draw_polyline(painter, cam, &m.polygon, true, modifier, true);
        let label = if m.kind == ModifierKind::FlatRegion {
            modifier_name(m.kind).to_string()
        } else {
            format!("{} {}", modifier_name(m.kind), fmt_ft_in_frac(m.height, 2))
        };
        draw_label(
            painter,
            cam,
            centroid(&m.polygon),
            &label,
            8.0,
            MODIFIER_COLOR,
        );
    }
    for r in &t.roads {
        draw_polyline(
            painter,
            cam,
            &r.centerline,
            false,
            egui::Stroke::new(0.8_f32, ROAD_COLOR),
            true,
        );
    }
}

/// An electrical plan symbol in world space.
pub fn draw_symbol(
    painter: &egui::Painter,
    cam: &Camera,
    strokes: &[ElStroke],
    color: Color32,
    width: f32,
) {
    let stroke = egui::Stroke::new(width, color);
    for s in strokes {
        match s {
            ElStroke::Line { a, b } => {
                painter.line_segment([sc(cam, *a), sc(cam, *b)], stroke);
            }
            ElStroke::Polyline {
                points,
                closed,
                filled,
            } => {
                let pts: Vec<Pos2> = points.iter().map(|p| sc(cam, *p)).collect();
                if *filled && *closed {
                    painter.add(Shape::convex_polygon(pts, color, stroke));
                } else if *closed {
                    painter.add(Shape::closed_line(pts, stroke));
                } else {
                    painter.add(Shape::line(pts, stroke));
                }
            }
            ElStroke::Arc { .. } => {
                painter.add(Shape::line(arc_points(cam, s), stroke));
            }
            ElStroke::Circle {
                center,
                radius,
                filled,
            } => {
                let r = ((*radius * cam.px_per_in) as f32).max(0.5);
                if *filled {
                    painter.circle_filled(sc(cam, *center), r, color);
                } else {
                    painter.circle_stroke(sc(cam, *center), r, stroke);
                }
            }
            ElStroke::Text { at, text, height } => {
                draw_label(painter, cam, *at, text, *height, color);
            }
        }
    }
}

/// Screen points along an arc stroke (empty for other strokes).
pub fn arc_points(cam: &Camera, s: &ElStroke) -> Vec<Pos2> {
    match s {
        ElStroke::Arc {
            center,
            radius,
            start,
            sweep,
        } => (0..=32)
            .map(|i| {
                let a = start + sweep * f64::from(i) / 32.0;
                sc(
                    cam,
                    Point::new(center.x + radius * a.cos(), center.y + radius * a.sin()),
                )
            })
            .collect(),
        ElStroke::Line { a, b } => vec![sc(cam, *a), sc(cam, *b)],
        _ => Vec::new(),
    }
}

/// The color devices are drawn in: the Electrical layer's.
fn device_color(cx: &EditorContext) -> Color32 {
    let [r, g, b] = cx
        .layers()
        .get(ELECTRICAL_LAYER)
        .map_or([200, 120, 0], |l| l.color);
    Color32::from_rgb(r, g, b)
}

/// Devices and their connection arcs, over the rooms and walls (CB-62..CB-67).
pub fn draw_devices(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    if !cx.layers().is_visible(ELECTRICAL_LAYER) {
        return;
    }
    let layer = electrical_layer(cx.floor, cx.floor());
    if layer.devices.is_empty() {
        return;
    }
    let color = device_color(cx);
    for d in &layer.devices {
        draw_symbol(painter, cam, &d.symbol_world(), color, 1.2);
        if !d.label.is_empty() && !d.hide_label {
            draw_label(
                painter,
                cam,
                d.position + Point::new(0.0, -8.0),
                &d.label,
                3.0,
                color,
            );
        }
    }
    let dash = egui::Stroke::new(1.0_f32, color);
    for c in &layer.connections {
        if let Some(arc) = layer.connection_arc(c) {
            let pts = arc_points(cam, &arc);
            if pts.len() >= 2 {
                painter.extend(Shape::dashed_line(&pts, dash, 5.0, 3.0));
            }
        }
    }
}

/// Is `p` inside the terrain perimeter?
pub fn inside_perimeter(t: &Terrain, p: Point) -> bool {
    t.perimeter.len() >= 3 && point_in_polygon(p, &t.perimeter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::WallKind;
    use plan_electrical::{place_free, DeviceKind};

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    #[test]
    fn electrical_round_trips_through_the_typed_slot() {
        let mut cx = cx();
        let mut layer = ElectricalLayer::default();
        let id = layer.add(place_free(DeviceKind::CeilingLight, Point::new(10.0, 20.0)));
        save_electrical(&mut cx.project, 0, &layer);
        assert!(cx.floor().electrical.is_some());
        assert!(cx.floor().cad.is_empty(), "no CAD record any more");
        let back = load_electrical(cx.floor());
        assert_eq!(back, layer);
        assert_eq!(back.device(id).unwrap().position, Point::new(10.0, 20.0));
        assert_eq!(
            cx.floor().electrical_as::<ElectricalLayer>().unwrap(),
            Some(layer.clone())
        );
        // The slot survives a project JSON round trip.
        let json = cx.project.to_json().unwrap();
        let project = Project::from_json(&json).unwrap();
        assert_eq!(load_electrical(&project.floors[0]), layer);
        // Saving again replaces the data instead of adding another.
        save_electrical(&mut cx.project, 0, &layer);
        assert!(cx.floor().cad.is_empty());
    }

    #[test]
    fn an_empty_layer_without_data_writes_nothing() {
        let mut cx = cx();
        save_electrical(&mut cx.project, 0, &ElectricalLayer::default());
        assert!(cx.floor().electrical.is_none());
        assert!(cx.floor().cad.is_empty());
    }

    #[test]
    fn terrain_round_trips_through_the_typed_slot() {
        let mut cx = cx();
        assert!(load_terrain(&cx.project).is_none());
        let mut rec = TerrainRecord::new();
        rec.terrain.perimeter = vec![
            Point::new(0.0, 0.0),
            Point::new(600.0, 0.0),
            Point::new(600.0, 400.0),
        ];
        rec.terrain
            .elevation_points
            .push(plan_terrain::ElevationPoint {
                pos: Point::new(10.0, 10.0),
                z: 36.0,
            });
        rec.contour_interval = 24.0;
        rec.built = true;
        save_terrain(&mut cx.project, &rec);
        assert_eq!(load_terrain(&cx.project), Some(rec.clone()));
        assert!(cx.project.terrain.is_some());
        let project = Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert_eq!(load_terrain(&project), Some(rec.clone()));
        assert_eq!(
            project.terrain_as::<Value>().unwrap().unwrap()["built"],
            true
        );
        assert!(cx.floor().cad.is_empty());
    }

    /// Writes the pre-typed-slot storage: a JSON text on a hidden layer.
    fn put_legacy_record(project: &mut Project, floor: usize, layer: &str, text: String) {
        let mut l = plan_core::Layer::new(layer, [128, 128, 128], 13);
        l.display = false;
        l.locked = true;
        project.layers.add(l);
        project.add_cad(
            floor,
            layer,
            CadItem::Text {
                pos: Point::ZERO,
                text,
                height: 0.1,
                angle: 0.0,
            },
        );
    }

    #[test]
    fn legacy_electrical_and_terrain_records_migrate_once() {
        let mut project = Project::new("old");
        project.floors.push(Floor::new("2nd Floor", 109.0));
        let mut layer = ElectricalLayer::default();
        layer.add(place_free(DeviceKind::Switch, Point::new(3.0, 4.0)));
        put_legacy_record(
            &mut project,
            1,
            ELECTRICAL_DATA_LAYER,
            serde_json::to_string(&layer).unwrap(),
        );
        let mut rec = TerrainRecord::new();
        rec.terrain.perimeter = vec![
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            Point::new(300.0, 300.0),
        ];
        rec.built = true;
        let terrain_text = rec.to_value().unwrap().to_string();
        put_legacy_record(&mut project, 0, TERRAIN_DATA_LAYER, terrain_text);
        // Something unrelated on the same floor stays.
        project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(1.0, 1.0),
            },
        );

        assert!(migrate_legacy_storage(&mut project));
        assert_eq!(load_electrical(&project.floors[1]), layer);
        assert!(project.floors[0].electrical.is_none());
        assert_eq!(load_terrain(&project), Some(rec.clone()));
        assert_eq!(project.floors[0].cad.len(), 1, "only the line is left");
        assert!(project.floors[1].cad.is_empty());
        assert!(project.layers.get(ELECTRICAL_DATA_LAYER).is_none());
        assert!(project.layers.get(TERRAIN_DATA_LAYER).is_none());
        // Nothing left to migrate.
        assert!(!migrate_legacy_storage(&mut project));
        assert_eq!(load_terrain(&project), Some(rec));
    }

    #[test]
    fn opening_a_project_migrates_the_legacy_records() {
        let mut project = Project::new("old");
        let mut layer = ElectricalLayer::default();
        layer.add(place_free(DeviceKind::Switch, Point::new(3.0, 4.0)));
        put_legacy_record(
            &mut project,
            0,
            ELECTRICAL_DATA_LAYER,
            serde_json::to_string(&layer).unwrap(),
        );
        let mut cx = cx();
        cx.set_project(project);
        assert_eq!(load_electrical(cx.floor()), layer);
        assert!(cx.floor().cad.is_empty());
        assert!(!cx.can_undo(), "the migration is not an undo step");
    }

    #[test]
    fn the_sample_plans_still_load() {
        let samples = [
            (
                "ranch-3bed",
                include_str!("../../../../samples/ranch-3bed.psplan"),
            ),
            (
                "studio-adu",
                include_str!("../../../../samples/studio-adu.psplan"),
            ),
            (
                "two-story-colonial",
                include_str!("../../../../samples/two-story-colonial.psplan"),
            ),
        ];
        for (name, text) in samples {
            let mut p =
                Project::from_json(text).unwrap_or_else(|e| panic!("{name} does not load: {e}"));
            assert!(
                p.floors.iter().any(|f| !f.walls.is_empty()),
                "{name} has walls"
            );
            // Nothing legacy in them, and the new slots default.
            assert!(!migrate_legacy_storage(&mut p), "{name}");
            assert!(p.terrain.is_none(), "{name}");
            assert!(p.floors.iter().all(|f| f.roofs.is_empty()), "{name}");
            for o in p.floors.iter().flat_map(|f| &f.openings) {
                assert!(o.extras.show_open_in_plan, "{name}");
            }
            // And they survive a save with the new fields.
            let again = Project::from_json(&p.to_json().unwrap())
                .unwrap_or_else(|e| panic!("{name} does not reload: {e}"));
            assert_eq!(again.floors.len(), p.floors.len(), "{name}");
            let mut cx = cx();
            cx.set_project(again);
            cx.refresh();
            assert!(!cx.rooms.is_empty(), "{name} has rooms");
        }
    }

    #[test]
    fn migration_keeps_filled_slots_and_unreadable_records() {
        let mut project = Project::new("both");
        let mut typed = ElectricalLayer::default();
        typed.add(place_free(DeviceKind::Switch, Point::ZERO));
        save_electrical(&mut project, 0, &typed);
        let mut old = ElectricalLayer::default();
        old.add(place_free(DeviceKind::Outlet110, Point::new(9.0, 9.0)));
        old.add(place_free(DeviceKind::Outlet110, Point::new(19.0, 9.0)));
        put_legacy_record(
            &mut project,
            0,
            ELECTRICAL_DATA_LAYER,
            serde_json::to_string(&old).unwrap(),
        );
        put_legacy_record(&mut project, 0, TERRAIN_DATA_LAYER, "{not json".into());
        migrate_legacy_storage(&mut project);
        assert_eq!(load_electrical(&project.floors[0]), typed);
        assert!(project.terrain.is_none());
        // Records that could not be moved are not destroyed.
        assert_eq!(project.floors[0].cad.len(), 2);
    }

    #[test]
    fn views_are_cached_until_the_record_changes() {
        let mut cx = cx();
        let mut layer = ElectricalLayer::default();
        layer.add(place_free(DeviceKind::Switch, Point::ZERO));
        save_electrical(&mut cx.project, 0, &layer);
        let a = electrical_layer(0, cx.floor());
        let b = electrical_layer(0, cx.floor());
        assert!(Rc::ptr_eq(&a, &b));
        layer.add(place_free(DeviceKind::Switch, Point::new(5.0, 0.0)));
        save_electrical(&mut cx.project, 0, &layer);
        assert_eq!(electrical_layer(0, cx.floor()).devices.len(), 2);
    }

    #[test]
    fn devices_are_picked_by_symbol_and_rope_lights_along_their_length() {
        let mut layer = ElectricalLayer::default();
        let a = layer.add(place_free(DeviceKind::Outlet110, Point::new(0.0, 0.0)));
        let mut rope = place_free(
            DeviceKind::RopeLight { length: 60.0 },
            Point::new(100.0, 0.0),
        );
        rope.angle = -std::f64::consts::FRAC_PI_2; // local +Y points along +X
        let r = layer.add(rope);
        assert_eq!(device_at(&layer, Point::new(2.0, 1.0), 1.0), Some(a));
        assert_eq!(device_at(&layer, Point::new(40.0, 0.0), 1.0), None);
        assert_eq!(device_at(&layer, Point::new(130.0, 1.0), 1.0), Some(r));
    }

    #[test]
    fn flip_side_moves_a_wall_device_to_the_other_face() {
        let mut cx = cx();
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let wall = cx.floor().wall(w).unwrap().clone();
        let mut d = place_on_wall(DeviceKind::Outlet110, &wall, 100.0, WallSide::Left);
        d.id = 7;
        d.label = "kept".into();
        assert!((d.position.y - 2.25).abs() < 1e-9);
        flip_side(&mut d, Some(&wall));
        assert!((d.position.y + 2.25).abs() < 1e-9);
        assert!((d.position.x - 100.0).abs() < 1e-9);
        assert_eq!((d.id, d.label.as_str()), (7, "kept"));
        // Facing flips with the face.
        assert!((d.angle + std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        slide_on_wall(&mut d, &wall, Point::new(150.4, 30.0), 1.0);
        assert!((d.position.x - 150.0).abs() < 1e-9);
        assert!((d.position.y + 2.25).abs() < 1e-9, "stays on its face");
    }

    #[test]
    fn terrain_elements_are_hit_and_removed() {
        let mut t = Terrain::default();
        t.elevation_points.push(plan_terrain::ElevationPoint {
            pos: Point::new(100.0, 100.0),
            z: 12.0,
        });
        assert_eq!(
            hit_terrain(&t, Point::new(102.0, 99.0), 5.0),
            Some(TerrainHit::Point(0))
        );
        assert_eq!(
            hit_terrain(&t, Point::new(600.0, 1.0), 5.0),
            Some(TerrainHit::Perimeter)
        );
        assert_eq!(hit_terrain(&t, Point::new(600.0, 500.0), 5.0), None);
        assert!(remove_terrain_element(&mut t, TerrainHit::Point(0)));
        assert!(t.elevation_points.is_empty());
        assert!(!remove_terrain_element(&mut t, TerrainHit::Point(0)));
    }

    #[test]
    fn building_hole_is_added_once() {
        let mut cx = cx();
        for (a, b) in [
            ((0.0, 0.0), (240.0, 0.0)),
            ((240.0, 0.0), (240.0, 144.0)),
            ((240.0, 144.0), (0.0, 144.0)),
            ((0.0, 144.0), (0.0, 0.0)),
        ] {
            cx.project.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                7.625,
                109.0,
                WallKind::Exterior,
            );
        }
        assert!(auto_building_hole(&mut cx));
        let rec = load_terrain(&cx.project).unwrap();
        assert_eq!(rec.terrain.features.len(), 1);
        assert_eq!(rec.terrain.features[0].kind, FeatureKind::Hole);
        assert!(!auto_building_hole(&mut cx));
        assert_eq!(load_terrain(&cx.project).unwrap().terrain.features.len(), 1);
        assert_eq!(cx.undo().as_deref(), Some("Terrain Hole Around Building"));
    }

    #[test]
    fn site_drawing_does_not_panic() {
        let mut cx = cx();
        let mut rec = TerrainRecord::new();
        rec.terrain.perimeter = Terrain::default().perimeter;
        rec.terrain.elevation_points = vec![
            plan_terrain::ElevationPoint {
                pos: Point::new(0.0, 0.0),
                z: 0.0,
            },
            plan_terrain::ElevationPoint {
                pos: Point::new(1200.0, 0.0),
                z: 120.0,
            },
        ];
        rec.built = true;
        save_terrain(&mut cx.project, &rec);
        let mut layer = ElectricalLayer::default();
        let a = layer.add(place_free(DeviceKind::Switch, Point::new(50.0, 50.0)));
        let b = layer.add(place_free(
            DeviceKind::CeilingLight,
            Point::new(150.0, 90.0),
        ));
        plan_electrical::connect(&mut layer, a, b);
        save_electrical(&mut cx.project, 0, &layer);
        let egui_ctx = egui::Context::default();
        let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, painter) =
                    ui.allocate_painter(egui::Vec2::new(800.0, 600.0), egui::Sense::hover());
                let mut cam = Camera::default_view();
                cam.rect = painter.clip_rect();
                for scale in [0.1, 0.5, 2.0] {
                    cam.px_per_in = scale;
                    draw_site(&cx, &painter, &cam);
                    draw_devices(&cx, &painter, &cam);
                }
            });
        });
        let view = terrain_view(&cx.project).unwrap();
        assert!(!view.contours.is_empty());
    }
}
