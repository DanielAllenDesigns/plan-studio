//! Scenario 60: round 15 architectural blocks, 3D solids, soffits, layered
//! material regions and distributed objects, driven through the real
//! commands, the Select tool, the specification windows and the 3D scene
//! (reference manual pp. 1057 to 1098; CB-427..CB-474).

use super::Sim;
use crate::dialogs::{arch_block, distribution, material_region, soffit, solids};
use crate::editor::{details_view, placed, transform, ObjectRef};
use crate::toolbar::Action;
use crate::tools::{
    arch_block as blocks, distribution as dist_tool, regions, solids as solid_tool, ToolId,
};
use plan_cabinets::{Cabinet, CabinetKind};
use plan_core::details::{DetailsLayer, SolidKind};
use plan_core::distribution::RegionStyle;
use plan_core::geometry::Point;
use plan_core::images::{DistKind, Distribution};
use plan_core::material_region::{LayerRole, MaterialLayer};
use plan_core::Id;

fn sim() -> Sim {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    sim
}

fn cabinet_at(sim: &mut Sim, kind: CabinetKind, x: f64, y: f64) -> Id {
    let mut c = Cabinet::new(kind, 24.0);
    c.position = Point::new(x, y);
    placed::add_cabinet(&mut sim.app.cx.project, 0, c).unwrap()
}

fn cabinet_x(sim: &Sim, id: Id) -> f64 {
    placed::cabinet_by_id(sim.app.cx.floor(), id)
        .unwrap()
        .position
        .x
}

fn center_of(sim: &Sim, id: Id) -> Point {
    let corners = placed::cabinet_by_id(sim.app.cx.floor(), id)
        .unwrap()
        .corners();
    Point::new(
        corners.iter().map(|p| p.x).sum::<f64>() / 4.0,
        corners.iter().map(|p| p.y).sum::<f64>() / 4.0,
    )
}

fn cmd(sim: &mut Sim, id: &'static str) {
    sim.action(Action::Custom(id));
}

fn labels(sim: &Sim) -> Vec<&'static str> {
    sim.app
        .cx
        .extra_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect()
}

fn frames(sim: &mut Sim) {
    let ctx = sim.ctx.clone();
    for _ in 0..2 {
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            arch_block::show_all(ctx, &mut sim.app.cx)
        });
    }
}

fn box_solid(sim: &mut Sim, x: f64, y: f64, side: f64) -> Id {
    details_view::add_solid(
        &mut sim.app.cx,
        SolidKind::Box {
            w: side,
            d: side,
            h: side,
        },
        Point::new(x, y),
    )
}

// ----- architectural blocks -----

