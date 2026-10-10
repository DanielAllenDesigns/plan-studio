//! Layered assemblies (Round 16, brief 13): the Material Layers Definition
//! behind a floor or ceiling platform, a finish or a roof plane (manual pp.
//! 769-770, 1088-1091).
//!
//! An [`Assembly`] is an ordered list of [`AssemblyLayer`]s, the highest layer
//! first, as a section view shows them (a floor's surface, a ceiling's
//! plenum; the manual's dropped ceiling is plenum, framing, drywall, paint
//! from the top). Each
//! layer has a material, a fill for Auto Detail, a [`LayerRole`], a thickness
//! and energy values; the Framing layer also carries joist or truss options
//! ([`FramingSpec`]).
//!
//! # Where definitions live and how they resolve
//!
//! Four definitions describe a floor level's platforms: Floor Structure,
//! Floor Finish, Ceiling Structure and Ceiling Finish ([`PlatformAssemblies`]).
//! Each can be set at three levels, and a level either owns a definition,
//! follows the level above ("Use Default") or is still the single thickness
//! an older plan stored ([`AssemblySlot`]):
//!
//! 1. plan-wide, in [`AssemblyLibrary::plan_wide`] (Default Settings >
//!    Floor/Ceiling Platform);
//! 2. the floor level, [`crate::floors::FloorSettings::platform`] (Floor
//!    Defaults);
//! 3. the room, [`crate::extras::RoomMisc::assemblies`] (Room Specification >
//!    Structure).
//!
//! [`resolve`] walks that chain. A slot that still holds the
//! legacy single thickness resolves to a one-layer assembly made from the old
//! field, so an older plan loads, shows and saves exactly as before: nothing
//! is written for a [`AssemblySlot::Legacy`] slot (DECISIONS LA1).
//!
//! The legacy thickness fields stay the number the rest of the program reads
//! (floor-to-floor rise, wall tops). [`crate::model::Project::sync_platform_mirrors`]
//! writes the resolved totals back into them whenever a definition changes.
//!
//! # Dropped ceilings
//!
//! A Ceiling Finish with a plenum and framing is a dropped (suspended)
//! ceiling. The room keeps its Ceiling Height and the platform keeps its
//! place (wall tops do not move); the finished ceiling hangs lower by the
//! thickness of the Air Gap and Framing layers ([`Assembly::drop`]). The
//! other (surface) layers stack on the platform's underside as before
//! ([`Assembly::surface_thickness`]).

use crate::extras::{RoomMisc, StructureLayer};
use crate::floors::FloorSettings;
use serde::{Deserialize, Serialize};

/// What a layer is in the construction (the Role column of the Material
/// Layers Definition dialog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LayerRole {
    #[default]
    Standard,
    /// The joists, rafters or trusses (the layer the framing options belong
    /// to).
    Framing,
    Sheathing,
    Finish,
    /// A plenum or other empty space: no purchase, no solid in 3D.
    AirGap,
    /// 3D cladding: the surface 3D views show over the layer below.
    Cladding,
}

impl LayerRole {
    pub const ALL: [LayerRole; 6] = [
        LayerRole::Standard,
        LayerRole::Framing,
        LayerRole::Sheathing,
        LayerRole::Finish,
        LayerRole::AirGap,
        LayerRole::Cladding,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LayerRole::Standard => "Standard",
            LayerRole::Framing => "Framing",
            LayerRole::Sheathing => "Sheathing",
            LayerRole::Finish => "Finish",
            LayerRole::AirGap => "Air Gap",
            LayerRole::Cladding => "3D Cladding",
        }
    }
}

/// How the framing layer is framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FramingMethod {
    #[default]
    Joists,
    Rafters,
    Trusses,
    /// Purlins over the rafters (a roof structure option, stored only).
    Purlins,
}

impl FramingMethod {
    pub const ALL: [FramingMethod; 4] = [
        FramingMethod::Joists,
        FramingMethod::Rafters,
        FramingMethod::Trusses,
        FramingMethod::Purlins,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FramingMethod::Joists => "Joists",
            FramingMethod::Rafters => "Rafters",
            FramingMethod::Trusses => "Trusses",
            FramingMethod::Purlins => "Purlins",
        }
    }
}

/// What the framing members are made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FramingConstruction {
    #[default]
    Lumber,
    IJoist,
    HatChannel,
}

impl FramingConstruction {
    pub const ALL: [FramingConstruction; 3] = [
        FramingConstruction::Lumber,
        FramingConstruction::IJoist,
        FramingConstruction::HatChannel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FramingConstruction::Lumber => "Lumber",
            FramingConstruction::IJoist => "I-Joist",
            FramingConstruction::HatChannel => "Hat Channel",
        }
    }
}

/// The framing options of a Framing layer. The layer's own thickness is the
/// member depth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FramingSpec {
    pub method: FramingMethod,
    pub construction: FramingConstruction,
    /// Member width, inches.
    pub width: f64,
    /// On-center spacing, inches.
    pub spacing: f64,
}

impl Default for FramingSpec {
    fn default() -> Self {
        Self {
            method: FramingMethod::Joists,
            construction: FramingConstruction::Lumber,
            width: 1.5,
            spacing: 16.0,
        }
    }
}

/// One layer of an [`Assembly`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssemblyLayer {
    /// Material name (a Materials library entry or a plain description).
    pub material: String,
    /// Fill the Auto Detail section view shows ("" = the material's own).
    pub fill: String,
    pub role: LayerRole,
    /// Thickness, inches.
    pub thickness: f64,
    /// Joist or truss options; only a Framing layer has them.
    pub framing: Option<FramingSpec>,
    /// R-value of the cavity path, and of the continuous path.
    pub r_cavity: f64,
    pub r_continuous: f64,
}

impl Default for AssemblyLayer {
    fn default() -> Self {
        Self {
            material: String::new(),
            fill: String::new(),
            role: LayerRole::Standard,
            thickness: 0.0,
            framing: None,
            r_cavity: 0.0,
            r_continuous: 0.0,
        }
    }
}

impl AssemblyLayer {
    pub fn new(material: impl Into<String>, role: LayerRole, thickness: f64) -> Self {
        Self {
            material: material.into(),
            role,
            thickness,
            framing: (role == LayerRole::Framing).then(FramingSpec::default),
            ..Self::default()
        }
    }

    /// Sets the role; a Framing layer gets default framing options and any
    /// other layer loses them.
    pub fn set_role(&mut self, role: LayerRole) {
        self.role = role;
        match role {
            LayerRole::Framing => {
                self.framing.get_or_insert_with(FramingSpec::default);
            }
            _ => self.framing = None,
        }
    }

