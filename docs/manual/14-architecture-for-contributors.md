# Chapter 14: Architecture for Contributors

This chapter is for people who want to read or change the code: the crate map,
how data flows, how to add a tool, and how to test. It complements
`docs/architecture-tools.md` (the original tool-plugin plan), which this chapter
brings up to date with the code.

## 14.1 Principles

1. **The plan is the model.** `plan-core` holds the data; every view (2D, 3D, elevations,
   schedules, documents) is derived from it. If you find yourself storing the same fact in two
   places, one of them should be computed.
2. **No GUI in the libraries.** Only `plan-app` and `plan-view3d` touch `egui`/OpenGL. Every other
   crate is plain Rust that can be tested headlessly and reused (exporters, batch tools).
3. **One crate per engine, one module per tool.** Engines (roofs, stairs, cabinets ...) are
   libraries with their own tests; the editor wires them to tools and dialogs.
4. **Inches everywhere**, as `f64`. Convert only at the edge (display, parse, export).
5. **Chief's words.** Names, status hints and tooltips use Chief Architect's vocabulary, and code
   comments cite the parity ids (`W-21`, `DW-8`, `CB-29`) from `docs/parity/*.md`.
6. **Honest stubs.** An unbuilt feature appears dimmed with Chief's name and says "not
   implemented yet"; it is never hidden, so the target is visible. A test in `toolbar.rs` (`flyout_entries_are_live_except_the_allowlist`) walks the flyouts of the built tool groups
   (`BUILT_GROUPS`) and fails on any dimmed entry that is not on its allow-list. **Since Round 8 that allow-list (`ALLOWED_NOT_IMPLEMENTED`) is empty**, so every entry of those groups is live and the test also fails if
   an allow-listed entry ever goes live without being removed. Dimmed buttons remain elsewhere (Display Options, Object Eyedropper, the Zoom toggle, Plan Database, Launch Help ...) because those buttons are not in the built groups. Since Round 10 `menus.rs` has a second test, `no_dimmed_rows_remain_outside_the_edit_menu`, and the door, window, Print, Preferences, Revision Cloud and Floor Defaults entries are live.

## 14.2 The crate map

Twenty-two crates in one Cargo workspace (`Cargo.toml`). Arrows point from a crate to what it
depends on.

```
plan-app ---> plan-core  plan-3d  plan-view3d  plan-roof  plan-cabinets  plan-stairs
          \-> plan-docs  plan-layout  plan-elevation  plan-library  plan-config
          \-> plan-terrain  plan-materials  plan-electrical  plan-spaceplan  plan-check
          \-> plan-render  plan-chiefplan  plan-framing  plan-import  plan-calib
plan-3d <--- plan-cabinets, plan-stairs, plan-electrical, plan-terrain, plan-materials,
             plan-framing, plan-elevation, plan-spaceplan, plan-render, plan-check
plan-layout -> plan-docs, plan-elevation        plan-view3d -> plan-render
plan-calib  -> plan-library (plan-app uses plan-calib for the Library Browser's Chief nodes)
plan-chiefplan -> plan-core      plan-config -> plan-core
```

### Model and geometry

**plan-core.** The data model and the pure geometry, with no GUI. `Project` holds floors; a `Floor`
holds walls, openings, dimensions, CAD objects, room names, symbols and groups, and **typed slots** for what
an engine crate or a view module owns: `cabinets`, `stairs`, `roofs`, `electrical`, `framing`, `foundation`, `details` and `schedules` per floor and
`terrain` and `layout` per project (opaque JSON read through typed accessors; chapter 12.2). Since Round 8 the CAD extras are typed fields too: `Floor.cad_attrs`
(`CadAttrs`, one entry per styled CAD object) and `Floor.cad_blocks` (`CadBlockInfo`), and `Project.text_macros` and `Project.note_types`. The project also holds typed values of its own: `info`
(Project Information), `lights` and `light_options` (the lights of chapter 10.13). It also has: `rooms` (planar-graph room detection from wall
centerlines, with interior and standard areas), `joins` (mitered wall outlines and per-layer bands),
`walls` (`WallClass` and its flyout variants, flags, curves, roof directives, connection and split helpers), `extras`
(the typed, serde-default dialog values of walls, openings and rooms, cross-section lines, the elevation rendering
options, and the slot accessors), `foundation` (`FoundationLayer`: slabs, slab holes, pads, piers and platform holes,
with the concrete takeoff math and the migration of the old record), `openings` (styles, casing,
labels), `floors` (build, insert, delete, exchange, foundations), `layers`, `layer_sets` (named layer sets
and saved plan views), `dimension`, `cad`,
`camera` (camera objects, walkthrough paths and the plan's lights), `symbols`, `groups`, `details` (`DetailsLayer`: corner boards, quoins, moldings, material regions, wall
hatches, decks and 3D solids, and the exterior-corner finder; chapter 17), `schedules` (`Schedule`, `ScheduleKind` and its column fields, `ProjectInfo` and its macro pairs; chapter 11),
`images` (picture specs, billboards and `Distribution` records: spacing, offset, scatter, seeded random patterns, and the placed-symbol fields `image`, `distribution`, `owner` and `solid`), `text_styles` (text styles, rich-text runs and markup, text macros and note types, and the printed-size arithmetic: `TextStyle::printed_in`, `text_height`, `TextStyles::drawn_height` and `placed_height`),
`transform` (Round 10: the `Xform` similarity, `AlignMode` and `Axis`, the alignment and distribution arithmetic and `parallel_end`; the editor applies the same `Xform` to the kinds stored as opaque records), `opening_symbol` (`plan_symbol`: the plan symbol of every door and window style as `SymbolPart`s in world inches, shared by the plan view, the PDF sheets and the tests), `openings` (styles, `OpeningVariantDefaults`, labels, mulling, `openings/spec.rs` for the dialog values), `dim_assoc` (associative dimensions: `DimAnchor`, `AnchorTarget`, `sync_dimension_anchors`) and `dimension` (the Locate Objects enums `WallLocate`, `OpeningLocate` and `ObjectLocate`, `AutoString`, `auto_exterior_set`), `underlay` (`Floor.underlays`: the picture path, pixel size and placement), `object_materials` (`Project.object_materials`: per-object material overrides from the Material Painter and Adjust Materials) and `io` (`write_atomic` and `write_atomic_with`: a temporary file next to the plan, flushed to disk and renamed over it, removed on any error; `save_project` and `load_project`), `history` (whole-project snapshot undo, 100 steps), `defaults`
(`PlanDefaults` and the Chief X18 Daniel template), `units` (feet-inches formatting and parsing, metric)
and `export::dxf` (ASCII DXF R12). It is the foundation; keep it free of GUI code and add a unit test
with every geometry change.