#[test]
fn a_block_moves_and_deletes_as_one_and_undoes_in_single_steps() {
    let mut sim = sim();
    let a = cabinet_at(&mut sim, CabinetKind::Base, 0.0, 0.0);
    let b = cabinet_at(&mut sim, CabinetKind::Base, 60.0, 0.0);
    sim.app.cx.selection.items = vec![ObjectRef::Cabinet(a), ObjectRef::Cabinet(b)];
    assert!(labels(&sim).contains(&"Make Architectural Block"));
    cmd(&mut sim, blocks::MAKE_BLOCK);
    assert_eq!(sim.app.cx.floor().blocks.len(), 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Make Architectural Block"));
    assert!(labels(&sim).contains(&"Explode Architectural Block"));
    assert!(
        !labels(&sim).contains(&"Make Architectural Block"),
        "already a block"
    );

    // A click on one cabinet selects the whole block.
    sim.app.cx.selection.items.clear();
    let c = center_of(&sim, a);
    sim.click(c.x, c.y);
    assert_eq!(sim.app.cx.selection.len(), 2);

    // Moving it moves both members.
    let items = sim.app.cx.selection.items.clone();
    sim.app.cx.begin_change("Move");
    transform::translate_objects(&mut sim.app.cx, &items, Point::new(100.0, 0.0));
    assert_eq!(cabinet_x(&sim, a), 100.0);
    assert_eq!(cabinet_x(&sim, b), 160.0);
    let id = sim.app.cx.floor().blocks.blocks[0].id;
    let (lo, hi) = blocks::block_bounds(&sim.app.cx, id).unwrap();
    assert!(
        lo.x >= 99.0 && hi.x >= 183.0,
        "the box follows: {lo:?} {hi:?}"
    );

    // Delete takes the block with its members; one undo brings it all back.
    sim.app.cx.delete_selection();
    assert!(sim.app.cx.floor().blocks.is_empty());
    assert!(placed::cabinet_by_id(sim.app.cx.floor(), a).is_none());
    sim.undo();
    assert_eq!(sim.app.cx.floor().blocks.len(), 1);
    assert!(placed::cabinet_by_id(sim.app.cx.floor(), b).is_some());
}

#[test]
fn explode_keeps_the_members_and_edit_sub_objects_picks_one() {
    let mut sim = sim();
    let a = cabinet_at(&mut sim, CabinetKind::Base, 0.0, 0.0);
    let b = cabinet_at(&mut sim, CabinetKind::Wall, 60.0, 0.0);
    sim.app.cx.selection.items = vec![ObjectRef::Cabinet(a), ObjectRef::Cabinet(b)];
    cmd(&mut sim, blocks::MAKE_BLOCK);
    cmd(&mut sim, blocks::EDIT_SUB_OBJECTS);
    assert!(blocks::editing_sub_objects());
    sim.app.cx.selection.items.clear();
    let c = center_of(&sim, a);
    sim.click(c.x, c.y);
    assert_eq!(sim.app.cx.selection.items, vec![ObjectRef::Cabinet(a)]);
    cmd(&mut sim, blocks::EDIT_SUB_OBJECTS);
    assert!(!blocks::editing_sub_objects());

    sim.app.cx.selection.items = vec![ObjectRef::Cabinet(a)];
    cmd(&mut sim, blocks::EXPLODE_BLOCK);
    assert!(sim.app.cx.floor().blocks.is_empty());
    assert!(placed::cabinet_by_id(sim.app.cx.floor(), a).is_some());
    assert_eq!(sim.app.cx.undo_label(), Some("Explode Architectural Block"));
}

#[test]
fn blocks_nest_and_the_specification_window_edits_a_block() {
    let mut sim = sim();
    let a = cabinet_at(&mut sim, CabinetKind::Base, 0.0, 0.0);
    let b = cabinet_at(&mut sim, CabinetKind::Base, 30.0, 0.0);
    let c = cabinet_at(&mut sim, CabinetKind::Base, 60.0, 0.0);
    sim.app.cx.selection.items = vec![ObjectRef::Cabinet(a), ObjectRef::Cabinet(b)];
    cmd(&mut sim, blocks::MAKE_BLOCK);
    sim.app.cx.selection.items = vec![
        ObjectRef::Cabinet(a),
        ObjectRef::Cabinet(b),
        ObjectRef::Cabinet(c),
    ];
    cmd(&mut sim, blocks::MAKE_BLOCK);
    let floor = sim.app.cx.floor();
    assert_eq!(floor.blocks.len(), 2);
    let outer_id = floor
        .blocks
        .blocks
        .iter()
        .find(|b| b.members.len() == 2)
        .unwrap()
        .id;
    assert_eq!(
        floor.blocks.depth_of(outer_id),
        1,
        "the first block sits inside the second"
    );

    arch_block::close();
    sim.app.cx.selection.items = vec![ObjectRef::Cabinet(a)];
    cmd(&mut sim, blocks::BLOCK_SPEC);
    assert!(arch_block::is_open());
    frames(&mut sim);
    assert!(arch_block::is_open());
    arch_block::close();

    let id = outer_id;
    let mut d = arch_block::ArchBlockDialog::new(&sim.app.cx, id).unwrap();
    d.draft_mut().name = "Island".into();
    assert!(d.apply(&mut sim.app.cx));
    assert_eq!(sim.app.cx.floor().blocks.get(id).unwrap().name, "Island");
}

#[test]
fn two_outlets_make_a_ganged_electrical_block_and_a_cabinet_cannot_join_them() {
    use crate::editor::site_view;
    use crate::tools::electrical::{placement, ElecVariant};
    let mut sim = sim();
    let kind = ElecVariant::Outlet110.kind().unwrap();
    sim.app.cx.project.add_wall(
        0,
        Point::new(0.0, 0.0),
        Point::new(300.0, 0.0),
        6.0,
        96.0,
        plan_core::WallKind::Interior,
    );
    sim.app.cx.refresh();
    let mut ids = Vec::new();
    for x in [100.0, 106.0] {
        let dev = placement(&sim.app.cx, kind, Point::new(x, 0.0), Point::new(x, 0.0)).unwrap();
        let mut id = 0;
        site_view::edit_electrical(&mut sim.app.cx, "Place", |layer, _| id = layer.add(dev));
        ids.push(id);
    }
    let cab = cabinet_at(&mut sim, CabinetKind::Base, 0.0, 0.0);
    sim.app.cx.selection.items = ids.iter().map(|i| ObjectRef::Device(*i)).collect();
    assert!(labels(&sim).contains(&"Make Ganged Electrical Block"));
    assert!(!labels(&sim).contains(&"Make Architectural Block"));
    cmd(&mut sim, blocks::MAKE_GANGED);
    let b = sim.app.cx.floor().blocks.blocks[0].clone();
    assert_eq!(b.kind, plan_core::arch_block::BlockKind::GangedElectrical);
    assert!(b.treat_as_one, "a ganged block is one object");
    // A device with a cabinet is refused.
    sim.app.cx.selection.items = vec![ObjectRef::Device(ids[0]), ObjectRef::Cabinet(cab)];
    assert!(
        blocks::make_block(&mut sim.app.cx, plan_core::arch_block::BlockKind::Standard).is_err()
    );
}

#[test]
fn walls_cannot_join_a_block_and_a_refusal_leaves_no_undo_step() {
    let mut sim = sim();
    let a = cabinet_at(&mut sim, CabinetKind::Base, 0.0, 0.0);
    let w = sim.app.cx.project.add_wall(
        0,
        Point::new(0.0, -10.0),
        Point::new(100.0, -10.0),
        6.0,
        96.0,
        plan_core::WallKind::Interior,
    );
    sim.app.cx.selection.items = vec![ObjectRef::Cabinet(a), ObjectRef::Wall(w)];
    assert!(!labels(&sim).contains(&"Make Architectural Block"));
    assert!(
        blocks::make_block(&mut sim.app.cx, plan_core::arch_block::BlockKind::Standard).is_err()
    );
    assert!(sim.app.cx.floor().blocks.is_empty());
}

// ----- 3D solids -----

#[test]
fn boolean_solids_pick_move_rotate_and_show_in_3d() {
    let mut sim = sim();
    let a = box_solid(&mut sim, 0.0, 0.0, 24.0);
    let b = box_solid(&mut sim, 12.0, 0.0, 24.0);
    sim.app.cx.selection.items = vec![ObjectRef::Detail(a), ObjectRef::Detail(b)];
    assert!(labels(&sim).contains(&"Union Solids"));
    let before = plan_3d::details::detail_meshes(&sim.app.cx.project).len();
    cmd(&mut sim, solid_tool::UNION);
    let id = match sim.app.cx.selection.items.as_slice() {
        [ObjectRef::Solid(id)] => *id,
        other => panic!("the result is selected: {other:?}"),
    };
    assert_eq!(sim.app.cx.undo_label(), Some("Union Solids"));
    // One mesh for the union instead of two for the boxes.
    let meshes = plan_3d::details::detail_meshes(&sim.app.cx.project);
    assert_eq!(meshes.len() + 1, before);

    // The Select tool picks it by its plan outline and moves it.
    sim.app.cx.selection.items.clear();
    sim.click(0.0, 0.0);
    assert_eq!(sim.app.cx.selection.items, vec![ObjectRef::Solid(id)]);
    let x0 = sim
        .app
        .cx
        .floor()
        .solid_layer
        .compound(id)
        .unwrap()
        .plan_bounds()
        .unwrap()
        .0
        .x;
    let items = sim.app.cx.selection.items.clone();
    sim.app.cx.begin_change("Move");
    transform::translate_objects(&mut sim.app.cx, &items, Point::new(50.0, 0.0));
    let x1 = sim
        .app
        .cx
        .floor()
        .solid_layer
        .compound(id)
        .unwrap()
        .plan_bounds()
        .unwrap()
        .0
        .x;
    assert!((x1 - x0 - 50.0).abs() < 1e-6);

    // Rotate 90 degrees: the long plan side becomes the tall one.
    let (lo, hi) = sim
        .app
        .cx
        .floor()
        .solid_layer
        .compound(id)
        .unwrap()
        .plan_bounds()
        .unwrap();
    let (w0, h0) = (hi.x - lo.x, hi.y - lo.y);
    let rep = transform::rotate_selection(&mut sim.app.cx, std::f64::consts::FRAC_PI_2, None);
    assert_eq!(rep.changed, 1);
    let (lo, hi) = sim
        .app
        .cx
        .floor()
        .solid_layer
        .compound(id)
        .unwrap()
        .plan_bounds()
        .unwrap();
    assert!(((hi.x - lo.x) - h0).abs() < 1e-6 && ((hi.y - lo.y) - w0).abs() < 1e-6);

    // Its specification opens from the spec route.
    solids::close();
    assert!(sim.app.cx.floor().solid_layer.compound(id).is_some());
    sim.app
        .cx
        .requests
        .push(crate::editor::EditorRequest::OpenSpec(ObjectRef::Solid(id)));
    sim.app.process_requests();
    assert!(solids::is_open());
    frames(&mut sim);
    solids::close();

    // Delete is one undo step.
    sim.app.cx.selection.items = vec![ObjectRef::Solid(id)];
    sim.app.cx.delete_selection();
    assert!(sim.app.cx.floor().solid_layer.compounds.is_empty());
    sim.undo();
    assert_eq!(sim.app.cx.floor().solid_layer.compounds.len(), 1);
}

#[test]
fn subtract_and_intersect_need_overlap_and_a_polyline_converts_to_a_solid() {
    let mut sim = sim();
    let a = box_solid(&mut sim, 0.0, 0.0, 24.0);
    let far = box_solid(&mut sim, 500.0, 0.0, 24.0);
    sim.app.cx.selection.items = vec![ObjectRef::Detail(a), ObjectRef::Detail(far)];
    cmd(&mut sim, solid_tool::INTERSECT);
    assert!(
        sim.app.cx.floor().solid_layer.compounds.is_empty(),
        "nothing is left"
    );
    assert_eq!(
        DetailsLayer::load(sim.app.cx.floor()).solids.len(),
        2,
        "the solids stay"
    );

    let cad = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        plan_core::cad::CadItem::Polyline {
            points: vec![
                Point::new(200.0, 200.0),
                Point::new(260.0, 200.0),
                Point::new(260.0, 240.0),
                Point::new(200.0, 240.0),
            ],
            closed: true,
        },
    );
    sim.app.cx.selection.items = vec![ObjectRef::Cad(cad)];
    assert!(labels(&sim).contains(&"Convert Polyline to Solid"));
    assert!(labels(&sim).contains(&"Convert Polyline to Material Region"));
    cmd(&mut sim, solid_tool::FROM_POLYLINE);
    assert_eq!(DetailsLayer::load(sim.app.cx.floor()).solids.len(), 3);
    assert!(sim.app.cx.floor().cad.is_empty());
}

