//! The Build and Tools windows: floor dialogs, the Room Specification host,
//! the Space Planning Assistant, Plan Check and Door/Window Check, Materials
//! List, Schedules and Create Construction Set.
//!
//! The shell calls [`dispatch`] for the menu actions and [`show_all`] once a
//! frame. The windows live in a thread-local [`Windows`] (the UI is single
//! threaded), so the shell needs no fields for them. Everything that changes
//! the plan goes through `cx.begin_change`, so each command is one undo step.

use super::floor::FloorDialog;
use super::room::RoomDialog;
use super::Outcome;
use crate::editor::rooms_edit::{self, FoundationSpec};
use crate::editor::{Camera, EditorContext, ObjectRef};
use crate::toolbar::Action;
use eframe::egui::{self, Color32, RichText};
use plan_check::{CheckOptions, Finding, Severity, Target};
use plan_core::geometry::Point;
use plan_docs::{
    door_schedule, materials_list, materials_to_csv, room_schedule, wall_schedule, window_schedule,
    MaterialLine, Schedule,
};
use plan_layout::{default_construction_set, render_pdf, LayoutRenderContext};
use plan_spaceplan::{
    build_house, generate_boxes, validate, BuildOptions, BuildReport, Questionnaire, RoomBox,
};
use plan_stairs::Stair;
use std::cell::RefCell;
use std::path::Path;

// ----- Plan Check state machine (like Chief's one-finding-at-a-time dialog) -----

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
    findings: Vec<Finding>,
    index: usize,
}

