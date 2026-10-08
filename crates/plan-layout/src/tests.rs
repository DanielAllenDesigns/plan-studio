//! Crate-level tests: sizing, packing, macros, PDF structure, clipping, JSON.

use super::*;
use crate::extent::schedule_for;
use crate::render::Prim;
use plan_core::{CadItem, CadObject, OpeningKind, Point, Project, WallKind};
use plan_docs::{Scale, SheetSize};
use plan_elevation::ViewDir;

/// A 40' x 30' house split into two rooms, with two doors and two windows.
fn two_room_house() -> Project {
    let mut p = Project::new("Smith Residence");
    let c = [
        Point::new(0.0, 0.0),
        Point::new(480.0, 0.0),
        Point::new(480.0, 360.0),
        Point::new(0.0, 360.0),
    ];
    let mut ids = [0; 4];
    for i in 0..4 {
        ids[i] = p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior);
    }
    let mid = p.add_wall(
        0,
        Point::new(240.0, 0.0),
        Point::new(240.0, 360.0),
        4.5,
        109.125,
        WallKind::Interior,
    );
    p.add_opening(0, ids[0], 120.0, OpeningKind::Door).unwrap();
    p.add_opening(0, mid, 180.0, OpeningKind::Door).unwrap();
    p.add_opening(0, ids[2], 120.0, OpeningKind::Window)
        .unwrap();
    p.add_opening(0, ids[2], 360.0, OpeningKind::Window)
        .unwrap();
    p
}

fn plan_source() -> BoxSource {
    BoxSource::PlanView {
        floor: 0,
        layer_set: "Floor Plan".into(),
    }
}

fn overlap(a: &LayoutBox, b: &LayoutBox) -> bool {
    let (p, q) = (a.bounds_in(), b.bounds_in());
    p[0] < q[2] - 1e-9 && q[0] < p[2] - 1e-9 && p[1] < q[3] - 1e-9 && q[1] < p[3] - 1e-9
}

fn text_of(pdf: &[u8]) -> String {
    pdf.iter().map(|&b| b as char).collect()
}

/// Minimal xref check: every table entry points at "N 0 obj".
fn check_xref(pdf: &[u8]) {
    let text = text_of(pdf);
    let sx = text.rfind("startxref\n").expect("startxref");
    let xref_off: usize = text[sx + 10..]
        .lines()
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(pdf[xref_off..].starts_with(b"xref\n"));
    let tail = String::from_utf8_lossy(&pdf[xref_off..]).into_owned();
    let mut lines = tail.lines().skip(1);
    let total: usize = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    assert!(lines.next().unwrap().starts_with("0000000000 65535 f"));
    for obj in 1..total {
        let off: usize = lines.next().unwrap()[..10].parse().unwrap();
        assert!(
            pdf[off..].starts_with(format!("{obj} 0 obj\n").as_bytes()),
            "object {obj} offset wrong"
        );
    }
    assert!(pdf.ends_with(b"%%EOF"));
}

#[test]
fn quarter_inch_is_one_and_a_half_points_per_plan_inch() {
    assert!((Scale::from_label("1/4\" = 1'").unwrap().points_per_inch() - 1.5).abs() < 1e-12);
}

#[test]
fn send_to_layout_sizes_plan_and_places_boxes_apart() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = Layout::new("t", SheetSize::ArchC);
    l.add_page(1, "Plan");
    let a = send_to_layout(&mut l, &cx, 1, plan_source(), Scale::QuarterInch, None);
    let b = send_to_layout(&mut l, &cx, 1, plan_source(), Scale::QuarterInch, None);
    assert_ne!(a, b);
    let boxes = &l.page(1).unwrap().boxes;
    // 40' x 30' centerline + 6.5" walls + 24" margin each side, at 1/4" = 1'.
    let (w, h) = boxes[0].size_in();
    assert!((w - (480.0 + 6.5 + 48.0) / 48.0).abs() < 1e-9, "w = {w}");
    assert!((h - (360.0 + 6.5 + 48.0) / 48.0).abs() < 1e-9, "h = {h}");
    assert!(w > 10.0 && w < 11.2 && h > 7.5 && h < 8.7);
    assert!(!overlap(&boxes[0], &boxes[1]));
    // The first box sits in the top-left of the drawing area.
    let (lo, hi) = l.drawing_area();
    let r = boxes[0].bounds_in();
    assert_eq!((r[0], r[3]), (lo.x, hi.y));
    assert_eq!(boxes[0].label.as_deref(), Some("1ST FLOOR PLAN"));
}

