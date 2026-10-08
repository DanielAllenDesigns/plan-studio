# Chapter 13: Hotkeys

Plan Studio is driven from the keyboard the way Chief Architect is: single keys for the
common tools, two- and three-key sequences for families (`D, H` for a hinged door), and
chords with all four modifiers for the rest. This chapter lists every default binding,
explains how Daniel's Chief hotkeys are loaded on top of them, and documents the
Customize Hotkeys dialog.

## 13.1 Three layers

The keys you get are built in three layers, each overriding the one before:

```
 1. Base table      toolbar::BINDINGS        (this chapter, 13.3)
 2. Daniel's Chief  UserHotkeys.xml          (13.4)  embedded in the program
 3. Your edits      ~/.plan-studio/hotkeys.json   written by Customize Hotkeys (13.7)
```

A **command** is anything the program can do and name: every toolbar button and flyout entry
(for example "Straight Exterior Wall"), plus a few extras (Save As, Quit, Save Current Defaults as
My Template, Reset to Chief X18 Template, Reference Grid, Current Wall Tool, Customize Hotkeys,
Layer Display Options, About Plan Studio). Bindings are stored by **command name**, so a key
works the moment the command is built, with no other change. A command that is still dimmed counts
as known but not live: its key reports "Not yet implemented: <name>" in the status bar.

When two layers want the same key, the later layer wins and the earlier binding is removed from the
other command. Plan Studio's own number-key aliases (`1` to `4`) and `Shift+Cmd+Z` for Redo survive
unless one of Daniel's keys takes the chord.

## 13.2 Rules

- Keys are ignored while a text field has focus, while a dialog is recording a key, and while a tool
  is collecting typed input (a length, an elevation, text).
- A **sequence** is one to four chords. After the first chord the status bar shows `D, ...` for 1.5
  seconds; the next chord must arrive in that time or the prefix is dropped. `Esc` clears a pending
  prefix. If a key is not a valid continuation, it is treated as a fresh press.
- A key can be both a complete binding and the start of a longer one only if the longer one is
  assigned; the dialog reports such prefix clashes as conflicts.
- Modifiers: `Ctrl` is the physical Control key and `Cmd` the Command key on a Mac. On Windows and
  Linux the program maps the Control key to `Cmd`, and `Ctrl` is not available, so every chord that
  needs both (`Ctrl+Alt+Cmd+6` and the other four-modifier chords) cannot be typed there (planned fix).
- `Alt` is Option on a Mac.
- Tools never read raw keys to activate themselves. Inside a running tool, keys such as `Esc`, `Enter`,
  `Tab`, `Delete` and the arrow keys go to the tool (13.9).

## 13.3 The base table (`toolbar::BINDINGS`)

The fixed table in `crates/plan-app/src/toolbar.rs`. These work with no Daniel file and no user edits.

| Key | Command | Note |
|---|---|---|
| `Space` | Select Objects |  |
| `Shift+Q` | Straight Exterior Wall |  |
| `Shift+W` | Window |  |
| `D, H` | Hinged Door |  |
| `Shift+Y` | Draw Stairs |  |
| `Shift+T` | Base Cabinet |  |
| `Shift+A` | Auto Exterior Dimensions |  |
| `Q` | Roof Plane |  |
| `Y` | Text |  |
| `K` | Circle |  |
| `Shift+P` | Rectangular Polyline |  |
| `Shift+J` | Full Camera |  |
| `Shift+K` | Perspective Full Overview |  |
| `Ctrl+F` | Fill Window |  |
| `H` | Pan Window |  |
| `F8` | Color flag |  |
| `Shift+F9` | Reference Grid flag |  |
| `Cmd+L` | Library Browser dock |  |
| `Cmd+Z` | Undo |  |
| `Shift+Cmd+Z` | Redo | Plan Studio alias |
| `Cmd+Y` | Redo | Daniel's file also binds Cmd+Y |
| `Cmd+N` | New Plan |  |
| `Cmd+O` | Open Plan |  |
| `Cmd+S` | Save |  |
| `1` | Select Objects | Plan Studio alias (kept unless a Daniel binding takes the key) |
| `2` | The wall flyout's current pick | Plan Studio alias (kept unless a Daniel binding takes the key) |
| `3` | Hinged Door | Plan Studio alias (kept unless a Daniel binding takes the key) |
| `4` | Window | Plan Studio alias (kept unless a Daniel binding takes the key) |

