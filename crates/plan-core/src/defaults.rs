//! Plan defaults: everything a new plan starts with, the way Chief Architect
//! starts from a template plan (wall types, wall/door/window/cabinet/dimension
//! defaults, room types, layers, grid). The values of
//! [`PlanDefaults::chief_x18_daniel`] are the ones captured from Daniel's
//! Chief X18 template (`docs/chief-x18-dialogs.md`). Lengths are inches.

use crate::dimension::{AutoString, DimFormat, ObjectLocate, OpeningLocate, WallLocate};
use crate::layer_sets::LayerSets;
use crate::layers::LayerSet;
use crate::model::{Project, WallKind, DEFAULT_CEILING_HEIGHT};
use crate::text_styles::TextStyles;
use crate::units::{LengthFormat, LengthUnit};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ----- wall types -----

/// One layer of a wall assembly. `thickness` is in inches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WallLayer {
    pub name: String,
    pub thickness: f64,
    /// The main (structural) layer; its exterior side is the framing line.
    pub is_main: bool,
    pub material: String,
}

impl WallLayer {
    /// A wall layer: `name`, `thickness` in inches, whether it is the main
    /// (structural) layer, and its `material`.
    pub fn new(name: &str, thickness: f64, is_main: bool, material: &str) -> Self {
        Self {
            name: name.into(),
            thickness,
            is_main,
            material: material.into(),
        }
    }
}

/// A named wall assembly. `layers` run from the exterior face to the interior
/// face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WallTypeDef {
    pub name: String,
    pub layers: Vec<WallLayer>,
    pub kind: WallKind,
}

impl WallTypeDef {
    /// Total thickness, inches.
    pub fn thickness(&self) -> f64 {
        self.layers.iter().map(|l| l.thickness).sum()
    }

    /// Distance from the exterior face to the exterior side of the main
    /// layer. With no main layer it is 0.
    pub fn main_layer_offset(&self) -> f64 {
        let mut offset = 0.0;
        for l in &self.layers {
            if l.is_main {
                return offset;
            }
            offset += l.thickness;
        }
        0.0
    }

    /// The main layer, if the type has one.
    pub fn main_layer(&self) -> Option<&WallLayer> {
        self.layers.iter().find(|l| l.is_main)
    }
}

// ----- wall defaults -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoofWallKind {
    Hip,
    FullGable,
    DutchGable,
    HighShedGable,
    KneeWall,
    ExtendSlopeDownward,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WallRoofDefaults {
    /// Roof pitch as rise per 12 of run.
    pub pitch_in_12: f64,
    pub overhang: f64,
    pub kind: RoofWallKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WallDefaults {
    /// Name of an entry of [`PlanDefaults::wall_types`].
    pub wall_type: String,
    pub height: f64,
    pub roof: WallRoofDefaults,
}

/// How the end of the roof structure is cut at an eave (RF-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EaveCut {
    /// A vertical end: the fascia hangs plumb.
    #[default]
    Plumb,
    /// A horizontal end: the board lies level under the eave.
    Level,
    /// An end square to the rafter: the fascia stands perpendicular to the
    /// roof plane.
    Square,
}

impl EaveCut {
    pub const ALL: [EaveCut; 3] = [EaveCut::Plumb, EaveCut::Level, EaveCut::Square];

    pub fn label(self) -> &'static str {
        match self {
            EaveCut::Plumb => "Plumb",
            EaveCut::Level => "Level",
            EaveCut::Square => "Square",
        }
    }
}

/// Roof detail defaults (Default Settings > Roof Defaults, RF-14, RF-15,
/// RF-28, RF-31): the sizes and switches behind the fascia, soffit, frieze,
/// rafter tails and attic walls that `plan-3d` draws around a roof, plus the
/// Build Roof baseline rule. Build Roof copies them into the roof settings
/// of its floor; a roof plane can override the eave options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoofDetailDefaults {
    /// Roof structure plus surface, thick along the plane normal, inches.
    pub thickness: f64,
    pub eave_cut: EaveCut,
    pub fascia: bool,
    /// Fascia board height, inches.
    pub fascia_height: f64,
    /// Fascia board thickness, inches.
    pub fascia_thickness: f64,
    pub soffit: bool,
    /// The eave soffit follows the roof slope instead of lying level.
    pub sloped_soffit: bool,
    pub rake_fascia: bool,
    /// A frieze board on the wall under the eave.
    pub frieze: bool,
    pub ridge_caps: bool,
    /// Gutters on the eaves (a plane can turn them off or on).
    pub gutters: bool,
    /// Gutter size, inches.
    pub gutter_size: f64,
    /// Flashing strip where a lower roof butts a wall.
    pub flashing: bool,
    /// Exposed rafter tails under the eave (no soffit there).
    pub rafter_tails: bool,
    /// Rafter spacing, inches on center.
    pub rafter_spacing: f64,
    /// Rafter width and depth, inches.
    pub rafter_width: f64,
    pub rafter_depth: f64,
    /// Generate attic walls above a lower roof (Auto Attic Walls).
    pub auto_attic_walls: bool,
    /// Wall type of the generated attic walls and of the part of a wall
    /// above its plate; empty keeps the wall's own type.
    pub attic_wall_type: String,
    /// "Lower Wall Type if Split by Butting Roof": the type of the part of
    /// a wall that stands below the butting roof; empty keeps the wall's
    /// own type.
    pub lower_wall_type: String,
    /// A roof plane under a wall cuts the wall's bottom (a second-floor wall
    /// standing on a lower roof).
    pub roof_cuts_wall_at_bottom: bool,
    /// Build Roof puts the underside of the structure at the top plate at
    /// the wall (no gap and no overlap at gable corners) instead of putting
    /// the eave tip at plate height.
    pub baseline_at_plate: bool,
}

impl Default for RoofDetailDefaults {
    fn default() -> Self {
        Self {
            thickness: 6.0,
            eave_cut: EaveCut::Plumb,
            fascia: true,
            fascia_height: 6.0,
            fascia_thickness: 1.5,
            soffit: true,
            sloped_soffit: false,
            rake_fascia: true,
            frieze: false,
            ridge_caps: true,
            gutters: false,
            gutter_size: 5.0,
            flashing: true,
            rafter_tails: false,
            rafter_spacing: 24.0,
            rafter_width: 1.5,
            rafter_depth: 5.5,
            auto_attic_walls: true,
            attic_wall_type: String::new(),
            lower_wall_type: String::new(),
            roof_cuts_wall_at_bottom: true,
            baseline_at_plate: true,
        }
    }
}

/// Defaults of the flyout wall variants (pony, half, glass, deck, fencing).
/// Not captured from Chief; typical values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallVariantDefaults {
    /// Pony wall upper type (the default exterior type).
    pub pony_upper_type: String,
    /// Pony wall lower type.
    pub pony_lower_type: String,
    /// Elevation of the pony split, inches.
    pub pony_split_height: f64,
    /// Half-wall top height, inches.
    pub half_wall_height: f64,
    /// Glass wall type.
    pub glass_type: String,
    /// Railing wall type.
    pub railing_type: String,
    pub railing_height: f64,
    pub deck_railing_type: String,
    pub deck_edge_type: String,
    /// Height of a deck edge (rim board), inches.
    pub deck_edge_height: f64,
    pub fencing_type: String,
    pub fencing_height: f64,
}

impl Default for WallVariantDefaults {
    fn default() -> Self {
        Self {
            pony_upper_type: "Stucco-6".into(),
            pony_lower_type: "Foundation-8".into(),
            pony_split_height: 36.0,
            half_wall_height: 36.0,
            glass_type: "Glass-1".into(),
            railing_type: "Railing-4".into(),
            railing_height: 36.0,
            deck_railing_type: "Deck Railing-4".into(),
            deck_edge_type: "Deck Edge-2".into(),
            deck_edge_height: 9.25,
            fencing_type: "Fence-Wood-2".into(),
            fencing_height: 72.0,
        }
    }
}

