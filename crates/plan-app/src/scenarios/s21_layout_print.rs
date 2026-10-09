//! Scenario 21: the house goes to a layout and out to paper. New Layout,
//! Send to Layout (plan and camera boxes), a text box, page CAD that moves
//! with undo, Page Setup, layout layers, the Print dialog to PDF with tiling,
//! the Materials List with the Master List and the construction set (L-1..L-9,
//! L-33..L-37, PR-1..PR-6).

use super::{draw_shell, Sim};
use crate::dialogs::layout::{
    LayoutTarget, LeaderSpec, PageChoice, PageSetup, Placement, SendSource, SendSpec, SheetSizes,
    SnapshotSpec, TextBoxSpec,
};
use crate::shell::layout_window::{self as lw, LayoutCommand, LayoutView};
use crate::toolbar::Action;
use crate::tools::ToolId;
use plan_core::geometry::Point;
use plan_core::{Id, Project};
use plan_docs::{materials_total, MasterList, Scale, SheetSize};
use plan_layout::{
    AlignEdge, BoxSource, CustomSheetSize, PaperSize, PreviewItem, PrintColor, PrintOptions,
    PrintScale, SheetChoice, Spread,
};
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
    // A second New Layout asks for the name of a second layout file (the
    // first is parked once it is made) instead of replacing the layout.
    lw::deactivate();
    sim.action(Action::FileNewLayout);
    assert!(
        sim.app.cx.status.contains("already has a layout"),
        "{}",
        sim.app.cx.status
    );
    assert!(lw::is_active() && lw::dialog_open(), "the name dialog is up");
    assert_eq!(lw::load(&sim.app.cx.project).unwrap().pages.len(), 2);
    assert!(sim.app.cx.project.layout_files.is_empty());
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
    assert_eq!(
        setup.sheet,
        plan_layout::SheetChoice::Standard(SheetSize::ArchC)
    );
    let before = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    let wider = PageSetup {
        sheet: plan_layout::SheetChoice::Standard(SheetSize::ArchD),
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

// ---------------------------------------------------------------- round 14 --

/// Three small text boxes on the first printed page, selected together.
fn three_boxes(sim: &mut Sim, v: &mut LayoutView) -> [Id; 3] {
    let p = &mut sim.app.cx.project;
    let a = v.add_text_box_in(p, [1.0, 1.0, 3.0, 2.0]).unwrap();
    let b = v.add_text_box_in(p, [4.0, 5.0, 8.0, 7.0]).unwrap();
    let c = v.add_text_box_in(p, [2.0, 9.0, 3.0, 10.0]).unwrap();
    v.select_boxes(&[a, b, c]);
    [a, b, c]
}

fn rect_of(v: &LayoutView, id: Id) -> [f64; 4] {
    let b = v
        .current_page()
        .unwrap()
        .boxes
        .iter()
        .find(|b| b.id == id)
        .unwrap();
    let (a, c) = b.rect_in;
    [a.x.min(c.x), a.y.min(c.y), a.x.max(c.x), a.y.max(c.y)]
}

#[test]
fn page_specification_sets_number_title_flags_and_a_sheet_of_its_own() {
    let mut sim = house();
    let mut v = view(&mut sim);
    v.send(&mut sim.app.cx.project, &plan_spec(0), None)
        .unwrap();
    let mut spec = v.page_spec().expect("a page specification");
    assert_eq!((spec.number, spec.title.as_str()), (1, "Page 1"));
    assert_eq!(spec.sheet, None, "the page follows the layout's sheet");
    spec.title = "Presentation Plan".into();
    spec.number = 7;
    spec.sheet = Some(SheetChoice::Standard(SheetSize::ArchD));
    let at = v.page;
    assert_eq!(
        v.apply_page_spec(&mut sim.app.cx.project, at, &spec),
        Ok(true)
    );
    assert_eq!(v.undo_label(), Some("Page Specification"));
    let l = v.layout().unwrap();
    assert_eq!(l.pages[at].sheet_number(), "A-7");
    assert_eq!(l.page_sheet_inches(&l.pages[at]), (36.0, 24.0));
    assert_eq!(l.sheet_inches(), (24.0, 18.0), "the layout is untouched");
    let pdf = lw::print_bytes(l, &sim.app.cx.project, None);
    assert!(
        has(&pdf, "/MediaBox [0 0 2592 1728]"),
        "the page's own sheet"
    );
    // The dialog reads the page back the way it was set.
    let again = v.page_spec().unwrap();
    assert_eq!(again.sheet, Some(SheetChoice::Standard(SheetSize::ArchD)));
    assert!(!again.portrait);
    assert_eq!(
        v.apply_page_spec(&mut sim.app.cx.project, at, &again),
        Ok(false),
        "nothing changed"
    );
    // A number another page has is refused and nothing changes.
    v.add_page(&mut sim.app.cx.project, false);
    let here = v.page;
    let mut clash = v.page_spec().unwrap();
    clash.number = 7;
    assert_eq!(
        v.apply_page_spec(&mut sim.app.cx.project, here, &clash),
        Err("Another page already has that sheet number")
    );
    assert_ne!(v.layout().unwrap().pages[here].number, 7);
    // Portrait turns the page's sheet upright; None follows the layout again.
    let mut up = v.page_spec().unwrap();
    up.sheet = Some(SheetChoice::Standard(SheetSize::Tabloid));
    up.portrait = true;
    up.no_title_block = true;
    assert_eq!(
        v.apply_page_spec(&mut sim.app.cx.project, here, &up),
        Ok(true)
    );
    let l = v.layout().unwrap();
    assert_eq!(l.page_sheet_inches(&l.pages[here]), (11.0, 17.0));
    assert!(l.pages[here].no_title_block);
    assert_eq!(v.page_spec().unwrap(), up);
    up.sheet = None;
    assert_eq!(
        v.apply_page_spec(&mut sim.app.cx.project, here, &up),
        Ok(true)
    );
    assert_eq!(v.layout().unwrap().pages[here].size_override_in, None);
}

#[test]
fn customize_sheet_sizes_add_a_poster_size_that_page_setup_and_print_use() {
    let mut sim = house();
    let mut v = view(&mut sim);
    let poster = CustomSheetSize::new("Poster", 40.0, 30.0);
    let sizes = SheetSizes {
        custom: vec![poster.clone()],
        hidden: SheetSize::ALL
            .into_iter()
            .filter(|s| s.label().starts_with("ISO"))
            .collect(),
    };
    assert!(v.apply_sheet_sizes(&mut sim.app.cx.project, &sizes));
    assert_eq!(v.undo_label(), Some("Customize Sheet Sizes"));
    assert!(
        !v.apply_sheet_sizes(&mut sim.app.cx.project, &sizes),
        "the same answers change nothing"
    );
    let labels: Vec<String> = v
        .layout()
        .unwrap()
        .size_choices()
        .iter()
        .map(SheetChoice::label)
        .collect();
    assert!(
        labels.contains(&"Poster (30 x 40)".to_string()),
        "{labels:?}"
    );
    assert!(labels.contains(&"ARCH C (18 x 24)".to_string()));
    assert!(!labels.iter().any(|n| n.starts_with("ISO")), "{labels:?}");
    // Page Setup picks the poster size; the PDF page is 40 x 30 inches.
    let mut setup = v.page_setup().unwrap();
    setup.sheet = SheetChoice::Custom(poster.clone());
    assert!(v.apply_page_setup(&mut sim.app.cx.project, &setup));
    assert_eq!(v.layout().unwrap().sheet_inches(), (40.0, 30.0));
    let pdf = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    assert!(has(&pdf, "/MediaBox [0 0 2880 2160]"));
    // Boxes sent now pack into the new area.
    let id = v
        .send(&mut sim.app.cx.project, &plan_spec(0), None)
        .unwrap();
    let l = v.layout().unwrap();
    let hi = l.drawing_area().1;
    let r = l.pages[1]
        .boxes
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .bounds_in();
    assert!(r[2] <= hi.x + 1e-9 && r[3] <= hi.y + 1e-9);
    // Page Setup remembers the custom sheet as its choice.
    assert_eq!(v.page_setup().unwrap().sheet, SheetChoice::Custom(poster));
    // Undo puts the ARCH C sheet back.
    assert!(v.undo(&mut sim.app.cx.project).is_some());
    assert!(v.undo(&mut sim.app.cx.project).is_some());
    assert_eq!(v.layout().unwrap().sheet_inches(), (24.0, 18.0));
}

#[test]
fn boxes_align_spread_and_copy_as_single_undo_steps() {
    let mut sim = house();
    let mut v = view(&mut sim);
    let [a, b, c] = three_boxes(&mut sim, &mut v);
    assert_eq!(v.selection_ids(), vec![a, b, c]);
    let p = &mut sim.app.cx.project;
    assert_eq!(v.align_selected(p, AlignEdge::Left), 2);
    assert_eq!(v.undo_label(), Some("Align Left"));
    for id in [a, b, c] {
        assert_eq!(rect_of(&v, id)[0], 1.0, "box {id}");
    }
    // Spread bottom to top: a (1..2), b (5..7), c (9..10) get equal gaps.
    assert_eq!(v.distribute_selected(p, Spread::Vertical), 1);
    assert_eq!(v.undo_label(), Some("Distribute Boxes Vertically"));
    let (ra, rb, rc) = (rect_of(&v, a), rect_of(&v, b), rect_of(&v, c));
    assert!(((rb[1] - ra[3]) - (rc[1] - rb[3])).abs() < 1e-9);
    // One box lines up with the drawing area.
    v.select_boxes(&[a]);
    assert_eq!(v.align_selected(p, AlignEdge::Right), 1);
    let area = v.layout().unwrap().drawing_area();
    assert!((rect_of(&v, a)[2] - area.1.x).abs() < 1e-9);
    // Copy to a second page and duplicate on the same page.
    v.add_page(p, false);
    let second = v.current_page().unwrap().number;
    v.set_page(1);
    v.select_boxes(&[a, b, c]);
    assert_eq!(v.copy_selected_to(p, second), 3);
    assert_eq!(v.undo_label(), Some("Copy Layout Boxes"));
    assert_eq!(v.current_page().unwrap().number, second, "the copies show");
    assert_eq!(v.current_page().unwrap().boxes.len(), 3);
    let copies = v.selection_ids();
    assert_eq!(copies.len(), 3);
    assert!(copies.iter().all(|id| ![a, b, c].contains(id)));
    assert_eq!(v.duplicate_here(p), 3);
    assert_eq!(v.current_page().unwrap().boxes.len(), 6);
    // Ids stay unique across the layout.
    let mut ids: Vec<Id> = v
        .layout()
        .unwrap()
        .pages
        .iter()
        .flat_map(|pg| pg.boxes.iter().map(|b| b.id))
        .collect();
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n);
    // Each is one undo step.
    assert_eq!(v.undo(p).as_deref(), Some("Copy Layout Boxes"));
    assert_eq!(v.current_page().unwrap().boxes.len(), 3);
}

