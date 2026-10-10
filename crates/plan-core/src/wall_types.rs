//! Wall Type Definitions depth (Round 16, brief 12; manual pp. 414-421).
//!
//! A [`WallTypeDef`] is an ordered list of layers from the exterior face to
//! the interior face. The two-field layers an older plan stored (name,
//! thickness, main flag, material) still load; everything this module adds
//! lives in two serde-default records so such a plan reads and saves the same:
//!
//! * [`WallLayerSpec`] (`WallLayer::spec`): the Wall Layer Specification
//!   dialog's values, the Role, the Extension, the fill that draws the layer
//!   in plan (the poché source) and the framing assembly;
//! * [`WallTypeProps`] (`WallTypeDef::props`): the Wall Properties tab, the
//!   Energy Values, the Partition Wall and Room Divider flags and the layers
//!   the platforms, dimensions, foundation and roof align to.
//!
//! # Main layers
//!
//! A type may have several Main layers; they sit together in the middle
//! ([`WallTypeDef::group_of`] names the three sections of the dialog's
//! table). The outermost Main layer is where windows, platforms, roof
//! baselines and the Dimension Layer live by default; walls join at the
//! interior surface of the innermost one. A change of the total thickness is
//! taken by the outermost Main layer ([`WallTypeDef::set_total`]).
//!
//! # Managing the list of types
//!
//! [`import_types`] merges the types of another plan (a same-named type that
//! differs arrives as `Name_2`), [`unused_names`] and [`delete_unused`] are
//! Delete All Unused, and [`LibraryWall`] is a wall saved in the Library
//! ([`add_to_library`] adds `_2` on a name collision).

use crate::assemblies::LayerRole;
use crate::defaults::{PlanDefaults, WallLayer, WallTypeDef};
use crate::fill_styles::FillStyle;
use crate::geometry::Point;
use crate::model::{Wall, WallKind};
use crate::walls::{WallClass, WallSpec};
use crate::Project;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The thinnest a Main layer may be, inches (manual correction, DECISIONS 47).
pub const MIN_MAIN_LAYER: f64 = 0.0625;

/// The thinnest any other layer may be, inches (the manual says very thin
/// layers can be made but a thickness of 0 should be avoided).
pub const MIN_LAYER: f64 = 0.0625;

/// The Main layer count over which Move Up and Move Down are offered.
pub const SINGLE_MAIN: usize = 1;

/// The section of the Wall Layers table a layer is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerGroup {
    Exterior,
    Main,
    Interior,
}

impl LayerGroup {
    pub fn label(self) -> &'static str {
        match self {
            LayerGroup::Exterior => "Exterior Layers",
            LayerGroup::Main => "Main Layers",
            LayerGroup::Interior => "Interior Layers",
        }
    }
}

/// The edge line of a wall layer (Wall Layer Specification > Line Style).
/// `None` follows the layer's display layer ("By Layer").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayerLine {
    /// Display layer of the layer's lines; `None` is the default layer.
    pub display_layer: Option<String>,
    pub color: Option<[u8; 3]>,
    pub style: Option<String>,
    /// Line weight, hundredths of a millimetre.
    pub weight: Option<u32>,
}

/// The framing assembly of a wall layer whose role is Framing (Wall Layer
/// Specification > General). A `None` size follows the plan-wide Framing
/// Defaults (the Active Defaults icon of the dialog).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayerFraming {
    /// Framing Member Definition names; empty follows the defaults.
    pub stud_construction: String,
    pub stud_width: Option<f64>,
    pub stud_spacing: Option<f64>,
    pub top_plate_construction: String,
    pub top_plate_width: Option<f64>,
    pub top_plate_count: Option<u32>,
    pub bottom_plate_construction: String,
    pub bottom_plate_width: Option<f64>,
    pub bottom_plate_count: Option<u32>,
    /// The longest board of a plate; `None` runs a plate the wall's length.
    pub max_plate_length: Option<f64>,
    /// Horizontal framing (girts) instead of vertical studs.
    pub horizontal: bool,
    /// Where the girts start, an absolute height, inches.
    pub bottom_run_elevation: f64,
    pub max_girt_length: Option<f64>,
    /// Materials panel: what camera views show for studs, plates and treated
    /// framing; empty follows the defaults.
    pub stud_material: String,
    pub plate_material: String,
    pub treated_material: String,
}

