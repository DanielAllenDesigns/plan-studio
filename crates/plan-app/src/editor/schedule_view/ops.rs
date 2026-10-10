//! The Edit toolbar commands of schedules and of the objects they list
//! (manual pp. 711 to 715, 92 to 93): Renumber Schedule, Move Row, Move Up /
//! Move Down in Schedule, Reset Column Widths, Align Left / Center / Right /
//! Justify, Open Row Object(s), Schedule to Text, Find Object in Plan, Find
//! Schedule(s) from Object and Create Schedule from Room. Each is one undo
//! step.

use super::{active_rooms, edit_floor, find, layout_rc, new_schedule, select, Layout};
use crate::editor::{
    rooms_edit, Camera, EditAction, EditActionKind, EditorContext, EditorRequest, ObjectRef,
};
use eframe::egui::{self, Color32, Pos2, Shape, Stroke};
use plan_core::geometry::Point;
use plan_core::schedules::{RoomRef, Schedule, ScheduleKind, ScheduleLayer, TextAlign};
use plan_core::{CadItem, Id, OpeningKind};
use plan_docs::schedule_kinds::{self, RowTarget};
use std::cell::{Cell, RefCell};

/// Edit command ids (`EditActionKind::Custom`).
pub mod cmd {
    pub const RENUMBER: &str = "schedule.renumber";
    pub const RESET_WIDTHS: &str = "schedule.reset_widths";
    pub const ALIGN_LEFT: &str = "schedule.align_left";
    pub const ALIGN_CENTER: &str = "schedule.align_center";
    pub const ALIGN_RIGHT: &str = "schedule.align_right";
    pub const ALIGN_JUSTIFY: &str = "schedule.align_justify";
    pub const OPEN_ROW: &str = "schedule.open_row";
    pub const TO_TEXT: &str = "schedule.to_text";
    pub const FIND_IN_PLAN: &str = "schedule.find_in_plan";
    pub const MOVE_UP: &str = "schedule.move_up";
    pub const MOVE_DOWN: &str = "schedule.move_down";
    pub const FIND_SCHEDULES: &str = "schedule.find_schedules";
    pub const FROM_ROOM: &str = "schedule.from_room";
    pub const MANAGE_CATEGORIES: &str = "schedule.manage_categories";
    pub const FIND_WALL: &str = "schedule.find_wall";
    pub const FIND_TRUSSES: &str = "schedule.find_trusses";
}

/// The command that opens the Schedule Defaults dialog of `kind` (Tools >
/// Schedules > Schedule Defaults).
pub fn defaults_command(kind: ScheduleKind) -> &'static str {
    match kind {
        ScheduleKind::Door => "schedule.defaults.door",
        ScheduleKind::Window => "schedule.defaults.window",
        ScheduleKind::Room => "schedule.defaults.room",
        ScheduleKind::Wall => "schedule.defaults.wall",
        ScheduleKind::Cabinet => "schedule.defaults.cabinet",
        ScheduleKind::Electrical => "schedule.defaults.electrical",
        ScheduleKind::Framing => "schedule.defaults.framing",
        ScheduleKind::Fixture => "schedule.defaults.fixture",
        ScheduleKind::Furniture => "schedule.defaults.furniture",
        ScheduleKind::Plant => "schedule.defaults.plant",
        ScheduleKind::Stair => "schedule.defaults.stair",
        ScheduleKind::RoomFinish => "schedule.defaults.room_finish",
        ScheduleKind::Note => "schedule.defaults.note",
        ScheduleKind::General => "schedule.defaults.general",
    }
}

thread_local! {
    /// The selected row: (floor, schedule id, row of the table).
    static ROW: Cell<Option<(usize, Id, usize)>> = const { Cell::new(None) };
    /// Where Find Object in Plan wants the view centred next.
    static FOCUS: Cell<Option<Point>> = const { Cell::new(None) };
    /// A Move Row that waits for the answer to the Automatic Sorting prompt.
    static PROMPT: RefCell<Option<SortPrompt>> = const { RefCell::new(None) };
}

/// A row move that needs Automatic Sorting turned off first.
#[derive(Debug, Clone, PartialEq)]
pub struct SortPrompt {
    pub floor: usize,
    pub id: Id,
    pub from: usize,
    pub to: usize,
}

