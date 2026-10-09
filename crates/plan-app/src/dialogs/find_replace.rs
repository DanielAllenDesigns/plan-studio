//! Edit > Find/Replace Text and Edit > Replace Fonts (TXT-12, TXT-26, TXT-45,
//! TXT-46; reference manual pp. 523, 543, 544).
//!
//! The window finds a string in the text of the plan, and of the layout files
//! kept in the plan, and replaces it:
//!
//! * **Searching In**: Current View (the active floor), Current File (every
//!   floor), All Open Files (every floor and every layout file of the plan)
//!   or Selected Objects.
//! * **Search Options**: Case Sensitive, Expand Percent Signs, Show in File
//!   (go to the floor or layout page of the result) and the Highlight Color.
//! * **Macro Options**: Exclude Macros, Macros Only or Include All; macro
//!   *names* are searched, never their values.
//! * **Results**: where each hit is (file, view, object type), Find
//!   Previous / Find Next, Replace for the current result, Replace All. Replace
//!   All is one undo step across the plan and the layouts.
//!
//! Replace Fonts lists the fonts the plan's text uses that this computer does
//! not have, offers a replacement family and face for each with a preview,
//! and applies them in one undo step.

// Hooks for the shell (`prompt_if_missing`, `take_goto`) are not called yet.
#![allow(dead_code)]

use crate::editor::selection::ObjectRef;
use crate::editor::EditorContext;
use crate::toolbar::Action;
use eframe::egui;
use plan_core::cad::CadItem;
use plan_core::find_text::{
    occurrences, replace_ranges, Area, MacroMode, Scope, TextHit, TextSearch,
};
use plan_layout::Layout;
use std::cell::RefCell;

/// Menu command: Edit > Replace Fonts.
pub const REPLACE_FONTS_PROMPT: &str = "text.replace_fonts";
/// Posted by the Replace Fonts window when Replace is pressed.
pub const REPLACE_FONTS_APPLY: &str = "text.replace_fonts_apply";

/// Results listed in the window at most.
const LIST_CAP: usize = 14;

// ===================================================================
// Layout text
// ===================================================================

/// Which text of a layout page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutField {
    /// A text in the page's CAD.
    Cad(plan_core::Id),
    /// A leader's text.
    Leader(plan_core::Id),
    /// The page title.
    Title,
}

/// One occurrence in a layout file of the plan.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutHit {
    /// 0 is the open layout, `n` the n-th parked layout file.
    pub file: usize,
    pub layout_name: String,
    /// The page number (`A-{number}`).
    pub page: u32,
    pub field: LayoutField,
    pub start: usize,
    pub len: usize,
    pub text: String,
}

/// A search result: in the plan or in a layout.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    Plan(TextHit),
    Layout(LayoutHit),
}

/// Where Show in File asks the shell to go (read with
/// [`FindReplaceDialog::take_goto`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Goto {
    /// A floor of the plan (already made current by the dialog).
    Floor(usize),
    /// A page of a layout file: `(file, page number)`.
    LayoutPage(usize, u32),
}

fn layout_values(project: &plan_core::Project) -> Vec<serde_json::Value> {
    project
        .layout
        .iter()
        .chain(project.layout_files.iter())
        .cloned()
        .collect()
}

fn layout_texts(layout: &Layout) -> Vec<(u32, LayoutField, String)> {
    let mut out = Vec::new();
    for p in &layout.pages {
        if !p.title.is_empty() {
            out.push((p.number, LayoutField::Title, p.title.clone()));
        }
        for o in &p.cad {
            if let CadItem::Text { text, .. } = &o.item {
                out.push((p.number, LayoutField::Cad(o.id), text.clone()));
            }
        }
        for l in &p.leaders {
            out.push((p.number, LayoutField::Leader(l.id), l.text.clone()));
        }
    }
    out
}

