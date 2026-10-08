# Chief Architect X18 — toolbar analysis

Captured 2026-10-07 from Chief Architect Premier X18 (macOS, "Default
Configuration" toolbars) by reading the window's accessibility tree, so the
names below are Chief's own tooltips and the order is exact. This is the
specification Plan Studio's toolbars are built to. Icons in Plan Studio are
original artwork drawn in the same visual language; no Chief assets are used.

## Visual language

| Property | Chief X18 | Plan Studio target |
|---|---|---|
| Bar background | dark gray `#3A3A3A`, title strip `#2C2C2C` | same |
| Button size | 27 × 27 px, 31 px pitch | 28 × 28, 32 px pitch |
| Flyout button | 39 × 27 px: icon + 12 px `▾` zone | icon + `▾` |
| Glyph size | ~18 px inside the 27 px button | 20 px SVG in 28 px button |
| Palette | red `#C8362B` for walls, roofs, framing, structure; blue `#2F6CB3` / light blue `#8DB8E8` for doors, windows, cabinets, electrical, stairs, cameras; yellow `#E8B82A` for dimensions and measuring; white/gray for file and CAD tools | same hex values |
| Style | flat, 1–2 px strokes, small isometric hints on 3D objects, a tiny red `+` or `✓` badge on "add/define" variants | same |
| Active tool | lighter square highlight behind the icon | `#5A5A5A` fill, 3 px radius |
| Disabled | glyph at ~35 % opacity | same |

Three toolbar areas: **row 1** (file, view, floor, 3D, materials, config),
**row 2** (Build tools, left to right in construction order), and a
**vertical bar on the right edge** (browsers, zoom, display toggles).
Group separators are thin vertical lines. Flyout buttons display the most
recently used variant of their group.

## Row 1 (41 items)

| # | x | Name | Kind | Plan Studio icon id |
|---|---|---|---|---|
| 1 | 1 | New Plan | button | `file_new` |
| 2 | 32 | Open Plan | button | `file_open` |
| 3 | 63 | Save | button | `file_save` |
| 4 | 93 | Print | button | `file_print` |
| 5 | 124 | Send to Layout | button | `send_to_layout` |
| 6 | 155 | Undo | button | `undo` |
| 7 | 186 | Redo | button | `redo` |
| 8 | 217 | Preferences | button | `preferences` |
| 9 | 247 | Launch Help | button | `help` |
| 10 | 276 | Edit Active View | button | `view_edit` |
| 11 | 307 | Save Active View | button | `view_save` |
| 12 | 338 | Save Active View As | button | `view_save_as` |
| 13 | 369 | *Saved view selector* ("Floor Plan View Dimensioned") | dropdown, 270 px | `view_plan` |
| 14 | 643 | Display Options | button | `display_options` |
| 15 | 674 | Default Settings | button | `default_settings` |
| 16 | 705 | *(title withheld by the app)* node-graph glyph | button | `plan_database` |
| 17 | 733 | Floor Defaults | button | `floor_defaults` |
| 18 | 764 | Down One Floor | button | `floor_down` |
| 19 | 795 | *Current floor number* ("1") | label | — |
| 20 | 826 | Up One Floor | button | `floor_up` |
| 21 | 855 | *(title withheld)* house glyph, 3D view flyout | flyout | `view_3d` |
| 22 | 897 | Full Camera | flyout | `camera_full` |
| 23 | 939 | Mouse-Orbit Camera | flyout | `camera_orbit` |
| 24 | 982 | Cross Section Slider | flyout | `cross_section` |
| 25 | 1024 | Create Walkthrough Path | flyout | `walkthrough` |
| 26 | 1066 | Standard (rendering technique) | flyout | `render_standard` |
| 27 | 1109 | Add Lights | flyout | `add_lights` |
| 28 | 1151 | Sun Angle | toggle | `sun_angle` |
| 29 | 1182 | Material Painter | button | `material_painter` |
| 30 | 1213 | Material Eyedropper | toggle | `material_eyedropper` |
| 31 | 1244 | Object Eyedropper | toggle | `object_eyedropper` |
| 32 | 1274 | Delete Surface | toggle | `delete_surface` |
| 33 | 1305 | Adjust Material Definition | toggle | `adjust_material` |
| 34 | 1336 | Interactive Material Editor | toggle | `material_editor` |
| 35 | 1365 | Default Configuration | toggle | `config_default` |
| 36 | 1396 | Space Planning Configuration | toggle | `config_space_planning` |
| 37 | 1426 | Extended Tool Configuration | toggle | `config_extended` |

## Row 2 — Build tools (30 items)

| # | x | Name shown (current flyout pick) | Kind | Group / flyout variants (from Chief's documentation) | Icon id |
|---|---|---|---|---|---|
| 1 | 1 | Select Objects | toggle | — | `select` |
| 2 | 32 | Straight Exterior Wall | flyout | Wall tools: Straight Exterior, Straight Interior, Straight Foundation, Straight Pony, Straight Half, Straight Room Divider, Straight Attic, Hatch Wall, Break Wall, Fix Wall Connections | `wall_exterior`, `wall_interior`, `wall_foundation`, `wall_pony`, `wall_half`, `wall_room_divider`, `wall_attic`, `wall_hatch`, `wall_break`, `wall_fix` |
| 3 | 74 | Straight Railing | flyout | Railing and Deck tools: Straight Railing, Straight Deck Railing, Straight Deck Edge, Straight Glass Panel Railing | `railing`, `deck_railing`, `deck_edge`, `glass_railing` |
| 4 | 116 | Curved Exterior Wall | flyout | Curved Wall tools: Curved Exterior, Curved Interior, Curved Foundation, Curved Railing, Curved Deck Railing, Curved Room Divider | `wall_curved`, `wall_curved_interior`, `railing_curved` |
| 5 | 159 | Hinged Door | flyout | Door tools: Hinged, Sliding, Pocket, Bifold, Garage, Doorway, Barn, Shower Door | `door_hinged`, `door_sliding`, `door_pocket`, `door_bifold`, `door_garage`, `doorway`, `door_barn` |
| 6 | 201 | Window | flyout | Window tools: Window, Bay Window, Box Window, Bow Window, Pass-Through | `window`, `window_bay`, `window_box`, `window_bow`, `pass_through` |
| 7 | 244 | Base Cabinet | flyout | Cabinet tools: Base, Wall, Full Height, Soffit, Shelf, Partition, Custom Countertop | `cabinet_base`, `cabinet_wall`, `cabinet_full`, `soffit`, `shelf`, `partition`, `countertop` |
| 8 | 286 | 110V Outlet | flyout | Electrical: 110V Outlet, 220V Outlet, Light, Switch, Connect Electrical, Auto Place Outlets | `outlet_110`, `outlet_220`, `light`, `switch`, `connect_electrical` |
| 9 | 328 | Draw Stairs | flyout | Stair tools: Draw Stairs, Click Stairs, Straight Stairs, Curved Stairs, Landing, Ramp, Elevator | `stairs`, `stairs_curved`, `landing`, `ramp` |
| 10 | 371 | Build New Floor | flyout | Floor tools: Build New Floor, Build Foundation, Insert Floor, Delete Floor | `floor_new`, `foundation`, `floor_insert`, `floor_delete` |
| 11 | 413 | Roof Plane | flyout | Roof tools: Roof Plane, Build Roof, Skylight, Dormer, Gable Line, Roof Hole, Join Roof Planes | `roof_plane`, `roof_build`, `skylight`, `dormer`, `gable_line` |
| 12 | 455 | Corner Boards | flyout | Trim tools: Corner Boards, Quoins, Fascia, Frieze, Shutters | `corner_boards`, `quoins` |
| 13 | 498 | General Framing | flyout | Framing: General Framing, Floor/Ceiling Truss, Roof Truss, Post, Beam, Build Framing | `framing_general`, `truss`, `post`, `beam` |
| 14 | 540 | Joist | flyout | Floor framing: Joist, Floor/Ceiling Beam, Rim Joist | `joist` |
| 15 | 582 | Rafter | flyout | Roof framing: Rafter, Roof Beam | `rafter` |
| 16 | 625 | Slab | flyout | Slab and Terrain: Slab, Slab with Footing, Terrain Perimeter, Elevation Line, Road, Driveway | `slab`, `terrain`, `elevation_line`, `road` |
| 17 | 667 | 3D Solid | flyout | Primitives: 3D Solid, Box, Cylinder, Sphere, Cone | `solid_3d`, `box`, `cylinder` |
| 18 | 707 | Paste Hold Position | button | — | `paste_hold` |
| 19 | 738 | Manual Dimension | flyout | Dimension tools: Manual, End-to-End, Interior, Point-to-Point, Angular, Center, Baseline, Story Pole | `dim_manual`, `dim_end_to_end`, `dim_interior`, `dim_angular` |
| 20 | 781 | Auto Exterior Dimensions | flyout | Automatic dimensions: Auto Exterior, Auto Interior, Auto Story Pole, Auto Elevation | `dim_auto_exterior`, `dim_auto_interior` |
| 21 | 823 | Leader Line | flyout | Text tools: Text, Rich Text, Leader Line, Text Line with Arrow, Callout, Marker, Note | `text`, `rich_text`, `leader_line`, `arrow_line`, `callout`, `marker`, `note` |
| 22 | 865 | Revision Cloud | toggle | — | `revision_cloud` |
| 23 | 896 | Place Point | flyout | Point tools: Place Point, Input Point, Point Marker | `point` |
| 24 | 938 | Draw Line | flyout | Line tools: Draw Line, Input Line, Draw Line with Arrow | `line` |
| 25 | 981 | Draw Arc | flyout | Arc tools: Draw Arc, Arc with Arrow, Input Arc | `arc` |
| 26 | 1023 | Circle | flyout | Circle tools: Circle, Oval, Ellipse, Circle About Center | `circle`, `ellipse` |
| 27 | 1065 | Rectangular Polyline | flyout | Box tools: Rectangular Polyline, Polyline, Polygon | `rect_polyline`, `polyline`, `polygon` |
| 28 | 1108 | Spline | toggle | — | `spline` |
| 29 | 1139 | Auto Detail | button | — | `auto_detail` |
| 30 | 1169 | Current CAD Layer | button | — | `cad_layer` |

## Right-edge vertical bar (21 items, top to bottom)

| # | y | Name | Kind | Icon id |
|---|---|---|---|---|
| 1 | 89 | Library Browser | toggle | `library_browser` |
| 2 | 120 | Project Browser | toggle | `project_browser` |
| 3 | 151 | Active Layer Display Options | toggle | `layer_display` |
| 4 | 180 | Zoom | toggle | `zoom` |
| 5 | 211 | Zoom In | button | `zoom_in` |
| 6 | 241 | Zoom Out | button | `zoom_out` |
| 7 | 272 | Undo Zoom | button | `zoom_undo` |
| 8 | 303 | Fill Window Selected Objects | button | `fill_selected` |
| 9 | 334 | Fill Window Building Only | button | `fill_building` |
| 10 | 365 | Fill Window | button | `fill_window` |
| 11 | 395 | Pan Window | toggle | `pan` |
| 12 | 424 | Reference Display | toggle | `reference_display` |
| 13 | 455 | Crosshairs | toggle | `crosshairs` |
| 14 | 486 | Color | toggle | `color` |
| 15 | 517 | Line Weights | toggle | `line_weights` |
| 16 | 547 | Drawing Sheet | toggle | `drawing_sheet` |
| 17 | 578 | Print Preview | toggle | `print_preview` |
| 18 | 609 | Temporary Dimensions | toggle | `temp_dimensions` |
| 19 | 640 | Connect CAD Segments | toggle | `connect_cad` |
| 20 | 670 | Arc Centers and Ends | toggle | `arc_centers` |
| 21 | 720 | *overflow chevron* | — | — |

## Behaviors

- Clicking the icon half of a flyout activates the shown variant; clicking
  the `▾` half (or press-and-hold) opens the variant list. The chosen variant
  becomes the button's face.
- Exactly one drawing tool is active at a time; `Select Objects` is the
  resting state and the Escape key returns to it.
- Toggles (Reference Display, Crosshairs, Temporary Dimensions, …) are
  independent view flags with a pressed look.
- Tooltips show the name and hotkey; the status bar's left segment repeats a
  one-line hint for the active tool.
- Toolbars are per-view-type: 3D views and layout pages show different rows.
  Plan Studio starts with the floor-plan set only.
