//! Wall extensions for Chief parity (`docs/parity/walls.md`, `roofs.md`).
//!
//! * W-21..W-24: exterior/interior side ([`Side`]) and the wall option flags
//!   ([`WallFlags`]).
//! * W-26..W-30: the reference line a thickness change holds fixed
//!   ([`ResizeAbout`]) and [`Project::set_wall_thickness_about`] /
//!   [`Project::set_wall_type`].
//! * W-46..W-49: the wall-type registry on the project and the per-wall
//!   `wall_type` name.
//! * W-52..W-56: foundation, pony, half, divider and railing walls (flags).
//! * W-64..W-68: curved walls ([`WallCurve`]).
//! * RF-18..RF-25: per-wall roof directives ([`WallRoofDirective`]).
//! * Editing: split/break, join collinear walls and junction queries.

use crate::defaults::{RoofWallKind, WallTypeDef};
use crate::geometry::{project_on_segment, Point};
use crate::joins::{self, wall_layer_bands};
use crate::model::{Id, Project, Wall, WallEnd, WallKind, DEFAULT_CEILING_HEIGHT};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[path = "wall_intersect.rs"]
pub mod intersect;
#[path = "wall_profile.rs"]
pub mod profile;
#[path = "wall_reset.rs"]
pub mod reset;
#[path = "wall_spec.rs"]
pub mod spec;
#[path = "wall_spec_tabs.rs"]
pub mod spec_tabs;
pub use spec::{
    default_cap_profiles, min_thickness, platform_adjust, CapPosition, CapProfile, CeilingPlatform,
    DoubleWall, FloorPlatform, PlatformAdjust, PlatformContext, WallBox, WallCap, WallFoundation,
    WallSpec, WallStructure,
};
pub use spec_tabs::{
    drawing_group_name, openings_area, wall_components, BalusterStyle, BandKind, CoveringBand,
    CoveringSide, NewelStyle, RailFill, RailProfile, SideCovering, WallComponent, WallCovering,
    WallInfo, WallMaterials, WallPaint, WallRailing, WallScheduleInfo, DEFAULT_DRAWING_GROUP,
    DRAWING_GROUPS,
};

/// Smallest allowed wall thickness, inches (W-30).
pub const MIN_WALL_THICKNESS: f64 = 0.125;
/// Chief's default facet angle for curved walls, degrees (W-65).
pub const DEFAULT_FACET_ANGLE_DEG: f64 = 7.5;
/// Tolerance used by the editing helpers when matching wall ends, inches.
const EDIT_TOL: f64 = 0.5;

/// One side of a wall, looking from start to end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Side {
    /// The +normal side (counter-clockwise from the direction).
    #[default]
    Left,
    Right,
}

impl Side {
    /// `+1` for [`Side::Left`] (along `Wall::normal`), `-1` for [`Side::Right`].
    pub fn sign(self) -> f64 {
        match self {
            Side::Left => 1.0,
            Side::Right => -1.0,
        }
    }
    pub fn opposite(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

/// Which reference a thickness or wall-type change holds fixed (W-26, W-27).
/// The default (used for walls from old files) is `WallCenter`, which keeps
/// the previous "grow equally on both sides" behaviour;
/// [`ResizeAbout::default_for`] gives Chief's per-kind default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ResizeAbout {
    /// The exterior side of the main (structural) layer.
    MainLayerOutside,
    /// The interior side of the main layer.
    MainLayerInside,
    #[default]
    WallCenter,
    /// The exterior face of the wall.
    OuterSurface,
    /// The interior face of the wall.
    InnerSurface,
}

impl ResizeAbout {
    /// Chief's default: main layer outside for exterior walls, wall center
    /// for interior walls.
    pub fn default_for(kind: WallKind) -> ResizeAbout {
        match kind {
            WallKind::Exterior => ResizeAbout::MainLayerOutside,
            WallKind::Interior => ResizeAbout::WallCenter,
        }
    }
}

/// Pony wall: the lower part uses another wall type up to `lower_height` (W-53).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PonyWall {
    pub lower_type: String,
    /// Elevation of the lower wall top, inches.
    pub lower_height: f64,
}

/// Fence look of a [`WallClass::Fencing`] wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FenceStyle {
    /// Pickets on two rails.
    #[default]
    Picket,
    /// Solid boards.
    Privacy,
    /// Posts and open rails.
    Rail,
}

/// Default half-wall top height, inches.
pub const DEFAULT_HALF_WALL_HEIGHT: f64 = 36.0;
/// Default elevation of the pony wall split, inches.
pub const DEFAULT_PONY_SPLIT: f64 = 36.0;
/// Default height of a foundation wall below its floor, inches.
pub const DEFAULT_FOUNDATION_HEIGHT: f64 = 48.0;
/// Layer of room dividers.
pub const ROOM_DIVIDER_LAYER: &str = "Walls, Invisible";
/// Layer of deck railings and deck edges.
pub const DECK_RAILING_LAYER: &str = "Deck Railing";
/// Layer of fencing.
pub const FENCING_LAYER: &str = "Fencing";

/// Which flyout wall a [`Wall`] is (W-52..W-58, R-3..R-5). `Standard` is an
/// ordinary exterior or interior wall (its [`WallKind`]); the others are the
/// Straight/Curved Foundation, Pony, Glass, Glass Pony, Half-Wall, Room
/// Divider, Railing, Deck Railing, Deck Edge and Fencing tools. A curved
/// variant is the same class with `Wall::curve` set.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum WallClass {
    #[default]
    Standard,
    Foundation,
    /// Upper part of `upper_type`, lower part of `lower_type`, split at
    /// `split_height`. `upper_sets_plan_display` picks which type's layers
    /// the plan draws.
    Pony {
        upper_type: String,
        lower_type: String,
        split_height: f64,
        upper_sets_plan_display: bool,
    },
    Glass,
    /// A solid `lower_type` wall up to `split_height`, glass above.
    GlassPony {
        lower_type: String,
        split_height: f64,
    },
    /// Top lowered to `height`.
    HalfWall {
        height: f64,
    },
    /// Invisible, zero thickness in 3D; only closes rooms.
    RoomDivider,
    Railing,
    DeckRailing,
    DeckEdge,
    Fencing {
        style: FenceStyle,
    },
}

impl WallClass {
    pub fn is_standard(&self) -> bool {
        matches!(self, WallClass::Standard)
    }

    /// Short name for the Wall Specification dialog.
    pub fn label(&self) -> &'static str {
        match self {
            WallClass::Standard => "Standard",
            WallClass::Foundation => "Foundation",
            WallClass::Pony { .. } => "Pony Wall",
            WallClass::Glass => "Glass Wall",
            WallClass::GlassPony { .. } => "Glass Pony Wall",
            WallClass::HalfWall { .. } => "Half-Wall",
            WallClass::RoomDivider => "Room Divider",
            WallClass::Railing => "Railing",
            WallClass::DeckRailing => "Deck Railing",
            WallClass::DeckEdge => "Deck Edge",
            WallClass::Fencing { .. } => "Fencing",
        }
    }

    /// The layer walls of this class are drawn on (`None` = the wall's own).
    pub fn default_layer(&self) -> Option<&'static str> {
        match self {
            WallClass::RoomDivider => Some(ROOM_DIVIDER_LAYER),
            WallClass::DeckRailing | WallClass::DeckEdge => Some(DECK_RAILING_LAYER),
            WallClass::Fencing { .. } => Some(FENCING_LAYER),
            _ => None,
        }
    }

    /// Railing-like classes draw posts and rails instead of a solid wall.
    pub fn is_railing(&self) -> bool {
        matches!(self, WallClass::Railing | WallClass::DeckRailing)
    }
}

/// Wall options and variants (W-24, W-52..W-58, R-3..R-5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WallFlags {
    /// Not drawn in plan or 3D.
    pub invisible: bool,
    /// Never defines a room boundary.
    pub no_room_definition: bool,
    /// Dimensions skip this wall.
    pub no_locate: bool,
    /// Invisible zero-height wall whose only job is to close a room (R-4).
    pub room_divider: bool,
    pub railing: bool,
    /// Lowered top, typically 42" (W-54).
    pub half_wall: bool,
    pub pony: Option<PonyWall>,
    pub foundation: bool,
    pub attic: bool,
    /// Generated by the program, not drawn (the invisible walls between
    /// platforms, W-25, W-63); regenerated rather than edited.
    pub auto_generated: bool,
    /// The off-angle icon was dismissed with Ignore / Ignore All (W-132).
    pub ignore_off_angle: bool,
    /// The unconnected-wall Caution symbol was dismissed (W-132).
    pub ignore_unconnected: bool,
    /// Auto Connect lock on the start end: it never snaps to other walls
    /// (W-135).
    pub lock_start: bool,
    /// Auto Connect lock on the end.
    pub lock_end: bool,
}

impl WallFlags {
    /// Whether the wall closes rooms. A room divider always does; otherwise
    /// `no_room_definition` and `invisible` walls are skipped (R-3..R-5).
    pub fn defines_rooms(&self) -> bool {
        self.room_divider || !(self.no_room_definition || self.invisible)
    }
}

/// A wall whose bottom is this far above the floor (or more) stands over
/// open floor space (a dormer wall on the roof deck, a clerestory wall) and
/// does not close a room at floor level.
pub const ROOM_BOUNDARY_MAX_BOTTOM: f64 = 48.0;

/// A curved wall stored as a true arc through `start`, the apex and `end`
/// (W-64..W-67). `bulge` is the signed sagitta: the distance from the chord
/// midpoint to the arc apex. Positive bulges toward the left (+normal) of
/// start-to-end. For curved walls, opening offsets are measured along the arc.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WallCurve {
    pub bulge: f64,
}

impl WallCurve {
    pub fn is_straight(&self) -> bool {
        self.bulge.abs() < 1e-9
    }

    /// Half the swept angle, radians, in `(0, PI]`.
    fn half_sweep(&self, chord: f64) -> f64 {
        2.0 * (2.0 * self.bulge.abs() / chord).atan()
    }

    /// Signed sweep from start to end, radians (negative = clockwise).
    pub fn sweep(&self, start: Point, end: Point) -> f64 {
        let chord = start.dist(end);
        if chord < 1e-9 || self.is_straight() {
            return 0.0;
        }
        -self.bulge.signum() * 2.0 * self.half_sweep(chord)
    }

    /// Arc center and radius, or `None` for a straight or degenerate wall.
    pub fn arc_center_radius(&self, start: Point, end: Point) -> Option<(Point, f64)> {
        let chord = start.dist(end);
        if chord < 1e-9 || self.is_straight() {
            return None;
        }
        let s = self.bulge;
        let r = (chord * chord / 4.0 + s * s) / (2.0 * s.abs());
        let n = end.sub(start).normalized().perp();
        let mid = Point::lerp(start, end, 0.5);
        Some((mid + n * (s - s.signum() * r), r))
    }

    /// Arc length, inches (the chord for a straight wall).
    pub fn arc_length(&self, start: Point, end: Point) -> f64 {
        match self.arc_center_radius(start, end) {
            Some((_, r)) => r * self.sweep(start, end).abs(),
            None => start.dist(end),
        }
    }

    /// `n + 1` points along the arc from start to end (`n` is clamped to at
    /// least 1). A straight wall returns evenly spaced chord points.
    pub fn sample_points(&self, start: Point, end: Point, n: usize) -> Vec<Point> {
        let n = n.max(1);
        let Some((c, r)) = self.arc_center_radius(start, end) else {
            return (0..=n)
                .map(|i| Point::lerp(start, end, i as f64 / n as f64))
                .collect();
        };
        let a0 = start.sub(c).angle();
        let sweep = self.sweep(start, end);
        (0..=n)
            .map(|i| {
                if i == 0 {
                    return start;
                }
                if i == n {
                    return end;
                }
                let a = a0 + sweep * i as f64 / n as f64;
                Point::new(c.x + r * a.cos(), c.y + r * a.sin())
            })
            .collect()
    }

    /// Number of facets for [`DEFAULT_FACET_ANGLE_DEG`] (at least 1).
    pub fn facet_count(&self, start: Point, end: Point) -> usize {
        let deg = self.sweep(start, end).abs() * 180.0 / PI;
        ((deg / DEFAULT_FACET_ANGLE_DEG).ceil() as usize).max(1)
    }

