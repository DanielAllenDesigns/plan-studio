//! Scenario 55 (round 15): stairs and railings. The Ramp and Curved Ramp
//! tools (1:12, landings every 30" of rise, handrails on both sides, plan and
//! 3D), a downward stair that lands on the terrain, Stairs to Deck, newels and
//! balusters taken from the library, the Staircase Specification's new
//! panels, the plan display options and old files that load unchanged
//! (CB-34, CB-105, CB-109, CB-110, CB-122, CB-158, CB-171, CB-180, CB-181,
//! CB-187, CB-200 in `docs/parity/cabinets-stairs-framing-terrain-library.md`).

use super::{draw_shell, Sim};
use crate::editor::site_view::{self, load_terrain};
use crate::editor::stairs_view::{self, PostKind, StairKind, StairObj};
use crate::editor::ObjectRef;
use crate::tools::library::user::{self as store, tests_support};
use crate::tools::library::{clear_active_item, library_catalog};
use crate::tools::terrain::TerrainVariant as TV;
use crate::tools::ToolId;
use plan_core::deck::{DeckSpec, DECK_ROOM_TYPE};
use plan_core::geometry::Point;
use plan_core::{RoomName, Wall, WallClass, WallKind};
use plan_library::{CatalogItem, ItemKind, Model3d, ModelPart, Placement, Symbol2d};
use plan_stairs::{
    stair_posts, tagged_meshes, tagged_meshes_skipping, ArrowStyle, PostSkip, RailStyle, SideKind,
    StairPart, StairShape,
};
use plan_terrain::ElevationPoint;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn stairs(sim: &Sim) -> Vec<StairObj> {
    stairs_view::load(sim.app.cx.floor())
}

fn only(sim: &Sim) -> StairObj {
    let all = stairs(sim);
    assert_eq!(all.len(), 1, "one stair expected");
    all.into_iter().next().unwrap()
}

fn count(parts: &[(StairPart, plan_3d::Mesh)], part: StairPart) -> usize {
    parts.iter().filter(|(p, _)| *p == part).count()
}

/// Lowest and highest corner of the meshes of `part`.
fn extent(parts: &[(StairPart, plan_3d::Mesh)], part: StairPart) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for (p, m) in parts {
        if *p != part {
            continue;
        }
        if let Some((a, b)) = m.bounds() {
            for k in 0..3 {
                lo[k] = lo[k].min(a[k]);
                hi[k] = hi[k].max(b[k]);
            }
        }
    }
    (lo, hi)
}

// ----- ramps -----

#[test]
fn the_ramp_tool_draws_a_1_to_12_ramp_with_handrails_on_both_sides() {
    let mut sim = house();
    let before = sim.plan_shapes().len();
    sim.tool(ToolId::StairsVariant(StairKind::Ramp));
    sim.drag((40.0, 200.0), (400.0, 200.0));
    let o = only(&sim);
    assert!(o.is_ramp());
    let StairShape::Ramp { slope_1_in } = o.stair.params.shape else {
        panic!("a ramp")
    };
    // 360" of run over the 30" maximum rise is exactly 1:12.
    assert!((slope_1_in - 12.0).abs() < 1e-6, "{slope_1_in}");
    assert!((o.stair.params.total_rise - 30.0).abs() < 1e-6);
    assert!(o.solution().code_ok);
    assert_eq!(o.solution().landings, 0);
    assert!(
        sim.app.cx.status.starts_with("Ramp: "),
        "{}",
        sim.app.cx.status
    );
    assert!(sim.app.cx.status.contains("slope 1:12"));
    // 3D: one sloped slab and a handrail on each side of the ramp.
    let parts = tagged_meshes(&o.stair);
    assert_eq!(count(&parts, StairPart::Ramp), 1);
    assert!(count(&parts, StairPart::Handrail) >= 2);
    let (lo, hi) = extent(&parts, StairPart::Handrail);
    // The rails reach a hand's height above the top of the ramp.
    assert!(hi[1] > 30.0 + 30.0, "{hi:?}");
    let ramp = extent(&parts, StairPart::Ramp);
    assert!(lo[2] <= ramp.0[2] + 6.0 && hi[2] >= ramp.1[2] - 6.0);
    // Plan: the ramp draws (outline, arrow, slope label) and the whole thing
    // is one undo step.
    assert!(sim.plan_shapes().len() > before + 5);
    assert!(!stairs_view::symbol_strokes(&o).is_empty());
    assert_eq!(sim.undo().as_deref(), Some("Draw Ramp"));
    assert!(stairs(&sim).is_empty());
}

