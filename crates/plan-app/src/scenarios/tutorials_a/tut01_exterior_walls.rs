//! Lesson 1, Exterior Walls (guide pp. 3-27). The plan in the guide is only
//! shown as pictures, so the shell is the cottage approximation.
use crate::dialogs::floor::FloorDialog;
use crate::dialogs::floor_defaults::{FloorDefaultsDialog, FloorDefaultsTarget};
use crate::scenarios::tutorials_support::*;
use crate::scenarios::Sim;
use crate::tools::dimension::DimMode;
use crate::tools::ToolId;

#[test]
fn six_drawn_walls_close_into_one_room_one_undo_step_each() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    for i in 0..COTTAGE.len() {
        let (a, b) = (COTTAGE[i], COTTAGE[(i + 1) % COTTAGE.len()]);
        let r = assert_one_undo_step(&mut sim, "draw wall", |s| s.drag(a, b));
        assert_eq!(r.commit.as_deref(), Some("Draw Wall"));
    }
    assert_eq!(sim.floor_walls(), 6);
    assert_eq!(sim.app.cx.rooms.len(), 1, "the shell is one room");
    assert!(crate::editor::rooms_edit::living_area_total_sq_ft(&sim.app.cx) > 1500.0);
}

#[test]
fn first_floor_ceiling_height_97_1_8_is_one_undo_step() {
    let mut sim = cottage();
    let mut settings = sim.app.cx.floor().settings.clone();
    settings.floor_structure_thickness = settings.floor_structure_thickness.max(1.0);
    let mut d = FloorDefaultsDialog::new(
        FloorDefaultsTarget::ThisFloor("1st Floor".into()),
        97.125,
        settings,
        vec![],
    );
    d.set_ceiling_height(97.125);
    assert_one_undo_step(&mut sim, "Floor Defaults", |s| {
        FloorDialog::Defaults(Box::new(d)).apply(&mut s.app.cx)
    });
    assert_eq!(sim.app.cx.floor().ceiling_height, 97.125);
}

#[test]
fn auto_exterior_dimensions_are_one_undo_step() {
    let mut sim = cottage();
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    assert_one_undo_step(&mut sim, "Auto Exterior Dimensions", |s| {
        s.click(240.0, 240.0)
    });
    assert!(sim.app.cx.floor().dimensions.len() >= 2);
    assert_eq!(sim.undo().as_deref().map(|_| ()), Some(()));
    assert!(sim.app.cx.floor().dimensions.is_empty());
}

#[test]
fn stone_6_wall_type_with_a_3_inch_layer() {
    // Lesson 1: the exterior default becomes Stone-6 and its stone layer is
    // made 3 in thick with a library material and a solid fill.
    use crate::dialogs::wall_types::{self, with_host, APPLY, OPEN};
    use plan_core::fill_styles::FillStyle;
    let mut sim = Sim::new();
    sim.app.cx.defaults.exterior_wall.wall_type = "stone-6".into();
    chic_cottage(&mut sim);
    let thick = |sim: &Sim| sim.app.cx.floor().walls[0].thickness;
    assert_eq!(
        sim.app.cx.floor().walls[0].wall_type.as_deref(),
        Some("stone-6")
    );
    let before = thick(&sim);
    wall_types::close_host();
    sim.app.cx.run_custom(OPEN);
    with_host(|d| {
        assert_eq!(d.selected_name(), Some("stone-6"));
        let t = d.table_mut();
        t.selected = 0;
        t.rows[0].thickness = 3.0;
        t.rows[0].material = "Stone Veneer".into();
        t.rows[0].spec.fill = Some(FillStyle::solid([150, 150, 150]));
    })
    .expect("the dialog is open");
    let depth = sim.app.cx.undo_depth();
    sim.app.cx.run_custom(APPLY);
    assert_eq!(sim.app.cx.undo_depth(), depth + 1, "one OK, one undo step");
    assert!((thick(&sim) - before - 1.5).abs() < 1e-9, "1.5 -> 3 in");
    assert_eq!(
        sim.app.cx.project.wall_layer_fill("stone-6", 0),
        Some(&FillStyle::solid([150, 150, 150]))
    );
    // The main (framing) layer keeps no fill of its own.
    assert!(sim.app.cx.project.wall_layer_fill("stone-6", 2).is_none());
    sim.undo();
    assert!((thick(&sim) - before).abs() < 1e-9);
    assert!(sim.app.cx.project.wall_layer_fill("stone-6", 0).is_none());
}

#[test]
#[ignore = "T7-01: R-143"]
fn floor_and_ceiling_platform_defaults() {
    assert_ignored_break("R-143");
}

#[test]
#[ignore = "T7-01: RF-164"]
fn closing_the_shell_builds_the_roof_and_exterior_dimensions_automatically() {
    // RF-164 Auto Refresh / DIM-52: the template builds both when a room closes.
    let sim = cottage();
    assert!(!sim.app.cx.floor().dimensions.is_empty());
    assert_ignored_break("RF-164");
}

#[test]
#[ignore = "T7-01: DIM-48"]
fn move_both_ends_of_a_wall_by_typed_dimension() {
    assert_ignored_break("DIM-48");
}

#[test]
#[ignore = "T7-01: TXT-65"]
fn rich_text_uppercase_button() {
    assert_ignored_break("TXT-65");
}

#[test]
#[ignore = "T7-01: APP-93"]
fn file_make_a_copy() {
    assert_ignored_break("APP-93");
}