/// Everything about a wall layer beyond name, thickness, Main flag and
/// material.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallLayerSpec {
    /// The layer's role; `None` is a layer an older plan stored, whose role
    /// [`WallLayer::resolved_role`] reads from its material.
    pub role: Option<LayerRole>,
    /// How far an Exterior layer reaches below its default bottom, inches
    /// (the Extension column; the highest one is the Brick Ledge Depth).
    pub extension: f64,
    /// The fill that draws the layer in plan view (the Fill column); `None`
    /// draws the layer plain, or with a Layer Fill Style assigned elsewhere.
    pub fill: Option<FillStyle>,
    /// Auto Detail draws insulation boxes in this layer.
    pub auto_detail_insulation: bool,
    /// Present for a Framing layer.
    pub framing: Option<LayerFraming>,
    /// The 3D Cladding profile (library name) of a Cladding layer.
    pub cladding: String,
    pub line: LayerLine,
}

impl WallLayerSpec {
    pub fn is_default(&self) -> bool {
        *self == WallLayerSpec::default()
    }
}

/// The Energy Values of a wall type (REScheck, manual p. 418).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallEnergy {
    /// "Framed": the on-center spacing comes from the framing layer.
    pub framed: bool,
    pub cavity_r: f64,
    pub continuous_r: f64,
}

/// The layers a wall type aligns other things to, as indices into
/// [`WallTypeDef::layers`] (the exterior surface of the layer). `None` is the
/// default, the outermost Main layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallAlign {
    /// Floor and ceiling platforms build to this layer.
    pub build_platform: Option<usize>,
    /// Deck platforms; `None` is "Match Platform to Exterior".
    pub deck_platform: Option<usize>,
    /// The Dimension Layer.
    pub dimension: Option<usize>,
    /// The Main layer of the foundation wall below aligns to this layer.
    pub foundation: Option<usize>,
    /// Offset of that foundation wall: positive toward the exterior.
    pub foundation_offset: f64,
    /// Adjacent roof planes build to this layer.
    pub roof: Option<usize>,
}

impl WallAlign {
    /// Calls `f` on every layer index held, so an edit of the layer list can
    /// keep them pointing at the same layers.
    pub fn each_index(&mut self, mut f: impl FnMut(&mut Option<usize>)) {
        f(&mut self.build_platform);
        f(&mut self.deck_platform);
        f(&mut self.dimension);
        f(&mut self.foundation);
        f(&mut self.roof);
    }
}

/// The Wall Properties tab and the flags of a wall type.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallTypeProps {
    /// Partition Wall: the wall stops at the surface layers of floors,
    /// ceilings and other walls instead of cutting through them (a glass
    /// shower wall).
    pub partition: bool,
    /// Room Divider: 0 in thick, drawn as a dashed pair of lines.
    pub room_divider: bool,
    /// Depth of the brick ledge under the Exterior layers, inches; follows
    /// the highest Extension ([`WallTypeDef::sync_derived`]).
    pub brick_ledge_depth: f64,
    pub align: WallAlign,
    pub energy: WallEnergy,
}

impl WallTypeProps {
    pub fn is_default(&self) -> bool {
        *self == WallTypeProps::default()
    }
}

impl WallLayer {
    /// The layer's role: the stored one, else what an older plan's layer
    /// reads as (a Main layer of framing material is Framing, an air space is
    /// an Air Gap, the rest Standard).
    pub fn resolved_role(&self) -> LayerRole {
        if let Some(r) = self.spec.role {
            return r;
        }
        let material = self.material.to_ascii_lowercase();
        let name = self.name.to_ascii_lowercase();
        if self.is_main && (material.contains("framing") || name.contains("framing")) {
            LayerRole::Framing
        } else if material == "air" || name.contains("air space") || name.contains("air gap") {
            LayerRole::AirGap
        } else {
            LayerRole::Standard
        }
    }

    /// Sets the role. A Framing layer gets framing options; any other loses
    /// them.
    pub fn set_role(&mut self, role: LayerRole) {
        self.spec.role = Some(role);
        match role {
            LayerRole::Framing => {
                self.spec.framing.get_or_insert_with(LayerFraming::default);
            }
            _ => self.spec.framing = None,
        }
    }

    /// An Air Gap shows nowhere in 3D and is not in the Materials List.
    pub fn is_air_gap(&self) -> bool {
        self.resolved_role() == LayerRole::AirGap
    }
}

/// What is wrong with a wall type definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeProblem {
    NoLayers,
    NoMainLayer,
    /// The Main layers are not next to each other.
    MainLayersSplit,
    /// A Main layer (index) thinner than [`MIN_MAIN_LAYER`].
    MainTooThin(usize),
    /// A layer (index) thinner than [`MIN_LAYER`] (a Room Divider may be 0).
    LayerTooThin(usize),
    /// An Air Gap (index) cannot be a Main layer.
    AirGapIsMain(usize),
    /// A layer (index) has no name.
    Unnamed(usize),
    /// Only an Exterior layer (index) can be extended.
    ExtensionNotExterior(usize),
}

