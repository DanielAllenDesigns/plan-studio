# Chief X18 Reference Manual audit, part 1: Program Overview, File Management, Project Planning, Defaults and Preferences, Toolbars and Hotkeys, Window and View, Creating, Displaying, Positioning and Editing Objects (manual pages 11 to 318)

Written 2026-10-08 on branch wip/round-14-partial (HEAD 129890a, with the Round 15 builders still editing the tree). Daniel's purpose: find every Chief feature that is not installed yet. This is a docs-only audit: no Rust was edited and no cargo was run.

**Method.** Every section of the Reference Manual in this page range was read from the extracted text at `~/plan-studio-dev/chief-docs/` (read only; nothing of it is copied here, every row below is in our own words and only dialog, tool and field names are Chief's). For each feature the status was taken from `docs/chief-feature-coverage.md` (the Round 14 audit), then `docs/parity/*.md` (the live rows, including "Coverage audit additions" and the Round 15 edits), then a code search of `crates/`. Where the code shows a Round 15 builder has already landed something, the row says so and carries a "verify in Chief" note if the layout is ours.

**Status words.** Works; Partial (present with a stated gap); Missing; Differs (by design); Out-of-scope (Ruby, cloud, licensing, 3D Warehouse, VR/gamepad hardware, vendor services); In progress (Round 15) when a brief in `~/plan-studio-dev/briefs/r15` covers it and the code is not finished. Evidence "NO SPEC" means no parity id covered the feature before this audit; those rows are appended as new ids (see the end of this file and the "Manual audit additions (part 1)" headings in `docs/parity/*.md`).

**Page numbers** are the printed page numbers of the manual (the numbers in its table of contents).

Contents of this file: 1 Program Overview; 2 File Management; 3 Project Planning; 4 Preferences and Default Settings; 5 Toolbars and Hotkeys; 6 Window and View Tools; 7 Creating Objects; 8 Displaying Objects; 9 Positioning Objects; 10 Editing Objects; then "Dialog panels" (the panel-by-panel comparison), "Gaps to build" (ranked) and the counts.

---

## 1. Program Overview (pp. 11 to 34)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Dashboard | 11 | Start window opened at launch or from File: New and Open buttons, recent documents, getting-started links, announcements, account link, build number and key prefix | Missing | APP-6 (File > Dashboard not in menus.rs) |
| Dashboard | 12 | Recent documents on the Dashboard with Pin and Unpin per entry so a document stays on top | Missing | NO SPEC -> APP-67 (files.rs recents are a plain list, no pin) |
| Dashboard | 12 | Getting Started, Training Videos and Download Catalogs links, Announcements feed, My Account link | Out-of-scope | APP-46, APP-44 (vendor services) |
| Dashboard | 133 | Preferences for the Dashboard: show at startup or open a new plan, reopen when the last tab closes, close when a tab opens, show the project name in recent lists | Missing | NO SPEC -> APP-68 |
| Environment | 13 | Cartesian drafting space, plan origin 0,0, absolute angle measured from +X | Works | plan-core geometry; status bar X/Y (status.rs) |
| Environment | 13 | Interface toggles show a check on the button; menus and toolbars show on/off state | Works | toolbar.rs, menus.rs `live(.. checked ..)` |
| Input devices | 14 | Left button draws/selects; right button selects any object with any tool and opens the context menu; right button as Alternate behavior | Partial | S-8 context menu; right-drag pans in Plan Studio (S-8 note); Alternate as right-button hold not wired (behaviors.rs has Alternate as a mode only) |
| Input devices | 15 | Middle button pans and acts as Move behavior; middle-click closes a tab | Partial | middle-drag pans (C-36); middle-as-Move and middle-click-close-tab: NO SPEC -> APP-69 |
| Input devices | 15 | Back and Forward buttons of a five-button mouse switch on Concentric and Resize behaviors | Missing | NO SPEC -> APP-70 |
| Input devices | 15 | Wheel zoom about the pointer (about 10 percent a click) | Works | LAY-25, LAY-35 |
| Input devices | 15 | Trackpad pinch zoom, Option-drag pan, two-finger pan, touch screens (two-finger cancel) | Partial | pinch zoom works (plan-app README); touch cancel gestures: NO SPEC -> APP-71 |
| Input devices | 15 | Pinch Zoom Sensitivity preference | Missing | NO SPEC -> APP-72 (Preferences > General) |
| Input devices | 15 | 3Dconnexion 3D mouse, gamepad navigation | Out-of-scope | hardware (see Gamepad Settings dialog, section 5) |
| Crosshairs | 16 | Crosshair cursor toggle (View > Crosshairs) for plan, elevation and 3D | Works | LAY-20 `ViewFlag::Crosshairs` |
| Crosshairs | 145 | Crosshair look: size as percent of the window, aperture gap, width, color, sync with the cursor, separate plan and 3D enable | Missing | NO SPEC -> LAY-38 (Preferences > Edit has no crosshair block) |
| Pointer icons | 16 | Cursor badges for Angle Snaps off, Alternate, Move, Resize, Concentric, Fillet, Connect CAD Segments off, rotate/resize about current point | Partial | S-66: status bar names the behavior; no cursor badges: NO SPEC -> APP-73 |
| Pointer icons | 16 | Cursor badges for problems: broken callout link, broken layout view link, Auto Rebuild Terrain off, Rebuild Walls/Floors/Ceilings off, bad wall connection, irregular wall angle, reversed wall | Missing | NO SPEC -> APP-74 (Plan Check and Fix Wall Connections exist; no pointer warning) |
| Pointer icons | 148 | Preference to hide the informative pointer icons | Missing | NO SPEC -> APP-75 (Preferences > Behaviors, "Behavior Indicators") |
| View windows | 17 | View window kinds: plan, camera/overview, cross section/elevation, CAD detail, materials list, layout; scrollbars toggle | Partial | tabs per kind in plan_tabs.rs; scrollbar toggle missing (APP-37) |
| View windows | 17 | Tear a view out of the main window into a second program window with its own menus and toolbars | Missing | APP-66 (one plan per window) |
| Side windows | 17 | Child Tool Palette, Library Browser, Project Browser, Active Layer Display Options, Walkthrough Preview, Action History as dockable windows | Partial | docks.rs; Tool Palette missing (APP-36); others Works |
| Side windows | 18 | Undock, redock, tab together or tile side windows; Ctrl/Cmd stops docking while dragging; top/bottom docking options | Partial | docks.rs docks left/right only; free docking and tabbing: NO SPEC -> APP-76 |
| Menus | 18 | Full menu bar reaching nearly every tool; menu path convention | Works | menus.rs |
| Menus | 18 | Keyboard access to menus (Alt then underlined letters, Esc steps back) | Missing | NO SPEC -> APP-77 (egui menus are mouse-only; macOS menu keys differ) |
| Menus | 19 | Menu items show their toolbar icon (preference "Show Icons") and hotkey | Partial | hotkeys shown; icon-in-menu preference: NO SPEC -> APP-78 |
| Toolbars | 19 | Edit toolbar below the drawing area, content follows selection type and view | Works | S-38..S-45 |
| Context menus | 19 | Right-click menu on an object (same commands as its Edit toolbar), empty space, library/project browser line, dialog text field | Partial | S-8 object and empty space; browser and text-field menus: NO SPEC -> APP-79 |
| Context menus | 124 | Preference: contextual menus on/off; "click twice to display" so the first right-click only selects | Missing | NO SPEC -> APP-80 |
| Tool Search | 19 | Search box on the toolbar that finds any tool or command by name or description and runs it; hotkey to focus it | Missing | NO SPEC -> APP-81 (plan-config catalog only lists the id) |
| Dialogs | 20 | Dialogs with a panel tree on the left, Ctrl+Tab / Ctrl+F6 to step panels, arrow keys in the tree | Partial | tabbed dialogs; no Ctrl+Tab stepping: NO SPEC -> APP-82 |
| Dialogs | 128 | Open a dialog on the last panel visited (preference) | Missing | NO SPEC -> APP-83 |
| Dialogs | 20 | Number boxes accept one arithmetic operation (+ - * /) typed after the value, with or without a unit | Missing | NO SPEC -> APP-84 (units.rs `parse_length` has no operators) |
| Dialogs | 20 | Number format and region settings (decimal mark, thousands separator, date format, currency) follow the operating system | Partial | locale not read; units.rs formats only |
| Elevation references | 21 | Per-object height reference: Absolute, From Floor, From Finished Floor, From Terrain, From Ceiling, From Roof | Missing | dialogs/opening.rs shows a disabled "From Floor"; NO SPEC -> R-89 |
| Elevation references | 21 | Default drawing group of a ceiling-hung or roof-hung symbol is 29 Soffits, otherwise 31 Fixtures/Furniture | Partial | LAY-36 drawing groups (Round 15, `drawing_groups.rs`); the rule itself: NO SPEC -> R-90 |
| Dialog size | 21 | Dialogs remember size and position (always, per session, never); Reset Dialog Sizes | Partial | egui memory of windows; the three-way preference missing; reset exists (PR-8) |
| Spec dialogs | 21 | Double-click or Open Object opens the specification; Tools > Active View > Edit Active View; Open Row Object(s) from a schedule | Works | S-7; plan_views.rs; schedule_view.rs |
| Spec dialogs | 22 | Group-selected objects of one type share a specification dialog: mixed values show a half-checked box or "No Change"; typing N undoes a field; "Multiple Defaults" label | In progress (Round 14) | W-83 multi-wall Open Object; other kinds: NO SPEC -> APP-85 |
| Dialog previews | 22 | Preview pane that rotates with the mouse, zooms with the wheel, labels wall sides, follows the panel (3D or plan), has Standard / Vector View / Glass House / Plan View, Fill Window, Mouse Orbit, Color and Show Line Weights buttons | Partial | previews in many dialogs (wall, opening, stairs, cabinets); technique buttons and orbit: NO SPEC -> APP-86 |
| Dialog previews | 23 | Materials preview shapes (cube, sphere, teapot, face) with room or backdrop behind and blur | Partial | tools/materials/spec.rs preview; shapes and blur: NO SPEC -> APP-87 |
| Dialog previews | 23 | Draggable splitters between panel list, settings and preview | Missing | NO SPEC -> APP-88 |
| Dialog keys | 23 | Keyboard navigation inside dialogs (Tab order, space/+/- on boxes, arrows in radio columns and tables, Enter closes, Cmd+. on Mac) | Partial | egui defaults; Enter-to-close not uniform: NO SPEC -> APP-89 |
| Dialog keys | 24 | Mac Dictation of numbers and units | Differs | macOS system rows (APP-23) |
| Edit handles | 24 | Move square, resize squares, rotate triangle; fixed screen size; some hidden when zoomed out; Esc or two-button click cancels | Works | S-11, S-12, S-27, S-100 |
| Edit handles | 25 | Special handles: Same Line Type, Same Wall Type, Edit Wall Intersection, door swing/hinge, dimension extend, label move/rotate, schedule column resize | Partial | wall/door/dimension handles Works (S-18..S-24, S-108); Same Line Type diamond handle: CAD-16 verify; schedule column resize: Partial |
| Status bar | 25 | Status bar reports selection type or count, handle meaning, tool name/description/hotkey, drawing length and angle, library item, floor or page, layer, drawing group, window size, render status, samples per second, redraw time, pointer XYZ | Partial | S-6, S-98: X/Y, floor, layer set, selection text; no per-object layer and drawing group, window size, redraw time, Z, render samples |
| Status bar | 124 | Choose which status items show (Active Status, Floor/Page, Object Layer, Object Drawing Group, Coordinates, window size, render status, redraw time) | Missing | NO SPEC -> APP-90 |
| Message boxes | 26 | Warnings with "Remember my choice / Do not show again"; Reset Message Boxes; Send Report button | Partial | `dont_ask_again` list (PR-8); no error-report upload (out of scope) |
| Name prompt | 26 | Small name dialog for New, Copy and Rename in Saved Defaults, Default Sets, Layer Sets, schedules, Make a Copy | Works | per-dialog inline fields (layer_sets.rs, layout.rs) |
| Toasts | 27 | Bottom-right toast notifications with expand-to-message and "do not show again" | Partial | status-bar messages only: NO SPEC -> APP-91 |
| Preferences/defaults | 27 | Preferences global, defaults per file, dynamic defaults, template plans | Works | PR-1, DS-1..DS-21, APP-60 |
| Drawing a plan | 28 | Recommended setup order: floor, foundation, framing, wall type defaults first | Works | DS-9, DS-11, DS-12, DS-19 |
| Drawing a plan | 29 | Move objects by typing a dimension value on a selected dimension line | Works | S-59, DIM-30 |
| Drawing a plan | 29 | Plan view per floor, reference floors, overviews, framing overview, cross section/elevation views, layouts | Works | R-65, C-*, L-* |
| Client sharing | 31 | Chief Architect Viewer and 3D Viewer app for clients | Out-of-scope | vendor (glTF/PDF export instead, L-48, L-20) |
| Account | 32 | Sign in/out, remember credentials, Forgot Password | Out-of-scope | APP-44 |
| Help | 33 | Tool tips, status bar help, online help menu | Works | APP-45, tooltips.rs |
| About | 34 | About box with license, version, release date, More Information panel | Works | APP-50 (license parts n/a) |
| Trial | 34 | Trial software limits, Purchase and Activate Full Version | Out-of-scope | licensing |

---

## 2. File Management (pp. 35 to 77)

Plan Studio keeps one `.psplan` file per plan (JSON) with its layout inside the project, saves safely (temp file then rename), archives the version it replaces under `Archives/<plan>/`, autosaves, and recovers after a crash (`files.rs`). Chief's "Project Management" (a hidden resource store of hashed files organised as Projects) is a different design: Plan Studio files are visible files the user organises in Finder, so that block is "Differs" unless a feature has a plain-file equivalent worth building.

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Compatibility | 35 | Open plans and layouts from Chief 10 through X17 and Home Designer | Works | plan-chiefplan import (`Import > Chief Plan`, L-50); layouts import: Partial |
| Compatibility | 35 | Files saved in X18 cannot be read by older versions; older files archived automatically on first save | Differs | Plan Studio format; archive on save Works (APP-59) |
| Compatibility | 35 | Windows and Mac files interchangeable | Works | JSON plan format |
| File association | 35 | Associate .plan/.layout/.calib/.calibz with the program (Preferences button) | Missing | NO SPEC -> APP-92 (mac_open.rs handles open-with; no association button, and `.psplan` only) |
| Home Designer | 35 | Opening Chief plans in the lite product | Out-of-scope | other product |
| Project Management | 36 | Resource store with Projects holding plans, layouts, templates and imported files, hashed names, hidden folder | Differs | by design: plain files; DECISIONS note not found, add one |
| Project Management | 36 | Preference to turn Project Management on or off; scheduled project backups; Asset Manager enable | Missing | NO SPEC -> APP-93 (Differs for the store; backups below) |
| New Project | 36 | New Project dialog: name, units, include plan with template, include layout with template, folders | Missing | NO SPEC -> APP-94 (File > New Plan only; units from template) |
| New Project | 37 | Project Folders to file projects by category when creating | Missing | NO SPEC -> APP-95 |
| Make a Copy | 37 | Copy a project or a document inside its project ("- Copy"), with links kept | Partial | `Save a Copy` (APP-19); linked copies of layouts: NO SPEC -> APP-96 |
| Legacy folder | 38 | New Project from Legacy Folder: import a folder of plan, layout and images into a project | Missing | NO SPEC -> APP-97 |
| Add to project | 38 | Add New Plan / New Layout to a project with a template choice | Partial | New Plan, New Layout File (APP-1, L-1) without a project container |
| Copy with links | 38 | Copy a plan or layout with the files linked to it into another project or a new folder | Missing | NO SPEC -> APP-98 |
| Replace With | 38 | Replace the contents of a plan or layout with another document, warning about linked files | Missing | NO SPEC -> APP-99 |
| Import file | 39 | Import any related file (a contract) into a project; Import File(s) to Common Documents; Import Template(s) | Missing | NO SPEC -> APP-100 |
| Save | 39 | Save, Save Entire Project (all plans and layouts in the project), Save All Plans/Layouts from the browser | Partial | Save works (APP-11); a project holds one plan plus its layouts, saved together |
| Export / import project | 39 | Export a project, plan or layout with all assets as a single .caproj; Import Project | Missing | NO SPEC -> APP-101 (`Backup Entire Plan` zips the plan and pictures: APP-18; import/restore of such a zip: NO SPEC) |
| Unmanaged files | 40 | Open a loose plan, then import it into a new or existing project, or keep it unmanaged | Differs | all Plan Studio files are plain files |
| Delete | 40 | Delete a plan, layout or project from the store (permanent, no undo) | Differs | done in Finder |
| Select Project dialogs | 40 | One Select Project dialog serving copy, add, open, replace, reference-plan, folder, move | Differs | no store |
| Import to Project | 42 | Prompts when sending a view to layout, linking a reference plan, or linking a callout to an unmanaged file | Differs | layouts live inside the project |
| Additional Import Selection | 42 | Choose further plans/layouts of the same folder to bring in | Missing | NO SPEC -> APP-102 |
| Asset Management | 43 | Asset Management dialog: search by text, owner, location; import new, insert, view, view with, rename, move to project browser, replace with, export, delete, merge, merge all identical; tile or table view; columns | Missing | NO SPEC -> APP-103 (images are referenced by path; `referenced_files` finds them for backup) |
| Export Managed Resources | 45 | Export chosen assets to a folder | Missing | NO SPEC -> APP-104 |
| Traditional files | 47 | Folder organisation advice; 260-character path limit; default Save As folder | Partial | default folder: Folders page (PR-9) covers autosave/templates; Open/Save As default folder: NO SPEC -> APP-105 |
| Thumbnails | 47 | Auto thumbnail on first save (small/large); Save Thumbnail Image from any view | Missing | APP-14 |
| New plan or layout | 48 | New Plan / New Layout from the template named in Preferences; Untitled naming; layout named after the first plan sent | Partial | APP-1, APP-2, L-10 |
| Units | 48 | Units fixed per file at creation (US or metric), chosen by template, not changeable later | Differs | Plan Studio lets the plan's units change (project units, `UnitSystem`); fine, more flexible |
| Templates | 48 | Plan and layout templates hold defaults, layers, wall definitions, page setup; system defaults fallback | Works | templates.rs, plan_defaults.rs, APP-60 |
| Select Template | 49 | Prompt when the configured template is missing: Load Installed or Load Custom | Missing | NO SPEC -> APP-106 |
| Referenced files | 49 | Plans and layouts reference textures, images, plant images, backdrops, pictures, PDFs outside the file | Works | `files.rs referenced_files` |
| Missing Files dialog | 50 | Opens when a view uses files that cannot be found (also Tools > Checks > Missing Files): table of missing resource, usage, owner object, location, in-use flag, path | Missing | NO SPEC -> APP-107 (view3d_panel/textures.rs notes missing textures silently) |
| Missing Files | 50 | Resolve: Replace from Library, Replace Resource (browse), Search Online, Delete Object, Clear File Reference, More Information, Add Search Directory; "do not show again this session" | Missing | NO SPEC -> APP-108 |
| Referenced Plans/Layouts | 51 | List files present and not found; browse to relink; replace reference | Differs | layout lives inside the plan project (L-1); relinking only matters for Reference Display of another plan |
| Link consequences | 52 | Unlinked layout boxes show a caution and a relink cursor | Differs | same reason |
| Link View dialog | 52 | Choose an open plan view to link a callout, the Reference Display or a layout box; keep box contents position when relinking | Partial | layout box links by view name (Layout Box Specification, L-*); callout link to another plan: NO SPEC -> APP-109 |
| Project Browser | 53 | Side window showing saved views, details, schedules, materials lists of the plan and pages of the layout | Works | APP-35, docks.rs |
| Project Browser | 54 | Show in Project Browser selects the active view | Works | APP-15 |
| Project Browser | 55 | Filter by text with suggestions; sort by name, date modified, date created, size; ascending/descending | Missing | NO SPEC -> APP-110 (Project dock has no filter or sort) |
| Project Browser | 56 | Advanced Search dialog: text, project folder, modified/created before and after dates, size larger/smaller | Missing | NO SPEC -> APP-111 |
| Project Browser | 56 | Project Folders and Tags (nested folders or multi-tag filtering, colors, pin, Filter Unfiled) | Missing | NO SPEC -> APP-112 |
| Project Browser | 56 | Categories per plan: CAD Details, Cameras by floor, Cross Sections, Floor Levels, Materials Lists, Plan Views, Schedules, Wall Details | Partial | Project dock lists views, layouts, cameras, schedules (APP-35); floor levels, CAD details, wall details and materials lists: Partial |
| Project Browser | 57 | Layout categories: CAD Details, Page Template / Used / Blank page icons, Tables | Partial | layout pages listed; page-state icons: NO SPEC -> APP-113 |
| Project Browser | 57 | Pin projects and documents; Common Documents and Plan and Layout Templates headings | Differs | no store |
| Project Browser | 58 | Assets listed under a project and draggable into a view | Missing | NO SPEC -> APP-114 |
| Project Browser | 58 | Details panel: modified, created, size, location, notes with hyperlink for any item including views | Missing | NO SPEC -> APP-115 |
| Project Browser | 59 | Preview panel with thumbnail; double-click preview opens the view; Update Preview(s) | Missing | NO SPEC -> APP-116 |
| Project Browser | 59 | Browser toolbar: Settings (which panels show), Update Previews, Preferences, Refresh, Only Show Open Projects, Show Default Documents, Add New Project, Import Legacy Folder | Missing | NO SPEC -> APP-117 |
| Project Browser | 59 | Rearrange, tile, lock and hide title bars of browser panels; reset panel layout (Preferences > Project Browser) | Missing | NO SPEC -> APP-118 |
| Browser menu | 60 | New View Folder inside Cameras, Cross Sections, CAD Details; drag views into folders; Expand All, Collapse All, Select All | Missing | NO SPEC -> APP-119 |
| Browser menu | 61 | Rename, Send to Layout, Exchange with Next/Previous Page, Add to Library (schedule or layout table), Update Preview | Partial | Send to Layout and page exchange Works (L-*); rename: Partial; Add to Library of a schedule: NO SPEC -> APP-120 |
| Browser menu | 61 | Open View, Open, Open With, Show Page, Open Page View, Find in Plan, Close View, Close All Views | Partial | open/close Works; Find in Plan from a schedule or camera: NO SPEC -> APP-121 (see section 3, Finding objects) |
| Browser menu | 61 | Saved Plan Views and CAD Details created from the browser; Insert Page Before/After | Works | Plan Views menu (LAY-*), page insert (L-*) |
| Browser menu | 62 | Edit View, Open Object (schedule/table spec), Save View, Delete with link warnings | Partial | Edit via Plan View Specification (LAY-2); Open Object for a schedule Works (L-25); delete with link warning: NO SPEC -> APP-122 |
| Pinning | 62 | Pin projects, plans and layouts to the top of lists; pinned recent documents | Missing | NO SPEC -> APP-123 |
| Open | 68 | Open dialog behavior: last folder remembered; plan opens on the floor last active; layout opens on its last page | Partial | open dialog Works (APP-3); last floor/page restore: NO SPEC -> APP-124 |
| Opening | 68 | Open all plans/layouts of a project or folder at once | Differs | no projects |
| Recent documents | 69 | File > Open Recent Documents with Clear Recent Documents; max count; show in submenu or at the bottom of the menu; Clear Pinned | Partial | APP-5; count and placement options: NO SPEC -> APP-125 |
| Save | 70 | Save without renaming; Save from the browser menu | Works | APP-11 |
| Save As | 70 | Save under a new name or folder; only for unmanaged or traditional files | Works | APP-12 |
| Save As Template | 71 | Purge chosen data categories and save a copy as a template; set as default for US/metric; open a new plan after | Partial | APP-13 "Save Current Defaults as My Template"; whole-plan template with purge checklist: NO SPEC -> APP-126 |
| Save As Template | 112 | Save as Layout Template with page-data purge options (template pages untouched) | Partial | L-10 (Save As Template for layouts) |
| Revisions | 71 | Revision file naming advice | Differs | n/a (advice) |
| Save dialog | 72 | OS save dialog, type by extension | Works | rfd/native dialogs |
| Back up managed resources | 73 | Zip all projects, assets and user catalog; Restore Managed Resources; scheduled backups | Partial | `Backup Entire Plan` for one plan (APP-18); whole-library backup/restore and schedule: NO SPEC -> APP-127 |
| Backup advice | 73 | Do not save across networks or removable media | Differs | n/a |
| Auto Archive | 74 | Archive folder per plan; archive by Hour, Day or Previous Save; naming with date and hour | Partial | files.rs keeps the newest N timestamped copies; Hourly/Daily/Previous-save choice: NO SPEC -> APP-128 |
| Auto Archive | 75 | Legacy archive of an older-version file when first saved (suffix _v10) | Differs | n/a |
| Manage Archives | 75 | File > Manage Auto Archives opens the folder; warning when archive count passes the limit | Works | APP-17 `archives_window`; warning threshold: Partial |
| Auto Save | 75 | Timed Auto Save files, retained after normal close, `_auto_save_bak` after a crash, offered at next open; Auto Save frequency preference | Works | files.rs autosave and recovery (APP-65) |
| Premier Data folder | 76 | A data folder holding Archives, Backups, Backdrops, Hotkeys, Images, lex (dictionaries), Database Libraries, Scripts, Templates, Textures, Toolbars, master materials list, sheetSizes, migration backups | Partial | `~/.plan-studio/` holds settings, hotkeys, templates, recovery, library; no single user-data layout documented: NO SPEC -> APP-129 |
| Data folder | 77 | Data folder location chosen in Preferences; rebuilt from defaults if missing | Partial | Folders page (PR-9) picks library/textures/backdrops/templates/autosave; data-folder-wide move: NO SPEC -> APP-130 |
| Exit | 77 | Exit prompts to save; deletes autosave and undo files; prompts to back up projects | Works | files.rs on_exit |

---

## 3. Project Planning (pp. 78 to 100)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Time Tracker | 78 | Log time per plan or layout file: Start Time Logging, Stop Time Logging, View Time Log; automatic start on open | Missing | APP-29 |
| Time Tracker | 78 | Time Log dialog: entries sortable by column; Add (starts now), Delete, Export Selected to CSV or text with total duration, Print Selected, Select All / Clear All, total duration | Missing | NO SPEC -> APP-131 |
| Time Tracker | 79 | Selected entry fields: user name, start, end (not editable while running), duration, notes; multi-select edits only user and notes | Missing | NO SPEC -> APP-132 |
| Time Tracker | 142 | Preferences > Time Tracker: auto-start on open, stop when idle for N minutes, idle timeout dialog, default user name | Missing | NO SPEC -> APP-133 |
| Space Planning | 80 | Room Box tools, one per room type (bedroom, kitchen and so on): click to drop a default-size box or drag any size; repeats until another tool | Missing | R-79 covers the Assistant only; tools: NO SPEC -> R-91 |
| Space Planning | 80 | Boxes live on the "Space Planning Boxes" layer, labelled with that layer's text style; show the wall extents that Build House will use | Partial | `rooms_edit.rs` draws boxes and labels (plan_symbols); no layer, no wall-extent outline: NO SPEC -> R-92 |
| Space Planning | 80 | Room Boxes are 2D polylines: move, resize, reshape like closed polylines; no curved edges; overlap by creation order, overlapped edges hidden | Partial | boxes drag and bump (R-79); polyline handles and overlap order: NO SPEC -> R-93 |
| Space Planning | 80 | Room Boxes saved with the plan (they are plan objects) | Missing | boxes are session state in `rooms_edit.rs` `State`, not in the Project: NO SPEC -> R-94 (data loss risk) |
| Space Planning | 81 | Edit tools Remove Overlapped Areas and Overlap Adjacent Room Box | Missing | NO SPEC -> R-95 |
| Space Planning | 81 | Build House: boxes become walls (exterior, interior, railing, deck railing from the wall-type defaults), rooms and railings; overlaps ignored | Works | R-79; plan-spaceplan `build_house`; `build_tools.rs build_house_from_boxes` (uses built-in wall types, not the Default Settings wall types: Partial) |
| Space Planning | 81 | Space planning on floor 1 and up (not floor 0); align upper boxes using the Reference Display, snaps to Endpoint and On Object | Partial | Reference Display works (LAY-9); upper-floor boxes: NO SPEC -> R-96 |
| Space Planning | 82 | Space Planning Assistant questionnaire creates the boxes (including a two-story option) | Works | R-79; plan-spaceplan Questionnaire |
| Space Planning | 82 | Space Planning toolbar configuration | Partial | R-80 stub toggle |
| Room Box Spec | 82 | Room Box Specification dialog: General (room name, function), Line Style, Fill Style | Missing | NO SPEC -> R-97 |
| Construction Lines | 83 | Construction Line tool (CAD > Line > Construction Line): guide lines for alignment, snap targets, dimension targets, usable by Center Object, Make Parallel/Perpendicular, Trim and Extend | Missing | NO SPEC -> CAD-62 (neither the captures nor any parity row list it) |
| Construction Lines | 83 | Own "Construction Lines" layer, lockable, printed when the layer is on; placeable on custom layers; Drawing Group 21 by default | Missing | NO SPEC -> CAD-63 |
| Construction Lines | 84 | Callouts on either or both ends; infinite length (not part of view extents, callouts pinned to the view edge); display on all floors or only the drawn floor; show in elevations when parallel to the line of sight, vertical ones in plan | Missing | NO SPEC -> CAD-64 |
| Construction Lines | 84 | Construction Line Order Management dialog: rule sets by view type and angle, count format, reverse direction, priority, add/copy/delete; collinear lines share a number | Missing | NO SPEC -> CAD-65 |
| Construction Lines | 86 | Construction Line Specification: panel with infinite in plan/elevation, display on all floors, include in automatic ordering, Define Rules; Callouts panel (display both/start/end/none per view, label with above/below text, insert macros, Automatic, shape, fill color and transparency, size and angle, custom outline color/style/weight, By Layer); Line Style panel; Text Style panel | Missing | NO SPEC -> CAD-66 |
| Construction Lines | 265 | Edit toolbar "Set as Default" for construction lines and Convert Polyline to/from a Construction Line | Missing | NO SPEC -> CAD-67 |
| Reference Display | 89 | Show other floors (or another plan file) dimmed in plan, camera and elevation views; snapping to reference objects while the reference can't be selected | Works | LAY-9, LAY-10, S-111; reference_display.rs |
| Reference Display | 89 | Default reference: floor below, "Reference Display Layer Set"; additional reference layer sets usable singly or combined | Partial | one layer set per view (reference_display.rs); combining several: NO SPEC -> LAY-39 |
| Reference Display | 89 | Settings are per saved plan view and can be changed in layout boxes | Partial | saved plan views carry `reference_floor` (plan_views.rs); layout box override: NO SPEC -> LAY-40 |
| Reference Display | 89 | Reference Model in camera and elevation views: another plan file shown, with its own rendering technique (Standard, Vector, Glass House, Technical Illustration) | Missing | NO SPEC -> LAY-41 |
| Change Floor/Reference | 89 | Dialog: current floor list, Show Reference Floor(s) check, table of reference rows in draw order, plan file per row (open plans or "Choose Existing Plan"), floor per row (Automatic, a fixed floor, Match Current), layer set per row with Define, Details column to include fill patterns, XOR drawing | Partial | `reference_display.rs` has show toggle, floor below / above / fixed floor, layer set, color; multiple rows, other plan files, Details, XOR: NO SPEC -> LAY-42 |
| Change Floor/Reference | 91 | Insert Above/Below, Move Up/Down, Delete reference rows; offsets X/Y/Z and angle for another plan | Missing | NO SPEC -> LAY-43 |
| Reference doc offset | 91 | Edit Reference Document Offset: marquee with Move and Rotate handles to align another plan | Missing | NO SPEC -> LAY-44 |
| Swap Floor/Reference | 91 | Go to the reference floor, making it current and the old floor the reference (toolbar button or hotkey; not with several references) | Missing | NO SPEC -> LAY-45 |
| Finding objects | 92 | Find Objects on Layer(s) (right-click a layer in the layer panel) | Partial | layer panel "Select Objects" selects them (layer_display.rs); no Select Location dialog |
| Finding objects | 92 | Find Object in Plan: from a schedule or materials list row select the object(s) in plan, opening a view if needed; Find in Plan from a saved camera or schedule | Missing | NO SPEC -> S-118 |
| Finding objects | 93 | Find Schedule(s) from Object edit button; Select Location dialog listing views, defaults or schedules with counts | Missing | NO SPEC -> S-119 |
| Finding objects | 92 | Find Wall from a framing member in wall detail or framing overview; Find Trusses from truss detail | Missing | NO SPEC -> S-120 (framing views exist; the back-link does not) |
| Finding objects | 92 | Find/Replace Text; Match Properties to find similar objects | Works | TXT-12 (Partial for fonts), S-116 |
| Finding objects | 92 | Tools > Checks > Missing Files and Referenced Plan Files | Missing | see section 2 |
| Plan Check | 94 | Plan Check examines the current floor and steps through findings: error counter, description, Next, Previous, Hold, Done; highlights the object; dialog remembers its position | Works | APP-25..27; dialogs/plan_check.rs (Previous, Next, Zoom to, Ignore; no Hold because the window stays open) |
| Plan Check | 94 | Auto-assign room types on first run, flag rooms it cannot type | Partial | plan-check infers rooms from fixtures: verify; the first-run assignment write-back: NO SPEC -> APP-134 |
| Loan Calculator | 95 | Calculate monthly payment, loan amount, term or interest rate; required and optional fields (taxes, insurance, PMI, other fees); Reset; result button | Missing | APP-30 (marked out of scope; a few lines of arithmetic, useful for client talks) |
| Plan Databases | 96 | Plan Database: collection of plan files with search data (Create, Edit, Search for Plans); folder path, include subfolders, relative path | Missing | APP-28 (that row describes the wrong feature: it is a plan-library finder, not an object list) |
| Plan Databases | 97 | Edit Plan Database dialog: add/remove plans, thumbnail, style, price, area, bedrooms, baths, floors, path, relink, open, description | Missing | NO SPEC -> APP-135 |
| Plan Databases | 98 | Find Plan Assistant: house style, floors, bedrooms, baths, size, price range, matching list, preview | Missing | NO SPEC -> APP-136 (Differs for a studio doing custom work: low value) |

---

## 4. Preferences and Default Settings (pp. 101 to 157)

The field-by-field comparison of the Preferences panels and of General Plan Defaults is in "Dialog panels" below; this table lists the features and behaviors.

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Defaults vs prefs | 101 | Defaults are per file, preferences global; import defaults from another plan | Works | PR-2, DS-*; import of a Chief plan's defaults: APP-2 "Import Chief Template" |
| Default Settings dialog | 102 | Searchable tree of all defaults of the open file; Edit button or double-click opens a defaults dialog | Works | APP-21, APP-58, DS-1..DS-20 (29 groups, Round 15) |
| Default Settings dialog | 102 | Double-click a parent or child tool button to open that tool's defaults; double-click the Select Objects button for General Plan Defaults | Missing | NO SPEC -> DS-22 (the toolbar buttons do not open defaults on double-click) |
| Default Settings dialog | 102 | Defaults dialogs for tools without a button (3D View, General CAD) reachable from an optional toolbar button; hotkeys can open any defaults dialog | Partial | every leaf reachable from the tree; per-defaults hotkeys: NO SPEC -> DS-23 |
| Default Settings dialog | 102 | Select several similar leaves (door defaults, base/wall/full-height cabinets, floor defaults, stair and ramp defaults, wall defaults) and edit them as a group | Missing | NO SPEC -> DS-24 |
| Preferences vs OS | 103 | Region settings drive units, currency, decimal mark, thousands separator and date formats | Partial | units from the template; no locale pickup: NO SPEC -> DS-25 |
| Dynamic defaults | 103 | Changing a dynamic default updates every object still set to "use default"; shown as Use Default radio, Default check box or a wrench icon in fields | Partial | wall Layer "Default", roof pitch/overhang "use default" (RF-25); not uniform across objects: NO SPEC -> DS-26 |
| Dynamic defaults | 103 | Materials are dynamic defaults too, including library symbols | Works | C-61, materials defaults |
| Set as Default | 104 | Edit-toolbar button copying the selected object's spec into the defaults for its kind (not for terrain paths) | Missing | NO SPEC -> DS-27 |
| Multiple Saved Defaults | 104 | Saved defaults for Manual Dimensions, Revision Clouds, Rich Text, Text, Callouts, Markers, Arrows; one active at a time, per saved view | Partial | Saved Dimension Defaults sets (DIM-40) and Text Styles (TXT-17); the other kinds have one default page each (DS-18, DS-4): NO SPEC -> DS-28 for named sets |
| Multiple Saved Defaults | 104 | Also for Room Functions, Structural Member Reporting, Framing Types | Partial | Room Types list (default_lists.rs); framing types: Partial (CB-36) |
| Multiple Saved Defaults | 105 | Pasting an object into another file recreates its saved default, text style and layer there (existing ones are reused) | Missing | S-85 (layers kept: Partial); saved dimension defaults and text styles: NO SPEC -> DS-29 |
| Text Styles | 105 | Text-bearing objects and object labels take a Text Style (font, color, size); editing a style updates every object using it, while assigning a new style to a default leaves existing text alone | Works | TXT-17; plan-core text_styles.rs; Text Style Management (DECISIONS 86) |
| Saved Defaults dialog | 105 | Per tool: available list with Edit, Copy (name prompt), Rename, Delete (blocked when in a set or in use), Select/Clear All; Currently Active drop-down | Partial | `DimensionSetsDialog` has Edit / Copy / Rename / Delete / Currently Active for dimensions only; same dialog for Text, Callouts, Markers, Arrows, Revision Clouds: NO SPEC -> DS-30 |
| Saved Defaults dialog | 133 | Preference: double-click a drawing tool's button edits the active default or opens the Saved Defaults dialog | Missing | NO SPEC -> DS-31 |
| Active Defaults dialog | 106 | Tools > Active Defaults: choose the Default Set; Save New Default Set; Edit/Rename Default Set; per annotation kind pick/add/edit/rename/delete a saved default; layer set and current CAD layer with Define | Partial | menus.rs "Active Defaults…" opens the Default Settings tree only (APP-24) |
| Default Sets | 108 | Named bundles of saved defaults plus layer set and current CAD layer; switch from a toolbar control, the Active Defaults dialog or by opening a saved view; "Using Active Defaults" when one member is changed | Partial | DS-6 Default Sets page stores them (Round 15); activating a set and the toolbar drop-down: NO SPEC -> DS-32 |
| Default Sets dialog | 109 | New (copy), Delete, rename, per-kind selection with Add/Edit/Rename/Delete, layer settings | Partial | `default_pages/plan.rs default_sets`; verify against Chief |
| Templates | 110 | New plan/layout is a copy of a template with defaults, default sets, layers, layer sets, saved plan views, wall types, text macros, CAD-detail drawings, anything drawn | Works | templates.rs, APP-2, APP-60; text macros and wall types seeded |
| Templates | 111 | Choose default plan and layout templates per unit system in Preferences > New Plans (select or import, path edit) | Partial | Preferences > Templates (APP-60); US and metric slots: NO SPEC -> DS-33 |
| Templates | 111 | Hide metric/US templates in the browser; Use as Default Layout Template from the browser menu | Missing | NO SPEC -> DS-34 |
| Templates | 111 | File > Templates > New Plan from Template / New Layout from Template picking a template for this plan only | Partial | APP-2 (menu holds Save Current Defaults as My Template, Import Chief Template, Reset); a general "new from template" chooser: NO SPEC -> DS-35 |
| Save As Template | 112 | Save as Plan Template dialog: tick data categories to purge, set as default for US/metric, open a new plan after | Missing | APP-13 (only the defaults are saved) |
| Layout Template | 113 | Save as Layout Template with page-data purge | Partial | L-10 |
| Import Settings | 114 | File > Import > Import Settings from Plan/Layout: pick categories (Multiple Saved Defaults, Default Settings, Layer Sets, Note Types, Saved Plan Views, Wall Types) and choose replace or rename on name clashes | Partial | Layer Sets: `Import From Plan File` in layer_sets.rs; Chief template import: APP-2; the general chooser and the other categories: NO SPEC -> DS-36 |
| Import Settings | 115 | Layouts import only Default Sets and Layer Sets; CAD, Floor, Framing, Foundation and General Plan defaults are never imported | Differs | same rule would apply; n/a until the chooser exists |
| Legacy Settings | 116 | Import Layer Sets from a .layers file | Missing | NO SPEC -> DS-37 (layer sets come in with Chief plan import: plan-chiefplan bridge) |
| Legacy Settings | 117 | Import Default Sets from a .cadefs file (overwrite or rename duplicates; keep layer sets that exist) | Missing | NO SPEC -> DS-38 |
| Legacy Settings | 117 | Import Wall Definitions from a .dat file (replace or keep same-name types) | Missing | NO SPEC -> DS-39 (Wall Type Definitions dialog edits types; no file import) |
| Legacy Settings | 118 | Import Note Types from a .json file | Missing | NO SPEC -> DS-40 (note types are stored with the plan: s40_roundtrips) |
| Reset to Defaults | 118 | Edit > Reset to Defaults: scope current floor or all floors; reset floor and ceiling heights, roof groups, roof directives in walls, delete roof gable lines, reset wall top and bottom heights, wall auto connections, overridden dimension text | Differs | our "Reset to Defaults" resets the defaults tree to the template (APP-22, DS-20); Chief's resets values in the plan. The plan-side reset is Missing: NO SPEC -> DS-41 |
| General Plan Defaults | 119 | Opened from the tree or by double-clicking Select Objects; layout has a reduced version | Partial | DS-13; `default_pages/plan.rs general`: see Dialog panels |
| General Plan Defaults | 120 | Warn before deleting selected objects; ignore casing when resizing wall openings; pitch as degrees (-89 to 89); arrow-key scroll distance (12 in) | Missing | NO SPEC -> DS-42 |
| General Plan Defaults | 120 | Framing direction lines: parallel or perpendicular to framing; draw joists, rafters, trusses as single lines | Missing | NO SPEC -> DS-43 (framing defaults exist; this switch does not) |
| General Plan Defaults | 121 | Living area measured from wall main layer or surface; suppress living-area label on new exterior rooms | Missing | NO SPEC -> DS-44 |
| General Plan Defaults | 121 | Opening indicators: hinge indicators point to the handle | Missing | NO SPEC -> DS-45 (DW-83 has indicators) |
| General Plan Defaults | 121 | Geographic location (latitude, longitude, time zone) for sun angles | Missing | LAY-37 |
| General Plan Defaults | 121 | Grade Level Marker height and Elevation Reference (grade or first-floor subfloor = 0) for Auto Story Pole dimensions | Missing | NO SPEC -> DS-46 |
| General Plan Defaults | 122 | Angle snap increments, additional allowed angles with opposing angles, angle style | Partial | Snap Settings and Snap Properties: increment list and allowed-angle text (PR-12, S-71); opposing-angle table and Number Style button: NO SPEC -> DS-47 |
| General Plan Defaults | 122 | Snap grid: on/off, snap unit (1 in plan, 1/16 in layout), show grid, dots; reference grid: show, size, dots | Partial | grid.snap and grid.spacing; show-as-dots: NO SPEC -> DS-48 |
| Preferences dialog | 123 | Preferences open without a plan; panel list | Works | PR-1 |
| Preferences panels | 123 | Appearance panel (right-click menus, status bar items, line weight display, color-off mode, toolbar style, icon scale, minimum dimension/label size) | Partial | PR-3, PR-5; see Dialog panels |
| Preferences panels | 126 | Colors panel: plan, layout and preview backgrounds, selection line/fill, handle fills, selected-edge handle, snap and reference grid colors, reset; interface theme System/Light/Dark/Custom with copy/rename/delete of themes | Partial | PR-3; theme: Canvas Theme and UI Brightness (APP-39); custom themes: NO SPEC -> PR-17 |
| Preferences panels | 128 | Dialogs/Side Windows panel (dialog size/position memory, last panel, preview technique, material preview options, which side windows may dock top/bottom) | Missing | NO SPEC -> PR-18 |
| Preferences panels | 129 | Library Browser panel (include web results, filtering, rename behavior, tile names, double-click closes, preview technique, panel lock/title bars/reset) | Partial | PR-4 (preview size and search options only) |
| Preferences panels | 130 | Project Browser panel (lock panels, title bars, reset, only active projects, show default documents, expand on open) | Missing | NO SPEC -> PR-19 |
| Preferences panels | 131 | Text panel (Enter key makes a new line in multi-line fields; leader line segments; leader lines create rich text) | Partial | PR-5 shows fonts and links; the two behaviors: NO SPEC -> PR-20 |
| Preferences panels | 131 | Pattern Editor panel (preview color/transparency, repeat-box color/weight) | Missing | NO SPEC -> PR-21 (custom patterns do not exist, see section 8) |
| Preferences panels | 132 | General panel (undo on/off and levels 1 to 100, update check, startup choice, dashboard behavior, error reports, timing log, highlight overridden dimension text, pinch zoom sensitivity, Mac scroll options, edit defaults on double-click) | Missing | NO SPEC -> PR-22 (undo levels exist only as a stored default: DS-13) |
| Preferences panels | 134 | File Management panel (auto save minutes, file locking, recent list size/placement/clear, thumbnails small/large, auto archive Hourly/Daily/Previous Save, archive warning count, copy referenced material files, file association) | Partial | autosave minutes (default 5) and archive count (default 20) exist in `files.rs FileSettings` but no Preferences page edits them; file locking, recent-list options, thumbnails, archive mode: NO SPEC -> PR-23 |
| Preferences panels | 135 | Folders panel (data folder, user library, system library, program paths list) | Partial | PR-9 (textures, backdrops, templates, autosave, user library) |
| Preferences panels | 137 | Project Management panel (use PM, scheduled backups with frequency, retention, reminder, local storage, migrate forward) | Differs | no store; scheduled backup of the plan: NO SPEC -> PR-24 |
| Preferences panels | 137 | Ruby panel (safe level, load path) | Out-of-scope | scripting |
| Preferences panels | 138 | New Plans panel (units choice, plan/layout template per unit system, open-and-save-as folder rule, default designer and client information, copy designer/client from templates) | Missing | NO SPEC -> PR-25 (Project Information holds designer/client per plan; the global defaults and the folder rule do not exist) |
| Preferences panels | 139 | Unit Conversions panel: custom length/area/volume units with a multiplier to a base unit, "default unit" flag, locked built-ins, sample | Partial | PR-16 is a converter, not the custom-unit table; custom units (for scaled imports): NO SPEC -> PR-26 |
| Preferences panels | 142 | Time Tracker panel | Missing | see section 3 |
| Preferences panels | 142 | Architectural panel (same wall type handles, select room before wall in 3D, stair sections move independently, skylight ceiling-hole default, legacy opening indicators) | Partial | PR-13 has auto-rebuild switches and cabinet options, not these |
| Preferences panels | 143 | CAD panel (always By Object for block fills, By Object on new blocks, Connect CAD Segments, Advanced Splines, end cap printed length, same-line-type handles, 3D annotation offset) | Partial | PR-14 has arc centers, end caps, line weights; see Dialog panels |
| Preferences panels | 144 | Edit panel (crosshairs, edit handle size/tolerance, start/end indicators, marquee mode, automatic view scrolling and speed 2D/3D) | Partial | PR-10 has rotate/resize about, marquee mode, snap switches; auto-scroll exists (S-99) without a speed preference |
| Preferences panels | 146 | Coordinate System panel (floating axes, fixed axes, origin indicator per view type; axis colors, grid color, line length, label height; origin color/draw order; sun/moon indicator) | Missing | LAY-21 |
| Preferences panels | 148 | Behaviors panel (rotate/resize about, rotate jump, find angles for Center and Reflect, orthogonal or polar movement, stop when connected, edit type, concentric jump, behavior indicators) | Partial | PR-11 has Edit Type, concentric/fillet/chamfer values, camera steps; see Dialog panels |
| Preferences panels | 149 | Snap Properties panel (objects in history, snap distance, max bump distance, snap cabinets after paste, always snap walls to allowed angles, indicator size and colors, angle snap grid, each object and extension snap, bumping options, dimension line separation snaps, angle snaps, Shift increment override) | Partial | PR-12 Works for kinds, increment, allowed angles, sensitivity, bumping distance; see Dialog panels |
| Preferences panels | 153 | Master List panel (select or import the materials master list file, new list) | Partial | PR-7 (waste, rounding, prices); a master list file: NO SPEC -> PR-27 |
| Preferences panels | 153 | Render panel (horizon lines, 3D mouse/gamepad navigation center, cycle render techniques list with order, GPU ray tracing and diagnostics, GPU residency, fractional sample counts, Mac retina options) | Partial | PR-6 render defaults; cycle-technique list: NO SPEC -> PR-28; GPU switches Out-of-scope |
| Preferences panels | 155 | Video Card Status panel | Differs | System Information window (APP-49) |
| Preferences panels | 155 | Reset Options panel: reset message boxes, dialog sizes, toolbars, templates, side windows, search folders, migration, preferences | Partial | PR-8 covers toolbars, dialog sizes, messages, side windows; templates, search folders, preferences-wide reset: NO SPEC -> PR-29 |
| Number/Angle Style | 156 | Number Style button opens a dialog setting how distances, coordinates and angles are written in dialogs and the status bar (fractional inches and feet, decimal, metric) | Partial | project length format (LengthFormat) in Dimension Defaults; the global dialog: NO SPEC -> PR-30 |
| Number/Angle Style | 156 | Angle styles: degrees/minutes/seconds, quadrant bearings, azimuth bearings, pitch (rise over 12 or 1000) | Missing | NO SPEC -> PR-31 (surveyors' bearings matter on site plans) |

---

## 5. Toolbars and Hotkeys (pp. 158 to 174)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Toolbar arrangement | 158 | Toolbars per view kind; buttons that do not apply to a view are greyed | Works | toolbar/config.rs `ViewKind` sets; TB-1 |
| Toolbar arrangement | 158 | Show or hide all toolbars (View > Toolbars) | Works | APP-37, menus.rs |
| Toolbar arrangement | 158 | Drop-down tool families (arrow beside a button, last used tool shown) | Works | toolbar.rs flyouts (W-2) |
| Toolbar arrangement | 158 | Child Tool Palette: parent buttons with a blue corner open a docked palette of child tools; the palette also follows the drop-down style; user choice in Preferences | Missing | APP-36 |
| Child Tool Palette | 159 | Palette settings: Fit Palette, Grid View, List View, Maintain Width | Missing | APP-36 |
| Drop-down controls | 159 | Saved Plan View control on a toolbar | Works | toolbar/config.rs `View Selector`; toolbar.rs row-1 selector |
| Drop-down controls | 159 | Active Layer Set control, Active Dimension Defaults control, Active Default Set control as toolbar drop-downs | Missing | NO SPEC -> TB-6 (layer sets pick in the dock; dimension default sets pick in Default Settings) |
| Edit toolbar | 159 | Special toolbar, bottom of the window above the status bar, blank with no selection, contents follow the selection | Works | S-38..S-45 |
| Toolbar configurations | 160 | Default, Terrain, Space Planning and Extended configurations switchable from a button, the dialog or the toolbar context menu | Partial | Space Planning and Extended are stub toggles (TB-1, `config_space_planning`, `config_extended`); Terrain configuration: NO SPEC -> TB-7 |
| Toolbar size | 160 | Toolbar button size 20 px, adjustable in Preferences | Partial | icon size choice (PR-3) |
| Toggling toolbars | 160 | Right-click an empty toolbar space to toggle individual toolbars per view; Close button on floating toolbars | Partial | Customize Toolbars dialog edits rows per view (TB-1, TB-2); right-click menu on the toolbar: NO SPEC -> TB-8 |
| Moving toolbars | 161 | Drag a toolbar by its grab bar, dock to any side, float, reshape; positions saved between sessions | Missing | NO SPEC -> TB-9 (rows are fixed at the top; rows can be ordered in the dialog: TB-2) |
| Add and remove buttons | 161 | Drag buttons between the dialog and toolbars; Empty Space spacer; not for the Edit toolbar | Partial | TB-1 list editor with separators; drag from dialog to toolbar: Differs (list editing instead) |
| New toolbars | 162 | Create a toolbar by dropping a button in free space; named "Custom ..."; assign to view types | Partial | TB-2 new row, rename, duplicate; per-view assignment: TB-1 |
| Lock toolbars | 162 | Tools > Toolbars and Hotkeys > Lock Toolbars and context-menu lock | Works | `ToolbarConfig.locked` (toolbar/config.rs) |
| Restore toolbars | 162 | Reset Toolbars restores all installed configurations | Works | TB-5 |
| Custom configurations | 162 | Copy a configuration, name it, save into the Toolbars folder, switch to it; icon for the configuration (128 px, three mapped greys) | Partial | TB-3 JSON export/import; named configurations as separate files and Choose Icon: NO SPEC -> TB-10 |
| Importing configs | 166 | Import .toolbar files with a name-conflict dialog (Replace, Import New, Skip, Rename, Replace/Import/Skip All) | Partial | TB-4 reads a Chief .toolbar and replaces the rows; conflict handling: NO SPEC -> TB-11 |
| Import conflicts | 167 | Place Library Object button conflict dialog (Use Existing, Use New, Keep Both) | Missing | NO SPEC -> TB-12 (no Place Library Object buttons) |
| Customization dialog | 163 | Tools panel: view-type picker, searchable list of available buttons with parents expandable and a description | Works | `customize_toolbars.rs` (TB-1) |
| Customization dialog | 165 | Toolbar panel: table of toolbars with a check per view type, rename, delete, reset | Partial | TB-2; per-toolbar view-type table: Partial |
| Customization dialog | 165 | Configurations panel: Switch To, Copy, Rename, Delete, Choose Icon, folder path | Missing | NO SPEC -> TB-13 |
| Place Library Object | 161 | Toolbar buttons bound to a library item, fill style or line style (Place Library Object button) | Missing | NO SPEC -> TB-14 |
| Hotkeys | 168 | Menu items show their hotkey; hotkeys run tools; system-reserved keys cannot be used | Works | HK-1, HK-2; shell/hotkeys.rs |
| Hotkeys | 169 | Number keys and numpad keys as hotkeys; numpad keys distinct for user-defined | Partial | keys recorded; number-vs-numpad rule: NO SPEC -> HK-8 |
| Create Hotkey List | 169 | Save the list of hotkeys as an .html page for printing | Partial | Customize Hotkeys > Print List writes the assigned keys as a two-column PDF (`dialogs/hotkeys.rs`), and Export writes JSON, CSV or Chief's XML (HK-6); an HTML file: NO SPEC -> HK-9 |
| Next/Last command | 169 | Esc cancels the selection or action, or re-activates the previous tool; Shift+Esc moves to the next of up to 100 recent tools; two separate hotkeys can be assigned | Partial | Esc clears/returns to Select (S-9, S-10); recent-tool ring: NO SPEC -> HK-10 |
| Customize Hotkeys | 169 | Dialog: search commands or keys; tool-available-in list; description; record up to four chords; Reset Hotkeys | Works | HK-1..HK-3, HK-7 |
| Customize Hotkeys | 170 | Hotkeys for any command including opening defaults dialogs; a few commands (Delete, Enter Coordinates) cannot be reassigned | Partial | commands list from the menus; per-defaults-dialog commands: NO SPEC -> HK-11 |
| Export/import hotkeys | 171 | File > Export > Hotkeys and File > Import > Hotkeys as XML (import replaces user file) | Works | HK-5, HK-6 |
| Gamepad settings | 171 | Gamepad dialog: enable, command-to-button list, thumbstick speed/sensitivity/invert/linear acceleration, height change | Out-of-scope | hardware (APP-31 family); Windows-only feature in Chief |

---

## 6. Window and View Tools (pp. 175 to 187)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| View windows | 175 | Tabbed document interface; each tab names the file, view kind and saved-view name | Works | plan_tabs.rs |
| Saved vs unsaved views | 175 | Most views are temporary; CAD details and auto elevations save themselves; a saved view keeps floor, extents, color toggle, layer set, saved defaults, watermark | Partial | `SavedPlanView` keeps name, layer set, floor, reference display, camera/zoom, default dimension and text style (plan_views.rs, LAY-2); color toggle, saved-defaults set and watermark: NO SPEC -> LAY-46 |
| Saving views | 176 | Tools > Active View > Save Active View / Save Active View As; prompts for a name; Save View from the browser menu | Works | menus.rs "Active View" ▸, plan_views.rs `Save Plan View` |
| Saving views | 176 | Per-view settings that do not need a re-save: saved defaults or active default set, print preview toggle, drawing sheet setup | Partial | print preview flag Works; the other two: NO SPEC -> LAY-47 |
| View specification dialogs | 176 | One specification dialog per saved view kind: plan, camera, cross section/elevation, materials list, CAD detail, layout page information | Partial | Plan View Specification (plan_views.rs), Camera dialog (C-*), Layout Page Specification (L-2); Materials List spec Works (L-24); CAD Detail spec: Partial |
| Coordinate indicators | 176 | Floating axes indicator (corner), fixed axes at the origin, origin crosshair; toggles per view type; not printed | Missing | LAY-21 |
| Coordinate indicators | 147 | Axis colors, line length, label height, origin color and draw order (front/back), sun/moon direction indicator size | Missing | NO SPEC -> LAY-48 |
| Plan views | 177 | Plan view is the main drawing view; reopens on the last used saved view, or an unsaved one | Works | LAY-2; plan_tabs.rs |
| Saved plan views | 177 | Attributes per saved view: layer set, default set, reference display, floor display, display toggles | Partial | layer set, floor, reference display, dimension and text defaults (plan_views.rs); default set and display toggles: NO SPEC -> LAY-49 |
| New plan view | 177 | Tools > New Plan View: a copy of the current view as a new tab | Works | menus.rs "New Plan View"; app_info::NEW_PLAN_VIEW |
| New saved plan view | 178 | New Saved Plan View dialog: name; "Copy Layer Set" with a new layer-set name; created from Duplicate in the browser or from a layout box | Partial | `plan_views.rs` New / Duplicate (copy of the active view, can copy the layer set via Layer Set Management); the dialog with Copy Layer Set: NO SPEC -> LAY-50; create from a layout box: NO SPEC |
| Open saved view | 178 | Open from the toolbar control, the browser, or from a layout box's Open View button | Partial | toolbar selector and Project dock; layout box Open View: NO SPEC -> LAY-51 |
| Rotate Plan View | 178 | Tools > Rotate Plan View: rotate the whole plan view (objects, grids, coordinates, sheet) to a typed angle, relative to the original, shown as -180..180; text rotates unless "Rotate with Plan" is off; remember zoom/rotation | Partial | LAY-17: 90 left, 90 right, back to north up; typed angle, "Remember Zoom/Rotation" and per-text opt-out: NO SPEC -> LAY-52 |
| Rotate Plan View | 179 | CAD Detail from View and Plan Footprint inherit the view rotation; layout orientation unaffected | Missing | NO SPEC -> LAY-53 |
| Reverse Plan | 179 | Mirror the whole plan left to right; room labels with absolute positions reset; terrain not mirrored; rebuilds model and layout views | Works | S-115 (Round 15, `view_commands.rs reverse_plan`; verify in Chief) |
| Plan View Spec | 179 | Dialog opened from the browser (Edit View) or Tools > Active View > Edit Active View | Works | plan_views.rs |
| Plan View Spec | 180 | General panel: name, Saved flag, floor ("Use Any Floor"), remember zoom/rotation, show color, show watermark, link to layout | Partial | name, floor, zoom stored; Saved flag (all views saved), show color, watermark, link-to-layout: NO SPEC -> LAY-54 |
| Plan View Spec | 180 | Wall display: Poché (dark fill over wall tops), pony wall part options | Missing | NO SPEC -> LAY-55 (poché exists in section/elevation fills only) |
| Plan View Spec | 180 | Save options: Prompt to Save, Always Save, Never Save | Missing | NO SPEC -> LAY-56 |
| Plan View Spec | 181 | Selected Defaults panel (saved defaults and layer settings used in the view) and Reference Display panel | Partial | layer set, dimension and text defaults, reference display (plan_views.rs); the full Selected Defaults list: NO SPEC -> LAY-57 |
| Multiple views | 181 | No limit on open views; tab tooltips with project and plan or full path; scroll arrows when tabs overflow; drag to reorder | Partial | tabs exist; tooltip paths, scrolling, reordering: NO SPEC -> APP-137 |
| Tiling | 181 | Drag a tab to a window region to tile; Window > Tile Horizontally / Tile Vertically (Shift+F6); order by recent use; Window > Tab Windows ends tiling | Works | APP-41, APP-42, view_commands.rs |
| Multiple windows | 182 | Drag a tab out to a second program window / monitor | Missing | APP-66 |
| Swapping views | 182 | Swap Views (F7) between the two latest; Select Next/Previous Tab; Ctrl+Tab and Cmd+} / Cmd+{ cycle | Works | APP-40, APP-42 |
| Window list | 183 | List of open views at the bottom of the Window menu with a check mark; More Windows dialog beyond eight; Select Current Tab drop-down for toolbars | Partial | APP-43 (Floor Plan View, Layout rows); open-view list and the dialog: NO SPEC -> APP-138 |
| View tools | 183 | Refresh Display (F5) | Works | APP-34 |
| View tools | 183 | Library Browser, Project Browser, Active Layer Display Options, Walkthrough Preview, Action History side windows | Works | LAY-3, S-79, C-71, APP-35 |
| View tools | 183 | Tool Palette toggle | Missing | APP-36 |
| View tools | 183 | Status Bar and Toolbars toggles | Works | APP-37 |
| View tools | 183 | Scrollbars toggle (right and bottom, arrow buttons shift by 12 in) | Missing | APP-37 (no scrollbars in the plan view) |
| View tools | 183 | Color toggle per view | Works | LAY-19 |
| View tools | 183 | Crosshairs toggle | Works | LAY-20 |
| View tools | 183 | Coordinate System Indicator toggles (Floating, Fixed, Origin) | Missing | LAY-21 |
| View tools | 184 | Reference Grid toggle (Shift+F9 in plan) | Works | LAY-22 |
| View tools | 184 | Angle Snap Grid toggle | Missing | LAY-23 |
| View tools | 184 | Temporary Dimensions toggle | Works | menus.rs, S-56 |
| View tools | 184 | Arc Centers and Ends toggle | Works | CAD-10 |
| View tools | 184 | Line Weights toggle | Works | LAY-7 |
| View tools | 184 | Drawing Sheet toggle and Print Preview toggle | Works | menus.rs "Drawing Sheet", "Print Preview" (L-19) |
| View tools | 184 | Watermarks toggle (text or image watermark) | Missing | L-53 |
| Zoom | 184 | Zoom tool: drag a marquee, then the previous tool returns | Works | LAY-24 (Round 15, `zoom_window_input`) |
| Zoom | 184 | Zoom In / Zoom Out by a factor of two; Undo Zoom (not in the undo history) | Works | LAY-25, LAY-26 |
| Zoom | 184 | In 3D, the Zoom tool does not move the camera | Differs | 3D zoom moves the camera (C-36); fine |
| Zoom | 185 | Mouse-wheel zoom about the pointer, about 10 percent a click; Ctrl/Cmd modifier depending on mouse | Works | LAY-35 |
| Fill Window | 185 | Fill Window (F6) fits everything visible incl. reference display, not CAD points; with the drawing sheet shown, fits the sheet | Works | LAY-28 (Round 15 `all_bounds`); sheet case: NO SPEC -> LAY-58 |
| Fill Window | 185 | Fill Window Selected Objects; Fill Window Building Only (walls, railings, roof planes even if layers off) | Works | S-96, LAY-27 (Round 15) |
| Fill Window | 185 | 3D behavior: perspective restores the original zoom, orthographic adjusts the field of view | Partial | C-36; orthographic fit: NO SPEC -> LAY-59 |
| Panning | 185 | Shift+arrow keys pan by the "inches scrolled by arrow key" setting | Missing | NO SPEC -> LAY-60 (arrow keys nudge the selection, S-92; with nothing selected no pan was found in select.rs or main.rs; verify) |
| Panning | 186 | Middle-button pan with a hand cursor; Option-drag on Mac; two-finger drag | Works | LAY-29 (middle- and right-drag pan) |
| Panning | 186 | Pan Window tool, double-click to keep it | Works | LAY-33 |
| Panning | 186 | Scrollbars | Missing | see above |
| Panning | 186 | Automatic view scrolling while drawing or dragging past the edge; speed settings | Partial | S-99 (Round 14); speed preference: NO SPEC -> LAY-61 |
| Closing | 186 | Close View (Ctrl+W / Ctrl+F4), middle-click a tab, close button, browser menu Close View | Partial | APP-8; middle-click close: NO SPEC -> APP-139 |
| Closing | 187 | Close All 3D Views; Close All Views; Close All Plans/Layouts of a project | Works | APP-9, APP-10 |
| Closing | 187 | Save prompt when closing the last view of a plan with unsaved changes; prompt to save an edited saved view | Partial | files.rs unsaved prompt Works; saved-view save prompt: NO SPEC -> APP-140 |

---

## 7. Creating Objects (pp. 188 to 204)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Object classes | 188 | CAD objects exist only in the view they were drawn in (plan, elevation, camera views, CAD detail, layout); architectural objects show in 2D and 3D; some are CAD-based | Partial | plan, elevation and layout CAD Works (CAD-*); CAD drawn in camera views: Missing (3D annotation, part 3) |
| Snap Settings menu | 188 | Edit > Snap Settings: Object Snaps, Angle Snaps, Grid Snaps, Bumping/Pushing | Works | S-68, S-72, S-73, snap_settings.rs |
| Edit Behaviors menu | 189 | Six global edit behaviors: Default, Alternate, Move, Resize, Concentric, Fillet | Partial | S-65: we have Default, Resize, Concentric, Fillet, Chamfer, Alternate (axis-locked move) and Replicate; no Move behavior; our Alternate is not Chief's (continuous drawing, angle-keeping reshape, line/arc switch) |
| Edit Behaviors menu | 189 | Connect CAD Segments toggle (also in Preferences, resets to on at exit) | Partial | CAD-4 toggle exists (ViewFlag::ConnectCad) |
| Edit Behaviors menu | 189 | Rotate/Resize About Current Point toggle | Partial | PR-10 stores it; applied by the transform code: verify |
| Arc Creation Modes | 189 | Five modes: Free Form (click and drag along the path), Center/Radius/End, Start/End/On Arc, Start/Tangent/End, Arc About Center | Partial | CAD-7: ThreePoint, CenterStartEnd, StartEndRadius, Tangent; no Free Form, no separate Arc About Center |
| Snap behaviors | 190 | Hold Ctrl/Cmd while dragging a handle to suspend snaps and movement restrictions | Partial | Alt suspends snaps (S-74); Ctrl as override: NO SPEC -> S-121 |
| Snap hotkeys | 190 | Temporarily enable one snap kind by holding a key during a drag when the category is off; keys shown in the Snap Settings submenu | Missing | NO SPEC -> S-122 |
| Object snaps | 190 | Object Snaps toggled from menu, button or Preferences; extend to the lowest-priority "On Object"; Bumping overrides snaps | Works | S-68, S-69 |
| Object snaps | 191 | Press 1 to clear snap indicators; hold S to suspend object snaps but keep extension anchors | Missing | NO SPEC -> S-123 |
| Object snaps | 191 | Snap kinds: Endpoint, Midpoint, Center, Quadrant, On Object, Points/Markers, Intersection | Works | snap.rs `SnapKind`, PR-12 |
| Object snaps | 191 | A multi-object selection snaps by its midpoint only | Missing | NO SPEC -> S-124 |
| Extension snaps | 191 | Extension anchors (small blue circles) set when hovering an endpoint, midpoint or quadrant; Tangent, Perpendicular, Orthogonal extension lines from the anchor (up to a set number of anchors) | Partial | snap.rs has Tangent, Perpendicular and a collinear Extension; no anchors, no Orthogonal extension, no history count: NO SPEC -> S-125 |
| Extension snaps | 191 | Wall intersection extension lines while drawing walls (needs Intersection snaps on) | Partial | W-34/W-36 intersection snaps; extension lines for walls: verify |
| Angle snaps | 192 | Draw at Allowed Angles (15 degree default); also governs rotation, arc radius; a cursor badge when off | Partial | S-71; badge: NO SPEC -> S-126 |
| Angle snaps | 192 | Angle snap increments plus additional allowed angles per file | Works | PR-12 `parse_angles`, Snap Settings list |
| Angle snaps | 193 | Hold Shift for restrictive snaps at 90 or 45 degrees (choice in Preferences) | Missing | NO SPEC -> S-127 (Preferences > Snap Properties "Angle Increment Override") |
| Angle snaps | 193 | Priority: object snaps above angle snaps above grid and On Object; walls give angle snaps priority | Works | S-69 |
| Angle Snap Grid | 193 | Visual rays of the allowed angles from the last point while drawing; hatch ticks at the snap unit; toggle in View and Preferences | Missing | LAY-23 |
| Grid snaps | 194 | Grid Snaps toggle (Ctrl+F9), in Plan Defaults, in the toolbar | Works | S-72 |
| Reference grid | 194 | Reference Grid for scale (not snapped); View toggle | Works | LAY-22 |
| Grid appearance | 194 | Snap and reference grid colors global; sizes per plan; dots or lines | Partial | colors in Appearance (PR-3); sizes in General (DS-13); dots: NO SPEC -> S-128 |
| Grid and angle | 194 | With both grid and angle snaps on, lengths snap in polar steps of the snap unit | Works | S-72 note |
| Nudging | 194 | Arrow keys nudge by the snap unit; Shift for the reference grid size; in 3D relative to the picked surface | Partial | S-92 (plan); 3D surface-relative nudge: NO SPEC -> S-129 |
| Creating | 195 | Five ways to add objects: click, click-drag, enter coordinates, distribution path/region, import | Works | tools/*, Distributed objects (CAD-51/52), imports |
| Creating | 195 | Layer hidden or locked prompts: offer to display or unlock; creation refused on a locked layer | Partial | CAD, dimension, camera and detail tools refuse on a locked layer (`cx.layers().is_locked` in tools/cad.rs, dimension.rs); the offer to unlock or to turn a hidden layer on: NO SPEC -> S-130 |
| Creating | 195 | Start near the origin 0,0; pointer position in the status bar | Works | status.rs |
| Click to create | 195 | Preview outline follows the pointer, back-edge center snaps; doors, windows, cabinets shrink to fit, others warn "not enough space" | Works | DW-3 ghost preview (Round 14), cabinets fit-to-gap |
| Click to create | 195 | Free-standing 3D click-to-create near a wall or inside the terrain perimeter | Partial | 3D placement of library items (C-*, L-*); verify |
| Click and drag | 195 | Hold Shift to slow the pointer while dragging; Esc or two-button click cancels | Partial | Esc Works (S-100); Shift slow-down: NO SPEC -> S-131 |
| Continuous drawing | 196 | Alternate behavior (or right-click drag, or right-click a Same Line Type handle) draws chains of lines/arcs by clicking; ends on Esc, double-click, closed shape; "Stop When Connected" option | Partial | Draw Line chains while Connect CAD Segments is on (CAD-3); right-click chain gesture and Stop When Connected: NO SPEC -> CAD-68 |
| Continuous drawing | 196 | Segments join into polylines only on the same layer with identical attributes | Works | CAD-4, CAD-15 |
| Enter Coordinates | 196 | Tab or Enter mid-drag opens the dialog: start location, Absolute or Relative to Start, Polar with Distance and Angle; remembered | Partial | typed length and angle (W-15, W-16, S-28); the absolute X/Y entry and remembered mode: NO SPEC -> S-132 |
| Input dialogs | 197 | Input Line, Input Arc, Input Point dialogs | Works | CAD-5, CAD-8, CAD-2 |
| Auto rebuild | 197 | Foundations, roofs, attic walls, framing rebuild with model changes; hand-edited ones stop rebuilding | Works | RF-6, R-61; the Preferences switches (PR-13) are stored but read by nothing yet |
| Distributing | 197 | Distributed Objects along a path or in a region | Works | CAD-51, CAD-52 |
| Importing / converting | 197 | Custom symbols, CAD drawings, pictures, metafiles; convert CAD objects to other types | Partial | imports Works (L-43..L-51); Convert Polyline: see section 10 |
| Layers and copy | 198 | An object on a custom layer pasted into another file creates the layer there | Missing | S-85 (Partial) |
| Cut/Copy/Paste | 198 | Copy to the system clipboard, usable across floors, views, files and other programs; objects cannot go where they could not be created | Partial | S-81..S-85: in-app clipboard with a cross-file file; system clipboard for text/pictures: NO SPEC -> S-133 |
| Cut/Copy/Paste | 198 | Do not paste US objects into metric files | Differs | n/a |
| Cut/Copy/Paste | 199 | Paste mode: click once to place; pasted objects are selected; text on the clipboard makes a Text object; an image opens the Paste Image dialog | Partial | S-82; text paste into a Text object and the Paste Image dialog: NO SPEC -> S-134 |
| Copy/Paste tool | 199 | Copy/Paste edit button: pastes by clicking or by dragging a handle (Move, Orthogonal Move, Concentric Resize, corner, Rotate) so the copy is moved/rotated/reshaped relative to the original; distance in the status bar | Partial | Replicate behavior and Ctrl-drag copy (S-94, S-65); handle-by-handle copy mode: NO SPEC -> S-135 |
| Copy/Paste tool | 200 | In Paste mode extra edit buttons: Sticky Mode, Paste Hold Position, Point to Point Move, Point to Point Center, Center Object, Reflect About Object, Main Edit Mode | Partial | Paste Hold Position Works (S-83); Reflect Copy About Object exists (S-105); the rest: NO SPEC -> S-136 |
| Sticky Mode | 201 | Stay in the current edit mode to repeat (Copy/Paste, Multiple Copy, Trim, Extend, Concentric Resize); Esc or Main Edit Mode to leave | Missing | NO SPEC -> S-137 |
| Paste Hold Position | 201 | Paste at the same coordinates on another floor or plan; not for walls, railings, fencing | Works | S-83 |
| Copy and Paste in Place | 201 | Copy at the same location, copy stays selected; not for walls, railings, fencing | Works | S-84 |
| Paste Special | 202 | Paste as EMF, DIB bitmap, plain text, file name, HTML text, or Model Objects | Partial | menus.rs "Paste Special" submenu exists; the representations: verify |
| Multiple Copy | 202 | Edit-toolbar Multiple Copy: drag the Move/Rotate/Orthogonal Move/Concentric Resize handle to lay evenly spaced copies; not for terrain perimeter, room boxes | Works | Round 15 `cad_ops.rs MULTIPLE_COPY_DRAG` (two clicks and ticks), `dialogs/multiple_copy.rs`; concentric and orthogonal variants: Partial |
| Multiple Copy | 203 | Array with the Alternate behavior: first drag sets the primary offset/count, second direction the secondary | Missing | NO SPEC -> S-138 |
| Multiple Copy dialog | 203 | Separate copy intervals for general objects, roof and floor trusses, rafters, joists/posts/beams, wall studs; rotation interval; fixed offset or evenly-distributed count (primary and secondary); global and remembered | Partial | `multiple_copy.rs`: one offset, count, optional turn per copy, step or total; per-object-class intervals and secondary offsets: NO SPEC -> S-139 |
| Transform/Replicate | 204 | Transform/Replicate Object dialog (see section 9) | Works | S-47, S-103 |

---

## 8. Displaying Objects (pp. 205 to 235)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Layer settings | 205 | CAD objects take line style, color and weight from their layer unless overridden; architectural objects draw in plan with layer attributes; Vector View edge lines use them too | Works | LAY-1; CAD ByLayer (CadAttrs) |
| Fill styles | 205 | Closed CAD shapes and many architectural objects (plan only) can be filled with a solid or pattern, transparent allowed | Partial | CAD fill: color, opacity, pattern name, spacing, angle (`FillAttr`); walls/rooms/slabs have their own fills; one common Fill Style panel: NO SPEC -> CAD-69 |
| Patterns and textures | 205 | Material patterns show in Technical Illustration, Hand Drawn and Vector views, textures elsewhere; 3D > Toggle Patterns | Partial | C-* materials/patterns; Toggle Patterns command: NO SPEC -> CAD-70 |
| Display layers | 205 | Layers organise display of every object in every view except materials lists | Works | LAY-1..LAY-5 |
| Display layers | 206 | Find which layer an object is on: spec dialog, status bar | Partial | spec dialogs show Layer; status bar layer line: S-98 Partial |
| Display layers | 206 | Find Objects on Layer(s) with Select Location dialog when the layer is used in several places | Missing | NO SPEC -> LAY-62 |
| Layer sets | 206 | Layer sets collect display settings; per-view-type default layer sets | Works | LAY-2, LAY-8, LAY-11 |
| In-use layers | 206 | "Used" column icons: objects on it, set in a default, or system default layer; tool tips say where | Partial | layer_display.rs has a Used column (icon kinds and tooltips: NO SPEC -> LAY-63) |
| Primary / secondary layers | 206 | An object has one primary layer (controls whether it shows) and secondary layers (countertop, face indicators, module lines, labels for cabinets) | Partial | cabinet and opening label layers (layers.rs `ensure_*_label_layer`); generic primary/secondary model: NO SPEC -> LAY-64 |
| Layers in views | 207 | Hidden layer: objects cannot be seen or selected; a hidden host hides its inserted doors and windows; only displayed objects export (DXF/EMF) and print | Works | LAY-4, S-5, export code |
| Appearance attrs | 207 | Layer attributes Color, Fill, Weight, Line Style (and Text Style for dimensions, labels, callouts, markers) | Partial | color, weight, line style, text style editable (layer_display.rs); layer Fill: Missing |
| Edge and pattern lines | 207 | 3D vector-view surface edge lines use layer attributes unless a slab/CAD-based object has Use Object Settings in 3D View Defaults | Partial | Vector View technique exists (C-50); Use Object Settings: NO SPEC -> LAY-65 |
| Locking layers | 208 | Locked layers: objects cannot be selected or drawn on; Delete Objects can still delete them; use locks for reference layers | Works | LAY-5, S-5, S-89 |
| Locking layers | 208 | Lock to protect plan opened in the Viewer | Out-of-scope | vendor viewer |
| Layer Display Options | 208 | Tools > Layer Settings > Display Options: layer table dialog; same table for object layers, layout page, layout views, and materials list (Name, Disp only) | Works | layer_display.rs, LAY-3; materials list variant: NO SPEC -> LAY-66 |
| Layer sets bar | 209 | Layer set drop-down, Copy Set, Modify All Layer Sets (one-time check), Reset Layer Names, Delete Unused Layers | Partial | set selector with New/Copy/Rename/Delete and Modify All Layer Sets Works; Reset Layer Names and Delete Unused Layers: Missing |
| Layer table | 209 | Name filter; sortable columns Used, Disp, Lock, Color, Fill, Line Style, Line Weight, Text Style | Partial | Name, Used, Disp, Lock, Ref, Color, Weight, Line Style, Text Style; no Fill column |
| Layer table | 210 | Select All (Ctrl+A) and multi-select by Shift/Ctrl; remembers the selection | Works | layer_display.rs |
| Layer table | 210 | New layer (unique name; added to all sets, hidden in others), Copy layer, Merge layers (objects and defaults move), Delete layer (unused, non-system), Reset Names | Missing | NO SPEC -> LAY-67 (`LayerSet::add` exists in plan-core; no UI uses it) |
| Layer properties | 210 | Properties of the selected layers: Display, Lock, Color, Line Weight, Line Style with library button, Text Style with Define, Fill Style preview; "No Change" for mixed | Partial | all but Fill; mixed values supported (multi-select edit) |
| Active Layer Display Options | 211 | Side window: shows the active layer set, or Object Layer Properties when objects are selected; contextual menu; Options button choosing what to show (layer-set control, management buttons, name filter, edit buttons, properties, show all layers, columns) | Partial | the dock exists (LAY-3) with filter and set controls; selection-driven Object Layer Properties and the Options/Columns menu: NO SPEC -> LAY-68 |
| Layer sets | 212 | Five ways to activate a layer set (toolbar control, default set, saved view, side window, Active Defaults) | Partial | dock and saved views; toolbar control and default sets: NO SPEC -> LAY-69 |
| Layer sets | 212 | Layer sets stored in the plan; layout views use the plan's sets; sets are per view; sets in templates | Works | LAY-2 |
| Modify All Layer Sets | 213 | Apply a layer edit to the same layer in every set | Works | layer_display.rs `all_sets` |
| Layer Set Defaults | 213 | Dialog (Default Settings > Layer Sets) choosing the initial layer set for nine view kinds incl. the reference floor; "Use Active Layer Set" | Missing | NO SPEC -> LAY-70 (LAY-11 lists defaults but no chooser) |
| Layer Set Management | 214 | Tools > Layer Settings > Layer Set Management: list with Define, New (from system defaults), Copy, Rename (not Default Set), Delete (not active, default or in use); active set for the current view | Works | LAY-8; layer_sets.rs (New, Copy, Rename, Delete, Make Active, Import From Plan File) |
| Select Layer | 215 | Select Layer dialog used by Current CAD Layer and the Layer Painter, with the table, filter, editable properties and "Use Default Layer" | Partial | Current CAD Layer chooser (LAY-6); Layer Painter bar (LAY-18, Round 15); no Use Default Layer, no shared dialog: NO SPEC -> LAY-71 |
| Layer panel | 215 | Layer panel in object dialogs: Default check box, drop-down, Define; drawing group selector | Partial | wall Layer tab, other dialogs; Drawing Group via Set Drawing Group (Round 15) |
| Object Layer Properties | 216 | Edit-toolbar button listing every layer an object touches, with name filter and Show All Layers | Missing | NO SPEC -> LAY-72 |
| Layer Painter | 217 | Layer Painter, Layer Eyedropper, Use Default Layer | Works | LAY-18 (Round 15, `tools/painters.rs`; verify in Chief) |
| Layer Hider | 217 | Click an object to turn off its primary layer in the active layer set | Missing | NO SPEC -> LAY-73 |
| Drawing groups | 218 | Every object belongs to a drawing group deciding front/back order; fixed order within a group by creation; reported in the status bar; also affects selection order | Works | LAY-36 (Round 15, `plan_core::drawing_group`, drawing_groups.rs) |
| Drawing groups | 218 | Edit tools Send to Back, Send Backward, Bring to Front, Bring Forward; Select Drawing Group dialog ("Default: Multiple Values") | Partial | Bring to Front / Send to Back / Set Drawing Group (S-113, Round 15 `DG_*`); Bring/Send Forward-Backward by one group: NO SPEC -> LAY-74 |
| Drawing groups | 219 | Default drawing group per object type in each defaults dialog | Works | drawing_groups.rs (Default Settings > Drawing Groups) |
| Display toggles | 219 | Refresh Display (F5) | Works | APP-34 |
| Color on/off | 219 | Per-view color toggle; off gives grayscale in 3D and black-and-white or grayscale in line views by the "Color Off Is" preference; vector-view color default in 3D View Defaults | Partial | LAY-19 toggles; the black-and-white vs grayscale choice: NO SPEC -> LAY-75 |
| Drawing Sheet / Print Preview | 220 | Drawing sheet outline and Print Preview toggles per view | Works | menus.rs, L-19 |
| Arc Centers and Ends | 220 | Toggle in the current elevation or on the current floor in all plan views | Works | CAD-10 |
| Watermark | 220 | Text or image watermark toggled per view | Missing | L-53 |
| Reference Grid / Crosshairs / Coordinate indicators / Angle Snap Grid / Temporary Dimensions / Line Weights | 220 | Display toggles listed with their scope (view, file or global) | Partial | Works except Coordinate indicators (LAY-21) and Angle Snap Grid (LAY-23) |
| Delete Surface | 221 | In 3D, hide one surface temporarily | Works | menus.rs "Delete Surface" |
| Fill styles | 221 | Fill Patterns library catalog; Fill Style Eyedropper and Painter tools (plan; not 3D) | Missing | CAD-56 |
| Fill styles | 222 | Import Patterns (.pat) into the User Catalog | Missing | NO SPEC -> CAD-71 |
| Fill gradients | 222 | Gradient editor: linear or radial, angle, color points with transparency and position, add/remove/reset, preview with background and solid-vs-pattern view | Missing | NO SPEC -> CAD-72 |
| Fill Style panel | 223 | Fill Style panel in many dialogs and the Fill Style Specification dialog (named, library); Layer Fill Style dialog for wall layers; cabinet box/countertop, rail, sprinkler, framing fill style dialogs | Partial | wall layers have fills (wall types dialog); cabinet and rail fills: Partial/Missing by dialog; one shared panel: NO SPEC -> CAD-73 |
| Fill Style panel | 224 | Pattern type list: Use Layer, Library, system patterns; scale, row offset (grid offset/step), width/height, concrete/sand spacing | Missing | NO SPEC -> CAD-74 (only `HATCHES`: 11 hatch names with spacing) |
| Fill Style panel | 225 | Horizontal/vertical offsets, angle, line weight; single color, use layer color, use background color, transparency, gradient | Partial | color, opacity, spacing, angle in `FillAttr`; offsets, line weight, background color source, gradient: NO SPEC -> CAD-75 |
| Fill Style panel | 225 | Pattern background: transparent or color (layer/view/custom), transparency, gradient | Missing | NO SPEC -> CAD-76 |
| Fill Style panel | 226 | Add to Library button on the Fill Style panel; preview width choice | Missing | NO SPEC -> CAD-77 |
| Poché | 226 | Dark fill on cut walls, floor and ceiling platforms and roof planes in plan, section, floor overview and sliced cameras (not glass walls, invisible walls, railings, retaining walls, fences); per-view and per layout box switch | Partial | section/elevation poche gray fill (plan-layout render.rs); plan-view Poché toggle and platform/roof cuts: NO SPEC -> CAD-78 |
| Custom patterns | 227 | Create Pattern from drawn CAD (edit button), CAD > Patterns > Create New Pattern, or User Catalog > New > Pattern; Pattern Specification dialog (name, repeat box size, horizontal/vertical shift) | Missing | NO SPEC -> CAD-79 |
| Custom patterns | 228 | Pattern window (like a CAD detail): uneditable preview, editable tile, repeat box (spec dialog and handles), color/transparency in Preferences, Save Active View updates the pattern, Add Pattern to Library | Missing | NO SPEC -> CAD-80 |
| Custom patterns | 229 | Pattern Tile Groups (next, previous, add, delete) and Infinite Pattern Lines with spec dialog | Missing | NO SPEC -> CAD-81 |
| Line styles | 230 | Line style on layer, on object, or from the library; draw a line directly with a library line style | Partial | four built-in styles (`LineStyle`: Solid, Dashed, Dotted, DashDot) on layers and CadAttrs; library line styles: NO SPEC -> CAD-82 |
| Line Style Management | 230 | CAD > Lines > Line Style Management dialog: table with sample, name, Used icon, Edit, New, Copy, Move Up/Down, Purge, Delete, Merge, Add to Library; Auto Purge Line Styles | Missing | NO SPEC -> CAD-83 |
| In-use line styles | 232 | Icons: red plus (on objects), wrench (in a default), S (system) with tool tips | Missing | NO SPEC -> CAD-84 |
| Import line styles | 232 | File > Import > Import Line Styles (.lin or .dat) | Missing | NO SPEC -> CAD-85 |
| Line Style Specification | 232 | Name; preview with highlight and repeating-segment marks; add Dash, Dot or Text components, insert before/after; reorder/remove; length, spacing, text and height scaled to the drawing scale; text uses the layer text style | Missing | NO SPEC -> CAD-86 |
| Select Color dialog | 234 | Tools > Color Chooser: basic colors, spectrum with luminosity bar, HSL/RGB fields, alpha or opacity where relevant, hex, custom colors (session), eyedropper from the screen, Create Material | Partial | APP-33 Color Chooser dialog; colors elsewhere use egui's picker (RGB, hex via egui; no screen eyedropper or Create Material button: NO SPEC -> CAD-87) |
| Color bars | 234 | Hover over a color bar for a tool tip with RGB values | Missing | NO SPEC -> CAD-88 |

---

## 9. Positioning Objects (pp. 236 to 251)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Moving | 236 | Move handle drag, grid-snapped, sticks at the original location with object snaps on | Works | S-13 |
| Moving | 236 | Move by edit behavior: orthogonal to the edges (Default/Resize/Concentric/Fillet), at allowed angles (Alternate), by any resize handle (Move) | Partial | S-65: no Move behavior; Alternate is axis lock, not allowed-angle move |
| Bumping/pushing | 236 | Objects bump into and push each other; stop and continue past if the drag goes on; push by dragging again the same way; CAD Stops Move and Wall Stops Move flags | Partial | S-73 bumping with distance; push-on-second-drag, CAD Stops Move and Wall Stops Move: NO SPEC -> S-140 |
| Bumping/pushing | 237 | Not applied to dimension or Enter Coordinates moves unless "for type-in movement" is on; roof planes and locked-layer objects can be bumped but not pushed; electrical connections, material regions unaffected | Missing | NO SPEC -> S-141 |
| Unrestricted movement | 237 | Hold Ctrl/Cmd while dragging to ignore snap restrictions and bumping | Partial | Alt frees (S-22); Ctrl: NO SPEC -> S-142 |
| Nudging | 237 | Arrow keys nudge by snap unit, Shift for the larger reference-grid size; direction follows a rotated plan view; optional Nudge hotkeys | Partial | S-92; rotated-view directions and separate Nudge commands: NO SPEC -> S-143 |
| Nudging | 238 | In 3D nudge relative to the selected surface (a rotated cabinet moves along its own wall) | Missing | NO SPEC -> S-144 |
| Point to Point Move | 238 | Click Point A on the object, then Point B target, snaps respected, Shift for allowed angles; combine with Copy/Paste to leave a copy; also with Edit Area | Works | S-52 (Shift-angle and the copy variant: verify) |
| Move to Framing Reference | 239 | Edit tool placing a framing member relative to a Framing Reference marker | Missing | NO SPEC -> S-145 (framing reference markers, part 3) |
| Moving with dimensions | 239 | Click a dimension value and type; bumping/pushing optional via preference | Works | S-59..S-62 |
| Enter Coordinates | 239 | Move edit handle drag, then Tab for typed coordinates; push objects with it when enabled | Partial | S-28 (typed length/angle); coordinate form: NO SPEC -> S-146 |
| Transform/Replicate | 239 | Move relative to itself or an absolute location from the dialog | Works | S-47, S-103 |
| Center Object | 239 | Center an object on an axis: collinear with an edge, through an edge midpoint, or the bisector of a corner; also centers relative to a room or the exterior room; preview shown; Find Angles preference | Partial | S-53 Works for objects against walls/rooms (edit_commands.rs `center`); exterior room, bisector axes and Find Angles toggle: verify in Chief |
| Center Object | 240 | Center Object keeps a copy via Copy/Paste first | Missing | NO SPEC -> S-147 |
| Point to Point Center | 240 | Center an object between two clicked points, with temporary CAD points offered (2 or 5 depending on slope) | Missing | NO SPEC -> S-148 |
| Align/Distribute Objects | 241 | Dialog with vertical choices (Don't Move, Top Edges, Centers, Bottom Edges, Space Evenly, Distribute Centers Evenly) and horizontal choices (Left Edges, Centers, Right Edges, Space Evenly, Distribute Centers Evenly); space options need three objects; plan view only for architectural objects | Partial | S-54: `transform.rs` Align/Distribute window with align modes and equal-gap distribute horizontally/vertically; Distribute Centers Evenly and combined horizontal+vertical in one apply: NO SPEC -> S-149 |
| Align/Distribute Along Line | 242 | Click an axis (collinear with an edge or midpoint, perpendicular through a midpoint, corner bisector) then Align To Line dialog: alignment None/Centers/Closest Edges; distribution None/Space Evenly/Distribute Centers Evenly; Distribute To Endpoints or Between Endpoints | Missing | NO SPEC -> S-150 |
| Aligning by eye | 243 | Crosshairs as an alignment aid | Works | LAY-20 |
| Aligning by snaps | 243 | Snap an edge to another object's edge; with grid snaps and angle snaps, alignment is by snap increments only | Works | S-69 |
| Aligning by dimensions | 244 | Temporary or manual dimension to relocate to the same value; text alignment via Text spec | Works | S-59, TXT-* |
| Stops | 244 | CAD Stops Move and Wall Stops Move as alignment guides | Missing | NO SPEC -> S-151 |
| Make Parallel/Perpendicular | 244 | Select an edge (near an end locks that end, near the middle locks the midpoint), pick a straight edge, 45 degree rule chooses parallel or perpendicular; adjacent polyline edges stretch | Works | S-49, CAD-54 (`MakeParallel`, `MakePerpendicular`) |
| Make Parallel/Perpendicular | 244 | Applies to CAD block instances (rotates the whole block) and to the entire polyline via a dialog option (session-wide); construction lines and cameras as axes | Missing | NO SPEC -> S-152 |
| Make Arc Tangent | 245 | Make the arc tangent to the attached line or arc; with both ends attached a Radius of Tangent Arc dialog asks for the radius | Partial | CAD-24 (Round 14): one-sided and polyline cases; the radius dialog for two attached ends: NO SPEC -> S-153 |
| Arc centers | 246 | Align arc centers across floors using the Reference Display plus Arc Centers and Ends and object snaps | Partial | Reference Display Works (LAY-9); Center snap on a reference arc: verify |
| Floors | 246 | Align objects on different floors with snaps to the Reference Display | Works | S-111, snap to reference objects (LAY-15) |
| Rotating | 246 | Rotate handle, angle snaps, Shift slows motion, Ctrl override; angles of lines via end handle; spec dialog angle fields | Works | S-15 (Partial: readout), S-101, S-102 |
| Rotate/Resize About | 247 | Rotate or resize about the object center or the current CAD point (Edit > Edit Behaviors toggle and Preference) | Partial | PR-10 stores it; "current point" as pivot: verify |
| Entering coordinates | 247 | Rotate via Enter Coordinates (distance and angle) | Partial | typed input (S-28) |
| Angular dimensions | 247 | Set Angular Dimension dialog: new value, rotate edge only or rotate the entire polyline about the angle vertex | Missing | NO SPEC -> S-154 (angular dimensions are part 2; the edit-by-value dialog) |
| Make Parallel dialog | 248 | Double-click Make Parallel: Rotate Selected Edge Only or Rotate Entire Polyline (session-wide) | Missing | NO SPEC -> S-155 |
| Transform/Replicate | 248 | Rotate to a relative or absolute angle from the dialog | Partial | `transform.rs`: one angle about the center or a point; relative-vs-absolute choice: NO SPEC -> S-156 |
| Multiple Copy | 248 | Rotated arrays with Multiple Copy | Works | cad_ops.rs MULTIPLE_COPY (turn per copy) |
| Reflect About Object | 248 | Reflect about another object: line (collinear or perpendicular through midpoint), polyline edge or corner bisector, room or wall, box/circle axis front to back; 3D only about a vertical axis; preview on the far side | Partial | S-48 (about a line/axis), `Reflect Copy About Object` (S-105); room/wall/corner-bisector axes and front-back axis: NO SPEC -> S-157 |
| Reflect About Object | 249 | Reflect leaves a copy via Copy/Paste | Works | S-105 Reflect Copy About Object |
| Transform/Replicate | 249 | Reflect horizontally or vertically from the dialog | Works | S-103 |
| Reverse Direction | 249 | Reverse a line, arc, open spline or open polyline (start and end swap); walkthrough path direction; molding profile side | Partial | CadMode::ReverseDirection (CAD-54); walkthrough path and molding: NO SPEC -> S-158 |
| Reverse Plan | 249 | Mirror the whole plan | Works | S-115 (Round 15) |
| Transform/Replicate | 250 | Dialog performs Copy, Move, Rotate, Resize, Reflect in that order; copies count (not for roof baselines) | Works | transform.rs "Transform/Replicate Object" |
| Transform/Replicate | 251 | Move by X/Y delta (relative to itself or current point), by X/Y position (absolute), or by angle and distance (relative to its own orientation or absolute); Z delta; fields update each other | Partial | X,Y or distance and angle, no absolute-position mode, no relative-angle option, no Z delta, no current-point reference: NO SPEC -> S-159 |
| Transform/Replicate | 251 | Resize factor (decimal) about center, absolute point or current point; not for lines, arcs, 3D solids | Partial | percent 1 to 1000; about center only: NO SPEC -> S-160 |
| Transform/Replicate | 251 | Reflect horizontally or vertically about center, absolute point, current point | Partial | about X or Y line through the center or a typed offset |

---

## 10. Editing Objects (pp. 252 to 318)

### 10.1 Edit behaviors and selecting (pp. 252 to 262)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Edit behaviors | 252 | Global active behavior, restored to Default at program exit; non-Default shows a pointer badge; each behavior can be summoned temporarily by a mouse button or key | Partial | S-66 (status bar indicator, reset on tool change); badges and temporary activators: NO SPEC -> S-161 |
| Default behavior | 252 | Default: adjust one corner angle without moving neighbours; box objects work like Alternate; move orthogonal to edges; rotation snaps to allowed angles; Alt+Z or Alt+/ summons it | Partial | S-14..S-16; summon key: NO SPEC -> S-162 |
| Alternate behavior | 253 | Continuous drawing; reshape keeping the angle between adjacent edges; end-handle drag switches line/arc; overrides Lock Center; moves at allowed angles; creates alternate-technique camera views; summoned by Alt or the right button | Missing | NO SPEC -> S-163 (our Alternate only locks a move to one axis, `behaviors.rs`) |
| Move behavior | 254 | Move an object with any resize handle; moves at allowed angles; summoned by Z or / | Missing | NO SPEC -> S-164 |
| Resize behavior | 254 | Corner drag scales proportionally (edges handle excluded); summoned by X, period or mouse X2 | Partial | S-65 Resize scales a CAD selection from the opposite corner; summon keys: NO SPEC -> S-165 |
| Concentric behavior | 254 | Every edge moves the same distance; irregular polylines keep no edge ratio; jump increments from Preferences (0 uses the snap unit); summoned by C, Command or mouse X1 | Partial | S-65 Concentric adds offset copies instead of resizing in place; jump setting: NO SPEC -> S-166 |
| Fillet behavior | 256 | Drag a corner handle to round that corner; all box corners at once with equal radius; none for circles/arcs; summoned by F | Partial | S-65, S-67 (polyline corner only) |
| Connect CAD Segments | 256 | Toggle that stops CAD parts joining into polylines; also affects stairs, ramps, roads | Partial | CAD-4 (flag exists); stairs and roads: NO SPEC -> S-167 |
| Rotate/Resize about | 256 | Pivot choice object center or current CAD point; pointer badge | Partial | see section 9 |
| Select Objects | 258 | Space bar or button; click the object or its label; select within 12 px (the snap distance) | Works | S-1, S-3, S-10 |
| Select Objects | 258 | Right-click selects any object whatever tool is active; right-click again deselects by clicking empty space | Partial | S-8; drawing-tool right-click select without using the tool: NO SPEC -> S-168 |
| Select Next Object | 258 | Edit button or Tab cycles overlapping candidates (not for groups) | Works | S-34 |
| Selecting similar | 258 | With a drawing tool active, left-click selects only that tool's object type | Missing | NO SPEC -> S-169 |
| Context menus | 258 | Right-click shows the object's edit tools (preference to disable it) | Works | S-8 |
| Selected edge | 258 | The edge nearest the click becomes the Selected Edge: bigger handle, editable in the spec dialog's Selected Line/Arc panel, movable with dimensions | Partial | S-16 mid-edge handle; selected-edge model in dialogs: Partial (CAD spec Selected Line panel: part 2) |
| Selected edge | 259 | Optional S and E markers at start and end of the selected edge or wall; Select Next Edge button | Missing | NO SPEC -> S-170 |
| Selected side | 259 | In 3D, the clicked side carries the handles; Select Next Side button | Missing | NO SPEC -> S-171 |
| Disconnect Edges | 259 | CAD > Lines > Disconnect Edges: click edges to edit them apart from the polyline; Disconnect Selected Edge button; not for closed items like slabs or roof planes | Missing | NO SPEC -> S-172 |
| Marquee select | 260 | Drag a marquee on empty space with Select Objects (or with Shift/Ctrl) selecting CAD and architectural objects; Ctrl-marquee deselects; Shift-marquee adds | Works | S-29, S-31 |
| Marquee by type | 260 | Hold Shift with a drawing tool active and drag: selects only that tool's object type | Missing | NO SPEC -> S-173 |
| Marquee modes | 260 | Intersected, Contained or By Center; Selection Mode toolbar buttons | Works | PR-10 `select::set_marquee_mode`; menu "Marquee Selection" |
| Marquee Select Similar | 260 | Edit button: marquee selects objects like the current ones; Restrictive Selection toggle (also material, layer); Select All Similar | Partial | S-32 Select Same Type selects all of one type at once; marquee, restrictive toggle: NO SPEC -> S-174 |
| Shift/Ctrl select | 261 | Add or remove one object at a time; also works in browsers, lists and file pickers | Works | S-31 |
| Select All | 261 | All objects on the current floor, elevation, detail window or layout page | Works | S-33 |
| Edit Area | 261 | Edit Area tools define a region and select what is inside (see 10.9) | Works | S-90 (Round 14) |
| Fence Select | 261 | Use a drawn CAD line, arc, polyline or spline as a fence to select every CAD object it touches; fence deselects itself | Missing | NO SPEC -> S-175 |
| Match Properties | 262 | Select rooms, cabinets, windows, doors sharing chosen attributes | Partial | S-116 Match Properties on the Edit toolbar (Round 15, `painters.rs match_properties`) |

### 10.2 Edit handles and toolbars by object family (pp. 262 to 287)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Line-based | 262 | Six handles: Move, two Extend/Change Angle, two Same Line Type (diamond), Rotate; Adjust Width round handles on stairs, roads, footing walls | Partial | S-17, S-18; Same Line Type diamonds: CAD-16 (verify); Adjust Width on stairs: CB-*, roads: Partial |
| Line-based | 263 | Right-drag an end to turn the line into an arc; drag parallel to change length, off-axis to change angle with snaps | Works | S-17, CAD-22 |
| Line-based | 263 | Position with dimensions (not length or width); extension-line snaps | Partial | S-59; extension anchors: see section 7 |
| Line edit toolbar | 264 | Buttons: Select Next Object, Open Object, Copy/Paste, Delete, Transform/Replicate, Multiple Copy, Make Parallel/Perpendicular, Point to Point Move, Center Object, Reflect About Object, View Drawing Group Edit Tools, Add Break, Complete Break, Reverse Direction, Convert Polyline, Revision Cloud(s) Around Objects, Change Line/Arc, Fence Select, Object Layer Properties, Intersect/Join Two Lines, Fillet Lines, Chamfer Lines, Extend Object(s), Trim Object(s), Align/Distribute, Align/Distribute Along Line, Concentric Resize, Set as Default | Partial | S-45, CAD-41: Works for Open, Copy, Delete, Transform, Multiple Copy, Make Parallel/Perpendicular, P2P Move, Center, Reflect, drawing-group tools, Add Break, Reverse Direction, Change Line/Arc, Fillet, Chamfer, Trim, Extend, Align/Distribute; Missing: Complete Break, Revision Cloud Around Objects, Fence Select, Object Layer Properties, Intersect/Join Two Lines, Align Along Line, Concentric Resize button, Set as Default, Select Next Object button (Tab works) |
| Arc-based | 265 | Seven handles: chord center Move, arc center Move, Rotate, Extend/Change Radius ends, Reshape (moves center), Resize (radius), Same Line Type; included angle follows allowed angles | Partial | S-17, CAD-9; arc-center Move handle and Reshape-with-locked-center rules: verify |
| Arc-based | 267 | Dimension a tangent extension line to locate an arc | Partial | DIM-30 |
| Arc edit toolbar | 267 | Adds Convert Curve to Polyline, Lock Center, Make Arc Tangent | Partial | Make Arc Tangent Works (CAD-24); Convert Curve to Polyline (choose side count): Missing; Lock Center: Missing for CAD arcs (walls have it) |
| Lock Center | 269 | Lock an arc's center; locked arcs lengthen/shorten along the curve; no Reshape handle; inside a polyline resizes along the arc | Missing | NO SPEC -> CAD-89 (the wall dialog has a Lock Center field: W-66) |
| Arc Centers and Ends | 269 | View > Arc Centers and Ends | Works | CAD-10 |
| Open polyline | 269 | Handles: Move, Rotate, Reshape at every joint, Extend at the ends, Move Line Segment per straight edge, Move Arc/Resize Arc/Reshape Arc per curved edge, Same Line Type, Adjust Width | Partial | S-16, CAD-16, CAD-20; per-segment Move Line Segment handle and arc handles: Partial |
| Open polyline toolbar | 271 | Adds Disconnect Selected Edge, Add to Library, Close Polyline, Convert to Spline, Convert Curve to Polyline, Lock Center | Partial | Convert to Spline Works (CadMode::ConvertToSpline); Close Polyline: Missing; Disconnect Selected Edge: Missing; Add to Library for CAD: Partial (symbol library only) |
| Close Polyline | 273 | Add an edge joining an open polyline's ends (not available while Connect CAD Segments is on) | Missing | NO SPEC -> CAD-90 (closing happens when drawing, CAD-26) |
| Closed polyline | 273 | Handles: Move, Rotate, Reshape at every vertex, Move Line Segment, arc handles; Adjust Width for stairs | Partial | S-16, CAD-16 |
| Closed polyline toolbar | 275 | Adds Edit Pattern, Union, Intersection, Subtract, Create Hole, Convert to Spline | Partial | Union/Subtract/Intersect Works (Round 15 `cad_ops` UNION/SUBTRACT/INTERSECT; menus "Polyline Union"); Create Hole, Edit Pattern: Missing |
| Polyline holes | 277 | Create Hole button or tool-specific Hole tools / "Hole in" checkbox; hole must lie inside; holes selectable independently; not for molding polylines or rope lights | Partial | slab/room holes exist as objects in their own tools (R-*, CB-*); generic Create Hole for CAD polylines: NO SPEC -> CAD-91 |
| Box-based | 277 | Ten handles (Move, four corner Resize, four Extend, Rotate); extend handle at the click point | Works | S-14, S-15 (Partial: resize anchors) |
| Box toolbar | 279 | Select Next, Open, Copy/Paste, Delete, Transform, Multiple Copy, Make Parallel/Perpendicular, P2P Move, Center, Reflect, Drawing Group tools, Convert Polyline, Revision Cloud Around, Extend, Trim, Align/Distribute, Object Layer Properties, Concentric Resize, Align Along Line | Partial | as the line toolbar |
| Spline-based | 280 | Move, Rotate, diamond Vertex handles, Same Line Type end handles | Works | CAD-29, CAD-30 |
| Advanced Splines | 283 | Control Handles and tangent line at every vertex; Lock Control Handle Angle (default on); Straighten Spline Segment; default advanced mode in Preferences | Missing | NO SPEC -> CAD-92 |
| Spline toolbar | 282 | Adds Convert Spline to Polyline, Advanced Splines, Straighten Spline Segment, Lock Control Handle Angle | Partial | Convert Spline to Polyline: `PolylineToLines`/ConvertToPolyline verify; others Missing |
| Circle/oval/ellipse | 285 | Eleven handles: Move, Rotate (ovals/ellipses only), eight Reshape, Concentric Resize on circles; circles cannot be reshaped | Partial | S-17 circle handles Works; oval and ellipse handles: Partial |
| Circle toolbar | 286 | Convert Curve to Polyline, Trim/Extend (not ellipses), Concentric Resize, Revision Cloud Around | Partial | Trim/Extend circle Works (Round 15); Convert Curve to Polyline Missing |
| Multiple objects | 287 | Dynamic defaults update many objects; shared spec dialog with "No Change"; Edit toolbar for group-selected objects; Object Painter | Partial | S-38 toolbar; shared spec dialog: In progress (W-83); Object Painter: S-116 Partial |

### 10.3 Resizing, reshaping and converting (pp. 288 to 297)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Resizing | 288 | Resize via handles per behavior, about center or current point; dimensions update; Ctrl override; Shift slows; spec dialog (Retain Aspect Ratio); dimensions on edges | Partial | S-14; Retain Aspect Ratio on images/symbols Works; Shift slow-down and Ctrl override: NO SPEC -> S-176 |
| Resizing | 289 | Only walls resize via dimensions; other objects move an adjacent edge instead | Works | S-59 |
| Concentric Resize | 289 | Edit button: resize so every edge moves the same distance; Sticky Mode; Set Concentric Jump dialog (global, same as Preferences) | Missing | NO SPEC -> CAD-93 (Concentric is an Edit Behavior that offsets copies, not this tool) |
| Reshaping | 289 | Right-drag end handle toggles line/arc (not closed polylines); Ctrl override; Shift slow | Works | CAD-22 |
| Reshaping | 290 | Angular and temporary dimensions reshape; Angular Dimension sets the angle where two segments meet | Partial | S-59; angular value edit: NO SPEC -> CAD-94 |
| Add Break | 290 | Partial break adds a corner/pivot (not for boxes, circles, north pointers, sun angles, joist direction lines, trusses); key 3; Sticky Mode; Edge/Corner Edit Handles toggles; drag a neighbouring handle to move the edge; remove by dragging a corner onto its neighbour with Endpoint snap | Partial | S-50 Break Line/Wall; CAD-21 Add Break and Delete Break; Sticky Mode, key 3 and handle toggles: NO SPEC -> CAD-95 |
| Complete Break | 290 | Sever an edge into two objects (only complete breaks for framing members); break at snap points | Missing | NO SPEC -> CAD-96 |
| Change Line/Arc | 291 | Edit button toggles a segment between straight and arc | Works | CAD-22 |
| Intersect/Join Two Lines | 291 | Join two non-parallel lines/arcs, or two edges of a polyline, extending or trimming and removing in-between edges | Missing | NO SPEC -> CAD-97 (Trim to Boundary and Extend to Boundary exist but do not join) |
| Fillet Lines | 292 | Set Fillet Radius dialog (0 means none), Fillet All Corners of a polyline (custom countertop and molding corners at walls excluded), Sticky Mode; joins two lines with an arc; arcs extend/contract instead | Partial | CAD-54 Fillet (`CadMode::Fillet`, radius in the option strip); Fillet All Corners, Sticky Mode: NO SPEC -> CAD-98 |
| Chamfer Lines | 292 | Chamfer Distance dialog (distance on both lines, not the bevel length); extends arcs to join; same use as Intersect/Join | Partial | CadMode::Chamfer; dialog and arc join: NO SPEC -> CAD-99 |
| Close Polyline | 293 | See above | Missing | NO SPEC -> CAD-100 |
| Simplify Polyline | 293 | Merge collinear edges and drop very short segments | Missing | NO SPEC -> CAD-101 |
| Convert Curve to Polyline | 293 | Turn an arc or circle into a polyline with a chosen side count (not for walls or stairs) | Missing | NO SPEC -> CAD-102 |
| Convert to Spline | 293 | CAD polylines, most 3D solids, slabs to splines (no arcs or lines) | Works | CadMode::ConvertToSpline (CAD-29) |
| Union, Intersection, Subtract | 293 | Boolean edit tools | Works | see 10.8 |
| CAD to Walls | 294 | Convert double CAD lines/arcs to walls, railings, windows, doors | Works | exchange.rs CAD to Walls (CAD-28) |
| CAD Detail from View | 294 | Create a CAD drawing of the current view | Works | CAD-35, tools/details.rs |
| Convert Polyline | 294 | Dialog listing polyline types: CAD (Plain Polyline, Construction Line, Revision Cloud), Architectural (3D Solid, 3D Solid Hole, Backsplash, Countertop, Face, Hole in Ceiling Platform, Hole in Floor Platform, Hole in Roof/Custom Ceiling, Landing, Skylight, Slab, Slab with Footing, Tray Ceiling, Molding Polyline), Terrain (Elevation Line/Region, Terrain Break, Garden Bed, Grass Region, Flat Region, Hill/Valley, Raised/Lowered Region, Terrain Wall, Terrain Curb, Sprinkler Line, Terrain Feature, Terrain Perimeter), Roads (Road perimeter/center line, Median, Marking, Stripe, Driveway, Sidewalk each as perimeter or center line), Other (Walkthrough Path, Material Region, Materials List Polyline, Revision Cloud, Construction Line) | Partial | `tools/cad/edit.rs` and `cad.rs` Convert Polyline cover slab, countertop, room, wall and a few more (CAD-28, CB-*, T-*); the full type list and a single dialog: NO SPEC -> CAD-103. Types missing include Construction Line (tool absent), 3D Solid/Hole/Face, Landing, Skylight, Tray Ceiling, Molding Polyline conversion, Road/Driveway/Sidewalk center-line forms, Material Region, Materials List Polyline: verify each |
| Convert Polyline | 297 | Retain Original Polyline check box; layer options (default for the new type, same layer, specified layer with Define) | Missing | NO SPEC -> CAD-104 |
| Convert to Solid | 297 | Turn a selected object into a 3D Solid | Missing | NO SPEC -> CAD-105 (3D solids are part 3; Default Settings page 3D Solid exists: DS-1) |
| Convert to Symbol | 297 | Tools > Symbol > Convert to Symbol from a 3D view of a custom object | Missing | NO SPEC -> CAD-106 (symbols come from import and Add to Library; no 3D-view-to-symbol) |
| Convert Spline to Polyline | 297 | Spline back into straight segments | Partial | CadMode::PolylineToLines/ConvertToPolyline verify |

### 10.4 Edit Area, Stretch, Trim, Boolean (pp. 298 to 305)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Edit Area | 298 | Edit > Edit Area tools define a rectangle and select inside it, cutting walls, railings, fences where the marquee crosses; cabinets count when more than half is inside; cameras and elevation symbols go along; CAD points not | Works | S-90 (Round 14, `tools/select/area.rs`; verify in Chief) |
| Edit Area variants | 298 | Edit Area (current floor, all objects), Edit Area Visible, Edit Area (All Floors), Edit Area (All Floors) Visible | Partial | Edit Area and Edit Area Visible only (menus.rs); both all-floors forms: NO SPEC -> S-177 |
| Edit Area polyline | 299 | A closed polyline used as the area marquee | Missing | NO SPEC -> S-178 |
| Edit Area handles | 299 | Move, Rotate, Reshape/Resize handles on the marquee; toolbar Copy/Paste, Delete, Transform, Multiple Copy (no concentric), Make Parallel, P2P Move, Center, Reflect | Partial | area.rs move and rotate; the toolbar subset: Partial |
| Allowed Angles | 300 | Place at Allowed Angles dialog when more than 1 percent of straight walls in the area are off-angle: rotate the largest off-angle group onto an allowed angle, or add the angles to the allowed list, or do nothing | Missing | NO SPEC -> S-179 |
| Stretch CAD | 300 | Draw a rectangular polyline through the CAD objects to stretch (edit its shape; one edge through the objects, the opposite outside), drag its Move handle; roads and text only move; stairs, ramps, sun-shadow polylines unaffected | Works | S-91 (Round 14, `area.rs stretch_cad`): lines and polylines; arcs, circles and boxes: Partial |
| Trim Objects | 301 | Edit button: cutting object can be any CAD-based object, group or block; trim lines, arcs, open polylines, circles and framing by clicking, by a drawn fence, or by a temporary fence line; Sticky Mode; Select Fence | Partial | Round 15 `cad_ops.rs` Trim to Boundary: click the boundary, then click objects (lines, polylines, arcs, circles); fence and temporary-fence variants, Sticky Mode and framing items: NO SPEC -> S-180 |
| Extend Objects | 302 | Same three methods for lines, arcs, open polylines, framing | Partial | Round 15 Extend to Boundary; fence variants: NO SPEC -> S-181 |
| Trim/Extend limits | 301 | Not usable on closed polylines | Works | cad_ops.rs |
| Union | 303 | Combine two or more closed polylines or solids (single or group selection); result type follows the originals (same type, CAD polyline, or 3D Solid); keep or delete originals prompt; not for holes | Works | Round 15 `cad_ops.rs` UNION through `plan_core::clip`; the keep-or-delete prompt and the mixed-type to 3D Solid rule: NO SPEC -> S-182 |
| Intersection | 304 | Shared area of overlapping polylines/solids | Works | cad_ops.rs INTERSECT |
| Subtraction | 305 | Remove the area of one object from another | Works | cad_ops.rs SUBTRACT |

### 10.5 Eyedropper and painters, style palettes, matching (pp. 305 to 312)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Eyedroppers | 305 | Four eyedroppers (Material, Object, Fill Style, Layer) each load attributes into the matching painter; cursor shows an eyedropper then a spray can; ignore locked and hidden layers; touching auto-generated roofs or framing turns Auto Rebuild off | Partial | Material Painter Works (C-55..C-61); Layer and Object Eyedropper/Painter Round 15 (S-116, LAY-18); Fill Style: Missing (CAD-56); locked-layer rule Works in painters.rs |
| Eyedroppers | 306 | Edit toolbar options while painting: scoping buttons, Blend Colors with Materials, Select Properties to Paint, Library Replacement Mode | Partial | materials painter has scope and blend; Replace From Library mode: NO SPEC -> S-183 |
| Painters | 307 | Load a painter from the Library Browser selection (material, fill style, style palette), by choosing the tool (Select Material/Library Object/Layer dialogs) or via an eyedropper | Partial | materials from the library Works; fill style and style palette: Missing |
| Scoping modes | 307 | Object, Marquee (plan; style palette, fill style, object painter), Room, Floor, Plan, plus Component for materials; remembered until quit | Partial | materials: Component, Object, Room, Floor, Plan, Blend (C-56); Object Painter modes dialog Component/Object/Room/Floor/Plan (S-116); Marquee mode: NO SPEC -> S-184 |
| Style Palettes | 308 | Library items collecting door, window, cabinet, countertop, backsplash and room properties (also inside architectural blocks); Style Painter applies them with the scoping modes in plan or camera views | Missing | NO SPEC -> S-185 |
| Style Palettes | 308 | Create from the Add Object(s) to Style Palette edit button, from User Catalog > New > Style Palette, or by copying one | Missing | NO SPEC -> S-186 |
| Style Palette Spec | 309 | Name; table of objects with Open Object, Use Object and Set Properties columns and delete; add object by type; "only apply to doors of the same type"; preview | Missing | NO SPEC -> S-187 |
| Object Eyedropper | 310 | Tools > Object Painter > Object Eyedropper; Select Properties to Paint; scoping | Partial | S-116 |
| Match Properties | 310 | Select every object sharing chosen attributes (plan view and CAD detail only) | Works | S-116 `match_properties` |
| Apply Properties | 311 | After matching, apply the attributes to other objects by click or marquee | Partial | painters.rs apply; marquee form: NO SPEC -> S-188 |
| Select Properties to Load | 311 | Dialog with search (Found vs Other Properties), property/value list with check boxes, Select All/Clear All, Select/Clear Transferrable, Select/Clear Found, Reset to Default (in the palette's Set Properties), asterisk for select-only properties | Partial | `dialogs/painters.rs` Object Painter Modes with a list of attributes; search, transferrable and found buttons: NO SPEC -> S-189 |
| Set as Default | 310 | Edit button copying a spec into the defaults | Missing | see section 4 |

### 10.6 Deleting, undo and redo (pp. 312 to 318)

| Section | Page | Feature | Status | Evidence |
|---|---|---|---|---|
| Deleting | 312 | Delete via Delete edit button, Edit > Delete, Delete or Backspace | Works | S-87, S-93 |
| Deleting | 313 | Whole categories via Delete Objects; objects on a deleted floor go with it; an attached arrow line goes with its object; auto-generated roofs, framing, Auto Exterior Dimensions cannot be deleted while auto rebuild is on | Partial | S-87, S-88; arrow cascade and the auto-rebuild guard: NO SPEC -> S-190 |
| Delete polyline edges | 313 | Delete an edge by disconnect+delete, by dragging a corner onto its neighbour, by complete breaks, or via Fillet/Chamfer joins | Missing | NO SPEC -> S-191 (CAD-21 Delete Break removes a vertex) |
| Delete Surface | 313 | 3D > Delete Surface | Works | menus.rs |
| Delete Objects | 313 | Dialog opened from Edit > Delete Objects, plan version also reachable in plan view only | Works | S-88 (Shift+Space) |
| Delete Objects | 314 | Scope: All Floors, All Rooms on This Floor, or Single Room (click rooms with the dialog open, objects whose center lies in the room); Select All / Clear All | Partial | `delete_objects.rs`: this floor or every floor; room scopes: NO SPEC -> S-192 |
| Delete Objects | 314 | Categories grouped: Objects, CAD, Framing, Schedules, Walls with sub-categories and group tick boxes | Partial | 16 flat categories (Walls, Doors, Windows, Cabinets, Fixtures, Stairs, Dimensions, CAD, Text, Roof Planes, Electrical, Foundation, Framing, Details, Schedules, Cameras); no groups or finer subcategories (wall kinds, framing kinds) |
| Delete Objects | 315 | Objects on locked layers can still be deleted through this dialog | Differs | ours leaves objects on hidden and locked layers alone (delete_objects.rs, S-89); Chief's rule is the opposite for locked ones, DECISIONS note needed |
| Delete Objects (layout) | 316 | In a layout: current page or all pages with a page range and template/non-template page filter; layout object types; page information to clear | Missing | NO SPEC -> S-193 |
| Undo/Redo | 316 | Undo and Redo (Ctrl+Z / Ctrl+Y), 50 actions by default, user level 1 to 100, including browser Delete/Rename/Copy | Works | S-75, S-76, S-78 (Differs: levels) |
| Undo/Redo | 317 | Not undoable: save, library changes, edits inside an open dialog, light adjustments, note type management, time log, pan/zoom, camera moves and technique changes | Partial | pan/zoom and camera moves are outside the stack (LAY-26); dialog edits are applied at OK as one step (S-80) |
| Action History | 317 | Side window listing undoable actions with tool icons and descriptions; newest on top, sort toggle; click an entry to jump back or forward; length tied to Maximum Undos | Works | S-79 (`action_history.rs`, jump to an entry; sort toggle: verify) |

---

## Dialog panels

Every specification, defaults and utility dialog in pages 11 to 318. "Ours" names the Plan Studio window (`crates/plan-app/src/dialogs/...` unless stated). Fields are named as Chief names them.

### A. Preferences dialog (manual pp. 123 to 156): 25 Chief panels, 15 ours

Plan Studio page list (`Page::ALL`): Appearance, Colors, Fonts (Chief's Text), Library Browser, Render, Materials List, Reset Options, Folders, Edit, Behaviors, Snap Properties, Architectural, CAD, General Plan Defaults (a link), Unit Conversions. Panels with no counterpart: Dialogs/Side Windows, Project Browser, Pattern Editor, General, File Management, Project Management, Ruby, New Plans, Time Tracker, Coordinate System, Master List (we have Materials List), Video Card Status.

| Chief panel | Chief fields | Ours has | Missing fields |
|---|---|---|---|
| Appearance (p. 123) | Contextual menus on/off, click twice to display; Status Bar visibility and items (Active Status, Floor Level/Layout Page, Object Layer, Object Drawing Group, Coordinates, View Window Size, Render Status, Screen Redraw Time); Show Line Weights and Minimum Display Weight; Display in Color when Possible (pictures); Show Icons in menus; Color Off Is (Black and White or Grayscale); Child Tool Palette or Drop Down toolbars; Scale Toolbar Icons for High DPI; Button Size; Minimum Display Size of temporary dimensions and labels | text size, icon halo, low-glare switches, icon size, status bar and toolbars on/off | everything except status bar on/off and icon size: contextual-menu switches, status items, line weight display and minimum, pictures in color, menu icons, color-off choice, toolbar style, button size in px, minimum display sizes |
| Colors (p. 126) | Plan/Detail, Layout, Preview backgrounds; Selection Line and Fill with opacity; Handle Fill, Secondary Handle Fill, Selected Edge Handle Fill; Snap Grid and Reference Grid colors; Reset; Interface Colors System/Force Light/Force Dark/Custom Color Theme with a theme table (copy, rename, delete) and per-element colors incl. Toolbar Button/Image Backgrounds | plan background, grid, text, temporary dimension, selection colors; canvas themes in the View menu | Layout and Preview backgrounds, selection fill and opacity, handle fills (3 kinds), snap grid vs reference grid colors, custom interface themes with copy/rename/delete |
| Dialogs/Side Windows (p. 128) | Save Dialog Size and Position (Always/Per Session/Never); Open Dialogs to Last Panel; Dialog Previews Standard or Vector; Material previews (Physically Based on launch, blurred backdrop); Side Window Drag Docking; Top/Bottom docking per window | none | whole panel |
| Library Browser (p. 129) | Include Web Results; Enable Filtering; rename behavior (double-click, single-click, none); Show Names in Tile Mode; Double-Click Closes Browser; Preview technique; Lock Panels; Section Title Bars; Reset Panel Layout | preview size, search options (names, descriptions, keywords, catalog names, whole words, match all) | web results, filtering switch, rename behavior, tile names, double-click closes, preview technique, panel lock/title bars/reset |
| Project Browser (p. 130) | Lock Panels, Visible Title Bars, Reset Panel Layout, Only Show Active Projects, Show Default Documents, Expand Plans and Layouts On Open | none | whole panel |
| Text (p. 131) | Enter creates a new line; leader line segments; leader lines create Rich Text | fonts list, links to text styles | the three settings |
| Pattern Editor (p. 131) | Pattern preview color and transparency; repeat box color and weight | none | whole panel (no custom patterns) |
| General (p. 132) | Undo on/off and Maximum Undos; update check and frequency; Startup (Dashboard or New Plan); Dashboard behaviors; error reports; Record Timing Log; Highlight Overridden Dimension Text; Pinch Zoom Sensitivity; Mac mouse optimizations and scroll sensitivity; Edit Defaults on Double-Click; Edit Active Default on Double-Click | none (undo cap 100 is a code constant, DS3) | whole panel |
| File Management (p. 134) | Auto Save and minutes; File Locking; recent document count, submenu or bottom of menu, clear pinned, clear recent; thumbnails and size; Auto Archive Hourly/Daily/Previous Save; archive warning and maximum files; copy referenced material files to data folder; associate files | autosave minutes and keep-N exist in `settings.json` (`files.rs FileSettings`) with no page | whole panel as a page; file locking, recents options, thumbnails, archive mode, warning, material copy, association |
| Folders (p. 135) | Data folder (or Unmanaged Data Folder); user library folders and "keep with data folder"; system library database folder; All Program Paths dialog | textures, backdrops, templates, autosave, user library, with found marks | data folder move, system library folder, Program Paths list |
| Project Management (p. 137) | Use Project Management; Back Up Managed Resources with location, frequency (hours or days and time), remind on exit, retention; local storage; Migrate Forward Legacy Projects | none | Differs for the store; the backup schedule is a candidate |
| Ruby (p. 137) | safe level, $LOAD_PATH | none | Out-of-scope |
| New Plans (p. 138) | U.S. or Metric units; plan and layout template per system (select/import/browse/edit path); hide other-unit templates; Open and Save As folder rule (last or fixed); default Designer and Client information (Define) and copy-from-template switches | Templates page exists in the Default Settings dialog | unit choice for new plans, per-unit templates, folder rule, global designer/client defaults |
| Unit Conversions (p. 139) | table of units with Add, Edit, Delete, Copy; locked built-ins; sample converter; Add Unit Conversion dialog (name, Default Unit, Length/Area/Volume, multiply-by with unit, sample) | a converter of one length into all units | custom units and the table |
| Time Tracker (p. 142) | auto start, stop after idle minutes, idle timeout dialog, default user name | none | whole panel |
| Architectural (p. 142) | Show Same Wall Type Handles; Select Room Before Wall in 3D; Stair Sections Move Independently; Skylights generate ceiling holes or manual polylines; legacy opening indicators | cabinet countertop join and gap fit, auto rebuild roofs/walls/foundations/attic walls, delete unused roof planes | the five Chief switches |
| CAD (p. 143) | Always use By Object for CAD block fill; By Object for new blocks; Connect CAD Segments; Advanced Splines; Endcap Printed Length; Show Same Line Type Handles; Offset from Drawing Surface for 3D annotation | arc centers, end caps, line weights table | block fill switches, Advanced Splines, same-line-type handle switch, 3D annotation offset |
| Edit (p. 144) | Crosshairs (plan/elevation, perspective, synchronize, color, width, size, aperture); Edit Handle Size and Tolerance; Show Start and End Indicators and size; Marquee Selection mode; Automatic View Scrolling with 2D and 3D speeds | rotate about, resize about, marquee mode, snap switches | crosshair block, handle size and tolerance, start/end indicators, scroll switch and speeds |
| Coordinate System (p. 146) | axes and origin indicators per view family; axis colors, grid color, line length, label height; origin color and draw order; sun/moon indicator and size | none | whole panel |
| Behaviors (p. 148) | Rotate/Resize About; Rotate Jump; Find Angles for Center and Reflect; Primary Movement orthogonal or polar; Stop When Connected; Edit Type; Concentric Jump; Behavior Indicators | Edit Type, concentric/fillet/chamfer values, Replicate copies and dialog, resize proportional, camera steps | Rotate Jump, Find Angles, Primary Movement method, Stop When Connected, Behavior Indicators |
| Snap Properties (p. 149) | Objects in History; Snap Distance; Maximum Bump Distance; Snap Cabinets After Paste; Always Snap Walls on Allowed Angles; Indicator Size; Indicator, Extension and Angle Snap Grid colors; Display Angle Snap Grid; Object Snaps (Endpoint, Midpoint, Center, Quadrant, On Object, Points/Markers, Intersection) and Extension Snaps (Tangent, Perpendicular, Orthogonal); Bumping/Pushing; Bumping/Pushing for Type-in Movement; Dimension Line Separation Snaps; Angle Snaps; Angle Increment Override | all snap kinds incl. Perpendicular/Tangent/Extension, angle increment and allowed angles, sensitivity, bumping and distance | Objects in History, Snap Cabinets After Paste, Always Snap Walls on Allowed Angles, indicator size/colors, Angle Snap Grid, Orthogonal extension, type-in bumping, dimension separation snaps, increment override |
| Master List (p. 153) | master materials list file (select/import/browse/path) and New | waste, rounding, prices, floors | the master list file |
| Render (p. 153) | Horizon Lines; 3D mouse/gamepad navigation center; Cycle Render Techniques list with order and Reset; GPU ray tracing and diagnostics; GPU residency; fractional samples; Mac retina options | ray-trace size, samples, clay start, latitude, preview quality, shadows, AO, PBR maps, max texture side | horizon lines, cycle-technique list; GPU items are Out-of-scope |
| Video Card Status (p. 155) | read-only video card information | System Information (APP-49) | Differs |
| Reset Options (p. 155) | Reset Message Boxes, Dialog Sizes, Toolbars, Templates, Side Windows, Search Folders, Migration, Preferences | toolbars, dialog sizes, messages, side windows, each with a second click | templates, search folders, migration, whole-preferences reset |

### B. Defaults, saved defaults and sets

| Dialog (page) | Chief panels and fields | Ours | Missing |
|---|---|---|---|
| Default Settings (p. 102) | search field; tree; Edit button; group-edit of similar leaves | `defaults.rs` tree of 29 groups with search (DS-1..DS-21) | multi-select group edit; double-click from toolbar buttons; hotkeys per dialog |
| General Plan Defaults, General panel (p. 120) | Warn Before Deleting; Ignore Casing When Resizing Wall Openings; Show Pitch as Degrees; Arrow Key Scroll Distance; Framing direction (parallel/perpendicular), Draw Joists etc. as Lines; Living Area (Main Layer or Surface, Suppress Label); Opening Indicators (Hinge Indicators Point to Handle); Geographic Location (latitude, longitude, time zone); Dimensions (Grade Level Marker height, Elevation Reference) | units, reference grid spacing, grid snap, angle snap increment, snap distance, bumping and distance, undo levels (stored only), wall join options | every field of Chief's General panel (the units, grid and snap items we show belong to Chief's Angle/Grid panel and to Preferences) |
| General Plan Defaults, Angle/Grid panel (p. 122) | Allowed Angles increment, Additional Angles table with opposing angles, Number Style; Snap Grid (use, unit, show, dots); Reference Grid (show, size, dots) | increment, allowed angles list (Snap Settings), grid snap on/off and unit, reference grid spacing | opposing-angle table, Number Style button, Show Snap Grid, dots options |
| General Layout Defaults (p. 119) | reduced set (no arrow-key scroll, framing, living area, opening indicators, geographic location, dimensions) | Layout page in Default Settings (page size, margin, box border, text style) | snap/reference grid for the layout, Warn Before Deleting |
| Saved Defaults (p. 105) | Available Saved Defaults: Edit, Copy, Rename, Delete, Select All, Clear All; Currently Active drop-down with Edit | `DimensionSetsDialog` (dimensions only) | same dialog for Text, Rich Text, Callouts, Markers, Arrows, Revision Clouds, Room Functions, Framing Types |
| Active Defaults (p. 106) | Default Set selector, Save New, Edit/Rename Set; Selected Defaults per kind with Add/Edit/Rename/Delete; Layers (layer set, current CAD layer with Define) | opens the Default Settings tree only | whole dialog |
| Default Sets (p. 109) | list, New, Delete; Name; Selected Defaults per kind; Layers | Default Sets page (stored values) | activation, toolbar control, per-view memory |
| New Default Name / Rename (p. 106) | name prompts, unique names | inline fields in the dimension dialog | for other kinds |
| Reset to Defaults (p. 118) | scope (current floor or all floors); Floor and Ceiling Heights; Roof Groups; Roof Directives in Walls; Roof Gable Lines (Delete); Wall Top Heights and Bottom Heights; Wall Auto Connections; Overridden Dimension Text (Entire Plan) | Reset to Template (resets the defaults tree) | the plan-side reset dialog |
| Import Default Settings (p. 114) | category tree (Multiple Saved Defaults, Default Settings, Layer Sets, Note Types, Saved Plan Views, Wall Types); Replace Originals or Rename Imported | Layer Set import from a plan file; Chief template import summary | the dialog and the other categories |
| Import Layer Sets / Default Sets / Wall Definitions / Note Types (pp. 116 to 118) | file pickers and conflict options | none | whole dialogs |
| Plan Template (Save as Template) (pp. 112, 113) | purge checklist; set as default per unit system; open new plan after | saves the defaults only | checklist, flags |
| Dialog Number/Angle Style (p. 156) | distance format (fractional inches/feet, decimal, metric), angle style (degrees, minutes, seconds, quadrant bearing, azimuth, pitch), accuracy | length format in Dimension Defaults | the global dialog and bearing/azimuth/pitch styles |
| Add Unit Conversion (p. 141) | name, default flag, measurement type, multiply by, sample | none | whole dialog |

### C. Views, layers, reference display, planning

| Dialog (page) | Chief panels and fields | Ours | Missing |
|---|---|---|---|
| Plan View Specification (p. 179) | General (name, Saved, Floor, Remember Zoom/Rotation, Show Color, Show Watermark, Link to Layout, Poché, pony wall options, Save Options Prompt/Always/Never); Selected Defaults; Reference Display | Name, Layer Set, Floor, Reference Display and Reference Floor, Dimension Defaults, Text Style, Zoom (`plan_views.rs`) | Saved flag, remember zoom/rotation, color, watermark, link to layout, poché, pony walls, save options; Selected Defaults list |
| New Saved Plan View (p. 178) | name; Copy Layer Set with a new name | New and Duplicate buttons | the Copy Layer Set option and name |
| Rotate Plan View (p. 178) | typed angle in degrees (relative to the original, shown -180..180) | 90 left / 90 right / north up | typed angle |
| Change Floor/Reference (p. 89) | Current Floor list; Show Reference Floor(s); reference table with plan file, floor, layer set + Define, Details, XOR, offsets X/Y/Z and angle; Insert Above/Below, Move Up/Down, Delete | `reference_display.rs`: show, floor below/above/fixed, layer set, color | table of several rows, other plan files, Details, XOR, offsets, row management |
| Reference rendering options (p. 91) | per-row technique (Standard, Glass House, Technical Illustration, Vector View) with Reference Rendering Technique Options | none | Missing (camera views) |
| Select Reference Document to Edit (p. 91) | list of referenced plans | none | Missing |
| Layer Display Options (p. 208) | layer set bar (Copy Set, Modify All, Reset Layer Names, Delete Unused Layers); table (Name filter, Used, Disp, Lock, Color, Fill, Line Style, Line Weight, Text Style); Select All, New, Copy, Merge, Delete, Reset Names; Properties for Selected (Display, Lock, Color, Line Weight, Line Style, Text Style with Define, Fill Style) | `layer_display.rs`: set selector with New/Copy/Rename/Delete, Modify All Layer Sets, name filter, Select All/None, Reset, Select Objects; columns Name, Used, Disp, Lock, Ref, Color, Weight, Line Style, Text Style; properties; Copy To Other Sets | Fill column/property, New/Copy/Merge/Delete layer, Reset Layer Names, Delete Unused Layers |
| Layer Set Defaults (p. 213) | initial layer set for nine view kinds with Define, "Use Active Layer Set" | none | whole dialog |
| Layer Set Management (p. 214) | Available Layer Sets (Define, New, Copy, Rename, Delete) and Active Layer Set for current view | `layer_sets.rs` (New, Copy, Rename, Delete, Make Active, Import From Plan File) | Works |
| Select Layer (p. 215) | table, filter, properties, Use Default Layer | Layer Painter bar picks a layer | shared dialog, Use Default Layer |
| Object Layer Properties (p. 216) | Name Filter; table of the object's layers; Show All Layers; properties for selected | none | whole dialog |
| Active Layer Display Options window (p. 211) | Options: Layer Set Control, Layer Management, Layer Name Filter, Layer Edit Buttons, Layer Properties, Show All Layers, Columns | dock with filter and set controls | Options menu items, Columns, selection-driven mode |
| Select Location (p. 93) | locations list with counts for layers, schedules, objects | none | whole dialog |
| Plan Check (p. 94) | error counter, description, Next, Previous, Hold, Done | Plan Check window with Previous, Next, Zoom to, Ignore, Settings, report export | Works (richer than Chief) |
| Time Log (p. 78) | entries table, Add/Delete/Export Selected/Print Selected, Select All/Clear All, total duration, selected entry (user, start, end, duration, notes) | none | whole dialog |
| Loan Calculator (p. 95) | Calculate, Result, Required (loan amount, term, rate, monthly payment), Optional (taxes, insurance, PMI, other fees), Reset | none | whole dialog |
| Create Plan Database / Edit Plan Database / Find Plan Assistant (pp. 96 to 100) | database file, search path, subfolders, relative path; plan list with style, price, area, beds, baths, floors, description; assistant pages House Style, House Size, Plan Details | none | whole dialogs |
| Construction Line Specification (p. 85) | Construction Line panel (infinite in plan/elevation, display on all floors, include in automatic ordering, Define Rules); Callouts panel; Line Style panel; Text Style panel | none | whole dialog and Order Management dialog |
| Room Box Specification (p. 82) | General (room name, function), Line Style, Fill Style | none | whole dialog |
| Space Planning Assistant (p. 82) | gathers rooms and options; Finish | questionnaire window | Works |

### D. Files, projects, assets

| Dialog (page) | Chief fields | Ours | Missing |
|---|---|---|---|
| Dashboard (p. 11) | New/Open, Recent Documents with pins, Getting Started and Resources, Announcements, My Account, Build and Key | none | Missing |
| New Project from Template (p. 37) | Project Name, units, Include Plan + template, Include Layout + template, Folders | none | Differs/Missing |
| Select Project dialogs (p. 40) | Search, Projects list, New Project, Select/Open | none | Differs |
| Import to Project / Additional Import Selection (p. 42) | new or existing project or keep unmanaged; list with Select All/Clear All | none | Differs |
| Asset Management (p. 44) | filter (text, owner, location), history buttons, asset table or tiles, Import New, Insert, View, View With, Rename, Move to Project Browser, Replace With, Export, Delete, Merge, Merge All Identical, Settings columns | none | Missing |
| Export Managed Resources (p. 45) | filter, list, destination | none | Missing |
| Export Project Options (p. 65) | table of plans, layouts, files with Include File check; Select All/Deselect All | Backup Entire Plan (zip) | Missing as a dialog |
| Import Project (p. 67) | destination folder and asset location | none | Missing |
| Select Template (p. 49) | Load Installed Template or Load Custom Template | none | Missing |
| Missing Files (p. 50) | table (resource, usage, owner, object, location, in use, path) and eight resolving buttons | none | Missing |
| Referenced Plans/Layouts (p. 51) | Files Present, Files Not Found, Browse or Replace Reference | none | Differs |
| Link View (p. 52) | open files tree, preview, Show/Hide Preview, Open Plan, Keep Layout Box Contents' Position | Layout Box Specification link choice | partly |
| Advanced Search (p. 62) | text, project folder, modified/created before/after, size larger/smaller, selected parameters | none | Missing |
| Project Browser panels (p. 54) | filters, Project Folders/Tags, Projects tree, Details/Notes, Preview, Toolbar | Project dock (tree of views) | most panels |

### E. Editing dialogs

| Dialog (page) | Chief fields | Ours | Missing |
|---|---|---|---|
| Transform/Replicate Object (p. 250) | Copy (count); Move (X/Y Delta, X/Y Position, Angle and Distance with Relative or Absolute Angle, Z Delta, reference Itself/Current Point/Absolute); Rotate (Absolute or Relative Angle); Resize (factor); Reflect (Horizontally or Vertically); About (Object Center, Absolute Point with X/Y, Current Point) | `transform.rs`: Make copies and count; Move by X/Y or distance and angle; Rotate degrees about center or a point; Resize percent; Reflect about X or Y line | Z delta, absolute position mode, relative/absolute angle switch, current-point reference, resize/reflect about a point |
| Multiple Copy (p. 203) | Offset Between Copies When Dragging or Evenly Distribute Copies; per-class Primary and Secondary offsets (general, trusses, rafters, joists/posts/beams, studs); Rotation of All Objects; Primary and Secondary Number of Copies | `multiple_copy.rs`: X/Y or distance/angle, step or total, turn per copy, Drag in Plan | per-class intervals, secondary offsets/counts |
| Align/Distribute Objects (p. 241) | Move Objects Vertically (Don't Move, Top Edges, Centers, Bottom Edges, Space Evenly, Distribute Centers Evenly); Move Objects Horizontally (Don't Move, Left Edges, Centers, Right Edges, Space Evenly, Distribute Centers Evenly) | align mode buttons and equal-gap distribute | Distribute Centers Evenly; combined two-axis dialog |
| Align To Line (p. 243) | Alignment (None, Centers, Closest Edges); Distribution (None, Space Evenly, Distribute Centers Evenly); Distribute To / Between Endpoints | none | whole dialog |
| Radius of Tangent Arc (p. 245) | radius field | none | whole dialog |
| Make Parallel (p. 248) | Rotate Selected Edge Only or Rotate Entire Polyline | none | whole dialog |
| Set Angular Dimension (p. 247) | previous value, new value, Rotate Edge or Rotate entire polyline | none | whole dialog |
| Set Fillet Radius / Chamfer Distance / Set Concentric Distance (pp. 292, 289) | one distance each | values in the CAD option strip and Edit Behaviors dialog | as separate prompts |
| Convert Curve to Polyline (p. 293) | number of sides | none | whole dialog |
| Convert Polyline (p. 294) | polyline types (CAD, Architectural, Terrain, Roads, Other); Retain Original; layer options (Default, Same, Specify with Define) | commands per target | the dialog |
| Place at Allowed Angles (p. 300) | three options | none | whole dialog |
| Paste Image / Paste Special (pp. 199, 202) | Screen-capture-like options; representation list | Paste Special submenu | Paste Image dialog |
| Select Properties to Load / Match Properties (p. 311) | search, Properties list, Select/Clear Transferrable, Select/Clear Found | Object Painter Modes dialog | search and the four buttons |
| Style Palette Specification (p. 309) | name, objects table (Open Object, Use Object, Set Properties, Delete), Add Object, same-type door switch, preview | none | whole dialog |
| Delete Objects, plan (p. 313) | Delete Scope (All Floors, All Rooms on This Floor, Single Room), Select/Clear All, Objects, CAD, Framing, Schedules, Walls groups | `delete_objects.rs`: 16 categories, this floor or every floor | room scopes, grouped categories, finer wall kinds |
| Delete Objects, layout (p. 316) | current page or all pages, range, template/non-template pages, layout objects, page information | none | whole dialog |
| Fill Style Specification (p. 223) | name, pattern type (Use Layer, Library, system), scale (row offset, width, height, X/Y scale), offsets and angle, line weight, color source and transparency or gradient, background, Add to Library, preview width | CAD fill: color, opacity, pattern name, spacing, angle | almost all |
| Fill Gradients (p. 222) | type linear/radial, angle, color points (color, transparency, position), Add/Remove/Reset, background and view options | none | whole dialog |
| Pattern Specification (p. 227) | name, width, height of repeat box, horizontal and vertical shift, preview | none | whole dialog and Pattern window |
| Infinite Pattern Line (p. 229) | X/Y position, angle, distance between lines | none | whole dialog |
| Line Style Management (p. 230) | table with Used; Edit, New, Copy, Move Up/Down, Purge, Delete, Merge, Add to Library; Auto Purge | none | whole dialog |
| Line Style Specification (p. 232) | name, preview with highlight/mark repeats, Dash/Dot/Text components, order, length, spacing, text, text height | none | whole dialog |
| Select Color (p. 234) | basic colors, spectrum, HSL/RGB, alpha/opacity, hex, custom colors, screen eyedropper, Add to Custom Colors, Create Material | Color Chooser window (egui picker) | eyedropper, custom swatches, Create Material |

### F. Toolbars, hotkeys, gamepad

| Dialog (page) | Chief panels and fields | Ours | Missing |
|---|---|---|---|
| Toolbar Customization (p. 163) | Tools panel (view type, searchable buttons, description, drag out); Toolbar panel (table with a check per view type, rename, Delete, Reset Toolbars); Configurations panel (Switch To, Copy, Rename, Delete, Choose Icon, Import, folder) | `customize_toolbars.rs`: view type, rows of buttons ordering, new rows, lock, reset, Chief .toolbar import, JSON export | Configurations panel (named configurations, icons), per-toolbar view table, drag-out, name-conflict dialogs |
| Customize Hotkeys (p. 169) | search; Tool Available In; Description; edit field with up to four chords; Reset Hotkeys | `hotkeys.rs` (all of it, plus conflict tools and print list) | Tool Available In list is not shown |
| Gamepad Settings (p. 171) | Buttons and Thumbsticks panels | none | Out-of-scope |

---

## Gaps to build

Ranked by how much a residential designer doing construction documents (custom homes and remodels, layout sheets) depends on the feature. Size: S under a day, M a few days, L a week or more. "File" is the parity file the rows were appended to.

| Rank | Gap | Why it matters on a custom home or remodel | Size | Parity file |
|---|---|---|---|---|
| 1 | Construction Lines (tool, infinite lines, callouts, ordering rule sets, specification dialog; usable as snap, dimension and Center/Trim targets) | The standard way to hold gridlines, setbacks and floor-to-floor alignment; also section/elevation reference lines | L | dimensions-text-cad.md (CAD) |
| 2 | Reference Display with several rows, another plan file, layer set per row, Details and XOR, offsets/angle, Swap Floor/Reference | Remodels: existing plan shown under the proposed plan, comparing options, aligning floors | L | dimensions-text-cad.md (LAY) |
| 3 | Line styles as a library (Line Style Management and Specification: dash, dot and text components; Import .lin) | Property lines, centerlines, hidden lines, "existing to remain" lines on CDs | L | dimensions-text-cad.md (LAY) |
| 4 | Layer management: New, Copy, Merge, Delete, Delete Unused, Reset Names, Fill column; Layer Set Defaults; Object Layer Properties; Layer Hider; Find Objects on Layer with Select Location | Layer discipline is how CD sheets are produced; today the layers are fixed | M | dimensions-text-cad.md (LAY) |
| 5 | Fill Styles: one Fill Style panel (pattern list, scale, offsets, angle, color source, background, gradient), library fill styles, Fill Style Painter, custom Patterns, Poché in plan views | Material hatches in plans and details; wall poché on every plan set | L | dimensions-text-cad.md (CAD) |
| 6 | Import Settings from Plan/Layout, Save as Template with purge list, New Plan from Template chooser, legacy .layers/.cadefs/.dat/note-type imports | Carrying a studio's wall types, layer sets and saved defaults from job to job | M | preferences-hotkeys-toolbars.md (APP) |
| 7 | Edit behaviors as Chief defines them: Alternate (continuous drawing, angle-keeping reshape), Move, temporary keys (Alt+Z, Z, X, C, F, slash, period), Ctrl to override restrictions, Shift to slow; fix the Alt and Ctrl meanings | Muscle memory: Alt and Ctrl do the opposite of Chief today | M | select-and-edit.md (S) |
| 8 | CAD edit tools: Intersect/Join Two Lines, Close Polyline, Simplify Polyline, Complete Break, Fillet All Corners, Concentric Resize with Set Concentric Jump and Sticky Mode, Lock Center, Convert Curve to Polyline, Fence Select, Create Hole, Disconnect Edges, Select Next Edge/Side, Point to Point Center, Align/Distribute Along Line, Revision Cloud Around Objects, Set as Default | Daily detail drafting | L (a group of 15 S/M items) | select-and-edit.md and dimensions-text-cad.md |
| 9 | Math in number fields (+ - * /) in every length box | Saves hundreds of keystrokes in dialogs; Chief users rely on it | S/M | preferences-hotkeys-toolbars.md (APP) |
| 10 | Preferences General, File Management, New Plans, Coordinate System, Dialogs/Side Windows, Project Browser, Edit (crosshair, handle size, indicators), Behaviors and Snap Properties gaps | Autosave, archive mode, startup, default folder, designer and client defaults | M | preferences-hotkeys-toolbars.md (PR) |
| 11 | General Plan Defaults: Warn Before Deleting, Ignore Casing, Show Pitch as Degrees, Arrow-Key Scroll Distance, framing line options, Living Area basis, Opening Indicator side, Geographic Location, Grade Level Marker and Elevation Reference | Living area and pitch conventions show up in every set | M | preferences-hotkeys-toolbars.md (DS) |
| 12 | Elevation References (Absolute, From Floor, Finished Floor, Terrain, Ceiling, Roof) on objects | Heights of fixtures, windows, soffits on remodels with changing floors | L | rooms-floors.md (R) |
| 13 | Style Palettes and Style Painter | Finish packages for clients; fast option studies | L | select-and-edit.md (S) |
| 14 | Room Boxes saved with the plan, Room Box tools per room type, overlap tools, Room Box Specification | The boxes are lost when the program closes today | M | rooms-floors.md (R) |
| 15 | Saved Defaults for all annotation kinds, Active Defaults dialog, activating a Default Set from a toolbar control, Set as Default, toolbar controls for layer set and dimension defaults | Switching annotation setups per sheet scale | M | preferences-hotkeys-toolbars.md (DS) |
| 16 | Project Browser: filter and sort, Advanced Search, Details/Notes, previews, new view folders, Find in Plan, Find Schedule(s) from Object | Navigation on large plans | M | preferences-hotkeys-toolbars.md (APP) |
| 17 | Missing Files dialog (with Replace, Search Directory) and Asset handling | Plans that move between machines lose textures and pictures silently | M | preferences-hotkeys-toolbars.md (APP) |
| 18 | Edit Area (All Floors), Edit Area Polyline, Place at Allowed Angles | Moving a whole remodel scheme across floors | M | select-and-edit.md (S) |
| 19 | Number/Angle Style dialog with quadrant and azimuth bearings; custom unit conversions | Site plans and scaled imports | M | preferences-hotkeys-toolbars.md (PR) |
| 20 | Plan View Specification gaps (remember zoom/rotation, show color, watermark, link to layout, poché, save options); typed Rotate Plan View angle | Saved views are the base of sheet production | M | dimensions-text-cad.md (LAY) |
| 21 | Time Tracker with Time Log (CSV export), preferences | Fee reconciliation for a studio | S/M | preferences-hotkeys-toolbars.md (APP) |
| 22 | Tool Search box, Tool Palette (child tools), Scrollbars, Coordinate System Indicators, Angle Snap Grid | Discoverability and orientation | M | preferences-hotkeys-toolbars.md (APP), dimensions-text-cad.md (LAY) |
| 23 | Delete Objects: room scopes, grouped categories, layout version, locked-layer rule | Cleaning up a remodel before a new scheme | S | select-and-edit.md (S) |
| 24 | Dashboard, Loan Calculator, Plan Database, Screen capture | Convenience only | S each | preferences-hotkeys-toolbars.md (APP) |
| 25 | Project-store features (projects, folders, caproj export/import, Asset Management dialog) | Differs by design; a backup/restore of a whole studio folder would cover the real need | M | preferences-hotkeys-toolbars.md (APP) |

---

## Counts

- Features enumerated in sections 1 to 10: **587** rows, plus **97** dialog-level rows in "Dialog panels" (684 in all), and about 250 individual Preferences, General Plan Defaults and dialog fields named in the panel tables.
- Section 1 to 10 status: **Works 157**, **Partial 218**, **Missing 180**, **Differs 22**, **Out-of-scope 9**, **In progress 1** (shared specification dialog for several objects, W-83).
- Rows with no parity spec before this audit (**NO SPEC**): **297**. Each now has a new id (APP-67 to APP-140, PR-17 to PR-31, DS-22 to DS-48, TB-6 to TB-14, HK-8 to HK-11, S-118 to S-193, LAY-38 to LAY-75, CAD-62 to CAD-106, R-89 to R-97), appended under "Manual audit additions (part 1)" in `docs/parity/preferences-hotkeys-toolbars.md`, `select-and-edit.md`, `dimensions-text-cad.md` and `rooms-floors.md`, and listed again at the end of `docs/parity-status.md` (totals table untouched). If another audit part appended to the same files before or after this run, ids may be numbered by whichever part ran last; each id is unique within the file at the time of the append.
- The doc's own rows carry the new id after "NO SPEC ->" so the two stay linked.

## Where the manual disagrees with what the project assumed

DECISIONS.md rows and parity statements that the manual contradicts or does not support (verify in Chief where the manual is ambiguous):

- **DS3** (Undo Levels, "Chief X18 keeps 20 steps"): the manual (p. 132, 316) says 50 actions by default and a Maximum Undos setting of 1 to 100. S-78 (Differs: levels) should be revisited too.
- **19** (menu rows removed as out of scope: Dashboard, Download Sample Plans, Close All Views, Save As Template, Save Thumbnail Image, Time Tracker, Plan Database, Loan Calculator, Screen Capture, Rotate Plan View, Reverse Plan, Tool Palette, Scrollbars, Coordinate System Indicator, Angle Snap Grid, Watermark, Zoom, Fill Window Building Only, Swap Views, Tile, Tab): several have since been built, and the rest are Chief features that Daniel's standing requirement says to build; the row is out of date.
- **77** (Shift holds CAD lines to 15 degrees): the manual (p. 193) says Shift restricts angle snaps to 90 or 45 degrees (a Preferences choice) and, while dragging, slows the pointer (p. 195, 246, 288).
- **53, 70, S-22, S-74** (Alt suspends snaps or frees the move, Alt starts a marquee over an object): in Chief Alt summons the Alternate edit behavior (p. 253) and Ctrl/Cmd overrides snaps and move restrictions (p. 190, 237); the S key suspends object snaps (p. 192). Plan Studio's Alt and Ctrl are effectively swapped.
- **70** (Marquee Selection kept for the session only): the manual puts the marquee mode in Preferences > Edit (p. 146) and requires Shift or Ctrl to start a marquee in Chief (p. 260).
- **DS7** (Reverse Plan leaves objects on hidden or locked layers where they are): the manual (p. 179) mirrors the entire plan on all floors, so those objects would end up on the wrong side.
- **DS8** (Rotate Plan View is session state only): the manual (p. 178) rotates to a typed angle and saves it with the view when Remember Zoom/Rotation is checked.
- **22** (UTC-stamped archives): differs from Chief's hour/day/previous-save naming (p. 74); acceptable, but the Hourly/Daily/Previous Save choice is missing.
- **41** (camera step sizes "come from Preferences > Behaviors"): the manual's Behaviors panel (p. 148) has no camera step settings.
- **S-5 / S-89** (objects on a locked layer can be selected for viewing; Delete Objects leaves locked-layer objects): the manual says locked-layer objects cannot be selected (p. 208, 257) and that Delete Objects can still delete most of them (p. 208, 313).
- **APP-28** (Plan Database described as a list of every object in the plan): the manual's Plan Database (p. 96) is a finder over a folder of plan files.
- **APP-30** (Loan Calculator "out of scope"): it is a small Chief dialog (p. 95); build it if Daniel wants the full set.
- **DS4** (Reset to Defaults): the manual's Reset to Defaults (p. 118) resets values inside the plan (heights, roof directives, gable lines, wall heights, auto connections, overridden dimension text); ours resets the defaults tree.
