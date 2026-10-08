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
Layer Display Options, About Plan Studio, and the Edit menu commands: Cut, Copy, Paste, Paste as Group,
Copy and Paste in Place, Duplicate, Delete Objects, Select All, Select Same Type, Group, Ungroup,
Transform/Replicate Object, Rotate Selection, Reflect About Object, Reflect Copy About Object, Point to Point Move,
Center Object, Make Parallel, Make Perpendicular, Distribute Horizontally and Vertically, the six Align commands,
Move to Front and Back, Lock Selection, Unlock Selection, Send to Layer and Action History). The File menu's Close Plan, Save a Copy, Revert to Saved, Backup Entire Plan and Manage Auto Archives (Round 11) are menu commands with no hotkey of their own. Bindings are stored by **command name**, so a key
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
- Modifiers: `Ctrl` is the physical Control key and `Cmd` the Command key on a Mac. Where there is no Command
  key (Windows, Linux), Chief's "Ctrl" is the Control key: the program treats `Cmd` as Control and folds the Mac
  Control modifier into it (`shell::hotkeys::platform_modifiers`), so Chief's four-modifier chords become
  `Ctrl+Alt+...` chords (`Ctrl+Alt+Cmd+6` is typed Ctrl+Alt+6, `Ctrl+Alt+Shift+Cmd+N` is Ctrl+Alt+Shift+N). Menus,
  tooltips and the Customize Hotkeys dialog then write `Ctrl+` where this chapter writes `Cmd+`. The tables below
  keep the Mac spelling.
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
| `Cmd+S` | Save | A safe save (chapter 12.2a) |
| `Shift+Cmd+Y` | Floor Defaults | Chief's own key; bound here as an alternate default so it works even where Daniel's file does not name it (`shell::hotkeys::alt_defaults`, with Adjust Lights) |
| `Cmd+,` | Preferences | Fixed key (Ctrl+, off the Mac); not in the Customize Hotkeys table |
| `F5` | Refresh Display | Fixed key; not in the Customize Hotkeys table |
| `1` | Select Objects | Plan Studio alias (kept unless a Daniel binding takes the key) |
| `2` | The wall flyout's current pick | Plan Studio alias (kept unless a Daniel binding takes the key) |
| `3` | Hinged Door | Plan Studio alias (kept unless a Daniel binding takes the key) |
| `4` | Window | Plan Studio alias (kept unless a Daniel binding takes the key) |

The Edit menu adds its own defaults in `shell::hotkeys::edit_defaults` (Daniel's chords take them away
where they clash):

| Key | Command | Note |
|---|---|---|
| `Cmd+X`, `Cmd+C` | Cut, Copy | egui delivers these as clipboard events; the hotkey map reads them as the chords |
| `Cmd+V` | Paste | The copy hangs on the pointer until a click (`Esc` cancels) |
| `C, P, P` | Copy and Paste in Place | |
| `Cmd+D` | Duplicate | 12" right and down |
| `Cmd+A` | Select All | |
| `Cmd+G` | Group | |
| `Shift+Space` | Delete Objects | The category dialog |

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
  (`D, H`, `S, L`, `E, O`, `D, T, M`, `E, A, O`). Examples of changes: Send to Layout `S, L` (it opens the Send to Layout dialog, chapter 11.3), Down One Floor
  `Ctrl+Z`, Up One Floor `Ctrl+A`, Fill Window `Ctrl+F`, Zoom In `-`, Pan Window `H`.

### The named bindings

Status: **Works** (the command runs today), **(planned)** (the command is on a dimmed button; the key is kept and reports
"Not yet implemented"), **No matching command yet** (Chief has the command; Plan Studio has no equivalent
and shows the key under "Chief bindings with no action in Plan Studio yet" in the dialog), or a flag toggle.
Of the 143 named bindings, 122 work or toggle a flag, 3 are planned (Display Options, Object Eyedropper, Zoom), 17 have no matching command and 1 (Straight Railing) works from the toolbar but not from its key (counted from the table below at the Round 10 working tree, with Refresh Display, which became live in a later round, moved from the third group to the first; the unit test in `shell/hotkeys.rs` pins the 143 and may count 121 and 18; the unit test in `shell/hotkeys.rs` pins the 143 and the Customize Hotkeys dialog shows the live tally). Round 10 made the door and window flyout keys, Floor Defaults, Preferences, Print and Revision Cloud live, along with the clipboard keys; off macOS Down One Floor loses its key, 13.6).