impl TypeProblem {
    pub fn message(&self) -> String {
        match self {
            TypeProblem::NoLayers => "A wall type needs at least one layer".into(),
            TypeProblem::NoMainLayer => "A wall type needs a main layer".into(),
            TypeProblem::MainLayersSplit => "The main layers must be next to each other".into(),
            TypeProblem::MainTooThin(i) => {
                format!("Layer {} is a main layer: at least 1/16\" thick", i + 1)
            }
            TypeProblem::LayerTooThin(i) => format!("Layer {} needs a thickness", i + 1),
            TypeProblem::AirGapIsMain(i) => {
                format!("Layer {} is an air gap, not a main layer", i + 1)
            }
            TypeProblem::Unnamed(i) => format!("Layer {} needs a name", i + 1),
            TypeProblem::ExtensionNotExterior(i) => {
                format!("Layer {} is not an exterior layer: no extension", i + 1)
            }
        }
    }
}

impl WallTypeDef {
    /// Indices of the Main layers, exterior first.
    pub fn main_indices(&self) -> Vec<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.is_main)
            .map(|(i, _)| i)
            .collect()
    }

    /// The first and last Main layer.
    pub fn main_span(&self) -> Option<(usize, usize)> {
        let first = self.layers.iter().position(|l| l.is_main)?;
        let last = self.layers.iter().rposition(|l| l.is_main)?;
        Some((first, last))
    }

    /// Which section of the table layer `i` is in. A layer between two Main
    /// layers is in the Main section.
    pub fn group_of(&self, i: usize) -> LayerGroup {
        match self.main_span() {
            Some((first, _)) if i < first => LayerGroup::Exterior,
            Some((_, last)) if i > last => LayerGroup::Interior,
            Some(_) => LayerGroup::Main,
            None => LayerGroup::Main,
        }
    }

    /// Distance from the exterior face to the exterior surface of layer `i`
    /// (`i == layers.len()` is the interior face), inches.
    pub fn depth_to(&self, i: usize) -> f64 {
        self.layers.iter().take(i).map(|l| l.thickness).sum()
    }

    /// Distance from the exterior face to the exterior side of the outermost
    /// Main layer (0 with no Main layer).
    pub fn outer_main_offset(&self) -> f64 {
        self.main_span()
            .map_or(0.0, |(first, _)| self.depth_to(first))
    }

    /// Distance from the exterior face to the interior side of the innermost
    /// Main layer, where walls that join this one stop (the total thickness
    /// with no Main layer).
    pub fn inner_main_offset(&self) -> f64 {
        self.main_span()
            .map_or_else(|| self.thickness(), |(_, last)| self.depth_to(last + 1))
    }

    /// Thickness of the whole Main section, inches.
    pub fn main_thickness(&self) -> f64 {
        self.main_span().map_or(0.0, |(first, last)| {
            self.depth_to(last + 1) - self.depth_to(first)
        })
    }

    /// Total thickness of the layers outside the outermost Main layer.
    pub fn exterior_thickness(&self) -> f64 {
        self.outer_main_offset()
    }

    /// Distance from the exterior face to the exterior surface of the layer
    /// `which` names, or of the outermost Main layer for `None` (or an index
    /// the type no longer has).
    pub fn aligned_offset(&self, which: Option<usize>) -> f64 {
        match which {
            Some(i) if i < self.layers.len() => self.depth_to(i),
            _ => self.outer_main_offset(),
        }
    }

    /// Where floor and ceiling platforms build to.
    pub fn platform_offset(&self) -> f64 {
        self.aligned_offset(self.props.align.build_platform)
    }

    /// Where Deck platforms attach ("Match Platform to Exterior" follows
    /// [`Self::platform_offset`]).
    pub fn deck_platform_offset(&self) -> f64 {
        match self.props.align.deck_platform {
            Some(i) => self.aligned_offset(Some(i)),
            None => self.platform_offset(),
        }
    }

    /// The Dimension Layer: where dimensions locate the wall.
    pub fn dimension_offset(&self) -> f64 {
        self.aligned_offset(self.props.align.dimension)
    }

    /// The layer a foundation wall below aligns its Main layer to, plus the
    /// Foundation Offset (positive toward the exterior, so nearer the
    /// exterior face).
    pub fn foundation_align_offset(&self) -> f64 {
        self.aligned_offset(self.props.align.foundation) - self.props.align.foundation_offset
    }

    /// Where roof planes beside the wall build to.
    pub fn roof_offset(&self) -> f64 {
        self.aligned_offset(self.props.align.roof)
    }

    /// The highest Extension of the Exterior layers, inches.
    pub fn max_extension(&self) -> f64 {
        let Some((first, _)) = self.main_span() else {
            return 0.0;
        };
        self.layers
            .iter()
            .take(first)
            .map(|l| l.spec.extension)
            .fold(0.0, f64::max)
    }

    /// Framed: the Main layer is a Framing layer.
    pub fn is_framed(&self) -> bool {
        self.main_span().is_some_and(|(first, _)| {
            self.layers[first..]
                .iter()
                .take_while(|l| l.is_main)
                .any(|l| l.resolved_role() == LayerRole::Framing)
        })
    }

    /// The material of the outermost Main layer reads as concrete.
    pub fn is_concrete(&self) -> bool {
        self.main_layer()
            .is_some_and(|l| l.material.to_ascii_lowercase().contains("concrete"))
    }

    /// The material of the outermost Main layer reads as masonry.
    pub fn is_masonry(&self) -> bool {
        self.main_layer().is_some_and(|l| {
            let m = l.material.to_ascii_lowercase();
            ["masonry", "brick", "block", "cmu", "stone"]
                .iter()
                .any(|k| m.contains(k))
        })
    }

    /// Brings the derived values up to date: the Brick Ledge Depth follows
    /// the highest Extension, extensions on layers that are not Exterior are
    /// dropped, and alignment indices past the last layer fall back to the
    /// default.
    pub fn sync_derived(&mut self) {
        let first = self.main_span().map_or(0, |(f, _)| f);
        for (i, l) in self.layers.iter_mut().enumerate() {
            if i >= first || l.spec.extension < 0.0 {
                l.spec.extension = 0.0;
            }
        }
        self.props.brick_ledge_depth = self.max_extension();
        let n = self.layers.len();
        self.props.align.each_index(|ix| {
            if ix.is_some_and(|i| i >= n) {
                *ix = None;
            }
        });
    }

    /// Everything wrong with the definition, first problem first.
    pub fn problems(&self) -> Vec<TypeProblem> {
        let mut out = Vec::new();
        if self.layers.is_empty() {
            out.push(TypeProblem::NoLayers);
            return out;
        }
        let Some((first, last)) = self.main_span() else {
            out.push(TypeProblem::NoMainLayer);
            return out;
        };
        if self.layers[first..=last].iter().any(|l| !l.is_main) {
            out.push(TypeProblem::MainLayersSplit);
        }
        for (i, l) in self.layers.iter().enumerate() {
            if l.name.trim().is_empty() {
                out.push(TypeProblem::Unnamed(i));
            }
            if l.is_main && l.thickness < MIN_MAIN_LAYER - 1e-9 && !self.props.room_divider {
                out.push(TypeProblem::MainTooThin(i));
            } else if !l.is_main && l.thickness < MIN_LAYER - 1e-9 {
                out.push(TypeProblem::LayerTooThin(i));
            }
            if l.is_main && l.is_air_gap() {
                out.push(TypeProblem::AirGapIsMain(i));
            }
            if i >= first && l.spec.extension > 0.0 {
                out.push(TypeProblem::ExtensionNotExterior(i));
            }
        }
        out
    }

    /// Sets the total thickness: the outermost Main layer takes the change.
    /// Returns `false` (and changes nothing) when that would leave it thinner
    /// than [`MIN_MAIN_LAYER`] or the type has no Main layer.
    pub fn set_total(&mut self, total: f64) -> bool {
        let Some((first, _)) = self.main_span() else {
            return false;
        };
        let want = self.layers[first].thickness + total - self.thickness();
        if !want.is_finite() || want < MIN_MAIN_LAYER - 1e-9 {
            return false;
        }
        self.layers[first].thickness = want;
        true
    }

    /// A Room Divider type: one 0 in layer.
    pub fn room_divider_type(name: &str) -> WallTypeDef {
        WallTypeDef {
            name: name.into(),
            layers: vec![WallLayer::new("Divider", 0.0, true, "Invisible")],
            kind: WallKind::Interior,
            props: WallTypeProps {
                room_divider: true,
                ..WallTypeProps::default()
            },
        }
    }

    /// The same definition with the layers in the opposite order (Reverse
    /// Layers on the definition): alignment indices follow their layers.
    pub fn reversed(&self) -> WallTypeDef {
        let n = self.layers.len();
        let mut out = self.clone();
        out.layers.reverse();
        // The exterior surface of layer i becomes the interior surface of
        // layer n-1-i; keep pointing at the layer, not the surface.
        out.props.align.each_index(|ix| {
            if let Some(i) = ix {
                *ix = Some(n - 1 - *i);
            }
        });
        out
    }
}