impl CheckWindow {
    pub fn new(kind: CheckKind, findings: Vec<Finding>) -> Self {
        Self {
            kind,
            findings,
            index: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.findings.len()
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

    pub fn report(&self) -> String {
        plan_check::report_markdown(&self.findings)
    }
}

/// Runs Plan Check or Door/Window Check on the active floor.
pub fn run_check(cx: &mut EditorContext, kind: CheckKind) -> Vec<Finding> {
    cx.refresh();
    match kind {
        CheckKind::DoorWindow => plan_check::door_window_check(&cx.project, cx.floor),
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
            plan_check::plan_check(
                &cx.project,
                cx.floor,
                &cx.rooms,
                &room_types,
                &stairs,
                &CheckOptions::default(),
            )
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
        _ => {}
    }
}

// ----- Space Planning -----

/// Boxes for a questionnaire (the Generate button).
pub fn generate(q: &Questionnaire) -> Vec<RoomBox> {
    generate_boxes(q)
}

/// Build House: turns the boxes into walls, doors, windows and room names as
/// one undo step. Returns `None` when there are no boxes.
pub fn build_house_from_boxes(cx: &mut EditorContext, boxes: &[RoomBox]) -> Option<BuildReport> {
    if boxes.is_empty() {
        return None;
    }
    let opts = BuildOptions {
        exterior_thickness: cx.defaults.exterior_thickness(),
        interior_thickness: cx.defaults.interior_thickness(),
        wall_height: cx.defaults.walls_for(plan_core::WallKind::Exterior).height,
        ..BuildOptions::default()
    };
    cx.begin_change("Build House");
    let report = build_house(&mut cx.project, boxes, &opts);
    cx.mark_dirty();
    cx.refresh();
    Some(report)
}

struct SpaceWindow {
    q: Questionnaire,
    message: String,
}

impl SpaceWindow {
    fn new() -> Self {
        Self {
            q: Questionnaire::default(),
            message: "Answer the questions, then Generate. Drag the boxes on the plan.".into(),
        }
    }
}

fn fit_camera(cam: &mut Camera, boxes: &[RoomBox]) {
    if boxes.is_empty() {
        return;
    }
    let (mut lo, mut hi) = (boxes[0].rect.0, boxes[0].rect.1);
    for b in boxes {
        lo = Point::new(lo.x.min(b.rect.0.x), lo.y.min(b.rect.0.y));
        hi = Point::new(hi.x.max(b.rect.1.x), hi.y.max(b.rect.1.y));
    }
    cam.center = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
    let (w, h) = (
        ((hi.x - lo.x) * 1.4).max(1.0),
        ((hi.y - lo.y) * 1.4).max(1.0),
    );
    let scale = (cam.rect.width() as f64 / w).min(cam.rect.height() as f64 / h);
    if scale.is_finite() {
        cam.px_per_in = scale.clamp(0.05, 50.0);
    }
}

fn space_window(
    ctx: &egui::Context,
    cx: &mut EditorContext,
    cam: &mut Camera,
    w: &mut SpaceWindow,
) -> bool {
    let mut open = true;
    let mut generate_clicked = false;
    let mut build_clicked = false;
    let mut clear_clicked = false;
    egui::Window::new("Space Planning Assistant")
        .id(egui::Id::new("space_planning"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_pos(ctx.screen_rect().right_top() + egui::vec2(-340.0, 90.0))
        .show(ctx, |ui| {
            let q = &mut w.q;
            egui::Grid::new("space_q").num_columns(2).show(ui, |ui| {
                ui.label("Bedrooms");
                ui.add(egui::DragValue::new(&mut q.bedrooms).range(1..=8));
                ui.end_row();
                ui.label("Baths");
                ui.add(
                    egui::DragValue::new(&mut q.baths)
                        .speed(0.5)
                        .range(0.5..=8.0)
                        .fixed_decimals(1),
                );
                ui.end_row();
                ui.label("Garage bays");
                ui.add(egui::DragValue::new(&mut q.garage_bays).range(0..=4));
                ui.end_row();
                ui.label("Stories");
                ui.add(egui::DragValue::new(&mut q.stories).range(1..=3));
                ui.end_row();
                for (label, v) in [
                    ("Living room (sq ft)", &mut q.living_sq_ft),
                    ("Kitchen (sq ft)", &mut q.kitchen_sq_ft),
                    ("Dining (sq ft)", &mut q.dining_sq_ft),
                    ("Master bedroom (sq ft)", &mut q.master_sq_ft),
                    ("Other bedrooms (sq ft)", &mut q.bedroom_sq_ft),
                ] {
                    ui.label(label);
                    ui.add(egui::DragValue::new(v).speed(4.0).range(40.0..=2000.0));
                    ui.end_row();
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut q.office, "Office");
                ui.checkbox(&mut q.laundry, "Laundry");
                ui.checkbox(&mut q.mudroom, "Mudroom");
                ui.checkbox(&mut q.pantry, "Pantry");
                ui.checkbox(&mut q.covered_porch, "Covered porch");
                ui.checkbox(&mut q.deck, "Deck");
            });
            ui.label(format!("About {:.0} sq ft", q.expected_area_sq_ft()));
            ui.separator();
            ui.horizontal(|ui| {
                generate_clicked = ui.button("Generate").clicked();
                clear_clicked = ui.button("Clear Boxes").clicked();
                build_clicked = ui
                    .add_enabled(
                        !rooms_edit::space_boxes().is_empty(),
                        egui::Button::new(RichText::new("Build House").strong()),
                    )
                    .on_hover_text("Turn the boxes into walls, doors and windows")
                    .clicked();
            });
            ui.weak(w.message.clone());
            let boxes = rooms_edit::space_boxes();
            for issue in validate(&boxes).iter().take(6) {
                ui.colored_label(Color32::from_rgb(0xE0, 0x8A, 0x1E), &issue.message);
            }
        });
    if generate_clicked {
        let boxes = generate(&w.q);
        fit_camera(cam, &boxes);
        w.message = format!("{} boxes. Drag them to rearrange.", boxes.len());
        rooms_edit::set_space_boxes(boxes);
    }
    if clear_clicked {
        rooms_edit::clear_space_boxes();
        w.message = "Boxes cleared.".into();
    }
    if build_clicked {
        let boxes = rooms_edit::space_boxes();
        if let Some(r) = build_house_from_boxes(cx, &boxes) {
            w.message = format!(
                "Built {} walls, {} doors, {} windows.",
                r.walls.len(),
                r.doors.len(),
                r.windows.len()
            );
            cx.status = format!("Built house: {}", w.message);
            rooms_edit::clear_space_boxes();
        }
    }
    open
}

// ----- Plan Check window -----

fn severity_color(s: Severity) -> Color32 {
    match s {
        Severity::Error => Color32::from_rgb(0xE0, 0x4B, 0x4B),
        Severity::Warning => Color32::from_rgb(0xE0, 0x8A, 0x1E),
        Severity::Info => Color32::from_rgb(0x6A, 0x9B, 0xD0),
    }
}

fn check_window(
    ctx: &egui::Context,
    cx: &mut EditorContext,
    cam: &mut Camera,
    w: &mut CheckWindow,
) -> bool {
    let mut open = true;
    let (mut prev, mut next, mut zoom, mut rerun, mut save) = (false, false, false, false, false);
    egui::Window::new(w.kind.title())
        .id(egui::Id::new("plan_check_window"))
        .open(&mut open)
        .collapsible(false)
        .default_width(420.0)
        .default_pos(ctx.screen_rect().right_top() + egui::vec2(-460.0, 90.0))
        .show(ctx, |ui| {
            ui.label(RichText::new(w.position_text()).strong());
            ui.separator();
            match w.current() {
                Some(f) => {
                    ui.colored_label(severity_color(f.severity), f.severity.label());
                    ui.label(RichText::new(f.rule).italics());
                    ui.add_space(4.0);
                    ui.label(&f.message);
                    if !f.fix.is_empty() {
                        ui.add_space(4.0);
                        ui.weak(format!("Fix: {}", f.fix));
                    }
                }
                None => {
                    ui.label("The plan passes these checks.");
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                prev = ui
                    .add_enabled(w.can_previous(), egui::Button::new("Previous"))
                    .clicked();
                next = ui
                    .add_enabled(w.can_next(), egui::Button::new("Next"))
                    .clicked();
                zoom = ui
                    .add_enabled(
                        w.current().is_some_and(|f| f.location.is_some()),
                        egui::Button::new("Zoom to"),
                    )
                    .clicked();
                rerun = ui.button("Check Again").clicked();
                save = ui.button("Save Report\u{2026}").clicked();
            });
        });
    if prev {
        w.previous();
    }
    if next {
        w.next();
    }
    if zoom {
        if let Some(f) = w.current().cloned() {
            zoom_to_finding(cx, cam, &f);
        }
    }
    if rerun {
        let findings = run_check(cx, w.kind);
        w.replace(findings);
    }
    if save {
        let name = format!("{}.md", w.kind.title().replace('/', "-"));
        cx.status = save_text(&name, "md", &w.report());
    }
    open
}

// ----- Schedules and Materials List -----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SchedKind {
    Door,
    Window,
    Room,
    Wall,
}

/// The live schedule of the active floor.
pub fn schedule_for(cx: &EditorContext, kind: SchedKind) -> Schedule {
    match kind {
        SchedKind::Door => door_schedule(&cx.project, cx.floor),
        SchedKind::Window => window_schedule(&cx.project, cx.floor),
        SchedKind::Room => room_schedule(&cx.project, cx.floor, &cx.rooms),
        SchedKind::Wall => wall_schedule(&cx.project, cx.floor),
    }
}

/// The materials list of the active floor.
pub fn materials_for(cx: &EditorContext) -> Vec<MaterialLine> {
    materials_list(&cx.project, cx.floor, &cx.rooms)
}

pub fn materials_csv(cx: &EditorContext) -> String {
    materials_to_csv(&materials_for(cx))
}

/// Writes `text` where the user picks. Returns the status message.
fn save_text(default_name: &str, ext: &str, text: &str) -> String {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(default_name)
        .add_filter(ext, &[ext])
        .save_file()
    else {
        return "Export cancelled".into();
    };
    write_file(&path, text.as_bytes())
}

fn write_file(path: &Path, bytes: &[u8]) -> String {
    match std::fs::write(path, bytes) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save {}: {e}", path.display()),
    }
}

fn table(ui: &mut egui::Ui, id: &str, columns: &[String], rows: &[Vec<String>]) {
    egui::ScrollArea::both()
        .max_height(320.0)
        .auto_shrink([true, true])
        .show(ui, |ui| {
            egui::Grid::new(id).striped(true).show(ui, |ui| {
                for c in columns {
                    ui.strong(c);
                }
                ui.end_row();
                for r in rows {
                    for c in r {
                        ui.label(c);
                    }
                    ui.end_row();
                }
            });
        });
}

fn schedule_window(ctx: &egui::Context, cx: &mut EditorContext, kind: SchedKind) -> bool {
    let sched = schedule_for(cx, kind);
    let mut open = true;
    let mut export = false;
    egui::Window::new(sched.title.clone())
        .id(egui::Id::new(("schedule", kind as u8)))
        .open(&mut open)
        .default_pos(
            ctx.screen_rect().center()
                + egui::vec2(30.0 * kind as u8 as f32, 20.0 * kind as u8 as f32),
        )
        .show(ctx, |ui| {
            table(
                ui,
                &format!("sched_{}", kind as u8),
                &sched.columns,
                &sched.rows,
            );
            ui.separator();
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new("Place on Plan"))
                    .on_hover_text("Placing a schedule on the plan comes with Layout");
                export = ui.button("Export CSV\u{2026}").clicked();
            });
        });
    if export {
        let name = format!("{}.csv", sched.title.replace(' ', "_"));
        cx.status = save_text(&name, "csv", &sched.to_csv());
    }
    open
}