/// The occurrences of `q.find` in the layout files of the plan.
pub fn find_in_layouts(project: &plan_core::Project, q: &TextSearch) -> Vec<LayoutHit> {
    let mut out = Vec::new();
    if q.find.is_empty() {
        return out;
    }
    for (file, v) in layout_values(project).into_iter().enumerate() {
        let Ok(layout) = serde_json::from_value::<Layout>(v) else {
            continue;
        };
        for (page, field, text) in layout_texts(&layout) {
            for (start, len) in occurrences(&text, q) {
                out.push(LayoutHit {
                    file,
                    layout_name: layout.name.clone(),
                    page,
                    field,
                    start,
                    len,
                    text: text.clone(),
                });
            }
        }
    }
    out
}

/// Replaces `ranges` of one layout text; returns whether it was found.
fn edit_layout_text(
    layout: &mut Layout,
    page: u32,
    field: LayoutField,
    edit: &dyn Fn(&str) -> String,
) -> bool {
    let Some(p) = layout.pages.iter_mut().find(|p| p.number == page) else {
        return false;
    };
    match field {
        LayoutField::Title => {
            p.title = edit(&p.title);
            true
        }
        LayoutField::Cad(id) => {
            for o in &mut p.cad {
                if o.id == id {
                    if let CadItem::Text { text, .. } = &mut o.item {
                        *text = edit(text);
                        return true;
                    }
                }
            }
            false
        }
        LayoutField::Leader(id) => {
            for l in &mut p.leaders {
                if l.id == id {
                    l.text = edit(&l.text);
                    return true;
                }
            }
            false
        }
    }
}

/// Runs `edit_all` on layout file `file` of the plan and stores it back when
/// it reports a change.
fn with_layout(
    project: &mut plan_core::Project,
    file: usize,
    edit_all: &mut dyn FnMut(&mut Layout) -> bool,
) -> bool {
    let slot = if file == 0 {
        project.layout.as_mut()
    } else {
        project.layout_files.get_mut(file - 1)
    };
    let Some(slot) = slot else { return false };
    let Ok(mut layout) = serde_json::from_value::<Layout>(slot.clone()) else {
        return false;
    };
    if !edit_all(&mut layout) {
        return false;
    }
    match serde_json::to_value(&layout) {
        Ok(v) => {
            *slot = v;
            true
        }
        Err(_) => false,
    }
}

/// Replaces every occurrence in the layout files; returns `(texts changed,
/// occurrences replaced)`.
fn replace_in_layouts(project: &mut plan_core::Project, q: &TextSearch) -> (usize, usize) {
    let (mut objects, mut occ) = (0, 0);
    let files = layout_values(project).len();
    for file in 0..files {
        with_layout(project, file, &mut |layout| {
            let mut changed = false;
            let texts = layout_texts(layout);
            for (page, field, text) in texts {
                let found = occurrences(&text, q);
                if found.is_empty() {
                    continue;
                }
                let with = q.replace.clone();
                let ok = edit_layout_text(layout, page, field, &|s| {
                    replace_ranges(s, &occurrences(s, q), &with)
                });
                if ok {
                    changed = true;
                    objects += 1;
                    occ += found.len();
                }
            }
            changed
        });
    }
    (objects, occ)
}

// ===================================================================
// The window
// ===================================================================

pub struct FindReplaceDialog {
    pub search: TextSearch,
    pub scope: Scope,
    /// Show in File: go to the floor or page of the current result.
    pub show_in_file: bool,
    /// Highlight Color of the current result.
    pub highlight: [u8; 3],
    /// What the last Find or Replace All reported.
    pub result: String,
    pub results: Vec<Found>,
    /// The current result, an index into `results`.
    pub current: usize,
    goto: Option<Goto>,
}

impl Default for FindReplaceDialog {
    fn default() -> Self {
        Self {
            search: TextSearch::default(),
            scope: Scope::CurrentView,
            show_in_file: true,
            highlight: [255, 214, 10],
            result: String::new(),
            results: Vec::new(),
            current: 0,
            goto: None,
        }
    }
}

/// The CAD objects selected on the active floor (what Selected Objects
/// searches).
fn selected_cad(cx: &EditorContext) -> Vec<plan_core::Id> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) | ObjectRef::Text(id) => Some(*id),
            _ => None,
        })
        .collect()
}

