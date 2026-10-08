//! Checks for the four extended catalogs (Plants, Bath & Kitchen, Lighting &
//! Electrical, Furniture & Exterior) and for the combined library.

use plan_library::{
    all_core_catalogs, catalog_bath_kitchen, catalog_furniture_exterior,
    catalog_lighting_electrical, catalog_plants, Catalog, CatalogItem, Library, Placement,
};
use std::collections::HashSet;

/// Symbols that intentionally draw outside their declared width and depth.
const OVERHANG: &[&str] = &[
    // Six chairs tucked around the table; the item size is the table's.
    "core.exterior.outdoor_dining_72x36",
];

fn new_items() -> Vec<CatalogItem> {
    [
        catalog_plants::catalog(),
        catalog_bath_kitchen::catalog(),
        catalog_lighting_electrical::catalog(),
        catalog_furniture_exterior::catalog(),
    ]
    .into_iter()
    .flat_map(|c| c.items)
    .collect()
}

#[test]
fn each_catalog_meets_its_minimum_size() {
    assert!(catalog_plants::catalog().items.len() >= 20);
    assert!(catalog_bath_kitchen::catalog().items.len() >= 24);
    assert!(catalog_lighting_electrical::catalog().items.len() >= 16);
    assert!(catalog_furniture_exterior::catalog().items.len() >= 24);
    assert_eq!(all_core_catalogs().len(), 5);
}

#[test]
fn ids_are_unique_across_all_catalogs() {
    let mut seen = HashSet::new();
    for cat in all_core_catalogs() {
        for item in &cat.items {
            assert!(seen.insert(item.id.clone()), "duplicate id {}", item.id);
            assert!(item.id.starts_with("core."), "{}", item.id);
        }
    }
    assert_eq!(seen.len(), Library::with_all_core().len());
}

#[test]
fn every_item_is_well_formed() {
    for item in new_items() {
        let id = &item.id;
        assert!(!item.name.is_empty(), "{id}");
        assert!(
            item.category.len() >= 2,
            "{id} category {:?}",
            item.category
        );
        assert!(!item.tags.is_empty(), "{id} has no tags");
        assert!(!item.symbol.is_empty(), "{id} has no symbol");
        assert!(
            item.symbol.strokes.len() <= 40,
            "{id}: {} strokes",
            item.symbol.strokes.len()
        );
        assert!(
            item.width > 0.0 && item.depth > 0.0 && item.height > 0.0,
            "{id}"
        );
        assert!(item.elevation >= 0.0, "{id}");
        if item.placement == Placement::Ceiling {
            assert_eq!(
                item.elevation, 0.0,
                "{id}: ceiling items attach at elevation 0"
            );
        }
    }
}

#[test]
fn symbols_match_declared_footprint_and_anchor() {
    for item in new_items() {
        let id = &item.id;
        let b = item.symbol.bounds().expect("bounds");
        if !OVERHANG.contains(&id.as_str()) {
            assert!(
                (b.width() - item.width).abs() <= 1.0,
                "{id}: symbol width {} vs {}",
                b.width(),
                item.width
            );
            assert!(
                (b.height() - item.depth).abs() <= 1.0,
                "{id}: symbol depth {} vs {}",
                b.height(),
                item.depth
            );
        }
        if item.placement == Placement::WallMounted {
            assert!(
                b.min.y.abs() < 1e-6,
                "{id}: back edge not on the wall (y={})",
                b.min.y
            );
            assert!(
                b.center().x.abs() < 0.5,
                "{id}: not centered on x ({})",
                b.center().x
            );
        } else if !OVERHANG.contains(&id.as_str()) {
            assert!(
                b.center().x.abs() < 0.5 && b.center().y.abs() < 0.5,
                "{id}: center ({}, {})",
                b.center().x,
                b.center().y
            );
        }
    }
}

#[test]
fn overhanging_items_really_overhang() {
    // The exemption list must not go stale.
    let items = new_items();
    for id in OVERHANG {
        let item = items.iter().find(|i| i.id == *id).expect(id);
        let b = item.symbol.bounds().unwrap();
        assert!(
            b.width() > item.width + 1.0 || b.height() > item.depth + 1.0,
            "{id}"
        );
    }
}

