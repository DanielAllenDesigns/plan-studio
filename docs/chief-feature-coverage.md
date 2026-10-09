# Chief Architect X18 feature coverage audit

Written 2026-10-08 (Round 14 coverage audit, standing track). Daniel's requirement: every feature found in Chief Architect X18 is built into Plan Studio, and that is re-confirmed every round. This file is the master inventory of Chief X18 features, each mapped to the parity spec ids that cover it (or **NO SPEC**), to code evidence and to a status. Sources: the five capture documents ([menus](chief-x18-menus.md), [toolbars](chief-x18-toolbars.md), [sub-tools](chief-x18-subtools.md), [dialogs](chief-x18-dialogs.md), [UI notes](chief-x18-ui-notes.md)), plus Chief Premier X18 features known from the product; rows from product knowledge rather than a capture say "(not captured)" and carry "verify in Chief" until someone has seen the real dialog.

No Rust was edited and no Chief file was read for this audit. Status comes from `docs/parity-status.md` (the per-id rows as they stood when this file was generated) for rows with a spec, and from a code search (menus.rs, toolbar.rs, dialog tab tables, tool and crate names) for rows without one. Where a row maps to several ids it is Works only when every id is Works, Missing only when every id is Missing, and Partial otherwise, so the percentages below are strict.

Status words follow `docs/parity-status.md`: Works, Partial, Missing, Differs-by-design. "In progress (Round 14)" marks a row that a Round 14 builder is working on at this moment (the briefs in the orchestrator scratchpad were read to find them); its status is the status before that work lands.

## Headline numbers

- Chief features inventoried: **1099** rows (43 menu, toolbar and dialog groups), of which 710 come from the captures and 389 from product knowledge (not captured).
- Covered by a parity spec id: **841** rows (76%). With no spec (**NO SPEC**): **258** rows (23%), of which 63 repeat a feature already counted in another row, leaving **195 new parity rows** appended to `docs/parity/*.md` and listed under "Coverage audit" in `docs/parity-status.md`.
- Overall against the FULL inventory: Works **703** (64%), Partial 287 (26%), Missing 95 (9%), Differs-by-design 14 (1%). Works plus Partial: 90%.
- Rows with a spec: 560 Works of 841 (67%). Rows without one: 143 Works of 258 (55%): the application shell, file and window commands and many dialog tabs were never specced, which is why the figure against the full inventory is lower than the 71% in `parity-status.md`.
- Rows a Round 14 builder is working on: 171.

How to read the tables: "Parity" lists the spec ids that cover the row; "NO SPEC (X-n)" means no id covered it before this audit and X-n is the parity row this audit appended for it (rows that share a feature share the id). Evidence names a file, function or test (from the parity row when there is one).

## Summary by Chief menu, toolbar and dialog group