    /// Facets so no facet of the arc strays more than `max_sag` inches from
    /// the true curve: at least the [`DEFAULT_FACET_ANGLE_DEG`] count, more
    /// for a large radius drawn at a high zoom (capped at 720).
    pub fn facet_count_for_sag(&self, start: Point, end: Point, max_sag: f64) -> usize {
        let base = self.facet_count(start, end);
        let Some((_, r)) = self.arc_center_radius(start, end) else {
            return base;
        };
        if max_sag <= 0.0 || max_sag >= r {
            return base;
        }
        // A chord spanning angle `a` has a sagitta of r (1 - cos(a / 2)).
        let step = 2.0 * (1.0 - max_sag / r).acos();
        let need = (self.sweep(start, end).abs() / step.max(1e-6)).ceil() as usize;
        base.max(need).min(720)
    }

    /// The curve for the sub-arc between two points of this arc, given the
    /// sub-sweep (radians, magnitude) and the radius. Keeps the bulge sign.
    fn sub_curve(&self, radius: f64, sub_sweep: f64) -> WallCurve {
        WallCurve {
            bulge: self.bulge.signum() * radius * (1.0 - (sub_sweep.abs() * 0.5).cos()),
        }
    }
}

/// Largest half-sweep a handle drag or a tangent fit may produce, radians
/// (a 340 degree arc); beyond it the arc is nearly a full circle.
const MAX_HALF_SWEEP: f64 = 170.0 * PI / 180.0;

impl WallCurve {
    /// Radius over a chord of length `chord`; `None` for a straight curve.
    pub fn radius(&self, chord: f64) -> Option<f64> {
        if chord < 1e-9 || self.is_straight() {
            return None;
        }
        let s = self.bulge.abs();
        Some((chord * chord / 4.0 + s * s) / (2.0 * s))
    }

    /// Total swept angle, radians, unsigned (0 for a straight curve).
    pub fn sweep_abs(&self, chord: f64) -> f64 {
        if chord < 1e-9 || self.is_straight() {
            return 0.0;
        }
        2.0 * self.half_sweep(chord)
    }

    /// The arc of `radius` over a chord (the minor arc), bulging to the left
    /// when `left`. `None` when the radius cannot span the chord.
    pub fn from_radius(chord: f64, radius: f64, left: bool) -> Option<WallCurve> {
        let half = chord * 0.5;
        if chord < 1e-9 || radius + 1e-9 < half {
            return None;
        }
        let s = radius - (radius * radius - half * half).max(0.0).sqrt();
        Some(WallCurve {
            bulge: if left { s } else { -s },
        })
    }

    /// The arc sweeping `sweep` radians (0 to 360 degrees exclusive) over a
    /// chord, bulging to the left when `left`.
    pub fn from_sweep(chord: f64, sweep: f64, left: bool) -> Option<WallCurve> {
        let sweep = sweep.abs();
        if chord < 1e-9 || sweep <= 1e-9 || sweep >= 2.0 * MAX_HALF_SWEEP {
            return None;
        }
        let s = chord * 0.5 * (sweep * 0.25).tan();
        Some(WallCurve {
            bulge: if left { s } else { -s },
        })
    }

    /// Unit tangent at the start, pointing along the direction of travel.
    pub fn tangent_at_start(&self, start: Point, end: Point) -> Point {
        let d = end.sub(start).normalized();
        let chord = start.dist(end);
        if self.is_straight() || chord < 1e-9 {
            return d;
        }
        rotate(d, self.bulge.signum() * self.half_sweep(chord))
    }

    /// Unit tangent at the end, pointing along the direction of travel.
    pub fn tangent_at_end(&self, start: Point, end: Point) -> Point {
        let d = end.sub(start).normalized();
        let chord = start.dist(end);
        if self.is_straight() || chord < 1e-9 {
            return d;
        }
        rotate(d, -self.bulge.signum() * self.half_sweep(chord))
    }

    /// The curve over `start`..`end` that leaves `start` along `tangent`
    /// (Make Arc Tangent, W-68). `None` when the tangent points back past the
    /// chord (no arc of under 340 degrees fits). A tangent along the chord
    /// gives a straight curve.
    pub fn tangent_to(start: Point, end: Point, tangent: Point) -> Option<WallCurve> {
        let chord = start.dist(end);
        let u = tangent.normalized();
        if chord < 1e-9 || u.length() < 1e-9 {
            return None;
        }
        let d = end.sub(start).normalized();
        let cross = d.cross(u);
        let cos = d.dot(u).clamp(-1.0, 1.0);
        let phi = cross.abs().atan2(cos);
        if phi < 1e-6 {
            return Some(WallCurve { bulge: 0.0 });
        }
        if phi > MAX_HALF_SWEEP {
            return None;
        }
        // The path leaves on the side `u` lies on; the apex is on that side.
        let s = chord * 0.5 * (phi * 0.5).tan();
        Some(WallCurve {
            bulge: if cross > 0.0 { s } else { -s },
        })
    }
}

/// `v` turned counter-clockwise by `a` radians.
fn rotate(v: Point, a: f64) -> Point {
    let (s, c) = a.sin_cos();
    Point::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Chord, radius, arc length and sweep of a curved wall, as the Select tool
/// and the drawing status show them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcReadout {
    pub chord: f64,
    pub radius: f64,
    pub arc_length: f64,
    pub sweep_deg: f64,
}

/// Roof directive of an exterior wall (RF-18..RF-25).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallRoofDirective {
    pub kind: RoofWallKind,
    /// Rise per 12 of run; `None` follows the roof defaults.
    pub pitch_in_12: Option<f64>,
    /// Second pitch: `(rise in 12, starts at height above floor)` (RF-25).
    pub upper_pitch: Option<(f64, f64)>,
    pub overhang: Option<f64>,
    pub auto_roof_return: bool,
    /// Length of the roof returns of Auto Roof Return, inches; `None` is the
    /// app default.
    pub return_length: Option<f64>,
    /// How far Extend Slope Downward continues below the eave, inches;
    /// `None` is the app default.
    pub extend_drop: Option<f64>,
    /// Roof Cuts Wall at Bottom for this wall (manual p. 428); `None`
    /// follows Roof Defaults.
    pub cuts_wall_at_bottom: Option<bool>,
    /// Include Frieze: the frieze molding of Build Roof runs along this
    /// wall at the roof line (default on).
    pub include_frieze: bool,
    /// Include Automatic End Truss Above: an attic wall above gets a Reduced
    /// Gable End Truss when trusses are built. Checked for a Full Gable
    /// Wall (stored; the truss builder reads it).
    pub end_truss_above: bool,
    /// Combine with Above Wall: balloon framing with the attic wall above
    /// (stored; needs an attic wall above to be offered).
    pub combine_with_above: bool,
}

impl Default for WallRoofDirective {
    fn default() -> Self {
        Self {
            kind: RoofWallKind::Hip,
            pitch_in_12: None,
            upper_pitch: None,
            overhang: None,
            auto_roof_return: false,
            return_length: None,
            extend_drop: None,
            cuts_wall_at_bottom: None,
            include_frieze: true,
            end_truss_above: false,
            combine_with_above: false,
        }
    }
}

/// How another wall meets one end of a wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallConnection {
    pub other: Id,
    /// The end of the queried wall that is joined.
    pub at: WallEnd,
    pub kind: joins::ConnectionKind,
}

impl Wall {
    pub fn new(start: Point, end: Point, thickness: f64, height: f64, kind: WallKind) -> Wall {
        Wall {
            id: 0,
            start,
            end,
            thickness,
            height,
            kind,
            layer: crate::model::DEFAULT_WALL_LAYER.to_string(),
            flags: WallFlags::default(),
            wall_type: None,
            resize_about: ResizeAbout::default_for(kind),
            curve: None,
            roof: WallRoofDirective::default(),
            exterior_side: Side::Left,
            extras: crate::extras::WallExtras::default(),
            class: WallClass::Standard,
            foundation_height: DEFAULT_FOUNDATION_HEIGHT,
            is_deck_edge: false,
            bottom_offset: 0.0,
            spec: WallSpec::default(),
        }
    }

    /// Makes this wall `class`, keeping the legacy [`WallFlags`] other code
    /// reads (foundation, pony, half wall, room divider, railing) and the
    /// deck-edge marker in step.
    pub fn set_class(&mut self, class: WallClass) {
        match &self.class {
            WallClass::Foundation => self.flags.foundation = false,
            WallClass::Pony { .. } | WallClass::GlassPony { .. } => self.flags.pony = None,
            WallClass::HalfWall { .. } => self.flags.half_wall = false,
            WallClass::RoomDivider => {
                self.flags.room_divider = false;
                self.flags.invisible = false;
            }
            WallClass::Railing | WallClass::DeckRailing => self.flags.railing = false,
            WallClass::DeckEdge => self.is_deck_edge = false,
            _ => {}
        }
        match &class {
            WallClass::Foundation => self.flags.foundation = true,
            WallClass::Pony {
                lower_type,
                split_height,
                ..
            }
            | WallClass::GlassPony {
                lower_type,
                split_height,
            } => {
                self.flags.pony = Some(PonyWall {
                    lower_type: lower_type.clone(),
                    lower_height: *split_height,
                })
            }
            WallClass::HalfWall { .. } => self.flags.half_wall = true,
            WallClass::RoomDivider => {
                self.flags.room_divider = true;
                self.flags.invisible = true;
            }
            WallClass::Railing | WallClass::DeckRailing => self.flags.railing = true,
            WallClass::DeckEdge => self.is_deck_edge = true,
            _ => {}
        }
        self.class = class;
    }

    /// Does the wall close rooms on its floor? Its flags allow it (see
    /// [`WallFlags::defines_rooms`]) and it stands on the floor: a wall raised
    /// [`ROOM_BOUNDARY_MAX_BOTTOM`] or more by its `bottom_offset` does not.
    pub fn defines_rooms(&self) -> bool {
        self.flags.defines_rooms() && self.bottom_offset < ROOM_BOUNDARY_MAX_BOTTOM
    }

    /// A foundation wall by class or by the legacy flag.
    pub fn is_foundation(&self) -> bool {
        self.class == WallClass::Foundation || self.flags.foundation
    }

    /// Room dividers close rooms but are never built or drawn as walls.
    pub fn is_room_divider(&self) -> bool {
        self.class == WallClass::RoomDivider || self.flags.room_divider
    }

    /// The closed plan polygon of a curved wall: the left offset curve
    /// forward, then the right one back, faceted per the curve's facet angle.
    /// Straight walls give their footprint.
    pub fn plan_polygon(&self) -> Vec<Point> {
        let Some(c) = self.curve.filter(|c| !c.is_straight()) else {
            return self.footprint().to_vec();
        };
        self.plan_polygon_n(c.facet_count(self.start, self.end).max(2))
    }

    /// [`Wall::plan_polygon`] with `n` facets along the arc (drawing a large
    /// radius at a high zoom wants more than the facet angle gives). A
    /// straight wall gives its footprint.
    pub fn plan_polygon_n(&self, n: usize) -> Vec<Point> {
        if !self.is_curved() {
            return self.footprint().to_vec();
        }
        let n = n.max(2);
        let half = self.thickness * 0.5;
        let left = self.offset_curve(half, n);
        let right = self.offset_curve(-half, n);
        left.into_iter().chain(right.into_iter().rev()).collect()
    }

    /// `n + 1` points of the arc offset sideways by `lateral` inches (positive
    /// toward the left of start-to-end), at evenly spaced angles from start to
    /// end. The offsets are radial, so every point lies exactly `lateral` from
    /// the centerline and the first and last are square to the end tangents.
    /// A straight wall gives evenly spaced points along the offset chord.
    pub fn offset_curve(&self, lateral: f64, n: usize) -> Vec<Point> {
        let n = n.max(1);
        let Some((c, r)) = self.arc_center_radius() else {
            let off = self.normal().scale(lateral);
            return (0..=n)
                .map(|i| Point::lerp(self.start, self.end, i as f64 / n as f64).add(off))
                .collect();
        };
        let sweep = self.curve.map_or(0.0, |k| k.sweep(self.start, self.end));
        let a0 = self.start.sub(c).angle();
        // The left of travel is toward the center when turning counter-clockwise.
        let radius = (r - sweep.signum() * lateral).max(0.0);
        (0..=n)
            .map(|i| {
                let a = a0 + sweep * i as f64 / n as f64;
                Point::new(c.x + radius * a.cos(), c.y + radius * a.sin())
            })
            .collect()
    }