| Command | Daniel's key | Status |
|---|---|---|
| 110V Outlet | `E, O` | Works |
| 220V Outlet | `Ctrl+Alt+Cmd+7` | Works |
| 3D View Defaults | `Cmd+1` | Works |
| Adjust Lights | `Ctrl+Alt+Cmd+L` | Works |
| Angular Dimension | `Ctrl+Alt+Cmd+F` | Works |
| Auto Dormer | `Ctrl+Alt+Shift+Cmd+Z` | Works |
| Auto Elevation Dimensions | `Ctrl+Alt+Cmd+H` | Works |
| Auto Exterior Dimensions | `Shift+A` | Works |
| Auto Floating Dormer | `Ctrl+Alt+Shift+Cmd+R` | Works |
| Auto Place Outlets | `E, A, O` | Works |
| Auto Story Pole Dimensions | `Ctrl+Alt+Cmd+I` | Works |
| Barn Door | `Ctrl+Alt+Cmd+P` | Works |
| Base Cabinet | `Shift+T` | Works |
| Base Filler | `Ctrl+Alt+Cmd+0` | Works |
| Baseline Dimension | `Ctrl+Alt+Cmd+D` | Works |
| Bay Window | `Ctrl+Alt+Cmd+S` | Works |
| Bifold Door | `Ctrl+Alt+Cmd+O` | Works |
| Bow Window | `Ctrl+Alt+Cmd+T` | Works |
| Box Window | `Ctrl+Alt+Cmd+U` | Works |
| Build Foundation | `Cmd+F` | Works |
| Build Framing | `Shift+Cmd+S` | Works |
| Build New Floor | `Shift+X` | Works |
| Build Roof | `Ctrl+Alt+Shift+Cmd+N` | Works |
| Bumping/Pushing | `F11` | No matching command yet |
| CAD Block Management | `V` | Works |
| CAD Detail Management | `Shift+V` | No matching command yet |
| Callout | `Ctrl+Alt+Cmd+K` | Works |
| Ceiling Plane | `Ctrl+Alt+Shift+Cmd+U` | Works |
| Centerline Dimension | `Ctrl+Alt+Cmd+G` | Works |
| Change Floor/Reference | `Shift+Cmd+G` | No matching command yet |
| Circle | `K` | Works |
| Close View | `Cmd+W` | No matching command yet |
| Color | `F8` | Toggles a flag: off draws the plan in grays |
| Concentric | `X, C` | No matching command yet |
| Connect CAD Segments | `Shift+F8` | Works |
| Copy | `Cmd+C` | Works |
| Copy and Paste in Place | `C, P, P` | Works |
| CPU Ray Trace | `J` | No matching command yet |
| Curve to Left | `Ctrl+Alt+Shift+Cmd+E` | Works |
| Curve to Right | `Ctrl+Alt+Shift+Cmd+F` | Works |
| Custom Backsplash | `Ctrl+Alt+Cmd+4` | Works |
| Custom Counter Hole | `Ctrl+Alt+Cmd+5` | Works |
| Custom Countertop | `Ctrl+Alt+Cmd+3` | Works |
| Cut | `Cmd+X` | Works |
| Delete | `Del` | No matching command yet (the tools take the key; Edit > Delete and the context menu delete the selection) |
| Delete Ceiling Planes | `Ctrl+Alt+Shift+Cmd+X` | Works |
| Delete Current Floor | `Ctrl+Alt+Shift+Cmd+J` | Works |
| Delete Foundation | `Ctrl+Alt+Shift+Cmd+K` | Works |
| Delete Roof Planes | `Ctrl+Alt+Shift+Cmd+W` | Works |
| Display Options | ``` | (planned) |
| Doorway | `D, W` | Works |
| Down One Floor | `Ctrl+Z` | Works on macOS. Off macOS it is not bound: see 13.6 (Undo keeps the key) |
| Draw Ramp | `Ctrl+Alt+Shift+Cmd+H` | Works |
| Draw Stairs | `Shift+Y` | Works |
| Drawing Sheet | `Alt+F3` | Toggles a flag: outlines the active layout's sheet |
| Edit All Roof Planes | `Ctrl+Alt+Shift+Cmd+P` | Works |
| Electrical Connection | `E, C` | Works |
| End to End Dimension | `D, E` | Works |
| Exchange With Floor Above | `Ctrl+Alt+Shift+Cmd+L` | Works |
| Exchange With Floor Below | `Ctrl+Alt+Shift+Cmd+M` | Works |
| Fill Window | `Ctrl+F` | Works |
| Fixed Door | `Ctrl+Alt+Cmd+R` | Works |
| Floor Defaults | `Shift+Cmd+Y` | Works |
| Full Camera | `Shift+J` | Works |
| Full Height | `Ctrl+Alt+Cmd+X` | Works |
| Full Height Filler | `Ctrl+Alt+Cmd+2` | Works |
| Gable/Roof Line | `Ctrl+Alt+Shift+Cmd+O` | Works |
| Garage Door | `G, D` | Works |
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
| Pass-Through | `Ctrl+Alt+Cmd+V` | Works |
| Paste Hold Position | `Alt+Cmd+V` | Works |
| Perpendicular Extensions | `]` | No matching command yet |
| Perspective Full Overview | `Shift+K` | Works |
| Pocket Door | `D, P` | Works |
| Point to Point Dimension | `Ctrl+Alt+Cmd+B` | Works |
| Preferences | `~` | Works |
| Print | `Cmd+P` | Works |
| Print Preview | `Alt+F2` | Toggles a flag: grays out everything outside the sheet |
| Rebuild Walls/Floors/Ceilings | `F12` | Works |
| Rectangular Polyline | `Shift+P` | Works |
| Redo | `Cmd+Y` | Works |
| Reference Display | `F9` | Toggles a flag: draws the floor below in gray |
| Reference Grid | `Shift+F9` | Works |
| Refresh Display | `F5` | Works (View > Refresh Display; the key is read directly) |
| Revision Cloud | `Ctrl+Alt+Shift+Cmd+!` | Works |
| Rich Text | `Ctrl+Alt+Cmd+J` | Works |
| Roof Hole | `Ctrl+Alt+Shift+Cmd+T` | Works |
| Roof Plane | `Q` | Works |
| Rope Light | `Ctrl+Alt+Shift+Cmd+A` | Works |
| Running Dimension | `Ctrl+Alt+Cmd+C` | Works |
| Save | `Cmd+S` | Works |
| Select All | `Cmd+A` | Works |
| Select Objects | `Space` | Works |
| Send to Layout | `S, L` | Works |
| Shelf | `Ctrl+Alt+Cmd+Y` | Works |
| Shower Door | `Ctrl+Alt+Cmd+Q` | Works |
| Skylight | `Ctrl+Alt+Shift+Cmd+S` | Works |
| Sliding Door | `S, D` | Works |
| Soffit | `T` | Works |
| Straight Exterior Wall | `Shift+Q` | Works |
| Straight Interior Wall | `Ctrl+Alt+Cmd+6` | Works |
| Straight Railing | `Cmd+Q` | The tool works from the toolbar and menu; the key is shown but not bound (Command-Q is the macOS Quit shortcut) |
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
| Wall Filler | `Ctrl+Alt+Cmd+1` | Works |
| Wall Niche | `Ctrl+Alt+Cmd+W` | Works |
| Window | `Shift+W` | Works |
| Zoom | `Shift+Z` | (planned) |
| Zoom In | `-` | Works |

## 13.5 Bindings whose names were not recovered

65 bindings in Daniel's file could not be named from the toolbar tables or the documented defaults.
They stay in `docs/chief-hotkeys-resolved.md` by id and are not bound in Plan Studio. Note that some of
them are single keys such as `W`, `L`, `R`, `2` and `3`; because they are unbound here, `2` and `3` keep their
Plan Studio meaning.

Two of them, `Cmd+D` (237) and `Cmd+V` (20196), are the chords Plan Studio's own Edit defaults use for Duplicate and Paste (13.3); they stay id-only in Daniel's file.

Key (Chief command id): `Shift+F4` (106); `W` (202); `2` (231); `Cmd+D` (237); `M, C` (242); `Shift+F3` (263); `P, P, M` (265); `3` (275); `Cmd+B` (367); `I` (472); `O` (473); `F` (490); `B` (491); `L` (492); `R` (493); `U` (496); `Shift+Cmd+R` (511); `Shift+Cmd+T` (516); `Shift+Cmd+U` (519); `Shift+Cmd+I` (520); `Shift+C` (570); `Shift+Cmd+P` (573); `D, D` (581); `C, S` (582); `Cmd+Space` (644); `Cmd+H` (646); `Tab` (650); `Cmd+E` (651); `Alt+Cmd+C` (652); `C, C` (663); `Cmd+F3` (668); `M, A` (675); `Cmd+K` (676); `Cmd+U` (713); `Shift+G` (714); `Shift+H` (716); `Shift+F12` (717); `Alt+O` (820); `Alt+P` (821); `Alt+D` (822); `Shift+Esc` (20142); `Alt+Cmd+S` (20150); `Esc` (20151); `Alt+Q` (20168); `Alt+T` (20185); `Alt+Shift+O` (20187); `Alt+Shift+T` (20188); `Alt+Shift+D` (20189); `Alt+Shift+P` (20190); `Cmd+V` (20196); `Alt+Shift+V` (20200); `1` (20223); `Cmd+R` (23413); `E, M` (23447); `M, M, U` (23452); `A, W, A` (23457); `A, W, B` (23458); `Shift+L` (23467); `Shift+R` (23468); `Shift+U` (23469); `Shift+D` (23470); `Shift+I` (23471); `Shift+O` (23472); `Ctrl+Alt+Cmd+E` (23625); `Ctrl+Alt+Shift+Cmd+V` (23732).

## 13.6 Known quirks

- **Zoom Out has no key.** Daniel rebound Zoom In to `-` and cleared Zoom Out's factory key. The Window menu follows
  the live hotkey map, so it shows `-` beside Zoom In and nothing beside Zoom Out; only `-` works and it zooms *in*.
- The Window menu (Zoom Out, Zoom In, Undo Zoom, Fill Window, Pan Window) and 3D > 3D View Defaults... read the
  live hotkey map, so Daniel's keys and your Customize Hotkeys edits show there. The other menus and the toolbar
  tooltips print the hotkey of each command from the tables at build time; a key you change in the dialog changes
  what the keys do but those can still show the old one.
- `Delete` and `Backspace` delete the selection through the tools' own key handling, not through the hotkey map.
  Daniel's `Del` (Delete) binding therefore shows under "no matching command" but still works.
- `` ` `` (Display Options), `Shift+B` (Object Eyedropper), `Shift+Z` (Zoom) and the other dimmed commands hold their keys but do
  nothing yet. `~` opens Preferences and `Cmd+P` the Print dialog. File > Print > Print Layout and Export Layout PDF work from the menu and have no key.
