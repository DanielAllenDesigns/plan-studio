//! Framing Types, Default Framing Members and Roles (manual pp. 885-889), the
//! Manual Framing Defaults (pp. 901-904) and the framing fields of the
//! Automatic Framing Defaults dialog that no other type holds (pp. 892-900).
//!
//! * A [`FramingType`] sets a member's composition and cross-section shape
//!   (lumber, I-joist, glulam, LVL, PSL, VSL, steel I / box / C / U channel,
//!   concrete rectangular / round), the Supporting Type of Steel C, whether
//!   U.S. plans list nominal sizes, and whether the name appears in labels.
//! * A [`FramingMemberDef`] ("Default Framing Member") names a type, a material
//!   and a [`Role`] but no size, so one definition serves several uses; it also
//!   carries the Materials List category (Auto, or one of Framing, Subfloor,
//!   Roofing and Decks-Walks).
//! * The *construction* of each Role in automatic framing is the definition the
//!   Automatic Framing Defaults dialog picks for it
//!   ([`FramingCatalog::constructions`]); a member's type is its own
//!   `framing_type` when it has one (Apply Framing Default Properties), else the
//!   type of the definition for its Role.
//!
//! The whole [`FramingCatalog`] is one single-key object in the first floor's
//! `framing` slot (the same place as the build settings), so every reader that
//! does not know it skips it; [`load`] and [`store`] read and write it.

use crate::manual::{FramingMaterial, FramingMember, MemberKind as ManualKind};
use crate::member::{Member, MemberKind, SectionShape};
use crate::reporting::ReportingSet;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Key of the single-key object that holds the catalog.
pub const CATALOG_KEY: &str = "FramingCatalog";

// ----- categories -----

/// The Materials List categories of framing (manual p. 932).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MaterialsCategory {
    /// Framing (F): all wall framing, posts and General Framing.
    Framing,
    /// Subfloor (SF): floor and ceiling joists and sheathing.
    Subfloor,
    /// Roofing (R): trusses, rafters and sheathing.
    Roofing,
    /// Decks-Walks (DW): posts, beams, joists and planking of Deck rooms.
    DecksWalks,
}

impl MaterialsCategory {
    pub const ALL: [MaterialsCategory; 4] = [
        MaterialsCategory::Framing,
        MaterialsCategory::Subfloor,
        MaterialsCategory::Roofing,
        MaterialsCategory::DecksWalks,
    ];

    pub fn name(self) -> &'static str {
        match self {
            MaterialsCategory::Framing => "Framing",
            MaterialsCategory::Subfloor => "Subfloor",
            MaterialsCategory::Roofing => "Roofing",
            MaterialsCategory::DecksWalks => "Decks-Walks",
        }
    }

    /// Chief's one- or two-letter code.
    pub fn code(self) -> &'static str {
        match self {
            MaterialsCategory::Framing => "F",
            MaterialsCategory::Subfloor => "SF",
            MaterialsCategory::Roofing => "R",
            MaterialsCategory::DecksWalks => "DW",
        }
    }
}

/// A definition's Materials List category: Auto Category (by Role) or one
/// chosen category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CategoryChoice {
    #[default]
    Auto,
    Fixed(MaterialsCategory),
}

impl CategoryChoice {
    pub fn name(self) -> &'static str {
        match self {
            CategoryChoice::Auto => "Auto Category",
            CategoryChoice::Fixed(c) => c.name(),
        }
    }
}

// ----- roles -----

/// What a framing member is for. A member's Role starts as its actual purpose
/// and position and can be changed; it only changes how the member is listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Role {
    Stud,
    KingStud,
    Trimmer,
    Cripple,
    Plate,
    Header,
    Sill,
    WallBlocking,
    FloorJoist,
    CeilingJoist,
    RimJoist,
    TrimmerJoist,
    HeaderJoist,
    JoistBlocking,
    Ledger,
    FloorBeam,
    Rafter,
    Ridge,
    Hip,
    Valley,
    Fascia,
    CollarTie,
    RoofBlocking,
    Purlin,
    RoofBeam,
    Truss,
    Post,
    GeneralFraming,
    DeckJoist,
    DeckBeam,
    DeckPost,
    DeckPlank,
}

impl Role {
    pub const ALL: [Role; 32] = [
        Role::Stud,
        Role::KingStud,
        Role::Trimmer,
        Role::Cripple,
        Role::Plate,
        Role::Header,
        Role::Sill,
        Role::WallBlocking,
        Role::FloorJoist,
        Role::CeilingJoist,
        Role::RimJoist,
        Role::TrimmerJoist,
        Role::HeaderJoist,
        Role::JoistBlocking,
        Role::Ledger,
        Role::FloorBeam,
        Role::Rafter,
        Role::Ridge,
        Role::Hip,
        Role::Valley,
        Role::Fascia,
        Role::CollarTie,
        Role::RoofBlocking,
        Role::Purlin,
        Role::RoofBeam,
        Role::Truss,
        Role::Post,
        Role::GeneralFraming,
        Role::DeckJoist,
        Role::DeckBeam,
        Role::DeckPost,
        Role::DeckPlank,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Role::Stud => "Stud",
            Role::KingStud => "King Stud",
            Role::Trimmer => "Trimmer",
            Role::Cripple => "Cripple",
            Role::Plate => "Plate",
            Role::Header => "Header",
            Role::Sill => "Sill",
            Role::WallBlocking => "Wall Blocking",
            Role::FloorJoist => "Floor Joist",
            Role::CeilingJoist => "Ceiling Joist",
            Role::RimJoist => "Rim Joist",
            Role::TrimmerJoist => "Trimmer Joist",
            Role::HeaderJoist => "Header Joist",
            Role::JoistBlocking => "Joist Blocking",
            Role::Ledger => "Ledger",
            Role::FloorBeam => "Floor/Ceiling Beam",
            Role::Rafter => "Rafter",
            Role::Ridge => "Ridge",
            Role::Hip => "Hip",
            Role::Valley => "Valley",
            Role::Fascia => "Fascia",
            Role::CollarTie => "Collar Tie",
            Role::RoofBlocking => "Roof Blocking",
            Role::Purlin => "Purlin",
            Role::RoofBeam => "Roof Beam",
            Role::Truss => "Truss",
            Role::Post => "Post",
            Role::GeneralFraming => "General Framing",
            Role::DeckJoist => "Deck Joist",
            Role::DeckBeam => "Deck Beam",
            Role::DeckPost => "Deck Post",
            Role::DeckPlank => "Deck Planking",
        }
    }

    /// The Role of an automatically generated member kind.
    pub fn of_member(kind: MemberKind) -> Role {
        match kind {
            MemberKind::Stud | MemberKind::CornerStud | MemberKind::TeeStud => Role::Stud,
            MemberKind::KingStud => Role::KingStud,
            MemberKind::TrimmerStud => Role::Trimmer,
            MemberKind::CrippleStud => Role::Cripple,
            MemberKind::TopPlate | MemberKind::BottomPlate => Role::Plate,
            MemberKind::Header => Role::Header,
            MemberKind::Sill => Role::Sill,
            MemberKind::RimJoist => Role::RimJoist,
            MemberKind::Joist => Role::FloorJoist,
            MemberKind::TrimmerJoist => Role::TrimmerJoist,
            MemberKind::HeaderJoist => Role::HeaderJoist,
            MemberKind::Blocking => Role::WallBlocking,
            MemberKind::Ledger => Role::Ledger,
            MemberKind::Rafter => Role::Rafter,
            MemberKind::Ridge => Role::Ridge,
            MemberKind::Hip => Role::Hip,
            MemberKind::Valley => Role::Valley,
            MemberKind::Fascia => Role::Fascia,
            MemberKind::CollarTie => Role::CollarTie,
            MemberKind::CeilingJoist => Role::CeilingJoist,
            MemberKind::TrussTopChord | MemberKind::TrussBottomChord | MemberKind::TrussWeb => {
                Role::Truss
            }
        }
    }

    /// The Role of a manually placed member; `None` for layout lines.
    pub fn of_manual(kind: ManualKind) -> Option<Role> {
        Some(match kind {
            ManualKind::GeneralFraming => Role::GeneralFraming,
            ManualKind::Post | ManualKind::PostWithFooting => Role::Post,
            ManualKind::Blocking | ManualKind::JoistBlocking => Role::JoistBlocking,
            ManualKind::Joist => Role::FloorJoist,
            ManualKind::FloorCeilingBeam => Role::FloorBeam,
            ManualKind::FloorCeilingTruss | ManualKind::RoofTruss | ManualKind::GirderTruss => {
                Role::Truss
            }
            ManualKind::Rafter => Role::Rafter,
            ManualKind::RoofBeam => Role::RoofBeam,
            ManualKind::RoofBlocking => Role::RoofBlocking,
            ManualKind::RoofPurlin => Role::Purlin,
            ManualKind::BearingLine | ManualKind::TrussBase => return None,
        })
    }

    /// The Materials List category of the Role under Auto Category.
    pub fn category(self) -> MaterialsCategory {
        match self {
            Role::Stud
            | Role::KingStud
            | Role::Trimmer
            | Role::Cripple
            | Role::Plate
            | Role::Header
            | Role::Sill
            | Role::WallBlocking
            | Role::Post
            | Role::GeneralFraming => MaterialsCategory::Framing,
            Role::FloorJoist
            | Role::CeilingJoist
            | Role::RimJoist
            | Role::TrimmerJoist
            | Role::HeaderJoist
            | Role::JoistBlocking
            | Role::Ledger
            | Role::FloorBeam => MaterialsCategory::Subfloor,
            Role::Rafter
            | Role::Ridge
            | Role::Hip
            | Role::Valley
            | Role::Fascia
            | Role::CollarTie
            | Role::RoofBlocking
            | Role::Purlin
            | Role::RoofBeam
            | Role::Truss => MaterialsCategory::Roofing,
            Role::DeckJoist | Role::DeckBeam | Role::DeckPost | Role::DeckPlank => {
                MaterialsCategory::DecksWalks
            }
        }
    }

    /// The name of the Default Framing Member a plan starts with for the Role.
    pub fn default_def(self) -> &'static str {
        match self {
            Role::Stud | Role::KingStud | Role::Trimmer | Role::Cripple => "Studs",
            Role::Plate | Role::Sill => "Plates",
            Role::Header => "Headers",
            Role::WallBlocking | Role::JoistBlocking | Role::RoofBlocking => "Blocking",
            Role::FloorJoist | Role::TrimmerJoist | Role::HeaderJoist => "Joists",
            Role::CeilingJoist => "Ceiling Joists",
            Role::RimJoist => "Rim Joists",
            Role::Ledger => "Ledgers",
            Role::FloorBeam => "Floor/Ceiling Beams",
            Role::Rafter | Role::Hip | Role::Valley | Role::CollarTie => "Rafters",
            Role::Ridge => "Ridge",
            Role::Fascia => "Fascia",
            Role::Purlin => "Purlins",
            Role::RoofBeam => "Roof Beams",
            Role::Truss => "Trusses",
            Role::Post => "Posts",
            Role::GeneralFraming => "General Framing",
            Role::DeckJoist => "Deck Joists",
            Role::DeckBeam => "Deck Beams",
            Role::DeckPost => "Deck Posts",
            Role::DeckPlank => "Deck Planking",
        }
    }
}