    /// The point `s` inches along the centerline (arc length for a curved
    /// wall) and its unit tangent in the direction of travel.
    pub fn frame_at(&self, s: f64) -> (Point, Point) {
        let len = self.path_length();
        let s = s.clamp(0.0, len);
        let Some((c, r)) = self.arc_center_radius() else {
            return (self.point_at(s), self.direction());
        };
        let sweep = self.curve.map_or(0.0, |k| k.sweep(self.start, self.end));
        let a = self.start.sub(c).angle() + sweep * s / len.max(1e-9);
        let p = Point::new(c.x + r * a.cos(), c.y + r * a.sin());
        let t = if sweep >= 0.0 {
            Point::new(-a.sin(), a.cos())
        } else {
            Point::new(a.sin(), -a.cos())
        };
        (p, t)
    }

    /// The point of the centerline closest to `p` and the unit direction of
    /// travel there: the tangent of an arc (past its ends, the nearer end),
    /// the wall's direction when straight.
    pub fn closest_point(&self, p: Point) -> (Point, Point) {
        let (Some((c, r)), Some(curve)) = (self.arc_center_radius(), self.curve) else {
            let (_, q) = crate::geometry::project_on_segment(p, self.start, self.end);
            return (q, self.direction());
        };
        let sweep = curve.sweep(self.start, self.end);
        let a0 = self.start.sub(c).angle();
        let v = p.sub(c);
        // How far round the arc the radial through `p` lies, in the arc's sense.
        let round = if sweep >= 0.0 {
            (v.angle() - a0).rem_euclid(PI * 2.0)
        } else {
            (a0 - v.angle()).rem_euclid(PI * 2.0)
        };
        if v.length() > 1e-9 && round <= sweep.abs() {
            let radial = v.normalized();
            let tangent = if sweep >= 0.0 {
                radial.perp()
            } else {
                -radial.perp()
            };
            return (c.add(radial.scale(r)), tangent);
        }
        if p.dist(self.start) <= p.dist(self.end) {
            (self.start, self.end_tangent(WallEnd::Start))
        } else {
            (self.end, -self.end_tangent(WallEnd::End))
        }
    }

    /// The readout of a curved wall for the Select tool and the drawing
    /// status (W-64, W-74): chord, radius, arc length and swept angle in
    /// inches and degrees. `None` for a straight wall.
    pub fn arc_readout(&self) -> Option<ArcReadout> {
        let (_, radius) = self.arc_center_radius()?;
        let curve = self.curve?;
        Some(ArcReadout {
            chord: self.length(),
            radius,
            arc_length: curve.arc_length(self.start, self.end),
            sweep_deg: curve.sweep_abs(self.length()).to_degrees(),
        })
    }

    /// Whether the wall is a real (non-straight) arc.
    pub fn is_curved(&self) -> bool {
        self.curve.is_some_and(|c| !c.is_straight())
    }

    /// Arc center and radius of a curved wall (W-65).
    pub fn arc_center_radius(&self) -> Option<(Point, f64)> {
        self.curve?.arc_center_radius(self.start, self.end)
    }

    /// `n + 1` centerline points from start to end (chord points when straight).
    pub fn sample_points(&self, n: usize) -> Vec<Point> {
        match self.curve {
            Some(c) => c.sample_points(self.start, self.end, n),
            None => WallCurve { bulge: 0.0 }.sample_points(self.start, self.end, n),
        }
    }

    /// Length along the centerline: the arc length for curved walls, the
    /// chord (same as [`Wall::length`]) otherwise.
    pub fn path_length(&self) -> f64 {
        match self.curve {
            Some(c) => c.arc_length(self.start, self.end),
            None => self.length(),
        }
    }

    /// Unit direction at `end` pointing into the wall along its centerline
    /// (the tangent for a curved wall).
    pub fn end_tangent(&self, end: WallEnd) -> Point {
        let curve = self.curve.unwrap_or(WallCurve { bulge: 0.0 });
        match end {
            WallEnd::Start => curve.tangent_at_start(self.start, self.end),
            WallEnd::End => -curve.tangent_at_end(self.start, self.end),
        }
    }

    /// Reverse Layers (W-23): the layer stack swaps faces. The centerline,
    /// thickness and openings stay put.
    pub fn reverse_layers(&mut self) {
        self.exterior_side = self.exterior_side.opposite();
    }

    /// Unit normal pointing to the exterior side (W-21).
    pub fn exterior_normal(&self) -> Point {
        self.normal() * self.exterior_side.sign()
    }
}

/// Moves an opening's center in proportion along a wall whose path length
/// changed by `k` to `new_len` (a straight wall made curved, a radius
/// edit), clamped so the opening still fits.
pub fn scale_opening_offset(o: &mut crate::model::Opening, k: f64, new_len: f64) {
    let half = o.width * 0.5;
    let c = o.center_offset * k;
    o.center_offset = if new_len > o.width {
        c.clamp(half, new_len - half)
    } else {
        new_len * 0.5
    };
}

/// What stays put when a curved wall's radius or arc angle is typed in the
/// Wall Specification (W-66, "Lock").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArcLock {
    /// The wall's start and end stay; the arc bends between them.
    #[default]
    Ends,
    /// The arc center stays; a new radius moves both ends along their radii
    /// and a new arc angle swings the end about the center.
    Center,
}

impl Wall {
    /// Lateral position (along the +normal) of the reference line `about`,
    /// resolved against the wall type `ty` (W-26).
    pub fn reference_lateral(&self, ty: Option<&WallTypeDef>, about: ResizeAbout) -> f64 {
        reference_lateral(self, ty, about)
    }

    /// The radius of the arc measured to the reference line `about` (W-66
    /// "Radius to"): the centerline radius for [`ResizeAbout::WallCenter`],
    /// larger toward the outside of the bend. `None` for a straight wall.
    pub fn radius_to(&self, ty: Option<&WallTypeDef>, about: ResizeAbout) -> Option<f64> {
        let (_, r) = self.arc_center_radius()?;
        let sign = self.curve?.bulge.signum();
        Some((r + sign * self.reference_lateral(ty, about)).max(0.0))
    }

    /// Scales the arc about its center so the centerline radius becomes
    /// `radius`, keeping the arc angle (the Arc Center lock). Returns the
    /// factor the path length grew by, or `None` for a straight wall or a
    /// radius below 1/8 inch.
    fn scale_about_center(&mut self, radius: f64) -> Option<f64> {
        let (c, r) = self.arc_center_radius()?;
        if radius < 0.125 || r < 1e-9 {
            return None;
        }
        let k = radius / r;
        let curve = self.curve?;
        self.start = c + (self.start - c) * k;
        self.end = c + (self.end - c) * k;
        self.curve = Some(WallCurve {
            bulge: curve.bulge * k,
        });
        Some(k)
    }

    /// Makes the radius to the reference line `about` equal `radius`
    /// (W-66). With [`ArcLock::Ends`] the ends stay and the arc is re-fitted
    /// (a radius under half the chord falls back to the semicircle, and a
    /// bend of over 180 degrees stays the long way round); with
    /// [`ArcLock::Center`] the arc center and angle stay and the ends move.
    /// Returns the factor the path length changed by (the openings follow),
    /// or `None` when the wall is straight or the radius cannot be used.
    pub fn set_radius_to(
        &mut self,
        ty: Option<&WallTypeDef>,
        about: ResizeAbout,
        radius: f64,
        lock: ArcLock,
    ) -> Option<f64> {
        let curve = self.curve.filter(|c| !c.is_straight())?;
        let sign = curve.bulge.signum();
        let centerline = radius - sign * self.reference_lateral(ty, about);
        let old_len = self.path_length();
        match lock {
            ArcLock::Center => self.scale_about_center(centerline),
            ArcLock::Ends => {
                let chord = self.length();
                let half = chord * 0.5;
                let r = centerline.max(half);
                let long_way = curve.sweep_abs(chord) > PI;
                let root = (r * r - half * half).max(0.0).sqrt();
                let s = if long_way { r + root } else { r - root };
                self.curve = Some(WallCurve { bulge: sign * s });
                Some(self.path_length() / old_len.max(1e-9))
            }
        }
    }

    /// Sets the arc angle (radians, 1 to 340 degrees). With
    /// [`ArcLock::Ends`] the chord stays; with [`ArcLock::Center`] the start,
    /// the center and the radius stay and the end swings about the center.
    /// Returns the factor the path length changed by, or `None` for a
    /// straight wall or an unusable angle.
    pub fn set_sweep_locked(&mut self, sweep: f64, lock: ArcLock) -> Option<f64> {
        let curve = self.curve.filter(|c| !c.is_straight())?;
        let left = curve.bulge > 0.0;
        let sweep = sweep.clamp(1f64.to_radians(), 340f64.to_radians());
        let old_len = self.path_length();
        let new_curve = match lock {
            ArcLock::Ends => WallCurve::from_sweep(self.length(), sweep, left)?,
            ArcLock::Center => {
                let (c, r) = self.arc_center_radius()?;
                // The sweep is counter-clockwise for a right-hand bulge.
                let signed = if left { -sweep } else { sweep };
                let a = self.start.sub(c).angle() + signed;
                self.end = Point::new(c.x + r * a.cos(), c.y + r * a.sin());
                WallCurve::from_sweep(self.length(), sweep, left)?
            }
        };
        self.curve = Some(new_curve);
        Some(self.path_length() / old_len.max(1e-9))
    }

    /// Where a point of the wall's plane lands when the centerline moves
    /// `lateral` inches toward the +normal: straight walls slide sideways,
    /// arcs keep their center and change radius.
    pub fn laterally_shifted(&self, p: Point, lateral: f64) -> Point {
        match (self.arc_center_radius(), self.curve) {
            (Some((c, r)), Some(curve)) => {
                let radius = (r + curve.bulge.signum() * lateral).max(0.125);
                c + (p - c).normalized() * radius
            }
            _ => p + self.normal() * lateral,
        }
    }

    /// Moves the centerline `lateral` inches toward the +normal (an arc
    /// keeps its center). Returns the factor the path length changed by.
    pub fn shift_laterally(&mut self, lateral: f64) -> f64 {
        if lateral.abs() < 1e-9 {
            return 1.0;
        }
        match (self.arc_center_radius(), self.curve) {
            (Some((_, r)), Some(curve)) => {
                let radius = (r + curve.bulge.signum() * lateral).max(0.125);
                self.scale_about_center(radius).unwrap_or(1.0)
            }
            _ => {
                let d = self.normal() * lateral;
                self.start = self.start + d;
                self.end = self.end + d;
                1.0
            }
        }
    }
}

impl Default for Wall {
    fn default() -> Self {
        Wall::new(
            Point::ZERO,
            Point::new(120.0, 0.0),
            crate::model::DEFAULT_INTERIOR_THICKNESS,
            DEFAULT_CEILING_HEIGHT,
            WallKind::Interior,
        )
    }
}

/// Lateral position (along the wall's +normal) of the reference line.
fn reference_lateral(wall: &Wall, ty: Option<&WallTypeDef>, about: ResizeAbout) -> f64 {
    let ext = wall.exterior_side.sign();
    let half = wall.thickness * 0.5;
    let bands = wall_layer_bands(wall, ty);
    let main = bands.iter().find(|b| b.is_main);
    match about {
        ResizeAbout::WallCenter => 0.0,
        ResizeAbout::OuterSurface => ext * half,
        ResizeAbout::InnerSurface => -ext * half,
        ResizeAbout::MainLayerOutside => main.map_or(ext * half, |b| b.outer),
        ResizeAbout::MainLayerInside => main.map_or(-ext * half, |b| b.inner),
    }
}

impl Project {
    /// The registered wall type called `name` (W-47).
    pub fn wall_type_def(&self, name: &str) -> Option<&WallTypeDef> {
        self.wall_types.iter().find(|t| t.name == name)
    }

    /// Add or replace (by name) a wall type in the project registry.
    pub fn register_wall_type(&mut self, ty: WallTypeDef) {
        match self.wall_types.iter_mut().find(|t| t.name == ty.name) {
            Some(slot) => *slot = ty,
            None => self.wall_types.push(ty),
        }
    }

    /// Move the centerline so the reference line `about` keeps its place when
    /// `old` is replaced by `new` (same position, different thickness/type).
    fn shifted_for_reference(
        &self,
        old: &Wall,
        new: &Wall,
        new_ty: Option<&WallTypeDef>,
        about: ResizeAbout,
    ) -> Point {
        let old_ty = old.wall_type.as_deref().and_then(|n| self.wall_type_def(n));
        let r_old = reference_lateral(old, old_ty, about);
        let r_new = reference_lateral(new, new_ty, about);
        old.normal() * (r_old - r_new)
    }

