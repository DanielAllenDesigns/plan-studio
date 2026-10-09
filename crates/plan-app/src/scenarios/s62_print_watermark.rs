//! Scenario 62 (round 16, brief 03): the Print dialog and Chief's Print
//! Source, Fit to Paper, To Scale and Check Plot; Drawing Sheet Setup for
//! each kind of view with the two-part Drawing Scale that Send to Layout
//! starts from; Scale to Fit and Center Sheet; program-wide Customize Sheet
//! Sizes; the Watermark on screen, in Print Preview and in the PDF (L-19,
//! L-53, L-192, L-199..L-227).

use super::s21_layout_print::isolate_home;
use super::{draw_shell, flatten, Sim};
use crate::dialogs::drawing_sheet as ds;
use crate::dialogs::print::{PrintDialog, PrintSource};
use crate::dialogs::watermark as wm;
use crate::shell::layout_window::{self as lw, LayoutView};
use crate::toolbar::{Action, ViewFlag};
use eframe::egui;
use plan_core::drawing_sheet::{DrawingScale, SheetDims, ViewSheet, ViewType};
use plan_core::watermark::{plan_key, WatermarkLayout};
use plan_docs::{MasterList, Scale, SheetSize};
use plan_layout::{
    CustomSheetSize, Layout, PaperSize, PreviewItem, PrintColor, PrintOptions, PrintScale,
    SheetSizeFile,
};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    isolate_home();
    lw::use_memory_master_list(MasterList::default());
    plan_layout::use_sheet_sizes(SheetSizeFile::default(), None);
    crate::dialogs::print::forget_remembered_settings();
    ds::set_context(ds::PrintViewContext::default());
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.app.cx.project.name = "Maple Court Residence".into();
    sim.app.cx.refresh();
    sim
}

/// A layout of `printed` ARCH D sheets, each with the plan on it.
fn arch_d_layout(sim: &mut Sim, printed: usize) -> Layout {
    let mut v = LayoutView::default();
    assert!(v.create(&mut sim.app.cx.project, None));
    for _ in 1..printed {
        v.add_page(&mut sim.app.cx.project, false);
    }
    let mut l = v.layout().unwrap().clone();
    l.sheet = SheetSize::ArchD;
    assert_eq!(l.sheet_inches(), (36.0, 24.0));
    assert_eq!(l.content_pages().len(), printed);
    l
}

fn pages(pdf: &[u8]) -> usize {
    let text = String::from_utf8_lossy(pdf);
    text.match_indices("/Type /Page")
        .filter(|(i, _)| !text[i + "/Type /Page".len()..].starts_with('s'))
        .count()
}

fn has(pdf: &[u8], needle: &str) -> bool {
    String::from_utf8_lossy(pdf).contains(needle)
}

fn count(pdf: &[u8], needle: &str) -> usize {
    String::from_utf8_lossy(pdf).matches(needle).count()
}

fn media_boxes(pdf: &[u8]) -> Vec<String> {
    let t = String::from_utf8_lossy(pdf).into_owned();
    t.match_indices("/MediaBox [")
        .map(|(i, _)| t[i + 11..].split(']').next().unwrap_or("").to_string())
        .collect()
}

fn marks(pages: &[plan_layout::PreviewPage]) -> usize {
    pages
        .iter()
        .flat_map(|p| p.items.iter())
        .filter(|i| matches!(i, PreviewItem::Mark(_)))
        .count()
}

fn full_size_on(paper: SheetSize) -> PrintOptions {
    PrintOptions {
        paper: PaperSize::Standard(paper),
        landscape: true,
        scale: PrintScale::Actual,
        margin_in: 0.0,
        ..PrintOptions::default()
    }
}

// ------------------------------------------------------------ watermark --

