//! Round 14: page sheets, custom sizes, the print preview picture, schedule
//! columns from the plan, the scene builder and hatch scaling.

use super::*;
use crate::canvas::{Canvas, Prim};
use crate::extent::schedule_for;
use crate::tests::two_room_house;
use plan_core::schedules::{FloorScope, Schedule as SpecSchedule, ScheduleLayer};
use plan_core::{Point, Project};
use plan_docs::{Scale, SheetSize};
use plan_elevation::{Drawing, EdgeKind, Line2, LineWeight, Region, RegionKind};
use std::cell::Cell;

fn text_of(pdf: &[u8]) -> String {
    pdf.iter().map(|&b| b as char).collect()
}

fn two_page_layout() -> Layout {
    let mut l = Layout::new("t", SheetSize::ArchC);
    l.add_page(1, "Plan");
    l.add_page(2, "Detail");
    l
}

// ------------------------------------------------------- sheets and pages --

#[test]
fn a_page_can_have_a_sheet_of_its_own_and_the_pdf_pages_follow() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = two_page_layout();
    l.set_page_sheet(1, Some(&SheetChoice::Standard(SheetSize::ArchD)), false);
    assert_eq!(l.page_sheet_inches(&l.pages[0]), (24.0, 18.0));
    assert_eq!(l.page_sheet_inches(&l.pages[1]), (36.0, 24.0));
    let t = text_of(&render_pdf(&l, &cx));
    assert!(
        t.contains("/MediaBox [0 0 1728 1296]"),
        "first page 24 x 18"
    );
    assert!(
        t.contains("/MediaBox [0 0 2592 1728]"),
        "second page 36 x 24"
    );
    // Portrait turns the page upright; None goes back to the layout's sheet.
    l.set_page_sheet(1, Some(&SheetChoice::Standard(SheetSize::ArchD)), true);
    assert_eq!(l.pages[1].size_override_in, Some((24.0, 36.0)));
    l.set_page_sheet(1, None, false);
    assert_eq!(l.pages[1].size_override_in, None);
    // The print path uses the page's sheet too.
    l.set_page_sheet(0, Some(&SheetChoice::Standard(SheetSize::ArchE)), false);
    let opts = PrintOptions {
        paper: PaperSize::Standard(SheetSize::ArchE),
        scale: PrintScale::Actual,
        margin_in: 0.0,
        ..PrintOptions::default()
    };
    let preview = layout_print_preview(&l, &cx, &opts);
    assert_eq!(preview.len(), 2);
    assert!((preview[0].scale - 1.0).abs() < 1e-9);
}

#[test]
fn a_page_packs_boxes_into_its_own_sheet_and_can_drop_its_title_block() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = two_page_layout();
    l.set_page_sheet(0, Some(&SheetChoice::Standard(SheetSize::Letter)), false);
    let id = send_to_layout(
        &mut l,
        &cx,
        1,
        BoxSource::text("NOTES", 12.0),
        Scale::QuarterInch,
        None,
    );
    let b = l
        .page(1)
        .unwrap()
        .boxes
        .iter()
        .find(|b| b.id == id)
        .unwrap();
    let area = l.page_drawing_area(l.page(1).unwrap());
    let r = b.bounds_in();
    // Top-left of the Letter-size area, not of the 24 x 18 sheet.
    assert_eq!((r[0], r[3]), (area.0.x, area.1.y));
    assert!(area.1.x < 11.0);
    // The title block and its "SHEET n OF m" go with Page Specification.
    let scenes = crate::extent::SceneSource::new(None);
    let shows_sheet_count = |l: &Layout, i: usize| {
        let mut cv = Canvas::new();
        let pages = l.content_pages();
        crate::render::draw_page(&mut cv, l, &pages, i, &cx, &scenes);
        cv.prims.iter().any(
            |p| matches!(p, Prim::Text { text, .. } if text.starts_with("SHEET ") && text.contains(" OF ")),
        )
    };
    assert!(shows_sheet_count(&l, 0));
    l.pages[0].no_title_block = true;
    assert!(!shows_sheet_count(&l, 0));
    assert!(shows_sheet_count(&l, 1));
}

