//! Scenario 30 (round 14): select feel, the clipboard file, Edit Area and
//! Stretch CAD, and the CAD drawing additions (Shift lines, arc modes,
//! polyline arcs, Delete Break, Make Arc Tangent, the Edit Behavior indicator).

use super::Sim;
use crate::editor::{clipboard, EditorContext, ObjectRef};
use crate::shell::status;
use crate::toolbar::Action;
use crate::tools::cad::{arcs, ArcMode, CadMode};
use crate::tools::select::{self, area, MarqueeMode};
use crate::tools::{KeyEvent, PointerEvent, ToolId, ToolResult};
use eframe::egui::{self, Key, Modifiers};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::{Id, OpeningKind, WallKind};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    }
}

fn alt() -> Modifiers {
    Modifiers {
        alt: true,
        ..Modifiers::NONE
    }
}

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Modifiers::NONE
    }
}

fn ev(sim: &Sim, x: f64, y: f64, m: Modifiers, down: bool) -> PointerEvent {
    sim.event(x, y).with_modifiers(m).with_down(down)
}

fn send_down(sim: &mut Sim, x: f64, y: f64, m: Modifiers) -> ToolResult {
    let e = ev(sim, x, y, m, true);
    let r = sim.app.tools.active_mut().pointer_down(&mut sim.app.cx, e);
    sim.finish(r)
}

fn send_move(sim: &mut Sim, x: f64, y: f64, m: Modifiers, down: bool) -> ToolResult {
    sim.app.cx.cursor_world = Some(p(x, y));
    let e = ev(sim, x, y, m, down);
    let r = sim.app.tools.active_mut().pointer_move(&mut sim.app.cx, e);
    sim.finish(r)
}

fn send_up(sim: &mut Sim, x: f64, y: f64, m: Modifiers) -> ToolResult {
    let e = ev(sim, x, y, m, false);
    let r = sim.app.tools.active_mut().pointer_up(&mut sim.app.cx, e);
    sim.finish(r)
}

/// Press at `a`, drag to `b` with `m` held, release.
fn drag_with(sim: &mut Sim, a: (f64, f64), b: (f64, f64), m: Modifiers) -> ToolResult {
    send_move(sim, a.0, a.1, m, false);
    send_down(sim, a.0, a.1, m);
    send_move(sim, b.0, b.1, m, true);
    send_up(sim, b.0, b.1, m)
}

/// A spot on the x axis, between `lo` and `hi`, where a press neither hits a
/// temporary-dimension label nor a handle of the selected object.
fn free_spot(sim: &Sim, lo: f64, hi: f64) -> f64 {
    let cx = &sim.app.cx;
    let handles = crate::editor::handles::handles_for(cx, cx.px_per_in);
    let tol = cx.pick_tol();
    let mut x = lo;
    while x <= hi {
        let at = p(x, 0.0);
        let free = cx.temp.hit_label(at, cx.px_per_in).is_none()
            && crate::editor::handles::hit_handle(&handles, at, tol).is_none();
        if free {
            return x;
        }
        x += 2.0;
    }
    panic!("no free spot between {lo} and {hi}");
}

/// Four exterior walls, 20' x 10', corner at the origin.
fn house(sim: &mut Sim) -> [Id; 4] {
    let c = [p(0.0, 0.0), p(240.0, 0.0), p(240.0, 120.0), p(0.0, 120.0)];
    let mut ids = [0; 4];
    for i in 0..4 {
        ids[i] =
            sim.app
                .cx
                .project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
    }
    sim.app.cx.refresh();
    ids
}

fn line(sim: &mut Sim, a: Point, b: Point) -> Id {
    sim.app
        .cx
        .project
        .add_cad(0, "CAD, Default", CadItem::Line { a, b })
}

fn line_ends(sim: &Sim, id: Id) -> (Point, Point) {
    match &sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .item
    {
        CadItem::Line { a, b } => (*a, *b),
        other => panic!("not a line: {other:?}"),
    }
}

// ----- S-6, S-98, DW-65, S-110: hover, selection text, host wall -----

