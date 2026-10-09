//! File > Import > Import Settings from Plan/Layout and the Legacy Settings
//! Import (manual pp. 114 to 118).
//!
//! * **Import Default Settings**: categories (Multiple Saved Defaults,
//!   Default Settings, Layer Sets, Note Types, Saved Plan Views, Wall Types)
//!   with their items; a category box checks all its items, is solid when
//!   some are checked; Replace Originals with Imported or Rename Imported;
//!   OK imports once, as one undo step.
//! * **Legacy**: Import Layer Sets (`.layers`), Import Default Sets
//!   (`.cadefs`), Import Wall Definitions (`.dat`), Import Note Types
//!   (`.json`).
//!
//! The sources are Plan Studio plans (`.psplan`), plan templates, defaults
//! files, and Chief `.plan` / `.layout` files, which are read by name and the
//! values the decoder knows (wall types, dimension sets, text styles). The
//! legacy files are Plan Studio's JSON exchange files; a Chief binary file is
//! read by the names in it (layer sets) and otherwise refused with a message.

use crate::editor::EditorContext;
use eframe::egui::{self, Align, Align2, Layout};
use plan_core::defaults::import::{
    self, items, Clash, DefaultSetsFile, ImportCategory, ImportItem, ImportSource, LayerFile,
    WallFile, DEFAULT_SETS_GROUP,
};
use plan_core::defaults::saved::SavedKind;
use plan_core::defaults::PlanDefaults;
use plan_core::Project;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;

/// File > Import > Import Settings from Plan/Layout.
pub const IMPORT_SETTINGS: &str = "import.settings";
/// File > Import > Legacy Settings Import > Import Layer Sets.
pub const LEGACY_LAYERS: &str = "import.legacy_layers";
/// ... > Import Default Sets.
pub const LEGACY_DEFAULT_SETS: &str = "import.legacy_default_sets";
/// ... > Import Wall Definitions.
pub const LEGACY_WALLS: &str = "import.legacy_walls";
/// ... > Import Note Types.
pub const LEGACY_NOTES: &str = "import.legacy_notes";

/// Runs a command by id; false when it is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        IMPORT_SETTINGS => {
            let Some(path) = rfd::FileDialog::new()
                .add_filter(
                    "Plan, layout or template",
                    &["psplan", "pstemplate", "plan", "layout", "json"],
                )
                .pick_file()
            else {
                return true;
            };
            match load_source(cx, &path) {
                Ok(src) => open_source(src, &file_label(&path)),
                Err(e) => cx.status = format!("Could not read {}: {e}", file_label(&path)),
            }
        }
        LEGACY_LAYERS | LEGACY_DEFAULT_SETS | LEGACY_WALLS | LEGACY_NOTES => {
            let (label, ext): (&str, &[&str]) = match id {
                LEGACY_LAYERS => ("Layer sets", &["layers", "json"]),
                LEGACY_DEFAULT_SETS => ("Default sets", &["cadefs", "json"]),
                LEGACY_WALLS => ("Wall definitions", &["dat", "json"]),
                _ => ("Note types", &["json"]),
            };
            let Some(path) = rfd::FileDialog::new().add_filter(label, ext).pick_file() else {
                return true;
            };
            match open_legacy(cx, id, &path) {
                Ok(()) => {}
                Err(e) => cx.status = format!("Could not import {}: {e}", file_label(&path)),
            }
        }
        _ => return false,
    }
    true
}

fn file_label(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

// ===================================================================
// Sources
// ===================================================================

/// Reads the plan, layout, template or defaults file at `path` as a source.
/// Settings only come from files in the units of the open plan.
pub fn load_source(cx: &EditorContext, path: &Path) -> Result<ImportSource, String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let src = match ext.as_str() {
        "psplan" => ImportSource::from_project(plan_core::io::load_project(path)?),
        "pstemplate" => {
            let t = crate::templates::load_plan_template(path)?;
            ImportSource {
                project: t.project,
                defaults: Some(t.defaults),
                layout: false,
                imperial: t.imperial,
            }
        }
        "plan" | "layout" | "tpl" => chief_source(path, ext == "layout")?,
        _ => {
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            if let Ok(d) = PlanDefaults::from_json(&text) {
                // A defaults file (Save Current Defaults as My Template).
                let project = Project::from_defaults("Imported", &d);
                ImportSource {
                    imperial: d.units.imperial,
                    project,
                    defaults: Some(d),
                    layout: false,
                }
            } else {
                ImportSource::from_project(Project::from_json(&text).map_err(|e| e.to_string())?)
            }
        }
    };
    if src.imperial != cx.defaults.units.imperial {
        return Err("Settings can only be imported from files that use the same units".into());
    }
    Ok(src)
}

