//! Wall Specification values that persist with a wall: the Structure tab's
//! platform intersections and wall intersections (W-39, W-60, W-62, W-63,
//! R-69), the Foundation tab (W-52) and the Wall Cap tab.
//!
//! The values live in [`WallSpec`] (`Wall.spec`, serde-default, so old files
//! load). What they do:
//!
//! * [`WallStructure::through_at_start`] / `through_at_end` make the wall run
//!   past the corner; the joined wall butts its face (`joins`).
//! * [`CeilingPlatform`] and [`FloorPlatform`] move the wall's top and bottom
//!   against the platforms between floors ([`platform_adjust`],
//!   [`Project::wall_platform_adjust`]); `plan-3d` builds the result.
//! * [`WallFoundation`] describes the footing under, and the sill plate on, a
//!   foundation wall ([`WallFoundation::footing_box`]).
//! * [`WallCap`] is the molding laid on top of the wall, usually a half wall.

use crate::defaults::WallTypeDef;
use crate::floors::FloorSettings;
use crate::model::{Project, Wall};
use crate::walls::MIN_WALL_THICKNESS;
use serde::{Deserialize, Serialize};

/// Smallest a main layer may be squeezed to by a thickness edit, inches.
/// The thinnest a Main layer may be (manual p. 399; DECISIONS 47): 1/16 in.
pub const MIN_MAIN_LAYER: f64 = crate::wall_types::MIN_MAIN_LAYER;

/// The Ceiling Platform choice of the Structure tab: how the wall meets the
/// ceiling and floor platforms above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CeilingPlatform {
    /// The platform above sits on the wall: the wall stops at its ceiling.
    #[default]
    Automatic,
    /// The wall stops at the ceiling platform (the same top, but a wall above
    /// no longer pulls it up).
    StopAtCeilingAbove,
    /// The wall runs up through the ceiling platform and the floor platform
    /// above, to the top of the floor platform (balloon framing).
    BalloonThroughCeilingAbove,
    /// The floor platform above hangs on the wall: the wall reaches the top
    /// of the floor platform, and the platform stops at the wall (its
    /// subflooring runs to the wall interior with `subfloor_to_interior`,
    /// a ledger carries the joists with `include_ledger`).
    HangFloorPlatformAbove {
        subfloor_to_interior: bool,
        include_ledger: bool,
    },
}

/// The Floor Platform choice of the Structure tab: how the wall meets the
/// platform it stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FloorPlatform {
    /// The wall stands on its floor platform.
    #[default]
    Automatic,
    /// The wall reaches down through its floor platform to the ceiling of the
    /// floor below.
    StopAtFloorBelow,
    /// The wall reaches down through its floor platform and the ceiling
    /// platform of the floor below (balloon framing).
    BalloonThroughFloorBelow,
}

/// The Structure tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallStructure {
    /// "Default Wall Top Height" is unchecked: the wall keeps its own height
    /// when the floor's ceiling height changes (W-60).
    pub custom_top: bool,
    /// "Default Wall Bottom Height" is unchecked: the wall keeps its own
    /// bottom (W-60).
    pub custom_bottom: bool,
    /// "Invisible Walls and Railings: Generate Between Platforms" (W-63).
    pub generate_between_platforms: bool,
    pub ceiling_platform: CeilingPlatform,
    pub floor_platform: FloorPlatform,
    /// Through Wall At Start: this wall runs past its start corner (W-39).
    pub through_at_start: bool,
    /// Through Wall At End.
    pub through_at_end: bool,
    /// "Bearing Wall" of the Framing section: the wall carries the floor or
    /// roof above. Stored with the wall; the framing builder does not read
    /// it yet (docs/integration-queue.md).
    pub bearing_wall: bool,
    /// Double Wall (W-149): how this wall behaves beside a parallel wall it
    /// touches. Not for curved walls.
    pub double: DoubleWall,
}