#[test]
fn solid_options_tilt_a_pyramid_and_the_3d_mesh_changes() {
    let mut sim = sim();
    let id = details_view::add_solid(
        &mut sim.app.cx,
        SolidKind::Pyramid {
            outline: plan_core::solids::PyramidSpec::default().outline(),
            h: 36.0,
        },
        Point::new(0.0, 0.0),
    );
    sim.app.cx.selection.items = vec![ObjectRef::Detail(id)];
    assert!(labels(&sim).contains(&"3D Solid Options"));
    let before = plan_3d::details::detail_meshes(&sim.app.cx.project);
    let top = |m: &[plan_3d::Mesh]| {
        m.iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .fold(f32::MIN, f32::max)
    };
    let h0 = top(&before);
    let mut d = solids::SolidDialog::new(&sim.app.cx, plan_core::ObjectRef::Detail(id)).unwrap();
    if let solids::Draft::Primitive { pyramid, .. } = d.draft_mut() {
        let p = pyramid.as_mut().unwrap();
        p.truncated = true;
        p.truncated_height = 12.0;
    }
    assert!(d.apply(&mut sim.app.cx));
    let after = plan_3d::details::detail_meshes(&sim.app.cx.project);
    assert!(
        top(&after) < h0,
        "the truncated pyramid is lower: {} vs {h0}",
        top(&after)
    );
    sim.undo();
    let again = plan_3d::details::detail_meshes(&sim.app.cx.project);
    assert!((top(&again) - h0).abs() < 1e-4);
}

