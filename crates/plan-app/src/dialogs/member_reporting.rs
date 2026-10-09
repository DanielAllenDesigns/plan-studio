//! Structural Member Reporting (manual pp. 905-908), Default Settings >
//! Materials > Structural Member Reporting and Tools > Materials List >
//! Structural Member Reporting:
//!
//! * the **Saved Structural Member Reporting Defaults** dialog: Available
//!   Defaults with Edit, Copy/Convert, Rename, Delete, Import and Export
//!   (`.calumber` files) and the Currently Active Default;
//! * the **New Structural Member Reporting Default** dialog (name, method, and
//!   the Buy List to copy board lengths from; only one Mixed Reporting default
//!   can exist);
//! * the **Structural Member Reporting** dialog of one default: Format (the
//!   method), Length Units, the Board Sizes table with priority up and down,
//!   the Kerf Width and the long-run option, and the totals of the plan's
//!   framing under the default;
//! * the **Board Specification** dialog: Actual Thickness and Depth, Length,
//!   Type, Treated and the formula.
//!
//! Every dialog works on a draft; Cancel drops all changes made in the
//! dialog. OK stores the defaults as one undo step. The Materials List reads
//! the active default (`plan-docs` materials engine).

use super::framing_defaults::{
    catalog_of, enum_combo, inches, name_combo, name_prompt, set_catalog, window,
};
use super::{row, section, Outcome, ERROR_RED};
use crate::editor::{framing_view, EditorContext};
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers};
use plan_framing::catalog::{self, FramingCatalog};
use plan_framing::reporting::{self, auto_inputs, manual_inputs, Report, ReportInput};
use plan_framing::{
    BoardSpec, LongRun, ReportMethod, ReportUnits, ReportingDefault, ReportingError,
};
use std::cell::RefCell;
use std::rc::Rc;

/// The plan's framing as the report sees it.
#[derive(Clone, Default)]
pub struct ReportContext {
    pub inputs: Vec<ReportInput>,
    /// List Cut Header Lengths (Openings panel of the Automatic Framing
    /// Defaults).
    pub list_cut_headers: bool,
}

impl ReportContext {
    /// Every automatic and manual member of every floor.
    pub fn of_project(project: &plan_core::Project, catalog: &FramingCatalog) -> Self {
        let auto = framing_view::members_for(project, 0, true);
        let manual = framing_view::manual_for(project, 0, true);
        let mut inputs = auto_inputs(&auto, catalog, 0);
        inputs.extend(manual_inputs(&manual, catalog, auto.len()));
        Self {
            inputs,
            list_cut_headers: project
                .floors
                .first()
                .is_some_and(|f| catalog::list_cut_headers(&f.framing)),
        }
    }

    /// The report of the framing under `d`.
    pub fn report(&self, d: &ReportingDefault) -> Report {
        reporting::report(&self.inputs, d, self.list_cut_headers)
    }
}

/// The report of the plan's framing under the Currently Active Default: what
/// the Materials List shows for framing.
pub fn active_report(cx: &EditorContext) -> Report {
    let catalog = catalog_of(cx);
    ReportContext::of_project(&cx.project, &catalog).report(catalog.reporting.active_default())
}

// ----- Board Specification -----

/// The Board Specification dialog on one row of the table.
pub struct BoardEditor {
    /// The row being edited; `None` for a new row.
    pub index: Option<usize>,
    pub board: BoardSpec,
    pub units: ReportUnits,
}

// ----- New default -----

/// The New Structural Member Reporting Default dialog.
pub struct NewDefault {
    pub name: String,
    pub method: ReportMethod,
    /// The Buy List to copy board lengths from (empty: the standard boards).
    pub copy_from: String,
    /// Converting: the default being copied (the dialog says so).
    pub convert_of: Option<String>,
}

// ----- the editor of one default -----

/// The Structural Member Reporting dialog of one default.
pub struct DefaultEditor {
    /// The default being edited, by its saved name.
    pub target: String,
    pub d: ReportingDefault,
    pub selected: Option<usize>,
    pub board: Option<BoardEditor>,
    context: Rc<ReportContext>,
}

impl DefaultEditor {
    pub fn new(target: &str, d: ReportingDefault, context: Rc<ReportContext>) -> Self {
        Self {
            target: target.to_string(),
            selected: (!d.boards.is_empty()).then_some(0),
            d,
            board: None,
            context,
        }
    }

