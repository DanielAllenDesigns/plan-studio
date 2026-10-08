//! Sorts scanned strings into the template categories of
//! `docs/chief-template-format.md` section 4 using suffix and keyword rules.

use crate::scan::{TemplateKind, TemplateScan};
use crate::values::{LayerSetData, WallStack};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One distinct name with how often it occurred and where it first did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub count: u32,
    pub first_offset: u64,
}

/// The categories a string can land in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    LayerSet,
    Layer,
    TextStyle,
    DimensionDefaults,
    RichTextDefaults,
    WallType,
    PlanView,
    Camera,
    Schedule,
    SheetSize,
    LayoutPage,
    TitleBlockMacro,
    Material,
    RoomType,
    Misc,
}

/// Everything found in one template, by category.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TemplateInventory {
    pub file_name: String,
    pub kind: Option<TemplateKind>,
    /// Strings seen by the walker (before classification), with duplicates.
    pub total_strings: usize,
    pub distinct_strings: usize,
    pub thumbnail_bytes: usize,
    pub layer_sets: Vec<Entry>,
    pub layers: Vec<Entry>,
    pub text_styles: Vec<Entry>,
    pub dimension_defaults: Vec<Entry>,
    pub rich_text_defaults: Vec<Entry>,
    pub wall_types: Vec<Entry>,
    pub plan_views: Vec<Entry>,
    pub cameras: Vec<Entry>,
    pub schedules: Vec<Entry>,
    pub sheet_sizes: Vec<Entry>,
    pub layout_pages: Vec<Entry>,
    pub title_block_macros: Vec<Entry>,
    pub materials: Vec<Entry>,
    pub room_types: Vec<Entry>,
    pub misc: Vec<Entry>,
    /// Resource paths from the tail table.
    pub resources: Vec<String>,
    /// Phase B: per-layer-set layer records, when they decoded.
    pub layer_set_data: Vec<LayerSetData>,
    /// Phase B: wall layer stacks, when any decoded.
    pub wall_stacks: Vec<WallStack>,
    /// Number of strings replaced by redaction (see [`crate::redact`]).
    pub redacted: usize,
}

impl TemplateInventory {
    /// The entries of one category.
    pub fn entries(&self, c: Category) -> &[Entry] {
        match c {
            Category::LayerSet => &self.layer_sets,
            Category::Layer => &self.layers,
            Category::TextStyle => &self.text_styles,
            Category::DimensionDefaults => &self.dimension_defaults,
            Category::RichTextDefaults => &self.rich_text_defaults,
            Category::WallType => &self.wall_types,
            Category::PlanView => &self.plan_views,
            Category::Camera => &self.cameras,
            Category::Schedule => &self.schedules,
            Category::SheetSize => &self.sheet_sizes,
            Category::LayoutPage => &self.layout_pages,
            Category::TitleBlockMacro => &self.title_block_macros,
            Category::Material => &self.materials,
            Category::RoomType => &self.room_types,
            Category::Misc => &self.misc,
        }
    }

    pub(crate) fn entries_mut(&mut self, c: Category) -> &mut Vec<Entry> {
        match c {
            Category::LayerSet => &mut self.layer_sets,
            Category::Layer => &mut self.layers,
            Category::TextStyle => &mut self.text_styles,
            Category::DimensionDefaults => &mut self.dimension_defaults,
            Category::RichTextDefaults => &mut self.rich_text_defaults,
            Category::WallType => &mut self.wall_types,
            Category::PlanView => &mut self.plan_views,
            Category::Camera => &mut self.cameras,
            Category::Schedule => &mut self.schedules,
            Category::SheetSize => &mut self.sheet_sizes,
            Category::LayoutPage => &mut self.layout_pages,
            Category::TitleBlockMacro => &mut self.title_block_macros,
            Category::Material => &mut self.materials,
            Category::RoomType => &mut self.room_types,
            Category::Misc => &mut self.misc,
        }
    }

