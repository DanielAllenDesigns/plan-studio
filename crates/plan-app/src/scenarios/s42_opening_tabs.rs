//! Scenario 42: the Door and Window Specification tabs of round 15. Each tab
//! is drawn, a field is edited, OK stores it with the opening in one undo
//! step, and the plan, the 3D scene, the framing or the schedule data follow
//! (DW-35, DW-56, DW-114..DW-118, DW-121, DW-123, Casing > Curved Wall
//! Casing).

use super::{draw_shell, Sim};
use crate::dialogs::OpeningDialog;
use crate::editor::ObjectRef;
use crate::tools::ToolId;
use crate::ActiveDialog;
use eframe::egui;
use plan_3d::{build_scene_with, Material, Mesh, SceneOptions};
use plan_core::geometry::Point;
use plan_core::opening_symbol::{plan_symbol, PartKind};
use plan_core::openings::spec::{
    BlindStyle, CurtainStyle, CurvedCasing, DoorSwing, HeaderMaterial, MillworkStyle, RoughMode,
    ShapeKind, WindowShape,
};
use plan_core::schedules::{Schedule, ScheduleKind};
use plan_core::{Id, Opening, OpeningKind, OpeningStyle};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn opening(sim: &Sim, id: Id) -> Opening {
    sim.app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == id)
        .expect("the opening")
        .clone()
}

/// Places a door or window on the south wall at `x` and returns its id.
fn place(sim: &mut Sim, tool: ToolId, x: f64) -> Id {
    sim.tool(tool);
    sim.click(x, 0.5);
    sim.esc();
    sim.app
        .cx
        .floor()
        .openings
        .iter()
        .max_by_key(|o| o.id)
        .map(|o| o.id)
        .expect("placed")
}

/// Opens the specification of `id`, draws each of `tabs` (so the page code
/// runs), lets `edit` change the dialog and presses OK.
fn spec(sim: &mut Sim, id: Id, tabs: &[&str], edit: impl FnOnce(&mut OpeningDialog)) {
    assert!(sim.open_spec(ObjectRef::Opening(id)));
    let ctx = egui::Context::default();
    {
        let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() else {
            panic!("no opening dialog");
        };
        for tab in tabs {
            assert!(d.draw_tab_for_test(&ctx, tab), "no live {tab} tab");
        }
        edit(d);
        d.draw_tab_for_test(&ctx, "General");
    }
    sim.ok();
    assert!(!sim.app.has_dialog(), "OK closes the dialog");
    assert_eq!(sim.app.cx.undo_label(), Some("Opening Specification"));
}

fn scene_meshes(sim: &Sim, id: Id) -> Vec<Mesh> {
    let opts = SceneOptions {
        show_casing: true,
        ..Default::default()
    };
    build_scene_with(&sim.app.cx.project, &opts)
        .meshes
        .into_iter()
        .filter(|m| m.object_id == Some(id))
        .collect()
}

fn symbol(sim: &Sim, id: Id) -> plan_core::opening_symbol::OpeningSymbol {
    let o = opening(sim, id);
    let wall = sim.app.cx.floor().wall(o.wall_id).unwrap().clone();
    plan_symbol(&wall, &o, -1.0)
}

#[test]
fn every_new_tab_is_live_for_both_kinds() {
    let mut sim = house();
    let door = place(&mut sim, ToolId::Door, 120.0);
    let win = place(&mut sim, ToolId::Window, 300.0);
    let ctx = egui::Context::default();
    for (id, tabs) in [
        (
            door,
            &[
                "Rough Opening",
                "Framing",
                "Energy Values",
                "Layer",
                "Materials",
                "Object Information",
            ][..],
        ),
        (
            win,
            &[
                "Rough Opening",
                "Framing",
                "Energy Values",
                "Layer",
                "Materials",
                "Object Information",
                "Shape",
                "Treatments",
            ][..],
        ),
    ] {
        sim.open_spec(ObjectRef::Opening(id));
        let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() else {
            panic!("no dialog");
        };
        for tab in tabs {
            assert!(d.draw_tab_for_test(&ctx, tab), "{tab}");
        }
        // A door has no Shape or Treatments tab.
        if id == door {
            assert!(!d.draw_tab_for_test(&ctx, "Shape"));
            assert!(!d.draw_tab_for_test(&ctx, "Treatments"));
        }
        sim.cancel();
    }
}

