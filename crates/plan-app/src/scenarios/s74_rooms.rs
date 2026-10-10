//! Scenario 74: room functions, the Exterior Room, Living Area labels and the
//! room Edit toolbar (Round 16, brief 15; R-98..R-115, R-145, R-146). Court
//! and Balcony rooms, an Open Below stairwell, the Exterior Room picked by a
//! click outside a wall and by Tab, its specification and its edge grips, a
//! Living Area label per structure on a two-structure plan, the 48 inch rule,
//! the label macros, the room polylines, Calculate Materials in Room, Create
//! Schedule from Room, Create Room Elevation Views, Auto Room Dimensions and
//! Turn Ceiling Off/On. Every action is one undo step.

use super::Sim;
use crate::dialogs::exterior_room::ExteriorRoomDialog;
use crate::dialogs::materials_list as ml;
use crate::dialogs::room::RoomDialog;
use crate::editor::rooms_edit::{self, ExteriorInit};
use crate::editor::{EditorContext, ObjectRef};
use crate::toolbar::Action;
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::{self, Key};
use plan_core::geometry::{polygon_area, Point};
use plan_core::living::{self, LivingTo};
use plan_core::schedules::{ScheduleKind, ScheduleLayer};
use plan_core::{CadItem, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn ring(cx: &mut EditorContext, x0: f64, y0: f64, x1: f64, y1: f64) {
    let c = [
        Point::new(x0, y0),
        Point::new(x1, y0),
        Point::new(x1, y1),
        Point::new(x0, y1),
    ];
    for i in 0..4 {
        cx.project
            .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 109.125, WallKind::Exterior);
    }
}

/// A 40' x 30' house of one room.
fn house() -> Sim {
    let mut sim = Sim::new();
    ring(sim.cx(), 0.0, 0.0, W, H);
    sim.tool(ToolId::Select);
    sim.cx().mark_dirty();
    sim.cx().refresh();
    assert_eq!(sim.app.cx.rooms.len(), 1);
    sim
}

/// The house with a partition at x = 240: a west room and an east room.
fn split_house() -> Sim {
    let mut sim = house();
    sim.cx().project.add_wall(
        0,
        Point::new(240.0, 0.0),
        Point::new(240.0, H),
        4.5,
        109.125,
        WallKind::Interior,
    );
    sim.cx().mark_dirty();
    sim.cx().refresh();
    assert_eq!(sim.app.cx.rooms.len(), 2);
    sim
}

/// The split house and a detached garage 80 inches east of it.
fn two_buildings() -> Sim {
    let mut sim = split_house();
    ring(sim.cx(), 600.0, 0.0, 840.0, 200.0);
    sim.cx().mark_dirty();
    sim.cx().refresh();
    assert_eq!(sim.app.cx.rooms.len(), 3);
    sim
}

const WEST: Point = Point { x: 120.0, y: 180.0 };
const EAST: Point = Point { x: 360.0, y: 180.0 };
const GARAGE: Point = Point { x: 720.0, y: 100.0 };

fn edit_room(sim: &mut Sim, p: Point, f: impl FnOnce(&mut RoomDialog)) {
    sim.app.cx.refresh();
    let idx = rooms_edit::room_index_at(&sim.app.cx, p).expect("a room there");
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    let mut d = RoomDialog::new(init);
    f(&mut d);
    assert!(!d.has_error());
    let draft = d.room_name().clone();
    assert!(rooms_edit::apply_room_spec(
        &mut sim.app.cx,
        d.room_index(),
        &draft,
        d.extras()
    ));
    assert_eq!(sim.app.cx.undo_label(), Some("Room Specification"));
}

fn entry(sim: &Sim, p: Point) -> plan_core::RoomName {
    let cx = &sim.app.cx;
    let idx = rooms_edit::room_index_at(cx, p).expect("a room there");
    rooms_edit::name_entry(cx, &cx.rooms[idx])
        .cloned()
        .expect("a named room")
}

