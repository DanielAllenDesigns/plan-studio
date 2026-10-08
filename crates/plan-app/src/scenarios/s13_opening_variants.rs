//! Scenario 13: the Door and Window flyouts, their symbols, labels, jamb
//! handles, temporary dimensions and mulling (DW-12, DW-24, DW-26..DW-28,
//! DW-38..DW-52, DW-59..DW-63, DW-77, DW-98, DW-107).

use super::{draw_shell, Sim};
use crate::editor::handles::{self, HandleKind};
use crate::editor::opening_view::opening_labels;
use crate::editor::tempdim::{self, TempDimKind};
use crate::editor::ObjectRef;
use crate::shell::hotkeys::{Chord, HotkeyMap, HotkeyState};
use crate::toolbar::{self, Action};
use crate::tools::{KeyEvent, PointerEvent, ToolId};
use eframe::egui::Key;
use plan_core::geometry::Point;
use plan_core::opening_symbol::{plan_symbol, PartKind};
use plan_core::{Id, Opening, OpeningKind, OpeningStyle};
use std::time::Instant;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn openings(sim: &Sim) -> Vec<Opening> {
    sim.app.cx.floor().openings.clone()
}

fn only(sim: &Sim) -> Opening {
    let v = openings(sim);
    assert_eq!(v.len(), 1, "{v:?}");
    v[0].clone()
}

/// Presses the flyout entry the way a toolbar click does.
fn pick(sim: &mut Sim, name: &str, flyout: &toolbar::Flyout) {
    let it = flyout
        .entries
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no flyout entry {name}"));
    assert!(it.enabled, "{name} is dimmed");
    assert!(
        !matches!(it.action, Action::NotImplemented(_)),
        "{name} is still a stub"
    );
    sim.action(it.action);
}

fn plain(key: Key) -> Chord {
    Chord {
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
        key,
    }
}

/// (flyout entry, style, width, symbol parts the plan must show)
type Case = (
    &'static str,
    OpeningStyle,
    f64,
    &'static [(PartKind, usize)],
);

const DOORS: &[Case] = &[
    (
        "Hinged Door",
        OpeningStyle::Hinged,
        36.0,
        &[(PartKind::Jamb, 2), (PartKind::Leaf, 1), (PartKind::Swing, 1)],
    ),
    (
        "Double Door",
        OpeningStyle::DoubleDoor,
        60.0,
        &[(PartKind::Jamb, 2), (PartKind::Leaf, 2), (PartKind::Swing, 2)],
    ),
    (
        "Doorway",
        OpeningStyle::Doorway,
        36.0,
        &[(PartKind::Jamb, 2), (PartKind::Leaf, 0), (PartKind::Swing, 0)],
    ),
    (
        "Sliding Door",
        OpeningStyle::Sliding,
        72.0,
        &[(PartKind::Leaf, 2), (PartKind::Arrow, 1), (PartKind::Swing, 0)],
    ),
    (
        "Pocket Door",
        OpeningStyle::Pocket,
        30.0,
        &[(PartKind::Leaf, 1), (PartKind::Hidden, 1)],
    ),
    (
        "Bifold Door",
        OpeningStyle::Bifold,
        48.0,
        &[(PartKind::Leaf, 1), (PartKind::Swing, 0)],
    ),
    (
        "Barn Door",
        OpeningStyle::Barn,
        36.0,
        &[(PartKind::Leaf, 1), (PartKind::Track, 1)],
    ),
    (
        "Fixed Door",
        OpeningStyle::Fixed,
        36.0,
        &[(PartKind::Glass, 1), (PartKind::Swing, 0)],
    ),
    (
        "Garage Door",
        OpeningStyle::Garage,
        108.0,
        &[(PartKind::Leaf, 1), (PartKind::Hidden, 1)],
    ),
    (
        "Shower Door",
        OpeningStyle::Shower,
        28.0,
        &[(PartKind::Leaf, 1), (PartKind::Swing, 1)],
    ),
];