#[test]
fn duplicate_labels_are_legal() {
    // Round 16 (DECISIONS 62 changed): the label is text, not a unique
    // number; a pattern with # numbers the pages that share it.
    let mut l = two_page_layout();
    l.pages[0].label = "A-#".into();
    l.pages[1].label = "A-#".into();
    assert_eq!(l.sheet_number_of(&l.pages[1]), "A-2");
    l.pages[1].label = "A-1".into();
    assert_eq!(l.sheet_number_of(&l.pages[0]), "A-1");
    assert_eq!(l.sheet_number_of(&l.pages[1]), "A-1");
}

#[test]
fn custom_sheet_sizes_join_the_lists_and_set_the_layout_sheet() {
    let mut l = Layout::new("t", SheetSize::ArchC);
    let all = l.size_choices();
    assert_eq!(all.len(), SheetSize::ALL.len());
    // Hiding a standard size takes it off the list, except the layout's own.
    l.hidden_sizes = vec![SheetSize::ArchC, SheetSize::IsoA0];
    let shown = l.size_choices();
    assert!(shown.contains(&SheetChoice::Standard(SheetSize::ArchC)));
    assert!(!shown.contains(&SheetChoice::Standard(SheetSize::IsoA0)));
    l.sheet = SheetSize::ArchD;
    assert!(!l
        .size_choices()
        .contains(&SheetChoice::Standard(SheetSize::ArchC)));
    // A custom size is listed after the standard ones and can be the sheet.
    let poster = CustomSheetSize::new("Poster", 40.0, 30.0);
    assert_eq!(poster.label(), "Poster (30 x 40)");
    assert_eq!(poster.problem(), None);
    l.custom_sizes.push(poster.clone());
    assert_eq!(
        l.size_choices().last(),
        Some(&SheetChoice::Custom(poster.clone()))
    );
    l.set_sheet_choice(&SheetChoice::Custom(poster.clone()));
    assert_eq!(l.sheet_inches(), (40.0, 30.0));
    // The standard size stays the smallest one that holds it (ARCH E1, 42 x 30).
    assert_eq!(l.sheet, SheetSize::ArchE1);
    assert_eq!(l.sheet_choice(), SheetChoice::Custom(poster));
    l.portrait = true;
    assert_eq!(l.sheet_inches(), (30.0, 40.0));
    l.set_sheet_choice(&SheetChoice::Standard(SheetSize::ArchC));
    assert!(l.custom_sheet.is_none());
    assert_eq!(l.sheet_inches(), (18.0, 24.0));
    // Bad sizes say why.
    assert!(CustomSheetSize::new(" ", 10.0, 10.0).problem().is_some());
    assert!(CustomSheetSize::new("Tiny", 0.5, 10.0).problem().is_some());
    assert!(CustomSheetSize::new("Huge", 20.0, 500.0)
        .problem()
        .is_some());
    assert!(CustomSheetSize::new("NaN", f64::NAN, 10.0)
        .problem()
        .is_some());
    // Daniel's preset leads with 18 x 24.
    assert_eq!(DANIEL_SIZES[0].inches(), (24.0, 18.0));
    assert_eq!(DANIEL_SIZES[1].inches(), (36.0, 24.0));
}

#[test]
fn the_new_fields_are_optional_in_old_files() {
    let l = two_page_layout();
    let mut v = serde_json::to_value(&l).unwrap();
    let obj = v.as_object_mut().unwrap();
    for k in ["custom_sizes", "hidden_sizes", "custom_sheet"] {
        obj.remove(k);
    }
    for p in obj["pages"].as_array_mut().unwrap() {
        let po = p.as_object_mut().unwrap();
        po.remove("size_override_in");
        po.remove("no_title_block");
    }
    let back: Layout = serde_json::from_value(v).unwrap();
    assert_eq!(back, l);
}

// ---------------------------------------------------------- print preview --

fn layout_with_plan(p: &Project) -> Layout {
    let cx = LayoutRenderContext::new(p);
    let mut l = Layout::new("t", SheetSize::ArchC);
    l.add_page(1, "Plan");
    send_to_layout(
        &mut l,
        &cx,
        1,
        BoxSource::PlanView {
            floor: 0,
            layer_set: "Floor Plan".into(),
        },
        Scale::QuarterInch,
        None,
    );
    l
}

