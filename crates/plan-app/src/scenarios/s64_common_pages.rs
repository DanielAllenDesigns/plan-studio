//! Scenario 64 (round 16, brief 05): the panels every specification dialog
//! shares. Object Information, Label, Schedule and Manufacturer open with the
//! dialogs of walls, doors and windows, cabinets, symbols, devices, stairs and
//! roof planes; what is typed shows in a schedule column and as a label on the
//! plan; and the Elevation Reference widget makes a cabinet follow the ceiling
//! (L-29, L-30, L-35, DW-118, W-118, R-89, R-90, CB-636, CB-637).

use super::{draw_shell, Sim};
use crate::dialogs::common_pages::{self, Placement};
use crate::dialogs::elevation_ref;
use crate::dialogs::object_info::{self, InfoSession, Page};
use crate::editor::placed::{self, add_cabinet};
use crate::editor::ObjectRef;
use crate::shell::view3d_panel;
use crate::tools::ToolId;
use plan_cabinets::{Cabinet, CabinetKind};
use plan_core::elevation_ref::{ElevationBase, ElevationEdge, ElevationRef};
use plan_core::geometry::Point;
use plan_core::schedules::ScheduleKind;
use plan_core::Id;
use plan_docs::schedule_kinds;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    sim.app.cx.refresh();
    sim
}

fn first_wall(sim: &Sim) -> Id {
    sim.wall_ids()[0]
}

fn cabinet(sim: &mut Sim) -> Id {
    let mut c = Cabinet::new(CabinetKind::Wall, 30.0);
    c.position = Point::new(100.0, 100.0);
    c.elevation = 54.0;
    add_cabinet(&mut sim.app.cx.project, 0, c).unwrap()
}

/// Draws every shared page of `s` once in a headless frame.
fn draw_all(s: &mut InfoSession) {
    let ctx = eframe::egui::Context::default();
    let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            for page in [
                Page::Components,
                Page::Info,
                Page::Label,
                Page::Schedule,
                Page::Manufacturer,
            ] {
                for placement in [Placement::Tab, Placement::Appended] {
                    object_info::draw_page(ui, s, page, placement);
                }
            }
        });
    });
}

#[test]
fn every_opted_in_object_takes_the_panels_its_kind_has() {
    let mut sim = house();
    let wall = first_wall(&sim);
    let opening = sim.app.cx.floor().openings[0].id;
    let cab = cabinet(&mut sim);
    let cx = &sim.app.cx;
    let names = |o: ObjectRef| -> Vec<String> {
        let s = InfoSession::for_object(cx, o).unwrap_or_else(|| panic!("{o:?} takes no panels"));
        let s = s.borrow();
        object_info::tabs(&s, &[])
            .into_iter()
            .map(|t| t.name)
            .collect()
    };
    let mut wall_tabs = names(ObjectRef::Wall(wall));
    assert_eq!(
        wall_tabs,
        ["Components", "Object Information", "Label", "Schedule"]
    );
    wall_tabs.clear();
    assert_eq!(
        names(ObjectRef::Opening(opening)),
        [
            "Components",
            "Object Information",
            "Label",
            "Schedule",
            "Manufacturer"
        ]
    );
    assert!(names(ObjectRef::Cabinet(cab)).contains(&"Manufacturer".to_string()));
    assert!(names(ObjectRef::Stair(9)).contains(&"Schedule".to_string()));
    assert!(names(ObjectRef::Device(9)).contains(&"Manufacturer".to_string()));
    assert!(names(ObjectRef::RoofPlane(9)).contains(&"Label".to_string()));
    // A dialog with a Label and a Schedule tab of its own keeps them: the
    // frame adds the rest under them instead of a second tab.
    let s = InfoSession::for_object(cx, ObjectRef::Opening(opening)).unwrap();
    let own = ["General", "Label", "Schedule"];
    let tabs: Vec<_> = object_info::tabs(&s.borrow(), &own)
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(tabs, ["Components", "Object Information", "Manufacturer"]);
    assert_eq!(
        object_info::appended(&s.borrow(), "Label"),
        Some(Page::Label)
    );
    assert_eq!(object_info::appended(&s.borrow(), "General"), None);
    // Every page draws for every placement without trouble.
    for o in [
        ObjectRef::Wall(wall),
        ObjectRef::Opening(opening),
        ObjectRef::Cabinet(cab),
    ] {
        draw_all(&mut InfoSession::for_object(cx, o).unwrap().borrow_mut());
    }
    // Dimensions take none.
    assert!(InfoSession::for_object(cx, ObjectRef::Dimension(1)).is_none());
}