const WINDOWS: &[Case] = &[
    (
        "Window",
        OpeningStyle::Window,
        32.0,
        &[(PartKind::Frame, 2), (PartKind::Glass, 1)],
    ),
    (
        "Bay Window",
        OpeningStyle::BayWindow,
        96.0,
        &[(PartKind::Frame, 2), (PartKind::Jamb, 2)],
    ),
    (
        "Bow Window",
        OpeningStyle::BowWindow,
        96.0,
        &[(PartKind::Frame, 2), (PartKind::Jamb, 2)],
    ),
    (
        "Box Window",
        OpeningStyle::BoxWindow,
        60.0,
        &[(PartKind::Frame, 2), (PartKind::Jamb, 2)],
    ),
    (
        "Pass-Through",
        OpeningStyle::PassThrough,
        48.0,
        &[(PartKind::Hidden, 1), (PartKind::Frame, 2)],
    ),
    (
        "Wall Niche",
        OpeningStyle::WallNiche,
        24.0,
        &[(PartKind::Jamb, 2), (PartKind::Frame, 1)],
    ),
    (
        "Casement Window",
        OpeningStyle::Casement,
        32.0,
        &[(PartKind::Swing, 1), (PartKind::Leaf, 1)],
    ),
    (
        "Fixed Window",
        OpeningStyle::Fixed,
        32.0,
        &[(PartKind::Glass, 1), (PartKind::Swing, 0)],
    ),
    (
        "Sliding Window",
        OpeningStyle::SlidingWindow,
        60.0,
        &[(PartKind::Leaf, 2), (PartKind::Arrow, 1)],
    ),
    (
        "Awning Window",
        OpeningStyle::Awning,
        36.0,
        &[(PartKind::Hidden, 1), (PartKind::Frame, 2)],
    ),
    (
        "Hopper Window",
        OpeningStyle::Hopper,
        36.0,
        &[(PartKind::Hidden, 1), (PartKind::Frame, 2)],
    ),
];

fn check_flyout(kind: OpeningKind, flyout: toolbar::Flyout, cases: &[Case]) {
    // Every entry of the flyout is one of the cases, and none is dimmed.
    assert_eq!(flyout.entries.len(), cases.len());
    for (name, style, width, parts) in cases {
        let mut sim = house();
        pick(&mut sim, name, &flyout);
        assert_eq!(sim.app.tools.active().name(), *name);
        sim.click(240.0, 0.0);
        let o = only(&sim);
        assert_eq!((o.kind, o.style), (kind, *style), "{name}");
        assert_eq!(o.width, *width, "{name}");
        // The flavor is placed with the label "Place Door" / "Place Window".
        let label = if kind == OpeningKind::Door {
            "Place Door"
        } else {
            "Place Window"
        };
        assert_eq!(sim.app.cx.undo_label(), Some(label), "{name}");
        // The symbol has the parts Chief draws for it.
        let wall = sim.app.cx.floor().wall(o.wall_id).unwrap().clone();
        let ext = plan_core::exterior_sign(&wall, &sim.app.cx.rooms);
        let sym = plan_symbol(&wall, &o, ext);
        for (part, n) in *parts {
            assert_eq!(sym.count(*part), *n, "{name}: {part:?} in {:?}", sym.parts);
        }
        // The wall stays cut: undo takes the whole flavor away again.
        sim.undo();
        assert!(openings(&sim).is_empty(), "{name}");
    }
}

#[test]
fn every_door_flyout_entry_places_its_flavor_with_its_symbol() {
    check_flyout(OpeningKind::Door, toolbar::door(), DOORS);
}

#[test]
fn every_window_flyout_entry_places_its_flavor_with_its_symbol() {
    check_flyout(OpeningKind::Window, toolbar::window(), WINDOWS);
}

#[test]
fn flavors_with_a_swing_take_it_from_the_pointer() {
    let mut sim = house();
    pick(&mut sim, "Casement Window", &toolbar::window());
    // Pointer on the wall's left side near the start: no flip, hinge at start.
    let wall = sim.app.cx.floor().walls[0].clone();
    let n = wall.normal();
    sim.click(60.0, 3.0 * n.y);
    sim.click(420.0, -3.0 * n.y);
    let mut ws = openings(&sim);
    ws.sort_by(|a, b| a.center_offset.total_cmp(&b.center_offset));
    assert!(!ws[0].swing_flipped && !ws[0].hinge_at_end);
    assert!(ws[1].swing_flipped && ws[1].hinge_at_end);
    // The plain window ignores the pointer.
    sim.tool(ToolId::Window);
    sim.click(240.0, -3.0 * n.y);
    let plain_window = openings(&sim)
        .into_iter()
        .find(|o| o.style == OpeningStyle::Window)
        .unwrap();
    assert!(!plain_window.swing_flipped);
}