fn labels(sim: &Sim) -> Vec<String> {
    rooms_edit::living_labels(&sim.app.cx)
        .iter()
        .map(|a| a.text())
        .collect()
}

fn toolbar(sim: &Sim) -> Vec<&'static str> {
    sim.app
        .cx
        .extra_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect()
}

fn run(sim: &mut Sim, id: &'static str) {
    sim.action(Action::Custom(id));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
}

// ---------------------------------------------------------------- functions --

#[test]
fn court_and_balcony_rooms_take_the_exterior_properties() {
    let mut sim = split_house();
    // Both rooms are interior at first: one structure, living area for both.
    let before = labels(&sim);
    assert_eq!(before.len(), 1, "{before:?}");
    // Courtyard has the Court function, Balcony the Balcony function.
    edit_room(&mut sim, WEST, |d| d.set_room_type("Courtyard"));
    edit_room(&mut sim, EAST, |d| d.set_room_type("Balcony"));
    for p in [WEST, EAST] {
        let n = entry(&sim, p);
        assert!(!n.has_ceiling, "an exterior room has no ceiling");
        assert!(!n.flat_ceiling);
        assert!(!n.misc.as_ref().unwrap().roof_over, "no roof over it");
        assert!(!n.options.build_foundation_below, "no foundation under it");
        assert!(sim
            .app
            .cx
            .defaults
            .room_type(&n.room_type)
            .is_some_and(|t| !t.include_in_living_area && !t.conditioned));
    }
    // Neither is a living room, so the structure has no Living Area label.
    assert!(labels(&sim).is_empty());
    // The Balcony platform is decking on joists; the Court is a slab.
    let balcony = entry(&sim, EAST);
    assert_eq!(balcony.misc.as_ref().unwrap().floor_structure.len(), 2);
    assert!(balcony.deck.is_none(), "only a Deck has deck framing");
    // Each edit was one undo step.
    assert_eq!(sim.undo().as_deref(), Some("Room Specification"));
    assert_eq!(entry(&sim, WEST).room_type, "Courtyard");
    let cx = &sim.app.cx;
    let east = rooms_edit::room_index_at(cx, EAST).unwrap();
    assert!(rooms_edit::name_entry(cx, &cx.rooms[east]).is_none_or(|n| n.room_type != "Balcony"));
}

#[test]
fn an_open_below_stairwell_has_no_floor_and_is_conditioned_but_not_living() {
    let mut sim = split_house();
    edit_room(&mut sim, EAST, |d| d.set_room_type("Open Below"));
    let n = entry(&sim, EAST);
    assert!(!n.has_floor && n.has_ceiling);
    let t = sim.app.cx.defaults.room_type("Open Below").unwrap();
    assert!(t.conditioned && !t.include_in_living_area);
    // The living area is now the west room alone.
    assert_eq!(labels(&sim), vec!["Living Area: 618 sq ft".to_string()]);
    // A crawl space is Open Below as well: no floor, no function of its own.
    edit_room(&mut sim, EAST, |d| d.set_room_type("Crawl Space"));
    let c = entry(&sim, EAST);
    assert!(!c.has_floor);
    assert_eq!(
        plan_core::rooms::function_class("Open Below", "Crawl Space"),
        plan_core::rooms::FunctionClass::Hybrid
    );
    assert_eq!(sim.undo().as_deref(), Some("Room Specification"));
    assert!(!entry(&sim, EAST).room_type.is_empty());
}