**plan-3d.** Turns a `Project` into renderable triangle meshes (`build_scene`, or `build_scene_with` and
`SceneOptions` for open doors, casing and lite grids) and exports glTF 2.0 (`gltf::export_gltf`, `write_gltf_files`):
walls with openings and wall-type materials, door leaves and window units in every
opening style, and floor and ceiling platforms. `wall_kinds` builds every wall class (foundation, pony, glass, glass pony, half-wall,
railings, deck edge, fencing) and curved walls as facets; `foundation` builds slabs, footings, pads, piers and the platform holes
cut in floors and ceilings; `roof` builds roof plane slabs with holes, skylights, ceiling planes and dormers; Round 10 added `clip` (2D clipping and the `TopProfile` of a wall top), `cover` (`RoofCover`: the roof and ceiling planes above each floor, the wall tops they give, butting roofs trimmed at taller walls and the attic walls; `build_scene_covered`, `SceneOptions::roof_cuts_walls`) and `eave` (fascia, soffit, rake boards, frieze, ridge caps and flashing from `plan-roof`'s `classify_edges`; since Round 11 also the eave cut, exposed rafter tails and gutters, with a plane's own `EaveOverrides`). Round 11 grew `cover` with `RoofDetail` (read from the floor's roof settings: the sizes and switches of Roof Defaults), `wall_top_clipped` (half, pony, foundation and curved walls cut by the roof but never raised), `bottom_cut` (Roof Cuts Wall at Bottom), the `Split` of a wall into attic, plate and lower parts with their wall types, and `slab::stem_walls` (concrete stem walls under a dropped garage or a room with a Stem Wall height). `opening`, `casing`, `doors` and `windows` read `OpeningSpec` (`plan-core`, `openings/spec.rs`) for the lintel, exterior sill, arch, hardware, shutters, sash and lites of an opening, take the wall's whole mulled unit for one shared casing, and `wall.rs` reads `Opening::niche_depth`; `details` builds the exterior details
(`detail_meshes`: corner boards, quoins, moldings, regions, decks, solids). `images` builds the picture quads and billboards (flat-colored; the viewport puts the bitmap on them). Floor and ceiling platforms are built per room, honoring each named room's floor height offset and ceiling height (`slab::room_levels`). Walls honor `Wall.bottom_offset`. It includes an ear-clipping triangulator
(with holes) and the `Material` enum other crates' meshes use. The enum (`mesh.rs`) includes the landscape materials Grass, Mulch, Foliage, Water, Asphalt and Gravel, which plan-terrain's landscape and road meshes use. Output is plain vertex and index buffers: X right, Y up, Z = -plan y, UVs in feet.

### Building elements

**plan-roof.** Automatic roofs: `build_roof(footprint, edges, baseline)` computes a weighted straight skeleton
and returns watertight `RoofPlane`s (hip, gable and shed edges, per-edge pitch and overhang);
`footprint_from_walls` traces the outer boundary. It falls back to uniform pitch or a bounding-box hip roof
when the exact solution fails and sets `Roof::approximate`. It also holds the roof features as input/output types: roof holes and
skylights (`roof_plane_with_holes`), ceiling planes (`CeilingPlane`), `auto_dormer` and `explode_dormer`, `apply_gable_line`, `roof_return`,
and per-edge `EdgeRoofSpec` (`build_roof_with_specs`, with `extend_slope_downward`); `join_planes` (Join Roof Planes, in `join.rs`) and `ceiling_planes_for_vaulted_room` (Build Ceiling Planes).
Floating dormers are a flag of the dormer record. Round 10 added Dutch gables and upper pitches; Round 11 added `edges` (`classify_edges`: what each plane edge is, eave, rake, ridge, hip, valley or top, which `plan-3d`'s eave builder reads), `plate_baseline` and `build_roof_at_plate` (seat the underside of the roof on the top plate), and `flat_roof_plane` (a level plane at a room's ceiling).

**plan-stairs.** The parametric stair engine: `solve` rounds the rise to whole risers and checks IRC R311.7;
`Stair` (with a `base` height for a section that starts on a landing) and `StairShape` (straight, L, U, winder, curved, ramp, landing);
`StairParams` carries the sides (`SideKind`: None, Wall, Railing, Half Wall), the stringer style, open risers, the `RailingParams` (rails, newels, infill) and an optional polygon `outline` for a landing;
`plan_symbol` (outline, riser lines, UP arrow, break line, and the sides), `meshes` and `tagged_meshes` (treads, risers, stringers, landing and winder slabs, ramps with landings, railings and walls, each labelled with a `StairPart`),
`footprint`, `top_point`, `curve_center`, `ramp_runs`, and the rail, newel and baluster geometry (`stair_railing`, `deck_edge_railing`, `polygon_slab`). The editor side is `editor/stairs_view.rs` (storage under the key `"x"`, picking, handles, joins between sections, the lock and
height rules, the four stair commands, Auto Stairwell), `tools/stairs.rs` and `dialogs/stairs.rs`; chapter 7.

**plan-cabinets.** The parametric cabinet engine: `Cabinet` (kind, countertop, backsplash, toe kick), `FaceLayout`
(the face-item tree that `resolve()` turns into rectangles), `plan_symbol`, `meshes`, `auto_label` (`B24`,
`W3030`), `run_along_wall`, fillers (`fit_between`), corner and blind cabinets, custom tops with edge profiles and cutouts, and `generate_countertops` (joining touching tops).

**plan-electrical.** Devices (18 kinds), plan symbols, `place_on_wall` and `place_free`, `auto_place_outlets`
(6' rule, door-jamb clearance, GFCI rules), `auto_place_room_light` and `auto_place_switch`, connections as
dashed arcs, `circuits` and `assign_circuits`, a schedule and legend, and 3D stand-in meshes.

**plan-terrain.** Chief-style terrain: `Terrain` (perimeter, elevation points/lines/regions, modifiers,
features with a height and style, road strips, and since Round 8 `breaks`, `walls` and `landscape`), `build_terrain` (grid, inverse-distance interpolation, Bowyer-Watson Delaunay, clipping,
smoothing that holds break lines), `elevation_at`, `contours` (marching triangles), `terrain_mesh` and `road_meshes`, `plan_symbols`,
and `auto_hole_for_building`. The `landscape` modules hold `TerrainBreak`, `TerrainWall` (walls and curbs), `Landscape` (garden bed, grass, water, stepping stones, plants, sprinklers) with the outline helpers
(`rectangle_outline`, `kidney_outline`, `arc_polyline`, `flatten_spline`), `landscape_meshes` (draped on the surface) and the plan items. The editor side is `editor/site_view.rs` and `site_view/landscape.rs`, `tools/terrain.rs` and `dialogs/terrain/object.rs`; chapter 9.

**plan-framing.** Chief's Build Framing as a library: `frame_wall` (plates, studs at 16" on center, king and
trimmer studs, plied headers, cripples, sills), `frame_floor` (joists, rim, blocking), `wall_detail` (the 2D
framing elevation), `takeoff` (counts, board feet, linear feet) and `FramingDefaults`; and the manually placed
framing: `manual` (the `FramingMember` kinds with per-kind defaults), `truss` (Fink, Howe, king post, scissor, attic and mono trusses),
`layout` (joist direction, bearing line, reference marker, truss base and the functions that honor them) and the manual takeoff with
`MaterialList::to_csv`. The editor calls it from Build > Framing and the three framing flyouts (`editor/framing_view.rs`,
`tools/framing.rs`, `dialogs/framing.rs`; chapter 11.11); no corner or T backing, no combined headers.

### Documents and output

**plan-docs.** Construction-document outputs: `schedule` (door, window, room and wall schedules to CSV or
Markdown), `schedule_kinds` (the rows, columns and callout labels of the schedules placed in the plan, for every `ScheduleKind`), `materials` (the Materials List: eleven categories, waste, stock lengths and prices from a `MasterList` kept in `~/.plan-studio/master-list.json`, `materials_report`, CSV and a schedule-style table for layout boxes and PDF pages), and a minimal
dependency-free PDF 1.4 writer (`PdfDoc`) with `plan_sheet`, a scaled plan sheet with a title block, color modes (color, grayscale, black and white), a uniform line width and bookmarks (a PDF outline).

**plan-layout.** Chief's Layout, headless: `Layout`, `LayoutPage`, `LayoutBox`, `BoxSource`, page background, Layout Edge weight and
a template page, title block templates (including Daniel's 18 x 24 with a REVISIONS table) and the macros, `send_to_layout` and
`send_to_layout_auto` (automatic scale up to 1/4"), `send_camera_to_layout`, `default_construction_set` (with a priced Materials List page since Round 10), and `render_pdf` (layer colors, weights and dashes; image boxes;
poche and shadow fills; material hatch; camera boxes, perspective boxes and picture files through hooks the app supplies). `LayoutRenderContext::new` fills the title block macros from `Project.info`.
Round 10 added the box sources text (aligned and bold), perspective, Materials List and sheet index, page CAD (box rotation in quarter turns came in Round 9), and the modules `print` (the Print dialog's engine: paper, scale, tiling, color, bookmarks; Round 11 added `print_model_pdf`), `textfit` (`TextFit`: wrap, shrink to fit, as typed), `annot` (page leaders, revision clouds and the shared annotation ids, with move, resize and remove for every kind, circles and arcs among them) and `layers` (the layout's own layer table: display, weight and color); Round 11 wired all of them to the editor (the tools, handles and Layer Display Options in `shell/layout_window.rs` and `dialogs/layout.rs`). It also gained `append_construction_set` (Daniel's sheet set into a live layout), a per-box `dpi` and `samples` for perspective boxes, the picture loader hook (PNG and JPEG through `plan-library`), and `plan_symbol`-based drawing of every door and window with its casing.

**plan-elevation.** Hidden-line vector drawings from a `plan-3d` scene: elevations, cross sections and the plan
overhead, as weighted `Line2` lists with `to_cad` and `svg`. Also face, cut and shadow regions, material hatch lines, shadows from a sun direction,
line weight by distance, and level and roof-pitch labels. Accuracy is about one pixel of the depth buffer;
no curves.

**plan-import.** Brings outside drawings in: an ASCII DXF reader, unit conversion, `to_cad_objects` and
`cad_to_walls` (parallel-line pairing). The editor calls it from File > Import and CAD > CAD to Walls
(`dialogs/exchange.rs`, chapter 12.4). No DWG, no binary DXF.

### Views and rendering

**plan-view3d.** An egui widget (`Viewport3d`) that draws a `plan_3d::Scene` with OpenGL through eframe's glow backend:
orbit, doll house, full camera (66" eye), elevation and plan-overhead cameras, opaque-then-translucent draw order,
an edge overlay, and the walkthrough pose and frame-sequence export (`walkthrough`, `export`). Camera math and edge extraction are plain Rust and unit tested; all GL is in a private module (`gpu`, with `gpu/tests`).
Round 11 added **texturing**: `texturing.rs` is the part that needs no GL and is unit tested (`needed_materials`, `next_uploads` and `MAX_UPLOADS_PER_FRAME`: at most two new material textures reach the card per frame; `ImageTexture` for a picture's bitmap; `glsl_planar_uv`, the shader's line-for-line copy of `plan_materials::textures::planar_uv`), while `gpu` creates the sRGB mipmapped textures (anisotropic where the driver offers it), keeps the resident ones by material and by picture key, and draws a mesh in one of three modes: flat color, planar material texture or picture. `viewport.rs` decodes the Chief textures a scene needs on a background thread, and `Viewport3d::textures_enabled`, `set_image_textures` and `context_lost` (re-uploads everything) are the app's handles; chapter 10.8a.

**plan-render.** A dependency-free CPU path tracer: BVH, Lambert plus GGX, glass, soft sun shadows, sky, Russian
roulette, tone mapping, optional denoise, deterministic multithreaded output and its own PNG writer. Round 11 added `albedo.rs`: a textured material looks its base color up in the same `plan_materials` bitmap through the same `planar_uv` (bilinear, repeating, brightness matched to the flat albedo within 1/4 to 4 times and capped at 0.95), only for opaque materials in the Physically Based technique and when `RenderSettings::textures` is on (the default). Pictures stay flat quads (a ray-tracer triangle carries no UV).

**plan-materials.** The material system as data: 45 materials, 2D hatch patterns, procedural textures, default
assignments, the nine rendering techniques, and sun position from date, time and latitude. Round 11 added `textures.rs` (`TextureStore`, `TextureImage`, `planar_uv`, `projection`, `textured`): for each of the 16 textured scene materials it loads Chief's own texture by file name from the install's folders at run time (`default_texture_dirs`: `PLAN_STUDIO_TEXTURES`, `~/Documents/Chief Architect Premier X18 Data/Textures`, `/Library/Application Support/Chief Architect Premier X18/Referenced Files`; `Name(36).jpg` carries its tile size) and otherwise generates a procedural bitmap, caches decoded images by bytes (512 MB, least recently used out) and is shared by the viewport and the ray tracer (`TextureStore::shared()`). `painter.rs` (Round 10) maps a painted library material to a scene material (`scene_material`, `nearest_by_color`, `build_material`).

### Library, checks, planning

**plan-library.** The catalog system: JSON catalog format, in-memory index with ranked search and a category
tree, and the built-in 2D symbol catalogs (starter, plants, bath and kitchen, lighting and electrical, furniture and
exterior). Round 11 added `image/`: the dependency-free decoder used for textures, 3D pictures and layout picture boxes (`decode`, `decode_file`; PNG of every color type and depth with Adam7, and baseline and progressive JPEG, into straight sRGB `Rgba8Image`s with box-filter `downscaled` and `mipmaps`; `inflate.rs`, `png.rs`, `jpeg.rs`, and a small PNG encoder). The Library Browser work is still in progress in the working tree (`archive`, `browse`, `manage`, `model`, `preview`, `rules`, `user`) and is not described here.

**plan-check.** Chief's Tools > Checks as a rule engine: twelve IRC-based rules, `Finding`s with severity, location,
rule text and fix, `plan_footprint`, `report_markdown`; limits in `CheckOptions`.

**plan-spaceplan.** The Space Planning Assistant: `Questionnaire`, `generate_boxes` (greedy affinity packer),
`bump`, `validate`, `build_house` and `plan_symbols`.

### Reading Chief's files (read-only)

**plan-calib.** Read-only access to `.calib` and `.calibz` catalogs: its own SQLite reader, inflate and zip code,
`ChiefCatalog`, `ChiefRegistry`, `ChiefLibrary::discover/search`, a `decode` module that recovers object sizes and
plan-view symbols from Chief's binary blobs (reverse-engineered, with measured coverage in its README), `mesh3d` (decoded triangles
fitted to a placed symbol, with a cache), and a bridge to `plan-library` items. The Library Browser uses it (chapter 6.6).

**plan-chiefplan.** Read-only reader for `.plan` and `.layout` templates: scan, classify, decode per-layer color, line
weight and flags, decode the object stream (wall types with real layer stacks, materials, text styles, rich text defaults, dimension
defaults, default heights, sheet size; `docs/chief-template-format.md` section 7), and seed `PlanDefaults` (wall types, layers, layer sets, text styles, dimension sets). The editor uses it
for File > Templates > Import Chief Template....

**plan-config.** Chief's user configuration: `UserHotkeys.xml` (208 of 2,284 commands carry keys in Daniel's file),
the `.toolbar` files and the preferences INI, with Daniel's files embedded at compile time. `templates` finds Chief's default plan and layout templates
(`detect_chief_templates`: the INI keys `Default Plan Template` and `Default Layout Template`, then a scan for the stock names; chapter 1.7.1).

### The application

**plan-app.** The desktop editor (binary `plan-studio`). Layout of `src/`:

```
main.rs         window, menu/toolbar/panel wiring, file operations, dialogs host
toolbar.rs      the three bars, flyout tables, BINDINGS, Action enum
menus.rs        the menu bar (Build and CAD menus generated from the flyout tables)
theme.rs        canvas themes, UI brightness, ~/.plan-studio/settings.json
plan_defaults.rs  defaults loading and the helpers that turn defaults into objects
icons.rs        embedded SVG icons
templates.rs    default templates: settings.json `templates` key, the template-seed.json cache, the overlay
                onto PlanDefaults, and new_layout
scenarios/      Chief-parity scenario tests (cfg(test) only); see 14.8
tools/          one module per Chief tool behind the Tool trait (select, wall, opening, pan,
                dimension, text, cad, cabinet, stairs, roof, electrical, library, camera, terrain,
                foundation, framing, details, schedule, underlay, materials); tools/cad/ holds the CAD edit tools (edit.rs)
                and per-object CAD looks (style.rs); tools/library/chief.rs is the Chief catalog backend
editor/         services shared by tools: EditorContext, selection, snap engine, handles,
                temporary dimensions, undo history, plan rendering, wall connections, edit actions
                and their dispatch, and per-object views (roof_view, stairs_view, site_view, placed,
                rooms_edit, framing_view, foundation_view, details_view, schedule_view, opening_view); the Edit menu's commands (edit_commands: ids, the
                right-click menu; clipboard; transform: Transform/Replicate, Align/Distribute and the click-driven modes; behaviors: Edit Behaviors;
                wall_edit: Break Wall, Remove Break, Reverse Layers, Change Line/Arc, Make Arc Tangent, Convert to Polyline; opening_edit: Center on Wall Segment,
                Mull, Unmull; typed_input: typed length and angle); restyle (View > Color
                and Line Weights) and sheet (the drawing sheet). site_view also holds migrate_legacy_storage,
                run when a plan is opened
shell/          docks, library browser (library_browser.rs and library_browser/chief_ui.rs for the Chief
                nodes, png.rs for thumbnails), the 3D panel (view3d_panel.rs: GL scene, vector elevations,
                walkthrough play and record, sun, lights), layout_window.rs (the layout view), the runtime
                hotkey map, and spec_dialogs (the one place that maps an object kind to its specification dialog)
files.rs        (Round 11) file operations: safe saves and `Archives/<plan>/` rotation, autosave, crash recovery files and
                the panic hook, the dirty check (a hash of the serialized plan), Save a Copy, Revert, Close Plan,
                Backup Entire Plan (a stored zip), Manage Auto Archives, Open Recent and drops; `FileState` is the
                window's memory, `drive_files` runs once a frame
mac_open.rs     (Round 11, macOS) Finder's "open documents" Apple Event: an Objective-C handler registered through the
                runtime, queued for `drive_files`
dialogs/        Chief-style specification dialogs on the shared frame (wall, opening, room, roof, foundation,
                framing, camera, symbol, details, cad, text, schedule_spec, project_info ...), the layout
                dialogs (layout.rs: Send to Layout, Layout Box Specification, Page Setup, Page Table,
                Print Image), print.rs (the Print dialog and its delivery), materials.rs (Materials List and Master List),
                preferences.rs, app_info.rs (Open Recent, File Information, Color Chooser, Help and System Information), unsaved.rs (the
                Unsaved Changes, Revert and Recover prompts and their keys),
                transform.rs, delete_objects.rs, send_to_layer.rs, action_history.rs, snap_settings.rs, edit_behaviors.rs (the Edit menu's dialogs),
                floor_defaults.rs, reference_display.rs and underlay.rs, Default Settings and its lists (default_lists: dimension sets, room types, text styles;
                defaults.rs: the tree and the Preferences > Templates page), Customize Hotkeys, Layer
                Display Options, file exchange and framing windows (exchange), the Build and Tools windows
```

## 14.3 How data flows in the editor

- `EditorContext` (in `editor/mod.rs`) owns the `Project`, the active floor, the selection, snap
  settings, defaults, view flags, the status text, the clipboard, temporary dimensions, session-only extras
  and cached derived data (`rooms`, `outlines`, `layer_outlines`).
- A tool changes the model like this: `cx.begin_change("Move Wall")` (snapshots the project for undo), mutate
  `cx.project`, `cx.mark_dirty()`, and return `ToolResult::committed("Move Wall")`. The shell calls `cx.refresh()`
  once a frame, which recomputes rooms and outlines when dirty.
- Pointer events arrive as `PointerEvent { world, snapped, snap, screen, modifiers, button, down, drag_delta }`
  already mapped to world inches; key events as `KeyEvent`. Activation hotkeys are never read by tools; they
  come from the runtime `HotkeyMap` (chapter 13).
- Objects are addressed by `ObjectRef` (Wall, Opening, Dimension, Cad, Cabinet, Stair, RoofPlane (a plane, a ceiling plane or a dormer), Symbol,
  Camera, Text, Room, Device, Terrain, Foundation (a slab, hole, pad, pier or platform hole), Framing (a placed member or layout line) and Detail (a corner board, quoin, molding, region, hatch, deck or solid)).
  Each kind's view module answers `exists`, `layer_of`, hit tests, box selection, handles and the specification dialog. A placed schedule is `ObjectRef::Schedule(Id)` since Round 8: `exists`, `layer_of` (the schedule's own layer),
  `hit_test_cx` (`schedule_view::pick`), `extra_in_rect`, group move, Delete (one undo step) and the double-click / `Enter` specification are wired, and the Schedule tool shares `cx.selection` with Select Objects. Pictures and
  distribution records are `ObjectRef::Symbol`. Single terrain objects (a wall, a bed, a plant run) are `ObjectRef::TerrainObject(TerrainHit)` (`editor/selection.rs`), so Select Objects picks, moves and deletes them, and `ObjectRef::Terrain` addresses the whole terrain. The Terrain tool's own hit test is `site_view::{TerrainHit, hit_terrain}`. A second, smaller `plan_core::ObjectRef` (Wall, Opening, Dimension, Cad,
  Symbol, Camera) is used by object groups and the core clipboard; the two types are not the same, so convert
  deliberately.
