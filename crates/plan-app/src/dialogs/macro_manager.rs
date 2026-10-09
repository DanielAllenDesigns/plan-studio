//! The Insert Macro menu and Text Macro Management (TXT-20, TXT-59, TXT-63,
//! TXT-64, TXT-65; reference manual pp. 568, 1456, 1457).
//!
//! * [`insert_macro_button`] is the Insert Macro button found beside every
//!   text field: a menu of Global (Project Information, File, Room, Time Date,
//!   Special Characters), User Defined, Referenced Object and Callout macros.
//!   Every entry stays selectable whatever the context.
//! * [`MacroManager`] is the Text Macro Management window: the list of macros
//!   with Edit, New, Copy, Delete, Import and Export, and Show Evaluation
//!   Error for a macro that does not evaluate. Import brings macros in from a
//!   file another plan, layout or library exported; a name the plan already
//!   has asks what to do (rename, discard, replace), for all of them at once
//!   if wanted. Ruby macros are out of scope: a user macro is plain text with
//!   `%name%` substitutions.

use crate::dialogs::Outcome;
use eframe::egui::{self, Align2, RichText, Ui};
use plan_core::macros::{
    conflicts, export_macros, free_name, global_names, help_for, import_macros, is_macro_name,
    is_reserved, parse_macro_pack, Env, ImportReport, MenuItem, Resolution,
};
use plan_core::text_styles::{TextMacro, TextMacros};
use plan_core::Project;

// ===================================================================
// Insert Macro
// ===================================================================

fn menu_items(ui: &mut Ui, items: &[MenuItem], picked: &mut Option<String>) {
    for item in items {
        match &item.insert {
            Some(m) => {
                if ui.button(&item.label).on_hover_text(m).clicked() {
                    *picked = Some(m.clone());
                    ui.close_menu();
                }
            }
            None => {
                ui.menu_button(&item.label, |ui| {
                    if item.children.is_empty() {
                        ui.weak("None defined");
                    }
                    menu_items(ui, &item.children, picked);
                });
            }
        }
    }
}

/// The Insert Macro button: returns the `%macro%` the user picked.
pub fn insert_macro_button(ui: &mut Ui, items: &[MenuItem]) -> Option<String> {
    let mut picked = None;
    ui.menu_button("Insert Macro", |ui| menu_items(ui, items, &mut picked))
        .response
        .on_hover_text("Insert a macro that gives the project, the date, the room or an object");
    picked
}

/// `text` with `insert` typed at character `at` (replacing the selection
/// `sel` when it is not empty). Returns the new text and the cursor after the
/// insertion.
pub fn insert_at(text: &str, sel: (usize, usize), insert: &str) -> (String, usize) {
    let n = text.chars().count();
    let (a, b) = (sel.0.min(sel.1).min(n), sel.0.max(sel.1).min(n));
    let byte = |c: usize| text.char_indices().nth(c).map_or(text.len(), |(i, _)| i);
    let mut out = String::with_capacity(text.len() + insert.len());
    out.push_str(&text[..byte(a)]);
    out.push_str(insert);
    out.push_str(&text[byte(b)..]);
    (out, a + insert.chars().count())
}

/// The Insert Macro menu for a plan.
pub fn menu_for(project: &Project) -> Vec<MenuItem> {
    plan_core::macros::insert_menu(project)
}

thread_local! {
    static CURRENT: std::cell::RefCell<Vec<MenuItem>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Remembers the Insert Macro menu of the plan being edited, for the text
/// fields that draw their button without the plan at hand (the callout,
/// marker and note dialogs). The dialogs set it when they open.
pub fn set_current_menu(items: Vec<MenuItem>) {
    CURRENT.with(|c| *c.borrow_mut() = items);
}

/// The menu [`set_current_menu`] remembered (the plan-less menu before any).
pub fn current_menu() -> Vec<MenuItem> {
    let items = CURRENT.with(|c| c.borrow().clone());
    if items.is_empty() {
        menu_for(&Project::new(""))
    } else {
        items
    }
}

/// An Insert Macro button that appends the chosen `%macro%` to `text`.
/// Returns whether the text changed.
pub fn insert_macro_into(ui: &mut Ui, text: &mut String) -> bool {
    match insert_macro_button(ui, &current_menu()) {
        Some(m) => {
            text.push_str(&m);
            true
        }
        None => false,
    }
}

// ===================================================================
// Text Macro Management
// ===================================================================

/// An imported file waiting for the user's answers about name conflicts.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingImport {
    pub incoming: Vec<TextMacro>,
    /// The macros whose names the plan already has, with the answer for
    /// each.
    pub conflicts: Vec<(String, Choice)>,
    /// Do for all: the first answer is used for every conflict.
    pub for_all: bool,
}

