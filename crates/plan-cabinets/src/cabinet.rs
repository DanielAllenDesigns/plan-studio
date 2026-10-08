//! The cabinet data model, Chief-style defaults, labels and wall runs.

use plan_core::geometry::Point;
use plan_core::Id;
use serde::{Deserialize, Serialize};

use crate::face::{FaceItem, FaceLayout};
use crate::geom;
use crate::top::{treat_corners, CornerTreatment, CustomTop, Cutout, CutoutKind, EdgeProfile};

/// Carcass panel thickness, inches (an appliance bay starts inside it).
const PANEL_IN: f64 = 0.75;

/// What kind of cabinet this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CabinetKind {
    Base,
    Wall,
    FullHeight,
    Soffit,
    Shelf,
    Partition,
    /// A thin strip closing the gap between a base cabinet and a wall.
    BaseFiller,
    WallFiller,
    FullHeightFiller,
    /// L-shaped corner base cabinet (diagonal front or pie-cut).
    CornerBase,
    CornerWall,
    /// Rectangular cabinet whose one end is hidden behind its neighbour.
    BlindBase,
    BlindWall,
    /// A free-form countertop slab (island, peninsula, vanity).
    CustomCountertop,
    /// A free-form backsplash strip along a path.
    CustomBacksplash,
    /// The Custom Counter Hole tool. A tool-only kind: holes live in the
    /// countertop they cut ([`Cabinet::cutouts`]); no cabinet of this kind is
    /// ever stored.
    CounterHole,
    /// The Soffit Polygon tool. A tool-only kind: the soffit it draws is a
    /// [`CabinetKind::Soffit`] with a polygon outline ([`Cabinet::custom`]);
    /// no cabinet of this kind is ever stored.
    SoffitPolygon,
}

impl CabinetKind {
    /// Every kind, in flyout order.
    pub const ALL: [CabinetKind; 17] = [
        CabinetKind::Base,
        CabinetKind::Wall,
        CabinetKind::FullHeight,
        CabinetKind::Soffit,
        CabinetKind::Shelf,
        CabinetKind::Partition,
        CabinetKind::BaseFiller,
        CabinetKind::WallFiller,
        CabinetKind::FullHeightFiller,
        CabinetKind::CornerBase,
        CabinetKind::CornerWall,
        CabinetKind::BlindBase,
        CabinetKind::BlindWall,
        CabinetKind::CustomCountertop,
        CabinetKind::CustomBacksplash,
        CabinetKind::CounterHole,
        CabinetKind::SoffitPolygon,
    ];

    pub fn is_filler(self) -> bool {
        matches!(
            self,
            CabinetKind::BaseFiller | CabinetKind::WallFiller | CabinetKind::FullHeightFiller
        )
    }

    pub fn is_corner(self) -> bool {
        matches!(self, CabinetKind::CornerBase | CabinetKind::CornerWall)
    }

    pub fn is_blind(self) -> bool {
        matches!(self, CabinetKind::BlindBase | CabinetKind::BlindWall)
    }

    pub fn is_custom(self) -> bool {
        matches!(
            self,
            CabinetKind::CustomCountertop | CabinetKind::CustomBacksplash
        )
    }

    /// Cabinets that sit on the floor and carry a countertop by default.
    pub fn is_base_like(self) -> bool {
        matches!(
            self,
            CabinetKind::Base
                | CabinetKind::BaseFiller
                | CabinetKind::CornerBase
                | CabinetKind::BlindBase
        )
    }

    /// Hung on the wall above the counter.
    pub fn is_wall_like(self) -> bool {
        matches!(
            self,
            CabinetKind::Wall
                | CabinetKind::WallFiller
                | CabinetKind::CornerWall
                | CabinetKind::BlindWall
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            CabinetKind::Base => "Base Cabinet",
            CabinetKind::Wall => "Wall Cabinet",
            CabinetKind::FullHeight => "Full Height Cabinet",
            CabinetKind::Soffit => "Soffit",
            CabinetKind::Shelf => "Shelf",
            CabinetKind::Partition => "Partition",
            CabinetKind::BaseFiller => "Base Filler",
            CabinetKind::WallFiller => "Wall Filler",
            CabinetKind::FullHeightFiller => "Full Height Filler",
            CabinetKind::CornerBase => "Corner Base Cabinet",
            CabinetKind::CornerWall => "Corner Wall Cabinet",
            CabinetKind::BlindBase => "Blind Base Cabinet",
            CabinetKind::BlindWall => "Blind Wall Cabinet",
            CabinetKind::CustomCountertop => "Custom Countertop",
            CabinetKind::CustomBacksplash => "Custom Backsplash",
            CabinetKind::CounterHole => "Custom Counter Hole",
            CabinetKind::SoffitPolygon => "Soffit Polygon",
        }
    }
}

/// A cabinet type that Chief's library offers as its own entry but that is
/// built from one of the [`CabinetKind`]s with its own size and face. The
/// preset is kept on the cabinet for labels and the schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CabinetPreset {
    /// A base cabinet 21" deep and 34 1/2" high, with a 5" drawer over doors.
    Vanity,
    /// A full-height cabinet, 24" wide, with two stacked doors.
    Pantry,
    /// A full-height oven tower: upper door, oven and microwave openings and
    /// a drawer below.
    TallOven,
    /// A refrigerator enclosure: a 70" open bay under an upper cabinet.
    Refrigerator,
}

impl CabinetPreset {
    pub const ALL: [CabinetPreset; 4] = [
        CabinetPreset::Vanity,
        CabinetPreset::Pantry,
        CabinetPreset::TallOven,
        CabinetPreset::Refrigerator,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CabinetPreset::Vanity => "Vanity Cabinet",
            CabinetPreset::Pantry => "Pantry Cabinet",
            CabinetPreset::TallOven => "Tall Oven Cabinet",
            CabinetPreset::Refrigerator => "Refrigerator Cabinet",
        }
    }

    /// The letters that open the cabinet's label.
    pub fn code(self) -> &'static str {
        match self {
            CabinetPreset::Vanity => "VB",
            CabinetPreset::Pantry => "PN",
            CabinetPreset::TallOven => "OC",
            CabinetPreset::Refrigerator => "REF",
        }
    }

    /// The kind the preset is built from.
    pub fn kind(self) -> CabinetKind {
        match self {
            CabinetPreset::Vanity => CabinetKind::Base,
            _ => CabinetKind::FullHeight,
        }
    }

    /// Chief's sizes: `(width, depth, height)`.
    pub fn size(self) -> (f64, f64, f64) {
        match self {
            CabinetPreset::Vanity => (30.0, 21.0, 34.5),
            CabinetPreset::Pantry => (24.0, 24.0, 84.0),
            CabinetPreset::TallOven => (30.0, 24.0, 84.0),
            CabinetPreset::Refrigerator => (36.0, 25.0, 84.0),
        }
    }

    /// The appliance an open bay of this preset holds, if it has one.
    pub fn bay_appliance(self) -> Option<&'static str> {
        match self {
            CabinetPreset::Refrigerator => Some("Refrigerator"),
            CabinetPreset::TallOven => Some("Oven"),
            _ => None,
        }
    }
}

/// An open bay for an appliance, in the cabinet's local frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplianceBay {
    /// The appliance it is meant for ("Dishwasher", "Range", "Refrigerator",
    /// "Oven", ...).
    pub name: String,
    /// Inside width span `(x0, x1)`.
    pub x: (f64, f64),
    /// Bottom and top of the bay above the cabinet bottom.
    pub z: (f64, f64),
}

/// Countertop slab sitting on top of the cabinet (inside its height).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Countertop {
    pub thickness: f64,
    pub overhang_front: f64,
    pub overhang_sides: f64,
    pub overhang_back: f64,
    /// Treatment of the front corners (the back is against the wall).
    #[serde(default)]
    pub corner: CornerTreatment,
    #[serde(default = "crate::top::default_corner_size")]
    pub corner_size: f64,
    /// Shape of the exposed top edge.
    #[serde(default)]
    pub edge: EdgeProfile,
    /// Size of that edge shape, inches.
    #[serde(default = "default_edge_size")]
    pub edge_size: f64,
}

fn default_edge_size() -> f64 {
    0.75
}

impl Default for Countertop {
    /// Chief's 1 1/2" top with a 1" front overhang. Sides and back are flush
    /// (Chief's dialog defaults to 1" all round, which would collide in a run).
    fn default() -> Self {
        Self {
            thickness: 1.5,
            overhang_front: 1.0,
            overhang_sides: 0.0,
            overhang_back: 0.0,
            corner: CornerTreatment::None,
            corner_size: crate::top::default_corner_size(),
            edge: EdgeProfile::Square,
            edge_size: default_edge_size(),
        }
    }
}

