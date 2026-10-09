//! Tray and coffered ceilings (Chief "Tray Ceiling Polyline", manual pp.
//! 457-460; R-111, R-112).
//!
//! # Model
//!
//! A tray ceiling is a closed CAD polyline (a [`CadItem::Polyline`] on the
//! "Ceiling Planes" layer, so it selects, moves, stretches, copies and
//! deletes like any CAD object) plus a [`TrayCeiling`] record kept in
//! [`Floor::trays`], keyed by the polyline's id. A record whose polyline is
//! gone is ignored and pruned ([`Floor::prune_trays`]).
//!
//! The polyline is the edge of the *hole* of the tray: the inner ceiling. The
//! outer ceiling is a ring of `width` inches around it. With the default
//! (Recess into Ceiling off) the inner ceiling is at the ceiling height of
//! the surface the tray sits in, `base`, and the ring is dropped by `depth`;
//! with Recess into Ceiling on the ring stays at `base` and the inner ceiling
//! is raised by `depth` into the ceiling platform. Vertical sides (Pitch
//! "Vertical") step straight; a pitch gives a sloped band of ceiling plane
//! along the inner edge with a horizontal run of `depth * 12 / pitch`.
//!
//! Tray ceilings nest: a polyline wholly inside another tray's inner ceiling
//! sits in that ceiling (its `base` is the parent's inner height). A coffered
//! ceiling is a grid of such polylines ([`coffer_cells`]).
//!
//! [`resolve`] turns the records into [`TrayGeom`]s with their heights worked
//! out and, for a tray that cannot generate, a [`Caution`] (the symbol Chief
//! draws inside it). Heights are inches above the floor datum of the floor
//! the tray is on (the room's floor offset included); the 3D and framing
//! layers add the floor's elevation.

use crate::cad::{CadItem, CadObject};
use crate::extras::StructureLayer;
use crate::foundation::offset_ring;
use crate::geometry::{
    dist_to_segment, point_in_polygon, polygon_area, segment_intersection, Point,
};
use crate::layers::{Layer, LineStyle};
use crate::model::{Floor, Id, Project};
use crate::rooms::polygon_inside;
use serde::{Deserialize, Serialize};

/// The layer tray ceiling polylines are drawn on.
pub const LAYER: &str = "Ceiling Planes";
/// Width of the outer ceiling a new tray starts with, inches.
pub const DEFAULT_WIDTH: f64 = 24.0;
/// Depth of the step a new tray starts with, inches.
pub const DEFAULT_DEPTH: f64 = 8.0;
/// On-center spacing of the tray's ceiling framing, inches.
pub const DEFAULT_RAFTER_SPACING: f64 = 16.0;
/// Tolerance for "inside" tests between a tray and its host, inches.
const TOL: f64 = 1.0;
/// A polyline needs this much plan area (square inches) to be a tray.
const MIN_AREA: f64 = 36.0;

// ---------------------------------------------------------------------------
// The record
// ---------------------------------------------------------------------------

/// A molding run along the inside edge of the step (Moldings panel; only for
/// vertical sides).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrayMolding {
    /// Profile name from the molding library.
    pub profile: String,
    /// Molding height, inches.
    pub height: f64,
    /// Horizontal offset from the polyline, positive away from the hole.
    pub h_offset: f64,
    /// Vertical offset from the top of the step, positive up.
    pub v_offset: f64,
}

impl Default for TrayMolding {
    fn default() -> Self {
        Self {
            profile: "Crown - Colonial 4 5/8".into(),
            height: 4.625,
            h_offset: 0.0,
            v_offset: 0.0,
        }
    }
}

/// A rope light run along the step (Rope Lights panel). Nothing but the two
/// offsets is set here; the Rope Light Specification holds the rest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RopeLight {
    /// Name of the rope light type.
    pub name: String,
    /// Horizontal offset from the polyline, positive away from the hole.
    pub h_offset: f64,
    /// Vertical offset above the lower ceiling, inches.
    pub v_offset: f64,
}

impl Default for RopeLight {
    fn default() -> Self {
        Self {
            name: "Rope Light".into(),
            h_offset: 1.0,
            v_offset: 2.0,
        }
    }
}

/// A line of the Components panel (Materials List information).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrayComponent {
    pub name: String,
    pub quantity: f64,
    pub unit: String,
}

impl Default for TrayComponent {
    fn default() -> Self {
        Self {
            name: String::new(),
            quantity: 1.0,
            unit: "ea".into(),
        }
    }
}

/// Materials panel: empty names follow the room's ceiling finish.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TrayMaterials {
    pub sides: String,
    pub top: String,
    pub ring: String,
}

/// The specification of one tray ceiling polyline (Tray Ceiling
/// Specification, manual pp. 458-460).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrayCeiling {
    /// Id of the CAD polyline that is the hole of the tray.
    pub id: Id,
    /// Width of the outer ceiling from its outside edge to the hole, inches.
    pub width: f64,
    /// Vertical distance from the lower ceiling to the upper one, inches.
    pub depth: f64,
    /// Recess into Ceiling: the ring stays at the room's ceiling and the
    /// inner ceiling is raised into the platform.
    pub recess: bool,
    /// Pitch of the sides as rise in 12, or `None` for Vertical.
    pub pitch: Option<f64>,
    /// Show the pitch in degrees.
    pub pitch_in_degrees: bool,
    /// Ceiling layers (structure) of the tray's ceiling planes.
    pub structure: Vec<StructureLayer>,
    pub use_room_material_sides: bool,
    pub use_room_material_top: bool,
    /// Retain Framing: framing is not rebuilt.
    pub retain_framing: bool,
    /// On-center spacing of the tray's ceiling framing, inches.
    pub rafter_spacing: f64,
    pub moldings: Vec<TrayMolding>,
    pub rope_lights: Vec<RopeLight>,
    pub materials: TrayMaterials,
    /// Custom label; the automatic label is blank.
    pub label: String,
    pub components: Vec<TrayComponent>,
}

