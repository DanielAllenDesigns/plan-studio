//! Scenario 21: the house goes to a layout and out to paper. New Layout,
//! Send to Layout (plan and camera boxes), a text box, page CAD that moves
//! with undo, Page Setup, layout layers, the Print dialog to PDF with tiling,
//! the Materials List with the Master List and the construction set (L-1..L-9,
//! L-33..L-37, PR-1..PR-6).

use super::{draw_shell, Sim};
use crate::dialogs::layout::{
    LeaderSpec, PageChoice, PageSetup, Placement, SendSource, SendSpec, TextBoxSpec,
};
use crate::shell::layout_window::{self as lw, LayoutCommand, LayoutView};
use crate::toolbar::Action;
use crate::tools::ToolId;
use plan_core::geometry::Point;
use plan_core::{Id, Project};
use plan_docs::{materials_total, MasterList, Scale, SheetSize};
use plan_layout::{BoxSource, PaperSize, PrintOptions, PrintScale};
use std::sync::Once;

const W: f64 = 480.0;
const H: f64 = 360.0;

/// Points `$HOME` at an empty temp folder for the whole test process, so no
/// scenario reads or writes `~/.plan-studio`. Idempotent.
pub(super) fn isolate_home() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join(format!("plan-studio-qa-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp home");
        std::env::set_var("HOME", &dir);
        std::env::remove_var("USERPROFILE");
    });
}

fn house() -> Sim {
    isolate_home();
    lw::use_memory_master_list(MasterList::default());
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.app.cx.project.name = "Maple Court Residence".into();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    sim
}

fn view(sim: &mut Sim) -> LayoutView {
    let mut v = LayoutView::default();
    assert!(v.create(&mut sim.app.cx.project, None));
    v
}

fn plan_spec(floor: usize) -> SendSpec {
    SendSpec {
        source: SendSource::Plan {
            floor,
            layer_set: "Default Set".into(),
        },
        page: PageChoice::Existing(1),
        scale: None,
        placement: Placement::FirstFree,
    }
}

fn pages(pdf: &[u8]) -> usize {
    let text = String::from_utf8_lossy(pdf);
    text.match_indices("/Type /Page")
        .filter(|(i, _)| !text[i + "/Type /Page".len()..].starts_with('s'))
        .count()
}

fn has(pdf: &[u8], needle: &str) -> bool {
    let text: String = pdf.iter().map(|&b| b as char).collect();
    text.contains(needle)
}

#[test]
fn new_layout_makes_a_template_page_and_page_one_and_a_second_one_reopens_it() {
    let mut sim = house();
    assert!(sim.app.cx.project.layout.is_none());
    sim.action(Action::FileNewLayout);
    assert!(lw::is_active(), "the layout view shows");
    let layout = lw::load(&sim.app.cx.project).expect("the plan has a layout now");
    assert_eq!(layout.pages.len(), 2);
    assert!(layout.pages[0].template_page);
    assert_eq!(layout.pages[1].number, 1);
    assert!(
        sim.app.cx.status.starts_with("New layout"),
        "{}",
        sim.app.cx.status
    );
    // The plan's Drawing Sheet follows the layout's sheet.
    assert_eq!(sim.app.cx.sheet.size, layout.sheet);
    // A second New Layout opens the same one instead of making another.
    lw::deactivate();
    sim.action(Action::FileNewLayout);
    assert!(
        sim.app.cx.status.contains("already has a layout"),
        "{}",
        sim.app.cx.status
    );
    assert_eq!(lw::load(&sim.app.cx.project).unwrap().pages.len(), 2);
    // Plan edits and layout edits share one undo stack: the layout step undoes.
    assert!(sim.app.cx.can_undo());
}

