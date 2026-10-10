//! The Door and Window Specification tabs that shape an opening beyond its
//! size: Sash, Lites, Lintel, Arch, Hardware and Shutters, the Niche depth,
//! Door Panels "Calculate from Width", the label offset (DW-63) and the
//! manufacturer width lists that resizing snaps to (DW-27).
//!
//! Everything lives in [`OpeningSpec`], stored with the opening in
//! `Opening.extras.spec`. The defaults are what the opening was before the
//! tabs existed, so an old plan reads and draws as it did.

use super::OpeningStyle;
use crate::model::{Opening, OpeningKind};
use serde::{Deserialize, Serialize};

mod shape;
mod tabs;
pub use shape::{clip_segment_convex, inset_convex, section, CornerCut, ShapeKind, WindowShape};
pub use tabs::{
    BayRoof, BayRoofKind, BlindStyle, CurtainStyle, CurvedCasing, DoorSwing, EnergyValues,
    FramedOpening, HeaderMaterial, InteriorShutterStyle, MillworkStyle, OpeningFraming,
    OpeningInfo, OpeningMaterials, PartPaint, RoughBox, RoughMode, RoughOpening, Treatments,
    DOOR_ENERGY_TYPES, DOOR_PARTS, WINDOW_ENERGY_TYPES, WINDOW_PARTS,
};

// ----- lites -----

/// How a window or door light is divided (Lites tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LiteStyle {
    /// An even grid of Lites Across x Lites Vertical.
    #[default]
    Standard,
    /// Crossing diagonal muntins.
    Diamond,
    /// A grid with its dividers pulled in to the border.
    Prairie,
    /// Dividers at the positions typed in the tab.
    Custom,
}

impl LiteStyle {
    pub const ALL: [LiteStyle; 4] = [
        LiteStyle::Standard,
        LiteStyle::Diamond,
        LiteStyle::Prairie,
        LiteStyle::Custom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LiteStyle::Standard => "Standard",
            LiteStyle::Diamond => "Diamond",
            LiteStyle::Prairie => "Prairie",
            LiteStyle::Custom => "Custom Grid",
        }
    }
}

// ----- lintel and sill -----

/// Shape of the exterior lintel over an opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LintelStyle {
    /// One flat board.
    #[default]
    Flat,
    /// A flat board with a projecting cap.
    Cap,
    /// A flat board with a keystone block at the center.
    Keystone,
}

impl LintelStyle {
    pub const ALL: [LintelStyle; 3] = [LintelStyle::Flat, LintelStyle::Cap, LintelStyle::Keystone];

    pub fn name(self) -> &'static str {
        match self {
            LintelStyle::Flat => "Flat",
            LintelStyle::Cap => "Capped",
            LintelStyle::Keystone => "Keystone",
        }
    }
}

/// Trim over the head of an opening (Lintel tab), inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lintel {
    pub exterior: bool,
    pub interior: bool,
    /// Height of the board.
    pub height: f64,
    /// How far it stands off the wall face.
    pub depth: f64,
    /// How far it runs past the casing on each side.
    pub extend: f64,
    pub style: LintelStyle,
}

impl Default for Lintel {
    fn default() -> Self {
        Self {
            exterior: false,
            interior: false,
            height: 3.5,
            depth: 1.5,
            extend: 1.0,
            style: LintelStyle::Flat,
        }
    }
}

impl Lintel {
    pub fn any(&self) -> bool {
        self.exterior || self.interior
    }
}

/// The exterior sill under a window (Lintel tab, Exterior Sill), inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExteriorSill {
    pub enabled: bool,
    /// How far it projects past the exterior face.
    pub depth: f64,
    /// Board thickness.
    pub height: f64,
    /// How far it runs past the casing on each side.
    pub extend: f64,
}

impl Default for ExteriorSill {
    fn default() -> Self {
        Self {
            enabled: false,
            depth: 2.0,
            height: 1.5,
            extend: 1.0,
        }
    }
}

// ----- arch -----

/// Shape of an arched head (Arch tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArchType {
    #[default]
    None,
    /// A half circle over the full width.
    RoundTop,
    /// A shallow circular segment.
    Segmental,
    /// A low, pointed four-centered arch.
    Tudor,
    /// A tall pointed arch of two circular arcs.
    Gothic,
    /// A very shallow segment.
    Eyebrow,
}

impl ArchType {
    pub const ALL: [ArchType; 6] = [
        ArchType::None,
        ArchType::RoundTop,
        ArchType::Segmental,
        ArchType::Tudor,
        ArchType::Gothic,
        ArchType::Eyebrow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ArchType::None => "No Arch",
            ArchType::RoundTop => "Round Top",
            ArchType::Segmental => "Segmental",
            ArchType::Tudor => "Tudor",
            ArchType::Gothic => "Gothic",
            ArchType::Eyebrow => "Eyebrow",
        }
    }
}

/// Segments in each half of a drawn arch curve.
pub const ARCH_HALF_SEGMENTS: usize = 8;

/// An arched head: the type and its rise, inches (`0` = the type's own).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Arch {
    pub kind: ArchType,
    pub height: f64,
}

impl Arch {
    pub fn is_arched(&self) -> bool {
        self.kind != ArchType::None
    }