- Things the model has no field for yet go in the typed slots of `plan-core` (`Floor.cabinets`, `stairs`, `roofs`, `electrical`,
  `framing`, `foundation`, `details`, `schedules` and `Project.terrain`, `layout`: opaque JSON the owning view module reads and writes with `begin_change` and
  `mark_dirty` like any other edit; `Project.info`, `lights` and `light_options` are ordinary typed fields), in the serde-default `extras` of walls, openings and rooms (chapter 12.2), or, for what is still
  not stored, in `SessionExtras` (per session). A file written before the slots existed is converted by `site_view::migrate_legacy_storage`
  (which calls `roof_view::migrate_legacy`, and that calls `plan_core::foundation::migrate_legacy` and `plan_core::camera::migrate_legacy`, which moves the old lights records) when `EditorContext::set_project` loads it; add a
  step there when you move another record into a slot. The CAD extras (own colors and weights, fills, arrows, blocks, text macros and note types) used to ride on tagged text records on the hidden layer
  `CAD, Data`; Round 8 moved them into the typed fields `Floor.cad_attrs`, `Floor.cad_blocks`, `Project.text_macros` and `Project.note_types`. `plan_core::cad::migrate_legacy` (called from `roof_view::migrate_legacy`, so from the same load-time chain) moves the old records once and drops the layer; a slot
  that is already filled wins. Pictures and distributions need no slot: they are fields of `PlacedSymbol`.