Notes:

- `2` starts whichever wall variant is currently on the Straight Wall button's face.
- `Space` returns to Select Objects from any tool.
- `Q`, `Y`, `K`, `Shift+A`, `Shift+P`, `Shift+T`, `Shift+Y` start tools that have several variants; the
  key picks the named variant (Roof Plane, Text, Circle, Auto Exterior Dimensions, Rectangular
  Polyline, Base Cabinet, Draw Stairs).
- Chief calls `Ctrl+F` Fill Window. Daniel binds the same key.

## 13.4 Daniel's Chief hotkeys

Daniel's Chief Architect X18 hotkey file (`UserHotkeys.xml`, product 28.1.1.6) has 2,284 command
records, of which **208 carry a key**. The `plan-config` crate embeds that file at compile time and
reads it at every start. The file stores command **ids**, not names, so the program recovers names
from three sources, strongest first: the toolbar name table (42 of the 208), a small known-id table (none
needed today) and a match against the documented default hotkey of exactly one command (101). That
names **143 of 208**; **65** remain id-only (13.5). The Customize Hotkeys dialog reports the tally at
the top ("Daniel's Chief hotkeys: N named, N have a Plan Studio command (N work today)").

The names recovered by the third source are inferences. If you rebound a chord that happens to equal another
command's default, the inferred name could be wrong. The generated list
`docs/chief-hotkeys-resolved.md` shows the source of every name.

Two details worth knowing:

- **Qt swaps the Mac modifiers.** In the XML, `Ctrl` is the Command key and `Meta` is the Control key. The loader
  swaps them once, so the table below uses the physical keys: `Ctrl` = Control, `Cmd` = Command.
- Daniel's setup is in `docs/daniel-chief-setup.md`: 101 of his bindings differ from factory (86 new, 15 changed),
  7 factory keys were cleared, 60 commands use `Ctrl+Alt+Cmd+...` chords, and 26 use two- and three-key sequences
  (`D, H`, `S, L`, `E, O`, `D, T, M`, `E, A, O`). Examples of changes: Send to Layout `S, L`, Down One Floor
  `Ctrl+Z`, Up One Floor `Ctrl+A`, Fill Window `Ctrl+F`, Zoom In `-`, Pan Window `H`.

### The named bindings

Status: **Works** (the command runs today), **(planned)** (the command is on a dimmed button; the key is kept and reports
"Not yet implemented"), **No matching command yet** (Chief has the command; Plan Studio has no equivalent
and shows the key under "Chief bindings with no action in Plan Studio yet" in the dialog), or a flag toggle.
Of the 143 named bindings, 82 work or toggle a flag, 37 are planned and 24 have no matching command.

