//! The Plan Check window (Tools > Checks > Plan Check and Door/Window
//! Check): findings one at a time with Previous, Next, Zoom to, Ignore, the
//! Plan Check Settings dialog, and the report as Markdown, PDF or a layout
//! page.
//!
//! The window is hosted by `build_tools` (`Windows::check`). Everything that
//! changes the plan (Ignore, new settings) is one undo step through
//! `cx.begin_change`. The rules, the settings and the ignore list live in
//! `plan-check`; the settings and the ignore list are kept with the plan
//! (`plan_check::CheckSettings::store`, `set_ignored_keys`).

use crate::editor::rooms_edit;
use crate::editor::{Camera, EditorContext, ObjectRef};
use eframe::egui::{self, Color32, RichText};
use plan_check::{CheckOptions, CheckRun, CheckSettings, Finding, Severity, Target, JURISDICTIONS};
use plan_docs::{PdfDoc, Schedule};
use plan_stairs::Stair;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckKind {
    Plan,
    DoorWindow,
}

impl CheckKind {
    pub fn title(self) -> &'static str {
        match self {
            CheckKind::Plan => "Plan Check",
            CheckKind::DoorWindow => "Door/Window Check",
        }
    }
}

/// The findings of one check and the one being shown.
pub struct CheckWindow {
    pub kind: CheckKind,
    pub(crate) findings: Vec<Finding>,
    index: usize,
    /// Findings the user ignored (kept for the count and Restore).
    ignored: Vec<Finding>,
    /// The floor the check ran on.
    floor: usize,
    /// Zoom to the object when stepping to a finding.
    zoom_each: bool,
    /// Zoom to the first finding on the next frame.
    pending_zoom: bool,
    settings: Option<SettingsDialog>,
}

impl CheckWindow {
    pub fn new(kind: CheckKind, findings: Vec<Finding>) -> Self {
        Self {
            kind,
            findings,
            index: 0,
            ignored: Vec::new(),
            floor: 0,
            zoom_each: true,
            pending_zoom: false,
            settings: None,
        }
    }

    /// A window for the result of a check run on `floor`.
    pub fn from_run(kind: CheckKind, floor: usize, run: CheckRun) -> Self {
        let mut w = Self::new(kind, run.findings);
        w.floor = floor;
        w.ignored = run.ignored;
        w.pending_zoom = true;
        w
    }

    pub fn count(&self) -> usize {
        self.findings.len()
    }