// ----- soffits -----

#[test]
fn the_soffit_specification_hangs_a_soffit_from_the_ceiling() {
    let mut sim = sim();
    let id = cabinet_at(&mut sim, CabinetKind::Soffit, 0.0, 0.0);
    sim.app.cx.selection.items = vec![ObjectRef::Cabinet(id)];
    assert!(labels(&sim).contains(&"Soffit Specification"));
    soffit::close();
    cmd(&mut sim, soffit::SOFFIT_SPEC);
    assert!(soffit::is_open());
    frames(&mut sim);
    soffit::close();
    let ceiling = sim.app.cx.floor().ceiling_height;
    let mut d = soffit::SoffitDialog::new(&sim.app.cx, id).unwrap();
    d.draft_mut().height = 12.0;
    d.draft_mut().elevation = ceiling - 12.0;
    assert!(d.apply(&mut sim.app.cx));
    let c = placed::cabinet_by_id(sim.app.cx.floor(), id).unwrap();
    assert!((c.elevation + c.height - ceiling).abs() < 1e-9);
}

// ----- material regions -----

#[test]
fn a_layered_region_draws_one_slab_per_solid_layer_and_feeds_the_materials_list() {
    let mut sim = sim();
    let id = details_view::add_floor_region(
        &mut sim.app.cx,
        vec![
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(120.0, 96.0),
            Point::new(0.0, 96.0),
        ],
    );
    sim.app.cx.selection.items = vec![ObjectRef::Detail(id)];
    assert!(labels(&sim).contains(&"Material Layers"));
    material_region::close();
    cmd(&mut sim, regions::LAYERS);
    assert!(material_region::is_open());
    frames(&mut sim);
    material_region::close();

    let one = plan_3d::details::detail_meshes(&sim.app.cx.project).len();
    let mut d = material_region::MaterialLayersDialog::new(&sim.app.cx, id).unwrap();
    {
        let s = d.structure_mut();
        s.layers[0] = MaterialLayer::new("Oak Flooring", 0.75);
        s.layers.push(MaterialLayer {
            role: LayerRole::AirGap,
            thickness: 1.0,
            ..MaterialLayer::new("Air", 1.0)
        });
        s.layers.push(MaterialLayer::new("Plywood", 0.5));
    }
    d.set_cut_finish_layers(true);
    assert!(d.apply(&mut sim.app.cx));
    let three = plan_3d::details::detail_meshes(&sim.app.cx.project).len();
    assert_eq!(
        three,
        one + 1,
        "two solid layers draw, the air gap does not"
    );

    let lines = plan_docs::materials::materials_list(&sim.app.cx.project, 0, &[]);
    let rows: Vec<_> = lines
        .iter()
        .filter(|l| l.item.contains("region layer"))
        .collect();
    assert_eq!(rows.len(), 2, "{rows:?}");
    let area_sq_ft = 120.0 * 96.0 / 144.0;
    assert!(
        rows.iter().all(|r| (r.net - area_sq_ft).abs() < 0.01),
        "{rows:?}"
    );

    sim.undo();
    assert!(sim.app.cx.floor().region_structure(id).is_none());
}