- **Adjust Lights** is `Ctrl+Alt+Cmd+L` on a Mac. Off macOS the Control and Command flags fold into one key, so the chord is `Ctrl+Alt+L` (the program binds it as a Chief default that needs Option, which the base table cannot express).
- **Revision Cloud** (`Ctrl+Alt+Shift+Cmd+!`) starts the tool from its toolbar toggle. `Cmd+Q` is shown beside Straight Railing but not bound, because it is the macOS Quit shortcut; the tool itself works from the
  toolbar and the Build menu.
- **Control+Z and Command+Z off macOS (Round 8).** Daniel binds Control+Z to Down One Floor and Command+Z to Undo. A Mac keeps the two keys apart. On Windows and Linux there is one Ctrl key, so both
  chords become `Ctrl+Z` and the program must pick one. **The Command chord wins: `Ctrl+Z` is Undo**, as in Chief's own Windows defaults, and **Down One Floor is reported as unmapped there**
  (Customize Hotkeys lists it under "Chief bindings with no action in Plan Studio yet" with the note "shared off macOS"). Up One Floor (`Ctrl+A`) is not affected. You can give Down One Floor another key in
  Customize Hotkeys; the open question is `DECISIONS.md` item 4. The rule is general: whenever two of Daniel's chords fold onto one sequence, a chord that used Command beats one that used Control.
