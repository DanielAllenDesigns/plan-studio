//! Layout boxes beyond plan views, layout CAD, printing.

use crate::tests::two_room_house;
use crate::*;
use plan_core::Point;
use plan_docs::{Scale, SheetSize};

fn text_of(pdf: &[u8]) -> String {
    pdf.iter().map(|&b| b as char).collect()
}

fn count(pdf: &[u8], needle: &str) -> usize {
    let n = needle.as_bytes();
    pdf.windows(n.len()).filter(|w| *w == n).count()
}

fn one_page(source: BoxSource, rect: ((f64, f64), (f64, f64))) -> Layout {
    let mut l = Layout::new("T", SheetSize::ArchC);
    l.page_background = false;
    l.add_page(1, "Plan");
    let mut b = LayoutBox::new(
        1,
        (
            Point::new(rect.0 .0, rect.0 .1),
            Point::new(rect.1 .0, rect.1 .1),
        ),
        source,
        Scale::QuarterInch,
    );
    b.border = false;
    l.page_mut(1).unwrap().boxes.push(b);
    l
}

#[test]
fn text_box_alignment_and_bold_reach_the_pdf() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let src = |align, bold| BoxSource::Text {
        text: "NOTE".into(),
        height_pt: 10.0,
        align,
        bold,
    };
    let at = |align, bold| {
        let l = one_page(src(align, bold), ((2.0, 2.0), (6.0, 3.0)));
        text_of(&render_pdf(&l, &cx))
    };
    let left = at(TextAlign::Left, false);
    let right = at(TextAlign::Right, true);
    // Left: 3 pt inside the box at x = 2" = 144 pt.
    assert!(left.contains("147 "), "{left}");
    // Right-aligned text ends 3 pt before 6" = 432 pt: starts at 429 - width.
    let w = plan_docs::PdfDoc::text_width_bold("NOTE", 10.0);
    let want = format!("/F2 10 Tf {} ", 429.0 - w);
    assert!(right.contains(&want[..want.len() - 4]), "{want} in {right}");
    // The text box JSON round trip keeps its style and reads the old form.
    let json = serde_json::to_string(&src(TextAlign::Center, true)).unwrap();
    let back: BoxSource = serde_json::from_str(&json).unwrap();
    assert_eq!(back, src(TextAlign::Center, true));
    let old: BoxSource = serde_json::from_str(r#"{"Text":{"text":"x","height_pt":9.0}}"#).unwrap();
    assert_eq!(old, BoxSource::text("x", 9.0));
}

#[test]
fn perspective_box_embeds_the_rendered_image() {
    let mut p = two_room_house();
    let id = p.add_camera(plan_core::CameraObject::new(
        plan_core::camera::CameraKind::FullCamera,
        Point::new(100.0, 100.0),
        0.0,
        "Living",
        0,
    ));
    let l = one_page(
        BoxSource::Perspective { camera_id: id },
        ((1.0, 1.0), (7.0, 5.5)),
    );
    let none = LayoutRenderContext::new(&p);
    assert_eq!(count(&render_pdf(&l, &none), "/Subtype/Image"), 0);
    let cx = LayoutRenderContext::new(&p).with_perspective_image(|_| {
        Some(PerspectiveImage {
            width: 8,
            height: 6,
            rgba: vec![200; 8 * 6 * 4],
        })
    });
    let pdf = render_pdf(&l, &cx);
    assert_eq!(count(&pdf, "/Subtype/Image"), 1);
    assert!(text_of(&pdf).contains("/Width 8 /Height 6"));
}

#[test]
fn layout_cad_line_box_and_text_render_on_the_page() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = Layout::new("T", SheetSize::ArchC);
    l.add_page(1, "Plan");
    let pg = l.page_mut(1).unwrap();
    let a = pg.add_line(Point::new(1.0, 2.0), Point::new(4.0, 2.0));
    let b = pg.add_rect(Point::new(5.0, 5.0), Point::new(7.0, 6.0));
    let c = pg.add_text(Point::new(1.0, 8.0), "SEE DETAIL 3", 0.2);
    assert_eq!([a, b, c], [1, 2, 3]);
    let t = text_of(&render_pdf(&l, &cx));
    assert!(t.contains("72 144 m 288 144 l S"), "line");
    assert!(
        t.contains("360 360 m 504 360 l 504 432 l 360 432 l s"),
        "box"
    );
    assert!(t.contains("(SEE DETAIL 3)"), "text");
}

#[test]
fn materials_box_draws_the_list_and_follows_the_plan() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let l = one_page(
        BoxSource::Materials {
            floor: None,
            category: Some("Doors".into()),
        },
        ((1.0, 1.0), (8.0, 4.0)),
    );
    let t = text_of(&render_pdf(&l, &cx));
    assert!(t.contains("(MATERIALS LIST - DOORS)"));
    assert!(t.contains("DR-001"));
    let (w, h) = source_size_in(
        &BoxSource::Materials {
            floor: None,
            category: None,
        },
        Scale::QuarterInch,
        &cx,
    );
    assert!(w > 4.0 && h > 2.0);
}

