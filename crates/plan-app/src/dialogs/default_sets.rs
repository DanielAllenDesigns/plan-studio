//! Default Sets, the Active Defaults dialog and the Selected Defaults panel
//! (manual pp. 106 to 110).
//!
//! * **Tools > Active Defaults** ([`ACTIVE_DEFAULTS`]): "Currently Using" (a
//!   Default Set or "Active Defaults"), Save New Default Set, Edit and Rename
//!   Default Set, the Selected Defaults of each annotation kind (pick, Add,
//!   Edit, Rename, Delete) and the Layers (Layer Set, Current CAD Layer).
//! * **Default Sets** ([`DEFAULT_SETS`]): the sets of the plan with New (copy
//!   of the selected one) and Delete, the Name, the Selected Defaults and the
//!   Layers of the selected set.
//! * The same panel is the Selected Defaults panel of the Plan View
//!   Specification (`plan_views.rs`).
//! * The three toolbar controls (Active Layer Set, Active Dimension Defaults,
//!   Active Default Set) read [`bar`] and ask for a pick with
//!   [`request_pick`].
//!
//! Switching the active defaults is not an undo step (it changes no object);
//! the lists the dialogs edit (Add, Rename, Delete, the sets) are the plan's
//! and are changed at once, the pick of a "Currently Using" and the layers
//! apply on OK.

use crate::dialogs::text::defaults::{self as text_defaults, DefaultsDialog, DefaultsKind};
use crate::dialogs::{DefaultsList, Outcome};
use crate::editor::EditorContext;
use eframe::egui::{self, Align, Align2, Layout};
use plan_core::defaults::saved::{DefaultSet, SavedKind};
use plan_core::Project;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// Command id: Tools > Active Defaults.
pub const ACTIVE_DEFAULTS: &str = "defaults.active";
/// Command id: Edit > Default Settings > Default Sets (the Default Sets
/// dialog), or the Default Sets toolbar button.
pub const DEFAULT_SETS: &str = "defaults.default_sets";
/// Command id: the toolbar controls ask for a pick with [`request_pick`].
pub const PICK: &str = "defaults.pick";

/// What "Currently Using" shows when no Default Set matches.
pub const USING_ACTIVE: &str = "Using Active Defaults";

// ===================================================================
// The Selected Defaults draft
// ===================================================================

/// The defaults a view or a dialog selects: a Default Set (or none), the
/// saved default of each annotation kind, the Layer Set and the Current CAD
/// Layer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Selected {
    /// The Default Set in use; empty is "Active Defaults".
    pub default_set: String,
    /// Saved default name by [`SavedKind::id`].
    pub picks: BTreeMap<String, String>,
    pub layer_set: String,
    pub cad_layer: String,
}

impl Selected {
    /// What is in force now.
    pub fn current(cx: &mut EditorContext) -> Selected {
        let using = cx.project.using_default_set(&mut cx.defaults);
        Selected {
            default_set: using.unwrap_or_default(),
            picks: cx.project.current_picks(&mut cx.defaults),
            layer_set: cx.project.shown_layer_set().to_string(),
            cad_layer: Project::current_cad_layer(&cx.defaults),
        }
    }

    /// What Default Set `name` selects.
    pub fn of_set(set: &DefaultSet, fallback: &Selected) -> Selected {
        let mut s = fallback.clone();
        s.default_set = set.name.clone();
        for (k, v) in &set.members {
            s.picks.insert(k.clone(), v.clone());
        }
        if !set.layer_set.is_empty() {
            s.layer_set = set.layer_set.clone();
        }
        if !set.cad_layer.is_empty() {
            s.cad_layer = set.cad_layer.clone();
        }
        s
    }

