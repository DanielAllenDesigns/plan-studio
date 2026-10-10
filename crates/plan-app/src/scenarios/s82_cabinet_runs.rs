//! Scenario 82 (round 16): cabinet runs. A U-shaped kitchen drawn with gaps
//! gets automatic fillers between cabinets and walls within 3 in, the
//! cabinets read their contractor codes (B24, W2430, BF3), merged cabinets
//! show module lines on their own layer, the cabinet schedule has its seven
//! categories, and the General Cabinet Defaults, Set as Default and dynamic
//! defaults work (CB-1..6, CB-10, CB-13, CB-19, CB-21, CB-475, CB-476,
//! CB-480..483, CB-488, CB-634, CB-635).

#![allow(clippy::field_reassign_with_default)]
use super::Sim;
use crate::dialogs::cabinet_defaults;
use crate::editor::placed::{self, add_cabinet, cabinet_by_id, load_cabinets};
use crate::editor::ObjectRef;
use crate::tools::cabinet::{default_cabinet, sync_auto_fillers};
use crate::tools::ToolId;
use plan_cabinets::{Cabinet, CabinetKind, ScheduleCategory, MODULE_LINES_LAYER};
use plan_core::defaults::GeneralCabinetDefaults;
use plan_core::geometry::Point;
use plan_core::WallKind;

/// A room 240 in wide with three walls: top (face y = 3), west (face x = 3)
/// and east (face x = 237).
fn room() -> Sim {
    let mut sim = Sim::new();
    placed::set_auto_join(false);
    let fl = &mut sim.app.cx.project;
    for (a, b) in [
        (Point::new(0.0, 0.0), Point::new(240.0, 0.0)),
        (Point::new(0.0, 0.0), Point::new(0.0, 240.0)),
        (Point::new(240.0, 0.0), Point::new(240.0, 240.0)),
    ] {
        fl.add_wall(0, a, b, 6.0, 96.0, WallKind::Interior);
    }
    sim.app.cx.refresh();
    sim
}

fn put(sim: &mut Sim, mut c: Cabinet, x: f64, y: f64, angle: f64) -> u64 {
    c.position = Point::new(x, y);
    c.angle = angle;
    add_cabinet(&mut sim.app.cx.project, 0, c).unwrap()
}

fn base(sim: &Sim) -> Cabinet {
    default_cabinet(&sim.app.cx, CabinetKind::Base)
}

fn wall_cab(sim: &Sim) -> Cabinet {
    default_cabinet(&sim.app.cx, CabinetKind::Wall)
}

fn all(sim: &Sim) -> Vec<Cabinet> {
    load_cabinets(sim.app.cx.floor())
}

fn autos(sim: &Sim) -> Vec<Cabinet> {
    all(sim).into_iter().filter(|c| c.auto_filler).collect()
}

fn users(sim: &Sim) -> Vec<Cabinet> {
    all(sim).into_iter().filter(|c| !c.auto_filler).collect()
}

/// The U kitchen drawn with gaps (see the module docs): a north run of five
/// base cabinets, two wall cabinets over it, a west leg of two, and a loose
/// shelf, partition, soffit and manual filler. Returns nothing placed by the
/// tool yet.
fn kitchen(sim: &mut Sim) {
    // North run, backs on the top wall: gaps 2 (west wall), 2, 3, 6, then
    // 2 from the east wall.
    for x in [5.0, 31.0, 58.0, 88.0, 211.0] {
        let c = base(sim);
        put(sim, c, x, 3.0, 0.0);
    }
    // Wall cabinets over the first two: 2 from the wall, 2 between them.
    for x in [5.0, 31.0] {
        let c = wall_cab(sim);
        put(sim, c, x, 3.0, 0.0);
    }
    // West leg facing east: 2 between them.
    for y in [100.0, 74.0] {
        let c = base(sim);
        put(sim, c, 3.0, y, -std::f64::consts::FRAC_PI_2);
    }
    // Loose pieces that never run.
    put(
        sim,
        Cabinet::new(CabinetKind::Shelf, 24.0),
        100.0,
        150.0,
        0.0,
    );
    put(
        sim,
        Cabinet::new(CabinetKind::Partition, 24.0),
        140.0,
        150.0,
        0.0,
    );
    put(
        sim,
        Cabinet::new(CabinetKind::Soffit, 24.0),
        180.0,
        150.0,
        0.0,
    );
    put(
        sim,
        Cabinet::filler(CabinetKind::BaseFiller, 3.0),
        60.0,
        200.0,
        0.0,
    );
}

