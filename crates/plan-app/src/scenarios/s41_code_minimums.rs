//! Scenario 41: code minimums in the tools. The IRC figures of Plan Check
//! are the minimums the app holds the plan to: new plans start at code-legal
//! values, the dialogs warn under a field that is past a limit and "Set to
//! code" fixes it, the preset changes the limits, and the live check keeps a
//! count of the findings the last edit left.
//!
//! The dialogs are driven through headless egui frames: the notice lines are
//! read from the painted text and "Set to code" is clicked with pointer
//! events.

use super::{draw_shell, Sim};
use crate::dialogs::stairs::StairDialog;
use crate::dialogs::OpeningDialog;
use crate::editor::code;
use crate::editor::stairs_view::{self, StairKind};
use crate::tools::ToolId;
use eframe::egui;
use plan_check::{CheckSettings, CodeMinimums, JURISDICTIONS};
use plan_core::{OpeningKind, PlanDefaults, RoomName};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    sim
}

// ----- headless frames -----

/// Painted text and where it is.
type Texts = Vec<(String, egui::Rect)>;

fn collect(shape: &egui::Shape, out: &mut Texts) {
    match shape {
        egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
        egui::Shape::Text(t) => out.push((
            t.galley.text().to_string(),
            egui::Rect::from_min_size(t.pos, t.galley.size()),
        )),
        _ => {}
    }
}

fn frame(
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    mut f: impl FnMut(&egui::Context),
) -> Texts {
    let out = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| f(ctx),
    );
    let mut texts = Vec::new();
    for cs in &out.shapes {
        collect(&cs.shape, &mut texts);
    }
    texts
}

/// Lays a dialog out (a few frames) and returns the text it paints.
fn settle(ctx: &egui::Context, f: &mut impl FnMut(&egui::Context)) -> Texts {
    let mut texts = Vec::new();
    for _ in 0..4 {
        texts = frame(ctx, Vec::new(), &mut *f);
    }
    texts
}

fn has(texts: &Texts, needle: &str) -> bool {
    texts.iter().any(|(t, _)| t.contains(needle))
}

/// Clicks the "Set to code" button on the same line as the notice that
/// contains `notice`.
fn click_set_to_code(ctx: &egui::Context, f: &mut impl FnMut(&egui::Context), notice: &str) {
    let texts = settle(ctx, f);
    let line = texts
        .iter()
        .find(|(t, _)| t.contains(notice))
        .unwrap_or_else(|| {
            panic!(
                "no notice {notice:?} in {:?}",
                texts.iter().map(|t| &t.0).collect::<Vec<_>>()
            )
        })
        .1;
    let button = texts
        .iter()
        .filter(|(t, _)| t == "Set to code")
        .min_by(|a, b| {
            (a.1.center().y - line.center().y)
                .abs()
                .total_cmp(&(b.1.center().y - line.center().y).abs())
        })
        .expect("a Set to code button")
        .1;
    let at = button.center();
    let press = |down| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: down,
        modifiers: egui::Modifiers::NONE,
    };
    frame(ctx, vec![egui::Event::PointerMoved(at)], &mut *f);
    frame(ctx, vec![press(true)], &mut *f);
    frame(ctx, vec![press(false)], &mut *f);
    settle(ctx, f);
}

fn draw_stair(sim: &mut Sim) -> stairs_view::StairObj {
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((100.0, 100.0), (250.0, 100.0));
    sim.tool(ToolId::Select);
    stairs_view::load(sim.app.cx.floor())[0].clone()
}

// ----- defaults -----

