//! Scenario 84: layered floor, ceiling and roof definitions (Round 16, brief
//! 13; R-25, R-27..R-29, R-56, R-122..R-124, R-143, R-144, RF-36): a 12 in
//! floor of 3/4 OSB over a 2x12, a hat-channel dropped ceiling and a tile
//! over backerboard finish, applied through the dialogs; the platform
//! heights in 3D, the Materials List lines and one undo step per OK.

use super::{draw_shell, Sim};
use crate::dialogs::assembly_def::{self, AssemblyDefDialog};
use crate::dialogs::floor::FloorDialog;
use crate::dialogs::room::RoomDialog;
use crate::editor::roof_view::{self, RoofFraming};
use crate::editor::rooms_edit;
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;
use plan_3d::Material;
use plan_core::assemblies::{
    Assembly, AssemblyKind, AssemblyLayer, AssemblyLibrary, AssemblySlot, FramingConstruction,
    FramingMethod, FramingSpec, LayerRole, RoofLayers,
};
use plan_core::geometry::Point;
use plan_core::{Project, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;
const WEST_AT: Point = Point { x: 120.0, y: 180.0 };
const EAST_AT: Point = Point { x: 360.0, y: 180.0 };
/// Plan x ranges that include a room outline of the west and east rooms.
const WEST: (f32, f32) = (-5.0, 10.0);
const EAST: (f32, f32) = (300.0, 500.0);

fn two_room_house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((240.0, 0.0), (240.0, H + 1.0));
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    assert_eq!(sim.app.cx.rooms.len(), 2);
    sim
}

fn layer(name: &str, role: LayerRole, t: f64) -> AssemblyLayer {
    AssemblyLayer::new(name, role, t)
}

/// 3/4 OSB over a 2x12 at 16 in: a 12 in floor platform.
fn floor_12() -> Assembly {
    let mut joist = layer("2x12", LayerRole::Framing, 11.25);
    joist.framing = Some(FramingSpec {
        method: FramingMethod::Joists,
        construction: FramingConstruction::Lumber,
        width: 1.5,
        spacing: 16.0,
    });
    Assembly::new(vec![layer("3/4 OSB", LayerRole::Sheathing, 0.75), joist])
}

/// Plenum, hat channel and drywall: a ceiling hung 12 in below the platform.
fn dropped_ceiling() -> Assembly {
    let mut hat = layer("Hat Channel", LayerRole::Framing, 0.875);
    hat.framing = Some(FramingSpec {
        method: FramingMethod::Joists,
        construction: FramingConstruction::HatChannel,
        width: 1.0,
        spacing: 16.0,
    });
    Assembly::new(vec![
        layer("Plenum", LayerRole::AirGap, 11.125),
        hat,
        layer("Drywall", LayerRole::Finish, 0.5),
    ])
}

/// Ceramic tile on backerboard.
fn tile_over_backer() -> Assembly {
    Assembly::new(vec![
        layer("Ceramic Tile", LayerRole::Finish, 0.375),
        layer("Backerboard", LayerRole::Standard, 0.5),
    ])
}

fn scene(sim: &Sim) -> plan_3d::Scene {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
}

fn heights(sim: &Sim, material: Material, x: (f32, f32)) -> Vec<f32> {
    scene(sim)
        .meshes
        .into_iter()
        .filter(|m| m.material == material)
        .flat_map(|m| m.vertices)
        .filter(|v| v.position[0] >= x.0 && v.position[0] <= x.1)
        .map(|v| v.position[1])
        .collect()
}

fn top(sim: &Sim, m: Material, x: (f32, f32)) -> f32 {
    heights(sim, m, x).into_iter().fold(f32::MIN, f32::max)
}

fn bottom(sim: &Sim, m: Material, x: (f32, f32)) -> f32 {
    heights(sim, m, x).into_iter().fold(f32::MAX, f32::min)
}

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

/// Opens Floor Defaults for the active floor, lets `f` edit the dialog and
/// applies it the way OK does.
fn floor_defaults(
    sim: &mut Sim,
    f: impl FnOnce(&mut crate::dialogs::floor_defaults::FloorDefaultsDialog),
) {
    let FloorDialog::Defaults(mut d) = FloorDialog::defaults_for_floor(&sim.app.cx) else {
        unreachable!()
    };
    f(&mut d);
    FloorDialog::Defaults(d).apply(&mut sim.app.cx);
}

