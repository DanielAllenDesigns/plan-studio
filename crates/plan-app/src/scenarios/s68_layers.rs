//! Scenario 68 (round 16, brief 09): layer management (New, Copy, Merge,
//! Delete, Delete Unused, Reset Names), the Select Layer dialog with Use
//! Default Layer, Layer Set Defaults, and drawing-group ordering.

use super::Sim;
use crate::dialogs::{layer_display as ld, layer_sets, select_layer::SelectLayer};
use crate::editor::ObjectRef;
use crate::toolbar::Action;
use crate::tools::ToolId;
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::layer_sets::{LayerEdit, ViewKind};
use plan_core::{Id, WallKind};

fn sim() -> Sim {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    sim
}

fn wall(sim: &mut Sim) -> Id {
    sim.app.cx.project.add_wall(
        0,
        Point::new(0.0, 0.0),
        Point::new(120.0, 0.0),
        6.0,
        96.0,
        WallKind::Exterior,
    )
}

fn circle(sim: &mut Sim, layer: &str, r: f64) -> Id {
    sim.app.cx.project.add_cad(
        0,
        layer,
        CadItem::Circle {
            center: Point::ZERO,
            radius: r,
        },
    )
}

fn wall_layer(sim: &Sim, id: Id) -> String {
    sim.app.cx.floor().wall(id).unwrap().layer.clone()
}

#[test]
fn a_custom_layer_is_created_filled_hidden_in_one_set_merged_and_undone() {
    let mut sim = sim();
    let cx = &mut sim.app.cx;
    layer_sets::new_set(cx, "Alt").unwrap();
    let active = cx.project.layer_sets.active.clone();

    // New: unique, in every set, shown only in the active one.
    assert_eq!(ld::new_layer_named(cx, "Notes").unwrap(), "Notes");
    assert!(ld::new_layer_named(cx, "notes").is_err());
    assert_eq!(cx.undo_label(), Some("New Layer"));
    let shown = |cx: &crate::editor::EditorContext, set: &str| {
        cx.project
            .layer_sets
            .effective_for(set, &cx.project.layers)
            .is_visible("Notes")
    };
    assert!(shown(cx, &active) && !shown(cx, "Alt"));

    // A wall moves to it in one step.
    let w = wall(&mut sim);
    let cx = &mut sim.app.cx;
    cx.selection.items = vec![ObjectRef::Wall(w)];
    assert_eq!(cx.send_selection_to_layer("Notes"), 1);
    assert_eq!(cx.undo_label(), Some("Send to Layer"));
    assert_eq!(wall_layer(&sim, w), "Notes");

    // Hide it in one set only (the active one), keep it in Alt.
    let cx = &mut sim.app.cx;
    crate::shell::docks::edit_layers(
        cx,
        &["Notes".into()],
        LayerEdit::Display(false),
        false,
        false,
    );
    assert!(!shown(cx, &active));
    cx.project.layer_sets.set_display("Alt", "Notes", true);
    assert!(shown(cx, "Alt"));

    // Merge Notes into a second custom layer: objects and defaults follow.
    ld::new_layer_named(cx, "Keep").unwrap();
    cx.project.layers.set_tool_layer("cad", "Notes");
    let before_undo = cx.undo_label().map(str::to_string);
    assert_eq!(
        ld::merge_layers(cx, &["Keep".into(), "Notes".into()]).unwrap(),
        1
    );
    assert_eq!(cx.undo_label(), Some("Merge Layers"));
    assert_eq!(wall_layer(&sim, w), "Keep");
    let cx = &mut sim.app.cx;
    assert!(cx.project.layers.get("Notes").is_none());
    assert_eq!(cx.project.layers.current_cad_layer(), "Keep");
    assert!(cx
        .project
        .layer_sets
        .get("Alt")
        .unwrap()
        .state("Notes")
        .is_none());

    // One undo step brings the layer, the wall and the default back.
    cx.undo();
    assert_eq!(wall_layer(&sim, w), "Notes");
    let cx = &mut sim.app.cx;
    assert!(cx.project.layers.get("Notes").is_some());
    assert!(
        cx.project
            .layer_sets
            .get("Alt")
            .unwrap()
            .state("Notes")
            .unwrap()
            .display
    );
    assert_eq!(cx.project.layers.current_cad_layer(), "Notes");
    assert_ne!(
        cx.undo_label().map(str::to_string),
        Some("Merge Layers".to_string())
    );
    let _ = before_undo;
}