#[test]
fn a_new_plan_starts_at_code_legal_defaults() {
    let sim = Sim::new();
    let d = &sim.app.cx.defaults;
    let m = CodeMinimums::default();
    // Stairs.
    assert!(d.code.stair_riser <= m.stair_riser_max);
    assert!(d.code.stair_tread >= m.stair_tread_min);
    assert!(d.code.stair_width >= m.stair_width_min);
    assert!(d.code.stair_headroom >= m.stair_headroom_min);
    // Railings: guard 36, handrail 36 within 34-38, balusters within a 4" sphere.
    assert!(d.code.guard_height >= 36.0);
    assert!(d.wall_variants.railing_height >= 36.0);
    assert!((34.0..=38.0).contains(&d.code.handrail_height));
    assert!(d.code.baluster_opening <= 4.0);
    // The bedroom window is an egress window.
    let w = &d.code.bedroom_window;
    let mut op = plan_core::Opening::new(
        1,
        0.0,
        OpeningKind::Window,
        w.width,
        w.height,
        w.sill_height,
    );
    op.style = plan_core::OpeningStyle::Casement;
    assert!(m.egress_shortfalls(&op, false).is_empty());
    assert!(w.egress);
    assert!(w.width >= 36.0 && w.height >= 60.0 || w.width * w.height >= 5.7 * 144.0);
    // The exterior door is at least 36 x 80; the interior door is untouched.
    assert!(d.exterior_door.width >= 36.0 && d.exterior_door.height >= 80.0);
    assert_eq!(
        d.interior_door.width,
        PlanDefaults::chief_x18_daniel().interior_door.width
    );
    // Footing: width from the table, at least 6" thick.
    assert!(d.code.footing_width >= m.footing_width(2));
    assert!(d.code.footing_thickness >= 6.0);
    // The garage wall: 5/8" gypsum on both faces.
    let t = d
        .wall_type(&d.code.garage_wall_type)
        .expect("garage wall type");
    assert_eq!(t.layers.first().unwrap().thickness, 0.625);
    assert_eq!(t.layers.last().unwrap().thickness, 0.625);
    assert!(t.layers.first().unwrap().material.contains("Type X"));
    // The ceiling height of the template is untouched (8'-1 1/8").
    let untouched = plan_core::Project::from_defaults("x", &PlanDefaults::chief_x18_daniel());
    assert_eq!(
        sim.app.cx.floor().ceiling_height,
        untouched.floors[0].ceiling_height
    );
}

#[test]
fn applying_the_minimums_raises_illegal_defaults_once() {
    let m = CodeMinimums::default();
    let mut d = PlanDefaults::chief_x18_daniel();
    d.code.stair_riser = 8.5;
    d.code.stair_tread = 9.0;
    d.code.stair_width = 30.0;
    d.code.stair_headroom = 76.0;
    d.code.guard_height = 30.0;
    d.wall_variants.railing_height = 30.0;
    d.code.handrail_height = 42.0;
    d.code.baluster_opening = 6.0;
    d.code.bedroom_window.width = 20.0;
    d.code.bedroom_window.height = 20.0;
    d.code.bedroom_window.sill_height = 54.0;
    d.exterior_door.width = 30.0;
    d.exterior_door.height = 78.0;
    d.code.footing_width = 8.0;
    d.code.footing_thickness = 4.0;
    let changed = code::apply_code_minimums(&mut d, &m);
    assert!(changed.len() >= 10, "{changed:?}");
    assert_eq!(d.code.stair_riser, 7.75);
    assert_eq!(d.code.stair_tread, 10.0);
    assert_eq!(d.code.stair_width, 36.0);
    assert_eq!(d.code.stair_headroom, 80.0);
    assert_eq!(d.code.guard_height, 36.0);
    assert_eq!(d.wall_variants.railing_height, 36.0);
    assert_eq!(d.code.handrail_height, 38.0);
    assert_eq!(d.code.baluster_opening, 4.0);
    let w = &d.code.bedroom_window;
    assert!(w.width >= 20.0 && w.height >= 24.0 && w.width * w.height >= 5.7 * 144.0);
    assert_eq!(w.sill_height, 44.0);
    assert_eq!(
        (d.exterior_door.width, d.exterior_door.height),
        (36.0, 80.0)
    );
    assert_eq!(d.code.footing_width, 15.0);
    assert_eq!(d.code.footing_thickness, 6.0);
    // A second run changes nothing.
    assert!(code::apply_code_minimums(&mut d, &m).is_empty());
    // A 96" door stays 96".
    let mut t = PlanDefaults::chief_x18_daniel();
    code::apply_code_minimums(&mut t, &m);
    assert_eq!(t.exterior_door.height, 96.0);
}