/// Selects row `row` of schedule `id` on `floor` (a click on its cell).
pub fn select_row(floor: usize, id: Id, row: usize) {
    ROW.with(|r| r.set(Some((floor, id, row))));
}

/// A click at `p` on schedule `id`: selects the row it falls on (or none).
pub fn note_click(cx: &EditorContext, id: Id, p: Point) {
    match row_at_point(cx, id, p) {
        Some(row) => select_row(cx.floor, id, row),
        None => clear_row(),
    }
}

/// Forgets the selected row.
pub fn clear_row() {
    ROW.with(|r| r.set(None));
}

/// The selected row of schedule `id` on `floor`.
pub fn selected_row_of(id: Id, floor: usize) -> Option<usize> {
    ROW.with(|r| r.get())
        .filter(|(f, i, _)| *f == floor && *i == id)
        .map(|(_, _, row)| row)
}

/// The point the view should centre on after a Find in Plan, once.
pub fn take_focus() -> Option<Point> {
    FOCUS.with(|f| f.take())
}

/// The Automatic Sorting prompt that waits for an answer.
pub fn pending_sort_prompt() -> Option<SortPrompt> {
    PROMPT.with(|p| p.borrow().clone())
}

/// Answers the Automatic Sorting prompt: `true` turns the sorting off and
/// moves the row, `false` leaves everything as it was.
pub fn answer_sort_prompt(cx: &mut EditorContext, turn_off: bool) {
    let Some(p) = PROMPT.with(|p| p.borrow_mut().take()) else {
        return;
    };
    if !turn_off {
        cx.status = "The row stays where it was".into();
        return;
    }
    let Some(mut def) = def_on(cx, p.floor, p.id) else {
        return;
    };
    def.sort.field.clear();
    do_move_row(cx, p.floor, def, p.from, p.to);
}

fn def_on(cx: &EditorContext, floor: usize, id: Id) -> Option<Schedule> {
    ScheduleLayer::load(cx.project.floors.get(floor)?)
        .find(id)
        .cloned()
}

fn rooms_for(cx: &EditorContext, floor: usize) -> plan_docs::schedule_kinds::ActiveRooms<'_> {
    active_rooms(cx, floor)
}

// ===================================================================
// Renumber and move rows
// ===================================================================

/// Renumber Schedule: closes the gaps in the numbers of schedule `id`, the
/// order of its rows unchanged (p. 715). Returns whether a number changed.
pub fn renumber_schedule(cx: &mut EditorContext, floor: usize, id: Id) -> bool {
    let Some(def) = def_on(cx, floor, id) else {
        return false;
    };
    let numbers = schedule_kinds::renumbered(&cx.project, &def, rooms_for(cx, floor));
    if numbers == def.numbers {
        cx.status = "The schedule numbers have no gaps".into();
        return false;
    }
    edit_floor(cx, floor, "Renumber Schedule", |l| {
        if let Some(s) = l.find_mut(id) {
            s.numbers = numbers;
        }
    });
    cx.status = "Renumbered the schedule".into();
    true
}

/// Renumber Schedule for every schedule of the given kinds on every floor,
/// as one undo step. Returns how many schedules changed.
pub fn renumber_kinds(cx: &mut EditorContext, kinds: &[ScheduleKind]) -> usize {
    let mut todo: Vec<(usize, Id, Vec<plan_core::schedules::NumRec>)> = Vec::new();
    for fi in 0..cx.project.floors.len() {
        let layer = ScheduleLayer::load(&cx.project.floors[fi]);
        for s in layer.schedules.iter().filter(|s| kinds.contains(&s.kind)) {
            let numbers = schedule_kinds::renumbered(&cx.project, s, rooms_for(cx, fi));
            if numbers != s.numbers {
                todo.push((fi, s.id, numbers));
            }
        }
    }
    if todo.is_empty() {
        return 0;
    }
    cx.begin_change("Renumber Schedule");
    let n = todo.len();
    for (fi, id, numbers) in todo {
        let mut layer = ScheduleLayer::load(&cx.project.floors[fi]);
        if let Some(s) = layer.find_mut(id) {
            s.numbers = numbers;
        }
        super::save(&mut cx.project, fi, &layer);
    }
    cx.mark_dirty();
    n
}