- **Lights** are the typed `Project.lights` (`PlanLight`) with `Project::{lights, add_light, update_light, remove_light, light_settings}`; the ray tracer reads them (`dialogs::camera::render_lights`).
- **The layout** lives in `Project.layout` as the JSON of a `plan_layout::Layout`; `shell/layout_window.rs` loads and stores it. **Undo is one stack for plan and layout** (Round 8, `docs/integration-queue.md` "Undo across plan and layout"): `LayoutView` parks the project as it was before each layout edit in `LayoutView::steps`, and the entry points that hold the context
  (`layout_window::{dispatch, show_central, show_dialogs, new_layout, undo, redo}`) hand them to `EditorContext::record_undo_step` at the end of the frame, so `cx.undo()` and `cx.redo()` step through both in the order the edits were made. `LayoutView::sync` reloads the layout from the restored project.
  The one limit: a plan edit and a layout edit made in the same frame record as "plan, then layout" whichever came first, which one pointer cannot produce.

## 14.4 How to add a tool

Example: a hypothetical "Soffit Line" tool. Follow the same steps for any Chief tool.

1. **Read the spec.** Find the tool's section in `docs/parity/*.md` and its flyout in
   `docs/chief-x18-subtools.md`. Note the ids you will implement (for example `CB-17`).