#[test]
fn a_12_inch_floor_of_osb_and_a_2x12_builds_a_12_inch_platform_in_one_undo_step() {
    let mut sim = two_room_house();
    let before = sim.app.cx.project.floors[0].settings.clone();
    let json_before = sim.app.cx.project.to_json().unwrap();
    assert!(
        !json_before.contains("\"platform\""),
        "an old plan has no layers"
    );
    let datum = sim.app.cx.floor().elevation as f32;

    floor_defaults(&mut sim, |d| {
        let mut w = AssemblyDefDialog::new(AssemblyKind::FloorStructure, floor_12(), d.library());
        assert!(w.error().is_none());
        assert_eq!(w.total_thickness(), 12.0);
        w.select(1);
        d.accept_editor(w);
    });
    assert_eq!(sim.app.cx.undo_label(), Some("Floor Defaults"));
    let st = &sim.app.cx.floor().settings;
    assert_eq!(st.floor_structure_thickness, 12.0, "the old field follows");
    assert!(st.platform.floor_structure.own().is_some());

    // OSB and joist are two layers of framing material from the datum down.
    let low = bottom(&sim, Material::Framing, WEST);
    let high = top(&sim, Material::Framing, WEST);
    assert!((high - datum).abs() < 0.01, "top {high}");
    assert!(
        (high - low - 12.0).abs() < 0.01,
        "a 12 in platform, got {}",
        high - low
    );
    // Both rooms (the floor is the level default).
    assert!((bottom(&sim, Material::Framing, EAST) - low).abs() < 0.01);

    // The floor above moves up by the 12 in platform.
    let upper = rooms_edit::build_new_floor(&mut sim.app.cx, true);
    let f0 = &sim.app.cx.project.floors[0];
    let f1 = &sim.app.cx.project.floors[upper];
    let want = f0.elevation + f0.ceiling_height + f0.settings.ceiling_structure_thickness + 12.0;
    assert!(
        (f1.elevation - want).abs() < 1e-9,
        "{} vs {want}",
        f1.elevation
    );
    assert_eq!(sim.undo().as_deref(), Some("Build New Floor"));

    // One undo step brings the single thickness back.
    assert_eq!(sim.undo().as_deref(), Some("Floor Defaults"));
    assert_eq!(sim.app.cx.project.floors[0].settings, before);
    assert_eq!(sim.app.cx.project.to_json().unwrap(), json_before);
}

#[test]
fn a_hat_channel_dropped_ceiling_lowers_the_finished_ceiling_and_keeps_the_wall_tops() {
    let mut sim = two_room_house();
    let h = sim.app.cx.floor().ceiling_height as f32;
    let walls_before = top(&sim, Material::WallExterior, (-10.0, 500.0));
    let plain_bottom = bottom(&sim, Material::Ceiling, EAST);
    assert!((plain_bottom - h).abs() < 0.01);

    edit_room(&mut sim, EAST_AT, |d| {
        d.set_platform(AssemblyKind::CeilingFinish, dropped_ceiling());
        assert_eq!(d.platform(AssemblyKind::CeilingFinish).drop(), 12.0);
    });
    // The finished ceiling of the east room hangs 12 in lower; the west
    // room and the wall tops stay.
    let hung = bottom(&sim, Material::Ceiling, EAST);
    assert!((hung - (h - 12.0)).abs() < 0.01, "finished ceiling {hung}");
    assert!((bottom(&sim, Material::Ceiling, WEST) - h).abs() < 0.01);
    assert!((top(&sim, Material::WallExterior, (-10.0, 500.0)) - walls_before).abs() < 0.01);
    // The hat channel is drawn as framing under the plenum; the platform is
    // where it was (plain finish: 5/8, here 1/2).
    assert!(heights(&sim, Material::Framing, EAST)
        .iter()
        .any(|y| (*y - (h - 12.0 + 0.5)).abs() < 0.01));
    let ceil_top = top(&sim, Material::Ceiling, EAST);
    assert!(
        (ceil_top - (h + 0.5 + 1.0)).abs() < 0.01,
        "platform top {ceil_top}"
    );

    // The old field is the surface the platform sits on.
    let misc = sim
        .app
        .cx
        .floor()
        .room_names
        .last()
        .unwrap()
        .misc
        .clone()
        .unwrap();
    assert_eq!(misc.ceiling_finish_thickness, 0.5);
    assert!(misc.assemblies.ceiling_finish.own().is_some());

    // Reopened, the room shows its definition and the drop.
    let idx = rooms_edit::room_index_at(&sim.app.cx, EAST_AT).unwrap();
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    let d = RoomDialog::new(init);
    assert_eq!(d.platform(AssemblyKind::CeilingFinish), dropped_ceiling());

    // One step undoes it.
    assert_eq!(sim.undo().as_deref(), Some("Room Specification"));
    assert!((bottom(&sim, Material::Ceiling, EAST) - h).abs() < 0.01);
}