| Command | Daniel's key | Status |
|---|---|---|
| 110V Outlet | `E, O` | Works |
| 220V Outlet | `Ctrl+Alt+Cmd+7` | Works |
| 3D View Defaults | `Cmd+1` | No matching command yet |
| Adjust Lights | `Ctrl+Alt+Cmd+L` | No matching command yet |
| Angular Dimension | `Ctrl+Alt+Cmd+F` | Works |
| Auto Dormer | `Ctrl+Alt+Shift+Cmd+Z` | (planned) |
| Auto Elevation Dimensions | `Ctrl+Alt+Cmd+H` | (planned) |
| Auto Exterior Dimensions | `Shift+A` | Works |
| Auto Floating Dormer | `Ctrl+Alt+Shift+Cmd+R` | (planned) |
| Auto Place Outlets | `E, A, O` | Works |
| Auto Story Pole Dimensions | `Ctrl+Alt+Cmd+I` | (planned) |
| Barn Door | `Ctrl+Alt+Cmd+P` | (planned) |
| Base Cabinet | `Shift+T` | Works |
| Base Filler | `Ctrl+Alt+Cmd+0` | (planned) |
| Baseline Dimension | `Ctrl+Alt+Cmd+D` | Works |
| Bay Window | `Ctrl+Alt+Cmd+S` | (planned) |
| Bifold Door | `Ctrl+Alt+Cmd+O` | (planned) |
| Bow Window | `Ctrl+Alt+Cmd+T` | (planned) |
| Box Window | `Ctrl+Alt+Cmd+U` | (planned) |
| Build Foundation | `Cmd+F` | Works |
| Build Framing | `Shift+Cmd+S` | (planned) |
| Build New Floor | `Shift+X` | Works |
| Build Roof | `Ctrl+Alt+Shift+Cmd+N` | Works |
| Bumping/Pushing | `F11` | No matching command yet |
| CAD Block Management | `V` | (planned) |
| CAD Detail Management | `Shift+V` | No matching command yet |
| Callout | `Ctrl+Alt+Cmd+K` | Works |
| Ceiling Plane | `Ctrl+Alt+Shift+Cmd+U` | (planned) |
| Centerline Dimension | `Ctrl+Alt+Cmd+G` | Works |
| Change Floor/Reference | `Shift+Cmd+G` | No matching command yet |
| Circle | `K` | Works |
| Close View | `Cmd+W` | No matching command yet |
| Color | `F8` | Toggles a flag; no visible effect yet |
| Concentric | `X, C` | No matching command yet |
| Connect CAD Segments | `Shift+F8` | Works |
| Copy | `Cmd+C` | No matching command yet |
| Copy and Paste in Place | `C, P, P` | No matching command yet |
| CPU Ray Trace | `J` | No matching command yet |
| Curve to Left | `Ctrl+Alt+Shift+Cmd+E` | Works |
| Curve to Right | `Ctrl+Alt+Shift+Cmd+F` | Works |
| Custom Backsplash | `Ctrl+Alt+Cmd+4` | (planned) |
| Custom Counter Hole | `Ctrl+Alt+Cmd+5` | (planned) |
| Custom Countertop | `Ctrl+Alt+Cmd+3` | (planned) |
| Cut | `Cmd+X` | No matching command yet |
| Delete | `Del` | No matching command yet |
| Delete Ceiling Planes | `Ctrl+Alt+Shift+Cmd+X` | (planned) |
| Delete Current Floor | `Ctrl+Alt+Shift+Cmd+J` | Works |
| Delete Foundation | `Ctrl+Alt+Shift+Cmd+K` | Works |
| Delete Roof Planes | `Ctrl+Alt+Shift+Cmd+W` | Works |
| Display Options | ``` | (planned) |
| Doorway | `D, W` | (planned) |
| Down One Floor | `Ctrl+Z` | Works |
| Draw Ramp | `Ctrl+Alt+Shift+Cmd+H` | Works |
| Draw Stairs | `Shift+Y` | Works |
| Drawing Sheet | `Alt+F3` | Toggles a flag; no visible effect yet |
| Edit All Roof Planes | `Ctrl+Alt+Shift+Cmd+P` | Works |
| Electrical Connection | `E, C` | Works |
| End to End Dimension | `D, E` | Works |
| Exchange With Floor Above | `Ctrl+Alt+Shift+Cmd+L` | Works |
| Exchange With Floor Below | `Ctrl+Alt+Shift+Cmd+M` | Works |
| Fill Window | `Ctrl+F` | Works |
| Fixed Door | `Ctrl+Alt+Cmd+R` | (planned) |
| Floor Defaults | `Shift+Cmd+Y` | (planned) |
| Full Camera | `Shift+J` | Works |
| Full Height | `Ctrl+Alt+Cmd+X` | Works |
| Full Height Filler | `Ctrl+Alt+Cmd+2` | (planned) |
| Gable/Roof Line | `Ctrl+Alt+Shift+Cmd+O` | Works |
| Garage Door | `G, D` | (planned) |
| GFCI Outlet | `Ctrl+Alt+Shift+Cmd+Y` | Works |
| Grid Snaps | `Cmd+F9` | No matching command yet |
| Hinged Door | `D, H` | Works |
| Insert New Floor | `Ctrl+Alt+Shift+Cmd+I` | Works |
| Interior Dimension | `D, I` | Works |
| L-Shaped Stair | `Ctrl+Alt+Shift+Cmd+C` | Works |
| Landing | `Ctrl+Alt+Shift+Cmd+G` | Works |
| Leader Line | `Alt+L` | Works |
| Library Browser | `Cmd+L` | Works |
| Light | `E, L` | Works |
| Manual Dimension | `Ctrl+Alt+Cmd+A` | Works |
| Marker | `Ctrl+Alt+Cmd+M` | Works |
| Master List | `Cmd+M` | No matching command yet |
| New Plan | `Cmd+N` | Works |
| New Project | `Cmd+N` | Works |
| Note | `Ctrl+Alt+Cmd+N` | Works |
| Object Eyedropper | `Shift+B` | (planned) |
| Object Snaps | `Shift+F11` | No matching command yet |
| Open Plan | `Cmd+O` | Works |
| Open Plan/Layout | `Cmd+O` | Works |
| Orthogonal Extensions | `\` | No matching command yet |
| Page Down | `Shift+M` | No matching command yet |
| Page Up | `Shift+N` | No matching command yet |
| Pan Window | `H` | Works |
| Partition | `Ctrl+Alt+Cmd+Z` | Works |
| Pass-Through | `Ctrl+Alt+Cmd+V` | (planned) |
| Paste Hold Position | `Alt+Cmd+V` | (planned) |
| Perpendicular Extensions | `]` | No matching command yet |
| Perspective Full Overview | `Shift+K` | Works |
| Pocket Door | `D, P` | (planned) |
| Point to Point Dimension | `Ctrl+Alt+Cmd+B` | Works |
| Preferences | `~` | (planned) |
| Print | `Cmd+P` | (planned) |
| Print Preview | `Alt+F2` | Toggles a flag; no visible effect yet |
| Rebuild Walls/Floors/Ceilings | `F12` | Works |
| Rectangular Polyline | `Shift+P` | Works |
| Redo | `Cmd+Y` | Works |
| Reference Display | `F9` | Toggles a flag; no visible effect yet |
| Reference Grid | `Shift+F9` | Works |
| Refresh Display | `F5` | No matching command yet |
| Revision Cloud | `Ctrl+Alt+Shift+Cmd+!` | (planned) |
| Rich Text | `Ctrl+Alt+Cmd+J` | Works |
| Roof Hole | `Ctrl+Alt+Shift+Cmd+T` | Works |
| Roof Plane | `Q` | Works |
| Rope Light | `Ctrl+Alt+Shift+Cmd+A` | Works |
| Running Dimension | `Ctrl+Alt+Cmd+C` | Works |
| Save | `Cmd+S` | Works |
| Select All | `Cmd+A` | No matching command yet |
| Select Objects | `Space` | Works |
| Send to Layout | `S, L` | (planned) |
| Shelf | `Ctrl+Alt+Cmd+Y` | Works |
| Shower Door | `Ctrl+Alt+Cmd+Q` | (planned) |
| Skylight | `Ctrl+Alt+Shift+Cmd+S` | Works |
| Sliding Door | `S, D` | (planned) |
| Soffit | `T` | Works |
| Straight Exterior Wall | `Shift+Q` | Works |
| Straight Interior Wall | `Ctrl+Alt+Cmd+6` | Works |
| Straight Railing | `Cmd+Q` | (planned) |
| Straight Stairs | `Ctrl+Alt+Shift+Cmd+B` | Works |
| Swap Views | `F7` | No matching command yet |
| Switch | `E, S` | Works |
| Tangent Extensions | `[` | No matching command yet |
| Tape Measure | `D, T, M` | Works |
| Text | `Y` | Works |
| Text Line with Arrow | `Alt+A` | Works |
| Tile Vertically | `Shift+F6` | No matching command yet |
| U-Shaped Stair | `Ctrl+Alt+Shift+Cmd+D` | Works |
| Undo | `Cmd+Z` | Works |
| Up One Floor | `Ctrl+A` | Works |
| Wall Cabinet | `Cmd+T` | Works |
| Wall Filler | `Ctrl+Alt+Cmd+1` | (planned) |
| Wall Niche | `Ctrl+Alt+Cmd+W` | (planned) |
| Window | `Shift+W` | Works |
| Zoom | `Shift+Z` | (planned) |
| Zoom In | `-` | Works |

## 13.5 Bindings whose names were not recovered

65 bindings in Daniel's file could not be named from the toolbar tables or the documented defaults.
They stay in `docs/chief-hotkeys-resolved.md` by id and are not bound in Plan Studio. Note that some of
them are single keys such as `W`, `L`, `R`, `2` and `3`; because they are unbound here, `2` and `3` keep their
Plan Studio meaning.

Key (Chief command id): `Shift+F4` (106); `W` (202); `2` (231); `Cmd+D` (237); `M, C` (242); `Shift+F3` (263); `P, P, M` (265); `3` (275); `Cmd+B` (367); `I` (472); `O` (473); `F` (490); `B` (491); `L` (492); `R` (493); `U` (496); `Shift+Cmd+R` (511); `Shift+Cmd+T` (516); `Shift+Cmd+U` (519); `Shift+Cmd+I` (520); `Shift+C` (570); `Shift+Cmd+P` (573); `D, D` (581); `C, S` (582); `Cmd+Space` (644); `Cmd+H` (646); `Tab` (650); `Cmd+E` (651); `Alt+Cmd+C` (652); `C, C` (663); `Cmd+F3` (668); `M, A` (675); `Cmd+K` (676); `Cmd+U` (713); `Shift+G` (714); `Shift+H` (716); `Shift+F12` (717); `Alt+O` (820); `Alt+P` (821); `Alt+D` (822); `Shift+Esc` (20142); `Alt+Cmd+S` (20150); `Esc` (20151); `Alt+Q` (20168); `Alt+T` (20185); `Alt+Shift+O` (20187); `Alt+Shift+T` (20188); `Alt+Shift+D` (20189); `Alt+Shift+P` (20190); `Cmd+V` (20196); `Alt+Shift+V` (20200); `1` (20223); `Cmd+R` (23413); `E, M` (23447); `M, M, U` (23452); `A, W, A` (23457); `A, W, B` (23458); `Shift+L` (23467); `Shift+R` (23468); `Shift+U` (23469); `Shift+D` (23470); `Shift+I` (23471); `Shift+O` (23472); `Ctrl+Alt+Cmd+E` (23625); `Ctrl+Alt+Shift+Cmd+V` (23732).

## 13.6 Known quirks

- **Zoom Out has no key.** Daniel rebound Zoom In to `-` and cleared Zoom Out's factory key. The Window menu still
  prints `-` next to Zoom Out and `+` next to Zoom In (its static labels), but only `-` works and it zooms *in*.
- The menus and tooltips print the hotkey of each command from the tables at build time; a key you change in the
  dialog changes what the keys do but the built-in tooltips can still show the old one.
- `Delete` and `Backspace` delete the selection through the tools' own key handling, not through the hotkey map.
  Daniel's `Del` (Delete) binding therefore shows under "no matching command" but still works.
- `~` (Preferences), `` ` `` (Display Options), `Cmd+P` (Print), `Cmd+Q` (Straight Railing) and the other dimmed
  commands hold their keys but do nothing yet.
- Four-modifier chords work on macOS only (13.2).
- The toolbar.rs comment says the four-modifier chords are "not bound on purpose" and the README repeats it.
  That predates the runtime hotkey map; today they are bound whenever Daniel's file names them.

## 13.7 The Customize Hotkeys dialog

**Tools > Toolbars and Hotkeys > Customize Hotkeys...** opens it, modeled on Chief's dialog.

| Control | What it does |
|---|---|
| **Show Commands/Hotkeys Containing** | A filter. It matches command names and the hotkey text; the list is sorted by name. |
| **Command Name / Hotkey table** | Every command and its current keys (several sequences are shown separated by semicolons). Dimmed names are commands not built yet; hover for "Not built yet; the key is kept for later". Click a row to select the command. |
| **Chief bindings with no action in Plan Studio yet** | A collapsed list of Daniel's named bindings that have no command here. |
| **Assign a sequence of up to 4 hotkeys to <command>** | Click the field (it says "Click here, then press keys"; it shows "Press keys..."), then press the keys one after another, up to four chords. `Esc` stops recording. |
| **Clear** | Empties the recorded sequence. |
| **Assign** | Adds the recorded sequence to the selected command. |
| **Already used by: ...** and **Reassign** | If the sequence is the same as, a prefix of, or begins with another command's sequence, nothing changes and the clashing commands are named. **Reassign** takes the sequence from them. |
| **Current hotkeys** and **Remove** | The selected command's sequences; pick one and press Remove. |
| **Reset Hotkeys** | Returns to the base table plus Daniel's keys, dropping all your edits. |
| **Help**, **Cancel**, **OK** | Help shows a one-line instruction. Cancel discards the edits. OK applies them and writes `~/.plan-studio/hotkeys.json`. |

The dialog edits a copy of the map. While it is open the main window ignores the keyboard so you can record any key.

### Saved file

`~/.plan-studio/hotkeys.json` holds only your **differences** from the defaults, by command name:

```json
{
  "version": 1,
  "overrides": {
    "Base Cabinet": ["Shift+B"],
    "Hinged Door": ["D, H", "Shift+D"],
    "Fill Window": []
  }
}
```

An empty list means "unbound". Key text follows Chief's file convention, where `Ctrl` is the Command key and
`Meta` is the Control key on a Mac; the dialog writes it for you, so you rarely edit the file by hand. Unknown
command names and unreadable sequences are skipped. If `HOME` is not set (some Windows setups) the dialog cannot save.

## 13.8 Keys by area

Where the manual covers each tool's keys in detail:

| Area | Chapter |
|---|---|
| Walls, Select Objects, temporary dimensions | 2 |
| Doors and windows | 3 |
| Floors, rooms | 4 |
| Dimensions, text, CAD | 5 |
| Cabinets, library | 6 |
| Stairs | 7 |
| Roofs | 8 |
| Electrical, terrain | 9 |
| 3D views | 10 |

## 13.9 Keys inside a tool

These go to the active tool, not the hotkey map.

| Key | Where | Does |
|---|---|---|
| `Esc` | Any tool | Cancels the operation in progress; otherwise returns to Select Objects. Clears a pending hotkey prefix. In a 3D view, returns to the plan. |
| `Enter` | Select, polyline and text tools | Opens the specification of the selection; finishes a polyline, text or typed value. |
| `Tab` | Select Objects | Cycles objects under the pointer. |
| `Tab` | Cabinet, Stairs, Electrical tools | Next cabinet kind; flips a stair turn; flips a wall device to the other side of the wall. |
| `Tab` | 3D view | Next camera. |
| `Delete`, `Backspace` | Select and most tools | Delete the selection (Terrain: the element under the pointer). |
| `Alt` | Wall tool | Suspends the angle snap. |
| `Shift` | Select, hotkey modifiers | Adds to the selection; `Shift`+drag pans in 3D. |
| Arrow keys | 3D Full Camera; Electrical free device | Walk; turn the device (`Shift` = 90 degrees). |
| `W` `A` `S` `D`, `Page Up`, `Page Down` | 3D Full Camera | Walk; raise or lower the eye. |
| `Backspace` | Polyline, spline | Drops the last vertex. |
