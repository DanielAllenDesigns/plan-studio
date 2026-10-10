//! The Default Settings > Framing dialogs (manual pp. 885-904):
//!
//! * **Framing Member Defaults Management** ("Framing Members"): the Default
//!   Framing Members of the plan with an In Use column and Edit, Copy,
//!   Rename, Delete, Merge, Select All, Clear All, Purge and Apply to Selected
//!   Members (Apply Framing Default Properties).
//! * **Automatic Framing Defaults** (`auto.rs`): the Foundation, floor level,
//!   Deck, Deck Support, Wall, Openings, Fireplaces, Roof and Trusses panels.
//! * **Manual Framing Defaults**: the General, Beams, Posts and Materials
//!   panels.
//!
//! All three work on drafts: OK stores the draft as one undo step, Cancel or
//! Escape drops it. The rules (unique names, in-use protection, merge,
//! purge) are `plan_framing::FramingCatalog`'s, so they are tested without a
//! window. The Framing Type Management dialog is `framing_types.rs` and the
//! Structural Member Reporting dialogs `member_reporting.rs`.

mod auto;

use super::{on, row, section, Outcome, SpecDialog, SpecPages, Tab, ERROR_RED};
use crate::editor::{framing_view, EditorContext};
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, Ui};
use plan_framing::catalog::{self, FramingCatalog, FramingMemberDef, Role};
use plan_framing::{AlignExterior, BeamPlacement, CatalogError, CategoryChoice, MaterialsCategory};
use std::cell::RefCell;

pub use auto::AutoDialog;

/// Command ids (Default Settings leaves, the toolbar buttons and menus).
pub const MEMBERS: &str = "framing.members";
pub const TYPES: &str = "framing.types";
pub const AUTOMATIC: &str = "framing.automatic";
pub const MANUAL: &str = "framing.manual";
pub const REPORTING: &str = "framing.reporting";
/// Applies the Default Framing Member named after the colon to the selected
/// members: `framing.apply:Joists - I Joists`.
pub const APPLY_PREFIX: &str = "framing.apply:";

// ----- storing the catalog -----

/// The catalog of the plan.
pub fn catalog_of(cx: &EditorContext) -> FramingCatalog {
    catalog::of_project(&cx.project)
}

/// Stores `new` as one undo step named `label` and gives the plan's automatic
/// members the shape of their (possibly edited) Framing Types. Returns false
/// when nothing changed.
pub fn set_catalog(cx: &mut EditorContext, new: &FramingCatalog, label: &str) -> bool {
    if catalog_of(cx) == *new && catalog::project_has_catalog(&cx.project) {
        return false;
    }
    cx.begin_change(label);
    store_catalog_raw(cx, new);
    cx.mark_dirty();
    cx.refresh();
    true
}

/// Stores the catalog without an undo step of its own (the caller opened
/// one) and restamps the members.
fn store_catalog_raw(cx: &mut EditorContext, new: &FramingCatalog) {
    if let Some(first) = cx.project.floors.first_mut() {
        catalog::store(&mut first.framing, new);
    }
    catalog::stamp_project(&mut cx.project, new);
}

/// Apply Framing Default Properties: gives the selected framing members the
/// type, Role, material and definition of the Default Framing Member `def`;
/// their sizes, lengths and places stay. One undo step. Returns how many
/// members changed.
pub fn apply_default_to_selection(cx: &mut EditorContext, def: &str) -> usize {
    let ids = framing_view::selected(cx);
    let cat = catalog_of(cx);
    if cat.def_named(def).is_none() {
        cx.status = format!("There is no Default Framing Member called \"{def}\"");
        return 0;
    }
    if ids.is_empty() {
        cx.status = "Select framing members to apply the default to".into();
        return 0;
    }
    let mut records = framing_view::load_records(cx.floor());
    let mut changed = 0;
    let mut touched = Vec::new();
    for r in &mut records {
        if let framing_view::Record::Manual(m) | framing_view::Record::Built(m) = r {
            if ids.contains(&m.id) {
                let before = m.clone();
                cat.apply_def_to_manual(def, m);
                if *m != before {
                    changed += 1;
                }
                touched.push(m.id);
            }
        }
    }
    if touched.is_empty() {
        cx.status = "Select framing members to apply the default to".into();
        return 0;
    }
    if changed > 0 {
        cx.begin_change("Apply Framing Default Properties");
        framing_view::store_records(cx.floor_mut(), &records);
        cx.mark_dirty();
        cx.refresh();
    }
    cx.status = format!(
        "Applied \"{def}\" to {} framing member{}",
        touched.len(),
        if touched.len() == 1 { "" } else { "s" }
    );
    changed
}