#[test]
fn a_ramp_steeper_than_1_to_12_or_taller_than_30_inches_is_flagged_and_split() {
    let mut sim = house();
    sim.tool(ToolId::StairsVariant(StairKind::Ramp));
    // 200" over 30" is 1:6.7: too steep for a public ramp.
    sim.drag((40.0, 200.0), (240.0, 200.0));
    let steep = only(&sim);
    assert!(!steep.solution().code_ok);
    assert!(steep.solution().warnings.iter().any(|w| w.contains("1:12")));
    assert!(sim.app.cx.status.contains("1:12"), "{}", sim.app.cx.status);
    // Plan Check reports it, with the rule the checker names.
    let run = crate::dialogs::plan_check::run_check_full(
        &mut sim.app.cx,
        crate::dialogs::plan_check::CheckKind::Plan,
    );
    assert!(
        run.findings.iter().any(|f| f.rule == "IRC R311.8 ramps"),
        "the steep ramp is a finding"
    );
    // A 60" rise is two runs and a 60" landing between them, and the section
    // view draws both runs.
    let mut o = steep;
    stairs_view::set_total_rise(&mut o, 60.0);
    stairs_view::set_run(&mut o, 360.0);
    let sol = o.solution();
    assert_eq!(sol.landings, 1);
    assert!((sol.total_run - (60.0 * 12.0 + 60.0)).abs() < 1.0 || sol.total_run > 700.0);
    let parts = tagged_meshes(&o.stair);
    assert!(count(&parts, StairPart::Ramp) >= 2, "two runs");
    assert!(count(&parts, StairPart::Landing) >= 1, "a landing between");
    assert!(stairs_view::elevation_points(&o).len() >= 4);
}

#[test]
fn a_curved_ramp_turns_climbs_and_has_rails_that_follow_it() {
    let mut sim = house();
    sim.tool(ToolId::StairsVariant(StairKind::CurvedRamp));
    sim.drag((240.0, 180.0), (240.0, 120.0));
    let o = only(&sim);
    assert!(o.is_ramp());
    let radius = o.stair.params.ramp_curve.expect("a curved ramp");
    assert!(radius > 0.0);
    assert!(
        sim.app.cx.status.starts_with("Curved ramp"),
        "{}",
        sim.app.cx.status
    );
    // It arrives at its rise at the end of the arc.
    let (_, z) = plan_stairs::top_point(&o.stair);
    assert!((z - (o.stair.floor_elevation + o.stair.params.total_rise)).abs() < 0.5);
    assert!(plan_stairs::curve_center(&o.stair).is_some());
    let sweep = plan_stairs::curve_sweep(&o.stair).unwrap();
    assert!(sweep.abs() > 0.1);
    let parts = tagged_meshes(&o.stair);
    assert!(count(&parts, StairPart::Handrail) >= 2);
    // Its handrails rise with the ramp: the top of the rails is higher than
    // the rise by a hand height.
    let (_, hi) = extent(&parts, StairPart::Handrail);
    assert!(f64::from(hi[1]) > o.stair.params.total_rise + 30.0);
    // The Ramp Specification opens on it and cancels without a change.
    let id = o.id();
    assert!(sim.open_spec(ObjectRef::Stair(id)));
    sim.cancel();
    assert_eq!(only(&sim).stair, o.stair);
}

// ----- the Staircase Specification -----