#[test]
fn view_watermark_is_per_saved_view_and_shows_in_the_preview_and_the_pdf_when_included() {
    let mut sim = house();
    let spec = &mut sim.app.cx.project.print_setup.watermark.spec;
    spec.text = "DRAFT".into();
    spec.layout = WatermarkLayout::Tile;
    spec.marks_per_row = 2;
    spec.marks_per_column = 2;
    let view = sim.app.cx.project.active_plan_view.clone();
    sim.action(Action::Custom(wm::TOGGLE));
    assert!(sim
        .app
        .cx
        .project
        .print_setup
        .watermark
        .is_on(&plan_key(&view)));
    assert_eq!(sim.app.cx.undo_label(), Some("Watermark"), "one undo step");
    // Saved with the plan: the view still has it on after a round trip.
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    assert!(back.print_setup.watermark.is_on(&plan_key(&view)));
    assert_eq!(back.print_setup.watermark.spec.text, "DRAFT");

    // A layout: every page shows it when the PDF and the preview include it.
    let l = arch_d_layout(&mut sim, 2);
    let project = &sim.app.cx.project;
    let on = PrintOptions {
        watermark: true,
        ..full_size_on(SheetSize::ArchD)
    };
    let off = full_size_on(SheetSize::ArchD);
    let with = lw::print_with(&l, project, &on);
    let without = lw::print_with(&l, project, &off);
    assert_eq!(
        count(&with, "(DRAFT) Tj"),
        4 * 2,
        "4 marks on each of 2 sheets"
    );
    assert_eq!(count(&without, "(DRAFT) Tj"), 0);
    assert!(has(&with, "/GSA30 gs"), "at 70% transparency");
    // The preview is drawn from the same pieces.
    let rcx = plan_layout::LayoutRenderContext::new(&sim.app.cx.project);
    let shown = plan_layout::layout_print_preview(&l, &rcx, &on);
    assert_eq!(marks(&shown), 8);
    assert_eq!(marks(&plan_layout::layout_print_preview(&l, &rcx, &off)), 0);

    // The Print dialog ticks Include Watermark when the view has it on.
    ds::set_context(ds::PrintViewContext {
        synced: true,
        watermark_on: true,
        watermark_ready: true,
        ..ds::PrintViewContext::default()
    });
    let d = PrintDialog::for_layout(2, (36.0, 24.0), "L");
    assert!(d.includes_watermark() && d.options().watermark);
    ds::set_context(ds::PrintViewContext::default());
}

#[test]
fn the_watermark_defaults_dialog_updates_the_view_behind_it_and_ok_stores_the_mark() {
    let mut sim = house();
    sim.action(Action::Custom(wm::DEFAULTS));
    assert!(wm::is_open());
    for _ in 0..2 {
        let ctx = sim.ctx.clone();
        let _ = ctx.run(egui::RawInput::default(), |c| {
            ds::show_all(c, &mut sim.app.cx)
        });
    }
    // The view shows the mark being edited though View > Watermark is off.
    assert!(wm::active_spec(&sim.app.cx).is_some());
    assert!(!wm::is_on(&sim.app.cx));
    let ctx = sim.ctx.clone();
    let mut input = egui::RawInput::default();
    input.events.push(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(input, |c| ds::show_all(c, &mut sim.app.cx));
    assert!(!wm::is_open(), "Cancel closes it");
    assert!(wm::active_spec(&sim.app.cx).is_none());
    assert_eq!(sim.app.cx.project.print_setup.watermark.spec.text, "DRAFT");
}

#[test]
fn the_watermark_and_the_printable_border_draw_on_the_plan() {
    let mut sim = house();
    sim.app.cx.view_flags.insert(ViewFlag::DrawingSheet);
    sim.app
        .cx
        .project
        .print_setup
        .set_sheet(ViewType::Plan, ViewSheet::default());
    let shapes_with = |sim: &mut Sim| {
        let cam = sim.app.camera;
        let ctx = sim.ctx.clone();
        let cx = &mut sim.app.cx;
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 900.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        let (_, painter) =
                            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
                        let mut cam = cam;
                        cam.rect = painter.clip_rect();
                        cam.px_per_in = 0.2;
                        ds::paint_overlays(cx, &painter, &cam);
                    });
            },
        );
        let mut flat = Vec::new();
        for s in out.shapes {
            flatten(s.shape, &mut flat);
        }
        flat
    };
    let texts = |shapes: &[egui::Shape]| {
        shapes
            .iter()
            .filter(|s| matches!(s, egui::Shape::Text(_)))
            .count()
    };
    let before = shapes_with(&mut sim);
    assert_eq!(texts(&before), 0, "no watermark while it is off");
    // The blue border of the printable area, inside the margins: a closed line.
    assert!(before
        .iter()
        .any(|s| matches!(s, egui::Shape::Path(p) if p.closed && p.points.len() == 4)));
    sim.action(Action::Custom(wm::TOGGLE));
    let after = shapes_with(&mut sim);
    assert_eq!(texts(&after), 9, "3 x 3 marks of the default watermark");
}

// ---------------------------------------------------- drawing sheet setup --

