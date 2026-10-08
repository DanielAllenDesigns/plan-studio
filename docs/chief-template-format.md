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

Answered by section 7 (Phase C): the field layout of wall type records and text style records; where wall layer stacks live; why the stack search of Phase B found nothing (the thickness values sit 518 bytes apart, one per layer record, so no run of adjacent `f64`s exists).

Still open:

- Meaning of the four header offset fields. The hypothesis is an index into the tail resource table, but this is not confirmed (`offsets[0]` is the resource table start and `offsets[3]` is EOF - 1; `[1]` and `[2]` are not decoded).
- Whether the X18 application writes the same body format as the x17 template (the template is dated 2025-08-20, and the app is X18). Confirm by diffing against a plan saved from X18 once one exists.
- Whether the `Walls, Default Fill Color` and `Dimensions, *` repeated blocks correspond to saved plan views or to layer sets. Confirm with Phase B.
- The unknowns listed at the end of section 7.

---

---

## 7. Phase C: the object stream (typed decoding)

Implemented in `crates/plan-chiefplan/src/decode.rs`; run with `cargo run --release -p plan-chiefplan --example summary -- <file>` (add `--json` for the whole summary). Numbers below are from `x17 Working Template 2025-08-20.plan` unless a layout is named. Confidence words: **High** = value matches an independent check (Chief's dialog, the UI capture in `plan-core`, or an obvious invariant such as a sum), **Medium** = consistent across all samples but the role is inferred, **Low** = pattern only.

### 7.1 Objects

Most template content is a stream of objects. Marker and header:

```
[01] CD AB <class u8> <version u8> <size u32 LE> <payload ...>
```

`size` counts its own four bytes, so an object spans `[size_pos, size_pos + size)` and the next sibling starts right there (verified on the first `Drywall` material: `size` 0x244 ends exactly at the next `01 CD AB 39`). The byte before `CD` is 1 for most top-level objects and other values when the object is nested. Objects nest (a wall type sits inside a wrapper, a material list component inside a room type), so scanning for `CD AB` and bounds-checking `size` gives a flat list that is good enough. A few `CD AB` byte pairs are chance matches with absurd sizes (for example class 169 with size 3.4 billion) and are dropped by the bounds check.

Strings inside payloads: `u32` length, bytes, **one NUL**, so `08 00 00 00 "Stucco-6" 00`. Doubles are `f64` LE, unaligned. Lengths are inches. 16-byte blobs are GUIDs.

Counts (plausible objects): x17 plan 4,578; default layout 810. Classes the decoders read (class, version, count in the x17 plan / default layout):

| Class | Version | Meaning | Plan | Layout |
|---|---|---|---|---|
| 57 (0x39) | 0 | material | 550 | 112 |
| 215 (0xD7) | 0 | wall type | 103 | 24 |
| 139 (0x8B) | 0 | text style | 15 | 7 |
| 129 (0x81) | 0 | dimension default set | 14 | 1 |
| 64 (0x40) | 1 | rich text defaults (plus 2 later copies with other values) | 15 | 1 |
| 23 (0x17) | 0 | room type | 54 | 28 |

All 26 templates decode without a failure (`phase_c_every_template_summarizes`): wall types 22 to 104 per file, text styles 7 to 15.

### 7.2 Materials (class 57) - High for id and name, Medium for colour

Inside the payload, after a copyright string, a category, a GUID and a name, comes `[id u16][u16][R G B][FF 88 13 00 00]`. `88 13 00 00` (5000) is the same in every sample, which makes `FF 88 13 00 00` a reliable anchor: `id` is the `u16` 7 bytes before it, the colour the 3 bytes before it, and the name is the string that ends one NUL before the id. 550 materials in the plan, 550 distinct ids, 146 with a name (the rest are unnamed colour-palette swatches). Every id used by a wall layer resolves. Samples: 35 `Drywall`, 46 `Fire Rated Drywall`, 156 `Sand Finish - Eggshell`, 165 `Fir Stud 16" OC`, 166 `Housewrap`, 167 `OSB-Hrz`, 168 `Fir Stud 16" OC, Yellow` (the 2x4 stud), 169 `Fir Stud 16" OC, Teal` (the 2x6 stud), 171 `Concrete`, 172 `Lap Siding`, 354 `Timber Bark`. Daniel colour-codes studs by size: yellow for 3 1/2", teal for 5 1/2".

### 7.3 Wall types (class 215) - High

Payload: `<name string>` (with its NUL), then the record count `N` (`u32`), then `N` records of **518 bytes**. The last record is a sentinel (thickness 0, material 35); the first `N - 1` are the layers, exterior face first. 103 wall types; none repeats a name. Method: the three siding types `Siding-4/-6/-8` have byte-identical bodies apart from GUIDs and one `f64` (3.5, 5.5, 7.49) at the stud layer; correlating that `f64` with the number in the name found the thickness, and the same offset gave plausible values for all 103 types.

Layer record (offsets from the record start):

| Offset | Field | Confidence |
|---|---|---|
| +0x00 | thickness, `f64`, inches | High (3.5 / 5.5 for 2x4 / 2x6 studs, 0.5 drywall, 0.01 housewrap) |
| +0x08 | material id, `u32` | High (resolves in the material table for every layer) |
| +0x0C | `i32`: 1, -1, -65535, 10, ... | unknown |
| +0x10 | `u32`: 0 or 1 | unknown |
| +0x15 | main layer flag, `u8` | High: set on exactly the stud layer of `Stucco-6`, `Siding-6`, `Brick-6`, `Interior-4`, `Interior-6`; on the concrete or CMU core of foundation types; on all three structural layers of `SIP` and both studs of `Interior-4, Double` |
| +0x16 | framing flag, `u8` | Medium: set on stud layers of exterior types and on `1 1/2"` furring studs; clear on `Interior-4` studs |
| +0x17 | `u8` set on `Insulation  Air Gap`, `Opening (no material)`, `Room Divider` layers | Low |
| +0x18 | `f64` module/spacing: 16 or 24 on studs, 96 on sheet goods, 7 or 8 on siding and brick, 16 on CMU, 0 on stucco | Medium |
| +0x2A | 16-byte GUID of the layer (it also appears again in the material list data of the same wall wrapper) | Medium |
| +0x3B, +0x43 | two `f64` 4.0 in every layer | unknown |
| +0x12E.. | pairs of 144.0 / 96.0 and 9.25 / 1.5 / 5.5 (texture and framing sizes) | unknown |

Validation: `Stucco-6` = 1.125 + 0.01 + 0.5 + 5.5 + 0.5 = **7.635"**, which Chief displays as 7 5/8" (the `plan-core` capture holds 7.625"). `Siding-6` = 7.01", `Interior-4` = 4.5", `Frame-3 1/2` = 3.5", `Footing-16` = 16", `8" CMU (block) Stem Wall` = 7.625". `Stucco-6` layers, exterior to interior: `Sand Finish - Eggshell` 1.125, `Housewrap` 0.01, `OSB-Hrz` 0.5, `Fir Stud 16" OC, Teal` 5.5 (main), `Drywall` 0.5.

The 108 wall type *names* of Phase A include five names that have no definition object: `Siding-6, Copy`, `Interior-6, Copy`, `Demolition-6, Copy`, `12" CMU (block) Stem Wall, Copy` and the material names `Fire Rated Drywall` and `Log Siding` (and the stray `stone-6,`). They come from pick lists. The default layout's 24 wall types decode the same way but use older material names (`sheetrock`, `fir stud 16" OC`, `stucco`) and a different stack for `Stucco-6` (stucco 1.125, sheathing 0.625, stud 5.5, sheetrock 0.5 = 7.75").

### 7.4 Text styles (class 139) - High for font, style, height and name

Payload: `height f64` | `font string` | `font style string` | 12 flag bytes | `name string` | `u8` | 16-byte GUID (the last 16 bytes of the object). 15 styles in the x17 plan (Phase A names found 12; `Room Label Style, Small`, `Square Footage` and `Room Label Style 2` were missed by its suffix rules):

| Name | Font | Style | Height (plan inches) |
|---|---|---|---|
| `Default Label Style` | Avenir | Book | 2.25 |
| `Room Label Style` | Avenir | Heavy | 8 |
| `1/4" Text Style` | Avenir | Book | 4.5 |
| `1/2" Text Style` | Avenir | Book | 2.25 |
| `1/8" Text Style` | Avenir | Book | 9 |
| `1" Text Style` | Avenir | Book | 1.125 |
| `3/16" Text Style` | Avenir | Book | 3.375 |
| `Default Text Style` | Avenir | Book | 4.5 |
| `Plot Plan Text Style` / `SF Plan Text Style` | Avenir | Book | 18 |
| `Schedule Style` | Avenir | Book | 4.5 |
| `Window Label Style` | Avenir | Book | 3 |
| `Room Label Style, Small` | Avenir | Heavy | 6 |
| `Room Label Style 2` | Avenir | Heavy | 8 |
| `Square Footage` | Avenir | Book | 16 |

The height is the plan size that prints 3/32" tall at the style's scale (4.5" at 1/4" scale, 9" at 1/8", 1.125" at 1"). `Heavy` is bold (weight byte 0x11 against 0x10). The other flag bytes vary (0 or 1 at flag +3 and +4) with no pattern against the names: unknown (transparent background or fill are guesses). Italic and underline never occur. **Text colour is not in this object** (None). The stock templates differ: `Residential Template.plan` has `Chief Blueprint` 6" for `Default Text Style` and `1/4" Text Style`. The x15 working template has the same 15 styles as x17.

Layout (7 styles): `Layout Text Style` Avenir Heavy 0.25, `CAD Text Style` Arial 0.25, `Dimension Text Style` Arial Narrow 0.125, `Callout Text Style` and `Marker Text Style` Chief Blueprint 0.125, `Default Text Style` Avenir Book 0.1875, `Room Label Style` Arial 0.125 (layout heights are paper inches).

### 7.5 Rich text defaults (class 64, version 1) - High

Settings are stored as strings: name, font, optional style, size, text colour hex, background hex, a 32-digit flag string. Every set in the x17 plan: font Avenir Book (Chief Blueprint with no style for `Framing Rich Text Defaults, Ceiling` / `, Roof` and `Kitchen and Bath`), size `0.166666666666667` (1/6"), text colour `004080` (0, 64, 128), background `ffffff`. Exception: `Foundation Rich Text Defaults` is `000000`. 15 sets. Two later copies of `1/4" Scale Rich Text Defaults` (Avenir Heavy 1.5, `000000`) belong to another structure and are ignored by the decoder (first occurrence wins).

### 7.6 Dimension defaults (class 129) - High for the 1/4" set, Medium for the role names

Fields are located from two anchors: the end of the name (arrow size, number format) and the `Avenir`/`Arial` font string that follows the text height (everything else). With the `1/4" Scale` set (name length 29) the absolute payload offsets are given in parentheses.

| Field | Anchor and offset | 1/4" | 1/2" | 1/8" | 1" |
|---|---|---|---|---|---|
| arrow size (`f64`) | name end + 0x29 (0x4B) | 2.25 | 1.125 | 4.5 | 0.75 |
| decimal places (`u32`) | name end + 0x3B (0x5D) | 4 | 4 | 4 | 4 |
| smallest fraction (`u32`) | name end + 0x3F (0x61) | 8 | 8 | 8 | 16 |
| fraction text size % (`u32`) | name end + 0x9E (0xC0) | 60 | 60 | 60 | invalid |
| text height (`f64`) | font - 8 (0x1B9) | 4.5 | 2.25 | 9 | 1.125 |
| text style | 16-byte GUID at font + 0x1E, matched to a text style's GUID | `1/4" Text Style` | `1/2" Text Style` | `1/8" Text Style` | `1" Text Style` |
| extension: length away | font + 0x32 | 3 | 1.5 | 6 | 0.75 |
| extension: fixed gap | font + 0x3A | 3 | 1.5 | 6 | 0.75 |
| extension: length towards | font + 0x42 | 3 | 3 | 3 | 3 |
| fixed proximity | font + 0x4B | 12 | 12 | 12 | 12 |
| line separation | font + 0x54 | 18 | 12 | 24 | 6 |
| reach | font + 0x5C | 24 | 24 | 36 | 12 |
| baseline separation | font + 0x64 | 18 | 12 | 24 | 6 |
| 1st line offset | font + 0x6C | 32 | 18 | 42 | 9 |
| exterior reach | font + 0x1C4 (0x385) | 240 | 192 | 192 | 192 |

The 1/4" values equal Daniel's UI capture (`plan-core` `PlanDefaults`): smallest fraction 8, fraction text 60%, arrow 2.25", extension 3" / 3", 1st line offset 32", line separation 18", reach 240" (checked by a unit test and `phase_c_text_styles_and_dimension_defaults`). The scale sets scale with the drawing scale (a 3" extension at 1/4" is 1.5" at 1/2" and 6" at 1/8").

The 14 sets resolve their text style as: `Plot Plan` to `Plot Plan Text Style`; the four scale sets as above; `NKBA` and `Kitchen and Bath` to `1/2" Text Style`; `Foundation` to `Default Text Style`. `Electrical`, `Framing`, `HVAC`, `Roof`, `Structural Steel` and `Legacy NKBA` have no style match (older layout); their extension, offset and separation fields past the first few read as zeros and are returned as `None`. `NKBA`, `Kitchen and Bath` and `1"` have a format block of a different shape: the fraction text size is out of range there and is reported `None`. The set count is 14 (Phase A listed the same 14 names).

Not decoded: **arrow style** (the dialog shows `tick`; a `u32` near name end + 0x15 holds 8, 7, 12, 6, 15 across sets with no match to arrow size, so it is not used), leader style, text position, rounding method, unit labels beyond the strings `ft` and `in` (present in every set), layer. Layout: the single `Layout Dimensions` set reads arrow 0.125, decimal places 6, smallest fraction 16, fraction text 100%.

### 7.7 Default heights (class 23 room types) - Medium

Every full room type definition (53 of 54 in the x17 plan, 7 in each stock plan template) holds an `f64` multiple of 1/8" between 90 and 150 inches at 0x276 to 0x27D from the start of the size field (it moves with the name length). In Daniel's plan it is **109.125** (109 1/8") in all 53. The stock `Residential Template.plan`, `Interior Template.plan` and `x15 Working Template` hold **96.0**, so this is a ceiling-height default that Daniel changed. Nine bytes later the same object has a second `f64` whose value depends on the room type: 23.25 (33 types), 83.375 (13: decks and similar), 84.875 (4: slab and garage types), 4.0 (3: basement and crawl space). Role unknown.

**Not found**: floor-to-floor height, foundation height, rough ceiling, stem wall height, floor and ceiling structure thicknesses. The object that holds the per-floor defaults was not identified. 109.125 does not occur anywhere else as an `f64` (whole file: 55 hits, 53 in room types and 2 inside a bogus object), so no separate wall-height default carries it. `108.0` occurs 4 times, in unrelated objects.

### 7.8 Default materials and colours - partly found

* The material table (7.2) holds all material colours (`Drywall` (239, 233, 218) and so on).
* Default exterior wall type `Stucco-6` (named in `docs/daniel-chief-setup.md`): exterior finish `Sand Finish - Eggshell`, framing `Fir Stud 16" OC, Teal`, interior finish `Drywall`. Default interior wall type `Interior-4`: finish `Drywall`, framing `Fir Stud 16" OC, Yellow`. These come from the decoded wall stacks (`TemplateSummary::default_materials`).
* **Roof and floor finish names were not found.** Room types and roof defaults refer to materials some other way than the material id or GUID (the id 161 `Shasta White` and the GUIDs of `Shasta White`, `Foam Underlayment` and `Color - Bone` do not appear in the room type objects). Not decoded.

### 7.9 Sheet size and the layout template

* **Plan**: the sheet is stored in numbers just before the name. For `ARCH C  (18" x 24")` (the double space is in the file) the preceding bytes are, in order: `f64` 0.1667, `f64` 0.1667 (margins), `f64` 23.8333, `f64` 17.8333 (printable area, landscape), `f64` 18.0, `f64` 24.0 (paper, portrait), one NUL byte, then the name (offset 0x1483). High. The same shape exists in the x15, Residential, Commercial and Interior plans.
* **Layout** `18x24 PRESENTATION LAYOUT TEMPLATE.layout`: **the 18 x 24 is not stored as numbers anywhere.** The only sheet-size entry is `ANSI B  (11" x 17")` with paper `f64`s (11.0, 17.0) before it (same NUL-byte shape; offset 0x27B1), preceded by a `US Letter` entry (215.9 x 279.4 mm) and the printer `EPSON_WF_7610_Series`. 18 x 24 is only in the file name. Searched: no `f64` 18.0 followed by 24.0 (or 24.0 / 18.0, or as `f32`) outside material texture sizes.
* **Pages and boxes**: no layout page or box objects were identified. The file body is the shared object stream (materials, wall types, styles, defaults), the project-information field names, and, after a path string at 0x18F4D7, an embedded JPEG of 669,454 bytes (start 0x18F571, end 0x232C7F exclusive; the firm logo) followed by one small object named `Page Template` (class 0x19C) and the resource table. So the template holds **0 pages** with Low confidence that this is complete (the box objects may use a class this survey never saw) and **no layout box scales**. The four "layout page" names of Phase A (`Layout Dimensions`, `Layout Box Labels`, `Layout Box Borders`, `Page Template`) are styles and layer-like names, not pages.
* **Title block text**: the macros are plain strings (offsets in the layout): `Floor Finish - %floor.name%` (0x343FF), `Ceiling Finish - %floor.name%`, `Top of Subfloor - %floor.name%`, `Rough Ceiling - %floor.name%`, `%room.name%`, `%simple_schedule_number%` (0x60CD0). The project block is a list of field names from `Project` (0x975DC) through the client's `Country/Region`: Project, Project Name, Street, City, State/Province, Zip/Postal Code, Country/Region, APN, Property Zone, Occupancy Group, Construction Type, Designer (Name, Company Name, Phone Number 1/2, Cell Phone Number, Fax Number, Web Site, E-mail Address, address), Client (same fields). **Positions of text on a page are not stored** in this template.

### 7.10 Unknowns, in one place

* Text colour and the 12 text style flag bytes; arrow style; leader style; any dimension field not in the 7.6 table.
* Layer record: `i32` at +0x0C, `u32` at +0x10, the two `4.0` doubles at +0x3B/+0x43, everything after +0x12E.
* Floor, foundation, rough ceiling and stem wall heights; the second room-type `f64` (23.25 and friends); roof and floor finish materials.
* Layout page, box, scale and title block geometry; the `Page Template` object's contents.
* Material categories (the strings before the name), material textures (referenced by GUID in the tail table) and the 404 unnamed materials.

## 8. Project files (geometry): see `docs/chief-plan-format.md`

A project `.plan` is the same object stream with the template's objects at the front (materials, wall types, text styles, defaults) and the model after them. Phase D (`plan_chiefplan::import`) adds these classes, with offsets, evidence and confidence in `docs/chief-plan-format.md`:

| Class | Meaning | Confidence |
|---|---|---|
| 30 | floor (elevation at +0x278, ceiling at +0x281; X17: +0x1FC, +0x205) | High |
| 6 | wall; children 31 (line), 40 (arc line), 4 (point), 215 (wall type copy), 9, 10 (openings) | High |
| 9, 10 | door, window: centre, width, height, head height; door hinge and swing bytes | High / Medium |
| 23 (inside a floor) | room: type, own name, bounding box centre | Medium |
| 24, 25 | dimension string points; text note position and string | Medium |

Two corrections to this document found while reading projects: wall type layer records are **518 bytes in X18 files but 377 bytes in X17 files** (cumulative layer starts instead of thicknesses, section 4.2 of the plan notes), and a wall refers to its type by the serial id written before the class 215 object (`01 <u32 id> 00 00 00 00`), not by name or GUID. This also partly answers the open question of section 7.7: the template's own four class 30 floors hold Daniel's per-floor defaults as the triple (elevation, ceiling, third value): foundation (-125.875, 109.125, 111.625), first floor (0, 121.125, 111.625), second (137.875, 109.125, 114.5), third (252.5, 97.125, 23.25). Foundation, rough ceiling and stem wall heights as separate fields, floor names, door styles and dimension line offsets are still unknown.

