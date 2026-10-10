//! Scenario 71: Wall Type Definitions depth (Round 16, brief 12; W-137,
//! W-138, W-150..W-153; manual pp. 414-421): Siding-6 gets a furred layer
//! with a fill, applied through Build > Wall > Define Wall Types; the walls
//! of the type take the new thickness, the plan fill follows the layer, one
//! undo step per OK; a Room Divider and a Partition wall type; the Library
//! round trip with `_2` suffixes; the saved plan keeps it all.

use super::{draw_shell, Sim};
use crate::dialogs::wall_types::{self, with_host, APPLY, OPEN};
use crate::tools::ToolId;
use plan_core::assemblies::LayerRole;
use plan_core::defaults::{WallLayer, WallTypeDef};
use plan_core::fill_styles::FillStyle;
use plan_core::wall_types as core_types;
use plan_core::WallKind;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    sim
}

fn exterior_type(sim: &Sim) -> String {
    sim.app
        .cx
        .project
        .floors
        .iter()
        .flat_map(|f| f.walls.iter())
        .find_map(|w| w.wall_type.clone())
        .expect("a drawn wall has a wall type")
}

fn open_and_edit(sim: &mut Sim, f: impl FnOnce(&mut wall_types::WallTypeDialog)) {
    wall_types::close_host();
    sim.app.cx.run_custom(OPEN);
    assert!(wall_types::host_open());
    with_host(f).expect("the dialog is open");
}