#[test]
fn send_to_layout_puts_the_plan_and_a_camera_on_pages_and_undo_takes_them_off() {
    let mut sim = house();
    // A full camera in the Project Browser.
    sim.tool(ToolId::CameraVariant(
        crate::tools::camera::CameraVariant::FullCamera,
    ));
    sim.drag((240.0, -80.0), (240.0, 180.0));
    sim.tool(ToolId::Select);
    let camera = sim
        .app
        .cx
        .project
        .cameras
        .first()
        .map(|c| c.id)
        .expect("the camera tool made a camera");
    let mut v = view(&mut sim);
    let plan = v
        .send(&mut sim.app.cx.project, &plan_spec(0), None)
        .unwrap();
    let l = v.layout().unwrap();
    let b = l.pages[1].boxes.iter().find(|b| b.id == plan).unwrap();
    assert!(matches!(b.source, BoxSource::PlanView { floor: 0, .. }));
    assert_eq!(b.scale, Scale::QuarterInch, "a 40 x 30 plan at 1/4\"");
    assert_eq!(v.undo_label(), Some("Send to Layout"));

    lw::set_perspective_size(64, 48, 1);
    let cam_spec = SendSpec {
        source: SendSource::Perspective {
            id: camera,
            name: "Camera 1".into(),
        },
        page: PageChoice::New,
        scale: None,
        placement: Placement::Centered,
    };
    let cam = v.send(&mut sim.app.cx.project, &cam_spec, None).unwrap();
    lw::clear_perspective_size();
    let l = v.layout().unwrap();
    assert_eq!(l.pages.len(), 3, "the camera got its own new page");
    assert!(matches!(
        l.pages[2]
            .boxes
            .iter()
            .find(|b| b.id == cam)
            .unwrap()
            .source,
        BoxSource::Perspective { .. }
    ));
    // Undo twice: the camera box (with its new page), then the plan box.
    assert_eq!(
        v.undo(&mut sim.app.cx.project).as_deref(),
        Some("Send to Layout")
    );
    assert_eq!(
        v.layout().unwrap().pages.len(),
        2,
        "the new page goes with it"
    );
    assert_eq!(v.layout().unwrap().pages[1].boxes.len(), 1);
    assert_eq!(
        v.undo(&mut sim.app.cx.project).as_deref(),
        Some("Send to Layout")
    );
    assert!(v.layout().unwrap().pages[1].boxes.is_empty());
    let _ = Id::default();
}

#[test]
fn a_text_box_is_drawn_edited_and_printed() {
    let mut sim = house();
    let mut v = view(&mut sim);
    let id = v
        .add_text_box_in(&mut sim.app.cx.project, [2.0, 2.0, 8.0, 4.0])
        .expect("a text box");
    assert_eq!(v.undo_label(), Some("Add Text Box"));
    let spec = TextBoxSpec {
        id,
        text: "GENERAL NOTES".into(),
        height_pt: 14.0,
        align: plan_layout::TextAlign::Center,
        bold: true,
        fit: plan_layout::TextFit::Wrap,
    };
    assert!(v.edit_text_box(&mut sim.app.cx.project, &spec));
    assert_eq!(v.undo_label(), Some("Edit Text Box"));
    let pdf = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    assert!(
        has(&pdf, "(GENERAL NOTES)"),
        "the text is on the printed page"
    );
    // The same spec again changes nothing and adds no step.
    assert!(!v.edit_text_box(&mut sim.app.cx.project, &spec));
    assert_eq!(
        v.undo(&mut sim.app.cx.project).as_deref(),
        Some("Edit Text Box")
    );
    let pdf = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    assert!(!has(&pdf, "(GENERAL NOTES)"));
}

#[test]
fn page_cad_lines_leaders_and_clouds_move_with_undo_and_redo() {
    let mut sim = house();
    let mut v = view(&mut sim);
    let line = v
        .add_cad_line(
            &mut sim.app.cx.project,
            Point::new(2.0, 2.0),
            Point::new(6.0, 2.0),
        )
        .unwrap();
    let leader = LeaderSpec {
        id: None,
        tip: Point::new(4.0, 6.0),
        elbow: Point::new(6.0, 8.0),
        bends: Vec::new(),
        text: "SEE DETAIL 3".into(),
        height_in: 0.125,
        arrow: true,
    };
    assert!(v.apply_leader(&mut sim.app.cx.project, &leader));
    let leader_id = v.layout().unwrap().pages[1].leaders[0].id;
    let cloud = crate::dialogs::layout::CloudSpec {
        id: None,
        rect: (Point::new(8.0, 8.0), Point::new(11.0, 10.0)),
        revision: "1".into(),
    };
    assert!(v.apply_cloud(&mut sim.app.cx.project, &cloud));
    let cloud_id = v.layout().unwrap().pages[1].clouds[0].id;

    let at = |v: &LayoutView| {
        let p = &v.layout().unwrap().pages[1];
        (
            match p.cad.iter().find(|c| c.id == line).unwrap().item {
                plan_core::CadItem::Line { a, .. } => a,
                _ => unreachable!(),
            },
            p.leaders.iter().find(|l| l.id == leader_id).unwrap().tip,
            p.clouds.iter().find(|c| c.id == cloud_id).unwrap().rect.0,
        )
    };
    let before = at(&v);
    for id in [line, leader_id, cloud_id] {
        assert!(v.nudge_annotation(&mut sim.app.cx.project, id, 1.0, 0.5));
        assert_eq!(v.undo_label(), Some("Move Layout Drawing"));
    }
    let moved = at(&v);
    assert!(moved.0.dist(before.0 + Point::new(1.0, 0.5)) < 1e-9);
    assert!(moved.1.dist(before.1 + Point::new(1.0, 0.5)) < 1e-9);
    assert!(moved.2.dist(before.2 + Point::new(1.0, 0.5)) < 1e-9);
    // Undo three times restores all three, redo brings them back.
    for _ in 0..3 {
        assert_eq!(
            v.undo(&mut sim.app.cx.project).as_deref(),
            Some("Move Layout Drawing")
        );
    }
    assert_eq!(at(&v), before);
    for _ in 0..3 {
        assert_eq!(
            v.redo(&mut sim.app.cx.project).as_deref(),
            Some("Move Layout Drawing")
        );
    }
    assert_eq!(at(&v), moved);
    // The project holds the moves, so a save and reopen keeps them.
    let back = Project::from_json(&sim.app.cx.project.to_json().unwrap()).unwrap();
    assert_eq!(lw::load(&back).unwrap().pages[1].leaders[0].tip, moved.1);
}

