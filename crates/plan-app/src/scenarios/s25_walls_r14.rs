//! Scenario 25 (round 14, walls): three- and four-way junctions and crossings
//! drawn with the wall tools, the multi-wall Open Object, Through Wall, the
//! Structure tab's platform intersections in the 3D scene, and the Foundation
//! and Wall Cap tabs (W-30, W-34, W-36, W-39, W-52, W-60, W-62, W-63, W-83,
//! W-103, R-69).

use super::Sim;
use crate::editor::ObjectRef;
use crate::tools::ToolId;
use plan_core::geometry::Point;
use plan_core::walls::{CeilingPlatform, FloorPlatform};
use plan_core::{Id, WallKind};

fn exterior() -> ToolId {
    ToolId::Wall {
        kind: WallKind::Exterior,
    }
}

fn interior() -> ToolId {
    ToolId::Wall {
        kind: WallKind::Interior,
    }
}

fn outline(sim: &Sim, id: Id) -> Vec<Point> {
    sim.app
        .cx
        .outlines
        .iter()
        .find(|o| o.wall_id == id)
        .expect("an outline")
        .polygon
        .clone()
}

fn near(a: Point, x: f64, y: f64) -> bool {
    a.dist(Point::new(x, y)) < 1e-6
}

/// Draws the long wall and, ending on it, a stem; returns (long wall pieces,
/// stem).
fn tee(sim: &mut Sim) -> (Vec<Id>, Id) {
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (240.0, 0.0));
    sim.tool(interior());
    sim.drag((120.0, 150.0), (120.0, 2.0));
    let ids = sim.wall_ids();
    let stem = *ids
        .iter()
        .find(|id| {
            let w = sim.app.cx.floor().wall(**id).unwrap();
            w.kind == WallKind::Interior
        })
        .unwrap();
    (ids.into_iter().filter(|i| *i != stem).collect(), stem)
}

#[test]
fn a_wall_ended_on_another_cuts_it_into_a_three_way_the_stem_butts_the_face() {
    let mut sim = Sim::new();
    let (pieces, stem) = tee(&mut sim);
    assert_eq!(pieces.len(), 2, "the long wall is cut at the tee");
    let sw = sim.app.cx.floor().wall(stem).unwrap().clone();
    let half = sim.app.cx.floor().wall(pieces[0]).unwrap().thickness * 0.5;
    // Both halves keep square ends at the junction.
    for id in &pieces {
        let w = sim.app.cx.floor().wall(*id).unwrap().clone();
        for (a, b) in outline(&sim, *id).iter().zip(w.footprint().iter()) {
            assert!(a.dist(*b) < 1e-6);
        }
    }
    // The stem's junction end sits on the long wall's north face.
    let o = outline(&sim, stem);
    let at_face: Vec<_> = o.iter().filter(|p| (p.y - half).abs() < 1e-6).collect();
    assert_eq!(at_face.len(), 2, "{o:?} (half {half}, stem {sw:?})");
}

#[test]
fn a_crossing_wall_cuts_both_into_a_four_way_where_the_thicker_wall_runs_through() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (240.0, 0.0));
    sim.tool(interior());
    sim.drag((120.0, 100.0), (120.0, -100.0));
    let ids = sim.wall_ids();
    assert_eq!(ids.len(), 4, "an X crossing makes four walls");
    let f = sim.app.cx.floor().clone();
    let x = Point::new(120.0, 0.0);
    assert!(f
        .walls
        .iter()
        .all(|w| w.start.dist(x) < 1e-6 || w.end.dist(x) < 1e-6));
    // The exterior pair runs through; the interior walls stop on its faces.
    let ext_half = f
        .walls
        .iter()
        .find(|w| w.kind == WallKind::Exterior)
        .unwrap()
        .thickness
        * 0.5;
    for w in &f.walls {
        let o = outline(&sim, w.id);
        if w.kind == WallKind::Exterior {
            for (a, b) in o.iter().zip(w.footprint().iter()) {
                assert!(a.dist(*b) < 1e-6, "exterior wall {} stays square", w.id);
            }
        } else {
            let on_face = o
                .iter()
                .filter(|p| (p.y.abs() - ext_half).abs() < 1e-6)
                .count();
            assert_eq!(on_face, 2, "interior wall {} {o:?}", w.id);
        }
    }
    // One undo step takes the crossing wall and both cuts back: the exterior
    // wall is whole again.
    sim.undo();
    assert_eq!(sim.floor_walls(), 1);
}

#[test]
fn a_crossing_stays_whole_when_the_plan_does_not_split_on_tee() {
    let mut sim = Sim::new();
    sim.cx().defaults.walls_connect.split_on_tee = false;
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (240.0, 0.0));
    sim.tool(interior());
    sim.drag((120.0, 100.0), (120.0, -100.0));
    assert_eq!(sim.floor_walls(), 2, "both walls pass through each other");
}

