//! Tools > Spell Check and the Check Spelling button of the text dialog
//! (TXT-21).
//!
//! The plan check walks every piece of text of the plan in a fixed order: the
//! text objects of each floor (including their rich text runs), dimension text
//! overrides, door and window labels, symbol labels, room names, and the
//! layout's text boxes, captions, page notes, leaders and page titles. At each
//! misspelled word it stops and shows Chief's Spelling dialog: the word in its
//! sentence, a Change To field, the suggestions (edit distance two or less
//! from the system word list), and Ignore, Ignore All, Change, Change All and
//! Add (to `~/.plan-studio/dictionary.txt`). Every Change and Change All is
//! one undo step.
//!
//! The text dialog runs the same dialog over the text being typed
//! ([`local_controls`]) and underlines misspelled words in red
//! ([`layouter`]).

use crate::editor::{EditorContext, ObjectRef};
use crate::shell::layout_window;
use crate::spell::{self, Speller, MAX_SUGGESTIONS};
use eframe::egui::{self, text::LayoutJob, Align2, Color32, Stroke, TextFormat, Vec2};
use plan_core::cad::CadItem;
use plan_core::{Id, Project};
use plan_layout::BoxSource;
use std::cell::RefCell;
use std::sync::Arc;

/// Menu and toolbar id: Tools > Spell Check...
pub const OPEN: &str = "spell.check";

const RED: Color32 = Color32::from_rgb(0xE0, 0x30, 0x30);

// ----- the places text lives -----

/// One piece of text of the plan.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// A text object of floor `floor`.
    CadText { floor: usize, id: Id },
    /// The text override of a dimension.
    DimText { floor: usize, id: Id },
    /// The label override of a door or window.
    OpeningLabel { floor: usize, id: Id },
    /// The label of a placed symbol.
    SymbolLabel { floor: usize, id: Id },
    /// The name of a room (index into the floor's name entries).
    RoomName { floor: usize, index: usize },
    /// A text box of a layout page.
    LayoutBoxText { page: usize, id: Id },
    /// The caption of a layout box.
    LayoutBoxLabel { page: usize, id: Id },
    /// A text note of a layout page.
    LayoutNote { page: usize, id: Id },
    /// A leader of a layout page.
    LayoutLeader { page: usize, id: Id },
    /// The title of a layout page.
    LayoutPageTitle { page: usize },
}

/// Every piece of text of the plan that can hold a word, in checking order.
pub fn sources(project: &Project) -> Vec<Source> {
    let mut out = Vec::new();
    for (fi, f) in project.floors.iter().enumerate() {
        for c in &f.cad {
            if matches!(c.item, CadItem::Text { .. }) {
                out.push(Source::CadText {
                    floor: fi,
                    id: c.id,
                });
            }
        }
        for d in &f.dimensions {
            if d.text_override.as_deref().is_some_and(|t| !t.is_empty()) {
                out.push(Source::DimText {
                    floor: fi,
                    id: d.id,
                });
            }
        }
        for o in &f.openings {
            if o.label_override.as_deref().is_some_and(|t| !t.is_empty()) {
                out.push(Source::OpeningLabel {
                    floor: fi,
                    id: o.id,
                });
            }
        }
        for s in &f.symbols {
            if !s.label.is_empty() {
                out.push(Source::SymbolLabel {
                    floor: fi,
                    id: s.id,
                });
            }
        }
        for (i, n) in f.room_names.iter().enumerate() {
            if !n.name.is_empty() {
                out.push(Source::RoomName {
                    floor: fi,
                    index: i,
                });
            }
        }
    }
    if let Some(layout) = layout_window::load(project) {
        for (pi, page) in layout.pages.iter().enumerate() {
            out.push(Source::LayoutPageTitle { page: pi });
            for b in &page.boxes {
                if matches!(b.source, BoxSource::Text { .. }) {
                    out.push(Source::LayoutBoxText { page: pi, id: b.id });
                }
                if b.label.as_deref().is_some_and(|t| !t.is_empty()) {
                    out.push(Source::LayoutBoxLabel { page: pi, id: b.id });
                }
            }
            for c in &page.cad {
                if matches!(c.item, CadItem::Text { .. }) {
                    out.push(Source::LayoutNote { page: pi, id: c.id });
                }
            }
            for l in &page.leaders {
                out.push(Source::LayoutLeader { page: pi, id: l.id });
            }
        }
    }
    out
}

