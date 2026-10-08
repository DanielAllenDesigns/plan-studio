//! Turns a [`TemplateInventory`] into Plan Studio defaults.
//!
//! Layer sets, text styles and dimension default sets are filled into the
//! seeded `PlanDefaults` (and also travel in [`TemplateSeed`] so callers can
//! apply them to other defaults with [`apply_seed`]); plan view and layout page
//! names have no slot in `PlanDefaults` and travel in [`TemplateSeed`] only.

use crate::classify::{Category, TemplateInventory};
use crate::values::{LayerSetData, WallStack};
use plan_core::defaults::{
    DimensionDefaultSet, DimensionDefaults, PlanDefaults, WallLayer, WallTypeDef,
};
use plan_core::layer_sets::{LayerSetDef, LayerSets, LayerState, DEFAULT_LAYER_SET_NAME};
use plan_core::layers::Layer;
use plan_core::model::WallKind;
use plan_core::text_styles::{plan_height_for_printed, TextStyle, TextStyles};
use serde::{Deserialize, Serialize};

/// Line weight (1/100 mm) for a layer whose value was not decoded.
const FALLBACK_LINE_WEIGHT: u32 = 18;
/// Thickness guessed for a wall type name without a number, inches.
const FALLBACK_WALL_THICKNESS: f64 = 4.5;
/// Added to the number in a wall type name to approximate its total
/// thickness (finishes), inches.
const NAME_NUMBER_ALLOWANCE: f64 = 1.0;
/// Layer sets tried first when picking colours and weights for new layers.
const PREFERRED_SOURCE_SETS: [&str; 2] = ["Working Layer Set", "Floor Plan Dimensioned Layer Set"];

/// A layer set from the template and, when its records decoded, the layers it
/// shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerSetSeed {
    pub name: String,
    pub visible_layers: Option<Vec<String>>,
}

/// Layout-template vocabulary.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LayoutSeed {
    pub sheet_size: Option<String>,
    pub pages: Vec<String>,
    pub macros: Vec<String>,
}

/// Defaults plus the template vocabulary that has no home in `PlanDefaults`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSeed {
    pub defaults: PlanDefaults,
    pub layer_sets: Vec<LayerSetSeed>,
    pub text_styles: Vec<String>,
    pub dimension_sets: Vec<String>,
    pub plan_views: Vec<String>,
    pub layout: LayoutSeed,
    /// Wall types added to `defaults` by this call.
    pub added_wall_types: Vec<String>,
    /// Added wall types whose thickness was guessed from the name.
    pub approximate_wall_types: Vec<String>,
    /// Layers added to `defaults.layers` by this call.
    pub added_layers: Vec<String>,
    /// Decoded layer sets (named ones only) with per-layer display, lock,
    /// colour and line weight. Empty when no table decoded.
    #[serde(default = "empty_layer_sets")]
    pub layer_set_defs: LayerSets,
    /// Text styles from the template's names.
    #[serde(default)]
    pub text_style_defs: TextStyles,
    /// Dimension default sets from the template's names.
    #[serde(default)]
    pub dimension_set_defs: Vec<DimensionDefaultSet>,
}

fn empty_layer_sets() -> LayerSets {
    LayerSets {
        sets: Vec::new(),
        active: String::new(),
    }
}