#[test]
fn layout_layers_hide_the_annotations_on_the_printed_page() {
    let mut sim = house();
    let mut v = view(&mut sim);
    v.apply_leader(
        &mut sim.app.cx.project,
        &LeaderSpec {
            id: None,
            tip: Point::new(4.0, 6.0),
            elbow: Point::new(6.0, 8.0),
            bends: Vec::new(),
            text: "SEE DETAIL 3".into(),
            height_in: 0.125,
            arrow: true,
        },
    );
    v.add_cad_line(
        &mut sim.app.cx.project,
        Point::new(2.0, 2.0),
        Point::new(6.0, 2.0),
    );
    let print = |v: &LayoutView, p: &Project| lw::print_bytes(v.layout().unwrap(), p, None);
    let shown = print(&v, &sim.app.cx.project);
    assert!(has(&shown, "(SEE DETAIL 3)"));
    let mut layers = v.layout().unwrap().layers.clone();
    layers
        .layers
        .iter_mut()
        .find(|l| l.name == plan_layout::LAYER_TEXT)
        .unwrap()
        .display = false;
    assert!(v.apply_layers(&mut sim.app.cx.project, &layers));
    assert_eq!(v.undo_label(), Some("Layout Layer Display"));
    let hidden = print(&v, &sim.app.cx.project);
    assert!(!has(&hidden, "(SEE DETAIL 3)"), "the Text layer is off");
    assert!(hidden.len() < shown.len());
    assert_eq!(
        v.undo(&mut sim.app.cx.project).as_deref(),
        Some("Layout Layer Display")
    );
    assert!(has(&print(&v, &sim.app.cx.project), "(SEE DETAIL 3)"));
}

#[test]
fn page_setup_changes_the_sheet_and_the_printed_page_size() {
    let mut sim = house();
    let mut v = view(&mut sim);
    let setup = v.page_setup().expect("a page setup");
    assert_eq!(setup.sheet, SheetSize::ArchC);
    let before = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    let wider = PageSetup {
        sheet: SheetSize::ArchD,
        margins_in: 0.75,
        ..setup.clone()
    };
    assert!(v.apply_page_setup(&mut sim.app.cx.project, &wider));
    assert_eq!(v.layout().unwrap().sheet, SheetSize::ArchD);
    let after = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    assert_ne!(before, after);
    let (w, h) = SheetSize::ArchD.inches();
    let media = format!("{} {}", (w * 72.0).round(), (h * 72.0).round());
    assert!(has(&after, &media), "MediaBox {media} in the PDF");
    // Page Setup is one undo step; the same setup again changes nothing.
    assert!(!v.apply_page_setup(&mut sim.app.cx.project, &wider));
    assert!(v.undo(&mut sim.app.cx.project).is_some());
    assert_eq!(v.layout().unwrap().sheet, SheetSize::ArchC);
}