#[test]
fn smaller_boxes_pack_left_to_right_then_down() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = Layout::new("t", SheetSize::ArchC);
    for _ in 0..3 {
        send_to_layout(&mut l, &cx, 1, plan_source(), Scale::EighthInch, None);
    }
    let boxes = &l.page(1).unwrap().boxes;
    let (r0, r1, r2) = (
        boxes[0].bounds_in(),
        boxes[1].bounds_in(),
        boxes[2].bounds_in(),
    );
    assert!(
        r1[0] > r0[2] && (r1[3] - r0[3]).abs() < 1e-9,
        "second beside first"
    );
    // 5.57" boxes: three do not fit in a 20.5" row with gutters? They do.
    assert!(r2[0] > r1[2] && (r2[3] - r0[3]).abs() < 1e-9);
    let (lo, hi) = l.drawing_area();
    for r in [r0, r1, r2] {
        assert!(r[0] >= lo.x && r[2] <= hi.x && r[1] >= lo.y && r[3] <= hi.y);
    }
    // A fourth wraps to the next shelf at the left edge.
    send_to_layout(&mut l, &cx, 1, plan_source(), Scale::EighthInch, None);
    let r3 = l.page(1).unwrap().boxes[3].bounds_in();
    assert_eq!(r3[0], lo.x);
    assert!(r3[3] < r0[1]);
    // Explicit placement wins.
    let id = send_to_layout(
        &mut l,
        &cx,
        1,
        BoxSource::Text {
            text: "NOTE".into(),
            height_pt: 12.0,
        },
        Scale::QuarterInch,
        Some(Point::new(3.0, 4.0)),
    );
    let b = l
        .page(1)
        .unwrap()
        .boxes
        .iter()
        .find(|b| b.id == id)
        .unwrap();
    assert_eq!(b.rect_in.0, Point::new(3.0, 4.0));
}

#[test]
fn macros_fill_sheet_number_and_project() {
    let p = two_room_house();
    let mut cx = LayoutRenderContext::new(&p);
    cx.macros.client = "Jane Smith".into();
    cx.macros.date = "2026-10-07".into();
    let l = default_construction_set(&p, 1);
    let pdf = render_pdf(&l, &cx);
    let t = text_of(&pdf);
    for want in [
        "(A-3)",
        "(Jane Smith)",
        "(2026-10-07)",
        "(SHEET TITLE)",
        "(DRAWN BY)",
    ] {
        assert!(t.contains(want), "missing {want}");
    }
    assert!(!t.contains("%sheet.number%") && !t.contains("%scale%"));
}

#[test]
fn default_set_renders_a_valid_pdf_with_one_page_per_sheet() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let l = default_construction_set(&p, 1);
    // Cover, plan, 2 elevation sheets, section, schedules, framing.
    assert_eq!(l.pages.len(), 7);
    assert_eq!(l.sheet, SheetSize::ArchC);
    let numbers: Vec<u32> = l.pages.iter().map(|p| p.number).collect();
    assert_eq!(numbers, vec![0, 1, 2, 3, 4, 5, 6]);
    let pdf = render_pdf(&l, &cx);
    assert!(pdf.starts_with(b"%PDF-1.4"));
    check_xref(&pdf);
    let count = |needle: &str| text_of(&pdf).matches(needle).count();
    assert_eq!(count("/Type /Page"), l.pages.len());
    assert!(pdf
        .windows(21)
        .any(|w| w == b"/MediaBox [0 0 1728 1".as_slice()));
    let t = text_of(&pdf);
    for want in [
        "(SHEET 1 OF 7)",
        "(SHEET 7 OF 7)",
        "(SHEET INDEX)",
        "(1ST FLOOR PLAN)",
        "(FRONT ELEVATION)",
        "(Door Schedule)",
        "(FRAMING PLAN)",
        "(SCALE: 1/4\" = 1'-0\")",
    ] {
        assert!(t.contains(want), "missing {want}");
    }
    // Every page of the set has content beyond the title block.
    for page in &l.pages[1..] {
        assert!(!page.boxes.is_empty(), "{} is empty", page.title);
    }
}