/// Backsplash standing on the countertop along the back edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Backsplash {
    pub height: f64,
    pub thickness: f64,
    /// Full-height backsplash: it rises to the underside of the wall cabinet
    /// above it (or to `full_height_to` when nothing hangs over it). The
    /// stored `height` is kept in step by [`crate::fit_full_height_backsplashes`].
    #[serde(default)]
    pub full_height: bool,
    /// How far above the cabinet's own top the strip stands: the thickness
    /// of the generated countertop that took the cabinet's slab (see
    /// `Cabinet::hand_over_top`), else 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub lift: f64,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// How high a full-height backsplash goes when no wall cabinet hangs above
/// it: the bottom of a standard wall cabinet (54"), inches above the floor.
pub const FULL_HEIGHT_TO: f64 = 54.0;

impl Backsplash {
    /// A backsplash `height` high and `thickness` thick.
    pub fn new(height: f64, thickness: f64) -> Self {
        Self {
            height,
            thickness,
            full_height: false,
            lift: 0.0,
        }
    }
}

/// Recessed toe kick under a base cabinet.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToeKick {
    pub height: f64,
    /// How far the kick is set back from the front of the cabinet.
    pub depth: f64,
}

impl Default for ToeKick {
    /// Chief's 4" high, 3" deep kick.
    fn default() -> Self {
        Self {
            height: 4.0,
            depth: 3.0,
        }
    }
}

/// Handle (pull/knob) style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandleStyle {
    None,
    Knob,
    /// A bar pull on two posts: vertical on a door, horizontal on a drawer.
    Pull,
    /// A cup (bin) pull: a flat half-round plate at the top edge of a drawer.
    Cup,
    /// An edge pull: a thin lip along the free edge of a door or the top of
    /// a drawer front.
    Edge,
}

impl HandleStyle {
    pub const ALL: [HandleStyle; 5] = [
        HandleStyle::None,
        HandleStyle::Knob,
        HandleStyle::Pull,
        HandleStyle::Cup,
        HandleStyle::Edge,
    ];

    pub fn name(self) -> &'static str {
        match self {
            HandleStyle::None => "None",
            HandleStyle::Knob => "Knob",
            HandleStyle::Pull => "Pull",
            HandleStyle::Cup => "Cup Pull",
            HandleStyle::Edge => "Edge Pull",
        }
    }

    /// The style called `name` (any case; `None` for an unknown name).
    pub fn from_name(name: &str) -> Option<HandleStyle> {
        let n = name.trim().to_ascii_lowercase();
        HandleStyle::ALL
            .into_iter()
            .find(|h| h.name().to_ascii_lowercase() == n)
            .or(match n.as_str() {
                "cup" => Some(HandleStyle::Cup),
                "edge" => Some(HandleStyle::Edge),
                "bar" | "bar pull" | "handle" => Some(HandleStyle::Pull),
                _ => None,
            })
    }
}

/// How a door or drawer front is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DoorProfile {
    /// One flat slab.
    #[default]
    Slab,
    /// Frame (stiles and rails) around a recessed flat panel.
    Shaker,
    /// Frame around a panel that stands proud of the frame's inner edge.
    Raised,
}

impl DoorProfile {
    pub const ALL: [DoorProfile; 3] = [DoorProfile::Slab, DoorProfile::Shaker, DoorProfile::Raised];

    pub fn name(self) -> &'static str {
        match self {
            DoorProfile::Slab => "Slab",
            DoorProfile::Shaker => "Shaker",
            DoorProfile::Raised => "Raised Panel",
        }
    }
}

/// Door hinge style (Chief's Door Hinges group).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HingeStyle {
    #[default]
    Hidden,
    Exposed,
}

/// Door panel and handle settings (Chief's Door/Drawer tab).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DoorStyle {
    pub name: String,
    pub thickness: f64,
    pub glass: bool,
    pub handle: HandleStyle,
    /// Handle distance from the top (base/full height) or bottom (wall) of the door.
    pub handle_from_top: f64,
    /// Handle distance from the free edge of the door.
    pub handle_from_edge: f64,
    pub profile: DoorProfile,
    /// Stile/rail width of framed profiles.
    pub frame_width: f64,
    /// Centre the handle vertically instead of using `handle_from_top`.
    pub handle_centered: bool,
    pub hinge: HingeStyle,
    /// Hinge distance from the top and bottom edge of the door.
    pub hinge_from_edge: f64,
    /// Length of a pull (bar, edge or cup), inches.
    pub handle_length: f64,
}

impl Default for DoorStyle {
    fn default() -> Self {
        Self {
            name: "Lincoln Door".to_string(),
            thickness: 0.75,
            glass: false,
            handle: HandleStyle::Knob,
            handle_from_top: 1.375,
            handle_from_edge: 1.375,
            profile: DoorProfile::Slab,
            frame_width: 2.25,
            handle_centered: false,
            hinge: HingeStyle::Hidden,
            hinge_from_edge: 3.0,
            handle_length: 4.0,
        }
    }
}

impl DoorStyle {
    /// The built-in door styles offered in the Door/Drawer tab.
    pub const BUILTIN: [(&'static str, DoorProfile); 4] = [
        ("Lincoln Door", DoorProfile::Slab),
        ("Slab Door", DoorProfile::Slab),
        ("Shaker Door", DoorProfile::Shaker),
        ("Raised Panel Door", DoorProfile::Raised),
    ];

    /// Switches to the built-in style called `name`; returns false when the
    /// name is not a built-in.
    pub fn apply_builtin(&mut self, name: &str) -> bool {
        match Self::BUILTIN.iter().find(|(n, _)| *n == name) {
            Some((n, profile)) => {
                self.name = (*n).to_string();
                self.profile = *profile;
                true
            }
            None => false,
        }
    }
}

/// Drawer front and handle settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawerStyle {
    pub name: String,
    pub thickness: f64,
    pub handle: HandleStyle,
    pub profile: DoorProfile,
    /// Centre the handle on the drawer front (otherwise it sits
    /// `handle_from_top` below the top).
    pub handle_centered: bool,
    /// Handle distance from the top of the drawer front when not centred.
    pub handle_from_top: f64,
    /// Length of a pull (bar, edge or cup), inches.
    pub handle_length: f64,
}

impl Default for DrawerStyle {
    fn default() -> Self {
        Self {
            name: "Lincoln Flat Panel Drawer".to_string(),
            thickness: 0.75,
            handle: HandleStyle::Knob,
            profile: DoorProfile::Slab,
            handle_centered: true,
            handle_from_top: 1.5,
            handle_length: 4.0,
        }
    }
}

impl DrawerStyle {
    /// The built-in drawer styles.
    pub const BUILTIN: [(&'static str, DoorProfile); 4] = [
        ("Lincoln Flat Panel Drawer", DoorProfile::Slab),
        ("Slab Drawer", DoorProfile::Slab),
        ("Shaker Drawer", DoorProfile::Shaker),
        ("Raised Panel Drawer", DoorProfile::Raised),
    ];

    pub fn apply_builtin(&mut self, name: &str) -> bool {
        match Self::BUILTIN.iter().find(|(n, _)| *n == name) {
            Some((n, profile)) => {
                self.name = (*n).to_string();
                self.profile = *profile;
                true
            }
            None => false,
        }
    }
}

/// How fronts sit relative to the opening (Chief's Door/Drawer Overlay).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Overlay {
    /// Fronts cover the box; `reveal` is the gap between neighbours.
    Full { reveal: f64 },
    /// Fronts overlap the opening by `overlap` on every side.
    Traditional { overlap: f64 },
    /// Fronts sit inside the opening with `clearance` all round.
    Inset { clearance: f64 },
}

impl Default for Overlay {
    /// Chief's default: full overlay with a 1/16" reveal.
    fn default() -> Self {
        Overlay::Full { reveal: 0.0625 }
    }
}

/// How the two arms of a corner cabinet meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CornerStyle {
    /// One angled door across the corner.
    #[default]
    Diagonal,
    /// An L with two square doors meeting at the inside corner (the pie-cut
    /// fronts of a lazy susan cabinet).
    PieCut,
}