/// How a wall frames beside a parallel wall it touches (manual pp. 412-413).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DoubleWall {
    /// The basic double wall: both walls frame through, platforms and walls
    /// they meet are not split.
    #[default]
    FrameThrough,
    /// Platforms and the walls they meet split at the boundary between the two
    /// walls (modular units): no framing member crosses it.
    SplitFraming,
    /// The inner wall's layers count as extra layers of the primary wall;
    /// rooms come from the primary wall only, so a furred wall defines none.
    Furred,
}

impl DoubleWall {
    pub const ALL: [DoubleWall; 3] = [
        DoubleWall::FrameThrough,
        DoubleWall::SplitFraming,
        DoubleWall::Furred,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DoubleWall::FrameThrough => "Frame Through",
            DoubleWall::SplitFraming => "Split Framing",
            DoubleWall::Furred => "Furred Wall",
        }
    }
}

impl Default for WallStructure {
    fn default() -> Self {
        Self {
            custom_top: false,
            custom_bottom: false,
            generate_between_platforms: true,
            ceiling_platform: CeilingPlatform::Automatic,
            floor_platform: FloorPlatform::Automatic,
            through_at_start: false,
            through_at_end: false,
            bearing_wall: false,
            double: DoubleWall::FrameThrough,
        }
    }
}

impl WallStructure {
    /// The "Default Wall Bottom Height" box for a wall of bottom `bottom`: it
    /// can only be on while the wall stands on its floor.
    pub fn default_bottom(&self, bottom: f64) -> bool {
        !self.custom_bottom && bottom.abs() < 1e-9
    }
}

/// The Foundation tab (W-52).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallFoundation {
    /// The wall is a slab footing edge ("Slab Footing").
    pub slab_footing: bool,
    /// A footing under the wall.
    pub footing: bool,
    /// Footing width and height, inches.
    pub footing_width: f64,
    pub footing_height: f64,
    /// The footing bottom follows the wall bottom minus the footing height
    /// (Automatic Footing Bottom Height); off, `footing_bottom` is used.
    pub auto_bottom: bool,
    /// Footing bottom below the wall bottom, inches (negative: lower), when
    /// `auto_bottom` is off.
    pub footing_bottom: f64,
    /// The footing is a wall-high stem rather than a pad.
    pub vertical_footing: bool,
    /// Offset of the footing from the wall centerline, inches (positive:
    /// toward the wall's left, +normal side).
    pub footing_offset: f64,
    pub center_on_main_layer: bool,
    pub align_on_outside: bool,
    /// Slab options: chamfer on a monolithic or regular slab.
    pub chamfer_monolithic: bool,
    pub chamfer_regular: bool,
    pub chamfer_width: f64,
    pub chamfer_height: f64,
    pub pour_number: u32,
    /// A sill plate on top of the wall. Chief's tab starts with it checked;
    /// here it starts off so existing foundation walls keep their height.
    pub sill_plate: bool,
    pub sill_construction: String,
    /// "Create Wall/Footing Below": Build Foundation puts a foundation wall
    /// or footing under this wall of the floor above (an interior wall,
    /// railing or invisible wall; manual p. 742).
    pub create_below: bool,
}

/// The Sill Plate construction a new wall starts with.
pub const DEFAULT_SILL_CONSTRUCTION: &str = "Sill Plate";
/// Height of a sill plate, inches (one 2x).
pub const SILL_PLATE_HEIGHT: f64 = 1.5;
/// Width of a sill plate: a 2x6 laid flat, inches.
pub const SILL_PLATE_WIDTH: f64 = 5.5;

impl Default for WallFoundation {
    fn default() -> Self {
        Self {
            slab_footing: false,
            footing: false,
            footing_width: 24.0,
            footing_height: 12.0,
            auto_bottom: true,
            footing_bottom: 0.0,
            vertical_footing: false,
            footing_offset: 0.0,
            center_on_main_layer: false,
            align_on_outside: false,
            chamfer_monolithic: true,
            chamfer_regular: false,
            chamfer_width: 4.0,
            chamfer_height: 4.0,
            pour_number: 1,
            sill_plate: false,
            sill_construction: DEFAULT_SILL_CONSTRUCTION.to_string(),
            create_below: false,
        }
    }
}

