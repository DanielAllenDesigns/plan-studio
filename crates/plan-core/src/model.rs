//! The plan model. Chief Architect's core idea is that the *plan is the model*:
//! walls, openings, floors and later cabinets/roofs/stairs live here once, and
//! the 2D plan, 3D view, elevations and schedules are all generated from it.

use crate::cad::{CadItem, CadObject};
use crate::camera::CameraObject;
use crate::defaults::WallTypeDef;
use crate::dimension::Dimension;
use crate::floors::FloorKind;
use crate::geometry::Point;
use crate::groups::ObjectGroup;
use crate::layer_sets::{LayerSets, SavedPlanView};
use crate::layers::LayerSet;
use crate::openings::{Casing, OpeningStyle};
use crate::rooms::Room;
use crate::symbols::PlacedSymbol;
use crate::text_styles::TextStyles;
use crate::walls::{ResizeAbout, Side, WallCurve, WallFlags, WallRoofDirective};
use serde::{Deserialize, Serialize};

pub type Id = u64;

/// Default exterior wall: 2x6 framing + sheathing + drywall ≈ 6 1/2".
pub const DEFAULT_EXTERIOR_THICKNESS: f64 = 6.5;
/// Default interior wall: 2x4 framing + 1/2" drywall both sides = 4 1/2".
pub const DEFAULT_INTERIOR_THICKNESS: f64 = 4.5;
/// Default ceiling height 9'-1 1/8" (matches a 9' wall with 1 1/8" subfloor stack).
pub const DEFAULT_CEILING_HEIGHT: f64 = 109.125;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallKind {
    Exterior,
    Interior,
}

/// Layer new walls are placed on.
pub const DEFAULT_WALL_LAYER: &str = "Walls, Normal";

fn default_wall_layer() -> String {
    DEFAULT_WALL_LAYER.to_string()
}

/// Which end of a wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallEnd {
    Start,
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wall {
    pub id: Id,
    /// Centerline start, inches.
    pub start: Point,
    /// Centerline end, inches.
    pub end: Point,
    #[serde(deserialize_with = "crate::foreign::thickness_or_default")]
    pub thickness: f64,
    #[serde(deserialize_with = "crate::foreign::height_or_default")]
    pub height: f64,
    pub kind: WallKind,
    /// Layer name; see [`LayerSet`]. Defaults to "Walls, Normal".
    #[serde(default = "default_wall_layer")]
    pub layer: String,
    /// Wall options and variants (W-24, W-52..W-58).
    #[serde(default)]
    pub flags: WallFlags,
    /// Name of an entry of `PlanDefaults::wall_types` / `Project::wall_types` (W-47).
    #[serde(default)]
    pub wall_type: Option<String>,
    /// Reference line a thickness change keeps fixed (W-26, W-27). Walls from
    /// old files load as `WallCenter`; [`Wall::new`] picks Chief's default per kind.
    #[serde(default)]
    pub resize_about: ResizeAbout,
    /// Curved wall (W-64..W-68); `None` for a straight wall.
    #[serde(default)]
    pub curve: Option<WallCurve>,
    /// Roof directive of this wall (RF-18..RF-25).
    #[serde(default)]
    pub roof: WallRoofDirective,
    /// Which side of start-to-end is the exterior (W-21); layers are laid out
    /// from that side. Defaults to the left (+normal) side.
    #[serde(default)]
    pub exterior_side: Side,
    /// Wall dialog values that persist with the wall.
    #[serde(default)]
    pub extras: crate::extras::WallExtras,
    /// Which flyout wall this is (foundation, pony, glass, half-wall, ...);
    /// `Standard` for ordinary walls and files from before the variants.
    #[serde(default)]
    pub class: crate::walls::WallClass,
    /// How far a [`crate::walls::WallClass::Foundation`] wall reaches below
    /// the floor it is drawn on, inches.
    #[serde(default = "default_foundation_height")]
    pub foundation_height: f64,
    /// Deck edge (rim board, no railing); set with the `DeckEdge` class.
    #[serde(default)]
    pub is_deck_edge: bool,
    /// Where the wall starts above the floor it is drawn on, inches (Chief's
    /// wall "Bottom" value). `height` is measured from here: the wall spans
    /// `bottom_offset..bottom_offset + height`. Zero for ordinary walls; dormer
    /// walls start at the roof surface.
    #[serde(default)]
    pub bottom_offset: f64,
    /// Structure, Foundation and Wall Cap tab values (W-39, W-52, W-60, W-62,
    /// R-69); see [`crate::walls::spec`].
    #[serde(default)]
    pub spec: crate::walls::WallSpec,
}

fn default_foundation_height() -> f64 {
    crate::walls::DEFAULT_FOUNDATION_HEIGHT
}