    /// All categories, in report order.
    pub const CATEGORIES: [Category; 15] = [
        Category::LayerSet,
        Category::Layer,
        Category::TextStyle,
        Category::DimensionDefaults,
        Category::RichTextDefaults,
        Category::WallType,
        Category::PlanView,
        Category::Camera,
        Category::Schedule,
        Category::SheetSize,
        Category::LayoutPage,
        Category::TitleBlockMacro,
        Category::Material,
        Category::RoomType,
        Category::Misc,
    ];

    /// Whether `name` is listed in `category` (exact, after trimming).
    pub fn contains(&self, category: Category, name: &str) -> bool {
        let name = name.trim();
        self.entries(category).iter().any(|e| e.name == name)
    }

    /// Adds `name` to a category (count 1 at offset 0 when new). Used to merge
    /// structurally detected layers.
    pub(crate) fn add_if_missing(&mut self, category: Category, name: &str, offset: u64) {
        let name = name.trim();
        if !self.contains(category, name) {
            self.entries_mut(category).push(Entry {
                name: name.to_string(),
                count: 1,
                first_offset: offset,
            });
        }
    }
}

// ----- rules -----

const ROOM_TYPES: [&str; 25] = [
    "Attic",
    "Bath",
    "Master Bath",
    "Bedroom",
    "Master Bedroom",
    "Bonus Room",
    "Closet",
    "Walk-In Closet",
    "Dining",
    "Dining Room",
    "Dressing Room",
    "Family Room",
    "Game Room",
    "Garage",
    "Great Room",
    "Hall",
    "Kitchen",
    "Laundry",
    "Laundry Room",
    "Living",
    "Living Room",
    "Office",
    "Porch",
    "Storage Room",
    "Utility Room",
];

/// First word(s) before the comma in layer names like `Walls, Normal`.
const LAYER_FAMILIES: [&str; 44] = [
    "Walls",
    "Dimensions",
    "Cabinets",
    "Cameras",
    "Terrain",
    "Framing",
    "Footings",
    "Stairs & Ramps",
    "Roofs",
    "Text",
    "Rooms",
    "CAD",
    "Electrical",
    "Fixtures",
    "Furniture",
    "Doors",
    "Windows",
    "Plants",
    "Sprinklers",
    "Millwork",
    "Hardware",
    "Casings",
    "Polylines",
    "Polylines 3D",
    "Revision Clouds",
    "Light Sources",
    "Architectural Blocks",
    "CAD Blocks",
    "Geometric Shapes",
    "Picture/PDF Boxes",
    "3D Solids",
    "Wall Niches",
    "Audio/Video",
    "Plumbing",
    "Mechanical",
    "Old Dimensions",
    "Pool",
    "Patterns",
    "Fireplaces",
    "Slabs",
    "Layout Box",
    "Deck",
    "Lines",
    "Notes",
];

/// Layer names that have no comma form.
const SINGLE_LAYERS: [&str; 36] = [
    "Rooms",
    "Slabs",
    "Plants",
    "Fences",
    "Default",
    "Windows",
    "Cameras",
    "Footings",
    "Foundation",
    "Electrical",
    "Doors",
    "Images",
    "Hardware",
    "Millwork",
    "Schedules",
    "Fireplaces",
    "Sprinklers",
    "Light Sources",
    "Architectural Blocks",
    "Geometric Shapes",
    "Corner Boards",
    "Opening Indicators",
    "Ceiling Surfaces",
    "Floor Surfaces",
    "Material Region",
    "Walkthrough Paths",
    "Old Framing",
    "Old Roof",
    "Old 3D Roof Framing",
    "Old 3D Floor Framing",
    "Old 3D Wall Framing",
    "Ceiling Break Lines",
    "Opening Header Lines",
    "Brick Ledge Lines",
    "Terrain Labels",
    "Terrain Perimeter",
];

/// Wall-type families written as `Family-<thickness>`.
const WALL_FAMILIES: [&str; 17] = [
    "siding",
    "stucco",
    "brick",
    "interior",
    "fire",
    "frame",
    "footing",
    "demolition",
    "wall",
    "stone",
    "icf",
    "foundation",
    "steel interior",
    "steel brick",
    "existing",
    "fir",
    "deck railing/fence",
];