#[test]
fn elevation_and_section_boxes_draw_lines() {
    let p = two_room_house();
    let l = default_construction_set(&p, 1);
    let cx = LayoutRenderContext::new(&p);
    for page in &l.pages {
        for b in &page.boxes {
            if matches!(
                b.source,
                BoxSource::Elevation { .. } | BoxSource::Section { .. }
            ) {
                let n = render_box_lines(b, &cx).len();
                assert!(n > 8, "{:?} {n}", b.source);
            }
        }
    }
}

#[test]
fn schedule_table_has_one_row_per_door() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let doors = p.floors[0]
        .openings
        .iter()
        .filter(|o| o.kind == OpeningKind::Door)
        .count();
    assert_eq!(doors, 2);
    let s = schedule_for(ScheduleKind::Door, &cx);
    assert_eq!(s.rows.len(), doors);

    let mut l = Layout::new("t", SheetSize::ArchC);
    let id = send_to_layout(
        &mut l,
        &cx,
        1,
        BoxSource::Schedule {
            kind: ScheduleKind::Door,
        },
        Scale::QuarterInch,
        None,
    );
    let b = l
        .page_mut(1)
        .unwrap()
        .boxes
        .iter_mut()
        .find(|b| b.id == id)
        .unwrap();
    b.border = false;
    let width = b.size_in().0;
    let lines = render_box_lines(b, &cx);
    let rules = lines
        .iter()
        .filter(|l| (l.a.y - l.b.y).abs() < 1e-9 && l.length() > width - 1e-6)
        .count();
    // Top rule, header rule and one rule under each door row.
    assert_eq!(rules, doors + 2);
    // Box height = title + header + rows at 0.25" each.
    assert!((b.size_in().1 - (21.6 / 72.0 + 0.25 * (doors + 1) as f64)).abs() < 1e-9);
}

#[test]
fn clipped_box_never_draws_outside_its_rect() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    // The plan is 22" wide at 1/2" = 1'; the box is 5" x 4".
    let mut b = LayoutBox::new(
        1,
        (Point::new(1.0, 1.0), Point::new(6.0, 5.0)),
        plan_source(),
        Scale::HalfInch,
    );
    let inside = |p: Point, r: [f64; 4]| {
        let e = 1e-6;
        p.x >= r[0] - e && p.x <= r[2] + e && p.y >= r[1] - e && p.y <= r[3] + e
    };
    let r = b.bounds_in();
    let lines = render_box_lines(&b, &cx);
    assert!(lines.len() > 6, "{}", lines.len());
    assert!(lines.iter().all(|l| inside(l.a, r) && inside(l.b, r)));
    // Fills are clipped too.
    let scenes = crate::extent::SceneSource::new(None);
    let rect_pt = r.map(|v| v * 72.0);
    for prim in crate::render::box_prims_for_test(&b, &cx, &scenes) {
        if let Prim::Fill { pts, .. } = prim {
            assert!(pts.iter().all(|&(x, y)| inside(
                Point::new(x / 72.0, y / 72.0),
                [
                    rect_pt[0] / 72.0,
                    rect_pt[1] / 72.0,
                    rect_pt[2] / 72.0,
                    rect_pt[3] / 72.0
                ]
            )));
        }
    }
    // Without clip the same content spills past the frame.
    b.clip = false;
    let spill = render_box_lines(&b, &cx);
    assert!(spill.iter().any(|l| !inside(l.a, r) || !inside(l.b, r)));
}