#[test]
fn the_specification_tabs_edit_stringers_newels_the_arrow_and_the_plan_display() {
    let mut sim = house();
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((60.0, 100.0), (60.0, 300.0));
    let o = only(&sim);
    let id = o.id();
    // The dialog opens and closes with OK without changing the stair.
    assert!(sim.open_spec(ObjectRef::Stair(id)));
    sim.ok();
    assert_eq!(only(&sim).stair, o.stair);
    // Edit the draft the way the tabs do.
    let plain = tagged_meshes(&o.stair).len();
    let mut d = o.clone();
    d.stair.params.stringers.centre = 1;
    d.stair.params.stringers.open_underneath = false;
    d.stair.params.runner.width = 24.0;
    d.stair.params.left_side = SideKind::Railing;
    d.stair.params.right_side = SideKind::Railing;
    d.stair.params.railing.style = RailStyle::Balusters {
        size: 1.25,
        spacing: 3.5,
    };
    d.stair.params.railing.newel.profile = plan_stairs::PostProfile::Turned;
    d.stair.params.plan.arrow = ArrowStyle::Open;
    d.stair.params.plan.number_treads = true;
    d.stair.params.plan.draw_balusters = true;
    d.stair.params.plan.floor_above = plan_stairs::DisplayRule::Always;
    assert!(stairs_view::apply_edit(&mut sim.app.cx, &d));
    let after = only(&sim);
    assert!(tagged_meshes(&after.stair).len() > plain + 20);
    // The plan numbers the treads and draws the balusters: more strokes.
    let strokes = stairs_view::symbol_strokes(&after).len();
    assert!(strokes > stairs_view::symbol_strokes(&o).len() + 10);
    // The Components tab counts the new parts.
    let names: Vec<String> = stairs_view::components(&after)
        .iter()
        .map(|c| c.name.clone())
        .collect();
    assert!(names.iter().any(|n| n.contains("Newel")), "{names:?}");
    // One undo step puts everything back.
    assert_eq!(sim.undo().as_deref(), Some("Stair Specification"));
    assert_eq!(only(&sim).stair, o.stair);
}

#[test]
fn a_stair_saved_before_round_15_loads_with_the_old_look() {
    let mut sim = house();
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((60.0, 100.0), (60.0, 300.0));
    let o = only(&sim);
    // Strip every key a round-15 file has and load it back.
    let mut v = sim.app.cx.floor().stairs[0].clone();
    if let Some(p) = v.get_mut("params").and_then(|p| p.as_object_mut()) {
        for k in [
            "stringers",
            "runner",
            "top_landing",
            "handrail_options",
            "plan",
            "flare_shape",
            "starter",
            "ramp_curve",
            "newel_item",
            "baluster_item",
            "edge_rails",
            "down",
            "walkline",
            "radius_ref",
            "u_gap",
        ] {
            p.remove(k);
        }
    }
    sim.app.cx.floor_mut().stairs[0] = v;
    let again = only(&sim);
    assert_eq!(again.stair.params, o.stair.params);
    assert_eq!(
        tagged_meshes(&again.stair).len(),
        tagged_meshes(&o.stair).len()
    );
}

// ----- newels and balusters from the library -----

fn cube_model() -> Model3d {
    let v = |x: f32, y: f32, z: f32| [x, y, z];
    Model3d {
        parts: vec![ModelPart {
            name: "post".into(),
            color: Some([120, 80, 40]),
            positions: vec![
                v(-1.0, 0.0, 0.0),
                v(1.0, 0.0, 0.0),
                v(1.0, 0.0, 2.0),
                v(-1.0, 0.0, 2.0),
                v(-1.0, 10.0, 0.0),
                v(1.0, 10.0, 0.0),
                v(1.0, 10.0, 2.0),
                v(-1.0, 10.0, 2.0),
            ],
            indices: vec![
                0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2,
                7, 6, 3, 0, 4, 3, 4, 7,
            ],
        }],
    }
}

fn post_item(name: &str, folder: &str) -> CatalogItem {
    CatalogItem::new(
        store::new_id(ItemKind::Model),
        name,
        Placement::FreeStanding,
        Symbol2d::new(Vec::new()),
    )
    .with_category(&["User", folder])
    .with_size(2.0, 2.0, 10.0)
}

