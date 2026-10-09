//! Corner trim, moldings, material regions, wall hatching, decks and 3D solids
//! (the Trim flyout, Floor/Wall Material Region, Wall Hatching, Polygon Shaped
//! Deck and the 3D Solid flyout; `docs/chief-x18-subtools.md`).
//!
//! Lengths are inches. Elevations are relative to the finished floor of the
//! floor the object sits on.
//!
//! # Storage
//!
//! A floor's [`DetailsLayer`] lives in the typed slot `Floor.details` (opaque
//! JSON, like `foundation`), saved and undone with the plan:
//! [`DetailsLayer::load`] / [`DetailsLayer::store`].
//!
//! # Corners
//!
//! [`exterior_corners`] finds the corners of the exterior walls from the
//! detected rooms: for every vertex of a room where both edges lie on
//! exterior walls it computes the apex of the two outer wall faces and the
//! axes of the trim there ([`CornerAxes`]). Corner boards and quoins are both
//! an "L" laid on the two outer faces from that apex.

use crate::geometry::{dist_to_segment, point_in_polygon, polygon_area, polygon_centroid, Point};
use crate::model::{Floor, Id, Project, Wall, WallKind};
use crate::rooms::Room;
use crate::walls::{Side, WallClass};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

mod cad_detail;
pub use cad_detail::*;

/// Layer corner boards and quoins are drawn on.
pub const CORNER_TRIM_LAYER: &str = "Corner Trim";
/// Layer moldings are drawn on.
pub const MOLDING_LAYER: &str = "Moldings";
/// Layer material regions and wall hatches are drawn on.
pub const REGION_LAYER: &str = "Material Regions";
/// Layer polygon decks are drawn on.
pub const DECK_LAYER: &str = "Decks";
/// Layer 3D solids are drawn on.
pub const SOLID_LAYER: &str = "3D Solids";

/// Default corner board width, inches.
pub const DEFAULT_CORNER_BOARD_WIDTH: f64 = 3.5;
/// Default corner board thickness, inches.
pub const DEFAULT_CORNER_BOARD_THICKNESS: f64 = 0.75;
/// Default quoin block length (the long side), inches.
pub const DEFAULT_QUOIN_WIDTH: f64 = 16.0;
/// Default quoin block height, inches.
pub const DEFAULT_QUOIN_HEIGHT: f64 = 8.0;
/// Default quoin depth (how far it stands off the wall), inches.
pub const DEFAULT_QUOIN_DEPTH: f64 = 1.5;
/// The short quoin block is this fraction of the long one.
pub const QUOIN_SHORT_RATIO: f64 = 0.6;
/// Default material region plate thickness, inches.
pub const DEFAULT_REGION_THICKNESS: f64 = 0.25;
/// Default deck board thickness, inches.
pub const DEFAULT_DECK_BOARD_THICKNESS: f64 = 1.5;
/// Height of a deck railing, inches.
pub const DECK_RAILING_HEIGHT: f64 = 36.0;
/// Default hatch scale.
pub const DEFAULT_HATCH_SCALE: f64 = 1.0;
/// Default hatch angle, degrees.
pub const DEFAULT_HATCH_ANGLE: f64 = 45.0;
/// Circles and spheres are drawn with this many segments.
pub const SOLID_SEGMENTS: usize = 24;

/// Material names used when an object has none picked.
pub const DEFAULT_TRIM_MATERIAL: &str = "Painted White Trim";
pub const DEFAULT_DECK_MATERIAL: &str = "Oak Flooring";
pub const DEFAULT_SOLID_MATERIAL: &str = "Concrete";
pub const DEFAULT_REGION_MATERIAL: &str = "Ceramic Tile 12x12";
pub const DEFAULT_QUOIN_MATERIAL: &str = "Stone Veneer – Fieldstone";

fn default_trim_material() -> String {
    DEFAULT_TRIM_MATERIAL.to_string()
}

fn default_corner_layer() -> String {
    CORNER_TRIM_LAYER.to_string()
}

// ===================================================================
// Corners
// ===================================================================

/// The plan line of a detail when it differs from its layer's (the Line Style
/// page of the detail dialogs): color, plotted weight in hundredths of a
/// millimetre, and dash. Every field `None` means "as the layer says".
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DetailStyle {
    pub color: Option<[u8; 3]>,
    pub weight: Option<u32>,
    pub dash: Option<crate::layers::LineStyle>,
}

impl DetailStyle {
    /// Nothing differs from the layer.
    pub fn is_default(&self) -> bool {
        *self == DetailStyle::default()
    }
}

/// The two wall faces meeting at a trim corner. All vectors are unit length.
/// `dir_a` / `dir_b` run along the outer faces away from the apex; `out_a` /
/// `out_b` are the outward normals of those faces (away from the building at
/// a convex corner), so a board on face A occupies `apex + dir_a * s +
/// out_a * t`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CornerAxes {
    pub dir_a: Point,
    pub dir_b: Point,
    pub out_a: Point,
    pub out_b: Point,
}

impl Default for CornerAxes {
    /// A right-angled corner: faces along +x and +y, building in the first
    /// quadrant.
    fn default() -> Self {
        Self {
            dir_a: Point::new(1.0, 0.0),
            dir_b: Point::new(0.0, 1.0),
            out_a: Point::new(0.0, -1.0),
            out_b: Point::new(-1.0, 0.0),
        }
    }
}

impl CornerAxes {
    /// The "L" of trim laid on the two faces from `apex`: `la` along face A,
    /// `lb` along face B, standing `t` off the wall. Counter-clockwise or
    /// clockwise depending on the corner; six points.
    pub fn l_polygon(&self, apex: Point, la: f64, lb: f64, t: f64) -> Vec<Point> {
        let (a, b) = (self.dir_a, self.dir_b);
        let (na, nb) = (self.out_a * t, self.out_b * t);
        vec![
            apex + a * la,
            apex + a * la + na,
            apex + na + nb,
            apex + b * lb + nb,
            apex + b * lb,
            apex,
        ]
    }

    /// The three convex pieces of the "L" (board on face A, board on face B,
    /// and the square where they meet), each four points.
    pub fn l_parts(&self, apex: Point, la: f64, lb: f64, t: f64) -> [[Point; 4]; 3] {
        let (a, b) = (self.dir_a, self.dir_b);
        let (na, nb) = (self.out_a * t, self.out_b * t);
        [
            [apex, apex + a * la, apex + a * la + na, apex + na],
            [apex, apex + b * lb, apex + b * lb + nb, apex + nb],
            [apex, apex + na, apex + na + nb, apex + nb],
        ]
    }
}

/// One corner of the exterior walls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExteriorCorner {
    /// Intersection of the two outer wall faces.
    pub apex: Point,
    /// The centerline junction of the two walls.
    pub vertex: Point,
    pub axes: CornerAxes,
    pub wall_a: Id,
    pub wall_b: Id,
    /// The lower of the two wall heights (floor to top plate), inches.
    pub height: f64,
    /// A corner that points out of the building (otherwise an inside corner).
    pub convex: bool,
}

/// Two room vertices within this distance are the same corner.
const SAME_CORNER: f64 = 1.0;
/// `|sin|` of the angle between the edges below which a vertex is not a corner.
const MIN_SIN: f64 = 0.02;

/// The straight exterior wall whose centerline carries the edge `a`-`b`.
fn exterior_wall_along(floor: &Floor, a: Point, b: Point) -> Option<&Wall> {
    let mid = Point::lerp(a, b, 0.5);
    let dir = (b - a).normalized();
    floor.walls.iter().find(|w| {
        w.kind == WallKind::Exterior
            && !w.flags.invisible
            && !w.is_curved()
            && matches!(w.class, WallClass::Standard | WallClass::Foundation)
            && w.length() > 1e-6
            && dist_to_segment(mid, w.start, w.end) <= 0.6
            && w.direction().cross(dir).abs() < 0.05
    })
}

