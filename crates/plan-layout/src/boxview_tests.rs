//! Layout boxes and Send to Layout (Round 16, brief 02): site-plan scales,
//! the too-big warning, Box Scale modes, pan and crop geometry, the box label
//! and Plot Lines on a synthetic box.

use super::*;
use crate::canvas::Prim;
use crate::tests::two_room_house;
use plan_docs::{Scale, SheetSize};
use plan_elevation::ViewDir;

fn plan() -> BoxSource {
    BoxSource::PlanView {
        floor: 0,
        layer_set: "Floor Plan".into(),
    }
}

fn layout() -> Layout {
    Layout::new("L", SheetSize::ArchD)
}

fn request(page: u32, source: BoxSource, scale: SendScale) -> SendRequest {
    SendRequest {
        page,
        source,
        scale,
        at: None,
        centre: None,
        options: SendOptions::default(),
    }
}

fn the_box(l: &Layout, id: plan_core::Id) -> &LayoutBox {
    l.pages
        .iter()
        .flat_map(|p| &p.boxes)
        .find(|b| b.id == id)
        .unwrap()
}

#[test]
fn one_inch_to_a_hundred_feet_is_exactly_one_to_twelve_hundred() {
    assert_eq!(Scale::OneInchEq100Ft.label(), "1\" = 100'-0\"");
    assert_eq!(model_per_paper(Scale::OneInchEq100Ft), 1200.0);
    assert_eq!(model_per_paper(Scale::Ratio(1200)), 1200.0);
    assert_eq!(paper_per_model(Scale::OneInchEq100Ft), 1.0 / 1200.0);
    // The same factor from every way of typing it.
    for text in [
        "1:1200",
        "1200",
        "1\" = 100'",
        "1 in = 100 ft",
        "1 m = 1200 m",
    ] {
        let ipf = parse_scale_text(text).unwrap_or_else(|| panic!("{text}"));
        assert!((ipf - 0.01).abs() < 1e-12, "{text}: {ipf}");
    }
    // The architectural ones keep their meaning.
    assert_eq!(parse_scale_text("1/4\" = 1'"), Some(0.25));
    assert_eq!(parse_scale_text("3/16 in = 1 ft"), Some(0.1875));
    assert_eq!(parse_scale_text("1:50"), Some(12.0 / 50.0));
    // Nonsense is refused.
    for bad in ["", "abc", "0", "-5", "1:0", "1\" = 0'"] {
        assert_eq!(parse_scale_text(bad), None, "{bad:?}");
    }
    // A factor on a list gives that list entry; a whole ratio gives 1:n; any
    // other keeps its exact factor.
    assert_eq!(
        scale_for_ipf(0.01),
        (Scale::OneInchEq100Ft, ScaleMode::Named)
    );
    assert_eq!(scale_for_ipf(0.25), (Scale::QuarterInch, ScaleMode::Named));
    assert_eq!(
        scale_for_ipf(12.0 / 37.0),
        (Scale::Ratio(37), ScaleMode::Named)
    );
    let (s, mode) = scale_for_ipf(0.1234);
    assert!(matches!(mode, ScaleMode::Custom(v) if (v - 0.1234).abs() < 1e-12));
    assert_eq!(s, nominal_scale(0.1234));
    assert_eq!(custom_label(12.0 / 37.0), "1:37");
}

