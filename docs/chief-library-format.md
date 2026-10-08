# Chief Architect Premier X18: library content and file format

Status: read-only analysis, 2026-10-07. Machine: this Mac, Chief Architect Premier X18 (build 28.0.2.72).

## Handling rules

- All `.calib` / `.calibz` catalogs, the `Content/` and `Referenced Files/` assets, and the manufacturer catalogs are Chief Architect and manufacturer licensed content. Do not copy them into this repo and do not commit them. This document describes structure only and contains no catalog payloads.
- Every inspection was read-only. SQLite was opened with `sqlite3 -readonly` or `mode=ro&immutable=1`. Zip content was streamed (`unzip -p`) or extracted only into the session scratchpad and deleted afterwards. The probe scripts live in the scratchpad, not in the repo.

## 1. Locations

| Location | Files | Size | Kind |
|---|---|---|---|
| `/Library/Application Support/Chief Architect Premier X18/Core Libraries/` | 15 (11 `.calib` + 4 metadata files) | 2.2 GiB | Core catalogs. CoreArchitectural 900 MiB, CoreInteriors 475 MiB, CorePlants 275 MiB, CoreMEP 250 MiB, CoreExteriors 200 MiB, CoreMaterials 125 MiB, CorePatterns 1.8 MB, CoreCAD 8.2 MiB, CoreShapes 1.0 MiB, CoreBackdrops 0.9 MiB, CoreLineStyles 0.6 MiB |
| `/Library/Application Support/Chief Architect Premier X18/Bonus Libraries/` | 279 | 7.5 GiB | Bonus catalogs. Bonus3DPlants 600 MiB, BonusClosetAccessories2 425 MiB, BonusTables 11 MB (sampled) |
| `/Library/Application Support/Chief Architect Premier X18/Manufacturer Libraries/` | 112 | 17 GiB | Manufacturer catalogs. Kohler 4.0 GiB, Wayfair 1.6 GiB, WineRacksAmerica 1.2 GiB, SimpsonStrongTie 1.0 GiB, SubZero 0.9 GiB, Viking 0.8 GiB, GE 0.7 GiB, Blum 225 MiB (sampled), CustomWoodProductsLink 180 KB (sampled) |
| `/Library/Application Support/Chief Architect Premier X18/Manufacturer Data/` | 219 | 9.2 MiB | Not inspected |
| `/Library/Application Support/Chief Architect Premier X18/Content/` | 23,056 | 5.9 GiB | Hash-named image assets (`<64 hex>-01`). Sampled: JPEG and PNG. Not resolved to any catalog reference (see 5.6) |
| `/Library/Application Support/Chief Architect Premier X18/Referenced Files/` | 23,616 | 7.4 GiB | Original texture and image files under readable names (`Gravel.jpg`, PNG normal maps, 4K `.hdr` environment maps up to 50 MiB) |
| `/Library/Application Support/Chief Architect Premier X18/Help/` | 3,028 | 60 MiB | HTML help. Not catalog data |
| `~/Documents/Chief Architect Premier X18 Data/Database Libraries/` | 3 + 2 migration backups | 376 KB | User library. `User_Library.calib` 208,896 B (1 object, 2 tags). `Trash.calib` 176,128 B (0 objects). `User_Library.calib_error` 0 B |
| `~/Documents/Chief Architect Premier X18 Data/` (whole folder) | - | 8.4 GB | Includes Database Libraries and Scripts |
| `~/Library/Application Support/Chief Architect Premier X18/` | 10 + Data/ | 51 MB (Data/) | `Chief Library.json` (76.7 KB, 402 catalog entries, the catalog registry). `Data/Managed Resources/Content` (404 files, 1.6 MB), `Canon` (32 MB), `Sandbox` (9 MB) |
| Dropbox / Google Drive / Rabbitt packs (user catalogs) | Spotlight: 4,382 `.calib` and 2,162 `.calibz` across the whole Mac | - | Mixed X13 to X18 installs, DMDz and Rabs catalogs, and vendor packs. Not inventoried individually |

Note: `/Library/Application Support/Chief Architect Premier X14/` and `X17` also hold `.calib` files. They are not X18.

## 2. Format conclusion

