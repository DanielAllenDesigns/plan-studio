//! Tools > Materials List (manual pp. 1368-1389): live lists and static
//! Reports made from the whole plan, one floor, a Materials List Polyline, a
//! room or the selection, with Chief's 21 columns, a Specification dialog
//! (General, Categories, Columns, Report, Text Style, Layer), saved lists in
//! the Project Browser and the Management dialog, the Master List, Details and
//! Find Object, and Export (TXT, CSV, XML, HTML, XLSX), Print and Send to
//! Layout.
//!
//! The take-off, columns, totals, exports and master list are in
//! `plan_docs::materials`; the data a plan keeps is `plan_core::materials_data`.
//! This module holds what the window remembers and the commands that start
//! a list ([`run_command`]); `table`, `spec`, `master` and `extras` draw the
//! pieces.

mod extras;
mod master;
mod spec;
mod table;

use crate::editor::{rooms_edit, Camera, EditorContext, EditorRequest, ObjectRef};
use crate::shell::layout_window as lw;
use eframe::egui;
use plan_core::materials_data::{
    ListKind, ListScope, ListSpec, MlColumn, ObjectAddr, ReportRow, SavedList,
};
use plan_core::Id;
use plan_docs::materials::list::{self, Calc, Calculated, ListLine};
use plan_docs::MasterList;
use std::cell::RefCell;
use std::collections::BTreeSet;

// ------------------------------------------------------------ command ids --

/// Command ids (`Action::Custom`) of the Tools > Materials List submenu and
/// the Edit toolbar.
pub mod cmd {
    pub const OPEN: &str = "materials.open";
    pub const ALL: &str = "materials.all";
    pub const FLOOR: &str = "materials.floor";
    pub const SELECTION: &str = "materials.selection";
    pub const ROOM: &str = "materials.room";
    pub const POLYLINE_TOOL: &str = "materials.polyline_tool";
    pub const POLYLINE_CALC: &str = "materials.polyline_calc";
    pub const POLYLINE_DEFAULTS: &str = "materials.polyline_defaults";
    pub const MASTER: &str = "materials.master";
    pub const MANAGE: &str = "materials.manage";
    pub const REPORT: &str = "materials.report";
    pub const SAVE: &str = "materials.save";
    pub const SAVE_AS: &str = "materials.save_as";
    pub const EDIT_VIEW: &str = "materials.edit_view";
    pub const UPDATE_FROM: &str = "materials.update_from";
    pub const UPDATE_TO: &str = "materials.update_to";
    pub const PRINT: &str = "materials.print";
    pub const EXPORT: &str = "materials.export";
}

// ------------------------------------------------------------------- state --

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum View {
    #[default]
    List,
    Master,
}

/// A cell being typed into.
#[derive(Clone, Debug, PartialEq)]
pub struct CellEdit {
    /// The row of the displayed list (an index into the lines).
    pub line: usize,
    pub col: MlColumn,
    pub text: String,
}