impl TemplateSeed {
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

/// Lower-cased name with runs of whitespace collapsed; used to avoid adding
/// `Walls,  Normal` next to an existing `Walls, Normal`.
pub fn collapse(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Best-effort thickness number in a wall type name: `Siding-6` is 6,
/// `Frame-3 1/2` is 3.5, `8" CMU (block) Stem Wall` is 8.
pub fn thickness_hint(name: &str) -> Option<f64> {
    let b = name.as_bytes();
    let parse_int = |from: usize| -> Option<(f64, usize)> {
        let digits = b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
        (digits > 0).then(|| {
            (
                name[from..from + digits].parse().unwrap_or(0.0),
                from + digits,
            )
        })
    };
    for (i, _) in name.match_indices('-') {
        if let Some((whole, mut end)) = parse_int(i + 1) {
            let mut value = whole;
            // Optional " 1/2" fraction.
            if b.get(end) == Some(&b' ') {
                if let Some((num, e1)) = parse_int(end + 1) {
                    if b.get(e1) == Some(&b'/') {
                        if let Some((den, e2)) = parse_int(e1 + 1) {
                            if den > 0.0 {
                                value += num / den;
                                end = e2;
                            }
                        }
                    }
                }
            }
            let _ = end;
            return Some(value);
        }
    }
    if let Some((n, end)) = parse_int(0) {
        if b.get(end) == Some(&b'"') {
            return Some(n);
        }
    }
    None
}

fn guess_kind(name: &str) -> WallKind {
    let lower = name.to_ascii_lowercase();
    let interior = [
        "interior",
        "existing interior",
        "steel interior",
        "room divider",
        "fire",
        "frame",
        "demolition",
        "seperation",
        "separation",
        "sip",
        "soundbarrier",
    ];
    if interior.iter().any(|p| lower.starts_with(p)) {
        WallKind::Interior
    } else {
        WallKind::Exterior
    }
}

fn stack_to_layers(stack: &WallStack) -> Vec<WallLayer> {
    let main = stack
        .thicknesses
        .iter()
        .enumerate()
        .fold(
            (0, f64::MIN),
            |best, (i, &t)| if t > best.1 { (i, t) } else { best },
        )
        .0;
    stack
        .thicknesses
        .iter()
        .enumerate()
        .map(|(i, &t)| WallLayer {
            name: if i == main {
                "Main".into()
            } else {
                format!("Layer {}", i + 1)
            },
            thickness: t,
            is_main: i == main,
            material: String::new(),
        })
        .collect()
}

fn approximate_type(name: &str) -> WallTypeDef {
    let thickness =
        thickness_hint(name).map_or(FALLBACK_WALL_THICKNESS, |n| n + NAME_NUMBER_ALLOWANCE);
    WallTypeDef {
        name: name.to_string(),
        layers: vec![WallLayer {
            name: "Main (approximate)".into(),
            thickness,
            is_main: true,
            material: "approximate".into(),
        }],
        kind: guess_kind(name),
    }
}

fn source_set(sets: &[LayerSetData]) -> Option<&LayerSetData> {
    PREFERRED_SOURCE_SETS
        .iter()
        .find_map(|p| {
            sets.iter()
                .find(|s| s.named && s.name.eq_ignore_ascii_case(p))
        })
        .or_else(|| sets.first())
}

fn names(inv: &TemplateInventory, c: Category) -> Vec<String> {
    inv.entries(c).iter().map(|e| e.name.clone()).collect()
}

fn pick_sheet_size(inv: &TemplateInventory) -> Option<String> {
    // Prefer the size whose dimensions appear in the template's file name
    // ("18x24 PRESENTATION LAYOUT TEMPLATE" -> `ARCH C (18" x 24")`).
    let file = inv.file_name.to_ascii_lowercase();
    let from_file_name = inv.sheet_sizes.iter().find(|e| {
        let nums: Vec<String> = e
            .name
            .split(|c: char| !c.is_ascii_digit())
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect();
        nums.len() >= 2 && nums.iter().all(|n| file.contains(n.as_str()))
    });
    // A file name like "18x24 ..." that matches no listed size means the
    // sheet is stored some other way; do not substitute an unrelated size.
    let names_dimensions = file
        .as_bytes()
        .windows(3)
        .any(|w| w[0].is_ascii_digit() && w[1] == b'x' && w[2].is_ascii_digit());
    from_file_name
        .or_else(|| {
            if names_dimensions {
                None
            } else {
                inv.sheet_sizes.first()
            }
        })
        .map(|e| e.name.clone())
}

/// Layer sets with their decoded per-layer state. Unnamed tables are skipped
/// and the first table of a repeated name wins. The active set is "Working
/// Layer Set" when present, else "Floor Plan Dimensioned Layer Set", else the
/// first. Empty when nothing decoded.
pub fn seed_layer_sets(inv: &TemplateInventory) -> LayerSets {
    let mut sets: Vec<LayerSetDef> = Vec::new();
    for data in inv.layer_set_data.iter().filter(|d| d.named) {
        if sets.iter().any(|s| s.name == data.name) {
            continue;
        }
        let mut def = LayerSetDef::new(data.name.clone());
        for l in &data.layers {
            if def.state(&l.name).is_some() {
                continue;
            }
            def.states.push(LayerState {
                layer: l.name.clone(),
                display: l.display,
                locked: l.locked,
                color: Some(l.color),
                line_weight: Some(u32::from(l.line_weight)),
                line_style: None,
                text_style: None,
            });
        }
        sets.push(def);
    }
    let active = PREFERRED_SOURCE_SETS
        .iter()
        .find_map(|p| sets.iter().find(|s| s.name == *p))
        .or_else(|| sets.first())
        .map(|s| s.name.clone())
        .unwrap_or_default();
    LayerSets { sets, active }
}

/// Scale in paper inches per foot named by a `<scale>" Text Style` name:
/// `1/4" Text Style` is 0.25, `1" Text Style` is 1.
fn text_style_scale(name: &str) -> Option<f64> {
    let head = name.strip_suffix("\" Text Style")?;
    let (num, den) = match head.split_once('/') {
        Some((n, d)) => (n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?),
        None => (head.trim().parse::<f64>().ok()?, 1.0),
    };
    (num > 0.0 && den > 0.0).then_some(num / den)
}

/// One style per text style name in the template. Names Plan Studio already
/// ships keep those values; `<scale>" Text Style` names get the plan height
/// that prints 1/8" at that scale; others copy "Default Text Style".
pub fn seed_text_styles(inv: &TemplateInventory) -> TextStyles {
    let known = TextStyles::chief_defaults();
    let mut out = TextStyles { styles: Vec::new() };
    for e in &inv.text_styles {
        if out.get(&e.name).is_some() {
            continue;
        }
        let style = if let Some(k) = known.get(&e.name) {
            k.clone()
        } else if let Some(scale) = text_style_scale(&e.name) {
            TextStyle::plan_sized(e.name.clone(), plan_height_for_printed(0.125, scale), false)
        } else {
            TextStyle {
                name: e.name.clone(),
                ..TextStyle::default()
            }
        };
        out.add(style);
    }
    out
}

fn dimension_sets_from(
    inv: &TemplateInventory,
    base: &DimensionDefaults,
) -> Vec<DimensionDefaultSet> {
    let mut out: Vec<DimensionDefaultSet> = Vec::new();
    for e in &inv.dimension_defaults {
        let name = e
            .name
            .strip_suffix(" Dimension Defaults")
            .unwrap_or(&e.name)
            .trim();
        if name.is_empty() || out.iter().any(|s| s.name.eq_ignore_ascii_case(name)) {
            continue;
        }
        out.push(DimensionDefaultSet::new(name, base.clone()));
    }
    out
}

/// One dimension default set per name in the template (the
/// ` Dimension Defaults` suffix is dropped); every set starts as a copy of
/// the default dimension settings.
pub fn seed_dimension_sets(inv: &TemplateInventory) -> Vec<DimensionDefaultSet> {
    dimension_sets_from(inv, &PlanDefaults::default().dimensions)
}

/// Layer-set states named like an existing layer up to whitespace/case use
/// that layer's exact name, so `Walls,  Normal` finds `Walls, Normal`.
fn canonical_sets(sets: &LayerSets, layers: &plan_core::layers::LayerSet) -> Vec<LayerSetDef> {
    let by_key: std::collections::HashMap<String, &str> = layers
        .layers
        .iter()
        .map(|l| (collapse(&l.name), l.name.as_str()))
        .collect();
    sets.sets
        .iter()
        .map(|set| {
            let mut out = LayerSetDef::new(set.name.clone());
            for st in &set.states {
                let name = by_key
                    .get(&collapse(&st.layer))
                    .map_or(st.layer.as_str(), |n| *n);
                if out.state(name).is_some() {
                    continue;
                }
                let mut st = st.clone();
                st.layer = name.to_string();
                out.states.push(st);
            }
            out
        })
        .collect()
}

/// Adds the seeded layer sets, text styles and dimension sets to `defaults`.
/// A project-default layer set list that is still the lone "Default Set" is
/// replaced by the template's sets; otherwise only missing sets are added.
/// Existing text styles and dimension sets are kept, and the active
/// dimension set never changes.
fn merge_vocabulary(
    defaults: &mut PlanDefaults,
    layer_sets: &LayerSets,
    text_styles: &TextStyles,
    dimension_sets: &[DimensionDefaultSet],
) {
    if !layer_sets.sets.is_empty() {
        let canon = canonical_sets(layer_sets, &defaults.layers);
        let ls = &mut defaults.layer_sets;
        if ls.sets.len() == 1 && ls.sets[0].name == DEFAULT_LAYER_SET_NAME {
            ls.sets = canon;
            ls.active = layer_sets.active.clone();
        } else {
            for set in canon {
                ls.add_set(set);
            }
        }
    }
    for st in &text_styles.styles {
        defaults.text_styles.add(st.clone());
    }
    for ds in dimension_sets {
        if defaults.dimension_set(&ds.name).is_none() {
            defaults.dimension_sets.push(ds.clone());
        }
    }
}

/// Merges a seed into existing defaults: wall types and layers the defaults
/// lack, plus the seeded layer sets, text styles and dimension sets (see
/// `seed_defaults`'s merge rules: a lone "Default Set" is replaced by the template's sets). Existing entries are never changed.
pub fn apply_seed(defaults: &mut PlanDefaults, seed: &TemplateSeed) {
    for t in &seed.defaults.wall_types {
        if defaults.wall_type(&t.name).is_none() {
            defaults.wall_types.push(t.clone());
        }
    }
    let mut known: std::collections::HashSet<String> = defaults
        .layers
        .layers
        .iter()
        .map(|l| collapse(&l.name))
        .collect();
    for l in &seed.defaults.layers.layers {
        if known.insert(collapse(&l.name)) {
            defaults.layers.add(l.clone());
        }
    }
    merge_vocabulary(
        defaults,
        &seed.layer_set_defs,
        &seed.text_style_defs,
        &seed.dimension_set_defs,
    );
}

/// `defaults.apply_seed(&seed)`.
pub trait ApplySeed {
    fn apply_seed(&mut self, seed: &TemplateSeed);
}

impl ApplySeed for PlanDefaults {
    fn apply_seed(&mut self, seed: &TemplateSeed) {
        apply_seed(self, seed);
    }
}

/// Builds the seed: wall types, layers, layer sets, text styles, dimension
/// sets and the template vocabulary. Existing entries of `base` are never
/// changed or replaced.
pub fn seed_defaults(inv: &TemplateInventory, base: PlanDefaults) -> TemplateSeed {
    let mut defaults = base;
    let mut added_wall_types = Vec::new();
    let mut approximate_wall_types = Vec::new();

    for e in &inv.wall_types {
        if defaults.wall_type(&e.name).is_some() {
            continue;
        }
        let def = match inv.wall_stacks.iter().find(|s| s.name == e.name) {
            Some(stack) => WallTypeDef {
                name: e.name.clone(),
                layers: stack_to_layers(stack),
                kind: guess_kind(&e.name),
            },
            None => {
                approximate_wall_types.push(e.name.clone());
                approximate_type(&e.name)
            }
        };
        added_wall_types.push(e.name.clone());
        defaults.wall_types.push(def);
    }

    // Layers: colours and weights from the best decoded set.
    let source = source_set(&inv.layer_set_data);
    let mut added_layers = Vec::new();
    let mut known: std::collections::HashSet<String> = defaults
        .layers
        .layers
        .iter()
        .map(|l| collapse(&l.name))
        .collect();
    for e in &inv.layers {
        if !known.insert(collapse(&e.name)) {
            continue;
        }
        let decoded = source.and_then(|s| {
            s.layers
                .iter()
                .find(|l| collapse(&l.name) == collapse(&e.name))
        });
        let (color, weight) = decoded.map_or(([0, 0, 0], FALLBACK_LINE_WEIGHT), |l| {
            (l.color, u32::from(l.line_weight))
        });
        // Visibility is per layer set, so new layers start visible.
        if defaults
            .layers
            .add(Layer::new(e.name.clone(), color, weight))
        {
            added_layers.push(e.name.clone());
        }
    }

    // Layer sets, first decoded table per name.
    let mut layer_sets: Vec<crate::bridge::LayerSetSeed> = Vec::new();
    for e in &inv.layer_sets {
        if layer_sets
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(&e.name))
        {
            continue;
        }
        let visible = inv
            .layer_set_data
            .iter()
            .find(|s| s.named && s.name.eq_ignore_ascii_case(&e.name))
            .map(LayerSetData::visible_layers);
        layer_sets.push(LayerSetSeed {
            name: e.name.clone(),
            visible_layers: visible,
        });
    }

    let layer_set_defs = seed_layer_sets(inv);
    let text_style_defs = seed_text_styles(inv);
    let dimension_set_defs = dimension_sets_from(inv, &defaults.dimensions);
    merge_vocabulary(
        &mut defaults,
        &layer_set_defs,
        &text_style_defs,
        &dimension_set_defs,
    );

    TemplateSeed {
        defaults,
        layer_sets,
        text_styles: names(inv, Category::TextStyle),
        dimension_sets: names(inv, Category::DimensionDefaults),
        plan_views: names(inv, Category::PlanView),
        layout: LayoutSeed {
            sheet_size: pick_sheet_size(inv),
            pages: names(inv, Category::LayoutPage),
            macros: names(inv, Category::TitleBlockMacro),
        },
        added_wall_types,
        approximate_wall_types,
        added_layers,
        layer_set_defs,
        text_style_defs,
        dimension_set_defs,
    }
}

/// Just the seeded `PlanDefaults` of [`seed_defaults`].
pub fn seed_plan_defaults(inv: &TemplateInventory, base: PlanDefaults) -> PlanDefaults {
    seed_defaults(inv, base).defaults
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::classify_strings;
    use crate::values::LayerValues;

    fn inventory(names: &[&str]) -> TemplateInventory {
        let strings: Vec<(u64, String)> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (i as u64 * 100, (*n).to_string()))
            .collect();
        let mut inv = classify_strings(&strings);
        inv.file_name = "18x24 TEST.layout".into();
        inv
    }