fn materials_window(ctx: &egui::Context, cx: &mut EditorContext) -> bool {
    let lines = materials_for(cx);
    let mut open = true;
    let mut export = false;
    egui::Window::new("Materials List")
        .id(egui::Id::new("materials_list"))
        .open(&mut open)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            let columns = ["Category", "Item", "Quantity", "Unit"].map(String::from);
            let rows: Vec<Vec<String>> = lines
                .iter()
                .map(|l| {
                    vec![
                        l.category.clone(),
                        l.item.clone(),
                        format!("{:.1}", l.quantity),
                        l.unit.clone(),
                    ]
                })
                .collect();
            table(ui, "materials_table", &columns, &rows);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(format!("{} lines for {}", lines.len(), cx.floor().name));
                export = ui.button("Export CSV\u{2026}").clicked();
            });
        });
    if export {
        cx.status = save_text("materials_list.csv", "csv", &materials_csv(cx));
    }
    open
}

// ----- Create Construction Set -----

/// The default construction set as PDF bytes.
pub fn construction_set_pdf(project: &plan_core::Project) -> Vec<u8> {
    let layout = default_construction_set(project, project.floors.len());
    let rcx = LayoutRenderContext::new(project);
    render_pdf(&layout, &rcx)
}

/// File > New Layout: Daniel's layout template (ARCH C 18x24, his title
/// block) with every floor sent in at the largest Chief scale that fits, one
/// floor per page, saved as a PDF. Plan Studio has no layout window yet, so
/// the new layout goes straight to paper.
pub fn new_layout_pdf(project: &plan_core::Project) -> Vec<u8> {
    let layout = new_layout_document(project);
    let rcx = LayoutRenderContext::new(project);
    render_pdf(&layout, &rcx)
}