/// Every corner of the exterior walls of `floor`, convex and inside corners,
/// found from the vertices of `rooms` whose two edges lie on straight
/// exterior walls (curved walls and walls that are not plain exterior walls
/// never make a corner). Corners within an inch of each other count once.
pub fn exterior_corners(floor: &Floor, rooms: &[Room]) -> Vec<ExteriorCorner> {
    let mut out: Vec<ExteriorCorner> = Vec::new();
    for room in rooms {
        let poly = &room.polygon;
        let n = poly.len();
        if n < 3 {
            continue;
        }
        for i in 0..n {
            let (prev, v, next) = (poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]);
            let (da, db) = ((prev - v).normalized(), (next - v).normalized());
            if da.length() < 0.5 || db.length() < 0.5 || da.cross(db).abs() < MIN_SIN {
                continue;
            }
            let (Some(wa), Some(wb)) = (
                exterior_wall_along(floor, prev, v),
                exterior_wall_along(floor, v, next),
            ) else {
                continue;
            };
            // A counter-clockwise polygon turns left at a convex vertex.
            let convex = (v - prev).cross(next - v) > 0.0;
            let toward = |n: Point, other: Point| {
                // Outward is away from the other wall at a convex corner and
                // toward it at an inside corner.
                if (n.dot(other) < 0.0) == convex {
                    n
                } else {
                    -n
                }
            };
            let (na, nb) = (toward(da.perp(), db), toward(db.perp(), da));
            let (ha, hb) = (wa.thickness * 0.5, wb.thickness * 0.5);
            let r = nb * hb - na * ha;
            let det = -da.cross(db);
            let s = db.cross(r) / det;
            let apex = v + na * ha + da * s;
            if out.iter().any(|c| c.apex.dist(apex) < SAME_CORNER) {
                continue;
            }
            out.push(ExteriorCorner {
                apex,
                vertex: v,
                axes: CornerAxes {
                    dir_a: da,
                    dir_b: db,
                    out_a: na,
                    out_b: nb,
                },
                wall_a: wa.id,
                wall_b: wb.id,
                height: wa.height.min(wb.height),
                convex,
            });
        }
    }
    out
}

/// The corner of `corners` nearest to `p` within `tol`.
pub fn corner_near(corners: &[ExteriorCorner], p: Point, tol: f64) -> Option<ExteriorCorner> {
    corners
        .iter()
        .map(|c| (c.apex.dist(p).min(c.vertex.dist(p)), c))
        .filter(|(d, _)| *d <= tol)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| *c)
}

// ===================================================================
// Corner boards and quoins
// ===================================================================

/// A corner board (Corner Boards, Auto Place Corner Boards): two boards in an
/// "L" on the outer faces of an exterior corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CornerBoard {
    pub id: Id,
    /// The apex of the two outer wall faces.
    pub wall_corner: Point,
    pub axes: CornerAxes,
    /// Width of each board face, inches.
    pub width: f64,
    /// How far the board stands off the wall, inches.
    pub thickness: f64,
    /// Bottom of the board above the floor, inches.
    pub base: f64,
    /// Height of the board, floor to top plate by default, inches.
    pub height: f64,
    pub material: String,
    pub layer: String,
    /// Own line look (color, weight, dash) when it differs from the layer's.
    pub style: DetailStyle,
}

impl Default for CornerBoard {
    fn default() -> Self {
        Self {
            id: 0,
            wall_corner: Point::ZERO,
            axes: CornerAxes::default(),
            width: DEFAULT_CORNER_BOARD_WIDTH,
            thickness: DEFAULT_CORNER_BOARD_THICKNESS,
            base: 0.0,
            height: 96.0,
            material: default_trim_material(),
            layer: default_corner_layer(),
            style: DetailStyle::default(),
        }
    }
}

impl CornerBoard {
    pub fn at(id: Id, c: &ExteriorCorner) -> Self {
        Self {
            id,
            wall_corner: c.apex,
            axes: c.axes,
            height: c.height,
            ..Self::default()
        }
    }

    /// The plan outline: the "L" of the two boards.
    pub fn outline(&self) -> Vec<Point> {
        self.axes
            .l_polygon(self.wall_corner, self.width, self.width, self.thickness)
    }

    /// The three convex pieces for 3D.
    pub fn parts(&self) -> [[Point; 4]; 3] {
        self.axes
            .l_parts(self.wall_corner, self.width, self.width, self.thickness)
    }

    /// Wood in the two boards, cubic inches.
    pub fn volume(&self) -> f64 {
        polygon_area(&self.outline()).abs() * self.height
    }
}

/// A set of quoins (Quoins, Auto Place Quoins): stacked blocks at a corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Quoin {
    pub id: Id,
    pub corner: Point,
    pub axes: CornerAxes,
    /// Length of the long block, inches.
    pub width: f64,
    /// Height of one block, inches.
    pub height: f64,
    /// How far the blocks stand off the wall, inches.
    pub depth: f64,
    /// Long and short blocks swap faces on every course.
    pub alternating: bool,
    /// Bottom of the stack above the floor, inches.
    pub base: f64,
    /// Height of the whole stack, inches.
    pub total_height: f64,
    pub material: String,
    pub layer: String,
    /// Own line look (color, weight, dash) when it differs from the layer's.
    pub style: DetailStyle,
}

impl Default for Quoin {
    fn default() -> Self {
        Self {
            id: 0,
            corner: Point::ZERO,
            axes: CornerAxes::default(),
            width: DEFAULT_QUOIN_WIDTH,
            height: DEFAULT_QUOIN_HEIGHT,
            depth: DEFAULT_QUOIN_DEPTH,
            alternating: true,
            base: 0.0,
            total_height: 96.0,
            material: DEFAULT_QUOIN_MATERIAL.to_string(),
            layer: default_corner_layer(),
            style: DetailStyle::default(),
        }
    }
}

impl Quoin {
    pub fn at(id: Id, c: &ExteriorCorner) -> Self {
        Self {
            id,
            corner: c.apex,
            axes: c.axes,
            total_height: c.height,
            ..Self::default()
        }
    }

    /// Number of courses in the stack (at least one).
    pub fn courses(&self) -> usize {
        if self.height <= 1e-6 {
            return 1;
        }
        ((self.total_height / self.height + 1e-9).floor() as usize).max(1)
    }

    /// Lengths `(face A, face B)` of the blocks of course `i`.
    pub fn course_lengths(&self, i: usize) -> (f64, f64) {
        let (long, short) = (self.width, self.width * QUOIN_SHORT_RATIO);
        if self.alternating && i % 2 == 1 {
            (short, long)
        } else {
            (long, short)
        }
    }

    /// The plan outline of the top course: the "L" of the blocks.
    pub fn outline(&self) -> Vec<Point> {
        let (la, lb) = self.course_lengths(0);
        self.axes.l_polygon(self.corner, la, lb, self.depth)
    }

    /// Elevation of the bottom of course `i`.
    pub fn course_base(&self, i: usize) -> f64 {
        self.base + self.height * i as f64
    }
}

// ===================================================================
// Moldings
// ===================================================================

/// The profile of a molding line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum MoldingProfile {
    #[default]
    Crown,
    Base,
    Chair,
    Casing,
    /// A custom cross section: points `(projection, height)` in inches,
    /// measured from the molding's bottom edge at the wall.
    Custom(Vec<Point>),
}

impl MoldingProfile {
    pub fn name(&self) -> &'static str {
        match self {
            MoldingProfile::Crown => "Crown",
            MoldingProfile::Base => "Base",
            MoldingProfile::Chair => "Chair",
            MoldingProfile::Casing => "Casing",
            MoldingProfile::Custom(_) => "Custom",
        }
    }

    /// `(height, width)` of the profile's box, inches.
    pub fn default_size(&self) -> (f64, f64) {
        match self {
            MoldingProfile::Crown => (4.5, 3.5),
            MoldingProfile::Base => (5.5, 0.75),
            MoldingProfile::Chair => (3.0, 1.0),
            MoldingProfile::Casing => (3.5, 0.75),
            MoldingProfile::Custom(pts) => {
                let (_, hi) = bounds(pts);
                (hi.y.max(0.5), hi.x.max(0.5))
            }
        }
    }

    /// Elevation of the bottom edge above the floor for a ceiling of height
    /// `ceiling`, inches.
    pub fn default_elevation(&self, ceiling: f64, height: f64) -> f64 {
        match self {
            MoldingProfile::Crown => (ceiling - height).max(0.0),
            MoldingProfile::Chair => 32.0,
            _ => 0.0,
        }
    }
}

/// A molding along a line or polyline (Molding Line, Molding Polyline). The
/// molding projects to the left of the drawing direction, from the bottom
/// edge at `elevation` up by `height`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MoldingLine {
    pub id: Id,
    pub polyline: Vec<Point>,
    pub profile: MoldingProfile,
    /// Vertical size of the profile, inches.
    pub height: f64,
    /// Projection from the wall, inches.
    pub width: f64,
    /// Elevation of the bottom edge above the floor, inches.
    pub elevation: f64,
    pub material: String,
    pub layer: String,
    /// Own line look (color, weight, dash) when it differs from the layer's.
    pub style: DetailStyle,
}