fn sheet_layout() -> Layout {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = default_construction_set(&p, 1);
    l.sheet = SheetSize::ArchD;
    let _ = cx;
    l
}

#[test]
fn a_24_by_36_sheet_tiled_on_letter_is_six_pages_with_overlap() {
    // 36 x 24 at 50% is 18 x 12; Letter portrait has 8 x 10.5 printable.
    let g = tile_grid((36.0, 24.0), 0.5, (8.0, 10.5), 0.5);
    assert_eq!((g.cols, g.rows, g.count()), (3, 2, 6));
    // Without the overlap the same sheet needs 3 x 2 as well, but at 100% the
    // overlap matters: 36 / 10.5 = 3.43 -> 4 columns, 24 / 8 = 3 rows.
    assert_eq!(
        tile_grid((36.0, 24.0), 1.0, (10.5, 8.0), 0.5).count(),
        4 * 4
    );
    assert_eq!(
        tile_grid((36.0, 24.0), 1.0, (10.5, 8.0), 0.0).count(),
        4 * 3
    );

    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let l = sheet_layout();
    let opts = PrintOptions {
        paper: PaperSize::Standard(SheetSize::Letter),
        landscape: false,
        scale: PrintScale::Percent(50.0),
        tiling: true,
        ..PrintOptions::default()
    };
    let pdf = print_layout_pdf(&l, &cx, &opts);
    let pages = l.content_pages().len();
    assert_eq!(count(&pdf, "/Type /Page "), 6 * pages);
    assert!(text_of(&pdf).contains("TILE 6 OF 6"));
    // Letter portrait media boxes.
    assert!(text_of(&pdf).contains("/MediaBox [0 0 612 792]"));
    // One bookmark per sheet, not per tile.
    assert_eq!(count(&pdf, "/Title ("), pages);
}

#[test]
fn print_honors_paper_scale_orientation_and_range() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let l = sheet_layout();
    let pages = l.content_pages().len();
    assert!(pages >= 3);
    let fit = print_layout_pdf(
        &l,
        &cx,
        &PrintOptions {
            paper: PaperSize::Standard(SheetSize::Tabloid),
            landscape: true,
            ..PrintOptions::default()
        },
    );
    let t = text_of(&fit);
    assert_eq!(count(&fit, "/Type /Page "), pages);
    assert!(t.contains("/MediaBox [0 0 1224 792]"));
    // Fit: 36 x 24 onto 16.5 x 10.5 printable is a 0.4375 scale.
    assert!(t.contains("0.4375 0 0 0.4375"));
    let ranged = print_layout_pdf(
        &l,
        &cx,
        &PrintOptions {
            range: Some((2, 3)),
            paper: PaperSize::Custom {
                width_in: 20.0,
                height_in: 30.0,
            },
            landscape: false,
            ..PrintOptions::default()
        },
    );
    assert_eq!(count(&ranged, "/Type /Page "), 2);
    assert!(text_of(&ranged).contains("/MediaBox [0 0 1440 2160]"));
    assert!(text_of(&ranged).contains("A-2 "));
}

#[test]
fn grayscale_and_black_and_white_print_without_rgb_operators() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let l = sheet_layout();
    let color = print_layout_pdf(&l, &cx, &PrintOptions::default());
    assert!(text_of(&color).contains(" rg\n") || text_of(&color).contains(" RG\n"));
    for mode in [PrintColor::Grayscale, PrintColor::BlackWhite] {
        let pdf = print_layout_pdf(
            &l,
            &cx,
            &PrintOptions {
                color: mode,
                ..PrintOptions::default()
            },
        );
        let t = text_of(&pdf);
        assert!(!t.contains(" rg") && !t.contains(" RG"), "{mode:?}");
    }
}

#[test]
fn line_weights_off_prints_hairlines() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let l = sheet_layout();
    let opts = |w| PrintOptions {
        scale: PrintScale::Actual,
        paper: PaperSize::Standard(SheetSize::ArchD),
        landscape: true,
        margin_in: 0.0,
        line_weights: w,
        ..PrintOptions::default()
    };
    let widths = |pdf: &[u8]| {
        let t = text_of(pdf);
        let mut set = std::collections::BTreeSet::new();
        for part in t.split(" w ") {
            if let Some(tok) = part.rsplit(' ').next() {
                if let Ok(v) = tok.parse::<f64>() {
                    set.insert((v * 100.0) as i64);
                }
            }
        }
        set
    };
    let on = widths(&print_layout_pdf(&l, &cx, &opts(true)));
    let off = widths(&print_layout_pdf(&l, &cx, &opts(false)));
    assert!(on.len() > 2, "{on:?}");
    assert_eq!(off, [50].into_iter().collect(), "{off:?}");
}