impl Wall {
    pub fn length(&self) -> f64 {
        self.start.dist(self.end)
    }
    /// Unit direction from start to end.
    pub fn direction(&self) -> Point {
        self.end.sub(self.start).normalized()
    }
    /// Unit normal (left side when walking start→end).
    pub fn normal(&self) -> Point {
        self.direction().perp()
    }
    pub fn point_at(&self, dist_from_start: f64) -> Point {
        self.start.add(self.direction().scale(dist_from_start))
    }
    /// Four corners of the wall footprint (centerline ± thickness/2).
    pub fn footprint(&self) -> [Point; 4] {
        let n = self.normal().scale(self.thickness * 0.5);
        [
            self.start.add(n),
            self.end.add(n),
            self.end.sub(n),
            self.start.sub(n),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpeningKind {
    Door,
    Window,
}

/// A door or window hosted in a wall. Position is measured along the wall
/// centerline from `start`, like Chief's "distance from wall end" fields.
///
/// Deserialization goes through [`crate::openings`] so files without the
/// newer fields load with sensible values (e.g. `Window` style for windows).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "crate::openings::OpeningDe")]
pub struct Opening {
    pub id: Id,
    pub wall_id: Id,
    /// Distance from the wall start to the opening center, inches.
    pub center_offset: f64,
    pub width: f64,
    pub height: f64,
    /// Bottom of the opening above the floor (0 for doors).
    pub sill_height: f64,
    pub kind: OpeningKind,
    /// Doors only: the leaf swings to the other side of the wall (DW-31, DW-32).
    pub swing_flipped: bool,
    /// Doors only: the hinge is on the wall-end jamb instead of the
    /// wall-start jamb (DW-31, DW-32).
    pub hinge_at_end: bool,
    /// Drawing style (DW-38..DW-58); defaults by kind.
    pub style: OpeningStyle,
    /// Replaces the automatic plan label (DW-62).
    pub label_override: Option<String>,
    /// Door/window schedule number (DW-60, DW-61).
    pub schedule_number: Option<String>,
    pub casing: Option<Casing>,
    /// Window lites `(across, vertical)`.
    pub lites: (u32, u32),
    pub egress: bool,
    pub tempered: bool,
    /// Dialog values that persist with the opening.
    pub extras: crate::extras::OpeningExtras,
    /// Windows with the same id form one mulled unit sharing a frame
    /// (DW-51); `None` for a window on its own.
    pub mull_group: Option<Id>,
}

impl Opening {
    pub fn default_door(id: Id, wall_id: Id, center_offset: f64) -> Self {
        Self {
            id,
            ..Opening::new(wall_id, center_offset, OpeningKind::Door, 36.0, 80.0, 0.0)
        }
    }
    pub fn default_window(id: Id, wall_id: Id, center_offset: f64) -> Self {
        Self {
            id,
            ..Opening::new(
                wall_id,
                center_offset,
                OpeningKind::Window,
                36.0,
                60.0,
                24.0,
            )
        }
    }
    pub fn start_offset(&self) -> f64 {
        self.center_offset - self.width * 0.5
    }
    pub fn end_offset(&self) -> f64 {
        self.center_offset + self.width * 0.5
    }
}

/// A room's user-assigned name. Rooms are derived from walls, so a name is
/// attached to a point inside the room instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomName {
    pub anchor: Point,
    pub name: String,
    pub room_type: String,
    /// Floor height offset on top of the floor datum, inches (R-23).
    #[serde(default)]
    pub floor_height_offset: f64,
    /// Ceiling height override from the room's own floor, inches (R-24).
    #[serde(default)]
    pub ceiling_height: Option<f64>,
    /// Floor finish name (R-27, R-36).
    #[serde(default)]
    pub floor_finish: Option<String>,
    /// Ceiling finish name (R-27, R-36).
    #[serde(default)]
    pub ceiling_finish: Option<String>,
    /// Include in living area; `None` follows the room type (R-42).
    #[serde(default)]
    pub include_in_living_area: Option<bool>,
    /// Build a ceiling over this room (R-30).
    #[serde(default = "default_true")]
    pub has_ceiling: bool,
    /// Build a floor under this room (R-30).
    #[serde(default = "default_true")]
    pub has_floor: bool,
    /// Rough (framed) ceiling height, inches (R-25).
    #[serde(default)]
    pub rough_ceiling: Option<f64>,
    /// Room is heated/cooled; `None` follows the room type.
    #[serde(default)]
    pub conditioned: Option<bool>,
    /// Stem wall height under the room, inches.
    #[serde(default)]
    pub stem_wall_height: Option<f64>,
    /// Plan fill of the room.
    #[serde(default)]
    pub fill_style: Option<crate::extras::RoomFill>,
    /// What the room's plan label shows.
    #[serde(default)]
    pub label: crate::extras::RoomLabelOptions,
    /// Moldings applied around the room.
    #[serde(default)]
    pub moldings: Vec<crate::extras::MoldingRef>,
    /// Roof-over, absolute heights, finish thicknesses and wall covering.
    #[serde(default)]
    pub misc: Option<crate::extras::RoomMisc>,
    /// Monolithic Slab Foundation flag of a first-floor room (R-31).
    #[serde(default)]
    pub monolithic_slab: Option<crate::rooms::RoomSlab>,
    /// Label text style and placement (R-46).
    #[serde(default)]
    pub label_style: crate::rooms::RoomLabelStyle,
    /// Deck Specification of a deck room: planking, framing and stairs to
    /// grade (CB-86); see [`crate::deck`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck: Option<crate::deck::DeckSpec>,
    /// Flat Ceiling Over This Room (Structure panel): on is a flat ceiling
    /// at the room's ceiling height; off is a cathedral ceiling that follows
    /// the underside of the roof above (R-146; tray ceilings need it on).
    #[serde(default = "default_true")]
    pub flat_ceiling: bool,
    /// Roof Group (General panel; R-114): rooms of a non-default group are
    /// roofed as a separate building, apart from the rest of the plan. 0 is
    /// the default group.
    #[serde(default, skip_serializing_if = "is_zero_group")]
    pub roof_group: u32,
    /// The Structure and Layer panel switches beyond the platforms (R-103,
    /// R-115, R-145); see [`crate::rooms::RoomOptions`].
    #[serde(default, skip_serializing_if = "crate::rooms::RoomOptions::is_default")]
    pub options: crate::rooms::RoomOptions,
}

fn is_zero_group(g: &u32) -> bool {
    *g == 0
}

fn default_true() -> bool {
    true
}

impl RoomName {
    pub fn new(anchor: Point, name: impl Into<String>, room_type: impl Into<String>) -> Self {
        Self {
            anchor,
            name: name.into(),
            room_type: room_type.into(),
            floor_height_offset: 0.0,
            ceiling_height: None,
            floor_finish: None,
            ceiling_finish: None,
            include_in_living_area: None,
            has_ceiling: true,
            has_floor: true,
            rough_ceiling: None,
            conditioned: None,
            stem_wall_height: None,
            fill_style: None,
            label: crate::extras::RoomLabelOptions::default(),
            moldings: Vec::new(),
            misc: None,
            monolithic_slab: None,
            label_style: crate::rooms::RoomLabelStyle::default(),
            deck: None,
            flat_ceiling: true,
            roof_group: 0,
            options: crate::rooms::RoomOptions::default(),
        }
    }
}

impl Default for RoomName {
    fn default() -> Self {
        RoomName::new(Point::ZERO, "", "")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Floor {
    pub name: String,
    /// Finished-floor elevation relative to the first floor, inches.
    #[serde(deserialize_with = "crate::foreign::finite_or_zero")]
    pub elevation: f64,
    #[serde(deserialize_with = "crate::foreign::height_or_default")]
    pub ceiling_height: f64,
    pub walls: Vec<Wall>,
    pub openings: Vec<Opening>,
    #[serde(default)]
    pub dimensions: Vec<Dimension>,
    #[serde(default)]
    pub cad: Vec<CadObject>,
    #[serde(default)]
    pub room_names: Vec<RoomName>,
    /// Foundation, normal or attic floor (R-55).
    #[serde(default)]
    pub kind: FloorKind,
    /// Placed library symbols (CB-55, CB-56).
    #[serde(default)]
    pub symbols: Vec<PlacedSymbol>,
    /// Opaque cabinet objects owned by `plan-cabinets`; see [`Floor::cabinets_as`].
    #[serde(default)]
    pub cabinets: Vec<serde_json::Value>,
    /// Opaque stair objects owned by `plan-stairs`; see [`Floor::stairs_as`].
    #[serde(default)]
    pub stairs: Vec<serde_json::Value>,
    /// Object groups (S-35..S-38).
    #[serde(default)]
    pub groups: Vec<ObjectGroup>,
    /// Opaque roof objects; see [`Floor::roofs_as`].
    #[serde(default)]
    pub roofs: Vec<serde_json::Value>,
    /// Opaque electrical data; see [`Floor::electrical_as`].
    #[serde(default)]
    pub electrical: Option<serde_json::Value>,
    /// Opaque framing objects; see [`Floor::framing_as`].
    #[serde(default)]
    pub framing: Vec<serde_json::Value>,
    /// Opaque slab / pad / pier data; see [`crate::foundation::FoundationLayer`].
    #[serde(default)]
    pub foundation: Option<serde_json::Value>,
    /// Opaque corner trim / moldings / material regions / decks / 3D solids;
    /// see [`crate::details::DetailsLayer`].
    #[serde(default)]
    pub details: Option<serde_json::Value>,
    /// Opaque schedules placed on this floor; see [`crate::schedules::ScheduleLayer`].
    #[serde(default)]
    pub schedules: Option<serde_json::Value>,
    /// Per-object CAD style extras (line, fill, arrow, rich text), one entry
    /// per styled CAD object; see [`crate::cad::CadAttrs`].
    #[serde(default)]
    pub cad_attrs: Vec<crate::cad::CadAttrs>,
    /// Name and placement points of the floor's CAD blocks (groups of CAD
    /// objects); see [`crate::cad::CadBlockInfo`].
    #[serde(default)]
    pub cad_blocks: Vec<crate::cad::CadBlockInfo>,
    /// Pictures placed under the plan for tracing; see [`crate::underlay`].
    #[serde(default)]
    pub underlays: Vec<crate::underlay::Underlay>,
    /// Set on the floors that are CAD details (CAD Detail Management, Auto
    /// Detail); see [`crate::details::CadDetailInfo`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<crate::details::CadDetailInfo>,
    /// Floor Defaults of this floor (R-56): platform and finish thicknesses,
    /// the room type and materials a new room starts with.
    #[serde(default)]
    pub settings: crate::floors::FloorSettings,
    /// Specification of each fireplace and chimney on this floor, keyed by
    /// the id of the placed symbol that stands for it (CB-87); see
    /// [`crate::fireplace`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fireplaces: Vec<crate::fireplace::Fireplace>,
    /// Objects with a drawing group of their own (Edit > Drawing Group); the
    /// rest draw in the group of their kind. See [`crate::drawing_group`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawing_groups: Vec<crate::drawing_group::GroupEntry>,
    /// Callouts, markers and notes: their records; the CAD objects they draw
    /// as are in [`Floor::cad`] (see [`crate::callout`]).
    #[serde(default, skip_serializing_if = "crate::callout::Annots::is_empty")]
    pub annots: crate::callout::Annots,
    /// Architectural blocks (see [`crate::arch_block`]).
    #[serde(default, skip_serializing_if = "crate::arch_block::BlockLayer::is_empty")]
    pub blocks: crate::arch_block::BlockLayer,
    /// Extra 3D solid spec fields and compound solids (see [`crate::solids`]).
    #[serde(default, skip_serializing_if = "crate::solids::SolidLayer::is_empty")]
    pub solid_layer: crate::solids::SolidLayer,
    /// Layer tables of material regions (see [`crate::material_region`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub region_layers: Vec<crate::material_region::RegionStructure>,
    /// Construction line records, keyed by the id of their CAD line (see
    /// [`crate::construction`]).
    #[serde(default, skip_serializing_if = "crate::construction::ConstructionLayer::is_empty")]
    pub construction: crate::construction::ConstructionLayer,
    /// Center Sheet (File > Print): where the middle of this floor's Drawing
    /// Sheet sits on the plan. `None` centers the sheet on the walls. Moves
    /// the sheet only, never an object (manual p. 1432).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet_center: Option<Point>,
    /// Tray and coffered ceilings, keyed by the id of their CAD polyline (see
    /// [`crate::tray`]).
    #[serde(default, skip_serializing_if = "crate::tray::TrayLayer::is_empty")]
    pub trays: crate::tray::TrayLayer,
    /// Exterior Room Specifications, one per structure with a non-blank
    /// specification (see [`crate::living`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exterior_rooms: Vec<crate::living::ExteriorRoom>,
}

impl Floor {
    pub fn new(name: impl Into<String>, elevation: f64) -> Self {
        Self {
            name: name.into(),
            elevation,
            ceiling_height: DEFAULT_CEILING_HEIGHT,
            walls: Vec::new(),
            openings: Vec::new(),
            dimensions: Vec::new(),
            cad: Vec::new(),
            room_names: Vec::new(),
            kind: FloorKind::default(),
            symbols: Vec::new(),
            cabinets: Vec::new(),
            stairs: Vec::new(),
            groups: Vec::new(),
            roofs: Vec::new(),
            electrical: None,
            framing: Vec::new(),
            foundation: None,
            details: None,
            schedules: None,
            cad_attrs: Vec::new(),
            cad_blocks: Vec::new(),
            underlays: Vec::new(),
            detail: None,
            settings: crate::floors::FloorSettings::default(),
            fireplaces: Vec::new(),
            drawing_groups: Vec::new(),
            annots: crate::callout::Annots::default(),
            blocks: crate::arch_block::BlockLayer::default(),
            solid_layer: crate::solids::SolidLayer::default(),
            region_layers: Vec::new(),
            construction: crate::construction::ConstructionLayer::default(),
            sheet_center: None,
            trays: crate::tray::TrayLayer::default(),
            exterior_rooms: Vec::new(),
        }
    }
    pub fn wall(&self, id: Id) -> Option<&Wall> {
        self.walls.iter().find(|w| w.id == id)
    }
    pub fn wall_mut(&mut self, id: Id) -> Option<&mut Wall> {
        self.walls.iter_mut().find(|w| w.id == id)
    }
    pub fn openings_on(&self, wall_id: Id) -> impl Iterator<Item = &Opening> {
        self.openings.iter().filter(move |o| o.wall_id == wall_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(default = "default_project_name")]
    pub name: String,
    #[serde(default = "default_floors")]
    pub floors: Vec<Floor>,
    #[serde(default = "default_next_id")]
    next_id: Id,
    /// Keys of the loaded file that this build has no field for; written back
    /// at the same place on save (QA-20). See [`crate::foreign`].
    #[serde(skip)]
    foreign: crate::foreign::Foreign,
    #[serde(default = "LayerSet::default_floor_plan")]
    pub layers: LayerSet,
    /// Camera objects shown in the plan (C-4..C-30).
    #[serde(default)]
    pub cameras: Vec<CameraObject>,
    /// Wall type definitions stored in the plan (W-47).
    #[serde(default)]
    pub wall_types: Vec<WallTypeDef>,
    /// Named layer sets overlaid on `layers` (display/lock/colour/weight).
    #[serde(default)]
    pub layer_sets: LayerSets,
    /// Saved plan views, each carrying a layer set.
    #[serde(default = "SavedPlanView::defaults")]
    pub plan_views: Vec<SavedPlanView>,
    /// Name of the active entry of `plan_views`.
    #[serde(default = "crate::layer_sets::default_active_plan_view")]
    pub active_plan_view: String,
    /// Text styles that `Layer::text_style` names resolve through.
    #[serde(default)]
    pub text_styles: TextStyles,
    /// Opaque terrain data; see [`Project::terrain_as`].
    #[serde(default)]
    pub terrain: Option<serde_json::Value>,
    /// Tools > Project Information (client, designer, job number, revisions).
    #[serde(default)]
    pub info: crate::schedules::ProjectInfo,
    /// Point lights of the plan (C-64), all floors; see [`crate::camera`].
    #[serde(default)]
    pub lights: Vec<crate::camera::PlanLight>,
    /// Plan-wide light options.
    #[serde(default)]
    pub light_options: crate::camera::LightSettings,
    /// The sun and interior lights of the 3D views (3D > Lighting).
    #[serde(default)]
    pub lighting: crate::camera_view::Lighting,
    /// The project's layout (Chief's layout file kept in the plan), as the
    /// JSON of a `plan_layout::Layout`. plan-core must not depend on
    /// plan-layout, so the app reads and writes it (`shell::layout_window`).
    #[serde(default)]
    pub layout: Option<serde_json::Value>,
    /// The plan's other layout files, parked while `layout` is the one that
    /// is open (the JSON of a `plan_layout::Layout` each; Send to Layout
    /// can pick the file a view goes to, see `shell::layout_window`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layout_files: Vec<serde_json::Value>,
    /// The plan's user text macros (Text Macro Management).
    #[serde(default)]
    pub text_macros: crate::text_styles::TextMacros,
    /// Text objects whose macros are evaluated again as the plan changes
    /// (see [`crate::macros`]).
    #[serde(default, skip_serializing_if = "crate::macros::MacroTexts::is_empty")]
    pub macro_texts: crate::macros::MacroTexts,
    /// The plan's note types (Note Type Management).
    #[serde(default)]
    pub note_types: crate::text_styles::NoteTypes,
    /// Saved Defaults of callouts, markers and notes (Default Settings > Text,
    /// Callouts and Markers); see [`crate::callout::AnnotDefaults`].
    #[serde(default)]
    pub annot_defaults: crate::callout::AnnotDefaults,
    /// The lists of Multiple Saved Defaults, the Default Sets and the Use
    /// Default state of objects; see [`crate::defaults::saved`].
    #[serde(default, skip_serializing_if = "crate::defaults::saved::SavedDefaults::is_empty")]
    pub saved_defaults: crate::defaults::saved::SavedDefaults,
    /// Material overrides of single objects (Material Painter, Adjust
    /// Materials); see [`crate::object_materials`].
    #[serde(default)]
    pub object_materials: Vec<crate::object_materials::ObjectMaterial>,
    /// Materials Defaults (Default Settings > Materials): the material each
    /// object class (Wall, Door, Window, Room, Cabinet, Roof...) gives its
    /// parts unless the object was painted; see [`crate::object_materials`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material_defaults: Vec<crate::object_materials::ClassMaterial>,
    /// How the 3D view shows doors and windows: casing on or off, doors
    /// open or closed (`SceneOptions::for_project`).
    #[serde(default)]
    pub opening_display: crate::openings::OpeningView3d,
    /// Electrical defaults of the plan (default device heights); opaque so
    /// plan-core needs no electrical types, see `plan_electrical::ElectricalDefaults`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub electrical_defaults: Option<serde_json::Value>,
    /// User-defined properties and their values (Tools > Property Manager);
    /// see [`crate::props`].
    #[serde(default, skip_serializing_if = "crate::props::PropTable::is_empty")]
    pub props: crate::props::PropTable,
    /// The drawing group of each kind of object (Default Settings >
    /// Drawing Groups); see [`crate::drawing_group`].
    #[serde(
        default,
        skip_serializing_if = "crate::drawing_group::DrawingGroupTable::is_default"
    )]
    pub drawing_group_defaults: crate::drawing_group::DrawingGroupTable,
    /// Materials List data: object information and component changes, the
    /// saved lists and the Materials List Polylines; see
    /// [`crate::materials_data`].
    #[serde(default, skip_serializing_if = "crate::materials_data::MaterialsData::is_empty")]
    pub materials: crate::materials_data::MaterialsData,
    /// Drawing Sheet Setup of each kind of view and the Watermark (File >
    /// Print, View > Watermark); see [`crate::drawing_sheet`].
    #[serde(default, skip_serializing_if = "crate::drawing_sheet::PrintSetup::is_default")]
    pub print_setup: crate::drawing_sheet::PrintSetup,
    /// Construction Line Defaults and the order rule sets (Construction
    /// Line Order Management); see [`crate::construction`].
    #[serde(
        default,
        skip_serializing_if = "crate::construction::ConstructionSettings::is_default"
    )]
    pub construction: crate::construction::ConstructionSettings,
    /// The Change Floor/Reference table: the reference rows in draw order;
    /// see [`crate::construction::ReferenceTable`].
    #[serde(
        default,
        skip_serializing_if = "crate::construction::ReferenceTable::is_default"
    )]
    pub reference_table: crate::construction::ReferenceTable,
    /// Library line styles, fill styles, custom patterns, the User Catalog
    /// entries and the Poché switch; see [`crate::fill_styles`].
    #[serde(default, skip_serializing_if = "crate::fill_styles::StyleBook::is_default")]
    pub styles: crate::fill_styles::StyleBook,
    /// Schedule Defaults per kind of schedule and the custom schedule
    /// categories; see [`crate::schedules::ScheduleSetup`].
    #[serde(default, skip_serializing_if = "crate::schedules::ScheduleSetup::is_default")]
    pub schedule_setup: crate::schedules::ScheduleSetup,
    /// Layered floor, ceiling and roof definitions: the plan-wide Floor/Ceiling
    /// Platform Defaults, the Backsplash and the saved named definitions; see
    /// [`crate::assemblies`].
    #[serde(
        default,
        skip_serializing_if = "crate::assemblies::AssemblyLibrary::is_empty"
    )]
    pub assemblies: crate::assemblies::AssemblyLibrary,
}