// ----- openings -----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpeningDefaults {
    pub width: f64,
    pub height: f64,
    pub thickness: f64,
    /// Library style name, e.g. "Door P04".
    pub style: String,
    pub casing_width: f64,
    pub casing_depth: f64,
    pub reveal: f64,
    pub jamb_width: f64,
    pub swing_angle: f64,
    pub sill_height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowDefaults {
    pub width: f64,
    pub height: f64,
    pub sill_height: f64,
    pub window_type: String,
    pub frame_width: f64,
    pub sash_width: f64,
    pub lites_across: u32,
    pub lites_vertical: u32,
    pub egress: bool,
    pub tempered: bool,
}

// ----- cabinets -----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaseCabinetDefaults {
    pub width: f64,
    pub depth: f64,
    /// Overall height including the countertop.
    pub height: f64,
    pub countertop_thickness: f64,
    pub countertop_overhang: f64,
    pub toe_kick_height: f64,
    pub toe_kick_depth: f64,
    pub door_style: String,
    pub drawer_style: String,
    pub handle: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WallCabinetDefaults {
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    /// Floor to cabinet bottom.
    pub elevation: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullHeightCabinetDefaults {
    pub width: f64,
    pub depth: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CabinetDefaults {
    pub base: BaseCabinetDefaults,
    pub wall: WallCabinetDefaults,
    pub full_height: FullHeightCabinetDefaults,
    /// Width a new filler starts with before it snaps to the gap it fills.
    #[serde(default = "default_filler_width")]
    pub filler_width: f64,
    /// Both legs of a corner base cabinet.
    #[serde(default = "default_corner_base_leg")]
    pub corner_base_leg: f64,
    /// Both legs of a corner wall cabinet.
    #[serde(default = "default_corner_wall_leg")]
    pub corner_wall_leg: f64,
    /// Overall width of a blind corner base cabinet.
    #[serde(default = "default_blind_width")]
    pub blind_base_width: f64,
    /// The hidden part of a blind corner cabinet.
    #[serde(default = "default_blind_hidden")]
    pub blind_hidden_width: f64,
    /// Soffit tool: width (along the run), depth and height, and the bottom's
    /// height above the floor.
    #[serde(default = "default_soffit")]
    pub soffit: BoxDefaults,
    /// Shelf tool; `elevation` is the shelf's height above the floor.
    #[serde(default = "default_shelf")]
    pub shelf: BoxDefaults,
    /// Partition tool.
    #[serde(default = "default_partition")]
    pub partition: BoxDefaults,
    /// The library types: Vanity, Pantry, Tall Oven and Refrigerator
    /// cabinets (sizes only; their faces come with the type).
    #[serde(default = "default_vanity")]
    pub vanity: BoxDefaults,
    #[serde(default = "default_pantry")]
    pub pantry: BoxDefaults,
    #[serde(default = "default_tall_oven")]
    pub tall_oven: BoxDefaults,
    #[serde(default = "default_refrigerator")]
    pub refrigerator: BoxDefaults,
    /// Edit > Default Settings > Cabinets > Countertop.
    #[serde(default)]
    pub countertop: CountertopDefaults,
    /// Edit > Default Settings > Cabinets > Backsplash.
    #[serde(default)]
    pub backsplash: BacksplashDefaults,
}

/// Size of a box-like cabinet kind (soffit, shelf, partition, library types).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxDefaults {
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    /// Bottom of the box above the floor.
    pub elevation: f64,
}

impl BoxDefaults {
    const fn new(width: f64, depth: f64, height: f64, elevation: f64) -> Self {
        Self {
            width,
            depth,
            height,
            elevation,
        }
    }
}

fn default_soffit() -> BoxDefaults {
    BoxDefaults::new(24.0, 12.0, 12.0, 84.0)
}

fn default_shelf() -> BoxDefaults {
    BoxDefaults::new(24.0, 12.0, 0.75, 48.0)
}

fn default_partition() -> BoxDefaults {
    BoxDefaults::new(24.0, 24.0, 36.0, 0.0)
}

fn default_vanity() -> BoxDefaults {
    BoxDefaults::new(30.0, 21.0, 34.5, 0.0)
}

fn default_pantry() -> BoxDefaults {
    BoxDefaults::new(24.0, 24.0, 84.0, 0.0)
}

fn default_tall_oven() -> BoxDefaults {
    BoxDefaults::new(30.0, 24.0, 84.0, 0.0)
}

fn default_refrigerator() -> BoxDefaults {
    BoxDefaults::new(36.0, 25.0, 84.0, 0.0)
}

/// Countertop settings a new base cabinet starts with (beyond the thickness
/// and front overhang on the Base page).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CountertopDefaults {
    pub overhang_sides: f64,
    pub overhang_back: f64,
    /// "Square", "Beveled", "Bullnose", "Ogee" or "Waterfall".
    pub edge: String,
    pub edge_size: f64,
    /// "None", "Clipped" or "Rounded".
    pub corner: String,
    pub corner_size: f64,
}

impl Default for CountertopDefaults {
    fn default() -> Self {
        Self {
            overhang_sides: 0.0,
            overhang_back: 0.0,
            edge: "Square".into(),
            edge_size: 0.75,
            corner: "None".into(),
            corner_size: 3.0,
        }
    }
}

/// Backsplash settings (off by default, as in Chief).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BacksplashDefaults {
    /// New base cabinets get a backsplash.
    pub enabled: bool,
    pub height: f64,
    pub thickness: f64,
    /// Rise to the wall cabinet above instead of `height`.
    pub full_height: bool,
}

impl Default for BacksplashDefaults {
    fn default() -> Self {
        Self {
            enabled: false,
            height: 4.0,
            thickness: 0.5,
            full_height: false,
        }
    }
}

fn default_filler_width() -> f64 {
    3.0
}

fn default_corner_base_leg() -> f64 {
    36.0
}

fn default_corner_wall_leg() -> f64 {
    24.0
}

fn default_blind_width() -> f64 {
    48.0
}

fn default_blind_hidden() -> f64 {
    15.0
}

// ----- dimensions -----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionDefaults {
    pub set_name: String,
    /// Smallest fraction denominator shown (8 means 1/8").
    pub smallest_fraction: u32,
    pub unit_indicators: bool,
    pub trailing_zeroes: bool,
    pub fraction_style: String,
    pub fraction_text_size_pct: u32,
    pub text_above_line: bool,
    pub leader_style: String,
    pub arrow_size: f64,
    pub extension_gap: f64,
    pub extension_past: f64,
    pub auto_exterior_offset: f64,
    pub auto_line_separation: f64,
    pub locate_openings_centers: bool,
    /// Name of the text style dimension text uses; empty means "Dimension
    /// Text Style" / the plan default.
    #[serde(default)]
    pub text_style: String,
    /// Extension lines: length toward the marked object, inches (0 = not set).
    #[serde(default)]
    pub extension_toward: f64,
    /// Extension lines: fixed proximity, the distance to the marked object,
    /// inches (0 = not set).
    #[serde(default)]
    pub extension_proximity: f64,
    /// Baseline dimensions: line separation, inches (0 = not set).
    #[serde(default)]
    pub baseline_separation: f64,
    /// Automatic dimensions: reach, inches (0 = not set).
    #[serde(default)]
    pub reach: f64,
    /// Decimal places shown in a decimal format (0 = not set).
    #[serde(default)]
    pub decimals: u32,
    /// Arrow style name (empty = the default arrow).
    #[serde(default)]
    pub arrow_style: String,
    /// Locate Objects, walls: the surface a dimension point snaps to (DIM-4).
    #[serde(default)]
    pub locate_walls: WallLocate,
    /// Locate Objects, openings: sides, centers or none. `None` follows the
    /// older `locate_openings_centers` flag; set it with
    /// [`DimensionDefaults::set_opening_locate`].
    #[serde(default)]
    pub locate_openings: Option<OpeningLocate>,
    /// Locate Objects, cabinets.
    #[serde(default)]
    pub locate_cabinets: ObjectLocate,
    /// Locate Objects, fixtures (placed symbols).
    #[serde(default)]
    pub locate_fixtures: ObjectLocate,
    /// Interior dimensions locate the interior surfaces of the walls
    /// (otherwise the located wall surface setting applies).
    #[serde(default = "default_true")]
    pub interior_locates_interior_surfaces: bool,
    /// The strings of Auto Exterior Dimensions, nearest the wall first;
    /// empty is Chief's set (openings, wall to wall, overall).
    #[serde(default)]
    pub auto_strings: Vec<AutoString>,
    /// Text and extension offsets are printed sizes: they hold their size on
    /// paper at any plan scale (DIM-7).
    #[serde(default)]
    pub printed_size: bool,
    /// Locate Objects of the temporary dimensions (DIM-40); `None` is the
    /// default group (wall surfaces, opening sides).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temp_locate: Option<crate::dimension::LocateGroup>,
    /// Locate Objects of the elevation dimensions (DIM-40); `None` is the
    /// default group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation_locate: Option<crate::dimension::LocateGroup>,
    /// The General, Setup, Extensions, Layer and Auto Story Pole settings of
    /// the Dimension Defaults dialog.
    #[serde(default)]
    pub setup: crate::dimension::DimSetup,
    /// The Locate panels of the dimension tools beyond the typed fields above.
    #[serde(default)]
    pub locates: crate::dimension::ToolLocates,
}