    /// Makes the selection the one in force: the Default Set when one is
    /// named (and its members are the picks), else each pick, then the Layer
    /// Set and the CAD layer. Not an undo step.
    pub fn apply(&self, cx: &mut EditorContext) {
        let whole_set = self.set_in(cx).is_some_and(|set| {
            set.members
                .iter()
                .all(|(k, v)| self.picks.get(k) == Some(v))
        });
        if whole_set {
            cx.project
                .default_set_activate(&mut cx.defaults, &self.default_set);
        } else {
            for k in SavedKind::ANNOTATION {
                cx.project.saved_commit(&mut cx.defaults, k);
            }
            for (kid, name) in &self.picks {
                if let Some(kind) = SavedKind::from_id(kid) {
                    cx.project.saved_activate(&mut cx.defaults, kind, name);
                }
            }
            cx.project.saved_defaults.active_set.clear();
        }
        if !self.layer_set.is_empty() && cx.project.layer_sets.get(&self.layer_set).is_some() {
            cx.project.show_layer_set(&self.layer_set);
        }
        if !self.cad_layer.is_empty() {
            cx.defaults.pages.insert(
                plan_core::defaults::saved::CURRENT_CAD_LAYER_KEY.to_string(),
                plan_core::defaults::PageValue::Text(self.cad_layer.clone()),
            );
        }
        cx.refresh_layer_view();
        cx.mark_dirty();
    }

    fn set_in(&self, cx: &EditorContext) -> Option<DefaultSet> {
        (!self.default_set.is_empty())
            .then(|| cx.project.saved_defaults.set(&self.default_set).cloned())
            .flatten()
    }
}

/// What the panel needs to draw its lists.
pub struct Env {
    pub names: BTreeMap<SavedKind, Vec<String>>,
    pub sets: Vec<DefaultSet>,
    pub layer_sets: Vec<String>,
    pub cad_layers: Vec<String>,
}

impl Env {
    pub fn of(cx: &mut EditorContext) -> Env {
        let mut names = BTreeMap::new();
        for k in SavedKind::ANNOTATION {
            names.insert(k, cx.project.saved_names(&cx.defaults, k));
        }
        Env {
            names,
            sets: cx.project.saved_defaults.sets.clone(),
            layer_sets: cx
                .project
                .layer_sets
                .names()
                .into_iter()
                .map(str::to_string)
                .collect(),
            cad_layers: cx
                .project
                .layers
                .layers
                .iter()
                .map(|l| l.name.clone())
                .collect(),
        }
    }
}

/// What the panel asks its dialog to do (they change the lists, so they run
/// with the context after the window is drawn).
#[derive(Debug, Clone, PartialEq)]
pub enum Ev {
    /// Add: a new saved default as a copy of the pick.
    Add(SavedKind, String),
    Edit(SavedKind, String),
    Rename(SavedKind, String),
    Delete(SavedKind, String),
    SaveNewSet,
    RenameSet(String),
    EditSets,
    DefineLayers,
}

