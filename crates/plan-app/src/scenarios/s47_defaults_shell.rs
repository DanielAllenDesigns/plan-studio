//! Scenario 47 (round 15): the 29 groups of Edit > Default Settings (every
//! leaf opens and every field of every generic page round-trips) and the
//! application-shell view commands (rubber-band Zoom, Zoom Previous, Fill
//! Window Building Only, Reverse Plan, Rotate Plan View, tiling, Swap Views,
//! tab cycling, Close All Views).

use super::{draw_shell, Sim};
use crate::dialogs::default_pages::{self, page::Lists, PageSpec};
use crate::dialogs::defaults::{self, edit_outcome, DefaultsOutcome, Leaf};
use crate::shell::view_commands::{self as vc, Tile, ViewEntry};
use crate::toolbar::Action;
use eframe::egui::{Key, Pos2};
use plan_core::cad::CadItem;
use plan_core::defaults::{PageValue, PlanDefaults};
use plan_core::geometry::Point;
use plan_core::OpeningKind;

fn custom(sim: &mut Sim, id: &'static str) {
    sim.action(Action::Custom(id));
}

// ===================================================================
// Default Settings
// ===================================================================

/// A value different from `cur` that the field accepts, if it has one.
fn changed(f: &default_pages::page::Field, cur: &PageValue, lists: &Lists) -> Option<PageValue> {
    use default_pages::page::{Kind, ListSource};
    Some(match &f.kind {
        Kind::Flag => PageValue::Bool(!matches!(cur, PageValue::Bool(true))),
        Kind::Length | Kind::Degrees | Kind::Number(_) => PageValue::Num(cur.num() + 3.0),
        Kind::Int(lo, hi, _) => {
            let n = match cur {
                PageValue::Int(n) => *n,
                other => other.num() as i64,
            };
            PageValue::Int(if n < *hi { n + 1 } else { (n - 1).max(*lo) })
        }
        Kind::Choice(names) => {
            let now = cur.text();
            PageValue::Text(names.iter().find(|n| **n != now)?.to_string())
        }
        Kind::List(src) => {
            let list = match src {
                ListSource::WallTypes => &lists.wall_types,
                ListSource::TextStyles => &lists.text_styles,
                ListSource::DimensionSets => &lists.dimension_sets,
                ListSource::RoomTypes => &lists.room_types,
            };
            let now = cur.text();
            PageValue::Text(list.iter().find(|n| n.as_str() != now)?.clone())
        }
        Kind::Text => PageValue::Text(format!("{}x", cur.text())),
        Kind::Color => PageValue::Text(if cur.text() == "#123456" {
            "#654321".into()
        } else {
            "#123456".into()
        }),
    })
}