    fn layer(name: &str, display: bool, color: [u8; 3], w: u16) -> LayerValues {
        LayerValues {
            name: name.into(),
            offset: 0,
            id: -1,
            display,
            locked: false,
            flags: u8::from(display),
            class_byte: 0,
            color,
            line_weight: w,
            secondary_color: [0, 0, 0, 255],
        }
    }

    #[test]
    fn thickness_hints() {
        assert_eq!(thickness_hint("Siding-6"), Some(6.0));
        assert_eq!(thickness_hint("Frame-3 1/2"), Some(3.5));
        assert_eq!(thickness_hint("Footing-16_2"), Some(16.0));
        assert_eq!(thickness_hint("Siding-6, Board & Batten"), Some(6.0));
        assert_eq!(thickness_hint("8\" CMU (block) Stem Wall"), Some(8.0));
        assert_eq!(thickness_hint("Room Divider"), None);
        assert_eq!(thickness_hint("ICF-Stucco"), None);
    }

    #[test]
    fn adds_missing_wall_types_and_keeps_existing() {
        let inv = inventory(&[
            "Siding-6",
            "Interior-4",
            "Frame-3 1/2",
            "Room Divider",
            "Brick-4",
        ]);
        let base = PlanDefaults::default();
        // Siding-6 and Interior-4 are already in the default wall type list.
        assert!(base.wall_type("Siding-6").is_some() && base.wall_type("Interior-4").is_some());
        let before = base.wall_types.clone();
        let seed = seed_defaults(&inv, base);
        assert_eq!(
            seed.added_wall_types,
            vec!["Frame-3 1/2", "Room Divider", "Brick-4"]
        );
        // Existing types are untouched and come first.
        assert_eq!(&seed.defaults.wall_types[..before.len()], &before[..]);
        assert_eq!(seed.defaults.wall_types.len(), before.len() + 3);
        // Name-number guess: 3.5 + 1.0, flagged approximate.
        let frame = seed.defaults.wall_type("Frame-3 1/2").unwrap();
        assert_eq!(frame.thickness(), 4.5);
        assert_eq!(frame.kind, WallKind::Interior);
        assert!(seed
            .approximate_wall_types
            .contains(&"Frame-3 1/2".to_string()));
        assert_eq!(seed.defaults.wall_type("Brick-4").unwrap().thickness(), 5.0);
        // No number in the name: fallback thickness.
        assert_eq!(
            seed.defaults.wall_type("Room Divider").unwrap().thickness(),
            4.5
        );
        // Seeding twice adds nothing more.
        let again = seed_defaults(&inv, seed.defaults.clone());
        assert!(again.added_wall_types.is_empty());
        assert_eq!(again.defaults.wall_types, seed.defaults.wall_types);
    }

