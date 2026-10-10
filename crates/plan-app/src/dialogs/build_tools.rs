//! The Build and Tools windows: floor dialogs, the Room Specification host,
//! the Space Planning Assistant, Plan Check and Door/Window Check, Materials
//! List, Schedules and Create Construction Set.
//!
//! The shell calls [`dispatch`] for the menu actions and [`show_all`] once a
//! frame. The windows live in a thread-local [`Windows`] (the UI is single
//! threaded), so the shell needs no fields for them. Everything that changes
//! the plan goes through `cx.begin_change`, so each command is one undo step.

use super::find_replace::FindReplaceDialog;
use super::floor::FloorDialog;
use super::project_info::{self, ProjectInfoDialog};
use super::room::RoomDialog;
use super::schedule_spec::ScheduleSpecDialog;
use super::Outcome;
use crate::editor::rooms_edit;
use crate::editor::schedule_view;
use crate::editor::{Camera, EditorContext, ObjectRef};
use crate::toolbar::Action;
use eframe::egui::{self, Color32, RichText};
#[cfg(test)]
use plan_check::{Finding, Severity, Target};
use plan_core::geometry::Point;
use plan_docs::{
    door_schedule, room_schedule, wall_schedule, window_schedule, MaterialLine, Schedule,
};
use plan_layout::render_pdf;
use plan_spaceplan::{
    build_house, generate_boxes, validate, BuildOptions, BuildReport, Questionnaire, RoomBox,
};
use std::cell::RefCell;
use std::path::Path;

// ----- Plan Check (the window, settings and report are in `plan_check.rs`) -----

pub use super::plan_check::{check_window, run_check_full, CheckKind, CheckWindow};
#[cfg(test)]
use super::plan_check::{run_check, zoom_to_finding};

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

// ----- Schedules and Materials List -----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SchedKind {
    Door,
    Window,
    Room,
    Wall,
}