    /// The rise of the curve above its springline for an opening `width` wide
    /// and `opening_height` tall: the typed height, else the type's own,
    /// never more than leaves a 1" straight part under the curve.
    pub fn rise(&self, width: f64, opening_height: f64) -> f64 {
        if !self.is_arched() {
            return 0.0;
        }
        let auto = match self.kind {
            ArchType::None => 0.0,
            ArchType::RoundTop => width * 0.5,
            ArchType::Segmental => width * 0.2,
            ArchType::Tudor => width * 0.3,
            ArchType::Gothic => width * 0.75,
            ArchType::Eyebrow => width * 0.1,
        };
        let rise = if self.height > 0.0 { self.height } else { auto };
        rise.clamp(0.0, (opening_height - 1.0).max(0.0))
    }

    /// The curve as `(u, v)`, `u` across `0..width` from the left springing
    /// and `v` above the springline, from left to right.
    pub fn profile(&self, width: f64, rise: f64) -> Vec<(f64, f64)> {
        let (w, r) = (width, rise);
        if !self.is_arched() || w <= 0.0 || r <= 1e-9 {
            return vec![(0.0, 0.0), (w, 0.0)];
        }
        let n = ARCH_HALF_SEGMENTS;
        match self.kind {
            ArchType::Gothic if r > w * 0.5 => {
                // Two arcs whose centers sit on the springline.
                let c = w * 0.25 + r * r / w;
                let mut half: Vec<(f64, f64)> = Vec::new();
                let a1 = ((w * 0.5 - c) / c).clamp(-1.0, 1.0).acos();
                for i in 0..=n {
                    let a =
                        std::f64::consts::PI - (std::f64::consts::PI - a1) * i as f64 / n as f64;
                    half.push((c + c * a.cos(), c * a.sin()));
                }
                mirror(half, w)
            }
            ArchType::Tudor => tudor(w, r),
            // Round top, segmental, eyebrow (and a gothic too low to point):
            // one circular segment.
            _ => segment(w, r),
        }
    }
}

/// Left half points then their mirror, ending at `(w, 0)`; the apex is shared.
fn mirror(left: Vec<(f64, f64)>, w: f64) -> Vec<(f64, f64)> {
    let mut out = left.clone();
    for p in left.iter().rev().skip(1) {
        out.push((w - p.0, p.1));
    }
    out
}

/// A circular segment through `(0,0)`, `(w,0)` and the apex `(w/2, r)`.
fn segment(w: f64, r: f64) -> Vec<(f64, f64)> {
    let radius = (w * w * 0.25 + r * r) / (2.0 * r);
    let (cu, cv) = (w * 0.5, r - radius);
    let a0 = ((0.0 - cv) / radius).clamp(-1.0, 1.0).asin();
    // Angle of the left springing measured from +u; the arc sweeps over the top.
    let start = std::f64::consts::PI - a0;
    let end = a0;
    let total = 2 * ARCH_HALF_SEGMENTS;
    (0..=total)
        .map(|i| {
            let a = start + (end - start) * i as f64 / total as f64;
            (cu + radius * a.cos(), cv + radius * a.sin())
        })
        .collect()
}

/// A four-centered arch: a small arc at each springing, a large arc each side
/// of the apex. Falls back to a segment when the construction does not close.
fn tudor(w: f64, r: f64) -> Vec<(f64, f64)> {
    let phi = 50f64.to_radians();
    let rc = r * 0.6;
    let a = (rc, 0.0);
    let p = (rc - rc * phi.cos(), rc * phi.sin());
    let q = (w * 0.5 - p.0, r - p.1);
    let d = (phi.cos(), -phi.sin());
    let qd = q.0 * d.0 + q.1 * d.1;
    if qd <= 1e-6 {
        return segment(w, r);
    }
    let big = (q.0 * q.0 + q.1 * q.1) / (2.0 * qd);
    let b = (p.0 + big * d.0, p.1 + big * d.1);
    if b.0 < w * 0.5 - 1e-9 {
        return segment(w, r);
    }
    let mut half: Vec<(f64, f64)> = Vec::new();
    // Lower arc: angle pi down to pi - phi about A.
    let low = 3;
    for i in 0..=low {
        let t = std::f64::consts::PI - phi * i as f64 / low as f64;
        half.push((a.0 + rc * t.cos(), a.1 + rc * t.sin()));
    }
    // Upper arc about B from P to the apex.
    let a0 = (p.1 - b.1).atan2(p.0 - b.0);
    let a1 = (r - b.1).atan2(w * 0.5 - b.0);
    let up = ARCH_HALF_SEGMENTS;
    for i in 1..=up {
        let t = a0 + (a1 - a0) * i as f64 / up as f64;
        half.push((b.0 + big * t.cos(), b.1 + big * t.sin()));
    }
    // Land exactly on the apex.
    if let Some(last) = half.last_mut() {
        *last = (w * 0.5, r);
    }
    mirror(half, w)
}

// ----- hardware -----

/// A door handle (Hardware tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HandleStyle {
    None,
    Knob,
    #[default]
    Lever,
    /// A vertical bar pull.
    Pull,
}

impl HandleStyle {
    pub const ALL: [HandleStyle; 4] = [
        HandleStyle::None,
        HandleStyle::Knob,
        HandleStyle::Lever,
        HandleStyle::Pull,
    ];