/// Corner cabinet settings. For corner kinds `Cabinet::width` is the leg
/// along the back wall and `Cabinet::depth` the leg along the side wall.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CornerSpec {
    pub style: CornerStyle,
    /// Round rotating shelves in a pie-cut corner.
    pub lazy_susan: bool,
    /// Depth of the two arms (24" base, 12" wall).
    pub arm_depth: f64,
}

impl Default for CornerSpec {
    fn default() -> Self {
        Self {
            style: CornerStyle::Diagonal,
            lazy_susan: false,
            arm_depth: 24.0,
        }
    }
}

/// Which end of a blind cabinet is hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlindSide {
    #[default]
    Left,
    Right,
}

/// The hidden stretch of a blind corner cabinet: a solid panel at one end
/// that tucks behind the neighbouring cabinet.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BlindSpec {
    pub side: BlindSide,
    pub blind_width: f64,
}

impl Default for BlindSpec {
    fn default() -> Self {
        Self {
            side: BlindSide::Left,
            blind_width: 15.0,
        }
    }
}

/// Crown, light rail and similar trim added to a cabinet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoldingKind {
    /// Along the top of wall and tall cabinets.
    Crown,
    /// Along the bottom of wall cabinets, hiding under-cabinet lights.
    LightRail,
}

/// One molding run: front plus both side returns.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Molding {
    pub kind: MoldingKind,
    /// How far it projects from the cabinet.
    pub projection: f64,
    pub height: f64,
}

impl Molding {
    pub fn crown() -> Self {
        Self {
            kind: MoldingKind::Crown,
            projection: 2.5,
            height: 3.5,
        }
    }

    pub fn light_rail() -> Self {
        Self {
            kind: MoldingKind::LightRail,
            projection: 0.75,
            height: 2.0,
        }
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            MoldingKind::Crown => "Crown Molding",
            MoldingKind::LightRail => "Light Rail",
        }
    }
}

/// A material choice for one cabinet part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MaterialChoice {
    /// The part's usual stand-in.
    #[default]
    Default,
    Wood,
    Painted,
    Stone,
    Concrete,
    Metal,
    Glass,
}

impl MaterialChoice {
    pub const ALL: [MaterialChoice; 7] = [
        MaterialChoice::Default,
        MaterialChoice::Wood,
        MaterialChoice::Painted,
        MaterialChoice::Stone,
        MaterialChoice::Concrete,
        MaterialChoice::Metal,
        MaterialChoice::Glass,
    ];

    pub fn name(self) -> &'static str {
        match self {
            MaterialChoice::Default => "Default",
            MaterialChoice::Wood => "Wood",
            MaterialChoice::Painted => "Painted",
            MaterialChoice::Stone => "Stone",
            MaterialChoice::Concrete => "Concrete",
            MaterialChoice::Metal => "Metal",
            MaterialChoice::Glass => "Glass",
        }
    }
}

/// Per-part materials (Chief's Materials tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PartMaterials {
    pub carcass: MaterialChoice,
    pub door: MaterialChoice,
    pub drawer: MaterialChoice,
    pub countertop: MaterialChoice,
    pub backsplash: MaterialChoice,
    pub toe_kick: MaterialChoice,
    pub molding: MaterialChoice,
}

/// One of the four vertical faces of a cabinet box (Chief's Front/Sides/Back).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FaceSide {
    #[default]
    Front,
    Left,
    Right,
    Back,
}

impl FaceSide {
    pub const ALL: [FaceSide; 4] = [
        FaceSide::Front,
        FaceSide::Left,
        FaceSide::Right,
        FaceSide::Back,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FaceSide::Front => "Front",
            FaceSide::Left => "Left",
            FaceSide::Right => "Right",
            FaceSide::Back => "Back",
        }
    }
}

/// What a side or the back of a cabinet is made of (Chief's Side Type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SideKind {
    /// The plain carcass panel.
    #[default]
    Plain,
    /// A finished slab panel in the door material.
    Finished,
    /// No panel: the box is open on this side.
    Open,
    /// Face items (doors, drawers, panels) laid out like the front.
    CustomFace,
}

impl SideKind {
    pub const ALL: [SideKind; 4] = [
        SideKind::Plain,
        SideKind::Finished,
        SideKind::Open,
        SideKind::CustomFace,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SideKind::Plain => "Plain Panel",
            SideKind::Finished => "Finished Panel",
            SideKind::Open => "Open",
            SideKind::CustomFace => "Custom Face",
        }
    }
}

/// The Left, Right or Back face of a cabinet when it is not the plain panel.
/// Its layout is used when `kind` is [`SideKind::CustomFace`]; the face is
/// `depth` wide (Left, Right) or `width` wide (Back), and runs left to right
/// as seen from outside.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SideFace {
    pub side: FaceSide,
    pub kind: SideKind,
    pub layout: FaceLayout,
}

/// A parametric cabinet. See the crate docs for the local frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cabinet {
    pub id: Id,
    pub kind: CabinetKind,
    /// Plan position of the local origin (back-left corner).
    pub position: Point,
    /// Counter-clockwise rotation in radians. At `0` the back faces -Y and the
    /// front faces +Y.
    pub angle: f64,
    pub width: f64,
    pub depth: f64,
    /// Overall height, including the countertop.
    pub height: f64,
    /// Bottom of the cabinet above the floor (0 for base, 54 for wall).
    pub elevation: f64,
    pub countertop: Option<Countertop>,
    pub backsplash: Option<Backsplash>,
    pub toe_kick: Option<ToeKick>,
    pub face: FaceLayout,
    pub door_style: DoorStyle,
    pub drawer_style: DrawerStyle,
    pub overlay: Overlay,
    /// Face-frame (stiles and rails) construction rather than frameless.
    pub framed: bool,
    /// Optional label override; empty means [`auto_label`]. May hold macros,
    /// see [`expand_label`].
    pub label: String,
    /// Corner cabinet settings (corner kinds only).
    #[serde(default)]
    pub corner: Option<CornerSpec>,
    /// Hidden end of a blind cabinet (blind kinds only).
    #[serde(default)]
    pub blind: Option<BlindSpec>,
    /// Outline and edge of a custom countertop or backsplash.
    #[serde(default)]
    pub custom: Option<CustomTop>,
    /// Sink and cooktop holes in the countertop (local frame).
    #[serde(default)]
    pub cutouts: Vec<Cutout>,
    /// Name of the appliance that fills this cabinet's opening (a dishwasher
    /// or range bay), if any.
    #[serde(default)]
    pub appliance: Option<String>,
    #[serde(default)]
    pub moldings: Vec<Molding>,
    #[serde(default)]
    pub materials: PartMaterials,
    /// Draw door swings and open drawers in plan (Chief's Opening Indicators).
    #[serde(default)]
    pub indicators: bool,
    /// Left, Right and Back faces that are not the plain carcass panel
    /// (rectangular cabinets).
    #[serde(default)]
    pub sides: Vec<SideFace>,
    /// Generated countertops only: the cabinets whose slabs this top joined
    /// and what they gave up (see [`crate::release_joined_top`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joined: Vec<crate::top::JoinedSource>,
    /// The plan label's offset from its default place, plan inches (the
    /// label's drag handle moves it). The label is drawn on the layer
    /// "Cabinets, Labels".
    #[serde(default, skip_serializing_if = "is_zero_point")]
    pub label_offset: Point,
    /// Opening Indicators in 3D: doors stand open with the shelves inside
    /// showing, and drawers are pulled out with their boxes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub indicators_3d: bool,
    /// The library type this cabinet was made from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<CabinetPreset>,
}

fn is_zero_point(p: &Point) -> bool {
    p.x == 0.0 && p.y == 0.0
}

impl Cabinet {
    fn blank(kind: CabinetKind, width: f64, depth: f64, height: f64, elevation: f64) -> Self {
        Self {
            id: 0,
            kind,
            position: Point::ZERO,
            angle: 0.0,
            width,
            depth,
            height,
            elevation,
            countertop: None,
            backsplash: None,
            toe_kick: None,
            face: FaceLayout::empty(),
            door_style: DoorStyle::default(),
            drawer_style: DrawerStyle::default(),
            overlay: Overlay::default(),
            framed: true,
            label: String::new(),
            corner: None,
            blind: None,
            custom: None,
            cutouts: Vec::new(),
            appliance: None,
            moldings: Vec::new(),
            materials: PartMaterials::default(),
            indicators: false,
            sides: Vec::new(),
            joined: Vec::new(),
            label_offset: Point::ZERO,
            indicators_3d: false,
            preset: None,
        }
    }