    pub fn ignored_count(&self) -> usize {
        self.ignored.len()
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn current(&self) -> Option<&Finding> {
        self.findings.get(self.index)
    }

    pub fn can_next(&self) -> bool {
        self.index + 1 < self.findings.len()
    }

    pub fn can_previous(&self) -> bool {
        self.index > 0
    }

    /// Next finding; stays on the last one.
    pub fn next(&mut self) -> bool {
        if self.can_next() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    pub fn previous(&mut self) -> bool {
        if self.can_previous() {
            self.index -= 1;
            true
        } else {
            false
        }
    }

    /// "Finding 3 of 12", or "No findings".
    pub fn position_text(&self) -> String {
        if self.count() == 0 {
            "No findings".into()
        } else {
            format!("Finding {} of {}", self.index() + 1, self.count())
        }
    }

    /// Replaces the findings after a re-run, staying near the same place.
    pub fn replace(&mut self, findings: Vec<Finding>) {
        self.index = self.index.min(findings.len().saturating_sub(1));
        self.findings = findings;
    }

    /// Replaces the findings and the ignored ones after a re-run.
    pub fn replace_run(&mut self, run: CheckRun) {
        self.ignored = run.ignored;
        self.replace(run.findings);
    }

    /// The status-bar line for the findings left and the ones ignored.
    pub fn summary(&self) -> String {
        plan_check::summary_line(self.kind.title(), &self.findings, self.ignored.len())
    }

    /// Ignores the finding being shown: it is remembered in the plan and drops
    /// out of the list (one undo step). Returns whether there was one.
    pub fn ignore_current(&mut self, cx: &mut EditorContext) -> bool {
        if self.index >= self.findings.len() {
            return false;
        }
        let f = self.findings.remove(self.index);
        cx.begin_change("Ignore Plan Check Finding");
        let mut keys = plan_check::ignored_keys(&cx.project);
        keys.insert(plan_check::finding_key(self.floor, &f));
        plan_check::set_ignored_keys(&mut cx.project, &keys);
        cx.mark_dirty();
        self.ignored.push(f);
        self.index = self.index.min(self.findings.len().saturating_sub(1));
        true
    }

    /// Forgets every ignored finding of the plan and checks again.
    pub fn restore_ignored(&mut self, cx: &mut EditorContext) {
        cx.begin_change("Restore Ignored Plan Check Findings");
        plan_check::set_ignored_keys(&mut cx.project, &Default::default());
        cx.mark_dirty();
        let run = run_check_full(cx, self.kind);
        self.replace_run(run);
    }

    pub fn report(&self) -> String {
        plan_check::report_markdown(&self.findings)
    }

    /// The findings as a plan-docs table (the report page of the layout).
    pub fn report_schedule(&self, floor_name: &str) -> Schedule {
        let t = plan_check::report_table(floor_name, &self.findings);
        Schedule {
            title: t.title,
            columns: t.columns,
            rows: t.rows,
        }
    }
}

/// Runs Plan Check or Door/Window Check on the active floor with the plan's
/// settings and ignore list; the findings left to look at.
#[cfg(test)]
pub fn run_check(cx: &mut EditorContext, kind: CheckKind) -> Vec<Finding> {
    run_check_full(cx, kind).findings
}

/// [`run_check`] with the ignored findings too.
pub fn run_check_full(cx: &mut EditorContext, kind: CheckKind) -> CheckRun {
    cx.refresh();
    match kind {
        CheckKind::DoorWindow => {
            let all = plan_check::door_window_check(&cx.project, cx.floor);
            plan_check::filter_findings(&cx.project, cx.floor, all)
        }
        CheckKind::Plan => {
            let room_types: Vec<(usize, String)> = cx
                .rooms
                .iter()
                .enumerate()
                .filter_map(|(i, r)| {
                    rooms_edit::name_entry(cx, r).map(|n| (i, n.room_type.clone()))
                })
                .collect();
            let stairs: Vec<Stair> = cx.floor().stairs_as().unwrap_or_default();
            plan_check::run_plan_check(&cx.project, cx.floor, &cx.rooms, &room_types, &stairs)
        }
    }
}

/// "Zoom to": centers the view on the finding and selects its object.
pub fn zoom_to_finding(cx: &mut EditorContext, cam: &mut Camera, f: &Finding) {
    if let Some(p) = f.location {
        cam.center = p;
    }
    match f.object {
        Some(Target::Wall(id)) => cx.select_only(ObjectRef::Wall(id)),
        Some(Target::Opening(id)) => cx.select_only(ObjectRef::Opening(id)),
        Some(Target::Room(i)) => rooms_edit::select_room(cx, i),
        Some(Target::Stair(id)) => cx.select_only(ObjectRef::Stair(id)),
        Some(Target::Symbol(id)) => cx.select_only(ObjectRef::Symbol(id)),
        Some(Target::Cabinet(id)) => cx.select_only(ObjectRef::Cabinet(id)),
        Some(Target::Roof(id)) => cx.select_only(ObjectRef::RoofPlane(id)),
        Some(Target::Detail(id)) => cx.select_only(ObjectRef::Detail(id)),
        None => {}
    }
}

// ----- the window -----

fn severity_color(s: Severity) -> Color32 {
    match s {
        Severity::Error => Color32::from_rgb(0xE0, 0x4B, 0x4B),
        Severity::Warning => Color32::from_rgb(0xE0, 0x8A, 0x1E),
        Severity::Info => Color32::from_rgb(0x6A, 0x9B, 0xD0),
    }
}

/// What the buttons of one frame asked for.
#[derive(Default)]
struct Clicks {
    prev: bool,
    next: bool,
    zoom: bool,
    ignore: bool,
    fix: bool,
    restore: bool,
    rerun: bool,
    settings: bool,
    save_md: bool,
    save_pdf: bool,
    to_layout: bool,
}

pub fn check_window(
    ctx: &egui::Context,
    cx: &mut EditorContext,
    cam: &mut Camera,
    w: &mut CheckWindow,
) -> bool {
    let mut open = true;
    let mut c = Clicks::default();
    egui::Window::new(w.kind.title())
        .id(egui::Id::new("plan_check_window"))
        .open(&mut open)
        .collapsible(false)
        .default_width(460.0)
        .default_pos(ctx.screen_rect().right_top() + egui::vec2(-500.0, 90.0))
        .show(ctx, |ui| {
            ui.label(RichText::new(w.position_text()).strong());
            ui.weak(w.summary());
            ui.separator();
            match w.current() {
                Some(f) => {
                    ui.horizontal(|ui| {
                        ui.colored_label(severity_color(f.severity), f.severity.label());
                        ui.label(RichText::new(f.rule).italics());
                    });
                    ui.add_space(4.0);
                    ui.label(&f.message);
                    if !f.fix.is_empty() {
                        ui.add_space(4.0);
                        ui.weak(format!("Fix: {}", f.fix));
                    }
                    ui.add_space(2.0);
                    ui.weak(f.where_text());
                }
                None => {
                    ui.label("The plan passes these checks.");
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                c.prev = ui
                    .add_enabled(w.can_previous(), egui::Button::new("Previous"))
                    .clicked();
                c.next = ui
                    .add_enabled(w.can_next(), egui::Button::new("Next"))
                    .clicked();
                c.zoom = ui
                    .add_enabled(w.current().is_some(), egui::Button::new("Zoom to"))
                    .clicked();
                c.ignore = ui
                    .add_enabled(w.current().is_some(), egui::Button::new("Ignore"))
                    .on_hover_text("Leave this finding out of the list; Restore brings it back")
                    .clicked();
                let fixable = w.current().is_some_and(crate::editor::code::can_fix);
                c.fix = ui
                    .add_enabled(fixable, egui::Button::new("Fix"))
                    .on_hover_text("Set the stair, railing or footing to the code minimum (one undo step)")
                    .clicked();
            });
            ui.checkbox(&mut w.zoom_each, "Zoom to each finding");
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                c.rerun = ui.button("Check Again").clicked();
                if w.kind == CheckKind::Plan {
                    c.settings = ui.button("Settings\u{2026}").clicked();
                }
                if w.ignored_count() > 0 {
                    c.restore = ui
                        .button(format!("Restore Ignored ({})", w.ignored_count()))
                        .clicked();
                }
                c.save_md = ui.button("Save Report\u{2026}").clicked();
                c.save_pdf = ui.button("Report PDF\u{2026}").clicked();
                c.to_layout = ui.button("Add to Layout").clicked();
            });
        });
    if let Some(d) = w.settings.as_mut() {
        match d.show(ctx) {
            SettingsResult::Open => {}
            SettingsResult::Cancel => w.settings = None,
            SettingsResult::Apply(s) => {
                w.settings = None;
                cx.begin_change("Plan Check Settings");
                s.store(&mut cx.project);
                cx.mark_dirty();
                c.rerun = true;
            }
            SettingsResult::ApplyToDefaults(s) => {
                w.settings = None;
                cx.begin_change("Apply Code Minimums to Defaults");
                s.store(&mut cx.project);
                cx.mark_dirty();
                cx.status = crate::editor::code::raise_defaults(cx);
                c.rerun = true;
            }
        }
    }
    handle(cx, cam, w, c);
    open
}

