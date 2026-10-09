//! The Door and Window Specification tabs of round 15: Rough Opening,
//! Framing, Energy Values, Treatments, Materials, Layer and Object
//! Information, plus the double-door swing choices and the curved wall
//! casing mode of the Options and Casing tabs.
//!
//! Every value lives in [`super::OpeningSpec`] (`Opening.extras.spec`) and
//! defaults to what the opening was before the tab existed, so an old plan
//! reads and draws as it did.

use crate::model::{Opening, OpeningKind};
use crate::units::fmt_ft_in;
use serde::{Deserialize, Serialize};

// ----- Rough Opening -----

/// How the Rough Opening tab works out the rough opening from the unit size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoughMode {
    /// The unit plus an additional width and height ("Additional Space").
    #[default]
    AdditionalSpace,
    /// The unit plus a gap on each side, at the top and at the bottom
    /// ("Clearance Gap").
    ClearanceGap,
}

impl RoughMode {
    pub const ALL: [RoughMode; 2] = [RoughMode::AdditionalSpace, RoughMode::ClearanceGap];

    pub fn name(self) -> &'static str {
        match self {
            RoughMode::AdditionalSpace => "Additional Space",
            RoughMode::ClearanceGap => "Clearance Gap",
        }
    }
}

/// The Rough Opening tab (DW-56): the opening the framer leaves in the wall,
/// larger than the unit, and what the plan and the schedule show of it. The
/// extra space is zero until the tab sets it, so a plan from before the tab
/// frames and draws as it did (Chief's own defaults are 2" wide and 2 1/2"
/// high; see DECISIONS.md).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoughOpening {
    pub mode: RoughMode,
    /// Additional Space: total extra width, shared by the two sides.
    pub add_width: f64,
    /// Additional Space: total extra height (a door's all above the head, a
    /// window's shared by head and sill).
    pub add_height: f64,
    /// Clearance Gap: each side, the top, the bottom.
    pub gap_side: f64,
    pub gap_top: f64,
    pub gap_bottom: f64,
    /// Add for Concrete Cutout: extra on each side of the rough opening in
    /// the slab below, `0` for none.
    pub concrete_each_side: f64,
    /// Plan Display "Show in Floor Below" of the concrete cutout.
    pub concrete_show_below: bool,
    /// Draw the rough opening in the plan (dashed lines across the wall).
    pub show_in_plan: bool,
}

impl Default for RoughOpening {
    fn default() -> Self {
        Self {
            mode: RoughMode::AdditionalSpace,
            add_width: 0.0,
            add_height: 0.0,
            gap_side: 0.25,
            gap_top: 0.5,
            gap_bottom: 0.0,
            concrete_each_side: 0.0,
            concrete_show_below: false,
            show_in_plan: false,
        }
    }
}

/// How far the rough opening runs past the unit on each side, inches.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RoughBox {
    pub left: f64,
    pub right: f64,
    pub bottom: f64,
    pub top: f64,
}

impl RoughOpening {
    /// The extents past a unit of `kind`.
    pub fn extents(&self, kind: OpeningKind) -> RoughBox {
        match self.mode {
            RoughMode::AdditionalSpace => {
                let w = self.add_width.max(0.0) * 0.5;
                let h = self.add_height.max(0.0);
                let (bottom, top) = match kind {
                    OpeningKind::Door => (0.0, h),
                    OpeningKind::Window => (h * 0.5, h * 0.5),
                };
                RoughBox {
                    left: w,
                    right: w,
                    bottom,
                    top,
                }
            }
            RoughMode::ClearanceGap => RoughBox {
                left: self.gap_side.max(0.0),
                right: self.gap_side.max(0.0),
                bottom: self.gap_bottom.max(0.0),
                top: self.gap_top.max(0.0),
            },
        }
    }
}

/// The rough opening in wall terms: where the framing stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramedOpening {
    /// Along the wall, inches from its start.
    pub start: f64,
    pub end: f64,
    /// Above the floor the wall stands on.
    pub bottom: f64,
    pub top: f64,
}

// ----- Framing -----

