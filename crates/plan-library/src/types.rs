//! The Library Browser's type filter: the twelve kinds of object Chief's
//! browser narrows by (cabinets, doors, windows, fixtures, furniture, plants,
//! materials, backdrops, moldings, images, electrical, hardware).
//!
//! An item's type is worked out from its [`ItemKind`], its category path, its
//! name and its tags ([`classify`]), so catalogs need no extra field. Items
//! that fit none of the twelve (a plain CAD block, say) have no type and are
//! hidden by an active type filter.

use crate::catalog::{CatalogItem, ItemKind};
use serde::{Deserialize, Serialize};

/// One entry of the browser's type filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibType {
    Cabinets,
    Doors,
    Windows,
    Fixtures,
    Furniture,
    Plants,
    Materials,
    Backdrops,
    Moldings,
    Images,
    Electrical,
    Hardware,
}

impl LibType {
    /// Every type, in the order the filter lists them.
    pub const ALL: [LibType; 12] = [
        LibType::Cabinets,
        LibType::Doors,
        LibType::Windows,
        LibType::Fixtures,
        LibType::Furniture,
        LibType::Plants,
        LibType::Materials,
        LibType::Backdrops,
        LibType::Moldings,
        LibType::Images,
        LibType::Electrical,
        LibType::Hardware,
    ];

    /// Display name (plural, as the filter chips read).
    pub fn label(self) -> &'static str {
        match self {
            LibType::Cabinets => "Cabinets",
            LibType::Doors => "Doors",
            LibType::Windows => "Windows",
            LibType::Fixtures => "Fixtures",
            LibType::Furniture => "Furniture",
            LibType::Plants => "Plants",
            LibType::Materials => "Materials",
            LibType::Backdrops => "Backdrops",
            LibType::Moldings => "Moldings",
            LibType::Images => "Images",
            LibType::Electrical => "Electrical",
            LibType::Hardware => "Hardware",
        }
    }

    /// The type named by `text` (case-insensitive, singular or plural).
    pub fn parse(text: &str) -> Option<LibType> {
        let t = text.trim().to_lowercase();
        let t = t.strip_suffix('s').unwrap_or(&t);
        LibType::ALL
            .into_iter()
            .find(|k| k.label().to_lowercase().trim_end_matches('s') == t)
    }
}

/// A word without its plural `s` (`doors` and `door` agree; `glass` stays).
fn singular(w: &str) -> &str {
    match w.strip_suffix('s') {
        Some(rest) if rest.len() > 2 && !rest.ends_with('s') => rest,
        _ => w,
    }
}

/// The lower-case words of `text`, singular.
struct Words {
    joined: String,
    tokens: Vec<String>,
}

impl Words {
    fn new(text: &str) -> Words {
        let lower = text.to_lowercase();
        let tokens = lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .map(|t| singular(t).to_string())
            .collect();
        Words {
            joined: lower,
            tokens,
        }
    }

    /// True when any of `words` is one of the words (or, for a phrase with a
    /// space, occurs in the text).
    fn has(&self, words: &[&str]) -> bool {
        words.iter().any(|w| {
            if w.contains(' ') {
                self.joined.contains(w)
            } else {
                let w = singular(w);
                self.tokens.iter().any(|t| t == w)
            }
        })
    }
}

/// The browser type of `item`, or `None` when it fits none of the twelve.
///
/// The item's [`ItemKind`] decides first (a swatch is a material, a picture
/// an image, a saved cabinet a cabinet, a device electrical); then words in
/// the category path, the name and the tags ([`classify_text`]), with the
/// narrower types (hardware, moldings) tried before the broad ones (doors,
/// furniture).
pub fn classify(item: &CatalogItem) -> Option<LibType> {
    match item.kind {
        ItemKind::Material => return Some(LibType::Materials),
        ItemKind::Image => return Some(LibType::Images),
        ItemKind::Cabinet => return Some(LibType::Cabinets),
        ItemKind::Device => return Some(LibType::Electrical),
        _ => {}
    }
    let mut text = item.category.join(" ");
    text.push(' ');
    text.push_str(&item.name);
    for t in &item.tags {
        text.push(' ');
        text.push_str(t);
    }
    classify_text(&text)
}