// ----- managing the list -----

/// A new name based on `base` that no type in `taken` has: `base_2`, `base_3`...
pub fn unique_name(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}_{n}");
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// What [`import_types`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Names of types added under their own name.
    pub added: Vec<String>,
    /// Types already there with the same definition.
    pub same: Vec<String>,
    /// `(name in the source, name it was given here)` for a same-named type
    /// that differs.
    pub renamed: Vec<(String, String)>,
}

impl ImportReport {
    /// The name `source` has here after the import.
    pub fn name_for(&self, source: &str) -> String {
        self.renamed
            .iter()
            .find(|(s, _)| s == source)
            .map_or_else(|| source.to_string(), |(_, n)| n.clone())
    }
}

/// Imports wall types into `dst` (Import Settings from Plan/Layout, the
/// Library button of the Wall Types panel). A type with the name and
/// definition of one already there is not added twice; one with the same name
/// and another definition arrives as `Name_2` (`Name_3`...).
pub fn import_types(dst: &mut Vec<WallTypeDef>, src: &[WallTypeDef]) -> ImportReport {
    let mut report = ImportReport::default();
    for t in src {
        match dst.iter().find(|d| d.name == t.name) {
            None => {
                dst.push(t.clone());
                report.added.push(t.name.clone());
            }
            Some(d) if d == t => report.same.push(t.name.clone()),
            Some(_) => {
                // An earlier import of this very definition is reused.
                let twin = dst.iter().find(|d| {
                    d.name.starts_with(&format!("{}_", t.name)) && {
                        let mut renamed = t.clone();
                        renamed.name = d.name.clone();
                        **d == renamed
                    }
                });
                if let Some(twin) = twin {
                    report.renamed.push((t.name.clone(), twin.name.clone()));
                    continue;
                }
                let name = unique_name(&t.name, |n| dst.iter().any(|d| d.name == n));
                let mut renamed = t.clone();
                renamed.name = name.clone();
                dst.push(renamed);
                report.renamed.push((t.name.clone(), name));
            }
        }
    }
    report
}

