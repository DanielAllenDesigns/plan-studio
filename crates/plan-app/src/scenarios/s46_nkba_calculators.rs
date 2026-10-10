//! Scenario 46: NKBA kitchen and bath checks and the structural calculators
//! (coverage audit Top 40 #38). Plan Check holds named kitchens and baths to
//! the NKBA guidelines (its own "NKBA" group with its own switch) and Tools >
//! Checks > Kitchen and Bath Report lists every guideline; Tools > Calculators
//! opens the Header/Beam, Joist Span, Rafter Span, Stair and Deck calculators,
//! whose Apply buttons write into the plan as one undo step each.

use super::{draw_shell, Sim};
use crate::dialogs::calculators as calc;
use crate::dialogs::plan_check::{self as pc, CheckKind};
use crate::editor::stairs_view::{self, StairKind};
use crate::editor::{framing_view, ObjectRef};
use crate::toolbar::Action;
use crate::tools::ToolId;
use plan_check::{CheckSettings, NkbaStatus};
use plan_core::deck::{DeckSpec, DECK_ROOM_TYPE};
use plan_core::geometry::Point;
use plan_core::{Id, RoomName, Wall, WallClass, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    calc::close();
    calc::edit(|s| *s = calc::State::default());
    sim
}

fn name_room(sim: &mut Sim, name: &str, ty: &str) {
    let at = Point::new(W / 2.0, H / 2.0);
    sim.app
        .cx
        .floor_mut()
        .room_names
        .push(RoomName::new(at, name, ty));
    sim.app.cx.refresh();
}

fn door(sim: &mut Sim, x: f64) -> Id {
    sim.tool(ToolId::Door);
    sim.click(x, 0.5);
    sim.esc();
    sim.tool(ToolId::Select);
    sim.app
        .cx
        .floor()
        .openings
        .iter()
        .max_by_key(|o| o.id)
        .map(|o| o.id)
        .expect("a door")
}

/// A 28 in door in the kitchen wall: under the 32 in clear the NKBA asks for.
fn narrow_door(sim: &mut Sim) {
    let id = door(sim, 200.0);
    let op = sim
        .app
        .cx
        .floor_mut()
        .openings
        .iter_mut()
        .find(|o| o.id == id)
        .unwrap();
    op.width = 28.0;
    sim.app.cx.refresh();
}

// ----- NKBA in Plan Check -----

#[test]
fn an_empty_named_kitchen_breaks_nkba_guidelines_and_the_findings_zoom() {
    let mut sim = house();
    name_room(&mut sim, "Kitchen", "Kitchen");
    narrow_door(&mut sim);
    let run = pc::run_check_full(&mut sim.app.cx, CheckKind::Plan);
    let nkba: Vec<_> = run
        .findings
        .iter()
        .filter(|f| f.rule.starts_with("NKBA"))
        .collect();
    assert!(!nkba.is_empty(), "an empty kitchen fails NKBA guidelines");
    assert!(nkba
        .iter()
        .all(|f| f.location.is_some() || f.object.is_some()));
}

#[test]
fn an_unnamed_room_is_not_judged_by_nkba() {
    let mut sim = house();
    let run = pc::run_check_full(&mut sim.app.cx, CheckKind::Plan);
    assert!(!run.findings.iter().any(|f| f.rule.starts_with("NKBA")));
}

#[test]
fn the_nkba_group_switch_in_the_plan_settings_silences_it() {
    let mut sim = house();
    name_room(&mut sim, "Kitchen", "Kitchen");
    narrow_door(&mut sim);
    let before = pc::run_check_full(&mut sim.app.cx, CheckKind::Plan);
    assert!(before.findings.iter().any(|f| f.rule.starts_with("NKBA")));
    let mut settings = CheckSettings::load(&sim.app.cx.project);
    assert!(settings.group_enabled("NKBA"));
    settings.set_group_enabled("NKBA", false);
    settings.store(&mut sim.app.cx.project);
    let run = pc::run_check_full(&mut sim.app.cx, CheckKind::Plan);
    assert!(!run.findings.iter().any(|f| f.rule.starts_with("NKBA")));
    let report = pc::nkba_report(&mut sim.app.cx);
    assert!(!report.rows.is_empty());
    assert!(
        report.rows.iter().all(|r| r.status != NkbaStatus::NotMet),
        "switched-off rules are not reported as failures"
    );
}