/// Whether a row move has to ask first: the rows are sorted by a column.
pub fn needs_sort_prompt(def: &Schedule) -> bool {
    !def.sort.field.is_empty()
}

/// Moves row `from` of schedule `id` to the place of row `to` (the Move Row
/// handle). When Automatic Sorting is on, a prompt asks first and `false`
/// comes back; the move runs when it is answered.
pub fn move_row(cx: &mut EditorContext, floor: usize, id: Id, from: usize, to: usize) -> bool {
    let Some(def) = def_on(cx, floor, id) else {
        return false;
    };
    if from == to {
        return false;
    }
    if needs_sort_prompt(&def) {
        PROMPT.with(|p| {
            *p.borrow_mut() = Some(SortPrompt {
                floor,
                id,
                from,
                to,
            })
        });
        cx.status = "Automatic Sorting is on: turn it off to move rows by hand".into();
        return false;
    }
    do_move_row(cx, floor, def, from, to)
}

fn do_move_row(
    cx: &mut EditorContext,
    floor: usize,
    def: Schedule,
    from: usize,
    to: usize,
) -> bool {
    let id = def.id;
    let Some(numbers) =
        schedule_kinds::moved_numbers(&cx.project, &def, floor, rooms_for(cx, floor), from, to)
    else {
        cx.status = "That row cannot be moved there".into();
        return false;
    };
    let mut def = def;
    def.numbers = numbers;
    // The numbers follow the new order, so grouping would undo the move.
    super::replace_as(cx, floor, def, "Move Schedule Row");
    select_row(floor, id, to);
    true
}

/// Move Up / Move Down in Schedule for the selected object: when the object
/// is in exactly one schedule its row moves one place.
pub fn move_object_in_schedule(cx: &mut EditorContext, up: bool) -> bool {
    let Some(ident) = selected_ident(cx) else {
        return false;
    };
    let listing = schedules_listing(cx, &ident);
    let [one] = listing.as_slice() else {
        cx.status = if listing.is_empty() {
            "The object is not in a schedule".into()
        } else {
            "The object is in more than one schedule".into()
        };
        return false;
    };
    let Some(def) = def_on(cx, one.floor, one.id) else {
        return false;
    };
    let Some(row) = one.row else {
        return false;
    };
    let rows = schedule_kinds::rows(&cx.project, &def, one.floor, rooms_for(cx, one.floor));
    let to = if up {
        row.checked_sub(1)
    } else {
        (row + 1 < rows.len()).then_some(row + 1)
    };
    let Some(to) = to else {
        cx.status = "The object is already at the end of the schedule".into();
        return false;
    };
    move_row(cx, one.floor, one.id, row, to)
}

// ===================================================================
// Column tools
// ===================================================================

/// Reset Column Widths: every column as wide as its widest text, no wrapping.
pub fn reset_column_widths(cx: &mut EditorContext, floor: usize, id: Id) -> bool {
    let Some(def) = def_on(cx, floor, id) else {
        return false;
    };
    if def.columns.iter().all(|c| c.width == 0.0) {
        cx.status = "The columns already fit their text".into();
        return false;
    }
    edit_floor(cx, floor, "Reset Column Widths", |l| {
        if let Some(s) = l.find_mut(id) {
            s.columns.iter_mut().for_each(|c| c.width = 0.0);
        }
    });
    true
}

/// Align Left / Center / Right / Justify: the contents of every column.
pub fn align_columns(cx: &mut EditorContext, floor: usize, id: Id, a: TextAlign) -> bool {
    let label = format!("Align {}", a.name());
    if def_on(cx, floor, id).is_none() {
        return false;
    }
    edit_floor(cx, floor, &label, |l| {
        if let Some(s) = l.find_mut(id) {
            s.h_align = a;
            s.columns.iter_mut().for_each(|c| c.align = None);
        }
    });
    true
}

// ===================================================================
// Schedule to text
// ===================================================================