fn default_true() -> bool {
    true
}

impl DimensionDefaults {
    /// How openings are located: the explicit setting, else the older
    /// centers flag.
    pub fn opening_locate(&self) -> OpeningLocate {
        self.locate_openings
            .unwrap_or(if self.locate_openings_centers {
                OpeningLocate::Centers
            } else {
                OpeningLocate::Sides
            })
    }

    /// Sets how openings are located, keeping the older flag in step.
    pub fn set_opening_locate(&mut self, mode: OpeningLocate) {
        self.locate_openings = Some(mode);
        self.locate_openings_centers = mode == OpeningLocate::Centers;
    }

    /// The Auto Exterior strings, nearest the wall first.
    pub fn exterior_strings(&self) -> Vec<AutoString> {
        if self.auto_strings.is_empty() {
            crate::dimension::DEFAULT_AUTO_STRINGS.to_vec()
        } else {
            self.auto_strings.clone()
        }
    }

    /// The Locate Objects group of the manual and automatic dimensions: the
    /// set's own settings.
    pub fn main_locate(&self) -> crate::dimension::LocateGroup {
        crate::dimension::LocateGroup {
            walls: self.locate_walls,
            openings: self.opening_locate(),
            cabinets: self.locate_cabinets,
            fixtures: self.locate_fixtures,
        }
    }

    /// The Locate Objects group of the temporary dimensions.
    pub fn temp_group(&self) -> crate::dimension::LocateGroup {
        self.temp_locate.unwrap_or_default()
    }

    /// The Locate Objects group of the elevation dimensions.
    pub fn elevation_group(&self) -> crate::dimension::LocateGroup {
        self.elevation_locate.unwrap_or_default()
    }

    /// Distance between automatic strings; 18" when unset.
    pub fn string_spacing(&self) -> f64 {
        if self.auto_line_separation > 0.0 {
            self.auto_line_separation
        } else {
            18.0
        }
    }
}

/// One of Chief's saved dimension default sets ("1/4\" Scale", "NKBA", ...):
/// how dimension text is formatted plus the automatic-dimension settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionDefaultSet {
    /// Short name as Chief lists it, e.g. `1/4" Scale`.
    pub name: String,
    pub format: DimFormat,
    pub auto: DimensionDefaults,
}

impl DimensionDefaultSet {
    /// A set from automatic-dimension settings; the text format is derived
    /// from them and `auto.set_name` becomes `<name> Dimension Defaults`.
    pub fn new(name: impl Into<String>, mut auto: DimensionDefaults) -> Self {
        let name = name.into();
        auto.set_name = format!("{name} Dimension Defaults");
        let denominator = auto.smallest_fraction.max(1);
        let format = DimFormat {
            smallest_fraction: denominator,
            unit_indicators: auto.unit_indicators,
            length: Some(LengthFormat {
                unit: LengthUnit::FeetInches,
                fraction_denominator: denominator,
                unit_indicators: auto.unit_indicators,
                trailing_zeroes: auto.trailing_zeroes,
                ..LengthFormat::default()
            }),
            label: auto.setup.label_options(auto.text_above_line),
        };
        Self { name, format, auto }
    }

    /// The same settings under another name.
    pub fn cloned_as(&self, name: impl Into<String>) -> Self {
        Self::new(name, self.auto.clone())
    }
}

/// Chief's dimension default sets as listed in Daniel's template. Only the
/// 1/4" set was captured in detail; the others start as copies of `base`.
fn chief_dimension_sets(base: &DimensionDefaults) -> Vec<DimensionDefaultSet> {
    [
        "1\" Scale",
        "1/2\" Scale",
        "1/4\" Scale",
        "1/8\" Scale",
        "Electrical",
        "Foundation",
        "Framing",
        "HVAC",
        "Kitchen and Bath",
        "Legacy NKBA",
        "NKBA",
        "Plot Plan",
        "Roof",
    ]
    .iter()
    .map(|n| DimensionDefaultSet::new(*n, base.clone()))
    .collect()
}

/// Name of the active dimension set in Daniel's template.
pub const DEFAULT_DIMENSION_SET: &str = "1/4\" Scale";

// ----- rooms -----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomTypeDef {
    pub name: String,
    /// Standard, Living, Utility, Deck, Garage, Porch or Open Below.
    pub function: String,
    pub include_in_living_area: bool,
    pub conditioned: bool,
    /// Empty means "use the plan default".
    pub default_floor_finish: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomDefaults {
    pub ceiling_height: f64,
    pub floor_finish_thickness: f64,
    pub ceiling_finish_thickness: f64,
    pub room_types: Vec<RoomTypeDef>,
    /// Floor Defaults a new floor starts with (R-56).
    #[serde(default)]
    pub floor: crate::floors::FloorSettings,
}

// ----- text, grid, units -----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextDefaults {
    pub font: String,
    /// Plan inches (6" is 1/8" on paper at 1/4" scale).
    pub height: f64,
    pub label_style_height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridDefaults {
    pub spacing: f64,
    pub snap: f64,
    pub angle_snap_deg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitDefaults {
    pub imperial: bool,
}

// ----- the whole set -----

/// Everything a new plan starts with. Missing keys in a JSON file fall back
/// to [`PlanDefaults::chief_x18_daniel`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanDefaults {
    pub name: String,
    pub exterior_wall: WallDefaults,
    pub interior_wall: WallDefaults,
    pub foundation_wall: WallDefaults,
    /// Defaults of the pony, half, glass, railing, deck and fencing walls.
    pub wall_variants: WallVariantDefaults,
    pub wall_types: Vec<WallTypeDef>,
    pub interior_door: OpeningDefaults,
    pub exterior_door: OpeningDefaults,
    pub window: WindowDefaults,
    pub cabinets: CabinetDefaults,
    pub dimensions: DimensionDefaults,
    /// Saved dimension default sets; `dimensions` mirrors the active one.
    pub dimension_sets: Vec<DimensionDefaultSet>,
    /// Name of the active entry of `dimension_sets`.
    pub active_dimension_set: String,
    pub rooms: RoomDefaults,
    pub layers: LayerSet,
    /// Layer sets (display/lock/colour overrides over `layers`).
    pub layer_sets: LayerSets,
    pub text_styles: TextStyles,
    pub text: TextDefaults,
    pub grid: GridDefaults,
    pub units: UnitDefaults,
    /// Wall connection behaviour.
    pub walls_connect: WallConnectDefaults,
    /// Editing behaviour (snapping, bumping).
    pub editing: EditingDefaults,
    /// How door and window labels read in plan (Default Settings > Labels).
    pub opening_labels: crate::openings::OpeningLabelDefaults,
    /// Default sizes of the door and window variants.
    pub opening_variants: crate::openings::OpeningVariantDefaults,
    /// Roof detail: eave cut, fascia, soffit, rafter tails, attic walls,
    /// and the Build Roof baseline rule (Default Settings > Roof Defaults).
    pub roof_detail: RoofDetailDefaults,
    /// Starting values the plan's code minimums set (Default Settings >
    /// Plan Check > Apply code minimums to defaults).
    pub code: CodeDefaults,
    /// The values of the Default Settings pages that have no typed slot of
    /// their own (CAD, Camera Tools, Schedules, Text, ...), by
    /// `"<page>.<field>"`. A page field that is missing here reads as the
    /// page's built-in value, so only changed values are stored; see
    /// [`PageValue`] and `plan-app`'s `dialogs/default_pages`.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub pages: std::collections::BTreeMap<String, PageValue>,
}

