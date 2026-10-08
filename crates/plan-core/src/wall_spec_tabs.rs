//! The remaining Wall Specification tabs (Round 15): Wall Covering (W-115),
//! Newels/Balusters (W-116), Rails (W-117), Materials, Components, Object
//! Information (W-118), Schedule and the Drawing Group field.
//!
//! Every value lives in [`crate::walls::WallSpec`] (`Wall.spec`, serde
//! default, so old files load unchanged). What they do:
//!
//! * [`WallCovering::bands`] lists the wainscot, chair rail, base and crown
//!   bands of each side with their heights; `plan-3d` sweeps the molding
//!   profile of the library (`rooms::molding_def`) along the wall face.
//! * [`WallRailing`] gives a railing wall its newels (posts), balusters,
//!   panels and rails; `plan-3d` `railing.rs` reads it. The defaults
//!   reproduce the railing built before the tabs existed.
//! * [`WallMaterials`] names a library material per layer or surface;
//!   [`Project::sync_wall_materials`] hands it to the per-object paint.
//! * [`wall_components`] is the Components tab: the area and volume of each
//!   layer of the wall type, net of its openings.
//! * [`WallInfo`] and [`WallScheduleInfo`] feed the Wall schedule columns.

use crate::defaults::WallTypeDef;
use crate::extras::MoldingKind;
use crate::model::{Id, Opening, Project, Wall};
use crate::rooms::{molding_def, MoldingDef, CHAIR_RAIL_HEIGHT};
use crate::walls::WallClass;
use serde::{Deserialize, Serialize};

// ----- Wall Covering (W-115) -----

/// Which face of the wall a covering sits on. The exterior side is the one
/// `Wall::exterior_side` names; the interior side is the other face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum CoveringSide {
    #[default]
    Interior,
    Exterior,
}

impl CoveringSide {
    pub const ALL: [CoveringSide; 2] = [CoveringSide::Interior, CoveringSide::Exterior];

    pub fn name(self) -> &'static str {
        match self {
            CoveringSide::Interior => "Interior",
            CoveringSide::Exterior => "Exterior",
        }
    }
}

/// One band of a covering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BandKind {
    /// A full-height covering (wall paper, tile, paneling).
    Covering,
    Wainscot,
    ChairRail,
    Base,
    Crown,
}

impl BandKind {
    pub fn name(self) -> &'static str {
        match self {
            BandKind::Covering => "Wall Covering",
            BandKind::Wainscot => "Wainscot",
            BandKind::ChairRail => "Chair Rail",
            BandKind::Base => "Base Molding",
            BandKind::Crown => "Crown Molding",
        }
    }

    /// The molding kind a profile of this band comes from.
    pub fn molding(self) -> Option<MoldingKind> {
        match self {
            BandKind::ChairRail => Some(MoldingKind::Chair),
            BandKind::Base => Some(MoldingKind::Base),
            BandKind::Crown => Some(MoldingKind::Crown),
            BandKind::Covering | BandKind::Wainscot => None,
        }
    }
}

/// Default thickness of a full-height covering, inches.
pub const COVERING_THICKNESS: f64 = 0.25;
/// Default wainscot height and thickness, inches.
pub const WAINSCOT_HEIGHT: f64 = 36.0;
pub const WAINSCOT_THICKNESS: f64 = 0.75;

/// What one face of the wall wears.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SideCovering {
    /// Material of a full-height covering; empty for none.
    pub covering: String,
    pub covering_thickness: f64,
    /// Material of the wainscot; empty for none.
    pub wainscot: String,
    /// Top of the wainscot above the wall bottom.
    pub wainscot_height: f64,
    pub wainscot_thickness: f64,
    /// Library profile names (`rooms::MOLDING_LIBRARY`); empty for none.
    pub chair_rail: String,
    /// Bottom of the chair rail above the wall bottom.
    pub chair_rail_height: f64,
    pub base: String,
    pub crown: String,
    /// Material the moldings are made of; empty for the trim material.
    pub molding_material: String,
}

impl Default for SideCovering {
    fn default() -> Self {
        Self {
            covering: String::new(),
            covering_thickness: COVERING_THICKNESS,
            wainscot: String::new(),
            wainscot_height: WAINSCOT_HEIGHT,
            wainscot_thickness: WAINSCOT_THICKNESS,
            chair_rail: String::new(),
            chair_rail_height: CHAIR_RAIL_HEIGHT,
            base: String::new(),
            crown: String::new(),
            molding_material: String::new(),
        }
    }
}