    #[test]
    fn approximate_type_rules() {
        let t = approximate_type("Siding-6");
        assert_eq!(t.thickness(), 7.0);
        assert!(t.layers[0].is_main);
        assert_eq!(t.layers[0].material, "approximate");
        assert_eq!(t.kind, WallKind::Exterior);
        assert_eq!(approximate_type("Interior-4").kind, WallKind::Interior);
        assert_eq!(approximate_type("Room Divider").thickness(), 4.5);
    }

    #[test]
    fn decoded_stack_wins_over_name_guess() {
        let mut inv = inventory(&["Zzz-9"]);
        // `Zzz-9` is not a wall family; add the entry directly.
        inv.wall_types.push(crate::classify::Entry {
            name: "Siding-6, Test".into(),
            count: 1,
            first_offset: 0,
        });
        inv.wall_stacks.push(WallStack {
            name: "Siding-6, Test".into(),
            offset: 0,
            thicknesses: vec![0.5, 5.5, 0.625],
            total: 6.625,
            float_bytes: 8,
        });
        let seed = seed_defaults(&inv, PlanDefaults::default());
        let t = seed.defaults.wall_type("Siding-6, Test").unwrap();
        assert_eq!(t.layers.len(), 3);
        assert_eq!(t.main_layer().unwrap().thickness, 5.5);
        assert!(!seed
            .approximate_wall_types
            .contains(&"Siding-6, Test".to_string()));
    }

