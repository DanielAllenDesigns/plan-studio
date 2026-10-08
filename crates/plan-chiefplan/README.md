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
| `bridge` | `seed_defaults(&inv, PlanDefaults) -> TemplateSeed` (`.defaults` is the seeded `PlanDefaults`), `to_json` |
| `redact` | client-string redaction |
| `lib.rs` | `build_inventory`, `scan_daniel_templates`, `write_inventory_json(dir, out)` |

```
cargo test -p plan-chiefplan
cargo test -p plan-chiefplan --release -- --ignored --nocapture   # real templates
cargo run -p plan-chiefplan --example inventory -- [dir] out.json # redacted inventory JSON
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

### Wall layer stacks (not decoded)

`find_wall_stack` looks for two or more consecutive, not-all-equal `f64` (then `f32`) values that are multiples of 1/32" and sum to a multiple of 1/16" between 2" and 30", in the 256 bytes after a wall type name that is not part of a pick list. Result on all 26 files: **no stacks found**. What the data shows:

- Wall type names occur in repeated pick lists (15 copies of the list in the default plan): each name is followed by `00 00` or `00 01` and the next name. No thickness data sits next to the names.
- Searching the whole default plan for f64 sequences `[0.5, 3.5, 0.5]`, `[0.5, 5.5, 0.5]`, `[0.5, 3.5]`, `[3.5, 0.5]`, `[5.5, 0.5]`, `[0.5, 4.0, 0.5]` finds none, so stacks are probably not stored as inch-valued f64 runs, or are keyed by index or GUID elsewhere.
- Material names (`Drywall`, `Sheathing Panels`, `Sand Finish - Eggshell`) appear in a separate materials table, not in wall records.

So `bridge` uses the fallback: a single main layer of `number in the name + 1.0"` (`Siding-6` is 7.0", `Frame-3 1/2` is 4.5", `8" CMU ...` is 9.0"), or 4.5" when the name has no number, with material `approximate`; those names are listed in `TemplateSeed.approximate_wall_types`. On the default plan this adds 102 wall types, all approximate.

## Bridge

`seed_defaults(&inv, base) -> TemplateSeed`:

- Wall types: every inventory name not already in `base.wall_types` is appended (existing entries untouched and first). Kind is guessed from the name (`Interior`, `Fire`, `Frame`, `Room Divider`... are interior).
- Layers: names not in `base.layers` are added with the colour and line weight from the `Working Layer Set` table (then `Floor Plan Dimensioned Layer Set`, then the first table), else black and 18. Names are compared with whitespace collapsed and case folded, so the template's `Walls,  Normal` does not duplicate `Walls, Normal`. On the default plan 342 layers are added.
- `layer_sets` (name plus `visible_layers` when the table decoded), `text_styles`, `dimension_sets`, `plan_views`, and `layout { sheet_size, pages, macros }`.
- `sheet_size`: the listed size whose numbers appear in the file name; `None` when the file name carries dimensions (`18x24`) and no listed size matches (true for the default layout, which does not list `ARCH C (18" x 24")`); the first listed size when the file name carries no dimensions.
- `seed_plan_defaults` returns just the `PlanDefaults`.

## Open questions

- Where wall layer stacks, text style fonts and sizes, and dimension default values live. Not found by name adjacency; likely index- or GUID-keyed records elsewhere in the body.
- Meaning of header `offsets[1]` and `offsets[2]`, of layer flag bits above bit 1, of the class byte and of the second colour.
- How the 13 unnamed layer-set tables are named (probably by a record that is not a plain `... Layer Set` string).
- Whether the display flag matches Chief's Layer Display Options. A one-set UI comparison (for example `Foundation Layer Set`) would settle it.
- Whether the 18 x 24 sheet is stored as numeric page setup; no string names it in the layout.
