//! Page leaders and clouds, the layout layer set, text fitting, the door and
//! window symbols, perspective quality, the sheet index and Daniel's sheet
//! set, Print Model.

use crate::canvas::{Canvas, Prim};
use crate::extent::SceneSource;
use crate::render::{box_prims, draw_page};
use crate::tests::two_room_house;
use crate::*;
use plan_core::opening_symbol::plan_symbol;
use plan_core::{OpeningKind, OpeningStyle, Point};
use plan_docs::{Scale, SheetSize};
use std::cell::Cell;

fn text_of(pdf: &[u8]) -> String {
    pdf.iter().map(|&b| b as char).collect()
}

fn page_prims(layout: &Layout, cx: &LayoutRenderContext) -> Vec<Prim> {
    let scenes = SceneSource::new(cx.scene);
    let pages = layout.content_pages();
    let mut cv = Canvas::new();
    draw_page(&mut cv, layout, &pages, 0, cx, &scenes);
    cv.prims
}

fn one_page_layout() -> Layout {
    let mut l = Layout::new("T", SheetSize::ArchC);
    l.page_background = false;
    l.add_page(1, "Page");
    l
}

fn texts(prims: &[Prim]) -> Vec<(String, f64)> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Text { text, size, .. } => Some((text.clone(), *size)),
            _ => None,
        })
        .collect()
}

fn widths(prims: &[Prim]) -> Vec<f64> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Stroke { pen, .. } => Some(pen.width),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------ leaders, clouds --

#[test]
fn a_leader_prints_its_line_arrowhead_and_text() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = one_page_layout();
    let page = l.page_mut(1).unwrap();
    page.add_leader(
        Point::new(5.0, 5.0),
        Point::new(7.0, 6.0),
        "VERIFY IN FIELD",
        0.125,
    );
    let prims = page_prims(&l, &cx);
    assert!(texts(&prims)
        .iter()
        .any(|(t, s)| t == "VERIFY IN FIELD" && (*s - 9.0).abs() < 1e-9));
    assert!(prims
        .iter()
        .any(|p| matches!(p, Prim::Fill { pts, .. } if pts.len() == 3)));
    // The line runs from the tip (5, 5) in points.
    assert!(prims.iter().any(|p| matches!(p, Prim::Stroke { pts, closed: false, .. } if pts.first() == Some(&(360.0, 360.0)))));
    // Hiding the Text layer drops the words but keeps the line.
    l.layers.set_visible(LAYER_TEXT, false);
    let prims = page_prims(&l, &cx);
    assert!(!texts(&prims).iter().any(|(t, _)| t == "VERIFY IN FIELD"));
    assert!(prims
        .iter()
        .any(|p| matches!(p, Prim::Fill { pts, .. } if pts.len() == 3)));
}

#[test]
fn a_revision_cloud_prints_a_closed_scalloped_loop_and_its_tag() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = one_page_layout();
    l.page_mut(1)
        .unwrap()
        .add_cloud(Point::new(4.0, 4.0), Point::new(8.0, 6.0), "3");
    let prims = page_prims(&l, &cx);
    let loops: Vec<usize> = prims
        .iter()
        .filter_map(|p| match p {
            Prim::Stroke {
                pts,
                closed: true,
                pen,
            } if pts.len() > 30 && pen.width >= 1.0 => Some(pts.len()),
            _ => None,
        })
        .collect();
    assert_eq!(loops.len(), 1, "one cloud outline");
    assert!(
        texts(&prims).iter().any(|(t, _)| t == "3"),
        "the revision tag"
    );
    l.layers.set_visible(LAYER_REVISION_CLOUDS, false);
    let prims = page_prims(&l, &cx);
    assert!(!texts(&prims).iter().any(|(t, _)| t == "3"));
}

#[test]
fn arcs_and_circles_on_a_page_print_as_strokes_on_the_cad_layer() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = one_page_layout();
    let page = l.page_mut(1).unwrap();
    page.add_circle(Point::new(6.0, 6.0), 1.0);
    page.add_arc(Point::new(10.0, 6.0), 1.0, 0.0, std::f64::consts::PI);
    assert!(page.cad.iter().all(|o| o.layer == LAYER_CAD));
    let before = page_prims(&l, &cx);
    let n_circle_like = before
        .iter()
        .filter(|p| matches!(p, Prim::Stroke { pts, .. } if pts.len() >= 20))
        .count();
    assert_eq!(n_circle_like, 2);
    l.layers.set_visible(LAYER_CAD, false);
    let after = page_prims(&l, &cx);
    assert!(
        after
            .iter()
            .filter(|p| matches!(p, Prim::Stroke { pts, .. } if pts.len() >= 20))
            .count()
            < 2
    );
}