/// A Chief plan or layout as a source: what the decoder reads (layer set,
/// plan view and Rich Text default names; wall types; dimension sets; text
/// styles) laid over the installed defaults.
fn chief_source(path: &Path, layout: bool) -> Result<ImportSource, String> {
    let seed = crate::templates::decode_plan_file(path)?;
    let d = crate::templates::overlay(crate::plan_defaults::embedded(), &seed);
    let mut project = Project::from_defaults("Chief", &d);
    project.wall_types = d.wall_types.clone();
    Ok(ImportSource {
        project,
        defaults: Some(d),
        layout,
        imperial: true,
    })
}

// ===================================================================
// The Import Default Settings window
// ===================================================================

struct Window {
    src: ImportSource,
    from: String,
    items: Vec<ImportItem>,
    picked: BTreeSet<ImportItem>,
    clash: Clash,
    open: BTreeSet<ImportCategory>,
    message: String,
}

/// A legacy import window.
enum Legacy {
    Layers {
        file: LayerFile,
        picked: BTreeSet<String>,
        clash: Clash,
    },
    DefaultSets {
        file: Box<DefaultSetsFile>,
        overwrite: bool,
        clash: Clash,
    },
    Walls {
        file: WallFile,
        replace: bool,
    },
}

#[derive(Default)]
struct State {
    window: Option<Window>,
    legacy: Option<Legacy>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Opens the Import Default Settings window on a source.
pub fn open_source(src: ImportSource, from: &str) {
    let list = items(&src);
    state(|s| {
        s.window = Some(Window {
            src,
            from: from.to_string(),
            items: list,
            picked: BTreeSet::new(),
            clash: Clash::Rename,
            open: BTreeSet::new(),
            message: String::new(),
        })
    });
}

/// Is the Import Default Settings window open?
pub fn is_open() -> bool {
    state(|s| s.window.is_some())
}

/// The items the open window offers.
#[cfg(test)]
pub fn offered() -> Vec<ImportItem> {
    state(|s| {
        s.window
            .as_ref()
            .map(|w| w.items.clone())
            .unwrap_or_default()
    })
}

/// Checks or clears items in the open window (what clicking the boxes does).
#[cfg(test)]
pub fn pick(items: &[ImportItem], on: bool) {
    state(|s| {
        if let Some(w) = s.window.as_mut() {
            for i in items {
                if on {
                    w.picked.insert(i.clone());
                } else {
                    w.picked.remove(i);
                }
            }
        }
    });
}

/// Sets how name clashes are treated in the open window.
#[cfg(test)]
pub fn set_clash(c: Clash) {
    state(|s| {
        if let Some(w) = s.window.as_mut() {
            w.clash = c;
        }
    });
}

/// OK in the open window: imports the checked items as one undo step and
/// closes it. Returns the status line.
pub fn accept(cx: &mut EditorContext) -> Option<String> {
    let w = state(|s| s.window.take())?;
    Some(run_import(cx, &w.src, &w.picked, w.clash))
}

fn run_import(
    cx: &mut EditorContext,
    src: &ImportSource,
    picked: &BTreeSet<ImportItem>,
    clash: Clash,
) -> String {
    if picked.is_empty() {
        return "Nothing was checked to import".into();
    }
    cx.begin_change("Import Settings");
    cx.project.saved_commit_all(&mut cx.defaults);
    let rep = cx
        .project
        .import_settings(&mut cx.defaults, src, picked, clash);
    if rep.total() == 0 {
        cx.cancel_change();
        return "Nothing was imported".into();
    }
    cx.refresh_layer_view();
    cx.mark_dirty();
    rep.summary()
}

/// The state of a category's check box.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    None,
    Some,
    All,
}

