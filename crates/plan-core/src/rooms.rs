//! Automatic room detection, the heart of Chief's "draw walls, get rooms".
//!
//! Walls are reduced to their centerlines, split at every intersection and
//! T-junction, snapped into a planar graph, and each bounded face of that
//! graph becomes a room. Faces are traced with the standard half-edge walk:
//! leaving a vertex, take the first edge clockwise from the one we arrived on,
//! which yields counter-clockwise (positive area) loops for interior faces and
//! one clockwise loop for the unbounded outside, which is discarded.

use crate::defaults::RoomTypeDef;
use crate::extras::MoldingKind;
use crate::geometry::{
    dist_to_segment, point_in_polygon, polygon_area, polygon_centroid, project_on_segment,
    segment_intersection, BoxGrid, Point,
};
use crate::model::{Floor, Project, RoomName, Wall, WallKind};
use crate::units::sq_in_to_sq_ft;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A detected room (R-1..R-18).
///
/// Three areas are carried:
/// * `area_sq_in`: area of the **centerline** polygon (`polygon`); kept for
///   compatibility with older callers.
/// * `interior_area_sq_in`: area of the **interior-surface** polygon
///   (`inner_polygon`), Chief's Interior Area (R-2, R-49).
/// * `standard_area_sq_in`: Chief's Standard Area (R-49): to the outside of
///   exterior walls and the centre of interior (shared) walls.
#[derive(Debug, Clone, Default)]
pub struct Room {
    /// Counter-clockwise centerline polygon, inches.
    pub polygon: Vec<Point>,
    /// Centerline area, square inches (see the type docs).
    pub area_sq_in: f64,
    pub centroid: Point,
    pub label: String,
    /// Counter-clockwise polygon along the interior wall surfaces (R-2).
    pub inner_polygon: Vec<Point>,
    /// Area of `inner_polygon`, square inches.
    pub interior_area_sq_in: f64,
    /// Standard Area, square inches (R-49).
    pub standard_area_sq_in: f64,
    /// Nested rooms (R-11): the centerline polygons of free-standing loops
    /// wholly inside this room (a closet pod, a chimney box). The areas above
    /// already exclude them and the floor and ceiling platforms have a hole
    /// under each.
    pub holes: Vec<Vec<Point>>,
    /// Area inside the outer surfaces of this room's walls, square inches:
    /// what this room takes out of an enclosing room's interior area when it
    /// is nested in one.
    pub outer_area_sq_in: f64,
}

impl Room {
    /// Centerline area in square feet.
    pub fn area_sq_ft(&self) -> f64 {
        sq_in_to_sq_ft(self.area_sq_in)
    }
    /// The centerline polygon (same as the `polygon` field).
    pub fn centerline_polygon(&self) -> &[Point] {
        &self.polygon
    }
    /// Interior Area in square feet (R-49).
    pub fn interior_area_sq_ft(&self) -> f64 {
        sq_in_to_sq_ft(self.interior_area_sq_in)
    }
    /// Alias of [`Room::interior_area_sq_ft`].
    pub fn interior_area(&self) -> f64 {
        self.interior_area_sq_ft()
    }
    /// Standard Area in square feet (R-49).
    pub fn standard_area_sq_ft(&self) -> f64 {
        sq_in_to_sq_ft(self.standard_area_sq_in)
    }
    /// Is `p` in the room proper: inside its polygon and not inside one of
    /// its nested rooms (R-11)?
    pub fn contains(&self, p: Point) -> bool {
        point_in_polygon(p, &self.polygon) && !self.holes.iter().any(|h| point_in_polygon(p, h))
    }
    /// The first name entry anchored in this room proper (an anchor inside a
    /// nested room belongs to that room, not to this one).
    pub fn name_entry<'a>(&self, names: &'a [RoomName]) -> Option<&'a RoomName> {
        names.iter().find(|n| self.contains(n.anchor))
    }
}

/// How far a Garage floor sits below the house floor by default, inches
/// (R-40, R-26).
pub const GARAGE_FLOOR_DROP: f64 = 24.0;
/// Thickness of the concrete slab under a garage or porch, inches.
pub const SLAB_FLOOR_THICKNESS: f64 = 4.0;

/// The three broad categories of room functions (manual p. 446).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum FunctionClass {
    /// Most functions: living and conditioned, a flat ceiling and a roof.
    #[default]
    Interior,
    /// Balcony, Court and Deck: open to the outside.
    Exterior,
    /// Attic, Garage, Open Below, Porch and Slab.
    Hybrid,
}

impl FunctionClass {
    pub fn name(self) -> &'static str {
        match self {
            FunctionClass::Interior => "Interior",
            FunctionClass::Exterior => "Exterior",
            FunctionClass::Hybrid => "Hybrid",
        }
    }
}

/// The room functions a Room Type can name, with their category (manual
/// p. 446). A function is a fixed set of properties; only the Room Type's
/// own settings (name, living and conditioned inclusion, platforms) edit.
/// DECISIONS 300 (corrected): Basement and Crawl Space are not functions, a
/// crawl space or stairwell uses Open Below; Balcony and Court exist.
pub const ROOM_FUNCTIONS: [(&str, FunctionClass); 10] = [
    ("Standard", FunctionClass::Interior),
    ("Utility", FunctionClass::Interior),
    ("Balcony", FunctionClass::Exterior),
    ("Court", FunctionClass::Exterior),
    ("Deck", FunctionClass::Exterior),
    ("Attic", FunctionClass::Hybrid),
    ("Garage", FunctionClass::Hybrid),
    ("Open Below", FunctionClass::Hybrid),
    ("Porch", FunctionClass::Hybrid),
    ("Slab", FunctionClass::Hybrid),
];

/// The names of [`ROOM_FUNCTIONS`] in order.
pub fn function_names() -> Vec<&'static str> {
    ROOM_FUNCTIONS.iter().map(|(n, _)| *n).collect()
}

/// The function a room type of the older lists means: "Living" was a plain
/// interior function, "Basement" an interior room on the foundation floor and
/// "Crawl Space" an Open Below room (DECISIONS 300 as corrected); a Flat Roof
/// keeps its own platform.
pub fn canonical_function(function: &str) -> &str {
    match function {
        "Living" | "Basement" => "Standard",
        "Crawl Space" => "Open Below",
        other => other,
    }
}

/// The category of `function` for a room type named `type_name`: the type
/// names Attic, Courtyard and Crawl Space behave as their functions whatever
/// function the type names (an older list names Utility for them).
pub fn function_class(function: &str, type_name: &str) -> FunctionClass {
    let f = effective_function(function, type_name);
    ROOM_FUNCTIONS
        .iter()
        .find(|(n, _)| *n == f)
        .map_or(FunctionClass::Interior, |(_, c)| *c)
}

/// The function a room behaves as: its type's function, except that the
/// older type names Attic, Courtyard and Crawl Space imply theirs.
pub fn effective_function<'a>(function: &'a str, type_name: &str) -> &'a str {
    match type_name {
        "Attic" => "Attic",
        "Courtyard" => "Court",
        "Crawl Space" => "Open Below",
        _ => canonical_function(function),
    }
}

/// How the Auto Place Outlets tool treats a room (manual p. 447).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutletPlacement {
    /// Outlets all around, GFCI over base cabinets of kitchens and baths.
    #[default]
    Full,
    /// Fewer outlets (hybrid rooms such as a garage or slab).
    Fewer,
    /// None (exterior rooms, porches, Open Below rooms).
    None,
}

/// Electrical behaviour of a room function and type (manual p. 447).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ElectricalRules {
    /// A light, switch or outlet on the wall of the room is a weatherproof
    /// or outdoor type, as the Electrical Defaults say.
    pub weatherproof: bool,
    pub outlets: OutletPlacement,
    /// Auto Place Outlets puts GFCI outlets over the base cabinets (kitchens
    /// and baths).
    pub gfci_over_base_cabinets: bool,
    /// Standard height outlets as well (kitchens; baths have none).
    pub standard_height_outlets: bool,
}

/// What Plan Check does with a room of a function (manual p. 447): the
/// habitable-room rules (smoke alarm in a bedroom, minimum areas, egress)
/// apply to interior rooms only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlanCheckRules {
    pub habitable_rules: bool,
}

/// What a room function (or a few named room types) sets on a room's
/// platforms and behaviour (R-40, R-41, R-101): the defaults of the
/// Structure switches and the floor height offset, which stay editable per
/// room.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDefaults {
    /// Interior, Exterior or Hybrid.
    pub class: FunctionClass,
    /// A floor platform under the room (off for Open Below, Attic, Courtyard).
    pub has_floor: bool,
    /// A ceiling platform over the room (off for Deck, Balcony, Court, Attic
    /// and Courtyard).
    pub has_ceiling: bool,
    /// The ceiling is flat (an exterior or Attic room has none to follow).
    pub flat_ceiling: bool,
    /// A roof is built over the room (off for exterior rooms and Attics).
    pub roof_over: bool,
    /// Included in the Living Area unless the room says otherwise.
    pub living_area: bool,
    /// Included in the Conditioned Area unless the room says otherwise.
    pub conditioned: bool,
    /// Build Foundation puts a foundation under the room (not a Deck or
    /// Balcony).
    pub build_foundation: bool,
    /// Doors and windows face a room of this function as outside.
    pub faces_outside: bool,
    /// Floor height offset from the floor datum, inches (a Garage drops it).
    pub floor_height_offset: f64,
    /// Floor finish thickness, inches; `None` keeps the floor's default.
    pub floor_finish_thickness: Option<f64>,
    /// Floor Structure layers; empty keeps the floor's default platform.
    pub floor_structure: Vec<crate::extras::StructureLayer>,
}

impl Default for FunctionDefaults {
    fn default() -> Self {
        Self {
            class: FunctionClass::Interior,
            has_floor: true,
            has_ceiling: true,
            flat_ceiling: true,
            roof_over: true,
            living_area: true,
            conditioned: true,
            build_foundation: true,
            faces_outside: false,
            floor_height_offset: 0.0,
            floor_finish_thickness: None,
            floor_structure: Vec::new(),
        }
    }
}

/// The platform defaults of a room with function `function` (a room type's
/// function, see [`ROOM_FUNCTIONS`]) and room type `type_name` (an Attic or
/// Courtyard has no floor platform whatever its function, and a Courtyard,
/// open to the sky, has no ceiling either; the older function names Basement
/// and Crawl Space still work). A Slab's floor platform is
/// [`SLAB_FLOOR_THICKNESS`] thick; use [`function_defaults_with`] for the
/// plan's own slab thickness.
pub fn function_defaults(function: &str, type_name: &str) -> FunctionDefaults {
    function_defaults_with(function, type_name, SLAB_FLOOR_THICKNESS)
}