#[test]
fn a_big_view_is_flagged_too_large_until_the_scale_comes_down() {
    let p = two_room_house(); // 40' x 30'
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    l.add_page(1, "Plans");
    // At 3" = 1' the plan is 120 x 90 inches: far too big for a D sheet.
    let (fit, _) = check_send(
        &l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::ThreeInch)),
    );
    assert!(fit.too_large);
    assert!(fit.size_in.0 > fit.area_in.0);
    let sent = send_view(
        &mut l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::ThreeInch)),
    );
    assert!(sent.too_large);
    assert!(sent.warning().unwrap().contains("too big for the sheet"));
    // The box is placed anyway, at the scale asked for.
    assert_eq!(the_box(&l, sent.id).scale, Scale::ThreeInch);
    // 1/4" fits, and so does a site scale.
    let (fit, _) = check_send(
        &l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::QuarterInch)),
    );
    assert!(!fit.too_large);
    let sent = send_view(
        &mut l,
        &cx,
        &request(2, plan(), SendScale::Named(Scale::OneInchEq100Ft)),
    );
    assert!(!sent.too_large && sent.warning().is_none());
    // At 1 in = 100 ft the 40' x 30' plan (walls and margins: about 534 x
    // 414 inches of building) is a box of about 0.45 x 0.35 paper inches,
    // exactly 1/1200 of the building.
    let (w, h) = sent.size_in;
    let frame = view_frame_in(
        &the_box(&l, sent.id).source,
        &the_box(&l, sent.id).view,
        &cx,
    )
    .unwrap();
    assert!((w - frame.0 / 1200.0).abs() < 1e-9, "{w}");
    assert!((h - frame.1 / 1200.0).abs() < 1e-9, "{h}");
    assert!(
        (w - 0.445).abs() < 0.01 && (h - 0.345).abs() < 0.01,
        "{w} x {h}"
    );
    // Largest that fits never goes past the ceiling it is given.
    let sent = send_view(
        &mut l,
        &cx,
        &request(3, plan(), SendScale::Largest(Scale::QuarterInch)),
    );
    assert_eq!(the_box(&l, sent.id).scale, Scale::QuarterInch);
    assert!(!sent.too_large);
    let sent = send_view(
        &mut l,
        &cx,
        &request(4, plan(), SendScale::Largest(Scale::ThreeInch)),
    );
    assert!(!sent.too_large);
    assert!(paper_per_model(the_box(&l, sent.id).scale) > paper_per_model(Scale::QuarterInch));
}

#[test]
fn site_scales_follow_the_architectural_ones_when_stepping_down() {
    let all = fit_scales();
    assert_eq!(all.first(), Some(&Scale::ThreeInch));
    assert_eq!(all.last(), Some(&Scale::OneInchEq100Ft));
    assert_eq!(
        next_smaller(Scale::OneInchEq20Ft),
        Some(Scale::OneInchEq30Ft)
    );
    assert_eq!(
        next_smaller(Scale::OneInchEq60Ft),
        Some(Scale::OneInchEq100Ft)
    );
    assert_eq!(next_smaller(Scale::OneInchEq100Ft), None);
    // A frame nothing on the lists fits down to the end gets the smallest.
    let l = layout();
    let huge = (12.0 * 5000.0, 12.0 * 5000.0);
    assert_eq!(
        largest_scale_that_fits(&l, None, huge, Scale::QuarterInch, true),
        Scale::OneInchEq100Ft
    );
}

#[test]
fn a_typed_ratio_and_fit_to_sheet_send_boxes_at_their_own_scales() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    let ipf = parse_scale_text("1:37").unwrap();
    let sent = send_view(&mut l, &cx, &request(1, plan(), SendScale::PerFoot(ipf)));
    let b = the_box(&l, sent.id);
    assert_eq!(b.scale, Scale::Ratio(37));
    assert_eq!(b.view.scale_mode, ScaleMode::Named);
    assert_eq!(b.scale_note(), "1:37");
    // 1:240 is 1 in = 20 ft, which is on the lists already.
    let ipf = parse_scale_text("1:240").unwrap();
    let sent = send_view(&mut l, &cx, &request(1, plan(), SendScale::PerFoot(ipf)));
    assert_eq!(the_box(&l, sent.id).scale, Scale::OneInchEq20Ft);
    // Fit to Sheet: about half the drawing area, and no scale.
    let sent = send_view(&mut l, &cx, &request(2, plan(), SendScale::FitToSheet));
    let b = the_box(&l, sent.id);
    assert!(b.is_no_scale());
    assert_eq!(b.scale_note(), "NOT TO SCALE");
    let (lo, hi) = l.drawing_area();
    let (aw, ah) = (hi.x - lo.x, hi.y - lo.y);
    let (w, h) = b.size_in();
    assert!(w <= aw * 0.5 + 1e-6 && h <= ah * 0.5 + 1e-6, "{w} x {h}");
    assert!(w > aw * 0.3 || h > ah * 0.3, "about half: {w} x {h}");
}