    /// Does the layer take up space (an Air Gap is a plenum, still space but
    /// no solid)?
    pub fn is_solid(&self) -> bool {
        self.role != LayerRole::AirGap
    }
}

/// The eight definitions a plan keeps (manual pp. 1088-1092).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AssemblyKind {
    FloorStructure,
    FloorFinish,
    CeilingStructure,
    CeilingFinish,
    RoofSurface,
    RoofStructure,
    RoofCeilingFinish,
    Backsplash,
}

impl AssemblyKind {
    pub const ALL: [AssemblyKind; 8] = [
        AssemblyKind::FloorStructure,
        AssemblyKind::FloorFinish,
        AssemblyKind::CeilingStructure,
        AssemblyKind::CeilingFinish,
        AssemblyKind::RoofSurface,
        AssemblyKind::RoofStructure,
        AssemblyKind::RoofCeilingFinish,
        AssemblyKind::Backsplash,
    ];

    /// The four platform kinds, in dialog order.
    pub const PLATFORM: [AssemblyKind; 4] = [
        AssemblyKind::FloorStructure,
        AssemblyKind::FloorFinish,
        AssemblyKind::CeilingStructure,
        AssemblyKind::CeilingFinish,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AssemblyKind::FloorStructure => "Floor Structure",
            AssemblyKind::FloorFinish => "Floor Finish",
            AssemblyKind::CeilingStructure => "Ceiling Structure",
            AssemblyKind::CeilingFinish => "Ceiling Finish",
            AssemblyKind::RoofSurface => "Roof Surface",
            AssemblyKind::RoofStructure => "Roof Structure",
            AssemblyKind::RoofCeilingFinish => "Roof Ceiling Finish",
            AssemblyKind::Backsplash => "Backsplash",
        }
    }

    /// The window title of the Material Layers Definition dialog.
    pub fn dialog_title(self) -> String {
        format!("{} Definition", self.label())
    }

    pub fn is_platform(self) -> bool {
        Self::PLATFORM.contains(&self)
    }

    /// The built-in definition: what Reset puts back and where a new layered
    /// definition starts. The platform ones match the single thicknesses a
    /// floor starts with.
    pub fn builtin(self) -> Assembly {
        use LayerRole::*;
        let l = AssemblyLayer::new;
        Assembly {
            layers: match self {
                AssemblyKind::FloorStructure => vec![
                    l("Plywood Subfloor", Sheathing, 1.0),
                    l("Floor Joist", Framing, 9.25),
                ],
                AssemblyKind::FloorFinish => vec![l("Hardwood", Finish, 0.75)],
                AssemblyKind::CeilingStructure => vec![l("Ceiling Joist", Framing, 5.5)],
                AssemblyKind::CeilingFinish => vec![l("Drywall", Finish, 0.625)],
                AssemblyKind::RoofSurface => vec![l("Asphalt Shingles", Finish, 0.5)],
                AssemblyKind::RoofStructure => vec![
                    l("Roof Sheathing", Sheathing, 0.5),
                    l("Rafter", Framing, 5.5),
                ],
                AssemblyKind::RoofCeilingFinish => vec![l("Drywall", Finish, 0.5)],
                AssemblyKind::Backsplash => {
                    vec![l("Tile", Finish, 0.25), l("Backerboard", Standard, 0.5)]
                }
            },
        }
    }

    /// The material a one-layer migration of the legacy thickness gets.
    fn legacy_layer(self, thickness: f64) -> Assembly {
        let (name, role) = match self {
            AssemblyKind::FloorStructure => ("Floor Platform", LayerRole::Standard),
            AssemblyKind::FloorFinish => ("Floor Finish", LayerRole::Finish),
            AssemblyKind::CeilingStructure => ("Ceiling Platform", LayerRole::Standard),
            _ => ("Ceiling Finish", LayerRole::Finish),
        };
        Assembly::from_thickness(name, role, thickness)
    }
}

/// An ordered stack of layers, the highest layer first.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Assembly {
    pub layers: Vec<AssemblyLayer>,
}

impl Assembly {
    pub fn new(layers: Vec<AssemblyLayer>) -> Self {
        Self { layers }
    }

    /// A single layer; nothing at all for a thickness of zero or less.
    pub fn from_thickness(material: &str, role: LayerRole, thickness: f64) -> Self {
        if thickness > 0.0 {
            Self::new(vec![AssemblyLayer::new(material, role, thickness)])
        } else {
            Self::default()
        }
    }

    /// The migration of an older layer stack (Floor/Ceiling Structure
    /// Define): the same materials and thicknesses; a layer named like
    /// framing gets the Framing role.
    pub fn from_structure(layers: &[StructureLayer]) -> Self {
        Self::new(
            layers
                .iter()
                .map(|s| {
                    let n = s.material.to_lowercase();
                    let role = if n.contains("joist") || n.contains("framing") {
                        LayerRole::Framing
                    } else {
                        LayerRole::Standard
                    };
                    AssemblyLayer::new(s.material.clone(), role, s.thickness)
                })
                .collect(),
        )
    }

    /// The older layer stack for readers that still use it.
    pub fn to_structure(&self) -> Vec<StructureLayer> {
        self.layers
            .iter()
            .map(|l| StructureLayer::new(l.material.clone(), l.thickness))
            .collect()
    }

    /// [`Assembly::from_structure`] for the stack of a `kind`: the older
    /// Ceiling Structure stack was listed from the underside up, so it is
    /// turned around to read from the top like every other definition.
    pub fn from_structure_of(kind: AssemblyKind, layers: &[StructureLayer]) -> Self {
        let mut a = Self::from_structure(layers);
        if kind == AssemblyKind::CeilingStructure {
            a.layers.reverse();
        }
        a
    }

