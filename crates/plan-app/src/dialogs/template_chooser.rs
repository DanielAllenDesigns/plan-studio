//! File > Templates (manual pp. 110 to 114): New Plan from Template and New
//! Layout from Template choosers, Save as Template with its purge checklist,
//! and the prompt that offers Load Installed or Load Custom when the
//! configured template is missing.
//!
//! Plan templates are Plan Studio files (`templates::PlanTemplate`) in
//! `~/.plan-studio/templates/plans`; layout templates are the ones Layout >
//! Save As Template makes. Chief's own `.plan` and `.layout` templates keep
//! seeding the defaults through `templates.rs` as before.
//!
//! Commands: [`NEW_PLAN`], [`NEW_LAYOUT`], [`SAVE_AS`].

use crate::editor::EditorContext;
use crate::templates::{self, DefaultTemplate, OwnTemplateSettings, TemplateEntry};
use eframe::egui::{self, Align, Align2, Layout};
use plan_core::defaults::template::{PlanTemplate, PurgeCategory};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// File > Templates > New Plan from Template.
pub const NEW_PLAN: &str = "templates.new_plan";
/// File > Templates > New Layout from Template.
pub const NEW_LAYOUT: &str = "templates.new_layout";
/// File > Templates > Save as Template.
pub const SAVE_AS: &str = "templates.save_as";

/// What the shell should do after a window answered.
#[derive(Debug, Clone)]
pub enum Request {
    /// Start an untitled plan from this template.
    NewPlan(Box<PlanTemplate>),
    /// Start an untitled plan from the installed defaults.
    NewInstalled,
}

#[derive(Default)]
struct State {
    chooser: Option<Chooser>,
    save: Option<SaveAs>,
    missing: Option<Missing>,
    request: Option<Request>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// The answer waiting for the shell, if any.
pub fn take_request() -> Option<Request> {
    state(|s| s.request.take())
}

/// Runs a command by id; false when it is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        NEW_PLAN => open_chooser(ChooserKind::Plan),
        NEW_LAYOUT => open_chooser(ChooserKind::Layout),
        SAVE_AS => open_save_as(cx),
        _ => return false,
    }
    true
}

// ===================================================================
// The chooser
// ===================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChooserKind {
    Plan,
    Layout,
}

/// One row of the chooser.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    label: String,
    imperial: bool,
    what: RowKind,
}

#[derive(Clone, Debug, PartialEq)]
enum RowKind {
    /// The installed defaults (what New Plan starts from without a template).
    Installed,
    Plan(TemplateEntry),
    Layout(String),
}

struct Chooser {
    kind: ChooserKind,
    selected: usize,
    settings: OwnTemplateSettings,
    message: String,
}

fn open_chooser(kind: ChooserKind) {
    state(|s| {
        s.chooser = Some(Chooser {
            kind,
            selected: 0,
            settings: templates::own_settings(),
            message: String::new(),
        })
    });
}

/// Is the chooser open?
pub fn chooser_open() -> Option<ChooserKind> {
    state(|s| s.chooser.as_ref().map(|c| c.kind))
}

fn rows(c: &Chooser, imperial_now: bool) -> Vec<Row> {
    let mut out = Vec::new();
    match c.kind {
        ChooserKind::Plan => {
            out.push(Row {
                label: "Installed Template".into(),
                imperial: imperial_now,
                what: RowKind::Installed,
            });
            for e in templates::list_plan_templates() {
                out.push(Row {
                    label: e.name.clone(),
                    imperial: e.imperial,
                    what: RowKind::Plan(e),
                });
            }
        }
        ChooserKind::Layout => {
            if let Some(dir) = crate::dialogs::layout::layout_templates_dir() {
                for t in crate::dialogs::layout::list_layout_templates(&dir) {
                    let metric = t.name.ends_with(templates::METRIC_SUFFIX);
                    out.push(Row {
                        label: t.name.clone(),
                        imperial: !metric,
                        what: RowKind::Layout(t.name.clone()),
                    });
                }
            }
        }
    }
    out.retain(|r| !(r.imperial && c.settings.hide_us) && !(!r.imperial && c.settings.hide_metric));
    out
}