#[test]
fn object_information_reaches_the_schedule_and_the_label_the_plan_in_one_undo_step() {
    let mut sim = house();
    let wall = first_wall(&sim);
    assert!(sim.open_spec(ObjectRef::Wall(wall)));
    sim.dialog_frame(false);
    let info = sim.app.spec.main_info().cloned().expect("panels armed");
    {
        let mut s = info.borrow_mut();
        s.draft.code = "EW-1".into();
        s.draft.comment = "Sheathe before framing inspection".into();
        s.pages.label = Some(plan_core::object_pages::LabelPage {
            specify: true,
            text: "%name% %code%".into(),
            offset_y: 12.0,
            ..Default::default()
        });
    }
    let steps = sim.app.cx.undo_depth();
    sim.ok();
    assert!(!sim.app.has_dialog());
    assert_eq!(sim.app.cx.undo_depth(), steps + 1, "one undo step");

    // The wall schedule shows the Code and Comment columns.
    let rows = schedule_kinds::entries(&sim.app.cx.project, ScheduleKind::Wall, None);
    let row = rows.iter().find(|e| e.id == wall).expect("the wall");
    assert_eq!(row.cell("code"), "EW-1");
    assert_eq!(row.cell("comment"), "Sheathe before framing inspection");

    // The label is on the plan, off the wall face and on its system layer.
    let ctx = common_pages::macro_context(&sim.app.cx.project, 0);
    let labels = common_pages::plan_labels(&sim.app.cx.project, 0, &ctx);
    let label = labels
        .iter()
        .find(|l| l.key == format!("wall:{wall}"))
        .expect("the wall's label");
    assert!(label.text.ends_with("EW-1"), "{}", label.text);
    assert_eq!(label.layer, "Walls, Labels");

    // One undo takes everything back; redo brings it again.
    sim.undo();
    let rows = schedule_kinds::entries(&sim.app.cx.project, ScheduleKind::Wall, None);
    assert_eq!(rows.iter().find(|e| e.id == wall).unwrap().cell("code"), "");
    assert!(common_pages::plan_labels(&sim.app.cx.project, 0, &ctx).is_empty());
    sim.redo();
    assert_eq!(
        common_pages::plan_labels(&sim.app.cx.project, 0, &ctx).len(),
        1
    );
}

#[test]
fn include_in_schedule_and_the_manufacturer_panel_are_stored_with_the_door() {
    let mut sim = house();
    let door = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.kind == plan_core::OpeningKind::Door)
        .unwrap()
        .id;
    let rows = |sim: &Sim| schedule_kinds::entries(&sim.app.cx.project, ScheduleKind::Door, None);
    assert!(rows(&sim).iter().any(|e| e.id == door));
    assert!(sim.open_spec(ObjectRef::Opening(door)));
    sim.dialog_frame(false);
    let info = sim.app.spec.main_info().cloned().expect("panels armed");
    {
        let mut s = info.borrow_mut();
        let mut sched = s.pages.schedule_or_default();
        sched.include = false;
        s.pages.schedule = Some(sched);
        let mut m = s.pages.manufacturer_or_default();
        m.name = "Therma-Tru".into();
        m.phone = "800-555-0100".into();
        s.pages.manufacturer = Some(m);
    }
    sim.ok();
    assert!(!rows(&sim).iter().any(|e| e.id == door), "left out");
    let stored = sim
        .app
        .cx
        .project
        .props
        .pages_of(&format!("door:{door}"))
        .unwrap();
    assert_eq!(stored.manufacturer_or_default().name, "Therma-Tru");
    sim.undo();
    assert!(rows(&sim).iter().any(|e| e.id == door));
    assert!(sim
        .app
        .cx
        .project
        .props
        .pages_of(&format!("door:{door}"))
        .is_none());
}

/// The highest point of the cabinet's meshes, inches above Z = 0.
fn cabinet_top(sim: &Sim) -> f32 {
    let eff = elevation_ref::effective_project(&sim.app.cx.project);
    let proj = eff.as_ref().unwrap_or(&sim.app.cx.project);
    view3d_panel::cabinet_meshes(proj)
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .fold(f32::MIN, f32::max)
}

#[test]
fn a_cabinet_measured_from_the_ceiling_follows_the_ceiling_height() {
    let mut sim = house();
    let cab = cabinet(&mut sim);
    let from_floor = cabinet_top(&sim);
    // The default reference leaves the cabinet where its typed height puts it.
    assert!(elevation_ref::effective_project(&sim.app.cx.project).is_none());

    // The typed number is -6: the top 6 in below the ceiling.
    {
        let mut c = placed::cabinet_by_id(sim.app.cx.floor(), cab).unwrap();
        c.elevation = -6.0;
        placed::apply_cabinet(&mut sim.app.cx, &c);
    }
    assert!(sim.open_spec(ObjectRef::Cabinet(cab)));
    sim.dialog_frame(false);
    let info = sim.app.spec.info_session().cloned().expect("panels armed");
    {
        let mut s = info.borrow_mut();
        assert!(s.takes_elevation());
        assert_eq!(s.elevation(), ElevationRef::default());
        // The top of the cabinet 6 in below the ceiling.
        s.set_elevation(ElevationRef::new(
            ElevationBase::FromCeiling,
            ElevationEdge::ToTop,
        ));
    }
    let steps = sim.app.cx.undo_depth();
    sim.ok();
    assert_eq!(sim.app.cx.undo_depth(), steps + 1, "one undo step");
    let ceiling = sim.app.cx.floor().ceiling_height as f32;
    assert!((cabinet_top(&sim) - (ceiling - 6.0)).abs() < 0.01);

    // Raising the ceiling raises the cabinet with it.
    sim.app.cx.project.floors[0].ceiling_height += 12.0;
    assert!((cabinet_top(&sim) - (ceiling + 6.0)).abs() < 0.01);

    // Undo returns to From Floor, where it was.
    sim.app.cx.project.floors[0].ceiling_height -= 12.0;
    sim.undo();
    assert!(elevation_ref::effective_project(&sim.app.cx.project).is_none());
    assert!(from_floor.is_finite());
}