// ----- framing types -----

/// The four compositions of a Framing Type (manual p. 887).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Composition {
    Wood,
    Steel,
    Concrete,
    Other,
}

impl Composition {
    pub const ALL: [Composition; 4] = [
        Composition::Wood,
        Composition::Steel,
        Composition::Concrete,
        Composition::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Composition::Wood => "Wood",
            Composition::Steel => "Steel",
            Composition::Concrete => "Concrete",
            Composition::Other => "Other",
        }
    }

    /// The shapes the composition offers.
    pub fn shapes(self) -> &'static [FramingShape] {
        match self {
            Composition::Wood => &[
                FramingShape::Lumber,
                FramingShape::IJoist,
                FramingShape::Glulam,
                FramingShape::EngineeredLumber,
                FramingShape::Lvl,
                FramingShape::Psl,
                FramingShape::Vsl,
            ],
            Composition::Steel => &[
                FramingShape::SteelI,
                FramingShape::SteelBox,
                FramingShape::CChannel,
                FramingShape::UChannel,
            ],
            Composition::Concrete => &[FramingShape::Rectangular, FramingShape::Circular],
            Composition::Other => &[FramingShape::Rectangular],
        }
    }
}

/// The shape of a Framing Type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FramingShape {
    Lumber,
    IJoist,
    Glulam,
    EngineeredLumber,
    Lvl,
    Psl,
    Vsl,
    SteelI,
    SteelBox,
    CChannel,
    UChannel,
    Rectangular,
    Circular,
}

impl FramingShape {
    pub fn name(self) -> &'static str {
        match self {
            FramingShape::Lumber => "Lumber",
            FramingShape::IJoist => "I-Joist",
            FramingShape::Glulam => "Glulam",
            FramingShape::EngineeredLumber => "Engineered Lumber",
            FramingShape::Lvl => "LVL",
            FramingShape::Psl => "PSL",
            FramingShape::Vsl => "VSL",
            FramingShape::SteelI => "Steel I",
            FramingShape::SteelBox => "Steel Box",
            FramingShape::CChannel => "C Channel",
            FramingShape::UChannel => "U Channel",
            FramingShape::Rectangular => "Rectangular",
            FramingShape::Circular => "Circular",
        }
    }

    /// The 3D and section shape.
    pub fn section(self) -> SectionShape {
        match self {
            FramingShape::IJoist => SectionShape::IJoist,
            FramingShape::SteelI => SectionShape::SteelI,
            FramingShape::SteelBox => SectionShape::SteelBox,
            FramingShape::CChannel => SectionShape::CChannel,
            FramingShape::UChannel => SectionShape::UChannel,
            FramingShape::Circular => SectionShape::Round,
            _ => SectionShape::Box,
        }
    }

    /// The manual framing material the shape corresponds to.
    pub fn material(self) -> FramingMaterial {
        match self {
            FramingShape::Glulam => FramingMaterial::Glulam,
            FramingShape::Lvl | FramingShape::Vsl | FramingShape::EngineeredLumber => {
                FramingMaterial::Lvl
            }
            FramingShape::Psl => FramingMaterial::Psl,
            FramingShape::SteelI
            | FramingShape::SteelBox
            | FramingShape::CChannel
            | FramingShape::UChannel => FramingMaterial::Steel,
            _ => FramingMaterial::Lumber,
        }
    }
}

/// A Framing Type: composition and shape of a member (manual pp. 887-889).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FramingType {
    pub name: String,
    pub composition: Composition,
    pub shape: FramingShape,
    /// Steel C only: the type of the plates (a U Channel type).
    pub supporting_type: Option<String>,
    /// U.S. plans: the Materials List uses the nominal size, not the actual.
    pub display_nominal: bool,
    /// Include the type's name in labels and in the Materials List
    /// description.
    pub include_name_in_labels: bool,
}

impl Default for FramingType {
    fn default() -> Self {
        Self {
            name: "Lumber".into(),
            composition: Composition::Wood,
            shape: FramingShape::Lumber,
            supporting_type: None,
            display_nominal: true,
            include_name_in_labels: false,
        }
    }
}