fn show_chooser(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut c) = state(|s| s.chooser.take()) else {
        return;
    };
    let list = rows(&c, cx.defaults.units.imperial);
    c.selected = c.selected.min(list.len().saturating_sub(1));
    let mut open = true;
    let mut create = false;
    let mut cancel = false;
    let title = match c.kind {
        ChooserKind::Plan => "New Plan from Template",
        ChooserKind::Layout => "New Layout from Template",
    };
    let mut settings = c.settings.clone();
    egui::Window::new(title)
        .id(egui::Id::new("template_chooser"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label("The new file uses the units of the template: pick a U.S. or a metric one.");
            egui::ScrollArea::vertical()
                .id_salt("template_rows")
                .max_height(240.0)
                .min_scrolled_height(120.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (i, r) in list.iter().enumerate() {
                        let units = if r.imperial { "U.S." } else { "Metric" };
                        let default_mark = match &r.what {
                            RowKind::Plan(e) => {
                                let file = e
                                    .path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default();
                                (c.settings.default_for(e.imperial) == Some(file.as_str()))
                                    .then_some("  (default)")
                            }
                            _ => None,
                        };
                        let text = format!("{}  [{units}]{}", r.label, default_mark.unwrap_or(""));
                        let resp = ui.selectable_label(i == c.selected, text);
                        if resp.clicked() {
                            c.selected = i;
                        }
                        if resp.double_clicked() {
                            c.selected = i;
                            create = true;
                        }
                    }
                    if list.is_empty() {
                        ui.weak("No templates here. Save a plan as a template first.");
                    }
                });
            ui.horizontal(|ui| {
                ui.checkbox(&mut settings.hide_metric, "Hide Metric Templates");
                ui.checkbox(&mut settings.hide_us, "Hide U.S. Templates");
            });
            if !c.message.is_empty() {
                ui.label(&c.message);
            }
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(!list.is_empty(), egui::Button::new("Create New"))
                    .clicked()
                {
                    create = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if settings != c.settings {
        c.settings = settings;
        let _ = templates::save_own_settings(&c.settings);
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if create {
        if let Some(r) = list.get(c.selected).cloned() {
            match answer(cx, &r) {
                Ok(()) => return,
                Err(e) => c.message = e,
            }
        }
    }
    if open && !cancel {
        state(|s| s.chooser = Some(c));
    }
}

/// The chooser's answer for a row: asks the shell to start the plan, or
/// makes the layout.
fn answer(cx: &mut EditorContext, r: &Row) -> Result<(), String> {
    match &r.what {
        RowKind::Installed => {
            state(|s| s.request = Some(Request::NewInstalled));
            Ok(())
        }
        RowKind::Plan(e) => {
            let t = templates::load_plan_template(&e.path)?;
            state(|s| s.request = Some(Request::NewPlan(Box::new(t))));
            Ok(())
        }
        RowKind::Layout(name) => {
            let dir = crate::dialogs::layout::layout_templates_dir().ok_or("No template folder")?;
            let t = crate::dialogs::layout::list_layout_templates(&dir)
                .into_iter()
                .find(|t| &t.name == name)
                .ok_or("That template is gone")?;
            if crate::shell::layout_window::new_layout_from_template(cx, &t) {
                Ok(())
            } else {
                Err(cx.status.clone())
            }
        }
    }
}

// ===================================================================
// Save as Template
// ===================================================================

struct SaveAs {
    name: String,
    purge: BTreeSet<PurgeCategory>,
    set_default: bool,
    open_new: bool,
    message: String,
}

fn open_save_as(cx: &EditorContext) {
    let stem = if cx.project.name.trim().is_empty() || cx.project.name == "Untitled" {
        "My Template".to_string()
    } else {
        format!("{} Template", cx.project.name.trim())
    };
    state(|s| {
        s.save = Some(SaveAs {
            name: stem,
            // Unless there is a reason to keep a category, delete them all.
            purge: PurgeCategory::ALL.iter().copied().collect(),
            set_default: false,
            open_new: false,
            message: String::new(),
        })
    });
}

/// Is the Save as Template window open?
pub fn save_as_open() -> bool {
    state(|s| s.save.is_some())
}

/// Saves the open plan as a template: the chosen categories are purged from
/// the copy, the file goes in the template folder, and, when asked, it
/// becomes the default for its units. Returns the file.
pub fn save_template(
    cx: &mut EditorContext,
    name: &str,
    purge: &BTreeSet<PurgeCategory>,
    set_default: bool,
) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Type a name for the template".into());
    }
    cx.project.saved_commit_all(&mut cx.defaults);
    let t = PlanTemplate::from_plan(name, &cx.project, &cx.defaults, purge);
    let path = templates::save_plan_template(&t)?;
    if set_default {
        let mut s = templates::own_settings();
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        s.set_default(t.imperial, &file);
        templates::save_own_settings(&s)?;
    }
    Ok(path)
}