    #[test]
    fn layers_registered_without_duplicates() {
        let mut inv = inventory(&[
            "Walls, Normal",
            "Walls,  Normal",
            "Dimensions, Plan",
            "Cabinets, Base",
        ]);
        inv.layer_set_data.push(LayerSetData {
            name: "Working Layer Set".into(),
            named: true,
            offset: 0,
            layers: vec![
                layer("Dimensions, Plan", false, [0, 0, 128], 18),
                layer("Walls, Normal", true, [9, 9, 9], 50),
            ],
        });
        let base = PlanDefaults::default();
        let before = base.layers.layers.len();
        let kept = base.layers.get("Walls, Normal").cloned().unwrap();
        let seed = seed_defaults(&inv, base);
        // `Walls,  Normal` collapses onto the existing layer; the existing
        // layer keeps its own colour.
        assert_eq!(seed.defaults.layers.get("Walls, Normal"), Some(&kept));
        assert!(seed.defaults.layers.get("Walls,  Normal").is_none());
        let dim = seed.defaults.layers.get("Dimensions, Plan").unwrap();
        assert_eq!(dim.color, [0, 0, 128]);
        assert!(dim.display, "new layers start visible");
        assert!(seed.added_layers.contains(&"Dimensions, Plan".to_string()));
        assert!(seed.defaults.layers.layers.len() > before);
        // Cabinets, Base already exists in the default floor-plan set.
        assert!(!seed.added_layers.contains(&"Cabinets, Base".to_string()));
    }