/// Is the category `cat` of `items` fully, partly or not checked in `picked`?
pub fn category_check(
    items: &[ImportItem],
    picked: &BTreeSet<ImportItem>,
    cat: ImportCategory,
) -> Check {
    let all: Vec<&ImportItem> = items.iter().filter(|i| i.category == cat).collect();
    let n = all.iter().filter(|i| picked.contains(*i)).count();
    match n {
        0 => Check::None,
        n if n == all.len() => Check::All,
        _ => Check::Some,
    }
}

fn group_label(item: &ImportItem) -> String {
    match item.category {
        ImportCategory::SavedDefaults if item.group == DEFAULT_SETS_GROUP => "Default Sets".into(),
        ImportCategory::SavedDefaults => SavedKind::from_id(&item.group)
            .map_or_else(|| item.group.clone(), |k| k.label().to_string()),
        _ => String::new(),
    }
}

fn show_window(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut w) = state(|s| s.window.take()) else {
        return;
    };
    let mut ok = false;
    let mut cancel = false;
    let mut open = true;
    egui::Window::new("Import Default Settings")
        .id(egui::Id::new("import_default_settings"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([420.0, 460.0])
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(format!("Importing from {}", w.from));
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("import_tree")
                .max_height(300.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for cat in ImportCategory::ALL {
                        let in_cat: Vec<ImportItem> = w
                            .items
                            .iter()
                            .filter(|i| i.category == cat)
                            .cloned()
                            .collect();
                        if in_cat.is_empty() {
                            continue;
                        }
                        let check = category_check(&w.items, &w.picked, cat);
                        ui.horizontal(|ui| {
                            let expanded = w.open.contains(&cat);
                            if ui
                                .small_button(if expanded { "\u{25BE}" } else { "\u{25B8}" })
                                .clicked()
                            {
                                if expanded {
                                    w.open.remove(&cat);
                                } else {
                                    w.open.insert(cat);
                                }
                            }
                            let mut on = check == Check::All;
                            let label = egui::RichText::new(cat.label());
                            let label = if check == Check::Some {
                                label.italics()
                            } else {
                                label
                            };
                            if ui.checkbox(&mut on, label).changed() {
                                for i in &in_cat {
                                    if on {
                                        w.picked.insert(i.clone());
                                    } else {
                                        w.picked.remove(i);
                                    }
                                }
                            }
                            ui.weak(format!("({})", in_cat.len()));
                        });
                        if w.open.contains(&cat) {
                            let mut last_group = String::new();
                            for i in &in_cat {
                                let g = group_label(i);
                                if g != last_group {
                                    if !g.is_empty() {
                                        ui.horizontal(|ui| {
                                            ui.add_space(28.0);
                                            ui.strong(&g);
                                        });
                                    }
                                    last_group = g;
                                }
                                ui.horizontal(|ui| {
                                    ui.add_space(40.0);
                                    let mut on = w.picked.contains(i);
                                    if ui.checkbox(&mut on, &i.name).changed() {
                                        if on {
                                            w.picked.insert(i.clone());
                                        } else {
                                            w.picked.remove(i);
                                        }
                                    }
                                });
                            }
                        }
                    }
                    if w.items.is_empty() {
                        ui.weak("This file has no settings to import.");
                    }
                });
            ui.add_space(4.0);
            ui.strong("Name Conflicts");
            ui.radio_value(
                &mut w.clash,
                Clash::Replace,
                "Replace Originals with Imported",
            );
            ui.radio_value(
                &mut w.clash,
                Clash::Rename,
                "Rename Imported (a number 2 is added)",
            );
            if !w.message.is_empty() {
                ui.label(&w.message);
            }
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("   OK   ").clicked() {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if ok {
        cx.status = run_import(cx, &w.src, &w.picked, w.clash);
    } else if open && !cancel {
        state(|s| s.window = Some(w));
    }
}

// ===================================================================
// Legacy imports
// ===================================================================

/// Reads the legacy file for command `id` and opens its window.
pub fn open_legacy(cx: &mut EditorContext, id: &str, path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path);
    match id {
        LEGACY_LAYERS => {
            let file = match text
                .as_ref()
                .ok()
                .and_then(|t| LayerFile::from_json(t).ok())
            {
                Some(f) => f,
                None => chief_layer_names(cx, path)?,
            };
            if file.sets.is_empty() {
                return Err("The file holds no layer sets".into());
            }
            let picked = file.sets.iter().map(|s| s.name.clone()).collect();
            state(|s| {
                s.legacy = Some(Legacy::Layers {
                    file,
                    picked,
                    clash: Clash::Rename,
                })
            });
        }
        LEGACY_DEFAULT_SETS => {
            let file = text
                .map_err(|e| e.to_string())
                .and_then(|t| DefaultSetsFile::from_json(&t).map_err(|e| e.to_string()))
                .map_err(|e| {
                    format!(
                        "{e}. Chief's own .cadefs files are not read; use Import Settings from Plan/Layout on the plan they came from"
                    )
                })?;
            state(|s| {
                s.legacy = Some(Legacy::DefaultSets {
                    file: Box::new(file),
                    overwrite: false,
                    clash: Clash::Rename,
                })
            });
        }
        LEGACY_WALLS => {
            let file = text
                .map_err(|e| e.to_string())
                .and_then(|t| WallFile::from_json(&t).map_err(|e| e.to_string()))
                .map_err(|e| {
                    format!(
                        "{e}. Chief's own wall definition .dat files are not read; use Import Settings from Plan/Layout on a plan that has the wall types"
                    )
                })?;
            state(|s| {
                s.legacy = Some(Legacy::Walls {
                    file,
                    replace: false,
                })
            });
        }
        _ => {
            let types = import::parse_note_types(&text.map_err(|e| e.to_string())?)?;
            cx.begin_change("Import Note Types");
            let rep = cx.project.import_note_types(&types, Clash::Rename);
            cx.mark_dirty();
            cx.status = format!("Imported {} note types", rep.total());
        }
    }
    Ok(())
}