/// Places one more base cabinet with the tool, which brings the fillers up
/// to date as part of its one undo step.
fn place_with_tool(sim: &mut Sim, x: f64, y: f64) {
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    sim.app.cx.selection.clear();
    sim.click(x, y);
}

fn kitchen_with_fillers() -> Sim {
    let mut sim = room();
    kitchen(&mut sim);
    place_with_tool(&mut sim, 150.0, 10.0);
    sim
}

fn widths(mut v: Vec<Cabinet>) -> Vec<f64> {
    v.sort_by(|a, b| a.width.total_cmp(&b.width));
    v.iter().map(|c| c.width).collect()
}

#[test]
fn a_u_kitchen_drawn_with_gaps_gets_fillers_and_contractor_labels() {
    let mut sim = room();
    kitchen(&mut sim);
    let before = all(&sim).len();
    assert!(autos(&sim).is_empty(), "nothing generates until an edit");
    place_with_tool(&mut sim, 150.0, 10.0);
    assert_eq!(all(&sim).len(), before + 1 + 7, "the cabinet and 7 fillers");
    let fillers = autos(&sim);
    // Base fillers: west wall 2, between 2, between 3, east wall 2, leg 2.
    let base_f: Vec<Cabinet> = fillers
        .iter()
        .filter(|f| f.kind == CabinetKind::BaseFiller)
        .cloned()
        .collect();
    assert_eq!(widths(base_f.clone()), vec![2.0, 2.0, 2.0, 2.0, 3.0]);
    // Wall fillers: west wall 2 and between the two wall cabinets 2.
    let wall_f: Vec<Cabinet> = fillers
        .iter()
        .filter(|f| f.kind == CabinetKind::WallFiller)
        .cloned()
        .collect();
    assert_eq!(widths(wall_f), vec![2.0, 2.0]);
    // The 6 in gap and the loose pieces get none; the generated ones are
    // out of the schedules and carry no label.
    assert!(fillers
        .iter()
        .all(|f| !f.in_schedule && f.display_label().is_empty()));
    // The filler copies its neighbour's height, top and toe kick.
    let a = users(&sim)
        .into_iter()
        .find(|c| c.kind == CabinetKind::Base && (c.position.x - 5.0).abs() < 1e-6)
        .unwrap();
    for f in &base_f {
        if f.angle.abs() < 1e-9 {
            assert_eq!(
                (f.height, f.depth, f.elevation),
                (a.height, a.depth, a.elevation)
            );
            assert_eq!(f.countertop, a.countertop);
            assert_eq!(f.toe_kick, a.toe_kick);
        }
    }
    // Contractor codes.
    let label = |c: &Cabinet| c.display_label();
    assert_eq!(label(&a), "B24");
    let wc = users(&sim)
        .into_iter()
        .find(|c| c.kind == CabinetKind::Wall)
        .unwrap();
    assert_eq!(label(&wc), "W2430");
    for c in users(&sim) {
        match c.kind {
            CabinetKind::Shelf | CabinetKind::Partition => assert_eq!(label(&c), ""),
            CabinetKind::Soffit => assert_eq!(label(&c), "SO24"),
            CabinetKind::BaseFiller => assert_eq!(label(&c), "BF3"),
            _ => {}
        }
    }
    // One undo step takes the placed cabinet and every filler away again.
    sim.undo();
    assert_eq!(all(&sim).len(), before);
    assert!(autos(&sim).is_empty());
    sim.redo();
    assert_eq!(autos(&sim).len(), 7);
    // The filler ids stay put while nothing changes: syncing again writes
    // nothing.
    let ids: Vec<u64> = autos(&sim).iter().map(|c| c.id).collect();
    sim.app.cx.begin_change("Sync");
    assert_eq!(sync_auto_fillers(&mut sim.app.cx), 0);
    sim.app.cx.cancel_change();
    assert_eq!(autos(&sim).iter().map(|c| c.id).collect::<Vec<_>>(), ids);
}

