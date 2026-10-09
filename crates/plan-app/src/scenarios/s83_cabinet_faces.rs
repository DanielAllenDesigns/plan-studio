//! Scenario 83 (round 16): cabinet face items, special shapes and custom
//! countertops. A kitchen island is built from a bow-front end cabinet with
//! false drawers and a rollout, two standard cabinets and a custom
//! countertop with a waterfall edge. Three cabinets are opened together
//! with No Change where values differ, and each OK is one undo step
//! (CB-479, CB-484, CB-486, CB-492..CB-497, CB-631..CB-633).

use super::Sim;
use crate::dialogs::cabinet_multi::Val2;
use crate::editor::actions::EditActionKind;
use crate::editor::placed::{add_cabinet, cabinet_by_id};
use crate::editor::ObjectRef;
use plan_cabinets::{
    Cabinet, CabinetStyle, EdgeMolding, FaceItem, FaceLayout, ItemKind, SpecialShape,
};
use plan_core::geometry::Point;
use plan_core::Id;

/// Places `c` at `(x, y)` on the active floor.
fn put(sim: &mut Sim, mut c: Cabinet, x: f64, y: f64) -> Id {
    c.position = Point::new(x, y);
    add_cabinet(&mut sim.app.cx.project, 0, c).unwrap()
}

/// The island: a bow-front end cabinet with false drawers and a rollout,
/// then a 30 in and a 36 in cabinet.
fn island(sim: &mut Sim) -> [Id; 3] {
    let mut end = Cabinet::base(24.0);
    end.convert(CabinetStyle::Special(SpecialShape::BowFront))
        .unwrap();
    end.set_special_amount(3.0).unwrap();
    end.face = FaceLayout {
        items: vec![
            ItemKind::Separation.make(1.5),
            ItemKind::FalseDrawer.make(6.0),
            ItemKind::FalseDoubleDrawer.make(6.0),
            ItemKind::Separation.make(1.5),
            ItemKind::Rollout.make(0.0),
            ItemKind::Separation.make(1.5),
        ],
        frame_width: 1.5,
    };
    let mut mid = Cabinet::base(30.0);
    mid.door_style.apply_builtin("Shaker Door");
    let mut last = Cabinet::base(36.0);
    last.door_style.apply_builtin("Raised Panel Door");
    [
        put(sim, end, 100.0, 100.0),
        put(sim, mid, 124.0, 100.0),
        put(sim, last, 154.0, 100.0),
    ]
}

#[test]
fn the_island_end_is_a_bow_front_with_false_drawers_and_a_rollout() {
    let mut sim = Sim::new();
    let [end, ..] = island(&mut sim);
    let c = cabinet_by_id(sim.app.cx.floor(), end).unwrap();
    assert_eq!(c.style(), CabinetStyle::Special(SpecialShape::BowFront));
    let leaves = c.face.resolve(c.face_height(), c.width).unwrap();
    let kinds: Vec<_> = leaves.iter().map(|r| r.item.kind()).collect();
    assert!(kinds.contains(&ItemKind::FalseDrawer));
    assert!(kinds.contains(&ItemKind::FalseDoubleDrawer));
    assert!(kinds.contains(&ItemKind::Rollout));
    // The bow pushes the front line out past the cabinet's depth.
    assert!(c.footprint_local().iter().any(|p| p.y > c.depth + 1.0));
}

#[test]
fn a_waterfall_edge_builds_a_slab_down_to_the_floor() {
    let mut top = Cabinet::custom_countertop(
        &[
            Point::new(100.0, 100.0),
            Point::new(190.0, 100.0),
            Point::new(190.0, 124.0),
            Point::new(100.0, 124.0),
        ],
        1.5,
        36.0,
    )
    .unwrap();
    assert!(top.set_waterfall(1, true));
    assert!(!top.set_waterfall(9, true), "no such edge");
    assert_eq!(top.waterfall_edges(), vec![1]);
    let (ring, drop) = top.waterfall_slab(1).unwrap();
    assert_eq!(ring.len(), 4);
    assert!((drop - top.elevation).abs() < 1e-9, "runs to the floor");
    top.top_spec.waterfall_auto_height = false;
    top.top_spec.waterfall_height = 20.0;
    assert!((top.waterfall_slab(1).unwrap().1 - 20.0).abs() < 1e-9);
    top.set_top_edge_molding(0, EdgeMolding::NoMolding, false);
    assert!(!top.top_edge_has_molding(0) && top.top_edge_has_molding(1));
}

