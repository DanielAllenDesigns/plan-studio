//! Scenario 44: decks, chimneys and fireplaces, split-level floors (CB-86,
//! CB-87, R-86). The Fireplace tools place each kind beside a wall, in a wall
//! (the wall is cut in 3D) and free standing; the specification opens from a
//! double-click and from the Edit toolbar; the chimney tops out by the 3-2-10
//! rule, cuts the roof and runs through the floors above as a chase. A deck
//! room has planking in the plan and in 3D, Build Framing > Deck makes the
//! joists, ledger, beams and posts, and the Room Specification keeps the Deck
//! tab. Rooms at different heights get a marked level change, a riser in 3D
//! and stairs from Add Steps.

use super::{draw_shell, Sim};
use crate::editor::fireplace_view::{self as fv, cmd};
use crate::editor::roof_view;
use crate::editor::{rooms_edit, stairs_view, ObjectRef};
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::toolbar::{self, Action};
use crate::tools::fireplace::FireplaceMode as M;
use crate::tools::roof::RoofMode;
use crate::tools::{KeyEvent, ToolId};
use plan_3d::{Material, Mesh, Scene};
use plan_core::deck::{DeckSpec, DECK_ROOM_TYPE};
use plan_core::fireplace::{
    chimney_poly, ChimneyTop, FireplaceKind, FIREPLACE_CATALOG_ID, FIREPLACE_LAYER,
};
use plan_core::geometry::{point_in_polygon, Point};
use plan_core::{Floor, Id, Project, RoomName, Wall, WallClass, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn scene(sim: &Sim) -> Scene {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
}

fn tris(scene: &Scene, material: Material) -> usize {
    scene
        .meshes
        .iter()
        .filter(|m| m.material == material)
        .map(Mesh::triangle_count)
        .sum()
}

fn tagged(scene: &Scene, id: Id) -> Vec<&Mesh> {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .collect()
}

fn max_y(ms: &[&Mesh]) -> f64 {
    ms.iter()
        .flat_map(|m| m.vertices.iter().map(|v| f64::from(v.position[1])))
        .fold(f64::MIN, f64::max)
}

/// The id of the one fireplace symbol on the active floor.
fn only_fireplace(sim: &Sim) -> Id {
    let ids: Vec<Id> = sim
        .app
        .cx
        .floor()
        .fireplace_symbols()
        .iter()
        .map(|(s, _)| s.id)
        .collect();
    assert_eq!(ids.len(), 1, "{ids:?}");
    ids[0]
}

fn place(sim: &mut Sim, mode: M, x: f64, y: f64) -> Id {
    sim.tool(ToolId::FireplaceVariant(mode));
    let r = sim.click(x, y);
    assert!(r.commit.is_some(), "{r:?}");
    sim.app.cx.selection.single().map_or(0, |o| match o {
        ObjectRef::Symbol(id) => id,
        _ => 0,
    })
}

// ----- fireplaces -----

#[test]
fn the_fireplace_tool_places_a_fireplace_against_the_wall_in_one_undo_step() {
    let mut sim = house();
    let id = place(&mut sim, M::Masonry, 240.0, 30.0);
    assert_eq!(only_fireplace(&sim), id);
    let (sym, fp) = fv::load(sim.app.cx.floor(), id).unwrap();
    assert_eq!(sym.catalog_id, FIREPLACE_CATALOG_ID);
    assert_eq!(sym.layer, FIREPLACE_LAYER);
    assert_eq!(fp.kind, FireplaceKind::Masonry);
    assert!(!fp.in_wall);
    // The bottom wall: the body's back is on its inside face and the front
    // faces the room (+y), give or take the hand-drawn tilt of the wall.
    assert!(sym.angle.abs() < 1.0, "{}", sym.angle);
    assert!(
        sym.position.y > 1.0 && sym.position.y < 6.0,
        "{:?}",
        sym.position
    );
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Symbol(id)));
    sim.undo();
    assert!(sim.app.cx.floor().symbols.is_empty());
    assert!(sim.app.cx.floor().fireplaces.is_empty());
    sim.redo();
    assert_eq!(only_fireplace(&sim), id);
}