/// The answer to a name conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Rename,
    Discard,
    Replace,
}

impl Choice {
    pub const ALL: [Choice; 3] = [Choice::Rename, Choice::Discard, Choice::Replace];

    pub fn label(&self) -> &'static str {
        match self {
            Choice::Rename => "Rename",
            Choice::Discard => "Discard",
            Choice::Replace => "Replace",
        }
    }
}

/// The macro being typed in the Edit / New area.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Editing {
    /// The macro's name before the edit; `None` for a new macro.
    pub original: Option<String>,
    pub name: String,
    pub text: String,
}

/// The Text Macro Management window.
pub struct MacroManager {
    draft: TextMacros,
    selected: Option<String>,
    editing: Option<Editing>,
    error: Option<String>,
    env: Env,
    pending: Option<PendingImport>,
    report: Option<String>,
}

impl MacroManager {
    pub fn new(macros: TextMacros) -> Self {
        Self {
            draft: macros,
            selected: None,
            editing: None,
            error: None,
            env: Env::bare(plan_core::macros::unix_now()),
            pending: None,
            report: None,
        }
    }

    /// The plan's global macros (the values Show Evaluation Error and the
    /// preview use).
    pub fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    pub fn draft(&self) -> &TextMacros {
        &self.draft
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn report(&self) -> Option<&str> {
        self.report.as_deref()
    }

    pub fn pending(&self) -> Option<&PendingImport> {
        self.pending.as_ref()
    }

    pub fn editing(&self) -> Option<&Editing> {
        self.editing.as_ref()
    }

    pub fn select(&mut self, name: &str) -> bool {
        let ok = self.draft.get(name).is_some();
        if ok {
            self.selected = Some(name.to_string());
            self.error = None;
        }
        ok
    }

    /// New: starts a blank macro.
    pub fn begin_new(&mut self) {
        self.editing = Some(Editing::default());
        self.error = None;
    }

    /// Edit: starts editing the selected macro (or `name`).
    pub fn begin_edit(&mut self, name: Option<&str>) -> bool {
        let name = name.map(str::to_string).or_else(|| self.selected.clone());
        let Some(m) = name.and_then(|n| self.draft.get(&n).cloned()) else {
            self.error = Some("Pick a macro to edit".into());
            return false;
        };
        self.editing = Some(Editing {
            original: Some(m.name.clone()),
            name: m.name,
            text: m.text,
        });
        self.error = None;
        true
    }

    /// Sets the text being edited, for tests and the Insert Macro button.
    pub fn set_editing(&mut self, name: &str, text: &str) {
        if let Some(e) = &mut self.editing {
            e.name = name.to_string();
            e.text = text.to_string();
        }
    }

    /// OK of the edit area: adds or changes the macro. The name must be new,
    /// made of letters, digits, `.`, `_` or `-`, and not a name the program
    /// uses.
    pub fn commit_edit(&mut self) -> bool {
        let Some(e) = self.editing.clone() else {
            return false;
        };
        let name = e.name.trim().to_string();
        if !is_macro_name(&name) {
            self.error = Some("Use a name of letters, digits, '.', '_' or '-'".into());
            return false;
        }
        let renamed = e.original.as_deref() != Some(name.as_str());
        if renamed && (is_reserved(&name) || self.draft.get(&name).is_some()) {
            self.error = Some(format!(
                "\"{name}\" is already a macro name; pick a name that is not built in"
            ));
            return false;
        }
        match e.original {
            Some(orig) => {
                if let Some(m) = self.draft.macros.iter_mut().find(|m| m.name == orig) {
                    m.name = name.clone();
                    m.text = e.text;
                }
                // Other macros that used the old name follow it.
                if renamed {
                    let (from, to) = (format!("%{orig}%"), format!("%{name}%"));
                    for m in &mut self.draft.macros {
                        m.text = m.text.replace(&from, &to);
                    }
                }
            }
            None => self.draft.macros.push(TextMacro {
                name: name.clone(),
                text: e.text,
            }),
        }
        self.selected = Some(name);
        self.editing = None;
        self.error = None;
        true
    }

    pub fn cancel_edit(&mut self) {
        self.editing = None;
    }

    /// Adds a macro directly (the old "new" fields).
    pub fn add_new(&mut self, name: &str, text: &str) -> bool {
        self.begin_new();
        self.set_editing(name, text);
        self.commit_edit()
    }

    /// Copy: a new macro called `<name>_2` with the same text.
    pub fn copy(&mut self, name: &str) -> Option<String> {
        let m = self.draft.get(name)?.clone();
        let new = free_name(&self.draft, name);
        self.draft.macros.push(TextMacro {
            name: new.clone(),
            text: m.text,
        });
        self.selected = Some(new.clone());
        Some(new)
    }

    /// Delete.
    pub fn delete(&mut self, name: &str) -> bool {
        let gone = self.draft.remove(name);
        if gone && self.selected.as_deref() == Some(name) {
            self.selected = None;
        }
        gone
    }

    /// Show Evaluation Error: why macro `name` does not evaluate, if it
    /// does not.
    pub fn evaluation_error(&self, name: &str) -> Option<String> {
        let m = self.draft.get(name)?;
        let env = self.env.clone().with_user(self.draft.clone());
        // Evaluate the macro through its own name so a loop shows up.
        env.evaluate(&format!("%{name}%"))
            .issues
            .first()
            .map(plan_core::macros::Issue::message)
            .or_else(|| {
                env.evaluate(&m.text)
                    .issues
                    .first()
                    .map(plan_core::macros::Issue::message)
            })
    }

    /// What the selected macro gives now.
    pub fn preview(&self, name: &str) -> Option<String> {
        self.draft.get(name)?;
        let env = self.env.clone().with_user(self.draft.clone());
        Some(env.expand(&format!("%{name}%")))
    }

    /// Export: the macros named `names` (all when empty) as a file text.
    pub fn export_text(&self, names: &[String]) -> String {
        export_macros(&self.draft, names)
    }

    /// Import: reads an exported file. Macros with new names come in at
    /// once; the answers for the others are collected in
    /// [`MacroManager::pending`].
    pub fn begin_import(&mut self, json: &str) -> Result<usize, String> {
        let incoming = parse_macro_pack(json)?;
        let clash = conflicts(&self.draft, &incoming);
        if clash.is_empty() {
            let report = import_macros(&mut self.draft, incoming, &mut |_| Resolution::Discard);
            self.report = Some(report_text(&report));
            self.pending = None;
            return Ok(0);
        }
        self.pending = Some(PendingImport {
            incoming,
            conflicts: clash.into_iter().map(|n| (n, Choice::Rename)).collect(),
            for_all: false,
        });
        Ok(self.pending.as_ref().map_or(0, |p| p.conflicts.len()))
    }

    /// Sets the answer for conflict `name`; with Do for all the answer goes
    /// to every conflict.
    pub fn choose(&mut self, name: &str, choice: Choice) {
        if let Some(p) = &mut self.pending {
            if p.for_all {
                for c in &mut p.conflicts {
                    c.1 = choice.clone();
                }
            } else if let Some(c) = p.conflicts.iter_mut().find(|c| c.0 == name) {
                c.1 = choice;
            }
        }
    }

    pub fn set_for_all(&mut self, on: bool) {
        if let Some(p) = &mut self.pending {
            p.for_all = on;
            if on {
                if let Some(first) = p.conflicts.first().map(|c| c.1.clone()) {
                    for c in &mut p.conflicts {
                        c.1 = first.clone();
                    }
                }
            }
        }
    }

    /// Finishes the import with the answers given.
    pub fn finish_import(&mut self) -> Option<ImportReport> {
        let p = self.pending.take()?;
        let report = import_macros(&mut self.draft, p.incoming, &mut |m| match p
            .conflicts
            .iter()
            .find(|c| c.0 == m.name)
            .map(|c| &c.1)
        {
            Some(Choice::Replace) => Resolution::Replace,
            Some(Choice::Discard) => Resolution::Discard,
            _ => Resolution::Rename(String::new()),
        });
        self.report = Some(report_text(&report));
        Some(report)
    }

    pub fn cancel_import(&mut self) {
        self.pending = None;
    }

    /// Draws the window.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let (mut open, mut ok, mut cancel) = (true, false, false);
        egui::Window::new("Text Macro Management")
            .id(egui::Id::new("text_macro_management"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([620.0, 520.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                self.body(ui, &mut ok, &mut cancel);
            });
        let esc = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if ok {
            Outcome::Ok
        } else if cancel || !open || esc {
            Outcome::Cancel
        } else {
            Outcome::Open
        }
    }

    fn body(&mut self, ui: &mut Ui, ok: &mut bool, cancel: &mut bool) {
        ui.label(RichText::new("User defined macros").strong());
        ui.horizontal(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("user_macros_list")
                .max_height(150.0)
                .min_scrolled_width(300.0)
                .show(ui, |ui| {
                    if self.draft.macros.is_empty() {
                        ui.weak("None yet. Press New.");
                    }
                    let names: Vec<String> =
                        self.draft.macros.iter().map(|m| m.name.clone()).collect();
                    for n in names {
                        let on = self.selected.as_deref() == Some(n.as_str());
                        let text = self
                            .draft
                            .get(&n)
                            .map(|m| m.text.clone())
                            .unwrap_or_default();
                        let line: String = text.chars().take(36).collect();
                        if ui.selectable_label(on, format!("%{n}%   {line}")).clicked() {
                            self.select(&n);
                        }
                    }
                });
            ui.vertical(|ui| {
                let sel = self.selected.clone();
                if ui.button("Edit").clicked() {
                    self.begin_edit(None);
                }
                if ui.button("New").clicked() {
                    self.begin_new();
                }
                if ui
                    .add_enabled(sel.is_some(), egui::Button::new("Copy"))
                    .clicked()
                {
                    if let Some(n) = &sel {
                        self.copy(n);
                    }
                }
                if ui
                    .add_enabled(sel.is_some(), egui::Button::new("Delete"))
                    .clicked()
                {
                    if let Some(n) = &sel {
                        self.delete(n);
                    }
                }
                if ui.button("Import").clicked() {
                    self.import_from_file();
                }
                if ui.button("Export").clicked() {
                    self.export_to_file();
                }
            });
        });
        if let Some(n) = self.selected.clone() {
            if let Some(v) = self.preview(&n) {
                let shown: String = v.chars().take(80).collect();
                ui.weak(format!("%{n}% gives: {shown}"));
            }
            if let Some(e) = self.evaluation_error(&n) {
                ui.colored_label(egui::Color32::from_rgb(0xE0, 0x4B, 0x4B), e);
            }
        }
        self.edit_area(ui);
        self.import_area(ui);
        if let Some(r) = &self.report {
            ui.weak(r.as_str());
        }
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::from_rgb(0xE0, 0x4B, 0x4B), e);
        }
        ui.separator();
        egui::CollapsingHeader::new("Built-in macros")
            .default_open(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("builtin_macros_list")
                    .max_height(160.0)
                    .show(ui, |ui| {
                        egui::Grid::new("builtin_macros")
                            .striped(true)
                            .show(ui, |ui| {
                                let project = Project::new("");
                                for name in global_names() {
                                    ui.monospace(format!("%{name}%"));
                                    ui.label(help_for(&project, &name).unwrap_or_default());
                                    ui.end_row();
                                }
                            });
                    });
            });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(RichText::new("   OK   ").strong()).clicked() {
                *ok = true;
            }
            if ui.button("Cancel").clicked() {
                *cancel = true;
            }
        });
    }

    fn edit_area(&mut self, ui: &mut Ui) {
        let Some(mut e) = self.editing.clone() else {
            return;
        };
        ui.separator();
        ui.label(
            RichText::new(if e.original.is_some() {
                "Edit Macro"
            } else {
                "New Macro"
            })
            .strong(),
        );
        let (mut done, mut stop) = (false, false);
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut e.name).desired_width(160.0));
        });
        ui.label("Text");
        ui.add(
            egui::TextEdit::multiline(&mut e.text)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            // A macro may use the others.
            let items = plan_core::macros::insert_menu(&Project::new(""));
            if let Some(m) = insert_macro_button(ui, &items) {
                e.text.push_str(&m);
            }
            if ui.button("Apply").clicked() {
                done = true;
            }
            if ui.button("Discard").clicked() {
                stop = true;
            }
        });
        self.editing = Some(e);
        if done {
            self.commit_edit();
        }
        if stop {
            self.cancel_edit();
        }
    }

    fn import_area(&mut self, ui: &mut Ui) {
        let Some(p) = self.pending.clone() else {
            return;
        };
        ui.separator();
        ui.label(RichText::new("Names that are already macros").strong());
        let mut for_all = p.for_all;
        let mut picks: Vec<(String, Choice)> = Vec::new();
        for (name, choice) in &p.conflicts {
            ui.horizontal(|ui| {
                ui.monospace(format!("%{name}%"));
                for c in Choice::ALL {
                    if ui.radio(*choice == c, c.label()).clicked() {
                        picks.push((name.clone(), c));
                    }
                }
            });
        }
        if ui.checkbox(&mut for_all, "Do for all").changed() {
            self.set_for_all(for_all);
        }
        for (n, c) in picks {
            self.choose(&n, c);
        }
        ui.horizontal(|ui| {
            if ui.button("Import").clicked() {
                self.finish_import();
            }
            if ui.button("Cancel Import").clicked() {
                self.cancel_import();
            }
        });
    }

    fn export_to_file(&mut self) {
        let names: Vec<String> = self.selected.iter().cloned().collect();
        let text = self.export_text(&names);
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("text_macros.json")
            .add_filter("Text macros", &["json"])
            .save_file()
        else {
            return;
        };
        self.report = Some(match std::fs::write(&path, text) {
            Ok(()) => format!("Exported to {}", path.display()),
            Err(e) => format!("Could not write {}: {e}", path.display()),
        });
    }

    fn import_from_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Text macros", &["json"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                if let Err(e) = self.begin_import(&text) {
                    self.error = Some(e);
                }
            }
            Err(e) => self.error = Some(format!("Could not read {}: {e}", path.display())),
        }
    }
}