#[test]
fn send_options_set_the_extent_the_saved_view_and_the_camera_link() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    // Current Screen: only the part of the view that was on screen.
    let mut req = request(1, plan(), SendScale::Named(Scale::QuarterInch));
    req.options.extent = SendExtent::CurrentScreen([0.0, 0.0, 240.0, 120.0]);
    let sent = send_view(&mut l, &cx, &req);
    let (w, h) = sent.size_in;
    assert!(
        (w - 240.0 / 48.0).abs() < 1e-9 && (h - 120.0 / 48.0).abs() < 1e-9,
        "{w} x {h}"
    );
    assert_eq!(
        the_box(&l, sent.id).view.extent,
        Some([0.0, 0.0, 240.0, 120.0])
    );
    // Entire Plan/View: Fill Window.
    let mut req = request(1, plan(), SendScale::Named(Scale::QuarterInch));
    req.options.extent = SendExtent::EntireView;
    let sent = send_view(&mut l, &cx, &req);
    assert!(the_box(&l, sent.id).view.fill_window);
    // Link Saved Plan View.
    let mut req = request(1, plan(), SendScale::Named(Scale::QuarterInch));
    req.options.link_saved_view = Some("Main".into());
    let sent = send_view(&mut l, &cx, &req);
    assert_eq!(
        the_box(&l, sent.id).view.saved_view.as_deref(),
        Some("Main")
    );
    // A section as Plot Lines keeps its lines; Live View, Always Update keeps
    // none; Update on Demand keeps a picture too.
    let section = BoxSource::Elevation {
        dir: ViewDir::Front,
    };
    for (link, kind, art) in [
        (CameraLink::PlotLines, UpdateKind::PlotLine, true),
        (CameraLink::OnDemand, UpdateKind::SemiDynamic, true),
        (CameraLink::Always, UpdateKind::Dynamic, false),
    ] {
        let mut req = request(2, section.clone(), SendScale::Named(Scale::EighthInch));
        req.options.camera = link;
        let sent = send_view(&mut l, &cx, &req);
        let b = the_box(&l, sent.id);
        assert_eq!(b.update_kind(), kind, "{link:?}");
        assert_eq!(b.view.art.is_some(), art, "{link:?}");
    }
}

#[test]
fn box_scale_modes_resize_the_box_or_leave_it() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    let sent = send_view(
        &mut l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::EighthInch)),
    );
    let mut b = the_box(&l, sent.id).clone();
    let (w0, h0) = b.size_in();
    let centre = (
        (b.bounds_in()[0] + b.bounds_in()[2]) / 2.0,
        (b.bounds_in()[1] + b.bounds_in()[3]) / 2.0,
    );
    // Scale Layout Box Contents Only (the default): the box follows the scale.
    assert!(rescale(&mut b, NewScale::Named(Scale::QuarterInch)));
    let (w1, h1) = b.size_in();
    assert!((w1 / w0 - 2.0).abs() < 1e-9 && (h1 / h0 - 2.0).abs() < 1e-9);
    let r = b.bounds_in();
    assert!(
        ((r[0] + r[2]) / 2.0 - centre.0).abs() < 1e-9,
        "resized about its centre"
    );
    assert!(
        !rescale(&mut b, NewScale::Named(Scale::QuarterInch)),
        "no change"
    );
    // Unchecked: the box keeps its size and the contents are cropped or
    // surrounded by space.
    b.view.resize_with_scale = false;
    let size = b.size_in();
    assert!(rescale(&mut b, NewScale::Named(Scale::OneInchEq100Ft)));
    assert_eq!(b.size_in(), size);
    assert_eq!(b.scale, Scale::OneInchEq100Ft);
    // A typed scale on no list keeps its exact factor.
    assert!(rescale(&mut b, NewScale::PerFoot(0.1234)));
    assert!((b.effective_ipf() - 0.1234).abs() < 1e-12);
    assert_eq!(b.scale_note(), "0.1234\" = 1'-0\"");
    // No Scale keeps the size and factor on the sheet but drops the scale.
    let was = b.effective_ipf();
    assert!(rescale(&mut b, NewScale::NoScale));
    assert!(b.is_no_scale());
    assert!((b.effective_ipf() - was).abs() < 1e-12);
    assert_eq!(b.scale_note(), "NOT TO SCALE");
}