/// The names of the wall types in use: by a wall of the project, a pony or
/// glass pony part, or a default of the plan.
pub fn used_names(project: &Project, defaults: Option<&PlanDefaults>) -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    for f in &project.floors {
        for w in &f.walls {
            if let Some(n) = &w.wall_type {
                used.insert(n.clone());
            }
            match &w.class {
                WallClass::Pony {
                    upper_type,
                    lower_type,
                    ..
                } => {
                    used.insert(upper_type.clone());
                    used.insert(lower_type.clone());
                }
                WallClass::GlassPony { lower_type, .. } => {
                    used.insert(lower_type.clone());
                }
                _ => {}
            }
        }
    }
    if let Some(d) = defaults {
        for n in [
            &d.exterior_wall.wall_type,
            &d.interior_wall.wall_type,
            &d.foundation_wall.wall_type,
            &d.roof_detail.attic_wall_type,
            &d.roof_detail.lower_wall_type,
        ] {
            used.insert(n.clone());
        }
        let v = &d.wall_variants;
        for n in [
            &v.pony_upper_type,
            &v.pony_lower_type,
            &v.glass_type,
            &v.railing_type,
            &v.deck_railing_type,
            &v.deck_edge_type,
            &v.fencing_type,
        ] {
            used.insert(n.clone());
        }
    }
    used.remove("");
    used
}

/// The types of `types` that `used` does not name (what Delete All Unused
/// removes).
pub fn unused_names(types: &[WallTypeDef], used: &BTreeSet<String>) -> Vec<String> {
    types
        .iter()
        .filter(|t| !used.contains(&t.name))
        .map(|t| t.name.clone())
        .collect()
}

/// Delete All Unused: removes the unused types, returns how many went. The
/// last type stays so a plan is never left with none.
pub fn delete_unused(types: &mut Vec<WallTypeDef>, used: &BTreeSet<String>) -> usize {
    let before = types.len();
    let keep_one = types.first().cloned();
    types.retain(|t| used.contains(&t.name));
    if types.is_empty() {
        if let Some(t) = keep_one {
            types.push(t);
        }
    }
    before - types.len()
}

// ----- legacy types -----

/// The wall type that replaces a legacy generic type ("Default (wood frame
/// 16\" OC)", "Default (concrete)", "Adjustable Thickness Wall"; manual p.
/// 416): named `Wall-X`, X the thickness rounded up. A wall that is not a
/// Foundation wall, Deck Railing or Fencing gets a 1/2 in exterior and a 1/2
/// in interior layer around its original layer, which becomes the Main layer
/// with a framing material. `single` keeps the one layer.
pub fn migrate_legacy(
    thickness: f64,
    kind: WallKind,
    exterior_material: &str,
    interior_material: &str,
    main_material: &str,
    single: bool,
    foundation: bool,
) -> WallTypeDef {
    let x = thickness.ceil().max(1.0) as i64;
    let name = if foundation {
        format!("Foundation Wall-{x}")
    } else {
        format!("Wall-{x}")
    };
    if single {
        return WallTypeDef {
            name,
            layers: vec![WallLayer::new("Wall", thickness, true, main_material)],
            kind,
            props: WallTypeProps::default(),
        };
    }
    let mut main = WallLayer::new("Framing", thickness, true, "Fir Framing");
    main.set_role(LayerRole::Framing);
    WallTypeDef {
        name,
        layers: vec![
            WallLayer::new("Exterior", 0.5, false, exterior_material),
            main,
            WallLayer::new("Interior", 0.5, false, interior_material),
        ],
        kind,
        props: WallTypeProps::default(),
    }
}