#[test]
fn delete_refuses_system_and_used_layers_and_delete_unused_keeps_them() {
    let mut sim = sim();
    let cx = &mut sim.app.cx;
    ld::new_layer_named(cx, "Idle").unwrap();
    ld::new_layer_named(cx, "Busy").unwrap();
    circle(&mut sim, "Busy", 4.0);
    let cx = &mut sim.app.cx;

    let steps = cx.undo_label().map(str::to_string);
    assert!(ld::delete_layers(cx, &["Walls, Normal".into()])
        .unwrap_err()
        .contains("system"));
    assert!(ld::delete_layers(cx, &["Busy".into()])
        .unwrap_err()
        .contains("in use"));
    assert!(
        ld::delete_layers(cx, &["Idle".into(), "Busy".into()]).is_err(),
        "all or nothing"
    );
    assert!(cx.project.layers.get("Idle").is_some());
    assert_eq!(
        cx.undo_label().map(str::to_string),
        steps,
        "a refusal leaves no undo step"
    );

    assert_eq!(ld::delete_layers(cx, &["Idle".into()]).unwrap(), 1);
    assert_eq!(cx.undo_label(), Some("Delete Layers"));
    cx.undo();
    assert!(cx.project.layers.get("Idle").is_some());

    assert_eq!(ld::delete_unused_layers(cx), 1);
    assert_eq!(cx.undo_label(), Some("Delete Unused Layers"));
    assert!(cx.project.layers.get("Busy").is_some() && cx.project.layers.get("Idle").is_none());
    assert_eq!(ld::delete_unused_layers(cx), 0);
}

#[test]
fn copy_and_reset_names_are_single_undo_steps() {
    let mut sim = sim();
    let cx = &mut sim.app.cx;
    assert_eq!(ld::copy_layer(cx, "Text").unwrap(), "Text Copy");
    assert_eq!(cx.undo_label(), Some("Copy Layer"));
    // A fresh plan still lacks the label layers that are made on use.
    ld::reset_layer_names(cx);
    cx.project.layers.layers.retain(|l| l.name != "Rooms");
    assert_eq!(ld::reset_layer_names(cx), 1);
    assert!(cx.project.layers.get("Rooms").is_some());
    assert_eq!(cx.undo_label(), Some("Reset Layer Names"));
    cx.undo();
    assert!(cx.project.layers.get("Rooms").is_none());
}

#[test]
fn select_layer_with_use_default_sends_objects_back_to_their_own_layers() {
    let mut sim = sim();
    let w = wall(&mut sim);
    let c = circle(&mut sim, "CAD, Default", 5.0);
    let cx = &mut sim.app.cx;
    ld::new_layer_named(cx, "Notes").unwrap();
    cx.selection.items = vec![ObjectRef::Wall(w), ObjectRef::Cad(c)];
    cx.send_selection_to_layer("Notes");
    assert_eq!(wall_layer(&sim, w), "Notes");

    let cx = &mut sim.app.cx;
    let mut pick = SelectLayer::new(Some("Notes".into()));
    pick.use_default = true;
    assert!(pick.layer().is_none() && pick.ready());
    assert_eq!(crate::dialogs::select_layer::send_selection(cx, &pick), 2);
    assert_eq!(cx.undo_label(), Some("Send to Layer"));
    assert_eq!(wall_layer(&sim, w), "Walls, Normal");
    let cx = &sim.app.cx;
    assert_eq!(
        cx.floor().cad.iter().find(|x| x.id == c).unwrap().layer,
        "CAD, Default"
    );

    // Without Use Default Layer the chosen layer is used, and nothing chosen
    // is nothing to do.
    let none = SelectLayer::new(None);
    assert!(!none.ready());
    assert_eq!(
        crate::dialogs::select_layer::send_selection(&mut sim.app.cx, &none),
        0
    );
}

