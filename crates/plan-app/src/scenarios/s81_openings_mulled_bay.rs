//! Scenario 81: door and window defaults per type, automatic mulling, Make
//! Mulled Unit, window levels, the Caution symbol, and bay, box and bow
//! windows as wall-section units (DW-48, DW-124, DW-154, DW-155, DW-158..DW-161,
//! DW-164, DW-167; manual pp. 103, 571, 603 to 642).
//!
//! Place two windows close together and they mull on their own; give one a
//! lintel and set it as the default and the other, which uses the default,
//! follows; block a unit, pick its components, explode it; put a bay window in,
//! see its roof, foundation and dimensions, edit its dialog and explode it.
//! Every user action is one undo step.

use super::{draw_shell, shape_colors, Sim};
use crate::editor::opening_edit::{
    DELETE_DUPLICATE, EXPLODE_BAY, MULL, SELECT_NEXT, SET_DEFAULT, UNMULL,
};
use crate::editor::opening_view::opening_labels;
use crate::editor::ObjectRef;
use crate::tools::opening::OpeningVariant;
use crate::tools::ToolId;
use crate::ActiveDialog;
use eframe::egui;
use plan_3d::{build_scene, Material};
use plan_core::geometry::Point;
use plan_core::opening_symbol::plan_symbol;
use plan_core::openings::bay::{roof_plane_count, BayUnit};
use plan_core::openings::types::DefaultKey;
use plan_core::openings::{Jamb, OpeningStyle};
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

fn get(sim: &Sim, id: Id) -> Opening {
    openings(sim)
        .into_iter()
        .find(|o| o.id == id)
        .unwrap_or_else(|| panic!("no opening {id}"))
}

fn top_wall(sim: &Sim) -> Id {
    sim.app.cx.floor().walls[0].id
}

/// Clicks `x` along the top wall with the Window tool and returns the new id.
fn window_at(sim: &mut Sim, x: f64) -> Id {
    sim.tool(ToolId::Window);
    let before = openings(sim).len();
    sim.click(x, 0.5);
    let v = openings(sim);
    assert_eq!(v.len(), before + 1, "{:?}", sim.app.cx.status);
    v.last().unwrap().id
}

fn label(sim: &Sim) -> Option<String> {
    sim.app.cx.undo_label().map(String::from)
}

fn select(sim: &mut Sim, ids: &[Id]) {
    sim.app.cx.selection.clear();
    for id in ids {
        sim.app.cx.selection.toggle(ObjectRef::Opening(*id));
    }
}

fn push_window(sim: &mut Sim, wall: Id, center: f64, sill: f64, height: f64) -> Id {
    let id = sim.app.cx.project.alloc_id();
    let mut o = Opening::default_window(id, wall, center);
    o.width = 36.0;
    o.sill_height = sill;
    o.height = height;
    sim.app.cx.project.floors[0].openings.push(o);
    id
}

// ----- automatic mulling -----

#[test]
fn windows_placed_together_mull_automatically_and_share_one_casing() {
    let mut sim = house();
    let a = window_at(&mut sim, 100.0);
    let b = window_at(&mut sim, 135.0);
    let (wa, wb) = (get(&sim, a), get(&sim, b));
    // The Minimum Separation (2 in to start with) is the gap, and the casing
    // the two share is as wide.
    let gap = wb.start_offset() - wa.end_offset();
    assert!((gap - 2.0).abs() < 1e-9, "gap {gap}");
    assert_eq!(
        plan_core::openings::mull::shared_casing_width(&wa, &wb),
        Some(2.0)
    );
    assert_eq!(sim.app.cx.project.auto_mull_members(0, a), vec![b]);
    // They stay two objects: no blocked unit.
    assert!(wa.mull_group.is_none() && wb.mull_group.is_none());
    // One casing goes around the pair.
    let span = sim.app.cx.project.casing_span(0, a).unwrap();
    assert_eq!(span, (wa.start_offset(), wb.end_offset()));
    // Sliding the second one against the first is refused: the separation holds.
    assert!(!sim
        .app
        .cx
        .project
        .slide_opening(0, b, wa.end_offset() + 1.0 + wb.width * 0.5));
    // A window far away mulls with nobody.
    let c = window_at(&mut sim, 300.0);
    assert!(sim.app.cx.project.auto_mull_members(0, c).is_empty());
    // One undo step per placement.
    assert_eq!(sim.undo().as_deref(), Some("Place Window"));
    assert_eq!(sim.undo().as_deref(), Some("Place Window"));
    assert_eq!(sim.app.cx.project.auto_mull_members(0, a), Vec::<Id>::new());
    assert_eq!(openings(&sim).len(), 1);
}