    /// Change a wall's total thickness, growing or shrinking it on the side
    /// the wall's [`ResizeAbout`] keeps fixed (W-27..W-29): the centerline
    /// shifts accordingly. A thickness edit changes the main layer only, so
    /// for a layered wall the main-layer references coincide with the surface
    /// references. Returns `false` for an unknown wall or a thickness below
    /// [`MIN_WALL_THICKNESS`].
    pub fn set_wall_thickness_about(&mut self, floor: usize, id: Id, new_thickness: f64) -> bool {
        if new_thickness.is_nan() || new_thickness < MIN_WALL_THICKNESS {
            return false;
        }
        let Some(old) = self.floors[floor].wall(id).cloned() else {
            return false;
        };
        let mut new = old.clone();
        new.thickness = new_thickness;
        let ty = old
            .wall_type
            .as_deref()
            .and_then(|n| self.wall_type_def(n))
            .cloned();
        let shift = self.shifted_for_reference(&old, &new, ty.as_ref(), old.resize_about);
        if let Some(w) = self.floors[floor].wall_mut(id) {
            w.thickness = new_thickness;
            w.start = w.start + shift;
            w.end = w.end + shift;
        }
        true
    }

    /// Give a wall a new wall type (registering it in the project) and set the
    /// thickness to the type's total, holding the `about` reference line fixed
    /// (W-27, W-49). `about` becomes the wall's stored reference. Returns
    /// `false` for an unknown wall or a type thinner than [`MIN_WALL_THICKNESS`].
    pub fn set_wall_type(
        &mut self,
        floor: usize,
        id: Id,
        ty: &WallTypeDef,
        about: ResizeAbout,
    ) -> bool {
        let t = ty.thickness();
        if t.is_nan() || t < MIN_WALL_THICKNESS {
            return false;
        }
        let Some(old) = self.floors[floor].wall(id).cloned() else {
            return false;
        };
        let mut new = old.clone();
        new.thickness = t;
        new.wall_type = Some(ty.name.clone());
        // Resolve the old stack before the registry entry may be replaced.
        let shift = self.shifted_for_reference(&old, &new, Some(ty), about);
        self.register_wall_type(ty.clone());
        if let Some(w) = self.floors[floor].wall_mut(id) {
            w.thickness = t;
            w.wall_type = Some(ty.name.clone());
            w.resize_about = about;
            w.start = w.start + shift;
            w.end = w.end + shift;
        }
        true
    }

    /// Split a wall at the point nearest `point` into two walls. The first
    /// piece (start side) keeps `id`; the second gets a fresh id. Openings
    /// move to the piece that holds them; flags, type and roof directive are
    /// preserved. Returns `None` if the wall is unknown, the point is within
    /// 0.01" of an end, or it falls inside an opening. Curved walls split
    /// into two arcs of the same radius.
    pub fn split_wall_at(&mut self, floor: usize, id: Id, point: Point) -> Option<(Id, Id)> {
        let wall = self.floors[floor].wall(id)?.clone();
        let curved = wall.is_curved();
        // Split position: arc length from the start, split point on the centerline.
        let (d, split_pt, curve_parts) = if curved {
            let curve = wall.curve?;
            let (c, r) = curve.arc_center_radius(wall.start, wall.end)?;
            let sweep = curve.sweep(wall.start, wall.end);
            let a0 = wall.start.sub(c).angle();
            let ap = point.sub(c).angle();
            // Signed angular travel from the start toward the point.
            let mut da = ap - a0;
            while da > PI {
                da -= 2.0 * PI;
            }
            while da < -PI {
                da += 2.0 * PI;
            }
            if da.signum() != sweep.signum() && da.abs() > 1e-9 {
                da += 2.0 * PI * sweep.signum();
            }
            let frac = (da / sweep).clamp(0.0, 1.0);
            let a = a0 + sweep * frac;
            let p = Point::new(c.x + r * a.cos(), c.y + r * a.sin());
            let s1 = sweep * frac;
            let s2 = sweep - s1;
            (
                r * s1.abs(),
                p,
                Some((curve.sub_curve(r, s1), curve.sub_curve(r, s2))),
            )
        } else {
            let (t, q) = project_on_segment(point, wall.start, wall.end);
            (t * wall.length(), q, None)
        };
        let total = wall.path_length();
        if d < 0.01 || total - d < 0.01 {
            return None;
        }
        let f = &self.floors[floor];
        if f.openings_on(id)
            .any(|o| o.start_offset() < d && o.end_offset() > d)
        {
            return None;
        }
        let new_id = self.alloc_id();
        let f = &mut self.floors[floor];
        let mut second = wall.clone();
        second.id = new_id;
        second.start = split_pt;
        second.end = wall.end;
        if let Some(w) = f.wall_mut(id) {
            w.end = split_pt;
        }
        if let Some((c1, c2)) = curve_parts {
            if let Some(w) = f.wall_mut(id) {
                w.curve = Some(c1);
            }
            second.curve = Some(c2);
        }
        f.walls.push(second);
        for o in f.openings.iter_mut().filter(|o| o.wall_id == id) {
            if o.center_offset > d {
                o.wall_id = new_id;
                o.center_offset -= d;
            }
        }
        for g in f.groups.iter_mut() {
            let had = g.members.contains(&crate::groups::ObjectRef::Wall(id));
            if had {
                g.members.push(crate::groups::ObjectRef::Wall(new_id));
            }
        }
        Some((id, new_id))
    }

    /// Reverse Layers on one wall (W-23). False for an unknown wall. The
    /// layer stack swaps faces and the main layer keeps its place: the
    /// centerline moves by twice the main layer's offset from it, and the
    /// ends of connected walls follow (a T-junction end slides with the
    /// wall it butts into). A wall whose main layer is centered does not
    /// move.
    pub fn reverse_wall_layers(&mut self, floor: usize, id: Id) -> bool {
        let Some(wall) = self.floors[floor].wall(id).cloned() else {
            return false;
        };
        let ty = wall
            .wall_type
            .as_deref()
            .and_then(|n| self.wall_type_def(n))
            .cloned();
        let main_center = wall_layer_bands(&wall, ty.as_ref())
            .iter()
            .find(|b| b.is_main)
            .map_or(0.0, |b| (b.outer + b.inner) * 0.5);
        let shift = 2.0 * main_center;
        // The neighbour ends that follow, found before anything moves: the
        // end of a corner or straight-on neighbour nearest our end, and the
        // end of a wall butting into this one. (A tee where this wall butts
        // into another leaves the through wall alone.)
        let mut follow: Vec<(Id, WallEnd)> = Vec::new();
        for c in self.wall_connections(floor, id) {
            if c.kind == joins::ConnectionKind::Tee {
                continue;
            }
            let at = if c.at == WallEnd::Start {
                wall.start
            } else {
                wall.end
            };
            if let Some(o) = self.floors[floor].wall(c.other) {
                let end = if o.start.dist(at) <= o.end.dist(at) {
                    WallEnd::Start
                } else {
                    WallEnd::End
                };
                follow.push((c.other, end));
            }
        }
        follow.extend(self.walls_butting_into(floor, id));
        let f = &mut self.floors[floor];
        let Some(w) = f.wall_mut(id) else {
            return false;
        };
        w.reverse_layers();
        if shift.abs() < 1e-9 {
            return true;
        }
        let before = w.clone();
        let k = w.shift_laterally(shift);
        for (other, end) in follow {
            if let Some(o) = f.wall_mut(other) {
                match end {
                    WallEnd::Start => o.start = before.laterally_shifted(o.start, shift),
                    WallEnd::End => o.end = before.laterally_shifted(o.end, shift),
                }
            }
        }
        if (k - 1.0).abs() > 1e-9 {
            let new_len = f.wall(id).map_or(0.0, |w| w.path_length());
            for o in f.openings.iter_mut().filter(|o| o.wall_id == id) {
                scale_opening_offset(o, k, new_len);
            }
        }
        true
    }

    /// Reverse Layers without moving the wall: the layer stack swaps faces
    /// about the centerline. Used by the automatic reverse when a room
    /// closes (DECISIONS DT3). False for an unknown wall.
    pub fn reverse_wall_layers_in_place(&mut self, floor: usize, id: Id) -> bool {
        let Some(f) = self.floors.get_mut(floor) else {
            return false;
        };
        let Some(w) = f.wall_mut(id) else {
            return false;
        };
        w.reverse_layers();
        true
    }

    /// Sets (or clears) the curve of a wall, keeping its openings in
    /// proportion along the centerline (W-67): the arc is longer than the
    /// chord, so offsets scale with the path length and are clamped to fit.
    pub fn set_wall_curve(&mut self, floor: usize, id: Id, curve: Option<WallCurve>) -> bool {
        let f = &mut self.floors[floor];
        let Some(w) = f.wall_mut(id) else {
            return false;
        };
        let old_len = w.path_length();
        w.curve = curve.filter(|c| !c.is_straight());
        let new_len = w.path_length();
        if old_len > 1e-9 && (new_len - old_len).abs() > 1e-9 {
            let k = new_len / old_len;
            for o in f.openings.iter_mut().filter(|o| o.wall_id == id) {
                scale_opening_offset(o, k, new_len);
            }
        }
        true
    }

    /// Change Line/Arc (W-67): a straight wall becomes an arc with `bulge`
    /// (a quarter of the chord when 0), a curved one becomes straight.
    /// Returns whether the wall is curved afterwards, or `None` for an
    /// unknown or zero-length wall.
    pub fn change_line_arc(&mut self, floor: usize, id: Id, bulge: f64) -> Option<bool> {
        let w = self.floors[floor].wall(id)?;
        if w.length() < 1e-6 {
            return None;
        }
        let curved = w.is_curved();
        let bulge = if bulge.abs() < 1e-9 {
            w.length() * 0.25
        } else {
            bulge
        };
        let next = (!curved).then_some(WallCurve { bulge });
        self.set_wall_curve(floor, id, next);
        Some(!curved)
    }

    /// Make Arc Tangent (S-55, W-68): the curved wall `id` is refit so it
    /// leaves the end it shares with a connected wall along that wall's
    /// direction. The start end is tried first. Returns the end that was
    /// matched, or why nothing changed.
    pub fn make_arc_tangent(&mut self, floor: usize, id: Id) -> Result<WallEnd, String> {
        let me = self.floors[floor]
            .wall(id)
            .cloned()
            .ok_or("The wall is gone")?;
        if !me.is_curved() {
            return Err("Make Arc Tangent needs a curved wall: use Change Line/Arc first".into());
        }
        for at in [WallEnd::Start, WallEnd::End] {
            let here = match at {
                WallEnd::Start => me.start,
                WallEnd::End => me.end,
            };
            for c in self.wall_connections(floor, id) {
                if c.at != at || c.kind == joins::ConnectionKind::Tee {
                    continue;
                }
                let Some(other) = self.floors[floor].wall(c.other).cloned() else {
                    continue;
                };
                // The neighbour's end that touches ours.
                let oe = if other.start.dist(here) <= other.end.dist(here) {
                    WallEnd::Start
                } else {
                    WallEnd::End
                };
                // The arc continues straight through the junction.
                let through = -other.end_tangent(oe);
                let bulge = match at {
                    WallEnd::Start => WallCurve::tangent_to(me.start, me.end, through),
                    WallEnd::End => WallCurve::tangent_to(me.end, me.start, through)
                        .map(|c| WallCurve { bulge: -c.bulge }),
                };
                let Some(curve) = bulge else {
                    return Err("No arc fits tangent to that wall".into());
                };
                self.set_wall_curve(floor, id, Some(curve));
                return Ok(at);
            }
        }
        Err("The curved wall is not connected to another wall".into())
    }

    /// Alias of [`Project::split_wall_at`] (Chief's Break Wall).
    pub fn break_wall(&mut self, floor: usize, id: Id, point: Point) -> Option<(Id, Id)> {
        self.split_wall_at(floor, id, point)
    }

