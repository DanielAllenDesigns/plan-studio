//! Lesson 10, Custom Ceilings (pp. 177-190): a dropped hat-channel ceiling from a
//! Ceiling Finish Definition, a cathedral deck ceiling by clearing Flat Ceiling Over
//! This Room, a hat-channel framing member, and a coffered ceiling with its soffits.
use crate::dialogs::framing_defaults;
use crate::dialogs::room::RoomDialog;
use crate::editor::rooms_edit;
use crate::scenarios::tutorials_support::*;
use crate::scenarios::{draw_shell, Sim};
use crate::tools::tray_ceiling::{self as tc, cmd};
use plan_core::assemblies::{
    Assembly, AssemblyKind, AssemblyLayer, FramingConstruction, FramingMethod, FramingSpec,
    LayerRole,
};
use plan_core::geometry::Point;
use plan_framing::catalog::{
    CategoryChoice, Composition, FramingMemberDef, FramingShape, FramingType, Role,
};

const ROOM_AT: Point = Point { x: 120.0, y: 90.0 };

fn house() -> Sim {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    draw_shell(&mut sim, 240.0, 180.0);
    sim.app.cx.refresh();
    sim
}

/// Drywall and paint over a hat channel at `spacing` on centre, hung under an air gap.
fn hat_channel_ceiling(spacing: f64) -> Assembly {
    let mut hat = AssemblyLayer::new("Hat Channel", LayerRole::Framing, 0.875);
    hat.framing = Some(FramingSpec {
        method: FramingMethod::Joists,
        construction: FramingConstruction::HatChannel,
        width: 1.0,
        spacing,
    });
    Assembly::new(vec![
        AssemblyLayer::new("Plenum", LayerRole::AirGap, 11.125),
        hat,
        AssemblyLayer::new("Drywall", LayerRole::Finish, 0.5),
        AssemblyLayer::new("Paint", LayerRole::Finish, 0.0625),
    ])
}

#[test]
fn ceiling_finish_with_drywall_paint_and_hat_channel_at_24_oc() {
    let mut sim = house();
    let idx = rooms_edit::room_index_at(&sim.app.cx, ROOM_AT).expect("a room");
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    let mut d = RoomDialog::new(init);
    d.set_platform(AssemblyKind::CeilingFinish, hat_channel_ceiling(24.0));
    let drop = d.platform(AssemblyKind::CeilingFinish).drop();
    assert!(
        drop > 11.0,
        "the plenum and channel hang the ceiling: {drop}"
    );
    let draft = d.room_name().clone();
    assert_one_undo_step(&mut sim, "Room Specification", |s| {
        assert!(rooms_edit::apply_room_spec(
            &mut s.app.cx,
            d.room_index(),
            &draft,
            d.extras()
        ));
    });
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
    let own = misc
        .assemblies
        .ceiling_finish
        .own()
        .expect("own ceiling finish");
    let spec = own
        .layers
        .iter()
        .find_map(|l| l.framing.as_ref())
        .expect("a framing layer");
    assert_eq!(spec.spacing, 24.0);
    assert_eq!(spec.construction, FramingConstruction::HatChannel);
    assert_eq!(own.layers.len(), 4);
}

#[test]
fn clearing_flat_ceiling_gives_a_cathedral_deck_ceiling_and_undoes() {
    let mut sim = house();
    rooms_edit::select_room(&mut sim.app.cx, 0);
    let flat = |s: &Sim| s.app.cx.floor().room_names.iter().all(|r| r.flat_ceiling);
    assert!(flat(&sim), "ceilings start flat");
    sim.action(crate::toolbar::Action::Custom(
        rooms_edit::commands::id::CEILING_OFF,
    ));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    let entry = rooms_edit::name_entry(&sim.app.cx, &sim.app.cx.rooms[0])
        .cloned()
        .unwrap();
    assert!(
        !entry.flat_ceiling,
        "Flat Ceiling Over This Room is cleared"
    );
    sim.undo();
    let entry = rooms_edit::name_entry(&sim.app.cx, &sim.app.cx.rooms[0]).cloned();
    assert!(entry.is_none_or(|e| e.flat_ceiling));
}

#[test]
fn a_hat_channel_member_joins_the_framing_defaults_as_one_undo_step() {
    let mut sim = house();
    let mut cat = framing_defaults::catalog_of(&sim.app.cx);
    cat.add_type(FramingType::new(
        "Hat Channel",
        Composition::Steel,
        FramingShape::UChannel,
    ))
    .unwrap();
    cat.add_def(FramingMemberDef {
        name: "Hat Channel Furring".into(),
        framing_type: "Hat Channel".into(),
        material: String::new(),
        role: Role::CeilingJoist,
        category: CategoryChoice::Auto,
    })
    .unwrap();
    assert_one_undo_step(&mut sim, "Framing Defaults", |s| {
        assert!(framing_defaults::set_catalog(
            &mut s.app.cx,
            &cat,
            "Framing Defaults"
        ));
    });
    let back = framing_defaults::catalog_of(&sim.app.cx);
    assert!(back.type_named("Hat Channel").is_some());
    let def = back.def_named("Hat Channel Furring").expect("the member");
    assert_eq!(def.role, Role::CeilingJoist);
    assert!(!back.def_in_use("Hat Channel Furring"));
}

#[test]
fn a_coffered_ceiling_cuts_the_soffit_ring_in_one_undo_step() {
    let mut sim = house();
    rooms_edit::select_room(&mut sim.app.cx, 0);
    assert_one_undo_step(&mut sim, "Make Coffered Ceiling", |s| {
        assert!(tc::run_command(&mut s.app.cx, cmd::COFFERED));
    });
    let geoms = tc::geoms(&sim.app.cx);
    assert!(geoms.len() >= 4, "a grid of coffers: {}", geoms.len());
    // The beams between the coffers (the soffit ring) stay at the ceiling.
    let h = sim.app.cx.floor().ceiling_height;
    assert!(geoms.iter().all(|t| t.ok() && t.h_inner > h));
}
