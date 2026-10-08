//! Scenario 39: Preferences, hotkeys and toolbar customisation, round 14.
//!
//! Every Preferences page keeps its choices in `preferences.json` and gives
//! them back; the pages draw; Reset Options resets what it names; the
//! hotkey dialog lists commands by menu, finds the clashes (including the
//! ones that exist only where Control and Command are one key), resolves them,
//! resets to Chief's defaults or Daniel's file, and writes Chief's own
//! `UserHotkeys.xml`; the toolbar dialog renames, orders and copies rows.

use super::s21_layout_print::isolate_home;
use crate::dialogs::customize_toolbars::ToolbarDialog;
use crate::dialogs::hotkeys::{find_conflicts, menu_of, FilterMode, HotkeyDialog, MENU_ORDER};
use crate::dialogs::preferences::pages::{
    self, EndCap, FolderKind, IconSize, PagePrefs, PrefsFile, PreviewQuality, PreviewSize,
    ResizeAbout, RotateAbout,
};
use crate::dialogs::preferences::ui::{self, Reset};
use crate::dialogs::preferences::{self, Page, Preferences};
use crate::editor::EditorContext;
use crate::shell::hotkeys::{Chord, HotkeyMap};
use crate::theme::{AppSettings, CanvasTheme, DockWidths};
use crate::toolbar::config::{self, ToolbarConfig, ViewKind};
use crate::tools::select::MarqueeMode;
use eframe::egui::{self, Key};
use plan_core::defaults::EditBehavior;
use plan_core::units::LengthUnit;

fn fresh() {
    isolate_home();
    preferences::set(Preferences::default());
    pages::set(PagePrefs::default());
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("plan-studio-s39-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn draw(page: Page, cx: &mut EditorContext, settings: &mut AppSettings) {
    let ctx = egui::Context::default();
    let mut actions = Vec::new();
    preferences::open(page);
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        preferences::show_all(ctx, cx, settings, &mut actions);
    });
    assert!(preferences::is_open());
}

// ----- Preferences: every page round-trips -----

type PageEdit = (Page, Box<dyn Fn(&mut PrefsFile)>);

