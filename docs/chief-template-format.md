# Chief Architect template files: format survey (.plan and .layout)

Scope: the default plan template (`x17 Working Template 2025-08-20.plan`) and the default layout template (`18x24 PRESENTATION LAYOUT TEMPLATE.layout`), plus an inventory of every other `.plan` and `.layout` in the Templates folder.

Method: read-only. Templates were inspected in place with `file`, `xxd`, `stat`, `unzip -l`, `strings`, `grep`, and short Python scripts run with `-I`. The scripts lived in the session scratchpad, not in this repo. No template file was copied into the repo. No Chief UI was driven.

Client-identifying text found inside a layout's leftover path or title strings has been redacted here as `[client project]`.

---

## 1. Files

Folder: `~/Documents/Chief Architect Premier X18 Data/Templates/`

Dates below are file mtimes (2026-08-09 17:09 for top-level files, 17:00 for the `Layout Borders` subfolder).

### Target files

| File | Bytes | Role |
|---|---|---|
| `x17 Working Template 2025-08-20.plan` | 9,460,333 (0x905A6D) | Default plan template |
| `18x24 PRESENTATION LAYOUT TEMPLATE.layout` | 2,306,730 (0x2332AA) | Default layout template |

### Other .plan files (top level)

| File | Bytes |
|---|---|
| x15 Working Template 2023-03-03.plan | 8,309,694 |
| Residential Template.plan | 6,364,339 |
| Interior Template.plan | 6,039,922 |
| Residential Template - Metric.plan | 5,452,028 |
| Commercial Template.plan | 5,447,696 |
| Interior Template - Metric.plan | 5,118,266 |

### Other .layout files (top level)

| File | Bytes |
|---|---|
| A1.layout | 636,367 |
| A2.layout | 633,912 |
| A3.layout | 633,309 |
| Arch B.layout | 610,042 |
| Arch C.layout | 616,008 |
| Arch D.layout | 688,656 |
| ArchD 24x36 Layout Template.layout | 600,609 |
| ISO1A 594x841 Layout Template.layout | 645,190 |
| Letter.layout | 579,528 |
| Tabloid Layout Template.layout | 590,665 |
| Tabloid Layout Template - Metric.layout | 635,991 |

### Layout Borders subfolder (nested .layout files, mtime 2026-08-09 17:00)

- `Layout Borders/Metric Layouts/`: A1 (635,604), A2 (633,182), A3 (632,573)
- `Layout Borders/Imperial Layouts/`: Arch B (601,681), Arch C (605,386), Arch D (650,056), Letter (579,369)

Totals: 7 .plan (including the target), 19 .layout (including the target and 7 nested). Not a .plan or .layout, but noted: `TemplateTextures.zip` (46 MB) sits in the same folder and is a real ZIP.

---

## 2. Format verdict

**Neither file is a standard container.** Both use a Chief-proprietary binary object stream. Neither is SQLite, ZIP, gzip, or a zlib-wrapped body.

- `file(1)` reports the `.plan` as "data" (correct: no known magic). It reports the `.layout` as "DIY-Thermocam raw data (Lepton 3.x)". That is a false match on random bytes. Ignore it.
- No `SQLite format 3` string in either file (count 0).
- `unzip -l` fails on both ("End-of-central-directory signature not found"). No `PK\x03\x04` signature found.
- gzip-like `1f 8b` byte pairs occur (the first five offsets were checked in each file), but none decompresses as gzip. They are random byte matches.
- Scanning for zlib headers (`78 01/5E/9C/DA`) found 1,382 candidates in the `.plan` and 30 in the `.layout`. Exactly one decompresses cleanly in the `.plan`, and it is the embedded PNG thumbnail's IDAT stream (offset 0x7E, 133,806 bytes uncompressed). No stream in the `.layout` decompresses.

Header bytes, both files (identical structure):

