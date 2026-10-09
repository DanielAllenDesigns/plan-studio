//! Scenario 88 (round 16, brief 04): schedules at construction-document
//! quality (L-23, L-27, L-28, L-57..L-75, L-233..L-235, S-118..S-120). A small
//! house gets door, window and room finish schedules with Area columns and a
//! Totals row; a long schedule wraps at a maximum table size; Open Row
//! Object(s) and Find Object in Plan work from a row; Renumber Schedule
//! closes gaps only; the Totals row survives the Excel export; each edit is
//! one undo step.

use super::Sim;
use crate::dialogs::{schedule_categories, select_location};
use crate::editor::schedule_view::{self as sv, cmd, HandleKind};
use crate::editor::{EditorRequest, ObjectRef};
use crate::tools::schedule as schedule_tool;
use crate::tools::ToolId;
use plan_core::geometry::Point;
use plan_core::schedules::{ScheduleKind, TextAlign, WrapBy};
use plan_core::{Id, OpeningKind, WallKind};

/// A 40 x 20 foot shell with a partition at x = 240 (two rooms), three doors
/// and three windows. Returns the door ids and the window ids, in order of
/// placement.
fn house(sim: &mut Sim) -> (Vec<Id>, Vec<Id>) {
    let p = &mut sim.app.cx.project;
    let c = [
        Point::new(0.0, 0.0),
        Point::new(480.0, 0.0),
        Point::new(480.0, 240.0),
        Point::new(0.0, 240.0),
    ];
    let mut walls = Vec::new();
    for i in 0..4 {
        walls.push(p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior));
    }
    let part = p.add_wall(
        0,
        Point::new(240.0, 0.0),
        Point::new(240.0, 240.0),
        4.5,
        109.125,
        WallKind::Interior,
    );
    let doors = vec![
        p.add_opening(0, walls[0], 100.0, OpeningKind::Door)
            .unwrap(),
        p.add_opening(0, walls[0], 360.0, OpeningKind::Door)
            .unwrap(),
        p.add_opening(0, part, 120.0, OpeningKind::Door).unwrap(),
    ];
    let windows = vec![
        p.add_opening(0, walls[2], 100.0, OpeningKind::Window)
            .unwrap(),
        p.add_opening(0, walls[2], 360.0, OpeningKind::Window)
            .unwrap(),
        p.add_opening(0, walls[3], 120.0, OpeningKind::Window)
            .unwrap(),
    ];
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    (doors, windows)
}

/// Places a schedule of `kind` with the tool, far below the plan.
fn place(sim: &mut Sim, kind: ScheduleKind, x: f64) -> Id {
    sim.tool(ToolId::ScheduleVariant(kind));
    let before = sv::load(&sim.app.cx).schedules.len();
    sim.click(x, -400.0);
    let layer = sv::load(&sim.app.cx);
    assert_eq!(layer.schedules.len(), before + 1, "one schedule is placed");
    layer.schedules.last().unwrap().id
}

fn table(sim: &Sim, id: Id) -> plan_docs::Schedule {
    let d = sv::find(&sim.app.cx, id).unwrap();
    sv::table_for(&sim.app.cx, &d, 0)
}

fn edit(sim: &mut Sim, id: Id, f: impl FnOnce(&mut plan_core::schedules::Schedule)) {
    let mut d = sv::find(&sim.app.cx, id).unwrap();
    f(&mut d);
    assert!(sv::replace(&mut sim.app.cx, 0, d));
}

fn show(d: &mut plan_core::schedules::Schedule, fields: &[&str]) {
    for f in fields {
        d.columns
            .iter_mut()
            .find(|c| c.field == *f)
            .unwrap()
            .visible = true;
    }
}