/// What the Materials List window remembers.
#[derive(Default)]
pub struct State {
    pub open: bool,
    pub view: View,
    pub spec: ListSpec,
    /// The frozen rows of a Report.
    pub rows: Vec<ReportRow>,
    /// The name the list was saved or opened under.
    pub saved_as: Option<String>,
    pub dirty: bool,
    pub selected: BTreeSet<usize>,
    pub expanded: BTreeSet<usize>,
    pub edit: Option<CellEdit>,
    pub status: String,
    /// The master list being looked at (edited in the Master List view).
    pub master: Option<MasterList>,
    pub master_dirty: bool,
    pub master_ui: master::MasterUi,
    pub spec_dialog: Option<spec::SpecDialog>,
    pub extras: extras::Extras,
    /// Find Object asked for `(floor, key)`; run with the camera.
    pub find: Option<(usize, String)>,
    /// The last list the window drew (for Details, Find, Export).
    pub shown: Vec<ListLine>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Runs `f` on the window's state.
pub fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

// --------------------------------------------------------------- calculate --

fn load_master(st: &State) -> MasterList {
    st.master.clone().unwrap_or_else(lw::load_master_list)
}

/// The lines of the list `st` describes.
pub fn compute(cx: &EditorContext, st: &State) -> Calculated {
    match st.spec.kind {
        ListKind::Report => Calculated {
            lines: list::thaw(&st.rows),
            note: None,
        },
        ListKind::Live => {
            let master = load_master(st);
            list::calculate(
                &Calc {
                    project: &cx.project,
                    master: &master,
                    active_rooms: Some((cx.floor, cx.rooms.as_slice())),
                },
                &st.spec,
            )
        }
    }
}

/// The key of an object for the Materials List (see [`super::object_info`]).
fn key_of(cx: &EditorContext, o: ObjectRef) -> Option<String> {
    super::object_info::object_key(cx, o)
}

/// The selected objects (and the selected room) as list addresses.
pub fn selection_addrs(cx: &EditorContext) -> Vec<ObjectAddr> {
    let mut v: Vec<ObjectAddr> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| {
            key_of(cx, *o).map(|key| ObjectAddr {
                floor: cx.floor,
                key,
            })
        })
        .collect();
    if let Some(i) = rooms_edit::selected_room(cx) {
        if let Some(key) = key_of(cx, ObjectRef::Room(i)) {
            v.push(ObjectAddr {
                floor: cx.floor,
                key,
            });
        }
    }
    v
}

// ------------------------------------------------------------- start a list --

fn start(st: &mut State, spec: ListSpec) {
    st.spec = spec;
    st.rows.clear();
    st.saved_as = None;
    st.dirty = false;
    st.selected.clear();
    st.expanded.clear();
    st.edit = None;
    st.view = View::List;
    st.open = true;
    st.shown.clear();
}

fn open_window() {
    super::build_tools::open_materials_window();
}

/// Calculate Materials for All Floors.
pub fn calculate_all(st: &mut State) {
    start(st, ListSpec::new("All Floors", ListScope::AllFloors));
}

/// A list of one floor (Restrict to Floor).
pub fn calculate_floor(st: &mut State, floor: usize, name: &str) {
    start(st, ListSpec::new(name, ListScope::Floor(floor)));
}

/// Calculate Materials From Selection. False when nothing that makes
/// materials is selected.
pub fn calculate_selection(cx: &EditorContext, st: &mut State) -> bool {
    let addrs = selection_addrs(cx);
    if addrs.is_empty() {
        st.status = "Select one or more objects first".into();
        return false;
    }
    start(st, ListSpec::new("Selection", ListScope::Selection(addrs)));
    true
}

/// Calculate Materials in Room: the selected room, else the room under the
/// view's centre of the active floor when there is only one.
pub fn calculate_room(cx: &EditorContext, st: &mut State) -> bool {
    let idx = rooms_edit::selected_room(cx).or_else(|| (cx.rooms.len() == 1).then_some(0));
    let Some(room) = idx.and_then(|i| cx.rooms.get(i)) else {
        st.status = "Select a room first".into();
        return false;
    };
    let name = plan_docs::room_name(cx.floor(), room);
    let scope = ListScope::Room {
        floor: cx.floor,
        x: room.centroid.x,
        y: room.centroid.y,
    };
    start(st, ListSpec::new(format!("Room {name}"), scope));
    true
}

/// Calculate From Area: the Materials List Polyline `cad_id`.
pub fn calculate_polyline(cx: &EditorContext, st: &mut State, cad_id: Id) -> bool {
    let Some(pl) = cx.project.materials.polyline(cad_id) else {
        st.status = "That is not a Materials List Polyline".into();
        return false;
    };
    let name = pl.name.clone();
    start(st, ListSpec::new(name, ListScope::Polyline(cad_id)));
    true
}

/// Opens a saved list (Edit in the Management dialog, a double-click in the
/// Project Browser). A live list is re-calculated from the plan as it is now.
pub fn open_saved(st: &mut State, project: &plan_core::Project, name: &str) -> bool {
    let Some(saved) = project.materials.list(name) else {
        return false;
    };
    let mut spec = saved.spec.clone();
    spec.normalize_columns();
    start(st, spec);
    st.rows = saved.rows.clone();
    st.saved_as = Some(name.to_string());
    true
}