#[test]
fn hover_and_selection_are_described_in_the_status_bar() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let ids = house(&mut sim);
    let door = sim
        .app
        .cx
        .project
        .add_opening(0, ids[0], 120.0, OpeningKind::Door)
        .unwrap();
    sim.app.cx.refresh();
    // Over the door: its style and size.
    sim.move_to(120.0, 0.0);
    sim.app.cx.cursor_world = Some(p(120.0, 0.0));
    let f = status::context_fields(&sim.app.cx, true);
    assert_eq!(
        f.hover.as_deref().map(|h| &h[..16]),
        Some("Hinged Door 3068")
    );
    assert!(f.z.is_some(), "Z is shown beside X and Y");
    // Over the bare wall: kind and length.
    sim.move_to(40.0, 0.0);
    sim.app.cx.cursor_world = Some(p(40.0, 0.0));
    let f = status::context_fields(&sim.app.cx, true);
    let hover = f.hover.unwrap();
    assert!(hover.starts_with("Exterior Wall, 20'"), "{hover}");
    // Another tool shows no hover text but the tooltip text exists for Select.
    assert!(status::context_fields(&sim.app.cx, false).hover.is_none());
    assert!(status::hover_tooltip(&sim.app.cx).is_some());
    // Selecting the door: the selection text, and the host wall is outlined.
    sim.click(120.0, 0.0);
    assert_eq!(
        sim.app.cx.selection.single(),
        Some(ObjectRef::Opening(door))
    );
    let f = status::context_fields(&sim.app.cx, true);
    assert!(
        f.selection
            .as_deref()
            .unwrap()
            .starts_with("1 Hinged Door selected, Z "),
        "{:?}",
        f.selection
    );
    assert_eq!(select::host_walls(&sim.app.cx), vec![ids[0]]);
    // Two walls: the count and the type.
    sim.app.cx.selection.set(ObjectRef::Wall(ids[0]));
    sim.app.cx.selection.add(ObjectRef::Wall(ids[1]));
    assert_eq!(
        status::context_fields(&sim.app.cx, true)
            .selection
            .as_deref(),
        Some("2 Straight Walls selected")
    );
}

// ----- S-30: Alt marquee over objects, the Marquee Selection setting -----

#[test]
fn alt_on_the_press_marquees_from_on_top_of_an_object() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let ids = house(&mut sim);
    // Without Alt the press on the wall drags it.
    drag_with(&mut sim, (120.0, 0.0), (120.0, 24.0), Modifiers::NONE);
    assert_ne!(sim.app.cx.floor().wall(ids[0]).unwrap().start.y, 0.0);
    sim.undo();
    assert_eq!(sim.app.cx.floor().wall(ids[0]).unwrap().start.y, 0.0);
    // With Alt the same press starts a marquee: right to left touches walls.
    sim.app.cx.selection.clear();
    drag_with(&mut sim, (120.0, 0.0), (-30.0, 40.0), alt());
    assert_eq!(sim.app.cx.floor().wall(ids[0]).unwrap().start.y, 0.0);
    assert!(sim.app.cx.selection.contains(ObjectRef::Wall(ids[0])));
    assert!(sim.app.cx.selection.contains(ObjectRef::Wall(ids[3])));
    assert!(!sim.app.cx.selection.contains(ObjectRef::Wall(ids[1])));
    // The marquee is not an undo step.
    assert!(sim.app.cx.undo_label().is_none());
}

#[test]
fn the_marquee_selection_setting_decides_enclosing_or_touching() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let ids = house(&mut sim);
    // Left to right over part of the bottom wall: By Drag Direction encloses,
    // which a part of a wall is not.
    select::set_marquee_mode(MarqueeMode::ByDirection);
    drag_with(&mut sim, (-20.0, -20.0), (60.0, 30.0), Modifiers::NONE);
    assert!(sim.app.cx.selection.is_empty());
    select::set_marquee_mode(MarqueeMode::Touching);
    drag_with(&mut sim, (-20.0, -20.0), (60.0, 30.0), Modifiers::NONE);
    assert!(sim.app.cx.selection.contains(ObjectRef::Wall(ids[0])));
    assert!(sim.app.cx.selection.contains(ObjectRef::Wall(ids[3])));
    // Enclosing never picks what only sticks in, even right to left.
    select::set_marquee_mode(MarqueeMode::Enclosing);
    sim.app.cx.selection.clear();
    drag_with(&mut sim, (60.0, 30.0), (-20.0, -20.0), Modifiers::NONE);
    assert!(sim.app.cx.selection.is_empty());
    // The Edit menu's commands set it.
    sim.action(Action::Custom(select::MARQUEE_TOUCHING));
    assert_eq!(select::marquee_mode(), MarqueeMode::Touching);
    sim.action(Action::Custom(select::MARQUEE_DIRECTION));
    assert_eq!(select::marquee_mode(), MarqueeMode::ByDirection);
}