impl Selected {
    /// Draws the Selected Defaults and Layers panels (and the Default Set row
    /// when `with_set`). Returns what the user asked for.
    pub fn panel(&mut self, ui: &mut egui::Ui, env: &Env, with_set: bool, salt: &str) -> Vec<Ev> {
        let mut ev = Vec::new();
        if with_set {
            ui.strong("Default Set");
            ui.horizontal(|ui| {
                ui.label("Currently Using");
                let shown = if self.default_set.is_empty() {
                    USING_ACTIVE.to_string()
                } else {
                    self.default_set.clone()
                };
                egui::ComboBox::from_id_salt((salt, "set"))
                    .selected_text(shown)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(self.default_set.is_empty(), "Active Defaults")
                            .clicked()
                        {
                            self.default_set.clear();
                        }
                        for s in &env.sets {
                            if ui
                                .selectable_label(self.default_set == s.name, &s.name)
                                .clicked()
                            {
                                *self = Selected::of_set(s, self);
                            }
                        }
                    });
                if self.default_set.is_empty()
                    && ui
                        .button("Save New Default Set")
                        .on_hover_text(
                            "Make a Default Set of the Selected Defaults and Layers below",
                        )
                        .clicked()
                {
                    ev.push(Ev::SaveNewSet);
                }
                if !self.default_set.is_empty() {
                    if ui.button("Edit Default Set").clicked() {
                        ev.push(Ev::EditSets);
                    }
                    if ui.button("Rename Default Set").clicked() {
                        ev.push(Ev::RenameSet(self.default_set.clone()));
                    }
                }
            });
            ui.add_space(4.0);
        }
        ui.strong("Selected Defaults");
        egui::Grid::new((salt, "picks"))
            .num_columns(3)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for kind in SavedKind::ANNOTATION {
                    ui.label(kind.label());
                    let names = env.names.get(&kind).cloned().unwrap_or_default();
                    let cur = self.picks.entry(kind.id().to_string()).or_default();
                    egui::ComboBox::from_id_salt((salt, kind.id()))
                        .selected_text(cur.clone())
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for n in &names {
                                ui.selectable_value(cur, n.clone(), n);
                            }
                        });
                    let pick = cur.clone();
                    ui.horizontal(|ui| {
                        if ui
                            .small_button("Add")
                            .on_hover_text("A new saved default based on this one")
                            .clicked()
                        {
                            ev.push(Ev::Add(kind, pick.clone()));
                        }
                        if ui.small_button("Edit").clicked() {
                            ev.push(Ev::Edit(kind, pick.clone()));
                        }
                        if ui.small_button("Rename").clicked() {
                            ev.push(Ev::Rename(kind, pick.clone()));
                        }
                        if ui.small_button("Delete").clicked() {
                            ev.push(Ev::Delete(kind, pick.clone()));
                        }
                    });
                    ui.end_row();
                }
            });
        ui.add_space(4.0);
        ui.strong("Layers");
        egui::Grid::new((salt, "layers"))
            .num_columns(3)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("Layer Set");
                egui::ComboBox::from_id_salt((salt, "layer_set"))
                    .selected_text(self.layer_set.clone())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for n in &env.layer_sets {
                            ui.selectable_value(&mut self.layer_set, n.clone(), n);
                        }
                    });
                if ui.small_button("Define").clicked() {
                    ev.push(Ev::DefineLayers);
                }
                ui.end_row();
                ui.label("Current CAD Layer");
                egui::ComboBox::from_id_salt((salt, "cad_layer"))
                    .selected_text(self.cad_layer.clone())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for n in &env.cad_layers {
                            ui.selectable_value(&mut self.cad_layer, n.clone(), n);
                        }
                    });
                if ui.small_button("Define").clicked() {
                    ev.push(Ev::DefineLayers);
                }
                ui.end_row();
            });
        // Changing a pick away from the set's member leaves "Active Defaults".
        if let Some(set) = env.sets.iter().find(|s| s.name == self.default_set) {
            if set
                .members
                .iter()
                .any(|(k, v)| self.picks.get(k) != Some(v))
            {
                self.default_set.clear();
            }
        }
        ev
    }
}

// ===================================================================
// State
// ===================================================================

/// What a name prompt is for.
#[derive(Debug, Clone, PartialEq)]
pub enum PromptFor {
    /// Copy saved default `.1` of `.0` under the typed name; its defaults
    /// dialog opens next.
    AddSaved(SavedKind, String),
    RenameSaved(SavedKind, String),
    SaveNewSet,
    RenameSet(String),
    /// A copy of Default Set `.0` (the New button of the Default Sets dialog).
    CopySet(String),
}

/// The small "New Default Name" style dialog.
#[derive(Debug, Clone)]
pub struct Prompt {
    pub title: String,
    pub what: PromptFor,
    pub text: String,
    pub error: String,
}

/// An Active Defaults window.
struct ActiveWindow {
    selected: Selected,
    /// Shown in the title bar: the active plan view.
    view: String,
    message: String,
}

/// A Default Sets window.
struct SetsWindow {
    selected: Option<String>,
    draft: Selected,
    message: String,
}

#[derive(Default)]
struct State {
    active: Option<ActiveWindow>,
    sets: Option<SetsWindow>,
    prompt: Option<Prompt>,
    /// A defaults dialog opened by an Edit button, and the kind and name of
    /// the saved default it edits.
    editor: Option<(SavedKind, String, DefaultsDialog)>,
    /// The Saved Dimension Defaults list, opened by the Edit button of the
    /// Manual Dimensions.
    list: Option<DefaultsList>,
    bar: Bar,
    pick: Option<Pick>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// The lists the toolbar controls draw from, refreshed every frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bar {
    pub layer_sets: Vec<String>,
    pub shown_layer_set: String,
    pub dimension_sets: Vec<String>,
    pub active_dimension_set: String,
    pub default_sets: Vec<String>,
    /// The Default Set in use; `None` is "Using Active Defaults".
    pub using_set: Option<String>,
}