/// A band of covering on one face, in wall-local heights (from the wall
/// bottom).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoveringBand<'a> {
    pub side: CoveringSide,
    pub kind: BandKind,
    pub lo: f64,
    pub hi: f64,
    /// How far the band stands off the face, inches.
    pub depth: f64,
    /// The band's material (empty: the usual one for its kind).
    pub material: &'a str,
    /// The molding profile of a chair rail, base or crown.
    pub profile: Option<&'static MoldingDef>,
}

impl SideCovering {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        self.covering.is_empty()
            && self.wainscot.is_empty()
            && self.chair_rail.is_empty()
            && self.base.is_empty()
            && self.crown.is_empty()
    }

    /// The bands of this face on a wall `height` tall, bottom to top. A
    /// molding whose name is not in the library is left out; a wainscot is
    /// held to the wall's height.
    pub fn bands(&self, side: CoveringSide, height: f64) -> Vec<CoveringBand<'_>> {
        let mut out = Vec::new();
        if height <= 0.0 {
            return out;
        }
        if !self.covering.is_empty() {
            out.push(CoveringBand {
                side,
                kind: BandKind::Covering,
                lo: 0.0,
                hi: height,
                depth: self.covering_thickness.max(0.0),
                material: &self.covering,
                profile: None,
            });
        }
        if !self.wainscot.is_empty() && self.wainscot_height > 0.0 {
            out.push(CoveringBand {
                side,
                kind: BandKind::Wainscot,
                lo: 0.0,
                hi: self.wainscot_height.min(height),
                depth: self.wainscot_thickness.max(0.0),
                material: &self.wainscot,
                profile: None,
            });
        }
        let mut molding = |kind: BandKind, name: &'_ str, lo: Option<f64>| {
            let Some(def) = molding_def(name).filter(|d| Some(d.kind) == kind.molding()) else {
                return;
            };
            let h = def.height();
            let (lo, hi) = match (kind, lo) {
                (BandKind::Crown, _) => ((height - h).max(0.0), height),
                (_, Some(lo)) => (lo, lo + h),
                _ => (0.0, h),
            };
            out.push(CoveringBand {
                side,
                kind,
                lo,
                hi,
                depth: def.projection(),
                material: &self.molding_material,
                profile: Some(def),
            });
        };
        molding(BandKind::Base, &self.base, None);
        molding(
            BandKind::ChairRail,
            &self.chair_rail,
            Some(self.chair_rail_height),
        );
        molding(BandKind::Crown, &self.crown, None);
        out
    }
}

/// The Wall Covering tab.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WallCovering {
    pub interior: SideCovering,
    pub exterior: SideCovering,
}

impl WallCovering {
    pub fn side(&self, side: CoveringSide) -> &SideCovering {
        match side {
            CoveringSide::Interior => &self.interior,
            CoveringSide::Exterior => &self.exterior,
        }
    }

    pub fn side_mut(&mut self, side: CoveringSide) -> &mut SideCovering {
        match side {
            CoveringSide::Interior => &mut self.interior,
            CoveringSide::Exterior => &mut self.exterior,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.interior.is_empty() && self.exterior.is_empty()
    }

    /// Every band of both faces on a wall `height` tall.
    pub fn bands(&self, height: f64) -> Vec<CoveringBand<'_>> {
        let mut v = self.interior.bands(CoveringSide::Interior, height);
        v.extend(self.exterior.bands(CoveringSide::Exterior, height));
        v
    }
}

impl Wall {
    /// The sign (along `normal`) of the face a covering of `side` sits on:
    /// the exterior face is on `exterior_side`.
    pub fn covering_face_sign(&self, side: CoveringSide) -> f64 {
        let ext = self.exterior_side.sign();
        match side {
            CoveringSide::Exterior => ext,
            CoveringSide::Interior => -ext,
        }
    }

    /// The height the wall's coverings are measured against: the stem of a
    /// foundation wall, the low top of a half wall, else the wall height.
    pub fn covering_height(&self) -> f64 {
        match &self.class {
            WallClass::Foundation => self.foundation_height,
            WallClass::HalfWall { height } => height.min(self.height).max(0.0),
            _ => self.height,
        }
    }
}

// ----- Newels/Balusters and Rails (W-116, W-117) -----

/// Shape of a newel post.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum NewelStyle {
    #[default]
    Square,
    Round,
    /// A square post with its corners cut.
    Chamfered,
}

impl NewelStyle {
    pub const ALL: [NewelStyle; 3] = [NewelStyle::Square, NewelStyle::Round, NewelStyle::Chamfered];

    pub fn name(self) -> &'static str {
        match self {
            NewelStyle::Square => "Square",
            NewelStyle::Round => "Round",
            NewelStyle::Chamfered => "Chamfered",
        }
    }
}