    /// The Left, Right or Back face `side` when it is not the plain panel.
    pub fn side_face(&self, side: FaceSide) -> Option<&SideFace> {
        self.sides.iter().find(|s| s.side == side)
    }

    /// What `side` is made of.
    pub fn side_kind(&self, side: FaceSide) -> SideKind {
        self.side_face(side).map_or(SideKind::Plain, |s| s.kind)
    }

    /// Sets what Left, Right or Back is made of (a plain panel is stored as
    /// nothing). The front is [`Cabinet::face`], not a side face.
    pub fn set_side_face(&mut self, side: FaceSide, kind: SideKind, layout: FaceLayout) {
        self.sides.retain(|s| s.side != side);
        if side != FaceSide::Front && kind != SideKind::Plain {
            self.sides.push(SideFace { side, kind, layout });
        }
    }

    /// The width of `side` as seen from outside: the width for the Front and
    /// Back, the depth for the Left and Right.
    pub fn side_width(&self, side: FaceSide) -> f64 {
        match side {
            FaceSide::Front => self.face_width(),
            FaceSide::Back => self.width,
            FaceSide::Left | FaceSide::Right => self.depth,
        }
    }

    /// A Chief default base cabinet: 24" deep, 36" high including a 1 1/2"
    /// top, 4" x 3" toe kick, drawer over door face.
    pub fn base(width: f64) -> Self {
        let mut c = Self::blank(CabinetKind::Base, width, 24.0, 36.0, 0.0);
        c.countertop = Some(Countertop::default());
        c.toe_kick = Some(ToeKick::default());
        c.face = FaceLayout::base_default(c.face_height());
        c
    }

    /// A Chief default wall cabinet: 12" deep, 30" high at 54".
    pub fn wall(width: f64) -> Self {
        let mut c = Self::blank(CabinetKind::Wall, width, 12.0, 30.0, 54.0);
        c.face = FaceLayout::wall_default(c.face_height());
        c
    }

    /// A Chief default full-height (pantry) cabinet: 24" deep, 84" high.
    pub fn full_height(width: f64) -> Self {
        let mut c = Self::blank(CabinetKind::FullHeight, width, 24.0, 84.0, 0.0);
        c.toe_kick = Some(ToeKick::default());
        c.face = FaceLayout::full_height_default(c.face_height());
        c
    }

    /// A base cabinet with a sink face (B36-SB style label).
    pub fn sink_base(width: f64) -> Self {
        let mut c = Self::base(width);
        c.face = FaceLayout::sink_base();
        c
    }

    /// A filler strip of `kind` (one of the three filler kinds), `width`
    /// inches wide, with the matching cabinet's depth, height and top.
    pub fn filler(kind: CabinetKind, width: f64) -> Self {
        let mut c = match kind {
            CabinetKind::WallFiller => Self::wall(width),
            CabinetKind::FullHeightFiller => Self::full_height(width),
            _ => Self::base(width),
        };
        c.kind = match kind {
            CabinetKind::WallFiller | CabinetKind::FullHeightFiller => kind,
            _ => CabinetKind::BaseFiller,
        };
        c.framed = false;
        c.face = FaceLayout::filler_panel();
        c
    }

    /// An L-shaped corner base cabinet: `leg` inches along both walls with
    /// 24" arms, a diagonal front by default.
    pub fn corner_base(leg: f64) -> Self {
        let mut c = Self::base(leg);
        c.kind = CabinetKind::CornerBase;
        c.depth = leg;
        c.corner = Some(CornerSpec::default());
        c
    }

    /// A corner wall cabinet (24" legs, 12" arms, hung at 54").
    pub fn corner_wall(leg: f64) -> Self {
        let mut c = Self::wall(leg);
        c.kind = CabinetKind::CornerWall;
        c.depth = leg;
        c.corner = Some(CornerSpec {
            arm_depth: 12.0,
            ..CornerSpec::default()
        });
        c
    }

    /// Makes a corner cabinet pie-cut, optionally with lazy-susan shelves.
    pub fn with_pie_cut(mut self, lazy_susan: bool) -> Self {
        if let Some(c) = self.corner.as_mut() {
            c.style = CornerStyle::PieCut;
            c.lazy_susan = lazy_susan;
        }
        self
    }

    /// A blind corner base: `width` overall, `blind_width` of it hidden.
    pub fn blind_base(width: f64, blind_width: f64, side: BlindSide) -> Self {
        let mut c = Self::base(width);
        c.kind = CabinetKind::BlindBase;
        c.blind = Some(BlindSpec { side, blind_width });
        c
    }

    pub fn blind_wall(width: f64, blind_width: f64, side: BlindSide) -> Self {
        let mut c = Self::wall(width);
        c.kind = CabinetKind::BlindWall;
        c.blind = Some(BlindSpec { side, blind_width });
        c
    }

    /// A base cabinet that is an empty bay for an appliance. A dishwasher
    /// keeps the countertop above it; a range loses countertop and toe kick.
    pub fn appliance_opening(name: &str, width: f64) -> Self {
        let mut c = Self::base(width);
        c.appliance = Some(name.to_string());
        c.framed = false;
        c.face = FaceLayout::opening();
        if !name.eq_ignore_ascii_case("Dishwasher") {
            c.countertop = None;
            c.toe_kick = None;
            c.height = 34.5;
        }
        c
    }

    /// Turns this cabinet into an open bay for `name` (a dishwasher, range,
    /// ...) or, with `None`, back into an ordinary cabinet with the default
    /// face for its kind. The countertop and toe kick are left as they are.
    pub fn set_appliance(&mut self, name: Option<&str>) {
        match name {
            Some(n) => {
                self.appliance = Some(n.to_string());
                self.framed = false;
                self.face = FaceLayout::opening();
            }
            None => {
                if self.appliance.take().is_some() {
                    self.framed = true;
                    let h = self.face_height();
                    self.face = match self.kind {
                        k if k.is_wall_like() => FaceLayout::wall_default(h),
                        CabinetKind::FullHeight => FaceLayout::full_height_default(h),
                        _ => FaceLayout::base_default(h),
                    };
                }
            }
        }
    }

    /// A 24" dishwasher bay.
    pub fn dishwasher_opening() -> Self {
        Self::appliance_opening("Dishwasher", 24.0)
    }

    /// A range bay (30" by default).
    pub fn range_opening(width: f64) -> Self {
        Self::appliance_opening("Range", width)
    }

    /// A vanity: a base cabinet 21" deep and 34 1/2" high (36" with its
    /// 1 1/2" top is the kitchen standard; a vanity runs lower), a 5" drawer
    /// over one door, or two doors from 30" wide.
    pub fn vanity(width: f64) -> Self {
        let (_, depth, height) = CabinetPreset::Vanity.size();
        let mut c = Self::base(width);
        c.depth = depth;
        c.height = height;
        c.preset = Some(CabinetPreset::Vanity);
        let mut face = FaceLayout::base_default(c.face_height());
        if width >= 30.0 {
            face.items = vec![
                FaceItem::Separation { height: 1.5 },
                FaceItem::Drawer { height: 5.0 },
                FaceItem::Separation { height: 1.5 },
                FaceItem::DoubleDoor { height: 0.0 },
                FaceItem::Separation { height: 1.5 },
            ];
        } else if let Some(FaceItem::Drawer { height }) = face.items.get_mut(1) {
            *height = 5.0;
        }
        c.face = face;
        c
    }

    /// A pantry: a full-height cabinet with two stacked doors.
    pub fn pantry(width: f64) -> Self {
        let (_, depth, height) = CabinetPreset::Pantry.size();
        let mut c = Self::full_height(width);
        c.depth = depth;
        c.height = height;
        c.preset = Some(CabinetPreset::Pantry);
        c.face = FaceLayout::full_height_default(c.face_height());
        c
    }

    /// A tall oven cabinet: an upper door, a 28 1/2" oven opening, a 17"
    /// microwave opening and a drawer under them.
    pub fn tall_oven(width: f64) -> Self {
        let (_, depth, height) = CabinetPreset::TallOven.size();
        let mut c = Self::full_height(width);
        c.depth = depth;
        c.height = height;
        c.preset = Some(CabinetPreset::TallOven);
        c.face = FaceLayout {
            items: vec![
                FaceItem::Separation { height: 1.5 },
                FaceItem::DoorAuto { height: 0.0 },
                FaceItem::Separation { height: 1.5 },
                FaceItem::Opening { height: 17.0 },
                FaceItem::Separation { height: 1.5 },
                FaceItem::Opening { height: 28.5 },
                FaceItem::Separation { height: 1.5 },
                FaceItem::Drawer { height: 8.0 },
                FaceItem::Separation { height: 1.5 },
            ],
            frame_width: 1.5,
        };
        c
    }