impl Default for MoldingLine {
    fn default() -> Self {
        let profile = MoldingProfile::default();
        let (height, width) = profile.default_size();
        Self {
            id: 0,
            polyline: Vec::new(),
            profile,
            height,
            width,
            elevation: 0.0,
            material: default_trim_material(),
            layer: MOLDING_LAYER.to_string(),
            style: DetailStyle::default(),
        }
    }
}

impl MoldingLine {
    pub fn new(id: Id, polyline: Vec<Point>, profile: MoldingProfile, ceiling: f64) -> Self {
        let (height, width) = profile.default_size();
        Self {
            id,
            polyline,
            elevation: profile.default_elevation(ceiling, height),
            profile,
            height,
            width,
            ..Self::default()
        }
    }

    /// Length of the line, inches.
    pub fn length(&self) -> f64 {
        self.polyline.windows(2).map(|s| s[0].dist(s[1])).sum()
    }

    /// The cross section as `(projection, height)` points, counter-clockwise:
    /// a `width` x `height` rectangle unless the profile is a usable
    /// [`MoldingProfile::Custom`] (scaled to the stated size).
    pub fn section(&self) -> Vec<Point> {
        if let MoldingProfile::Custom(pts) = &self.profile {
            if pts.len() >= 3 && polygon_area(pts).abs() > 1e-9 {
                let (_, hi) = bounds(pts);
                let (kx, ky) = (self.width / hi.x.max(1e-9), self.height / hi.y.max(1e-9));
                let mut s: Vec<Point> =
                    pts.iter().map(|p| Point::new(p.x * kx, p.y * ky)).collect();
                if polygon_area(&s) < 0.0 {
                    s.reverse();
                }
                return s;
            }
        }
        vec![
            Point::new(0.0, 0.0),
            Point::new(self.width, 0.0),
            Point::new(self.width, self.height),
            Point::new(0.0, self.height),
        ]
    }

    /// Volume of material, cubic inches (section area times length).
    pub fn volume(&self) -> f64 {
        polygon_area(&self.section()).abs() * self.length()
    }
}

// ===================================================================
// Material regions and wall hatching
// ===================================================================

/// Where a [`MaterialRegion`] lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RegionKind {
    /// A polygon on the floor.
    #[default]
    Floor,
    /// A rectangle on a face of one wall.
    Wall(Id),
}

/// A region of another material on the floor or on a wall face (Floor
/// Material Region, Wall Material Region).
///
/// For [`RegionKind::Floor`] `outline` is a plan polygon. For
/// [`RegionKind::Wall`] it is a rectangle in the wall-face frame: `x` is the
/// distance along the wall from its start (`u`), `y` the height above the
/// floor (`v`). `side` picks the face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialRegion {
    pub id: Id,
    pub kind: RegionKind,
    pub outline: Vec<Point>,
    /// Name of a `plan_materials` library material.
    pub material: String,
    /// Plate thickness, inches.
    pub thickness: f64,
    /// A floor region replaces (is flush with) the finish layers it covers
    /// instead of lying on top of them.
    pub cut_finish_layers: bool,
    /// Which face of the wall (wall regions only).
    pub side: Side,
    pub layer: String,
    /// Own line look (color, weight, dash) when it differs from the layer's.
    pub style: DetailStyle,
}

impl Default for MaterialRegion {
    fn default() -> Self {
        Self {
            id: 0,
            kind: RegionKind::Floor,
            outline: Vec::new(),
            material: DEFAULT_REGION_MATERIAL.to_string(),
            thickness: DEFAULT_REGION_THICKNESS,
            cut_finish_layers: false,
            side: Side::Left,
            layer: REGION_LAYER.to_string(),
            style: DetailStyle::default(),
        }
    }
}

/// The corners of a wall-face rectangle `u0..u1` x `v0..v1`, in the order
/// the region stores them.
pub fn wall_rect(u0: f64, u1: f64, v0: f64, v1: f64) -> Vec<Point> {
    let (ua, ub) = (u0.min(u1), u0.max(u1));
    let (va, vb) = (v0.min(v1), v0.max(v1));
    vec![
        Point::new(ua, va),
        Point::new(ub, va),
        Point::new(ub, vb),
        Point::new(ua, vb),
    ]
}

impl MaterialRegion {
    pub fn floor(id: Id, outline: Vec<Point>) -> Self {
        Self {
            id,
            outline,
            ..Self::default()
        }
    }

    pub fn wall(id: Id, wall_id: Id, side: Side, u0: f64, u1: f64, v0: f64, v1: f64) -> Self {
        Self {
            id,
            kind: RegionKind::Wall(wall_id),
            outline: wall_rect(u0, u1, v0, v1),
            side,
            ..Self::default()
        }
    }

    pub fn is_floor(&self) -> bool {
        matches!(self.kind, RegionKind::Floor)
    }

    pub fn wall_id(&self) -> Option<Id> {
        match self.kind {
            RegionKind::Wall(w) => Some(w),
            RegionKind::Floor => None,
        }
    }

    /// `(u0, u1, v0, v1)` of a wall region.
    pub fn uv_bounds(&self) -> Option<(f64, f64, f64, f64)> {
        if self.is_floor() || self.outline.is_empty() {
            return None;
        }
        let (lo, hi) = bounds(&self.outline);
        Some((lo.x, hi.x, lo.y, hi.y))
    }

    /// Area of the region, square inches (plan area for the floor, face area
    /// for a wall).
    pub fn area(&self) -> f64 {
        polygon_area(&self.outline).abs()
    }

    /// The plan polygon of the region: the floor outline, or the strip of
    /// the wall the rectangle covers (across the whole wall thickness).
    pub fn plan_polygon(&self, floor: &Floor) -> Option<Vec<Point>> {
        match self.kind {
            RegionKind::Floor => Some(self.outline.clone()),
            RegionKind::Wall(id) => {
                let w = floor.wall(id)?;
                let (u0, u1, _, _) = self.uv_bounds()?;
                Some(wall_strip(w, u0, u1))
            }
        }
    }

    /// Moves a floor region by `d`. Wall regions follow their wall.
    pub fn translate(&mut self, d: Point) {
        if self.is_floor() {
            translate_all(&mut self.outline, d);
        }
    }
}

/// The part of a straight wall between `u0` and `u1` along it, across its
/// whole thickness (four points).
pub fn wall_strip(w: &Wall, u0: f64, u1: f64) -> Vec<Point> {
    let len = w.length();
    let (a, b) = (u0.clamp(0.0, len), u1.clamp(0.0, len));
    let n = w.normal() * (w.thickness * 0.5);
    let (pa, pb) = (w.point_at(a), w.point_at(b));
    vec![pa + n, pb + n, pb - n, pa - n]
}

/// Hatching drawn on a wall in plan (Wall Hatching).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallHatch {
    pub id: Id,
    pub wall_id: Id,
    /// Name of a `plan_materials` pattern ("Lines", "Brick", ...).
    pub pattern: String,
    pub scale: f64,
    /// Angle of line patterns, degrees.
    pub angle: f64,
    pub layer: String,
    /// Own line look (color, weight, dash) when it differs from the layer's.
    pub style: DetailStyle,
}

impl Default for WallHatch {
    fn default() -> Self {
        Self {
            id: 0,
            wall_id: 0,
            pattern: "Lines".to_string(),
            scale: DEFAULT_HATCH_SCALE,
            angle: DEFAULT_HATCH_ANGLE,
            layer: REGION_LAYER.to_string(),
            style: DetailStyle::default(),
        }
    }
}

// ===================================================================
// Decks
// ===================================================================

/// A deck platform drawn as a polygon (Polygon Shaped Deck).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeckPolygon {
    pub id: Id,
    pub outline: Vec<Point>,
    /// Elevation of the top of the deck above the floor, inches.
    pub elevation: f64,
    /// A railing around the edge.
    pub railing: bool,
    /// Thickness of the decking boards, inches.
    pub board_thickness: f64,
    pub material: String,
    pub layer: String,
    /// Own line look (color, weight, dash) when it differs from the layer's.
    pub style: DetailStyle,
}

impl Default for DeckPolygon {
    fn default() -> Self {
        Self {
            id: 0,
            outline: Vec::new(),
            elevation: 0.0,
            railing: false,
            board_thickness: DEFAULT_DECK_BOARD_THICKNESS,
            material: DEFAULT_DECK_MATERIAL.to_string(),
            layer: DECK_LAYER.to_string(),
            style: DetailStyle::default(),
        }
    }
}

impl DeckPolygon {
    pub fn new(id: Id, outline: Vec<Point>) -> Self {
        Self {
            id,
            outline,
            ..Self::default()
        }
    }

    /// Plan area, square inches.
    pub fn area(&self) -> f64 {
        polygon_area(&self.outline).abs()
    }