2. **Create the module.** `crates/plan-app/src/tools/soffit_line.rs` with a struct that implements `Tool`:
   - `id()` returns the `ToolId`; `name()` returns Chief's name (for example "Soffit"); `hint()` is the status-bar
     text.
   - `activate`/`deactivate` reset state; `pointer_down/move/up`, `double_click` and `key` drive a state machine;
     `draw_overlay` draws the ghost; `edit_toolbar` returns the Edit toolbar buttons for the selection.
   - `frame()` runs once per frame if the tool owns dialogs or palettes. `set_variant()` receives the
     `ToolId::...Variant(..)` payload when a flyout entry (or its hotkey) is picked; the tool object
     is shared by all variants of its family, so state survives a switch.
3. **Register it.** Add `pub mod soffit_line;` and one line in `registry()` in `tools/mod.rs`. If the tool has
   flyout variants, add a `ToolId::SoffitVariant(Mode)` and map it in `ToolId::base()`.
4. **Put it on the toolbar.** In `toolbar.rs`, replace the dimmed `todo("icon", "Name")` entry with
   `item("icon", "Name", Action::SetTool(ToolId::...))` (and `with_hotkey(...)` for the tooltip). The Build and CAD
   menus and the command list for the hotkey dialog pick it up automatically, because they are generated from
   the same flyout tables. Add an SVG to the icon set in `icons.rs` if none fits.