/// The layout `new_layout_pdf` prints.
pub fn new_layout_document(project: &plan_core::Project) -> plan_layout::Layout {
    let settings = crate::templates::load_settings();
    let seed = crate::templates::refresh(&settings, false);
    let mut layout = crate::templates::new_layout(
        &format!("{} Layout", project.name),
        seed.cache.layout.as_ref(),
    );
    let rcx = LayoutRenderContext::new(project);
    for floor in 0..project.floors.len() {
        let page = u32::try_from(floor).unwrap_or(0);
        if layout.page(page).is_none() {
            let title = plan_layout::plan_label(project, floor);
            layout.add_page(page, title);
        }
        let source = plan_layout::BoxSource::PlanView {
            floor,
            layer_set: project.layer_sets.active.clone(),
        };
        plan_layout::send_to_layout_auto(&mut layout, &rcx, page, source, None, None);
    }
    layout
}

fn new_layout_from_template(cx: &mut EditorContext) {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{} Layout.pdf", cx.project.name))
        .add_filter("pdf", &["pdf"])
        .save_file()
    else {
        cx.status = "New Layout cancelled".into();
        return;
    };
    let bytes = new_layout_pdf(&cx.project);
    cx.status = write_file(&path, &bytes);
}

fn create_construction_set(cx: &mut EditorContext) {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{} Construction Set.pdf", cx.project.name))
        .add_filter("pdf", &["pdf"])
        .save_file()
    else {
        cx.status = "Create Construction Set cancelled".into();
        return;
    };
    let bytes = construction_set_pdf(&cx.project);
    cx.status = write_file(&path, &bytes);
}

// ----- the window host -----

#[derive(Default)]
struct Windows {
    room: Option<RoomDialog>,
    floor: Option<FloorDialog>,
    space: Option<SpaceWindow>,
    check: Option<CheckWindow>,
    materials: bool,
    schedules: Vec<SchedKind>,
}

thread_local! {
    static WINDOWS: RefCell<Windows> = RefCell::new(Windows::default());
}

fn with_windows<R>(f: impl FnOnce(&mut Windows) -> R) -> R {
    WINDOWS.with(|w| f(&mut w.borrow_mut()))
}