    /// The report of the plan's framing under the default being edited.
    pub fn report(&self) -> Report {
        self.context.report(&self.d)
    }

    pub fn new_board(&mut self) {
        let mut board = self
            .selected
            .and_then(|i| self.d.boards.get(i))
            .cloned()
            .unwrap_or_default();
        if !self.d.method.has_lengths() {
            board.length = 0.0;
        }
        self.board = Some(BoardEditor {
            index: None,
            board,
            units: self.d.units,
        });
    }

    pub fn edit_board(&mut self) -> bool {
        let Some(i) = self.selected else {
            return false;
        };
        let Some(b) = self.d.boards.get(i) else {
            return false;
        };
        self.board = Some(BoardEditor {
            index: Some(i),
            board: b.clone(),
            units: self.d.units,
        });
        true
    }

    /// OK in the Board Specification dialog.
    pub fn board_ok(&mut self) {
        let Some(ed) = self.board.take() else {
            return;
        };
        match ed.index {
            Some(i) if i < self.d.boards.len() => self.d.boards[i] = ed.board,
            _ => {
                self.d.boards.push(ed.board);
                self.selected = Some(self.d.boards.len() - 1);
            }
        }
    }

    pub fn delete_board(&mut self) {
        if let Some(i) = self.selected.filter(|i| *i < self.d.boards.len()) {
            self.d.boards.remove(i);
            self.selected = if self.d.boards.is_empty() {
                None
            } else {
                Some(i.min(self.d.boards.len() - 1))
            };
        }
    }

    pub fn increase_priority(&mut self) {
        if let Some(i) = self.selected {
            if self.d.priority_up(i) {
                self.selected = Some(i - 1);
            }
        }
    }

    pub fn decrease_priority(&mut self) {
        if let Some(i) = self.selected {
            if self.d.priority_down(i) {
                self.selected = Some(i + 1);
            }
        }
    }
}

// ----- the Saved Defaults dialog -----

enum Prompt {
    Rename(String),
}

/// The Saved Structural Member Reporting Defaults dialog.
pub struct SavedDialog {
    pub draft: FramingCatalog,
    pub selected: Option<String>,
    pub editor: Option<DefaultEditor>,
    pub new_default: Option<NewDefault>,
    prompt: Option<Prompt>,
    pub message: String,
    context: Rc<ReportContext>,
}

impl SavedDialog {
    pub fn new(catalog: FramingCatalog, context: ReportContext) -> Self {
        let active = catalog.reporting.active.clone();
        Self {
            draft: catalog,
            selected: Some(active),
            editor: None,
            new_default: None,
            prompt: None,
            message: String::new(),
            context: Rc::new(context),
        }
    }

    fn set(&self) -> &reporting::ReportingSet {
        &self.draft.reporting
    }

    pub fn select(&mut self, name: &str) {
        if self.set().get(name).is_some() {
            self.selected = Some(name.to_string());
        }
    }

    /// Edit: opens the Structural Member Reporting dialog on the selected
    /// default.
    pub fn edit_selected(&mut self) -> bool {
        let Some(d) = self.selected.as_deref().and_then(|n| self.set().get(n)) else {
            return false;
        };
        self.editor = Some(DefaultEditor::new(&d.name.clone(), d.clone(), self.context.clone()));
        true
    }

    /// OK in the editor: the edited default replaces the saved one.
    pub fn editor_ok(&mut self) -> Result<(), ReportingError> {
        let Some(ed) = self.editor.take() else {
            return Ok(());
        };
        let r = self.draft.reporting.replace(&ed.target, ed.d.clone());
        if let Err(e) = &r {
            self.message = e.to_string();
            self.editor = Some(ed);
        }
        r
    }

    /// Copy/Convert: opens the New dialog for a copy of the selected default.
    pub fn copy_convert(&mut self) -> bool {
        let Some(src) = self.selected.clone() else {
            return false;
        };
        let Some(d) = self.set().get(&src) else {
            return false;
        };
        let method = if d.method == ReportMethod::Mixed {
            ReportMethod::BuyList
        } else {
            d.method
        };
        let name = unique(&format!("{src} Copy"), |n| self.set().get(n).is_some());
        self.new_default = Some(NewDefault {
            name,
            method,
            copy_from: src.clone(),
            convert_of: Some(src),
        });
        true
    }

