//! Scenario 61: layout boxes and Send to Layout (Round 16, brief 02). A plan
//! goes out at 1 in = 100 ft and another at 3 in = 1 ft (the too-big
//! warning), a section goes out as Plot Lines on a second page, the boxes are
//! rescaled, panned, recentered and fitted, a linked saved plan view is
//! changed and followed, semi-dynamic and Plot Lines views are updated, plot
//! lines are edited, and each of those is one undo step (L-145, L-150, L-155,
//! L-156, L-161, L-199..L-207, L-229, L-231, L-232).

#![allow(clippy::field_reassign_with_default)]
use super::s21_layout_print::isolate_home;
use super::{draw_shell, Sim};
use crate::dialogs::layout::{PageChoice, Placement, SendSource, SendSpec};
use crate::dialogs::send_to_layout::SendDetails;
use crate::shell::layout_window::{self as lw, LayoutView};
use plan_core::camera::CameraKind;
use plan_core::geometry::Point;
use plan_core::{CameraObject, Id};
use plan_docs::{MasterList, Scale};
use plan_layout::{
    BoxSource, CameraLink, LineSpec, LineType, NewScale, ScaleMode, SendOptions, SendScale,
    UpdateKind, UpdateScope,
};

const W: f64 = 480.0;
const H: f64 = 360.0;

/// The house, an empty layout and the exterior elevation camera.
fn house() -> (Sim, LayoutView, Id) {
    isolate_home();
    lw::use_memory_master_list(MasterList::default());
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.app.cx.project.name = "Maple Court Residence".into();
    sim.app.cx.refresh();
    let cam = sim.app.cx.project.add_camera(CameraObject::new(
        CameraKind::Elevation,
        Point::new(W / 2.0, -120.0),
        90.0,
        "Front Elevation",
        0,
    ));
    let mut v = LayoutView::default();
    assert!(v.create(&mut sim.app.cx.project, None));
    // A second page for the elevation.
    assert!(v.add_page(&mut sim.app.cx.project, false));
    (sim, v, cam)
}

fn plan_spec(page: u32) -> SendSpec {
    SendSpec {
        source: SendSource::Plan {
            floor: 0,
            layer_set: "Default Set".into(),
        },
        page: PageChoice::Existing(page),
        scale: None,
        placement: Placement::FirstFree,
    }
}

fn camera_spec(id: Id, page: u32) -> SendSpec {
    SendSpec {
        source: SendSource::Camera {
            id,
            name: "Front Elevation".into(),
        },
        page: PageChoice::Existing(page),
        scale: None,
        placement: Placement::FirstFree,
    }
}

fn details(scale: SendScale, options: SendOptions) -> SendDetails {
    SendDetails {
        scale,
        options,
        snap_to_point: false,
        show_page: true,
        all_remaining: false,
        as_image: false,
    }
}

fn first_page(v: &LayoutView) -> u32 {
    v.layout()
        .unwrap()
        .pages
        .iter()
        .find(|p| !p.template_page)
        .unwrap()
        .number
}

fn boxes(v: &LayoutView) -> usize {
    v.layout()
        .unwrap()
        .pages
        .iter()
        .map(|p| p.boxes.len())
        .sum()
}

fn the_box(v: &LayoutView, id: Id) -> plan_layout::LayoutBox {
    v.layout()
        .unwrap()
        .pages
        .iter()
        .flat_map(|p| p.boxes.iter())
        .find(|b| b.id == id)
        .cloned()
        .unwrap()
}

/// Steps back one undo step and says what it was.
fn undo(v: &mut LayoutView, sim: &mut Sim) -> String {
    v.undo(&mut sim.app.cx.project).unwrap_or_default()
}