#[test]
fn a_library_newel_and_baluster_replace_the_built_in_posts_in_3d() {
    tests_support::fresh(false);
    clear_active_item();
    let newel = store::add(post_item("Turned Newel", "Newels"), Some(&cube_model())).unwrap();
    let baluster =
        store::add(post_item("Iron Baluster", "Balusters"), Some(&cube_model())).unwrap();
    // The two folders list their items for the Newels/Balusters tab.
    assert!(stairs_view::library_posts(PostKind::Newel)
        .iter()
        .any(|(id, _)| *id == newel.id));
    assert!(stairs_view::library_posts(PostKind::Baluster)
        .iter()
        .any(|(id, _)| *id == baluster.id));
    assert!(!stairs_view::library_posts(PostKind::Newel)
        .iter()
        .any(|(id, _)| *id == baluster.id));
    let _ = library_catalog();

    let mut sim = house();
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((60.0, 100.0), (60.0, 300.0));
    let mut o = only(&sim);
    o.stair.params.left_side = SideKind::Railing;
    o.stair.params.right_side = SideKind::Railing;
    o.stair.params.railing.style = RailStyle::Balusters {
        size: 1.25,
        spacing: 3.5,
    };
    assert!(stairs_view::apply_edit(&mut sim.app.cx, &o));
    let o = only(&sim);
    let built_in = stairs_view::part_meshes(&o, 0.0).len();
    // The posts the library would take over.
    let posts = stair_posts(&o.stair);
    assert!(posts.newels.len() >= 4, "{}", posts.newels.len());
    assert!(posts.balusters.len() >= 20);

    // A newel from the library: the built-in newels go and one model stands
    // at each place a newel would.
    let mut with_newel = o.clone();
    with_newel.stair.params.newel_item = newel.id.clone();
    let skipped = tagged_meshes_skipping(
        &with_newel.stair,
        PostSkip {
            newels: true,
            balusters: false,
        },
    )
    .len();
    let got = stairs_view::part_meshes(&with_newel, 0.0).len();
    assert_eq!(got, skipped + posts.newels.len(), "one model per newel");
    assert_ne!(got, built_in);
    // A baluster too.
    let mut with_both = with_newel.clone();
    with_both.stair.params.baluster_item = baluster.id.clone();
    let skipped = tagged_meshes_skipping(
        &with_both.stair,
        PostSkip {
            newels: true,
            balusters: true,
        },
    )
    .len();
    let got = stairs_view::part_meshes(&with_both, 0.0).len();
    assert_eq!(got, skipped + posts.newels.len() + posts.balusters.len());
    // An item that no longer exists falls back to the built-in posts.
    let mut gone = o.clone();
    gone.stair.params.newel_item = "user.model.404".into();
    gone.stair.params.baluster_item = "user.model.405".into();
    assert_eq!(stairs_view::part_meshes(&gone, 0.0).len(), built_in);
    tests_support::fresh(false);
}

// ----- stairs to grade and to a deck -----

/// A 16 x 8 ft deck against the house wall along y = 0.
fn deck_sim() -> Sim {
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
    sim
}

/// Builds flat terrain `drop` inches below the floor around the deck.
fn flat_terrain(sim: &mut Sim, drop: f64) {
    sim.tool(ToolId::TerrainVariant(TV::Perimeter));
    for (x, y) in [
        (-300.0, -300.0),
        (600.0, -300.0),
        (600.0, 500.0),
        (-300.0, 500.0),
    ] {
        sim.click(x, y);
    }
    sim.key(crate::tools::KeyEvent::key(eframe::egui::Key::Enter));
    site_view::edit_terrain(&mut sim.app.cx, "Survey", |r| {
        for (x, y) in [
            (-300.0, -300.0),
            (600.0, -300.0),
            (600.0, 500.0),
            (-300.0, 500.0),
        ] {
            r.terrain.elevation_points.push(ElevationPoint {
                pos: Point::new(x, y),
                z: -drop,
            });
        }
    });
    sim.tool(ToolId::TerrainVariant(TV::Build));
    sim.click(0.0, 0.0);
    assert!(load_terrain(&sim.app.cx.project).is_some());
}

#[test]
fn stairs_to_deck_leave_the_deck_edge_and_land_on_the_terrain() {
    let mut sim = deck_sim();
    flat_terrain(&mut sim, 40.0);
    sim.tool(ToolId::StairsVariant(StairKind::ToDeck));
    // Click near the far edge of the deck.
    sim.click(96.0, 90.0);
    let o = only(&sim);
    assert!(o.stair.params.down, "the arrow says DN");
    assert!(!o.is_ramp() && !o.is_landing());
    // The terrain is 40" below the deck: that is the stair's rise.
    assert!(
        (o.stair.params.total_rise - 40.0).abs() < 3.0,
        "{}",
        o.stair.params.total_rise
    );
    assert!(
        (o.stair.bottom_elevation() + 40.0).abs() < 1.0,
        "the bottom is on the ground: {}",
        o.stair.bottom_elevation()
    );
    // It starts at the deck edge (y = 96) and runs away from the house.
    let (top, z) = plan_stairs::top_point(&o.stair);
    assert!(z.abs() < 4.0, "arrives at the deck: {z}");
    assert!((top.y - 96.0).abs() < 8.0, "{top:?}");
    assert!(o.solution().code_ok, "{:?}", o.solution().warnings);
    // 40" is over the 30" guard rule: both sides get a guard.
    assert_eq!(o.stair.params.left_side, SideKind::Railing);
    assert_eq!(sim.undo().as_deref(), Some("Stairs to Deck"));
    assert!(stairs(&sim).is_empty());
    // Away from any deck or room there is nothing to attach to.
    sim.click(5000.0, 5000.0);
    assert!(stairs(&sim).is_empty());
    assert!(
        sim.app.cx.status.contains("Stairs to Deck"),
        "{}",
        sim.app.cx.status
    );
}