    /// A refrigerator enclosure: an open 70" bay (with the toe kick removed)
    /// under one upper door.
    pub fn refrigerator(width: f64) -> Self {
        let (_, depth, height) = CabinetPreset::Refrigerator.size();
        let mut c = Self::full_height(width);
        c.depth = depth;
        c.height = height;
        c.toe_kick = None;
        c.preset = Some(CabinetPreset::Refrigerator);
        c.face = FaceLayout {
            items: vec![
                FaceItem::Separation { height: 1.5 },
                FaceItem::DoubleDoor { height: 0.0 },
                FaceItem::Separation { height: 1.5 },
                FaceItem::Opening { height: 70.0 },
            ],
            frame_width: 1.5,
        };
        c
    }

    /// A cabinet made from a library type, `width` wide.
    pub fn from_preset(preset: CabinetPreset, width: f64) -> Self {
        match preset {
            CabinetPreset::Vanity => Self::vanity(width),
            CabinetPreset::Pantry => Self::pantry(width),
            CabinetPreset::TallOven => Self::tall_oven(width),
            CabinetPreset::Refrigerator => Self::refrigerator(width),
        }
    }

    /// The open bays of this cabinet an appliance can sit in, local frame:
    /// the whole inside of an appliance opening, or each `Opening` item of a
    /// tall oven or refrigerator cabinet.
    pub fn appliance_bays(&self) -> Vec<ApplianceBay> {
        let inside = (PANEL_IN, (self.width - PANEL_IN).max(PANEL_IN));
        let z0 = self.toe_kick.map_or(0.0, |t| t.height);
        if let Some(name) = &self.appliance {
            let top = self.height - self.countertop.map_or(0.0, |c| c.thickness);
            return vec![ApplianceBay {
                name: name.clone(),
                x: inside,
                z: (z0, top),
            }];
        }
        let Some(name) = self.preset.and_then(CabinetPreset::bay_appliance) else {
            return Vec::new();
        };
        let Ok(items) = self.face.resolve(self.face_height(), self.face_width()) else {
            return Vec::new();
        };
        let mut bays: Vec<ApplianceBay> = items
            .iter()
            .filter(|r| matches!(r.item, FaceItem::Opening { .. }))
            .map(|r| {
                let (_, y, _, h) = r.rect;
                ApplianceBay {
                    name: name.to_string(),
                    x: inside,
                    z: (z0 + y, z0 + y + h),
                }
            })
            .collect();
        // A tall oven's two openings: the larger holds the oven, the smaller
        // the microwave.
        if self.preset == Some(CabinetPreset::TallOven) && bays.len() == 2 {
            let small = if bays[0].z.1 - bays[0].z.0 < bays[1].z.1 - bays[1].z.0 {
                0
            } else {
                1
            };
            bays[small].name = "Microwave".to_string();
        }
        bays
    }

    /// The hardware named in the schedule: the door handle style, with the
    /// drawer handle after a slash when it differs.
    pub fn hardware_name(&self) -> String {
        let (d, w) = (self.door_style.handle, self.drawer_style.handle);
        if d == w {
            d.name().to_string()
        } else {
            format!("{} / {}", d.name(), w.name())
        }
    }

    /// The finish named in the schedule: the door material, or the carcass
    /// when the doors keep their default.
    pub fn finish_name(&self) -> String {
        let m = self.materials;
        let pick = if m.door != MaterialChoice::Default {
            m.door
        } else {
            m.carcass
        };
        pick.name().to_string()
    }

    /// A free-form countertop over `outline` (plan coordinates, any
    /// winding): the slab is `thickness` thick with its top at `top`.
    pub fn custom_countertop(outline: &[Point], thickness: f64, top: f64) -> Option<Self> {
        let ring = geom::ccw(outline);
        let (lo, hi) = geom::bbox(&ring)?;
        if ring.len() < 3 || geom::area(&ring) < 1e-6 {
            return None;
        }
        let mut c = Self::blank(
            CabinetKind::CustomCountertop,
            hi.x - lo.x,
            hi.y - lo.y,
            thickness,
            top - thickness,
        );
        c.position = lo;
        c.framed = false;
        c.custom = Some(CustomTop {
            outline: ring.iter().map(|p| p.sub(lo)).collect(),
            thickness,
            edge: EdgeProfile::Square,
            edge_size: 0.75,
            closed: true,
            corner: CornerTreatment::None,
            corner_size: crate::top::default_corner_size(),
        });
        Some(c)
    }

    /// A backsplash strip of `thickness` along the open `path`, `height`
    /// high, standing on `base` (usually the countertop top).
    pub fn custom_backsplash(
        path: &[Point],
        height: f64,
        thickness: f64,
        base: f64,
    ) -> Option<Self> {
        if path.len() < 2 {
            return None;
        }
        let strip = geom::thicken_path(path, thickness);
        let (lo, hi) = geom::bbox(&strip)?;
        let mut c = Self::blank(
            CabinetKind::CustomBacksplash,
            hi.x - lo.x,
            hi.y - lo.y,
            height,
            base,
        );
        c.position = lo;
        c.framed = false;
        c.custom = Some(CustomTop {
            outline: path.iter().map(|p| p.sub(lo)).collect(),
            thickness,
            edge: EdgeProfile::Square,
            edge_size: 0.0,
            closed: false,
            corner: CornerTreatment::None,
            corner_size: crate::top::default_corner_size(),
        });
        Some(c)
    }

    /// A soffit over the polygon `ring` (plan coordinates): `height` thick,
    /// its bottom `elevation` above the floor. `None` for fewer than three
    /// distinct corners.
    pub fn soffit_polygon(ring: &[Point], height: f64, elevation: f64) -> Option<Self> {
        let ring = geom::ccw(ring);
        if ring.len() < 3 || geom::area(&ring) < 1e-6 {
            return None;
        }
        let (lo, hi) = geom::bbox(&ring)?;
        let mut c = Self::blank(
            CabinetKind::Soffit,
            hi.x - lo.x,
            hi.y - lo.y,
            height,
            elevation,
        );
        c.position = lo;
        c.framed = false;
        c.custom = Some(CustomTop {
            outline: ring.iter().map(|p| p.sub(lo)).collect(),
            thickness: height,
            edge: EdgeProfile::Square,
            edge_size: 0.0,
            closed: true,
            corner: CornerTreatment::None,
            corner_size: crate::top::default_corner_size(),
        });
        Some(c)
    }

    /// A cabinet of any kind with sensible defaults: base, wall and full
    /// height use their Chief defaults; the rest are bare boxes (soffit 12x12
    /// at 84", shelf 12" deep at 48", partition 24" deep x 36" high). Custom
    /// kinds start as a `width` square (the tools replace the outline).
    pub fn new(kind: CabinetKind, width: f64) -> Self {
        match kind {
            CabinetKind::Base => Self::base(width),
            CabinetKind::Wall => Self::wall(width),
            CabinetKind::FullHeight => Self::full_height(width),
            CabinetKind::Soffit => Self::blank(kind, width, 12.0, 12.0, 84.0),
            CabinetKind::Shelf => Self::blank(kind, width, 12.0, 0.75, 48.0),
            CabinetKind::Partition => Self::blank(kind, width, 24.0, 36.0, 0.0),
            CabinetKind::BaseFiller | CabinetKind::WallFiller | CabinetKind::FullHeightFiller => {
                Self::filler(kind, width)
            }
            CabinetKind::CornerBase => Self::corner_base(width),
            CabinetKind::CornerWall => Self::corner_wall(width),
            CabinetKind::BlindBase => Self::blind_base(width, 15.0, BlindSide::Left),
            CabinetKind::BlindWall => Self::blind_wall(width, 12.0, BlindSide::Left),
            CabinetKind::CustomCountertop => {
                let sq = [
                    Point::ZERO,
                    Point::new(width, 0.0),
                    Point::new(width, width),
                    Point::new(0.0, width),
                ];
                Self::custom_countertop(&sq, 1.5, 36.0).unwrap_or_else(|| Self::base(width))
            }
            CabinetKind::CustomBacksplash => {
                Self::custom_backsplash(&[Point::ZERO, Point::new(width, 0.0)], 4.0, 0.5, 36.0)
                    .unwrap_or_else(|| Self::base(width))
            }
            // Never stored: an inert shelf-thin placeholder.
            CabinetKind::CounterHole => Self::blank(kind, width, width, 0.75, 36.0),
            // Never stored: an inert soffit-sized placeholder.
            CabinetKind::SoffitPolygon => Self::blank(kind, width, 12.0, 12.0, 84.0),
        }
    }