    /// Length of the edge, inches.
    pub fn perimeter(&self) -> f64 {
        let n = self.outline.len();
        (0..n)
            .map(|i| self.outline[i].dist(self.outline[(i + 1) % n]))
            .sum()
    }

    /// Volume of decking, cubic inches.
    pub fn volume(&self) -> f64 {
        self.area() * self.board_thickness
    }

    pub fn translate(&mut self, d: Point) {
        translate_all(&mut self.outline, d);
    }
}

// ===================================================================
// 3D solids
// ===================================================================

/// The shape of a [`Solid3d`]. Outlines are relative to the solid's
/// position and rotate with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SolidKind {
    /// Centered on the position.
    Box {
        w: f64,
        d: f64,
        h: f64,
    },
    Cylinder {
        r: f64,
        h: f64,
    },
    /// Sits on its elevation: the center is `r` above it.
    Sphere {
        r: f64,
    },
    Cone {
        r: f64,
        h: f64,
    },
    /// A prism over an outline (3D Solid).
    PolylineSolid {
        outline: Vec<Point>,
        h: f64,
    },
    Pyramid {
        outline: Vec<Point>,
        h: f64,
    },
    /// A flat, two-sided polygon at the elevation (Face).
    Face {
        polygon: Vec<Point>,
    },
}

impl Default for SolidKind {
    fn default() -> Self {
        SolidKind::Box {
            w: 24.0,
            d: 24.0,
            h: 24.0,
        }
    }
}

impl SolidKind {
    pub fn name(&self) -> &'static str {
        match self {
            SolidKind::Box { .. } => "Box",
            SolidKind::Cylinder { .. } => "Cylinder",
            SolidKind::Sphere { .. } => "Sphere",
            SolidKind::Cone { .. } => "Cone",
            SolidKind::PolylineSolid { .. } => "3D Solid",
            SolidKind::Pyramid { .. } => "Pyramid",
            SolidKind::Face { .. } => "Face",
        }
    }
}

/// A 3D primitive (3D Solid flyout).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Solid3d {
    pub id: Id,
    pub kind: SolidKind,
    /// Plan position of the solid's origin.
    pub position: Point,
    /// Elevation of the bottom above the floor, inches.
    pub elevation: f64,
    /// Rotation about the vertical axis through `position`, degrees.
    pub rotation: f64,
    pub material: String,
    pub layer: String,
    /// Own line look (color, weight, dash) when it differs from the layer's.
    pub style: DetailStyle,
}

impl Default for Solid3d {
    fn default() -> Self {
        Self {
            id: 0,
            kind: SolidKind::default(),
            position: Point::ZERO,
            elevation: 0.0,
            rotation: 0.0,
            material: DEFAULT_SOLID_MATERIAL.to_string(),
            layer: SOLID_LAYER.to_string(),
            style: DetailStyle::default(),
        }
    }
}

/// Rotates `p` about the origin by `deg` degrees counter-clockwise.
pub fn rotate_deg(p: Point, deg: f64) -> Point {
    let (s, c) = deg.to_radians().sin_cos();
    Point::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

/// A regular polygon of [`SOLID_SEGMENTS`] points approximating a circle.
pub fn circle_points(center: Point, r: f64) -> Vec<Point> {
    (0..SOLID_SEGMENTS)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / SOLID_SEGMENTS as f64;
            Point::new(center.x + r * a.cos(), center.y + r * a.sin())
        })
        .collect()
}

impl Solid3d {
    pub fn new(id: Id, kind: SolidKind, position: Point) -> Self {
        Self {
            id,
            kind,
            position,
            ..Self::default()
        }
    }

    /// The footprint relative to the position, before rotation.
    pub fn local_footprint(&self) -> Vec<Point> {
        match &self.kind {
            SolidKind::Box { w, d, .. } => {
                let (hw, hd) = (w * 0.5, d * 0.5);
                vec![
                    Point::new(-hw, -hd),
                    Point::new(hw, -hd),
                    Point::new(hw, hd),
                    Point::new(-hw, hd),
                ]
            }
            SolidKind::Cylinder { r, .. } | SolidKind::Cone { r, .. } | SolidKind::Sphere { r } => {
                circle_points(Point::ZERO, *r)
            }
            SolidKind::PolylineSolid { outline, .. } | SolidKind::Pyramid { outline, .. } => {
                outline.clone()
            }
            SolidKind::Face { polygon } => polygon.clone(),
        }
    }

    /// The footprint in plan coordinates.
    pub fn footprint(&self) -> Vec<Point> {
        self.local_footprint()
            .into_iter()
            .map(|p| self.position + rotate_deg(p, self.rotation))
            .collect()
    }

    /// Height of the solid (a face is flat), inches.
    pub fn height(&self) -> f64 {
        match &self.kind {
            SolidKind::Box { h, .. }
            | SolidKind::Cylinder { h, .. }
            | SolidKind::Cone { h, .. }
            | SolidKind::PolylineSolid { h, .. }
            | SolidKind::Pyramid { h, .. } => *h,
            SolidKind::Sphere { r } => 2.0 * r,
            SolidKind::Face { .. } => 0.0,
        }
    }

    /// Volume, cubic inches (a face has none; the cylinder and cone are
    /// true circles, not their polygon).
    pub fn volume(&self) -> f64 {
        match &self.kind {
            SolidKind::Box { w, d, h } => w * d * h,
            SolidKind::Cylinder { r, h } => PI * r * r * h,
            SolidKind::Sphere { r } => 4.0 / 3.0 * PI * r * r * r,
            SolidKind::Cone { r, h } => PI * r * r * h / 3.0,
            SolidKind::PolylineSolid { outline, h } => polygon_area(outline).abs() * h,
            SolidKind::Pyramid { outline, h } => polygon_area(outline).abs() * h / 3.0,
            SolidKind::Face { .. } => 0.0,
        }
    }

    pub fn translate(&mut self, d: Point) {
        self.position = self.position + d;
    }

    /// Is the footprint an outline the user drew (3D Solid, Pyramid, Face),
    /// as opposed to a box or round solid made from a size?
    pub fn has_outline(&self) -> bool {
        matches!(
            self.kind,
            SolidKind::PolylineSolid { .. } | SolidKind::Pyramid { .. } | SolidKind::Face { .. }
        )
    }

    /// Moves corner `i` of an outline solid to the plan point `to`. Returns
    /// whether it moved.
    pub fn move_outline_vertex(&mut self, i: usize, to: Point) -> bool {
        let local = rotate_deg(to - self.position, -self.rotation);
        let pts = match &mut self.kind {
            SolidKind::PolylineSolid { outline, .. } | SolidKind::Pyramid { outline, .. } => {
                outline
            }
            SolidKind::Face { polygon } => polygon,
            _ => return false,
        };
        pts.get_mut(i).map(|p| *p = local).is_some()
    }
}

// ===================================================================
// The layer
// ===================================================================

/// Addresses one object of a [`DetailsLayer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetailRef {
    CornerBoard(Id),
    Quoin(Id),
    Molding(Id),
    Region(Id),
    Hatch(Id),
    Deck(Id),
    Solid(Id),
}

impl DetailRef {
    pub fn id(self) -> Id {
        match self {
            DetailRef::CornerBoard(i)
            | DetailRef::Quoin(i)
            | DetailRef::Molding(i)
            | DetailRef::Region(i)
            | DetailRef::Hatch(i)
            | DetailRef::Deck(i)
            | DetailRef::Solid(i) => i,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            DetailRef::CornerBoard(_) => "Corner Board",
            DetailRef::Quoin(_) => "Quoin",
            DetailRef::Molding(_) => "Molding",
            DetailRef::Region(_) => "Material Region",
            DetailRef::Hatch(_) => "Wall Hatch",
            DetailRef::Deck(_) => "Deck",
            DetailRef::Solid(_) => "3D Solid",
        }
    }
}

/// Everything the Trim, Material Region, Deck and 3D Solid tools put on one
/// floor.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DetailsLayer {
    pub corner_boards: Vec<CornerBoard>,
    pub quoins: Vec<Quoin>,
    pub moldings: Vec<MoldingLine>,
    pub regions: Vec<MaterialRegion>,
    pub hatches: Vec<WallHatch>,
    pub decks: Vec<DeckPolygon>,
    pub solids: Vec<Solid3d>,
}

macro_rules! lookup {
    ($get:ident, $get_mut:ident, $field:ident, $ty:ty) => {
        pub fn $get(&self, id: Id) -> Option<&$ty> {
            self.$field.iter().find(|x| x.id == id)
        }
        pub fn $get_mut(&mut self, id: Id) -> Option<&mut $ty> {
            self.$field.iter_mut().find(|x| x.id == id)
        }
    };
}

