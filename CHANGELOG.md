# Changelog

All notable changes to Plan Studio are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

No version has been tagged yet (the workspace is `0.1.0`, pre-alpha), so the work is
recorded by **Round**: the numbered chunks of work that each landed as one commit. A Round
heading names its commit, and the first release (`v0.1.0`) will fold all of them together
(see [docs/release-checklist.md](docs/release-checklist.md)). Details of every feature are in
the [manual](docs/manual/00-index.md); the plan for what is next is in [ROADMAP.md](ROADMAP.md).
Test counts are the workspace totals the commit messages and manual state.

## [Unreleased]

_Nothing yet; Round 16 continues on the `wip/round-14-partial` branch workflow._

## [0.1.0] - 2026-10-09

First tagged release: everything from Round 1 through Rounds 14-16 below, on macOS, Windows and Linux (see docs/release-checklist.md and the README's Download section). Chief Architect content is never bundled; Plan Studio reads your own Chief libraries, textures and templates at run time.

### Added (Plan Agent, 2026-10-09)
- **Plan Agent** (`crates/plan-agent`, `shell/agent_panel.rs`): a dock where a change is described in plain words ("add a 24' x 30' garage on the right with a door into the mudroom"); Claude edits the plan through 23 strict tools (walls, rectangles, openings, room names, floors, foundations, a plan validator, measure, tunable parameters) and the result lands as ONE undo step with Fix Wall Connections applied. Streaming transcript, "Tweaks" sliders, effort selector, usage and cost footer; Preferences > Plan Agent holds the masked key (user preferences only). Manual: `docs/manual/plan-agent.md`; decisions AG1 to AG7; `cargo run -p plan-agent --example smoke` for a live check.

No version has been tagged yet; the first tagged release (`v0.1.0`) will fold all of the rounds together (see [docs/release-checklist.md](docs/release-checklist.md)). Round 13 is commit `9ba0ae7`; Round 14 is in the working tree.

### Round 13 - 2026-10-08 (commit 9ba0ae7)

#### Added

- **System fonts** (`plan-app/fonts.rs`, `plan-docs`): text styles use the fonts installed on the machine (Replace Fonts, Preferences > Fonts); a family that is not installed falls back to the bundled font with one note on the status bar; the PDF writer embeds the fonts it may and says so when a font does not allow embedding.
- **Openings in 3D** (`plan-3d/opening*`): casing, jambs and sills, open doors (Show Doors Open), transoms over doors, and openings in curved walls.
- **3D view interaction** (`shell/view3d_panel/`): drag to move a picked object, an orbit centre at the picked point, hover highlight, and per-mesh colour for painted surfaces.
- **Chief `.plan` import, stage 2** (`plan-chiefplan/import/`): cabinets, library symbols (as `chief-plan.<name>`; one the catalog does not know draws as a labelled box in plan and a labelled block in 3D), electrical devices, stairs, roof planes and room labels, and the X17 and X16 dimension layouts.
- **Roofs**: dormer overhangs, Dutch gable faces and roof plane handles.
- **Stairs**: spiral stairs, flared bottom treads, landing rails and stair labels.
- **Walls**: radius-to and lock on the wall tool, Reverse Layers.
- **Layout**: page templates, bent leaders, the revision table, XLSX schedule export and the print preview modes.

### Rounds 14, 15 and 16 (this merge)

Rounds 14-16 ran as 60+ parallel builder briefs against two audits: a menu/toolbar/dialog
coverage audit (`docs/chief-feature-coverage.md`, 1,099 features) and a seven-part audit of
Chief's Reference Manual and Tutorial Guide (`docs/chief-manual-coverage/`, 3,296 features
and 1,169 tutorial steps, ~1,500 new parity rows, 215 deduplicated gaps, 62 decision
corrections, a 35-brief Round 16 plan). Headlines, by area:

- **Walls**: 3+ wall junctions, Through Wall At Start/End, Structure, Foundation and Wall Cap
  tabs, multi-wall Open Object, Roof tab on interior walls, bearing and retain-framing flags,
  Wall Covering, Newels/Balusters, Rails, Materials, Components, Object Information and
  Schedule tabs; shared opening-placement rules in `plan-core`.
- **Doors and windows**: placement ghost and alignment snaps, 2 in junction clearance,
  thresholds, sill lines, both swing arcs, jambs and opening indicators in plan, Schedule tab
  and Renumber, Rough Opening, Framing, Energy, Object Information, Shape, Treatments and
  Materials tabs, double-door swing options, curved-wall casing modes, bay/box/bow roof options.
- **Rooms, floors, foundations**: Build Foundation types (footings, piers, monolithic slab),
  basements and crawl spaces, attic floor, finish thicknesses in 3D, per-room moldings and
  surface materials, Room Types, decks with framing and planking, fireplaces and chimneys
  (3-2-10 rule, roof cut, chase), split-level floors, tray and coffered ceilings, layered
  floor/ceiling/roof assemblies with a Material Layers Definition dialog, Elevation Reference.
- **Roofs**: style presets, half hip, per-wall roof buttons, wings at different plate heights,
  Extend Slope Downward, Edit All Roof Planes, Structure > Define, polygon holes, dormer
  tools, roof trim (rafter tails, ridge caps, gutters, frieze, shadow boards, boxed/flush
  soffits), Gable/Roof Line objects, skylight shapes and Edit Skylight Shape.
- **Cabinets and stairs**: all 14 Cabinet Specification tabs, push/bump, library door styles,
  face item and shelf specifications, special cabinet types, custom countertops, automatic
  fillers, module lines and Chief-style labels (3DB24), multi-cabinet Open Object; stair
  handrail sides, bullnose, dashed treads through the stairwell, Stair Schedule, spiral stairs.
- **Framing**: Build Framing dialog, ceiling joists, bearing lines, retain flags, span checks,
  Framing Overview, dimensioned wall details, tray-ceiling framing.
- **Electrical**: 3-way and 4-way switch promotion, WP and dedicated outlets, default heights,
  exterior outlets in Auto Place, schedule rows.