fn colors(page: &PreviewPage) -> Vec<[u8; 3]> {
    let mut out = Vec::new();
    for i in &page.items {
        match i {
            PreviewItem::Fill { color, .. }
            | PreviewItem::Stroke { color, .. }
            | PreviewItem::Text { color, .. } => out.push(*color),
            _ => {}
        }
    }
    out
}

#[test]
fn the_preview_is_a_picture_of_the_chosen_colour_mode() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let l = layout_with_plan(&p);
    let base = PrintOptions {
        paper: PaperSize::Standard(SheetSize::ArchC),
        scale: PrintScale::Actual,
        margin_in: 0.0,
        ..PrintOptions::default()
    };
    let color = layout_print_preview(&l, &cx, &base);
    assert_eq!(color.len(), 1);
    // In colour the sheet background and the layer colours show.
    assert!(colors(&color[0])
        .iter()
        .any(|c| c[0] != c[1] || c[1] != c[2]));
    let gray = layout_print_preview(
        &l,
        &cx,
        &PrintOptions {
            color: PrintColor::Grayscale,
            ..base.clone()
        },
    );
    assert!(colors(&gray[0])
        .iter()
        .all(|c| c[0] == c[1] && c[1] == c[2]));
    assert!(
        colors(&gray[0]).iter().any(|c| c[0] > 0 && c[0] < 255),
        "grayscale keeps mid grays"
    );
    let bw = layout_print_preview(
        &l,
        &cx,
        &PrintOptions {
            color: PrintColor::BlackWhite,
            ..base.clone()
        },
    );
    assert!(colors(&bw[0])
        .iter()
        .all(|c| *c == [0, 0, 0] || *c == [255, 255, 255]));
    assert_eq!(bw[0].color, PrintColor::BlackWhite);
    // The sheet sits on the paper, captioned like its PDF bookmark.
    assert_eq!(color[0].caption, "A-1 Plan");
    assert_eq!(color[0].paper_in, (24.0, 18.0));
    assert!((color[0].scale - 1.0).abs() < 1e-9);
}

#[test]
fn the_preview_has_the_print_dialogs_line_weights_and_scale() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let l = layout_with_plan(&p);
    let widths = |page: &PreviewPage| -> Vec<f64> {
        page.items
            .iter()
            .filter_map(|i| match i {
                PreviewItem::Stroke { width_pt, .. } => Some(*width_pt),
                _ => None,
            })
            .collect()
    };
    let full = PrintOptions {
        paper: PaperSize::Standard(SheetSize::ArchC),
        scale: PrintScale::Actual,
        ..PrintOptions::default()
    };
    let weighted = layout_print_preview(&l, &cx, &full);
    let w = widths(&weighted[0]);
    assert!(
        w.iter().any(|v| (*v - 0.5).abs() > 0.05),
        "pen weights differ"
    );
    let hair = layout_print_preview(
        &l,
        &cx,
        &PrintOptions {
            line_weights: false,
            ..full.clone()
        },
    );
    assert!(widths(&hair[0]).iter().all(|v| (*v - 0.5).abs() < 1e-9));
    // Fit onto Letter scales the pen widths and the picture with the page.
    let letter = layout_print_preview(
        &l,
        &cx,
        &PrintOptions {
            paper: PaperSize::Standard(SheetSize::Letter),
            ..PrintOptions::default()
        },
    );
    let s = letter[0].scale;
    assert!(s < 0.5 && s > 0.2, "{s}");
    let max_full = w.iter().cloned().fold(0.0, f64::max);
    let max_letter = widths(&letter[0]).iter().cloned().fold(0.0, f64::max);
    assert!((max_letter - max_full * s).abs() < 1e-6);
    // Tiling cuts the sheet into the same pages the PDF has.
    let tiled = layout_print_preview(
        &l,
        &cx,
        &PrintOptions {
            paper: PaperSize::Standard(SheetSize::Letter),
            scale: PrintScale::Actual,
            tiling: true,
            ..PrintOptions::default()
        },
    );
    assert!(tiled.len() > 1);
    assert!(
        tiled[1].caption.contains("TILE 2 OF"),
        "{}",
        tiled[1].caption
    );
    // Every item is clipped to the printable area.
    assert!(matches!(
        tiled[0].items.first(),
        Some(PreviewItem::ClipBegin(_))
    ));
    assert!(matches!(tiled[0].items.last(), Some(PreviewItem::ClipEnd)));
}