    pub fn name(self) -> &'static str {
        match self {
            HandleStyle::None => "None",
            HandleStyle::Knob => "Knob",
            HandleStyle::Lever => "Lever",
            HandleStyle::Pull => "Handle",
        }
    }
}

/// Handle and hinges of a door, drawn in 3D as simple shapes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hardware {
    /// Draw the hardware at all (off keeps a plain slab).
    pub enabled: bool,
    pub handle: HandleStyle,
    /// Up from the bottom of the door to the handle, inches.
    pub handle_height: f64,
    /// In from the latch edge, inches.
    pub in_from_edge: f64,
    pub hinges: u32,
    /// In from the top and bottom of the door to the end hinges.
    pub hinge_inset: f64,
}

impl Default for Hardware {
    fn default() -> Self {
        Self {
            enabled: false,
            handle: HandleStyle::Lever,
            handle_height: 36.0,
            in_from_edge: 3.0,
            hinges: 3,
            hinge_inset: 7.0,
        }
    }
}

// ----- shutters -----

/// Exterior shutter style (Shutters tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShutterStyle {
    #[default]
    None,
    /// Flat boards with raised panels.
    Panel,
    /// Horizontal louvers.
    Louver,
}

impl ShutterStyle {
    pub const ALL: [ShutterStyle; 3] = [
        ShutterStyle::None,
        ShutterStyle::Panel,
        ShutterStyle::Louver,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ShutterStyle::None => "None",
            ShutterStyle::Panel => "Panel",
            ShutterStyle::Louver => "Louver",
        }
    }
}

/// Which sides of the opening carry a shutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShutterSides {
    /// One each side.
    #[default]
    Both,
    Left,
    Right,
}

impl ShutterSides {
    pub const ALL: [ShutterSides; 3] =
        [ShutterSides::Both, ShutterSides::Left, ShutterSides::Right];

    pub fn name(self) -> &'static str {
        match self {
            ShutterSides::Both => "Both Sides",
            ShutterSides::Left => "Left Side Only",
            ShutterSides::Right => "Right Side Only",
        }
    }

    /// `(left, right)` shutter present, looking at the exterior face.
    pub fn present(self) -> (bool, bool) {
        match self {
            ShutterSides::Both => (true, true),
            ShutterSides::Left => (true, false),
            ShutterSides::Right => (false, true),
        }
    }
}

/// Exterior shutters (windows and doors), inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shutters {
    pub style: ShutterStyle,
    /// Width of one shutter; `0` = half the opening width.
    pub width: f64,
    pub sides: ShutterSides,
    /// Paint color.
    pub color: [u8; 3],
    /// Louver pitch.
    pub louver_size: f64,
    /// Closed over the opening instead of standing beside it.
    pub closed: bool,
    /// Stand outside the casing instead of overlapping it.
    pub outside_casing: bool,
}

impl Default for Shutters {
    fn default() -> Self {
        Self {
            style: ShutterStyle::None,
            width: 0.0,
            sides: ShutterSides::Both,
            color: [38, 62, 50],
            louver_size: 1.0,
            closed: false,
            outside_casing: false,
        }
    }
}

/// Thickness of a shutter board, inches.
pub const SHUTTER_THICKNESS: f64 = 1.0;

impl Shutters {
    pub fn present(&self) -> bool {
        self.style != ShutterStyle::None
    }

    /// Width of one shutter beside an opening `opening_width` wide.
    pub fn leaf_width(&self, opening_width: f64) -> f64 {
        if self.width > 0.0 {
            self.width
        } else {
            opening_width * 0.5
        }
    }

    /// Wall offsets `(from, to)` of the shutters present, `casing` being the
    /// casing width + reveal on each side of the opening span `s0..s1`.
    pub fn spans(&self, s0: f64, s1: f64, casing: f64) -> Vec<(f64, f64)> {
        if !self.present() {
            return Vec::new();
        }
        let w = self.leaf_width(s1 - s0);
        let (left, right) = self.sides.present();
        let mut out = Vec::new();
        if self.closed {
            // Each half of the opening, hinged at its own side.
            let half = (s1 - s0) * 0.5;
            if left {
                out.push((s0, s0 + half));
            }
            if right {
                out.push((s1 - half, s1));
            }
        } else {
            let gap = if self.outside_casing { casing } else { 0.5 };
            if left {
                out.push((s0 - gap - w, s0 - gap));
            }
            if right {
                out.push((s1 + gap, s1 + gap + w));
            }
        }
        out
    }
}

// ----- casing profile -----

/// The shape of the casing boards in 3D (Casing tab, Profile).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CasingProfile {
    /// Plain boards, head and legs the same width.
    #[default]
    Flat,
    /// A head cap: the head board has a projecting cap on top of it.
    Cap,
    /// Plinth blocks at the foot of each leg and a block at each head corner.
    Plinth,
}

impl CasingProfile {
    pub const ALL: [CasingProfile; 3] = [
        CasingProfile::Flat,
        CasingProfile::Cap,
        CasingProfile::Plinth,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CasingProfile::Flat => "Flat",
            CasingProfile::Cap => "Head Cap",
            CasingProfile::Plinth => "Plinth Blocks",
        }
    }
}