    /// Height available to the face: above the toe kick, below the countertop.
    pub fn face_height(&self) -> f64 {
        let toe = self.toe_kick.map_or(0.0, |t| t.height);
        let top = self.countertop.map_or(0.0, |c| c.thickness);
        (self.height - toe - top).max(0.0)
    }

    /// Width the face items share: the whole width, less a blind cabinet's
    /// hidden end.
    pub fn face_width(&self) -> f64 {
        (self.width - self.blind.map_or(0.0, |b| b.blind_width)).max(0.0)
    }

    /// Map a local-frame point (inches) to plan coordinates.
    pub fn to_plan(&self, local: Point) -> Point {
        let (s, c) = self.angle.sin_cos();
        Point::new(
            self.position.x + local.x * c - local.y * s,
            self.position.y + local.x * s + local.y * c,
        )
    }

    /// The bounding box footprint in plan, counter-clockwise from the
    /// back-left corner. For corner and custom kinds see [`Cabinet::footprint`].
    pub fn corners(&self) -> [Point; 4] {
        [
            self.to_plan(Point::new(0.0, 0.0)),
            self.to_plan(Point::new(self.width, 0.0)),
            self.to_plan(Point::new(self.width, self.depth)),
            self.to_plan(Point::new(0.0, self.depth)),
        ]
    }

    /// The arm depth of a corner cabinet, clamped to its legs.
    fn arm(&self) -> f64 {
        let a = self.corner.map_or(24.0, |c| c.arm_depth);
        a.clamp(1.0, self.width.min(self.depth))
    }

    /// The true outline in the local frame: the L or pentagon of a corner
    /// cabinet, the outline of a custom countertop, the strip of a custom
    /// backsplash, otherwise the width by depth rectangle.
    pub fn footprint_local(&self) -> Vec<Point> {
        let (w, d) = (self.width, self.depth);
        if let Some(custom) = &self.custom {
            return custom.footprint();
        }
        if self.kind.is_corner() {
            let a = self.arm();
            let style = self.corner.map_or(CornerStyle::Diagonal, |c| c.style);
            return match style {
                CornerStyle::PieCut => vec![
                    Point::new(0.0, 0.0),
                    Point::new(w, 0.0),
                    Point::new(w, a),
                    Point::new(a, a),
                    Point::new(a, d),
                    Point::new(0.0, d),
                ],
                CornerStyle::Diagonal => vec![
                    Point::new(0.0, 0.0),
                    Point::new(w, 0.0),
                    Point::new(w, a),
                    Point::new(a, d),
                    Point::new(0.0, d),
                ],
            };
        }
        vec![
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, d),
            Point::new(0.0, d),
        ]
    }

    /// [`Cabinet::footprint_local`] in plan coordinates.
    pub fn footprint(&self) -> Vec<Point> {
        self.footprint_local()
            .into_iter()
            .map(|p| self.to_plan(p))
            .collect()
    }

    /// The countertop outline in the local frame (overhangs included), or
    /// `None` when the cabinet has no countertop.
    pub fn top_local(&self) -> Option<Vec<Point>> {
        self.top_ring(true)
    }

    /// [`Cabinet::top_local`] without the corner treatment: the ring joined
    /// tops are made from.
    pub fn top_local_square(&self) -> Option<Vec<Point>> {
        self.top_ring(false)
    }

    fn top_ring(&self, treated: bool) -> Option<Vec<Point>> {
        if let Some(custom) = &self.custom {
            return (self.kind == CabinetKind::CustomCountertop).then(|| {
                if treated {
                    custom.treated()
                } else {
                    custom.footprint()
                }
            });
        }
        let t = self.countertop?;
        let (w, d) = (self.width, self.depth);
        if self.kind.is_corner() {
            let a = self.arm();
            let o = t.overhang_front;
            let style = self.corner.map_or(CornerStyle::Diagonal, |c| c.style);
            return Some(match style {
                CornerStyle::PieCut => vec![
                    Point::new(0.0, 0.0),
                    Point::new(w, 0.0),
                    Point::new(w, a + o),
                    Point::new(a + o, a + o),
                    Point::new(a + o, d),
                    Point::new(0.0, d),
                ],
                CornerStyle::Diagonal => {
                    // Offset the diagonal front outward by the overhang.
                    let n = Point::new(d - a, w - a).normalized();
                    let ny = if n.y.abs() < 1e-9 { 1.0 } else { n.y };
                    let nx = if n.x.abs() < 1e-9 { 1.0 } else { n.x };
                    vec![
                        Point::new(0.0, 0.0),
                        Point::new(w, 0.0),
                        Point::new(w, a + o / ny),
                        Point::new(a + o / nx, d),
                        Point::new(0.0, d),
                    ]
                }
            });
        }
        let (x0, x1) = (-t.overhang_sides, w + t.overhang_sides);
        let (y0, y1) = (-t.overhang_back, d + t.overhang_front);
        let ring = vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ];
        if treated {
            // Only the front corners are exposed.
            Some(treat_corners(&ring, t.corner, t.corner_size, |p| {
                p.y > d * 0.5
            }))
        } else {
            Some(ring)
        }
    }

    /// [`Cabinet::top_local`] in plan coordinates.
    pub fn top_polygon(&self) -> Option<Vec<Point>> {
        self.top_local()
            .map(|r| r.into_iter().map(|p| self.to_plan(p)).collect())
    }

    /// [`Cabinet::top_local_square`] in plan coordinates.
    pub fn top_polygon_square(&self) -> Option<Vec<Point>> {
        self.top_local_square()
            .map(|r| r.into_iter().map(|p| self.to_plan(p)).collect())
    }

    /// The countertop's cutouts as hole rings in the local frame.
    pub fn holes_local(&self) -> Vec<Vec<Point>> {
        self.cutouts.iter().map(|c| c.outline.clone()).collect()
    }

    /// Volume of countertop material in cubic inches: the slab of a custom
    /// countertop, or a cabinet's own top, less its cutouts.
    pub fn countertop_volume(&self) -> f64 {
        let (thickness, outline) = match (&self.custom, self.countertop) {
            (Some(c), _) if self.kind == CabinetKind::CustomCountertop => {
                (c.thickness, c.footprint())
            }
            (_, Some(t)) => match self.top_local() {
                Some(r) => (t.thickness, r),
                None => return 0.0,
            },
            _ => return 0.0,
        };
        geom::region_area(&outline, &self.holes_local()) * thickness
    }

    /// Adds a sink or cooktop hole centred on the countertop. Returns false
    /// (changing nothing) when the cabinet has no countertop or the fixture
    /// does not fit inside it.
    pub fn add_cutout(&mut self, kind: CutoutKind) -> bool {
        let (name, fw, fd): (&str, f64, f64) = match kind {
            CutoutKind::Sink => ("Sink", 30.0, 18.0),
            CutoutKind::Cooktop => ("Cooktop", 30.0, 21.0),
            CutoutKind::Custom => ("Opening", 12.0, 12.0),
        };
        let Some(top) = self.top_local() else {
            return false;
        };
        let Some((lo, hi)) = geom::bbox(&top) else {
            return false;
        };
        let margin = 2.0;
        let (avail_w, avail_d) = (hi.x - lo.x - 2.0 * margin, hi.y - lo.y - 2.0 * margin);
        // The fixture may shrink to 60% of its nominal size to fit, no more.
        if avail_w < fw * 0.6 || avail_d < fd * 0.6 {
            return false;
        }
        let (fw, fd) = (fw.min(avail_w), fd.min(avail_d));
        let cx = (lo.x + hi.x) / 2.0;
        let cy = (lo.y + hi.y) / 2.0;
        let cut = Cutout::rect(kind, name, Point::new(cx, cy), fw, fd);
        if !cut
            .outline
            .iter()
            .all(|p| plan_core::geometry::point_in_polygon(*p, &top))
        {
            return false;
        }
        self.cutouts.push(cut);
        true
    }

    /// The label drawn in plan: the override with its macros expanded, or the
    /// automatic one.
    pub fn display_label(&self) -> String {
        if self.label.is_empty() {
            auto_label(self)
        } else {
            expand_label(&self.label, self)
        }
    }
}