/// What a header is made of (Framing tab, Construction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HeaderMaterial {
    /// Dimensional lumber plies, 1 1/2" each.
    #[default]
    Lumber,
    /// Laminated veneer lumber plies, 1 3/4" each.
    Lvl,
}

impl HeaderMaterial {
    pub const ALL: [HeaderMaterial; 2] = [HeaderMaterial::Lumber, HeaderMaterial::Lvl];

    pub fn name(self) -> &'static str {
        match self {
            HeaderMaterial::Lumber => "Wall Header - Lumber",
            HeaderMaterial::Lvl => "Wall Header - LVL",
        }
    }

    /// Thickness of one ply, inches.
    pub fn ply(self) -> f64 {
        match self {
            HeaderMaterial::Lumber => 1.5,
            HeaderMaterial::Lvl => 1.75,
        }
    }
}

/// The Framing tab (DW-114): the header, trimmers, king studs and sill of
/// this opening where they differ from the Framing Defaults. `None` follows
/// the defaults (and, for the header depth, the table by width).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningFraming {
    /// Include Header.
    pub include_header: bool,
    /// Plies in the header (Count).
    pub header_plies: Option<u32>,
    /// Depth of the header (`None` is "Calculate from Width").
    pub header_depth: Option<f64>,
    pub header_material: HeaderMaterial,
    pub trimmers: Option<u32>,
    pub king_studs: Option<u32>,
    /// A window's framed sill under the opening.
    pub sill: bool,
}

impl Default for OpeningFraming {
    fn default() -> Self {
        Self {
            include_header: true,
            header_plies: None,
            header_depth: None,
            header_material: HeaderMaterial::Lumber,
            trimmers: None,
            king_studs: None,
            sill: true,
        }
    }
}

// ----- Energy Values -----

/// The Energy Values tab (DW-115): U-factor and solar heat gain stored for
/// the schedules and energy reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnergyValues {
    /// The door or glazing type, as picked (free text when none fits).
    pub construction: String,
    pub u_factor: f64,
    /// Solar heat gain coefficient, `0..=1`.
    pub shgc: f64,
}

impl Default for EnergyValues {
    fn default() -> Self {
        Self {
            construction: String::new(),
            u_factor: 0.30,
            shgc: 0.30,
        }
    }
}

/// Door types of the Energy Values tab with their usual U-factor and SHGC.
pub const DOOR_ENERGY_TYPES: [(&str, f64, f64); 4] = [
    ("Solid (under 50% glazing)", 0.30, 0.30),
    ("Half Glass (50% to 80% glazing)", 0.35, 0.35),
    ("Full Glass (over 80% glazing)", 0.45, 0.40),
    ("Insulated Metal", 0.20, 0.00),
];

/// Glazing types of the Energy Values tab with their usual U-factor and SHGC.
pub const WINDOW_ENERGY_TYPES: [(&str, f64, f64); 5] = [
    ("Single Pane", 1.10, 0.80),
    ("Double Pane", 0.50, 0.65),
    ("Double Pane Low-E", 0.30, 0.30),
    ("Triple Pane Low-E", 0.20, 0.25),
    ("Insulated Glass, Argon", 0.28, 0.28),
];

// ----- Object Information -----

/// The Object Information tab (DW-118): the opening's code and description.
/// Manufacturer, model, supplier and the notes (the comment) are the Schedule
/// tab's own fields (`OpeningSpec::schedule`), so both tabs and the schedule
/// columns show one value.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningInfo {
    /// The ID or code the office uses for this unit.
    pub id: String,
    pub description: String,
}

// ----- Materials -----

/// One component of an opening and the material it was given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartPaint {
    /// The component ("Frame", "Sash", "Glass", "Casing", ...).
    pub part: String,
    /// The `plan_materials` library material, by name.
    pub material: String,
    /// Its colour when it was picked, so 3D shows it without the library.
    pub rgb: [u8; 3],
}

/// The Materials tab (DW-117): a material per component. A component without
/// an entry keeps its usual look.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningMaterials {
    pub parts: Vec<PartPaint>,
}