/// Wall types that carry no `-<thickness>` suffix.
const WALL_EXACT: [&str; 16] = [
    "room divider",
    "interior railing",
    "railing/fence",
    "deck railing/fence",
    "glass wall",
    "glass block",
    "glass shower",
    "soundbarrier",
    "sip",
    "cbs",
    "icf",
    "fire rated drywall",
    "log siding",
    "concrete block 1",
    "foundation wall",
    "foundation stone",
];

/// `Family-4`, `Family-3 1/2`, `Family-6, Variant`, `Family-16_2`, or
/// `Existing Family-4`: returns true when the family before the dash is a
/// known wall family and a digit follows.
fn is_dashed_wall_type(name: &str) -> bool {
    let Some(dash) = name.find('-') else {
        return false;
    };
    let family = name[..dash].trim().to_ascii_lowercase();
    let after = &name[dash + 1..];
    if !after.starts_with(|c: char| c.is_ascii_digit()) {
        // ICF-Siding, ICF-Stucco, ICF-Stone front
        return family == "icf";
    }
    let family = family.strip_prefix("existing ").unwrap_or(&family);
    WALL_FAMILIES.contains(&family)
}

/// Wall type names of the stem-wall style: `8" CMU (block) Stem Wall`,
/// `Foundation 8" Concrete Stem Wall 4" Brick Ledge`, `10" Superior Wall`...
fn is_stem_wall_type(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let starts_dim = name.starts_with(|c: char| c.is_ascii_digit()) && name.contains('"');
    let keywords = [
        "cmu",
        "concrete",
        "superior wall",
        "stem wall",
        "crawl wall",
    ];
    let has_kw = keywords.iter().any(|k| lower.contains(k));
    (starts_dim && has_kw && lower.contains("wall"))
        || (lower.starts_with("foundation ") && name.contains('"') && has_kw)
        || lower.contains("seperation wall")
        || lower.contains("separation wall")
}

/// Whether a trimmed name is a wall type.
pub fn is_wall_type_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let base = lower.split([',', '_']).next().unwrap_or("").trim();
    is_dashed_wall_type(name)
        || WALL_EXACT.contains(&base)
        || WALL_EXACT.contains(&lower.as_str())
        || is_stem_wall_type(name)
}

fn is_layer_name(name: &str) -> bool {
    if SINGLE_LAYERS.contains(&name) {
        return true;
    }
    let Some(comma) = name.find(',') else {
        return false;
    };
    comma <= 24 && LAYER_FAMILIES.contains(&name[..comma].trim())
}

fn is_macro(name: &str) -> bool {
    if name.starts_with('=') && name.len() > 1 {
        return true;
    }
    match name.find('%') {
        Some(first) => name[first + 1..].contains('%') && name[first + 1..].find('%') != Some(0),
        None => false,
    }
}

fn is_sheet_size(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower == "us letter" || lower == "letter" || lower == "tabloid" {
        return true;
    }
    let has_digit = name.chars().any(|c| c.is_ascii_digit());
    let paper_word = [
        "arch ", "ansi ", "iso ", "a0", "a1", "a2", "a3", "a4", "us ",
    ]
    .iter()
    .any(|p| lower.starts_with(p));
    has_digit && name.ends_with(')') && name.contains('(') && (name.contains(" x ") || paper_word)
}

fn is_material_category(name: &str) -> bool {
    // "2100 - Footings and foundation"
    let digits = name.chars().take_while(char::is_ascii_digit).count();
    digits == 4 && name[digits..].starts_with(" - ") && name.len() > digits + 3
}

fn is_plan_view(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("plan view") || lower == "square footage view"
}

fn is_camera(name: &str) -> bool {
    name.strip_prefix("Camera ")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

/// Cheap "is this prose-like" test that keeps `misc` free of binary noise.
fn looks_like_text(name: &str) -> bool {
    let len = name.chars().count();
    let letters = name.chars().filter(|c| c.is_ascii_alphabetic()).count();
    letters >= 3
        && letters * 10 >= len * 6
        && name.chars().any(|c| "aeiouAEIOU".contains(c))
        && name.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '"' || c == '(')
}

