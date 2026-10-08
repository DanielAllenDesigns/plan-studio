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
//! * Build Terrain: one click builds the surface and shows the contours (CB-53).
//!
//! The typed value goes through `cx.temp.editing` (the shell forwards typed
//! text while it is set); Enter accepts, an empty field takes the default
//! shown, Esc cancels. Delete removes the element under the pointer; a
//! double-click outside a drawing opens the Terrain Specification.
//!
//! The terrain lives in `floor.cad` through `editor::site_view` (TODO:
//! plan-core field); `site_view::draw_site` draws it.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::terrain::TerrainDialog;
use crate::dialogs::Outcome;
use crate::editor::site_view::{
    draw_polyline, edit_terrain, hit_terrain, load_terrain, remove_terrain_element, terrain_view,
    TerrainRecord,
};
use crate::editor::tempdim::EditField;
use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Color32, FontId, Key, Pos2, Rect};
use plan_core::geometry::Point;
use plan_core::units::{fmt_ft_in, parse_ft_in};
use plan_terrain::{
    ElevationLine, ElevationPoint, ElevationRegion, Feature, FeatureKind, Modifier, ModifierKind,
    RoadKind, RoadStrip,
};
use std::cell::RefCell;

/// Default Hill/Valley height, inches.
const DEFAULT_HILL: f64 = 72.0;
/// Default Raised/Lowered Region height, inches.
const DEFAULT_REGION_SHIFT: f64 = 24.0;
const DEFAULT_ROAD_WIDTH: f64 = 240.0;
const DEFAULT_DRIVEWAY_WIDTH: f64 = 144.0;
const DEFAULT_SIDEWALK_WIDTH: f64 = 48.0;
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
}

/// What the clicks of a flavor draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Draw {
    Spot,
    Polyline,
    Polygon,
    Command,
}

impl TerrainVariant {
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
        }
    }

    fn draw(self) -> Draw {
        match self {
            TerrainVariant::ElevationPoint => Draw::Spot,
            TerrainVariant::ElevationLine
            | TerrainVariant::Road
            | TerrainVariant::Driveway
            | TerrainVariant::Sidewalk => Draw::Polyline,
            TerrainVariant::Build => Draw::Command,
            _ => Draw::Polygon,
        }
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
            TerrainVariant::Road => Some(RoadKind::Road),
            TerrainVariant::Driveway => Some(RoadKind::Driveway),
            TerrainVariant::Sidewalk => Some(RoadKind::Sidewalk),
            _ => None,
        }
    }

    /// Fewest points a finished shape needs.
    fn min_points(self) -> usize {
        match self.draw() {
            Draw::Polygon => 3,
            Draw::Polyline => 2,
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
        }
    }
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
        self.dialog.borrow().is_some()
    }

    fn flush(&mut self, cx: &mut EditorContext) -> Option<ToolResult> {
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

    /// Ends the shape: closes a perimeter, adds a hole or flat region, or asks
    /// for the value the shape needs.
    fn finish(&mut self, cx: &mut EditorContext) -> ToolResult {
        let v = self.variant;
        if self.points.len() < v.min_points() {
            cx.status = format!("{} needs at least {} points", v.name(), v.min_points());
            return ToolResult::consumed();
        }
        let pts = std::mem::take(&mut self.points);
        let at = pts[pts.len() - 1];
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
                        material: String::new(),
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
            TerrainVariant::ElevationLine => {
                self.begin_entry(cx, Pending::Line(pts), at);
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
                Draw::Polygon => {
                    format!("{}: click the corners, Enter closes the shape", v.name())
                }
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
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        self.hover = Some(if self.points.is_empty() {
            p.snapped
        } else {
            self.snapped(cx, &p)
        });
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
            Draw::Polyline | Draw::Polygon => {
                if self.points.is_empty()
                    && v == TerrainVariant::Perimeter
                    && load_terrain(&cx.project).is_some_and(|r| r.has_perimeter())
                {
                    cx.status =
                        "The plan already has a terrain perimeter (Delete removes it)".into();
                    return ToolResult::consumed();
                }
                let pt = if self.points.is_empty() {
                    p.snapped
                } else {
                    self.snapped(cx, &p)
                };
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
                ToolResult::consumed()
            }
        }
    }

    fn frame(&mut self, cx: &mut EditorContext, _ctx: &egui::Context) {
        // An OK in the Terrain Specification applies right away.
        let _ = self.flush(cx);
    }

    fn double_click(&mut self, cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        if self.entry.is_some() {
            return ToolResult::consumed();
        }
        if !self.points.is_empty() {
            return self.finish(cx);
        }
        let rec = load_terrain(&cx.project).unwrap_or_default();
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
        let closed = self.variant.draw() == Draw::Polygon && self.entry.is_some();
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
        // One record only, hidden from the plan.
        assert_eq!(project.floors[0].cad.len(), 1);
        assert!(!project
            .layers
            .is_visible(crate::editor::site_view::TERRAIN_DATA_LAYER));
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
}