#[test]
fn every_fireplace_tool_places_its_kind_and_the_flyout_lists_them() {
    let group = toolbar::build_menu()
        .into_iter()
        .flat_map(|g| g.flyouts)
        .find(|f| f.group == "Fireplace")
        .expect("a Fireplace submenu of Build");
    let names: Vec<&str> = group.entries.iter().map(|e| e.name).collect();
    assert_eq!(
        names,
        [
            "Fireplace",
            "Fireplace in Wall",
            "Prefab Fireplace",
            "Chimney"
        ]
    );
    for e in &group.entries {
        assert!(matches!(
            e.action,
            Action::SetTool(ToolId::FireplaceVariant(_))
        ));
    }
    for (mode, kind, in_wall) in [
        (M::Masonry, FireplaceKind::Masonry, false),
        (M::InWall, FireplaceKind::Masonry, true),
        (M::Prefab, FireplaceKind::Prefab, false),
        (M::Chimney, FireplaceKind::ChimneyOnly, false),
    ] {
        let mut sim = house();
        sim.tool(ToolId::FireplaceVariant(mode));
        assert_eq!(sim.app.tools.active().name(), mode.name());
        place(&mut sim, mode, 240.0, 30.0);
        let id = only_fireplace(&sim);
        let (_, fp) = fv::load(sim.app.cx.floor(), id).unwrap();
        assert_eq!(fp.kind, kind, "{mode:?}");
        assert_eq!(fp.in_wall, in_wall, "{mode:?}");
    }
}

#[test]
fn a_fireplace_in_the_wall_cuts_the_wall_in_3d() {
    let mut sim = house();
    let before = scene(&sim);
    let wall_tris: usize = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .map(|w| {
            tagged(&before, w.id)
                .iter()
                .map(|m| m.triangle_count())
                .sum::<usize>()
        })
        .sum();
    // Built into the bottom wall: the wall loses the stretch under the body.
    place(&mut sim, M::InWall, 240.0, 30.0);
    let id = only_fireplace(&sim);
    let (_, fp) = fv::load(sim.app.cx.floor(), id).unwrap();
    assert!(fp.in_wall);
    let after = scene(&sim);
    let cut_tris: usize = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .map(|w| {
            tagged(&after, w.id)
                .iter()
                .map(|m| m.triangle_count())
                .sum::<usize>()
        })
        .sum();
    assert_ne!(cut_tris, wall_tris, "the wall mesh changed");
    // And the same fireplace beside the wall leaves the wall alone.
    let (sym, mut fp) = fv::load(sim.app.cx.floor(), id).unwrap();
    fp.in_wall = false;
    assert!(fv::apply(sim.cx(), &sym, &fp));
    let plain = scene(&sim);
    let plain_tris: usize = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .map(|w| {
            tagged(&plain, w.id)
                .iter()
                .map(|m| m.triangle_count())
                .sum::<usize>()
        })
        .sum();
    assert_eq!(plain_tris, wall_tris);
}

#[test]
fn the_fireplace_has_a_firebox_hearth_mantel_and_chimney_in_3d_and_a_symbol_in_the_plan() {
    let mut sim = house();
    let plain_shapes = sim.plan_shapes().len();
    let id = place(&mut sim, M::Masonry, 240.0, 30.0);
    let s = scene(&sim);
    let parts = tagged(&s, id);
    let mats: Vec<Material> = parts.iter().map(|m| m.material).collect();
    for m in [Material::Brick, Material::Stone, Material::DoorPanel] {
        assert!(mats.contains(&m), "{m:?} in {mats:?}");
    }
    // The plan draws the hatch, firebox, hearth and chimney X over the outline.
    let shapes = sim.plan_shapes().len();
    assert!(shapes > plain_shapes + 20, "{shapes} vs {plain_shapes}");
    // Hiding the Fireplaces layer hides the symbol and its parts.
    sim.app
        .cx
        .project
        .layers
        .set_display(FIREPLACE_LAYER, false);
    let hidden = sim.plan_shapes().len();
    assert!(hidden < shapes);
}