```
00000000: 01 ca 1a 10 00 00 36 00 00 00 00 00 00 00 ed 23   magic, then 0x36 (54) at 0x06
00000010: 90 00 00 00 00 00 23 24 90 00 00 00 00 00 49 36   u64 LE fields start at 0x0E
00000020: 00 00 00 00 00 00 6c 5a 90 00 00 00 00 00 01 00
00000030: 00 00 00 00 00 00 01 ca 1a 10 00 00 ce 0e 00 00   magic repeats at 0x36
00000040: 89 50 4e 47 0d 0a 1a 0a ...                       PNG thumbnail begins
```

Decoded header (little-endian):

| Field | `.plan` | `.layout` |
|---|---|---|
| magic at 0x00 and 0x36 | `01 CA 1A 10` | `01 CA 1A 10` |
| u64 at 0x0E | 0x9023ED | 0x23313C |
| u64 at 0x16 | 0x902423 | 0x233172 |
| u64 at 0x1E | 0x3649 | 0x137 |
| u64 at 0x26 | 0x905A6C (= file size - 1) | 0x2332A9 (= file size - 1) |
| thumbnail | PNG at 0x40, 3,790 bytes | PNG at 0x40, 8,665 bytes |

Reading: the four u64 fields look like offsets into the file. The last one always equals EOF minus one. In the `.plan`, two offsets (0x9023ED, 0x902423) land immediately before the resource table near EOF (the table starts at 0x90244F). This is a hypothesis from pattern alone. The offsets are not decoded further.

Trailer, both files: the tail is a table of length-prefixed resource paths (texture and backdrop files under `.../Chief Architect Premier X18 Data/Textures/` and `.../Backdrops/`), each followed by small binary records. The `.layout` tail includes a reference to a `.plan` file, a logo path on a network volume, and a texture path. The `.plan` tail includes about 109 path records, mostly texture and normal/roughness maps.

---

## 3. Container structure (working model)

1. **Header (0x00 to 0x3F)**: magic `01 CA 1A 10`, 0x36 at 0x06, offset-like u64 fields at 0x0E to 0x2E, magic repeated at 0x36.
2. **Thumbnail (0x40 to end of IEND)**: a standard PNG. The `.plan` is 3,790 bytes; the `.layout` is 8,665 bytes. Extractable with a simple PNG-chunk walk, and viewable directly.
3. **Body**: a tagged stream of little-endian `uint32` fields and **u32 length-prefixed 8-bit strings**. The string convention matches the Chief library format documented in `docs/chief-library-format.md` (section on strings, MacRoman `©` 0xA9). Evidence: "Elevation View Layer Set" (24 chars) is preceded by `18 00 00 00`, and "Reference Display Layer Set" (27 chars) by `1B 00 00 00`.
4. **Tail**: resource path table (see above).

Stats from the scanner (ASCII runs of 6+ characters after the thumbnail, length-prefixed printable records). These include a lot of binary noise:

| Metric | `.plan` | `.layout` |
|---|---|---|
| ASCII runs >=6 (total / distinct) | 40,606 / 2,718 | 3,875 / 1,393 |
| Length-prefixed printable records (total / distinct) | 37,153 / 1,873 | 2,726 / 754 |
| Path-like records | 109 | 5 |
| Compressed streams that decompress (excluding PNG) | 0 | 0 |

The name categories below are the reliable part of the output. Most of the rest is noise.

---

## 4. Recovered readable content

Counts are distinct names in each category, after removing binary noise. Counts for `.plan` / `.layout`.

### 4.1 Layer sets

`.plan` (26): 3D Framing, Camera View, DWG EXPORT, Electrical, Elevation View, Floor Plan Dimensioned, Floor Plan Shell, Foundation, Framing, Framing Layer Set Main Level, Framing Porch, "Framing, Ceiling", "Framing, Roof", HVAC, Kitchen & Bath Elevation, Kitchen & Bath, Plot Plan, Presentation Elevation View, Presentation, Reference Display, Roof Plan, Section View, Square Footage, Steel Framing, Window Schedule, Working. (Each name ends in "Layer Set".)