/// The components of a door the Materials tab lists.
pub const DOOR_PARTS: [&str; 5] = ["Door Panel", "Jamb", "Casing", "Hardware", "Threshold"];
/// The components of a window the Materials tab lists.
pub const WINDOW_PARTS: [&str; 6] = ["Frame", "Sash", "Glass", "Casing", "Sill", "Threshold"];

impl OpeningMaterials {
    /// The paint of `part`, when it has one.
    pub fn get(&self, part: &str) -> Option<&PartPaint> {
        self.parts.iter().find(|p| p.part == part)
    }

    /// The colour of `part`, when it has a material.
    pub fn color(&self, part: &str) -> Option<[u8; 3]> {
        self.get(part).map(|p| p.rgb)
    }

    /// Gives `part` a material, or takes it back with `None`.
    pub fn set(&mut self, part: &str, paint: Option<(&str, [u8; 3])>) {
        self.parts.retain(|p| p.part != part);
        if let Some((material, rgb)) = paint {
            self.parts.push(PartPaint {
                part: part.to_string(),
                material: material.to_string(),
                rgb,
            });
        }
    }
}

// ----- Treatments -----

/// Curtain style (Treatments tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CurtainStyle {
    #[default]
    None,
    /// Two flat panels beside the window, on a rod.
    Panels,
    /// Two pleated panels (folds in the cloth).
    Pleated,
    /// A short valance across the head.
    Valance,
}

impl CurtainStyle {
    pub const ALL: [CurtainStyle; 4] = [
        CurtainStyle::None,
        CurtainStyle::Panels,
        CurtainStyle::Pleated,
        CurtainStyle::Valance,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CurtainStyle::None => "None",
            CurtainStyle::Panels => "Straight Panels",
            CurtainStyle::Pleated => "Pleated Panels",
            CurtainStyle::Valance => "Valance",
        }
    }
}

/// Blind style (Treatments tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlindStyle {
    #[default]
    None,
    /// Horizontal slats on a head rail.
    Horizontal,
    /// Vertical slats.
    Vertical,
    /// One roller shade.
    Roller,
}

impl BlindStyle {
    pub const ALL: [BlindStyle; 4] = [
        BlindStyle::None,
        BlindStyle::Horizontal,
        BlindStyle::Vertical,
        BlindStyle::Roller,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BlindStyle::None => "None",
            BlindStyle::Horizontal => "Horizontal Blinds",
            BlindStyle::Vertical => "Vertical Blinds",
            BlindStyle::Roller => "Roller Shade",
        }
    }
}

/// Interior shutter style (Treatments tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum InteriorShutterStyle {
    #[default]
    None,
    /// Full-height louvered leaves inside the casing.
    Plantation,
    /// Leaves over the lower half of the window.
    Cafe,
}

impl InteriorShutterStyle {
    pub const ALL: [InteriorShutterStyle; 3] = [
        InteriorShutterStyle::None,
        InteriorShutterStyle::Plantation,
        InteriorShutterStyle::Cafe,
    ];

    pub fn name(self) -> &'static str {
        match self {
            InteriorShutterStyle::None => "None",
            InteriorShutterStyle::Plantation => "Plantation (full height)",
            InteriorShutterStyle::Cafe => "Cafe (lower half)",
        }
    }
}

/// Exterior millwork over or under the casing (Treatments tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MillworkStyle {
    #[default]
    None,
    /// A flat board with a projecting cap.
    Cornice,
    /// A flat header board.
    Header,
    /// A flat apron board (below the casing).
    Apron,
}

impl MillworkStyle {
    pub const ABOVE: [MillworkStyle; 3] = [
        MillworkStyle::None,
        MillworkStyle::Cornice,
        MillworkStyle::Header,
    ];
    pub const BELOW: [MillworkStyle; 2] = [MillworkStyle::None, MillworkStyle::Apron];

    pub fn name(self) -> &'static str {
        match self {
            MillworkStyle::None => "None",
            MillworkStyle::Cornice => "Cornice",
            MillworkStyle::Header => "Header Board",
            MillworkStyle::Apron => "Apron Board",
        }
    }
}