// ----- S-99: auto-scroll -----

#[test]
fn a_drag_at_the_canvas_edge_scrolls_the_view_toward_it() {
    let rect = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(800.0, 600.0));
    let dt = 1.0 / 60.0;
    let inside = select::auto_scroll_vector(rect, egui::pos2(500.0, 350.0), dt);
    assert_eq!(inside, egui::Vec2::ZERO);
    // Left edge: the content moves right (the view scrolls left).
    let left = select::auto_scroll_vector(rect, egui::pos2(110.0, 350.0), dt);
    assert!(left.x > 0.0 && left.y == 0.0, "{left:?}");
    let right = select::auto_scroll_vector(rect, egui::pos2(895.0, 350.0), dt);
    assert!(right.x < 0.0, "{right:?}");
    let top = select::auto_scroll_vector(rect, egui::pos2(500.0, 55.0), dt);
    assert!(top.y > 0.0);
    let bottom = select::auto_scroll_vector(rect, egui::pos2(500.0, 648.0), dt);
    assert!(bottom.y < 0.0);
    // Deeper is faster, and past the edge is capped.
    let near = select::auto_scroll_vector(rect, egui::pos2(120.0, 350.0), dt).x;
    let deep = select::auto_scroll_vector(rect, egui::pos2(101.0, 350.0), dt).x;
    let far = select::auto_scroll_vector(rect, egui::pos2(-500.0, 350.0), dt).x;
    assert!(near < deep && deep <= far);
    assert!((far - 900.0 * dt).abs() < 1e-3, "capped at 900 px/s: {far}");
}

#[test]
fn the_select_tool_reports_a_drag_in_progress_for_the_shell() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    house(&mut sim);
    assert!(!select::drag_in_progress());
    send_down(&mut sim, 120.0, 0.0, Modifiers::NONE);
    assert!(!select::drag_in_progress(), "a bare press does not scroll");
    send_move(&mut sim, 120.0, 30.0, Modifiers::NONE, true);
    assert!(select::drag_in_progress());
    send_up(&mut sim, 120.0, 30.0, Modifiers::NONE);
    assert!(!select::drag_in_progress());
    // A marquee counts once it is a drag.
    send_down(&mut sim, -50.0, -50.0, Modifiers::NONE);
    send_move(&mut sim, -10.0, -10.0, Modifiers::NONE, true);
    assert!(select::drag_in_progress());
    send_up(&mut sim, -10.0, -10.0, Modifiers::NONE);
    assert!(!select::drag_in_progress());
}

// ----- S-94: Ctrl-drag copies -----

#[test]
fn ctrl_drag_copies_the_selection_in_one_undo_step() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let l = line(&mut sim, p(0.0, 0.0), p(100.0, 0.0));
    // (The length label of the selected line sits at its middle: press away
    // from it.)
    sim.click(25.0, 0.0);
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Cad(l)));
    let x = free_spot(&sim, 10.0, 90.0);
    let res = drag_with(&mut sim, (x, 0.0), (x, 60.0), ctrl());
    assert_eq!(res.commit.as_deref(), Some("Copy Objects"));
    assert_eq!(sim.app.cx.floor().cad.len(), 2);
    // The original stayed, the copy moved and is what is selected now.
    assert_eq!(line_ends(&sim, l), (p(0.0, 0.0), p(100.0, 0.0)));
    let copy = match sim.app.cx.selection.single() {
        Some(ObjectRef::Cad(id)) => id,
        other => panic!("{other:?}"),
    };
    assert_ne!(copy, l);
    assert_eq!(line_ends(&sim, copy), (p(0.0, 60.0), p(100.0, 60.0)));
    assert_eq!(sim.undo().as_deref(), Some("Copy Objects"));
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
    assert!(sim.redo().is_some());
    assert_eq!(sim.app.cx.floor().cad.len(), 2);
}

#[test]
fn ctrl_drag_of_a_wall_leaves_the_walls_around_it_alone() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let ids = house(&mut sim);
    sim.click(60.0, 0.0);
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Wall(ids[0])));
    drag_with(&mut sim, (60.0, 0.0), (60.0, -72.0), ctrl());
    let f = sim.app.cx.floor();
    assert_eq!(f.walls.len(), 5);
    // Every original wall is where it was; the copy sits 72" below.
    assert_eq!(f.wall(ids[0]).unwrap().start, p(0.0, 0.0));
    assert_eq!(f.wall(ids[3]).unwrap().start, p(0.0, 120.0));
    assert_eq!(f.wall(ids[3]).unwrap().end, p(0.0, 0.0));
    let copy = f.walls.last().unwrap();
    assert_eq!((copy.start, copy.end), (p(0.0, -72.0), p(240.0, -72.0)));
}