    /// New: opens the New dialog.
    pub fn new_default(&mut self) {
        let name = unique("Reporting", |n| self.set().get(n).is_some());
        self.new_default = Some(NewDefault {
            name,
            method: ReportMethod::BuyList,
            copy_from: self.set().active.clone(),
            convert_of: None,
        });
    }

    /// OK in the New dialog.
    pub fn new_ok(&mut self) -> Result<(), ReportingError> {
        let Some(n) = self.new_default.take() else {
            return Ok(());
        };
        let r = match &n.convert_of {
            Some(src) => self.draft.reporting.copy_convert(src, &n.name, n.method),
            None => {
                let from = (!n.copy_from.is_empty()).then_some(n.copy_from.as_str());
                self.draft.reporting.add(&n.name, n.method, from)
            }
        };
        match &r {
            Ok(()) => self.selected = Some(n.name.trim().to_string()),
            Err(e) => {
                self.message = e.to_string();
                self.new_default = Some(n);
            }
        }
        r
    }

    pub fn rename_selected(&mut self, new: &str) -> Result<(), ReportingError> {
        let old = self.selected.clone().ok_or(ReportingError::Missing)?;
        self.draft.reporting.rename(&old, new)?;
        self.selected = Some(new.trim().to_string());
        Ok(())
    }

    pub fn delete_selected(&mut self) -> Result<(), ReportingError> {
        let name = self.selected.clone().ok_or(ReportingError::Missing)?;
        let r = self.draft.reporting.delete(&name);
        match &r {
            Ok(()) => {
                self.message.clear();
                self.selected = Some(self.draft.reporting.active.clone());
            }
            Err(e) => self.message = e.to_string(),
        }
        r
    }

    /// Import: reads a `.calumber` file's text into the list.
    pub fn import_text(&mut self, text: &str) -> Result<String, String> {
        let d = ReportingDefault::from_json(text)
            .ok_or_else(|| "That is not a Structural Member Reporting file.".to_string())?;
        let name = self
            .draft
            .reporting
            .import(d)
            .map_err(|e| e.to_string())?;
        self.selected = Some(name.clone());
        Ok(name)
    }

    /// Export: the selected default as `.calumber` text.
    pub fn export_text(&self) -> Option<String> {
        let d = self.set().get(self.selected.as_deref()?)?;
        Some(d.to_json())
    }