/// Save Active View: files the list under `name` (the name it has when
/// `None`) as one undo step. Returns the name it was saved as.
pub fn save_list(cx: &mut EditorContext, st: &mut State, name: Option<&str>) -> String {
    let wanted = name.unwrap_or(&st.spec.name).trim().to_string();
    let wanted = if wanted.is_empty() {
        "Materials List".to_string()
    } else {
        wanted
    };
    let replaces = st.saved_as.as_deref() == Some(wanted.as_str());
    cx.begin_change("Save Materials List");
    let final_name = if replaces || cx.project.materials.list(&wanted).is_none() {
        wanted
    } else {
        cx.project.materials.unique_name(&wanted)
    };
    st.spec.name = final_name.clone();
    cx.project.materials.save_list(SavedList {
        spec: st.spec.clone(),
        rows: if st.spec.kind == ListKind::Report {
            st.rows.clone()
        } else {
            Vec::new()
        },
    });
    cx.mark_dirty();
    st.saved_as = Some(final_name.clone());
    st.dirty = false;
    st.status = format!("Saved the list as {final_name}");
    final_name
}

/// Save Active View As: a copy under a new name.
pub fn save_list_as(cx: &mut EditorContext, st: &mut State, new_name: &str) -> String {
    st.saved_as = None;
    save_list(cx, st, Some(new_name))
}

/// Generate a Report: freezes the lines of the live list into a static
/// Report (editable, no longer linked to the plan).
pub fn generate_report(cx: &EditorContext, st: &mut State) {
    let lines = compute(cx, st).lines;
    st.rows = list::freeze(&lines);
    st.spec.kind = ListKind::Report;
    st.spec.name = format!("{} Report", st.spec.name);
    st.saved_as = None;
    st.dirty = true;
    st.selected.clear();
    st.expanded.clear();
    st.status = "Made a Report from the list; it no longer follows the plan".into();
}

/// Management: deletes the saved list `name`, as one undo step.
pub fn delete_saved(cx: &mut EditorContext, name: &str) -> bool {
    if cx.project.materials.list(name).is_none() {
        return false;
    }
    cx.begin_change("Delete Materials List");
    cx.project.materials.delete_list(name);
    cx.mark_dirty();
    true
}

/// Management: renames a saved list.
pub fn rename_saved(cx: &mut EditorContext, name: &str, new_name: &str) -> bool {
    let ok = cx.project.materials.list(name).is_some()
        && (name == new_name.trim() || cx.project.materials.list(new_name.trim()).is_none())
        && !new_name.trim().is_empty();
    if !ok {
        return false;
    }
    cx.begin_change("Rename Materials List");
    cx.project.materials.rename_list(name, new_name);
    cx.mark_dirty();
    true
}

/// Management: copies a saved list under a new name.
pub fn copy_saved(cx: &mut EditorContext, name: &str, new_name: &str) -> Option<String> {
    cx.project.materials.list(name)?;
    cx.begin_change("Copy Materials List");
    let made = cx.project.materials.copy_list(name, new_name);
    cx.mark_dirty();
    made
}

// ------------------------------------------------------------- find object --

/// The object a key stands for, on `floor` of the project.
pub fn object_of_key(cx: &EditorContext, floor: usize, key: &str) -> Option<ObjectRef> {
    let mut parts = key.split(':');
    let kind = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    let id = |i: usize| rest.get(i).and_then(|s| s.parse::<Id>().ok());
    match kind {
        "wall" => Some(ObjectRef::Wall(id(0)?)),
        "door" | "window" => Some(ObjectRef::Opening(id(0)?)),
        "cabinet" => Some(ObjectRef::Cabinet(id(0)?)),
        "symbol" => Some(ObjectRef::Symbol(id(0)?)),
        "stair" => Some(ObjectRef::Stair(id(0)?)),
        "roof" => Some(ObjectRef::RoofPlane(id(0)?)),
        "framing" => Some(ObjectRef::Framing(id(0)?)),
        "foundation" => Some(ObjectRef::Foundation(id(0)?)),
        "device" => Some(ObjectRef::Device(id(1)?)),
        "room" => {
            let _ = floor;
            let (x, y) = rest.get(1)?.split_once(',')?;
            let (x, y) = (x.parse::<f64>().ok()?, y.parse::<f64>().ok()?);
            cx.rooms
                .iter()
                .position(|r| {
                    (r.centroid.x.round() - x).abs() < 1.5 && (r.centroid.y.round() - y).abs() < 1.5
                })
                .map(ObjectRef::Room)
        }
        _ => None,
    }
}