#[test]
fn double_click_opens_the_specification_and_ok_is_one_undo_step() {
    let mut sim = house();
    let id = place(&mut sim, M::Masonry, 240.0, 30.0);
    let (sym, _) = fv::load(sim.app.cx.floor(), id).unwrap();
    // Inside the body, away from the walls' pick radius.
    let inside = Point::new(sym.position.x, sym.position.y + 12.0);
    let r = sim.double_click(inside.x, inside.y);
    assert!(r.consumed);
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    // Cancel changes nothing.
    sim.dialog_frame_key(Some(eframe::egui::Key::Escape));
    sim.dialog_frame(false);
    assert!(sim.app.cx.floor().fireplaces[0].name == "Fireplace");
    // Open again and press OK: one undo step, the record is kept.
    let steps = sim.app.cx.floor().fireplaces.len();
    sim.double_click(inside.x, inside.y);
    sim.ok();
    assert_eq!(sim.app.cx.floor().fireplaces.len(), steps);
    assert!(sim.undo().is_some_and(|l| l == "Fireplace Specification"));
}

#[test]
fn the_edit_toolbar_hands_the_specification_to_the_fireplace_tool_and_back() {
    let mut sim = house();
    let id = place(&mut sim, M::Masonry, 240.0, 30.0);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Symbol(id));
    let actions = sim.app.cx.extra_edit_actions();
    assert!(
        actions.iter().any(|a| a.label == "Fireplace Specification"),
        "{:?}",
        actions.iter().map(|a| a.label).collect::<Vec<_>>()
    );
    sim.action(Action::Custom(cmd::OPEN_SPEC));
    // The tool took over and shows the dialog.
    assert_eq!(sim.app.tools.active().name(), "Fireplace");
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.dialog_frame_key(Some(eframe::egui::Key::Escape));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    // After the dialog closes the Select tool is back.
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
}

#[test]
fn the_select_tool_moves_a_fireplace_and_deleting_it_takes_its_specification() {
    let mut sim = house();
    let id = place(&mut sim, M::Masonry, 240.0, 30.0);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Symbol(id));
    sim.app
        .cx
        .translate_extra(&[ObjectRef::Symbol(id)], Point::new(40.0, 0.0));
    let (sym, _) = fv::load(sim.app.cx.floor(), id).unwrap();
    assert!((sym.position.x - 280.0).abs() < 1.5, "{:?}", sym.position);
    sim.key(KeyEvent::key(eframe::egui::Key::Delete));
    assert!(sim.app.cx.floor().symbols.is_empty());
    assert!(sim.app.cx.floor().fireplaces.is_empty());
    sim.undo();
    assert_eq!(only_fireplace(&sim), id);
}

// ----- chimneys -----

fn roofed_house() -> Sim {
    let mut sim = house();
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    sim
}

fn roof_tris(sim: &Sim) -> usize {
    let ids: Vec<Id> = roof_view::load(sim.app.cx.floor())
        .planes
        .iter()
        .map(|p| p.id)
        .collect();
    let s = scene(sim);
    ids.iter()
        .map(|id| {
            tagged(&s, *id)
                .iter()
                .map(|m| m.triangle_count())
                .sum::<usize>()
        })
        .sum()
}