#[test]
fn rough_opening_edits_the_model_the_plan_and_the_framing_and_undoes() {
    let mut sim = house();
    let id = place(&mut sim, ToolId::Door, 120.0);
    let plain = symbol(&sim, id).parts.len();
    spec(&mut sim, id, &["Rough Opening"], |d| {
        let r = &mut d.draft_mut().extras.spec.rough;
        r.mode = RoughMode::AdditionalSpace;
        r.add_width = 4.0;
        r.add_height = 2.5;
        r.show_in_plan = true;
    });
    let o = opening(&sim, id);
    assert_eq!(o.rough_width(), o.width + 4.0);
    assert_eq!(o.rough_height(), o.height + 2.5);
    assert_eq!(o.header_bottom(), o.height + 2.5);
    // The plan shows the rough opening as two dashed lines.
    assert_eq!(symbol(&sim, id).parts.len(), plain + 2);
    // The framing stands around it: the header is 4" longer than before.
    let wall = sim.app.cx.floor().wall(o.wall_id).unwrap().clone();
    let defaults = plan_framing::FramingDefaults::default();
    let header_len = |o: &Opening| {
        plan_framing::frame_wall(&wall, &[o], 0.0, &defaults)
            .iter()
            .find(|m| m.kind == plan_framing::MemberKind::Header)
            .map(|m| m.length)
            .unwrap()
    };
    let mut bare = o.clone();
    bare.extras.spec.rough = Default::default();
    assert!((header_len(&o) - header_len(&bare) - 4.0).abs() < 1e-9);
    // One undo step puts it all back.
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    let o = opening(&sim, id);
    assert_eq!(o.rough_width(), o.width);
    assert_eq!(symbol(&sim, id).parts.len(), plain);
}

#[test]
fn framing_tab_overrides_reach_plan_framing() {
    let mut sim = house();
    let id = place(&mut sim, ToolId::Window, 300.0);
    spec(&mut sim, id, &["Framing"], |d| {
        let f = &mut d.draft_mut().extras.spec.framing;
        f.header_plies = Some(3);
        f.header_depth = Some(9.25);
        f.header_material = HeaderMaterial::Lvl;
        f.trimmers = Some(2);
        f.king_studs = Some(2);
        f.sill = false;
    });
    let o = opening(&sim, id);
    let wall = sim.app.cx.floor().wall(o.wall_id).unwrap().clone();
    let m = plan_framing::frame_wall(&wall, &[&o], 0.0, &plan_framing::FramingDefaults::default());
    let count = |k| m.iter().filter(|x| x.kind == k).count();
    use plan_framing::MemberKind as K;
    assert_eq!(count(K::Header), 3);
    assert_eq!(count(K::TrimmerStud), 4);
    assert_eq!(count(K::KingStud), 4);
    assert_eq!(count(K::Sill), 0);
    assert!(m
        .iter()
        .filter(|x| x.kind == K::Header)
        .all(|h| h.lumber.depth == 9.25 && (h.lumber.thickness - 1.75).abs() < 1e-9));
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    let o = opening(&sim, id);
    assert_eq!(o.extras.spec.framing, Default::default());
}

#[test]
fn energy_layer_and_object_information_are_stored_and_undone() {
    let mut sim = house();
    let id = place(&mut sim, ToolId::Window, 300.0);
    spec(
        &mut sim,
        id,
        &["Energy Values", "Layer", "Object Information"],
        |d| {
            let s = &mut d.draft_mut().extras.spec;
            s.energy.construction = "Triple Pane Low-E".into();
            s.energy.u_factor = 0.2;
            s.energy.shgc = 0.25;
            s.layer = Some("Windows, Labels".into());
            s.info.id = "W-1".into();
            s.info.description = "Kitchen window".into();
            s.schedule.manufacturer = "Therma".into();
            s.schedule.comment = "Egress".into();
        },
    );
    let o = opening(&sim, id);
    let s = &o.extras.spec;
    assert_eq!((s.energy.u_factor, s.energy.shgc), (0.2, 0.25));
    assert_eq!(o.layer_name(), "Windows, Labels");
    assert_eq!(s.info.id, "W-1");
    assert_eq!(s.schedule.manufacturer, "Therma");
    assert_eq!(s.schedule.comment, "Egress");
    // The window schedule lists them.
    let mut def = Schedule::new(ScheduleKind::Window, Point::ZERO);
    for field in [
        "u_factor",
        "shgc",
        "description",
        "object_id",
        "manufacturer",
        "rough",
    ] {
        def.columns
            .iter_mut()
            .find(|c| c.field == field)
            .unwrap_or_else(|| panic!("no {field} column"))
            .visible = true;
    }
    let t = plan_docs::schedule_kinds::table(&sim.app.cx.project, &def, 0, None);
    assert_eq!(t.rows.len(), 1, "{:?}", t.rows);
    for want in ["0.20", "0.25", "Kitchen window", "W-1", "Therma"] {
        assert!(
            t.rows[0].iter().any(|c| c == want),
            "{want}: {:?}",
            t.rows[0]
        );
    }
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    let o = opening(&sim, id);
    assert_eq!(o.layer_name(), "Windows");
    assert_eq!(o.extras.spec.energy.u_factor, 0.30);
    assert!(o.extras.spec.info.id.is_empty());
}