- **`.calib` = SQLite 3 database.** Not zip, not a custom container. Chief writes its own binary records inside the BLOB columns (section 5).
- **`.calibz` = ZIP (deflate).** Verified on `Pro Plan Tools-240523.calibz` (966 MB): 920 entries (524 JPEG, 394 PNG, 1 BMP, 1 `.calib`). The embedded `.calib` begins with `SQLite format 3`. So a `.calibz` is a texture pack plus its catalog.
- **`Content/*`, `Referenced Files/*`:** plain JPEG, PNG, or HDR files, not wrapped.
- **`Chief Library.json`:** plain JSON index.
- **`LibraryObjects.Thumbnail`:** PNG.
- **`LibraryObjects.LinkTable`:** Chief binary (starts `3D 0F 00 00`) that embeds the original texture file paths.

### Exact header bytes (`.calib`)

```
CoreCAD.calib (8.5 MB)
00000000: 5351 4c69 7465 2066 6f72 6d61 7420 3300  SQLite format 3.
00000010: 1000 0101 0040 2020 0000 0033 0000 0829  .....@  ...3...)

User_Library.calib (208 KB)
00000000: 5351 4c69 7465 2066 6f72 6d61 7420 3300  SQLite format 3.
00000010: 1000 0101 0040 2020 0000 0032 0000 0033  .....@  ...2...3

CustomWoodProductsLink.calib (180 KB)  same 16-byte prefix pattern, page size 0x1000
```

- Bytes 16-17 = `10 00` → page size 4096.
- Bytes 18-19 = `01 01` → legacy rollback-journal mode, not WAL. No `-wal` file is needed. Check for a hot `-journal` file before reading.
- `file(1)` reports: "SQLite 3.x database, user version 196643, last written using SQLite version 3043001, schema 4, UTF-8". The user-version word is identical across all Chief catalogs.

### Large-file check

`CoreArchitectural.calib` (943,718,400 B) has an allocation within 0.1% of its apparent size. It is not sparse. Its page 2 begins `05 00 00 00 02 0f f4`, a standard interior/leaf B-tree page.

## 3. SQLite schema (catalog tables)

Common to every catalog (CoreCAD, BonusTables, Blum, CoreArchitectural, User_Library):

- **LibraryObjects** (`LibraryObjectId` PK, `Type`, `Lock`, `CopyrightId`, `Metric`, `ElementVersion`, `LibSymDataId`, `PlantDataId`, `ProductFilterId`, `CreationTime`, `ModificationTime`, `UniqueId` TEXT UUID 36 chars UNIQUE, `Legacy_ShortcutDatabaseId`, `Legacy_ImportName`, `Legacy_Name`, `Notes`, `ExtractionVersion`, `ProcessedFolders`, `LinkTable` BLOB, `Thumbnail` BLOB PNG, `PersistentId` TEXT UUID UNIQUE)
- **Data4LibraryObjects** (`LibraryObjectId`, `Data` BLOB): per-object CDAB record (5.1)
- **SymbolData4LibraryObjects** (`LibraryObjectId`, `SymbolData` BLOB): symbol geometry, Core catalogs only
- **LibrarySymbolData** (`LibSymDataId`, `symDxf`, `symBlock`, `twoDRep`, `drawInfo`, `lightData` BLOB, `isSymElement`): symbol record (5.2)
- **AssociatedData** (`AssociatedDataId`, `AssociatedDataBlob` BLOB): the bulk payload (5.3). Joined to the object by row order, not by a visible key
- **Tags** (`TagUniqueId` UUID PK, `Name`, `GroupType`, `ValueType`, `Parent` UUID, `Color`, `Pinned`): category tree. Recursive CTE with `prevent_tags_cycle_trigger`
- **Tags4LibraryObjects** (`Tags4LibraryObjectsId`, `LibraryObjectUniqueId` to LibraryObjects.UniqueId, `TagUniqueId`, `Value` BLOB, `Alias` TEXT): links objects to tags, and `Alias` holds the object display name
- **AUX_Tags4LibraryObjects** (same shape): empty in all samples
- **Keywords** (`KeywordId`, `Keyword` UNIQUE NOCASE) and **Keywords4LibraryObjects** (`LibraryObjectId`, `KeywordId`, `AutoGenerated`)
- **Styles** (`StyleId`, `Style`: values 10000 to 10009) and **Styles4LibraryObjects** (`LibraryObjectId`, `StyleId`)
- **GlobalData** (`GlobalId`, `CategoryId`, `Lock`, `convertedAlbs`, `DatabaseUniqueId` UUID, `Retired`, `ExportVersion` TEXT, `NeedsIDScrambling`): one row. `DatabaseUniqueId` equals the root Tags row for the catalog
- **Copyrights** (`CopyrightId`, `Copyright` UNIQUE)
- **ProductFilter** (`FilterId`, `Filter` TEXT UNIQUE): a bit string per filter
- **LibraryViews** (`LibraryViewId`, `LibraryView` TEXT, `Name`): the text is an XML `<TreeView>` header with `Product` name, `ProductVersion` (28.0.2.72), `FileVersion` (4090), `Lock`. It is metadata, not a category tree
- **ExtractionData** (`LibraryObjectId`, `ExtractionKey`, `ExtractionValue`): integer key/value per object (keys 6, 8, 13 seen)
- **PlantData**, **ShortcutDatabases**, **PersistentTagIDs**, **Upgrade25Helper**, **sqlite_stat1**: empty or helper tables in the samples
- **AA_InspectLibraryObjects**: a view joining LibraryObjects with the aliased tags