#[test]
fn the_preference_decides_whether_a_new_plan_is_seeded() {
    crate::dialogs::preferences::pages::update(|p| p.architectural.seed_code_defaults = false);
    let mut d = PlanDefaults::chief_x18_daniel();
    d.code.stair_riser = 8.5;
    assert!(code::seed_new_plan(&mut d).is_empty());
    assert_eq!(d.code.stair_riser, 8.5);
    crate::dialogs::preferences::pages::update(|p| p.architectural.seed_code_defaults = true);
    assert!(!code::seed_new_plan(&mut d).is_empty());
    assert_eq!(d.code.stair_riser, 7.75);
}

#[test]
fn file_new_seeds_the_defaults_again() {
    let mut sim = Sim::new();
    sim.app.cx.defaults.code.stair_riser = 9.0;
    sim.app.cx.defaults.code.guard_height = 20.0;
    sim.app.new_project();
    assert_eq!(sim.app.cx.defaults.code.stair_riser, 7.75);
    assert_eq!(sim.app.cx.defaults.code.guard_height, 36.0);
}

#[test]
fn the_apply_button_raises_the_defaults_in_one_undo_step() {
    let mut sim = house();
    sim.app.cx.defaults.code.stair_width = 30.0;
    let before = sim.app.cx.undo_label().map(str::to_string);
    let status = code::apply_to_defaults(&mut sim.app.cx);
    assert!(status.contains("stair width"), "{status}");
    assert_eq!(sim.app.cx.defaults.code.stair_width, 36.0);
    assert_eq!(
        sim.app.cx.undo_label(),
        Some("Apply Code Minimums to Defaults")
    );
    sim.undo();
    assert_eq!(sim.app.cx.undo_label().map(str::to_string), before);
    assert!(code::apply_to_defaults(&mut sim.app.cx).contains("already meet"));
}

#[test]
fn a_menu_command_applies_the_minimums_and_toggles_the_live_check() {
    let mut sim = house();
    sim.app.cx.defaults.code.stair_tread = 8.0;
    assert!(crate::editor::placed::run_command(
        &mut sim.app.cx,
        crate::dialogs::plan_check::APPLY_DEFAULTS
    ));
    assert_eq!(sim.app.cx.defaults.code.stair_tread, 10.0);
    let on = crate::dialogs::preferences::pages::current()
        .architectural
        .check_while_drawing;
    assert!(crate::editor::placed::run_command(
        &mut sim.app.cx,
        crate::dialogs::plan_check::CHECK_LIVE
    ));
    let now = crate::dialogs::preferences::pages::current()
        .architectural
        .check_while_drawing;
    assert_ne!(on, now);
    assert!(sim.app.cx.status.contains("Check while drawing"));
}

// ----- the stair tool -----

#[test]
fn a_new_stair_starts_at_the_plans_minimums() {
    let mut sim = house();
    let mut s = CheckSettings::irc_2021();
    s.options.riser_max = 7.0;
    s.name_from_limits();
    s.store(&mut sim.app.cx.project);
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    code::publish(&sim.app.cx);
    let o = draw_stair(&mut sim);
    assert!(o.stair.params.riser_height_target <= 7.0 + 1e-9);
    assert!(o.stair.params.width >= 36.0);
    assert!(o.stair.params.headroom_min >= 80.0);
    assert!(o.stair.params.railing.height >= 34.0);
}

// ----- the notices -----