#[test]
fn resizing_a_non_scaled_box_by_its_corner_resizes_the_view() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    let sent = send_view(&mut l, &cx, &request(1, plan(), SendScale::FitToSheet));
    let mut b = the_box(&l, sent.id).clone();
    let (w0, _) = b.size_in();
    let ipf0 = b.effective_ipf();
    let r = b.bounds_in();
    // The corner handle (Alternate behavior): the view grows with the border.
    resize_no_scale(
        &mut b,
        [
            r[0],
            r[1],
            r[0] + (r[2] - r[0]) * 1.5,
            r[1] + (r[3] - r[1]) * 1.5,
        ],
    );
    assert!((b.size_in().0 / w0 - 1.5).abs() < 1e-9);
    assert!((b.effective_ipf() / ipf0 - 1.5).abs() < 1e-9);
    // A scaled box keeps its scale when the border moves (crops).
    let sent = send_view(
        &mut l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::EighthInch)),
    );
    let mut s = the_box(&l, sent.id).clone();
    let r = s.bounds_in();
    resize_no_scale(&mut s, [r[0], r[1], r[0] + 1.0, r[1] + 1.0]);
    assert_eq!(s.scale, Scale::EighthInch);
    assert_eq!(s.effective_ipf(), 0.125);
}

#[test]
fn panning_crops_and_scale_to_fit_fills_the_box() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    let sent = send_view(
        &mut l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::EighthInch)),
    );
    let mut b = the_box(&l, sent.id).clone();
    let frame = view_frame_in(&b.source, &b.view, &cx).unwrap();
    // The content map sends the view's corners to the box.
    let map = box_content_map(&b, &cx).unwrap();
    let before = map.to_paper(plan_core::Point::new(0.0, 0.0));
    // Panning by a drag of (1, -0.5) paper inches moves the contents by it.
    pan_by(&mut b, (1.0, -0.5));
    let after = box_content_map(&b, &cx)
        .unwrap()
        .to_paper(plan_core::Point::new(0.0, 0.0));
    assert!((after.x - before.x - 1.0).abs() < 1e-9 && (after.y - before.y + 0.5).abs() < 1e-9);
    // And back from paper to the view.
    let back = box_content_map(&b, &cx).unwrap().to_source(after);
    assert!(back.dist(plan_core::Point::new(0.0, 0.0)) < 1e-6);
    // Recenter puts the middle of the view at the middle of the box.
    recenter(&mut b, frame);
    let map = box_content_map(&b, &cx).unwrap();
    let mid = map.to_paper(plan_core::Point::new(240.0, 180.0));
    let r = b.bounds_in();
    assert!(
        (mid.x - (r[0] + r[2]) / 2.0).abs() < 1e-6,
        "{mid:?} in {r:?}"
    );
    assert!((mid.y - (r[1] + r[3]) / 2.0).abs() < 1e-6);
    // Make the box much bigger than its view and fit the view to it.
    b.rect_in = (
        plan_core::Point::new(1.0, 1.0),
        plan_core::Point::new(11.0, 7.0),
    );
    let ipf = scale_box_to_fit(&mut b, frame);
    assert!((b.effective_ipf() - ipf).abs() < 1e-12);
    // The whole view (with its margins) is inside, touching on one axis.
    let map = box_content_map(&b, &cx).unwrap();
    let (cw, ch) = (map.content.0 / 72.0, map.content.1 / 72.0);
    assert!(cw <= 10.0 + 1e-6 && ch <= 6.0 + 1e-6, "{cw} x {ch}");
    assert!(
        (cw - 10.0).abs() < 1e-6 || (ch - 6.0).abs() < 1e-6,
        "{cw} x {ch}"
    );
    // A quarter turn pans along the turned axes.
    let mut t = the_box(&l, sent.id).clone();
    t.rotation_deg = 90.0;
    pan_by(&mut t, (1.0, 0.0));
    assert_eq!(t.view.pan_in, (0.0, -1.0));
}

#[test]
fn a_panned_box_draws_its_lines_where_the_pan_put_them() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    let sent = send_view(
        &mut l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::EighthInch)),
    );
    let mut b = the_box(&l, sent.id).clone();
    b.border = false;
    b.clip = false;
    let first = |b: &LayoutBox| {
        let lines = render_box_lines(b, &cx);
        let x = lines
            .iter()
            .map(|l| l.a.x.min(l.b.x))
            .fold(f64::MAX, f64::min);
        let y = lines
            .iter()
            .map(|l| l.a.y.min(l.b.y))
            .fold(f64::MAX, f64::min);
        (x, y, lines.len())
    };
    let (x0, y0, n0) = first(&b);
    pan_by(&mut b, (0.5, 0.25));
    let (x1, y1, n1) = first(&b);
    assert_eq!(n0, n1);
    assert!((x1 - x0 - 0.5).abs() < 1e-6 && (y1 - y0 - 0.25).abs() < 1e-6);
}

