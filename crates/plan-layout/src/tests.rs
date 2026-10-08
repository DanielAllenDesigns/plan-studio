//! Crate-level tests: sizing, packing, macros, PDF structure, clipping, JSON.

use super::*;
use crate::canvas::Prim;
use crate::extent::schedule_for;
use plan_core::{
    CadItem, CadObject, Dimension, DimensionKind, LineStyle, OpeningKind, Point, Project, WallKind,
};
use plan_docs::{Scale, SheetSize};
use plan_elevation::ViewDir;

/// A 40' x 30' house split into two rooms, with two doors and two windows.
pub(crate) fn two_room_house() -> Project {
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
        BoxSource::text("NOTE", 12.0),
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
    // Cover, site, plan, 2 elevation sheets, section, details, schedules,
    // materials, framing.
    assert_eq!(l.pages.len(), 10);
    assert_eq!(l.sheet, SheetSize::ArchC);
    let numbers: Vec<u32> = l.pages.iter().map(|p| p.number).collect();
    assert_eq!(numbers, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
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
        "(SHEET 1 OF 10)",
        "(SHEET 10 OF 10)",
        "(MATERIALS LIST - FRAMING)",
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
        pages: vec![l.pages[1].clone(), l.pages[5].clone()],
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

// ------------------------------------------------- Chief layout quality --

/// The content stream of every page, in order.
fn page_streams(pdf: &[u8]) -> Vec<String> {
    let t = text_of(pdf);
    let mut out = Vec::new();
    let mut rest = t.as_str();
    while let Some(i) = rest.find("stream\n") {
        let body = &rest[i + 7..];
        let end = body.find("endstream").expect("endstream");
        // Image data streams are binary garbage; page streams hold operators.
        if body[..end.min(80)].contains(" w\n") || body.starts_with("q ") {
            out.push(body[..end].to_string());
        }
        rest = &body[end + 9..];
    }
    out
}

fn vertical_dim_house() -> Project {
    let mut p = two_room_house();
    p.add_dimension(
        0,
        Dimension {
            id: 0,
            kind: DimensionKind::Manual,
            start: Point::new(0.0, 0.0),
            end: Point::new(0.0, 360.0),
            offset: 36.0,
            text_override: None,
            anchors: [None, None],
            hide_ext: [false, false],
            auto_group: Default::default(),
            text_style: None,
        },
    );
    p
}

#[test]
fn pdf_uses_layer_rgb_colors_clip_rects_and_page_background() {
    let p = vertical_dim_house();
    let cx = LayoutRenderContext::new(&p);
    let l = default_construction_set(&p, 1);
    let boxes: usize = l.pages.iter().map(|pg| pg.boxes.len()).sum();
    let clipped: usize = l
        .pages
        .iter()
        .flat_map(|pg| &pg.boxes)
        .filter(|b| b.clip)
        .count();
    assert!(clipped >= 8, "{clipped} of {boxes}");
    let pdf = render_pdf(&l, &cx);
    let t = text_of(&pdf);
    // Layer colors: Doors (0, 70, 200) stroke, the manual dimension (180, 0, 0) text.
    assert!(t.contains("0 0.275 0.784 RG"), "door layer stroke color");
    assert!(t.contains("0.706 0 0 rg"), "dimension text fill color");
    assert!(t.contains(" rg\n") && t.contains(" RG\n"));
    // One PDF clip rectangle per clipped box.
    let clips = t.matches("re W n").count();
    assert!(clips >= clipped, "{clips} clips for {clipped} boxes");
    assert_eq!(t.matches("re W n").count(), clipped);
    // The background is the first drawing on every page.
    let bg = "q 0.976 0.973 0.957 rg 0 0 1728 1296 re f Q";
    let streams = page_streams(&pdf);
    assert_eq!(streams.len(), l.pages.len());
    for (i, st) in streams.iter().enumerate() {
        let first_draw = st.find(" re f Q").expect("background rect");
        assert!(st[..first_draw + 7].ends_with(bg), "page {i}");
        for op in ["Tj", " S Q", " l S", " f Q\nq"] {
            if let Some(at) = st.find(op) {
                assert!(at > first_draw, "page {i}: {op} before the background");
            }
        }
    }
    // Daniel's layout edge: 0.18 mm = 0.51 pt page border.
    assert!(t.contains("q 0.51 w "));
}

#[test]
fn page_background_can_be_switched_off() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = default_construction_set(&p, 1);
    l.page_background = false;
    assert!(!text_of(&render_pdf(&l, &cx)).contains("0.976 0.973 0.957 rg"));
    assert!(Layout::new("n", SheetSize::ArchC).page_background);
}

#[test]
fn vertical_dimension_text_is_rotated() {
    let p = vertical_dim_house();
    let cx = LayoutRenderContext::new(&p);
    let l = default_construction_set(&p, 1);
    let t = text_of(&render_pdf(&l, &cx));
    // text_rotated at 90 degrees: cos 0, sin 1.
    assert!(t.contains(" 0 1 -1 0 "), "rotated Tm missing");
    assert!(
        t.contains(" Tm (30'-0\")") || t.contains("Tm (30'"),
        "dimension label"
    );
}

#[test]
fn layer_line_styles_become_pdf_dashes_and_weights_become_points() {
    let mut p = two_room_house();
    p.layers.get_mut("Doors").unwrap().line_style = LineStyle::Dashed;
    p.layers.get_mut("Doors").unwrap().line_weight = 100; // 1 mm
    let cx = LayoutRenderContext::new(&p);
    let mut l = Layout::new("t", SheetSize::ArchC);
    send_to_layout(&mut l, &cx, 1, plan_source(), Scale::QuarterInch, None);
    let t = text_of(&render_pdf(&l, &cx));
    assert!(t.contains("[6 3] 0 d"), "dash pattern");
    // 1 mm = 2.835 pt: the door jambs (solid) and swing use it scaled.
    assert!(t.contains("q 2.835 w "), "1 mm layer weight in points");
    let b = &l.pages[0].boxes[0];
    let n_dashed = render_box_lines(b, &cx).len();
    assert!(n_dashed > 10);
}

#[test]
fn bold_title_block_labels_and_room_names() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let l = default_construction_set(&p, 1);
    let t = text_of(&render_pdf(&l, &cx));
    // Bold font F2 is selected before the title block labels.
    assert!(t.contains("/F2 5.5 Tf"));
    let bold_label = t.find("(SHEET TITLE)").unwrap();
    assert!(t[..bold_label].rfind("/F2").unwrap() > t[..bold_label].rfind("/F1").unwrap_or(0));
}