impl SchedKind {
    /// The placeable schedule kind this window lists.
    pub fn plan_kind(self) -> plan_core::schedules::ScheduleKind {
        use plan_core::schedules::ScheduleKind as K;
        match self {
            SchedKind::Door => K::Door,
            SchedKind::Window => K::Window,
            SchedKind::Room => K::Room,
            SchedKind::Wall => K::Wall,
        }
    }
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

/// The materials list of the active floor, with the Master List's waste and
/// prices (`dialogs::materials`).
#[cfg_attr(not(test), allow(dead_code))]
pub fn materials_for(cx: &EditorContext) -> Vec<MaterialLine> {
    super::materials::lines_for_floor(cx)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn materials_csv(cx: &EditorContext) -> String {
    super::materials::csv_for_floor(cx)
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

/// Asks for a file name and writes `bytes` there (an Excel workbook).
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

/// The schedule table; when `clickable`, returns the row that was clicked.
fn clickable_table(
    ui: &mut egui::Ui,
    id: &str,
    columns: &[String],
    rows: &[Vec<String>],
    clickable: bool,
) -> Option<usize> {
    let mut clicked = None;
    egui::ScrollArea::both()
        .max_height(320.0)
        .auto_shrink([true, true])
        .show(ui, |ui| {
            egui::Grid::new(id).striped(true).show(ui, |ui| {
                for c in columns {
                    ui.strong(c);
                }
                ui.end_row();
                for (i, r) in rows.iter().enumerate() {
                    for c in r {
                        if clickable {
                            let resp = ui
                                .add(egui::Label::new(c).sense(egui::Sense::click()))
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if resp.clicked() {
                                clicked = Some(i);
                            }
                        } else {
                            ui.label(c);
                        }
                    }
                    ui.end_row();
                }
            });
        });
    clicked
}

/// Edit from schedule (L-27): selects the object a schedule row stands for
/// and centres the plan on it, going to its floor first. Rows of several
/// objects (grouped) select the first one. False when the row has no
/// selectable object (a totals line, a plant of the terrain).
pub fn select_row_target(
    cx: &mut EditorContext,
    cam: &mut Camera,
    targets: &[plan_docs::schedule_kinds::RowTarget],
) -> bool {
    use plan_core::schedules::ScheduleKind as K;
    let Some(t) = targets.first() else {
        return false;
    };
    if t.floor >= cx.project.floors.len() {
        return false;
    }
    if t.floor != cx.floor {
        cx.floor = t.floor;
        cx.reset_view_state();
    }
    cx.refresh();
    cam.center = t.position;
    let object = match t.kind {
        K::Door | K::Window => Some(ObjectRef::Opening(t.id)),
        K::Wall => Some(ObjectRef::Wall(t.id)),
        K::Cabinet => Some(ObjectRef::Cabinet(t.id)),
        K::Electrical => Some(ObjectRef::Device(t.id)),
        K::Fixture | K::Furniture | K::Plant => Some(ObjectRef::Symbol(t.id)),
        K::Note => Some(ObjectRef::Cad(t.id)),
        K::Stair => Some(ObjectRef::Stair(t.id)),
        K::Framing => Some(ObjectRef::Framing(t.id)),
        K::Room | K::RoomFinish => match rooms_edit::room_index_at(cx, t.position) {
            Some(i) => {
                rooms_edit::select_room(cx, i);
                cx.status = "Room selected".into();
                return true;
            }
            None => None,
        },
        K::General => None,
    };
    match object {
        Some(o) if t.id != 0 && o.exists_in(&cx.project, cx.floor) => {
            rooms_edit::clear_room_selection();
            cx.selection.set(o);
            cx.status = "Selected from the schedule".into();
            true
        }
        _ => {
            cx.status = "That row has no object to select in the plan".into();
            false
        }
    }
}

fn schedule_window(
    ctx: &egui::Context,
    cx: &mut EditorContext,
    cam: &mut Camera,
    kind: SchedKind,
) -> bool {
    let sched = schedule_for(cx, kind);
    let mut open = true;
    let mut export = false;
    let mut export_xlsx = false;
    let mut place = false;
    let mut clicked = None;
    egui::Window::new(sched.title.clone())
        .id(egui::Id::new(("schedule", kind as u8)))
        .open(&mut open)
        .default_pos(
            ctx.screen_rect().center()
                + egui::vec2(30.0 * kind as u8 as f32, 20.0 * kind as u8 as f32),
        )
        .show(ctx, |ui| {
            clicked = clickable_table(
                ui,
                &format!("sched_{}", kind as u8),
                &sched.columns,
                &sched.rows,
                true,
            );
            ui.separator();
            ui.horizontal(|ui| {
                place = ui
                    .button("Place on Plan")
                    .on_hover_text(
                        "Puts this schedule in the plan as a table that stays up to date",
                    )
                    .clicked();
                export = ui.button("Export CSV\u{2026}").clicked();
                export_xlsx = ui.button("Export Excel\u{2026}").clicked();
            });
        });
    if let Some(row) = clicked {
        // The rows are the objects of the live schedule of the same kind, in
        // the same order; select the one clicked.
        let def = plan_core::schedules::Schedule::new(kind.plan_kind(), Point::ZERO);
        let rooms = Some((cx.floor, cx.rooms.as_slice()));
        let targets = plan_docs::schedule_kinds::row_targets(&cx.project, &def, cx.floor, rooms);
        if targets.len() == sched.rows.len() {
            if let Some(t) = targets.get(row) {
                select_row_target(cx, cam, t);
            }
        }
    }
    if place {
        let id = schedule_view::add(cx, kind.plan_kind(), cam.center);
        schedule_view::select(cx, id);
        cx.status = format!("{} placed in the plan", sched.title);
    }
    if export {
        let name = format!("{}.csv", sched.title.replace(' ', "_"));
        cx.status = save_text(&name, "csv", &sched.to_csv());
    }
    if export_xlsx {
        let name = format!("{}.xlsx", sched.title.replace(' ', "_"));
        cx.status = save_bytes(&name, "xlsx", &sched.to_xlsx());
    }
    open
}

/// A placed schedule shown in its own window: the same table as in the plan.
fn placed_window(
    ctx: &egui::Context,
    cx: &mut EditorContext,
    cam: &mut Camera,
    floor: usize,
    id: plan_core::Id,
) -> bool {
    let layer = plan_core::schedules::ScheduleLayer::load(&cx.project.floors[floor]);
    let Some(def) = layer.find(id) else {
        return false;
    };
    let sched = schedule_view::table_for(cx, def, floor);
    let mut open = true;
    let mut export = false;
    let mut export_xlsx = false;
    let mut clicked = None;
    egui::Window::new(sched.title.clone())
        .id(egui::Id::new(("placed_schedule", id)))
        .open(&mut open)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            clicked = clickable_table(
                ui,
                &format!("placed_sched_{id}"),
                &sched.columns,
                &sched.rows,
                true,
            );
            ui.weak("Click a row to select the object in the plan.");
            ui.separator();
            ui.horizontal(|ui| {
                export = ui.button("Export CSV\u{2026}").clicked();
                export_xlsx = ui.button("Export Excel\u{2026}").clicked();
            });
        });
    if let Some(row) = clicked {
        let rooms = (floor == cx.floor).then_some((floor, cx.rooms.as_slice()));
        let targets = plan_docs::schedule_kinds::row_targets(&cx.project, def, floor, rooms);
        if let Some(t) = targets.get(row) {
            select_row_target(cx, cam, t);
        }
    }
    if export {
        let name = format!("{}.csv", sched.title.replace(' ', "_"));
        cx.status = save_text(&name, "csv", &sched.to_csv());
    }
    if export_xlsx {
        let name = format!("{}.xlsx", sched.title.replace(' ', "_"));
        cx.status = save_bytes(&name, "xlsx", &sched.to_xlsx());
    }
    open
}

/// The Materials List window: live lists and Reports with Chief's 21 columns,
/// the Master List, saved lists and the dialogs that go with them (see
/// `dialogs::materials_list`).
fn materials_window(ctx: &egui::Context, cx: &mut EditorContext, cam: &mut Camera) -> bool {
    super::materials_list::show(ctx, cx, cam)
}

/// Puts the Materials List window up (the Tools > Materials List commands
/// and the Project Browser).
pub fn open_materials_window() {
    with_windows(|w| w.materials = true);
}

// ----- Create Construction Set -----

/// The default construction set as PDF bytes (its Materials List page is
/// priced from the user's Master List).
pub fn construction_set_pdf(project: &plan_core::Project) -> Vec<u8> {
    let master = crate::shell::layout_window::load_master_list();
    let layout = plan_layout::default_construction_set_with(project, project.floors.len(), &master);
    let rcx = crate::shell::layout_window::render_context(project);
    render_pdf(&layout, &rcx)
}

/// File > New Layout: makes the project's layout from the template and shows
/// it in the layout window.
fn new_layout_from_template(cx: &mut EditorContext) {
    crate::shell::layout_window::new_layout(cx);
}

fn create_construction_set(cx: &mut EditorContext) {
    // The sheets go into the live layout first (Cover with the sheet index,
    // Site, plans, elevations, sections, details, schedules); the PDF is the
    // copy to send out.
    let added = crate::shell::layout_window::install_construction_set(cx);
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{} Construction Set.pdf", cx.project.name))
        .add_filter("pdf", &["pdf"])
        .save_file()
    else {
        cx.status = format!("{added}; no PDF saved");
        return;
    };
    let bytes = construction_set_pdf(&cx.project);
    cx.status = write_file(&path, &bytes);
}

// ----- the window host -----

#[derive(Default)]
struct Windows {
    room: Option<RoomDialog>,
    /// The Properties tab of the Room Specification.
    room_props: Option<super::property_manager::SharedSession>,
    /// The Components and Object Information tabs of the Room Specification.
    room_info: Option<super::object_info::SharedInfo>,
    floor: Option<FloorDialog>,
    space: Option<SpaceWindow>,
    check: Option<CheckWindow>,
    materials: bool,
    schedules: Vec<SchedKind>,
    /// Tools > Project Information.
    project_info: Option<ProjectInfoDialog>,
    /// Edit > Find/Replace Text.
    find_replace: Option<FindReplaceDialog>,
    /// Edit > Snap Settings.
    snap_settings: Option<super::snap_settings::SnapSettingsDialog>,
    /// Edit > Edit Behaviors.
    edit_behaviors: Option<super::edit_behaviors::EditBehaviorsDialog>,
    /// Schedule Specification of a placed schedule.
    sched_spec: Option<ScheduleSpecDialog>,
    /// A double-click asked for this schedule's specification `(floor, id)`.
    sched_spec_request: Option<(usize, plan_core::Id)>,
    /// Placed schedules shown in their own window `(floor, id)`.
    placed: Vec<(usize, plan_core::Id)>,
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
            let d = FloorDialog::new_floor(&cx.project, cx.floor, &cx.defaults);
            with_windows(|w| w.floor = Some(d));
        }
        Action::InsertFloor => {
            rooms_edit::insert_floor(cx);
        }
        Action::InsertFloorBelow => {
            // Insert New Floor: a floor below this one, derived from its
            // walls, with the Build New Floor options (manual p. 764).
            let d = FloorDialog::insert_floor(&cx.project, cx.floor, &cx.defaults);
            with_windows(|w| w.floor = Some(d));
        }
        Action::FloorDefaults => {
            let d = FloorDialog::defaults_for_floor(cx);
            with_windows(|w| w.floor = Some(d));
        }
        Action::FoundationDefaults => {
            let d = FloorDialog::foundation_defaults(cx);
            with_windows(|w| w.floor = Some(d));
        }
        Action::PlanFloorDefaults => {
            let d = FloorDialog::defaults_for_plan(cx);
            with_windows(|w| w.floor = Some(d));
        }
        Action::ReferenceDisplayOptions => {
            let d = FloorDialog::reference(cx);
            with_windows(|w| w.floor = Some(d));
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
            let d = FloorDialog::build_foundation(cx);
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
            let run = run_check_full(cx, kind);
            let window = CheckWindow::from_run(kind, cx.floor, run);
            cx.status = window.summary();
            with_windows(|w| w.check = Some(window));
        }
        Action::PlanFootprint => {
            // The outer wall faces (L-38); the rooms' footprint when there
            // are no walls to trace.
            if crate::tools::cad_ops::plan_footprint(cx).is_none() {
                rooms_edit::add_plan_footprint(cx);
            }
        }
        Action::MaterialsList => {
            super::materials_list::run_command(cx, super::materials_list::cmd::OPEN);
        }
        Action::DoorSchedule => open_schedule(SchedKind::Door),
        Action::WindowSchedule => open_schedule(SchedKind::Window),
        Action::RoomSchedule => open_schedule(SchedKind::Room),
        Action::WallSchedule => open_schedule(SchedKind::Wall),
        Action::CreateConstructionSet => create_construction_set(cx),
        Action::FileNewLayout => new_layout_from_template(cx),
        Action::ProjectInfo => open_project_info(cx),
        Action::FindReplaceText => with_windows(|w| {
            if w.find_replace.is_none() {
                w.find_replace = Some(FindReplaceDialog::new());
            }
        }),
        Action::SnapSettings => {
            let d = super::snap_settings::SnapSettingsDialog::new(cx);
            with_windows(|w| w.snap_settings = Some(d));
        }
        Action::EditBehaviors => {
            let d = super::edit_behaviors::EditBehaviorsDialog::new(cx);
            with_windows(|w| w.edit_behaviors = Some(d));
        }
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

/// Send to Layout for a placed schedule: a box that shows its table, put on
/// the last page of the plan's layout (one undo step). False, with a status
/// message, when there is no layout or no such schedule.
pub fn send_schedule_to_layout(cx: &mut EditorContext, floor: usize, id: plan_core::Id) -> bool {
    use crate::shell::layout_window as lw;
    let found = cx.project.floors.get(floor).is_some_and(|f| {
        plan_core::schedules::ScheduleLayer::load(f)
            .find(id)
            .is_some()
    });
    if !found {
        cx.status = "That schedule is no longer in the plan".into();
        return false;
    }
    let Some(mut layout) = lw::load(&cx.project) else {
        cx.status = "Start a layout first (File > New Layout), then send the schedule".into();
        return false;
    };
    let page = layout.content_pages().last().map_or(1, |p| p.number);
    cx.refresh();
    let ctx = plan_layout::LayoutRenderContext::new(&cx.project);
    plan_layout::send_to_layout(
        &mut layout,
        &ctx,
        page,
        plan_layout::BoxSource::PlacedSchedule { floor, id },
        plan_docs::Scale::QuarterInch,
        None,
    );
    drop(ctx);
    cx.begin_change("Send to Layout");
    lw::store(&mut cx.project, &layout);
    cx.mark_dirty();
    cx.status = format!("Schedule sent to layout page A-{page}");
    true
}

/// Opens the Project Information dialog on the plan's current values.
pub fn open_project_info(cx: &EditorContext) {
    let d = ProjectInfoDialog::new(&cx.project.info);
    with_windows(|w| w.project_info = Some(d));
}

/// Is the Project Information dialog open?
#[cfg(test)]
pub fn project_info_open() -> bool {
    with_windows(|w| w.project_info.is_some())
}

/// Closes the Project Information dialog without applying it.
#[cfg(test)]
pub fn close_project_info() {
    with_windows(|w| w.project_info = None);
}

/// Asks for the Schedule Specification of schedule `id` placed on `floor`;
/// the dialog opens on the next frame.
pub fn open_schedule_spec(floor: usize, id: plan_core::Id) {
    with_windows(|w| w.sched_spec_request = Some((floor, id)));
}

/// Builds the Schedule Specification dialog for schedule `id` on `floor`.
fn schedule_spec_dialog(
    cx: &EditorContext,
    floor: usize,
    id: plan_core::Id,
) -> Option<ScheduleSpecDialog> {
    let layer = plan_core::schedules::ScheduleLayer::load(cx.project.floors.get(floor)?);
    let def = layer.find(id)?.clone();
    let styles = cx
        .project
        .text_styles
        .names()
        .into_iter()
        .map(String::from)
        .collect();
    let mut layers: Vec<String> = cx
        .project
        .layers
        .layers
        .iter()
        .map(|l| l.name.clone())
        .collect();
    if !layers.contains(&def.layer) {
        layers.push(def.layer.clone());
    }
    let size = {
        let l = schedule_view::layout_of(cx, &def, floor);
        (l.width, l.height)
    };
    Some(
        ScheduleSpecDialog::new(floor, def, styles, layers)
            .with_context(super::schedule_spec::SpecContext::from_cx(cx))
            .with_size(size)
            .with_props(cx.project.props.defs.clone()),
    )
}

/// Draws every open window of this module and applies what the user accepted.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext, cam: &mut Camera) {
    let mut w = with_windows(std::mem::take);
    w.show(ctx, cx, cam);
    with_windows(|slot| *slot = w);
    super::plan_check::show_text_report(ctx);
    // Schedule Defaults, Select Location, Manage Custom Schedule Categories
    // and the other windows of the schedule commands.
    super::schedule_spec::show_extras(ctx, cx, cam);
    // Fix Off Angle Wall (W-133).
    super::fix_connections::show(ctx, cx);
}