    /// Merge two straight walls that share an end and run in the same line
    /// with the same thickness, height, kind, layer, type and flags. The
    /// merged wall keeps `a`'s id and direction; `b`'s openings move onto it.
    /// Returns the merged id, or `None` if the walls cannot be merged.
    pub fn join_collinear_walls(&mut self, floor: usize, a: Id, b: Id) -> Option<Id> {
        if a == b {
            return None;
        }
        let wa = self.floors[floor].wall(a)?.clone();
        let wb = self.floors[floor].wall(b)?.clone();
        if wa.is_curved()
            || wb.is_curved()
            || (wa.thickness - wb.thickness).abs() > 1e-9
            || (wa.height - wb.height).abs() > 1e-9
            || wa.kind != wb.kind
            || wa.class != wb.class
            || wa.layer != wb.layer
            || wa.wall_type != wb.wall_type
            || wa.flags != wb.flags
            || wa.exterior_side != wb.exterior_side
        {
            return None;
        }
        if wa.direction().cross(wb.direction()).abs() > 1e-3 {
            return None;
        }
        let la = wa.length();
        let lb = wb.length();
        // Which ends touch.
        let ends = [
            (wa.end, wb.start, true, true),
            (wa.end, wb.end, true, false),
            (wa.start, wb.end, false, true),
            (wa.start, wb.start, false, false),
        ];
        let &(_, _, a_end, b_start) = ends
            .iter()
            .find(|(pa, pb, _, _)| pa.dist(*pb) <= EDIT_TOL)?;
        // Offsets are re-based onto a's frame.
        let (new_start, new_end, a_shift, b_map): (Point, Point, f64, Box<dyn Fn(f64) -> f64>) =
            match (a_end, b_start) {
                // a.end -- b.start: continue forward.
                (true, true) => (wa.start, wb.end, 0.0, Box::new(move |o| o + la)),
                // a.end -- b.end: b runs backward.
                (true, false) => (wa.start, wb.start, 0.0, Box::new(move |o| la + (lb - o))),
                // b.end -- a.start: b precedes a.
                (false, true) => (wb.start, wa.end, lb, Box::new(|o| o)),
                // b.start -- a.start: b precedes a, reversed.
                (false, false) => (wb.end, wa.end, lb, Box::new(move |o| lb - o)),
            };
        let f = &mut self.floors[floor];
        for o in f.openings.iter_mut() {
            if o.wall_id == a {
                o.center_offset += a_shift;
            } else if o.wall_id == b {
                o.wall_id = a;
                o.center_offset = b_map(o.center_offset);
            }
        }
        if let Some(w) = f.wall_mut(a) {
            w.start = new_start;
            w.end = new_end;
        }
        f.walls.retain(|w| w.id != b);
        for g in f.groups.iter_mut() {
            g.members
                .retain(|m| *m != crate::groups::ObjectRef::Wall(b));
        }
        Some(a)
    }

    /// Walls joined to the ends of wall `id` (corner, collinear continuation
    /// or T-junction where this wall butts into another), using the same
    /// matching as the join geometry in [`crate::joins`].
    pub fn wall_connections(&self, floor: usize, id: Id) -> Vec<WallConnection> {
        let walls = &self.floors[floor].walls;
        let Some(i) = walls.iter().position(|w| w.id == id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for at in [WallEnd::Start, WallEnd::End] {
            for (j, kind) in joins::wall_end_joins(walls, i, at, EDIT_TOL) {
                out.push(WallConnection {
                    other: walls[j].id,
                    at,
                    kind,
                });
            }
        }
        out
    }

    /// Walls whose end butts into the interior of wall `id` (the through wall
    /// side of a T-junction), with the butting end.
    pub fn walls_butting_into(&self, floor: usize, id: Id) -> Vec<(Id, WallEnd)> {
        let walls = &self.floors[floor].walls;
        let mut out = Vec::new();
        for (j, w) in walls.iter().enumerate() {
            if w.id == id {
                continue;
            }
            for at in [WallEnd::Start, WallEnd::End] {
                for (k, kind) in joins::wall_end_joins(walls, j, at, EDIT_TOL) {
                    if kind == joins::ConnectionKind::Tee && walls[k].id == id {
                        out.push((w.id, at));
                    }
                }
            }
        }
        out
    }
}

/// Room an opening keeps between its jamb and the end of its wall, inches
/// (W-85); the editor's own margin is the same.
pub const OPENING_JAMB_MARGIN: f64 = 2.0;

/// Which end of a wall stays put while its length changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthLock {
    Start,
    End,
}

impl WallClass {
    /// Whether the invisible walls between platforms (W-63) are generated
    /// under a wall of this class: the ordinary walls, not railings, fences,
    /// dividers, half walls or foundation walls.
    fn generates_platform_walls(&self) -> bool {
        matches!(
            self,
            WallClass::Standard
                | WallClass::Pony { .. }
                | WallClass::Glass
                | WallClass::GlassPony { .. }
        )
    }
}

impl Project {
    /// The shortest wall `id` of `floor` may be made while `lock` stays put
    /// and its openings keep their distance from that end (W-85): the span
    /// the openings take plus their jamb margin. Zero when it has none (or
    /// no such wall).
    pub fn min_wall_length(&self, floor: usize, id: Id, lock: LengthLock) -> f64 {
        let Some(f) = self.floors.get(floor) else {
            return 0.0;
        };
        let Some(w) = f.wall(id) else { return 0.0 };
        let len = w.path_length();
        f.openings_on(id)
            .map(|o| match lock {
                LengthLock::Start => o.end_offset() + OPENING_JAMB_MARGIN,
                LengthLock::End => len - o.start_offset() + OPENING_JAMB_MARGIN,
            })
            .fold(0.0, f64::max)
    }

    /// Generate Between Platforms (W-63, W-25): removes the invisible walls
    /// generated on `floor` before and makes new ones where a wall with
    /// "Generate Invisible Walls and Railings Between Platforms" on stands
    /// under a wall of the floor above. Each fills the gap between the top of
    /// the lower wall and the floor above's platform over the stretch the
    /// upper wall rests on; it is invisible, defines no room and is not
    /// located. A wall that balloons through the platform has no gap and gets
    /// none. Returns how many were generated.
    pub fn generate_between_platforms(&mut self, floor: usize) -> usize {
        let Some(f) = self.floors.get(floor) else {
            return 0;
        };
        let mut made: Vec<Wall> = Vec::new();
        if let Some(up) = self.floors.get(floor + 1) {
            for w in &f.walls {
                if w.flags.invisible
                    || w.flags.auto_generated
                    || w.is_curved()
                    || !w.spec.structure.generate_between_platforms
                    || !w.class.generates_platform_walls()
                    || w.length() < 1e-6
                {
                    continue;
                }
                let adj = self.wall_platform_adjust(floor, w);
                let gap = self.platform_gap_above(floor, w) - adj.raise;
                if gap <= 0.01 {
                    continue;
                }
                let (u, n, len) = (w.direction(), w.normal(), w.length());
                let mut spans: Vec<(f64, f64)> = Vec::new();
                for o in &up.walls {
                    if o.flags.invisible
                        || o.flags.auto_generated
                        || o.is_curved()
                        || !o.class.generates_platform_walls()
                        || u.cross(o.direction()).abs() > 0.02
                    {
                        continue;
                    }
                    let off = o.start.sub(w.start).dot(n);
                    if off.abs() > (w.thickness + o.thickness) * 0.5 {
                        continue;
                    }
                    let (a, b) = (o.start.sub(w.start).dot(u), o.end.sub(w.start).dot(u));
                    let (lo, hi) = (a.min(b).max(0.0), a.max(b).min(len));
                    if hi - lo > 1.0 {
                        spans.push((lo, hi));
                    }
                }
                spans.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut merged: Vec<(f64, f64)> = Vec::new();
                for (lo, hi) in spans {
                    match merged.last_mut() {
                        Some(last) if lo <= last.1 + 1.0 => last.1 = last.1.max(hi),
                        _ => merged.push((lo, hi)),
                    }
                }
                for (lo, hi) in merged {
                    let mut g = Wall::new(w.point_at(lo), w.point_at(hi), w.thickness, gap, w.kind);
                    g.layer = w.layer.clone();
                    g.wall_type = w.wall_type.clone();
                    g.resize_about = w.resize_about;
                    g.exterior_side = w.exterior_side;
                    g.bottom_offset = w.bottom_offset + w.height;
                    g.flags.invisible = true;
                    g.flags.no_room_definition = true;
                    g.flags.no_locate = true;
                    g.flags.auto_generated = true;
                    g.spec.structure.generate_between_platforms = false;
                    made.push(g);
                }
            }
        }
        let count = made.len();
        // Nothing moved: keep the walls (and their ids) as they are.
        let same_wall = |a: &Wall, b: &Wall| {
            a.start.dist(b.start) < 1e-6
                && a.end.dist(b.end) < 1e-6
                && (a.height - b.height).abs() < 1e-6
                && (a.bottom_offset - b.bottom_offset).abs() < 1e-6
                && (a.thickness - b.thickness).abs() < 1e-6
        };
        let old: Vec<&Wall> = self.floors[floor]
            .walls
            .iter()
            .filter(|w| w.flags.auto_generated)
            .collect();
        if old.len() == made.len() && made.iter().all(|m| old.iter().any(|o| same_wall(o, m))) {
            return count;
        }
        self.floors[floor].walls.retain(|w| !w.flags.auto_generated);
        for mut g in made {
            g.id = self.alloc_id();
            self.floors[floor].walls.push(g);
        }
        count
    }

    /// Whether any wall asks for invisible walls between platforms, or any
    /// generated wall stands to be removed.
    pub fn has_platform_walls(&self) -> bool {
        self.floors.iter().any(|f| {
            f.walls
                .iter()
                .any(|w| w.flags.auto_generated || w.spec.structure.generate_between_platforms)
        })
    }

    /// Brings the generated platform walls up to date after an edit; does
    /// nothing (and costs one scan) when no wall uses the option. Returns how
    /// many walls are generated.
    pub fn sync_platform_walls(&mut self) -> usize {
        if !self.has_platform_walls() {
            return 0;
        }
        self.generate_all_between_platforms()
    }

    /// [`Project::generate_between_platforms`] on every floor. Returns how
    /// many walls were generated in all.
    pub fn generate_all_between_platforms(&mut self) -> usize {
        (0..self.floors.len())
            .map(|i| self.generate_between_platforms(i))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::OpeningKind;

    fn proj_with_wall(t: f64, kind: WallKind) -> (Project, Id) {
        let mut p = Project::new("t");
        let id = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            t,
            DEFAULT_CEILING_HEIGHT,
            kind,
        );
        (p, id)
    }

