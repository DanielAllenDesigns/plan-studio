//! Layer Set Management (Tools > Layer Settings): the plan's layer sets with
//! New, Copy, Rename, Delete, Make Active and Import From Plan File. The same
//! operations serve the set buttons of Layer Display Options.
//!
//! Every operation is one undo step. `run_command` opens the window from its
//! menu id; `show_all` draws it (the shell calls it from `docks::show_dialogs`).

use crate::editor::EditorContext;
use eframe::egui::{self, Align2, Vec2};
use plan_core::layer_sets::{LayerSets, ViewKind, USE_ACTIVE_LAYER_SET};
use plan_core::{LayerSet, Project};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;

/// Menu id: Tools > Layer Settings > Layer Set Management...
pub const OPEN: &str = "layers.set_management";
/// Menu id: Tools > Layer Settings > Active Layers by Tool... (includes the
/// Current CAD Layer, LAY-6).
pub const ACTIVE_LAYERS: &str = "layers.active_by_tool";
/// Menu id: Tools > Layer Settings > Layer Set Defaults... (LAY-70).
pub const DEFAULTS: &str = "layers.set_defaults";

// ----- operations -----

/// New Layer Set: a set whose layers all take the base layers' look, named
/// `name`. Returns the name it got.
pub fn new_set(cx: &mut EditorContext, name: &str) -> Result<String, String> {
    let name = check_name(cx, name)?;
    cx.begin_change("New Layer Set");
    let mut def = LayerSets::from_layers(&cx.project.layers).sets.remove(0);
    def.name = name.clone();
    cx.project.layer_sets.add_set(def);
    cx.mark_dirty();
    Ok(name)
}

/// Copy Layer Set: `from` copied under `name`.
pub fn copy_set(cx: &mut EditorContext, from: &str, name: &str) -> Result<String, String> {
    let name = check_name(cx, name)?;
    if cx.project.layer_sets.get(from).is_none() {
        return Err(format!("There is no layer set \"{from}\""));
    }
    cx.begin_change("Copy Layer Set");
    cx.project.layer_sets.copy_set(from, &name);
    cx.mark_dirty();
    Ok(name)
}

/// Renames a layer set; the plan views that show it follow.
pub fn rename_set(cx: &mut EditorContext, old: &str, name: &str) -> Result<String, String> {
    let name = check_name(cx, name)?;
    if cx.project.layer_sets.get(old).is_none() {
        return Err(format!("There is no layer set \"{old}\""));
    }
    cx.begin_change("Rename Layer Set");
    cx.project.rename_layer_set(old, &name);
    cx.mark_dirty();
    Ok(name)
}

/// Deletes a layer set; the plan views that showed it fall back to the active
/// set. The last set cannot be deleted.
pub fn delete_set(cx: &mut EditorContext, name: &str) -> Result<(), String> {
    if cx.project.layer_sets.get(name).is_none() {
        return Err(format!("There is no layer set \"{name}\""));
    }
    if cx.project.layer_sets.sets.len() <= 1 {
        return Err("The last layer set cannot be deleted".into());
    }
    cx.begin_change("Delete Layer Set");
    cx.project.delete_layer_set(name);
    cx.mark_dirty();
    Ok(())
}

/// Makes `name` the active layer set (and the set of the active plan view).
pub fn activate_set(cx: &mut EditorContext, name: &str) -> bool {
    if cx.project.layer_sets.get(name).is_none() || cx.project.shown_layer_set() == name {
        return false;
    }
    cx.begin_change("Active Layer Set");
    cx.project.show_layer_set(name);
    cx.mark_dirty();
    true
}

/// A name that is free in this plan's layer sets.
fn check_name(cx: &EditorContext, name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Type a name for the layer set".into());
    }
    if cx.project.layer_sets.get(name).is_some() {
        return Err(format!("A layer set named \"{name}\" exists already"));
    }
    Ok(name.to_string())
}