// ----- window chrome -----

/// A dialog window with an OK / Cancel row; `body` draws the content.
pub(super) fn window(
    ctx: &egui::Context,
    title: &str,
    key: &str,
    size: [f32; 2],
    enabled: bool,
    error: Option<&str>,
    body: impl FnOnce(&mut Ui),
) -> Outcome {
    let mut outcome = Outcome::Open;
    let mut open = true;
    egui::Window::new(title)
        .id(egui::Id::new(("framing_defaults", key)))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(size)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                let h = (ui.available_height() - 44.0).max(60.0);
                ui.allocate_ui(egui::vec2(ui.available_width(), h), |ui| body(ui));
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = egui::Button::new(egui::RichText::new("   OK   ").strong());
                    if ui.add_enabled(error.is_none(), ok).clicked() {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                    if let Some(e) = error {
                        ui.colored_label(ERROR_RED, e);
                    }
                });
            });
        });
    if !open {
        outcome = Outcome::Cancel;
    }
    if enabled && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        outcome = Outcome::Cancel;
    }
    outcome
}

/// A one-line name prompt (Rename).
pub(super) fn name_prompt(
    ctx: &egui::Context,
    title: &str,
    text: &mut String,
    error: Option<&str>,
) -> Outcome {
    let mut outcome = Outcome::Open;
    let mut open = true;
    egui::Window::new(title)
        .id(egui::Id::new(("framing_name_prompt", title)))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            let r = ui.add(egui::TextEdit::singleline(text).desired_width(260.0));
            r.request_focus();
            ui.add_space(6.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(error.is_none(), egui::Button::new("   OK   "))
                    .clicked()
                {
                    outcome = Outcome::Ok;
                }
                if ui.button("Cancel").clicked() {
                    outcome = Outcome::Cancel;
                }
                if let Some(e) = error {
                    ui.colored_label(ERROR_RED, e);
                }
            });
        });
    if !open {
        outcome = Outcome::Cancel;
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        outcome = Outcome::Cancel;
    } else if error.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
        outcome = Outcome::Ok;
    }
    outcome
}

/// A combo box over names; a value that is not among them stays listed.
pub(super) fn name_combo(ui: &mut Ui, salt: &str, value: &mut String, options: &[String]) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for o in options {
                if ui.selectable_label(value == o, o).clicked() {
                    *value = o.clone();
                }
            }
        });
}

/// A combo box over values with names.
pub(super) fn enum_combo<T: Copy + PartialEq>(
    ui: &mut Ui,
    salt: &str,
    value: &mut T,
    options: &[T],
    name: impl Fn(T) -> &'static str,
) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(name(*value))
        .show_ui(ui, |ui| {
            for &o in options {
                if ui.selectable_label(*value == o, name(o)).clicked() {
                    *value = o;
                }
            }
        });
}

/// An inch field.
pub(super) fn inches(ui: &mut Ui, v: &mut f64, lo: f64, hi: f64) {
    ui.add(
        egui::DragValue::new(v)
            .speed(0.125)
            .range(lo..=hi)
            .suffix("\""),
    );
}

// ----- Framing Member Defaults Management -----

/// What a click in the list does to the selection.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Click {
    Plain,
    Toggle,
    Range,
}

/// The editor of one or more Default Framing Members.
pub struct DefEditor {
    /// The definitions being edited, by name (several: Multiple Edit).
    pub targets: Vec<String>,
    pub def: FramingMemberDef,
}

/// The Framing Member Defaults Management dialog.
pub struct MembersDialog {
    pub draft: FramingCatalog,
    selected: Vec<String>,
    anchor: Option<String>,
    pub editor: Option<DefEditor>,
    prompt: Option<String>,
    pub message: String,
    /// Set by Apply to Selected Members: the definition to apply on OK.
    pub apply: Option<String>,
}

impl MembersDialog {
    pub fn new(catalog: FramingCatalog) -> Self {
        let first = catalog.defs.first().map(|d| d.name.clone());
        Self {
            draft: catalog,
            selected: first.iter().cloned().collect(),
            anchor: first,
            editor: None,
            prompt: None,
            message: String::new(),
            apply: None,
        }
    }

    pub fn selected(&self) -> &[String] {
        &self.selected
    }