/// [`function_defaults`] with the slab thickness of the Foundation Defaults
/// (a Slab room's floor platform is that thick, manual p. 447).
pub fn function_defaults_with(
    function: &str,
    type_name: &str,
    slab_thickness: f64,
) -> FunctionDefaults {
    use crate::extras::StructureLayer as L;
    let mut d = FunctionDefaults::default();
    let f = effective_function(function, type_name);
    d.class = function_class(function, type_name);
    if d.class != FunctionClass::Interior {
        // Exterior and hybrid rooms are out of the Living Area and the
        // Conditioned Area, apart from Open Below (conditioned).
        d.living_area = false;
        d.conditioned = f == "Open Below";
    }
    match f {
        "Garage" => {
            d.floor_height_offset = -GARAGE_FLOOR_DROP;
            d.floor_finish_thickness = Some(0.0);
            d.floor_structure = vec![L::new("Concrete", SLAB_FLOOR_THICKNESS)];
        }
        "Slab" => {
            d.floor_height_offset = -GARAGE_FLOOR_DROP;
            d.floor_finish_thickness = Some(0.0);
            d.floor_structure = vec![L::new("Concrete", slab_thickness.max(0.5))];
        }
        "Deck" | "Balcony" => {
            d.has_ceiling = false;
            d.flat_ceiling = false;
            d.roof_over = false;
            d.build_foundation = false;
            d.faces_outside = true;
            d.floor_finish_thickness = Some(0.0);
            d.floor_structure = vec![L::new("Decking", 1.5), L::new("Joist", 7.25)];
        }
        "Court" => {
            d.has_ceiling = false;
            d.flat_ceiling = false;
            d.roof_over = false;
            d.build_foundation = false;
            d.faces_outside = true;
            d.floor_finish_thickness = Some(0.0);
            d.floor_structure = vec![L::new("Concrete", SLAB_FLOOR_THICKNESS)];
        }
        "Porch" => {
            d.floor_finish_thickness = Some(0.0);
            d.floor_structure = vec![L::new("Concrete", SLAB_FLOOR_THICKNESS)];
        }
        // The roof generator ignores an Attic and it has no ceiling or floor
        // platform of its own: the room below supplies them.
        "Attic" => {
            d.has_floor = false;
            d.has_ceiling = false;
            d.flat_ceiling = false;
            d.roof_over = false;
        }
        "Open Below" => d.has_floor = false,
        // A roof platform: a membrane deck with no ceiling under it.
        "Flat Roof" => {
            d.has_ceiling = false;
            d.floor_finish_thickness = Some(0.0);
            d.floor_structure = vec![L::new("Membrane", 0.5), L::new("Joist", 7.25)];
        }
        _ => {}
    }
    // An open courtyard type has the ground for a floor.
    if type_name == "Courtyard" {
        d.has_floor = false;
    }
    // Rooms of a foundation floor (R-18): a basement has a concrete slab on
    // the ground and takes its ceiling from the platform of the floor above;
    // a crawl space (Open Below) has the ground for a floor and the floor
    // above for a ceiling. Both are told by the type's name (DECISIONS 300).
    if function == "Basement" || type_name == "Basement" {
        d.has_floor = true;
        d.has_ceiling = true;
        // The slab sits on the ground inside the foundation walls.
        d.floor_height_offset = SLAB_FLOOR_THICKNESS;
        d.floor_finish_thickness = Some(0.0);
        d.floor_structure = vec![crate::extras::StructureLayer::new(
            "Concrete",
            SLAB_FLOOR_THICKNESS,
        )];
    }
    if function == "Crawl Space" || type_name == "Crawl Space" {
        d.has_floor = false;
        d.has_ceiling = false;
        d.floor_finish_thickness = Some(0.0);
        d.floor_structure = Vec::new();
    }
    d
}

/// Electrical behaviour of a room (manual p. 447): outdoor fixtures on the
/// walls of exterior rooms, no Auto Place Outlets in exterior rooms, Porches
/// and Open Below rooms, fewer in other hybrid rooms, GFCI outlets over the
/// base cabinets of kitchens and baths.
pub fn electrical_rules(function: &str, type_name: &str) -> ElectricalRules {
    let f = effective_function(function, type_name);
    let class = function_class(function, type_name);
    let t = type_name.to_lowercase();
    let wet = t.contains("kitchen") || t.contains("bath");
    ElectricalRules {
        weatherproof: class == FunctionClass::Exterior,
        outlets: match (class, f) {
            (FunctionClass::Exterior, _) | (_, "Porch" | "Open Below") => OutletPlacement::None,
            (FunctionClass::Hybrid, _) => OutletPlacement::Fewer,
            _ => OutletPlacement::Full,
        },
        gfci_over_base_cabinets: class == FunctionClass::Interior && wet,
        standard_height_outlets: class == FunctionClass::Interior && t.contains("kitchen"),
    }
}

/// What Plan Check does with a room of this function.
pub fn plan_check_rules(function: &str, type_name: &str) -> PlanCheckRules {
    PlanCheckRules {
        habitable_rules: function_class(function, type_name) == FunctionClass::Interior,
    }
}

/// Does a door or window between rooms of these two functions face outside
/// (manual p. 447)? An exterior-type room beside an interior one does: a
/// window faces out, a hinged or sliding door takes the Exterior Defaults
/// and shows a threshold. Open Below and the other hybrid rooms are treated
/// as interior.
pub fn opening_faces_outside(a: (&str, &str), b: (&str, &str)) -> bool {
    let out = |(f, t): (&str, &str)| function_class(f, t) == FunctionClass::Exterior;
    out(a) != out(b)
}

/// Give `name` the platform defaults `d` (R-41): the Structure switches, the
/// floor height offset, the Floor Structure, the floor finish (`default_finish`
/// when the function sets none), Roof Over This Room, Flat Ceiling Over This
/// Room and Build Foundation Below. Run when a room's type changes; every
/// value stays editable afterwards.
pub fn apply_function_defaults(name: &mut RoomName, d: &FunctionDefaults, default_finish: f64) {
    name.has_floor = d.has_floor;
    name.has_ceiling = d.has_ceiling;
    name.flat_ceiling = d.flat_ceiling;
    name.floor_height_offset = d.floor_height_offset;
    name.options.build_foundation_below = d.build_foundation;
    let mut misc = name.misc.take().unwrap_or_default();
    misc.roof_over = d.roof_over;
    misc.floor_structure = d.floor_structure.clone();
    misc.floor_finish_thickness = d.floor_finish_thickness.unwrap_or(default_finish);
    // The type's own floor platform replaces a layered one made before.
    misc.assemblies.floor_structure = crate::assemblies::AssemblySlot::Legacy;
    misc.assemblies.floor_finish = crate::assemblies::AssemblySlot::Legacy;
    name.misc = Some(misc);
}

// ----- Room options of the Structure and Layer panels (R-103, R-115, R-145) -----

/// Whether a resize of a platform keeps its top or its bottom where it is
/// (Structure panel, On Structure Resize).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ResizeLock {
    #[default]
    FloorTop,
    FloorBottom,
}

fn is_true(b: &bool) -> bool {
    *b
}

/// The Room Specification fields beyond the platforms: the Structure panel's
/// switches (R-115, R-145) and the Layer panel (R-103).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomOptions {
    /// Room Supplies Floor for the Room Above: this foundation-level room's
    /// slab and curbs are the floor of the room above (a garage on a slab).
    pub supplies_floor_above: bool,
    /// Floor Supplied by the Foundation 'Room' Below: the floor platform is a
    /// slab on the floor below.
    pub floor_from_foundation: bool,
    /// Build Foundation Below: Build Foundation puts a foundation under it.
    #[serde(skip_serializing_if = "is_true")]
    pub build_foundation_below: bool,
    /// Raised Floor For Bump Out: the room's floor and the ceiling of the
    /// room below build independently.
    pub raised_floor_bump_out: bool,
    /// Retain Floor/Ceiling Framing when framing is rebuilt globally.
    pub retain_framing: bool,
    /// Framing Group (without a monolithic slab).
    pub framing_group: u32,
    /// Slab Pour Number (with a monolithic slab).
    pub pour_number: u32,
    /// On Structure Resize: which side of the floor platform stays.
    pub resize_lock: ResizeLock,
    /// Shelf Ceiling: no Attic Walls over the interior walls of the room.
    pub shelf_ceiling: bool,
    /// Use Soffit Surface for Ceiling: roof over the room framed like a
    /// fascia.
    pub soffit_surface_ceiling: bool,
    /// Layer panel: the layer the room is on ("" = Rooms).
    pub layer: String,
    /// Layer panel: Drawing Group ("" = the room's own group).
    pub drawing_group: String,
}

impl Default for RoomOptions {
    fn default() -> Self {
        Self {
            supplies_floor_above: false,
            floor_from_foundation: false,
            build_foundation_below: true,
            raised_floor_bump_out: false,
            retain_framing: false,
            framing_group: 0,
            pour_number: 0,
            resize_lock: ResizeLock::FloorTop,
            shelf_ceiling: false,
            soffit_surface_ceiling: false,
            layer: String::new(),
            drawing_group: String::new(),
        }
    }
}

impl RoomOptions {
    pub fn is_default(&self) -> bool {
        *self == RoomOptions::default()
    }

    /// The layer the room is drawn on.
    pub fn layer_name(&self) -> &str {
        if self.layer.trim().is_empty() {
            ROOM_LAYER
        } else {
            &self.layer
        }
    }
}

/// The layer rooms are drawn on unless the Layer panel names another.
pub const ROOM_LAYER: &str = "Rooms";

// ----- Room Type Defaults: the settings a type hands its rooms (R-99) -----

/// The settings of a Room Type beyond its name, function and living and
/// conditioned inclusion (manual p. 445): ceiling and floor structure and
/// finish, deck framing and supports, layer and drawing group, fill style,
/// moldings and the label. Assigning the type to a room copies them to it
/// ([`apply_type_spec`]).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomTypeSpec {
    /// Floor and Ceiling Structure and Finish; a slot that is not set follows
    /// the function and the floor.
    #[serde(skip_serializing_if = "crate::assemblies::PlatformAssemblies::is_legacy")]
    pub assemblies: crate::assemblies::PlatformAssemblies,
    /// Planking, framing and supports of a Deck room.
    pub deck: Option<crate::deck::DeckSpec>,
    pub layer: String,
    pub drawing_group: String,
    pub fill: Option<crate::extras::RoomFill>,
    /// Base, chair rail and crown moldings.
    pub moldings: Vec<crate::extras::MoldingRef>,
    /// What the label shows (the offset is per room and ignored).
    pub label: Option<crate::extras::RoomLabelOptions>,
}

impl RoomTypeSpec {
    pub fn is_default(&self) -> bool {
        *self == RoomTypeSpec::default()
    }
}

/// Gives `name` the settings of its Room Type's `spec` (overriding what the
/// room had, manual p. 446). Platform definitions the spec sets replace the
/// room's; a spec without a Deck Specification leaves a Deck room's alone.
pub fn apply_type_spec(name: &mut RoomName, spec: &RoomTypeSpec) {
    if !spec.assemblies.is_legacy() {
        let mut misc = name.misc.take().unwrap_or_default();
        for kind in crate::assemblies::AssemblyKind::PLATFORM {
            if let crate::assemblies::AssemblySlot::Own(a) = spec.assemblies.slot(kind) {
                misc.assemblies
                    .set(kind, crate::assemblies::AssemblySlot::Own(a.clone()));
            }
        }
        name.misc = Some(misc);
    }
    if let Some(d) = &spec.deck {
        name.deck = Some(d.clone());
    }
    name.options.layer = spec.layer.clone();
    name.options.drawing_group = spec.drawing_group.clone();
    if spec.fill.is_some() {
        name.fill_style = spec.fill.clone();
    }
    if !spec.moldings.is_empty() {
        name.moldings = spec.moldings.clone();
    }
    if let Some(l) = &spec.label {
        let offset = name.label.offset;
        name.label = l.clone();
        name.label.offset = offset;
    }
}

// ----- Room Specification: slab flag, label style, moldings (round 14) -----

/// Default thickness of a monolithic slab, inches (R-31).
pub const MONOLITHIC_SLAB_THICKNESS: f64 = 4.0;
/// Default depth of the thickened edge of a monolithic slab, inches (R-31).
pub const MONOLITHIC_STEM_HEIGHT: f64 = 12.0;