#[test]
fn materials_paint_the_components_in_3d_and_reach_the_project() {
    let mut sim = house();
    let id = place(&mut sim, ToolId::Window, 300.0);
    spec(&mut sim, id, &["Materials"], |d| {
        let m = &mut d.draft_mut().extras.spec.materials;
        m.set("Frame", Some(("Painted White Trim", [11, 22, 33])));
        m.set("Sash", Some(("Fir Framing", [44, 55, 66])));
        m.set("Glass", Some(("Clear Glass", [77, 88, 99])));
        m.set("Casing", Some(("Oak Flooring", [111, 122, 133])));
    });
    let meshes = scene_meshes(&sim, id);
    for rgb in [[11, 22, 33], [44, 55, 66], [77, 88, 99], [111, 122, 133]] {
        assert!(meshes.iter().any(|m| m.color == Some(rgb)), "{rgb:?}");
    }
    assert!(meshes
        .iter()
        .filter(|m| m.color == Some([77, 88, 99]))
        .all(|m| m.material == Material::WindowGlass));
    // The Material Painter's per-object paint sees the same materials.
    assert!(sim.app.cx.project.sync_opening_materials(id));
    assert_eq!(
        sim.app.cx.project.object_material(id, "Sash"),
        Some("Fir Framing")
    );
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    assert!(opening(&sim, id).extras.spec.materials.parts.is_empty());
    assert!(scene_meshes(&sim, id).iter().all(|m| m.color.is_none()));
}

#[test]
fn a_window_shape_changes_the_3d_glazing_and_the_plan_head_marks() {
    let mut sim = house();
    let id = place(&mut sim, ToolId::Window, 300.0);
    let area_of = |sim: &Sim| -> usize {
        scene_meshes(sim, id)
            .iter()
            .filter(|m| m.material == Material::WindowGlass)
            .map(Mesh::triangle_count)
            .sum()
    };
    let plain_glass = area_of(&sim);
    let marks = symbol(&sim, id).count(PartKind::Hidden);
    spec(&mut sim, id, &["Shape"], |d| {
        let (w, h) = (d.draft().width, d.draft().height);
        d.draft_mut().extras.spec.shape = WindowShape::preset(ShapeKind::HalfRound, w, h);
        d.draft_mut().lites = (2, 2);
    });
    let o = opening(&sim, id);
    assert!(o.is_shaped());
    // The curved head is glazed with more triangles than the rectangle.
    assert!(area_of(&sim) > plain_glass);
    // The wall fills the corners the curve leaves.
    assert!(scene_meshes(&sim, id)
        .iter()
        .any(|m| m.material == Material::WallExterior));
    // The plan marks the head, like an arch.
    assert_eq!(symbol(&sim, id).count(PartKind::Hidden), marks + 2);
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    assert!(!opening(&sim, id).is_shaped());
    assert_eq!(area_of(&sim), plain_glass);
}

#[test]
fn treatments_are_built_in_3d_and_left_out_of_the_plan() {
    let mut sim = house();
    let id = place(&mut sim, ToolId::Window, 300.0);
    // OK on the dialog stores its frame width too, so compare from there.
    spec(&mut sim, id, &["Treatments"], |_| {});
    let before_plan = symbol(&sim, id);
    let before = scene_meshes(&sim, id).len();
    let tris_before: usize = scene_meshes(&sim, id)
        .iter()
        .map(Mesh::triangle_count)
        .sum();
    spec(&mut sim, id, &["Treatments"], |d| {
        let t = &mut d.draft_mut().extras.spec.treatments;
        t.curtain = CurtainStyle::Panels;
        t.curtain_color = [150, 20, 20];
        t.blind = BlindStyle::Roller;
        t.blind_color = [20, 150, 20];
        t.millwork_above = MillworkStyle::Header;
    });
    let meshes = scene_meshes(&sim, id);
    // The curtains and the blind are colored meshes of their own; the
    // millwork joins the plain trim.
    assert!(meshes.len() >= before + 2);
    assert!(meshes.iter().map(Mesh::triangle_count).sum::<usize>() > tris_before + 50);
    assert!(meshes.iter().any(|m| m.color == Some([150, 20, 20])));
    assert!(meshes.iter().any(|m| m.color == Some([20, 150, 20])));
    // The plan omits them.
    assert_eq!(symbol(&sim, id), before_plan);
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    assert_eq!(scene_meshes(&sim, id).len(), before);
}