#[test]
fn old_layouts_without_the_new_fields_load_with_the_default_layers() {
    let l = one_page_layout();
    let mut v = serde_json::to_value(&l).unwrap();
    v.as_object_mut().unwrap().remove("layers");
    for p in v["pages"].as_array_mut().unwrap() {
        p.as_object_mut().unwrap().remove("leaders");
        p.as_object_mut().unwrap().remove("clouds");
    }
    let back: Layout = serde_json::from_value(v).unwrap();
    assert_eq!(back.layers, LayoutLayers::default());
    assert!(back.pages[0].leaders.is_empty() && back.pages[0].clouds.is_empty());
}

// ---------------------------------------------------------------- layers --

#[test]
fn layer_weights_set_the_pens_and_hiding_removes_borders_and_the_title_block() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = one_page_layout();
    let page = l.page_mut(1).unwrap();
    page.add_line(Point::new(2.0, 2.0), Point::new(4.0, 2.0));
    let mut b = LayoutBox::new(
        1,
        (Point::new(6.0, 6.0), Point::new(10.0, 9.0)),
        BoxSource::text("hello", 12.0),
        Scale::QuarterInch,
    );
    b.border = true;
    page.boxes.push(b);
    let w0 = widths(&page_prims(&l, &cx));
    assert!(
        w0.iter().any(|w| (*w - 0.5).abs() < 1e-9),
        "CAD line at its layer weight"
    );
    assert!(
        w0.iter().any(|w| (*w - 0.75).abs() < 1e-9),
        "box border at its layer weight"
    );
    l.layers.set_weight(LAYER_CAD, 2.0);
    l.layers.set_weight(LAYER_BOX_BORDERS, 1.5);
    let w1 = widths(&page_prims(&l, &cx));
    assert!(w1.iter().any(|w| (*w - 2.0).abs() < 1e-9));
    assert!(w1.iter().any(|w| (*w - 1.5).abs() < 1e-9));
    // Hide borders and the title block.
    l.layers.set_visible(LAYER_BOX_BORDERS, false);
    l.layers.set_visible(LAYER_TITLE_BLOCK, false);
    let prims = page_prims(&l, &cx);
    assert!(
        !widths(&prims).iter().any(|w| (*w - 1.5).abs() < 1e-9),
        "no border"
    );
    assert!(
        !texts(&prims).iter().any(|(t, _)| t.starts_with("SHEET ")),
        "no title block"
    );
    // The title block weight scales its pens.
    let mut m = one_page_layout();
    let base = widths(&page_prims(&m, &cx))
        .iter()
        .cloned()
        .fold(0.0, f64::max);
    m.layers.set_weight(LAYER_TITLE_BLOCK, 1.5);
    let heavy = widths(&page_prims(&m, &cx))
        .iter()
        .cloned()
        .fold(0.0, f64::max);
    assert!((heavy / base - 2.0).abs() < 1e-6, "{base} -> {heavy}");
}

// ------------------------------------------------------------ text boxes --

fn text_box_prims(text: &str, fit: TextFit, clip: bool, w: f64, h: f64, pt: f64) -> Vec<Prim> {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let scenes = SceneSource::new(None);
    let mut b = LayoutBox::new(
        1,
        (Point::new(1.0, 1.0), Point::new(1.0 + w, 1.0 + h)),
        BoxSource::text(text, pt),
        Scale::QuarterInch,
    );
    b.border = false;
    b.clip = clip;
    b.text_fit = fit;
    box_prims(&b, &cx, &scenes, &LayoutLayers::default())
}

const SENTENCE: &str = "General notes: all dimensions are to face of stud unless noted otherwise";

