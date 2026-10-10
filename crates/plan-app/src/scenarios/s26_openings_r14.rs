//! Scenario 26: doors and windows, round 14: the placement feel (the "no"
//! cursor and ghost, click-then-drag, alignment snaps, wall junction
//! clearance, touching windows: DW-3, DW-4, DW-5, DW-10, DW-75, DW-87), the
//! plan detail (threshold, sill, both swing arcs, indicators, jambs, recess:
//! DW-35, DW-57, DW-81..DW-85) and the schedule data (Renumber Schedule, the
//! Schedule tab: DW-61, L-26, L-29).

use super::{draw_shell, Sim};
use crate::editor::opening_edit::{RENUMBER, RENUMBER_DOORS};
use crate::editor::ObjectRef;
use crate::toolbar::Action;
use crate::tools::ToolId;
use eframe::egui;
use plan_core::geometry::Point;
use plan_core::opening_symbol::{plan_symbol, PartKind};
use plan_core::{Id, Opening, OpeningKind, WallKind};

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

/// The top wall of the shell (the first one drawn) and its length.
fn top(sim: &Sim) -> (Id, f64) {
    let w = &sim.app.cx.floor().walls[0];
    (w.id, w.path_length())
}

fn cursor(sim: &Sim) -> egui::CursorIcon {
    sim.app.tools.active().cursor()
}

/// The shapes the active tool's overlay draws in one headless frame.
fn overlay_shapes(sim: &mut Sim) -> usize {
    let ctx = sim.ctx.clone();
    let mut cam = sim.app.camera;
    let out = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let (resp, painter) =
                        ui.allocate_painter(ui.available_size(), egui::Sense::hover());
                    cam.rect = resp.rect;
                    sim.app
                        .tools
                        .active()
                        .draw_overlay(&sim.app.cx, &painter, &cam);
                });
        },
    );
    out.shapes.len()
        + out
            .shapes
            .iter()
            .map(|c| match &c.shape {
                egui::Shape::Vec(v) => v.len(),
                _ => 0,
            })
            .sum::<usize>()
}

#[test]
fn the_door_tool_says_no_off_a_wall_and_shows_a_ghost_on_one() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    let before = overlay_shapes(&mut sim);
    // In the middle of the room: the "no" glyph, no ghost, nothing placed.
    sim.move_to(240.0, 180.0);
    assert_eq!(cursor(&sim), egui::CursorIcon::NotAllowed);
    assert_eq!(overlay_shapes(&mut sim), before);
    sim.click(240.0, 180.0);
    assert!(openings(&sim).is_empty());
    assert!(sim.app.cx.undo_label().is_none_or(|l| l != "Place Door"));
    // On the wall: the crosshair and a ghost.
    sim.move_to(120.0, 0.5);
    assert_eq!(cursor(&sim), egui::CursorIcon::Crosshair);
    assert!(overlay_shapes(&mut sim) > before);
    // Over the door just placed, a second one has no place: "no" again.
    sim.click(120.0, 0.5);
    assert_eq!(openings(&sim).len(), 1);
    sim.move_to(125.0, 0.5);
    assert_eq!(cursor(&sim), egui::CursorIcon::NotAllowed);
}

#[test]
fn click_then_drag_slides_the_new_door_and_undo_removes_it_whole() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    let (wall, _) = top(&sim);
    sim.drag((100.0, 0.5), (330.0, 0.5));
    let v = openings(&sim);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].wall_id, wall);
    assert!(
        (v[0].center_offset - 330.0).abs() < 1.0,
        "{}",
        v[0].center_offset
    );
    // The hinge follows the nearer end (the far half now).
    assert!(v[0].hinge_at_end);
    // One undo step for the click and the drag together.
    assert_eq!(sim.undo().as_deref(), Some("Place Door"));
    assert!(openings(&sim).is_empty());
    // A plain click still places where it was pressed.
    sim.click(100.0, 0.5);
    assert!((openings(&sim)[0].center_offset - 100.0).abs() < 1.0);
}

#[test]
fn the_center_snaps_to_the_midpoint_of_the_wall() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    let (_, len) = top(&sim);
    sim.click(len * 0.5 + 3.0, 0.5);
    let v = openings(&sim);
    // The midpoint, or the middle of the free span between the two corners
    // (the same place to a thousandth of an inch in this hand-drawn shell).
    assert!(
        (v[0].center_offset - len * 0.5).abs() < 0.05,
        "{} vs {}",
        v[0].center_offset,
        len * 0.5
    );
}