impl FramingType {
    pub fn new(name: &str, composition: Composition, shape: FramingShape) -> Self {
        Self {
            name: name.into(),
            composition,
            shape,
            // Lumber lists its nominal size; the others their actual size.
            display_nominal: shape == FramingShape::Lumber,
            // Only the types that are not plain lumber say so in labels.
            include_name_in_labels: shape != FramingShape::Lumber,
            ..Self::default()
        }
    }

    /// Fixes what the composition does not allow: a shape the composition
    /// lacks becomes its first shape, and only Steel C keeps a Supporting
    /// Type.
    pub fn normalize(&mut self) {
        if !self.composition.shapes().contains(&self.shape) {
            self.shape = self.composition.shapes()[0];
        }
        if self.shape != FramingShape::CChannel {
            self.supporting_type = None;
        }
    }
}

/// The Framing Types a plan starts with.
pub fn default_types() -> Vec<FramingType> {
    let mut steel_c = FramingType::new("Steel C", Composition::Steel, FramingShape::CChannel);
    steel_c.supporting_type = Some("Steel U Channel".into());
    vec![
        FramingType::new("Lumber", Composition::Wood, FramingShape::Lumber),
        FramingType::new("I-Joist", Composition::Wood, FramingShape::IJoist),
        FramingType::new("Glulam", Composition::Wood, FramingShape::Glulam),
        FramingType::new(
            "Engineered Lumber",
            Composition::Wood,
            FramingShape::EngineeredLumber,
        ),
        FramingType::new("LVL", Composition::Wood, FramingShape::Lvl),
        FramingType::new("PSL", Composition::Wood, FramingShape::Psl),
        FramingType::new("VSL", Composition::Wood, FramingShape::Vsl),
        FramingType::new("Steel I", Composition::Steel, FramingShape::SteelI),
        FramingType::new("Steel Box", Composition::Steel, FramingShape::SteelBox),
        steel_c,
        FramingType::new(
            "Steel U Channel",
            Composition::Steel,
            FramingShape::UChannel,
        ),
        FramingType::new(
            "Solid Concrete",
            Composition::Concrete,
            FramingShape::Rectangular,
        ),
        FramingType::new(
            "Round Concrete",
            Composition::Concrete,
            FramingShape::Circular,
        ),
    ]
}

// ----- default framing members -----

/// A Default Framing Member: type, material and Role, no size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FramingMemberDef {
    pub name: String,
    /// The Framing Type, by name.
    pub framing_type: String,
    /// The 3D appearance material (not used for the Materials List).
    pub material: String,
    pub role: Role,
    pub category: CategoryChoice,
}

impl Default for FramingMemberDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            framing_type: "Lumber".into(),
            material: "Framing Lumber".into(),
            role: Role::GeneralFraming,
            category: CategoryChoice::Auto,
        }
    }
}

impl FramingMemberDef {
    fn new(name: &str, ty: &str, material: &str, role: Role) -> Self {
        Self {
            name: name.into(),
            framing_type: ty.into(),
            material: material.into(),
            role,
            category: CategoryChoice::Auto,
        }
    }
}

/// The Default Framing Members a plan starts with: one for the Roles that
/// share a definition, plus a few alternatives ("Joists - I Joists",
/// "Beams - Glulam", the steel ones).
pub fn default_defs() -> Vec<FramingMemberDef> {
    let wood = "Framing Lumber";
    vec![
        FramingMemberDef::new("Studs", "Lumber", wood, Role::Stud),
        FramingMemberDef::new("Plates", "Lumber", wood, Role::Plate),
        FramingMemberDef::new("Headers", "Lumber", wood, Role::Header),
        FramingMemberDef::new("Blocking", "Lumber", wood, Role::WallBlocking),
        FramingMemberDef::new("Joists", "Lumber", wood, Role::FloorJoist),
        FramingMemberDef::new("Joists - I Joists", "I-Joist", wood, Role::FloorJoist),
        FramingMemberDef::new("Ceiling Joists", "Lumber", wood, Role::CeilingJoist),
        FramingMemberDef::new("Rim Joists", "Lumber", wood, Role::RimJoist),
        FramingMemberDef::new("Ledgers", "Lumber", wood, Role::Ledger),
        FramingMemberDef::new("Floor/Ceiling Beams", "Lumber", wood, Role::FloorBeam),
        FramingMemberDef::new("Beams - Glulam", "Glulam", wood, Role::FloorBeam),
        FramingMemberDef::new("Beams - Steel", "Steel I", "Steel", Role::FloorBeam),
        FramingMemberDef::new("Rafters", "Lumber", wood, Role::Rafter),
        FramingMemberDef::new("Ridge", "Lumber", wood, Role::Ridge),
        FramingMemberDef::new("Fascia", "Lumber", wood, Role::Fascia),
        FramingMemberDef::new("Purlins", "Lumber", wood, Role::Purlin),
        FramingMemberDef::new("Roof Beams", "Lumber", wood, Role::RoofBeam),
        FramingMemberDef::new("Trusses", "Lumber", wood, Role::Truss),
        FramingMemberDef::new("Posts", "Lumber", wood, Role::Post),
        FramingMemberDef::new("Posts - Steel", "Steel Box", "Steel", Role::Post),
        FramingMemberDef::new("General Framing", "Lumber", wood, Role::GeneralFraming),
        FramingMemberDef::new("Steel Studs", "Steel C", "Steel", Role::Stud),
        FramingMemberDef {
            category: CategoryChoice::Fixed(MaterialsCategory::DecksWalks),
            ..FramingMemberDef::new("Deck Joists", "Lumber", "Treated Lumber", Role::DeckJoist)
        },
        FramingMemberDef {
            category: CategoryChoice::Fixed(MaterialsCategory::DecksWalks),
            ..FramingMemberDef::new("Deck Beams", "Lumber", "Treated Lumber", Role::DeckBeam)
        },
        FramingMemberDef {
            category: CategoryChoice::Fixed(MaterialsCategory::DecksWalks),
            ..FramingMemberDef::new("Deck Posts", "Lumber", "Treated Lumber", Role::DeckPost)
        },
        FramingMemberDef {
            category: CategoryChoice::Fixed(MaterialsCategory::DecksWalks),
            ..FramingMemberDef::new("Deck Planking", "Lumber", "Decking", Role::DeckPlank)
        },
    ]
}

// ----- manual framing defaults -----

/// Where a Floor/Ceiling Beam goes relative to the joists (manual p. 903).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BeamPlacement {
    /// The beam is as tall as the joists and they hang on it.
    WithJoists,
    /// The beam is under the joists, which bear on it.
    #[default]
    UnderJoists,
}

impl BeamPlacement {
    pub const ALL: [BeamPlacement; 2] = [BeamPlacement::WithJoists, BeamPlacement::UnderJoists];

    pub fn name(self) -> &'static str {
        match self {
            BeamPlacement::WithJoists => "With Joists",
            BeamPlacement::UnderJoists => "Under Joists",
        }
    }
}

/// Which wall surface a beam drawn over an exterior wall aligns with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AlignExterior {
    #[default]
    OuterLayer,
    MainLayer,
}

impl AlignExterior {
    pub const ALL: [AlignExterior; 2] = [AlignExterior::OuterLayer, AlignExterior::MainLayer];

    pub fn name(self) -> &'static str {
        match self {
            AlignExterior::OuterLayer => "Outer Layer",
            AlignExterior::MainLayer => "Main Layer",
        }
    }
}

/// The shape of a post footing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootingShape {
    #[default]
    Square,
    Round,
}

/// Construction, ply width, depth and ply count of a beam or post default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SectionDefault {
    /// The Default Framing Member, by name.
    pub construction: String,
    /// Width of each ply; the whole width when there is one ply.
    pub ply_width: f64,
    pub depth: f64,
    pub plies: u32,
    /// Posts: keep each ply square when resized.
    pub match_depth: bool,
}

