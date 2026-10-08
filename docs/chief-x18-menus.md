# Chief Architect X18 — pull-down menu analysis

Captured 2026-10-07 from Chief Architect Premier X18 on macOS with a plan
view active. `|` marks a separator, `▸` a submenu, `(off)` an item that was
disabled in an empty plan, `✓` a checked toggle. Shortcuts are Chief's macOS
defaults. The last column says how Plan Studio treats each item.

Menu bar: **File · Edit · Build · Terrain · Library · 3D · CAD · Tools · View
· Window · Account · Help** (plus the macOS application menu holding
About, Preferences…, Services, Hide, Quit).

## File

New Plan ⌘N · New Layout · Templates ▸ (New Plan From Template…, New Layout
From Template…) | Open Plan… ⌘O · Open Layout… · Open Recent Documents ▸ |
Dashboard… · Download Sample Plans… | Close View ⌘W · Close All 3D Views ·
Close All Views | Save ⌘S · Save As… · Save As Template… · Save Thumbnail
Image (off) · Show in Project Browser · View File Information… · Manage Auto
Archives… (off) | Export ▸ · Import ▸ · Print ▸ | Send to Layout… (S, L)

Plan Studio: New, Open, Recent, Save, Save As implemented; Close View,
Export ▸ (DXF, PDF, glTF), Import ▸, Print ▸ and Send to Layout are
Phase 1/4 items and appear disabled until then.

## Edit

Undo ⌘Z · Redo ⌘Y | Cut ⌘X · Copy ⌘C · Copy and Paste in Place (C, P, P) ·
Paste ▸ | Delete ⌦ · Delete Objects… ⇧Space | ✓ Select Objects Space ·
Select All ⌘A | Snap Settings ▸ · Edit Behaviors ▸ · Arc Creation Modes ▸ |
Edit Area ▸ · Stretch CAD | Find/Replace Text… · Replace Fonts… | Default
Settings… · Reset to Defaults… | AutoFill ▸ · Start Dictation · Emoji &
Symbols (macOS-supplied)

Notable: **Space** toggles Select Objects, which is also the resting state of
every tool. Snap Settings ▸ holds Object Snaps, Angle Snaps, Grid Snaps,
Bumping/Pushing and the individual object-snap types. Edit Behaviors ▸ holds
Default, Replicate, Resize, Concentric, Fillet, Chamfer.

## Build

Wall ▸ · Railing and Deck ▸ · Fencing ▸ · Door ▸ · Window ▸ · Floor ▸ ·
Roof ▸ · Slab ▸ · Framing ▸ · Trim ▸ · Stairs ▸ · Cabinet ▸ · Electrical ▸ ·
3D Solid ▸ · Image ▸ · Distributed Objects ▸

Each submenu is the same list as the matching toolbar flyout; the exact
contents are recorded in `chief-x18-subtools.md`.

## Terrain

Create Terrain Perimeter | Terrain Specification… (off) · Build Terrain (off)
· Clear Terrain (off) | Elevation Data ▸ · Modifier ▸ · Feature ▸ · Garden
Bed ▸ · Grass Region ▸ · Water Feature ▸ · Stepping Stone ▸ · Terrain Wall
and Curb ▸ · Road ▸ · Driveway ▸ · Sidewalk ▸ · Plant ▸ · Sprinkler ▸

## Library

Import Library (.calib, .calibz)… | Get Additional Content… · Install Core
Content · Update Library Catalogs

## 3D

Create Orthographic View ▸ · Create Perspective View ▸ · Create Auto
Elevations ▸ | Move Camera with Mouse ▸ · Move Camera with Keyboard ▸ · Move
Camera ▸ · Orbit Camera ▸ · Tilt Camera ▸ · View Direction ▸ · Isometric
Views ▸ | Walkthroughs ▸ | Materials ▸ · Material Painter ▸ · Adjust
Materials ▸ · Adjust 3D Cladding ▸ · Material Builder… | Lighting ▸ · Camera
View Options ▸ · Rendering Techniques ▸ · Toggle Patterns (off) · Delete
Surface (off) · Rebuild 3D | 3D View Defaults… ⌘1

## CAD

Current CAD Layer… | Points ▸ · Lines ▸ · Arcs ▸ · Circles ▸ · Boxes ▸ ·
Revision Cloud · Spline | Dimensions ▸ · Automatic Dimensions ▸ | Text ▸ ·
Patterns ▸ · CAD Blocks ▸ | Sun Angle · North Pointer | Plan Footprint · Auto
Detail (off) | CAD Block Management… · CAD Detail Management… · CAD Detail
From View | CAD to Walls…

## Tools

Layer Settings ▸ · Floor/Reference Display ▸ · Active View ▸ · Active
Defaults… | Checks ▸ · Toolbars and Hotkeys ▸ · Symbol ▸ · Space Planning ▸ ·
Plan Database ▸ · Time Tracker ▸ · Schedules ▸ · Materials List ▸ · Object
Painter ▸ · Fill Style Painter ▸ | Project Information… · Loan Calculator… ·
Ruby Console… | Screen Capture ▸ · Color Chooser… | New Plan View · Rotate
Plan View… · Reverse Plan

## View

Refresh Display F5 | Library Browser ⌘L · Project Browser · Tool Palette ·
✓ Active Layer Display Options · Walkthrough Preview · Action History |
✓ Status Bar · ✓ Scrollbars · ✓ Toolbars | ✓ Color F8 · Crosshairs ·
Coordinate System Indicator – Floating · – Fixed · – Origin | ✓ Reference
Grid ⇧F9 · Angle Snap Grid · ✓ Temporary Dimensions · Arc Centers and Ends |
✓ Line Weights · Drawing Sheet · Watermark | Enter Full Screen

Plan Studio: these are the same flags as the right-edge toolbar toggles, so
the menu and the bar share one `ViewFlag` set.

## Window

Zoom ⇧Z · Zoom Out − · Zoom In + · Undo Zoom · Fill Window Building Only ·
Fill Window Selected Objects · Fill Window ⌃F · Pan Window H · Swap Views F7
| Tile Horizontally · Tile Vertically · Tab Windows | Select Next Tab ⌃⇥ ·
Select Previous Tab ⌃⇧⇥ | ✓ Untitled 1: Floor Plan View Dimensioned

## Account

Sign In to Chief Architect Account… · Make License Available… · My Account…

## Help

Launch Help… · View Tutorial Guide… · View Reference Manual… · Visit Chief
Architect Website… · View Training Videos… · Download Program Updates… ·
ChiefTalk… · Technical Support… · Export Logs… · System Information…

## Design notes for Plan Studio

1. Keep Chief's menu titles and order so muscle memory transfers: File, Edit,
   Build, Terrain, Library, 3D, CAD, Tools, View, Window, Help. Account is
   omitted (no licensing).
2. Build and CAD menus are generated from the same tool tables that drive the
   toolbars, so a sub-tool exists in exactly one place in code.
3. Unbuilt items are shown disabled with the Chief name, never hidden, so the
   target shape of the product is visible from day one.
4. Shortcuts to honor first: ⌘N/⌘O/⌘S, ⌘Z/⌘Y, Space (select), ⌃F (fill
   window), H (pan), F8 (color), ⇧F9 (grid), ⌘L (library).