/// Schedule to Text: the schedule becomes a tabbed Text object at the same
/// place and stops updating. One undo step.
pub fn schedule_to_text(cx: &mut EditorContext, floor: usize, id: Id) -> bool {
    let Some(def) = def_on(cx, floor, id) else {
        return false;
    };
    let table = super::table_for(cx, &def, floor);
    let text = table.to_tsv(def.show_title, def.show_headings);
    let h = super::text_height(&cx.project, &def);
    cx.begin_change("Schedule to Text");
    let mut layer = ScheduleLayer::load(&cx.project.floors[floor]);
    layer.remove(id);
    super::save(&mut cx.project, floor, &layer);
    let at = def.position;
    cx.project.add_cad(
        floor,
        "CAD, Default",
        CadItem::Text {
            pos: at,
            text,
            height: h,
            angle: def.angle.to_radians(),
        },
    );
    cx.selection.items.retain(|o| *o != ObjectRef::Schedule(id));
    cx.mark_dirty();
    cx.status = "The schedule is text now and no longer updates".into();
    true
}

// ===================================================================
// Finding objects
// ===================================================================

/// The object a row target stands for, if it can be selected.
pub fn object_of(cx: &EditorContext, t: &RowTarget) -> Option<ObjectRef> {
    use ScheduleKind as K;
    if t.id == 0 && !matches!(t.kind, K::Room | K::RoomFinish) {
        return None;
    }
    let o = match t.kind {
        K::Door | K::Window => ObjectRef::Opening(t.id),
        K::Wall => ObjectRef::Wall(t.id),
        K::Cabinet => ObjectRef::Cabinet(t.id),
        K::Electrical => ObjectRef::Device(t.id),
        K::Fixture | K::Furniture | K::Plant => ObjectRef::Symbol(t.id),
        K::Note => ObjectRef::Cad(t.id),
        K::Stair => ObjectRef::Stair(t.id),
        K::Framing => ObjectRef::Framing(t.id),
        K::Room | K::RoomFinish | K::General => return None,
    };
    o.exists_in(&cx.project, t.floor).then_some(o)
}

/// Find Object in Plan: goes to the floor of the objects behind `targets`,
/// selects them (a room is selected as a room) and asks the view to centre on
/// them. Returns how many objects are selected.
pub fn find_in_plan(cx: &mut EditorContext, targets: &[RowTarget]) -> usize {
    let Some(first) = targets.iter().find(|t| t.floor < cx.project.floors.len()) else {
        cx.status = "That row has no object to find in the plan".into();
        return 0;
    };
    let floor = first.floor;
    if floor != cx.floor {
        cx.floor = floor;
        cx.reset_view_state();
    }
    cx.refresh();
    rooms_edit::clear_room_selection();
    cx.selection.clear();
    let mut n = 0;
    let mut sum = Point::ZERO;
    let mut room: Option<usize> = None;
    for t in targets.iter().filter(|t| t.floor == floor) {
        if let Some(o) = object_of(cx, t) {
            cx.selection.add(o);
            n += 1;
            sum = sum.add(t.position);
        } else if matches!(t.kind, ScheduleKind::Room | ScheduleKind::RoomFinish) && room.is_none()
        {
            if let Some(i) = rooms_edit::room_index_at(cx, t.position) {
                room = Some(i);
                sum = sum.add(t.position);
                n += 1;
            }
        }
    }
    if let Some(i) = room {
        if n == 1 {
            rooms_edit::select_room(cx, i);
        }
    }
    if n == 0 {
        cx.status = "That row has no object to find in the plan".into();
        return 0;
    }
    let centre = sum.scale(1.0 / n as f64);
    FOCUS.with(|f| f.set(Some(centre)));
    cx.status = format!("Found {n} object(s) in the plan");
    n
}

/// The objects behind row `row` of schedule `id` on `floor`.
pub fn row_targets_of(cx: &EditorContext, floor: usize, id: Id, row: usize) -> Vec<RowTarget> {
    def_on(cx, floor, id)
        .and_then(|def| {
            schedule_kinds::row_targets(&cx.project, &def, floor, rooms_for(cx, floor))
                .into_iter()
                .nth(row)
        })
        .unwrap_or_default()
}

/// Find Object in Plan from the selected row of the selected schedule.
pub fn find_selected_row_in_plan(cx: &mut EditorContext) -> usize {
    let Some(id) = super::selected(cx) else {
        return 0;
    };
    let Some(row) = selected_row_of(id, cx.floor) else {
        cx.status = "Click a row of the schedule first".into();
        return 0;
    };
    let t = row_targets_of(cx, cx.floor, id, row);
    find_in_plan(cx, &t)
}