/// The text of `s`.
pub fn get(project: &Project, s: &Source) -> Option<String> {
    let floor = |fi: usize| project.floors.get(fi);
    match s {
        Source::CadText { floor: fi, id } => {
            floor(*fi)?
                .cad
                .iter()
                .find(|c| c.id == *id)
                .and_then(|c| match &c.item {
                    CadItem::Text { text, .. } => Some(text.clone()),
                    _ => None,
                })
        }
        Source::DimText { floor: fi, id } => floor(*fi)?
            .dimensions
            .iter()
            .find(|d| d.id == *id)
            .and_then(|d| d.text_override.clone()),
        Source::OpeningLabel { floor: fi, id } => floor(*fi)?
            .openings
            .iter()
            .find(|o| o.id == *id)
            .and_then(|o| o.label_override.clone()),
        Source::SymbolLabel { floor: fi, id } => floor(*fi)?.symbol(*id).map(|s| s.label.clone()),
        Source::RoomName { floor: fi, index } => {
            floor(*fi)?.room_names.get(*index).map(|n| n.name.clone())
        }
        _ => {
            let layout = layout_window::load(project)?;
            match s {
                Source::LayoutPageTitle { page } => {
                    layout.pages.get(*page).map(|p| p.title.clone())
                }
                Source::LayoutBoxText { page, id } => layout
                    .pages
                    .get(*page)?
                    .boxes
                    .iter()
                    .find(|b| b.id == *id)
                    .and_then(|b| match &b.source {
                        BoxSource::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    }),
                Source::LayoutBoxLabel { page, id } => layout
                    .pages
                    .get(*page)?
                    .boxes
                    .iter()
                    .find(|b| b.id == *id)
                    .and_then(|b| b.label.clone()),
                Source::LayoutNote { page, id } => layout
                    .pages
                    .get(*page)?
                    .cad
                    .iter()
                    .find(|c| c.id == *id)
                    .and_then(|c| match &c.item {
                        CadItem::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    }),
                Source::LayoutLeader { page, id } => layout
                    .pages
                    .get(*page)?
                    .leaders
                    .iter()
                    .find(|l| l.id == *id)
                    .map(|l| l.text.clone()),
                _ => None,
            }
        }
    }
}

/// Where the text is, for the dialog.
pub fn describe(project: &Project, s: &Source) -> String {
    let fname = |fi: usize| {
        project
            .floors
            .get(fi)
            .map_or_else(String::new, |f| f.name.clone())
    };
    match s {
        Source::CadText { floor, .. } => format!("{}: text", fname(*floor)),
        Source::DimText { floor, .. } => format!("{}: dimension text", fname(*floor)),
        Source::OpeningLabel { floor, .. } => format!("{}: door or window label", fname(*floor)),
        Source::SymbolLabel { floor, .. } => format!("{}: symbol label", fname(*floor)),
        Source::RoomName { floor, .. } => format!("{}: room name", fname(*floor)),
        Source::LayoutBoxText { page, .. } => format!("Layout page {}: text box", page + 1),
        Source::LayoutBoxLabel { page, .. } => format!("Layout page {}: box caption", page + 1),
        Source::LayoutNote { page, .. } => format!("Layout page {}: note", page + 1),
        Source::LayoutLeader { page, .. } => format!("Layout page {}: leader", page + 1),
        Source::LayoutPageTitle { page } => format!("Layout page {}: title", page + 1),
    }
}