impl Default for TrayCeiling {
    fn default() -> Self {
        Self {
            id: 0,
            width: DEFAULT_WIDTH,
            depth: DEFAULT_DEPTH,
            recess: false,
            pitch: None,
            pitch_in_degrees: false,
            structure: vec![StructureLayer::new("Drywall", 0.625)],
            use_room_material_sides: true,
            use_room_material_top: true,
            retain_framing: false,
            rafter_spacing: DEFAULT_RAFTER_SPACING,
            moldings: Vec::new(),
            rope_lights: Vec::new(),
            materials: TrayMaterials::default(),
            label: String::new(),
            components: Vec::new(),
        }
    }
}

impl TrayCeiling {
    /// Vertical sides (no side ceiling planes).
    pub fn is_vertical(&self) -> bool {
        self.pitch.is_none_or(|p| p <= 1e-6)
    }

    /// Total structure thickness, inches.
    pub fn structure_thickness(&self) -> f64 {
        crate::extras::structure_thickness(&self.structure)
    }

    /// Horizontal run of the sloped side, inches (zero when vertical).
    pub fn run(&self) -> f64 {
        match self.pitch {
            Some(p) if p > 1e-6 => self.depth.max(0.0) * 12.0 / p,
            _ => 0.0,
        }
    }

    /// The pitch as the dialog shows it: rise in 12, or degrees.
    pub fn pitch_display(&self) -> Option<f64> {
        self.pitch.map(|p| {
            if self.pitch_in_degrees {
                (p / 12.0).atan().to_degrees()
            } else {
                p
            }
        })
    }

    /// Sets the pitch from a dialog value (degrees or rise in 12). Values
    /// between -89 and 89 degrees are taken; zero or less means Vertical
    /// here (a flat side has no step).
    pub fn set_pitch_display(&mut self, v: f64) {
        let rise = if self.pitch_in_degrees {
            12.0 * v.clamp(-89.0, 89.0).to_radians().tan()
        } else {
            v
        };
        self.pitch = (rise > 1e-6).then_some(rise);
    }
}

/// The tray ceiling records of a floor (the "trays" slot).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TrayLayer {
    pub trays: Vec<TrayCeiling>,
}

impl TrayLayer {
    pub fn is_empty(&self) -> bool {
        self.trays.is_empty()
    }

    pub fn get(&self, id: Id) -> Option<&TrayCeiling> {
        self.trays.iter().find(|t| t.id == id)
    }

    /// Adds or replaces the record with `t.id`.
    pub fn set(&mut self, t: TrayCeiling) {
        match self.trays.iter().position(|x| x.id == t.id) {
            Some(i) => self.trays[i] = t,
            None => self.trays.push(t),
        }
    }

    pub fn remove(&mut self, id: Id) -> Option<TrayCeiling> {
        let i = self.trays.iter().position(|x| x.id == id)?;
        Some(self.trays.remove(i))
    }
}

// ---------------------------------------------------------------------------
// Floor and Project accessors
// ---------------------------------------------------------------------------

impl Floor {
    /// The closed outline of the CAD polyline `id`, counter-clockwise.
    fn polyline_outline(&self, id: Id) -> Option<Vec<Point>> {
        self.cad.iter().find_map(|c| match &c.item {
            CadItem::Polyline {
                points,
                closed: true,
            } if c.id == id && points.len() >= 3 => Some(ccw(points)),
            _ => None,
        })
    }

    /// The tray ceiling `id` when its polyline still exists.
    pub fn tray(&self, id: Id) -> Option<&TrayCeiling> {
        self.trays
            .get(id)
            .filter(|_| self.polyline_outline(id).is_some())
    }

    /// Is the CAD object `id` a tray ceiling polyline?
    pub fn is_tray(&self, id: Id) -> bool {
        self.tray(id).is_some()
    }

    /// The hole outline of tray `id`, counter-clockwise.
    pub fn tray_outline(&self, id: Id) -> Option<Vec<Point>> {
        self.tray(id)?;
        self.polyline_outline(id)
    }

    /// Ids of the tray ceilings that still have a polyline, in record order.
    pub fn tray_ids(&self) -> Vec<Id> {
        self.trays
            .trays
            .iter()
            .map(|t| t.id)
            .filter(|id| self.polyline_outline(*id).is_some())
            .collect()
    }

    /// Forgets the records whose polyline is gone. Returns how many.
    pub fn prune_trays(&mut self) -> usize {
        let before = self.trays.trays.len();
        let ids: Vec<Id> = self.trays.trays.iter().map(|t| t.id).collect();
        let gone: Vec<Id> = ids
            .into_iter()
            .filter(|id| self.polyline_outline(*id).is_none())
            .collect();
        self.trays.trays.retain(|t| !gone.contains(&t.id));
        before - self.trays.trays.len()
    }
}

impl Project {
    /// Makes sure the tray layer exists; true when it was added.
    pub fn ensure_tray_layer(&mut self) -> bool {
        if self.layers.get(LAYER).is_some() {
            return false;
        }
        let mut layer = Layer::new(LAYER, [150, 60, 150], 18);
        layer.line_style = LineStyle::Dashed;
        self.layers.add(layer);
        true
    }

    /// Adds a tray ceiling whose hole is `outline`. Returns the polyline id,
    /// or `None` for an unusable outline (fewer than three points, a tiny or
    /// self-crossing polygon) or a bad floor.
    pub fn add_tray(
        &mut self,
        floor: usize,
        outline: &[Point],
        mut spec: TrayCeiling,
    ) -> Option<Id> {
        if floor >= self.floors.len() || !usable_outline(outline) {
            return None;
        }
        self.ensure_tray_layer();
        let id = self.alloc_id();
        spec.id = id;
        let f = &mut self.floors[floor];
        f.cad.push(CadObject {
            id,
            layer: LAYER.to_string(),
            item: CadItem::Polyline {
                points: ccw(outline),
                closed: true,
            },
        });
        f.trays.set(spec);
        Some(id)
    }