fn handle(cx: &mut EditorContext, cam: &mut Camera, w: &mut CheckWindow, c: Clicks) {
    let mut zoom = c.zoom;
    if c.prev {
        zoom |= w.previous() && w.zoom_each;
    }
    if c.next {
        zoom |= w.next() && w.zoom_each;
    }
    if c.fix {
        if let Some(f) = w.current().cloned() {
            match crate::editor::code::fix_finding(cx, &f) {
                Some(msg) => {
                    let run = run_check_full(cx, w.kind);
                    w.replace_run(run);
                    cx.status = msg;
                }
                None => cx.status = "Nothing to fix: the value already meets the code".into(),
            }
        }
    }
    if c.ignore && w.ignore_current(cx) {
        cx.status = w.summary();
        zoom |= w.zoom_each;
    }
    if c.restore {
        w.restore_ignored(cx);
        cx.status = w.summary();
    }
    if c.rerun {
        let run = run_check_full(cx, w.kind);
        w.replace_run(run);
        cx.status = w.summary();
    }
    if c.settings {
        w.settings = Some(SettingsDialog::new(CheckSettings::load(&cx.project)));
    }
    if std::mem::take(&mut w.pending_zoom) && w.zoom_each {
        zoom = true;
    }
    if zoom {
        if let Some(f) = w.current().cloned() {
            zoom_to_finding(cx, cam, &f);
        }
    }
    let floor_name = cx
        .project
        .floors
        .get(w.floor)
        .map_or_else(String::new, |f| f.name.clone());
    if c.save_md {
        let name = format!("{}.md", w.kind.title().replace('/', "-"));
        cx.status = save_bytes(&name, "md", w.report().as_bytes());
    }
    if c.save_pdf {
        let name = format!("{} Report.pdf", w.kind.title().replace('/', "-"));
        let bytes = report_pdf(&cx.project.name, &w.report_schedule(&floor_name));
        cx.status = save_bytes(&name, "pdf", &bytes);
    }
    if c.to_layout {
        cx.status = add_report_page(cx, &w.report_schedule(&floor_name));
    }
}