#[test]
fn tile_over_backerboard_is_a_floor_finish_and_the_materials_list_reads_the_layers() {
    let mut sim = two_room_house();
    let datum = sim.app.cx.floor().elevation as f32;
    edit_room(&mut sim, WEST_AT, |d| {
        d.set_platform(AssemblyKind::FloorFinish, tile_over_backer());
        d.set_platform(AssemblyKind::FloorStructure, floor_12());
        d.set_platform(AssemblyKind::CeilingFinish, dropped_ceiling());
    });
    // The finished floor is 7/8 in above the platform datum: tile on top of
    // the backer, which sits on the OSB.
    let tile_top = top(&sim, Material::Stone, WEST);
    let backer_top = top(&sim, Material::Concrete, WEST);
    assert!((tile_top - (datum + 0.875)).abs() < 0.01, "tile {tile_top}");
    assert!(
        (backer_top - (datum + 0.5)).abs() < 0.01,
        "backer {backer_top}"
    );
    // The west room's floor platform is 12 in below the datum, east is not.
    let low = bottom(&sim, Material::Framing, WEST);
    assert!((low - (datum - 12.0)).abs() < 0.01, "platform bottom {low}");

    let lines = plan_docs::materials_list(&sim.app.cx.project, 0, &sim.app.cx.rooms);
    let has = |prefix: &str| lines.iter().any(|l| l.item.starts_with(prefix));
    assert!(
        has("Subfloor 3/4 OSB 3/4\" 4x8 sheet"),
        "{:?}",
        items(&lines)
    );
    assert!(has("Flooring - Ceramic Tile 3/8\""), "{:?}", items(&lines));
    assert!(
        has("Underlayment - Backerboard 1/2\""),
        "{:?}",
        items(&lines)
    );
    assert!(
        has("Floor framing 2x12 11 1/4\" @ 16\" o.c."),
        "{:?}",
        items(&lines)
    );
    assert!(has("Wallboard 1/2\" 4x8 sheet"), "{:?}", items(&lines));
    assert!(has("Ceiling framing Hat Channel"), "{:?}", items(&lines));
    // The plenum buys nothing.
    assert!(!lines.iter().any(|l| l.item.contains("Plenum")));
    // The east room still lists the two rows it always had.
    assert!(lines
        .iter()
        .any(|l| l.item.starts_with("Ceiling drywall 1/2\" 4x8 sheet")));
    assert!(lines
        .iter()
        .any(|l| l.item.starts_with("Flooring - ") && !l.item.contains("Ceramic")));
}

fn items(lines: &[plan_docs::MaterialLine]) -> Vec<&str> {
    lines.iter().map(|l| l.item.as_str()).collect()
}