/// Open Row Object(s): selects the objects of the selected row and asks for
/// the specification of the first. False when the row has no object.
pub fn open_row_objects(cx: &mut EditorContext, floor: usize, id: Id, row: usize) -> bool {
    let t = row_targets_of(cx, floor, id, row);
    let objects: Vec<ObjectRef> = t
        .iter()
        .filter(|x| x.floor == floor)
        .filter_map(|x| object_of(cx, x))
        .collect();
    let Some(first) = objects.first().copied() else {
        // Rooms of a Room schedule open through the room selection.
        if let Some(r) = t
            .first()
            .filter(|x| matches!(x.kind, ScheduleKind::Room | ScheduleKind::RoomFinish))
        {
            if let Some(i) = rooms_edit::room_index_at(cx, r.position) {
                cx.selection.clear();
                rooms_edit::select_room(cx, i);
                cx.requests
                    .push(EditorRequest::OpenSpec(ObjectRef::Room(i)));
                return true;
            }
        }
        cx.status = "That row has no object to open".into();
        return false;
    };
    // Identical rooms on different floors share a row: nothing to open.
    if t.iter().any(|x| x.floor != floor) {
        cx.status = "That row stands for objects on several floors".into();
        return false;
    }
    cx.selection.clear();
    for o in &objects {
        cx.selection.add(*o);
    }
    cx.requests.push(EditorRequest::OpenSpec(first));
    true
}

// ===================================================================
// From an object to its schedules
// ===================================================================

/// An object that a schedule can list.
#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    pub kind: ScheduleKind,
    pub floor: usize,
    pub id: Id,
    pub position: Option<Point>,
}

/// The schedule kind and id of a selectable object on the active floor.
pub fn ident_of(cx: &EditorContext, o: ObjectRef) -> Option<Ident> {
    let floor = cx.floor;
    let f = cx.floor();
    let (kind, id, position) = match o {
        ObjectRef::Opening(id) => {
            let op = f.openings.iter().find(|x| x.id == id)?;
            (
                match op.kind {
                    OpeningKind::Door => ScheduleKind::Door,
                    OpeningKind::Window => ScheduleKind::Window,
                },
                id,
                None,
            )
        }
        ObjectRef::Wall(id) => (ScheduleKind::Wall, id, None),
        ObjectRef::Cabinet(id) => (ScheduleKind::Cabinet, id, None),
        ObjectRef::Device(id) => (ScheduleKind::Electrical, id, None),
        ObjectRef::Stair(id) => (ScheduleKind::Stair, id, None),
        ObjectRef::Symbol(id) => {
            let s = f.symbols.iter().find(|s| s.id == id)?;
            (schedule_kinds::symbol_kind(s)?, id, None)
        }
        ObjectRef::Cad(id) => (ScheduleKind::Note, id, None),
        ObjectRef::Room(i) => (ScheduleKind::Room, 0, Some(cx.rooms.get(i)?.centroid)),
        _ => return None,
    };
    Some(Ident {
        kind,
        floor,
        id,
        position,
    })
}

fn selected_ident(cx: &EditorContext) -> Option<Ident> {
    ident_of(cx, cx.selection.single()?)
}

/// A schedule that lists an object.
#[derive(Debug, Clone, PartialEq)]
pub struct Listing {
    pub floor: usize,
    pub id: Id,
    pub title: String,
    pub kind: ScheduleKind,
    /// The row of the table that holds the object.
    pub row: Option<usize>,
    /// How many rows the schedule has.
    pub rows: usize,
}

/// Every schedule of the plan that lists `ident` (Find Schedule(s) from
/// Object).
pub fn schedules_listing(cx: &EditorContext, ident: &Ident) -> Vec<Listing> {
    let mut out = Vec::new();
    for (fi, f) in cx.project.floors.iter().enumerate() {
        for def in ScheduleLayer::load(f).schedules {
            let rooms = rooms_for(cx, fi);
            let rows = schedule_kinds::rows(&cx.project, &def, fi, rooms);
            let hit = rows.iter().position(|e| {
                let kind_ok = e.kind == ident.kind
                    || (ident.kind == ScheduleKind::Room && e.kind == ScheduleKind::RoomFinish);
                kind_ok
                    && e.floor == ident.floor
                    && match ident.position {
                        Some(p) if e.id == 0 => e.position.dist(p) < 1.0,
                        _ => e.id == ident.id,
                    }
            });
            if hit.is_some() {
                out.push(Listing {
                    floor: fi,
                    id: def.id,
                    title: def.display_title(),
                    kind: def.kind,
                    row: hit,
                    rows: rows.len(),
                });
            }
        }
    }
    out
}