#[test]
fn door_window_and_room_finish_schedules_total_their_areas() {
    let mut sim = Sim::new();
    let (doors, _) = house(&mut sim);
    let door = place(&mut sim, ScheduleKind::Door, 0.0);
    assert_eq!(sim.app.cx.undo_label(), Some("Place Door Schedule"));
    let t = table(&sim, door);
    assert_eq!(
        t.rows.len(),
        3,
        "three doors, no Totals row until Area shows"
    );
    edit(&mut sim, door, |d| show(d, &["area"]));
    let t = table(&sim, door);
    assert_eq!(t.rows.len(), 4, "the doors and the Totals row");
    let area = t.columns.iter().position(|c| c == "Area").unwrap();
    let sum: f64 = (0..3)
        .map(|r| t.rows[r][area].parse::<f64>().unwrap())
        .sum();
    let total: f64 = t.rows[3][area].parse().unwrap();
    assert!((sum - total).abs() < 0.2, "{sum} vs {total}");
    assert_eq!(t.rows[3][0], "Totals");
    assert_eq!(sim.app.cx.undo_label(), Some("Schedule Specification"));
    assert_eq!(doors.len(), 3);

    let win = place(&mut sim, ScheduleKind::Window, 300.0);
    edit(&mut sim, win, |d| show(d, &["area"]));
    assert_eq!(table(&sim, win).rows.len(), 4);

    let rf = place(&mut sim, ScheduleKind::RoomFinish, 600.0);
    edit(&mut sim, rf, |d| show(d, &["area", "volume"]));
    let t = table(&sim, rf);
    assert_eq!(t.rows.len(), 3, "two rooms and the Totals row: {t:?}");
    let a = t
        .columns
        .iter()
        .position(|c| c.starts_with("Area"))
        .unwrap();
    let v = t.columns.iter().position(|c| c == "Volume").unwrap();
    let rooms: f64 = (0..2).map(|r| t.rows[r][a].parse::<f64>().unwrap()).sum();
    assert!((rooms - t.rows[2][a].parse::<f64>().unwrap()).abs() < 0.2);
    assert!(t.rows[2][v].parse::<f64>().unwrap() > rooms * 8.0);
}

#[test]
fn a_schedule_wraps_at_a_maximum_table_size() {
    let mut sim = Sim::new();
    house(&mut sim);
    let id = place(&mut sim, ScheduleKind::Wall, 0.0);
    let natural = sv::layout_of(&sim.app.cx, &sv::find(&sim.app.cx, id).unwrap(), 0);
    assert_eq!(natural.pieces.len(), 1);
    let row_h = natural.row_h;
    // Room for the title, the headings and three rows.
    let max = natural.title_h + row_h * 4.0 + 0.5;
    edit(&mut sim, id, |d| {
        d.wrap.enabled = true;
        d.wrap.by = WrapBy::MaxSize(max);
    });
    let d = sv::find(&sim.app.cx, id).unwrap();
    let l = sv::layout_of(&sim.app.cx, &d, 0);
    assert_eq!(l.table.rows.len(), 5, "five walls");
    assert_eq!(l.pieces.len(), 2, "three rows, then two");
    assert_eq!(l.pieces[0].count, 3);
    assert_eq!(l.pieces[1].count, 2);
    assert!(l.pieces[0].h <= max + 1e-6);
    // Side by side, with the offset between them.
    let gap = l.pieces[1].x - (l.pieces[0].x + l.pieces[0].w);
    assert!((gap - d.wrap.offset).abs() < 1e-6);
    // Swapped, the tables stack.
    edit(&mut sim, id, |d| {
        d.swap = true;
        d.wrap.by = WrapBy::Entries(2);
    });
    let d = sv::find(&sim.app.cx, id).unwrap();
    let l = sv::layout_of(&sim.app.cx, &d, 0);
    assert_eq!(l.pieces.len(), 3, "2 + 2 + 1 walls");
    assert!(l.pieces[1].y > l.pieces[0].y + l.pieces[0].h - 1e-6);
    assert!(l.swapped);
    // The title shows on the first table only when asked.
    edit(&mut sim, id, |d| d.wrap.title_each = false);
    let d = sv::find(&sim.app.cx, id).unwrap();
    let l = sv::layout_of(&sim.app.cx, &d, 0);
    assert!(l.pieces[0].title && !l.pieces[1].title);
}