### Row counts (verified)

| Table | CoreCAD | BonusTables | Blum | CoreArchitectural | User_Library |
|---|---|---|---|---|---|
| LibraryObjects | 254 | 49 | 769 | 1,947 | 1 |
| Data4LibraryObjects | 254 | 49 | 769 | 1,947 | 1 |
| LibrarySymbolData | 0 | 49 (48 with symDxf and symBlock) | 762 (all blob columns NULL) | 1,403 (all blob columns NULL) | 0 |
| SymbolData4LibraryObjects | 43 rows (6 non-NULL) | 49 (NULL) | 0 | 0 | 0 |
| AssociatedData | 254 (NULL) | 49 (NULL) | 769 rows, 215.5 MB | 1,947 rows, 887 MB | 1 |
| Tags | 62 | 4 | 166 | 379 | 2 |
| Tags4LibraryObjects | 254 | 49 | 1,158 | - | 1 |
| Keywords / links | 79 / 993 | 5 / 101 | 156 / 3,198 | 686 keywords | 0 / 0 |
| Copyrights | 1 | 2 | 1 | - | 0 |
| ExtractionData | 0 | 147 | 1,123 | - | 0 |

- Table sizes by blob (largest first): CoreArchitectural AssociatedData 887 MB, Thumbnail 23 MB, Data 9 MB. CoreInteriors AssociatedData 360 MB, symDxf 64 MB, SymbolData 21 MB. CoreExteriors symDxf 110 MB, AssociatedData 32 MB, SymbolData 23 MB. CorePlants Thumbnail 256 MB (3,616 objects).
- `LibraryObjects.Type` observed: CoreCAD {12: 215, 32: 39}, BonusTables {1: 49}, Blum {0: 45, 2: 7, 14: 717}, CoreArchitectural {0: 510, 15: 374, 11: 214, 2: 203, 7: 155, 6: 112}. Meaning not confirmed. Type 32 objects in CoreCAD carry text-note `Notes`.

### Chief Library.json (catalog registry)

- 402 entries: `{"category", "name", "soft_deleted", "uuid"}`.
- Category counts: 1 = 11 (matches the 11 Core `.calib` files), 2 = 111 (Manufacturer folder has 112 files), 4 = 280 (Bonus folder has 279 files), plus 3 soft-deleted entries.
- Verified by sample: CoreCAD, CoreArchitectural = category 1; Blum, CustomWoodProductsLink = category 2; BonusTables = category 4. Each entry's `uuid` equals that catalog's `GlobalData.DatabaseUniqueId` (4 of 4 checked). `User_Library.calib` and `Trash.calib` are not in the registry.

## 4. How the fields map (what the samples show)

