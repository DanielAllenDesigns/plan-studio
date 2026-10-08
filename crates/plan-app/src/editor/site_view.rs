//! Site view: rendering, hit-testing and storage of the electrical devices
//! (CB-62..CB-67) and the terrain (CB-51, CB-52) in the plan.
//!
//! # Storage (TODO: plan-core fields)
//!
//! `plan-core` has no slots for these yet, so both are stored as one JSON
//! record in `floor.cad`, a [`CadItem::Text`] on a reserved, hidden layer:
//!
//! * `"Electrical, Data"`: the floor's `plan_electrical::ElectricalLayer`;
//! * `"Terrain, Data"`: a [`TerrainRecord`] (the `plan_terrain::Terrain` plus the
//!   contour interval and the built flag) for the whole project, kept on the
//!   first floor that holds one (floor 0 for a new record).
//!
//! The layers are added to the project hidden and locked so the renderer and
//! the Select tool skip the records; undo and redo restore them with the rest
//! of the project. [`draw_site`] and [`draw_devices`] are called from
//! `render::draw_plan`; parsed records and the built terrain surface are
//! cached per thread, keyed by a hash of the record text.

use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Shape};
use plan_core::cad::CadItem;
use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::units::fmt_ft_in_frac;
use plan_core::{Floor, Id, Layer, Project, Wall};
use plan_electrical::{place_on_wall, Device, ElectricalLayer, Stroke as ElStroke, WallSide};
use plan_terrain::{
    auto_hole_for_building, build_terrain, contours, plan_symbols, Contour, FeatureKind,
    ModifierKind, Stroke as TerrainStroke, StrokeKind, Terrain, TerrainSurface,
};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// Reserved layer of the electrical record.
pub const ELECTRICAL_DATA_LAYER: &str = "Electrical, Data";
/// Reserved layer of the terrain record.
pub const TERRAIN_DATA_LAYER: &str = "Terrain, Data";
/// The visible layer devices are drawn on.
pub const ELECTRICAL_LAYER: &str = "Electrical";
/// Default visible layer of the terrain.
pub const TERRAIN_LAYER: &str = "Terrain";
/// Chief's default contour interval, inches.
pub const DEFAULT_CONTOUR_INTERVAL: f64 = 12.0;
/// How near a click must be to a device symbol to pick it, inches (at least).
const DEVICE_PICK_MIN: f64 = 5.0;
const RECORD_TEXT_HEIGHT: f64 = 0.1;

// ----- record storage -----

fn hash_str(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn record_text<'a>(floor: &'a Floor, layer: &str) -> Option<&'a str> {
    floor.cad.iter().find_map(|c| match &c.item {
        CadItem::Text { text, .. } if c.layer == layer => Some(text.as_str()),
        _ => None,
    })
}

/// Adds the reserved record layer (hidden and locked) if the project lacks it.
fn ensure_data_layer(project: &mut Project, name: &str) {
    if project.layers.get(name).is_none() {
        let mut l = Layer::new(name, [128, 128, 128], 13);
        l.display = false;
        l.locked = true;
        project.layers.add(l);
    }
}

fn write_record(project: &mut Project, floor: usize, layer: &str, text: String) {
    ensure_data_layer(project, layer);
    let existing = project.floors[floor]
        .cad
        .iter_mut()
        .find(|c| c.layer == layer && matches!(c.item, CadItem::Text { .. }));
    match existing {
        Some(c) => {
            if let CadItem::Text { text: t, .. } = &mut c.item {
                *t = text;
            }
        }
        None => {
            project.add_cad(
                floor,
                layer,
                CadItem::Text {
                    pos: Point::ZERO,
                    text,
                    height: RECORD_TEXT_HEIGHT,
                    angle: 0.0,
                },
            );
        }
    }
}

/// The electrical layer of `floor` (empty when it has no record).
pub fn load_electrical(floor: &Floor) -> ElectricalLayer {
    record_text(floor, ELECTRICAL_DATA_LAYER)
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or_default()
}

/// Stores `layer` as the electrical record of `floor`. An empty layer with no
/// existing record writes nothing.
pub fn save_electrical(project: &mut Project, floor: usize, layer: &ElectricalLayer) {
    let has_record = record_text(&project.floors[floor], ELECTRICAL_DATA_LAYER).is_some();
    if !has_record && layer.devices.is_empty() && layer.connections.is_empty() {
        return;
    }
    if let Ok(text) = serde_json::to_string(layer) {
        write_record(project, floor, ELECTRICAL_DATA_LAYER, text);
    }
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
        }
    }

    pub fn has_perimeter(&self) -> bool {
        self.terrain.perimeter.len() >= 3
    }

    fn to_json(&self) -> Option<String> {
        let terrain = serde_json::to_value(&self.terrain).ok()?;
        Some(
            json!({
                "terrain": terrain,
                "contour_interval": self.contour_interval,
                "built": self.built,
                "layer": self.layer,
            })
            .to_string(),
        )
    }

    fn from_json(s: &str) -> Option<Self> {
        let v: Value = serde_json::from_str(s).ok()?;
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
        })
    }
}

impl Default for TerrainRecord {
    fn default() -> Self {
        Self::new()
    }
}