/// What each page changes, in the general or the page preferences.
fn page_edits() -> Vec<PageEdit> {
    vec![
        (
            Page::Appearance,
            Box::new(|f| {
                f.pages.appearance.background = Some([10, 20, 30]);
                f.pages.appearance.grid = Some([40, 50, 60]);
                f.pages.appearance.text = Some([70, 80, 90]);
                f.pages.appearance.temp_dim = Some([1, 2, 3]);
                f.pages.appearance.icon_size = IconSize::Large;
                f.general.icon_halo = true;
            }),
        ),
        (
            Page::Colors,
            Box::new(|f| f.general.selection_color = Some([200, 100, 50])),
        ),
        (Page::Fonts, Box::new(|f| f.general.text_size_pct = 125)),
        (
            Page::Library,
            Box::new(|f| {
                f.pages.library_browser.preview_size = PreviewSize::Large;
                f.pages.library_browser.search_keywords = false;
                f.pages.library_browser.whole_words = true;
                f.pages.library_browser.match_all_words = false;
            }),
        ),
        (
            Page::Render,
            Box::new(|f| {
                f.pages.render.preview_quality = PreviewQuality::High;
                f.pages.render.shadows = false;
                f.pages.render.ambient_occlusion = false;
                f.general.render_samples = 256;
            }),
        ),
        (
            Page::MaterialsList,
            Box::new(|f| {
                f.pages.materials.apply_waste = false;
                f.pages.materials.round_up = false;
                f.pages.materials.show_prices = false;
                f.pages.materials.all_floors = true;
            }),
        ),
        (
            Page::ResetOptions,
            Box::new(|f| f.pages.dont_ask_again = vec!["delete.floor".into()]),
        ),
        (
            Page::Folders,
            Box::new(|f| {
                f.pages.folders.set(FolderKind::Textures, "/data/tex");
                f.pages.folders.set(FolderKind::Backdrops, "/data/back");
                f.pages.folders.set(FolderKind::Templates, "/data/tpl");
                f.pages.folders.set(FolderKind::Autosave, "/data/auto");
                f.pages.folders.set(FolderKind::UserLibrary, "/data/user");
            }),
        ),
        (
            Page::Edit,
            Box::new(|f| {
                f.pages.edit.rotate_about = RotateAbout::Pointer;
                f.pages.edit.resize_about = ResizeAbout::Center;
                f.pages.edit.set_marquee_mode(MarqueeMode::Touching);
            }),
        ),
        (
            Page::Behaviors,
            Box::new(|f| {
                f.pages.editing.behavior.mode = EditBehavior::Replicate;
                f.pages.editing.behavior.replicate_dialog = true;
                f.pages.editing.behavior.replicate_copies = 4;
                f.pages.editing_saved = true;
                f.pages.behaviors.camera_move_in = 36.0;
                f.pages.behaviors.camera_turn_deg = 10.0;
                f.pages.behaviors.camera_tilt_deg = 2.5;
            }),
        ),
        (
            Page::Snaps,
            Box::new(|f| {
                f.pages.editing.snap_extension = true;
                f.pages.editing.snap_tangent = false;
                f.pages.editing.snap_distance_px = 14.0;
                f.pages.editing.snap_angles = vec![0.0, 45.0, 90.0];
                f.pages.editing_saved = true;
            }),
        ),
        (
            Page::Architectural,
            Box::new(|f| {
                f.pages.architectural.auto_rebuild_roofs = false;
                f.pages.architectural.auto_rebuild_walls = false;
                f.pages.architectural.auto_rebuild_foundations = false;
                f.pages.architectural.auto_rebuild_attic_walls = false;
                f.pages.architectural.delete_unused_roof_planes = true;
                f.general.fit_gap_tolerance = 3.0;
            }),
        ),
        (
            Page::Cad,
            Box::new(|f| {
                f.pages.cad.show_arc_centers = false;
                f.pages.cad.end_caps = EndCap::Round;
                f.pages
                    .cad
                    .line_weights
                    .insert("Layout Edge Line Weight".into(), 24);
            }),
        ),
        // General Plan Defaults is a link; nothing is kept on the page.
        (Page::PlanDefaults, Box::new(|_| {})),
        (
            Page::UnitConversions,
            Box::new(|f| {
                f.pages.units.input_unit = LengthUnit::Millimeters;
                f.pages.units.fraction_denominator = 32;
                f.pages.units.decimals = 3;
            }),
        ),
    ]
}

#[test]
fn every_page_is_covered_and_round_trips_through_preferences_json() {
    fresh();
    let edits = page_edits();
    assert_eq!(edits.len(), Page::ALL.len(), "a case for every page");
    for page in Page::ALL {
        assert!(edits.iter().any(|(p, _)| *p == page), "{page:?} has a case");
    }
    for (page, edit) in &edits {
        let mut file = PrefsFile::default();
        edit(&mut file);
        if *page != Page::PlanDefaults {
            assert_ne!(file, PrefsFile::default(), "{page:?} changes something");
        }
        let path = scratch(&format!("{page:?}.json"));
        pages::write_file_at(&path, &file).unwrap();
        let back = pages::read_file_at(&path).unwrap();
        assert_eq!(back, file, "{page:?} reads back");
        // Applying the read file makes it the live preferences.
        preferences::set(back.general.clone());
        pages::set(back.pages.clone());
        assert_eq!(preferences::current(), file.general, "{page:?} general");
        assert_eq!(pages::current(), file.pages, "{page:?} pages");
        let _ = std::fs::remove_file(&path);
    }
    fresh();
}

#[test]
fn applying_a_file_reaches_the_editor() {
    fresh();
    let mut f = PrefsFile::default();
    f.pages.appearance.icon_size = IconSize::Small;
    f.pages.edit.set_marquee_mode(MarqueeMode::Enclosing);
    f.pages.render.preview_quality = PreviewQuality::Low;
    f.pages.render.shadows = false;
    f.general.icon_halo = true;
    preferences::set(f.general);
    pages::set(f.pages);
    assert_eq!(pages::icon_px(), 16.0);
    assert_eq!(crate::tools::select::marquee_mode(), MarqueeMode::Enclosing);
    assert!(preferences::icon_halo());
    let v = pages::view_settings();
    assert!(!v.shadows && v.quality == plan_view3d::Quality::Low);
    fresh();
    assert_eq!(pages::icon_px(), 20.0);
    assert_eq!(
        crate::tools::select::marquee_mode(),
        MarqueeMode::ByDirection
    );
}