/// Handles the menu actions of this module (Build > Floor, Tools > Space
/// Planning, Checks, Schedules, Materials List, Construction Set).
pub fn dispatch(cx: &mut EditorContext, action: Action) {
    cx.refresh();
    match action {
        Action::BuildNewFloor => {
            let d = FloorDialog::new_floor(&cx.project);
            with_windows(|w| w.floor = Some(d));
        }
        Action::InsertFloor => {
            rooms_edit::insert_floor(cx);
        }
        Action::DeleteFloor => {
            if cx.project.floors.len() < 2 {
                cx.status = "Cannot delete the only floor".into();
            } else {
                let d = FloorDialog::confirm_delete(&cx.floor().name);
                with_windows(|w| w.floor = Some(d));
            }
        }
        Action::DeleteFoundation => {
            rooms_edit::delete_foundation(cx);
        }
        Action::ExchangeFloorAbove => {
            rooms_edit::exchange_floor(cx, true);
        }
        Action::ExchangeFloorBelow => {
            rooms_edit::exchange_floor(cx, false);
        }
        Action::BuildFoundation => {
            let d = FloorDialog::foundation(FoundationSpec::from_defaults(&cx.defaults));
            with_windows(|w| w.floor = Some(d));
        }
        Action::RebuildAll => rooms_edit::rebuild_all(cx),
        Action::SpacePlanning => with_windows(|w| {
            if w.space.is_none() {
                w.space = Some(SpaceWindow::new());
            }
        }),
        Action::PlanCheck | Action::DoorWindowCheck => {
            let kind = if action == Action::PlanCheck {
                CheckKind::Plan
            } else {
                CheckKind::DoorWindow
            };
            let findings = run_check(cx, kind);
            cx.status = format!("{}: {} findings", kind.title(), findings.len());
            with_windows(|w| w.check = Some(CheckWindow::new(kind, findings)));
        }
        Action::PlanFootprint => {
            rooms_edit::add_plan_footprint(cx);
        }
        Action::MaterialsList => with_windows(|w| w.materials = true),
        Action::DoorSchedule => open_schedule(SchedKind::Door),
        Action::WindowSchedule => open_schedule(SchedKind::Window),
        Action::RoomSchedule => open_schedule(SchedKind::Room),
        Action::WallSchedule => open_schedule(SchedKind::Wall),
        Action::CreateConstructionSet => create_construction_set(cx),
        Action::FileNewLayout => new_layout_from_template(cx),
        _ => {}
    }
}

fn open_schedule(kind: SchedKind) {
    with_windows(|w| {
        if !w.schedules.contains(&kind) {
            w.schedules.push(kind);
        }
    });
}

/// Draws every open window of this module and applies what the user accepted.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext, cam: &mut Camera) {
    let mut w = with_windows(std::mem::take);
    w.show(ctx, cx, cam);
    with_windows(|slot| *slot = w);
}