/// The Treatments tab (DW-123): curtains, blinds and interior shutters on the
/// room side and millwork on the outside. They are built in 3D only; the plan
/// does not draw them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Treatments {
    pub curtain: CurtainStyle,
    pub curtain_color: [u8; 3],
    /// Bottom of the curtains above the floor.
    pub curtain_off_floor: f64,
    /// Rod height above the casing.
    pub curtain_above_casing: f64,
    pub blind: BlindStyle,
    pub blind_color: [u8; 3],
    /// How far the blind is lowered, `0..=1` of the glass height.
    pub blind_lowered: f64,
    pub shutter: InteriorShutterStyle,
    pub shutter_color: [u8; 3],
    /// Leaves closed over the window; open, they fold to the sides.
    pub shutter_closed: bool,
    pub millwork_above: MillworkStyle,
    pub millwork_above_height: f64,
    /// Width of the board, `0` for the casing's width.
    pub millwork_above_width: f64,
    pub millwork_below: MillworkStyle,
    pub millwork_below_height: f64,
    /// How far the board runs past the casing on each side.
    pub millwork_below_extend: f64,
}

impl Default for Treatments {
    fn default() -> Self {
        Self {
            curtain: CurtainStyle::None,
            curtain_color: [196, 184, 160],
            curtain_off_floor: 18.0,
            curtain_above_casing: 2.0,
            blind: BlindStyle::None,
            blind_color: [236, 232, 222],
            blind_lowered: 0.5,
            shutter: InteriorShutterStyle::None,
            shutter_color: [246, 244, 238],
            shutter_closed: true,
            millwork_above: MillworkStyle::None,
            millwork_above_height: 12.0,
            millwork_above_width: 0.0,
            millwork_below: MillworkStyle::None,
            millwork_below_height: 12.0,
            millwork_below_extend: 0.0,
        }
    }
}

impl Treatments {
    /// Whether any room-side treatment is on.
    pub fn any_interior(&self) -> bool {
        self.curtain != CurtainStyle::None
            || self.blind != BlindStyle::None
            || self.shutter != InteriorShutterStyle::None
    }

    pub fn any(&self) -> bool {
        self.any_interior()
            || self.millwork_above != MillworkStyle::None
            || self.millwork_below != MillworkStyle::None
    }
}

// ----- door swing and curved casing -----

/// Which leaves of a double door swing (Options tab, Door Swing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DoorSwing {
    /// Both doors swing.
    #[default]
    Both,
    /// Only the leaf on the wall-start side swings; the other stays shut.
    LeftOnly,
    /// Only the leaf on the wall-end side swings.
    RightOnly,
}

impl DoorSwing {
    pub const ALL: [DoorSwing; 3] = [DoorSwing::Both, DoorSwing::LeftOnly, DoorSwing::RightOnly];

    pub fn name(self) -> &'static str {
        match self {
            DoorSwing::Both => "Both Doors Swing",
            DoorSwing::LeftOnly => "Left Swing Only",
            DoorSwing::RightOnly => "Right Swing Only",
        }
    }

    /// Whether the leaf on the start side (`false`) or end side (`true`)
    /// swings.
    pub fn swings(self, end_leaf: bool) -> bool {
        match self {
            DoorSwing::Both => true,
            DoorSwing::LeftOnly => !end_leaf,
            DoorSwing::RightOnly => end_leaf,
        }
    }
}

/// How the casing of an opening in a curved wall is laid (Casing tab, Curved
/// Wall Casing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CurvedCasing {
    /// Straight boards square to the wall at the middle of the opening.
    Straight,
    /// Boards square to the wall where each stands (radial).
    #[default]
    Radial,
    /// Boards bent to follow the wall's curve.
    Parallel,
}

impl CurvedCasing {
    pub const ALL: [CurvedCasing; 3] = [
        CurvedCasing::Straight,
        CurvedCasing::Radial,
        CurvedCasing::Parallel,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CurvedCasing::Straight => "Straight",
            CurvedCasing::Radial => "Radial",
            CurvedCasing::Parallel => "Parallel",
        }
    }
}

// ----- helpers on the opening -----

