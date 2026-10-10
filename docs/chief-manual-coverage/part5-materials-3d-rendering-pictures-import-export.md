# Chief X18 Reference Manual audit, part 5: Materials, 3D Cladding, 3D Views, 3D Rendering, Pictures/Images/Walkthroughs, Importing and Exporting (manual pages 1099-1306)

Written 2026-10-08 by the manual audit (part 5). Source: Chief Architect's Reference Manual pages 1099 to 1306 (chapters 34 Materials, 35 3D Cladding, 37 3D Views, 39 3D Rendering, 40 Pictures, Images and Walkthroughs, 41 Importing and Exporting), read in full from `~/plan-studio-dev/chief-docs` (read only). Every row below is written in my own words; only tool, dialog, panel and field names are Chief's. Nothing from the manual text is stored in the repo.

**How status was found.** For each feature I checked `docs/chief-feature-coverage.md` (the Round 14 audit, now partly stale), then `docs/parity/*.md` (including the Round 14 and Round 15 3D and materials sections and the "Coverage audit additions"), then the code (grep of `crates/` on branch `wip/round-14-partial`, read only, with the Round 15 builders still editing). Where the earlier audit and the working tree disagree I trust the tree. Statuses: **Works**, **Partial**, **Missing**, **Differs** (differs by design), **Out-of-scope** (Ruby, cloud, licensing, VR and input hardware, vendor services, Windows-only formats), **In progress (Round 15)** (a brief in `~/plan-studio-dev/briefs/r15` covers it and the code is not finished). Evidence "NO SPEC (id)" means no parity id covered the feature before this audit; those rows were appended as new ids (see the end of this file and the "Manual audit additions (part 5)" headings in `docs/parity/*.md`). Page numbers are the printed numbers of the manual.

## Counts

| Measure | Count |
|---|---|
| Features enumerated (table rows) | 455 |
| Works | 59 |
| Partial | 188 |
| Missing | 175 |
| Differs | 12 |
| Out-of-scope | 17 |
| In progress (Round 15) | 4 |
| Rows that had no parity spec (NO SPEC) | 207 rows, folded into 116 new parity rows |
| Dialogs and dialog groups compared in the panel table | 36 |

## 34. Materials (pp. 1099-1139)

### 34.1 About materials

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1099 | Materials give objects their 3D look, supply Material List quantities and (as emissive area lights) light the ray-traced views; they are applied with the Material Painter or an object's Materials panel | Works | C-56..C-61, `tools/materials.rs`, `plan-materials`; library in `~/.plan-studio/materials.json` plus the core library |
| 1099 | A material carries a texture for rendered views, a CAD pattern for vector and hand-drawn views, and a solid Material Color | Partial | `MaterialDef` (color, texture, pattern) in `plan-materials/src/material.rs`; the pattern draws the plan and elevation hatch (C-61a) but no pattern lines appear in 3D vector views, and the Material Color sits on the Properties tab, not the Pattern tab |
| 1100 | Toggle Patterns turns the pattern lines of the material on or off in vector, technical-illustration and hand-drawn views | Missing | C-74 (no toggle; no pattern layer in 3D views) |
| 1100 | Keep Pattern/Texture in Sync: one check box ties scale, offset and angle of pattern and texture together | Missing | NO SPEC (C-80): the Pattern and Texture tabs hold separate scale and angle (`pattern_scale`, `texture_scale_in`) |
| 1100 | Stretch to Fit textures (artwork that does not tile and is stretched across the surface, for picture frames) | Missing | NO SPEC (C-81): Texture tab has only a tile size; no stretch flag |
| 1100 | Fourteen material map kinds: ambient occlusion, anisotropy, bump, emissive, metal, normal, opacity, rotation, roughness, texture, translucent, transmission color, transmission roughness, transparent | Partial | NO SPEC (C-82): normal, roughness, metallic, height (bump), ambient occlusion and opacity maps from a package (BC-PKG1, BC-PKG6, BC-PKG7, BC-PKG8) plus a scalar bump; anisotropy, rotation, emissive, translucent, transmission and transparent maps are not read (BC-PKG11) |
| 1101 | Invert check box next to each map (all but texture, normal, ambient occlusion and transmission color) | Missing | NO SPEC (C-83): only `MaterialDef.normal_flip_y` for a DirectX normal map; no per-map invert |
| 1101 | Emissive materials act as Area Lights in ray-traced views, and a fixture's bulb material can take its color and intensity from the fixture's own lights | Partial | NO SPEC (C-84): `plan-render` `AreaLight` panels are sampled directly and can be set in the Ray Trace window, and an Emissive class material glows (C-61c); the plan's Emissive materials do not become area lights, and there is no "Apply Emissive and Color from Light(s)" switch |
| 1101 | Every material has a Materials List calculation method (piece, area, volume, none) | Missing | NO SPEC (C-85): the Materials List tab holds price and unit only; the take-off adds surface areas (C-61f) |

### 34.2 Material Defaults

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1101 | Edit > Default Settings > Materials > Edit opens the Material Defaults dialog: default materials for library objects without a defaults dialog, and seeds for the door, window and cabinet defaults | In progress (Round 15) | C-60, `tools/materials/defaults.rs` (Default Settings > Materials) works today for six classes only (Wall, Door, Window, Cabinet, Roof, Room); the defaults_pages brief rebuilds the Default Settings tree with a Materials group |
| 1102 | Material defaults are dynamic: changing one updates every object that uses the default | Works | C-60, DECISIONS 129 (own paint, then class default, then built-in); test `class_defaults_paint_unpainted_objects_and_the_wall_paint_wins` |
| 1102 | Named default categories shared across dialogs (Room Moldings for base, crown, chair rail and casing; Cabinet, Cabinet Door/Drawer, Countertop and Hardware for the cabinet defaults) | Missing | NO SPEC (C-86): defaults.rs has no moldings, countertop, hardware or door/drawer categories; the Cabinet class holds the cabinet parts |
| 1102 | Material Defaults dialog: scrollable category list, Shift/Ctrl multi-select, Select Material button or either preview box opens Select Material | Partial | NO SPEC (C-86): `tools/materials/defaults.rs` shows class and part rows with a picker per row; no multi-select, no preview boxes (same row as the categories gap) |

### 34.3 Material Painter tools

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1102 | The Material Painter works in every rendering technique except Glass House | Works | `tools/materials/paint.rs` picks through the 3D pick hook in any live technique (verify in Chief for the Glass House exception) |
| 1103 | Painting a room's floor or ceiling finish adds that material to the room's Floor/Ceiling Finish Definition; painting a deck's planking or framing edits its floor structure definition | Partial | R-36 floor_finish and ceiling_finish; DECISIONS 129 writes Room class defaults to the floor settings; deck planking and framing paint is not tied to a structure definition (verify in Chief) |
| 1103 | The painter becomes active from 3D > Material Painter, by clicking a material in the Library Browser while a 3D view is active, or when the Eyedropper is used | Works | `tools/materials/browser.rs` (a click makes a material active), menus.rs, `PAINTER`/`EYEDROPPER` |
| 1103 | Select Material dialog opens when the painter starts; Use Default Material check box (library panel) or "Use Default" row (plan panel) paints the default | Partial | NO SPEC (C-87): C-56: the palette window and its Use Default Material button; there is no modal Select Material dialog |
| 1103 | Copy Selected Material button: copies the chosen material, opens Define Material and loads the copy into the painter | Missing | NO SPEC (C-88): the Material Specification saves to My Materials under the same name (C-58); no copy-and-paint button |
| 1103 | Status Bar names the material being painted and the material under the pointer; roller cursor in blend mode, spray-can cursor otherwise | Partial | `tools/materials/paint.rs` sets a status message on pick; cursor badges follow APP-73 (not built) |
| 1103 | Material Eyedropper loads a surface's material into the painter | Works | C-57 |
| 1104 | Five scoping modes: Component, Object, Room, Floor, Plan, which replace instances of the clicked surface's material across the chosen extent | Works | C-56, DECISIONS 126 (ours paints the extent and narrows it with a Scope dropdown; Chief replaces instances of one material; verify in Chief) |
| 1104 | The same five scoping modes appear on Adjust Material Definition and the Interactive Material Editor | Missing | NO SPEC (C-89): Adjust opens the clicked material's specification without a scope; no mode buttons |
| 1104 | Blend Colors With Materials mixes a solid color into a textured or patterned surface in all five modes | Works | C-56 `blend_colors_mixes_the_active_material_into_the_current_one`, `plan_materials::blend_materials` |
| 1104 | On walls, Component and Object modes paint only the section of wall that defines the room clicked; Room mode takes the walls and solid railings with the same original material; Floor and Plan modes take all walls and railings | Partial | DECISIONS 126 (wall face chosen by the room test; Room mode walls on the room outline); railings are not painted by scope (verify in Chief) |
| 1105 | Wall materials applied with the Painter are cosmetic and are not counted in the Materials List (only foundation wall footings are) | Differs | DECISIONS 131 and C-61f count painted wall faces in Materials List by Surface; a deliberate extra |

### 34.4 Materials panel of specification dialogs

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1105 | Most specification dialogs have a Materials panel with a table of components, their materials and "Default:" prefixes, Shift/Ctrl multi-select, and rename of a library symbol's component | Partial | Materials tabs in wall, door, window, stairs, room, roof, fireplace and library-symbol dialogs (DW-117, R-36); one drop-down per component, "Default (usual look)" wording, no multi-select, no component rename |
| 1105 | Caution symbol on the panel name and "Texture Missing" in the preview when a material's texture file is missing | Missing | NO SPEC (C-90): `tools/materials` shows no missing-file warning |
| 1106 | Two preview boxes show the color, pattern and texture of the selected component; clicking either opens Select Material | Partial | a color swatch only (`swatch` in `tools/materials.rs`) |
| 1106 | Panel preview: Standard to see textures, Vector View to see patterns, with the dialog preview controls | Partial | APP-86 and APP-87 (dialog preview buttons and material preview shapes) |