#[test]
fn a_plan_goes_out_at_one_inch_to_a_hundred_feet_and_a_big_one_is_warned_about() {
    let (mut sim, mut v, _) = house();
    let page = first_page(&v);
    let p = &mut sim.app.cx.project;
    // 1 in = 100 ft: exactly 1:1200 on paper, and it fits.
    let id = v
        .send_with(
            p,
            &plan_spec(page),
            &details(
                SendScale::Named(Scale::OneInchEq100Ft),
                SendOptions::default(),
            ),
            None,
        )
        .unwrap();
    assert_eq!(v.last_send_warning(), None);
    let b = the_box(&v, id);
    assert_eq!(b.scale, Scale::OneInchEq100Ft);
    assert_eq!(b.scale_note(), "1\" = 100'-0\"");
    let rcx = lw::render_context(p);
    let frame = plan_layout::view_frame_in(&b.source, &b.view, &rcx).unwrap();
    drop(rcx);
    let (w, h) = b.size_in();
    assert!((w * 1200.0 - frame.0).abs() < 1e-6 && (h * 1200.0 - frame.1).abs() < 1e-6);
    assert_eq!(v.undo_label(), Some("Send to Layout"));

    // 3 in = 1 ft: the box is placed but the warning says it is too big.
    let before = boxes(&v);
    let big = v
        .send_with(
            p,
            &plan_spec(page),
            &details(SendScale::Named(Scale::ThreeInch), SendOptions::default()),
            None,
        )
        .unwrap();
    let warning = v.last_send_warning().expect("too big for the sheet");
    assert!(warning.contains("too big for the sheet"), "{warning}");
    assert_eq!(boxes(&v), before + 1);
    assert!(the_box(&v, big).size_in().0 > 100.0);
    // One undo step took it out again; the 1:1200 box stays.
    assert_eq!(undo(&mut v, &mut sim), "Send to Layout");
    assert_eq!(boxes(&v), before);

    // A typed ratio that is on no list keeps its exact factor.
    let p = &mut sim.app.cx.project;
    let ipf = plan_layout::parse_scale_text("1:37").unwrap();
    let odd = v
        .send_with(
            p,
            &plan_spec(page),
            &details(SendScale::PerFoot(ipf), SendOptions::default()),
            None,
        )
        .unwrap();
    assert_eq!(the_box(&v, odd).scale_note(), "1:37");
    // Fit to Sheet: about half the sheet, no scale.
    let fit = v
        .send_with(
            p,
            &plan_spec(page),
            &details(SendScale::FitToSheet, SendOptions::default()),
            None,
        )
        .unwrap();
    assert!(the_box(&v, fit).is_no_scale());
    assert_eq!(the_box(&v, fit).scale_note(), "NOT TO SCALE");
}