- **Dimensions, text, CAD**: text boxes with alignment, border and fill, Text Style
  Management, per-dimension Format and Arrow overrides, multi-point strings, Add/Delete
  Extension Line, links kept on paste, typeable CAD temporary dimensions, every Chief dimension
  tool with per-tool Locate defaults, callouts (ten shapes, cross-section lines, linked
  callouts), markers, notes and note schedules, the rich text edit bar, construction lines and
  the full Reference Display, custom line styles, the fill style system with poché, Boolean
  polylines, drawing groups (CAD default 21), Multiple Copy, Trim and Extend, DXF export
  options, Plan Footprint from outer faces.
- **Layout and print**: multiple layout files, Page Specification, program-wide sheet sizes,
  true Print Preview, 3D scenes in elevation boxes, schedules in DXF and the construction set,
  multi-sheet XLSX, site and metric scale lists to 1" = 100', watermark, sheet setup per view,
  revision tables per page (partial), page numbering with # prefixes (partial).
- **3D and cameras**: four-tab Camera Specification, Floor and Glass House cameras, backdrop
  images from Chief's folder at run time, wedge and tilt handles, Lighting dialog and light
  sets, Save Camera, key-frame walkthroughs recorded to PNG sequences or Motion-JPEG video,
  360 panorama with a self-contained HTML viewer, Vector View and Technical Illustration
  pictures, progressive Final View, Undo Zoom in 3D.
- **Materials**: Material Specification (Pattern, Texture, Properties, Materials List),
  painter modes with scope, Materials Defaults per object class, by-surface take-off,
  Lightbeans PBR material packages (zip import, normal/roughness/metallic/AO/opacity maps in
  the viewport and ray tracer, a Lightbeans folder in the Library Browser, a Downloads watch).
- **Library**: Chief's catalog tree with Trash, filters and previews, Library Object
  Specification, Convert to Symbol, Replace From Library, JSON export of user items; 3D symbol
  import from STL, 3DS and COLLADA; Export Picture of any view.
- **Terrain**: feature kinds, road markings, stepped retaining walls, plant forms and chooser,
  survey import from DXF, GPX and XYZ, cut-and-fill report, Terrain Specification parity,
  import assistants.
- **Checks and code**: 22 IRC 2021 rules with a Georgia preset and rule groups, 31 NKBA
  kitchen and bath guidelines with a report, header/joist/rafter/stair/deck calculators, and
  code minimums wired into the tools (code-legal defaults, inline "Set to code" notices, live
  check while drawing).
- **Data and files**: Property Manager with custom properties on every object, Excel export
  for editing and import with a review dialog, DXF import (ASCII and binary, every entity) with
  the Import Drawing Assistant, Chief `.plan` import stage 3 (catalog GUIDs, roof edge flags,
  stair heights, connections), data-safety fixes QA-20 to QA-29 (unknown keys preserved, id
  repair, NaN sanitising, undo grouping, foreign records kept), Windows and Linux packaging
  with a tagged-release workflow, README and contributor docs, the manual through chapter 20.
- **Preferences and defaults**: 15 Preferences pages, grouped hotkeys with conflict
  resolution and Chief XML export, toolbar customisation, the full 29-group Default Settings
  tree, saved defaults and saved plan views (partial), window commands (tiling, swap, zoom,
  Reverse Plan, Rotate Plan View), Layer and Object Painters, spell check.

Known gaps are tracked in `docs/integration-queue.md`; tests of half-built Round 16 features
are tagged `#[ignore = "R16-xx in progress"]`, and `plan-app` carries a temporary crate-level
`allow(dead_code)` until the Round 16 gate.

#### Round 14 integration pass

#### Added