/// Import From Plan File: brings the sets `names` of `other` into the plan,
/// with the layers they need. Returns the names they got here.
pub fn import_sets(
    cx: &mut EditorContext,
    other: &Project,
    names: &[String],
) -> Result<Vec<String>, String> {
    if names.is_empty() {
        return Err("Pick the layer sets to import".into());
    }
    cx.begin_change("Import Layer Sets");
    let mut layers: LayerSet = cx.project.layers.clone();
    let added =
        cx.project
            .layer_sets
            .import_sets(&other.layer_sets, &other.layers, names, &mut layers);
    cx.project.layers = layers;
    cx.project.ensure_opening_label_layers();
    cx.mark_dirty();
    if added.is_empty() {
        return Err("None of those layer sets were found".into());
    }
    Ok(added)
}

/// Loads a plan file for [`import_sets`].
pub fn load_plan(path: &Path) -> Result<Project, String> {
    plan_core::io::load_project(path)
}

// ----- the window -----

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    None,
    New,
    Copy,
    Rename,
}

/// A plan file whose layer sets can be imported.
struct Source {
    label: String,
    project: Project,
    picks: BTreeSet<String>,
}

#[derive(Default)]
struct State {
    open: bool,
    tools_open: bool,
    defaults_open: bool,
    selected: Option<String>,
    mode: Mode,
    name: String,
    message: String,
    source: Option<Source>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Opens the window.
pub fn open() {
    state(|s| s.open = true);
}

/// Is the window open?
pub fn is_open() -> bool {
    state(|s| s.open)
}

/// Opens the Active Layers by Tool window.
pub fn open_tools() {
    state(|s| s.tools_open = true);
}

/// Runs a menu command by id; false when the id is not ours.
pub fn run_command(_cx: &mut EditorContext, id: &str) -> bool {
    match id {
        OPEN => open(),
        ACTIVE_LAYERS => open_tools(),
        DEFAULTS => state(|s| s.defaults_open = true),
        _ => return false,
    }
    true
}

/// Chooses the layer `tool` (a `plan_core::layers::TOOL_LAYERS` key) draws
/// on, as one undo step. `false` when the tool or layer is unknown or the
/// choice does not change.
pub fn set_tool_layer(cx: &mut EditorContext, tool: &str, layer: &str) -> bool {
    if cx.project.layers.tool_layer(tool) == layer {
        return false;
    }
    let mut probe = cx.project.layers.clone();
    if !probe.set_tool_layer(tool, layer) {
        return false;
    }
    cx.begin_change("Active Layer");
    cx.project.layers.set_tool_layer(tool, layer);
    cx.mark_dirty();
    true
}

/// Puts every tool back on its default layer (one undo step).
pub fn reset_tool_layers(cx: &mut EditorContext) -> bool {
    if cx.project.layers.tool_layers.is_empty() {
        return false;
    }
    cx.begin_change("Reset Active Layers");
    cx.project.layers.reset_tool_layers();
    cx.mark_dirty();
    true
}

/// Layer Set Defaults: chooses the layer set a new view of `kind` starts
/// with (`None` or "Use Active Layer Set" follows the active set). One undo
/// step; `false` when nothing changes or the set is unknown.
pub fn set_view_default(cx: &mut EditorContext, kind: ViewKind, set: Option<&str>) -> bool {
    let set = set.filter(|s| *s != USE_ACTIVE_LAYER_SET);
    if set.is_some_and(|s| cx.project.layer_sets.get(s).is_none()) {
        return false;
    }
    if cx.project.layer_set_defaults.choice(kind) == set {
        return false;
    }
    cx.begin_change("Layer Set Defaults");
    cx.project.layer_set_defaults.set(kind, set);
    cx.mark_dirty();
    true
}

fn show_defaults(ctx: &egui::Context, cx: &mut EditorContext) {
    if !state(|s| s.defaults_open) {
        return;
    }
    let mut open = true;
    let names: Vec<String> = cx
        .project
        .layer_sets
        .names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut pick: Option<(ViewKind, Option<String>)> = None;
    let mut define: Option<String> = None;
    egui::Window::new("Layer Set Defaults")
        .id(egui::Id::new("layer_set_defaults"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label("The layer set a new view of each kind starts with.");
            egui::Grid::new("layer_set_defaults_grid")
                .num_columns(3)
                .spacing(Vec2::new(10.0, 4.0))
                .show(ui, |ui| {
                    for kind in ViewKind::ALL {
                        ui.label(kind.label());
                        let current = cx.project.layer_set_defaults.choice(kind);
                        egui::ComboBox::from_id_salt(("layer_set_default", kind))
                            .selected_text(current.unwrap_or(USE_ACTIVE_LAYER_SET))
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(current.is_none(), USE_ACTIVE_LAYER_SET)
                                    .clicked()
                                {
                                    pick = Some((kind, None));
                                }
                                for n in &names {
                                    if ui.selectable_label(current == Some(n), n).clicked() {
                                        pick = Some((kind, Some(n.clone())));
                                    }
                                }
                            });
                        let target = cx.project.initial_layer_set(kind);
                        if ui
                            .small_button("Define\u{2026}")
                            .on_hover_text("Open the Layer Set Management window on this set")
                            .clicked()
                        {
                            define = Some(target);
                        }
                        ui.end_row();
                    }
                });
        });
    if let Some((kind, set)) = pick {
        set_view_default(cx, kind, set.as_deref());
    }
    if let Some(set) = define {
        state(|s| {
            s.selected = Some(set);
            s.open = true;
        });
    }
    if !open {
        state(|s| s.defaults_open = false);
    }
}