    /// A click on the row of `name`.
    pub fn click(&mut self, name: &str, how: Click) {
        let names: Vec<String> = self.draft.defs.iter().map(|d| d.name.clone()).collect();
        match how {
            Click::Plain => {
                self.selected = vec![name.to_string()];
                self.anchor = Some(name.to_string());
            }
            Click::Toggle => {
                if let Some(i) = self.selected.iter().position(|n| n == name) {
                    self.selected.remove(i);
                } else {
                    self.selected.push(name.to_string());
                }
                self.anchor = Some(name.to_string());
            }
            Click::Range => {
                let a = self
                    .anchor
                    .as_ref()
                    .and_then(|a| names.iter().position(|n| n == a))
                    .unwrap_or(0);
                let b = names.iter().position(|n| n == name).unwrap_or(a);
                let (lo, hi) = (a.min(b), a.max(b));
                self.selected = names[lo..=hi].to_vec();
            }
        }
        self.sort_selection();
    }

    fn sort_selection(&mut self) {
        let order: Vec<&str> = self.draft.defs.iter().map(|d| d.name.as_str()).collect();
        self.selected
            .sort_by_key(|n| order.iter().position(|o| *o == n).unwrap_or(usize::MAX));
        self.selected.dedup();
    }

    pub fn select_all(&mut self) {
        self.selected = self.draft.defs.iter().map(|d| d.name.clone()).collect();
    }

    pub fn clear_all(&mut self) {
        self.selected.clear();
    }

    /// Edit: opens the editor on the selected definitions.
    pub fn edit_selected(&mut self) -> bool {
        let Some(first) = self.selected.first().and_then(|n| self.draft.def_named(n)) else {
            return false;
        };
        self.editor = Some(DefEditor {
            targets: self.selected.clone(),
            def: first.clone(),
        });
        true
    }

    /// OK in the editor: one definition is replaced (and may be renamed);
    /// several get the type, material, Role and category of the edited one.
    pub fn editor_ok(&mut self) -> Result<(), CatalogError> {
        let Some(ed) = self.editor.take() else {
            return Ok(());
        };
        let result = if ed.targets.len() == 1 {
            let new_name = ed.def.name.trim().to_string();
            let r = self.draft.edit_def(&ed.targets[0], ed.def.clone());
            if r.is_ok() {
                self.selected = vec![new_name.clone()];
                self.anchor = Some(new_name);
            }
            r
        } else {
            for t in &ed.targets {
                if let Some(d) = self.draft.defs.iter_mut().find(|d| d.name == *t) {
                    d.framing_type = ed.def.framing_type.clone();
                    d.material = ed.def.material.clone();
                    d.role = ed.def.role;
                    d.category = ed.def.category;
                }
            }
            Ok(())
        };
        if let Err(e) = &result {
            // Keep the editor open on the same values.
            self.editor = Some(ed);
            self.message = e.to_string();
        }
        result
    }

    /// Copy: each selected definition is copied; the copies are selected.
    pub fn copy_selected(&mut self) -> usize {
        let mut made = Vec::new();
        for n in self.selected.clone() {
            if let Ok(copy) = self.draft.copy_def(&n) {
                made.push(copy);
            }
        }
        let n = made.len();
        if n > 0 {
            self.selected = made.clone();
            self.anchor = made.first().cloned();
            self.sort_selection();
        }
        n
    }

    /// Rename the (single) selected definition.
    pub fn rename_selected(&mut self, new: &str) -> Result<(), CatalogError> {
        let [only] = self.selected.as_slice() else {
            return Err(CatalogError::Missing);
        };
        let only = only.clone();
        self.draft.rename_def(&only, new)?;
        self.selected = vec![new.trim().to_string()];
        self.anchor = self.selected.first().cloned();
        Ok(())
    }

    /// Delete: the selected definitions that are not in use go; the message
    /// says where the others are used. Returns how many were deleted.
    pub fn delete_selected(&mut self) -> usize {
        let mut deleted = 0;
        let mut refused = Vec::new();
        for n in self.selected.clone() {
            match self.draft.delete_def(&n) {
                Ok(()) => deleted += 1,
                Err(CatalogError::InUse(w)) => refused.push(format!("\"{n}\" is in use: {w}")),
                Err(_) => {}
            }
        }
        self.selected.retain(|n| self.draft.def_named(n).is_some());
        self.message = refused.first().cloned().unwrap_or_default();
        deleted
    }

