//! Placement rules and default layers of library items.
//!
//! Every item has a [`Placement`]. From it, and from optional per-item
//! overrides, a placement tool learns what the item snaps to when it is
//! placed ([`placement_rules`]), which layer it goes on ([`default_layer`])
//! and, for new items, which placement suits their category
//! ([`placement_for_category`]).

use crate::catalog::{CatalogItem, ItemKind, Placement};

/// What a placed copy of an item snaps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementRules {
    /// Snaps flush to the nearest wall face.
    pub snap_to_wall: bool,
    /// Hangs from the ceiling.
    pub snap_to_ceiling: bool,
    /// Stands on the floor.
    pub snap_to_floor: bool,
    /// Turns to face away from the nearest wall (needs `snap_to_wall`
    /// distance to apply).
    pub auto_rotate: bool,
}

/// The rules of `item`: its [`Placement`] decides what it snaps to, and
/// `CatalogItem::auto_rotate` overrides whether it turns to the wall.
pub fn placement_rules(item: &CatalogItem) -> PlacementRules {
    let (wall, ceiling, floor) = match item.placement {
        Placement::WallMounted => (true, false, false),
        Placement::FreeStanding => (false, false, true),
        Placement::Ceiling => (false, true, false),
        Placement::Countertop => (false, false, false),
    };
    let auto_rotate = item
        .auto_rotate
        .unwrap_or(item.placement == Placement::WallMounted);
    PlacementRules {
        snap_to_wall: wall || auto_rotate,
        snap_to_ceiling: ceiling,
        snap_to_floor: floor,
        auto_rotate,
    }
}

fn mentions(words: &[&str], text: &str) -> bool {
    let t = text.to_lowercase();
    words.iter().any(|w| t.contains(w))
}

/// The placement that suits a category path and item name: lights and fans
/// hang from the ceiling; plumbing fixtures, devices and mirrors sit on the
/// wall; small appliances rest on the counter; the rest stands on the floor.
pub fn placement_for_category(category: &[String], name: &str) -> Placement {
    let text = format!("{} {}", category.join(" "), name);
    if mentions(&["ceiling", "chandelier", "pendant", "smoke", "ceiling fan"], &text)
        || (mentions(&["light", "fan"], &text) && !mentions(&["switch", "floor", "table", "lamp", "sconce", "wall"], &text))
    {
        Placement::Ceiling
    } else if mentions(
        &[
            "toilet", "sink", "lavatory", "tub", "shower", "outlet", "receptacle", "switch",
            "mirror", "sconce", "thermostat", "vent", "register", "plumbing", "wall",
        ],
        &text,
    ) {
        Placement::WallMounted
    } else if mentions(&["countertop", "microwave", "toaster", "coffee", "faucet"], &text) {
        Placement::Countertop
    } else {
        Placement::FreeStanding
    }
}

/// The layer placed copies of `item` go on: its own `layer`, else by kind
/// and category (cabinets on the cabinet layers, electrical on `Electrical`,
/// text on `Text`, everything else on `CAD, Default`).
pub fn default_layer(item: &CatalogItem) -> String {
    if let Some(l) = item.layer.as_ref().filter(|l| !l.trim().is_empty()) {
        return l.clone();
    }
    let text = format!("{} {}", item.category.join(" "), item.name);
    match item.kind {
        ItemKind::Cabinet => {
            if item.placement == Placement::WallMounted || mentions(&["wall cabinet", "upper"], &text)
            {
                "Cabinets, Wall"
            } else {
                "Cabinets, Base"
            }
        }
        ItemKind::Device => "Electrical",
        ItemKind::Text => "Text",
        _ if mentions(&["electrical", "lighting", "receptacle", "switch"], &text) => "Electrical",
        _ => "CAD, Default",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol2d;

    fn item(name: &str, cat: &[&str], placement: Placement) -> CatalogItem {
        CatalogItem::new("x", name, placement, Symbol2d::default()).with_category(cat)
    }

    #[test]
    fn rules_follow_placement_and_the_override() {
        let wall = placement_rules(&item("Toilet", &[], Placement::WallMounted));
        assert!(wall.snap_to_wall && wall.auto_rotate && !wall.snap_to_floor);
        let free = placement_rules(&item("Chair", &[], Placement::FreeStanding));
        assert!(free.snap_to_floor && !free.snap_to_wall && !free.auto_rotate);
        let ceil = placement_rules(&item("Fan", &[], Placement::Ceiling));
        assert!(ceil.snap_to_ceiling && !ceil.snap_to_wall);
        let mut bookcase = item("Bookcase", &[], Placement::FreeStanding);
        bookcase.auto_rotate = Some(true);
        let r = placement_rules(&bookcase);
        assert!(r.snap_to_wall && r.auto_rotate && r.snap_to_floor);
        let mut loose = item("Vanity", &[], Placement::WallMounted);
        loose.auto_rotate = Some(false);
        assert!(!placement_rules(&loose).auto_rotate);
    }

    #[test]
    fn categories_suggest_a_placement() {
        let cat = |c: &[&str], n: &str| {
            placement_for_category(&c.iter().map(|s| s.to_string()).collect::<Vec<_>>(), n)
        };
        assert_eq!(cat(&["Lighting"], "Pendant"), Placement::Ceiling);
        assert_eq!(cat(&["Plumbing"], "Toilet"), Placement::WallMounted);
        assert_eq!(cat(&["Furniture"], "Sofa"), Placement::FreeStanding);
        assert_eq!(cat(&["Appliances"], "Microwave"), Placement::Countertop);
        assert_eq!(cat(&["Lighting"], "Floor Lamp"), Placement::FreeStanding);
        assert_eq!(cat(&["Lighting"], "Wall Sconce"), Placement::WallMounted);
    }

    #[test]
    fn layers_follow_kind_category_or_the_item() {
        let mut it = item("Sofa", &["Furniture"], Placement::FreeStanding);
        assert_eq!(default_layer(&it), "CAD, Default");
        it.kind = ItemKind::Cabinet;
        assert_eq!(default_layer(&it), "Cabinets, Base");
        it.placement = Placement::WallMounted;
        assert_eq!(default_layer(&it), "Cabinets, Wall");
        it.kind = ItemKind::Text;
        assert_eq!(default_layer(&it), "Text");
        it.kind = ItemKind::Symbol;
        it.category = vec!["Electrical".into()];
        assert_eq!(default_layer(&it), "Electrical");
        it.layer = Some("Furniture".into());
        assert_eq!(default_layer(&it), "Furniture");
    }
}
