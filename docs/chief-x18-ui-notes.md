# Chief Architect X18 — UI study

Observations taken from Chief Architect Premier X18 on macOS on 2026-10-07,
used only to inform Plan Studio's layout and workflow. No Chief code or assets
are used.

## Window layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ Toolbar row 1: file/edit, view selector, display + view tools        │
│ Toolbar row 2: Build tools (select, walls, doors, windows, cabinets… │
├───────────────────────────────────────────────┬──────────────────┬───┤
│ Document tab: "Untitled 1: Floor Plan View…"  │ Docked side panel │ V │
│                                               │ (Active Layer     │ e │
│ Drawing area (light paper background, grid)   │  Display Options, │ r │
│                                               │  Library Browser, │ t │
│                                               │  Project Browser) │ . │
├───────────────────────────────────────────────┴──────────────────┴───┤
│ Status bar:                 X: -1222 5/8", Y: 722 3/8", Z: 0"  2192 x 12…│
└──────────────────────────────────────────────────────────────────────┘
```

- A view-name dropdown ("Floor Plan View Dimensioned") in toolbar row 1
  switches saved plan views, each carrying its own layer set.
- The right-edge vertical toolbar holds view/navigation tools: zoom in/out,
  fill window, pan, layer display, reference display, select same type.
- Status bar: live cursor coordinates in architectural units with fractions,
  plus the current floor and drawing extents. Plan Studio mirrors this.

## Build toolbar (row 2), left to right

Select Objects, Wall tools, Railing/fencing, Curved walls, Door tools,
Window tools, Cabinet tools, Electrical, Stairs, Fixtures/furniture (library),
Roof tools, Ceiling/soffit, Framing, Deck/foundation, Terrain, Cameras (3D),
Elevation cameras, Dimensions, Text, Arrows/leaders, Polyline/CAD tools
(line, arc, circle, box, spline), Layout box, CAD detail.

Each button is a flyout (small ▾) with variants, e.g. Wall → Straight
Exterior, Straight Interior, Railing, Half-wall, Hatch wall, Invisible room
divider.

## Menus

`File`: New Plan, New Layout, Templates, Open Plan, Open Layout, Open Recent,
Dashboard, Download Sample Plans, Import Project, Customize Sheet Sizes.
`Library`: Import Library, Get Additional Content, Install Core Content,
Update Library Catalogs. `3D`: Material Builder. `Tools`: Toolbars and
Hotkeys, Master List. (Most commands live in toolbars and context menus rather
than the menu bar.)

## Behaviors worth copying

1. **Plan is the model.** Draw walls; rooms, 3D, elevations and schedules are
   derived. Edits anywhere update everything.
2. **Wall drawing** is click-drag or click-click, with 15° angle snaps and
   a temporary dimension readout; walls auto-join at corners and clean up
   T-intersections.
3. **Openings are wall-hosted.** Doors and windows slide along their wall and
   are placed by clicking the wall; defaults come from a per-type Defaults
   dialog.
4. **Layers + layer sets** drive what each saved view shows.
5. **Defaults dialogs** for every object type, and a dialog on double-click
   for every object instance.
6. **Reference display** overlays the floor below in gray.
7. **Dimensions are live**: typing a value into a dimension moves the object.
