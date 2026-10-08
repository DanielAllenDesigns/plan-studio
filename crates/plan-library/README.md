# plan-library

The catalog system for Plan Studio, the counterpart of Chief Architect's
Library Browser. It provides a JSON catalog format, an in-memory index with
ranked search and a category tree, and a built-in starter catalog of 2D plan
symbols.

All lengths are **inches**.

## Quick start

```rust
use plan_library::{core_catalog, Catalog, Library};

let mut lib = Library::default();
lib.add(core_catalog());                       // built-in symbols
// lib.add(Catalog::from_json(&std::fs::read_to_string("mine.json")?)?);

let hits = lib.search("sink");                 // ranked, case-insensitive
let toilet = lib.get("core.plumbing.toilet_elongated").unwrap();
let tree = lib.tree();                         // root "Library" -> "Architectural" -> ...
```

## Catalog file format

A catalog is one JSON object:

```json
{
  "name": "My Catalog",
  "items": [
    {
      "id": "mine.furniture.stool",
      "name": "Bar Stool",
      "category": ["Architectural", "Furniture", "Seating"],
      "width": 16, "depth": 16, "height": 30, "elevation": 0,
      "placement": "free_standing",
      "tags": ["bar", "kitchen"],
      "symbol": {
        "strokes": [
          { "type": "circle", "center": { "x": 0, "y": 0 }, "radius": 8 },
          { "type": "polyline", "closed": false,
            "points": [ { "x": -4, "y": 0 }, { "x": 4, "y": 0 } ] },
          { "type": "arc", "center": { "x": 0, "y": 0 }, "radius": 6,
            "start_deg": 0, "end_deg": 180 }
        ]
      },
      "model3d": null,
      "manufacturer": null
    }
  ]
}
```

### Item fields

| Field | Meaning |
| --- | --- |
| `id` | Unique, stable id. Convention: `<catalog>.<group>.<name>`, lower snake case, e.g. `core.plumbing.toilet_elongated`. |
| `name` | Display name. Search ranks name matches first. |
| `category` | Path from the root, e.g. `["Architectural","Plumbing","Toilets"]`. Drives the browser tree. An empty path files the item at the tree root. |
| `width`, `depth`, `height` | Default size in inches. `width` is the symbol's X extent, `depth` its Y extent. |
| `elevation` | Default height of the item's bottom above the floor, in inches. |
| `placement` | `wall_mounted`, `free_standing`, `ceiling` or `countertop`. |
| `tags` | Optional extra search keywords (default empty). |
| `symbol` | The 2D plan block (see below). |
| `model3d` | Optional. Reserved for a future glTF path; unused today. |
| `manufacturer` | Optional manufacturer name. |

`tags`, `model3d`, `manufacturer` and a polyline's `closed` flag may be omitted.

### Symbol strokes

A symbol is a list of strokes in a local frame: **X to the right, Y into the
room**, in inches.

* `{"type":"polyline","points":[...],"closed":bool}` : connected line segments.
* `{"type":"arc","center":P,"radius":r,"start_deg":a,"end_deg":b}` : swept
  **counter-clockwise** from `a` to `b` degrees (0 is +X, 90 is +Y). A sweep of
  360 or more is a full circle.
* `{"type":"circle","center":P,"radius":r}`.

### Origin convention

* `wall_mounted`: the origin is the **back-center** of the item. The back edge
  lies on `y = 0` (the wall face) and the symbol extends towards `+y`. Use this
  for anything whose back sits against a wall: toilets, cabinets, beds, outlets.
* `free_standing`, `ceiling`, `countertop`: the origin is the **center**.

`width`/`depth` describe the item's footprint. A symbol may draw a little
beyond it for annotation, such as refrigerator door swings or the chairs around
a dining table.

To place an item in a plan, map it with
`symbol.transformed(position, angle_rad, scale)`: scale first, then rotate
counter-clockwise about the origin, then translate. `symbol.bounds()` gives the
exact bounding box (arcs included).

## Adding a catalog

**From a JSON file** (no code): write a file in the format above and load it.

```rust
let json = std::fs::read_to_string("catalogs/lighting.json")?;
library.add(plan_library::Catalog::from_json(&json)?);
```

Keep ids unique across everything you load; if two items share an id,
`Library::get` returns the one from the catalog added first.

**From code**: build a `Catalog` and add it, then optionally save it with
`Catalog::to_json()`.

```rust
use plan_library::{Catalog, CatalogItem, Placement, Stroke, Symbol2d};
use plan_core::geometry::Point;

let stool = CatalogItem::new(
    "mine.furniture.stool",
    "Bar Stool",
    Placement::FreeStanding,
    Symbol2d::new(vec![Stroke::Circle { center: Point::ZERO, radius: 8.0 }]),
)
.with_category(&["Architectural", "Furniture", "Seating"])
.with_size(16.0, 16.0, 30.0)
.with_tags(&["bar", "kitchen"]);

let mut catalog = Catalog::new("My Catalog");
catalog.items.push(stool);
```

**Extending the built-in catalog**: add the symbol builder and the
`CatalogItem` entry to the matching section of `src/starter.rs`. Stroke
helpers (`rect`, `rounded_rect`, `circle`, `arc`, `line`, ...) live in
`src/shapes.rs`. The tests require unique ids, non-empty symbols, and symbol
bounds that match the declared width and depth.

## Search and the category tree

`Library::search(query)` is case-insensitive. The query is split on whitespace
and every term must match. Ranking, best first: name (whole word, prefix, then
substring), then tags, then category path; ties sort by name. An empty query
returns everything.

`Library::tree()` returns a virtual root named `"Library"` whose children are
the top-level categories. Every node carries `count` (items in its subtree),
`children` (sorted by name) and `item_ids` (items filed directly there).

## Layout

| File | Contents |
| --- | --- |
| `src/symbol.rs` | `Symbol2d`, `Stroke`, `Bounds`, transforms |
| `src/catalog.rs` | `CatalogItem`, `Catalog`, `Placement`, JSON |
| `src/library.rs` | `Library`, search, `CategoryNode` |
| `src/shapes.rs` | stroke constructors for the starter symbols |
| `src/starter.rs` | `core_catalog()` and its symbols |