/// Tools > Checks > Plan Check Settings: opens the Plan Check window (running
/// the check when it is not up) with its Settings dialog showing.
pub(super) fn open_check_settings(cx: &mut EditorContext) {
    cx.refresh();
    let up = with_windows(|w| w.check.as_ref().is_some_and(|c| c.kind == CheckKind::Plan));
    if !up {
        let run = run_check_full(cx, CheckKind::Plan);
        let window = CheckWindow::from_run(CheckKind::Plan, cx.floor, run);
        cx.status = window.summary();
        with_windows(|w| w.check = Some(window));
    }
    with_windows(|w| {
        if let Some(c) = w.check.as_mut() {
            c.open_settings(cx);
        }
    });
}

/// Is the Plan Check Settings dialog showing?
#[cfg(test)]
pub fn check_settings_open() -> bool {
    with_windows(|w| w.check.as_ref().is_some_and(CheckWindow::settings_open))
}

impl Windows {
    fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext, cam: &mut Camera) {
        // Room Specification.
        if self.room.is_none() && self.floor.is_none() {
            if let Some(idx) = rooms_edit::take_room_dialog_request(cx) {
                self.room = rooms_edit::room_dialog_init(cx, idx).map(RoomDialog::new);
                self.room_props = super::property_manager::PropSession::for_object(
                    cx,
                    crate::editor::ObjectRef::Room(idx),
                );
                self.room_info = super::object_info::InfoSession::for_object(
                    cx,
                    crate::editor::ObjectRef::Room(idx),
                );
            }
        }
        if let Some(mut d) = self.room.take() {
            let shown = super::object_info::with_current(self.room_info.as_ref(), || {
                super::property_manager::with_current(self.room_props.as_ref(), || d.show(ctx))
            });
            match shown {
                Outcome::Open => self.room = Some(d),
                outcome => {
                    if outcome == Outcome::Ok {
                        let depth = super::property_manager::before_apply(cx);
                        rooms_edit::apply_room_spec(cx, d.room_index(), d.room_name(), d.extras());
                        // Layer definitions saved from the Structure panel.
                        for n in d.take_saved() {
                            cx.project
                                .assemblies
                                .save_named(n.kind, &n.name, n.assembly);
                        }
                        super::property_manager::after_apply(cx, self.room_props.as_ref(), depth);
                        super::object_info::after_apply(cx, self.room_info.as_ref(), depth);
                    }
                    self.room_props = None;
                    self.room_info = None;
                    // The Enter that closed the dialog also reached the Select
                    // tool, which asked for it again.
                    let _ = rooms_edit::take_room_dialog_request(cx);
                }
            }
        }
        // Exterior Room Specification (R-106).
        super::exterior_room::show(ctx, cx);
        // Floor dialogs.
        if let Some(mut d) = self.floor.take() {
            match d.show(ctx) {
                Outcome::Open => self.floor = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    let new_floor = matches!(d, FloorDialog::NewFloor { .. });
                    let floors = cx.project.floors.len();
                    d.apply(cx);
                    // The Floor Defaults of the floor just built open next
                    // (manual p. 762).
                    if new_floor && cx.project.floors.len() > floors {
                        self.floor = Some(FloorDialog::defaults_for_floor(cx));
                    }
                }
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
            self.materials = materials_window(ctx, cx, cam);
        }
        let kinds = std::mem::take(&mut self.schedules);
        self.schedules = kinds
            .into_iter()
            .filter(|k| schedule_window(ctx, cx, cam, *k))
            .collect();
        self.show_placed_schedules(ctx, cx, cam);
        self.show_project_info(ctx, cx);
        if let Some(mut d) = self.find_replace.take() {
            if d.show(ctx, cx) {
                self.find_replace = Some(d);
            }
        }
        if let Some(mut d) = self.snap_settings.take() {
            if d.show(ctx, cx) {
                self.snap_settings = Some(d);
            }
        }
        if let Some(mut d) = self.edit_behaviors.take() {
            if d.show(ctx, cx) {
                self.edit_behaviors = Some(d);
            }
        }
    }

    /// Project Information: OK stores the values as one undo step.
    fn show_project_info(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        let Some(mut d) = self.project_info.take() else {
            return;
        };
        match d.show(ctx) {
            Outcome::Open => self.project_info = Some(d),
            Outcome::Cancel => {}
            Outcome::Ok => {
                project_info::apply(cx, d.draft());
            }
        }
    }

    /// The Schedule Specification dialog and the placed-schedule windows.
    fn show_placed_schedules(
        &mut self,
        ctx: &egui::Context,
        cx: &mut EditorContext,
        cam: &mut Camera,
    ) {
        if let Some((floor, id)) = self.sched_spec_request.take() {
            if self.sched_spec.is_none() {
                self.sched_spec = schedule_spec_dialog(cx, floor, id);
            }
        }
        if let Some(mut d) = self.sched_spec.take() {
            let outcome = d.show(ctx);
            let actions = d.take_actions();
            if actions.export_for_editing {
                // Exports the schedule as the dialog shows it, edits included.
                super::property_manager::request_export_one(d.floor(), d.draft().clone());
            }
            if actions.import_props {
                super::property_manager::request_import();
            }
            if actions.send_to_layout {
                // The box shows the stored schedule, so store the edits first.
                schedule_view::replace(cx, d.floor(), d.draft().clone());
                send_schedule_to_layout(cx, d.floor(), d.id());
            }
            if actions.export_csv || actions.export_xlsx || actions.open_window {
                // Preview the unsaved edits in the table that is exported.
                let table = schedule_view::table_for(cx, d.draft(), d.floor());
                if actions.export_csv {
                    let name = format!("{}.csv", table.title.replace(' ', "_"));
                    cx.status = save_text(&name, "csv", &table.to_csv());
                }
                if actions.export_xlsx {
                    let name = format!("{}.xlsx", table.title.replace(' ', "_"));
                    cx.status = save_bytes(&name, "xlsx", &table.to_xlsx());
                }
                if actions.open_window {
                    // The window reads the stored schedule, so store the edits.
                    schedule_view::replace(cx, d.floor(), d.draft().clone());
                    self.placed_open(d.floor(), d.id());
                }
            }
            match outcome {
                Outcome::Open => self.sched_spec = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    schedule_view::replace(cx, d.floor(), d.draft().clone());
                }
            }
        }
        let placed = std::mem::take(&mut self.placed);
        self.placed = placed
            .into_iter()
            .filter(|(f, id)| placed_window(ctx, cx, cam, *f, *id))
            .collect();
    }

    fn placed_open(&mut self, floor: usize, id: plan_core::Id) {
        if !self.placed.contains(&(floor, id)) {
            self.placed.push((floor, id));
        }
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

    #[test]
    fn a_placed_schedule_can_be_sent_to_the_layout() {
        use plan_core::schedules::{Schedule, ScheduleKind};
        let mut cx = house();
        let id = schedule_view::add(&mut cx, ScheduleKind::Door, Point::new(10.0, 10.0));
        // No layout yet.
        assert!(!send_schedule_to_layout(&mut cx, 0, id));
        assert!(cx.status.contains("layout"), "{}", cx.status);
        let mut layout = plan_layout::Layout::new("t", plan_docs::SheetSize::ArchC);
        layout.add_page(1, "Schedules");
        crate::shell::layout_window::store(&mut cx.project, &layout);
        assert!(send_schedule_to_layout(&mut cx, 0, id));
        let back = crate::shell::layout_window::load(&cx.project).unwrap();
        let boxes = &back.page(1).unwrap().boxes;
        assert_eq!(boxes.len(), 1);
        assert_eq!(
            boxes[0].source,
            plan_layout::BoxSource::PlacedSchedule { floor: 0, id }
        );
        assert_eq!(cx.undo_label(), Some("Send to Layout"));
        cx.undo();
        let back = crate::shell::layout_window::load(&cx.project).unwrap();
        assert!(back.page(1).unwrap().boxes.is_empty());
        // A schedule that does not exist is refused.
        let _ = Schedule::new(ScheduleKind::Door, Point::ZERO);
        assert!(!send_schedule_to_layout(&mut cx, 0, 999));
    }

    #[test]
    fn clicking_a_schedule_row_selects_the_object() {
        use plan_core::schedules::{Schedule, ScheduleKind};
        let mut cx = house();
        let mut cam = Camera::default_view();
        for (kind, want) in [
            (ScheduleKind::Door, "opening"),
            (ScheduleKind::Window, "opening"),
            (ScheduleKind::Wall, "wall"),
            (ScheduleKind::Room, "room"),
        ] {
            let def = Schedule::new(kind, Point::ZERO);
            let rooms = Some((0, cx.rooms.as_slice()));
            let targets = plan_docs::schedule_kinds::row_targets(&cx.project, &def, 0, rooms);
            assert!(!targets.is_empty(), "{kind:?}");
            cx.selection.clear();
            assert!(
                select_row_target(&mut cx, &mut cam, &targets[0]),
                "{kind:?}"
            );
            match (want, cx.selection.single()) {
                ("opening", Some(ObjectRef::Opening(_))) | ("wall", Some(ObjectRef::Wall(_))) => {}
                ("room", None) => {
                    assert!(rooms_edit::selected_room(&cx).is_some());
                }
                (w, got) => panic!("{kind:?}: wanted {w}, got {got:?}"),
            }
            assert_eq!(cam.center, targets[0][0].position);
        }
        // A totals line has nothing behind it.
        assert!(!select_row_target(&mut cx, &mut cam, &[]));
    }

    #[test]
    fn a_row_on_another_floor_switches_to_that_floor() {
        use plan_core::schedules::{FloorScope, Schedule, ScheduleKind};
        let mut cx = house();
        let mut cam = Camera::default_view();
        cx.project
            .floors
            .push(plan_core::Floor::new("2nd Floor", 109.0));
        let w = cx.project.add_wall(
            1,
            Point::ZERO,
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        cx.project
            .add_opening(1, w, 60.0, OpeningKind::Window)
            .unwrap();
        let mut def = Schedule::new(ScheduleKind::Window, Point::ZERO);
        def.floor_scope = FloorScope::All;
        let targets = plan_docs::schedule_kinds::row_targets(&cx.project, &def, 0, None);
        let upstairs = targets
            .iter()
            .find(|t| t[0].floor == 1)
            .expect("a row on floor 2");
        assert!(select_row_target(&mut cx, &mut cam, upstairs));
        assert_eq!(cx.floor, 1);
        assert!(matches!(cx.selection.single(), Some(ObjectRef::Opening(_))));
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
            // The same table as an Excel workbook: a zip with the sheet in it.
            let xlsx = schedule_for(&cx, kind).to_xlsx();
            let parts = plan_library::archive::read_zip(&xlsx).expect("a workbook");
            let sheet = parts
                .iter()
                .find(|(n, _)| n == "xl/worksheets/sheet1.xml")
                .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
                .expect("a sheet");
            assert!(sheet.matches("<row ").count() >= 2, "{kind:?}");
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

    /// One frame of `show_all`, with Enter pressed (OK) when `enter`.
    fn frame(ctx: &egui::Context, cx: &mut EditorContext, enter: bool) {
        let mut input = egui::RawInput::default();
        if enter {
            input.events.push(egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        let mut cam = Camera::default_view();
        let _ = ctx.run(input, |ctx| show_all(ctx, cx, &mut cam));
    }

    #[test]
    fn the_project_information_action_opens_its_dialog() {
        let mut cx = house();
        close_project_info();
        assert!(!project_info_open());
        dispatch(&mut cx, Action::ProjectInfo);
        assert!(project_info_open());
        close_project_info();
    }

    #[test]
    fn opening_a_selected_schedule_asks_for_its_specification() {
        use plan_core::schedules::ScheduleKind;
        let mut cx = house();
        let id = schedule_view::add(&mut cx, ScheduleKind::Door, Point::new(0.0, -40.0));
        let mut spec = crate::shell::spec_dialogs::SpecDialogs::default();
        assert!(spec.open(&mut cx, crate::editor::ObjectRef::Schedule(id)));
        assert_eq!(with_windows(|w| w.sched_spec_request), Some((0, id)));
        assert!(!spec.open(&mut cx, crate::editor::ObjectRef::Schedule(id + 1000)));
    }

    #[test]
    fn schedule_specification_ok_stores_the_edits_as_one_undo_step() {
        use plan_core::schedules::{ScheduleKind, ScheduleLayer};
        let mut cx = house();
        let id = schedule_view::add(&mut cx, ScheduleKind::Door, Point::new(0.0, -40.0));
        open_schedule_spec(0, id);
        let ctx = egui::Context::default();
        frame(&ctx, &mut cx, false);
        frame(&ctx, &mut cx, false);
        with_windows(|w| {
            let d = w.sched_spec.as_mut().expect("dialog opened");
            assert_eq!(d.id(), id);
            let i = d
                .draft()
                .columns
                .iter()
                .position(|c| c.field == "swing")
                .unwrap();
            d.draft_mut().set_column_visible(i, false);
            d.draft_mut().title = "Doors".into();
        });
        frame(&ctx, &mut cx, true);
        assert!(with_windows(|w| w.sched_spec.is_none()), "OK closes it");
        let def = ScheduleLayer::load(cx.floor()).schedules[0].clone();
        assert!(!def.visible_columns().any(|c| c.field == "swing"));
        let t = schedule_view::table_for(&cx, &def, 0);
        assert_eq!(t.title, "Doors");
        assert!(!t.columns.contains(&"Swing".to_string()));
        assert_eq!(cx.undo_label(), Some("Schedule Specification"));
        cx.undo();
        let back = ScheduleLayer::load(cx.floor()).schedules[0].clone();
        assert!(back.visible_columns().any(|c| c.field == "swing"));
    }

    #[test]
    fn cancel_leaves_the_schedule_alone_and_open_in_window_lists_it() {
        use plan_core::schedules::{ScheduleKind, ScheduleLayer};
        let mut cx = house();
        let id = schedule_view::add(&mut cx, ScheduleKind::Window, Point::ZERO);
        let before = ScheduleLayer::load(cx.floor());
        open_schedule_spec(0, id);
        let ctx = egui::Context::default();
        frame(&ctx, &mut cx, false);
        frame(&ctx, &mut cx, false);
        with_windows(|w| {
            let d = w.sched_spec.as_mut().unwrap();
            d.draft_mut().title = "Edited".into();
            d.raise_open_window();
        });
        frame(&ctx, &mut cx, false);
        // Open in Window stores the edits and shows the table.
        assert_eq!(with_windows(|w| w.placed.clone()), vec![(0, id)]);
        assert_ne!(ScheduleLayer::load(cx.floor()), before);
        // The placed window draws.
        frame(&ctx, &mut cx, false);
        assert_eq!(with_windows(|w| w.placed.len()), 1);
        // Deleting the schedule closes its window.
        schedule_view::delete(&mut cx, id);
        frame(&ctx, &mut cx, false);
        assert!(with_windows(|w| w.placed.is_empty()));
    }

    #[test]
    fn project_information_ok_applies_and_undoes() {
        let mut cx = house();
        open_project_info(&cx);
        let ctx = egui::Context::default();
        frame(&ctx, &mut cx, false);
        with_windows(|w| {
            let d = w.project_info.as_mut().expect("dialog open");
            d.info_mut().client_name = "Pat Smith".into();
            d.info_mut().project_number = "26-014".into();
        });
        frame(&ctx, &mut cx, true);
        assert!(!project_info_open());
        assert_eq!(cx.project.info.client_name, "Pat Smith");
        assert_eq!(cx.undo_label(), Some("Project Information"));
        cx.undo();
        assert!(cx.project.info.client_name.is_empty());
    }

    #[test]
    fn schedule_windows_map_to_placeable_kinds() {
        use plan_core::schedules::ScheduleKind as K;
        let kinds = [
            (SchedKind::Door, K::Door),
            (SchedKind::Window, K::Window),
            (SchedKind::Room, K::Room),
            (SchedKind::Wall, K::Wall),
        ];
        for (w, k) in kinds {
            assert_eq!(w.plan_kind(), k);
        }
    }
}