#[test]
fn the_two_key_chief_hotkeys_pick_the_door_flavors() {
    let mut st = HotkeyState::new(HotkeyMap::defaults());
    let mut now = Instant::now();
    let mut press = |keys: [Key; 2]| {
        let mut out = Vec::new();
        for k in keys {
            st.press(plain(k), now, &mut out);
            now += std::time::Duration::from_millis(100);
        }
        out
    };
    let tool = |style| {
        Action::SetTool(crate::tools::opening::OpeningVariant::door(style).tool_id())
    };
    assert_eq!(press([Key::D, Key::W]), vec![tool(OpeningStyle::Doorway)]);
    assert_eq!(press([Key::S, Key::D]), vec![tool(OpeningStyle::Sliding)]);
    assert_eq!(press([Key::D, Key::P]), vec![tool(OpeningStyle::Pocket)]);
    assert_eq!(press([Key::G, Key::D]), vec![tool(OpeningStyle::Garage)]);
    assert_eq!(press([Key::D, Key::H]), vec![Action::SetTool(ToolId::Door)]);
}

#[test]
fn the_flyout_hint_and_name_follow_the_flavor() {
    let mut sim = house();
    pick(&mut sim, "Sliding Door", &toolbar::door());
    assert_eq!(sim.app.tools.active().name(), "Sliding Door");
    assert!(sim.app.tools.active().hint().starts_with("Sliding Door"));
    assert_eq!(
        sim.app.tools.active_id(),
        crate::tools::opening::OpeningVariant::door(OpeningStyle::Sliding).tool_id()
    );
    // Switching back keeps the one tool object but resets the flavor.
    sim.tool(ToolId::Door);
    assert_eq!(sim.app.tools.active().name(), "Hinged Door");
    sim.tool(ToolId::Window);
    assert_eq!(sim.app.tools.active().name(), "Window");
}

// ----- labels (DW-59..DW-63, DW-77, DW-94) -----

#[test]
fn a_placed_door_is_labelled_with_its_size_until_a_schedule_numbers_it() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    let id = only(&sim).id;
    let label = |sim: &mut Sim| {
        sim.app.cx.refresh();
        opening_labels(&sim.app.cx)
            .into_iter()
            .find(|l| l.opening == id)
            .map(|l| (l.text, l.is_mark))
    };
    // The exterior door is 3'-0" x 8'-0".
    assert_eq!(label(&mut sim), Some(("3080".to_string(), false)));
    // A Door Schedule on the plan numbers it: the label becomes the mark.
    let sid = crate::editor::schedule_view::add(
        &mut sim.app.cx,
        plan_core::schedules::ScheduleKind::Door,
        Point::new(0.0, -90.0),
    );
    let mut def = crate::editor::schedule_view::find(&sim.app.cx, sid).unwrap();
    def.show_labels = true;
    assert!(crate::editor::schedule_view::replace(
        &mut sim.app.cx,
        0,
        def
    ));
    assert_eq!(label(&mut sim), Some(("D01".to_string(), true)));
    // A second door takes the next mark (DW-77); undoing it frees the mark
    // again (DW-94).
    sim.click(360.0, 0.0);
    let second = openings(&sim)
        .into_iter()
        .find(|o| o.id != id && o.kind == OpeningKind::Door)
        .unwrap()
        .id;
    sim.app.cx.refresh();
    let marks: Vec<(Id, String)> = opening_labels(&sim.app.cx)
        .into_iter()
        .map(|l| (l.opening, l.text))
        .collect();
    assert!(marks.contains(&(id, "D01".into())) && marks.contains(&(second, "D02".into())));
    sim.undo();
    assert_eq!(openings(&sim).len(), 1);
    assert_eq!(label(&mut sim), Some(("D01".to_string(), true)));
}

// ----- handles and temporary dimensions (DW-12, DW-26..DW-28) -----