    /// Merge: the first selected definition stays and replaces the others.
    pub fn merge_selected(&mut self) -> usize {
        if self.selected.len() < 2 {
            return 0;
        }
        let names = self.selected.clone();
        let n = self.draft.merge_defs(&names).unwrap_or(0);
        self.selected = names.into_iter().take(1).collect();
        n
    }

    pub fn purge(&mut self) -> usize {
        let n = self.draft.purge_defs();
        self.selected.retain(|x| self.draft.def_named(x).is_some());
        self.message = format!(
            "Purged {n} unused definition{}",
            if n == 1 { "" } else { "s" }
        );
        n
    }

    /// Why OK is not allowed.
    fn error(&self) -> Option<String> {
        self.draft
            .defs
            .is_empty()
            .then(|| "At least one Default Framing Member is required".to_string())
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        // The editor and the Rename prompt sit on top of the list.
        if self.editor.is_some() {
            self.show_editor(ctx);
        }
        if let Some(text) = self.prompt.as_mut() {
            let only = self.selected.first().cloned().unwrap_or_default();
            let t = text.trim().to_string();
            let err = if t.is_empty() {
                Some("A name is required")
            } else if t != only && self.draft.def_named(&t).is_some() {
                Some("That name is already used")
            } else {
                None
            };
            match name_prompt(ctx, "Rename Framing Member", text, err) {
                Outcome::Open => {}
                Outcome::Cancel => self.prompt = None,
                Outcome::Ok => {
                    let new = self.prompt.take().unwrap_or_default();
                    if let Err(e) = self.rename_selected(&new) {
                        self.message = e.to_string();
                    }
                }
            }
        }
        let enabled = self.editor.is_none() && self.prompt.is_none();
        let error = self.error();
        let me = &mut *self;
        let mut apply = None;
        let outcome = window(
            ctx,
            "Framing Member Defaults Management",
            "members",
            [760.0, 460.0],
            enabled,
            error.as_deref(),
            |ui| apply = me.body(ui),
        );
        if let Some(a) = apply {
            self.apply = Some(a);
            return Outcome::Ok;
        }
        outcome
    }