#[test]
fn image_data_box_embeds_an_rgb_image_xobject() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = Layout::new("t", SheetSize::ArchC);
    let rgba: Vec<u8> = (0..8 * 4)
        .flat_map(|i| [i as u8 * 7, 40, 200, 255])
        .collect();
    let id = send_to_layout(
        &mut l,
        &cx,
        1,
        BoxSource::ImageData {
            width: 8,
            height: 4,
            rgba,
        },
        Scale::QuarterInch,
        None,
    );
    let (w, h) = l.page(1).unwrap().boxes[0].size_in();
    assert_eq!(id, 1);
    assert!((w - 4.0).abs() < 1e-9 && (h - 2.0).abs() < 1e-9);
    let pdf = render_pdf(&l, &cx);
    let t = text_of(&pdf);
    assert!(t.contains("/Subtype/Image") && t.contains("/ColorSpace/DeviceRGB"));
    assert!(t.contains("/Width 8 /Height 4"));
    assert!(t.contains(" Do Q"));
    check_xref(&pdf);
    // The placeholder frame still prints for path-only images, with no XObject.
    let mut l2 = Layout::new("t", SheetSize::ArchC);
    send_to_layout(
        &mut l2,
        &cx,
        1,
        BoxSource::Image {
            path: "a.png".into(),
        },
        Scale::QuarterInch,
        None,
    );
    assert!(!text_of(&render_pdf(&l2, &cx)).contains("/Subtype/Image"));
    // Bad pixel data degrades to a placeholder rather than a broken PDF.
    let mut l3 = Layout::new("t", SheetSize::ArchC);
    send_to_layout(
        &mut l3,
        &cx,
        1,
        BoxSource::ImageData {
            width: 3,
            height: 3,
            rgba: vec![0; 5],
        },
        Scale::QuarterInch,
        None,
    );
    let t3 = text_of(&render_pdf(&l3, &cx));
    assert!(!t3.contains("/Subtype/Image") && t3.contains("invalid pixel data"));
    // JSON round trip keeps the pixels.
    let json = serde_json::to_string(&l).unwrap();
    assert_eq!(serde_json::from_str::<Layout>(&json).unwrap(), l);
}