#[test]
fn the_kitchen_and_bath_report_opens_from_the_checks_menu_and_exports() {
    let mut sim = house();
    name_room(&mut sim, "Kitchen", "Kitchen");
    narrow_door(&mut sim);
    sim.action(Action::Custom(pc::NKBA_REPORT));
    assert!(pc::text_report_open());
    assert!(
        sim.app.cx.status.starts_with("NKBA:"),
        "{}",
        sim.app.cx.status
    );
    let report = pc::nkba_report(&mut sim.app.cx);
    let (met, not_met) = report.counts();
    assert!(met + not_met > 0);
    assert!(report.markdown().contains("## Kitchen"));
    let sched = pc::nkba_schedule(&report, "First Floor");
    assert_eq!(sched.columns.len(), 5);
    assert_eq!(sched.rows.len(), report.rows.len());
    let xlsx = sched.to_xlsx();
    assert_eq!(&xlsx[..2], b"PK", "an xlsx is a zip");
}

#[test]
fn a_plan_without_kitchen_or_bath_has_an_empty_report() {
    let mut sim = house();
    let report = pc::nkba_report(&mut sim.app.cx);
    assert!(report.rows.is_empty());
    assert!(report.markdown().contains("No kitchen or bathroom"));
}

// ----- the Calculators menu -----

#[test]
fn each_calculators_menu_command_opens_its_tab() {
    let mut sim = house();
    for (id, tab) in [
        (calc::HEADER, calc::Tab::Header),
        (calc::JOIST, calc::Tab::Joist),
        (calc::RAFTER, calc::Tab::Rafter),
        (calc::STAIR, calc::Tab::Stair),
        (calc::DECK, calc::Tab::Deck),
    ] {
        calc::close();
        sim.action(Action::Custom(id));
        assert!(calc::is_open(), "{id} opens the window");
        assert_eq!(calc::snapshot().tab, tab);
    }
}