    fn set_active(&mut self, name: &str) {
        self.draft.reporting.set_active(name);
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        if self.editor.is_some() {
            self.show_editor(ctx);
        }
        if self.new_default.is_some() {
            self.show_new(ctx);
        }
        if let Some(Prompt::Rename(text)) = self.prompt.as_mut() {
            let old = self.selected.clone().unwrap_or_default();
            let t = text.trim().to_string();
            let err = if t.is_empty() {
                Some("A name is required")
            } else if t != old && self.draft.reporting.get(&t).is_some() {
                Some("That name is already used (names are case-sensitive)")
            } else {
                None
            };
            match name_prompt(ctx, "Rename Current Default", text, err) {
                Outcome::Open => {}
                Outcome::Cancel => self.prompt = None,
                Outcome::Ok => {
                    let Some(Prompt::Rename(new)) = self.prompt.take() else {
                        return Outcome::Open;
                    };
                    if let Err(e) = self.rename_selected(&new) {
                        self.message = e.to_string();
                    }
                }
            }
        }
        let enabled = self.editor.is_none() && self.new_default.is_none() && self.prompt.is_none();
        let me = &mut *self;
        window(
            ctx,
            "Saved Structural Member Reporting Defaults",
            "reporting_saved",
            [620.0, 420.0],
            enabled,
            None,
            |ui| me.body(ui),
        )
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        ui.label("Available Defaults");
        let mut pick = None;
        let mut edit = false;
        egui::ScrollArea::vertical()
            .id_salt("reporting_saved_list")
            .max_height((ui.available_height() - 110.0).max(80.0))
            .show(ui, |ui| {
                egui::Grid::new("reporting_saved_grid")
                    .striped(true)
                    .num_columns(3)
                    .show(ui, |ui| {
                        for h in ["Name", "Method", "Active"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for d in &self.draft.reporting.defaults {
                            let sel = self.selected.as_deref() == Some(&d.name);
                            let r = ui.selectable_label(sel, &d.name);
                            if r.clicked() {
                                pick = Some(d.name.clone());
                            }
                            if r.double_clicked() {
                                edit = true;
                            }
                            ui.label(d.method.name());
                            ui.label(if self.draft.reporting.active == d.name { "\u{2713}" } else { "" });
                            ui.end_row();
                        }
                    });
            });
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
            if ui.add_enabled(some, egui::Button::new("Edit...")).clicked() {
                self.message.clear();
                self.edit_selected();
            }
            if ui.button("New...").clicked() {
                self.message.clear();
                self.new_default();
            }
            if ui.add_enabled(some, egui::Button::new("Copy/Convert...")).clicked() {
                self.message.clear();
                self.copy_convert();
            }
            if ui.add_enabled(some, egui::Button::new("Rename...")).clicked() {
                self.message.clear();
                self.prompt = self.selected.clone().map(Prompt::Rename);
            }
            if ui.add_enabled(some, egui::Button::new("Delete")).clicked() {
                let _ = self.delete_selected();
            }
            if ui.button("Import...").clicked() {
                self.import_file();
            }
            if ui.add_enabled(some, egui::Button::new("Export...")).clicked() {
                self.export_file();
            }
        });
        ui.add_space(6.0);
        let names: Vec<String> = self.draft.reporting.defaults.iter().map(|d| d.name.clone()).collect();
        let mut active = self.draft.reporting.active.clone();
        ui.horizontal(|ui| {
            ui.label("Currently Active Default");
            name_combo(ui, "reporting_active", &mut active, &names);
        });
        if active != self.draft.reporting.active {
            self.set_active(&active);
        }
    }

    fn import_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Structural Member Reporting", &["calumber"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match self.import_text(&text) {
                Ok(name) => self.message = format!("Imported \"{name}\""),
                Err(e) => self.message = e,
            },
            Err(e) => self.message = format!("Could not read the file: {e}"),
        }
    }

    fn export_file(&mut self) {
        let Some(text) = self.export_text() else {
            return;
        };
        let name = self.selected.clone().unwrap_or_default();
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Structural Member Reporting", &["calumber"])
            .set_file_name(format!("{name}.calumber"))
            .save_file()
        else {
            return;
        };
        self.message = match std::fs::write(&path, text) {
            Ok(()) => format!("Exported \"{name}\""),
            Err(e) => format!("Could not write the file: {e}"),
        };
    }

    fn show_new(&mut self, ctx: &egui::Context) {
        let buy_lists: Vec<String> = self
            .draft
            .reporting
            .defaults
            .iter()
            .filter(|d| d.method == ReportMethod::BuyList)
            .map(|d| d.name.clone())
            .collect();
        let has_mixed = self.draft.reporting.has_mixed();
        let taken: Vec<String> = self.draft.reporting.defaults.iter().map(|d| d.name.clone()).collect();
        let Some(n) = self.new_default.as_mut() else {
            return;
        };
        let name = n.name.trim().to_string();
        let error = if name.is_empty() {
            Some("A name is required")
        } else if taken.contains(&name) {
            Some("That name is already used (names are case-sensitive)")
        } else {
            None
        };
        let mut ok = false;
        let mut cancel = false;
        let mut open = true;
        egui::Window::new("New Structural Member Reporting Default")
            .id(egui::Id::new("reporting_new"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                row(ui, "Name", |ui| {
                    ui.add(egui::TextEdit::singleline(&mut n.name).desired_width(240.0));
                });
                ui.label("Reporting Method");
                for m in ReportMethod::ALL {
                    let can = m != ReportMethod::Mixed || !has_mixed;
                    ui.add_enabled_ui(can, |ui| {
                        ui.radio_value(&mut n.method, m, m.name());
                    });
                }
                if has_mixed {
                    ui.weak("Only one Mixed Reporting default can be present in a plan.");
                }
                let from_enabled = n.method == ReportMethod::BuyList && n.convert_of.is_none();
                ui.add_enabled_ui(from_enabled, |ui| {
                    row(ui, "Copy Board Lengths From", |ui| {
                        name_combo(ui, "reporting_copy_from", &mut n.copy_from, &buy_lists)
                    });
                });
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
            self.new_default = None;
        } else if ok {
            let _ = self.new_ok();
        }
    }

    fn show_editor(&mut self, ctx: &egui::Context) {
        let types: Vec<String> = self.draft.types.iter().map(|t| t.name.clone()).collect();
        let taken: Vec<String> = self.draft.reporting.defaults.iter().map(|d| d.name.clone()).collect();
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        let title = format!("Structural Member Reporting - {}", ed.target);
        let mut ok = false;
        let mut cancel = false;
        let mut open = true;
        let name_err = {
            let n = ed.d.name.trim();
            (n.is_empty() || (n != ed.target && taken.iter().any(|t| t == n)))
                .then_some("The name is empty or already used")
        };
        let me = &mut *ed;
        egui::Window::new(title)
            .id(egui::Id::new("reporting_editor"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([640.0, 560.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                let h = (ui.available_height() - 44.0).max(80.0);
                ui.allocate_ui(egui::vec2(ui.available_width(), h), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("reporting_editor_scroll")
                        .show(ui, |ui| editor_body(ui, me));
                });
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(name_err.is_none(), egui::Button::new("   OK   "))
                        .clicked()
                    {
                        ok = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    if let Some(e) = name_err {
                        ui.colored_label(ERROR_RED, e);
                    }
                });
            });
        // The Board Specification dialog sits on top.
        let ed = self.editor.as_mut().expect("editor still open");
        if ed.board.is_some() {
            show_board(ctx, ed, &types);
        }
        if !open || cancel {
            self.editor = None;
        } else if ok {
            let _ = self.editor_ok();
        }
    }
}