// ----- the plan-wide 3D display of openings -----

/// How the 3D view shows the doors and windows of a plan, kept in the plan
/// (`Project::opening_display`): casing, jambs, sills and thresholds, and
/// whether the doors stand open.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningView3d {
    /// Casing, window stools and aprons, jambs and thresholds are built.
    pub casing: bool,
    /// Every door is shown open ("Show Doors Open").
    pub doors_open: bool,
    /// The angle hinged doors stand open at when `doors_open`, degrees.
    pub open_angle_deg: f64,
    /// Minimum Separation between window and door units (Window Defaults,
    /// manual p. 603), inches. The tools keep it in step with the defaults so
    /// the model functions (`Project::slide_opening` and friends) can read it.
    pub min_separation: f64,
    /// Ignore Casing for Opening Resize (General Plan Defaults).
    pub ignore_casing: bool,
}

impl Default for OpeningView3d {
    fn default() -> Self {
        Self {
            casing: true,
            doors_open: false,
            open_angle_deg: 90.0,
            min_separation: super::placement::DEFAULT_MIN_SEPARATION,
            ignore_casing: false,
        }
    }
}

// ----- the spec -----

// ----- plan detail, indicators and schedule data (round 14) -----

/// The Opening Indicators tab: graphic marks the plan adds to an opening
/// (DW-83). Both are off for an opening from before the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningIndicators {
    /// An "X" over a fixed unit, an arrow for the way an awning or hopper
    /// opens.
    pub show_in_plan: bool,
    /// An arrowhead at the free end of every swing arc, the way it opens.
    pub swing_arrows: bool,
}

/// What an opening recessed into its wall is recessed to (Options panel,
/// Recessed To Layer; manual pp. 589, 622, 642).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RecessTo {
    /// A typed depth from the exterior face.
    #[default]
    Depth,
    /// The exterior side of the main (structural) layer of the wall.
    MainLayer,
    /// The exterior side of the sheathing layer.
    SheathingLayer,
}

/// The Schedule tab of a door or window (L-29, DW-61): the data a schedule
/// lists besides the size. The mark itself is `Opening::schedule_number`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningSchedule {
    /// "Include in Schedule": a clear box leaves the opening out of the door
    /// or window schedule and out of the numbering.
    pub include: bool,
    pub manufacturer: String,
    pub model: String,
    pub supplier: String,
    pub comment: String,
}

impl Default for OpeningSchedule {
    fn default() -> Self {
        Self {
            include: true,
            manufacturer: String::new(),
            model: String::new(),
            supplier: String::new(),
            comment: String::new(),
        }
    }
}