/// One stored value of a Default Settings page field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PageValue {
    Bool(bool),
    /// A whole number (counts, percentages, undo levels).
    Int(i64),
    /// A length (inches), an angle (degrees) or another measure.
    Num(f64),
    /// A name or a choice.
    Text(String),
}

impl PageValue {
    /// The value as a number (a flag is 0 or 1, text is 0).
    pub fn num(&self) -> f64 {
        match self {
            PageValue::Bool(b) => f64::from(u8::from(*b)),
            PageValue::Int(i) => *i as f64,
            PageValue::Num(n) => *n,
            PageValue::Text(t) => t.trim().parse().unwrap_or(0.0),
        }
    }

    /// The value as a whole number.
    pub fn int(&self) -> i64 {
        self.num().round() as i64
    }

    /// The value as a flag.
    pub fn flag(&self) -> bool {
        match self {
            PageValue::Bool(b) => *b,
            PageValue::Text(t) => matches!(t.as_str(), "true" | "yes" | "on"),
            other => other.num() != 0.0,
        }
    }

    /// The value as text.
    pub fn text(&self) -> String {
        match self {
            PageValue::Bool(b) => b.to_string(),
            PageValue::Int(i) => i.to_string(),
            PageValue::Num(n) => n.to_string(),
            PageValue::Text(t) => t.clone(),
        }
    }

    /// Equal values regardless of how they are stored (`Int(2)` is `Num(2.0)`).
    pub fn same(&self, other: &PageValue) -> bool {
        match (self, other) {
            (PageValue::Text(a), PageValue::Text(b)) => a == b,
            (PageValue::Text(_), _) | (_, PageValue::Text(_)) => false,
            (a, b) => (a.num() - b.num()).abs() < 1e-9,
        }
    }
}

impl PlanDefaults {
    /// The stored value of page field `key`, if the page changed it.
    pub fn page_value(&self, key: &str) -> Option<&PageValue> {
        self.pages.get(key)
    }

    /// A stored number, else `fallback`.
    pub fn page_num(&self, key: &str, fallback: f64) -> f64 {
        self.pages.get(key).map_or(fallback, PageValue::num)
    }

    /// A stored flag, else `fallback`.
    pub fn page_flag(&self, key: &str, fallback: bool) -> bool {
        self.pages.get(key).map_or(fallback, PageValue::flag)
    }

    /// A stored text, else `fallback`.
    pub fn page_text(&self, key: &str, fallback: &str) -> String {
        self.pages
            .get(key)
            .map_or_else(|| fallback.to_string(), PageValue::text)
    }

    /// Stores page field `key`; a value equal to the built-in `builtin`
    /// removes the entry so the template stays small.
    pub fn set_page_value(&mut self, key: &str, value: PageValue, builtin: &PageValue) {
        if value.same(builtin) {
            self.pages.remove(key);
        } else {
            self.pages.insert(key.to_string(), value);
        }
    }

    /// Forgets every stored value of the page whose keys start with `prefix`
    /// (a page's Reset button).
    pub fn clear_page(&mut self, prefix: &str) {
        self.pages.retain(|k, _| !k.starts_with(prefix));
    }
}

/// The code-legal values that tools and dialogs start from where the plan
/// defaults have no other slot: stairs, railings, the bedroom window, the
/// footing and the garage wall type. Inches. The app seeds them from the
/// plan's code minimums (`apply_code_minimums` in plan-app); the values here
/// are the 2021 IRC ones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CodeDefaults {
    /// Riser height a new stair aims for (R311.7.5.1: 7 3/4" at most).
    pub stair_riser: f64,
    /// Tread depth (R311.7.5.2: 10" at least).
    pub stair_tread: f64,
    /// Stair width (R311.7.1: 36" at least).
    pub stair_width: f64,
    /// Headroom (R311.7.2: 80" at least).
    pub stair_headroom: f64,
    /// Guard height (R312.1.2: 36").
    pub guard_height: f64,
    /// Handrail height (R311.7.8.1: 34" to 38").
    pub handrail_height: f64,
    /// Widest opening in a guard (R312.1.3: a 4" sphere must not pass).
    pub baluster_opening: f64,
    /// The window a new bedroom window starts as: egress-sized.
    pub bedroom_window: WindowDefaults,
    /// Footing width under a foundation wall (Table R403.1(1)).
    pub footing_width: f64,
    /// Footing thickness (R403.1.1: 6" at least).
    pub footing_thickness: f64,
    /// Wall type for the wall between a garage and the house ("" = none).
    pub garage_wall_type: String,
}

impl Default for CodeDefaults {
    fn default() -> Self {
        Self {
            stair_riser: 7.5,
            stair_tread: 10.0,
            stair_width: 36.0,
            stair_headroom: 80.0,
            guard_height: 36.0,
            handrail_height: 36.0,
            baluster_opening: 4.0,
            bedroom_window: WindowDefaults {
                width: 36.0,
                height: 60.0,
                sill_height: 36.0,
                window_type: "Single Casement".into(),
                frame_width: 0.75,
                sash_width: 1.5,
                lites_across: 1,
                lites_vertical: 1,
                egress: true,
                tempered: false,
            },
            footing_width: crate::floors::WALL_FOOTING.0,
            footing_thickness: crate::floors::WALL_FOOTING.1,
            garage_wall_type: String::new(),
        }
    }
}

/// How walls connect when drawn or edited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallConnectDefaults {
    /// Split a wall where another wall meets it in a tee.
    pub split_on_tee: bool,
    /// Minimum connect distance, inches.
    pub connect_distance_min: f64,
}

impl Default for WallConnectDefaults {
    fn default() -> Self {
        Self {
            split_on_tee: true,
            connect_distance_min: 6.0,
        }
    }
}

/// What dragging an edit handle or an object's body does (Edit > Edit
/// Behaviors, S-65).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EditBehavior {
    /// Move and resize in place.
    #[default]
    Default,
    /// A body drag scales the selection from the opposite corner.
    Resize,
    /// A body drag of a polyline leaves the original and adds an offset copy.
    Concentric,
    /// Dragging a polyline corner rounds it with a fillet.
    Fillet,
    /// Dragging a polyline corner cuts it off with a chamfer.
    Chamfer,
    /// A body drag moves along the dominant axis only.
    Alternate,
    /// A body drag leaves the original and places copies at the drag delta.
    Replicate,
}

impl EditBehavior {
    pub const ALL: [EditBehavior; 7] = [
        EditBehavior::Default,
        EditBehavior::Resize,
        EditBehavior::Concentric,
        EditBehavior::Fillet,
        EditBehavior::Chamfer,
        EditBehavior::Alternate,
        EditBehavior::Replicate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EditBehavior::Default => "Default",
            EditBehavior::Resize => "Resize",
            EditBehavior::Concentric => "Concentric",
            EditBehavior::Fillet => "Fillet",
            EditBehavior::Chamfer => "Chamfer",
            EditBehavior::Alternate => "Alternate",
            EditBehavior::Replicate => "Replicate",
        }
    }
}

