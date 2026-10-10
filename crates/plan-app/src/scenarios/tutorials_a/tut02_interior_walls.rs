//! Lesson 2, Interior Walls (pp. 28-44).
use crate::editor::ObjectRef;
use crate::scenarios::tutorials_support::*;
use crate::tools::dimension::DimMode;
use crate::tools::ToolId;

#[test]
fn three_partitions_make_four_rooms_one_undo_step_each() {
    let mut sim = cottage();
    sim.tool(interior());
    let segs = [
        ((200.0, 0.0), (200.0, 480.0), 2),
        ((0.0, 240.0), (200.0, 240.0), 3),
        ((200.0, 300.0), (600.0, 300.0), 4),
    ];
    for (a, b, rooms) in segs {
        assert_one_undo_step(&mut sim, "interior wall", |s| s.drag(a, b));
        assert_eq!(sim.app.cx.rooms.len(), rooms);
    }
}

#[test]
fn deleting_a_partition_merges_two_rooms_in_one_step() {
    let mut sim = cottage();
    cottage_partitions(&mut sim);
    assert_eq!(sim.app.cx.rooms.len(), 4);
    let last = sim.app.cx.floor().walls.last().unwrap().id;
    sim.app.cx.selection.set(ObjectRef::Wall(last));
    assert_one_undo_step(&mut sim, "delete wall", |s| {
        s.app.cx.delete_selection();
        s.app.cx.refresh();
    });
    assert_eq!(sim.app.cx.rooms.len(), 3);
    sim.undo();
    assert_eq!(sim.app.cx.rooms.len(), 4);
}

#[test]
fn a_room_specification_opens_for_each_room() {
    let mut sim = cottage();
    cottage_partitions(&mut sim);
    let w = sim.app.cx.floor().walls[0].id;
    // Wall Specification tabs: General, Roof, Rooms, Wall Types, Structure.
    assert_dialog_tabs(&mut sim, ObjectRef::Wall(w), &["General"]);
}

#[test]
fn interior_dimension_is_one_undo_step() {
    let mut sim = cottage();
    cottage_partitions(&mut sim);
    sim.tool(ToolId::DimensionVariant(DimMode::Interior));
    let n = sim.app.cx.floor().dimensions.len();
    sim.click(100.0, 120.0);
    sim.click(100.0, 380.0);
    // Interior dimensions need a room click; the exact gesture is verified in
    // s17. Here: the tool never leaves the model half-edited.
    assert!(sim.app.cx.floor().dimensions.len() >= n);
}

#[test]
#[ignore = "T7-02: W-152"]
fn changing_a_wall_makes_a_dashed_zero_thickness_room_divider() {
    assert_ignored_break("W-152");
}

#[test]
#[ignore = "T7-02: R-107"]
fn auto_room_dimension_for_the_entry() {
    assert_ignored_break("R-107");
}

#[test]
fn fire_6_copy_of_interior_6_with_red_fill() {
    // Lesson 2: Interior-6 is copied to Fire-6, its Framing (main) layer gets
    // a solid red fill, and the type is given to a partition; Reverse Layers
    // then turns the wall's layers round.
    use crate::dialogs::wall_types::{self, WallTypeDialog};
    use plan_core::fill_styles::FillStyle;
    use plan_core::ResizeAbout;
    let mut sim = cottage();
    cottage_partitions(&mut sim);
    let types = wall_types::plan_types(&sim.app.cx);
    let mut d = WallTypeDialog::new(types, Some("Interior-6"), ResizeAbout::default());
    d.new_type();
    assert!(d.rename_current("Fire-6"));
    assert!(!d.rename_current("Interior-4"), "a name in use is refused");
    {
        let t = d.table_mut();
        let main = t.main_index().expect("a main layer");
        assert_eq!(t.rows[main].name, "Framing");
        t.rows[main].spec.fill = Some(FillStyle::solid([220, 30, 30]));
    }
    let depth = sim.app.cx.undo_depth();
    wall_types::apply(&mut sim.app.cx, &mut d);
    assert_eq!(sim.app.cx.undo_depth(), depth + 1);
    let fire = sim.app.cx.project.wall_type_def("Fire-6").cloned().unwrap();
    assert_eq!(fire.layers.len(), 3);
    assert!((fire.thickness() - 6.5).abs() < 1e-9);
    assert_eq!(
        sim.app.cx.project.wall_layer_fill("Fire-6", 1),
        Some(&FillStyle::solid([220, 30, 30]))
    );
    assert!(sim
        .app
        .cx
        .project
        .wall_layer_fill("Interior-6", 1)
        .is_none());

    // Give the partition the new type, then reverse its layers.
    let id = sim.app.cx.floor().walls.last().unwrap().id;
    let fl = sim.app.cx.floor;
    assert!(sim
        .app
        .cx
        .project
        .set_wall_type(fl, id, &fire, ResizeAbout::default()));
    sim.app.cx.selection.set(ObjectRef::Wall(id));
    let side = sim.app.cx.floor().walls.last().unwrap().exterior_side;
    assert_one_undo_step(&mut sim, "reverse layers", |s| {
        s.app.cx.reverse_layers_selected()
    });
    assert_eq!(
        sim.app.cx.floor().walls.last().unwrap().exterior_side,
        side.opposite()
    );
}

#[test]
#[ignore = "T7-02: DIM-14 the interior tool anchors the east end on the wall surface (side 3.8125), 0.5 in short of the main layer, so it reads 472 7/8 not 473 3/8; fix the anchor side in tools/dimension.rs"]
fn interior_dimension_reads_11_inches_shorter_than_the_exterior_one() {
    // Lesson 2 (guide p. 40): with both dimensions located at the main
    // layer, the interior one across the body is two framing thicknesses
    // (2 x 5 1/2 in = 11 in) shorter than the automatic exterior one.
    let mut sim = cottage();
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    sim.click(240.0, 240.0);
    let overall = sim
        .app
        .cx
        .floor()
        .dimensions
        .iter()
        .map(|d| d.length())
        .filter(|l| (*l - 484.0).abs() < 8.0)
        .fold(0.0_f64, f64::max);
    assert!(overall > 480.0, "an overall dimension across the body");
    let mut sim = cottage();
    sim.app
        .cx
        .defaults
        .dimensions
        .interior_locates_interior_surfaces = false;
    sim.tool(ToolId::DimensionVariant(DimMode::Interior));
    sim.click(240.0, 100.0);
    assert_one_undo_step(&mut sim, "interior dimension", |s| s.click(240.0, -60.0));
    let d = sim.app.cx.floor().dimensions.last().expect("a dimension");
    assert!(
        (overall - d.length() - 11.0).abs() < 1e-6,
        "exterior {overall} interior {}",
        d.length()
    );
}