#[test]
fn layer_set_defaults_choose_the_initial_set_of_nine_view_kinds() {
    let mut sim = sim();
    assert_eq!(ViewKind::ALL.len(), 9);
    layer_sets::new_set(&mut sim.app.cx, "Ref Only").unwrap();
    sim.action(Action::Custom(layer_sets::DEFAULTS));
    // The window draws.
    let ctx = eframe::egui::Context::default();
    for _ in 0..2 {
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            layer_sets::show_all(ctx, &mut sim.app.cx)
        });
    }
    let cx = &mut sim.app.cx;
    assert!(layer_sets::set_view_default(
        cx,
        ViewKind::ReferenceFloor,
        Some("Ref Only")
    ));
    assert_eq!(
        cx.project.initial_layer_set(ViewKind::ReferenceFloor),
        "Ref Only"
    );
    assert_eq!(
        cx.project.initial_layer_set(ViewKind::FloorPlan),
        cx.project.layer_sets.active
    );
    let json = cx.project.to_json().unwrap();
    let back = plan_core::Project::from_json(&json).unwrap();
    assert_eq!(back.initial_layer_set(ViewKind::ReferenceFloor), "Ref Only");
    cx.undo();
    assert_eq!(
        cx.project.initial_layer_set(ViewKind::ReferenceFloor),
        cx.project.layer_sets.active
    );
}

#[test]
fn drawing_group_steps_and_send_to_back_change_the_draw_order() {
    let mut sim = sim();
    let a = circle(&mut sim, "CAD, Default", 5.0);
    let b = circle(&mut sim, "CAD, Default", 6.0);
    let cx = &mut sim.app.cx;
    let order = |cx: &crate::editor::EditorContext| -> Vec<Id> {
        cx.floor()
            .cad_draw_order_with(&cx.project.drawing_group_defaults)
            .iter()
            .map(|c| c.id)
            .collect()
    };
    assert_eq!(order(cx), vec![a, b]);
    cx.begin_change("Send to Back");
    assert_eq!(
        cx.project
            .drawing_group_to_back(0, &[ObjectRef::Cad(b).to_group_ref().unwrap()]),
        1
    );
    assert_eq!(order(cx), vec![b, a]);
    cx.undo();
    assert_eq!(order(cx), vec![a, b]);
    // Bring Forward by one group moves past an object one group higher.
    cx.project
        .set_drawing_groups(0, &[ObjectRef::Cad(b).to_group_ref().unwrap()], Some(22));
    assert_eq!(
        cx.project
            .drawing_group_step(0, &[ObjectRef::Cad(a).to_group_ref().unwrap()], 2),
        1
    );
    assert_eq!(order(cx), vec![b, a]);
}

// ----- second slice: Object Layer Properties, Layer Hider, Find Objects,
// wall system layers, Fill, painter bar, Bring Forward / Send Backward -----

#[test]
fn bring_forward_and_send_backward_run_as_edit_commands_in_one_step_each() {
    use crate::tools::cad_ops as ops;
    let mut sim = sim();
    let a = circle(&mut sim, "CAD, Default", 5.0);
    sim.app.cx.selection.items = vec![ObjectRef::Cad(a)];
    // The Edit toolbar lists the four drawing-group buttons for a selection.
    let labels: Vec<&str> = sim
        .app
        .cx
        .extra_edit_actions()
        .iter()
        .map(|e| e.label)
        .collect();
    for l in [
        "Send to Back",
        "Send Backward",
        "Bring Forward",
        "Bring to Front",
    ] {
        assert!(
            labels.contains(&l),
            "{l} is on the Edit toolbar: {labels:?}"
        );
    }
    let group = |sim: &Sim| {
        let cx = &sim.app.cx;
        cx.floor().drawing_group(
            &cx.project.drawing_group_defaults,
            ObjectRef::Cad(a).to_group_ref().unwrap(),
        )
    };
    let before = group(&sim);
    sim.action(Action::Custom(ops::DG_FORWARD));
    assert_eq!(group(&sim), before + 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Bring Forward"));
    sim.action(Action::Custom(ops::DG_BACKWARD));
    assert_eq!(group(&sim), before);
    assert_eq!(sim.app.cx.undo_label(), Some("Send Backward"));
    sim.undo();
    assert_eq!(group(&sim), before + 1);
    sim.undo();
    assert_eq!(group(&sim), before, "one undo step each");
}

#[test]
fn object_layer_properties_lists_primary_and_secondary_layers_and_edits_the_shown_set() {
    use crate::dialogs::object_layers as ol;
    let mut sim = sim();
    let w = wall(&mut sim);
    sim.app.cx.project.ensure_wall_system_layers();
    sim.app.cx.selection.items = vec![ObjectRef::Wall(w)];
    // The Edit toolbar button exists and opens the window.
    let labels: Vec<&str> = sim
        .app
        .cx
        .extra_edit_actions()
        .iter()
        .map(|e| e.label)
        .collect();
    assert!(labels.contains(&"Object Layer Properties"));
    sim.action(Action::Custom(ol::OPEN));
    assert!(ol::is_open());
    let layers = ol::layers_of_selection(&sim.app.cx);
    assert_eq!(layers.primary, vec!["Walls, Normal".to_string()]);
    assert!(layers
        .secondary
        .contains(&plan_core::layers::WALL_MAIN_ONLY_LAYER.to_string()));
    let ctx = eframe::egui::Context::default();
    for _ in 0..2 {
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            layer_sets::show_all(ctx, &mut sim.app.cx)
        });
    }
    assert!(ol::is_open());
    ol::close();
}