#[test]
fn a_current_screen_box_shows_only_its_part_and_a_saved_view_picks_the_floor() {
    let mut p = two_room_house();
    let mut l = layout();
    let b = {
        let cx = LayoutRenderContext::new(&p);
        let mut req = request(1, plan(), SendScale::Named(Scale::QuarterInch));
        req.options.extent = SendExtent::CurrentScreen([0.0, 0.0, 120.0, 120.0]);
        let sent = send_view(&mut l, &cx, &req);
        the_box(&l, sent.id).clone()
    };
    let cx = LayoutRenderContext::new(&p);
    let b = &b;
    let map = box_content_map(b, &cx).unwrap();
    let r = b.bounds_in();
    let lo = map.to_paper(plan_core::Point::new(0.0, 0.0));
    let hi = map.to_paper(plan_core::Point::new(120.0, 120.0));
    assert!((lo.x - r[0]).abs() < 1e-9 && (hi.x - r[2]).abs() < 1e-9);
    // A missing floor is a missing view; a saved plan view that exists links.
    let mut gone = b.clone();
    gone.source = BoxSource::PlanView {
        floor: 5,
        layer_set: "Floor Plan".into(),
    };
    assert_eq!(
        missing_view(&gone, &p).map(|m| m.text()),
        Some("Missing layout view: floor 6".to_string())
    );
    let mut linked = b.clone();
    linked.view.saved_view = Some("Nope".into());
    assert!(matches!(missing_view(&linked, &p), Some(Missing::View(_))));
    drop(cx);
    p.plan_views.push(plan_core::layer_sets::SavedPlanView::new(
        "Main",
        "Floor Plan",
    ));
    assert!(
        missing_view(&linked, &p).is_some(),
        "Nope is still not there"
    );
    linked.view.saved_view = Some("Main".into());
    // The saved view's layer set is not in the plan: that is what is missing.
    assert!(matches!(
        missing_view(&linked, &p),
        Some(Missing::LayerSet(_))
    ));
    p.layer_sets
        .sets
        .push(plan_core::layer_sets::LayerSetDef::new("Floor Plan"));
    assert_eq!(missing_view(&linked, &p), None);
}

#[test]
fn a_missing_view_gets_a_caution_symbol_unless_ignored() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    l.add_page(1, "P");
    let sent = send_view(
        &mut l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::EighthInch)),
    );
    let strokes = |l: &Layout| {
        let b = &l.pages[0].boxes[0];
        render_box_artwork(b, &LayoutRenderContext::new(&p))
            .lines
            .len()
    };
    let ok = strokes(&l);
    l.pages[0].boxes[0].source = BoxSource::PlanView {
        floor: 9,
        layer_set: "Floor Plan".into(),
    };
    let broken = strokes(&l);
    // Nothing of the plan is drawn, but the caution triangle is.
    assert!(broken < ok);
    assert!(broken >= 3, "the triangle");
    l.pages[0].boxes[0].view.ignore_missing = true;
    assert!(
        strokes(&l) < broken,
        "Ignore Invalid Links leaves the box unchanged"
    );
    let _ = sent;
}