#[test]
fn escape_during_a_ctrl_drag_removes_the_copies_and_restores_the_selection() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let l = line(&mut sim, p(0.0, 0.0), p(100.0, 0.0));
    sim.click(25.0, 0.0);
    let x = free_spot(&sim, 10.0, 90.0);
    send_down(&mut sim, x, 0.0, ctrl());
    send_move(&mut sim, x, 40.0, ctrl(), true);
    assert_eq!(sim.app.cx.floor().cad.len(), 2);
    sim.esc();
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Cad(l)));
    assert!(sim.app.cx.undo_label().is_none());
}

// ----- S-28, S-23: typed input during a drag -----

#[test]
fn a_typed_distance_moves_a_selection_by_exactly_that_much() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let a = line(&mut sim, p(0.0, 0.0), p(100.0, 0.0));
    let b = line(&mut sim, p(0.0, 20.0), p(100.0, 20.0));
    sim.app.cx.selection.set(ObjectRef::Cad(a));
    sim.app.cx.selection.add(ObjectRef::Cad(b));
    send_move(&mut sim, 50.0, 0.0, Modifiers::NONE, false);
    send_down(&mut sim, 50.0, 0.0, Modifiers::NONE);
    send_move(&mut sim, 58.0, 0.0, Modifiers::NONE, true);
    assert!(sim.app.cx.typed_input.is_armed());
    // The drag shows its live distance until something is typed.
    assert!(sim
        .app
        .cx
        .readout
        .as_deref()
        .unwrap()
        .starts_with("Distance:"));
    for ch in ["3", "'", "6"] {
        let r = sim.key(KeyEvent::text(ch));
        assert!(r.consumed);
    }
    assert!(sim.app.cx.readout.as_deref().unwrap().contains("3'6|"));
    let r = sim.key(KeyEvent::key(Key::Enter));
    assert!(r.commit.is_some());
    // 3'6" along the direction of the drag (east).
    assert_eq!(line_ends(&sim, a), (p(42.0, 0.0), p(142.0, 0.0)));
    assert_eq!(line_ends(&sim, b), (p(42.0, 20.0), p(142.0, 20.0)));
    assert!(!sim.app.cx.typed_input.is_armed());
    assert_eq!(sim.undo().as_deref(), Some("Move Objects"));
    assert_eq!(line_ends(&sim, a), (p(0.0, 0.0), p(100.0, 0.0)));
}

#[test]
fn a_typed_distance_and_angle_move_a_wall_perpendicular_or_any_object() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let ids = house(&mut sim);
    sim.click(60.0, 0.0);
    send_move(&mut sim, 60.0, 0.0, Modifiers::NONE, false);
    send_down(&mut sim, 60.0, 0.0, Modifiers::NONE);
    send_move(&mut sim, 60.0, 10.0, Modifiers::NONE, true);
    sim.key(KeyEvent::text("2'"));
    let r = sim.key(KeyEvent::key(Key::Enter));
    assert!(r.commit.is_some());
    // The wall went 24" up (the side the pointer was on); its neighbours
    // stretched to follow.
    let w = sim.app.cx.floor().wall(ids[0]).unwrap();
    assert_eq!((w.start.y, w.end.y), (24.0, 24.0));
    assert_eq!(sim.app.cx.floor().wall(ids[1]).unwrap().start.y, 24.0);
}

#[test]
fn a_typed_angle_turns_a_selected_cad_object() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let l = line(&mut sim, p(0.0, 0.0), p(100.0, 0.0));
    sim.click(50.0, 0.0);
    let handle = crate::editor::handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in)
        .into_iter()
        .find(|h| h.kind == crate::editor::handles::HandleKind::Rotate)
        .expect("the Rotate handle");
    send_move(&mut sim, handle.pos.x, handle.pos.y, Modifiers::NONE, false);
    send_down(&mut sim, handle.pos.x, handle.pos.y, Modifiers::NONE);
    send_move(
        &mut sim,
        handle.pos.x + 5.0,
        handle.pos.y,
        Modifiers::NONE,
        true,
    );
    // Digits go to the angle first on a rotate.
    sim.key(KeyEvent::text("90"));
    assert!(sim
        .app
        .cx
        .readout
        .as_deref()
        .unwrap()
        .starts_with("Rotate: 90|"));
    let r = sim.key(KeyEvent::key(Key::Enter));
    assert!(r.commit.is_some());
    let (a, b) = line_ends(&sim, l);
    // A quarter turn about its center (50, 0): the line stands on end.
    assert!(
        a.dist(p(50.0, -50.0)) < 1e-6 && b.dist(p(50.0, 50.0)) < 1e-6,
        "{a:?} {b:?}"
    );
}