/// Replace All: swaps the text in every matching text of `scope` in one undo
/// step (the plan's text, and with All Open Files the layouts' too). Returns
/// `(objects changed, occurrences replaced)`; a search that matches nothing
/// records no step.
pub fn replace_all(cx: &mut EditorContext, search: &TextSearch, scope: Scope) -> (usize, usize) {
    if search.find.is_empty() {
        return (0, 0);
    }
    let selected = selected_cad(cx);
    let area = Area::new(scope, cx.floor, &selected);
    let in_plan = !cx.project.find_hits(search, &area).is_empty();
    let in_layouts =
        scope == Scope::AllOpenFiles && !find_in_layouts(&cx.project, search).is_empty();
    if !in_plan && !in_layouts {
        return (0, 0);
    }
    cx.begin_change("Replace Text");
    let (mut objects, mut occurrences) = cx.project.replace_all_hits(search, &area);
    if scope == Scope::AllOpenFiles {
        let (o, n) = replace_in_layouts(&mut cx.project, search);
        objects += o;
        occurrences += n;
    }
    cx.mark_dirty();
    cx.status = format!(
        "Replaced {occurrences} occurrence{} in {objects} text object{}",
        if occurrences == 1 { "" } else { "s" },
        if objects == 1 { "" } else { "s" },
    );
    (objects, occurrences)
}