/// A box in wall-local coordinates: `across` is the lateral extent along the
/// wall's +normal from the centerline, `up` the vertical extent from the wall
/// bottom (negative: below it). The box runs the wall's whole length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallBox {
    pub across: (f64, f64),
    pub up: (f64, f64),
}

impl WallFoundation {
    /// The footing under `wall` as a box. `None` when the wall has none.
    ///
    /// The width is centered on the wall (plus `footing_offset`), on the
    /// main layer with `center_on_main_layer`, or flush with the outside face
    /// with `align_on_outside`. The top meets the wall's bottom; a pad is
    /// `footing_height` thick below it (or down to `footing_bottom`).
    pub fn footing_box(&self, wall: &Wall, main_layer_center: f64) -> Option<WallBox> {
        if !self.footing || self.footing_width <= 0.0 || self.footing_height <= 0.0 {
            return None;
        }
        let w = self.footing_width;
        let half = wall.thickness * 0.5;
        let ext = wall.exterior_side.sign();
        let centre = if self.align_on_outside {
            // The footing's outside edge is flush with the outside face.
            ext * (half - w * 0.5)
        } else if self.center_on_main_layer {
            main_layer_center
        } else {
            0.0
        } + self.footing_offset;
        let bottom = if self.auto_bottom {
            -self.footing_height
        } else {
            self.footing_bottom.min(-1e-9)
        };
        let top = if self.vertical_footing {
            wall.height.max(self.footing_height)
        } else {
            0.0
        };
        Some(WallBox {
            across: (centre - w * 0.5, centre + w * 0.5),
            up: (bottom, top),
        })
    }

    /// The sill plate on top of `wall` (height `top` above its bottom) as a
    /// box, `None` when the wall has none. The plate is the narrower of
    /// [`SILL_PLATE_WIDTH`] and the wall thickness.
    pub fn sill_box(&self, wall: &Wall, top: f64) -> Option<WallBox> {
        if !self.sill_plate {
            return None;
        }
        let half = SILL_PLATE_WIDTH.min(wall.thickness) * 0.5;
        Some(WallBox {
            across: (-half, half),
            up: (top, top + SILL_PLATE_HEIGHT),
        })
    }
}

/// Which side of the wall a cap profile sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CapPosition {
    #[default]
    Inside,
    Center,
    Outside,
}

/// A profile of the Wall Cap table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapProfile {
    pub name: String,
    /// Width across the wall, inches; ignored with `WallCap::full_wall_width`.
    pub width: f64,
    pub height: f64,
    pub horizontal_offset: f64,
    pub vertical_offset: f64,
}

/// The profiles the Wall Cap table starts with.
pub fn default_cap_profiles() -> Vec<CapProfile> {
    let p = |name: &str, width: f64, height: f64| CapProfile {
        name: name.to_string(),
        width,
        height,
        horizontal_offset: 0.0,
        vertical_offset: 0.0,
    };
    vec![
        p("Flat Cap", 5.5, 1.5),
        p("Overhanging Cap", 9.5, 2.0),
        p("Thick Coping", 7.5, 3.0),
    ]
}

/// The Wall Cap tab: a cap laid on the top of the wall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallCap {
    pub enabled: bool,
    /// Name of the selected [`default_cap_profiles`] entry.
    pub profile: String,
    /// The cap is as wide as the wall instead of its profile width.
    pub full_wall_width: bool,
    pub position: CapPosition,
    /// The cap is also put on the lower wall of a pony wall.
    pub split_pony_wall: bool,
}

impl Default for WallCap {
    fn default() -> Self {
        Self {
            enabled: false,
            profile: "Flat Cap".to_string(),
            full_wall_width: true,
            position: CapPosition::Inside,
            split_pony_wall: false,
        }
    }
}