impl SectionDefault {
    /// Total Width: all plies together.
    pub fn total_width(&self) -> f64 {
        self.ply_width * f64::from(self.plies.max(1))
    }
}

impl Default for SectionDefault {
    fn default() -> Self {
        Self {
            construction: "General Framing".into(),
            ply_width: 1.5,
            depth: 3.5,
            plies: 1,
            match_depth: false,
        }
    }
}

/// The Manual Framing Defaults dialog (manual pp. 901-904): General, Beams and
/// Posts panels. They affect framing drawn from now on, not what is in the
/// plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ManualFramingDefaults {
    /// General Framing: construction, width, depth.
    pub general: SectionDefault,
    pub floor_beam: SectionDefault,
    pub roof_beam: SectionDefault,
    pub placement: BeamPlacement,
    pub align_exterior: AlignExterior,
    pub post: SectionDefault,
    pub post_footing: SectionDefault,
    /// Footing height above the floor's subfloor, inches.
    pub footing_height: f64,
    pub footing_thickness: f64,
    pub footing_width: f64,
    pub footing_shape: FootingShape,
    /// Footing rebar size number and count.
    pub rebar_size: u32,
    pub rebar_count: u32,
}

impl Default for ManualFramingDefaults {
    fn default() -> Self {
        Self {
            general: SectionDefault {
                construction: "General Framing".into(),
                ply_width: 1.5,
                depth: 3.5,
                plies: 1,
                match_depth: false,
            },
            floor_beam: SectionDefault {
                construction: "Floor/Ceiling Beams".into(),
                ply_width: 3.5,
                depth: 9.25,
                plies: 1,
                match_depth: false,
            },
            roof_beam: SectionDefault {
                construction: "Roof Beams".into(),
                ply_width: 3.5,
                depth: 9.25,
                plies: 1,
                match_depth: false,
            },
            placement: BeamPlacement::UnderJoists,
            align_exterior: AlignExterior::OuterLayer,
            post: SectionDefault {
                construction: "Posts".into(),
                ply_width: 3.5,
                depth: 3.5,
                plies: 1,
                match_depth: true,
            },
            post_footing: SectionDefault {
                construction: "Posts".into(),
                ply_width: 3.5,
                depth: 3.5,
                plies: 1,
                match_depth: true,
            },
            footing_height: 0.0,
            footing_thickness: 12.0,
            footing_width: 24.0,
            footing_shape: FootingShape::Square,
            rebar_size: 4,
            rebar_count: 4,
        }
    }
}

impl ManualFramingDefaults {
    /// The section default a manual tool draws with.
    pub fn section_for(&self, kind: ManualKind) -> Option<&SectionDefault> {
        Some(match kind {
            ManualKind::GeneralFraming => &self.general,
            ManualKind::FloorCeilingBeam => &self.floor_beam,
            ManualKind::RoofBeam => &self.roof_beam,
            ManualKind::Post => &self.post,
            ManualKind::PostWithFooting => &self.post_footing,
            _ => return None,
        })
    }

    /// Gives a newly drawn member the section of its tool's default:
    /// the ply width, depth and ply count, and the footing of a post with
    /// footing. Other kinds are left alone.
    pub fn apply_new(&self, m: &mut FramingMember) {
        let Some(d) = self.section_for(m.kind) else {
            return;
        };
        m.plies = d.plies.max(1);
        m.depth = d.depth;
        m.width = d.total_width();
        if m.kind == ManualKind::PostWithFooting {
            m.footing_spec.size = self.footing_width;
            m.footing_spec.thickness = self.footing_thickness;
        }
        if m.kind == ManualKind::FloorCeilingBeam {
            m.bearing_beam = self.placement == BeamPlacement::UnderJoists;
        }
    }
}

// ----- automatic framing defaults the other types do not hold -----

/// A roof size row: on/off, width, depth and construction (manual p. 899).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoofSizeRow {
    pub on: bool,
    pub width: f64,
    pub depth: f64,
    pub construction: String,
}

impl RoofSizeRow {
    fn new(width: f64, depth: f64, construction: &str) -> Self {
        Self {
            on: true,
            width,
            depth,
            construction: construction.into(),
        }
    }
}

impl Default for RoofSizeRow {
    fn default() -> Self {
        Self::new(1.5, 5.5, "Rafters")
    }
}

/// The Automatic Framing Defaults fields that `FramingDefaults`,
/// `RoofFramingDefaults` and `BuildOptions` do not hold, by panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoFramingExtras {
    // Floor level panels
    /// Use the Ceiling Structure Definition of the Floor/Ceiling Platform
    /// Defaults, per floor level.
    pub ceiling_structure_default: Vec<bool>,
    pub floor_structure_default: Vec<bool>,
    /// Max Rim Joist Length is on (the length itself is
    /// `DetailOptions::max_rim_length`).
    pub max_rim_on: bool,
    // Wall panel
    pub stud_width: f64,
    pub top_plate_width: f64,
    pub bottom_plate_thickness: f64,
    pub max_girt_length: f64,
    pub max_plate_length: f64,
    pub allow_balloon: bool,
    pub block_exterior: bool,
    pub block_interior: bool,
    // Openings panel
    /// Bay/Box/Bow Trimmers: most trimmers per side.
    pub bay_max_trimmers: u32,
    /// Thickness of the thinner trimmer for a resized component.
    pub bay_component_thickness: f64,
    // Fireplaces panel
    pub fireplace_header_type: String,
    pub fireplace_header_thickness: f64,
    pub fireplace_header_count: u32,
    pub double_trimmer_at: f64,
    pub triple_trimmer_at: f64,
    pub fireplace_sill_thickness: f64,
    pub fireplace_double_sill: bool,
    // Roof panel
    pub angled_dormer_hole: bool,
    pub max_subfascia_on: bool,
    pub max_subfascia_length: f64,
    pub roof_blocking_style: crate::build::BlockingStyle,
    pub roof_blocking_vertical: bool,
    pub use_room_ceiling_finish: bool,
    pub flat_under_eave_subfascia: bool,
    pub ridge: RoofSizeRow,
    pub rafters: RoofSizeRow,
    pub lookouts: RoofSizeRow,
    pub shoe_plate: RoofSizeRow,
    pub gable_subfascia: RoofSizeRow,
    pub eave_subfascia: RoofSizeRow,
    pub gable_fascia: RoofSizeRow,
    pub eave_fascia: RoofSizeRow,
    pub roof_blocking: RoofSizeRow,
    // Trusses panel
    pub include_peak_trusses: bool,
    pub end_truss_blocking: bool,
    pub end_truss_vertical_spacing: f64,
    pub end_truss_rollout_offset: f64,
    pub end_truss_rollout_auto: bool,
}

impl Default for AutoFramingExtras {
    fn default() -> Self {
        Self {
            ceiling_structure_default: Vec::new(),
            floor_structure_default: Vec::new(),
            max_rim_on: false,
            stud_width: 1.5,
            top_plate_width: 3.5,
            bottom_plate_thickness: 1.5,
            max_girt_length: 192.0,
            max_plate_length: 192.0,
            allow_balloon: true,
            block_exterior: false,
            block_interior: false,
            bay_max_trimmers: 2,
            bay_component_thickness: 1.5,
            fireplace_header_type: "Lumber".into(),
            fireplace_header_thickness: 1.5,
            fireplace_header_count: 2,
            double_trimmer_at: 36.0,
            triple_trimmer_at: 60.0,
            fireplace_sill_thickness: 1.5,
            fireplace_double_sill: false,
            angled_dormer_hole: true,
            max_subfascia_on: false,
            max_subfascia_length: 192.0,
            roof_blocking_style: crate::build::BlockingStyle::InLine,
            roof_blocking_vertical: false,
            use_room_ceiling_finish: true,
            flat_under_eave_subfascia: false,
            ridge: RoofSizeRow::new(1.5, 7.25, "Ridge"),
            rafters: RoofSizeRow::new(1.5, 7.25, "Rafters"),
            lookouts: RoofSizeRow::new(1.5, 3.5, "Rafters"),
            shoe_plate: RoofSizeRow::new(1.5, 3.5, "Plates"),
            gable_subfascia: RoofSizeRow::new(1.5, 5.5, "Fascia"),
            eave_subfascia: RoofSizeRow::new(1.5, 5.5, "Fascia"),
            gable_fascia: RoofSizeRow::new(1.0, 5.5, "Fascia"),
            eave_fascia: RoofSizeRow::new(1.0, 5.5, "Fascia"),
            roof_blocking: RoofSizeRow::new(1.5, 7.25, "Blocking"),
            include_peak_trusses: true,
            end_truss_blocking: false,
            end_truss_vertical_spacing: 24.0,
            end_truss_rollout_offset: 24.0,
            end_truss_rollout_auto: true,
        }
    }
}