/// Shape of a baluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum BalusterStyle {
    #[default]
    Square,
    Round,
    /// A flat bar, wider along the rail than across it.
    FlatBar,
}

impl BalusterStyle {
    pub const ALL: [BalusterStyle; 3] = [
        BalusterStyle::Square,
        BalusterStyle::Round,
        BalusterStyle::FlatBar,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BalusterStyle::Square => "Square",
            BalusterStyle::Round => "Round",
            BalusterStyle::FlatBar => "Flat Bar",
        }
    }
}

/// What fills the bays between newels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum RailFill {
    /// Balusters.
    #[default]
    Balusters,
    /// A glass panel in each bay.
    GlassPanel,
    /// A solid panel in each bay.
    SolidPanel,
}

impl RailFill {
    pub const ALL: [RailFill; 3] = [RailFill::Balusters, RailFill::GlassPanel, RailFill::SolidPanel];

    pub fn name(self) -> &'static str {
        match self {
            RailFill::Balusters => "Balusters",
            RailFill::GlassPanel => "Glass Panels",
            RailFill::SolidPanel => "Solid Panels",
        }
    }
}

/// Profile of a rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum RailProfile {
    #[default]
    Rectangular,
    Round,
}

impl RailProfile {
    pub const ALL: [RailProfile; 2] = [RailProfile::Rectangular, RailProfile::Round];

    pub fn name(self) -> &'static str {
        match self {
            RailProfile::Rectangular => "Rectangular",
            RailProfile::Round => "Round",
        }
    }
}

/// Newel posts, balusters, panels and rails of a railing wall (Newels/
/// Balusters and Rails tabs). Defaults reproduce the railing built before the
/// tabs: 3 1/2" square posts at most 8' apart and at each end, 3/4" square
/// balusters 4" apart, a 1 1/2" top rail with its top at 36".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallRailing {
    // Newels
    pub newel_style: NewelStyle,
    pub newel_size: f64,
    /// Top of the newels above the railing's bottom; zero follows the top rail.
    pub newel_height: f64,
    /// Greatest distance between newels, inches.
    pub newel_spacing: f64,
    /// A newel at each end of the run.
    pub newel_at_ends: bool,
    /// A cap on each newel.
    pub newel_cap: bool,
    /// The posts run down to the beam under the railing (a deck's rim).
    pub post_to_beam: bool,
    pub beam_depth: f64,
    // Balusters and panels
    pub fill: RailFill,
    pub baluster_style: BalusterStyle,
    pub baluster_size: f64,
    /// Greatest distance between baluster centers, inches.
    pub baluster_spacing: f64,
    /// Balusters in each bay instead of the spacing (`None`: by spacing).
    pub balusters_per_bay: Option<u32>,
    pub panel_thickness: f64,
    // Rails
    /// Top of the top rail above the railing's bottom; `None` is 36".
    pub top_rail_top: Option<f64>,
    pub top_rail_profile: RailProfile,
    pub top_rail_height: f64,
    pub top_rail_width: f64,
    pub bottom_rail: bool,
    pub bottom_rail_profile: RailProfile,
    pub bottom_rail_height: f64,
    pub bottom_rail_width: f64,
    /// Bottom of the bottom rail above the railing's bottom.
    pub bottom_rail_gap: f64,
}

/// Top of the top rail when the tab leaves it open, inches.
pub const DEFAULT_RAIL_TOP: f64 = 36.0;
/// The greatest gap a guard may leave, inches (IRC R312.1.3 sphere rule).
pub const MAX_BALUSTER_CLEAR: f64 = 4.0;

impl Default for WallRailing {
    fn default() -> Self {
        Self {
            newel_style: NewelStyle::Square,
            newel_size: 3.5,
            newel_height: 0.0,
            newel_spacing: 96.0,
            newel_at_ends: true,
            newel_cap: false,
            post_to_beam: false,
            beam_depth: 9.25,
            fill: RailFill::Balusters,
            baluster_style: BalusterStyle::Square,
            baluster_size: 0.75,
            baluster_spacing: 4.0,
            balusters_per_bay: None,
            panel_thickness: 0.375,
            top_rail_top: None,
            top_rail_profile: RailProfile::Rectangular,
            top_rail_height: 1.5,
            top_rail_width: 2.1,
            bottom_rail: true,
            bottom_rail_profile: RailProfile::Rectangular,
            bottom_rail_height: 1.5,
            bottom_rail_width: 1.0,
            bottom_rail_gap: 3.0,
        }
    }
}