// ----- S-85: the clipboard file -----

fn scratch_file(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("plan-studio-r14-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("clipboard.json")
}

#[test]
fn a_copy_in_one_plan_pastes_into_another_with_its_layers() {
    let path = scratch_file("copy");
    clipboard::set_file_path(Some(path.clone()));
    // Plan A: a wall with a door, a CAD line on a layer of its own.
    let mut a = Sim::new();
    let ids = house(&mut a);
    a.app
        .cx
        .project
        .add_opening(0, ids[0], 120.0, OpeningKind::Door)
        .unwrap();
    a.app
        .cx
        .project
        .layers
        .layers
        .push(plan_core::Layer::new("Survey Notes", [10, 120, 10], 25));
    let note = a.app.cx.project.add_cad(
        0,
        "Survey Notes",
        CadItem::Line {
            a: p(0.0, 300.0),
            b: p(50.0, 300.0),
        },
    );
    a.app.cx.selection.set(ObjectRef::Wall(ids[0]));
    a.app.cx.selection.add(ObjectRef::Cad(note));
    a.app.cx.run_custom(crate::editor::edit_commands::ids::COPY);
    assert!(path.exists(), "Copy wrote the clipboard file");
    // Plan B is another plan in another window (another process: it has not
    // seen the file yet): it has no such layer.
    clipboard::set_file_path(Some(path.clone()));
    let mut b = EditorContext::new(crate::plan_defaults::embedded());
    assert!(b.clipboard.is_none());
    assert!(b.project.layers.get("Survey Notes").is_none());
    assert!(clipboard::poll_file_now(&mut b), "the newer file is taken");
    let clip = b
        .clipboard
        .clone()
        .expect("the clipboard came from the file");
    assert_eq!(clip.walls.len(), 1);
    assert_eq!(clip.openings.len(), 1);
    assert_eq!(clip.cad.len(), 1);
    b.begin_change("Paste");
    let made = clip.paste(&mut b, p(10.0, 10.0), false);
    assert_eq!(made.len(), 2);
    assert_eq!(b.floor().walls.len(), 1);
    assert_eq!(b.floor().openings.len(), 1);
    // The layer travelled by name and was made on arrival.
    let layer = b.project.layers.get("Survey Notes").expect("layer made");
    assert_eq!(layer.color, [10, 120, 10]);
    assert_eq!(b.floor().cad[0].layer, "Survey Notes");
    // A second look at the unchanged file changes nothing.
    assert!(!clipboard::poll_file_now(&mut b));
    // A layer the plan already has keeps its own look.
    clipboard::set_file_path(Some(path.clone()));
    let mut c = EditorContext::new(crate::plan_defaults::embedded());
    c.project
        .layers
        .layers
        .push(plan_core::Layer::new("Survey Notes", [200, 0, 0], 50));
    assert!(clipboard::poll_file_now(&mut c));
    c.clipboard
        .clone()
        .unwrap()
        .paste(&mut c, Point::ZERO, false);
    assert_eq!(
        c.project.layers.get("Survey Notes").unwrap().color,
        [200, 0, 0]
    );
    assert_eq!(
        c.project
            .layers
            .layers
            .iter()
            .filter(|l| l.name == "Survey Notes")
            .count(),
        1
    );
    clipboard::reset_file_path();
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn the_clipboard_file_survives_junk_and_a_missing_home() {
    let path = scratch_file("junk");
    clipboard::set_file_path(Some(path.clone()));
    std::fs::write(&path, "not json").unwrap();
    let mut cx = EditorContext::new(crate::plan_defaults::embedded());
    assert!(!clipboard::poll_file_now(&mut cx));
    assert!(cx.clipboard.is_none());
    std::fs::write(&path, r#"{"format":"something else"}"#).unwrap();
    // A different mtime is needed for the poll to look again.
    clipboard::set_file_path(Some(path.clone()));
    assert!(!clipboard::poll_file_now(&mut cx));
    // No file at all (tests have no home folder): nothing happens.
    clipboard::set_file_path(None);
    assert!(!clipboard::poll_file_now(&mut cx));
    clipboard::reset_file_path();
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

// ----- S-96: Fill Window Selected -----

#[test]
fn fill_window_selected_frames_the_selection() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let l = line(&mut sim, p(1000.0, 1000.0), p(1100.0, 1050.0));
    house(&mut sim);
    sim.app.camera.rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    sim.app.cx.selection.set(ObjectRef::Cad(l));
    sim.action(Action::Custom(crate::dialogs::app_info::FILL_SELECTED));
    // The shell picks the request up once a frame and frames the selection.
    assert!(crate::dialogs::app_info::take_fill_selected_request());
    let (lo, hi) = crate::dialogs::app_info::selection_frame(&sim.app.cx).expect("a frame");
    sim.app.fill_rect(lo, hi);
    let cam = sim.app.camera;
    // The view is centered on the line and shows it at a useful size.
    assert!((cam.center.x - 1050.0).abs() < 1.0 && (cam.center.y - 1025.0).abs() < 1.0);
    assert!(cam.px_per_in > 4.0, "{}", cam.px_per_in);
    // Nothing selected: nothing to frame.
    sim.app.cx.selection.clear();
    assert!(crate::dialogs::app_info::selection_frame(&sim.app.cx).is_none());
}

#[test]
fn the_fill_window_selected_button_is_wired() {
    // The right-bar button used to be a stub (S-96).
    let src = include_str!("../toolbar.rs");
    let wired = src
        .match_indices("\"fill_selected\"")
        .any(|(at, _)| src[at..(at + 220).min(src.len())].contains("FILL_SELECTED"));
    assert!(wired, "the button runs FILL_SELECTED");
}

// ----- S-90, S-91: Edit Area and Stretch CAD from the menu -----

#[test]
fn the_edit_menu_commands_start_edit_area_and_stretch_cad() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Pan);
    let ids = house(&mut sim);
    sim.action(Action::Custom(select::EDIT_AREA));
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
    assert!(area::active());
    // The rubber band, then a move of the right half 48" to the right.
    sim.drag((100.0, -40.0), (300.0, 160.0));
    assert!(area::region().is_some());
    let res = sim.drag((200.0, 60.0), (248.0, 60.0));
    assert_eq!(res.commit.as_deref(), Some("Move Edit Area"));
    assert_eq!(
        sim.app.cx.floor().wall(ids[0]).unwrap().end,
        p(288.0, 0.0),
        "the wall crossing the edge stretched"
    );
    assert_eq!(sim.undo().as_deref(), Some("Move Edit Area"));
    sim.esc();
    assert!(!area::active());
    // Visible and Stretch CAD arm the same band.
    sim.action(Action::Custom(select::EDIT_AREA_VISIBLE));
    assert!(area::active());
    sim.esc();
    sim.action(Action::Custom(select::STRETCH_CAD));
    assert!(area::active());
    // Switching tools ends it.
    sim.tool(ToolId::Pan);
    assert!(!area::active());
}

#[test]
fn edit_area_visible_leaves_hidden_layers_alone() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    house(&mut sim);
    sim.app
        .cx
        .project
        .layers
        .layers
        .push(plan_core::Layer::new("Hidden Notes", [0, 0, 0], 25));
    let note = sim.app.cx.project.add_cad(
        0,
        "Hidden Notes",
        CadItem::Line {
            a: p(10.0, 50.0),
            b: p(60.0, 50.0),
        },
    );
    sim.app.cx.project.layers.layers.last_mut().unwrap().display = false;
    for visible_only in [true, false] {
        sim.app.cx.project.floors[0].cad[0].item = CadItem::Line {
            a: p(10.0, 50.0),
            b: p(60.0, 50.0),
        };
        sim.action(Action::Custom(if visible_only {
            select::EDIT_AREA_VISIBLE
        } else {
            select::EDIT_AREA
        }));
        sim.drag((-20.0, -20.0), (260.0, 140.0));
        sim.drag((100.0, 60.0), (100.0, 100.0));
        let moved = line_ends(&sim, note).0.y != 50.0;
        assert_eq!(moved, !visible_only, "visible_only {visible_only}");
        sim.esc();
        sim.undo();
    }
}