#[test]
fn the_room_dialog_shows_the_function_and_the_room_information() {
    let mut sim = split_house();
    edit_room(&mut sim, WEST, |d| d.set_room_type("Garage"));
    let idx = rooms_edit::room_index_at(&sim.app.cx, WEST).unwrap();
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    assert!(init.centerline_area_sq_ft > 0.0);
    assert!(init.standard_area_sq_ft >= init.interior_area_sq_ft);
    assert!(
        !living::room_in_living_area(
            sim.app.cx.floor(),
            &sim.app.cx.rooms[idx],
            &sim.app.cx.defaults.rooms.room_types
        ),
        "a garage is not living area"
    );
    let ctx = egui::Context::default();
    let mut d = RoomDialog::new(init);
    // Draw every tab, the Structure tab with its cross section included.
    let n = 12;
    for tab in 0..n {
        let _ = ctx.run(egui::RawInput::default(), |c| {
            egui::CentralPanel::default().show(c, |ui| d.page_for_test(ui, tab));
        });
    }
}

// ------------------------------------------------------------ living area --

#[test]
fn a_living_area_label_per_structure_rounds_to_the_nearest_square_foot() {
    let mut sim = two_buildings();
    let l = labels(&sim);
    assert_eq!(l.len(), 2, "{l:?}");
    // The house: 486 x 366 in to the outside of the 6 in exterior walls,
    // 1,235.25 sq ft, shown as 1,235.
    let house = (486.0 * 366.0 / 144.0_f64).round() as i64;
    assert_eq!(house, 1235);
    assert!(
        l.contains(&format!(
            "Living Area: {} sq ft",
            living::group_thousands(house)
        )),
        "{l:?}"
    );
    // The garage: 246 x 206 in, to the nearest sq ft.
    let garage = (246.0 * 206.0 / 144.0_f64).round() as i64;
    assert!(l.contains(&format!("Living Area: {garage} sq ft")), "{l:?}");
    // Per-structure sums, not a plan total.
    let by_floor = rooms_edit::living_area_by_floor(&sim.app.cx);
    let total: f64 = by_floor.iter().map(|(_, a)| a).sum();
    assert!((total - house as f64 - garage as f64).abs() < 1e-9);
    // Make the garage detached garage: hybrid, out of the living area; its
    // structure then has no label.
    edit_room(&mut sim, GARAGE, |d| d.set_room_type("Garage"));
    assert_eq!(labels(&sim).len(), 1);
    // Include it by hand and the label is back (the per-room 3-way box).
    edit_room(&mut sim, GARAGE, |d| d.set_living(Some(true)));
    assert_eq!(labels(&sim).len(), 2);
    assert!(rooms_edit::living_area_report(&sim.app.cx).starts_with("Total living area"));
}

#[test]
fn a_rough_ceiling_under_48_inches_keeps_a_room_out_and_48_brings_it_in() {
    let mut sim = house();
    let set_rough = |sim: &mut Sim, r: f64| {
        edit_room(sim, Point::new(240.0, 180.0), |d| {
            d.room_name_mut().rough_ceiling = Some(r);
            d.room_name_mut().ceiling_height = Some(r - 1.0);
        });
    };
    set_rough(&mut sim, 47.9);
    assert!(labels(&sim).is_empty(), "47.9 in is out");
    set_rough(&mut sim, 48.0);
    assert_eq!(labels(&sim).len(), 1, "48 in counts");
    // An explicit Exclude beats any height; an explicit Include beats 47.9.
    edit_room(&mut sim, Point::new(240.0, 180.0), |d| {
        d.set_living(Some(false))
    });
    assert!(labels(&sim).is_empty());
    set_rough(&mut sim, 40.0);
    edit_room(&mut sim, Point::new(240.0, 180.0), |d| {
        d.set_living(Some(true))
    });
    assert_eq!(labels(&sim).len(), 1);
}