impl FindReplaceDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Where the current result is, for the line above the Results list:
    /// file, view and object type.
    pub fn location(&self, cx: &EditorContext) -> String {
        match self.results.get(self.current) {
            Some(Found::Plan(h)) => {
                format!("{}: {}", cx.project.name, cx.project.site_location(&h.site))
            }
            Some(Found::Layout(h)) => format!(
                "{}: A-{}, {}",
                h.layout_name,
                h.page,
                match h.field {
                    LayoutField::Title => "Page title",
                    LayoutField::Cad(_) => "Text",
                    LayoutField::Leader(_) => "Leader text",
                }
            ),
            None => String::new(),
        }
    }

    /// Find: lists what matches and says how many.
    pub fn find(&mut self, cx: &mut EditorContext) {
        self.results.clear();
        self.current = 0;
        if self.search.find.is_empty() {
            self.result = "Enter the text to find".into();
            return;
        }
        let selected = selected_cad(cx);
        let area = Area::new(self.scope, cx.floor, &selected);
        self.results.extend(
            cx.project
                .find_hits(&self.search, &area)
                .into_iter()
                .map(Found::Plan),
        );
        if self.scope == Scope::AllOpenFiles {
            self.results.extend(
                find_in_layouts(&cx.project, &self.search)
                    .into_iter()
                    .map(Found::Layout),
            );
        }
        let n = self.results.len();
        self.result = if n == 0 {
            "No matches".into()
        } else {
            format!("{n} match{}", if n == 1 { "" } else { "es" })
        };
        self.reveal(cx);
    }

    /// Find Next.
    pub fn find_next(&mut self, cx: &mut EditorContext) {
        if self.results.is_empty() {
            self.find(cx);
            return;
        }
        self.current = (self.current + 1) % self.results.len();
        self.reveal(cx);
    }

    /// Find Previous.
    pub fn find_previous(&mut self, cx: &mut EditorContext) {
        if self.results.is_empty() {
            self.find(cx);
            return;
        }
        self.current = (self.current + self.results.len() - 1) % self.results.len();
        self.reveal(cx);
    }

    /// Selects the current result; with Show in File also goes to its view.
    fn reveal(&mut self, cx: &mut EditorContext) {
        let Some(found) = self.results.get(self.current).cloned() else {
            return;
        };
        match found {
            Found::Plan(h) => {
                if h.site.floor != cx.floor {
                    if !self.show_in_file {
                        return;
                    }
                    cx.floor = h.site.floor.min(cx.project.floors.len() - 1);
                    self.goto = Some(Goto::Floor(cx.floor));
                }
                if h.site.id != 0 {
                    cx.selection.set(ObjectRef::Cad(h.site.id));
                }
            }
            Found::Layout(h) => {
                if self.show_in_file {
                    self.goto = Some(Goto::LayoutPage(h.file, h.page));
                }
            }
        }
    }

    /// The view Show in File wants the shell to go to, once.
    pub fn take_goto(&mut self) -> Option<Goto> {
        self.goto.take()
    }

    /// Replace: the current result only, then the next one.
    pub fn replace_current(&mut self, cx: &mut EditorContext) {
        let Some(found) = self.results.get(self.current).cloned() else {
            self.result = "Find something first".into();
            return;
        };
        let with = self.search.replace.clone();
        cx.begin_change("Replace Text");
        let done = match &found {
            Found::Plan(h) => cx.project.replace_hit(h, &with),
            Found::Layout(h) => {
                let (start, len, page, field) = (h.start, h.len, h.page, h.field);
                with_layout(&mut cx.project, h.file, &mut |layout| {
                    edit_layout_text(layout, page, field, &|s| {
                        replace_ranges(s, &[(start, len)], &with)
                    })
                })
            }
        };
        if !done {
            self.result = "That text is no longer there".into();
            return;
        }
        cx.mark_dirty();
        let at = self.current;
        self.find(cx);
        // The search starts over; stay near where we were.
        if !self.results.is_empty() {
            self.current = at.min(self.results.len() - 1);
            self.reveal(cx);
        }
        self.result = format!("Replaced one; {}", self.result.to_lowercase());
    }

    /// Replace All with the dialog's settings.
    pub fn replace_all(&mut self, cx: &mut EditorContext) {
        if self.search.find.is_empty() {
            self.result = "Enter the text to find".into();
            return;
        }
        let (objects, occurrences) = replace_all(cx, &self.search, self.scope);
        self.results.clear();
        self.current = 0;
        self.result = if occurrences == 0 {
            "No matches".into()
        } else {
            format!("Replaced {occurrences} in {objects} text object(s)")
        };
    }

    /// Draws the window; false once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let (mut find, mut next, mut prev, mut replace, mut replace_all) =
            (false, false, false, false, false);
        egui::Window::new("Find/Replace Text")
            .id(egui::Id::new("find_replace_text"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("find_replace_grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Find");
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut self.search.find).desired_width(240.0),
                        );
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            find = true;
                        }
                        ui.end_row();
                        ui.label("Replace With");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search.replace)
                                .desired_width(240.0),
                        );
                        ui.end_row();
                        ui.label("Searching In");
                        egui::ComboBox::from_id_salt("find_scope")
                            .selected_text(self.scope.label())
                            .show_ui(ui, |ui| {
                                for s in Scope::ALL {
                                    ui.selectable_value(&mut self.scope, s, s.label());
                                }
                            });
                        ui.end_row();
                    });
                ui.separator();
                ui.label(egui::RichText::new("Search Options").strong());
                ui.checkbox(&mut self.search.match_case, "Case Sensitive");
                ui.checkbox(&mut self.search.whole_word, "Whole Words Only");
                ui.checkbox(&mut self.search.expand_percent, "Expand Percent Signs");
                ui.checkbox(&mut self.show_in_file, "Show in File");
                ui.horizontal(|ui| {
                    ui.label("Highlight Color");
                    ui.color_edit_button_srgb(&mut self.highlight);
                });
                ui.label(egui::RichText::new("Macro Options").strong());
                ui.horizontal(|ui| {
                    for m in MacroMode::ALL {
                        ui.radio_value(&mut self.search.macros, m, m.label());
                    }
                });
                ui.separator();
                ui.label(egui::RichText::new("Results").strong());
                let loc = self.location(cx);
                if !loc.is_empty() {
                    ui.weak(loc);
                }
                ui.columns(2, |cols| {
                    egui::ScrollArea::vertical()
                        .id_salt("find_results")
                        .max_height(150.0)
                        .show(&mut cols[0], |ui| {
                            for (i, f) in self.results.iter().enumerate().take(LIST_CAP) {
                                let text = match f {
                                    Found::Plan(h) => h.text.clone(),
                                    Found::Layout(h) => h.text.clone(),
                                };
                                let line: String = text.chars().take(34).collect();
                                if ui.selectable_label(i == self.current, line).clicked() {
                                    self.current = i;
                                    prev = false;
                                }
                            }
                            if self.results.len() > LIST_CAP {
                                ui.weak(format!("{} more", self.results.len() - LIST_CAP));
                            }
                        });
                    cols[1].vertical(|ui| {
                        find |= ui.button("Find").clicked();
                        prev |= ui.button("Find Previous").clicked();
                        next |= ui.button("Find Next").clicked();
                        replace |= ui.button("Replace").clicked();
                        replace_all |= ui.button("Replace All").clicked();
                    });
                });
                if !self.result.is_empty() {
                    ui.label(&self.result);
                }
            });
        if find {
            self.find(cx);
        }
        if prev {
            self.find_previous(cx);
        }
        if next {
            self.find_next(cx);
        }
        if replace {
            self.replace_current(cx);
        }
        if replace_all {
            self.replace_all(cx);
        }
        open
    }
}

