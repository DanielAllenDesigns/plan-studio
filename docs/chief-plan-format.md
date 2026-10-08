# Chief Architect project files (.plan): geometry classes and the importer

Scope: how a Chief Architect X17/X18 project `.plan` stores its floors, walls, doors, windows, rooms, dimensions, text, cabinets, library objects, electrical devices, stairs and roof planes, and how `plan_chiefplan::import::import_plan` turns them into a Plan Studio `Project`. The template-level objects (wall types, materials, text styles, dimension defaults) are in `docs/chief-template-format.md` section 7; this document starts where that one stops.

Method: read-only, by differential analysis. Nothing from a Chief file is copied into the repository (tests build synthetic bytes; real-file tests are `#[ignore]` and skip when the file is absent). Ground truth came from the PDFs Daniel printed from the same projects: their vector line work gave wall outlines, door swing arcs, text positions and the door/window schedules. A translation vote (every `f64` pair of the plan against every wall corner of the PDF, 1.5 PDF points per plan inch at 1/4" scale) found the plan-to-sheet offset with 326 coinciding corners against at most 107 for any other offset, and the decoded geometry was then overlaid on the PDF outlines. The object stage (sections 4.8 to 4.14) had no PDF to overlay; its evidence is internal: cabinets against the walls behind them, wall devices against the nearest wall, integer ratios (run / tread, rise / riser, pitch in quarter inches), a stair flight against the `STAIR WELL` room box, and every decoded record loaded into the real Plan Studio types. Client project names stay out of this repository: the jobs are called A to D here (and in the tests, which read them from environment variables), and `redact` strips client names from anything else the crate writes.

Reference projects (Daniel's own Chief X17/X18 archives):

| Job | Gen. | Used for |
|---|---|---|
| **A**: a construction set and its 18-sheet PDF | X18 | main ground truth (4 floors, 228 walls, 34-window schedule, one curved wall) |
| **B**: another construction set | X18 | second house (212 walls) |
| **C**: a schematic-design file | X18 | off-grid sizes (303 walls) |
| **D**: a construction set | X17 | older generation (143 walls) |
| **E**: a construction set, 10' ceilings (199 walls, 354 loose devices and 16 gang boxes, 37 roof planes) | X18 | stage 3: gang boxes, connection arcs, roof edges |
| **F**: a construction set (273 walls, 533 devices, 39 roof planes, a stair with four landings) | X18 | stage 3: stair stacking, roofs, connections |
| **G**: a construction set (a winder stair, 23 roof planes) | X18 | stage 3: preferred-riser flights |
| **H**: a schematic design (293 walls, 32 library objects, a U stair) | X18 | stage 3: catalog GUIDs, U stair |
| `x17 Working Template 2025-08-20.plan` | X18 | default template: four empty floors, no geometry |

Stage 3 (this round) went back to what stage 2 could not decode, using cross-file statistics over the saved files of the archive (10 to 14 files per question, 7 distinct projects; autosave copies and the 950 MB drawing were left out; no file was driven in Chief and none was copied): the catalog link of placed objects (section 4.10: it exists, in the class 114 entry), roof edge joins, eave overhangs and the eave edge (4.13), stair stacking heights (4.12), electrical gang boxes and connection arcs (4.11), corner cabinets (4.9) and the identity of classes 34, 35, 36, 38, 41, 64, 73, 101 and 104 (section 3). Section 4.15 lists every stage-3 field with its confidence, its unit test and its real-file test.

After the object stage, 128 of the archived projects (the first 128 of 200 listed, X16 to X18 saves) were imported again: all 128 without an error or a panic, 0.05 to 0.3 s each once the file is cached. Stage 1: every archived project of 3 to 170 MB (191 files, both generations) was run through the importer: all 191 imported without an error or a panic; 171 of them hold 25 to 535 walls, the other 20 are site plans, templates and early design files with 0 to 9 walls.

## 1. File generations

The first four bytes tell the generation: `01 CA 1A 10` is an X18 file, `01 CA 7F 0F` an X17 file (X18's own Archives folder holds both: jobs last saved by X17 and the numbered `_NNNN` copies of migrated jobs keep the X17 format). The object stream (`[01] CD AB <class> <version> <u32 size> <payload>`, section 7.1 of the template notes) is the same in both. They differ in a few fixed offsets: the floor header (section 3), the wall type layer record (section 4.2), and in the record sizes of dimensions (909, 554 and 534 bytes per point for X18, X17 and the older `01 CA E2 0E` files) and the roof pitch offset (+76, +75). Rooms decode the same way in X17 and X18.

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
| 48.0 | text label holding a typed room name (`kitchen`) or an area macro (`%area.round%`); its frame is a rectangle of line records | High (frame, text) | `RoomName` (typed names; section 4.14) |
| 68.0 | framing member (`Wall Framing - Lumber`, `Rafters`) | High | not imported |
| 79.0 | wall framing assembly (owns class 68 members, `in`/`ft`/`mm` unit strings) | Medium | not imported |
| 120.1 | material list component | High | not imported |
| 81.1 | wall connection record (two object ids and a GUID, identical in the two joined walls) | Medium | not imported |
| 15.0 | **cabinet / box**: base, wall and tall cabinets, shelves, soffits (section 4.9) | High (position, size, kind), Low (face layout) | `Floor.cabinets` (`plan_cabinets::Cabinet` JSON) |
| 21.0 | **electrical device** (`Duplex`, `Single Pole`, `Recessed Down Light 6`; section 4.11) | High (position, kind) | `Floor.electrical` (`ElectricalLayer` JSON) |
| 109.0 | countertop: a free-form top on a floor (island), or one of the 1,158-byte parts of a cabinet (two on a base cabinet = it has a top) | Medium | `Cabinet.countertop`, `CustomCountertop` |
| 18.1 | **molding polyline** (ridge caps, cornices, fascia runs; profile strings `Ogee`, `CA-001`, `Default Ridge Cap`; on the top floors, 114 in one project). Stage 1 read these as roof planes | Medium | not imported |
| 46.0 | **stair flight** (stringers, risers, newels, balusters, `Classic Rail`; section 4.12). Stage 1 read these as railings | Medium | `Floor.stairs` (`plan_stairs::Stair`, straight) |
| 47.0 | **stair landing** (`landing floor`, `joist_quantity`; a rectangle of four line records) | Medium | `Floor.stairs` (`Landing` shape) |
| 50.0 | **roof plane** (outline, baseline, pitch, per-edge flags; 15 to 56 KB; section 4.13) | Medium | `Floor.roofs` (plane record with the eave edge first) |
| 50.1 | part of a roof plane (the overhang ring and the footprint polygon) | Low | not imported |
| 123.0 | **library object** placed in the plan (toilet, range, bed, chair; section 4.10) | High (position, size, catalog GUID where an installed catalog holds it) | `PlacedSymbol` |
| 16.0 | furniture group (a bed with its nightstands): holds class 123 members | Medium | members only |
| 114.0 | library object entry (name, category tags, a GUID per placed copy **and the library item's UniqueId**); also the door and drawer style entries of a cabinet | High (item GUID), Medium (rest) | name, tags, catalog GUID |
| 52.0 | floor / ceiling surface objects (7 to 15 per floor) | Low | not imported |
| 93.0, 122.0 | molding/trim sets inside walls and openings (`CA-001`, `CS-05`, `SOLDIER COURSE`) | Low | not imported |
| 54.0 | tag-like object inside walls (position and two sizes) | Low | not imported |
| 34.1 | **electrical connection arc**: the dashed arc between two devices (a switch and its light, or light to light); two leg records then the arc sampled in 10" chords (section 4.11) | Medium | `ElectricalLayer.connections`, `Device.switched_by` |
| 150.0 | **electrical gang box**: two or three class 21 devices sharing a box (section 4.11) | Medium | its devices |
| 35.0, 36.1, 38.0 | **extent records** of 294 or 310 bytes: four `f64` at +246 (min and max pairs, e.g. 928.754, 928.754, 662.254, 662.254 on a 36.1); 36.1 sits under view/layout objects (class 80), 35.0 and 38.0 under framing assemblies | Low | counted |
| 41.0 | 678-byte record among the CAD lines (class 31 siblings); constant pen values 0.5, 5.0, 1.53; no plan coordinate found at any alignment | None | counted |
| 64.1, 73.1 | **text box** (64.1; strings such as `Avenir`, `Book`, `004080`, `0.1666...`) and its **text format record** (73.1; font name, style, colour as hex, size; one child per text box, 250 to 310 bytes) | Low (64), Medium (73) | counted |
| 101.1, 104.1 | 60- and 48-byte property records under material list components (class 120): all bytes after the header vary, nothing readable | None | counted |
| 7.1, 100.1, 102.1, 156.1, 67.0 | not identified | None | counted |

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

### 4.5 Rooms (class 23 in a floor), Medium, X18 and X17

```
+0x139        string  room TYPE name ("Default" for an ordinary room; "WINE CELLAR", "mud room")
 ...          16-byte GUID of the type, flag bytes
p-32          f64 f64  bounding box width, height of the room
p-16          f64 f64  bounding box centre x, y        <- Plan Studio anchor
p             byte k, 00 x 6, 80, 01, 80   (found by search after the type string)
p+0x121       string  the room's own name, when set ("F. Porch", "3 car Garage")
```

Earlier in the block come two areas in square inches (10,498.855 and 6,707.68 for a 72.9 sq ft porch whose box is 121.98 x 54.99); some rooms hold `-541265.15` twice instead. The centre of `F. Porch` (639.47, 511.36) is where the PDF prints its label. A room with type `Default` and no own name prints its label from a separate class 48 object (`kitchen`, `family rm`); section 4.14 decodes those and the importer names the room from them. In one project 19 of 68 rooms have a name of their own; the typed labels name 7 more.

**X17 files** use the same record (checked on 14 first-floor rooms of a job saved by X17, e.g. `future bedrm` 164 x 168.5 at (1004.5, 598.4), all inside the walls' box): type string at `+0x109`, the anchor 68 to 90 bytes after it, so the search span after the type string is 0x200 (it was 0x140). Stage 1 reported all 57 rooms of job D lost; they now decode (26 named of 62). A room whose box was never stored (the `-541265.15` marker) reads garbage in `w, h, cx, cy` and can import with a wrong anchor.

### 4.6 Dimensions (class 24), Medium (points) / Low (line)

A linear dimension string stores its points in 909-byte records in X18 files and **554-byte records in X17 files** (the 106 dimension objects of a job saved by X17 have the sizes 1,962, 2,516, 3,070 and 4,178 bytes: two, three, four and six points, 554 bytes per point): the first point appears at `+481` and again 217 bytes later (`+698`), each further point at `+698 + 909 k` (X18 offsets; X17: first point at `+733` or `+735`, further ones at `first + 217 + 554 k`; the spacings found by search, so the first offset may move by a few bytes). Files with the header `01 CA E2 0E` (a 2024 job from an older X16-era save) use 534-byte records (sizes 1,905, 2,439, 2,973 and 4,041 bytes). The importer tries the three record sizes and keeps the longest string; job D went from 0 to 146 dimensions, and the 2024 job from 0 to 207 (it reports as an unknown generation). 159 of the point pairs of one project's first-floor dimensions coincide with PDF wall corners. About half of the class 24 objects have this shape; the rest (angular, radial, elevation, leader) are skipped and counted. **The dimension line position was not found** in any of the fields compared, so the importer offsets every string 36" away from the centre of the floor's walls (`dims::assumed_offset`), one `Dimension` per consecutive pair of points. Text overrides, dimension sets and arrow styles are not read.

### 4.7 Text (class 25), Medium (position) / Low (size)

`+246 f64` is the x of the horizontal centre and `+254 f64` the y of the **top** of the text (13 notes compared with the PDF's text boxes: x within 0.5", top within 1.2"); the strings follow: font (`Avenir`), style (`Book`/`Heavy`), the text. Size, rotation and box width were not found: Plan Studio gets 4.5" (Default Text Style) for regular and 8" (Room Label Style) for bold text, angle 0, and a bottom-left anchor estimated at 0.6 x height per character. `Wall Layer N - Viewed From Outside` objects are automatic labels and are skipped. Only notes that are not parts of walls, rooms, cabinets or devices are imported.

### 4.8 The box block: where a box-shaped object stands (cabinets, library objects), High

Cabinets (class 15), library objects (class 123, and the furniture groups of class 16) and the hosted openings (classes 9 and 10) carry one placement record, found by a 32-byte anchor (offsets in a cabinet vary with the strings in front of it: 826, 1992, 2872 ... from the `CD` byte):

```
anchor  00 00 00 00 00 00 30 40 | 00 00 00 00 00 00 18 40 | 00 x 16     (16.0, 6.0, zeros)
+0x00   f64 x       the BACK edge, middle of the width, plan inches
+0x08   f64 y
+0x10   f64 cos     \ unit vector of the direction the FRONT faces
+0x18   f64 sin     /
+0x20   f64 depth   front to back
+0x28   f64 width   along the back edge
+0x30   f64 height
+0x38   f64 top     elevation of the top above the floor (top - height = bottom)
```

Evidence, four jobs (A, B, C, D) and 284 cabinet boxes that are not soffits or islands: the point lies within half the wall thickness plus 3" of the nearest wall reference line in 257 (58 of 65, 66 of 75, 84 of 89, 49 of 55; the rest are islands and peninsulas), and in 255 of those 257 the front direction points away from that wall. In job C the point sits 3.25" or 3.5" from the wall behind it (a 6.5" or 7" wall) for 43 of the first floor's 50 boxes, and neighbouring wall cabinets are `(w1 + w2) / 2` apart along the wall (38.9" for 41.8" and 36"), so `(x, y)` is the middle of the width. A pantry cabinet at `x = 912.58` with `cos = -1` has its back on the wall whose line is at 915.83 (half a 6.5" wall = 3.25").

Plan Studio's cabinet origin is the back-left corner and its angle `a` turns local `+Y` (the front) to `(-sin a, cos a)`; so `a = atan2(-cos, sin)` and the origin is the back middle moved half a width against the width axis: `(x - sin * w/2, y + cos * w/2)`. Library symbols keep the back middle as their position and take `a` in degrees.

Not every class 15/123 object is placed in absolute coordinates: a class 123 inside a cabinet (a sink, a dishwasher) has `(x, y) = (0, 0)` in the cabinet's own frame; the importer reads those through their cabinet (section 4.9) and imports as symbols only objects whose parent is a floor or a furniture group.

### 4.9 Cabinets (class 15) and countertops (class 109), High (box) / Low (faces)

Class 15 is Chief's generic box: base, wall and tall cabinets, shelves and soffits. The kind comes from the block and the strings:

| Plan Studio kind | rule (bottom = top - height) | Confidence |
|---|---|---|
| `Soffit` | the object contains `soffit_` strings (`soffit_material_data...`): 6" boxes at 108" to 120" | High |
| `Shelf` | height <= 1.5" and bottom above the floor (0.75" boards at 69.25") | Medium |
| `Wall` | bottom >= 1" | High |
| `FullHeight` | on the floor and over 40" high | High |
| `Base` | the rest | High |

A base cabinet has **two** class 109 children of 1,158 bytes, every wall or tall cabinet one (32 of 40 base cabinets in one job, 55 of 55 others): two means "has a countertop" and the base imports with Plan Studio's default 1.5" top. The door and drawer fronts are class 114 children named after their style (`Lincoln Door`, `Lincoln Flat Panel Drawer`, `Framed Panel`, `Element 001 Door`): the importer takes the style names and infers the face layout (a drawer over a door, doors only, a bank of three drawers); that is a guess (Low). A class 123 library object below a cabinet (in the cabinet itself or in a class 62 part; 13 in job A) is built into it, in the cabinet's own frame at `(0, 0)`: a sink, lavatory or basin gives the sink-base face (a tip-out `Sink` front over double doors; 9 cabinets in job A), a dishwasher an open bay with `appliance: "Dishwasher"` (1 in A, 1 in D). **A cabinet label is not stored** (only the `%automatic_description%` macro, and `=label` as one of the material list's expressions), so labels stay automatic. A square 36" box standing in the corner of two walls is a **corner cabinet** (Low: geometry only; see the next paragraph); blind cabinets import as plain boxes, and cutouts, moldings and per-part materials are not read.

**Corner cabinets (stage 3).** Nothing marks them in the file: class 15 objects have no line records of their own (0 of 555 cabinets in 11 projects), the 262 distinct strings of the class include no `corner`, `blind` or `lazy`, and equal-size cabinets differ only in GUID and id. What marks them is where they stand. Of 495 boxes only three are square (36" x 36", 30" to 36" high), and all three have their back-right corner within 3.5" of two perpendicular walls (half a 6.5" wall plus the join): the inside corner of a kitchen. The importer reads a base or wall box that is square within 1.5", 30" to 60" a side, with exactly one back corner on two perpendicular walls (within half the wall thickness + 3"), as `CornerBase` / `CornerWall`: legs = the side, arms 24" (12" for a wall cabinet), a diagonal front, the origin at the wall corner (a back-right corner turns the cabinet a quarter turn). A 36" square cabinet that is not an L and stands in a corner would be misread. Verify in Chief (DECISIONS.md).

**Exact face layout (stage 3, not decodable).** The class 114 style entries of a cabinet do not list its fronts: the sequence of door/drawer names is `Door, Drawer, Drawer, Door` for every base cabinet from 24" to 118" wide (124 base cabinets; 40 more carry the same sequence without the class 62 parts) and grows by the same four-entry block per extra section (`dDDDDddDDD` at 48" to 72.8"), and the three 47-byte class 62 parts are byte-identical in every cabinet that has them. Drawer heights and door splits are not in these objects; the layout stays inferred from the style names.

A class 109 object sitting directly on a floor (2 per job) is a **free-form countertop** (an island): its outline is a closed chain of 90-byte line records from `+671` (section 4.13), the thickness is the `f64` at `size - 2777` and the top elevation the one at `size - 2769` (checked on four islands of three jobs: 1.5" and 36", 3" and 37.5"). It imports as a `CustomCountertop`.

### 4.10 Library objects (class 123, 16, 114), High (position, size, catalog GUID where installed) / Medium (name fallback)

A placed fixture, appliance or piece of furniture is a class 123 object with the box block (65 of 65 on one first floor: a 36" x 30" toilet, a queen bed 84" x 66.8", a dryer 33.2" x 27", armchairs 23.9" x 22", a 128" x 86" patio set) and a class 114 child with the object's name and category tags (`Elongated Toilet`, `ADA`, `Universal Design`). A bed set is a class 16 group of class 123 members (bed and two nightstands); the group is not imported, its members are.

**The catalog link (stage 3).** Stage 2 concluded that the plan stores none, because it compared against the 11 Core catalogs only (8 of 147). The entry does carry one. Its layout, from the `CD` byte:

```
+8        16 bytes  GUID of this placed copy (different for every copy)
+0x18     u32 len + "Copyright 2010, Chief Architect, Inc."   (the item's copyright line)
          u32 len + the item's name
          zeros, u32 count, the category tags as length-prefixed strings
U         16 bytes  GUID of the library item
U+16      01 01 00 00 00 00 (00|01) 01 (14|0e) 00 00 00 01 00 00 00 00 00 00 00 02 00 00 00
          the anchor: a property list of 20 entries (X18) or 14 (X17) follows (1, 2, 3, 4, 5, 6, 0x0b, ...)
U+325     16 bytes  a second GUID (constant per item; matches nothing in the catalogs)
```

The GUID is written in the byte order of the catalog's `UniqueId` text: the hex of the 16 bytes, grouped 8-4-4-4-12 in lower case, is the `LibraryObjects.UniqueId` string (`dcc1f4d7-c11e-4042-bcc8-93db33af85f2` is `Beveled Mirror (vert)` of CoreInteriors). Matched against `UniqueId` of **all** installed catalogs (398 `.calib` files, 93,765 objects: Core, Bonus, Manufacturer), 222 of 222 anchored GUIDs read from 10 saved files (7 distinct projects) name the same item as the placed copy, 214 with the identical display name and 8 differing only in `Under-Mount` against `Undermount` (**High** where it matches). 236 of 435 anchored entries match an installed catalog (X17 files carry the same anchor with the count 14: 65 of the 74 entries of the X17 job match). The others are the items of the 2010-era libraries (`Elongated Toilet`, `Round Tank Toilet`, `Oval (undermount)`, `Standard (right)`, `Washer (curved front loading)`, `Offset Undermount Sink`: GUIDs held by no X13 to X18 `CoreArchitectural` or other catalog, so no link exists to follow) and a few Wayfair, Kohler or Sub-Zero entries of another version. 53 of 488 entries have no anchor (a newer entry layout: `Warner Parsons Chair`, `Round Drain`); of those 35 contain a catalog GUID at some other offset, so the importer also reports every GUID-shaped window after the name as a candidate. The `catalog_guid` and `guid_candidates` of a symbol reach `ImportOptions::symbol_resolver` as a `SymbolQuery` with the name and tags; a resolver tries the GUID first and the display name second (display names are in `Tags4LibraryObjects.Alias`: 4 of the 12 most common unmatched names exist there, e.g. `Elongated Toilet` has two catalog items in CoreArchitectural; the others are not in any installed catalog and stay `chief-plan.<name>` boxes). The app's resolver (`plan-app/src/chief_link.rs`) does exactly that and answers `chief.<catalog-uuid>.<object id>`; without a resolver the item is still named `chief-plan.<slug of the name>`.

### 4.11 Electrical devices (class 21), High (position, kind) / Low (everything else)

Class 21 objects directly on a floor are the devices (534 in one job; 18 more sit in the template section and are ignored). The strings give the kind: name first (`Duplex`, `GFCI`, `GFCI WP`, `Single Pole`, `Three Way`, `Four Way`, `Recessed Down Light 6`, `flush mount`, `Exhaust (light)`, `Ceiling Fan (lights)`, `CO/Smoke Detector`, `Bryant Sconce 3`, `Glass Jar Pendant 02`), then the voltage or category (`110V`, `Outlets`, `Switches`, `Lighting`) and the mount (`Wall Mounted`, `Ceiling Mounted`). Light fixtures that start with material components (`5720 - Electrical fixtures`) have the name later.

The position is a pair of `f64`s at `+751`, `+755` or `+1181` (plain devices) or `+1624` (fixtures with components), found by shape: `x, y` inside the walls' box (+100"), 12 zero bytes, and at `+28` an `f64` in `[-2 pi, 2 pi]`, which is the constant `4.71239` (3 pi / 2) in all 296 first-floor devices of one job: the facing is not stored. 78 duplex outlets differ only in GUID, id, `x`, `y` and a small integer (+36), so **no per-device height** is stored either. The importer takes the height from the kind (12" outlets, 48" switches, the ceiling height for ceiling lights ...) and computes the host wall: the nearest wall reference line within half its thickness + 4" (all 268 wall devices of job A, 246 of 246 in D, 247 of 253 in B), facing out of the wall on the device's side. The small integer at `+36` (0x13b, 0x10a, 0x8a ...) was compared with the object ids of the walls and matched none. **Circuits** were not found (see the end of this section). **Switch-to-light connections** are the dashed arcs of class 34 version 1 and **gang boxes** class 150 objects (stage 3, below).

**Gang boxes.** A class 150 object directly on a floor holds two or three class 21 devices that share a box: `Duplex` with `GFCI`, `Single Pole` with `Four Way`, two `Three Way`s. In job E 16 boxes hold 36 devices against 354 loose ones; the devices of one box are 3.29" to 5.25" apart (6 pairs at 5.25", 5 at 3.29"). They have the same layout as loose devices (34 of 36 have a position by the shape search) and are imported like them (`electrical_in_groups`). Medium: the role of the container is inferred from its contents, its own bytes hold nothing but the `description` macros.

**Connection arcs.** Class 34 version 1 directly on a floor (4,882 to 16,974 bytes) is the dashed arc drawn by Connect Devices. Its first two line records are the legs of a three-point arc (start, middle point, end; the second starts where the first ends) and the rest of the object is the same arc sampled in 10" chords (a chord record each 388 bytes from +2448, the first one starting at the arc's start). Evidence, five files: in job F 251 of 368 arc ends lie within 1" of a device (lights end on the light's own point) and the other 117 within 19" (a wall switch's arc leaves 10" to 18" from its point, on the symbol's edge), and the pairs are `Recessed Down Light 6` to `Recessed Down Light 6` and `flush mount` to `flush mount` (the daisy chain of a lighting run: 22 and 11 in one file), `Single Pole`, `Three Way` and `Four Way` to a light (13, 11 and 4 in job F), `Three Way` to `Three Way` (travelers). Class 34 version 0 (326 bytes) under class 97 blocks is a part of a block definition. Plan Studio's `Connection { from, to, arc_bulge }` takes the nearest device within 20" of each end, `from` = the switch end when one end is a switch kind, and `arc_bulge` = the signed distance of the middle point from the chord (left of `from` to `to` positive, as `ElectricalLayer::bend_connection`). A load gets its switches in `switched_by`. Confidence: High for the two ends (device positions coincide), Medium for the roles, Low for arcs that end on no device (generic CAD arcs share the class; they are not imported). In the archive 117 arcs import in job E, 183 in job F.

**Not found: circuits and the facing.** Nothing in a device or a connection holds a circuit number; the classes that could (36, 38, 41) hold bounding boxes and pen values (section 3). The `f64` at +28 after the position is `4.71239` (3 pi / 2) for about 60% of the devices and one of 0, +-pi/2, pi for the rest, but it is unrelated to the wall the device sits on (the difference to the outward wall normal is 0 or pi equally often), so the facing is still derived from the host wall.

### 4.12 Stairs: flights (class 46) and landings (class 47), Medium

A stair is several class 46 flights (and class 47 landings) directly on a floor; a U-shaped stair is two flights and a landing, a stair with a winder adds a one-tread flight. Stage 1 read class 46 as a railing because of the `Classic Rail` strings; it is the stair (its materials are `stair stringer`, risers, `carpet runner`, newels, balusters and rails), and class 47 carries `landing floor`, `joist_quantity` and `perimeter`.

```
flight (46):  the first line record at +671 (section 4.13): x, y, dx, dy, length
                x, y   middle of the bottom riser; dx, dy the direction of travel
                length the run: treads x tread depth
              at R (offset varies; 9,077 in the jobs read): R-137 width, R rise, R+8 tread depth, R+18 riser height
landing (47): the first line record and three more: the four edges of a rectangle (about 48" x 46")
```

Evidence: in job A the foundation flight of a straight run starts at (378.9, 1022.8), goes +y 149.19" with a 41.9" width, and the room `STAIR WELL` has its centre at x = 379.7 and a 43.5" x 236" box covering y 936 to 1172: the flight fills the north end of the room. The run divided by the tread depth is a whole number in every flight read (5, 7, 6, 3, 14 and 2 treads of 10.5", 10.66" and 12"), the rise divided by the riser height is a whole number (137.875 / 7.6597 = 18), and the rise equals the distance to the next floor (137.875 on the first floor of one job, 126.75 + 0.875 on the foundation). Plan Studio gets each flight as a straight stair (`risers = treads + 1`, `total_rise = risers x riser`), each landing as a `Landing` over its outline with half the flight rise, and the elevation of the floor it stands on. The rail sides are not decoded (Low).

**Stacking heights (stage 3).** The spec block that holds the width, rise, tread and riser also holds where the flight stands, as elevations against the first-floor datum (0 = the first floor; the foundation stair has negative values, a second-floor stair values from 137.875):

```
R-129  f64  top     elevation of the last tread's surface
R-121  f64  bottom  elevation of the first tread's surface
R-113  f64  the bottom again, or 0.875 lower for a flight that starts on a floor
```

`bottom + treads x riser = top` for all 35 flights of 6 projects whose block the search finds (4 more flights use a block it does not find): High. A flight that starts on a landing has `bottom` = the previous flight's top + one riser, so a U or L stair chains exactly: in job H, 0.875 to 71.75 (9 treads of 7.875), the landing at 79.625, 79.625 to 126.875 (6 treads); the other flights of that stair, going the other way, 0.875 to 56.0 (7 treads) and 63.875 to 126.875 (8 treads) with the landing at 56.0 + 7.875 = 63.875. The stair's last riser reaches the upper floor's elevation + 0.875 (138.75 on a floor at 137.875; 0.875 above the first floor on a foundation stair), and a flight that starts on a floor starts 0.875 above its nominal elevation: 0.875 = 7/8" is read as the floor finish offset (Medium). Plan Studio's `Stair::base` is `bottom - 0.875 - floor elevation`, snapped to 0 under an inch; a landing is at the `base` of the flight that leaves it (or the top of the flight that arrives + one riser), which for the example gives 78.75 = ten risers, equal to the first flight's `total_rise` in the import. Landings with no flight at their edge keep half the rise (stage 2).

### 4.13 Edge records and roof planes (class 50), High (records) / Medium (planes)

Many objects store an outline as a chain of **line records**: `04 20`, then `f64 x, y, dx, dy, length` (the same five fields as a wall's line, class 31), the end of one record being the start of the next. The records of one outline sit at a fixed stride in each object type: 90 bytes (countertops, landings, label frames), 349 to 365 (roof planes). Zero-length edges sit between some edges.

A **roof plane** is a class 50 version 0 object directly on a floor (31 to 56 KB; 40 in one job on the first, second and attic floors). Its own bytes before its first child hold

1. a closed chain of line records, the plane's outline in plan including its overhang;
2. a list of object ids (neighbouring walls and planes, 8 bytes each);
3. one more line record that does not continue the chain: the **baseline** (the eave line the pitch is measured from). Counted from the start of that record: `+55` an `f64` baseline height, inches above the first-floor datum, and `+76` (`+75` in X17 files) an `f64` pitch angle in radians.

The angles convert to the round pitches a designer types: 0.7854 = 12/12, 0.6947 = 10/12, 0.5880 = 8/12, 0.4636 = 6/12, 0.32175 = 4/12, 0.0416 = 0.5/12, 0.9273 = 16/12 (every pitch of the 94 planes read in three jobs is a multiple of a quarter inch of rise). The baseline heights (254.58 on an attic floor whose walls end at 247.0, 129.84 over a 121.125" ceiling) sit just above the wall tops; the planes' joint box on the attic floor of one job is x 341 to 1313, y 94 to 1226 against the second-floor walls' 353 to 1301, 118 to 1220 (12" overhang). The two child objects (class 50 version 0 again) hold the overhang ring and the footprint polygon of the same plane and the same baseline; they are not read.

**Per-edge flags, overhang and the eave edge (stage 3).** Counted from the `x` field of each outline record:

```
+46  3 bytes  FF FF FF = a free edge; n 00 00 = the edge is joined to another plane (n 0..4)
+49  1 byte   0/1: the edge overhangs the wall
+51  1 byte   1 on every joined edge (and on a few free ones)
```

Evidence, 181 planes of 10 files, 1,073 edges of 6" or more: 343 of 347 joined edges coincide with an edge of another plane of the floor in plan (joined = ridge, hip or valley), 642 of 726 free edges have no neighbour (a free edge not parallel to the baseline is a gable end, a rake); of 289 free edges with the overhang byte at 0, 217 lie within 4" of a wall line, of 386 with it at 1, 290 are 4" or more off it (the edge projects past the wall): Medium for both. The overhang size is not stored as a number (no `f64` of the plane equals it): it is the distance from the baseline (which lies on the wall, within 3" of a wall line in most planes) to the outline's eave edge, which the outline already includes: 12" in 30 planes, 16" in 19, 15" in 10, then 9", 18", 12.5" (Medium). The importer rotates each outline so that its first edge is the eave edge (the edge parallel to the baseline furthest out), because `plan_roof::classify_edges` takes `polygon3d[0] -> [1]` as the eave: without that the fascia, soffit and gutter of an imported plane would follow an arbitrary edge. The plane record gains `overhang` (the measured eave projection) and `chief_edges` (per edge: start, end, `role` = eave, ridge, hip_or_valley, rake or top, `joined`, `overhangs`; Plan Studio ignores the key and drops it when the plane is edited). Not decoded: fascia sizes, holes and skylights, materials, dormer structure.

Plan Studio's record needs vertex heights: the plane through the baseline rises with `tan(angle)` toward the polygon's centroid side, so a vertex at distance `d` beyond the baseline is at `base + d tan(angle)`; the overhang beyond the baseline hangs below it, as in Chief. Planes whose outline repeats an earlier plane of the floor (same corners within 0.1") are dropped and counted (none in the four jobs). Stage 1 read class 18 as the roof plane; class 18 is the molding polyline (ridge caps, cornices).

### 4.14 Room labels (class 48), High (frame, text) / Medium (use)

A class 48 object directly on a floor is a text label attached to an area. Most hold the area macro `%area.round%` (12 per floor); some hold a typed name (`kitchen`, `family rm`, `3 car garage`, `pwd`, `dining rm`, `office-bedrm`; 9 on the first floor of job A) and a few an area note (`AREA BELOW GRADE`, `CATHEDRAL CEILING`). The text is a plain length-prefixed string; the **frame** is a rectangle of four line records at about `+680`:

```
kitchen    frame (711.96,1054.86) to (891.96,802.86), centre (801.96, 928.86)
room       centre (795, 929), box 181 x 265
```

The frame is the label's text area, centred where the text prints (stage 1 looked at the top edge, `y = 1054.86`, for a label printed near 922 and concluded the position was off). The centre of every typed label of job A falls inside a decoded room's box; the `3 car garage` frame centre (1024, 739) is 172" from its room's centre (1196, 680) but inside the room's 405 x 270 box, so rooms are matched by containment (smallest box first), not by nearest centre. Area notes are skipped. A label that lands in a room that already shows a name is dropped (7 of the 9 labels of job A named a room; 2 sat in rooms with a name of their own).

### 4.15 Stage-3 fields: confidence, unit test, real-file test

Unit tests are synthetic (built in the test); real-file tests are `#[ignore]` in `crates/plan-chiefplan/tests/real_stage3.rs` (`CHIEF_PLAN_A/B/C/X17` plus `CHIEF_PLAN_EXTRA`, `:`-separated) and in `crates/plan-app/src/chief_link.rs` (the catalogs).

| Field | Where | Confidence | Unit test | Real-file test |
|---|---|---|---|---|
| Library item GUID (anchor) | 114 entry, 16 bytes before `01 01 00 00 00 00 (0/1) 01 (14/0e) ...` | High where an installed catalog holds it (222 of 222) | `symbols::reads_the_item_guid_at_the_anchor_and_other_candidates`, `tests::stage3_catalog_guid_reaches_the_resolver` | `placed_objects_carry_a_catalog_guid`, `chief_link::installed_catalogs_resolve_guids_and_names` |
| GUID candidates (other layouts) | GUID-shaped windows after the name | Low (shape only; the resolver checks each) | same | same |
| Roof edge join | outline record +46 | Medium (343 of 347 joined edges share a neighbour) | `roofs::the_eave_edge_comes_first_with_flags_and_overhang`, `tests::stage3_roof_plane_has_its_eave_first_and_edge_flags` | `roof_planes_have_the_eave_first_and_joined_edges_share_a_neighbour` |
| Roof edge overhang byte | outline record +49 | Medium | same | same |
| Eave overhang size | baseline to eave edge distance | Medium (clusters at 12", 16", 15") | same | same (range check) |
| Eave edge first | geometry | High (structural) | same | same (parallel to the baseline) |
| Stair top and bottom elevation | spec block R-129, R-121 | High (35 of 35 chain) | `stairs::stacked_flights_carry_their_heights_and_base`, `heights_that_do_not_chain_are_ignored`, `tests::stage3_stairs_stack_on_the_landing` | `stacked_flights_chain_through_their_landings` |
| Floor finish offset 0.875" | stair bottoms and tops | Medium | same | same |
| Gang box devices | class 150 children | Medium | `tests::stage3_electrical_connections_and_gang_boxes` | `electrical_connections_join_real_devices` |
| Connection arcs | class 34.1 first record start, last chord end, middle point | High (ends), Medium (roles) | `electrical::decodes_a_connection_arc_and_its_bulge`, `connections_need_two_joined_records_and_a_real_chord`, same end-to-end | `electrical_connections_join_real_devices` |
| Corner cabinets | geometry of square boxes | Low | `cabinets::a_square_box_in_a_wall_corner_is_a_corner_cabinet`, `other_boxes_stay_plain` | none (3 samples in 495 boxes; counted in `cabinet_corners`) |

What stage 3 searched and did not find: circuits and device facing (4.11), rail sides of stairs (4.12: the block holds the material list's macros `=left_bracket_description` only), cabinet labels (4.9), blind cabinets (4.9), hole and skylight records of roof planes, fascia sizes.

## 5. Importer behaviour (`plan_chiefplan::import`)

`import_plan(path, &ImportOptions) -> Result<ImportResult { project, report }>`:

1. `ObjectTree::build`, generation from the header, floors, wall types.
2. Project shell: `Project::from_defaults` over `PlanDefaults::default()` (Daniel's), seeded from the file's own layers, layer sets, text styles and wall types by `seed_defaults`/`apply_seed` (X18 only: the string-level scan needs the X18 header). The wall types the walls use replace same-named defaults with the file's definition (`bridge::wall_type_def`), so the registry and the wall thickness agree.
3. Floors, walls (with `set_class`), openings, `heal_joins`, room names, dimensions, text, in that order; ids come from `Project::alloc_id`.
4. The **object stage** (`import::objects`): cabinets and free-form countertops (`Floor.cabinets`), library objects (`Floor.symbols`), electrical devices (`Floor.electrical`), stair flights and landings (`Floor.stairs`), roof planes (`Floor.roofs`), then room names from typed labels. Cabinets, devices, stairs and roofs are written as the JSON their owning crates serialize (`plan_cabinets::Cabinet`, `plan_electrical::ElectricalLayer`, `plan_stairs::Stair`, the plane record of `roof_view.rs`): this crate does not depend on them. The shapes were checked by deserializing the imported projects of jobs A, B, C and D into the real types (every cabinet, stair and device, including `plan_stairs::footprint` and `Cabinet::corners`), and each decoder's tests pin the JSON keys. Each stage has an `ImportOptions` switch (`cabinets symbols electrical stairs roofs room_labels`), and `symbol_resolver` maps a placed library object (a `SymbolQuery`: catalog GUID, GUID candidates, name, tags) to a catalog id, GUID first and name second.
5. `ImportReport`: `counts` (`floors walls curved_walls wall_ends_joined doors windows rooms rooms_named dimensions texts wall_types cabinets cabinet_soffits cabinet_corners countertops symbols symbols_with_guid symbols_linked symbols_in_cabinets electrical_devices electrical_on_wall electrical_in_groups electrical_connections stairs stair_landings stairs_stacked roof_planes roof_duplicates roof_edges_joined roof_gable_edges roof_overhangs room_labels room_labels_applied`), per-floor counts, `skipped_classes` (class, version, label, confidence, count; the classes an importer consumed are no longer listed) and `warnings` in plain language. `summary()` names walls, openings, rooms, dimensions, texts, cabinets (with free-form countertops), symbols, devices, stairs (flights and landings) and roof planes.

Measured on the real projects (release build, the file already in the OS cache): 0.3 s for a 75 MB file, 0.1 s for the 200 MB X17 job (the first read of a file from a cold disk took 18 s: that is disk, not decoding).

| File | Floors | Walls | Doors | Windows | Named rooms | Dims | Texts | Curved |
|---|---|---|---|---|---|---|---|---|
| A | 4 | 228 | 46 | 34 | 19 | 174 | 137 | 1 |
| B | 4 | 212 | 53 | 35 | 26 | 416 | 177 | - |
| C | 4 | 303 | 104 | 60 | 44 | 7 | 37 | - |
| D (X17) | 4 | 143 | 45 | 44 | 26 (was 0) | 146 (was 0) | 61 | - |
| `x17 Working Template 2025-08-20.plan` | 2 | 0 | 0 | 0 | 0 | 0 | 0 | - |

The object stage on the same jobs:

| File | Cabinet boxes | Free-form tops | Library objects (left in cabinets) | Electrical devices (wall devices with a host) | Stair flights + landings | Roof planes | Typed room labels (applied) |
|---|---|---|---|---|---|---|---|
| A | 65 (34 base, 12 shelf, 10 tall, 9 wall) | 2 | 62 (13) | 401 (268 of 268) | 7 + 3 | 31 (3 not read) | 9 (7) |
| B | 80 (39 base, 19 shelf, 5 soffit, 6 tall, 11 wall) | 1 | 51 (10) | 391 (247 of 253) | 7 + 3 | 40 (3 not read) | 4 (3) |
| C | 109 (44 base, 11 shelf, 20 soffit, 27 tall, 7 wall) | 2 | 34 (14) | 0 | 7 + 3 | 0 | 0 |
| D (X17) | 55 (33 base, 14 shelf, 6 tall, 2 wall) | 0 | 62 (12) | 367 (246 of 246) | 7 + 3 | 23 (4 not read) | 0 |

Checks the `#[ignore]` tests assert (`tests/real_objects.rs`): every cabinet origin lies inside the walls' box (24" margin); at least 80% of the non-island cabinet backs sit against a wall and at least 90% of those face away from it; library objects lie in the walls' box and 60% or more in a room of `detect_rooms` (A 58 of 62, B 51 of 51, C 34 of 34, D 59 of 62); wall devices have a host wall (95% or more); every roof pitch is a quarter-inch step, the planes' joint box contains the top living floor's walls (6" tolerance) and their plan area is at least half the box; at least three straight flights with plausible width (20" to 140") and tread (6" to 16"); import time under 2 s.

## 6. Not decoded, and why

* **Floor names**, the third floor header value, floor-specific wall defaults: no string or field found. Names are positional.
* **Door style** (doorway vs hinged), **window kind**, window swing: no byte pattern found; style comes from widths.
* **Dimension line offset, text, set**: not found (the object stores the points of a string; the line is probably recomputed).
* **Room polygons** (Plan Studio detects rooms from the walls and anchors the names).
* **Text size, rotation, alignment**; leader and callout objects.
* **Cabinets**: the catalog name and label (only the `%automatic_description%` macro is stored), the door and drawer layout (inferred from style names), blind cabinets (plain boxes; corner cabinets are guessed from where a square box stands, 4.9), cutouts, per-part materials, moldings (93). Door and drawer style entries do not say which front is where.
* **Library objects**: a catalog item for the 2010-era library objects (their GUIDs are in no installed catalog; the name is the only link, 4.10), the object's rotation about other axes, materials.
* **Electrical**: a per-device height and facing (neither is stored), and circuits (nothing found; connections are the class 34.1 arcs, 4.11).
* **Stairs**: the rail and wall sides (the block holds macros only), stringer and tread styles, winders and curved stairs (a one-tread flight stands in), and the 1 flight of job C that has no readable spec.
* **Roof planes**: fascia sizes, holes and skylights, materials, dormer structure, and a few class 50 objects that have an outline but no baseline record (3 in job A, 22 to 79 KB each, probably ceiling or deck surfaces: the pitch bytes read `4.2e-314`); those are left out and counted (3 in A, 3 in B, 4 in D).
* **Framing (68, 79)**, **moldings (93, 122) and molding polylines (18)**, **floor/ceiling surfaces (52)**: counted only.
* **X17 files**: walls, wall types, openings, floors, rooms, dimensions and roofs decode; layers and layer sets are not read (the string scan needs the X18 header).
* **Wall bottoms, sloped tops, wall-specific layer overrides, connection joins** (class 81 only counted).
* **Major arcs** and bay/bow walls (no sample).

## 7. Reproducing

```
cargo run --release -p plan-chiefplan --example import -- "House.plan" [--json] [--no-seed] [--project out.json]
cargo run --release -p plan-chiefplan --example object_survey -- "House.plan"
CHIEF_PLAN_A=... CHIEF_PLAN_B=... CHIEF_PLAN_C=... CHIEF_PLAN_X17=... \
  cargo test -p plan-chiefplan --release --test real_import --test real_objects -- --ignored --nocapture
```