/// Draws the windows that are open.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    show_tools(ctx, cx);
    show_defaults(ctx, cx);
    super::object_layers::show(ctx, cx);
    super::layer_display::show_define(ctx, cx);
    if !is_open() {
        return;
    }
    let mut open = true;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        egui::Window::new("Layer Set Management")
            .id(egui::Id::new("layer_set_management"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(520.0, 420.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| body(ui, cx, &mut st));
    });
    if !open {
        state(|s| s.open = false);
    }
}

fn body(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut State) {
    let names: Vec<String> = cx
        .project
        .layer_sets
        .names()
        .into_iter()
        .map(str::to_string)
        .collect();
    if st
        .selected
        .as_ref()
        .is_none_or(|s| !names.iter().any(|n| n == s))
    {
        st.selected = Some(cx.project.layer_sets.active.clone());
    }
    ui.label(format!(
        "{} layer sets \u{2013} the active set is {}",
        names.len(),
        cx.project.layer_sets.active
    ));
    egui::ScrollArea::vertical()
        .id_salt("layer_set_list")
        .max_height(220.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for n in &names {
                let active = *n == cx.project.layer_sets.active;
                let shown = if active {
                    format!("{n}  (active)")
                } else {
                    n.clone()
                };
                let on = st.selected.as_deref() == Some(n.as_str());
                let r = ui.selectable_label(on, shown);
                if r.clicked() {
                    st.selected = Some(n.clone());
                }
                if r.double_clicked() {
                    activate_set(cx, n);
                }
            }
        });
    ui.separator();
    let sel = st.selected.clone().unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        if ui.button("Make Active").clicked() {
            activate_set(cx, &sel);
        }
        if ui.button("New\u{2026}").clicked() {
            st.mode = Mode::New;
            st.name = cx.project.layer_sets.free_name("New Layer Set");
        }
        if ui.button("Copy\u{2026}").clicked() {
            st.mode = Mode::Copy;
            st.name = cx.project.layer_sets.free_name(&format!("{sel} Copy"));
        }
        if ui.button("Rename\u{2026}").clicked() {
            st.mode = Mode::Rename;
            st.name = sel.clone();
        }
        if ui.button("Delete").clicked() {
            st.message = match delete_set(cx, &sel) {
                Ok(()) => format!("Deleted {sel}"),
                Err(e) => e,
            };
        }
        if ui.button("Import From Plan File\u{2026}").clicked() {
            pick_plan(st);
        }
    });
    if st.mode != Mode::None {
        ui.horizontal(|ui| {
            ui.label(match st.mode {
                Mode::New => "New set name",
                Mode::Copy => "Name of the copy",
                _ => "New name",
            });
            ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(200.0));
            if ui.button("OK").clicked() {
                let name = st.name.clone();
                let res = match st.mode {
                    Mode::New => new_set(cx, &name),
                    Mode::Copy => copy_set(cx, &sel, &name),
                    Mode::Rename => rename_set(cx, &sel, &name),
                    Mode::None => unreachable!(),
                };
                match res {
                    Ok(n) => {
                        st.message = format!("Layer set {n}");
                        st.selected = Some(n);
                        st.mode = Mode::None;
                    }
                    Err(e) => st.message = e,
                }
            }
            if ui.button("Cancel").clicked() {
                st.mode = Mode::None;
            }
        });
    }
    import_section(ui, cx, st);
    if !st.message.is_empty() {
        ui.separator();
        ui.label(&st.message);
    }
}