// ===================================================================
// Replace Fonts
// ===================================================================

/// One font the plan's text uses.
#[derive(Debug, Clone, PartialEq)]
pub struct FontRow {
    pub font: String,
    /// This computer has no such family.
    pub missing: bool,
    /// The family that replaces it ("" leaves it).
    pub with: String,
    /// "", "Bold", "Italic" or "Bold Italic".
    pub face: String,
}

/// The faces the Face list offers.
pub const FACES: [&str; 4] = ["Regular", "Bold", "Italic", "Bold Italic"];

fn installed(font: &str) -> bool {
    let cat = crate::fonts::catalog();
    cat.has_family(font)
        || crate::fonts::family_candidates(font)
            .iter()
            .any(|c| cat.has_family(c))
}

/// The Replace Fonts table for a plan: the fonts its text uses, the missing
/// ones first. `all` lists the installed ones too.
pub fn font_rows(cx: &EditorContext, all: bool) -> Vec<FontRow> {
    let default_with = crate::fonts::catalog()
        .families()
        .into_iter()
        .find(|f| f == "Arial" || f == "Helvetica")
        .unwrap_or_else(|| "Arial".to_string());
    let mut rows: Vec<FontRow> = cx
        .project
        .fonts_in_use()
        .into_iter()
        .map(|f| FontRow {
            missing: !installed(&f),
            with: default_with.clone(),
            face: "Regular".into(),
            font: f,
        })
        .filter(|r| all || r.missing)
        .collect();
    rows.sort_by_key(|r| !r.missing);
    rows
}

/// Applies the rows (those with a replacement) as one undo step. Returns how
/// many styles and runs changed.
pub fn apply_font_rows(cx: &mut EditorContext, rows: &[FontRow]) -> usize {
    let todo: Vec<&FontRow> = rows
        .iter()
        .filter(|r| {
            !r.with.trim().is_empty() && !plan_core::text_styles::same_font_family(&r.font, &r.with)
        })
        .collect();
    if todo.is_empty() {
        return 0;
    }
    cx.begin_change("Replace Fonts");
    let mut n = 0;
    for r in todo {
        let face = if r.face == "Regular" {
            ""
        } else {
            r.face.as_str()
        };
        n += cx.project.replace_font_everywhere(&r.font, &r.with, face);
    }
    cx.mark_dirty();
    cx.status = format!(
        "Replaced fonts in {n} style{}/run{}",
        if n == 1 { "" } else { "s" },
        if n == 1 { "" } else { "s" }
    );
    n
}

#[derive(Default)]
struct FontPrompt {
    rows: Vec<FontRow>,
    all: bool,
    selected: usize,
    /// The window is up.
    open: bool,
}

thread_local! {
    static PROMPT: RefCell<FontPrompt> = RefCell::new(FontPrompt::default());
}