/// Goes to the floor of schedule `id` and selects it.
pub fn show_schedule(cx: &mut EditorContext, floor: usize, id: Id, row: Option<usize>) {
    if floor != cx.floor {
        cx.floor = floor;
        cx.reset_view_state();
        cx.refresh();
    }
    select(cx, id);
    if let Some(r) = row {
        select_row(floor, id, r);
    }
    if let Some(def) = find(cx, id) {
        let l = layout_rc(cx, &def, floor);
        let (lo, hi) = l.bounds(def.position);
        FOCUS.with(|f| f.set(Some(Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))));
    }
    cx.status = "Found the schedule".into();
}

/// Find Schedule(s) from Object: the schedule is selected when there is one,
/// the Select Location dialog asks which when there are several. Returns how
/// many schedules list the object.
pub fn find_schedules_from_selection(cx: &mut EditorContext) -> usize {
    let Some(ident) = selected_ident(cx) else {
        return 0;
    };
    let list = schedules_listing(cx, &ident);
    match list.as_slice() {
        [] => {
            cx.status = "No schedule lists this object".into();
        }
        [one] => show_schedule(cx, one.floor, one.id, one.row),
        _ => crate::dialogs::select_location::ask(list.clone()),
    }
    list.len()
}

/// Find Wall from a framing member (a stud, plate or header of a wall
/// detail or the framing overview): goes to the floor that holds `wall` and
/// selects it. False when no floor has that wall.
pub fn find_wall(cx: &mut EditorContext, wall: Id) -> bool {
    let Some(fi) = cx
        .project
        .floors
        .iter()
        .position(|f| f.wall(wall).is_some())
    else {
        cx.status = "That member does not frame a wall".into();
        return false;
    };
    if fi != cx.floor {
        cx.floor = fi;
        cx.reset_view_state();
        cx.refresh();
    }
    cx.selection.set(ObjectRef::Wall(wall));
    if let Some(w) = cx.floor().wall(wall) {
        let mid = w.start.add(w.end).scale(0.5);
        FOCUS.with(|f| f.set(Some(mid)));
    }
    cx.status = "Selected the wall the member frames".into();
    true
}

/// The truss records of the active floor: `(id, truss detail)`, the detail
/// being the truss specification the manual truss was drawn from.
fn truss_records(cx: &EditorContext) -> Vec<(Id, serde_json::Value)> {
    cx.floor()
        .framing
        .iter()
        .filter_map(|v| {
            let m = v
                .as_object()
                .filter(|o| o.len() == 1)
                .and_then(|o| o.get("Manual").or_else(|| o.get("Built")))?;
            let id = m.get("id")?.as_u64()?;
            let truss = m.get("truss").filter(|t| !t.is_null())?.clone();
            Some((id, truss))
        })
        .collect()
}

/// Find Trusses from a truss detail: selects every truss of the active floor
/// built to the same detail as truss `like`. Returns how many.
pub fn find_trusses(cx: &mut EditorContext, like: Id) -> usize {
    let all = truss_records(cx);
    let Some((_, detail)) = all.iter().find(|(id, _)| *id == like) else {
        cx.status = "That is not a truss".into();
        return 0;
    };
    let ids: Vec<Id> = all
        .iter()
        .filter(|(_, d)| d == detail)
        .map(|(id, _)| *id)
        .collect();
    cx.selection.clear();
    for id in &ids {
        cx.selection.add(ObjectRef::Framing(*id));
    }
    cx.status = format!("Selected {} truss(es) with the same detail", ids.len());
    ids.len()
}

// ===================================================================
// Create Schedule from Room
// ===================================================================