    const DANIEL_LAYER_SETS: [&str; 34] = [
        "Camera View Layer Set",
        "Kitchen & Bath Elevation Layer Set",
        "Elevation View Layer Set",
        "Section View Layer Set",
        "3D Framing Layer Set",
        "Electrical Layer Set",
        "Framing Layer Set",
        "HVAC Layer Set",
        "Plot Plan Layer Set",
        "Roof Plan Layer Set",
        "Kitchen & Bath Layer Set",
        "Steel Framing Layer Set",
        "Presentation Layer Set",
        "Reference Display Layer Set",
        "Working Layer Set",
        "Floor Plan Dimensioned Layer Set",
        "Floor Plan Shell Layer Set",
        "Foundation Layer Set",
        "Framing, Ceiling Layer Set",
        "Framing Porch Layer Set",
        "Framing, Roof Layer Set",
        "Square Footage Layer Set",
        "Window Schedule Layer Set",
        "DWG EXPORT Layer Set",
        "Presentation Elevation View Layer Set",
        "Kitchen and Bath Elevation Layer Set",
        "Framing, Floor Layer Set",
        "Foundation Plan Dimensioned Layer Set",
        "Detail Layer Set",
        "DWG Export Layer Set",
        "Terrain Plan Layer Set",
        "Kitchen and Bath Layer Set",
        "3D Steel Framing Layer Set",
        "FINISH FLOOR Layer Set",
    ];

    /// Daniel-shaped: 34 named sets (one table repeated) plus 13 unnamed
    /// tables.
    fn daniel_shaped_inventory() -> TemplateInventory {
        let mut inv = inventory(&[
            "Walls, Normal",
            "Doors, Interior",
            "Dimensions, Plan",
            "1/4\" Text Style",
        ]);
        let mut data = |name: String, named: bool, i: usize| {
            inv.layer_set_data.push(LayerSetData {
                name,
                named,
                offset: i as u64,
                layers: vec![
                    layer("Walls,  Normal", i.is_multiple_of(2), [1, 2, 3], 50),
                    LayerValues {
                        locked: true,
                        ..layer("Doors, Interior", true, [4, 5, 6], 25)
                    },
                    layer("Dimensions, Plan", !i.is_multiple_of(3), [7, 8, 9], 18),
                ],
            });
        };
        for (i, n) in DANIEL_LAYER_SETS.iter().enumerate() {
            data((*n).to_string(), true, i);
        }
        data("DWG EXPORT Layer Set".into(), true, 40); // repeated name: first wins
        for i in 0..13 {
            data(format!("(unnamed set {i})"), false, 100 + i);
        }
        inv
    }

    #[test]
    fn bridge_produces_34_named_layer_sets() {
        let inv = daniel_shaped_inventory();
        assert_eq!(inv.layer_set_data.len(), 48);
        let sets = seed_layer_sets(&inv);
        assert_eq!(sets.sets.len(), 34);
        assert!(sets.sets.iter().all(|s| !s.name.starts_with('(')));
        assert_eq!(sets.active, "Working Layer Set");
        let working = sets.get("Working Layer Set").unwrap();
        assert_eq!(working.states.len(), 3);
        let walls = working.state("Walls,  Normal").unwrap();
        assert_eq!(walls.color, Some([1, 2, 3]));
        assert_eq!(walls.line_weight, Some(50));
        assert!(working.state("Doors, Interior").unwrap().locked);
        // Display flags come from each set's own table.
        let i = DANIEL_LAYER_SETS
            .iter()
            .position(|n| *n == "Working Layer Set")
            .unwrap();
        assert_eq!(walls.display, i.is_multiple_of(2));
        // No decoded tables -> nothing seeded.
        let none = seed_layer_sets(&inventory(&["Walls, Normal"]));
        assert!(none.sets.is_empty() && none.active.is_empty());
    }