#[test]
fn text_wraps_at_the_box_width() {
    let prims = text_box_prims(SENTENCE, TextFit::Wrap, true, 1.5, 3.0, 10.0);
    let t = texts(&prims);
    assert!(t.len() >= 4, "{t:?}");
    for (line, size) in &t {
        assert!(
            plan_docs::PdfDoc::text_width(line, *size) <= 1.5 * 72.0,
            "{line}"
        );
    }
    assert_eq!(
        t.iter()
            .map(|(l, _)| l.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        SENTENCE
    );
}

#[test]
fn off_keeps_one_line_per_break() {
    let prims = text_box_prims(SENTENCE, TextFit::Off, true, 2.0, 3.0, 10.0);
    assert_eq!(texts(&prims).len(), 1);
}

#[test]
fn lines_below_the_box_are_dropped_and_a_clipping_box_keeps_the_half_cut_one() {
    // 0.5" tall = 36 pt: 10 pt type at 1.25 spacing starts lines at 0, 12.5, 25 (and 37.5 is out).
    let clipped = text_box_prims(SENTENCE, TextFit::Wrap, true, 2.0, 0.5, 10.0);
    assert_eq!(texts(&clipped).len(), 3);
    // Without a clip only whole lines are drawn (two fit in 36 pt).
    let free = text_box_prims(SENTENCE, TextFit::Wrap, false, 2.0, 0.5, 10.0);
    assert_eq!(texts(&free).len(), 2);
}

#[test]
fn shrink_to_fit_sets_all_the_text_in_smaller_type() {
    let prims = text_box_prims(SENTENCE, TextFit::Shrink, true, 2.0, 0.5, 12.0);
    let t = texts(&prims);
    let joined = t
        .iter()
        .map(|(l, _)| l.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(joined, SENTENCE, "nothing dropped");
    assert!(t.iter().all(|(_, s)| *s < 12.0));
    assert!(t.len() as f64 * t[0].1 * 1.25 <= 36.0 + 1e-6);
}

#[test]
fn text_box_fit_round_trips_through_json() {
    let mut b = LayoutBox::new(
        1,
        (Point::new(0.0, 0.0), Point::new(2.0, 1.0)),
        BoxSource::text("x", 10.0),
        Scale::QuarterInch,
    );
    b.text_fit = TextFit::Shrink;
    b.dpi = 150;
    b.samples = 24;
    let back: LayoutBox = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
    assert_eq!(back, b);
    let mut v = serde_json::to_value(&b).unwrap();
    for k in ["dpi", "samples", "text_fit"] {
        v.as_object_mut().unwrap().remove(k);
    }
    let old: LayoutBox = serde_json::from_value(v).unwrap();
    assert_eq!((old.dpi, old.samples, old.text_fit), (0, 0, TextFit::Wrap));
}

// --------------------------------------------- doors and windows on pages --

fn plan_box(p: &plan_core::Project) -> (LayoutBox, LayoutRenderContext<'_>) {
    let cx = LayoutRenderContext::new(p);
    let mut b = LayoutBox::new(
        1,
        (Point::new(1.0, 1.0), Point::new(16.0, 14.0)),
        BoxSource::PlanView {
            floor: 0,
            layer_set: "All".into(),
        },
        Scale::QuarterInch,
    );
    b.border = false;
    (b, cx)
}

fn stroke_count(p: &plan_core::Project) -> (usize, usize) {
    let (b, cx) = plan_box(p);
    let prims = crate::render::box_prims_for_test(&b, &cx, &SceneSource::new(None));
    let strokes = prims
        .iter()
        .filter(|p| matches!(p, Prim::Stroke { .. }))
        .count();
    let dashed = prims
        .iter()
        .filter(|p| matches!(p, Prim::Stroke { pen, .. } if pen.dash != crate::canvas::Dash::Solid))
        .count();
    (strokes, dashed)
}

#[test]
fn every_door_and_window_style_draws_its_plan_symbol_on_the_page() {
    let base = two_room_house();
    let (base_strokes, base_dashed) = stroke_count(&base);
    // The same house with the first door and first window turned into other
    // styles draws a different symbol (the page used to know only a hinged
    // door and a plain window).
    let door_styles = [
        OpeningStyle::Sliding,
        OpeningStyle::Pocket,
        OpeningStyle::Bifold,
        OpeningStyle::Barn,
        OpeningStyle::DoubleDoor,
        OpeningStyle::Garage,
    ];
    let mut seen = std::collections::BTreeSet::new();
    for style in door_styles {
        let mut p = two_room_house();
        let id = p.floors[0]
            .openings
            .iter()
            .find(|o| o.kind == OpeningKind::Door)
            .unwrap()
            .id;
        p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap()
            .style = style;
        let (strokes, dashed) = stroke_count(&p);
        // The symbol of the opening is exactly what plan_symbol says.
        let f = &p.floors[0];
        let o = f.openings.iter().find(|o| o.id == id).unwrap();
        let w = f.wall(o.wall_id).unwrap();
        let sym = plan_symbol(w, o, 1.0);
        assert!(!sym.parts.is_empty());
        seen.insert((strokes, dashed));
        assert!(strokes > 0);
    }
    assert!(
        seen.len() >= 3,
        "styles draw differently: {seen:?} vs {base_strokes} {base_dashed}"
    );
    // A pocket door shows its hidden pocket as a dashed line.
    let mut p = two_room_house();
    let id = p.floors[0]
        .openings
        .iter()
        .find(|o| o.kind == OpeningKind::Door)
        .unwrap()
        .id;
    p.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
        .unwrap()
        .style = OpeningStyle::Pocket;
    assert!(stroke_count(&p).1 > base_dashed);
}

#[test]
fn a_bay_window_projects_past_the_wall_on_the_page() {
    let flat = two_room_house();
    let mut bay = two_room_house();
    let id = bay.floors[0]
        .openings
        .iter()
        .find(|o| o.kind == OpeningKind::Window)
        .unwrap()
        .id;
    bay.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
        .unwrap()
        .style = OpeningStyle::BayWindow;
    let extent = |p: &plan_core::Project| {
        let (b, cx) = plan_box(p);
        let prims = crate::render::box_prims_for_test(&b, &cx, &SceneSource::new(None));
        let mut lo = (f64::INFINITY, f64::INFINITY);
        let mut hi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in prims {
            if let Prim::Stroke { pts, .. } = p {
                for (x, y) in pts {
                    lo = (lo.0.min(x), lo.1.min(y));
                    hi = (hi.0.max(x), hi.1.max(y));
                }
            }
        }
        (lo, hi)
    };
    let (l0, h0) = extent(&flat);
    let (l1, h1) = extent(&bay);
    assert!(l1.1 < l0.1 - 1.0 || h1.1 > h0.1 + 1.0 || l1.0 < l0.0 - 1.0 || h1.0 > h0.0 + 1.0);
    let f = &bay.floors[0];
    let o = f.openings.iter().find(|o| o.id == id).unwrap();
    let w = f.wall(o.wall_id).unwrap();
    assert!(plan_symbol(w, o, 1.0).max_reach(w) > w.thickness * 0.5 + 10.0);
}

// ----------------------------------------------------------- perspectives --

#[test]
fn perspective_pixels_follow_the_dpi_and_stay_in_budget() {
    assert_eq!(perspective_pixels(6.0, 4.5, 0), (480, 360));
    assert_eq!(perspective_pixels(6.0, 4.5, 150), (900, 675));
    let (w, h) = perspective_pixels(22.0, 16.0, 600);
    assert!(u64::from(w) * u64::from(h) <= MAX_PERSPECTIVE_PIXELS + 10_000);
    assert!(w <= MAX_PERSPECTIVE_SIDE_PX && h <= MAX_PERSPECTIVE_SIDE_PX);
    assert!(
        (f64::from(w) / f64::from(h) - 22.0 / 16.0).abs() < 0.01,
        "aspect kept"
    );
    assert_eq!(perspective_pixels(0.01, 0.01, 1), (8, 8));
}

#[test]
fn a_perspective_box_asks_the_renderer_for_its_dpi_samples_and_size() {
    let p = two_room_house();
    let asked: Cell<Option<PerspectiveRequest>> = Cell::new(None);
    let cx = LayoutRenderContext::new(&p).with_perspective_render(|r| {
        asked.set(Some(*r));
        Some(PerspectiveImage {
            width: r.width,
            height: r.height,
            rgba: vec![200; (r.width * r.height * 4) as usize],
        })
    });
    let mut b = LayoutBox::new(
        1,
        (Point::new(1.0, 1.0), Point::new(7.0, 5.5)),
        BoxSource::Perspective { camera_id: 4 },
        Scale::QuarterInch,
    );
    let scenes = SceneSource::new(None);
    let lay = LayoutLayers::default();
    box_prims(&b, &cx, &scenes, &lay);
    let r = asked.get().unwrap();
    assert_eq!(
        (r.camera_id, r.width, r.height, r.samples),
        (4, 480, 360, 8)
    );
    b.dpi = 150;
    b.samples = 32;
    let cx2 = LayoutRenderContext::new(&p).with_perspective_render(|r| {
        asked.set(Some(*r));
        None
    });
    box_prims(&b, &cx2, &scenes, &lay);
    let r = asked.get().unwrap();
    assert_eq!((r.width, r.height, r.samples), (900, 675, 32));
    // A box turned a quarter is rendered upright in its turned frame.
    b.rotation_deg = 90.0;
    box_prims(
        &b,
        &LayoutRenderContext::new(&p).with_perspective_render(|r| {
            asked.set(Some(*r));
            None
        }),
        &scenes,
        &lay,
    );
    let r = asked.get().unwrap();
    assert_eq!((r.width, r.height), (675, 900));
}

#[test]
fn the_camera_only_hook_still_serves_a_box_without_the_sized_one() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p).with_perspective_image(|_| {
        Some(PerspectiveImage {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
        })
    });
    let mut l = one_page_layout();
    l.page_mut(1).unwrap().boxes.push(LayoutBox::new(
        1,
        (Point::new(1.0, 1.0), Point::new(7.0, 5.5)),
        BoxSource::Perspective { camera_id: 1 },
        Scale::QuarterInch,
    ));
    assert!(text_of(&render_pdf(&l, &cx)).contains("/Subtype/Image"));
}