#[test]
fn a_plan_view_prints_at_a_drawing_scale_and_tiles() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let opts = PrintOptions {
        paper: PaperSize::Standard(SheetSize::Letter),
        landscape: true,
        scale: PrintScale::Drawing(Scale::QuarterInch),
        ..PrintOptions::default()
    };
    let (pdf, scale) = print_plan_view_pdf(&cx, 0, "All", "FIRST FLOOR PLAN", &opts);
    assert_eq!(scale, Scale::QuarterInch);
    // 40' x 30' plus margins at 1/4" is about 22 x 20 inches: one page, clipped.
    assert_eq!(count(&pdf, "/Type /Page "), 1);
    assert!(text_of(&pdf).contains("SCALE: 1/4"));
    let tiled = PrintOptions {
        tiling: true,
        ..opts.clone()
    };
    let (pdf, _) = print_plan_view_pdf(&cx, 0, "All", "FIRST FLOOR PLAN", &tiled);
    assert!(count(&pdf, "/Type /Page ") >= 4);
    // Fit picks a scale that fits Letter on one page.
    let fit = PrintOptions {
        scale: PrintScale::Fit,
        ..opts
    };
    let (pdf, s) = print_plan_view_pdf(&cx, 0, "All", "FIRST FLOOR PLAN", &fit);
    assert_eq!(count(&pdf, "/Type /Page "), 1);
    assert!(s.inches_per_foot() < 0.25, "{s:?}");
    // A custom ratio.
    let ratio = PrintOptions {
        scale: PrintScale::Ratio(50.0),
        ..fit
    };
    let (_, s) = print_plan_view_pdf(&cx, 0, "All", "x", &ratio);
    assert_eq!(s, Scale::Ratio(50));
}

#[test]
fn print_image_rasterises_the_plan_lines() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let (w, h, rgba) = plan_view_image(&cx, 0, "All", Scale::EighthInch, 400);
    assert_eq!(w, 400);
    assert!(h > 200 && h < 600);
    assert_eq!(rgba.len(), (w * h * 4) as usize);
    let dark = rgba.chunks(4).filter(|px| px[0] < 128).count();
    assert!(dark > 200, "{dark}");
    assert!(rgba.chunks(4).filter(|px| px[0] == 255).count() > dark);
}

#[test]
fn box_artwork_carries_texts_and_images_turned_like_the_page() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p).with_perspective_image(|_| {
        Some(PerspectiveImage {
            width: 4,
            height: 2,
            rgba: vec![255; 4 * 2 * 4],
        })
    });
    let mut l = one_page(
        BoxSource::Text {
            text: "HELLO".into(),
            height_pt: 12.0,
            align: TextAlign::Center,
            bold: true,
        },
        ((2.0, 2.0), (6.0, 3.0)),
    );
    let b = &mut l.page_mut(1).unwrap().boxes[0];
    let a = render_box_artwork(b, &cx);
    assert_eq!(a.texts.len(), 1);
    assert!(a.texts[0].bold && a.texts[0].angle == 0.0);
    // Centred: the text starts left of the middle (4") by half its width.
    let w = plan_docs::PdfDoc::text_width_bold("HELLO", 12.0) / 72.0;
    assert!(
        (a.texts[0].x - (4.0 - w / 2.0)).abs() < 1e-6,
        "{}",
        a.texts[0].x
    );
    // Turned a quarter, the text runs up the page.
    b.rotation_deg = 90.0;
    b.clip = false;
    let a = render_box_artwork(b, &cx);
    assert!((a.texts[0].angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    // A perspective box yields an image, turned with the box.
    let mut l = one_page(
        BoxSource::Perspective { camera_id: 1 },
        ((1.0, 1.0), (5.0, 3.0)),
    );
    let b = &mut l.page_mut(1).unwrap().boxes[0];
    let a = render_box_artwork(b, &cx);
    assert_eq!((a.images[0].width, a.images[0].height), (4, 2));
    b.rotation_deg = 90.0;
    b.clip = false;
    let a = render_box_artwork(b, &cx);
    assert_eq!((a.images[0].width, a.images[0].height), (2, 4));
}

#[test]
fn image_boxes_draw_the_picture_the_loader_reads() {
    let p = two_room_house();
    let l = one_page(
        BoxSource::Image {
            path: "plans/site.png".into(),
        },
        ((1.0, 1.0), (5.0, 4.0)),
    );
    // Without a loader (or when it fails) the box is a framed placeholder.
    let plain = LayoutRenderContext::new(&p);
    assert_eq!(count(&render_pdf(&l, &plain), "/Subtype/Image"), 0);
    assert!(text_of(&render_pdf(&l, &plain)).contains("IMAGE: plans/site.png"));
    let cx = LayoutRenderContext::new(&p).with_picture_loader(|path| {
        assert_eq!(path, "plans/site.png");
        Some(PerspectiveImage {
            width: 6,
            height: 3,
            rgba: vec![90; 6 * 3 * 4],
        })
    });
    let pdf = render_pdf(&l, &cx);
    assert_eq!(count(&pdf, "/Subtype/Image"), 1);
    assert!(text_of(&pdf).contains("/Width 6 /Height 3"));
    // Sent to a layout it takes the picture's proportions (4" wide, 2" tall).
    let (w, h) = source_size_in(
        &BoxSource::Image {
            path: "plans/site.png".into(),
        },
        Scale::QuarterInch,
        &cx,
    );
    assert_eq!((w, h), (4.0, 2.0));
}