/// A Chief `.layers` file: the layer set names the decoder finds, each a
/// copy of the active set (the values behind the names are not read).
fn chief_layer_names(cx: &EditorContext, path: &Path) -> Result<LayerFile, String> {
    let inv = plan_chiefplan::build_inventory(path).map_err(|e| e.to_string())?;
    let base = cx
        .project
        .layer_sets
        .active_set()
        .cloned()
        .ok_or("No active layer set")?;
    let sets = inv
        .layer_sets
        .iter()
        .map(|e| {
            let mut s = base.clone();
            s.name = e.name.clone();
            s
        })
        .collect();
    Ok(LayerFile {
        sets,
        layers: cx.project.layers.clone(),
    })
}

/// Is a legacy window open?
pub fn legacy_is_open() -> bool {
    state(|s| s.legacy.is_some())
}

/// OK in the open legacy window. Returns the status line.
pub fn accept_legacy(cx: &mut EditorContext) -> Option<String> {
    let l = state(|s| s.legacy.take())?;
    Some(run_legacy(cx, l))
}

fn run_legacy(cx: &mut EditorContext, l: Legacy) -> String {
    cx.begin_change("Import Settings");
    let rep = match l {
        Legacy::Layers {
            file,
            picked,
            clash,
        } => {
            let names: Vec<String> = picked.into_iter().collect();
            cx.project.import_layer_file(&file, &names, clash)
        }
        Legacy::DefaultSets {
            file,
            overwrite,
            clash,
        } => {
            cx.project.saved_commit_all(&mut cx.defaults);
            cx.project
                .import_default_sets_file(&mut cx.defaults, &file, overwrite, clash)
        }
        Legacy::Walls { file, replace } => cx.project.import_wall_file(&file, replace),
    };
    if rep.total() == 0 {
        cx.cancel_change();
        return "Nothing was imported".into();
    }
    cx.refresh_layer_view();
    cx.mark_dirty();
    rep.summary()
}