/// Find Object in Plan: goes to the object's floor, selects it and centres
/// the plan on it. False when it is gone.
pub fn find_object(
    cx: &mut EditorContext,
    cam: Option<&mut Camera>,
    floor: usize,
    key: &str,
) -> bool {
    if floor >= cx.project.floors.len() {
        return false;
    }
    if floor != cx.floor {
        cx.floor = floor;
        cx.reset_view_state();
    }
    cx.refresh();
    let Some(o) = object_of_key(cx, floor, key) else {
        cx.status = "That object is not in the plan".into();
        return false;
    };
    if !o.exists_in(&cx.project, cx.floor) && !matches!(o, ObjectRef::Room(_)) {
        cx.status = "That object is not in the plan".into();
        return false;
    }
    if let Some(c) = object_center(cx, o) {
        if let Some(cam) = cam {
            cam.center = c;
        }
    }
    match o {
        ObjectRef::Room(i) => rooms_edit::select_room(cx, i),
        other => {
            rooms_edit::clear_room_selection();
            cx.selection.set(other);
        }
    }
    cx.status = "Found the object in the plan".into();
    true
}

/// Where an object is, for centring the view.
fn object_center(cx: &EditorContext, o: ObjectRef) -> Option<plan_core::Point> {
    use plan_core::Point;
    let f = cx.floor();
    match o {
        ObjectRef::Wall(id) => f
            .wall(id)
            .map(|w| Point::new((w.start.x + w.end.x) * 0.5, (w.start.y + w.end.y) * 0.5)),
        ObjectRef::Opening(id) => {
            let op = f.openings.iter().find(|x| x.id == id)?;
            let w = f.wall(op.wall_id)?;
            let t = op.center_offset / w.length().max(1e-9);
            Some(Point::new(
                w.start.x + (w.end.x - w.start.x) * t,
                w.start.y + (w.end.y - w.start.y) * t,
            ))
        }
        ObjectRef::Room(i) => cx.rooms.get(i).map(|r| r.centroid),
        ObjectRef::Symbol(id) => f.symbol(id).map(|s| s.position),
        _ => None,
    }
}

// ----------------------------------------------------------------- commands --

#[cfg(test)]
pub use extras::export_bytes;
pub use extras::open_polyline_spec;
#[cfg(test)]
pub use table::apply_edit;

/// The record of a new Materials List Polyline (with the plan's defaults).
pub fn new_polyline_record(
    project: &plan_core::Project,
    floor: usize,
    cad_id: Id,
) -> plan_core::materials_data::MaterialsPolyline {
    extras::new_polyline(project, floor, cad_id)
}

/// The Edit toolbar buttons of the Materials List for the current selection:
/// Calculate Materials From Selection, in Room, and Calculate Materials List
/// for a selected Materials List Polyline.
pub fn edit_buttons(cx: &EditorContext) -> Vec<crate::editor::EditAction> {
    use crate::editor::{EditAction, EditActionKind};
    let button = |id: &'static str, label: &'static str| {
        EditAction::new(EditActionKind::Custom {
            id,
            label,
            icon: "",
        })
    };
    let mut v = Vec::new();
    if cx
        .selection
        .items
        .iter()
        .any(|o| extras::is_polyline(cx, *o).is_some())
    {
        v.push(button(cmd::POLYLINE_CALC, "Calculate Materials List"));
    }
    if rooms_edit::selected_room(cx).is_some() {
        v.push(button(cmd::ROOM, "Calculate Materials in Room"));
    }
    if !selection_addrs(cx).is_empty() {
        v.push(button(cmd::SELECTION, "Calculate Materials From Selection"));
    }
    v
}