/// The toolbar controls' lists.
pub fn bar() -> Bar {
    state(|s| s.bar.clone())
}

/// What a toolbar control picked.
#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    LayerSet(String),
    DimensionDefaults(String),
    DefaultSet(String),
}

/// A toolbar control picked something; [`PICK`] applies it.
pub fn request_pick(p: Pick) {
    state(|s| s.pick = Some(p));
}

/// Runs a command by id; false when it is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        ACTIVE_DEFAULTS => open_active(cx),
        DEFAULT_SETS => open_sets(cx),
        PICK => {
            if let Some(p) = state(|s| s.pick.take()) {
                apply_pick(cx, &p);
            }
        }
        _ => return false,
    }
    true
}

/// Applies a toolbar pick: the layer set shown, the active dimension
/// defaults, a Default Set.
pub fn apply_pick(cx: &mut EditorContext, p: &Pick) {
    match p {
        Pick::LayerSet(n) => {
            if cx.project.show_layer_set(n) {
                cx.refresh_layer_view();
                cx.mark_dirty();
                cx.status = format!("Layer set: {n}");
            }
        }
        Pick::DimensionDefaults(n) => {
            if cx
                .project
                .saved_activate(&mut cx.defaults, SavedKind::ManualDimensions, n)
            {
                cx.status = format!("Dimension defaults: {n}");
            }
        }
        Pick::DefaultSet(n) => {
            if cx.project.default_set_activate(&mut cx.defaults, n) {
                cx.refresh_layer_view();
                cx.mark_dirty();
                cx.status = format!("Default Set: {n}");
            }
        }
    }
}

/// Opens Tools > Active Defaults.
pub fn open_active(cx: &mut EditorContext) {
    let selected = Selected::current(cx);
    let view = cx.project.active_plan_view.clone();
    state(|s| {
        s.active = Some(ActiveWindow {
            selected,
            view,
            message: String::new(),
        })
    });
}

/// Is the Active Defaults dialog open?
pub fn active_is_open() -> bool {
    state(|s| s.active.is_some())
}

/// Opens the Default Sets dialog on the set in use (or the first).
pub fn open_sets(cx: &mut EditorContext) {
    let using = cx.project.using_default_set(&mut cx.defaults);
    let first = cx
        .project
        .saved_defaults
        .sets
        .first()
        .map(|s| s.name.clone());
    let sel = using.or(first);
    let draft = match sel
        .as_deref()
        .and_then(|n| cx.project.saved_defaults.set(n).cloned())
    {
        Some(set) => Selected::of_set(&set, &Selected::current(cx)),
        None => Selected::current(cx),
    };
    state(|s| {
        s.sets = Some(SetsWindow {
            selected: sel,
            draft,
            message: String::new(),
        })
    });
}

/// Is the Default Sets dialog open?
pub fn sets_is_open() -> bool {
    state(|s| s.sets.is_some())
}

/// Asks for a name before `what` (Add, Rename, Save New Default Set).
pub fn prompt(title: &str, what: PromptFor, text: &str) {
    state(|s| {
        s.prompt = Some(Prompt {
            title: title.to_string(),
            what,
            text: text.to_string(),
            error: String::new(),
        })
    });
}

/// Is a name prompt open?
pub fn prompt_is_open() -> bool {
    state(|s| s.prompt.is_some())
}

/// Types the name into the open prompt and answers OK (the tests do what the
/// user does). Returns the error text, if any.
pub fn answer_prompt(cx: &mut EditorContext, text: &str) -> Result<(), String> {
    let Some(mut p) = state(|s| s.prompt.take()) else {
        return Err("No prompt".into());
    };
    p.text = text.to_string();
    match run_prompt(cx, &p) {
        Ok(()) => Ok(()),
        Err(e) => {
            p.error = e.clone();
            state(|s| s.prompt = Some(p));
            Err(e)
        }
    }
}