#[test]
fn drawing_sheet_setup_is_per_view_and_its_scale_is_the_default_for_print_and_send_to_layout() {
    let mut sim = house();
    // The plan: ARCH C at 1/8" = 1'.
    ds::open(&sim.app.cx, ViewType::Plan);
    assert!(ds::setup_is_open());
    let plan = ViewSheet {
        sheet: SheetDims::new("ARCH C (18 x 24)", 24.0, 18.0),
        scale: DrawingScale::per_foot(0.125),
        margins_in: [0.5; 4],
        ..ViewSheet::default()
    };
    sim.app
        .cx
        .project
        .print_setup
        .set_sheet(ViewType::Plan, plan.clone());
    ds::sync(&mut sim.app.cx, &sim.app.camera);
    assert_eq!(sim.app.cx.sheet.size, SheetSize::ArchC);
    assert_eq!(sim.app.cx.sheet.scale, Scale::EighthInch);
    // Its Drawing Scale is what Send to Layout and Print start from.
    assert_eq!(
        ds::default_scale(&sim.app.cx.project, ViewType::Plan),
        Scale::EighthInch
    );
    let mut v = LayoutView::default();
    assert!(v.create(&mut sim.app.cx.project, None));
    let spec = crate::dialogs::layout::SendSpec {
        source: crate::dialogs::layout::SendSource::Plan {
            floor: 0,
            layer_set: "Default Set".into(),
        },
        page: crate::dialogs::layout::PageChoice::Existing(1),
        scale: Some(ds::default_scale(&sim.app.cx.project, ViewType::Plan)),
        placement: crate::dialogs::layout::Placement::FirstFree,
    };
    v.send(&mut sim.app.cx.project, &spec, None).unwrap();
    let boxes = &v.layout().unwrap().pages[1].boxes;
    assert_eq!(boxes[0].scale, Scale::EighthInch);
    // A view type that has no setup of its own inherits the plan's; a layout is 1 in = 1 in.
    let ps = &sim.app.cx.project.print_setup;
    assert_eq!(ps.sheet_for(ViewType::Elevation), plan);
    assert!((ps.sheet_for(ViewType::Layout).scale.ratio() - 1.0).abs() < 1e-12);
    assert_eq!(
        ds::default_scale(&sim.app.cx.project, ViewType::Elevation),
        Scale::EighthInch
    );
    // The CAD Detail gets its own: metric 1:20 on A3, stored apart.
    let detail = ViewSheet {
        sheet: SheetDims::new("ISO A3", 420.0 / 25.4, 297.0 / 25.4),
        scale: DrawingScale::ratio_mm(20.0),
        ..ViewSheet::default()
    };
    sim.app
        .cx
        .project
        .print_setup
        .set_sheet(ViewType::CadDetail, detail.clone());
    assert_eq!(
        ds::default_scale(&sim.app.cx.project, ViewType::CadDetail),
        Scale::Ratio(20)
    );
    assert_eq!(
        ds::default_scale(&sim.app.cx.project, ViewType::Plan),
        Scale::EighthInch
    );
    // Everything is saved with the plan and loads back.
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.print_setup.sheet_for(ViewType::CadDetail), detail);
    // An old plan without the table loads with the defaults.
    let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
    old.as_object_mut().unwrap().remove("print_setup");
    let loaded: plan_core::Project = serde_json::from_value(old).unwrap();
    assert!(loaded.print_setup.is_default());
}

#[test]
fn the_print_dialog_starts_from_the_views_drawing_sheet_and_prints_the_window_it_names() {
    let mut sim = house();
    sim.app.cx.view_flags.insert(ViewFlag::DrawingSheet);
    sim.app.cx.project.print_setup.set_sheet(
        ViewType::Plan,
        ViewSheet {
            scale: DrawingScale::per_foot(0.25),
            margins_in: [0.25, 0.25, 0.5, 0.5],
            ..ViewSheet::default()
        },
    );
    ds::sync(&mut sim.app.cx, &sim.app.camera);
    let d = PrintDialog::for_plan(0, "Default Set", "FIRST FLOOR PLAN");
    // Print Source follows the Drawing Sheet toggle; the paper matches the sheet.
    assert_eq!(d.source(), PrintSource::DrawingSheet);
    let o = d.options();
    assert_eq!(o.paper, PaperSize::Standard(SheetSize::ArchD));
    assert_eq!(o.scale, PrintScale::Drawing(Scale::QuarterInch));
    let window = o.plan_window.expect("the Drawing Sheet's footprint");
    // 36 x 24 at 1/4" = 1' is 1728 x 1152 plan inches, centered on the house.
    assert!((window[2] - window[0] - 1728.0).abs() < 1e-6);
    assert!((window[3] - window[1] - 1152.0).abs() < 1e-6);
    assert!(
        (window[0] + window[2]) * 0.5 - W * 0.5 < 8.0,
        "centred on the walls"
    );
    assert_eq!(o.edge_margins_in, Some([0.25, 0.25, 0.5, 0.5]));
    // It prints as one 36 x 24 page.
    let (pdf, scale) = lw::print_plan(&sim.app.cx.project, 0, "FIRST FLOOR PLAN", &o);
    assert_eq!(scale, Scale::QuarterInch);
    assert_eq!(pages(&pdf), 1);
    assert_eq!(media_boxes(&pdf), ["0 0 2592 1728"]);
    // Current View prints the part on screen instead.
    ds::set_context(ds::PrintViewContext {
        sheet_shown: false,
        view_window: Some([0.0, 0.0, 480.0, 240.0]),
        synced: true,
        ..ds::context()
    });
    let d = PrintDialog::for_plan(0, "Default Set", "FIRST FLOOR PLAN");
    assert_eq!(d.source(), PrintSource::CurrentView);
    assert_eq!(d.options().plan_window, Some([0.0, 0.0, 480.0, 240.0]));
    ds::set_context(ds::PrintViewContext::default());
}