/// The tab values of a door or window beyond the ones on [`Opening`] itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningSpec {
    // Sash tab (windows). Side width is `extras.sash_width`, the frame width
    // `extras.frame_width`.
    pub has_sash: bool,
    pub sash_top: f64,
    pub sash_bottom: f64,
    /// Middle width: the post between sashes and between mulled units.
    pub mullion_width: f64,
    // Lites tab.
    pub lite_style: LiteStyle,
    pub muntin_width: f64,
    /// Custom grid: vertical dividers as fractions `0..1` across the width.
    pub custom_across: Vec<f64>,
    /// Custom grid: horizontal dividers as fractions `0..1` up the height.
    pub custom_up: Vec<f64>,
    // Casing tab.
    pub casing_interior: bool,
    pub casing_exterior: bool,
    /// Draw the casing as rectangles on the wall faces in plan (DW-79).
    pub casing_in_plan: bool,
    /// Width, depth and reveal of the exterior casing; `None` uses the
    /// opening's own `casing` (the interior one) on both faces.
    pub casing_exterior_size: Option<super::Casing>,
    /// The casing profile in 3D.
    pub casing_profile: CasingProfile,
    /// Options tab, "Show Open in 3D": the leaf or panels are built open.
    pub show_open_in_3d: bool,
    /// The "Open" slider, `0..=1`: how far a door shown open in 3D is open
    /// (a hinged leaf swings that fraction of its Swing Angle, a sliding,
    /// pocket, bifold, barn or garage door travels that fraction).
    pub open_fraction: f64,
    pub lintel: Lintel,
    pub sill: ExteriorSill,
    pub arch: Arch,
    pub hardware: Hardware,
    pub shutters: Shutters,
    /// Depth of a wall niche, inches (DW-49).
    pub niche_depth: f64,
    /// Door Panels "Calculate from Width" (hinged doors become double ones
    /// from [`DOUBLE_DOOR_FROM`]).
    pub calc_panels: bool,
    /// The plan label moved from its default spot: along the wall and
    /// across it toward the room, inches (DW-63).
    pub label_offset: (f64, f64),
    /// Options tab "Swings Both Directions" (DW-35): a double-acting door
    /// draws its arcs on both sides of the wall.
    pub swings_both: bool,
    /// Sill/Threshold tab (DW-81): the thin threshold line across an
    /// exterior door in plan.
    pub threshold: bool,
    /// Jamb tab (DW-82): the door's jamb blocks beside each jamb line in plan.
    pub jamb_in_plan: bool,
    /// Jamb and Frame tabs "Size Includes Jamb / Frame" (DW-82). Clear, the
    /// opening in the wall is wider than the unit by the jamb on each side.
    pub size_includes_frame: bool,
    /// Options tab "Recessed into Wall" (DW-57): how far from the exterior
    /// wall face the leaf stands, inches; `None` keeps it on the centerline.
    pub recess_depth: Option<f64>,
    /// Recessed To Layer: what `recess_depth` follows. A layer choice is
    /// worked out from the wall's type by `Project::sync_recess_depths`.
    pub recess_to: RecessTo,
    /// Opening Indicators tab (DW-83).
    pub indicators: OpeningIndicators,
    /// Schedule tab (L-29).
    pub schedule: OpeningSchedule,
    /// Options tab "Door Swing": which leaves of a double door swing.
    pub door_swing: DoorSwing,
    /// Options tab "Swings from Center": the two leaves of a double door
    /// hinge in the middle.
    pub swings_from_center: bool,
    /// Casing tab "Curved Wall Casing".
    pub curved_casing: CurvedCasing,
    /// Rough Opening tab (DW-56).
    pub rough: RoughOpening,
    /// Framing tab (DW-114).
    pub framing: OpeningFraming,
    /// Energy Values tab (DW-115).
    pub energy: EnergyValues,
    /// Layer tab (DW-116): the layer name, `None` for the kind's own.
    pub layer: Option<String>,
    /// Materials tab (DW-117).
    pub materials: OpeningMaterials,
    /// Object Information tab (DW-118).
    pub info: OpeningInfo,
    /// Shape tab of a window (DW-121).
    pub shape: WindowShape,
    /// Treatments tab of a window (DW-123).
    pub treatments: Treatments,
    /// Options tab, Bay Roof: the roof over a bay, box or bow window.
    pub bay_roof: BayRoof,
    /// General panel, Window Type (DW-164).
    pub window_type: super::types::WindowType,
    /// General panel, Component Size of the types that have one; 0 makes the
    /// components identical.
    pub component_size: f64,
    /// General panel, Louver Size of a louvered window.
    pub louver_size: f64,
    /// The settings that follow the defaults (Use Default).
    #[serde(skip_serializing_if = "is_no_default")]
    pub dynamic: super::types::UseDefault,
    /// General panel, Hinged and Sliding doors: Interior (`Some(false)`) or
    /// Exterior (`Some(true)`) regardless of the wall; `None` follows the wall.
    pub exterior_door: Option<bool>,
    /// Window Level (manual p. 611): 0 is drawn in the layer colour and picked
    /// first; the others draw light grey.
    pub level: u8,
    /// The Mulled Unit Specification of a component of a blocked unit.
    pub mulled: Option<super::mull::MulledSpec>,
    /// The Bay/Box and Bow Window Specification (manual p. 639).
    pub bay: super::bay::BayUnit,
}

fn is_no_default(u: &super::types::UseDefault) -> bool {
    !u.any()
}

impl Default for OpeningSpec {
    fn default() -> Self {
        Self {
            has_sash: true,
            sash_top: 1.5,
            sash_bottom: 1.5,
            mullion_width: 1.5,
            lite_style: LiteStyle::Standard,
            muntin_width: 0.875,
            custom_across: Vec::new(),
            custom_up: Vec::new(),
            casing_interior: true,
            casing_exterior: true,
            casing_in_plan: false,
            casing_exterior_size: None,
            casing_profile: CasingProfile::Flat,
            show_open_in_3d: false,
            open_fraction: 1.0,
            lintel: Lintel::default(),
            sill: ExteriorSill::default(),
            arch: Arch::default(),
            hardware: Hardware::default(),
            shutters: Shutters::default(),
            niche_depth: DEFAULT_NICHE_DEPTH,
            calc_panels: false,
            label_offset: (0.0, 0.0),
            swings_both: false,
            threshold: true,
            jamb_in_plan: true,
            size_includes_frame: true,
            recess_depth: None,
            recess_to: RecessTo::Depth,
            indicators: OpeningIndicators::default(),
            schedule: OpeningSchedule::default(),
            door_swing: DoorSwing::Both,
            swings_from_center: false,
            curved_casing: CurvedCasing::Radial,
            rough: RoughOpening::default(),
            framing: OpeningFraming::default(),
            energy: EnergyValues::default(),
            layer: None,
            materials: OpeningMaterials::default(),
            info: OpeningInfo::default(),
            shape: WindowShape::default(),
            treatments: Treatments::default(),
            bay_roof: BayRoof::default(),
            window_type: super::types::WindowType::default(),
            component_size: 0.0,
            louver_size: 2.0,
            dynamic: super::types::UseDefault::default(),
            exterior_door: None,
            level: 0,
            mulled: None,
            bay: super::bay::BayUnit::default(),
        }
    }
}

/// Width from which an older casement window has two sashes, inches.
pub const DOUBLE_CASEMENT_FROM: f64 = 48.0;
/// Default depth of a wall niche, inches.
pub const DEFAULT_NICHE_DEPTH: f64 = 3.5;
/// Width from which a calculated hinged door has two leaves, inches.
pub const DOUBLE_DOOR_FROM: f64 = 40.0;
/// Default sash and frame widths, inches.
pub const DEFAULT_SASH_WIDTH: f64 = 1.5;
pub const DEFAULT_FRAME_WIDTH: f64 = 0.75;

