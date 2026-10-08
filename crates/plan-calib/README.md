# plan-calib

Read-only access to Chief Architect `.calib` / `.calibz` library catalogs, so
Plan Studio can browse a user's own Chief libraries (Core, Bonus,
Manufacturer, User) at runtime. `std` + `serde_json` only; no SQLite or zip
crates.

## Licensing note

Chief catalogs, their thumbnails and the textures in `.calibz` packs are
Chief Architect and manufacturer licensed content. This crate **reads them in
place** from the user's installation. It never ships, copies or commits any of
it:

- `.gitignore` excludes `*.calib`, `*.calibz`, `*.calib_error`.
- Unit tests use synthetic fixtures generated at test time with the `sqlite3`
  and `zip` command line tools (tests skip if a tool is missing).
- Integration tests against the real install are `#[ignore]` (CI has no Chief).
- Opening a `.calibz` writes its embedded `.calib` to
  `<temp dir>/plan-studio/` as a cache. That is a derived copy of the user's own
  file outside the repo; delete it any time.
- The catalog index cache `~/.plan-studio/chief-catalog-index.json` stores only
  file paths, sizes, mtimes and catalog UUIDs.

## Format summary

Full notes: `docs/chief-library-format.md`.

- `.calib` is a SQLite 3 database (rollback-journal mode, UTF-8). Objects are
  rows of `LibraryObjects`; the display name is `Tags4LibraryObjects.Alias`;
  categories are the `Tags` tree (`Parent`, root `GroupType` 2, root UUID equals
  `GlobalData.DatabaseUniqueId`); keywords are `Keywords4LibraryObjects` joined
  to `Keywords`; thumbnails are PNG blobs in `LibraryObjects.Thumbnail`.
- `.calibz` is a zip (deflate) of textures plus one `.calib`.
- `Chief Library.json` lists the installed catalogs by UUID (category 1 Core,
  2 Manufacturer, 4 Bonus). The files sit in `Core Libraries`, `Bonus
  Libraries` and `Manufacturer Libraries` under
  `/Library/Application Support/Chief Architect Premier X18/`.
- `AssociatedData` blobs (keyed by `LibraryObjectId`) often start with a JSON
  document at offset 16 (u32 LE length at offset 12), followed by an
  undecoded binary stream.

## Modules

| Module | Role |
|---|---|
| `sqlite` | Minimal read-only SQLite reader: header checks (rejects WAL, non-UTF-8, hot journal), `sqlite_master`, table b-tree scan, rowid lookup, overflow chains, column list parsed from `CREATE TABLE`. Positioned reads only (`FileExt::read_at`; the `seek-io` feature or non-Unix targets use seek+read). Memory stays bounded: one page at a time plus a cache of at most 4096 interior pages. `RowIter::with_blobs(false)` reports `Value::BlobLen` without reading blob bytes; `with_max_columns(n)` stops decoding early; `Db::read_column` reads one column (optionally a prefix) of one row. |
| `inflate`, `zip` | RFC 1951 inflate (stored, fixed, dynamic), streaming through a 64 KiB window. Zip reader with EOCD, zip64, central directory, CRC-32 verification. `CalibZ` lists entries, `extract_calib_to(path)` streams the catalog out, `extract_calib_to_temp()` caches it, `texture(basename)` extracts one image. |
| `catalog` | `ChiefCatalog::open(path)` (either extension): `id`, `name`, `tags`, `category_tree`, `objects` (lazy), `count`, `thumbnail`, `associated_json`, `texture`. |
| `registry` | `ChiefRegistry::load_default()` reads the X18 registry (X17 fallback) and resolves each UUID to a file by opening each candidate's `GlobalData` once; cached by path, size and mtime. |
| `decode` | Decoders for Chief's binary blobs: `decode_size` (default width/depth/height), `decode_symbol_2d` / `symbol_from_meshes` (plan symbol from geometry), `parse_face_stream` (symDxf), `parse_triangle_meshes` (`CD AB 74 00`), `decode_cdab_records` (generic record walker), `decode_object` (everything for one object). See "Decoded layouts". |
| `bridge` | `to_plan_library(&catalog, limit)` maps objects to `plan_library::CatalogItem` plus a thumbnail side map and `BridgeStats`; sizes and plan symbols come from `decode`. |
| root | `ChiefLibrary::discover()`, `catalogs()`, `open(idx)`, `search(query, max)`, `stats()`. |

## How Plan Studio uses it

1. At startup (or lazily when the library browser opens) call
   `ChiefLibrary::discover()`. It reads the registry and the uuid cache; on the
   first run it also peeks at `GlobalData` of each `.calib` once (under a second
   for the 400 installed catalogs).
2. Show `catalogs()` as the top level (User, Core, Bonus, Manufacturer). Open one
   with `open(idx)`; show `category_tree()` as the folder tree and page through
   `objects()`, which touches only the leading columns of each row.