    /// Make Tray Ceiling in Room: a tray whose hole follows `room_outline`
    /// (the room's interior outline) inset by `spec.width`, so the outer
    /// ceiling is the ring of that width along the walls.
    pub fn make_tray_in_room(
        &mut self,
        floor: usize,
        room_outline: &[Point],
        spec: TrayCeiling,
    ) -> Option<Id> {
        let hole = inset_outline(room_outline, spec.width)?;
        self.add_tray(floor, &hole, spec)
    }

    /// Make Nested Tray Ceiling: a new, smaller tray following the perimeter
    /// of tray `parent`, inset by `spec.width`.
    pub fn make_nested_tray(&mut self, floor: usize, parent: Id, spec: TrayCeiling) -> Option<Id> {
        let outline = self.floors.get(floor)?.tray_outline(parent)?;
        let hole = inset_outline(&outline, spec.width)?;
        self.add_tray(floor, &hole, spec)
    }

    /// Converts the closed CAD polyline `id` into a tray ceiling (Convert
    /// Polyline). False when it is not a usable closed polyline or already a
    /// tray.
    pub fn convert_polyline_to_tray(&mut self, floor: usize, id: Id, spec: TrayCeiling) -> bool {
        self.ensure_tray_layer();
        let Some(f) = self.floors.get_mut(floor) else {
            return false;
        };
        if f.trays.get(id).is_some() {
            return false;
        }
        let Some(obj) = f.cad.iter_mut().find(|c| c.id == id) else {
            return false;
        };
        let CadItem::Polyline {
            points,
            closed: true,
        } = &mut obj.item
        else {
            return false;
        };
        if !usable_outline(points) {
            return false;
        }
        *points = ccw(points);
        obj.layer = LAYER.to_string();
        let mut spec = spec;
        spec.id = id;
        f.trays.set(spec);
        true
    }

    /// Make Coffered Ceiling: a `cols` by `rows` grid of tray polylines in
    /// `region` (see [`coffer_cells`]), each recessed into the ceiling with
    /// `spec`'s depth. Returns the ids made (empty when the grid does not fit).
    pub fn make_coffered_ceiling(
        &mut self,
        floor: usize,
        region: &[Point],
        cols: usize,
        rows: usize,
        beam: f64,
        spec: &TrayCeiling,
    ) -> Vec<Id> {
        let mut out = Vec::new();
        for cell in coffer_cells(region, cols, rows, beam) {
            let mut s = spec.clone();
            s.recess = true;
            // The ring a coffer needs is half a beam on each side.
            s.width = (beam / 2.0).max(0.5);
            if let Some(id) = self.add_tray(floor, &cell, s) {
                out.push(id);
            }
        }
        out
    }
}

impl Project {
    /// Sets Flat Ceiling Over This Room of the room that holds `anchor`
    /// (Turn Off Ceiling / Turn On Ceiling): off makes a cathedral ceiling
    /// that follows the roof above, on a flat one. A room without a name
    /// entry gets an unnamed one at `anchor`. Returns false when `floor` is
    /// out of range.
    pub fn set_flat_ceiling(
        &mut self,
        floor: usize,
        anchor: Point,
        flat: bool,
        rooms: &[crate::rooms::Room],
    ) -> bool {
        if floor >= self.floors.len() {
            return false;
        }
        let (name, ty) = rooms
            .iter()
            .find(|r| r.contains(anchor))
            .and_then(|r| r.name_entry(&self.floors[floor].room_names))
            .map(|n| (n.name.clone(), n.room_type.clone()))
            .unwrap_or_default();
        self.set_room_name(floor, anchor, name, ty, rooms);
        let near = |n: &crate::model::RoomName| n.anchor == anchor;
        if let Some(n) = self.floors[floor].room_names.iter_mut().find(|n| near(n)) {
            n.flat_ceiling = flat;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Polygon helpers
// ---------------------------------------------------------------------------

/// `pts` counter-clockwise.
pub fn ccw(pts: &[Point]) -> Vec<Point> {
    let mut v = pts.to_vec();
    if polygon_area(&v) < 0.0 {
        v.reverse();
    }
    v
}

/// Does the polygon cross itself? (Edges that merely share an end do not.)
pub fn self_intersects(pts: &[Point]) -> bool {
    let n = pts.len();
    if n < 4 {
        return false;
    }
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        for j in (i + 1)..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (c, d) = (pts[j], pts[(j + 1) % n]);
            if let Some((t, u)) = segment_intersection(a, b, c, d) {
                // A touch at an end counts as a crossing too: a figure-eight
                // pinched at a vertex is not a simple polygon.
                let _ = (t, u);
                return true;
            }
        }
    }
    false
}

fn usable_outline(pts: &[Point]) -> bool {
    pts.len() >= 3 && polygon_area(pts).abs() >= MIN_AREA && !self_intersects(pts)
}

/// `outline` moved `d` inches inward; `None` when nothing is left.
pub fn inset_outline(outline: &[Point], d: f64) -> Option<Vec<Point>> {
    if outline.len() < 3 {
        return None;
    }
    let ring = ccw(outline);
    let out = offset_ring(&ring, d.max(0.0));
    // Every edge keeps its direction: an inset past the half width flips them.
    let n = ring.len();
    let kept = (0..n).all(|i| {
        let j = (i + 1) % n;
        (out[j] - out[i]).dot(ring[j] - ring[i]) > 0.0
    });
    let shrunk =
        kept && polygon_area(&out) > MIN_AREA && polygon_area(&out) < polygon_area(&ring) + 1e-6;
    (shrunk && !self_intersects(&out)).then_some(out)
}

/// `outline` moved `d` inches outward.
pub fn outset_outline(outline: &[Point], d: f64) -> Vec<Point> {
    offset_ring(&ccw(outline), -d.max(0.0))
}

/// Distance from `p` to the edges of `poly`.
pub fn dist_to_outline(p: Point, poly: &[Point]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]))
        .fold(f64::MAX, f64::min)
}