/// The Project Browser's Open on a saved list.
pub fn open_from_browser(cx: &EditorContext, name: &str) -> bool {
    let ok = with_state(|st| open_saved(st, &cx.project, name));
    if ok {
        open_window();
    }
    ok
}

/// Is `id` one of this module's commands?
pub fn is_command(id: &str) -> bool {
    id.starts_with("materials.")
}

/// Runs a Materials List command (menu, Edit toolbar). True when handled.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    if !is_command(id) {
        return false;
    }
    cx.refresh();
    let started = with_state(|st| match id {
        cmd::OPEN => {
            st.open = true;
            true
        }
        cmd::ALL => {
            calculate_all(st);
            true
        }
        cmd::FLOOR => {
            let name = cx.floor().name.clone();
            calculate_floor(st, cx.floor, &name);
            true
        }
        cmd::SELECTION => {
            let ok = calculate_selection(cx, st);
            if !ok {
                cx.status = st.status.clone();
            }
            ok
        }
        cmd::ROOM => {
            let ok = calculate_room(cx, st);
            if !ok {
                cx.status = st.status.clone();
            }
            ok
        }
        cmd::POLYLINE_CALC => {
            let target = cx.selection.items.iter().find_map(|o| match o {
                ObjectRef::Cad(id) if cx.project.materials.polyline(*id).is_some() => Some(*id),
                _ => None,
            });
            match target {
                Some(cad) => calculate_polyline(cx, st, cad),
                None => {
                    cx.status = "Select a Materials List Polyline first".into();
                    false
                }
            }
        }
        cmd::MASTER => {
            st.open = true;
            st.view = View::Master;
            true
        }
        cmd::MANAGE => {
            st.extras.open_management();
            true
        }
        cmd::REPORT => {
            generate_report(cx, st);
            st.open = true;
            true
        }
        cmd::SAVE => {
            if st.open {
                save_list(cx, st, None);
            }
            st.open
        }
        cmd::SAVE_AS => {
            st.extras.ask_save_as(&st.spec.name);
            st.open
        }
        cmd::EDIT_VIEW => {
            st.spec_dialog = Some(spec::SpecDialog::new(&st.spec, cx));
            st.open = true;
            true
        }
        cmd::UPDATE_FROM => {
            if st.open {
                update_from_master(cx, st);
            }
            st.open
        }
        cmd::UPDATE_TO => {
            if st.open {
                update_to_master(cx, st);
            }
            st.open
        }
        cmd::EXPORT => {
            st.extras.open_export();
            st.open = true;
            true
        }
        cmd::PRINT => {
            if st.open {
                cx.status = print_list(cx, st, crate::dialogs::print::Destination::Viewer);
            }
            st.open
        }
        cmd::POLYLINE_DEFAULTS => {
            st.extras.open_polyline_defaults(&cx.project);
            true
        }
        _ => false,
    });
    if id == cmd::POLYLINE_TOOL {
        cx.requests.push(EditorRequest::SetTool(
            crate::tools::ToolId::MaterialsPolyline,
        ));
        return true;
    }
    if started {
        open_window();
    }
    started
}

/// Update From Master List on the selected rows (a Report) or the whole list.
pub fn update_from_master(cx: &mut EditorContext, st: &mut State) {
    let master = load_master(st);
    match st.spec.kind {
        ListKind::Report => {
            let n = list::report_update_from_master(&mut st.rows, &master);
            st.dirty = st.dirty || n > 0;
            st.status = format!("Updated {n} line(s) from the Master List");
        }
        ListKind::Live => {
            // A live list reads the Master List every time it is calculated.
            st.status = "A live list follows the Master List; nothing to update".into();
            let _ = cx;
        }
    }
}