#[test]
fn open_row_objects_selects_the_objects_and_asks_for_their_dialog() {
    let mut sim = Sim::new();
    let (doors, _) = house(&mut sim);
    let id = place(&mut sim, ScheduleKind::Door, 0.0);
    sv::select(&mut sim.app.cx, id);
    // No row picked yet: the command says so and does nothing.
    sim.app.cx.requests.clear();
    sim.action(crate::toolbar::Action::Custom(cmd::OPEN_ROW));
    assert!(sim.app.cx.status.contains("row"), "{}", sim.app.cx.status);
    // Click the second row of the table with the Schedule tool.
    let d = sv::find(&sim.app.cx, id).unwrap();
    let l = sv::layout_of(&sim.app.cx, &d, 0);
    let p0 = &l.pieces[0];
    let (x, y) = p0.cell_origin(2, 1);
    let at = l.frame(d.position).to_plan(x + 1.0, y + 1.0);
    sim.tool(ToolId::ScheduleVariant(ScheduleKind::Door));
    sim.click(at.x, at.y);
    assert_eq!(sv::selected_row_of(id, 0), Some(1));
    assert!(sv::open_row_objects(&mut sim.app.cx, 0, id, 1));
    let objs: Vec<ObjectRef> = sim.app.cx.selection.items.clone();
    assert_eq!(objs.len(), 1);
    assert!(matches!(objs[0], ObjectRef::Opening(i) if doors.contains(&i)));
    assert!(sim
        .app
        .cx
        .requests
        .iter()
        .any(|r| matches!(r, EditorRequest::OpenSpec(ObjectRef::Opening(_)))));
}

#[test]
fn find_in_plan_goes_to_the_floor_and_selects_the_object() {
    let mut sim = Sim::new();
    let (doors, _) = house(&mut sim);
    // A second floor with its own door.
    sim.app
        .cx
        .project
        .floors
        .push(plan_core::Floor::new("2nd Floor", 109.0));
    let w = sim.app.cx.project.add_wall(
        1,
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        6.5,
        109.0,
        WallKind::Exterior,
    );
    let upstairs = sim
        .app
        .cx
        .project
        .add_opening(1, w, 80.0, OpeningKind::Door)
        .unwrap();
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let id = place(&mut sim, ScheduleKind::Door, 0.0);
    edit(&mut sim, id, |d| {
        d.floor_scope = plan_core::schedules::FloorScope::All
    });
    let t = table(&sim, id);
    assert_eq!(t.rows.len(), 4);
    let targets = sv::row_targets_of(&sim.app.cx, 0, id, 3);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id, upstairs);
    assert_eq!(targets[0].floor, 1);
    assert_eq!(sim.app.cx.floor, 0);
    assert_eq!(sv::find_in_plan(&mut sim.app.cx, &targets), 1);
    assert_eq!(sim.app.cx.floor, 1, "the plan goes to the object's floor");
    assert_eq!(
        sim.app.cx.selection.single(),
        Some(ObjectRef::Opening(upstairs))
    );
    assert!(sv::take_focus().is_some());
    // And back to a door of the first floor.
    let t0 = sv::row_targets_of(&sim.app.cx, 0, id, 0);
    assert_eq!(t0[0].id, doors[0]);
}