/// The number of leaves or panels a door of `style` and `width` has when its
/// panels are calculated from the width (Door Options "Calculate from Width").
pub fn door_panel_count(style: OpeningStyle, width: f64) -> usize {
    use crate::opening_symbol::{bifold_panels, sliding_panels};
    match style {
        OpeningStyle::Sliding => sliding_panels(width),
        OpeningStyle::Bifold => bifold_panels(width),
        OpeningStyle::DoubleDoor => 2,
        OpeningStyle::Hinged if width >= DOUBLE_DOOR_FROM => 2,
        _ => 1,
    }
}

impl Opening {
    /// The tab values of this opening.
    pub fn spec(&self) -> &OpeningSpec {
        &self.extras.spec
    }

    /// The style the plan and 3D draw: a hinged or double door whose panels
    /// are calculated from the width becomes single or double by its width.
    pub fn effective_style(&self) -> OpeningStyle {
        if self.kind == OpeningKind::Door
            && self.extras.spec.calc_panels
            && matches!(self.style, OpeningStyle::Hinged | OpeningStyle::DoubleDoor)
        {
            if self.width >= DOUBLE_DOOR_FROM {
                OpeningStyle::DoubleDoor
            } else {
                OpeningStyle::Hinged
            }
        } else {
            self.style
        }
    }

    /// How many sashes a casement window has: its type's (Single, Double or
    /// Triple Casement), or for a window whose type is not a casement one (an
    /// older plan) one, and two from [`DOUBLE_CASEMENT_FROM`] wide.
    pub fn casement_sashes(&self) -> usize {
        let t = self.extras.spec.window_type;
        if t.style() == OpeningStyle::Casement {
            t.components()
        } else if self.width >= DOUBLE_CASEMENT_FROM {
            2
        } else {
            1
        }
    }

    /// Frame (window) or jamb (door) width, inches.
    pub fn frame_width(&self) -> f64 {
        let w = match self.kind {
            OpeningKind::Window => self.extras.frame_width,
            OpeningKind::Door => self.extras.jamb_width,
        };
        w.unwrap_or(DEFAULT_FRAME_WIDTH).max(0.0)
    }

    /// Sash side width, inches.
    pub fn sash_side(&self) -> f64 {
        self.extras
            .sash_width
            .unwrap_or(DEFAULT_SASH_WIDTH)
            .max(0.0)
    }

    /// Depth of a wall niche in a wall `wall_thickness` thick, inches: the
    /// typed depth (3 1/2" by default), leaving at least 1" of wall behind.
    pub fn niche_depth(&self, wall_thickness: f64) -> f64 {
        self.extras
            .spec
            .niche_depth
            .clamp(0.5, (wall_thickness - 1.0).max(0.5))
    }

    /// The casing width + reveal on each side, inches.
    pub fn casing_reach(&self) -> f64 {
        let c = self.casing.unwrap_or_default();
        c.width + c.reveal
    }

    /// Whether the opening has an arched head.
    pub fn is_arched(&self) -> bool {
        self.extras.spec.arch.is_arched()
    }

    /// Whether a window's Shape tab gives it an outline other than a
    /// rectangle (a door never has one).
    pub fn is_shaped(&self) -> bool {
        self.kind == OpeningKind::Window && self.extras.spec.shape.is_shaped()
    }
}

// ----- standard widths (DW-27) -----

/// The manufacturer widths of one style, inches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleWidths {
    pub style: OpeningStyle,
    pub widths: Vec<f64>,
}

/// Standard widths per style and the option to snap resizing to them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StandardWidths {
    /// Resizing a jamb snaps the width to the nearest standard one.
    pub snap: bool,
    pub lists: Vec<StyleWidths>,
    /// Used for a door style without a list of its own.
    pub door_fallback: Vec<f64>,
    /// Used for a window style without a list of its own.
    pub window_fallback: Vec<f64>,
}