#[test]
fn a_riser_of_eight_inches_shows_the_notice_and_set_to_code_fixes_it_in_one_undo_step() {
    let mut sim = house();
    let o = draw_stair(&mut sim);
    code::publish(&sim.app.cx);
    // Store the 8" riser the way the dialog's OK does.
    let mut d = StairDialog::new(o.clone());
    d.draft_mut().stair.params.riser_height_target = 8.0;
    assert!(stairs_view::apply_edit(&mut sim.app.cx, d.draft()));
    let stored = stairs_view::find(sim.app.cx.floor(), o.id()).unwrap();
    assert_eq!(stored.stair.params.riser_height_target, 8.0);

    // Open the dialog on it: the notice is there, with the citation.
    let ctx = egui::Context::default();
    let mut d = StairDialog::new(stored.clone());
    let mut show = |ctx: &egui::Context| {
        d.show(ctx);
    };
    let texts = settle(&ctx, &mut show);
    assert!(
        has(&texts, "IRC R311.7.5.1 riser height: max 7 3/4\""),
        "{:?}",
        texts.iter().map(|t| &t.0).collect::<Vec<_>>()
    );
    assert!(
        !has(&texts, "IRC R311.7.5.2 tread depth"),
        "the tread is legal"
    );

    click_set_to_code(&ctx, &mut show, "IRC R311.7.5.1 riser height");
    drop(show);
    assert_eq!(d.draft().stair.params.riser_height_target, 7.75);
    // The notice is gone once the field is legal.
    let mut show = |ctx: &egui::Context| {
        d.show(ctx);
    };
    assert!(!has(
        &settle(&ctx, &mut show),
        "IRC R311.7.5.1 riser height"
    ));
    drop(show);

    // OK: one undo step, and it undoes back to the 8" stair.
    let steps = sim.app.cx.undo_label().map(str::to_string);
    assert!(stairs_view::apply_edit(&mut sim.app.cx, d.draft()));
    assert_eq!(sim.app.cx.undo_label(), Some("Stair Specification"));
    assert_eq!(steps.as_deref(), Some("Stair Specification"));
    sim.undo();
    let back = stairs_view::find(sim.app.cx.floor(), o.id()).unwrap();
    assert_eq!(back.stair.params.riser_height_target, 8.0);
}

#[test]
fn the_tread_width_and_headroom_notices_follow_their_limits() {
    let mut sim = house();
    let o = draw_stair(&mut sim);
    code::publish(&sim.app.cx);
    let mut bad = o.clone();
    bad.stair.params.tread_depth = 9.0;
    bad.stair.params.width = 30.0;
    bad.stair.params.headroom_min = 72.0;
    let ctx = egui::Context::default();
    let mut d = StairDialog::new(bad);
    let mut show = |ctx: &egui::Context| {
        d.show(ctx);
    };
    let texts = settle(&ctx, &mut show);
    for n in [
        "IRC R311.7.1 stair width: min 3'-0\"",
        "IRC R311.7.5.2 tread depth: min 10\"",
        "IRC R311.7.2 headroom: min 6'-8\"",
    ] {
        assert!(
            has(&texts, n),
            "{n}: {:?}",
            texts.iter().map(|t| &t.0).collect::<Vec<_>>()
        );
    }
    click_set_to_code(&ctx, &mut show, "IRC R311.7.5.2 tread depth");
    drop(show);
    assert_eq!(d.draft().stair.params.tread_depth, 10.0);
    assert_eq!(
        d.draft().stair.params.width,
        30.0,
        "only the clicked field moves"
    );
}

#[test]
fn the_preset_changes_the_minimums_the_dialogs_hold_to() {
    let mut sim = house();
    code::publish(&sim.app.cx);
    assert_eq!(code::active().stair_riser_max, 7.75);
    assert_eq!(code::active().label(), "IRC 2021");

    // Georgia 2020: a different edition.
    CheckSettings::preset(JURISDICTIONS[1])
        .unwrap()
        .store(&mut sim.app.cx.project);
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    code::publish(&sim.app.cx);
    assert_eq!(code::active().code_year, 2018);
    assert_eq!(code::active().label(), "IRC 2018 (Georgia)");

    // A stricter riser limit changes the notice of a 7.5" riser.
    let mut s = CheckSettings::irc_2021();
    s.options.riser_max = 7.0;
    s.name_from_limits();
    s.store(&mut sim.app.cx.project);
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    code::publish(&sim.app.cx);
    assert_eq!(code::active().stair_riser_max, 7.0);
    let o = draw_stair(&mut sim);
    let mut o7 = o.clone();
    o7.stair.params.riser_height_target = 7.5;
    let ctx = egui::Context::default();
    let mut d = StairDialog::new(o7);
    let mut show = |ctx: &egui::Context| {
        d.show(ctx);
    };
    assert!(has(
        &settle(&ctx, &mut show),
        "IRC R311.7.5.1 riser height: max 7\""
    ));
}