#[test]
fn a_wall_ended_on_a_joined_corner_makes_a_three_way_and_the_pair_keeps_its_join() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (200.0, 0.0));
    sim.drag((200.0, 0.0), (200.0, 150.0));
    let (a, b) = {
        let ids = sim.wall_ids();
        (ids[0], ids[1])
    };
    let corner_before = outline(&sim, a);
    // A third wall from the east ends 3 inches short of the corner.
    sim.drag((400.0, 0.0), (203.0, 0.0));
    let ids = sim.wall_ids();
    assert_eq!(ids.len(), 3);
    let c = ids[2];
    assert!(
        sim.app
            .cx
            .floor()
            .wall(c)
            .unwrap()
            .end
            .dist(Point::new(200.0, 0.0))
            < 1e-6
    );
    // The pair a-b is now a through line a-c with b butting: a's end is square.
    let half = sim.app.cx.floor().wall(a).unwrap().thickness * 0.5;
    let oa = outline(&sim, a);
    assert!(
        near(oa[1], 200.0, half) && near(oa[2], 200.0, -half),
        "{oa:?}"
    );
    assert!(
        !near(corner_before[1], 200.0, half),
        "it was mitered before"
    );
    let ob = outline(&sim, b);
    assert_eq!(
        ob.iter().filter(|p| (p.y - half).abs() < 1e-6).count(),
        2,
        "{ob:?}"
    );
}

#[test]
fn several_walls_open_one_dialog_and_only_the_edited_fields_reach_them() {
    let mut sim = Sim::new();
    sim.tool(interior());
    sim.drag((0.0, 0.0), (120.0, 0.0));
    sim.drag((0.0, 100.0), (120.0, 100.0));
    sim.drag((0.0, 200.0), (120.0, 200.0));
    let ids = sim.wall_ids();
    assert_eq!(ids.len(), 3);
    sim.cx().project.floors[0]
        .wall_mut(ids[1])
        .unwrap()
        .thickness = 6.0;
    sim.cx().project.floors[0]
        .wall_mut(ids[2])
        .unwrap()
        .flags
        .invisible = true;
    sim.cx().refresh();
    sim.tool(ToolId::Select);
    // Select all three and press Open Object.
    for id in &ids {
        sim.cx().selection.add(ObjectRef::Wall(*id));
    }
    assert!(sim.cx().selection.single().is_none());
    assert!(sim
        .cx()
        .common_edit_actions()
        .iter()
        .any(|a| a.kind == crate::editor::actions::EditActionKind::OpenObject));
    sim.cx()
        .apply_edit_action(crate::editor::actions::EditActionKind::OpenObject);
    sim.app.process_requests();
    assert!(sim.app.has_dialog());
    {
        let d = sim
            .app
            .spec
            .walls_dialog_mut()
            .expect("the multi-wall dialog");
        assert_eq!(d.multi_ids().unwrap().len(), 3);
        assert!(d.is_mixed("thickness") && d.is_mixed("invisible"));
        assert!(!d.is_mixed("height"));
        // Edit the Through Wall At End box only.
        d.edit_field("through_end", |w| w.spec.structure.through_at_end = true);
        assert_eq!(d.touched(), vec!["through_end"]);
    }
    let undo_before = sim.app.cx.undo_label().map(str::to_string);
    sim.ok();
    assert!(!sim.app.has_dialog());
    assert_eq!(sim.app.cx.undo_label(), Some("Wall Specification"));
    for id in &ids {
        let w = sim.app.cx.floor().wall(*id).unwrap();
        assert!(w.spec.structure.through_at_end);
    }
    // The thickness and invisible flag were left alone.
    let f = sim.app.cx.floor();
    assert_eq!(f.wall(ids[1]).unwrap().thickness, 6.0);
    assert!(f.wall(ids[2]).unwrap().flags.invisible && !f.wall(ids[0]).unwrap().flags.invisible);
    // One undo step puts all three back.
    sim.undo();
    assert!(sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .all(|w| !w.spec.structure.through_at_end));
    assert_eq!(sim.app.cx.undo_label().map(str::to_string), undo_before);
}