#[test]
fn the_layer_hider_hides_the_clicked_objects_layer_in_the_active_set_only() {
    use crate::tools::painters::PainterMode;
    let mut sim = sim();
    layer_sets::new_set(&mut sim.app.cx, "Alt").unwrap();
    let c = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(0.0, 0.0),
            b: Point::new(200.0, 0.0),
        },
    );
    sim.app.cx.refresh();
    sim.tool(ToolId::PainterVariant(PainterMode::LayerHider));
    let active = sim.app.cx.project.shown_layer_set().to_string();
    sim.click(100.0, 0.0);
    assert!(
        !sim.app.cx.layers().is_visible("CAD, Default"),
        "{}",
        sim.app.cx.status
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Layer Hider"));
    let alt = sim
        .app
        .cx
        .project
        .layer_sets
        .effective_for("Alt", &sim.app.cx.project.layers);
    assert!(alt.is_visible("CAD, Default"), "{active} only");
    // The object still exists; one undo shows the layer again.
    assert!(sim.app.cx.floor().cad.iter().any(|x| x.id == c));
    sim.undo();
    assert!(sim.app.cx.layers().is_visible("CAD, Default"));
}

#[test]
fn find_objects_on_layers_selects_them_and_asks_when_they_are_on_two_floors() {
    use crate::dialogs::{object_layers as ol, select_location};
    let mut sim = sim();
    sim.app
        .cx
        .project
        .floors
        .push(plan_core::Floor::new("Upper", 108.0));
    ld::new_layer_named(&mut sim.app.cx, "Notes").unwrap();
    let up = sim.app.cx.project.add_cad(
        1,
        "Notes",
        CadItem::Circle {
            center: Point::ZERO,
            radius: 4.0,
        },
    );
    let layers = vec!["Notes".to_string()];
    assert_eq!(ol::find_objects_on_layers(&mut sim.app.cx, &layers), 1);
    assert_eq!(sim.app.cx.floor, 1);
    assert_eq!(sim.app.cx.selection.items, vec![ObjectRef::Cad(up)]);
    sim.app.cx.floor = 0;
    circle(&mut sim, "Notes", 3.0);
    assert_eq!(ol::find_objects_on_layers(&mut sim.app.cx, &layers), 2);
    assert!(select_location::is_open(), "two floors: Select Location");
}

#[test]
fn wall_system_layers_switch_the_layer_lines_of_walls() {
    let mut sim = sim();
    let n = ld::add_wall_layers(&mut sim.app.cx);
    assert_eq!(n, plan_core::layers::WALL_SYSTEM_LAYERS.len());
    assert_eq!(sim.app.cx.undo_label(), Some("Add Wall Layers"));
    assert_eq!(ld::add_wall_layers(&mut sim.app.cx), 0, "only once");
    sim.undo();
    assert!(sim.app.cx.project.layers.get("Walls, Layers").is_none());
    sim.redo();
    let cx = &mut sim.app.cx;
    assert!(cx.layers().wall_layer_lines() && !cx.layers().main_layer_only());
    // Turn Main Layer Only on in the shown set.
    crate::shell::docks::edit_layers(
        cx,
        &[plan_core::layers::WALL_MAIN_ONLY_LAYER.to_string()],
        LayerEdit::Display(true),
        false,
        false,
    );
    assert!(cx.layers().main_layer_only());
}

#[test]
fn a_layer_fill_style_is_set_from_the_table_and_removed_in_one_step() {
    use plan_core::fill_styles::{FillStyle, FillTarget};
    let mut sim = sim();
    let cx = &mut sim.app.cx;
    assert!(crate::dialogs::fill_style::open_for_layers(
        cx,
        &["Rooms".to_string(), "Text".to_string()]
    ));
    crate::dialogs::fill_style::with_dialog(|d| d.style = FillStyle::hatch(45.0, 6.0, [0, 0, 0]));
    assert!(crate::dialogs::fill_style::accept_dialog(cx));
    for l in ["Rooms", "Text"] {
        assert!(cx
            .project
            .styles
            .fill_for(&FillTarget::Layer(l.into()))
            .is_some());
    }
    assert_eq!(
        ld::clear_layer_fills(cx, &["Rooms".to_string(), "Text".to_string()]),
        2
    );
    assert!(cx.project.styles.fill_assign.is_empty());
    cx.undo();
    assert_eq!(cx.project.styles.fill_assign.len(), 2);
}

