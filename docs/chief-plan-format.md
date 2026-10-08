# Chief Architect project files (.plan): geometry classes and the importer

Scope: how a Chief Architect X17/X18 project `.plan` stores its floors, walls, doors, windows, rooms, dimensions and text, and how `plan_chiefplan::import::import_plan` turns them into a Plan Studio `Project`. The template-level objects (wall types, materials, text styles, dimension defaults) are in `docs/chief-template-format.md` section 7; this document starts where that one stops.

Method: read-only, by differential analysis. Nothing from a Chief file is copied into the repository (tests build synthetic bytes; real-file tests are `#[ignore]` and skip when the file is absent). Ground truth came from the PDFs Daniel printed from the same projects: their vector line work gave wall outlines, door swing arcs, text positions and the door/window schedules. A translation vote (every `f64` pair of the plan against every wall corner of the PDF, 1.5 PDF points per plan inch at 1/4" scale) found the plan-to-sheet offset with 326 coinciding corners against at most 107 for any other offset, and the decoded geometry was then overlaid on the PDF outlines. Client project names stay out of this repository: the jobs are called A to D here (and in the tests, which read them from environment variables), and `redact` strips client names from anything else the crate writes.

Reference projects (Daniel's own Chief X17/X18 archives):

| Job | Gen. | Used for |
|---|---|---|
| **A**: a construction set and its 18-sheet PDF | X18 | main ground truth (4 floors, 228 walls, 34-window schedule, one curved wall) |
| **B**: another construction set | X18 | second house (212 walls) |
| **C**: a schematic-design file | X18 | off-grid sizes (303 walls) |
| **D**: a construction set | X17 | older generation (143 walls) |
| `x17 Working Template 2025-08-20.plan` | X18 | default template: four empty floors, no geometry |

Every archived project of 3 to 170 MB (191 files, both generations) was run through the importer: all 191 imported without an error or a panic; 171 of them hold 25 to 535 walls, the other 20 are site plans, templates and early design files with 0 to 9 walls.

## 1. File generations

The first four bytes tell the generation: `01 CA 1A 10` is an X18 file, `01 CA 7F 0F` an X17 file (X18's own Archives folder holds both: jobs last saved by X17 and the numbered `_NNNN` copies of migrated jobs keep the X17 format). The object stream (`[01] CD AB <class> <version> <u32 size> <payload>`, section 7.1 of the template notes) is the same in both. They differ in a few fixed offsets: the floor header (section 3), the wall type layer record (section 4.2), and rooms and dimensions, which this crate decodes for X18 only.

## 2. Objects, nesting, the base header

* `size` counts its own four bytes, so an object ends at `end = (marker + 4) + size`. All offsets below are **relative to the `CD` byte** (`marker`).
* Objects nest by byte span. A real object is always version 0 or 1; chance matches of `CD AB` inside textures and meshes carry versions such as 28, 77, 107 or 255 (all 26 of them in one 75 MB file). `import::tree::ObjectTree` keeps versions 0 and 1 and rejects an object that runs past its parent's end. A project has 40,000 to 90,000 objects.
* Many plan objects start with the same base header: `+8` 16-byte GUID, `+0x18` flag byte, `+0x19` `u32` object id, `+0x1D` `FE`, `+0x1E` `u32`, `+0x22` `FF FF 00 00 00 FF FF FF`, `+0x2A` `i32` **layer id** (the id of the layer table records of the template notes, section "Layer record frame"; `-55` is `Default`, meaning "the layer of the object's type"). Walls, floors, rooms and texts carry it; the small line and point objects have shorter headers.
* A child is preceded by a wrapper `01 <id u16> 00 00 00 00 00 00 ...`; a **shared** object (a wall type) by `01 <serial id u32> 00 00 00 00`. Objects that many parents use (wall types, materials) are written once, in full, at their first use, and referred to by that serial id afterwards (section 4.2).
* Classes 4 (point) and 31/40 (line) are tiny helpers used by many parents; class 120 (material list component, with strings such as `=material_data.formatted_size`) is the bulk of every plan (9,000 of 40,000 objects).

## 3. Class table

Confidence: **High** = checked against an independent source (PDF geometry, a schedule, an invariant); **Medium** = consistent across all samples, role inferred; **Low** = pattern only; **None** = counted, not understood.

| Class.ver | Meaning | Confidence | Imported as |
|---|---|---|---|
| 30.0 | **floor** (4 per project: foundation, first, second, attic) | High | `Floor` (elevation, ceiling) |
| 6.0 | **wall** | High | `Wall` |
| 31.0 | straight **line**: the wall's reference line (`x, y, dx, dy, length`) | High | wall start/end |
| 40.0 | **arc line**: line fields plus centre, radius, two angles | Medium (one wall, radius 24.27" matches the PDF) | `Wall::curve` |
| 4.0 | 2-D **point** (24 bytes: `f64 x, y` at +8/+16): wall corners and break points | Medium | (fallback for a wall without a line) |
| 215.0 | **wall type** (a full copy inside the first wall that uses it, 13 of the 228 walls of one project; the unused rest in a pool at the end) | High | `WallTypeDef`, thickness |
| 9.0 | **door** (child of a wall; also pass-throughs, niches) | High | `Opening` (Door) |
| 10.0 | **window** (child of a wall) | High | `Opening` (Window) |
| 23.0 | **room** when nested in a floor; **room type** definition outside floors (53 in the template) | High (type vs room), Medium (fields) | `RoomName` |
| 24.0 | **dimension** (linear strings; other shapes exist) | Medium | `Dimension` |
| 25.0 | **text** note | Medium | `CadItem::Text` |
| 48.0 | text label holding a room name (`kitchen`) or an area macro (`%area.round%`) | Medium | not imported (position not found) |
| 68.0 | framing member (`Wall Framing - Lumber`, `Rafters`) | High | not imported |
| 79.0 | wall framing assembly (owns class 68 members, `in`/`ft`/`mm` unit strings) | Medium | not imported |
| 120.1 | material list component | High | not imported |
| 81.1 | wall connection record (two object ids and a GUID, identical in the two joined walls) | Medium | not imported |
| 15.0 | **cabinet** (strings `Framed Panel`, `cabinet doors`) | Medium | not imported |
| 21.0 | electrical device (`Outlets`, `Wall Mounted`, `GFCI`) | Medium | not imported |
| 109.0 | countertop-like object (`5450 - Countertops`, layer `Cabinets, Base`) | Low | not imported |
| 18.1 | roof plane (only on the top floor, 114 in one project, `CA-001` molding, 12 on layer `Roofs, Ridge Caps`) | Low | not imported |
| 46.0 | railing (`Classic Rail`) | Low | not imported |
| 50.0, 52.0 | surface objects of the top floors | Low | not imported |
| 93.0, 122.0 | molding/trim sets inside walls and openings (`CA-001`, `CS-05`, `SOLDIER COURSE`) | Low | not imported |
| 54.0 | tag-like object inside walls (position and two sizes) | Low | not imported |
| 34.1, 36.1, 38.0, 41.0, 64.1, 73.1, 101.1, 104.1 | not identified (36.1 sits on `Framing, ...` layers) | None | counted |

Counts per class are printed by `cargo run --release -p plan-chiefplan --example object_survey -- <file>`; `ImportReport::skipped_classes` lists what a floor holds that nothing imported.

## 4. Decoded classes

### 4.1 Floors (class 30), High / Low

A class 30 object that is not nested in another class 30 is a floor, in file order bottom to top. The three values (each preceded by a flag byte 0/1) sit at the same offsets in every X18 file read (checked on four projects):

| Offset (X18 / X17) | Field | Examples |
|---|---|---|
| `+0x278` / `+0x1FC` | `f64` elevation relative to the first floor, inches | -132.25, 0, 137.875, 252.5 |
| `+0x281` / `+0x205` | `f64` ceiling height as Plan Studio's `ceiling_height` (ceiling + 1 1/8") | 115.5, 121.125, 109.125, 97.125 |
| `+0x28A` / `+0x20E` | `f64` not identified | 118, 118, 114.5, 23.25 |

Cross-checks: the second floor sits at 121.125 + 16.75 = 137.875, the foundation top at -16.75 (115.5 - 132.25), and a wall's stored top elevation on the second floor is 137.875 + 109.125 = 247.0. A slab-on-grade job stores -8, 8, 4 and 0, 145.125, 4, so values from 4" are accepted at the known offsets and from 24" in the blind search used for unknown generations.

**Floor names are not stored anywhere found** (the strings `1st Floor`/`2nd Floor` occur only inside elevation dimensions). The importer names floors by position: elevation under -1" is `Foundation`, the last floor holding roof planes is `Attic`, the rest `1st Floor`, `2nd Floor`... (Low). Trailing floors with nothing on them (the template always carries four) are dropped, always keeping the first floor above grade.

### 4.2 Wall types (class 215), High

X18 layout: section 7.3 of the template notes (518-byte layer records, thickness at +0, material id at +8, main flag at +0x15). X17 layout, found here: **377-byte records** and a 51-byte tail (`size = 8 + string + 4 + count * 377 + 51`); record `k` holds the layer's *start* (cumulative from the exterior face: 0, 4.0, 5.0, 5.01, 5.51, 11.01 for `Brick-6`) at +0, the material id at +8 and the main flag at +0x15; the last record holds only the total; a zero-thickness layer (material 88) may sit at either end and is dropped. Checked on every wall type of one X17 project (146, all decode).

**A wall refers to its type by serial id.** Outside its children, a wall holds `01 <u32 id> 00 x 12`, and the id is the serial id (`01 <id> 00 00 00 00` immediately before a class 215 `CD`). The first wall that uses a type owns a full copy of it; later walls only carry the id, and unused pool types follow at the end of the file. In one project all 63 first-floor walls and all 65 foundation walls resolve, and the resolved thicknesses agree with the PDF: `Interior-6` shows outline lines at +-3.25" (6.5" total), `Brick-6_2, dbl` at +-8.25" (16.52" total).

### 4.3 Walls (class 6), High (line, type) / Medium (height, reference)

Children: the line (class 31 or 40, first child at +0x169), points (class 4), the type copy (215), openings (9, 10), parts (68, 79, 93, 120, 122, 54, 81). Line fields at `+0x32 f64 x`, `+0x3A y`, `+0x42 dx`, `+0x4A dy`, `+0x52 length`; the wall runs from `(x, y)` to `(x + dx * length, y + dy * length)`. Coordinates are plan inches, y up, one origin per file (the same for every floor). Overlaying the 63 first-floor lines of one project on its PDF outlines puts every one on a wall.

**Reference line.** For a wall whose layers are symmetric about the main layer (`Interior-6`, `Brick-6_2, dbl`, stem walls) the line is the centre of the main layer, i.e. of the wall. For an exterior wall (`Brick-6`: brick 4, gap 1, housewrap 0.01, OSB 0.5, stud 5.5 main, drywall 0.5) the PDF shows the main layer spanning -5.25" to +0.25" of the line measured to the left of the wall direction, the drywall face at +0.78": the line is 0.25" inside the main layer's interior face, the exterior layers lie to the **right** of the direction in all 24 exterior walls measured on that project's first floor. The importer therefore sets `exterior_side = Right` and shifts the centreline by `(main/2 - 0.25) + (outside - inside)/2` to the right (5.005" for `Brick-6`), keeping the file's total thickness. This rule is measured on one wall type, so other asymmetric types may sit a fraction of an inch off (Medium/Low).

**Joins.** Chief joins walls at their reference lines, Plan Studio at centrelines, so right after the shift an interior wall stops 4.75 to 5" short of the exterior wall's centreline and two exterior walls meet 7" apart at a corner. `walls::heal_joins` moves wall ends along their own line (at most 12", never collapsing or flipping a wall) so that ends of non-parallel walls within 12" meet at the intersection of the centrelines and an end inside another wall's thickness reaches that wall's centreline, shifting the openings of a wall whose start moved. On the first floor of the reference project `detect_rooms` finds 17 rooms before and 19 after (tolerance 6") and 6 of its 7 named room anchors fall inside a detected room (5 of 7 before); at 12" all 7 do. The count of moved ends is `counts["wall_ends_joined"]`.

**Curved walls** have a class 40 line instead of 31: the chord fields at the same offsets, then `+0x60/+0x68` centre, `+0x70` radius, `+0x78/+0x80` the angles at the start and end points (radians: pi and pi/2 for a quarter circle). One wall seen (radius 24.27", chord 34.31"). The importer takes the minor arc and sets `WallCurve::bulge` from the centre side. No other curved wall exists in the 40 projects checked (87 to 543 walls each, no wall without a line), so major arcs and bay/bow walls are untested.

**Height.** The top of the wall is stored as an absolute elevation, twice in a row (start and end of a sloped top), 0x169+96 to ~0x500 into the wall: 121.125 on a 10' first floor, 139.125 for 12' garage walls, 247.0 on the second floor. The importer takes the largest equal pair, subtracts the floor elevation, and falls back to the floor's ceiling value when the pair is missing (foundation walls store none) or the result is under 24" or over 300" (attic floors store values below their own elevation).

**Class, layer, flags** come from the type name only (Medium): `Room Divider` is a room divider on `Walls, Invisible`, `Railing` and `Deck Railing/Fence` are railing classes, `Glass ...` (not `Glass Block`) is glass, stem-wall/footing/CMU/concrete types on the foundation floor are foundation walls on `Walls,  Foundation`. Chief stores no wall flag byte that was found.

### 4.4 Doors and windows (classes 9, 10), High (position, size) / Medium (flags) / Low (style)

An opening is a descendant of its host wall. A run of five `f64`s sits at an offset that varies by object variant (+0x312 and +0x3F5 in two X18 variants, +0x291 in an X17 one), so it is found by shape from +0x100 on:

```
A+0x00  distance from the wall start to the opening CENTRE, inches
A+0x08  width            A+0x12  depth (0.625 or 0)
A+0x1A  height           A+0x22  head height above the floor
```

Evidence: all 34 windows of one project's window schedule decode with the scheduled width and height (36x96 x9, 30x54 x2, 32x54 x5, 72x18, ...; two 48x60 windows read 46 and 45.5), door widths match the door schedule (30, 32, 34, 36, 72, 108, 120, 192, 216), and with the position read as the centre every one of the 32 first-floor openings tested falls in a gap of the PDF wall outlines (read as an edge, 20 of them overlap the wall by 0.6 to 1.0 of their width). In schematic-design files sizes are not on a grid (32.0851"), so only the depth is grid-checked. Sill height is head height minus height.

**Door flags**, from 25 doors whose swing arcs were found in the PDF (all four combinations present: 6 hinge-start/left, 6 start/right, 5 end/left, 8 end/right): the byte at `A+0x2A4` is the swing side (0 left of the wall direction, 1 right), the byte at `A+0x2FD` the hinge (0 at the wall start, 1 at the wall end). Each separates the 25 doors perfectly; Plan Studio's `swing_flipped` is "swings right" and `hinge_at_end` the hinge byte. Not checked on windows or X17 files; a byte other than 0/1 leaves the defaults.

**Style** is guessed (Low): the string `Garage` marks a garage door; otherwise width >= 150" slider, over 100" doorway, from 54" double, else hinged; a head higher than the height means a pass-through. A doorway and a hinged door both carry the string `Door P04`; the byte that distinguishes them was not found. Window kind (fixed, casement) was not found. A few percent of door/window objects (job C: 2 of 166; a large modern house: 13) use a variant without the run; they are counted in the report and skipped.

### 4.5 Rooms (class 23 in a floor), Medium, X18 only

```
+0x139        string  room TYPE name ("Default" for an ordinary room; "WINE CELLAR", "mud room")
 ...          16-byte GUID of the type, flag bytes
p-32          f64 f64  bounding box width, height of the room
p-16          f64 f64  bounding box centre x, y        <- Plan Studio anchor
p             byte k, 00 x 6, 80, 01, 80   (found by search after the type string)
p+0x121       string  the room's own name, when set ("F. Porch", "3 car Garage")
```

Earlier in the block come two areas in square inches (10,498.855 and 6,707.68 for a 72.9 sq ft porch whose box is 121.98 x 54.99); some rooms hold `-541265.15` twice instead. The centre of `F. Porch` (639.47, 511.36) is where the PDF prints its label. A room with type `Default` and no own name prints its label from a separate class 48 object (`kitchen`, `family rm`), whose position was not decoded (its anchor lines sit at y = 1052.96 for a label printed near y = 922), so those rooms import without a name. In one project 19 of 68 rooms have a name; the rest are listed in the warnings.

### 4.6 Dimensions (class 24), Medium (points) / Low (line)

A linear dimension string stores its points in 909-byte records: the first point appears at `+481` and again 217 bytes later (`+698`), each further point at `+698 + 909 k` (X18 offsets; the 217/909 spacing found by search, so the first offset may move by a few bytes). 159 of the point pairs of one project's first-floor dimensions coincide with PDF wall corners. About half of the class 24 objects have this shape; the rest (angular, radial, elevation, leader) are skipped and counted. **The dimension line position was not found** in any of the fields compared, so the importer offsets every string 36" away from the centre of the floor's walls (`dims::assumed_offset`), one `Dimension` per consecutive pair of points. Text overrides, dimension sets and arrow styles are not read.

### 4.7 Text (class 25), Medium (position) / Low (size)

`+246 f64` is the x of the horizontal centre and `+254 f64` the y of the **top** of the text (13 notes compared with the PDF's text boxes: x within 0.5", top within 1.2"); the strings follow: font (`Avenir`), style (`Book`/`Heavy`), the text. Size, rotation and box width were not found: Plan Studio gets 4.5" (Default Text Style) for regular and 8" (Room Label Style) for bold text, angle 0, and a bottom-left anchor estimated at 0.6 x height per character. `Wall Layer N - Viewed From Outside` objects are automatic labels and are skipped. Only notes that are not parts of walls, rooms, cabinets or devices are imported.

## 5. Importer behaviour (`plan_chiefplan::import`)

`import_plan(path, &ImportOptions) -> Result<ImportResult { project, report }>`:

1. `ObjectTree::build`, generation from the header, floors, wall types.
2. Project shell: `Project::from_defaults` over `PlanDefaults::default()` (Daniel's), seeded from the file's own layers, layer sets, text styles and wall types by `seed_defaults`/`apply_seed` (X18 only: the string-level scan needs the X18 header). The wall types the walls use replace same-named defaults with the file's definition (`bridge::wall_type_def`), so the registry and the wall thickness agree.
3. Floors, walls (with `set_class`), openings, `heal_joins`, room names, dimensions, text, in that order; ids come from `Project::alloc_id`.
4. `ImportReport`: `counts` (`floors walls curved_walls wall_ends_joined doors windows rooms rooms_named dimensions texts wall_types`), per-floor counts, `skipped_classes` (class, version, label, confidence, count) and `warnings` in plain language.

Measured on the real projects (release build, the file already in the OS cache): 0.3 s for a 75 MB file.

| File | Floors | Walls | Doors | Windows | Named rooms | Dims | Texts | Curved |
|---|---|---|---|---|---|---|---|---|
| A | 4 | 228 | 46 | 34 | 19 | 174 | 137 | 1 |
| B | 4 | 212 | 53 | 35 | 26 | 416 | 177 | - |
| C | 4 | 303 | 104 | 60 | 44 | 7 | 37 | - |
| D (X17) | 4 | 143 | 46 | 41 | 0 | 0 | 0 | - |
| `x17 Working Template 2025-08-20.plan` | 2 | 0 | 0 | 0 | 0 | 0 | 0 | - |

## 6. Not decoded, and why

* **Floor names**, the third floor header value, floor-specific wall defaults: no string or field found. Names are positional.
* **Door style** (doorway vs hinged), **window kind**, window swing: no byte pattern found; style comes from widths.
* **Dimension line offset, text, set**: not found (the object stores the points of a string; the line is probably recomputed).
* **Room labels** (class 48) and their link to rooms; **room polygons** (Plan Studio detects rooms from the walls and anchors the names).
* **Text size, rotation, alignment**; leader and callout objects.
* **Cabinets (class 15)**, placed symbols/library objects (the symbol GUID would be in the 116-byte library blobs), **roof planes (18.1)**, stairs, railings, electrical devices (21), framing (68, 79), moldings (93, 122): classes identified by strings and counts only; their position/size fields were not located. Cabinet records show width/depth/height candidates at +0x35A/+0x362/+0x36A (24, 45 or 27, 96) but no position.
* **X17 files**: walls, wall types, openings and floors decode everywhere; layers and layer sets are not read (the string scan needs the X18 header); dimensions (point records are 554 bytes apart there, not 909) and rooms (type string at +0x109, anchor bytes elsewhere) fail in job D (57 of 57 rooms), although a schematic-design job saved by X17 decodes its rooms.
* **Wall bottoms, sloped tops, wall-specific layer overrides, connection joins** (class 81 only counted).
* **Major arcs** and bay/bow walls (no sample).

## 7. Reproducing

```
cargo run --release -p plan-chiefplan --example import -- "House.plan" [--json] [--no-seed] [--project out.json]
cargo run --release -p plan-chiefplan --example object_survey -- "House.plan"
CHIEF_PLAN_A=... CHIEF_PLAN_B=... CHIEF_PLAN_C=... CHIEF_PLAN_X17=... \
  cargo test -p plan-chiefplan --release --test real_import -- --ignored --nocapture
```