fn terrain_record_text(project: &Project) -> Option<(usize, &str)> {
    project
        .floors
        .iter()
        .enumerate()
        .find_map(|(i, f)| record_text(f, TERRAIN_DATA_LAYER).map(|t| (i, t)))
}

/// The project's terrain record, if one was saved.
pub fn load_terrain(project: &Project) -> Option<TerrainRecord> {
    terrain_record_text(project).and_then(|(_, t)| TerrainRecord::from_json(t))
}

/// Stores the terrain record (on the floor that already holds one, else floor 0).
pub fn save_terrain(project: &mut Project, rec: &TerrainRecord) {
    let floor = terrain_record_text(project).map_or(0, |(i, _)| i);
    if let Some(text) = rec.to_json() {
        write_record(project, floor, TERRAIN_DATA_LAYER, text);
    }
}

// ----- caches -----

type ElecCache = Option<(usize, u64, Rc<ElectricalLayer>)>;
type TerrainCache = Option<(u64, Rc<TerrainView>)>;

thread_local! {
    static ELEC_CACHE: RefCell<ElecCache> = const { RefCell::new(None) };
    static TERRAIN_CACHE: RefCell<TerrainCache> = const { RefCell::new(None) };
}

/// The floor's electrical layer, parsed once per change of the record.
pub fn electrical_layer(floor_index: usize, floor: &Floor) -> Rc<ElectricalLayer> {
    let Some(text) = record_text(floor, ELECTRICAL_DATA_LAYER) else {
        return Rc::new(ElectricalLayer::default());
    };
    let h = hash_str(text);
    ELEC_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((i, hh, layer)) = c.as_ref() {
            if *i == floor_index && *hh == h {
                return layer.clone();
            }
        }
        let layer: Rc<ElectricalLayer> = Rc::new(serde_json::from_str(text).unwrap_or_default());
        *c = Some((floor_index, h, layer.clone()));
        layer
    })
}

/// The terrain plus what was derived from it (surface and contours once built).
pub struct TerrainView {
    pub record: TerrainRecord,
    pub surface: Option<TerrainSurface>,
    pub contours: Vec<Contour>,
    pub symbols: Vec<TerrainStroke>,
}

impl TerrainView {
    fn new(record: TerrainRecord) -> Self {
        let (surface, cont) = if record.built && record.has_perimeter() {
            let s = build_terrain(&record.terrain);
            let c = contours(&s, record.contour_interval);
            (Some(s), c)
        } else {
            (None, Vec::new())
        };
        let symbols = plan_symbols(&record.terrain, &cont);
        Self {
            record,
            surface,
            contours: cont,
            symbols,
        }
    }
}

/// The built view of the project's terrain (cached by record text).
pub fn terrain_view(project: &Project) -> Option<Rc<TerrainView>> {
    let (_, text) = terrain_record_text(project)?;
    let h = hash_str(text);
    TERRAIN_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((hh, v)) = c.as_ref() {
            if *hh == h {
                return Some(v.clone());
            }
        }
        let view = Rc::new(TerrainView::new(TerrainRecord::from_json(text)?));
        *c = Some((h, view.clone()));
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

// ----- terrain hit-testing -----

/// A terrain element under the pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainHit {
    Perimeter,
    Point(usize),
    Line(usize),
    Region(usize),
    Modifier(usize),
    Feature(usize),
    Road(usize),
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
    if !cx.layers().is_visible(&view.record.layer) {
        return;
    }
    let px = |w: f64, min: f32| ((w as f32) * 1.6).max(min);
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
            TerrainStroke::Text { at, text, height } => {
                draw_label(painter, cam, *at, text, *height, MAJOR_CONTOUR_COLOR);
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
        if !d.label.is_empty() {
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
    fn electrical_record_round_trips_and_hides_its_layer() {
        let mut cx = cx();
        let mut layer = ElectricalLayer::default();
        let id = layer.add(place_free(DeviceKind::CeilingLight, Point::new(10.0, 20.0)));
        save_electrical(&mut cx.project, 0, &layer);
        let back = load_electrical(cx.floor());
        assert_eq!(back, layer);
        assert_eq!(back.device(id).unwrap().position, Point::new(10.0, 20.0));
        assert!(!cx.layers().is_visible(ELECTRICAL_DATA_LAYER));
        assert!(cx.layers().is_locked(ELECTRICAL_DATA_LAYER));
        // The record survives a project JSON round trip.
        let json = cx.project.to_json().unwrap();
        let project = Project::from_json(&json).unwrap();
        assert_eq!(load_electrical(&project.floors[0]), layer);
        // Saving again replaces the record instead of adding another.
        save_electrical(&mut cx.project, 0, &layer);
        assert_eq!(cx.floor().cad.len(), 1);
    }

    #[test]
    fn an_empty_layer_without_a_record_writes_nothing() {
        let mut cx = cx();
        save_electrical(&mut cx.project, 0, &ElectricalLayer::default());
        assert!(cx.floor().cad.is_empty());
    }

    #[test]
    fn terrain_record_round_trips() {
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
        let project = Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert_eq!(load_terrain(&project), Some(rec));
        assert_eq!(cx.floor().cad.len(), 1);
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
