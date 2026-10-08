//! Scenario 11: every tool of the registry and of every flyout. Each one is
//! activated, gets a click-click, a double-click, Escape and a dialog /
//! overlay frame; nothing may panic and `hint()` must say something. The
//! tools that create nothing on a click-click are listed (not a failure:
//! many need a drag, a selection or a typed value).

use super::{draw_shell, Sim};
use crate::editor::stairs_view::StairKind;
use crate::editor::{placed, roof_view, rooms_edit, site_view, stairs_view, ObjectRef};
use crate::shell::hotkeys::collect_commands;
use crate::toolbar::Action;
use crate::tools::cad::CadMode;
use crate::tools::dimension::DimMode;
use crate::tools::electrical::ElecVariant;
use crate::tools::foundation::FoundationVariant;
use crate::tools::terrain::TerrainVariant;
use crate::tools::text::TextMode;
use crate::tools::{registry, ToolId};
use plan_cabinets::CabinetKind;
use plan_core::WallKind;
use std::panic::{catch_unwind, AssertUnwindSafe};

const W: f64 = 480.0;
const H: f64 = 360.0;

/// Every tool id the toolbars, flyouts and menus can pick.
fn all_tool_ids() -> Vec<ToolId> {
    let mut ids: Vec<ToolId> = Vec::new();
    let mut add = |id: ToolId| {
        if !ids.contains(&id) {
            ids.push(id);
        }
    };
    // The registry: one object per tool.
    for t in registry() {
        add(t.id());
    }
    // Everything the commands (toolbar buttons, flyouts, menus) activate.
    for c in collect_commands() {
        if let Action::SetTool(id) = c.action {
            add(id);
        }
    }
    // The variant lists, in case a flyout entry is missing from the toolbar.
    for m in DimMode::ALL {
        add(ToolId::DimensionVariant(m));
    }
    for m in CadMode::ALL {
        add(ToolId::CadVariant(m));
    }
    for m in TextMode::ALL {
        add(ToolId::TextVariant(m));
    }
    for k in StairKind::ALL {
        add(ToolId::StairsVariant(k));
    }
    for v in FoundationVariant::ALL {
        add(ToolId::FoundationVariant(v));
    }
    for k in crate::tools::cabinet::KINDS {
        add(ToolId::CabinetVariant(k));
    }
    for v in [
        ElecVariant::Outlet110,
        ElecVariant::Outlet220,
        ElecVariant::Gfci,
        ElecVariant::Light,
        ElecVariant::RopeLight,
        ElecVariant::Switch,
        ElecVariant::Switch3Way,
        ElecVariant::CeilingFan,
        ElecVariant::SmokeDetector,
        ElecVariant::Connection,
        ElecVariant::AutoOutlets,
    ] {
        add(ToolId::ElectricalVariant(v));
    }
    for v in [
        TerrainVariant::Perimeter,
        TerrainVariant::ElevationPoint,
        TerrainVariant::ElevationLine,
        TerrainVariant::ElevationRegion,
        TerrainVariant::Hill,
        TerrainVariant::Valley,
        TerrainVariant::Raised,
        TerrainVariant::Lowered,
        TerrainVariant::Flat,
        TerrainVariant::Road,
        TerrainVariant::Driveway,
        TerrainVariant::Sidewalk,
        TerrainVariant::Hole,
        TerrainVariant::Build,
    ] {
        add(ToolId::TerrainVariant(v));
    }
    ids
}

/// A house with a door, a window, a partition and a cabinet: something under
/// every click.
fn lived_in_house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    sim.click(200.0, 10.0);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    sim
}

/// What a click-click on a fresh house leaves behind.
fn fingerprint(sim: &Sim) -> (String, Option<String>, usize) {
    (
        format!("{:?}", sim.app.cx.project),
        sim.app.cx.undo_label().map(String::from),
        sim.app.cx.requests.len() + sim.requests.len(),
    )
}

#[test]
fn the_toolbars_reach_a_large_set_of_distinct_tools() {
    let ids = all_tool_ids();
    // Every registered tool is reachable.
    for t in registry() {
        assert!(ids.iter().any(|i| i.same_tool(t.id())), "{:?}", t.id());
    }
    assert!(ids.len() >= 80, "only {} tool ids", ids.len());
}