/// Create Schedule from Room: a schedule of `kind` limited to the room
/// `room` of the active floor (Include Objects from Room), placed with its
/// upper-left corner at `at`. One undo step. Returns its id.
pub fn create_from_room(
    cx: &mut EditorContext,
    kind: ScheduleKind,
    room: usize,
    at: Point,
) -> Option<Id> {
    let r = cx.rooms.get(room)?;
    let rr = RoomRef::at(cx.floor, r.centroid);
    let name = r
        .name_entry(&cx.floor().room_names)
        .map(|n| n.name.clone())
        .unwrap_or_default();
    let mut s = new_schedule(cx, kind, at);
    s.rooms = vec![rr];
    s.floors.clear();
    s.floor_scope = plan_core::schedules::FloorScope::ThisFloor;
    if !name.is_empty() {
        s.title = format!("{} {}", name, kind.title());
    }
    let rooms = rooms_for(cx, cx.floor);
    s.numbers = schedule_kinds::snapshot_numbers(&cx.project, &s, rooms);
    let id = s.id;
    let fl = cx.floor;
    edit_floor(cx, fl, "Create Schedule from Room", |l| {
        l.add(s);
    });
    select(cx, id);
    cx.status = "Schedule created from the room".into();
    Some(id)
}

// ===================================================================
// The Edit toolbar
// ===================================================================

fn action(id: &'static str, label: &'static str, enabled: bool) -> EditAction {
    EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled,
    }
}

/// The Edit toolbar buttons for the selection: the schedule tools when a
/// schedule is selected, Find Schedule(s) and Move Up / Down in Schedule for
/// an object a schedule can list, Create Schedule from Room for a room.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    let Some(one) = cx.selection.single() else {
        return v;
    };
    if let ObjectRef::Schedule(id) = one {
        let row = selected_row_of(id, cx.floor);
        v.push(action(cmd::RENUMBER, "Renumber Schedule", true));
        v.push(action(cmd::RESET_WIDTHS, "Reset Column Widths", true));
        v.push(action(cmd::ALIGN_LEFT, "Align Left", true));
        v.push(action(cmd::ALIGN_CENTER, "Center", true));
        v.push(action(cmd::ALIGN_RIGHT, "Align Right", true));
        v.push(action(cmd::ALIGN_JUSTIFY, "Justify", true));
        v.push(action(
            cmd::FIND_IN_PLAN,
            "Find Object in Plan",
            row.is_some(),
        ));
        v.push(action(cmd::OPEN_ROW, "Open Row Object(s)", row.is_some()));
        v.push(action(cmd::TO_TEXT, "Schedule to Text", true));
        return v;
    }
    if let Some(ident) = ident_of(cx, one) {
        let listing = schedules_listing(cx, &ident);
        v.push(action(
            cmd::FIND_SCHEDULES,
            "Find Schedule(s)",
            !listing.is_empty(),
        ));
        if listing.len() == 1 {
            v.push(action(cmd::MOVE_UP, "Move Up in Schedule", true));
            v.push(action(cmd::MOVE_DOWN, "Move Down in Schedule", true));
        }
    }
    if let ObjectRef::Framing(id) = one {
        if truss_records(cx).iter().any(|(t, _)| *t == id) {
            v.push(action(cmd::FIND_TRUSSES, "Find Trusses", true));
        }
    }
    v
}