#[test]
fn the_basement_of_a_foundation_counts_at_48_inches() {
    let mut sim = house();
    let mut spec = rooms_edit::FoundationSpec::from_defaults(&sim.app.cx.defaults);
    spec.stem_height = 108.0;
    rooms_edit::build_foundation(sim.cx(), spec);
    sim.cx().refresh();
    let cx = &sim.app.cx;
    let basement = cx.project.floors[0].room_names[0].room_type.clone();
    assert_eq!(basement, "Basement");
    let t = cx.defaults.room_type("Basement").unwrap();
    assert!(t.include_in_living_area, "a basement is a living room");
    assert_eq!(t.function, "Standard", "Basement is not a function");
    let f = &cx.project.floors[0];
    let rooms = plan_core::detect_rooms(&f.walls, 0.5);
    assert!(living::room_in_living_area(
        f,
        &rooms[0],
        &cx.defaults.rooms.room_types
    ));
    assert!(living::rough_ceiling(f, f.room_names.first()) >= 48.0);
}

#[test]
fn label_macros_report_dimensions_interior_area_and_standard_area() {
    let mut sim = split_house();
    edit_room(&mut sim, WEST, |d| {
        d.extras_mut().label.template = "%dimensions%|%internal_area%|%standard_area%".into();
    });
    let cx = &sim.app.cx;
    let idx = rooms_edit::room_index_at(cx, WEST).unwrap();
    let text = rooms_edit::room_label_text(cx, &cx.rooms[idx]);
    let parts: Vec<&str> = text.split('|').collect();
    assert_eq!(parts.len(), 3, "{text}");
    assert!(parts[0].contains('x'), "{text}");
    // Interior: 234.75 x 354 in, standard: 243 x 366 in.
    let interior = (234.75 * 354.0_f64 / 144.0).round();
    let standard = (243.0 * 366.0_f64 / 144.0).round();
    assert_eq!(parts[1], format!("{interior} sq ft"), "{text}");
    assert_eq!(parts[2], format!("{standard} sq ft"), "{text}");
    assert!(rooms_edit::LABEL_MACROS
        .iter()
        .any(|(m, _)| *m == "%standard_area%"));
}

#[test]
fn living_area_to_the_main_layer_and_the_label_switch_come_from_general_plan_defaults() {
    let mut sim = house();
    assert_eq!(
        rooms_edit::living_basis(&sim.app.cx.defaults),
        LivingTo::OutsideSurface
    );
    assert!(rooms_edit::show_living_label(&sim.app.cx.defaults));
    let key = living::LIVING_TO_KEY;
    sim.cx().defaults.pages.insert(
        key.to_string(),
        plan_core::defaults::PageValue::Text("Outside of Main Layer".into()),
    );
    assert_eq!(
        rooms_edit::living_basis(&sim.app.cx.defaults),
        LivingTo::MainLayer
    );
    sim.cx().defaults.pages.insert(
        living::SHOW_LIVING_LABEL_KEY.to_string(),
        plan_core::defaults::PageValue::Bool(false),
    );
    assert!(!rooms_edit::show_living_label(&sim.app.cx.defaults));
    // Hidden: clicking where the label is picks nothing.
    let at = rooms_edit::living_labels(&sim.app.cx)[0].label_at;
    assert!(rooms_edit::exterior::exterior_label_at(&sim.app.cx, at).is_none());
    // The General Plan Defaults page lists both settings.
    let page = crate::dialogs::default_pages::spec("general").expect("the page");
    let to = page.field(living::LIVING_TO_KEY).expect("Living Area to");
    assert_eq!(to.label, "Living Area to");
    let show = page
        .field(living::SHOW_LIVING_LABEL_KEY)
        .expect("Show Living Area Label");
    assert_eq!(show.label, "Show Living Area Label");
}

// ------------------------------------------------------------ Exterior Room --