#[test]
fn a_filler_follows_its_cabinet_when_the_gap_changes() {
    let mut sim = kitchen_with_fillers();
    // Close the 3 in gap between the second and third cabinets to 2 in by
    // moving the third: the filler follows (it now sits 55 to 57), and the
    // 6 in gap stays open.
    let c = users(&sim)
        .into_iter()
        .find(|c| c.kind == CabinetKind::Base && (c.position.x - 58.0).abs() < 1e-6)
        .unwrap();
    let mut moved = c.clone();
    moved.position.x = 57.0;
    sim.app.cx.begin_change("Move");
    assert!(placed::replace_cabinet(&mut sim.app.cx.project, 0, &moved));
    placed::rejoin_if_enabled(&mut sim.app.cx);
    let w: Vec<f64> = autos(&sim)
        .iter()
        .filter(|f| f.kind == CabinetKind::BaseFiller)
        .map(|f| f.width)
        .collect();
    assert!(!w.contains(&3.0), "{w:?}");
    assert!(
        autos(&sim).iter().any(|f| f.kind == CabinetKind::BaseFiller
            && f.width == 2.0
            && (f.position.x - 55.0).abs() < 1e-6),
        "a 2 in filler follows to 55"
    );
    assert_eq!(autos(&sim).len(), 7);
    // Move it out of reach on both sides: its filler goes.
    moved.position.x = 60.0;
    placed::replace_cabinet(&mut sim.app.cx.project, 0, &moved);
    placed::rejoin_if_enabled(&mut sim.app.cx);
    assert_eq!(autos(&sim).len(), 6);
}

#[test]
fn module_lines_follow_their_layer_and_the_partial_setting() {
    let mut sim = kitchen_with_fillers();
    let cabs = all(&sim);
    let display = plan_cabinets::run_display(&cabs, 3.0, false);
    // Joins: west filler|A, A|f, f|B, B|f, f|C, E|east filler (north run),
    // wall filler|W1, W1|f, f|W2, and the leg's L|f|L.
    assert_eq!(display.lines.len(), 11, "{:?}", display.lines);
    // The layer exists from the first sync and hides the lines.
    assert!(sim.app.cx.project.layers.get(MODULE_LINES_LAYER).is_some());
    let on = sim.plan_shapes().len();
    sim.app
        .cx
        .project
        .layers
        .set_display(MODULE_LINES_LAYER, false);
    let off = sim.plan_shapes().len();
    assert!(on > off, "module lines are drawn: {on} against {off}");
    // Partial lines are short grey ticks: far fewer shapes than dashes.
    sim.app
        .cx
        .project
        .layers
        .set_display(MODULE_LINES_LAYER, true);
    sim.app
        .cx
        .defaults
        .cabinets
        .general
        .show_partial_module_lines = true;
    let partial = sim.plan_shapes().len();
    assert!(partial < on, "{partial} against {on}");
    assert!(partial > off);
}