#[test]
fn a_bedroom_window_below_egress_shows_the_notice_and_set_to_code_fixes_the_sill() {
    let mut sim = house();
    sim.app.cx.floor_mut().room_names.push(RoomName::new(
        plan_core::geometry::Point::new(240.0, 180.0),
        "Bedroom",
        "Bedroom",
    ));
    sim.tool(ToolId::Window);
    sim.click(240.0, 0.0);
    sim.tool(ToolId::Select);
    let id = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.kind == OpeningKind::Window)
        .unwrap()
        .id;
    {
        let o = sim
            .app
            .cx
            .floor_mut()
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap();
        o.width = 24.0;
        o.height = 24.0;
        o.sill_height = 50.0;
    }
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    code::publish(&sim.app.cx);
    assert!(
        code::window_in_sleeping_room(id),
        "the window is in the bedroom"
    );

    let op = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == id)
        .unwrap()
        .clone();
    let wall = sim.app.cx.floor().wall(op.wall_id).unwrap().clone();
    let ctx = egui::Context::default();
    let mut d = OpeningDialog::for_opening(op, &wall, Vec::new(), Default::default());
    let mut show = |ctx: &egui::Context| {
        d.show(ctx);
    };
    let texts = settle(&ctx, &mut show);
    let all: Vec<_> = texts.iter().map(|t| t.0.clone()).collect();
    assert!(has(&texts, "IRC R310.2.1 net clear opening"), "{all:?}");
    // The house is on the grade floor: the smaller 5.0 sq ft applies.
    assert!(has(&texts, "min 5.0 sq ft"), "{all:?}");
    assert!(
        has(&texts, "IRC R310.2.2 sill height: max 3'-8\""),
        "{all:?}"
    );
    click_set_to_code(&ctx, &mut show, "IRC R310.2.2 sill height");
    drop(show);
    assert_eq!(d.draft().sill_height, 44.0);
    // The size notices are still there; click the area one.
    let mut show = |ctx: &egui::Context| {
        d.show(ctx);
    };
    click_set_to_code(&ctx, &mut show, "IRC R310.2.1 net clear opening");
    drop(show);
    let o = d.draft();
    assert!(
        o.width * o.height >= 5.0 * 144.0,
        "{}x{}",
        o.width,
        o.height
    );
}

#[test]
fn a_window_outside_a_bedroom_gets_no_egress_notice() {
    let mut sim = house();
    sim.app.cx.floor_mut().room_names.push(RoomName::new(
        plan_core::geometry::Point::new(240.0, 180.0),
        "Living",
        "Living",
    ));
    sim.tool(ToolId::Window);
    sim.click(240.0, 0.0);
    sim.tool(ToolId::Select);
    let id = sim.app.cx.floor().openings[0].id;
    sim.app.cx.floor_mut().openings[0].sill_height = 50.0;
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    code::publish(&sim.app.cx);
    assert!(!code::window_in_sleeping_room(id));
}

#[test]
fn the_exit_door_is_held_to_its_width_and_height() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Select);
    let id = sim.app.cx.floor().openings[0].id;
    {
        let o = &mut sim.app.cx.floor_mut().openings[0];
        o.width = 30.0;
        o.height = 78.0;
    }
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    code::publish(&sim.app.cx);
    assert!(code::door_is_required_egress(id));
    let op = sim.app.cx.floor().openings[0].clone();
    let wall = sim.app.cx.floor().wall(op.wall_id).unwrap().clone();
    let ctx = egui::Context::default();
    let mut d = OpeningDialog::for_opening(op, &wall, Vec::new(), Default::default());
    let mut show = |ctx: &egui::Context| {
        d.show(ctx);
    };
    let texts = settle(&ctx, &mut show);
    assert!(has(
        &texts,
        "IRC R311.2 egress door width (leaf): min 3'-0\""
    ));
    assert!(has(&texts, "IRC R311.2 egress door height: min 6'-8\""));
    click_set_to_code(&ctx, &mut show, "egress door width");
    drop(show);
    assert_eq!(d.draft().width, 36.0);
}

// ----- Plan Check: Fix and the live count -----