    /// The list and its buttons. Returns the definition to Apply to Selected
    /// Members when that button was clicked.
    fn body(&mut self, ui: &mut Ui) -> Option<String> {
        let mut apply = None;
        let mut click: Option<(String, Click)> = None;
        ui.label("Available Framing Members");
        egui::ScrollArea::vertical()
            .id_salt("framing_members_list")
            .max_height((ui.available_height() - 90.0).max(80.0))
            .show(ui, |ui| {
                egui::Grid::new("framing_members_grid")
                    .striped(true)
                    .num_columns(6)
                    .show(ui, |ui| {
                        for h in ["Name", "Type", "Material", "Role", "Category", "In Use"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        let mut defs: Vec<&FramingMemberDef> = self.draft.defs.iter().collect();
                        defs.sort_by_key(|d| d.name.to_lowercase());
                        for d in defs {
                            let sel = self.selected.contains(&d.name);
                            let r = ui.selectable_label(sel, &d.name);
                            if r.clicked() {
                                let m = ui.input(|i| i.modifiers);
                                let how = if m.shift {
                                    Click::Range
                                } else if m.command || m.ctrl {
                                    Click::Toggle
                                } else {
                                    Click::Plain
                                };
                                click = Some((d.name.clone(), how));
                            }
                            ui.label(&d.framing_type);
                            ui.label(&d.material);
                            ui.label(d.role.name());
                            ui.label(d.category.name());
                            ui.label(if self.draft.def_in_use(&d.name) {
                                "\u{2713}"
                            } else {
                                ""
                            });
                            ui.end_row();
                        }
                    });
            });
        if let Some((n, how)) = click {
            self.click(&n, how);
        }
        if !self.message.is_empty() {
            ui.colored_label(ERROR_RED, &self.message);
        }
        ui.separator();
        let one = self.selected.len() == 1;
        let some = !self.selected.is_empty();
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(some, egui::Button::new("Edit...")).clicked() {
                self.message.clear();
                self.edit_selected();
            }
            if ui.add_enabled(some, egui::Button::new("Copy")).clicked() {
                self.message.clear();
                self.copy_selected();
            }
            if ui
                .add_enabled(one, egui::Button::new("Rename..."))
                .clicked()
            {
                self.message.clear();
                self.prompt = self.selected.first().cloned();
            }
            if ui.add_enabled(some, egui::Button::new("Delete")).clicked() {
                self.delete_selected();
            }
            if ui
                .add_enabled(self.selected.len() > 1, egui::Button::new("Merge"))
                .clicked()
            {
                self.message.clear();
                self.merge_selected();
            }
            if ui.button("Select All").clicked() {
                self.select_all();
            }
            if ui.button("Clear All").clicked() {
                self.clear_all();
            }
            if ui.button("Purge").clicked() {
                self.purge();
            }
        });
        ui.horizontal(|ui| {
            let enabled = one;
            if ui
                .add_enabled(enabled, egui::Button::new("Apply to Selected Members"))
                .on_hover_text(
                    "Gives the selected framing members the type, Role and material of this definition (OK and apply)",
                )
                .clicked()
            {
                apply = self.selected.first().cloned();
            }
        });
        apply
    }

    fn show_editor(&mut self, ctx: &egui::Context) {
        let types: Vec<String> = self.draft.types.iter().map(|t| t.name.clone()).collect();
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        let single = ed.targets.len() == 1;
        let title = if single {
            "Framing Member Defaults"
        } else {
            "Multiple Framing Member Defaults"
        };
        let name_taken = single
            && ed.def.name.trim() != ed.targets[0]
            && self.draft.def_named(ed.def.name.trim()).is_some();
        let error = if single && ed.def.name.trim().is_empty() {
            Some("A name is required")
        } else if name_taken {
            Some("That name is already used")
        } else {
            None
        };
        let mut ok = false;
        let mut cancel = false;
        let mut open = true;
        egui::Window::new(title)
            .id(egui::Id::new("framing_member_editor"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                if single {
                    row(ui, "Name", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut ed.def.name).desired_width(220.0));
                    });
                } else {
                    ui.weak(format!("Editing {} definitions", ed.targets.len()));
                }
                row(ui, "Framing Type", |ui| {
                    name_combo(ui, "fm_type", &mut ed.def.framing_type, &types)
                });
                row(ui, "Material", |ui| {
                    ui.add(egui::TextEdit::singleline(&mut ed.def.material).desired_width(220.0));
                });
                row(ui, "Role", |ui| {
                    enum_combo(ui, "fm_role", &mut ed.def.role, &Role::ALL, Role::name)
                });
                row(ui, "Materials List Category", |ui| {
                    let mut opts = vec![CategoryChoice::Auto];
                    opts.extend(MaterialsCategory::ALL.map(CategoryChoice::Fixed));
                    enum_combo(ui, "fm_cat", &mut ed.def.category, &opts, CategoryChoice::name)
                });
                ui.weak("A definition has a type, material and Role but no size, so one serves several uses.");
                ui.add_space(6.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(error.is_none(), egui::Button::new("   OK   "))
                        .clicked()
                    {
                        ok = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    if let Some(e) = error {
                        ui.colored_label(ERROR_RED, e);
                    }
                });
            });
        if !open || cancel || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.editor = None;
        } else if ok {
            let _ = self.editor_ok();
        }
    }
}

// ----- Manual Framing Defaults -----

const MANUAL_TABS: &[Tab] = &[on("General"), on("Beams"), on("Posts"), on("Materials")];

/// The Manual Framing Defaults dialog (General, Beams, Posts, Materials).
pub struct ManualDialog {
    frame: SpecDialog,
    form: ManualForm,
}

struct ManualForm {
    draft: FramingCatalog,
}

impl ManualDialog {
    pub fn new(catalog: FramingCatalog) -> Self {
        Self {
            frame: SpecDialog::new("Manual Framing Defaults", "manual_framing"),
            form: ManualForm { draft: catalog },
        }
    }

    pub fn draft(&self) -> &FramingCatalog {
        &self.form.draft
    }

    pub fn draft_mut(&mut self) -> &mut FramingCatalog {
        &mut self.form.draft
    }

    pub fn start_on(&mut self, tab: usize) {
        self.frame.start_on(tab.min(MANUAL_TABS.len() - 1));
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }
}

fn def_names(c: &FramingCatalog) -> Vec<String> {
    c.defs.iter().map(|d| d.name.clone()).collect()
}