impl WallRailing {
    /// Top of the top rail above the railing's bottom.
    pub fn rail_top(&self) -> f64 {
        self.top_rail_top.unwrap_or(DEFAULT_RAIL_TOP)
    }

    /// Top of the newels: their own height, else the underside of the top
    /// rail, which runs over them; the cap stands on that.
    pub fn post_top(&self) -> f64 {
        let t = if self.newel_height > 0.0 {
            self.newel_height
        } else {
            self.rail_top() - self.top_rail_height
        };
        t + if self.newel_cap { NEWEL_CAP_HEIGHT } else { 0.0 }
    }

    /// Newels along a run of `length`: one at each end when asked, none
    /// closer than `newel_spacing` apart. A run with none at its ends still
    /// gets the ones the spacing needs inside it.
    pub fn newel_count(&self, length: f64) -> usize {
        if length <= 0.0 {
            return 0;
        }
        let spacing = self.newel_spacing.max(self.newel_size.max(1.0));
        let bays = (length / spacing).ceil().max(1.0) as usize;
        if self.newel_at_ends {
            bays + 1
        } else {
            bays.saturating_sub(1)
        }
    }

    /// The newel centers along a run of `length`, evenly spaced.
    pub fn newel_centers(&self, length: f64) -> Vec<f64> {
        let n = self.newel_count(length);
        if n == 0 {
            return Vec::new();
        }
        let half = self.newel_size * 0.5;
        let lo = half.min(length * 0.5);
        let hi = (length - half).max(lo);
        if self.newel_at_ends {
            (0..n)
                .map(|i| {
                    if n == 1 {
                        length * 0.5
                    } else {
                        (length * i as f64 / (n - 1) as f64).clamp(lo, hi)
                    }
                })
                .collect()
        } else {
            let bays = n + 1;
            (1..=n).map(|i| length * i as f64 / bays as f64).collect()
        }
    }

    /// Baluster centers along a run of `length`, clear of the newels at
    /// `newels`. By spacing: one every `baluster_spacing`, starting half a
    /// spacing in; by count: that many evenly spread in each bay.
    pub fn baluster_centers(&self, length: f64, newels: &[f64]) -> Vec<f64> {
        if self.fill != RailFill::Balusters || length <= 0.0 {
            return Vec::new();
        }
        let clear = self.newel_size * 0.5 + self.baluster_size * 0.5;
        let mut out = Vec::new();
        match self.balusters_per_bay {
            Some(n) if n > 0 => {
                let mut edges: Vec<f64> = vec![0.0];
                edges.extend(newels.iter().copied());
                edges.push(length);
                edges.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
                for w in edges.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    for k in 1..=n {
                        out.push(a + (b - a) * f64::from(k) / f64::from(n + 1));
                    }
                }
            }
            _ => {
                let spacing = self.baluster_spacing.max(0.5);
                let mut s = spacing * 0.5;
                while s < length {
                    if newels.iter().all(|c| (c - s).abs() >= clear) {
                        out.push(s);
                    }
                    s += spacing;
                }
            }
        }
        out
    }

    /// The widest gap between balusters, newels and ends along a run (the
    /// guard's clear opening), inches; zero for a panel railing.
    pub fn widest_gap(&self, length: f64) -> f64 {
        if self.fill != RailFill::Balusters {
            return 0.0;
        }
        let newels = self.newel_centers(length);
        let mut stops: Vec<(f64, f64)> = vec![(0.0, 0.0)];
        stops.extend(newels.iter().map(|c| (*c, self.newel_size * 0.5)));
        stops.extend(
            self.baluster_centers(length, &newels)
                .into_iter()
                .map(|c| (c, self.baluster_size * 0.5)),
        );
        stops.push((length, 0.0));
        stops.sort_by(|a, b| a.0.total_cmp(&b.0));
        stops
            .windows(2)
            .map(|w| (w[1].0 - w[1].1) - (w[0].0 + w[0].1))
            .fold(0.0, f64::max)
    }
}

/// Height of a newel cap, inches.
pub const NEWEL_CAP_HEIGHT: f64 = 1.0;

// ----- Materials -----

/// One layer or surface of a wall and the library material it was given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WallPaint {
    /// The layer name of the wall type or a surface ("Exterior Wall
    /// Surface", "Interior Wall Surface", "Wall Cap", "Footing", "Sill
    /// Plate").
    pub part: String,
    pub material: String,
    pub rgb: [u8; 3],
}

/// The Materials tab: a material per layer or surface; a part without an
/// entry keeps its usual look.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WallMaterials {
    pub parts: Vec<WallPaint>,
}