#[test]
fn a_plan_view_previews_at_its_print_scale() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let opts = PrintOptions {
        paper: PaperSize::Standard(SheetSize::ArchC),
        color: PrintColor::BlackWhite,
        scale: PrintScale::Drawing(Scale::QuarterInch),
        ..PrintOptions::default()
    };
    let (pages, scale) = plan_view_print_preview(&cx, 0, "Floor Plan", "FIRST FLOOR", &opts);
    assert_eq!(scale, Scale::QuarterInch);
    assert_eq!(pages.len(), 1);
    assert!(colors(&pages[0])
        .iter()
        .all(|c| *c == [0, 0, 0] || *c == [255, 255, 255]));
    // The PDF and the preview agree on the scale.
    let (pdf, pdf_scale) = print_plan_view_pdf(&cx, 0, "Floor Plan", "FIRST FLOOR", &opts);
    assert_eq!(pdf_scale, scale);
    assert!(pdf.starts_with(b"%PDF"));
}

// -------------------------------------------------------------- hatching --

fn brick_drawing() -> Drawing {
    let poly = vec![
        Point::new(0.0, 0.0),
        Point::new(480.0, 0.0),
        Point::new(480.0, 240.0),
        Point::new(0.0, 240.0),
    ];
    let mut d = Drawing::new(vec![
        Line2 {
            a: Point::new(0.0, 0.0),
            b: Point::new(480.0, 0.0),
            weight: LineWeight::Heavy,
            kind: EdgeKind::Silhouette,
        },
        // A hatch line made for 1/4"; it is replaced for the box's scale.
        Line2 {
            a: Point::new(0.0, 10.0),
            b: Point::new(480.0, 10.0),
            weight: LineWeight::Light,
            kind: EdgeKind::Hatch,
        },
    ]);
    d.regions = vec![Region {
        polygon: poly,
        material: plan_3d::Material::Brick,
        object_id: None,
        kind: RegionKind::Face,
    }];
    d
}

fn hatch_strokes(l: &Layout, p: &Project) -> usize {
    let drawing = brick_drawing();
    let cx = LayoutRenderContext::new(p).with_camera_drawing(move |_| Some(drawing.clone()));
    let scenes = crate::extent::SceneSource::new(None);
    let b = &l.pages[0].boxes[0];
    crate::render::box_prims(b, &cx, &scenes, &l.layers)
        .iter()
        .filter(|p| matches!(p, Prim::Stroke { pen, .. } if (pen.width - 0.18).abs() < 1e-9))
        .count()
}

#[test]
fn a_camera_boxs_hatch_is_made_again_for_the_box_scale() {
    let mut p = Project::new("h");
    let cam = p.alloc_id();
    let mut l = Layout::new("t", SheetSize::ArchC);
    l.add_page(1, "Elevations");
    let area = l.drawing_area();
    let rect = (area.0, Point::new(area.0.x + 14.0, area.0.y + 8.0));
    let mut b = LayoutBox::new(
        1,
        rect,
        BoxSource::Camera { camera_id: cam },
        Scale::QuarterInch,
    );
    b.clip = false;
    l.pages[0].boxes.push(b);
    let at_quarter = hatch_strokes(&l, &p);
    l.pages[0].boxes[0].scale = Scale::OneInchEq20Ft;
    let at_twenty = hatch_strokes(&l, &p);
    l.pages[0].boxes[0].scale = Scale::HalfInch;
    let at_half = hatch_strokes(&l, &p);
    assert!(at_quarter > 10, "{at_quarter}");
    // A small drawing scale coarsens the courses; a large one keeps them fine.
    assert!(at_twenty < at_quarter / 2, "{at_twenty} vs {at_quarter}");
    assert!(at_half >= at_quarter, "{at_half} vs {at_quarter}");
}

// -------------------------------------------------- schedule specification --

fn project_with_door_spec(hide: &str) -> Project {
    let mut p = two_room_house();
    let mut layer = ScheduleLayer::default();
    let mut def = SpecSchedule::new(plan_core::schedules::ScheduleKind::Door, Point::ZERO);
    def.floor_scope = FloorScope::ThisFloor;
    for c in &mut def.columns {
        if c.field == hide {
            c.visible = false;
        }
    }
    layer.add(def);
    layer.store(&mut p.floors[0]);
    p
}