/// Construction, Ply Width, Total Width, Depth and Ply Count rows.
fn section_rows(
    ui: &mut Ui,
    salt: &str,
    names: &[String],
    s: &mut plan_framing::catalog::SectionDefault,
    show_match: bool,
) {
    row(ui, "Construction", |ui| {
        name_combo(ui, &format!("{salt}_con"), &mut s.construction, names);
        if ui.small_button("Define...").clicked() {
            request_members();
        }
    });
    row(ui, "Ply Width", |ui| {
        inches(ui, &mut s.ply_width, 0.25, 48.0);
        ui.weak(format!("Total Width {:.3}\"", s.total_width()));
    });
    if show_match {
        ui.checkbox(&mut s.match_depth, "Match Depth");
        if s.match_depth {
            s.depth = s.ply_width;
        }
    }
    row(ui, "Depth", |ui| {
        ui.add_enabled_ui(!(show_match && s.match_depth), |ui| {
            inches(ui, &mut s.depth, 0.25, 96.0)
        });
    });
    row(ui, "Ply Count", |ui| {
        ui.add(egui::DragValue::new(&mut s.plies).range(1..=8));
    });
}

impl ManualForm {
    fn general(&mut self, ui: &mut Ui) {
        let names = def_names(&self.draft);
        section(ui, "General Framing");
        let s = &mut self.draft.manual.general;
        row(ui, "Construction", |ui| {
            name_combo(ui, "mf_gen_con", &mut s.construction, &names);
            if ui.small_button("Define...").clicked() {
                request_members();
            }
        });
        row(ui, "Width", |ui| inches(ui, &mut s.ply_width, 0.25, 48.0));
        row(ui, "Depth", |ui| inches(ui, &mut s.depth, 0.25, 96.0));
        ui.weak("These set the initial attributes of General Framing drawn from now on.");
    }

    fn beams(&mut self, ui: &mut Ui) {
        let names = def_names(&self.draft);
        section(ui, "Floor/Ceiling Beams");
        section_rows(
            ui,
            "mf_fb",
            &names,
            &mut self.draft.manual.floor_beam,
            false,
        );
        section(ui, "Roof Beams");
        section_rows(ui, "mf_rb", &names, &mut self.draft.manual.roof_beam, false);
        section(ui, "Options");
        let m = &mut self.draft.manual;
        row(ui, "Placement", |ui| {
            enum_combo(
                ui,
                "mf_place",
                &mut m.placement,
                &BeamPlacement::ALL,
                BeamPlacement::name,
            )
        });
        row(ui, "Align Exterior with", |ui| {
            enum_combo(
                ui,
                "mf_align",
                &mut m.align_exterior,
                &AlignExterior::ALL,
                AlignExterior::name,
            )
        });
        ui.weak("With Joists: the beam is as tall as the joists and they hang on it. Under Joists: the joists bear on it.");
    }

    fn posts(&mut self, ui: &mut Ui) {
        let names = def_names(&self.draft);
        section(ui, "Posts");
        section_rows(ui, "mf_post", &names, &mut self.draft.manual.post, true);
        section(ui, "Post with Footings");
        section_rows(
            ui,
            "mf_pwf",
            &names,
            &mut self.draft.manual.post_footing,
            true,
        );
        let m = &mut self.draft.manual;
        row(ui, "Footing Height Above Floor", |ui| {
            inches(ui, &mut m.footing_height, 0.0, 96.0)
        });
        row(ui, "Footing Thickness", |ui| {
            inches(ui, &mut m.footing_thickness, 1.0, 96.0)
        });
        row(ui, "Footing Width", |ui| {
            inches(ui, &mut m.footing_width, 1.0, 240.0)
        });
        row(ui, "Footing Shape", |ui| {
            ui.radio_value(
                &mut m.footing_shape,
                plan_framing::catalog::FootingShape::Square,
                "Square",
            );
            ui.radio_value(
                &mut m.footing_shape,
                plan_framing::catalog::FootingShape::Round,
                "Round",
            );
        });
        row(ui, "Footing Rebar Size Number", |ui| {
            ui.add(egui::DragValue::new(&mut m.rebar_size).range(2..=18));
        });
        row(ui, "Footing Rebar Count", |ui| {
            ui.add(egui::DragValue::new(&mut m.rebar_count).range(0..=40));
        });
    }

    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Materials");
        ui.weak("The material of a framing object affects only how it looks in 3D views and in framing schedules; the Materials List comes from its type.");
        for (label, con) in [
            (
                "General Framing",
                self.draft.manual.general.construction.clone(),
            ),
            (
                "Floor/Ceiling Beams",
                self.draft.manual.floor_beam.construction.clone(),
            ),
            (
                "Roof Beams",
                self.draft.manual.roof_beam.construction.clone(),
            ),
            ("Posts", self.draft.manual.post.construction.clone()),
        ] {
            let mat = self
                .draft
                .def_named(&con)
                .map_or("(missing definition)", |d| d.material.as_str());
            row(ui, label, |ui| ui.label(format!("{mat}  ({con})")));
        }
    }
}