- **`Cmd+Q` on a Mac** is the system's Quit and skips the unsaved-changes prompt: the program keeps unsaved work in `~/.plan-studio/recovery/` and offers it at the next launch (chapter 12.2a). File > Quit and the window's close button ask first.
- Four-modifier chords work on every platform; off macOS they are the `Ctrl+Alt+...` chords of 13.2.
- Older notes say the four-modifier chords are "not bound on purpose". That predates the runtime hotkey map;
  today they are bound whenever Daniel's file names them.

## 13.7 The Customize Hotkeys dialog

**Tools > Toolbars and Hotkeys > Customize Hotkeys...** opens it, modeled on Chief's dialog.

| Control | What it does |
|---|---|
| **Show Commands/Hotkeys Containing** | A search. It matches the command name, its group and its hotkey text; the list is sorted by group, then name. |
| **Show:** All, Assigned, Unassigned, In conflict | A filter on the table: every command, only the commands that have a key, only those without one, or only the commands whose keys clash with another's. |
| **Command Name / Hotkey / Group table** | Every command, its current keys (several sequences are shown separated by semicolons) and its group (File, Edit, Walls ...). Dimmed names are commands not built yet; hover for "Not built yet; the key is kept for later". A clashing row ends with "(conflict)" in amber. Click a row to select the command. |
| **Conflicts (n)** | An amber list under the table of every sequence that two or more commands share, as `sequence: command, command`. It is open while there are conflicts. Reassign (below) or Remove a key to clear one; a conflict is not a refusal, so a map that already has them (an old `hotkeys.json`, or an import) still loads. |
| **Chief bindings with no action in Plan Studio yet** | A collapsed list of Daniel's named bindings that have no command here. A line above the table counts Daniel's bindings: how many are named, how many have a Plan Studio command and how many work today. |
| **Assign a sequence of up to 4 hotkeys to <command>** | Click the field (it says "Click here, then press keys"; it shows "Press keys..."), then press the keys one after another, up to four chords. `Esc` stops recording. |
| **Clear** | Empties the recorded sequence. |
| **Assign** | Adds the recorded sequence to the selected command. |
| **Already used by: ...** and **Reassign** | **While you record**, the dialog warns as soon as the sequence is the same as, a prefix of, or begins with another command's sequence, and names the commands. Assign then changes nothing; **Reassign** takes the sequence from them. |
| **Current hotkeys** and **Remove** | The selected command's sequences; pick one and press Remove. |
| **Reset Hotkeys** | Returns to the base table plus Daniel's keys, dropping all your edits. |
| **Import Chief Hotkeys...** | Reads a Chief `UserHotkeys.xml` over the keys above: every command the file binds that Plan Studio has gets the file's keys (its current keys are replaced, and any other command holding a key loses it); commands the file does not bind keep what they have. Plan Studio's own extra keys on an imported command, such as the number keys, are replaced too; Reset Hotkeys brings them back. The names are recovered from Chief's command catalog because the file stores ids only. A line reports how many bindings, commands and unmapped keys the file had. |
| **Export...** | Saves the keys. **JSON** (`hotkeys.json`) is the file of differences described below; **CSV** is the whole list as `Group,Command,Hotkeys` for a spreadsheet. The extension you type picks the format. |
| **Print List** | Makes a two-column PDF (US Letter, "Plan Studio hotkeys" with the number of commands with keys, in groups) of every assigned key in a temporary file and opens it in your system viewer, where you print it. |
| **Help**, **Cancel**, **OK** | Help opens this chapter in the Help viewer (1.6a). Cancel discards the edits. OK applies them and writes `~/.plan-studio/hotkeys.json`. |

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
command names and unreadable sequences are skipped. The settings folder is found from `HOME`, else `USERPROFILE`, else `HOMEDRIVE` plus `HOMEPATH`.

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
| Plan Check | 18 |
| 3D views | 10 |
| Framing tools and Build Framing | 11.11 |
| Layout view, schedules, Project Information | 11.2 to 11.4 |
| Slabs, pads, piers, platform holes | 16 |
| Trim, material regions, decks, 3D solids | 17 |