fn report_text(r: &ImportReport) -> String {
    let mut parts = Vec::new();
    if !r.added.is_empty() {
        parts.push(format!("{} added", r.added.len()));
    }
    if !r.renamed.is_empty() {
        parts.push(format!("{} renamed", r.renamed.len()));
    }
    if !r.replaced.is_empty() {
        parts.push(format!("{} replaced", r.replaced.len()));
    }
    if !r.discarded.is_empty() {
        parts.push(format!("{} discarded", r.discarded.len()));
    }
    if !r.rejected.is_empty() {
        parts.push(format!("{} not allowed", r.rejected.len()));
    }
    if parts.is_empty() {
        "Nothing to import".into()
    } else {
        format!("Imported: {}", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> MacroManager {
        let mut m = TextMacros::default();
        m.add("firm", "Daniel Allen Designs");
        MacroManager::new(m)
    }

    #[test]
    fn new_edit_copy_and_delete_change_the_draft() {
        let mut d = manager();
        assert!(d.add_new("stamp", "Drawn by %firm%"));
        assert_eq!(d.draft().macros.len(), 2);
        // Built-in and duplicate names are refused with a reason.
        d.begin_new();
        d.set_editing("date.short", "x");
        assert!(!d.commit_edit());
        assert!(d.error().unwrap().contains("already a macro name"));
        d.set_editing("firm", "x");
        assert!(!d.commit_edit());
        d.set_editing("bad name", "x");
        assert!(!d.commit_edit());
        d.cancel_edit();
        // Edit renames and the macros that used the name follow.
        assert!(d.begin_edit(Some("firm")));
        d.set_editing("office", "Daniel Allen Designs, LLC");
        assert!(d.commit_edit());
        assert_eq!(d.draft().get("stamp").unwrap().text, "Drawn by %office%");
        assert!(d.draft().get("firm").is_none());
        assert_eq!(d.selected(), Some("office"));
        let c = d.copy("office").unwrap();
        assert_eq!(c, "office_2");
        assert_eq!(
            d.draft().get("office_2").unwrap().text,
            "Daniel Allen Designs, LLC"
        );
        assert!(d.delete("office_2") && !d.delete("office_2"));
        assert!(!d.begin_edit(Some("nope")));
    }

    #[test]
    fn evaluation_errors_name_the_problem() {
        let mut d = manager();
        d.add_new("a", "%b%");
        d.add_new("b", "%a%");
        d.add_new("bad", "%missing% here");
        d.add_new("good", "%firm% in %date.short%");
        assert!(d.evaluation_error("a").unwrap().contains("uses itself"));
        assert!(d.evaluation_error("bad").unwrap().contains("%missing%"));
        assert!(d.evaluation_error("good").is_none());
        let shown = d.preview("good").unwrap();
        assert!(shown.starts_with("Daniel Allen Designs in "), "{shown}");
        assert!(!shown.contains('%'));
    }

    #[test]
    fn export_and_import_ask_about_conflicts_once_or_for_all() {
        let mut a = manager();
        a.add_new("stamp", "Drawn by %firm%");
        a.add_new("scale", "1/4\" = 1'-0\"");
        let file = a.export_text(&[]);
        // A plan that has none of them takes them all.
        let mut fresh = MacroManager::new(TextMacros::default());
        assert_eq!(fresh.begin_import(&file), Ok(0));
        assert_eq!(fresh.draft().macros.len(), 3);
        assert!(fresh.report().unwrap().contains("3 added"));
        // A plan with two clashes answers each, or for all.
        let mut b = MacroManager::new(TextMacros::default());
        b.add_new("firm", "Other Firm");
        b.add_new("scale", "NTS");
        assert_eq!(b.begin_import(&file), Ok(2));
        assert_eq!(b.draft().macros.len(), 2);
        b.choose("firm", Choice::Replace);
        b.choose("scale", Choice::Rename);
        let r = b.finish_import().unwrap();
        assert_eq!(r.added, ["stamp"]);
        assert_eq!(r.replaced, ["firm"]);
        assert_eq!(r.renamed, [("scale".to_string(), "scale_2".to_string())]);
        assert_eq!(b.draft().get("firm").unwrap().text, "Daniel Allen Designs");
        assert_eq!(b.draft().get("scale").unwrap().text, "NTS");
        assert_eq!(b.draft().get("scale_2").unwrap().text, "1/4\" = 1'-0\"");
        // Do for all.
        let mut c = MacroManager::new(TextMacros::default());
        c.add_new("firm", "Other Firm");
        c.add_new("scale", "NTS");
        c.begin_import(&file).unwrap();
        c.set_for_all(true);
        c.choose("firm", Choice::Discard);
        assert!(c
            .pending()
            .unwrap()
            .conflicts
            .iter()
            .all(|x| x.1 == Choice::Discard));
        let r = c.finish_import().unwrap();
        assert_eq!(r.discarded.len(), 2);
        assert_eq!(c.draft().get("firm").unwrap().text, "Other Firm");
        // A bad file says so.
        assert!(c.begin_import("not json").is_err());
        assert!(c.begin_import("{\"format\":\"x\",\"macros\":[]}").is_err());
    }

    #[test]
    fn insert_at_puts_the_macro_at_the_cursor_or_over_the_selection() {
        assert_eq!(
            insert_at("Plan: ", (6, 6), "%plan.name%"),
            ("Plan: %plan.name%".into(), 17)
        );
        assert_eq!(insert_at("a X b", (2, 3), "%y%"), ("a %y% b".into(), 5));
        assert_eq!(insert_at("\u{b0}C", (1, 1), "+"), ("\u{b0}+C".into(), 2));
        assert_eq!(insert_at("ab", (9, 9), "!"), ("ab!".into(), 3));
    }

    #[test]
    fn the_window_and_the_menu_draw() {
        let mut d = manager();
        d.add_new("stamp", "x");
        d.begin_edit(Some("stamp"));
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = d.show(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                let items = menu_for(&Project::new("x"));
                assert!(insert_macro_button(ui, &items).is_none());
            });
        });
    }
}