#[test]
fn auto_scale_picks_quarter_inch_on_arch_d_and_eighth_on_letter() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut d = Layout::new("d", SheetSize::ArchD);
    send_to_layout_auto(&mut d, &cx, 1, plan_source(), None, None);
    assert_eq!(d.page(1).unwrap().boxes[0].scale, Scale::QuarterInch);
    let mut c = Layout::new("c", SheetSize::ArchC);
    send_to_layout_auto(&mut c, &cx, 1, plan_source(), None, None);
    assert_eq!(c.page(1).unwrap().boxes[0].scale, Scale::QuarterInch);
    let mut ltr = Layout::new("l", SheetSize::Letter);
    send_to_layout_auto(&mut ltr, &cx, 1, plan_source(), None, None);
    assert_eq!(ltr.page(1).unwrap().boxes[0].scale, Scale::EighthInch);
    // Still within the drawing area.
    let (lo, hi) = ltr.drawing_area();
    let r = ltr.page(1).unwrap().boxes[0].bounds_in();
    assert!(r[0] >= lo.x - 1e-9 && r[2] <= hi.x + 1e-9);
    // An explicit scale is honored; the pure largest-fit search can go bigger.
    let mut e = Layout::new("e", SheetSize::ArchD);
    send_to_layout_auto(&mut e, &cx, 1, plan_source(), Some(Scale::EighthInch), None);
    assert_eq!(e.page(1).unwrap().boxes[0].scale, Scale::EighthInch);
    assert_eq!(
        fit_largest_scale(&d, &cx, &plan_source(), Scale::ThreeInch),
        Scale::HalfInch
    );
    // A plan far too big for Letter steps below 1/8" to 1" = 10'.
    let mut big = Project::new("Big");
    for (a, b) in [((0.0, 0.0), (800.0, 0.0)), ((800.0, 0.0), (800.0, 300.0))] {
        big.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            6.5,
            109.125,
            WallKind::Exterior,
        );
    }
    let bcx = LayoutRenderContext::new(&big);
    let s = fit_largest_scale(&ltr, &bcx, &plan_source(), Scale::QuarterInch);
    assert_eq!(s, Scale::OneInchEq10Ft);
}

#[test]
fn sheet_sizes_available_lists_all_with_current_first() {
    let l = Layout::new("t", SheetSize::Tabloid);
    let v = l.sheet_sizes_available();
    assert_eq!(v.len(), SheetSize::ALL.len());
    assert_eq!(v[0], SheetSize::Tabloid);
    assert_eq!(v.iter().filter(|s| **s == SheetSize::Tabloid).count(), 1);
    assert!(v.contains(&SheetSize::IsoA1));
}

#[test]
fn template_page_repeats_on_every_page_and_is_not_printed() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = Layout::new("t", SheetSize::ArchC);
    l.sheet_index = true;
    l.add_page(0, "Template").template_page = true;
    l.page_mut(0).unwrap().cad.push(CadObject {
        id: 1,
        layer: "CAD, Default".into(),
        item: CadItem::Text {
            pos: Point::new(1.0, 1.0),
            text: "ISSUED FOR %sheet.number% OF %page.count%".into(),
            height: 0.2,
            angle: 0.0,
        },
    });
    l.add_page(1, "Plan one");
    l.add_page(2, "Plan two");
    assert_eq!(l.content_pages().len(), 2);
    assert_eq!(l.template_pages().len(), 1);
    let pdf = render_pdf(&l, &cx);
    let t = text_of(&pdf);
    assert_eq!(t.matches("/Type /Page").count(), 2);
    assert!(t.contains("(ISSUED FOR A-1 OF 2)") && t.contains("(ISSUED FOR A-2 OF 2)"));
    assert!(t.contains("(SHEET 2 OF 2)"));
    // The template is not in the sheet index.
    assert!(t.contains("(A-1)") && !t.contains("(TEMPLATE)"));
    check_xref(&pdf);
}

