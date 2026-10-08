# plan-chiefplan

Read-only reader for Chief Architect `.plan` and `.layout` template files, so Plan Studio can seed its own templates from the names (and, where decoded, the values) in the user's own Chief templates. `std` + `serde` + `plan-core` only; no unsafe, no writes.

## Licensing and handling rules

- Templates are the user's files and Chief's format. This crate **reads them in place** and never copies one. `.gitignore` excludes `*.plan` and `*.layout`.
- Unit tests use synthetic buffers built in the test. Every test that touches a real template is `#[ignore]` (`tests/real_templates.rs`).
- Output is names and numbers only. `redact` removes client-specific strings (project plan file names, network-volume paths, address-like text, ISO dates) and any resource path outside the stock Textures/Backdrops folders before an inventory is written.

## Modules

| Module | Role |
|---|---|
| `scan` | `scan(path) -> TemplateScan`: header, thumbnail PNG, every length-prefixed string with its offset, tail resource table, kind by extension and by content |
| `classify` | `classify(&scan) -> TemplateInventory`: suffix/keyword rules into 15 categories, de-duplicated with counts and first offsets |
| `values` | Phase B: per-layer colour, line weight, display and lock flags; wall stack search; `calibrate` across files |
| `decode` | Phase C: the object stream. `TemplateSummary` with wall types (layer stacks), text styles, rich text defaults, dimension defaults, materials, default heights, paper sizes, layout info. Filled into `TemplateInventory::summary` |
| `bridge` | `seed_defaults(&inv, PlanDefaults) -> TemplateSeed` (`.defaults` is the seeded `PlanDefaults`), `to_json`; `wall_type_defs`, `text_styles_from_template`, `dimension_defaults_from_template` |
| `redact` | client-string redaction |
| `lib.rs` | `build_inventory`, `scan_daniel_templates`, `write_inventory_json(dir, out)` |

```
cargo test -p plan-chiefplan
cargo test -p plan-chiefplan --release -- --ignored --nocapture   # real templates
cargo run -p plan-chiefplan --example inventory -- [dir] out.json # redacted inventory JSON
cargo run --release -p plan-chiefplan --example summary -- [file] [--json]  # Phase C decode of one template
```

## Phase A: what the scanner relies on

- Header (64 bytes, little-endian): magic `01 CA 1A 10`; `u64` at 0x06 = 0x36 (offset of the second magic); `u64` at 0x0E, 0x16, 0x1E, 0x26; `u64` at 0x2E (= 1); second magic at 0x36; `u32` at 0x3C = PNG length. Thumbnail PNG at 0x40. Offsets: `[0]` = start of the resource table (verified on all 26 files: `Root Folder` string follows), `[3]` = file size - 1, `[1]` and `[2]` not decoded.
- Body: tagged stream of `u32` fields and `u32`-length-prefixed 8-bit strings. A string is accepted when the length is 1..=512, every byte is printable ASCII (plus MacRoman `(c)`, `(R)`, `TM`, degree) and at least one byte is alphanumeric. Names may carry stray spaces (`" Working Layer Set"`, `"DWG EXPORT Layer Set "`, `"ANSI B  (11" x 17")"`): classification trims ends, and sheet names also collapse inner double spaces.
- Kind: a `Page Template` / `Layout Text Style` / `Layout Dimensions` string marks a layout, `... Plan View` strings mark a plan. Content and extension agree on all 26 files.
- Resource table: strings at or after `offsets[0]` that contain `/` or end in an image/plan extension.

Real-file results (default templates, run 2026-10-08):

| | plan `x17 Working Template 2025-08-20.plan` | layout `18x24 PRESENTATION LAYOUT TEMPLATE.layout` |
|---|---|---|
| strings walked / distinct | 39,846 / 1,967 | 3,077 / 837 |
| layer sets | 34 | 6 |
| layers | 356 | 180 |
| text styles | 12 | 7 |
| dimension default sets | 14 | 0 |
| wall types | 108 | 24 |
| plan views | 20 | 0 |
| resources | 98 | 2 |

All 26 templates scan; summary in `docs/daniel-template-inventory.md`.

Known classification limits: `misc` is a fuzzy bucket (fonts, copyright lines, material names, leftovers); `Fir-4` style names are accepted as wall types because the template has them; `... Note` and `... Schedule` both go to `schedules`; layout page entries include `Layout Box Labels` even though the plan also uses it as a layer name.

## Phase B: what could and could not be decoded

Method: for each layer name, compare the bytes around it across the 47 layer-set copies in the default plan, then across all 26 files, and grade each field by agreement. `same-set` is P(equal) for the same layer in the same-named set of two different files; `other-set` is the same for two differently named sets of one file. A field that is a real per-set setting has same-set high and other-set clearly lower.

### Layer record frame (decoded)

`o` = offset of the name's length prefix, `e` = `o + 4 + len`.