`.layout` (6): Camera View, Elevation View, Floor Plan Dimensioned, Presentation, Reference Display, Square Footage.

### 4.2 Layers (named layers)

Recognizable layer names (`.plan`; a few hundred candidate names from a noisy grep, sample below):
- Walls: "Walls, Default Fill Color" (47 occurrences), "Walls, Attic", "Walls, Foundation", "Walls, Invisible", "Walls, No Locate", "Walls, Normal", "Walls, Parapet", "Walls, Pool Walls"
- Terrain: "Terrain, Elevation Data" (47 occurrences)
- Dimensions: "Dimensions, Plan" (47), "Dimensions, Framing - Floor", "Dimensions, Framing - Roof", "Dimensions, Kitchen & Bath", "Dimensions, Electrical", "Dimensions, HVAC", "Dimensions, Structural Steel", "Dimensions, Roofs", "Old Dimensions, Legacy", "Rooms, Interior Dimensions"
- Cameras: "Cameras, Elevations", "Cameras, Elevations Windows", "Cameras, Cross Sections", "Cameras, Perspective Overview", "Cameras, Wall Elevation", "Cameras, Orthographic", "Cameras, Labels"
- Cabinets: "Cabinets, Base", "Cabinets, Full", "Cabinets, Wall", "Cabinets, Soffits", "Cabinets, Doors & Drawers", "Cabinets, Countertops", "Cabinets, Custom Backsplashes", "Cabinets, Module Lines"
- Electrical: "Electrical Box", "Electrical Breaker Panel", "Electrical Callout Defaults", "Electrical Dimension Defaults", "Electrical Marker Defaults", "Electrical Panel", "Electrical Plan View", "ELECTRICAL - DATA - AUDIO LEGEND"
- CAD: "CAD, Default", "CAD, Electrical", "CAD, Framing", "CAD, Framing 2", "CAD, Kitchen & Bath", "CAD, Plot Plan", "CAD, Roofs", "CAD, Structural Steel", "CAD, Retaining Walls", "CAD, Pool Outlines", "CAD, Landscape Pavers", "CAD, GIS IMAGES"
- Other: "Deck Railing/Fence", "Interior Railing", "Stairs & Ramps, Details", "Stairs & Ramps, Stringers"

`.layout` (~124 layer-style names): adds "Framing, Floor Joists", "Framing, Roof Trusses", "Framing, Headers", "Framing, Posts", "Framing, Sill Plates", "Framing, Rim Joists", "Framing, Floor Blocking", "Roof Eave Subfascia", "Roof Gable Subfascia", "Roof Ridge", "Roof Planes", "Deck Beam", "Deck Post", "Floor Surfaces", "Terrain Perimeter", "Terrain Labels", "Text, Callouts", "Text, Markers", "Text, Notes", "Fixtures, Interior", "Fixtures, Exterior", "Furniture, Interior", "Walls, Hatching", "Walls, Layers", "Walls, Main Layer Only", "Walls, Thru Wall Lines".

Note: "Walls, Default Fill Color", "Terrain, Elevation Data", and the "Dimensions, *" family each recur about 47 times in the `.plan`, at a fairly regular spacing of roughly 85 KB. That suggests one repeated per-view or per-layer-set block rather than one-off entries. Not confirmed.

### 4.3 Saved plan views and cameras

`.plan` (36): Working Plan View, Floor Plan View Dimensioned, Floor Plan View Shell, Foundation Plan View (and "...Dimensioned"), Framing, Floor / Ceiling / Roof / Porch / Porch Roof / Steel Columns-review Plan View, HVAC Plan View, Kitchen and Bath Plan View, Electrical Plan View, Plot Plan View, Roof Plan View, Presentation Plan View, DWG Export Plan View, Square Footage View, Structural Steel Plan View, "WINDWOS/ DOORS PLAN VIEW", Camera 1.