#[test]
fn new_macros_fill_the_title_block() {
    let p = two_room_house();
    let mut cx = LayoutRenderContext::new(&p);
    cx.macros.client = "Jane Smith".into();
    cx.macros.address = "12 Oak Lane".into();
    cx.macros.project_number = "26-014".into();
    cx.macros.revision = "B".into();
    cx.macros.date = "2026-10-07".into();
    let mut l = default_construction_set(&p, 1);
    l.title_block
        .fields
        .push(("JOB".into(), "No. %project.number%".into()));
    l.title_block
        .fields
        .push(("REV".into(), "%revision% of %page.count%".into()));
    l.title_block
        .fields
        .push(("ISSUED".into(), "%date.long%".into()));
    let t = text_of(&render_pdf(&l, &cx));
    for want in [
        "(12 Oak Lane)",
        "(No. 26-014)",
        "(B of 10)",
        "(October 7, 2026)",
    ] {
        assert!(t.contains(want), "missing {want}");
    }
    assert!(!t.contains("%page.count%") && !t.contains("%date.long%"));
}

#[test]
fn daniel_title_block_has_a_five_row_revision_table() {
    let p = two_room_house();
    let mut cx = LayoutRenderContext::new(&p);
    cx.macros.revisions = vec![
        ("1".into(), "2026-09-01".into(), "ISSUED FOR REVIEW".into()),
        ("2".into(), "2026-09-20".into(), "CLIENT COMMENTS".into()),
    ];
    let l = default_construction_set(&p, 1);
    assert_eq!(l.title_block, TitleBlockTemplate::from_daniel_18x24());
    let t = text_of(&render_pdf(&l, &cx));
    for want in [
        "(REVISIONS)",
        "(DESCRIPTION)",
        "(ISSUED FOR REVIEW)",
        "(CLIENT COMMENTS)",
    ] {
        assert!(t.contains(want), "missing {want}");
    }
    // The presentation block has no table.
    let mut plain = l.clone();
    plain.title_block = TitleBlockTemplate::presentation_18x24();
    assert!(!text_of(&render_pdf(&plain, &cx)).contains("(REVISIONS)"));
}

#[test]
fn multi_floor_plan_labels_are_spelled_out() {
    let mut p = two_room_house();
    p.build_new_floor(true);
    let cx = LayoutRenderContext::new(&p);
    let mut l = Layout::new("t", SheetSize::ArchC);
    for floor in 0..2 {
        send_to_layout(
            &mut l,
            &cx,
            floor as u32 + 1,
            BoxSource::PlanView {
                floor,
                layer_set: "Floor Plan".into(),
            },
            Scale::EighthInch,
            None,
        );
    }
    assert_eq!(
        l.page(1).unwrap().boxes[0].label.as_deref(),
        Some("FIRST FLOOR PLAN")
    );
    assert_eq!(
        l.page(2).unwrap().boxes[0].label.as_deref(),
        Some("SECOND FLOOR PLAN")
    );
    let t = text_of(&render_pdf(&l, &cx));
    assert!(t.contains("(FIRST FLOOR PLAN)") && t.contains("(SECOND FLOOR PLAN)"));
    assert!(t.contains("(SCALE: 1/8\" = 1'-0\")"));
}

#[test]
fn elevation_hatch_adds_strokes_and_can_be_switched_off() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let l = default_construction_set(&p, 1);
    let mut b = l
        .pages
        .iter()
        .flat_map(|pg| &pg.boxes)
        .find(|b| matches!(b.source, BoxSource::Elevation { .. }))
        .unwrap()
        .clone();
    assert!(b.hatch_materials);
    let with = render_box_lines(&b, &cx).len();
    b.hatch_materials = false;
    let without = render_box_lines(&b, &cx).len();
    assert!(with > without + 10, "{with} vs {without}");
    // The helper works on a bare scene too.
    let scene = plan_3d::build_scene(&p);
    let strokes = wall_face_hatch(&scene, ViewDir::Front, None, 0.25);
    assert!(!strokes.is_empty());
}