fn show_legacy(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut l) = state(|s| s.legacy.take()) else {
        return;
    };
    let mut ok = false;
    let mut cancel = false;
    let mut open = true;
    let title = match &l {
        Legacy::Layers { .. } => "Import Layer Sets",
        Legacy::DefaultSets { .. } => "Import Default Sets",
        Legacy::Walls { .. } => "Import Wall Definitions",
    };
    egui::Window::new(title)
        .id(egui::Id::new("legacy_import"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            match &mut l {
                Legacy::Layers {
                    file,
                    picked,
                    clash,
                } => {
                    ui.strong("Layer Sets to Import");
                    egui::ScrollArea::vertical()
                        .id_salt("legacy_layers")
                        .max_height(220.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for s in &file.sets {
                                let mut on = picked.contains(&s.name);
                                if ui.checkbox(&mut on, &s.name).changed() {
                                    if on {
                                        picked.insert(s.name.clone());
                                    } else {
                                        picked.remove(&s.name);
                                    }
                                }
                            }
                        });
                    ui.horizontal(|ui| {
                        if ui.button("Select All").clicked() {
                            *picked = file.sets.iter().map(|s| s.name.clone()).collect();
                        }
                        if ui.button("Clear All").clicked() {
                            picked.clear();
                        }
                    });
                    ui.strong("Existing Layer Sets");
                    ui.radio_value(clash, Clash::Replace, "Replace");
                    ui.radio_value(clash, Clash::Rename, "Create a New Copy");
                }
                Legacy::DefaultSets {
                    file,
                    overwrite,
                    clash,
                } => {
                    ui.label(format!(
                        "{} Default Sets, {} saved defaults, {} layer sets",
                        file.default_sets.len(),
                        file.saved.len(),
                        file.layer_sets.len()
                    ));
                    ui.checkbox(overwrite, "Overwrite Existing Defaults");
                    ui.strong("Duplicate Names");
                    ui.radio_value(clash, Clash::Replace, "Replace Existing Defaults");
                    ui.radio_value(clash, Clash::Rename, "Rename Duplicate Defaults");
                }
                Legacy::Walls { file, replace } => {
                    ui.label(format!("{} wall type definitions", file.wall_types.len()));
                    ui.checkbox(
                        replace,
                        "Replace existing wall definitions with the same name",
                    );
                }
            }
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("   OK   ").clicked() {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if ok {
        cx.status = run_legacy(cx, l);
    } else if open && !cancel {
        state(|s| s.legacy = Some(l));
    }
}

/// Draws the import windows; call once a frame.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    show_window(ctx, cx);
    show_legacy(ctx, cx);
}