/// Replaces the bytes `start..end` of the text of `s` by `new`. False when
/// the text is not there or sits on a locked layer.
pub fn set_range(project: &mut Project, s: &Source, start: usize, end: usize, new: &str) -> bool {
    let Some(old) = get(project, s) else {
        return false;
    };
    if start > end || end > old.len() || !old.is_char_boundary(start) || !old.is_char_boundary(end)
    {
        return false;
    }
    let text = spell::splice(&old, start, end, new);
    match s {
        Source::CadText { floor, id } => {
            let Some(layer) = project
                .floors
                .get(*floor)
                .and_then(|f| f.cad.iter().find(|c| c.id == *id))
                .map(|c| c.layer.clone())
            else {
                return false;
            };
            if project.layers.is_locked(&layer) {
                return false;
            }
            let runs = project.floors[*floor]
                .cad_attrs(*id)
                .map(|a| a.runs)
                .filter(|r| !r.is_empty());
            if let Some(c) = project.floors[*floor].cad.iter_mut().find(|c| c.id == *id) {
                if let CadItem::Text { text: t, .. } = &mut c.item {
                    *t = text;
                }
            }
            if let Some(runs) = runs {
                let joined: String = runs.iter().map(|r| r.text.as_str()).collect();
                let runs = if joined == old {
                    spell::replace_range_in_runs(&runs, start, end, new)
                } else {
                    vec![plan_core::text_styles::RichRun {
                        text: spell::splice(&old, start, end, new),
                        ..runs[0].clone()
                    }]
                };
                project.edit_cad_attrs(*floor, *id, |a| a.runs = runs);
            }
            true
        }
        Source::DimText { floor, id } => project
            .floors
            .get_mut(*floor)
            .and_then(|f| f.dimensions.iter_mut().find(|d| d.id == *id))
            .map(|d| d.text_override = Some(text))
            .is_some(),
        Source::OpeningLabel { floor, id } => project
            .floors
            .get_mut(*floor)
            .and_then(|f| f.openings.iter_mut().find(|o| o.id == *id))
            .map(|o| o.label_override = Some(text))
            .is_some(),
        Source::SymbolLabel { floor, id } => project
            .floors
            .get_mut(*floor)
            .and_then(|f| f.symbols.iter_mut().find(|s| s.id == *id))
            .map(|s| s.label = text)
            .is_some(),
        Source::RoomName { floor, index } => project
            .floors
            .get_mut(*floor)
            .and_then(|f| f.room_names.get_mut(*index))
            .map(|n| n.name = text)
            .is_some(),
        _ => {
            let Some(mut layout) = layout_window::load(project) else {
                return false;
            };
            let done = match s {
                Source::LayoutPageTitle { page } => layout
                    .pages
                    .get_mut(*page)
                    .map(|p| p.title = text)
                    .is_some(),
                Source::LayoutBoxText { page, id } => layout
                    .pages
                    .get_mut(*page)
                    .and_then(|p| p.boxes.iter_mut().find(|b| b.id == *id))
                    .and_then(|b| match &mut b.source {
                        BoxSource::Text { text: t, .. } => Some(*t = text),
                        _ => None,
                    })
                    .is_some(),
                Source::LayoutBoxLabel { page, id } => layout
                    .pages
                    .get_mut(*page)
                    .and_then(|p| p.boxes.iter_mut().find(|b| b.id == *id))
                    .map(|b| b.label = Some(text))
                    .is_some(),
                Source::LayoutNote { page, id } => layout
                    .pages
                    .get_mut(*page)
                    .and_then(|p| p.cad.iter_mut().find(|c| c.id == *id))
                    .and_then(|c| match &mut c.item {
                        CadItem::Text { text: t, .. } => Some(*t = text),
                        _ => None,
                    })
                    .is_some(),
                Source::LayoutLeader { page, id } => layout
                    .pages
                    .get_mut(*page)
                    .and_then(|p| p.leaders.iter_mut().find(|l| l.id == *id))
                    .map(|l| l.text = text)
                    .is_some(),
                _ => false,
            };
            if done {
                layout_window::store(project, &layout);
            }
            done
        }
    }
}

// ----- the dialog -----

/// What the user pressed.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    Ignore,
    IgnoreAll,
    Add,
    Change(String),
    ChangeAll(String),
    Close,
}

/// The word being asked about and the lists the dialog shows.
struct Current {
    start: usize,
    end: usize,
    word: String,
    text: String,
    place: String,
    change_to: String,
    suggestions: Vec<String>,
}

impl Current {
    fn new(sp: &Speller, text: String, start: usize, end: usize, place: String) -> Self {
        let word = text[start..end].to_string();
        let suggestions = sp.suggest(&word, MAX_SUGGESTIONS);
        Self {
            start,
            end,
            change_to: suggestions.first().cloned().unwrap_or_else(|| word.clone()),
            word,
            text,
            place,
            suggestions,
        }
    }
}

/// The sentence around the word with the word marked.
fn context_job(c: &Current, ui: &egui::Ui) -> LayoutJob {
    const SIDE: usize = 48;
    let before: Vec<char> = c.text[..c.start].chars().collect();
    let after: Vec<char> = c.text[c.end..].chars().collect();
    let head: String = before[before.len().saturating_sub(SIDE)..].iter().collect();
    let tail: String = after[..after.len().min(SIDE)].iter().collect();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let plain = TextFormat::simple(font.clone(), ui.visuals().text_color());
    let mut job = LayoutJob::default();
    if before.len() > SIDE {
        job.append("\u{2026}", 0.0, plain.clone());
    }
    job.append(&head, 0.0, plain.clone());
    job.append(
        &c.word,
        0.0,
        TextFormat {
            underline: Stroke::new(1.5_f32, RED),
            color: RED,
            ..plain.clone()
        },
    );
    job.append(&tail, 0.0, plain.clone());
    if after.len() > SIDE {
        job.append("\u{2026}", 0.0, plain);
    }
    job
}