impl Default for StandardWidths {
    fn default() -> Self {
        let s = |style, w: &[f64]| StyleWidths {
            style,
            widths: w.to_vec(),
        };
        Self {
            snap: false,
            lists: vec![
                s(
                    OpeningStyle::Hinged,
                    &[18.0, 24.0, 28.0, 30.0, 32.0, 34.0, 36.0],
                ),
                s(OpeningStyle::DoubleDoor, &[48.0, 60.0, 64.0, 72.0]),
                s(
                    OpeningStyle::Doorway,
                    &[24.0, 28.0, 30.0, 32.0, 36.0, 42.0, 48.0, 60.0, 72.0],
                ),
                s(OpeningStyle::Sliding, &[60.0, 72.0, 96.0]),
                s(OpeningStyle::Pocket, &[24.0, 28.0, 30.0, 32.0, 36.0]),
                s(OpeningStyle::Bifold, &[24.0, 30.0, 36.0, 48.0, 60.0, 72.0]),
                s(OpeningStyle::Barn, &[30.0, 36.0, 42.0]),
                s(OpeningStyle::Garage, &[96.0, 108.0, 144.0, 192.0, 216.0]),
                s(OpeningStyle::Shower, &[24.0, 28.0, 30.0, 32.0, 36.0]),
                s(OpeningStyle::Window, &[24.0, 30.0, 36.0, 48.0, 60.0, 72.0]),
                s(
                    OpeningStyle::Casement,
                    &[18.0, 24.0, 30.0, 36.0, 48.0, 60.0],
                ),
                s(OpeningStyle::SlidingWindow, &[36.0, 48.0, 60.0, 72.0]),
                s(OpeningStyle::Awning, &[24.0, 30.0, 36.0, 48.0]),
                s(OpeningStyle::Hopper, &[24.0, 30.0, 36.0]),
                s(OpeningStyle::BayWindow, &[72.0, 96.0, 120.0]),
                s(OpeningStyle::BowWindow, &[96.0, 120.0, 144.0]),
                s(OpeningStyle::BoxWindow, &[48.0, 60.0, 72.0]),
                s(OpeningStyle::PassThrough, &[36.0, 48.0, 60.0]),
                s(OpeningStyle::WallNiche, &[16.0, 24.0, 36.0]),
            ],
            door_fallback: vec![24.0, 28.0, 30.0, 32.0, 36.0],
            window_fallback: vec![24.0, 30.0, 36.0, 42.0, 48.0, 54.0, 60.0, 66.0, 72.0],
        }
    }
}

impl StandardWidths {
    /// The widths offered for `style` of `kind`, ascending. `Fixed` has a
    /// door list and a window list: the kind picks.
    pub fn for_style(&self, kind: OpeningKind, style: OpeningStyle) -> Vec<f64> {
        let own = self
            .lists
            .iter()
            .find(|l| {
                l.style == style && (style != OpeningStyle::Fixed || kind == OpeningKind::Window)
            })
            .map(|l| l.widths.clone());
        let mut v = own.unwrap_or_else(|| match kind {
            OpeningKind::Door => self.door_fallback.clone(),
            OpeningKind::Window => self.window_fallback.clone(),
        });
        v.retain(|w| *w > 0.0);
        v.sort_by(f64::total_cmp);
        v.dedup();
        v
    }

    /// The standard width nearest `width` (`width` itself with no list).
    pub fn nearest(&self, kind: OpeningKind, style: OpeningStyle, width: f64) -> f64 {
        self.for_style(kind, style)
            .into_iter()
            .min_by(|a, b| (a - width).abs().total_cmp(&(b - width).abs()))
            .unwrap_or(width)
    }