`.layout` (4): "Cameras, Inactive", "Cameras, Labels", and the Camera and Elevation View layer sets.

### 4.4 Text styles and rich text defaults

`.plan` (27): `1/8" Text Style`, `1/4" Text Style`, `1/2" Text Style`, `1" Text Style`, `3/16" Text Style`, `Default Text Style`, `Default Label Style`, `Room Label Style`, `Window Label Style`, `Schedule Style`, `SF Plan Text Style`, `Plot Plan Text Style`, plus matching `Rich Text Defaults` for each scale (`1/8"`, `1/4"`, `1/2"`, `1"` Scale), plus "Foundation", "Framing (incl. Ceiling and Roof)", "Electrical", "HVAC", "Kitchen and Bath", "NKBA", "Plot Plan", "Roof", "Structural Steel" Rich Text Defaults.

`.layout` (8): `Default Text Style`, `Layout Text Style`, `Dimension Text Style`, `Callout Text Style`, `Marker Text Style`, `CAD Text Style`, `Room Label Style`, `1/4" Scale Rich Text Defaults`.

Fonts seen: Arial (103 hits in the `.plan`, 44 in the `.layout`), "Arial Narrow" (`.layout` only).

### 4.5 Dimension default sets

`.plan` (40 dimension-related names): scale sets `1" Scale Dimension Defaults`, `1/2" Scale Dimension Defaults`, `1/4" Scale Dimension Defaults`, `1/8" Scale Dimension Defaults`; discipline sets `Electrical Dimension Defaults`, `Foundation Dimension Defaults`, `Framing Dimension Defaults`, `HVAC Dimension Defaults`, `Kitchen and Bath Dimension Defaults`, `Legacy NKBA Dimension Defaults`, `NKBA Dimension Defaults`, `Plot Plan Dimension Defaults`, `Roof Dimension Defaults`, `Structural Steel Dimension Defaults`; plus layer-level dimension entries ("Dimensions, Framing 2", "Dimensions, Legends Wall", "Dimensions, Automatic Foundation", "Dimensions, DETAILS", "Dimensions, Legacy").

`.layout` (7 matches, mixed with layer-set names): "Dimensions", "Dimensions, Legacy", "Layout Dimensions", "Dimension Text Style", "Floor Plan Dimensioned Layer Set", "Rooms, Interior Dimensions", "Old Dimensions, Legacy".

### 4.6 Wall types

`.plan` (about 130 names): siding and stucco variants, e.g. `Siding-4`, `Siding-6`, `Siding-8`, `Siding-11`, `Siding-6, Board & Batten Grey`, `Siding-6, Cedar Shake White`, `Siding-6, B&B Charcoal`, `Siding-6, Log`, `Siding-6, SHAKE`, `Siding-6, teal`, `Siding-4 Continuous Insulation`, `Stucco-4`, `Stucco-6`, `ICF-Stucco`, `ICF-Siding`, `ICF-Stone front`, `Brick-4`, `Brick-6`, `Existing Brick-4`, `Existing Siding-4`, `Fire-4`, `Fire-6`, `Frame-3 1/2`, `Frame-5 1/2`, `Interior-4`, `Interior-6`, `Interior-8`, `Interior-11`, `Footing-16`, `Footing-22`, `Footing-24`, `Footing-48`, `8" CMU (block) Stem Wall`, `8" CMU (block) Stem Wall, Stucco`, `Foundation stone-6`, `Room Divider`, `Interior Railing`, `Fire Rated Drywall`, `Log Siding`.