#[test]
fn plot_lines_draw_from_the_kept_picture_and_color_fill_reaches_the_pdf() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    let section = BoxSource::Elevation {
        dir: ViewDir::Front,
    };
    let mut req = request(1, section, SendScale::Named(Scale::EighthInch));
    req.options.camera = CameraLink::PlotLines;
    let sent = send_view(&mut l, &cx, &req);
    let live = {
        let mut b = the_box(&l, sent.id).clone();
        b.view.camera = CameraLink::Always;
        b.view.art = None;
        b
    };
    let plot = the_box(&l, sent.id).clone();
    // Lines of the plot picture and of the live view agree in number (the
    // picture is the live drawing kept).
    let (n_plot, n_live) = (
        render_box_lines(&plot, &cx).len(),
        render_box_lines(&live, &cx).len(),
    );
    assert!(n_plot > 4);
    assert_eq!(n_plot, n_live);
    // Editing the kept lines changes only the plot box.
    let art = plot.view.art.clone().unwrap();
    let (edges, patterns) = art.counts();
    assert!(edges > 0);
    let mut edited = plot.clone();
    let victims: Vec<_> = art.lines.iter().take(3).map(|l| l.id).collect();
    edited.view.art.as_mut().unwrap().delete_lines(&victims);
    assert_eq!(render_box_lines(&edited, &cx).len(), n_plot - 3);
    assert_eq!(render_box_lines(&live, &cx).len(), n_live);
    let _ = patterns;
    // Color Fill puts material colors under the lines; off, only grays.
    let rgb_fills = |b: &LayoutBox| {
        let scenes = crate::extent::SceneSource::for_context(&cx);
        crate::render::box_prims_for_test(b, &cx, &scenes)
            .into_iter()
            .filter(|p| {
                matches!(
                    p,
                    Prim::Fill {
                        color: plan_docs::PdfColor::Rgb(..),
                        ..
                    }
                )
            })
            .count()
    };
    let mut colored = plot.clone();
    let off = rgb_fills(&colored);
    colored.view.plot.color_fill = true;
    assert!(rgb_fills(&colored) > off);
    // The PDF of the page carries it too.
    l.pages[0].boxes[0].view.plot.color_fill = true;
    let pdf = render_pdf(&l, &cx);
    assert!(pdf.len() > 400);
}

#[test]
fn edge_and_pattern_defaults_change_every_line_of_their_kind() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    let mut req = request(
        1,
        BoxSource::Elevation {
            dir: ViewDir::Front,
        },
        SendScale::Named(Scale::EighthInch),
    );
    req.options.camera = CameraLink::PlotLines;
    let sent = send_view(&mut l, &cx, &req);
    let mut b = the_box(&l, sent.id).clone();
    b.border = false;
    b.clip = false;
    let widths = |b: &LayoutBox| {
        let scenes = crate::extent::SceneSource::for_context(&cx);
        let mut w: Vec<u32> = crate::render::box_prims_for_test(b, &cx, &scenes)
            .into_iter()
            .filter_map(|p| match p {
                Prim::Stroke { pen, .. } => Some((pen.width * 100.0).round() as u32),
                _ => None,
            })
            .collect();
        w.sort_unstable();
        w.dedup();
        w
    };
    let natural = widths(&b);
    assert!(natural.len() > 1, "weight classes: {natural:?}");
    b.view.plot.use_edge_defaults = true;
    b.view.plot.edge_weight_pt = 0.5;
    b.view.plot.use_pattern_defaults = true;
    b.view.plot.pattern_weight_pt = 0.5;
    assert_eq!(widths(&b), vec![50]);
    // A line of its own weight beats the defaults.
    let id = b.view.art.as_ref().unwrap().lines[0].id;
    b.view.art.as_mut().unwrap().set_spec(
        &[id],
        &LineSpec {
            weight_pt: Some(Some(1.5)),
            ..LineSpec::default()
        },
    );
    assert_eq!(widths(&b), vec![50, 150]);
}

#[test]
fn use_layout_line_scaling_keeps_weights_and_off_scales_them_with_the_view() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    let mut width_of = |scale: Scale, layout_scaling: bool| {
        let sent = send_view(&mut l, &cx, &request(1, plan(), SendScale::Named(scale)));
        let mut b = the_box(&l, sent.id).clone();
        b.view.layout_line_scaling = layout_scaling;
        b.border = false;
        let scenes = crate::extent::SceneSource::for_context(&cx);
        crate::render::box_prims_for_test(&b, &cx, &scenes)
            .into_iter()
            .filter_map(|p| match p {
                Prim::Stroke { pen, .. } => Some(pen.width),
                _ => None,
            })
            .fold(0.0, f64::max)
    };
    let (a, b) = (
        width_of(Scale::QuarterInch, true),
        width_of(Scale::EighthInch, true),
    );
    assert!(
        (a - b).abs() < 1e-9,
        "layout line scaling: same weights on the page"
    );
    let (a, b) = (
        width_of(Scale::QuarterInch, false),
        width_of(Scale::EighthInch, false),
    );
    assert!(
        (a / b - 2.0).abs() < 1e-6,
        "off: weights follow the scale ({a} vs {b})"
    );
}

