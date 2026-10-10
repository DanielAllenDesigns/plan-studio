//! The Framing Type Management dialog and the Framing Type dialog (manual
//! pp. 887-889), Default Settings > Framing > Framing Types.
//!
//! The management dialog lists the Framing Types of the plan (sortable by
//! any column, with an In Use column) with New, Edit, Copy, Rename and
//! Delete; Delete refuses a type in use and says where it is used. The
//! Framing Type dialog sets the Composition (wood, steel, concrete, other),
//! the Shape that composition offers, the Supporting Type of Steel C (the
//! U Channel of its plates), Display Nominal Sizes and Include Name in
//! Labels. OK stores the list as one undo step and gives the plan's
//! automatic members the shape of their (edited) types.

use super::framing_defaults::{
    catalog_of, enum_combo, name_combo, name_prompt, set_catalog, window,
};
use super::{row, Outcome, ERROR_RED};
use crate::editor::EditorContext;
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers};
use plan_framing::catalog::{Composition, FramingCatalog, FramingShape, FramingType};
use plan_framing::CatalogError;
use std::cell::RefCell;

/// A column of the list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Column {
    Name,
    Composition,
    Shape,
    InUse,
}

/// The editor of one Framing Type (the Framing Type dialog).
pub struct TypeEditor {
    /// The type being edited by name; `None` for a new type.
    pub target: Option<String>,
    pub ty: FramingType,
}

/// The Framing Type Management dialog.
pub struct TypesDialog {
    pub draft: FramingCatalog,
    selected: Option<String>,
    sort: (Column, bool),
    pub editor: Option<TypeEditor>,
    prompt: Option<String>,
    pub message: String,
}