fn default_project_name() -> String {
    "Untitled Plan".to_string()
}

fn default_floors() -> Vec<Floor> {
    vec![Floor::new("1st Floor", 0.0)]
}

fn default_next_id() -> Id {
    1
}

/// Minimum clear distance between an opening jamb and a wall end or another opening.
const OPENING_MARGIN: f64 = 2.0;

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            floors: default_floors(),
            next_id: 1,
            foreign: crate::foreign::Foreign::default(),
            layers: LayerSet::default_floor_plan(),
            cameras: Vec::new(),
            wall_types: Vec::new(),
            layer_sets: LayerSets::default(),
            plan_views: SavedPlanView::defaults(),
            active_plan_view: crate::layer_sets::default_active_plan_view(),
            text_styles: TextStyles::default(),
            terrain: None,
            info: crate::schedules::ProjectInfo::default(),
            lights: Vec::new(),
            light_options: crate::camera::LightSettings::default(),
            lighting: crate::camera_view::Lighting::default(),
            layout: None,
            layout_files: Vec::new(),
            text_macros: crate::text_styles::TextMacros::default(),
            macro_texts: crate::macros::MacroTexts::default(),
            note_types: crate::text_styles::NoteTypes::default(),
            annot_defaults: crate::callout::AnnotDefaults::default(),
            saved_defaults: Default::default(),
            object_materials: Vec::new(),
            material_defaults: Vec::new(),
            opening_display: crate::openings::OpeningView3d::default(),
            electrical_defaults: None,
            props: crate::props::PropTable::default(),
            drawing_group_defaults: crate::drawing_group::DrawingGroupTable::default(),
            materials: crate::materials_data::MaterialsData::default(),
            print_setup: crate::drawing_sheet::PrintSetup::default(),
            construction: crate::construction::ConstructionSettings::default(),
            reference_table: crate::construction::ReferenceTable::default(),
            styles: crate::fill_styles::StyleBook::default(),
            schedule_setup: crate::schedules::ScheduleSetup::default(),
            assemblies: crate::assemblies::AssemblyLibrary::default(),
        }
    }

    pub fn alloc_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// The next id [`Project::alloc_id`] will hand out (undo gives it back
    /// with the rest of the plan).
    pub fn next_id(&self) -> Id {
        self.next_id
    }

    pub fn add_wall(
        &mut self,
        floor: usize,
        start: Point,
        end: Point,
        thickness: f64,
        height: f64,
        kind: WallKind,
    ) -> Id {
        let id = self.alloc_id();
        self.floors[floor].walls.push(Wall {
            id,
            ..Wall::new(start, end, thickness, height, kind)
        });
        id
    }

    /// Move one end of a wall. Openings keep their distance from the wall
    /// start, so callers may want to re-validate them afterwards.
    /// Returns `false` if the wall does not exist.
    pub fn move_wall_endpoint(
        &mut self,
        floor: usize,
        id: Id,
        which_end: WallEnd,
        new_pos: Point,
    ) -> bool {
        match self.floors[floor].wall_mut(id) {
            Some(w) => {
                match which_end {
                    WallEnd::Start => w.start = new_pos,
                    WallEnd::End => w.end = new_pos,
                }
                true
            }
            None => false,
        }
    }

    /// Move a whole wall by `delta`. Returns `false` if the wall does not exist.
    pub fn translate_wall(&mut self, floor: usize, id: Id, delta: Point) -> bool {
        match self.floors[floor].wall_mut(id) {
            Some(w) => {
                w.start = w.start.add(delta);
                w.end = w.end.add(delta);
                true
            }
            None => false,
        }
    }

    /// Add a dimension (its `id` is replaced with a fresh one) and return the id.
    pub fn add_dimension(&mut self, floor: usize, mut dim: Dimension) -> Id {
        let id = self.alloc_id();
        dim.id = id;
        self.floors[floor].dimensions.push(dim);
        id
    }

    pub fn remove_dimension(&mut self, floor: usize, id: Id) {
        self.floors[floor].dimensions.retain(|d| d.id != id);
    }

    /// Add a CAD item on `layer` and return its id.
    pub fn add_cad(&mut self, floor: usize, layer: impl Into<String>, item: CadItem) -> Id {
        let id = self.alloc_id();
        self.floors[floor].cad.push(CadObject {
            id,
            layer: layer.into(),
            item,
        });
        id
    }

    pub fn remove_cad(&mut self, floor: usize, id: Id) {
        self.floors[floor].cad.retain(|c| c.id != id);
    }

    /// Name the room containing `anchor`. Any existing name whose anchor lies
    /// in the same detected room (from `rooms`) is replaced. If `anchor` is
    /// not inside any room, names anchored within 1" of it are replaced.
    pub fn set_room_name(
        &mut self,
        floor: usize,
        anchor: Point,
        name: impl Into<String>,
        room_type: impl Into<String>,
        rooms: &[Room],
    ) {
        let names = &mut self.floors[floor].room_names;
        let in_replaced = |n: &RoomName| match rooms.iter().find(|r| r.contains(anchor)) {
            Some(room) => room.contains(n.anchor),
            None => n.anchor.dist(anchor) <= 1.0,
        };
        // Renaming keeps the room's other properties (R-14, R-21).
        let previous = names.iter().find(|n| in_replaced(n)).cloned();
        names.retain(|n| !in_replaced(n));
        let mut entry = previous.unwrap_or_default();
        entry.anchor = anchor;
        entry.name = name.into();
        entry.room_type = room_type.into();
        names.push(entry);
    }

    /// Remove a wall and every opening hosted in it.
    pub fn remove_wall(&mut self, floor: usize, id: Id) {
        let f = &mut self.floors[floor];
        f.walls.retain(|w| w.id != id);
        f.openings.retain(|o| o.wall_id != id);
    }

    pub fn remove_opening(&mut self, floor: usize, id: Id) {
        self.floors[floor].openings.retain(|o| o.id != id);
    }

    /// Place an opening on `wall_id` centered as close to `center_offset` as
    /// fits. Returns `None` if the wall is too short or the spot overlaps
    /// another opening.
    pub fn add_opening(
        &mut self,
        floor: usize,
        wall_id: Id,
        center_offset: f64,
        kind: OpeningKind,
    ) -> Option<Id> {
        let id = self.alloc_id();
        let mut opening = match kind {
            OpeningKind::Door => Opening::default_door(id, wall_id, center_offset),
            OpeningKind::Window => Opening::default_window(id, wall_id, center_offset),
        };
        let f = &mut self.floors[floor];
        let wall_len = f.wall(wall_id)?.path_length();
        let half = opening.width * 0.5;
        if wall_len < opening.width + 2.0 * OPENING_MARGIN {
            return None;
        }
        opening.center_offset = opening
            .center_offset
            .clamp(half + OPENING_MARGIN, wall_len - half - OPENING_MARGIN);
        let overlaps = f
            .openings_on(wall_id)
            .any(|o| crate::openings::openings_conflict(&opening, o, OPENING_MARGIN));
        if overlaps {
            return None;
        }
        f.openings.push(opening);
        Some(id)
    }

    /// The plan as pretty JSON. Keys a newer build wrote are written back
    /// (QA-20); a NaN or infinity is written as 0 because `null` would make
    /// the file unreadable (QA-23, see [`Project::sanitize`]).
    pub fn to_json(&self) -> serde_json::Result<String> {
        if self.foreign.is_empty() {
            return crate::foreign::to_pretty_finite(self);
        }
        let (mut v, _) = crate::foreign::to_value_finite(self)?;
        self.foreign.apply(&mut v);
        serde_json::to_string_pretty(&v)
    }

    /// Reads a plan. A file without `name`, `floors` or `next_id` (even `{}`)
    /// loads with the defaults (QA-21), `next_id` is raised above every id in
    /// the file (QA-22) and the keys this build has no field for are kept for
    /// the next save (QA-20).
    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        // serde reads a struct from a JSON array too (`[]` would become an
        // empty plan); a plan file is an object.
        if !s.trim_start().starts_with('{') {
            return Err(<serde_json::Error as serde::de::Error>::custom(
                "a plan file is a JSON object",
            ));
        }
        let mut p: Project = serde_json::from_str(s)?;
        // A file that omits `floors` gets the default floor (serde default);
        // one that says `"floors": []` is damaged, not an empty plan.
        if p.floors.is_empty() {
            return Err(<serde_json::Error as serde::de::Error>::custom(
                "a plan needs at least one floor",
            ));
        }
        // Layered platform definitions own the thickness fields the rest of
        // the program reads; bring them in step (a plan with none is
        // untouched).
        p.sync_platform_mirrors();
        if let (Ok(loaded), Ok((typed, _))) = (
            serde_json::from_str::<serde_json::Value>(s),
            crate::foreign::to_value_finite(&p),
        ) {
            p.foreign = crate::foreign::Foreign::capture(&loaded, &typed);
            let used = crate::foreign::max_id(&loaded).max(crate::foreign::max_id(&typed));
            p.next_id = p.next_id.max(used + 1);
        }
        Ok(p)
    }

    /// After an undo: `self` is the restored plan and `undone` the plan it
    /// replaces. Ids a tool took just before it recorded the undo step belong
    /// to objects only `undone` holds; they are given back, so undo restores
    /// `next_id` too (QA-27). Never lowers `next_id` to an id still in use.
    pub fn give_back_ids(&mut self, undone: &Project) {
        let (Ok((mine, _)), Ok((theirs, _))) = (
            crate::foreign::to_value_finite(self),
            crate::foreign::to_value_finite(undone),
        ) else {
            return;
        };
        let mine_ids = crate::foreign::ids(&mine);
        let low = crate::foreign::ids(&theirs)
            .into_iter()
            .filter(|i| !mine_ids.contains(i) && *i < self.next_id)
            .min();
        if let Some(low) = low {
            let floor = crate::foreign::max_id(&mine) + 1;
            self.next_id = low.max(floor).min(self.next_id);
        }
    }

    /// Number of foreign keys (from a newer build) kept for the next save.
    pub fn foreign_key_count(&self) -> usize {
        self.foreign.len()
    }

    /// Problems with the plan's ids: a `next_id` that is not above every id,
    /// and ids used twice within one list. Empty when the ids are sound.
    pub fn validate_ids(&self) -> Vec<String> {
        let mut out = Vec::new();
        let used = self.highest_id();
        if self.next_id <= used {
            out.push(format!(
                "next_id {} is not above the highest id in use ({used})",
                self.next_id
            ));
        }
        fn dupes(out: &mut Vec<String>, what: &str, floor: &str, ids: impl Iterator<Item = Id>) {
            let mut seen = std::collections::BTreeSet::new();
            for id in ids {
                if !seen.insert(id) {
                    out.push(format!("{what} id {id} is used twice on {floor}"));
                }
            }
        }
        for f in &self.floors {
            dupes(&mut out, "wall", &f.name, f.walls.iter().map(|w| w.id));
            dupes(&mut out, "opening", &f.name, f.openings.iter().map(|o| o.id));
            dupes(&mut out, "dimension", &f.name, f.dimensions.iter().map(|d| d.id));
            dupes(&mut out, "CAD object", &f.name, f.cad.iter().map(|c| c.id));
        }
        out
    }

    /// The highest `id` in any slot of the plan (typed or opaque).
    pub fn highest_id(&self) -> Id {
        crate::foreign::to_value_finite(self)
            .map(|(v, _)| crate::foreign::max_id(&v))
            .unwrap_or(0)
    }

    /// Raises `next_id` above every id in the plan. Returns true when it had
    /// to (a stale counter would hand out ids already in use, QA-22).
    pub fn repair_ids(&mut self) -> bool {
        if self.floors.is_empty() {
            self.floors = default_floors();
        }
        let want = self.highest_id() + 1;
        if self.next_id < want {
            self.next_id = want;
            return true;
        }
        false
    }

    /// How many numbers in the plan are NaN or infinite.
    pub fn non_finite_count(&self) -> usize {
        crate::foreign::count_non_finite(self)
    }

    /// Replaces every NaN or infinity in the plan with 0 and returns how many
    /// there were (QA-23). A save writes them as 0 anyway; this makes the
    /// plan in memory match what is written. Callers say so in the status bar.
    pub fn sanitize(&mut self) -> usize {
        let Ok((v, n)) = crate::foreign::to_value_finite(self) else {
            return 0;
        };
        if n == 0 {
            return 0;
        }
        if let Ok(clean) = serde_json::from_value::<Project>(v) {
            let (foreign, next_id) = (self.foreign.clone(), self.next_id);
            *self = clean;
            self.foreign = foreign;
            self.next_id = self.next_id.max(next_id);
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openings_clamp_and_reject_overlap() {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            DEFAULT_INTERIOR_THICKNESS,
            DEFAULT_CEILING_HEIGHT,
            WallKind::Interior,
        );
        // Asked for center at 5", clamped to 18 + 2 = 20".
        let d = p.add_opening(0, w, 5.0, OpeningKind::Door).unwrap();
        let o = p.floors[0].openings.iter().find(|o| o.id == d).unwrap();
        assert!((o.center_offset - 20.0).abs() < 1e-9);
        // Overlapping second door is rejected.
        assert!(p.add_opening(0, w, 30.0, OpeningKind::Door).is_none());
        // Far enough along fits.
        assert!(p.add_opening(0, w, 90.0, OpeningKind::Door).is_some());
        // Too short a wall rejects.
        let short = p.add_wall(
            0,
            Point::new(0.0, 50.0),
            Point::new(30.0, 50.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        assert!(p.add_opening(0, short, 15.0, OpeningKind::Door).is_none());
    }

    #[test]
    fn json_round_trip() {
        let mut p = Project::new("rt");
        p.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            6.5,
            109.125,
            WallKind::Exterior,
        );
        let s = p.to_json().unwrap();
        let q = Project::from_json(&s).unwrap();
        assert_eq!(q.floors[0].walls.len(), 1);
        assert_eq!(q.name, "rt");
    }

    #[test]
    fn old_json_without_new_fields_still_loads() {
        let old = r#"{
            "name": "legacy",
            "floors": [{
                "name": "1st Floor",
                "elevation": 0.0,
                "ceiling_height": 109.125,
                "walls": [{
                    "id": 1,
                    "start": {"x": 0.0, "y": 0.0},
                    "end": {"x": 100.0, "y": 0.0},
                    "thickness": 6.5,
                    "height": 109.125,
                    "kind": "Exterior"
                }],
                "openings": []
            }],
            "next_id": 2
        }"#;
        let p = Project::from_json(old).unwrap();
        assert_eq!(p.floors[0].walls[0].layer, "Walls, Normal");
        assert!(p.floors[0].dimensions.is_empty());
        assert!(p.floors[0].cad.is_empty());
        assert!(p.floors[0].room_names.is_empty());
        assert!(p.layers.get("Doors").is_some());
        // And the new format round-trips.
        let q = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(q.layers, p.layers);
    }

    #[test]
    fn edit_helpers() {
        use crate::cad::CadItem;
        use crate::dimension::{Dimension, DimensionKind};
        let mut p = Project::new("e");
        let w = p.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        assert!(p.move_wall_endpoint(0, w, WallEnd::End, Point::new(150.0, 0.0)));
        assert!((p.floors[0].walls[0].length() - 150.0).abs() < 1e-9);
        assert!(p.translate_wall(0, w, Point::new(10.0, 20.0)));
        assert_eq!(p.floors[0].walls[0].start, Point::new(10.0, 20.0));
        assert!(!p.translate_wall(0, 999, Point::ZERO));

        let d = p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::ZERO,
                Point::new(10.0, 0.0),
                6.0,
            ),
        );
        assert_eq!(p.floors[0].dimensions[0].id, d);
        p.remove_dimension(0, d);
        assert!(p.floors[0].dimensions.is_empty());

        let c = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(5.0, 5.0),
            },
        );
        assert_eq!(p.floors[0].cad[0].id, c);
        p.remove_cad(0, c);
        assert!(p.floors[0].cad.is_empty());
    }

    #[test]
    fn room_names_replace_within_same_room() {
        let room = Room {
            polygon: vec![
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(100.0, 100.0),
                Point::new(0.0, 100.0),
            ],
            area_sq_in: 10_000.0,
            centroid: Point::new(50.0, 50.0),
            label: "Room 1".into(),
            ..Room::default()
        };
        let rooms = [room];
        let mut p = Project::new("r");
        p.set_room_name(0, Point::new(10.0, 10.0), "Kitchen", "Kitchen", &rooms);
        p.set_room_name(0, Point::new(90.0, 90.0), "Pantry", "Pantry", &rooms);
        assert_eq!(p.floors[0].room_names.len(), 1);
        assert_eq!(p.floors[0].room_names[0].name, "Pantry");
        // A point outside every room adds a separate name.
        p.set_room_name(0, Point::new(500.0, 500.0), "Yard", "Other", &rooms);
        assert_eq!(p.floors[0].room_names.len(), 2);
    }
}