#[test]
fn hidden_layers_and_line_weight_scale_apply() {
    let mut p = two_room_house();
    let b = LayoutBox::new(
        1,
        (Point::new(1.0, 1.0), Point::new(13.0, 10.0)),
        plan_source(),
        Scale::QuarterInch,
    );
    let n_all = render_box_lines(&b, &LayoutRenderContext::new(&p)).len();
    p.layers.set_display("Doors", false);
    p.layers.set_display("Windows", false);
    let n_hidden = render_box_lines(&b, &LayoutRenderContext::new(&p)).len();
    assert!(n_hidden < n_all);
    // "All" ignores layer visibility.
    let mut all = b.clone();
    all.source = BoxSource::PlanView {
        floor: 0,
        layer_set: "All".into(),
    };
    assert_eq!(
        render_box_lines(&all, &LayoutRenderContext::new(&p)).len(),
        n_all
    );
}

#[test]
fn cad_detail_and_image_sources_render() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let items = vec![CadObject {
        id: 1,
        layer: "CAD, Default".into(),
        item: CadItem::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(36.0, 0.0),
                Point::new(36.0, 12.0),
            ],
            closed: false,
        },
    }];
    let src = BoxSource::CadDetail {
        name: "Sill".into(),
        items,
    };
    let mut l = Layout::new("t", SheetSize::Tabloid);
    send_to_layout(&mut l, &cx, 1, src, Scale::HalfInch, None);
    send_to_layout(
        &mut l,
        &cx,
        1,
        BoxSource::Image {
            path: "photo.jpg".into(),
        },
        Scale::HalfInch,
        None,
    );
    let t = text_of(&render_pdf(&l, &cx));
    assert!(t.contains("(SILL)") && t.contains("(IMAGE: photo.jpg)"));
}

#[test]
fn layout_json_round_trip() {
    let p = two_room_house();
    let mut l = default_construction_set(&p, 1);
    // Exact binary fractions, so the JSON text parser reproduces them bit for bit.
    let page = l.page_mut(1).unwrap();
    for b in &mut page.boxes {
        b.rect_in = (Point::new(0.5, 1.25), Point::new(4.5, 3.75));
    }
    page.boxes.push(LayoutBox::new(
        99,
        (Point::new(1.0, 1.0), Point::new(3.0, 3.0)),
        BoxSource::CadDetail {
            name: "d".into(),
            items: vec![CadObject {
                id: 2,
                layer: "CAD, Default".into(),
                item: CadItem::Line {
                    a: Point::ZERO,
                    b: Point::new(1.0, 1.0),
                },
            }],
        },
        Scale::ThreeSixteenths,
    ));
    let l = Layout {
        pages: vec![l.pages[1].clone(), l.pages[4].clone()],
        ..l
    };
    let json = serde_json::to_string(&l).unwrap();
    let back: Layout = serde_json::from_str(&json).unwrap();
    assert_eq!(back.pages.len(), 2);
    assert_eq!(back.pages[1].boxes.len(), l.pages[1].boxes.len());
    assert_eq!(back.pages[0], l.pages[0]);
    assert_eq!(back.title_block, l.title_block);
    assert_eq!(back.sheet, l.sheet);
    assert!(json.contains("QuarterInch") && json.contains("Section"));
    // The section cut offset (-180.0) is exact too, so the whole layout matches.
    assert_eq!(back.pages[1].boxes[0].source, l.pages[1].boxes[0].source);
    assert_eq!(back, l);
    let elev = BoxSource::Elevation { dir: ViewDir::Left };
    let j = serde_json::to_string(&elev).unwrap();
    assert_eq!(serde_json::from_str::<BoxSource>(&j).unwrap(), elev);
}

#[test]
fn empty_layout_still_makes_a_pdf() {
    let p = Project::new("Empty");
    let cx = LayoutRenderContext::new(&p);
    let l = Layout::new("e", SheetSize::Letter);
    check_xref(&render_pdf(&l, &cx));
}