/// The Active Layers by Tool window: which layer each tool draws on.
fn show_tools(ctx: &egui::Context, cx: &mut EditorContext) {
    if !state(|s| s.tools_open) {
        return;
    }
    let mut open = true;
    egui::Window::new("Active Layers by Tool")
        .id(egui::Id::new("active_layers_by_tool"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(420.0, 460.0))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label("The layer each tool draws on. New CAD objects go on the Current CAD Layer; walls, doors and windows keep their own layers.");
            let mut pick: Option<(&'static str, String)> = None;
            egui::Grid::new("active_layers_grid")
                .num_columns(2)
                .spacing(Vec2::new(10.0, 4.0))
                .striped(true)
                .show(ui, |ui| {
                    for info in plan_core::layers::TOOL_LAYERS {
                        ui.label(info.label);
                        let current = cx.project.layers.tool_layer(info.key);
                        if let Some(l) = super::select_layer::layer_combo(
                            ui,
                            cx,
                            ("tool_layer", info.key),
                            &current,
                            220.0,
                        ) {
                            pick = Some((info.key, l));
                        }
                        ui.end_row();
                    }
                });
            if let Some((tool, layer)) = pick {
                set_tool_layer(cx, tool, &layer);
            }
            ui.separator();
            if ui.button("Reset to Default Layers").clicked() {
                reset_tool_layers(cx);
            }
        });
    if !open {
        state(|s| s.tools_open = false);
    }
}

/// Picks the plan file to import from and reads it.
fn pick_plan(st: &mut State) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Plan Studio", &[crate::files::EXT])
        .pick_file()
    else {
        return;
    };
    match load_plan(&path) {
        Ok(project) => {
            st.message.clear();
            st.source = Some(Source {
                label: path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into(),
                ),
                project,
                picks: BTreeSet::new(),
            });
        }
        Err(e) => st.message = format!("Could not read the plan: {e}"),
    }
}

fn import_section(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut State) {
    let Some(src) = st.source.as_mut() else {
        return;
    };
    ui.separator();
    ui.strong(format!("Layer sets in {}", src.label));
    let names: Vec<String> = src
        .project
        .layer_sets
        .names()
        .into_iter()
        .map(str::to_string)
        .collect();
    egui::ScrollArea::vertical()
        .id_salt("layer_set_import_list")
        .max_height(140.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for n in &names {
                let mut on = src.picks.contains(n);
                if ui.checkbox(&mut on, n).changed() {
                    if on {
                        src.picks.insert(n.clone());
                    } else {
                        src.picks.remove(n);
                    }
                }
            }
        });
    let mut done = false;
    let mut cancel = false;
    ui.horizontal(|ui| {
        if ui.button("Select All").clicked() {
            src.picks = names.iter().cloned().collect();
        }
        if ui.button("Select None").clicked() {
            src.picks.clear();
        }
        if ui.button("Import").clicked() {
            let picks: Vec<String> = src.picks.iter().cloned().collect();
            st.message = match import_sets(cx, &src.project, &picks) {
                Ok(added) => {
                    done = true;
                    format!("Imported {}", added.join(", "))
                }
                Err(e) => e,
            };
        }
        if ui.button("Close").clicked() {
            cancel = true;
        }
    });
    if done || cancel {
        st.source = None;
    }
}