#[test]
fn a_downward_stair_takes_its_height_from_the_terrain() {
    let mut sim = deck_sim();
    flat_terrain(&mut sim, 52.0);
    let p = &sim.app.cx.project;
    let drop = stairs_view::terrain_drop(p, Point::new(96.0, 200.0), 0.0).expect("terrain there");
    assert!((drop - 52.0).abs() < 3.0, "{drop}");
    // A drop of less than a minimum riser is not a drop.
    assert!(stairs_view::terrain_drop(p, Point::new(96.0, 200.0), -50.0).is_none());
    let o = stairs_view::build_down(
        &sim.app.cx.project,
        0,
        StairKind::Draw,
        plan_stairs::Turn::Left,
        Point::new(96.0, 100.0),
        Some(Point::new(96.0, 300.0)),
    );
    assert!(o.stair.params.down);
    assert!((o.stair.params.total_rise - drop).abs() < 0.01);
    assert!((o.stair.base + drop).abs() < 0.01);
    // The stair's top is where the pointer went down, its bottom where it
    // came up.
    let (top, _) = plan_stairs::top_point(&o.stair);
    assert!(top.dist(Point::new(96.0, 100.0)) < 12.0, "{top:?}");
    // Without terrain, a 30" stoop.
    let bare = house();
    let o = stairs_view::build_down(
        &bare.app.cx.project,
        0,
        StairKind::Draw,
        plan_stairs::Turn::Left,
        Point::new(100.0, 100.0),
        Some(Point::new(100.0, 250.0)),
    );
    assert!((o.stair.params.total_rise - stairs_view::DEFAULT_DROP).abs() < 1e-9);
}

// ----- stairwell and the Edit toolbar -----

#[test]
fn auto_stairwell_is_offered_for_a_stair_that_reaches_a_floor_above_and_undoes_in_one_step() {
    let mut sim = house();
    sim.action(crate::toolbar::Action::BuildNewFloor);
    sim.ok();
    assert_eq!(sim.app.cx.project.floors.len(), 2);
    sim.action(crate::toolbar::Action::FloorDown);
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((100.0, 100.0), (100.0, 300.0));
    let o = only(&sim);
    let cmds = stairs_view::edit_commands(&sim.app.cx);
    assert!(cmds
        .iter()
        .any(|(c, on)| *c == stairs_view::StairCommand::AutoStairwell && *on));
    assert!(stairs_view::run_command(
        &mut sim.app.cx,
        stairs_view::StairCommand::AutoStairwell
    ));
    assert_eq!(sim.undo().as_deref(), Some("Auto Stairwell"));
    assert_eq!(only(&sim).stair, o.stair);
}

// ----- Alt and right-drag, edit modes, radius handles, sections -----

/// Press at `a`, drag to `b` and release with the Alt key held or the right
/// mouse button, as the stair tools read them.
fn drag_reversed(sim: &mut Sim, a: (f64, f64), b: (f64, f64), right_button: bool) {
    use eframe::egui::{Modifiers, PointerButton};
    let mods = if right_button {
        Modifiers::NONE
    } else {
        Modifiers::ALT
    };
    let button = if right_button {
        PointerButton::Secondary
    } else {
        PointerButton::Primary
    };
    let mk = |sim: &Sim, p: (f64, f64), down: bool| {
        let mut e = sim.event(p.0, p.1).with_down(down).with_modifiers(mods);
        e.button = button;
        e
    };
    let e = mk(sim, a, false);
    let r = sim.app.tools.active_mut().pointer_move(&mut sim.app.cx, e);
    sim.finish(r);
    let e = mk(sim, a, true);
    let r = sim.app.tools.active_mut().pointer_down(&mut sim.app.cx, e);
    sim.finish(r);
    let e = mk(sim, b, true);
    let r = sim.app.tools.active_mut().pointer_move(&mut sim.app.cx, e);
    sim.finish(r);
    let e = mk(sim, b, false);
    let r = sim.app.tools.active_mut().pointer_up(&mut sim.app.cx, e);
    sim.finish(r);
}