/// The Monolithic Slab Foundation flag of a room on the first floor (R-31):
/// the room's floor is a concrete slab of `thickness` with a thickened edge
/// `stem_height` deep under its exterior walls, and the foundation floor is
/// not needed under it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomSlab {
    /// Slab thickness, inches.
    pub thickness: f64,
    /// Depth of the thickened edge below the floor datum, inches.
    pub stem_height: f64,
}

impl Default for RoomSlab {
    fn default() -> Self {
        Self {
            thickness: MONOLITHIC_SLAB_THICKNESS,
            stem_height: MONOLITHIC_STEM_HEIGHT,
        }
    }
}

/// Where a room's plan label sits before it is dragged (R-44, R-46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LabelPlacement {
    /// At the room's label point (inside the room, near its centroid).
    #[default]
    Center,
    Top,
    Bottom,
    Left,
    Right,
}

impl LabelPlacement {
    pub const ALL: [LabelPlacement; 5] = [
        LabelPlacement::Center,
        LabelPlacement::Top,
        LabelPlacement::Bottom,
        LabelPlacement::Left,
        LabelPlacement::Right,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LabelPlacement::Center => "Center of Room",
            LabelPlacement::Top => "Near Top Wall",
            LabelPlacement::Bottom => "Near Bottom Wall",
            LabelPlacement::Left => "Near Left Wall",
            LabelPlacement::Right => "Near Right Wall",
        }
    }
}

/// The Label tab's appearance options (R-46): a named text style (empty is
/// "Use Layer Text Style", which for room labels is "Room Label Style") and
/// where the label sits before it is dragged.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomLabelStyle {
    pub text_style: String,
    pub placement: LabelPlacement,
}

/// The text style room labels use when a room names none.
pub const ROOM_LABEL_TEXT_STYLE: &str = "Room Label Style";

impl RoomLabelStyle {
    /// The text style to draw the label in.
    pub fn style_name(&self) -> &str {
        if self.text_style.trim().is_empty() {
            ROOM_LABEL_TEXT_STYLE
        } else {
            &self.text_style
        }
    }
}

impl Room {
    /// A point inside the room proper: the centroid, or another interior
    /// point for a concave room.
    pub fn interior_point(&self) -> Point {
        if self.contains(self.centroid) {
            return self.centroid;
        }
        let n = self.polygon.len();
        for i in 0..n {
            let (a, b, c) = (
                self.polygon[i],
                self.polygon[(i + 1) % n],
                self.polygon[(i + 2) % n],
            );
            let t = Point::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0);
            if self.contains(t) {
                return t;
            }
        }
        self.centroid
    }

    /// Where the label of this room sits for `placement` (R-44, R-46): the
    /// interior point for [`LabelPlacement::Center`], else the middle of the
    /// named side of the interior bounds, `inset` inches in from the wall
    /// (pulled toward the interior point when that spot is outside the room).
    pub fn label_point(&self, placement: LabelPlacement, inset: f64) -> Point {
        let anchor = self.interior_point();
        if placement == LabelPlacement::Center {
            return anchor;
        }
        let poly = if self.inner_polygon.len() >= 3 {
            &self.inner_polygon
        } else {
            &self.polygon
        };
        let (lo, hi) = crate::foundation::bounds(poly);
        let mid = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
        let want = match placement {
            LabelPlacement::Top => Point::new(mid.x, hi.y - inset),
            LabelPlacement::Bottom => Point::new(mid.x, lo.y + inset),
            LabelPlacement::Left => Point::new(lo.x + inset, mid.y),
            LabelPlacement::Right => Point::new(hi.x - inset, mid.y),
            LabelPlacement::Center => anchor,
        };
        for k in 0..=8 {
            let p = Point::lerp(want, anchor, f64::from(k) / 8.0);
            if self.contains(p) {
                return p;
            }
        }
        anchor
    }
}

/// One molding profile of the library (R-34): a name, what it is for and its
/// cross section as `(projection, height)` points in inches, counter-clockwise,
/// measured from the molding's bottom edge at the wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoldingDef {
    pub name: &'static str,
    pub kind: MoldingKind,
    pub section: &'static [(f64, f64)],
}

impl MoldingDef {
    /// Vertical size of the profile, inches.
    pub fn height(&self) -> f64 {
        self.section.iter().map(|p| p.1).fold(0.0, f64::max)
    }

    /// How far the profile projects from the wall, inches.
    pub fn projection(&self) -> f64 {
        self.section.iter().map(|p| p.0).fold(0.0, f64::max)
    }

    /// The cross section as plan points `(projection, height)`.
    pub fn points(&self) -> Vec<Point> {
        self.section
            .iter()
            .map(|&(x, y)| Point::new(x, y))
            .collect()
    }
}

/// The molding library the Moldings tab picks base, crown and chair rail
/// profiles from.
pub const MOLDING_LIBRARY: [MoldingDef; 9] = [
    MoldingDef {
        name: "Base - Square 3 1/4",
        kind: MoldingKind::Base,
        section: &[(0.0, 0.0), (0.5, 0.0), (0.5, 3.25), (0.0, 3.25)],
    },
    MoldingDef {
        name: "Base - Colonial 5 1/4",
        kind: MoldingKind::Base,
        section: &[
            (0.0, 0.0),
            (0.75, 0.0),
            (0.75, 0.35),
            (0.6, 0.5),
            (0.5, 0.9),
            (0.5, 4.6),
            (0.65, 4.8),
            (0.65, 5.05),
            (0.4, 5.25),
            (0.0, 5.25),
        ],
    },
    MoldingDef {
        name: "Base - Craftsman 7 1/4",
        kind: MoldingKind::Base,
        section: &[
            (0.0, 0.0),
            (0.75, 0.0),
            (0.75, 6.25),
            (1.0, 6.25),
            (1.0, 7.0),
            (0.75, 7.25),
            (0.0, 7.25),
        ],
    },
    MoldingDef {
        name: "Crown - Cove 3 5/8",
        kind: MoldingKind::Crown,
        section: &[
            (0.0, 0.0),
            (0.28, 1.39),
            (1.06, 2.57),
            (2.24, 3.35),
            (3.625, 3.625),
            (0.0, 3.625),
        ],
    },
    MoldingDef {
        name: "Crown - Colonial 4 5/8",
        kind: MoldingKind::Crown,
        section: &[
            (0.0, 0.0),
            (0.5, 0.0),
            (0.5, 0.5),
            (1.1, 1.0),
            (1.9, 1.6),
            (2.6, 2.4),
            (3.1, 3.3),
            (3.75, 3.5),
            (3.75, 4.625),
            (0.0, 4.625),
        ],
    },
    MoldingDef {
        name: "Crown - Stepped 6",
        kind: MoldingKind::Crown,
        section: &[
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 1.5),
            (2.5, 1.5),
            (2.5, 3.0),
            (4.0, 3.0),
            (4.0, 4.5),
            (5.0, 4.5),
            (5.0, 6.0),
            (0.0, 6.0),
        ],
    },
    MoldingDef {
        name: "Chair Rail - Simple 2 1/2",
        kind: MoldingKind::Chair,
        section: &[
            (0.0, 0.0),
            (0.5, 0.0),
            (0.5, 0.5),
            (0.75, 0.75),
            (0.75, 1.9),
            (0.4, 2.2),
            (0.4, 2.5),
            (0.0, 2.5),
        ],
    },
    MoldingDef {
        name: "Chair Rail - Colonial 3",
        kind: MoldingKind::Chair,
        section: &[
            (0.0, 0.0),
            (0.6, 0.0),
            (0.6, 0.4),
            (0.9, 0.7),
            (0.9, 1.5),
            (1.0, 1.7),
            (1.0, 2.4),
            (0.6, 2.8),
            (0.4, 3.0),
            (0.0, 3.0),
        ],
    },
    MoldingDef {
        name: "Chair Rail - Flat 3 1/2",
        kind: MoldingKind::Chair,
        section: &[(0.0, 0.0), (0.5, 0.0), (0.5, 3.5), (0.0, 3.5)],
    },
];

/// Surface materials the Materials tab offers for floors (R-36). Each name
/// maps to a 3D surface to a 3D surface: wood names keep the
/// wood floor, the rest lay a plate of their own material.
pub const FLOOR_SURFACES: [&str; 8] = [
    "Oak Hardwood",
    "Maple Hardwood",
    "Ceramic Tile",
    "Marble",
    "Concrete",
    "Brick",
    "Stone",
    "Painted Trim White",
];

/// Surface materials the Materials tab offers for ceilings.
pub const CEILING_SURFACES: [&str; 5] = [
    "Painted Drywall",
    "White Paint",
    "Wood Planks",
    "Stucco",
    "Plaster",
];

/// Surface materials the Materials tab offers for interior walls.
pub const WALL_SURFACES: [&str; 8] = [
    "Painted Drywall",
    "Plaster",
    "Wood Paneling",
    "Brick",
    "Stone",
    "Ceramic Tile",
    "Stucco",
    "Concrete",
];

/// The library profile with this name (case-insensitive).
pub fn molding_def(name: &str) -> Option<&'static MoldingDef> {
    let n = name.trim();
    MOLDING_LIBRARY
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(n))
}

/// The library profiles for one kind of molding, in list order.
pub fn molding_defs(kind: MoldingKind) -> Vec<&'static MoldingDef> {
    MOLDING_LIBRARY.iter().filter(|m| m.kind == kind).collect()
}

/// Height above the finished floor where a chair rail starts, inches.
pub const CHAIR_RAIL_HEIGHT: f64 = 32.0;

/// Elevation range `(bottom, top)` of a molding of `kind` and `height`
/// above the room's finished floor, for a finished ceiling `ceiling` above
/// that floor, inches. Base sits on the floor, a chair rail at
/// [`CHAIR_RAIL_HEIGHT`], crown hangs from the ceiling.
pub fn molding_span(kind: MoldingKind, height: f64, ceiling: f64) -> (f64, f64) {
    match kind {
        MoldingKind::Base => (0.0, height),
        MoldingKind::Chair => (CHAIR_RAIL_HEIGHT, CHAIR_RAIL_HEIGHT + height),
        MoldingKind::Crown => ((ceiling - height).max(0.0), ceiling),
    }
}

/// The wall that owns the interior-surface edge `p`-`q` of a room: parallel
/// to it, with the edge about half a thickness from its centerline. Curved
/// walls own no edge (their facets are not straight walls).
pub fn edge_wall(floor: &Floor, p: Point, q: Point) -> Option<&Wall> {
    use crate::geometry::dist_to_segment;
    let mid = Point::lerp(p, q, 0.5);
    let dir = q.sub(p).normalized();
    floor
        .walls
        .iter()
        .filter(|w| w.length() > 1e-6 && !w.is_curved())
        .filter(|w| w.direction().cross(dir).abs() < 0.02)
        .filter_map(|w| {
            let d = dist_to_segment(mid, w.start, w.end);
            let off = (d - w.thickness * 0.5).abs();
            (d <= w.thickness * 0.5 + 1.5 && off <= 1.5).then_some((off, w))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, w)| w)
}

/// An opening seen along an interior-surface edge: where it lies along the
/// edge and how high it reaches above the floor datum, inches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeOpening {
    pub from: f64,
    pub to: f64,
    pub sill: f64,
    pub head: f64,
}

/// The openings of the wall under the interior-surface edge `p`-`q`, sorted
/// along the edge.
pub fn edge_openings(floor: &Floor, p: Point, q: Point) -> Vec<EdgeOpening> {
    let dir = q.sub(p).normalized();
    let mut out: Vec<EdgeOpening> = Vec::new();
    if let Some(w) = edge_wall(floor, p, q) {
        for o in floor.openings_on(w.id) {
            let c = w.point_at(o.center_offset).sub(p).dot(dir);
            out.push(EdgeOpening {
                from: c - o.width * 0.5,
                to: c + o.width * 0.5,
                sill: o.sill_height,
                head: o.sill_height + o.height,
            });
        }
    }
    out.sort_by(|a, b| a.from.total_cmp(&b.from));
    out
}