fn run_prompt(cx: &mut EditorContext, p: &Prompt) -> Result<(), String> {
    let name = p.text.trim().to_string();
    match &p.what {
        PromptFor::AddSaved(kind, from) => {
            cx.project
                .saved_copy(&mut cx.defaults, *kind, from, &name)?;
            // The defaults dialog of the new saved default opens next.
            edit_saved(cx, *kind, &name);
            cx.status = format!("Added the saved default \"{name}\"");
        }
        PromptFor::RenameSaved(kind, old) => {
            cx.project
                .saved_rename(&mut cx.defaults, *kind, old, &name)?;
            cx.status = format!("Renamed \"{old}\" to \"{name}\"");
        }
        PromptFor::SaveNewSet => {
            cx.project.default_set_save_new(&mut cx.defaults, &name)?;
            cx.status = format!("Saved the Default Set \"{name}\"");
        }
        PromptFor::RenameSet(old) => {
            cx.project.default_set_rename(old, &name)?;
            cx.status = format!("Renamed the Default Set to \"{name}\"");
        }
        PromptFor::CopySet(from) => {
            cx.project.default_set_copy(from, &name)?;
            cx.status = format!("Added the Default Set \"{name}\"");
        }
    }
    cx.mark_dirty();
    Ok(())
}

/// Edit: makes saved default `name` of `kind` the one in use and opens the
/// defaults dialog of the kind on it. (Chief's Edit opens the dialog of the
/// saved default picked in the list; the values a dialog edits are the
/// tools', so the pick becomes the active one, see DECISIONS DS12.)
pub fn edit_saved(cx: &mut EditorContext, kind: SavedKind, name: &str) {
    if !cx.project.saved_activate(&mut cx.defaults, kind, name) {
        cx.status = format!("There is no saved default \"{name}\"");
        return;
    }
    match kind {
        SavedKind::ManualDimensions => {
            let list = DefaultsList::dimensions(cx);
            state(|s| s.list = Some(list));
        }
        SavedKind::Text
        | SavedKind::RichText
        | SavedKind::Callouts
        | SavedKind::Markers
        | SavedKind::Notes => {
            let dk = match kind {
                SavedKind::Text => DefaultsKind::Text,
                SavedKind::RichText => DefaultsKind::RichText,
                SavedKind::Callouts => DefaultsKind::Callout,
                SavedKind::Markers => DefaultsKind::Marker,
                _ => DefaultsKind::Note,
            };
            let d = text_defaults::open(cx, dk);
            state(|s| s.editor = Some((kind, name.to_string(), d)));
        }
        other => {
            if let Some(page) = other.page_id() {
                crate::dialogs::default_pages::request_open(page);
            }
        }
    }
}

/// Is an Edit dialog (the annotation defaults or the dimension sets) open?
pub fn editor_is_open() -> bool {
    state(|s| s.editor.is_some() || s.list.is_some())
}

// ===================================================================
// Drawing
// ===================================================================

/// Draws every window of this module and refreshes the toolbar lists; call
/// once a frame.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    refresh_bar(cx);
    show_active(ctx, cx);
    show_sets(ctx, cx);
    show_prompt(ctx, cx);
    show_editors(ctx, cx);
}

fn refresh_bar(cx: &mut EditorContext) {
    let bar = Bar {
        layer_sets: cx
            .project
            .layer_sets
            .names()
            .into_iter()
            .map(str::to_string)
            .collect(),
        shown_layer_set: cx.project.shown_layer_set().to_string(),
        dimension_sets: cx
            .defaults
            .dimension_sets
            .iter()
            .map(|s| s.name.clone())
            .collect(),
        active_dimension_set: cx.defaults.active_dimension_set.clone(),
        default_sets: cx
            .project
            .saved_defaults
            .sets
            .iter()
            .map(|s| s.name.clone())
            .collect(),
        using_set: cx.project.using_default_set(&mut cx.defaults),
    };
    state(|s| {
        if s.bar != bar {
            s.bar = bar;
        }
    });
}