impl SpecPages for ManualForm {
    fn tabs(&self) -> &'static [Tab] {
        MANUAL_TABS
    }

    fn error(&self) -> Option<String> {
        let m = &self.draft.manual;
        [
            &m.general,
            &m.floor_beam,
            &m.roof_beam,
            &m.post,
            &m.post_footing,
        ]
        .iter()
        .find(|s| self.draft.def_named(&s.construction).is_none())
        .map(|s| format!("\"{}\" is not a Default Framing Member", s.construction))
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.general(ui),
            1 => self.beams(ui),
            2 => self.posts(ui),
            _ => self.materials(ui),
        }
    }

    fn preview(&self, painter: &egui::Painter, rect: egui::Rect) {
        auto::sketch_members(painter, rect);
    }
}

// ----- window state -----

enum Open {
    Members(Box<MembersDialog>),
    Automatic(Box<AutoDialog>),
    Manual(Box<ManualDialog>),
}

thread_local! {
    /// What is asked for and what is open.
    static REQUEST: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static AUTO_TAB: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
}

/// Asks for the Framing Member Defaults Management dialog.
pub fn request_members() {
    REQUEST.with(|r| r.borrow_mut().push(MEMBERS));
}

/// Asks for the Automatic Framing Defaults dialog (`tab` is the panel index:
/// 0 Foundation, then the floor levels, Deck ... Trusses).
pub fn request_automatic(tab: usize) {
    AUTO_TAB.with(|t| t.set(tab));
    REQUEST.with(|r| r.borrow_mut().push(AUTOMATIC));
}

/// Asks for the Manual Framing Defaults dialog.
pub fn request_manual() {
    REQUEST.with(|r| r.borrow_mut().push(MANUAL));
}

/// Runs the framing dialog commands; false for any other id.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        MEMBERS => request_members(),
        AUTOMATIC => request_automatic(0),
        MANUAL => request_manual(),
        TYPES => super::framing_types::request(),
        REPORTING => super::member_reporting::request_saved(),
        _ => match id.strip_prefix(APPLY_PREFIX) {
            Some(def) => {
                apply_default_to_selection(cx, def);
            }
            None => return false,
        },
    }
    true
}

/// Draws the framing dialogs that are open; call once a frame.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    super::framing_types::show(ctx, cx);
    super::member_reporting::show(ctx, cx);
    for req in REQUEST.with(|r| std::mem::take(&mut *r.borrow_mut())) {
        let open = match req {
            MEMBERS => Open::Members(Box::new(MembersDialog::new(catalog_of(cx)))),
            AUTOMATIC => {
                let settings = framing_view::settings(&cx.project);
                let mut d = AutoDialog::new(&settings, catalog_of(cx));
                d.start_on(AUTO_TAB.with(std::cell::Cell::get));
                Open::Automatic(Box::new(d))
            }
            _ => Open::Manual(Box::new(ManualDialog::new(catalog_of(cx)))),
        };
        OPEN.with(|o| *o.borrow_mut() = Some(open));
    }
    let Some(open) = OPEN.with(|o| o.borrow_mut().take()) else {
        return;
    };
    let keep = match open {
        Open::Members(mut d) => match d.show(ctx) {
            Outcome::Open => Some(Open::Members(d)),
            Outcome::Cancel => None,
            Outcome::Ok => {
                if set_catalog(cx, &d.draft, "Framing Member Defaults") {
                    cx.status = "Saved the Framing Member Defaults".into();
                }
                if let Some(def) = d.apply.take() {
                    apply_default_to_selection(cx, &def);
                }
                None
            }
        },
        Open::Automatic(mut d) => match d.show(ctx) {
            Outcome::Open => Some(Open::Automatic(d)),
            Outcome::Cancel => None,
            Outcome::Ok => {
                commit_automatic(cx, d.settings(), d.catalog());
                None
            }
        },
        Open::Manual(mut d) => match d.show(ctx) {
            Outcome::Open => Some(Open::Manual(d)),
            Outcome::Cancel => None,
            Outcome::Ok => {
                if set_catalog(cx, d.draft(), "Manual Framing Defaults") {
                    cx.status = "Saved the Manual Framing Defaults".into();
                }
                None
            }
        },
    };
    OPEN.with(|o| *o.borrow_mut() = keep);
}