#[test]
fn every_page_draws_with_changed_values() {
    fresh();
    let mut cx = EditorContext::new(crate::plan_defaults::embedded());
    let mut settings = AppSettings::default();
    for (page, edit) in page_edits() {
        let mut f = PrefsFile::default();
        edit(&mut f);
        preferences::set(f.general);
        pages::set(f.pages);
        draw(page, &mut cx, &mut settings);
    }
    fresh();
}

#[test]
fn the_canvas_colors_go_over_the_theme() {
    fresh();
    let mut pal = CanvasTheme::Paper.palette();
    let theme = pal;
    preferences::tint_palette(&mut pal);
    assert_eq!(pal.background, theme.background);
    pages::update(|p| {
        p.appearance.background = Some([1, 2, 3]);
        p.appearance.grid = Some([100, 110, 120]);
        p.appearance.text = Some([9, 9, 9]);
        p.appearance.temp_dim = Some([200, 0, 0]);
    });
    preferences::tint_palette(&mut pal);
    assert_eq!(pal.background, egui::Color32::from_rgb(1, 2, 3));
    assert_eq!(pal.grid_major, egui::Color32::from_rgb(100, 110, 120));
    assert_ne!(pal.grid_minor, pal.grid_major);
    assert_eq!(pal.text, egui::Color32::from_rgb(9, 9, 9));
    assert_eq!(pal.dimension_text, egui::Color32::from_rgb(200, 0, 0));
    fresh();
}

#[test]
fn saved_editing_defaults_reach_a_plan_on_the_first_frame() {
    fresh();
    let mut mine = plan_core::defaults::EditingDefaults {
        snap_distance_px: 7.0,
        snap_extension: true,
        ..Default::default()
    };
    mine.behavior.mode = EditBehavior::Concentric;
    pages::remember_editing(&mine);
    let mut cx = EditorContext::new(crate::plan_defaults::embedded());
    let mut settings = AppSettings::default();
    assert_ne!(cx.defaults.editing, mine);
    draw(Page::Snaps, &mut cx, &mut settings);
    assert_eq!(cx.defaults.editing, mine);
    fresh();
}

#[test]
fn the_snaps_page_keeps_what_its_switches_set() {
    fresh();
    let mut cx = EditorContext::new(crate::plan_defaults::embedded());
    let mut settings = AppSettings::default();
    // Drawing a page changes nothing by itself.
    let before = cx.defaults.editing.clone();
    draw(Page::Snaps, &mut cx, &mut settings);
    draw(Page::Edit, &mut cx, &mut settings);
    draw(Page::Behaviors, &mut cx, &mut settings);
    assert_eq!(cx.defaults.editing, before);
    assert!(!pages::current().editing_saved);
    fresh();
}

#[test]
fn unit_conversions_show_every_unit() {
    let p = pages::UnitPrefs::default();
    let rows = ui::convert("12'-6 1/2\"", &p);
    assert_eq!(rows.len(), 6);
    let get = |label: &str| {
        rows.iter()
            .find(|(l, _)| *l == label)
            .map(|(_, t)| t.clone())
            .unwrap()
    };
    assert_eq!(get("Feet and inches"), "12'-6 1/2\"");
    assert_eq!(get("Millimeters"), "3822.7 mm");
    assert_eq!(get("Inches"), "150 1/2\"");
    let m = ui::convert("3000 mm", &p);
    let metric = m.iter().find(|(l, _)| *l == "Meters").unwrap();
    assert_eq!(metric.1, "3 m");
    let inches = m.iter().find(|(l, _)| *l == "Inches").unwrap();
    assert_eq!(inches.1, "118 1/8\"");
    // A bare number reads in the page's unit.
    let bare = ui::convert(
        "300",
        &pages::UnitPrefs {
            input_unit: LengthUnit::Millimeters,
            ..p.clone()
        },
    );
    assert!(bare
        .iter()
        .any(|(l, t)| *l == "Centimeters" && t == "30 cm"));
    assert!(ui::convert("abc", &p).is_empty());
    assert!(ui::convert("", &p).is_empty());
}