    /// The jamb edge a resize drag lands on: with snapping on, the dragged
    /// jamb is placed so the width is the nearest standard one and the other
    /// jamb stays. `edge` is the wall offset under the pointer.
    pub fn snap_edge(&self, o: &Opening, jamb: super::Jamb, edge: f64) -> f64 {
        if !self.snap {
            return edge;
        }
        match jamb {
            super::Jamb::Start => {
                let w = self.nearest(o.kind, o.effective_style(), o.end_offset() - edge);
                o.end_offset() - w
            }
            super::Jamb::End => {
                let w = self.nearest(o.kind, o.effective_style(), edge - o.start_offset());
                o.start_offset() + w
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arch_rise_defaults_and_clamps() {
        let a = Arch {
            kind: ArchType::RoundTop,
            height: 0.0,
        };
        assert_eq!(a.rise(36.0, 80.0), 18.0);
        // A typed height wins, but leaves a straight part under the curve.
        let t = Arch {
            kind: ArchType::Segmental,
            height: 200.0,
        };
        assert_eq!(t.rise(36.0, 80.0), 79.0);
        assert_eq!(Arch::default().rise(36.0, 80.0), 0.0);
    }

    #[test]
    fn every_arch_profile_runs_springing_to_springing_and_peaks_in_the_middle() {
        for kind in [
            ArchType::RoundTop,
            ArchType::Segmental,
            ArchType::Tudor,
            ArchType::Gothic,
            ArchType::Eyebrow,
        ] {
            let a = Arch { kind, height: 0.0 };
            let (w, r) = (36.0, a.rise(36.0, 80.0));
            let p = a.profile(w, r);
            assert!(p.len() >= 5, "{kind:?}");
            let (first, last) = (p[0], p[p.len() - 1]);
            assert!(
                first.0.abs() < 1e-6 && first.1.abs() < 1e-6,
                "{kind:?} {first:?}"
            );
            assert!(
                (last.0 - w).abs() < 1e-6 && last.1.abs() < 1e-6,
                "{kind:?} {last:?}"
            );
            let top = p.iter().map(|q| q.1).fold(f64::MIN, f64::max);
            assert!((top - r).abs() < 1e-6, "{kind:?} top {top} rise {r}");
            // Symmetric about the middle.
            for q in &p {
                assert!(
                    p.iter()
                        .any(|m| (m.0 - (w - q.0)).abs() < 1e-6 && (m.1 - q.1).abs() < 1e-6),
                    "{kind:?} not symmetric at {q:?}"
                );
            }
            // Never below the springline or outside the width.
            assert!(p
                .iter()
                .all(|q| q.1 >= -1e-9 && q.0 >= -1e-9 && q.0 <= w + 1e-9));
        }
        // A round top is a half circle: the 45 degree point is on the circle.
        let a = Arch {
            kind: ArchType::RoundTop,
            height: 0.0,
        };
        let p = a.profile(36.0, 18.0);
        assert!(p
            .iter()
            .all(|q| ((q.0 - 18.0).hypot(q.1) - 18.0).abs() < 1e-6));
        // Gothic is pointed: the curve rises to a point, not a crest.
        let g = Arch {
            kind: ArchType::Gothic,
            height: 0.0,
        };
        let p = g.profile(36.0, 27.0);
        let mid = p.len() / 2;
        assert!(p[mid - 1].1 < p[mid].1 && p[mid + 1].1 < p[mid].1);
    }

    #[test]
    fn calculated_panels_follow_the_width() {
        assert_eq!(door_panel_count(OpeningStyle::Hinged, 36.0), 1);
        assert_eq!(door_panel_count(OpeningStyle::Hinged, 60.0), 2);
        assert_eq!(door_panel_count(OpeningStyle::Sliding, 72.0), 2);
        assert_eq!(door_panel_count(OpeningStyle::Sliding, 144.0), 3);
        assert_eq!(door_panel_count(OpeningStyle::Bifold, 72.0), 4);
        let mut o = Opening::new(1, 60.0, OpeningKind::Door, 60.0, 80.0, 0.0);
        assert_eq!(o.effective_style(), OpeningStyle::Hinged);
        o.extras.spec.calc_panels = true;
        assert_eq!(o.effective_style(), OpeningStyle::DoubleDoor);
        o.width = 30.0;
        assert_eq!(o.effective_style(), OpeningStyle::Hinged);
        o.style = OpeningStyle::DoubleDoor;
        assert_eq!(o.effective_style(), OpeningStyle::Hinged);
    }

    #[test]
    fn standard_widths_snap_the_dragged_jamb() {
        let mut w = StandardWidths::default();
        let mut o = Opening::new(1, 50.0, OpeningKind::Door, 36.0, 80.0, 0.0);
        // Off: the edge is untouched.
        assert_eq!(w.snap_edge(&o, super::super::Jamb::End, 79.0), 79.0);
        w.snap = true;
        // Spans 32..68. End jamb to 79 = 47" wide -> nearest hinged width 36.
        assert_eq!(w.snap_edge(&o, super::super::Jamb::End, 79.0), 68.0);
        // End jamb to 60 = 28" -> 28 exactly.
        assert_eq!(w.snap_edge(&o, super::super::Jamb::End, 60.0), 60.0);
        // Start jamb to 40 = 28" wide from the end at 68.
        assert_eq!(w.snap_edge(&o, super::super::Jamb::Start, 41.0), 40.0);
        // A window uses the window list.
        o.kind = OpeningKind::Window;
        o.style = OpeningStyle::Window;
        assert_eq!(w.nearest(o.kind, o.style, 41.0), 36.0);
        assert_eq!(w.nearest(o.kind, o.style, 43.0), 48.0);
        // Garage doors have big widths.
        assert_eq!(
            w.nearest(OpeningKind::Door, OpeningStyle::Garage, 100.0),
            96.0
        );
        assert!(w
            .for_style(OpeningKind::Door, OpeningStyle::Hinged)
            .contains(&30.0));
    }

    #[test]
    fn shutter_spans_sit_beside_or_over_the_opening() {
        let mut s = Shutters {
            style: ShutterStyle::Panel,
            ..Shutters::default()
        };
        let beside = s.spans(100.0, 136.0, 3.75);
        assert_eq!(beside.len(), 2);
        // Half the opening width each, standing off the jamb by the gap.
        assert!((beside[0].1 - 99.5).abs() < 1e-9 && (beside[0].0 - 81.5).abs() < 1e-9);
        assert!((beside[1].0 - 136.5).abs() < 1e-9);
        s.outside_casing = true;
        assert!((s.spans(100.0, 136.0, 3.75)[0].1 - 96.25).abs() < 1e-9);
        s.closed = true;
        assert_eq!(
            s.spans(100.0, 136.0, 3.75),
            vec![(100.0, 118.0), (118.0, 136.0)]
        );
        s.sides = ShutterSides::Left;
        assert_eq!(s.spans(100.0, 136.0, 3.75).len(), 1);
        s.style = ShutterStyle::None;
        assert!(s.spans(100.0, 136.0, 3.75).is_empty());
    }

    #[test]
    fn the_spec_round_trips_and_old_files_get_defaults() {
        let mut o = Opening::default();
        o.extras.spec.arch.kind = ArchType::Gothic;
        o.extras.spec.shutters.style = ShutterStyle::Louver;
        o.extras.spec.label_offset = (4.0, -2.0);
        o.extras.spec.lite_style = LiteStyle::Diamond;
        let back: Opening = serde_json::from_str(&serde_json::to_string(&o).unwrap()).unwrap();
        assert_eq!(back.extras.spec, o.extras.spec);
        let old: Opening = serde_json::from_str(
            r#"{"id":1,"wall_id":2,"center_offset":50.0,"width":36.0,"height":80.0,
                "sill_height":0.0,"kind":"Door"}"#,
        )
        .unwrap();
        assert_eq!(old.extras.spec, OpeningSpec::default());
        assert_eq!(old.extras.spec.niche_depth, 3.5);
    }
}