#[test]
fn the_minimum_separation_of_the_window_defaults_governs_placement_and_mulling() {
    let mut sim = house();
    sim.app.cx.defaults.window.min_separation = 6.0;
    let a = window_at(&mut sim, 100.0);
    let b = window_at(&mut sim, 140.0);
    let (wa, wb) = (get(&sim, a), get(&sim, b));
    assert!((wb.start_offset() - wa.end_offset() - 6.0).abs() < 1e-9);
    // 6 in is inside the two casings (7 1/2 in): mulled, sharing a 6 in casing.
    assert_eq!(
        plan_core::openings::mull::shared_casing_width(&wa, &wb),
        Some(6.0)
    );
    // With 10 in apart the casings no longer touch.
    let mut sim = house();
    sim.app.cx.defaults.window.min_separation = 10.0;
    let a = window_at(&mut sim, 100.0);
    let b = window_at(&mut sim, 145.0);
    let (wa, wb) = (get(&sim, a), get(&sim, b));
    assert!((wb.start_offset() - wa.end_offset() - 10.0).abs() < 1e-9);
    assert!(!plan_core::openings::mull::auto_mulled(&wa, &wb));
}

#[test]
fn an_opening_stops_where_its_casing_meets_an_intersecting_wall() {
    let mut sim = house();
    let a = window_at(&mut sim, 60.0);
    // Pull the start jamb to the corner: it stops short by the casing.
    let before = get(&sim, a).start_offset();
    sim.app.cx.project.resize_opening(0, a, Jamb::Start, -100.0);
    let stopped = get(&sim, a).start_offset();
    assert!(stopped < before);
    // With Ignore Casing for Opening Resize it goes further, to the wall.
    sim.app.cx.defaults.window.ignore_casing = true;
    crate::editor::opening_edit::sync_rules(&mut sim.app.cx);
    sim.app.cx.project.resize_opening(0, a, Jamb::Start, -100.0);
    let free = get(&sim, a).start_offset();
    assert!(free < stopped - 3.0, "{free} vs {stopped}");
}

// ----- defaults per type, dynamic defaults, Set as Default -----

#[test]
fn set_as_default_moves_the_openings_that_use_the_default() {
    let mut sim = house();
    let a = window_at(&mut sim, 100.0);
    let b = window_at(&mut sim, 300.0);
    // Both start on Use Default in every group.
    for id in [a, b] {
        let d = get(&sim, id).extras.spec.dynamic;
        assert!(
            d.casing && d.lintel && d.sash && d.frame && d.window_type,
            "{d:?}"
        );
    }
    // Give `a` a lintel in its Specification, as the Lintel tab does.
    assert!(sim.open_spec(ObjectRef::Opening(a)));
    {
        let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() else {
            panic!("no opening dialog");
        };
        let l = &mut d.draft_mut().extras.spec.lintel;
        l.exterior = true;
        l.height = 5.0;
    }
    sim.ok();
    let wa = get(&sim, a);
    assert!(wa.extras.spec.lintel.exterior);
    // The edited group stopped following; the others did not.
    assert!(!wa.extras.spec.dynamic.lintel);
    assert!(wa.extras.spec.dynamic.casing && wa.extras.spec.dynamic.sash);
    assert!(!get(&sim, b).extras.spec.lintel.exterior);
    // Set as Default copies `a` into the main window default; `b` follows.
    select(&mut sim, &[a]);
    sim.app.cx.run_custom(SET_DEFAULT);
    assert_eq!(label(&sim).as_deref(), Some("Set as Default"));
    let key = DefaultKey::main_window();
    let def = sim
        .app
        .cx
        .defaults
        .opening_variants
        .type_default(key)
        .expect("the default is stored");
    assert!(def.template.extras.spec.lintel.exterior);
    assert!(get(&sim, b).extras.spec.lintel.exterior, "b follows");
    assert_eq!(get(&sim, b).extras.spec.lintel.height, 5.0);
    // `a` uses the default from now on.
    assert!(get(&sim, a).extras.spec.dynamic.lintel);
    // One undo step puts the windows back.
    assert_eq!(sim.undo().as_deref(), Some("Set as Default"));
    assert!(!get(&sim, b).extras.spec.lintel.exterior);
    // A window placed after the change starts from the default.
    let c = window_at(&mut sim, 400.0);
    assert!(get(&sim, c).extras.spec.lintel.exterior);
    assert!(get(&sim, c).extras.spec.dynamic.lintel);
}