#[test]
fn every_tool_survives_click_double_click_escape_and_a_dialog_frame() {
    let ids = all_tool_ids();
    let mut panicked: Vec<String> = Vec::new();
    let mut no_hint: Vec<String> = Vec::new();
    let mut creates_nothing: Vec<String> = Vec::new();
    let mut creates: Vec<String> = Vec::new();
    for id in &ids {
        let id = *id;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut sim = lived_in_house();
            sim.tool(id);
            let name = sim.app.tools.active().name().to_string();
            assert!(!name.is_empty(), "{id:?} has no name");
            let hint = sim.app.tools.active().hint();
            assert!(!hint.trim().is_empty(), "{id:?} has no hint");
            assert_ne!(sim.app.cx.status, "Tool not implemented yet");
            let before = fingerprint(&sim);
            // Click-click in the middle of the room ...
            sim.click(240.0, 180.0);
            sim.click(300.0, 220.0);
            let mut changed = fingerprint(&sim) != before;
            // ... and next to the south wall (doors, windows, devices).
            if !changed {
                let mut near_wall = lived_in_house();
                near_wall.tool(id);
                let b = fingerprint(&near_wall);
                near_wall.click(60.0, 2.0);
                near_wall.click(160.0, 2.0);
                changed = fingerprint(&near_wall) != b;
            }
            sim.double_click(240.0, 180.0);
            sim.esc();
            sim.drag((60.0, 100.0), (200.0, 100.0));
            sim.esc();
            sim.click(100.0, 0.0);
            sim.esc();
            // Overlay and dialogs draw.
            sim.dialog_frame(false);
            let _ = sim.plan_shapes();
            // The tool can be left and re-entered.
            sim.tool(ToolId::Select);
            sim.tool(id);
            sim.tool(ToolId::Select);
            (name, changed)
        }));
        match outcome {
            Ok((name, created)) => {
                if created {
                    creates.push(name);
                } else {
                    creates_nothing.push(format!("{name} ({id:?})"));
                }
            }
            Err(_) => panicked.push(format!("{id:?}")),
        }
    }
    eprintln!(
        "tools that changed the plan on a click-click ({}): {creates:?}",
        creates.len()
    );
    eprintln!(
        "tools that create nothing on a click-click ({}):\n  {}",
        creates_nothing.len(),
        creates_nothing.join("\n  ")
    );
    let _ = &mut no_hint;
    assert!(panicked.is_empty(), "tools that panicked: {panicked:?}");
}

#[test]
fn tools_that_place_objects_on_a_plain_click_do_so_with_one_undo_step() {
    // The one-click tools of the toolbar each leave exactly one undo step.
    let cases: Vec<(ToolId, &str)> = vec![
        (ToolId::Door, "Place Door"),
        (ToolId::Window, "Place Window"),
        (
            ToolId::CabinetVariant(CabinetKind::Base),
            "Place Base Cabinet",
        ),
        (ToolId::ElectricalVariant(ElecVariant::Outlet110), ""),
    ];
    for (id, label) in cases {
        let mut sim = Sim::new();
        draw_shell(&mut sim, W, H);
        let before = sim.app.cx.undo_label().map(String::from);
        sim.tool(id);
        sim.click(200.0, 3.0);
        let after = sim.app.cx.undo_label().map(String::from);
        assert_ne!(before, after, "{id:?} left no undo step");
        if !label.is_empty() {
            assert_eq!(after.as_deref(), Some(label));
        }
        sim.undo();
        assert_eq!(sim.app.cx.undo_label().map(String::from), before, "{id:?}");
    }
}

fn spec_opened(sim: &mut Sim, o: ObjectRef) -> bool {
    sim.app.dialog = None;
    sim.app.spec = Default::default();
    let _ = rooms_edit::take_room_dialog_request(&sim.app.cx);
    let _ = crate::shell::view3d_panel::Outbox::global().take();
    sim.app.cx.selection.set(o);
    sim.app
        .cx
        .requests
        .push(crate::editor::EditorRequest::OpenSpec(o));
    sim.app.process_requests();
    match o {
        ObjectRef::Room(idx) => rooms_edit::take_room_dialog_request(&sim.app.cx) == Some(idx),
        ObjectRef::Camera(id) => crate::shell::view3d_panel::Outbox::global()
            .take()
            .contains(&crate::shell::view3d_panel::ViewRequest::OpenCameraSpec(id)),
        _ => sim.app.has_dialog(),
    }
}