/// Carries out what the panel asked for.
fn handle(cx: &mut EditorContext, ev: Ev, sel: &mut Selected) -> Option<String> {
    match ev {
        Ev::Add(kind, from) => {
            let suggestion = format!("{from} Copy");
            prompt(
                &format!("New {} Default Name", kind.label()),
                PromptFor::AddSaved(kind, from),
                &suggestion,
            );
        }
        Ev::Edit(kind, name) => {
            edit_saved(cx, kind, &name);
            sel.picks.insert(kind.id().to_string(), name);
            sel.default_set.clear();
        }
        Ev::Rename(kind, name) => prompt(
            "Rename Current Default",
            PromptFor::RenameSaved(kind, name.clone()),
            &name,
        ),
        Ev::Delete(kind, name) => {
            return match cx.project.saved_delete(&mut cx.defaults, kind, &name) {
                Ok(()) => {
                    // The picks follow the deletion.
                    sel.picks.insert(
                        kind.id().to_string(),
                        cx.project.saved_active(&cx.defaults, kind),
                    );
                    cx.mark_dirty();
                    Some(format!("Deleted \"{name}\""))
                }
                Err(e) => Some(e),
            };
        }
        Ev::SaveNewSet => {
            // The set takes the draft's picks and layers.
            sel.apply(cx);
            prompt("New Default Set", PromptFor::SaveNewSet, "");
        }
        Ev::RenameSet(old) => prompt(
            "Rename Default Set",
            PromptFor::RenameSet(old.clone()),
            &old,
        ),
        Ev::EditSets => open_sets(cx),
        Ev::DefineLayers => crate::dialogs::layer_sets::open(),
    }
    None
}

/// Carries out what the Selected Defaults panel asked for (the Plan View
/// Specification hosts the same panel). Returns a line for its message.
pub fn handle_event(cx: &mut EditorContext, ev: Ev, sel: &mut Selected) -> Option<String> {
    handle(cx, ev, sel)
}