    #[test]
    fn seed_defaults_fills_layer_sets_over_the_base_layers() {
        let inv = daniel_shaped_inventory();
        let seed = seed_defaults(&inv, PlanDefaults::default());
        let ls = &seed.defaults.layer_sets;
        // The lone "Default Set" is replaced by the template's 34.
        assert_eq!(ls.sets.len(), 34);
        assert!(ls.get("Default Set").is_none());
        assert_eq!(ls.active, "Working Layer Set");
        assert_eq!(seed.layer_set_defs.sets.len(), 34);
        // `Walls,  Normal` was folded onto the existing `Walls, Normal`.
        let set = ls.get("Presentation Layer Set").unwrap();
        assert!(set.state("Walls, Normal").is_some());
        assert!(set.state("Walls,  Normal").is_none());
        let eff = ls.effective(&seed.defaults.layers);
        assert_eq!(eff.get("Walls, Normal").unwrap().color, [1, 2, 3]);
        assert!(eff.is_locked("Doors, Interior"));
        // Seeding the result again changes nothing.
        let again = seed_defaults(&inv, seed.defaults.clone());
        assert_eq!(again.defaults.layer_sets, seed.defaults.layer_sets);
        assert_eq!(again.defaults.text_styles, seed.defaults.text_styles);
        assert_eq!(again.defaults.dimension_sets, seed.defaults.dimension_sets);
    }

    #[test]
    fn text_style_seed() {
        let inv = inventory(&[
            "Default Label Style",
            "Room Label Style",
            "1/4\" Text Style",
            "1/2\" Text Style",
            "1/8\" Text Style",
            "Plot Plan Text Style",
            "Default Text Style",
            "Schedule Style",
            "1\" Text Style",
            "Window Label Style",
            "3/16\" Text Style",
            "SF Plan Text Style",
        ]);
        let t = seed_text_styles(&inv);
        assert_eq!(t.styles.len(), 12);
        let h = |n: &str| t.get(n).unwrap().height_in;
        assert_eq!(h("Default Text Style"), 6.0);
        assert_eq!(h("1/4\" Text Style"), 6.0);
        assert_eq!(h("1/2\" Text Style"), 3.0);
        assert_eq!(h("1/8\" Text Style"), 12.0);
        assert_eq!(h("1\" Text Style"), 1.5);
        assert_eq!(h("3/16\" Text Style"), 8.0);
        assert_eq!(h("Plot Plan Text Style"), 6.0);
        assert!(t.get("Room Label Style").unwrap().bold);
        // Merged into the defaults: template names plus the shipped ones.
        let seed = seed_defaults(&inv, PlanDefaults::default());
        let ts = &seed.defaults.text_styles;
        assert!(ts.get("Dimension Text Style").is_some());
        assert!(ts.get("3/16\" Text Style").is_some());
        assert_eq!(ts.styles.len(), 13);
        assert_eq!(text_style_scale("3/16\" Text Style"), Some(0.1875));
        assert_eq!(text_style_scale("Room Label Style"), None);
    }