/// The mode and the parameters of each mode (Edit > Edit Behaviors).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditBehaviorSettings {
    pub mode: EditBehavior,
    /// Resize: keep the proportions (one scale for both axes).
    pub resize_proportional: bool,
    /// Concentric: offset distance, inches; 0 follows the drag.
    pub concentric_distance: f64,
    /// Concentric: how many offset copies.
    pub concentric_copies: u32,
    /// Fillet: radius, inches; 0 follows the drag.
    pub fillet_radius: f64,
    /// Chamfer: distance cut back along both sides of the corner, inches;
    /// 0 follows the drag.
    pub chamfer_distance: f64,
    /// Alternate: lock the move to the dominant axis.
    pub alternate_lock_axis: bool,
    /// Replicate: copies placed, each one more delta along.
    pub replicate_copies: u32,
    /// Replicate: after the drag, open Transform/Replicate Object with the
    /// drag as its Move and the copy count, instead of placing the copies.
    pub replicate_dialog: bool,
}

impl Default for EditBehaviorSettings {
    fn default() -> Self {
        Self {
            mode: EditBehavior::Default,
            resize_proportional: false,
            concentric_distance: 0.0,
            concentric_copies: 1,
            fillet_radius: 0.0,
            chamfer_distance: 0.0,
            alternate_lock_axis: true,
            replicate_copies: 1,
            replicate_dialog: false,
        }
    }
}

/// Editing preferences (Daniel's Chief setup: Bumping on at distance 5), the
/// Snap Settings (S-68..S-72, CAD-40) and the Edit Behaviors (S-65).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditingDefaults {
    /// Angle snap increment, degrees (0, 15, 30, 45 or 90 in the dialog).
    pub angle_snap_deg: f64,
    pub snap_distance_px: f64,
    pub bumping: bool,
    /// Bumping distance, inches.
    pub bumping_distance: f64,
    /// Master switch of the object snaps.
    pub object_snaps: bool,
    pub snap_endpoint: bool,
    pub snap_midpoint: bool,
    pub snap_intersection: bool,
    pub snap_perpendicular: bool,
    pub snap_on_object: bool,
    pub snap_center: bool,
    pub snap_quadrant: bool,
    pub snap_tangent: bool,
    /// Snap to the extension of a wall or CAD line beyond its end.
    pub snap_extension: bool,
    /// Snap to CAD points and markers.
    pub snap_markers: bool,
    pub grid_snaps: bool,
    pub angle_snaps: bool,
    /// Allowed drawing angles, degrees counter-clockwise from east, each one
    /// also allowing its opposite. Empty: every multiple of `angle_snap_deg`.
    pub snap_angles: Vec<f64>,
    pub behavior: EditBehaviorSettings,
}

impl Default for EditingDefaults {
    fn default() -> Self {
        Self {
            angle_snap_deg: 15.0,
            snap_distance_px: 10.0,
            bumping: true,
            bumping_distance: 5.0,
            object_snaps: true,
            snap_endpoint: true,
            snap_midpoint: true,
            snap_intersection: true,
            snap_perpendicular: true,
            snap_on_object: true,
            snap_center: true,
            snap_quadrant: true,
            snap_tangent: true,
            snap_extension: false,
            snap_markers: true,
            grid_snaps: true,
            angle_snaps: true,
            snap_angles: Vec::new(),
            behavior: EditBehaviorSettings::default(),
        }
    }
}

impl Default for PlanDefaults {
    fn default() -> Self {
        Self::chief_x18_daniel()
    }
}

/// Thickness used when a wall type name is not in the list.
const FALLBACK_FOUNDATION_THICKNESS: f64 = 8.0;

impl PlanDefaults {
    pub fn wall_type(&self, name: &str) -> Option<&WallTypeDef> {
        self.wall_types.iter().find(|t| t.name == name)
    }

    pub fn room_type(&self, name: &str) -> Option<&RoomTypeDef> {
        self.rooms.room_types.iter().find(|t| t.name == name)
    }

    fn thickness_of(&self, w: &WallDefaults, fallback: f64) -> f64 {
        self.wall_type(&w.wall_type)
            .map_or(fallback, WallTypeDef::thickness)
    }

    pub fn exterior_thickness(&self) -> f64 {
        self.thickness_of(&self.exterior_wall, crate::DEFAULT_EXTERIOR_THICKNESS)
    }

    pub fn interior_thickness(&self) -> f64 {
        self.thickness_of(&self.interior_wall, crate::DEFAULT_INTERIOR_THICKNESS)
    }

    pub fn foundation_thickness(&self) -> f64 {
        self.thickness_of(&self.foundation_wall, FALLBACK_FOUNDATION_THICKNESS)
    }

    /// Wall defaults for a wall kind (foundation walls are exterior walls
    /// with their own defaults, so they are not reachable from here).
    pub fn walls_for(&self, kind: WallKind) -> &WallDefaults {
        match kind {
            WallKind::Exterior => &self.exterior_wall,
            WallKind::Interior => &self.interior_wall,
        }
    }

    pub fn dimension_set(&self, name: &str) -> Option<&DimensionDefaultSet> {
        self.dimension_sets.iter().find(|s| s.name == name)
    }

    /// The active dimension default set, if its name is in the list.
    pub fn active_dimension(&self) -> Option<&DimensionDefaultSet> {
        self.dimension_set(&self.active_dimension_set)
    }

    /// Makes `name` the active dimension set and loads its settings into
    /// `dimensions`. `false` (and no change) if there is no such set.
    pub fn set_active_dimension_set(&mut self, name: &str) -> bool {
        let Some(set) = self.dimension_set(name) else {
            return false;
        };
        let auto = set.auto.clone();
        self.active_dimension_set = name.to_string();
        self.dimensions = auto;
        true
    }

    /// The dimension text format these defaults describe.
    pub fn dim_format(&self) -> DimFormat {
        DimFormat {
            smallest_fraction: self.dimensions.smallest_fraction.max(1),
            unit_indicators: self.dimensions.unit_indicators,
            length: None,
            label: self.dimensions.setup.label_options(self.dimensions.text_above_line),
        }
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }

    /// Reads a defaults file; any problem is returned as text.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = std::fs::read_to_string(path.as_ref()).map_err(|e| e.to_string())?;
        Self::from_json(&text).map_err(|e| e.to_string())
    }

    /// Reads a defaults file, falling back to Daniel's Chief X18 template
    /// when the file is missing or unreadable.
    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        Self::load(path).unwrap_or_else(|_| Self::chief_x18_daniel())
    }

    /// Daniel's Chief X18 template, as captured in `docs/chief-x18-dialogs.md`.
    pub fn chief_x18_daniel() -> Self {
        let layers = LayerSet::default_floor_plan();
        let roof = WallRoofDefaults {
            pitch_in_12: 8.0,
            overhang: 16.0,
            kind: RoofWallKind::Hip,
        };
        let door = |width, casing_width, casing_depth, thickness| OpeningDefaults {
            width,
            height: 96.0,
            thickness,
            style: "Door P04".into(),
            casing_width,
            casing_depth,
            reveal: 0.25,
            jamb_width: 0.75,
            swing_angle: 90.0,
            sill_height: 0.0,
        };
        let dimensions = DimensionDefaults {
            set_name: "1/4\" Scale Dimension Defaults".into(),
            smallest_fraction: 8,
            unit_indicators: true,
            trailing_zeroes: true,
            fraction_style: "Diagonal".into(),
            fraction_text_size_pct: 60,
            text_above_line: true,
            leader_style: "Square Corner".into(),
            arrow_size: 2.25,
            extension_gap: 3.0,
            extension_past: 3.0,
            auto_exterior_offset: 32.0,
            auto_line_separation: 18.0,
            locate_openings_centers: true,
            text_style: String::new(),
            extension_toward: 0.0,
            extension_proximity: 0.0,
            baseline_separation: 0.0,
            reach: 0.0,
            decimals: 0,
            arrow_style: String::new(),
            locate_walls: WallLocate::MainLayer,
            locate_openings: None,
            locate_cabinets: ObjectLocate::Sides,
            locate_fixtures: ObjectLocate::Sides,
            interior_locates_interior_surfaces: true,
            auto_strings: Vec::new(),
            printed_size: false,
            temp_locate: None,
            elevation_locate: None,
            setup: Default::default(),
            locates: Default::default(),
        };
        PlanDefaults {
            name: "Chief X18 (Daniel)".into(),
            exterior_wall: WallDefaults {
                wall_type: "Stucco-6".into(),
                height: 109.125,
                roof: roof.clone(),
            },
            interior_wall: WallDefaults {
                wall_type: "Interior-4".into(),
                height: 109.125,
                roof: roof.clone(),
            },
            foundation_wall: WallDefaults {
                wall_type: "Foundation-8".into(),
                // Not captured from Chief; a typical stem-wall height.
                height: 48.0,
                roof,
            },
            wall_variants: WallVariantDefaults::default(),
            wall_types: chief_wall_types(),
            interior_door: door(30.0, 3.5, 0.75, 1.375),
            // Casing is the exterior casing (3 1/4" x 1"); thickness is not
            // captured, 1 3/4" is the usual exterior door.
            exterior_door: door(36.0, 3.25, 1.0, 1.75),
            window: WindowDefaults {
                width: 32.0,
                height: 72.0,
                sill_height: 24.0,
                window_type: "Single Casement".into(),
                frame_width: 0.75,
                sash_width: 1.5,
                lites_across: 1,
                lites_vertical: 1,
                egress: true,
                tempered: true,
            },
            cabinets: CabinetDefaults {
                base: BaseCabinetDefaults {
                    width: 24.0,
                    depth: 24.0,
                    height: 36.0,
                    countertop_thickness: 1.5,
                    countertop_overhang: 1.0,
                    toe_kick_height: 4.0,
                    toe_kick_depth: 3.0,
                    door_style: "Lincoln Door".into(),
                    drawer_style: "Lincoln Flat Panel Drawer".into(),
                    handle: "Knob".into(),
                },
                wall: WallCabinetDefaults {
                    width: 24.0,
                    depth: 12.0,
                    height: 30.0,
                    elevation: 54.0,
                },
                full_height: FullHeightCabinetDefaults {
                    width: 24.0,
                    depth: 24.0,
                    height: 84.0,
                },
                filler_width: default_filler_width(),
                corner_base_leg: default_corner_base_leg(),
                corner_wall_leg: default_corner_wall_leg(),
                blind_base_width: default_blind_width(),
                blind_hidden_width: default_blind_hidden(),
                soffit: default_soffit(),
                shelf: default_shelf(),
                partition: default_partition(),
                vanity: default_vanity(),
                pantry: default_pantry(),
                tall_oven: default_tall_oven(),
                refrigerator: default_refrigerator(),
                countertop: CountertopDefaults::default(),
                backsplash: BacksplashDefaults::default(),
            },
            dimensions: dimensions.clone(),
            dimension_sets: chief_dimension_sets(&dimensions),
            active_dimension_set: DEFAULT_DIMENSION_SET.into(),
            rooms: RoomDefaults {
                ceiling_height: DEFAULT_CEILING_HEIGHT,
                floor_finish_thickness: 0.75,
                ceiling_finish_thickness: 0.625,
                room_types: chief_room_types(),
                floor: crate::floors::FloorSettings::default(),
            },
            layer_sets: LayerSets::from_layers(&layers),
            layers,
            text_styles: TextStyles::chief_defaults(),
            text: TextDefaults {
                font: "Arial".into(),
                height: 6.0,
                label_style_height: 4.5,
            },
            grid: GridDefaults {
                spacing: 12.0,
                snap: 1.0,
                angle_snap_deg: 15.0,
            },
            units: UnitDefaults { imperial: true },
            walls_connect: WallConnectDefaults::default(),
            editing: EditingDefaults::default(),
            opening_labels: crate::openings::OpeningLabelDefaults::default(),
            opening_variants: crate::openings::OpeningVariantDefaults::default(),
            roof_detail: RoofDetailDefaults::default(),
            code: CodeDefaults::default(),
            pages: Default::default(),
        }
    }
}

fn wall_type(name: &str, kind: WallKind, layers: Vec<WallLayer>) -> WallTypeDef {
    WallTypeDef {
        name: name.into(),
        layers,
        kind,
    }
}

fn chief_wall_types() -> Vec<WallTypeDef> {
    let l = WallLayer::new;
    let ext = WallKind::Exterior;
    let int = WallKind::Interior;
    vec![
        wall_type(
            "Stucco-6",
            ext,
            vec![
                // The captured total is 7 5/8"; the stucco layer carries the
                // difference (1 1/16") so the stack adds up.
                l("Stucco", 1.0625, false, "Sand Finish - Eggshell"),
                // Housewrap (1/16") folded into the OSB (1/2").
                l("Sheathing", 0.5625, false, "OSB-Hrz + Housewrap"),
                l("Framing", 5.5, true, "Fir Framing"),
                l("Drywall", 0.5, false, "Drywall"),
            ],
        ),
        wall_type(
            "Siding-6",
            ext,
            vec![
                l("Siding", 0.5, false, "Siding"),
                l("Sheathing", 0.5, false, "OSB-Hrz"),
                l("Framing", 5.5, true, "Fir Framing"),
                l("Drywall", 0.5, false, "Drywall"),
            ],
        ),
        wall_type(
            "Brick-6",
            ext,
            vec![
                l("Brick", 3.625, false, "Brick"),
                l("Air Space", 1.0, false, "Air"),
                l("Sheathing", 0.5, false, "OSB-Hrz"),
                l("Framing", 5.5, true, "Fir Framing"),
                l("Drywall", 0.5, false, "Drywall"),
            ],
        ),
        wall_type(
            "Foundation-8",
            ext,
            vec![l("Concrete", 8.0, true, "Concrete")],
        ),
        wall_type(
            "stone-6",
            ext,
            vec![
                // Not captured in detail; a stone veneer on a 2x6 wall.
                l("Stone", 1.5, false, "Stone Veneer"),
                l("Sheathing", 0.5, false, "OSB-Hrz"),
                l("Framing", 5.5, true, "Fir Framing"),
                l("Drywall", 0.5, false, "Drywall"),
            ],
        ),
        wall_type("Glass-1", ext, vec![l("Glass", 1.0, true, "Glass")]),
        wall_type("Railing-4", int, vec![l("Rail", 4.0, true, "Wood")]),
        wall_type("Deck Railing-4", ext, vec![l("Rail", 4.0, true, "Wood")]),
        wall_type("Deck Edge-2", ext, vec![l("Rim Board", 1.5, true, "Wood")]),
        wall_type("Fence-Wood-2", ext, vec![l("Boards", 1.5, true, "Wood")]),
        wall_type(
            "Interior-4",
            int,
            vec![
                l("Drywall", 0.5, false, "Drywall"),
                l("Framing", 3.5, true, "Fir Framing"),
                l("Drywall", 0.5, false, "Drywall"),
            ],
        ),
        wall_type(
            "Interior-6",
            int,
            vec![
                l("Drywall", 0.5, false, "Drywall"),
                l("Framing", 5.5, true, "Fir Framing"),
                l("Drywall", 0.5, false, "Drywall"),
            ],
        ),
    ]
}