/// Classifies one trimmed name. `None` means "noise".
pub fn classify_name(name: &str) -> Option<Category> {
    if name.chars().count() < 2 {
        return None;
    }
    if is_macro(name) {
        return Some(Category::TitleBlockMacro);
    }
    if name.ends_with("Layer Set") {
        return Some(Category::LayerSet);
    }
    if name.ends_with("Rich Text Defaults") {
        return Some(Category::RichTextDefaults);
    }
    if name.ends_with("Dimension Defaults") {
        return Some(Category::DimensionDefaults);
    }
    if name.ends_with("Text Style") || name.ends_with("Label Style") || name == "Schedule Style" {
        return Some(Category::TextStyle);
    }
    if is_plan_view(name) {
        return Some(Category::PlanView);
    }
    if is_camera(name) {
        return Some(Category::Camera);
    }
    if is_material_category(name) {
        return Some(Category::Material);
    }
    if ROOM_TYPES.contains(&name) {
        return Some(Category::RoomType);
    }
    if is_sheet_size(name) {
        return Some(Category::SheetSize);
    }
    if name == "Page Template" || name.starts_with("Layout ") || name.ends_with(" Page") {
        return Some(Category::LayoutPage);
    }
    if name.ends_with(" Schedule") || name.ends_with(" Note") {
        return Some(Category::Schedule);
    }
    if is_wall_type_name(name) {
        return Some(Category::WallType);
    }
    if is_layer_name(name) {
        return Some(Category::Layer);
    }
    looks_like_text(name).then_some(Category::Misc)
}

/// Classifies `(offset, string)` pairs. Names are trimmed (templates contain
/// names like `" Working Layer Set"`); duplicates collapse into one entry with
/// an occurrence count and the first offset.
pub fn classify_strings(strings: &[(u64, String)]) -> TemplateInventory {
    let mut inv = TemplateInventory {
        total_strings: strings.len(),
        ..TemplateInventory::default()
    };
    let mut index: HashMap<(Category, String), usize> = HashMap::new();
    let mut distinct = std::collections::HashSet::new();
    for (offset, raw) in strings {
        let name = raw.trim();
        distinct.insert(name);
        let Some(cat) = classify_name(name) else {
            continue;
        };
        // Sheet names are stored with stray double spaces ("ANSI B  (11" x
        // 17")"); normalise them so they match the names Chief displays.
        let collapsed;
        let name = if cat == Category::SheetSize {
            collapsed = name.split_whitespace().collect::<Vec<_>>().join(" ");
            collapsed.as_str()
        } else {
            name
        };
        let list = inv.entries_mut(cat);
        match index.get(&(cat, name.to_string())) {
            Some(&i) => list[i].count += 1,
            None => {
                index.insert((cat, name.to_string()), list.len());
                list.push(Entry {
                    name: name.to_string(),
                    count: 1,
                    first_offset: *offset,
                });
            }
        }
    }
    inv.distinct_strings = distinct.len();
    inv
}