3. Load thumbnails on demand with `thumbnail(library_object_id)` as rows scroll
   into view.
4. For global search use `search("shaker door", 50)`; it streams catalog by
   catalog and stops at `max`.
5. To place an item, convert a page with `to_plan_library(&cat, Some(n))`. Each
   item gets id `chief.<catalog-uuid>.<library_object_id>`, its category path,
   keywords as tags and `FreeStanding` placement. The size is decoded from the
   object's blobs (see below); the symbol is the decoded plan view when its
   footprint is within 10% of the size, else a placeholder (rectangle, X and
   the name's first letter). Items for which nothing gives a size are 24 x 24 x
   24 in and tagged `size-unknown`. `BridgeResult::stats` reports
   `decoded_symbols`, `decoded_sizes` and `total`. Thumbnails come back in
   `BridgeResult::thumbnails` because `CatalogItem` has no field for them.
   Each conversion reads the object's geometry blobs (up to a few MB), so page
   large catalogs.

## Tests

```text
cargo test -p plan-calib                                   # fast, synthetic fixtures
cargo clippy -p plan-calib --all-targets -- -D warnings
cargo test -p plan-calib --release -- --ignored --nocapture --test-threads=1
```

`tests/real_decode.rs` (ignored, use `--release`) surveys decode coverage over
Core Architectural, Interiors and MEP and renders sample symbols to SVG
(`PLAN_CALIB_SVG_DIR`). `examples/probe.rs` is the reverse-engineering probe: it
dumps hex and number views of an object's blobs to a directory outside the repo.

The ignored suite (`tests/real_install.rs`) needs the real X18 install: it opens
`CoreArchitectural.calib`, builds its category tree, reads a thumbnail, extracts
a `.calibz` (set `PLAN_CALIB_TEST_CALIBZ` to choose the pack), checks that
`discover()` finds at least 400 catalogs, and opens and lists every installed
catalog.

## Decoded layouts

All little endian, all lengths in inches. Found by differential analysis on the
local Core catalogs (X18 build 28.0.2.72); counts are from that install.

### Size: `Data4LibraryObjects.Data`

The object's default size is three floats followed by three exact 1.0 scale
factors, in the library-object record (the `CD AB 72` record, which follows a
`Copyright` string):

```text
FF FF FF FF [flag]  w d h  1.0 1.0 1.0     f32 form (most Interiors / Exteriors)
FF FF FF FF  03  0.5  w d h  1.0 1.0 1.0   f64 form (parametric Architectural / MEP;
                                           Interiors "Dianne Sofa" has no 0.5)
```

`w` is X, `d` is Y (front to back), `h` is Z. The anchor bytes vary by record
version, so `decode::find_size_record` searches for three plausible values
(0.05 to 2000) followed by `1.0 x3` as f32 or f64 and takes the first candidate
after the first `Copyright` string (composite objects embed their parts' records
later). Examples: `Cubby Bench` 50 x 16 x 18 (f32 at 0x184), `Door E29`
38 x 1.375 x 79.875 (f64), `Dianne Sofa` 80.129 x 40.729 x 40.0 (f64 at 0xd6f).

Check: against the bounds of the `symDxf` geometry, 179 of 187 Interiors and 300
of 342 Exteriors records agree within 3% (the rest are partial or nominal sizes).
Coverage: 897 of 970 Interiors, 1530 of 1947 Architectural, 344 of 378 MEP objects
carry a record. Without one the size falls back to the bounds of decoded
meshes (`SizeSource::MeshBounds`). `elevation` is always 0: no blob field for it
was found (pendant lights have negative mesh Z, so mesh Z is not an elevation).

Plants (Core Plants): the record is `00 A6 91 3C 00 00 00 00 00 00 F0 3F` followed by
an f64 and, within the 48 bytes before it, a u32. The f64 is taken as the spread
and the last non-zero u32 as the height. This is **inferred** from tree
thumbnails (spruce about 0.4 as wide as tall: 47.6 vs 120), not proven.

### Plan symbol: `LibrarySymbolData.symDxf`

`symDxf` is **not DXF**. Neither a text DXF group-code stream nor AutoCAD binary
DXF (2-byte code plus typed value) parses: 0 of 541 blobs (190 Interiors, 351
Exteriors) are consumed to 90% by a strict binary-DXF reader (`examples/probe.rs`
repeats the test). It is a **3D polygon face stream** in object space (X right,
Y towards the back, front at -Y, Z up):

```text
0   u16 version (0x092b, 0x05bf, 0x0601 ...)
2   u32 0xFFFFFFFF
6   u32 face count (streams that start 0xFFFFFFxx instead have no count)
    faces, each:
      u16 n        vertex count (3 and 4 dominate; up to 31 seen)
      u16 flags    low byte edge-visibility bits (0x03, 0x06, 0x0f ...), 0x1000 group start
      u32 aux      0 or 1
      u16 tag      material / sub-object index
      n x (f64 x, f64 y, f64 z)     88% are floats widened to f64 (low 24 bits 0)
      trailer      FF-filled link words, in some streams a layer-name string
                   ("Adjust Light", "A-FIXT-MAIN-0")
```

Faces are located by scanning for `n/flags/aux` plus `n` plausible coordinates (the
trailer length varies). All 541 blobs yield faces; where the header carries a count
it equals the faces found for 129 of 174 Interiors blobs and is within a few
faces for the rest. `symBlock` holds the block name and a parameter record; it is
not needed for the geometry and not decoded.

### Triangle meshes: `CD AB 74 00` in `AssociatedData` and `SymbolData`

```text
CD AB 74 00 | u16 flags | u32 version 1..5 | u32 N
N x 48 bytes: f64 x, y, z, then three f64 that were 0 in every sample
u32 M
M x 80 bytes: u32 a, b, c (vertex indices of one triangle), u32 partner triangle
              (the other half of the quad), u32 e, u32 f (ids), 14 zero words
```

588 of 588 sampled records validate (indices below N, sizes inside the blob). This
is the tessellated geometry of most Architectural, MEP and Interiors objects
(toilets, tubs, sinks, sofas, doors); `AssociatedData` stores it after the JSON
header (`decode::split_associated`). Objects are often built from several parts;
only this mesh kind is decoded, so some objects are partial (see below).

### Plan view

Chief draws library objects in plan from their 3D geometry, and no catalog holds a
separate 2D symbol (`twoDRep`, `drawInfo` are NULL everywhere). `decode::plan_view`
therefore projects the meshes straight down with hidden-line removal: weld
vertices, rasterize a 200-cell depth buffer, keep boundary, crease (> 20 degrees)
and silhouette edges that are not under a higher surface, chain, simplify
(Douglas-Peucker), turn near-circles into `Stroke::Circle`, rotate 180 degrees and
centre on the XY bounds so the front faces +Y as `plan_library` expects. A decoded
symbol is used by the bridge only when its footprint is within 10% of the size.

### `CD AB` records

`decode_cdab_records` splits a blob at every `CD AB <u16 kind>` (kinds seen:
0x61 block, 0x72 library object, 0x7b / 0x78 / 0x6d wrappers, 0x74 mesh, 0x30 /
0x1f / 0x23 geometry parts, 0x19d, 0x5fe3, 0x403b). A record's `raw_len` runs to the
next magic. Strings are `u32 len, bytes, NUL` (Latin-1 / UTF-8 `(c)` sign,
`description`, `%automatic_description%`, keywords, size-variant names such as
`72W34 5/16D`, `Main`, `Pillow`, `Base`); numbers are heuristic f64 / f32 runs.

### Measured coverage (Core Architectural + Interiors + MEP, 3295 objects)

| | Architectural | Interiors | MEP | all |
|---|---|---|---|---|
| size decoded | 82.6% | 99.1% | 98.1% | 89.3% |
| plan symbol decoded | 82.6% | 97.4% | 98.1% | 88.8% |
| symbol bounds within 10% of decoded size | 73.6% | 73.5% | 76.0% | 73.9% |

(The last row is over objects that have both: 1184 of 1609, 695 of 945, 282 of
371.) Release build, whole survey in about 5 s.

## Not decoded yet

- About a quarter of the geometry is partial: the decoded meshes cover only part of
  the object (a chair's seat but not its legs, `Dianne Sofa` without cushions). The
  other parts are `CD AB 30` / `1f` / `23` records (parametric solids with
  profiles) and `19d` / `403b` / `f530` records, none decoded. The bridge falls back
  to the placeholder when the footprint disagrees with the size.
- Parametric cabinets (`Double Drawer Base`, `End Cabinet Base`: 404-byte
  `AssociatedData`) have no geometry in the catalog; Chief generates them.
- Door swing arcs and window symbols: nothing in the blobs of Core doors mentions a
  swing, so a decoded door is its slab only (`Door E29`: 38 x 0.69 in).
- Windows such as `3018 A` have no size record (the size is probably encoded in the
  name or a parameter table); they fall back to mesh bounds or the default.
- Elevation (floor to bottom) of wall and ceiling items.
- Meaning of the `0x74` header words, the partner/e/f fields of the triangle records,
  the three trailing doubles of each vertex, `symBlock`, the 0x61 CAD blocks in
  `SymbolData` (CoreCAD details), `LibraryObjects.Type`, `LinkTable` texture paths and
  `Content/<hash>-01` resolution.
- Plant spread/height semantics (inferred) and the plant plan symbol: it is a canopy
  circle, Chief draws a textured image.