fn chief_room_types() -> Vec<RoomTypeDef> {
    // (name, function, include in living area, conditioned)
    const STD: (&str, bool, bool) = ("Standard", true, true);
    const UTIL: (&str, bool, bool) = ("Utility", true, true);
    const EXCLUDED_UTIL: (&str, bool, bool) = ("Utility", false, false);
    const DECK: (&str, bool, bool) = ("Deck", false, false);
    let rows: &[(&str, (&str, bool, bool))] = &[
        ("Attic", EXCLUDED_UTIL),
        ("Balcony", DECK),
        ("Bath", STD),
        ("Bedroom", STD),
        ("Bedroom #2", STD),
        ("Bedroom #3", STD),
        ("Bedroom #4", STD),
        ("Bedroom #5", STD),
        ("Bonus Room", STD),
        ("Breakfast", STD),
        ("Closet", STD),
        ("Courtyard", ("Standard", false, false)),
        ("Crawl Space", EXCLUDED_UTIL),
        ("Deck", DECK),
        ("Den", STD),
        ("Dinette", STD),
        ("Dining", STD),
        ("Dining Room", STD),
        ("Dressing Room", STD),
        ("Entry", STD),
        ("Family Room", STD),
        ("Flat Roof", EXCLUDED_UTIL),
        ("Foyer", STD),
        ("Garage", ("Garage", false, true)),
        ("Great Room", STD),
        ("Hall", STD),
        ("Kitchen", STD),
        ("Laundry", UTIL),
        ("Library", STD),
        ("Living", STD),
        ("Loft", STD),
        ("Master Bath", STD),
        ("Master Bedroom", STD),
        ("Mechanical", UTIL),
        ("Mud Room", STD),
        ("Nook", STD),
        ("Office", STD),
        ("Open Below", ("Open Below", false, true)),
        ("Pantry", STD),
        ("Porch", ("Porch", false, false)),
        ("Powder Room", STD),
        ("Slab", UTIL),
        ("Storage", UTIL),
        ("Study", STD),
    ];
    let mut types: Vec<RoomTypeDef> = rows
        .iter()
        .map(|(name, (function, living, cond))| RoomTypeDef {
            name: (*name).into(),
            function: (*function).into(),
            include_in_living_area: *living,
            conditioned: *cond,
            default_floor_finish: String::new(),
        })
        .collect();
    for name in ["Unspecified", "Utility"] {
        types.push(RoomTypeDef {
            name: name.into(),
            function: if name == "Utility" {
                "Utility"
            } else {
                "Standard"
            }
            .into(),
            include_in_living_area: true,
            conditioned: true,
            default_floor_finish: String::new(),
        });
    }
    types
}