impl DetailsLayer {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.corner_boards.len()
            + self.quoins.len()
            + self.moldings.len()
            + self.regions.len()
            + self.hatches.len()
            + self.decks.len()
            + self.solids.len()
    }

    // ----- storage -----

    /// The layer stored on `floor` (empty when it has none or the data does
    /// not parse).
    pub fn load(floor: &Floor) -> Self {
        floor
            .details
            .as_ref()
            .map(crate::foreign::read_layer)
            .unwrap_or_default()
    }

    /// Stores the layer on `floor`; an empty layer clears the slot.
    pub fn store(&self, floor: &mut Floor) {
        // Records this build cannot read stay in the slot (QA-28).
        floor.details = crate::foreign::layer_slot(self, self.is_empty(), floor.details.as_ref());
    }

    // ----- lookup -----

    lookup!(corner_board, corner_board_mut, corner_boards, CornerBoard);
    lookup!(quoin, quoin_mut, quoins, Quoin);
    lookup!(molding, molding_mut, moldings, MoldingLine);
    lookup!(region, region_mut, regions, MaterialRegion);
    lookup!(hatch, hatch_mut, hatches, WallHatch);
    lookup!(deck, deck_mut, decks, DeckPolygon);
    lookup!(solid, solid_mut, solids, Solid3d);

    /// The object with this id, if any.
    pub fn find(&self, id: Id) -> Option<DetailRef> {
        if self.corner_board(id).is_some() {
            Some(DetailRef::CornerBoard(id))
        } else if self.quoin(id).is_some() {
            Some(DetailRef::Quoin(id))
        } else if self.molding(id).is_some() {
            Some(DetailRef::Molding(id))
        } else if self.region(id).is_some() {
            Some(DetailRef::Region(id))
        } else if self.hatch(id).is_some() {
            Some(DetailRef::Hatch(id))
        } else if self.deck(id).is_some() {
            Some(DetailRef::Deck(id))
        } else if self.solid(id).is_some() {
            Some(DetailRef::Solid(id))
        } else {
            None
        }
    }

    /// The layer the object is drawn on.
    pub fn layer_of(&self, r: DetailRef) -> Option<String> {
        match r {
            DetailRef::CornerBoard(i) => self.corner_board(i).map(|x| x.layer.clone()),
            DetailRef::Quoin(i) => self.quoin(i).map(|x| x.layer.clone()),
            DetailRef::Molding(i) => self.molding(i).map(|x| x.layer.clone()),
            DetailRef::Region(i) => self.region(i).map(|x| x.layer.clone()),
            DetailRef::Hatch(i) => self.hatch(i).map(|x| x.layer.clone()),
            DetailRef::Deck(i) => self.deck(i).map(|x| x.layer.clone()),
            DetailRef::Solid(i) => self.solid(i).map(|x| x.layer.clone()),
        }
    }

    // ----- editing -----

    /// Removes the object; returns whether it existed.
    pub fn remove(&mut self, r: DetailRef) -> bool {
        fn drop_id<T>(v: &mut Vec<T>, id: Id, get: impl Fn(&T) -> Id) -> bool {
            let n = v.len();
            v.retain(|x| get(x) != id);
            v.len() != n
        }
        match r {
            DetailRef::CornerBoard(i) => drop_id(&mut self.corner_boards, i, |x| x.id),
            DetailRef::Quoin(i) => drop_id(&mut self.quoins, i, |x| x.id),
            DetailRef::Molding(i) => drop_id(&mut self.moldings, i, |x| x.id),
            DetailRef::Region(i) => drop_id(&mut self.regions, i, |x| x.id),
            DetailRef::Hatch(i) => drop_id(&mut self.hatches, i, |x| x.id),
            DetailRef::Deck(i) => drop_id(&mut self.decks, i, |x| x.id),
            DetailRef::Solid(i) => drop_id(&mut self.solids, i, |x| x.id),
        }
    }

    /// Moves the object by `d`; returns whether it existed. Wall regions
    /// and hatches belong to their wall and do not move.
    pub fn translate(&mut self, r: DetailRef, d: Point) -> bool {
        match r {
            DetailRef::CornerBoard(i) => self
                .corner_board_mut(i)
                .map(|x| x.wall_corner = x.wall_corner + d)
                .is_some(),
            DetailRef::Quoin(i) => self.quoin_mut(i).map(|x| x.corner = x.corner + d).is_some(),
            DetailRef::Molding(i) => self
                .molding_mut(i)
                .map(|x| translate_all(&mut x.polyline, d))
                .is_some(),
            DetailRef::Region(i) => self.region_mut(i).map(|x| x.translate(d)).is_some(),
            DetailRef::Hatch(i) => self.hatch(i).is_some(),
            DetailRef::Deck(i) => self.deck_mut(i).map(|x| x.translate(d)).is_some(),
            DetailRef::Solid(i) => self.solid_mut(i).map(|x| x.translate(d)).is_some(),
        }
    }

    /// The plan bounding box `(lo, hi)` of the object.
    pub fn plan_bounds(&self, floor: &Floor, r: DetailRef) -> Option<(Point, Point)> {
        let pts: Vec<Point> = match r {
            DetailRef::CornerBoard(i) => self.corner_board(i)?.outline(),
            DetailRef::Quoin(i) => self.quoin(i)?.outline(),
            DetailRef::Molding(i) => {
                let m = self.molding(i)?;
                let mut pts = m.polyline.clone();
                // The molding projects to the left of the line.
                for s in m.polyline.windows(2) {
                    let n = (s[1] - s[0]).normalized().perp() * m.width;
                    pts.push(s[0] + n);
                    pts.push(s[1] + n);
                }
                pts
            }
            DetailRef::Region(i) => self.region(i)?.plan_polygon(floor)?,
            DetailRef::Hatch(i) => {
                let w = floor.wall(self.hatch(i)?.wall_id)?;
                w.footprint().to_vec()
            }
            DetailRef::Deck(i) => self.deck(i)?.outline.clone(),
            DetailRef::Solid(i) => self.solid(i)?.footprint(),
        };
        (!pts.is_empty()).then(|| bounds(&pts))
    }

    // ----- corners (reshaping) -----

    /// The corners the user can drag, in plan coordinates: the outline of a
    /// floor region or deck, the points of a molding polyline, and the outline
    /// of a 3D solid made from one (3D Solid, Pyramid, Face). `None` for
    /// everything else (trim, hatches, wall regions and the primitive solids).
    pub fn vertices(&self, r: DetailRef) -> Option<Vec<Point>> {
        match r {
            DetailRef::Region(i) => self
                .region(i)
                .filter(|x| x.is_floor())
                .map(|x| x.outline.clone()),
            DetailRef::Deck(i) => self.deck(i).map(|x| x.outline.clone()),
            DetailRef::Molding(i) => self.molding(i).map(|x| x.polyline.clone()),
            DetailRef::Solid(i) => self
                .solid(i)
                .filter(|x| x.has_outline())
                .map(|x| x.footprint()),
            _ => None,
        }
    }

    /// Moves corner `i` (an index of [`vertices`](Self::vertices)) to the plan
    /// point `to`. Returns whether it moved.
    pub fn move_vertex(&mut self, r: DetailRef, i: usize, to: Point) -> bool {
        fn set(pts: &mut [Point], i: usize, to: Point) -> bool {
            pts.get_mut(i).map(|p| *p = to).is_some()
        }
        match r {
            DetailRef::Region(id) => self
                .region_mut(id)
                .filter(|x| x.is_floor())
                .is_some_and(|x| set(&mut x.outline, i, to)),
            DetailRef::Deck(id) => self
                .deck_mut(id)
                .is_some_and(|x| set(&mut x.outline, i, to)),
            DetailRef::Molding(id) => self
                .molding_mut(id)
                .is_some_and(|x| set(&mut x.polyline, i, to)),
            DetailRef::Solid(id) => self
                .solid_mut(id)
                .is_some_and(|x| x.move_outline_vertex(i, to)),
            _ => false,
        }
    }

    // ----- following walls -----

    /// Drops the wall material regions and wall hatches whose wall is not on
    /// `floor` any more. Returns how many went.
    pub fn drop_orphans(&mut self, floor: &Floor) -> usize {
        let n = self.regions.len() + self.hatches.len();
        self.regions
            .retain(|r| r.wall_id().is_none_or(|w| floor.wall(w).is_some()));
        self.hatches.retain(|h| floor.wall(h.wall_id).is_some());
        n - self.regions.len() - self.hatches.len()
    }

    /// Makes corner boards and quoins follow their walls after the walls moved
    /// from `before` to `after`: the apex slides to where the two outer faces
    /// meet now and the axes turn with the walls. (Wall material regions and
    /// hatches are measured along their wall, so they follow on their own.)
    /// Returns whether anything changed.
    pub fn follow_walls(&mut self, before: &[Wall], after: &[Wall]) -> bool {
        let mut changed = false;
        for b in &mut self.corner_boards {
            changed |= follow_corner(&mut b.wall_corner, &mut b.axes, before, after);
        }
        for q in &mut self.quoins {
            changed |= follow_corner(&mut q.corner, &mut q.axes, before, after);
        }
        changed
    }

    // ----- auto placement -----

    /// Auto Place Corner Boards: a board on every convex exterior corner of
    /// `floor` that has none yet. `alloc` hands out ids. Returns how many
    /// were added.
    pub fn auto_corner_boards(
        &mut self,
        floor: &Floor,
        rooms: &[Room],
        alloc: &mut dyn FnMut() -> Id,
    ) -> usize {
        let mut n = 0;
        for c in exterior_corners(floor, rooms).iter().filter(|c| c.convex) {
            if self
                .corner_boards
                .iter()
                .any(|b| b.wall_corner.dist(c.apex) < SAME_CORNER)
            {
                continue;
            }
            self.corner_boards.push(CornerBoard::at(alloc(), c));
            n += 1;
        }
        n
    }

    /// Auto Place Quoins: a stack on every convex exterior corner that has
    /// none yet. Returns how many were added.
    pub fn auto_quoins(
        &mut self,
        floor: &Floor,
        rooms: &[Room],
        alloc: &mut dyn FnMut() -> Id,
    ) -> usize {
        let mut n = 0;
        for c in exterior_corners(floor, rooms).iter().filter(|c| c.convex) {
            if self
                .quoins
                .iter()
                .any(|q| q.corner.dist(c.apex) < SAME_CORNER)
            {
                continue;
            }
            self.quoins.push(Quoin::at(alloc(), c));
            n += 1;
        }
        n
    }
}