/// The cells of a coffered ceiling: a `cols` by `rows` grid of rectangles in
/// the bounding box of `region`, separated from each other and from the
/// region's edge by `beam` inches. Cells that would stick out of `region` are
/// left out. Row-major from the lower-left, counter-clockwise.
pub fn coffer_cells(region: &[Point], cols: usize, rows: usize, beam: f64) -> Vec<Vec<Point>> {
    if region.len() < 3 || cols == 0 || rows == 0 || beam < 0.0 {
        return Vec::new();
    }
    let (lo, hi) = crate::foundation::bounds(region);
    let cw = ((hi.x - lo.x) - beam * (cols as f64 + 1.0)) / cols as f64;
    let ch = ((hi.y - lo.y) - beam * (rows as f64 + 1.0)) / rows as f64;
    if cw < 1.0 || ch < 1.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let x0 = lo.x + beam + c as f64 * (cw + beam);
            let y0 = lo.y + beam + r as f64 * (ch + beam);
            let cell = vec![
                Point::new(x0, y0),
                Point::new(x0 + cw, y0),
                Point::new(x0 + cw, y0 + ch),
                Point::new(x0, y0 + ch),
            ];
            if polygon_inside(&cell, region, TOL) {
                out.push(cell);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Resolving
// ---------------------------------------------------------------------------

/// A room as the trays see it: its interior outline, its ceiling height above
/// the floor datum and whether its ceiling is flat and built.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomCeiling {
    pub outline: Vec<Point>,
    /// Finished ceiling height above the floor datum, inches.
    pub height: f64,
    /// Flat Ceiling Over This Room (and a ceiling over it at all).
    pub flat: bool,
}

/// Why a tray ceiling cannot generate (the Caution symbol's tool tip).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caution {
    /// The polyline is not a usable closed shape.
    BadShape,
    /// It does not lie inside a single room.
    OutsideRoom,
    /// Its room has no flat ceiling (a cathedral ceiling or none).
    RoomNotFlat,
    /// The depth is zero or less.
    NoDepth,
    /// The sloped side is wider than the outer ceiling.
    SlopeTooWide,
}

impl Caution {
    /// The tool tip text.
    pub fn text(self) -> &'static str {
        match self {
            Caution::BadShape => "The tray ceiling polyline must be a closed shape that does not cross itself.",
            Caution::OutsideRoom => "The tray ceiling polyline must lie inside a room.",
            Caution::RoomNotFlat => "A tray ceiling needs a room with a flat ceiling: turn on Flat Ceiling Over This Room.",
            Caution::NoDepth => "The tray ceiling needs a Depth greater than zero.",
            Caution::SlopeTooWide => "The sloped sides are wider than the outer ceiling: increase the Width or the Pitch.",
        }
    }
}

/// One tray ceiling worked out: outlines and heights.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayGeom {
    pub id: Id,
    /// The hole (the polyline), counter-clockwise.
    pub inner: Vec<Point>,
    /// The outside edge of the outer ceiling, counter-clockwise.
    pub outer: Vec<Point>,
    /// The tray this one is nested in.
    pub parent: Option<Id>,
    /// 0 for a tray sitting in the room's own ceiling.
    pub level: usize,
    /// Height of the ceiling the tray sits in, above the floor datum.
    pub base: f64,
    /// Height of the outer ceiling (the ring).
    pub h_outer: f64,
    /// Height of the inner ceiling (the hole).
    pub h_inner: f64,
    /// Horizontal run of the sloped side (zero for vertical sides).
    pub run: f64,
    /// Why it cannot generate, if it cannot.
    pub caution: Option<Caution>,
}

impl TrayGeom {
    /// The tray generates (no Caution).
    pub fn ok(&self) -> bool {
        self.caution.is_none()
    }

    /// Lower of the two ceilings.
    pub fn low(&self) -> f64 {
        self.h_outer.min(self.h_inner)
    }

    /// Upper of the two ceilings.
    pub fn high(&self) -> f64 {
        self.h_outer.max(self.h_inner)
    }

    /// Perimeter of the hole, inches.
    pub fn perimeter(&self) -> f64 {
        let n = self.inner.len();
        (0..n)
            .map(|i| self.inner[i].dist(self.inner[(i + 1) % n]))
            .sum()
    }

    /// Ceiling height this tray gives plan point `p`, or `None` when `p` is
    /// outside its outer ceiling. Heights of nested trays are not looked at;
    /// see [`ceiling_height_at`].
    pub fn height_at(&self, p: Point) -> Option<f64> {
        if !point_in_polygon(p, &self.outer) {
            return None;
        }
        if point_in_polygon(p, &self.inner) {
            return Some(self.h_inner);
        }
        if self.run > 1e-6 {
            let d = dist_to_outline(p, &self.inner);
            if d < self.run {
                return Some(self.h_inner + (self.h_outer - self.h_inner) * (d / self.run));
            }
        }
        Some(self.h_outer)
    }
}