#[test]
fn a_standard_schedule_box_follows_the_plans_schedule_specification() {
    let plain = two_room_house();
    let cx = LayoutRenderContext::new(&plain);
    let standard = schedule_for(ScheduleKind::Door, &cx);
    assert!(standard.columns.len() > 2);
    // Without a placed door schedule the standard columns stay.
    assert_eq!(standard.rows.len(), 2);

    let hide = plan_core::schedules::ScheduleKind::Door
        .default_columns()
        .into_iter()
        .filter(|c| c.visible)
        .nth(1)
        .expect("a second visible column")
        .field;
    let p = project_with_door_spec(&hide);
    let cx = LayoutRenderContext::new(&p);
    let t = schedule_for(ScheduleKind::Door, &cx);
    let want: Vec<String> = plan_core::schedules::ScheduleKind::Door
        .default_columns()
        .into_iter()
        .filter(|c| c.visible && c.field != hide)
        .map(|c| c.title)
        .collect();
    assert_eq!(t.columns, want, "the hidden column is gone from the box");
    assert_eq!(t.rows.len(), 2, "both doors, over every floor");
    // The title stays the standard one unless the plan's schedule has its own.
    assert_eq!(t.title, standard.title);
}

#[test]
fn the_construction_set_adds_the_plans_other_placed_schedules() {
    let mut p = two_room_house();
    let mut layer = ScheduleLayer::default();
    let id = layer.add(SpecSchedule::new(
        plan_core::schedules::ScheduleKind::Cabinet,
        Point::ZERO,
    ));
    // A door schedule is covered by the standard Door box.
    layer.add(SpecSchedule::new(
        plan_core::schedules::ScheduleKind::Door,
        Point::new(10.0, 10.0),
    ));
    layer.store(&mut p.floors[0]);
    let l = default_construction_set(&p, 1);
    let sched = l.pages.iter().find(|pg| pg.title == "Schedules").unwrap();
    let placed: Vec<_> = sched
        .boxes
        .iter()
        .filter_map(|b| match b.source {
            BoxSource::PlacedSchedule { floor, id } => Some((floor, id)),
            _ => None,
        })
        .collect();
    assert_eq!(placed, vec![(0, id)]);
    assert!(sched.boxes.iter().any(|b| matches!(
        b.source,
        BoxSource::Schedule {
            kind: ScheduleKind::Door
        }
    )));
}

// ------------------------------------------------------------ scene hook --

#[test]
fn elevations_use_the_contexts_scene_builder_once() {
    let p = two_room_house();
    let calls = Cell::new(0);
    let cx = LayoutRenderContext::new(&p).with_scene_builder(|proj| {
        calls.set(calls.get() + 1);
        plan_3d::build_scene(proj)
    });
    let mut l = Layout::new("t", SheetSize::ArchC);
    l.add_page(1, "Elevations");
    for dir in [
        plan_elevation::ViewDir::Front,
        plan_elevation::ViewDir::Left,
    ] {
        send_to_layout(
            &mut l,
            &cx,
            1,
            BoxSource::Elevation { dir },
            Scale::EighthInch,
            None,
        );
    }
    let pdf = render_pdf(&l, &cx);
    assert!(pdf.starts_with(b"%PDF"));
    // Sending measured with one builder call each; printing made one more.
    assert!(calls.get() >= 1, "the builder was used");
    let after_print = calls.get();
    let _ = render_pdf(&l, &cx);
    assert_eq!(calls.get(), after_print + 1, "one scene per print");
    // A context scene wins over the builder.
    let scene = plan_3d::build_scene(&p);
    let mut cx2 = LayoutRenderContext::new(&p).with_scene_builder(|_| panic!("not asked"));
    cx2.scene = Some(&scene);
    let _ = render_pdf(&l, &cx2);
}

#[test]
fn the_construction_set_measures_on_the_given_scene_builder() {
    let p = two_room_house();
    let calls = Cell::new(0);
    let mut l = Layout::new("t", SheetSize::ArchC);
    let added = append_construction_set_in(
        &mut l,
        &p,
        1,
        &plan_docs::MasterList::default(),
        Some(&|proj: &Project| {
            calls.set(calls.get() + 1);
            plan_3d::build_scene(proj)
        }),
    );
    assert!(added > 5);
    assert_eq!(calls.get(), 1);
}