    #[test]
    fn wall_new_and_default() {
        let w = Wall::new(
            Point::ZERO,
            Point::new(10.0, 0.0),
            6.5,
            96.0,
            WallKind::Exterior,
        );
        assert_eq!(w.resize_about, ResizeAbout::MainLayerOutside);
        assert_eq!(w.exterior_side, Side::Left);
        let i = Wall::new(
            Point::ZERO,
            Point::new(10.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        assert_eq!(i.resize_about, ResizeAbout::WallCenter);
        let d = Wall::default();
        assert_eq!(d.kind, WallKind::Interior);
        assert!(d.length() > 0.0);
    }

    #[test]
    fn thickness_about_main_layer_shifts_center_by_half_delta() {
        // Exterior side is Left (+y), so the outside face is at y = +3.25.
        let (mut p, id) = proj_with_wall(6.5, WallKind::Exterior);
        assert_eq!(
            p.floors[0].wall(id).unwrap().resize_about,
            ResizeAbout::MainLayerOutside
        );
        assert!(p.set_wall_thickness_about(0, id, 8.5));
        let w = p.floors[0].wall(id).unwrap();
        assert!((w.thickness - 8.5).abs() < 1e-9);
        // Outside face fixed at +3.25, so the center moves to the interior side by 1".
        assert!((w.start.y - -1.0).abs() < 1e-9 && (w.end.y - -1.0).abs() < 1e-9);
        assert!((w.start.y + w.thickness * 0.5 - 3.25).abs() < 1e-9);

        // Interior face fixed.
        let (mut p, id) = proj_with_wall(6.5, WallKind::Exterior);
        p.floors[0].wall_mut(id).unwrap().resize_about = ResizeAbout::MainLayerInside;
        assert!(p.set_wall_thickness_about(0, id, 4.5));
        let w = p.floors[0].wall(id).unwrap();
        assert!((w.start.y - -1.0).abs() < 1e-9 || (w.start.y - 1.0).abs() < 1e-9);
        assert!((w.start.y - w.thickness * 0.5 - -3.25).abs() < 1e-9);

        // Wall center stays put; too-thin and unknown walls are refused.
        let (mut p, id) = proj_with_wall(4.5, WallKind::Interior);
        assert!(p.set_wall_thickness_about(0, id, 6.5));
        assert_eq!(p.floors[0].wall(id).unwrap().start.y, 0.0);
        assert!(!p.set_wall_thickness_about(0, id, 0.0));
        assert!(!p.set_wall_thickness_about(0, 999, 6.0));
    }

    #[test]
    fn set_wall_type_holds_reference_line() {
        let d = crate::defaults::PlanDefaults::chief_x18_daniel();
        let i4 = d.wall_type("Interior-4").unwrap().clone();
        let i6 = d.wall_type("Interior-6").unwrap().clone();

        // WallCenter: centerline unchanged.
        let (mut p, id) = proj_with_wall(4.5, WallKind::Interior);
        assert!(p.set_wall_type(0, id, &i4, ResizeAbout::WallCenter));
        assert!(p.set_wall_type(0, id, &i6, ResizeAbout::WallCenter));
        let w = p.floors[0].wall(id).unwrap();
        assert_eq!(w.start.y, 0.0);
        assert!((w.thickness - 6.5).abs() < 1e-9);
        assert_eq!(w.wall_type.as_deref(), Some("Interior-6"));

        // MainLayerOutside: the main layer's outside line (y = 2.25 - 0.5 = 1.75
        // for Interior-4) stays; Interior-6's main outside is at start.y + 3.25 - 0.5.
        let (mut p, id) = proj_with_wall(4.5, WallKind::Interior);
        assert!(p.set_wall_type(0, id, &i4, ResizeAbout::WallCenter));
        let before = p.floors[0].wall(id).unwrap().start.y + 2.25 - 0.5;
        assert!(p.set_wall_type(0, id, &i6, ResizeAbout::MainLayerOutside));
        let w = p.floors[0].wall(id).unwrap();
        let after = w.start.y + w.thickness * 0.5 - 0.5;
        assert!((before - after).abs() < 1e-9, "{before} vs {after}");
        assert!((w.start.y - -1.0).abs() < 1e-9);
        // The registry holds both definitions.
        assert!(p.wall_type_def("Interior-4").is_some());
    }

    #[test]
    fn curve_geometry() {
        let c = WallCurve { bulge: 2.0 };
        let (a, b) = (Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let (center, r) = c.arc_center_radius(a, b).unwrap();
        assert!((r - 7.25).abs() < 1e-9);
        assert!(center.dist(Point::new(5.0, -5.25)) < 1e-9);
        let pts = c.sample_points(a, b, 2);
        assert!(pts[1].dist(Point::new(5.0, 2.0)) < 1e-9);
        for p in c.sample_points(a, b, 8) {
            assert!((p.dist(center) - r).abs() < 1e-9);
        }
        // Negative bulge goes to the right.
        let n = WallCurve { bulge: -2.0 }.sample_points(a, b, 2);
        assert!(n[1].dist(Point::new(5.0, -2.0)) < 1e-9);
        // Semicircle.
        let semi = WallCurve { bulge: 5.0 };
        assert!((semi.arc_length(a, b) - 5.0 * PI).abs() < 1e-9);
        assert!(WallCurve { bulge: 0.0 }.arc_center_radius(a, b).is_none());
        let w = Wall {
            curve: Some(c),
            ..Wall::new(a, b, 4.5, 96.0, WallKind::Interior)
        };
        assert!(w.is_curved() && w.arc_center_radius().is_some());
        assert!(w.path_length() > w.length());
    }

    #[test]
    fn split_keeps_openings_on_the_right_piece() {
        let (mut p, id) = proj_with_wall(4.5, WallKind::Interior);
        let d1 = p.add_opening(0, id, 50.0, OpeningKind::Door).unwrap();
        let d2 = p.add_opening(0, id, 190.0, OpeningKind::Window).unwrap();
        p.floors[0].wall_mut(id).unwrap().flags.no_locate = true;
        p.floors[0].wall_mut(id).unwrap().wall_type = Some("Interior-4".into());
        // Splitting through an opening is refused.
        assert!(p.split_wall_at(0, id, Point::new(50.0, 3.0)).is_none());
        // Ends are refused.
        assert!(p.split_wall_at(0, id, Point::new(0.0, 0.0)).is_none());
        let (a, b) = p.split_wall_at(0, id, Point::new(120.0, 7.0)).unwrap();
        assert_eq!(a, id);
        assert_ne!(a, b);
        let f = &p.floors[0];
        assert!((f.wall(a).unwrap().length() - 120.0).abs() < 1e-9);
        assert!((f.wall(b).unwrap().length() - 120.0).abs() < 1e-9);
        assert_eq!(f.wall(b).unwrap().start, Point::new(120.0, 0.0));
        assert!(f.wall(b).unwrap().flags.no_locate);
        assert_eq!(f.wall(b).unwrap().wall_type.as_deref(), Some("Interior-4"));
        let o1 = f.openings.iter().find(|o| o.id == d1).unwrap();
        let o2 = f.openings.iter().find(|o| o.id == d2).unwrap();
        assert_eq!(o1.wall_id, a);
        assert_eq!(o2.wall_id, b);
        assert!((o2.center_offset - 70.0).abs() < 1e-9);
        // break_wall is an alias; join puts everything back.
        let merged = p.join_collinear_walls(0, a, b).unwrap();
        assert_eq!(merged, a);
        let f = &p.floors[0];
        assert_eq!(f.walls.len(), 1);
        assert!((f.wall(a).unwrap().length() - 240.0).abs() < 1e-9);
        let o2 = f.openings.iter().find(|o| o.id == d2).unwrap();
        assert_eq!(o2.wall_id, a);
        assert!((o2.center_offset - 190.0).abs() < 1e-9);
        assert!(p.break_wall(0, a, Point::new(100.0, 0.0)).is_some());
    }

    #[test]
    fn split_curved_wall_into_two_arcs() {
        let mut p = Project::new("c");
        let id = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        p.floors[0].wall_mut(id).unwrap().curve = Some(WallCurve { bulge: 20.0 });
        let (a, b) = p.split_wall_at(0, id, Point::new(50.0, 20.0)).unwrap();
        let f = &p.floors[0];
        let (wa, wb) = (f.wall(a).unwrap(), f.wall(b).unwrap());
        assert!(wa.end.dist(Point::new(50.0, 20.0)) < 1e-6);
        let (_, r0) = Wall {
            curve: Some(WallCurve { bulge: 20.0 }),
            ..Wall::new(
                Point::ZERO,
                Point::new(100.0, 0.0),
                4.5,
                96.0,
                WallKind::Interior,
            )
        }
        .arc_center_radius()
        .unwrap();
        assert!((wa.arc_center_radius().unwrap().1 - r0).abs() < 1e-6);
        assert!((wb.arc_center_radius().unwrap().1 - r0).abs() < 1e-6);
        assert!((wa.path_length() - wb.path_length()).abs() < 1e-6);
    }

    #[test]
    fn join_refuses_mismatched_walls() {
        let mut p = Project::new("j");
        let a = p.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let b = p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(200.0, 0.0),
            6.5,
            96.0,
            WallKind::Interior,
        );
        let c = p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(100.0, 90.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        assert!(p.join_collinear_walls(0, a, b).is_none());
        assert!(p.join_collinear_walls(0, a, c).is_none());
        assert!(p.join_collinear_walls(0, a, a).is_none());
    }

    #[test]
    fn connections_and_flags() {
        let mut p = Project::new("k");
        let a = p.add_wall(
            0,
            Point::ZERO,
            Point::new(120.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let b = p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 100.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let c = p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(240.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let t = p.add_wall(
            0,
            Point::new(60.0, 80.0),
            Point::new(60.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        use joins::ConnectionKind::*;
        let ca = p.wall_connections(0, a);
        assert!(ca
            .iter()
            .any(|x| x.other == b && x.at == WallEnd::End && x.kind == Corner));
        assert!(ca.iter().any(|x| x.other == c && x.kind == Through));
        let ct = p.wall_connections(0, t);
        assert_eq!(ct.len(), 1);
        assert_eq!((ct[0].other, ct[0].at, ct[0].kind), (a, WallEnd::End, Tee));
        assert_eq!(p.walls_butting_into(0, a), vec![(t, WallEnd::End)]);
        assert!(p.wall_connections(0, 999).is_empty());

        let mut f = WallFlags::default();
        assert!(f.defines_rooms());
        f.invisible = true;
        assert!(!f.defines_rooms());
        f.room_divider = true;
        assert!(f.defines_rooms());
    }

    #[test]
    fn a_wall_raised_off_the_floor_closes_no_room() {
        use crate::model::{Project, WallKind};
        let mut p = Project::new("t");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(144.0, 0.0),
            Point::new(144.0, 120.0),
            Point::new(0.0, 120.0),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Interior);
        }
        assert_eq!(crate::detect_rooms(&p.floors[0].walls, 0.5).len(), 1);
        // Low raise (a sill-high wall): still closes the room.
        p.floors[0].walls[0].bottom_offset = 30.0;
        assert!(p.floors[0].walls[0].defines_rooms());
        assert_eq!(crate::detect_rooms(&p.floors[0].walls, 0.5).len(), 1);
        // Raised well above the floor (a dormer wall on the roof deck): no room.
        p.floors[0].walls[0].bottom_offset = ROOM_BOUNDARY_MAX_BOTTOM;
        assert!(!p.floors[0].walls[0].defines_rooms());
        assert!(crate::detect_rooms(&p.floors[0].walls, 0.5).is_empty());
        // A room divider that is raised is still a divider of the flags, but
        // not of the floor.
        p.floors[0].walls[0].bottom_offset = 0.0;
        p.floors[0].walls[0].flags.invisible = true;
        assert!(!p.floors[0].walls[0].defines_rooms());
    }

    #[test]
    fn old_wall_json_defaults() {
        let w: Wall = serde_json::from_str(
            r#"{"id":1,"start":{"x":0.0,"y":0.0},"end":{"x":10.0,"y":0.0},
                "thickness":6.5,"height":96.0,"kind":"Exterior"}"#,
        )
        .unwrap();
        assert_eq!(w.flags, WallFlags::default());
        assert_eq!(w.roof, WallRoofDirective::default());
        assert!(w.curve.is_none() && w.wall_type.is_none());
        assert_eq!(w.bottom_offset, 0.0, "old walls start at the floor");
        let mut raised = w.clone();
        raised.bottom_offset = 31.5;
        let s = serde_json::to_string(&raised).unwrap();
        let back: Wall = serde_json::from_str(&s).unwrap();
        assert_eq!(back.roof, w.roof);
        assert_eq!(back.bottom_offset, 31.5);
    }

    fn every_class() -> Vec<WallClass> {
        vec![
            WallClass::Standard,
            WallClass::Foundation,
            WallClass::Pony {
                upper_type: "Stucco-6".into(),
                lower_type: "Foundation-8".into(),
                split_height: 36.0,
                upper_sets_plan_display: true,
            },
            WallClass::Glass,
            WallClass::GlassPony {
                lower_type: "Siding-6".into(),
                split_height: 30.0,
            },
            WallClass::HalfWall { height: 36.0 },
            WallClass::RoomDivider,
            WallClass::Railing,
            WallClass::DeckRailing,
            WallClass::DeckEdge,
            WallClass::Fencing {
                style: FenceStyle::Privacy,
            },
        ]
    }

    #[test]
    fn every_wall_class_round_trips_through_json_straight_and_curved() {
        for class in every_class() {
            for curved in [false, true] {
                let mut w = Wall::new(
                    Point::ZERO,
                    Point::new(120.0, 0.0),
                    6.5,
                    96.0,
                    WallKind::Exterior,
                );
                w.set_class(class.clone());
                w.foundation_height = 40.0;
                if curved {
                    w.curve = Some(WallCurve { bulge: 20.0 });
                }
                let back: Wall = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                assert_eq!(back.class, class);
                assert_eq!(back.foundation_height, 40.0);
                assert_eq!(back.is_deck_edge, class == WallClass::DeckEdge);
                assert_eq!(back.flags, w.flags);
                assert_eq!(back.curve, w.curve);
            }
        }
    }

    #[test]
    fn set_class_keeps_the_legacy_flags_in_step() {
        let mut w = Wall::default();
        w.set_class(WallClass::RoomDivider);
        assert!(w.flags.room_divider && w.flags.invisible && w.is_room_divider());
        assert!(w.flags.defines_rooms());
        w.set_class(WallClass::Foundation);
        assert!(!w.flags.room_divider && !w.flags.invisible);
        assert!(w.flags.foundation && w.is_foundation());
        w.set_class(WallClass::Pony {
            upper_type: "a".into(),
            lower_type: "b".into(),
            split_height: 30.0,
            upper_sets_plan_display: false,
        });
        assert!(!w.flags.foundation);
        assert_eq!(w.flags.pony.as_ref().unwrap().lower_height, 30.0);
        w.set_class(WallClass::HalfWall { height: 36.0 });
        assert!(w.flags.pony.is_none() && w.flags.half_wall);
        w.set_class(WallClass::DeckEdge);
        assert!(w.is_deck_edge && !w.flags.half_wall);
        w.set_class(WallClass::Standard);
        assert_eq!(w.flags, WallFlags::default());
        assert!(!w.is_deck_edge);
    }

    #[test]
    fn old_wall_json_is_a_standard_wall() {
        let w: Wall = serde_json::from_str(
            r#"{"id":1,"start":{"x":0.0,"y":0.0},"end":{"x":10.0,"y":0.0},
                "thickness":6.5,"height":96.0,"kind":"Interior"}"#,
        )
        .unwrap();
        assert!(w.class.is_standard());
        assert_eq!(w.foundation_height, DEFAULT_FOUNDATION_HEIGHT);
    }

    #[test]
    fn curved_plan_polygon_is_a_band_around_the_arc() {
        let mut w = Wall::new(
            Point::ZERO,
            Point::new(100.0, 0.0),
            10.0,
            96.0,
            WallKind::Interior,
        );
        assert_eq!(w.plan_polygon().len(), 4);
        w.curve = Some(WallCurve { bulge: 25.0 });
        let poly = w.plan_polygon();
        assert!(poly.len() > 8);
        let (min, max) = poly
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        // The apex bulges 25" to the left (+y) and the band is 10" thick.
        assert!((max - 30.0).abs() < 0.5, "{max}");
        assert!(min < 0.0 && min > -6.0, "{min}");
    }

    #[test]
    fn different_classes_do_not_merge() {
        let mut p = Project::new("t");
        let a = p.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let b = p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(200.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        p.floors[0].wall_mut(b).unwrap().class = WallClass::Glass;
        assert!(p.join_collinear_walls(0, a, b).is_none());
        p.floors[0].wall_mut(b).unwrap().class = WallClass::Standard;
        assert!(p.join_collinear_walls(0, a, b).is_some());
    }

    #[test]
    fn a_room_divider_closes_a_room() {
        let mut p = Project::new("t");
        let c = [
            Point::ZERO,
            Point::new(200.0, 0.0),
            Point::new(200.0, 150.0),
            Point::new(0.0, 150.0),
        ];
        let mut ids = Vec::new();
        for i in 0..4 {
            ids.push(p.add_wall(0, c[i], c[(i + 1) % 4], 4.5, 96.0, WallKind::Interior));
        }
        let rooms = |p: &Project| crate::rooms::detect_rooms(&p.floors[0].walls, 0.5).len();
        assert_eq!(rooms(&p), 1);
        p.floors[0]
            .wall_mut(ids[3])
            .unwrap()
            .set_class(WallClass::RoomDivider);
        assert_eq!(rooms(&p), 1, "the divider still closes the room");
        // A plain invisible wall does not.
        let w = p.floors[0].wall_mut(ids[3]).unwrap();
        w.set_class(WallClass::Standard);
        w.flags.invisible = true;
        assert_eq!(rooms(&p), 0);
    }

    #[test]
    fn curve_radius_sweep_and_tangent_round_trip() {
        let chord = 120.0;
        let c = WallCurve { bulge: 30.0 };
        let r = c.radius(chord).unwrap();
        let back = WallCurve::from_radius(chord, r, true).unwrap();
        assert!((back.bulge - 30.0).abs() < 1e-9);
        let sweep = c.sweep_abs(chord);
        let back = WallCurve::from_sweep(chord, sweep, false).unwrap();
        assert!((back.bulge + 30.0).abs() < 1e-9);
        assert!(WallCurve::from_radius(chord, 59.0, true).is_none());
        // A semicircle: radius half the chord.
        let semi = WallCurve::from_radius(chord, 60.0, true).unwrap();
        assert!((semi.bulge - 60.0).abs() < 1e-9);
        // Tangent at the start of a left-bulging arc turns left of the chord.
        let (a, b) = (Point::new(0.0, 0.0), Point::new(chord, 0.0));
        let t = c.tangent_at_start(a, b);
        assert!(t.y > 0.0 && t.x > 0.0);
        let fit = WallCurve::tangent_to(a, b, t).unwrap();
        assert!((fit.bulge - 30.0).abs() < 1e-9, "{}", fit.bulge);
        let end = c.tangent_at_end(a, b);
        assert!(end.y < 0.0);
        assert!(WallCurve::tangent_to(a, b, Point::new(-1.0, 0.0)).is_none());
        assert!(WallCurve::tangent_to(a, b, Point::new(1.0, 0.0))
            .unwrap()
            .is_straight());
    }

    #[test]
    fn reverse_layers_flips_the_exterior_side_only() {
        let (mut p, id) = proj_with_wall(6.0, WallKind::Exterior);
        let before = p.floors[0].wall(id).unwrap().clone();
        assert!(p.reverse_wall_layers(0, id));
        let w = p.floors[0].wall(id).unwrap();
        assert_eq!(w.exterior_side, before.exterior_side.opposite());
        assert_eq!(
            (w.start, w.end, w.thickness),
            (before.start, before.end, 6.0)
        );
        let bands = joins::wall_layer_bands(w, None);
        let old = joins::wall_layer_bands(&before, None);
        assert!((bands[0].outer + old[0].outer).abs() < 1e-9);
        p.reverse_wall_layers(0, id);
        assert_eq!(
            p.floors[0].wall(id).unwrap().exterior_side,
            before.exterior_side
        );
        assert!(!p.reverse_wall_layers(0, 9999));
    }

    #[test]
    fn change_line_arc_keeps_openings_in_proportion() {
        let (mut p, id) = proj_with_wall(6.0, WallKind::Exterior);
        let o = p
            .add_opening(0, id, 120.0, crate::model::OpeningKind::Window)
            .unwrap();
        assert_eq!(p.change_line_arc(0, id, 60.0), Some(true));
        let w = p.floors[0].wall(id).unwrap();
        assert!(w.is_curved() && (w.curve.unwrap().bulge - 60.0).abs() < 1e-9);
        let len = w.path_length();
        assert!(len > 240.0);
        let oo = p.floors[0].openings.iter().find(|x| x.id == o).unwrap();
        assert!((oo.center_offset - len * 0.5).abs() < 1e-6);
        assert_eq!(p.change_line_arc(0, id, 0.0), Some(false));
        let w = p.floors[0].wall(id).unwrap();
        assert!(w.curve.is_none());
        let oo = p.floors[0].openings.iter().find(|x| x.id == o).unwrap();
        assert!((oo.center_offset - 120.0).abs() < 1e-6);
        // A zero bulge request on a straight wall uses a quarter of the chord.
        p.change_line_arc(0, id, 0.0);
        assert_eq!(p.floors[0].wall(id).unwrap().curve.unwrap().bulge, 60.0);
    }

    #[test]
    fn make_arc_tangent_continues_the_neighbour() {
        let mut p = Project::new("t");
        let a = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let b = p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(200.0, 100.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        // Straight second wall: an error.
        assert!(p.make_arc_tangent(0, b).is_err());
        p.floors[0].wall_mut(b).unwrap().curve = Some(WallCurve { bulge: -5.0 });
        assert_eq!(p.make_arc_tangent(0, b), Ok(WallEnd::Start));
        let w = p.floors[0].wall(b).unwrap();
        // It leaves (100, 0) heading east, like wall `a`.
        let t = w.end_tangent(WallEnd::Start);
        assert!((t.x - 1.0).abs() < 1e-9 && t.y.abs() < 1e-9, "{t:?}");
        assert!(w.curve.unwrap().bulge < 0.0);
        let _ = a;
        // Tangent at the End of a wall that continues past the junction.
        let c = p.add_wall(
            0,
            Point::new(200.0, 100.0),
            Point::new(200.0, 200.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let _ = c;
        p.floors[0].wall_mut(b).unwrap().curve = Some(WallCurve { bulge: 20.0 });
        // The start end is tried first and fits again.
        assert_eq!(p.make_arc_tangent(0, b), Ok(WallEnd::Start));
        // An unconnected curved wall reports why.
        let lone = p.add_wall(
            0,
            Point::new(500.0, 0.0),
            Point::new(600.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        p.floors[0].wall_mut(lone).unwrap().curve = Some(WallCurve { bulge: 10.0 });
        assert!(p
            .make_arc_tangent(0, lone)
            .unwrap_err()
            .contains("not connected"));
    }

    #[test]
    fn make_arc_tangent_at_the_end_when_only_the_end_is_joined() {
        let mut p = Project::new("t");
        let arc = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        p.floors[0].wall_mut(arc).unwrap().curve = Some(WallCurve { bulge: 10.0 });
        assert_eq!(p.make_arc_tangent(0, arc), Ok(WallEnd::End));
        let w = p.floors[0].wall(arc).unwrap();
        // The arc arrives heading north, into the next wall's start.
        let t = -w.end_tangent(WallEnd::End);
        assert!(t.x.abs() < 1e-9 && (t.y - 1.0).abs() < 1e-9, "{t:?}");
    }

    #[test]
    fn offset_curves_are_radial_and_frame_at_walks_the_arc() {
        let mut w = Wall::new(
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            8.0,
            96.0,
            WallKind::Exterior,
        );
        w.curve = WallCurve::from_radius(240.0, 120.0, true);
        let (c, r) = w.arc_center_radius().unwrap();
        // A semicircle: the radius is half the chord, the arc length pi r.
        assert!((r - 120.0).abs() < 1e-9);
        let ro = w.arc_readout().unwrap();
        assert!((ro.arc_length - std::f64::consts::PI * 120.0).abs() < 1e-6);
        assert!((ro.chord - 240.0).abs() < 1e-9 && (ro.sweep_deg - 180.0).abs() < 1e-6);
        // The curve bulges left, so the left side is the outside of the circle.
        for p in w.offset_curve(4.0, 12) {
            assert!((p.dist(c) - 124.0).abs() < 1e-9);
        }
        for p in w.offset_curve(-4.0, 12) {
            assert!((p.dist(c) - 116.0).abs() < 1e-9);
        }
        let poly = w.plan_polygon_n(12);
        assert_eq!(poly.len(), 26);
        // The ends are square to the tangent: the arc leaves heading north, so
        // start-left is 4" west of the start.
        assert!(poly[0].dist(Point::new(-4.0, 0.0)) < 1e-9);
        // Halfway along the arc is the apex, heading in the direction of travel.
        let (mid, tangent) = w.frame_at(w.path_length() * 0.5);
        assert!(mid.dist(Point::new(120.0, 120.0)) < 1e-6);
        assert!(tangent.dist(Point::new(1.0, 0.0)) < 1e-6);
        let (start, t0) = w.frame_at(0.0);
        assert!(start.dist(w.start) < 1e-9 && t0.dist(Point::new(0.0, 1.0)) < 1e-9);
        // A straight wall has no readout and a straight offset.
        let line = Wall::new(
            Point::ZERO,
            Point::new(60.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        assert!(line.arc_readout().is_none());
        assert_eq!(line.offset_curve(3.0, 2)[1], Point::new(30.0, 3.0));
    }

    /// A 6" type with its 3" main layer toward the interior: siding 2",
    /// main 3", drywall 1".
    fn off_centre_type() -> WallTypeDef {
        use crate::defaults::WallLayer;
        WallTypeDef {
            props: Default::default(),
            name: "Off-3".into(),
            layers: vec![
                WallLayer::new("Siding", 2.0, false, "Siding"),
                WallLayer::new("Frame", 3.0, true, "Framing"),
                WallLayer::new("Drywall", 1.0, false, "Drywall"),
            ],
            kind: WallKind::Exterior,
        }
    }

    fn arc_wall() -> Wall {
        let mut w = Wall::new(
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        w.curve = WallCurve::from_radius(240.0, 200.0, true);
        w
    }

    #[test]
    fn radius_to_adds_the_reference_offset() {
        let w = arc_wall();
        let ty = off_centre_type();
        let (_, r) = w.arc_center_radius().unwrap();
        let to = |a| w.radius_to(Some(&ty), a).unwrap();
        assert!((to(ResizeAbout::WallCenter) - r).abs() < 1e-9);
        // The wall bulges left and its exterior is the left side, so the
        // outer surface is the larger circle.
        assert!((to(ResizeAbout::OuterSurface) - (r + 3.0)).abs() < 1e-9);
        assert!((to(ResizeAbout::InnerSurface) - (r - 3.0)).abs() < 1e-9);
        assert!((to(ResizeAbout::MainLayerOutside) - (r + 1.0)).abs() < 1e-9);
        assert!((to(ResizeAbout::MainLayerInside) - (r - 2.0)).abs() < 1e-9);
        let line = Wall::default();
        assert!(line.radius_to(None, ResizeAbout::WallCenter).is_none());
    }

    #[test]
    fn set_radius_with_the_ends_locked_refits_the_arc() {
        let mut w = arc_wall();
        let ty = off_centre_type();
        let (s, e) = (w.start, w.end);
        let before = w.path_length();
        let k = w
            .set_radius_to(Some(&ty), ResizeAbout::OuterSurface, 303.0, ArcLock::Ends)
            .unwrap();
        assert_eq!((w.start, w.end), (s, e));
        let got = w.radius_to(Some(&ty), ResizeAbout::OuterSurface).unwrap();
        assert!((got - 303.0).abs() < 1e-6, "{got}");
        assert!((k - w.path_length() / before).abs() < 1e-9 && k < 1.0);
        // Below half the chord the arc is a semicircle, never a failure.
        w.set_radius_to(None, ResizeAbout::WallCenter, 10.0, ArcLock::Ends)
            .unwrap();
        assert!((w.arc_center_radius().unwrap().1 - 120.0).abs() < 1e-6);
    }

    #[test]
    fn a_long_way_round_arc_stays_that_way() {
        let mut w = arc_wall();
        w.curve = WallCurve::from_sweep(240.0, 250f64.to_radians(), true);
        let chord = w.length();
        assert!(w.curve.unwrap().sweep_abs(chord) > PI);
        w.set_radius_to(None, ResizeAbout::WallCenter, 180.0, ArcLock::Ends)
            .unwrap();
        assert!(w.curve.unwrap().sweep_abs(chord) > PI);
        assert!((w.arc_center_radius().unwrap().1 - 180.0).abs() < 1e-6);
    }

    #[test]
    fn set_radius_with_the_center_locked_moves_the_ends() {
        let mut w = arc_wall();
        let (c0, _) = w.arc_center_radius().unwrap();
        let sweep0 = w.curve.unwrap().sweep_abs(w.length());
        let len0 = w.path_length();
        let k = w
            .set_radius_to(None, ResizeAbout::WallCenter, 300.0, ArcLock::Center)
            .unwrap();
        let (c1, r1) = w.arc_center_radius().unwrap();
        assert!(c1.dist(c0) < 1e-9 && (r1 - 300.0).abs() < 1e-9);
        assert!((w.curve.unwrap().sweep_abs(w.length()) - sweep0).abs() < 1e-9);
        assert!((k - 1.5).abs() < 1e-9 && (w.path_length() - len0 * 1.5).abs() < 1e-6);
        assert!((w.start.dist(c1) - 300.0).abs() < 1e-9 && (w.end.dist(c1) - 300.0).abs() < 1e-9);
    }

    #[test]
    fn set_sweep_with_the_center_locked_swings_the_end() {
        let mut w = arc_wall();
        let (c0, r0) = w.arc_center_radius().unwrap();
        let start = w.start;
        w.set_sweep_locked(90f64.to_radians(), ArcLock::Center)
            .unwrap();
        let (c1, r1) = w.arc_center_radius().unwrap();
        assert_eq!(w.start, start);
        assert!(c1.dist(c0) < 1e-6 && (r1 - r0).abs() < 1e-6);
        assert!((w.curve.unwrap().sweep_abs(w.length()).to_degrees() - 90.0).abs() < 1e-6);
        assert!(w.curve.unwrap().bulge > 0.0, "still bulges left");
        // With the ends locked the chord stays.
        let chord = w.length();
        w.set_sweep_locked(120f64.to_radians(), ArcLock::Ends)
            .unwrap();
        assert!((w.length() - chord).abs() < 1e-9);
        assert!((w.curve.unwrap().sweep_abs(chord).to_degrees() - 120.0).abs() < 1e-6);
        assert!(Wall::default()
            .set_sweep_locked(1.0, ArcLock::Ends)
            .is_none());
    }

    #[test]
    fn reverse_layers_keeps_the_main_layer_in_place() {
        let (mut p, id) = proj_with_wall(6.0, WallKind::Exterior);
        let ty = off_centre_type();
        p.register_wall_type(ty.clone());
        p.floors[0].wall_mut(id).unwrap().wall_type = Some(ty.name.clone());
        // A wall running north from the end of the first, and an opening.
        let n = p.add_wall(
            0,
            Point::new(240.0, 0.0),
            Point::new(240.0, 200.0),
            6.0,
            DEFAULT_CEILING_HEIGHT,
            WallKind::Exterior,
        );
        let o = p
            .add_opening(0, id, 120.0, crate::model::OpeningKind::Window)
            .unwrap();
        let main_abs = |p: &Project| {
            let w = p.floors[0].wall(id).unwrap();
            let ty = p.wall_type_def("Off-3");
            let b = wall_layer_bands(w, ty);
            let m = b.iter().find(|b| b.is_main).unwrap();
            w.start.y + (m.outer + m.inner) * 0.5
        };
        let before = main_abs(&p);
        assert!(p.reverse_wall_layers(0, id));
        let after = main_abs(&p);
        assert!((before - after).abs() < 1e-9, "{before} {after}");
        let w = p.floors[0].wall(id).unwrap();
        assert!(w.start.y.abs() > 0.9, "the centerline moved");
        assert_eq!(w.start.y, w.end.y);
        // The wall that met the end follows it.
        let north = p.floors[0].wall(n).unwrap();
        assert!(north.start.dist(w.end) < 1e-9);
        assert_eq!(north.end, Point::new(240.0, 200.0));
        assert_eq!(
            p.floors[0]
                .openings
                .iter()
                .find(|x| x.id == o)
                .unwrap()
                .center_offset,
            120.0
        );
        // Reversing twice puts everything back.
        assert!(p.reverse_wall_layers(0, id));
        let w = p.floors[0].wall(id).unwrap();
        assert!(w.start.y.abs() < 1e-9 && w.exterior_side == Side::Left);
        assert!(
            p.floors[0]
                .wall(n)
                .unwrap()
                .start
                .dist(Point::new(240.0, 0.0))
                < 1e-9
        );
    }

    #[test]
    fn reverse_layers_on_an_arc_shifts_it_concentrically() {
        let (mut p, id) = proj_with_wall(6.0, WallKind::Exterior);
        let ty = off_centre_type();
        p.register_wall_type(ty.clone());
        {
            let w = p.floors[0].wall_mut(id).unwrap();
            w.wall_type = Some(ty.name.clone());
            w.curve = WallCurve::from_radius(240.0, 200.0, true);
        }
        let o = p
            .add_opening(0, id, 100.0, crate::model::OpeningKind::Window)
            .unwrap();
        let (c0, r0) = p.floors[0].wall(id).unwrap().arc_center_radius().unwrap();
        let len0 = p.floors[0].wall(id).unwrap().path_length();
        assert!(p.reverse_wall_layers(0, id));
        let w = p.floors[0].wall(id).unwrap();
        let (c1, r1) = w.arc_center_radius().unwrap();
        assert!(c1.dist(c0) < 1e-6);
        // Left bulge: the shift is -1" toward the +normal, which is toward the center.
        assert!((r1 - (r0 - 1.0)).abs() < 1e-6, "{r0} {r1}");
        let k = w.path_length() / len0;
        let off = p.floors[0]
            .openings
            .iter()
            .find(|x| x.id == o)
            .unwrap()
            .center_offset;
        assert!((off - 100.0 * k).abs() < 1e-6);
    }

    #[test]
    fn the_minimum_length_holds_the_openings_from_the_locked_end() {
        let mut p = Project::new("t");
        let id = p.add_wall(
            0,
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        assert_eq!(p.min_wall_length(0, id, LengthLock::Start), 0.0);
        let d = p
            .add_opening(0, id, 100.0, crate::model::OpeningKind::Door)
            .unwrap();
        let o = p.floors[0]
            .openings
            .iter()
            .find(|o| o.id == d)
            .unwrap()
            .clone();
        let start = p.min_wall_length(0, id, LengthLock::Start);
        assert!((start - (o.end_offset() + OPENING_JAMB_MARGIN)).abs() < 1e-9);
        let end = p.min_wall_length(0, id, LengthLock::End);
        assert!((end - (240.0 - o.start_offset() + OPENING_JAMB_MARGIN)).abs() < 1e-9);
        assert_eq!(p.min_wall_length(0, 999, LengthLock::Start), 0.0);
    }

    fn two_floor_house() -> (Project, Id, Id) {
        let mut p = Project::new("t");
        let low = p.add_wall(
            0,
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            p.floors[0].ceiling_height,
            WallKind::Exterior,
        );
        p.build_new_floor(false);
        let up = p.add_wall(
            1,
            Point::new(60.0, 0.0),
            Point::new(180.0, 0.0),
            6.0,
            p.floors[1].ceiling_height,
            WallKind::Exterior,
        );
        (p, low, up)
    }

    #[test]
    fn invisible_walls_fill_the_gap_under_the_upper_wall() {
        let (mut p, low, _) = two_floor_house();
        assert_eq!(p.generate_between_platforms(0), 1);
        let w = p.floors[0].wall(low).unwrap().clone();
        let g = p.floors[0]
            .walls
            .iter()
            .find(|x| x.flags.auto_generated)
            .unwrap();
        // Only the stretch the upper wall rests on.
        assert_eq!(
            (g.start, g.end),
            (Point::new(60.0, 0.0), Point::new(180.0, 0.0))
        );
        let gap = p.floors[1].elevation - (p.floors[0].elevation + w.bottom_offset + w.height);
        assert!(
            gap > 0.0 && (g.height - gap).abs() < 1e-9,
            "{} vs {gap}",
            g.height
        );
        assert_eq!(g.bottom_offset, w.height);
        assert!(g.flags.invisible && g.flags.no_room_definition && g.flags.no_locate);
        assert!(!g.flags.defines_rooms());
        // Regenerating replaces, never piles up.
        assert_eq!(p.generate_between_platforms(0), 1);
        assert_eq!(
            p.floors[0]
                .walls
                .iter()
                .filter(|x| x.flags.auto_generated)
                .count(),
            1
        );
        // The top floor has nothing above it.
        assert_eq!(p.generate_between_platforms(1), 0);
    }

    #[test]
    fn the_option_off_or_a_balloon_wall_generates_nothing() {
        let (mut p, low, _) = two_floor_house();
        p.floors[0]
            .wall_mut(low)
            .unwrap()
            .spec
            .structure
            .generate_between_platforms = false;
        assert_eq!(p.generate_between_platforms(0), 0);
        p.floors[0]
            .wall_mut(low)
            .unwrap()
            .spec
            .structure
            .generate_between_platforms = true;
        p.floors[0]
            .wall_mut(low)
            .unwrap()
            .spec
            .structure
            .ceiling_platform = CeilingPlatform::BalloonThroughCeilingAbove;
        assert_eq!(p.generate_between_platforms(0), 0);
        // A wall alone on its floor, nothing above it, makes none either.
        let (mut q, _, up) = two_floor_house();
        q.remove_wall(1, up);
        assert_eq!(q.generate_between_platforms(0), 0);
    }

    #[test]
    fn syncing_keeps_ids_until_a_wall_moves_and_drops_them_when_the_option_goes() {
        let (mut p, low, up) = two_floor_house();
        assert_eq!(p.sync_platform_walls(), 1);
        let ids = |p: &Project| -> Vec<Id> {
            p.floors[0]
                .walls
                .iter()
                .filter(|w| w.flags.auto_generated)
                .map(|w| w.id)
                .collect()
        };
        let first = ids(&p);
        assert_eq!(first.len(), 1);
        // Nothing changed: the same wall stays.
        assert_eq!(p.sync_platform_walls(), 1);
        assert_eq!(ids(&p), first);
        // The upper wall is shortened: the generated one follows.
        p.floors[1].wall_mut(up).unwrap().end = Point::new(150.0, 0.0);
        p.sync_platform_walls();
        let g = p.floors[0]
            .walls
            .iter()
            .find(|w| w.flags.auto_generated)
            .unwrap();
        assert_eq!(g.end, Point::new(150.0, 0.0));
        // The option off removes it.
        p.floors[0]
            .wall_mut(low)
            .unwrap()
            .spec
            .structure
            .generate_between_platforms = false;
        p.sync_platform_walls();
        assert!(ids(&p).is_empty());
        // And a project that never used it is left alone.
        let mut q = Project::new("t");
        assert!(!q.has_platform_walls());
        assert_eq!(q.sync_platform_walls(), 0);
    }

    #[test]
    fn two_upper_walls_make_two_generated_walls() {
        let (mut p, _, _) = two_floor_house();
        p.add_wall(
            1,
            Point::new(200.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        assert_eq!(p.generate_between_platforms(0), 2);
        assert_eq!(p.generate_all_between_platforms(), 2);
    }
}