    /// [`Assembly::to_structure`] for the stack of a `kind` (see
    /// [`Assembly::from_structure_of`]).
    pub fn to_structure_of(&self, kind: AssemblyKind) -> Vec<StructureLayer> {
        let mut v = self.to_structure();
        if kind == AssemblyKind::CeilingStructure {
            v.reverse();
        }
        v
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Total thickness, inches.
    pub fn total_thickness(&self) -> f64 {
        self.layers.iter().map(|l| l.thickness.max(0.0)).sum()
    }

    /// The first Framing layer.
    pub fn framing_layer(&self) -> Option<&AssemblyLayer> {
        self.layers.iter().find(|l| l.role == LayerRole::Framing)
    }

    /// The framing options of the first Framing layer, with its depth.
    pub fn framing(&self) -> Option<(FramingSpec, f64)> {
        self.framing_layer()
            .map(|l| (l.framing.unwrap_or_default(), l.thickness))
    }

    /// Thickness of the Air Gap and Framing layers: how far a Ceiling Finish
    /// hangs the finished ceiling below the platform it is fixed to.
    pub fn drop(&self) -> f64 {
        self.layers
            .iter()
            .filter(|l| matches!(l.role, LayerRole::AirGap | LayerRole::Framing))
            .map(|l| l.thickness.max(0.0))
            .sum()
    }

    /// Thickness of the layers that are not part of the [`Assembly::drop`]:
    /// the surface the platform sits on.
    pub fn surface_thickness(&self) -> f64 {
        self.total_thickness() - self.drop()
    }

    /// Thermal resistance, cavity path plus continuous path, summed over the
    /// layers.
    pub fn r_value(&self) -> f64 {
        self.layers
            .iter()
            .map(|l| l.r_cavity.max(0.0) + l.r_continuous.max(0.0))
            .sum()
    }

    /// Where each layer lies in a section when the top of the stack is at
    /// elevation `top`: the first layer highest, each one below the last.
    /// This is what a section drawing (Auto Detail) or a 3D builder stacks.
    pub fn spans_down(&self, top: f64) -> Vec<LayerSpan<'_>> {
        let mut y = top;
        self.layers
            .iter()
            .map(|layer| {
                let t = layer.thickness.max(0.0);
                let span = LayerSpan {
                    layer,
                    y0: y - t,
                    y1: y,
                };
                y -= t;
                span
            })
            .collect()
    }

    /// Adds `layer` at `at` (clamped to the end); returns the index it got.
    pub fn insert(&mut self, at: usize, layer: AssemblyLayer) -> usize {
        let at = at.min(self.layers.len());
        self.layers.insert(at, layer);
        at
    }

    /// Removes layer `i`; returns whether there was one.
    pub fn remove(&mut self, i: usize) -> bool {
        if i < self.layers.len() {
            self.layers.remove(i);
            true
        } else {
            false
        }
    }

    /// Moves layer `i` one place toward the room side; returns its new index.
    pub fn move_up(&mut self, i: usize) -> Option<usize> {
        (i > 0 && i < self.layers.len()).then(|| {
            self.layers.swap(i, i - 1);
            i - 1
        })
    }

    /// Moves layer `i` one place away from the room side.
    pub fn move_down(&mut self, i: usize) -> Option<usize> {
        (i + 1 < self.layers.len()).then(|| {
            self.layers.swap(i, i + 1);
            i + 1
        })
    }

    /// Why the definition cannot be used, if it cannot.
    pub fn error(&self) -> Option<String> {
        for (i, l) in self.layers.iter().enumerate() {
            if !l.thickness.is_finite() || l.thickness < 0.0 {
                return Some(format!("Layer {}: thickness cannot be negative", i + 1));
            }
            if let Some(f) = l.framing {
                if f.width <= 0.0 {
                    return Some(format!("Layer {}: the framing needs a width", i + 1));
                }
                if f.spacing < 1.0 {
                    return Some(format!("Layer {}: spacing must be at least 1 inch", i + 1));
                }
            }
        }
        None
    }
}

/// One layer's place in a section, elevations in inches (see
/// [`Assembly::spans_down`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerSpan<'a> {
    pub layer: &'a AssemblyLayer,
    pub y0: f64,
    pub y1: f64,
}

/// Where one of the four platform definitions comes from at one level.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum AssemblySlot {
    /// Nothing defined: the single thickness an older plan stored. Never
    /// written to a file.
    #[default]
    Legacy,
    /// "Use Default": the level above decides.
    Default,
    /// This level's own definition.
    Own(Assembly),
}

impl AssemblySlot {
    pub fn is_legacy(&self) -> bool {
        matches!(self, AssemblySlot::Legacy)
    }

    pub fn own(&self) -> Option<&Assembly> {
        match self {
            AssemblySlot::Own(a) => Some(a),
            _ => None,
        }
    }
}

/// The four platform definitions of one level.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PlatformAssemblies {
    #[serde(skip_serializing_if = "AssemblySlot::is_legacy")]
    pub floor_structure: AssemblySlot,
    #[serde(skip_serializing_if = "AssemblySlot::is_legacy")]
    pub floor_finish: AssemblySlot,
    #[serde(skip_serializing_if = "AssemblySlot::is_legacy")]
    pub ceiling_structure: AssemblySlot,
    #[serde(skip_serializing_if = "AssemblySlot::is_legacy")]
    pub ceiling_finish: AssemblySlot,
}

impl PlatformAssemblies {
    /// Nothing defined at this level.
    pub fn is_legacy(&self) -> bool {
        AssemblyKind::PLATFORM
            .iter()
            .all(|k| self.slot(*k).is_legacy())
    }

    /// The slot of a platform kind (any other kind reads as Legacy).
    pub fn slot(&self, kind: AssemblyKind) -> &AssemblySlot {
        static LEGACY: AssemblySlot = AssemblySlot::Legacy;
        match kind {
            AssemblyKind::FloorStructure => &self.floor_structure,
            AssemblyKind::FloorFinish => &self.floor_finish,
            AssemblyKind::CeilingStructure => &self.ceiling_structure,
            AssemblyKind::CeilingFinish => &self.ceiling_finish,
            _ => &LEGACY,
        }
    }

    /// Sets the slot of a platform kind; other kinds are ignored.
    pub fn set(&mut self, kind: AssemblyKind, slot: AssemblySlot) {
        match kind {
            AssemblyKind::FloorStructure => self.floor_structure = slot,
            AssemblyKind::FloorFinish => self.floor_finish = slot,
            AssemblyKind::CeilingStructure => self.ceiling_structure = slot,
            AssemblyKind::CeilingFinish => self.ceiling_finish = slot,
            _ => {}
        }
    }

    /// Does any definition here own layers?
    pub fn has_own(&self) -> bool {
        AssemblyKind::PLATFORM
            .iter()
            .any(|k| self.slot(*k).own().is_some())
    }
}

/// A definition saved under a name, for reuse (Material Layers Definition >
/// Library).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedAssembly {
    pub name: String,
    pub kind: AssemblyKind,
    pub assembly: Assembly,
}

/// What a plan keeps for assemblies.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssemblyLibrary {
    /// The plan-wide Floor/Ceiling Platform Defaults. `Default` reads as
    /// `Legacy` here: there is no level above.
    #[serde(skip_serializing_if = "PlatformAssemblies::is_legacy")]
    pub plan_wide: PlatformAssemblies,
    /// The Backsplash definition of the plan (stored; the cabinet tools do
    /// not read it yet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backsplash: Option<Assembly>,
    /// Definitions saved by name.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub named: Vec<NamedAssembly>,
}