#[test]
fn the_calculator_window_draws_every_tab() {
    let mut sim = house();
    for tab in calc::Tab::ALL {
        calc::edit(|s| {
            s.open = true;
            s.tab = tab;
        });
        for _ in 0..3 {
            let ctx = sim.ctx.clone();
            let cx = &mut sim.app.cx;
            let _ = ctx.run(
                eframe::egui::RawInput {
                    screen_rect: Some(eframe::egui::Rect::from_min_size(
                        eframe::egui::Pos2::ZERO,
                        eframe::egui::vec2(1400.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ctx| calc::show_all(ctx, cx),
            );
        }
        assert!(calc::is_open());
    }
}

// ----- Apply to selected -----

#[test]
fn header_apply_sizes_the_selected_openings_from_their_own_width_in_one_undo_step() {
    let mut sim = house();
    let a = door(&mut sim, 120.0);
    let b = door(&mut sim, 300.0);
    sim.app.cx.selection.set(ObjectRef::Opening(a));
    sim.app.cx.selection.items.push(ObjectRef::Opening(b));
    // Make the second opening much wider than the first.
    sim.app
        .cx
        .floor_mut()
        .openings
        .iter_mut()
        .find(|o| o.id == b)
        .unwrap()
        .width = 96.0;
    sim.action(Action::Custom(calc::HEADER));
    let undo_before = sim.app.cx.undo_depth();
    let msg = calc::apply_current(&mut sim.app.cx);
    assert!(msg.contains("Sized the headers of 2"), "{msg}");
    assert_eq!(sim.app.cx.undo_depth(), undo_before + 1);
    let get = |sim: &Sim, id: Id| {
        sim.app
            .cx
            .floor()
            .openings
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .extras
            .spec
            .framing
            .clone()
    };
    let (fa, fb) = (get(&sim, a), get(&sim, b));
    assert!(fa.include_header && fb.include_header);
    assert!(fb.header_depth.unwrap() >= fa.header_depth.unwrap());
    assert!(fb.trimmers.unwrap() >= fa.trimmers.unwrap());
    sim.undo();
    assert_ne!(get(&sim, b), fb, "one undo reverts both headers");
}

#[test]
fn header_apply_needs_a_selected_opening() {
    let mut sim = house();
    sim.action(Action::Custom(calc::HEADER));
    let msg = calc::apply_current(&mut sim.app.cx);
    assert_eq!(msg, "Select a door or window first");
}

#[test]
fn header_use_selection_takes_the_opening_width() {
    let mut sim = house();
    let a = door(&mut sim, 200.0);
    sim.app
        .cx
        .floor_mut()
        .openings
        .iter_mut()
        .find(|o| o.id == a)
        .unwrap()
        .width = 60.0;
    sim.app.cx.selection.set(ObjectRef::Opening(a));
    sim.action(Action::Custom(calc::HEADER));
    assert_eq!(calc::snapshot().header.span, 60.0);
}

#[test]
fn joist_apply_sets_the_floor_framing_defaults_in_one_undo_step() {
    let mut sim = house();
    calc::edit(|s| {
        s.tab = calc::Tab::Joist;
        s.joist_span = 14.0 * 12.0;
        s.joist.spacing = 12.0;
    });
    let before = framing_view::settings(&sim.app.cx.project);
    let n = sim.app.cx.undo_depth();
    let msg = calc::apply_current(&mut sim.app.cx);
    assert!(msg.starts_with("Floor joists 2x"), "{msg}");
    assert_eq!(sim.app.cx.undo_depth(), n + 1);
    let after = framing_view::settings(&sim.app.cx.project);
    assert_eq!(after.walls.joist_spacing, 12.0);
    assert!(
        after.walls.joist_size.depth >= 7.0,
        "a 14 ft span needs a 2x8 or deeper"
    );
    sim.undo();
    let undone = framing_view::settings(&sim.app.cx.project);
    assert_eq!(undone.walls.joist_spacing, before.walls.joist_spacing);
}

#[test]
fn a_joist_span_beyond_the_table_is_refused_without_touching_the_plan() {
    let mut sim = house();
    calc::edit(|s| {
        s.tab = calc::Tab::Joist;
        s.joist_span = 40.0 * 12.0;
    });
    let n = sim.app.cx.undo_depth();
    let msg = calc::apply_current(&mut sim.app.cx);
    assert!(msg.starts_with("No joist in the table"), "{msg}");
    assert_eq!(sim.app.cx.undo_depth(), n);
}

#[test]
fn rafter_apply_sets_the_roof_framing_defaults() {
    let mut sim = house();
    calc::edit(|s| {
        s.tab = calc::Tab::Rafter;
        s.rafter_run = 12.0 * 12.0;
        s.rafter.spacing = 24.0;
    });
    let n = sim.app.cx.undo_depth();
    let msg = calc::apply_current(&mut sim.app.cx);
    assert!(msg.starts_with("Rafters 2x"), "{msg}");
    assert_eq!(sim.app.cx.undo_depth(), n + 1);
    assert_eq!(
        framing_view::settings(&sim.app.cx.project).roof.spacing,
        24.0
    );
}

#[test]
fn stair_apply_gives_the_selected_stair_its_rise_risers_and_tread() {
    let mut sim = house();
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((100.0, 100.0), (250.0, 100.0));
    sim.tool(ToolId::Select);
    let id = stairs_view::load(sim.app.cx.floor())[0].id();
    sim.app.cx.selection.set(ObjectRef::Stair(id));
    sim.action(Action::Custom(calc::STAIR));
    calc::edit(|s| {
        s.stair.total_rise = 105.0;
        s.stair.target_riser = 7.0;
        s.stair.tread = 10.0;
    });
    let n = sim.app.cx.undo_depth();
    let msg = calc::apply_current(&mut sim.app.cx);
    assert!(msg.starts_with("Stair: 15 risers"), "{msg}");
    assert_eq!(sim.app.cx.undo_depth(), n + 1);
    let o = stairs_view::find(sim.app.cx.floor(), id).unwrap();
    assert_eq!(o.stair.params.total_rise, 105.0);
    assert_eq!(o.stair.params.tread_depth, 10.0);
    sim.undo();
    let o = stairs_view::find(sim.app.cx.floor(), id).unwrap();
    assert_ne!(o.stair.params.total_rise, 105.0);
}

#[test]
fn stair_apply_needs_a_selected_stair() {
    let mut sim = house();
    sim.action(Action::Custom(calc::STAIR));
    assert_eq!(calc::apply_current(&mut sim.app.cx), "Select a stair first");
}

#[test]
fn deck_apply_writes_the_deck_specification_of_the_selected_deck_room() {
    let mut sim = Sim::new();
    let pts = [(0.0, 0.0), (192.0, 0.0), (192.0, 96.0), (0.0, 96.0)];
    for i in 0..4 {
        let (a, b) = (pts[i], pts[(i + 1) % 4]);
        let mut w = if i == 0 {
            Wall::new(
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.5,
                109.0,
                WallKind::Exterior,
            )
        } else {
            let mut w = Wall::new(
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                1.5,
                36.0,
                WallKind::Exterior,
            );
            w.class = WallClass::DeckEdge;
            w.is_deck_edge = true;
            w
        };
        w.id = sim.app.cx.project.alloc_id();
        sim.app.cx.floor_mut().walls.push(w);
    }
    let mut name = RoomName::new(Point::new(96.0, 48.0), "Deck", DECK_ROOM_TYPE);
    name.has_ceiling = false;
    name.deck = Some(DeckSpec::default());
    sim.app.cx.floor_mut().room_names.push(name);
    sim.app.cx.refresh();
    calc::close();
    calc::edit(|s| *s = calc::State::default());
    crate::editor::rooms_edit::select_room(&mut sim.app.cx, 0);
    sim.action(Action::Custom(calc::DECK));
    calc::edit(|s| {
        s.deck.joist_nominal = 10;
        s.deck.joist_spacing = 12.0;
        s.deck.beam_nominal = 10;
        s.deck.beam_plies = 3;
    });
    let n = sim.app.cx.undo_depth();
    let msg = calc::apply_current(&mut sim.app.cx);
    assert!(msg.starts_with("Deck joists 2x10 at 12\""), "{msg}");
    assert_eq!(sim.app.cx.undo_depth(), n + 1);
    let f = sim.app.cx.floor().room_names[0]
        .deck
        .as_ref()
        .unwrap()
        .framing
        .clone();
    assert_eq!(f.joist_size, "2x10");
    assert_eq!(f.joist_spacing, 12.0);
    assert_eq!(f.beam_size, "2x10");
    assert_eq!(f.beam_plies, 3);
    assert!(f.post_spacing >= 24.0);
}

#[test]
fn deck_apply_needs_a_deck_room() {
    let mut sim = house();
    name_room(&mut sim, "Kitchen", "Kitchen");
    crate::editor::rooms_edit::select_room(&mut sim.app.cx, 0);
    sim.action(Action::Custom(calc::DECK));
    assert_eq!(
        calc::apply_current(&mut sim.app.cx),
        "The selected room is not a deck"
    );
}