#[test]
fn a_door_beside_a_partition_keeps_its_jamb_off_the_partition_face() {
    let mut sim = house();
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((200.0, 1.0), (200.0, 200.0));
    let part = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .find(|w| w.kind == WallKind::Interior)
        .expect("partition")
        .clone();
    let (host_id, _) = top(&sim);
    let host = sim.app.cx.floor().wall(host_id).unwrap().clone();
    let s = part.start.x - host.start.x;
    let reach = part.thickness * 0.5 + 2.0;
    sim.tool(ToolId::Door);
    sim.click(s + 8.0, 0.5);
    let v = openings(&sim);
    assert_eq!(v.len(), 1, "{:?}", sim.app.cx.status);
    assert!(
        v[0].start_offset() >= s + reach - 1e-6 || v[0].end_offset() <= s - reach + 1e-6,
        "door {}..{} against the partition at {s}",
        v[0].start_offset(),
        v[0].end_offset()
    );
    // Clicked right on the partition: refused, one door only.
    sim.click(s, 0.5);
    assert_eq!(openings(&sim).len(), 1);
}

#[test]
fn windows_keep_the_minimum_separation_and_block_into_a_unit_with_the_edit_toolbar() {
    let mut sim = house();
    sim.tool(ToolId::Window);
    sim.click(100.0, 0.5);
    sim.click(135.0, 0.5);
    let mut v: Vec<Opening> = openings(&sim)
        .into_iter()
        .filter(|o| o.kind == OpeningKind::Window)
        .collect();
    assert_eq!(v.len(), 2, "{:?}", sim.app.cx.status);
    v.sort_by(|a, b| a.center_offset.total_cmp(&b.center_offset));
    // They keep the Minimum Separation (2 in) and mull on their own (Round 16).
    assert!((v[1].start_offset() - v[0].end_offset() - 2.0).abs() < 1e-9);
    // Make Mulled Unit blocks the pair (the second is selected).
    sim.app.cx.selection.set(ObjectRef::Opening(v[1].id));
    sim.action(Action::Custom(crate::editor::opening_edit::MULL));
    let m = openings(&sim);
    assert!(m.iter().all(|o| o.mull_group.is_some()), "{m:?}");
}

#[test]
fn an_exterior_door_and_a_sill_window_draw_their_plan_lines() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(100.0, 0.5);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.5);
    let (wall_id, _) = top(&sim);
    let wall = sim.app.cx.floor().wall(wall_id).unwrap().clone();
    let (door, win) = {
        let v = openings(&sim);
        (
            v.iter()
                .find(|o| o.kind == OpeningKind::Door)
                .unwrap()
                .clone(),
            v.iter()
                .find(|o| o.kind == OpeningKind::Window)
                .unwrap()
                .clone(),
        )
    };
    let ext = plan_core::exterior_sign(&wall, &sim.app.cx.rooms);
    let sym = plan_symbol(&wall, &door, ext);
    assert_eq!(
        sym.count(PartKind::Threshold),
        1,
        "threshold on an exterior door"
    );
    assert_eq!(plan_symbol(&wall, &win, ext).count(PartKind::Sill), 0);
    let base = sim.plan_shapes().len();
    // Turn the window's exterior sill on, and the door's threshold off.
    {
        let fl = sim.app.cx.floor;
        let o = sim.app.cx.project.floors[fl]
            .openings
            .iter_mut()
            .find(|o| o.id == win.id)
            .unwrap();
        o.extras.spec.sill.enabled = true;
    }
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let with_sill = sim.plan_shapes().len();
    assert!(with_sill > base, "{with_sill} vs {base}");
    {
        let fl = sim.app.cx.floor;
        let o = sim.app.cx.project.floors[fl]
            .openings
            .iter_mut()
            .find(|o| o.id == door.id)
            .unwrap();
        o.extras.spec.threshold = false;
    }
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert!(sim.plan_shapes().len() < with_sill);
}

#[test]
fn swings_both_ways_indicators_recess_and_size_without_frame_in_the_plan() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(100.0, 0.5);
    let (wall_id, _) = top(&sim);
    let wall = sim.app.cx.floor().wall(wall_id).unwrap().clone();
    let id = openings(&sim)[0].id;
    let ext = plan_core::exterior_sign(&wall, &sim.app.cx.rooms);
    let edit = |sim: &mut Sim, f: &dyn Fn(&mut Opening)| {
        let fl = sim.app.cx.floor;
        let o = sim.app.cx.project.floors[fl]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap();
        f(o);
        sim.app.cx.mark_dirty();
        sim.app.cx.refresh();
    };
    let sym = |sim: &Sim| {
        let wall = sim.app.cx.floor().wall(wall_id).unwrap();
        let o = sim
            .app
            .cx
            .floor()
            .openings
            .iter()
            .find(|o| o.id == id)
            .unwrap();
        plan_symbol(wall, o, ext)
    };
    assert_eq!(sym(&sim).count(PartKind::Swing), 1);
    // Door jambs show in plan by default (DW-82).
    assert_eq!(sym(&sim).count(PartKind::Frame), 2);
    edit(&mut sim, &|o| o.extras.spec.swings_both = true);
    assert_eq!(sym(&sim).count(PartKind::Swing), 2);
    edit(&mut sim, &|o| o.extras.spec.indicators.swing_arrows = true);
    assert_eq!(sym(&sim).count(PartKind::Indicator), 2);
    // Recessed: the hinge stands off the centerline.
    let leaf_y = |s: &plan_core::opening_symbol::OpeningSymbol| {
        s.of(PartKind::Leaf).next().unwrap().points[0].y
    };
    assert!(leaf_y(&sym(&sim)).abs() < 1e-9);
    edit(&mut sim, &|o| o.extras.spec.recess_depth = Some(0.5));
    assert!(leaf_y(&sym(&sim)).abs() > 0.5);
    // A size without the frame clears a wider opening in the wall.
    let tight = sym(&sim).span;
    edit(&mut sim, &|o| o.extras.spec.size_includes_frame = false);
    let wide = sym(&sim).span;
    assert!(
        wide.0 < tight.0 - 0.1 && wide.1 > tight.1 + 0.1,
        "{tight:?} {wide:?}"
    );
}