`.layout` (about 50): `Brick-4`, `Brick-4, No Brick Ledge`, `Brick-6`, `Concrete Block 1`, `Fire-4`, `Fire-6`, `Footing-16`, `Foundation Wall-16`, `Foundation 8" CMU (block) Stem Wall`, `Foundation 8" Concrete Stem Wall`, `Foundation 8" Concrete Stem Wall 4" Brick Ledge`, `Frame-3 1/2`, `Frame-5 1/2`, `Glass Wall`, `Interior Railing`, `Interior-4`, `Interior-6`, `Room Divider`, `Siding-4`, `Siding-6`, `Stucco-4`, `Stucco-6`, `Wall-4`.

Most common wall name in `.plan`: `Siding-6` (179 hits).

### 4.7 Room types and room labels

`.plan` and `.layout` both list the usual residential room labels (25 distinct): Attic, Bath, Master Bath, Bedroom, Master Bedroom, Bonus Room, Closet, Walk-In Closet, Dining, Dining Room, Dressing Room, Family Room, Game Room, Garage, Great Room, Hall, Kitchen, Laundry, Laundry Room, Living, Living Room, Office, Porch, Storage Room, Utility Room.

Note: these look like label defaults, not a separate room-type table. Room type objects were not identified in the binary.

### 4.8 Schedules, notes, and material list categories

Schedules (`.plan`, about 20 incl. notes): Window Schedule, Door Schedule, Cabinet Schedule, Custom Schedule, Electrical Schedule, Fixture Schedule, Framing Schedule, Furniture Schedule, Note Schedule, Plant Schedule, Room Finish Schedule, Wall Schedule, plus the Note families (Bath, Electrical, Framing, General, HVAC, Kitchen, Plot Plan, Roof).

Material list categories (`.plan`, 16): 2100 Footings and foundation, 2105 Rebar and reinforcing steel, 2110 Concrete block, 2200 Waterproofing, 2300 Termite protection, 3140 Trusses, 3150 Miscellaneous lumber, 3300 Windows, 3350 Skylights, 3910 Gutters and downspouts, 4100 Roofing material, 4400 Insulation, 5200 Interior trim material, 5610 Hardware, 5720 Electrical fixtures, 6600 Landscaping. (`.layout`: only "6600 - Landscaping".)

### 4.9 Layout sheets, page setup, and title block

- Sheet / paper names: `ARCH C (18" x 24")` (`.plan`), `ANSI B (11" x 17")` (`.layout`), `US Letter` (`.layout`). A printer name, `EPSON_WF_7610_Series`, also appears in the `.layout`.
- Page structure: `Page Template`, `Layout Box Borders`, `Layout Box Labels`, `Layout Box Export Contents`, `Layout Dimensions`, `Layout Text Style`.
- Line styles listed in the `.layout`: ISO dash, dot, and long-dash families (about 20).
- Title block fields and macros (`.layout`): `Floor Finish - %floor.name%`, `Ceiling Finish - %floor.name%`, `Rough Ceiling - %floor.name%`, `Top of Subfloor - %floor.name%`, `%room.name%`, `%simple_schedule_number%`, `%automatic_description%`, `=material_data.source_description + ' - ' + material_data.description`, `=material_data.formatted_size`, `=material_data.quantity`, `=material_data.manufacturer`.
- Copyright line: `Chief Architect Premier X18`, copyright 2026 Chief Architect, Inc. (`.plan`, `.layout`).
- Project reference left in the layout: a `.plan` filename and a view name for a client project (redacted here). A logo path on a network volume is also present (`/Users/danielsievers/.CMVolumes/...`). Treat this layout as carrying client-related strings if it is shared.

### 4.10 Macros and formula strings (plan)

Material list formulas, e.g. `=formatted_size + " ir=" + owner.arch_inside_radius_string`, `="handle: " + interior_handle_description`, `="no. " + horizontal_rebar_size.to_s`, `=has_footing ? 0 : material_data.quantity`. These are Chief's expression language for material list columns. They look like plain ASCII, so they are easy to recover.

---

## 5. Recommended approach

### Verdict