impl WallCap {
    /// The selected profile of the default table (the first when the name is
    /// unknown).
    pub fn selected(&self) -> CapProfile {
        let table = default_cap_profiles();
        table
            .iter()
            .find(|p| p.name == self.profile)
            .unwrap_or(&table[0])
            .clone()
    }

    /// The cap on `wall` as a box above the wall top (`up` is measured from
    /// the top, so `(0, height)` is the cap resting on it). `None` when the
    /// wall has no cap.
    pub fn cap_box(&self, wall: &Wall) -> Option<WallBox> {
        if !self.enabled {
            return None;
        }
        let p = self.selected();
        let width = if self.full_wall_width {
            wall.thickness
        } else {
            p.width
        };
        let half = wall.thickness * 0.5;
        let ext = wall.exterior_side.sign();
        // `Inside` hugs the interior face, `Outside` the exterior face.
        let centre = match self.position {
            CapPosition::Center => 0.0,
            CapPosition::Inside => -ext * (half - width * 0.5),
            CapPosition::Outside => ext * (half - width * 0.5),
        } + p.horizontal_offset;
        Some(WallBox {
            across: (centre - width * 0.5, centre + width * 0.5),
            up: (p.vertical_offset, p.vertical_offset + p.height),
        })
    }
}

/// Every Wall Specification value that is stored with the wall.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WallSpec {
    pub structure: WallStructure,
    pub foundation: WallFoundation,
    pub cap: WallCap,
    /// Wall Covering tab (W-115).
    pub covering: crate::walls::spec_tabs::WallCovering,
    /// Newels/Balusters and Rails tabs of a railing wall (W-116, W-117).
    pub railing: crate::walls::spec_tabs::WallRailing,
    /// Materials tab.
    pub materials: crate::walls::spec_tabs::WallMaterials,
    /// Object Information tab (W-118).
    pub info: crate::walls::spec_tabs::WallInfo,
    /// Schedule tab.
    pub schedule: crate::walls::spec_tabs::WallScheduleInfo,
    /// Layer tab's Drawing Group; `None` is the default group
    /// ([`crate::walls::spec_tabs::DEFAULT_DRAWING_GROUP`]).
    pub drawing_group: Option<u32>,
    /// Stepped and raked top and bottom edges (W-141).
    pub profile: crate::walls::profile::WallProfile,
    /// Layers slid along the wall by Edit Wall Intersections (W-144).
    pub layer_joins: Vec<crate::walls::intersect::LayerJoin>,
}

/// How a wall's platform options move its top and bottom, inches.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlatformAdjust {
    /// How far the bottom reaches below the wall's own bottom (zero or more).
    pub lower: f64,
    /// How far the top reaches above the wall's own top (zero or more).
    pub raise: f64,
}

/// The platform thicknesses a wall meets: its own floor's settings and those
/// of the floors above and below (`None` where there is no such floor).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlatformContext {
    pub this: PlatformThickness,
    pub above: Option<PlatformThickness>,
    pub below: Option<PlatformThickness>,
}

/// The two thicknesses of [`FloorSettings`] a platform intersection needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlatformThickness {
    pub floor: f64,
    pub ceiling: f64,
}

impl From<&FloorSettings> for PlatformThickness {
    fn from(s: &FloorSettings) -> Self {
        Self {
            floor: s.floor_structure_thickness,
            ceiling: s.ceiling_structure_thickness,
        }
    }
}

impl PlatformContext {
    /// The context of floor `idx` of `project`.
    pub fn of(project: &Project, idx: usize) -> Self {
        let settings = |i: usize| {
            project
                .floors
                .get(i)
                .map(|f| PlatformThickness::from(&f.settings))
        };
        Self {
            this: settings(idx).unwrap_or(PlatformThickness {
                floor: 0.0,
                ceiling: 0.0,
            }),
            above: settings(idx + 1),
            below: idx.checked_sub(1).and_then(settings),
        }
    }
}

