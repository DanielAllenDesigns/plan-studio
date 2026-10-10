//! The on-disk catalog format: items grouped into a named [`Catalog`].

use crate::symbol::{Stroke, Symbol2d};
use plan_core::SymbolSchedule;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How an item attaches to the building, which also fixes its symbol origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// Sits against (or on) a wall. Symbol origin is the back-center, with
    /// the back edge on `y = 0` and the item extending towards `+y`.
    WallMounted,
    /// Stands on the floor away from walls. Symbol origin is the center.
    FreeStanding,
    /// Attached to the ceiling. Symbol origin is the center.
    Ceiling,
    /// Rests on a countertop or is set into one. Symbol origin is the center.
    Countertop,
}

/// What kind of object a library item is (the Library Browser "type" filter
/// and the default layer follow from it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// A plain plan symbol (furniture, fixtures drawn from a block).
    #[default]
    Symbol,
    /// A saved cabinet; placing it makes a cabinet object.
    Cabinet,
    /// A plumbing or appliance fixture.
    Fixture,
    /// An electrical device.
    Device,
    /// A saved group of CAD lines, arcs and circles.
    CadBlock,
    /// A saved text note.
    Text,
    /// A material swatch.
    Material,
    /// A picture (Create Image Library).
    Image,
    /// An imported 3D model (OBJ or glTF).
    Model,
}

impl ItemKind {
    /// Every kind, in the order the type filter lists them.
    pub const ALL: [ItemKind; 9] = [
        ItemKind::Symbol,
        ItemKind::Cabinet,
        ItemKind::Fixture,
        ItemKind::Device,
        ItemKind::CadBlock,
        ItemKind::Text,
        ItemKind::Material,
        ItemKind::Image,
        ItemKind::Model,
    ];

    /// Display name.
    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Symbol => "Symbol",
            ItemKind::Cabinet => "Cabinet",
            ItemKind::Fixture => "Fixture",
            ItemKind::Device => "Device",
            ItemKind::CadBlock => "CAD Block",
            ItemKind::Text => "Text",
            ItemKind::Material => "Material",
            ItemKind::Image => "Image",
            ItemKind::Model => "3D Model",
        }
    }

    fn is_symbol(&self) -> bool {
        *self == ItemKind::Symbol
    }
}

/// What a placed copy of an item starts with besides its size: the choices of
/// the Library Object Specification (Open Object) that a User Catalog item
/// remembers.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectDefaults {
    /// Placed copies are mirrored left to right.
    pub flip: bool,
    /// Label text of placed copies.
    pub label: String,
    /// Schedule settings of placed copies.
    pub schedule: Option<SymbolSchedule>,
    /// Library-specific choices (`door_style`, `cabinet_door`, `hardware`).
    pub options: BTreeMap<String, String>,
}

impl ObjectDefaults {
    /// True when nothing differs from a plain placed copy.
    pub fn is_default(&self) -> bool {
        *self == ObjectDefaults::default()
    }
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn is_zero3(v: &[f64; 3]) -> bool {
    v.iter().all(|c| *c == 0.0)
}

/// One placeable object in a catalog (a Library Browser entry).
///
/// `width` is the extent along the symbol's X axis and `depth` the extent
/// along Y. A symbol may draw slightly beyond that footprint when it shows
/// swing arcs or tucked-in chairs; those extras are annotation, not size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogItem {
    /// Stable unique identifier, e.g. `"core.plumbing.toilet_elongated"`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Category path from the root, e.g. `["Architectural","Plumbing","Toilets"]`.
    pub category: Vec<String>,
    /// Default width in inches (symbol X extent).
    pub width: f64,
    /// Default depth in inches (symbol Y extent).
    pub depth: f64,
    /// Default height in inches.
    pub height: f64,
    /// Default height of the item's bottom above the floor, in inches.
    pub elevation: f64,
    /// Attachment style and symbol-origin convention.
    pub placement: Placement,
    /// Extra search keywords.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The plan-view block.
    pub symbol: Symbol2d,
    /// Path of the 3D model file for 3D views. User-library items keep their
    /// mesh as a `.psm` file ([`crate::model::Model3d`]) relative to the
    /// library folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model3d: Option<String>,
    /// Optional manufacturer name for product-specific items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// What the item is (see [`ItemKind`]).
    #[serde(default, skip_serializing_if = "ItemKind::is_symbol")]
    pub kind: ItemKind,
    /// Style keyword (`"modern"`, `"traditional"`), matched by the style
    /// filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Default layer of placed copies; `None` uses the category's layer
    /// ([`crate::rules::default_layer`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// Overrides whether the item turns to face away from the nearest wall
    /// when placed; `None` follows [`Placement`] (wall-mounted items do).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_rotate: Option<bool>,
    /// Rotation of the 3D model about the vertical axis, degrees.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub model_rotation: f64,
    /// Offset of the 3D model from the symbol origin (x, up, plan y), inches.
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub model_origin: [f64; 3],
    /// The saved object itself for kinds that place a real plan object: a
    /// cabinet's JSON, or the CAD items of a block / text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    /// What placed copies start with (Open Object > OK).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<ObjectDefaults>,
}