| Group | Items | Specced | Works | Partial | Missing | Differs | NO SPEC | In progress (Round 14) |
|---|---|---|---|---|---|---|---|---|
| File menu | 38 | 12 | 24 | 8 | 6 | 0 | 26 | 1 |
| Edit menu | 30 | 23 | 22 | 4 | 3 | 1 | 7 | 3 |
| Build > Walls, railings, fencing | 35 | 29 | 25 | 8 | 2 | 0 | 6 | 3 |
| Build > Doors and windows | 23 | 23 | 20 | 3 | 0 | 0 | 0 | 3 |
| Build > Floors, foundation, slabs | 19 | 10 | 17 | 2 | 0 | 0 | 9 | 2 |
| Build > Roofs | 15 | 15 | 14 | 1 | 0 | 0 | 0 | 0 |
| Build > Framing and trim | 32 | 26 | 21 | 11 | 0 | 0 | 6 | 7 |
| Build > Stairs | 17 | 16 | 7 | 9 | 1 | 0 | 1 | 9 |
| Build > Cabinets | 21 | 17 | 20 | 1 | 0 | 0 | 4 | 1 |
| Build > Electrical | 23 | 14 | 23 | 0 | 0 | 0 | 9 | 0 |
| Build > 3D solid, images, distributed objects | 16 | 0 | 14 | 2 | 0 | 0 | 16 | 0 |
| Terrain menu | 58 | 53 | 50 | 7 | 1 | 0 | 5 | 5 |
| Library menu | 13 | 7 | 6 | 2 | 3 | 2 | 6 | 0 |
| 3D menu | 37 | 35 | 17 | 16 | 4 | 0 | 2 | 18 |
| CAD menu | 68 | 63 | 52 | 13 | 1 | 2 | 5 | 9 |
| Tools menu | 40 | 21 | 22 | 8 | 10 | 0 | 19 | 2 |
| View menu | 20 | 8 | 15 | 1 | 4 | 0 | 12 | 0 |
| Window menu | 15 | 1 | 6 | 5 | 4 | 0 | 14 | 1 |
| Account and Help | 14 | 0 | 5 | 0 | 2 | 7 | 14 | 0 |
| Toolbar row 1 | 37 | 31 | 23 | 12 | 2 | 0 | 6 | 7 |
| Toolbar row 2 (Build tools) | 25 | 22 | 23 | 2 | 0 | 0 | 3 | 2 |
| Right-edge vertical toolbar | 21 | 10 | 17 | 3 | 1 | 0 | 11 | 1 |
| Edit toolbar (contextual) by object | 13 | 13 | 9 | 4 | 0 | 0 | 0 | 2 |
| Right-click context menus | 2 | 2 | 2 | 0 | 0 | 0 | 0 | 0 |
| Default Settings tree pages | 30 | 20 | 10 | 13 | 7 | 0 | 10 | 2 |
| Wall Specification tabs | 16 | 11 | 0 | 10 | 6 | 0 | 5 | 5 |
| Door Specification tabs | 20 | 12 | 9 | 4 | 7 | 0 | 8 | 2 |
| Window Specification tabs | 14 | 9 | 8 | 2 | 4 | 0 | 5 | 1 |
| Room Types and Room Specification | 10 | 9 | 1 | 9 | 0 | 0 | 1 | 7 |
| Cabinet Specification tabs | 7 | 7 | 1 | 6 | 0 | 0 | 0 | 6 |
| Dimension Defaults tabs | 11 | 10 | 3 | 7 | 1 | 0 | 1 | 3 |
| Other object dialogs (not captured) | 41 | 36 | 27 | 14 | 0 | 0 | 5 | 9 |
| Preferences dialog pages | 18 | 17 | 9 | 9 | 0 | 0 | 1 | 8 |
| Product features not in the captures: 3D, rendering and presentation | 25 | 21 | 12 | 8 | 4 | 1 | 4 | 7 |
| Documentation and layout | 35 | 33 | 20 | 13 | 2 | 0 | 2 | 5 |
| Import and export formats | 13 | 7 | 5 | 5 | 3 | 0 | 6 | 1 |
| Architectural objects and building systems | 39 | 33 | 19 | 16 | 4 | 0 | 6 | 8 |
| Editing tools | 19 | 16 | 13 | 2 | 3 | 1 | 3 | 2 |
| Program features | 12 | 3 | 8 | 0 | 4 | 0 | 9 | 0 |
| Snap Settings, Edit Behaviors and detail rows | 62 | 55 | 42 | 16 | 4 | 0 | 7 | 7 |
| Layers, line weights and project settings (not captured) | 11 | 9 | 8 | 3 | 0 | 0 | 2 | 1 |
| Layout window commands (Chief's Layout menus, not in the captures) | 81 | 81 | 54 | 27 | 0 | 0 | 0 | 20 |
| Additional product features (not captured) | 3 | 1 | 0 | 1 | 2 | 0 | 2 | 1 |
| **All groups** | **1099** | **841** | **703** | **287** | **95** | **14** | **258** | **171** |

### NO SPEC rows by area

| Area | New parity rows | Works | Partial | Missing | Differs | Parity file |
|---|---|---|---|---|---|---|
| Walls (W-) | 13 | 7 | 1 | 5 | 0 | `docs/parity/walls.md` |
| Select and edit (S-) | 5 | 3 | 0 | 2 | 0 | `docs/parity/select-and-edit.md` |
| Doors and windows (DW-) | 13 | 6 | 0 | 7 | 0 | `docs/parity/doors-windows.md` |
| Rooms and floors (R-) | 17 | 10 | 2 | 5 | 0 | `docs/parity/rooms-floors.md` |
| Roofs (RF-) | 1 | 0 | 0 | 1 | 0 | `docs/parity/roofs.md` |
| Dimensions (DIM-) | 1 | 0 | 0 | 1 | 0 | `docs/parity/dimensions-text-cad.md` |
| Text (TXT-) | 2 | 1 | 0 | 1 | 0 | `docs/parity/dimensions-text-cad.md` |
| CAD, images and details (CAD-) | 16 | 9 | 2 | 5 | 0 | `docs/parity/dimensions-text-cad.md` |
| Layers, display and view commands (LAY-) | 22 | 12 | 4 | 6 | 0 | `docs/parity/dimensions-text-cad.md` |
| 3D views, cameras and rendering (C-) | 8 | 2 | 0 | 5 | 1 | `docs/parity/3d-views-cameras.md` |
| Cabinets, stairs, framing, terrain, library (CB-) | 21 | 7 | 6 | 7 | 1 | `docs/parity/cabinets-stairs-framing-terrain-library.md` |
| Layout, print, schedules, import and export (L-) | 9 | 4 | 1 | 4 | 0 | `docs/parity/documentation-layout.md` |
| Electrical (E-) | 1 | 1 | 0 | 0 | 0 | `docs/parity/electrical.md` |
| Application shell: files, windows, help, tools menu (APP-) | 66 | 36 | 9 | 18 | 3 | `docs/parity/preferences-hotkeys-toolbars.md` |

### What Round 14 builders are working on

Rows marked "In progress (Round 14)" belong to these briefs (read from the orchestrator scratchpad): walls (multi-wall Open Object, 3+ wall joins, Wall Specification Structure and Foundation tabs), openings (placement feel, plan detail, marks), roofs (presets, wings, dormers, plane editing, bay roofs), rooms and foundations, cabinets and stairs, select/clipboard/CAD (hover, Edit Area, Stretch CAD, typed input, arc modes), text and dimensions, layout and print (page set-up, print preview, schedules in output), cameras and 3D (Camera Specification, walkthrough, lighting, Project Browser cameras), framing, electrical, terrain, materials, images/underlays/CAD details, preferences/hotkeys/toolbars, Plan Check, and the tool-dialog sweep. The tool-dialog sweep writes `docs/tool-dialog-sweep.md` with the Chief tabs each dialog still lacks; that table and the tab rows in this file (Wall, Door, Window, Room, Cabinet, Dimension and the "Other dialogs" group) should be reconciled when it lands.

## Full mapped list

One row per Chief feature, grouped as in the summary table. Source "cap" is a capture document, "n" is product knowledge (not captured).

### File menu

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | File >> New Plan (Cmd+N) | cap | NO SPEC (APP-1) | Works | menus.rs file_menu "New Plan"; dialogs/app_info.rs new_plan (clean-plan and unsaved prompt in files.rs) |
| 2 | File >> New Layout | cap | L-1 | Works | shell/layout_window.rs; dialogs/layout.rs; hotkey S,L; Project Browser camera list (right-click > Send to Layout, `LayoutCommand::SendCamer… |
| 3 | File >> Templates > New Plan From Template… | cap | NO SPEC (APP-2) | Partial | File > Templates holds Save Current Defaults as My Template, Import Chief Template, Reset to Chief X18 Template (templates.rs); there is no… |
| 4 | File >> Templates > New Layout From Template… | cap | L-10 | Works | Page Template page and flag (Page Specification); Daniel 18x24 block; Save As Template / Apply Template; New Layout File starts from the de… |
| 5 | File >> Open Plan… (Cmd+O) | cap | NO SPEC (APP-3) | Works | files.rs open_dialog, perform; test unsaved_plans_prompt_and_clean_plans_go_straight_through |
| 6 | File >> Open Layout… | cap | NO SPEC (APP-4) | Works | menus.rs "Open Layout…"; layout_window.rs |
| 7 | File >> Open Recent Documents ▸ | cap | NO SPEC (APP-5) | Works | menus.rs recent_menu; files.rs test recents_dedupe_and_cap |
| 8 | File >> Dashboard… | cap | NO SPEC (APP-6) | Missing | not in menus.rs |
| 9 | File >> Download Sample Plans… | cap | NO SPEC (APP-7) | Missing | not present; repo has samples/ but no in-app download |
| 10 | File >> Close View (Cmd+W) | cap | NO SPEC (APP-8) | Works | menus.rs "Close View"; plan_tabs.rs |
| 11 | File >> Close All 3D Views | cap | NO SPEC (APP-9) | Missing | only Close View and Close Plan exist |
| 12 | File >> Close All Views | cap | NO SPEC (APP-10) | Missing | only Close View and Close Plan exist |
| 13 | File >> Save (Cmd+S) | cap | NO SPEC (APP-11) | Works | files.rs mark_saved; tests save_archives_the_old_version_and_a_failed_save_changes_nothing, dirty_tracking_follows_edits_undo_and_saves |
| 14 | File >> Save As… | cap | NO SPEC (APP-12) | Works | menus.rs "Save As…"; files.rs perform |
| 15 | File >> Save As Template… | cap | NO SPEC (APP-13) | Partial | "Save Current Defaults as My Template…" saves the plan defaults, not a whole plan as a template file (templates.rs) |
| 16 | File >> Save Thumbnail Image | cap | NO SPEC (APP-14) | Missing | not in menus.rs; plan thumbnail is only read when importing Chief plans |
| 17 | File >> Show in Project Browser | cap | NO SPEC (APP-15) | Works | menus.rs "Show in Project Browser"; docks.rs Project dock |
| 18 | File >> View File Information… | cap | NO SPEC (APP-16) | Works | app_info::FILE_INFO; dialogs/app_info.rs |
| 19 | File >> Manage Auto Archives… | cap | NO SPEC (APP-17) | Works | files.rs archives_window; test rotation_keeps_only_the_newest_n_archives |
| 20 | File >> Backup Entire Plan… (not captured) | n | NO SPEC (APP-18) | Works | files.rs backup_entire_plan; test a_backup_zips_the_plan_and_the_pictures_it_uses |
| 21 | File >> Save a Copy… (not captured) | n | NO SPEC (APP-19) | Works | files.rs save_a_copy; test revert_reloads_the_saved_file |
| 22 | File >> Revert to Saved (not captured) | n | NO SPEC (APP-19) | Works | files.rs save_a_copy; test revert_reloads_the_saved_file |
| 23 | File >> Export ▸ DXF/DWG drawing | cap | L-44, L-45 | Partial | plan-core export/dxf.rs R12 ASCII; elevation DXF Gap: No version, units, selection options; no DWG. |
| 24 | File >> Export ▸ PDF (construction set, layout) | cap | L-20 | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes Gap: The two standard fonts are ref… |
| 25 | File >> Export ▸ Elevation DXF | cap | L-44 | Partial | plan-core export/dxf.rs R12 ASCII; elevation DXF Gap: No version, units, selection options; no DWG. |
| 26 | File >> Export ▸ glTF 3D model (not captured) | n | NO SPEC (L-48) | Works | menus.rs "glTF…"; plan-view3d export.rs |
| 27 | File >> Export ▸ Picture / 3D view image (PNG, JPEG, BMP, TIFF) | cap | NO SPEC (L-49) | Partial | no Export Picture row in menus.rs; Print Image… saves the floor plan lines as a PNG (dialogs/print.rs), Ray Trace > Save PNG saves a render… |
| 28 | File >> Export ▸ 3D Viewer / 360 Panorama / Walkthrough video (not captured) | n | C-71 | Partial | Record Walkthrough writes a numbered PNG sequence (view3d_panel.rs record_walkthrough); no video file, no 3D Viewer file, no 360 panorama |
| 29 | File >> Import ▸ Import Drawing (DXF) | cap | L-43 | Works | dialogs/exchange.rs import window: units, scale, rotation, base and insertion point, per-layer map (keep, skip, plan layer, new name), Conv… |
| 30 | File >> Import ▸ Chief Plan (.plan) (not captured) | n | NO SPEC (L-50) | Works | menus.rs "Chief Plan…"; plan-chiefplan crate; main.rs import_chief_plan |
| 31 | File >> Import ▸ Picture (PNG, JPEG) | cap | NO SPEC (CAD-46) | Works | tools/images.rs; menus.rs "Picture (PNG, JPEG)…" |
| 32 | File >> Import ▸ Underlay picture/PDF | cap | L-46 | Partial, In progress (Round 14) | `plan_core::underlay`, `tools/underlay.rs` (+ `pdf.rs`, `inflate.rs`, `trace.rs`), `dialogs/underlay.rs`: PNG and JPEG (baseline and progre… |
| 33 | File >> Import ▸ Import Project / Merge Plan (not captured) | n | NO SPEC (L-51) | Missing | no way to merge another plan file into the open plan |
| 34 | File >> Print ▸ Print… | cap | L-18 | Works | File > Print dialog (dialogs/print.rs): PDF file, system printer picked from `lpstat -p` (default printer otherwise) through `lp -d`, or vi… |
| 35 | File >> Print ▸ Print Preview | cap | L-19 | Works | Print Preview window (`PrintPreviewDialog`, `plan_layout::layout_print_preview` / `plan_view_print_preview`): the pages as the Print dialog… |
| 36 | File >> Print ▸ Print Image / Print Model | cap | L-19 | Works | Print Preview window (`PrintPreviewDialog`, `plan_layout::layout_print_preview` / `plan_view_print_preview`): the pages as the Print dialog… |
| 37 | File >> Send to Layout… (S, L) | cap | L-1, L-2 | Works | shell/layout_window.rs; dialogs/layout.rs; hotkey S,L; Project Browser camera list (right-click > Send to Layout, `LayoutCommand::SendCamer… |
| 38 | File >> Quit (Cmd+Q) | cap | NO SPEC (APP-20) | Works | files.rs on_exit; test the_close_button_asks_first_when_there_are_unsaved_changes |

### Edit menu

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Edit >> Undo (Cmd+Z) | cap | S-75, S-76 | Works | plan-core history.rs; scenarios s12_hotkeys |
| 2 | Edit >> Redo (Cmd+Y) | cap | S-75, S-76 | Works | plan-core history.rs; scenarios s12_hotkeys |
| 3 | Edit >> Cut | cap | S-81 | Works | clipboard.rs; edit_commands.rs |
| 4 | Edit >> Copy (Cmd+X, Cmd+C) | cap | S-81 | Works | clipboard.rs; edit_commands.rs |
| 5 | Edit >> Copy and Paste in Place (C, P, P) | cap | S-84 | Works | shell/hotkeys.rs edit_defaults |
| 6 | Edit >> Paste ▸ (Paste, Paste Hold Position, Paste Special) | cap | S-82, S-83 | Works | transform.rs Mode::Paste; edit_tests.rs |
| 7 | Edit >> Delete (Del) | cap | S-87 | Works | select.rs test delete_removes_the_selection_with_its_openings_and_undoes |
| 8 | Edit >> Delete Objects… (Shift+Space) | cap | S-88 | Works | dialogs/delete_objects.rs |
| 9 | Edit >> Select Objects (Space) | cap | S-10 | Works | toolbar.rs bindings; scenarios s12_hotkeys |
| 10 | Edit >> Select All (Cmd+A) | cap | S-33 | Works | selection.rs select_all; edit_tests.rs |
| 11 | Edit >> Snap Settings ▸ (object, angle, grid) | cap | S-68, S-69, S-70, S-71, S-72, S-74 | Works | dialogs/snap_settings.rs, snap.rs SnapSettings, `editing` defaults |
| 12 | Edit >> Edit Behaviors ▸ (Default, Replicate, Resize, Concentric, Fillet, Chamfer) | cap | S-65, S-67 | Works | dialogs/edit_behaviors.rs; editor/behaviors.rs modes Default, Resize, Concentric, Fillet, Chamfer, Alternate, Replicate (hands the drag to… |
| 13 | Edit >> Arc Creation Modes ▸ | cap | CAD-7 | Partial, In progress (Round 14) | CadMode::Arc option strip (three-point, center-start-end, tangent) Gap: Start-end-radius mode absent; Edit > Arc Creation Modes inert. |
| 14 | Edit >> Edit Area ▸ | cap | S-90 | Missing, In progress (Round 14) | menus.rs:300 inert Gap: Not built. |
| 15 | Edit >> Stretch CAD | cap | S-91 | Missing, In progress (Round 14) | menus.rs:301 inert Gap: Not built. |
| 16 | Edit >> Find/Replace Text… | cap | TXT-12 | Partial | dialogs/find_replace.rs; Edit > Find/Replace Text (menus.rs:505, Action::FindReplaceText); plan-core find_text.rs Gap: Find/Replace works;… |
| 17 | Edit >> Replace Fonts… | cap | TXT-12 | Partial | dialogs/find_replace.rs; Edit > Find/Replace Text (menus.rs:505, Action::FindReplaceText); plan-core find_text.rs Gap: Find/Replace works;… |
| 18 | Edit >> Default Settings… (tree) | cap | NO SPEC (APP-21) | Partial | dialogs/defaults.rs TREE has Walls (3), Doors (2), Windows, Dimension, Text, Floors and Rooms, Roofs, Cabinets, Framing, Terrain, Preferenc… |
| 19 | Edit >> Reset to Defaults… | cap | NO SPEC (APP-22) | Missing | shown dimmed (menus.rs inert list) |
| 20 | Edit >> Preferences… (Cmd+,) | cap | PR-1 | Works | `dialogs/preferences.rs Page::ALL`; `every_page_draws_with_changed_values` |
| 21 | Edit >> AutoFill ▸ / Start Dictation / Emoji & Symbols (macOS-supplied) | cap | NO SPEC (APP-23) | Differs-by-design | macOS system rows; Plan Studio shows them dimmed (menus.rs) and does not supply them |
| 22 | Edit >> Duplicate (not captured) | n | S-35, S-36 | Works | edit_commands.rs group_selection/ungroup_selection; select.rs pointer_down; plan-core groups.rs |
| 23 | Edit >> Group (not captured) | n | S-35, S-36 | Works | edit_commands.rs group_selection/ungroup_selection; select.rs pointer_down; plan-core groups.rs |
| 24 | Edit >> Ungroup (not captured) | n | S-35, S-36 | Works | edit_commands.rs group_selection/ungroup_selection; select.rs pointer_down; plan-core groups.rs |
| 25 | Edit >> Explode (Chief Edit toolbar) (not captured) | n | S-35, S-36 | Works | edit_commands.rs group_selection/ungroup_selection; select.rs pointer_down; plan-core groups.rs |
| 26 | Edit >> Move to Front (not captured) | n | NO SPEC (S-113) | Works | edit_commands.rs FRONT/BACK; menus.rs "Move to Front" |
| 27 | Edit >> Move to Back (draw order) (not captured) | n | NO SPEC (S-113) | Works | edit_commands.rs FRONT/BACK; menus.rs "Move to Front" |
| 28 | Edit >> Lock (not captured) | n | NO SPEC (S-114) | Works | edit_commands.rs LOCK/UNLOCK |
| 29 | Edit >> Unlock selection (not captured) | n | NO SPEC (S-114) | Works | edit_commands.rs LOCK/UNLOCK |
| 30 | Edit >> Send to Layer… | cap | LAY-13 | Works | editor/edit_commands.rs ids::LAYER (Layer button on the Edit toolbar); dialogs/send_to_layer.rs; test edit_tests.rs the_edit_toolbar_offers… |

### Build > Walls, railings, fencing

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Build Walls >> Straight Exterior Wall (Shift+Q) | cap | W-1, W-2, W-6 | Works | tools/wall.rs WallVariant::spec; template-seeded defaults (plan-core defaults.rs) |
| 2 | Build Walls >> Straight Interior Wall (Ctrl+Opt+Cmd+6) | cap | W-1 | Works | tools/wall.rs WallVariant::spec; template-seeded defaults (plan-core defaults.rs) |
| 3 | Build Walls >> Wall drawing gestures: click chain, drag, close loop, typed length and angle, snaps, guides (not captured) | n | W-3, W-4, W-5, W-14, W-15, W-16, W-17, W-18 | Partial | tools/wall.rs advance_chain Gap: Right-click ends the chain here (Chief keeps it). |
| 4 | Build Walls >> Wall joins: corners, T, 3-way, crossings, repair (not captured) | n | W-32, W-34, W-35, W-36, W-37, W-38 | Partial, In progress (Round 14) | manual 2.10, qa-findings: split_on_tee Gap: Plan Studio splits the through wall by default; `walls_connect.split_on_tee` (plan-core default… |
| 5 | Build Walls >> Wall editing: length, angle, thickness, lock, break, openings keep position (not captured) | n | W-27, W-28, W-75, W-76, W-77, W-78, W-85 | Partial | tempdim.rs WallGap moves wall Gap: Moves wall, no face-to-face resize of thickness. |
| 6 | Build Walls >> Straight Foundation Wall | cap | W-52 | Partial, In progress (Round 14) | walls/wall_spec.rs WallFoundation; dialogs/wall.rs Foundation tab (footing, slab chamfers, sill plate); plan-3d wall.rs spec_meshes; tests… |
| 7 | Build Walls >> Curved Foundation Wall | cap | W-52 | Partial, In progress (Round 14) | walls/wall_spec.rs WallFoundation; dialogs/wall.rs Foundation tab (footing, slab chamfers, sill plate); plan-3d wall.rs spec_meshes; tests… |
| 8 | Build Walls >> Straight Pony Wall | cap | W-53 | Works | PonyWall in walls.rs; dialogs/wall.rs |
| 9 | Build Walls >> Straight Half-Wall | cap | W-54 | Works | WallClass::Half |
| 10 | Build Walls >> Straight Glass Wall | cap | W-57 | Works | wall.rs tests; Glass-1 type; plan-3d tests/curved_walls.rs |
| 11 | Build Walls >> Straight Glass Pony Wall | cap | W-57 | Works | wall.rs tests; Glass-1 type; plan-3d tests/curved_walls.rs |
| 12 | Build Walls >> Room Divider (invisible wall) | cap | W-55 | Works | wall.rs test room_dividers_close_rooms |
| 13 | Build Walls >> Slab Footing wall tool | cap | W-58 | Works | tools/details Slab Footing |
| 14 | Build Walls >> Wall Hatching | cap | W-59 | Works | tools/details.rs |
| 15 | Build Walls >> Wall Material Region | cap | W-59 | Works | tools/details.rs |
| 16 | Build Walls >> Straight Attic Wall tool (not captured as a tool) (not captured) | n | RF-31 | Partial | plan-3d cover.rs attic panels, RoofDetail::auto_attic_walls, RoofTypes::attic Gap: Generated at scene time above a lower roof beside a tall… |
| 17 | Build Walls >> Curved Exterior Wall | cap | W-64, W-65, W-66 | Partial | dialogs/wall.rs arc_section Gap: General tab Arc section: Curved Wall check box, Radius, Arc Angle, Arc Rise, side, arc length and center r… |
| 18 | Build Walls >> Curved Interior Wall | cap | W-64, W-65, W-66 | Partial | dialogs/wall.rs arc_section Gap: General tab Arc section: Curved Wall check box, Radius, Arc Angle, Arc Rise, side, arc length and center r… |
| 19 | Build Walls >> Curved Foundation Wall | cap | W-64, W-65 | Works | wall.rs tests curved_variants_take_a_third_click_for_the_arc, the_arc_passes_through_the_third_point, the_arc_readout_gives_radius_arc_leng… |
| 20 | Build Walls >> Curved Pony Wall | cap | W-64, W-65 | Works | wall.rs tests curved_variants_take_a_third_click_for_the_arc, the_arc_passes_through_the_third_point, the_arc_readout_gives_radius_arc_leng… |
| 21 | Build Walls >> Curved Half-Wall | cap | W-64, W-65 | Works | wall.rs tests curved_variants_take_a_third_click_for_the_arc, the_arc_passes_through_the_third_point, the_arc_readout_gives_radius_arc_leng… |
| 22 | Build Walls >> Fix Wall Connections | cap | W-41 | Works | editor/actions.rs FixWallConnections; connect.rs fix_wall_connections_action |
| 23 | Build Walls >> Break Wall | cap | W-43, W-44 | Works | editor/wall_edit.rs break_wall_at/break_click; plan-core split_wall_at |
| 24 | Build Walls >> Add Break | cap | W-43, W-44 | Works | editor/wall_edit.rs break_wall_at/break_click; plan-core split_wall_at |
| 25 | Build Walls >> Straight Railing (Cmd+Q) | cap | W-56 | Works | rail style tab; plan-3d |
| 26 | Build Walls >> Curved Railing | cap | W-56 | Works | rail style tab; plan-3d |
| 27 | Build Walls >> Straight Deck Railing | cap | W-56 | Works | tools/wall.rs WallStyle::DeckRailing |
| 28 | Build Walls >> Curved Deck Railing | cap | W-56 | Works | tools/wall.rs WallStyle::DeckRailing |
| 29 | Build Walls >> Straight Deck Edge | cap | NO SPEC (W-106) | Works | tools/wall.rs WallStyle::DeckEdge; toolbar.rs railing_deck() |
| 30 | Build Walls >> Curved Deck Edge | cap | NO SPEC (W-106) | Works | tools/wall.rs WallStyle::DeckEdge; toolbar.rs railing_deck() |
| 31 | Build Walls >> Polygon Shaped Deck… | cap | NO SPEC (W-107) | Works | tools/details.rs DetailsVariant::PolygonDeck; toolbar.rs railing_deck() |
| 32 | Build Walls >> Straight Fencing | cap | NO SPEC (W-108) | Works | tools/wall.rs WallStyle::Fencing; toolbar.rs fencing(); plan-3d wall_kinds |
| 33 | Build Walls >> Curved Fencing | cap | NO SPEC (W-108) | Works | tools/wall.rs WallStyle::Fencing; toolbar.rs fencing(); plan-3d wall_kinds |
| 34 | Build Walls >> Build Deck Framing | cap | NO SPEC (CB-86) | Missing | no deck framing command (joists and beams come from Build Framing on floor platforms) |
| 35 | Build Walls >> Calculate Structural Materials for Deck | cap | CB-42 | Missing | none Gap: Out of scope. |

### Build > Doors and windows

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Build Openings >> Hinged Door (D,H) | cap | DW-38 | Works | opening_symbol.rs Hinged and DoubleDoor; tools/opening.rs Double Door entry |
| 2 | Build Openings >> Doorway (D,W) | cap | DW-39 | Works | opening_symbol.rs Doorway; toolbar.rs door() D, W |
| 3 | Build Openings >> Sliding Door (S,D) | cap | DW-40 | Works | opening_symbol.rs Sliding (2 to 4 panels, arrow); plan-3d doors.rs; toolbar.rs S, D |
| 4 | Build Openings >> Pocket Door (D,P) | cap | DW-41 | Works | opening_symbol.rs Pocket (dashed pocket past a jamb); toolbar.rs D, P |
| 5 | Build Openings >> Bifold Door | cap | DW-42 | Works | opening_symbol.rs Bifold (V per pair); plan-3d doors.rs |
| 6 | Build Openings >> Garage Door (G,D) | cap | DW-43, DW-90 | Works | opening_symbol.rs Garage (panel and dashed overhead path); plan-3d doors.rs; toolbar.rs G, D |
| 7 | Build Openings >> Barn Door | cap | DW-44 | Works | opening_symbol.rs Barn (panel off the face, track); plan-3d doors.rs |
| 8 | Build Openings >> Shower Door | cap | DW-45 | Works | opening_symbol.rs Shower (glass leaf and arc) and Fixed door (glass pane) |
| 9 | Build Openings >> Fixed Door | cap | DW-45 | Works | opening_symbol.rs Shower (glass leaf and arc) and Fixed door (glass pane) |
| 10 | Build Openings >> Double Door (Options: Double Door Only) (not captured) | n | DW-38, DW-30 | Works | opening_symbol.rs Hinged and DoubleDoor; tools/opening.rs Double Door entry |
| 11 | Build Openings >> Window (Shift+W) | cap | DW-47 | Works | opening_symbol.rs Window, Casement (swing arcs), Fixed, SlidingWindow, Awning, Hopper |
| 12 | Build Openings >> Casement Window | cap | DW-47 | Works | opening_symbol.rs Window, Casement (swing arcs), Fixed, SlidingWindow, Awning, Hopper |
| 13 | Build Openings >> Fixed Window | cap | DW-47 | Works | opening_symbol.rs Window, Casement (swing arcs), Fixed, SlidingWindow, Awning, Hopper |
| 14 | Build Openings >> Sliding Window | cap | DW-47 | Works | opening_symbol.rs Window, Casement (swing arcs), Fixed, SlidingWindow, Awning, Hopper |
| 15 | Build Openings >> Awning Window | cap | DW-47 | Works | opening_symbol.rs Window, Casement (swing arcs), Fixed, SlidingWindow, Awning, Hopper |
| 16 | Build Openings >> Hopper Window | cap | DW-47 | Works | opening_symbol.rs Window, Casement (swing arcs), Fixed, SlidingWindow, Awning, Hopper |
| 17 | Build Openings >> Bay Window | cap | DW-48 | Partial, In progress (Round 14) | opening_symbol.rs projection_footprint (bay, bow, box); plan-3d windows.rs shares it; roof: plan-3d roof.rs bay_roof_into (hip or shed roof… |
| 18 | Build Openings >> Box Window | cap | DW-48 | Partial, In progress (Round 14) | opening_symbol.rs projection_footprint (bay, bow, box); plan-3d windows.rs shares it; roof: plan-3d roof.rs bay_roof_into (hip or shed roof… |
| 19 | Build Openings >> Bow Window | cap | DW-48 | Partial, In progress (Round 14) | opening_symbol.rs projection_footprint (bay, bow, box); plan-3d windows.rs shares it; roof: plan-3d roof.rs bay_roof_into (hip or shed roof… |
| 20 | Build Openings >> Pass-Through | cap | DW-49 | Works | opening_symbol.rs PassThrough and WallNiche (cut band); Opening::niche_depth (spec.niche_depth, 3 1/2 in default); dialogs/opening.rs Wall… |
| 21 | Build Openings >> Wall Niche | cap | DW-49 | Works | opening_symbol.rs PassThrough and WallNiche (cut band); Opening::niche_depth (spec.niche_depth, 3 1/2 in default); dialogs/opening.rs Wall… |
| 22 | Build Openings >> Skylight | cap | RF-43 | Works | roof_view.rs skylight; 3D curb/frame/glass |
| 23 | Build Openings >> Mulled / combined windows (not captured) | n | DW-51, DW-52 | Works | Project::mull_openings / unmull_openings; opening_edit.rs Mull and Unmull; scenarios s13 |

### Build > Floors, foundation, slabs

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Build Floors >> Build New Floor (Shift+X) | cap | R-59 | Works | floors.rs build_new_floor_with; dialogs/floor.rs |
| 2 | Build Floors >> Insert New Floor | cap | R-60 | Works | floors.rs insert_floor_above, delete_floor |
| 3 | Build Floors >> Delete Current Floor | cap | R-60 | Works | floors.rs insert_floor_above, delete_floor |
| 4 | Build Floors >> Build Foundation (Cmd+F) | cap | R-61, R-62 | Partial, In progress (Round 14) | floors.rs build_foundation; FoundationKind Gap: Walls with footings, monolithic, piers; no basement. |
| 5 | Build Floors >> Delete Foundation | cap | R-63 | Works | floors.rs |
| 6 | Build Floors >> Exchange With Floor Above | cap | R-64 | Works | floors.rs exchange_floors |
| 7 | Build Floors >> Exchange With Floor Below | cap | R-64 | Works | floors.rs exchange_floors |
| 8 | Build Floors >> Floor Defaults | cap | R-56 | Works | dialogs/floor_defaults.rs; Floor.settings; toolbar button, Build > Floor, Default Settings |
| 9 | Build Floors >> Floor Material Region | cap | NO SPEC (R-72) | Works | tools/details.rs DetailsVariant::FloorMaterialRegion; toolbar.rs floor() |
| 10 | Build Floors >> Hole in Floor Platform | cap | NO SPEC (R-73) | Works | tools/foundation.rs FoundationVariant::FloorHole |
| 11 | Build Floors >> Hole in Ceiling Platform | cap | NO SPEC (R-74) | Works | tools/foundation.rs FoundationVariant::CeilingHole |
| 12 | Build Floors >> Rebuild Walls/Floors/Ceilings (F12) | cap | W-91 | Works | Action::RebuildAll (toolbar.rs:974) |
| 13 | Build Floors >> Build Attic / Attic floor (not captured) | n | R-68 | Partial, In progress (Round 14) | FloorKind::Attic Gap: Not auto-created by Build Roof. |
| 14 | Build Floors >> Slab | cap | NO SPEC (R-75) | Works | tools/foundation.rs FoundationVariant::Slab; docs/manual/16-foundation-slabs.md |
| 15 | Build Floors >> Slab with Footing | cap | NO SPEC (R-76) | Works | FoundationVariant::SlabFooting; dialogs/foundation.rs |
| 16 | Build Floors >> Slab Hole | cap | NO SPEC (R-77) | Works | FoundationVariant::SlabHole, SlabHoleFooting |
| 17 | Build Floors >> Slab Hole with Footing | cap | NO SPEC (R-77) | Works | FoundationVariant::SlabHole, SlabHoleFooting |
| 18 | Build Floors >> Square Pad | cap | NO SPEC (R-78) | Works | FoundationVariant::SquarePad, RoundPier |
| 19 | Build Floors >> Round Pier | cap | NO SPEC (R-78) | Works | FoundationVariant::SquarePad, RoundPier |

### Build > Roofs

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Build Roofs >> Roof Plane | cap | RF-35, RF-36 | Works | tools/roof.rs RoofMode::Plane |
| 2 | Build Roofs >> Build Roof (dialog) | cap | RF-1, RF-2, RF-3 | Works | dialogs/roof.rs; tools/roof.rs Build Roof; scenarios/s06_roof.rs |
| 3 | Build Roofs >> Ceiling Plane | cap | RF-45, RF-46 | Works | plan-roof ceiling.rs; dialogs/roof.rs CeilingDialog |
| 4 | Build Roofs >> Build Ceiling Planes | cap | RF-45, RF-46 | Works | plan-roof ceiling.rs; dialogs/roof.rs CeilingDialog |
| 5 | Build Roofs >> Gable/Roof Line | cap | RF-44 | Works | plan-roof gable.rs; tools/roof.rs |
| 6 | Build Roofs >> Roof Hole | cap | RF-42 | Works | tools/roof.rs HolePoly (click corners, double-click, Enter or the first corner closes); roof_view.rs add_hole_polygon, polygon_self_interse… |
| 7 | Build Roofs >> Skylight (roof) | cap | RF-43 | Works | roof_view.rs skylight; 3D curb/frame/glass |
| 8 | Build Roofs >> Auto Dormer | cap | RF-48 | Works | plan-roof dormer.rs; dialogs/roof.rs; s06_roof |
| 9 | Build Roofs >> Auto Floating Dormer | cap | RF-49 | Works | roof_view.rs DormerRecord::floating |
| 10 | Build Roofs >> Edit All Roof Planes | cap | RF-39 | Works | dialogs/roof.rs AllPlanesDialog; tools/roof.rs RoofMode::EditAll; roof_view.rs AllPlanesEdit, apply_all; tests edit_all_roof_planes_applies… |
| 11 | Build Roofs >> Delete Roof Planes | cap | RF-40 | Works | tools/roof.rs |
| 12 | Build Roofs >> Delete Ceiling Planes | cap | RF-40 | Works | tools/roof.rs |
| 13 | Build Roofs >> Join Roof Planes (not captured) | n | RF-41 | Works | plan-roof join.rs; roof_view join_planes_record |
| 14 | Build Roofs >> Roof Return (Auto Roof Return) / gable returns (not captured) | n | RF-27 | Partial | roof_view.rs RF-27; Roof Return tool; dialogs/wall.rs Auto Roof Return with Length; test auto_roof_return_wraps_the_corners_of_a_gable_end… |
| 15 | Build Roofs >> Explode Dormer (not captured) | n | RF-50, RF-51 | Works | tools/roof.rs dormer_ghost (outline of the dormer under the pointer while Auto Dormer / Auto Floating Dormer is active), the Dormer Specifi… |

### Build > Framing and trim

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Build Framing >> General Framing | cap | CB-35 | Works | tools/framing.rs 19 tools |
| 2 | Build Framing >> Post | cap | CB-35 | Works | tools/framing.rs 19 tools |
| 3 | Build Framing >> Post with Footing | cap | CB-35 | Works | tools/framing.rs 19 tools |
| 4 | Build Framing >> Blocking | cap | CB-35 | Works | tools/framing.rs 19 tools |
| 5 | Build Framing >> Build Framing (Shift+Cmd+S) | cap | CB-36, CB-37 | Partial, In progress (Round 14) | Build Framing / Build All Framing commands; Framing Defaults page (`dialogs/framing.rs` FramingDefaultsDialog, `framing_view::settings`) Ga… |
| 6 | Build Framing >> Build All Framing | cap | CB-36 | Partial, In progress (Round 14) | Build Framing / Build All Framing commands; Framing Defaults page (`dialogs/framing.rs` FramingDefaultsDialog, `framing_view::settings`) Ga… |
| 7 | Build Framing >> Framing Reference Marker | cap | CB-38 | Works | plan-framing wall.rs: header table, kings/trimmers/cripples/sills, double top plates, corner and tee backing, 48" blocking |
| 8 | Build Framing >> Joist | cap | CB-39 | Works | plan-framing floor.rs: rim joists, stairwell headers and trimmers (`frame_floor_holes`) |
| 9 | Build Framing >> Joist Blocking | cap | CB-39 | Works | plan-framing floor.rs: rim joists, stairwell headers and trimmers (`frame_floor_holes`) |
| 10 | Build Framing >> Joist Direction | cap | CB-39 | Works | plan-framing floor.rs: rim joists, stairwell headers and trimmers (`frame_floor_holes`) |
| 11 | Build Framing >> Bearing Line | cap | CB-39 | Works | plan-framing floor.rs: rim joists, stairwell headers and trimmers (`frame_floor_holes`) |
| 12 | Build Framing >> Floor/Ceiling Beam | cap | CB-39 | Works | plan-framing floor.rs: rim joists, stairwell headers and trimmers (`frame_floor_holes`) |
| 13 | Build Framing >> Floor/Ceiling Truss | cap | CB-39 | Works | plan-framing floor.rs: rim joists, stairwell headers and trimmers (`frame_floor_holes`) |
| 14 | Build Framing >> Rafter | cap | RF-53 | Works | tools/framing.rs 19 tools |
| 15 | Build Framing >> Roof Beam | cap | RF-53 | Works | tools/framing.rs 19 tools |
| 16 | Build Framing >> Roof Blocking | cap | RF-53 | Works | tools/framing.rs 19 tools |
| 17 | Build Framing >> Roof Purlin | cap | RF-53 | Works | tools/framing.rs 19 tools |
| 18 | Build Framing >> Roof Truss | cap | RF-54, RF-55 | Partial, In progress (Round 14) | tools/framing.rs; plan-framing truss.rs Gap: Girder auto-doubling not confirmed. |
| 19 | Build Framing >> Girder Truss | cap | RF-54, RF-55 | Partial, In progress (Round 14) | tools/framing.rs; plan-framing truss.rs Gap: Girder auto-doubling not confirmed. |
| 20 | Build Framing >> Roof Truss Direction | cap | RF-54, RF-55 | Partial, In progress (Round 14) | tools/framing.rs; plan-framing truss.rs Gap: Girder auto-doubling not confirmed. |
| 21 | Build Framing >> Truss Base | cap | RF-54, RF-55 | Partial, In progress (Round 14) | tools/framing.rs; plan-framing truss.rs Gap: Girder auto-doubling not confirmed. |
| 22 | Build Framing >> Delete Framing (not captured) | n | CB-37 | Partial, In progress (Round 14) | rebuild replaces built; manual kept Gap: No auto-rebuild on edits; no Retain option. |
| 23 | Build Trim >> Corner Boards | cap | NO SPEC (W-109) | Works | tools/details.rs DetailsVariant::CornerBoards, AutoCornerBoards; manual 17 |
| 24 | Build Trim >> Auto Place Corner Boards | cap | NO SPEC (W-109) | Works | tools/details.rs DetailsVariant::CornerBoards, AutoCornerBoards; manual 17 |
| 25 | Build Trim >> Quoins | cap | NO SPEC (W-110) | Works | DetailsVariant::Quoins, AutoQuoins |
| 26 | Build Trim >> Auto Place Quoins | cap | NO SPEC (W-110) | Works | DetailsVariant::Quoins, AutoQuoins |
| 27 | Build Trim >> Molding Line | cap | NO SPEC (W-111) | Works | DetailsVariant::MoldingLine, MoldingPolyline; plan-3d details.rs mitering |
| 28 | Build Trim >> Molding Polyline | cap | NO SPEC (W-111) | Works | DetailsVariant::MoldingLine, MoldingPolyline; plan-3d details.rs mitering |
| 29 | Build Trim >> Fascia (not captured) | n | RF-14 | Partial | fascia and frieze come from roof planes and Roof wall options (RF-14, RF-18); shutters are a window property (DW-84); no separate trim tools |
| 30 | Build Trim >> Frieze (not captured) | n | RF-14 | Partial | fascia and frieze come from roof planes and Roof wall options (RF-14, RF-18); shutters are a window property (DW-84); no separate trim tools |
| 31 | Build Trim >> Shutters (not captured) | n | RF-14 | Partial | fascia and frieze come from roof planes and Roof wall options (RF-14, RF-18); shutters are a window property (DW-84); no separate trim tools |
| 32 | Build Trim >> Window trim (listed in the toolbar capture) (not captured) | n | RF-14 | Partial | fascia and frieze come from roof planes and Roof wall options (RF-14, RF-18); shutters are a window property (DW-84); no separate trim tools |

### Build > Stairs

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Build Stairs >> Draw Stairs (Shift+Y) | cap | CB-23, CB-24 | Works | tools/stairs.rs; plan-stairs solver |
| 2 | Build Stairs >> Click Stairs | cap | CB-23 | Works | tools/stairs.rs; plan-stairs solver |
| 3 | Build Stairs >> Straight Stairs | cap | CB-25 | Works | StairShape |
| 4 | Build Stairs >> L-Shaped Stair | cap | CB-25 | Works | StairShape |
| 5 | Build Stairs >> U-Shaped Stair | cap | CB-25 | Works | StairShape |
| 6 | Build Stairs >> Curve to Left | cap | CB-26 | Partial, In progress (Round 14) | StairShape::Curved; winders Gap: Flared apron of Flare/Curve open. |
| 7 | Build Stairs >> Curve to Right | cap | CB-26 | Partial, In progress (Round 14) | StairShape::Curved; winders Gap: Flared apron of Flare/Curve open. |
| 8 | Build Stairs >> Curved Stairs | cap | CB-26 | Partial, In progress (Round 14) | StairShape::Curved; winders Gap: Flared apron of Flare/Curve open. |
| 9 | Build Stairs >> Spiral Stairs (not captured) | n | CB-26 | Partial, In progress (Round 14) | StairShape::Curved; winders Gap: Flared apron of Flare/Curve open. |
| 10 | Build Stairs >> Landing | cap | CB-27 | Works | stairs_view.rs connect |
| 11 | Build Stairs >> Draw Ramp | cap | CB-34 | Works | plan-stairs |
| 12 | Build Stairs >> Auto Stairwell | cap | CB-29, CB-30 | Partial, In progress (Round 14) | QA-04 fixed; PlatformHole Gap: Guard railings around opening open. |
| 13 | Build Stairs >> Add Stair Breakline | cap | CB-33 | Partial, In progress (Round 14) | stairs_view.rs Gap: Dashed hidden treads on other floors open. |
| 14 | Build Stairs >> Remove Stair Breakline | cap | CB-33 | Partial, In progress (Round 14) | stairs_view.rs Gap: Dashed hidden treads on other floors open. |
| 15 | Build Stairs >> Flare Stairs (bullnose, flared apron) | cap | CB-26 | Partial, In progress (Round 14) | StairShape::Curved; winders Gap: Flared apron of Flare/Curve open. |
| 16 | Build Stairs >> Curve Stairs (curve an existing run) | cap | CB-26 | Partial, In progress (Round 14) | StairShape::Curved; winders Gap: Flared apron of Flare/Curve open. |
| 17 | Build Stairs >> Elevator (listed in the toolbar capture) | cap | NO SPEC (CB-69) | Missing | no elevator tool or symbol (grep Elevator in crates finds nothing) |

### Build > Cabinets

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Build Cabinets >> Base Cabinet (Shift+T) | cap | CB-1, CB-2, CB-6 | Works | toolbar.rs Cabinet flyout (16 kinds); tools/cabinet.rs |
| 2 | Build Cabinets >> Wall Cabinet (Cmd+T) | cap | CB-1, CB-6 | Works | toolbar.rs Cabinet flyout (16 kinds); tools/cabinet.rs |
| 3 | Build Cabinets >> Full Height Cabinet | cap | CB-1 | Works | toolbar.rs Cabinet flyout (16 kinds); tools/cabinet.rs |
| 4 | Build Cabinets >> Soffit (T) | cap | CB-17 | Partial, In progress (Round 14) | Soffit kind Gap: Cabinet-like box, not polyline soffit. |
| 5 | Build Cabinets >> Shelf | cap | CB-18 | Works | CabinetKind |
| 6 | Build Cabinets >> Partition | cap | CB-18 | Works | CabinetKind |
| 7 | Build Cabinets >> Base Filler | cap | CB-19 | Works | tools/cabinet.rs |
| 8 | Build Cabinets >> Wall Filler | cap | CB-19 | Works | tools/cabinet.rs |
| 9 | Build Cabinets >> Full Height Filler | cap | CB-19 | Works | tools/cabinet.rs |
| 10 | Build Cabinets >> Custom Countertop | cap | CB-15 | Works | tools/cabinet.rs |
| 11 | Build Cabinets >> Custom Backsplash | cap | CB-15 | Works | tools/cabinet.rs |
| 12 | Build Cabinets >> Custom Counter Hole | cap | CB-15 | Works | tools/cabinet.rs |
| 13 | Build Cabinets >> Corner Base Cabinet (not captured) | n | NO SPEC (CB-70) | Works | toolbar.rs cabinet() "Corner Base Cabinet", "Corner Wall Cabinet"; plan-cabinets kinds |
| 14 | Build Cabinets >> Corner Wall Cabinet (not captured) | n | NO SPEC (CB-70) | Works | toolbar.rs cabinet() "Corner Base Cabinet", "Corner Wall Cabinet"; plan-cabinets kinds |
| 15 | Build Cabinets >> Blind Base Cabinet (not captured) | n | NO SPEC (CB-71) | Works | toolbar.rs cabinet() "Blind Base Cabinet", "Blind Wall Cabinet" |
| 16 | Build Cabinets >> Blind Wall Cabinet (not captured) | n | NO SPEC (CB-71) | Works | toolbar.rs cabinet() "Blind Base Cabinet", "Blind Wall Cabinet" |
| 17 | Build Cabinets >> Vanity preset (not captured) | n | CB-16 | Works | plan_cabinets::CabinetPreset (toolbar.rs cabinet()) |
| 18 | Build Cabinets >> Pantry preset (not captured) | n | CB-16 | Works | plan_cabinets::CabinetPreset (toolbar.rs cabinet()) |
| 19 | Build Cabinets >> Tall Oven preset (not captured) | n | CB-16 | Works | plan_cabinets::CabinetPreset (toolbar.rs cabinet()) |
| 20 | Build Cabinets >> Refrigerator surround preset (not captured) | n | CB-16 | Works | plan_cabinets::CabinetPreset (toolbar.rs cabinet()) |
| 21 | Build Cabinets >> Cabinet auto-fill, labels, schedule, defaults (not captured) | n | CB-5, CB-13, CB-20, CB-21 | Works | Fillers fill the gap; a cabinet dragged into a gap within 2" of its width takes the gap (`plan_cabinets::fit_to_gap`, `tools/cabinet.rs::fi… |

### Build > Electrical

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Build Electrical >> 110V Outlet (E,O) | cap | E-1, E-3 | Works | `plan-electrical/src/symbol.rs`; `tests::flags_voltage_and_symbols_of_the_outlet_kinds`; scenario `the_flag_symbols_are_drawn_in_the_plan` |
| 2 | Build Electrical >> 220V Outlet | cap | E-1, E-2 | Works | `plan-electrical/src/symbol.rs`; `tests::flags_voltage_and_symbols_of_the_outlet_kinds`; scenario `the_flag_symbols_are_drawn_in_the_plan` |
| 3 | Build Electrical >> GFCI Outlet | cap | E-1, E-2 | Works | `plan-electrical/src/symbol.rs`; `tests::flags_voltage_and_symbols_of_the_outlet_kinds`; scenario `the_flag_symbols_are_drawn_in_the_plan` |
| 4 | Build Electrical >> WP Outlet | cap | E-1, E-2 | Works | `plan-electrical/src/symbol.rs`; `tests::flags_voltage_and_symbols_of_the_outlet_kinds`; scenario `the_flag_symbols_are_drawn_in_the_plan` |
| 5 | Build Electrical >> Dedicated Outlet | cap | E-1, E-2 | Works | `plan-electrical/src/symbol.rs`; `tests::flags_voltage_and_symbols_of_the_outlet_kinds`; scenario `the_flag_symbols_are_drawn_in_the_plan` |
| 6 | Build Electrical >> Switch (E,S) | cap | E-4 | Works | `place::auto_place_switch`, `tools/electrical.rs::auto_place_floor_switches` (round 13) |
| 7 | Build Electrical >> Dimmer Switch | cap | E-4 | Works | `place::auto_place_switch`, `tools/electrical.rs::auto_place_floor_switches` (round 13) |
| 8 | Build Electrical >> Light (E,L) | cap | CB-66 | Works | Light, Recessed, Pendant, Wall, Rope, fans, detectors, thermostat, doorbell, jacks; 3D fixture meshes (`plan_electrical::electrical_meshes`… |
| 9 | Build Electrical >> Recessed Light | cap | CB-66 | Works | Light, Recessed, Pendant, Wall, Rope, fans, detectors, thermostat, doorbell, jacks; 3D fixture meshes (`plan_electrical::electrical_meshes`… |
| 10 | Build Electrical >> Pendant Light | cap | CB-66 | Works | Light, Recessed, Pendant, Wall, Rope, fans, detectors, thermostat, doorbell, jacks; 3D fixture meshes (`plan_electrical::electrical_meshes`… |
| 11 | Build Electrical >> Wall Light | cap | CB-66 | Works | Light, Recessed, Pendant, Wall, Rope, fans, detectors, thermostat, doorbell, jacks; 3D fixture meshes (`plan_electrical::electrical_meshes`… |
| 12 | Build Electrical >> Rope Light | cap | CB-66 | Works | Light, Recessed, Pendant, Wall, Rope, fans, detectors, thermostat, doorbell, jacks; 3D fixture meshes (`plan_electrical::electrical_meshes`… |
| 13 | Build Electrical >> Electrical Connection (E,C) | cap | E-5, E-6 | Works | `layer::connect_in`; s08 `a_light_snaps_to_the_room_center_and_a_switch_is_connected_to_it` |
| 14 | Build Electrical >> Auto Place Outlets (E,A,O) | cap | E-7, E-8, E-9, E-10 | Works | `place::auto_place_outlets`; `tests::outlets_on_the_40x30_shell_follow_the_nec_rules` |
| 15 | Build Electrical >> Ceiling Fan (not captured) | n | NO SPEC (E-17) | Works | toolbar.rs electrical() entries (E::CeilingFan ... E::Panel); plan-electrical DeviceKind |
| 16 | Build Electrical >> Smoke Detector (not captured) | n | NO SPEC (E-17) | Works | toolbar.rs electrical() entries (E::CeilingFan ... E::Panel); plan-electrical DeviceKind |
| 17 | Build Electrical >> CO Detector (not captured) | n | NO SPEC (E-17) | Works | toolbar.rs electrical() entries (E::CeilingFan ... E::Panel); plan-electrical DeviceKind |
| 18 | Build Electrical >> Thermostat (not captured) | n | NO SPEC (E-17) | Works | toolbar.rs electrical() entries (E::CeilingFan ... E::Panel); plan-electrical DeviceKind |
| 19 | Build Electrical >> Doorbell (not captured) | n | NO SPEC (E-17) | Works | toolbar.rs electrical() entries (E::CeilingFan ... E::Panel); plan-electrical DeviceKind |
| 20 | Build Electrical >> Data Jack (not captured) | n | NO SPEC (E-17) | Works | toolbar.rs electrical() entries (E::CeilingFan ... E::Panel); plan-electrical DeviceKind |
| 21 | Build Electrical >> Phone Jack (not captured) | n | NO SPEC (E-17) | Works | toolbar.rs electrical() entries (E::CeilingFan ... E::Panel); plan-electrical DeviceKind |
| 22 | Build Electrical >> TV Jack (not captured) | n | NO SPEC (E-17) | Works | toolbar.rs electrical() entries (E::CeilingFan ... E::Panel); plan-electrical DeviceKind |
| 23 | Build Electrical >> Electrical Panel (not captured) | n | NO SPEC (E-17) | Works | toolbar.rs electrical() entries (E::CeilingFan ... E::Panel); plan-electrical DeviceKind |

### Build > 3D solid, images, distributed objects

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Build Solids >> 3D Solid | cap | NO SPEC (C-72) | Works | tools/details.rs DetailsVariant::Solid3d, Face, Cone, Cylinder, Pyramid, Sphere; toolbar.rs solid_3d() |
| 2 | Build Solids >> Face | cap | NO SPEC (C-72) | Works | tools/details.rs DetailsVariant::Solid3d, Face, Cone, Cylinder, Pyramid, Sphere; toolbar.rs solid_3d() |
| 3 | Build Solids >> Cone | cap | NO SPEC (C-72) | Works | tools/details.rs DetailsVariant::Solid3d, Face, Cone, Cylinder, Pyramid, Sphere; toolbar.rs solid_3d() |
| 4 | Build Solids >> Cylinder | cap | NO SPEC (C-72) | Works | tools/details.rs DetailsVariant::Solid3d, Face, Cone, Cylinder, Pyramid, Sphere; toolbar.rs solid_3d() |
| 5 | Build Solids >> Pyramid | cap | NO SPEC (C-72) | Works | tools/details.rs DetailsVariant::Solid3d, Face, Cone, Cylinder, Pyramid, Sphere; toolbar.rs solid_3d() |
| 6 | Build Solids >> Sphere | cap | NO SPEC (C-72) | Works | tools/details.rs DetailsVariant::Solid3d, Face, Cone, Cylinder, Pyramid, Sphere; toolbar.rs solid_3d() |
| 7 | Build Solids >> 3D Solid Feature (custom molding profile) | cap | NO SPEC (CB-72) | Partial | tools/images.rs ImageMode::SolidFeature; no profile editor (parity-status "Details" open items) |
| 8 | Build Images >> Create Image (picture box) | cap | NO SPEC (CAD-47) | Works | tools/images.rs ImageMode::CreateImage; dialogs/images.rs |
| 9 | Build Images >> Create Billboard Image | cap | NO SPEC (CAD-48) | Works | ImageMode::BillboardImage |
| 10 | Build Images >> Create Image Library item | cap | NO SPEC (CAD-49) | Partial | ImageMode::ImageLibrary stores an image as a user-library symbol; round 14 details builder in flight |
| 11 | Build Images >> Point to Point Resize (not captured) | n | NO SPEC (CAD-50) | Works | ImageMode::PointToPointResize, RotateToAlign; tools/underlay.rs |
| 12 | Build Images >> Rotate to Align (picture tracing) (not captured) | n | NO SPEC (CAD-50) | Works | ImageMode::PointToPointResize, RotateToAlign; tools/underlay.rs |
| 13 | Build Distributed >> Polyline Distribution Path | cap | NO SPEC (CAD-51) | Works | ImageMode::PolylinePath, SplinePath; dialogs/images.rs DIST |
| 14 | Build Distributed >> Spline Distribution Path | cap | NO SPEC (CAD-51) | Works | ImageMode::PolylinePath, SplinePath; dialogs/images.rs DIST |
| 15 | Build Distributed >> Polyline Distribution Region | cap | NO SPEC (CAD-52) | Works | ImageMode::PolylineRegion, SplineRegion |
| 16 | Build Distributed >> Spline Distribution Region | cap | NO SPEC (CAD-52) | Works | ImageMode::PolylineRegion, SplineRegion |

### Terrain menu

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Terrain >> Create Terrain Perimeter | cap | CB-43 | Works | tools/terrain.rs; plan-terrain `build_terrain_with_progress`; site_view `build_surface_with_progress` |
| 2 | Terrain >> Terrain Specification… | cap | CB-51 | Works | dialogs/terrain.rs (General, Contours, Building Pad, Materials, Layer; Cut and Fill Report and Import Terrain Data forms); `TerrainRecord::… |
| 3 | Terrain >> Build Terrain | cap | CB-43 | Works | tools/terrain.rs; plan-terrain `build_terrain_with_progress`; site_view `build_surface_with_progress` |
| 4 | Terrain >> Clear Terrain | cap | CB-43 | Works | tools/terrain.rs; plan-terrain `build_terrain_with_progress`; site_view `build_surface_with_progress` |
| 5 | Terrain >> Elevation Line | cap | CB-44 | Works | TerrainVariant; `ElevationLine::{spline, reflatten}`; dialogs/terrain/object.rs Elevation Point/Line/Region Specification; tests `elevation… |
| 6 | Terrain >> Elevation Point | cap | CB-44 | Works | TerrainVariant; `ElevationLine::{spline, reflatten}`; dialogs/terrain/object.rs Elevation Point/Line/Region Specification; tests `elevation… |
| 7 | Terrain >> Elevation Region | cap | CB-44 | Works | TerrainVariant; `ElevationLine::{spline, reflatten}`; dialogs/terrain/object.rs Elevation Point/Line/Region Specification; tests `elevation… |
| 8 | Terrain >> Elevation Spline | cap | CB-44 | Works | TerrainVariant; `ElevationLine::{spline, reflatten}`; dialogs/terrain/object.rs Elevation Point/Line/Region Specification; tests `elevation… |
| 9 | Terrain >> Terrain Break | cap | CB-44 | Works | TerrainVariant; `ElevationLine::{spline, reflatten}`; dialogs/terrain/object.rs Elevation Point/Line/Region Specification; tests `elevation… |
| 10 | Terrain >> Hill | cap | CB-45 | Works | Hill, Valley, Raised, Lowered, Flat; Hill/Valley/Raised Region/Lowered Region/Flat Region Specification (dialogs/terrain/object.rs) |
| 11 | Terrain >> Valley | cap | CB-45 | Works | Hill, Valley, Raised, Lowered, Flat; Hill/Valley/Raised Region/Lowered Region/Flat Region Specification (dialogs/terrain/object.rs) |
| 12 | Terrain >> Raised Region | cap | CB-45 | Works | Hill, Valley, Raised, Lowered, Flat; Hill/Valley/Raised Region/Lowered Region/Flat Region Specification (dialogs/terrain/object.rs) |
| 13 | Terrain >> Lowered Region | cap | CB-45 | Works | Hill, Valley, Raised, Lowered, Flat; Hill/Valley/Raised Region/Lowered Region/Flat Region Specification (dialogs/terrain/object.rs) |
| 14 | Terrain >> Flat Region (Cut/Fill) | cap | CB-45 | Works | Hill, Valley, Raised, Lowered, Flat; Hill/Valley/Raised Region/Lowered Region/Flat Region Specification (dialogs/terrain/object.rs) |
| 15 | Terrain >> Rectangular Feature | cap | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 16 | Terrain >> Kidney Shaped Feature | cap | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 17 | Terrain >> Spline Feature | cap | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 18 | Terrain >> Polyline Feature | cap | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 19 | Terrain >> Round Feature | cap | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 20 | Terrain >> Terrain Hole | cap | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 21 | Terrain >> Polyline Garden Bed | cap | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |
| 22 | Terrain >> Kidney Garden Bed | cap | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |
| 23 | Terrain >> Spline Garden Bed | cap | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |
| 24 | Terrain >> Polyline Grass Region | cap | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |
| 25 | Terrain >> Kidney Grass Region | cap | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |
| 26 | Terrain >> Spline Grass Region | cap | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |
| 27 | Terrain >> Polyline Water Feature | cap | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |
| 28 | Terrain >> Spline Water Feature | cap | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |
| 29 | Terrain >> Polyline Stepping Stone | cap | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |
| 30 | Terrain >> Spline Stepping Stone | cap | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |
| 31 | Terrain >> Straight Terrain Wall | cap | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 32 | Terrain >> Straight Terrain Curb | cap | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 33 | Terrain >> Curved Terrain Wall | cap | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 34 | Terrain >> Curved Terrain Curb | cap | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 35 | Terrain >> Polyline Road | cap | CB-48 | Works | tools/terrain.rs scape.rs; mesh.rs `road_meshes`; RoadKind::Marking; tests `a_road_marking_lies_on_the_crown_of_the_road_it_crosses`, s36 `… |
| 36 | Terrain >> Spline Road | cap | CB-48 | Works | tools/terrain.rs scape.rs; mesh.rs `road_meshes`; RoadKind::Marking; tests `a_road_marking_lies_on_the_crown_of_the_road_it_crosses`, s36 `… |
| 37 | Terrain >> Polyline Road Marking | cap | CB-48 | Works | tools/terrain.rs scape.rs; mesh.rs `road_meshes`; RoadKind::Marking; tests `a_road_marking_lies_on_the_crown_of_the_road_it_crosses`, s36 `… |
| 38 | Terrain >> Spline Road Marking | cap | CB-48 | Works | tools/terrain.rs scape.rs; mesh.rs `road_meshes`; RoadKind::Marking; tests `a_road_marking_lies_on_the_crown_of_the_road_it_crosses`, s36 `… |
| 39 | Terrain >> Polyline Driveway | cap | CB-48 | Works | tools/terrain.rs scape.rs; mesh.rs `road_meshes`; RoadKind::Marking; tests `a_road_marking_lies_on_the_crown_of_the_road_it_crosses`, s36 `… |
| 40 | Terrain >> Spline Driveway | cap | CB-48 | Works | tools/terrain.rs scape.rs; mesh.rs `road_meshes`; RoadKind::Marking; tests `a_road_marking_lies_on_the_crown_of_the_road_it_crosses`, s36 `… |
| 41 | Terrain >> Polyline Sidewalk | cap | CB-48 | Works | tools/terrain.rs scape.rs; mesh.rs `road_meshes`; RoadKind::Marking; tests `a_road_marking_lies_on_the_crown_of_the_road_it_crosses`, s36 `… |
| 42 | Terrain >> Spline Sidewalk | cap | CB-48 | Works | tools/terrain.rs scape.rs; mesh.rs `road_meshes`; RoadKind::Marking; tests `a_road_marking_lies_on_the_crown_of_the_road_it_crosses`, s36 `… |
| 43 | Terrain >> Plant (single) | cap | CB-50 | Partial, In progress (Round 14) | plant runs (`Landscape`); Plant Chooser (`tools/terrain/scape.rs plant_categories, plants_in`; dialogs/terrain/object.rs `plant_chooser`);… |
| 44 | Terrain >> Polyline Plant | cap | CB-50 | Partial, In progress (Round 14) | plant runs (`Landscape`); Plant Chooser (`tools/terrain/scape.rs plant_categories, plants_in`; dialogs/terrain/object.rs `plant_chooser`);… |
| 45 | Terrain >> Spline Plant | cap | CB-50 | Partial, In progress (Round 14) | plant runs (`Landscape`); Plant Chooser (`tools/terrain/scape.rs plant_categories, plants_in`; dialogs/terrain/object.rs `plant_chooser`);… |
| 46 | Terrain >> Polyline Sprinkler | cap | CB-50 | Partial, In progress (Round 14) | plant runs (`Landscape`); Plant Chooser (`tools/terrain/scape.rs plant_categories, plants_in`; dialogs/terrain/object.rs `plant_chooser`);… |
| 47 | Terrain >> Spline Sprinkler | cap | CB-50 | Partial, In progress (Round 14) | plant runs (`Landscape`); Plant Chooser (`tools/terrain/scape.rs plant_categories, plants_in`; dialogs/terrain/object.rs `plant_chooser`);… |
| 48 | Terrain >> Make Terrain Hole Around Building(s) | cap | CB-47 | Works | site_view::auto_building_hole and auto_building_pad; TerrainVariant::BuildingPad (toolbar.rs:1588 Building Pad entry); the pad is levelled… |
| 49 | Terrain >> Place Terrain Elevation Reference Point | cap | CB-47 | Works | site_view::auto_building_hole and auto_building_pad; TerrainVariant::BuildingPad (toolbar.rs:1588 Building Pad entry); the pad is levelled… |
| 50 | Terrain >> Remove Terrain Elevation Reference Point | cap | CB-47 | Works | site_view::auto_building_hole and auto_building_pad; TerrainVariant::BuildingPad (toolbar.rs:1588 Building Pad entry); the pad is levelled… |
| 51 | Terrain >> Plant Chooser (Plant Chooser dialog) (not captured) | n | NO SPEC (CB-73) | Partial | dialogs/terrain/object.rs "Plant Chooser" window over plan-library plant catalog (catalog_plants.rs); no hardiness zone, sun or water filter |
| 52 | Terrain >> Hardiness zones / plant growth size by age (not captured) | n | NO SPEC (CB-74) | Missing | no hardiness or growth-age data (grep finds nothing) |
| 53 | Terrain >> Import terrain data (DXF points, text, GPX) (not captured) | n | NO SPEC (CB-75) | Works | plan-terrain import.rs; toolbar.rs site_objects() "Import Terrain Data…" |
| 54 | Terrain >> Terrain Cut and Fill Report (not captured) | n | NO SPEC (CB-76) | Works | plan-terrain; dialogs/terrain.rs REPORT_TABS; plan-docs terrain_report.rs |
| 55 | Terrain >> Building Pad (not captured) | n | NO SPEC (CB-77) | Works | toolbar.rs site_objects() "Building Pad"; dialogs/terrain.rs Building Pad tab |
| 56 | Terrain >> North Pointer / Scale Bar (site plan symbols) | cap | L-14 | Partial | plan view box with terrain layers; the construction set has a Site Plan sheet (first floor plan at up to 1/8 in) Gap: North Pointer and Sca… |
| 57 | Terrain >> Contour lines and labels (not captured) | n | CB-52 | Works | site_view terrain_feature_meshes; manual 9.7 |
| 58 | Terrain >> Terrain pond / pool (water feature depth) (not captured) | n | CB-49 | Works | landscape.rs; landscape_plan.rs `ripple_lines` |

### Library menu

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Library >> Import Library (.calib, .calibz)… | cap | CB-59 | Works | tools/library/chief.rs register_catalog (a .calib or .calibz is added read-only and read in place, under the User Catalog node), user.rs import_library_file (Plan Studio JSON); DECISIONS.md #3 and LR1: Chief content is never copied |
| 2 | Library >> Get Additional Content… | cap | NO SPEC (CB-78) | Missing | not in menus.rs (needs Chief's online content service) |
| 3 | Library >> Install Core Content | cap | NO SPEC (CB-79) | Differs-by-design | Plan Studio reads the user's installed Chief catalogs at runtime and ships its own starter catalog; it never installs or bundles Chief cont… |
| 4 | Library >> Update Library Catalogs | cap | NO SPEC (CB-80) | Partial | Library > Catalog Settings… rescans the folder; no automatic update check (CB-53) |
| 5 | Library >> Library Browser (Cmd+L) | cap | CB-53, CB-54, CB-61 | Works | shell/library_panel.rs, shell/library_browser.rs |
| 6 | Library >> Add selection to User Library (not captured) | n | CB-58 | Works | tools/library/user.rs, make.rs; ~/.plan-studio/user-library.json |
| 7 | Library >> Add Active Material to Library (not captured) | n | CB-58 | Works | tools/library/user.rs, make.rs; ~/.plan-studio/user-library.json |
| 8 | Library >> Import 3D Model (OBJ, glTF) (not captured) | n | NO SPEC (CB-81) | Works | menus.rs; tools/library/make.rs; plan-import obj.rs, gltf.rs |
| 9 | Library >> Import 3D Model (3DS, SKP, DAE, STL formats) (not captured) | n | NO SPEC (CB-82) | Missing | plan-import has OBJ and glTF only |
| 10 | Library >> Library items with 3D models / catalog link | cap | CB-60 | Partial | plan-calib decoded meshes; user models (OBJ, glTF) in tools/library/user.rs placed_meshes Gap: Imported models carry a mesh, unit/up-axis o… |
| 11 | Library >> Manufacturer catalogs, 3D Warehouse, bonus catalogs (not captured) | n | NO SPEC (CB-83) | Missing | no manufacturer content service; 3D Warehouse login is out of scope |
| 12 | Library >> Library Browser: search, favorites, list/thumbnail (not captured) | n | CB-54, CB-61 | Works | plan-library browse.rs and types.rs (twelve-type filter), shell/library_browser/user_ui.rs (List, Grid, Names), library_panel/thumbs.rs (path-traced cached thumbnails) |
| 13 | Library >> Replace From Library, Library Data in dialogs (Library… buttons) (not captured) | n | CB-57 | Works | tools/library/convert.rs replace_selected (the whole selection, one undo step); user.rs replace_other (cabinet, device) |

### 3D menu

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | 3D >> Orthographic Full / Floor Overview, elevations and plan overhead views | cap | C-15, C-16 | Partial, In progress (Round 14) | Orthographic Full Overview + 4 elevations Gap: No floor/framing ortho overview, no isometric SW/SE/NE/NW. |
| 2 | 3D >> Full Camera (Shift+J) | cap | C-4, C-5, C-7 | Works | tools/camera.rs |
| 3 | 3D >> Perspective Full Overview (Shift+K) | cap | C-10 | Works | toolbar.rs V::FullOverview (Shift+K) |
| 4 | 3D >> Perspective Floor Overview | cap | C-11 | Works | toolbar.rs:1720 |
| 5 | 3D >> Perspective Framing Overview | cap | C-12 | Missing, In progress (Round 14) | no entry in toolbar.rs Gap: Framing members only via manual framing in 3D. |
| 6 | 3D >> Create Auto Elevations ▸ (exterior, back-clipped, interior) | cap | C-21 | Works | manual 10.11; tools/camera.rs |
| 7 | 3D >> Cross Section/Elevation Camera | cap | C-17, C-18 | Works | tools/camera.rs; `plan_elevation::section_free` (free-angle `FreeView`); dialogs/camera.rs `free_view` |
| 8 | 3D >> Back-Clipped Cross Section Camera | cap | C-19 | Works | tools/camera.rs; clip handle |
| 9 | 3D >> Wall Elevation Camera | cap | C-20 | Works | manual 10.11; `wall_elevation` + `free_view` |
| 10 | 3D >> Doll House View | cap | C-13 | Works | toolbar.rs DollHouse |
| 11 | 3D >> Move Camera with Mouse ▸ (Mouse-Orbit Camera) | cap | C-34 | Partial, In progress (Round 14) | view3d_panel.rs gestures Gap: No explicit Move Camera tool choices. |
| 12 | 3D >> Move Camera with Keyboard ▸ | cap | C-38 | Partial, In progress (Round 14) | WASD/arrows/PgUp/PgDn in Full Camera; 3D > Move Camera with Keyboard steps (`nudge.rs`) Gap: The menu steps move or turn by fixed amounts (… |
| 13 | 3D >> Move Camera ▸ / Orbit Camera ▸ / Tilt Camera ▸ (menu steps) | cap | C-34, C-35, C-40 | Partial, In progress (Round 14) | view3d_panel.rs gestures Gap: No explicit Move Camera tool choices. |
| 14 | 3D >> View Direction ▸ | cap | C-41 | Missing, In progress (Round 14) | 3D > View Direction (eight compass snaps, `nudge.rs` test `view_direction_snaps_the_yaw_and_keeps_the_target`) Gap: Applies to the overview… |
| 15 | 3D >> Isometric Views ▸ | cap | C-15 | Partial, In progress (Round 14) | Orthographic Full Overview + 4 elevations Gap: No floor/framing ortho overview, no isometric SW/SE/NE/NW. |
| 16 | 3D >> Walkthroughs ▸ (Create Path, Play, Record) | cap | C-71 | Works | tools/camera.rs; Play/Record |
| 17 | 3D >> Material Painter ▸ (Component, Object, Room, Floor, Plan modes) | cap | C-56, C-60 | Works | tools/materials.rs Paint, Eyedropper and Erase modes through the 3D pick hook; Adjust Materials per part; painted_textures shows a painted… |
| 18 | 3D >> Materials ▸ Material Eyedropper | cap | C-57 | Works | tools/materials.rs PainterMode::Eyedropper; tests eyedropper_and_delete_surface, s23 the_material_painter_overrides_a_wall_in_3d_through_th… |
| 19 | 3D >> Materials ▸ Object Eyedropper | cap | C-57 | Works | tools/materials.rs PainterMode::Eyedropper; tests eyedropper_and_delete_surface, s23 the_material_painter_overrides_a_wall_in_3d_through_th… |
| 20 | 3D >> Adjust Materials ▸ Adjust Material Definition | cap | C-58 | Partial, In progress (Round 14) | tools/materials.rs ADJUST (Adjust Materials: a material per part of the selected object) and BUILDER (edits a user material's color, roughn… |
| 21 | 3D >> Adjust 3D Cladding ▸ | cap | NO SPEC (C-73) | Missing | no 3D cladding adjust tool (siding start point, course offset) |
| 22 | 3D >> Material Builder… | cap | C-61 | Works | tools/materials.rs builder_window (color, roughness, metallic, transparency, pattern, texture file) saves to ~/.plan-studio/materials.json;… |
| 23 | 3D >> Add Lights | cap | C-64 | Partial, In progress (Round 14) | Project.lights; Adjust Lights dialog Gap: Point lights only; no auto-place or color temperature. |
| 24 | 3D >> Adjust Lights | cap | C-64, C-66 | Partial, In progress (Round 14) | Project.lights; Adjust Lights dialog Gap: Point lights only; no auto-place or color temperature. |
| 25 | 3D >> Adjust Sunlight | cap | C-63 | Partial, In progress (Round 14) | Sun Angle window (date, time, latitude, azimuth/altitude) Gap: No longitude, DST, north direction; no Move Sun/Moon. |
| 26 | 3D >> Move Sun | cap | C-63 | Partial, In progress (Round 14) | Sun Angle window (date, time, latitude, azimuth/altitude) Gap: No longitude, DST, north direction; no Move Sun/Moon. |
| 27 | 3D >> Move Moon | cap | C-63 | Partial, In progress (Round 14) | Sun Angle window (date, time, latitude, azimuth/altitude) Gap: No longitude, DST, north direction; no Move Sun/Moon. |
| 28 | 3D >> Toggle Sunlight | cap | C-63 | Partial, In progress (Round 14) | Sun Angle window (date, time, latitude, azimuth/altitude) Gap: No longitude, DST, north direction; no Move Sun/Moon. |
| 29 | 3D >> Default lighting and light sets | cap | C-62, C-65 | Partial, In progress (Round 14) | manual 10.6: not built |
| 30 | 3D >> Camera View Options ▸ | cap | C-31, C-32 | Partial, In progress (Round 14) | dialogs/camera.rs Rendering tab, elevation options, `CameraObject.vector` Gap: Per camera: hatch, shadows, depth weights, labels, level cal… |
| 31 | 3D >> Rendering Techniques ▸ | cap | C-45, C-46 | Partial, In progress (Round 14) | plan-materials nine techniques mapped to `plan_view3d::Look`; manual 10.4 Gap: All nine draw in GL (Technical Illustration and Line Drawing… |
| 32 | 3D >> Toggle Patterns | cap | NO SPEC (C-74) | Missing | shown dimmed in Chief's menu; no toggle between material patterns and textures in 3D (grep finds nothing) |
| 33 | 3D >> Delete Surface | cap | C-55 | Works | tools/materials.rs PainterMode::Erase (Delete Surface); s23 the_material_painter_overrides_a_wall_in_3d_through_the_pick_hook; tools/materi… |
| 34 | 3D >> Rebuild 3D | cap | C-54 | Works | menus.rs:547 |
| 35 | 3D >> 3D View Defaults… (Cmd+1) | cap | C-68 | Partial, In progress (Round 14) | dialogs/camera.rs Gap: Eye height, angle, technique only. |
| 36 | 3D >> Ray Trace… (render image to file) (not captured) | n | C-51 | Works | plan-render; Ray Trace window |
| 37 | 3D >> Show Doors Open / Casing, Jambs and Sills toggles (not captured) | n | C-44 | Works | menus.rs three_d_menu; DW-79..DW-85 3D casing work |

### CAD menu

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | CAD >> Current CAD Layer… | cap | CAD-1, LAY-6 | Works | plan-core layers.rs current_cad_layer; tools/cad.rs draw_layer; toolbar.rs:2136 Current CAD Layer button opens Active Layers by Tool (dialo… |
| 2 | CAD >> Place Point | cap | CAD-2 | Works | CadMode::PlacePoint etc. |
| 3 | CAD >> Input Point | cap | CAD-2 | Works | CadMode::PlacePoint etc. |
| 4 | CAD >> Point Marker | cap | CAD-2 | Works | CadMode::PlacePoint etc. |
| 5 | CAD >> Delete Temporary Points | cap | CAD-2 | Works | CadMode::PlacePoint etc. |
| 6 | CAD >> Draw Line | cap | CAD-3 | Works | CadMode::Line; Shift+F8 toggle |
| 7 | CAD >> Input Line | cap | CAD-5, CAD-39 | Works | CadMode::InputLine |
| 8 | CAD >> Line With Arrow | cap | CAD-6 | Works | CadMode::LineArrow; Add Arrow Backoff Point |
| 9 | CAD >> Polyline | cap | CAD-15, CAD-16 | Works | CadMode::Polyline |
| 10 | CAD >> Draw Arc | cap | CAD-7, CAD-9 | Partial, In progress (Round 14) | CadMode::Arc option strip (three-point, center-start-end, tangent) Gap: Start-end-radius mode absent; Edit > Arc Creation Modes inert. |
| 11 | CAD >> Input Arc | cap | CAD-8 | Works | CadMode::InputArc |
| 12 | CAD >> Arc With Arrow | cap | CAD-6, CAD-7 | Partial, In progress (Round 14) | CadMode::Arc option strip (three-point, center-start-end, tangent) Gap: Start-end-radius mode absent; Edit > Arc Creation Modes inert. |
| 13 | CAD >> Circle (K) | cap | CAD-11 | Works | CadMode variants |
| 14 | CAD >> Circle About Center | cap | CAD-11 | Works | CadMode variants |
| 15 | CAD >> Ellipse | cap | CAD-11 | Works | CadMode variants |
| 16 | CAD >> Oval | cap | CAD-11 | Works | CadMode variants |
| 17 | CAD >> Rectangular Polyline (Shift+P) | cap | CAD-12 | Works | CadMode; edit_tests |
| 18 | CAD >> Box | cap | CAD-12 | Works | CadMode; edit_tests |
| 19 | CAD >> Regular Polygon | cap | CAD-12 | Works | CadMode; edit_tests |
| 20 | CAD >> Cross Box | cap | CAD-12 | Works | CadMode; edit_tests |
| 21 | CAD >> Blocking Box | cap | CAD-12 | Works | CadMode; edit_tests |
| 22 | CAD >> Insulation | cap | CAD-12 | Works | CadMode; edit_tests |
| 23 | CAD >> Revision Cloud | cap | CAD-36 | Works | CadMode::RevisionCloud |
| 24 | CAD >> Spline | cap | CAD-29, CAD-30 | Works | CadMode::Spline |
| 25 | CAD >> Dimensions ▸ Manual Dimension | cap | DIM-11 | Works | tools/dimension.rs DimMode::Manual |
| 26 | CAD >> Dimensions ▸ End to End | cap | DIM-13 | Works | DimMode::EndToEnd |
| 27 | CAD >> Dimensions ▸ Interior | cap | DIM-14 | Works | DimMode::Interior |
| 28 | CAD >> Dimensions ▸ Point to Point | cap | DIM-15 | Works | DimMode::PointToPoint |
| 29 | CAD >> Dimensions ▸ Running | cap | DIM-16 | Works | DimMode::Running |
| 30 | CAD >> Dimensions ▸ Baseline | cap | DIM-17 | Works | DimMode::Baseline |
| 31 | CAD >> Dimensions ▸ Angular | cap | DIM-18 | Works | DimMode::Angular |
| 32 | CAD >> Dimensions ▸ Centerline | cap | DIM-19 | Works | DimMode::Centerline |
| 33 | CAD >> Dimensions ▸ Tape Measure | cap | DIM-20 | Works | DimMode::TapeMeasure |
| 34 | CAD >> Automatic Dimensions ▸ Auto Exterior | cap | DIM-24, DIM-25, DIM-26 | Works | plan-core dimension.rs auto_exterior_set; DimensionDefaults.auto_strings |
| 35 | CAD >> Automatic Dimensions ▸ Auto Interior | cap | DIM-27 | Works | DimMode::AutoInterior |
| 36 | CAD >> Automatic Dimensions ▸ Auto Elevation | cap | DIM-28 | Differs-by-design | dimension.rs plan-axis strings Gap: Plan strings, not elevation view objects. |
| 37 | CAD >> Automatic Dimensions ▸ Auto Story Pole | cap | DIM-28 | Differs-by-design | dimension.rs plan-axis strings Gap: Plan strings, not elevation view objects. |
| 38 | CAD >> Automatic Dimensions ▸ Auto NKBA (not captured) | n | DIM-28 | Partial | toolbar.rs auto_dimensions() "AutoNkba" exists; no NKBA clearance rules audit (kitchen and bath clearances) |
| 39 | CAD >> Dimension Defaults (Saved Dimension Defaults set) | cap | DIM-40, DIM-6 | Partial | DimensionDefaults.temp_locate/elevation_locate (LocateGroup); default_lists.rs Locate Objects tab (Manual and Automatic / Temporary / Eleva… |
| 40 | CAD >> Text ▸ Text (Y) | cap | TXT-1, TXT-2, TXT-3 | Works | tools/text.rs (press-drag from the anchor sets `TextBox::width`/`height`; the box keeps its dragged top); tests `dragging_defines_a_text_bo… |
| 41 | CAD >> Text ▸ Rich Text | cap | TXT-4 | Partial | tools/text.rs RichRun markup Gap: Markup runs not a WYSIWYG box; italic not drawn. |
| 42 | CAD >> Text ▸ Leader Line | cap | TXT-5 | Works | TextMode::LeaderLine |
| 43 | CAD >> Text ▸ Text Line with Arrow | cap | TXT-6 | Works | TextMode::ArrowLine |
| 44 | CAD >> Text ▸ Callout | cap | TXT-7 | Works | TextMode::Callout (circle, hexagon, square) |
| 45 | CAD >> Text ▸ Marker | cap | TXT-8 | Works | TextMode::Marker |
| 46 | CAD >> Text ▸ Note, Note Type Management | cap | TXT-9 | Works | TextMode::Note; dialogs/text/manage.rs |
| 47 | CAD >> Text ▸ Text Macro Management | cap | NO SPEC (TXT-20) | Works | dialogs/text/ "Text Macro Management"; plan-core text_styles.rs |
| 48 | CAD >> Patterns ▸ Hatch Closed Shape (CAD hatch patterns) | cap | NO SPEC (CAD-53) | Works | menus.rs cad_menu "Patterns" > "Hatch Closed Shape"; tools/cad/edit.rs hatch_pattern, plan_hatch, apply_hatch |
| 49 | CAD >> Make CAD Block | cap | CAD-31 | Partial, In progress (Round 14) | CadMode::MakeBlock Gap: Block is a named group in the plan, not in the Library Browser. |
| 50 | CAD >> Edit CAD Block | cap | CAD-32 | Partial, In progress (Round 14) | Edit CAD Block edits name/points Gap: No detail window. |
| 51 | CAD >> Explode CAD Block | cap | CAD-32 | Partial, In progress (Round 14) | Edit CAD Block edits name/points Gap: No detail window. |
| 52 | CAD >> Insert CAD Block | cap | CAD-33 | Partial, In progress (Round 14) | Add Insertion Point tool Gap: Instances are groups; no uniform resize. |
| 53 | CAD >> Add Insertion Point | cap | CAD-33 | Partial, In progress (Round 14) | Add Insertion Point tool Gap: Instances are groups; no uniform resize. |
| 54 | CAD >> Add Arrow Backoff Point | cap | CAD-6 | Works | CadMode::LineArrow; Add Arrow Backoff Point |
| 55 | CAD >> CAD Block Management… | cap | CAD-34 | Works | dialogs/cad/blocks.rs |
| 56 | CAD >> Sun Angle | cap | C-63 | Partial, In progress (Round 14) | Sun Angle window (date, time, latitude, azimuth/altitude) Gap: No longitude, DST, north direction; no Move Sun/Moon. |
| 57 | CAD >> North Pointer | cap | NO SPEC (CB-84) | Works | toolbar.rs site_objects() "North Pointer"; plan-terrain site_symbols.rs; dialogs/terrain.rs North angle |
| 58 | CAD >> Plan Footprint | cap | L-38 | Partial | Tools > Checks > Plan Footprint (CAD polyline + area) Gap: Traces room boundary, not outer wall faces. |
| 59 | CAD >> Auto Detail | cap | L-39 | Partial, In progress (Round 14) | CAD > Auto Detail and the toolbar button: `tools/details/cad_detail.rs build` / `auto_detail` (section or elevation to a detail: cut lines,… |
| 60 | CAD >> CAD Detail Management… | cap | L-41 | Works | CAD > CAD Detail Management (`dialogs/details/management.rs`): list, rename, duplicate, delete, open in a tab, send to layout (`CadDetail`… |
| 61 | CAD >> CAD Detail From View | cap | L-40 | Works | CAD > CAD Detail From View (`tools::details::detail_from_view`): a selected section or elevation is Auto Detail, otherwise the active floor… |
| 62 | CAD >> CAD to Walls… | cap | L-42, W-89 | Works | plan_import::cad_to_walls; dialogs/exchange.rs |
| 63 | CAD >> Edit CAD ▸ (Offset, Trim, Extend, Break Line, Fillet, Chamfer, Make Parallel/Perpendicular, Reverse Di… (not captured) | n | NO SPEC (CAD-54) | Works | tools/cad/edit.rs; CadMode::Offset, Trim, Extend, BreakLine, Fillet, Chamfer, ReverseDirection; edit_tests.rs "Trim Line" |
| 64 | CAD >> Convert to Polyline | cap | CAD-23 | Works | CadMode converts |
| 65 | CAD >> Convert to Spline | cap | CAD-23 | Works | CadMode converts |
| 66 | CAD >> Polyline to Lines | cap | CAD-23 | Works | CadMode converts |
| 67 | CAD >> Boolean polyline operations (union, subtract, intersect) (not captured) | n | NO SPEC (CAD-55) | Missing | no polygon boolean tool in plan-app (grep boolean in tools finds nothing) |
| 68 | CAD >> CAD layer set, line style, fill style, arrow style lists (CAD Defaults) (not captured) | n | CAD-13, CAD-38 | Works | dialogs/cad.rs Line/Fill Style tabs |

### Tools menu

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Tools >> Layer Settings ▸ Display Options… / Layer Set Management… | cap | LAY-1, LAY-2, LAY-3, LAY-8, LAY-11 | Works | plan-core layers.rs |
| 2 | Tools >> Layer Settings ▸ Active Layers by Tool… | cap | NO SPEC (LAY-16) | Works | menus.rs "Active Layers by Tool…"; dialogs/layer_sets.rs ACTIVE_LAYERS |
| 3 | Tools >> Floor/Reference Display ▸ | cap | R-65, R-66, LAY-9, LAY-10 | Works | dialogs/reference_display.rs (reference_walls); render.rs draw_reference_floor; snap.rs snap_with_reference |
| 4 | Tools >> Active View ▸ Plan Views (Plan View Specification, Save, Reset, Add Template Views) | cap | LAY-2, R-70 | Partial | Project.plan_views Gap: Layer views yes; floor pinning unverified. |
| 5 | Tools >> Active View ▸ Rotate Plan View… | cap | NO SPEC (LAY-17) | Missing | no plan rotation (grep finds nothing) |
| 6 | Tools >> Reverse Plan | cap | NO SPEC (S-115) | Missing | no mirror-the-whole-plan command (Edit > Reflect works on a selection, S-48) |
| 7 | Tools >> Active Defaults… | cap | NO SPEC (APP-24) | Partial | menus.rs "Active Defaults…" opens the Default Settings tree; there is no "set the active default set" |
| 8 | Tools >> Checks ▸ Plan Check | cap | NO SPEC (APP-25) | Works | plan-check crate (IRC rules, rules_irc.rs); dialogs/plan_check.rs; docs/manual/18-plan-check.md |
| 9 | Tools >> Checks ▸ Door/Window Check | cap | NO SPEC (APP-26) | Works | plan-check door_window_check; menus.rs |
| 10 | Tools >> Checks ▸ Plan Check Settings (jurisdiction, rule groups) (not captured) | n | NO SPEC (APP-27) | Works | plan-check settings.rs; dialogs/plan_check.rs |
| 11 | Tools >> Toolbars and Hotkeys ▸ Customize Toolbars… | cap | TB-1, TB-2, TB-3, TB-4, TB-5 | Works | `dialogs/customize_toolbars.rs`; `ticking_a_button_adds_it_after_the_pick_and_unticking_removes_it`, `move_remove_and_separators_act_on_the… |
| 12 | Tools >> Toolbars and Hotkeys ▸ Customize Hotkeys… | cap | HK-1, HK-2, HK-3, HK-5, HK-6, HK-7 | Works | `dialogs/hotkeys.rs menu_of`, `grouped_commands`; `the_list_is_grouped_by_menu_and_the_search_narrows_it`. The menu of a command is read fr… |
| 13 | Tools >> Symbol ▸ (Symbol Specification, Replace From Library, Edit Library Symbol) | cap | CB-57 | Works | tools/library/convert.rs replace_selected; dialogs/symbol.rs |
| 14 | Tools >> Space Planning ▸ Space Planning Assistant… | cap | NO SPEC (R-79) | Works | plan-spaceplan crate (questionnaire, room boxes, bump, validate, build_house); menus.rs tools_menu |
| 15 | Tools >> Space Planning ▸ Room Planner / Space Planning Configuration toolbar (not captured) | n | NO SPEC (R-80) | Partial | room boxes exist; the Space Planning toolbar configuration toggle is a stub (toolbar.rs config_space_planning) |
| 16 | Tools >> Plan Database ▸ | cap | NO SPEC (APP-28) | Missing | toolbar button "Plan Database" is a stub; menus.rs has no row |
| 17 | Tools >> Time Tracker ▸ | cap | NO SPEC (APP-29) | Missing | nothing in crates (grep finds nothing) |
| 18 | Tools >> Schedules ▸ Door Schedule | cap | L-23, L-24, L-25, L-27, L-28 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 19 | Tools >> Schedules ▸ Window Schedule | cap | L-23, L-24, L-25, L-27, L-28 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 20 | Tools >> Schedules ▸ Room Schedule | cap | L-23, L-24, L-25, L-27, L-28 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 21 | Tools >> Schedules ▸ Wall Schedule | cap | L-23, L-24, L-25, L-27, L-28 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 22 | Tools >> Schedules ▸ Cabinet / Electrical / Fixture / Framing / Furniture / Plant / Note schedules | cap | L-23, CB-21, E-13 | Partial | schedule_kinds.rs builds door, window, room, wall, cabinet, electrical, stair and framing tables; fixture, furniture, plant and note schedu… |
| 23 | Tools >> Schedules ▸ Room Finish Schedule | cap | L-30 | Works | plan-core schedules.rs ScheduleKind::RoomFinish (floor, wall, base, crown and ceiling finishes); tools/schedule.rs |
| 24 | Tools >> Schedules ▸ Custom Schedule | cap | L-31 | Partial, In progress (Round 14) | Schedule Specification builds the table (kind, floors, filter, columns shown/renamed/ordered, sort, group, totals; Show All and Reset Colum… |
| 25 | Tools >> Schedules ▸ Manage Custom Schedules | cap | L-31 | Partial, In progress (Round 14) | Schedule Specification builds the table (kind, floors, filter, columns shown/renamed/ordered, sort, group, totals; Show All and Reset Colum… |
| 26 | Tools >> Schedules ▸ Renumber Door Schedule | cap | DW-61 | Works | opening_edit.rs `renumber`, `renumber_marks` (Edit toolbar "Renumber Schedule", Schedules > Renumber Door / Window Schedule; marks written… |
| 27 | Tools >> Schedules ▸ Renumber Window Schedule | cap | DW-61 | Works | opening_edit.rs `renumber`, `renumber_marks` (Edit toolbar "Renumber Schedule", Schedules > Renumber Door / Window Schedule; marks written… |
| 28 | Tools >> Schedules ▸ Place on Plan (not captured) | n | L-24, L-20 | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes Gap: The two standard fonts are ref… |
| 29 | Tools >> Schedules ▸ Create Construction Set (not captured) | n | L-24, L-20 | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes Gap: The two standard fonts are ref… |
| 30 | Tools >> Materials List ▸ | cap | L-33, L-34, L-36 | Works | Tools > Materials List window: the eleven categories (Foundation ... Landscaping), category filter, active floor or all floors |
| 31 | Tools >> Materials List ▸ Framing Takeoff (not captured) | n | CB-41 | Works | Framing members counted in stock lengths with board feet, waste and unit prices; roofing lines; CSV, PDF page in the construction set, layo… |
| 32 | Tools >> Object Painter ▸ | cap | NO SPEC (S-116) | Missing | no object painter (copy a source object's settings onto others) |
| 33 | Tools >> Fill Style Painter ▸ | cap | NO SPEC (CAD-56) | Missing | no fill style painter |
| 34 | Tools >> Layer Painter / Layer Eyedropper (not captured) | n | NO SPEC (LAY-18) | Missing | no layer painter or layer eyedropper (Send to Layer works, LAY-13) |
| 35 | Tools >> Project Information… | cap | NO SPEC (L-52) | Works | dialogs/project_info.rs tabs Client, Project, Designer, Revisions, Custom Fields; feeds title block macros (L-9) |
| 36 | Tools >> Loan Calculator… | cap | NO SPEC (APP-30) | Missing | out of scope for a design tool |
| 37 | Tools >> Ruby Console… | cap | NO SPEC (APP-31) | Missing | no scripting console (out of scope, see recommendation) |
| 38 | Tools >> Screen Capture ▸ | cap | NO SPEC (APP-32) | Missing | no in-app screen capture to clipboard / file |
| 39 | Tools >> Color Chooser… | cap | NO SPEC (APP-33) | Works | app_info::COLOR_CHOOSER; dialogs/app_info.rs |
| 40 | Tools >> New Plan View | cap | LAY-2 | Works | plan-core layer_sets.rs; Project.plan_views |

### View menu

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | View >> Refresh Display (F5) | cap | NO SPEC (APP-34) | Works | app_info::REFRESH; menus.rs view_menu |
| 2 | View >> Library Browser (Cmd+L) | cap | CB-53 | Works | shell/library_panel.rs |
| 3 | View >> Project Browser | cap | NO SPEC (APP-35) | Works | shell/docks.rs Project dock (views, layouts, cameras, schedules); main.rs |
| 4 | View >> Tool Palette | cap | NO SPEC (APP-36) | Missing | no floating tool palette (toolbars and flyouts only) |
| 5 | View >> Active Layer Display Options | cap | LAY-3 | Works | dialogs/layer_display.rs (dock and modal); shell/docks.rs `edit_layers` |
| 6 | View >> Walkthrough Preview | cap | C-71 | Works | tools/camera.rs; Play/Record |
| 7 | View >> Action History | cap | S-79 | Works | dialogs/action_history.rs |
| 8 | View >> Status Bar / Scrollbars / Toolbars (show-hide) | cap | NO SPEC (APP-37) | Partial | Status Bar and Toolbars toggles live (app_info TOGGLE_STATUS_BAR, TOGGLE_TOOLBARS); no Scrollbars toggle in menus.rs |
| 9 | View >> Color (F8) | cap | NO SPEC (LAY-19) | Works | ViewFlag::Color; toolbar.rs; menus.rs |
| 10 | View >> Crosshairs | cap | NO SPEC (LAY-20) | Works | ViewFlag::Crosshairs; menus.rs |
| 11 | View >> Coordinate System Indicator (Floating / Fixed / Origin) | cap | NO SPEC (LAY-21) | Missing | only the status-bar X/Y readout; no on-canvas axis indicator (toolbar/config.rs maps the label only) |
| 12 | View >> Reference Grid (Shift+F9) | cap | NO SPEC (LAY-22) | Works | ViewFlag::ReferenceGrid; menus.rs "Reference Grid" |
| 13 | View >> Angle Snap Grid | cap | NO SPEC (LAY-23) | Missing | no angle-snap grid display toggle |
| 14 | View >> Temporary Dimensions | cap | S-64 | Works | tempdim.rs |
| 15 | View >> Arc Centers and Ends | cap | CAD-10 | Works | menus.rs:874 ViewFlag::ArcCenters |
| 16 | View >> Line Weights | cap | LAY-7 | Works | restyle.rs |
| 17 | View >> Drawing Sheet | cap | L-19 | Works | Print Preview window (`PrintPreviewDialog`, `plan_layout::layout_print_preview` / `plan_view_print_preview`): the pages as the Print dialog… |
| 18 | View >> Watermark | cap | NO SPEC (L-53) | Missing | no watermark option on screen or in print |
| 19 | View >> Enter Full Screen | cap | NO SPEC (APP-38) | Works | app_info::FULL_SCREEN; menus.rs |
| 20 | View >> Canvas theme and UI brightness (Plan Studio extra) (not captured) | n | NO SPEC (APP-39) | Works | menus.rs Canvas Theme, UI Brightness; theme.rs |

### Window menu

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Window >> Zoom (rubber-band, Shift+Z) | cap | NO SPEC (LAY-24) | Partial | toolbar.rs "Zoom" is a stub toggle; mouse wheel and Zoom In/Out work |
| 2 | Window >> Zoom Out (-) | cap | NO SPEC (LAY-25) | Works | Action::ZoomOut/ZoomIn; menus.rs window_menu |
| 3 | Window >> Zoom In (+) | cap | NO SPEC (LAY-25) | Works | Action::ZoomOut/ZoomIn; menus.rs window_menu |
| 4 | Window >> Undo Zoom | cap | NO SPEC (LAY-26) | Works | Action::UndoZoom |
| 5 | Window >> Fill Window Building Only | cap | NO SPEC (LAY-27) | Missing | toolbar.rs fill_building stub; menus.rs has no row |
| 6 | Window >> Fill Window Selected Objects | cap | S-96 | Partial, In progress (Round 14) | main.rs:155 fill_window; toolbar.rs:1971 Gap: Fits whole plan; selected variant stub. |
| 7 | Window >> Fill Window (Ctrl+F) | cap | NO SPEC (LAY-28) | Works | Action::FillWindow; menus.rs |
| 8 | Window >> Pan Window (H) | cap | NO SPEC (LAY-29) | Works | Action::TogglePan; tools/pan.rs |
| 9 | Window >> Swap Views (F7) | cap | NO SPEC (APP-40) | Missing | no second-view swap (grep swap_views finds nothing) |
| 10 | Window >> Tile Horizontally | cap | NO SPEC (APP-41) | Missing | views are tabs only; no tiling (grep Tile finds nothing) |
| 11 | Window >> Tile Vertically | cap | NO SPEC (APP-41) | Missing | views are tabs only; no tiling (grep Tile finds nothing) |
| 12 | Window >> Tab Windows | cap | NO SPEC (APP-42) | Partial | plan_tabs.rs tabs for plan, 3D, layout views; no Ctrl+Tab cycle menu rows |
| 13 | Window >> Select Next Tab | cap | NO SPEC (APP-42) | Partial | plan_tabs.rs tabs for plan, 3D, layout views; no Ctrl+Tab cycle menu rows |
| 14 | Window >> Select Previous Tab | cap | NO SPEC (APP-42) | Partial | plan_tabs.rs tabs for plan, 3D, layout views; no Ctrl+Tab cycle menu rows |
| 15 | Window >> List of open views | cap | NO SPEC (APP-43) | Works | menus.rs window_menu "Floor Plan View", "Layout" rows |

### Account and Help

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Account >> Sign In | cap | NO SPEC (APP-44) | Differs-by-design | no licensing or accounts in an open-source app (Account menu omitted by design, chief-x18-menus.md) |
| 2 | Account >> Make License Available | cap | NO SPEC (APP-44) | Differs-by-design | no licensing or accounts in an open-source app (Account menu omitted by design, chief-x18-menus.md) |
| 3 | Account >> My Account | cap | NO SPEC (APP-44) | Differs-by-design | no licensing or accounts in an open-source app (Account menu omitted by design, chief-x18-menus.md) |
| 4 | Help >> Launch Help… | cap | NO SPEC (APP-45) | Works | app_info HELP, HELP_REFERENCE, HELP_TUTORIAL; docs/manual |
| 5 | Help >> View Reference Manual… | cap | NO SPEC (APP-45) | Works | app_info HELP, HELP_REFERENCE, HELP_TUTORIAL; docs/manual |
| 6 | Help >> View Tutorial Guide… | cap | NO SPEC (APP-45) | Works | app_info HELP, HELP_REFERENCE, HELP_TUTORIAL; docs/manual |
| 7 | Help >> View Training Videos | cap | NO SPEC (APP-46) | Differs-by-design | replaced by Plan Studio on GitHub… and Keyboard Shortcuts… (help_menu); no vendor services |
| 8 | Help >> ChiefTalk | cap | NO SPEC (APP-46) | Differs-by-design | replaced by Plan Studio on GitHub… and Keyboard Shortcuts… (help_menu); no vendor services |
| 9 | Help >> Visit Website | cap | NO SPEC (APP-46) | Differs-by-design | replaced by Plan Studio on GitHub… and Keyboard Shortcuts… (help_menu); no vendor services |
| 10 | Help >> Technical Support | cap | NO SPEC (APP-46) | Differs-by-design | replaced by Plan Studio on GitHub… and Keyboard Shortcuts… (help_menu); no vendor services |
| 11 | Help >> Download Program Updates… | cap | NO SPEC (APP-47) | Missing | no update checker (releases are on GitHub) |
| 12 | Help >> Export Logs… | cap | NO SPEC (APP-48) | Missing | no log export (status bar keeps the last 50 messages) |
| 13 | Help >> System Information… | cap | NO SPEC (APP-49) | Works | app_info::SYSTEM_INFO; menus.rs |
| 14 | Help >> About Plan Studio / Keyboard Shortcuts… (not captured) | n | NO SPEC (APP-50) | Works | app_info ABOUT, HELP_HOTKEYS |

### Toolbar row 1

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Toolbar 1 >> New Plan | cap | NO SPEC (APP-51) | Works | toolbar.rs row1 buttons -> files.rs |
| 2 | Toolbar 1 >> Open Plan | cap | NO SPEC (APP-51) | Works | toolbar.rs row1 buttons -> files.rs |
| 3 | Toolbar 1 >> Save buttons | cap | NO SPEC (APP-51) | Works | toolbar.rs row1 buttons -> files.rs |
| 4 | Toolbar 1 >> Print button | cap | L-18 | Works | File > Print dialog (dialogs/print.rs): PDF file, system printer picked from `lpstat -p` (default printer otherwise) through `lp -d`, or vi… |
| 5 | Toolbar 1 >> Send to Layout button | cap | L-1 | Works | shell/layout_window.rs; dialogs/layout.rs; hotkey S,L; Project Browser camera list (right-click > Send to Layout, `LayoutCommand::SendCamer… |
| 6 | Toolbar 1 >> Undo | cap | S-75 | Works | plan-core history.rs; scenarios s12_hotkeys |
| 7 | Toolbar 1 >> Redo buttons | cap | S-75 | Works | plan-core history.rs; scenarios s12_hotkeys |
| 8 | Toolbar 1 >> Preferences button | cap | PR-1 | Works | `dialogs/preferences.rs Page::ALL`; `every_page_draws_with_changed_values` |
| 9 | Toolbar 1 >> Launch Help button | cap | NO SPEC (APP-52) | Works | toolbar.rs "Launch Help" -> app_info::HELP |
| 10 | Toolbar 1 >> Edit Active View | cap | LAY-2, LAY-8 | Works | plan-core layer_sets.rs; Project.plan_views |
| 11 | Toolbar 1 >> Save Active View | cap | LAY-2, LAY-8 | Works | plan-core layer_sets.rs; Project.plan_views |
| 12 | Toolbar 1 >> Save Active View As | cap | LAY-2, LAY-8 | Works | plan-core layer_sets.rs; Project.plan_views |
| 13 | Toolbar 1 >> Saved view selector dropdown | cap | LAY-2, R-70 | Partial | Project.plan_views Gap: Layer views yes; floor pinning unverified. |
| 14 | Toolbar 1 >> Display Options button | cap | LAY-3 | Partial | toolbar.rs row 1 "Display Options" is a stub button (NotImplemented); the same dialog is Tools > Layer Settings > Display Options… |
| 15 | Toolbar 1 >> Default Settings button | cap | NO SPEC (APP-53) | Works | toolbar.rs -> Action::DefaultSettings |
| 16 | Toolbar 1 >> Plan Database button | cap | NO SPEC (APP-28) | Missing | toolbar.rs "Plan Database" is a stub button |
| 17 | Toolbar 1 >> Floor Defaults button | cap | R-56 | Works | dialogs/floor_defaults.rs; Floor.settings; toolbar button, Build > Floor, Default Settings |
| 18 | Toolbar 1 >> Down One Floor | cap | R-67 | Works | main.rs |
| 19 | Toolbar 1 >> Current floor | cap | R-67 | Works | main.rs |
| 20 | Toolbar 1 >> Up One Floor | cap | R-67 | Works | main.rs |
| 21 | Toolbar 1 >> 3D view flyout (Perspective Full Overview, Floor Overview, Doll House, Orthographic) | cap | C-10, C-11, C-13, C-15 | Partial, In progress (Round 14) | Orthographic Full Overview + 4 elevations Gap: No floor/framing ortho overview, no isometric SW/SE/NE/NW. |
| 22 | Toolbar 1 >> Full Camera flyout (section cameras, auto elevations) | cap | C-4, C-17, C-19, C-20, C-21 | Works | tools/camera.rs |
| 23 | Toolbar 1 >> Mouse-Orbit Camera flyout | cap | C-34, C-35 | Partial, In progress (Round 14) | view3d_panel.rs gestures Gap: No explicit Move Camera tool choices. |
| 24 | Toolbar 1 >> Cross Section Slider flyout | cap | C-23 | Works | toolbar.rs cross_section_slider; view3d_panel `slider_camera` |
| 25 | Toolbar 1 >> Create Walkthrough Path flyout | cap | C-71 | Works | tools/camera.rs; Play/Record |
| 26 | Toolbar 1 >> Rendering technique flyout (Standard and others) | cap | C-45, C-46, C-47, C-48, C-49, C-50, C-51, C-52 | Partial, In progress (Round 14) | plan-materials nine techniques mapped to `plan_view3d::Look`; manual 10.4 Gap: All nine draw in GL (Technical Illustration and Line Drawing… |
| 27 | Toolbar 1 >> Add Lights flyout | cap | C-64 | Partial, In progress (Round 14) | Project.lights; Adjust Lights dialog Gap: Point lights only; no auto-place or color temperature. |
| 28 | Toolbar 1 >> Sun Angle toggle | cap | C-63 | Partial, In progress (Round 14) | Sun Angle window (date, time, latitude, azimuth/altitude) Gap: No longitude, DST, north direction; no Move Sun/Moon. |
| 29 | Toolbar 1 >> Material Painter toggle | cap | C-56 | Works | tools/materials.rs Paint, Eyedropper and Erase modes through the 3D pick hook; Adjust Materials per part; painted_textures shows a painted… |
| 30 | Toolbar 1 >> Material Eyedropper toggle | cap | C-57 | Works | tools/materials.rs PainterMode::Eyedropper; tests eyedropper_and_delete_surface, s23 the_material_painter_overrides_a_wall_in_3d_through_th… |
| 31 | Toolbar 1 >> Object Eyedropper toggle | cap | C-57 | Missing | toolbar.rs "Object Eyedropper" is a stub toggle (Material Eyedropper works) |
| 32 | Toolbar 1 >> Delete Surface toggle | cap | C-55 | Works | tools/materials.rs PainterMode::Erase (Delete Surface); s23 the_material_painter_overrides_a_wall_in_3d_through_the_pick_hook; tools/materi… |
| 33 | Toolbar 1 >> Adjust Material Definition | cap | C-58 | Partial, In progress (Round 14) | tools/materials.rs ADJUST (Adjust Materials: a material per part of the selected object) and BUILDER (edits a user material's color, roughn… |
| 34 | Toolbar 1 >> Interactive Material Editor | cap | C-59 | Partial, In progress (Round 14) | tools/materials.rs LIST (Materials window: library with swatches, the active material) Gap: Browses and activates; no in-place editing from… |
| 35 | Toolbar 1 >> Default Configuration toggle | cap | TB-1 | Partial | toolbar/config.rs keeps per-view sets and a Customize dialog; the three configuration toggles in row 1 are stubs |
| 36 | Toolbar 1 >> Space Planning Configuration toggle | cap | TB-1 | Partial | stub toggle; the Space Planning Assistant opens from Tools |
| 37 | Toolbar 1 >> Extended Tool Configuration toggle | cap | TB-1 | Partial | stub toggle; Customize Toolbars can add any tool |

### Toolbar row 2 (Build tools)

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Toolbar 2 >> Select Objects | cap | S-1, S-10 | Works | tools/select.rs test click_selects_and_empty_click_deselects |
| 2 | Toolbar 2 >> Wall flyout (straight / curved) | cap | W-2 | Works | toolbar.rs Straight Wall flyout; manual 2.2 |
| 3 | Toolbar 2 >> Railing and Deck flyout | cap | W-56 | Works | rail style tab; plan-3d |
| 4 | Toolbar 2 >> Door flyout | cap | DW-108 | Works | toolbar.rs; opening.rs hint |
| 5 | Toolbar 2 >> Window flyout | cap | DW-108 | Works | toolbar.rs; opening.rs hint |
| 6 | Toolbar 2 >> Cabinet flyout | cap | CB-1 | Works | toolbar.rs Cabinet flyout (16 kinds); tools/cabinet.rs |
| 7 | Toolbar 2 >> Electrical flyout | cap | CB-62 | Works | tools/electrical.rs |
| 8 | Toolbar 2 >> Stairs flyout | cap | CB-22 | Works | tools/stairs.rs; toolbar Stairs flyout |
| 9 | Toolbar 2 >> Floor flyout | cap | R-59 | Works | floors.rs build_new_floor_with; dialogs/floor.rs |
| 10 | Toolbar 2 >> Roof flyout | cap | RF-1 | Works | dialogs/roof.rs; tools/roof.rs Build Roof; scenarios/s06_roof.rs |
| 11 | Toolbar 2 >> Trim flyout | cap | NO SPEC (W-112) | Works | toolbar.rs trim() |
| 12 | Toolbar 2 >> General / Floor-Ceiling / Roof Framing flyouts | cap | CB-35 | Works | tools/framing.rs 19 tools |
| 13 | Toolbar 2 >> Slab flyout | cap | NO SPEC (R-81) | Works | toolbar.rs slab() |
| 14 | Toolbar 2 >> 3D Solid flyout | cap | NO SPEC (C-75) | Works | toolbar.rs solid_3d() |
| 15 | Toolbar 2 >> Paste Hold Position button | cap | S-83 | Works | edit_commands.rs ids::PASTE_HOLD; toolbar.rs button |
| 16 | Toolbar 2 >> Manual Dimension flyout | cap | DIM-11 | Works | tools/dimension.rs DimMode::Manual |
| 17 | Toolbar 2 >> Auto Exterior Dimensions flyout | cap | DIM-24 | Works | plan-core dimension.rs auto_exterior_set; DimensionDefaults.auto_strings |
| 18 | Toolbar 2 >> Text / Leader Line flyout | cap | TXT-1 | Works | tools/text.rs (press-drag from the anchor sets `TextBox::width`/`height`; the box keeps its dragged top); tests `dragging_defines_a_text_bo… |
| 19 | Toolbar 2 >> Revision Cloud toggle | cap | CAD-36 | Works | CadMode::RevisionCloud |
| 20 | Toolbar 2 >> Schedule flyout (place schedule on plan) (not captured) | n | L-24 | Works | schedule_view.rs live table; Floors option; Schedule, Placed Schedule and Materials List layout boxes, refreshed by Update Views |
| 21 | Toolbar 2 >> Point / Line / Arc / Circle / Box flyouts | cap | CAD-2, CAD-3, CAD-7, CAD-11, CAD-12 | Partial, In progress (Round 14) | CadMode::Arc option strip (three-point, center-start-end, tangent) Gap: Start-end-radius mode absent; Edit > Arc Creation Modes inert. |
| 22 | Toolbar 2 >> Spline toggle | cap | CAD-29 | Works | CadMode::Spline |
| 23 | Toolbar 2 >> Auto Detail button | cap | L-39 | Partial, In progress (Round 14) | CAD > Auto Detail and the toolbar button: `tools/details/cad_detail.rs build` / `auto_detail` (section or elevation to a detail: cut lines,… |
| 24 | Toolbar 2 >> Current CAD Layer button | cap | CAD-1 | Works | plan-core layers.rs current_cad_layer; tools/cad.rs draw_layer; toolbar.rs:2136 Current CAD Layer button opens Active Layers by Tool (dialo… |
| 25 | Toolbar 2 >> Terrain / Site flyouts (Terrain Configuration toolbar) (not captured) | n | CB-43 | Works | tools/terrain.rs; plan-terrain `build_terrain_with_progress`; site_view `build_surface_with_progress` |

### Right-edge vertical toolbar

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Right bar >> Library Browser toggle | cap | CB-53 | Works | shell/library_panel.rs |
| 2 | Right bar >> Project Browser toggle | cap | NO SPEC (APP-54) | Works | toolbar.rs view_slots Dock::Project; shell/docks.rs |
| 3 | Right bar >> Active Layer Display Options toggle | cap | LAY-3 | Works | dialogs/layer_display.rs (dock and modal); shell/docks.rs `edit_layers` |
| 4 | Right bar >> Zoom toggle | cap | NO SPEC (LAY-30) | Partial | toolbar.rs "Zoom" stub; wheel and Zoom In/Out buttons work |
| 5 | Right bar >> Zoom In | cap | NO SPEC (LAY-31) | Works | Action::ZoomIn, ZoomOut, UndoZoom |
| 6 | Right bar >> Zoom Out | cap | NO SPEC (LAY-31) | Works | Action::ZoomIn, ZoomOut, UndoZoom |
| 7 | Right bar >> Undo Zoom | cap | NO SPEC (LAY-31) | Works | Action::ZoomIn, ZoomOut, UndoZoom |
| 8 | Right bar >> Fill Window Selected Objects | cap | S-96 | Partial, In progress (Round 14) | main.rs:155 fill_window; toolbar.rs:1971 Gap: Fits whole plan; selected variant stub. |
| 9 | Right bar >> Fill Window Building Only | cap | NO SPEC (LAY-27) | Missing | toolbar.rs fill_building stub |
| 10 | Right bar >> Fill Window | cap | NO SPEC (LAY-32) | Works | Action::FillWindow |
| 11 | Right bar >> Pan Window | cap | NO SPEC (LAY-33) | Works | Action::TogglePan |
| 12 | Right bar >> Reference Display toggle | cap | LAY-9 | Works | render.rs draw_reference_floor; snap.rs snap_with_reference |
| 13 | Right bar >> Crosshairs toggle | cap | NO SPEC (LAY-20) | Works | ViewFlag::Crosshairs |
| 14 | Right bar >> Color toggle | cap | NO SPEC (LAY-19) | Works | ViewFlag::Color |
| 15 | Right bar >> Line Weights toggle | cap | LAY-7 | Works | restyle.rs |
| 16 | Right bar >> Drawing Sheet toggle | cap | L-19 | Works | Print Preview window (`PrintPreviewDialog`, `plan_layout::layout_print_preview` / `plan_view_print_preview`): the pages as the Print dialog… |
| 17 | Right bar >> Print Preview toggle | cap | L-19 | Works | Print Preview window (`PrintPreviewDialog`, `plan_layout::layout_print_preview` / `plan_view_print_preview`): the pages as the Print dialog… |
| 18 | Right bar >> Temporary Dimensions toggle | cap | S-64 | Works | tempdim.rs |
| 19 | Right bar >> Connect CAD Segments toggle | cap | CAD-3, CAD-4 | Partial | Connect flag; Convert to Polyline Gap: Moving shared vertex moves both: not confirmed. |
| 20 | Right bar >> Arc Centers and Ends toggle | cap | CAD-10 | Works | menus.rs:874 ViewFlag::ArcCenters |
| 21 | Right bar >> Overflow chevron (hidden buttons menu) | cap | NO SPEC (APP-55) | Works | toolbar/config.rs draw_bar overflow |

### Edit toolbar (contextual) by object

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Edit toolbar >> Common buttons (Transform, Reflect, Center, Point to Point, Layer, Same Type) | cap | S-38, S-39, S-40 | Works | actions.rs common_edit_actions |
| 2 | Edit toolbar >> Straight wall buttons | cap | S-41 | Works | editor/wall_edit.rs edit_actions plus selection_edit_actions: Fix Wall Connections, Reverse Layers, Break Wall, Remove Break, Change Line/A… |
| 3 | Edit toolbar >> Curved wall buttons | cap | S-42 | Works | wall_edit::edit_actions |
| 4 | Edit toolbar >> Door / window buttons | cap | S-43 | Works | editor/opening_edit.rs edit_actions: Reverse Swing, Flip Hinge, Reverse Side, Center on Wall Segment, Mull, Unmull, Reset Label Position; s… |
| 5 | Edit toolbar >> Text buttons | cap | S-44 | Works | editor/actions.rs common_edit_actions: Open Object, Delete, Cut, Copy, Paste in Place plus selection_edit_actions (Transform, Reflect, Cent… |
| 6 | Edit toolbar >> CAD line, arc, polyline, circle buttons | cap | S-45, CAD-41, CAD-42 | Works | tools/cad/edit.rs edit_actions (Fillet, Chamfer, Offset, Trim, Extend, Break, Reverse Direction, Make Parallel/Perpendicular, the converts)… |
| 7 | Edit toolbar >> Dimension buttons | cap | DIM-41 | Works | tools/dimension.rs `edit_actions` (Reverse, Convert to Manual, Align, Distribute, Add Extension Line, Delete Extension Line), `DimMode::Ext… |
| 8 | Edit toolbar >> Room buttons (not captured) | n | S-107 | Works | select.rs test clicking_inside_a_room_selects_the_room_and_double_click_opens_it |
| 9 | Edit toolbar >> Roof plane buttons (not captured) | n | RF-38, RF-39 | Partial, In progress (Round 14) | roof_view.rs PlaneHandle, drag_handle, apply_handle_drag; handles.rs HandleKind::{Pitch, EdgeMove}, roof_plane_handle; tests a_selected_roo… |
| 10 | Edit toolbar >> Stair buttons (not captured) | n | CB-28 | Partial | stairs_view.rs StairHandleKind Gap: No per-landing corner handles. |
| 11 | Edit toolbar >> Cabinet buttons (not captured) | n | CB-8 | Partial | Width from either end, depth from the front and the back, four corner handles (`Reshape(1..=6)` in `placed.rs`), rotate, a label handle (`H… |
| 12 | Edit toolbar >> Camera buttons (not captured) | n | C-25 | Partial, In progress (Round 14) | camera.rs CamHandle (move, aim, clip) Gap: FOV cone-edge handles absent. |
| 13 | Edit toolbar >> Terrain object buttons (not captured) | n | CB-44 | Works | TerrainVariant; `ElevationLine::{spline, reflatten}`; dialogs/terrain/object.rs Elevation Point/Line/Region Specification; tests `elevation… |

### Right-click context menus

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Context menu >> Object right-click menu (Open Object, Delete, Copy, layer ...) (not captured) | n | S-8 | Works | main.rs canvas_context_menu; edit_commands.rs context_entries |
| 2 | Context menu >> Right-click while a tool is active (Select Objects at top) (not captured) | n | DW-109 | Works | main.rs has_canvas_menu |

### Default Settings tree pages

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Default Settings >> 3D Solid defaults | cap | NO SPEC (CB-85) | Missing | not in the Default Settings tree (dialogs/defaults.rs TREE); a placed solid has its own dialog (dialogs/symbol.rs) |
| 2 | Default Settings >> 3D View Defaults | cap | C-68 | Partial, In progress (Round 14) | dialogs/camera.rs Gap: Eye height, angle, technique only. |
| 3 | Default Settings >> Cabinets (Base, Wall, Full Height, Soffit, ...) | cap | CB-20 | Works | plan-core CabinetDefaults; dialogs/cabinet.rs CabinetDefaultsDialog; reached from Default Settings > Cabinets (dialogs/defaults.rs Leaf::Ca… |
| 4 | Default Settings >> CAD defaults (line style, fill, arrows, text, layer) | cap | NO SPEC (CAD-57) | Missing | not in the tree; each CAD item has a dialog (dialogs/cad.rs) but no default set |
| 5 | Default Settings >> Camera Tools defaults | cap | C-5 | Partial | shell/view3d_panel.rs camera_defaults() (eye height, FOV) edited in the 3D View Defaults dialog; not a tree leaf |
| 6 | Default Settings >> Corner Trim defaults | cap | NO SPEC (W-113) | Missing | not in the tree; the trim tools use built-in sizes (tools/details.rs) |
| 7 | Default Settings >> Default Sets (named sets of defaults) | cap | DIM-40 | Partial | only Saved Dimension Defaults sets exist (default_lists.rs); no named sets for walls, text or CAD |
| 8 | Default Settings >> Dimension | cap | DIM-6, DIM-40 | Partial | DimensionDefaults.temp_locate/elevation_locate (LocateGroup); default_lists.rs Locate Objects tab (Manual and Automatic / Temporary / Eleva… |
| 9 | Default Settings >> Distributed Objects defaults | cap | NO SPEC (CAD-58) | Missing | not in the tree |
| 10 | Default Settings >> Doors (interior, exterior, garage, ...) | cap | DW-6, DW-7, DW-105 | Works | opening.rs test doors_and_windows_come_from_the_templates |
| 11 | Default Settings >> Dormer defaults | cap | RF-51 | Partial | Roof Defaults page holds overhang and pitch; Auto Dormer dialog has its own values (dialogs/roof.rs DORMER_TABS) |
| 12 | Default Settings >> Electrical defaults | cap | E-11 | Works | `defaults.rs`, `Project::electrical_defaults`; dialog General tab; `tests::default_heights_are_stored_in_the_project`; s32 `device_heights_… |
| 13 | Default Settings >> Floors and Rooms (Floor Defaults, Room Types) | cap | R-56, R-37 | Works | dialogs/floor_defaults.rs; Floor.settings; toolbar button, Build > Floor, Default Settings |
| 14 | Default Settings >> Foundation defaults | cap | NO SPEC (R-82) | Missing | not in the tree; Build Foundation dialog (dialogs/foundation.rs) takes the values per build |
| 15 | Default Settings >> Framing defaults | cap | CB-36 | Partial, In progress (Round 14) | Build Framing / Build All Framing commands; Framing Defaults page (`dialogs/framing.rs` FramingDefaultsDialog, `framing_view::settings`) Ga… |
| 16 | Default Settings >> General (units, defaults, auto save) | cap | NO SPEC (APP-56) | Partial | Preferences > General Plan Defaults is a link (PR-15); measurement units live in Project Information / exchange options |
| 17 | Default Settings >> Image defaults | cap | NO SPEC (CAD-59) | Missing | not in the tree |
| 18 | Default Settings >> Layout defaults (page, box, title block, text) | cap | L-10 | Partial | layout templates carry the page set-up; no defaults page |
| 19 | Default Settings >> Materials defaults | cap | C-61 | Works | tools/materials/defaults.rs (Materials Defaults); dialogs/materials.rs |
| 20 | Default Settings >> Plan defaults (scale, view, layers, snaps) | cap | NO SPEC (APP-57) | Partial | plan defaults are seeded from the template (plan_defaults.rs, templates.rs) and the editing defaults dialog (Snap Settings); no single Plan… |
| 21 | Default Settings >> Railing and Deck defaults | cap | W-56 | Partial | railing and deck wall types are edited in Wall Type Definitions; no Railing and Deck leaf |
| 22 | Default Settings >> Roofs (Roof Defaults) | cap | RF-5 | Works | dialogs/wall.rs roof() (Specify Pitch and Specify Overhang per wall); dialogs/defaults.rs Roofs > Roof Defaults; roof_view.rs RoofSettings |
| 23 | Default Settings >> Schedules defaults | cap | L-25 | Partial | Schedule Specification dialog per placed table; no default set (dialogs/schedule_spec.rs) |
| 24 | Default Settings >> Slab defaults | cap | NO SPEC (R-83) | Missing | not in the tree |
| 25 | Default Settings >> Stairs defaults | cap | CB-23 | Partial | stair riser/tread defaults are built in (CB-23 verify in Chief); no tree leaf |
| 26 | Default Settings >> Terrain defaults | cap | CB-51 | Works | dialogs/terrain.rs (General, Contours, Building Pad, Materials, Layer; Cut and Fill Report and Import Terrain Data forms); `TerrainRecord::… |
| 27 | Default Settings >> Text (styles, defaults) | cap | TXT-17 | Works | plan-core text_styles.rs; Default Settings Text Styles |
| 28 | Default Settings >> Walls (exterior, interior, foundation, pony, half, railing, attic...) | cap | W-1, W-26, W-51 | Partial | tree has Exterior, Interior and Foundation only; pony, half, railing, deck, fence and attic walls use their wall types |
| 29 | Default Settings >> Windows | cap | DW-6, DW-105 | Works | opening.rs test doors_and_windows_come_from_the_templates |
| 30 | Default Settings >> Default Settings search field | cap | NO SPEC (APP-58) | Works | dialogs/defaults.rs search "Filter the tree" |

### Wall Specification tabs

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Wall dialog >> General tab (type flags, thickness, length, angle, lock, options, curved wall) | cap | W-24, W-25, W-66, W-75 | Partial | dialogs/wall.rs; flags in walls.rs Gap: Invisible/No Room/No Locate/Foundation/Railing work; Lock Center, Terrain Retaining, Attic, Auto-Ge… |
| 2 | Wall dialog >> Structure tab (heights, platform intersections, through wall, rim joist, stud layout, framing) | cap | W-39, W-60, W-62, W-63 | Partial, In progress (Round 14) | walls/wall_spec.rs WallStructure.through_at_start/end; joins.rs Role::Extend; dialogs/wall.rs Structure tab; tests through_wall_runs_past_t… |
| 3 | Wall dialog >> Roof tab (roof options, pitch, overhang, auto roof return) | cap | RF-18, RF-19, RF-20, RF-21, RF-22, RF-23, RF-24, RF-25, RF-26, RF-27, RF-28 | Partial | roof_view.rs RF-27; Roof Return tool; dialogs/wall.rs Auto Roof Return with Length; test auto_roof_return_wraps_the_corners_of_a_gable_end… |
| 4 | Wall dialog >> Foundation tab (footing, slab chamfer, sill plate) | cap | W-52 | Partial, In progress (Round 14) | walls/wall_spec.rs WallFoundation; dialogs/wall.rs Foundation tab (footing, slab chamfers, sill plate); plan-3d wall.rs spec_meshes; tests… |
| 5 | Wall dialog >> Wall Types tab (type, pony wall) | cap | W-46, W-47, W-53, W-79 | Partial | dialogs/wall_types.rs Gap: Wall Types Library button disabled. |
| 6 | Wall dialog >> Wall Cap tab (profile table, position) | cap | NO SPEC (W-114) | Partial | dialogs/wall.rs "Wall Cap" tab live for half walls (W-54 cap); profile library and Add to Library are not wired |
| 7 | Wall dialog >> Wall Covering tab (coverings, position, options) | cap | NO SPEC (W-115) | Works | dialogs/wall/tabs.rs wall_covering; wall_spec_tabs.rs; plan-3d wall.rs covering_meshes; s49 |
| 8 | Wall dialog >> Rail Style tab | cap | W-56 | Partial | dialogs/wall.rs rail_style: Railing Height and a fixed description (posts, rails, 3/4 in balusters); no style choices |
| 9 | Wall dialog >> Newels/Balusters tab (railing wall) | cap | NO SPEC (W-116) | Works | dialogs/wall/tabs.rs newels_balusters; plan-3d railing.rs; s49 |
| 10 | Wall dialog >> Rails tab (railing wall) | cap | NO SPEC (W-117) | Works | dialogs/wall/tabs.rs rails; plan-3d railing.rs; s49 |
| 11 | Wall dialog >> Layer tab (layer, drawing group) | cap | W-80 | Partial, In progress (Round 14) | dialogs/wall.rs Gap: Drawing Group disabled. |
| 12 | Wall dialog >> Materials tab (component tree, material per layer) | cap | C-60 | Partial | tab disabled in the Wall dialog (room, stairs and electrical dialogs have it); per-surface wall materials are set with the Material Painter… |
| 13 | Wall dialog >> Label tab (label content, appearance, position) | cap | W-81 | Partial | dialogs/wall.rs Label Gap: Border/text style/alignment disabled. |
| 14 | Wall dialog >> Components tab (materials list rows per wall layer) | cap | L-35 | Missing, In progress (Round 14) | tab disabled in the Wall dialog |
| 15 | Wall dialog >> Object Information tab (code, comment, manufacturer, supplier) | cap | NO SPEC (W-118) | Works | dialogs/wall/tabs.rs object_information; Wall schedule columns; plan-docs test |
| 16 | Wall dialog >> Schedule tab (include in schedule, callout, category) | cap | L-29 | Missing, In progress (Round 14) | tab disabled in the Wall dialog |

### Door Specification tabs

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Door dialog >> General tab (style, type, swing, size, position, panel, barn door) | cap | DW-14, DW-29, DW-30, DW-31, DW-34, DW-46, DW-54 | Works | dialogs/opening.rs |
| 2 | Door dialog >> Options tab (panels, plan display, open display, glass, swings, safety, recess) | cap | DW-36, DW-57 | Partial, In progress (Round 14) | plan-3d casing.rs add_jambs (jamb boards through the whole wall thickness); spec.recess_depth (Options tab "Recessed To Layer": the leaf an… |
| 3 | Door dialog >> Casing tab (interior and exterior casing, double wall, curved wall) | cap | DW-79 | Partial | opening_symbol.rs casing_parts; opening_view.rs draw_casing; Casing tab > Show Casing in Plan (spec.casing_in_plan); honors Use Interior /… |
| 4 | Door dialog >> Lintel tab | cap | NO SPEC (DW-111) | Works | dialogs/opening.rs DOOR_TABS "Lintel" live; plan-3d openings |
| 5 | Door dialog >> Sill/Threshold tab | cap | DW-81, DW-85 | Works | opening_symbol.rs add_threshold_and_sill (PartKind::Threshold: hinged, double, sliding and fixed doors in exterior walls; spec.threshold, S… |
| 6 | Door dialog >> Lites tab | cap | NO SPEC (DW-112) | Works | dialogs/opening.rs "Lites" live (lites across/vertical, muntin width) |
| 7 | Door dialog >> Jamb tab | cap | DW-82 | Partial, In progress (Round 14) | Door jamb blocks in plan (opening_symbol.rs add_frame_blocks, spec.jamb_in_plan, extras.jamb_width) and window frame blocks; spec.size_incl… |
| 8 | Door dialog >> Arch tab | cap | NO SPEC (DW-113) | Works | dialogs/opening.rs "Arch" live |
| 9 | Door dialog >> Hardware tab (handles, locks, hinges, sliding tracks) | cap | DW-55 | Partial | dialogs/opening.rs "Hardware" tab live; hardware library selection is limited |
| 10 | Door dialog >> Shutters tab | cap | DW-84 | Works | spec.shutters (Shutters tab: style, sides, width, color, closed, outside casing); opening_symbol.rs add_shutters; plan-3d casing.rs add_shu… |
| 11 | Door dialog >> Opening Indicators tab | cap | DW-83 | Works | spec.indicators (Opening Indicators tab: Show Opening Indicators in Plan, Swing Direction Arrows); opening_symbol.rs add_indicators (PartKi… |
| 12 | Door dialog >> Rough Opening tab (total size, header height, additional space / clearance gaps, concrete cuto… | cap | DW-56 | Works | dialogs/opening/tabs.rs rough_opening (Additional Space or Clearance Gap, concrete cutout stored, Show Rough Opening in Plan); spec.rough; Opening::framed feeds plan-framing; schedule Rough Opening column; s42 rough_opening_edits_the_model_the_plan_and_the_framing_and_undoes. Use Clearance Gaps / Panel Offset of General (DW-56 remainder) is not built |
| 13 | Door dialog >> Framing tab (header, trimmers, king studs, sills) | cap | NO SPEC (DW-114) | Works | dialogs/opening/tabs.rs framing; plan-framing wall.rs frame_opening; s42 framing_tab_overrides_reach_plan_framing |
| 14 | Door dialog >> Energy Values tab (U-factor, SHGC) | cap | NO SPEC (DW-115) | Works | dialogs/opening/tabs.rs energy_values; spec.energy; schedule U-Factor and SHGC columns |
| 15 | Door dialog >> Layer tab | cap | NO SPEC (DW-116) | Partial | dialogs/opening/tabs.rs layer_tab stores spec.layer (Opening::layer_name); the plan drawing still uses the fixed Doors layer (integration queue) |
| 16 | Door dialog >> Materials tab | cap | NO SPEC (DW-117) | Works | dialogs/opening/tabs.rs materials_tab; spec.materials; plan-3d opening.rs paints each component; Project::sync_opening_materials |
| 17 | Door dialog >> Label tab | cap | DW-59, DW-62, DW-63, DW-104 | Works | openings.rs size_text / plan_label; opening_view.rs draws the label; scenarios s13 |
| 18 | Door dialog >> Components tab | cap | DW-55 | Missing | tab disabled in DOOR_TABS |
| 19 | Door dialog >> Object Information tab | cap | NO SPEC (DW-118) | Works | dialogs/opening/tabs.rs object_information (ID, description, manufacturer, model, supplier, notes shared with the Schedule tab) |
| 20 | Door dialog >> Schedule tab | cap | DW-60, DW-61 | Works | opening_view.rs shows the schedule mark in its bubble when a schedule numbers the opening |

### Window Specification tabs

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Window dialog >> General tab (type, swing, size, position, component options) | cap | DW-47, DW-50, DW-51, DW-54 | Works | opening_symbol.rs Window, Casement (swing arcs), Fixed, SlidingWindow, Awning, Hopper |
| 2 | Window dialog >> Options tab (corner blocks, egress, tempered, display open, recess) | cap | DW-36, DW-86 | Works | opening_symbol.rs plan_symbol uses extras.show_open_in_plan (closed leaf, no arc); Show Open in 3D with its Open slider is `spec.show_open_… |
| 3 | Window dialog >> Casing / Lintel / Sill tabs | cap | DW-79, DW-85 | Partial | opening_symbol.rs casing_parts; opening_view.rs draw_casing; Casing tab > Show Casing in Plan (spec.casing_in_plan); honors Use Interior /… |
| 4 | Window dialog >> Sash tab | cap | NO SPEC (DW-119) | Works | dialogs/opening.rs WINDOW_TABS "Sash" live |
| 5 | Window dialog >> Frame tab | cap | DW-82 | Partial, In progress (Round 14) | Door jamb blocks in plan (opening_symbol.rs add_frame_blocks, spec.jamb_in_plan, extras.jamb_width) and window frame blocks; spec.size_incl… |
| 6 | Window dialog >> Lites tab | cap | NO SPEC (DW-120) | Works | dialogs/opening.rs "Lites" live |
| 7 | Window dialog >> Shape tab (heights, corners) | cap | NO SPEC (DW-121) | Works | dialogs/opening/tabs.rs shape_tab; plan-core spec/shape.rs WindowShape; plan-3d opening/shape.rs; s42 a_window_shape_changes_the_3d_glazing_and_the_plan_head_marks |
| 8 | Window dialog >> Arch tab | cap | NO SPEC (DW-122) | Works | dialogs/opening.rs "Arch" live |
| 9 | Window dialog >> Treatments tab (curtains, blinds, exterior millwork) | cap | NO SPEC (DW-123) | Partial | dialogs/opening/tabs.rs treatments_tab; plan-3d opening/treatments.rs (curtains, blinds, interior shutters, exterior millwork; plan omits them); library styles are not built |
| 10 | Window dialog >> Shutters tab | cap | DW-84 | Works | spec.shutters (Shutters tab: style, sides, width, color, closed, outside casing); opening_symbol.rs add_shutters; plan-3d casing.rs add_shu… |
| 11 | Window dialog >> Opening Indicators tab | cap | DW-83 | Works | spec.indicators (Opening Indicators tab: Show Opening Indicators in Plan, Swing Direction Arrows); opening_symbol.rs add_indicators (PartKi… |
| 12 | Window dialog >> Rough Opening / Framing / Energy Values tabs | cap | DW-56 | Works | Same tabs as the door (dialogs/opening/tabs.rs); a window's rough opening shares its extra height between head and sill and its framed sill follows it |
| 13 | Window dialog >> Layer / Materials / Components / Object Information tabs | cap | DW-55 | Partial | Layer (stored, plan drawing integration pending), Materials and Object Information built (dialogs/opening/tabs.rs); the Components tab stays dimmed |
| 14 | Window dialog >> Label and Schedule tabs | cap | DW-59, DW-60, DW-62, DW-104 | Works | openings.rs size_text / plan_label; opening_view.rs draws the label; scenarios s13 |

### Room Types and Room Specification

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Room dialog >> Room Types list dialog (Edit, Copy, Rename, Delete, In Use) | cap | R-37, R-38, R-39 | Partial, In progress (Round 14) | manual 4.10: Add, Edit, Rename, Delete Gap: No Copy, Select All, Clear All. |
| 2 | Room dialog >> General tab (name, function, living area, conditioned) | cap | R-20, R-21, R-40, R-41, R-42, R-43 | Partial | dialogs/room.rs Gap: Function shown read-only (follows type). |
| 3 | Room dialog >> Structure tab (heights, finishes, slab, stem wall, platforms) | cap | R-22, R-23, R-24, R-25, R-26, R-27, R-28, R-29, R-30, R-31, R-32 | Partial, In progress (Round 14) | RoomName.rough_ceiling stored Gap: 3D ignores it. |
| 4 | Room dialog >> Moldings tab (base, crown, chair rail) | cap | R-34 | Partial, In progress (Round 14) | RoomName.moldings Gap: Base/Crown names stored; 3D molding geometry not built. |
| 5 | Room dialog >> Fill Style tab | cap | R-35 | Works | RoomName.fill_style; render |
| 6 | Room dialog >> Materials tab | cap | R-36 | Partial, In progress (Round 14) | floor_finish/ceiling_finish Gap: Other surfaces planned. |
| 7 | Room dialog >> Label tab | cap | R-44, R-45, R-46, R-47, R-48, R-50 | Partial, In progress (Round 14) | Label tab Gap: Text Style disabled. |
| 8 | Room dialog >> Components tab | cap | L-35 | Partial, In progress (Round 14) | dialogs/room.rs Components live (floor and ceiling finish layers) |
| 9 | Room dialog >> Wall Covering tab (room wall coverings) (not captured) | n | NO SPEC (R-84) | Partial | dialogs/room.rs "Wall Covering" tab live; walls cannot yet carry their own coverings (see Wall dialog) |
| 10 | Room dialog >> Object Information and Schedule tabs | cap | L-29, L-30 | Partial, In progress (Round 14) | Door and window Schedule tab (dialogs/opening.rs `schedule_tab`: Include in Schedule, Mark, Manufacturer, Model, Supplier, Comment in spec.… |

### Cabinet Specification tabs

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Cabinet dialog >> General (style, size, countertop, backsplash, toe kick) | cap | CB-7, CB-14 | Partial, In progress (Round 14) | dialogs/cabinet.rs Gap: Fill Style, Components, Object Info, Schedule disabled; Sides/Back not editable. |
| 2 | Cabinet dialog >> Box Construction | cap | CB-7 | Partial, In progress (Round 14) | dialogs/cabinet.rs Gap: Fill Style, Components, Object Info, Schedule disabled; Sides/Back not editable. |
| 3 | Cabinet dialog >> Front/Sides/Back (face item tree) | cap | CB-10, CB-11 | Works | dialogs/cabinet.rs Front/Sides/Back (Left, Right and Back faces: Plain, Finished Panel, Open, Custom Face) |
| 4 | Cabinet dialog >> Door/Drawer (styles, handles, hinges) | cap | CB-12 | Partial, In progress (Round 14) | dialogs/cabinet.rs Door/Drawer Gap: Built-in styles only; library styles planned. Hardware is complete: knob, bar pull, cup pull, edge pull… |
| 5 | Cabinet dialog >> Accessories (pilasters, feet, side panels) | cap | CB-7 | Partial, In progress (Round 14) | dialogs/cabinet.rs Gap: Fill Style, Components, Object Info, Schedule disabled; Sides/Back not editable. |
| 6 | Cabinet dialog >> Opening Indicators / Moldings / Layer / Fill Style / Materials / Label | cap | CB-7 | Partial, In progress (Round 14) | dialogs/cabinet.rs Gap: Fill Style, Components, Object Info, Schedule disabled; Sides/Back not editable. |
| 7 | Cabinet dialog >> Components / Object Information / Schedule | cap | CB-7, CB-21 | Partial, In progress (Round 14) | dialogs/cabinet.rs Gap: Fill Style, Components, Object Info, Schedule disabled; Sides/Back not editable. |

### Dimension Defaults tabs

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Dimension dialog >> General tab (baseline separation, rounding, text position, leader, 3D display) | cap | DIM-6, DIM-8 | Works | dialogs/default_lists.rs Saved Dimension Defaults; 14 template sets |
| 2 | Dimension dialog >> Setup Automatic tab (exterior, room, elevation) | cap | DIM-24, DIM-40 | Partial | DimensionDefaults.temp_locate/elevation_locate (LocateGroup); default_lists.rs Locate Objects tab (Manual and Automatic / Temporary / Eleva… |
| 3 | Dimension dialog >> Setup Temporary tab | cap | S-56, S-62, DIM-40 | Partial | Locate groups merged into one Locate Objects tab (default_lists.rs DIM_TABS); temporary-dimension row limit and reach are not separate fiel… |
| 4 | Dimension dialog >> Locate Manual / End to End / Centerline / Interior tabs | cap | DIM-4, DIM-11, DIM-13, DIM-14, DIM-19 | Partial | one merged Locate Objects tab (default_lists.rs) instead of one tab per tool |
| 5 | Dimension dialog >> Locate Auto Exterior / Auto Room / Auto Elevation / Elevations tabs | cap | DIM-26, DIM-27, DIM-28 | Partial | merged; Auto Elevation is Differs-by-design (DIM-28) |
| 6 | Dimension dialog >> Primary Format tab | cap | DIM-6 | Works | dialogs/default_lists.rs Saved Dimension Defaults; 14 template sets |
| 7 | Dimension dialog >> Secondary Format tab (second unit in parentheses) | cap | NO SPEC (DIM-46) | Missing | no secondary format in default_lists.rs DIM_TABS or dialogs/dimension.rs |
| 8 | Dimension dialog >> Extensions tab (length, fixed proximity, centerline mark) | cap | DIM-7 | Works | render.rs DimLook; DimensionDefaults.printed_size; TextStyle printed size |
| 9 | Dimension dialog >> Arrow tab | cap | DIM-39 | Partial, In progress (Round 14) | dialogs/dimension.rs (tabs live, Format and Arrow editable); see DIM-31 Gap: Secondary format and tolerance absent. |
| 10 | Dimension dialog >> Text Style tab | cap | DIM-39 | Partial, In progress (Round 14) | dialogs/dimension.rs (tabs live, Format and Arrow editable); see DIM-31 Gap: Secondary format and tolerance absent. |
| 11 | Dimension dialog >> Layer tab | cap | DIM-39 | Partial, In progress (Round 14) | dialogs/dimension.rs (tabs live, Format and Arrow editable); see DIM-31 Gap: Secondary format and tolerance absent. |

### Other object dialogs (not captured)

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Other dialogs >> Stairs Specification (General, Style, Newels/Balusters, Rails, Line Style, Fill, Materials,… (not captured) | n | CB-32 | Partial, In progress (Round 14) | dialogs/stairs.rs Gap: Components and Schedule tabs open. |
| 2 | Other dialogs >> Landing Specification (not captured) | n | CB-27, CB-32 | Partial, In progress (Round 14) | dialogs/stairs.rs Gap: Components and Schedule tabs open. |
| 3 | Other dialogs >> Roof Plane Specification (General, Holes, Build Roof Edge, Options, Structure, Materials) (not captured) | n | RF-36 | Works | dialogs/roof.rs Structure tab with Define (framing, member size, spacing, sheathing, roofing, ceiling framing); roof_view.rs RoofStructure,… |
| 4 | Other dialogs >> Edit All Roof Planes (not captured) | n | RF-39 | Works | dialogs/roof.rs AllPlanesDialog; tools/roof.rs RoofMode::EditAll; roof_view.rs AllPlanesEdit, apply_all; tests edit_all_roof_planes_applies… |
| 5 | Other dialogs >> Build Roof dialog (not captured) | n | RF-2 | Works | dialogs/roof.rs tabs Roof/Options/Materials |
| 6 | Other dialogs >> Dormer / Skylight / Roof Hole specification (not captured) | n | RF-51, RF-43, RF-42 | Works | roof_view.rs slide_dormer; tools/roof.rs drag_step; tests a_dormer_slides_onto_the_other_plane_when_dragged_across_the_ridge, dragging_a_do… |
| 7 | Other dialogs >> Camera Specification (position, direction, FOV, backdrop, rendering) (not captured) | n | C-30 | Works | dialogs/camera.rs |
| 8 | Other dialogs >> Camera View Options dialog (not captured) | n | C-31 | Partial, In progress (Round 14) | dialogs/camera.rs Rendering tab, elevation options, `CameraObject.vector` Gap: Per camera: hatch, shadows, depth weights, labels, level cal… |
| 9 | Other dialogs >> 3D View Defaults dialog (not captured) | n | C-68 | Partial, In progress (Round 14) | dialogs/camera.rs Gap: Eye height, angle, technique only. |
| 10 | Other dialogs >> Framing member specification (General, Truss, Line Style, Layer) (not captured) | n | CB-35 | Works | tools/framing.rs 19 tools |
| 11 | Other dialogs >> Build Framing dialog (not captured) | n | CB-36 | Partial, In progress (Round 14) | Build Framing / Build All Framing commands; Framing Defaults page (`dialogs/framing.rs` FramingDefaultsDialog, `framing_view::settings`) Ga… |
| 12 | Other dialogs >> Electrical Specification (General, Switches, Materials, Label, Layer) (not captured) | n | E-12 | Partial, In progress (Round 14) | `dialogs/electrical.rs` |
| 13 | Other dialogs >> Slab / Footing / Pad / Pier specification (not captured) | n | NO SPEC (R-85) | Works | dialogs/foundation.rs SLAB_TABS, HOLE_TABS, PAD_TABS |
| 14 | Other dialogs >> Build Foundation dialog (not captured) | n | R-61, R-62 | Partial, In progress (Round 14) | floors.rs build_foundation; FoundationKind Gap: Walls with footings, monolithic, piers; no basement. |
| 15 | Other dialogs >> Terrain Specification (not captured) | n | CB-51 | Works | dialogs/terrain.rs (General, Contours, Building Pad, Materials, Layer; Cut and Fill Report and Import Terrain Data forms); `TerrainRecord::… |
| 16 | Other dialogs >> Terrain object dialogs (hill, garden bed, plant, road, driveway, sidewalk, sprinkler) (not captured) | n | CB-45, CB-48, CB-49, CB-50 | Partial, In progress (Round 14) | plant runs (`Landscape`); Plant Chooser (`tools/terrain/scape.rs plant_categories, plants_in`; dialogs/terrain/object.rs `plant_chooser`);… |
| 17 | Other dialogs >> Text Specification (Text, Text Style, Appearance, Layer) (not captured) | n | TXT-16 | Partial, In progress (Round 14) | dialogs/text.rs Appearance tab: alignment left/center/right and top/middle/bottom, wrap width, minimum height, border (margin, weight), bac… |
| 18 | Other dialogs >> CAD object specifications (line, arc, circle, polyline, spline) (not captured) | n | CAD-38 | Works | dialogs/cad.rs |
| 19 | Other dialogs >> Symbol (library object) Specification (General, Options, 3D, Layer, Label) (not captured) | n | CB-57, CB-60 | Partial | dialogs/symbol.rs Library Object Specification: General (Keep aspect, Reflect), Options, Materials, Label, Layer, Object Information, Schedule; dialogs/library_object.rs (Open Object) Gap: the 3D and Components tabs are disabled; Options and Schedule choices are stored, not yet used by 3D or schedules |
| 20 | Other dialogs >> Image / Distributed Object specification (not captured) | n | NO SPEC (CAD-60) | Works | dialogs/images.rs IMAGE_TABS, DIST_TABS |
| 21 | Other dialogs >> Layout Box Specification (not captured) | n | L-4 | Partial | dialogs/layout.rs General/Source/Line Style; quarter-turn Rotation, rotate knob on the selected box, hit test and outline use the turned co… |
| 22 | Other dialogs >> Page Specification / Page Setup / Customize Sheet Sizes (not captured) | n | L-7, L-8 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 23 | Other dialogs >> Schedule Specification (not captured) | n | L-25 | Works | dialogs/schedule_spec.rs |
| 24 | Other dialogs >> Send to Layout dialog (not captured) | n | L-2 | Works | `dialogs/layout.rs SendDialog` (Layout file, View, Page, Scale, Position; `LayoutTarget`), `layout_window.rs send_to`, `new_layout_file`, `… |
| 25 | Other dialogs >> Print dialog (scale, tiling, color mode) (not captured) | n | L-18 | Works | File > Print dialog (dialogs/print.rs): PDF file, system printer picked from `lpstat -p` (default printer otherwise) through `lp -d`, or vi… |
| 26 | Other dialogs >> Export DXF / DWG dialog (version, units, selection) (not captured) | n | L-44 | Partial | plan-core export/dxf.rs R12 ASCII; elevation DXF Gap: No version, units, selection options; no DWG. |
| 27 | Other dialogs >> Plan View Specification (not captured) | n | LAY-2 | Works | plan-core layer_sets.rs; Project.plan_views |
| 28 | Other dialogs >> Layer Display Options / Layer Set dialogs (not captured) | n | LAY-1, LAY-3 | Works | plan-core layers.rs |
| 29 | Other dialogs >> Reference Display dialog (not captured) | n | R-65 | Works | dialogs/reference_display.rs (reference_walls); render.rs draw_reference_floor; snap.rs snap_with_reference |
| 30 | Other dialogs >> Wall Type Definitions (layers, materials, library) (not captured) | n | W-46, W-47, W-49 | Partial | dialogs/wall_types.rs Gap: Wall Types Library button disabled. |
| 31 | Other dialogs >> Material Specification / Material Builder (Pattern, Texture, Properties, Materials List) (not captured) | n | C-61 | Works | tools/materials.rs builder_window (color, roughness, metallic, transparency, pattern, texture file) saves to ~/.plan-studio/materials.json;… |
| 32 | Other dialogs >> Fill Style dialog (pattern, color, scale, angle) (not captured) | n | CAD-13 | Works | dialogs/cad.rs Line/Fill Style tabs |
| 33 | Other dialogs >> Line Style / Arrow Style dialogs (not captured) | n | CAD-13, CAD-38 | Works | dialogs/cad.rs Line/Fill Style tabs |
| 34 | Other dialogs >> Number Style (Primary Format) dialog (not captured) | n | DIM-6 | Works | dialogs/default_lists.rs Saved Dimension Defaults; 14 template sets |
| 35 | Other dialogs >> Revision Table / Revision Cloud dialog (Project Information > Revisions) (not captured) | n | NO SPEC (L-54) | Works | dialogs/project_info.rs "Revisions" tab; plan-layout revision table |
| 36 | Other dialogs >> Plan Check Settings and results (not captured) | n | NO SPEC (APP-27) | Works | dialogs/plan_check.rs |
| 37 | Other dialogs >> Delete Objects dialog (not captured) | n | S-88 | Works | dialogs/delete_objects.rs |
| 38 | Other dialogs >> Transform/Replicate dialog (not captured) | n | S-103 | Works | dialogs/transform.rs |
| 39 | Other dialogs >> Align/Distribute dialog (not captured) | n | S-54 | Works | dialogs/transform.rs AlignDialog; plan-core transform.rs |
| 40 | Other dialogs >> Find/Replace Text dialog (not captured) | n | TXT-12 | Partial | dialogs/find_replace.rs; Edit > Find/Replace Text (menus.rs:505, Action::FindReplaceText); plan-core find_text.rs Gap: Find/Replace works;… |
| 41 | Other dialogs >> Unsaved changes prompt / auto-archive / recover (not captured) | n | NO SPEC (APP-59) | Works | dialogs/unsaved.rs; files.rs auto archive |

### Preferences dialog pages

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Preferences >> Appearance / Colors page | cap | PR-3 | Works | `preferences::tint_palette`, `pages::icon_px` (read by `toolbar.rs` for every icon); `the_canvas_colors_go_over_the_theme`, `applying_a_fil… |
| 2 | Preferences >> Text page | cap | PR-5 | Works | `preferences.rs fonts`, `ui::text_links` (opens Default Settings) |
| 3 | Preferences >> Library Browser page | cap | PR-4 | Partial, In progress (Round 14) | Page and file: `ui::library_browser`, `pages::LibraryBrowserPrefs`. `shell/library_browser.rs` still draws at its `PREVIEW_PX` and searches… |
| 4 | Preferences >> Render page | cap | PR-6 | Partial, In progress (Round 14) | `pages::view_settings()` returns the `ViewSettings` a new 3D view should start with; the 3D panel still starts from `ViewSettings::default(… |
| 5 | Preferences >> Materials List page | cap | PR-7 | Partial, In progress (Round 14) | Page and file only (`pages::MaterialsPrefs`); `dialogs/materials.rs` does not read them yet (queued). Verify in Chief for the option set |
| 6 | Preferences >> Reset Options page | cap | PR-8 | Works | `ui::run_reset`, `ui::reset_options`; `reset_options_reset_what_they_name`; `pages::tests::dont_ask_again_is_kept_and_reset`. No dialog ask… |
| 7 | Preferences >> Folders page | cap | PR-9 | Partial, In progress (Round 14) | `ui::folders`, `pages::folder_status`, `pages::folder(kind)`; `pages::tests::folders_show_the_default_or_the_override_and_whether_it_exists… |
| 8 | Preferences >> Edit page | cap | PR-10 | Partial, In progress (Round 14) | `ui::edit`; marquee is applied live (`select::set_marquee_mode`); the snap switches write the plan's editing defaults; rotate/resize about… |
| 9 | Preferences >> Behaviors page | cap | PR-11 | Partial, In progress (Round 14) | `ui::behaviors`; Edit Type and Replicate settings go to `EditingDefaults.behavior` live; the camera steps are kept in `PagePrefs.behaviors`… |
| 10 | Preferences >> Snap Properties page | cap | PR-12 | Works | `ui::snaps`; `saved_editing_defaults_reach_a_plan_on_the_first_frame`; `ui::parse_angles` test `allowed_angles_parse_and_print`. Kept in `p… |
| 11 | Preferences >> Architectural page | cap | PR-13 | Partial, In progress (Round 14) | Cabinet options work as before. The auto-rebuild switches are kept (`PagePrefs.architectural`) and read by nothing yet: the roof and wall r… |
| 12 | Preferences >> CAD page | cap | PR-14 | Partial, In progress (Round 14) | `ui::cad`, `pages::CadPrefs` (Daniel's X18 line weights at first); the CAD painter does not read them yet (queued) |
| 13 | Preferences >> General Plan Defaults page | cap | PR-15 | Works | `ui::plan_defaults` |
| 14 | Preferences >> Unit Conversions page | cap | PR-16 | Works | `ui::convert`; `unit_conversions_show_every_unit`. Chief's page is not captured; this is a converter with a starting unit and rounding (ver… |
| 15 | Preferences >> Fonts page (system fonts, replace fonts) (not captured) | n | TXT-12 | Partial | dialogs/find_replace.rs; Edit > Find/Replace Text (menus.rs:505, Action::FindReplaceText); plan-core find_text.rs Gap: Find/Replace works;… |
| 16 | Preferences >> Templates page (default plan and layout templates) (not captured) | n | NO SPEC (APP-60) | Works | dialogs/defaults.rs Templates; templates.rs |
| 17 | Preferences >> Measurement units: feet-inches, decimal feet, inches, millimeters, centimeters, meters (not captured) | n | DIM-6, W-19 | Works | plan-core units.rs UnitSystem, LengthUnit; dialogs/dimension.rs units; project units |
| 18 | Preferences >> Hotkeys and Toolbars (Customize dialogs) (not captured) | n | HK-1, TB-1 | Works | `dialogs/hotkeys.rs menu_of`, `grouped_commands`; `the_list_is_grouped_by_menu_and_the_search_narrows_it`. The menu of a command is read fr… |

### Product features not in the captures: 3D, rendering and presentation

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Presentation (not captured) >> Ray Trace / Physically Based rendering to an image | n | C-51, C-52, C-62 | Works | plan-render; Ray Trace window |
| 2 | Presentation (not captured) >> Final View vs Preview quality, ray-trace region and resolution | n | C-45 | Partial, In progress (Round 14) | View3d quality settings (plan-view3d quality.rs, View3d RenderSettings); no render-region rectangle |
| 3 | Presentation (not captured) >> Photon mapping / global illumination bake | n | NO SPEC (C-76) | Differs-by-design | plan-render is a path tracer (plan-render integrator.rs); it needs no photon pass |
| 4 | Presentation (not captured) >> 360 panorama / Virtual Tour export | n | NO SPEC (C-77) | Missing | no equirectangular or cube camera (grep panorama finds nothing) |
| 5 | Presentation (not captured) >> Walkthrough video file export (mp4/mov) | n | C-71 | Partial | records a numbered PNG sequence only (view3d_panel.rs record_walkthrough) |
| 6 | Presentation (not captured) >> Walkthrough paths: key frames, speed, look-at | n | C-71 | Works | tools/camera.rs; Play/Record |
| 7 | Presentation (not captured) >> 3D Viewer / Chief 3D sharing (cloud) | n | NO SPEC (C-78) | Missing | Chief's online 3D Viewer service is out of scope; glTF export is the replacement (menus.rs glTF…) |
| 8 | Presentation (not captured) >> Photo sharing / Share to social, Chief cloud | n | NO SPEC (C-79) | Missing | cloud sharing is out of scope; Export Picture (Missing) is the local equivalent |
| 9 | Presentation (not captured) >> Camera clipping planes and Back Clipped views | n | C-19, C-28 | Works | tools/camera.rs; clip handle |
| 10 | Presentation (not captured) >> Backdrops (3D sky and ground images) | n | C-70 | Partial, In progress (Round 14) | sky gradient and ground fade behind Standard and Physically Based (`quality::sky_colors`) Gap: Colours derive from the technique's backgrou… |
| 11 | Presentation (not captured) >> Sun angles, date/time/latitude, shadows | n | C-63, C-67 | Partial, In progress (Round 14) | Sun Angle window (date, time, latitude, azimuth/altitude) Gap: No longitude, DST, north direction; no Move Sun/Moon. |
| 12 | Presentation (not captured) >> Lighting sets (Day, Evening, Interior lights) | n | C-65 | Missing, In progress (Round 14) | manual 10.6: not built |
| 13 | Presentation (not captured) >> Vector View technique | n | C-47 | Works | plan-elevation; Vector View panel |
| 14 | Presentation (not captured) >> Technical Illustration technique | n | C-48 | Works | manual 10.7 |
| 15 | Presentation (not captured) >> Watercolor / Line Drawing / Duotone techniques | n | C-49 | Partial, In progress (Round 14) | composite-pass filters in `pipeline.rs` (wash, wobbling edge darkening, paper grain; flat lines; two-tone) Gap: Screen-space filters, not C… |
| 16 | Presentation (not captured) >> Glass House | n | C-50 | Works | manual 10.4; `Look::GlassHouse` |
| 17 | Presentation (not captured) >> Doll House View | n | C-13 | Works | toolbar.rs DollHouse |
| 18 | Presentation (not captured) >> Overview cameras (full, floor, framing, orthographic) | n | C-10, C-11, C-12, C-14, C-15 | Partial, In progress (Round 14) | no entry in toolbar.rs Gap: Framing members only via manual framing in 3D. |
| 19 | Presentation (not captured) >> Cross Section / Elevation / Back-Clipped / Wall Elevation cameras | n | C-17, C-18, C-19, C-20 | Works | tools/camera.rs; `plan_elevation::section_free` (free-angle `FreeView`); dialogs/camera.rs `free_view` |
| 20 | Presentation (not captured) >> Auto Interior and Exterior Elevations with dimensions | n | C-21, DIM-28 | Works | manual 10.11; tools/camera.rs |
| 21 | Presentation (not captured) >> Section Fill (poche) in cross sections | n | C-18 | Works | plan-elevation; manual 10.7 |
| 22 | Presentation (not captured) >> Hatch and material patterns in elevations | n | L-13, C-18 | Works | Elevation and section boxes hatch at the box scale (`wall_face_hatch`); camera drawings are hatched again for the box scale (`Drawing::reha… |
| 23 | Presentation (not captured) >> Material patterns, textures, material classes (metal, glass, mirror) | n | C-61 | Works | tools/materials.rs builder_window (color, roughness, metallic, transparency, pattern, texture file) saves to ~/.plan-studio/materials.json;… |
| 24 | Presentation (not captured) >> Plan Materials list (materials used in the plan) | n | C-61 | Partial | Materials list dialog (tools/materials LIST) lists library and plan materials; no usage counts |
| 25 | Presentation (not captured) >> Edge lines, line weights, and fog / sky color in 3D | n | C-69, C-70 | Partial, In progress (Round 14) | edge overlay; vector line weights from layer pens (`dialogs::camera::layer_weights`) Gap: No weight setting for Standard edges. |

### Documentation and layout

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Documents (not captured) >> Layout multi-page set, page template, sheet index | n | L-7, L-10, L-11 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 2 | Documents (not captured) >> Layout boxes: plan, elevation, section, CAD detail, schedule, text | n | L-4, L-5, L-6 | Partial, In progress (Round 14) | dialogs/layout.rs General/Source/Line Style; quarter-turn Rotation, rotate knob on the selected box, hit test and outline use the turned co… |
| 3 | Documents (not captured) >> Layout box linking and refresh | n | L-3, L-17 | Works | layout_window.rs live redraw; Update Layout Views |
| 4 | Documents (not captured) >> Layout line weights, scale, fill scaling | n | L-12, L-13, L-22 | Works | plan-layout render; Line weight scaling 0.1-5x |
| 5 | Documents (not captured) >> Layout page drawing: CAD, text, dimensions on the sheet | n | L-15, L-16 | Works | plan-layout render.rs draw_cad_item_styled, dimension_text_pt; tests printed_size_text_prints_the_same_size_at_any_box_scale |
| 6 | Documents (not captured) >> Revision tables and revision clouds on sheets | n | CAD-36, L-9 | Works | project_info.rs Revisions; layout title block revision table; CAD Revision Cloud |
| 7 | Documents (not captured) >> Title block macros and project information fields | n | L-9 | Works | plan-core schedules.rs ProjectInfo::macro_pairs and expand (%client.phone%, %client.email%, %company%, %checked.by%, %custom.key%); shell/l… |
| 8 | Documents (not captured) >> Print to PDF, multi-page, bookmarks, fonts | n | L-20 | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes Gap: The two standard fonts are ref… |
| 9 | Documents (not captured) >> Print Model, Print Image, Print Layout | n | L-19 | Works | Print Preview window (`PrintPreviewDialog`, `plan_layout::layout_print_preview` / `plan_view_print_preview`): the pages as the Print dialog… |
| 10 | Documents (not captured) >> Door schedule | n | L-23, L-24 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 11 | Documents (not captured) >> Window schedule | n | L-23, L-24 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 12 | Documents (not captured) >> Room schedule | n | L-23, L-24 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 13 | Documents (not captured) >> Wall schedule | n | L-23, L-24 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 14 | Documents (not captured) >> Cabinet schedule | n | CB-21 | Works | schedule_kinds.rs; C-01 labels; columns Width, Depth, Height, Elevation, Countertop, Door Style, Drawer Style, Finish, Hardware (the new on… |
| 15 | Documents (not captured) >> Fixture / Furniture / Plumbing schedule | n | L-23 | Partial | Fixture and furniture rows come from placed library symbols by category (schedule_kinds.rs); plumbing fixtures are filed under Fixture |
| 16 | Documents (not captured) >> Electrical schedule | n | E-13 | Works | `plan-electrical/src/circuit.rs::schedule_rows`; `tests::the_schedule_rows_carry_mark_type_height_circuit_and_flags`; `plan-docs/schedule_k… |
| 17 | Documents (not captured) >> Room Finish schedule | n | L-30 | Works | plan-core schedules.rs ScheduleKind::RoomFinish (floor, wall, base, crown and ceiling finishes); tools/schedule.rs |
| 18 | Documents (not captured) >> Framing schedule | n | CB-41 | Works | Framing members counted in stock lengths with board feet, waste and unit prices; roofing lines; CSV, PDF page in the construction set, layo… |
| 19 | Documents (not captured) >> Plant schedule | n | L-23 | Partial | rows built from placed plants (schedule_kinds.rs Plant) |
| 20 | Documents (not captured) >> Note schedule (notes by type) | n | TXT-9 | Partial | Note Types exist (TXT-9); a note schedule table is not built |
| 21 | Documents (not captured) >> Custom schedules, column editor, grouping, totals | n | L-25, L-28, L-31 | Partial, In progress (Round 14) | Schedule Specification builds the table (kind, floors, filter, columns shown/renamed/ordered, sort, group, totals; Show All and Reset Colum… |
| 22 | Documents (not captured) >> Materials List / estimating (waste, price, stock lengths) | n | L-33, L-34, L-35, L-36 | Partial, In progress (Round 14) | Walls with a type are quantities by layer (`materials.rs component_of`): sheathing and drywall in sheets, siding/stucco/brick/stone in Sidi… |
| 23 | Documents (not captured) >> Export Materials List to XLS and CSV | n | L-37, L-32 | Works | CSV and Excel export; Send to Layout table box; PDF sheet and construction-set pages; Excel from the layout page (`tables_file`) |
| 24 | Documents (not captured) >> Annotation Sets (named text/dimension display groups) | n | NO SPEC (LAY-34) | Missing | no annotation sets; layer sets and plan views (LAY-2) cover the same job by layers |
| 25 | Documents (not captured) >> Dimension defaults per view / plan view | n | LAY-2, DIM-40 | Partial | DimensionDefaults.temp_locate/elevation_locate (LocateGroup); default_lists.rs Locate Objects tab (Manual and Automatic / Temporary / Eleva… |
| 26 | Documents (not captured) >> Saved Plan Views, Layer Sets, Default Sets | n | LAY-2, LAY-8, LAY-11 | Works | plan-core layer_sets.rs; Project.plan_views |
| 27 | Documents (not captured) >> Auto Detail and CAD Detail from View | n | L-39, L-40 | Partial, In progress (Round 14) | CAD > Auto Detail and the toolbar button: `tools/details/cad_detail.rs build` / `auto_detail` (section or elevation to a detail: cut lines,… |
| 28 | Documents (not captured) >> Wall detail views (cross-section of a wall) | n | CB-40 | Partial, In progress (Round 14) | Framing Overview plan view; `framing_view::elevation_scene` swaps the wall skins for studs; `plan_framing::wall_detail` Gap: dialogs/camera… |
| 29 | Documents (not captured) >> Insert Point / Elevation Point / Marker symbols | n | TXT-8, CB-44 | Works | TextMode::Marker |
| 30 | Documents (not captured) >> Section and elevation markers linked to views | n | TXT-8 | Partial | Marker tool places bubbles; linking a marker to a camera or detail is a verify-in-Chief item (TXT-8) |
| 31 | Documents (not captured) >> Spell check in text | n | NO SPEC (TXT-21) | Missing | no spell checker (grep spell finds only layout words) |
| 32 | Documents (not captured) >> Rich Text with fonts, bullets, tables | n | TXT-4 | Partial | tools/text.rs RichRun markup Gap: Markup runs not a WYSIWYG box; italic not drawn. |
| 33 | Documents (not captured) >> Text macros (project, date, room name, schedule number) | n | TXT-12, R-47, DW-62 | Partial | dialogs/find_replace.rs; Edit > Find/Replace Text (menus.rs:505, Action::FindReplaceText); plan-core find_text.rs Gap: Find/Replace works;… |
| 34 | Documents (not captured) >> Arrows on text lines and leaders | n | TXT-5, TXT-6 | Works | TextMode::LeaderLine |
| 35 | Documents (not captured) >> Callouts and Markers | n | TXT-7, TXT-8 | Works | TextMode::Callout (circle, hexagon, square) |

### Import and export formats

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Exchange (not captured) >> Import DXF | n | L-43 | Works | dialogs/exchange.rs import window: units, scale, rotation, base and insertion point, per-layer map (keep, skip, plan layer, new name), Conv… |
| 2 | Exchange (not captured) >> Import DWG | n | L-43 | Works | dialogs/exchange.rs import window: units, scale, rotation, base and insertion point, per-layer map (keep, skip, plan layer, new name), Conv… |
| 3 | Exchange (not captured) >> Export DXF | n | L-44, L-45 | Partial | plan-core export/dxf.rs R12 ASCII; elevation DXF Gap: No version, units, selection options; no DWG. |
| 4 | Exchange (not captured) >> Export DWG | n | L-44 | Partial | plan-core export/dxf.rs R12 ASCII; elevation DXF Gap: No version, units, selection options; no DWG. |
| 5 | Exchange (not captured) >> Import PDF as underlay | n | L-46 | Partial, In progress (Round 14) | `plan_core::underlay`, `tools/underlay.rs` (+ `pdf.rs`, `inflate.rs`, `trace.rs`), `dialogs/underlay.rs`: PNG and JPEG (baseline and progre… |
| 6 | Exchange (not captured) >> Import pictures (PNG, JPEG, TIFF, BMP) | n | NO SPEC (CAD-61) | Partial | tools/images.rs reads PNG and JPEG; TIFF and BMP are not read |
| 7 | Exchange (not captured) >> Import 3D symbols OBJ / glTF | n | NO SPEC (CB-81) | Works | plan-import obj.rs, gltf.rs; menus.rs Library > Import 3D Model |
| 8 | Exchange (not captured) >> Import 3DS / SketchUp SKP / COLLADA DAE / STL | n | NO SPEC (CB-82) | Missing | see Library menu row |
| 9 | Exchange (not captured) >> Import terrain data (survey points) | n | NO SPEC (CB-75) | Works | plan-terrain import.rs |
| 10 | Exchange (not captured) >> Export OBJ / STL / 3DS / COLLADA 3D models | n | NO SPEC (L-55) | Missing | only glTF is exported (plan-view3d export.rs) |
| 11 | Exchange (not captured) >> Export PDF (single view, layout set, construction set) | n | L-20 | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes Gap: The two standard fonts are ref… |
| 12 | Exchange (not captured) >> Export picture (JPEG, PNG, TIFF) of a view | n | NO SPEC (L-49) | Missing | see File menu row |
| 13 | Exchange (not captured) >> Export schedule tables as CSV and Excel | n | L-32, L-37 | Works | CSV and Excel (.xlsx, `plan_docs::xlsx`) in the schedule windows, the Schedule Specification, the Materials List, the Framing Takeoff and t… |

### Architectural objects and building systems

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Building (not captured) >> Stairs, ramps, landings, newels and balusters | n | CB-22, CB-23, CB-27, CB-31, CB-34 | Works | tools/stairs.rs; toolbar Stairs flyout |
| 2 | Building (not captured) >> Railings (straight, curved) and deck railings | n | W-56 | Works | rail style tab; plan-3d |
| 3 | Building (not captured) >> Deck framing and decking boards (planking) | n | NO SPEC (CB-86) | Missing | no deck planking or deck framing command (grep planking finds nothing); plan-framing has a Ledger member kind and floor platforms are frame… |
| 4 | Building (not captured) >> Decks and porches as rooms (function Deck, Porch) | n | R-40, R-53 | Partial | rooms.rs function_defaults; plan-3d room_function tests Gap: Garage drops 24" on a 4" slab with no finish; Deck and Porch have no ceiling a… |
| 5 | Building (not captured) >> Foundation types: slab, stem wall, piers, pony walls, stepped footings | n | R-61, R-62, W-52 | Partial, In progress (Round 14) | floors.rs build_foundation; FoundationKind Gap: Walls with footings, monolithic, piers; no basement. |
| 6 | Building (not captured) >> Retaining walls (terrain walls, curbs) | n | CB-46 | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report`; `Feature::round`; TerrainVariant::{PolylineFeature, Ro… |
| 7 | Building (not captured) >> Chimneys and fireplaces | n | NO SPEC (CB-87) | Partial | fireplace symbols come from the library catalog; Roof Hole carries a chimney opening (plan-roof hole.rs); no chimney object that runs throu… |
| 8 | Building (not captured) >> Niches, pass-throughs, wall openings | n | DW-49 | Works | opening_symbol.rs PassThrough and WallNiche (cut band); Opening::niche_depth (spec.niche_depth, 3 1/2 in default); dialogs/opening.rs Wall… |
| 9 | Building (not captured) >> Skylights (roof plane skylights, tubular) | n | RF-43 | Works | roof_view.rs skylight; 3D curb/frame/glass |
| 10 | Building (not captured) >> Garage doors | n | DW-43, DW-90 | Works | opening_symbol.rs Garage (panel and dashed overhead path); plan-3d doors.rs; toolbar.rs G, D |
| 11 | Building (not captured) >> Bay, box and bow windows | n | DW-48, RF-29 | Partial, In progress (Round 14) | opening_symbol.rs projection_footprint (bay, bow, box); plan-3d windows.rs shares it; roof: plan-3d roof.rs bay_roof_into (hip or shed roof… |
| 12 | Building (not captured) >> Window treatments (curtains, blinds) | n | NO SPEC (DW-123) | Missing | see Window dialog Treatments tab |
| 13 | Building (not captured) >> Door hardware, casing and moldings | n | DW-55, DW-79 | Partial | Library Style stored Gap: Components tab disabled. |
| 14 | Building (not captured) >> Molding polylines and 3D moldings (crown, base, chair rail) | n | R-34 | Partial, In progress (Round 14) | RoomName.moldings Gap: Base/Crown names stored; 3D molding geometry not built. |
| 15 | Building (not captured) >> Soffits (ceiling soffits, cabinet soffits) | n | CB-17 | Partial, In progress (Round 14) | Soffit kind Gap: Cabinet-like box, not polyline soffit. |
| 16 | Building (not captured) >> Counters, custom countertops, backsplash | n | CB-14, CB-15 | Works | Touching base cabinets join into one generated top that regenerates on a move, resize, add or delete (`placed::rejoin_countertops`, Prefere… |
| 17 | Building (not captured) >> Cabinet modules, corner and blind cabinets, appliances | n | CB-1, CB-16, CB-19 | Works | toolbar.rs Cabinet flyout (16 kinds); tools/cabinet.rs |
| 18 | Building (not captured) >> Fixtures and appliances from the library | n | CB-53, CB-55, CB-56 | Works | shell/library_browser.rs |
| 19 | Building (not captured) >> Furniture from the library | n | CB-53, CB-55 | Works | shell/library_browser.rs |
| 20 | Building (not captured) >> Plumbing and HVAC symbols and connections | n | NO SPEC (CB-88) | Partial | plumbing and HVAC symbols come from the library; there are no supply/drain/duct connection objects (only electrical connections exist) |
| 21 | Building (not captured) >> Electrical connections, circuits, auto outlets | n | E-5, E-7, E-14 | Works | `layer::connect_in`; s08 `a_light_snaps_to_the_room_center_and_a_switch_is_connected_to_it` |
| 22 | Building (not captured) >> Attic walls and auto attic | n | RF-31, RF-32, RF-33 | Partial | plan-3d cover.rs attic panels, RoofDetail::auto_attic_walls, RoofTypes::attic Gap: Generated at scene time above a lower roof beside a tall… |
| 23 | Building (not captured) >> Roof trusses, girders, truss base | n | RF-54, RF-55 | Partial, In progress (Round 14) | tools/framing.rs; plan-framing truss.rs Gap: Girder auto-doubling not confirmed. |
| 24 | Building (not captured) >> Roof baseline polylines and per-edge overrides | n | RF-35, RF-37 | Works | tools/roof.rs RoofMode::Plane |
| 25 | Building (not captured) >> Curved roofs / curved roof planes | n | NO SPEC (RF-61) | Missing | roof planes are planar; curved walls give faceted eaves only (grep curved roof finds nothing) |
| 26 | Building (not captured) >> Roof holes, skylights, dormers | n | RF-42, RF-43, RF-48, RF-49, RF-50 | Works | tools/roof.rs HolePoly (click corners, double-click, Enter or the first corner closes); roof_view.rs add_hole_polygon, polygon_self_interse… |
| 27 | Building (not captured) >> Gutters, ridge caps, fascia, soffit, gable returns | n | RF-14, RF-15, RF-27 | Partial | fascia, soffit, gable returns work; gutters and ridge caps are Include options with a simple model (roofs.md RF-27) |
| 28 | Building (not captured) >> Eave and rake overhangs, frieze | n | RF-10, RF-15, RF-26 | Works | roof_view.rs:1077 overhang + thick/2 |
| 29 | Building (not captured) >> Room types, room schedule, finish schedule | n | R-37, R-39, L-30 | Works | dialogs/default_lists.rs |
| 30 | Building (not captured) >> Living area and standard area | n | R-49, R-51, R-53 | Works | rooms.rs standard_area_sq_in |
| 31 | Building (not captured) >> Floor and ceiling platforms | n | R-69, W-62 | Partial, In progress (Round 14) | See W-62: Structure tab platform intersections in 3D Gap: Stacked walls use the options per wall; automatic matching of an upper wall over… |
| 32 | Building (not captured) >> Split-level floors (floors at different heights on one level) | n | NO SPEC (R-86) | Missing | floors are whole levels; a room can drop below its floor (R-23, R-26) but there is no split-level floor object (grep split level finds noth… |
| 33 | Building (not captured) >> Invisible walls and room dividers | n | W-55, R-4, R-5 | Works | wall.rs test room_dividers_close_rooms |
| 34 | Building (not captured) >> Pony walls and half walls | n | W-53, W-54 | Works | PonyWall in walls.rs; dialogs/wall.rs |
| 35 | Building (not captured) >> Wall hatching, wall layers, wall caps | n | W-59, W-46, W-50 | Partial | render.rs wall layer bands Gap: Check Display Options Wall Layers toggle coverage. |
| 36 | Building (not captured) >> Wall types editor (layers, materials, library) | n | W-46, W-47, W-49 | Partial | dialogs/wall_types.rs Gap: Wall Types Library button disabled. |
| 37 | Building (not captured) >> Stud and joist direction, framing reference | n | CB-38, CB-39 | Works | plan-framing wall.rs: header table, kings/trimmers/cripples/sills, double top plates, corner and tee backing, 48" blocking |
| 38 | Building (not captured) >> Build framing, trusses, framing overview | n | CB-36, C-12 | Partial, In progress (Round 14) | Build Framing / Build All Framing commands; Framing Defaults page (`dialogs/framing.rs` FramingDefaultsDialog, `framing_view::settings`) Ga… |
| 39 | Building (not captured) >> Auto rebuild options (walls, roofs, foundations) | n | RF-6, PR-13 | Partial, In progress (Round 14) | Cabinet options work as before. The auto-rebuild switches are kept (`PagePrefs.architectural`) and read by nothing yet: the roof and wall r… |

### Editing tools

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Editing (not captured) >> Layer Painter / Layer Eyedropper | n | NO SPEC (LAY-18) | Missing | see Tools menu row |
| 2 | Editing (not captured) >> Edit Area tools | n | S-90 | Missing, In progress (Round 14) | menus.rs:300 inert Gap: Not built. |
| 3 | Editing (not captured) >> Transform / Replicate Object | n | S-47, S-103, S-104 | Works | dialogs/transform.rs; transform.rs transform_replicate |
| 4 | Editing (not captured) >> Point to Point Move | n | S-52 | Works | transform.rs Mode::PointToPoint |
| 5 | Editing (not captured) >> Center Object | n | S-53 | Works | transform.rs center_opening_in_wall/center_between_walls/center_in_room |
| 6 | Editing (not captured) >> Align / Distribute | n | S-54 | Works | dialogs/transform.rs AlignDialog; plan-core transform.rs |
| 7 | Editing (not captured) >> Multiple Copy (linear and radial arrays) | n | S-104 | Works | transform.rs transform_replicate |
| 8 | Editing (not captured) >> Make Parallel / Make Perpendicular | n | S-49 | Works | editor/transform.rs make_parallel (walls and CAD lines); test edit_tests.rs make_parallel_and_perpendicular_turn_walls_about_their_start |
| 9 | Editing (not captured) >> Fillet / Chamfer | n | CAD-20, S-65, S-67 | Works | editor/behaviors.rs apply_vertex (Fillet and Chamfer behaviors on a polyline corner handle); tests fillet_rounds_a_dragged_corner, chamfer_… |
| 10 | Editing (not captured) >> Trim / Extend / Offset | n | NO SPEC (CAD-54) | Works | see CAD menu Edit CAD row |
| 11 | Editing (not captured) >> Boolean polyline operations | n | NO SPEC (CAD-55) | Missing | see CAD menu row |
| 12 | Editing (not captured) >> Marquee by area (window vs crossing) | n | S-29, S-30 | Partial, In progress (Round 14) | select.rs Gap: Body drag wins; Alt marquee not seen. |
| 13 | Editing (not captured) >> Reference Display of other floors | n | R-65, LAY-9 | Works | dialogs/reference_display.rs (reference_walls); render.rs draw_reference_floor; snap.rs snap_with_reference |
| 14 | Editing (not captured) >> Baseline, point-to-point, angular dimensions | n | DIM-15, DIM-17, DIM-18 | Works | DimMode::PointToPoint |
| 15 | Editing (not captured) >> Auto Story Pole and Auto NKBA dimensions | n | DIM-28 | Differs-by-design | dimension.rs plan-axis strings Gap: Plan strings, not elevation view objects. |
| 16 | Editing (not captured) >> Select Same Type / Select All of a type | n | S-32 | Partial | edit_commands.rs select_same_type Gap: Select Same Type selects every object of the same type at once (Edit menu, toolbar, context menu); t… |
| 17 | Editing (not captured) >> Group / Ungroup, Select Group Member | n | S-35, S-36 | Works | edit_commands.rs group_selection/ungroup_selection; select.rs pointer_down; plan-core groups.rs |
| 18 | Editing (not captured) >> Undo levels, Action History panel | n | S-78, S-79 | Works | history.rs cap 100 |
| 19 | Editing (not captured) >> Copy to other floor / Paste to Current Floor | n | S-85 | Works | menus.rs Paste Special "On Current Floor" |

### Program features

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Program (not captured) >> Project Browser (views, layouts, cameras, schedules) | n | NO SPEC (APP-35) | Works | shell/docks.rs; see View menu row |
| 2 | Program (not captured) >> Library Browser with catalogs, search, favorites, user library | n | CB-53, CB-54, CB-58, CB-61 | Works | shell/library_browser.rs |
| 3 | Program (not captured) >> Active Layer Display Options dock | n | LAY-3 | Works | dialogs/layer_display.rs (dock and modal); shell/docks.rs `edit_layers` |
| 4 | Program (not captured) >> Auto Archive and Backup Entire Plan | n | NO SPEC (APP-18) | Works | see File menu rows |
| 5 | Program (not captured) >> Templates and plan defaults | n | NO SPEC (APP-61) | Works | templates.rs, plan_defaults.rs |
| 6 | Program (not captured) >> Plan Check and building code presets | n | NO SPEC (APP-25) | Works | see Tools menu rows |
| 7 | Program (not captured) >> Ruby macros and scripting | n | NO SPEC (APP-31) | Missing | see Tools > Ruby Console row |
| 8 | Program (not captured) >> Time tracking | n | NO SPEC (APP-29) | Missing | see Tools > Time Tracker row |
| 9 | Program (not captured) >> Metric units and metric dimensions | n | DIM-6, W-19 | Works | plan-core units.rs; UnitSystem and LengthUnit (feet-inches, inches, decimal feet, mm, cm, m) |
| 10 | Program (not captured) >> Space Planning Assistant | n | NO SPEC (R-79) | Works | see Tools menu row |
| 11 | Program (not captured) >> Room Planner import | n | NO SPEC (R-87) | Missing | no Room Planner or Space Planning import beyond the Assistant |
| 12 | Program (not captured) >> Interactive tutorials and sample plans | n | NO SPEC (APP-62) | Missing | manual chapters only; sample plans are repo files |

### Snap Settings, Edit Behaviors and detail rows

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Snap Settings >> Object Snaps master switch and types list | cap | S-68, S-69 | Works | dialogs/snap_settings.rs, snap.rs SnapSettings, `editing` defaults |
| 2 | Snap Settings >> Endpoint snap | cap | S-68 | Works | dialogs/snap_settings.rs, snap.rs SnapSettings, `editing` defaults |
| 3 | Snap Settings >> Midpoint snap | cap | S-68 | Works | dialogs/snap_settings.rs, snap.rs SnapSettings, `editing` defaults |
| 4 | Snap Settings >> Intersection snap | cap | S-68 | Works | dialogs/snap_settings.rs, snap.rs SnapSettings, `editing` defaults |
| 5 | Snap Settings >> Perpendicular snap | cap | S-68 | Works | dialogs/snap_settings.rs, snap.rs SnapSettings, `editing` defaults |
| 6 | Snap Settings >> On Object (nearest) snap | cap | S-68 | Works | dialogs/snap_settings.rs, snap.rs SnapSettings, `editing` defaults |
| 7 | Snap Settings >> Center and Quadrant snaps on arcs and circles | cap | CAD-40 | Works | snap.rs cad_rounds/cad_segments/cad_intersections/on_extension/cad_marker_points |
| 8 | Snap Settings >> Tangent snap | cap | CAD-40 | Works | snap.rs cad_rounds/cad_segments/cad_intersections/on_extension/cad_marker_points |
| 9 | Snap Settings >> Extension snap | cap | S-68 | Works | dialogs/snap_settings.rs, snap.rs SnapSettings, `editing` defaults |
| 10 | Snap Settings >> Points / Markers snap | cap | CAD-2 | Works | CadMode::PlacePoint etc. |
| 11 | Snap Settings >> Angle Snaps (15 degree, parallel, perpendicular) | cap | S-71 | Works | snap.rs angle_snap; wall.rs |
| 12 | Snap Settings >> Grid Snaps (grid snap unit) | cap | S-72 | Works | defaults grid.snap |
| 13 | Snap Settings >> Bumping / Pushing | cap | S-73 | Partial | tools/cabinet.rs neighbor alignment Gap: Cabinets bump; no pushing. |
| 14 | Snap Settings >> Snap distance (sensitivity) (not captured) | n | S-70 | Works | EditorContext::snap_tol |
| 15 | Edit Behaviors >> Default behavior | cap | S-65 | Works | dialogs/edit_behaviors.rs; editor/behaviors.rs modes Default, Resize, Concentric, Fillet, Chamfer, Alternate, Replicate (hands the drag to… |
| 16 | Edit Behaviors >> Replicate | cap | S-65 | Works | dialogs/edit_behaviors.rs; editor/behaviors.rs modes Default, Resize, Concentric, Fillet, Chamfer, Alternate, Replicate (hands the drag to… |
| 17 | Edit Behaviors >> Resize | cap | S-65 | Works | dialogs/edit_behaviors.rs; editor/behaviors.rs modes Default, Resize, Concentric, Fillet, Chamfer, Alternate, Replicate (hands the drag to… |
| 18 | Edit Behaviors >> Concentric | cap | CAD-25 | Works | editor/behaviors.rs concentric (circle, arc, line, polyline body drag leaves offset copies); CadMode::Offset; tests concentric_leaves_the_o… |
| 19 | Edit Behaviors >> Fillet | cap | CAD-20 | Works | editor/behaviors.rs apply_vertex (Fillet and Chamfer behaviors on a polyline corner handle); tests fillet_rounds_a_dragged_corner, chamfer_… |
| 20 | Edit Behaviors >> Chamfer | cap | CAD-20 | Works | editor/behaviors.rs apply_vertex (Fillet and Chamfer behaviors on a polyline corner handle); tests fillet_rounds_a_dragged_corner, chamfer_… |
| 21 | Arc Creation Modes >> Three-point, Center-Start-End, Start-End-Radius, Tangent | cap | CAD-7 | Partial, In progress (Round 14) | CadMode::Arc option strip (three-point, center-start-end, tangent) Gap: Start-end-radius mode absent; Edit > Arc Creation Modes inert. |
| 22 | Edit Area >> Edit Area / Edit Area Visible | cap | S-90 | Missing, In progress (Round 14) | menus.rs:300 inert Gap: Not built. |
| 23 | Marquee Selection >> window / crossing / by-area modes | cap | S-29, S-30 | Partial, In progress (Round 14) | select.rs Gap: Body drag wins; Alt marquee not seen. |
| 24 | Select >> Select Objects hovering highlight and status text | cap | S-6 | Partial, In progress (Round 14) | select.rs update_hover; cx.hover Gap: Hover highlight yes; status line does not name the object. |
| 25 | Select >> Object handles (move, resize, rotate, reshape) | cap | S-12, S-13, S-14, S-15, S-16 | Partial | handles.rs ResizeStart/End Gap: Edit Behaviors > Resize scales a CAD selection from the opposite corner and Shift or the setting keeps prop… |
| 26 | Select >> Open Object (double-click, Enter) | cap | S-7, S-93 | Works | select.rs double_click; shell/spec_dialogs.rs |
| 27 | Select >> Select objects by layer, hidden/locked layer behavior (not captured) | n | S-5 | Works | editor/actions.rs check_unlocked, is_locked |
| 28 | Select >> Cursor shapes and selection color | cap | S-95, S-97 | Works | handles.rs CursorIcon |
| 29 | Edit toolbar >> Copy / Cut / Paste / Delete / Duplicate buttons | cap | S-38, S-81, S-87 | Works | actions.rs common_edit_actions |
| 30 | Edit toolbar >> Transform/Replicate Object | cap | S-47, S-103 | Works | dialogs/transform.rs; transform.rs transform_replicate |
| 31 | Edit toolbar >> Reflect About Object / Line | cap | S-48, S-105 | Works | transform.rs Mode::Reflect; plan-core transform.rs |
| 32 | Edit toolbar >> Rotate (about center, about point) | cap | S-101, S-102 | Works | transform.rs rotate_selection, group_rotate_handle; dialogs/transform.rs |
| 33 | Edit toolbar >> Make Parallel / Make Perpendicular | cap | S-49 | Works | editor/transform.rs make_parallel (walls and CAD lines); test edit_tests.rs make_parallel_and_perpendicular_turn_walls_about_their_start |
| 34 | Edit toolbar >> Break Line / Break Wall | cap | S-50 | Works | editor/wall_edit.rs BREAK_WALL button (break_wall_at); CadMode::BreakLine for CAD; s14 the_wall_edit_commands_are_toolbar_buttons_that_run |
| 35 | Edit toolbar >> Point to Point Move | cap | S-52 | Works | transform.rs Mode::PointToPoint |
| 36 | Edit toolbar >> Center Object | cap | S-53 | Works | transform.rs center_opening_in_wall/center_between_walls/center_in_room |
| 37 | Edit toolbar >> Align / Distribute | cap | S-54 | Works | dialogs/transform.rs AlignDialog; plan-core transform.rs |
| 38 | Edit toolbar >> Reverse Layers / Reverse Swing / Flip Hinge | cap | S-55, DW-32 | Works | editor/wall_edit.rs |
| 39 | Edit toolbar >> Send to Layer | cap | LAY-13 | Works | editor/edit_commands.rs ids::LAYER (Layer button on the Edit toolbar); dialogs/send_to_layer.rs; test edit_tests.rs the_edit_toolbar_offers… |
| 40 | Edit toolbar >> Select Same Type | cap | S-32 | Partial | edit_commands.rs select_same_type Gap: Select Same Type selects every object of the same type at once (Edit menu, toolbar, context menu); t… |
| 41 | Edit toolbar >> Convert to Polyline / Convert to Walls | cap | W-90, CAD-23 | Works | editor/wall_edit.rs convert_to_polyline |
| 42 | Edit toolbar >> Open Object / Object Information | cap | S-7 | Works | select.rs double_click; shell/spec_dialogs.rs |
| 43 | Edit toolbar >> Add to Library / Make CAD Block | cap | CB-58, CAD-31 | Partial, In progress (Round 14) | CadMode::MakeBlock Gap: Block is a named group in the plan, not in the Library Browser. |
| 44 | Edit toolbar >> Lock / Unlock (not captured) | n | NO SPEC (S-117) | Works | edit_commands.rs LOCK, UNLOCK |
| 45 | Application menu >> About, Preferences, Services, Hide, Quit (macOS application menu) | cap | NO SPEC (APP-63) | Partial | About, Preferences and Quit are rows of the Plan Studio menus (menus.rs); there is no native macOS application menu (eframe draws its own b… |
| 46 | Window >> Zoom tools: Zoom, Zoom In, Zoom Out, Undo Zoom, Fill Window, Pan (hotkeys) | cap | NO SPEC (LAY-35) | Works | toolbar.rs view_slots; Action::ZoomIn... |
| 47 | Project >> Plan information stored in the file (version, units, floors, layers, views) (not captured) | n | NO SPEC (APP-64) | Works | plan-core model.rs Project; docs/chief-plan-format.md |
| 48 | Project >> Reflected Ceiling Plan view (not captured) | n | RF-47 | Partial | ceiling planes and a Ceiling layer set exist; a saved Reflected Ceiling Plan view with ceiling features (lights, soffits) in plan is not a… |
| 49 | Project >> Electrical Service / panel schedule (not captured) | n | E-14 | Partial | circuits are listed in the Electrical schedule (E-13, E-14); there is no service/panel dialog |
| 50 | Project >> Kitchen and Bath design checks (NKBA clearances) (not captured) | n | NO SPEC (CB-89) | Partial | Plan Check covers IRC bath clearances (R307) and NKBA kitchen aisle width and counter depth (plan-check rules_fixtures.rs); work triangle,… |
| 51 | Project >> Structural Calculators (beam, header, joist spans) (not captured) | n | CB-42 | Missing | none Gap: Out of scope. |
| 52 | Project >> Engineered lumber, framing materials (I-joist, LVL, glulam) in framing (not captured) | n | CB-35 | Partial | framing members take a size/material name; engineered lumber stock lists are not modelled |
| 53 | Project >> Coffered and tray ceilings (not captured) | n | RF-45 | Partial | ceiling planes make tray ceilings; coffered beams need ceiling beam objects (framing) |
| 54 | Project >> Columns, posts and pilasters (decorative) (not captured) | n | CB-35 | Partial | Post and Post with Footing exist; decorative round and tapered columns come from the library only |
| 55 | Project >> Custom texture and image import for materials (not captured) | n | C-61 | Works | tools/materials.rs builder_window (color, roughness, metallic, transparency, pattern, texture file) saves to ~/.plan-studio/materials.json;… |
| 56 | Project >> Undo/redo across dialogs and one step per action (not captured) | n | S-76, S-80 | Works | begin_change |
| 57 | Project >> Autosave and crash recovery (not captured) | n | NO SPEC (APP-65) | Works | files.rs auto archive and recovery; dialogs/unsaved.rs |
| 58 | Project >> Plan units in metric (mm, cm, m) and unit conversions (not captured) | n | PR-16 | Works | `ui::convert`; `unit_conversions_show_every_unit`. Chief's page is not captured; this is a converter with a starting unit and rounding (ver… |
| 59 | Project >> Multiple documents and tabs (several plans open) (not captured) | n | NO SPEC (APP-66) | Missing | one plan per window; open replaces the plan after the unsaved prompt (files.rs open_dialog); views are tabs (editor/plan_tabs.rs) |
| 60 | Edit >> Undo restores selection after Undo/Redo | cap | S-77 | Partial | selection.retain_existing Gap: Selection kept only if object still exists. |
| 61 | Edit Behaviors >> Edit behavior indicator and reset (status bar) | cap | S-66 | Missing, In progress (Round 14) | none Gap: Depends on S-65. |
| 62 | Edit >> Paste across files keeps layers (clipboard between plans) | cap | S-85 | Partial, In progress (Round 14) | clipboard in memory only Gap: No cross-file clipboard. |

### Layers, line weights and project settings (not captured)

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Layers (not captured) >> Layer table: name, color, line weight, display, lock, ByLayer | n | LAY-1 | Works | plan-core layers.rs |
| 2 | Layers (not captured) >> Layer Sets and the Default layer set names | n | LAY-2, LAY-8, LAY-11 | Works | plan-core layer_sets.rs; Project.plan_views |
| 3 | Layers (not captured) >> Drawing Groups (line-weight groups, object type to group) | n | NO SPEC (LAY-36) | Partial | the Wall dialog's Layer tab shows a Drawing Group; there is no drawing group table that maps each object kind to a pen group (dialogs/wall.… |
| 4 | Layers (not captured) >> Line weight table by drawing group (Preferences > CAD line weights) | n | PR-14, L-12 | Partial, In progress (Round 14) | `ui::cad`, `pages::CadPrefs` (Daniel's X18 line weights at first); the CAD painter does not read them yet (queued) |
| 5 | Layers (not captured) >> Object display: hide a layer in a view, lock layer, layer filters | n | LAY-3, LAY-4, LAY-5 | Works | dialogs/layer_display.rs (dock and modal); shell/docks.rs `edit_layers` |
| 6 | Layers (not captured) >> Plan view for ceiling, electrical, framing, HVAC from the template layer sets | n | LAY-2, LAY-11 | Works | plan-core layer_sets.rs; Project.plan_views |
| 7 | Layers (not captured) >> Current CAD layer and Active Layers for tools | n | LAY-6 | Works | dialogs/layer_sets.rs (Active Layers by Tool); plan-core layers.rs tool_layer; tools/cad.rs draw_layer; toolbar.rs:2136 Current CAD Layer b… |
| 8 | Layers (not captured) >> Project Information: client, address, designer, revision, custom fields | n | L-9 | Works | plan-core schedules.rs ProjectInfo::macro_pairs and expand (%client.phone%, %client.email%, %company%, %checked.by%, %custom.key%); shell/l… |
| 9 | Layers (not captured) >> Plan elevation datum and north angle (site orientation) | n | NO SPEC (LAY-37) | Partial | north angle lives in the North Pointer object and Terrain Specification (dialogs/terrain.rs North angle); there is no plan-wide datum or la… |
| 10 | Layers (not captured) >> Display units and drawing scale of the active view (1/4" = 1'-0") | n | L-22 | Works | architectural and metric scale lists; custom ratio 1:n in the Print dialog and `Scale::Ratio` |
| 11 | Layers (not captured) >> Plan scale lists and rounding of dimensions | n | L-22, DIM-6 | Works | architectural and metric scale lists; custom ratio 1:n in the Print dialog and `Scale::Ratio` |

### Layout window commands (Chief's Layout menus, not in the captures)

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Layout menu >> Send to Layout… (not captured) | n | L-1, L-2 | Works | shell/layout_window.rs; dialogs/layout.rs; hotkey S,L; Project Browser camera list (right-click > Send to Layout, `LayoutCommand::SendCamer… |
| 2 | Layout menu >> Send All Floors to Layout (not captured) | n | L-1, L-2 | Works | shell/layout_window.rs; dialogs/layout.rs; hotkey S,L; Project Browser camera list (right-click > Send to Layout, `LayoutCommand::SendCamer… |
| 3 | Layout menu >> Layout Box Specification… (not captured) | n | L-4 | Partial | dialogs/layout.rs General/Source/Line Style; quarter-turn Rotation, rotate knob on the selected box, hit test and outline use the turned co… |
| 4 | Layout menu >> Delete Layout Box (not captured) | n | L-6 | Works | `layout_window.rs`: 8 handles, nudge; Shift-click multi-select and group drag; `align_selected`, `distribute_selected`, `copy_selected_to`,… |
| 5 | Layout menu >> Duplicate Layout Box (not captured) | n | L-6 | Works | `layout_window.rs`: 8 handles, nudge; Shift-click multi-select and group drag; `align_selected`, `distribute_selected`, `copy_selected_to`,… |
| 6 | Layout menu >> Open Source View (jump from a box to its plan or camera) (not captured) | n | L-17 | Works | layout stored in plan |
| 7 | Layout menu >> Copy Layout Box to Page… (not captured) | n | L-6 | Works | `layout_window.rs`: 8 handles, nudge; Shift-click multi-select and group drag; `align_selected`, `distribute_selected`, `copy_selected_to`,… |
| 8 | Layout menu >> Align Layout Boxes (not captured) | n | L-6 | Works | `layout_window.rs`: 8 handles, nudge; Shift-click multi-select and group drag; `align_selected`, `distribute_selected`, `copy_selected_to`,… |
| 9 | Layout menu >> Spread Horizontally (not captured) | n | L-6 | Works | `layout_window.rs`: 8 handles, nudge; Shift-click multi-select and group drag; `align_selected`, `distribute_selected`, `copy_selected_to`,… |
| 10 | Layout menu >> Spread Vertically (not captured) | n | L-6 | Works | `layout_window.rs`: 8 handles, nudge; Shift-click multi-select and group drag; `align_selected`, `distribute_selected`, `copy_selected_to`,… |
| 11 | Layout menu >> Update Layout Views (refresh linked boxes) (not captured) | n | L-3 | Works | layout_window.rs live redraw; Update Layout Views |
| 12 | Layout menu >> Insert Page Before (not captured) | n | L-7 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 13 | Layout menu >> Insert Page After (not captured) | n | L-7 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 14 | Layout menu >> Duplicate Page (not captured) | n | L-7 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 15 | Layout menu >> Delete Page (not captured) | n | L-7 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 16 | Layout menu >> Exchange With Previous Page (not captured) | n | L-7 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 17 | Layout menu >> Exchange With Next Page (not captured) | n | L-7 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 18 | Layout menu >> Previous Page (not captured) | n | L-7 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 19 | Layout menu >> Next Page (not captured) | n | L-7 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 20 | Layout menu >> Layout Page Table… (not captured) | n | L-7, L-11 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 21 | Layout menu >> Page Specification… (not captured) | n | L-7 | Works | `PageSpecDialog` (title, sheet number, Page Template flag, own sheet size and orientation, no title block), `LayoutView::apply_page_spec`,… |
| 22 | Layout menu >> Page Setup… (not captured) | n | L-8 | Works | Page Setup lists Arch, ANSI, ISO with landscape/portrait; `SheetSizesDialog` (Customize Sheet Sizes: custom named sizes, hide standard size… |
| 23 | Layout menu >> Customize Sheet Sizes… (not captured) | n | L-8 | Works | Page Setup lists Arch, ANSI, ISO with landscape/portrait; `SheetSizesDialog` (Customize Sheet Sizes: custom named sizes, hide standard size… |
| 24 | Layout menu >> Project Information… (title block fields) (not captured) | n | L-9 | Works | plan-core schedules.rs ProjectInfo::macro_pairs and expand (%client.phone%, %client.email%, %company%, %checked.by%, %custom.key%); shell/l… |
| 25 | Layout menu >> Fit Page in Window (not captured) | n | L-19 | Works | menus.rs layout_menu "Fit Page in Window"; layout_window.rs |
| 26 | Layout menu >> Layer Display Options… (layout layers) (not captured) | n | LAY-3 | Works | dialogs/layer_display.rs (dock and modal); shell/docks.rs `edit_layers` |
| 27 | Layout menu >> Add Sheet Index (not captured) | n | L-11 | Works | Sheet index table box (`BoxSource::SheetIndex`) that follows page titles, on the cover of the construction set and from the layout toolbar;… |
| 28 | Layout menu >> Save As Template… (not captured) | n | L-10 | Works | Page Template page and flag (Page Specification); Daniel 18x24 block; Save As Template / Apply Template; New Layout File starts from the de… |
| 29 | Layout menu >> Apply Template… (not captured) | n | L-10 | Works | Page Template page and flag (Page Specification); Daniel 18x24 block; Save As Template / Apply Template; New Layout File starts from the de… |
| 30 | Layout menu >> New Layout File… (not captured) | n | L-10 | Works | Page Template page and flag (Page Specification); Daniel 18x24 block; Save As Template / Apply Template; New Layout File starts from the de… |
| 31 | Layout menu >> Export Table as CSV… (not captured) | n | L-32, L-37 | Works | CSV and Excel (.xlsx, `plan_docs::xlsx`) in the schedule windows, the Schedule Specification, the Materials List, the Framing Takeoff and t… |
| 32 | Layout menu >> Export Table to Excel… (not captured) | n | L-32, L-37 | Works | CSV and Excel (.xlsx, `plan_docs::xlsx`) in the schedule windows, the Schedule Specification, the Materials List, the Framing Takeoff and t… |
| 33 | Layout menu >> Print Model… (not captured) | n | L-18, L-19, L-20 | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes Gap: The two standard fonts are ref… |
| 34 | Layout menu >> Print Layout… (not captured) | n | L-18, L-19, L-20 | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes Gap: The two standard fonts are ref… |
| 35 | Layout menu >> Export Layout PDF… (not captured) | n | L-18, L-19, L-20 | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes Gap: The two standard fonts are ref… |
| 36 | Layout menu >> Layout (JSON) import and export (not captured) | n | L-17 | Works | menus.rs File > Import/Export Layout (JSON); FileCommand::ImportLayout, ExportLayout |
| 37 | Layout menu >> Export Construction Set PDF… (not captured) | n | L-20 | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes Gap: The two standard fonts are ref… |
| 38 | Layout menu >> Drawing Sheet Setup… (not captured) | n | L-19 | Works | Print Preview window (`PrintPreviewDialog`, `plan_layout::layout_print_preview` / `plan_view_print_preview`): the pages as the Print dialog… |
| 39 | 3D menu details >> Front Elevation view (not captured) | n | C-15, C-16 | Partial, In progress (Round 14) | Orthographic Full Overview + 4 elevations Gap: No floor/framing ortho overview, no isometric SW/SE/NE/NW. |
| 40 | 3D menu details >> Back Elevation view (not captured) | n | C-15, C-16 | Partial, In progress (Round 14) | Orthographic Full Overview + 4 elevations Gap: No floor/framing ortho overview, no isometric SW/SE/NE/NW. |
| 41 | 3D menu details >> Left Elevation view (not captured) | n | C-15, C-16 | Partial, In progress (Round 14) | Orthographic Full Overview + 4 elevations Gap: No floor/framing ortho overview, no isometric SW/SE/NE/NW. |
| 42 | 3D menu details >> Right Elevation view (not captured) | n | C-15, C-16 | Partial, In progress (Round 14) | Orthographic Full Overview + 4 elevations Gap: No floor/framing ortho overview, no isometric SW/SE/NE/NW. |
| 43 | 3D menu details >> Plan Overhead view (not captured) | n | C-15, C-16 | Partial, In progress (Round 14) | Orthographic Full Overview + 4 elevations Gap: No floor/framing ortho overview, no isometric SW/SE/NE/NW. |
| 44 | 3D menu details >> Export Elevations (DXF)… (not captured) | n | L-44 | Partial | plan-core export/dxf.rs R12 ASCII; elevation DXF Gap: No version, units, selection options; no DWG. |
| 45 | 3D menu details >> Auto Back-Clipped and Auto Interior Elevations (not captured) | n | C-21 | Works | manual 10.11; tools/camera.rs |
| 46 | 3D menu details >> Walkthrough Path from CAD Polyline (not captured) | n | C-71 | Works | tools/camera.rs; Play/Record |
| 47 | 3D menu details >> Play Walkthrough (not captured) | n | C-71 | Works | tools/camera.rs; Play/Record |
| 48 | 3D menu details >> Save Camera (not captured) | n | C-29, C-3 | Partial, In progress (Round 14) | shell/view3d_panel.rs Gap: Single 3D panel; per-view options limited to technique and elevation settings. |
| 49 | 3D menu details >> Restore Camera (not captured) | n | C-29, C-3 | Partial, In progress (Round 14) | shell/view3d_panel.rs Gap: Single 3D panel; per-view options limited to technique and elevation settings. |
| 50 | 3D menu details >> Delete Camera (not captured) | n | C-29, C-3 | Partial, In progress (Round 14) | shell/view3d_panel.rs Gap: Single 3D panel; per-view options limited to technique and elevation settings. |
| 51 | 3D menu details >> Move camera forward (not captured) | n | C-38 | Partial, In progress (Round 14) | WASD/arrows/PgUp/PgDn in Full Camera; 3D > Move Camera with Keyboard steps (`nudge.rs`) Gap: The menu steps move or turn by fixed amounts (… |
| 52 | 3D menu details >> Move camera back (not captured) | n | C-38 | Partial, In progress (Round 14) | WASD/arrows/PgUp/PgDn in Full Camera; 3D > Move Camera with Keyboard steps (`nudge.rs`) Gap: The menu steps move or turn by fixed amounts (… |
| 53 | 3D menu details >> Turn camera left (not captured) | n | C-38 | Partial, In progress (Round 14) | WASD/arrows/PgUp/PgDn in Full Camera; 3D > Move Camera with Keyboard steps (`nudge.rs`) Gap: The menu steps move or turn by fixed amounts (… |
| 54 | 3D menu details >> Turn camera right (not captured) | n | C-38 | Partial, In progress (Round 14) | WASD/arrows/PgUp/PgDn in Full Camera; 3D > Move Camera with Keyboard steps (`nudge.rs`) Gap: The menu steps move or turn by fixed amounts (… |
| 55 | 3D menu details >> Raise camera (not captured) | n | C-38 | Partial, In progress (Round 14) | WASD/arrows/PgUp/PgDn in Full Camera; 3D > Move Camera with Keyboard steps (`nudge.rs`) Gap: The menu steps move or turn by fixed amounts (… |
| 56 | 3D menu details >> Lower camera (not captured) | n | C-38 | Partial, In progress (Round 14) | WASD/arrows/PgUp/PgDn in Full Camera; 3D > Move Camera with Keyboard steps (`nudge.rs`) Gap: The menu steps move or turn by fixed amounts (… |
| 57 | 3D menu details >> Tilt camera up (not captured) | n | C-40 | Partial, In progress (Round 14) | drag pitch; spec dialog; 3D > Tilt Camera (5 degree steps, `nudge.rs`) Gap: Steps only, no tilt drag tool or typed value in the menu. |
| 58 | 3D menu details >> Tilt camera down (not captured) | n | C-40 | Partial, In progress (Round 14) | drag pitch; spec dialog; 3D > Tilt Camera (5 degree steps, `nudge.rs`) Gap: Steps only, no tilt drag tool or typed value in the menu. |
| 59 | Edit menu details >> Paste as Group (not captured) | n | S-83, S-85 | Partial, In progress (Round 14) | clipboard in memory only Gap: No cross-file clipboard. |
| 60 | Edit menu details >> Paste Special > As Group / On Current Floor (not captured) | n | S-83, S-85 | Partial, In progress (Round 14) | clipboard in memory only Gap: No cross-file clipboard. |
| 61 | Edit menu details >> Rotate Selection (not captured) | n | S-101, S-105 | Works | transform.rs rotate_selection, group_rotate_handle; dialogs/transform.rs |
| 62 | Edit menu details >> Reflect Copy About Object (not captured) | n | S-101, S-105 | Works | transform.rs rotate_selection, group_rotate_handle; dialogs/transform.rs |
| 63 | Edit menu details >> Distribute Horizontally (not captured) | n | S-54 | Works | dialogs/transform.rs AlignDialog; plan-core transform.rs |
| 64 | Edit menu details >> Distribute Vertically (not captured) | n | S-54 | Works | dialogs/transform.rs AlignDialog; plan-core transform.rs |
| 65 | Edit menu details >> Cabinet Defaults… (not captured) | n | CB-20 | Works | plan-core CabinetDefaults; dialogs/cabinet.rs CabinetDefaultsDialog; reached from Default Settings > Cabinets (dialogs/defaults.rs Leaf::Ca… |
| 66 | Tools menu details >> Underlays… (manage picture and PDF underlays) (not captured) | n | L-46 | Partial, In progress (Round 14) | `plan_core::underlay`, `tools/underlay.rs` (+ `pdf.rs`, `inflate.rs`, `trace.rs`), `dialogs/underlay.rs`: PNG and JPEG (baseline and progre… |
| 67 | Tools menu details >> Save Plan View (not captured) | n | LAY-2 | Works | plan-core layer_sets.rs; Project.plan_views |
| 68 | Tools menu details >> Reset Plan View (not captured) | n | LAY-2 | Works | plan-core layer_sets.rs; Project.plan_views |
| 69 | Tools menu details >> Add Template Plan Views (not captured) | n | LAY-2 | Works | plan-core layer_sets.rs; Project.plan_views |
| 70 | Tools menu details >> Door Schedule (not captured) | n | L-23, L-24 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 71 | Tools menu details >> Window Schedule (not captured) | n | L-23, L-24 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 72 | Tools menu details >> Room Schedule (not captured) | n | L-23, L-24 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 73 | Tools menu details >> Wall Schedule (menu rows) (not captured) | n | L-23, L-24 | Works | plan-core schedules.rs ScheduleKind::ALL (14 kinds: Door, Window, Room, Wall, Cabinet, Electrical, Framing, Fixture, Furniture, Plant, Stai… |
| 74 | Tools menu details >> Renumber Door Schedule (not captured) | n | DW-61 | Works | opening_edit.rs `renumber`, `renumber_marks` (Edit toolbar "Renumber Schedule", Schedules > Renumber Door / Window Schedule; marks written… |
| 75 | Tools menu details >> Renumber Window Schedule (not captured) | n | DW-61 | Works | opening_edit.rs `renumber`, `renumber_marks` (Edit toolbar "Renumber Schedule", Schedules > Renumber Door / Window Schedule; marks written… |
| 76 | Tools menu details >> Detail Components… (CAD detail component chooser) (not captured) | n | CAD-35 | Works | `tools/details/cad_detail.rs` (`auto_detail`, `detail_from_view`, management ops), `dialogs/details/management.rs`, `plan_core::details::Ca… |
| 77 | File menu details >> Export ▸ Construction Set PDF… (not captured) | n | L-20 | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes Gap: The two standard fonts are ref… |
| 78 | File menu details >> Print ▸ Drawing Sheet Setup… (not captured) | n | L-19 | Works | Print Preview window (`PrintPreviewDialog`, `plan_layout::layout_print_preview` / `plan_view_print_preview`): the pages as the Print dialog… |
| 79 | File menu details >> Import ▸ Underlay Picture (PNG, JPEG, PDF)… (not captured) | n | L-46 | Partial, In progress (Round 14) | `plan_core::underlay`, `tools/underlay.rs` (+ `pdf.rs`, `inflate.rs`, `trace.rs`), `dialogs/underlay.rs`: PNG and JPEG (baseline and progre… |
| 80 | Library menu details >> Add Selection to Library (not captured) | n | CB-58 | Works | tools/library/user.rs, make.rs; ~/.plan-studio/user-library.json |
| 81 | Library menu details >> Export Library (Plan Studio JSON)… (not captured) | n | CB-58 | Works | tools/library/user.rs export_library_to (user items only, plan-studio-library.json; a .calib name is refused) |

### Additional product features (not captured)

| # | Chief feature | Src | Parity | Status | Evidence |
|---|---|---|---|---|---|
| 1 | Foundation (not captured) >> Stepped footings (footing steps down a sloping lot) | n | NO SPEC (R-88) | Missing | no footing steps in plan-core rooms.rs / dialogs/foundation.rs (grep stepped footing finds nothing) |
| 2 | Foundation (not captured) >> Grade beams and monolithic slab thickened edges | n | R-62 | Partial, In progress (Round 14) | tools/foundation.rs slabs/piers Gap: Not tied to Build Foundation. |
| 3 | Exchange (not captured) >> Export Materials List to HTML | n | NO SPEC (L-56) | Missing | Materials List exports CSV and XLSX (plan-docs materials.rs, xlsx.rs); no HTML table (grep html in plan-docs finds nothing) |

## Spec gaps (NO SPEC items)

Every row below had no parity id before this audit. Each now has a row in the parity file named in the heading, appended at the end of that file under "Coverage audit additions" and repeated in `docs/parity-status.md` under "Coverage audit". Status is from a code search. Items from product knowledge need a Chief session to confirm their behavior.

### Walls (`docs/parity/walls.md`, W- ids)

- **W-106** Build Walls >> Straight Deck Edge: Deck edge wall that defines the deck outline, floor platform and fascia. Status: Works. Also listed as: Build Walls >> Curved Deck Edge.
- **W-107** Build Walls >> Polygon Shaped Deck…: Deck from a polygon with a deck-edge wall and a floor platform. Status: Works.
- **W-108** Build Walls >> Straight Fencing: Fence wall type with posts, rails and pickets. Status: Works. Also listed as: Build Walls >> Curved Fencing.
- **W-109** Build Trim >> Corner Boards: Exterior corner trim placed by hand or around every corner. Status: Works. Also listed as: Build Trim >> Auto Place Corner Boards.
- **W-110** Build Trim >> Quoins: Stone or brick corner blocks. Status: Works. Also listed as: Build Trim >> Auto Place Quoins.
- **W-111** Build Trim >> Molding Line: Crown, base, chair rail or exterior trim drawn along a line or polyline from a profile. Status: Works. Also listed as: Build Trim >> Molding Polyline.
- **W-112** Toolbar 2 >> Trim flyout: Trim tools flyout. Status: Works.
- **W-113** Default Settings >> Corner Trim defaults: Corner board and quoin size and material defaults. Status: Missing.
- **W-114** Wall dialog >> Wall Cap tab (profile table, position): Cap profile table, vertical and horizontal position, rotation, reflect, materials-list count. Status: Partial.
- **W-115** Wall dialog >> Wall Covering tab (coverings, position, options): Interior/exterior wall coverings (tile wainscot, paneling) with top/bottom offsets, per wall side. Status: Missing.
- **W-116** Wall dialog >> Newels/Balusters tab (railing wall): Newel and baluster style, spacing and count for railing walls. Status: Missing.
- **W-117** Wall dialog >> Rails tab (railing wall): Top rail, bottom rail and handrail profiles for railing walls. Status: Missing.
- **W-118** Wall dialog >> Object Information tab (code, comment, manufacturer, supplier): Descriptive fields and custom fields carried to schedules. Status: Missing.

### Select and edit (`docs/parity/select-and-edit.md`, S- ids)

- **S-113** Edit >> Move to Front (not captured): Change the draw order of CAD and plan objects. Status: Works. Also listed as: Edit >> Move to Back (draw order).
- **S-114** Edit >> Lock (not captured): Lock objects so they cannot be moved or edited. Status: Works. Also listed as: Edit >> Unlock selection.
- **S-115** Tools >> Reverse Plan: Mirror the whole plan left to right (a flipped floor plan). Status: Missing.
- **S-116** Tools >> Object Painter ▸: Paint one object's properties (style, size, layer) onto other objects of the same type. Status: Missing.
- **S-117** Edit toolbar >> Lock / Unlock (not captured): Lock selected objects against editing. Status: Works.

### Doors and windows (`docs/parity/doors-windows.md`, DW- ids)

- **DW-111** Door dialog >> Lintel tab: Lintel profile above the opening with extend and wrap. Status: Works.
- **DW-112** Door dialog >> Lites tab: Divided lites in a door panel. Status: Works.
- **DW-113** Door dialog >> Arch tab: Arched head options for the door. Status: Works.
- **DW-114** Door dialog >> Framing tab (header, trimmers, king studs, sills): Header construction, trimmer and king stud counts per opening. Status: Missing.
- **DW-115** Door dialog >> Energy Values tab (U-factor, SHGC): Door type, U-factor and solar heat gain for energy reports. Status: Missing.
- **DW-116** Door dialog >> Layer tab: Layer and drawing group of the door. Status: Missing.
- **DW-117** Door dialog >> Materials tab: Material per door component (panel, frame, casing, glass). Status: Missing.
- **DW-118** Door dialog >> Object Information tab: Code, comment, manufacturer, supplier and custom fields. Status: Missing.
- **DW-119** Window dialog >> Sash tab: Sash widths, depth, inset, curved options. Status: Works.
- **DW-120** Window dialog >> Lites tab: Window lites, muntin width, round-top arch rays. Status: Works.
- **DW-121** Window dialog >> Shape tab (heights, corners): Custom window shapes: raked sides, angled top corners and bottom corners. Status: Missing.
- **DW-122** Window dialog >> Arch tab: Arched head options. Status: Works.
- **DW-123** Window dialog >> Treatments tab (curtains, blinds, exterior millwork): Curtains, blinds and millwork above and below the casing with library styles. Status: Missing. Also listed as: Building (not captured) >> Window treatments (curtains, blinds).

### Rooms and floors (`docs/parity/rooms-floors.md`, R- ids)

- **R-72** Build Floors >> Floor Material Region: Polygon on the floor with its own material and fill. Status: Works.
- **R-73** Build Floors >> Hole in Floor Platform: Cut a hole in a floor platform (stairwell, open to below). Status: Works.
- **R-74** Build Floors >> Hole in Ceiling Platform: Cut a hole in a ceiling platform (attic stair, chase). Status: Works.
- **R-75** Build Floors >> Slab: Slab polyline with thickness, fill and a footing option (no parity spec: manual chapter 16 only). Status: Works.
- **R-76** Build Floors >> Slab with Footing: Slab with a perimeter footing. Status: Works.
- **R-77** Build Floors >> Slab Hole: Hole cut in a slab, optionally with its own footing. Status: Works. Also listed as: Build Floors >> Slab Hole with Footing.
- **R-78** Build Floors >> Square Pad: Footing pad and pier placed under posts and beams. Status: Works. Also listed as: Build Floors >> Round Pier.
- **R-79** Tools >> Space Planning ▸ Space Planning Assistant…: Questionnaire to colored room boxes, arrange, then build the house plan. Status: Works. Also listed as: Program (not captured) >> Space Planning Assistant.
- **R-80** Tools >> Space Planning ▸ Room Planner / Space Planning Configuration toolbar (not captured) (not captured): Space Planning toolbar configuration; Room Planner to import a room-planner sketch. Status: Partial.
- **R-81** Toolbar 2 >> Slab flyout: Slab and pier tools flyout. Status: Works.
- **R-82** Default Settings >> Foundation defaults: Footing size, slab thickness, stem wall height defaults used by Build Foundation. Status: Missing.
- **R-83** Default Settings >> Slab defaults: Slab thickness, footing and fill defaults. Status: Missing.
- **R-84** Room dialog >> Wall Covering tab (room wall coverings) (not captured): Wall coverings applied to every wall of a room (wainscot, tile). Status: Partial.
- **R-85** Other dialogs >> Slab / Footing / Pad / Pier specification (not captured): Slab specification (General, Fill Style, Line Style, Layer). Status: Works.
- **R-86** Building (not captured) >> Split-level floors (floors at different heights on one level) (not captured): Two or more floor platforms on one story at different elevations joined by stairs. Status: Missing.
- **R-87** Program (not captured) >> Room Planner import (not captured): Import a room-planner layout and build walls from its room outlines. Status: Missing.
- **R-88** Foundation (not captured) >> Stepped footings (footing steps down a sloping lot) (not captured): Footings that step in height along a foundation wall on a sloping lot, with the step shown in plan and 3D. Status: Missing.

### Roofs (`docs/parity/roofs.md`, RF- ids)

- **RF-61** Building (not captured) >> Curved roofs / curved roof planes (not captured): Barrel and bell-curve roof planes on curved baselines. Status: Missing.

### Dimensions (`docs/parity/dimensions-text-cad.md`, DIM- ids)

- **DIM-46** Dimension dialog >> Secondary Format tab (second unit in parentheses): Show a second measurement system next to the primary dimension text. Status: Missing.

### Text (`docs/parity/dimensions-text-cad.md`, TXT- ids)

- **TXT-20** CAD >> Text ▸ Text Macro Management: User-defined %macro% text such as %project_name% inserted into labels and notes. Status: Works.
- **TXT-21** Documents (not captured) >> Spell check in text (not captured): Spell-check text boxes, notes and labels before printing. Status: Missing.

### CAD, images and details (`docs/parity/dimensions-text-cad.md`, CAD- ids)

- **CAD-46** File >> Import ▸ Picture (PNG, JPEG): Import Picture File into a picture box with Image Specification. Status: Works.
- **CAD-47** Build Images >> Create Image (picture box): Picture on the plan, in layout and in 3D. Status: Works.
- **CAD-48** Build Images >> Create Billboard Image: Image that always faces the 3D camera (people, trees). Status: Works.
- **CAD-49** Build Images >> Create Image Library item: Store a picture or symbol as a reusable image-library entry (Create Image Library). Status: Partial.
- **CAD-50** Build Images >> Point to Point Resize (not captured): Scale and rotate a picture or underlay by two clicks. Status: Works. Also listed as: Build Images >> Rotate to Align (picture tracing).
- **CAD-51** Build Distributed >> Polyline Distribution Path: Repeat a symbol along a path (fence posts, trees, balusters). Status: Works. Also listed as: Build Distributed >> Spline Distribution Path.
- **CAD-52** Build Distributed >> Polyline Distribution Region: Fill a region with scattered symbols (shrubs, gravel). Status: Works. Also listed as: Build Distributed >> Spline Distribution Region.
- **CAD-53** CAD >> Patterns ▸ Hatch Closed Shape (CAD hatch patterns): Fill a closed CAD shape with a hatch pattern from the pattern list. Status: Works.
- **CAD-54** CAD >> Edit CAD ▸ (Offset, Trim, Extend, Break Line, Fillet, Chamfer, Make Parallel/Perpendicular, Reverse Direction) (not captured): CAD edit tools that change a line or polyline: offset copy, trim to a cutter, extend to the next object, break at a point, fillet or chamfer a corner. Status: Works. Also listed as: Editing (not captured) >> Trim / Extend / Offset.
- **CAD-55** CAD >> Boolean polyline operations (union, subtract, intersect) (not captured): Combine closed polylines into one outline by union, subtraction or intersection (Chief: Edit toolbar Subtract/Union/Intersect polylines). Status: Missing. Also listed as: Editing (not captured) >> Boolean polyline operations.
- **CAD-56** Tools >> Fill Style Painter ▸: Pick up a fill style and paint it onto polylines, rooms, slabs. Status: Missing.
- **CAD-57** Default Settings >> CAD defaults (line style, fill, arrows, text, layer): Default line style, weight, fill, arrow and layer for new CAD lines, boxes, polylines and arcs. Status: Missing.
- **CAD-58** Default Settings >> Distributed Objects defaults: Default spacing and symbol for distribution paths and regions. Status: Missing.
- **CAD-59** Default Settings >> Image defaults: Default size, transparency and layer for pictures and billboards. Status: Missing.
- **CAD-60** Other dialogs >> Image / Distributed Object specification (not captured): Picture box and distribution path/region dialogs. Status: Works.
- **CAD-61** Exchange (not captured) >> Import pictures (PNG, JPEG, TIFF, BMP) (not captured): Insert scanned plans and photos as picture boxes. Status: Partial.

### Layers, display and view commands (`docs/parity/dimensions-text-cad.md`, LAY- ids)

- **LAY-16** Tools >> Layer Settings ▸ Active Layers by Tool…: Which layer each tool draws on (Chief: Active Layers for Tools). Status: Works.
- **LAY-17** Tools >> Active View ▸ Rotate Plan View…: Rotate the plan on screen and in layout to put a skewed lot square to the page. Status: Missing.
- **LAY-18** Tools >> Layer Painter / Layer Eyedropper (not captured) (not captured): Click an object to copy its layer, click others to put them on it. Status: Missing. Also listed as: Editing (not captured) >> Layer Painter / Layer Eyedropper.
- **LAY-19** View >> Color (F8): Toggle color and black-and-white plan display. Status: Works. Also listed as: Right bar >> Color toggle.
- **LAY-20** View >> Crosshairs: Full-window crosshair cursor for alignment. Status: Works. Also listed as: Right bar >> Crosshairs toggle.
- **LAY-21** View >> Coordinate System Indicator (Floating / Fixed / Origin): Show the X/Y axis and origin symbol on the plan. Status: Missing.
- **LAY-22** View >> Reference Grid (Shift+F9): Display the drawing grid. Status: Works.
- **LAY-23** View >> Angle Snap Grid: Show the angle-snap rays from the last point while drawing. Status: Missing.
- **LAY-24** Window >> Zoom (rubber-band, Shift+Z): Drag a rectangle to zoom to it. Status: Partial.
- **LAY-25** Window >> Zoom Out (-): Step zoom. Status: Works. Also listed as: Window >> Zoom In (+).
- **LAY-26** Window >> Undo Zoom: Return to the previous zoom. Status: Works.
- **LAY-27** Window >> Fill Window Building Only: Fit the building (walls and rooms) in the window, ignoring far-away CAD. Status: Missing. Also listed as: Right bar >> Fill Window Building Only.
- **LAY-28** Window >> Fill Window (Ctrl+F): Fit all objects in the window. Status: Works.
- **LAY-29** Window >> Pan Window (H): Hand-pan the plan. Status: Works.
- **LAY-30** Right bar >> Zoom toggle: Rubber-band zoom tool. Status: Partial.
- **LAY-31** Right bar >> Zoom In: Zoom steps. Status: Works. Also listed as: Right bar >> Undo Zoom; Right bar >> Zoom Out.
- **LAY-32** Right bar >> Fill Window: Fit everything. Status: Works.
- **LAY-33** Right bar >> Pan Window: Pan tool. Status: Works.
- **LAY-34** Documents (not captured) >> Annotation Sets (named text/dimension display groups) (not captured): Named sets that show or hide annotation (dimension, text) per view; verify in Chief. Status: Missing.
- **LAY-35** Window >> Zoom tools: Zoom, Zoom In, Zoom Out, Undo Zoom, Fill Window, Pan (hotkeys): Zoom and pan commands. Status: Works.
- **LAY-36** Layers (not captured) >> Drawing Groups (line-weight groups, object type to group) (not captured): Drawing groups assign each object type (wall, door, dimension) a pen/line weight group that Preferences and printing set. Status: Partial.
- **LAY-37** Layers (not captured) >> Plan elevation datum and north angle (site orientation) (not captured): Set the plan's north direction, site latitude/longitude and elevation datum once for sun angles and terrain. Status: Partial.

### 3D views, cameras and rendering (`docs/parity/3d-views-cameras.md`, C- ids)

- **C-72** Build Solids >> 3D Solid: Primitive 3D solids and faces with a material, edited by dialog. Status: Works. Also listed as: Build Solids >> Cone; Build Solids >> Cylinder; Build Solids >> Face; Build Solids >> Pyramid; Build Solids >> Sphere.
- **C-73** 3D >> Adjust 3D Cladding ▸: Adjust the position, scale and offset of siding/brick on a wall surface in 3D. Status: Missing.
- **C-74** 3D >> Toggle Patterns: Show plan/elevation material patterns instead of textures in the 3D view. Status: Missing.
- **C-75** Toolbar 2 >> 3D Solid flyout: 3D solid flyout. Status: Works.
- **C-76** Presentation (not captured) >> Photon mapping / global illumination bake (not captured): Older Chief photon-map light bake; modern Chief uses ray tracing only. Status: Differs-by-design.
- **C-77** Presentation (not captured) >> 360 panorama / Virtual Tour export (not captured): Render a 360-degree panorama and a clickable virtual tour for sharing with clients. Status: Missing.
- **C-78** Presentation (not captured) >> 3D Viewer / Chief 3D sharing (cloud) (not captured): Upload a 3D model for clients to view in a browser. Status: Missing.
- **C-79** Presentation (not captured) >> Photo sharing / Share to social, Chief cloud (not captured): Send images and models to Chief cloud services. Status: Missing.

### Cabinets, stairs, framing, terrain, library (`docs/parity/cabinets-stairs-framing-terrain-library.md`, CB- ids)

- **CB-69** Build Stairs >> Elevator (listed in the toolbar capture): Elevator cab and shaft that cuts the floor platforms and opens on landings. Status: Missing.
- **CB-70** Build Cabinets >> Corner Base Cabinet (not captured): L-shaped corner units with lazy-susan or blind options. Status: Works. Also listed as: Build Cabinets >> Corner Wall Cabinet.
- **CB-71** Build Cabinets >> Blind Base Cabinet (not captured): Cabinets with a blind corner section. Status: Works. Also listed as: Build Cabinets >> Blind Wall Cabinet.
- **CB-72** Build Solids >> 3D Solid Feature (custom molding profile): Draw a 2D profile and extrude or sweep it into a 3D solid or molding. Status: Partial.
- **CB-73** Terrain >> Plant Chooser (Plant Chooser dialog) (not captured): Browse plants by name, type, size, hardiness zone and sun; place with a drawn canopy. Status: Partial.
- **CB-74** Terrain >> Hardiness zones / plant growth size by age (not captured) (not captured): Plant data carries USDA zone and growth size over years, used by the Plant Chooser filter. Status: Missing.
- **CB-75** Terrain >> Import terrain data (DXF points, text, GPX) (not captured): Load survey points from a DXF/CSV/text file into elevation points. Status: Works. Also listed as: Exchange (not captured) >> Import terrain data (survey points).
- **CB-76** Terrain >> Terrain Cut and Fill Report (not captured): Cut and fill volumes between existing and finished grade. Status: Works.
- **CB-77** Terrain >> Building Pad (not captured): Flat pad under the building at the first-floor elevation. Status: Works.
- **CB-78** Library >> Get Additional Content…: Download extra library catalogs, bonus catalogs and manufacturer content from Chief's servers. Status: Missing.
- **CB-79** Library >> Install Core Content: Install the Core Library catalogs from the installer. Status: Differs-by-design.
- **CB-80** Library >> Update Library Catalogs: Refresh the catalog index and pull catalog updates. Status: Partial.
- **CB-81** Library >> Import 3D Model (OBJ, glTF) (not captured): Bring an outside 3D model in as a library symbol (Chief imports 3DS, OBJ, SKP, DAE, STL via Import 3D Symbol). Status: Works. Also listed as: Exchange (not captured) >> Import 3D symbols OBJ / glTF.
- **CB-82** Library >> Import 3D Model (3DS, SKP, DAE, STL formats) (not captured) (not captured): Import 3DS, SketchUp SKP, COLLADA DAE and STL meshes into the library. Status: Missing. Also listed as: Exchange (not captured) >> Import 3DS / SketchUp SKP / COLLADA DAE / STL.
- **CB-83** Library >> Manufacturer catalogs, 3D Warehouse, bonus catalogs (not captured) (not captured): Product catalogs from manufacturers and the SketchUp 3D Warehouse inside the Library Browser. Status: Missing.
- **CB-84** CAD >> North Pointer: Plan symbol that points north and sets the sun's compass bearing. Status: Works.
- **CB-85** Default Settings >> 3D Solid defaults: Default material, size and layer for 3D solids. Status: Missing.
- **CB-86** Building (not captured) >> Deck framing and decking boards (planking) (not captured): Deck surface boards, joists, beams, ledger and footings built from the deck outline. Status: Missing. Also listed as: Build Walls >> Build Deck Framing.
- **CB-87** Building (not captured) >> Chimneys and fireplaces (not captured): Chimney and fireplace tools that build the firebox, flue, chase and cap through roof and floors. Status: Partial.
- **CB-88** Building (not captured) >> Plumbing and HVAC symbols and connections (not captured): Plumbing fixtures with pipe connections, HVAC registers and duct runs on their own layers. Status: Partial.
- **CB-89** Project >> Kitchen and Bath design checks (NKBA clearances) (not captured): Check kitchen work triangle, clearances and counter lengths against NKBA guidelines. Status: Partial.

### Layout, print, schedules, import and export (`docs/parity/documentation-layout.md`, L- ids)

- **L-48** File >> Export ▸ glTF 3D model (not captured): Export the 3D model for viewers (Chief exports 3D Viewer, OBJ, 3DS, COLLADA, STL, SKP). Status: Works.
- **L-49** File >> Export ▸ Picture / 3D view image (PNG, JPEG, BMP, TIFF): One Export Picture command that saves the active view (plan, 3D, layout page) as a picture file with chosen size and format (PNG, JPEG, TIFF, BMP). Status: Partial. Also listed as: Exchange (not captured) >> Export picture (JPEG, PNG, TIFF) of a view.
- **L-50** File >> Import ▸ Chief Plan (.plan) (not captured): Open a Chief Architect plan (read-only decode of Daniel's install files, Round 13/14 stages). Status: Works.
- **L-51** File >> Import ▸ Import Project / Merge Plan (not captured) (not captured): Import another plan's objects, layers, library items into this plan. Status: Missing.
- **L-52** Tools >> Project Information…: Client, project, designer and revision data used in title blocks and macros. Status: Works.
- **L-53** View >> Watermark: Overlay a DRAFT / NOT FOR CONSTRUCTION watermark on screen and prints. Status: Missing.
- **L-54** Other dialogs >> Revision Table / Revision Cloud dialog (Project Information > Revisions) (not captured): Revision list that feeds a revision table on the sheet. Status: Works.
- **L-55** Exchange (not captured) >> Export OBJ / STL / 3DS / COLLADA 3D models (not captured): Export the 3D model for 3D printing and other programs. Status: Missing.
- **L-56** Exchange (not captured) >> Export Materials List to HTML (not captured): Save the materials list or a schedule as an HTML page. Status: Missing.

### Electrical (`docs/parity/electrical.md`, E- ids)

- **E-17** Build Electrical >> Ceiling Fan (not captured): Low-voltage, safety and fan symbols placed on a wall or ceiling with a height and schedule row. Status: Works. Also listed as: Build Electrical >> CO Detector; Build Electrical >> Data Jack; Build Electrical >> Doorbell; Build Electrical >> Electrical Panel; Build Electrical >> Phone Jack; Build Electrical >> Smoke Detector; Build Electrical >> TV Jack; Build Electrical >> Thermostat.

### Application shell: files, windows, help, tools menu (`docs/parity/preferences-hotkeys-toolbars.md`, APP- ids)

- **APP-1** File >> New Plan (Cmd+N): new plan from the shipped Chief X18 defaults. Status: Works.
- **APP-2** File >> Templates > New Plan From Template…: File > Templates > New Plan From Template: choose a saved plan template and start a new untitled plan from it. Status: Partial.
- **APP-3** File >> Open Plan… (Cmd+O): Open a Plan Studio plan file; offers to save unsaved changes. Status: Works.
- **APP-4** File >> Open Layout…: Open a layout file. Status: Works.
- **APP-5** File >> Open Recent Documents ▸: Recent plans and layouts list with Clear Menu. Status: Works.
- **APP-6** File >> Dashboard…: Chief's start page (tutorials, news, recent files). Status: Missing.
- **APP-7** File >> Download Sample Plans…: Download Chief's sample plans from the internet. Status: Missing.
- **APP-8** File >> Close View (Cmd+W): Close the active view tab. Status: Works.
- **APP-9** File >> Close All 3D Views: Close every open 3D camera view at once. Status: Missing.
- **APP-10** File >> Close All Views: Close every open view tab of the plan. Status: Missing.
- **APP-11** File >> Save (Cmd+S): Save the plan. Status: Works.
- **APP-12** File >> Save As…: Save under a new name. Status: Works.
- **APP-13** File >> Save As Template…: Save the open plan as a .tpl template (defaults, layers, views). Status: Partial.
- **APP-14** File >> Save Thumbnail Image: Write a preview image of the plan view into the file / as a PNG. Status: Missing.
- **APP-15** File >> Show in Project Browser: Open the Project Browser and select the active view. Status: Works.
- **APP-16** File >> View File Information…: Plan file size, dates, version, object counts. Status: Works.
- **APP-17** File >> Manage Auto Archives…: List and restore the automatic backups. Status: Works.
- **APP-18** File >> Backup Entire Plan… (not captured): Save a copy of the plan with its images and library items. Status: Works. Also listed as: Program (not captured) >> Auto Archive and Backup Entire Plan.
- **APP-19** File >> Save a Copy… (not captured): Save a copy without switching to it; reload the last saved version. Status: Works. Also listed as: File >> Revert to Saved.
- **APP-20** File >> Quit (Cmd+Q): Quit with save prompt. Status: Works.
- **APP-21** Edit >> Default Settings… (tree): Searchable tree of every object type's defaults; each leaf opens the object's specification dialog. Status: Partial.
- **APP-22** Edit >> Reset to Defaults…: Reset the selected default set / active defaults to the program values. Status: Missing.
- **APP-23** Edit >> AutoFill ▸ / Start Dictation / Emoji & Symbols (macOS-supplied): Operating-system text services, not Chief features. Status: Differs-by-design.
- **APP-24** Tools >> Active Defaults…: Pick which saved default set (walls, dimension, etc.) is active. Status: Partial.
- **APP-25** Tools >> Checks ▸ Plan Check: Check the plan against residential code rules and list findings with zoom-to. Status: Works. Also listed as: Program (not captured) >> Plan Check and building code presets.
- **APP-26** Tools >> Checks ▸ Door/Window Check: Check door and window sizes, egress and swing clearances. Status: Works.
- **APP-27** Tools >> Checks ▸ Plan Check Settings (jurisdiction, rule groups) (not captured): Choose the code edition and rule groups. Status: Works. Also listed as: Other dialogs >> Plan Check Settings and results.
- **APP-28** Tools >> Plan Database ▸: Browse and edit every object in the plan as a database list (select by type, change layer). Status: Missing. Also listed as: Toolbar 1 >> Plan Database button.
- **APP-29** Tools >> Time Tracker ▸: Log time spent on the plan per session and report it by project. Status: Missing. Also listed as: Program (not captured) >> Time tracking.
- **APP-30** Tools >> Loan Calculator…: Mortgage payment calculator. Status: Missing.
- **APP-31** Tools >> Ruby Console…: Run Ruby scripts and macros against the plan. Status: Missing. Also listed as: Program (not captured) >> Ruby macros and scripting.
- **APP-32** Tools >> Screen Capture ▸: Capture the window or a region to the clipboard or a file. Status: Missing.
- **APP-33** Tools >> Color Chooser…: Pick a color and copy its values. Status: Works.
- **APP-34** View >> Refresh Display (F5): Redraw the view and rebuild cached drawing data. Status: Works.
- **APP-35** View >> Project Browser: Tree of plan views, 3D views, layouts, pages and schedules; double-click to open. Status: Works. Also listed as: Program (not captured) >> Project Browser (views, layouts, cameras, schedules).
- **APP-36** View >> Tool Palette: Docked palette listing the tools of the current toolbar configuration. Status: Missing.
- **APP-37** View >> Status Bar / Scrollbars / Toolbars (show-hide): Show or hide status bar, scrollbars and toolbars. Status: Partial.
- **APP-38** View >> Enter Full Screen: Full-screen window. Status: Works.
- **APP-39** View >> Canvas theme and UI brightness (Plan Studio extra) (not captured): Paper, Low Glare, Dark and High Contrast themes (accessibility feature, not in Chief). Status: Works.
- **APP-40** Window >> Swap Views (F7): Switch between the last two active views. Status: Missing.
- **APP-41** Window >> Tile Horizontally: Show plan and 3D views side by side. Status: Missing. Also listed as: Window >> Tile Vertically.
- **APP-42** Window >> Tab Windows: Tabbed views and tab cycling. Status: Partial. Also listed as: Window >> Select Next Tab; Window >> Select Previous Tab.
- **APP-43** Window >> List of open views: Open views listed for switching. Status: Works.
- **APP-44** Account >> Sign In: Chief account sign-in and licence management. Status: Differs-by-design. Also listed as: Account >> Make License Available; Account >> My Account.
- **APP-45** Help >> Launch Help…: Open the built-in manual chapters. Status: Works. Also listed as: Help >> View Reference Manual…; Help >> View Tutorial Guide….
- **APP-46** Help >> View Training Videos: Links to Chief's video library, forum and support. Status: Differs-by-design. Also listed as: Help >> ChiefTalk; Help >> Technical Support; Help >> Visit Website.
- **APP-47** Help >> Download Program Updates…: Check for and download program updates. Status: Missing.
- **APP-48** Help >> Export Logs…: Save diagnostic logs for support. Status: Missing.
- **APP-49** Help >> System Information…: Show OS, GPU and version information. Status: Works.
- **APP-50** Help >> About Plan Studio / Keyboard Shortcuts… (not captured): About box and shortcut list. Status: Works.
- **APP-51** Toolbar 1 >> New Plan: File buttons. Status: Works. Also listed as: Toolbar 1 >> Open Plan; Toolbar 1 >> Save buttons.
- **APP-52** Toolbar 1 >> Launch Help button: Opens the help manual. Status: Works.
- **APP-53** Toolbar 1 >> Default Settings button: Opens the Default Settings tree. Status: Works.
- **APP-54** Right bar >> Project Browser toggle: Dock the Project Browser. Status: Works.
- **APP-55** Right bar >> Overflow chevron (hidden buttons menu): Menu for toolbar buttons that do not fit. Status: Works.
- **APP-56** Default Settings >> General (units, defaults, auto save): General plan defaults: measurement units, backup interval, auto archive, file options. Status: Partial.
- **APP-57** Default Settings >> Plan defaults (scale, view, layers, snaps): Drawing scale, grid, snap, layer set and view defaults for the plan. Status: Partial.
- **APP-58** Default Settings >> Default Settings search field: Filter the tree by name. Status: Works.
- **APP-59** Other dialogs >> Unsaved changes prompt / auto-archive / recover (not captured): Prompt to save, auto-archives of the open plan. Status: Works.
- **APP-60** Preferences >> Templates page (default plan and layout templates) (not captured): Default plan and layout template paths and seeding from a Chief template. Status: Works.
- **APP-61** Program (not captured) >> Templates and plan defaults (not captured): Plan and layout templates seeded from Chief's template files. Status: Works.
- **APP-62** Program (not captured) >> Interactive tutorials and sample plans (not captured): Guided tutorials and downloadable samples. Status: Missing.
- **APP-63** Application menu >> About, Preferences, Services, Hide, Quit (macOS application menu): macOS application menu with About, Preferences, Services, Hide and Quit. Status: Partial.
- **APP-64** Project >> Plan information stored in the file (version, units, floors, layers, views) (not captured): Plan file content: floors, layers, defaults, views. Status: Works.
- **APP-65** Project >> Autosave and crash recovery (not captured): Periodic autosave and recovery of unsaved work. Status: Works.
- **APP-66** Project >> Multiple documents and tabs (several plans open) (not captured): Open several plans at once and copy between them. Status: Missing.

## Top 40 to build next

Ranked by how often a residential designer doing custom homes and remodels, with construction documents built in layout, reaches for the feature. It mixes features that have a spec and are Partial or Missing with the NO SPEC gaps this audit found. "In progress" means a Round 14 builder already has it; the rank still shows where it falls once that lands. Size: S under a day, M a few days, L a week or more of one builder.

| # | Feature | Why a designer needs it | Parity | Status now | Size |
|---|---|---|---|---|---|
| 1 | Edit Area and Stretch CAD: rubber-band a region and move, rotate or copy everything in it | Moving or copying a whole wing, a bath group or a stretch of the plan without selecting object by object; a daily remodel move | S-90, S-91 | Missing, In progress (Round 14) | M |
| 2 | Door and window Rough Opening and Framing tabs: rough size, header height, clearance gaps, header and trimmer construction | Rough openings are on every door and window schedule and drive the wall framing headers in the construction set | DW-56, DW-114 | Missing | M |
| 3 | Wall Specification Structure and Foundation tabs: through walls, platform intersections, balloon walls, footing and stem wall | Multi-story walls and foundations are set up here on every two-story plan | W-39, W-52, W-62, W-63, R-69 | Partial, In progress (Round 14) | L |
| 4 | Fill Window Building Only and rubber-band Zoom | Navigation done dozens of times a session; Fill Window Building Only ignores far-away survey CAD | LAY-27, LAY-24 | Partial | S |
| 5 | Wall drawing gestures: right-click and drag-release keep the chain, dashed alignment guides to other endpoints | The first thing done in every plan; today it differs from Chief's feel and needs one live comparison | W-3, W-4, W-14 | Partial | S |
| 6 | Build Foundation types, basement and crawl space rooms, attic floor from Build Roof | Basements, crawl spaces and attics are in most remodel and custom jobs | R-61, R-62, R-18, R-68 | Partial, In progress (Round 14) | L |
| 7 | Export Picture of any view (live 3D, layout page, plan) as PNG or JPEG | Sending a plan, elevation or rendering to a client by email or into a proposal; only Print Image (plan lines, PNG) and Ray Trace > Save PNG write images today | L-49 | Partial | S |
| 8 | Cabinet Specification depth: Front tree, Door/Drawer styles from the library, Moldings, soffit from a polyline | Kitchens and baths are the highest-value interior work | CB-7, CB-12, CB-17, CB-4 | Partial, In progress (Round 14) | M |
| 9 | Stairs: guard rails around the stairwell, Stair Specification tabs, break line and UP/DN symbol, flared and curved runs | Nearly every two-story plan has a stair checked for railings in plan review | CB-29, CB-32, CB-33, CB-26 | Partial, In progress (Round 14) | M |
| 10 | Room finish surfaces: rough ceiling, finish thickness, moldings and materials per room, label style | Finish schedules and interior 3D views of rooms | R-25, R-27, R-34, R-36, R-46 | Partial, In progress (Round 14) | M |
| 11 | Text box: wrap width, handles, alignment, border and background, rich text | General notes and room notes go on every sheet | TXT-16, TXT-4, TXT-14 | Partial, In progress (Round 14) | M |
| 12 | Dimension Specification: format tabs, multi-point strings as one object, Align/Distribute, default sets | Fixing one dimension string without retyping each piece | DIM-31, DIM-39, DIM-2, DIM-36 | Partial, In progress (Round 14) | M |
| 13 | Schedules in the DXF and construction-set PDF, custom schedule builder, Schedule tab data and component quantities | Door, window and finish schedules are on every permit set; supplier and mark data are typed by hand today | L-31, L-29, L-35, L-32 | Partial, In progress (Round 14) | M |
| 14 | Auto Detail, CAD Detail From View and CAD Detail Management | Wall sections and typical details are re-drawn on every job | L-39, L-41, CAD-35, L-40 | Partial, In progress (Round 14) | M |
| 15 | Open several plans at once and paste between plans (keeping layers) | Remodels reuse details, symbols and whole rooms from earlier plans | APP-66, S-85 | Partial, In progress (Round 14) | L |
| 16 | Deck framing and decking boards from the deck outline | Decks and porches are on most residential jobs and their framing is in the permit set | CB-86 | Missing | M |
| 17 | Wall Covering on walls and rooms (wainscot, tile, paneling) | Interior finish coverage shown in 3D and in the finish schedule | W-115, R-84 | Partial | M |
| 18 | Window Shape tab: raked sides, angled top corners, custom heights | Gable-end and transom windows are common in custom homes | DW-121 | Missing | M |
| 19 | Bay, box and bow windows with their roofs, depth and seat | Bays are common in residential elevations; today they project a fixed depth and have no roof | DW-48, RF-29 | Partial, In progress (Round 14) | M |
| 20 | Roof finishing: Auto Roof Return, plane handles, truss types and web layout | Fine-tuning a roof after Build Roof and framing it for the permit set | RF-27, RF-38, RF-54, RF-55 | Partial, In progress (Round 14) | M |
| 21 | Chimneys and fireplaces that run through floors, roof and cap | Nearly every custom home has a fireplace; today only a library symbol and a roof hole exist | CB-87 | Partial | M |
| 22 | DXF/DWG export options (version, units, blocks, selection) and PDF underlay with calibration | Surveyors and engineers still exchange DXF; scanned surveys arrive as PDF | L-44, L-45, L-46 | Partial, In progress (Round 14) | M |
| 23 | Select feedback: hover description, selection count and Z in the status bar, host wall highlight | Small daily frictions on large plans | S-6, S-98, S-110 | Partial, In progress (Round 14) | S |
| 24 | Camera View Options, View Direction snaps, light sets (Day, Evening, Interior) | Client views: day and evening exteriors and interior lighting | C-31, C-41, C-65 | Partial, In progress (Round 14) | M |
| 25 | Railing walls: Wall Cap, Newels/Balusters and Rails tabs | Deck, balcony and stair railings are specified in permit drawings | W-114, W-116, W-117 | Partial | M |
| 26 | Import SketchUp, 3DS, COLLADA and STL models into the library | Clients and suppliers send furniture and fixtures as SketchUp or 3DS models | CB-82 | Missing | M |
| 27 | Door and window Materials and Layer tabs | Painting a door and its casing a different color per opening in 3D | DW-117, DW-116 | Missing | S |
| 28 | Layer Painter / Layer Eyedropper and Object Painter | Moving or restyling many objects to match one without a dialog | LAY-18, S-116 | Missing | S |
| 29 | Reverse Plan and Rotate Plan View | Flipping a plan for a mirrored lot and squaring a skewed lot to the page | S-115, LAY-17 | Missing | M |
| 30 | Split-level floors on one story | Hillside and renovation jobs with half-levels | R-86 | Missing | L |
| 31 | Boolean polyline operations: union, subtract, intersect | Building site outlines and complex CAD shapes | CAD-55 | Missing | M |
| 32 | Tile windows, swap views, tab cycling | Working in plan and 3D at the same time | APP-41, APP-40, APP-42 | Partial | M |
| 33 | Drawing Groups: line-weight groups mapped to object types | Consistent plot line weights across the permit set | LAY-36 | Partial | M |
| 34 | Spell check for text boxes, notes and labels | Typos on a permit sheet cost a resubmittal | TXT-21 | Missing | S |
| 35 | Default Settings tree completed: CAD, Stairs, Slab, Foundation, Image, Corner Trim, 3D Solid pages | One place to set defaults before drawing; today 17 of 29 groups are missing | APP-21, CAD-57 | Partial | M |
| 36 | 360 panorama and walkthrough video file export | Client presentations | C-77, C-71 | Partial | M |
| 37 | Adjust 3D Cladding: siding and brick start point and course offset | Elevation views for siding and brick take-offs | C-73 | Missing | S |
| 38 | Structural calculators for beams, headers and deck materials | Sizing a header or beam without a separate tool; kitchen NKBA clearance checks complete the pair | CB-42, CB-88 | Partial | M |
| 39 | Window treatments: curtains, blinds and exterior millwork on windows | Interior design presentations and 3D views of finished rooms show curtains and blinds | DW-123 | Missing | M |
| 40 | Object Information tabs (code, comment, manufacturer, supplier) on walls, doors and windows | Supplier and model data flow into schedules and the materials list | W-118, DW-118 | Missing | S |

## Features that are intentionally out of scope

| Chief feature | Recommendation |
|---|---|
| Ruby console and macros (Tools > Ruby Console) | Out of scope for the first release. Chief's Ruby API is a licensed scripting surface; Plan Studio's answer is a documented Rust API and the headless scenario helpers. Revisit only if a scripting need appears (Daniel has not used Ruby macros). |
| 3D Warehouse login, manufacturer catalogs, Get Additional Content, Install Core Content | Out of scope. They need Chief's online services and licensed content; Plan Studio reads the catalogs Daniel already has installed (plan-calib) and ships its own starter catalog. Keep Import Library and the Library Browser; skip the rest. |
| Chief 3D Viewer, photo sharing, cloud project sharing, Dashboard news, training videos, ChiefTalk, Technical Support | Out of scope (vendor cloud). The local equivalents are glTF export, Export Picture and a Plan Studio GitHub link. |
| Account, licence and My Account menu | Differs by design (open source, no licence). |
| Loan Calculator | Out of scope; not a design feature. |
| Time Tracker | Defer. Useful for billing hourly work but Daniel bills by phase; build only after the Top 40. |
| Plan Database | Defer. Select Same Type, the schedules and the Project Browser cover most of its use. |
| macOS Start Dictation, AutoFill, Emoji and Symbols | Operating-system rows; show them only where the platform supplies them. |
| Photon mapping | Differs by design: plan-render is a path tracer, so no photon pass exists or is needed. |
| DWG read and write | Defer. Needs a licensed or clean-room reader; DXF covers consultants today. Offer a "convert DWG to DXF first" message. |
| Room Planner import | Low value; the Space Planning Assistant already makes a plan from a questionnaire. |
| Annotation Sets, Tool Palette, Coordinate System Indicator, Watermark, Angle Snap Grid | Low value for Daniel's work; build on request. |