impl Project {
    /// A new project that starts from `d`: its first-floor ceiling height
    /// and layer set.
    pub fn from_defaults(name: impl Into<String>, d: &PlanDefaults) -> Project {
        let mut p = Project::new(name);
        p.floors[0].ceiling_height = d.rooms.ceiling_height;
        p.floors[0].settings = d.rooms.floor.clone();
        p.layers = d.layers.clone();
        p.layer_sets = d.layer_sets.clone();
        p.text_styles = d.text_styles.clone();
        // The starting view shows the defaults' active layer set.
        for v in &mut p.plan_views {
            v.layer_set = d.layer_sets.active.clone();
        }
        p.wall_types = d.wall_types.clone();
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_wall_type_thicknesses() {
        let d = PlanDefaults::chief_x18_daniel();
        let t = |n: &str| d.wall_type(n).unwrap().thickness();
        assert_eq!(t("Stucco-6"), 7.625);
        assert_eq!(t("Interior-4"), 4.5);
        assert_eq!(t("Siding-6"), 7.0);
        assert_eq!(t("Brick-6"), 11.125);
        assert_eq!(t("Interior-6"), 6.5);
        assert_eq!(t("Foundation-8"), 8.0);
        assert_eq!(d.exterior_thickness(), 7.625);
        assert_eq!(d.interior_thickness(), 4.5);
        assert_eq!(d.foundation_thickness(), 8.0);
        assert!(d.wall_type("nope").is_none());
    }

    #[test]
    fn main_layer_offsets() {
        let d = PlanDefaults::chief_x18_daniel();
        let off = |n: &str| d.wall_type(n).unwrap().main_layer_offset();
        assert_eq!(off("Stucco-6"), 1.625);
        assert_eq!(off("Interior-4"), 0.5);
        assert_eq!(off("Foundation-8"), 0.0);
        assert_eq!(off("Brick-6"), 5.125);
        let none = WallTypeDef {
            name: "x".into(),
            kind: WallKind::Interior,
            layers: vec![WallLayer::new("A", 1.0, false, "")],
        };
        assert_eq!(none.main_layer_offset(), 0.0);
        // Every shipped type has exactly one main layer.
        for t in &d.wall_types {
            assert_eq!(
                t.layers.iter().filter(|l| l.is_main).count(),
                1,
                "{}",
                t.name
            );
        }
    }

    #[test]
    fn captured_values() {
        let d = PlanDefaults::chief_x18_daniel();
        assert_eq!(d.exterior_wall.height, 109.125);
        assert_eq!(d.exterior_wall.roof.pitch_in_12, 8.0);
        assert_eq!(d.exterior_wall.roof.kind, RoofWallKind::Hip);
        assert_eq!(d.interior_door.width, 30.0);
        assert_eq!(d.exterior_door.casing_width, 3.25);
        assert_eq!(d.window.sill_height, 24.0);
        assert_eq!(d.cabinets.base.countertop_thickness, 1.5);
        assert_eq!(d.dimensions.smallest_fraction, 8);
        assert_eq!(d.dim_format().fmt_len(109.125), "9'-1 1/8\"");
        // 1/8" resolution: 1/16" values round to the nearest eighth.
        assert_eq!(d.dim_format().fmt_len(10.03125), "0'-10\"");
        assert_eq!(d.dim_format().fmt_len(10.1875), "0'-10 1/4\"");
    }

    #[test]
    fn json_round_trip() {
        let d = PlanDefaults::chief_x18_daniel();
        let back = PlanDefaults::from_json(&d.to_json().unwrap()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn partial_json_falls_back_per_field() {
        let d = PlanDefaults::from_json(r#"{"name": "Mine"}"#).unwrap();
        assert_eq!(d.name, "Mine");
        assert_eq!(d.exterior_wall.wall_type, "Stucco-6");
        assert!(PlanDefaults::from_json("not json").is_err());
    }

    #[test]
    fn room_type_lookup() {
        let d = PlanDefaults::chief_x18_daniel();
        let fr = d.room_type("Flat Roof").unwrap();
        assert_eq!(fr.function, "Utility");
        assert!(!fr.include_in_living_area && !fr.conditioned);
        let g = d.room_type("Garage").unwrap();
        assert_eq!(g.function, "Garage");
        assert!(!g.include_in_living_area);
        let k = d.room_type("Kitchen").unwrap();
        assert!(k.include_in_living_area && k.conditioned);
        assert!(d.room_type("Bedroom #5").is_some());
        assert!(d.room_type("Utility").is_some());
        assert!(d.room_type("Nope").is_none());
        let mut names: Vec<_> = d.rooms.room_types.iter().map(|t| &t.name).collect();
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n, "duplicate room type");
    }

    #[test]
    fn from_defaults_applies_ceiling_and_layers() {
        let mut d = PlanDefaults::chief_x18_daniel();
        d.rooms.ceiling_height = 120.0;
        d.layers.set_display("Doors", false);
        let p = Project::from_defaults("Mine", &d);
        assert_eq!(p.name, "Mine");
        assert_eq!(p.floors[0].ceiling_height, 120.0);
        assert_eq!(p.wall_types.len(), d.wall_types.len());
        assert!(!p.layers.is_visible("Doors"));
    }

    #[test]
    fn locate_objects_settings_default_and_load_from_old_files() {
        let d = PlanDefaults::default();
        let a = &d.dimensions;
        assert_eq!(a.locate_walls, WallLocate::MainLayer);
        assert_eq!(a.locate_cabinets, ObjectLocate::Sides);
        assert_eq!(a.locate_fixtures, ObjectLocate::Sides);
        assert!(a.interior_locates_interior_surfaces);
        assert!(!a.printed_size);
        // Daniel's template locates opening centers, through the older flag.
        assert!(a.locate_openings_centers && a.locate_openings.is_none());
        assert_eq!(a.opening_locate(), OpeningLocate::Centers);
        let mut b = a.clone();
        b.set_opening_locate(OpeningLocate::Sides);
        assert_eq!(b.opening_locate(), OpeningLocate::Sides);
        assert!(!b.locate_openings_centers);
        b.set_opening_locate(OpeningLocate::None);
        assert_eq!(b.opening_locate(), OpeningLocate::None);
        assert_eq!(
            a.exterior_strings(),
            vec![
                AutoString::Openings,
                AutoString::WallToWall,
                AutoString::Overall
            ]
        );
        assert_eq!(a.string_spacing(), 18.0);
        // A dimension set saved before these settings loads with the defaults.
        let mut v = serde_json::to_value(a).unwrap();
        for k in [
            "locate_walls",
            "locate_openings",
            "locate_cabinets",
            "locate_fixtures",
            "interior_locates_interior_surfaces",
            "auto_strings",
            "printed_size",
        ] {
            v.as_object_mut().unwrap().remove(k);
        }
        let old: DimensionDefaults = serde_json::from_value(v).unwrap();
        assert_eq!(&old, a);
        // And the saved sets carry their own.
        let mut sets = d.dimension_sets.clone();
        sets[0].auto.locate_walls = WallLocate::Centers;
        sets[0].auto.auto_strings = vec![AutoString::Overall];
        let back: Vec<DimensionDefaultSet> =
            serde_json::from_str(&serde_json::to_string(&sets).unwrap()).unwrap();
        assert_eq!(back[0].auto.locate_walls, WallLocate::Centers);
        assert_eq!(back[0].auto.exterior_strings(), vec![AutoString::Overall]);
    }

    #[test]
    fn dimension_sets_seeded_with_active_quarter_scale() {
        let mut d = PlanDefaults::chief_x18_daniel();
        let names: Vec<_> = d.dimension_sets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "1\" Scale",
                "1/2\" Scale",
                "1/4\" Scale",
                "1/8\" Scale",
                "Electrical",
                "Foundation",
                "Framing",
                "HVAC",
                "Kitchen and Bath",
                "Legacy NKBA",
                "NKBA",
                "Plot Plan",
                "Roof"
            ]
        );
        assert_eq!(d.active_dimension_set, "1/4\" Scale");
        let active = d.active_dimension().unwrap();
        assert_eq!(active.format.smallest_fraction, 8);
        assert_eq!(active.auto, d.dimensions);
        assert_eq!(active.format.fmt_len(109.125), "9'-1 1/8\"");
        assert_eq!(active.auto.set_name, "1/4\" Scale Dimension Defaults");
        // Switching the active set loads its settings into `dimensions`.
        d.dimension_sets[0].auto.smallest_fraction = 16;
        assert!(d.set_active_dimension_set("1\" Scale"));
        assert_eq!(d.dimensions.smallest_fraction, 16);
        assert_eq!(d.dim_format().smallest_fraction, 16);
        assert!(!d.set_active_dimension_set("nope"));
        assert_eq!(d.active_dimension_set, "1\" Scale");
    }

    #[test]
    fn layer_sets_and_text_styles_in_defaults_and_old_json() {
        let d = PlanDefaults::chief_x18_daniel();
        assert_eq!(d.layer_sets.sets.len(), 1);
        assert_eq!(d.layer_sets.effective(&d.layers), d.layers);
        assert!(d.text_styles.get("Default Text Style").is_some());
        // Old defaults JSON without the new fields still loads.
        let mut v: serde_json::Value = serde_json::from_str(&d.to_json().unwrap()).unwrap();
        for k in [
            "layer_sets",
            "text_styles",
            "dimension_sets",
            "active_dimension_set",
        ] {
            assert!(v.as_object_mut().unwrap().remove(k).is_some());
        }
        let old = PlanDefaults::from_json(&v.to_string()).unwrap();
        assert_eq!(old, d);
        let p = Project::from_defaults("X", &d);
        assert_eq!(p.layer_sets, d.layer_sets);
        assert_eq!(p.text_styles, d.text_styles);
        assert_eq!(
            p.current_plan_view().unwrap().layer_set,
            d.layer_sets.active
        );
    }

    #[test]
    fn load_or_default_handles_missing_and_bad_files() {
        let d = PlanDefaults::load_or_default("/nonexistent/plan-studio/defaults.json");
        assert_eq!(d, PlanDefaults::chief_x18_daniel());
        let dir = std::env::temp_dir().join(format!("plan-core-defaults-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("d.json");
        let mut custom = PlanDefaults::chief_x18_daniel();
        custom.window.width = 40.0;
        std::fs::write(&path, custom.to_json().unwrap()).unwrap();
        assert_eq!(PlanDefaults::load_or_default(&path).window.width, 40.0);
        std::fs::write(&path, "garbage").unwrap();
        assert!(PlanDefaults::load(&path).is_err());
        assert_eq!(PlanDefaults::load_or_default(&path).window.width, 32.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn connect_and_editing_defaults() {
        let d = PlanDefaults::chief_x18_daniel();
        assert!(d.walls_connect.split_on_tee);
        assert_eq!(d.walls_connect.connect_distance_min, 6.0);
        assert_eq!(d.editing.angle_snap_deg, 15.0);
        assert_eq!(d.editing.snap_distance_px, 10.0);
        assert!(d.editing.bumping);
        assert_eq!(d.editing.bumping_distance, 5.0);
        // Round trip, and an old file without the keys still loads.
        let back: PlanDefaults = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
        let mut v = serde_json::to_value(&d).unwrap();
        let o = v.as_object_mut().unwrap();
        o.remove("walls_connect");
        o.remove("editing");
        let old: PlanDefaults = serde_json::from_value(v).unwrap();
        assert_eq!(old.walls_connect, WallConnectDefaults::default());
        assert_eq!(old.editing, EditingDefaults::default());
        let partial: PlanDefaults =
            serde_json::from_str(r#"{"editing":{"bumping_distance":8.0}}"#).unwrap();
        assert_eq!(partial.editing.bumping_distance, 8.0);
        assert!(partial.editing.bumping);
        // The snap and behavior settings added later default on / Default.
        assert!(partial.editing.object_snaps && partial.editing.snap_tangent);
        assert!(partial.editing.snap_angles.is_empty());
        assert_eq!(partial.editing.behavior.mode, EditBehavior::Default);
    }

    #[test]
    fn edit_behavior_settings_round_trip() {
        let mut d = PlanDefaults::chief_x18_daniel();
        d.editing.behavior.mode = EditBehavior::Fillet;
        d.editing.behavior.fillet_radius = 18.0;
        d.editing.snap_angles = vec![0.0, 45.0, 90.0];
        d.editing.snap_center = false;
        let back: PlanDefaults = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back.editing, d.editing);
        assert_eq!(EditBehavior::ALL.len(), 7);
        assert_eq!(EditBehavior::Concentric.label(), "Concentric");
    }

    #[test]
    fn cabinet_defaults_added_later_fall_back_when_missing() {
        let d = PlanDefaults::chief_x18_daniel();
        let mut v = serde_json::to_value(&d.cabinets).unwrap();
        let o = v.as_object_mut().unwrap();
        for k in [
            "filler_width",
            "corner_base_leg",
            "corner_wall_leg",
            "blind_base_width",
            "blind_hidden_width",
            "soffit",
            "shelf",
            "partition",
            "vanity",
            "pantry",
            "tall_oven",
            "refrigerator",
            "countertop",
            "backsplash",
        ] {
            o.remove(k);
        }
        let back: CabinetDefaults = serde_json::from_value(v).unwrap();
        assert_eq!(back, d.cabinets);
        assert_eq!(back.filler_width, 3.0);
        assert_eq!((back.corner_base_leg, back.corner_wall_leg), (36.0, 24.0));
    }
}