- **Name:** `Tags4LibraryObjects.Alias` holds the object's display name. Blum: "39C355B.20 (screw-on)", "20K4A00A02 Narrow aluminum door mounting plate". CoreArchitectural: "Marble Fireplace 5 - 12ft", "Door E29". `LibraryObjects.Legacy_Name` is empty in X18 samples.
- **Category:** the `Tags` tree (Parent to TagUniqueId). Root rows have `Parent` NULL and `GroupType` 2, and the root UUID equals `GlobalData.DatabaseUniqueId`. Verified parent links: BonusTables "Utility Tables", "Side Tables" and "Vanities" all have parent "Tables No.1". Blum tags include "Blum", "Hinge Systems", "CLIP", "Lift Systems" and "AVENTOS HS top", but the full Blum hierarchy was not traced.
- **Keywords:** full-text labels, many duplicated per object.
- **Per-object parameters:** the Data record holds name/value string pairs (for example "description" followed by "%automatic_description%", "Brass", "Metal", "Nickel Polished" for a Blum hinge). The exact parameter schema is not decoded.
- **Copyright:** `Copyrights` (for example "Copyright© 2024, Blum®"). Also stored inside Data records.
- **Size and dimensions:** not decoded. They are probably in the Data or AssociatedData records. Verify before relying on them.
- **Price, vendor SKU:** not seen as separate columns. Part numbers are in Alias and in the Data strings.
- **Textures:** the `LinkTable` blob embeds the original path (for example `C:/Productivity/P4/content/Main/Textures/Fabric/Woven/Wicker Chocolate.jpg`, or `...TemplateTextures.zip#zip:BrushedMetalBump_CAAB.jpg`). Resolve by basename against `Referenced Files/`, or against the `.calibz` entry list.

## 5. Binary structures inside BLOBs

### 5.1 Chief CDAB record (`Data4LibraryObjects.Data`, `SymbolData`, record stream in `AssociatedData`)

- Bytes 0-1: `CD AB` (magic 0xABCD, little-endian). Bytes 2-3: 16-bit LE kind. Seen values: `0x0061`, `0x0072`, `0x007B`, `0x0015`, `0x0140`, `0x0042`.
- Then a header whose width depends on the kind. Observed: CoreCAD kind 0x0061 has a u32 length at offset 6 and the string at offset 10. BonusTables kind 0x0072 has `FF FF FF FF` at offset 4 and a u32 length at offset 8.
- Strings are u32 LE length-prefixed, 8-bit, using MacRoman-style `©` (0xA9). Verified copyright strings: "Copyright© 2006, Chief Architect, Inc." (38 bytes) and "Copyright© 2005, Chief Architect, Inc.".
- Sizes: Data records are 0.5 KB to 10 KB each. Blum records are about 1.8 KB.
- Not yet decoded: the full field map per kind.

### 5.2 Symbol geometry (`LibrarySymbolData`)

- `symDxf` (BonusTables, 48 rows, 1.8 MB largest): begins `BF 05 00 00 00 00 00 FF FF FF FF` or `01 06 00 00 00 00 00 FF FF FF FF`. The `FF FF FF FF` looks like a -1 sentinel. Contains ASCII AIA-style layer names ("A-NONE-NONE-0", "A-FIXT-MAIN-0", "A-EQPM-HDWR-0", "A-WDWK-MOLD-0"), so the blob is a DXF-equivalent entity stream in binary. Coordinate encoding is not confirmed (the bytes look like IEEE-754 floats; byte order to be verified).
- `symBlock` (same 7-byte preamble): followed by a u32 LE length and a name string. Examples: "Wine...", "Servi...", "Basket", "Storage", "Vinta...". So `symBlock` holds the block or symbol name.
- `twoDRep`, `drawInfo`, `lightData`: NULL in every sampled catalog. The 2D plan representation is either `symDxf`/`symBlock` or lives elsewhere. Unknown.
- `LibrarySymbolData.isSymElement`: 1 for BonusTables symbols, 0 for Blum objects (Blum has no symbol geometry at all).

### 5.3 AssociatedData envelope (the bulk payload)

- Observed by row: CoreArchitectural leading 4 bytes `01 00 00 00` (1,339 rows), `00 00 00 00` (592), `02 00 00 00` (16). Blum: `00 00 00 00` (751), `01 00 00 00` (18).
- Verified: on 131 CoreArchitectural blobs, bytes 0-11 are zero (or a small flag), a u32 LE JSON length sits at offset 12, and UTF-8 JSON text starts at offset 16. All 131 parsed. On 751 Blum blobs, the byte at offset 16 is `{`. Observed JSON: `{"Macros": [ ... ], "Version": 4}` (CoreArchitectural) and `{"Version": 3}` (Blum). The "Macros" array is the parametric macro list for the object (134 Blum blobs contain a "Macros" key within their first 4 KB; not checked for emptiness).
- After the JSON: a binary stream. It contains `01 00 00 00` markers and nested `CD AB` records (for example `CDAB 61 00 B8 0C 00 00` in an 8.2 MB Blum blob). This stream is probably the per-object geometry or parameter data. Not decoded.
- zlib: no zlib header (0x78 0x01/0x5E/0x9C/0xDA) at the start of any Blum blob. Not checked deeper into the blob.
- Blobs can be large (average about 455 KB in CoreArchitectural, largest 3.3 MB). They span SQLite overflow pages.

