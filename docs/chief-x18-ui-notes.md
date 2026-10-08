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

## Plan Studio accessibility measurements (2026-10-08)

Plan Studio's own choices for a light-sensitive user; the numbers are asserted
in `theme.rs` tests (WCAG contrast ratio, sRGB luminance).

### UI scale and text

* UI scale 100 / 125 / 150 / 175 % is egui's zoom factor (`AppSettings::ui_scale_pct`,
  settings key `ui_scale_pct`). Text, toolbar buttons, dock widths, dialogs, handles
  and icons all grow together; the SVG icons are rasterized at points x
  pixels-per-point, so they stay crisp. `theme::sync_scale` keeps the setting and the
  zoom in step with Preferences > Text size and Cmd+Plus / Cmd+Minus; the range is clamped.
* Body text 15 pt (never smaller), 17 pt with "Larger text" (`large_text`); headings
  body + 3; small 11 pt minimum. Rows are 24 pt tall, item spacing 5 pt.
* "Reduce motion" (`reduce_motion`) sets egui's `animation_time` to 0 and turns scroll
  animation off.
* Dock widths (`dock_widths`: library, project, layers, properties) are saved when
  the mouse is released; they are limited to 220..700 pt and half the window.
* Dialog size and position are remembered per dialog type under `dialog_geometry`.

### Chrome colors and contrast (text on panel and the other pairs)

| Pair | Default (Paper / Low Glare / Dark) | High Contrast |
|---|---|---|
| text on panel (`#F2F2F2` / `#FFFFFF` on `#3A3A3A` / `#000000`) | 10.16 | 21.00 |
| text on hover fill (`#4A4A4A` / `#2E2E2E`) | 7.92 | 13.58 |
| text on accent (`#4D8EDC` / `#0B57D0`) | 3.02 | 6.39 |
| text on status bar (`#2C2C2C` / `#000000`) | 12.47 | 21.00 |
| text on active button (`#5A5A5A` / `#0B57D0`) | 6.16 | 6.39 |
| outline and focus ring on panel (`#8A8A8A` / `#D8D8D8`) | 3.29 | 14.73 |
| accent on panel | 3.37 | 3.29 |

Every pair is at least 3:1 in every theme; text on the panel is at least 7:1.

### Canvas themes: background and grid, and ink on the background

| Theme | Background | Minor / major grid | Luminance | Selection | Hover | Text |
|---|---|---|---|---|---|---|
| Paper | `#FAFAF7` | `#E8E8E8` / `#D7D7D7` | 0.954 | `#CD5F00` 3.84 | `#288CFF` 3.20 | 15.94 |
| Low Glare (default) | `#C4C0B8` | `#B0ACA4` (1.25) / `#A09C94` (1.51) | 0.529 | `#AA4600` 3.23 | `#2864AA` 3.31 | 9.19 |
| Dark | `#26282C` | `#34363A` / `#424448` | 0.021 | `#FFAA3C` 7.78 | `#5AA0FF` 5.56 | 10.77 |
| High Contrast | `#FFFFFF` | `#C8C8C8` / `#969696` | 1.000 | `#FF0000` 4.00 | `#0000FF` 8.59 | 21.00 |

Low Glare keeps the background at about half the paper's luminance and the grid
lines 1.25:1 and 1.51:1 against it, so the grid never competes with the walls (wall
stroke `#282828`, 8.1:1). The old Low Glare selection `#C86414` was 2.2:1 and the
Paper one `#FF8C00` 2.2:1; both were darkened to reach 3:1.

### Toolbar, docks, status bar, dialogs

* Toolbar buttons 28 pt with a 4 pt gap (32 pt pitch), glyph 20 pt. Hover: 1.5 pt
  outline ring (3.29:1); active tool: `#5A5A5A` fill plus a 2 pt accent ring; keyboard
  focus: 2 pt white ring. Flyout rows are 30 pt tall and 280 pt wide.
* Tooltips: name, hotkey in brackets, and a one-line description taken from the
  manual's command tables (`shell/tooltips.rs`) when one exists.
* Icons: every icon with colored ink carries the white halo underlay (white stroke,
  0.5 opacity, stroke width + 1.2); white and light-gray icons are their own halo on
  the dark chrome. `icons.rs` tests list offenders.
* Docks: tab strip (Library, Project, Layers) with the active tab lit and an accent
  bar; Tab / Shift+Tab walk the controls, arrow keys move between rows and open or
  close tree nodes, Enter presses, Escape and F6 leave the dock.
* Status bar: X / Y, tool hint, floor, layer set, zoom (percent of life size at 96 pt
  per inch), Undo label, "Saved n min ago", and a click-through log of the last 50
  messages. The "Aa" button opens the Appearance menu.
* Dialogs: Enter is OK unless a button has the focus, Esc is Cancel, Cmd+Enter is
  Apply (`SpecDialog::show_with_apply`); the window never opens larger than 96 % of
  the screen and drops the preview panel below 560 pt.