fn window_at(sim: &mut Sim, x: f64) -> Id {
    sim.tool(ToolId::Window);
    sim.click(x, 0.0);
    let id = openings(sim)
        .into_iter()
        .filter(|o| o.kind == OpeningKind::Window)
        .max_by(|a, b| {
            // The newest window has the highest id.
            a.id.cmp(&b.id)
        })
        .unwrap()
        .id;
    sim.tool(ToolId::Select);
    id
}

fn opening(sim: &Sim, id: Id) -> Opening {
    sim.app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == id)
        .unwrap()
        .clone()
}

fn handle(sim: &Sim, kind: HandleKind) -> Point {
    handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in)
        .into_iter()
        .find(|h| h.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} handle"))
        .pos
}

#[test]
fn dragging_a_jamb_handle_resizes_and_keeps_the_other_jamb() {
    let mut sim = house();
    let id = window_at(&mut sim, 240.0);
    sim.app.cx.selection.set(ObjectRef::Opening(id));
    let before = opening(&sim, id);
    let (start, end) = (before.start_offset(), before.end_offset());
    // Both jambs have a handle next to the center one.
    let hs = handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in);
    assert!(hs.iter().any(|h| h.kind == HandleKind::ResizeStart));
    assert!(hs.iter().any(|h| h.kind == HandleKind::ResizeEnd));
    let from = handle(&sim, HandleKind::ResizeEnd);
    sim.drag((from.x, from.y), (from.x + 24.0, from.y));
    let after = opening(&sim, id);
    assert_eq!(after.start_offset(), start);
    assert!((after.end_offset() - (end + 24.0)).abs() < 1.0, "{after:?}");
    assert_eq!(sim.app.cx.undo_label(), Some("Resize Opening"));
    // The start handle moves the other way and keeps the end.
    let from = handle(&sim, HandleKind::ResizeStart);
    let end_before = opening(&sim, id).end_offset();
    sim.drag((from.x, from.y), (from.x - 12.0, from.y));
    let again = opening(&sim, id);
    assert_eq!(again.end_offset(), end_before);
    assert!((again.start_offset() - (start - 12.0)).abs() < 1.0);
    // One undo per drag.
    sim.undo();
    assert_eq!(opening(&sim, id).start_offset(), after.start_offset());
    sim.undo();
    assert_eq!(opening(&sim, id).width, before.width);
}

#[test]
fn resize_stops_at_the_wall_end_and_the_neighbour() {
    let mut sim = house();
    let a = window_at(&mut sim, 120.0);
    let b = window_at(&mut sim, 240.0);
    sim.app.cx.selection.set(ObjectRef::Opening(a));
    let from = handle(&sim, HandleKind::ResizeEnd);
    // Far past the neighbour: stops two inches short of it.
    sim.drag((from.x, from.y), (from.x + 300.0, from.y));
    let ob = opening(&sim, b);
    assert!((opening(&sim, a).end_offset() - (ob.start_offset() - 2.0)).abs() < 1e-9);
    // The first window's start handle stops at the wall end clearance.
    let from = handle(&sim, HandleKind::ResizeStart);
    sim.drag((from.x, from.y), (from.x - 500.0, from.y));
    assert_eq!(opening(&sim, a).start_offset(), 2.0);
}

#[test]
fn the_temporary_dimensions_show_width_and_the_gaps_to_ends_and_neighbours() {
    let mut sim = house();
    let a = window_at(&mut sim, 120.0);
    let b = window_at(&mut sim, 300.0);
    sim.app.cx.selection.set(ObjectRef::Opening(b));
    sim.app.cx.refresh();
    let (oa, ob) = (opening(&sim, a), opening(&sim, b));
    let value = |sim: &Sim, kind| {
        sim.app
            .cx
            .temp
            .dims
            .iter()
            .find(|d| d.kind == kind)
            .map(|d| d.value)
    };
    assert_eq!(value(&sim, TempDimKind::OpeningWidth), Some(ob.width));
    // To the left the neighbour is nearer than the wall start.
    assert_eq!(
        value(&sim, TempDimKind::OpeningToStart),
        Some(ob.start_offset() - oa.end_offset())
    );
    let len = sim.app.cx.floor().walls[0].length();
    assert_eq!(
        value(&sim, TempDimKind::OpeningToEnd),
        Some(len - ob.end_offset())
    );
}