#[test]
fn placements_follow_the_rules() {
    let lib = Library::with_all_core();
    let get = |id: &str| lib.get(id).unwrap_or_else(|| panic!("missing {id}"));
    let wall = |id: &str, elev: f64| {
        let i = get(id);
        assert_eq!(i.placement, Placement::WallMounted, "{id}");
        assert_eq!(i.elevation, elev, "{id}");
    };
    wall("core.lighting.wall_sconce", 66.0);
    wall("core.lighting.vanity_bar_3light", 78.0);
    wall("core.bathkitchen.range_hood_30", 66.0);
    wall("core.bathkitchen.range_hood_36", 66.0);
    wall("core.bathkitchen.wall_oven_30", 30.0);
    wall("core.furniture.tv_65_wall", 48.0);
    for id in [
        "core.bathkitchen.cooktop_30_4burner",
        "core.bathkitchen.cooktop_36_5burner",
        "core.bathkitchen.sink_undermount_oval",
        "core.bathkitchen.sink_farmhouse_33",
        "core.bathkitchen.sink_bar_15",
    ] {
        assert_eq!(get(id).placement, Placement::Countertop, "{id}");
    }
    for id in [
        "core.lighting.chandelier_28",
        "core.lighting.recessed_4in",
        "core.lighting.recessed_6in",
        "core.lighting.ceiling_fan_light_52",
        "core.lighting.exhaust_fan_14",
    ] {
        assert_eq!(get(id).placement, Placement::Ceiling, "{id}");
        assert_eq!(get(id).elevation, 0.0, "{id}");
    }
    assert_eq!(
        get("core.plants.oak_20ft").placement,
        Placement::FreeStanding
    );
}

#[test]
fn plant_sizes_and_names() {
    let lib = Library::with_all_core();
    for (id, size) in [
        ("core.plants.crape_myrtle_10ft", 120.0),
        ("core.plants.oak_20ft", 240.0),
        ("core.plants.maple_30ft", 360.0),
        ("core.plants.boxwood_2ft", 24.0),
        ("core.plants.hydrangea_3ft", 36.0),
        ("core.plants.holly_4ft", 48.0),
    ] {
        let i = lib.get(id).unwrap();
        assert_eq!((i.width, i.depth), (size, size), "{id}");
    }
    assert_eq!(
        lib.get("core.plants.oak_20ft").unwrap().category,
        ["Plants", "Trees", "Deciduous"]
    );
    for name in [
        "Oak",
        "Maple",
        "Crape Myrtle",
        "Boxwood",
        "Holly",
        "Hydrangea",
        "Japanese Maple",
        "Magnolia",
        "Pine",
        "Cypress",
    ] {
        let hits = lib.search(name);
        assert!(
            hits.iter().any(|i| i.id.starts_with("core.plants.")),
            "no plant found for {name}"
        );
    }
}

#[test]
fn search_sink_returns_many_with_name_matches_first() {
    let lib = Library::with_all_core();
    let hits = lib.search("sink");
    assert!(hits.len() >= 8, "only {} results", hits.len());
    let named = hits
        .iter()
        .take_while(|i| i.name.to_lowercase().contains("sink"))
        .count();
    assert!(named >= 8, "only {named} leading name matches");
    for later in &hits[named..] {
        assert!(
            !later.name.to_lowercase().contains("sink"),
            "{} ranked too low",
            later.name
        );
    }
    // The old starter sinks are still found alongside the new ones.
    assert!(hits.iter().any(|i| i.id == "core.plumbing.vanity_sink_30"));
    assert!(hits
        .iter()
        .any(|i| i.id == "core.bathkitchen.sink_farmhouse_33"));
}

#[test]
fn search_finds_new_items_by_tag_and_name() {
    let lib = Library::with_all_core();
    assert_eq!(
        lib.search("chandelier")[0].id,
        "core.lighting.chandelier_28"
    );
    assert_eq!(lib.search("bidet")[0].id, "core.bathkitchen.bidet");
    assert!(lib
        .search("200a")
        .iter()
        .any(|i| i.id == "core.electrical.panel_200a"));
    assert!(lib
        .search("pickup")
        .iter()
        .any(|i| i.id == "core.exterior.pickup_80x230"));
}

#[test]
fn tree_counts_sum_to_total() {
    let lib = Library::with_all_core();
    let total = lib.len();
    assert!(total >= 40 + 20 + 24 + 16 + 24);
    let tree = lib.tree();
    assert_eq!(tree.count, total);
    let top: usize = tree.children.iter().map(|c| c.count).sum();
    assert_eq!(top + tree.item_ids.len(), total);
    for name in [
        "Architectural",
        "Plants",
        "Bath & Kitchen",
        "Lighting",
        "Electrical",
        "Furniture",
        "Exterior",
    ] {
        assert!(tree.child(name).is_some(), "missing top-level {name}");
    }
    fn check(node: &plan_library::CategoryNode) {
        let kids: usize = node.children.iter().map(|c| c.count).sum();
        assert_eq!(node.count, kids + node.item_ids.len(), "{}", node.name);
        node.children.iter().for_each(check);
    }
    check(&tree);
}

#[test]
fn json_round_trips_for_every_catalog() {
    for cat in all_core_catalogs() {
        let json = cat.to_json().unwrap();
        let back = Catalog::from_json(&json).unwrap();
        assert_eq!(back, cat, "{}", cat.name);
    }
}