#[test]
fn scale_to_fit_and_center_sheet_work_from_the_print_menu_and_move_no_object() {
    let mut sim = house();
    let walls: Vec<_> = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .map(|w| (w.start, w.end))
        .collect();
    let before = sim.app.cx.undo_label().map(str::to_string);
    sim.action(Action::Custom(ds::CENTER_SHEET));
    let c = sim
        .app
        .cx
        .floor()
        .sheet_center
        .expect("stored for the floor");
    assert!(
        (c.x - W * 0.5).abs() < 8.0 && (c.y - H * 0.5).abs() < 8.0,
        "{c:?}"
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Center Sheet"));
    sim.undo();
    assert!(sim.app.cx.floor().sheet_center.is_none());
    assert_eq!(sim.app.cx.undo_label().map(str::to_string), before);
    sim.redo();
    // Scale to Fit: a 40' x 30' house on an ARCH D picks the biggest scale that fits.
    sim.action(Action::Custom(ds::SCALE_TO_FIT));
    assert_eq!(sim.app.cx.undo_label(), Some("Scale to Fit"));
    let vs = sim.app.cx.project.print_setup.sheet_for(ViewType::Plan);
    // 40' x 30' (plus the wall faces) on 35.5 x 23.5 printable inches: 1" = 1' is
    // 40 x 30, too big; 3/4" is 30 x 22.9, which fits.
    assert_eq!(ds::scale_of(&vs), Scale::ThreeQuarterInch);
    let after: Vec<_> = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .map(|w| (w.start, w.end))
        .collect();
    assert_eq!(walls, after, "no coordinate moved");
    // Each floor has its own sheet place.
    sim.app
        .cx
        .project
        .floors
        .push(plan_core::Floor::new("2nd Floor", 108.0));
    assert!(sim.app.cx.project.floors[1].sheet_center.is_none());
}

// --------------------------------------------------------------- printing --

#[test]
fn check_plot_half_scale_pages_and_sizes_at_arch_d_and_letter() {
    let mut sim = house();
    let l = arch_d_layout(&mut sim, 2);
    let project = &sim.app.cx.project;
    // ARCH D sheets on ARCH D paper: two pages, 36 x 24.
    let full = lw::print_with(&l, project, &full_size_on(SheetSize::ArchD));
    assert_eq!(pages(&full), 2);
    assert_eq!(media_boxes(&full), ["0 0 2592 1728"; 2]);
    // Fit to Paper on Letter: two pages of 11 x 8.5 (landscape).
    let letter = lw::print_with(
        &l,
        project,
        &PrintOptions {
            paper: PaperSize::Standard(SheetSize::Letter),
            scale: PrintScale::FitPercent(95.0),
            ..PrintOptions::default()
        },
    );
    assert_eq!(pages(&letter), 2);
    assert_eq!(media_boxes(&letter), ["0 0 792 612"; 2]);
    // Check Plot at 1/2: the paper adjusts to the 18 x 12 reduced sheet.
    let papers: Vec<(String, (f64, f64))> = SheetSize::ALL
        .into_iter()
        .map(|z| (z.label().to_string(), z.inches()))
        .collect();
    let i = plan_layout::paper_for_check_plot((36.0, 24.0), 0.5, [0.0; 4], &papers).unwrap();
    assert_eq!(papers[i].0, SheetSize::ArchB.label());
    let check = |paper: SheetSize, tiling: bool| {
        lw::print_with(
            &l,
            project,
            &PrintOptions {
                paper: PaperSize::Standard(paper),
                landscape: true,
                margin_in: 0.0,
                scale: PrintScale::CheckPlot {
                    scale: Scale::QuarterInch,
                    fraction: 0.5,
                },
                tiling,
                ..PrintOptions::default()
            },
        )
    };
    let b = check(SheetSize::ArchB, false);
    assert_eq!(pages(&b), 2, "one ARCH B page for each sheet");
    assert_eq!(media_boxes(&b), ["0 0 1296 864"; 2]);
    assert!(has(&b, "0.5 0 0 0.5"), "drawing and lines at half size");
    // On Letter the half-size sheet needs four pages with crop marks.
    let on_letter = check(SheetSize::Letter, true);
    assert_eq!(pages(&on_letter), 4 * 2);
    assert_eq!(media_boxes(&on_letter)[0], "0 0 792 612");
    assert!(has(&on_letter, "TILE 4 OF 4"));
    // The dialog reports the sheet and the paper to the user.
    let d = PrintDialog::for_layout(2, (36.0, 24.0), "Layout");
    let info = d.info();
    assert!(info[0].contains("36 x 24 in"), "{info:?}");
    // The preview shows the same reduced pages and page count.
    let rcx = plan_layout::LayoutRenderContext::new(project);
    let pv = plan_layout::layout_print_preview(
        &l,
        &rcx,
        &PrintOptions {
            paper: PaperSize::Standard(SheetSize::ArchB),
            landscape: true,
            margin_in: 0.0,
            scale: PrintScale::CheckPlot {
                scale: Scale::QuarterInch,
                fraction: 0.5,
            },
            color: PrintColor::Grayscale,
            ..PrintOptions::default()
        },
    );
    assert_eq!(pv.len(), 2);
    assert_eq!(pv[0].scale, 0.5);
    assert_eq!(pv[0].paper_in, (18.0, 12.0));
}

// ------------------------------------------------------------ sheet sizes --

#[test]
fn sheet_sizes_are_program_wide_and_old_layout_sizes_are_taken_over() {
    let mut sim = house();
    let dir = std::env::temp_dir().join(format!("ps-s62-sizes-{}", std::process::id()));
    let path = dir.join("sheetsizes.json");
    plan_layout::use_sheet_sizes(SheetSizeFile::default(), Some(path.clone()));
    // Customize Sheet Sizes from the Print menu and from the Layout menu is one dialog.
    sim.action(Action::Layout(lw::LayoutCommand::CustomizeSheetSizes));
    assert!(ds::customize_is_open());
    for _ in 0..2 {
        let ctx = sim.ctx.clone();
        let _ = ctx.run(egui::RawInput::default(), |c| {
            ds::show_all(c, &mut sim.app.cx)
        });
    }
    // OK writes ~/.plan-studio/sheetsizes.json (here, a temp file) for every plan.
    let mut file = plan_layout::global_sheet_sizes();
    file.custom.push(CustomSheetSize::new("Poster", 30.0, 40.0));
    file.hidden = vec![SheetSize::IsoA0];
    plan_layout::set_global_sheet_sizes(file).unwrap();
    let on_disk = SheetSizeFile::load_from(&path);
    assert_eq!(on_disk.custom.len(), 1);
    assert_eq!(on_disk.hidden, [SheetSize::IsoA0]);
    // The Print dialog and the Drawing Sheet Setup list it.
    let papers = plan_layout::global_custom_papers();
    assert_eq!(papers, [("Poster (30 x 40)".to_string(), (40.0, 30.0))]);
    let choices = plan_layout::global_sheet_sizes().choices(None);
    assert!(choices
        .iter()
        .any(|c| matches!(c, plan_layout::SheetChoice::Custom(c) if c.name == "Poster")));
    assert!(!choices.contains(&plan_layout::SheetChoice::Standard(SheetSize::IsoA0)));
    // A layout from before kept sizes of its own: they still load and are adopted.
    let mut old = Layout::new("Old", SheetSize::ArchC);
    old.custom_sizes = vec![CustomSheetSize::new("Banner", 12.0, 60.0)];
    let loaded: Layout = serde_json::from_str(&serde_json::to_string(&old).unwrap()).unwrap();
    assert_eq!(loaded.custom_sizes.len(), 1);
    assert!(plan_layout::adopt_layout_sheet_sizes(&loaded));
    assert_eq!(SheetSizeFile::load_from(&path).custom.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
    plan_layout::use_sheet_sizes(SheetSizeFile::default(), None);
}