#[test]
fn a_click_outside_an_exterior_wall_selects_the_exterior_room() {
    let mut sim = house();
    assert_eq!(rooms_edit::selected_exterior(&sim.app.cx), None);
    // Inside the room: the room, not the Exterior Room.
    sim.click(240.0, 180.0);
    assert!(rooms_edit::selected_room(&sim.app.cx).is_some());
    assert_eq!(rooms_edit::selected_exterior(&sim.app.cx), None);
    // Just outside the south wall.
    sim.click(240.0, -9.0);
    assert_eq!(rooms_edit::selected_exterior(&sim.app.cx), Some(0));
    assert_eq!(rooms_edit::selected_room(&sim.app.cx), None);
    assert_eq!(sim.app.cx.status, "Exterior Room");
    // Far from the walls selects nothing.
    sim.click(240.0, -300.0);
    assert_eq!(rooms_edit::selected_exterior(&sim.app.cx), None);
    // The Living Area label under the house picks it too.
    let at = rooms_edit::living_labels(&sim.app.cx)[0].label_at;
    sim.click(at.x, at.y);
    assert_eq!(rooms_edit::selected_exterior(&sim.app.cx), Some(0));
    // Escape lets go.
    sim.esc();
    assert_eq!(rooms_edit::selected_exterior(&sim.app.cx), None);
}

#[test]
fn tab_reaches_the_exterior_room_after_the_wall_you_clicked() {
    let mut sim = house();
    sim.move_to(240.0, -2.0);
    sim.app.cx.cursor_world = Some(Point::new(240.0, -2.0));
    sim.key(KeyEvent::key(Key::Tab));
    assert!(matches!(
        sim.app.cx.selection.single(),
        Some(ObjectRef::Wall(_))
    ));
    sim.key(KeyEvent::key(Key::Tab));
    assert_eq!(rooms_edit::selected_exterior(&sim.app.cx), Some(0));
    assert!(sim.app.cx.selection.is_empty());
    // Round again: back to the wall.
    sim.key(KeyEvent::key(Key::Tab));
    assert!(matches!(
        sim.app.cx.selection.single(),
        Some(ObjectRef::Wall(_))
    ));
}

#[test]
fn the_exterior_room_specification_covers_the_exterior_walls_in_one_step() {
    let mut sim = two_buildings();
    sim.click(240.0, -9.0);
    let ext = rooms_edit::selected_exterior(&sim.app.cx).expect("selected");
    sim.double_click(240.0, -9.0);
    assert!(rooms_edit::take_exterior_dialog_request(&sim.app.cx).is_some());
    let init: ExteriorInit = rooms_edit::exterior_dialog_init(&sim.app.cx, ext).unwrap();
    assert_eq!(init.exterior_walls, 4, "the house, not the garage");
    assert_eq!(init.rooms, 2);
    let mut d = ExteriorRoomDialog::new(init);
    d.spec_mut().covering.covering = plan_core::rooms::WALL_SURFACES[0].to_string();
    d.spec_mut().surface_material = plan_core::rooms::WALL_SURFACES[0].to_string();
    assert!(!d.has_error());
    assert!(rooms_edit::apply_exterior_spec(
        &mut sim.app.cx,
        d.init().anchor,
        d.spec(),
        d.ceiling_height(),
        d.floor_thickness(),
    ));
    assert_eq!(sim.app.cx.undo_label(), Some("Exterior Room Specification"));
    let covered = |sim: &Sim| {
        sim.app.cx.project.floors[0]
            .walls
            .iter()
            .filter(|w| !w.spec.covering.exterior.covering.is_empty())
            .count()
    };
    assert_eq!(covered(&sim), 4);
    assert_eq!(sim.app.cx.project.floors[0].exterior_rooms.len(), 1);
    // One undo takes all of it back.
    sim.undo();
    assert_eq!(covered(&sim), 0);
    assert!(sim.app.cx.project.floors[0].exterior_rooms.is_empty());
}