impl crate::model::Project {
    /// Hands the Materials tab of opening `id` to the project's per-object
    /// paint (`Project::object_materials`), so the Material Painter, Adjust
    /// Materials and the textures of the library see the same materials. It
    /// only sets: a component the tab leaves alone keeps whatever it was
    /// painted. Returns whether anything changed.
    pub fn sync_opening_materials(&mut self, id: crate::model::Id) -> bool {
        let Some(paints) = self
            .floors
            .iter()
            .flat_map(|f| &f.openings)
            .find(|o| o.id == id)
            .map(|o| o.extras.spec.materials.parts.clone())
        else {
            return false;
        };
        let mut changed = false;
        for p in paints {
            changed |= self.set_object_material(id, &p.part, &p.material);
        }
        changed
    }
}

impl Opening {
    /// How far the rough opening runs past the unit.
    pub fn rough_box(&self) -> RoughBox {
        self.extras.spec.rough.extents(self.kind)
    }

    /// Total width of the rough opening, inches.
    pub fn rough_width(&self) -> f64 {
        let b = self.rough_box();
        self.width + b.left + b.right
    }

    /// Total height of the rough opening, inches.
    pub fn rough_height(&self) -> f64 {
        let b = self.rough_box();
        self.height + b.bottom + b.top
    }

    /// Bottom of the header: the top of the rough opening above the floor.
    pub fn header_bottom(&self) -> f64 {
        self.sill_height + self.height + self.rough_box().top
    }

    /// Width of the cutout in the slab below (rough width plus Add for
    /// Concrete Cutout on each side), `None` when the tab does not ask for one.
    pub fn concrete_cutout_width(&self) -> Option<f64> {
        let each = self.extras.spec.rough.concrete_each_side;
        (each > 0.0).then(|| self.rough_width() + 2.0 * each)
    }

    /// The rough opening in wall terms (what the framing stands around).
    pub fn framed(&self) -> FramedOpening {
        let b = self.rough_box();
        FramedOpening {
            start: self.start_offset() - b.left,
            end: self.end_offset() + b.right,
            bottom: (self.sill_height - b.bottom).max(0.0),
            top: self.sill_height + self.height + b.top,
        }
    }

    /// The "Rough Opening" text of the schedule: width x height.
    pub fn rough_text(&self) -> String {
        format!(
            "{} x {}",
            fmt_ft_in(self.rough_width()),
            fmt_ft_in(self.rough_height())
        )
    }