/// Format a dimension as a whole number, or with up to two trimmed decimals.
fn num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Short code of an appliance for labels (`DW`, `RNG`, ...).
fn appliance_code(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "dishwasher" => "DW".to_string(),
        "range" | "oven" => "RNG".to_string(),
        "refrigerator" | "fridge" => "REF".to_string(),
        "microwave" => "MW".to_string(),
        other => other.chars().take(3).collect::<String>().to_uppercase(),
    }
}

/// The letters that open a cabinet's label: `B`, `W`, `FH`, `BF`, `BDC`...
pub fn type_code(cabinet: &Cabinet) -> String {
    if let Some(p) = cabinet.preset {
        return p.code().to_string();
    }
    let style = cabinet.corner.map(|c| (c.style, c.lazy_susan));
    match cabinet.kind {
        CabinetKind::Base => cabinet
            .appliance
            .as_deref()
            .map_or_else(|| "B".to_string(), appliance_code),
        CabinetKind::Wall => "W".to_string(),
        CabinetKind::FullHeight => "FH".to_string(),
        CabinetKind::Soffit | CabinetKind::SoffitPolygon => "SO".to_string(),
        CabinetKind::Shelf => "SH".to_string(),
        CabinetKind::Partition => "PT".to_string(),
        CabinetKind::BaseFiller => "BF".to_string(),
        CabinetKind::WallFiller => "WF".to_string(),
        CabinetKind::FullHeightFiller => "FHF".to_string(),
        CabinetKind::CornerBase => match style {
            Some((CornerStyle::PieCut, true)) => "BLS",
            Some((CornerStyle::PieCut, false)) => "BPC",
            _ => "BDC",
        }
        .to_string(),
        CabinetKind::CornerWall => match style {
            Some((CornerStyle::PieCut, _)) => "WPC",
            _ => "WDC",
        }
        .to_string(),
        CabinetKind::BlindBase => "BBC".to_string(),
        CabinetKind::BlindWall => "WBC".to_string(),
        CabinetKind::CustomCountertop => "CT".to_string(),
        CabinetKind::CustomBacksplash => "BS".to_string(),
        CabinetKind::CounterHole => "CH".to_string(),
    }
}

/// Industry-style label: `B24`, `B36-SB` (sink base), `W3030` (width then
/// height), `FH2484`, `BF3`, `WF330`, `BBC48`, `DW24`, `BDC36`, `SO..`,
/// `SH..`, `PT..`. Wall-hung and tall kinds append their height.
pub fn auto_label(cabinet: &Cabinet) -> String {
    let w = num(cabinet.width);
    let h = num(cabinet.height);
    let code = type_code(cabinet);
    match cabinet.kind {
        CabinetKind::Base if cabinet.appliance.is_none() && cabinet.face.has_appliance("Sink") => {
            format!("B{w}-SB")
        }
        CabinetKind::Wall
        | CabinetKind::FullHeight
        | CabinetKind::WallFiller
        | CabinetKind::FullHeightFiller
        | CabinetKind::CornerWall
        | CabinetKind::BlindWall => format!("{code}{w}{h}"),
        CabinetKind::CustomCountertop
        | CabinetKind::CustomBacksplash
        | CabinetKind::CounterHole => code,
        _ => format!("{code}{w}"),
    }
}

/// Expands the label macros of `template` (Chief's cabinet label text):
///
/// | macro | value |
/// |---|---|
/// | `<L>` | the automatic label (`B36`, `W3030`) |
/// | `<T>` | the type code (`B`, `W`, `FH`, `VB`...) |
/// | `<W>` `<D>` `<H>` | width, depth, height, whole inches or trimmed decimals |
/// | `<WxD>` `<WxH>` `<WxDxH>` | sizes joined with `x` |
/// | `<N>` | the cabinet's name (`Base Cabinet`, `Vanity Cabinet`) |
/// | `<S>` | the door style (`Shaker Door`) |
/// | `<F>` | the finish (door material) |
/// | `<HW>` | the hardware (`Knob`, `Pull / Knob`) |
/// | `<A>` | the appliance a bay holds, if any |
pub fn expand_label(template: &str, cabinet: &Cabinet) -> String {
    let (w, d, h) = (num(cabinet.width), num(cabinet.depth), num(cabinet.height));
    let name = cabinet
        .preset
        .map_or_else(|| cabinet.kind.name(), CabinetPreset::name);
    template
        .replace("<WxDxH>", &format!("{w}x{d}x{h}"))
        .replace("<WxD>", &format!("{w}x{d}"))
        .replace("<WxH>", &format!("{w}x{h}"))
        .replace("<L>", &auto_label(cabinet))
        .replace("<W>", &w)
        .replace("<D>", &d)
        .replace("<H>", &h)
        .replace("<T>", &type_code(cabinet))
        .replace("<N>", name)
        .replace("<S>", &cabinet.door_style.name)
        .replace("<F>", &cabinet.finish_name())
        .replace("<HW>", &cabinet.hardware_name())
        .replace("<A>", cabinet.appliance.as_deref().unwrap_or(""))
}