#[test]
fn print_model_embeds_one_render_at_the_dpi_asked() {
    let p = two_room_house();
    let asked: Cell<Option<PerspectiveRequest>> = Cell::new(None);
    let cx = LayoutRenderContext::new(&p).with_perspective_render(|r| {
        asked.set(Some(*r));
        Some(PerspectiveImage {
            width: r.width,
            height: r.height,
            rgba: vec![180; (r.width * r.height * 4) as usize],
        })
    });
    let opts = PrintOptions::default(); // Letter landscape, 0.25" margins
    let pdf = print_model_pdf(&cx, 2, 100, 16, "FRONT VIEW", &opts);
    assert!(pdf.starts_with(b"%PDF-1.4"));
    let r = asked.get().unwrap();
    // 10.5" x 7.7" printable at 100 dpi.
    assert_eq!(
        (r.camera_id, r.width, r.height, r.samples),
        (2, 1050, 770, 16)
    );
    let t = text_of(&pdf);
    assert!(t.contains("/Subtype/Image") && t.contains("FRONT VIEW"));
    assert!(t.contains("1050 x 770 px"));
    // A higher DPI asks for more pixels.
    print_model_pdf(&cx, 2, 200, 16, "x", &opts);
    assert_eq!(asked.get().unwrap().width, 2100);
}