fn same_value(a: &PageValue, b: &PageValue) -> bool {
    match (a, b) {
        (PageValue::Num(x), PageValue::Num(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

#[test]
fn the_tree_has_chiefs_29_groups_and_every_leaf_is_named() {
    let leaves = defaults::all_leaves();
    let mut groups: Vec<&str> = leaves.iter().map(|(g, _, _)| *g).collect();
    groups.dedup();
    assert_eq!(groups.len(), 29, "groups: {groups:?}");
    for must in [
        "3D Solid",
        "3D View Defaults",
        "Cabinets",
        "CAD",
        "Camera Tools",
        "Dimension",
        "Doors",
        "Dormer",
        "Electrical",
        "Floors and Rooms",
        "Foundation",
        "Framing",
        "General",
        "Materials",
        "Plan",
        "Railing and Deck",
        "Roofs",
        "Schedules",
        "Slab",
        "Stairs",
        "Terrain",
        "Text",
        "Walls",
        "Windows",
    ] {
        assert!(groups.contains(&must), "missing group {must}");
    }
    // Every page a leaf names exists.
    for (g, n, l) in &leaves {
        if let Leaf::Page(id) = l {
            assert!(default_pages::spec(id).is_some(), "{g} > {n}: no page {id}");
        }
    }
}

#[test]
fn every_field_of_every_generic_page_round_trips_through_the_template() {
    let ids = default_pages::all_ids();
    assert!(ids.len() >= 60, "only {} pages", ids.len());
    let mut fields = 0;
    for id in &ids {
        let spec: PageSpec = default_pages::spec(id).unwrap_or_else(|| panic!("no page {id}"));
        let base = crate::plan_defaults::embedded();
        let lists = Lists::of(&base);
        for f in spec.fields() {
            let cur = f.read(&base);
            let Some(v) = changed(f, &cur, &lists) else {
                continue;
            };
            let mut d: PlanDefaults = base.clone();
            let mut page = default_pages::GenericPage::new(spec.clone(), &d);
            page.set(&f.key, v.clone());
            assert!(page.error().is_none(), "{id}/{}: {:?}", f.key, page.error());
            page.apply(&mut d);
            let got = f.read(&d);
            assert!(
                same_value(&got, &v),
                "{id}/{}: wrote {v:?}, read {got:?}",
                f.key
            );
            // Saved with the template and read back.
            let json = serde_json::to_string(&d).unwrap();
            let back: PlanDefaults = serde_json::from_str(&json).unwrap();
            assert!(
                same_value(&f.read(&back), &v),
                "{id}/{}: lost in the template JSON",
                f.key
            );
            // Reset Page puts the built-in value back.
            page.reset();
            let mut e = d.clone();
            page.apply(&mut e);
            assert!(
                same_value(&f.read(&e), &f.read(&PlanDefaults::default())) || f.bind.is_some(),
                "{id}/{}: Reset Page did not restore the built-in value",
                f.key
            );
            fields += 1;
        }
    }
    assert!(fields >= 150, "only {fields} fields round-tripped");
}

#[test]
fn every_leaf_of_the_tree_opens_its_window_and_a_page_saves_one_field() {
    let mut sim = Sim::new();
    let mut pages = 0;
    for (group, name, leaf) in defaults::all_leaves() {
        let what = format!("{group} > {name}");
        match leaf {
            Leaf::Page(id) => {
                assert_eq!(edit_outcome(leaf), DefaultsOutcome::Open, "{what}");
                sim.dialog_frame(false);
                sim.dialog_frame(false);
                assert_eq!(default_pages::open_id().as_deref(), Some(id), "{what}");
                // Change the first field that has a different value.
                let lists = Lists::of(&sim.app.cx.defaults);
                let change = default_pages::with_open(|p| {
                    let spec = p.spec().clone();
                    for f in spec.fields() {
                        let cur = f.read(&sim.app.cx.defaults);
                        if let Some(v) = changed(f, &cur, &lists) {
                            p.set(&f.key, v.clone());
                            return Some((f.clone(), v));
                        }
                    }
                    None
                })
                .unwrap_or_else(|| panic!("{what}: the page is not open"));
                let (f, v) = change.unwrap_or_else(|| panic!("{what}: no editable field"));
                sim.ok();
                let got = f.read(&sim.app.cx.defaults);
                assert!(same_value(&got, &v), "{what}/{}: {got:?} != {v:?}", f.key);
                assert!(default_pages::open_id().is_none(), "{what}: OK closes it");
                pages += 1;
            }
            Leaf::Entry(e) => {
                assert_eq!(edit_outcome(leaf), DefaultsOutcome::Edit(e), "{what}");
                sim.app.open_defaults_entry(e);
                sim.dialog_frame(false);
                assert!(
                    sim.app.has_dialog() || !sim.app.cx.status.is_empty(),
                    "{what}: nothing opened"
                );
                sim.cancel();
            }
            Leaf::RoofDefaults => {
                edit_outcome(leaf);
                sim.dialog_frame(false);
                assert!(defaults::roof_defaults_open(), "{what}");
                sim.cancel();
            }
            Leaf::CabinetDefaults(_) => {
                edit_outcome(leaf);
                sim.dialog_frame(false);
                assert!(defaults::cabinet_defaults_open(), "{what}");
                sim.cancel();
            }
            Leaf::FramingDefaults(_) => {
                edit_outcome(leaf);
                sim.dialog_frame(false);
                assert!(defaults::framing_defaults_open(), "{what}");
                sim.cancel();
            }
            Leaf::TerrainDefaults => {
                edit_outcome(leaf);
                sim.dialog_frame(false);
                assert!(
                    crate::dialogs::default_settings_terrain::is_open(),
                    "{what}"
                );
                sim.cancel();
            }
            Leaf::Electrical => {
                edit_outcome(leaf);
                sim.dialog_frame(false);
                assert!(default_pages::electrical::is_open(), "{what}");
                sim.cancel();
            }
            Leaf::Platforms => {
                edit_outcome(leaf);
                sim.dialog_frame(false);
                assert!(crate::dialogs::assembly_def::page_is_open(), "{what}");
                sim.cancel();
            }
            Leaf::Run(action) => {
                assert_eq!(edit_outcome(leaf), DefaultsOutcome::Run(action), "{what}");
                sim.action(action);
                sim.dialog_frame(false);
            }
        }
    }
    assert!(pages >= 60, "only {pages} generic pages were driven");
}

#[test]
fn reset_to_template_puts_the_defaults_back_but_not_the_plan() {
    let mut sim = Sim::new();
    let template = crate::plan_defaults::template_base().0;
    draw_shell(&mut sim, 240.0, 180.0);
    let walls = sim.floor_walls();
    sim.cx().defaults.grid.spacing = 77.0;
    default_pages::request_open("general");
    sim.dialog_frame(false);
    sim.cancel();
    custom(&mut sim, defaults::RESET_TO_DEFAULTS);
    sim.dialog_frame(false);
    assert_eq!(sim.cx().defaults.grid.spacing, template.grid.spacing);
    assert_eq!(sim.floor_walls(), walls, "the plan's own data is untouched");
}

// ===================================================================
// Window and view commands
// ===================================================================

fn shell_plan(sim: &mut Sim) {
    draw_shell(sim, 240.0, 180.0);
    sim.app.camera.rect =
        eframe::egui::Rect::from_min_size(Pos2::ZERO, eframe::egui::vec2(800.0, 600.0));
}

#[test]
fn zoom_then_drag_zooms_to_the_rectangle_and_undo_zoom_comes_back() {
    let mut sim = Sim::new();
    shell_plan(&mut sim);
    let before = sim.app.camera;
    custom(&mut sim, vc::ZOOM_WINDOW);
    assert!(sim.app.shell_views.zoom_armed);
    sim.app
        .zoom_window_drag(Pos2::new(100.0, 100.0), Pos2::new(300.0, 250.0));
    assert!(
        !sim.app.shell_views.zoom_armed,
        "one drag, then the Zoom is off"
    );
    let z = sim.app.camera;
    assert!((z.px_per_in / before.px_per_in - 4.0).abs() < 1e-6);
    let mid = before.screen_to_world(Pos2::new(200.0, 175.0));
    assert!((z.center.x - mid.x).abs() < 1e-6 && (z.center.y - mid.y).abs() < 1e-6);
    sim.action(Action::UndoZoom);
    assert!((sim.app.camera.px_per_in - before.px_per_in).abs() < 1e-9);
    // A click zooms in on the point.
    custom(&mut sim, vc::ZOOM_WINDOW);
    sim.app
        .zoom_window_drag(Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    assert!((sim.app.camera.px_per_in / before.px_per_in - 2.0).abs() < 1e-6);
    // Arming twice switches it off again.
    custom(&mut sim, vc::ZOOM_WINDOW);
    custom(&mut sim, vc::ZOOM_WINDOW);
    assert!(!sim.app.shell_views.zoom_armed);
}

#[test]
fn zoom_previous_toggles_between_the_last_two_zooms() {
    let mut sim = Sim::new();
    shell_plan(&mut sim);
    let a = sim.app.camera.px_per_in;
    sim.action(Action::ZoomIn);
    let b = sim.app.camera.px_per_in;
    assert!(b > a);
    custom(&mut sim, vc::ZOOM_PREVIOUS);
    assert!((sim.app.camera.px_per_in - a).abs() < 1e-9);
    custom(&mut sim, vc::ZOOM_PREVIOUS);
    assert!((sim.app.camera.px_per_in - b).abs() < 1e-9);
}

#[test]
fn fill_window_building_only_frames_the_walls_and_fill_window_everything() {
    let mut sim = Sim::new();
    shell_plan(&mut sim);
    let (wlo, whi) = vc::building_bounds(&sim.app.cx).unwrap();
    // A note far off to the right of the house.
    sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(900.0, 40.0),
            b: Point::new(1000.0, 40.0),
        },
    );
    sim.app.cx.refresh();
    custom(&mut sim, vc::FILL_BUILDING);
    let c = sim.app.camera;
    assert!(
        (c.center.x - (wlo.x + whi.x) / 2.0).abs() < 1e-6,
        "the building is centred"
    );
    let building_scale = c.px_per_in;
    sim.action(Action::FillWindow);
    let all = sim.app.camera;
    assert!(
        all.px_per_in < building_scale,
        "Fill Window takes in the CAD line as well"
    );
    assert!(all.center.x > c.center.x);
    // Undo Zoom steps back through both.
    sim.action(Action::UndoZoom);
    assert!((sim.app.camera.px_per_in - building_scale).abs() < 1e-9);
}

#[test]
fn rotate_plan_view_turns_only_the_view() {
    let mut sim = Sim::new();
    shell_plan(&mut sim);
    let walls_before = format!("{:?}", sim.app.cx.floor().walls);
    let p = Point::new(240.0, 0.0);
    let s0 = sim.app.camera.world_to_screen(p);
    custom(&mut sim, vc::ROTATE_LEFT);
    assert!((sim.app.camera.rotation - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    let s1 = sim.app.camera.world_to_screen(p);
    assert!(
        (s0.x - s1.x).abs() > 1.0 || (s0.y - s1.y).abs() > 1.0,
        "the point moved on screen"
    );
    // The pointer maps back through the rotation.
    let back = sim.app.camera.screen_to_world(s1);
    assert!((back.x - p.x).abs() < 1e-3 && (back.y - p.y).abs() < 1e-3);
    assert_eq!(
        format!("{:?}", sim.app.cx.floor().walls),
        walls_before,
        "the plan did not change"
    );
    assert!(!sim.app.cx.can_undo() || sim.app.cx.undo_label() != Some("Rotate Plan View"));
    // The plan still draws (dimensions and text follow the turned view).
    assert!(!sim.plan_shapes().is_empty());
    custom(&mut sim, vc::ROTATE_RIGHT);
    custom(&mut sim, vc::ROTATE_RIGHT);
    custom(&mut sim, vc::ROTATE_RESET);
    assert_eq!(sim.app.camera.rotation, 0.0);
}

#[test]
fn reverse_plan_mirrors_everything_as_one_undo_step_and_text_stays_readable() {
    let mut sim = Sim::new();
    shell_plan(&mut sim);
    let wall = sim.wall_ids()[0];
    sim.app
        .cx
        .project
        .add_opening(0, wall, 60.0, OpeningKind::Door)
        .expect("a door");
    sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(10.0, 10.0),
            b: Point::new(60.0, 30.0),
        },
    );
    let text = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Text {
            pos: Point::new(20.0, 90.0),
            text: "Kitchen".into(),
            height: 6.0,
            angle: 0.0,
        },
    );
    sim.app.cx.refresh();
    let (lo, hi) = vc::building_bounds(&sim.app.cx).unwrap();
    let mid = (lo.x + hi.x) / 2.0;
    let openings = sim.app.cx.floor().openings.len();
    let walls = sim.floor_walls();
    let before = sim.app.cx.project.clone();
    let depth_label = sim.app.cx.undo_label().map(str::to_string);

    custom(&mut sim, vc::REVERSE_PLAN);
    assert_eq!(sim.app.cx.undo_label(), Some("Reverse Plan"));
    assert_eq!(sim.floor_walls(), walls);
    assert_eq!(sim.app.cx.floor().openings.len(), openings);
    // The CAD line is mirrored about the middle of the plan.
    let line = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find_map(|c| match &c.item {
            CadItem::Line { a, b } => Some((*a, *b)),
            _ => None,
        })
        .unwrap();
    assert!((line.0.x - (2.0 * mid - 10.0)).abs() < 1e-6, "{line:?}");
    assert!((line.1.x - (2.0 * mid - 60.0)).abs() < 1e-6);
    assert!((line.0.y - 10.0).abs() < 1e-6, "left to right only");
    // Text keeps reading left to right.
    match &sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == text)
        .unwrap()
        .item
    {
        CadItem::Text { angle, pos, .. } => {
            assert!(angle.cos() > 0.0, "upright, not upside down: {angle}");
            assert!(pos.x > mid, "it moved to the other side");
        }
        other => panic!("{other:?}"),
    }
    // One undo step brings it all back; redo does it again.
    sim.undo();
    assert!(super::same(
        &sim.app.cx.project.floors[0].walls,
        &before.floors[0].walls
    ));
    assert!(super::same(
        &sim.app.cx.project.floors[0].cad,
        &before.floors[0].cad
    ));
    assert_eq!(sim.app.cx.undo_label().map(str::to_string), depth_label);
    sim.redo();
    assert_eq!(sim.app.cx.undo_label(), Some("Reverse Plan"));
    // Reversing twice is the identity.
    custom(&mut sim, vc::REVERSE_PLAN);
    let again = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find_map(|c| match &c.item {
            CadItem::Line { a, .. } => Some(*a),
            _ => None,
        })
        .unwrap();
    assert!((again.x - 10.0).abs() < 1e-6);
}

fn open_3d(sim: &mut Sim) {
    sim.app.view3d.active = true;
    sim.app.sync_views();
}

#[test]
fn tile_swap_and_the_tab_ring_follow_the_open_views() {
    let mut sim = Sim::new();
    shell_plan(&mut sim);
    // With one view there is nothing to tile, swap or cycle to.
    custom(&mut sim, vc::TILE_VERTICALLY);
    assert_eq!(sim.app.shell_views.tile, Tile::Tabs);
    custom(&mut sim, vc::NEXT_TAB);
    assert!(
        sim.app.cx.status.contains("only one view"),
        "{}",
        sim.app.cx.status
    );

    open_3d(&mut sim);
    assert_eq!(sim.app.view_entries().len(), 2);
    custom(&mut sim, vc::TILE_VERTICALLY);
    assert_eq!(sim.app.shell_views.tile, Tile::Vertical);
    custom(&mut sim, vc::TILE_HORIZONTALLY);
    assert_eq!(sim.app.shell_views.tile, Tile::Horizontal);
    assert!(!sim.app.shell_views.swapped);
    custom(&mut sim, vc::SWAP_VIEWS);
    assert!(sim.app.shell_views.swapped, "tiled: the panes trade places");
    custom(&mut sim, vc::TAB_WINDOWS);
    assert_eq!(sim.app.shell_views.tile, Tile::Tabs);

    // Tabs: Swap Views trades the plan and the 3D view.
    assert!(sim.app.view3d.active);
    custom(&mut sim, vc::SWAP_VIEWS);
    assert!(!sim.app.view3d.active);
    custom(&mut sim, vc::SWAP_VIEWS);
    assert!(sim.app.view3d.active);

    // Ctrl+Tab goes round the ring both ways.
    custom(&mut sim, vc::NEXT_TAB);
    assert!(
        !sim.app.view3d.active,
        "3D was last, next wraps to the plan"
    );
    custom(&mut sim, vc::NEXT_TAB);
    assert!(sim.app.view3d.active);
    custom(&mut sim, vc::PREVIOUS_TAB);
    assert_eq!(
        sim.app.current_view_entry(),
        ViewEntry::Plan(sim.app.cx.project.active_plan_view.clone())
    );
}

#[test]
fn the_tab_ring_visits_every_plan_view_then_the_3d_view() {
    let mut sim = Sim::new();
    shell_plan(&mut sim);
    let first = sim.app.cx.project.active_plan_view.clone();
    assert!(sim
        .app
        .cx
        .project
        .add_plan_view(plan_core::SavedPlanView::new("Second", "Default Set")));
    crate::editor::plan_tabs::with_tabs(|t| t.open_view(&mut sim.app.cx, "Second"));
    open_3d(&mut sim);
    crate::editor::plan_tabs::with_tabs(|t| t.sync(&sim.app.cx.project));
    let entries = sim.app.view_entries();
    assert_eq!(entries.len(), 3, "{entries:?}");
    assert_eq!(entries[2], ViewEntry::ThreeD);
    // From the 3D view: next is the first plan tab, then the second.
    custom(&mut sim, vc::NEXT_TAB);
    let on_show = sim.app.current_view_entry();
    assert_eq!(on_show, entries[0].clone());
    custom(&mut sim, vc::NEXT_TAB);
    assert_eq!(sim.app.current_view_entry(), entries[1].clone());
    assert_ne!(entries[0], entries[1]);
    let _ = first;
}

#[test]
fn close_view_and_close_all_views_leave_one_plan_view() {
    let mut sim = Sim::new();
    shell_plan(&mut sim);
    assert!(sim
        .app
        .cx
        .project
        .add_plan_view(plan_core::SavedPlanView::new("Second", "Default Set")));
    crate::editor::plan_tabs::with_tabs(|t| t.open_view(&mut sim.app.cx, "Second"));
    open_3d(&mut sim);
    custom(&mut sim, vc::TILE_VERTICALLY);
    // Close View closes the 3D view on show and ends the tiling.
    custom(&mut sim, vc::CLOSE_VIEW);
    assert!(!sim.app.view3d.active && !sim.app.shell_views.open_3d);
    assert_eq!(sim.app.shell_views.tile, Tile::Tabs);
    // Close All Views: back to one plan view.
    open_3d(&mut sim);
    custom(&mut sim, vc::CLOSE_ALL_VIEWS);
    assert!(!sim.app.view3d.active && !sim.app.shell_views.open_3d);
    assert!(!crate::shell::layout_window::is_active());
    let tabs = crate::editor::plan_tabs::with_tabs(|t| {
        t.sync(&sim.app.cx.project);
        t.open().len()
    });
    assert_eq!(tabs, 1, "only the plan view on show stays");
    assert_eq!(sim.app.view_entries().len(), 1);
    // The Layout view joins the ring while it is open.
    sim.action(Action::Layout(
        crate::shell::layout_window::LayoutCommand::ShowLayout,
    ));
    assert!(crate::shell::layout_window::is_active());
    assert!(sim.app.view_entries().contains(&ViewEntry::Layout));
    custom(&mut sim, vc::CLOSE_VIEW);
    assert!(!crate::shell::layout_window::is_active());
    assert!(!sim.app.shell_views.open_layout);
}

#[test]
fn the_tiled_window_draws_both_views_in_a_headless_frame() {
    let mut sim = Sim::new();
    shell_plan(&mut sim);
    open_3d(&mut sim);
    custom(&mut sim, vc::TILE_VERTICALLY);
    let ctx = sim.ctx.clone();
    let input = eframe::egui::RawInput {
        screen_rect: Some(eframe::egui::Rect::from_min_size(
            Pos2::ZERO,
            eframe::egui::vec2(1200.0, 700.0),
        )),
        ..Default::default()
    };
    let mut drew = false;
    let _ = ctx.run(input, |ctx| {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            drew = sim.app.tiled_central(ctx, ui);
        });
    });
    assert!(drew, "the tiled central panel took over");
    // The plan pane is half the window.
    assert!(
        sim.app.camera.rect.width() < 700.0,
        "{:?}",
        sim.app.camera.rect
    );
}

#[test]
fn the_window_menu_and_hotkeys_reach_the_commands() {
    // Every command id is dispatched by the shell, not left to the generic path.
    for id in [
        vc::ZOOM_WINDOW,
        vc::ZOOM_PREVIOUS,
        vc::FILL_BUILDING,
        vc::REVERSE_PLAN,
        vc::TILE_HORIZONTALLY,
        vc::TILE_VERTICALLY,
        vc::SWAP_VIEWS,
        vc::NEXT_TAB,
        vc::PREVIOUS_TAB,
        vc::CLOSE_ALL_VIEWS,
    ] {
        assert!(vc::is_command(id), "{id}");
    }
    let menus = include_str!("../menus.rs");
    for needle in [
        "Fill Window Building Only",
        "Tile Horizontally",
        "Tile Vertically",
        "Swap Views",
        "Select Next Tab",
        "Select Previous Tab",
        "Rotate Plan View",
        "Zoom Previous",
        "Reverse Plan",
        "Close All Views",
    ] {
        assert!(menus.contains(needle), "the menus have no {needle} row");
    }
    let main = include_str!("../main.rs");
    assert!(main.contains("Key::Tab") && main.contains("Key::F7"));
    let _ = Key::Tab;
}