#[test]
fn a_double_door_swings_one_leaf_or_from_the_center() {
    let mut sim = house();
    let id = place(&mut sim, ToolId::Door, 240.0);
    spec(&mut sim, id, &["Options"], |d| {
        d.draft_mut().style = OpeningStyle::DoubleDoor;
        d.draft_mut().width = 60.0;
    });
    assert_eq!(symbol(&sim, id).count(PartKind::Swing), 2);
    sim.undo();
    spec(&mut sim, id, &["Options"], |d| {
        d.draft_mut().style = OpeningStyle::DoubleDoor;
        d.draft_mut().width = 60.0;
        d.draft_mut().extras.spec.door_swing = DoorSwing::LeftOnly;
    });
    assert_eq!(symbol(&sim, id).count(PartKind::Swing), 1);
    assert_eq!(symbol(&sim, id).count(PartKind::Leaf), 2);
    sim.undo();
    spec(&mut sim, id, &["Options"], |d| {
        d.draft_mut().style = OpeningStyle::DoubleDoor;
        d.draft_mut().width = 60.0;
        d.draft_mut().extras.spec.swings_from_center = true;
    });
    let o = opening(&sim, id);
    let leaves: Vec<_> = symbol(&sim, id).of(PartKind::Leaf).cloned().collect();
    assert_eq!(leaves.len(), 2);
    let mid = o.center_offset;
    let wall_start = sim.app.cx.floor().wall(o.wall_id).unwrap().start;
    for l in leaves {
        assert!(
            (l.points[0].x - wall_start.x - mid).abs() < 1e-6,
            "both hinges stand in the middle"
        );
    }
}

#[test]
fn the_casing_tab_chooses_how_curved_wall_casing_is_laid() {
    let mut sim = house();
    let id = place(&mut sim, ToolId::Window, 300.0);
    spec(&mut sim, id, &["Casing"], |d| {
        let s = &mut d.draft_mut().extras.spec;
        s.casing_in_plan = true;
        s.curved_casing = CurvedCasing::Parallel;
    });
    let o = opening(&sim, id);
    assert_eq!(o.extras.spec.curved_casing, CurvedCasing::Parallel);
    // On the shell's straight wall the three modes draw the same boards; on a
    // curved wall the parallel ones are bent.
    let mut wall = sim.app.cx.floor().wall(o.wall_id).unwrap().clone();
    let straight = plan_core::opening_symbol::casing_parts(&wall, &o, None, -1.0);
    wall.curve = Some(plan_core::WallCurve { bulge: 60.0 });
    let curved = plan_core::opening_symbol::casing_parts(&wall, &o, None, -1.0);
    assert!(straight.iter().all(|p| p.points.len() == 4));
    assert!(curved.iter().all(|p| p.points.len() > 4));
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
}

#[test]
fn a_size_without_the_frame_is_cut_wider_in_the_plan_and_in_3d() {
    let mut sim = house();
    let id = place(&mut sim, ToolId::Window, 300.0);
    // A 2" frame (the Frame tab's width) before the tab is touched.
    let fl = sim.app.cx.floor;
    sim.app.cx.project.floors[fl]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
        .unwrap()
        .extras
        .frame_width = Some(2.0);
    let x_range = |sim: &Sim| {
        let ms = scene_meshes(sim, id);
        ms.iter()
            .filter(|m| m.material == Material::WindowFrame)
            .flat_map(|m| m.vertices.iter().map(|v| v.position[0]))
            .fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
    };
    let (a0, a1) = x_range(&sim);
    spec(&mut sim, id, &["Frame"], |d| {
        d.draft_mut().extras.spec.size_includes_frame = false;
    });
    let (b0, b1) = x_range(&sim);
    assert!(
        (a0 - b0 - 2.0).abs() < 0.01 && (b1 - a1 - 2.0).abs() < 0.01,
        "{a0}..{a1} -> {b0}..{b1}"
    );
    let o = opening(&sim, id);
    assert_eq!(o.kind, OpeningKind::Window);
    let cleared = symbol(&sim, id).span;
    assert!((cleared.0 - (o.start_offset() - 2.0)).abs() < 1e-6);
    // Undo puts the wall hole back.
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    assert_eq!(x_range(&sim), (a0, a1));
}