/// Pieces of the edge `p`-`q` (as `(from, to)` distances along it) not
/// crossed by an opening of the owning wall between `bottom` and `top` above
/// the floor datum.
fn edge_pieces(floor: &Floor, p: Point, q: Point, bottom: f64, top: f64) -> Vec<(f64, f64)> {
    let len = p.dist(q);
    let mut pieces = Vec::new();
    let mut cursor = 0.0;
    for o in edge_openings(floor, p, q)
        .into_iter()
        .filter(|o| o.sill < top && o.head > bottom)
    {
        if o.from > cursor {
            pieces.push((cursor, o.from.min(len)));
        }
        cursor = cursor.max(o.to);
    }
    if cursor < len {
        pieces.push((cursor, len));
    }
    pieces.retain(|(a, b)| b - a > 0.5);
    pieces
}

impl Room {
    /// The paths a molding spanning `bottom..top` above the floor datum runs
    /// along inside this room (R-34): the interior-surface outline, broken
    /// where an opening of the wall reaches into that height range. Corners
    /// stay joined, so a molding that meets no opening is one closed path
    /// (first point repeated at the end). Each path is counter-clockwise, the
    /// molding projecting to its left (into the room).
    pub fn molding_runs(&self, floor: &Floor, bottom: f64, top: f64) -> Vec<Vec<Point>> {
        let poly = if self.inner_polygon.len() >= 3 {
            &self.inner_polygon
        } else {
            &self.polygon
        };
        let n = poly.len();
        if n < 3 {
            return Vec::new();
        }
        // Every uncut piece of every edge, with whether it starts and ends
        // at a corner of the outline.
        let mut pieces: Vec<(Point, Point, bool, bool)> = Vec::new();
        for i in 0..n {
            let (p, q) = (poly[i], poly[(i + 1) % n]);
            let len = p.dist(q);
            if len < 1e-6 {
                continue;
            }
            let dir = q.sub(p).normalized();
            for (a, b) in edge_pieces(floor, p, q, bottom, top) {
                pieces.push((p + dir * a, p + dir * b, a < 1e-6, len - b < 1e-6));
            }
        }
        let mut runs: Vec<Vec<Point>> = Vec::new();
        let mut prev_open_end = false;
        for &(pa, pb, at_start, at_end) in &pieces {
            if at_start && prev_open_end && !runs.is_empty() {
                if let Some(last) = runs.last_mut() {
                    last.push(pb);
                }
            } else {
                runs.push(vec![pa, pb]);
            }
            prev_open_end = at_end;
        }
        // The last run continues into the first around the outline's start.
        let wraps = pieces.first().is_some_and(|f| f.2) && pieces.last().is_some_and(|l| l.3);
        if wraps && runs.len() > 1 {
            let first = runs.remove(0);
            if let Some(last) = runs.last_mut() {
                last.extend(first.into_iter().skip(1));
            }
        } else if wraps && runs.len() == 1 {
            let r = &mut runs[0];
            if r[0].dist(r[r.len() - 1]) > 1e-6 {
                let p0 = r[0];
                r.push(p0);
            }
        }
        runs
    }
}

/// Is `inner` wholly inside `outer`: every vertex and edge midpoint inside
/// the polygon or on its boundary (within `tol`), and no bigger than it
/// (R-40, Open Below)?
pub fn polygon_inside(inner: &[Point], outer: &[Point], tol: f64) -> bool {
    if inner.len() < 3 || outer.len() < 3 {
        return false;
    }
    let ok = |p: Point| {
        point_in_polygon(p, outer)
            || (0..outer.len())
                .any(|i| dist_to_segment(p, outer[i], outer[(i + 1) % outer.len()]) <= tol)
    };
    let n = inner.len();
    (0..n).all(|i| ok(inner[i]) && ok(Point::lerp(inner[i], inner[(i + 1) % n], 0.5)))
        && polygon_area(inner).abs() <= polygon_area(outer).abs() * 1.02 + 1.0
}

/// Ignore faces smaller than this (slivers from near-coincident walls). 1 sq ft.
const MIN_ROOM_AREA_SQ_IN: f64 = 144.0;

/// Detect rooms. Same as [`detect_rooms_inner`]: `polygon`/`area_sq_in` stay
/// the centerline values and the interior/standard fields are filled in too.
pub fn detect_rooms(walls: &[Wall], tol: f64) -> Vec<Room> {
    detect_rooms_inner(walls, tol)
}

/// Detect rooms and compute their interior-surface polygons (R-2): each
/// centerline polygon edge is offset inward by half the thickness of the wall
/// that owns it and adjacent offset edges are intersected. Walls whose flags
/// say they do not define rooms, or that stand raised above the floor, are skipped ([`crate::walls::Wall::defines_rooms`],
/// R-3..R-5). Curved walls bound rooms as faceted arcs (R-12).
pub fn detect_rooms_inner(walls: &[Wall], tol: f64) -> Vec<Room> {
    detect_rooms_with(walls, tol, true)
}

/// [`detect_rooms_inner`], with the position index switched on or off (off is
/// the plain scan the index must agree with).
fn detect_rooms_with(walls: &[Wall], tol: f64, indexed: bool) -> Vec<Room> {
    let defining: Vec<Wall> = expand_curves(walls)
        .into_iter()
        .filter(|w| w.defines_rooms())
        .collect();
    let mut rooms = detect_centerline_rooms(&defining, tol, indexed);
    // Each room edge looks for its owning wall among the walls near it.
    let reach = tol.max(0.5);
    let near = WallsNear::new(&defining, reach, indexed);
    for r in rooms.iter_mut() {
        fill_surface_areas(r, &defining, &near, tol);
    }
    nest_rooms(&mut rooms);
    rooms
}

/// Nested rooms (R-11): a room whose polygon lies wholly inside another
/// room's becomes a hole of the smallest such room, and that room's areas
/// give up what the island covers. Rooms that share wall edges (neighbours)
/// are never nested: every vertex of an island must lie strictly inside the
/// enclosing polygon.
fn nest_rooms(rooms: &mut [Room]) {
    let n = rooms.len();
    if n < 2 {
        return;
    }
    let bounds: Vec<(Point, Point)> = rooms
        .iter()
        .map(|r| crate::foundation::bounds(&r.polygon))
        .collect();
    // Index of the smallest enclosing room of each room.
    let mut parent: Vec<Option<usize>> = vec![None; n];
    for j in 0..n {
        let mut best: Option<usize> = None;
        for i in 0..n {
            if i == j || rooms[i].area_sq_in <= rooms[j].area_sq_in {
                continue;
            }
            let (ilo, ihi) = bounds[i];
            let (jlo, jhi) = bounds[j];
            if jlo.x < ilo.x || jlo.y < ilo.y || jhi.x > ihi.x || jhi.y > ihi.y {
                continue;
            }
            let inside = rooms[j].polygon.iter().all(|&p| {
                point_in_polygon(p, &rooms[i].polygon) && !on_boundary(p, &rooms[i].polygon)
            });
            if inside && best.is_none_or(|b| rooms[i].area_sq_in < rooms[b].area_sq_in) {
                best = Some(i);
            }
        }
        parent[j] = best;
    }
    for j in 0..n {
        let Some(i) = parent[j] else { continue };
        let hole = rooms[j].polygon.clone();
        let (area, outer) = (
            rooms[j].area_sq_in,
            rooms[j].outer_area_sq_in.max(rooms[j].area_sq_in),
        );
        let r = &mut rooms[i];
        r.holes.push(hole);
        r.area_sq_in = (r.area_sq_in - area).max(0.0);
        r.interior_area_sq_in = (r.interior_area_sq_in - outer).max(0.0);
        r.standard_area_sq_in = (r.standard_area_sq_in - area).max(0.0);
    }
}

/// Is `p` on the outline of `poly` (within a hair)?
fn on_boundary(p: Point, poly: &[Point]) -> bool {
    (0..poly.len()).any(|i| dist_to_segment(p, poly[i], poly[(i + 1) % poly.len()]) < 1e-6)
}

/// Replace curved walls by their faceted chords (same id and properties).
pub(crate) fn expand_curves(walls: &[Wall]) -> Vec<Wall> {
    let mut out = Vec::with_capacity(walls.len());
    for w in walls {
        match w.curve {
            Some(c) if !c.is_straight() => {
                let n = c.facet_count(w.start, w.end);
                let pts = c.sample_points(w.start, w.end, n);
                for pair in pts.windows(2) {
                    let mut f = w.clone();
                    f.curve = None;
                    f.start = pair[0];
                    f.end = pair[1];
                    out.push(f);
                }
            }
            _ => out.push(w.clone()),
        }
    }
    out
}

/// Offset a counter-clockwise polygon: edge `i` (from vertex `i` to `i+1`)
/// moves left (inward) by `d[i]`; negative values move it outward. Adjacent
/// offset edges are intersected; parallel neighbours with different offsets
/// get a step. Returns `None` for a degenerate result.
pub(crate) fn offset_polygon(poly: &[Point], d: &[f64]) -> Option<Vec<Point>> {
    let n = poly.len();
    if n < 3 {
        return None;
    }
    let dirs: Vec<Point> = (0..n)
        .map(|i| poly[(i + 1) % n].sub(poly[i]).normalized())
        .collect();
    let mut out = Vec::with_capacity(n + 2);
    for j in 0..n {
        let prev = (j + n - 1) % n;
        let (dp, dc) = (dirs[prev], dirs[j]);
        let a = poly[j] + dp.perp() * d[prev];
        let b = poly[j] + dc.perp() * d[j];
        let cross = dp.cross(dc);
        if cross.abs() < 1e-6 {
            if (d[prev] - d[j]).abs() > 1e-9 {
                out.push(a);
            }
            out.push(b);
        } else {
            let t = (b - a).cross(dc) / cross;
            out.push(a + dp * t);
        }
    }
    let area = polygon_area(&out);
    (area.is_finite() && area > 0.0).then_some(out)
}

/// Walls found by position: a broad-phase over the walls' bounding boxes
/// (small lists are scanned instead). Candidates come back in wall order.
pub(crate) struct WallsNear {
    grid: Option<BoxGrid>,
    len: usize,
    reach: f64,
}

/// Below this many walls a scan beats building a grid.
const GRID_MIN_WALLS: usize = 24;

impl WallsNear {
    /// `reach` is how far from a wall's own box a query point still counts.
    pub(crate) fn new(walls: &[Wall], reach: f64, indexed: bool) -> Self {
        let grid = (indexed && walls.len() >= GRID_MIN_WALLS).then(|| {
            let boxes: Vec<(Point, Point)> = walls
                .iter()
                .map(|w| {
                    (
                        Point::new(
                            w.start.x.min(w.end.x) - reach,
                            w.start.y.min(w.end.y) - reach,
                        ),
                        Point::new(
                            w.start.x.max(w.end.x) + reach,
                            w.start.y.max(w.end.y) + reach,
                        ),
                    )
                })
                .collect();
            BoxGrid::new(&boxes)
        });
        WallsNear {
            grid,
            len: walls.len(),
            reach,
        }
    }

    /// The walls that may lie within `reach` of `p`, ascending.
    fn around(&self, p: Point, out: &mut Vec<usize>) {
        match &self.grid {
            Some(g) => g.query(p, p, out),
            None => {
                out.clear();
                out.extend(0..self.len);
            }
        }
    }
}