/// Which level a resolved definition came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Room,
    Floor,
    PlanWide,
    /// A single thickness of an older plan, read as one layer.
    Legacy,
}

/// A definition and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub assembly: Assembly,
    pub source: Source,
}

impl Resolved {
    /// Was the definition made with the layered dialog (anywhere in the
    /// chain) rather than read from an older plan's thickness?
    pub fn is_layered(&self) -> bool {
        self.source != Source::Legacy
    }
}

impl AssemblyLibrary {
    pub fn is_empty(&self) -> bool {
        *self == AssemblyLibrary::default()
    }

    /// The saved definition `name` of `kind`.
    pub fn named_of(&self, kind: AssemblyKind, name: &str) -> Option<&NamedAssembly> {
        self.named.iter().find(|n| n.kind == kind && n.name == name)
    }

    /// Saves `assembly` under `name` (replacing a saved one of that name and
    /// kind).
    pub fn save_named(&mut self, kind: AssemblyKind, name: &str, assembly: Assembly) {
        match self
            .named
            .iter_mut()
            .find(|n| n.kind == kind && n.name == name)
        {
            Some(n) => n.assembly = assembly,
            None => self.named.push(NamedAssembly {
                name: name.to_string(),
                kind,
                assembly,
            }),
        }
    }
}

/// The definition of `kind` at the floor level: the floor's own, else the
/// plan-wide one a floor that follows the default holds a copy of
/// ([`FloorSettings::platform_inherited`], refreshed by
/// [`FloorSettings::sync_thicknesses`]), else the floor's single thickness
/// read as one layer.
pub fn resolve_floor(kind: AssemblyKind, floor: &FloorSettings) -> Resolved {
    match floor.platform.slot(kind) {
        AssemblySlot::Own(a) => Resolved {
            assembly: a.clone(),
            source: Source::Floor,
        },
        AssemblySlot::Default => match floor.platform_inherited.slot(kind) {
            AssemblySlot::Own(a) => Resolved {
                assembly: a.clone(),
                source: Source::PlanWide,
            },
            _ => Resolved {
                assembly: floor_legacy(kind, floor),
                source: Source::Legacy,
            },
        },
        AssemblySlot::Legacy => Resolved {
            assembly: floor_legacy(kind, floor),
            source: Source::Legacy,
        },
    }
}

/// The definition of `kind` for a room: its own, else the floor level's when
/// it follows the default, else what the room stored the older way (a finish
/// thickness, a Define layer stack).
pub fn resolve(kind: AssemblyKind, floor: &FloorSettings, room: Option<&RoomMisc>) -> Resolved {
    let Some(m) = room else {
        return resolve_floor(kind, floor);
    };
    match m.assemblies.slot(kind) {
        AssemblySlot::Own(a) => Resolved {
            assembly: a.clone(),
            source: Source::Room,
        },
        AssemblySlot::Default => resolve_floor(kind, floor),
        AssemblySlot::Legacy => match kind {
            AssemblyKind::FloorStructure if !m.floor_structure.is_empty() => Resolved {
                assembly: Assembly::from_structure_of(kind, &m.floor_structure),
                source: Source::Legacy,
            },
            AssemblyKind::CeilingStructure if !m.ceiling_structure.is_empty() => Resolved {
                assembly: Assembly::from_structure_of(kind, &m.ceiling_structure),
                source: Source::Legacy,
            },
            AssemblyKind::FloorFinish => Resolved {
                assembly: kind.legacy_layer(m.floor_finish_thickness),
                source: Source::Legacy,
            },
            AssemblyKind::CeilingFinish => Resolved {
                assembly: kind.legacy_layer(m.ceiling_finish_thickness),
                source: Source::Legacy,
            },
            _ => resolve_floor(kind, floor),
        },
    }
}

/// The floor's single thickness of `kind` read as one layer.
fn floor_legacy(kind: AssemblyKind, f: &FloorSettings) -> Assembly {
    let t = match kind {
        AssemblyKind::FloorStructure => f.floor_structure_thickness,
        AssemblyKind::FloorFinish => f.floor_finish_thickness,
        AssemblyKind::CeilingStructure => f.ceiling_structure_thickness,
        _ => f.ceiling_finish_thickness,
    };
    kind.legacy_layer(t)
}

/// The number a legacy thickness field holds for an assembly of `kind`: the
/// whole thickness, except a ceiling finish, whose field is the surface the
/// platform sits on (the drop hangs below the finished ceiling).
pub fn legacy_value(kind: AssemblyKind, a: &Assembly) -> f64 {
    match kind {
        AssemblyKind::CeilingFinish => a.surface_thickness(),
        _ => a.total_thickness(),
    }
}

impl FloorSettings {
    /// Writes the resolved totals of the four definitions into the single
    /// thickness fields the rest of the program reads (see the module docs).
    /// A slot that is still Legacy keeps its field. Returns whether any field
    /// changed.
    pub fn sync_thicknesses(&mut self, lib: &AssemblyLibrary) -> bool {
        let mut changed = false;
        // A floor that follows the default holds a copy of the plan-wide
        // definition, so the 3D builders need only the floor.
        for kind in AssemblyKind::PLATFORM {
            let copy = match (self.platform.slot(kind), lib.plan_wide.slot(kind)) {
                (AssemblySlot::Default, own @ AssemblySlot::Own(_)) => own.clone(),
                _ => AssemblySlot::Legacy,
            };
            if *self.platform_inherited.slot(kind) != copy {
                self.platform_inherited.set(kind, copy);
                changed = true;
            }
        }
        for kind in AssemblyKind::PLATFORM {
            if self.platform.slot(kind).is_legacy() {
                continue;
            }
            let value = legacy_value(kind, &resolve_floor(kind, self).assembly);
            let field = match kind {
                AssemblyKind::FloorStructure => &mut self.floor_structure_thickness,
                AssemblyKind::FloorFinish => &mut self.floor_finish_thickness,
                AssemblyKind::CeilingStructure => &mut self.ceiling_structure_thickness,
                _ => &mut self.ceiling_finish_thickness,
            };
            if (*field - value).abs() > 1e-9 {
                *field = value;
                changed = true;
            }
        }
        changed
    }
}