#[test]
fn a_chimney_tops_out_three_feet_over_the_roof_and_cuts_a_hole_in_it() {
    let mut sim = roofed_house();
    let plain = roof_tris(&sim);
    assert!(plain > 0);
    let id = place(&mut sim, M::Masonry, 240.0, 150.0);
    let s = scene(&sim);
    let roof_surface = plan_3d::fireplace::chimney_tops(&sim.app.cx.project);
    assert_eq!(roof_surface.len(), 1);
    let top = roof_surface[0].2;
    // Higher than the plate (109") plus the 3 ft rule, and lower than the
    // ridge of a 480 x 360 hip roof plus the rule.
    assert!(top > 109.0 + 36.0, "{top}");
    // The meshes of the chimney reach the top and the flue above.
    let parts = tagged(&s, id);
    let tallest = max_y(&parts);
    assert!(tallest >= top && tallest < top + 24.0, "{tallest} vs {top}");
    // The roof has a hole where the chimney passes: more triangles, and none
    // inside the shaft.
    assert!(roof_tris(&sim) > plain, "{} vs {plain}", roof_tris(&sim));
    let shaft = {
        let (sym, fp) = fv::load(sim.app.cx.floor(), id).unwrap();
        chimney_poly(&fp, &sym)
    };
    let ids: Vec<Id> = roof_view::load(sim.app.cx.floor())
        .planes
        .iter()
        .map(|p| p.id)
        .collect();
    for pid in ids {
        for m in tagged(&s, pid) {
            for t in m.indices.chunks(3) {
                // The top surface of the roof only (the faces that look up the
                // steepest): the walls of the hole and the underside lean with
                // the slope.
                let up = m
                    .vertices
                    .iter()
                    .map(|v| v.normal[1])
                    .fold(f32::MIN, f32::max);
                if m.vertices[t[0] as usize].normal[1] < up - 1e-3 {
                    continue;
                }
                let corners: Vec<Point> = t
                    .iter()
                    .map(|i| {
                        let p = m.vertices[*i as usize].position;
                        Point::new(f64::from(p[0]), -f64::from(p[2]))
                    })
                    .collect();
                // Triangles with no area (the bridges that join a hole to
                // the outline) cover nothing.
                if plan_core::geometry::polygon_area(&corners).abs() < 1e-3 {
                    continue;
                }
                let c = corners
                    .iter()
                    .fold(Point::ZERO, |a, p| a + *p * (1.0 / 3.0));
                assert!(
                    !point_in_polygon(c, &shaft) || shaft_edge_distance(&shaft, c) < 0.01,
                    "a roof triangle inside the chimney at {c:?}: plane {pid}"
                );
            }
        }
    }
}

fn shaft_edge_distance(poly: &[Point], p: Point) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| plan_core::geometry::dist_to_segment(p, poly[i], poly[(i + 1) % n]))
        .fold(f64::MAX, f64::min)
}

#[test]
fn a_fixed_height_chimney_ends_where_it_is_told() {
    let mut sim = roofed_house();
    let id = place(&mut sim, M::Masonry, 240.0, 150.0);
    let (sym, mut fp) = fv::load(sim.app.cx.floor(), id).unwrap();
    fp.chimney.top = ChimneyTop::Height(300.0);
    assert!(fv::apply(sim.cx(), &sym, &fp));
    let tops = plan_3d::fireplace::chimney_tops(&sim.app.cx.project);
    assert_eq!(tops[0].2, 300.0);
}

#[test]
fn the_chimney_chase_runs_up_through_the_floors_above_and_cuts_their_platforms() {
    let mut sim = house();
    // A second floor with the same shell.
    let mut upper = Floor::new("2nd Floor", 120.0);
    for w in sim.app.cx.floor().walls.clone() {
        let mut copy = w.clone();
        copy.id = sim.app.cx.project.alloc_id();
        upper.walls.push(copy);
    }
    sim.app.cx.project.floors.push(upper);
    sim.app.cx.refresh();
    let id = place(&mut sim, M::Masonry, 240.0, 150.0);
    assert_eq!(
        plan_core::fireplace::chases_on_floor(&sim.app.cx.project, 1).len(),
        1
    );
    let with_chase = scene(&sim);
    let (sym, mut fp) = fv::load(sim.app.cx.floor(), id).unwrap();
    fp.chimney.chase_through_floors = false;
    assert!(fv::apply(sim.cx(), &sym, &fp));
    let without = scene(&sim);
    // The chase cuts holes in the upper floor's platforms: its floor and
    // ceiling slabs have more triangles than the plain ones.
    assert!(
        tris(&with_chase, Material::Floor) > tris(&without, Material::Floor),
        "{} vs {}",
        tris(&with_chase, Material::Floor),
        tris(&without, Material::Floor)
    );
    // On the upper floor the plan draws the chase.
    sim.app.cx.floor = 1;
    sim.app.cx.refresh();
    let with_shapes = sim.plan_shapes().len();
    sim.app.cx.floor = 0;
    let (sym, mut fp) = fv::load(sim.app.cx.floor(), id).unwrap();
    fp.chimney.chase_through_floors = true;
    assert!(fv::apply(sim.cx(), &sym, &fp));
    sim.app.cx.floor = 1;
    sim.app.cx.refresh();
    assert!(sim.plan_shapes().len() > with_shapes);
}