impl Floor {
    /// The floor's details data as a typed object (`None` when unset).
    pub fn details_as<T: DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        self.details
            .as_ref()
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
    }

    /// Replace the floor's details data.
    pub fn set_details<T: Serialize>(&mut self, value: &T) -> Result<(), serde_json::Error> {
        self.details = Some(serde_json::to_value(value)?);
        Ok(())
    }
}

/// Convenience: the details layer of floor `fi` of `project`.
pub fn load_floor(project: &Project, fi: usize) -> DetailsLayer {
    project
        .floors
        .get(fi)
        .map(DetailsLayer::load)
        .unwrap_or_default()
}

// ===================================================================
// Helpers
// ===================================================================

/// Bounding box `(lo, hi)` of a point set (zero for an empty set).
pub fn bounds(pts: &[Point]) -> (Point, Point) {
    let mut lo = Point::new(f64::MAX, f64::MAX);
    let mut hi = Point::new(f64::MIN, f64::MIN);
    for p in pts {
        lo.x = lo.x.min(p.x);
        lo.y = lo.y.min(p.y);
        hi.x = hi.x.max(p.x);
        hi.y = hi.y.max(p.y);
    }
    if pts.is_empty() {
        (Point::ZERO, Point::ZERO)
    } else {
        (lo, hi)
    }
}

/// A wall whose outer face carries the corner at `apex` along direction `dir`:
/// a straight exterior wall parallel to `dir` with the apex on its face.
fn corner_wall(walls: &[Wall], apex: Point, dir: Point) -> Option<&Wall> {
    walls
        .iter()
        .filter(|w| {
            w.kind == WallKind::Exterior
                && !w.flags.invisible
                && !w.is_curved()
                && matches!(w.class, WallClass::Standard | WallClass::Foundation)
                && w.length() > 1e-6
                && w.direction().cross(dir).abs() < 0.05
                && ((apex - w.start).dot(w.normal()).abs() - w.thickness * 0.5).abs() <= 0.6
        })
        .map(|w| (dist_to_segment(apex, w.start, w.end), w))
        .filter(|(d, w)| *d <= w.thickness + 0.6)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, w)| w)
}

/// Moves one trim corner to where the faces of its two walls meet after the
/// walls changed from `before` to `after` (see [`DetailsLayer::follow_walls`]).
fn follow_corner(apex: &mut Point, axes: &mut CornerAxes, before: &[Wall], after: &[Wall]) -> bool {
    let (Some(wa), Some(wb)) = (
        corner_wall(before, *apex, axes.dir_a),
        corner_wall(before, *apex, axes.dir_b),
    ) else {
        return false;
    };
    if wa.id == wb.id {
        return false;
    }
    let (Some(na), Some(nb)) = (
        after.iter().find(|w| w.id == wa.id),
        after.iter().find(|w| w.id == wb.id),
    ) else {
        return false;
    };
    // The face line of a wall: through the centerline offset to the side the
    // apex was on.
    let face = |old: &Wall, new: &Wall| {
        let side = (*apex - old.start).dot(old.normal()).signum();
        (
            new.start + new.normal() * (side * new.thickness * 0.5),
            new.direction(),
        )
    };
    let ((pa, da), (pb, db)) = (face(wa, na), face(wb, nb));
    let det = da.cross(db);
    if det.abs() < 0.02 || na.length() < 1e-6 || nb.length() < 1e-6 {
        return false;
    }
    let new_apex = pa + da * ((pb - pa).cross(db) / det);
    let turn = |new_dir: Point, old_dir: Point, new_n: Point, old_out: Point| {
        (
            if new_dir.dot(old_dir) >= 0.0 {
                new_dir
            } else {
                -new_dir
            },
            if new_n.dot(old_out) >= 0.0 {
                new_n
            } else {
                -new_n
            },
        )
    };
    let (dir_a, out_a) = turn(da, axes.dir_a, na.normal(), axes.out_a);
    let (dir_b, out_b) = turn(db, axes.dir_b, nb.normal(), axes.out_b);
    let new_axes = CornerAxes {
        dir_a,
        dir_b,
        out_a,
        out_b,
    };
    let moved = new_apex.dist(*apex) > 1e-9;
    let turned = [
        new_axes.dir_a.dist(axes.dir_a),
        new_axes.dir_b.dist(axes.dir_b),
        new_axes.out_a.dist(axes.out_a),
        new_axes.out_b.dist(axes.out_b),
    ]
    .iter()
    .any(|d| *d > 1e-9);
    if moved {
        *apex = new_apex;
    }
    if turned {
        *axes = new_axes;
    }
    moved || turned
}

fn translate_all(pts: &mut [Point], d: Point) {
    for p in pts {
        *p = *p + d;
    }
}

/// Is `p` inside the outline (the deck, region or footprint)?
pub fn contains(outline: &[Point], p: Point) -> bool {
    outline.len() >= 3 && point_in_polygon(p, outline)
}