Direct parsing is feasible for **names and their positions**. It is not yet feasible for **setting values** (for example, whether "Elevation View Layer Set" hides the foundation layer, or what text size "1/4" Text Style" uses). No record schema is documented, and the record layout is not decoded.

### Phase A: name inventory (recommended now, low risk)

Write a read-only Rust scanner (no unsafe, no writes) that:
- Walks the body and returns every `u32 LE length + 8-bit string` record with its offset, using the same rule as the library-format doc.
- Filters into the categories in section 4 (layer sets, layers, text styles, dimension defaults, wall types, plan views, schedules, material categories, page names, macros).
- Runs over all 26 `.plan` and `.layout` files, producing a JSON inventory per file.

Expected value: the names above are complete enough to validate a future reader against Chief's UI lists.

### Phase B: value decoding by diffing existing templates (recommended next, no UI needed)

There are 7 `.plan` and 19 `.layout` files. Several share the same named layer set, text style, or dimension default (for example, "Elevation View Layer Set" and "Dimensions, Plan" appear in multiple files). Diffing the bytes of a record block between two templates that differ only in one setting is a read-only way to find the field offsets and types. Start with one layer set and one text style, since those names are the most repeated.

This is more reliable than guessing the schema from strings alone, and it does not require Chief to be running.

### Phase C: UI capture (only if Phase B cannot resolve a value)

Chief's dialogs, in the order that would be needed to capture the same settings by hand (names taken from `docs/chief-x18-dialogs.md` and `docs/chief-x18-menus.md`):

1. **Layer Settings > Active Layer Display Options**: one capture per layer set (26 in the `.plan`). Record which layers are on, off, or frozen in each.
2. **Default Settings > Text** (Text Style Defaults): `Default Text Style`, the `1/8"` through `1"` text styles, and the per-layer-set Rich Text Defaults. Record font, size, and color for each.
3. **Default Settings > Dimension > Dimensions** (Saved Dimension Defaults list, then one "Dimension Defaults - <name>" dialog per entry): the `1/8"`, `1/4"`, `1/2"`, `1"` scale sets and the NKBA, Framing, Foundation, and Electrical sets.
4. **Plan View dialog** (Plan View..., or saved plan views list): one capture per saved view listed in section 4.3, with the layer set it uses.
5. **Default Settings > Layout**, plus the Layout page setup dialog: page size and `Page Template` for each sheet (ARCH C 18x24, ANSI B 11x17, US Letter), plus the `Layout Text Style` and `Layout Dimensions` defaults.
6. **Title block**: the `Layout Box Labels` boxes on the layout and the macro insert list used by each. Record which macro strings appear in which box.
7. **Walls > Exterior Wall Defaults** (and the interior equivalent), per wall type name in section 4.6. The exterior dialog is already partly captured in `docs/chief-x18-dialogs.md`.
8. **Default Settings > Default Sets**: the list of named default sets.

Note: the project's dialog notes say the Text, Layout, Plan, and Schedules dialogs have not been captured yet ("Still to capture", `docs/chief-x18-dialogs.md`). Those captures would need to be done first.

### Summary

- Phase A (names): do now. The scanner is small and read-only, and the output is verifiable against the UI lists above.
- Phase B (values by diffing templates): do next, using existing files. No UI capture needed.
- Phase C (UI capture): only for values Phase B cannot resolve. Requires Daniel to run Chief and open the dialogs listed above.

---

## 6. Open questions

- Exact field layout of layer records and text style records (requires Phase B).
- Meaning of the four header offset fields. The hypothesis is an index into the tail resource table, but this is not confirmed.
- Whether the X18 application writes the same body format as the x17 template (the template is dated 2025-08-20, and the app is X18). Confirm by diffing against a plan saved from X18 once one exists.
- Whether the `Walls, Default Fill Color` and `Dimensions, *` repeated blocks correspond to saved plan views or to layer sets. Confirm with Phase B.