/// The wall that owns the polygon edge `a -> b`: nearest parallel wall to the
/// edge midpoint (the thicker one on ties).
pub(crate) fn edge_owner<'a>(
    walls: &'a [Wall],
    near: &WallsNear,
    a: Point,
    b: Point,
    tol: f64,
) -> Option<&'a Wall> {
    let mid = Point::lerp(a, b, 0.5);
    let dir = b.sub(a).normalized();
    let mut best: Option<(f64, &Wall)> = None;
    let mut candidates = Vec::new();
    near.around(mid, &mut candidates);
    debug_assert!(near.reach >= tol.max(0.5));
    for &i in &candidates {
        let w = &walls[i];
        if w.length() <= tol || w.direction().cross(dir).abs() > 1e-3 {
            continue;
        }
        let d = dist_to_segment(mid, w.start, w.end);
        if d > tol.max(0.5) {
            continue;
        }
        let better = match best {
            None => true,
            Some((bd, bw)) => {
                d < bd - 1e-9 || ((d - bd).abs() <= 1e-9 && w.thickness > bw.thickness)
            }
        };
        if better {
            best = Some((d, w));
        }
    }
    best.map(|(_, w)| w)
}

fn fill_surface_areas(room: &mut Room, walls: &[Wall], near: &WallsNear, tol: f64) {
    let n = room.polygon.len();
    let owners: Vec<Option<&Wall>> = (0..n)
        .map(|i| edge_owner(walls, near, room.polygon[i], room.polygon[(i + 1) % n], tol))
        .collect();
    let inner_d: Vec<f64> = owners
        .iter()
        .map(|o| o.map_or(0.0, |w| w.thickness * 0.5))
        .collect();
    let std_d: Vec<f64> = owners
        .iter()
        .map(|o| match o {
            Some(w) if w.kind == WallKind::Exterior => -w.thickness * 0.5,
            _ => 0.0,
        })
        .collect();
    room.inner_polygon =
        offset_polygon(&room.polygon, &inner_d).unwrap_or_else(|| room.polygon.clone());
    room.interior_area_sq_in = polygon_area(&room.inner_polygon);
    let outer_d: Vec<f64> = inner_d.iter().map(|d| -d).collect();
    room.outer_area_sq_in = offset_polygon(&room.polygon, &outer_d)
        .map(|p| polygon_area(&p))
        .unwrap_or(room.area_sq_in);
    room.standard_area_sq_in = offset_polygon(&room.polygon, &std_d)
        .map(|p| polygon_area(&p))
        .unwrap_or(room.area_sq_in);
}

impl Project {
    /// Total living area of `floor` in square feet (R-51, R-42): the interior
    /// area of every room whose Living Area setting resolves to included. A
    /// room's setting is its name entry's `include_in_living_area` override,
    /// else the `include_in_living_area` of its room type in `room_types`
    /// (unnamed or unknown types count as included).
    pub fn living_area_sq_ft(
        &self,
        floor: usize,
        rooms: &[Room],
        room_types: &[RoomTypeDef],
    ) -> f64 {
        let names = &self.floors[floor].room_names;
        let total: f64 = rooms
            .iter()
            .filter(|r| {
                let Some(n) = r.name_entry(names) else {
                    return true;
                };
                n.include_in_living_area.unwrap_or_else(|| {
                    room_types
                        .iter()
                        .find(|t| t.name == n.room_type)
                        .is_none_or(|t| t.include_in_living_area)
                })
            })
            .map(|r| {
                if r.inner_polygon.is_empty() {
                    r.area_sq_in
                } else {
                    r.interior_area_sq_in
                }
            })
            .sum();
        sq_in_to_sq_ft(total)
    }
}