#[test]
fn a_door_type_has_its_own_default_and_interior_and_exterior_are_apart() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(150.0, 0.5);
    let id = openings(&sim)[0].id;
    // In an exterior wall: the Exterior Hinged Door default.
    let key = DefaultKey::of(&get(&sim, id), WallKind::Exterior);
    assert_eq!(key.name(), "Exterior Hinged Door");
    select(&mut sim, &[id]);
    {
        let o = sim.app.cx.project.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap();
        o.width = 42.0;
        o.extras.spec.hardware.hinges = 4;
    }
    sim.app.cx.run_custom(SET_DEFAULT);
    let v = &sim.app.cx.defaults.opening_variants;
    assert_eq!(
        v.type_default(key)
            .unwrap()
            .template
            .extras
            .spec
            .hardware
            .hinges,
        4
    );
    assert!(v
        .type_default(DefaultKey::new(
            OpeningKind::Door,
            OpeningStyle::Hinged,
            false
        ))
        .is_none());
    // A pocket door is its own default.
    let pocket = DefaultKey::new(OpeningKind::Door, OpeningStyle::Pocket, false);
    assert!(v.type_default(pocket).is_none());
    assert_eq!(pocket.name(), "Pocket Door");
}

// ----- Make Mulled Unit, components, levels, Caution -----

#[test]
fn a_mulled_unit_is_blocked_picked_by_component_and_exploded() {
    let mut sim = house();
    let wall = top_wall(&sim);
    let a = window_at(&mut sim, 100.0);
    let b = window_at(&mut sim, 135.0);
    // A transom-sized window over `a` on Window Level 1.
    let over = get(&sim, a).center_offset;
    let top = push_window(&mut sim, wall, over, 100.0, 20.0);
    sim.app.cx.project.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == top)
        .unwrap()
        .extras
        .spec
        .level = 1;
    // Block the pair into a unit: nothing moves.
    let (sa, sb) = (get(&sim, a).start_offset(), get(&sim, b).start_offset());
    select(&mut sim, &[a, b]);
    sim.app.cx.run_custom(MULL);
    assert_eq!(label(&sim).as_deref(), Some("Make Mulled Unit"));
    assert_eq!(get(&sim, a).start_offset(), sa);
    assert_eq!(get(&sim, b).start_offset(), sb);
    let group = get(&sim, a).mull_group.expect("a is in a unit");
    assert_eq!(get(&sim, b).mull_group, Some(group));
    let spec = sim.app.cx.project.mulled_spec(0, a).unwrap().clone();
    assert!(!spec.treat_as_door);
    // The hole of the unit is the box around both components.
    let hole = sim.app.cx.project.unit_hole(0, a).unwrap();
    assert_eq!((hole.s0, hole.s1), (sa, get(&sim, b).end_offset()));
    // One label for the whole unit: the size of the unit.
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let labels = opening_labels(&sim.app.cx);
    let for_unit: Vec<_> = labels
        .iter()
        .filter(|l| l.opening == a || l.opening == b)
        .collect();
    assert_eq!(for_unit.len(), 1, "{labels:?}");
    // Select Next Object walks the stack under the selection: level 0 first.
    select(&mut sim, &[a]);
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let wall_ref = get(&sim, a);
    let order = plan_core::openings::mull::pick_order(
        &sim.app.cx.floor().openings,
        wall_ref.wall_id,
        wall_ref.center_offset,
    );
    assert_eq!(order, vec![a, top]);
    sim.app.cx.run_custom(SELECT_NEXT);
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Opening(top)));
    sim.app.cx.run_custom(SELECT_NEXT);
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Opening(a)));
    // Moving the unit moves both.
    assert!(sim
        .app
        .cx
        .project
        .slide_opening(0, a, get(&sim, a).center_offset + 30.0));
    assert_eq!(get(&sim, b).start_offset(), sb + 30.0);
    // Explode: two windows again, one undo step.
    select(&mut sim, &[a]);
    sim.app.cx.run_custom(UNMULL);
    assert_eq!(label(&sim).as_deref(), Some("Explode Mulled Unit"));
    assert!(get(&sim, a).mull_group.is_none() && get(&sim, b).mull_group.is_none());
    assert_eq!(sim.undo().as_deref(), Some("Explode Mulled Unit"));
    assert!(get(&sim, a).mull_group.is_some());
    assert_eq!(sim.undo().as_deref(), Some("Make Mulled Unit"));
}