#[test]
fn typing_a_width_resizes_about_the_center_and_a_gap_moves_the_opening() {
    let mut sim = house();
    let a = window_at(&mut sim, 120.0);
    let b = window_at(&mut sim, 300.0);
    sim.app.cx.selection.set(ObjectRef::Opening(b));
    sim.app.cx.refresh();
    let i = sim
        .app
        .cx
        .temp
        .dims
        .iter()
        .position(|d| d.kind == TempDimKind::OpeningWidth)
        .unwrap();
    let before = opening(&sim, b);
    assert!(sim.app.cx.temp.begin_edit(i));
    sim.app.cx.temp.editing.as_mut().unwrap().text = "48".into();
    let label = tempdim::commit_edit(&mut sim.app.cx).unwrap();
    assert_eq!(label, "Resize Opening");
    let after = opening(&sim, b);
    assert_eq!(after.width, 48.0);
    assert!((after.center_offset - before.center_offset).abs() < 1e-9);
    // The gap to the neighbour on the left: type 30" and the window moves.
    sim.app.cx.selection.set(ObjectRef::Opening(b));
    sim.app.cx.temp.cancel();
    sim.app.cx.refresh();
    let j = sim
        .app
        .cx
        .temp
        .dims
        .iter()
        .position(|d| d.kind == TempDimKind::OpeningToStart)
        .unwrap();
    assert!(sim.app.cx.temp.begin_edit(j));
    sim.app.cx.temp.editing.as_mut().unwrap().text = "30".into();
    tempdim::commit_edit(&mut sim.app.cx).unwrap();
    let moved = opening(&sim, b);
    assert!((moved.start_offset() - (opening(&sim, a).end_offset() + 30.0)).abs() < 1e-9);
    assert_eq!(moved.width, 48.0);
    // A width that does not fit is refused and leaves no undo step.
    sim.app.cx.temp.cancel();
    sim.app.cx.refresh();
    let n = sim.app.cx.floor().openings.len();
    let i = sim
        .app
        .cx
        .temp
        .dims
        .iter()
        .position(|d| d.kind == TempDimKind::OpeningWidth)
        .unwrap();
    sim.app.cx.temp.begin_edit(i);
    sim.app.cx.temp.editing.as_mut().unwrap().text = "900".into();
    assert!(tempdim::commit_edit(&mut sim.app.cx).is_err());
    assert_eq!(sim.app.cx.floor().openings.len(), n);
    assert_eq!(opening(&sim, b).width, 48.0);
}

#[test]
fn typing_a_number_while_a_jamb_is_dragged_sets_the_width() {
    let mut sim = house();
    let id = window_at(&mut sim, 240.0);
    sim.app.cx.selection.set(ObjectRef::Opening(id));
    let from = handle(&sim, HandleKind::ResizeEnd);
    let start = opening(&sim, id).start_offset();
    sim.move_to(from.x, from.y);
    sim.down(from.x, from.y);
    // Move past the drag threshold, then type 5'.
    let ev = PointerEvent::at(&sim.app.cx, Point::new(from.x + 30.0, from.y)).with_down(true);
    sim.app
        .tools
        .active_mut()
        .pointer_move(&mut sim.app.cx, ev);
    assert!(sim.app.cx.typed_input.is_armed());
    for ch in ["6", "0"] {
        let r = sim.key(KeyEvent::text(ch));
        assert!(r.consumed);
    }
    sim.key(KeyEvent::key(Key::Enter));
    let after = opening(&sim, id);
    assert_eq!(after.start_offset(), start);
    assert_eq!(after.width, 60.0);
    assert!(!sim.app.cx.typed_input.is_armed());
    assert_eq!(sim.app.cx.undo_label(), Some("Resize Opening"));
}