fn show_active(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut w) = state(|s| s.active.take()) else {
        return;
    };
    let env = Env::of(cx);
    let mut events = Vec::new();
    let mut ok = false;
    let mut cancel = false;
    let mut open = true;
    egui::Window::new(format!("Active Defaults - {}", w.view))
        .id(egui::Id::new("active_defaults_dialog"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            events = w.selected.panel(ui, &env, true, "active");
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
    for ev in events {
        if let Some(m) = handle(cx, ev, &mut w.selected) {
            w.message = m;
        }
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !prompt_is_open() {
        cancel = true;
    }
    if ok {
        w.selected.apply(cx);
        cx.status = match &w.selected.default_set {
            n if n.is_empty() => "Using Active Defaults".to_string(),
            n => format!("Default Set: {n}"),
        };
    } else if open && !cancel {
        state(|s| s.active = Some(w));
    }
}

fn show_sets(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut w) = state(|s| s.sets.take()) else {
        return;
    };
    let env = Env::of(cx);
    let mut events = Vec::new();
    let mut close = false;
    let mut open = true;
    let mut pick: Option<String> = None;
    let mut new_copy = false;
    let mut delete = false;
    let mut rename_to: Option<String> = None;
    let mut name_text = w.selected.clone().unwrap_or_default();
    egui::Window::new("Default Sets")
        .id(egui::Id::new("default_sets_dialog"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.columns(2, |cols| {
                let ui = &mut cols[0];
                ui.strong("Default Sets");
                egui::ScrollArea::vertical()
                    .id_salt("default_sets_list")
                    .max_height(260.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for s in &env.sets {
                            let on = w.selected.as_deref() == Some(s.name.as_str());
                            if ui.selectable_label(on, &s.name).clicked() && !on {
                                pick = Some(s.name.clone());
                            }
                        }
                    });
                ui.horizontal(|ui| {
                    if ui
                        .button("New")
                        .on_hover_text("A copy of the selected set")
                        .clicked()
                    {
                        new_copy = true;
                    }
                    if ui.button("Delete").clicked() {
                        delete = true;
                    }
                });
                let ui = &mut cols[1];
                ui.strong("Default Set");
                ui.horizontal(|ui| {
                    ui.label("Name");
                    let r = ui.add(egui::TextEdit::singleline(&mut name_text).desired_width(200.0));
                    if r.lost_focus() && w.selected.as_deref() != Some(name_text.as_str()) {
                        rename_to = Some(name_text.clone());
                    }
                });
                events = w.draft.panel(ui, &env, false, "sets");
            });
            if !w.message.is_empty() {
                ui.label(&w.message);
            }
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
    if let Some(n) = pick {
        if let Some(set) = cx.project.saved_defaults.set(&n).cloned() {
            w.draft = Selected::of_set(&set, &w.draft);
            w.selected = Some(n);
        }
    }
    if new_copy {
        if let Some(from) = w.selected.clone() {
            prompt(
                "New Default Set",
                PromptFor::CopySet(from.clone()),
                &format!("{from} Copy"),
            );
        }
    }
    if delete {
        if let Some(n) = w.selected.clone() {
            w.message = match cx.project.default_set_delete(&n) {
                Ok(()) => {
                    w.selected = cx
                        .project
                        .saved_defaults
                        .sets
                        .first()
                        .map(|s| s.name.clone());
                    format!("Deleted the Default Set \"{n}\"")
                }
                Err(e) => e,
            };
        }
    }
    if let (Some(new), Some(old)) = (rename_to, w.selected.clone()) {
        w.message = match cx.project.default_set_rename(&old, &new) {
            Ok(()) => {
                w.selected = Some(new.trim().to_string());
                String::new()
            }
            Err(e) => e,
        };
    }
    // The picks and layers of the selected set write straight to it.
    let mut draft = w.draft.clone();
    for ev in events {
        if let Some(m) = handle(cx, ev, &mut draft) {
            w.message = m;
        }
    }
    w.draft = draft;
    if let Some(sel) = w.selected.clone() {
        if let Some(set) = cx
            .project
            .saved_defaults
            .sets
            .iter_mut()
            .find(|s| s.name == sel)
        {
            set.members = w.draft.picks.clone();
            set.layer_set = w.draft.layer_set.clone();
            set.cad_layer = w.draft.cad_layer.clone();
        }
    }
    if open && !close {
        state(|s| s.sets = Some(w));
    }
}

fn show_prompt(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut p) = state(|s| s.prompt.take()) else {
        return;
    };
    let mut ok = false;
    let mut cancel = false;
    egui::Window::new(&p.title)
        .id(egui::Id::new("default_name_prompt"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label("Type a short, descriptive, unique name.");
            let r = ui.add(egui::TextEdit::singleline(&mut p.text).desired_width(260.0));
            r.request_focus();
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                ok = true;
            }
            if !p.error.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(0xE0, 0x4B, 0x4B), &p.error);
            }
            ui.horizontal(|ui| {
                if ui.button("   OK   ").clicked() {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if ok {
        match run_prompt(cx, &p) {
            Ok(()) => {}
            Err(e) => {
                p.error = e;
                state(|s| s.prompt = Some(p));
            }
        }
    } else if !cancel {
        state(|s| s.prompt = Some(p));
    }
}

fn show_editors(ctx: &egui::Context, cx: &mut EditorContext) {
    if let Some((kind, name, mut d)) = state(|s| s.editor.take()) {
        match d.show(ctx) {
            Outcome::Open => state(|s| s.editor = Some((kind, name, d))),
            Outcome::Cancel => {}
            Outcome::Ok => {
                if d.apply(cx) {
                    cx.project.saved_commit(&mut cx.defaults, kind);
                    cx.status = format!("Saved the {} default \"{name}\"", kind.label());
                }
            }
        }
    }
    if let Some(mut list) = state(|s| s.list.take()) {
        if list.show(ctx, cx) {
            state(|s| s.list = Some(list));
        }
    }
}

#[cfg(test)]
pub(crate) fn reset_state() {
    state(|s| *s = State::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cx() -> EditorContext {
        reset_state();
        crate::editor::plan_tabs::plain_cx()
    }

    fn frame(cx: &mut EditorContext) {
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show_all(ctx, cx));
        }
    }

    #[test]
    fn save_new_default_set_names_the_picks_and_the_layers() {
        let mut cx = cx();
        open_active(&mut cx);
        assert!(active_is_open());
        // Add a Text default, pick it, save the whole as a set.
        cx.project
            .saved_copy(&mut cx.defaults, SavedKind::Text, "Default", "Big")
            .unwrap();
        cx.project
            .layer_sets
            .copy_set("Default Set", "Notes Layers");
        let mut sel = Selected::current(&mut cx);
        sel.picks.insert("text".into(), "Big".into());
        sel.layer_set = "Notes Layers".into();
        sel.apply(&mut cx);
        prompt("New Default Set", PromptFor::SaveNewSet, "");
        assert!(prompt_is_open());
        assert!(answer_prompt(&mut cx, "").is_err(), "a name is needed");
        answer_prompt(&mut cx, "Big Text").unwrap();
        let set = cx.project.saved_defaults.set("Big Text").unwrap();
        assert_eq!(set.member(SavedKind::Text), Some("Big"));
        assert_eq!(set.layer_set, "Notes Layers");
        assert_eq!(
            cx.project.using_default_set(&mut cx.defaults).as_deref(),
            Some("Big Text")
        );
        frame(&mut cx);
        assert_eq!(bar().using_set.as_deref(), Some("Big Text"));
    }

    #[test]
    fn the_toolbar_picks_switch_the_set_the_dimensions_and_the_layer_set() {
        let mut cx = cx();
        cx.project.layer_sets.copy_set("Default Set", "Plot Layers");
        cx.project
            .default_set_save_new(&mut cx.defaults, "A")
            .unwrap();
        cx.defaults.set_active_dimension_set("1/8\" Scale");
        cx.project
            .default_set_save_new(&mut cx.defaults, "B")
            .unwrap();
        frame(&mut cx);
        let b = bar();
        assert_eq!(b.default_sets, vec!["A", "B"]);
        assert_eq!(b.using_set.as_deref(), Some("B"));
        request_pick(Pick::DefaultSet("A".into()));
        assert!(run_command(&mut cx, PICK));
        assert_eq!(cx.defaults.active_dimension_set, "1/4\" Scale");
        request_pick(Pick::DimensionDefaults("Plot Plan".into()));
        run_command(&mut cx, PICK);
        assert_eq!(cx.defaults.active_dimension_set, "Plot Plan");
        frame(&mut cx);
        assert_eq!(bar().using_set, None, "a member changed: Active Defaults");
        request_pick(Pick::LayerSet("Plot Layers".into()));
        run_command(&mut cx, PICK);
        assert_eq!(cx.project.shown_layer_set(), "Plot Layers");
        assert!(!run_command(&mut cx, "nope"));
    }

    #[test]
    fn the_windows_draw_and_the_sets_dialog_edits_the_selected_set() {
        let mut cx = cx();
        cx.project
            .default_set_save_new(&mut cx.defaults, "A")
            .unwrap();
        cx.project.default_set_copy("A", "B").unwrap();
        run_command(&mut cx, DEFAULT_SETS);
        assert!(sets_is_open());
        frame(&mut cx);
        run_command(&mut cx, ACTIVE_DEFAULTS);
        frame(&mut cx);
        assert!(active_is_open() && sets_is_open());
        reset_state();
    }

    #[test]
    fn the_add_prompt_copies_the_pick_and_opens_its_dialog() {
        let mut cx = cx();
        prompt(
            "New Callout Default Name",
            PromptFor::AddSaved(SavedKind::Callouts, "Default".into()),
            "",
        );
        answer_prompt(&mut cx, "Section Callout").unwrap();
        assert!(cx
            .project
            .saved_names(&cx.defaults, SavedKind::Callouts)
            .contains(&"Section Callout".to_string()));
        assert_eq!(
            cx.project.saved_active(&cx.defaults, SavedKind::Callouts),
            "Section Callout"
        );
        assert!(editor_is_open(), "its defaults dialog opens next");
        frame(&mut cx);
        reset_state();
    }

    #[test]
    fn delete_says_why_it_cannot() {
        let mut cx = cx();
        cx.project
            .saved_copy(&mut cx.defaults, SavedKind::Markers, "Default", "M2")
            .unwrap();
        cx.project
            .default_set_save_new(&mut cx.defaults, "S")
            .unwrap();
        let mut sel = Selected::current(&mut cx);
        let m = handle(
            &mut cx,
            Ev::Delete(SavedKind::Markers, "Default".into()),
            &mut sel,
        );
        assert!(m.unwrap().contains("Default Set"));
        let m = handle(
            &mut cx,
            Ev::Delete(SavedKind::Markers, "M2".into()),
            &mut sel,
        );
        assert!(m.unwrap().starts_with("Deleted"));
        assert_eq!(
            sel.picks.get("markers").map(String::as_str),
            Some("Default")
        );
    }
}