/// The trays of `floor` worked out against `rooms`. A tray's host is the
/// room that holds its whole hole; trays nest by containment, the smallest
/// bigger tray whose hole holds the tray's hole being the parent. Results are
/// in record order.
pub fn resolve(floor: &Floor, rooms: &[RoomCeiling]) -> Vec<TrayGeom> {
    struct Item {
        id: Id,
        inner: Vec<Point>,
        rec: TrayCeiling,
    }
    let items: Vec<Item> = floor
        .trays
        .trays
        .iter()
        .filter_map(|t| {
            Some(Item {
                id: t.id,
                inner: floor.polyline_outline(t.id)?,
                rec: t.clone(),
            })
        })
        .collect();
    // Parents first: bigger holes come before smaller ones.
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| {
        polygon_area(&items[b].inner)
            .abs()
            .total_cmp(&polygon_area(&items[a].inner).abs())
    });
    let mut done: Vec<Option<TrayGeom>> = vec![None; items.len()];
    for &k in &order {
        let it = &items[k];
        let area = polygon_area(&it.inner).abs();
        // The smallest earlier tray whose hole holds this one.
        let parent = order
            .iter()
            .take_while(|&&j| j != k)
            .filter_map(|&j| done[j].as_ref())
            .filter(|g| {
                polygon_area(&g.inner).abs() > area + 1.0
                    && polygon_inside(&it.inner, &g.inner, TOL)
            })
            .min_by(|a, b| {
                polygon_area(&a.inner)
                    .abs()
                    .total_cmp(&polygon_area(&b.inner).abs())
            });
        let host = rooms
            .iter()
            .filter(|r| polygon_inside(&it.inner, &r.outline, TOL))
            .min_by(|a, b| {
                polygon_area(&a.outline)
                    .abs()
                    .total_cmp(&polygon_area(&b.outline).abs())
            });
        let rec = &it.rec;
        let base = parent
            .map(|g| g.h_inner)
            .or_else(|| host.map(|r| r.height))
            .unwrap_or(0.0);
        let depth = rec.depth.max(0.0);
        let (h_outer, h_inner) = if rec.recess {
            (base, base + depth)
        } else {
            (base - depth, base)
        };
        let run = rec.run();
        let caution = if !usable_outline(&it.inner) {
            Some(Caution::BadShape)
        } else if host.is_none() {
            Some(Caution::OutsideRoom)
        } else if host.is_some_and(|r| !r.flat) {
            Some(Caution::RoomNotFlat)
        } else if rec.depth <= 1e-6 {
            Some(Caution::NoDepth)
        } else if run > rec.width + 1e-6 {
            Some(Caution::SlopeTooWide)
        } else {
            None
        };
        done[k] = Some(TrayGeom {
            id: it.id,
            outer: outset_outline(&it.inner, rec.width),
            inner: it.inner.clone(),
            parent: parent.map(|g| g.id),
            level: parent.map_or(0, |g| g.level + 1),
            base,
            h_outer,
            h_inner,
            run,
            caution,
        });
    }
    done.into_iter().flatten().collect()
}

/// The finished ceiling height at plan point `p`: the deepest tray ceiling
/// that holds `p` (trays that cannot generate are skipped), else `room_height`.
pub fn ceiling_height_at(trays: &[TrayGeom], p: Point, room_height: f64) -> f64 {
    trays
        .iter()
        .filter(|g| g.ok())
        .filter_map(|g| g.height_at(p).map(|h| (g, h)))
        .max_by(|(a, _), (b, _)| {
            a.level.cmp(&b.level).then(
                polygon_area(&b.outer)
                    .abs()
                    .total_cmp(&polygon_area(&a.outer).abs()),
            )
        })
        .map_or(room_height, |(_, h)| h)
}

/// The rooms of `floor` (their interior outlines) as trays see them. A room
/// without a name entry has the floor's ceiling height and a flat ceiling.
pub fn room_ceilings(floor: &Floor, rooms: &[crate::rooms::Room]) -> Vec<RoomCeiling> {
    rooms
        .iter()
        .map(|r| {
            let named = r.name_entry(&floor.room_names);
            RoomCeiling {
                outline: if r.inner_polygon.len() >= 3 {
                    r.inner_polygon.clone()
                } else {
                    r.polygon.clone()
                },
                height: named
                    .and_then(|n| n.ceiling_height)
                    .unwrap_or(floor.ceiling_height)
                    + named.map_or(0.0, |n| n.floor_height_offset),
                flat: named.is_none_or(room_flat),
            }
        })
        .collect()
}

/// Does the named room have a flat ceiling that is built (Flat Ceiling Over
/// This Room on and a ceiling over the room)?
pub fn room_flat(name: &crate::model::RoomName) -> bool {
    name.has_ceiling && name.flat_ceiling
}

// ---------------------------------------------------------------------------
// Moldings, rope lights, materials
// ---------------------------------------------------------------------------

/// A rope light run in the plan, ready for the electrical layer to light.
#[derive(Debug, Clone, PartialEq)]
pub struct RopePath {
    /// The tray the run belongs to.
    pub tray: Id,
    pub name: String,
    /// Closed loop in the plan, inches.
    pub points: Vec<Point>,
    /// Height above the floor datum, inches.
    pub elevation: f64,
}

impl RopePath {
    /// Length of the run, inches.
    pub fn length(&self) -> f64 {
        let n = self.points.len();
        (0..n)
            .map(|i| self.points[i].dist(self.points[(i + 1) % n]))
            .sum()
    }
}

/// The rope light runs of a tray: its hole's outline moved `h_offset` away
/// from the hole, at the lower ceiling plus `v_offset`. Sloped sides carry no
/// rope lights (manual p. 459), and a tray with a Caution makes none.
pub fn rope_light_paths(geom: &TrayGeom, rec: &TrayCeiling) -> Vec<RopePath> {
    if !geom.ok() || !rec.is_vertical() {
        return Vec::new();
    }
    rec.rope_lights
        .iter()
        .map(|r| RopePath {
            tray: geom.id,
            name: r.name.clone(),
            points: if r.h_offset.abs() < 1e-9 {
                geom.inner.clone()
            } else if r.h_offset > 0.0 {
                outset_outline(&geom.inner, r.h_offset)
            } else {
                inset_outline(&geom.inner, -r.h_offset).unwrap_or_else(|| geom.inner.clone())
            },
            elevation: geom.low() + r.v_offset,
        })
        .collect()
}

/// The molding runs of a tray (only for vertical sides): the hole's outline
/// at the top of the step plus the offsets. Returns `(profile, height,
/// outline, bottom elevation)`.
pub fn molding_runs(geom: &TrayGeom, rec: &TrayCeiling) -> Vec<(String, f64, Vec<Point>, f64)> {
    if !geom.ok() || !rec.is_vertical() {
        return Vec::new();
    }
    rec.moldings
        .iter()
        .map(|m| {
            let outline = if m.h_offset > 1e-9 {
                outset_outline(&geom.inner, m.h_offset)
            } else {
                geom.inner.clone()
            };
            // Crown hangs from the upper ceiling of the step.
            (
                m.profile.clone(),
                m.height,
                outline,
                geom.high() - m.height + m.v_offset,
            )
        })
        .collect()
}