#[test]
fn dragging_the_exterior_rooms_top_grip_sets_the_default_ceiling_height() {
    let mut sim = house();
    sim.click(240.0, -9.0);
    let s = rooms_edit::exterior::structures(&sim.app.cx).swap_remove(0);
    let top = rooms_edit::exterior::grips(&sim.app.cx, &s)[0].1;
    let before = sim.app.cx.floor().ceiling_height;
    let res = sim.drag((top.x, top.y), (top.x, top.y + 12.0));
    assert!(res.commit.is_some(), "the drag commits");
    assert!((sim.app.cx.floor().ceiling_height - (before + 12.0)).abs() < 1.0);
    // The walls that stood at the old ceiling height followed it.
    assert!(sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .all(|w| (w.height - sim.app.cx.floor().ceiling_height).abs() < 0.01));
    // One undo step.
    sim.undo();
    assert!((sim.app.cx.floor().ceiling_height - before).abs() < 1e-9);
    // Floor 1 cannot move its floor: the bottom edge has no grip.
    assert!(!rooms_edit::exterior::set_default_heights(
        sim.cx(),
        living::ExteriorEdge::Bottom,
        20.0
    ));
}

// -------------------------------------------------------- room Edit toolbar --

#[test]
fn a_selected_room_offers_the_room_edit_buttons() {
    let mut sim = split_house();
    sim.click(WEST.x, WEST.y);
    let l = toolbar(&sim);
    for want in [
        "Open Object",
        "Calculate Materials in Room",
        "Turn Ceiling Off",
        "Make Room Polyline",
        "Make Standard Area Polyline",
        "Expand Room Polyline",
        "Create Schedule from Room",
        "Create Room Elevation Views",
        "Auto Room Dimensions",
    ] {
        assert!(l.contains(&want), "{want} missing from {l:?}");
    }
    // Expand is dimmed: no invisible wall around this room.
    let expand = sim
        .app
        .cx
        .extra_edit_actions()
        .into_iter()
        .find(|a| a.label == "Expand Room Polyline")
        .unwrap();
    assert!(!expand.enabled);
    // The Exterior Room has its own buttons.
    sim.click(240.0, -9.0);
    let l = toolbar(&sim);
    assert!(l.contains(&"Make Living Area Polyline"), "{l:?}");
    assert!(l.contains(&"Make Room Polyline"));
    assert!(!l.contains(&"Turn Ceiling Off"));
}

#[test]
fn turn_ceiling_off_and_on_flips_the_flat_ceiling_one_step_each() {
    let mut sim = split_house();
    sim.click(WEST.x, WEST.y);
    assert!(toolbar(&sim).contains(&"Turn Ceiling Off"));
    run(&mut sim, rooms_edit::commands::id::CEILING_OFF);
    assert!(!entry(&sim, WEST).flat_ceiling);
    assert_eq!(sim.app.cx.undo_label(), Some("Turn Off Ceiling"));
    assert!(toolbar(&sim).contains(&"Turn Ceiling On"));
    run(&mut sim, rooms_edit::commands::id::CEILING_ON);
    assert!(entry(&sim, WEST).flat_ceiling);
    assert_eq!(sim.app.cx.undo_label(), Some("Turn On Ceiling"));
    sim.undo();
    assert!(!entry(&sim, WEST).flat_ceiling);
}

