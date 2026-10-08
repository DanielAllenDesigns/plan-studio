//! Plan defaults: everything a new plan starts with, the way Chief Architect
//! starts from a template plan (wall types, wall/door/window/cabinet/dimension
//! defaults, room types, layers, grid). The values of
//! [`PlanDefaults::chief_x18_daniel`] are the ones captured from Daniel's
//! Chief X18 template (`docs/chief-x18-dialogs.md`). Lengths are inches.

use crate::dimension::DimFormat;
use crate::layers::LayerSet;
use crate::model::{Project, WallKind, DEFAULT_CEILING_HEIGHT};
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
    fn new(name: &str, thickness: f64, is_main: bool, material: &str) -> Self {
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
}

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
    pub wall_types: Vec<WallTypeDef>,
    pub interior_door: OpeningDefaults,
    pub exterior_door: OpeningDefaults,
    pub window: WindowDefaults,
    pub cabinets: CabinetDefaults,
    pub dimensions: DimensionDefaults,
    pub rooms: RoomDefaults,
    pub layers: LayerSet,
    pub text: TextDefaults,
    pub grid: GridDefaults,
    pub units: UnitDefaults,
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

    /// The dimension text format these defaults describe.
    pub fn dim_format(&self) -> DimFormat {
        DimFormat {
            smallest_fraction: self.dimensions.smallest_fraction.max(1),
            unit_indicators: self.dimensions.unit_indicators,
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
            },
            dimensions: DimensionDefaults {
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
            },
            rooms: RoomDefaults {
                ceiling_height: DEFAULT_CEILING_HEIGHT,
                floor_finish_thickness: 0.75,
                ceiling_finish_thickness: 0.625,
                room_types: chief_room_types(),
            },
            layers: LayerSet::default_floor_plan(),
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
        p.layers = d.layers.clone();
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
}