#[test]
fn a_chimney_on_its_own_is_one_shaft() {
    let mut sim = house();
    place(&mut sim, M::Chimney, 240.0, 150.0);
    let id = only_fireplace(&sim);
    let (sym, fp) = fv::load(sim.app.cx.floor(), id).unwrap();
    assert_eq!(fp.kind, FireplaceKind::ChimneyOnly);
    assert_eq!((sym.width, sym.depth), (48.0, 24.0));
    let s = scene(&sim);
    let mats: Vec<Material> = tagged(&s, id).iter().map(|m| m.material).collect();
    assert!(!mats.contains(&Material::DoorPanel) && !mats.contains(&Material::Stone));
}

// ----- decks -----

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

#[test]
fn a_deck_room_has_planking_in_3d_and_in_the_plan() {
    let mut sim = deck_sim();
    assert_eq!(sim.app.cx.rooms.len(), 1);
    let s = scene(&sim);
    let boards = tris(&s, Material::Floor);
    assert!(boards > 100, "{boards}");
    // Board lines in the plan.
    let with = sim.plan_shapes().len();
    sim.app.cx.floor_mut().room_names[0]
        .deck
        .as_mut()
        .unwrap()
        .planking
        .enabled = false;
    let without = sim.plan_shapes().len();
    assert!(with > without + 10, "{with} vs {without}");
    // Without boards the room keeps its platform slab.
    assert!(tris(&scene(&sim), Material::Floor) > 0);
    assert!(tris(&scene(&sim), Material::Floor) < boards);
}

#[test]
fn board_width_gap_direction_and_a_picture_frame_border_change_the_boards() {
    let mut sim = deck_sim();
    let base = tris(&scene(&sim), Material::Floor);
    let set = |sim: &mut Sim, f: &dyn Fn(&mut DeckSpec)| {
        f(sim.app.cx.floor_mut().room_names[0].deck.as_mut().unwrap());
    };
    set(&mut sim, &|d| d.planking.board_width = 3.5);
    let narrow = tris(&scene(&sim), Material::Floor);
    assert!(narrow > base);
    set(&mut sim, &|d| {
        d.planking.board_width = 5.5;
        d.planking.border = true;
    });
    let framed = tris(&scene(&sim), Material::Floor);
    assert!(framed != base);
    set(&mut sim, &|d| {
        d.planking.border = false;
        d.planking.angle = 90.0;
    });
    let turned = tris(&scene(&sim), Material::Floor);
    assert!(turned != base);
}