/// Are any of the framing default dialogs open or asked for?
#[cfg(test)]
pub(crate) fn is_open() -> bool {
    OPEN.with(|o| o.borrow().is_some()) || REQUEST.with(|r| !r.borrow().is_empty())
}

/// OK of the Automatic Framing Defaults: the settings and the catalog are
/// stored as one undo step.
pub fn commit_automatic(
    cx: &mut EditorContext,
    settings: &framing_view::FramingSettings,
    catalog: &FramingCatalog,
) {
    let settings_changed = framing_view::settings(&cx.project) != *settings;
    let catalog_changed = !catalog::project_has_catalog(&cx.project) || catalog_of(cx) != *catalog;
    if settings_changed {
        // Opens the step; the catalog joins it.
        framing_view::set_settings(cx, settings.clone());
    }
    if catalog_changed {
        if !settings_changed {
            cx.begin_change("Automatic Framing Defaults");
        }
        store_catalog_raw(cx, catalog);
        cx.mark_dirty();
        cx.refresh();
    }
    cx.status = "Saved the Automatic Framing Defaults; Build Framing applies them".into();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dlg() -> MembersDialog {
        MembersDialog::new(FramingCatalog::default())
    }

    #[test]
    fn the_list_selects_with_plain_toggle_and_range_clicks() {
        let mut d = dlg();
        d.click("Plates", Click::Plain);
        assert_eq!(d.selected(), ["Plates"]);
        d.click("Blocking", Click::Toggle);
        assert_eq!(d.selected(), ["Plates", "Blocking"]);
        d.click("Studs", Click::Plain);
        d.click("Blocking", Click::Range);
        assert_eq!(d.selected(), ["Studs", "Plates", "Headers", "Blocking"]);
        d.clear_all();
        assert!(d.selected().is_empty());
        d.select_all();
        assert_eq!(d.selected().len(), d.draft.defs.len());
    }

    #[test]
    fn copy_rename_edit_and_delete_work_on_the_selection() {
        let mut d = dlg();
        d.click("Joists - I Joists", Click::Plain);
        assert_eq!(d.copy_selected(), 1);
        assert_eq!(d.selected(), ["Joists - I Joists 2"]);
        d.rename_selected("Floor I-Joists").unwrap();
        assert!(d.draft.def_named("Floor I-Joists").is_some());
        assert!(d.rename_selected("Studs").is_err(), "names are unique");
        assert!(d.edit_selected());
        d.editor.as_mut().unwrap().def.framing_type = "LVL".into();
        d.editor.as_mut().unwrap().def.name = "Floor LVL".into();
        d.editor_ok().unwrap();
        assert_eq!(d.draft.def_named("Floor LVL").unwrap().framing_type, "LVL");
        assert_eq!(d.delete_selected(), 1, "an unused definition is deleted");
        d.click("Studs", Click::Plain);
        assert_eq!(d.delete_selected(), 0, "an in-use one stays");
        assert!(d.message.contains("in use"), "{}", d.message);
    }

    #[test]
    fn multiple_edit_sets_the_type_of_every_selected_definition() {
        let mut d = dlg();
        d.click("Posts", Click::Plain);
        d.click("Posts - Steel", Click::Toggle);
        assert!(d.edit_selected());
        d.editor.as_mut().unwrap().def.framing_type = "PSL".into();
        d.editor_ok().unwrap();
        assert_eq!(d.draft.def_named("Posts").unwrap().framing_type, "PSL");
        assert_eq!(
            d.draft.def_named("Posts - Steel").unwrap().framing_type,
            "PSL"
        );
        assert_eq!(
            d.draft.def_named("Posts - Steel").unwrap().name,
            "Posts - Steel"
        );
    }

    #[test]
    fn merge_keeps_the_first_and_purge_drops_the_unused() {
        let mut d = dlg();
        d.click("Joists", Click::Plain);
        d.click("Joists - I Joists", Click::Toggle);
        assert_eq!(d.merge_selected(), 1);
        assert!(d.draft.def_named("Joists - I Joists").is_none());
        let before = d.draft.defs.len();
        let purged = d.purge();
        assert!(purged > 0);
        assert_eq!(d.draft.defs.len(), before - purged);
        assert!(d.draft.def_named("Studs").is_some());
    }
}