/// The browser type that the words of `text` (a category path, a name and
/// keywords joined) describe, for objects that are not [`CatalogItem`]s yet
/// (the rows of a Chief catalog).
pub fn classify_text(text: &str) -> Option<LibType> {
    let w = Words::new(text);
    if w.has(&[
        "hardware",
        "knob",
        "hinge",
        "drawer pull",
        "handle",
        "latch",
        "deadbolt",
        "lockset",
    ]) {
        return Some(LibType::Hardware);
    }
    if w.has(&[
        "molding",
        "moulding",
        "baseboard",
        "crown",
        "casing",
        "wainscot",
    ]) {
        return Some(LibType::Moldings);
    }
    if w.has(&["backdrop", "skyline"]) {
        return Some(LibType::Backdrops);
    }
    if w.has(&["cabinet", "vanity", "casework"]) {
        return Some(LibType::Cabinets);
    }
    if w.has(&["window", "skylight"]) && !w.has(&["treatment"]) {
        return Some(LibType::Windows);
    }
    if w.has(&["door"]) {
        return Some(LibType::Doors);
    }
    if w.has(&[
        "electrical",
        "outlet",
        "receptacle",
        "switch",
        "lighting",
        "light",
        "chandelier",
        "sconce",
        "ceiling fan",
        "smoke",
        "alarm",
        "thermostat",
        "panel",
    ]) {
        return Some(LibType::Electrical);
    }
    if w.has(&[
        "plant",
        "tree",
        "shrub",
        "flower",
        "bush",
        "hedge",
        "palm",
        "landscape",
        "grass",
        "garden",
    ]) {
        return Some(LibType::Plants);
    }
    if w.has(&[
        "plumbing",
        "fixture",
        "toilet",
        "sink",
        "lavatory",
        "bathtub",
        "tub",
        "shower",
        "faucet",
        "appliance",
        "refrigerator",
        "fridge",
        "range",
        "cooktop",
        "oven",
        "dishwasher",
        "washer",
        "dryer",
        "water heater",
        "furnace",
    ]) {
        return Some(LibType::Fixtures);
    }
    if w.has(&[
        "furniture",
        "chair",
        "table",
        "sofa",
        "couch",
        "bed",
        "desk",
        "dresser",
        "shelf",
        "bookcase",
        "ottoman",
        "bench",
        "stool",
        "mattress",
        "lamp",
    ]) {
        return Some(LibType::Furniture);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Placement;
    use crate::symbol::Symbol2d;

    fn item(name: &str, cat: &[&str]) -> CatalogItem {
        CatalogItem::new("x", name, Placement::FreeStanding, Symbol2d::default()).with_category(cat)
    }

    #[test]
    fn kinds_decide_first() {
        let mut i = item("Anything", &["Door"]);
        i.kind = ItemKind::Material;
        assert_eq!(classify(&i), Some(LibType::Materials));
        i.kind = ItemKind::Image;
        assert_eq!(classify(&i), Some(LibType::Images));
        i.kind = ItemKind::Cabinet;
        assert_eq!(classify(&i), Some(LibType::Cabinets));
        i.kind = ItemKind::Device;
        assert_eq!(classify(&i), Some(LibType::Electrical));
    }

    #[test]
    fn words_in_the_path_name_and_tags_pick_the_type() {
        let cases = [
            (
                "Shaker Door",
                &["Architectural", "Doors"][..],
                LibType::Doors,
            ),
            ("Double Hung", &["Windows"][..], LibType::Windows),
            ("Toilet", &["Plumbing"][..], LibType::Fixtures),
            ("Sofa", &["Furniture", "Living"][..], LibType::Furniture),
            ("Red Oak", &["Plants", "Trees"][..], LibType::Plants),
            ("Crown 4", &["Trim", "Crown Molding"][..], LibType::Moldings),
            ("Knob", &["Door Hardware"][..], LibType::Hardware),
            (
                "Duplex",
                &["Electrical", "Outlets"][..],
                LibType::Electrical,
            ),
            (
                "Base 24",
                &["Kitchen", "Base Cabinets"][..],
                LibType::Cabinets,
            ),
            ("Mountains", &["Backdrops"][..], LibType::Backdrops),
        ];
        for (name, cat, want) in cases {
            assert_eq!(classify(&item(name, cat)), Some(want), "{name}");
        }
        // Hardware wins over the door it belongs to; tags count.
        let mut t = item("Thing", &["Misc"]);
        t.tags = vec!["cabinet".into()];
        assert_eq!(classify(&t), Some(LibType::Cabinets));
        assert_eq!(classify(&item("Blob", &["Misc"])), None);
        assert_eq!(
            classify_text("Kitchen Sinks undermount"),
            Some(LibType::Fixtures)
        );
        assert_eq!(classify_text("nothing here"), None);
        // Whole words only: an outdoor mat is no door, a bedroom no bed.
        assert_eq!(classify(&item("Mat", &["Outdoor"])), None);
        assert_eq!(
            classify(&item("Panel Door", &["Doors"])),
            Some(LibType::Doors)
        );
    }

    #[test]
    fn labels_round_trip_through_parse() {
        for t in LibType::ALL {
            assert_eq!(LibType::parse(t.label()), Some(t));
        }
        assert_eq!(LibType::parse("door"), Some(LibType::Doors));
        assert_eq!(LibType::parse("nonsense"), None);
    }
}