impl RoomMisc {
    /// Writes the resolved definitions into the older fields (finish
    /// thicknesses, Define layer stacks) so readers of those keep working.
    /// Legacy slots keep their fields. Returns whether any field changed.
    pub fn sync_legacy(&mut self, floor: &FloorSettings) -> bool {
        let mut changed = false;
        for kind in AssemblyKind::PLATFORM {
            if self.assemblies.slot(kind).is_legacy() {
                continue;
            }
            let own = matches!(self.assemblies.slot(kind), AssemblySlot::Own(_));
            let a = resolve(kind, floor, Some(self)).assembly;
            match kind {
                AssemblyKind::FloorStructure => {
                    // A room that follows the default keeps an empty stack
                    // ("follows the floor's default platform").
                    let want = if own {
                        a.to_structure_of(kind)
                    } else {
                        Vec::new()
                    };
                    if self.floor_structure != want {
                        self.floor_structure = want;
                        changed = true;
                    }
                }
                AssemblyKind::CeilingStructure => {
                    let want = if own {
                        a.to_structure_of(kind)
                    } else {
                        Vec::new()
                    };
                    if self.ceiling_structure != want {
                        self.ceiling_structure = want;
                        changed = true;
                    }
                }
                AssemblyKind::FloorFinish => {
                    let v = legacy_value(kind, &a);
                    if (self.floor_finish_thickness - v).abs() > 1e-9 {
                        self.floor_finish_thickness = v;
                        changed = true;
                    }
                }
                _ => {
                    let v = legacy_value(kind, &a);
                    if (self.ceiling_finish_thickness - v).abs() > 1e-9 {
                        self.ceiling_finish_thickness = v;
                        changed = true;
                    }
                }
            }
        }
        changed
    }
}

impl crate::model::Project {
    /// Brings the single-thickness fields (floor rise, finish thicknesses,
    /// room stacks) in step with the layered definitions after any of them
    /// changed, and restacks the floors when a platform thickness moved.
    /// Returns whether anything changed. Plans with no layered definition are
    /// untouched.
    pub fn sync_platform_mirrors(&mut self) -> bool {
        let lib = self.assemblies.clone();
        let (mut floor_moved, mut changed) = (false, false);
        for f in &mut self.floors {
            if f.settings.sync_thicknesses(&lib) {
                floor_moved = true;
            }
            let settings = f.settings.clone();
            for n in &mut f.room_names {
                if let Some(m) = n.misc.as_mut() {
                    changed |= m.sync_legacy(&settings);
                }
            }
        }
        if floor_moved {
            self.restack_floors();
        }
        changed || floor_moved
    }

    /// After floor `idx` got new Floor Defaults (`old` are the ones it had):
    /// a named room that still holds the finish thickness the floor had, and
    /// has no definition of its own, starts to follow the floor's layered
    /// finish, the way a Chief room follows its floor's defaults. Rooms with
    /// a finish of their own keep it. Returns how many rooms changed.
    pub fn rooms_follow_floor_finish(&mut self, idx: usize, old: &FloorSettings) -> usize {
        let Some(f) = self.floors.get_mut(idx) else {
            return 0;
        };
        let new = f.settings.clone();
        let mut n = 0;
        for room in &mut f.room_names {
            let Some(m) = room.misc.as_mut() else {
                continue;
            };
            for (kind, held, was) in [
                (
                    AssemblyKind::FloorFinish,
                    m.floor_finish_thickness,
                    old.floor_finish_thickness,
                ),
                (
                    AssemblyKind::CeilingFinish,
                    m.ceiling_finish_thickness,
                    old.ceiling_finish_thickness,
                ),
            ] {
                if m.assemblies.slot(kind).is_legacy()
                    && !new.platform.slot(kind).is_legacy()
                    && (held - was).abs() < 1e-9
                {
                    m.assemblies.set(kind, AssemblySlot::Default);
                    n += 1;
                }
            }
        }
        n
    }

    /// Sets the plan-wide definition of a platform `kind` (Default Settings >
    /// Floor/Ceiling Platform); `None` takes it back to nothing defined.
    /// Floors that still hold the thickness the plan-wide level had before
    /// start following it, the way a Chief floor follows its defaults.
    pub fn set_plan_wide_assembly(&mut self, kind: AssemblyKind, assembly: Option<Assembly>) {
        if !kind.is_platform() {
            return;
        }
        let before = match self.assemblies.plan_wide.slot(kind) {
            AssemblySlot::Own(a) => legacy_value(kind, a),
            _ => legacy_value(kind, &floor_legacy(kind, &FloorSettings::default())),
        };
        for f in &mut self.floors {
            let current = legacy_value(kind, &floor_legacy(kind, &f.settings));
            if f.settings.platform.slot(kind).is_legacy() && (current - before).abs() < 1e-9 {
                f.settings.platform.set(kind, AssemblySlot::Default);
            }
        }
        self.assemblies.plan_wide.set(
            kind,
            assembly.map_or(AssemblySlot::Legacy, AssemblySlot::Own),
        );
        self.sync_platform_mirrors();
    }
}

/// The three definitions of one roof plane (Structure panel: Roof Surface,
/// Roof Structure, Roof Ceiling Finish; manual pp. 1088-1091). Layers are
/// listed from the top, as everywhere.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RoofLayers {
    pub surface: Assembly,
    pub structure: Assembly,
    pub ceiling_finish: Assembly,
}

/// What the roof builders read from a plane's [`RoofLayers`]: the same
/// numbers the older Define sheet held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoofNumbers {
    pub trusses: bool,
    /// Purlins are stored on the Framing layer; no builder reads them yet.
    pub purlins: bool,
    pub member_width: Option<f64>,
    pub member_depth: Option<f64>,
    pub spacing: Option<f64>,
    /// Sheathing-role layers of the structure, inches.
    pub sheathing: f64,
    /// Everything else over the framing: the surface and any other layer of
    /// the structure, inches.
    pub roofing: f64,
}

impl RoofLayers {
    /// Layers for a plane described by the older numbers: a surface, a
    /// sheathing layer and a framing layer.
    pub fn from_numbers(
        roofing: f64,
        sheathing: f64,
        depth: f64,
        width: f64,
        spacing: f64,
        trusses: bool,
    ) -> Self {
        let mut framing = AssemblyLayer::new(
            if trusses { "Truss Top Chord" } else { "Rafter" },
            LayerRole::Framing,
            depth,
        );
        framing.framing = Some(FramingSpec {
            method: if trusses {
                FramingMethod::Trusses
            } else {
                FramingMethod::Rafters
            },
            construction: FramingConstruction::Lumber,
            width,
            spacing,
        });
        let mut structure = Vec::new();
        if sheathing > 0.0 {
            structure.push(AssemblyLayer::new(
                "Roof Sheathing",
                LayerRole::Sheathing,
                sheathing,
            ));
        }
        structure.push(framing);
        Self {
            surface: Assembly::from_thickness("Roofing", LayerRole::Finish, roofing),
            structure: Assembly::new(structure),
            ceiling_finish: AssemblyKind::RoofCeilingFinish.builtin(),
        }
    }

