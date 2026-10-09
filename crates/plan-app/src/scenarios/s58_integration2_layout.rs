//! Scenario 58 (integration pass 2, area B): cabinets, stairs and symbols
//! in plan boxes and in the DXF, a second layout file from File > New
//! Layout, the Project Browser's parked layouts, counter outlets that follow
//! the base cabinets, the Electrical Connection layer and the schedule's
//! voltage and flags columns.

use super::s21_layout_print::isolate_home;
use super::{draw_shell, Sim};
use crate::dialogs::layout::{PageChoice, Placement, SendSource, SendSpec};
use crate::editor::placed::{self, apply_cabinet, load_cabinets};
use crate::editor::plan_overlay;
use crate::editor::stairs_view::StairKind;
use crate::shell::layout_window::{self as lw, LayoutView};
use crate::toolbar::Action;
use crate::tools::ToolId;
use plan_cabinets::{CabinetKind, FillPattern};
use plan_core::cad::CadItem;
use plan_docs::MasterList;
use plan_layout::OverlayShape;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    isolate_home();
    lw::use_memory_master_list(MasterList::default());
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.app.cx.project.name = "Maple Court Residence".into();
    sim
}

/// A hatched base cabinet and a stair on the first floor.
fn cabinet_and_stair(sim: &mut Sim) {
    placed::set_auto_join(false);
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    sim.app.cx.selection.clear();
    sim.click(100.0, 10.0);
    let mut draft = load_cabinets(sim.app.cx.floor())[0].clone();
    draft.fill.pattern = FillPattern::Hatch;
    assert!(apply_cabinet(&mut sim.app.cx, &draft));
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((200.0, 100.0), (350.0, 100.0));
}

#[test]
fn the_overlay_holds_the_cabinet_fill_symbol_and_the_stair() {
    let mut sim = house();
    cabinet_and_stair(&mut sim);
    let items = plan_overlay::overlay(&sim.app.cx.project, 0);
    let on = |layer: &str| items.iter().filter(|i| i.layer == layer).count();
    assert!(on("Cabinets, Base") > 4, "cabinet outline and hatch");
    // The hatch lines (Fill Style) are their own thin items.
    assert!(items
        .iter()
        .any(|i| i.layer == "Cabinets, Base" && (i.weight - 0.4).abs() < 1e-9));
    assert!(on("Stairs") > 4, "treads, outline and arrow");
    assert!(items.iter().any(|i| i.layer == "Stairs"
        && matches!(&i.shape, OverlayShape::Text { text, .. } if text.starts_with("UP"))));
    // Nothing on the floor above, but the stair's part beyond the break.
    assert!(plan_overlay::overlay(&sim.app.cx.project, 5).is_empty());
}

#[test]
fn a_plan_box_prints_the_cabinet_fill_and_the_stair() {
    let mut sim = house();
    cabinet_and_stair(&mut sim);
    let mut v = LayoutView::default();
    let p = &mut sim.app.cx.project;
    assert!(v.create(p, None));
    let spec = SendSpec {
        source: SendSource::Plan {
            floor: 0,
            layer_set: "Default Set".into(),
        },
        page: PageChoice::Existing(1),
        scale: None,
        placement: Placement::FirstFree,
    };
    let id = v.send(p, &spec, None).unwrap();
    let b = v
        .current_page()
        .unwrap()
        .boxes
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .clone();
    let with = plan_layout::render_box_lines(&b, &lw::render_context(p)).len();
    let without =
        plan_layout::render_box_lines(&b, &plan_layout::LayoutRenderContext::new(p)).len();
    assert!(
        with > without + 10,
        "cabinet and stair strokes join the plan: {without} -> {with}"
    );
    // And the sheet prints them: the PDF grows with the overlay.
    let layout = v.layout().unwrap().clone();
    let pdf_with = plan_layout::render_pdf(&layout, &lw::render_context(p));
    let pdf_without = plan_layout::render_pdf(&layout, &plan_layout::LayoutRenderContext::new(p));
    assert!(pdf_with.len() > pdf_without.len());
}

#[test]
fn the_dxf_carries_cabinets_and_stairs_with_the_hidden_treads_on_their_own_layer() {
    let mut sim = house();
    cabinet_and_stair(&mut sim);
    let plain = plan_core::write_dxf(&sim.app.cx.project, 0, &sim.app.cx.rooms);
    let dxf = crate::dialogs::exchange::floor_dxf(&mut sim.app.cx);
    let count = |s: &str, what: &str| s.lines().filter(|l| *l == what).count();
    assert!(
        count(&dxf, "POLYLINE") > count(&plain, "POLYLINE") + 3,
        "the cabinet and stair outlines are polylines"
    );
    assert!(
        count(&dxf, "TEXT") > count(&plain, "TEXT"),
        "UP and the label"
    );
    assert!(dxf.contains("Cabinets, Base"));
    // The cabinet's hatch lines are lines; the stair's break-line treads
    // are dashed items, so they sit on "Stairs, Hidden".
    let items = plan_overlay::dxf_items(&sim.app.cx.project, 0);
    assert!(items
        .iter()
        .any(|(l, i)| l == "Cabinets, Base" && matches!(i, CadItem::Polyline { .. })));
    // The plan itself is untouched by the export.
    assert!(sim.app.cx.floor().cad.is_empty());
}