#[test]
fn a_section_sent_as_plot_lines_is_rescaled_panned_fitted_and_updated() {
    let (mut sim, mut v, cam) = house();
    let page = v.layout().unwrap().pages.last().unwrap().number;
    let mut options = SendOptions::default();
    options.camera = CameraLink::PlotLines;
    options.plot.color_fill = true;
    let id = v
        .send_with(
            &mut sim.app.cx.project,
            &camera_spec(cam, page),
            &details(SendScale::Named(Scale::EighthInch), options),
            None,
        )
        .unwrap();
    let b = the_box(&v, id);
    assert_eq!(b.update_kind(), UpdateKind::PlotLine);
    let art = b.view.art.clone().expect("the lines are kept");
    let (edges, patterns) = art.counts();
    assert!(edges > 4, "the elevation's edge lines: {edges}");
    let _ = patterns;
    assert!(b.view.plot.color_fill);
    v.selected = Some(id);

    // Rescale Layout View: 1/8" -> 1/4", the box grows with it. One step.
    let (w0, h0) = b.size_in();
    assert!(v.rescale_selected(
        &mut sim.app.cx.project,
        NewScale::Named(Scale::QuarterInch),
        None
    ));
    let (w1, h1) = the_box(&v, id).size_in();
    assert!((w1 / w0 - 2.0).abs() < 1e-9 && (h1 / h0 - 2.0).abs() < 1e-9);
    assert_eq!(v.undo_label(), Some("Rescale Layout View"));
    // Undoing puts the scale and the size back in one step.
    assert_eq!(undo(&mut v, &mut sim), "Rescale Layout View");
    assert_eq!(the_box(&v, id).scale, Scale::EighthInch);
    assert_eq!(the_box(&v, id).size_in(), (w0, h0));

    // Pan/Scale: a pan of (0.5, 0.25) paper inches, one step.
    assert!(v.pan_selected(&mut sim.app.cx.project, (0.5, 0.25)));
    assert_eq!(the_box(&v, id).view.pan_in, (0.5, 0.25));
    assert_eq!(undo(&mut v, &mut sim), "Pan Layout Box");
    assert_eq!(the_box(&v, id).view.pan_in, (0.0, 0.0));
    // Typed scale of the Pan/Scale tool.
    assert_eq!(
        v.set_scale_text(&mut sim.app.cx.project, "1:96"),
        Ok(false),
        "1/8\" is 1:96"
    );
    assert_eq!(v.set_scale_text(&mut sim.app.cx.project, "1:48"), Ok(true));
    assert_eq!(the_box(&v, id).scale_note(), "1/4\" = 1'-0\"");
    assert!(v.set_scale_text(&mut sim.app.cx.project, "banana").is_err());
    undo(&mut v, &mut sim);

    // Scale Layout Box Contents to Fit: a bigger box fills with the view.
    v.set_box_bounds(
        &mut sim.app.cx.project,
        id,
        [1.0, 1.0, 13.0, 9.0],
        "Resize Layout Box",
    );
    assert!(v.scale_selected_to_fit(&mut sim.app.cx.project));
    let fitted = the_box(&v, id);
    let rcx = lw::render_context(&sim.app.cx.project);
    let map = plan_layout::box_content_map(&fitted, &rcx).unwrap();
    drop(rcx);
    let (cw, ch) = (map.content.0 / 72.0, map.content.1 / 72.0);
    assert!(cw <= 12.0 + 1e-6 && ch <= 8.0 + 1e-6);
    assert!(
        (cw - 12.0).abs() < 1e-6 || (ch - 8.0).abs() < 1e-6,
        "{cw} x {ch}"
    );
    assert_eq!(undo(&mut v, &mut sim), "Scale Layout Box Contents to Fit");
    // Recenter after a pan.
    assert!(v.pan_selected(&mut sim.app.cx.project, (2.0, 0.0)));
    assert!(v.recenter_selected(&mut sim.app.cx.project));
    assert_eq!(undo(&mut v, &mut sim), "Recenter Layout Box Contents");

    // The model changes: a Plot Lines view keeps what it has until updated.
    let before = the_box(&v, id).view.art.clone().unwrap();
    {
        let p = &mut sim.app.cx.project;
        p.add_wall(
            0,
            Point::new(W, 0.0),
            Point::new(W + 240.0, 0.0),
            6.5,
            109.125,
            plan_core::WallKind::Exterior,
        );
    }
    assert_eq!(the_box(&v, id).view.art.as_ref().unwrap(), &before);
    // Print does not update it.
    let _ = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    assert_eq!(the_box(&v, id).view.art.as_ref().unwrap(), &before);
    // Update All Live Views leaves it alone, Update All Plot Line Views does not.
    let r = v.update_views(&mut sim.app.cx.project, &UpdateScope::LiveViews);
    assert_eq!(r.updated, 0);
    let r = v.update_views(&mut sim.app.cx.project, &UpdateScope::PlotLines);
    assert_eq!(r.updated, 1);
    let after = the_box(&v, id).view.art.clone().unwrap();
    assert!(
        after.bounds.1.x > before.bounds.1.x,
        "the new wall is in the view"
    );
    assert_eq!(undo(&mut v, &mut sim), "Update Layout Views");
    assert_eq!(the_box(&v, id).view.art.as_ref().unwrap(), &before);
}