#[test]
fn a_furred_siding_layer_reaches_the_walls_the_plan_fill_and_one_undo_step() {
    let mut sim = house();
    let name = exterior_type(&sim);
    let before = sim
        .app
        .cx
        .project
        .floors
        .iter()
        .flat_map(|f| f.walls.iter())
        .find(|w| w.wall_type.as_deref() == Some(name.as_str()))
        .map(|w| w.thickness)
        .unwrap();
    let layers_before = sim
        .app
        .cx
        .defaults
        .wall_type(&name)
        .map_or(0, |t| t.layers.len());
    let depth_before = sim.app.cx.undo_depth();
    open_and_edit(&mut sim, |d| {
        let t = d.table_mut();
        t.selected = 0;
        t.insert_below();
        let i = t.selected;
        t.rows[i].name = "Furring".into();
        t.rows[i].thickness = 0.75;
        t.rows[i].material = "Furring".into();
        t.set_role(i, LayerRole::Standard);
        t.rows[i].spec.fill = Some(FillStyle::solid([200, 120, 40]));
    });
    sim.app.cx.run_custom(APPLY);
    assert!(!wall_types::host_open());

    let ty = sim.app.cx.project.wall_type_def(&name).cloned().unwrap();
    assert_eq!(ty.layers.len(), layers_before + 1);
    assert_eq!(ty.layers[1].name, "Furring");
    let after = sim
        .app
        .cx
        .project
        .floors
        .iter()
        .flat_map(|f| f.walls.iter())
        .find(|w| w.wall_type.as_deref() == Some(name.as_str()))
        .map(|w| w.thickness)
        .unwrap();
    assert!((after - before - 0.75).abs() < 1e-9, "{before} -> {after}");
    // The plan fill follows the layer.
    let fill = sim
        .app
        .cx
        .project
        .wall_layer_fill(&name, 1)
        .expect("a fill");
    assert_eq!(fill, &FillStyle::solid([200, 120, 40]));
    assert!(sim.app.cx.project.wall_layer_fill(&name, 0).is_none());
    // One OK, one undo step.
    assert_eq!(sim.app.cx.undo_depth(), depth_before + 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Wall Type Definitions"));
    sim.undo();
    let back = sim
        .app
        .cx
        .project
        .floors
        .iter()
        .flat_map(|f| f.walls.iter())
        .find(|w| w.wall_type.as_deref() == Some(name.as_str()))
        .map(|w| w.thickness)
        .unwrap();
    assert!((back - before).abs() < 1e-9);
    assert!(sim.app.cx.project.wall_layer_fill(&name, 1).is_none());
}

#[test]
fn a_room_divider_and_a_partition_wall_are_types_that_keep_their_flags() {
    let mut sim = house();
    open_and_edit(&mut sim, |d| {
        d.new_room_divider();
        d.new_type();
        let t = d.table_mut();
        t.props.partition = true;
        t.rows[0].thickness = 0.5;
        t.props.room_divider = false;
        t.rows[0].name = "Glass".into();
    });
    sim.app.cx.run_custom(APPLY);
    let p = &sim.app.cx.project;
    let divider = p.wall_type_def("Room Divider").expect("the divider type");
    assert!(divider.props.room_divider);
    assert_eq!(divider.thickness(), 0.0);
    assert!(divider.problems().is_empty());
    let partition = p
        .wall_type_def("Room Divider copy 1")
        .expect("the partition type");
    assert!(partition.props.partition);
    assert!(!partition.props.room_divider);
    // The defaults list (and so the saved template) has them too.
    assert!(sim
        .app
        .cx
        .defaults
        .wall_types
        .iter()
        .any(|t| t.name == "Room Divider"));
}

#[test]
fn delete_all_unused_removes_types_from_the_plan_and_the_defaults_in_one_step() {
    let mut sim = house();
    let keep = exterior_type(&sim);
    let total_before = sim.app.cx.defaults.wall_types.len();
    open_and_edit(&mut sim, |d| {
        let n = d.delete_unused();
        assert!(n > 0);
    });
    sim.app.cx.run_custom(APPLY);
    let left = &sim.app.cx.defaults.wall_types;
    assert!(left.len() < total_before);
    assert!(left.iter().any(|t| t.name == keep), "the type in use stays");
    sim.undo();
}

#[test]
fn library_walls_get_a_suffix_and_come_back_with_their_type() {
    let sim = house();
    let w = sim.app.cx.floor().walls[0].clone();
    let ty: Option<WallTypeDef> = w
        .wall_type
        .as_deref()
        .and_then(|n| sim.app.cx.project.wall_type_def(n).cloned())
        .or_else(|| {
            w.wall_type
                .as_deref()
                .and_then(|n| sim.app.cx.defaults.wall_type(n).cloned())
        });
    let mut lib = Vec::new();
    let a = core_types::add_to_library(&mut lib, "Garage Wall", &w, ty.as_ref());
    let b = core_types::add_to_library(&mut lib, "Garage Wall", &w, ty.as_ref());
    assert_eq!((a.as_str(), b.as_str()), ("Garage Wall", "Garage Wall_2"));
    // Drawn in another plan that already has a different type of that name.
    let mut other = vec![WallTypeDef {
        name: ty.as_ref().unwrap().name.clone(),
        layers: vec![WallLayer::new("Slab", 8.0, true, "Concrete")],
        kind: WallKind::Exterior,
        props: Default::default(),
    }];
    let name = core_types::bring_type(&mut other, ty.as_ref().unwrap());
    assert_eq!(name, format!("{}_2", ty.unwrap().name));
}

#[test]
fn the_saved_plan_keeps_roles_fills_and_flags_and_an_old_plan_loads() {
    let mut sim = house();
    let name = exterior_type(&sim);
    open_and_edit(&mut sim, |d| {
        let t = d.table_mut();
        t.set_role(0, LayerRole::Cladding);
        t.rows[0].spec.fill = Some(FillStyle::solid([1, 2, 3]));
        t.props.align.dimension = Some(1);
    });
    sim.app.cx.run_custom(APPLY);
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    let ty = back.wall_type_def(&name).unwrap();
    assert_eq!(ty.layers[0].resolved_role(), LayerRole::Cladding);
    assert_eq!(ty.props.align.dimension, Some(1));
    assert!(ty.layers[0].spec.fill.is_some());
    // The two-field layer an old plan stored loads with the default spec.
    let old = r#"{"name":"A","thickness":1.0,"is_main":true,"material":"X"}"#;
    let l: WallLayer = serde_json::from_str(old).unwrap();
    assert!(l.spec.is_default());
}