#[test]
fn the_label_follows_its_position_shape_macros_and_layer() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    let sent = send_view(
        &mut l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::QuarterInch)),
    );
    let mut b = the_box(&l, sent.id).clone();
    let layers = LayoutLayers::default();
    let scenes = crate::extent::SceneSource::for_context(&cx);
    let texts = |b: &LayoutBox, layers: &LayoutLayers| -> Vec<(String, f64)> {
        let mut cv = crate::canvas::Canvas::new();
        let _ = &scenes;
        cv.prims
            .extend(crate::render::label_prims_for_test(b, &cx, layers));
        cv.prims
            .into_iter()
            .filter_map(|p| match p {
                Prim::Text { text, y, .. } => Some((text, y)),
                _ => None,
            })
            .collect()
    };
    let r = b.bounds_in();
    // Bottom left, as before: the caption, then the scale note under it.
    let t = texts(&b, &layers);
    assert_eq!(t[0].0, b.label.clone().unwrap());
    assert!((t[0].1 - (r[1] * 72.0 - 13.0)).abs() < 1e-9);
    assert_eq!(t[1].0, "SCALE: 1/4\" = 1'-0\"");
    // Top: above the box.
    b.view.label.position = LabelPos::TopCenter;
    let t = texts(&b, &layers);
    assert!(t[0].1 > r[3] * 72.0);
    // Macros: the scale, the view name and the page it is on.
    b.label = Some("%view_name% at %scale%".into());
    let t = texts(&b, &layers);
    assert_eq!(t[0].0, format!("{} at 1/4\" = 1'-0\"", plan_label(&p, 0)));
    // A callout shape adds the outline and its text.
    b.view.label.shape = plan_core::callout::CalloutShape::Circle;
    b.view.label.callout_text = "5".into();
    let mut cv = crate::canvas::Canvas::new();
    cv.prims
        .extend(crate::render::label_prims_for_test(&b, &cx, &layers));
    assert!(cv
        .prims
        .iter()
        .any(|p| matches!(p, Prim::Stroke { closed: true, pts, .. } if pts.len() > 8)));
    assert!(texts(&b, &layers).iter().any(|(t, _)| t == "5"));
    // The Layout Box Labels layer hides it all.
    let mut hidden = layers.clone();
    hidden.set_visible(LAYER_BOX_LABELS, false);
    assert!(texts(&b, &hidden).is_empty());
}

#[test]
fn a_label_linked_to_a_layout_page_reports_that_pages_label() {
    let mut p = two_room_house();
    let mut l = layout();
    l.add_page(1, "Plans");
    l.add_page(2, "Sections");
    l.page_mut(2).unwrap().label = "A5.#".into();
    // The plan keeps its layout as JSON, which the callout lookup reads.
    p.layout = serde_json::to_value(&l).ok();
    let cx = LayoutRenderContext::new(&p);
    cx.set_sheet_index(&l);
    let sent = send_view(
        &mut l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::QuarterInch)),
    );
    let mut b = the_box(&l, sent.id).clone();
    b.label = Some("SEE %linked_view_layout_page_label%".into());
    b.view.label.link = Some(plan_core::callout::ViewLink {
        kind: plan_core::callout::ViewKind::LayoutPage,
        id: 2,
        name: String::new(),
    });
    let text = crate::render::expand_label(b.label.as_deref().unwrap(), &b, &cx);
    assert_eq!(
        text,
        format!("SEE {}", l.sheet_number_of(l.page(2).unwrap()))
    );
    assert!(text.contains("A5."), "{text}");
    // A link to a page that is gone reports nothing.
    b.view.label.link.as_mut().unwrap().id = 99;
    assert_eq!(
        crate::render::expand_label(b.label.as_deref().unwrap(), &b, &cx),
        "SEE "
    );
}