// ----- CAD-14: Shift-constrained lines -----

fn cad_tool(sim: &mut Sim, mode: CadMode) {
    sim.tool(ToolId::CadVariant(mode));
}

#[test]
fn shift_holds_a_cad_line_to_15_degree_steps() {
    let mut sim = Sim::new();
    cad_tool(&mut sim, CadMode::Line);
    sim.app.cx.defaults.editing.angle_snaps = false;
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.click(0.0, 0.0);
    // 100" east and 25" north is 14 degrees: Shift makes it 15.
    send_move(&mut sim, 100.0, 25.0, shift(), false);
    send_down(&mut sim, 100.0, 25.0, shift());
    send_up(&mut sim, 100.0, 25.0, shift());
    let c = sim.app.cx.floor().cad.last().unwrap();
    let CadItem::Line { a, b } = &c.item else {
        panic!()
    };
    let ang = b.sub(*a).angle().to_degrees();
    assert!((ang - 15.0).abs() < 1e-6, "{ang}");
    // Without Shift the pointer's own angle stands.
    sim.app.cx.selection.clear();
    sim.click(0.0, 500.0);
    sim.click(100.0, 525.0);
    let CadItem::Line { a, b } = &sim.app.cx.floor().cad.last().unwrap().item else {
        panic!()
    };
    assert!((b.sub(*a).angle().to_degrees() - 14.03).abs() < 0.1);
}