| Where | Meaning | Confidence |
|---|---|---|
| `o-8..o-6` | `E0 3F` (tail of an f64 0.5), the frame anchor | frame |
| `o-6` | flags byte: bit 0 = display, bit 1 = lock | display Medium, lock Low |
| `o-5` | per-layer constant byte, values 0..3, meaning unknown | Low |
| `o-4..o` | `i32` layer id: -140..-1 (140 layers) and 0..214 (214 layers) | frame |
| `e` | `00`; `e+4` = `FF` | frame |
| `e+1..e+4` | RGB colour (alpha `FF` at `e+4`) | High |
| `e+5..e+7` | line weight, `u16` LE, 1/100 mm (values 0..50) | High |
| `e+9`, `e+10..e+26` | `01` then a 16-byte GUID (same GUID for a layer in every set) | not used |
| `e+84..e+88` | a second RGBA value; equals the colour for about 70% of records, `FFFFFFFF` on label layers | Low (meaning unknown) |

Every layer set stores its own full copy of the table (354 records in the default plan: 140 with negative ids, 214 with ids 0 and up), so colour, weight and flags are per set. The default plan has 47 tables; 34 got a name from the nearest `... Layer Set` string in the 60 KB before the table, 13 are reported as `(unnamed set N)`. `Reference Display Layer Set` was identified by its lock bits (304 of 354 locked, versus 0 to 26 in every other set).

Cross-file calibration (26 files, 219 sets, 56,842 records, all matched the frame):

| Field | same-set | other-set | Confidence |
|---|---|---|---|
| colour (RGB) | 0.948 | 0.700 | High |
| line weight | 0.972 | 0.933 | High |
| display flag | 0.958 | 0.697 | Medium |
| lock flag | 0.998 | 0.922 | Low |
| class byte | 0.802 | 1.000 | Low |
| second colour | 0.955 | 0.720 | Low |

Why colour is High: stable across files for the same set, differs between sets, and the values make sense (dimension and text layers are navy `(0,0,128)`, revision clouds red `(185,0,0)`, retaining walls grey). Why display is only Medium: it behaves like a per-set setting, and sets such as Plot Plan show terrain data while Roof Plan hides walls, but several sets show layers a person would not expect (for example `Windows` hidden in `Floor Plan Dimensioned Layer Set`), and nothing here was compared with Chief's Layer Display Options dialog. Lock is Low: it is one bit set on almost every layer of the reference set and a handful elsewhere. Bridge behaviour: new layers always start visible and unlocked; the display flags only feed `LayerSetSeed.visible_layers`.

Not decoded: line style, the meaning of `o-5` and the second colour, anything past `e+26` except the second colour, per-layer text style.

### Wall layer stacks (Phase B: not found; Phase C: decoded)

`find_wall_stack` looks for adjacent `f64` thicknesses after a wall type name and finds nothing, which is why Phase B called stacks undecoded: the thickness of each layer sits in its own 518-byte record, so no run of adjacent values exists. Phase C reads the records (below). `find_wall_stack` stays for completeness; `bridge` prefers the Phase C stacks, then a Phase B stack, then the name-number guess.

## Phase C: typed objects

`decode::summarize_bytes` (and `plan_chiefplan::summarize(path)`) scan the object stream `[01] CD AB <class> <version> <u32 size> <payload>` and read the classes below. Formats, offsets and confidence are in `docs/chief-template-format.md` section 7.

| What | Class | x17 plan | Default layout | Confidence |
|---|---|---|---|---|
| Wall types: name, layers (material, thickness, main, framing), total | 215 | 103 (no name repeats) | 24 | High |
| Materials: id, name, colour | 57 | 550 (146 named) | 112 | High (id, name), Medium (colour) |
| Text styles: name, font, style, height, bold, GUID | 139 | 15 | 7 | High; colour not stored here |
| Rich text defaults: font, size, text and background colour | 64 | 15 | 1 | High |
| Dimension defaults: arrow, extension, separation, number format, text style | 129 | 14 | 1 | High for the 1/4" set, Medium elsewhere |
| Default ceiling height (room types) | 23 | 109.125 in 53 of 53 | none | Medium |
| Sheet size | strings | `ARCH C (18" x 24")`, printable 23.833 x 17.833 | `ANSI B (11" x 17")` only | High |
| Layout: page template, printer, logo JPEG, macros, project fields | strings | n/a | see below | Medium |