#[test]
fn the_live_check_counts_a_stair_edit_and_fix_clears_it_in_one_undo_step() {
    let mut sim = house();
    let o = draw_stair(&mut sim);
    crate::dialogs::preferences::pages::update(|p| p.architectural.check_while_drawing = true);
    code::frame(&mut sim.app.cx);
    let clean = code::live_summary();
    assert!(clean.runs >= 1);

    // A 30" wide stair with 9" treads and low headroom.
    let mut bad = o.clone();
    bad.stair.params.width = 30.0;
    bad.stair.params.tread_depth = 9.0;
    bad.stair.params.headroom_min = 72.0;
    assert!(stairs_view::apply_edit(&mut sim.app.cx, &bad));
    sim.app.cx.refresh();
    code::frame(&mut sim.app.cx);
    let dirty = code::live_summary();
    assert!(
        dirty.errors >= clean.errors + 3,
        "{} errors after, {} before",
        dirty.errors,
        clean.errors
    );
    assert!(dirty.rerun[0], "the stair group was re-counted");
    assert!(!dirty.rerun[3], "the foundation group was not");
    assert!(dirty.groups[0] > clean.groups[0]);
    assert_eq!(code::badge_count(), dirty.errors + dirty.warnings);
    let status = code::status_text(&sim.app.cx).unwrap();
    assert!(status.starts_with("Live check: "), "{status}");

    // Plan Check's Fix on each stair finding.
    let run = crate::dialogs::plan_check::run_check_full(
        &mut sim.app.cx,
        crate::dialogs::plan_check::CheckKind::Plan,
    );
    let f = run
        .findings
        .iter()
        .find(|f| f.rule == "IRC R311.7.1 stair width")
        .expect("the width finding")
        .clone();
    assert!(code::can_fix(&f));
    let label_before = sim.app.cx.undo_label().map(str::to_string);
    let msg = code::fix_finding(&mut sim.app.cx, &f).expect("fixed");
    assert!(msg.contains("code minimums"), "{msg}");
    assert_eq!(sim.app.cx.undo_label(), Some("Fix Stair to Code"));
    let fixed = stairs_view::find(sim.app.cx.floor(), o.id()).unwrap();
    assert_eq!(fixed.stair.params.width, 36.0);
    assert_eq!(fixed.stair.params.tread_depth, 10.0);
    assert_eq!(fixed.stair.params.headroom_min, 80.0);
    sim.app.cx.refresh();
    code::frame(&mut sim.app.cx);
    assert_eq!(code::live_summary().errors, clean.errors);
    // One undo step brings the bad stair back.
    sim.undo();
    assert_eq!(sim.app.cx.undo_label().map(str::to_string), label_before);
    assert_eq!(
        stairs_view::find(sim.app.cx.floor(), o.id())
            .unwrap()
            .stair
            .params
            .width,
        30.0
    );
    // Fixing a stair that is already legal does nothing.
    sim.redo();
    assert!(code::fix_finding(&mut sim.app.cx, &f).is_none());
}

#[test]
fn the_live_check_is_quiet_when_switched_off() {
    let mut sim = house();
    draw_stair(&mut sim);
    crate::dialogs::preferences::pages::update(|p| p.architectural.check_while_drawing = false);
    code::frame(&mut sim.app.cx);
    assert_eq!(code::live_summary().runs, 0);
    assert_eq!(code::badge_count(), 0);
    assert!(code::status_text(&sim.app.cx).is_none());
    crate::dialogs::preferences::pages::update(|p| p.architectural.check_while_drawing = true);
}

#[test]
fn the_status_bar_names_the_code_while_the_settings_are_open() {
    let sim = house();
    assert!(code::status_text(&sim.app.cx).is_none_or(|t| !t.contains("Code:")));
    code::note_settings_open();
    let t = code::status_text(&sim.app.cx).unwrap();
    assert!(t.contains("Code: IRC 2021"), "{t}");
    code::note_settings_closed();
}