// ----- CAD-7: arc creation modes -----

#[test]
fn the_edit_menu_picks_the_arc_creation_mode() {
    let mut sim = Sim::new();
    assert_eq!(crate::tools::cad::current_arc_mode(), ArcMode::ThreePoint);
    for m in ArcMode::ALL {
        sim.action(Action::Custom(m.command()));
        assert_eq!(crate::tools::cad::current_arc_mode(), m);
        assert!(sim.app.cx.status.contains(m.name()));
    }
    assert_eq!(ArcMode::ALL.len(), 4);
}

#[test]
fn start_end_radius_bulges_toward_the_third_click_with_that_radius() {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 1.0;
    sim.action(Action::Custom(ArcMode::StartEndRadius.command()));
    cad_tool(&mut sim, CadMode::Arc);
    sim.click(0.0, 0.0);
    sim.click(100.0, 0.0);
    // 80" from the chord's middle (50, 0), below it.
    sim.click(50.0, -80.0);
    let CadItem::Arc {
        center,
        radius,
        start_angle,
        end_angle,
    } = sim.app.cx.floor().cad.last().unwrap().item.clone()
    else {
        panic!("an arc")
    };
    assert!((radius - 80.0).abs() < 1e-6, "{radius}");
    // The arc is the minor one on the third click's side: its middle is below
    // the chord.
    let sweep = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
    let mid_angle = start_angle + sweep * 0.5;
    let apex = p(
        center.x + radius * mid_angle.cos(),
        center.y + radius * mid_angle.sin(),
    );
    assert!(apex.y < -1.0, "{apex:?}");
    assert!(sweep < std::f64::consts::PI);
    // The ends are the clicked points.
    let at = |a: f64| p(center.x + radius * a.cos(), center.y + radius * a.sin());
    let ends = [at(start_angle), at(end_angle)];
    assert!(ends.iter().any(|e| e.dist(p(0.0, 0.0)) < 1e-6));
    assert!(ends.iter().any(|e| e.dist(p(100.0, 0.0)) < 1e-6));
    sim.action(Action::Custom(ArcMode::ThreePoint.command()));
}

#[test]
fn a_tangent_arc_continues_the_line_just_drawn_in_two_clicks() {
    let mut sim = Sim::new();
    cad_tool(&mut sim, CadMode::Line);
    sim.app.cx.defaults.editing.angle_snaps = false;
    sim.app.cx.defaults.grid.snap = 1.0;
    sim.click(0.0, 0.0);
    sim.click(100.0, 0.0);
    sim.action(Action::Custom(ArcMode::StartEndTangent.command()));
    cad_tool(&mut sim, CadMode::Arc);
    // Start where the line ended; one more click ends the arc.
    sim.click(100.0, 0.0);
    sim.click(150.0, 50.0);
    let CadItem::Arc {
        center,
        radius,
        start_angle,
        end_angle,
    } = sim.app.cx.floor().cad.last().unwrap().item.clone()
    else {
        panic!("an arc")
    };
    // It leaves (100, 0) heading east and curves left to (150, 50): a quarter
    // circle of radius 50 about (100, 50).
    assert!(center.dist(p(100.0, 50.0)) < 1e-6 && (radius - 50.0).abs() < 1e-6);
    assert!((start_angle + std::f64::consts::FRAC_PI_2).abs() < 1e-6);
    assert!(end_angle.abs() < 1e-6);
    // Elsewhere it takes the usual third click.
    sim.click(500.0, 500.0);
    sim.click(560.0, 500.0);
    assert_eq!(sim.app.cx.floor().cad.len(), 2, "two clicks so far");
    sim.click(500.0, 560.0);
    assert_eq!(sim.app.cx.floor().cad.len(), 3);
    sim.action(Action::Custom(ArcMode::ThreePoint.command()));
}

