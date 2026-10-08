# Daniel's Chief Architect setup (X18, with X17 notes)

Inventory date: 2026-10-07. Read-only pass over the Chief data folders, preference files and the Chief app bundles. The only files copied are the active toolbar and hotkey files, listed in section 6. Values are quoted exactly as stored. Nothing here includes account, license, email or client-folder information.

## Where the settings actually live

The brief pointed at ~/Library/Preferences and ~/Library/Application Support. Those folders hold only window and panel geometry. The real settings are in these places:

- **Drawing and UI preferences:** `~/.config/Chief Architect Inc/Chief Architect Premier X18.ini`, plus `Chief Architect Premier X18 - Local Only.ini` for machine-specific settings.
- **Toolbars and hotkeys:** `~/Documents/Chief Architect Premier X18 Data/Toolbars/` and `.../Hotkeys/`. The active set is named in `~/Library/Application Support/Chief Architect Premier X18/Local Settings - Chief Architect Premier X18.json`.
- **Templates:** `~/Documents/Chief Architect Premier X18 Data/Templates/`.
- **Factory defaults for comparison:** `/Applications/Chief Architect Premier X18.app/Contents/MacOS/Toolbars/` and `.../Hotkeys/UserHotkeys.xml`. The factory hotkey file is the X17 internal build (27.0.3.71), so some hotkey differences may be X17-to-X18 changes rather than Daniel's choices.

Migration history: Chief migrated X17 to X18 on 2026-08-09 (`Migrate_Settings_Log.txt`). Most X18 files carry that date.

## Notable customizations (summary)

1. Custom color theme selected: `selected color theme = Smoke 2 -Daniel Allen Design` (15 themes defined).
2. 101 hotkey bindings differ from factory (86 new keys, 15 changed), and 7 factory keys were cleared.
3. 60 commands are bound to `Meta+Ctrl+Alt+...` chords (59 of them added over factory), a pattern that suggests an external macro tool. Not verified.
4. 26 commands use two- and three-key sequences (`D, H`, `S, L`, `E, O`, `M, M, U`, `A, W, A`), which looks like a deliberate mnemonic scheme.
5. Zoom In is bound to `-` (factory `Num++`), and Zoom Out's factory key `Num+-` was cleared.
6. Tool Search (ID 23912) was removed from the File Management toolbar in all four toolbar sets.
7. Default plan template is `x17 Working Template 2025-08-20.plan` (9.5 MB), and the default layout template is `18x24 PRESENTATION LAYOUT TEMPLATE.layout`.
8. Cad snap set is narrow: Perpendicular is off while End Points, Mid Points, Center, Quadrant, Intersections and Tangents are on. Bumping is on at 5.
9. Crosshairs are off. Crosshair, Cad snap extension and pattern-tile colors are pure blue (0, 0, 255).
10. Background is RGB (245, 241, 239) and the layout background is RGB (249, 248, 244), both warm off-white. Layout Edge line weight is 18, and Layout Pattern and Pattern Tile are 10.

## 1. Inventory

### 1a. Chief Architect Premier X18 Data (`~/Documents/Chief Architect Premier X18 Data/`)