fn show_save_as(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut w) = state(|s| s.save.take()) else {
        return;
    };
    let mut ok = false;
    let mut cancel = false;
    let mut open = true;
    let imperial = cx.defaults.units.imperial;
    egui::Window::new("Save as Plan Template")
        .id(egui::Id::new("save_as_template"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Template Name");
                ui.add(egui::TextEdit::singleline(&mut w.name).desired_width(240.0));
            });
            ui.add_space(4.0);
            ui.strong("Delete From the Template");
            ui.weak("Check each category of object or data to delete from the saved copy.");
            egui::ScrollArea::vertical()
                .id_salt("purge_list")
                .max_height(240.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for c in PurgeCategory::ALL {
                        let mut on = w.purge.contains(&c);
                        if ui.checkbox(&mut on, c.label()).changed() {
                            if on {
                                w.purge.insert(c);
                            } else {
                                w.purge.remove(&c);
                            }
                        }
                    }
                });
            ui.horizontal(|ui| {
                if ui.button("Select All").clicked() {
                    w.purge = PurgeCategory::ALL.iter().copied().collect();
                }
                if ui.button("Clear All").clicked() {
                    w.purge.clear();
                }
            });
            ui.add_space(4.0);
            ui.checkbox(
                &mut w.set_default,
                if imperial {
                    "Set Template as Default for U.S. Units Plans"
                } else {
                    "Set Template as Default for Metric Plans"
                },
            );
            ui.checkbox(&mut w.open_new, "Open New Plan After Saving Template");
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
        match save_template(cx, &w.name, &w.purge, w.set_default) {
            Ok(path) => {
                cx.status = format!("Saved the template to {}", path.display());
                if w.open_new {
                    if let Ok(t) = templates::load_plan_template(&path) {
                        state(|s| s.request = Some(Request::NewPlan(Box::new(t))));
                    }
                }
                return;
            }
            Err(e) => w.message = e,
        }
    }
    if open && !cancel {
        state(|s| s.save = Some(w));
    }
}

// ===================================================================
// The template is missing
// ===================================================================

struct Missing {
    file: String,
    imperial: bool,
}

/// Opens the prompt that says the configured template cannot be found and
/// offers to load the installed one or to pick another file (manual p. 49).
pub fn open_missing(file: String, imperial: bool) {
    state(|s| s.missing = Some(Missing { file, imperial }));
}

/// Is the missing-template prompt open?
pub fn missing_open() -> bool {
    state(|s| s.missing.is_some())
}

