# Parity: Preferences, Customize Hotkeys, Customize Toolbars

Round 14. Reference: Chief Architect X18 Preferences dialog, Customize Hotkeys and Toolbar
Customization. Lines marked **verify in Chief** are the nearest Chief-like choice where the exact
behavior is not captured (see DECISIONS.md rows for this round). Evidence names a file and a test.
Tests live in `crates/plan-app/src/scenarios/s39_prefs_r14.rs` unless another file is named.

## Preferences (Edit > Preferences, Cmd+,)

| Id | Chief behavior | Status | Evidence |
|---|---|---|---|
| PR-1 | One window with a page list: Appearance, Colors, Text, Library Browser, Render, Materials List, Reset Options, Folders, Edit, Behaviors, Snap Properties, Architectural, CAD, General Plan Defaults, Unit Conversions | Works | `dialogs/preferences.rs Page::ALL`; `every_page_draws_with_changed_values` |
| PR-2 | Every page is kept in `~/.plan-studio/preferences.json`: versioned, each key serde-defaulted, older files filled from the defaults, a newer file's version kept | Works | `preferences/pages.rs PrefsFile`; `every_page_is_covered_and_round_trips_through_preferences_json`; `pages::tests::a_partial_or_old_file_fills_the_rest_from_the_defaults` |
| PR-3 | Appearance: background, grid, selection, temporary dimension and text colors over the canvas theme; low-glare canvas and interface switches; icon size | Works | `preferences::tint_palette`, `pages::icon_px` (read by `toolbar.rs` for every icon); `the_canvas_colors_go_over_the_theme`, `applying_a_file_reaches_the_editor`. The low-glare switches set the canvas theme and the UI brightness (saved with the theme in `settings.json`). Temporary dimensions share `Palette::dimension_text` with the dimension tool's rubber band. Verify in Chief for which colors Chief lists |
| PR-4 | Library Browser: thumbnail size, search options (names, descriptions, keywords, catalog names, whole words, match all) | Partial | Page and file: `ui::library_browser`, `pages::LibraryBrowserPrefs`. `shell/library_browser.rs` still draws at its `PREVIEW_PX` and searches as before (docs/integration-queue.md, "Preferences round 14") |
| PR-5 | Text: interface text size, installed fonts, link to the default text styles | Works | `preferences.rs fonts`, `ui::text_links` (opens Default Settings) |
| PR-6 | Render: ray-trace defaults, preview quality, shadows and ambient occlusion | Partial | `pages::view_settings()` returns the `ViewSettings` a new 3D view should start with; the 3D panel still starts from `ViewSettings::default()` (queued) |
| PR-7 | Materials List: waste, rounding, prices, floors | Partial | Page and file only (`pages::MaterialsPrefs`); `dialogs/materials.rs` does not read them yet (queued). Verify in Chief for the option set |
| PR-8 | Reset Options: reset toolbars, dialog sizes, "don't ask again" messages, side windows; each asks for a second click | Works | `ui::run_reset`, `ui::reset_options`; `reset_options_reset_what_they_name`; `pages::tests::dont_ask_again_is_kept_and_reset`. No dialog asks "don't ask again" yet; `pages::dont_ask` / `set_dont_ask` are the hooks |
| PR-9 | Folders: library, textures, backdrops, templates, autosave, user library, each with an "exists" mark, Browse and Default | Partial | `ui::folders`, `pages::folder_status`, `pages::folder(kind)`; `pages::tests::folders_show_the_default_or_the_override_and_whether_it_exists`. The library folder is the Chief catalog setting (`settings.json`), read by the Library Browser. The other folders are stored and validated; the code that reads textures, templates and the user library still uses its own fixed paths (queued) |
| PR-10 | Edit: rotate about, resize about, marquee selection, snap switches | Partial | `ui::edit`; marquee is applied live (`select::set_marquee_mode`); the snap switches write the plan's editing defaults; rotate/resize about are kept in `PagePrefs.edit` and not yet read by the Rotate and Resize handles (queued) |
| PR-11 | Behaviors: Edit Type default, Replicate dialog, camera step sizes | Partial | `ui::behaviors`; Edit Type and Replicate settings go to `EditingDefaults.behavior` live; the camera steps are kept in `PagePrefs.behaviors` and `shell/view3d_panel/nudge.rs` still uses 24 in / 15 / 5 degrees (queued) |
| PR-12 | Snap Properties: every snap kind, sensitivity, angle increment and allowed angles, bumping and its distance | Works | `ui::snaps`; `saved_editing_defaults_reach_a_plan_on_the_first_frame`; `ui::parse_angles` test `allowed_angles_parse_and_print`. Kept in `preferences.json` and laid over each run's plan defaults once at startup; opening another plan keeps that plan's own (queued: call `pages::apply_editing` after a file opens) |
| PR-13 | Architectural: cabinet countertop join and gap fit; auto rebuild roofs, walls, foundations, attic walls; delete unused roof planes | Partial | Cabinet options work as before. The auto-rebuild switches are kept (`PagePrefs.architectural`) and read by nothing yet: the roof and wall rebuild code has no such switch (queued) |
| PR-14 | CAD: arc centers, end caps, line weights | Partial | `ui::cad`, `pages::CadPrefs` (Daniel's X18 line weights at first); the CAD painter does not read them yet (queued) |
| PR-15 | General Plan Defaults: a link to Default Settings | Works | `ui::plan_defaults` |
| PR-16 | Unit Conversions: a length in any unit shown in all of them | Works | `ui::convert`; `unit_conversions_show_every_unit`. Chief's page is not captured; this is a converter with a starting unit and rounding (verify in Chief) |

## Customize Hotkeys

| Id | Chief behavior | Status | Evidence |
|---|---|---|---|
| HK-1 | Full command list with Chief's menus as groups, searchable | Works | `dialogs/hotkeys.rs menu_of`, `grouped_commands`; `the_list_is_grouped_by_menu_and_the_search_narrows_it`. The menu of a command is read from its flyout group and name (Plan Studio's menus are code, not data); verify in Chief |
| HK-2 | A key already used shows its owner before Assign; Reassign takes it | Works | existing `a_recorded_sequence_shows_its_clash_before_assign` |
| HK-3 | The conflict list resolves with one click: keep the key for one command, take it from the others; Resolve All | Works | `HotkeyDialog::resolve`, `resolve_all`; `a_clash_is_found_and_resolved_in_favor_of_one_command` |
| HK-4 | Keys that are two keys on the Mac but one where Control and Command fold (Windows, Linux) are listed | Works | `HotkeyMap::folded_collisions`; `keys_that_fold_together_without_a_command_key_are_listed` (the Mac branch checks the collision; elsewhere the assignment is refused as an ordinary clash) |
| HK-5 | Import `UserHotkeys.xml` | Works | existing `importing_chief_hotkeys_again_restores_daniels_keys_over_edits` |
| HK-6 | Export `UserHotkeys.xml` in Chief's format: every command id of Daniel's file, Plan Studio's first sequence for the commands it has, Daniel's key for the rest | Works | `HotkeyMap::to_chief_xml`, `plan_config::write_hotkeys_xml`; `the_keys_export_as_chiefs_user_hotkeys_xml_and_import_back`. Chief keeps one sequence per command, so extra sequences (Plan Studio's number keys) are not written; commands with no Chief id are counted in the summary |
| HK-7 | Reset to Chief defaults, or to Daniel's file | Works | `HotkeyMap::reset_to_chief_defaults`, `HotkeyDialog::reset_to_chief`, `reset`; `reset_goes_to_daniels_file_or_to_chiefs_defaults` |

## Customize Toolbars

| Id | Chief behavior | Status | Evidence |
|---|---|---|---|
| TB-1 | Per view kind: add, remove and reorder buttons, separators, new rows | Works (before this round) | `dialogs/customize_toolbars.rs`; `ticking_a_button_adds_it_after_the_pick_and_unticking_removes_it`, `move_remove_and_separators_act_on_the_picked_entry`, `rows_are_added_named_hidden_and_deleted` |
| TB-2 | Rename a row, move a row up or down, duplicate a row | Works | `ViewSet::rename_row`, `move_row`, `duplicate_row`; `toolbar_rows_are_renamed_ordered_and_copied` |
| TB-3 | Load and save the configuration as JSON | Works | `export_json`, `import_json`; the same test round-trips order and names |
| TB-4 | Read a Chief `.toolbar` file | Works | `import_chief_text`; `a_chief_toolbar_file_still_imports_into_the_dialog`, `importing_a_chief_toolbar_file_replaces_the_standard_rows` |
| TB-5 | Reset toolbars (also from Preferences > Reset Options) | Works | `config::reset_all_live`; `a_committed_toolbar_set_is_live_until_reset_toolbars` |

<!-- coverage-audit:start -->
## Coverage audit additions (2026-10-08)

Rows added by the Round 14 coverage audit (`docs/chief-feature-coverage.md`): Chief X18 features found in the menu, toolbar, sub-tool and dialog captures, or known from the product, that no row above covered. Status comes from a code search, not a Chief session; "verify in Chief" marks behavior known only from the product. Variants of one flyout or tab share one row.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| APP-1 | New Plan (Cmd+N): new plan from the shipped Chief X18 defaults. | Works | menus.rs file_menu "New Plan"; dialogs/app_info.rs new_plan (clean-plan and unsaved prompt in files.rs) |
| APP-2 | Templates > New Plan From Template…: File > Templates > New Plan From Template: choose a saved plan template and start a new untitled plan from it. | Partial | File > Templates holds Save Current Defaults as My Template, Import Chief Template, Reset to Chief X18 Template (templates.rs); there is no "pick a template and open a new plan from it" row |
| APP-3 | Open Plan… (Cmd+O): Open a Plan Studio plan file; offers to save unsaved changes. | Works | files.rs open_dialog, perform; test unsaved_plans_prompt_and_clean_plans_go_straight_through |
| APP-4 | Open Layout…: Open a layout file. | Works | menus.rs "Open Layout…"; layout_window.rs |
| APP-5 | Open Recent Documents ▸: Recent plans and layouts list with Clear Menu. | Works | menus.rs recent_menu; files.rs test recents_dedupe_and_cap |
| APP-6 | Dashboard…: Chief's start page (tutorials, news, recent files). | Missing | not in menus.rs |
| APP-7 | Download Sample Plans…: Download Chief's sample plans from the internet. | Missing | not present; repo has samples/ but no in-app download |
| APP-8 | Close View (Cmd+W): Close the active view tab. | Works | menus.rs "Close View"; plan_tabs.rs |
| APP-9 | Close All 3D Views: Close every open 3D camera view at once. | Missing | only Close View and Close Plan exist |
| APP-10 | Close All Views: Close every open view tab of the plan. | Missing | only Close View and Close Plan exist |
| APP-11 | Save (Cmd+S): Save the plan. | Works | files.rs mark_saved; tests save_archives_the_old_version_and_a_failed_save_changes_nothing, dirty_tracking_follows_edits_undo_and_saves |
| APP-12 | Save As…: Save under a new name. | Works | menus.rs "Save As…"; files.rs perform |
| APP-13 | Save As Template…: Save the open plan as a .tpl template (defaults, layers, views). | Partial | "Save Current Defaults as My Template…" saves the plan defaults, not a whole plan as a template file (templates.rs) |
| APP-14 | Save Thumbnail Image: Write a preview image of the plan view into the file / as a PNG. | Missing | not in menus.rs; plan thumbnail is only read when importing Chief plans |
| APP-15 | Show in Project Browser: Open the Project Browser and select the active view. | Works | menus.rs "Show in Project Browser"; docks.rs Project dock |
| APP-16 | View File Information…: Plan file size, dates, version, object counts. | Works | app_info::FILE_INFO; dialogs/app_info.rs |
| APP-17 | Manage Auto Archives…: List and restore the automatic backups. | Works | files.rs archives_window; test rotation_keeps_only_the_newest_n_archives |
| APP-18 | Backup Entire Plan…: Save a copy of the plan with its images and library items. Also covers: Auto Archive and Backup Entire Plan. (Not captured; verify in Chief.) | Works | files.rs backup_entire_plan; test a_backup_zips_the_plan_and_the_pictures_it_uses |
| APP-19 | Save a Copy…: Save a copy without switching to it; reload the last saved version. Also covers: Revert to Saved. (Not captured; verify in Chief.) | Works | files.rs save_a_copy; test revert_reloads_the_saved_file |
| APP-20 | Quit (Cmd+Q): Quit with save prompt. | Works | files.rs on_exit; test the_close_button_asks_first_when_there_are_unsaved_changes |
| APP-21 | Default Settings… (tree): Searchable tree of every object type's defaults; each leaf opens the object's specification dialog. | Partial | dialogs/defaults.rs TREE has Walls (3), Doors (2), Windows, Dimension, Text, Floors and Rooms, Roofs, Cabinets, Framing, Terrain, Preferences/Templates; 17 of Chief's 29 top-level groups are missing (see Default Settings pages) |
| APP-22 | Reset to Defaults…: Reset the selected default set / active defaults to the program values. | Missing | shown dimmed (menus.rs inert list) |
| APP-23 | AutoFill ▸ / Start Dictation / Emoji & Symbols (macOS-supplied): Operating-system text services, not Chief features. | Differs-by-design | macOS system rows; Plan Studio shows them dimmed (menus.rs) and does not supply them |
| APP-24 | Active Defaults…: Pick which saved default set (walls, dimension, etc.) is active. | Partial | menus.rs "Active Defaults…" opens the Default Settings tree; there is no "set the active default set" |
| APP-25 | Checks ▸ Plan Check: Check the plan against residential code rules and list findings with zoom-to. Also covers: Plan Check and building code presets. | Works | plan-check crate (IRC rules, rules_irc.rs); dialogs/plan_check.rs; docs/manual/18-plan-check.md |
| APP-26 | Checks ▸ Door/Window Check: Check door and window sizes, egress and swing clearances. | Works | plan-check door_window_check; menus.rs |
| APP-27 | Checks ▸ Plan Check Settings (jurisdiction, rule groups): Choose the code edition and rule groups. Also covers: Plan Check Settings and results. (Not captured; verify in Chief.) | Works | plan-check settings.rs; dialogs/plan_check.rs |
| APP-28 | Plan Database ▸: Browse and edit every object in the plan as a database list (select by type, change layer). Also covers: Plan Database button. | Missing | toolbar button "Plan Database" is a stub; menus.rs has no row |
| APP-29 | Time Tracker ▸: Log time spent on the plan per session and report it by project. Also covers: Time tracking. | Missing | nothing in crates (grep finds nothing) |
| APP-30 | Loan Calculator…: Mortgage payment calculator. | Missing | out of scope for a design tool |
| APP-31 | Ruby Console…: Run Ruby scripts and macros against the plan. Also covers: Ruby macros and scripting. | Missing | no scripting console (out of scope, see recommendation) |
| APP-32 | Screen Capture ▸: Capture the window or a region to the clipboard or a file. | Missing | no in-app screen capture to clipboard / file |
| APP-33 | Color Chooser…: Pick a color and copy its values. | Works | app_info::COLOR_CHOOSER; dialogs/app_info.rs |
| APP-34 | Refresh Display (F5): Redraw the view and rebuild cached drawing data. | Works | app_info::REFRESH; menus.rs view_menu |
| APP-35 | Project Browser: Tree of plan views, 3D views, layouts, pages and schedules; double-click to open. Also covers: Project Browser (views, layouts, cameras, schedules). | Works | shell/docks.rs Project dock (views, layouts, cameras, schedules); main.rs |
| APP-36 | Tool Palette: Docked palette listing the tools of the current toolbar configuration. | Missing | no floating tool palette (toolbars and flyouts only) |
| APP-37 | Status Bar / Scrollbars / Toolbars (show-hide): Show or hide status bar, scrollbars and toolbars. | Partial | Status Bar and Toolbars toggles live (app_info TOGGLE_STATUS_BAR, TOGGLE_TOOLBARS); no Scrollbars toggle in menus.rs |
| APP-38 | Enter Full Screen: Full-screen window. | Works | app_info::FULL_SCREEN; menus.rs |
| APP-39 | Canvas theme and UI brightness (Plan Studio extra): Paper, Low Glare, Dark and High Contrast themes (accessibility feature, not in Chief). (Not captured; verify in Chief.) | Works | menus.rs Canvas Theme, UI Brightness; theme.rs |
| APP-40 | Swap Views (F7): Switch between the last two active views. | Missing | no second-view swap (grep swap_views finds nothing) |
| APP-41 | Tile Horizontally: Show plan and 3D views side by side. Also covers: Tile Vertically. | Missing | views are tabs only; no tiling (grep Tile finds nothing) |
| APP-42 | Tab Windows: Tabbed views and tab cycling. Also covers: Select Next Tab; Select Previous Tab. | Partial | plan_tabs.rs tabs for plan, 3D, layout views; no Ctrl+Tab cycle menu rows |
| APP-43 | List of open views: Open views listed for switching. | Works | menus.rs window_menu "Floor Plan View", "Layout" rows |
| APP-44 | Sign In: Chief account sign-in and licence management. Also covers: Make License Available; My Account. | Differs-by-design | no licensing or accounts in an open-source app (Account menu omitted by design, chief-x18-menus.md) |
| APP-45 | Launch Help…: Open the built-in manual chapters. Also covers: View Reference Manual…; View Tutorial Guide…. | Works | app_info HELP, HELP_REFERENCE, HELP_TUTORIAL; docs/manual |
| APP-46 | View Training Videos: Links to Chief's video library, forum and support. Also covers: ChiefTalk; Visit Website; Technical Support. | Differs-by-design | replaced by Plan Studio on GitHub… and Keyboard Shortcuts… (help_menu); no vendor services |
| APP-47 | Download Program Updates…: Check for and download program updates. | Missing | no update checker (releases are on GitHub) |
| APP-48 | Export Logs…: Save diagnostic logs for support. | Missing | no log export (status bar keeps the last 50 messages) |
| APP-49 | System Information…: Show OS, GPU and version information. | Works | app_info::SYSTEM_INFO; menus.rs |
| APP-50 | About Plan Studio / Keyboard Shortcuts…: About box and shortcut list. (Not captured; verify in Chief.) | Works | app_info ABOUT, HELP_HOTKEYS |
| APP-51 | New Plan: File buttons. Also covers: Open Plan; Save buttons. | Works | toolbar.rs row1 buttons -> files.rs |
| APP-52 | Launch Help button: Opens the help manual. | Works | toolbar.rs "Launch Help" -> app_info::HELP |
| APP-53 | Default Settings button: Opens the Default Settings tree. | Works | toolbar.rs -> Action::DefaultSettings |
| APP-54 | Project Browser toggle: Dock the Project Browser. | Works | toolbar.rs view_slots Dock::Project; shell/docks.rs |
| APP-55 | Overflow chevron (hidden buttons menu): Menu for toolbar buttons that do not fit. | Works | toolbar/config.rs draw_bar overflow |
| APP-56 | General (units, defaults, auto save): General plan defaults: measurement units, backup interval, auto archive, file options. | Partial | Preferences > General Plan Defaults is a link (PR-15); measurement units live in Project Information / exchange options |
| APP-57 | Plan defaults (scale, view, layers, snaps): Drawing scale, grid, snap, layer set and view defaults for the plan. | Partial | plan defaults are seeded from the template (plan_defaults.rs, templates.rs) and the editing defaults dialog (Snap Settings); no single Plan page |
| APP-58 | Default Settings search field: Filter the tree by name. | Works | dialogs/defaults.rs search "Filter the tree" |
| APP-59 | Unsaved changes prompt / auto-archive / recover: Prompt to save, auto-archives of the open plan. (Not captured; verify in Chief.) | Works | dialogs/unsaved.rs; files.rs auto archive |
| APP-60 | Templates page (default plan and layout templates): Default plan and layout template paths and seeding from a Chief template. (Not captured; verify in Chief.) | Works | dialogs/defaults.rs Templates; templates.rs |
| APP-61 | Templates and plan defaults: Plan and layout templates seeded from Chief's template files. (Not captured; verify in Chief.) | Works | templates.rs, plan_defaults.rs |
| APP-62 | Interactive tutorials and sample plans: Guided tutorials and downloadable samples. (Not captured; verify in Chief.) | Missing | manual chapters only; sample plans are repo files |
| APP-63 | About, Preferences, Services, Hide, Quit (macOS application menu): macOS application menu with About, Preferences, Services, Hide and Quit. | Partial | About, Preferences and Quit are rows of the Plan Studio menus (menus.rs); there is no native macOS application menu (eframe draws its own bar) |
| APP-64 | Plan information stored in the file (version, units, floors, layers, views): Plan file content: floors, layers, defaults, views. (Not captured; verify in Chief.) | Works | plan-core model.rs Project; docs/chief-plan-format.md |
| APP-65 | Autosave and crash recovery: Periodic autosave and recovery of unsaved work. (Not captured; verify in Chief.) | Works | files.rs auto archive and recovery; dialogs/unsaved.rs |
| APP-66 | Multiple documents and tabs (several plans open): Open several plans at once and copy between them. (Not captured; verify in Chief.) | Missing | one plan per window; open replaces the plan after the unsaved prompt (files.rs open_dialog); views are tabs (editor/plan_tabs.rs) |
<!-- coverage-audit:end -->

## Code minimums (round 14)

Daniel's request, not a Chief feature: the IRC figures of Plan Check are the minimums the tools, dialogs and defaults use (`plan_check::CodeMinimums`, `editor/code.rs`, `dialogs/code_notice.rs`). Chief has no equivalent notice; the layout of the notice is ours ("verify in Chief" where a Chief dialog would show a warning).

| ID | Feature | Status | Evidence |
|---|---|---|---|
| CM-1 | One list of minimums derived from the Plan Check settings and jurisdiction preset | Works | plan-check minimums.rs (`CodeMinimums`, tests `each_jurisdiction_has_its_own_minimums`, `a_plans_settings_and_amendments_are_stored_with_it`) |
| CM-2 | New plans and Default Settings start at code-legal values (stairs, rails, bedroom window, exterior door, footing, garage wall type) | Works | code.rs `apply_code_minimums`; s41 `a_new_plan_starts_at_code_legal_defaults`, `applying_the_minimums_raises_illegal_defaults_once`, `file_new_seeds_the_defaults_again`; verify in Chief for which defaults Chief itself seeds |
| CM-3 | Apply code minimums to defaults (Plan Check Settings button and Tools > Checks command) | Partial | s41 `the_apply_button_raises_the_defaults_in_one_undo_step`; the defaults are not restored by Undo (DECISIONS 459) |
| CM-4 | Code notice under a field with the citation and Set to code | Works | code_notice.rs; s41 `a_riser_of_eight_inches_shows_the_notice_and_set_to_code_fixes_it_in_one_undo_step`; verify in Chief |
| CM-5 | Notices in Staircase, Railing wall, Window (egress), Door (exit), Room, Foundation and Framing dialogs | Works | s41 `the_tread_width_and_headroom_notices_follow_their_limits`, `a_bedroom_window_below_egress_shows_the_notice_and_set_to_code_fixes_the_sill`, `the_exit_door_is_held_to_its_width_and_height`; Room, Foundation, Framing and rail notices are drawn by the same widget and have no scenario test |
| CM-6 | Stair tool and Auto Place Outlets start from the minimums | Works | stairs_view.rs `build` calls `code::legalize_stair_params`; tools/electrical.rs calls `code::outlet_options`; s41 `a_new_stair_starts_at_the_plans_minimums`, `auto_place_outlets_takes_its_spacing_from_the_minimums` |
| CM-7 | Plan Check Fix for stairs and footings (one undo step) | Works | code.rs `fix_finding`; s41 `the_live_check_counts_a_stair_edit_and_fix_clears_it_in_one_undo_step`, `a_footing_finding_is_fixed_on_the_foundation_floor` |
| CM-8 | Status bar shows the code edition while the settings are open | Works | s41 `the_status_bar_names_the_code_while_the_settings_are_open` |
| CM-9 | Check while drawing: live count in the status bar and a badge on the Plan Check toolbar button | Partial | code.rs `update_live`; s41 `the_live_check_counts_a_stair_edit_and_fix_clears_it_in_one_undo_step`, `live_check_stays_fast_on_a_full_house`; the badge shows only when a toolbar holds the Plan Check button (the stock toolbars do not) |