#[test]
fn print_quality_override_sets_every_perspective_box() {
    let mut l = one_page_layout();
    for (id, src) in [
        (1, BoxSource::Perspective { camera_id: 1 }),
        (2, BoxSource::text("x", 10.0)),
    ] {
        l.page_mut(1).unwrap().boxes.push(LayoutBox::new(
            id,
            (Point::new(1.0, 1.0), Point::new(3.0, 3.0)),
            src,
            Scale::QuarterInch,
        ));
    }
    let q = with_perspective_quality(&l, 300, 0);
    let boxes = &q.pages[0].boxes;
    assert_eq!((boxes[0].dpi, boxes[0].samples), (300, 0));
    assert_eq!((boxes[1].dpi, boxes[1].samples), (0, 0));
    assert_eq!(l.pages[0].boxes[0].dpi, 0, "the original is untouched");
}

// -------------------------------------------- sheet index, Daniel's sheet set --

#[test]
fn a_sheet_index_box_lists_every_printed_sheet_and_follows_renames() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = one_page_layout();
    l.add_page(2, "Floor Plan");
    l.add_page(3, "Template").template_page = true;
    l.page_mut(1).unwrap().boxes.push(LayoutBox::new(
        1,
        (Point::new(1.0, 10.0), Point::new(5.0, 14.0)),
        BoxSource::SheetIndex,
        Scale::QuarterInch,
    ));
    let t = text_of(&render_pdf(&l, &cx));
    for want in ["(SHEET INDEX)", "(A-1)", "(PAGE)", "(A-2)", "(FLOOR PLAN)"] {
        assert!(t.contains(want), "missing {want}");
    }
    assert!(!t.contains("(TEMPLATE)"), "template pages are not listed");
    l.page_mut(2).unwrap().title = "Roof Plan".into();
    assert!(text_of(&render_pdf(&l, &cx)).contains("(ROOF PLAN)"));
}