// ----- distributed objects -----

#[test]
fn distribution_options_respace_the_copies_and_explode_releases_them() {
    let mut sim = sim();
    let mut d = Distribution::new(
        DistKind::Region,
        false,
        vec![
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            Point::new(0.0, 120.0),
        ],
        "Shrub",
        [12.0, 12.0, 24.0],
    );
    d.spacing = 30.0;
    let rec = sim.app.cx.project.add_distribution(0, d);
    let n0 = sim.app.cx.project.distribution_copies(0, rec);

    sim.app.cx.selection.items = vec![ObjectRef::Symbol(rec)];
    assert!(labels(&sim).contains(&"Distribution Options"));
    distribution::close();
    cmd(&mut sim, dist_tool::OPTIONS);
    assert!(distribution::is_open());
    frames(&mut sim);
    distribution::close();

    let mut dd = distribution::DistributionDialog::new(&sim.app.cx, rec).unwrap();
    dd.options_mut().region_style = RegionStyle::EvenlyScattered;
    dd.set_spacing(20.0);
    assert!(dd.apply(&mut sim.app.cx));
    let n1 = sim.app.cx.project.distribution_copies(0, rec);
    assert!(n1 > n0, "{n0} to {n1}");

    cmd(&mut sim, dist_tool::EXPLODE);
    assert!(sim.app.cx.floor().symbol(rec).is_none());
    assert_eq!(sim.app.cx.floor().symbols.len(), n1);
    sim.undo();
    assert_eq!(sim.app.cx.project.distribution_copies(0, rec), n1);
}