/// The surfaces besides the wall type's layers that the Materials tab lists.
pub const SURFACE_PARTS: [&str; 5] = [
    "Exterior Wall Surface",
    "Interior Wall Surface",
    "Wall Cap",
    "Footing",
    "Sill Plate",
];

impl WallMaterials {
    pub fn get(&self, part: &str) -> Option<&WallPaint> {
        self.parts.iter().find(|p| p.part == part)
    }

    pub fn color(&self, part: &str) -> Option<[u8; 3]> {
        self.get(part).map(|p| p.rgb)
    }

    /// Gives `part` a material, or takes it back with `None`.
    pub fn set(&mut self, part: &str, paint: Option<(&str, [u8; 3])>) {
        self.parts.retain(|p| p.part != part);
        if let Some((material, rgb)) = paint {
            self.parts.push(WallPaint {
                part: part.to_string(),
                material: material.to_string(),
                rgb,
            });
        }
    }

    /// The parts the tab lists for a wall of type `ty`: its layers (named
    /// "Layer: Siding"), then the surfaces.
    pub fn part_names(ty: Option<&WallTypeDef>) -> Vec<String> {
        let mut v: Vec<String> = ty
            .map(|t| {
                t.layers
                    .iter()
                    .map(|l| format!("Layer: {}", l.name))
                    .collect()
            })
            .unwrap_or_default();
        v.extend(SURFACE_PARTS.iter().map(|s| (*s).to_string()));
        v
    }
}

impl Project {
    /// Hands the Materials tab of wall `id` to the project's per-object paint
    /// (`Project::object_materials`), so the Material Painter and Adjust
    /// Materials see the same materials. It only sets. Returns whether
    /// anything changed.
    pub fn sync_wall_materials(&mut self, id: Id) -> bool {
        let Some(paints) = self
            .floors
            .iter()
            .flat_map(|f| &f.walls)
            .find(|w| w.id == id)
            .map(|w| w.spec.materials.parts.clone())
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

// ----- Object Information and Schedule -----

/// The Object Information tab: the wall's code and description.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WallInfo {
    /// The ID or code the office uses for this wall.
    pub id: String,
    pub description: String,
}

/// The Schedule tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallScheduleInfo {
    /// "Include in Schedule": a cleared box leaves the wall out of the Wall
    /// schedule and its numbering.
    pub include: bool,
    pub manufacturer: String,
    pub model: String,
    pub supplier: String,
    pub comment: String,
}