#[test]
fn the_print_dialog_to_pdf_counts_one_page_fitted_and_many_tiled() {
    let mut sim = house();
    let mut v = view(&mut sim);
    v.send(&mut sim.app.cx.project, &plan_spec(0), None)
        .unwrap();
    v.add_page(&mut sim.app.cx.project, false);
    let layout = v.layout().unwrap().clone();
    let printed = layout.content_pages().len();
    assert_eq!(printed, 2);

    let fit = PrintOptions {
        paper: PaperSize::Standard(SheetSize::Letter),
        landscape: true,
        scale: PrintScale::Fit,
        tiling: false,
        ..PrintOptions::default()
    };
    let one_each = lw::print_with(&layout, &sim.app.cx.project, &fit);
    assert!(one_each.starts_with(b"%PDF"));
    assert_eq!(
        pages(&one_each),
        printed,
        "each sheet fitted on one letter page"
    );

    // Tiled at 100%: an 18 x 24 sheet needs several letter pages.
    let tiled = PrintOptions {
        tiling: true,
        scale: PrintScale::Actual,
        ..fit.clone()
    };
    let grid = plan_layout::tile_grid(
        SheetSize::ArchC.inches(),
        1.0,
        (11.0 - 0.5, 8.5 - 0.5),
        tiled.overlap_in,
    );
    let per_sheet = grid.cols * grid.rows;
    assert!(per_sheet >= 6, "{}x{} tiles", grid.cols, grid.rows);
    let many = lw::print_with(&layout, &sim.app.cx.project, &tiled);
    assert_eq!(pages(&many), per_sheet * printed);
    // A page range prints just that sheet's tiles.
    let range = PrintOptions {
        range: Some((1, 1)),
        ..tiled
    };
    let first = lw::print_with(&layout, &sim.app.cx.project, &range);
    assert_eq!(pages(&first), per_sheet);
}

#[test]
fn the_materials_list_totals_follow_the_master_list_prices_and_waste() {
    let mut sim = house();
    let plain = lw::load_master_list();
    let lines = crate::dialogs::build_tools::materials_for(&sim.app.cx);
    assert!(lines.len() >= 5);
    assert!(
        lines.iter().all(|l| l.price.is_none()),
        "an unpriced master list"
    );
    assert_eq!(materials_total(&lines), 0.0);

    // Price every row at $2 a unit and drop the waste.
    let mut list = MasterList::without_waste();
    for l in &lines {
        list.set_price(&l.key, &l.unit, 2.0);
    }
    lw::use_memory_master_list(list);
    let priced = crate::dialogs::build_tools::materials_for(&sim.app.cx);
    let net: f64 = priced.iter().map(|l| l.quantity).sum();
    assert!((materials_total(&priced) - 2.0 * net).abs() < 0.01 * net.max(1.0));
    assert!(priced.iter().all(|l| l.unit_price == Some(2.0)));

    // Waste raises the counts to buy (10% on framing here).
    let mut waste = MasterList::without_waste();
    waste.waste.insert("Framing".into(), 10.0);
    lw::use_memory_master_list(waste);
    let wasteful = crate::dialogs::build_tools::materials_for(&sim.app.cx);
    let framing = |ls: &[plan_docs::MaterialLine]| -> f64 {
        ls.iter()
            .filter(|l| l.category == "Framing")
            .map(|l| l.quantity)
            .sum()
    };
    assert!(
        framing(&wasteful) > framing(&priced),
        "{} vs {}",
        framing(&wasteful),
        framing(&priced)
    );
    let _ = plain;

    // The list goes to the layout as a table box with the same rows.
    let msg = lw::send_materials(&mut sim.app.cx, None, None);
    assert_eq!(msg, "Sent the Materials List to the layout");
    let layout = lw::load(&sim.app.cx.project).unwrap();
    assert!(layout
        .content_pages()
        .iter()
        .flat_map(|p| p.boxes.iter())
        .any(|b| matches!(b.source, BoxSource::Materials { .. })));
}

#[test]
fn the_construction_set_installs_ten_pages_with_a_sheet_index() {
    let mut sim = house();
    sim.action(Action::FileNewLayout);
    let msg = lw::install_construction_set(&mut sim.app.cx);
    assert!(msg.starts_with("Added the construction set"), "{msg}");
    let layout = lw::load(&sim.app.cx.project).unwrap();
    let titles: Vec<&str> = layout
        .content_pages()
        .iter()
        .map(|p| p.title.as_str())
        .collect();
    assert_eq!(titles.len(), 10, "{titles:?}");
    for want in ["Cover", "Site Plan", "Details", "Schedules"] {
        assert!(titles.contains(&want), "{titles:?}");
    }
    let cover = layout.pages.iter().find(|p| p.title == "Cover").unwrap();
    assert!(cover
        .boxes
        .iter()
        .any(|b| b.source == BoxSource::SheetIndex));
    let pdf = lw::print_bytes(&layout, &sim.app.cx.project, None);
    assert!(has(&pdf, "(SHEET INDEX)") && has(&pdf, "(SITE PLAN)"));
    assert_eq!(pages(&pdf), 10);
    // One undo step removes the whole set.
    assert_eq!(sim.app.cx.undo_label(), Some("Create Construction Set"));
    let _ = LayoutCommand::ShowPlan;
}