#[test]
fn old_layout_json_without_new_fields_still_loads() {
    let p = two_room_house();
    let l = default_construction_set(&p, 1);
    let mut v = serde_json::to_value(&l).unwrap();
    v.as_object_mut().unwrap().remove("page_background");
    v.as_object_mut().unwrap().remove("edge_line_weight");
    for pg in v["pages"].as_array_mut().unwrap() {
        pg.as_object_mut().unwrap().remove("template_page");
        for b in pg["boxes"].as_array_mut().unwrap() {
            b.as_object_mut().unwrap().remove("hatch_materials");
        }
    }
    v["title_block"]
        .as_object_mut()
        .unwrap()
        .remove("revision_rows");
    let back: Layout = serde_json::from_value(v).unwrap();
    assert!(back.page_background && back.edge_line_weight == 18);
    assert!(back
        .pages
        .iter()
        .flat_map(|p| &p.boxes)
        .all(|b| b.hatch_materials));
    assert_eq!(back.title_block.revision_rows, 0);
}

#[test]
fn camera_boxes_draw_what_the_hook_returns() {
    use plan_elevation::{Drawing, EdgeKind, Line2, LineWeight};
    let p = two_room_house();
    let drawing = Drawing::new(vec![Line2 {
        a: Point::new(0.0, 0.0),
        b: Point::new(240.0, 120.0),
        weight: LineWeight::Heavy,
        kind: EdgeKind::Silhouette,
    }]);
    let hooked = LayoutRenderContext::new(&p)
        .with_camera_drawing(move |id| (id == 7).then(|| drawing.clone()));
    let source = BoxSource::Camera { camera_id: 7 };
    let mut l = Layout::new("t", SheetSize::ArchC);
    let id = send_to_layout(&mut l, &hooked, 1, source, Scale::QuarterInch, None);
    let b = l
        .page(1)
        .unwrap()
        .boxes
        .iter()
        .find(|b| b.id == id)
        .unwrap();
    // 240" x 120" plus the 12" margin each side at 1/4" = 1'.
    let (w, h) = b.size_in();
    assert!((w - (240.0 + 24.0) / 48.0).abs() < 1e-9, "w = {w}");
    assert!((h - (120.0 + 24.0) / 48.0).abs() < 1e-9, "h = {h}");
    assert_eq!(b.label.as_deref(), Some("CAMERA VIEW"));
    // The box strokes the diagonal (plus its border); without the hook only
    // the border is drawn.
    let with = render_box_lines(b, &hooked);
    let without = render_box_lines(b, &LayoutRenderContext::new(&p));
    assert_eq!(
        with.len(),
        without.len() + 1,
        "{} vs {}",
        with.len(),
        without.len()
    );
    assert!(render_pdf(&l, &hooked).len() > render_pdf(&l, &LayoutRenderContext::new(&p)).len());
    // The hook is asked once however often the box is measured and drawn.
    let calls = std::cell::Cell::new(0);
    let counting = LayoutRenderContext::new(&p).with_camera_drawing(|_| {
        calls.set(calls.get() + 1);
        None
    });
    let _ = render_box_lines(b, &counting);
    let _ = render_box_lines(b, &counting);
    assert_eq!(calls.get(), 1);
    // The source survives a JSON round trip.
    let json = serde_json::to_string(&l).unwrap();
    let back: Layout = serde_json::from_str(&json).unwrap();
    assert!(matches!(
        back.page(1).unwrap().boxes[0].source,
        BoxSource::Camera { camera_id: 7 }
    ));
}

fn stroke_bounds(prims: &[Prim]) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in prims {
        if let Prim::Stroke { pts, .. } = p {
            for q in pts {
                b = [b[0].min(q.0), b[1].min(q.1), b[2].max(q.0), b[3].max(q.1)];
            }
        }
    }
    b
}