/// Classifies a whole scan and copies kind, resources and thumbnail size.
pub fn classify(scan: &TemplateScan) -> TemplateInventory {
    let mut inv = classify_strings(&scan.strings);
    inv.kind = Some(scan.kind);
    inv.resources = scan.resources.clone();
    inv.thumbnail_bytes = scan.thumbnail_png.as_ref().map_or(0, Vec::len);
    inv
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(s: &str) -> Option<Category> {
        classify_name(s.trim())
    }

    #[test]
    fn fixed_string_list() {
        use Category::*;
        let cases: &[(&str, Option<Category>)] = &[
            ("Elevation View Layer Set", Some(LayerSet)),
            (" Working Layer Set", Some(LayerSet)),
            ("DWG EXPORT Layer Set ", Some(LayerSet)),
            ("Framing, Ceiling Layer Set", Some(LayerSet)),
            ("1/4\" Text Style", Some(TextStyle)),
            ("Room Label Style", Some(TextStyle)),
            ("Schedule Style", Some(TextStyle)),
            ("1/4\" Scale Dimension Defaults", Some(DimensionDefaults)),
            ("NKBA Dimension Defaults", Some(DimensionDefaults)),
            ("1/4\" Scale Rich Text Defaults", Some(RichTextDefaults)),
            ("Siding-6", Some(WallType)),
            ("Siding-6, Board & Batten Grey", Some(WallType)),
            ("Stucco-6", Some(WallType)),
            ("Brick-4", Some(WallType)),
            ("Existing Brick-4", Some(WallType)),
            ("Interior-4", Some(WallType)),
            ("Interior-6", Some(WallType)),
            ("Frame-3 1/2", Some(WallType)),
            ("Footing-16_2", Some(WallType)),
            ("ICF-Stucco", Some(WallType)),
            ("Foundation 8\" CMU (block) Stem Wall", Some(WallType)),
            ("8\" CMU (block) Stem Wall, Stucco", Some(WallType)),
            ("Room Divider", Some(WallType)),
            ("Walls, Default Fill Color", Some(Layer)),
            ("Walls,  Normal", Some(Layer)),
            ("Dimensions, Plan", Some(Layer)),
            ("Cabinets, Base", Some(Layer)),
            ("Cameras, Elevations", Some(Layer)),
            ("Terrain, Elevation Data", Some(Layer)),
            ("Rooms", Some(Layer)),
            ("Working Plan View", Some(PlanView)),
            ("Floor Plan View Dimensioned", Some(PlanView)),
            ("WINDWOS/ DOORS PLAN VIEW", Some(PlanView)),
            ("Square Footage View", Some(PlanView)),
            ("Camera 1", Some(Camera)),
            ("Door Schedule", Some(Schedule)),
            ("Bath Note", Some(Schedule)),
            ("ARCH C (18\" x 24\")", Some(SheetSize)),
            ("ANSI B (11\" x 17\")", Some(SheetSize)),
            ("US Letter", Some(SheetSize)),
            ("Page Template", Some(LayoutPage)),
            ("Layout Box Labels", Some(LayoutPage)),
            ("Floor Finish - %floor.name%", Some(TitleBlockMacro)),
            ("%room.name%", Some(TitleBlockMacro)),
            ("=material_data.quantity", Some(TitleBlockMacro)),
            ("2100 - Footings and foundation", Some(Material)),
            ("Master Bedroom", Some(RoomType)),
            ("Walk-In Closet", Some(RoomType)),
            ("Avenir", Some(Misc)),
            ("d", None),
            ("%", None),
            ("fff?", None),
            ("", None),
        ];
        for (s, want) in cases {
            assert_eq!(cat(s), *want, "classifying {s:?}");
        }
    }

    #[test]
    fn dedup_counts_and_first_offsets() {
        let strings: Vec<(u64, String)> = vec![
            (100, "Siding-6".into()),
            (200, "Stucco-6".into()),
            (300, "Siding-6".into()),
            (400, " Working Layer Set".into()),
            (500, "Working Layer Set".into()),
            (600, "Door Schedule".into()),
        ];
        let inv = classify_strings(&strings);
        assert_eq!(inv.total_strings, 6);
        assert_eq!(inv.distinct_strings, 4);
        assert_eq!(inv.wall_types.len(), 2);
        assert_eq!(
            inv.wall_types[0],
            Entry {
                name: "Siding-6".into(),
                count: 2,
                first_offset: 100
            }
        );
        // Leading space trimmed, both spellings merged.
        assert_eq!(inv.layer_sets.len(), 1);
        assert_eq!(inv.layer_sets[0].name, "Working Layer Set");
        assert_eq!(inv.layer_sets[0].count, 2);
        assert!(inv.contains(Category::Schedule, "Door Schedule"));
        assert!(!inv.contains(Category::Schedule, "Window Schedule"));
    }

    #[test]
    fn wall_type_rules_do_not_swallow_layers() {
        for layer in [
            "Walls, Normal",
            "Footings, Deck Post",
            "Framing, Posts",
            "Foundation",
        ] {
            assert!(!is_wall_type_name(layer), "{layer}");
        }
        for wt in [
            "Fire-4",
            "Wall-4",
            "Steel Interior-4, 5/8",
            "Demolition-6, Copy",
        ] {
            assert!(is_wall_type_name(wt), "{wt}");
        }
    }
}