#[test]
fn a_floor_that_uses_the_default_follows_the_plan_wide_page_and_ok_is_one_step() {
    let mut sim = two_room_house();
    // Default Settings > Floor/Ceiling Platform.
    assembly_def::request_page();
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(assembly_def::page_is_open());
    let depths = assembly_def::with_page(|p| {
        // The depths reported are the floor's single thicknesses.
        let d = p.depths();
        let mut w = AssemblyDefDialog::new(
            AssemblyKind::FloorStructure,
            floor_12(),
            &AssemblyLibrary::default(),
        );
        w.select(0);
        p.slots_mut().set(
            AssemblyKind::FloorStructure,
            AssemblySlot::Own(w.into_assembly()),
        );
        d
    })
    .expect("the page is open");
    assert_eq!(depths[0], 10.25);
    assert_eq!(depths[3], 0.625);
    sim.ok();
    assert!(!assembly_def::page_is_open());
    assert_eq!(
        sim.app.cx.undo_label(),
        Some("Floor/Ceiling Platform Defaults")
    );
    // The floor still at the default thickness follows the plan-wide one.
    let st = &sim.app.cx.floor().settings;
    assert_eq!(st.platform.floor_structure, AssemblySlot::Default);
    assert_eq!(st.floor_structure_thickness, 12.0);
    let low = bottom(&sim, Material::Framing, WEST);
    let datum = sim.app.cx.floor().elevation as f32;
    assert!((low - (datum - 12.0)).abs() < 0.01);

    // Floor Defaults shows it as Use Default; turning that off keeps a copy.
    floor_defaults(&mut sim, |d| {
        assert_eq!(
            d.library_effective(AssemblyKind::FloorStructure)
                .unwrap()
                .total_thickness(),
            12.0
        );
        d.set_follow_default(AssemblyKind::FloorStructure, false);
        assert!(d.settings().platform.floor_structure.own().is_some());
    });
    assert!(sim
        .app
        .cx
        .floor()
        .settings
        .platform
        .floor_structure
        .own()
        .is_some());

    // One undo step per OK: the Floor Defaults, then the page.
    assert_eq!(sim.undo().as_deref(), Some("Floor Defaults"));
    assert_eq!(
        sim.app.cx.floor().settings.platform.floor_structure,
        AssemblySlot::Default
    );
    assert_eq!(
        sim.undo().as_deref(),
        Some("Floor/Ceiling Platform Defaults")
    );
    assert!(sim.app.cx.project.assemblies.is_empty());
    assert_eq!(sim.app.cx.floor().settings.floor_structure_thickness, 10.25);
    assert!(sim.app.cx.floor().settings.platform.is_legacy());
}

#[test]
fn a_plan_with_old_thicknesses_loads_draws_and_saves_exactly_as_before() {
    let mut sim = two_room_house();
    sim.app.cx.project.floors[0]
        .settings
        .floor_structure_thickness = 11.0;
    sim.app.cx.project.floors[0].settings.floor_finish_thickness = 0.5;
    let json = sim.app.cx.project.to_json().unwrap();
    let loaded = Project::from_json(&json).unwrap();
    assert_eq!(loaded.to_json().unwrap(), json, "byte-stable");
    assert!(loaded.assemblies.is_empty() && loaded.floors[0].settings.platform.is_legacy());
    let a = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    let b = build_view_scene(&loaded, &ViewScope::default());
    let sum = |s: &plan_3d::Scene| s.meshes.iter().map(|m| m.triangle_count()).sum::<usize>();
    assert_eq!(sum(&a), sum(&b));
    assert_eq!(a.meshes.len(), b.meshes.len());
}

#[test]
fn a_roof_plane_takes_its_surface_structure_and_ceiling_finish_from_layers() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    let planes = roof_view::load(sim.app.cx.floor()).planes;
    let mut rec = planes[0].clone();
    // Rafters 2x10 at 16 under 5/8 sheathing, shingles over underlayment,
    // purlins stored on the framing layer.
    let mut layers = RoofLayers::from_numbers(0.5, 0.625, 9.25, 1.5, 16.0, false);
    layers
        .surface
        .insert(0, layer("Underlayment", LayerRole::Standard, 0.125));
    layers.structure.layers[1].framing.as_mut().unwrap().method = FramingMethod::Purlins;
    layers
        .structure
        .insert(0, layer("Cladding", LayerRole::Cladding, 0.25));
    rec.set_layers(Some(layers.clone()), roof_view::RoofStructure::default());
    let st = rec.structure.unwrap();
    assert_eq!(
        (st.member_depth, st.member_width, st.spacing),
        (9.25, 1.5, 16.0)
    );
    assert_eq!(st.framing, RoofFraming::Rafters);
    assert_eq!(st.sheathing, 0.625);
    // Everything over the framing but the sheathing is roofing.
    assert!((st.roofing - (0.5 + 0.125 + 0.25)).abs() < 1e-9);
    assert!((rec.eave.thickness.unwrap() - layers.thickness()).abs() < 1e-9);
    assert_eq!(rec.eave.rafter_depth, Some(9.25));

    assert!(roof_view::apply_plane_edit(
        &mut sim.app.cx.project,
        0,
        &rec
    ));
    let again = roof_view::load(&sim.app.cx.project.floors[0]);
    let got = again.plane(rec.id).unwrap();
    assert_eq!(
        got.layers.as_ref(),
        Some(&layers),
        "the layers are kept in the plan"
    );
    assert_eq!(got.structure, rec.structure);
    // Handing the plane back to Roof Defaults drops both.
    let mut back = got.clone();
    back.set_structure(None);
    assert!(back.layers.is_none() && back.structure.is_none());
}