#[test]
fn a_placed_schedule_box_shows_the_table_of_the_plan() {
    use plan_core::schedules::{Schedule, ScheduleKind as K, ScheduleLayer};
    let mut p = two_room_house();
    let mut def = Schedule::new(K::Door, Point::ZERO);
    def.totals = true;
    let mut layer = ScheduleLayer::default();
    let id = layer.add(def);
    layer.store(&mut p.floors[0]);
    let cx = LayoutRenderContext::new(&p);
    let src = BoxSource::PlacedSchedule { floor: 0, id };
    let (w, h) = source_size_in(&src, Scale::QuarterInch, &cx);
    assert!(w > 1.0 && h > 0.5, "{w} x {h}");
    let mut l = Layout::new("t", SheetSize::ArchC);
    l.add_page(1, "Schedules");
    send_to_layout(&mut l, &cx, 1, src.clone(), Scale::QuarterInch, None);
    let pdf = render_pdf(&l, &cx);
    // The table title and the totals line are printed.
    let text = text_of(&pdf);
    assert!(text.contains("Door Schedule"), "title in the PDF");
    assert!(text.contains("Total"), "totals line in the PDF");
    // A schedule that is gone leaves a labelled placeholder and still prints.
    let mut gone = l.clone();
    gone.pages[0].boxes[0].source = BoxSource::PlacedSchedule { floor: 0, id: 999 };
    let pdf = render_pdf(&gone, &cx);
    check_xref(&pdf);
    assert!(text_of(&pdf).contains("not found"));
    // The source survives the JSON.
    let back: Layout = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
    assert_eq!(back, l);
}

#[test]
fn a_rotated_box_turns_its_content_about_its_centre() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let scenes = crate::extent::SceneSource::new(None);
    // A wide text box: 4" x 1".
    let mut b = LayoutBox::new(
        1,
        (Point::new(1.0, 1.0), Point::new(5.0, 2.0)),
        BoxSource::text("ROTATE ME", 10.0),
        Scale::QuarterInch,
    );
    b.border = true;
    b.clip = false;
    let flat = crate::render::box_prims_for_test(&b, &cx, &scenes);
    assert_eq!(b.quarter_turns(), 0);
    let text_angle = |prims: &[Prim]| {
        prims.iter().find_map(|p| match p {
            Prim::Text { angle, text, .. } if text == "ROTATE ME" => Some(*angle),
            _ => None,
        })
    };
    assert_eq!(text_angle(&flat), Some(0.0));
    b.rotation_deg = 90.0;
    assert_eq!(b.quarter_turns(), 1);
    let turned = crate::render::box_prims_for_test(&b, &cx, &scenes);
    let a = text_angle(&turned).unwrap();
    assert!((a - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    // The frame (the border) stays where the box is.
    let frame = |prims: &[Prim]| {
        let borders: Vec<Prim> = prims
            .iter()
            .filter(|p| matches!(p, Prim::Stroke { closed: true, .. }))
            .cloned()
            .collect();
        stroke_bounds(&borders)
    };
    assert_eq!(frame(&flat), frame(&turned));
    // 270 and -90 are the same turn; 360 is none.
    b.rotation_deg = -90.0;
    assert_eq!(b.quarter_turns(), 3);
    b.rotation_deg = 360.0;
    assert_eq!(b.quarter_turns(), 0);
    // The turned plan content stays inside the box's centre line extent:
    // a 90 degree turn of a plan lays it out in the swapped box.
    let mut plan = LayoutBox::new(
        2,
        (Point::new(1.0, 1.0), Point::new(9.0, 5.0)),
        plan_source(),
        Scale::EighthInch,
    );
    plan.border = false;
    plan.clip = false;
    plan.rotation_deg = 90.0;
    let turned = stroke_bounds(&crate::render::box_prims_for_test(&plan, &cx, &scenes));
    let (cx_pt, cy_pt) = (5.0 * 72.0, 3.0 * 72.0);
    assert!(
        turned[0] <= cx_pt && turned[2] >= cx_pt && turned[1] <= cy_pt && turned[3] >= cy_pt,
        "{turned:?} around the centre"
    );
    // Old files without the field load as unrotated.
    let mut v = serde_json::to_value(&b).unwrap();
    v.as_object_mut().unwrap().remove("rotation_deg");
    let old: LayoutBox = serde_json::from_value(v).unwrap();
    assert_eq!(old.quarter_turns(), 0);
}

#[test]
fn a_portrait_layout_swaps_the_sheet_and_prints_upright() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = Layout::new("t", SheetSize::ArchC);
    l.add_page(1, "Plan");
    let (w, h) = l.sheet_inches();
    assert_eq!((w, h), SheetSize::ArchC.inches());
    l.portrait = true;
    let (pw, ph) = l.sheet_inches();
    assert_eq!((pw, ph), (h.min(w), h.max(w)));
    // The drawing area follows the sheet.
    let (lo, hi) = l.drawing_area();
    assert!(hi.y - lo.y > hi.x - lo.x);
    let pdf = text_of(&render_pdf(&l, &cx));
    let media = format!("/MediaBox [0 0 {} {}]", pw * 72.0, ph * 72.0);
    assert!(
        pdf.contains(&media) || pdf.contains(&format!("{} {}", pw * 72.0, ph * 72.0)),
        "{media}"
    );
    let back: Layout = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
    assert!(back.portrait);
    let mut v = serde_json::to_value(&l).unwrap();
    v.as_object_mut().unwrap().remove("portrait");
    let old: Layout = serde_json::from_value(v).unwrap();
    assert!(!old.portrait);
}