/// Centroid of an outline (zero for an empty one).
pub fn centroid(outline: &[Point]) -> Point {
    if outline.len() < 3 {
        outline.first().copied().unwrap_or(Point::ZERO)
    } else {
        polygon_centroid(outline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;
    use crate::rooms::detect_rooms;

    fn box_project() -> Project {
        let mut p = Project::new("t");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, 108.0, WallKind::Exterior);
        }
        p
    }

    fn alloc_from(project: &mut Project) -> impl FnMut() -> Id + '_ {
        move || project.alloc_id()
    }

    #[test]
    fn the_slot_round_trips_and_an_empty_layer_clears_it() {
        let mut p = box_project();
        let mut l = DetailsLayer::default();
        l.corner_boards.push(CornerBoard {
            id: 5,
            ..CornerBoard::default()
        });
        l.quoins.push(Quoin {
            id: 6,
            alternating: false,
            ..Quoin::default()
        });
        l.moldings.push(MoldingLine::new(
            7,
            vec![Point::new(0.0, 0.0), Point::new(96.0, 0.0)],
            MoldingProfile::Custom(vec![
                Point::new(0.0, 0.0),
                Point::new(2.0, 0.0),
                Point::new(0.0, 3.0),
            ]),
            109.0,
        ));
        l.regions.push(MaterialRegion::wall(
            8,
            1,
            Side::Right,
            12.0,
            60.0,
            0.0,
            48.0,
        ));
        l.regions.push(MaterialRegion::floor(
            9,
            vec![Point::ZERO, Point::new(10.0, 0.0), Point::new(0.0, 10.0)],
        ));
        l.hatches.push(WallHatch {
            id: 10,
            wall_id: 1,
            ..WallHatch::default()
        });
        l.decks.push(DeckPolygon::new(
            11,
            vec![Point::ZERO, Point::new(96.0, 0.0), Point::new(96.0, 96.0)],
        ));
        l.solids.push(Solid3d::new(
            12,
            SolidKind::Sphere { r: 12.0 },
            Point::new(5.0, 5.0),
        ));
        l.store(&mut p.floors[0]);
        assert!(p.floors[0].details.is_some());
        // Through the project JSON and back.
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(DetailsLayer::load(&back.floors[0]), l);
        assert_eq!(l.len(), 8);
        DetailsLayer::default().store(&mut p.floors[0]);
        assert!(p.floors[0].details.is_none());
        // Old files without the slot load as empty.
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v["floors"][0].as_object_mut().unwrap().remove("details");
        let old: Project = serde_json::from_value(v).unwrap();
        assert!(DetailsLayer::load(&old.floors[0]).is_empty());
    }

    #[test]
    fn a_rectangle_has_four_convex_exterior_corners_with_outer_apexes() {
        let p = box_project();
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let corners = exterior_corners(&p.floors[0], &rooms);
        assert_eq!(corners.len(), 4);
        assert!(corners.iter().all(|c| c.convex));
        // The apex is the outer face corner: half a wall thickness outside.
        let h = 6.5 * 0.5;
        for want in [
            Point::new(-h, -h),
            Point::new(240.0 + h, -h),
            Point::new(240.0 + h, 180.0 + h),
            Point::new(-h, 180.0 + h),
        ] {
            assert!(
                corners.iter().any(|c| c.apex.dist(want) < 1e-6),
                "no corner at {want:?}"
            );
        }
        for c in &corners {
            assert!((c.height - 108.0).abs() < 1e-9);
            // The outward normals point away from the building.
            let centre = Point::new(120.0, 90.0);
            assert!((centre - c.apex).dot(c.axes.out_a) < 0.0);
            assert!((centre - c.apex).dot(c.axes.out_b) < 0.0);
        }
    }

    #[test]
    fn auto_corner_boards_on_a_rectangle_yield_four_and_do_not_repeat() {
        let mut p = box_project();
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let floor = p.floors[0].clone();
        let mut layer = DetailsLayer::default();
        let n = layer.auto_corner_boards(&floor, &rooms, &mut alloc_from(&mut p));
        assert_eq!(n, 4);
        assert_eq!(layer.corner_boards.len(), 4);
        let b = &layer.corner_boards[0];
        assert_eq!((b.width, b.thickness, b.height), (3.5, 0.75, 108.0));
        // The L outline lies outside the walls.
        for q in b.outline() {
            let inside = point_in_polygon(q, &rooms[0].polygon);
            assert!(!inside, "{q:?} is inside the building");
        }
        // Placing again adds none.
        let again = layer.auto_corner_boards(&floor, &rooms, &mut alloc_from(&mut p));
        assert_eq!(again, 0);
        assert_eq!(layer.corner_boards.len(), 4);
        // Ids are distinct.
        let mut ids: Vec<Id> = layer.corner_boards.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn an_l_shaped_building_gets_boards_on_its_convex_corners_only() {
        let mut p = Project::new("l");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 100.0),
            Point::new(120.0, 100.0),
            Point::new(120.0, 200.0),
            Point::new(0.0, 200.0),
        ];
        for i in 0..6 {
            p.add_wall(0, c[i], c[(i + 1) % 6], 6.5, 108.0, WallKind::Exterior);
        }
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let all = exterior_corners(&p.floors[0], &rooms);
        assert_eq!(all.len(), 6);
        assert_eq!(all.iter().filter(|c| !c.convex).count(), 1);
        let floor = p.floors[0].clone();
        let mut layer = DetailsLayer::default();
        assert_eq!(
            layer.auto_corner_boards(&floor, &rooms, &mut alloc_from(&mut p)),
            5
        );
    }

    #[test]
    fn auto_quoins_alternate_long_and_short_blocks() {
        let mut p = box_project();
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let floor = p.floors[0].clone();
        let mut layer = DetailsLayer::default();
        assert_eq!(
            layer.auto_quoins(&floor, &rooms, &mut alloc_from(&mut p)),
            4
        );
        let q = &layer.quoins[0];
        assert!(q.alternating);
        assert_eq!((q.width, q.height, q.depth), (16.0, 8.0, 1.5));
        // 108" / 8" = 13 courses.
        assert_eq!(q.courses(), 13);
        let (a0, b0) = q.course_lengths(0);
        let (a1, b1) = q.course_lengths(1);
        assert_eq!((a0, b0), (16.0, 16.0 * QUOIN_SHORT_RATIO));
        assert_eq!((a1, b1), (b0, a0));
        assert_eq!(q.course_lengths(2), (a0, b0));
        let flat = Quoin {
            alternating: false,
            ..q.clone()
        };
        assert_eq!(flat.course_lengths(1), flat.course_lengths(0));
        assert!((q.course_base(3) - 24.0).abs() < 1e-9);
    }

    #[test]
    fn corner_near_finds_the_clicked_corner() {
        let p = box_project();
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let corners = exterior_corners(&p.floors[0], &rooms);
        let hit = corner_near(&corners, Point::new(238.0, 2.0), 8.0).unwrap();
        assert!(hit.vertex.dist(Point::new(240.0, 0.0)) < 1e-6);
        assert!(corner_near(&corners, Point::new(120.0, 90.0), 8.0).is_none());
    }

    #[test]
    fn molding_sections_and_lengths() {
        let m = MoldingLine::new(
            1,
            vec![
                Point::new(0.0, 0.0),
                Point::new(96.0, 0.0),
                Point::new(96.0, 48.0),
            ],
            MoldingProfile::Crown,
            109.125,
        );
        assert_eq!(m.length(), 144.0);
        assert_eq!(m.section().len(), 4);
        assert!((m.elevation - (109.125 - 4.5)).abs() < 1e-9);
        assert!((m.volume() - 4.5 * 3.5 * 144.0).abs() < 1e-6);
        let base = MoldingLine::new(2, vec![], MoldingProfile::Base, 109.0);
        assert_eq!(base.elevation, 0.0);
        let custom = MoldingLine {
            profile: MoldingProfile::Custom(vec![
                Point::new(0.0, 0.0),
                Point::new(2.0, 0.0),
                Point::new(0.0, 3.0),
            ]),
            ..MoldingLine::default()
        };
        assert_eq!(custom.section().len(), 3);
    }

    #[test]
    fn deck_polygon_volume_and_perimeter() {
        let d = DeckPolygon::new(
            1,
            vec![
                Point::new(0.0, 0.0),
                Point::new(144.0, 0.0),
                Point::new(144.0, 96.0),
                Point::new(0.0, 96.0),
            ],
        );
        assert_eq!(d.area(), 144.0 * 96.0);
        assert_eq!(d.perimeter(), 480.0);
        assert!((d.volume() - 144.0 * 96.0 * 1.5).abs() < 1e-6);
    }

    #[test]
    fn solid_volumes_and_footprints() {
        let b = Solid3d::new(
            1,
            SolidKind::Box {
                w: 10.0,
                d: 20.0,
                h: 30.0,
            },
            Point::new(100.0, 100.0),
        );
        assert_eq!(b.volume(), 6000.0);
        let fp = b.footprint();
        assert!(fp
            .iter()
            .all(|p| (p.x - 100.0).abs() <= 5.0 && (p.y - 100.0).abs() <= 10.0));
        let rotated = Solid3d {
            rotation: 90.0,
            ..b.clone()
        };
        let fp = rotated.footprint();
        assert!(fp
            .iter()
            .all(|p| (p.x - 100.0).abs() <= 10.0 + 1e-9 && (p.y - 100.0).abs() <= 5.0 + 1e-9));
        let s = Solid3d::new(2, SolidKind::Sphere { r: 12.0 }, Point::ZERO);
        assert!((s.volume() - 4.0 / 3.0 * PI * 1728.0).abs() < 1e-6);
        assert_eq!(s.height(), 24.0);
        assert_eq!(s.footprint().len(), SOLID_SEGMENTS);
        let face = Solid3d::new(
            3,
            SolidKind::Face {
                polygon: vec![Point::ZERO, Point::new(1.0, 0.0), Point::new(0.0, 1.0)],
            },
            Point::ZERO,
        );
        assert_eq!(face.volume(), 0.0);
    }

    #[test]
    fn find_remove_and_translate() {
        let mut l = DetailsLayer::default();
        l.decks.push(DeckPolygon::new(
            3,
            vec![Point::ZERO, Point::new(10.0, 0.0), Point::new(0.0, 10.0)],
        ));
        l.solids
            .push(Solid3d::new(4, SolidKind::Sphere { r: 3.0 }, Point::ZERO));
        assert_eq!(l.find(3), Some(DetailRef::Deck(3)));
        assert_eq!(l.find(4), Some(DetailRef::Solid(4)));
        assert_eq!(l.find(9), None);
        assert!(l.translate(DetailRef::Solid(4), Point::new(5.0, 0.0)));
        assert_eq!(l.solid(4).unwrap().position, Point::new(5.0, 0.0));
        assert_eq!(l.layer_of(DetailRef::Deck(3)).as_deref(), Some(DECK_LAYER));
        assert!(l.remove(DetailRef::Deck(3)));
        assert!(!l.remove(DetailRef::Deck(3)));
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn wall_region_geometry_follows_the_wall() {
        let p = box_project();
        let floor = &p.floors[0];
        let w = &floor.walls[0];
        let r = MaterialRegion::wall(1, w.id, Side::Left, 24.0, 72.0, 0.0, 48.0);
        assert_eq!(r.uv_bounds(), Some((24.0, 72.0, 0.0, 48.0)));
        assert_eq!(r.area(), 48.0 * 48.0);
        let strip = r.plan_polygon(floor).unwrap();
        assert_eq!(strip.len(), 4);
        let (lo, hi) = bounds(&strip);
        assert!((lo.x - 24.0).abs() < 1e-9 && (hi.x - 72.0).abs() < 1e-9);
        assert!((hi.y - lo.y - 6.5).abs() < 1e-9);
    }
    #[test]
    fn orphans_are_wall_regions_and_hatches_of_missing_walls() {
        let mut p = box_project();
        let (a, b) = (p.floors[0].walls[0].id, p.floors[0].walls[1].id);
        let mut l = DetailsLayer::default();
        l.regions
            .push(MaterialRegion::wall(1, a, Side::Left, 0.0, 40.0, 0.0, 40.0));
        l.regions
            .push(MaterialRegion::wall(2, b, Side::Left, 0.0, 40.0, 0.0, 40.0));
        l.regions
            .push(MaterialRegion::floor(3, vec![Point::ZERO; 3]));
        l.hatches.push(WallHatch {
            id: 4,
            wall_id: a,
            ..WallHatch::default()
        });
        assert_eq!(l.drop_orphans(&p.floors[0]), 0);
        p.remove_wall(0, a);
        assert_eq!(l.drop_orphans(&p.floors[0]), 2);
        assert_eq!(
            l.regions.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert!(l.hatches.is_empty());
    }

    #[test]
    fn corners_follow_their_walls_when_one_moves() {
        let mut p = box_project();
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let mut l = DetailsLayer::default();
        let mut next = 100;
        let mut alloc = || {
            next += 1;
            next
        };
        assert_eq!(l.auto_corner_boards(&p.floors[0], &rooms, &mut alloc), 4);
        assert_eq!(l.auto_quoins(&p.floors[0], &rooms, &mut alloc), 4);
        let before = p.floors[0].walls.clone();
        // Nothing moved: nothing follows.
        assert!(!l.follow_walls(&before, &before));
        // Move the north wall (y = 180) down 30" the way a perpendicular
        // move does: its two neighbours shrink to meet it.
        let ids: Vec<Id> = before.iter().map(|w| w.id).collect();
        let mut after = before.clone();
        for w in &mut after {
            if w.id == ids[2] {
                w.start.y = 150.0;
                w.end.y = 150.0;
            }
            if w.id == ids[1] {
                w.end.y = 150.0;
            }
            if w.id == ids[3] {
                w.start.y = 150.0;
            }
        }
        assert!(l.follow_walls(&before, &after));
        // The two north corners moved 30" south; the south ones did not.
        let ys: Vec<f64> = l.corner_boards.iter().map(|b| b.wall_corner.y).collect();
        assert_eq!(
            ys.iter()
                .filter(|y| (**y - 180.0 + 30.0 - 3.25).abs() < 1e-6)
                .count(),
            2,
            "{ys:?}"
        );
        assert_eq!(
            ys.iter().filter(|y| (**y + 3.25).abs() < 1e-6).count(),
            2,
            "{ys:?}"
        );
        let qs: Vec<f64> = l.quoins.iter().map(|q| q.corner.y).collect();
        assert_eq!(
            qs.iter()
                .filter(|y| (**y - 150.0 - 3.25).abs() < 1e-6)
                .count(),
            2,
            "{qs:?}"
        );
        // The boards still hug the new faces: their outlines are unchanged in
        // shape, so a second follow with the same walls changes nothing.
        assert!(!l.follow_walls(&before, &after));
        // A moved wall that carries a wall region leaves the region measured
        // along it.
        let r = MaterialRegion::wall(1, ids[2], Side::Left, 24.0, 72.0, 0.0, 48.0);
        p.floors[0].walls = after;
        let strip = r.plan_polygon(&p.floors[0]).unwrap();
        assert!(strip.iter().all(|q| (q.y - 150.0).abs() <= 3.25 + 1e-9));
    }

    #[test]
    fn rotated_walls_turn_the_corner_axes() {
        let p = box_project();
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let mut l = DetailsLayer::default();
        let mut next = 0;
        l.auto_corner_boards(&p.floors[0], &rooms, &mut || {
            next += 1;
            next
        });
        let before = p.floors[0].walls.clone();
        let mut after = before.clone();
        // Stretch the east wall's north end 30" east: the north wall tilts and
        // the east wall leans.
        let east = before[1].id;
        for w in &mut after {
            if w.id == east {
                w.end.x = 270.0;
            }
            if w.id == before[2].id {
                w.start.x = 270.0;
            }
        }
        l.follow_walls(&before, &after);
        let ne = l
            .corner_boards
            .iter()
            .find(|b| b.wall_corner.x > 250.0 && b.wall_corner.y > 100.0)
            .expect("the north-east board followed");
        let (a, b) = (ne.axes.dir_a, ne.axes.dir_b);
        // Unit axes, still pointing along the faces away from the apex.
        assert!((a.length() - 1.0).abs() < 1e-9 && (b.length() - 1.0).abs() < 1e-9);
        assert!(a.cross(b).abs() > 0.5);
        assert!(ne.axes.out_a.dot(a).abs() < 1e-9 && ne.axes.out_b.dot(b).abs() < 1e-9);
    }

    #[test]
    fn corners_of_regions_decks_moldings_and_outline_solids_can_be_moved() {
        let mut l = DetailsLayer::default();
        let sq = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        l.decks.push(DeckPolygon::new(1, sq.clone()));
        l.regions.push(MaterialRegion::floor(2, sq.clone()));
        l.regions.push(MaterialRegion::wall(
            3,
            99,
            Side::Left,
            0.0,
            10.0,
            0.0,
            10.0,
        ));
        l.moldings.push(MoldingLine::new(
            4,
            vec![Point::ZERO, Point::new(50.0, 0.0)],
            MoldingProfile::Base,
            96.0,
        ));
        let mut s = Solid3d::new(
            5,
            SolidKind::PolylineSolid {
                outline: sq.clone(),
                h: 12.0,
            },
            Point::new(100.0, 100.0),
        );
        s.rotation = 90.0;
        l.solids.push(s);
        l.solids
            .push(Solid3d::new(6, SolidKind::Sphere { r: 5.0 }, Point::ZERO));
        assert_eq!(l.vertices(DetailRef::Deck(1)).unwrap().len(), 4);
        assert_eq!(l.vertices(DetailRef::Region(2)).unwrap().len(), 4);
        assert!(l.vertices(DetailRef::Region(3)).is_none());
        assert_eq!(l.vertices(DetailRef::Molding(4)).unwrap().len(), 2);
        assert!(l.vertices(DetailRef::Solid(6)).is_none());
        assert!(l.move_vertex(DetailRef::Deck(1), 2, Point::new(20.0, 20.0)));
        assert_eq!(l.deck(1).unwrap().outline[2], Point::new(20.0, 20.0));
        assert!(l.move_vertex(DetailRef::Molding(4), 1, Point::new(60.0, 5.0)));
        assert!(!l.move_vertex(DetailRef::Molding(4), 2, Point::ZERO));
        assert!(!l.move_vertex(DetailRef::Region(3), 0, Point::ZERO));
        assert!(!l.move_vertex(DetailRef::Solid(6), 0, Point::ZERO));
        // A rotated outline solid takes plan coordinates.
        let target = Point::new(90.0, 130.0);
        assert!(l.move_vertex(DetailRef::Solid(5), 1, target));
        let foot = l.solid(5).unwrap().footprint();
        assert!(foot[1].dist(target) < 1e-9, "{:?}", foot[1]);
    }
}