/// How far the platform options of `s` move a wall's bottom and top.
///
/// * Floor Platform *Stop at Floor Below*: the bottom drops by this floor's
///   floor platform; *Balloon Through Floor Below* also by the ceiling
///   platform of the floor below. With no floor below nothing moves.
/// * Ceiling Platform *Balloon Through Ceiling Above* and *Hang Floor
///   Platform Above*: the top rises by this floor's ceiling platform and the
///   floor platform of the floor above, to the top of that platform. With no
///   floor above nothing moves. *Automatic* and *Stop at Ceiling Above* keep
///   the top at the ceiling.
pub fn platform_adjust(s: &WallStructure, ctx: &PlatformContext) -> PlatformAdjust {
    let mut a = PlatformAdjust::default();
    if let Some(below) = ctx.below {
        a.lower = match s.floor_platform {
            FloorPlatform::Automatic => 0.0,
            FloorPlatform::StopAtFloorBelow => ctx.this.floor,
            FloorPlatform::BalloonThroughFloorBelow => ctx.this.floor + below.ceiling,
        };
    }
    if let Some(above) = ctx.above {
        a.raise = match s.ceiling_platform {
            CeilingPlatform::Automatic | CeilingPlatform::StopAtCeilingAbove => 0.0,
            CeilingPlatform::BalloonThroughCeilingAbove
            | CeilingPlatform::HangFloorPlatformAbove { .. } => ctx.this.ceiling + above.floor,
        };
    }
    a
}

impl Project {
    /// The platform adjustment of `wall` standing on floor `floor`.
    pub fn wall_platform_adjust(&self, floor: usize, wall: &Wall) -> PlatformAdjust {
        platform_adjust(&wall.spec.structure, &PlatformContext::of(self, floor))
    }

    /// Whether the floor above `floor` is higher than the wall tops of
    /// `floor`, so that invisible walls and railings are generated between the
    /// platforms ("Generate Between Platforms", W-63): the gap in inches
    /// between the top of `wall` and the floor above (zero when none).
    pub fn platform_gap_above(&self, floor: usize, wall: &Wall) -> f64 {
        let (Some(f), Some(up)) = (self.floors.get(floor), self.floors.get(floor + 1)) else {
            return 0.0;
        };
        (up.elevation - (f.elevation + wall.bottom_offset + wall.height)).max(0.0)
    }
}

/// The fixed part of a wall type's thickness, inches: the layers other than
/// the main layer, which the thickness edit stretches.
pub fn fixed_layers_thickness(ty: &WallTypeDef) -> f64 {
    ty.layers
        .iter()
        .filter(|l| !l.is_main)
        .map(|l| l.thickness)
        .sum()
}