#[test]
fn a_window_level_other_than_zero_draws_light_grey() {
    let mut sim = house();
    let a = window_at(&mut sim, 100.0);
    let grey = egui::Color32::from_gray(170);
    let count = |sim: &mut Sim| {
        sim.plan_shapes()
            .iter()
            .filter(|s| shape_colors(s).contains(&grey))
            .count()
    };
    sim.app.cx.refresh();
    let level_zero = count(&mut sim);
    sim.app.cx.project.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == a)
        .unwrap()
        .extras
        .spec
        .level = 1;
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert!(count(&mut sim) > level_zero);
}

#[test]
fn four_openings_in_one_place_raise_a_caution_and_delete_duplicate_thins_them() {
    let mut sim = house();
    let wall = top_wall(&sim);
    let ids: Vec<Id> = (0..4)
        .map(|_| push_window(&mut sim, wall, 200.0, 24.0, 60.0))
        .collect();
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert_eq!(sim.app.cx.project.stacked_clusters(0).len(), 1);
    // The Caution symbol is drawn over them.
    let yellow = egui::Color32::from_rgb(0xF2, 0xC1, 0x2E);
    let drawn = |sim: &mut Sim| {
        sim.plan_shapes()
            .iter()
            .any(|s| shape_colors(s).contains(&yellow))
    };
    assert!(drawn(&mut sim));
    select(&mut sim, &[ids[0]]);
    let labels: Vec<_> = crate::editor::opening_edit::edit_actions(&sim.app.cx)
        .into_iter()
        .map(|a| a.label)
        .collect();
    assert!(labels.contains(&"Delete Duplicate"), "{labels:?}");
    sim.app.cx.run_custom(DELETE_DUPLICATE);
    assert_eq!(label(&sim).as_deref(), Some("Delete Duplicate"));
    assert!(sim.app.cx.project.stacked_clusters(0).is_empty());
    assert_eq!(openings(&sim).len(), 3);
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert!(!drawn(&mut sim));
    assert_eq!(sim.undo().as_deref(), Some("Delete Duplicate"));
    assert_eq!(openings(&sim).len(), 4);
}

// ----- bay, box and bow windows -----

fn bay_tool(style: OpeningStyle) -> ToolId {
    OpeningVariant::window(style).tool_id()
}

#[test]
fn a_bay_window_is_a_wall_section_unit_with_a_hip_roof_a_foundation_and_dimensions() {
    let mut sim = house();
    sim.tool(bay_tool(OpeningStyle::BayWindow));
    sim.click(240.0, 0.5);
    let o = openings(&sim)[0].clone();
    assert_eq!(o.style, OpeningStyle::BayWindow);
    // 4 ft 2 in wide, 1 ft deep, sides at 45 degrees (manual p. 604).
    assert_eq!(o.width, 50.0);
    let bay = &o.extras.spec.bay;
    assert_eq!(
        (bay.angle_for(o.style), bay.depth_for(o.style)),
        (45.0, 12.0)
    );
    assert_eq!(label(&sim).as_deref(), Some("Place Window"));
    // A hip roof with two extra planes for the California ridge.
    assert_eq!(roof_plane_count(o.style, bay, &o.extras.spec.bay_roof), 5);
    // The plan symbol reaches the depth past the face.
    let wall = sim.app.cx.floor().walls[0].clone();
    let ext = plan_core::exterior_sign(&wall, &sim.app.cx.rooms);
    let sym = plan_symbol(&wall, &o, ext);
    assert!((sym.max_reach(&wall) - (wall.thickness * 0.5 + 12.0)).abs() < 1e-6);
    // 3D: the roof, and a foundation under it on the first floor.
    let scene = build_scene(&sim.app.cx.project);
    let has = |m: Material| {
        scene
            .meshes
            .iter()
            .any(|x| x.object_id == Some(o.id) && x.material == m)
    };
    assert!(has(Material::Roof) && has(Material::Concrete));
    // The depth handle's model side: set the depth, the unit follows.
    assert!(sim.app.cx.project.set_bay_depth(0, o.id, 20.0));
    assert_eq!(
        get(&sim, o.id)
            .extras
            .spec
            .bay
            .depth_for(OpeningStyle::BayWindow),
        20.0
    );
}