/// One line of the step for the Materials List.
#[derive(Debug, Clone, PartialEq)]
pub struct StepLine {
    pub category: String,
    pub item: String,
    pub size: String,
    pub quantity: f64,
    pub unit: String,
}

/// The Materials List lines of a tray's step: the side wall area, the ring
/// of ceiling, moldings and rope lights by length, and the Components panel.
pub fn step_lines(geom: &TrayGeom, rec: &TrayCeiling) -> Vec<StepLine> {
    if !geom.ok() {
        return Vec::new();
    }
    let name = if rec.label.trim().is_empty() {
        "Tray ceiling".to_string()
    } else {
        format!("Tray ceiling {}", rec.label.trim())
    };
    let mut out = Vec::new();
    let depth = rec.depth.max(0.0);
    let slope = if geom.run > 1e-6 {
        (depth * depth + geom.run * geom.run).sqrt()
    } else {
        depth
    };
    out.push(StepLine {
        category: "Interior Finishes".into(),
        item: format!("{name} step sides"),
        size: if geom.run > 1e-6 {
            format!("{:.1} in sloped", slope)
        } else {
            format!("{:.1} in", depth)
        },
        quantity: geom.perimeter() * slope / 144.0,
        unit: "sq ft".into(),
    });
    let ring_area = (polygon_area(&geom.outer).abs() - polygon_area(&geom.inner).abs()).max(0.0);
    out.push(StepLine {
        category: "Interior Finishes".into(),
        item: format!("{name} outer ceiling"),
        size: format!("{:.1} in wide", rec.width),
        quantity: ring_area / 144.0,
        unit: "sq ft".into(),
    });
    for (profile, height, outline, _) in molding_runs(geom, rec) {
        let n = outline.len();
        let len: f64 = (0..n).map(|i| outline[i].dist(outline[(i + 1) % n])).sum();
        out.push(StepLine {
            category: "Interior Finishes".into(),
            item: format!("{name} molding {profile}"),
            size: format!("{height:.1} in"),
            quantity: len / 12.0,
            unit: "lf".into(),
        });
    }
    for r in rope_light_paths(geom, rec) {
        out.push(StepLine {
            category: "Electrical".into(),
            item: format!("{name} {}", r.name),
            size: String::new(),
            quantity: r.length() / 12.0,
            unit: "lf".into(),
        });
    }
    for c in &rec.components {
        if !c.name.trim().is_empty() {
            out.push(StepLine {
                category: "Interior Finishes".into(),
                item: c.name.clone(),
                size: String::new(),
                quantity: c.quantity,
                unit: c.unit.clone(),
            });
        }
    }
    out
}