    #[test]
    fn dimension_set_seed_and_apply() {
        let inv = inventory(&[
            "Plot Plan Dimension Defaults",
            "1/2\" Scale Dimension Defaults",
            "1/4\" Scale Dimension Defaults",
            "Structural Steel Dimension Defaults",
            "NKBA Dimension Defaults",
        ]);
        let sets = seed_dimension_sets(&inv);
        let names: Vec<_> = sets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Plot Plan",
                "1/2\" Scale",
                "1/4\" Scale",
                "Structural Steel",
                "NKBA"
            ]
        );
        assert_eq!(sets[0].auto.set_name, "Plot Plan Dimension Defaults");
        let seed = seed_defaults(&inv, PlanDefaults::default());
        // The 13 shipped sets plus Structural Steel; the active set is kept.
        assert_eq!(seed.defaults.dimension_sets.len(), 14);
        assert_eq!(seed.defaults.active_dimension_set, "1/4\" Scale");
        assert!(seed.defaults.dimension_set("Structural Steel").is_some());

        // apply_seed into other defaults: additive, existing entries kept.
        let mut other = PlanDefaults::default();
        other.window.width = 40.0;
        other.dimension_sets.retain(|s| s.name != "Roof");
        let roof_less = other.dimension_sets.len();
        other.apply_seed(&seed);
        assert_eq!(other.window.width, 40.0);
        // Only the template's own names are added (Structural Steel).
        assert_eq!(other.dimension_sets.len(), roof_less + 1);
        assert!(other.dimension_set("Structural Steel").is_some());
        assert!(other.dimension_set("Roof").is_none());
        assert_eq!(other.active_dimension_set, "1/4\" Scale");
        let before = other.clone();
        apply_seed(&mut other, &seed);
        assert_eq!(other, before);
    }

    #[test]
    fn apply_seed_adds_layer_sets_beside_existing_ones() {
        let inv = daniel_shaped_inventory();
        let seed = seed_defaults(&inv, PlanDefaults::default());
        let mut other = PlanDefaults::default();
        other.layer_sets.copy_set("Default Set", "Mine");
        other.layer_sets.set_active("Mine");
        other.apply_seed(&seed);
        // 2 existing + 34 template sets; active stays on the user's set.
        assert_eq!(other.layer_sets.sets.len(), 36);
        assert_eq!(other.layer_sets.active, "Mine");
        assert_eq!(other.wall_types, seed.defaults.wall_types);
    }

    /// Needs Daniel's template on disk: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn real_template_yields_at_least_30_layer_sets() {
        let Some(dir) = crate::templates_dir() else {
            return;
        };
        let path = dir.join(crate::DEFAULT_PLAN_TEMPLATE);
        if !path.exists() {
            return;
        }
        let inv = crate::build_inventory(&path).unwrap();
        let sets = seed_layer_sets(&inv);
        assert!(sets.sets.len() >= 30, "only {} sets", sets.sets.len());
        assert!(sets.sets.iter().all(|s| !s.states.is_empty()));
        assert!(!seed_text_styles(&inv).styles.is_empty());
    }

    #[test]
    fn vocabulary_and_layout_seed() {
        let mut inv = inventory(&[
            "Elevation View Layer Set",
            "Elevation View Layer Set",
            "Reference Display Layer Set",
            "1/4\" Text Style",
            "1/4\" Scale Dimension Defaults",
            "Working Plan View",
            "ANSI B (11\" x 17\")",
            "ARCH C (18\" x 24\")",
            "Page Template",
            "Floor Finish - %floor.name%",
        ]);
        inv.layer_set_data.push(LayerSetData {
            name: "Elevation View Layer Set".into(),
            named: true,
            offset: 0,
            layers: vec![
                layer("A, one", true, [0; 3], 1),
                layer("B, two", false, [0; 3], 1),
            ],
        });
        let seed = seed_defaults(&inv, PlanDefaults::default());
        assert_eq!(seed.layer_sets.len(), 2);
        assert_eq!(
            seed.layer_sets[0].visible_layers.as_deref(),
            Some(&["A, one".to_string()][..])
        );
        assert_eq!(seed.layer_sets[1].visible_layers, None);
        assert_eq!(seed.text_styles, vec!["1/4\" Text Style"]);
        assert_eq!(seed.dimension_sets, vec!["1/4\" Scale Dimension Defaults"]);
        assert_eq!(seed.plan_views, vec!["Working Plan View"]);
        // File name "18x24 TEST" selects the 18 x 24 sheet.
        assert_eq!(
            seed.layout.sheet_size.as_deref(),
            Some("ARCH C (18\" x 24\")")
        );
        // ... and picks nothing when no listed size matches the name.
        inv.file_name = "30x42 TEST.layout".into();
        assert_eq!(
            seed_defaults(&inv, PlanDefaults::default())
                .layout
                .sheet_size,
            None
        );
        inv.file_name = "Plain.layout".into();
        assert_eq!(
            seed_defaults(&inv, PlanDefaults::default())
                .layout
                .sheet_size
                .as_deref(),
            Some("ANSI B (11\" x 17\")")
        );
        assert_eq!(seed.layout.pages, vec!["Page Template"]);
        assert_eq!(seed.layout.macros, vec!["Floor Finish - %floor.name%"]);
        let json = seed.to_json().unwrap();
        assert!(json.contains("\"layer_sets\"") && json.contains("\"approximate_wall_types\""));
        let back: TemplateSeed = serde_json::from_str(&json).unwrap();
        assert_eq!(back, seed);
        assert_eq!(
            seed_plan_defaults(&inv, PlanDefaults::default()),
            seed_defaults(&inv, PlanDefaults::default()).defaults
        );
    }
}