#[test]
fn three_cabinets_open_together_with_no_change_and_ok_is_one_undo_step() {
    let mut sim = Sim::new();
    let ids = island(&mut sim);
    sim.tool(crate::tools::ToolId::Select);
    for id in &ids {
        sim.cx().selection.add(ObjectRef::Cabinet(*id));
    }
    assert!(sim
        .cx()
        .common_edit_actions()
        .iter()
        .any(|a| a.kind == EditActionKind::OpenObject));
    sim.cx().apply_edit_action(EditActionKind::OpenObject);
    sim.app.process_requests();
    assert!(sim.app.has_dialog());
    {
        let d = sim.app.spec.cabinets_dialog_mut().expect("multi dialog");
        assert_eq!(d.ids().len(), 3);
        // Widths and door styles differ; the countertop and framing do not.
        assert!(d.is_mixed("width"));
        assert!(d.is_mixed("door_style"));
        assert!(!d.is_mixed("framed"));
        assert!(!d.is_mixed("height"));
        assert!(d.touched().is_empty());
        d.edit("door_style", Val2::Text("Slab Door".into()));
        assert_eq!(d.touched(), vec!["door_style"]);
        assert!(!d.is_mixed("door_style"), "edited fields stop being mixed");
    }
    let before: Vec<f64> = ids
        .iter()
        .map(|i| cabinet_by_id(sim.app.cx.floor(), *i).unwrap().width)
        .collect();
    sim.ok();
    assert!(!sim.app.has_dialog());
    assert_eq!(sim.app.cx.undo_label(), Some("Cabinet Specification"));
    for (id, w) in ids.iter().zip(&before) {
        let c = cabinet_by_id(sim.app.cx.floor(), *id).unwrap();
        assert_eq!(c.door_style.name, "Slab Door");
        assert_eq!(c.width, *w, "No Change fields keep their values");
    }
    // One undo puts all three door styles back.
    sim.undo();
    let names: Vec<String> = ids
        .iter()
        .map(|i| {
            cabinet_by_id(sim.app.cx.floor(), *i)
                .unwrap()
                .door_style
                .name
        })
        .collect();
    assert_ne!(names[0], "Slab Door");
    assert_eq!(names[1], "Shaker Door");
    assert_eq!(names[2], "Raised Panel Door");
}

#[test]
fn a_same_value_typed_over_several_cabinets_reaches_all_of_them() {
    let mut sim = Sim::new();
    let ids = island(&mut sim);
    sim.tool(crate::tools::ToolId::Select);
    for id in &ids {
        sim.cx().selection.add(ObjectRef::Cabinet(*id));
    }
    sim.cx().apply_edit_action(EditActionKind::OpenObject);
    sim.app.process_requests();
    {
        let d = sim.app.spec.cabinets_dialog_mut().unwrap();
        d.edit("has_bs", Val2::Bool(true));
        d.edit("bs_height", Val2::Len(5.0));
        d.edit("open_doors", Val2::Bool(true));
    }
    sim.ok();
    for id in &ids {
        let c = cabinet_by_id(sim.app.cx.floor(), *id).unwrap();
        assert_eq!(c.backsplash.as_ref().map(|b| b.height), Some(5.0));
        assert!(c.show_open.doors);
    }
    sim.undo();
    for id in &ids {
        assert!(cabinet_by_id(sim.app.cx.floor(), *id)
            .unwrap()
            .backsplash
            .is_none());
    }
}

#[test]
fn one_cabinet_dialog_ok_is_one_undo_step_and_keeps_item_settings() {
    let mut sim = Sim::new();
    let [end, ..] = island(&mut sim);
    assert!(sim.open_spec(ObjectRef::Cabinet(end)));
    {
        let d = sim.app.spec.cabinet_dialog_mut().expect("single dialog");
        let cab = d.draft_mut();
        // The second drawer becomes a drawer with a stop at 50 percent open.
        let item = &mut cab.face.items[1];
        item.props_mut().unwrap().percent_open = Some(50.0);
        *item = ItemKind::Drawer
            .make(6.0)
            .with_props(item.props().cloned().unwrap());
        assert_eq!(item.kind(), ItemKind::Drawer);
    }
    sim.ok();
    let c = cabinet_by_id(sim.app.cx.floor(), end).unwrap();
    assert_eq!(c.face.items[1].kind(), ItemKind::Drawer);
    assert_eq!(c.face.items[1].props().unwrap().percent_open, Some(50.0));
    assert!(matches!(
        c.face.items[2].base(),
        FaceItem::FalseDoubleDrawer { .. }
    ));
    sim.undo();
    let c = cabinet_by_id(sim.app.cx.floor(), end).unwrap();
    assert_eq!(c.face.items[1].kind(), ItemKind::FalseDrawer);
}