    pub fn get(&self, kind: AssemblyKind) -> &Assembly {
        match kind {
            AssemblyKind::RoofSurface => &self.surface,
            AssemblyKind::RoofCeilingFinish => &self.ceiling_finish,
            _ => &self.structure,
        }
    }

    /// Sets one of the three definitions; other kinds are ignored.
    pub fn set(&mut self, kind: AssemblyKind, a: Assembly) {
        match kind {
            AssemblyKind::RoofSurface => self.surface = a,
            AssemblyKind::RoofStructure => self.structure = a,
            AssemblyKind::RoofCeilingFinish => self.ceiling_finish = a,
            _ => {}
        }
    }

    /// Total thickness of the plane along its normal: surface plus structure.
    pub fn thickness(&self) -> f64 {
        self.surface.total_thickness() + self.structure.total_thickness()
    }

    pub fn numbers(&self) -> RoofNumbers {
        let framing = self.structure.framing_layer();
        let spec = framing.and_then(|l| l.framing);
        let sheathing: f64 = self
            .structure
            .layers
            .iter()
            .filter(|l| l.role == LayerRole::Sheathing)
            .map(|l| l.thickness.max(0.0))
            .sum();
        let depth = framing.map(|l| l.thickness.max(0.0));
        let other = self.structure.total_thickness() - sheathing - depth.unwrap_or(0.0);
        RoofNumbers {
            trusses: spec.is_some_and(|f| f.method == FramingMethod::Trusses),
            purlins: spec.is_some_and(|f| f.method == FramingMethod::Purlins),
            member_width: spec.map(|f| f.width),
            member_depth: depth,
            spacing: spec.map(|f| f.spacing),
            sheathing,
            roofing: self.surface.total_thickness() + other.max(0.0),
        }
    }

    /// Why the layers cannot be used, if they cannot.
    pub fn error(&self) -> Option<String> {
        self.surface
            .error()
            .or_else(|| self.structure.error())
            .or_else(|| self.ceiling_finish.error())
    }
}

/// Dropped-ceiling height math for a room whose Ceiling Height is
/// `ceiling_height` and whose Ceiling Finish is `finish`: where the finished
/// (hung) ceiling is, and where the ceiling platform's underside is. The wall
/// tops and the platform do not move; only the finished ceiling lowers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CeilingHeights {
    /// Height of the finished ceiling above the room's floor, inches.
    pub finished: f64,
    /// Height of the underside of the ceiling platform, inches.
    pub platform_bottom: f64,
}