#[test]
fn plot_lines_are_selected_edited_drawn_and_replaced_by_an_update() {
    let (mut sim, mut v, cam) = house();
    let page = v.layout().unwrap().pages.last().unwrap().number;
    let mut options = SendOptions::default();
    options.camera = CameraLink::PlotLines;
    let id = v
        .send_with(
            &mut sim.app.cx.project,
            &camera_spec(cam, page),
            &details(SendScale::Named(Scale::QuarterInch), options),
            None,
        )
        .unwrap();
    v.selected = Some(id);
    let b = the_box(&v, id);
    let art = b.view.art.clone().unwrap();
    let n = art.lines.len();
    // Click a line on the page to select it.
    let rcx = lw::render_context(&sim.app.cx.project);
    let map = plan_layout::box_content_map(&b, &rcx).unwrap();
    let target = art
        .lines
        .iter()
        .find(|l| l.kind == LineType::Edge)
        .unwrap()
        .clone();
    let on_paper = map.to_paper(Point::lerp(target.a, target.b, 0.5));
    drop(rcx);
    assert!(v.select_line_at(&sim.app.cx.project, on_paper.x, on_paper.y, 0.05, false));
    assert!(v.selected_lines().contains(&target.id) || v.selected_lines().len() == 1);

    // Layout Line Specification: pattern line, heavy weight, own color.
    let spec = LineSpec {
        kind: Some(LineType::Pattern),
        weight_pt: Some(Some(1.5)),
        color: Some(Some([200, 0, 0])),
        ..LineSpec::default()
    };
    assert_eq!(v.apply_line_spec(&mut sim.app.cx.project, &spec), 1);
    let changed = the_box(&v, id).view.art.unwrap();
    let edited = changed
        .lines
        .iter()
        .find(|l| v.selected_lines().contains(&l.id))
        .unwrap();
    assert_eq!(edited.kind, LineType::Pattern);
    assert_eq!(edited.weight_pt, Some(1.5));
    assert_eq!(undo(&mut v, &mut sim), "Layout Line Specification");

    // Draw a new line inside the box, then delete the selected one.
    let new = v
        .add_plot_line(
            &mut sim.app.cx.project,
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
        )
        .unwrap();
    assert_eq!(the_box(&v, id).view.art.unwrap().lines.len(), n + 1);
    assert!(the_box(&v, id).view.art.unwrap().line(new).unwrap().added);
    assert_eq!(v.delete_selected_lines(&mut sim.app.cx.project), 1);
    assert_eq!(the_box(&v, id).view.art.unwrap().lines.len(), n);
    assert_eq!(undo(&mut v, &mut sim), "Delete Layout Lines");
    assert_eq!(undo(&mut v, &mut sim), "Draw Layout Line");
    assert_eq!(the_box(&v, id).view.art.unwrap().lines.len(), n);

    // The lines are the box's own: an update deletes the edit and the line
    // you drew, and brings the generated ones back.
    v.add_plot_line(
        &mut sim.app.cx.project,
        Point::new(0.0, 0.0),
        Point::new(60.0, 0.0),
    );
    assert_eq!(the_box(&v, id).view.art.unwrap().lines.len(), n + 1);
    let r = v.update_views(&mut sim.app.cx.project, &UpdateScope::Selected(vec![id]));
    // Nothing in the plan changed, but the picture is made again.
    assert_eq!(r.updated, 1);
    let art = the_box(&v, id).view.art.unwrap();
    assert_eq!(art.lines.len(), n);
    assert!(art.lines.iter().all(|l| !l.added));
}