## 13.9 Keys inside a tool

These go to the active tool, not the hotkey map.

| Key | Where | Does |
|---|---|---|
| `Esc` | Any tool | Cancels the operation in progress; otherwise returns to Select Objects. Clears a pending hotkey prefix. In a 3D view, returns to the plan. |
| `Enter` | Select, polyline and text tools | Opens the specification of the selection; finishes a polyline, text or typed value. |
| `Tab` | Select Objects | Cycles objects under the pointer. |
| `Tab` | Cabinet, Stairs, Electrical tools | Next cabinet kind; flips the turn of an L, U, winder or curved stair; flips a wall device to the other side of the wall. |
| `Shift+Tab` | Cabinet tool | Next **library type**: Vanity, Pantry, Tall Oven, Refrigerator, then back to the plain kinds (chapter 6.2). |
| `Tab` | 3D view | Next camera. |
| `Delete`, `Backspace` | Select and most tools | Delete the selection (Terrain: the element under the pointer). |
| `Alt` | Wall tool | Suspends every snap (object, angle and grid). |
| `Shift` | Wall tool | Holds the Angle Snap increment even where angle snaps are off (chapter 2.2). |
| Digits, `Tab`, `Enter`, `Backspace`, `Esc` | Wall tool after the first click; Select Objects while dragging a wall end | Type a length (`12'6`), `Tab` to the angle, `Enter` to place it; `Backspace` edits, `Esc` drops the typed text (chapter 2.2). The digit hotkeys step aside while this is active. |
| Digits, `Enter`, `Esc` | Select Objects while dragging a door or window jamb handle or move handle | A typed width, or the gap to the nearer wall end or neighbor (chapter 3.2). `Alt` skips the 1" snap. |
| `Esc`, right click | Paste (copy hanging on the pointer) | Cancel the paste. A click drops it (chapter 2.5). |
| `Enter`, `Cmd+D`, `Esc` | **Unsaved Changes** prompt (New, Open, Close Plan, Quit; chapter 12.2a) | `Enter` Save, `Cmd+D` (`Ctrl+D` off macOS) Don't Save, `Esc` Cancel. While any prompt is open the window takes no hotkeys, so `Cmd+D` here is not Duplicate. |
| `Enter`, `Esc` | **Revert to Saved** prompt | `Enter` Revert, `Esc` Cancel. There is no Don't Save key. |
| `Enter` | **Recover Unsaved Work** prompt (after a crash, or when an autosave is newer than the plan) | `Enter` Recover. Discard has no key, so a stray `Enter` cannot delete the copy. |
| `Shift` | Select, hotkey modifiers | Adds to the selection; `Shift`+drag pans in 3D. |
| Arrow keys | 3D Full Camera; Electrical free device | Walk; turn the device (`Shift` = 90 degrees). |
| Arrow keys, `Shift`+arrow keys | 3D view, Select Objects, an object selected (not the Full Camera) | Nudge the selection one snap unit (ten with `Shift`) along the plan axis nearest the arrow's direction on screen. One undo step, "Move Objects". |
| Alt-click | 3D overview and doll house views | Makes the clicked surface point the orbit centre (Round 13). |
| Double-click | 3D view | On an object, opens its specification; on empty space, frames the whole building again, keeping the viewing angle. |
| Drag a selected object | 3D view, Select Objects (cabinet, placed symbol, device, detail, stair) | Slides it along the floor with the plan's snapping; `Alt` suspends the snap, `Shift` holds the move to the object's own axis (or the one across it), `Esc` puts it back. One undo step, "Move Objects". |
| `Alt` + press | Select Objects | Starts a marquee even on top of an object (chapter 2.5). |
| `Ctrl` (`Cmd` on a Mac) held at the start of a drag | Select Objects; Edit Area | Copies the selection instead of moving it (chapter 2.5). |
| Digits, `Tab`, `Enter`, `Esc` | Select Objects while dragging a move or a rotate | Type the distance (and, after `Tab`, the angle) of a move, or the degrees of a turn (chapter 2.5). |
| `Shift` | Draw Line | Holds the line to 15-degree steps (chapter 5.4). |
| `W` `A` `S` `D`, `Page Up`, `Page Down` | 3D Full Camera | Walk; raise or lower the eye. |
| `Backspace` | Polyline, spline | Drops the last vertex. |
| `Enter`, double-click | Slab tools, platform holes, Truss Base | Closes the polygon and makes the object. |
| `Backspace`, `Delete` | Slab tools, Truss Base | Drops the last corner; with none, deletes the selected object. |
| `Shift` or `Cmd` + click | Framing tools | Picks a placed framing object (`Shift` on a picked one drops it). |
| `Cmd` + click or drag | Slab tools | Picks an object, or moves it. |
| `Shift`, `Alt` | Roof Return | `Shift` makes a half return, `Alt` a boxed one. |
| `Enter` | Cabinet tool, a temporary dimension being edited | Applies the typed value by moving or resizing the selected cabinet (chapter 6.2). |
| `G` | Cabinet tool | Generate Countertop: joins the countertops of touching base cabinets (chapter 6.2). |
| `Enter`, `Backspace`, `Esc` | Custom Countertop, Backsplash, Counter Hole; polygon details | Finish, drop the last corner, cancel (chapters 6.2, 17.2). |
| `Cmd` + click or drag | Details tools | Picks an existing detail, or moves it (chapter 17.2). |
| Arrow keys, `Shift`+arrow keys | Layout view | Nudge the selected layout box 1/16", or 1/4" with `Shift`. |
| `Delete`, `Backspace`, `Esc` | Layout view | Delete the selected box; `Esc` clears the selection or a placement. `Alt` while dragging a box turns off the 1/16" snap (chapter 11.3). |
| `Enter` | Walkthrough path, Fillet, Chamfer, Offset | Finishes the path; starts a typed radius, distances or offset distance (chapters 10.12, 5.4). |
| `Delete` | Add Lights | Removes the selected light (chapter 10.13). |
| `Enter`, `Backspace`, `Esc` | Stairs tool (Landing) | Finish the polygon landing, drop its last corner, cancel it (chapter 7.2). A double-click also finishes it, or alone places a 3' square. |
| `Delete`, `Backspace` | Stairs tool | Removes the selected stair, its stairwell walls and its stairwell hole (one undo step). |
| `Enter`, double-click, `Esc` | Terrain tools | `Enter` or a double-click ends a polyline and `Enter` closes a polygon; `Esc` cancels the drawing or the typed value; an empty typed field takes the default shown (chapter 9.6). |
| `Delete`, arrow keys | Terrain tools | Remove, or nudge, the terrain element under the pointer (chapter 9.6). |
| `Enter`, double-click, `Backspace`, `Delete`, `Esc` | Distribution Path and Region tools | Finish the path or region (2 points, or 3 for a region); drop the last point; `Esc` clears the points, then leaves the tool (chapter 6.7). |
| `Esc` | Create Image, Create Billboard Image | Forgets the chosen picture file, so the next click asks for another (chapter 6.7). |
| `Cmd+Z`, `Cmd+Y` | Layout view | Undo and redo through **one history shared by the plan and the layout** (chapter 11.3). |