// ----- saved files -----

#[test]
fn blocks_compound_solids_layers_and_options_survive_save_and_load() {
    let mut sim = sim();
    let a = cabinet_at(&mut sim, CabinetKind::Base, 0.0, 0.0);
    let b = cabinet_at(&mut sim, CabinetKind::Base, 30.0, 0.0);
    sim.app.cx.selection.items = vec![ObjectRef::Cabinet(a), ObjectRef::Cabinet(b)];
    cmd(&mut sim, blocks::MAKE_BLOCK);
    let s1 = box_solid(&mut sim, 200.0, 0.0, 24.0);
    let s2 = box_solid(&mut sim, 212.0, 0.0, 24.0);
    sim.app.cx.selection.items = vec![ObjectRef::Detail(s1), ObjectRef::Detail(s2)];
    cmd(&mut sim, solid_tool::UNION);
    let region = details_view::add_floor_region(
        &mut sim.app.cx,
        vec![
            Point::new(0.0, 100.0),
            Point::new(60.0, 100.0),
            Point::new(60.0, 160.0),
            Point::new(0.0, 160.0),
        ],
    );
    let mut d = material_region::MaterialLayersDialog::new(&sim.app.cx, region).unwrap();
    d.structure_mut()
        .layers
        .push(MaterialLayer::new("Plywood", 0.5));
    d.apply(&mut sim.app.cx);

    let json = sim.app.cx.project.to_json().unwrap();
    let loaded = plan_core::Project::from_json(&json).unwrap();
    assert_eq!(loaded.floors[0].blocks, sim.app.cx.floor().blocks);
    assert_eq!(loaded.floors[0].solid_layer, sim.app.cx.floor().solid_layer);
    assert_eq!(
        loaded.floors[0].region_layers,
        sim.app.cx.floor().region_layers
    );
    assert_eq!(
        loaded.to_json().unwrap(),
        json,
        "a second save is byte identical"
    );
}

#[test]
fn every_new_command_is_safe_with_nothing_selected() {
    let mut sim = sim();
    for id in [
        blocks::MAKE_BLOCK,
        blocks::MAKE_GANGED,
        blocks::EXPLODE_BLOCK,
        blocks::BLOCK_SPEC,
        solid_tool::UNION,
        solid_tool::SUBTRACT,
        solid_tool::INTERSECT,
        solid_tool::FROM_POLYLINE,
        solid_tool::OPTIONS,
        regions::FROM_POLYLINE,
        regions::LAYERS,
        dist_tool::OPTIONS,
        dist_tool::EXPLODE,
        soffit::SOFFIT_SPEC,
    ] {
        cmd(&mut sim, id);
        assert!(
            sim.app.cx.can_undo() == false,
            "{id} must not leave an undo step"
        );
    }
}