### 34.5 Select Material / Select Library Object dialog

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1106 | Select Material dialog with a Library Materials panel, a Plan Materials panel and (only for a library symbol's component) a Material Defaults panel | Missing | NO SPEC (C-87): a drop-down list per component; no modal dialog (see the new row above) |
| 1106 | Library Materials panel: a modal Library Browser with search, filters, tags, folder tree, preview with Cube, Sphere, Teapot or Plane and a Default Room or backdrop behind it | Partial | Library dock Objects/Materials switch with search and category filter (C-61e, `tools/materials/browser.rs`); no preview shapes or backdrop choice |
| 1107 | Library panel Settings menu, Tile Mode, Search Subfolders | In progress (Round 15) | CB library rows; the Round 15 library brief covers the Library Browser dock (search, filter bar, preview pane); the Library dock Materials view has search and categories today |
| 1108 | Add New buttons under the folder list: Symbol, Line Style, Fill Style, Backdrop, Material (saved in the User Catalog) | Partial | Save to My Materials (C-61e); the other four are not offered from a picker |
| 1108 | Plan Materials panel: list of materials already in the plan, with "Use Default" at the top | Missing | NO SPEC (C-91): no Plan Materials list (new row below) |
| 1108 | Material Defaults panel: tie a library symbol component to a Material Defaults category | Missing | NO SPEC (C-87): symbols paint by name (Symbol Specification Materials tab, DECISIONS 132) |
| 1108 | Rendering technique, object shape and backdrop chosen in Select Material, Plan Materials and Define Material are shared between those dialogs (shape and backdrop also with the Library Browser) | Missing | NO SPEC (C-92): no preview technique, shape or backdrop controls anywhere |

### 34.6 Editing materials

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1108 | Material definitions are plan-specific: editing a material changes only the current plan, not the library or other plans | Differs | NO SPEC (C-93): materials live in the user library by name (`Project::object_materials` stores names); a plan carries no copy of the definition, so a plan opened elsewhere loses custom materials |
| 1108 | Three ways to customise: edit the original and replace all instances in the plan, edit a copy for some instances, edit the library item | Partial | C-58: OK saves a copy to My Materials under the same name, Revert to Library deletes it; no per-instance copy choice |
| 1109 | Adjust Material Definition: click a surface, choose a scope mode, answer the Create Copy of Material prompt (Create a Copy with a name, or Edit the Source Material), edit in Define Material | Partial | NO SPEC (C-89): C-58 opens the Material Specification of the clicked surface's material; no scope buttons, no copy prompt |
| 1109 | Interactive Material Editor: an axis polyline at the click with handles for scale (round), Y scale and X scale (square), rotation (triangle) and offset (move); Tab on a handle opens an exact-value box | Missing | NO SPEC (C-94): C-59 gives live preview while the specification is open, not on-surface handles |
| 1110 | Editing with the Interactive Material Editor changes the plan's copy of the material only, not the library | Differs | see the plan-specific definitions row |
| 1111 | Five editing scoping modes (Component, Object, Room, Floor, Plan; Plan is the default and edits the source) | Missing | NO SPEC (C-89): see the scoping-modes row |
| 1111 | Plan Materials dialog and right-click > Open Object in the User Catalog open Define Material for editing | Partial | library dock Edit button opens the specification (C-61e); Plan Materials dialog Missing |

### 34.7 Creating materials

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1111 | New materials copy their texture and map files into the program data folder (Textures/Material Data) so they stay with the program | Partial | package import copies maps to `~/.plan-studio/textures/<package>/` (BC-PKG2); a texture chosen on the Texture tab keeps its original path |
| 1111 | Plan Materials dialog New and Copy buttons; Add to Library | Missing | NO SPEC (C-91): see the Plan Materials row |
| 1112 | Right-click an unlocked library folder > New > Material | Partial | Library dock Materials view creates and saves My Materials (C-61e) |
| 1112 | Paste Image: copy an image, then Edit > Paste to make a material from it | Missing | NO SPEC (C-95): no image-clipboard paste into the library |
| 1112 | Screen Capture to a material (Stretch to Fit by default) | Missing | NO SPEC (C-95): no screen capture tool (see the Screen Capture rows) |
| 1112 | Blending a color with a textured material makes a new plan material named after the textured one with a "--PAINTED:" suffix and the color material name | Works | `plan_materials::blend_name`, `BLEND_PREFIX` (naming follows Chief; verify in Chief) |
| 1113 | Tools > Color Chooser makes a custom color to turn into a material | Works | menus.rs Color Chooser; materials take a color in the Material Specification |
| 1113 | Convert Textures to Materials: a whole folder of texture images becomes a library of materials with the same folder structure | Missing | NO SPEC (C-96): Lightbeans zips import one package at a time (BC-PKG3); no folder conversion |
| 1113 | Create Plan Materials Library: a library folder made from all materials used in the plan | Missing | NO SPEC (C-96): no such command |

### 34.8 Materials and the Materials List

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1113 | Structure type of a material (Concrete, Masonry, Other, Framing) decides brick ledges, rebar, steel mesh and pour counts | Missing | NO SPEC (C-85): no structure type on `MaterialDef` |
| 1113 | Calculation methods Area, Count, Linear, Volume, None; Count uses width, height and depth; Linear is strip length; None hides the material | Missing | NO SPEC (C-85): Materials List by Surface (C-61f) adds areas only |
| 1114 | Representing overlap: set the pattern to the exposed size, the texture to the rendered size and the Materials List panel to the full piece size, without Update from Pattern | Missing | NO SPEC (C-85): no such per-material sizes |
| 1114 | An Air Gap layer is neither counted nor drawn in 3D | Partial | W-138 (wall layer role Air Gap); verify the 3D effect |

### 34.9 Managing plan materials

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1114 | Plan Materials dialog (3D > Materials > Plan Materials): search box, sortable list of the materials in the plan, In Use column | Missing | NO SPEC (C-91): no dialog; the Materials window lists library and plan materials without usage counts (`dialogs/materials.rs`) |
| 1115 | Buttons Edit, New, Copy, Purge (remove unused), Delete (unused only), Merge (many into the first), Add to Library, Replace (swap a material, defaults included, with a library one) | Missing | NO SPEC (C-91): same |
| 1116 | Preview with Physically Based, Standard or Vector View, zoom and orbit, Color on/off, Restore Original View, Cube, Sphere, Teapot, Plane; choices persist after closing | Missing | NO SPEC (C-92): no material preview pane beyond the sphere swatch |
| 1116 | Keep the total below 1024 materials per plan | Differs | no limit in our plan; a plan holds names only |

### 34.10 Define Material dialog

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1117 | Open it from Adjust Material Definition, Plan Materials (Edit, New, Copy), a User Catalog Open Object (multi-select hides some settings) or New > Material | Partial | Adjust Material Definition and the library dock Edit; no multi-select edit |
| 1117 | Manufacturer catalog materials show fewer options | Out-of-scope | vendor catalogs (CB-83) |
| 1117 | Add to Library button at the bottom | Works | OK saves to My Materials (`tools/materials/spec.rs`) |
| 1117 | Four panels: Pattern, Texture, Properties, Materials List | Works | `tools/materials/spec.rs` tabs Pattern, Texture, Properties, Materials List (C-61) |
| 1118 | Pattern panel: material name (renaming redefines it) | Works | Name field in the Material Specification |
| 1118 | Pattern panel colors: Material Color, pattern Line Color, Line Weight, Shading Contrast (used when the Vector View option Apply Shading Contrast is on) | Partial | NO SPEC (C-97): Material Color is on the Properties tab; Line Color, Line Weight and Shading Contrast are missing |
| 1119 | Pattern Type drop-down with Library choice, custom patterns, Pattern from Texture and Add Pattern to Library | Partial | NO SPEC (C-97): a CAD-pattern drop-down (`PATTERNS`) only; no library or custom patterns |
| 1119 | Pattern scale: Width, Height (brick, tile, shingles, U's), Spacing (concrete, sand), X/Y Scale for imported patterns, Retain Aspect Ratio | Partial | one Scale multiplier (`pattern_scale`) |
| 1119 | Pattern offset and angle: Row Offset, Horizontal and Vertical Offset, Angle counter-clockwise, Global Symbol Mapping | Partial | Angle only (`pattern_angle`); Global Symbol Mapping under the sync row |
| 1120 | Pattern copyright line and a square pattern preview | Partial | the hatch preview exists (`hatch_preview`); no copyright field |
| 1120 | Dialog preview pane: orbit, zoom, technique (Physically Based, Standard, Vector), Rotate Spherical Backdrop, Mouse Orbit, Color, Restore Original View, shapes, backdrops | Missing | NO SPEC (C-92): see the preview pane row |
| 1121 | Texture panel: material name; Texture Source path with Browse, Remove, .jpg/.bmp/.png/.gif/.tif and textures inside .zip files | Partial | NO SPEC (C-98): picture file with Browse, a Generated button and a search of Chief's texture folders (C-61b); PNG and JPEG read, no TIFF, BMP or GIF, no zip |
| 1121 | Texture Scale X and Y, Stretch to Fit, Retain Aspect Ratio, Reset Original Aspect Ratio | Partial | NO SPEC (C-81): Tile size width and height (C-61b); no Stretch to Fit, no aspect lock, no reset |
| 1122 | Texture Offset X and Y, Angle, Global Symbol Mapping | Partial | offset and angle work (baked into the bitmap, DECISIONS 130); Global Symbol Mapping Missing (shared row) |
| 1122 | Material Color on the Texture panel: Blend with Texture, Color button, Set Material Color Using Texture | Partial | NO SPEC (C-99): blend color and amount (C-61b); no "use the texture's predominant color" button |
| 1122 | Bump, Normal and Ambient Occlusion maps on the Texture panel, each with a name, a Remove button and (except normal and AO) Invert | Partial | Bump slider plus package maps with on/off switches (BC-PKG8); the dedicated invert and remove are missing (see the Invert row) |
| 1123 | 360 panorama / file path pickers for map files (Select/Import with project management, Browse/Edit Path otherwise) | Partial | Browse for a picture; no Edit Path box |
| 1123 | Texture preview square and the dialog preview pane | Partial | no texture preview square (see the preview pane row) |
| 1123 | Properties panel: a Material Class list of ten classes (General, Matte, Mirror, Plastic, Polished, Predefined Metal, Shiny Metal, Translucent, Transparent, Water) | Partial | NO SPEC (C-100): seven classes: General, Plastic, Metal, Glass, Mirror, Emissive, Transparent (`MaterialClass`); Matte, Polished, Predefined Metal, Shiny Metal, Translucent and Water are missing |
| 1125 | Class settings: Apply Emissive and Color from Light(s), Diffuse, Emissive presets and value, Metal type, Metallic, Opacity Map, Reflection (+color), Roughness (+map), Thin, Transparency | Partial | NO SPEC (C-100): sliders for Roughness, Metalness, Transparency, Emissive and Bump (C-61c); no Diffuse, Reflection color, Thin, Metal type or emissive presets |
| 1126 | Water class: wave style (Basic, Detailed, Ripply, Flowing Sheet), Wave Chop, Wind Speed, Wind Direction, Wave Scale | Missing | NO SPEC (C-100): no Water class |
| 1126 | Material Class Mix for General materials: Metal Map, Translucent Map with Translucency, Transmission Color Map, Transmission Roughness (+map), Transparent Map, Index of Refraction | Missing | NO SPEC (C-82): see the maps row |
| 1127 | Clear Coat (ray-traced): roughness, roughness map, normal map, use base normal or bump | Missing | NO SPEC (C-101): no clear coat |
| 1127 | Brushed (ray-traced): anisotropy, anisotropy map, rotation, rotation map | Missing | NO SPEC (C-101): no brushed finish |
| 1128 | Materials List panel: structure type, calculation method, Width, Height, Depth, Update from Pattern | Missing | NO SPEC (C-85): the tab holds Manufacturer, Supplier, Price, per unit and Accounting code (ours, C-61d); none of Chief's fields |
| 1128 | Manufacturer, supplier, price and accounting code fields on the material | Differs | C-61d: ours; Chief keeps cost in the Materials List specification (part 6) |

### 34.11 Pattern from Texture dialog

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1129 | Pattern from Texture: Source map and file, Simple threshold, Advanced Low/High thresholds, Auto Compute Thresholds, Use Threshold Factor (1-10), Filter Size, Original and Output previews | Missing | NO SPEC (C-102): no edge-trace of a texture into a CAD pattern |

### 34.12 Material Builder and the Masonry/Stone, Tile and Wood builders

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1131 | 3D > Material Builder opens a dialog built on Substance: pick a builder, set inputs, set outputs, Add to Library | Differs | NO SPEC (C-103): "Material Builder" in our 3D menu opens the Material Specification dialog (C-61); no parametric builders |
| 1132 | Builder choices: Masonry and Stone, Tile, Wood, or an external Substance (.sbsar) file | Missing | NO SPEC (C-103): see the builder row; the .sbsar import is the vendor format |
| 1132 | External Substance (.sbsar) file import | Out-of-scope | Adobe Substance engine |
| 1132 | Material Outputs: maps included, name, Material Scale (default 20 inches), Open Material When Added to Library; Reset to Defaults | Missing | NO SPEC (C-103): same |
| 1133 | Masonry and Stone builder inputs: output size, random seed and Randomize, shape, pattern layout (1-3 colors, checker or random), stone fill including custom image, roughness, invert, scale, cleavage, roundness, wany edge, row offset | Missing | NO SPEC (C-103): procedural stone and brick exist as texture kinds (`ProceduralKind::Stone`, `Brick`) but have no input dialog |
| 1134 | Masonry and Stone margins and stone groups: margin fill, width, hue, saturation, lightness; up to three stone groups each with color, hue, saturation, lightness; maps Texture, Roughness, Normal | Missing | NO SPEC (C-103): same |
| 1136 | Tile builder: shape, layout, row offset, tile edge bevel, grout color and width, up to three tile groups with material, invert, scale, color, roughness, finish, hue, saturation, lightness; maps include Metal | Missing | NO SPEC (C-103): `ProceduralKind::Tile` has a grout color only |
| 1138 | Wood builder: species (or image), cut/board layout, board color variation, roughness, orientation, stain color and amount, weathering (age, paint level and color, lichen, mold) | Missing | NO SPEC (C-103): `ProceduralKind::Wood` has grain color and ring spacing only |

## 35. 3D Cladding (pp. 1140-1145)

### 35.1 What 3D cladding is, creating and editing it

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1140 | 3D Cladding: a material that uses real 3D profiles (and optional randomised molding symbols) to give siding and roofing depth on structural layers | Missing | C-73 (no cladding in `plan-3d`; the word appears only in elevation material labels) |
| 1140 | Create from Adjust 3D Cladding on a layer with a regular material, from a Material Layers Definition layer whose role is changed to 3D Cladding (Edit button), or from a Wall Type Definition layer's Edit Layer > role 3D Cladding > Edit | Missing | NO SPEC (C-104): W-138 lists role 3D Cladding in the wall layer dialog but nothing builds it |
| 1140 | Edit by the Adjust tool, by the layer definitions, or Open Object on a User Catalog cladding | Missing | NO SPEC (C-104): same |
| 1141 | Cladding definitions are plan-specific | Missing | same |
| 1141 | Adjust 3D Cladding tool: click a surface in a camera view, choose a scope mode, edit in the specification dialog; status bar names the cladding | Missing | C-73 |
| 1141 | Interactive 3D Cladding Editor: an axis polyline with a Move handle (offsets) and a Rotate handle (Row Angle); Tab opens exact entry | Missing | NO SPEC (C-105): no spec; the cladding itself is missing |
| 1142 | Five cladding scoping modes (Component, Object, Room, Floor, Plan) | Missing | NO SPEC (C-105): same |
| 1142 | Generate Random 3D Moldings: number of variations, HSL color range, vary texture offset horizontally or vertically | Missing | NO SPEC (C-106): no spec |

### 35.2 3D Cladding Specification dialog

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1143 | General panel, Profiles table: name, Width, Height, Repeat Distance (piece length that drives the Materials List count), Horizontal Offset, Vertical Offset, Retain Aspect Ratio; buttons Add New, Make Copy, Edit (object information and schedule), Replace, Default, Delete, Add to Library | Missing | NO SPEC (C-107): no spec (the dialog does not exist) |
| 1144 | Selected Profile Options: preview with position indicator, Vertical Position, Auto Lap, Profile Rotation, Reflect Horizontal and Vertical, Count Components in Materials List | Missing | NO SPEC (C-107): same |
| 1145 | 3D Cladding Options: Library Name, Row Overlap/Gap, Angle Rows, Offset Rows Up, Offset Even Rows In, Randomly Offset Texture U and V, Randomly Distribute 3D Moldings | Missing | NO SPEC (C-107): same |
| 1145 | Materials panel: one material per profile component, including generated random moldings; Add 3D Cladding to Library | Missing | NO SPEC (C-107): same |

## 37. 3D Views (pp. 1146-1200)

### 37.1 Types of 3D views

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1146 | Three view categories: camera views, overviews, cross section/elevation views; every view is orthographic or perspective (cameras always perspective, sections always orthographic, overviews either) | Partial | C-10..C-21; the perspective overviews exist, the orthographic overviews do not (C-15) |
| 1146 | New perspective views default to the Standard technique, orthographic views to Vector View | Works | `dialogs/camera.rs` elevation cameras open in Vector View; perspective cameras open Standard |
| 1147 | Rendered views use textures and lighting; Vector Views use edge lines, patterns, limited shadows and are suited to layout and print | Works | C-45..C-47 |

### 37.2 3D view defaults and camera defaults dialogs

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1147 | Eleven camera defaults dialogs (Full Camera; Perspective Full, Floor, Framing Overview; Cross Section/Elevation; Back Clipped Cross Section; Wall Elevation; Orthographic Full, Floor, Framing Overview), opened from Default Settings or by double-clicking a tool button | In progress (Round 15) | Default Settings > Camera Tools has nine stored pages (`dialogs/default_pages/camera.rs`; Doll House and Glass House pages are ours, the fields and the "Sketch" technique are invented); the defaults_pages brief rebuilds the tree; double-click on the tool button does not open them |
| 1148 | Each defaults dialog has the same settings as its specification dialog, and several can be edited together with Shift/Ctrl | Missing | NO SPEC (C-108): the pages hold a few invented fields (eye height, angle, technique, depth) not the specification dialog's panels |
| 1148 | Full Camera Defaults > Reflections is also what walkthroughs use | Missing | NO SPEC (C-109): no Reflections setting |
| 1149 | 3D View Defaults dialog: Camera Bumps Off Walls (walk up stairs), Turn Automatically near walls, Legacy Compatible Texture Mapping, Auto Adjust Electrical Default Glass Properties, Always Display Active Cameras, Display Openings Independent of Walls and Roofs | In progress (Round 15) | NO SPEC (C-110): C-68 covers the dialog name; ours (`defaults_window` in `view3d_panel.rs`) has eye height, angle of view, technique and the callout shape, size and name, none of Chief's six options; the defaults_pages brief adds a 3D View Defaults group (stored fields where the model has none) |
| 1150 | Surface Edge Lines for Vector Views: Use Layer Settings, Use Object Settings | Missing | NO SPEC (C-110): line weights come from layer pens only for opted-in cameras (C-47, C-69) |
| 1147 | Generic Sun Defaults dialog (initial sun intensity, color, angle for new cameras) | Missing | NO SPEC (C-111): `Project.lighting` is plan-wide; no defaults dialog |
| 1147 | Layer Set Defaults: which layer set each 3D view tool starts with | Missing | NO SPEC (C-112): no layer set per 3D view (layer_sets.rs has plan views only) |
| 1147 | Rendering Technique Defaults dialog: initial options of each technique | Missing | NO SPEC (C-113): no technique options at all |

### 37.3 3D view tools

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1150 | Camera views (Full Camera) and overviews (Full, Floor, Framing, Isometric) and cross section/elevation views (Cross Section/Elevation, Back Clipped, Wall Elevation, Auto Elevations, Room Elevations) | Partial | C-4, C-10..C-21; ours also has Floor Camera, Doll House and Glass House views (beyond Chief's list) |
| 1150 | Perspective Full Overview | Works | C-10 |
| 1150 | Perspective Floor Overview (current floor, ceiling removed, lower floors visible) | Works | C-11, C15-7 (`CameraView.floors`) |
| 1151 | Perspective Framing Overview (framing and foundation only, 3D Framing layer set, offers to build framing) | Partial | C-12 (`CameraKind::FramingOverview` exists; the scene already shows built framing) |
| 1151 | Orthographic Full, Floor and Framing Overviews | Missing | C-15; `CameraMode` has elevations and a plan overhead only |
| 1151 | Isometric Overviews (axes at 120 degrees, about 1.2 scale, only from an orthographic overview; moving the camera drops them) | Missing | C-15; DECISIONS 41 notes the angled orthographic camera is not built |
| 1151 | Cross Section/Elevation view (all floors) | Works | C-17, C-18 |
| 1151 | Back Clipped Cross Section | Works | C-19 |
| 1151 | Wall Elevation (one wall of the room the camera is in) | Works | C-20 |
| 1151 | Auto Elevation tools: Front, Back, Left, Right and All Elevations as separate tools | Partial | NO SPEC (C-114): C-21: Auto Elevations makes the four at once; Auto Back-Clipped and Auto Interior as well; the single-side tools are not offered |
| 1151 | Create Room Elevation Views edit tool on a selected room | Partial | C-21 Auto Interior Elevations is a camera tool clicked in a room; an edit button on a selected room is not verified |

### 37.4 Creating camera views and overviews

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1152 | Click and drag: click sets the camera, the drag sets the line of sight, release sets the focal point; default field of view 55 degrees | Partial | C-4, C-5 (ours opens at 60 degrees; the manual says 55: a disagreement with C-5) |
| 1152 | Right-click and drag creates the camera with the Alternate rendering technique (all but overviews) | Missing | NO SPEC (C-115): no Alternate technique in the camera defaults |
| 1152 | A camera needs a drag at least as long as the Snap Distance | Partial | `tools/camera.rs` refuses a section drag under 12 in (`MIN_SECTION_LEN`); Chief uses the current Snap Distance, so the threshold differs |
| 1152 | Copy and paste a camera symbol makes a new camera | Works | C-29 |
| 1152 | A camera view from one frame of a Walkthrough Path preview | Missing | NO SPEC (C-116): no Create Camera View in the preview |
| 1152 | Camera height default 60 in (1500 mm), measured from the room's subfloor, the terrain inside the terrain perimeter, or the default subfloor | Partial | C-5 uses 66 in from the floor; terrain-relative height is not verified (disagrees with C-5) |
| 1153 | Overviews generate at once at a fixed 120 degree angle, focal point at the model center ignoring terrain; the camera stands back by model size | Partial | C-10 (our overview orbit angle and distance rules are ours; verify in Chief) |
| 1153 | An overview gets a plan symbol and can be copied and pasted | Differs | DECISIONS 166 and C-14: ours has no overview symbol in the plan |
| 1153 | Floor Overview via Up/Down One Floor, or only the current floor via the camera defaults | Works | C15-7, `view_settings::scope_of` |
| 1154 | Isometric Overviews menu and rescaling | Missing | see the Isometric row (C-15) |
| 1155 | Cross section/elevation lines and dimensions are true length; only these views take the 2D CAD tools; they can be annotated, sent to layout, printed to scale | Partial | vector elevations print to scale and go to layout (L-5); CAD tools, text and dimensions drawn on them are not saved with the view (new row below) |
| 1155 | Click-drag a section, back-clipped or wall elevation; wall elevation must be clicked inside a room; section line and back clipping line show | Works | C-17..C-20 |
| 1156 | Objects that the line of sight does not cut and that are inside the back clip (a window in elevation) keep their 3D definition and can be selected, moved and stretched in the view | Missing | NO SPEC (C-117): elevation views are drawings (`plan_elevation::Drawing`), not editable 3D |
| 1156 | Auto Elevations need a wall or railing; direction follows the axes and Rotate Plan View; repeats are numbered and spaced progressively | Partial | C-21; repeat numbering unverified |
| 1156 | Section callouts show the layout page label as Text Below Line once sent to layout | Works | C-24 |

### 37.5 Displaying 3D views

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1157 | Each view window shows the plan name, view type and saved name in its title bar; any number of windows can be open | Partial | C-3 (one 3D panel; views are tabs per kind) |
| 1157 | Rebuild 3D regenerates the model without closing views | Works | C-54 |
| 1157 | Layer Display Options per view, with a layer set per view type (Camera View, Section View, 3D Framing set); "CAD, Clip Lines" layer | Missing | NO SPEC (C-112): the layer set is per plan view only; layers apply to the 3D model through the scene filter (`layer_sets.rs`) |
| 1157 | Plan symbols: a camera symbol shows position, direction, field of view, line of sight, focal point and a letter for its technique; a section symbol shows location, clip plane, line of sight and back clip | Partial | NO SPEC (C-118): C-24, C-28 (symbols and clip line); the technique letter (S, V, Ph, C, G, T, W, H, D) is not drawn |
| 1158 | A camera or section can show as a callout (label and arrow) instead of the symbol | Works | C-24 (`callouts_on`) |
| 1158 | Camera symbols sit on the Cameras layer; Always Display Active Cameras keeps the active symbol visible with the layer off | Partial | the layer exists; the preference is not built (see the 3D View Defaults options row) |
| 1158 | Full Overview, Cross Section and Back Clipped symbols show on all floors; Full Camera, Floor Overview and Wall Elevation only on their floor; a per-camera Display on All Floors | Partial | NO SPEC (C-119): `CameraView.show_in_plan` only; no all-floors flag |
| 1158 | Move a camera to another floor with Up One Floor/Down One Floor while its view is active | Missing | NO SPEC (C-120): no floor change for an open camera |
| 1158 | Camera labels on the "Cameras, Labels" layer; numbered in creation order; the label uses the camera name; saved callouts have label and extra text | Works | C-24, Label tab (`CameraLabel`) |
| 1159 | Camera symbols are screen-only (printed only through Print Image), included in CAD Detail from View and DXF; callouts print, go to layout and export | Partial | callouts are on layout; symbols in a detail unverified |
| 1159 | Rendered views: View > Color toggles color, Rendering Technique, Adjust Lights, Toggle Patterns | Partial | ViewFlag::Color (View menu), techniques C-45, lights C-64; Toggle Patterns Missing (C-74) |
| 1159 | Vector views: Patterns, 3D Views layer, color toggle, line style by layer or per object, pattern line weight and color from the material, Show Line Weights | Partial | C-47, C-69; pattern lines in 3D vector views are not drawn |
| 1159 | Backdrop or background color behind the model | Partial | C-70 |
| 1160 | Delete 3D Surface: click a surface to remove it from every 3D view (not from the model); Alt removes one triangle; again restores the last; Rebuild Walls/Floors/Ceilings, reopening or a new view restores all | Partial | C-55 (a paint override clears a surface's paint; no face hiding, no triangle mode, no Rebuild restore) |

### 37.6 Navigating in camera views

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1160 | Incremental Move Distance (pan and forward/back dolly) and Incremental Rotate Angle (orbit, tilt, side dolly) are set per camera; Shift while stepping holds the redraw | Missing | NO SPEC (C-121): DECISIONS 41: fixed 24 in, 15 and 5 degree steps; Chief keeps the increments in the Camera Specification (disagrees with DECISIONS 41) |
| 1160 | Ctrl/Cmd+Alt+S spins a camera view or overview; Esc stops | Missing | NO SPEC (C-122): no auto-spin |
| 1161 | Wheel and trackpad pan and zoom with any tool; zoom speed follows the distance to the object under the pointer, Shift speeds up and Ctrl/Cmd slows; pan speed follows the line of sight | Partial | NO SPEC (C-123): wheel dolly and pan work (C-36, C-37); modifier speeds are not verified |
| 1161 | Zooming moves the camera and does not change the clipping distance, so close objects can drop out | Missing | NO SPEC (C-124): no Clip Surfaces Within value |
| 1161 | 3Dconnexion 3D mice and gamepads | Out-of-scope | input hardware |
| 1162 | Mouse-Orbit Camera: left drag orbits about the focal point, right drag tilts, release while moving to "throw" the view, Mac Option+Shift temporary orbit | Partial | C-34, C-35 (orbit works; right-drag tilt, throw and the temporary modifier are missing) |
| 1162 | Mouse-Pan Camera: left drag pans, right drag orbits | Partial | C-36 (pan; the swapped right-drag is missing) |
| 1162 | Mouse-Dolly Camera: drag up/down to move forward/back and left/right to turn; right drag tilts | Missing | NO SPEC (C-125): C-34 (only orbit and pan modes) |
| 1162 | Mouse-Tilt Camera: left drag tilts, right drag orbits | Missing | NO SPEC (C-125): same |
| 1163 | 3D Center Camera on Point: click an object to look at that point without moving the camera | Partial | C-39 (Alt-click sets the orbit centre; no marker) |
| 1163 | 3D Focus on Object and the Focus on Selected edit button | Missing | NO SPEC (C-125): no focus-on-object |
| 1163 | Move Camera with Keyboard modes (Orbit, Pan, Dolly, Tilt) on the arrow keys with the camera's increments | Partial | C-38, DECISIONS 41 (fixed steps; WASD in Full Camera) |
| 1163 | Move Camera tools (forward, back, left, right, up, down; walk stairs when Camera Bumps Off Walls is on) | Partial | 3D > Move Camera steps (`nudge.rs`); no stair walking |
| 1164 | Orbit Camera tools (Move In/Out along the line of sight, Orbit Up/Down/Left/Right with stops; not in section views) | Partial | 3D > Orbit Camera has Left, Right, Up, Down; Move In and Move Out are missing |
| 1164 | Tilt Camera tools (Tilt Up/Down, Turn Left/Right) | Works | 3D > Tilt Camera and the keyboard turn steps (`nudge.rs`) |
| 1165 | View Direction tools: Front, Back, Top, Bottom, Left Side, Right Side, Restore Original View | Partial | NO SPEC (C-126): C-41: eight compass snaps; Top, Bottom and Restore Original View are missing |

### 37.7 Working in 3D views

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1165 | Place windows, doors, cabinets, electrical objects, corner trim and most library objects by clicking in a 3D view, against walls, on floor platforms or inside the terrain perimeter | Missing | NO SPEC (C-127): placing is plan-only; the 3D view selects and drags (C-43) |
| 1166 | Draw custom countertops, roof planes, terrain features and roads in camera views and overviews; Build Framing and Build Roof dialogs from 3D | Missing | NO SPEC (C-127): same |
| 1166 | Select with Select Objects; edit handles on a handle surface (a wall has two resize handles, a cabinet top ten); all moves in the surface plane | Partial | NO SPEC (C-128): C-43 selects and drags across the floor; no handle surface or resize handles in 3D |
| 1166 | Moves in 1 inch steps (changeable) with Grid Snaps; Ctrl/Cmd for unrestricted; temporary dimensions | Partial | 3D drag uses the Select tool's snapping (C-43); temporary dimensions missing (same row) |
| 1166 | Raising the Exterior Room's wall height in 3D changes the floor's default ceiling height | Missing | no wall height handle in 3D |
| 1166 | Several 3D windows rebuild together; fewer windows is faster | Works | one panel |
| 1166 | Edit materials in 3D: Adjust Material Definition, Material Painter, Material Eyedropper | Works | C-56..C-58 |
| 1166 | Text, CAD and Dimension tools in cross section/elevation views; annotations laid over the view and saved with it | Missing | NO SPEC (C-129): elevations carry generated dimensions and labels (C-47) but user CAD, text and dimensions on a section are not saved with the view (CAD Detail from View is the workaround) |
| 1167 | Auto Detail from a cross section; CAD Detail from View for other vector views | Works | L-39, L-40 |
| 1167 | Rich Text, Text and Note tools plus dimensions in camera views and overviews | Missing | NO SPEC (C-130): DIM-62 (dimension lines in camera views) |

### 37.8 Editing 3D views

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1167 | Cross Section Lines: CAD lines Chief draws where the section plane cuts objects, on the locked "Cross Section Lines" layer, replaced on redraw; dimensions to them lock to automatic Point Markers | Missing | NO SPEC (C-131): the cut is drawn as poché regions in the elevation drawing (C-18), not as editable lines |
| 1167 | Open a saved camera from the Project Browser: Open View, Find in Plan, Edit View | Partial | NO SPEC (C-132): Project Browser Cameras list: Restore, Rename, Delete, Send to Layout; Find in Plan and Edit View missing |
| 1168 | Edit a camera in its specification dialog (position, technique, layer set, backdrop) | Partial | C-30; layer set missing |
| 1168 | Camera symbol edit tools (Open View, Send Camera's View to Layout, Set as Default, Create Walkthrough) | Partial | NO SPEC (C-133): Open Object and Send to Layout exist (L-5); Set as Default is missing |
| 1168 | Six camera handles: rotate label, move label, line of sight, move, focal point, rotate (about the center) | Partial | C-25, C-26, C-27: move, aim, clip, wedge handles; label handles and rotate-about-center are missing |
| 1169 | Seven section handles: rotate/move label, move clip plane, two resize clip line handles (left click concentric, right click moves the end and the camera), move, focal point | Partial | C-28 (clip handles); label handles and the two-way resize handle are not verified |
| 1169 | Several cameras can be selected to move, rotate or delete, or get a revision cloud | Partial | moving and deleting several selected cameras is generic Select behaviour (verify for cameras); revision clouds around cameras are not built |
| 1169 | Right-click in empty 3D space: in Vector Views File/Edit/Tools/Window commands, otherwise view tools and toggles | Missing | NO SPEC (C-134): no context menu in the 3D view |
| 1169 | Up One Floor/Down One Floor moves an active camera | Missing | see the floor-change row |
| 1170 | Camera height follows the terrain outside, the room floor inside, and stays relative to its own floor upstairs | Partial | eye height is from the floor; terrain-relative height not verified |
| 1170 | Zoom In, Zoom Out and Fill Window crop the view without moving the camera (or change the field of view with a Render preference) | Partial | C-37 (wheel dolly; fit; no rubber band) |
| 1170 | Field of View indicators in the camera symbol | Works | C-24, C-7 |
| 1170 | Reset Saved Camera (All) and (Position): undo changes to a saved view; a prompt asks to restore or keep when the view closes | Missing | NO SPEC (C-135): no reset command or prompt |

### 37.9 Clipping and annotating in cameras

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1171 | Scene Clipping options of the Cross Section/Elevation Specification control how much of the model is shown | Partial | Section Length and Back Clip Distance only (C-19, C-28) |
| 1171 | Clip Lines on the "CAD, Clip Lines" layer with handles for side and elevation clipping | Missing | NO SPEC (C-136): no clip lines |
| 1171 | Front clipping plane (the Cross Section Line) at the camera; an object it cuts is drawn but not selectable; move it by X and Y or by the handles | Works | C-17, C-28 |
| 1171 | Back clip: a Back Clip After value or the end of the line of sight | Works | C-19 |
| 1171 | Stepped cutting planes: Add Break on the section line, drag the handle beside the break perpendicular for a step and along it to move the step | Missing | NO SPEC (C-137): the section line is straight (`SectionLine { a, b }`) |
| 1172 | Side clipping: Clip Sides with a Clip Width, handle or Clip Lines to change it; default is the full width; 3D > Camera View Options > Clip Sides toggles | Partial | NO SPEC (C-136): the cut line's length always limits the width (`SectionCut.half_width`); there is no full-width default or toggle |
| 1172 | Top and bottom clipping: Clip Elevation with Bottom and Top Clip Elevation, or Clip Lines | Missing | NO SPEC (C-136): no vertical clip |
| 1173 | Add Break and Make Parallel/Perpendicular customise the top and bottom planes | Missing | see the stepped-plane row |
| 1173 | Clip to Room (default for Wall Elevation; Ignore Railings and Invisible Walls; Ignore Walls Above) | Partial | NO SPEC (C-138): Wall Elevations and Auto Interior Elevations back-clip to the room (C-20, C-21); no check box and no ignore options |
| 1173 | Scene Clipping defaults for the section tools are restricted; Set as Default (in a view) updates Wall Elevation, Back Clipped or Cross Section defaults by which boxes are checked | Missing | NO SPEC (C-133): no Set as Default for cameras |

### 37.10 Detailing cross section/elevation views

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1173 | CAD objects, text and dimensions placed on a section are saved with the view and a prompt appears on close | Missing | see the annotation row above |
| 1173 | Cabinet and other object labels display in sections | Partial | material labels and level callouts are options (C-47) |
| 1173 | Auto Elevation Dimension tools make dimension sets and story poles in sections | Partial | DIM-61 (automatic elevation dimensions in the Vector View) |
| 1173 | Annotations use the view's Active Defaults (Selected Defaults panel) | Missing | NO SPEC (C-139): no Selected Defaults in the camera dialog; Tools > Active View > Active Defaults is the plan's |
| 1173 | CAD objects snap to the section's snap grid | Missing | no CAD in sections |
| 1174 | CAD Detail From View makes an editable 2D drawing of a Vector View | Works | L-40 |
| 1174 | Annotated sections can be sent to layout | Partial | sections go to layout (L-5); annotations are not stored |
| 1174 | Auto Detail on a cross section: closed polylines for wall layers with their fill styles, floor and ceiling platform layers, roof and ceiling plane layers, foundation walls, footings and floors, slabs (Concrete fill), material regions and backsplashes; offers the Current CAD Layer or each part's own layer | Partial | L-39, `tools/details/cad_detail.rs` (wall assembly from the wall type layers, framing members, insulation, hatch, notes); platforms, roofs, slabs, regions and the layer choice are unverified |
| 1175 | Using Auto Detail twice duplicates the objects | Works | behaviour is the same in ours (no guard) |
| 1175 | Auto Detail as Insulation draws an Insulation CAD box for a layer; an Air Gap layer gets no object | Works | L-39 (insulation), W-138 (roles) |
| 1175 | Depth Cue: Use Depth Cue, Keep Start/End in Sync, Start and End distances, Fog Opacity, Fog Color; does not affect text, dimensions or CAD | Missing | NO SPEC (C-140): the Backdrop tab has Fog for rendered views (C15-4) but no section/elevation depth cue |
| 1176 | Cross Section Slider dialog: several cutting planes, each with a check box, a position slider and a typed position measured from the first edge cut; keep working while open; saved with the camera; not carried into CAD Detail or plot-line sheets | Partial | NO SPEC (C-141): C-23: one plane along the elevation direction, a slider in the Vector View; no multi-plane dialog, no perspective-camera use, not saved with the camera |

### 37.11 Annotating 3D views

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1177 | Leader Line, Rich Text, Text and most Dimension tools (not Angular, not Auto Elevation Dimensions) in camera views and overviews; notes show too; items live in that view only and need the view saved; a Stationary Walkthrough keeps them | Missing | NO SPEC (C-130): DIM-62 for dimensions; the rest is new |
| 1177 | Selected Defaults for dimensions and text in 3D views from the camera defaults or Edit Active View | Missing | see the Selected Defaults row |
| 1178 | Draw Mode buttons: Draw on Bounding Box, Draw on Surface, Set Offset (Offset from Draw Surface, global, also in Preferences > CAD); a circle with cross hairs marks the target | Missing | NO SPEC (C-142): no drawing surfaces in 3D |
| 1178 | CAD Detail from View ignores text and dimensions of a camera view | Differs | not applicable until annotations exist |
| 1178 | Dimension Selected Edge edit tool in 3D | Missing | NO SPEC (C-142): DIM-62 |
| 1179 | Annotations sit on the layers of the view's Selected Defaults; they show in front unless wholly hidden; not in Glass House; line styles without text; not in Plot Lines layouts | Missing | NO SPEC (C-130): same |
| 1179 | Object labels display in camera views when their layers are on | Missing | NO SPEC (C-143): no labels in the live 3D view |
| 1179 | Editing annotations: dimension lines get two move handles; no move/align/reflect edit tools | Missing | same |
| 1179 | Move Object (Local/Global), Move Object's Label (Local/Global), Rotate Object (Local/Global), Rotate Label (Local/Global) edit tools on a plane or axis | Missing | NO SPEC (C-144): no 3D annotation |

### 37.12 Virtual reality

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1180 | 3D > Virtual Reality with a headset (Windows only; SteamVR; Perspective, Standard technique) | Out-of-scope | VR hardware |
| 1181 | Room View and Overview VR modes, teleport rules (surfaces under 45 degrees), controller commands (teleport, undo, redo, reset, transition, update lighting) | Out-of-scope | VR hardware |
| 1182 | Virtual Reality dialog: status, driver info, line colors, Display Both Eyes, Update Lighting on Teleport, curved teleport line, teleport reference height, overview rotation locks | Out-of-scope | VR hardware |

### 37.13 Saving, exporting and printing 3D views

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1184 | Save Active View, or the Saved box, keeps a camera with its cross section slider, sunlight, shadows and technique settings; it is listed in the Project Browser with a unique name | Partial | C-3, DECISIONS 160 and 166 (technique, quality, backdrop, shadows, lighting override); slider and sunlight position are not saved |
| 1184 | A prompt to save appears when annotations or changed settings are left on a saved view | Missing | see the Reset Saved Camera row |
| 1185 | Open a saved view from the Open View edit button, the context menu, a double-click on the symbol or on the name in the Project Browser | Partial | Project Browser Restore and the symbol's Open Object; Open View button unverified |
| 1185 | CAD Detail from View for any Vector View | Works | L-40 |
| 1185 | Export a 3D view as .bmp, .jpg, .png, .tif or .webp; Physically Based and Clay as .hdr; File > Export > Picture | Partial | NO SPEC (C-145): L-49, DECISIONS 403 (PNG, JPEG, BMP, TIFF); WebP and HDR are missing |
| 1185 | Export 360 Panorama | Partial | C-77 |
| 1185 | Vector Views as .emf metafiles | Out-of-scope | Windows only; the manual says macOS has no metafile support |
| 1185 | Backup Entire Plan keeps images, textures and backdrops with the plan | Partial | part 1 (Exporting a project); material textures are copied only for packages |
| 1185 | Print Image prints any non-vector view as a picture | Works | menus.rs File > Print > Print Image; `dialogs/print.rs` |
| 1185 | Send the view to layout (File > Send to Layout, or Send Camera's View to Layout for a saved inactive camera) | Works | L-5, `dialogs/camera.rs send_camera_to_layout` |

### 37.14 Camera Specification dialogs

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1186 | Open from a camera symbol's Open Object, Edit Active View, or Project Browser > Edit View; the title names the tool | Partial | C-30 (Open Object and a camera menu); the Project Browser Edit View entry is missing |
| 1186 | Panels: Camera, Positioning, Below Grade, Selected Defaults, Plan Display, Backdrop, Layer, Label | Partial | four tabs: Camera, Backdrop, Rendering, Label (`dialogs/camera.rs` TABS); see the Dialog panels table |
| 1187 | General: Name (unique), Saved, Show Color, Show Watermark | Partial | NO SPEC (C-146): Name works; Saved is implicit; Show Color and Show Watermark are missing |
| 1188 | Rendering: technique with Define (Rendering Technique Options), Alternate technique in the defaults | Partial | technique list yes; Define Missing (technique options) |
| 1188 | Show Shadows; Ray Casted Sun Shadows (better, slower, still camera only) | Partial | C-67 (shadows); the path-traced Final View is our high quality (C15-9) |
| 1188 | Reflections, Animate Water, Light Bloom | Missing | NO SPEC (C-109): no per-camera switches |
| 1188 | Ambient Occlusion amount (slider) | Partial | NO SPEC (C-147): SSAO fixed by view quality (`quality.rs`); no per-camera amount |
| 1188 | Upscaling: Sharpening and Super Resolution factors | Missing | NO SPEC (C-148): none in the dialog (the Final View sharpens internally) |
| 1188 | Depth of Field: enable, F-Stop, Focus Distance | Partial | NO SPEC (C-149): aperture and focus in the Ray Trace window (C-51); not a camera setting |
| 1189 | Lighting: Use Sunlight, Adjust Sunlight; Automatic with a Maximum number of lights, or a Light Set with Adjust Lights | Partial | NO SPEC (C-150): C-65 light sets and the lighting override on the Rendering tab; Use Sunlight and Maximum Lights are missing |
| 1189 | Options: Poché, Field of View, Clip Surfaces Within, Show Lower Floors, Hide Camera-Facing Exterior Walls, Extend Terrain to Horizon | Partial | NO SPEC (C-151): Field of view (C-7) and floors displayed (C15-7) work; the rest are missing |
| 1189 | Poché check box on the camera | Missing | NO SPEC (C-152): sections always draw poché |
| 1190 | Positioning: Height Above Floor, Tilt Angle, Camera Angle (absolute; 0 degrees points left in plan), X Position, Y Position | Works | C-30 (Camera tab Position X/Y, Height, Direction, Tilt) |
| 1190 | Navigation: Incremental Move Distance, Incremental Rotate Angle | Missing | see the increments row |
| 1190 | Below Grade panel: Override Color, Style, Weight; applies below the terrain perimeter or an absolute height; lists the object types | Missing | NO SPEC (C-153): no below-grade line overrides |
| 1190 | Selected Defaults panel | Missing | see above |
| 1190 | Plan Display panel: Display on All Floors; Display as Callout (label, Text Below Line with Automatic, size, arrow None/Small/Large, Filled); symbol size, Show Camera Focal Point, Show Field of View Indicators, FOV Indicator Length | Partial | callout on/number/shape/size/name (C-24, 3D View Defaults); the symbol options are missing (see the Plan Display row) |
| 1192 | Backdrop panel: backdrop name with Select Backdrop and Remove Backdrop | Partial | C-70 (a picture by name from Chief's Backdrops folder, a sky color) |
| 1193 | Use Generated Sky with Starlight Intensity, Star Density, Moon Luminance, Moon Intensity, Moon Tilt, Moon Direction, Moon and Sun Angular Radius, Reset | Missing | NO SPEC (C-154): the sky is a gradient (`quality::sky_colors`) with no sun disc, moon or stars |
| 1193 | Spherical Panoramic Backdrop: Horizontal and Vertical Tile, Horizontal Span, Horizontal Offset, Vertical Max and Min, Eye Level | Missing | NO SPEC (C-155): a backdrop is a flat picture |
| 1193 | Background Color: Use Plan/Detail Background or a custom color | Partial | sky color (C-70) |
| 1194 | Layer panel: the layer of the camera symbol | Missing | NO SPEC (C-156): cameras sit on one layer |
| 1194 | Label panel: Suppress Label, position and orientation | Partial | Label tab: text, show in plan, show in view (`CameraLabel`); no position or orientation |

### 37.15 Cross Section/Elevation Specification dialogs

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1194 | Panels: Camera, Positioning, Below Grade, Selected Defaults, Plan Display, Backdrop, Layer, Arrow, Label | Partial | the same four tabs as the camera dialog with a section panel (`elevation_rendering`, `clipping`) |
| 1195 | Camera panel General and Rendering as above, Show Shadows, Ray Casted Sun Shadows | Partial | see the camera rows |
| 1196 | Sharpening (Standard and Watercolor sections), Use Sunlight, Adjust Sunlight | Partial | sun azimuth and altitude for section shadows (C-47, `ElevationRender`) |
| 1196 | Scene Clipping: Poché; Framing Back Clip with Back Clip Framing After; Back Clip with Back Clip After; Clip Sides with Clip Width; Clip Elevation with Bottom and Top; Clip to Room with Ignore Railings and Invisible Walls and Ignore Walls Above | Partial | NO SPEC (C-157): Back Clip After and Section Length only; the other six groups are missing (rows above) |
| 1197 | Positioning: X and Y position only | Works | C-28 |
| 1197 | Below Grade and Selected Defaults panels | Missing | see the camera rows |
| 1198 | Plan Display: Display on All Floors; Display as Callout needed for the symbol to print or export; Placement (Center, Left Side, Right Side, Both Sides, Custom with offset); Callout Label; Text Below Line (Automatic = layout page label); Callout Size; Arrow; Cross Section Line Style and Weight (by layer) | Partial | NO SPEC (C-158): callout show and number (C-24); placement, line style and weight are missing |
| 1199 | Standard Options: Default boxes, symbol size, Show Camera Focal Point, Clip Plane Indicator Length | Missing | same row |
| 1199 | Backdrop panel in sections | Partial | see Backdrop above |
| 1199 | Layer panel with the layer and the Drawing Group | Missing | see the Layer row; drawing groups are LAY-36 |
| 1200 | Arrow panel: an arrow on the clip plane line when callout placement is Left or Right | Missing | NO SPEC (C-158): the Arrow panel is not offered on cameras |
| 1200 | Label panel | Partial | see above |

## 39. 3D Rendering (pp. 1201-1244)

### 39.1 Rendering tips

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1201 | GPU ray tracing for Physically Based and Clay: progressive samples, Maximum Samples when exporting or sending to layout, sample count in the Status Bar, denoise before export | Partial | C-51 (a CPU progressive path tracer with samples, denoiser and block preview; C15-9 Final View refines in the panel); Differs from GPU by design |
| 1201 | Denoise View command swaps the live ray trace for a denoised offline copy and back | Missing | NO SPEC (C-159): the denoiser runs on save (`plan-render/src/denoise.rs`); no view toggle |
| 1202 | DLSS Real-Time Denoising on Nvidia cards | Out-of-scope | vendor GPU feature |
| 1202 | Tone mapping operators in Physically Based and Clay: Hable or ACES | Partial | NO SPEC (C-160): `plan_render::ToneMap` is ACES, Reinhard or Linear; no Hable and no per-view choice in the UI |
| 1202 | Super Resolution: render below native resolution and upscale (not in sections) | Missing | NO SPEC (C-148): see the Upscaling row |
| 1203 | Area Lights are Emissive materials that light Physically Based and Clay views; Light Sets stage lighting per view | Partial | see the emissive area light row; light sets work (C-65) |
| 1203 | Shadows in any camera view except Glass House; sunlight through windows with ray-cast shadows; shadows on by default in Full Cameras; Toggle Shadows in Camera View Options | Works | C-67, DECISIONS 28; a Shadows toggle in the Shading menu |
| 1203 | Sun shadow polylines can show in plan view for a date, time and place | Missing | NO SPEC (C-161): no shadow polylines in plan (see Sun Angle objects) |
| 1203 | Reflections: Mirror material flat surfaces reflect other objects in perspective views; Physically Based also models reflections on other surfaces; Toggle Reflections | Partial | the ray tracer reflects mirror surfaces (C-51); the live GL view has no planar reflections and no toggle |
| 1204 | Material definitions, textures, maps and the Water type control rendered appearance | Partial | see the Define Material rows |
| 1204 | Transparency three ways: General transparency, Transparent class with index of refraction, Translucent | Partial | NO SPEC (C-100): Transparency slider and a Transparent/Glass class (C-61c); no Translucent class, no index of refraction |
| 1204 | Opaque Window/Door Glass option per technique (color from the material or custom), overridable per door | Missing | NO SPEC (C-162): windows and doors keep their glass; no technique option and no per-door override |
| 1204 | Image objects stand for trees and cars with one surface | Works | tools/images.rs, `plan-3d/src/images.rs` |
| 1204 | Backdrop contributes to global illumination in Physically Based and Clay; Generated Sky | Missing | NO SPEC (C-155): the path tracer has its own sky (Preetham); a backdrop picture does not light the scene |
| 1205 | Water Animation of Water materials in Physically Based; Toggle Water Animation | Missing | NO SPEC (C-109): no Water class |
| 1205 | Hand Drawn Lines on Top over most techniques; Toggle Hand Drawn Lines on Top | Missing | NO SPEC (C-163): the Line Drawing technique is a separate look; no overlay |
| 1205 | Hide Camera-Facing Exterior Walls (also hides attic walls, cabinets and symbols on those walls, and their doors and windows); also a Walkthrough Path option | Missing | NO SPEC (C-151): no such view option |

### 39.2 Lighting

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1205 | Lighting works in Standard, Physically Based, Clay, Watercolor and Duotone perspective views | Works | C-62, C-67 |
| 1205 | At most 20 lights render by default (the brightest in the camera's room first); Maximum Lights per camera | Partial | GL uses a fixed count by quality (`quality.rs max_lights`); no per-camera maximum (see the camera Lighting row) |
| 1206 | Six light kinds in Standard views: a Default interior light where a room has none, light fixtures, Added Lights, sunlight, ambient light, area light materials | Partial | `plan_materials::default_room_light`; electrical lights shine (C-64); the rest below |
| 1206 | Light fixtures: an electrical symbol with Specify as Light, with light sources of a type, color, intensity, offset and Show Position | Partial | electrical fixtures emit light when "Use electrical" is on (C-64, `LightSettings`); the Light Data panel is part 3's electrical dialog |
| 1206 | Added Lights (Add Lights tool): Point or Spot; layers "Light Sources" and its labels; need a room; 2D symbols that do not exist in 3D, optional red cross hairs and a blue spot arrow | Partial | NO SPEC (C-164): C-64, C-66: point lights only, a hidden-layer record, no position cross hairs |
| 1207 | Sunlight alone lights exterior daytime views and shines through windows with shadows on; 3D > Lighting > Toggle Sunlight; Use Only Backdrop for Lighting for overcast scenes | Partial | NO SPEC (C-165): the sun is on by default and can be overridden per camera (C-63); no Toggle Sunlight command, no overcast option |
| 1207 | Generic Sun (view-specific; defaults dialog; Sun Follows Camera) or a Sun Angle placed in plan | Partial | the plan keeps one sun with a per-camera override (`Project.lighting`, DECISIONS 164); see the Sun Angle rows |
| 1207 | Move Sun and Move Moon tools (drag, or Ctrl/Cmd+click to place) with the Sun/Moon Direction Indicator in the corner | Missing | NO SPEC (C-166): no tool and no moon |
| 1207 | Three ambient lights: Interior, Daytime and Nighttime; Ambient Occlusion amount per camera | Partial | NO SPEC (C-167): one Ambient level (`Lighting.ambient`, camera override); not three |
| 1208 | Backdrop Intensity day and night in the Physically Based and Clay options | Missing | NO SPEC (C-113): no technique options |
| 1208 | Light types: Point, Spot, Area; any punctual light except a Sun Angle can be Point or Spot | Partial | point only |
| 1209 | Create a point light with a click; a spot light by dragging up to two feet; spot handles Move, Rotate (direction) and Cut Off Angle | Partial | point light by click (C-64); spot lights Missing (new row above) |
| 1209 | Area Light materials: a fixture uses either its punctual lights or its area lights; the area light is on or off as a whole | Missing | see the emissive row |
| 1209 | Light Intensity (constant per light) and Brightness (percent per light set); Adjust Brightness edit tool | Missing | NO SPEC (C-168): the light's Power is the only level; no brightness per set |
| 1210 | Lights display by layer ("Electrical", "Light Sources" and their label layers) | Partial | C-66 (layer name differs) |
| 1210 | Turn Light(s) On/Off in 3D edit buttons, or the object's On box, or the Adjust Lights dialog | Partial | Adjust Lights switches and light sets (C-64, C-65); the edit buttons are missing |
| 1210 | Position indicators (cross hairs) for punctual lights in every technique but Hand Drawn Lines, only when the light is in use | Missing | see the position indicators row |
| 1211 | Light Sets: automatic mode ranks lights by room, intensity, distance; interior lights are off outdoors; Light Sets replace this; sets stay when lights are deleted | Works | C-65, C15-3 (`Lighting.sets`, `a_deleted_light_leaves_every_light_set`) |
| 1211 | Adjust Lights dialog: Current Camera, Lighting Mode (Automatic, Light Set), Light Set controls (select, Modify All Light Sets, New, Copy, Rename, Delete, Reset Brightness), Update Lighting Automatically with Update | Partial | `AdjustLightsDialog` has named sets, add and delete, per-camera set (C-65); Copy, Rename, Modify All, Reset Brightness and Automatic update are not verified |
| 1213 | Light Source Properties table with customisable columns (On, Use Area Lights, Count, Room, Floor, Type, Intensity, Color, In Use, Show Position, Brightness), expandable fixtures with their punctual and area lights | Partial | NO SPEC (C-169): a list with on, color and shadows per light; the fixture tree and the columns are missing |
| 1214 | Adjust Light, Adjust Material and Adjust Brightness buttons | Missing | NO SPEC (C-169): same |
| 1214 | Create/Edit Light Set from Selection edit tool (new set with only the selected lights on, or add them to a set) | Missing | NO SPEC (C-170): sets are made inside Adjust Lights |
| 1214 | Adjust Lights from the Library Browser for selected fixtures | Missing | no such menu |
| 1214 | Adjust Area Light Material dialog: create a copy, edit the source or pick another material; Apply Emissive and Color from Light(s); Emissive preset list; Material Color; texture; Define Material | Missing | NO SPEC (C-171): see the emissive row |
| 1216 | Light Specification dialog for an Added Light: Location (Light Display on all floors or one floor, Absolute Elevation, Display Size), Light Data, Layer, Label | Missing | NO SPEC (C-172): Added Lights edit in Adjust Lights; no specification dialog |

### 39.3 Sun angles and shadows

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1217 | North Pointer tool defines true north for sunlight, conditioned area totals and bearings; several pointers stay in step; default is up | Partial | NO SPEC (C-173): Terrain > North Pointer places one (site symbols); the sun, bearings and the REScheck orientation do not read it |
| 1218 | Sun Angle objects: location (latitude, longitude, time zone) in General Plan Defaults; a date and time per Sun Angle; daylight saving; USNO formulas | Missing | NO SPEC (C-161): `SunDate { month, day, hours, latitude }` for one plan sun; no longitude, time zone or daylight saving |
| 1218 | Create a Sun Angle with CAD > Sun Angle on Floor 0 or 1 or with New in Adjust Sunlight; it can be moved and resized, not rotated; several are allowed but one is lit | Partial | View > Sun Angle toggles a window (`sun_window` in `view3d_panel.rs`); no placed Sun Angle objects |
| 1219 | Sun Angles show date and time in plan and make Sun Shadow polylines (hatched, shaped by terrain) on the "Sun Angles & Shadows" layer; Make Shadow and Delete Shadow edit tools | Missing | NO SPEC (C-161): same |
| 1219 | Stationary Walkthrough for a sun study with Sun Angles | Missing | see the Stationary Walkthrough row |
| 1220 | Sun Angle Specification: Earth Data (date with calendar, time, daylight saving, time zone), Plan View Display (symbol length, Show Date, Make/Delete Shadow, Always Update), Location, Solar Angles (altitude, direction) | Missing | NO SPEC (C-161): the Sun Angle window shows date, time (hours), latitude and the result |
| 1221 | Lighting Data: Casts Shadows, Intensity, Color; Line Style, Fill Style and Arrow panels | Missing | same |
| 1222 | Adjust Sunlight dialog: Use Generic Sun (intensity, color, Sun Follows Camera, tilt, direction, Reset to Defaults) or Use Sun Angle (list, New, Edit, Delete); per key frame Interpolate Sun; Reset to Defaults | Partial | NO SPEC (C-111): 3D > Lighting dialog: azimuth, height, strength, date; per-camera override (C-63); no color, no Sun Follows Camera, no Sun Angle list, no key-frame sun |
| 1224 | Generic Sun default color is pure white | Works | the sun is white |

### 39.4 Rendering techniques and their options

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1224 | Techniques apply to any camera, section or overview; set the default and Alternate in the camera defaults, or change it from the menu | Partial | C-45; no Alternate technique |
| 1225 | Standard | Works | C-46 |
| 1225 | Vector View | Works | C-47 |
| 1225 | Physically Based | Partial | C-51: CPU path tracer in the Ray Trace window and Final View; Physically Based in the live view is the GL look; not in orthographic sections (matches) |
| 1226 | Clay | Works | C-52 (options Missing, below) |
| 1226 | Glass House | Works | C-50 |
| 1226 | Technical Illustration | Works | C-48 |
| 1227 | Watercolor | Partial | C-49 (GL post-process and `plan_render::stylize`; the numbers are ours, C15-5) |
| 1227 | Hand Drawn Lines | Partial | Line Drawing (C-49) is a flat white look with dark lines; no squiggle lines |
| 1227 | Duotone | Partial | C-49 (GL two-tone; not in the path tracer) |
| 1227 | Esc cancels long Watercolor and Hand Drawn Lines renders | Differs | ours draw in real time |
| 1228 | Next/Previous Rendering Technique tools | Missing | NO SPEC (C-174): not on the toolbar or menu |
| 1228 | Rendering Technique Options dialog (3D > Rendering Techniques > Technique Options, double-click the parent button, Define in the camera dialog); updates the view live; saved with a saved view; a Reference version in Change Floor/Reference | Missing | NO SPEC (C-113): no such dialog |
| 1228 | Standard panel: Use Backdrop Image When Available; Opaque Window/Door Glass with a color choice; Interior, Daytime and Nighttime Ambient; Hand Drawn Lines on Top; Reset to Defaults | Missing | same |
| 1230 | Vector View panel: Force Color Off (reference only), backdrop, opaque glass, Shadow Intensity, Apply Shading Contrast | Missing | same |
| 1231 | Physically Based panel: backdrop; Exposure (Automatic or Manual); Tone Mapping Operator; Global Illumination (Opaque Bounces, Transmissive/Specular Bounces, DLSS, Maximum Samples, Daytime and Nighttime Background Intensity, Use Only Backdrop for Lighting); Color Adjustment (Hue, Saturation, Brightness); Hand Drawn Lines on Top | Partial | NO SPEC (C-113): the Ray Trace window sets size, samples, bounces, exposure and tone curve (`RenderSettings`); not saved per camera; no color adjustment |
| 1232 | Clay panel: backdrop, opaque glass, Camera Exposure, exposure mode and tone mapping, Color (layer color, material color or custom with Hue/Saturation/Brightness), Material (ignore bump/normal, metalness, AO map; override roughness), Lights (override color), GI, Hand Drawn Lines | Missing | NO SPEC (C-113): Clay is a fixed matte look |
| 1235 | Glass House panel: backdrop, color, Transparency, Line Thickness | Missing | NO SPEC (C-113): fixed look |
| 1235 | Technical Illustration panel: Warm and Cool colors and blends, Shadow Intensity, Surface Edge Line Style (layer weights or a thickness) | Missing | NO SPEC (C-113): fixed look |
| 1237 | Watercolor panel: Base Technique; Smooth Amount, Paint Contrast, Turbulence Strength and Scale, Pigment Settling Strength, Edge Strength; Hand Drawn Lines on Top | Missing | NO SPEC (C-113): fixed look |
| 1238 | Hand Drawn Lines panel: line color, Thickness (scaled pixels), Extend Amount, Squiggle Amplitude and Frequency | Missing | NO SPEC (C-163): no hand-drawn technique |
| 1239 | Duotone panel: Light and Dark colors, Desaturation, Tone | Missing | NO SPEC (C-113): fixed look |
| 1239 | Rendering Technique Defaults dialog with Restore Initial Settings | Missing | same |

### 39.5 3D backdrops

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1240 | A backdrop picture sits behind 3D views and scales to the window; it also lights Physically Based and Clay views | Partial | C-70 (picture or sky color behind the model); no lighting contribution |
| 1241 | Pick a backdrop by clicking in the view with a library backdrop selected, or with Select Backdrop; backdrops are view-specific; .hdr files for high dynamic range | Partial | NO SPEC (C-175): Backdrop tab picks by name from Chief's Backdrops folder; click-to-apply from the Library and .hdr are missing |
| 1241 | Spherical Panoramic Backdrops wrap round the model; Rotate Spherical Backdrop command; Generated Sky | Missing | see the Spherical row above |
| 1242 | Backdrops are used by default in Perspective Full Cameras and in Standard, Watercolor and Physically Based only, chosen per tool and per technique | Partial | per-camera backdrop (C-70); no per-tool defaults and no per-technique switch |
| 1242 | Add backdrops: File > Import > Backdrop, User Catalog New > Backdrop, paste, screen capture, Create Backdrop Library from a folder | Missing | NO SPEC (C-175): the backdrop list is read from Chief's folder; nothing imports |
| 1243 | Backdrop Specification dialog: name, location (copied into the My Backdrops folder), spherical options (tile, span, offset, vertical max and min, eye level), copyright, preview; .hdr imports as spherical | Missing | NO SPEC (C-175): same |

## 40. Pictures, Images and Walkthroughs (pp. 1245-1284)

### 40.1 Pictures, picture files and Image objects

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1245 | An Image object is a 3D-sized picture that shows in plan and 3D (not in CAD details); a Picture (Picture File Box) is a 2D object for plan, section, CAD detail and layout pages and never shows in cameras | Partial | `plan_core::images` (Image objects work); a Picture box does not exist: File > Import > Picture places an Image object (`tools/images.rs import_picture`); underlays serve tracing (L-43) |
| 1245 | Images that face the camera, and Billboard Images that do not | Works | tools/images.rs Create Image and Create Billboard Image, `plan-3d/src/images.rs` |
| 1245 | Place an Image from the Library in a plan view, camera view or overview | Partial | placing is plan-only (see the placing-in-3D row) |

### 40.2 Images

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1246 | Create Image or Billboard Image opens the Image Specification (file, 2D plan symbol, size and position), then a click places it; .bmp, .jpg, .png, .gif, .tif | Partial | NO SPEC (CAD-136): a file box first, then the click and the Image Specification (`import_picture`); PNG and JPEG only (GIF, TIFF, BMP are not read) |
| 1246 | A file outside the data folder is copied into Data/Images/-My Images | Differs | the object keeps the original path (`ImageSpec.path`) |
| 1246 | New > Image in the Library; Paste Image; Screen Capture to an Image | Partial | Create Image Library saves a picture to My Library; paste and capture missing |
| 1246 | Create Image Library: convert a whole folder of images into Image objects with the same folder structure | Partial | NO SPEC (CAD-137): `ImageMode::ImageLibrary` saves one picture; the folder conversion is missing |
| 1247 | Images draw on the Images layer, in front of most plan objects; they print, go to layout and export to DXF; the plan shows a 2D symbol (any CAD block) | Partial | NO SPEC (CAD-138): layer and layout yes; a chosen CAD-block symbol is missing |
| 1247 | In 3D, images show in every technique except Clay and Glass House; in Physically Based only when not set to face the camera | Partial | `plan-3d/src/images.rs`; per-technique exceptions unverified |
| 1247 | Plant images are in the Materials List and schedules by default; plain images need a custom component | Partial | plants: part 6 |
| 1248 | Edit handles: in plan rotate, move, resize (aspect kept); in 3D only a resize and a move handle | Partial | plan handles work (S rows); the 3D handles are missing (see the 3D handles row) |
| 1248 | Dimension lines can locate image centers in plan view, if the dimension default allows | Partial | DIM locate settings (verify image centres) |
| 1249 | Image Specification, Image panel: file (Select/Import or Browse/Edit Path), 2D Plan Symbol and Select CAD block | Partial | NO SPEC (CAD-138): tabs General, Image, Layer, Label (`dialogs/images.rs`); file and billboard only |
| 1250 | Size/Elevation: Height, Width, Retain Aspect Ratio, Reset Original Aspect Ratio, Elevation Reference with Top and Bottom heights | Partial | NO SPEC (CAD-138): width, height, keep aspect, elevation from floor, position, rotation; no Elevation Reference, no Reset Original Aspect Ratio |
| 1250 | Center Point X and Y; Block Line/Fill Style; Reverse Image; Image Always Faces Camera (off for billboards, needed for ray-traced views) | Partial | NO SPEC (CAD-138): position and "Flip" and the Billboard check box; Block Line/Fill Style is missing |
| 1250 | Copyright line and preview | Missing | NO SPEC (CAD-138): no copyright field |
| 1250 | Transparency panel: use the file's alpha, or a custom transparency color with Tolerance and an eyedropper over a magnified preview | Partial | "Make one colour transparent" with colour and tolerance; no magnifier eyedropper |
| 1251 | Layer panel with the layer and Drawing Group | Works | Layer tab (LAY-36) |
| 1252 | Fill Style panel for the 2D symbol | Missing | NO SPEC (CAD-138): no Fill Style tab |

### 40.3 Pictures: export, import, Picture Box

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1252 | Export Picture of the current view (BMP, JPG, PNG; Physically Based and Clay also as .hdr); Screen Capture; Export 360 Panorama; Print Image to PDF | Partial | File > Export > Picture (PNG, JPEG, BMP, TIFF) for the plan, an elevation, a camera drawing or the 3D view (L-49, DECISIONS 403); .hdr missing |
| 1253 | Export Picture dialog: Use Active Window Size, Pixels or Units, Width and Height, Resolution in pixels per inch or mm (metadata only), Retain Aspect Ratio, Transparent Background, Open in Default Image Viewer; settings persist; a version in Send to Layout | Partial | NO SPEC (L-76): pixel width or paper size at a DPI, background white or transparent, JPEG quality, format (`dialogs/export_picture.rs`); Use Active Window Size, Units, the default-viewer option and persistence are missing |
| 1254 | Save to Disk or Save to Project/Assets (project management) | Out-of-scope | Chief project management and cloud |
| 1254 | Import pictures with File > Import > Import Picture, by dragging a file from the OS, by Paste Special, or by Screen Capture; the Picture Box lands at the view center | Partial | NO SPEC (CAD-139): File > Import > Picture; drag-drop of a .zip only (`files.rs`); no picture drag-drop, no Paste Special |
| 1254 | A Picture Box references its file; edits outside show after reopening the plan | Partial | references by path |
| 1255 | Picture Box Specification, General: file (Select/Import or Browse/Edit Path), Save in Plan with PNG or JPEG and Quality %, Center X/Y/Angle, Size with Retain Aspect Ratio, Reset Original Aspect Ratio, Reset Cropping, Grayscale, Reflect, Brightness and Contrast 1-100, preview | Missing | NO SPEC (CAD-139): no Picture Box object; the nearest are the Image Specification and the Underlays window (name, opacity, rotation, lock) |
| 1256 | Picture Box Line Style (Show Outline), Fill Style (transparent areas), Label (file name) | Missing | same |
| 1263 | Picture, metafile and PDF boxes live on the "Picture/PDF Boxes" layer; pictures and PDF boxes in Drawing Group 38 (back), metafiles in 21 | Missing | NO SPEC (CAD-139): no such layer or group (drawing groups: LAY-36) |
| 1263 | Corner handles resize the contents at the same ratio; Extend handles crop or pad without scaling | Missing | same |
| 1263 | Pictures can be located by dimensions; use the whole-object move to keep the ratio | Partial | DIM locate rules |
| 1264 | Point to Point Resize edit tool: two points with a known distance, a typed distance, optional Retain Aspect Ratio | Works | `ImageMode::PointToPointResize` (tools/images.rs), `dialogs/underlay.rs` (L-43) |
| 1264 | Resize Factor in Transform/Replicate Object to scale a picture to the plan scale for tracing | Works | `editor/transform.rs` resize_factor |

### 40.4 Metafiles and PDF files

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1257 | Metafiles (.emf, .wmf): vector pictures exported from plan, CAD details, layout pages and Vector Views (marquee, Metafile Size, Use Line Weight) and imported as Metafile Boxes; not on macOS | Out-of-scope | Windows-only format (the manual says macOS cannot import or export metafiles) |
| 1258 | Metafile Box Specification: center and angle, size, aspect, cropping, Line Style, Fill Style, Label | Out-of-scope | same |
| 1259 | Export PDF from any view (File > Export > Export PDF; Print View or Print Image; "Chief Architect Save as PDF" printer) | Works | L-20 (Export Layout PDF, Construction Set PDF), File > Print > Print Image |
| 1259 | Import PDF (File > Import > Import PDF or drag): multi-page choice of current page, a range or all pages, one PDF Box per page; only 2D data | Partial | NO SPEC (L-77): File > Import > Underlay Picture (PNG, JPEG, PDF) for PDFs whose pages carry one embedded picture (L-46, `tools/underlay/pdf.rs`); no vector PDF rendering, no page range, one underlay per import |
| 1261 | PDF Box Specification: file with Browse or Edit Path, Page, Save in Plan, center and angle, size and aspect, Reset Cropping, Line Style (Show Outline), Fill Style, Label (file and page) | Missing | NO SPEC (L-77): the Underlays window has a PDF page box, name, opacity, rotation and lock |

### 40.5 Walkthrough videos

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1265 | A walkthrough is a series of pictures saved as .mp4, .wmv or .avi | Partial | Motion-JPEG AVI and a PNG sequence (C15-2, `plan-render/src/avi.rs`); no H.264/MP4 or WMV (dependency-free writer) |
| 1265 | Create Walkthrough Path draws the route and sets camera height, angle and speed before recording | Works | C-71, `tools/camera.rs` Walkthrough; ours is a polyline smoothed by Catmull-Rom, Chief's is a spline |
| 1265 | Create Orbital Walkthrough Path edit tool: a circle around a camera's or overview's focal point, radius equal to the line of sight, every frame facing the focal point | Missing | NO SPEC (C-176): no orbital path |
| 1265 | Create Stationary Walkthrough: a time-lapse sun study from one perspective view with the date and time overlaid | Missing | NO SPEC (C-176): no sun-study video |
| 1265 | Record Walkthrough frame by frame: Record, Pause Recording, Save Frame and Stop Recording while moving the camera; each redraw is a frame | Missing | NO SPEC (C-177): recording follows a path only |
| 1266 | Play Walkthrough opens a recorded file in the system player | Partial | 3D > Walkthroughs > Play Walkthrough plays the path in the panel (C-71), not a file in the OS player |
| 1266 | Walkthrough Preview side window | Partial | View > Walkthrough Preview runs the same playback (C-71); see the Preview rows |
| 1266 | Codec choice remembered across sessions; Windows Media Video 9 or H.264 by default | Differs | Motion-JPEG only, no codec picker |
| 1266 | Walkthrough Paths: Connect CAD Segments on, draw a spline, edit curvature, add Key Frames; convert a CAD polyline or spline into a path | Partial | C-71, Walkthrough Path from CAD Polyline; spline curvature editing is missing |
| 1267 | Record along a path with the edit button or the preview's Record; a Record Walkthrough Options dialog sets Quality, Resolution, rendering options, codec, name and location | Partial | Record Walkthrough dialog: picture size, fps, samples, format, folder (`RecordDialog`); no codec, no per-path quality block |
| 1267 | Paths show in plan only, on the "Walkthrough Paths" layer, in front, print and export; an automatic label (floor and number) and an optional custom label on "Polylines, Labels" | Partial | NO SPEC (C-178): paths draw in plan (C-71); the label rules are missing |
| 1268 | Reverse Direction edit button for a path | Missing | NO SPEC (C-178): CAD Reverse Direction exists (`tools/cad/edit.rs`); not offered for walkthrough paths |
| 1268 | Key Frames change camera direction, tilt, height, speed, sunlight and the floor; Add Key Frame edit tool with Sticky Mode; the frame takes values from its neighbours; Delete Key Frame (at least two stay) | Partial | NO SPEC (C-179): path nodes are key frames with height, look direction, tilt and hold (C-71, DECISIONS 165); time, speed after, sun and floor are missing |
| 1268 | A Key Frame symbol shows its number, points the Camera Angle and sits at its Time; key frame handles in two colors | Partial | nodes draw in plan; numbering and angle arrows not verified |
| 1268 | Paths follow stairs and ramps and continue on the next floor with a dashed extension | Missing | NO SPEC (C-180): paths are on one floor; node floor is not stored |
| 1269 | Walkthrough Path Specification dialog (also the Defaults dialog): General (Key Frames list with Floor, Time, Speed After, Camera Angle, Tilt, Height, Sunlight, Pause), Camera, Backdrop, Polyline, Selected Line/Arc, Line Style, Label | Partial | NO SPEC (C-181): the Camera dialog's Walkthrough section: speed, node table (x, y, height, look, tilt, hold), fps (`walkthrough_page`) |
| 1271 | Key Frame Symbol Size; Camera Angles absolute or relative to the path; Camera Heights absolute or relative to floor/terrain | Missing | NO SPEC (C-181): path heights are above the floor; angle is "along the path" or fixed |
| 1271 | Resolution list with Custom Width and Height; Duration; Frames per Second 1-100; Video Codec in the defaults | Partial | fps and picture size in the Record dialog; duration follows speed |
| 1272 | Camera and Backdrop panels of the path (technique, shadows, lighting, sunlight per key frame) | Partial | the path camera carries a technique and a backdrop (`CameraView`); no per-key-frame sun |
| 1272 | Polyline panel reports the path length; Selected Line/Arc panels; Line Style; Label | Partial | path length is reported (`walk_length`) |
| 1273 | Walkthrough Path Previews: Active Walkthrough list with Define; preview pane (drag to set angle and tilt); timeline slider with current frame, Start and End Frame handles, key frame diamonds and pause bars | Partial | NO SPEC (C-116): C-71: Play with a scrub bar and key-frame jumps; no Start/End range, no drag-to-aim |
| 1274 | Preview controls: Use Recording Quality or Use Standard Quality, Create Camera View, Create Sun Angle, previous/next key frame and frame, Play from Beginning or Current Frame, Record | Partial | NO SPEC (C-116): key-frame jumps and Play only |
| 1275 | Key Frames list in the preview, with Floor, Time, Speed After, angles, height, sunlight source (Sun From Key Frame, Interpolating), Adjust Sunlight, Pause | Missing | NO SPEC (C-179): see the Key Frame row |
| 1276 | Preview Options: Preview Pane, Key Frame Settings, Horizontal or Vertical Layout | Missing | NO SPEC (C-116): fixed panel |
| 1276 | Generate: path or Stationary, check General, Camera and Backdrop panels, Super Resolution, preview, then Record; Esc or Cancel in the Progress dialog stops | Partial | Record Walkthrough with a progress run that can be stopped (`stopping_early_leaves_a_playable_movie`) |
| 1277 | Walkthrough Options dialogs (Record Walkthrough Options for a path, Stationary Walkthrough Options) | Partial | Record Walkthrough dialog only |
| 1278 | Resolution: list, Width, Height, Retain Aspect Ratio | Works | Record dialog sizes (`RecordDialog`) |
| 1279 | Walkthrough Options: Compression 0-100, Duration Along Path or Duration, Frames Per Second 1-100, Video Codec | Partial | quality and fps; duration is derived; no codec |
| 1279 | Sun Angle Options: Time Lapse or Seasonal interpolation, Start and End Sun Angles with New, Edit, Delete, Elapsed Clock Time, Sun Time Overlay | Missing | NO SPEC (C-176): see Stationary Walkthrough |
| 1279 | Frames: All or a Custom range from the preview; totals reported | Partial | whole path only |
| 1279 | Save to Disk, Project or Assets, or the Chief Cloud (10 files) | Out-of-scope | project management and cloud (Disk is covered by the folder choice) |
| 1280 | Time Overlay Settings: Color, Print Size, Font, Include Date, Layout, Angle, Transparency, Margins | Missing | NO SPEC (C-176): see Stationary Walkthrough |
| 1281 | Frame by Frame Walkthrough steps: Frame Rate 1-100, Compression, Codec, Save to Disk or Cloud, Record, move the camera, Pause Recording, Save Frame, Stop; zoom and scrollbars do not make frames | Missing | see the Record row |

### 40.6 Screen captures

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1282 | Tools > Screen Capture: Screen Capture Setup (Save Capture As Picture, Backdrop, Material or Image; Hide Chief While Capturing; Capture, Done) and Capture Screen with a marquee; stored as a Picture in the view or as a library item with a time-stamped name | Missing | NO SPEC (APP-141): no screen capture tool |
| 1284 | Capture View to Clipboard: the whole current view window, including handles and selection feedback, to the clipboard; toolbar button and hotkey | Missing | NO SPEC (APP-142): no view capture |

## 41. Importing and Exporting (pp. 1285-1306)

### 41.1 3D Viewer, As-Built and Room Planner

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1285 | Export Chief Architect 3D Viewer File to the cloud (signed-in account; 99 models; lights exported; reference models excluded) | Out-of-scope | Chief cloud service; glTF export is the local equivalent (C-78, `plan-view3d/src/export.rs`) |
| 1286 | 3D Viewer export options: Create New or Replace Existing, name, description, Cameras button (Select Cameras to Export: include, initial view), Notes button (Select Notes to Export), Surface Count and Texture Size | Out-of-scope | same |
| 1288 | Import Chief As-Built File (iPhone/iPad LiDAR app, cloud): choose layers for walls, windows, doors and annotation; ceiling height, door and window sizes and casing carry over | Out-of-scope | Chief mobile app and cloud |
| 1289 | Import Room Planner File (.room from the RENDR mobile app) | Out-of-scope | vendor app format |

### 41.2 Importing 2D drawings

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1289 | File > Import > Import Drawing (DWG/DXF) into plan, section, CAD detail or layout; dragging a file onto the window starts it | Partial | NO SPEC (L-78): File > Import > Import Drawing (DXF) into the plan (L-43); DWG is not read; a drag-drop of a drawing file is not handled |
| 1290 | Import Drawing dialog: files list; Show Import Assistant; Show For Each File; Create CAD Blocks with Place In Current View, Auto Position Blocks, Add to Library | Missing | NO SPEC (L-78): the import window has units, scale, rotation, base and insertion point, layer prefix and map, Convert to walls (`dialogs/exchange.rs`); no block options |
| 1290 | Reads AutoCAD files up to version 2025; only Model Space (the first Paper Space page becomes a CAD block); no xrefs; Z mapped to zero; thickness ignored | Partial | ASCII DXF only (`plan_import::parse_dxf`); Paper Space and DWG are not read |
| 1291 | Entities imported: lines, circles, arcs, ellipses, splines (as polylines), polylines and lightweight polylines (bulges become arcs, widths ignored), points (only when a layer becomes Elevation Data), text and multi-line text (as rich text, first font wins, Arial fallback), multileaders, Unicode text, blocks and inserts, hatch (as solid polylines), 2D solids, 3D faces and polyface meshes, rotated/aligned/3-point angular dimensions, attributes, line styles by name, layers | Partial | NO SPEC (L-79): `plan-import/src/dxf.rs` reads LINE, LWPOLYLINE, POLYLINE, CIRCLE, ARC, TEXT, MTEXT and INSERT; ellipses, splines, hatch, solids, 3D faces, dimensions, attributes, points and multileaders are not read (a count of skipped entities is reported) |
| 1291 | Import or skip layers; three mappings: one Chief layer, same-name layers (created when absent), or Advanced Layer Mapping | Partial | a per-layer map (keep, skip, plan layer, new name) and a name prefix; same-name creation is the default (L-43) |
| 1292 | Import Drawing Assistant, Select File page: Browse, Polylines (join lines with shared end points), Boxes (closed rectangles), CAD blocks all or referenced only, Import Hatch entities, password for protected files | Missing | NO SPEC (L-80): no join-lines-to-polylines pass, no box detection |
| 1293 | Select Layers: visible layers checked, frozen unchecked, Select All and Clear All, Convert To Terrain Perimeter or Elevation Data (plan view only) | Partial | NO SPEC (L-80): layer list with keep/skip; Convert To terrain missing |
| 1294 | Layer Mapping page and Advanced Layer Mapping table with New markers and a Layer Display Options button | Partial | the layer map text in the import window |
| 1295 | Duplicate CAD Blocks page: auto-name (_Copy_1), replace, keep existing, or manage each block; Advanced page with Auto Name, Replace, Use Existing | Missing | NO SPEC (L-80): no block-name conflict handling |
| 1297 | Drawing Unit with a custom scale unit; dimensions as dimension lines or as CAD blocks; Move drawing to the origin | Partial | units and an extra scale; base-point choices (drawing origin or lower-left corner) and insertion point; dimension handling is missing |
| 1297 | After import the drawing is one selection with move and rotate handles; Fill Window finds it; large coordinates are moved with Transform/Replicate | Partial | imported objects are selected; the 6-digit warning is not given |
| 1298 | CAD to Walls turns imported lines into walls, doors, windows and railings | Works | `plan_import::cad_to_walls` (Convert to walls in the import window) |

### 41.3 Exporting 2D DXF/DWG files

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1298 | Export Current View (plan, CAD detail or orthographic 3D view; a perspective view is not to scale) and Export All Floors (one file, layer names with a floor suffix such as "Electrical-2"); custom line styles become solid | Partial | NO SPEC (L-81): File > Export > DXF (the active floor, all floors or a pick, "Floor name on each layer", 2D or 3D) and Elevation DXF (L-44, L-45); detail and 3D vector views export only through the camera drawing |
| 1299 | Export Drawing dialog: AutoCAD version, Layer Set with Define, Split Wall Assemblies Into Layers, Export Only Displayed Layers or all used and named layers, Scaling Unit, Create Associative Dimensions, Export Pattern Lines, Export Filled Areas as 2D solids, Export AutoCAD Index Colors; .dwg, .dxf or binary .dxf | Partial | NO SPEC (L-81): units, layer naming (Chief or AIA with a map), line weights, text as text or lines, floors, 2D or 3D (`dialogs/dxf_options.rs`); R12 ASCII DXF only: no version, DWG, binary DXF, wall-layer split, associative dimensions, pattern lines, filled areas or index colors |
| 1300 | Supported export entities: line, arc, circle, multi-line and Unicode text, polylines with bulges, block inserts | Partial | plan-core `export/dxf.rs` writes lines, arcs, circles, polylines and text (no block inserts) |
| 1301 | Dimensions export as aligned, rotated or 3-point angular dimension entities with an associated block; added text becomes text and lines | Differs | DIM-70: dimensions export as lines and text |
| 1301 | Layer mapping to AutoCAD: name, nearest color, display, size to line weight, style to line type, lock | Partial | names, colors and weights (`line_weights`); line types and lock are not written |

### 41.4 360 panorama, thermal envelope and REScheck

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1302 | Export 360 Panorama: Height and Width in pixels, Limit Dimensions to Powers of Two, 2:1 ratio, Save to Disk or Project/Assets, Save as Backdrop (an .hdr for Physically Based or Clay, else a .jpg), Save to Chief Cloud | Partial | NO SPEC (L-82): C-77, C15-1: widths 1024-8192 at 2:1, samples, PNG plus an HTML viewer; Save as Backdrop is missing; the cloud is out of scope |
| 1303 | Replace Existing 360 Panorama / Walkthrough dialogs (cloud) | Out-of-scope | cloud service |
| 1304 | Export Thermal Envelope Data: a .csv by floor level with direction and area of each envelope component (floor platforms, ceiling platforms, walls, doors, windows) | Missing | NO SPEC (L-83): the data exists (door and window U-factor and SHGC, DW-115; Conditioned room radios, R-43) but nothing exports it |
| 1304 | Export to REScheck (.rxl): dialog with Group Similar Walls and Group Similar Doors/Windows; project data (front faces, conditioned floor area, owner/agent from client info, designer info, New Construction, 1-and-2 Family Detached); location and permit not exported | Missing | NO SPEC (L-83): same |
| 1305 | Envelope data exported: floors (assembly, area, cavity and continuous R-values), slabs on grade (perimeter and R-value), ceilings, walls (assembly, orientation, area, R-values, grouped by orientation), doors and windows (assembly, orientation, area, U-factor, SHGC); skylights not exported; wall labels carry over | Missing | NO SPEC (L-83): same |
| 1305 | Rooms of an interior type are conditioned by default (Open Below too); a room can be forced in or out in the Room Specification | Partial | R-43 (Conditioned/Unconditioned radios exist in the room dialog; the area total is not exported) |
| 1306 | Orientation: walls, doors and windows within 45 degrees of north, south, east or west of the North Pointer (or screen up); a curved wall uses its chord | Missing | NO SPEC (L-83): same row; the North Pointer is not read by anything (see North Pointer) |

## Dialog panels

For every specification and defaults dialog in pages 1099-1306: Chief's panel list, what Plan Studio has today, the panels missing, and the fields missing per panel. A new parity id (in parentheses) names the gap; "C-n" ids are existing rows.

| Dialog | Pages | Chief panels | Plan Studio today | Missing panels | Missing fields per panel |
|---|---|---|---|---|---|
| Material Defaults | 1101-1102 | one list of material categories, a Select Material button and two preview boxes | Default Settings > Materials: class and part rows with a picker (`tools/materials/defaults.rs`, six classes) | list-style category table, preview boxes | categories: Room Moldings, Cabinet Door/Drawer, Countertop, Hardware and the rest of Chief's list; multi-select (C-86) |
| Select Material / Select Library Object | 1106-1108 | Library Materials, Plan Materials, Material Defaults (the last only for a library symbol's component) | per-component drop-down lists in each object dialog; the Library dock lists materials (C-61e) | the whole modal dialog and all three panels | Use Default Material, preview shapes (Cube, Sphere, Teapot, Plane), backdrop, Add New buttons, Settings menu, Tile Mode, Search Subfolders (C-87) |
| Plan Materials | 1114-1116 | one dialog: Search, list with In Use column, Edit, New, Copy, Purge, Delete, Merge, Add to Library, Replace, preview | the Materials window lists library and plan materials without usage counts (`dialogs/materials.rs`) | the dialog | In Use, Purge, Delete, Merge, Replace, Add to Library, New, Copy, preview (C-91) |
| Define Material (Material Specification) | 1117-1128 | Pattern, Texture, Properties, Materials List, plus a preview pane | the same four tab names (`tools/materials/spec.rs`) | none by name; the preview pane is missing (C-92) | Pattern: Line Color, Line Weight, Shading Contrast, Pattern Type with Library/custom, Width/Height/X-Y scale, Row Offset, H/V Offset, Global Symbol Mapping, Copyright, Add Pattern to Library, Keep in Sync (C-97). Texture: Stretch to Fit, Retain/Reset Aspect Ratio, Set Material Color Using Texture, TIFF/BMP/GIF/zip sources, per-map Remove and Invert (C-81, C-83). Properties: classes Matte, Polished, Predefined Metal, Shiny Metal, Translucent, Water; Diffuse, Reflection, Thin, Metal type, index of refraction, class-mix maps, Clear Coat, Brushed (C-100). Materials List: Structure Type, Calculation Method, Width/Height/Depth, Update from Pattern (C-85) |
| Pattern from Texture | 1129-1130 | Source, Simple/Advanced thresholds, Filter Size, Original/Output Image | none | the dialog | all fields (C-102) |
| Material Builder | 1131-1139 | Select Builder (Masonry and Stone, Tile, Wood, .sbsar), Builder Inputs, Material Outputs, Add to Library, preview | 3D > Material Builder opens the Material Specification dialog; procedural kinds exist as library textures | the builders | the three builders' inputs, margins/grout/stone/tile/wood groups, weathering, Material Scale, Reset to Defaults (C-103) |
| Create Copy of Material | 1109 | Create a Copy (name) or Edit the Source Material | none (OK saves a copy under the same name, C-58) | the prompt | both options and the scoping modes (C-89) |
| 3D Cladding Specification | 1143-1145 | General (Profiles table, Selected Profile Options, 3D Cladding Options, Generate Random 3D Moldings), Materials | none | the whole dialog | everything (C-107); Generate Random 3D Moldings (C-106) |
| 3D View Defaults | 1149-1150 | General Options (six), Surface Edge Lines (two) | window with eye height, angle of view, technique, callout shape/size/name (`defaults_window`) | none by name | all eight Chief options (C-110) |
| Camera Defaults (eleven dialogs) | 1147-1149 | same panels as the camera and section specifications; Alternate technique | Default Settings > Camera Tools: nine pages, a few fields (`default_pages/camera.rs`); In progress (Round 15, defaults_pages) | Perspective Framing/Orthographic Overviews pages; Chief's panels | Alternate technique, Reflections, every specification field (C-108) |
| Camera Specification | 1186-1194 | Camera, Positioning, Below Grade, Selected Defaults, Plan Display, Backdrop, Layer, Label | Camera, Backdrop, Rendering, Label (`dialogs/camera.rs`) | Below Grade, Selected Defaults, Plan Display (as a tab), Layer; Positioning is inside Camera | Camera: Show Color, Show Watermark, Define, Reflections, Animate Water, Light Bloom, Ambient Occlusion amount, Sharpening, Super Resolution, Depth of Field, Use Sunlight, Maximum Lights, Poché, Clip Surfaces Within, Show Lower Floors, Hide Camera-Facing Exterior Walls, Extend Terrain to Horizon; Navigation: Incremental Move/Rotate; Backdrop: Generated Sky, Spherical settings; Plan Display: all floors, symbol size, focal point, FOV indicators; Label: position and orientation |
| Cross Section/Elevation Specification | 1194-1200 | Camera, Positioning, Below Grade, Selected Defaults, Plan Display, Backdrop, Layer, Arrow, Label | the same four tabs as the camera dialog with section fields | Below Grade, Selected Defaults, Layer, Arrow | Scene Clipping: Poché, Framing Back Clip, Clip Sides and Width, Clip Elevation, Clip to Room with its two options; Plan Display: placement (Center, Left, Right, Both, Custom), line style and weight, Callout Size, arrow |
| Cross Section Slider | 1176-1177 | several cutting planes with check boxes, sliders and typed positions; saved with the camera | a slider in the Vector View, one plane (C-23) | multi-plane dialog | planes, typed position, save with the camera (C-141) |
| Depth Cue | 1175-1176 | Use Depth Cue, Keep Start/End in Sync, Start, End, Fog Opacity, Fog Color | none | the dialog | all (C-140) |
| Rendering Technique Options / Defaults | 1228-1240 | one panel per technique (Standard, Vector View, Physically Based, Clay, Glass House, Technical Illustration, Watercolor, Hand Drawn Lines, Duotone) | none; the Ray Trace window has size, samples, bounces, exposure, tone curve | all nine panels and the Defaults dialog | every field listed in the Rendering Techniques rows (C-113) |
| Adjust Lights | 1211-1214 | Light Settings, Light Source Properties table, Adjust buttons | Adjust Lights dialog with sets, per-light on/color/shadows/power (`AdjustLightsDialog`) | the fixture tree, the column set | Use Area Lights, Count, Room, Floor, Type, In Use, Show Position, Brightness, Modify All Light Sets, Copy/Rename Set, Reset Brightness, Update Lighting Automatically (C-169) |
| Adjust Area Light Material | 1214-1216 | Options (copy, edit source, select other), Adjust Area Light | none | the dialog | all (C-171) |
| Light Specification | 1216-1217 | Location, Light Data, Layer, Label | none (edit in Adjust Lights) | all four | Light Display floors, Absolute Elevation, Display Size (C-172) |
| Sun Angle Specification | 1220-1222 | Earth Data, Lighting Data, Line Style, Fill Style, Arrow | the Sun Angle window: month/day, time, latitude, or azimuth/altitude | all five panels | date calendar, daylight saving, time zone, longitude, symbol length, Show Date, Make/Delete Shadow, Casts Shadows, Intensity, Color (C-161) |
| Adjust Sunlight | 1222-1224 | Use Generic Sun, Use Sun Angle, key-frame Interpolate Sun, Reset to Defaults | 3D > Lighting dialog | Use Sun Angle list | Intensity, Color, Sun Follows Camera, Tilt, Direction (C-111) |
| Backdrop Specification | 1242-1244 | Preview, Backdrop (name, Location), Spherical Backdrop Options, Copyright | none (names read from Chief's folder) | the dialog | all (C-175) |
| Image Specification | 1248-1252 | Image, Transparency, Layer, Fill Style | General, Image, Layer, Label | Fill Style | 2D plan symbol, Elevation Reference, Reset Aspect, Block Line/Fill Style, Reverse, Copyright (CAD-138) |
| Export Picture | 1252-1254 | Image Size, Image Properties, Options, Save to | one window (size by width or paper/DPI, format, JPEG quality, background) | none | Use Active Window Size, Units, Resolution metadata, Open in Default Image Viewer, WebP/HDR (L-76) |
| Picture Box Specification | 1255-1257 | General, Line Style, Fill Style, Label | none | all | file, Save in Plan, cropping, Grayscale, Reflect, Brightness, Contrast (CAD-139) |
| PDF Box Specification / PDF import | 1260-1262 | General, Line Style, Fill Style, Label; import page choice | Underlays window | all panels | page range, Save in Plan, cropping (L-77) |
| Metafile Box Specification | 1258-1259 | General, Line Style, Fill Style, Label | none (Windows-only) | out of scope | - |
| Walkthrough Path Specification / Defaults | 1269-1272 | General (Key Frames), Camera, Backdrop, Polyline, Selected Line/Arc, Line Style, Label | the Camera dialog's walkthrough section | Backdrop, Polyline, Selected Line/Arc, Line Style as walkthrough panels | Time, Speed After, Sunlight, Pause per key frame; Absolute/Relative angle and height; Resolution presets; Duration; Key Frame Symbol Size (C-181, C-179) |
| Walkthrough Preview (side window) | 1273-1276 | Active Walkthrough, preview pane, timeline, controls, Key Frames, Options | View > Walkthrough Preview plays the path with scrub and key-frame jumps | pane drag, Start/End frames, quality switch, Create Camera View/Sun Angle, key frame editor, layout options | all listed (C-116) |
| Walkthrough Options / Time Overlay | 1277-1281 | Resolution, Walkthrough Options, Sun Angle Options, Frames, Save; Time Overlay Settings | Record Walkthrough dialog (size, fps, samples, format, folder) | Sun Angle Options, Frames range, codec, Time Overlay Settings | Time Lapse/Seasonal, Start/End Sun Angles, Sun Time Overlay, all Time Overlay fields (C-176) |
| Screen Capture Setup | 1283 | Save Capture As (Picture, Backdrop, Material, Image), Hide Chief While Capturing, Capture, Done | none | the dialog | all (APP-141) |
| Import Drawing / Import Drawing Assistant | 1290-1297 | Import Drawing dialog; Assistant pages Select File, Select Layers, Layer Mapping, Advanced Layer Mapping, Duplicate CAD Blocks, Advanced Duplicate, Drawing Unit, Import Complete | one import window: units, scale, rotation, base point, insertion point, layer prefix and map, Convert to walls | Duplicate CAD Blocks pages, Select File options | join lines to polylines and boxes, CAD block options, Convert To terrain, dimensions as CAD blocks, password (L-80, L-78) |
| Export Drawing | 1299-1300 | AutoCAD File Format Options, Layer Options, Other Options | DXF options window: units, layer naming, line weights, text mode, floors, 2D/3D | AutoCAD version | version, Layer Set, Split Wall Assemblies, displayed-only vs used layers, associative dimensions, pattern lines, filled areas, index colors, DWG/binary DXF (L-81) |
| Export 360 Panorama | 1302-1303 | size, Limit to Powers of Two, Save to Disk/Project, Save as Backdrop, Cloud | Export 360 Panorama window (width, samples, file) | Save as Backdrop | HDR output, backdrop save (L-82) |
| Export to REScheck | 1304-1306 | Group Similar Walls, Group Similar Doors/Windows, Export | none | the dialog | all (L-83) |
| Virtual Reality | 1182-1184 | status, options, room view, overview | none | out of scope | - |
| 3D Viewer export | 1285-1287 | create or replace, name, description, Cameras, Notes | none | out of scope (cloud) | - |

## Gaps to build

Ranked by how much a residential designer producing construction documents (custom homes, remodels, layout sheets) depends on the feature. Size: S (days), M (about a week), L (more than a week). "In progress" means a Round 15 brief covers part of it.

| Rank | Gap | Why it matters | Size | Parity file / ids |
|---|---|---|---|---|
| 1 | Draw text, CAD lines and dimensions on a cross section or elevation view and save them with the view | Section and elevation sheets are annotated by hand; today the only route is CAD Detail from View, which stops updating | L | 3d-views-cameras.md C-129 |
| 2 | Scene clipping for sections: Clip Sides, Clip Elevation, Clip Lines, stepped cutting planes, Clip to Room options, Framing Back Clip, Poché switch, Set as Default | Wall sections, kitchen and bath elevations and framing sections need these to show one wall or one bay | M | 3d-views-cameras.md C-136, C-137, C-138, C-157, C-152, C-133 |
| 3 | DWG import and export, AutoCAD versions, binary DXF, Import Drawing Assistant (joined lines, hatch, ellipses, splines, solids, dimensions, duplicate blocks, terrain conversion) | Surveyors and engineers send DWG; consultants want DWG back; today only an ASCII DXF subset moves | L | documentation-layout.md L-78, L-80, L-79, L-81 |
| 4 | Plan Materials dialog, Select Material dialog and plan-specific material definitions that travel with the plan | Material selections are client decisions; a plan opened elsewhere loses custom materials, and there is no list of what the plan uses | L | 3d-views-cameras.md C-91, C-87, C-93 |
| 5 | Camera and section specification panels: Plan Display (placement, line style and weight, arrow, symbol options), Layer and Drawing Group, Selected Defaults, Below Grade | Section marks and callouts on layout sheets are controlled here; Below Grade drives the grade-line look of sections | M | 3d-views-cameras.md C-158, C-119, C-156, C-139, C-153 |
| 6 | Thermal Envelope Data (CSV) and REScheck (.rxl) export | Energy-code submittals for permits; the U-factor, SHGC and conditioned-room data already exist in the model | M | documentation-layout.md L-83 |
| 7 | Material definition depth: Pattern tab line color/weight/shading contrast, Materials List structure type and calculation method, missing classes (Matte, Polished, Predefined Metal, Shiny Metal, Translucent, Water), Stretch to Fit | The pattern lines set how siding and masonry read in elevations; structure type and calculation method drive quantity take-offs | M | 3d-views-cameras.md C-97, C-85, C-100, C-81 |
| 8 | Rendering Technique Options and Defaults (Vector View shadow intensity and shading contrast, Technical Illustration, Watercolor, Clay, Duotone, Glass House, Hand Drawn Lines on Top) | The same dialog tunes presentation and sketch-style elevations; today each technique is a fixed look | L | 3d-views-cameras.md C-113, C-163 |
| 9 | Hide Camera-Facing Exterior Walls | Shows a room layout from an exterior view for client reviews of remodels | S | 3d-views-cameras.md C-151 |
| 10 | Picture Box and PDF Box objects (import with page range, cropping, Save in Plan, Show Outline) and vector PDF underlays | Client sketches, site photos, scanned plats and consultant PDFs go on the plan and on layout pages | L | dimensions-text-cad.md CAD-139; documentation-layout.md L-77 |
| 11 | Orthographic Full/Floor/Framing Overviews and Isometric Overviews | Axonometric views for details and framing explanations | M | existing C-15 |
| 12 | Sun Angle objects with date, time, place and plan shadow polylines, North Pointer driving the sun, Sunlight Defaults, Move Sun/Moon, Toggle Sunlight | Shadow studies on site plans and sun studies for clients | M | 3d-views-cameras.md C-161, C-173, C-111, C-166, C-165 |
| 13 | Annotations in camera views (Text, Leader Line, Note, dimensions on a drawing surface) | Client presentation views with callouts | L | 3d-views-cameras.md C-130, C-142 |
| 14 | Walkthrough completeness: Key Frame fields (time, speed, sun, pause, floor), paths across stairs, orbital and stationary (sun study) walkthroughs, frame-by-frame recording, Preview side window features | Client fly-throughs and sun-study videos | M | 3d-views-cameras.md C-179, C-176, C-177, C-116, C-180 |
| 15 | Camera specification rendering options: Reflections, Animate Water, Light Bloom, Ambient Occlusion amount, Super Resolution, Depth of Field, Maximum Lights, Use Sunlight, Show Color, Watermark, Clip Surfaces Within | Quality control per presentation camera | M | 3d-views-cameras.md C-109, C-149, C-150 |
| 16 | Incremental Move Distance and Rotate Angle per camera; Mouse-Dolly, Mouse-Tilt, Focus on Object; View Direction Top/Bottom/Restore; Auto Elevation tools one side at a time | Everyday camera handling; the fixed 24 in / 15 degree steps (DECISIONS 41) are too coarse for rooms | S | 3d-views-cameras.md C-121, C-125, C-126, C-114 |
| 17 | Cross Section Lines and Point Markers; Depth Cue for sections and elevations | Dimensions to cut walls in sections stay attached; depth cue gives elevations a layered look | M | 3d-views-cameras.md C-131, C-140 |
| 18 | Spot lights, Light Specification, brightness per light set, Adjust Lights columns, Create/Edit Light Set from Selection, area lights from Emissive materials | Interior lighting studies | M | 3d-views-cameras.md C-164, C-172, C-168, C-169 |
| 19 | 3D Cladding (siding and roofing with real depth) and Adjust 3D Cladding | Realistic exteriors; siding start point and course alignment | L | 3d-views-cameras.md C-104 (C-73) |
| 20 | Material Builder with Masonry and Stone, Tile and Wood builders; Pattern from Texture; Convert Textures to Materials; Create Plan Materials Library | Custom finishes; low need for permit documents | L | 3d-views-cameras.md C-103, C-102, C-96 |
| 21 | Screen Capture tools and Capture View to Clipboard | Quick pictures for emails and client messages | S | preferences-hotkeys-toolbars.md APP-141, APP-142 |
| 22 | Backdrop import, folder import, HDR backdrops, Generated Sky, spherical panoramic backdrops, Rotate Spherical Backdrop | Client renderings with site photos behind the model | M | 3d-views-cameras.md C-175, C-154, C-155 |
| 23 | Export Picture options (active window size, units, open in viewer, remembered settings), WebP and HDR, Save 360 Panorama as a backdrop | Small output niceties | S | documentation-layout.md L-76, L-82 |
| 24 | 3D View Defaults options (camera bumps off walls, auto turn, display of active cameras and openings, surface edge lines) and Camera defaults dialogs with Chief's fields | Defaults for new views; In progress (Round 15, defaults_pages) for the Camera Tools pages | S | 3d-views-cameras.md C-110, C-108 |

## Where this audit disagrees with DECISIONS.md or with earlier audits

- **DECISIONS 41** (3D menu camera steps are fixed at 24 inches, 15 and 5 degrees, "Chief's step sizes come from Preferences > Behaviors"): the manual (pp. 1160, 1163, 1190) keeps the Incremental Move Distance and Incremental Rotate Angle in each camera's Camera Specification dialog (Navigation group), and they drive pan, dolly, orbit, tilt and the keyboard steps.
- **DECISIONS 127** (material classes General, Plastic, Metal, Glass, Mirror, Emissive, Transparent with typical values): the manual (pp. 1123-1126) lists ten classes: General, Matte, Mirror, Plastic, Polished, Predefined Metal, Shiny Metal, Translucent, Transparent and Water. Glass and Emissive are not classes (glass is a Transparent material; emissive is a setting of General, Translucent and Transparent).
- **DECISIONS 131 and C-61f** (Materials List by Surface counts both faces of every painted wall): the manual (p. 1105) says wall materials applied with the Material Painter are cosmetic and are not calculated in the Materials List, apart from foundation wall footings.
- **DECISIONS 166 and parity C-14** (an overview keeps its eye and target and has no symbol in the plan): the manual (pp. 1153, 1157) gives every overview a camera symbol in plan view that can be selected, edited and copied.
- **DECISIONS 130** (bump maps are not read yet; texture angle other than 90 degree multiples may seam): consistent with the manual, which lists Bump, Normal and Ambient Occlusion maps on the Texture panel (p. 1122); this is a gap, not a conflict.
- **DECISIONS 126** (Material Painter Room/Floor/Plan paint an extent narrowed by a Scope drop-down): the manual (pp. 1104-1105) defines the five modes as replacing instances of one material across the object, room, floor or plan; the walls and railings rule differs a little (Room mode takes walls and solid railings that share the original material). Verify in Chief.
- **DECISIONS 165** (walkthrough segments run at the walking speed and nodes hold for seconds): the manual (pp. 1268-1271) gives each Key Frame a Time, a Speed After and an optional Pause, and Camera Angle can be absolute or relative to the path.
- **Parity C-5** (eye height 66 in, field of view 60 degrees from memory): the manual (p. 1152) says cameras start at 60 in (1500 mm) and a 55 degree field of view; `plan-core` camera.rs keeps 66 in and 60 degrees.
- **Parity C-62** (note: "Shadows are on by default (Chief's default is off)"): the manual (p. 1203) says shadows are enabled in Full Camera views by default.
- **Parity C-14** (overview cameras are generated, not placed objects): see DECISIONS 166 above.
- **docs/chief-feature-coverage.md 3D menu table** is stale in places: it lists "Orthographic Full Overview + 4 elevations", but the 3D menu offers four elevations and a Plan Overhead (no Orthographic Full, Floor or Framing Overview); its rows for Light Sets, 360 panorama, Walkthrough video and Export Picture were Missing or Partial and are Works or Partial now (C15-1, C15-2, C15-3, DECISIONS 403).
- **Default Settings > Camera Tools pages** (`default_pages/camera.rs`) list Doll House and Glass House pages and a "Sketch" technique; the manual's eleven defaults dialogs (pp. 1147-1149) do not include them and use the specification dialogs' panels.

## New parity rows appended by this audit

| Id | Feature | Status | Parity file |
|---|---|---|---|
| C-80 | Keep Pattern/Texture in Sync and Global Symbol Mapping | Missing | 3d-views-cameras.md |
| C-81 | Stretch to Fit textures and Retain Aspect Ratio on the Texture tab | Partial | 3d-views-cameras.md |
| C-82 | Material maps beyond the PBR set (anisotropy, rotation, emissive, translucent, transmission, transparent) | Partial | 3d-views-cameras.md |
| C-83 | Per-map Invert and Remove on the Texture and Properties tabs | Missing | 3d-views-cameras.md |
| C-84 | Emissive materials as area lights in ray-traced views | Partial | 3d-views-cameras.md |
| C-85 | Material structure type and Materials List calculation method (Area, Count, Linear, Volume, None) | Missing | 3d-views-cameras.md |
| C-86 | Material Defaults categories and multi-select (Room Moldings, Cabinet Door/Drawer, Countertop, Hardware) | Partial | 3d-views-cameras.md |
| C-87 | Select Material dialog (Library Materials, Plan Materials and Material Defaults panels) | Partial | 3d-views-cameras.md |
| C-88 | Copy Selected Material button in the painter | Missing | 3d-views-cameras.md |
| C-89 | Scoping modes and the Create Copy of Material prompt for material editing | Partial | 3d-views-cameras.md |
| C-90 | Missing-texture warning on the Materials panel | Missing | 3d-views-cameras.md |
| C-91 | Plan Materials dialog (In Use, Purge, Merge, Replace, Add to Library) | Missing | 3d-views-cameras.md |
| C-92 | Material preview pane (shapes, techniques, backdrops, orbit) | Missing | 3d-views-cameras.md |
| C-93 | Plan-specific material definitions that travel with the plan | Differs-by-design | 3d-views-cameras.md |
| C-94 | Interactive Material Editor axis handles (scale, rotation, offset) | Missing | 3d-views-cameras.md |
| C-95 | Create materials from a pasted image or a screen capture (Stretch to Fit) | Missing | 3d-views-cameras.md |
| C-96 | Convert Textures to Materials and Create Plan Materials Library | Missing | 3d-views-cameras.md |
| C-97 | Pattern tab: line color, line weight, shading contrast, offsets and library patterns | Partial | 3d-views-cameras.md |
| C-98 | Texture tab sources: TIFF, BMP, GIF and zip textures | Partial | 3d-views-cameras.md |
| C-99 | Set Material Color Using Texture | Partial | 3d-views-cameras.md |
| C-100 | Material classes Matte, Polished, Predefined Metal, Shiny Metal, Translucent, Water and their settings | Partial | 3d-views-cameras.md |
| C-101 | Clear Coat and Brushed settings (ray-traced) | Missing | 3d-views-cameras.md |
| C-102 | Pattern from Texture dialog | Missing | 3d-views-cameras.md |
| C-103 | Material Builder dialog with parametric builders (Masonry and Stone, Tile, Wood) | Missing | 3d-views-cameras.md |
| C-104 | 3D Cladding definition: profiles on structural layers (create, edit, library) | Missing | 3d-views-cameras.md |
| C-105 | Adjust 3D Cladding, Interactive 3D Cladding Editor and scoping modes | Missing | 3d-views-cameras.md |
| C-106 | Random 3D molding variations for cladding | Missing | 3d-views-cameras.md |
| C-107 | 3D Cladding Specification dialog (General and Materials panels) | Missing | 3d-views-cameras.md |
| C-108 | Camera defaults dialogs: the same panels as the specification dialogs | Missing | 3d-views-cameras.md |
| C-109 | Reflections, Animate Water and Light Bloom camera options | Missing | 3d-views-cameras.md |
| C-110 | 3D View Defaults options (camera bumps, auto turn, display of active cameras and openings, surface edge lines) | Missing | 3d-views-cameras.md |
| C-111 | Sunlight Defaults and Adjust Sunlight dialog (Generic Sun, Sun Follows Camera, key frames) | Partial | 3d-views-cameras.md |
| C-112 | A layer set per 3D view | Missing | 3d-views-cameras.md |
| C-113 | Rendering Technique Options and Defaults dialogs (one panel per technique) | Partial | 3d-views-cameras.md |
| C-114 | Auto Elevation tools one side at a time (Front, Back, Left, Right) and repeat numbering | Partial | 3d-views-cameras.md |
| C-115 | Alternate rendering technique for cameras (right-drag creation) | Missing | 3d-views-cameras.md |
| C-116 | Walkthrough Preview side window (frame range, quality, Create Camera View, Create Sun Angle) | Partial | 3d-views-cameras.md |
| C-117 | Selecting and editing model objects inside an elevation or section view | Missing | 3d-views-cameras.md |
| C-118 | Technique letter in the camera symbol | Partial | 3d-views-cameras.md |
| C-119 | Plan Display panel of the camera specification (all floors, symbol size, focal point, field of view indicators) | Partial | 3d-views-cameras.md |
| C-120 | Move a camera to another floor | Missing | 3d-views-cameras.md |
| C-121 | Incremental Move Distance and Incremental Rotate Angle per camera | Missing | 3d-views-cameras.md |
| C-122 | Mouse-Orbit throw and auto-spin | Missing | 3d-views-cameras.md |
| C-123 | Wheel zoom speed rules in 3D (distance-based, Shift faster, Ctrl slower) | Partial | 3d-views-cameras.md |
| C-124 | Clip Surfaces Within distance | Missing | 3d-views-cameras.md |
| C-125 | Mouse-Dolly, Mouse-Tilt, 3D Center on Point and 3D Focus on Object camera tools | Missing | 3d-views-cameras.md |
| C-126 | View Direction: Top, Bottom and Restore Original View | Partial | 3d-views-cameras.md |
| C-127 | Placing and drawing objects in 3D views | Missing | 3d-views-cameras.md |
| C-128 | Edit handles and temporary dimensions on the handle surface in 3D views | Partial | 3d-views-cameras.md |
| C-129 | Draw text, CAD and dimensions on a section or elevation view (saved with the view) | Missing | 3d-views-cameras.md |
| C-130 | Text, Leader Line and Note annotations in camera views | Missing | 3d-views-cameras.md |
| C-131 | Cross Section Lines layer and Point Markers | Missing | 3d-views-cameras.md |
| C-132 | Project Browser camera menu: Open View, Find in Plan, Edit View | Partial | 3d-views-cameras.md |
| C-133 | Set as Default for camera views | Partial | 3d-views-cameras.md |
| C-134 | Context menu of the 3D view | Missing | 3d-views-cameras.md |
| C-135 | Reset Saved Camera (All and Position) and the save-on-close prompt | Missing | 3d-views-cameras.md |
| C-136 | Clip Sides, Clip Elevation and Clip Lines | Partial | 3d-views-cameras.md |
| C-137 | Stepped cutting planes (Add Break on a section line) | Missing | 3d-views-cameras.md |
| C-138 | Clip to Room options for elevations | Partial | 3d-views-cameras.md |
| C-139 | Selected Defaults panel of camera specifications | Missing | 3d-views-cameras.md |
| C-140 | Depth Cue for cross section and elevation views | Missing | 3d-views-cameras.md |
| C-141 | Cross Section Slider: several planes in camera views, saved with the camera | Partial | 3d-views-cameras.md |
| C-142 | Draw on Bounding Box and Draw on Surface for dimensions and text in 3D views | Missing | 3d-views-cameras.md |
| C-143 | Object labels in camera views | Missing | 3d-views-cameras.md |
| C-144 | Move and Rotate (Local and Global) edit tools for 3D annotations | Missing | 3d-views-cameras.md |
| C-145 | Picture export: WebP and HDR for ray-traced views | Partial | 3d-views-cameras.md |
| C-146 | Show Color and Show Watermark camera options | Partial | 3d-views-cameras.md |
| C-147 | Ambient Occlusion amount per camera | Partial | 3d-views-cameras.md |
| C-148 | Super Resolution and Sharpening (Upscaling) camera options | Missing | 3d-views-cameras.md |
| C-149 | Depth of Field in the camera specification | Partial | 3d-views-cameras.md |
| C-150 | Use Sunlight and Maximum Lights in the camera Lighting group | Partial | 3d-views-cameras.md |
| C-151 | Hide Camera-Facing Exterior Walls | Partial | 3d-views-cameras.md |
| C-152 | Poché switch on cross section cameras | Missing | 3d-views-cameras.md |
| C-153 | Below Grade panel (line overrides under the terrain or a height) | Missing | 3d-views-cameras.md |
| C-154 | Generated Sky backdrop (sun, moon, stars) | Missing | 3d-views-cameras.md |
| C-155 | Spherical panoramic backdrops and Rotate Spherical Backdrop | Missing | 3d-views-cameras.md |
| C-156 | Layer and Drawing Group panel for cameras and sections | Missing | 3d-views-cameras.md |
| C-157 | Framing Back Clip for section views | Partial | 3d-views-cameras.md |
| C-158 | Callout placement, section line style and weight (Plan Display of sections) | Partial | 3d-views-cameras.md |
| C-159 | Denoise View toggle and DLSS-style real-time denoise | Missing | 3d-views-cameras.md |
| C-160 | Tone mapping operator and exposure (Hable, ACES) | Partial | 3d-views-cameras.md |
| C-161 | Sun Angle objects (date, time, place, shadows) | Missing | 3d-views-cameras.md |
| C-162 | Opaque Window/Door Glass option | Missing | 3d-views-cameras.md |
| C-163 | Hand Drawn Lines on Top overlay | Missing | 3d-views-cameras.md |
| C-164 | Spot lights and light position indicators | Partial | 3d-views-cameras.md |
| C-165 | Toggle Sunlight and overcast lighting (Use Only Backdrop for Lighting) | Partial | 3d-views-cameras.md |
| C-166 | Move Sun and Move Moon tools with the direction indicator | Missing | 3d-views-cameras.md |
| C-167 | Interior, daytime and nighttime ambient light levels | Partial | 3d-views-cameras.md |
| C-168 | Light brightness per light set (Adjust Brightness) | Missing | 3d-views-cameras.md |
| C-169 | Adjust Lights dialog columns (fixtures, In Use, Brightness, Show Position) | Partial | 3d-views-cameras.md |
| C-170 | Create/Edit Light Set from Selection | Missing | 3d-views-cameras.md |
| C-171 | Adjust Area Light Material dialog | Missing | 3d-views-cameras.md |
| C-172 | Light Specification dialog (Location, Light Data, Layer, Label) | Missing | 3d-views-cameras.md |
| C-173 | North Pointer drives the sun and bearings | Partial | 3d-views-cameras.md |
| C-174 | Next and Previous Rendering Technique commands | Missing | 3d-views-cameras.md |
| C-175 | Backdrops in the library: import, folder import, HDR and the Backdrop Specification dialog | Partial | 3d-views-cameras.md |
| CAD-136 | Image picture formats (GIF, TIFF, BMP, WebP) and the Data folder copy | Partial | dimensions-text-cad.md |
| CAD-137 | Create Image Library from a folder of pictures | Partial | dimensions-text-cad.md |
| CAD-138 | Image Specification: 2D plan symbol, elevation reference, reverse, copyright, fill style | Partial | dimensions-text-cad.md |
| L-76 | Export Picture options: active window size, open in viewer, remembered settings | Partial | documentation-layout.md |
| CAD-139 | Picture Box objects (import, Picture Box Specification, cropping) | Partial | dimensions-text-cad.md |
| L-77 | PDF Box objects and page-range import | Partial | documentation-layout.md |
| C-176 | Orbital and Stationary Walkthroughs (sun study) | Missing | 3d-views-cameras.md |
| C-177 | Frame-by-frame walkthrough recording (Record, Pause, Save Frame, Stop) | Missing | 3d-views-cameras.md |
| C-178 | Walkthrough path labels, Reverse Direction and Key Frame tools | Partial | 3d-views-cameras.md |
| C-179 | Key Frame fields: time, speed after, sunlight, pause, floor | Partial | 3d-views-cameras.md |
| C-180 | Walkthrough path across floors (stairs and ramps) | Missing | 3d-views-cameras.md |
| C-181 | Walkthrough Path Specification panels (Key Frames list, Resolution, Duration, Camera, Backdrop, Line Style, Label) | Partial | 3d-views-cameras.md |
| APP-141 | Screen Capture tools (Picture, Backdrop, Material or Image) | Missing | preferences-hotkeys-toolbars.md |
| APP-142 | Capture View to Clipboard | Missing | preferences-hotkeys-toolbars.md |
| L-78 | DWG import and the Import Drawing dialog options (CAD blocks, current view, library) | Partial | documentation-layout.md |
| L-79 | Import Drawing entity coverage (ellipse, spline, hatch, solid, dimension, attribute, point) | Partial | documentation-layout.md |
| L-80 | Import Drawing Assistant (join lines, boxes, layers, duplicate blocks, unit scale) | Partial | documentation-layout.md |
| L-81 | DXF/DWG export: AutoCAD version, DWG and binary DXF, split wall assemblies, pattern lines | Partial | documentation-layout.md |
| L-82 | Export 360 Panorama: Save as Backdrop and HDR output | Partial | documentation-layout.md |
| L-83 | Thermal Envelope Data (CSV) and REScheck (.rxl) export | Missing | documentation-layout.md |