impl TypesDialog {
    pub fn new(catalog: FramingCatalog) -> Self {
        let first = catalog.types.first().map(|t| t.name.clone());
        Self {
            draft: catalog,
            selected: first,
            sort: (Column::Name, true),
            editor: None,
            prompt: None,
            message: String::new(),
        }
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn select(&mut self, name: &str) {
        if self.draft.type_named(name).is_some() {
            self.selected = Some(name.to_string());
        }
    }

    /// Click on a column header: sorts by it, a second click reverses.
    pub fn sort_by(&mut self, c: Column) {
        self.sort = if self.sort.0 == c {
            (c, !self.sort.1)
        } else {
            (c, true)
        };
    }

    pub fn sorted_by(&self) -> (Column, bool) {
        self.sort
    }

    /// The types in the order the list shows them.
    pub fn rows(&self) -> Vec<&FramingType> {
        let mut v: Vec<&FramingType> = self.draft.types.iter().collect();
        let (col, asc) = self.sort;
        v.sort_by(|a, b| {
            let o = match col {
                Column::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                Column::Composition => a.composition.name().cmp(b.composition.name()),
                Column::Shape => a.shape.name().cmp(b.shape.name()),
                Column::InUse => self
                    .draft
                    .type_in_use(&a.name)
                    .cmp(&self.draft.type_in_use(&b.name)),
            }
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            if asc {
                o
            } else {
                o.reverse()
            }
        });
        v
    }

    /// New: opens the Framing Type dialog on a fresh type.
    pub fn new_type(&mut self) {
        let name = (1..)
            .map(|n| format!("Framing Type {n}"))
            .find(|n| self.draft.type_named(n).is_none())
            .unwrap_or_default();
        self.editor = Some(TypeEditor {
            target: None,
            ty: FramingType {
                name,
                ..FramingType::default()
            },
        });
    }

    /// Edit: opens the Framing Type dialog on the selected type.
    pub fn edit_selected(&mut self) -> bool {
        let Some(t) = self.selected.as_deref().and_then(|n| self.draft.type_named(n)) else {
            return false;
        };
        self.editor = Some(TypeEditor {
            target: Some(t.name.clone()),
            ty: t.clone(),
        });
        true
    }

    /// Copy: a new type based on the selected one, opened in its dialog to
    /// be customised (the copy is already in the list).
    pub fn copy_selected(&mut self) -> Option<String> {
        let name = self.selected.clone()?;
        let copy = self.draft.copy_type(&name).ok()?;
        self.selected = Some(copy.clone());
        self.edit_selected();
        Some(copy)
    }

    /// OK in the Framing Type dialog.
    pub fn editor_ok(&mut self) -> Result<(), CatalogError> {
        let Some(ed) = self.editor.take() else {
            return Ok(());
        };
        let r = match &ed.target {
            Some(t) => self.draft.edit_type(t, ed.ty.clone()),
            None => self.draft.add_type(ed.ty.clone()),
        };
        match &r {
            Ok(()) => self.selected = Some(ed.ty.name.trim().to_string()),
            Err(e) => {
                self.message = e.to_string();
                self.editor = Some(ed);
            }
        }
        r
    }

    pub fn rename_selected(&mut self, new: &str) -> Result<(), CatalogError> {
        let old = self.selected.clone().ok_or(CatalogError::Missing)?;
        self.draft.rename_type(&old, new)?;
        self.selected = Some(new.trim().to_string());
        Ok(())
    }

    /// Delete: refuses a type in use and says where it is used.
    pub fn delete_selected(&mut self) -> Result<(), CatalogError> {
        let name = self.selected.clone().ok_or(CatalogError::Missing)?;
        let r = self.draft.delete_type(&name);
        match &r {
            Ok(()) => {
                self.message.clear();
                self.selected = self.draft.types.first().map(|t| t.name.clone());
            }
            Err(CatalogError::InUse(w)) => {
                self.message = format!("\"{name}\" is in use: {w}");
            }
            Err(e) => self.message = e.to_string(),
        }
        r
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        if self.editor.is_some() {
            self.show_editor(ctx);
        }
        if let Some(text) = self.prompt.as_mut() {
            let old = self.selected.clone().unwrap_or_default();
            let t = text.trim().to_string();
            let err = if t.is_empty() {
                Some("A name is required")
            } else if t != old && self.draft.type_named(&t).is_some() {
                Some("That name is already used")
            } else {
                None
            };
            match name_prompt(ctx, "Rename Framing Type", text, err) {
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
        let me = &mut *self;
        window(
            ctx,
            "Framing Type Management",
            "types",
            [680.0, 420.0],
            enabled,
            None,
            |ui| me.body(ui),
        )
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        ui.label("Available Framing Types");
        let mut sort = None;
        let mut pick = None;
        let mut edit = false;
        egui::ScrollArea::vertical()
            .id_salt("framing_types_list")
            .max_height((ui.available_height() - 70.0).max(80.0))
            .show(ui, |ui| {
                egui::Grid::new("framing_types_grid")
                    .striped(true)
                    .num_columns(4)
                    .show(ui, |ui| {
                        for (c, label) in [
                            (Column::Name, "Name"),
                            (Column::Composition, "Composition"),
                            (Column::Shape, "Shape"),
                            (Column::InUse, "In Use"),
                        ] {
                            let arrow = if self.sort.0 == c {
                                if self.sort.1 {
                                    " \u{25B2}"
                                } else {
                                    " \u{25BC}"
                                }
                            } else {
                                ""
                            };
                            if ui
                                .add(egui::Button::new(egui::RichText::new(format!("{label}{arrow}")).strong()).frame(false))
                                .clicked()
                            {
                                sort = Some(c);
                            }
                        }
                        ui.end_row();
                        for t in self.rows() {
                            let sel = self.selected.as_deref() == Some(&t.name);
                            let r = ui.selectable_label(sel, &t.name);
                            if r.clicked() {
                                pick = Some(t.name.clone());
                            }
                            if r.double_clicked() {
                                edit = true;
                            }
                            ui.label(t.composition.name());
                            ui.label(t.shape.name());
                            ui.label(if self.draft.type_in_use(&t.name) { "\u{2713}" } else { "" });
                            ui.end_row();
                        }
                    });
            });
        if let Some(c) = sort {
            self.sort_by(c);
        }
        if let Some(n) = pick {
            self.select(&n);
            self.message.clear();
        }
        if edit {
            self.edit_selected();
        }
        if !self.message.is_empty() {
            ui.colored_label(ERROR_RED, &self.message);
        }
        ui.separator();
        let some = self.selected.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui.button("New...").clicked() {
                self.message.clear();
                self.new_type();
            }
            if ui.add_enabled(some, egui::Button::new("Edit...")).clicked() {
                self.message.clear();
                self.edit_selected();
            }
            if ui.add_enabled(some, egui::Button::new("Copy...")).clicked() {
                self.message.clear();
                self.copy_selected();
            }
            if ui.add_enabled(some, egui::Button::new("Rename...")).clicked() {
                self.message.clear();
                self.prompt = self.selected.clone();
            }
            if ui.add_enabled(some, egui::Button::new("Delete")).clicked() {
                let _ = self.delete_selected();
            }
        });
    }

    fn show_editor(&mut self, ctx: &egui::Context) {
        let steel_u: Vec<String> = self
            .draft
            .types
            .iter()
            .filter(|t| t.shape == FramingShape::UChannel)
            .map(|t| t.name.clone())
            .collect();
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        let target = ed.target.clone();
        let name = ed.ty.name.trim().to_string();
        let error = if name.is_empty() {
            Some("A name is required")
        } else if target.as_deref() != Some(&name) && self.draft.type_named(&name).is_some() {
            Some("That name is already used")
        } else {
            None
        };
        let mut ok = false;
        let mut cancel = false;
        let mut open = true;
        egui::Window::new("Framing Type")
            .id(egui::Id::new("framing_type_editor"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                row(ui, "Name", |ui| {
                    ui.add(egui::TextEdit::singleline(&mut ed.ty.name).desired_width(220.0));
                });
                row(ui, "Composition", |ui| {
                    enum_combo(ui, "ft_comp", &mut ed.ty.composition, &Composition::ALL, Composition::name)
                });
                // The shapes the composition offers; a shape it lacks is
                // replaced by its first.
                ed.ty.normalize();
                row(ui, "Shape", |ui| {
                    enum_combo(
                        ui,
                        "ft_shape",
                        &mut ed.ty.shape,
                        ed.ty.composition.shapes(),
                        FramingShape::name,
                    )
                });
                ed.ty.normalize();
                if ed.ty.shape == FramingShape::CChannel {
                    row(ui, "Supporting Type", |ui| {
                        let mut cur = ed.ty.supporting_type.clone().unwrap_or_default();
                        name_combo(ui, "ft_support", &mut cur, &steel_u);
                        ed.ty.supporting_type = (!cur.is_empty()).then_some(cur);
                    });
                    ui.weak("The U Channel type of the top and bottom wall plates.");
                }
                ui.checkbox(&mut ed.ty.display_nominal, "Display Nominal Sizes")
                    .on_hover_text("U.S. plans list the nominal size in the Materials List instead of the actual dimensions");
                ui.checkbox(&mut ed.ty.include_name_in_labels, "Include Name in Labels")
                    .on_hover_text("Puts the type's name in object labels and the Materials List description");
                ui.weak("Whatever the composition, any material can be assigned for display.");
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
            // A Copy already added its type; Cancel leaves the copy in the
            // list (it can be deleted).
            self.editor = None;
        } else if ok {
            let _ = self.editor_ok();
        }
    }
}

// ----- window state -----

thread_local! {
    static REQUESTED: RefCell<bool> = const { RefCell::new(false) };
    static OPEN: RefCell<Option<TypesDialog>> = const { RefCell::new(None) };
}

/// Asks for the Framing Type Management dialog.
pub fn request() {
    REQUESTED.with(|r| *r.borrow_mut() = true);
}

/// Draws the dialog if it is open; call once a frame.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if REQUESTED.with(|r| std::mem::take(&mut *r.borrow_mut())) {
        OPEN.with(|o| *o.borrow_mut() = Some(TypesDialog::new(catalog_of(cx))));
    }
    let Some(mut d) = OPEN.with(|o| o.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => OPEN.with(|o| *o.borrow_mut() = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            if set_catalog(cx, &d.draft, "Framing Types") {
                cx.status = "Saved the Framing Types".into();
            }
        }
    }
}