fn save_bytes(default_name: &str, ext: &str, bytes: &[u8]) -> String {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(default_name)
        .add_filter(ext, &[ext])
        .save_file()
    else {
        return "Export cancelled".into();
    };
    write_file(&path, bytes)
}

fn write_file(path: &Path, bytes: &[u8]) -> String {
    match std::fs::write(path, bytes) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save {}: {e}", path.display()),
    }
}

// ----- the report page -----

const PAGE_W: f64 = 792.0;
const PAGE_H: f64 = 612.0;
const MARGIN: f64 = 36.0;
const TEXT_PT: f64 = 8.0;
const LINE_PT: f64 = 10.0;
/// Column widths in points: No., Severity, Code reference, Where, Finding, Fix.
const COLS: [f64; 6] = [24.0, 46.0, 118.0, 100.0, 238.0, 194.0];

/// Wraps `text` into lines no wider than `width` points.
fn wrap(text: &str, width: f64, size: f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if line.is_empty() || PdfDoc::text_width(&candidate, size) <= width {
            line = candidate;
        } else {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// The report table as a PDF: letter, landscape, the header repeated on every
/// page.
pub fn report_pdf(project_name: &str, table: &Schedule) -> Vec<u8> {
    let mut doc = PdfDoc::new(PAGE_W, PAGE_H);
    let mut page = 1;
    let mut y = start_page(&mut doc, project_name, table, page);
    if table.rows.is_empty() {
        doc.text(MARGIN, y - LINE_PT, 10.0, "No findings.");
    }
    for row in &table.rows {
        let cells: Vec<Vec<String>> = row
            .iter()
            .zip(COLS)
            .map(|(cell, w)| wrap(cell, w - 6.0, TEXT_PT))
            .collect();
        let height = cells.iter().map(Vec::len).max().unwrap_or(1) as f64 * LINE_PT + 4.0;
        if y - height < MARGIN + LINE_PT {
            page += 1;
            doc.new_page();
            y = start_page(&mut doc, project_name, table, page);
        }
        let mut x = MARGIN;
        for (lines, w) in cells.iter().zip(COLS) {
            for (k, l) in lines.iter().enumerate() {
                doc.text(x + 3.0, y - LINE_PT * (k as f64 + 1.0), TEXT_PT, l);
            }
            x += w;
        }
        y -= height;
        doc.line(MARGIN, y, MARGIN + COLS.iter().sum::<f64>(), y, 0.25);
    }
    doc.finish()
}

/// Draws the page header and the table heading; returns the y below it.
fn start_page(doc: &mut PdfDoc, project_name: &str, table: &Schedule, page: usize) -> f64 {
    doc.set_font_bold(true);
    doc.text(MARGIN, PAGE_H - MARGIN - 8.0, 14.0, &table.title);
    doc.set_font_bold(false);
    doc.text(MARGIN, PAGE_H - MARGIN - 22.0, TEXT_PT, project_name);
    doc.text(
        PAGE_W - MARGIN - 40.0,
        MARGIN - 14.0,
        TEXT_PT,
        &format!("Page {page}"),
    );
    let top = PAGE_H - MARGIN - 36.0;
    let total = COLS.iter().sum::<f64>();
    doc.line(MARGIN, top, MARGIN + total, top, 1.0);
    doc.set_font_bold(true);
    let mut x = MARGIN;
    for (name, w) in table.columns.iter().zip(COLS) {
        doc.text(x + 3.0, top - LINE_PT, TEXT_PT, name);
        x += w;
    }
    doc.set_font_bold(false);
    doc.line(
        MARGIN,
        top - LINE_PT - 4.0,
        MARGIN + total,
        top - LINE_PT - 4.0,
        1.0,
    );
    top - LINE_PT - 4.0
}

/// Adds a "Plan Check" page with the report as a text box to the project's
/// layout. Returns the status text.
pub fn add_report_page(cx: &mut EditorContext, table: &Schedule) -> String {
    use plan_layout::{BoxSource, LayoutBox};
    let Some(mut layout) = crate::shell::layout_window::load(&cx.project) else {
        return "Make a layout first (File > New Layout), then add the Plan Check page".into();
    };
    let (lo, hi) = layout.drawing_area();
    let width_pt = (hi.x - lo.x) * 72.0 - 12.0;
    let chars = (width_pt / (TEXT_PT * 0.5)).max(20.0) as usize;
    let mut text = String::new();
    if table.rows.is_empty() {
        text.push_str("No findings.");
    }
    for row in &table.rows {
        let [no, sev, rule, at, msg, fix] = &row[..] else {
            continue;
        };
        let entry = format!("{no}. {sev}: {rule} ({at}). {msg} Fix: {fix}");
        let mut first = true;
        for l in wrap(&entry, chars as f64 * TEXT_PT * 0.5, TEXT_PT) {
            if !first {
                text.push_str("    ");
            }
            text.push_str(&l);
            text.push('\n');
            first = false;
        }
    }
    let number = layout
        .pages
        .iter()
        .map(|p| p.number)
        .max()
        .map_or(1, |m| m + 1);
    let box_id = layout
        .pages
        .iter()
        .flat_map(|p| p.boxes.iter().map(|b| b.id))
        .max()
        .map_or(1, |m| m + 1);
    layout
        .add_page(number, "Plan Check")
        .boxes
        .push(LayoutBox::new(
            box_id,
            (lo, hi),
            BoxSource::text(text, TEXT_PT),
            plan_docs::Scale::QuarterInch,
        ));
    cx.begin_change("Add Plan Check Page");
    crate::shell::layout_window::store(&mut cx.project, &layout);
    cx.mark_dirty();
    format!("Added page {number}, Plan Check, to the layout")
}

// ----- Tools > Checks > Plan Check Settings -----

/// Menu id: Tools > Checks > Plan Check Settings...
pub const SETTINGS: &str = "check.settings";
/// Menu id: Tools > Checks > Check While Drawing (a preference).
pub const CHECK_LIVE: &str = "check.live";
/// Menu id: Tools > Checks > Apply Code Minimums to Defaults.
pub const APPLY_DEFAULTS: &str = "check.apply_defaults";

/// Runs this module's menu commands by id; false when `id` is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        SETTINGS => {
            super::build_tools::open_check_settings(cx);
            true
        }
        CHECK_LIVE => {
            super::preferences::pages::update(|p| {
                p.architectural.check_while_drawing = !p.architectural.check_while_drawing;
            });
            let on = super::preferences::pages::current()
                .architectural
                .check_while_drawing;
            cx.status = format!("Check while drawing is {}", if on { "on" } else { "off" });
            true
        }
        APPLY_DEFAULTS => {
            cx.status = crate::editor::code::apply_to_defaults(cx);
            true
        }
        _ => false,
    }
}