#[test]
fn allowed_angles_parse_and_print() {
    assert_eq!(
        ui::parse_angles("0, 45; 90 135\u{b0} 450 x"),
        vec![0.0, 45.0, 90.0, 135.0]
    );
    assert_eq!(ui::angles_text(&[0.0, 22.5, 90.0]), "0, 22.5, 90");
    assert!(ui::parse_angles("").is_empty());
}

// ----- Reset Options -----

#[test]
fn reset_options_reset_what_they_name() {
    fresh();
    let ctx = egui::Context::default();
    let mut settings = AppSettings::default();

    // Toolbars: a custom row and a renamed one go.
    let mut cfg = ToolbarConfig::daniel_default();
    cfg.view_mut(ViewKind::Plan).add_row("Mine");
    config::set_current_unsaved(cfg);
    assert!(config::current()
        .view(ViewKind::Plan)
        .unwrap()
        .bar("custom-1")
        .is_some());
    let note = ui::run_reset(&ctx, &mut settings, Reset::Toolbars);
    assert!(note.contains("Daniel's Chief set"), "{note}");
    assert_eq!(config::current(), ToolbarConfig::daniel_default());

    // Side windows.
    settings.dock_widths.library = 555.0;
    ui::run_reset(&ctx, &mut settings, Reset::SideWindows);
    assert_eq!(settings.dock_widths, DockWidths::default());

    // "Don't ask again" messages.
    pages::set_dont_ask("a");
    pages::set_dont_ask("b");
    let note = ui::run_reset(&ctx, &mut settings, Reset::DontAskAgain);
    assert!(note.starts_with("2 message"), "{note}");
    assert!(!pages::dont_ask("a"));

    // Dialog sizes: a window's remembered size is dropped.
    let show = |ctx: &egui::Context| {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::Window::new("Remembered")
                .id(egui::Id::new("s39_window"))
                .default_size([321.0, 123.0])
                .show(ctx, |ui| {
                    ui.label("x");
                });
        });
    };
    show(&ctx);
    assert!(ctx
        .memory(|m| m.area_rect(egui::Id::new("s39_window")))
        .is_some());
    let note = ui::run_reset(&ctx, &mut settings, Reset::DialogSizes);
    assert!(note.contains("Dialog sizes"), "{note}");
    assert!(ctx
        .memory(|m| m.area_rect(egui::Id::new("s39_window")))
        .is_none());

    // Every reset has its own label and explanation.
    let labels: std::collections::HashSet<_> = Reset::ALL.iter().map(|r| r.label()).collect();
    assert_eq!(labels.len(), Reset::ALL.len());
    assert!(Reset::ALL.iter().all(|r| !r.what().is_empty()));
    fresh();
}

// ----- hotkeys -----

fn chord(key: Key) -> Chord {
    Chord {
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
        key,
    }
}

fn dialog() -> HotkeyDialog {
    HotkeyDialog::new(&HotkeyMap::defaults())
}

fn with_prefix_clash() -> HotkeyMap {
    let mut map = HotkeyMap::defaults();
    // `D` alone starts `D, H` (Hinged Door).
    assert_eq!(
        map.apply_overrides_json(r#"{"overrides": {"Zoom In": ["D"]}}"#),
        1
    );
    map
}

#[test]
fn the_list_is_grouped_by_menu_and_the_search_narrows_it() {
    let mut d = dialog();
    let all = d.visible_commands().len();
    let grouped = d.grouped_commands();
    let total: usize = grouped.iter().map(|(_, c)| c.len()).sum();
    assert_eq!(total, all, "every command is in a menu");
    // Menus come in the order of Chief's menu bar.
    let positions: Vec<usize> = grouped
        .iter()
        .map(|(m, _)| MENU_ORDER.iter().position(|x| x == m).unwrap())
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
    let menu = |name: &str| {
        let c = d
            .visible_commands()
            .into_iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("{name}"));
        menu_of(c)
    };
    assert_eq!(menu("Hinged Door"), "Build");
    assert_eq!(menu("Cut"), "Edit");
    assert_eq!(menu("Save As"), "File");
    assert_eq!(menu("Quit"), "File");
    assert_eq!(menu("Zoom In"), "View");
    for m in ["File", "Edit", "Build", "CAD", "3D", "View"] {
        assert!(grouped.iter().any(|(g, _)| *g == m), "{m} has commands");
    }
    let tools = grouped
        .iter()
        .find(|(g, _)| *g == "Tools")
        .map_or(0, |(_, c)| c.len());
    assert!(tools * 2 < all, "{tools} of {all} landed in Tools");
    // A search keeps the menus it hits.
    d.set_filter("hinged");
    let hit = d.grouped_commands();
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0].0, "Build");
    assert_eq!(hit[0].1[0].name, "Hinged Door");
    d.set_filter("zzzz");
    assert!(d.grouped_commands().is_empty());
}