pub fn ceiling_heights(ceiling_height: f64, finish: &Assembly) -> CeilingHeights {
    CeilingHeights {
        finished: ceiling_height - finish.drop(),
        platform_bottom: ceiling_height + finish.surface_thickness(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, RoomName};

    fn floor_12() -> Assembly {
        Assembly::new(vec![
            AssemblyLayer::new("3/4 OSB", LayerRole::Sheathing, 0.75),
            AssemblyLayer::new("2x12", LayerRole::Framing, 11.25),
        ])
    }

    fn dropped() -> Assembly {
        Assembly::new(vec![
            AssemblyLayer::new("Plenum", LayerRole::AirGap, 11.125),
            {
                let mut hat = AssemblyLayer::new("Hat Channel", LayerRole::Framing, 0.875);
                hat.framing = Some(FramingSpec {
                    method: FramingMethod::Joists,
                    construction: FramingConstruction::HatChannel,
                    width: 1.0,
                    spacing: 16.0,
                });
                hat
            },
            AssemblyLayer::new("Drywall", LayerRole::Finish, 0.5),
        ])
    }

    #[test]
    fn layer_totals_add_up_and_the_drop_is_the_plenum_and_framing() {
        let f = floor_12();
        assert_eq!(f.total_thickness(), 12.0);
        let (spec, depth) = f.framing().unwrap();
        assert_eq!(depth, 11.25);
        assert_eq!(spec.spacing, 16.0);
        let d = dropped();
        assert_eq!(d.total_thickness(), 12.5);
        assert_eq!(d.drop(), 12.0);
        assert_eq!(d.surface_thickness(), 0.5);
        assert_eq!(Assembly::default().total_thickness(), 0.0);
    }

    #[test]
    fn a_section_stacks_the_layers_down_from_the_top() {
        let a = floor_12();
        let spans = a.spans_down(0.0);
        assert_eq!((spans[0].y0, spans[0].y1), (-0.75, 0.0));
        assert_eq!((spans[1].y0, spans[1].y1), (-12.0, -0.75));
        assert_eq!(spans[1].layer.role, LayerRole::Framing);
        assert!(Assembly::default().spans_down(5.0).is_empty());
    }

    #[test]
    fn insert_remove_and_move_keep_the_order() {
        let mut a = floor_12();
        a.insert(
            1,
            AssemblyLayer::new("Underlayment", LayerRole::Standard, 0.25),
        );
        let names: Vec<_> = a.layers.iter().map(|l| l.material.as_str()).collect();
        assert_eq!(names, ["3/4 OSB", "Underlayment", "2x12"]);
        assert_eq!(a.move_down(0), Some(1));
        assert_eq!(a.move_up(0), None);
        assert_eq!(a.layers[0].material, "Underlayment");
        assert!(a.remove(0));
        assert!(!a.remove(5));
        assert_eq!(a.total_thickness(), 12.0);
    }

    #[test]
    fn a_framing_role_gets_framing_options_and_loses_them_again() {
        let mut l = AssemblyLayer::new("Plenum", LayerRole::AirGap, 4.0);
        assert!(l.framing.is_none());
        l.set_role(LayerRole::Framing);
        assert_eq!(l.framing, Some(FramingSpec::default()));
        l.set_role(LayerRole::Finish);
        assert!(l.framing.is_none());
    }

    #[test]
    fn errors_name_the_layer() {
        let mut a = floor_12();
        assert!(a.error().is_none());
        a.layers[1].thickness = -1.0;
        assert!(a.error().unwrap().starts_with("Layer 2"));
        a.layers[1].thickness = 11.25;
        a.layers[1].framing.as_mut().unwrap().spacing = 0.0;
        assert!(a.error().unwrap().contains("spacing"));
    }

    #[test]
    fn dropped_ceiling_height_math_keeps_wall_tops_and_platform() {
        let h = ceiling_heights(96.0, &dropped());
        assert_eq!(h.finished, 96.0 - 12.0);
        assert_eq!(h.platform_bottom, 96.5);
        // A plain drywall finish is the old rule: the finished ceiling stays
        // at the Ceiling Height and the platform starts a finish above it.
        let plain = ceiling_heights(96.0, &AssemblyKind::CeilingFinish.builtin());
        assert_eq!(plain.finished, 96.0);
        assert_eq!(plain.platform_bottom, 96.625);
    }

    #[test]
    fn the_default_chain_runs_room_then_floor_then_plan_wide_then_legacy() {
        let mut lib = AssemblyLibrary::default();
        let mut floor = FloorSettings::default();
        // Nothing layered: the floor's thickness is one layer.
        let r = resolve(AssemblyKind::FloorStructure, &floor, None);
        assert_eq!(r.source, Source::Legacy);
        assert_eq!(r.assembly.total_thickness(), 10.25);
        assert!(!r.is_layered());
        // Plan-wide only helps a floor that follows the default.
        lib.plan_wide
            .set(AssemblyKind::FloorStructure, AssemblySlot::Own(floor_12()));
        floor.sync_thicknesses(&lib);
        assert_eq!(
            resolve_floor(AssemblyKind::FloorStructure, &floor).source,
            Source::Legacy
        );
        floor
            .platform
            .set(AssemblyKind::FloorStructure, AssemblySlot::Default);
        floor.sync_thicknesses(&lib);
        let r = resolve_floor(AssemblyKind::FloorStructure, &floor);
        assert_eq!(
            (r.source, r.assembly.total_thickness()),
            (Source::PlanWide, 12.0)
        );
        assert_eq!(floor.floor_structure_thickness, 12.0);
        // The floor's own definition beats the plan-wide one.
        let own = Assembly::from_thickness("Slab", LayerRole::Standard, 6.0);
        floor
            .platform
            .set(AssemblyKind::FloorStructure, AssemblySlot::Own(own));
        floor.sync_thicknesses(&lib);
        let r = resolve_floor(AssemblyKind::FloorStructure, &floor);
        assert_eq!(
            (r.source, r.assembly.total_thickness()),
            (Source::Floor, 6.0)
        );
        assert!(
            floor.platform_inherited.is_legacy(),
            "no copy kept once it owns"
        );
        // A room that follows the default reads the floor; its own wins.
        let mut misc = RoomMisc::default();
        misc.assemblies
            .set(AssemblyKind::FloorStructure, AssemblySlot::Default);
        let r = resolve(AssemblyKind::FloorStructure, &floor, Some(&misc));
        assert_eq!(r.source, Source::Floor);
        misc.assemblies
            .set(AssemblyKind::FloorStructure, AssemblySlot::Own(floor_12()));
        let r = resolve(AssemblyKind::FloorStructure, &floor, Some(&misc));
        assert_eq!(
            (r.source, r.assembly.total_thickness()),
            (Source::Room, 12.0)
        );
    }

    #[test]
    fn a_room_keeps_what_it_stored_the_old_way() {
        let floor = FloorSettings::default();
        let mut misc = RoomMisc {
            floor_finish_thickness: 0.5,
            ..RoomMisc::default()
        };
        misc.floor_structure = vec![
            StructureLayer::new("Subfloor", 0.75),
            StructureLayer::new("Joist", 9.25),
        ];
        let s = resolve(AssemblyKind::FloorStructure, &floor, Some(&misc));
        assert_eq!(
            (s.source, s.assembly.total_thickness()),
            (Source::Legacy, 10.0)
        );
        assert_eq!(s.assembly.framing().unwrap().1, 9.25);
        let f = resolve(AssemblyKind::FloorFinish, &floor, Some(&misc));
        assert_eq!(f.assembly.total_thickness(), 0.5);
        // A zero finish is no layer at all.
        misc.floor_finish_thickness = 0.0;
        assert!(resolve(AssemblyKind::FloorFinish, &floor, Some(&misc))
            .assembly
            .is_empty());
    }

    #[test]
    fn syncing_writes_the_totals_into_the_old_fields() {
        let mut lib = AssemblyLibrary::default();
        let mut floor = FloorSettings::default();
        floor
            .platform
            .set(AssemblyKind::FloorStructure, AssemblySlot::Own(floor_12()));
        floor
            .platform
            .set(AssemblyKind::CeilingFinish, AssemblySlot::Own(dropped()));
        assert!(floor.sync_thicknesses(&lib));
        assert_eq!(floor.floor_structure_thickness, 12.0);
        // The ceiling finish field is the surface; the drop hangs below.
        assert_eq!(floor.ceiling_finish_thickness, 0.5);
        assert_eq!(
            floor.floor_finish_thickness, 0.75,
            "legacy slots keep theirs"
        );
        assert!(!floor.sync_thicknesses(&lib), "already in step");
        // A floor that follows the plan-wide definition tracks it.
        let mut follower = FloorSettings::default();
        follower
            .platform
            .set(AssemblyKind::FloorFinish, AssemblySlot::Default);
        lib.plan_wide.set(
            AssemblyKind::FloorFinish,
            AssemblySlot::Own(Assembly::new(vec![
                AssemblyLayer::new("Tile", LayerRole::Finish, 0.375),
                AssemblyLayer::new("Backerboard", LayerRole::Standard, 0.5),
            ])),
        );
        follower.sync_thicknesses(&lib);
        assert_eq!(follower.floor_finish_thickness, 0.875);
        assert_eq!(
            resolve_floor(AssemblyKind::FloorFinish, &follower)
                .assembly
                .layers
                .len(),
            2
        );
    }

    #[test]
    fn rooms_at_the_old_floor_finish_follow_a_new_layered_one() {
        let mut p = Project::new("t");
        let old = p.floors[0].settings.clone();
        let keeps = RoomName {
            misc: Some(RoomMisc {
                floor_finish_thickness: 0.25,
                ..RoomMisc::default()
            }),
            ..RoomName::default()
        };
        let follows = RoomName {
            misc: Some(RoomMisc {
                floor_finish_thickness: old.floor_finish_thickness,
                ceiling_finish_thickness: old.ceiling_finish_thickness,
                ..RoomMisc::default()
            }),
            ..RoomName::default()
        };
        p.floors[0].room_names = vec![keeps, follows];
        p.floors[0].settings.platform.set(
            AssemblyKind::FloorFinish,
            AssemblySlot::Own(Assembly::new(vec![
                AssemblyLayer::new("Tile", LayerRole::Finish, 0.375),
                AssemblyLayer::new("Backerboard", LayerRole::Standard, 0.5),
            ])),
        );
        p.sync_platform_mirrors();
        assert_eq!(p.rooms_follow_floor_finish(0, &old), 1);
        let rooms = &p.floors[0].room_names;
        assert!(rooms[0].misc.as_ref().unwrap().assemblies.is_legacy());
        let f = rooms[1].misc.as_ref().unwrap();
        assert_eq!(f.assemblies.floor_finish, AssemblySlot::Default);
        assert!(
            f.assemblies.ceiling_finish.is_legacy(),
            "the ceiling finish is unchanged"
        );
        p.sync_platform_mirrors();
        let f = p.floors[0].room_names[1].misc.as_ref().unwrap();
        assert!((f.floor_finish_thickness - 0.875).abs() < 1e-9);
    }

    #[test]
    fn the_old_ceiling_stack_reads_from_the_top() {
        // The older Ceiling Structure stack was listed from the underside up.
        let old = vec![
            StructureLayer::new("Drywall", 0.5),
            StructureLayer::new("Joist", 5.5),
        ];
        let a = Assembly::from_structure_of(AssemblyKind::CeilingStructure, &old);
        assert_eq!(a.layers[0].material, "Joist");
        assert_eq!(a.layers[0].role, LayerRole::Framing);
        assert_eq!(a.to_structure_of(AssemblyKind::CeilingStructure), old);
    }

    #[test]
    fn a_room_sync_keeps_a_follower_empty_and_an_owner_mirrored() {
        let floor = FloorSettings::default();
        let mut misc = RoomMisc::default();
        misc.assemblies
            .set(AssemblyKind::FloorStructure, AssemblySlot::Own(floor_12()));
        misc.assemblies
            .set(AssemblyKind::CeilingStructure, AssemblySlot::Default);
        misc.ceiling_structure = vec![StructureLayer::new("Stale", 1.0)];
        assert!(misc.sync_legacy(&floor));
        assert_eq!(misc.floor_structure.len(), 2);
        assert!(misc.ceiling_structure.is_empty());
    }

    #[test]
    fn setting_the_plan_wide_definition_moves_the_floors_still_at_the_old_one() {
        let mut p = Project::new("t");
        p.build_new_floor(true);
        // Floor 2 was customised by hand, floor 1 is still at the default.
        p.floors[1].settings.floor_structure_thickness = 14.0;
        p.set_plan_wide_assembly(AssemblyKind::FloorStructure, Some(floor_12()));
        assert_eq!(
            p.floors[0].settings.platform.floor_structure,
            AssemblySlot::Default
        );
        assert!(p.floors[1].settings.platform.floor_structure.is_legacy());
        assert_eq!(p.floors[0].settings.floor_structure_thickness, 12.0);
        assert_eq!(p.floors[1].settings.floor_structure_thickness, 14.0);
        // The rise to floor 2 follows floor 2's own platform.
        assert!(
            (p.floors[1].elevation - (p.floors[0].elevation + p.floors[0].ceiling_height + 14.0))
                .abs()
                < 1e-9
        );
        p.set_plan_wide_assembly(AssemblyKind::FloorStructure, None);
        assert!(p.assemblies.is_empty());
    }

    #[test]
    fn a_plan_with_no_assemblies_writes_none_and_round_trips_byte_stable() {
        let mut p = Project::new("legacy");
        p.floors[0].settings.floor_structure_thickness = 11.0;
        let n = RoomName {
            misc: Some(RoomMisc {
                floor_finish_thickness: 0.5,
                floor_structure: vec![StructureLayer::new("Joist", 9.25)],
                ..RoomMisc::default()
            }),
            ..RoomName::default()
        };
        p.floors[0].room_names.push(n);
        let json = p.to_json().unwrap();
        assert!(!json.contains("assemblies"));
        assert!(!json.contains("platform"));
        assert!(!json.contains("Own"));
        let back = Project::from_json(&json).unwrap();
        assert_eq!(
            back.to_json().unwrap(),
            json,
            "load then save changes nothing"
        );
        // The one-layer view of the old numbers is what the chain returns.
        let m = back.floors[0].room_names[0].misc.as_ref();
        let r = resolve(AssemblyKind::FloorFinish, &back.floors[0].settings, m);
        assert_eq!(r.assembly.total_thickness(), 0.5);
        assert!(!r.is_layered());
    }

    #[test]
    fn layered_definitions_round_trip() {
        let mut p = Project::new("layered");
        p.floors[0]
            .settings
            .platform
            .set(AssemblyKind::FloorStructure, AssemblySlot::Own(floor_12()));
        p.assemblies
            .plan_wide
            .set(AssemblyKind::CeilingFinish, AssemblySlot::Own(dropped()));
        p.assemblies
            .save_named(AssemblyKind::CeilingFinish, "Hat channel", dropped());
        p.sync_platform_mirrors();
        let json = p.to_json().unwrap();
        let back = Project::from_json(&json).unwrap();
        assert_eq!(
            back.floors[0].settings.platform,
            p.floors[0].settings.platform
        );
        assert_eq!(back.assemblies, p.assemblies);
        assert_eq!(back.to_json().unwrap(), json);
        assert!(back
            .assemblies
            .named_of(AssemblyKind::CeilingFinish, "Hat channel")
            .is_some());
    }

    #[test]
    fn roof_layers_hand_the_old_numbers_to_the_builders() {
        let l = RoofLayers::from_numbers(0.5, 0.5, 5.5, 1.5, 24.0, false);
        assert_eq!(l.thickness(), 6.5);
        let n = l.numbers();
        assert_eq!(
            (n.member_depth, n.member_width, n.spacing),
            (Some(5.5), Some(1.5), Some(24.0))
        );
        assert_eq!((n.sheathing, n.roofing), (0.5, 0.5));
        assert!(!n.trusses && !n.purlins);
        // An underlayment layer in the structure counts with the roofing; a
        // truss and a purlin option are read from the framing layer.
        let mut l = l;
        l.structure.insert(
            0,
            AssemblyLayer::new("Underlayment", LayerRole::Standard, 0.125),
        );
        l.structure.layers[2].framing.as_mut().unwrap().method = FramingMethod::Purlins;
        let n = l.numbers();
        assert!((n.roofing - 0.625).abs() < 1e-9);
        assert!(n.purlins && !n.trusses);
        assert!(
            (l.thickness() - 6.625).abs() < 1e-9,
            "thickness is the sum of the layers"
        );
        l.structure.layers[2].framing.as_mut().unwrap().method = FramingMethod::Trusses;
        assert!(l.numbers().trusses);
        // No framing layer: the builders keep their own member sizes.
        l.structure.layers.retain(|x| x.role != LayerRole::Framing);
        assert_eq!(l.numbers().member_depth, None);
    }

    #[test]
    fn every_kind_has_a_builtin_definition() {
        for k in AssemblyKind::ALL {
            let b = k.builtin();
            assert!(!b.is_empty(), "{}", k.label());
            assert!(b.error().is_none());
        }
        assert_eq!(
            AssemblyKind::FloorStructure.builtin().total_thickness(),
            10.25
        );
        assert_eq!(
            AssemblyKind::CeilingFinish.builtin().total_thickness(),
            0.625
        );
    }
}