impl CheckWindow {
    /// Opens the Plan Check Settings dialog over this window.
    pub fn open_settings(&mut self, cx: &EditorContext) {
        self.settings = Some(SettingsDialog::new(CheckSettings::load(&cx.project)));
    }

    /// Is the Plan Check Settings dialog showing?
    #[cfg(test)]
    pub fn settings_open(&self) -> bool {
        self.settings.is_some()
    }
}

// ----- a plain text report window (the Chief plan import report) -----

struct TextReport {
    title: String,
    headline: String,
    text: String,
}

thread_local! {
    static REPORT: std::cell::RefCell<Option<TextReport>> = const { std::cell::RefCell::new(None) };
}

/// Shows `text` (one line per fact) in a window titled `title`, with
/// `headline` above it. Used for the full File > Import > Chief Plan report;
/// the status bar only gets the headline.
pub fn open_text_report(title: &str, headline: &str, text: &str) {
    REPORT.with(|r| {
        *r.borrow_mut() = Some(TextReport {
            title: title.to_string(),
            headline: headline.to_string(),
            text: text.to_string(),
        });
    });
}

/// Is the text report window open?
#[cfg(test)]
pub fn text_report_open() -> bool {
    REPORT.with(|r| r.borrow().is_some())
}

/// Draws the text report window (when open); Copy puts the text on the
/// clipboard, Close (or the window's x) dismisses it.
pub fn show_text_report(ctx: &egui::Context) {
    let Some(report) = REPORT.with(|r| r.borrow_mut().take()) else {
        return;
    };
    let mut open = true;
    let mut close = false;
    egui::Window::new(&report.title)
        .id(egui::Id::new("text_report_window"))
        .open(&mut open)
        .collapsible(false)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.label(RichText::new(&report.headline).strong());
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(&report.text).monospace())
                            .wrap()
                            .selectable(true),
                    );
                });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Copy").clicked() {
                    ui.ctx().copy_text(report.text.clone());
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
    if open && !close {
        REPORT.with(|r| *r.borrow_mut() = Some(report));
    }
}