#[test]
fn build_deck_framing_makes_the_members_and_replaces_the_skirt() {
    let mut sim = deck_sim();
    let before = scene(&sim);
    let skirt = tris(&before, Material::Framing);
    assert!(skirt > 0, "the unframed deck has a skirt");
    sim.action(Action::Custom(cmd::BUILD_DECK));
    let members = crate::editor::framing_view::manual_members(sim.app.cx.floor());
    assert!(members.len() >= 19, "{}", members.len());
    assert!(members.iter().any(|m| m.label.contains("ledger")));
    assert!(members.iter().any(|m| m.label.contains("beam")));
    assert!(members.iter().any(|m| m.label.contains("post")));
    assert!(sim.app.cx.status.starts_with("Built deck framing"));
    let after = scene(&sim);
    // Joists, rims, ledger, beam and posts mesh as framing boxes.
    assert!(tris(&after, Material::Framing) > skirt);
    // One undo step takes it all back.
    sim.undo();
    assert!(crate::editor::framing_view::manual_members(sim.app.cx.floor()).is_empty());
    assert_eq!(tris(&scene(&sim), Material::Framing), skirt);
    sim.redo();
    // Delete Deck Framing.
    sim.action(Action::Custom(cmd::CLEAR_DECK));
    assert!(crate::editor::framing_view::manual_members(sim.app.cx.floor()).is_empty());
}

#[test]
fn steps_at_level_changes_are_in_the_floor_menu_and_deck_framing_in_the_framing_menu() {
    let all: Vec<&'static str> = toolbar::build_menu()
        .into_iter()
        .flat_map(|g| g.flyouts)
        .flat_map(|f| f.entries.into_iter().map(|e| e.name))
        .collect();
    assert!(all.contains(&"Add Steps at Level Changes"));
    // Build > Framing > Build Deck Framing runs the same command as the Edit
    // toolbar button of a selected deck room.
    let mut sim = deck_sim();
    sim.app.cx.selection.set(ObjectRef::Room(0));
    let ids: Vec<&str> = sim
        .app
        .cx
        .extra_edit_actions()
        .iter()
        .filter_map(|a| match a.kind {
            crate::editor::EditActionKind::Custom { id, .. } => Some(id),
            _ => None,
        })
        .collect();
    assert!(ids.contains(&cmd::BUILD_DECK) && ids.contains(&cmd::CLEAR_DECK));
}

#[test]
fn stairs_to_grade_are_built_with_the_framing_and_rebuilt_in_place() {
    let mut sim = deck_sim();
    {
        let outline = crate::editor::fireplace_view::deck::decks(&sim.app.cx)[0]
            .outline
            .clone();
        let right = (0..outline.len())
            .find(|&i| {
                let (p, q) = (outline[i], outline[(i + 1) % outline.len()]);
                p.x > 150.0 && q.x > 150.0
            })
            .unwrap();
        let spec = sim.app.cx.floor_mut().room_names[0].deck.as_mut().unwrap();
        spec.stairs.to_grade = true;
        spec.stairs.edge = right;
    }
    sim.action(Action::Custom(cmd::BUILD_DECK));
    assert_eq!(stairs_view::load(sim.app.cx.floor()).len(), 1);
    sim.action(Action::Custom(cmd::BUILD_DECK));
    assert_eq!(stairs_view::load(sim.app.cx.floor()).len(), 1);
}

#[test]
fn the_room_specification_keeps_the_deck_tab() {
    let mut sim = deck_sim();
    // The dialog opens with the Deck Specification of the room, and OK writes
    // what the Deck tab changed.
    let init = rooms_edit::room_dialog_init(sim.cx(), 0).unwrap();
    let mut dialog = crate::dialogs::room::RoomDialog::new(init);
    assert!(dialog.room_name().deck.is_some());
    {
        let spec = dialog.room_name_mut().deck.as_mut().unwrap();
        spec.planking.board_width = 3.5;
        spec.framing.joist_spacing = 24.0;
    }
    assert!(rooms_edit::apply_room_spec(
        sim.cx(),
        0,
        dialog.room_name(),
        dialog.extras()
    ));
    let d = sim.app.cx.floor().room_names[0].deck.clone().unwrap();
    assert_eq!(d.planking.board_width, 3.5);
    assert_eq!(d.framing.joist_spacing, 24.0);
    // A room that stops being a deck (another room type) loses the
    // specification, and the draft keeps no stale one.
    dialog.set_room_type("Bedroom");
    assert!(dialog.room_name().deck.is_none());
    assert!(rooms_edit::apply_room_spec(
        sim.cx(),
        0,
        dialog.room_name(),
        dialog.extras()
    ));
    assert!(sim.app.cx.floor().room_names[0].deck.is_none());
}