#[test]
fn alt_or_the_right_button_draws_the_stair_downward_to_the_terrain() {
    for right in [false, true] {
        let mut sim = deck_sim();
        flat_terrain(&mut sim, 52.0);
        sim.tool(ToolId::StairsVariant(StairKind::Draw));
        drag_reversed(&mut sim, (96.0, 120.0), (96.0, 320.0), right);
        let o = only(&sim);
        assert!(o.stair.params.down, "right button: {right}");
        assert!((o.stair.params.total_rise - 52.0).abs() < 1.0);
        assert!((o.stair.bottom_elevation() + 52.0).abs() < 1.0);
        assert!(
            sim.app.cx.status.starts_with("Stairs down"),
            "{}",
            sim.app.cx.status
        );
        // The plan says DN at the top of the run.
        let labels = stairs_view::riser_label(&o);
        assert!(!labels.is_empty());
        assert_eq!(sim.undo().as_deref(), Some("Draw Stairs Down"));
    }
}

#[test]
fn flare_curve_and_starter_tread_are_edit_modes_with_their_own_handles() {
    use crate::editor::stairs_view::{EditMode, StairCommand, StairHandleKind as K};
    let mut sim = house();
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((60.0, 100.0), (60.0, 300.0));
    let o = only(&sim);
    let id = o.id();
    sim.app.cx.selection.set(ObjectRef::Stair(id));
    let kinds = |o: &StairObj| -> Vec<K> {
        stairs_view::handles(o, sim.app.cx.px_per_in)
            .iter()
            .map(|h| h.kind)
            .collect()
    };
    assert!(!kinds(&o).contains(&K::Flare(0)));
    // Flare/Curve Stairs switches the handles without touching the stair.
    assert!(!stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::FlareCurve
    ));
    assert_eq!(stairs_view::edit_mode(id), EditMode::FlareCurve);
    let hs = stairs_view::handles(&o, sim.app.cx.px_per_in);
    for k in [
        K::Flare(0),
        K::Flare(1),
        K::Flare(2),
        K::Flare(3),
        K::FlareStart,
        K::FlareSoften,
        K::CurveAll,
    ] {
        assert!(hs.iter().any(|h| h.kind == k), "{k:?}");
    }
    // Dragging the bottom left corner 12" out widens the bottom treads there.
    let h = hs.iter().find(|h| h.kind == K::Flare(0)).unwrap();
    let to = Point::new(h.pos.x - 12.0, h.pos.y);
    let flared = stairs_view::drag_handle(&o, K::Flare(0), h.pos, to);
    assert!((flared.stair.params.flare_shape.corners[0] - 12.0).abs() < 0.01);
    let wide = |s: &StairObj| extent(&tagged_meshes(&s.stair), StairPart::Tread).0[0];
    assert!(
        wide(&flared) < wide(&o) - 5.0,
        "{} vs {}",
        wide(&flared),
        wide(&o)
    );
    // The curve handle bulges the treads; the start handle limits the flare.
    let curve = hs.iter().find(|h| h.kind == K::CurveAll).unwrap();
    let curved = stairs_view::drag_handle(
        &o,
        K::CurveAll,
        curve.pos,
        Point::new(curve.pos.x, curve.pos.y - 4.0),
    );
    assert!(curved.stair.params.flare_shape.curve_all > 3.0);
    // Starter Tread: a click on its handle goes round none, one, two.
    assert!(!stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::FlareCurve
    ));
    assert_eq!(stairs_view::edit_mode(id), EditMode::Normal);
    assert!(!stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::StarterTread
    ));
    let hs = stairs_view::handles(&o, sim.app.cx.px_per_in);
    let st = hs
        .iter()
        .find(|h| h.kind == K::Starter)
        .expect("starter handle");
    let one = stairs_view::drag_handle(&o, K::Starter, st.pos, st.pos);
    assert_eq!(one.stair.params.starter, plan_stairs::Starter::One);
    let two = stairs_view::drag_handle(&one, K::Starter, st.pos, st.pos);
    assert_eq!(two.stair.params.starter, plan_stairs::Starter::Two);
    assert!(
        tagged_meshes(&two.stair).len() != tagged_meshes(&o.stair).len() || wide(&two) < wide(&o),
        "starter treads change the bottom steps"
    );
    stairs_view::set_edit_mode(id, EditMode::Normal);
}