impl Windows {
    fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext, cam: &mut Camera) {
        // Room Specification.
        if self.room.is_none() && self.floor.is_none() {
            if let Some(idx) = rooms_edit::take_room_dialog_request(cx) {
                self.room = rooms_edit::room_dialog_init(cx, idx).map(RoomDialog::new);
            }
        }
        if let Some(mut d) = self.room.take() {
            match d.show(ctx) {
                Outcome::Open => self.room = Some(d),
                outcome => {
                    if outcome == Outcome::Ok {
                        rooms_edit::apply_room_spec(cx, d.room_index(), d.room_name(), d.extras());
                    }
                    // The Enter that closed the dialog also reached the Select
                    // tool, which asked for it again.
                    let _ = rooms_edit::take_room_dialog_request(cx);
                }
            }
        }
        // Floor dialogs.
        if let Some(mut d) = self.floor.take() {
            match d.show(ctx) {
                Outcome::Open => self.floor = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => match d {
                    FloorDialog::NewFloor { derive, .. } => {
                        rooms_edit::build_new_floor(cx, derive);
                    }
                    FloorDialog::BuildFoundation { spec, .. } => {
                        rooms_edit::build_foundation(cx, spec);
                    }
                    FloorDialog::ConfirmDelete { .. } => {
                        rooms_edit::delete_floor(cx);
                    }
                },
            }
        }
        if let Some(mut s) = self.space.take() {
            if space_window(ctx, cx, cam, &mut s) {
                self.space = Some(s);
            } else {
                rooms_edit::clear_space_boxes();
            }
        }
        if let Some(mut c) = self.check.take() {
            if check_window(ctx, cx, cam, &mut c) {
                self.check = Some(c);
            }
        }
        if self.materials {
            self.materials = materials_window(ctx, cx);
        }
        let kinds = std::mem::take(&mut self.schedules);
        self.schedules = kinds
            .into_iter()
            .filter(|k| schedule_window(ctx, cx, *k))
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
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
        cx.mark_dirty();
        cx.refresh();
        cx
    }

    fn finding(n: usize) -> Finding {
        Finding {
            rule: "IRC test",
            severity: Severity::Warning,
            message: format!("finding {n}"),
            location: Some(Point::new(n as f64 * 10.0, 5.0)),
            object: None,
            fix: "fix it".into(),
        }
    }

    #[test]
    fn check_window_steps_through_findings() {
        let mut w = CheckWindow::new(CheckKind::Plan, (0..3).map(finding).collect());
        assert_eq!(w.position_text(), "Finding 1 of 3");
        assert!(!w.can_previous());
        assert!(w.next());
        assert!(w.next());
        assert!(!w.next());
        assert_eq!(w.position_text(), "Finding 3 of 3");
        assert_eq!(w.current().unwrap().message, "finding 2");
        assert!(w.previous());
        assert_eq!(w.index(), 1);
        w.replace(vec![finding(9)]);
        assert_eq!(w.index(), 0);
        assert_eq!(w.count(), 1);
        w.replace(Vec::new());
        assert_eq!(w.position_text(), "No findings");
        assert!(w.current().is_none());
    }

    #[test]
    fn zoom_to_centers_the_camera_and_selects() {
        let mut cx = house();
        let mut cam = Camera::default_view();
        let wall = cx.floor().walls[0].id;
        let mut f = finding(4);
        f.object = Some(Target::Wall(wall));
        zoom_to_finding(&mut cx, &mut cam, &f);
        assert_eq!(cam.center, Point::new(40.0, 5.0));
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(wall)));
    }

    #[test]
    fn plan_check_runs_on_the_active_floor() {
        let mut cx = house();
        let findings = run_check(&mut cx, CheckKind::Plan);
        let w = CheckWindow::new(CheckKind::Plan, findings);
        assert_eq!(w.count(), w.findings.len());
        assert!(w.report().starts_with("# Plan Check Report"));
        let dw = run_check(&mut cx, CheckKind::DoorWindow);
        assert!(dw.len() <= w.count() + 50);
    }

    #[test]
    fn space_planning_generates_and_builds_with_undo() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let boxes = generate(&Questionnaire::default());
        assert!(boxes.len() >= 6, "{}", boxes.len());
        assert!(cx.floor().walls.is_empty());
        let report = build_house_from_boxes(&mut cx, &boxes).unwrap();
        assert!(!report.walls.is_empty());
        assert!(!cx.project.floors[0].walls.is_empty());
        assert!(!cx.project.floors[0].room_names.is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Build House"));
        assert!(cx.project.floors[0].walls.is_empty());
        assert!(build_house_from_boxes(&mut cx, &[]).is_none());
    }

    #[test]
    fn schedules_and_materials_export_csv() {
        let cx = house();
        for kind in [
            SchedKind::Door,
            SchedKind::Window,
            SchedKind::Room,
            SchedKind::Wall,
        ] {
            let csv = schedule_for(&cx, kind).to_csv();
            assert!(csv.lines().count() >= 2, "{kind:?}: {csv}");
        }
        assert!(materials_csv(&cx).lines().count() >= 2);
    }

    #[test]
    fn dispatch_opens_and_runs_commands() {
        let mut cx = house();
        dispatch(&mut cx, Action::PlanFootprint);
        assert_eq!(cx.floor().cad.len(), 2);
        dispatch(&mut cx, Action::InsertFloor);
        assert_eq!(cx.project.floors.len(), 2);
        dispatch(&mut cx, Action::PlanCheck);
        assert!(with_windows(|w| w.check.is_some()));
        dispatch(&mut cx, Action::DoorSchedule);
        dispatch(&mut cx, Action::DoorSchedule);
        assert_eq!(with_windows(|w| w.schedules.len()), 1);
        dispatch(&mut cx, Action::DeleteFloor);
        assert!(with_windows(|w| w.floor.is_some()));
    }

    #[test]
    fn construction_set_is_a_pdf() {
        let cx = house();
        let pdf = construction_set_pdf(&cx.project);
        assert!(pdf.starts_with(b"%PDF"));
        assert!(pdf.len() > 1000);
    }
}