// ----- split level -----

fn split_sim() -> Sim {
    let mut sim = Sim::new();
    let corners = [(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)];
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        let mut w = Wall::new(
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            4.5,
            109.0,
            WallKind::Exterior,
        );
        w.id = sim.app.cx.project.alloc_id();
        sim.app.cx.floor_mut().walls.push(w);
    }
    let mut mid = Wall::new(
        Point::new(120.0, 0.0),
        Point::new(120.0, 120.0),
        4.5,
        109.0,
        WallKind::Interior,
    );
    mid.id = sim.app.cx.project.alloc_id();
    mid.class = WallClass::RoomDivider;
    mid.flags.room_divider = true;
    sim.app.cx.floor_mut().walls.push(mid);
    sim.app
        .cx
        .floor_mut()
        .room_names
        .push(RoomName::new(Point::new(60.0, 60.0), "Low", "Living"));
    let mut high = RoomName::new(Point::new(180.0, 60.0), "High", "Living");
    high.floor_height_offset = 24.0;
    sim.app.cx.floor_mut().room_names.push(high);
    sim.app.cx.refresh();
    sim
}

#[test]
fn rooms_at_different_heights_get_a_marked_level_change_and_a_riser() {
    let mut sim = split_sim();
    let changes = crate::editor::fireplace_view::deck::level_changes(&sim.app.cx);
    assert_eq!(changes.len(), 1);
    assert!((changes[0].rise() - 24.0).abs() < 1e-9);
    // The plan marks the step.
    let marked = sim.plan_shapes().len();
    sim.app.cx.floor_mut().room_names[1].floor_height_offset = 0.0;
    let flat = sim.plan_shapes().len();
    assert!(marked > flat + 3, "{marked} vs {flat}");
    sim.app.cx.floor_mut().room_names[1].floor_height_offset = 24.0;
    // The divider leaves a riser mesh in 3D.
    let s = scene(&sim);
    let before = tris(&s, Material::Floor);
    sim.app.cx.floor_mut().room_names[1].floor_height_offset = 0.0;
    let level = tris(&scene(&sim), Material::Floor);
    assert!(before > level, "{before} vs {level}");
}

#[test]
fn add_steps_puts_a_stair_between_the_levels_in_one_undo_step() {
    let mut sim = split_sim();
    sim.app.cx.selection.set(ObjectRef::Room(0));
    let actions = sim.app.cx.extra_edit_actions();
    assert!(actions
        .iter()
        .any(|a| a.label == "Add Steps at Level Change"));
    sim.action(Action::Custom(cmd::ADD_STEPS));
    let stairs = stairs_view::load(sim.app.cx.floor());
    assert_eq!(stairs.len(), 1);
    assert!((stairs[0].stair.params.total_rise - 24.0).abs() < 1e-9);
    // The stair meshes appear in the 3D scene.
    let s = scene(&sim);
    assert!(s.meshes.iter().any(|m| m.material == Material::Framing));
    sim.undo();
    assert!(stairs_view::load(sim.app.cx.floor()).is_empty());
    // A flat floor has nothing to add steps to.
    sim.app.cx.floor_mut().room_names[1].floor_height_offset = 0.0;
    sim.app.cx.refresh();
    sim.action(Action::Custom(cmd::ADD_STEPS));
    assert!(sim.app.cx.status.contains("no level change"));
}

#[test]
fn the_plan_round_trips_with_a_fireplace_a_deck_and_a_split_level() {
    let mut sim = deck_sim();
    place(&mut sim, M::Prefab, 96.0, 70.0);
    sim.action(Action::Custom(cmd::BUILD_DECK));
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.floors[0].fireplaces.len(), 1);
    assert!(
        back.floors[0].room_names[0]
            .deck
            .as_ref()
            .unwrap()
            .framing
            .built
    );
}