#[test]
fn renumber_closes_gaps_and_new_doors_go_to_the_bottom() {
    let mut sim = Sim::new();
    let (doors, _) = house(&mut sim);
    let id = place(&mut sim, ScheduleKind::Door, 0.0);
    let marks = |sim: &Sim| -> Vec<(Id, String)> {
        let d = sv::find(&sim.app.cx, id).unwrap();
        plan_docs::schedule_kinds::rows(&sim.app.cx.project, &d, 0, None)
            .into_iter()
            .map(|e| (e.id, e.cell("mark").to_string()))
            .collect()
    };
    assert_eq!(
        marks(&sim).iter().map(|m| m.1.as_str()).collect::<Vec<_>>(),
        ["D01", "D02", "D03"]
    );
    // The first door goes: the others keep D02 and D03.
    sim.app.cx.begin_change("Delete Door");
    sim.app.cx.project.remove_opening(0, doors[0]);
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let m = marks(&sim);
    assert_eq!(
        m.iter().map(|m| m.1.as_str()).collect::<Vec<_>>(),
        ["D02", "D03"]
    );
    // A door placed afterwards, far to the left, is the next number.
    let wall = sim.app.cx.floor().walls[0].id;
    let late = sim
        .app
        .cx
        .project
        .add_opening(0, wall, 20.0, OpeningKind::Door)
        .unwrap();
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let m = marks(&sim);
    assert_eq!(m.last().unwrap(), &(late, "D04".to_string()));
    // Renumber Schedule closes the gap and keeps the order: one undo step.
    sv::select(&mut sim.app.cx, id);
    sim.action(crate::toolbar::Action::Custom(cmd::RENUMBER));
    assert_eq!(sim.app.cx.undo_label(), Some("Renumber Schedule"));
    let m = marks(&sim);
    assert_eq!(
        m.iter().map(|m| m.1.as_str()).collect::<Vec<_>>(),
        ["D01", "D02", "D03"]
    );
    assert_eq!(m.last().unwrap().0, late);
    // Renumbering again changes nothing and takes no undo step.
    let n = sim.app.cx.undo_label().map(str::to_string);
    sim.action(crate::toolbar::Action::Custom(cmd::RENUMBER));
    assert_eq!(sim.app.cx.undo_label().map(str::to_string), n);
    assert_eq!(sim.undo().as_deref(), Some("Renumber Schedule"));
    assert_eq!(marks(&sim).last().unwrap().1, "D04");
}

#[test]
fn the_excel_export_keeps_the_totals_row() {
    let mut sim = Sim::new();
    house(&mut sim);
    let id = place(&mut sim, ScheduleKind::Window, 0.0);
    edit(&mut sim, id, |d| {
        show(d, &["area"]);
        d.totals_label = "Total glass".into();
    });
    let t = table(&sim, id);
    let bytes = t.to_xlsx();
    let sheets = plan_docs::xlsx_read::read_xlsx(&bytes).unwrap();
    assert_eq!(sheets.len(), 1);
    let rows = &sheets[0].rows;
    assert_eq!(rows.len(), t.rows.len() + 1, "headings and every row");
    let last = rows.last().unwrap();
    assert_eq!(last[0], "Total glass");
    let a = t.columns.iter().position(|c| c == "Area").unwrap();
    assert_eq!(
        last[a].parse::<f64>().unwrap(),
        t.rows.last().unwrap()[a].parse::<f64>().unwrap()
    );
    // The CSV has it too.
    assert!(t
        .to_csv()
        .lines()
        .last()
        .unwrap()
        .starts_with("Total glass"));
}

#[test]
fn the_edit_tools_are_one_undo_step_each() {
    let mut sim = Sim::new();
    house(&mut sim);
    let id = place(&mut sim, ScheduleKind::Door, 0.0);
    sv::select(&mut sim.app.cx, id);
    edit(&mut sim, id, |d| d.columns[0].width = 40.0);
    let buttons: Vec<&str> = sim
        .app
        .cx
        .extra_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect();
    for want in [
        "Renumber Schedule",
        "Reset Column Widths",
        "Align Left",
        "Center",
        "Align Right",
        "Justify",
        "Schedule to Text",
    ] {
        assert!(buttons.contains(&want), "{want} in {buttons:?}");
    }
    sim.action(crate::toolbar::Action::Custom(cmd::RESET_WIDTHS));
    assert_eq!(sim.app.cx.undo_label(), Some("Reset Column Widths"));
    assert!(sv::find(&sim.app.cx, id)
        .unwrap()
        .columns
        .iter()
        .all(|c| c.width == 0.0));
    sim.action(crate::toolbar::Action::Custom(cmd::ALIGN_RIGHT));
    assert_eq!(sim.app.cx.undo_label(), Some("Align Right"));
    assert_eq!(sv::find(&sim.app.cx, id).unwrap().h_align, TextAlign::Right);
    assert_eq!(sim.undo().as_deref(), Some("Align Right"));
    assert_eq!(sv::find(&sim.app.cx, id).unwrap().h_align, TextAlign::Left);
    // Schedule to Text: a text object with tabs replaces the schedule.
    let cads = sim.app.cx.floor().cad.len();
    sim.action(crate::toolbar::Action::Custom(cmd::TO_TEXT));
    assert_eq!(sim.app.cx.undo_label(), Some("Schedule to Text"));
    assert!(sv::find(&sim.app.cx, id).is_none());
    assert_eq!(sim.app.cx.floor().cad.len(), cads + 1);
    let text = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find_map(|c| match &c.item {
            plan_core::CadItem::Text { text, .. } if text.contains('\t') => Some(text.clone()),
            _ => None,
        })
        .expect("a tabbed text object");
    assert!(text.contains("Mark\tFloor"), "{text}");
    sim.undo();
    assert!(sv::find(&sim.app.cx, id).is_some());
}