#[test]
fn fillers_join_the_continuous_countertop_of_their_run() {
    let mut sim = room();
    placed::set_auto_join(true);
    kitchen(&mut sim);
    place_with_tool(&mut sim, 150.0, 10.0);
    // The north run from the west wall through the third cabinet is one top
    // that includes the three fillers between and beside them.
    let tops: Vec<Cabinet> = all(&sim)
        .into_iter()
        .filter(|c| !c.joined.is_empty())
        .collect();
    let fillers: Vec<u64> = autos(&sim)
        .iter()
        .filter(|f| f.kind == CabinetKind::BaseFiller && f.angle.abs() < 1e-9)
        .map(|f| f.id)
        .collect();
    let biggest = tops.iter().max_by_key(|t| t.joined.len()).unwrap();
    assert!(biggest.joined.len() >= 6, "{}", biggest.joined.len());
    let covered = fillers
        .iter()
        .filter(|id| biggest.joined.iter().any(|j| j.id == **id))
        .count();
    assert!(covered >= 3, "{covered} fillers under the top");
    placed::set_auto_join(false);
}

#[test]
fn turning_automatic_fillers_off_removes_them_and_back_on_restores_them() {
    let mut sim = kitchen_with_fillers();
    assert_eq!(autos(&sim).len(), 7);
    let mut g = sim.app.cx.defaults.cabinets.general.clone();
    g.create_automatic_fillers = false;
    assert!(cabinet_defaults::apply(&mut sim.app.cx, &g));
    assert!(autos(&sim).is_empty());
    // Without fillers only touching cabinets merge.
    let display = plan_cabinets::run_display(&all(&sim), plan_cabinets::merge_reach(false), false);
    assert!(display.lines.is_empty());
    // The change is one undo step.
    sim.undo();
    assert_eq!(autos(&sim).len(), 7);
    // The angled variant alone.
    g.create_automatic_fillers = true;
    g.create_automatic_fillers_angled = false;
    cabinet_defaults::apply(&mut sim.app.cx, &g);
    assert_eq!(autos(&sim).len(), 7, "straight fillers stay");
}

#[test]
fn the_schedule_lists_seven_categories_and_never_an_automatic_filler() {
    let sim = kitchen_with_fillers();
    let counts = plan_cabinets::schedule_counts(&all(&sim));
    assert_eq!(counts.len(), 7);
    let get = |cat| counts.iter().find(|c| c.0 == cat).unwrap().1;
    assert_eq!(get(ScheduleCategory::Base), 5 + 2 + 1);
    assert_eq!(get(ScheduleCategory::Wall), 2);
    assert_eq!(get(ScheduleCategory::FullHeight), 0);
    assert_eq!(get(ScheduleCategory::Fillers), 1, "the manual filler only");
    assert_eq!(get(ScheduleCategory::Soffits), 1);
    assert_eq!(get(ScheduleCategory::Shelves), 1);
    assert_eq!(get(ScheduleCategory::Partitions), 1);
    // The schedule table has the same rows with or without the generated
    // fillers.
    let rows = |sim: &Sim| {
        plan_docs::schedule_kinds::table(
            &sim.app.cx.project,
            &plan_core::schedules::Schedule::new(
                plan_core::schedules::ScheduleKind::Cabinet,
                Point::ZERO,
            ),
            0,
            None,
        )
        .rows
        .len()
    };
    let with = rows(&sim);
    let mut sim = sim;
    let mut g = sim.app.cx.defaults.cabinets.general.clone();
    g.create_automatic_fillers = false;
    cabinet_defaults::apply(&mut sim.app.cx, &g);
    assert_eq!(rows(&sim), with);
}

#[test]
fn suppress_label_hides_one_label_and_the_labels_layer_hides_all() {
    let mut sim = kitchen_with_fillers();
    let labels = |sim: &mut Sim| {
        sim.plan_shapes()
            .iter()
            .filter(|s| matches!(s, eframe::egui::Shape::Text(_)))
            .count()
    };
    let n = labels(&mut sim);
    assert!(n >= 8, "{n} labels");
    let mut a = users(&sim)
        .into_iter()
        .find(|c| c.kind == CabinetKind::Base)
        .unwrap();
    a.suppress_label = true;
    assert!(placed::apply_cabinet(&mut sim.app.cx, &a));
    assert_eq!(labels(&mut sim), n - 1);
    sim.app
        .cx
        .project
        .layers
        .set_display(plan_core::layers::CABINET_LABEL_LAYER, false);
    assert!(
        labels(&mut sim) + 8 <= n,
        "the labels layer hides the labels"
    );
}