#[test]
fn daniels_sheet_set_has_cover_site_plans_elevations_sections_details_schedules() {
    let p = two_room_house();
    let l = default_construction_set(&p, 1);
    let titles: Vec<&str> = l.pages.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Cover",
            "Site Plan",
            "1st Floor Plan",
            "Elevations: Front and Back",
            "Elevations: Left and Right",
            "Building Section",
            "Details",
            "Schedules",
            "Materials List",
            "Framing Plan"
        ]
    );
    let cover = l.page(0).unwrap();
    assert!(cover
        .boxes
        .iter()
        .any(|b| b.source == BoxSource::SheetIndex));
    assert!(
        !l.sheet_index,
        "the index is a box now, not the page-1 table"
    );
    let site = l.page(1).unwrap();
    assert_eq!(site.boxes[0].label.as_deref(), Some("SITE PLAN"));
    assert!(site.boxes[0].scale.inches_per_foot() <= Scale::EighthInch.inches_per_foot());
    let cx = LayoutRenderContext::new(&p);
    let t = text_of(&render_pdf(&l, &cx));
    for want in [
        "(SHEET INDEX)",
        "(SITE PLAN)",
        "(DETAILS)",
        "(BUILDING SECTION)",
    ] {
        assert!(t.contains(want), "missing {want}");
    }
}

#[test]
fn appending_the_set_to_a_live_layout_drops_empty_pages_and_numbers_after_the_rest() {
    let p = two_room_house();
    let mut live = Layout::new("Live", SheetSize::ArchC);
    live.add_page(0, "Template").template_page = true;
    live.add_page(1, "Page 1"); // empty
    live.add_page(2, "My Notes")
        .add_text(Point::new(2.0, 2.0), "keep me", 0.2);
    let added = append_construction_set(&mut live, &p, 1, &plan_docs::MasterList::default());
    assert_eq!(added, 10);
    let titles: Vec<&str> = live.pages.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(titles[0], "Template");
    assert_eq!(titles[1], "My Notes");
    assert_eq!(titles[2], "Cover");
    assert!(!titles.contains(&"Page 1"));
    let numbers: Vec<u32> = live.pages.iter().map(|p| p.number).collect();
    let mut sorted = numbers.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        numbers.len(),
        "page numbers are unique: {numbers:?}"
    );
    // Box ids are unique across the layout.
    let mut ids: Vec<_> = live
        .pages
        .iter()
        .flat_map(|p| p.boxes.iter().map(|b| b.id))
        .collect();
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n);
    // The cover's index lists the user's page too.
    let cx = LayoutRenderContext::new(&p);
    assert!(text_of(&render_pdf(&live, &cx)).contains("(MY NOTES)"));
    // Boxes sit inside the live layout's own drawing area.
    let (lo, hi) = live.drawing_area();
    for b in live
        .pages
        .iter()
        .flat_map(|p| p.boxes.iter())
        .filter(|b| b.border)
    {
        let r = b.bounds_in();
        assert!(r[0] >= lo.x - 1e-6 && r[2] <= hi.x + 3.0, "{r:?}");
    }
}

#[test]
fn casing_drawn_in_plan_reaches_the_page() {
    let base = two_room_house();
    let (plain, _) = stroke_count(&base);
    let mut p = two_room_house();
    let id = p.floors[0]
        .openings
        .iter()
        .find(|o| o.kind == OpeningKind::Door)
        .unwrap()
        .id;
    p.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
        .unwrap()
        .extras
        .spec
        .casing_in_plan = true;
    let (cased, _) = stroke_count(&p);
    assert!(cased > plain, "{cased} vs {plain}");
}