/// Is the dialog open or asked for?
#[cfg(test)]
pub(crate) fn is_open() -> bool {
    REQUESTED.with(|r| *r.borrow()) || OPEN.with(|o| o.borrow().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dlg() -> TypesDialog {
        TypesDialog::new(FramingCatalog::default())
    }

    #[test]
    fn the_list_sorts_by_a_column_and_a_second_click_reverses_it() {
        let mut d = dlg();
        let names = |d: &TypesDialog| d.rows().iter().map(|t| t.name.clone()).collect::<Vec<_>>();
        let asc = names(&d);
        assert_eq!(asc[0], "Engineered Lumber");
        d.sort_by(Column::Name);
        assert_eq!(d.sorted_by(), (Column::Name, false));
        let mut rev = asc.clone();
        rev.reverse();
        assert_eq!(names(&d), rev);
        d.sort_by(Column::Composition);
        assert_eq!(d.rows()[0].composition, Composition::Concrete);
        d.sort_by(Column::InUse);
        assert!(!d.draft.type_in_use(&d.rows()[0].name), "unused first");
    }

    #[test]
    fn edit_changes_the_type_and_the_shape_follows_the_composition() {
        let mut d = dlg();
        d.select("Lumber");
        assert!(d.edit_selected());
        let ed = d.editor.as_mut().unwrap();
        ed.ty.shape = FramingShape::IJoist;
        d.editor_ok().unwrap();
        assert_eq!(d.draft.type_named("Lumber").unwrap().shape, FramingShape::IJoist);
        // A steel composition cannot keep a wood shape.
        d.edit_selected();
        let ed = d.editor.as_mut().unwrap();
        ed.ty.composition = Composition::Steel;
        ed.ty.normalize();
        assert_eq!(ed.ty.shape, FramingShape::SteelI);
        d.editor_ok().unwrap();
        assert_eq!(d.draft.type_named("Lumber").unwrap().composition, Composition::Steel);
    }

    #[test]
    fn copy_makes_a_new_type_to_customise_and_rename_updates_references() {
        let mut d = dlg();
        d.select("Glulam");
        let copy = d.copy_selected().unwrap();
        assert_eq!(copy, "Glulam 2");
        assert_eq!(d.editor.as_ref().unwrap().target.as_deref(), Some("Glulam 2"));
        d.editor_ok().unwrap();
        d.rename_selected("Glulam Heavy").unwrap();
        assert!(d.draft.type_named("Glulam Heavy").is_some());
        // Renaming a type a definition uses renames it there too.
        d.select("Lumber");
        d.rename_selected("Dimension Lumber").unwrap();
        assert_eq!(d.draft.def_named("Studs").unwrap().framing_type, "Dimension Lumber");
    }

    #[test]
    fn new_adds_a_type_with_a_free_name_and_refuses_a_duplicate() {
        let mut d = dlg();
        d.new_type();
        assert_eq!(d.editor.as_ref().unwrap().ty.name, "Framing Type 1");
        d.editor.as_mut().unwrap().ty.name = "Lumber".into();
        assert_eq!(d.editor_ok(), Err(CatalogError::BadName));
        assert!(d.editor.is_some(), "the dialog stays open on the error");
        d.editor.as_mut().unwrap().ty.name = "Cedar".into();
        d.editor_ok().unwrap();
        assert!(d.draft.type_named("Cedar").is_some());
    }

    #[test]
    fn delete_refuses_a_type_in_use_and_names_the_place() {
        let mut d = dlg();
        d.select("Lumber");
        assert!(matches!(d.delete_selected(), Err(CatalogError::InUse(_))));
        assert!(d.message.contains("Default Framing Member"), "{}", d.message);
        d.select("VSL");
        assert!(d.delete_selected().is_ok());
        assert!(d.draft.type_named("VSL").is_none());
    }
}