// ----- Plan Check Settings -----

type LimitField = fn(&mut CheckOptions) -> &mut f64;

/// The limits the dialog edits: label, field, unit.
const LIMITS: &[(&str, LimitField, &str)] = &[
    ("Habitable room area", |o| &mut o.min_room_area, " sq ft"),
    ("Habitable room dimension", |o| &mut o.min_room_dim, "\""),
    ("Ceiling height", |o| &mut o.min_ceiling, "\""),
    ("Bath ceiling height", |o| &mut o.min_ceiling_bath, "\""),
    ("Hallway width", |o| &mut o.min_hall_width, "\""),
    ("Egress opening area", |o| &mut o.egress_min_area, " sq ft"),
    ("Egress opening width", |o| &mut o.egress_min_w, "\""),
    ("Egress opening height", |o| &mut o.egress_min_h, "\""),
    ("Egress sill height (max)", |o| &mut o.egress_max_sill, "\""),
    ("Entry door clear width", |o| &mut o.min_door_width, "\""),
    ("Exterior door height", |o| &mut o.entry_door_height, "\""),
    ("Bath door width", |o| &mut o.bath_door, "\""),
    ("Landing outside a door", |o| &mut o.exit_landing, "\""),
    ("Stair width", |o| &mut o.stair_min_width, "\""),
    ("Riser height (max)", |o| &mut o.riser_max, "\""),
    ("Tread depth", |o| &mut o.tread_min, "\""),
    ("Stair headroom", |o| &mut o.headroom, "\""),
    ("Stair landing depth", |o| &mut o.landing_min, "\""),
    ("Guard required above", |o| &mut o.guard_drop, "\""),
    ("Stair guard height", |o| &mut o.stair_guard_height, "\""),
    (
        "Garage door to house",
        |o| &mut o.garage_house_door_min,
        "\"",
    ),
    ("Openable glazing share", |o| &mut o.vent_ratio, ""),
    ("Toilet side clearance", |o| &mut o.wc_side_clear, "\""),
    ("Toilet front clearance", |o| &mut o.wc_front_clear, "\""),
    ("Shower and tub side", |o| &mut o.shower_min_dim, "\""),
    ("Shower area", |o| &mut o.shower_min_area, " sq in"),
    ("Kitchen walkway", |o| &mut o.kitchen_walkway, "\""),
    ("Kitchen work aisle", |o| &mut o.kitchen_work_aisle, "\""),
    ("Counter depth", |o| &mut o.counter_depth_min, "\""),
    ("Roof pitch minimum", |o| &mut o.roof_pitch_min, ":12"),
    (
        "Roof pitch, single underlayment",
        |o| &mut o.roof_pitch_underlay,
        ":12",
    ),
    (
        "Roof pitch, steep advisory",
        |o| &mut o.roof_pitch_steep,
        ":12",
    ),
];

enum SettingsResult {
    Open,
    Cancel,
    Apply(Box<CheckSettings>),
    /// OK, and raise the plan defaults to these settings' code minimums.
    ApplyToDefaults(Box<CheckSettings>),
}

/// Tools > Checks > Plan Check Settings: the jurisdiction preset, the limits
/// and which rules run.
struct SettingsDialog {
    settings: CheckSettings,
}

impl SettingsDialog {
    fn new(settings: CheckSettings) -> Self {
        Self { settings }
    }