// ----- CAD-21, CAD-22, CAD-24 through the Select tool -----

#[test]
fn dragging_the_diamond_on_a_polyline_arc_edge_sets_its_bulge() {
    let mut sim = Sim::new();
    let id = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Polyline {
            points: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0)],
            closed: false,
        },
    );
    // Make the first edge an arc the way the tool does.
    let mut l = arcs::logical_of(&sim.app.cx, id).unwrap();
    l.toggle_edge(0, p(50.0, -10.0)).unwrap();
    arcs::replace_polyline(&mut sim.app.cx, id, &l);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Cad(id));
    let (edge, apex) = arcs::arc_handles(&sim.app.cx, id)[0];
    assert_eq!(edge, 0);
    let before = arcs::logical_of(&sim.app.cx, id).unwrap().bulge[0].unwrap();
    // Drag the diamond to 40" below the chord: bulge = 2 * 40 / 100.
    let res = drag_with(&mut sim, (apex.x, apex.y), (50.0, -40.0), Modifiers::NONE);
    assert_eq!(res.commit.as_deref(), Some("Curve Polyline Edge"));
    let after = arcs::logical_of(&sim.app.cx, id).unwrap().bulge[0].unwrap();
    assert!((after - 0.8).abs() < 1e-9, "{after}");
    assert!((before - arcs::DEFAULT_BULGE).abs() < 1e-9);
    sim.undo();
    let back = arcs::logical_of(&sim.app.cx, id).unwrap().bulge[0].unwrap();
    assert!((back - before).abs() < 1e-9);
}

#[test]
fn polyline_arcs_are_plain_points_for_everything_else() {
    let mut sim = Sim::new();
    let id = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Polyline {
            points: vec![p(0.0, 0.0), p(100.0, 0.0)],
            closed: false,
        },
    );
    let mut l = arcs::logical_of(&sim.app.cx, id).unwrap();
    l.toggle_edge(0, p(50.0, 10.0)).unwrap();
    arcs::replace_polyline(&mut sim.app.cx, id, &l);
    // The bounds see the curve: the arc rises above the chord.
    let (lo, hi) = sim.app.cx.floor().cad[0].bounds();
    assert!(hi.y > 10.0 && lo.y.abs() < 1e-9, "{lo:?} {hi:?}");
    // The record survives a save and load.
    let json = sim.app.cx.project.to_json().unwrap();
    let back = plan_core::Project::from_json(&json).unwrap();
    assert_eq!(back.floors[0].cad_attrs(id).unwrap().arc_edges.len(), 1);
    // Moving the polyline keeps the arc editable.
    let mut cx = sim.app.cx;
    cx.selection.set(ObjectRef::Cad(id));
    crate::editor::transform::translate_objects(&mut cx, &[ObjectRef::Cad(id)], p(30.0, 40.0));
    assert_eq!(arcs::logical_of(&cx, id).unwrap().bulge[0].is_some(), true);
}

// ----- S-66: the Edit Behavior indicator -----

#[test]
fn the_edit_behavior_shows_in_the_status_bar_until_select_is_left() {
    use plan_core::defaults::EditBehavior;
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    sim.app.cx.defaults.editing.behavior.mode = EditBehavior::Concentric;
    let f = status::context_fields(&sim.app.cx, true);
    assert_eq!(f.behavior.as_deref(), Some("Edit Behavior: Concentric"));
    sim.tool(ToolId::CadVariant(CadMode::Line));
    assert_eq!(
        sim.app.cx.defaults.editing.behavior.mode,
        EditBehavior::Default
    );
    assert!(status::context_fields(&sim.app.cx, false)
        .behavior
        .is_none());
}