/// A one-line summary of a set.
#[cfg(test)]
pub fn set_summary(def: &plan_core::layer_sets::LayerSetDef) -> String {
    let hidden = def.states.iter().filter(|s| !s.display).count();
    let locked = def.states.iter().filter(|s| s.locked).count();
    format!("{hidden} hidden, {locked} locked")
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::layer_sets::DEFAULT_LAYER_SET_NAME;

    fn cx() -> EditorContext {
        crate::editor::plan_tabs::plain_cx()
    }

    #[test]
    fn new_copy_rename_delete_are_undoable_and_validated() {
        let mut cx = cx();
        let first = cx.project.layer_sets.active.clone();
        assert_eq!(new_set(&mut cx, " Blank ").unwrap(), "Blank");
        assert!(new_set(&mut cx, "Blank").is_err(), "names are unique");
        assert!(new_set(&mut cx, "  ").is_err());
        assert_eq!(cx.undo_label(), Some("New Layer Set"));
        assert_eq!(cx.project.layer_sets.sets.len(), 2);

        // A copy starts equal to its source and is independent.
        cx.project.edit_layers(
            &["Doors".to_string()],
            &plan_core::layer_sets::LayerEdit::Display(false),
            false,
        );
        assert_eq!(copy_set(&mut cx, &first, "Doorless").unwrap(), "Doorless");
        assert!(copy_set(&mut cx, "nope", "X").is_err());
        let copy = cx.project.layer_sets.get("Doorless").unwrap().clone();
        assert!(!copy.state("Doors").unwrap().display);
        cx.project.layer_sets.set_display("Doorless", "Doors", true);
        assert!(
            !cx.project
                .layer_sets
                .get(&first)
                .unwrap()
                .state("Doors")
                .unwrap()
                .display
        );

        // Rename repoints the plan views and the active pointer.
        assert_eq!(rename_set(&mut cx, &first, "Main").unwrap(), "Main");
        assert_eq!(cx.project.layer_sets.active, "Main");
        assert_eq!(cx.project.current_plan_view().unwrap().layer_set, "Main");
        assert!(rename_set(&mut cx, "Main", "Blank").is_err());
        assert!(rename_set(&mut cx, "zzz", "Q").is_err());

        // Delete: the plan view falls back; the last set stays.
        assert!(delete_set(&mut cx, "Main").is_ok());
        assert_ne!(cx.project.current_plan_view().unwrap().layer_set, "Main");
        assert!(delete_set(&mut cx, "zzz").is_err());
        assert!(delete_set(&mut cx, "Blank").is_ok());
        assert!(
            delete_set(&mut cx, "Doorless").is_err(),
            "the last set stays"
        );
        cx.undo();
        assert_eq!(cx.project.layer_sets.sets.len(), 2);
        assert_eq!(
            set_summary(cx.project.layer_sets.get("Doorless").unwrap()),
            "0 hidden, 0 locked"
        );
    }

    #[test]
    fn activating_a_set_moves_the_active_plan_view_to_it() {
        let mut cx = cx();
        new_set(&mut cx, "Other").unwrap();
        assert!(activate_set(&mut cx, "Other"));
        assert_eq!(cx.project.layer_sets.active, "Other");
        assert_eq!(cx.project.current_plan_view().unwrap().layer_set, "Other");
        assert!(!activate_set(&mut cx, "Other"), "already active");
        assert!(!activate_set(&mut cx, "nope"));
        cx.undo();
        assert_eq!(cx.project.layer_sets.active, DEFAULT_LAYER_SET_NAME);
    }

    #[test]
    fn import_brings_sets_and_their_layers_from_another_plan() {
        let mut other = Project::new("Other");
        other
            .layers
            .add(plan_core::Layer::new("Imported Layer", [4, 4, 4], 20));
        other.layer_sets.copy_set(DEFAULT_LAYER_SET_NAME, "Theirs");
        other
            .layer_sets
            .set_display("Theirs", "Imported Layer", false);
        let mut cx = cx();
        assert!(import_sets(&mut cx, &other, &[]).is_err());
        assert!(import_sets(&mut cx, &other, &["nope".to_string()]).is_err());
        let added = import_sets(&mut cx, &other, &["Theirs".to_string()]).unwrap();
        assert_eq!(added, vec!["Theirs"]);
        assert!(cx.project.layers.get("Imported Layer").is_some());
        cx.project.show_layer_set("Theirs");
        cx.refresh();
        assert!(!cx.layers().is_visible("Imported Layer"));
        // Importing again renames.
        let again = import_sets(&mut cx, &other, &["Theirs".to_string()]).unwrap();
        assert_eq!(again, vec!["Theirs (2)"]);
        cx.undo();
        assert!(cx.project.layer_sets.get("Theirs (2)").is_none());
    }

    #[test]
    fn plan_files_load_for_import() {
        let dir = std::env::temp_dir().join(format!("layer-sets-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("o.psplan");
        let mut other = Project::new("O");
        other
            .layer_sets
            .copy_set(DEFAULT_LAYER_SET_NAME, "From File");
        plan_core::io::save_project(&other, &path).unwrap();
        let loaded = load_plan(&path).unwrap();
        assert!(loaded.layer_sets.get("From File").is_some());
        assert!(load_plan(&dir.join("missing.psplan")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn each_tool_has_its_active_layer_and_it_is_undoable() {
        let mut cx = cx();
        assert_eq!(cx.project.layers.current_cad_layer(), "CAD, Default");
        assert!(set_tool_layer(&mut cx, "cad", "Text"));
        assert_eq!(cx.project.layers.current_cad_layer(), "Text");
        assert_eq!(
            cx.project.layers.tool_layer("dimensions"),
            "Dimensions, Manual"
        );
        assert!(!set_tool_layer(&mut cx, "cad", "Text"), "no change");
        assert!(!set_tool_layer(&mut cx, "cad", "No Such Layer"));
        assert!(!set_tool_layer(&mut cx, "nope", "Text"));
        assert!(set_tool_layer(&mut cx, "dimensions", "Rooms"));
        assert_eq!(cx.undo_label(), Some("Active Layer"));
        cx.undo();
        assert_eq!(
            cx.project.layers.tool_layer("dimensions"),
            "Dimensions, Manual"
        );
        assert_eq!(cx.project.layers.current_cad_layer(), "Text");
        assert!(reset_tool_layers(&mut cx));
        assert!(!reset_tool_layers(&mut cx));
        assert_eq!(cx.project.layers.current_cad_layer(), "CAD, Default");
        // The choice is part of the plan file.
        set_tool_layer(&mut cx, "cad", "Rooms");
        let json = cx.project.to_json().unwrap();
        let back = Project::from_json(&json).unwrap();
        assert_eq!(back.layers.current_cad_layer(), "Rooms");
    }

    #[test]
    fn layer_set_defaults_are_one_undo_step_each() {
        let mut cx = cx();
        new_set(&mut cx, "Framing Only").unwrap();
        assert!(set_view_default(
            &mut cx,
            ViewKind::FramingPlan,
            Some("Framing Only")
        ));
        assert_eq!(cx.undo_label(), Some("Layer Set Defaults"));
        assert_eq!(
            cx.project.initial_layer_set(ViewKind::FramingPlan),
            "Framing Only"
        );
        assert!(!set_view_default(
            &mut cx,
            ViewKind::FramingPlan,
            Some("Framing Only")
        ));
        assert!(!set_view_default(
            &mut cx,
            ViewKind::RoofPlan,
            Some("Ghost")
        ));
        assert!(set_view_default(
            &mut cx,
            ViewKind::FramingPlan,
            Some(USE_ACTIVE_LAYER_SET)
        ));
        assert!(cx.project.layer_set_defaults.is_default());
        cx.undo();
        assert_eq!(
            cx.project.initial_layer_set(ViewKind::FramingPlan),
            "Framing Only"
        );
    }

    #[test]
    fn the_windows_draw_and_the_commands_open_them() {
        let ctx = egui::Context::default();
        let mut cx = cx();
        assert!(!run_command(&mut cx, "nope"));
        assert!(run_command(&mut cx, OPEN));
        assert!(run_command(&mut cx, ACTIVE_LAYERS));
        assert!(run_command(&mut cx, DEFAULTS));
        assert!(is_open());
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show_all(ctx, &mut cx));
        }
        state(|s| {
            s.open = false;
            s.tools_open = false;
            s.defaults_open = false;
        });
        assert!(!is_open());
    }
}