| File or folder | Kind | Size | Modified | Readable? |
|---|---|---|---|---|
| Toolbars/Default Configuration.toolbar | Toolbar set (text) | 7,783 B | 2026-10-07 20:01 | Yes. Copied. |
| Toolbars/Extended Tool Configuration.toolbar | Toolbar set (text) | 7,427 B | 2026-10-07 20:01 | Yes. Copied. |
| Toolbars/Space Planning Configuration.toolbar | Toolbar set (text) | 7,459 B | 2026-10-07 20:01 | Yes. Copied. |
| Toolbars/Terrain Configuration.toolbar | Toolbar set (text) | 7,654 B | 2026-10-07 20:01 | Yes. Copied. |
| Toolbars/*.toolbar.bak (4 files) | Prior toolbar backups (text) | 7,427 to 7,718 B | 2026-08-09 17:09 | Yes. Not copied (backups). |
| Toolbars/*.png (4 files) | Toolbar icons (128x128) | 1.5 to 3.0 KB each | 2026-08-09 17:00 | No, image. Not copied. |
| Toolbars/ConfigurationButtons.dat | Binary | 169 B | 2026-10-07 20:01 | No, binary. Not copied. |
| Toolbars/libraryItems.tbdata (and .bak) | Binary | 8 B each | 2026-10-07 20:01 | No, binary. Not copied. |
| Hotkeys/UserHotkeys.xml | Hotkey map (XML) | 141,892 B | 2026-09-23 22:16 | Yes. Copied. |
| Hotkeys/UserHotkeys.xsd | Hotkey schema (XML) | 1,158 B | 2026-08-09 17:00 | Yes. Copied as the hotkey schema. |
| Hotkeys/UserHotkeys_MigrateBackup.xml | Migration copy of X17 hotkeys | 140,056 B | 2026-08-09 17:00 | Yes. Not copied. |
| Templates/ (27 entries) | Plan and layout templates | about 101 MB total | 2026-08-09 17:09 | Binary. Names only (section 2). |
| Scripts/Site Area Analysis - Plines/ | Third-party Ruby macro package (6 files) | 448 B to 226 KB | 2026-08-09 17:09 | Ruby and README readable. Not copied (not toolbar or hotkey). |
| Textures/ (149 files) | Material textures | 171 MB | 2026-08-09 | Not read. |
| Backdrops/ (3 images) | Backdrop images | 3.3 MB | 2026-08-09 | Not read. |
| Images/-My Images/ (2 JPG) | Images | 209 KB each | 2026-08-09 | Not read. |
| Database Libraries/ (User_Library.calib, Trash.calib, User_Library.calib_error) | Binary user library | 376 KB total | 2026-09-27 | Binary. Not read, not copied (library). |
| Archives/ (55 files) | Archived projects | 8.0 GB | 2026-10-07 | Not read. |
| Common Documents/, Backups/ | Empty | 0 | 2026-08-09 | n/a |
| Migrate_Settings_Log.txt | Migration log (text) | 5,809 B | 2026-08-09 17:09 | Yes, used for paths. Not copied. |
| Migrate_Settings_Manifest.json | Migration manifest | 28.7 MB | 2026-08-09 17:03 | Not read. |
| mmaster.mat | Binary master list | 639 B | 2026-10-07 20:01 | Binary. Not read. |

### 1b. Preferences and app state (X18)

| File | Kind | Size | Modified | Readable? |
|---|---|---|---|---|
| ~/.config/Chief Architect Inc/Chief Architect Premier X18.ini | Main preferences (INI) | 30,249 B | 2026-10-07 21:28 | Yes. Values in section 5. Not copied (contains personal-info keys). |
| ~/.config/Chief Architect Inc/Chief Architect Premier X18 - Local Only.ini | Machine preferences (INI) | 28,658 B | 2026-10-07 21:28 | Yes. Drawing-relevant keys in section 5. Not copied (contains client project folder paths). |
| ~/.config/Chief Architect Inc/Chief Architect Premier X18 - Dialog Sizes.ini | Dialog geometry (INI) | 9,123 B | 2026-10-07 22:30 | Yes, not summarized (window sizes only). Not copied. |
| ~/.config/Chief Architect Inc/Chief Architect Premier X18 - Authentication.ini | Sign-in state | 237 B | 2026-10-07 21:28 | Not opened (authentication data). Not copied. |
| ~/Library/Preferences/com.chiefarchitect.chief-architect-premier-x18.plist | macOS preferences | 1,486 B | 2026-10-07 19:56 | Yes. Only panel and window frames. No drawing settings. |
| ~/Library/Preferences/com.chiefarchitect.Chief Architect Premier X18 Help.plist | macOS preferences | 166 B | 2026-09-07 | Yes. Help window geometry only. |
| ~/Library/Application Support/Chief Architect Premier X18/Local Settings - Chief Architect Premier X18.json | Pointer file | 689 B | 2026-10-07 21:28 | Yes. Names the active toolbar set and hotkey file. |
| ~/Library/Application Support/Chief Architect Premier X18/Chief Library.json | Catalog index | 76,744 B | 2026-08-09 | Not read. |
| ~/Library/Application Support/Chief Architect Premier X18/User Project Tag List.json | Tag list | 23 B | 2026-08-09 | Yes. Empty. |
| ~/Library/Application Support/Chief Architect Premier X18/Message Log.txt, Rendering Log.txt | Logs | 473 KB, 46 KB | 2026-10-07 | Not read. |
| ~/Library/Application Support/Chief Architect Premier X18/Data/Managed Resources/ | Content cache (metadata.db 33 MB, 406 small files) | n/a | 2026-09-28 (Content folder) | Not read. |

### 1c. Chief Architect Premier X17 (comparison only)

| File or folder | Kind | Size | Modified | Readable? |
|---|---|---|---|---|
| ~/Documents/Chief Architect Premier X17 Data/Hotkeys/UserHotkeys.xml | Hotkey map (XML) | 138,587 B | 2025-11-06 | Yes. Compared to X18 and factory. Not copied (superseded, and the name would clash). |
| ~/Documents/Chief Architect Premier X17 Data/Toolbars/ (4 .toolbar, 4 .bak) | Toolbar sets (text) | 7.4 to 7.7 KB each | 2026-09-22 (active), 2025-08-25 (.bak) | Yes. Compared to X18. Not copied. |
| ~/.config/Chief Architect Inc/Chief Architect Premier X17.ini | Main preferences (INI) | 26,808 B | 2026-09-22 | Yes. Compared to X18. Not copied. |
| ~/.config/Chief Architect Inc/Chief Architect Premier X17 - Local Only.ini | Machine preferences (INI) | 28,178 B | 2026-09-22 | Yes. Not summarized. Not copied. |
| ~/Library/Preferences/com.chiefarchitect.chief-architect-premier-x17.plist | macOS preferences | 4,019 B | 2026-09-18 | Yes. Panel and table settings only. |
| ~/Documents/Chief Architect Premier X17 Data/Backups/ (3 zips) | Full backups | 1.5 GB total | 2025-08-20 to 2025-08-25 | Not read. |
| ~/Documents/Chief Architect Premier X17 Data/Templates/ (20 entries) | Templates | about 83 MB | 2025-08-25 | Names in section 2. |
| ~/Documents/Chief Architect Premier X17 Data/sheetSizes.sheet | Sheet size table | 1,525 B | 2026-08-06 | Not read. |
| ~/Documents/Chief Architect Premier X17 Data/Archives/ (535 files) | Archived projects | 40 GB | 2026-09-18 | Not read. |

## 2. Templates

All X18 templates were dated 2026-08-09 17:09 by the migration. Sizes are in bytes.

Plan templates in `Templates/` (binary, not parsed):

| Template | Size |
|---|---|
| x17 Working Template 2025-08-20.plan (default plan template) | 9,460,333 |
| x15 Working Template 2023-03-03.plan | 8,309,694 |
| Residential Template.plan | 6,364,339 |
| Interior Template.plan | 6,039,922 |
| Residential Template - Metric.plan (metric template file) | 5,452,028 |
| Commercial Template.plan | 5,447,696 |
| Interior Template - Metric.plan | 5,118,266 |

Layout templates (`.layout`):

| Template | Size |
|---|---|
| 18x24 PRESENTATION LAYOUT TEMPLATE.layout (default layout template) | 2,306,730 |
| A1.layout | 636,367 |
| A2.layout | 633,912 |
| A3.layout | 633,309 |
| Arch B.layout | 610,042 |
| Arch C.layout | 616,008 |
| Arch D.layout | 688,656 |
| ArchD 24x36 Layout Template.layout | 600,609 |
| ISO1A 594x841 Layout Template.layout (metric layout template) | 645,190 |
| Letter.layout | 579,528 |
| Tabloid Layout Template.layout | 590,665 |
| Tabloid Layout Template - Metric.layout | 635,991 |

Other: `TemplateTextures.zip` (46,214,684 B), and a `Layout Borders/` folder with Imperial and Metric subfolders.

Defaults named in the preferences file: Default Plan Template is `x17 Working Template 2025-08-20.plan`, Default Layout Template is `18x24 PRESENTATION LAYOUT TEMPLATE.layout`, Metric Template File is `Residential Template - Metric.plan`, and Metric Layout Template is `ISO1A 594x841 Layout Template.layout`.

## 3. Toolbars

The active set is **Default Configuration.toolbar** (named in Local Settings). Four sets exist: Default, Space Planning, Extended Tool and Terrain. Each file has a header and then one block per toolbar: the toolbar name, a line of flags, and the command IDs in button order. The command ID names come from a name table at the end of each file. Every ID resolves to a name. Two entries (20181 and 20183) were merged in the name table, so their names were set by hand.

#### 3a. Changes from the factory toolbar sets

| Set | Change |
|---|---|
| All four | Tool Search (ID 23912) removed from File Management. |
| Default, Space Planning, Terrain | Structural Member Reporting dialog (ID 586, "Structural Member Reporting...") removed from Materials List. The "Structural Member Reporting Control" button (23887) stays. |
| Default only | Paste Hold Position (ID 20199) added to the front of Annotate. |
| Default only | Floor Defaults... (ID 518) added to the front of Floor Up/Down. |

The factory header reads version 29 and the user files read version 30. That is a re-save, not a change.

#### 3b. Button order per toolbar

Buttons are listed in the order they appear, left to right. Rows that say "(no buttons)" are section headers with no buttons in the file.


#### Default Configuration

- **Active Default Set Control Toolbar**: Edit Active View; Save Active View; Save Active View As; Active Default Set Control; Display Options; Default Settings
- **Active Layer Set Control Toolbar**: Edit Active View; Save Active View; Save Active View As; Active Layer Set Control; Display Options; Default Settings
- **Active Saved Plans**: Edit Active View; Save Active View; Save Active View As; Saved Plan View Control; Display Options; Default Settings; Referenced Plans/Layouts
- **Annotate**: Paste Hold Position; Dimension Tools; Automatic Dimension Tools; Text Tools; Revision Cloud; Point Tools; Line Tools; Arc Tools; Circle Tools; Box Tools; Spline; Auto Detail; Current CAD Layer
- **Arc Creation Modes**: Arc About Center; Center/Radius/End Arc; Free Form Arc; Start/End/On Arc; Start/Tangent/End Arc
- **Architectural Features**: Select Objects; Straight Wall Tools; Railing and Deck Tools; Curved Wall Tools; Door Tools; Window Tools; Cabinet Tools; Electrical Tools; Stair Tools; Floor Tools; Roof Tools; Trim Tools; General Framing Tools; Floor/Ceiling Framing Tools; Roof Framing Tools; Slab Tools; 3D Solid Tools
- **Cameras and Navigation**: Orthographic View Tools; Camera View Tools; Move Camera With Mouse; Camera View Options; Walkthrough Tools; Rendering Techniques; Camera View Lighting Tools; Sun Angle; Material Painter; Material Eyedropper; Object Eyedropper; Delete Surface; Adjust Material Definition; Interactive Material Editor
- **Edit**: (no buttons)
- **Edit Modes**: Concentric [C or , or X1 Button]; Fillet [F]; Default [ALT+Z or ALT+/ or Left Button]; Resize [X or . or X2 Button]
- **File Management**: New Plan; Open Plan; New Project; Open Plan/Layout; Save; Save Entire Project; Print; Send to Layout; Undo; Redo; Preferences; Launch Help
- **Floor Up/Down**: Floor Defaults; Down One Floor; Change Floor/Reference; Up One Floor
- **Layout**: Select Objects; Edit Layout Lines; Insert Page Before; Insert Page After; Duplicate Page; Delete Page; Exchange With Previous Page; Exchange With Next Page; Add Layout Revision; Layout Revision Table; Layout Page Table; Update Layout Views; Text Tools; Revision Cloud; Point Tools; Line Tools; Arc Tools; Circle Tools; Box Tools; Spline; Page Down; Change Layout Page; Page Up
- **Materials List**: Save Active View; Structural Member Reporting Control; Materials List Management; Master List; Update from Master List; Update to Master List; Edit Active View; Generate a Report; Export Materials List
- **Page Management**: Edit Page Information; Display Options; Default Settings; Referenced Plans/Layouts
- **Ray Trace**: Export Picture (BMP,JPG,PNG); Print Image; Save Thumbnail Image; Adjust Image Properties; Adjust Effects; Pause Ray Trace; Start/Resume Ray Trace
- **Resources**: Library Browser; Project Browser; Active Layer Display Options
- **Selection Toggles**: Select Intersected Objects; Select Contained Objects; Select Objects By Center
- **Snap Toggles**: Object Snaps; Angle Snaps; Grid Snaps; Bumping/Pushing; Endpoint; Midpoint; Center; Quadrant; On Object; Points/Markers; Intersections; Tangent Extensions; Perpendicular Extensions; Orthogonal Extensions
- **Toggle Modes**: Reference Display; Crosshairs; Color; Line Weights; Drawing Sheet; Print Preview; Temporary Dimensions; Connect CAD Segments; Arc Centers and Ends; Coordinate System Indicator - Floating; Reference Grid; Grid Snaps; Object Snaps; Angle Snaps
- **Toolbar Configurations**: Default Configuration; Space Planning Configuration; Extended Tool Configuration
- **Touch Screen Quick Access**: Select Objects; Camera View Tools; Move Camera With Mouse; Rendering Techniques
- **Zoom**: Zoom; Zoom In; Zoom Out; Undo Zoom; Fill Window Selected Objects; Fill Window Building Only; Fill Window; Pan Window

#### Space Planning Configuration

- **Active Default Sets**: Edit Active View; Save Active View; Save Active View As; Active Default Set Control; Display Options; Default Settings
- **Active Layer Set Control Toolbar**: Edit Active View; Save Active View; Save Active View As; Active Layer Set Control; Display Options; Default Settings
- **Active Saved Plan Views**: Edit Active View; Save Active View; Save Active View As; Saved Plan View Control; Display Options; Default Settings; Referenced Plans/Layouts
- **Annotate**: Dimension Tools; Automatic Dimension Tools; Text Tools; Revision Cloud; Point Tools; Line Tools; Arc Tools; Circle Tools; Box Tools; Spline; Auto Detail; Current CAD Layer
- **Arc Creation Modes**: Arc About Center; Center/Radius/End Arc; Free Form Arc; Start/End/On Arc; Start/Tangent/End Arc
- **Cameras and Navigation**: Orthographic View Tools; Camera View Tools; Move Camera With Mouse; Camera View Options; Walkthrough Tools; Rendering Techniques; Camera View Lighting Tools; Sun Angle; Material Painter; Material Eyedropper; Object Eyedropper; Delete Surface; Adjust Material Definition; Interactive Material Editor
- **Edit**: (no buttons)
- **Edit Modes**: Concentric [C or , or X1 Button]; Fillet [F]; Default [ALT+Z or ALT+/ or Left Button]; Resize [X or . or X2 Button]
- **File Management**: New Plan; Open Plan; Save; Print; Send to Layout; Undo; Redo; Preferences; Launch Help
- **Floor Up/Down**: Down One Floor; Change Floor/Reference; Up One Floor
- **Layout**: Select Objects; Edit Layout Lines; Insert Page Before; Insert Page After; Duplicate Page; Delete Page; Exchange With Previous Page; Exchange With Next Page; Add Layout Revision; Layout Revision Table; Layout Page Table; Update Layout Views; Text Tools; Revision Cloud; Point Tools; Line Tools; Arc Tools; Circle Tools; Box Tools; Spline; Page Down; Change Layout Page; Page Up
- **Materials List**: Save Active View; Structural Member Reporting Control; Materials List Management; Master List; Update from Master List; Update to Master List; Edit Active View; Generate a Report; Export Materials List
- **Page Management**: Edit Page Information; Display Options; Default Settings; Referenced Plans/Layouts
- **Ray Trace**: Export Picture (BMP,JPG,PNG); Print Image; Save Thumbnail Image; Adjust Image Properties; Adjust Effects; Pause Ray Trace; Start/Resume Ray Trace
- **Resources**: Library Browser; Project Browser; Active Layer Display Options
- **Selection Toggles**: Select Intersected Objects; Select Contained Objects; Select Objects By Center
- **Snap Toggles**: Object Snaps; Angle Snaps; Grid Snaps; Bumping/Pushing; Endpoint; Midpoint; Center; Quadrant; On Object; Points/Markers; Intersections; Tangent Extensions; Perpendicular Extensions; Orthogonal Extensions
- **Space Planning Features**: Select Objects; Build House; Bathroom; Bedroom; Closet; Deck; Dining Room; Entry; Family Room; Garage; Hallway; Kitchen; Laundry Room; Living Room; Office; Porch; Stairwell
- **Toggle Modes**: Reference Display; Crosshairs; Color; Line Weights; Drawing Sheet; Print Preview; Temporary Dimensions; Connect CAD Segments; Arc Centers and Ends; Coordinate System Indicator - Floating; Reference Grid; Grid Snaps; Object Snaps; Angle Snaps
- **Toolbar Configurations**: Default Configuration; Space Planning Configuration; Extended Tool Configuration
- **Touch Screen Quick Access**: Select Objects; Camera View Tools; Move Camera With Mouse; Rendering Techniques
- **Zoom**: Zoom; Zoom In; Zoom Out; Undo Zoom; Fill Window Selected Objects; Fill Window Building Only; Fill Window; Pan Window

#### Extended Tool Configuration

- **Annotations & CAD**: Active Dimension Defaults Control; Dimension Tools; Automatic Dimension Tools; Text Tools; Revision Cloud; Point Tools; Line Tools; Arc Tools; Circle Tools; Box Tools; Spline; Plan Footprint; Auto Detail; CAD Detail Management; CAD Detail From View; Current CAD Layer
- **Arc Creation Modes**: Arc About Center; Center/Radius/End Arc; Free Form Arc; Start/End/On Arc; Start/Tangent/End Arc
- **Architectural Tools**: Select Objects; Straight Wall Tools; Railing and Deck Tools; Curved Wall Tools; Door Tools; Window Tools; Cabinet Tools; Electrical Tools; Stair Tools; Floor Tools; Roof Tools; General Framing Tools; Floor/Ceiling Framing Tools; Roof Framing Tools; Trim Tools; Slab Tools; 3D Solid Tools
- **Camera Tools**: Orthographic View Tools; Camera View Tools; Move Camera With Mouse; Camera View Options; Walkthrough Tools; CPU Ray Trace; Rendering Techniques; Camera View Lighting Tools; Material Painter; Material Eyedropper; Object Eyedropper; Delete Surface; Adjust Material Definition; Interactive Material Editor
- **Edit**: (no buttons)
- **Edit Modes**: Concentric [C or , or X1 Button]; Fillet [F]; Default [ALT+Z or ALT+/ or Left Button]; Resize [X or . or X2 Button]
- **File Management**: New Plan; Open Plan; Save; Print; Send to Layout; Undo; Redo; Check Spelling; Preferences; Launch Help
- **Floor Up/Down**: Down One Floor; Change Floor/Reference; Up One Floor
- **Layout**: Select Objects; Edit Page Information; Edit Layout Lines; Insert Page Before; Insert Page After; Duplicate Page; Delete Page; Exchange With Previous Page; Exchange With Next Page; Add Layout Revision; Layout Revision Table; Layout Page Table; Update Layout Views; Referenced Plans/Layouts; Page Down; Change Layout Page; Page Up
- **Plan Setup**: Layer Set Management; Display Options; Active Layer Set Control; Active Default Set Control; Default Sets; Active Defaults; Default Settings
- **Ray Trace**: Save Thumbnail Image; Print Image; Export Picture (BMP,JPG,PNG); Adjust Image Properties; Adjust Effects; Pause Ray Trace; Start/Resume Ray Trace
- **Resources**: Library Browser; Project Browser; Active Layer Display Options
- **Saved Plan Views**: Edit Active View; Save Active View; Save Active View As; Saved Plan View Control; Referenced Plans/Layouts
- **Selection Toggles**: Select Intersected Objects; Select Contained Objects; Select Objects By Center
- **Snap Toggles**: Object Snaps; Angle Snaps; Grid Snaps; Bumping/Pushing; Endpoint; Midpoint; Center; Quadrant; On Object; Points/Markers; Intersections; Tangent Extensions; Perpendicular Extensions; Orthogonal Extensions
- **Terrain**: Terrain Tools; Terrain Elevation Tools; Terrain Modifier Tools; Terrain Feature Tools; Fencing Tools; Terrain Wall and Curb Tools; Plant Tools
- **Time Tracker**: Structural Member Reporting Control; Materials List Tools; Schedule Tools; Master List; Update from Master List; Edit Active View; Generate a Report; Update to Master List; View Time Log; Start Time Logging; Stop Time Logging
- **Toggle Modes**: Reference Display; Crosshairs; Color; Drawing Sheet; Line Weights; Print Preview; Temporary Dimensions; Connect CAD Segments; Arc Centers and Ends; Reference Grid
- **Touch Screen Quick Access**: Select Objects; Camera View Tools; Move Camera With Mouse; Rendering Techniques
- **Zoom**: Zoom; Zoom In; Zoom Out; Undo Zoom; Fill Window Selected Objects; Fill Window Building Only; Fill Window; Pan Window

#### Terrain Configuration

- **Active Default Set Control**: Edit Active View; Save Active View; Save Active View As; Active Default Set Control; Display Options; Default Settings
- **Active Layer Set Control Toolbar**: Edit Active View; Save Active View; Save Active View As; Active Layer Set Control; Display Options; Default Settings
- **Active Saved Plan Views**: Edit Active View; Save Active View; Save Active View As; Saved Plan View Control; Display Options; Default Settings; Referenced Plans/Layouts
- **Annotate**: Dimension Tools; Automatic Dimension Tools; Text Tools; Revision Cloud; Point Tools; Line Tools; Arc Tools; Circle Tools; Box Tools; Spline; Auto Detail; Current CAD Layer
- **Arc Creation Modes**: Arc About Center; Center/Radius/End Arc; Free Form Arc; Start/End/On Arc; Start/Tangent/End Arc
- **Cameras and Navigation**: Orthographic View Tools; Camera View Tools; Move Camera With Mouse; Camera View Options; Walkthrough Tools; Rendering Techniques; Camera View Lighting Tools; Sun Angle; Material Painter; Material Eyedropper; Object Eyedropper; Delete Surface; Adjust Material Definition; Interactive Material Editor
- **Edit**: (no buttons)
- **Edit Modes**: Concentric [C or , or X1 Button]; Fillet [F]; Default [ALT+Z or ALT+/ or Left Button]; Resize [X or . or X2 Button]
- **File Management**: New Plan; Open Plan; Save; Print; Send to Layout; Undo; Redo; Preferences; Launch Help
- **Floor Up/Down**: Down One Floor; Change Floor/Reference; Up One Floor
- **Layout**: Select Objects; Edit Layout Lines; Insert Page Before; Insert Page After; Duplicate Page; Delete Page; Exchange With Previous Page; Exchange With Next Page; Add Layout Revision; Layout Revision Table; Layout Page Table; Update Layout Views; Text Tools; Revision Cloud; Point Tools; Line Tools; Arc Tools; Circle Tools; Box Tools; Spline; Page Down; Change Layout Page; Page Up
- **Materials List**: Save Active View; Structural Member Reporting Control; Materials List Management; Master List; Update from Master List; Update to Master List; Edit Active View; Generate a Report; Export Materials List
- **Page Management**: Edit Page Information; Display Options; Default Settings; Referenced Plans/Layouts
- **Ray Trace**: Export Picture (BMP,JPG,PNG); Print Image; Save Thumbnail Image; Adjust Image Properties; Adjust Effects; Pause Ray Trace; Start/Resume Ray Trace
- **Resources**: Library Browser; Project Browser; Active Layer Display Options
- **Selection Toggles**: Select Intersected Objects; Select Contained Objects; Select Objects By Center
- **Snap Toggles**: Object Snaps; Angle Snaps; Grid Snaps; Bumping/Pushing; Endpoint; Midpoint; Center; Quadrant; On Object; Points/Markers; Intersections; Tangent Extensions; Perpendicular Extensions; Orthogonal Extensions
- **Terrain Features**: Select Objects; Terrain Tools; Terrain Elevation Tools; Terrain Modifier Tools; Terrain Feature Tools; Garden Bed Tools; Grass Tools; Water Feature Tools; Railing and Deck Tools; Fencing Tools; Terrain Wall and Curb Tools; Road Tools; Driveway Tools; Sidewalk Tools; Stepping Stone Tools; Slab Tools; 3D Solid Tools; Plant Tools; Sprinkler Tools
- **Toggle Modes**: Reference Display; Crosshairs; Color; Line Weights; Drawing Sheet; Print Preview; Temporary Dimensions; Connect CAD Segments; Arc Centers and Ends; Coordinate System Indicator - Floating; Reference Grid; Grid Snaps; Object Snaps; Angle Snaps
- **Toolbar Configurations**: Default Configuration; Space Planning Configuration; Extended Tool Configuration
- **Touch Screen Quick Access**: Select Objects; Camera View Tools; Move Camera With Mouse; Rendering Techniques
- **Zoom**: Zoom; Zoom In; Zoom Out; Undo Zoom; Fill Window Selected Objects; Fill Window Building Only; Fill Window; Pan Window

## 4. Hotkeys

Source: `Hotkeys/UserHotkeys.xml` (product 28.1.1.6, file version 4122). The file has 2,284 command records. 208 of them carry a key. Keys are shown exactly as stored. In the file, `Ctrl`, `Meta`, `Alt` and `Shift` are the modifier tokens. The mapping of `Meta` and `Ctrl` to the Mac keys was not verified.

Command names come from the name table in the toolbar files. That covers 42 of the 208 bindings. The rest are shown by command ID and can be named later if needed.

### 4a. Changes from factory

Factory means the bundled `UserHotkeys.xml` from the X18 app. That file is the X17 internal build, so some of these differences may be X17-to-X18 changes rather than choices Daniel made.

| ID | Command | Factory key | Daniel's key |
|---|---|---|---|
| 112 | Send to Layout | `Ctrl+U` | `S, L` |
| 202 | (name not in toolbar table) | `L` | `W` |
| 240 | Down One Floor | `Shift+M` | `Meta+Z` |
| 241 | Up One Floor | `Shift+N` | `Meta+A` |
| 335 | (name not in toolbar table) | `Shift+E` | `D, H` |
| 360 | (name not in toolbar table) | `W` | `Shift+Q` |
| 380 | (name not in toolbar table) | `Ctrl+R` | `Meta+Ctrl+Alt+Shift+N` |
| 404 | Fill Window | `F6` | `Meta+F` |
| 505 | Object Snaps | `Shift+S` | `Shift+F11` |
| 590 | Zoom In | `Num++` | `-` |
| 663 | (name not in toolbar table) | `Ctrl+G` | `C, C` |
| 675 | (name not in toolbar table) | `Ctrl+J` | `M, A` |
| 683 | (name not in toolbar table) | `Ctrl+0` | `Ctrl+F` |
| 713 | (name not in toolbar table) | `H` | `Ctrl+U` |
| 20043 | Pan Window | `P` | `H` |


Cleared (factory key removed):

| ID | Command | Factory key |
|---|---|---|
| 203 | (name not in toolbar table) | `E` |
| 403 | Zoom Out | `Num+-` |
| 497 | (name not in toolbar table) | `D` |
| 20093 | Temporary Dimensions | `T` |
| 21000 | (name not in toolbar table) | `Ctrl+F` |
| 23754 | (name not in toolbar table) | `Shift+Q` |
| 23959 | (name not in toolbar table) | `Ctrl+Shift+N` |

### 4b. All bindings

Every command with a key. "custom" means the key differs from the factory file.

| ID | Command | Key(s) | vs. factory |
|---|---|---|---|
| 101 | New Plan | `Ctrl+N` | factory |
| 102 | Open Plan | `Ctrl+O` | factory |
| 105 | Save | `Ctrl+S` | factory |
| 106 | (name not in toolbar name table) | `Shift+F4` | factory |
| 109 | Print | `Ctrl+P` | factory |
| 112 | Send to Layout | `S, L` | custom |
| 115 | (name not in toolbar name table) | `Ctrl+W` | factory |
| 172 | (name not in toolbar name table) | `Meta+Ctrl+Alt+4` | custom |
| 202 | (name not in toolbar name table) | `W` | custom |
| 207 | (name not in toolbar name table) | `K` | factory |
| 218 | (name not in toolbar name table) | `Shift+P` | factory |
| 224 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+P` | custom |
| 228 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+U` | custom |
| 230 | (name not in toolbar name table) | `Q` | factory |
| 231 | (name not in toolbar name table) | `2` | factory |
| 232 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+O` | custom |
| 237 | (name not in toolbar name table) | `Ctrl+D` | factory |
| 240 | Down One Floor | `Meta+Z` | custom |
| 241 | Up One Floor | `Meta+A` | custom |
| 242 | (name not in toolbar name table) | `M, C` | custom |
| 263 | (name not in toolbar name table) | `Shift+F3` | factory |
| 265 | (name not in toolbar name table) | `P, P, M` | custom |
| 275 | (name not in toolbar name table) | `3` | factory |
| 311 | (name not in toolbar name table) | `Meta+Ctrl+Alt+L` | custom |
| 322 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+H` | custom |
| 329 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Z` | custom |
| 330 | (name not in toolbar name table) | `Shift+T` | factory |
| 331 | (name not in toolbar name table) | `Meta+Ctrl+Alt+X` | custom |
| 332 | (name not in toolbar name table) | `Ctrl+T` | factory |
| 333 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Y` | custom |
| 334 | (name not in toolbar name table) | `T` | custom |
| 335 | (name not in toolbar name table) | `D, H` | custom |
| 336 | (name not in toolbar name table) | `S, D` | custom |
| 337 | (name not in toolbar name table) | `D, P` | custom |
| 338 | (name not in toolbar name table) | `Meta+Ctrl+Alt+O` | custom |
| 339 | (name not in toolbar name table) | `G, D` | custom |
| 341 | (name not in toolbar name table) | `Shift+W` | factory |
| 342 | (name not in toolbar name table) | `Meta+Ctrl+Alt+S` | custom |
| 343 | (name not in toolbar name table) | `Meta+Ctrl+Alt+U` | custom |
| 344 | (name not in toolbar name table) | `Meta+Ctrl+Alt+T` | custom |
| 359 | Select Objects | `Space` | factory |
| 360 | (name not in toolbar name table) | `Shift+Q` | custom |
| 365 | (name not in toolbar name table) | `Ctrl+Q` | custom |
| 367 | (name not in toolbar name table) | `Ctrl+B` | factory |
| 380 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+N` | custom |
| 382 | (name not in toolbar name table) | `Ctrl+Shift+S` | factory |
| 383 | (name not in toolbar name table) | `Meta+Ctrl+Alt+R` | custom |
| 392 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+W` | custom |
| 393 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+G` | custom |
| 395 | (name not in toolbar name table) | `Shift+K` | factory |
| 400 | (name not in toolbar name table) | `F5` | factory |
| 401 | Zoom | `Shift+Z` | factory |
| 404 | Fill Window | `Meta+F` | custom |
| 406 | Display Options | ``` | factory |
| 409 | CAD Detail Management | `Shift+V` | factory |
| 410 | Reference Display | `F9` | factory |
| 424 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+S` | custom |
| 426 | (name not in toolbar name table) | `Ctrl+1` | factory |
| 427 | Color | `F8` | factory |
| 428 | (name not in toolbar name table) | `Meta+Ctrl+Alt+H` | custom |
| 431 | (name not in toolbar name table) | `Shift+J` | factory |
| 434 | (name not in toolbar name table) | `F7` | factory |
| 435 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+R` | custom |
| 436 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+Z` | custom |
| 438 | Library Browser | `Ctrl+L` | factory |
| 439 | Preferences | `~` | factory |
| 441 | (name not in toolbar name table) | `Meta+Ctrl+Alt+3` | custom |
| 447 | (name not in toolbar name table) | `Meta+Ctrl+Alt+5` | custom |
| 454 | (name not in toolbar name table) | `Meta+Ctrl+Alt+M` | custom |
| 456 | (name not in toolbar name table) | `D, I` | custom |
| 459 | (name not in toolbar name table) | `Meta+Ctrl+Alt+A` | custom |
| 460 | (name not in toolbar name table) | `Y` | factory |
| 461 | (name not in toolbar name table) | `Meta+Ctrl+Alt+K` | custom |
| 462 | (name not in toolbar name table) | `E, O` | custom |
| 463 | (name not in toolbar name table) | `Meta+Ctrl+Alt+7` | custom |
| 464 | (name not in toolbar name table) | `E, L` | custom |
| 465 | (name not in toolbar name table) | `E, S` | custom |
| 468 | (name not in toolbar name table) | `E, C` | custom |
| 471 | (name not in toolbar name table) | `V` | factory |
| 472 | (name not in toolbar name table) | `I` | factory |
| 473 | (name not in toolbar name table) | `O` | factory |
| 479 | (name not in toolbar name table) | `F12` | factory |
| 490 | (name not in toolbar name table) | `F` | factory |
| 491 | (name not in toolbar name table) | `B` | factory |
| 492 | (name not in toolbar name table) | `L` | factory |
| 493 | (name not in toolbar name table) | `R` | factory |
| 496 | (name not in toolbar name table) | `U` | factory |
| 499 | Connect CAD Segments | `Shift+F8` | factory |
| 503 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+B` | custom |
| 505 | Object Snaps | `Shift+F11` | custom |
| 511 | (name not in toolbar name table) | `Ctrl+Shift+R` | factory |
| 516 | (name not in toolbar name table) | `Ctrl+Shift+T` | factory |
| 518 | Floor Defaults | `Ctrl+Shift+Y` | factory |
| 519 | (name not in toolbar name table) | `Ctrl+Shift+U` | factory |
| 520 | (name not in toolbar name table) | `Ctrl+Shift+I` | factory |
| 544 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+E` | custom |
| 545 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+F` | custom |
| 546 | (name not in toolbar name table) | `Shift+Y` | factory |
| 562 | (name not in toolbar name table) | `Meta+Ctrl+Alt+N` | custom |
| 569 | Master List | `Ctrl+M` | factory |
| 570 | (name not in toolbar name table) | `Shift+C` | factory |
| 573 | (name not in toolbar name table) | `Ctrl+Shift+P` | factory |
| 581 | (name not in toolbar name table) | `D, D` | custom |
| 582 | (name not in toolbar name table) | `C, S` | custom |
| 590 | Zoom In | `-` | custom |
| 642 | (name not in toolbar name table) | `E, A, O` | custom |
| 644 | (name not in toolbar name table) | `Ctrl+Space` | factory |
| 646 | (name not in toolbar name table) | `Ctrl+H` | factory |
| 648 | (name not in toolbar name table) | `Shift+A` | factory |
| 650 | (name not in toolbar name table) | `Tab` | factory |
| 651 | (name not in toolbar name table) | `Ctrl+E` | factory |
| 652 | (name not in toolbar name table) | `Ctrl+Alt+C` | factory |
| 654 | (name not in toolbar name table) | `Del` | factory |
| 655 | Undo | `Ctrl+Z` | factory |
| 659 | (name not in toolbar name table) | `D, E` | custom |
| 663 | (name not in toolbar name table) | `C, C` | custom |
| 667 | Redo | `Ctrl+Y` | factory |
| 668 | (name not in toolbar name table) | `Ctrl+F3` | factory |
| 675 | (name not in toolbar name table) | `M, A` | custom |
| 676 | (name not in toolbar name table) | `Ctrl+K` | factory |
| 680 | (name not in toolbar name table) | `Shift+X` | factory |
| 681 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+I` | custom |
| 682 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+J` | custom |
| 683 | (name not in toolbar name table) | `Ctrl+F` | custom |
| 684 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+K` | custom |
| 685 | Change Floor/Reference | `Ctrl+Shift+G` | factory |
| 691 | (name not in toolbar name table) | `Meta+Ctrl+Alt+F` | custom |
| 713 | (name not in toolbar name table) | `Ctrl+U` | custom |
| 714 | (name not in toolbar name table) | `Shift+G` | factory |
| 716 | (name not in toolbar name table) | `Shift+H` | factory |
| 717 | (name not in toolbar name table) | `Shift+F12` | factory |
| 723 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+T` | custom |
| 734 | (name not in toolbar name table) | `Meta+Ctrl+Alt+G` | custom |
| 738 | (name not in toolbar name table) | `Meta+Ctrl+Alt+V` | custom |
| 757 | (name not in toolbar name table) | `Meta+Ctrl+Alt+B` | custom |
| 765 | (name not in toolbar name table) | `Meta+Ctrl+Alt+D` | custom |
| 791 | (name not in toolbar name table) | `Meta+Ctrl+Alt+6` | custom |
| 820 | (name not in toolbar name table) | `Alt+O` | factory |
| 821 | (name not in toolbar name table) | `Alt+P` | factory |
| 822 | (name not in toolbar name table) | `Alt+D` | factory |
| 873 | CPU Ray Trace | `J` | factory |
| 880 | Grid Snaps | `Ctrl+F9` | factory |
| 4005 | (name not in toolbar name table) | `Shift+F6` | factory |
| 20015 | (name not in toolbar name table) | `D, T, M` | custom |
| 20040 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+X` | custom |
| 20043 | Pan Window | `H` | custom |
| 20047 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+L` | custom |
| 20048 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+M` | custom |
| 20051 | (name not in toolbar name table) | `D, W` | custom |
| 20056 | Page Down | `Shift+M` | factory |
| 20057 | Page Up | `Shift+N` | factory |
| 20064 | Drawing Sheet | `Alt+F3` | factory |
| 20092 | (name not in toolbar name table) | `Alt+A` | factory |
| 20099 | Tangent Extensions | `[` | factory |
| 20100 | Perpendicular Extensions | `]` | factory |
| 20101 | Orthogonal Extensions | `\` | factory |
| 20105 | Print Preview | `Alt+F2` | factory |
| 20108 | Reference Grid | `Shift+F9` | factory |
| 20114 | Concentric [C or , or X1 Button] | `X, C` | custom |
| 20120 | Bumping/Pushing | `F11` | custom |
| 20142 | (name not in toolbar name table) | `Shift+Esc` | factory |
| 20150 | (name not in toolbar name table) | `Ctrl+Alt+S` | factory |
| 20151 | (name not in toolbar name table) | `Esc` | factory |
| 20155 | (name not in toolbar name table) | `Meta+Ctrl+Alt+0` | custom |
| 20156 | (name not in toolbar name table) | `Meta+Ctrl+Alt+2` | custom |
| 20157 | (name not in toolbar name table) | `Meta+Ctrl+Alt+1` | custom |
| 20168 | (name not in toolbar name table) | `Alt+Q` | factory |
| 20185 | (name not in toolbar name table) | `Alt+T` | factory |
| 20187 | (name not in toolbar name table) | `Alt+Shift+O` | factory |
| 20188 | (name not in toolbar name table) | `Alt+Shift+T` | factory |
| 20189 | (name not in toolbar name table) | `Alt+Shift+D` | factory |
| 20190 | (name not in toolbar name table) | `Alt+Shift+P` | factory |
| 20194 | (name not in toolbar name table) | `Ctrl+X` | factory |
| 20195 | (name not in toolbar name table) | `Ctrl+C` | factory |
| 20196 | (name not in toolbar name table) | `Ctrl+V` | factory |
| 20199 | Paste Hold Position | `Ctrl+Alt+V` | factory |
| 20200 | (name not in toolbar name table) | `Alt+Shift+V` | factory |
| 20223 | (name not in toolbar name table) | `1` | factory |
| 20229 | (name not in toolbar name table) | `Alt+L` | factory |
| 20237 | (name not in toolbar name table) | `Meta+Ctrl+Alt+J` | custom |
| 20249 | Revision Cloud | `Meta+Ctrl+Alt+Shift+!` | custom |
| 20305 | (name not in toolbar name table) | `Meta+Ctrl+Alt+I` | custom |
| 20315 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+C` | custom |
| 20316 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+D` | custom |
| 20317 | (name not in toolbar name table) | `Meta+Ctrl+Alt+W` | custom |
| 20345 | (name not in toolbar name table) | `Meta+Ctrl+Alt+P` | custom |
| 20347 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Q` | custom |
| 20357 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+Y` | custom |
| 22996 | (name not in toolbar name table) | `Meta+Ctrl+Alt+C` | custom |
| 23413 | (name not in toolbar name table) | `Ctrl+R` | custom |
| 23428 | (name not in toolbar name table) | `Ctrl+A` | factory |
| 23446 | (name not in toolbar name table) | `C, P, P` | custom |
| 23447 | (name not in toolbar name table) | `E, M` | custom |
| 23452 | (name not in toolbar name table) | `M, M, U` | custom |
| 23457 | (name not in toolbar name table) | `A, W, A` | custom |
| 23458 | (name not in toolbar name table) | `A, W, B` | custom |
| 23466 | Object Eyedropper | `Shift+B` | factory |
| 23467 | (name not in toolbar name table) | `Shift+L` | factory |
| 23468 | (name not in toolbar name table) | `Shift+R` | factory |
| 23469 | (name not in toolbar name table) | `Shift+U` | factory |
| 23470 | (name not in toolbar name table) | `Shift+D` | factory |
| 23471 | (name not in toolbar name table) | `Shift+I` | factory |
| 23472 | (name not in toolbar name table) | `Shift+O` | factory |
| 23625 | (name not in toolbar name table) | `Meta+Ctrl+Alt+E` | custom |
| 23713 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+A` | custom |
| 23732 | (name not in toolbar name table) | `Meta+Ctrl+Alt+Shift+V` | custom |
| 23841 | Open Plan/Layout | `Ctrl+O` | custom |
| 23958 | New Project | `Ctrl+N` | factory |

## 5. Preferences that affect drawing behavior

Source: `~/.config/Chief Architect Inc/Chief Architect Premier X18.ini` under `[%General]` unless a row says otherwise. Local-only rows come from `Chief Architect Premier X18 - Local Only.ini`. Colors are written as (R, G, B) with alpha where the file gives one.

### 5a. Units, templates and dimensions

| Setting | Value | Note |
|---|---|---|
| Units | `0` | Likely U.S. units, since the default plan template is not the metric one. The file does not say this directly. Not confirmed. |
| Hide Non-preferred Unit Templates | `true` | |
| Default Plan Template | `x17 Working Template 2025-08-20.plan` | Stored as a file name, not a full path. |
| Default Layout Template | `18x24 PRESENTATION LAYOUT TEMPLATE.layout` | |
| Metric Template File | `Residential Template - Metric.plan` | |
| Metric Layout Template | `ISO1A 594x841 Layout Template.layout` | |
| Chamfer Distance (U.S. Units) / (Metric) | `12` / `25` | |
| Fillet Radius (U.S Units) / (Metric) | `12` / `25` | |
| Offset From Draw Surface (U.S Units) | `3` | |
| Concentric Jump Distance (U.S. Units) | `0` | |
| Dimension Rotation | `1` | |
| Dimension Separation Snaps | `true` | |
| Show Temporary Dimensions | `true` | |
| Highlight Overridden Dimension Text | `false` | |
| Minimum Display Size for Dimensions / for Labels | `0` / `0` | |
| DXF DWG Export: Imperial Scaling Unit / Metric Scaling Unit | `in` / `mm` | |
| DXF DWG Export: Create Associative Dimensions | `true` | |
| DXF DWG Import: Imperial Units / Metric Units | `false` / `mm` | |
| DXF DWG Import: Map To One Layer / Map To This Layer | `true` / `"CAD, WALL DETAIL LAYER"` | |

### 5b. Snapping and bumping

| Setting | Value |
|---|---|
| Object Snapping Enabled | `true` |
| Cad Snap Dist | `5` |
| Cad Snap History | `2` |
| Cad Snap Indicator Size | `6` |
| Cad Snap to Center, End Points, Intersections, Mid Points | all `true` |
| Cad Snap to Orthogonal, Points, Quadrant, Tangents | all `true` |
| Cad Snap to Perpendicular | `false` |
| Cad New Line Polar | `true` |
| Walls Only Snap On Allowed Angles | `true` |
| Restricted Allowed Angles Per Quadrant | `1` |
| Display Angle Snap Grid | `false` |
| Snap Objects On Paste | `true` |
| Snap To Cad Point On Layout Send | `false` |
| Bumping On / Bump Distance / Type in Bumping On | `true` / `5` / `true` |
| Edit Handle Clickable Tolerance / Select Handle Size | `1` / `5` |
| Edit Defaults On Double Click / Edit Active Default On Double Click | `true` / `true` |
| Wall Edit Mode | `true` |
| Select Room Before Wall | `false` |
| Multiple Copy intervals (key "0 0 0") / ncopies columns | `36` / `2` (X17 had `30` and `4`) |

### 5c. Crosshair, grid and colors

| Setting | Value |
|---|---|
| Cross Hair On | `false` |
| Cross Hair Size / Width / Aperture Size | `100` / `1` / `0` |
| Synchronize Cross Hair and Cursor | `false` |
| CrossHairs in perspective views | `true` |
| Cross Hair Color | (0, 0, 255), alpha 204 |
| Snap Grid Color | (227, 227, 227) |
| Reference Grid Color | (221, 221, 255) |
| Coordinate System Grid Color | (221, 221, 255), alpha 200 |
| Angle Snap Grid Color | (224, 224, 224) |
| Cad Snap Extension Color | (0, 0, 255) |
| Cad Snap Indicator Color | (207, 52, 35) |
| Selected Edge Handle Fill Color | (207, 52, 35), alpha 204 |
| Handle Fill Color | (255, 126, 121) |
| Secondary Handle Fill Color | (0, 249, 0) |
| Moving Color | (4, 51, 255) |
| Pattern Tile Color | (0, 0, 255) |
| Pattern Preview Color | (0, 0, 0), alpha 127 |
| Origin Indicator Color | (0, 0, 0) |
| Selection Highlight Color | (139, 130, 115), alpha 65 |
| Find Text Highlight Color | (255, 255, 0), alpha 140 |
| Background Color | (245, 241, 239) |
| Layout Background Color | (249, 248, 244) |
| Preview Background Color | (255, 255, 255) |
| Coordinate System Indicator X / Y / Z axis | (255, 0, 0) / (0, 0, 255) / (0, 255, 0) |
| Coordinate System Indicator Axis Line Size | `48` |
| 2D Origin / 2D from 3D Origin / 3D Origin Coordinate System Indicator On | `false` / `false` / `false` |
| Draw Origin Indicator in Foreground | `false` |
| Draw Sun\Moon Direction Indicator when Moving with Tools | `true` |
| Interface Colors (and macOS version) | `2` (X17 had `3`) |
| Selected color theme | `Smoke 2 -Daniel Allen Design` (15 themes defined) |
| Color Chooser | 24 custom swatches defined; Next Add Index `20` |

### 5d. Line weights and display

| Setting | Value |
|---|---|
| Thick Lines | `true` |
| Minimum Display Line Weight | `1` |
| Layout Edge Line Weight | `18` |
| Layout Pattern Line Weight | `10` |
| Pattern Tile Line Weight | `10` |
| Layout Use Pattern Attributes | `true` |
| Use Layout Line Scaling | `true` |
| Line Endcap Length | `0.0625` |
| Line Format | `0` |
| Line Number Height | `64` |
| Number of Leader Line Segments | `2` |
| Use Rich Text With Leader Line | `false` |
| Overlay Transparency | `20` |
| Show Floor / Show Layer | `true` / `true` |
| Color Off is Grayscale | `true` |
| Grayscale Images When Color Off in Plan and Vector Views | `false` |
| Blur Material Preview Backdrop | `true` |
| Use render view in library browser preview pane | `true` |

### 5e. Saving, undo and toolbars

| Setting | Value | Note |
|---|---|---|
| Autosave | `1` | On. No autosave interval key was found in these files. |
| Undo Levels | `99` | |
| Backup File Warning | `true` | |
| Backup Frequency / Max Backup Files | `2` / `14` | Local Only file. |
| Enabled Managed Resource Backups / Interval | `true` / `1` | Local Only file. |
| Lock All Toolbars | `true` | Local Only file. |
| Toolbar Button Size | `18` | Local Only file. |
| Drop Down Toolbars | `true` | |
| Default Print Range / Print in Color | `1` / `true` | Local Only file. |
| Active Toolbar Configuration | `Default Configuration.toolbar` | Local Settings JSON. |
| Hotkey file | `UserHotkeys.xml` | Local Settings JSON. |

### 5f. What changed from X17 to X18

- Default plan and layout template names are stored without the full path now.
- Interface Colors moved from 3 to 2.
- Multiple Copy changed from 4 columns at 30 to 2 columns at 36.
- Color Chooser custom swatches were re-set (most of the 24 slots changed).
- Auto-check and "set the user library to edit mode automatically" keys were dropped.

## 6. Files copied to `docs/chief-config-raw/`

Only toolbar and hotkey files, kept under their own names:

- `Default Configuration.toolbar` (7,783 B)
- `Extended Tool Configuration.toolbar` (7,427 B)
- `Space Planning Configuration.toolbar` (7,459 B)
- `Terrain Configuration.toolbar` (7,654 B)
- `UserHotkeys.xml` (141,892 B)
- `UserHotkeys.xsd` (1,158 B). This is the hotkey schema. It is not user data. Delete it if you want only the user's own edits.

Not copied: `.bak` files, toolbar icons, binary files (`ConfigurationButtons.dat`, `libraryItems.tbdata`), the X17 hotkey and toolbar files, plan and layout templates, library files, the INI and plist preference files, and the Authentication file.

## 7. Limits and open items

- Command names are known for 42 of 208 hotkey bindings. The other 166 are listed by ID.
- The Units value (`0`) and the Autosave interval were not confirmed.
- The factory hotkey baseline is the X17 internal build, so some X17-to-X18 differences may show up as Daniel's choices.
- The main INI was read with personal-information keys excluded. The Local Only INI was read for drawing settings only, because it holds client project folder paths. No license, serial or sign-in values were read or copied.
- `Scripts/Site Area Analysis - Plines/` is a third-party Ruby macro package (2017). It depends on the Ruby safe level (`Ruby Safe Level = 1` in the INI) and a load path set in Preferences. Its presence is noted, but its settings were not extracted.
- Not examined: `Chief Library.json`, `Managed Resources`, `Archives`, the migration manifest and the backup zips.