// ----- the catalog -----

/// Types, Default Framing Members, role constructions, manual defaults,
/// automatic extras and Structural Member Reporting of a plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FramingCatalog {
    pub types: Vec<FramingType>,
    pub defs: Vec<FramingMemberDef>,
    /// The construction automatic framing uses for a Role, by Role name: the
    /// Default Framing Member the Automatic Framing Defaults dialog picked.
    /// A Role that is not here uses [`Role::default_def`].
    pub constructions: BTreeMap<String, String>,
    pub manual: ManualFramingDefaults,
    pub auto: AutoFramingExtras,
    pub reporting: ReportingSet,
}

impl Default for FramingCatalog {
    fn default() -> Self {
        Self {
            types: default_types(),
            defs: default_defs(),
            constructions: BTreeMap::new(),
            manual: ManualFramingDefaults::default(),
            auto: AutoFramingExtras::default(),
            reporting: ReportingSet::default(),
        }
    }
}

/// Why a catalog edit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// The name is empty or already used.
    BadName,
    /// No such entry.
    Missing,
    /// The entry is in use; the text says where.
    InUse(String),
    /// The last entry cannot be removed.
    Last,
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CatalogError::BadName => write!(f, "The name is empty or already used."),
            CatalogError::Missing => write!(f, "There is no such entry."),
            CatalogError::InUse(w) => write!(f, "It is in use: {w}"),
            CatalogError::Last => write!(f, "The last entry cannot be deleted."),
        }
    }
}

impl FramingCatalog {
    // ----- lookups -----

    pub fn type_named(&self, name: &str) -> Option<&FramingType> {
        self.types.iter().find(|t| t.name == name)
    }

    pub fn def_named(&self, name: &str) -> Option<&FramingMemberDef> {
        self.defs.iter().find(|d| d.name == name)
    }

    /// The Default Framing Member automatic framing uses for `role`.
    pub fn def_for(&self, role: Role) -> Option<&FramingMemberDef> {
        let name = self
            .constructions
            .get(role.name())
            .map_or(role.default_def(), String::as_str);
        self.def_named(name)
            .or_else(|| self.def_named(role.default_def()))
    }

    /// The Framing Type for `role`: that of its definition (lumber when the
    /// definition or its type is gone).
    pub fn type_for_role(&self, role: Role) -> FramingType {
        self.def_for(role)
            .and_then(|d| self.type_named(&d.framing_type))
            .cloned()
            .unwrap_or_default()
    }

    /// The Framing Type of an automatic member: its own `framing_type` when
    /// set, else the type of the definition for its Role.
    pub fn type_of_member(&self, m: &Member) -> FramingType {
        if !m.framing_type.is_empty() {
            if let Some(t) = self.type_named(&m.framing_type) {
                return t.clone();
            }
        }
        self.type_for_role(m.role.unwrap_or_else(|| Role::of_member(m.kind)))
    }

    /// The Role of an automatic member.
    pub fn role_of_member(m: &Member) -> Role {
        m.role.unwrap_or_else(|| Role::of_member(m.kind))
    }

    /// The Materials List category of an automatic member: its definition's
    /// category when that is fixed, else the category of its Role.
    pub fn category_of_member(&self, m: &Member) -> MaterialsCategory {
        let role = Self::role_of_member(m);
        match self.def_for(role).map(|d| d.category) {
            Some(CategoryChoice::Fixed(c)) => c,
            _ => role.category(),
        }
    }

    /// The Materials List category of a manual member (`None` for layout
    /// lines).
    pub fn category_of_manual(&self, m: &FramingMember) -> Option<MaterialsCategory> {
        let role = m.role.or_else(|| Role::of_manual(m.kind))?;
        Some(match self.def_named(&m.member_def).map(|d| d.category) {
            Some(CategoryChoice::Fixed(c)) => c,
            _ => match self.def_for(role).map(|d| d.category) {
                Some(CategoryChoice::Fixed(c)) => c,
                _ => role.category(),
            },
        })
    }

    // ----- in use -----

    /// Where a Framing Type is used: definitions, supporting types and the
    /// fireplace header; the first is named in the message.
    pub fn type_use(&self, name: &str) -> Option<String> {
        if let Some(d) = self.defs.iter().find(|d| d.framing_type == name) {
            return Some(format!("the Default Framing Member \"{}\"", d.name));
        }
        if let Some(t) = self
            .types
            .iter()
            .find(|t| t.supporting_type.as_deref() == Some(name))
        {
            return Some(format!("the Supporting Type of \"{}\"", t.name));
        }
        (self.auto.fireplace_header_type == name).then(|| "the Fireplaces panel".to_string())
    }

    /// Where a Default Framing Member is used: a Role's construction, a
    /// manual default or a roof size row.
    pub fn def_use(&self, name: &str) -> Option<String> {
        if let Some((role, _)) = self.constructions.iter().find(|(_, v)| *v == name) {
            return Some(format!("the construction of {role}"));
        }
        if Role::ALL
            .iter()
            .any(|r| !self.constructions.contains_key(r.name()) && r.default_def() == name)
        {
            return Some("an automatic framing default".to_string());
        }
        let m = &self.manual;
        for (label, s) in [
            ("General Framing", &m.general),
            ("Floor/Ceiling Beams", &m.floor_beam),
            ("Roof Beams", &m.roof_beam),
            ("Posts", &m.post),
            ("Posts with Footings", &m.post_footing),
        ] {
            if s.construction == name {
                return Some(format!("the {label} manual default"));
            }
        }
        let a = &self.auto;
        // The rows with a Role of their own (ridge, rafters, fascia, blocking)
        // use that Role's construction; the others carry theirs.
        for (label, r) in [
            ("lookouts", &a.lookouts),
            ("shoe plate", &a.shoe_plate),
            ("gable subfascia", &a.gable_subfascia),
            ("eave subfascia", &a.eave_subfascia),
        ] {
            if r.construction == name {
                return Some(format!("the Roof Size {label}"));
            }
        }
        None
    }

    /// Is the Default Framing Member in use (the In Use column)?
    pub fn def_in_use(&self, name: &str) -> bool {
        self.def_use(name).is_some()
    }

    pub fn type_in_use(&self, name: &str) -> bool {
        self.type_use(name).is_some()
    }

    // ----- Framing Type management -----

    fn type_name_ok(&self, name: &str) -> bool {
        !name.trim().is_empty() && self.type_named(name.trim()).is_none()
    }

    /// Adds a type (New / Copy result); the name must be new.
    pub fn add_type(&mut self, mut t: FramingType) -> Result<(), CatalogError> {
        t.name = t.name.trim().to_string();
        if !self.type_name_ok(&t.name) {
            return Err(CatalogError::BadName);
        }
        t.normalize();
        self.types.push(t);
        Ok(())
    }