- **Plan Check Settings** in the Tools > Checks menu opens the Plan Check window with its Settings dialog showing (`dialogs/plan_check.rs`); a text report window (`open_text_report`) for logs that are not findings.
- **Camera steps in the 3D menu** (`shell/view3d_panel/nudge.rs`): Move Camera with Mouse and with Keyboard, Move Camera, Orbit Camera, Tilt Camera and View Direction (eight compass snaps); 24" per Move step, 15 degrees per Orbit or Turn step, 5 degrees per Tilt step. Isometric Views and Undo Zoom in 3D are still open (`docs/integration-queue.md`).
- **Default Settings > Terrain > Terrain Defaults** (`dialogs/default_settings_terrain.rs`) opens the Terrain Specification on the plan's terrain record, one undo step.
- **Stand-in blocks**: a placed symbol whose catalog item is unknown (the importer's `chief-plan.<name>`, a Chief object whose catalog is not installed) is a labelled box in plan and a block with the same label painted over it in 3D (`plan_library::standin`, `view3d_panel/stand_in.rs`); a Chief object whose catalog is missing used to draw nothing in 3D.
- **Tests**: `scenarios/r14_integration.rs` (11 tests), including one that runs every row of the Tools menu.

#### Changed

- File > Import > Chief Plan... asks about unsaved changes first, like File > Open (`Pending::ImportChief`); the status bar gets a one-line headline of the counts and the full summary (counts and warnings) opens in a report window.
- Font notes (a family that is not installed, a font that does not allow embedding) reach the status bar, once each (`fonts::post_notes`).
- The "Known issues" line of Round 12 about the Chief import having no prompt and no report window is fixed by the two changes above.

### Planned

- Attic floors from Build Roof.
- Ties from dimensions to stairs, roof planes and framing.
- Layout: Save As Template, opening labels on pages, CAD-detail boxes from the Send to Layout dialog.
- Bump maps, and the ray tracer drawing pictures with their bitmaps.
- Chief `.plan` import: framing and the other classes that are recognized but not decoded.
- A live manual QA pass on macOS, Windows and Linux, then the first tagged release.

## Round 12 - 2026-10-08 (`066ea0d`, with Rounds 10 to 12)

About 3,120 tests (3,161 `#[test]` functions less 39 that are `#[ignore]`d, counted from the source, not from a `cargo test` run; Round 11 was about 2,750 at its last count). Run `cargo test --workspace` for the exact number before tagging. Round 12 also finished the Round 11 work that was still in flight when that section was written.

### Added

- **Curved walls, complete** (`plan-core/{joins, walls}.rs`, `plan-3d/{wall/arc, opening, wall_kinds}.rs`, `tools/wall.rs`): doors and windows are cut through arcs in 3D with each unit standing square to the arc's tangent; the plan outline and every layer of a curved wall are exact arcs (`curved_layer_outlines`); curved walls are mitered against straight and curved neighbors and get tee cuts where an end meets a side; curved pony, half, foundation and glass walls and curved gable ends; a radius, arc length and chord readout while the arc is set. Limits: a straight wall drawn to meet an arc ends square until the connection hook is wired at the gate; an opening's plan symbol is drawn on the chord.
- **Sections and elevations** (`plan-elevation/{view, dims, mlabels, dxf, styles}.rs`, `plan-core/camera.rs`, `tools/camera.rs`, `dialogs/camera.rs`): free-angle cuts (a section or wall elevation along a camera line at any angle); plan callouts with view numbers and layout sheet references (`A-3`), their shape, size and name set in 3D View Defaults; automatic elevation dimension strings (floor to floor, openings, overall); material labels; per-layer line weights in vector views; Auto Interior Elevations (four wall elevations of a room); Export DXF from the vector view and from the Camera Specification, with the lines on layers by weight class.
- **Plan Check** (`plan-check/{rules_code, rules_fixtures, rules_mep, settings, report}.rs`, `dialogs/plan_check.rs`, chapter 18): 52 rules with IRC, NEC and NKBA code references; a Settings dialog with the IRC 2021 preset, 32 limits and a tick per rule; Previous, Next, Zoom to, Ignore and Restore Ignored; a status line (`Plan Check: 2 errors, 3 warnings, 6 info (4 ignored)`); Markdown, PDF and layout-page reports. Settings and the ignore list are stored in the plan's Project Information custom fields.
- **Terrain polish** (`plan-terrain/{grading, site_symbols}.rs`, `tools/terrain.rs`, `dialogs/terrain*`): terrain walls and curbs cut the surface with a grade step; cut and fill pads with a slope ratio and cubic-yard volumes; the building pad; contour labels and settings; draggable control points on kidney and spline features; ripple water; road crown and curbs; Build Terrain stages in the readout; an auto-rebuild switch; the North Pointer (sets the plan's north angle) and the Scale Bar.
- **Rendering** (`plan-view3d/{quality, pipeline}.rs`, `plan-render/{sky, denoise, integrator}.rs`, `shell/view3d_panel.rs`, `dialogs/camera.rs`): in the live view, sun shadow maps with PCF, screen-space ambient occlusion, a sky gradient, GGX shading with Fresnel and per-material roughness, up to 8 point lights, FXAA, Technical Illustration edge lines and the Watercolor wash, and a Shading menu (Shadows, Ambient occlusion, Low/Medium/High quality, Exposure); in the ray tracer, next-event estimation of area lights, an edge-preserving denoiser guided by albedo, normal and depth, the Preetham clear sky with a turbidity slider, depth of field (aperture and focus), exposure in EV, and Save Image at 1x, 2x or 4x.
- **Cabinets** (`plan-cabinets`, `tools/cabinet.rs`, `dialogs/cabinet.rs`): temporary dimensions you can type into; a cabinet clicked into a gap takes the gap's size; label macros (`<L> <T> <W> <D> <H> <WxD> <N> <S> <F> <HW> <A>`) and a `Cabinets, Labels` layer with a draggable label; the Vanity, Pantry, Tall Oven and Refrigerator library types (flyout entries and `Shift+Tab`); a Waterfall countertop edge; full-height backsplashes; appliance bays that library appliances snap into; hardware styles (Knob, Pull, Cup Pull, Edge Pull); the Cabinet Defaults dialog (ten tabs, Edit > Default Settings); more schedule columns; Opening Indicators in 3D.
- **Layers and views** (`dialogs/{layer_display, layer_sets, plan_views}.rs`, `editor/plan_tabs.rs`, `shell/docks.rs`): a Layer Display Options table per layer set with multi-select and Modify All Layer Sets; Layer Set Management (New, Copy, Rename, Delete, Make Active, Import From Plan File); plan views as tabs above the plan; Plan View Specification, Save Plan View and Reset Plan View; Add Template Plan Views (Daniel's 20); Active Layers by Tool; Project Browser nodes for plan views, schedules and CAD details.
- **Customization and help** (`toolbar/config.rs`, `dialogs/{customize_toolbars, help, hotkeys, app_info}.rs`, `scripts/macos-dmg.sh`, `release.yml`): a toolbar set per view type (floor plan, 3D, vector elevation, layout) saved in `~/.plan-studio/toolbars.json`; Customize Toolbars with Chief `.toolbar` import, export and reset; Customize Hotkeys with filters, a conflict list, Chief `UserHotkeys.xml` import, JSON and CSV export and a printable PDF list; an in-app Help viewer (chapter tree, search, back and forward) built from `docs/manual`; About Plan Studio; the app icon; a `.dmg` for each macOS package of the Release workflow.
- **Library management** (`plan-library/{manage, browse, archive, model, preview}.rs`, `plan-import/{obj, gltf}.rs`, `shell/library_browser/user_ui.rs`, `tools/library/user.rs`): the User Catalog with folders, favorites, recents and filters; a preview pane (2D symbol or a software-shaded 3D view); Add Selection to Library and Add Active Material to Library; OBJ and glTF/GLB model import with units and up axis; `.psm` models; export and import of Plan Studio's own library zip (`.calibz` extension, not readable by Chief).
- **Electrical and framing polish** (`plan-electrical`, `plan-framing`, `tools/electrical.rs`, `dialogs/{electrical, framing}.rs`, `editor/framing_view.rs`): connections that bend, 3-way and 4-way pairs, Auto Place Switches, 23 device kinds with 3D fixture meshes in the finish chosen for each, the Electrical Service Specification tabs; corner and tee studs, blocking, the header table, stairwell trimmers and headers, rafter tail cuts and birdsmouths, built framing in the 3D view, the Framing Defaults window, the Framing Overview, and the takeoff by member type.
- **Chief `.plan` import** (`plan-chiefplan/import/`, `docs/chief-plan-format.md`): File > Import > Chief Plan... reads a project's floors, walls (types, heights, arcs), doors, windows, named rooms, dimensions and text; floor names are positional, door styles are guessed from the width, dimension lines are placed 36" outside; cabinets, roof planes, stairs, electrical, framing and library symbols are counted and skipped. The command does not ask about unsaved changes yet.
- **Menus**: Print Image prints the open 3D view (ray traced); Print Model, Layer Display Options and Add Sheet Index have their menu rows; Tools > Plan Views, Layer Settings and Toolbars and Hotkeys submenus; Library > Add Selection to Library, Import 3D Model, Export Library and Import Library.
- **Tests and QA**: ten more scenario files, `s15` to `s24` (95 tests), and findings QA-08 to QA-11 in `docs/qa-findings.md` (Auto Exterior on a hand-drawn shell, the File menu rows that reached nothing, archive rotation, a billboard vanishing in the Doll House).

### Changed

- The Help menu rows (Launch Help, View Tutorial Guide, View Reference Manual, Keyboard Shortcuts) open the in-app viewer instead of the system's default application; the manual is embedded at build time by `crates/plan-app/build.rs`, so a new chapter needs no code change.
- The standard widths, snap-to-standard and the shaping-tab values that new openings start with are now saved with the plan defaults when a Default Door, Exterior Door or Window dialog is accepted (`opening_variants`).
- The manual is brought up to Round 12: chapters 1 to 6 and 9 to 15, a new chapter 18 (Plan Check), the glossary and the status table of chapter 0. README, ROADMAP and the release checklist are refreshed.

### Known issues

- A straight wall drawn to meet an arc ends square until the connection hook is wired at the gate; an opening's plan symbol on a curved wall sits on the chord.
- File > Import > Chief Plan... replaces the open plan without the unsaved-changes prompt and has no report window (the status bar carries the counts and notes).
- The Status column of `docs/qa-findings.md` may still say "open" for QA-08 to QA-11 although no scenario test is `#[ignore]`d any more.
- The `#[allow(dead_code)]` comments in `dialogs/cabinet.rs` and `dialogs/framing.rs` still say the Cabinet and Framing Defaults windows are not opened from a menu; they are.

## Round 11 - 2026-10-08 (`066ea0d`, with Rounds 10 to 12; the parts that were still in flight when this section was first written landed in Round 12)

About 2,750 tests (2,821 `#[test]` functions less 45 that are `#[ignore]`d, at the last count; the number is still rising as the unfinished Round 11 work lands, counted from the source, not from a `cargo test` run; Round 10 was about 2,430). Run `cargo test --workspace` for the exact number before tagging.

### Added

- **Textures in 3D** (`plan-library/image/`, `plan-materials/textures.rs`, `plan-view3d/{texturing, gpu}`, `plan-render/albedo.rs`, `shell/view3d_panel/textures.rs`): 16 of the 23 scene materials show a bitmap in the Standard and Physically Based techniques and in the ray tracer. Chief's own texture files are read at run time from your install (`PLAN_STUDIO_TEXTURES`, `~/Documents/Chief Architect Premier X18 Data/Textures`, `/Library/Application Support/Chief Architect Premier X18/Referenced Files`) and never copied; a generated bitmap stands in for any that is missing. Planar mapping at the file's real tile size (`Brick(36).jpg` repeats every 36"), mipmapped and anisotropic on the card, at most two uploads a frame, decoding on a background thread, a Textures switch in the 3D bar. Pictures and billboards show their own PNG or JPEG bitmap. A decoder written for the program reads PNG (every type and depth, Adam7) and JPEG (baseline and progressive, CMYK, any subsampling). Limits: no bump maps, a painted object shows the texture of the closest scene material, pictures are flat quads in the ray tracer, the mapping is planar, not triplanar.
- **Doors and windows** (`plan-core/openings/spec.rs`, `plan-3d`, `dialogs/opening.rs`): the Sash, Lites (Standard, Diamond, Prairie, Custom Grid), Lintel with an exterior sill, Arch (Round Top, Segmental, Tudor, Gothic, Eyebrow), Hardware and Shutters tabs, built in 3D and so in the elevations; Calculate from Width for door panels; a wall niche's depth; a door mulled with sidelite windows in one frame and one casing; standard widths per style with snap-to-standard on the jamb handles; the Doors, Labels and Windows, Labels layers with a draggable label and Reset Label Position; the new opening is selected after it is placed.
- **Roofs** (`plan-3d/{cover, eave}.rs`, `plan-roof/{edges, spec}.rs`, `dialogs/{roof, defaults}.rs`): Roof Defaults (Edit > Default Settings > Roofs, and a Detail tab in Build Roof) with the eave cut (plumb, level, square), fascia, soffit (level or sloped), frieze, ridge caps, gutters, flashing, exposed rafter tails, Auto Attic Walls, the attic and lower wall types, Roof Cuts Wall at Bottom and the baseline-at-top-plate rule; per-plane eave choices on the Roof Plane Specification's Options tab; half, pony, foundation and curved walls cut by the roof.
- **Rooms** (`plan-core/{extras, rooms}.rs`, `plan-3d/slab.rs`, `editor/{roof_view, snap}.rs`, `shell/docks.rs`): Roof Over This Room off leaves the room out of Build Roof; Flat Roof Over This Room and the Flat Roof room type; concrete stem walls in 3D under a dropped garage floor or a room with a Stem Wall height; snapping to the ends and crossings of the Reference Display floor's walls; a Ref column in Active Layer Display Options; `Shift+Cmd+Y` bound to Floor Defaults as a base key.
- **File management** (`plan-app/{files, mac_open}.rs`, `dialogs/unsaved.rs`, `plan-core/io.rs`): saves go to a temporary file renamed over the plan; the replaced version is copied to `Archives/<plan>/` (the newest 20); Save a Copy, Revert to Saved, Close Plan, Backup Entire Plan (a zip of the plan and its pictures), Manage Auto Archives; autosave every 5 minutes while there are unsaved changes, never over the file; recovery offered when an autosave is newer than the plan and after a crash or a skipped prompt (`~/.plan-studio/recovery/`, the newest 10, a panic hook); Open Recent with Clear Menu; drag and drop; a Finder double-click through an Apple Event handler (and the `.psplan` document type in the macOS bundle; a MIME file for Linux); unsaved-changes prompts for New, Open, Close and Quit (`Enter` Save, `Cmd+D` Don't Save, `Esc` Cancel); a dot in the window title; "Saved 2 min ago" in the status bar. Times in names and prompts are UTC.
- **Dimensions and snaps** (`plan-core/{dimension, dim_assoc, export/dxf}.rs`, `tools/dimension.rs`, `editor/snap.rs`): Auto NKBA Dimensions; curved walls in Auto Exterior; Locate Objects groups for temporary and elevation dimensions; printed-size text picked by the box it is drawn in; DXF and PDF output that honor hidden extension lines and text-style sizes; Reverse Dimension, Convert to Manual, Align and Distribute Dimensions on the Edit toolbar; CAD intersection snaps, an Extension snap (off by default) and a Points/Markers snap.
- **Layout** (`plan-layout/{annot, layers, textfit, print}.rs`, `shell/layout_window.rs`, `dialogs/{layout, print}.rs`): page circles, arcs, leaders and revision clouds with move, resize and nudge; the layout's five-layer set with Layout Layer Display Options; text boxes that wrap, shrink to fit or stay as typed; every door and window symbol and its casing on pages; JPEG as well as PNG picture boxes; a DPI and sample count for each perspective box and an Update Views that renders on a thread with a progress bar; Print Model; a printer list (`lpstat -p`) for the system printer; the sheet index as a table box; Daniel's sheet set (ten sheets for a one-floor plan) installed into the live layout by Create Construction Set; Send to Layout from a Project Browser camera. The engine and dialog for Print Image of the 3D view are in, but the File menu does not call them yet.

### Changed

- The Release workflow (`.github/workflows/release.yml`) runs `cargo test --workspace` on each runner before it builds a package.
- Create Construction Set now adds the sheets to the plan's layout as well as offering the PDF copy.
- The manual brought up to Round 11: chapters 1 to 5, 8, 10 to 15 (file management is the new 12.2a, Roof Defaults 8.4a, textures 10.8a), and the status table of chapter 0. The in-flight parts (Chief `.plan` import, electrical and framing, the Library Browser, toolbars and help) are described under Round 12.

### Known issues

- (Fixed in Round 12.) The standard widths and the tab values new openings start with were not saved with the plan defaults.

## Round 10 - 2026-10-08 (`066ea0d`, with Rounds 10 to 12)

About 2,430 tests. The figure is counted from the source (the `#[test]` functions that are not `#[ignore]`d), not from a `cargo test` run; the same count matched the reported totals within ten at Rounds 7 and 8. Run `cargo test --workspace` for the exact number before tagging.

### Added

- **Edit commands** (`editor/{clipboard, edit_commands, transform}.rs`, `plan-core/transform.rs`): Cut, Copy and Paste (the copy hangs on the pointer until a click), Paste Hold Position,
  Paste Special > As Group, Copy and Paste in Place (`C, P, P`), Duplicate (`Cmd+D`, 12" right and down), Select All (`Cmd+A`), Select Same Type, Group and Ungroup (`Cmd+G`), Delete Objects (`Shift+Space`, by object type),
  Transform/Replicate Object (move, rotate, resize and reflect with copies as arrays), Rotate and a rotate handle on multi-selections, Reflect About Object, Point to Point Move, Center Object,
  Make Parallel and Make Perpendicular, Align and Distribute, Move to Front and Back, Lock and Unlock (by layer), Send to Layer and the Action History window. A right-click context menu opens the same commands. Every command has an id, a hotkey name and a menu row.
- **Doors and windows** (`plan-core/{openings, opening_symbol}.rs`, `editor/{opening_view, opening_edit}.rs`, `dialogs/opening.rs`, `plan-3d`): ten door styles and eleven window flavors as tools, each with a plan symbol and a 3D unit;
  size or `D01` / `W01` labels in plan with the Label tab; jamb resize handles, typed widths and jamb distances; Flip Hinge, Reverse Side, Center on Wall Segment, Mull and Unmull. The variant default sizes are estimates (`DECISIONS.md`).
- **Walls** (`tools/wall.rs`, `editor/{typed_input, wall_edit, behaviors}.rs`, `dialogs/{snap_settings, edit_behaviors}.rs`): typed length and angle while drawing and while dragging a wall end, Shift (hold the angle increment) and Alt (suspend every snap), an angle label,
  Edit > Snap Settings (object snaps one by one, grid and angle snaps, bumping, snap distance), Edit > Edit Behaviors, Break Wall, Remove Break, Reverse Layers, Change Line/Arc with a bulge handle, Make Arc Tangent, Convert to Polyline, Fix Wall Connections, and the Arc section of the Wall Specification.
- **Dimensions** (`plan-core/{dimension, dim_assoc, text_styles}.rs`, `tools/dimension.rs`, `dialogs/{dimension, default_lists}.rs`): Locate Objects settings; dimensions tied to walls, openings, cabinets and fixtures that follow them; Auto Exterior Dimensions in up to three strings per side
  (openings, wall to wall, overall) for any direction of wall, so a turned shell works; an openings string in Auto Interior Dimensions; printed-size text and dimensions (Text Styles Character Height or Printed Size); the Dimension Specification's located objects, per-point extension line toggle and Text Style tab.
- **Roofs in 3D** (`plan-3d/{clip, cover, eave}.rs`, `plan-roof/edges.rs`): walls follow the roof (gable triangles, hip clipping, interior walls rising to a vaulted ceiling, attic walls, butting roofs trimmed with flashing), and the eave detail (fascia, soffit, rake boards, optional frieze, ridge and hip caps).
  Limits: curved, pony, half and foundation walls keep flat tops; the eave cut and rafter tails are not drawn yet.
- **Rooms and floors** (`editor/rooms_edit.rs`, `dialogs/{room, floor, floor_defaults, reference_display}.rs`): room function defaults (a garage floor 24" down on a slab, no ceiling over a deck or porch, no floor under Open Below, Attic and Courtyard rooms), Floor Defaults, Build New Floor with derive options and a foundation,
  Insert New Floor Below, the Reference Display dialog (any floor, a layer set, a color), nested rooms, draggable room labels with a label template (`<name> <type> <area> <std_area> <cl_area> <dims> <ceiling> <floor> <perimeter>`), and Floor and Ceiling Structure Define.
- **Layout, print and materials** (`shell/layout_window.rs`, `dialogs/{print, materials}.rs`, `plan-layout/print.rs`, `plan-docs/materials.rs`): text boxes, perspective camera boxes (ray traced, 480 x 360 by default), picture boxes (PNG), Materials List boxes, a layout CAD row (line, rectangle, polyline, text), a rotation knob;
  the Print dialog (PDF, system printer or viewer; paper, scale, tiling, color, line weights, page range), Print Preview, Print Image and PDF bookmarks; a Materials List with eleven categories, waste factors, stock lengths and unit prices from a Master List
  (`~/.plan-studio/master-list.json`), CSV, PDF and Send to Layout; the construction set gains a Materials List sheet (eight sheets for a one-floor plan).
- **Underlays** (`plan-core/underlay.rs`, `tools/underlay*`, `dialogs/underlay.rs`): pictures (PNG, JPEG, scanned PDF) placed under the plan for tracing, with two-point calibration, opacity, rotation, show and lock.
- **Materials tools** (`tools/materials.rs`, `dialogs/materials.rs`, `plan-core/object_materials.rs`, `plan-materials/painter.rs`): Materials..., Material Painter, Material Eyedropper, Adjust Materials..., Material Builder... and Delete Surface. The 3D view shows a painted object in the closest of its fixed scene materials (no textures yet).
- **Preferences and menus** (`dialogs/{preferences, app_info}.rs`, `menus.rs`): Preferences (appearance, colors, library, folders, render, edit, snaps, architectural), Open Recent Documents, View File Information, Color Chooser, New Plan View, Refresh Display (`F5`), Fill Window Selected Objects,
  Status Bar and Toolbars toggles, Enter Full Screen, the Help menu and System Information. Chief menu rows with no Plan Studio counterpart are removed rather than dimmed (`DECISIONS.md` item 19).
- **Cabinets**: depth and corner resize handles, fit to a gap, and automatic joining of touching base countertops (Preferences > Architectural).
- **Hotkeys**: `Cmd+X`, `Cmd+C`, `Cmd+V`, `Cmd+A`, `Cmd+D`, `Cmd+G`, `C, P, P` and `Shift+Space`; the door and window flyout keys, Floor Defaults (`Shift+Cmd+Y`), Preferences, Print and Revision Cloud become live. 121 of Daniel's 143 named bindings work.
- Two scenario files (`s13_opening_variants`, `s14_wall_edit_and_snaps`).

### Changed

- The manual brought up to Round 10 and, where it was behind, to Round 9: the Edit menu and clipboard (ch. 2.5), every door and window style (ch. 3), rooms and floors (ch. 4), dimensions and printed size (ch. 5), the Round 9 cabinet faces and countertop edges (ch. 6), the stairwell guard (ch. 7), the wall Roof tab, Dutch gable and upper pitch (ch. 8),
  roofs in 3D (ch. 8.3, 10), the Print dialog, boxes and Materials List (ch. 11), underlays (ch. 12), the hotkey table (ch. 13), the new modules (ch. 14) and the glossary. README and ROADMAP refreshed.
- `DECISIONS.md` items 11 to 20 record the open questions of this round (Reverse Layers, Shift while drawing, Edit Behaviors, Lock, Duplicate, Resize, painted materials, automatic countertop join, removed menu rows, underlay formats).

## Round 9 - 2026-10-08 (`0ceb404`)

About 1,950 tests (counted from the source as for Round 10; the commit message gives no total).

### Added

- **3D picking.** Click an object in the 3D view to select it (orange highlight), Shift-click to add, double-click to open its specification, `Delete` to delete it.
- **Terrain selection.** Select Objects picks, moves, nudges and deletes single terrain objects (features, breaks, walls, curbs, roads and landscape objects), with vertex handles on small ones.
- **Landscape materials.** Grass, Mulch, Foliage, Water, Asphalt and Gravel in the 3D scene in place of the brown stand-ins; billboards turn to face the camera in the live view.
- **Queue sweep.** Roofs: the Roof Return settings dialog, Dutch gable and upper pitch (gambrel and mansard) roofs, knee walls and the Roof tab of the Wall Specification. Cabinets: editable side and back faces, countertop corner treatments and an Ogee edge, the Soffit Polygon tool.
  Dimensions and text: associative dimensions, Find/Replace Text, Edit toolbar buttons for the CAD edit tools. Schedules: click a row to select the object, grouping and totals, Stair, Room Finish and Note schedules, schedule layout boxes. Layout: box rotation and portrait sheets, the extra Project Information macros, layout JSON import and export.
  Stairs: a guard railing around the stairwell opening, dashed treads beyond the break line, landing corner handles. Details: own line styles, molding miters, custom molding profiles. A dimension pasted from the clipboard is not tied to the walls the original was.
- **Performance.** Linear room detection and wall joins, cached schedules, framing and layout signatures, a 591-wall sample and a benchmark, a release profile.
- `docs/parity-status.md` (765 parity ids), the manual for Round 8, the changelog and the release checklist; the bundle version comes from `Cargo.toml`.

## Round 8 - 2026-10-08 (`6f6a7b9`)

1,852 tests.

### Added

- **Stairs rebuilt.** Rectangle and polygon landings that join stair sections and take the height of the
  section arriving on them; curved stairs and winders; ramps with landings; Click Stairs; a Wall, Railing
  (newels, balusters, rails; panels, solid, cable or glass infill) or Half Wall on each side; open or closed
  risers; closed, open or no stringers; lock settings, Bottom Height and Top Height and Fit to floor-to-floor;
  the Staircase Specification (General, Style, Newels/Balusters, Rails, Line Style, Fill Style, Materials, Label)
  and the Landing Specification. Stairs, ramps, landings and their railings are in the 3D scene.
- **Terrain and landscaping.** Straight and curved terrain walls and curbs, Elevation Spline, Terrain Break,
  Rectangular, Kidney Shaped and Spline features, garden beds, grass regions, water features, stepping stones,
  spline roads, driveways and sidewalks, plant and sprinkler runs; each on its own layer, edited in an object
  specification dialog. The terrain surface, roads and landscape objects are in the 3D scene.
- **Images.** Create Image, Create Billboard Image and Create Image Library (saved to `~/.plan-studio/user-library.json`),
  Polyline and Spline Distribution Path and Region with spacing, offset, scatter and seeded random patterns, and 3D Solid
  Feature. PNG pictures are textured in the plan; JPEG pictures are a framed placeholder (no JPEG decoder yet). Distributions
  follow their record when it moves.
- **Typed CAD storage.** CAD object styles, CAD blocks, text macros and note types are typed fields of the plan
  (`Floor.cad_attrs`, `Floor.cad_blocks`, `Project.text_macros`, `Project.note_types`), with a one-time migration of the old hidden `CAD, Data` records.
- **Schedules in the normal selection** (`ObjectRef::Schedule`): Select Objects picks, moves, deletes and opens a placed schedule.
- **Tools > Project Information** is its own action rather than a tool.
- A hidden **Standard Area** column for the Room Schedule.
- Manual brought up to Round 7, with a new exterior-details chapter (17).

### Changed

- **Plan and layout share one undo stack.** Edit > Undo, `Cmd+Z` and the layout toolbar step back through both in the order the edits were made.
- **Hotkeys off macOS:** Daniel's Control+Z (Down One Floor) and Command+Z (Undo) fold onto one `Ctrl+Z` on Windows and Linux; the Command chord
  wins, so `Ctrl+Z` is Undo and Down One Floor is reported as unmapped there (`55aabaf`; `DECISIONS.md` item 4).
- No flyout entry of the built tool groups is a stub any more (`ALLOWED_NOT_IMPLEMENTED` in `toolbar.rs` is empty).
- Auto Stairwell now cuts a hole in the platform of the floor above as well as adding the room-divider walls; the hole and walls follow the stair.

### Fixed

The seven findings of the first scenario pass (`docs/qa-findings.md`):

- QA-01: doors swing and hinge from where the pointer is on the wall.
- QA-02: a room's own floor offset and ceiling height reach the 3D platforms.
- QA-03: the Room Schedule reports the Interior Area, and its ceiling column reads a room's override.
- QA-04: Auto Stairwell cuts the opening in the upper floor in 3D.
- QA-05: cabinets are in the 3D scene.
- QA-06: stairs are in the 3D scene.
- QA-07: each roof mode names itself after its toolbar entry.

## Round 7 - 2026-10-08 (`8112e1c`)

1,728 tests.

### Added

- **Layout view.** File > New Layout from Daniel's 18 x 24 template, page tabs, boxes you move and resize with handles, the Send to Layout
  dialog (plan views and cameras), Layout Box Specification, Page Setup, Layout Page Table, Print Layout and Export Layout PDF. The layout is saved in the plan.
- **Schedules in the plan.** Door, window, room, cabinet, electrical, framing, plant, fixture, furniture and general schedules placed as live tables,
  with D01 / W03 / C-01 / F-01 callout labels and the Schedule Specification.
- **Project Information** dialog (client, project, designer, revisions, custom fields) feeding the title block macros.
- **CAD, Text and Dimension completeness:** fillet, chamfer, offset, trim, extend, break, CAD blocks and their manager, hatch fills, boxes and insulation,
  rich text, text macros, note types, story pole and elevation dimensions, styled CAD drawing in the plan.
- **Cabinet completeness:** fillers, corner and blind cabinets, custom countertops, backsplashes and counter holes, Generate Countertop, appliance
  openings, the face editor, door and drawer styles, moldings and materials tabs, sinks and cooktops.
- Details selection with handles and wall-following trim; lights as a typed slot; Wall Bottom Height; Explode Dormer keeps its walls; an Adjust Lights hotkey.
- 94 headless scenario tests driving every tool, and `docs/qa-findings.md` (seven open findings).

### Fixed

- The macOS-only hotkey test imports are gated so Windows and Linux clippy pass (`0834122`).

## Round 6 - 2026-10-08 (`9134a2a`)

### Added

- **Template seeding:** new plans start from Daniel's Chief template (Chief `.ini` default templates, decoded wall stacks, Avenir text styles, dimension sets); a
  Preferences > Templates page; File > New Layout prints the 18 x 24 template layout.
- Framing and foundation objects in the normal selection, with handles; the Ceiling Plane dialog.
- Roofs: Join Roof Planes, Auto Floating Dormer, Build Ceiling Planes, Extend Slope Downward, Auto Roof Return.
- **Exterior details:** corner boards, quoins, moldings, floor and wall material regions, wall hatching, polygon decks and 3D solids, with plan, 3D and dialogs.
- Vector View and Technical Illustration elevations drawn from `plan-elevation` in the 3D panel; wall elevation and auto elevation cameras; camera-backed layout boxes;
  walkthrough paths that play and record; Add and Adjust Lights and the Sun Angle feeding the ray tracer and shadows.
- Manual chapters, README and ROADMAP brought up to Round 5; 3D menu auto elevations and walkthroughs live.

### Fixed

- `plan-chiefplan` inventory paths use forward slashes on every platform (Windows CI, `d63f0fe`).

## Round 5 - 2026-10-08 (`b038749`)

### Added

- **Chief catalogs in the Library Browser:** Core, Bonus, Manufacturer and User catalogs read at runtime and never copied, with thumbnails, search,
  2D symbols, 3D meshes and Replace From Library.
- **Wall kinds:** foundation, pony, glass, glass pony, half-wall, room divider, deck railing and edge, fencing, and curved variants, in plan, 3D and the dialog.
- **Slab, footing, pad, pier and platform-hole tools** with specification dialogs, selection and 3D.
- **Roof features:** holes and skylights, ceiling planes, auto dormers, gable lines, roof returns, per-edge Build Roof overrides.
- **Framing:** manual members, direction and bearing lines, reference markers, trusses and a material list; 19 framing tools.
- Elevations: material hatches, section poche and back-clip, shadows, depth weights and labels; camera dialog options.
- Layouts: PDF clip rectangles, layer colors and weights, Daniel's 18 x 24 title block, a template page, poche fills, automatic scale.
- Chief template decode: 103 wall types, 15 text styles, 14 dimension sets.

### Changed

- Storage moved to typed slots: roofs, electrical, terrain, foundation, room, opening and wall extras, and section lines.
- Hotkey tests are platform-independent (Windows and Linux CI); connect distance and tee split come from the plan defaults.

## Round 4 - 2026-10-08 (`0d9ce61`)

### Added

- Wall flags, a hinge-side control, live hotkey labels, 3D View Defaults (`Cmd+1`), and the Saved Dimension Defaults, Room Types and Text Styles editors.
- DXF export and import, CAD to Walls, elevation DXF export, Build Framing with a takeoff; view toggles (Color, Line Weights, Drawing Sheet, Print Preview,
  Reference Display) that draw; Project Browser cameras and views.
- `plan-core` typed slots (roofs, electrical, framing, terrain), room, opening and wall extras, and section lines; the `plan-docs` PDF writer (RGB, dashes, clip,
  rotated and bold text, images, the full sheet and scale catalog); Chief meshes placed through `plan-3d`.

### Changed

- Cleanup from a manual audit; docs refreshed (README, ROADMAP, parity, architecture).

### Fixed

- Windows: `.gitattributes` forces LF and the Chief config parsers tolerate CRLF; settings paths and modifier mapping for non-macOS.

## Round 3 - 2026-10-08 (`b5ba113`)

958 tests, clippy `-D warnings` clean.

### Added

- **Every tool family**, wired by an integration pass: select and edit for every object kind with handles, Tab, marquee and the edit toolbar; wall auto-connect;
  dimensions (manual, end to end, interior, point, baseline, running, centerline, angular, automatic); text, leaders, callouts, markers and notes; the full CAD set
  with arc modes and splines; cabinets with Chief's face-item editor; library placement with wall auto-rotate; stairs with Auto Stairwell; roofs (Build Roof from
  per-wall directives, manual planes, holes, skylights); electrical with auto outlets; terrain with contours; rooms, floors and foundations; space planning, plan check, schedules, the materials list and the construction set PDF.
- Specification dialogs for every object; Default Settings; Customize Hotkeys loaded from Daniel's Chief hotkey file; Active Layer Display Options with layer sets; the Project and Library Browsers.
- **The 3D view:** orbit, doll house, full camera, elevations, camera objects, cross sections, nine rendering techniques, ray trace and glTF export.
- `plan-calib` reads all 403 Chief catalogs (decoded sizes, meshes and plan symbols); `plan-chiefplan` reads Daniel's template (34 layer sets, text styles, dimension sets).
- Low-glare UI, Daniel's Chief defaults and Import Chief Template; metric units; walkthrough paths; railings and decks; roof framing; samples; the release workflow; a 16-chapter user manual.

## Round 2 - 2026-10-08 (`337e9f6`)

483 tests.

### Added

- **The tool-plugin editor:** `editor/` (context, selection, snap engine, edit handles, temporary dimensions with type-to-move, undo and redo, mitered layered-wall rendering)
  and `tools/` (Select with Chief's perpendicular move, joined-end stretch, marquee and Tab cycling; Wall with click-drag and click-click and T-splits; Door and Window with a ghost and distance readouts).
- The Wall Type Definitions dialog with a main layer and resize-about.
- Parity-driven `plan-core` work: wall flags, types, curves and roof directives; layered joins aligned at the main layer; four-state swings; inner-face room polygons; living area;
  floors and foundations; cameras; placed symbols; groups; the clipboard.
- **New engines** (the commit subject counts twelve): `plan-terrain`, `plan-materials`, `plan-layout`, `plan-electrical`, `plan-spaceplan`, `plan-check`, `plan-render` (CPU path tracer), `plan-config`
  (Daniel's Chief hotkeys, 143 of 208 named, and four toolbar sets), and an expanded `plan-library` (147 symbols).
- Docs: Chief library format research (`.calib` is SQLite, `.calibz` is a zip) and Daniel's Chief setup inventory.

## Round 1 - 2026-10-07 (`a373ce1`)

### Added

- Engine crates, all dependency-free and unit-tested: `plan-core` (undo and redo history, mitered wall joins, dimensions, CAD and text, layers, DXF export, Daniel's Chief X18 plan defaults),
  `plan-3d` (wall, opening, floor and ceiling meshes with holes; glTF export), `plan-view3d` (the egui/glow 3D viewport with Chief camera modes), `plan-roof` (weighted straight-skeleton
  automatic roofs, per-edge pitch), `plan-cabinets`, `plan-stairs` and `plan-framing` (parametric engines with 2D symbols, 3D meshes and takeoffs), `plan-docs` (door, window, room and wall schedules,
  materials list, PDF sheets), `plan-elevation` (hidden-line elevations and sections), `plan-import` (DXF reader, CAD to walls), `plan-library` (catalog format, search, 42 starter plan symbols),
  and a `plan-config` skeleton.
- Chief-style specification dialogs (Wall, Door, Window, Default Settings), startup from Daniel's Chief defaults, brighter icons with white halos, higher-contrast UI text.
- Docs: Chief parity behavior specs (`docs/parity`), Daniel's Chief setup inventory, the tool-plugin architecture plan.

## Round 0 - 2026-10-07 (`e4c12b7` to `7155648`)

The foundation, before the work was numbered.

### Added

- `plan-core` (walls, doors and windows, floors, project JSON, 2D geometry, feet-and-inches units, automatic room detection by planar face tracing that handles T-junctions and crossings; 10 unit tests)
  and `plan-app` (an egui/eframe desktop editor with a grid, endpoint, grid and 15 degree angle snaps, continuous wall drawing, door and window placement, selection, live room labels, save and load, and a Chief-style status bar).
- README, a phased ROADMAP mirroring Chief Architect's Build tools, the X18 UI study notes, a macOS bundle script, and CI for macOS, Linux and Windows.
- Chief-style toolbars (three bars), flyouts with variants and hotkeys from Chief's own command list, 152 original SVG icons, the full menu bar with Build, CAD and Terrain menus
  generated from the toolbar tables, two-key hotkey sequences, and the Low Glare canvas theme (with Paper, Dark and High Contrast) and a UI brightness slider (`32779e7`).
- Analyses of Chief X18's toolbars, menus, sub-tools and specification dialogs; a dimension dialog analysis and a shared dialog frame spec; a screenshot of the toolbars (`0e4dbea`, `9f8bd17`).
- The open-decisions log, `DECISIONS.md` (`7155648`).