/// Everything the tools made, as selectable references.
fn made_objects(sim: &Sim) -> Vec<ObjectRef> {
    let cx = &sim.app.cx;
    let f = cx.floor();
    let mut v: Vec<ObjectRef> = Vec::new();
    v.extend(f.walls.iter().map(|w| ObjectRef::Wall(w.id)));
    v.extend(f.openings.iter().map(|o| ObjectRef::Opening(o.id)));
    v.extend(f.dimensions.iter().map(|d| ObjectRef::Dimension(d.id)));
    for c in &f.cad {
        match c.item {
            plan_core::cad::CadItem::Text { .. } => v.push(ObjectRef::Text(c.id)),
            _ => v.push(ObjectRef::Cad(c.id)),
        }
    }
    v.extend(
        placed::load_cabinets(f)
            .iter()
            .map(|c| ObjectRef::Cabinet(c.id)),
    );
    v.extend(
        stairs_view::load(f)
            .iter()
            .map(|s| ObjectRef::Stair(s.id())),
    );
    let set = roof_view::load(f);
    v.extend(set.planes.iter().map(|p| ObjectRef::RoofPlane(p.id)));
    v.extend(set.dormers.iter().map(|d| ObjectRef::RoofPlane(d.id)));
    v.extend(set.ceilings.iter().map(|c| ObjectRef::RoofPlane(c.id)));
    v.extend(
        site_view::load_electrical(f)
            .devices
            .iter()
            .map(|d| ObjectRef::Device(d.id)),
    );
    v.extend(f.symbols.iter().map(|s| ObjectRef::Symbol(s.id)));
    v.extend(
        cx.project
            .cameras_on(cx.floor)
            .map(|c| ObjectRef::Camera(c.id)),
    );
    if site_view::load_terrain(&cx.project).is_some() {
        v.push(ObjectRef::Terrain);
    }
    v.extend((0..cx.rooms.len()).map(ObjectRef::Room));
    v.extend(
        plan_core::foundation::FoundationLayer::load(f)
            .slabs
            .iter()
            .map(|s| ObjectRef::Foundation(s.id)),
    );
    v
}

#[test]
fn every_object_the_tools_make_opens_a_specification_dialog_that_closes_again() {
    use crate::tools::dimension::DimMode;
    use crate::tools::roof::RoofMode;
    use eframe::egui::Key;
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    // Openings, cabinets, a stair, a roof, an interior wall.
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((240.0, 0.0), (240.0, H + 1.0));
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    sim.click(100.0, 10.0);
    sim.tool(ToolId::CabinetVariant(CabinetKind::Wall));
    sim.app.cx.selection.clear();
    sim.click(100.0, 10.0);
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((300.0, 100.0), (420.0, 100.0));
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    // Annotations.
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    sim.click(240.0, 180.0);
    sim.tool(ToolId::TextVariant(TextMode::Text));
    sim.click(60.0, 200.0);
    sim.key(crate::tools::KeyEvent::text("Den"));
    sim.key(crate::tools::KeyEvent::key(Key::Enter));
    sim.tool(ToolId::CadVariant(CadMode::Line));
    sim.drag((50.0, 300.0), (150.0, 300.0));
    sim.esc();
    // Electrical, terrain, slab.
    sim.tool(ToolId::ElectricalVariant(ElecVariant::Outlet110));
    sim.click(40.0, 8.0);
    sim.tool(ToolId::TerrainVariant(TerrainVariant::Perimeter));
    for (x, y) in [
        (-300.0, -300.0),
        (780.0, -300.0),
        (780.0, 660.0),
        (-300.0, 660.0),
    ] {
        sim.click(x, y);
    }
    sim.key(crate::tools::KeyEvent::key(Key::Enter));
    sim.tool(ToolId::FoundationVariant(FoundationVariant::Slab));
    sim.drag((-10.0, -10.0), (W + 12.0, H + 12.0));
    sim.tool(ToolId::CameraVariant(
        crate::tools::camera::CameraVariant::FullCamera,
    ));
    sim.click(150.0, 150.0);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    sim.app.cx.refresh();

    let objects = made_objects(&sim);
    let mut kinds: Vec<&str> = objects.iter().map(|o| o.type_name()).collect();
    kinds.sort();
    kinds.dedup();
    eprintln!("object kinds made: {kinds:?}");
    for want in [
        "Wall",
        "Opening",
        "Cabinet",
        "Stairs",
        "Roof Plane",
        "Dimension",
        "Text",
        "CAD Object",
        "Electrical Device",
        "Terrain",
        "Room",
        "Foundation Object",
        "Camera",
    ] {
        assert!(kinds.contains(&want), "no {want} was made: {kinds:?}");
    }
    let mut without: Vec<ObjectRef> = Vec::new();
    for o in &objects {
        if !spec_opened(&mut sim, *o) {
            without.push(*o);
            continue;
        }
        // The dialog draws and closes again (OK or, for the tool-owned ones, Esc).
        sim.dialog_frame(false);
        sim.dialog_frame(false);
        sim.cancel();
        assert!(!sim.app.has_dialog(), "{o:?} dialog did not close");
        sim.app.dialog = None;
    }
    assert!(
        without.is_empty(),
        "no specification dialog for {without:?}"
    );
}