impl CatalogItem {
    /// Creates an item with zero size and no category; refine it with the
    /// `with_*` methods.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        placement: Placement,
        symbol: Symbol2d,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category: Vec::new(),
            width: 0.0,
            depth: 0.0,
            height: 0.0,
            elevation: 0.0,
            placement,
            tags: Vec::new(),
            symbol,
            model3d: None,
            manufacturer: None,
            kind: ItemKind::Symbol,
            style: None,
            layer: None,
            auto_rotate: None,
            model_rotation: 0.0,
            model_origin: [0.0; 3],
            payload: None,
            defaults: None,
        }
    }

    /// Sets the category path.
    pub fn with_category(mut self, path: &[&str]) -> Self {
        self.category = path.iter().map(|s| (*s).to_owned()).collect();
        self
    }

    /// Sets the default width, depth and height in inches.
    pub fn with_size(mut self, width: f64, depth: f64, height: f64) -> Self {
        self.width = width;
        self.depth = depth;
        self.height = height;
        self
    }

    /// Sets the default bottom elevation above the floor in inches.
    pub fn with_elevation(mut self, elevation: f64) -> Self {
        self.elevation = elevation;
        self
    }

    /// Sets the search tags.
    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|s| (*s).to_owned()).collect();
        self
    }

    /// The category path joined for display, e.g. `"Architectural > Plumbing"`.
    pub fn category_label(&self) -> String {
        self.category.join(" > ")
    }
}

/// Builds an item from the pieces every catalog entry needs. `size` is
/// `(width, depth, height, elevation)` in inches.
pub(crate) fn entry(
    id: &str,
    name: &str,
    placement: Placement,
    category: &[&str],
    size: (f64, f64, f64, f64),
    tags: &[&str],
    strokes: Vec<Stroke>,
) -> CatalogItem {
    CatalogItem::new(id, name, placement, Symbol2d::new(strokes))
        .with_category(category)
        .with_size(size.0, size.1, size.2)
        .with_elevation(size.3)
        .with_tags(tags)
}

/// A named collection of items, stored as one JSON file.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Catalog {
    /// Catalog display name, e.g. `"Core Catalog"`.
    pub name: String,
    /// The items it contains.
    pub items: Vec<CatalogItem>,
}

impl Catalog {
    /// Creates an empty catalog.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
        }
    }

    /// Serializes to pretty-printed JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parses a catalog from JSON.
    pub fn from_json(json: &str) -> Result<Catalog, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_json_uses_defaults() {
        let json = r#"{
            "name": "Mini",
            "items": [{
                "id": "x.a", "name": "A", "category": ["Cat"],
                "width": 1, "depth": 2, "height": 3, "elevation": 0,
                "placement": "free_standing",
                "symbol": { "strokes": [ { "type": "circle",
                    "center": {"x": 0, "y": 0}, "radius": 1 } ] }
            }]
        }"#;
        let c = Catalog::from_json(json).unwrap();
        assert_eq!(c.items.len(), 1);
        assert!(c.items[0].tags.is_empty());
        assert_eq!(c.items[0].model3d, None);
        assert_eq!(c.items[0].placement, Placement::FreeStanding);
    }

    #[test]
    fn optional_fields_round_trip() {
        let mut item = CatalogItem::new("x.b", "B", Placement::Ceiling, Symbol2d::default())
            .with_category(&["A", "B"])
            .with_size(1.0, 2.0, 3.0)
            .with_elevation(4.0)
            .with_tags(&["t"]);
        item.model3d = Some("models/b.glb".into());
        item.manufacturer = Some("Acme".into());
        let mut cat = Catalog::new("C");
        cat.items.push(item.clone());
        let back = Catalog::from_json(&cat.to_json().unwrap()).unwrap();
        assert_eq!(back.items[0], item);
        assert_eq!(back.items[0].category_label(), "A > B");
    }

    #[test]
    fn bad_json_is_an_error() {
        assert!(Catalog::from_json("{ not json").is_err());
    }
}
