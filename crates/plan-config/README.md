# plan-config

Reads Chief Architect's user configuration into Plan Studio data. No XML crate,
no INI crate; the only dependencies are `serde` and `serde_json` (for the JSON
output). Daniel's own files are embedded at compile time with `include_str!`
from `docs/chief-config-raw/`.

```rust
let cfg = plan_config::load_daniel_config();   // hotkeys (named), 4 toolbar sets, preferences
let bindings = plan_config::to_plan_studio_bindings(&cfg.hotkeys); // named bindings only
let json = plan_config::to_json(&cfg);
```

## How the app uses it

| Need | Call |
|---|---|
| Hotkey table for `toolbar::BINDINGS` and tooltips | `to_plan_studio_bindings(&cfg.hotkeys)` gives `PlanBinding { command_id, command_name, chord_text, keys }`. `KeyChord::symbols()` gives the tooltip form (`⌃⌥⌘6`), `Display` the Chief text (`Ctrl+Alt+Cmd+6`). Match `command_name` to Plan Studio's tool names; unbound/unknown ids are simply absent. |
| Toolbar layout | `cfg.toolbars[0]` is the Default set. `set.build_toolbar()` is the Build bar; `set.layout_order()` is dock, row, position order. Each flyout button carries its variants in `flyout`. |
| Preferences | `cfg.preferences` is Daniel's captured values. To read the live INI (`~/.config/Chief Architect Inc/Chief Architect Premier X18.ini`) the app reads the file itself and calls `preferences_from_ini(&parse_ini(&text))`. Keys the INI lacks keep Daniel's values. This crate never touches the disk. |
| Other hotkey/toolbar files | `parse_hotkeys_xml`, `resolve_names`, `parse_toolbar`, `parse_toolbar_named`. |
| Persistence | `to_json` / `from_json` for `DanielConfig`, `Vec<PlanBinding>`, or any type here. |

Regenerate `docs/chief-hotkeys-resolved.md` with
`cargo run -p plan-config --example gen_hotkeys_md > docs/chief-hotkeys-resolved.md`
(a test fails if the file is stale).

## Formats discovered

### `UserHotkeys.xml`

```xml
<UserHotkeys xsi:noNamespaceSchemaLocation="UserHotkeys.xsd">
  <product><name/><productVersion/><fileVersion/></product>
  <command><id>335</id><keyCodes>D, H</keyCodes></command>
  <command><id>101</id><keyCodes/></command>   <!-- unbound -->
```

* 2,284 `<command>` records; only the 208 with non-empty `<keyCodes>` become
  bindings (`HotkeyFile::total_commands` keeps the 2,284).
* The file stores **ids only**. Despite what the inventory first assumed, no
  names are in it; names come from the sources below.
* `keyCodes` is one chord, or a sequence separated by `", "` (`D, H`,
  `D, T, M`). A chord is `+`-joined modifiers then the key: `Meta+Ctrl+Alt+Shift+Q`,
  `Shift+F4`, `-`, `Space`, `Esc`, `Del`, `` ` ``.
* **Qt swaps the Mac modifiers**: `Ctrl` is the Command key, `Meta` is the
  Control key. Checked against the captured defaults (Straight Railing is
  Command-Q and is stored as `Ctrl+Q`; Straight Interior Wall is
  Control-Option-Command-6 and is stored as `Meta+Ctrl+Alt+6`).
  `KeyChord` uses physical meaning: `ctrl` = Control, `meta` = Command, and the
  swap happens once, in `KeyChord::parse_file`. `to_file_string` swaps back.

### `.toolbar` ("Chief Toolbar File: 9.0")

Plain text, `\n` lines, but every block is one long line.

```text
Chief Toolbar File: 9.0
Chief Architect Premier<TAB>  30
000000ff00000000fd00...                 hex of a Qt QMainWindow::saveState() blob
<toolbar name><TAB>                     per toolbar, three lines:
   0    1    3   14                       view types it appears in (blank = contextual)
 359 20221 20218  796 ...                 command ids, left to right (blank = none)
...                                      (22 toolbars in the Default set; alphabetical)
<blank>
Buttons
 101 &New Plan 102 &Open Plan... 105 &Save ...2000 Default Configuration20016 Layout Page &Table...