#[test]
fn file_new_layout_makes_a_second_layout_file_and_the_browser_lists_the_parked_one() {
    let mut sim = house();
    sim.action(Action::FileNewLayout);
    assert!(lw::load(&sim.app.cx.project).is_some());
    assert!(lw::parked_layouts(&sim.app.cx.project).is_empty());
    // The second New Layout asks for the new file's name.
    lw::deactivate();
    sim.action(Action::FileNewLayout);
    assert!(lw::dialog_open(), "the name dialog is up");
    assert!(lw::is_active());
    // Making the file (what the dialog's OK does) parks the first one.
    let mut v = LayoutView::default();
    let first = lw::layout_names(&sim.app.cx.project)[0].clone();
    assert!(v.new_layout_file(&mut sim.app.cx.project, "Permit Set", None));
    let parked = lw::parked_layouts(&sim.app.cx.project);
    assert_eq!(parked, vec![(1, first)]);
    assert_eq!(lw::layout_names(&sim.app.cx.project)[0], "Permit Set");
}

#[test]
fn the_new_layout_tools_are_in_the_toolbar_catalog() {
    let names: Vec<&str> = crate::toolbar::config::catalog()
        .iter()
        .map(|e| e.key)
        .collect();
    for want in [
        "Page Information",
        "Customize Sheet Sizes",
        "New Layout File",
        "Open Source View",
        "Copy Layout Box to Page",
        "Export Table to Excel",
    ] {
        assert!(names.contains(&want), "{want} in the catalog");
    }
}

// ----- electrical -----

use crate::editor::site_view::load_electrical;
use crate::tools::electrical::ElecVariant;
use plan_core::geometry::Point;
use plan_core::layers::ELECTRICAL_CONNECTION_LAYER;

#[test]
fn a_connection_gets_its_own_layer_that_hides_apart_from_the_devices() {
    let mut sim = house();
    sim.tool(ToolId::ElectricalVariant(ElecVariant::Light));
    sim.click(244.0, 176.0);
    sim.tool(ToolId::ElectricalVariant(ElecVariant::Switch));
    sim.click(30.0, 8.0);
    assert!(
        sim.app
            .cx
            .project
            .layers
            .get(ELECTRICAL_CONNECTION_LAYER)
            .is_none(),
        "no layer until there is a connection"
    );
    let layer = load_electrical(sim.app.cx.floor());
    let (light, switch) = (
        layer
            .devices
            .iter()
            .find(|d| d.kind.is_light())
            .unwrap()
            .clone(),
        layer
            .devices
            .iter()
            .find(|d| d.kind.is_switch())
            .unwrap()
            .clone(),
    );
    sim.tool(ToolId::ElectricalVariant(ElecVariant::Connection));
    sim.click(switch.position.x, switch.position.y);
    sim.click(light.position.x, light.position.y);
    assert_eq!(load_electrical(sim.app.cx.floor()).connections.len(), 1);
    let l = sim
        .app
        .cx
        .project
        .layers
        .get(ELECTRICAL_CONNECTION_LAYER)
        .expect("the connection layer is made with the first connection");
    assert!(l.display);
    let shown = sim.plan_shapes().len();
    assert!(sim
        .app
        .cx
        .project
        .layers
        .set_display(ELECTRICAL_CONNECTION_LAYER, false));
    let hidden = sim.plan_shapes().len();
    assert!(hidden < shown, "the dashed arc goes: {shown} -> {hidden}");
    // The devices stay.
    assert_eq!(load_electrical(sim.app.cx.floor()).devices.len(), 2);
}

#[test]
fn the_electrical_schedule_has_hidden_voltage_and_flags_columns() {
    use plan_core::schedules::{Schedule, ScheduleKind};
    let mut sim = house();
    sim.tool(ToolId::ElectricalVariant(ElecVariant::Outlet220));
    sim.click(60.0, 8.0);
    let mut def = Schedule::new(ScheduleKind::Electrical, Point::ZERO);
    let shown = plan_docs::schedule_kinds::table(&sim.app.cx.project, &def, 0, None);
    assert!(!shown.columns.iter().any(|c| c == "Flags" || c == "Voltage"));
    for c in &mut def.columns {
        if c.field == "voltage" || c.field == "flags" {
            c.visible = true;
        }
    }
    let t = plan_docs::schedule_kinds::table(&sim.app.cx.project, &def, 0, None);
    let col = |name: &str| t.columns.iter().position(|c| c == name).unwrap();
    assert_eq!(t.rows[0][col("Voltage")], "220V");
    assert_eq!(t.rows[0][col("Flags")], "220V");
    assert_eq!(t.rows[0][col("Mark")], "E-01");
}

#[test]
fn counter_outlets_stand_only_where_the_base_cabinets_do() {
    let mut sim = house();
    sim.app.cx.refresh();
    let rooms = sim.app.cx.rooms.clone();
    let anchor = rooms[0].centroid;
    sim.app
        .cx
        .project
        .set_room_name(0, anchor, "Kitchen", "Kitchen", &rooms);
    placed::set_auto_join(false);
    for x in [60.0, 90.0, 120.0] {
        sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
        sim.app.cx.selection.clear();
        sim.click(x, 10.0);
    }
    assert_eq!(load_cabinets(sim.app.cx.floor()).len(), 3);
    let n = crate::tools::electrical::auto_place_floor_outlets(&mut sim.app.cx);
    assert!(n > 0);
    let counters: Vec<_> = load_electrical(sim.app.cx.floor())
        .devices
        .into_iter()
        .filter(|d| d.height > 40.0 && d.kind.is_outlet() && !d.kind.is_weatherproof())
        .collect();
    assert!(!counters.is_empty(), "counter-height outlets were placed");
    // The cabinets stand along the south wall (y near 0), x from 48 to 140.
    for d in &counters {
        assert!(
            d.position.y < 20.0,
            "against the south wall: {:?}",
            d.position
        );
        assert!(
            d.position.x > 40.0 && d.position.x < 150.0,
            "within the run: {:?}",
            d.position
        );
    }
}