    /// Copy: a new type based on `name`, called `<name> 2` (3, ...).
    pub fn copy_type(&mut self, name: &str) -> Result<String, CatalogError> {
        let src = self
            .type_named(name)
            .cloned()
            .ok_or(CatalogError::Missing)?;
        let new = unique(&src.name, |n| self.type_named(n).is_some());
        let mut t = src;
        t.name = new.clone();
        self.types.push(t);
        Ok(new)
    }

    /// Rename a type; definitions, supporting types and members that name it
    /// follow.
    pub fn rename_type(&mut self, old: &str, new: &str) -> Result<(), CatalogError> {
        let new = new.trim();
        if new == old {
            return Ok(());
        }
        if self.type_named(old).is_none() {
            return Err(CatalogError::Missing);
        }
        if !self.type_name_ok(new) {
            return Err(CatalogError::BadName);
        }
        for t in &mut self.types {
            if t.name == old {
                t.name = new.into();
            }
            if t.supporting_type.as_deref() == Some(old) {
                t.supporting_type = Some(new.into());
            }
        }
        for d in &mut self.defs {
            if d.framing_type == old {
                d.framing_type = new.into();
            }
        }
        if self.auto.fireplace_header_type == old {
            self.auto.fireplace_header_type = new.into();
        }
        Ok(())
    }

    /// Replace the type called `name` with `edited` (Edit); a changed name is
    /// a rename.
    pub fn edit_type(&mut self, name: &str, mut edited: FramingType) -> Result<(), CatalogError> {
        edited.name = edited.name.trim().to_string();
        edited.normalize();
        if edited.name != name {
            self.rename_type(name, &edited.name.clone())?;
        }
        let slot = self
            .types
            .iter_mut()
            .find(|t| t.name == edited.name)
            .ok_or(CatalogError::Missing)?;
        *slot = edited;
        Ok(())
    }

    /// Delete a type that is not in use.
    pub fn delete_type(&mut self, name: &str) -> Result<(), CatalogError> {
        if self.type_named(name).is_none() {
            return Err(CatalogError::Missing);
        }
        if let Some(w) = self.type_use(name) {
            return Err(CatalogError::InUse(w));
        }
        if self.types.len() == 1 {
            return Err(CatalogError::Last);
        }
        self.types.retain(|t| t.name != name);
        Ok(())
    }

    // ----- Default Framing Member management -----

    fn def_name_ok(&self, name: &str) -> bool {
        !name.trim().is_empty() && self.def_named(name.trim()).is_none()
    }

    pub fn add_def(&mut self, mut d: FramingMemberDef) -> Result<(), CatalogError> {
        d.name = d.name.trim().to_string();
        if !self.def_name_ok(&d.name) {
            return Err(CatalogError::BadName);
        }
        self.defs.push(d);
        Ok(())
    }

    pub fn copy_def(&mut self, name: &str) -> Result<String, CatalogError> {
        let src = self.def_named(name).cloned().ok_or(CatalogError::Missing)?;
        let new = unique(&src.name, |n| self.def_named(n).is_some());
        let mut d = src;
        d.name = new.clone();
        self.defs.push(d);
        Ok(new)
    }

    /// Rename a definition; every place that names it follows.
    pub fn rename_def(&mut self, old: &str, new: &str) -> Result<(), CatalogError> {
        let new = new.trim();
        if new == old {
            return Ok(());
        }
        if self.def_named(old).is_none() {
            return Err(CatalogError::Missing);
        }
        if !self.def_name_ok(new) {
            return Err(CatalogError::BadName);
        }
        self.rename_refs(old, new);
        for d in &mut self.defs {
            if d.name == old {
                d.name = new.into();
            }
        }
        Ok(())
    }

    /// Points every reference to the definition `from` at `to`.
    fn rename_refs(&mut self, from: &str, to: &str) {
        for v in self.constructions.values_mut() {
            if v == from {
                *v = to.into();
            }
        }
        // A Role that used `from` by default keeps it as an explicit pick, so
        // the Role does not fall back to another definition.
        for r in Role::ALL {
            if r.default_def() == from && !self.constructions.contains_key(r.name()) {
                self.constructions.insert(r.name().into(), to.into());
            }
        }
        let m = &mut self.manual;
        for s in [
            &mut m.general,
            &mut m.floor_beam,
            &mut m.roof_beam,
            &mut m.post,
            &mut m.post_footing,
        ] {
            if s.construction == from {
                s.construction = to.into();
            }
        }
        let a = &mut self.auto;
        for r in [
            &mut a.ridge,
            &mut a.rafters,
            &mut a.lookouts,
            &mut a.shoe_plate,
            &mut a.gable_subfascia,
            &mut a.eave_subfascia,
            &mut a.gable_fascia,
            &mut a.eave_fascia,
            &mut a.roof_blocking,
        ] {
            if r.construction == from {
                r.construction = to.into();
            }
        }
    }

    /// Replace the definition called `name` with `edited`.
    pub fn edit_def(
        &mut self,
        name: &str,
        mut edited: FramingMemberDef,
    ) -> Result<(), CatalogError> {
        edited.name = edited.name.trim().to_string();
        if edited.name != name {
            self.rename_def(name, &edited.name.clone())?;
        }
        let slot = self
            .defs
            .iter_mut()
            .find(|d| d.name == edited.name)
            .ok_or(CatalogError::Missing)?;
        *slot = edited;
        Ok(())
    }

    /// Delete a definition that is not in use.
    pub fn delete_def(&mut self, name: &str) -> Result<(), CatalogError> {
        if self.def_named(name).is_none() {
            return Err(CatalogError::Missing);
        }
        if let Some(w) = self.def_use(name) {
            return Err(CatalogError::InUse(w));
        }
        self.defs.retain(|d| d.name != name);
        Ok(())
    }

    /// Merge: the first name survives, every use of the others points at it,
    /// and the others are deleted. Returns the number removed.
    pub fn merge_defs(&mut self, names: &[String]) -> Result<usize, CatalogError> {
        let Some(keep) = names.first() else {
            return Err(CatalogError::Missing);
        };
        if self.def_named(keep).is_none() {
            return Err(CatalogError::Missing);
        }
        let mut removed = 0;
        for other in &names[1..] {
            if other == keep || self.def_named(other).is_none() {
                continue;
            }
            self.rename_refs(other, keep);
            self.defs.retain(|d| d.name != *other);
            removed += 1;
        }
        Ok(removed)
    }

    /// Purge: delete every definition that is not in use. Returns the number
    /// removed.
    pub fn purge_defs(&mut self) -> usize {
        let unused: Vec<String> = self
            .defs
            .iter()
            .filter(|d| !self.def_in_use(&d.name))
            .map(|d| d.name.clone())
            .collect();
        self.defs.retain(|d| !unused.contains(&d.name));
        unused.len()
    }

    /// Sets the construction of a Role (Automatic Framing Defaults).
    pub fn set_construction(&mut self, role: Role, def: &str) {
        if self.def_named(def).is_some() {
            self.constructions.insert(role.name().into(), def.into());
        }
    }

    // ----- applying -----

    /// Apply Framing Default Properties to an automatic member: its type,
    /// Role and shape; the size and position stay.
    pub fn apply_def_to_member(&self, def: &str, m: &mut Member) -> bool {
        let Some(d) = self.def_named(def) else {
            return false;
        };
        m.framing_type = d.framing_type.clone();
        m.role = Some(d.role);
        m.shape = self
            .type_named(&d.framing_type)
            .map_or(SectionShape::Box, |t| t.shape.section());
        true
    }

    /// Apply Framing Default Properties to a manual member: type, Role,
    /// material and definition; the section, length and position stay.
    pub fn apply_def_to_manual(&self, def: &str, m: &mut FramingMember) -> bool {
        let Some(d) = self.def_named(def) else {
            return false;
        };
        let ty = self.type_named(&d.framing_type);
        m.member_def = d.name.clone();
        m.framing_type = d.framing_type.clone();
        m.role = Some(d.role);
        if let Some(t) = ty {
            m.material = t.shape.material();
        }
        true
    }