```

* **Items** are bare command ids. They are the same id space as `UserHotkeys.xml`.
* **Flyouts are not in the file.** A button such as 20221 "Straight Wall Tools"
  is the whole flyout group; Chief remembers the active variant elsewhere. The
  crate fills `flyout` from `docs/chief-x18-subtools.md`.
* **Separators are not in the file**, so `separator_before` is always `false`.
* **`Buttons` table**: one line of `%4d <label>` records concatenated with no
  other delimiter (`... Options2000 Default Configuration20016 Layout ...`). Ids
  ascend, and a label ends at the first position that starts a record with a
  larger id. Labels carry `&` mnemonics and `...`; `clean_label` removes them.
  Every id used by a toolbar has a label in every file (tested).
* **Hex blob** = `QMainWindow::saveState()` (magic `0xFF`, version, a `0xFD`
  dock-widget section that is empty here, then a `0xFC` toolbar section):
  `i32 lineCount`, then per line `i32 dock, i32 count`, then per toolbar
  `QString name` (i32 byte length + UTF-16BE), `u8 flags`, `i32 pos`, `i32 size`
  (-1), and 8 reserved bytes. Dock `0`=left, `1`=right, `2`=top, `3`=bottom
  (Qt's internal order). `flags` bit 0 is "shown"; contextual toolbars (empty
  view list: Snap Toggles, Arc Creation Modes, ...) and view-specific ones
  (Layout, Ray Trace, Materials List) have it clear. Toolbars absent from the
  blob would be `Placement::Floating`; none are.
* The four sets are the Default, Extended Tool, Space Planning and Terrain
  configurations. The set name is not stored in the file;
  `parse_toolbar_named` takes it from the file name and `parse_toolbar` infers it
  from the Build toolbar's name (`Architectural Features`, `Architectural
  Tools`, `Space Planning Features`, `Terrain Features`).
* In the Default set the Build bar is `Architectural Features`, and its
  17 items match row 2 of `docs/chief-x18-toolbars.md` in order (a test checks
  the first ten against that document's tooltip names).

### Preferences INI

Qt-style `[section]` / `key=value`; the drawing settings are in `[%General]`.
`parse_ini` is case-insensitive on keys and tolerant of BOMs, CRLF, comments and
quoted values. `preferences_from_ini` reads: `selected color theme`,
`Background Color`, `Layout Background Color`, `Cross Hair Color` (any
`(R, G, B[, A])`-like text), `Cross Hair On`, `Autosave`, `Default Plan
Template`, `Default Layout Template`, `Cad Snap to End Points / Mid Points /
Center / Quadrant / Intersections / Tangents / Perpendicular / On Object /
Orthogonal`, `Bumping On`, `Bump Distance`, and every key ending in `Line
Weight`. The names come from `docs/daniel-chief-setup.md` section 5; the exact
INI spellings of colors and of `On Object` were not captured, so those readers
are deliberately lenient.

Assumptions in `daniel_x18()`: `on_object` and `extension` (the inventory does
not name them) are on, Chief's own default. `extension` maps to `Cad Snap to
Orthogonal`.

## Command-name recovery

`resolve_names(&mut HotkeyFile, &CommandCatalog)` fills `command_name` and
`name_source`, strongest source first:

1. `Xml`: a name in the file (none in Daniel's).
2. `ToolbarButton`: the id is in a `Buttons` table (42 of 208).
3. `KnownId`: `catalog::KNOWN_IDS`, a deliberately tiny table of ids evidenced
   in `docs/daniel-chief-setup.md` that are in no `Buttons` table. None of them
   are bound, so it adds nothing to the 208 today.
4. `MatchedDefaultHotkey`: the binding's chord equals the documented Chief
   default of exactly one command (pairs read from `chief-x18-subtools.md` and
   `chief-x18-menus.md` by `parse_hotkey_doc`; `chief-x18-toolbars.md` has no
   hotkeys). To avoid wrong guesses it is applied only when (a) no other binding
   in the file uses the same chord, (b) one command owns that default, and (c)
   the name is not already taken. Zoom In shows why the order matters: Daniel
   rebound it to `-`, Chief's default for Zoom Out, and the id says Zoom In.

This is an inference: if Daniel rebound a chord that happens to equal another
command's default, `MatchedDefaultHotkey` names would be wrong. Check
`name_source` before trusting one. Result for Daniel's file: **143 of 208
named** (42 + 101), 65 left as "unknown id".

## Layout

`src/xml.rs` (reader) · `chord.rs` · `hotkeys.rs` · `catalog.rs` (name sources
and the inventory-document reader) · `toolbar.rs` · `prefs.rs` · `daniel.rs`
(embedded files, JSON) · `tests.rs`. `examples/gen_hotkeys_md.rs` writes the
resolved-hotkeys document.