#[test]
fn project_information_macros_reach_the_title_block() {
    let mut p = two_room_house();
    p.info.company = "Daniel Allen Designs".into();
    p.info.client_phone = "404-555-0100".into();
    p.info.checked_by = "DS".into();
    p.info.custom.push(("permit".into(), "BP-22".into()));
    let ctx = crate::render::macros_for(&p);
    assert_eq!(
        ctx.expand("%company%|%client.phone%|%checked.by%|%custom.permit%"),
        "Daniel Allen Designs|404-555-0100|DS|BP-22"
    );
    // The built-in macros are not duplicated into the extras.
    assert!(ctx.extra.iter().all(|(k, _)| !MacroContext::is_builtin(k)));
}

/// The font size (points) of the text op that shows `shown`, e.g. `(KITCHEN)`.
fn font_size_of(pdf_text: &str, shown: &str) -> f64 {
    let at = pdf_text
        .find(shown)
        .unwrap_or_else(|| panic!("{shown} not in the page"));
    let tf = pdf_text[..at]
        .rfind(" Tf")
        .expect("a font op before the text");
    let before = &pdf_text[..tf];
    before
        .rsplit(' ')
        .next()
        .and_then(|n| n.parse().ok())
        .expect("a size before Tf")
}

#[test]
fn printed_size_text_prints_the_same_size_at_any_box_scale() {
    let render = |p: &Project, scale: Scale| -> String {
        let cx = LayoutRenderContext::new(p);
        let mut l = Layout::new("t", SheetSize::ArchC);
        send_to_layout(&mut l, &cx, 1, plan_source(), scale, None);
        text_of(&render_pdf(&l, &cx))
    };
    let mut p = vertical_dim_house();
    p.add_cad(
        0,
        "Text",
        CadItem::Text {
            pos: Point::new(100.0, 100.0),
            text: "KITCHEN".into(),
            height: 6.0,
            angle: 0.0,
        },
    );
    // Character height: the plan height scales with the box (6" is 9 pt at
    // 1/4", 4.5 pt at 1/8"); the dimension number is 4.5" tall in the plan.
    let (q, e) = (
        render(&p, Scale::QuarterInch),
        render(&p, Scale::EighthInch),
    );
    assert!((font_size_of(&q, "(KITCHEN)") - 9.0).abs() < 1e-6);
    assert!((font_size_of(&e, "(KITCHEN)") - 4.5).abs() < 1e-6);
    assert!((font_size_of(&q, "(30'-0\")") - 6.75).abs() < 1e-6);
    assert!((font_size_of(&e, "(30'-0\")") - 3.375).abs() < 1e-6);
    // Printed size: 1/8" (9 pt) text and 3/32" (6.75 pt) dimension numbers on
    // the page whatever the scale.
    for name in ["Default Text Style", "Dimension Text Style"] {
        let mut st = p.text_styles.get(name).unwrap().clone();
        st.use_printed_size(true);
        let i = p
            .text_styles
            .styles
            .iter()
            .position(|s| s.name == name)
            .unwrap();
        p.text_styles.styles[i] = st;
    }
    for scale in [Scale::QuarterInch, Scale::EighthInch, Scale::HalfInch] {
        let t = render(&p, scale);
        assert!(
            (font_size_of(&t, "(KITCHEN)") - 9.0).abs() < 1e-6,
            "{scale:?} text"
        );
        assert!(
            (font_size_of(&t, "(30'-0\")") - 6.75).abs() < 1e-6,
            "{scale:?} dimension"
        );
    }
}