impl Default for WallScheduleInfo {
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

// ----- Drawing Group -----

/// Chief's default drawing group for a wall ("29 - Wall").
pub const DEFAULT_DRAWING_GROUP: u32 = 29;

/// Drawing groups the Layer tab offers (number, name); verify against Chief.
pub const DRAWING_GROUPS: [(u32, &str); 8] = [
    (10, "Floor"),
    (20, "Room"),
    (27, "Foundation Wall"),
    (29, "Wall"),
    (30, "Door"),
    (31, "Window"),
    (40, "Cabinet"),
    (50, "Fixture"),
];

/// The display text of a drawing group.
pub fn drawing_group_name(group: u32) -> String {
    DRAWING_GROUPS
        .iter()
        .find(|(n, _)| *n == group)
        .map_or_else(|| format!("{group}"), |(n, name)| format!("{n} \u{2013} {name}"))
}

// ----- Components -----

/// One row of the Components tab.
#[derive(Debug, Clone, PartialEq)]
pub struct WallComponent {
    pub name: String,
    pub material: String,
    /// Layer thickness, inches (zero for a part that is not a layer).
    pub thickness: f64,
    pub area_sq_ft: f64,
    pub volume_cu_ft: f64,
}

/// The area of the wall face the openings take, square inches: each opening
/// clipped to the wall's length and height.
pub fn openings_area(wall: &Wall, openings: &[Opening]) -> f64 {
    let (len, height) = (wall.path_length(), wall.covering_height());
    openings
        .iter()
        .filter(|o| o.wall_id == wall.id)
        .map(|o| {
            let w = (o.end_offset().min(len) - o.start_offset().max(0.0)).max(0.0);
            let h = (o.sill_height + o.height).min(height) - o.sill_height.max(0.0);
            w * h.max(0.0)
        })
        .sum()
}

/// The Components tab: each layer of the wall's type with its net area (the
/// wall face less its openings) and volume, then the cap, footing and sill
/// plate the wall has. A wall with no type lists one row for the wall.
pub fn wall_components(
    wall: &Wall,
    ty: Option<&WallTypeDef>,
    openings: &[Opening],
) -> Vec<WallComponent> {
    let sq_ft = 144.0;
    let cu_ft = 1728.0;
    let length = wall.path_length();
    let gross = length * wall.covering_height();
    let net = (gross - openings_area(wall, openings)).max(0.0);
    let mut rows = Vec::new();
    match ty.filter(|t| !t.layers.is_empty()) {
        Some(t) => {
            for l in &t.layers {
                rows.push(WallComponent {
                    name: l.name.clone(),
                    material: l.material.clone(),
                    thickness: l.thickness,
                    area_sq_ft: net / sq_ft,
                    volume_cu_ft: net * l.thickness / cu_ft,
                });
            }
        }
        None => rows.push(WallComponent {
            name: "Wall".to_string(),
            material: String::new(),
            thickness: wall.thickness,
            area_sq_ft: net / sq_ft,
            volume_cu_ft: net * wall.thickness / cu_ft,
        }),
    }
    let spec = &wall.spec;
    let mut boxed = |name: &str, material: &str, b: Option<crate::walls::WallBox>| {
        if let Some(b) = b {
            let (w, h) = (b.across.1 - b.across.0, b.up.1 - b.up.0);
            rows.push(WallComponent {
                name: name.to_string(),
                material: material.to_string(),
                thickness: w,
                area_sq_ft: length * h / sq_ft,
                volume_cu_ft: length * w * h / cu_ft,
            });
        }
    };
    boxed(
        "Footing",
        "Concrete",
        spec.foundation.footing_box(wall, 0.0),
    );
    boxed(
        "Sill Plate",
        "Fir Framing",
        spec.foundation.sill_box(wall, wall.height),
    );
    boxed("Wall Cap", "Trim", spec.cap.cap_box(wall));
    for band in spec.covering.bands(wall.covering_height()) {
        let h = band.hi - band.lo;
        let area = length * h - covering_hole_area(wall, openings, band.lo, band.hi);
        rows.push(WallComponent {
            name: format!("{} ({})", band.kind.name(), band.side.name()),
            material: band.material.to_string(),
            thickness: band.depth,
            area_sq_ft: area.max(0.0) / sq_ft,
            volume_cu_ft: area.max(0.0) * band.depth / cu_ft,
        });
    }
    rows
}

/// The part of the openings that falls in the height range `lo..hi`, square
/// inches.
fn covering_hole_area(wall: &Wall, openings: &[Opening], lo: f64, hi: f64) -> f64 {
    let len = wall.path_length();
    openings
        .iter()
        .filter(|o| o.wall_id == wall.id)
        .map(|o| {
            let w = (o.end_offset().min(len) - o.start_offset().max(0.0)).max(0.0);
            let h = ((o.sill_height + o.height).min(hi) - o.sill_height.max(lo)).max(0.0);
            w * h
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::WallLayer;
    use crate::geometry::Point;
    use crate::model::{OpeningKind, WallKind};

    fn wall() -> Wall {
        Wall::new(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        )
    }

    #[test]
    fn the_covering_bands_take_their_heights_from_the_molding_library() {
        let mut c = SideCovering {
            wainscot: "Beadboard".into(),
            base: "Colonial Base".into(),
            chair_rail: "Chair Rail".into(),
            crown: "Cove Crown".into(),
            ..SideCovering::default()
        };
        let all = crate::rooms::MOLDING_LIBRARY;
        // Use real library names of each kind.
        let name = |k: MoldingKind| all.iter().find(|m| m.kind == k).unwrap().name.to_string();
        c.base = name(MoldingKind::Base);
        c.chair_rail = name(MoldingKind::Chair);
        c.crown = name(MoldingKind::Crown);
        let bands = c.bands(CoveringSide::Interior, 96.0);
        let kinds: Vec<_> = bands.iter().map(|b| b.kind).collect();
        assert_eq!(
            kinds,
            vec![
                BandKind::Wainscot,
                BandKind::Base,
                BandKind::ChairRail,
                BandKind::Crown
            ]
        );
        let base = bands.iter().find(|b| b.kind == BandKind::Base).unwrap();
        let def = molding_def(&c.base).unwrap();
        assert_eq!((base.lo, base.hi), (0.0, def.height()));
        assert_eq!(base.depth, def.projection());
        let chair = bands.iter().find(|b| b.kind == BandKind::ChairRail).unwrap();
        assert_eq!(chair.lo, CHAIR_RAIL_HEIGHT);
        let crown = bands.iter().find(|b| b.kind == BandKind::Crown).unwrap();
        assert_eq!(crown.hi, 96.0);
        assert!((crown.lo - (96.0 - molding_def(&c.crown).unwrap().height())).abs() < 1e-9);
        // The wainscot stops at the wall top; an unknown or wrong-kind name
        // is left out.
        assert_eq!(bands[0].hi, 36.0);
        assert_eq!(c.bands(CoveringSide::Interior, 20.0)[0].hi, 20.0);
        c.base = "No Such Molding".into();
        c.crown = name(MoldingKind::Base);
        let kinds: Vec<_> = c
            .bands(CoveringSide::Exterior, 96.0)
            .iter()
            .map(|b| b.kind)
            .collect();
        assert_eq!(kinds, vec![BandKind::Wainscot, BandKind::ChairRail]);
        assert!(WallCovering::default().is_empty());
    }

    #[test]
    fn the_exterior_covering_sits_on_the_exterior_face() {
        let mut w = wall();
        assert_eq!(w.covering_face_sign(CoveringSide::Exterior), 1.0);
        assert_eq!(w.covering_face_sign(CoveringSide::Interior), -1.0);
        w.exterior_side = crate::walls::Side::Right;
        assert_eq!(w.covering_face_sign(CoveringSide::Exterior), -1.0);
    }

    #[test]
    fn the_default_railing_matches_the_old_fixed_one() {
        let r = WallRailing::default();
        assert_eq!(r.newel_count(240.0), 4);
        assert_eq!(r.newel_count(96.0), 2);
        assert_eq!(r.newel_count(0.0), 0);
        assert_eq!(r.rail_top(), 36.0);
        assert_eq!(r.post_top(), 34.5);
        let newels = r.newel_centers(240.0);
        assert_eq!(newels.first().copied(), Some(1.75));
        assert_eq!(newels.last().copied(), Some(238.25));
        let balusters = r.baluster_centers(240.0, &newels);
        assert!(balusters.iter().all(|b| newels
            .iter()
            .all(|n| (b - n).abs() >= 3.5 * 0.5 + 0.375)));
        assert_eq!(balusters[0], 2.0 + 2.0);
        assert!(r.widest_gap(240.0) < 4.0 + 1e-9, "{}", r.widest_gap(240.0));
    }

    #[test]
    fn newel_spacing_and_end_posts_follow_the_tab() {
        let mut r = WallRailing {
            newel_spacing: 48.0,
            ..WallRailing::default()
        };
        assert_eq!(r.newel_count(240.0), 6);
        r.newel_at_ends = false;
        assert_eq!(r.newel_count(240.0), 4);
        let c = r.newel_centers(240.0);
        assert_eq!(c.len(), 4);
        assert!((c[0] - 48.0).abs() < 1e-9);
        r.newel_at_ends = true;
        r.newel_height = 42.0;
        r.newel_cap = true;
        assert_eq!(r.post_top(), 43.0);
        r.newel_height = 0.0;
        r.top_rail_top = Some(42.0);
        assert_eq!(r.rail_top(), 42.0);
    }

    #[test]
    fn a_baluster_count_spreads_evenly_in_each_bay() {
        let r = WallRailing {
            balusters_per_bay: Some(3),
            newel_spacing: 100.0,
            ..WallRailing::default()
        };
        let newels = r.newel_centers(200.0);
        assert_eq!(newels.len(), 3);
        let b = r.baluster_centers(200.0, &newels);
        // Two bays of three.
        assert_eq!(b.len(), 6);
        assert!(b.windows(2).all(|w| w[1] > w[0]));
        // Panels have no balusters and no gap.
        let panel = WallRailing {
            fill: RailFill::GlassPanel,
            ..r
        };
        assert!(panel.baluster_centers(200.0, &newels).is_empty());
        assert_eq!(panel.widest_gap(200.0), 0.0);
    }

    #[test]
    fn a_wide_baluster_spacing_shows_in_the_widest_gap() {
        let r = WallRailing {
            baluster_spacing: 8.0,
            ..WallRailing::default()
        };
        assert!(r.widest_gap(96.0) > MAX_BALUSTER_CLEAR);
    }

    #[test]
    fn the_materials_set_replace_and_clear() {
        let mut m = WallMaterials::default();
        m.set("Layer: Siding", Some(("Cedar", [150, 100, 60])));
        assert_eq!(m.color("Layer: Siding"), Some([150, 100, 60]));
        m.set("Layer: Siding", Some(("Brick", [160, 70, 50])));
        assert_eq!(m.parts.len(), 1);
        assert_eq!(m.get("Layer: Siding").unwrap().material, "Brick");
        m.set("Layer: Siding", None);
        assert!(m.get("Layer: Siding").is_none());
        let ty = WallTypeDef {
            name: "T".into(),
            layers: vec![WallLayer::new("Siding", 0.75, false, "Siding")],
            kind: WallKind::Exterior,
        };
        let names = WallMaterials::part_names(Some(&ty));
        assert_eq!(names[0], "Layer: Siding");
        assert_eq!(names.len(), 1 + SURFACE_PARTS.len());
    }

    #[test]
    fn the_materials_tab_reaches_the_projects_paint() {
        let mut p = Project::new("t");
        let id = p.add_wall(
            0,
            Point::ZERO,
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        assert!(!p.sync_wall_materials(id));
        p.floors[0]
            .wall_mut(id)
            .unwrap()
            .spec
            .materials
            .set("Exterior Wall Surface", Some(("Brick", [160, 70, 50])));
        assert!(p.sync_wall_materials(id));
        assert_eq!(p.object_material(id, "Exterior Wall Surface"), Some("Brick"));
        assert!(!p.sync_wall_materials(id));
        assert!(!p.sync_wall_materials(9999));
    }

    #[test]
    fn components_list_each_layer_net_of_the_openings() {
        let mut p = Project::new("t");
        let id = p.add_wall(
            0,
            Point::ZERO,
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let door = p.add_opening(0, id, 60.0, OpeningKind::Door).unwrap();
        let f = &p.floors[0];
        let w = f.wall(id).unwrap().clone();
        let o: Vec<Opening> = f.openings.clone();
        let hole = o.iter().find(|x| x.id == door).map(|d| d.width * d.height).unwrap();
        let ty = WallTypeDef {
            name: "T".into(),
            layers: vec![
                WallLayer::new("Siding", 1.0, false, "Siding"),
                WallLayer::new("Framing", 5.0, true, "Fir Framing"),
            ],
            kind: WallKind::Exterior,
        };
        let rows = wall_components(&w, Some(&ty), &o);
        assert_eq!(rows.len(), 2);
        let gross = 120.0 * 96.0;
        assert!((rows[0].area_sq_ft - (gross - hole) / 144.0).abs() < 1e-9);
        assert!((rows[1].volume_cu_ft - (gross - hole) * 5.0 / 1728.0).abs() < 1e-9);
        // Without a type the wall is one row.
        assert_eq!(wall_components(&w, None, &o)[0].name, "Wall");
        // A cap, a footing and a wainscot add rows.
        let mut w2 = w.clone();
        w2.spec.cap.enabled = true;
        w2.spec.foundation.footing = true;
        w2.spec.covering.interior.wainscot = "Beadboard".into();
        let names: Vec<_> = wall_components(&w2, Some(&ty), &o)
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert!(names.contains(&"Footing".to_string()));
        assert!(names.contains(&"Wall Cap".to_string()));
        assert!(names.contains(&"Wainscot (Interior)".to_string()));
    }

    #[test]
    fn a_wainscot_component_skips_the_door_opening() {
        let mut p = Project::new("t");
        let id = p.add_wall(
            0,
            Point::ZERO,
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        p.add_opening(0, id, 60.0, OpeningKind::Door).unwrap();
        let mut w = p.floors[0].wall(id).unwrap().clone();
        w.spec.covering.interior.wainscot = "Beadboard".into();
        let open = p.floors[0].openings.clone();
        let d = &open[0];
        let rows = wall_components(&w, None, &open);
        let wains = rows.iter().find(|r| r.name.starts_with("Wainscot")).unwrap();
        let want = (120.0 * 36.0 - d.width * 36.0_f64.min(d.height)) / 144.0;
        assert!((wains.area_sq_ft - want).abs() < 1e-9, "{}", wains.area_sq_ft);
    }

    #[test]
    fn the_schedule_defaults_include_the_wall_and_old_files_load() {
        let s = WallScheduleInfo::default();
        assert!(s.include);
        let spec: crate::walls::WallSpec = serde_json::from_str("{}").unwrap();
        assert!(spec.schedule.include);
        assert_eq!(spec.railing, WallRailing::default());
        assert!(spec.covering.is_empty());
        assert_eq!(spec.drawing_group, None);
        assert_eq!(drawing_group_name(29), "29 \u{2013} Wall");
        assert_eq!(drawing_group_name(7), "7");
    }

    #[test]
    fn the_new_tab_values_round_trip() {
        let mut w = wall();
        w.spec.railing.newel_style = NewelStyle::Round;
        w.spec.railing.balusters_per_bay = Some(5);
        w.spec.covering.exterior.crown = "x".into();
        w.spec.materials.set("Wall Cap", Some(("Oak", [1, 2, 3])));
        w.spec.info.id = "W-1".into();
        w.spec.schedule.include = false;
        w.spec.drawing_group = Some(27);
        let back: Wall = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert_eq!(back.spec, w.spec);
    }
}