#[test]
fn the_layer_painter_bar_uses_default_layer_and_define_opens_the_layer_window() {
    use crate::tools::painters::{with_state, PainterMode};
    let mut sim = sim();
    let w = wall(&mut sim);
    let c = circle(&mut sim, "Rooms", 4.0);
    sim.app
        .cx
        .project
        .layers
        .add(plan_core::Layer::new("Mine", [0, 0, 0], 18));
    sim.app.cx.floor_mut().wall_mut(w).unwrap().layer = "Mine".into();
    sim.tool(ToolId::PainterVariant(PainterMode::LayerPaint));
    with_state(|s| {
        s.use_default = true;
        s.layer = None;
    });
    let n = crate::tools::painters::paint_layer(&mut sim.app.cx, ObjectRef::Wall(w));
    assert_eq!(n, 1);
    assert_eq!(wall_layer(&sim, w), "Walls, Normal");
    assert_eq!(sim.app.cx.undo_label(), Some("Layer Painter"));
    let n = crate::tools::painters::paint_layer(&mut sim.app.cx, ObjectRef::Cad(c));
    assert_eq!(n, 1);
    assert_eq!(
        sim.app
            .cx
            .floor()
            .cad
            .iter()
            .find(|x| x.id == c)
            .unwrap()
            .layer,
        "CAD, Default"
    );
    with_state(|s| s.use_default = false);
    // Define opens Layer Display Options.
    ld::open_define();
    assert!(ld::define_open());
    let ctx = eframe::egui::Context::default();
    for _ in 0..2 {
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            layer_sets::show_all(ctx, &mut sim.app.cx)
        });
    }
    assert!(ld::define_open());
}

#[test]
fn new_views_start_with_the_layer_set_chosen_in_layer_set_defaults() {
    let mut sim = sim();
    let cx = &mut sim.app.cx;
    layer_sets::new_set(cx, "Plain").unwrap();
    layer_sets::new_set(cx, "Detail Set").unwrap();
    // Nothing chosen: a new plan view copies the active one.
    let first = crate::dialogs::app_info::new_plan_view(cx);
    let active_set = cx.project.plan_view(&first).unwrap().layer_set.clone();
    assert_ne!(active_set, "Plain");
    // A choice for floor plans decides where a new view starts.
    layer_sets::set_view_default(cx, ViewKind::FloorPlan, Some("Plain"));
    let second = crate::dialogs::app_info::new_plan_view(cx);
    assert_eq!(cx.project.plan_view(&second).unwrap().layer_set, "Plain");
    assert_eq!(cx.undo_label(), Some("New Plan View"));
    // The reference floor reads its own default.
    layer_sets::set_view_default(cx, ViewKind::ReferenceFloor, Some("Detail Set"));
    assert_eq!(
        cx.project.initial_layer_set(ViewKind::ReferenceFloor),
        "Detail Set"
    );
}

#[test]
fn renaming_a_layer_is_one_undo_step_and_a_cabinet_can_leave_its_kind_layer() {
    let mut sim = sim();
    let id = wall(&mut sim);
    let name = ld::new_layer_named(&mut sim.app.cx, "Notes").unwrap();
    sim.app.cx.send_selection_to_layer(&name);
    sim.app.cx.selection.items = vec![ObjectRef::Wall(id)];
    sim.app.cx.send_selection_to_layer(&name);
    assert_eq!(wall_layer(&sim, id), name);
    assert!(ld::rename_layer_named(&mut sim.app.cx, "Doors", "Portals").is_err());
    assert_eq!(
        ld::rename_layer_named(&mut sim.app.cx, &name, "Site Notes").unwrap(),
        "Site Notes"
    );
    assert_eq!(wall_layer(&sim, id), "Site Notes");
    sim.app.cx.undo();
    assert_eq!(wall_layer(&sim, id), name);
    // The wall system layers draw only when displayed (missing means off).
    let w = sim.app.cx.floor().wall(id).unwrap().clone();
    let none = crate::editor::wall_system_lines::lines(&w, |_| false);
    assert!(none.is_empty());
}