#[cfg(test)]
pub(crate) fn reset_state() {
    state(|s| *s = State::default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::defaults::import::ImportCategory as C;

    fn source() -> ImportSource {
        let mut d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("Src", &d);
        p.layer_sets.copy_set("Default Set", "Plot Layer Set");
        p.saved_copy(&mut d, SavedKind::RichText, "Default", "Plot")
            .unwrap();
        p.note_types.add("Roof Note", "R");
        ImportSource {
            project: p,
            defaults: Some(d),
            layout: false,
            imperial: true,
        }
    }

    #[test]
    fn the_category_box_is_solid_when_some_items_are_checked() {
        reset_state();
        let mut cx = crate::editor::plan_tabs::plain_cx();
        open_source(source(), "src.psplan");
        let list = offered();
        let sets: Vec<ImportItem> = list
            .iter()
            .filter(|i| i.category == C::LayerSets)
            .cloned()
            .collect();
        assert!(sets.len() >= 2);
        let picked = |s: &[ImportItem]| s.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(
            category_check(&list, &BTreeSet::new(), C::LayerSets),
            Check::None
        );
        assert_eq!(
            category_check(&list, &picked(&sets[..1]), C::LayerSets),
            Check::Some
        );
        assert_eq!(
            category_check(&list, &picked(&sets), C::LayerSets),
            Check::All
        );
        // Check one set and the Rich Text default; OK imports exactly those.
        let plot = sets
            .iter()
            .find(|i| i.name == "Plot Layer Set")
            .unwrap()
            .clone();
        let rich = list
            .iter()
            .find(|i| i.name == "Plot" && i.group == "rich_text")
            .unwrap()
            .clone();
        pick(&[plot, rich], true);
        set_clash(Clash::Rename);
        let n_sets = cx.project.layer_sets.sets.len();
        let status = accept(&mut cx).unwrap();
        assert!(status.starts_with("Imported 2"), "{status}");
        assert_eq!(cx.project.layer_sets.sets.len(), n_sets + 1);
        assert_eq!(cx.undo_label(), Some("Import Settings"));
        assert!(cx
            .project
            .saved_names(&mut cx.defaults, SavedKind::RichText)
            .contains(&"Plot".to_string()));
        assert!(
            cx.project.note_types.get("Roof Note").is_none(),
            "unchecked: not imported"
        );
        cx.undo();
        assert_eq!(cx.project.layer_sets.sets.len(), n_sets);
        assert!(accept(&mut cx).is_none(), "the window closed");
    }

    #[test]
    fn nothing_checked_changes_nothing_and_makes_no_undo_step() {
        reset_state();
        let mut cx = crate::editor::plan_tabs::plain_cx();
        open_source(source(), "src.psplan");
        let status = accept(&mut cx).unwrap();
        assert!(status.contains("Nothing"));
        assert!(!cx.can_undo());
    }

    #[test]
    fn a_plan_file_in_other_units_is_refused() {
        let mut cx = crate::editor::plan_tabs::plain_cx();
        let dir = std::env::temp_dir().join(format!("plan-studio-imp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut d = PlanDefaults::chief_x18_daniel();
        d.units.imperial = false;
        let path = dir.join("metric.json");
        std::fs::write(&path, d.to_json().unwrap()).unwrap();
        assert!(load_source(&cx, &path).unwrap_err().contains("same units"));
        let mut d2 = PlanDefaults::chief_x18_daniel();
        d2.units.imperial = true;
        let ok = dir.join("us.json");
        std::fs::write(&ok, d2.to_json().unwrap()).unwrap();
        let src = load_source(&cx, &ok).unwrap();
        assert!(src.defaults.is_some());
        let proj_path = dir.join("p.psplan");
        plan_core::io::save_project(&cx.project, &proj_path).unwrap();
        let src = load_source(&cx, &proj_path).unwrap();
        assert!(!items(&src).is_empty());
        cx.status.clear();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_files_import_through_their_windows() {
        reset_state();
        let mut cx = crate::editor::plan_tabs::plain_cx();
        let dir = std::env::temp_dir().join(format!("plan-studio-leg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = source();
        // Layer sets.
        let lf = LayerFile::export(&src.project, &[]);
        let p = dir.join("a.layers");
        std::fs::write(&p, lf.to_json().unwrap()).unwrap();
        open_legacy(&mut cx, LEGACY_LAYERS, &p).unwrap();
        assert!(legacy_is_open());
        let s = accept_legacy(&mut cx).unwrap();
        assert!(s.starts_with("Imported"), "{s}");
        assert!(cx.project.layer_sets.get("Plot Layer Set").is_some());
        // Default sets.
        let mut d = src.defaults.clone().unwrap();
        let mut sp = src.project.clone();
        sp.default_set_save_new(&mut d, "Plot Set").unwrap();
        let df = DefaultSetsFile::export(&sp, &d);
        let p = dir.join("a.cadefs");
        std::fs::write(&p, df.to_json().unwrap()).unwrap();
        open_legacy(&mut cx, LEGACY_DEFAULT_SETS, &p).unwrap();
        accept_legacy(&mut cx).unwrap();
        assert!(cx.project.saved_defaults.set("Plot Set").is_some());
        // Wall definitions.
        let wf = WallFile {
            wall_types: src.project.wall_types.iter().take(2).cloned().collect(),
        };
        let p = dir.join("a.dat");
        std::fs::write(&p, wf.to_json().unwrap()).unwrap();
        open_legacy(&mut cx, LEGACY_WALLS, &p).unwrap();
        let s = accept_legacy(&mut cx).unwrap();
        assert!(s.contains("Nothing"), "same names are kept: {s}");
        // Note types import at once.
        let p = dir.join("n.json");
        std::fs::write(&p, r#"[{"name":"Plumbing Note","prefix":"P"}]"#).unwrap();
        open_legacy(&mut cx, LEGACY_NOTES, &p).unwrap();
        assert!(cx.project.note_types.get("Plumbing Note").is_some());
        // A Chief binary is refused with a message.
        let p = dir.join("chief.cadefs");
        std::fs::write(&p, [0u8, 1, 2, 3]).unwrap();
        assert!(open_legacy(&mut cx, LEGACY_DEFAULT_SETS, &p).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_windows_draw() {
        reset_state();
        let mut cx = crate::editor::plan_tabs::plain_cx();
        open_source(source(), "x");
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show_all(ctx, &mut cx));
        }
        assert!(is_open());
        reset_state();
        assert!(!run_command(&mut cx, "nope"));
    }
}