#[test]
fn the_inner_and_outer_radius_handles_resize_a_curved_stair() {
    use crate::editor::stairs_view::StairHandleKind as K;
    let mut sim = house();
    sim.tool(ToolId::StairsVariant(StairKind::Curved));
    sim.drag((240.0, 180.0), (240.0, 120.0));
    let o = only(&sim);
    let c = plan_stairs::curve_center(&o.stair).expect("a centre");
    let hs = stairs_view::handles(&o, sim.app.cx.px_per_in);
    let outer = hs.iter().find(|h| h.kind == K::OuterRadius).expect("outer");
    let inner = hs.iter().find(|h| h.kind == K::InnerRadius).expect("inner");
    let radial = |p: Point, extra: f64| {
        let v = p - c;
        c + v * ((v.length() + extra) / v.length())
    };
    // 20" more on the outside edge: the stair is 20" wider and the inside
    // radius stays.
    let bigger = stairs_view::drag_handle(&o, K::OuterRadius, outer.pos, radial(outer.pos, 20.0));
    let (w0, w1) = (o.stair.params.width, bigger.stair.params.width);
    assert!((w1 - w0 - 20.0).abs() < 2.0, "{w0} -> {w1}");
    let StairShape::Curved { inner_radius: r0 } = o.stair.params.shape else {
        panic!()
    };
    let StairShape::Curved { inner_radius: r1 } = bigger.stair.params.shape else {
        panic!()
    };
    assert!((r1 - r0).abs() < 1e-6);
    // 12" more on the inside edge: the inside radius grows, the outside stays.
    let moved = stairs_view::drag_handle(&o, K::InnerRadius, inner.pos, radial(inner.pos, 12.0));
    let StairShape::Curved { inner_radius: r2 } = moved.stair.params.shape else {
        panic!()
    };
    assert!(r2 > r0 + 8.0, "{r0} -> {r2}");
    assert!(
        ((r2 + moved.stair.params.width) - (r0 + w0)).abs() < 1.0,
        "the outside edge stays"
    );
    // The same handles resize a curved ramp.
    sim.undo();
    sim.tool(ToolId::StairsVariant(StairKind::CurvedRamp));
    sim.drag((240.0, 180.0), (240.0, 120.0));
    let ramp = only(&sim);
    let hs = stairs_view::handles(&ramp, sim.app.cx.px_per_in);
    assert!(hs.iter().any(|h| h.kind == K::OuterRadius));
}

#[test]
fn complete_break_and_disconnect_turn_one_stair_into_sections_in_one_undo_step() {
    use crate::editor::stairs_view::StairCommand;
    let mut sim = house();
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((60.0, 100.0), (60.0, 300.0));
    let id = only(&sim).id();
    let whole = only(&sim);
    sim.app.cx.selection.set(ObjectRef::Stair(id));
    assert!(stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::CompleteBreak
    ));
    let all = stairs(&sim);
    assert_eq!(all.len(), 3, "a flight, a landing and a flight");
    assert_eq!(all.iter().filter(|o| o.is_landing()).count(), 1);
    assert!(sim.app.cx.status.starts_with("Complete Break"));
    assert_eq!(sim.undo().as_deref(), Some("Complete Break"));
    assert_eq!(only(&sim).stair, whole.stair);

    sim.undo();
    sim.tool(ToolId::StairsVariant(StairKind::LShaped));
    sim.click(200.0, 150.0);
    let l = only(&sim);
    sim.app.cx.selection.set(ObjectRef::Stair(l.id()));
    assert!(stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::DisconnectSubsection
    ));
    assert!(stairs(&sim).len() >= 3);
    assert_eq!(sim.undo().as_deref(), Some("Disconnect Subsection"));
    assert_eq!(only(&sim).stair, l.stair);
    // A ramp or a landing offers neither.
    sim.undo();
    sim.tool(ToolId::StairsVariant(StairKind::Ramp));
    sim.drag((40.0, 200.0), (400.0, 200.0));
    let ramp = only(&sim);
    sim.app.cx.selection.set(ObjectRef::Stair(ramp.id()));
    assert!(!stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::CompleteBreak
    ));
    assert!(!stairs_view::run_command(
        &mut sim.app.cx,
        StairCommand::DisconnectSubsection
    ));
    assert_eq!(stairs(&sim).len(), 1);
}