/// The Spelling dialog body. Returns the button pressed.
fn body(ui: &mut egui::Ui, c: &mut Current, footer: &str) -> Option<Cmd> {
    let mut cmd = None;
    ui.label(egui::RichText::new(&c.place).weak());
    ui.label("Not in dictionary:");
    let job = context_job(c, ui);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(380.0);
        ui.add(egui::Label::new(job).wrap());
    });
    ui.add_space(4.0);
    ui.label("Change to:");
    let edit = ui.add(
        egui::TextEdit::singleline(&mut c.change_to)
            .desired_width(f32::INFINITY)
            .id(egui::Id::new("spell_change_to")),
    );
    // Enter in the field is Change; the key must not reach the dialog below.
    if edit.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
    {
        cmd = Some(Cmd::Change(c.change_to.clone()));
    }
    ui.label("Suggestions:");
    egui::ScrollArea::vertical()
        .max_height(130.0)
        .id_salt("spell_suggestions")
        .show(ui, |ui| {
            if c.suggestions.is_empty() {
                ui.weak("(no suggestions)");
            }
            for s in c.suggestions.clone() {
                let r = ui.selectable_label(c.change_to == s, &s);
                if r.clicked() {
                    c.change_to = s.clone();
                }
                if r.double_clicked() {
                    cmd = Some(Cmd::Change(s));
                }
            }
        });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button("Ignore").clicked() {
            cmd = Some(Cmd::Ignore);
        }
        if ui.button("Ignore All").clicked() {
            cmd = Some(Cmd::IgnoreAll);
        }
        if ui.button("Add").clicked() {
            cmd = Some(Cmd::Add);
        }
        let ok = !c.change_to.trim().is_empty();
        if ui.add_enabled(ok, egui::Button::new("Change")).clicked() {
            cmd = Some(Cmd::Change(c.change_to.clone()));
        }
        if ui
            .add_enabled(ok, egui::Button::new("Change All"))
            .clicked()
        {
            cmd = Some(Cmd::ChangeAll(c.change_to.clone()));
        }
        if ui.button("Close").clicked() {
            cmd = Some(Cmd::Close);
        }
    });
    if !footer.is_empty() {
        ui.weak(footer);
    }
    cmd
}

/// A note on the state of the word list.
fn dictionary_note(sp: &Speller) -> String {
    if sp.system_words() == 0 {
        "No system word list was found (/usr/share/dict/words); only the built-in design \
         words and ~/.plan-studio/dictionary.txt are known."
            .to_string()
    } else {
        format!(
            "{} words + {} of yours (~/.plan-studio/dictionary.txt)",
            sp.system_words(),
            sp.custom_words()
        )
    }
}

fn finished_body(ui: &mut egui::Ui, text: &str) -> bool {
    ui.label(text);
    ui.add_space(6.0);
    ui.button("Close").clicked()
}

// ----- the whole-plan check -----

#[derive(Default)]
struct PlanSession {
    open: bool,
    /// Where the search resumes: source index and byte offset.
    source_idx: usize,
    offset: usize,
    current: Option<Current>,
    found: Option<Source>,
    changes: usize,
    finished: bool,
    note: String,
}

thread_local! {
    static PLAN: RefCell<PlanSession> = RefCell::new(PlanSession::default());
}

/// Is the plan check open?
pub fn is_open() -> bool {
    PLAN.with(|p| p.borrow().open)
}

/// The misspelled word the plan check is stopped at.
#[cfg(test)]
pub fn current_word() -> Option<String> {
    PLAN.with(|p| p.borrow().current.as_ref().map(|c| c.word.clone()))
}

/// The suggestions listed for the word the plan check is stopped at.
#[cfg(test)]
pub fn current_suggestions() -> Vec<String> {
    PLAN.with(|p| {
        p.borrow()
            .current
            .as_ref()
            .map(|c| c.suggestions.clone())
            .unwrap_or_default()
    })
}

/// Has the plan check reached the end of the plan?
#[cfg(test)]
pub fn is_finished() -> bool {
    PLAN.with(|p| p.borrow().finished)
}

/// Runs a menu or toolbar command by id; false when it is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    if id != OPEN {
        return false;
    }
    start(cx);
    true
}

/// Starts a check of the whole plan.
pub fn start(cx: &mut EditorContext) {
    let note = spell::with(|sp| {
        sp.reset_ignored();
        dictionary_note(sp)
    });
    PLAN.with(|p| {
        *p.borrow_mut() = PlanSession {
            open: true,
            note,
            ..PlanSession::default()
        }
    });
    advance(cx);
}

/// Finds the next misspelled word from the session's position.
fn advance(cx: &mut EditorContext) {
    let (mut i, mut off) = PLAN.with(|p| {
        let p = p.borrow();
        (p.source_idx, p.offset)
    });
    let srcs = sources(&cx.project);
    let mut found: Option<(usize, Source, Current)> = None;
    spell::with(|sp| {
        while i < srcs.len() {
            if let Some(text) = get(&cx.project, &srcs[i]) {
                if let Some(m) = sp
                    .misspellings(&text, false)
                    .into_iter()
                    .find(|m| m.start >= off)
                {
                    let place = describe(&cx.project, &srcs[i]);
                    found = Some((
                        i,
                        srcs[i].clone(),
                        Current::new(sp, text, m.start, m.end, place),
                    ));
                    return;
                }
            }
            i += 1;
            off = 0;
        }
    });
    match found {
        Some((i, src, cur)) => {
            select_source(cx, &src);
            PLAN.with(|p| {
                let mut p = p.borrow_mut();
                p.source_idx = i;
                p.offset = cur.start;
                p.found = Some(src);
                p.current = Some(cur);
                p.finished = false;
            });
        }
        None => PLAN.with(|p| {
            let mut p = p.borrow_mut();
            p.current = None;
            p.found = None;
            p.finished = true;
        }),
    }
}