#[test]
fn the_handles_rotate_resize_wrap_and_sort_in_one_step_each() {
    let mut sim = Sim::new();
    house(&mut sim);
    let id = place(&mut sim, ScheduleKind::Door, 0.0);
    sv::select(&mut sim.app.cx, id);
    sim.tool(ToolId::ScheduleVariant(ScheduleKind::Door));
    let d = sv::find(&sim.app.cx, id).unwrap();
    let l = sv::layout_of(&sim.app.cx, &d, 0);
    let hs = sv::handles::handles(&d, &l);
    let kinds: Vec<HandleKind> = hs.iter().map(|h| h.kind).collect();
    for want in [
        HandleKind::Move,
        HandleKind::ResizeLeft,
        HandleKind::ResizeRight,
        HandleKind::Rotate,
        HandleKind::Wrap,
        HandleKind::ResizeColumn(0),
        HandleKind::MoveRow(0),
        HandleKind::MoveColumn(0),
        HandleKind::Sort(0),
    ] {
        assert!(kinds.contains(&want), "{want:?} in {kinds:?}");
    }
    // Rotate: drag the triangle to the right of the middle -> 90 degrees.
    let rot = hs.iter().find(|h| h.kind == HandleKind::Rotate).unwrap().at;
    let centre = l.frame(d.position).centre;
    let target = Point::new(centre.x - 100.0, centre.y);
    let before = sim.app.cx.undo_label().map(str::to_string);
    sim.drag((rot.x, rot.y), (target.x, target.y));
    let turned = sv::find(&sim.app.cx, id).unwrap();
    assert!(
        (turned.angle.abs() - 90.0).abs() < 1.0,
        "angle {}",
        turned.angle
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Rotate Schedule"));
    sim.undo();
    assert_eq!(sim.app.cx.undo_label().map(str::to_string), before);
    assert_eq!(sv::find(&sim.app.cx, id).unwrap().angle, 0.0);
    // Resize the right edge: the last column widens.
    let right = hs
        .iter()
        .find(|h| h.kind == HandleKind::ResizeRight)
        .unwrap()
        .at;
    sim.drag((right.x, right.y), (right.x + 24.0, right.y));
    let wide = sv::layout_of(&sim.app.cx, &sv::find(&sim.app.cx, id).unwrap(), 0);
    assert!(
        (wide.width - (l.width + 24.0)).abs() < 0.5,
        "{} vs {}",
        wide.width,
        l.width
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Resize Schedule"));
    sim.undo();
    // Sort by Column: a click, then another reverses.
    let sort = hs
        .iter()
        .find(|h| h.kind == HandleKind::Sort(2))
        .unwrap()
        .at;
    sim.click(sort.x, sort.y);
    let s1 = sv::find(&sim.app.cx, id).unwrap();
    assert_eq!(s1.sort.field, "width");
    assert!(!s1.sort.descending);
    assert_eq!(sim.app.cx.undo_label(), Some("Sort Schedule"));
    sim.click(sort.x, sort.y);
    assert!(sv::find(&sim.app.cx, id).unwrap().sort.descending);
    // Wrap: dragged inward the diamond wraps the schedule; the screen shows
    // several tables.
    let wrap = hs.iter().find(|h| h.kind == HandleKind::Wrap).unwrap().at;
    let inward = Point::new(wrap.x, wrap.y + l.row_h * 2.0 + 2.0);
    sim.drag((wrap.x, wrap.y), (inward.x, inward.y));
    let w = sv::find(&sim.app.cx, id).unwrap();
    assert!(w.wrap.enabled);
    assert!(sv::layout_of(&sim.app.cx, &w, 0).pieces.len() > 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Wrap Schedule"));
}

#[test]
fn move_row_renumbers_and_asks_before_leaving_automatic_sorting() {
    let mut sim = Sim::new();
    let (doors, _) = house(&mut sim);
    let id = place(&mut sim, ScheduleKind::Door, 0.0);
    let order = |sim: &Sim| -> Vec<Id> {
        let d = sv::find(&sim.app.cx, id).unwrap();
        plan_docs::schedule_kinds::rows(&sim.app.cx.project, &d, 0, None)
            .iter()
            .map(|e| e.id)
            .collect()
    };
    let start = order(&sim);
    assert_eq!(start.len(), 3);
    assert!(sv::move_row(&mut sim.app.cx, 0, id, 2, 0));
    assert_eq!(sim.app.cx.undo_label(), Some("Move Schedule Row"));
    let now = order(&sim);
    assert_eq!(now, [start[2], start[0], start[1]]);
    // With Automatic Sorting on, the move waits for an answer.
    edit(&mut sim, id, |d| d.sort.field = "width".into());
    assert!(!sv::move_row(&mut sim.app.cx, 0, id, 1, 0));
    assert!(sv::pending_sort_prompt().is_some());
    let undo_before = sim.app.cx.undo_label().map(str::to_string);
    sv::answer_sort_prompt(&mut sim.app.cx, false);
    assert_eq!(sim.app.cx.undo_label().map(str::to_string), undo_before);
    assert!(!sv::find(&sim.app.cx, id).unwrap().sort.field.is_empty());
    assert!(!sv::move_row(&mut sim.app.cx, 0, id, 1, 0));
    sv::answer_sort_prompt(&mut sim.app.cx, true);
    assert!(sv::find(&sim.app.cx, id).unwrap().sort.field.is_empty());
    assert_eq!(sim.app.cx.undo_label(), Some("Move Schedule Row"));
    let _ = doors;
}

#[test]
fn create_schedule_from_room_lists_only_that_room() {
    let mut sim = Sim::new();
    house(&mut sim);
    // A cabinet in each room.
    for x in [30.0, 330.0] {
        let mut c = plan_cabinets::Cabinet::base(24.0);
        c.position = Point::new(x, 20.0);
        crate::editor::placed::add_cabinet(&mut sim.app.cx.project, 0, c).unwrap();
    }
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert_eq!(sim.app.cx.rooms.len(), 2);
    let west = sim
        .app
        .cx
        .rooms
        .iter()
        .position(|r| r.centroid.x < 240.0)
        .unwrap();
    sim.app.cx.selection.clear();
    crate::editor::rooms_edit::select_room(&mut sim.app.cx, west);
    let buttons: Vec<&str> = sim
        .app
        .cx
        .extra_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect();
    assert!(
        buttons.contains(&"Create Schedule from Room"),
        "{buttons:?}"
    );
    // The type chooser, then the click in the plan.
    sim.action(crate::toolbar::Action::Custom(
        crate::editor::rooms_edit::run_ids::SCHEDULE,
    ));
    assert!(crate::dialogs::schedule_spec::type_chooser_open());
    // Answer the chooser the way a click on OK does: arm the tool.
    let c = sim.app.cx.rooms[west].centroid;
    schedule_tool::arm_from_room(plan_core::schedules::RoomRef::at(0, c), "West".into());
    sim.tool(ToolId::ScheduleVariant(ScheduleKind::Cabinet));
    sim.click(0.0, -400.0);
    assert_eq!(sim.app.cx.undo_label(), Some("Create Schedule from Room"));
    let layer = sv::load(&sim.app.cx);
    let s = layer.schedules.last().unwrap();
    assert_eq!(s.rooms.len(), 1);
    assert_eq!(table(&sim, s.id).rows.len(), 1, "only the west cabinet");
    // Without the room scope both are listed.
    let all = place(&mut sim, ScheduleKind::Cabinet, 300.0);
    assert_eq!(table(&sim, all).rows.len(), 2);
    assert!(schedule_tool::armed_room().is_none());
}

#[test]
fn find_schedules_from_an_object_asks_when_there_are_several() {
    let mut sim = Sim::new();
    let (doors, _) = house(&mut sim);
    let a = place(&mut sim, ScheduleKind::Door, 0.0);
    sim.app.cx.selection.set(ObjectRef::Opening(doors[0]));
    // One schedule: it is selected at once.
    assert_eq!(sv::find_schedules_from_selection(&mut sim.app.cx), 1);
    assert_eq!(sv::selected(&sim.app.cx), Some(a));
    // Two: the Select Location dialog opens.
    let b = place(&mut sim, ScheduleKind::Door, 300.0);
    sim.app.cx.selection.set(ObjectRef::Opening(doors[0]));
    assert_eq!(sv::find_schedules_from_selection(&mut sim.app.cx), 2);
    assert!(select_location::is_open());
    let ident = sv::ident_of(&sim.app.cx, ObjectRef::Opening(doors[0])).unwrap();
    let list = sv::schedules_listing(&sim.app.cx, &ident);
    let locs = select_location::locations(&sim.app.cx, &list);
    select_location::go(&mut sim.app.cx, &locs[1].target);
    assert_eq!(sv::selected(&sim.app.cx), Some(b));
    // Move Up / Down in Schedule needs exactly one schedule.
    let labels: Vec<&str> = sim
        .app
        .cx
        .extra_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect();
    let _ = labels;
}

#[test]
fn move_up_and_down_in_schedule_work_for_an_object_in_one_schedule() {
    let mut sim = Sim::new();
    let (doors, _) = house(&mut sim);
    let id = place(&mut sim, ScheduleKind::Door, 0.0);
    sim.app.cx.selection.set(ObjectRef::Opening(doors[1]));
    let labels: Vec<&str> = sim
        .app
        .cx
        .extra_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect();
    assert!(labels.contains(&"Move Up in Schedule"), "{labels:?}");
    assert!(labels.contains(&"Find Schedule(s)"));
    let order = |sim: &Sim| -> Vec<Id> {
        let d = sv::find(&sim.app.cx, id).unwrap();
        plan_docs::schedule_kinds::rows(&sim.app.cx.project, &d, 0, None)
            .iter()
            .map(|e| e.id)
            .collect()
    };
    let before = order(&sim);
    let pos = before.iter().position(|d| *d == doors[1]).unwrap();
    sim.action(crate::toolbar::Action::Custom(cmd::MOVE_UP));
    let after = order(&sim);
    assert_eq!(after.iter().position(|d| *d == doors[1]), Some(pos - 1));
    sim.action(crate::toolbar::Action::Custom(cmd::MOVE_DOWN));
    assert_eq!(order(&sim), before);
}

#[test]
fn categories_scope_and_numbers_survive_a_save() {
    let mut sim = Sim::new();
    let (_, windows) = house(&mut sim);
    schedule_categories::open();
    schedule_categories::add_category(&mut sim.app.cx, "Glazing").unwrap();
    sim.app.cx.selection.set(ObjectRef::Opening(windows[0]));
    assert_eq!(
        schedule_categories::assign_selection(&mut sim.app.cx, "Glazing"),
        1
    );
    let id = place(&mut sim, ScheduleKind::Door, 0.0);
    edit(&mut sim, id, |d| {
        d.set_category("Custom/Glazing", true);
        d.set_category("Door/Hinged Door", false);
        d.wrap.enabled = true;
        d.wrap.by = WrapBy::Entries(1);
        d.swap = true;
        d.min_rows = 4;
    });
    let t = table(&sim, id);
    assert_eq!(t.rows.len(), 4, "the window in the custom category, padded");
    let json = sim.app.cx.project.to_json().unwrap();
    let back = plan_core::Project::from_json(&json).unwrap();
    let layer = plan_core::schedules::ScheduleLayer::load(&back.floors[0]);
    let s = layer.find(id).unwrap();
    assert!(s.swap && s.wrap.enabled && s.min_rows == 4);
    assert_eq!(s.wrap.by, WrapBy::Entries(1));
    assert!(s.categories["Custom/Glazing"]);
    assert!(!s.numbers.is_empty());
    assert!(back.schedule_setup.category("Glazing").is_some());
}