#[test]
fn a_linked_saved_plan_view_is_followed_and_can_be_unlinked() {
    let (mut sim, mut v, _) = house();
    let page = first_page(&v);
    let p = &mut sim.app.cx.project;
    // A second floor, and a saved plan view that shows the first.
    assert_eq!(p.build_new_floor(false), 1, "a blank second floor");
    p.layer_sets
        .sets
        .push(plan_core::layer_sets::LayerSetDef::new("Main Set"));
    let mut view = plan_core::layer_sets::SavedPlanView::new("Main Plan", "Main Set");
    view.floor = Some(0);
    p.plan_views.push(view);
    let mut options = SendOptions::default();
    options.link_saved_view = Some("Main Plan".into());
    let id = v
        .send_with(
            p,
            &plan_spec(page),
            &details(SendScale::Named(Scale::EighthInch), options),
            None,
        )
        .unwrap();
    v.selected = Some(id);
    let size_of = |v: &LayoutView, p: &plan_core::Project| {
        let b = the_box(v, id);
        let rcx = lw::render_context(p);
        plan_layout::view_frame_in(&b.source, &b.view, &rcx).unwrap()
    };
    let first_floor = size_of(&v, &sim.app.cx.project);
    assert!(first_floor.0 > 400.0, "the house: {first_floor:?}");
    // Change the saved view: it shows the (empty) second floor now, and the
    // box follows without being sent again.
    // (The project already carries Chief's default saved views: find ours.)
    let at = |p: &plan_core::Project| {
        p.plan_views
            .iter()
            .position(|v| v.name == "Main Plan")
            .unwrap()
    };
    let i = at(&sim.app.cx.project);
    sim.app.cx.project.plan_views[i].floor = Some(1);
    let second_floor = size_of(&v, &sim.app.cx.project);
    assert!(second_floor.0 < first_floor.0, "{second_floor:?}");
    // Delete the saved view: the box is flagged as a missing view.
    let saved = sim.app.cx.project.plan_views.remove(i);
    assert!(plan_layout::missing_view(&the_box(&v, id), &sim.app.cx.project).is_some());
    let rcx = lw::render_context(&sim.app.cx.project);
    let art = plan_layout::render_box_artwork(&the_box(&v, id), &rcx);
    assert!(
        art.texts
            .iter()
            .any(|t| t.text.contains("Missing layout view")),
        "the caution says what is missing"
    );
    drop(rcx);
    sim.app.cx.project.plan_views.push(saved);
    // Unlink: the box keeps the floor the saved view gave it, and no link.
    assert!(v.unlink_saved_view(&mut sim.app.cx.project));
    let b = the_box(&v, id);
    assert_eq!(b.view.saved_view, None);
    assert!(matches!(b.source, BoxSource::PlanView { floor: 1, .. }));
    assert_eq!(undo(&mut v, &mut sim), "Unlink Saved Plan View");
    assert_eq!(
        the_box(&v, id).view.saved_view.as_deref(),
        Some("Main Plan")
    );
}

#[test]
fn update_on_demand_views_update_in_the_printout_and_on_request() {
    let (mut sim, mut v, cam) = house();
    let page = v.layout().unwrap().pages.last().unwrap().number;
    let mut options = SendOptions::default();
    options.camera = CameraLink::OnDemand;
    let id = v
        .send_with(
            &mut sim.app.cx.project,
            &camera_spec(cam, page),
            &details(SendScale::Named(Scale::EighthInch), options),
            None,
        )
        .unwrap();
    assert_eq!(the_box(&v, id).update_kind(), UpdateKind::SemiDynamic);
    let pdf_before = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    sim.app.cx.project.add_wall(
        0,
        Point::new(W, 0.0),
        Point::new(W + 240.0, 0.0),
        6.5,
        109.125,
        plan_core::WallKind::Exterior,
    );
    // The layout page still shows the old picture ...
    let stale = the_box(&v, id).view.art.clone().unwrap();
    // ... but printing updates it (the printout differs), without changing
    // the layout itself.
    let pdf_after = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    assert_ne!(pdf_before, pdf_after);
    assert_eq!(the_box(&v, id).view.art.as_ref().unwrap(), &stale);
    // Update Selected View makes it current on the page, as one step.
    v.selected = Some(id);
    let r = v.update_views(&mut sim.app.cx.project, &UpdateScope::Selected(vec![id]));
    assert_eq!(r.updated, 1);
    assert_ne!(the_box(&v, id).view.art.as_ref().unwrap(), &stale);
    assert_eq!(undo(&mut v, &mut sim), "Update Layout Views");
    // A Live View, Always Update keeps no picture and has nothing to update.
    let mut live = SendOptions::default();
    live.camera = CameraLink::Always;
    let id2 = v
        .send_with(
            &mut sim.app.cx.project,
            &camera_spec(cam, page),
            &details(SendScale::Named(Scale::EighthInch), live),
            None,
        )
        .unwrap();
    assert!(the_box(&v, id2).view.art.is_none());
    let r = v.update_views(&mut sim.app.cx.project, &UpdateScope::Selected(vec![id2]));
    assert_eq!((r.updated, r.already_current), (0, 1));
}