/// Opens the Replace Fonts window for the plan (Edit > Replace Fonts, and on
/// opening a file that has missing fonts: see [`prompt_if_missing`]).
pub fn open_replace_fonts(cx: &mut EditorContext) {
    let rows = font_rows(cx, false);
    if rows.is_empty() {
        let any = !cx.project.fonts_in_use().is_empty();
        cx.status = if any {
            "Every font the plan uses is installed. Use \"Show all fonts\" to replace one anyway."
                .into()
        } else {
            "The plan's text uses no fonts to replace".into()
        };
        // Still open it on the full list when asked from the menu.
        PROMPT.with(|p| {
            *p.borrow_mut() = FontPrompt {
                rows: font_rows(cx, true),
                all: true,
                selected: 0,
                open: any,
            };
        });
        return;
    }
    PROMPT.with(|p| {
        *p.borrow_mut() = FontPrompt {
            rows,
            all: false,
            selected: 0,
            open: true,
        };
    });
}

/// Opens Replace Fonts only when a font of the plan is missing (a file was
/// just opened). Returns whether it opened.
pub fn prompt_if_missing(cx: &mut EditorContext) -> bool {
    let rows = font_rows(cx, false);
    if rows.is_empty() {
        return false;
    }
    PROMPT.with(|p| {
        *p.borrow_mut() = FontPrompt {
            rows,
            all: false,
            selected: 0,
            open: true,
        };
    });
    true
}

/// Is the Replace Fonts window up?
pub fn replace_fonts_open() -> bool {
    PROMPT.with(|p| p.borrow().open)
}

/// The rows the window shows, for tests.
pub fn replace_fonts_rows() -> Vec<FontRow> {
    PROMPT.with(|p| p.borrow().rows.clone())
}

/// Sets the replacement of the row for `font`, for tests and shortcuts.
pub fn set_replacement(font: &str, with: &str, face: &str) {
    PROMPT.with(|p| {
        for r in &mut p.borrow_mut().rows {
            if r.font == font {
                r.with = with.to_string();
                r.face = face.to_string();
            }
        }
    });
}

/// Closes the window without applying it.
pub fn close_replace_fonts() {
    PROMPT.with(|p| p.borrow_mut().open = false);
}

/// Applies the window's rows and closes it (the Replace button's command).
pub fn apply_replace_fonts(cx: &mut EditorContext) -> usize {
    let rows = PROMPT.with(|p| p.borrow().rows.clone());
    let n = apply_font_rows(cx, &rows);
    PROMPT.with(|p| p.borrow_mut().open = false);
    n
}