fn show_missing(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(m) = state(|s| s.missing.take()) else {
        return;
    };
    let mut installed = false;
    let mut custom = false;
    let mut cancel = false;
    egui::Window::new("Select Template")
        .id(egui::Id::new("template_missing"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(format!(
                "The {} template \"{}\" cannot be found.",
                if m.imperial { "U.S." } else { "metric" },
                m.file
            ));
            ui.label("Load the installed template, or choose another file.");
            ui.horizontal(|ui| {
                if ui.button("Load Installed").clicked() {
                    installed = true;
                }
                if ui.button("Load Custom\u{2026}").clicked() {
                    custom = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if installed {
        state(|s| s.request = Some(Request::NewInstalled));
    } else if custom {
        let mut dlg =
            rfd::FileDialog::new().add_filter("Plan template", &[templates::PLAN_TEMPLATE_EXT]);
        if let Some(dir) = templates::plan_templates_dir().filter(|d| d.is_dir()) {
            dlg = dlg.set_directory(dir);
        }
        match dlg.pick_file() {
            Some(p) => match templates::load_plan_template(&p) {
                Ok(t) => state(|s| s.request = Some(Request::NewPlan(Box::new(t)))),
                Err(e) => {
                    cx.status = format!("Could not read the template: {e}");
                    state(|s| s.missing = Some(m));
                }
            },
            None => state(|s| s.missing = Some(m)),
        }
    } else if !cancel {
        state(|s| s.missing = Some(m));
    }
}

// ===================================================================
// The shell
// ===================================================================

/// Draws the windows of this module; call once a frame.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    show_chooser(ctx, cx);
    show_save_as(ctx, cx);
    show_missing(ctx, cx);
}

impl crate::PlanApp {
    /// New Plan with a configured default template: starts the plan from it
    /// (`true`), or asks what to do when the file is gone (`true`: nothing
    /// else happens yet). `false` when no Plan Studio template is configured.
    pub(crate) fn new_project_from_default_template(&mut self) -> bool {
        let imperial = self.cx.defaults.units.imperial;
        match templates::default_plan_template(imperial) {
            DefaultTemplate::NotConfigured => false,
            DefaultTemplate::Found(p) => match templates::load_plan_template(&p) {
                Ok(t) => {
                    self.start_plan_from(&t);
                    true
                }
                Err(e) => {
                    open_missing(p.display().to_string(), imperial);
                    self.cx.status = format!("Could not read the default template: {e}");
                    true
                }
            },
            DefaultTemplate::Missing(file) => {
                open_missing(file, imperial);
                true
            }
        }
    }

    /// Starts an untitled plan from `t`: its defaults and its plan.
    pub(crate) fn start_plan_from(&mut self, t: &PlanTemplate) {
        let (project, defaults) = t.instantiate();
        self.cx.defaults = defaults;
        crate::dialogs::defaults::forget_dialog_edits(&mut self.cx);
        self.cx.set_project(project);
        self.path = None;
        self.tools.restart(&mut self.cx);
        self.files.rebaseline(&self.cx);
        self.cx.status = format!("New plan from the template \"{}\"", t.name);
    }

    /// Carries out what a template window answered; call once a frame.
    pub(crate) fn poll_template_requests(&mut self) {
        match take_request() {
            Some(Request::NewPlan(t)) => self.start_plan_from(&t),
            Some(Request::NewInstalled) => self.new_project_plain(),
            None => {}
        }
    }
}

/// What clicking Load Installed in the missing-template prompt does (tests).
#[cfg(test)]
pub(crate) fn load_installed_for_tests() {
    state(|s| {
        s.missing = None;
        s.request = Some(Request::NewInstalled);
    });
}

#[cfg(test)]
pub(crate) fn reset_state() {
    state(|s| *s = State::default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;
    use plan_core::WallKind;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("plan-studio-tpl-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn cx_with_walls() -> EditorContext {
        let mut cx = crate::editor::plan_tabs::plain_cx();
        cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        cx.project.name = "Smith".into();
        cx
    }

    #[test]
    fn save_as_template_purges_the_copy_and_sets_the_default_per_units() {
        reset_state();
        let dir = tmp("save");
        templates::set_plan_templates_dir_for_tests(Some(dir.clone()));
        let mut cx = cx_with_walls();
        let all: BTreeSet<_> = PurgeCategory::ALL.iter().copied().collect();
        assert!(save_template(&mut cx, "  ", &all, false).is_err());
        let path = save_template(&mut cx, "Smith Template", &all, true).unwrap();
        assert_eq!(cx.floor().walls.len(), 1, "the open plan keeps its walls");
        let t = templates::load_plan_template(&path).unwrap();
        assert!(t.project.floors[0].walls.is_empty());
        assert_eq!(t.project.name, "Untitled");
        assert_eq!(
            templates::own_settings().plan_us.as_deref(),
            Some("Smith Template.pstemplate")
        );
        assert_eq!(templates::own_settings().plan_metric, None);
        let found = templates::default_plan_template(true);
        assert_eq!(found, DefaultTemplate::Found(path.clone()));
        // A metric plan's template is named for its units and is the metric default.
        cx.defaults.units.imperial = false;
        let m = save_template(&mut cx, "Smith Template", &BTreeSet::new(), true).unwrap();
        assert!(m
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("- Metric.pstemplate"));
        assert_eq!(
            templates::own_settings().plan_metric.as_deref(),
            Some("Smith Template - Metric.pstemplate")
        );
        let listed = templates::list_plan_templates();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|e| e.imperial) && listed.iter().any(|e| !e.imperial));
        // Nothing purged: the walls are in the metric copy.
        assert_eq!(
            templates::load_plan_template(&m).unwrap().project.floors[0]
                .walls
                .len(),
            1
        );
        // The default goes missing: the prompt's case.
        std::fs::remove_file(&path).unwrap();
        cx.defaults.units.imperial = true;
        assert_eq!(
            templates::default_plan_template(true),
            DefaultTemplate::Missing("Smith Template.pstemplate".into())
        );
        templates::set_plan_templates_dir_for_tests(None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_chooser_hides_metric_or_us_templates_and_answers_with_the_template() {
        reset_state();
        let dir = tmp("chooser");
        templates::set_plan_templates_dir_for_tests(Some(dir.clone()));
        let mut cx = cx_with_walls();
        save_template(&mut cx, "Alpha", &BTreeSet::new(), false).unwrap();
        cx.defaults.units.imperial = false;
        save_template(&mut cx, "Alpha", &BTreeSet::new(), false).unwrap();
        cx.defaults.units.imperial = true;
        run_command(&mut cx, NEW_PLAN);
        assert_eq!(chooser_open(), Some(ChooserKind::Plan));
        let c = state(|s| s.chooser.take()).unwrap();
        assert_eq!(rows(&c, true).len(), 3, "installed, U.S., metric");
        let mut hide = c.settings.clone();
        hide.hide_metric = true;
        let c2 = Chooser {
            settings: hide,
            ..c
        };
        let list = rows(&c2, true);
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|r| r.imperial));
        let alpha = list.iter().find(|r| r.label == "Alpha").unwrap().clone();
        answer(&mut cx, &alpha).unwrap();
        match take_request() {
            Some(Request::NewPlan(t)) => assert_eq!(t.name, "Alpha"),
            other => panic!("{other:?}"),
        }
        answer(&mut cx, &list[0]).unwrap();
        assert!(matches!(take_request(), Some(Request::NewInstalled)));
        templates::set_plan_templates_dir_for_tests(None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_windows_draw() {
        reset_state();
        let mut cx = cx_with_walls();
        let ctx = egui::Context::default();
        run_command(&mut cx, NEW_PLAN);
        run_command(&mut cx, NEW_LAYOUT);
        run_command(&mut cx, SAVE_AS);
        open_missing("Gone.pstemplate".into(), true);
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show_all(ctx, &mut cx));
        }
        assert!(save_as_open() && missing_open());
        reset_state();
        assert!(!run_command(&mut cx, "nope"));
    }
}