#[test]
fn renumber_schedule_closes_gaps_from_the_menu_and_the_toolbar() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    // Drawn right to left; the doors are alike, so a new schedule numbers
    // them in the order they were placed (p. 715).
    for x in [400.0, 250.0, 100.0] {
        sim.click(x, 0.5);
    }
    sim.esc();
    let ids: Vec<Id> = openings(&sim).iter().map(|o| o.id).collect();
    assert_eq!(ids.len(), 3);
    let sid = crate::editor::schedule_view::add(
        &mut sim.app.cx,
        plan_core::schedules::ScheduleKind::Door,
        Point::new(0.0, -300.0),
    );
    let marks = |sim: &Sim| -> Vec<(Id, String)> {
        let d = crate::editor::schedule_view::find(&sim.app.cx, sid).unwrap();
        plan_docs::schedule_kinds::rows(&sim.app.cx.project, &d, 0, None)
            .into_iter()
            .map(|e| (e.id, e.cell("mark").to_string()))
            .collect()
    };
    assert_eq!(marks(&sim)[0], (ids[0], "D01".to_string()));
    // The first door goes; Schedules > Renumber Door Schedule closes the gap.
    sim.app.cx.begin_change("Delete Door");
    sim.app.cx.project.remove_opening(0, ids[0]);
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert_eq!(marks(&sim)[0].1, "D02");
    sim.action(Action::Custom(RENUMBER_DOORS));
    assert_eq!(
        marks(&sim),
        [(ids[1], "D01".to_string()), (ids[2], "D02".to_string())]
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Renumber Schedule"));
    // No mark is written into the doors.
    assert!(openings(&sim).iter().all(|o| o.schedule_number.is_none()));
    // The plan labels read the schedule's marks.
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let labels = crate::editor::opening_view::opening_labels(&sim.app.cx);
    let first = labels.iter().find(|l| l.opening == ids[1]).unwrap();
    assert!(first.text.contains("D01") || !first.is_mark, "{first:?}");
    // The Edit toolbar's command is one undo step too.
    sim.undo();
    assert_eq!(marks(&sim)[0].1, "D02");
    sim.app.cx.selection.set(ObjectRef::Opening(ids[1]));
    sim.action(Action::Custom(RENUMBER));
    assert_eq!(marks(&sim)[0].1, "D01");
    assert_eq!(sim.undo().as_deref(), Some("Renumber Schedule"));
    assert_eq!(marks(&sim)[0].1, "D02");
}

#[test]
fn the_schedule_lists_supplier_data_and_leaves_out_what_the_tab_excludes() {
    use plan_core::schedules::{Schedule, ScheduleKind};
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(100.0, 0.5);
    sim.click(300.0, 0.5);
    let ids: Vec<Id> = openings(&sim).iter().map(|o| o.id).collect();
    {
        let fl = sim.app.cx.floor;
        let o = &mut sim.app.cx.project.floors[fl].openings[0];
        o.extras.spec.schedule.supplier = "Acme Millwork".into();
        o.extras.spec.schedule.comment = "Primed".into();
        let o2 = &mut sim.app.cx.project.floors[fl].openings[1];
        o2.extras.spec.schedule.include = false;
    }
    let mut def = Schedule::new(ScheduleKind::Door, Point::ZERO);
    for field in ["supplier", "comment"] {
        def.columns
            .iter_mut()
            .find(|c| c.field == field)
            .unwrap()
            .visible = true;
    }
    let t = plan_docs::schedule_kinds::table(&sim.app.cx.project, &def, 0, None);
    assert_eq!(t.rows.len(), 1, "{:?}", t.rows);
    assert!(t.rows[0].iter().any(|c| c == "Acme Millwork"));
    assert!(t.rows[0].iter().any(|c| c == "Primed"));
    let _ = ids;
}