/// Shows the object the word belongs to in the plan.
fn select_source(cx: &mut EditorContext, s: &Source) {
    let (floor, obj) = match s {
        Source::CadText { floor, id } => (*floor, Some(ObjectRef::Text(*id))),
        Source::DimText { floor, id } => (*floor, Some(ObjectRef::Dimension(*id))),
        Source::OpeningLabel { floor, id } => (*floor, Some(ObjectRef::Opening(*id))),
        Source::SymbolLabel { floor, id } => (*floor, Some(ObjectRef::Symbol(*id))),
        Source::RoomName { floor, .. } => (*floor, None),
        _ => return,
    };
    if floor != cx.floor && floor < cx.project.floors.len() {
        cx.floor = floor;
        cx.reset_view_state();
    }
    match obj {
        Some(o) => cx.selection.set(o),
        None => cx.selection.clear(),
    }
}

/// Applies a button of the plan check.
pub fn run(cx: &mut EditorContext, cmd: Cmd) {
    let Some((src, cur)) = PLAN.with(|p| {
        let p = p.borrow();
        p.found
            .clone()
            .zip(p.current.as_ref().map(|c| (c.start, c.end, c.word.clone())))
    }) else {
        if cmd == Cmd::Close {
            PLAN.with(|p| p.borrow_mut().open = false);
        }
        return;
    };
    let (start, end, word) = cur;
    let resume = |offset: usize| PLAN.with(|p| p.borrow_mut().offset = offset);
    match cmd {
        Cmd::Close => {
            PLAN.with(|p| p.borrow_mut().open = false);
            return;
        }
        Cmd::Ignore => resume(end),
        Cmd::IgnoreAll => {
            spell::with(|sp| sp.ignore(&word));
            resume(end);
        }
        Cmd::Add => {
            if let Err(e) = spell::with(|sp| sp.add_custom(&word)) {
                cx.status = format!("Spell Check: {e}");
            }
            resume(end);
        }
        Cmd::Change(new) => {
            cx.begin_change("Spell Check");
            if set_range(&mut cx.project, &src, start, end, &new) {
                cx.mark_dirty();
                cx.status = format!("Changed \"{word}\" to \"{new}\"");
                PLAN.with(|p| p.borrow_mut().changes += 1);
                resume(start + new.len());
            } else {
                cx.cancel_change();
                cx.status = "That text cannot be changed (locked layer?)".into();
                resume(end);
            }
        }
        Cmd::ChangeAll(new) => {
            let n = change_all(cx, &word, &new, "Spell Check: Change All");
            cx.status = format!("Changed {n} \"{word}\" to \"{new}\"");
            PLAN.with(|p| {
                let mut p = p.borrow_mut();
                p.changes += n;
                p.offset = start + new.len();
            });
        }
    }
    advance(cx);
}

/// Replaces every misspelled `word` (any case) of the plan by `new` as one
/// undo step. Returns how many were replaced.
pub fn change_all(cx: &mut EditorContext, word: &str, new: &str, label: &str) -> usize {
    let lower = word.to_lowercase();
    let srcs = sources(&cx.project);
    cx.begin_change(label);
    let mut n = 0;
    for s in &srcs {
        let Some(text) = get(&cx.project, s) else {
            continue;
        };
        let mut hits = spell::with(|sp| sp.misspellings(&text, false));
        hits.retain(|m| m.word.to_lowercase() == lower);
        // From the end, so the earlier ranges stay valid.
        for m in hits.iter().rev() {
            let repl = spell::adapt_case(&m.word, new);
            if set_range(&mut cx.project, s, m.start, m.end, &repl) {
                n += 1;
            }
        }
    }
    if n == 0 {
        cx.cancel_change();
    } else {
        cx.mark_dirty();
    }
    n
}

/// Draws the plan check when it is open (call every frame).
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    if !is_open() {
        return;
    }
    let mut open = true;
    let mut cmd = None;
    // The session is taken out while the window draws so the buttons can
    // call back into the editor.
    let mut session = PLAN.with(|p| std::mem::take(&mut *p.borrow_mut()));
    egui::Window::new("Spelling")
        .id(egui::Id::new("spell_check_plan"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            if let Some(c) = session.current.as_mut() {
                cmd = body(ui, c, &session.note);
            } else if session.finished {
                let text = format!(
                    "Spell check finished. {} change{}.",
                    session.changes,
                    if session.changes == 1 { "" } else { "s" }
                );
                if finished_body(ui, &text) {
                    cmd = Some(Cmd::Close);
                }
            }
        });
    PLAN.with(|p| *p.borrow_mut() = session);
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        open = false;
    }
    if !open {
        PLAN.with(|p| p.borrow_mut().open = false);
    }
    if let Some(c) = cmd {
        if PLAN.with(|p| p.borrow().current.is_some()) {
            run(cx, c);
        } else {
            PLAN.with(|p| p.borrow_mut().open = false);
        }
    }
}