/// Draws the Replace Fonts window when it is up; pressing Replace pushes
/// [`REPLACE_FONTS_APPLY`] into `out`. The menu bar calls this each frame.
pub fn show_prompts(ctx: &egui::Context, out: &mut Vec<Action>) {
    if !replace_fonts_open() {
        return;
    }
    let mut state = PROMPT.with(|p| std::mem::take(&mut *p.borrow_mut()));
    let mut open = state.open;
    let mut apply = false;
    let families = crate::fonts::catalog().families();
    egui::Window::new("Replace Fonts")
        .id(egui::Id::new("replace_fonts_prompt"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("These fonts are used by the text of this plan.");
            ui.checkbox(&mut state.all, "Show fonts that are installed too");
            if state.all && !state.rows.iter().any(|r| !r.missing) {
                // The list was built without the installed ones.
            }
            egui::Grid::new("replace_fonts_grid")
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("Font");
                    ui.strong("Status");
                    ui.strong("Replace With");
                    ui.strong("Face");
                    ui.end_row();
                    for (i, r) in state.rows.iter_mut().enumerate() {
                        if !state.all && !r.missing {
                            continue;
                        }
                        if ui.selectable_label(i == state.selected, &r.font).clicked() {
                            state.selected = i;
                        }
                        ui.label(if r.missing { "Missing" } else { "Installed" });
                        egui::ComboBox::from_id_salt(("replace_with", i))
                            .selected_text(r.with.clone())
                            .show_ui(ui, |ui| {
                                for f in &families {
                                    ui.selectable_value(&mut r.with, f.clone(), f.as_str());
                                }
                            });
                        egui::ComboBox::from_id_salt(("replace_face", i))
                            .selected_text(r.face.clone())
                            .show_ui(ui, |ui| {
                                for f in FACES {
                                    ui.selectable_value(&mut r.face, f.to_string(), f);
                                }
                            });
                        ui.end_row();
                    }
                });
            ui.separator();
            ui.label(egui::RichText::new("Preview").strong());
            if let Some(r) = state.rows.get(state.selected) {
                let spec = crate::fonts::spec_named(
                    &r.with,
                    r.face.contains("Bold"),
                    r.face.contains("Italic"),
                );
                crate::fonts::face_preview(ui, &spec);
            }
            ui.horizontal(|ui| {
                if ui.button("Replace").clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    state.open = false;
                }
            });
        });
    state.open = state.open && open;
    if apply {
        out.push(Action::Custom(REPLACE_FONTS_APPLY));
    }
    PROMPT.with(|p| *p.borrow_mut() = state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::cad::CadItem;
    use plan_core::geometry::Point;

    fn cx_with(texts: &[&str]) -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        for t in texts {
            cx.project.add_cad(
                0,
                "CAD, Default",
                CadItem::Text {
                    pos: Point::ZERO,
                    text: (*t).into(),
                    height: 3.0,
                    angle: 0.0,
                },
            );
        }
        cx
    }

    fn texts(cx: &EditorContext) -> Vec<String> {
        cx.floor()
            .cad
            .iter()
            .filter_map(|c| match &c.item {
                CadItem::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn find_counts_and_replace_all_is_one_undo_step() {
        let mut cx = cx_with(&["Bath 1", "Hall", "Bath 2 and Bath 3"]);
        let mut d = FindReplaceDialog::new();
        d.search.find = "bath".into();
        d.search.replace = "Bathroom".into();
        d.find(&mut cx);
        assert_eq!(d.result, "3 matches");
        assert_eq!(d.results.len(), 3);
        d.replace_all(&mut cx);
        assert_eq!(
            texts(&cx),
            vec!["Bathroom 1", "Hall", "Bathroom 2 and Bathroom 3"]
        );
        assert_eq!(cx.undo_label(), Some("Replace Text"));
        cx.undo();
        assert_eq!(texts(&cx), vec!["Bath 1", "Hall", "Bath 2 and Bath 3"]);
    }

    #[test]
    fn nothing_found_records_no_step() {
        let mut cx = cx_with(&["Hall"]);
        let mut d = FindReplaceDialog::new();
        d.search.find = "Porch".into();
        d.replace_all(&mut cx);
        assert_eq!(d.result, "No matches");
        assert!(!cx.can_undo());
        d.search.find.clear();
        d.find(&mut cx);
        assert_eq!(d.result, "Enter the text to find");
    }

    #[test]
    fn find_next_previous_and_replace_walk_the_results() {
        let mut cx = cx_with(&["Bath 1", "Hall", "Bath 2"]);
        let mut d = FindReplaceDialog::new();
        d.search.find = "bath".into();
        d.search.replace = "Suite".into();
        d.find(&mut cx);
        assert_eq!(d.current, 0);
        // The current result is selected.
        assert_eq!(cx.selection.items.len(), 1);
        d.find_next(&mut cx);
        assert_eq!(d.current, 1);
        d.find_next(&mut cx);
        assert_eq!(d.current, 0);
        d.find_previous(&mut cx);
        assert_eq!(d.current, 1);
        assert!(d.location(&cx).contains("1st Floor, Text"));
        // Replace changes only the current one.
        d.replace_current(&mut cx);
        assert_eq!(texts(&cx), vec!["Bath 1", "Hall", "Suite 2"]);
        assert_eq!(cx.undo_label(), Some("Replace Text"));
        assert_eq!(d.results.len(), 1);
        // Nothing found when asking for something else.
        d.search.find = "zzz".into();
        d.find(&mut cx);
        assert!(d.results.is_empty());
        d.find_next(&mut cx);
        assert!(d.results.is_empty());
    }

    #[test]
    fn the_scopes_and_selection_limit_the_search() {
        let mut cx = cx_with(&["Bath 1", "Bath 2"]);
        cx.project
            .floors
            .push(plan_core::model::Floor::new("2nd Floor", 109.0));
        let c = cx.project.add_cad(
            1,
            "CAD, Default",
            CadItem::Text {
                pos: Point::ZERO,
                text: "Bath 3".into(),
                height: 3.0,
                angle: 0.0,
            },
        );
        let mut d = FindReplaceDialog::new();
        d.search.find = "bath".into();
        d.scope = Scope::CurrentView;
        d.find(&mut cx);
        assert_eq!(d.results.len(), 2);
        d.scope = Scope::CurrentFile;
        d.find(&mut cx);
        assert_eq!(d.results.len(), 3);
        // Show in File moves to the floor of the result.
        d.current = 1;
        d.find_next(&mut cx);
        assert_eq!(cx.floor, 1);
        assert_eq!(d.take_goto(), Some(Goto::Floor(1)));
        assert!(cx.selection.contains(ObjectRef::Cad(c)));
        // Without Show in File the view stays.
        cx.floor = 0;
        d.show_in_file = false;
        d.find_next(&mut cx);
        assert_eq!(cx.floor, 0);
        // Selected Objects.
        d.scope = Scope::SelectedObjects;
        cx.selection.clear();
        d.find(&mut cx);
        assert!(d.results.is_empty());
        let first = cx.floor().cad[0].id;
        cx.selection.set(ObjectRef::Cad(first));
        d.find(&mut cx);
        assert_eq!(d.results.len(), 1);
        d.search.replace = "Den".into();
        d.replace_all(&mut cx);
        assert_eq!(texts(&cx), vec!["Den 1", "Bath 2"]);
    }

    #[test]
    fn the_window_draws() {
        let mut cx = cx_with(&["Hall"]);
        let mut d = FindReplaceDialog::new();
        d.search.find = "Hall".into();
        d.find(&mut cx);
        let ctx = egui::Context::default();
        let mut open = true;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            open = d.show(ctx, &mut cx);
        });
        assert!(open);
    }

    #[test]
    fn replace_fonts_lists_missing_fonts_and_replaces_them_in_one_step() {
        let mut cx = cx_with(&["Hall"]);
        // A style in a font nobody has, and a rich text run in another.
        cx.project.text_styles.styles[0].font = "Zzyzx Display".into();
        let id = cx.floor().cad[0].id;
        cx.project.edit_cad_attrs(0, id, |a| {
            a.runs = vec![plan_core::text_styles::RichRun {
                text: "Hall".into(),
                font: Some("Zzyzx Display".into()),
                ..Default::default()
            }];
        });
        let rows = font_rows(&cx, false);
        assert!(rows.iter().any(|r| r.font == "Zzyzx Display" && r.missing));
        open_replace_fonts(&mut cx);
        assert!(replace_fonts_open());
        set_replacement("Zzyzx Display", "Arial", "Bold");
        let n = apply_replace_fonts(&mut cx);
        assert!(n >= 2, "{n}");
        assert!(!replace_fonts_open());
        assert_eq!(cx.undo_label(), Some("Replace Fonts"));
        assert!(cx.project.text_styles.styles[0].font == "Arial");
        assert!(cx.project.text_styles.styles[0].bold);
        let run = &cx.floor().cad_attrs(id).unwrap().runs[0];
        assert_eq!(run.font.as_deref(), Some("Arial"));
        assert!(run.bold);
        cx.undo();
        assert_eq!(cx.project.text_styles.styles[0].font, "Zzyzx Display");
        // Nothing missing now: no step is recorded for an empty mapping.
        let rows = vec![FontRow {
            font: "Arial".into(),
            missing: false,
            with: "Arial".into(),
            face: "Regular".into(),
        }];
        assert_eq!(apply_font_rows(&mut cx, &rows), 0);
        // The window draws.
        open_replace_fonts(&mut cx);
        let ctx = egui::Context::default();
        let mut out = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_prompts(ctx, &mut out));
        close_replace_fonts();
    }
}