fn editor_body(ui: &mut egui::Ui, ed: &mut DefaultEditor) {
    section(ui, "Format");
    row(ui, "Name", |ui| {
        ui.add(egui::TextEdit::singleline(&mut ed.d.name).desired_width(240.0));
    });
    row(ui, "Reporting Method", |ui| ui.label(ed.d.method.name()));
    row(ui, "Length Units", |ui| {
        ui.add_enabled_ui(ed.d.method.has_lengths(), |ui| {
            enum_combo(ui, "rep_units", &mut ed.d.units, &ReportUnits::ALL, ReportUnits::name)
        });
    });
    section(ui, "Board Sizes");
    let units = ed.d.units;
    let has_lengths = ed.d.method.has_lengths();
    let mut pick = None;
    let mut open_edit = false;
    egui::ScrollArea::vertical()
        .id_salt("rep_boards")
        .max_height(170.0)
        .show(ui, |ui| {
            egui::Grid::new("rep_boards_grid")
                .striped(true)
                .num_columns(5)
                .show(ui, |ui| {
                    for h in ["Size", "Length", "Type", "Treated", "Priority"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for (i, b) in ed.d.boards.iter().enumerate() {
                        let r = ui.selectable_label(ed.selected == Some(i), b.size_name());
                        if r.clicked() {
                            pick = Some(i);
                        }
                        if r.double_clicked() {
                            open_edit = true;
                        }
                        ui.label(if has_lengths { units.format(b.length) } else { String::new() });
                        ui.label(&b.type_name);
                        ui.label(if b.treated { "\u{2713}" } else { "" });
                        ui.label((i + 1).to_string());
                        ui.end_row();
                    }
                });
        });
    if let Some(i) = pick {
        ed.selected = Some(i);
    }
    if open_edit {
        ed.edit_board();
    }
    ui.horizontal_wrapped(|ui| {
        let sel = ed.selected.is_some();
        if ui.add_enabled(sel, egui::Button::new("Edit...")).clicked() {
            ed.edit_board();
        }
        if ui.button("New...").clicked() {
            ed.new_board();
        }
        if ui.add_enabled(sel, egui::Button::new("Delete")).clicked() {
            ed.delete_board();
        }
        let i = ed.selected.unwrap_or(0);
        if ui
            .add_enabled(sel && i > 0, egui::Button::new("Increase Priority"))
            .clicked()
        {
            ed.increase_priority();
        }
        if ui
            .add_enabled(sel && i + 1 < ed.d.boards.len(), egui::Button::new("Decrease Priority"))
            .clicked()
        {
            ed.decrease_priority();
        }
    });
    ui.weak("If a piece could be cut from more than one board, the one higher in the table is used.");
    if matches!(ed.d.method, ReportMethod::BuyList | ReportMethod::CutList) {
        section(ui, "Buy List Cut Board Options");
        row(ui, "Kerf Width", |ui| inches(ui, &mut ed.d.kerf, 0.0, 1.0));
    }
    if ed.d.method == ReportMethod::BuyList {
        section(ui, "Buy List Options for Long Board Runs");
        for lr in LongRun::ALL {
            ui.radio_value(&mut ed.d.long_runs, lr, lr.name());
        }
        ui.weak("Long runs are wall plates and girts, deck planking and rim joists; Other holds boards longer than any listed.");
    }
    if ed.d.method == ReportMethod::Mixed {
        ui.weak("Mixed Reporting counts studs, joists, rafters, posts and beams as pieces and totals plates, blocking, rim joists, ridges and headers in linear feet. List Cut Header Lengths is on the Openings panel of the Automatic Framing Defaults.");
    }
    section(ui, "The framing in this plan");
    let report = ed.report();
    ui.label(format!(
        "{} lines, {:.0} pieces, {:.1} linear feet",
        report.lines.len(),
        report.pieces(),
        report.linear_feet()
    ));
    egui::CollapsingHeader::new("Lines").default_open(false).show(ui, |ui| {
        egui::Grid::new("rep_lines").striped(true).show(ui, |ui| {
            for l in &report.lines {
                ui.label(l.category.name());
                ui.label(&l.description);
                ui.label(format!("{:.1} {}", l.qty, l.unit));
                ui.end_row();
            }
        });
    });
}

fn show_board(ctx: &egui::Context, ed: &mut DefaultEditor, types: &[String]) {
    let has_lengths = ed.d.method.has_lengths();
    let Some(b) = ed.board.as_mut() else {
        return;
    };
    let mut ok = false;
    let mut cancel = false;
    let mut open = true;
    let units = b.units;
    egui::Window::new("Board Specification")
        .id(egui::Id::new("reporting_board"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            row(ui, "Actual Thickness", |ui| inches(ui, &mut b.board.thickness, 0.25, 48.0));
            row(ui, "Actual Depth", |ui| inches(ui, &mut b.board.depth, 0.25, 96.0));
            if has_lengths {
                row(ui, "Length", |ui| {
                    let mut v = units.from_inches(b.board.length);
                    let suffix = match units {
                        ReportUnits::Feet => " ft",
                        ReportUnits::Inches => " in",
                        ReportUnits::Meters => " m",
                        ReportUnits::Millimeters => " mm",
                    };
                    if ui
                        .add(egui::DragValue::new(&mut v).speed(0.25).range(0.0..=5000.0).suffix(suffix))
                        .changed()
                    {
                        b.board.length = units.to_inches(v);
                    }
                });
            }
            row(ui, "Type", |ui| name_combo(ui, "rep_board_type", &mut b.board.type_name, types));
            ui.checkbox(&mut b.board.treated, "Treated");
            row(ui, "Formula", |ui| {
                ui.add(egui::TextEdit::singleline(&mut b.board.formula).desired_width(220.0));
            });
            ui.add_space(6.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("   OK   ").clicked() {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if !open || cancel {
        ed.board = None;
    } else if ok {
        ed.board_ok();
    }
}

/// `base`, `base 2`, ... — the first name `taken` does not know.
fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| base.to_string())
}

// ----- window state -----

thread_local! {
    static REQUESTED: RefCell<bool> = const { RefCell::new(false) };
    static OPEN: RefCell<Option<SavedDialog>> = const { RefCell::new(None) };
}

/// Asks for the Saved Structural Member Reporting Defaults dialog.
pub fn request_saved() {
    REQUESTED.with(|r| *r.borrow_mut() = true);
}

/// Draws the dialogs if open; call once a frame.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if REQUESTED.with(|r| std::mem::take(&mut *r.borrow_mut())) {
        let catalog = catalog_of(cx);
        let context = ReportContext::of_project(&cx.project, &catalog);
        OPEN.with(|o| *o.borrow_mut() = Some(SavedDialog::new(catalog, context)));
    }
    let Some(mut d) = OPEN.with(|o| o.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => OPEN.with(|o| *o.borrow_mut() = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            if set_catalog(cx, &d.draft, "Structural Member Reporting") {
                cx.status = "Saved the Structural Member Reporting defaults".into();
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

    fn dlg() -> SavedDialog {
        SavedDialog::new(FramingCatalog::default(), ReportContext::default())
    }

    #[test]
    fn new_makes_a_default_and_only_one_mixed_can_exist() {
        let mut d = dlg();
        d.new_default();
        let n = d.new_default.as_mut().unwrap();
        n.name = "Legacy".into();
        n.method = ReportMethod::Mixed;
        d.new_ok().unwrap();
        assert_eq!(d.selected.as_deref(), Some("Legacy"));
        d.new_default();
        let n = d.new_default.as_mut().unwrap();
        n.name = "Second".into();
        n.method = ReportMethod::Mixed;
        assert_eq!(d.new_ok(), Err(ReportingError::SecondMixed));
        assert!(d.new_default.is_some(), "the dialog stays open on the error");
        assert!(d.message.contains("Mixed"));
    }

    #[test]
    fn copy_convert_to_a_cut_list_keeps_the_boards_and_linear_drops_the_lengths() {
        let mut d = dlg();
        d.select("Buy List");
        assert!(d.copy_convert());
        let n = d.new_default.as_mut().unwrap();
        assert_eq!(n.name, "Buy List Copy");
        n.method = ReportMethod::LinearLength;
        d.new_ok().unwrap();
        let lin = d.draft.reporting.get("Buy List Copy").unwrap();
        assert_eq!(lin.method, ReportMethod::LinearLength);
        assert!(lin.boards.iter().all(|b| b.length == 0.0));
        assert!(lin.boards.len() < d.draft.reporting.get("Buy List").unwrap().boards.len());
    }

    #[test]
    fn rename_delete_and_active_follow_the_rules() {
        let mut d = dlg();
        d.new_default();
        d.new_default.as_mut().unwrap().name = "Cuts".into();
        d.new_default.as_mut().unwrap().method = ReportMethod::CutList;
        d.new_ok().unwrap();
        d.rename_selected("Job Cuts").unwrap();
        assert!(d.rename_selected("Buy List").is_err(), "names are unique");
        d.set_active("Job Cuts");
        assert_eq!(d.draft.reporting.active, "Job Cuts");
        d.delete_selected().unwrap();
        assert_eq!(d.draft.reporting.active, "Buy List");
        assert!(d.delete_selected().is_err(), "the last default stays");
    }

    #[test]
    fn the_editor_edits_boards_and_their_priority_and_ok_replaces_the_default() {
        let mut d = dlg();
        assert!(d.edit_selected());
        let ed = d.editor.as_mut().unwrap();
        let first = ed.d.boards[0].clone();
        ed.selected = Some(1);
        ed.increase_priority();
        assert_eq!(ed.selected, Some(0));
        assert_eq!(ed.d.boards[1], first);
        ed.decrease_priority();
        assert_eq!(ed.d.boards[0], first);
        ed.new_board();
        ed.board.as_mut().unwrap().board.length = 24.0 * 12.0;
        ed.board_ok();
        let n = ed.d.boards.len();
        assert_eq!(ed.d.boards[n - 1].length, 288.0);
        assert_eq!(ed.selected, Some(n - 1));
        ed.edit_board();
        ed.board.as_mut().unwrap().board.treated = true;
        ed.board_ok();
        assert!(ed.d.boards[n - 1].treated);
        ed.delete_board();
        assert_eq!(ed.d.boards.len(), n - 1);
        ed.d.kerf = 0.25;
        d.editor_ok().unwrap();
        assert_eq!(d.draft.reporting.get("Buy List").unwrap().kerf, 0.25);
        assert!(d.editor.is_none());
    }

    #[test]
    fn import_and_export_round_trip_a_default() {
        let mut d = dlg();
        let text = d.export_text().unwrap();
        let name = d.import_text(&text).unwrap();
        assert_eq!(name, "Buy List 2");
        assert!(d.import_text("nope").is_err());
        assert_eq!(d.draft.reporting.defaults.len(), 2);
    }

    #[test]
    fn the_editor_draws_and_the_dialogs_draw() {
        let ctx = egui::Context::default();
        let mut d = dlg();
        d.edit_selected();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = d.show(ctx);
        });
        d.editor_ok().unwrap();
        d.copy_convert();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = d.show(ctx);
        });
    }
}