// ----- the Library -----

/// A wall saved in the Library (manual p. 375): the wall's specification and
/// its wall type, without its doors, windows or edited heights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryWall {
    pub name: String,
    pub wall: Wall,
    pub wall_type: Option<WallTypeDef>,
}

impl LibraryWall {
    /// The wall drawn from `start` to `end`: a fresh copy of the saved
    /// specification with `id`.
    pub fn draw(&self, id: crate::Id, start: Point, end: Point) -> Wall {
        let mut w = self.wall.clone();
        w.id = id;
        w.start = start;
        w.end = end;
        w
    }
}

/// Saves `wall` and its type in the Library under `name`. A library item with
/// that name makes the new one `name_2` (`name_3`...). Returns the name used.
pub fn add_to_library(
    lib: &mut Vec<LibraryWall>,
    name: &str,
    wall: &Wall,
    ty: Option<&WallTypeDef>,
) -> String {
    let name = unique_name(name, |n| lib.iter().any(|l| l.name == n));
    let mut w = wall.clone();
    // Not its doors and windows (they are floor objects), not an edited top.
    w.bottom_offset = 0.0;
    w.spec = WallSpec {
        structure: crate::walls::spec::WallStructure {
            custom_top: false,
            ..w.spec.structure
        },
        ..w.spec
    };
    lib.push(LibraryWall {
        name: name.clone(),
        wall: w,
        wall_type: ty.cloned(),
    });
    name
}

/// The type of a library wall as it arrives in a plan that holds `types`:
/// the existing one when the definition is the same, `Name_2` when only the
/// name is. Returns the name the wall's `wall_type` takes.
pub fn bring_type(types: &mut Vec<WallTypeDef>, ty: &WallTypeDef) -> String {
    let report = import_types(types, std::slice::from_ref(ty));
    report.name_for(&ty.name)
}