    /// The layer the opening is on: the Layer tab's choice, else the fixed
    /// Doors or Windows layer.
    pub fn layer_name(&self) -> &str {
        self.extras
            .spec
            .layer
            .as_deref()
            .filter(|l| !l.trim().is_empty())
            .unwrap_or(match self.kind {
                OpeningKind::Door => "Doors",
                OpeningKind::Window => "Windows",
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn door() -> Opening {
        Opening::default_door(1, 1, 60.0)
    }

    #[test]
    fn rough_opening_starts_at_the_unit_size() {
        let d = door();
        assert_eq!(d.rough_width(), 36.0);
        assert_eq!(d.rough_height(), 80.0);
        assert_eq!(d.header_bottom(), 80.0);
        let f = d.framed();
        assert_eq!((f.start, f.end, f.bottom, f.top), (42.0, 78.0, 0.0, 80.0));
        assert_eq!(d.concrete_cutout_width(), None);
    }

    #[test]
    fn additional_space_splits_the_width_and_a_doors_height_goes_up() {
        let mut d = door();
        d.extras.spec.rough.add_width = 2.0;
        d.extras.spec.rough.add_height = 2.5;
        assert_eq!(d.rough_width(), 38.0);
        assert_eq!(d.rough_height(), 82.5);
        assert_eq!(d.header_bottom(), 82.5);
        let f = d.framed();
        assert_eq!((f.start, f.end), (41.0, 79.0));
        assert_eq!(f.bottom, 0.0);
        // A window shares the height between head and sill.
        let mut w = Opening::default_window(2, 1, 60.0);
        w.extras.spec.rough.add_height = 2.0;
        let f = w.framed();
        assert_eq!((f.bottom, f.top), (23.0, 85.0));
    }

    #[test]
    fn clearance_gaps_add_per_side() {
        let mut d = door();
        d.extras.spec.rough.mode = RoughMode::ClearanceGap;
        d.extras.spec.rough.gap_side = 0.25;
        d.extras.spec.rough.gap_top = 0.5;
        assert_eq!(d.rough_width(), 36.5);
        assert_eq!(d.rough_height(), 80.5);
        d.extras.spec.rough.concrete_each_side = 4.0;
        assert_eq!(d.concrete_cutout_width(), Some(44.5));
    }

    #[test]
    fn rough_text_reads_like_a_schedule_size() {
        let mut d = door();
        d.extras.spec.rough.add_width = 2.0;
        d.extras.spec.rough.add_height = 2.5;
        assert_eq!(d.rough_text(), "3'-2\" x 6'-10 1/2\"");
    }

    #[test]
    fn the_layer_follows_the_tab_and_falls_back_to_the_kind() {
        let mut d = door();
        assert_eq!(d.layer_name(), "Doors");
        d.extras.spec.layer = Some("Doors, Labels".into());
        assert_eq!(d.layer_name(), "Doors, Labels");
        d.extras.spec.layer = Some("  ".into());
        assert_eq!(d.layer_name(), "Doors");
        assert_eq!(Opening::default_window(2, 1, 5.0).layer_name(), "Windows");
    }

    #[test]
    fn materials_set_replace_and_clear() {
        let mut m = OpeningMaterials::default();
        m.set("Frame", Some(("Painted White Trim", [250, 250, 250])));
        m.set("Frame", Some(("Oak", [160, 110, 60])));
        assert_eq!(m.parts.len(), 1);
        assert_eq!(m.color("Frame"), Some([160, 110, 60]));
        m.set("Frame", None);
        assert!(m.get("Frame").is_none());
    }

    #[test]
    fn the_materials_tab_reaches_the_projects_paint() {
        let mut p = crate::model::Project::new("t");
        let w = p.add_wall(
            0,
            crate::geometry::Point::ZERO,
            crate::geometry::Point::new(120.0, 0.0),
            4.5,
            96.0,
            crate::model::WallKind::Exterior,
        );
        let id = p.add_opening(0, w, 60.0, OpeningKind::Window).unwrap();
        assert!(!p.sync_opening_materials(id), "nothing to hand over");
        p.floors[0].openings[0]
            .extras
            .spec
            .materials
            .set("Sash", Some(("Painted White Trim", [250, 250, 250])));
        assert!(p.sync_opening_materials(id));
        assert_eq!(p.object_material(id, "Sash"), Some("Painted White Trim"));
        // Already there: no change; and a component the tab leaves alone is kept.
        p.set_object_material(id, "Glass", "Clear Glass");
        assert!(!p.sync_opening_materials(id));
        assert_eq!(p.object_material(id, "Glass"), Some("Clear Glass"));
        assert!(!p.sync_opening_materials(9999));
    }

    #[test]
    fn door_swing_picks_the_leaves() {
        assert!(DoorSwing::Both.swings(false) && DoorSwing::Both.swings(true));
        assert!(DoorSwing::LeftOnly.swings(false) && !DoorSwing::LeftOnly.swings(true));
        assert!(!DoorSwing::RightOnly.swings(false) && DoorSwing::RightOnly.swings(true));
    }

    #[test]
    fn the_tab_values_default_and_round_trip() {
        let mut spec = crate::openings::OpeningSpec::default();
        assert_eq!(spec.energy.u_factor, 0.30);
        assert!(spec.framing.include_header && spec.framing.sill);
        assert!(!spec.treatments.any());
        spec.treatments.curtain = CurtainStyle::Pleated;
        spec.framing.trimmers = Some(2);
        spec.materials.set("Sash", Some(("Oak", [1, 2, 3])));
        let json = serde_json::to_string(&spec).unwrap();
        let back: crate::openings::OpeningSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
        // A plan from before the tabs has none of the keys.
        let old: crate::openings::OpeningSpec = serde_json::from_str("{}").unwrap();
        assert_eq!(old, crate::openings::OpeningSpec::default());
    }
}