/// Runs a schedule command; false when `id` is not one of them.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    let floor = cx.floor;
    let sel = super::selected(cx);
    match id {
        cmd::RENUMBER => {
            if let Some(s) = sel {
                renumber_schedule(cx, floor, s);
            }
        }
        cmd::RESET_WIDTHS => {
            if let Some(s) = sel {
                reset_column_widths(cx, floor, s);
            }
        }
        cmd::ALIGN_LEFT | cmd::ALIGN_CENTER | cmd::ALIGN_RIGHT | cmd::ALIGN_JUSTIFY => {
            let a = match id {
                cmd::ALIGN_LEFT => TextAlign::Left,
                cmd::ALIGN_CENTER => TextAlign::Center,
                cmd::ALIGN_RIGHT => TextAlign::Right,
                _ => TextAlign::Justify,
            };
            if let Some(s) = sel {
                align_columns(cx, floor, s, a);
            }
        }
        cmd::FIND_IN_PLAN => {
            find_selected_row_in_plan(cx);
        }
        cmd::OPEN_ROW => {
            if let Some(s) = sel {
                match selected_row_of(s, floor) {
                    Some(row) => {
                        open_row_objects(cx, floor, s, row);
                    }
                    None => cx.status = "Click a row of the schedule first".into(),
                }
            }
        }
        cmd::TO_TEXT => {
            if let Some(s) = sel {
                schedule_to_text(cx, floor, s);
            }
        }
        cmd::MOVE_UP => {
            move_object_in_schedule(cx, true);
        }
        cmd::MOVE_DOWN => {
            move_object_in_schedule(cx, false);
        }
        cmd::FIND_SCHEDULES => {
            find_schedules_from_selection(cx);
        }
        cmd::FIND_TRUSSES => {
            if let Some(ObjectRef::Framing(id)) = cx.selection.single() {
                find_trusses(cx, id);
            }
        }
        cmd::FROM_ROOM => {
            if let Some(ObjectRef::Room(i)) = cx.selection.single() {
                crate::dialogs::schedule_spec::ask_schedule_type(i);
            }
        }
        cmd::MANAGE_CATEGORIES => crate::dialogs::schedule_categories::open(),
        _ => {
            let Some(slug) = id.strip_prefix("schedule.defaults.") else {
                return false;
            };
            let Some(kind) = crate::dialogs::schedule_spec::kind_from_slug(slug) else {
                return false;
            };
            crate::dialogs::schedule_spec::request_defaults(kind);
        }
    }
    true
}

// ===================================================================
// Drawing helpers
// ===================================================================

/// Highlights the selected row of a schedule.
pub fn draw_row_highlight(
    painter: &egui::Painter,
    cam: &Camera,
    s: &Schedule,
    l: &Layout,
    row: usize,
    color: Color32,
) {
    let f = l.frame(s.position);
    for piece in &l.pieces {
        if piece.swapped {
            // A swapped schedule's rows are its columns.
            if row >= piece.first && row < piece.first + piece.count {
                let c = piece.head_cols + row - piece.first;
                let (x, y) = piece.cell_origin(0, c);
                let (w, h) = (piece.col_w[c], piece.row_h.iter().sum::<f64>());
                paint_quad(painter, cam, &f, (x, y, x + w, y + h), color);
            }
        } else if row >= piece.first && row < piece.first + piece.count {
            let r = piece.head_rows + row - piece.first;
            let (x, y) = piece.cell_origin(r, 0);
            paint_quad(
                painter,
                cam,
                &f,
                (
                    x,
                    y,
                    x + piece.col_w.iter().sum::<f64>(),
                    y + piece.row_h[r],
                ),
                color,
            );
        }
    }
}

fn paint_quad(
    painter: &egui::Painter,
    cam: &Camera,
    f: &super::Frame,
    r: (f64, f64, f64, f64),
    color: Color32,
) {
    let pts: Vec<Pos2> = [(r.0, r.1), (r.2, r.1), (r.2, r.3), (r.0, r.3)]
        .iter()
        .map(|(x, y)| cam.world_to_screen(f.to_plan(*x, *y)))
        .collect();
    painter.add(Shape::convex_polygon(
        pts.clone(),
        color.gamma_multiply(0.18),
        Stroke::new(1.5_f32, color),
    ));
}

/// Which row (object index) a click at plan point `p` falls on, if any.
pub fn row_at_point(cx: &EditorContext, id: Id, p: Point) -> Option<usize> {
    let def = find(cx, id)?;
    let l = layout_rc(cx, &def, cx.floor);
    let (x, y) = l.frame(def.position).to_local(p);
    if l.swapped {
        // Objects are columns.
        for piece in &l.pieces {
            if y < piece.y || y > piece.y + piece.h {
                continue;
            }
            let mut left = piece.x;
            for (c, w) in piece.col_w.iter().enumerate() {
                if c >= piece.head_cols && x >= left && x < left + w {
                    return Some(piece.first + c - piece.head_cols);
                }
                left += w;
            }
        }
        None
    } else {
        for piece in &l.pieces {
            if x < piece.x || x > piece.x + piece.w {
                continue;
            }
            let mut top = piece.y + piece.title_h;
            for (r, rh) in piece.row_h.iter().enumerate() {
                if r >= piece.head_rows && y >= top && y < top + rh {
                    return Some(piece.first + r - piece.head_rows);
                }
                top += rh;
            }
        }
        None
    }
}