#[test]
fn plan_display_options_show_closed_fronts_and_pilasters() {
    let mut sim = room();
    let mut c = base(&sim);
    c.accessories.pilaster = plan_cabinets::PilasterStyle::Plain;
    put(&mut sim, c, 60.0, 3.0, 0.0);
    let plain = sim.plan_shapes().len();
    sim.app
        .cx
        .defaults
        .cabinets
        .general
        .show_closed_doors_drawers = true;
    let fronts = sim.plan_shapes().len();
    assert!(fronts > plain, "{fronts} against {plain}");
    sim.app.cx.defaults.cabinets.general.show_pilasters = true;
    let pilasters = sim.plan_shapes().len();
    assert!(pilasters > fronts, "{pilasters} against {fronts}");
}

#[test]
fn set_as_default_copies_a_cabinet_and_dynamic_defaults_move_the_others() {
    let mut sim = room();
    let a = {
        let c = base(&sim);
        put(&mut sim, c, 20.0, 3.0, 0.0)
    };
    let b = {
        let mut c = base(&sim);
        // This one was set to its own toe kick height.
        c.toe_kick.as_mut().unwrap().height = 6.0;
        put(&mut sim, c, 80.0, 3.0, 0.0)
    };
    // Change the default toe kick: the first follows, the second keeps 6.
    let old = sim.app.cx.defaults.cabinets.clone();
    sim.app.cx.defaults.cabinets.base.toe_kick_height = 5.0;
    sim.app.cx.defaults.cabinets.base.countertop_thickness = 2.0;
    assert_eq!(
        crate::tools::cabinet::apply_dynamic_defaults(&mut sim.app.cx, &old),
        2
    );
    let fa = cabinet_by_id(sim.app.cx.floor(), a).unwrap();
    let fb = cabinet_by_id(sim.app.cx.floor(), b).unwrap();
    assert_eq!(fa.toe_kick.unwrap().height, 5.0);
    assert_eq!(fb.toe_kick.unwrap().height, 6.0);
    // The thicker top raises both by half an inch.
    assert_eq!((fa.height, fa.countertop.unwrap().thickness), (36.5, 2.0));
    // One undo step.
    sim.undo();
    assert_eq!(
        cabinet_by_id(sim.app.cx.floor(), a)
            .unwrap()
            .toe_kick
            .unwrap()
            .height,
        4.0
    );
    // Set as Default from the second cabinet.
    sim.app.cx.selection.set(ObjectRef::Cabinet(b));
    sim.app.cx.defaults.cabinets.base.toe_kick_height = 4.0;
    assert!(crate::tools::cabinet::set_as_default(&mut sim.app.cx));
    assert_eq!(sim.app.cx.defaults.cabinets.base.toe_kick_height, 6.0);
    assert_eq!(sim.app.cx.status, "Base Cabinet Defaults have been updated");
    // A special shape cannot be a default.
    let corner = {
        let c = default_cabinet(&sim.app.cx, CabinetKind::CornerBase);
        put(&mut sim, c, 150.0, 3.0, 0.0)
    };
    sim.app.cx.selection.set(ObjectRef::Cabinet(corner));
    assert!(!crate::tools::cabinet::set_as_default(&mut sim.app.cx));
}