#[test]
fn a_clash_is_found_and_resolved_in_favor_of_one_command() {
    let mut d = HotkeyDialog::new(&with_prefix_clash());
    let c = d.conflicts();
    // Zoom In's `D` starts every `D, x` sequence there is.
    assert!(c.len() >= 3, "{c:?}");
    assert!(c
        .iter()
        .all(|c| c.commands.contains(&"Zoom In".to_string())));
    let hinged = c
        .iter()
        .find(|c| c.commands.contains(&"Hinged Door".to_string()))
        .expect("D against D, H");
    assert_eq!(hinged.sequence, "D");
    assert_eq!(hinged.owners.len(), 2);
    let owners = hinged.owners.clone();
    // Keep D for Zoom In: Hinged Door loses `D, H`, the rest still clash.
    assert_eq!(d.resolve(&owners, "Zoom In"), 1);
    assert_eq!(d.conflicts().len(), c.len() - 1);
    assert!(!d.draft_hotkeys("Hinged Door").contains("D, H"));
    assert_eq!(d.draft_hotkeys("Zoom In"), "D");
    // Keep the long sequence instead: Zoom In gives up its D, which was
    // behind every clash.
    let mut d = HotkeyDialog::new(&with_prefix_clash());
    let owners = hinged.owners.clone();
    assert_eq!(d.resolve(&owners, "Hinged Door"), 1);
    assert!(d.conflicts().is_empty(), "{:?}", d.conflicts());
    assert!(d.draft_hotkeys("Hinged Door").starts_with("D, H"));
    assert_eq!(d.draft_hotkeys("Zoom In"), "");
    // Resolve All settles every clash at once.
    let mut d = HotkeyDialog::new(&with_prefix_clash());
    d.set_filter_mode(FilterMode::Conflicts);
    assert!(d.visible_commands().len() > 3);
    assert!(d.resolve_all() >= 1);
    assert!(d.conflicts().is_empty());
    assert!(d.visible_commands().is_empty());
}

#[test]
fn keys_that_fold_together_without_a_command_key_are_listed() {
    let mut map = HotkeyMap::defaults();
    let base = map.folded_collisions();
    if cfg!(target_os = "macos") {
        // Daniel's Control+Z (Down One Floor) and Command+Z (Undo) are two
        // keys on the Mac and one key elsewhere; the list says so.
        let undo = base
            .iter()
            .find(|c| c.commands.contains(&"Undo".to_string()))
            .unwrap_or_else(|| panic!("{base:?}"));
        assert!(undo.commands.contains(&"Down One Floor".to_string()));
        assert_eq!(undo.sequence, "Ctrl+Z");
        assert_eq!(undo.as_typed().len(), 2);
    } else {
        assert!(base.is_empty(), "{base:?}");
    }
    let control = Chord {
        ctrl: true,
        ..chord(Key::F12)
    };
    let command = Chord {
        meta: true,
        ..chord(Key::F12)
    };
    assert!(map.assign("Zoom In", vec![control], false).is_ok());
    let second = map.assign("Zoom Out", vec![command], false);
    if cfg!(target_os = "macos") {
        // Two different keys here...
        assert!(second.is_ok(), "{second:?}");
        assert!(find_conflicts(&map).is_empty());
        // ...and one key where Control and Command are the same.
        let folded = map.folded_collisions();
        assert_eq!(folded.len(), base.len() + 1, "{folded:?}");
        let ours = folded
            .iter()
            .find(|c| c.sequence == "Ctrl+F12")
            .expect("the new pair");
        assert_eq!(
            ours.commands,
            vec!["Zoom In".to_string(), "Zoom Out".to_string()]
        );
        // Resolving keeps one of them.
        let mut d = HotkeyDialog::new(&map);
        let owners = ours.owners.clone();
        d.resolve(&owners, "Zoom In");
        assert_eq!(d.folded_collisions().len(), base.len());
    } else {
        // There the second assignment is already an ordinary clash.
        assert_eq!(second, Err(vec!["Zoom In".to_string()]));
        assert!(map.folded_collisions().is_empty());
    }
}