/// Update to Master List: the selected rows (every row when none is
/// selected) go into the Master List file.
pub fn update_to_master(cx: &mut EditorContext, st: &mut State) {
    let lines = compute(cx, st).lines;
    let rows: Vec<usize> = if st.selected.is_empty() {
        (0..lines.len()).collect()
    } else {
        st.selected.iter().copied().collect()
    };
    let mut master = load_master(st);
    let n = list::update_to_master(&mut master, &lines, &rows);
    match lw::save_master_list(&master) {
        Ok(()) => st.status = format!("Saved {n} line(s) to the Master List"),
        Err(e) => st.status = e,
    }
    st.master = Some(master);
}

/// File > Print while the window is up: the list as a PDF table.
pub fn print_list(
    cx: &EditorContext,
    st: &State,
    dest: crate::dialogs::print::Destination,
) -> String {
    let lines = compute(cx, st).lines;
    if lines.is_empty() {
        return "The Materials List is empty".into();
    }
    let sub = format!("{} - {}", cx.project.name, st.spec.scope.title());
    let bytes = plan_docs::materials::export::to_pdf(&lines, &st.spec, &st.spec.name, &sub);
    crate::dialogs::print::deliver(&bytes, dest, 1, "materials_list")
}

/// Is a Materials List window up (so File > Print prints it)?
#[allow(dead_code)] // the hook File > Print takes when it routes here
pub fn is_active() -> bool {
    STATE.with(|s| s.try_borrow().map(|s| s.open).unwrap_or(false))
}

// -------------------------------------------------------------- the window --

/// Draws the window and the dialogs that belong to it; false once nothing of
/// it is up.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext, cam: &mut Camera) -> bool {
    let mut st = STATE.with(|s| std::mem::take(&mut *s.borrow_mut()));
    if st.master.is_none() && st.view == View::Master {
        st.master = Some(lw::load_master_list());
    }
    let mut keep = true;
    if st.open {
        let mut open = true;
        egui::Window::new("Materials List")
            .id(egui::Id::new("materials_list_r15"))
            .open(&mut open)
            .default_pos(ctx.screen_rect().center() - egui::vec2(380.0, 260.0))
            .default_size(egui::vec2(900.0, 560.0))
            .resizable(true)
            .show(ctx, |ui| contents(ui, ctx, cx, &mut st));
        if !open {
            let needs = st.dirty || (st.saved_as.is_none() && !st.shown.is_empty());
            let name = st.spec.name.clone();
            st.extras.ask_close(needs, &name);
            if !st.extras.close_prompt_up() {
                st.open = false;
            }
        }
    }
    // The dialogs of the window and the Materials List Polyline's own.
    extras::show_all(ctx, cx, &mut st);
    spec::show(ctx, cx, &mut st);
    // Find Object asked for in this frame.
    if let Some((floor, key)) = st.find.take() {
        find_object(cx, Some(cam), floor, &key);
    }
    if !st.open && !st.extras.any_up() && st.spec_dialog.is_none() {
        if st.master_dirty {
            // Leaving unsaved Master List edits behind: they are dropped.
            st.master = None;
            st.master_dirty = false;
        }
        keep = false;
    }
    STATE.with(|s| *s.borrow_mut() = st);
    keep
}

fn contents(ui: &mut egui::Ui, ctx: &egui::Context, cx: &mut EditorContext, st: &mut State) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut st.view, View::List, "Materials List");
        if ui
            .selectable_value(&mut st.view, View::Master, "Master List")
            .clicked()
            && st.master.is_none()
        {
            st.master = Some(lw::load_master_list());
        }
        ui.separator();
        if ui
            .button("By Surface\u{2026}")
            .on_hover_text("The take-off by surface and material (the Materials tools' window)")
            .clicked()
        {
            st.extras.open_by_surface = true;
        }
    });
    ui.separator();
    match st.view {
        View::List => table::list_view(ui, ctx, cx, st),
        View::Master => master::master_view(ui, cx, st),
    }
    if !st.status.is_empty() {
        ui.separator();
        ui.label(st.status.clone());
    }
}

/// The detail lines for the Properties of the cell tool tips (count tool tip).
pub fn count_tip(l: &ListLine) -> String {
    list::count_tooltip(l)
}