#[test]
fn edit_toolbar_opens_and_closes_doors_and_makes_a_molding_polyline() {
    use crate::editor::cabinet_edit::{CLOSE_DOORS, MOLDING_LAYER, MOLDING_POLYLINE, OPEN_DOORS};
    let mut sim = Sim::new();
    let ids = island(&mut sim);
    let mut wall = Cabinet::wall(30.0);
    wall.moldings.push(plan_cabinets::Molding::crown());
    let wid = put(&mut sim, wall, 100.0, 50.0);
    sim.tool(crate::tools::ToolId::Select);
    for id in ids.iter().chain([&wid]) {
        sim.cx().selection.add(ObjectRef::Cabinet(*id));
    }
    let labels: Vec<_> = sim
        .cx()
        .extra_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect();
    assert!(labels.contains(&"Open Cabinet Doors/Drawers"), "{labels:?}");
    assert!(labels.contains(&"Close Cabinet Doors/Drawers"));
    assert!(labels.contains(&"Generate Custom Countertop"));
    assert!(labels.contains(&"Make Cabinet Molding Polyline"));
    // Open: one undo step for all four cabinets.
    sim.cx().run_custom(OPEN_DOORS);
    for id in ids.iter().chain([&wid]) {
        let c = cabinet_by_id(sim.app.cx.floor(), *id).unwrap();
        assert!(c.show_open.doors && c.show_open.drawers && c.show_open.rollouts);
    }
    sim.cx().run_custom(CLOSE_DOORS);
    assert!(!cabinet_by_id(sim.app.cx.floor(), wid)
        .unwrap()
        .show_open
        .any());
    assert_eq!(sim.app.cx.undo_label(), Some("Close Cabinet Doors/Drawers"));
    sim.undo();
    assert!(
        cabinet_by_id(sim.app.cx.floor(), wid)
            .unwrap()
            .show_open
            .doors
    );
    sim.undo();
    assert!(!cabinet_by_id(sim.app.cx.floor(), wid)
        .unwrap()
        .show_open
        .any());
    // Make Cabinet Molding Polyline: the crown becomes a polyline on its own layer.
    let cad_before = sim.app.cx.floor().cad.len();
    sim.cx().run_custom(MOLDING_POLYLINE);
    let cad = &sim.app.cx.floor().cad;
    assert_eq!(cad.len(), cad_before + 1);
    assert!(cad.iter().any(|o| o.layer == MOLDING_LAYER));
    assert!(cabinet_by_id(sim.app.cx.floor(), wid)
        .unwrap()
        .moldings
        .is_empty());
    sim.undo();
    assert_eq!(sim.app.cx.floor().cad.len(), cad_before);
    assert_eq!(
        cabinet_by_id(sim.app.cx.floor(), wid)
            .unwrap()
            .moldings
            .len(),
        1
    );
}

#[test]
fn the_editor_stores_exposed_ends_for_an_island_after_an_edit() {
    let mut sim = Sim::new();
    let ids = island(&mut sim);
    // The sync runs with every cabinet edit; call it as the editor does.
    sim.app.cx.begin_change("test");
    crate::tools::cabinet::sync_auto_fillers(&mut sim.app.cx);
    let c = cabinet_by_id(sim.app.cx.floor(), ids[1]).unwrap();
    let ends = c.ends.expect("ends are stored for a plain cabinet");
    // The middle cabinet touches its neighbours on both sides.
    assert!(ends.left && ends.right && !ends.back);
    let end = cabinet_by_id(sim.app.cx.floor(), ids[0]).unwrap();
    assert!(
        !end.ends.is_none_or(|e| e.left),
        "the island end stands free on the left"
    );
}