    /// Gives every automatic member the shape of its Framing Type. Members
    /// follow their definitions by name, so this is what an edit of a type or
    /// of a Role's construction does to a plan. Returns how many members
    /// changed shape.
    pub fn stamp_members(&self, members: &mut [Member]) -> usize {
        let mut changed = 0;
        for m in members {
            let shape = self.type_of_member(m).shape.section();
            if m.shape != shape {
                m.shape = shape;
                changed += 1;
            }
        }
        changed
    }

    /// The Materials List description of a member's size: `2x10` for a type
    /// that lists nominal sizes, the actual dimensions otherwise, with the
    /// type's name in front when it asks for that.
    pub fn size_text(&self, ty: &FramingType, lumber: &crate::Lumber) -> String {
        let size = if ty.display_nominal {
            lumber.nominal_name()
        } else {
            format!(
                "{}x{}",
                crate::format_inches(lumber.thickness),
                crate::format_inches(lumber.depth)
            )
        };
        if ty.include_name_in_labels {
            format!("{} {}", ty.name, size)
        } else {
            size
        }
    }
}

/// `base`, `base 2`, `base 3`, ... — the first name `taken` does not know.
fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| base.to_string())
}

// ----- storage -----

/// The catalog stored in the framing slot of the first floor (the defaults
/// when none, or when it does not parse).
pub fn load(first_floor_framing: &[Value]) -> FramingCatalog {
    first_floor_framing
        .iter()
        .find_map(|v| v.as_object()?.get(CATALOG_KEY).cloned())
        .and_then(|v| serde_json::from_value::<FramingCatalog>(v).ok())
        .unwrap_or_default()
}

/// Is `v` the stored catalog object?
pub fn is_catalog(v: &Value) -> bool {
    v.as_object()
        .is_some_and(|o| o.len() == 1 && o.contains_key(CATALOG_KEY))
}

/// Is a catalog stored in the slot (the plan has used the framing dialogs)?
pub fn is_stored(first_floor_framing: &[Value]) -> bool {
    first_floor_framing.iter().any(is_catalog)
}

/// Writes the catalog into the slot, replacing the stored one.
pub fn store(first_floor_framing: &mut Vec<Value>, catalog: &FramingCatalog) {
    first_floor_framing.retain(|v| !is_catalog(v));
    if let Ok(v) = serde_json::to_value(catalog) {
        let mut o = serde_json::Map::new();
        o.insert(CATALOG_KEY.to_string(), v);
        first_floor_framing.push(Value::Object(o));
    }
}

/// The catalog of a project's first floor.
pub fn of_project(project: &plan_core::Project) -> FramingCatalog {
    project
        .floors
        .first()
        .map(|f| load(&f.framing))
        .unwrap_or_default()
}

/// Is a catalog stored in the project?
pub fn project_has_catalog(project: &plan_core::Project) -> bool {
    project
        .floors
        .first()
        .is_some_and(|f| is_stored(&f.framing))
}

/// List Cut Header Lengths of the stored Framing Defaults (the Openings
/// panel): the setting the Mixed Reporting method reads. False when the plan
/// never saved the framing defaults.
pub fn list_cut_headers(first_floor_framing: &[Value]) -> bool {
    first_floor_framing
        .iter()
        .find_map(|v| {
            v.as_object()?
                .get("FramingSettings")?
                .pointer("/build/detail/list_cut_header_lengths")?
                .as_bool()
        })
        .unwrap_or(false)
}

/// Re-stamps the shape of every automatic member stored on `floor` from
/// `catalog`. Returns how many members changed. Manual records and settings
/// are not touched.
pub fn stamp_floor(floor: &mut plan_core::Floor, catalog: &FramingCatalog) -> usize {
    let mut changed = 0;
    for v in &mut floor.framing {
        let Some(o) = v.as_object() else { continue };
        // Records and the stored settings are one-key objects; a member has
        // many keys.
        if o.len() < 2 {
            continue;
        }
        let Ok(mut m) = serde_json::from_value::<Member>(v.clone()) else {
            continue;
        };
        let shape = catalog.type_of_member(&m).shape.section();
        if m.shape != shape {
            m.shape = shape;
            if let Ok(nv) = serde_json::to_value(&m) {
                *v = nv;
                changed += 1;
            }
        }
    }
    changed
}