/// Per-type usage count across the project's walls.
pub fn usage(project: &Project) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for f in &project.floors {
        for w in &f.walls {
            if let Some(n) = &w.wall_type {
                *m.entry(n.clone()).or_insert(0) += 1;
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::PlanDefaults;

    fn stucco() -> WallTypeDef {
        PlanDefaults::chief_x18_daniel()
            .wall_type("Stucco-6")
            .unwrap()
            .clone()
    }

    #[test]
    fn old_two_field_layers_load_and_save_unchanged() {
        let old = r#"{"name":"Old","kind":"Exterior","layers":[
            {"name":"Siding","thickness":0.5,"is_main":false,"material":"Siding"},
            {"name":"Framing","thickness":5.5,"is_main":true,"material":"Fir Framing"}]}"#;
        let t: WallTypeDef = serde_json::from_str(old).unwrap();
        assert_eq!(t.layers.len(), 2);
        assert!(t.layers[0].spec.is_default());
        assert!(t.props.is_default());
        // Nothing new is written for a type that has nothing new.
        let back = serde_json::to_string(&t).unwrap();
        assert!(!back.contains("spec") && !back.contains("props"), "{back}");
        // The role of an old layer reads from its material.
        assert_eq!(t.layers[1].resolved_role(), LayerRole::Framing);
        assert_eq!(t.layers[0].resolved_role(), LayerRole::Standard);
    }

    #[test]
    fn new_values_round_trip() {
        let mut t = stucco();
        t.layers[0].spec.extension = 2.0;
        t.layers[0].spec.fill = Some(FillStyle::hatch(45.0, 6.0, [1, 2, 3]));
        t.layers[2].set_role(LayerRole::Framing);
        t.props.partition = true;
        t.props.align.dimension = Some(1);
        t.sync_derived();
        let json = serde_json::to_string(&t).unwrap();
        let back: WallTypeDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
        assert_eq!(back.props.brick_ledge_depth, 2.0);
    }

    #[test]
    fn sections_follow_the_main_layers() {
        let t = stucco();
        assert_eq!(t.group_of(0), LayerGroup::Exterior);
        assert_eq!(t.group_of(1), LayerGroup::Exterior);
        assert_eq!(t.group_of(2), LayerGroup::Main);
        assert_eq!(t.group_of(3), LayerGroup::Interior);
        let mut m = t.clone();
        m.layers[3].is_main = true;
        m.layers[3].thickness = 0.5;
        assert_eq!(m.main_indices(), vec![2, 3]);
        assert!((m.main_thickness() - 6.0).abs() < 1e-9);
        assert!((m.outer_main_offset() - 1.625).abs() < 1e-9);
        assert!((m.inner_main_offset() - m.thickness()).abs() < 1e-9);
        assert!(m.problems().is_empty());
        // A gap between two Main layers is still the Main section.
        let mut gap = t.clone();
        gap.layers[0].is_main = true;
        assert_eq!(gap.group_of(1), LayerGroup::Main);
        assert!(gap.problems().contains(&TypeProblem::MainLayersSplit));
    }

    #[test]
    fn alignment_layers_default_to_the_outermost_main_layer() {
        let mut t = stucco();
        let main = t.outer_main_offset();
        assert_eq!(t.platform_offset(), main);
        assert_eq!(t.dimension_offset(), main);
        assert_eq!(t.deck_platform_offset(), main);
        assert_eq!(t.roof_offset(), main);
        t.props.align.build_platform = Some(0);
        t.props.align.dimension = Some(1);
        t.props.align.foundation_offset = 0.5;
        assert_eq!(t.platform_offset(), 0.0);
        // "Match Platform to Exterior" follows the build layer.
        assert_eq!(t.deck_platform_offset(), 0.0);
        t.props.align.deck_platform = Some(1);
        assert_eq!(t.deck_platform_offset(), t.depth_to(1));
        assert_eq!(t.dimension_offset(), t.depth_to(1));
        assert_eq!(t.foundation_align_offset(), main - 0.5);
        // An index the type no longer has falls back.
        t.layers.truncate(1);
        t.sync_derived();
        assert_eq!(t.props.align.dimension, None);
    }

    #[test]
    fn extension_lifts_the_brick_ledge() {
        let mut t = stucco();
        t.layers[0].spec.extension = 1.5;
        t.layers[1].spec.extension = 3.0;
        t.layers[3].spec.extension = 9.0;
        assert!(t.problems().contains(&TypeProblem::ExtensionNotExterior(3)));
        t.sync_derived();
        assert_eq!(t.layers[3].spec.extension, 0.0);
        assert_eq!(t.props.brick_ledge_depth, 3.0);
        assert!(t.problems().is_empty());
    }

    #[test]
    fn roles_are_validated() {
        let mut t = stucco();
        t.layers[2].set_role(LayerRole::AirGap);
        assert!(t.problems().contains(&TypeProblem::AirGapIsMain(2)));
        t.layers[2].set_role(LayerRole::Framing);
        assert!(t.layers[2].spec.framing.is_some());
        assert!(t.is_framed());
        t.layers[2].set_role(LayerRole::Standard);
        assert!(t.layers[2].spec.framing.is_none());
        t.layers[1].thickness = 0.0;
        assert!(t.problems().contains(&TypeProblem::LayerTooThin(1)));
        t.layers[1].thickness = 0.5;
        t.layers[2].thickness = 0.05;
        assert!(t.problems().contains(&TypeProblem::MainTooThin(2)));
        t.layers[2].thickness = 0.0625;
        assert!(t.problems().is_empty());
        for l in &mut t.layers {
            l.is_main = false;
        }
        assert_eq!(t.problems(), vec![TypeProblem::NoMainLayer]);
        t.layers.clear();
        assert_eq!(t.problems(), vec![TypeProblem::NoLayers]);
    }

    #[test]
    fn the_total_edit_is_taken_by_the_outermost_main_layer() {
        let mut t = stucco();
        let total = t.thickness();
        assert!(t.set_total(total + 1.0));
        assert!((t.layers[2].thickness - 6.5).abs() < 1e-9);
        // The main layer may go down to 1/16 in, and no further.
        let fixed = t.thickness() - t.layers[2].thickness;
        assert!(t.set_total(fixed + MIN_MAIN_LAYER));
        assert!((t.layers[2].thickness - MIN_MAIN_LAYER).abs() < 1e-9);
        let before = t.clone();
        assert!(!t.set_total(fixed));
        assert_eq!(t, before);
        // With two Main layers the outer one takes it.
        let mut m = stucco();
        m.layers[3].is_main = true;
        let thick = m.layers[3].thickness;
        assert!(m.set_total(m.thickness() + 2.0));
        assert!((m.layers[2].thickness - 7.5).abs() < 1e-9);
        assert_eq!(m.layers[3].thickness, thick);
    }

    #[test]
    fn a_room_divider_type_is_zero_thick() {
        let d = WallTypeDef::room_divider_type("Room Divider");
        assert_eq!(d.thickness(), 0.0);
        assert!(d.props.room_divider);
        assert!(d.problems().is_empty());
        // The same layer on an ordinary type is a problem.
        let mut o = d.clone();
        o.props.room_divider = false;
        assert!(o.problems().contains(&TypeProblem::MainTooThin(0)));
    }

    #[test]
    fn import_renames_a_different_type_and_reuses_a_twin() {
        let mut here = vec![stucco()];
        let mut other = stucco();
        other.layers[0].thickness = 2.0;
        let mut third = stucco();
        third.name = "Third".into();
        let r = import_types(&mut here, &[stucco(), other.clone(), third]);
        assert_eq!(r.same, vec!["Stucco-6".to_string()]);
        assert_eq!(r.added, vec!["Third".to_string()]);
        assert_eq!(
            r.renamed,
            vec![("Stucco-6".to_string(), "Stucco-6_2".to_string())]
        );
        assert_eq!(here.len(), 3);
        assert_eq!(here[1].name, "Stucco-6_2");
        assert_eq!(here[1].layers, other.layers);
        // The same import again adds nothing: the twin is found.
        let r = import_types(&mut here, &[other]);
        assert_eq!(r.name_for("Stucco-6"), "Stucco-6_2");
        assert_eq!(here.len(), 3);
        // A third, different one is _3.
        let mut more = stucco();
        more.layers[0].thickness = 3.0;
        let r = import_types(&mut here, &[more]);
        assert_eq!(r.name_for("Stucco-6"), "Stucco-6_3");
    }

    #[test]
    fn unused_types_go_but_one_stays() {
        let d = PlanDefaults::chief_x18_daniel();
        let mut types = d.wall_types.clone();
        let p = Project::new("t");
        let used = used_names(&p, Some(&d));
        assert!(used.contains("Stucco-6") && used.contains("Interior-4"));
        let unused = unused_names(&types, &used);
        assert!(unused.contains(&"Brick-6".to_string()));
        let n = types.len();
        let gone = delete_unused(&mut types, &used);
        assert_eq!(gone, unused.len());
        assert_eq!(types.len(), n - gone);
        assert!(types.iter().all(|t| used.contains(&t.name)));
        // With nothing used the first type is kept.
        let mut types = d.wall_types.clone();
        delete_unused(&mut types, &BTreeSet::new());
        assert_eq!(types.len(), 1);
    }

    #[test]
    fn library_walls_get_a_suffix_on_a_collision() {
        let mut lib = Vec::new();
        let w = Wall::new(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            7.625,
            96.0,
            WallKind::Exterior,
        );
        let ty = stucco();
        assert_eq!(
            add_to_library(&mut lib, "Garage Wall", &w, Some(&ty)),
            "Garage Wall"
        );
        assert_eq!(
            add_to_library(&mut lib, "Garage Wall", &w, Some(&ty)),
            "Garage Wall_2"
        );
        assert_eq!(
            add_to_library(&mut lib, "Garage Wall", &w, Some(&ty)),
            "Garage Wall_3"
        );
        let drawn = lib[0].draw(9, Point::new(10.0, 10.0), Point::new(10.0, 90.0));
        assert_eq!((drawn.id, drawn.start.y, drawn.end.y), (9, 10.0, 90.0));
        assert_eq!(drawn.thickness, 7.625);
        // The type arrives in a plan that has a different one of that name.
        let mut here = vec![{
            let mut o = stucco();
            o.layers[0].thickness = 2.0;
            o
        }];
        assert_eq!(bring_type(&mut here, &ty), "Stucco-6_2");
        assert_eq!(bring_type(&mut here, &ty), "Stucco-6_2");
        assert_eq!(here.len(), 2);
    }

    #[test]
    fn legacy_generic_types_become_wall_x() {
        let t = migrate_legacy(
            5.5,
            WallKind::Exterior,
            "Siding",
            "Drywall",
            "Fir Framing",
            false,
            false,
        );
        assert_eq!(t.name, "Wall-6");
        assert_eq!(t.layers.len(), 3);
        assert_eq!(t.main_layer().unwrap().thickness, 5.5);
        assert_eq!(t.layers[0].thickness, 0.5);
        assert!(t.is_framed());
        let f = migrate_legacy(8.0, WallKind::Exterior, "", "", "Concrete", true, true);
        assert_eq!(f.name, "Foundation Wall-8");
        assert_eq!(f.layers.len(), 1);
    }

    #[test]
    fn reversing_a_definition_keeps_alignments_on_their_layers() {
        let mut t = stucco();
        t.props.align.dimension = Some(0);
        let r = t.reversed();
        assert_eq!(r.layers[0].name, "Drywall");
        assert_eq!(r.props.align.dimension, Some(3));
        assert_eq!(r.layers[3].name, t.layers[0].name);
    }
}