    fn show(&mut self, ctx: &egui::Context) -> SettingsResult {
        crate::editor::code::note_settings_open();
        let mut result = SettingsResult::Open;
        let mut open = true;
        egui::Window::new("Plan Check Settings")
            .id(egui::Id::new("plan_check_settings"))
            .open(&mut open)
            .collapsible(false)
            .default_width(480.0)
            .default_height(520.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Jurisdiction");
                    let mut name = self.settings.jurisdiction.clone();
                    egui::ComboBox::from_id_salt("plan_check_jurisdiction")
                        .selected_text(&name)
                        .show_ui(ui, |ui| {
                            for j in JURISDICTIONS {
                                ui.selectable_value(&mut name, j.to_string(), j);
                            }
                        });
                    if name != self.settings.jurisdiction {
                        if let Some(p) = CheckSettings::preset(&name) {
                            self.settings = p;
                        } else {
                            self.settings.jurisdiction = name;
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(380.0)
                    .show(ui, |ui| self.body(ui));
                ui.separator();
                ui.weak(format!(
                    "Code: {}",
                    plan_check::CodeMinimums::from(&self.settings).label()
                ));
                if ui
                    .button("Apply code minimums to defaults")
                    .on_hover_text(
                        "OK, and raise the stair, railing, bedroom window, exterior door, footing and garage wall defaults to these minimums",
                    )
                    .clicked()
                {
                    result = SettingsResult::ApplyToDefaults(Box::new(self.settings.clone()));
                }
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        result = SettingsResult::Apply(Box::new(self.settings.clone()));
                    }
                    if ui.button("Cancel").clicked() {
                        result = SettingsResult::Cancel;
                    }
                });
            });
        if !open {
            result = SettingsResult::Cancel;
        }
        result
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Limits")
            .default_open(false)
            .show(ui, |ui| {
                let before = self.settings.options.clone();
                egui::Grid::new("plan_check_limits").show(ui, |ui| {
                    for (label, field, unit) in LIMITS {
                        ui.label(*label);
                        let v = field(&mut self.settings.options);
                        ui.add(egui::DragValue::new(v).speed(0.1).suffix(*unit));
                        ui.end_row();
                    }
                });
                if self.settings.options != before {
                    self.settings.name_from_limits();
                }
            });
        let mut group = "";
        let mut enable_group: Option<(&str, bool)> = None;
        let catalog = plan_check::rule_catalog();
        for g in catalog.iter().map(|r| r.group) {
            if g == group {
                continue;
            }
            group = g;
            egui::CollapsingHeader::new(g)
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.small_button("All on").clicked() {
                            enable_group = Some((g, true));
                        }
                        if ui.small_button("All off").clicked() {
                            enable_group = Some((g, false));
                        }
                    });
                    for r in catalog.iter().filter(|r| r.group == g) {
                        let mut on = self.settings.is_enabled(r.id);
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut on, r.id).changed() {
                                self.settings.set_enabled(r.id, on);
                            }
                            ui.colored_label(severity_color(r.severity), r.severity.singular());
                        })
                        .response
                        .on_hover_text(r.summary);
                    }
                });
        }
        if let Some((g, on)) = enable_group {
            for r in catalog.iter().filter(|r| r.group == g) {
                self.settings.set_enabled(r.id, on);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::{OpeningKind, WallKind};

    fn house() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        let mut ids = Vec::new();
        for i in 0..4 {
            ids.push(
                cx.project
                    .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior),
            );
        }
        cx.project
            .add_opening(0, ids[0], 120.0, OpeningKind::Door)
            .unwrap();
        cx.project
            .add_opening(0, ids[2], 120.0, OpeningKind::Window)
            .unwrap();
        // A low ceiling and a bedroom with a small, high window give the
        // check several findings to walk through.
        cx.project.floors[0].ceiling_height = 76.0;
        cx.project.floors[0]
            .room_names
            .push(plan_core::RoomName::new(
                Point::new(120.0, 90.0),
                "Bedroom",
                "Bedroom",
            ));
        cx.mark_dirty();
        cx.refresh();
        cx
    }

    fn window(cx: &mut EditorContext) -> CheckWindow {
        let run = run_check_full(cx, CheckKind::Plan);
        CheckWindow::from_run(CheckKind::Plan, cx.floor, run)
    }

    #[test]
    fn ignoring_walks_on_and_survives_a_second_run() {
        let mut cx = house();
        let mut w = window(&mut cx);
        let total = w.count();
        assert!(total >= 2, "{total}");
        let first = w.current().unwrap().clone();
        assert!(w.ignore_current(&mut cx));
        assert_eq!(w.count(), total - 1);
        assert_eq!(w.ignored_count(), 1);
        assert_ne!(w.current().unwrap(), &first);
        assert!(w.summary().ends_with("(1 ignored)"), "{}", w.summary());
        // A fresh run of the same plan leaves it out; the plan remembers it.
        let again = run_check_full(&mut cx, CheckKind::Plan);
        assert_eq!(again.findings.len(), total - 1);
        assert_eq!(again.ignored, vec![first]);
        // It is one undo step, and undoing brings the finding back.
        assert_eq!(cx.undo().as_deref(), Some("Ignore Plan Check Finding"));
        assert_eq!(
            run_check_full(&mut cx, CheckKind::Plan).findings.len(),
            total
        );
    }

    #[test]
    fn restore_brings_ignored_findings_back() {
        let mut cx = house();
        let mut w = window(&mut cx);
        let total = w.count();
        w.ignore_current(&mut cx);
        w.ignore_current(&mut cx);
        assert_eq!(w.count(), total - 2);
        w.restore_ignored(&mut cx);
        assert_eq!(w.count(), total);
        assert_eq!(w.ignored_count(), 0);
    }

    #[test]
    fn ignoring_the_last_finding_leaves_an_empty_window() {
        let mut cx = house();
        let mut w = window(&mut cx);
        while w.count() > 0 {
            assert!(w.ignore_current(&mut cx));
        }
        assert_eq!(w.position_text(), "No findings");
        assert!(!w.ignore_current(&mut cx));
        assert!(w.summary().starts_with("Plan Check: no findings"));
    }

    #[test]
    fn settings_switch_rules_off_for_the_next_run() {
        let mut cx = house();
        let before = run_check(&mut cx, CheckKind::Plan);
        let rule = before[0].rule;
        let mut s = CheckSettings::load(&cx.project);
        s.set_enabled(rule, false);
        s.store(&mut cx.project);
        cx.mark_dirty();
        let after = run_check(&mut cx, CheckKind::Plan);
        assert!(after.iter().all(|f| f.rule != rule));
        assert!(after.len() < before.len());
    }

    #[test]
    fn zoom_selects_every_kind_of_object() {
        let mut cx = house();
        let mut cam = Camera::default_view();
        let mut f = Finding {
            rule: "IRC test",
            severity: Severity::Info,
            message: String::new(),
            location: Some(Point::new(7.0, 9.0)),
            object: Some(Target::Stair(4)),
            fix: String::new(),
        };
        zoom_to_finding(&mut cx, &mut cam, &f);
        assert_eq!(cam.center, Point::new(7.0, 9.0));
        assert_eq!(cx.selection.single(), Some(ObjectRef::Stair(4)));
        f.object = Some(Target::Roof(8));
        zoom_to_finding(&mut cx, &mut cam, &f);
        assert_eq!(cx.selection.single(), Some(ObjectRef::RoofPlane(8)));
        f.object = Some(Target::Cabinet(2));
        zoom_to_finding(&mut cx, &mut cam, &f);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Cabinet(2)));
    }

    #[test]
    fn report_is_a_pdf_and_a_schedule() {
        let mut cx = house();
        let w = window(&mut cx);
        let s = w.report_schedule("1st Floor");
        assert_eq!(s.rows.len(), w.count());
        assert_eq!(s.columns.len(), 6);
        let pdf = report_pdf("House", &s);
        assert!(pdf.starts_with(b"%PDF"));
        // Many findings run onto more pages.
        let long = Schedule {
            title: "Plan Check".into(),
            columns: s.columns.clone(),
            rows: (0..200)
                .map(|i| {
                    vec![
                        i.to_string(),
                        "Error".into(),
                        "IRC".into(),
                        "Wall 1".into(),
                        "m ".repeat(60),
                        "f".into(),
                    ]
                })
                .collect(),
        };
        let pages = report_pdf("House", &long)
            .windows(11)
            .filter(|w| *w == b"/Type /Page")
            .count();
        assert!(pages > 2, "{pages}");
        let empty = report_pdf(
            "House",
            &Schedule {
                rows: Vec::new(),
                ..s
            },
        );
        assert!(empty.starts_with(b"%PDF"));
    }

    #[test]
    fn the_report_page_goes_into_the_layout() {
        let mut cx = house();
        let w = window(&mut cx);
        let table = w.report_schedule("1st Floor");
        // No layout yet.
        assert!(add_report_page(&mut cx, &table).starts_with("Make a layout"));
        crate::shell::layout_window::new_layout(&mut cx);
        let before = crate::shell::layout_window::load(&cx.project)
            .unwrap()
            .pages
            .len();
        let status = add_report_page(&mut cx, &table);
        assert!(status.contains("Plan Check"), "{status}");
        let layout = crate::shell::layout_window::load(&cx.project).unwrap();
        assert_eq!(layout.pages.len(), before + 1);
        let page = layout.pages.last().unwrap();
        assert_eq!(page.title, "Plan Check");
        assert_eq!(page.boxes.len(), 1);
    }

    #[test]
    fn wrap_breaks_long_text() {
        let lines = wrap(&"word ".repeat(40), 100.0, TEXT_PT);
        assert!(lines.len() > 3);
        assert!(lines
            .iter()
            .all(|l| PdfDoc::text_width(l, TEXT_PT) <= 100.0 + 1.0));
        assert_eq!(wrap("", 100.0, TEXT_PT), vec![String::new()]);
    }
}