#[test]
fn current_screen_as_image_and_snap_to_the_active_cad_point() {
    let (mut sim, mut v, _) = house();
    let page = first_page(&v);
    let p = &mut sim.app.cx.project;
    // Current Screen: just the part of the plan that was on screen.
    let mut options = SendOptions::default();
    options.extent = plan_layout::SendExtent::CurrentScreen([0.0, 0.0, 240.0, 120.0]);
    let id = v
        .send_with(
            p,
            &plan_spec(page),
            &details(SendScale::Named(Scale::QuarterInch), options),
            None,
        )
        .unwrap();
    let (w, h) = the_box(&v, id).size_in();
    assert!(
        (w - 240.0 / 48.0).abs() < 1e-6 && (h - 120.0 / 48.0).abs() < 1e-6,
        "{w} x {h}"
    );
    assert!(matches!(the_box(&v, id).view.scale_mode, ScaleMode::Named));
    // As Image: a static picture box that cannot be updated.
    let mut image = details(SendScale::Named(Scale::EighthInch), SendOptions::default());
    image.as_image = true;
    let pic = v.send_with(p, &plan_spec(page), &image, None).unwrap();
    let b = the_box(&v, pic);
    assert!(matches!(b.source, BoxSource::ImageData { .. }));
    assert_eq!(b.update_kind(), UpdateKind::Static);
    v.selected = Some(pic);
    let r = v.update_views(p, &UpdateScope::Selected(vec![pic]));
    assert_eq!((r.updated, r.skipped), (0, 1));
    // Snap to Active CAD Point: the new box's lower left takes the end of a
    // line already on the page.
    v.add_cad_line(p, Point::new(10.0, 10.0), Point::new(14.0, 10.0));
    let mut snap = details(
        SendScale::Named(Scale::OneInchEq100Ft),
        SendOptions::default(),
    );
    snap.snap_to_point = true;
    let spec = SendSpec {
        placement: Placement::Click,
        ..plan_spec(page)
    };
    let id = v.send_with(p, &spec, &snap, Some((10.2, 10.1))).unwrap();
    let r = lw::bounds(&the_box(&v, id));
    assert!(
        (r[0] - 10.0).abs() < 1e-9 && (r[1] - 10.0).abs() < 1e-9,
        "{r:?}"
    );
}

#[test]
fn old_layouts_open_with_the_new_label_layer_and_boxes_without_a_view() {
    let (mut sim, mut v, _) = house();
    let page = first_page(&v);
    let p = &mut sim.app.cx.project;
    v.send_with(
        p,
        &plan_spec(page),
        &lw::default_details(&plan_spec(page)),
        None,
    )
    .unwrap();
    // A layout saved before the Layout Box Labels layer and the box view.
    let mut json = p.layout.clone().unwrap();
    json["layers"]["layers"]
        .as_array_mut()
        .unwrap()
        .retain(|l| l["name"] != "Layout Box Labels");
    for pg in json["pages"].as_array_mut().unwrap() {
        for b in pg["boxes"].as_array_mut().unwrap() {
            b.as_object_mut().unwrap().remove("view");
        }
    }
    p.layout = Some(json);
    let layout = lw::load(p).unwrap();
    assert!(layout.layers.get("Layout Box Labels").is_some());
    let b = &layout
        .pages
        .iter()
        .find(|pg| pg.number == page)
        .unwrap()
        .boxes[0];
    assert_eq!(b.view, plan_layout::BoxView::default());
    assert_eq!(b.update_kind(), UpdateKind::Dynamic);
}