#[test]
fn a_thickness_typed_over_several_walls_reaches_every_wall_in_one_undo_step() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (120.0, 0.0));
    sim.drag((0.0, 100.0), (120.0, 100.0));
    let ids = sim.wall_ids();
    sim.tool(ToolId::Select);
    for id in &ids {
        sim.cx().selection.add(ObjectRef::Wall(*id));
    }
    sim.cx()
        .apply_edit_action(crate::editor::actions::EditActionKind::OpenObject);
    sim.app.process_requests();
    {
        let d = sim.app.spec.walls_dialog_mut().unwrap();
        d.edit_field("thickness", |w| w.thickness = 9.5);
    }
    sim.ok();
    for id in &ids {
        assert_eq!(sim.app.cx.floor().wall(*id).unwrap().thickness, 9.5);
    }
    assert_eq!(sim.app.cx.undo_label(), Some("Wall Specification"));
    sim.undo();
    for id in &ids {
        assert_ne!(sim.app.cx.floor().wall(*id).unwrap().thickness, 9.5);
    }
}

#[test]
fn through_wall_at_end_runs_the_wall_past_the_corner() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (240.0, 0.0));
    sim.drag((240.0, 0.0), (240.0, 150.0));
    let (a, b) = {
        let ids = sim.wall_ids();
        (ids[0], ids[1])
    };
    let wa = sim.app.cx.floor().wall(a).unwrap().clone();
    let half = wa.thickness * 0.5;
    assert!(
        !near(outline(&sim, a)[1], 240.0 + half, half),
        "mitered first"
    );
    assert!(sim.open_spec(ObjectRef::Wall(a)));
    if let Some(crate::ActiveDialog::Wall(d)) = sim.app.dialog.as_mut() {
        d.draft_mut().spec.structure.through_at_end = true;
    } else {
        panic!("the Wall Specification is open");
    }
    sim.ok();
    let oa = outline(&sim, a);
    assert!(
        near(oa[1], 240.0 + half, half) && near(oa[2], 240.0 + half, -half),
        "{oa:?}"
    );
    // The other wall butts a's face.
    let ob = outline(&sim, b);
    assert_eq!(
        ob.iter().filter(|p| (p.y - half).abs() < 1e-6).count(),
        2,
        "{ob:?}"
    );
}

#[test]
fn the_structure_tab_balloon_wall_rises_through_the_floor_platform_in_3d() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (240.0, 0.0));
    let id = sim.wall_ids()[0];
    sim.cx().project.build_new_floor(false);
    sim.cx().refresh();
    let top = |sim: &Sim| {
        let scene = plan_3d::build_scene(&sim.app.cx.project);
        scene
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(id))
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .fold(f32::MIN, f32::max)
    };
    let ceiling = sim.app.cx.project.floors[0].ceiling_height as f32;
    assert!((top(&sim) - ceiling).abs() < 1e-3);
    sim.cx().floor = 0;
    assert!(sim.open_spec(ObjectRef::Wall(id)));
    if let Some(crate::ActiveDialog::Wall(d)) = sim.app.dialog.as_mut() {
        d.draft_mut().spec.structure.ceiling_platform = CeilingPlatform::BalloonThroughCeilingAbove;
    }
    sim.ok();
    let upper = sim.app.cx.project.floors[1].elevation as f32;
    assert!((top(&sim) - upper).abs() < 1e-3, "{} vs {upper}", top(&sim));
    assert_eq!(
        sim.app
            .cx
            .floor()
            .wall(id)
            .unwrap()
            .spec
            .structure
            .ceiling_platform,
        CeilingPlatform::BalloonThroughCeilingAbove
    );
    // Undo restores the stop at the ceiling.
    sim.undo();
    assert!((top(&sim) - ceiling).abs() < 1e-3);
    let _ = FloorPlatform::Automatic;
}

#[test]
fn a_foundation_wall_with_a_footing_and_a_capped_half_wall_build_in_3d() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (240.0, 0.0));
    let id = sim.wall_ids()[0];
    assert!(sim.open_spec(ObjectRef::Wall(id)));
    if let Some(crate::ActiveDialog::Wall(d)) = sim.app.dialog.as_mut() {
        let w = d.draft_mut();
        w.set_class(plan_core::WallClass::Foundation);
        w.spec.foundation.footing = true;
        w.spec.foundation.footing_width = 24.0;
        w.spec.foundation.sill_plate = true;
    }
    sim.ok();
    let scene = plan_3d::build_scene(&sim.app.cx.project);
    let concrete: Vec<_> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == plan_3d::Material::Concrete)
        .collect();
    assert!(!concrete.is_empty(), "the footing is built");
    let lowest = concrete
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .fold(f32::MAX, f32::min);
    let reach = sim.app.cx.floor().wall(id).unwrap().foundation_height as f32;
    assert!(lowest <= -reach - 11.9, "footing bottom {lowest}");
    // The sill plate stands on the stem wall.
    assert!(scene
        .meshes
        .iter()
        .any(|m| m.object_id == Some(id) && m.material == plan_3d::Material::Framing));
}