/// [`stamp_floor`] for every floor of `project`.
pub fn stamp_project(project: &mut plan_core::Project, catalog: &FramingCatalog) -> usize {
    project
        .floors
        .iter_mut()
        .map(|f| stamp_floor(f, catalog))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumber::TWO_BY_TEN;
    use crate::member::Transform3;

    fn joist() -> Member {
        let tf = Transform3 {
            origin: [0.0; 3],
            axis_x: [1.0, 0.0, 0.0],
            axis_y: [0.0, 1.0, 0.0],
        };
        Member::new(MemberKind::Joist, TWO_BY_TEN, 120.0, tf, None)
    }

    #[test]
    fn the_default_catalog_has_the_chief_types_and_every_role_has_a_definition() {
        let c = FramingCatalog::default();
        for name in [
            "Lumber", "I-Joist", "Glulam", "LVL", "PSL", "VSL", "Steel I", "Steel C",
        ] {
            assert!(c.type_named(name).is_some(), "{name}");
        }
        for r in Role::ALL {
            assert!(c.def_for(r).is_some(), "{}", r.name());
            assert!(c.type_named(&c.def_for(r).unwrap().framing_type).is_some());
        }
        let steel_c = c.type_named("Steel C").unwrap();
        assert_eq!(steel_c.supporting_type.as_deref(), Some("Steel U Channel"));
        assert!(!c.type_named("Lumber").unwrap().include_name_in_labels);
    }

    #[test]
    fn roles_and_categories_follow_the_member_kind() {
        assert_eq!(Role::of_member(MemberKind::TopPlate), Role::Plate);
        assert_eq!(Role::of_member(MemberKind::Joist), Role::FloorJoist);
        assert_eq!(Role::of_manual(ManualKind::RoofBeam), Some(Role::RoofBeam));
        assert_eq!(Role::of_manual(ManualKind::BearingLine), None);
        assert_eq!(Role::Stud.category(), MaterialsCategory::Framing);
        assert_eq!(Role::FloorJoist.category(), MaterialsCategory::Subfloor);
        assert_eq!(Role::Rafter.category(), MaterialsCategory::Roofing);
        assert_eq!(Role::DeckJoist.category(), MaterialsCategory::DecksWalks);
        assert_eq!(MaterialsCategory::DecksWalks.code(), "DW");
    }

    #[test]
    fn a_member_resolves_its_type_through_its_role_and_a_construction_changes_it() {
        let mut c = FramingCatalog::default();
        let j = joist();
        assert_eq!(c.type_of_member(&j).name, "Lumber");
        c.set_construction(Role::FloorJoist, "Joists - I Joists");
        assert_eq!(c.type_of_member(&j).name, "I-Joist");
        // An explicit type on the member wins over the Role's construction.
        let mut own = joist();
        own.framing_type = "LVL".into();
        assert_eq!(c.type_of_member(&own).name, "LVL");
    }

    #[test]
    fn editing_a_type_to_i_joist_restamps_the_floor_members() {
        let mut c = FramingCatalog::default();
        let mut ms = vec![joist(), joist()];
        assert_eq!(c.stamp_members(&mut ms), 0);
        let mut lumber = c.type_named("Lumber").unwrap().clone();
        lumber.composition = Composition::Wood;
        lumber.shape = FramingShape::IJoist;
        c.edit_type("Lumber", lumber).unwrap();
        assert_eq!(c.stamp_members(&mut ms), 2);
        assert_eq!(ms[0].shape, SectionShape::IJoist);
        // The I-joist mesh is an extruded outline: more triangles than a box.
        assert!(ms[0].mesh().triangle_count() > 12);
    }

    #[test]
    fn apply_default_keeps_the_size_and_changes_type_and_role() {
        let c = FramingCatalog::default();
        let mut m = joist();
        assert!(c.apply_def_to_member("Joists - I Joists", &mut m));
        assert_eq!(m.framing_type, "I-Joist");
        assert_eq!(m.role, Some(Role::FloorJoist));
        assert_eq!(m.shape, SectionShape::IJoist);
        assert_eq!(m.lumber, TWO_BY_TEN);
        assert_eq!(m.length, 120.0);
        let mut fm = FramingMember::new(
            1,
            ManualKind::FloorCeilingBeam,
            plan_core::Point::new(0.0, 0.0),
            plan_core::Point::new(96.0, 0.0),
        );
        let (depth, width) = (fm.depth, fm.width);
        assert!(c.apply_def_to_manual("Beams - Steel", &mut fm));
        assert_eq!(fm.material, FramingMaterial::Steel);
        assert_eq!(fm.member_def, "Beams - Steel");
        assert_eq!((fm.depth, fm.width), (depth, width));
        assert!(!c.apply_def_to_manual("Nope", &mut fm));
    }

    #[test]
    fn definitions_copy_rename_merge_purge_and_refuse_to_delete_what_is_in_use() {
        let mut c = FramingCatalog::default();
        assert!(c.def_in_use("Studs"));
        assert!(matches!(c.delete_def("Studs"), Err(CatalogError::InUse(_))));
        assert!(!c.def_in_use("Joists - I Joists"));
        let copy = c.copy_def("Joists - I Joists").unwrap();
        assert_eq!(copy, "Joists - I Joists 2");
        c.rename_def(&copy, "My I Joists").unwrap();
        assert!(c.def_named("My I Joists").is_some());
        assert_eq!(c.rename_def("Studs", "Posts"), Err(CatalogError::BadName));
        // Renaming a definition that a Role uses keeps the Role on it.
        c.rename_def("Studs", "Wall Studs").unwrap();
        assert_eq!(c.def_for(Role::Stud).unwrap().name, "Wall Studs");
        // Merge: the others point at the first and disappear.
        c.set_construction(Role::FloorJoist, "My I Joists");
        let removed = c
            .merge_defs(&["Joists".to_string(), "My I Joists".to_string()])
            .unwrap();
        assert_eq!(removed, 1);
        assert_eq!(c.def_for(Role::FloorJoist).unwrap().name, "Joists");
        assert!(c.def_named("My I Joists").is_none());
        let before = c.defs.len();
        let purged = c.purge_defs();
        assert!(purged > 0 && c.defs.len() == before - purged);
        assert!(c.def_named("Wall Studs").is_some(), "in-use entries stay");
        c.delete_def("Fascia").unwrap_or(());
    }

    #[test]
    fn a_type_in_use_cannot_be_deleted_and_the_message_names_where() {
        let mut c = FramingCatalog::default();
        match c.delete_type("Lumber") {
            Err(CatalogError::InUse(w)) => assert!(w.contains("Default Framing Member")),
            other => panic!("{other:?}"),
        }
        assert!(c.delete_type("PSL").is_err() || c.type_named("PSL").is_none());
        let copy = c.copy_type("VSL").unwrap();
        c.delete_type(&copy).unwrap();
        // Steel U Channel is the Supporting Type of Steel C.
        assert!(matches!(
            c.delete_type("Steel U Channel"),
            Err(CatalogError::InUse(_))
        ));
        c.rename_type("Steel U Channel", "U Plates").unwrap();
        assert_eq!(
            c.type_named("Steel C").unwrap().supporting_type.as_deref(),
            Some("U Plates")
        );
    }

    #[test]
    fn a_type_keeps_a_shape_its_composition_has_and_only_steel_c_has_a_supporting_type() {
        let mut t = FramingType::new("X", Composition::Steel, FramingShape::Lumber);
        t.supporting_type = Some("Y".into());
        t.normalize();
        assert_eq!(t.shape, FramingShape::SteelI);
        assert_eq!(t.supporting_type, None);
    }

    #[test]
    fn size_text_follows_nominal_and_name_flags() {
        let c = FramingCatalog::default();
        assert_eq!(
            c.size_text(c.type_named("Lumber").unwrap(), &TWO_BY_TEN),
            "2x10"
        );
        assert_eq!(
            c.size_text(c.type_named("I-Joist").unwrap(), &TWO_BY_TEN),
            "I-Joist 1 1/2x9 1/4"
        );
    }

    #[test]
    fn manual_defaults_shape_new_posts_beams_and_footings() {
        let mut d = ManualFramingDefaults::default();
        d.floor_beam.plies = 2;
        d.floor_beam.ply_width = 1.75;
        d.floor_beam.depth = 11.875;
        d.footing_width = 30.0;
        d.footing_thickness = 10.0;
        assert_eq!(d.floor_beam.total_width(), 3.5);
        let mut beam = FramingMember::new(
            1,
            ManualKind::FloorCeilingBeam,
            plan_core::Point::new(0.0, 0.0),
            plan_core::Point::new(96.0, 0.0),
        );
        d.apply_new(&mut beam);
        assert_eq!((beam.plies, beam.width, beam.depth), (2, 3.5, 11.875));
        assert!(beam.bearing_beam, "Under Joists is a bearing beam");
        d.placement = BeamPlacement::WithJoists;
        d.apply_new(&mut beam);
        assert!(!beam.bearing_beam);
        let mut post = FramingMember::new(
            2,
            ManualKind::PostWithFooting,
            plan_core::Point::new(0.0, 0.0),
            plan_core::Point::new(0.0, 0.0),
        );
        d.apply_new(&mut post);
        assert_eq!(
            (post.footing_spec.size, post.footing_spec.thickness),
            (30.0, 10.0)
        );
        let mut line = FramingMember::new(
            3,
            ManualKind::BearingLine,
            plan_core::Point::new(0.0, 0.0),
            plan_core::Point::new(9.0, 0.0),
        );
        let before = line.clone();
        d.apply_new(&mut line);
        assert_eq!(line, before);
    }

    #[test]
    fn list_cut_headers_is_read_from_the_stored_framing_settings() {
        let none: Vec<Value> = Vec::new();
        assert!(!list_cut_headers(&none));
        let on = vec![serde_json::json!({"FramingSettings": {"build": {"detail": {
            "list_cut_header_lengths": true}}}})];
        assert!(list_cut_headers(&on));
        let off = vec![serde_json::json!({"FramingSettings": {"build": {}}})];
        assert!(!list_cut_headers(&off));
    }

    #[test]
    fn the_catalog_round_trips_through_the_framing_slot_and_stamps_a_project() {
        let mut slot: Vec<Value> = Vec::new();
        assert!(!is_stored(&slot));
        let mut c = FramingCatalog::default();
        c.set_construction(Role::FloorJoist, "Joists - I Joists");
        store(&mut slot, &c);
        store(&mut slot, &c);
        assert_eq!(slot.len(), 1);
        assert_eq!(load(&slot), c);
        let mut p = plan_core::Project::new("t");
        p.floors[0]
            .framing
            .push(serde_json::to_value(joist()).unwrap());
        store(&mut p.floors[0].framing, &c);
        assert_eq!(stamp_project(&mut p, &c), 1);
        assert_eq!(stamp_project(&mut p, &c), 0);
        let m: Member = serde_json::from_value(p.floors[0].framing[0].clone()).unwrap();
        assert_eq!(m.shape, SectionShape::IJoist);
    }
}