fn polylines(sim: &Sim) -> Vec<Vec<Point>> {
    sim.app
        .cx
        .floor()
        .cad
        .iter()
        .filter_map(|c| match &c.item {
            CadItem::Polyline {
                points,
                closed: true,
            } => Some(points.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn room_polylines_follow_the_surfaces_the_standard_area_and_the_living_area() {
    let mut sim = split_house();
    sim.click(WEST.x, WEST.y);
    let room = sim.app.cx.rooms[rooms_edit::selected_room(&sim.app.cx).unwrap()].clone();
    // Make Room Polyline: the inner surfaces, 237 x 357 in.
    let ids = rooms_edit::commands::make_room_polyline(sim.cx());
    assert_eq!(ids.len(), 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Make Room Polyline"));
    let p = polylines(&sim);
    assert!((polygon_area(&p[0]).abs() - room.interior_area_sq_in).abs() < 1e-6);
    // Make Standard Area Polyline: the outside of the exterior walls and the
    // center of the partition, 243 x 366 in.
    rooms_edit::commands::make_standard_area_polyline(sim.cx());
    let p = polylines(&sim);
    assert_eq!(p.len(), 2);
    assert!((polygon_area(&p[1]).abs() - 243.0 * 366.0).abs() < 1e-6);
    // The polylines are static: moving a wall leaves them as drawn.
    let before = polylines(&sim);
    let w = sim.wall_ids()[0];
    sim.app.cx.project.floors[0].wall_mut(w).unwrap().thickness = 8.0;
    sim.cx().mark_dirty();
    sim.cx().refresh();
    assert_eq!(polylines(&sim), before);
    // Make Living Area Polyline from the Exterior Room covers both rooms
    // (the bottom wall is 8 in thick now: click a little farther out).
    sim.click(240.0, -12.0);
    let n = polylines(&sim).len();
    let ids = rooms_edit::commands::make_living_area_polyline(sim.cx());
    assert_eq!(ids.len(), 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Make Living Area Polyline"));
    let rings = polylines(&sim);
    assert_eq!(rings.len(), n + 1);
    let area = polygon_area(rings.last().unwrap()).abs() / 144.0;
    let label = &rooms_edit::living_labels(&sim.app.cx)[0];
    assert!((area - label.area_sq_in / 144.0).abs() < 1.0, "{area}");
    // Make Room Polyline on the Exterior Room is the footprint.
    let ids = rooms_edit::commands::make_exterior_polyline(sim.cx());
    assert_eq!(ids.len(), 1);
    sim.undo();
    assert_eq!(polylines(&sim).len(), n + 1);
}

#[test]
fn expand_room_polyline_ignores_an_invisible_wall() {
    let mut sim = split_house();
    let w = sim.wall_ids().last().copied().unwrap();
    sim.app.cx.project.floors[0]
        .wall_mut(w)
        .unwrap()
        .flags
        .room_divider = true;
    sim.cx().mark_dirty();
    sim.cx().refresh();
    sim.click(WEST.x, WEST.y);
    let expand = sim
        .app
        .cx
        .extra_edit_actions()
        .into_iter()
        .find(|a| a.label == "Expand Room Polyline")
        .unwrap();
    assert!(expand.enabled);
    assert!(rooms_edit::commands::expand_room_polyline(sim.cx()));
    let e = rooms_edit::commands::expanded_room().expect("an expanded room");
    assert!((e.room.area_sq_in - W * H).abs() < 1e-6, "spans both rooms");
    // The polyline buttons now work on the expanded room.
    let ids = rooms_edit::commands::make_room_polyline(sim.cx());
    assert_eq!(ids.len(), 1);
    let p = polylines(&sim);
    assert!((polygon_area(&p[0]).abs() - e.room.interior_area_sq_in).abs() < 1e-6);
    // No undo step for the expansion itself.
    assert_eq!(sim.app.cx.undo_label(), Some("Make Room Polyline"));
    // Another selection ends it.
    sim.click(EAST.x, EAST.y);
    assert!(rooms_edit::commands::expanded_room().is_none());
}

#[test]
fn elevation_views_and_room_dimensions_come_from_the_selected_room() {
    let mut sim = split_house();
    sim.click(WEST.x, WEST.y);
    let ids = rooms_edit::commands::create_room_elevation_views(sim.cx());
    assert_eq!(ids.len(), 4, "one view of each wall");
    assert_eq!(sim.app.cx.project.cameras.len(), 4);
    assert_eq!(sim.app.cx.undo_label(), Some("Create Room Elevation Views"));
    // A second run finds the views right: no repeats, no second undo step.
    let again = rooms_edit::commands::create_room_elevation_views(sim.cx());
    assert_eq!(again, ids);
    assert_eq!(sim.app.cx.project.cameras.len(), 4);
    assert_eq!(sim.app.cx.undo_label(), Some("Create Room Elevation Views"));
    sim.undo();
    assert!(sim.app.cx.project.cameras.is_empty());
    // Auto Room Dimensions: one string per wall of the room.
    let n = rooms_edit::commands::auto_room_dimensions(sim.cx());
    assert_eq!(n, 4);
    assert_eq!(sim.app.cx.floor().dimensions.len(), 4);
    assert_eq!(sim.app.cx.undo_label(), Some("Auto Room Dimensions"));
    // Run again: the room's strings are replaced, not doubled.
    rooms_edit::commands::auto_room_dimensions(sim.cx());
    assert_eq!(sim.app.cx.floor().dimensions.len(), 4);
    sim.undo();
    assert_eq!(sim.app.cx.floor().dimensions.len(), 4);
    sim.undo();
    assert!(sim.app.cx.floor().dimensions.is_empty());
}

#[test]
fn calculate_materials_in_room_and_create_schedule_from_room() {
    let mut sim = split_house();
    // A window in the west room's wall so the schedule has a row.
    sim.tool(ToolId::Window);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Select);
    sim.cx().refresh();
    sim.click(WEST.x, WEST.y);
    // Calculate Materials in Room: the room's finishes and its window.
    run(&mut sim, ml::cmd::ROOM);
    let lines = ml::with_state(|st| st.shown.clone());
    assert!(lines.iter().any(|l| l.line.item.starts_with("Flooring")));
    assert!(lines.iter().any(|l| l.line.category == "Windows"));
    assert!(!lines.iter().any(|l| l.line.category == "Framing"));
    // Create Schedule from Room: a window schedule limited to the room.
    let before = ScheduleLayer::load(sim.app.cx.floor()).schedules.len();
    let id = rooms_edit::commands::create_schedule_from_room(
        sim.cx(),
        ScheduleKind::Window,
        Point::new(-300.0, 100.0),
    )
    .expect("a schedule");
    let layer = ScheduleLayer::load(sim.app.cx.floor());
    assert_eq!(layer.schedules.len(), before + 1);
    let s = layer.find(id).unwrap();
    assert_eq!(s.kind, ScheduleKind::Window);
    assert_eq!(s.rooms.len(), 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Create Schedule from Room"));
    sim.undo();
    assert_eq!(
        ScheduleLayer::load(sim.app.cx.floor()).schedules.len(),
        before
    );
    // The Edit button asks for the schedule type first.
    sim.click(WEST.x, WEST.y);
    run(&mut sim, rooms_edit::commands::id::SCHEDULE);
    assert!(crate::dialogs::schedule_spec::type_chooser_open());
}

#[test]
fn the_structure_panel_options_are_kept_with_the_room() {
    let mut sim = split_house();
    edit_room(&mut sim, WEST, |d| {
        let n = d.room_name_mut();
        n.options.supplies_floor_above = true;
        n.options.retain_framing = true;
        n.options.shelf_ceiling = true;
        n.options.framing_group = 2;
        n.flat_ceiling = false;
        n.roof_group = 3;
        n.options.layer = "CAD, Default".into();
    });
    let n = entry(&sim, WEST);
    assert!(n.options.supplies_floor_above && n.options.retain_framing && n.options.shelf_ceiling);
    assert_eq!(n.options.framing_group, 2);
    assert!(!n.flat_ceiling);
    assert_eq!(n.roof_group, 3);
    assert_eq!(n.options.layer_name(), "CAD, Default");
    // They round-trip through the file.
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    let m = &back.floors[0].room_names[0];
    assert!(m.options.supplies_floor_above && !m.flat_ceiling);
    // Undo takes the whole spec back.
    sim.undo();
    let cx = &sim.app.cx;
    let idx = rooms_edit::room_index_at(cx, WEST).unwrap();
    assert!(
        rooms_edit::name_entry(cx, &cx.rooms[idx]).is_none_or(|n| !n.options.supplies_floor_above)
    );
}