5. **Hotkeys.** Daniel's Chief bindings map by command name, so a tool whose name matches (for example
   "Soffit" -> `T`) is live as soon as the entry is no longer `NotImplemented`. For a Plan Studio default
   binding add a `bind(...)` row to `toolbar::BINDINGS`.
6. **Change the model safely.** `cx.begin_change(label)` before mutating, `cx.mark_dirty()` after. One user
   gesture is one undo step. Respect locked layers with `cx.check_unlocked(...)`. Use `cx.snap_at(...)` for snapping
   and `cx.fmt_dim(...)` for lengths.
7. **Dialog.** Implement `SpecPages` in `dialogs/<name>.rs` (tab list built with `on("Name")` and `off("Name")`,
   `page(ui, tab)`, `preview(painter, rect)`, `error()`), wrap it in `SpecDialog::new("Title", key)`, and add one
   arm to `SpecDialogs::open` in `shell/spec_dialogs.rs` so double-click and `Enter` open it for your `ObjectRef`. Controls the model cannot store yet are drawn with `dis_check`, `dis_radio` or
   `session_check`.
8. **Tests.** Drive the tool with synthetic `PointerEvent`s on a fresh `EditorContext::new(plan_defaults::embedded())`
   and assert on the `Project`; no GUI is needed. The registry test checks every `ToolId` is registered once.