### 5.4 LinkTable (per object)

- Starts `3D 0F 00 00 00 00 00 00` (699 Blum rows) or `3D 0F 00 00 01 00 00 00` (70 Blum rows). Then embedded path strings for textures and materials. Blum references about 140 `.jpg` paths; BonusTables about 25.

### 5.5 Thumbnail

- PNG (`89 50 4E 47 0D 0A 1A 0A`) for every sampled object. Blum thumbnails total 7 MB.

### 5.6 Content/ and Referenced Files/

- `Referenced Files/` files keep their original names (examples: `Gravel.jpg`, `(10A)ESSENTIAL_WO_MELTEDCHOCOLATE_PANEL2_72dpi.jpg`, `sunflowers_SergejMajboroda_4k.hdr`, `PanelInsulationNORMALDC20.png`). LinkTable paths resolve by basename.
- `Content/` holds 23,056 hash-named files. Samples: JPEG 256x256 (Photoshop CS2 EXIF) and PNG 1024x1024 RGB. Name is 64 hex plus `-01`. The name is not the SHA-256 of the file (one sample checked: name prefix `00016cea...`, SHA-256 `13665a0f...`). No 64-hex token in the sampled catalogs matches a `Content/` name (0 of 8 tokens across BonusTables, Blum and CustomWoodProductsLink). Resolution mechanism not identified (candidates: Chief's managed-resource index or `Data/Managed Resources`). Unresolved.

## 6. Parsing plan for a dependency-free Rust reader

Use `std` only: `std::fs::File`, `std::os::unix::fs::FileExt::read_exact_at`, and no mmap crate. The files are up to 4.3 GB, so never read a whole catalog into memory.

1. **SQLite read-only layer (minimal).**
   - Parse the 100-byte header: page size at offset 16 (`0x0001` means 65536), journal mode bytes 18-19 (reject WAL `2` for now), and reserved-space byte 20.
   - Walk `sqlite_master` on page 1. Support table B-tree leaf (0x0D) and interior (0x05) pages. Index pages (0x02, 0x0A) are needed only for indexed lookups.
   - Decode records: varint header, serial types 0 (NULL), 1-6 (ints), 7 (f64), 8/9 (constants), even >= 12 (BLOB), odd >= 13 (TEXT).
   - Follow overflow page chains for large BLOBs (the 4-byte first-overflow pointer at the end of the cell).
   - Ignore the freelist. Skip `sqlite_stat1`.
   - Refuse to read if a hot `-journal` file exists beside the catalog.
2. **Catalog layer.** Read `GlobalData`, `Tags`, `Tags4LibraryObjects`, `LibraryObjects`, `Keywords`, `Keywords4LibraryObjects`, `Copyrights`, `LibrarySymbolData`. Join on `LibraryObjects.UniqueId` and `Tags.TagUniqueId`. Build the category tree from `Parent`.
3. **Chief BLOB decoders.**
   - Names: take `Tags4LibraryObjects.Alias` first, then scan Data records for u32 length-prefixed strings. This is the quickest win.
   - Categories: from the `Tags` tree.
   - AssociatedData: read the u32 at offset 12, then the JSON from offset 16 (write a small JSON scanner, not a full parser, or use a hand-written parser). Then walk `CD AB` records.
   - Symbol geometry: decode the `symDxf` entity stream. Start by finding the float layout. Use a known object (a plain 2x4 or a box) to calibrate.
   - LinkTable: scan for paths ending in `.jpg`, `.png`, `.hdr`, and split on `#zip:`.
4. **Zip layer (for `.calibz`).**
   - Read the EOCD record at the end, then the central directory, then local headers. Support method 0 (stored) and 8 (deflate). Add zip64 EOCD handling for safety.
   - Implement inflate (RFC 1951, about 300 lines). The same inflate also serves PNG (zlib, RFC 1950) if you need texture bytes decoded.
   - Extract only the single `.calib` entry, stream it to a temp file in the scratchpad, and open it with the SQLite layer. Never write textures to the repo.
5. **Images.** Keep textures as raw bytes for now. Decode PNG with the inflate layer later if a preview is needed. JPEG decoding is large, so defer it or use an external tool.
6. **Textures and materials.** Resolve LinkTable basenames against `Referenced Files/` and the `.calibz` entries. Leave `Content/` unresolved until its key is found.
7. **Tests and repo hygiene.**
   - Unit tests with small synthetic SQLite fixtures written by the test itself, not copied from Chief.
   - Integration tests that skip when the Chief path is missing.
   - Add `*.calib`, `*.calibz`, and `*.calib_error` to `.gitignore`. Never commit fixtures taken from the Chief install.

## 7. Open questions

- Full field map of the CDAB record for each kind (where the dimensions, price-like fields, and material IDs sit).
- Float encoding and byte order in `symDxf`, and the entity types it contains.
- What the second half of the AssociatedData stream holds (geometry, macro code, or parameters).
- What `LibraryObjects.Type` values mean (0, 2, 6, 7, 9, 11, 12, 14, 15, 32).
- How `Content/<hash>-01` is referenced (not from any catalog table or `Referenced Files/`).
- The meaning of registry category 3 (3 soft-deleted entries).

### 7.1 Decoded 2026-10-08 (plan-calib `decode`)

Answers to the first, second and fourth open questions above, found by differential
analysis on Core Interiors, Exteriors, Architectural and MEP. Full layouts with
offsets are in `crates/plan-calib/README.md`; this is the summary.

- **Size.** `Data4LibraryObjects.Data` holds the default size as `w d h` followed by
  three 1.0 scale factors, as f32 (`FF FF FF FF 01 w d h 1 1 1`, most Interiors and
  Exteriors, e.g. `Cubby Bench` 50 x 16 x 18) or f64 (`FF FF FF FF 03 0.5 w d h 1 1 1`,
  parametric objects, e.g. `Door E29` 38 x 1.375 x 79.875). Verified against the
  geometry bounds: 179 of 187 Interiors and 300 of 342 Exteriors within 3%. Present in
  897 of 970 Interiors, 1530 of 1947 Architectural and 344 of 378 MEP objects.
- **`symDxf` is not DXF.** 0 of 541 blobs parse as text or binary DXF. It is a list of 3D
  polygon faces: `u16 version, u32 FFFFFFFF, u32 face count`, then per face `u16 n,
  u16 flags, u32 aux, u16 tag, n x (f64 x, y, z)` and an FF-filled trailer that may
  carry a layer name. Coordinates are inches; the front of an object faces -Y.
  (The "AIA layer names" seen earlier are those trailer strings.)
- **Geometry elsewhere.** `AssociatedData` (after its JSON header) and
  `SymbolData4LibraryObjects` carry `CD AB 74 00` triangle meshes: `u32 N`, N x 48-byte
  vertices (x, y, z + 3 zero doubles), `u32 M`, M x 80-byte triangle records
  (`u32 a, b, c`, partner, two ids, padding). 588 of 588 sampled records validate.
  Core Architectural, MEP and most Interiors objects have only this; no catalog
  stores a separate 2D symbol (`twoDRep`, `drawInfo` stay NULL), so plan symbols are
  derived from the 3D geometry.
- **Not decoded:** the other geometry record kinds (`CD AB 30`, `1f`, `23`, `19d`,
  `403b`, `f530`), so about a quarter of the objects have partial meshes; parametric
  cabinet and window records; door swings; elevation; `symBlock`; `LibraryObjects.Type`.
- Plant records (Core Plants) hold a spread and a height after the anchor
  `00 A6 91 3C 00 00 00 00 00 00 F0 3F`; the assignment is inferred, not proven.

## 8. Working files

Probe scripts and extracted samples are in `/private/tmp/claude-501/-Users-danielsievers-Documents-Clauade-Code-Folder/cb8a3685-aa10-43d9-b6c5-fe60e024942e/scratchpad/calib-sniff/`. The only extracted sample left there is `symDxf_6.bin` and `symDxf_1.bin` (BonusTables geometry). Delete them when no longer needed, because they are licensed content.