fn detect_centerline_rooms(walls: &[Wall], tol: f64, indexed: bool) -> Vec<Room> {
    let segs = split_segments(walls, tol, indexed);

    let mut nodes = NodeIndex::new(tol, indexed);
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut seen_edges: HashSet<(usize, usize)> = HashSet::new();
    for (a, b) in segs {
        let ia = nodes.index(a);
        let ib = nodes.index(b);
        if ia == ib {
            continue;
        }
        let key = (ia.min(ib), ia.max(ib));
        if seen_edges.insert(key) {
            edges.push(key);
        }
    }
    let nodes = nodes.points;

    // Adjacency sorted by outgoing angle, ascending (counter-clockwise order).
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for &(a, b) in &edges {
        adj[a].push(b);
        adj[b].push(a);
    }
    for (i, list) in adj.iter_mut().enumerate() {
        let o = nodes[i];
        list.sort_by(|&p, &q| {
            let ap = nodes[p].sub(o).angle();
            let aq = nodes[q].sub(o).angle();
            ap.partial_cmp(&aq).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let mut visited: HashSet<(usize, usize)> = HashSet::new();
    let mut rooms = Vec::new();
    let max_steps = edges.len() * 2 + 2;

    for &(a, b) in &edges {
        for (u0, v0) in [(a, b), (b, a)] {
            if visited.contains(&(u0, v0)) {
                continue;
            }
            let mut loop_nodes = Vec::new();
            let (mut u, mut v) = (u0, v0);
            let mut steps = 0;
            loop {
                visited.insert((u, v));
                loop_nodes.push(u);
                let list = &adj[v];
                let idx = list
                    .iter()
                    .position(|&n| n == u)
                    .expect("edge must be in adjacency");
                // First edge clockwise from the reversed incoming direction.
                let next = list[(idx + list.len() - 1) % list.len()];
                u = v;
                v = next;
                steps += 1;
                if (u, v) == (u0, v0) || steps > max_steps {
                    break;
                }
            }
            let polygon: Vec<Point> = loop_nodes.iter().map(|&i| nodes[i]).collect();
            let area = polygon_area(&polygon);
            if area > MIN_ROOM_AREA_SQ_IN {
                let centroid = polygon_centroid(&polygon);
                rooms.push(Room {
                    polygon,
                    area_sq_in: area,
                    centroid,
                    ..Room::default()
                });
            }
        }
    }

    // Stable, readable ordering: top-left rooms first.
    rooms.sort_by(|r, s| {
        (-r.centroid.y, r.centroid.x)
            .partial_cmp(&(-s.centroid.y, s.centroid.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (i, r) in rooms.iter_mut().enumerate() {
        r.label = format!("Room {}", i + 1);
    }
    rooms
}

/// The graph's nodes: points merged within `tol`, found through a hash of
/// `tol`-sized cells instead of a scan. `index` answers with the lowest node
/// index within `tol`, the same as scanning the list from the start.
struct NodeIndex {
    points: Vec<Point>,
    tol: f64,
    /// Off: every lookup scans the list (what the hash must agree with).
    hashed: bool,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl NodeIndex {
    fn new(tol: f64, hashed: bool) -> Self {
        NodeIndex {
            points: Vec::new(),
            tol,
            hashed,
            cells: HashMap::new(),
        }
    }

    fn cell(&self, p: Point) -> (i64, i64) {
        (
            (p.x / self.tol).floor() as i64,
            (p.y / self.tol).floor() as i64,
        )
    }

    fn index(&mut self, p: Point) -> usize {
        if self.hashed && self.tol > 0.0 && self.tol.is_finite() {
            let (cx, cy) = self.cell(p);
            let mut best: Option<usize> = None;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(list) = self.cells.get(&(cx + dx, cy + dy)) {
                        for &i in list {
                            if best.is_none_or(|b| i < b) && self.points[i].dist(p) <= self.tol {
                                best = Some(i);
                            }
                        }
                    }
                }
            }
            if let Some(i) = best {
                return i;
            }
            self.points.push(p);
            let i = self.points.len() - 1;
            self.cells.entry((cx, cy)).or_default().push(i);
            return i;
        }
        // A zero or odd tolerance: the plain scan.
        if let Some(i) = self.points.iter().position(|n| n.dist(p) <= self.tol) {
            return i;
        }
        self.points.push(p);
        self.points.len() - 1
    }
}

/// Break every wall centerline at the points where other walls end on it
/// (T-junctions) or cross it, so the graph is planar.
fn split_segments(walls: &[Wall], tol: f64, indexed: bool) -> Vec<(Point, Point)> {
    let near = WallsNear::new(walls, tol, indexed);
    let mut candidates = Vec::new();
    let mut out = Vec::new();
    for (i, w) in walls.iter().enumerate() {
        let (a, b) = (w.start, w.end);
        let len = a.dist(b);
        if len < tol {
            continue;
        }
        let mut ts = vec![0.0, 1.0];
        // Only walls whose box meets this wall's box (grown by `tol`) can end
        // on it or cross it.
        match &near.grid {
            Some(g) => g.query(
                Point::new(a.x.min(b.x) - tol, a.y.min(b.y) - tol),
                Point::new(a.x.max(b.x) + tol, a.y.max(b.y) + tol),
                &mut candidates,
            ),
            None => {
                candidates.clear();
                candidates.extend(0..walls.len());
            }
        }
        for &j in &candidates {
            if i == j {
                continue;
            }
            let o = &walls[j];
            for p in [o.start, o.end] {
                let (t, q) = project_on_segment(p, a, b);
                if q.dist(p) <= tol && t > 0.0 && t < 1.0 {
                    ts.push(t);
                }
            }
            if let Some((t, u)) = segment_intersection(a, b, o.start, o.end) {
                if t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0 {
                    ts.push(t);
                }
            }
        }
        ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        ts.dedup_by(|x, y| (*x - *y).abs() * len < tol);
        for k in 0..ts.len() - 1 {
            let p = Point::lerp(a, b, ts[k]);
            let q = Point::lerp(a, b, ts[k + 1]);
            if p.dist(q) > tol {
                out.push((p, q));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WallKind;

    fn wall(id: u64, x0: f64, y0: f64, x1: f64, y1: f64) -> Wall {
        wall_t(id, x0, y0, x1, y1, 4.5, WallKind::Interior)
    }

    fn wall_t(id: u64, x0: f64, y0: f64, x1: f64, y1: f64, t: f64, kind: WallKind) -> Wall {
        Wall {
            id,
            ..Wall::new(Point::new(x0, y0), Point::new(x1, y1), t, 109.125, kind)
        }
    }

    fn box_walls(t: f64) -> Vec<Wall> {
        let k = WallKind::Exterior;
        vec![
            wall_t(1, 0.0, 0.0, 240.0, 0.0, t, k),
            wall_t(2, 240.0, 0.0, 240.0, 120.0, t, k),
            wall_t(3, 240.0, 120.0, 0.0, 120.0, t, k),
            wall_t(4, 0.0, 120.0, 0.0, 0.0, t, k),
        ]
    }

    /// A 20' x 10' room with a free-standing 4' x 3' closet loop inside it.
    fn room_with_closet(t: f64) -> Vec<Wall> {
        let k = WallKind::Interior;
        let mut walls = box_walls(t);
        walls.extend([
            wall_t(5, 60.0, 40.0, 108.0, 40.0, t, k),
            wall_t(6, 108.0, 40.0, 108.0, 76.0, t, k),
            wall_t(7, 108.0, 76.0, 60.0, 76.0, t, k),
            wall_t(8, 60.0, 76.0, 60.0, 40.0, t, k),
        ]);
        walls
    }

    #[test]
    fn a_closet_loop_inside_a_room_is_nested() {
        let rooms = detect_rooms(&room_with_closet(4.5), 0.5);
        assert_eq!(rooms.len(), 2, "outer room and closet");
        let (outer, closet) = if rooms[0].holes.is_empty() {
            (&rooms[1], &rooms[0])
        } else {
            (&rooms[0], &rooms[1])
        };
        assert_eq!(outer.holes.len(), 1);
        assert!(closet.holes.is_empty());
        let island = 48.0 * 36.0;
        assert!((outer.area_sq_in - (240.0 * 120.0 - island)).abs() < 1e-6);
        assert!((closet.area_sq_in - island).abs() < 1e-6);
        // The interior area gives up the closet's outside surfaces, not just
        // its centerline area.
        let outside = (48.0 + 4.5) * (36.0 + 4.5);
        let box_inner = (240.0 - 4.5) * (120.0 - 4.5);
        assert!((outer.interior_area_sq_in - (box_inner - outside)).abs() < 1e-6);
        // Points in the closet are in the closet, not the room around it.
        let inside = Point::new(80.0, 58.0);
        assert!(closet.contains(inside) && !outer.contains(inside));
        assert!(outer.contains(Point::new(30.0, 30.0)));
        // A name anchored in the closet belongs to the closet.
        let names = vec![RoomName::new(inside, "Closet", "Closet")];
        assert!(closet.name_entry(&names).is_some());
        assert!(outer.name_entry(&names).is_none());
    }

    #[test]
    fn neighbouring_rooms_are_not_nested() {
        let mut walls = box_walls(4.5);
        walls.push(wall(5, 120.0, 0.0, 120.0, 120.0));
        let rooms = detect_rooms(&walls, 0.5);
        assert_eq!(rooms.len(), 2);
        assert!(rooms.iter().all(|r| r.holes.is_empty()));
        assert!((rooms.iter().map(|r| r.area_sq_in).sum::<f64>() - 240.0 * 120.0).abs() < 1e-6);
    }

    #[test]
    fn function_defaults_follow_chief() {
        let g = function_defaults("Garage", "Garage");
        assert_eq!(g.floor_height_offset, -GARAGE_FLOOR_DROP);
        assert_eq!(g.floor_finish_thickness, Some(0.0));
        assert!(g.has_floor && g.has_ceiling);
        for f in ["Deck", "Balcony", "Court"] {
            let d = function_defaults(f, f);
            assert!(d.has_floor && !d.has_ceiling, "{f}");
            assert!(!d.roof_over && !d.flat_ceiling, "{f}: open to the outside");
            assert!(!d.floor_structure.is_empty());
        }
        // A porch is hybrid: it has a ceiling and a roof over it (p. 446).
        let porch = function_defaults("Porch", "Porch");
        assert!(porch.has_floor && porch.has_ceiling && porch.roof_over);
        assert!(!porch.floor_structure.is_empty());
        assert!(!function_defaults("Open Below", "Open Below").has_floor);
        let roof = function_defaults("Flat Roof", "Flat Roof");
        assert!(
            roof.has_floor && !roof.has_ceiling,
            "a roof deck has no ceiling"
        );
        assert!(!roof.floor_structure.is_empty());
        assert!(!function_defaults("Utility", "Attic").has_floor);
        let court = function_defaults("Standard", "Courtyard");
        assert!(!court.has_floor && !court.has_ceiling, "open to the sky");
        let attic = function_defaults("Attic", "Attic");
        assert!(
            !attic.has_ceiling && !attic.roof_over,
            "ignored by the roof generator"
        );
        assert_eq!(
            function_defaults("Standard", "Bath"),
            FunctionDefaults::default()
        );
        let mut name = RoomName::new(Point::ZERO, "Garage", "Garage");
        apply_function_defaults(&mut name, &g, 0.75);
        assert_eq!(name.floor_height_offset, -24.0);
        let misc = name.misc.clone().unwrap();
        assert_eq!(misc.floor_finish_thickness, 0.0);
        assert_eq!(
            crate::extras::structure_thickness(&misc.floor_structure),
            4.0
        );
        // Back to a plain room: the floor's own finish returns.
        apply_function_defaults(&mut name, &FunctionDefaults::default(), 0.75);
        assert_eq!(name.floor_height_offset, 0.0);
        assert_eq!(name.misc.unwrap().floor_finish_thickness, 0.75);
    }

    #[test]
    fn four_walls_make_one_room() {
        // 20' x 10' box.
        let walls = vec![
            wall(1, 0.0, 0.0, 240.0, 0.0),
            wall(2, 240.0, 0.0, 240.0, 120.0),
            wall(3, 240.0, 120.0, 0.0, 120.0),
            wall(4, 0.0, 120.0, 0.0, 0.0),
        ];
        let rooms = detect_rooms(&walls, 0.5);
        assert_eq!(rooms.len(), 1);
        assert!((rooms[0].area_sq_ft() - 200.0).abs() < 1e-6);
    }

    #[test]
    fn t_junction_splits_into_two_rooms() {
        let walls = vec![
            wall(1, 0.0, 0.0, 240.0, 0.0),
            wall(2, 240.0, 0.0, 240.0, 120.0),
            wall(3, 240.0, 120.0, 0.0, 120.0),
            wall(4, 0.0, 120.0, 0.0, 0.0),
            // Partition ending on the top and bottom walls (two T-junctions).
            wall(5, 120.0, 0.0, 120.0, 120.0),
        ];
        let rooms = detect_rooms(&walls, 0.5);
        assert_eq!(rooms.len(), 2);
        for r in &rooms {
            assert!(
                (r.area_sq_ft() - 100.0).abs() < 1e-6,
                "got {}",
                r.area_sq_ft()
            );
        }
    }

    #[test]
    fn dangling_wall_makes_no_room() {
        let walls = vec![
            wall(1, 0.0, 0.0, 100.0, 0.0),
            wall(2, 100.0, 0.0, 100.0, 80.0),
        ];
        assert!(detect_rooms(&walls, 0.5).is_empty());
    }

    #[test]
    fn crossing_walls_make_four_rooms() {
        let walls = vec![
            wall(1, 0.0, 0.0, 240.0, 0.0),
            wall(2, 240.0, 0.0, 240.0, 240.0),
            wall(3, 240.0, 240.0, 0.0, 240.0),
            wall(4, 0.0, 240.0, 0.0, 0.0),
            wall(5, 120.0, 0.0, 120.0, 240.0),
            wall(6, 0.0, 120.0, 240.0, 120.0),
        ];
        let rooms = detect_rooms(&walls, 0.5);
        assert_eq!(rooms.len(), 4);
    }

    #[test]
    fn inner_polygon_is_offset_by_half_thickness() {
        let rooms = detect_rooms_inner(&box_walls(6.0), 0.5);
        assert_eq!(rooms.len(), 1);
        let r = &rooms[0];
        // Centerline values are unchanged.
        assert!((r.area_sq_ft() - 200.0).abs() < 1e-6);
        assert_eq!(r.centerline_polygon().len(), r.polygon.len());
        let expect = (240.0 - 6.0) * (120.0 - 6.0) / 144.0;
        assert!(
            (r.interior_area_sq_ft() - expect).abs() < 1e-6,
            "{}",
            r.interior_area_sq_ft()
        );
        assert!((r.interior_area() - expect).abs() < 1e-6);
        assert!(r
            .inner_polygon
            .iter()
            .all(|p| p.x >= 3.0 - 1e-9 && p.x <= 237.0 + 1e-9));
        // Standard area: outside of the exterior walls.
        let std = (240.0 + 6.0) * (120.0 + 6.0) / 144.0;
        assert!((r.standard_area_sq_ft() - std).abs() < 1e-6);
        // detect_rooms fills the same fields.
        let r2 = &detect_rooms(&box_walls(6.0), 0.5)[0];
        assert!((r2.interior_area_sq_ft() - expect).abs() < 1e-6);
    }

    #[test]
    fn inner_polygon_with_partition_and_mixed_thickness() {
        let mut walls = box_walls(6.0);
        walls.push(wall_t(5, 120.0, 0.0, 120.0, 120.0, 4.0, WallKind::Interior));
        let rooms = detect_rooms_inner(&walls, 0.5);
        assert_eq!(rooms.len(), 2);
        for r in &rooms {
            // 120 wide centerline: 3 (outer) + 2 (partition) off, 120 tall: 3 + 3 off.
            let expect = (120.0 - 3.0 - 2.0) * (120.0 - 6.0) / 144.0;
            assert!(
                (r.interior_area_sq_ft() - expect).abs() < 1e-6,
                "{}",
                r.interior_area_sq_ft()
            );
            // Standard: out 3 on three sides, centre of the partition.
            let std = (120.0 + 3.0) * (120.0 + 6.0) / 144.0;
            assert!((r.standard_area_sq_ft() - std).abs() < 1e-6);
        }
    }

    #[test]
    fn flags_control_room_definition() {
        let mut walls = box_walls(6.0);
        let mut div = wall_t(5, 120.0, 0.0, 120.0, 120.0, 4.0, WallKind::Interior);
        div.flags.room_divider = true;
        div.flags.invisible = true;
        walls.push(div);
        assert_eq!(detect_rooms_inner(&walls, 0.5).len(), 2);
        // An invisible wall that is not a divider does not split rooms.
        walls[4].flags.room_divider = false;
        assert_eq!(detect_rooms_inner(&walls, 0.5).len(), 1);
        walls[4].flags.invisible = false;
        walls[4].flags.no_room_definition = true;
        assert_eq!(detect_rooms_inner(&walls, 0.5).len(), 1);
        // A divider overrides no_room_definition.
        walls[4].flags.room_divider = true;
        assert_eq!(detect_rooms_inner(&walls, 0.5).len(), 2);
    }

    #[test]
    fn curved_wall_bounds_a_room() {
        // A D-shaped room: straight chord closed by a semicircular wall.
        let mut walls = vec![wall(1, 0.0, 0.0, 100.0, 0.0)];
        let mut arc = wall(2, 100.0, 0.0, 0.0, 0.0);
        arc.curve = Some(crate::walls::WallCurve { bulge: -50.0 });
        walls.push(arc);
        let rooms = detect_rooms_inner(&walls, 0.5);
        assert_eq!(rooms.len(), 1);
        let half_disc = std::f64::consts::PI * 50.0 * 50.0 / 2.0;
        assert!((rooms[0].area_sq_in - half_disc).abs() / half_disc < 0.02);
        assert!(rooms[0].interior_area_sq_in < rooms[0].area_sq_in);
    }

    #[test]
    fn living_area_uses_inner_area_and_type_flags() {
        let walls = {
            let mut w = box_walls(6.0);
            w.push(wall_t(5, 120.0, 0.0, 120.0, 120.0, 6.0, WallKind::Interior));
            w
        };
        let rooms = detect_rooms_inner(&walls, 0.5);
        let d = crate::defaults::PlanDefaults::chief_x18_daniel();
        let mut p = Project::new("l");
        let both = p.living_area_sq_ft(0, &rooms, &d.rooms.room_types);
        let one = rooms[0].interior_area_sq_ft();
        assert!((both - 2.0 * one).abs() < 1e-6);
        // Name the left room Garage: excluded.
        let left = rooms.iter().find(|r| r.centroid.x < 120.0).unwrap();
        p.set_room_name(0, left.centroid, "Garage", "Garage", &rooms);
        let after = p.living_area_sq_ft(0, &rooms, &d.rooms.room_types);
        assert!((after - one).abs() < 1e-6, "{after} vs {one}");
        // Per-room override wins over the type default.
        p.floors[0].room_names[0].include_in_living_area = Some(true);
        let over = p.living_area_sq_ft(0, &rooms, &d.rooms.room_types);
        assert!((over - 2.0 * one).abs() < 1e-6);
    }

    /// A messy grid: gaps, short walls, diagonals, Ts, mixed thickness.
    fn messy_plan(n: usize, seed: u64) -> Vec<Wall> {
        let mut s = seed;
        let mut rnd = move || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut walls = Vec::new();
        let mut id = 0;
        let mut add = |walls: &mut Vec<Wall>, a: (f64, f64), b: (f64, f64), t: f64, k| {
            id += 1;
            walls.push(wall_t(id, a.0, a.1, b.0, b.1, t, k));
        };
        for r in 0..=n {
            for c in 0..n {
                if rnd() < 0.15 {
                    continue;
                }
                let (x, y) = (c as f64 * 120.0, r as f64 * 120.0);
                let len = if rnd() < 0.1 { 60.0 } else { 120.0 };
                let k = if r == 0 || r == n {
                    WallKind::Exterior
                } else {
                    WallKind::Interior
                };
                add(
                    &mut walls,
                    (x, y),
                    (x + len, y),
                    [4.5, 6.0, 7.625][(r + c) % 3],
                    k,
                );
            }
        }
        for c in 0..=n {
            for r in 0..n {
                if rnd() < 0.15 {
                    continue;
                }
                let (x, y) = (c as f64 * 120.0, r as f64 * 120.0);
                add(&mut walls, (x, y), (x, y + 120.0), 4.5, WallKind::Interior);
            }
        }
        for _ in 0..n {
            let x = (rnd() * n as f64).floor() * 120.0;
            let y = (rnd() * n as f64).floor() * 120.0;
            add(
                &mut walls,
                (x, y),
                (x + 120.0, y + 120.0),
                4.5,
                WallKind::Interior,
            );
            add(
                &mut walls,
                (x + 60.0, y),
                (x + 60.0, y + 120.0),
                4.5,
                WallKind::Interior,
            );
        }
        walls
    }

    #[test]
    fn the_position_index_gives_the_rooms_a_scan_gives() {
        for (n, seed) in [(5, 7), (9, 8), (13, 9)] {
            let walls = messy_plan(n, seed);
            assert!(walls.len() > GRID_MIN_WALLS, "{} walls", walls.len());
            let fast = format!("{:?}", detect_rooms_with(&walls, 0.5, true));
            let scan = format!("{:?}", detect_rooms_with(&walls, 0.5, false));
            assert_eq!(fast, scan, "plan {n}");
            assert!(fast.len() > 100);
        }
    }

    fn box_floor(t: f64) -> Floor {
        let mut f = Floor::new("1st Floor", 0.0);
        f.walls = box_walls(t);
        f
    }

    #[test]
    fn basement_and_crawl_space_functions_set_their_platforms() {
        let b = function_defaults("Basement", "Basement");
        assert!(b.has_floor && b.has_ceiling);
        assert_eq!(b.floor_height_offset, SLAB_FLOOR_THICKNESS);
        assert_eq!(b.floor_finish_thickness, Some(0.0));
        assert_eq!(
            crate::extras::structure_thickness(&b.floor_structure),
            SLAB_FLOOR_THICKNESS
        );
        // The room type's name works without the function (the template's
        // Crawl Space type has the Utility function).
        let c = function_defaults("Utility", "Crawl Space");
        assert!(!c.has_floor && !c.has_ceiling);
        assert!(function_defaults("Crawl Space", "Whatever")
            .floor_structure
            .is_empty());
        assert!(function_defaults("Utility", "Storage").has_floor);
    }

    #[test]
    fn chiefs_room_types_keep_their_living_area_defaults() {
        let d = crate::defaults::PlanDefaults::chief_x18_daniel();
        let types = &d.rooms.room_types;
        let living = |n: &str| {
            types
                .iter()
                .find(|t| t.name == n)
                .unwrap_or_else(|| panic!("no room type {n}"))
                .include_in_living_area
        };
        for n in [
            "Bedroom",
            "Bonus Room",
            "Closet",
            "Dining Room",
            "Dressing Room",
            "Entry",
            "Family Room",
            "Foyer",
            "Kitchen",
            "Living",
            "Master Bath",
        ] {
            assert!(living(n), "{n} counts as living area");
        }
        for n in [
            "Courtyard",
            "Crawl Space",
            "Deck",
            "Flat Roof",
            "Garage",
            "Porch",
        ] {
            assert!(!living(n), "{n} is left out of the living area");
        }
        assert!(types.iter().any(|t| t.name == "Utility"));
        assert!(types.iter().any(|t| t.name == "Dinette"));
    }

    #[test]
    fn the_molding_library_has_clean_profiles_of_each_kind() {
        for kind in [MoldingKind::Base, MoldingKind::Crown, MoldingKind::Chair] {
            assert!(molding_defs(kind).len() >= 2, "{kind:?}");
        }
        for m in MOLDING_LIBRARY.iter() {
            let pts = m.points();
            assert!(polygon_area(&pts) > 0.0, "{} is counter-clockwise", m.name);
            assert!(m.height() > 1.0 && m.projection() > 0.2, "{}", m.name);
            // A simple outline: no two edges cross.
            let n = pts.len();
            for i in 0..n {
                for j in i + 2..n {
                    if i == 0 && j == n - 1 {
                        continue;
                    }
                    let (a, b) = (pts[i], pts[(i + 1) % n]);
                    let (c, d) = (pts[j], pts[(j + 1) % n]);
                    assert!(
                        segment_intersection(a, b, c, d).is_none(),
                        "{} edges {i} and {j} cross",
                        m.name
                    );
                }
            }
            assert_eq!(molding_def(m.name), Some(m));
        }
        assert_eq!(
            molding_def("crown - cove 3 5/8").map(|m| m.kind),
            Some(MoldingKind::Crown)
        );
        assert!(molding_def("No such profile").is_none());
        assert_eq!(molding_span(MoldingKind::Base, 5.25, 108.0), (0.0, 5.25));
        assert_eq!(molding_span(MoldingKind::Crown, 3.5, 108.0), (104.5, 108.0));
        assert_eq!(
            molding_span(MoldingKind::Chair, 3.0, 108.0),
            (CHAIR_RAIL_HEIGHT, CHAIR_RAIL_HEIGHT + 3.0)
        );
    }

    #[test]
    fn a_molding_without_openings_is_one_closed_run_around_the_room() {
        let f = box_floor(6.0);
        let room = detect_rooms(&f.walls, 0.5).remove(0);
        let runs = room.molding_runs(&f, 0.0, 5.0);
        assert_eq!(runs.len(), 1);
        let r = &runs[0];
        assert!(r[0].dist(r[r.len() - 1]) < 1e-6, "closed");
        assert_eq!(r.len(), 5, "four corners and the repeat");
        // Along the interior surfaces (3" in from the centerlines).
        assert!(r
            .iter()
            .all(|p| p.x > 2.9 && p.x < 237.1 && p.y > 2.9 && p.y < 117.1));
    }

    #[test]
    fn a_door_breaks_the_base_but_not_the_crown() {
        use crate::model::{Opening, OpeningKind};
        let mut f = box_floor(6.0);
        // A 36" door in the bottom wall (id 1), centered 100" along it.
        f.openings
            .push(Opening::new(1, 100.0, OpeningKind::Door, 36.0, 80.0, 0.0));
        let room = detect_rooms(&f.walls, 0.5).remove(0);
        let base = room.molding_runs(&f, 0.0, 5.0);
        assert_eq!(
            base.len(),
            1,
            "one open run: the base stops either side of the door"
        );
        let r = &base[0];
        assert!(r[0].dist(r[r.len() - 1]) > 1.0, "open, not closed");
        // The gap is the door's 36" (82..118 along the wall).
        let ends = [r[0], r[r.len() - 1]];
        let xs: Vec<f64> = ends.iter().map(|p| p.x).collect();
        assert!(xs.iter().any(|x| (x - 118.0).abs() < 0.01), "{xs:?}");
        assert!(xs.iter().any(|x| (x - 82.0).abs() < 0.01), "{xs:?}");
        // Crown at the ceiling runs over the door.
        let crown = room.molding_runs(&f, 104.5, 108.0);
        assert_eq!(crown.len(), 1);
        assert!(crown[0][0].dist(crown[0][crown[0].len() - 1]) < 1e-6);
        // A window with a 36" sill leaves the base alone, but stops a chair
        // rail at 32".."35".
        f.openings.clear();
        f.openings.push(Opening::new(
            1,
            100.0,
            OpeningKind::Window,
            36.0,
            48.0,
            36.0,
        ));
        assert!(
            room.molding_runs(&f, 0.0, 5.0)[0][0].dist(room.molding_runs(&f, 0.0, 5.0)[0][4])
                < 1e-6
        );
        let chair = room.molding_runs(&f, 32.0, 35.0);
        assert!(chair.len() == 1 && chair[0][0].dist(chair[0][chair[0].len() - 1]) < 1e-6);
        // Two doors split the base into two runs.
        f.openings.clear();
        f.openings
            .push(Opening::new(1, 60.0, OpeningKind::Door, 36.0, 80.0, 0.0));
        f.openings
            .push(Opening::new(3, 120.0, OpeningKind::Door, 30.0, 80.0, 0.0));
        let two = room.molding_runs(&f, 0.0, 5.0);
        assert_eq!(two.len(), 2, "{two:?}");
    }

    #[test]
    fn open_below_covers_only_rooms_wholly_inside() {
        let hole = vec![
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 120.0),
            Point::new(0.0, 120.0),
        ];
        let same = hole.clone();
        let inner = vec![
            Point::new(10.0, 10.0),
            Point::new(100.0, 10.0),
            Point::new(100.0, 100.0),
            Point::new(10.0, 100.0),
        ];
        let sticking_out = vec![
            Point::new(100.0, 10.0),
            Point::new(300.0, 10.0),
            Point::new(300.0, 100.0),
            Point::new(100.0, 100.0),
        ];
        let bigger = vec![
            Point::new(-50.0, -50.0),
            Point::new(300.0, -50.0),
            Point::new(300.0, 200.0),
            Point::new(-50.0, 200.0),
        ];
        assert!(polygon_inside(&same, &hole, 1.0));
        assert!(polygon_inside(&inner, &hole, 1.0));
        assert!(!polygon_inside(&sticking_out, &hole, 1.0));
        assert!(!polygon_inside(&bigger, &hole, 1.0));
        assert!(!polygon_inside(&[], &hole, 1.0));
    }

    #[test]
    fn a_label_can_sit_near_a_wall_and_stays_inside_the_room() {
        let room = detect_rooms(&box_walls(6.0), 0.5).remove(0);
        let c = room.label_point(LabelPlacement::Center, 12.0);
        assert!(c.dist(Point::new(120.0, 60.0)) < 1.0);
        let top = room.label_point(LabelPlacement::Top, 12.0);
        let bottom = room.label_point(LabelPlacement::Bottom, 12.0);
        let left = room.label_point(LabelPlacement::Left, 12.0);
        let right = room.label_point(LabelPlacement::Right, 12.0);
        assert!(top.y > c.y && bottom.y < c.y && left.x < c.x && right.x > c.x);
        assert!((top.y - (117.0 - 12.0)).abs() < 0.01);
        for p in [top, bottom, left, right] {
            assert!(room.contains(p));
        }
        // An inset bigger than the room falls back toward the middle.
        let deep = room.label_point(LabelPlacement::Top, 200.0);
        assert!(room.contains(deep));
        assert_eq!(LabelPlacement::ALL.len(), 5);
        assert_eq!(
            RoomLabelStyle::default().style_name(),
            ROOM_LABEL_TEXT_STYLE
        );
        let own = RoomLabelStyle {
            text_style: "Schedule Style".into(),
            ..RoomLabelStyle::default()
        };
        assert_eq!(own.style_name(), "Schedule Style");
    }

    #[test]
    fn room_slab_and_label_style_survive_a_round_trip() {
        let mut n = RoomName::new(Point::new(1.0, 2.0), "Den", "Den");
        n.monolithic_slab = Some(RoomSlab {
            thickness: 5.0,
            stem_height: 14.0,
        });
        n.label_style = RoomLabelStyle {
            text_style: "Schedule Style".into(),
            placement: LabelPlacement::Bottom,
        };
        let json = serde_json::to_string(&n).unwrap();
        let back: RoomName = serde_json::from_str(&json).unwrap();
        assert_eq!(back, n);
        // Older files have neither.
        let old = r#"{"anchor":{"x":0.0,"y":0.0},"name":"A","room_type":"B"}"#;
        let old: RoomName = serde_json::from_str(old).unwrap();
        assert!(old.monolithic_slab.is_none());
        assert_eq!(old.label_style, RoomLabelStyle::default());
    }

    #[test]
    fn node_index_returns_the_first_node_within_tolerance() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(0.3, 0.0),
            Point::new(0.6, 0.0),
            Point::new(-0.4, 0.4),
            Point::new(10.0, 10.0),
            Point::new(10.2, 10.1),
            Point::new(0.5, 0.0),
        ];
        let (mut a, mut b) = (NodeIndex::new(0.5, true), NodeIndex::new(0.5, false));
        for p in pts {
            assert_eq!(a.index(p), b.index(p), "{p:?}");
        }
        assert_eq!(a.points, b.points);
    }

    // ----- Round 16, brief 15: room functions as property sets -----

    #[test]
    fn the_three_categories_hold_the_manuals_functions() {
        let of = |c: FunctionClass| -> Vec<&str> {
            ROOM_FUNCTIONS
                .iter()
                .filter(|(_, k)| *k == c)
                .map(|(n, _)| *n)
                .collect()
        };
        assert_eq!(of(FunctionClass::Exterior), ["Balcony", "Court", "Deck"]);
        assert_eq!(
            of(FunctionClass::Hybrid),
            ["Attic", "Garage", "Open Below", "Porch", "Slab"]
        );
        assert_eq!(of(FunctionClass::Interior), ["Standard", "Utility"]);
        // Basement and Crawl Space are not functions (DECISIONS 300).
        assert!(!function_names().contains(&"Basement"));
        assert!(!function_names().contains(&"Crawl Space"));
        assert_eq!(canonical_function("Crawl Space"), "Open Below");
        assert_eq!(canonical_function("Basement"), "Standard");
        assert_eq!(
            function_class("Utility", "Crawl Space"),
            FunctionClass::Hybrid
        );
        assert_eq!(
            function_class("Standard", "Courtyard"),
            FunctionClass::Exterior
        );
    }

    #[test]
    fn living_and_conditioned_defaults_follow_the_function() {
        for (f, _) in ROOM_FUNCTIONS {
            let d = function_defaults(f, f);
            let interior = d.class == FunctionClass::Interior;
            assert_eq!(
                d.living_area, interior,
                "{f}: only interior rooms are living"
            );
            assert_eq!(
                d.conditioned,
                interior || f == "Open Below",
                "{f}: interior and Open Below are conditioned"
            );
        }
    }

    #[test]
    fn ceilings_and_roofs_follow_the_function() {
        let d = |f: &str| function_defaults(f, f);
        // Interior rooms have a flat ceiling and a roof above them.
        assert!(d("Standard").has_ceiling && d("Standard").flat_ceiling && d("Standard").roof_over);
        // Exterior rooms are open to the outside.
        for f in ["Balcony", "Court", "Deck"] {
            assert!(
                !d(f).has_ceiling && !d(f).roof_over && !d(f).flat_ceiling,
                "{f}"
            );
            assert!(!d(f).build_foundation && d(f).faces_outside, "{f}");
        }
        // Attics get no ceiling and are ignored by the roof generator.
        assert!(!d("Attic").has_ceiling && !d("Attic").roof_over);
        // Garage, Slab and Porch are exterior-like but generate a ceiling and
        // a roof by default.
        for f in ["Garage", "Slab", "Porch"] {
            assert!(d(f).has_ceiling && d(f).roof_over, "{f}");
            assert_ne!(d(f).class, FunctionClass::Exterior);
        }
        // Open Below has no floor platform.
        assert!(!d("Open Below").has_floor && d("Open Below").has_ceiling);
        // Applying the defaults sets Roof Over, Flat Ceiling and Build
        // Foundation Below on the room.
        let mut n = RoomName::new(Point::ZERO, "Deck", "Deck");
        apply_function_defaults(&mut n, &d("Deck"), 0.75);
        assert!(!n.has_ceiling && !n.flat_ceiling && !n.options.build_foundation_below);
        assert!(!n.misc.as_ref().unwrap().roof_over);
        apply_function_defaults(&mut n, &FunctionDefaults::default(), 0.75);
        assert!(n.has_ceiling && n.flat_ceiling && n.options.build_foundation_below);
        assert!(n.misc.unwrap().roof_over);
    }

    #[test]
    fn floors_and_foundations_follow_the_function() {
        // Garage and Slab rooms drop to Floor 0 with a concrete slab.
        let g = function_defaults("Garage", "Garage");
        assert_eq!(g.floor_height_offset, -GARAGE_FLOOR_DROP);
        // A Slab's floor platform is as thick as the Foundation Defaults' slab.
        let s = function_defaults_with("Slab", "Slab", 6.0);
        assert_eq!(s.floor_height_offset, -GARAGE_FLOOR_DROP);
        assert_eq!(crate::extras::structure_thickness(&s.floor_structure), 6.0);
        let s4 = function_defaults("Slab", "Slab");
        assert_eq!(
            crate::extras::structure_thickness(&s4.floor_structure),
            SLAB_FLOOR_THICKNESS
        );
        // A Deck makes no foundation.
        assert!(!function_defaults("Deck", "Deck").build_foundation);
        assert!(function_defaults("Garage", "Garage").build_foundation);
        // A stairwell or crawl space is an Open Below room: no floor.
        assert!(!function_defaults("Open Below", "Stairwell").has_floor);
        assert!(!function_defaults("Standard", "Crawl Space").has_floor);
    }

    #[test]
    fn electrical_rules_follow_the_function_and_the_type() {
        let e = |f: &str, t: &str| electrical_rules(f, t);
        // Fixtures on the wall of an exterior room are weatherproof.
        assert!(e("Deck", "Deck").weatherproof && e("Court", "Courtyard").weatherproof);
        assert!(!e("Porch", "Porch").weatherproof, "a porch is hybrid");
        // Auto Place Outlets: none in exterior rooms, porches, Open Below.
        for (f, t) in [
            ("Deck", "Deck"),
            ("Balcony", "Balcony"),
            ("Porch", "Porch"),
            ("Open Below", "Open Below"),
        ] {
            assert_eq!(e(f, t).outlets, OutletPlacement::None, "{f}");
        }
        // Fewer in other hybrid rooms, all around in interior ones.
        assert_eq!(e("Garage", "Garage").outlets, OutletPlacement::Fewer);
        assert_eq!(e("Slab", "Slab").outlets, OutletPlacement::Fewer);
        assert_eq!(e("Standard", "Bedroom").outlets, OutletPlacement::Full);
        // GFCI over base cabinets in kitchens and baths; kitchens also get
        // standard-height outlets, baths do not.
        assert!(e("Standard", "Kitchen").gfci_over_base_cabinets);
        assert!(e("Standard", "Master Bath").gfci_over_base_cabinets);
        assert!(!e("Standard", "Bedroom").gfci_over_base_cabinets);
        assert!(e("Standard", "Kitchen").standard_height_outlets);
        assert!(!e("Standard", "Bath").standard_height_outlets);
        // Plan Check applies the habitable rules to interior rooms only.
        assert!(plan_check_rules("Standard", "Bedroom").habitable_rules);
        assert!(!plan_check_rules("Deck", "Deck").habitable_rules);
        assert!(!plan_check_rules("Garage", "Garage").habitable_rules);
    }

    #[test]
    fn a_door_or_window_between_exterior_and_interior_rooms_faces_outside() {
        let bed = ("Standard", "Bedroom");
        let deck = ("Deck", "Deck");
        assert!(opening_faces_outside(bed, deck));
        assert!(opening_faces_outside(deck, bed));
        assert!(!opening_faces_outside(bed, bed));
        // Open Below and the other hybrid rooms are treated as interior.
        assert!(!opening_faces_outside(bed, ("Open Below", "Open Below")));
        assert!(!opening_faces_outside(bed, ("Garage", "Garage")));
        assert!(!opening_faces_outside(bed, ("Porch", "Porch")));
    }

    #[test]
    fn a_room_type_spec_overrides_the_room_it_is_given_to() {
        use crate::assemblies::{Assembly, AssemblyKind, AssemblySlot};
        let mut spec = RoomTypeSpec::default();
        assert!(spec.is_default());
        spec.assemblies.set(
            AssemblyKind::FloorFinish,
            AssemblySlot::Own(Assembly::default()),
        );
        spec.layer = "CAD, Default".into();
        spec.drawing_group = "Sleeping".into();
        spec.moldings = vec![crate::extras::MoldingRef {
            kind: crate::extras::MoldingKind::Base,
            profile: "Base - Colonial 5 1/4".into(),
            height: 5.25,
        }];
        spec.fill = Some(crate::extras::RoomFill {
            color: [1, 2, 3],
            pattern: "Solid".into(),
            alpha: 0.5,
        });
        spec.label = Some(crate::extras::RoomLabelOptions {
            show_dimensions: true,
            ..Default::default()
        });
        assert!(!spec.is_default());
        let mut n = RoomName::new(Point::ZERO, "Bedroom", "Bedroom");
        n.label.offset = Point::new(5.0, 6.0);
        n.fill_style = None;
        apply_type_spec(&mut n, &spec);
        assert_eq!(n.options.layer, "CAD, Default");
        assert_eq!(n.options.layer_name(), "CAD, Default");
        assert_eq!(n.options.drawing_group, "Sleeping");
        assert_eq!(n.moldings.len(), 1);
        assert!(n.fill_style.is_some());
        assert!(n.label.show_dimensions);
        assert_eq!(
            n.label.offset,
            Point::new(5.0, 6.0),
            "the position is the room's"
        );
        assert!(matches!(
            n.misc
                .as_ref()
                .unwrap()
                .assemblies
                .slot(AssemblyKind::FloorFinish),
            AssemblySlot::Own(_)
        ));
        // The layer name defaults to Rooms.
        assert_eq!(RoomOptions::default().layer_name(), ROOM_LAYER);
    }

    #[test]
    fn room_options_stay_out_of_the_file_until_they_are_set() {
        let mut n = RoomName::new(Point::ZERO, "Garage", "Garage");
        let json = serde_json::to_string(&n).unwrap();
        assert!(!json.contains("options"), "{json}");
        n.options.supplies_floor_above = true;
        n.options.pour_number = 3;
        let json = serde_json::to_string(&n).unwrap();
        let back: RoomName = serde_json::from_str(&json).unwrap();
        assert!(back.options.supplies_floor_above && back.options.build_foundation_below);
        assert_eq!(back.options.pour_number, 3);
        // A room from an older file has the defaults.
        let old: RoomName =
            serde_json::from_str(r#"{"anchor":{"x":1.0,"y":2.0},"name":"A","room_type":"B"}"#)
                .unwrap();
        assert!(old.options.is_default() && old.flat_ceiling);
    }
}