9. **Document.** Update the matching chapter of this manual (remove "(planned)"), tick the parity doc if it has a gap
   table, and cite the ids in doc comments.

Engine first, editor second is the usual order: write the algorithm in its own crate with tests (as `plan-roof`,
`plan-stairs`, `plan-terrain` did), then add the tool that calls it.

## 14.5 Adding a rule, a catalog or a format

- **A Plan Check rule**: write one function in `plan-check/src/rules.rs` with a doc comment naming its IRC section,
  call it from `plan_check`, and test it.
- **A library catalog**: write a JSON file in the catalog format (`plan-library` README) and load it with
  `Catalog::from_json`, or add a module `catalog_*.rs` next to the starter ones. Ids are `core.<group>.<name>` and
  must be unique; symbols have at most 40 strokes and their bounds must match width and depth within 1".
- **A file format**: put the reader or writer in a library crate with `std` only if you can, take and return strings or
  bytes, and test round trips. The editor then calls it from a menu action.

## 14.6 Testing and quality gates

```bash
cargo test -p plan-core                  # one crate
cargo test --workspace                   # everything (1,852 tests after Round 8; about 2,750 in the Round 11 working tree: 2,821 `#[test]` attributes minus 45 `#[ignore]`d at the last count, from the source on 2026-10-08, and still rising)
cargo test -p plan-app                   # editor state machines, dialogs, hotkeys (no window needed)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

- Every geometry change in `plan-core` gets a unit test. Tool tests use synthetic events; dialog tests drive the
  pure form logic (`WallForm::set_length`, `LayerTable`, `FaceLayout` commands) rather than egui drawing.
- Tests that need a real Chief install (`plan-calib`, `plan-chiefplan`) are `#[ignore]`; run them with
  `cargo test -p plan-calib --release -- --ignored --nocapture --test-threads=1`. Everything else uses synthetic
  fixtures built at test time (some build SQLite and zip files with the `sqlite3` and `zip` command-line tools and
  skip if the tool is missing).
- Generated documents are checked: `docs/chief-hotkeys-resolved.md` is regenerated with
  `cargo run -p plan-config --example gen_hotkeys_md > docs/chief-hotkeys-resolved.md`, and a test fails when it is
  stale.