#[test]
fn reset_goes_to_daniels_file_or_to_chiefs_defaults() {
    let mut d = dialog();
    assert!(d.is_default());
    let daniel: Vec<(String, String)> = d
        .visible_commands()
        .iter()
        .map(|c| (c.name.clone(), d.draft_hotkeys(&c.name)))
        .collect();
    d.reset_to_chief();
    assert!(!d.is_default());
    let chief: Vec<(String, String)> = d
        .visible_commands()
        .iter()
        .map(|c| (c.name.clone(), d.draft_hotkeys(&c.name)))
        .collect();
    let differ = daniel.iter().zip(&chief).filter(|(a, b)| a != b).count();
    assert!(differ >= 10, "Daniel's file changes {differ} commands");
    // Chief's defaults are still a clean map with keys on it.
    assert!(d.conflicts().is_empty(), "{:?}", d.conflicts());
    assert!(chief.iter().filter(|(_, k)| !k.is_empty()).count() > 30);
    // Reset Hotkeys brings Daniel's file back; so does an edit and a reset.
    d.reset();
    assert!(d.is_default());
    let back: Vec<(String, String)> = d
        .visible_commands()
        .iter()
        .map(|c| (c.name.clone(), d.draft_hotkeys(&c.name)))
        .collect();
    assert_eq!(back, daniel);
}

#[test]
fn the_keys_export_as_chiefs_user_hotkeys_xml_and_import_back() {
    let mut d = dialog();
    d.select("Zoom Out");
    // One key on Zoom Out: Chief keeps one sequence per command.
    d.choose(0);
    while d.remove_chosen() {
        d.choose(0);
    }
    d.record(chord(Key::F11));
    assert!(matches!(
        d.assign(true),
        crate::dialogs::hotkeys::AssignResult::Assigned
    ));
    assert_eq!(d.draft_hotkeys("Zoom Out"), "F11");

    let (xml, stats) = d.export_chief_xml();
    assert!(xml.starts_with("<?xml"));
    assert!(xml.contains("<UserHotkeys"));
    assert!(xml.contains("Chief Architect Premier X18"));
    assert!(stats.commands > 100, "{stats:?}");
    assert!(stats.summary().starts_with("Wrote"));

    // It is a file Chief's own reader (ours) takes: every command is listed.
    let file = plan_config::parse_hotkeys_xml(&xml).unwrap();
    let daniel = plan_config::load_daniel_config().hotkeys;
    assert_eq!(file.command_ids, daniel.command_ids);
    assert_eq!(file.total_commands, daniel.total_commands);
    assert!(file.bindings.len() >= 100);

    // Another dialog reading it gets the same keys on the commands it names.
    let mut other = dialog();
    let r = other.import_chief_xml(&xml).unwrap();
    assert!(r.commands >= 100, "{r:?}");
    assert_eq!(other.draft_hotkeys("Zoom Out"), "F11");
    for name in ["Hinged Door", "Zoom In", "Cut"] {
        let first = |dlg: &HotkeyDialog| {
            dlg.draft_hotkeys(name)
                .split("; ")
                .next()
                .unwrap_or_default()
                .to_string()
        };
        assert_eq!(first(&other), first(&d), "{name}");
    }
    // Commands Plan Studio does not have keep the key Daniel's file gives.
    let kept = file
        .bindings
        .iter()
        .filter(|b| daniel.bindings.iter().any(|x| x.command_id == b.command_id))
        .count();
    assert!(kept >= 100);
}