#[test]
fn the_box_border_and_fill_follow_the_box_and_the_borders_layer() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    let sent = send_view(
        &mut l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::QuarterInch)),
    );
    let mut b = the_box(&l, sent.id).clone();
    let scenes = crate::extent::SceneSource::for_context(&cx);
    let prims =
        |b: &LayoutBox, layers: &LayoutLayers| crate::render::box_prims(b, &cx, &scenes, layers);
    let layers = LayoutLayers::default();
    let [x0, y0, x1, y1] = b.bounds_in().map(|v| v * 72.0);
    let rect_pts = vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    let border = |ps: &[Prim]| {
        ps.iter().rev().find_map(|p| match p {
            Prim::Stroke {
                closed: true,
                pen,
                pts,
            } if *pts == rect_pts => Some(*pen),
            _ => None,
        })
    };
    let base = border(&prims(&b, &layers)).unwrap();
    assert_eq!(base.width, 0.75);
    // This box's own border line style.
    b.view.border = BorderStyle {
        color: Some([200, 0, 0]),
        weight_pt: Some(2.0),
        dash: Some(plan_core::LineStyle::Dashed),
    };
    let own = border(&prims(&b, &layers)).unwrap();
    assert_eq!(own.width, 2.0);
    assert_eq!(own.color, plan_docs::PdfColor::Rgb(200, 0, 0));
    assert_eq!(own.dash, crate::canvas::Dash::Dashed);
    // A solid fill shows with the borders layer, and goes with it.
    b.view.fill = Some(plan_core::fill_styles::FillStyle::solid([10, 120, 30]));
    let has_fill = |ps: &[Prim]| {
        ps.iter().any(|p| {
            matches!(
                p,
                Prim::Fill {
                    color: plan_docs::PdfColor::Rgb(10, 120, 30),
                    ..
                }
            )
        })
    };
    assert!(has_fill(&prims(&b, &layers)));
    let mut hidden = layers.clone();
    hidden.set_visible(LAYER_BOX_BORDERS, false);
    assert!(!has_fill(&prims(&b, &hidden)));
    assert!(border(&prims(&b, &hidden)).is_none());
}

#[test]
fn snap_to_the_active_cad_point_takes_the_nearest_corner() {
    let mut l = layout();
    l.add_page(1, "P");
    l.page_mut(1).unwrap().cad.push(plan_core::CadObject {
        id: 1,
        layer: "Layout CAD".into(),
        item: plan_core::CadItem::Line {
            a: plan_core::Point::new(3.0, 4.0),
            b: plan_core::Point::new(9.0, 4.0),
        },
    });
    let near = snap_point(&l, 1, plan_core::Point::new(3.2, 4.1), 0.5);
    assert_eq!(near, plan_core::Point::new(3.0, 4.0));
    let far = snap_point(&l, 1, plan_core::Point::new(6.0, 8.0), 0.5);
    assert_eq!(far, plan_core::Point::new(6.0, 8.0));
    assert_eq!(
        snap_point(&l, 7, plan_core::Point::new(1.0, 1.0), 5.0),
        plan_core::Point::new(1.0, 1.0)
    );
    // The box being placed must not snap to its own corners.
    l.page_mut(1).unwrap().boxes.push(LayoutBox::new(
        5,
        (
            plan_core::Point::new(6.0, 6.0),
            plan_core::Point::new(7.0, 7.0),
        ),
        BoxSource::text("x", 12.0),
        Scale::QuarterInch,
    ));
    let at = plan_core::Point::new(6.1, 6.1);
    assert_eq!(snap_point(&l, 1, at, 0.5), plan_core::Point::new(6.0, 6.0));
    assert_eq!(snap_point_except(&l, 1, at, 0.5, Some(5)), at);
}

#[test]
fn old_layout_boxes_without_a_view_load_unchanged() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let mut l = layout();
    send_view(
        &mut l,
        &cx,
        &request(1, plan(), SendScale::Named(Scale::QuarterInch)),
    );
    let mut json = serde_json::to_value(&l).unwrap();
    // A layout saved before boxes had a view.
    for p in json["pages"].as_array_mut().unwrap() {
        for b in p["boxes"].as_array_mut().unwrap() {
            b.as_object_mut().unwrap().remove("view");
        }
    }
    let back: Layout = serde_json::from_value(json).unwrap();
    assert_eq!(back.pages[0].boxes[0].view, BoxView::default());
    assert_eq!(back.pages[0].boxes[0].scale, Scale::QuarterInch);
    // And the new fields round-trip.
    let mut l2 = l.clone();
    l2.pages[0].boxes[0].view.scale_mode = ScaleMode::Custom(0.123);
    l2.pages[0].boxes[0].view.saved_view = Some("Main".into());
    let back: Layout = serde_json::from_str(&serde_json::to_string(&l2).unwrap()).unwrap();
    assert_eq!(back, l2);
}