/// Place a row of cabinets along a wall on its +side (the left of the
/// `wall_start` to `wall_end` direction), backs against the wall face.
///
/// `wall_thickness` is the wall's full thickness and the start/end points are
/// its centreline, so the backs sit `wall_thickness / 2` off the centreline.
/// Cabinets are laid end to end from `wall_start`; the run is not clipped to
/// the wall length. Ids are left at `0` for the caller to assign.
pub fn run_along_wall(
    wall_start: Point,
    wall_end: Point,
    wall_thickness: f64,
    widths: &[f64],
    kind: CabinetKind,
) -> Vec<Cabinet> {
    let dir = wall_end.sub(wall_start).normalized();
    let normal = dir.perp();
    let angle = if dir == Point::ZERO { 0.0 } else { dir.angle() };
    let origin = wall_start.add(normal.scale(wall_thickness / 2.0));
    let mut offset = 0.0;
    widths
        .iter()
        .map(|&w| {
            let mut c = Cabinet::new(kind, w);
            c.position = origin.add(dir.scale(offset));
            c.angle = angle;
            offset += w;
            c
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_defaults_match_chief() {
        let c = Cabinet::base(24.0);
        assert_eq!(
            (c.width, c.depth, c.height, c.elevation),
            (24.0, 24.0, 36.0, 0.0)
        );
        assert_eq!(
            c.toe_kick,
            Some(ToeKick {
                height: 4.0,
                depth: 3.0
            })
        );
        let top = c.countertop.unwrap();
        assert_eq!((top.thickness, top.overhang_front), (1.5, 1.0));
        // Box height under the 1.5" top.
        assert_eq!(c.height - top.thickness, 34.5);
        assert_eq!(c.overlay, Overlay::Full { reveal: 0.0625 });
        assert!(c.framed);
    }

    #[test]
    fn wall_and_full_height_defaults() {
        let w = Cabinet::wall(30.0);
        assert_eq!((w.depth, w.height, w.elevation), (12.0, 30.0, 54.0));
        assert!(w.countertop.is_none() && w.toe_kick.is_none());
        let f = Cabinet::full_height(24.0);
        assert_eq!((f.depth, f.height), (24.0, 84.0));
        assert!(f.face.resolve(f.face_height(), 21.0).is_ok());
    }

    #[test]
    fn labels() {
        assert_eq!(auto_label(&Cabinet::base(36.0)), "B36");
        assert_eq!(auto_label(&Cabinet::sink_base(36.0)), "B36-SB");
        assert_eq!(auto_label(&Cabinet::wall(30.0)), "W3030");
        assert_eq!(auto_label(&Cabinet::full_height(24.0)), "FH2484");
        assert_eq!(auto_label(&Cabinet::base(37.5)), "B37.5");
    }

    #[test]
    fn run_of_three_covers_84_without_overlap() {
        let run = run_along_wall(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            &[24.0, 36.0, 24.0],
            CabinetKind::Base,
        );
        assert_eq!(run.len(), 3);
        // Backs on the +Y wall face (centreline + 3").
        assert!(run
            .iter()
            .all(|c| (c.position.y - 3.0).abs() < 1e-9 && c.angle == 0.0));
        let spans: Vec<(f64, f64)> = run
            .iter()
            .map(|c| (c.position.x, c.position.x + c.width))
            .collect();
        for pair in spans.windows(2) {
            assert!((pair[0].1 - pair[1].0).abs() < 1e-9, "gap or overlap");
        }
        assert_eq!(spans[0].0, 0.0);
        assert!((spans[2].1 - 84.0).abs() < 1e-9);
    }

    #[test]
    fn run_on_vertical_wall_faces_plus_side() {
        // Wall runs +Y; its +side (left) is -X, so fronts must face -X.
        let run = run_along_wall(
            Point::new(0.0, 0.0),
            Point::new(0.0, 100.0),
            4.0,
            &[24.0],
            CabinetKind::Base,
        );
        let c = &run[0];
        let front = c.to_plan(Point::new(0.0, 1.0)).sub(c.position);
        assert!((front.x + 1.0).abs() < 1e-9 && front.y.abs() < 1e-9);
        assert!((c.position.x + 2.0).abs() < 1e-9);
    }

    #[test]
    fn serde_round_trip() {
        let mut c = Cabinet::sink_base(36.0);
        c.angle = 0.5;
        c.position = Point::new(12.0, -3.5);
        c.overlay = Overlay::Inset { clearance: 0.0625 };
        c.label = "X".into();
        let json = serde_json::to_string(&c).unwrap();
        let back: Cabinet = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn corner_footprints_and_tops() {
        use plan_core::geometry::polygon_area;
        let pie = Cabinet::corner_base(36.0).with_pie_cut(false);
        assert!((polygon_area(&pie.footprint_local()) - 1152.0).abs() < 1e-9);
        let diag = Cabinet::corner_base(36.0);
        assert!((polygon_area(&diag.footprint_local()) - 1224.0).abs() < 1e-9);
        // The bounding box (corners()) is the 36 x 36 square either way.
        for c in [&pie, &diag] {
            let q = c.corners();
            assert_eq!((q[2].x, q[2].y), (36.0, 36.0));
        }
        // The top adds the 1" front overhang along every front.
        let top = diag.top_local().unwrap();
        assert!(polygon_area(&top) > polygon_area(&diag.footprint_local()));
        assert!(
            (top[2].y - (24.0 + 2f64.sqrt())).abs() < 1e-9,
            "{:?}",
            top[2]
        );
        let ptop = pie.top_local().unwrap();
        assert_eq!(ptop[3], Point::new(25.0, 25.0));
        // A wall corner has 12" arms and no top.
        let wall = Cabinet::corner_wall(24.0);
        assert_eq!((wall.depth, wall.elevation), (24.0, 54.0));
        assert!(wall.top_local().is_none());
        assert!((polygon_area(&wall.footprint_local()) - 24.0 * 24.0 + 144.0 / 2.0).abs() < 1e-9);
        // The footprint turns with the cabinet.
        let mut r = diag.clone();
        r.angle = std::f64::consts::FRAC_PI_2;
        r.position = Point::new(100.0, 100.0);
        let fp = r.footprint();
        assert!(fp
            .iter()
            .all(|p| p.x <= 100.0 + 1e-9 && p.y >= 100.0 - 1e-9));
    }

    #[test]
    fn labels_follow_chief_style_and_macros() {
        assert_eq!(
            auto_label(&Cabinet::filler(CabinetKind::BaseFiller, 3.0)),
            "BF3"
        );
        assert_eq!(
            auto_label(&Cabinet::filler(CabinetKind::WallFiller, 3.0)),
            "WF330"
        );
        assert_eq!(
            auto_label(&Cabinet::filler(CabinetKind::FullHeightFiller, 3.0)),
            "FHF384"
        );
        assert_eq!(
            auto_label(&Cabinet::blind_base(48.0, 15.0, BlindSide::Right)),
            "BBC48"
        );
        assert_eq!(
            auto_label(&Cabinet::blind_wall(36.0, 12.0, BlindSide::Left)),
            "WBC3630"
        );
        assert_eq!(auto_label(&Cabinet::dishwasher_opening()), "DW24");
        assert_eq!(auto_label(&Cabinet::range_opening(30.0)), "RNG30");
        assert_eq!(auto_label(&Cabinet::corner_base(36.0)), "BDC36");
        assert_eq!(
            auto_label(&Cabinet::corner_base(36.0).with_pie_cut(true)),
            "BLS36"
        );
        assert_eq!(
            auto_label(&Cabinet::corner_base(36.0).with_pie_cut(false)),
            "BPC36"
        );
        assert_eq!(auto_label(&Cabinet::corner_wall(24.0)), "WDC2430");
        assert_eq!(auto_label(&Cabinet::wall(24.0)), "W2430");
        assert_eq!(auto_label(&Cabinet::new(CabinetKind::Soffit, 36.0)), "SO36");
        // Macros expand in an override; an empty override uses the automatic label.
        let mut c = Cabinet::wall(24.0);
        c.label = "<T>-<W>x<H> (<L>)".into();
        assert_eq!(c.display_label(), "W-24x30 (W2430)");
        c.label = "Pantry <D>".into();
        assert_eq!(c.display_label(), "Pantry 12");
        c.label.clear();
        assert_eq!(c.display_label(), "W2430");
        // Overrides without macros stay literal.
        c.label = "X1".into();
        assert_eq!(c.display_label(), "X1");
    }

    #[test]
    fn new_kinds_build_with_chief_style_defaults() {
        for kind in CabinetKind::ALL {
            let c = Cabinet::new(kind, 24.0);
            assert_eq!(c.kind, kind);
            assert!(c.width > 0.0 && c.depth > 0.0 && c.height > 0.0, "{kind:?}");
            assert_eq!(c.kind.is_corner(), c.corner.is_some());
            assert_eq!(c.kind.is_blind(), c.blind.is_some());
            assert_eq!(c.kind.is_custom(), c.custom.is_some());
            assert!(!kind.name().is_empty());
        }
        let f = Cabinet::filler(CabinetKind::BaseFiller, 3.0);
        assert_eq!((f.width, f.depth, f.height), (3.0, 24.0, 36.0));
        assert!(!f.framed && f.countertop.is_some());
        let wf = Cabinet::new(CabinetKind::WallFiller, 3.0);
        assert_eq!((wf.depth, wf.elevation, wf.height), (12.0, 54.0, 30.0));
        let b = Cabinet::blind_base(48.0, 15.0, BlindSide::Left);
        assert_eq!(b.face_width(), 33.0);
        let dw = Cabinet::dishwasher_opening();
        assert!(dw.countertop.is_some() && dw.appliance.as_deref() == Some("Dishwasher"));
        let rg = Cabinet::range_opening(30.0);
        assert!(rg.countertop.is_none() && rg.toe_kick.is_none() && rg.height == 34.5);
    }

    #[test]
    fn old_files_without_the_new_fields_still_load() {
        let mut v = serde_json::to_value(Cabinet::base(24.0)).unwrap();
        let obj = v.as_object_mut().unwrap();
        for k in [
            "corner",
            "blind",
            "custom",
            "cutouts",
            "appliance",
            "moldings",
            "materials",
            "indicators",
        ] {
            obj.remove(k);
        }
        let ds = obj.get_mut("door_style").unwrap().as_object_mut().unwrap();
        for k in [
            "profile",
            "frame_width",
            "handle_centered",
            "hinge",
            "hinge_from_edge",
        ] {
            ds.remove(k);
        }
        let c: Cabinet = serde_json::from_value(v).unwrap();
        assert_eq!(c, Cabinet::base(24.0));
    }

    #[test]
    fn built_in_door_and_drawer_styles() {
        let mut d = DoorStyle::default();
        assert!(d.apply_builtin("Shaker Door"));
        assert_eq!(
            (d.name.as_str(), d.profile),
            ("Shaker Door", DoorProfile::Shaker)
        );
        assert!(d.apply_builtin("Raised Panel Door"));
        assert_eq!(d.profile, DoorProfile::Raised);
        assert!(!d.apply_builtin("Nope"));
        let mut dr = DrawerStyle::default();
        assert!(dr.apply_builtin("Shaker Drawer"));
        assert_eq!(dr.profile, DoorProfile::Shaker);
    }

    #[test]
    fn appliance_bays_toggle_back_to_an_ordinary_cabinet() {
        let mut c = Cabinet::base(24.0);
        c.set_appliance(Some("Dishwasher"));
        assert_eq!(c.face.items, FaceLayout::opening().items);
        assert!(!c.framed);
        assert_eq!(auto_label(&c), "DW24");
        c.set_appliance(Some("Range"));
        assert_eq!(auto_label(&c), "RNG24");
        c.set_appliance(None);
        assert!(c.framed && c.appliance.is_none());
        assert_eq!(c.face, FaceLayout::base_default(c.face_height()));
        assert_eq!(auto_label(&c), "B24");
        // Clearing a cabinet that never was a bay changes nothing.
        let before = c.clone();
        c.set_appliance(None);
        assert_eq!(c, before);
    }
}