#[test]
fn the_hotkeys_window_draws_grouped_and_flat_with_a_folded_clash() {
    let mut d = HotkeyDialog::new(&with_prefix_clash());
    d.select("Zoom In");
    let ctx = egui::Context::default();
    for _ in 0..2 {
        let mut outcome = crate::dialogs::Outcome::Open;
        let _ = ctx.run(egui::RawInput::default(), |ctx| outcome = d.show(ctx));
        assert_eq!(outcome, crate::dialogs::Outcome::Open);
    }
}

// ----- toolbars -----

#[test]
fn toolbar_rows_are_renamed_ordered_and_copied() {
    let mut d = ToolbarDialog::new(&ToolbarConfig::daniel_default(), ViewKind::Plan);
    let a = d.add_row("Alpha").unwrap();
    let b = d.add_row("Beta").unwrap();
    d.select_bar(&a);
    d.toggle("Points");
    d.toggle("Lines");
    d.pick(0);
    d.add_separator();
    let ids =
        |d: &ToolbarDialog| -> Vec<String> { d.set().bars.iter().map(|b| b.id.clone()).collect() };
    assert_eq!(&ids(&d)[3..], &[a.clone(), b.clone()]);

    // Order: Beta up puts it before Alpha; the standard rows stay first.
    d.select_bar(&b);
    assert!(d.move_row(false));
    assert_eq!(&ids(&d)[3..], &[b.clone(), a.clone()]);
    assert!(!d.move_row(false), "already first");
    d.select_bar(&a);
    assert!(!d.move_row(true), "already last");
    d.select_bar(config::ROW1);
    assert!(!d.move_row(true), "a standard row does not move");

    // Rename.
    d.select_bar(&b);
    assert!(d.rename_row("  Bee  "));
    assert_eq!(d.bar().name, "Bee");
    assert!(!d.rename_row("   "));
    d.select_bar(config::ROW2);
    assert!(d.rename_row("Build and Draw"));

    // Duplicate carries the buttons and separators.
    d.select_bar(&a);
    let copy = d.duplicate_row().unwrap();
    assert_eq!(d.bar().id, copy);
    assert_eq!(d.bar().name, "Alpha copy");
    let original = d.set().bar(&a).unwrap().items.clone();
    assert_eq!(d.bar().items, original);
    assert!(original.contains(&"Points".to_string()));
    assert!(original.contains(&config::SEPARATOR.to_string()));

    // It saves and loads as JSON, order and names included.
    let json = d.export_json();
    let mut again = ToolbarDialog::new(&ToolbarConfig::daniel_default(), ViewKind::Plan);
    again.import_json(&json).unwrap();
    assert_eq!(ids(&again), ids(&d));
    assert_eq!(again.set().bar(&b).unwrap().name, "Bee");

    // Locked: every row edit is refused.
    d.set_locked(true);
    assert!(!d.rename_row("No"));
    assert!(!d.move_row(true));
    assert!(d.duplicate_row().is_none());
}

#[test]
fn a_committed_toolbar_set_is_live_until_reset_toolbars() {
    fresh();
    let mut d = ToolbarDialog::new(&ToolbarConfig::daniel_default(), ViewKind::Plan);
    let id = d.add_row("Mine").unwrap();
    d.toggle("Points");
    d.commit().unwrap();
    let live = config::current();
    let bar = live.view(ViewKind::Plan).unwrap().bar(&id).unwrap();
    assert_eq!(bar.name, "Mine");
    assert!(bar.contains("Points"));
    config::reset_all_live().unwrap();
    assert_eq!(config::current(), ToolbarConfig::daniel_default());
    // A lock is kept through the reset, as in the dialog.
    let mut locked = config::current();
    locked.locked = true;
    config::set_current_unsaved(locked);
    config::reset_all_live().unwrap();
    assert!(config::current().locked);
    config::set_current_unsaved(ToolbarConfig::daniel_default());
}

#[test]
fn a_chief_toolbar_file_still_imports_into_the_dialog() {
    const FILE: &str =
        include_str!("../../../../docs/chief-config-raw/Default Configuration.toolbar");
    let mut d = ToolbarDialog::new(&ToolbarConfig::daniel_default(), ViewKind::Plan);
    let report = d.import_chief_text(FILE).unwrap();
    assert!(report.total > 20, "{}", report.summary());
    assert!(d.import_chief_text("not a toolbar file").is_err());
}