#[test]
fn a_plan_holds_several_layout_files_and_send_to_picks_the_file() {
    let mut sim = house();
    let mut v = view(&mut sim);
    let p = &mut sim.app.cx.project;
    v.send(p, &plan_spec(0), None).unwrap();
    let first = lw::layout_names(p)[0].clone();
    assert_eq!(first, "Maple Court Residence Layout");
    // A second file opens and parks the first.
    assert!(v.new_layout_file(p, "Presentation Set", None));
    assert_eq!(v.undo_label(), Some("New Layout File"));
    assert_eq!(
        lw::layout_names(p),
        vec!["Presentation Set".to_string(), first.clone()]
    );
    assert_eq!(p.layout_files.len(), 1);
    assert!(v.layout().unwrap().pages[1].boxes.is_empty());
    assert!(!v.new_layout_file(p, " presentation set ", None), "taken");
    assert!(!v.new_layout_file(p, "  ", None), "empty");
    // Send to the first file by name: it opens, the box lands on a new page,
    // and the whole action is one undo step.
    let spec = SendSpec {
        page: PageChoice::New,
        ..plan_spec(0)
    };
    let id = v
        .send_to(p, &spec, &LayoutTarget::Existing(first.clone()), None, None)
        .unwrap();
    assert_eq!(lw::layout_names(p)[0], first);
    let l = v.layout().unwrap();
    assert!(l
        .pages
        .iter()
        .flat_map(|pg| pg.boxes.iter())
        .any(|b| b.id == id));
    assert_eq!(v.undo_label(), Some("Send to Layout"));
    assert_eq!(v.undo(p).as_deref(), Some("Send to Layout"));
    assert_eq!(lw::layout_names(p)[0], "Presentation Set");
    assert_eq!(v.layout().unwrap().name, "Presentation Set");
    // The parked file and the open one survive a save and open.
    let back = Project::from_json(&p.to_json().unwrap()).unwrap();
    assert_eq!(lw::layout_names(&back), lw::layout_names(p));
    // A new file made by Send to Layout.
    let made = v
        .send_to(
            p,
            &spec,
            &LayoutTarget::New("Permit Set".into()),
            None,
            None,
        )
        .unwrap();
    assert_eq!(lw::layout_names(p)[0], "Permit Set");
    assert!(v
        .layout()
        .unwrap()
        .pages
        .iter()
        .flat_map(|pg| pg.boxes.iter())
        .any(|b| b.id == made));
    assert_eq!(p.layout_files.len(), 2);
    // A name that is taken is refused and nothing changes.
    let before = lw::layout_names(p);
    assert!(v
        .send_to(
            p,
            &spec,
            &LayoutTarget::New("Permit Set".into()),
            None,
            None
        )
        .is_err());
    assert_eq!(lw::layout_names(p), before);
    // Switching is an undo step too.
    assert!(v.switch_layout(p, 1));
    assert_eq!(v.undo_label(), Some("Open Layout"));
    assert_ne!(lw::layout_names(p)[0], "Permit Set");
    assert!(!v.switch_layout(p, 0) && !v.switch_layout(p, 9));
}