#[test]
fn a_bay_needs_30_inches_and_a_straight_wall() {
    let mut sim = house();
    // A 33 in wall outside the shell: 4 in of wall-end clearance leave 29 in.
    sim.app.cx.project.add_wall(
        0,
        Point::new(600.0, 100.0),
        Point::new(633.0, 100.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    sim.app.cx.refresh();
    sim.tool(bay_tool(OpeningStyle::BayWindow));
    sim.click(616.0, 100.0);
    assert!(openings(&sim).is_empty());
    assert!(sim.app.cx.status.contains("2'-6"), "{}", sim.app.cx.status);
    // A 46 in wall takes the unit at the width there is (42 in).
    sim.app.cx.project.add_wall(
        0,
        Point::new(700.0, 100.0),
        Point::new(746.0, 100.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    sim.app.cx.refresh();
    sim.click(723.0, 100.0);
    let v = openings(&sim);
    assert_eq!(v.len(), 1, "{}", sim.app.cx.status);
    assert!(
        v[0].width >= 30.0 && v[0].width <= 42.0 + 1e-9,
        "{}",
        v[0].width
    );
}

#[test]
fn the_bay_specification_edits_the_unit_and_a_bow_has_its_sections() {
    let mut sim = house();
    sim.tool(bay_tool(OpeningStyle::BowWindow));
    sim.click(240.0, 0.5);
    let id = openings(&sim)[0].id;
    assert_eq!(get(&sim, id).width, 70.0);
    let sections = |sim: &Sim| {
        let o = get(sim, id);
        plan_core::openings::bay::bay_shape(o.style, o.width, &o.extras.spec.bay)
            .sections
            .len()
    };
    assert_eq!(sections(&sim), 5);
    assert!(sim.open_spec(ObjectRef::Opening(id)));
    let ctx = egui::Context::default();
    {
        let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() else {
            panic!("no opening dialog");
        };
        assert!(d.draw_tab_for_test(&ctx, "General"));
        assert!(d.draw_tab_for_test(&ctx, "Options"));
        let b = &mut d.draft_mut().extras.spec.bay;
        b.segments = 7;
        b.raised_floor = Some(plan_core::openings::bay::RaisedFloor::default());
        b.roof.rectangular = true;
    }
    sim.ok();
    assert_eq!(sections(&sim), 7);
    let b = get(&sim, id).extras.spec.bay;
    assert!(b.raised_floor.is_some() && b.roof.rectangular);
    // A raised unit gets no foundation in 3D.
    let scene = build_scene(&sim.app.cx.project);
    assert!(!scene
        .meshes
        .iter()
        .any(|m| m.object_id == Some(id) && m.material == Material::Concrete));
    // The roof of a rectangular hip is three planes.
    let o = get(&sim, id);
    assert_eq!(
        roof_plane_count(o.style, &o.extras.spec.bay, &o.extras.spec.bay_roof),
        3
    );
}

#[test]
fn exploding_a_bay_window_makes_walls_and_windows_in_one_undo_step() {
    let mut sim = house();
    sim.tool(bay_tool(OpeningStyle::BoxWindow));
    sim.click(240.0, 0.5);
    let id = openings(&sim)[0].id;
    {
        let o = sim.app.cx.project.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap();
        o.extras.spec.bay = BayUnit::for_style(OpeningStyle::BoxWindow);
        o.extras.spec.bay.lowered_ceiling = Some(Default::default());
    }
    let (walls, wins, rooms) = (
        sim.app.cx.floor().walls.len(),
        openings(&sim).len(),
        sim.app.cx.floor().room_names.len(),
    );
    select(&mut sim, &[id]);
    sim.app.cx.run_custom(EXPLODE_BAY);
    assert_eq!(label(&sim).as_deref(), Some("Explode Bay/Bow Window"));
    assert_eq!(sim.app.cx.floor().walls.len(), walls + 3);
    // The unit is replaced by three windows and a pass-through in the wall.
    assert_eq!(openings(&sim).len(), wins - 1 + 3 + 1);
    assert!(get_opt(&sim, id).is_none());
    // A lowered ceiling made a room of its own.
    assert_eq!(sim.app.cx.floor().room_names.len(), rooms + 1);
    assert!(
        sim.app
            .cx
            .floor()
            .room_names
            .last()
            .unwrap()
            .options
            .raised_floor_bump_out
    );
    assert_eq!(sim.undo().as_deref(), Some("Explode Bay/Bow Window"));
    assert!(get_opt(&sim, id).is_some());
    assert_eq!(sim.app.cx.floor().walls.len(), walls);
    assert_eq!(sim.app.cx.floor().room_names.len(), rooms);
}

fn get_opt(sim: &Sim, id: Id) -> Option<Opening> {
    openings(sim).into_iter().find(|o| o.id == id)
}