Sample values: `Stucco-6` is `Sand Finish - Eggshell` 1.125", `Housewrap` 0.01", `OSB-Hrz` 0.5", `Fir Stud 16" OC, Teal` 5.5" (main), `Drywall` 0.5", 7.635" in total (Chief shows 7 5/8"). `Interior-4` is 4.5". Text styles are Avenir Book; `1/4" Text Style` is 4.5 plan inches, `1/8"` 9, `1"` 1.125, `Room Label Style` Avenir Heavy 8. The `1/4" Scale Dimension Defaults` set holds arrow 2.25", extension 3", 1st line offset 32", line separation 18", smallest fraction 1/8, fraction text 60%: the same numbers as the UI capture in `plan-core`.

What did **not** decode, and why:

- **Floor, foundation, rough ceiling and stem wall heights**: the object holding per-floor defaults was not identified. The only height found is the room type ceiling height.
- **Roof and floor finish materials**: room types and roof defaults do not refer to materials by id or GUID in a form we found.
- **Arrow style, text colour, leader style**: not present in the objects read, or no stable pattern.
- **Layout pages, boxes, box scales, title block positions, and the 18 x 24 sheet**: the layout template stores no page or box objects we could identify (one `Page Template` object, an embedded logo JPEG, the shared styles); 18 x 24 appears only in the file name, so `LayoutInfo::sheet_from_file_name` supplies it and `LayoutInfo::page_count` is `Some(0)` with Low confidence.

Decoded numbers disagree with two built-ins in `plan-core`: the stock Chief templates (and `plan-core`'s `TextStyles::chief_defaults`) use 6" `Chief Blueprint`/Arial for `Default Text Style`, Daniel's x17 template stores 4.5" Avenir. `seed_text_styles` keeps names Plan Studio already ships (so `Default Text Style` stays 6"); the decoded values are in `TemplateSummary::text_styles` and `bridge::text_styles_from_template`.

## Bridge

`seed_defaults(&inv, base) -> TemplateSeed`:

- Wall types: every decoded wall type not already in `base.wall_types` is appended with its real layer stack (`bridge::wall_type_def`: layer name and material are the material name, `is_main` from the main flag, exterior face first). Then every inventory name still missing gets the Phase B stack or the name-number guess (`approximate_wall_types`; on Daniel's plan only 7 names remain: pick-list entries such as `Siding-6, Copy` that have no definition). Existing entries stay untouched and first. Kind is guessed from the name (`Interior`, `Fire`, `Frame`, `Room Divider`... are interior). `TemplateSeed::decoded_wall_types` lists the decoded ones.
- Layers: names not in `base.layers` are added with the colour and line weight from the `Working Layer Set` table (then `Floor Plan Dimensioned Layer Set`, then the first table), else black and 18. Names are compared with whitespace collapsed and case folded, so the template's `Walls,  Normal` does not duplicate `Walls, Normal`. On the default plan 342 layers are added.
- `layer_sets` (name plus `visible_layers` when the table decoded), `text_styles`, `dimension_sets`, `plan_views`, and `layout { sheet_size, pages, macros, sheet_dimensions_in, printable_in }`.
- Text styles (`text_style_defs`): every decoded style, not only the names Phase A found (15 on the x17 plan). Names Plan Studio already ships keep their values; others take the decoded font, weight and plan height.
- Dimension sets (`dimension_set_defs`): the decoded numbers (smallest fraction, fraction text size, arrow size, extension gap and reach, 1st line offset, line separation) laid over the base settings, for the 14 sets; `decoded_dimension_sets` lists them. `default_height_in` carries the room-type height (109.125).
- `sheet_dimensions_in`: the decoded paper size of `sheet_size`, else the `NNxMM` in the file name (`(18.0, 24.0)` for the default layout).
- `sheet_size`: the listed size whose numbers appear in the file name; `None` when the file name carries dimensions (`18x24`) and no listed size matches (true for the default layout, which does not list `ARCH C (18" x 24")`); the first listed size when the file name carries no dimensions.
- `seed_plan_defaults` returns just the `PlanDefaults`.

## TODO(plan-core)

Gaps found while bridging (plan-core is read-only for this crate, so the bridge works around them):

- `TODO(plan-core)`: `WallLayer::new` is private; `bridge::wall_type_def` fills the public fields directly.
- `TODO(plan-core)`: `TextStyle` has only `plan_sized` (Arial, black); the bridge sets `font` and `italic` afterwards. There is no field for the font style string (`Book`/`Heavy`; only `bold` is kept) or a way to hold the 12 unknown flag bytes.
- `TODO(plan-core)`: `DimensionDefaults` has no slot for the dimension text style name, extension length towards the object, fixed proximity, baseline separation, reach, exterior reach, decimal places or arrow style. They stay in `TemplateDimensionDefaults` and are not applied.
- `TODO(plan-core)`: no type for layout sheets (sheet size in inches, printable area, margins) or rich text defaults; they stay in `LayoutSeed` and `TemplateSummary`.

## Open questions

- Where per-floor defaults live (floor-to-floor, foundation, rough ceiling, stem wall heights) and what the second room-type `f64` (23.25, 83.375, 84.875, 4.0) means.
- Text colour, arrow style, leader style; the 12 text style flag bytes.
- Layout pages, boxes, scales and title block geometry (none found in the default layout template).
- Meaning of header `offsets[1]` and `offsets[2]`, of layer flag bits above bit 1, of the class byte and of the second colour.
- How the 13 unnamed layer-set tables are named (probably by a record that is not a plain `... Layer Set` string).
- Whether the display flag matches Chief's Layer Display Options. A one-set UI comparison (for example `Foundation Layer Set`) would settle it.