#[test]
fn print_preview_draws_the_page_in_the_chosen_colour_mode() {
    let mut sim = house();
    let mut v = view(&mut sim);
    v.send(&mut sim.app.cx.project, &plan_spec(0), None)
        .unwrap();
    let opts = PrintOptions {
        paper: PaperSize::Standard(SheetSize::ArchC),
        scale: PrintScale::Actual,
        margin_in: 0.0,
        color: PrintColor::BlackWhite,
        ..PrintOptions::default()
    };
    let pages = v.print_preview_pages(&sim.app.cx.project, &opts);
    assert_eq!(pages.len(), 1);
    let page = &pages[0];
    assert_eq!(page.caption, "A-1 Page 1");
    let ink: Vec<[u8; 3]> = page
        .items
        .iter()
        .filter_map(|i| match i {
            PreviewItem::Stroke { color, .. }
            | PreviewItem::Fill { color, .. }
            | PreviewItem::Text { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert!(ink.len() > 50, "the plan and the title block draw");
    assert!(ink.iter().all(|c| *c == [0, 0, 0] || *c == [255, 255, 255]));
    // Colour keeps its colours; the same options print a PDF page of the
    // same paper.
    let colour = v.print_preview_pages(
        &sim.app.cx.project,
        &PrintOptions {
            color: PrintColor::Color,
            ..opts.clone()
        },
    );
    assert!(colour[0]
        .items
        .iter()
        .any(|i| matches!(i, PreviewItem::Fill { color, .. } if color[0] != color[2])));
    let pdf = lw::print_with(v.layout().unwrap(), &sim.app.cx.project, &opts);
    assert!(has(&pdf, "/MediaBox [0 0 1728 1296]"));
    assert_eq!(page.paper_in, (24.0, 18.0));
}

#[test]
fn schedule_tables_export_to_csv_and_excel() {
    let mut sim = house();
    let mut v = view(&mut sim);
    let p = &mut sim.app.cx.project;
    let door = v
        .add_source_box(
            p,
            "Add Schedule",
            BoxSource::Schedule {
                kind: plan_layout::ScheduleKind::Door,
            },
        )
        .unwrap();
    let window = v
        .add_source_box(
            p,
            "Add Schedule",
            BoxSource::Schedule {
                kind: plan_layout::ScheduleKind::Window,
            },
        )
        .unwrap();
    // The selected box alone.
    v.select_boxes(&[door]);
    let (name, csv) = v.tables_file(p, false).expect("a table");
    assert_eq!(name, "Door Schedule");
    let text = String::from_utf8(csv).unwrap();
    assert!(text.lines().count() >= 2, "{text}");
    let (_, xlsx) = v.tables_file(p, true).unwrap();
    assert!(xlsx.starts_with(b"PK"), "a zip");
    let parts = plan_library::archive::read_zip(&xlsx).unwrap();
    assert!(parts.iter().any(|(n, _)| n == "xl/worksheets/sheet1.xml"));
    assert!(!parts.iter().any(|(n, _)| n == "xl/worksheets/sheet2.xml"));
    // With nothing selected every table on the page goes, a sheet each.
    v.select_boxes(&[]);
    let (name, xlsx) = v.tables_file(p, true).unwrap();
    assert_eq!(name, "Page 1");
    let parts = plan_library::archive::read_zip(&xlsx).unwrap();
    assert!(parts.iter().any(|(n, _)| n == "xl/worksheets/sheet2.xml"));
    let wb = parts
        .iter()
        .find(|(n, _)| n == "xl/workbook.xml")
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .unwrap();
    assert!(
        wb.contains("Door Schedule") && wb.contains("Window Schedule"),
        "{wb}"
    );
    // Several tables in a CSV: each under its title.
    let (_, csv) = v.tables_file(p, false).unwrap();
    let text = String::from_utf8(csv).unwrap();
    assert!(text.contains("Door Schedule\n") && text.contains("Window Schedule\n"));
    // A page without tables has nothing to export.
    v.add_page(p, false);
    assert!(v.tables_file(p, true).is_none());
    let _ = window;
}

#[test]
fn the_construction_set_prints_the_plans_other_schedules_and_door_boxes_follow_the_spec() {
    use plan_core::schedules::{Schedule, ScheduleKind, ScheduleLayer};
    let mut sim = house();
    let mut layer = ScheduleLayer::default();
    layer.add(Schedule::new(ScheduleKind::Cabinet, Point::new(0.0, -80.0)));
    let mut door = Schedule::new(ScheduleKind::Door, Point::new(100.0, -80.0));
    let hidden = door
        .columns
        .iter()
        .find(|c| c.field == "swing")
        .map(|c| c.title.clone())
        .expect("a swing column");
    layer.add(door.clone());
    layer.store(&mut sim.app.cx.project.floors[0]);
    sim.action(Action::FileNewLayout);
    let msg = lw::install_construction_set(&mut sim.app.cx);
    assert!(msg.starts_with("Added the construction set"), "{msg}");
    let layout = lw::load(&sim.app.cx.project).unwrap();
    let sched = layout
        .pages
        .iter()
        .find(|p| p.title == "Schedules")
        .unwrap();
    assert!(
        sched
            .boxes
            .iter()
            .any(|b| matches!(b.source, BoxSource::PlacedSchedule { .. })),
        "the cabinet schedule follows the plan"
    );
    let pdf = lw::print_bytes(&layout, &sim.app.cx.project, None);
    assert!(has(&pdf, "(Cabinet Schedule)"));
    assert!(
        has(&pdf, &format!("({hidden})")),
        "the door box shows Swing"
    );
    // Hiding the column in the plan's door schedule hides it on the sheet.
    for c in &mut door.columns {
        if c.field == "swing" {
            c.visible = false;
        }
    }
    let mut layer = ScheduleLayer::load(&sim.app.cx.project.floors[0]);
    layer.schedules.retain(|s| s.kind != ScheduleKind::Door);
    layer.add(door);
    layer.store(&mut sim.app.cx.project.floors[0]);
    let pdf = lw::print_bytes(&layout, &sim.app.cx.project, None);
    assert!(!has(&pdf, &format!("({hidden})")), "{hidden} is gone");
    assert!(has(&pdf, "(Door Schedule)"));
}

#[test]
fn a_picture_of_the_3d_view_goes_to_the_layout_and_into_the_pdf() {
    let mut sim = house();
    let mut v = view(&mut sim);
    let p = &mut sim.app.cx.project;
    let view3d = crate::shell::view3d_panel::Snapshot3d {
        scene: lw::view_scene(p),
        camera: plan_render::Camera::from_plan(Point::new(240.0, -300.0), 90.0, 66.0, 50.0),
    };
    let snap = SnapshotSpec {
        width_in: 4.0,
        dpi: 20,
        samples: 1,
    };
    let id = v
        .send_to(
            p,
            &plan_spec(0),
            &LayoutTarget::Current,
            Some((snap, view3d)),
            None,
        )
        .unwrap();
    assert_eq!(v.undo_label(), Some("Send 3D View to Layout"));
    let l = v.layout().unwrap();
    let b = l.pages[1].boxes.iter().find(|b| b.id == id).unwrap();
    let BoxSource::ImageData {
        width,
        height,
        rgba,
    } = &b.source
    else {
        panic!("a picture box, not {:?}", b.source);
    };
    assert_eq!((*width, *height), (80, 60), "4 x 3 inches at 20 dpi");
    assert_eq!(rgba.len(), 80 * 60 * 4);
    assert!(rgba.iter().any(|v| *v != 0), "something was drawn");
    assert_eq!(b.size_in(), (4.0, 3.0));
    assert_eq!(b.label.as_deref(), Some("3D VIEW"));
    let pdf = lw::print_bytes(l, p, None);
    assert!(has(&pdf, "/Subtype/Image"), "the picture is in the PDF");
    // The picture saves with the plan as text, not as a list of numbers.
    let layout_json = serde_json::to_string(l).unwrap();
    assert!(
        layout_json.len() < rgba.len() * 2,
        "{} bytes of layout JSON for {} bytes of pixels",
        layout_json.len(),
        rgba.len()
    );
    let json = p.to_json().unwrap();
    let back = Project::from_json(&json).unwrap();
    assert_eq!(lw::load(&back).unwrap(), *l);
    // An empty view has nothing to send.
    let empty = crate::shell::view3d_panel::Snapshot3d {
        scene: plan_3d::Scene::default(),
        camera: plan_render::Camera::from_plan(Point::ZERO, 0.0, 66.0, 50.0),
    };
    assert!(v
        .send_to(
            p,
            &plan_spec(0),
            &LayoutTarget::Current,
            Some((snap, empty)),
            None
        )
        .is_err());
}

#[test]
fn layout_drawings_use_the_3d_views_scene() {
    let mut sim = house();
    let p = &mut sim.app.cx.project;
    let plain = plan_3d::build_scene(p).meshes.len();
    let stair = plan_stairs::Stair::new(
        p.alloc_id(),
        Point::new(100.0, 100.0),
        0.0,
        plan_stairs::StairParams::default(),
    );
    p.floors[0].set_stairs(&[stair]).unwrap();
    let with = lw::view_scene(p).meshes.len();
    assert!(
        with > plain,
        "the stair is in the layout scene: {with} vs {plain}"
    );
    // The render context the layout prints with builds its scene that way:
    // an elevation box sized from it is at least as large as from walls alone.
    let rcx = lw::render_context(p);
    let src = BoxSource::Elevation {
        dir: plan_elevation::ViewDir::Front,
    };
    let (w, h) = plan_layout::source_size_in(&src, Scale::QuarterInch, &rcx);
    assert!(w > 0.0 && h > 0.0);
}