#[test]
fn a_wall_cabinet_over_a_free_standing_refrigerator_hangs_from_its_top() {
    let mut sim = room();
    let mut fridge = plan_core::PlacedSymbol::new(
        "appliances/refrigerator-side-by-side",
        Point::new(60.0, 3.0),
        36.0,
        30.0,
        70.0,
    );
    fridge.layer = "Fixtures, Interior".into();
    sim.app.cx.project.add_symbol(0, fridge);
    sim.tool(ToolId::CabinetVariant(CabinetKind::Wall));
    sim.app.cx.selection.clear();
    sim.click(60.0, 10.0);
    let w = users(&sim)
        .into_iter()
        .find(|c| c.kind == CabinetKind::Wall)
        .unwrap();
    assert_eq!(w.elevation, 70.0);
    // Elsewhere along the wall it keeps its usual bottom.
    sim.app.cx.selection.clear();
    sim.click(160.0, 10.0);
    let far = users(&sim)
        .into_iter()
        .filter(|c| c.kind == CabinetKind::Wall)
        .max_by(|a, b| a.position.x.total_cmp(&b.position.x))
        .unwrap();
    assert_eq!(far.elevation, 54.0);
}

#[test]
fn a_narrow_space_takes_a_smaller_cabinet_and_a_too_narrow_one_takes_none() {
    let mut sim = room();
    // 20 in between two cabinets on the north wall.
    for x in [10.0, 54.0] {
        let c = base(&sim);
        put(&mut sim, c, x, 3.0, 0.0);
    }
    // Spaces: [34, 54] is 20 in: an 18 in cabinet (3 in increment).
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    sim.app.cx.selection.clear();
    sim.click(44.0, 10.0);
    let placed_one = users(&sim)
        .into_iter()
        .find(|c| (c.position.x - 34.0).abs() < 1e-6)
        .expect("the cabinet sits in the space");
    assert_eq!(placed_one.width, 18.0, "{:?}", placed_one.width);
    // A 6 in space with a 9 in minimum: nothing is placed.
    sim.app.cx.defaults.cabinets.general.min_cabinet_width = 9.0;
    let count = users(&sim).len();
    for x in [100.0, 130.0] {
        let c = base(&sim);
        put(&mut sim, c, x, 3.0, 0.0);
    }
    // 100..124 and 130..154: a 6 in space.
    sim.app.cx.selection.clear();
    sim.click(127.0, 10.0);
    assert_eq!(users(&sim).len(), count + 2, "no cabinet was placed");
    assert!(sim.app.cx.status.contains("Minimum Cabinet Width"));
}

#[test]
fn resizing_steps_follow_the_general_cabinet_defaults() {
    let mut sim = room();
    let id = {
        let c = base(&sim);
        put(&mut sim, c, 60.0, 3.0, 0.0)
    };
    let step = |sim: &Sim| crate::tools::cabinet::width_step(&sim.app.cx);
    assert_eq!(step(&sim), 3.0);
    sim.app.cx.defaults.cabinets.general.resize_increment = 1.5;
    assert_eq!(step(&sim), 1.5);
    sim.app.cx.defaults.cabinets.general.resize_by_grid = true;
    assert_eq!(step(&sim), sim.app.cx.snap_unit());
    // A value under 1/16 is lifted to 1/16.
    sim.app.cx.defaults.cabinets.general.resize_by_grid = false;
    sim.app.cx.defaults.cabinets.general.resize_increment = 0.0;
    assert_eq!(step(&sim), 1.0 / 16.0);
    assert!(cabinet_by_id(sim.app.cx.floor(), id).is_some());
}

#[test]
fn the_general_cabinet_defaults_dialog_opens_and_keeps_its_values() {
    let mut sim = room();
    assert!(!cabinet_defaults::is_open());
    cabinet_defaults::request_open();
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(cabinet_defaults::is_open());
    sim.cancel();
    sim.dialog_frame(false);
    assert!(!cabinet_defaults::is_open());
    // The record round-trips with the plan defaults.
    let mut g = GeneralCabinetDefaults::default();
    g.min_cabinet_width = 4.0;
    g.show_pilasters = true;
    sim.app.cx.defaults.cabinets.general = g.clone();
    let json = serde_json::to_string(&sim.app.cx.defaults).unwrap();
    let back: plan_core::defaults::PlanDefaults = serde_json::from_str(&json).unwrap();
    assert_eq!(back.cabinets.general, g);
}