/// Every tray of `floor` as a Materials List contribution.
pub fn floor_step_lines(floor: &Floor, rooms: &[crate::rooms::Room]) -> Vec<StepLine> {
    let geoms = resolve(floor, &room_ceilings(floor, rooms));
    geoms
        .iter()
        .filter_map(|g| floor.tray(g.id).map(|r| step_lines(g, r)))
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RoomName;
    use crate::{detect_rooms, WallKind};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    fn room(x1: f64, y1: f64, h: f64) -> RoomCeiling {
        RoomCeiling {
            outline: rect(0.0, 0.0, x1, y1),
            height: h,
            flat: true,
        }
    }

    fn project_with_room() -> Project {
        let mut p = Project::new("t");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
        p
    }

    #[test]
    fn tray_in_room_follows_the_room_inset_by_the_width() {
        let mut p = Project::new("t");
        let spec = TrayCeiling {
            width: 24.0,
            ..Default::default()
        };
        let id = p
            .make_tray_in_room(0, &rect(0.0, 0.0, 240.0, 180.0), spec)
            .unwrap();
        let hole = p.floors[0].tray_outline(id).unwrap();
        assert!((polygon_area(&hole) - 192.0 * 132.0).abs() < 1e-6);
        assert!(p.floors[0].is_tray(id));
        assert_eq!(p.floors[0].cad.last().unwrap().layer, LAYER);
        // A room too small for the width makes nothing.
        assert!(p
            .make_tray_in_room(0, &rect(0.0, 0.0, 40.0, 40.0), TrayCeiling::default())
            .is_none());
    }

    #[test]
    fn step_down_ring_and_hole_heights() {
        let mut p = Project::new("t");
        let id = p
            .make_tray_in_room(0, &rect(0.0, 0.0, 240.0, 180.0), TrayCeiling::default())
            .unwrap();
        let g = resolve(&p.floors[0], &[room(240.0, 180.0, 96.0)]);
        assert_eq!(g.len(), 1);
        let t = &g[0];
        assert_eq!(t.id, id);
        assert!(t.ok());
        assert_eq!((t.base, t.h_inner, t.h_outer), (96.0, 96.0, 88.0));
        // The ring is dropped 8", the hole stays at the room's ceiling.
        assert_eq!(ceiling_height_at(&g, Point::new(10.0, 10.0), 96.0), 88.0);
        assert_eq!(ceiling_height_at(&g, Point::new(120.0, 90.0), 96.0), 96.0);
        // Outside every tray: the room's own height.
        assert_eq!(ceiling_height_at(&[], Point::new(10.0, 10.0), 96.0), 96.0);
    }

    #[test]
    fn recess_raises_the_hole_into_the_platform() {
        let mut p = Project::new("t");
        let spec = TrayCeiling {
            recess: true,
            depth: 10.0,
            ..Default::default()
        };
        p.make_tray_in_room(0, &rect(0.0, 0.0, 240.0, 180.0), spec)
            .unwrap();
        let g = resolve(&p.floors[0], &[room(240.0, 180.0, 96.0)]);
        assert_eq!((g[0].h_outer, g[0].h_inner), (96.0, 106.0));
        assert_eq!(ceiling_height_at(&g, Point::new(120.0, 90.0), 96.0), 106.0);
        assert_eq!(ceiling_height_at(&g, Point::new(5.0, 5.0), 96.0), 96.0);
    }

    #[test]
    fn nested_tray_sits_in_the_parent_hole() {
        let mut p = Project::new("t");
        let parent = p
            .make_tray_in_room(0, &rect(0.0, 0.0, 240.0, 180.0), TrayCeiling::default())
            .unwrap();
        let nested = p
            .make_nested_tray(
                0,
                parent,
                TrayCeiling {
                    width: 12.0,
                    depth: 4.0,
                    ..Default::default()
                },
            )
            .unwrap();
        let g = resolve(&p.floors[0], &[room(240.0, 180.0, 96.0)]);
        let (gp, gn) = (&g[0], &g[1]);
        assert_eq!((gp.id, gn.id), (parent, nested));
        assert_eq!(gn.parent, Some(parent));
        assert_eq!(gn.level, 1);
        assert_eq!(gn.base, gp.h_inner);
        assert_eq!(gn.h_outer, 96.0 - 4.0);
        // The nested hole is the parent hole inset by 12.
        assert!((polygon_area(&gn.inner) - 168.0 * 108.0).abs() < 1e-6);
        // Heights: room ring 88, nested ring 92, center 96.
        assert_eq!(ceiling_height_at(&g, Point::new(10.0, 10.0), 96.0), 88.0);
        assert_eq!(ceiling_height_at(&g, Point::new(30.0, 90.0), 96.0), 92.0);
        assert_eq!(ceiling_height_at(&g, Point::new(120.0, 90.0), 96.0), 96.0);
    }

    #[test]
    fn coffered_grid_of_three_by_two() {
        let region = rect(0.0, 0.0, 240.0, 160.0);
        let cells = coffer_cells(&region, 3, 2, 8.0);
        assert_eq!(cells.len(), 6);
        // (240 - 4*8) / 3 = 69.33 wide, (160 - 3*8) / 2 = 68 deep.
        let a = polygon_area(&cells[0]);
        assert!((a - (208.0 / 3.0) * 68.0).abs() < 1e-6);
        for c in &cells {
            assert!((polygon_area(c) - a).abs() < 1e-6);
        }
        // The beam between two coffers is 8" wide.
        assert!((cells[1][0].x - cells[0][1].x - 8.0).abs() < 1e-9);
        assert!((cells[3][0].y - cells[0][3].y - 8.0).abs() < 1e-9);
        // A grid that cannot fit makes none.
        assert!(coffer_cells(&region, 3, 2, 60.0).is_empty());
        // Made into trays: six recessed coffers, beams at the room's height.
        let mut p = Project::new("t");
        let spec = TrayCeiling {
            depth: 6.0,
            ..Default::default()
        };
        let ids = p.make_coffered_ceiling(0, &region, 3, 2, 8.0, &spec);
        assert_eq!(ids.len(), 6);
        let g = resolve(&p.floors[0], &[room(240.0, 160.0, 96.0)]);
        assert!(g.iter().all(|t| t.ok() && t.level == 0 && t.recess_check()));
        let c = cells[0][0] + Point::new(30.0, 30.0);
        assert_eq!(ceiling_height_at(&g, c, 96.0), 102.0);
        // On a beam: flush with the room's ceiling.
        let beam = Point::new(cells[0][1].x + 4.0, 40.0);
        assert_eq!(ceiling_height_at(&g, beam, 96.0), 96.0);
    }

    impl TrayGeom {
        fn recess_check(&self) -> bool {
            self.h_inner > self.h_outer
        }
    }

    #[test]
    fn sloped_side_interpolates_across_the_run() {
        let mut p = Project::new("t");
        // 8" deep at 12 in 12: a 8" run.
        let spec = TrayCeiling {
            pitch: Some(12.0),
            ..Default::default()
        };
        p.make_tray_in_room(0, &rect(0.0, 0.0, 240.0, 180.0), spec)
            .unwrap();
        let g = resolve(&p.floors[0], &[room(240.0, 180.0, 96.0)]);
        let t = &g[0];
        assert!(t.ok());
        assert!((t.run - 8.0).abs() < 1e-9);
        // The hole starts 24" in from x = 0: 4" out from its edge is halfway.
        let mid = t.height_at(Point::new(24.0 - 4.0, 90.0)).unwrap();
        assert!((mid - 92.0).abs() < 1e-9, "{mid}");
        // Past the run: the dropped ceiling.
        assert_eq!(t.height_at(Point::new(24.0 - 12.0, 90.0)).unwrap(), 88.0);
        // A run wider than the ring is a Caution.
        let mut q = Project::new("t");
        let spec = TrayCeiling {
            width: 6.0,
            pitch: Some(12.0),
            ..Default::default()
        };
        q.make_tray_in_room(0, &rect(0.0, 0.0, 240.0, 180.0), spec)
            .unwrap();
        let g = resolve(&q.floors[0], &[room(240.0, 180.0, 96.0)]);
        assert_eq!(g[0].caution, Some(Caution::SlopeTooWide));
        // A caution tray changes no heights.
        assert_eq!(ceiling_height_at(&g, Point::new(3.0, 3.0), 96.0), 96.0);
    }

    #[test]
    fn caution_for_a_cathedral_room_or_no_room() {
        let mut p = Project::new("t");
        p.make_tray_in_room(0, &rect(0.0, 0.0, 240.0, 180.0), TrayCeiling::default())
            .unwrap();
        let mut r = room(240.0, 180.0, 96.0);
        r.flat = false;
        let g = resolve(&p.floors[0], &[r]);
        assert_eq!(g[0].caution, Some(Caution::RoomNotFlat));
        let g = resolve(&p.floors[0], &[]);
        assert_eq!(g[0].caution, Some(Caution::OutsideRoom));
        // A bow tie is not a tray.
        let bow = vec![
            Point::new(10.0, 10.0),
            Point::new(100.0, 100.0),
            Point::new(100.0, 10.0),
            Point::new(10.0, 100.0),
        ];
        assert!(p.add_tray(0, &bow, TrayCeiling::default()).is_none());
    }

    #[test]
    fn rope_lights_and_moldings_need_vertical_sides() {
        let mut p = Project::new("t");
        let spec = TrayCeiling {
            rope_lights: vec![RopeLight::default()],
            moldings: vec![TrayMolding::default()],
            ..Default::default()
        };
        let id = p
            .make_tray_in_room(0, &rect(0.0, 0.0, 240.0, 180.0), spec)
            .unwrap();
        let g = resolve(&p.floors[0], &[room(240.0, 180.0, 96.0)]);
        let rec = p.floors[0].tray(id).unwrap();
        let ropes = rope_light_paths(&g[0], rec);
        assert_eq!(ropes.len(), 1);
        // 1" out from the hole at the dropped ceiling plus 2".
        assert_eq!(ropes[0].elevation, 88.0 + 2.0);
        assert!((polygon_area(&ropes[0].points) - 194.0 * 134.0).abs() < 1e-6);
        assert_eq!(molding_runs(&g[0], rec).len(), 1);
        // Sloped sides: neither.
        let mut sloped = rec.clone();
        sloped.pitch = Some(24.0);
        assert!(rope_light_paths(&g[0], &sloped).is_empty());
        assert!(molding_runs(&g[0], &sloped).is_empty());
    }

    #[test]
    fn step_lines_cover_sides_ring_moldings_and_ropes() {
        let mut p = Project::new("t");
        let spec = TrayCeiling {
            rope_lights: vec![RopeLight::default()],
            moldings: vec![TrayMolding::default()],
            components: vec![TrayComponent {
                name: "LED driver".into(),
                quantity: 2.0,
                unit: "ea".into(),
            }],
            ..Default::default()
        };
        let id = p
            .make_tray_in_room(0, &rect(0.0, 0.0, 240.0, 180.0), spec)
            .unwrap();
        let g = resolve(&p.floors[0], &[room(240.0, 180.0, 96.0)]);
        let lines = step_lines(&g[0], p.floors[0].tray(id).unwrap());
        let sides = &lines[0];
        // Perimeter 2 * (192 + 132) = 648 in times 8 in deep.
        assert!((sides.quantity - 648.0 * 8.0 / 144.0).abs() < 1e-9);
        assert_eq!(sides.unit, "sq ft");
        let ring = &lines[1];
        assert!((ring.quantity - (240.0 * 180.0 - 192.0 * 132.0) / 144.0).abs() < 1e-9);
        assert!(lines
            .iter()
            .any(|l| l.item.contains("molding") && l.unit == "lf"));
        assert!(lines.iter().any(|l| l.category == "Electrical"));
        assert!(lines
            .iter()
            .any(|l| l.item == "LED driver" && l.quantity == 2.0));
    }

    #[test]
    fn records_without_a_polyline_are_ignored_and_pruned() {
        let mut p = Project::new("t");
        let id = p
            .make_tray_in_room(0, &rect(0.0, 0.0, 240.0, 180.0), TrayCeiling::default())
            .unwrap();
        p.floors[0].cad.retain(|c| c.id != id);
        assert!(!p.floors[0].is_tray(id));
        assert!(resolve(&p.floors[0], &[room(240.0, 180.0, 96.0)]).is_empty());
        assert_eq!(p.floors[0].prune_trays(), 1);
        assert!(p.floors[0].trays.is_empty());
    }

    #[test]
    fn convert_polyline_makes_a_tray_once() {
        let mut p = Project::new("t");
        let id = p.alloc_id();
        p.floors[0].cad.push(CadObject {
            id,
            layer: crate::cad::DEFAULT_CAD_LAYER.into(),
            item: CadItem::Polyline {
                points: rect(40.0, 40.0, 140.0, 120.0),
                closed: true,
            },
        });
        assert!(p.convert_polyline_to_tray(0, id, TrayCeiling::default()));
        assert!(p.floors[0].is_tray(id));
        assert!(!p.convert_polyline_to_tray(0, id, TrayCeiling::default()));
        // An open polyline is refused.
        let open = p.alloc_id();
        p.floors[0].cad.push(CadObject {
            id: open,
            layer: crate::cad::DEFAULT_CAD_LAYER.into(),
            item: CadItem::Polyline {
                points: rect(0.0, 0.0, 50.0, 50.0),
                closed: false,
            },
        });
        assert!(!p.convert_polyline_to_tray(0, open, TrayCeiling::default()));
    }

    #[test]
    fn room_ceilings_read_the_named_room() {
        let mut p = project_with_room();
        p.floors[0].ceiling_height = 108.0;
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert_eq!(room_ceilings(&p.floors[0], &rooms)[0].height, 108.0);
        let mut n = RoomName::new(Point::new(120.0, 90.0), "Living", "Living Room");
        n.ceiling_height = Some(120.0);
        p.floors[0].room_names.push(n);
        let rc = room_ceilings(&p.floors[0], &rooms);
        assert_eq!(rc[0].height, 120.0);
        assert!(rc[0].flat);
        p.floors[0].room_names[0].flat_ceiling = false;
        assert!(!room_ceilings(&p.floors[0], &rooms)[0].flat);
    }

    #[test]
    fn pitch_converts_between_rise_and_degrees() {
        let mut t = TrayCeiling::default();
        t.set_pitch_display(6.0);
        assert_eq!(t.pitch, Some(6.0));
        t.pitch_in_degrees = true;
        t.set_pitch_display(45.0);
        assert!((t.pitch.unwrap() - 12.0).abs() < 1e-9);
        assert!((t.pitch_display().unwrap() - 45.0).abs() < 1e-9);
        t.set_pitch_display(0.0);
        assert!(t.is_vertical());
    }

    #[test]
    fn records_round_trip_through_json() {
        let spec = TrayCeiling {
            pitch: Some(8.0),
            rope_lights: vec![RopeLight::default()],
            ..Default::default()
        };
        let s = serde_json::to_string(&spec).unwrap();
        assert_eq!(serde_json::from_str::<TrayCeiling>(&s).unwrap(), spec);
        // A record with only an id still loads, with the defaults.
        let t: TrayCeiling = serde_json::from_str(r#"{"id":7}"#).unwrap();
        assert_eq!((t.id, t.depth, t.recess), (7, DEFAULT_DEPTH, false));
    }
}