#[test]
fn typing_a_number_while_an_opening_slides_sets_the_gap_to_the_nearer_side() {
    let mut sim = house();
    let id = window_at(&mut sim, 100.0);
    sim.app.cx.selection.set(ObjectRef::Opening(id));
    let from = handle(&sim, HandleKind::PerpendicularMove);
    sim.move_to(from.x, from.y);
    sim.down(from.x, from.y);
    let ev = PointerEvent::at(&sim.app.cx, Point::new(from.x + 20.0, from.y)).with_down(true);
    sim.app
        .tools
        .active_mut()
        .pointer_move(&mut sim.app.cx, ev);
    assert!(sim.app.cx.typed_input.is_armed());
    for ch in ["3", "0"] {
        sim.key(KeyEvent::text(ch));
    }
    sim.key(KeyEvent::key(Key::Enter));
    // The wall start is nearer: the window's start jamb is 30" from it.
    assert_eq!(opening(&sim, id).start_offset(), 30.0);
    assert_eq!(sim.app.cx.undo_label(), Some("Move Opening"));
}

// ----- mulling (DW-51, DW-98) -----

#[test]
fn two_adjacent_windows_mull_into_one_unit_and_unmull_splits_it() {
    let mut sim = house();
    let a = window_at(&mut sim, 200.0);
    let b = window_at(&mut sim, 240.0);
    // 32" windows 8" apart (the second click clamps nothing).
    assert!(opening(&sim, b).start_offset() - opening(&sim, a).end_offset() < 12.0);
    sim.app.cx.selection.set(ObjectRef::Opening(a));
    sim.app.cx.selection.toggle(ObjectRef::Opening(b));
    sim.app.cx.run_custom(crate::editor::opening_edit::MULL);
    assert_eq!(opening(&sim, a).end_offset(), opening(&sim, b).start_offset());
    assert!(opening(&sim, a).mull_group.is_some());
    assert_eq!(sim.app.cx.undo_label(), Some("Mull Windows"));
    // One width for the unit shows in the dimensions.
    sim.app.cx.selection.set(ObjectRef::Opening(a));
    sim.app.cx.refresh();
    let unit = sim
        .app
        .cx
        .temp
        .dims
        .iter()
        .find(|d| d.kind == TempDimKind::OpeningUnitWidth)
        .unwrap()
        .value;
    assert_eq!(unit, opening(&sim, a).width + opening(&sim, b).width);
    // Dragging one moves both.
    let from = handle(&sim, HandleKind::PerpendicularMove);
    let (sa, sb) = (opening(&sim, a).start_offset(), opening(&sim, b).start_offset());
    sim.drag((from.x, from.y), (from.x + 40.0, from.y));
    assert!((opening(&sim, a).start_offset() - (sa + 40.0)).abs() < 1.0);
    assert!((opening(&sim, b).start_offset() - (sb + 40.0)).abs() < 1.0);
    // The inner jamb has no handle: only the unit's ends do.
    let hs = handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in);
    assert!(hs.iter().any(|h| h.kind == HandleKind::ResizeStart));
    assert!(!hs.iter().any(|h| h.kind == HandleKind::ResizeEnd));
    // Unmull.
    sim.app.cx.run_custom(crate::editor::opening_edit::UNMULL);
    assert!(openings(&sim).iter().all(|o| o.mull_group.is_none()));
    assert_eq!(sim.app.cx.undo_label(), Some("Unmull Windows"));
}

#[test]
fn center_on_wall_segment_is_in_the_edit_toolbar_of_an_opening() {
    let mut sim = house();
    let a = window_at(&mut sim, 100.0);
    sim.app.cx.selection.set(ObjectRef::Opening(a));
    let buttons = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    assert!(buttons.iter().any(|b| b.label == "Center on Wall Segment"));
    sim.app
        .cx
        .run_custom(crate::editor::opening_edit::CENTER_SEGMENT);
    let wall_len = sim.app.cx.floor().walls[0].length();
    let o = opening(&sim, a);
    assert!((o.center_offset - wall_len * 0.5).abs() < 1.0, "{o:?}");
}

#[test]
fn opening_style_survives_save_and_reopen() {
    let mut sim = house();
    pick(&mut sim, "Sliding Door", &toolbar::door());
    sim.click(240.0, 0.0);
    let json = sim.app.cx.project.to_json().unwrap();
    let back = plan_core::Project::from_json(&json).unwrap();
    let o = &back.floors[0].openings[0];
    assert_eq!((o.style, o.width), (OpeningStyle::Sliding, 72.0));
}