- CI (`.github/workflows/ci.yml`) runs `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` on Linux, macOS and Windows for every push to `main` and every pull request. The Linux job
  installs the GUI build packages (GTK 3, xkbcommon, Wayland, xcb, Mesa GL). `ROADMAP.md` still shows CI as unchecked. The Release workflow (`.github/workflows/release.yml`) also runs `cargo test --workspace` on each matrix runner before it builds a package (Round 11), so a tag on a broken commit publishes nothing.
- The timed tests of the decoder and the textures read real Chief files and are `#[ignore]`d: `cargo test -p plan-library chief_textures -- --ignored --nocapture` decodes the textures of your Chief install and prints the times. The file tests of `files.rs` write only under the system temp folder; file and archive names use UTC so they sort the same on every machine.
- Keep `plan-core` free of GUI dependencies and Chief content out of the repository (see 12.7).

## 14.7 Where the decisions are

`DECISIONS.md` records open decisions (license, app name, Chief catalog handling) and the queued work.
`ROADMAP.md` is the feature inventory by phase. `docs/parity/` holds the per-area behavior specs with stable ids.
`docs/chief-x18-*.md` are the captures of Chief's UI the program is built to.

## 14.8 Scenario tests and the QA findings

`crates/plan-app/src/scenarios/` holds fourteen files of Chief-parity scenario tests (`s01_house_shell` to `s14_wall_edit_and_snaps`, compiled only for tests; `s13_opening_variants` covers the Door and Window flyouts, their symbols, labels, jamb handles, temporary dimensions and mulling; `s14_wall_edit_and_snaps` covers typed wall dimensions through the shell's key path, the wall Edit toolbar commands, and the Snap Settings and Edit Behaviors windows). Each drives the real tools through pointer and key events on a `PlanApp` with no window,
then checks the model, the undo stack, the dialog requests, the 3D scene and the documents. The working tree has begun adding more (`s15_edit_commands`, `s16_walls_typed_and_edit`, `s17_dimensions`, and a scratch file `s99_scratch`); they are still in progress and are not described here. The Round 11 features that have landed are tested in the crates they touched, for example `plan-3d/tests/{opening_spec, opening_variants, roof_detail, roof_walls, room_function}.rs`, `plan-render/tests/textures.rs`, `plan-library/src/image/tests.rs`, `plan-materials/src/textures/tests.rs`, `plan-view3d/src/texturing/tests.rs` and the `files.rs` tests.

- **`Sim`** (`scenarios/mod.rs`) is the harness. `Sim::new()` builds a `PlanApp` with the embedded defaults and a headless `egui::Context`. `sim.tool(ToolId)` activates a tool the way a toolbar click does; `move_to`, `down`, `up`, `click`, `drag` and `double_click`
  send pointer events at plan coordinates and run what the shell does after each call (`finish_tool_call`, `refresh`); `key`, `esc` and `action` send key events and toolbar actions; `undo` and `redo` read the step names; `open_spec`, `ok`, `cancel` and `dialog_frame` open a specification dialog and run its frames headlessly.
- **Findings.** A scenario that exposed a bug is marked `#[ignore = "QA-nn"]` so the gate stays green, and the bug is written up in `docs/qa-findings.md` with the repro, what Chief does and what Plan Studio does (the file now has a Status column). Run
  `cargo test -p plan-app scenarios -- --include-ignored` to see ignored ones fail; when one is fixed, delete its `#[ignore]` line.
- **Results.** The first pass (Round 7) found seven. **Round 8 fixed all seven and no `#[ignore]` is left** in `scenarios/` (s01 to s14; the in-progress `s17_dimensions` has one test ignored as QA-08, not yet written up in `docs/qa-findings.md`) (96 `#[test]` functions across the twelve files; the whole workspace has 1,852 tests): QA-01 door swing and hinge from the pointer (`plan_core::openings::door_defaults_for_pointer`),
  QA-02 a room's floor offset and ceiling height in 3D (`plan_3d` builds platforms per room; `project_hash` includes room names), QA-03 the Room Schedule's Interior Area (and a hidden Standard Area column), QA-04 the Auto Stairwell hole in the floor above, QA-05 cabinets and QA-06 stairs in the 3D scene
  (and in `project_hash`), QA-07 each roof mode named like its toolbar entry. Round 8 also added `editing_the_dimension_text_and_cad_dialogs_is_one_undo_step_each` to `s07_dimensions_text_cad`: it changes a value in each of the Dimension, Text and CAD dialogs, presses OK and checks that each is exactly one undo step that restores the old value.
- **Test-access limits.** The Dimension, Text and CAD dialogs now expose a `#[cfg(test)] draft_mut()` (through `SpecDialogs::{dimension,text,cad}_draft_mut`), so a scenario changes a value, presses OK and checks one undo step. The Electrical, Terrain and Roof dialogs are owned by their tools and are driven through the real overlay frame.
- `s11_every_tool` walks every tool id (213 of them at the Round 7 commit; Round 8 added the stair, terrain and image variants) from the registry, the toolbars, the flyouts and the menus and checks that none panics and that each has a name and a hint; `docs/qa-findings.md` lists which ones create nothing on a click-click and why.

To add a scenario, copy the shape of the nearest file (each has a small `house()` helper that draws a shell), drive the tools with `Sim`, and assert on `sim.cx().project`, the history labels and `build_view_scene`.
