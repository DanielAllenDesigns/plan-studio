# plan-library

The catalog system for Plan Studio, the counterpart of Chief Architect's
Library Browser. It provides a JSON catalog format, an in-memory index with
ranked search and a category tree, and built-in catalogs of 2D plan symbols
(a starter set plus four larger catalogs, listed below).

All lengths are **inches**.

## Quick start

```rust
use plan_library::{core_catalog, Catalog, Library};

let mut lib = Library::default();
lib.add(core_catalog());                       // built-in starter symbols
// or: let lib = Library::with_all_core();     // starter + all four catalogs below
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

## Built-in catalogs

`all_core_catalogs()` returns the starter `core_catalog()` plus the four
catalogs below, and `Library::with_all_core()` loads them all (`Library::with_core()`
still loads only the starter set). Each catalog module exposes
`pub fn catalog() -> Catalog`.

| Catalog (module) | Items | Top-level categories and contents |
| --- | --- | --- |
| Core Catalog (`starter`, `core_catalog()`) | 40+ | `Architectural`: plumbing, appliances, cabinets, furniture, electrical, exterior |
| Plants (`catalog_plants`) | 21 | `Plants`: deciduous trees (10/20/30 ft canopy), evergreens, palm, ornamental trees (Japanese maple, magnolia), round shrubs (2/3/4 ft), hedges (4/8 ft), ground cover, flower bed, perennials, grasses, boulder, planters |
| Bath & Kitchen (`catalog_bath_kitchen`) | 35 | `Bath & Kitchen`: toilets, bidet, urinal, 8 sinks, 5 tubs, 4 showers, water heater, stacked washer/dryer, range hoods, cooktops, wall and double ovens, refrigerators, wine fridge, compactor, ice maker |
| Lighting & Electrical (`catalog_lighting_electrical`) | 22 | `Lighting`: chandelier, pendant, track, recessed 4/6 in, drum, sconce, under-cabinet, vanity bar, step, exterior lantern, post and flood lights, ceiling and exhaust fans, heat lamp. `Electrical`: 240V dryer, USB and floor outlets, data and TV jacks, 200A panel |
| Furniture & Exterior (`catalog_furniture_exterior`) | 27 | `Furniture`: sectional, chaise, recliner, ottoman, bookcase, media console, TV, pianos, crib and beds, bench, island with seating, bar stools. `Exterior`: outdoor dining, lounge, grill, fire pit, hot tub, kidney pool, mailbox, bicycle, SUV, pickup |

Conventions in the extended catalogs, in addition to the origin convention above:

* Ids are `core.<group>.<name>` (`core.plants.oak_20ft`, `core.bathkitchen.sink_bar_15`,
  `core.lighting.recessed_6in`, `core.furniture.bed_twin_39x75`).
* Items that back onto a wall (toilets, tubs, showers, hoods, ovens, refrigerators,
  beds, case goods, sconces, vanity bars, outlets, the TV) are `wall_mounted`;
  cooktops and drop-in sinks are `countertop`; fixtures that attach to the
  ceiling are `ceiling` with `elevation` 0; everything else is `free_standing`.
* `elevation` is set where it matters: sconce 66, vanity bar 78, range hood 66,
  wall oven 30, TV 48.
* A symbol has at most 40 strokes. Its bounds match `width` and `depth` within
  1 inch, except `core.exterior.outdoor_dining_72x36`, whose six chairs overhang
  the 72x36 table (see `tests/catalogs.rs`).

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

**Extending the built-in catalogs**: add the symbol builder and the item to the
matching catalog module (`src/starter.rs` for the starter set, or one of the
`src/catalog_*.rs` files). Stroke helpers (`rect`, `rounded_rect`, `circle`,
`arc`, `line`, plus `ellipse`, `scalloped_circle`, `star`, `wavy_rect`, `fitted`,
`blob`) live in `src/shapes.rs`, and the extended catalogs build items with
`catalog::entry`. The tests require unique ids across all catalogs, non-empty
symbols, and symbol bounds that match the declared width and depth.

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
| `src/catalog_plants.rs` | `catalog_plants::catalog()` |
| `src/catalog_bath_kitchen.rs` | `catalog_bath_kitchen::catalog()` |
| `src/catalog_lighting_electrical.rs` | `catalog_lighting_electrical::catalog()` |
| `src/catalog_furniture_exterior.rs` | `catalog_furniture_exterior::catalog()` |
| `tests/catalogs.rs` | checks for the four extended catalogs and `with_all_core()` |

## User catalog support

* `manage`: folders (category paths under `User`), favorites and recents (`UserMeta`), rename, duplicate, move and delete.
* `browse`: `Filter` (query, type, catalog, style or manufacturer, size range, favorites, category) and `SortKey`.
* `model`: `Model3d` (indexed triangles, `.psm` file format) and the `preview` software rasterizer.
* `archive`: the user-library export (a stored zip with a `.calibz` extension). Chief cannot read it.
* `rules`: `placement_rules` (what an item snaps to, auto-rotate) and `default_layer`.