// ----- the text dialog -----

#[derive(Default)]
struct LocalSession {
    open: bool,
    offset: usize,
    current: Option<Current>,
    changes: usize,
    finished: bool,
    note: String,
}

thread_local! {
    static LOCAL: RefCell<LocalSession> = RefCell::new(LocalSession::default());
}

fn local_advance(s: &mut LocalSession, text: &str, markup: bool) {
    let off = s.offset;
    let found = spell::with(|sp| {
        sp.misspellings(text, markup)
            .into_iter()
            .find(|m| m.start >= off)
            .map(|m| Current::new(sp, text.to_string(), m.start, m.end, "This text".into()))
    });
    s.finished = found.is_none();
    s.current = found;
}

/// The Check Spelling button of the text dialog and, while a check runs, the
/// Spelling window over `text`. `markup` is true when the text holds
/// `<b>`-style tags (they are not words). Returns true when the text changed.
pub fn local_controls(ui: &mut egui::Ui, text: &mut String, markup: bool) -> bool {
    let ctx = ui.ctx().clone();
    let mut changed = false;
    ui.horizontal(|ui| {
        if ui.button("Check Spelling\u{2026}").clicked() {
            let note = spell::with(|sp| {
                sp.reset_ignored();
                dictionary_note(sp)
            });
            let mut s = LocalSession {
                open: true,
                note,
                ..LocalSession::default()
            };
            local_advance(&mut s, text, markup);
            LOCAL.with(|l| *l.borrow_mut() = s);
        }
        let count = spell::with_ready(|sp| sp.misspellings(text, markup).len());
        match count {
            Some(0) => {
                ui.weak("No misspelled words");
            }
            Some(n) => {
                ui.colored_label(RED, format!("{n} misspelled"));
            }
            None => {
                ui.weak("Reading the word list\u{2026}");
            }
        }
    });
    if !LOCAL.with(|l| l.borrow().open) {
        return false;
    }
    let mut s = LOCAL.with(|l| std::mem::take(&mut *l.borrow_mut()));
    let mut open = true;
    let mut cmd = None;
    egui::Window::new("Spelling")
        .id(egui::Id::new("spell_check_local"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(&ctx, |ui| {
            if let Some(c) = s.current.as_mut() {
                cmd = body(ui, c, &s.note);
            } else {
                let t = format!(
                    "Spell check finished. {} change{}.",
                    s.changes,
                    if s.changes == 1 { "" } else { "s" }
                );
                if finished_body(ui, &t) {
                    cmd = Some(Cmd::Close);
                }
            }
        });
    // Escape closes only this window, not the dialog under it.
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        open = false;
    }
    if !open {
        s.open = false;
    }
    if let Some(c) = cmd {
        changed = local_run(&mut s, text, markup, c);
    }
    LOCAL.with(|l| *l.borrow_mut() = s);
    changed
}

/// Applies a button of the text dialog's check to `text`. True when the text
/// changed.
fn local_run(s: &mut LocalSession, text: &mut String, markup: bool, cmd: Cmd) -> bool {
    let Some((start, end, word)) = s.current.as_ref().map(|c| (c.start, c.end, c.word.clone()))
    else {
        s.open = false;
        return false;
    };
    let mut changed = false;
    match cmd {
        Cmd::Close => {
            s.open = false;
            return false;
        }
        Cmd::Ignore => s.offset = end,
        Cmd::IgnoreAll => {
            spell::with(|sp| sp.ignore(&word));
            s.offset = end;
        }
        Cmd::Add => {
            let _ = spell::with(|sp| sp.add_custom(&word));
            s.offset = end;
        }
        Cmd::Change(new) => {
            *text = spell::splice(text, start, end, &new);
            s.offset = start + new.len();
            s.changes += 1;
            changed = true;
        }
        Cmd::ChangeAll(new) => {
            let lower = word.to_lowercase();
            let mut hits = spell::with(|sp| sp.misspellings(text, markup));
            hits.retain(|m| m.word.to_lowercase() == lower);
            for m in hits.iter().rev() {
                *text = spell::splice(text, m.start, m.end, &spell::adapt_case(&m.word, &new));
                s.changes += 1;
            }
            s.offset = start + new.len();
            changed = !hits.is_empty();
        }
    }
    local_advance(s, text, markup);
    changed
}

// ----- the red underline -----

/// A `TextEdit` layouter that underlines misspelled words in red. `markup` is
/// true for text with `<b>`-style tags.
pub fn layouter(markup: bool) -> impl FnMut(&egui::Ui, &str, f32) -> Arc<egui::Galley> {
    move |ui, text, wrap_width| {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let plain = TextFormat::simple(font, ui.visuals().text_color());
        let spans = match spell::with_ready(|sp| sp.misspellings(text, markup)) {
            Some(s) => s,
            None => {
                // The word list is still being read.
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(150));
                Vec::new()
            }
        };
        let mut job = LayoutJob::default();
        let mut at = 0;
        for m in &spans {
            if m.start > at {
                job.append(&text[at..m.start], 0.0, plain.clone());
            }
            job.append(
                &text[m.start..m.end],
                0.0,
                TextFormat {
                    underline: Stroke::new(1.5_f32, RED),
                    ..plain.clone()
                },
            );
            at = m.end;
        }
        job.append(&text[at..], 0.0, plain);
        job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(job))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spell::{install_for_test, test_speller};
    use plan_core::geometry::Point;

    fn words() -> Speller {
        test_speller(&[
            "the", "kitchen", "master", "bedroom", "bath", "wall", "room", "door", "open", "to",
            "notes", "garage",
        ])
    }

    fn cx_with_text(texts: &[&str]) -> (EditorContext, Vec<Id>) {
        install_for_test(words(), None);
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let ids = texts
            .iter()
            .map(|t| {
                cx.project.add_cad(
                    0,
                    "CAD, Default",
                    CadItem::Text {
                        pos: Point::ZERO,
                        text: (*t).into(),
                        height: 3.0,
                        angle: 0.0,
                    },
                )
            })
            .collect();
        (cx, ids)
    }

    fn text_of(cx: &EditorContext, id: Id) -> String {
        get(&cx.project, &Source::CadText { floor: 0, id }).unwrap()
    }

    #[test]
    fn the_check_walks_the_text_of_the_plan_in_order() {
        let (mut cx, ids) = cx_with_text(&["The kitchn door", "fine notes", "the bedrom wall"]);
        start(&mut cx);
        assert_eq!(current_word().as_deref(), Some("kitchn"));
        // The text it stopped at is selected.
        assert_eq!(cx.selection.single(), Some(ObjectRef::Text(ids[0])));
        let suggestions = PLAN.with(|p| p.borrow().current.as_ref().unwrap().suggestions.clone());
        assert_eq!(suggestions.first().map(String::as_str), Some("kitchen"));
        run(&mut cx, Cmd::Ignore);
        assert_eq!(current_word().as_deref(), Some("bedrom"));
        run(&mut cx, Cmd::Ignore);
        assert!(current_word().is_none());
        assert!(PLAN.with(|p| p.borrow().finished));
    }

    #[test]
    fn change_replaces_the_word_and_is_one_undo_step() {
        let (mut cx, ids) = cx_with_text(&["The kitchn door"]);
        let steps = cx.action_history().0.len();
        start(&mut cx);
        run(&mut cx, Cmd::Change("kitchen".into()));
        assert_eq!(text_of(&cx, ids[0]), "The kitchen door");
        assert_eq!(cx.action_history().0.len(), steps + 1);
        assert_eq!(cx.undo_label(), Some("Spell Check"));
        cx.undo();
        assert_eq!(text_of(&cx, ids[0]), "The kitchn door");
    }

    #[test]
    fn change_all_fixes_every_occurrence_in_one_step_keeping_the_case() {
        let (mut cx, ids) = cx_with_text(&["kitchn and Kitchn", "KITCHN", "bedrom"]);
        let steps = cx.action_history().0.len();
        start(&mut cx);
        run(&mut cx, Cmd::ChangeAll("kitchen".into()));
        assert_eq!(text_of(&cx, ids[0]), "kitchen and Kitchen");
        assert_eq!(text_of(&cx, ids[1]), "KITCHEN");
        assert_eq!(text_of(&cx, ids[2]), "bedrom");
        assert_eq!(cx.action_history().0.len(), steps + 1);
        // The check goes on to the next problem.
        assert_eq!(current_word().as_deref(), Some("bedrom"));
        cx.undo();
        assert_eq!(text_of(&cx, ids[1]), "KITCHN");
    }

    #[test]
    fn ignore_all_skips_the_word_everywhere_and_add_teaches_it() {
        let (mut cx, _) = cx_with_text(&["zorb here", "the zorb", "plugh"]);
        start(&mut cx);
        assert_eq!(current_word().as_deref(), Some("zorb"));
        run(&mut cx, Cmd::IgnoreAll);
        assert_eq!(current_word().as_deref(), Some("plugh"));
        run(&mut cx, Cmd::Add);
        assert!(current_word().is_none());
        assert!(spell::with(|sp| sp.is_correct("plugh")));
        // A new check forgets Ignore All but not Add.
        start(&mut cx);
        assert_eq!(current_word().as_deref(), Some("zorb"));
    }

    #[test]
    fn rich_text_runs_stay_in_step_with_the_words() {
        let (mut cx, ids) = cx_with_text(&["a kitchn here"]);
        let runs = vec![
            plan_core::text_styles::RichRun::plain("a "),
            plan_core::text_styles::RichRun {
                bold: true,
                ..plan_core::text_styles::RichRun::plain("kitchn")
            },
            plan_core::text_styles::RichRun::plain(" here"),
        ];
        cx.project.edit_cad_attrs(0, ids[0], |a| a.runs = runs);
        start(&mut cx);
        run(&mut cx, Cmd::Change("kitchen".into()));
        assert_eq!(text_of(&cx, ids[0]), "a kitchen here");
        let runs = cx.floor().cad_attrs(ids[0]).unwrap().runs;
        let joined: String = runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(joined, "a kitchen here");
        assert!(runs.iter().any(|r| r.bold && r.text == "kitchen"));
    }

    #[test]
    fn labels_names_and_dimension_text_are_checked_too() {
        let (mut cx, _) = cx_with_text(&[]);
        let d = cx.project.add_dimension(
            0,
            plan_core::Dimension::new(
                0,
                plan_core::DimensionKind::Manual,
                Point::ZERO,
                Point::new(100.0, 0.0),
                12.0,
            ),
        );
        cx.project.floors[0]
            .dimensions
            .iter_mut()
            .find(|x| x.id == d)
            .unwrap()
            .text_override = Some("Garge door".into());
        cx.project.floors[0]
            .room_names
            .push(plan_core::RoomName::new(
                Point::new(10.0, 10.0),
                "Mastr bath",
                "Bath",
            ));
        let srcs = sources(&cx.project);
        assert!(srcs.contains(&Source::DimText { floor: 0, id: d }));
        assert!(srcs.contains(&Source::RoomName { floor: 0, index: 0 }));
        start(&mut cx);
        assert_eq!(current_word().as_deref(), Some("Garge"));
        run(&mut cx, Cmd::Change("Garage".into()));
        assert_eq!(current_word().as_deref(), Some("Mastr"));
        run(&mut cx, Cmd::Change("Master".into()));
        assert_eq!(cx.floor().room_names[0].name, "Master bath");
        let dim = cx.floor().dimensions.iter().find(|x| x.id == d).unwrap();
        assert_eq!(dim.text_override.as_deref(), Some("Garage door"));
    }

    #[test]
    fn a_locked_layer_refuses_the_change_without_an_undo_step() {
        let (mut cx, ids) = cx_with_text(&["The kitchn"]);
        cx.project.layers.set_locked("CAD, Default", true);
        let steps = cx.action_history().0.len();
        start(&mut cx);
        run(&mut cx, Cmd::Change("kitchen".into()));
        assert_eq!(text_of(&cx, ids[0]), "The kitchn");
        assert_eq!(cx.action_history().0.len(), steps);
    }

    #[test]
    fn the_text_dialog_check_works_on_the_typed_text() {
        install_for_test(words(), None);
        let mut text = "<b>kitchn</b> and the bedrom".to_string();
        let mut s = LocalSession::default();
        local_advance(&mut s, &text, true);
        assert_eq!(s.current.as_ref().map(|c| c.word.as_str()), Some("kitchn"));
        assert!(local_run(
            &mut s,
            &mut text,
            true,
            Cmd::Change("kitchen".into())
        ));
        assert_eq!(text, "<b>kitchen</b> and the bedrom");
        assert_eq!(s.current.as_ref().map(|c| c.word.as_str()), Some("bedrom"));
        assert!(!local_run(&mut s, &mut text, true, Cmd::Ignore));
        assert!(s.current.is_none() && s.finished);
        // Change All inside the text.
        let mut text = "bedrom, bedrom and Bedrom".to_string();
        let mut s = LocalSession::default();
        local_advance(&mut s, &text, false);
        assert!(local_run(
            &mut s,
            &mut text,
            false,
            Cmd::ChangeAll("bedroom".into())
        ));
        assert_eq!(text, "bedroom, bedroom and Bedroom");
    }

    #[test]
    fn the_underline_layouter_marks_misspelled_words() {
        install_for_test(words(), None);
        let ctx = egui::Context::default();
        let mut marked = 0;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut lay = layouter(false);
                let galley = lay(ui, "the kitchn is fine", 400.0);
                marked = galley
                    .job
                    .sections
                    .iter()
                    .filter(|s| s.format.underline.width > 0.0)
                    .count();
            });
        });
        assert_eq!(marked, 1);
    }

    #[test]
    fn the_dialog_windows_draw() {
        let (mut cx, _) = cx_with_text(&["The kitchn door"]);
        start(&mut cx);
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show_all(ctx, &mut cx));
        }
        let mut text = "kitchn".to_string();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                local_controls(ui, &mut text, false);
            });
        });
        assert_eq!(text, "kitchn");
    }
}