#[test]
fn a_footing_finding_is_fixed_on_the_foundation_floor() {
    let mut sim = house();
    // Build a foundation under the house through the dialog's own function.
    let mut spec = crate::editor::rooms_edit::FoundationSpec::from_defaults(&sim.app.cx.defaults);
    spec.footing = true;
    spec.footing_width = 8.0;
    spec.footing_depth = 3.0;
    crate::editor::rooms_edit::build_foundation(&mut sim.app.cx, spec);
    let idx = sim
        .app
        .cx
        .project
        .floors
        .iter()
        .position(|f| f.kind == plan_core::FloorKind::Foundation)
        .expect("a foundation floor");
    let b = sim.app.cx.project.floors[idx].settings.foundation.unwrap();
    assert!(b.footing_width < 12.0);
    let f = plan_check::Finding {
        rule: "IRC R403.1.1 footing size",
        severity: plan_check::Severity::Warning,
        message: String::new(),
        location: None,
        object: None,
        fix: String::new(),
    };
    assert!(code::can_fix(&f));
    let msg = code::fix_finding(&mut sim.app.cx, &f).expect("fixed");
    assert!(msg.starts_with("Footing set to"), "{msg}");
    assert_eq!(sim.app.cx.undo_label(), Some("Fix Footing to Code"));
    let b = sim.app.cx.project.floors[idx].settings.foundation.unwrap();
    let m = code::code_minimums(&sim.app.cx);
    assert!(b.footing_width >= m.footing_width(1));
    assert!(b.footing_depth >= m.footing_min_thickness);
}

#[test]
fn auto_place_outlets_takes_its_spacing_from_the_minimums() {
    let mut s = CheckSettings::irc_2021();
    s.options.min_hall_width = 36.0;
    let m = CodeMinimums::from(&s);
    let mut o = plan_electrical::AutoOutletOptions {
        max_spacing: 200.0,
        kitchen_counter_spacing: 60.0,
        min_wall_segment: 12.0,
        ..Default::default()
    };
    code::outlet_options(&m, &mut o);
    assert_eq!(o.max_spacing, 144.0);
    assert_eq!(o.kitchen_counter_spacing, 48.0);
    assert_eq!(o.min_wall_segment, 24.0);
}

#[test]
fn live_check_stays_fast_on_a_full_house() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/large-house.psplan");
    let text = std::fs::read_to_string(path).expect("samples/large-house.psplan");
    let project = plan_core::Project::from_json(&text).expect("large sample loads");
    let mut sim = Sim::new();
    sim.app.cx.set_project(project);
    sim.app.cx.refresh();
    crate::dialogs::preferences::pages::update(|p| p.architectural.check_while_drawing = true);
    code::frame(&mut sim.app.cx);
    let first = code::live_summary();
    // Median time of an edit that touches only the openings.
    let mut times = Vec::new();
    for i in 0..15 {
        let w = {
            let f = sim.app.cx.floor_mut();
            let o = f
                .openings
                .iter_mut()
                .find(|o| o.kind == OpeningKind::Window)
                .unwrap();
            o.width += if i % 2 == 0 { 1.0 } else { -1.0 };
            o.width
        };
        let _ = w;
        sim.app.cx.mark_dirty();
        sim.app.cx.refresh();
        let t = std::time::Instant::now();
        code::frame(&mut sim.app.cx);
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|a, b| a.total_cmp(b));
    let median = times[times.len() / 2];
    println!(
        "live check on the large sample: median {median:.3} ms per edit ({} runs, first run {} us, {} findings)",
        code::live_summary().runs,
        first.micros,
        first.total()
    );
    // Where the time goes: the whole check alone, for comparison.
    let mut check = Vec::new();
    for _ in 0..15 {
        let t = std::time::Instant::now();
        let _ = crate::dialogs::plan_check::run_check_full(
            &mut sim.app.cx,
            crate::dialogs::plan_check::CheckKind::Plan,
        );
        check.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    check.sort_by(|a, b| a.total_cmp(b));
    println!(
        "the whole Plan Check alone: median {:.3} ms",
        check[check.len() / 2]
    );
    assert!(code::live_summary().runs > first.runs);
    // The budget is 1 ms; the gate only guards against a blow-up on a busy machine.
    assert!(median < 25.0, "live check took {median:.2} ms");
}