/// The thinnest `wall` may be made: the layers of its type other than the
/// outermost main layer, plus [`MIN_MAIN_LAYER`] for that layer (or just the sum of
/// all layers for a type with no main layer). A wall with no type may be as
/// thin as [`MIN_WALL_THICKNESS`].
pub fn min_thickness(ty: Option<&WallTypeDef>) -> f64 {
    match ty {
        None => MIN_WALL_THICKNESS,
        Some(t) if t.layers.is_empty() => MIN_WALL_THICKNESS,
        // A Room Divider type is 0 in thick.
        Some(t) if t.props.room_divider => 0.0,
        // The total cannot drop below the old total less the outermost Main
        // layer, plus that layer at its least (1/16 in; DECISIONS 47).
        Some(t) if t.layers.iter().any(|l| l.is_main) => {
            let outer = t.main_layer().map_or(0.0, |l| l.thickness);
            t.thickness() - outer + MIN_MAIN_LAYER
        }
        Some(t) => t.thickness().max(MIN_WALL_THICKNESS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::WallLayer;
    use crate::geometry::Point;
    use crate::model::WallKind;

    fn wall() -> Wall {
        Wall::new(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        )
    }

    fn ctx(
        this: (f64, f64),
        above: Option<(f64, f64)>,
        below: Option<(f64, f64)>,
    ) -> PlatformContext {
        let s = |(floor, ceiling): (f64, f64)| PlatformThickness { floor, ceiling };
        PlatformContext {
            this: s(this),
            above: above.map(s),
            below: below.map(s),
        }
    }

    #[test]
    fn automatic_platforms_move_nothing() {
        let a = platform_adjust(
            &WallStructure::default(),
            &ctx((10.25, 2.0), Some((10.25, 2.0)), Some((10.25, 2.0))),
        );
        assert_eq!(a, PlatformAdjust::default());
    }

    #[test]
    fn balloon_through_the_ceiling_rises_to_the_top_of_the_floor_above() {
        let mut s = WallStructure {
            ceiling_platform: CeilingPlatform::BalloonThroughCeilingAbove,
            ..WallStructure::default()
        };
        let a = platform_adjust(&s, &ctx((10.25, 2.0), Some((11.0, 0.0)), None));
        assert!((a.raise - 13.0).abs() < 1e-9, "{a:?}");
        assert_eq!(a.lower, 0.0);
        // Hanging the floor platform reaches the same height.
        s.ceiling_platform = CeilingPlatform::HangFloorPlatformAbove {
            subfloor_to_interior: true,
            include_ledger: false,
        };
        let h = platform_adjust(&s, &ctx((10.25, 2.0), Some((11.0, 0.0)), None));
        assert!((h.raise - 13.0).abs() < 1e-9);
        // Stop at ceiling above keeps the top at the ceiling.
        s.ceiling_platform = CeilingPlatform::StopAtCeilingAbove;
        assert_eq!(
            platform_adjust(&s, &ctx((10.25, 2.0), Some((11.0, 0.0)), None)).raise,
            0.0
        );
        // No floor above: nothing to balloon through.
        s.ceiling_platform = CeilingPlatform::BalloonThroughCeilingAbove;
        assert_eq!(
            platform_adjust(&s, &ctx((10.25, 2.0), None, None)).raise,
            0.0
        );
    }

    #[test]
    fn floor_platform_options_lower_the_bottom() {
        let mut s = WallStructure {
            floor_platform: FloorPlatform::StopAtFloorBelow,
            ..WallStructure::default()
        };
        let below = Some((10.25, 1.5));
        let a = platform_adjust(&s, &ctx((10.25, 0.0), None, below));
        assert!((a.lower - 10.25).abs() < 1e-9);
        s.floor_platform = FloorPlatform::BalloonThroughFloorBelow;
        let b = platform_adjust(&s, &ctx((10.25, 0.0), None, below));
        assert!((b.lower - 11.75).abs() < 1e-9);
        // The ground floor has no floor below.
        assert_eq!(
            platform_adjust(&s, &ctx((10.25, 0.0), None, None)).lower,
            0.0
        );
    }

    #[test]
    fn the_project_resolves_the_floors_around_a_wall() {
        let mut p = Project::new("t");
        p.build_new_floor(false);
        let mut w = wall();
        w.spec.structure.ceiling_platform = CeilingPlatform::BalloonThroughCeilingAbove;
        let a = p.wall_platform_adjust(0, &w);
        let want = p.floors[0].settings.ceiling_structure_thickness
            + p.floors[1].settings.floor_structure_thickness;
        assert!((a.raise - want).abs() < 1e-9 && a.raise > 0.0, "{a:?}");
        // The upper floor has nothing above it.
        assert_eq!(p.wall_platform_adjust(1, &w).raise, 0.0);
    }

    #[test]
    fn a_wall_with_a_footing_gets_a_box_under_it() {
        let w = wall();
        let mut f = WallFoundation::default();
        assert!(f.footing_box(&w, 0.0).is_none());
        f.footing = true;
        let b = f.footing_box(&w, 0.0).unwrap();
        assert_eq!(b.across, (-12.0, 12.0));
        assert_eq!(b.up, (-12.0, 0.0));
        f.align_on_outside = true;
        let b = f.footing_box(&w, 0.0).unwrap();
        // Left is the exterior side by default: the footing's outer edge is
        // flush with the outside face at +3.
        assert!((b.across.1 - 3.0).abs() < 1e-9, "{b:?}");
        f.auto_bottom = false;
        f.footing_bottom = -20.0;
        assert_eq!(f.footing_box(&w, 0.0).unwrap().up.0, -20.0);
        f.vertical_footing = true;
        assert_eq!(f.footing_box(&w, 0.0).unwrap().up.1, 96.0);
    }

    #[test]
    fn the_sill_plate_sits_on_the_wall() {
        let w = wall();
        let f = WallFoundation {
            sill_plate: true,
            ..WallFoundation::default()
        };
        assert!(WallFoundation::default().sill_box(&w, 96.0).is_none());
        let b = f.sill_box(&w, 96.0).unwrap();
        assert_eq!(b.up, (96.0, 97.5));
        assert!((b.across.1 - 2.75).abs() < 1e-9 || (b.across.1 - 3.0).abs() < 1e-9);
        let off = WallFoundation {
            sill_plate: false,
            ..f
        };
        assert!(off.sill_box(&w, 96.0).is_none());
    }

    #[test]
    fn a_cap_lies_on_the_wall_top() {
        let w = wall();
        let mut c = WallCap::default();
        assert!(c.cap_box(&w).is_none());
        c.enabled = true;
        let b = c.cap_box(&w).unwrap();
        assert_eq!(b.across, (-3.0, 3.0));
        assert_eq!(b.up, (0.0, 1.5));
        c.full_wall_width = false;
        c.profile = "Overhanging Cap".into();
        c.position = CapPosition::Center;
        let b = c.cap_box(&w).unwrap();
        assert_eq!(b.across, (-4.75, 4.75));
        assert_eq!(b.up, (0.0, 2.0));
        c.profile = "nope".into();
        assert_eq!(c.selected().name, "Flat Cap");
    }

    #[test]
    fn the_minimum_thickness_is_the_fixed_layers_plus_a_sliver() {
        let ty = WallTypeDef {
            props: Default::default(),
            name: "T".into(),
            layers: vec![
                WallLayer::new("Siding", 0.75, false, "Siding"),
                WallLayer::new("Framing", 3.5, true, "Fir Framing"),
                WallLayer::new("Drywall", 0.5, false, "Drywall"),
            ],
            kind: WallKind::Exterior,
        };
        assert!((min_thickness(Some(&ty)) - (1.25 + MIN_MAIN_LAYER)).abs() < 1e-9);
        assert_eq!(min_thickness(None), MIN_WALL_THICKNESS);
        let no_main = WallTypeDef {
            props: Default::default(),
            layers: vec![
                WallLayer::new("A", 2.0, false, "A"),
                WallLayer::new("B", 1.0, false, "B"),
            ],
            ..ty
        };
        assert_eq!(min_thickness(Some(&no_main)), 3.0);
    }

    #[test]
    fn the_spec_defaults_load_from_old_json() {
        let mut v = serde_json::to_value(wall()).unwrap();
        v.as_object_mut().unwrap().remove("spec");
        let w: Wall = serde_json::from_value(v).unwrap();
        assert_eq!(w.spec, WallSpec::default());
        assert!(w.spec.structure.generate_between_platforms);
        let mut w2 = w.clone();
        w2.spec.structure.through_at_end = true;
        w2.spec.cap.enabled = true;
        w2.spec.foundation.footing = true;
        let back: Wall = serde_json::from_str(&serde_json::to_string(&w2).unwrap()).unwrap();
        assert_eq!(back.spec, w2.spec);
    }

    #[test]
    fn the_default_bottom_box_needs_a_wall_on_its_floor() {
        let s = WallStructure::default();
        assert!(s.default_bottom(0.0));
        assert!(!s.default_bottom(24.0));
        let c = WallStructure {
            custom_bottom: true,
            ..s
        };
        assert!(!c.default_bottom(0.0));
    }
}
