//! Lesson 28, Sending Views to Layout (pp. 483-500). Real: the site plan is
//! sent at 1 in = 100 ft (1:1200 paper to model) to its own page, every
//! content page prints at 36 x 24 and the PDF has one page per content
//! page. Also the too-large warning, Plot Lines and Live View, the callout
//! page link and the linked saved plan view with a 1/2 in section.
#![allow(clippy::field_reassign_with_default)]
use super::support_b::*;
use crate::dialogs::layout::{BoxSpec, PageChoice, Placement, SendSource, SendSpec};
use crate::dialogs::send_to_layout::SendDetails;
use crate::shell::layout_window::LayoutView;
use plan_core::callout::{ViewKind, ViewLink};
use plan_core::camera::CameraKind;
use plan_core::geometry::Point;
use plan_core::{CameraObject, Id};
use plan_docs::{Scale, SheetSize};
use plan_layout::{CameraLink, SendOptions, SendScale, SheetChoice, UpdateKind};

fn plan_spec(page: PageChoice, scale: Option<Scale>) -> SendSpec {
    SendSpec {
        source: SendSource::Plan {
            floor: 0,
            layer_set: "Default Set".into(),
        },
        page,
        scale,
        placement: Placement::FirstFree,
    }
}

#[test]
fn site_plan_at_1200_and_the_pdf_has_one_arch_d_page_per_content_page() {
    let (mut sim, mut v) = cottage_layout();
    let floor_plan = v
        .send(
            &mut sim.app.cx.project,
            &plan_spec(PageChoice::Existing(1), None),
            None,
        )
        .unwrap();
    let site = v
        .send(
            &mut sim.app.cx.project,
            &plan_spec(PageChoice::New, Some(Scale::OneInchEq100Ft)),
            None,
        )
        .unwrap();
    let l = v.layout().unwrap();
    assert_eq!(
        l.pages.len(),
        3,
        "template page, floor plan page, site page"
    );
    assert!(l.pages[1].boxes.iter().any(|b| b.id == floor_plan));
    let b = l.pages[2]
        .boxes
        .iter()
        .find(|b| b.id == site)
        .expect("the site box");
    assert_eq!(b.scale, Scale::OneInchEq100Ft);
    assert!(
        (12.0 / b.scale.inches_per_foot() - 1200.0).abs() < 1e-9,
        "1:1200 paper to model"
    );
    // Drawing Sheet Setup: both content pages on ARCH D.
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    for i in [1, 2] {
        d.select(i);
        d.sheet_mut().unwrap().sheet = Some(SheetChoice::Standard(SheetSize::ArchD));
    }
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    let l = v.layout().unwrap();
    let pdf = crate::shell::layout_window::print_bytes(l, &sim.app.cx.project, None);
    assert_eq!(pdf_pages(&pdf), l.content_pages().len());
    assert_eq!(pdf_pages(&pdf), 2);
    assert!(pdf_has(&pdf, "/MediaBox [0 0 2592 1728]"), "36 x 24 in");
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

/// The cottage layout with an elevation camera in front of the house.
fn with_camera() -> (crate::scenarios::Sim, LayoutView, Id) {
    let (mut sim, v) = cottage_layout();
    let cam = sim.app.cx.project.add_camera(CameraObject::new(
        CameraKind::Elevation,
        Point::new(300.0, -120.0),
        90.0,
        "Front Elevation",
        0,
    ));
    (sim, v, cam)
}

fn box_of(v: &LayoutView, id: Id) -> plan_layout::LayoutBox {
    v.layout()
        .unwrap()
        .pages
        .iter()
        .flat_map(|p| p.boxes.iter())
        .find(|b| b.id == id)
        .cloned()
        .unwrap()
}

#[test]
fn too_large_warning() {
    let (mut sim, mut v) = cottage_layout();
    let p = &mut sim.app.cx.project;
    // A small sheet for the site page.
    let mut d = v.page_info_dialog(p).unwrap();
    d.select(1);
    d.sheet_mut().unwrap().sheet = Some(SheetChoice::Standard(SheetSize::ArchA));
    assert_eq!(v.apply_page_info(p, &d), Ok(true));
    let send = |v: &mut LayoutView, p: &mut plan_core::Project, scale| {
        v.send_with(
            p,
            &plan_spec(PageChoice::Existing(1), None),
            &details(SendScale::Named(scale), SendOptions::default()),
            None,
        )
        .unwrap()
    };
    send(&mut v, p, Scale::ThreeInch);
    let warning = v.last_send_warning().expect("too big for the sheet");
    assert!(warning.contains("too big for the sheet"), "{warning}");
    // The site scale of the lesson fits, with no warning.
    send(&mut v, p, Scale::OneInchEq100Ft);
    assert_eq!(v.last_send_warning(), None);
}

#[test]
fn plot_lines_and_live_view() {
    let (mut sim, mut v, cam) = with_camera();
    let p = &mut sim.app.cx.project;
    let mut options = SendOptions::default();
    options.camera = CameraLink::PlotLines;
    options.plot.color_fill = true;
    let id = v
        .send_with(
            p,
            &camera_spec(cam, 1),
            &details(SendScale::Named(Scale::EighthInch), options),
            None,
        )
        .unwrap();
    let b = box_of(&v, id);
    assert_eq!(b.update_kind(), UpdateKind::PlotLine);
    assert!(b.view.art.is_some(), "the lines are kept");
    assert!(b.view.plot.color_fill);
    // Back to a Live View: the picture goes and the box follows the plan.
    let mut live = b.clone();
    live.view.camera = CameraLink::Always;
    assert!(v.apply_spec(
        p,
        &BoxSpec {
            layout_box: live,
            page: 1
        }
    ));
    let b = box_of(&v, id);
    assert_eq!(b.update_kind(), UpdateKind::Dynamic);
    assert!(b.view.art.is_none());
    assert_eq!(v.undo(p).as_deref(), Some("Layout Box Specification"));
    assert_eq!(box_of(&v, id).update_kind(), UpdateKind::PlotLine);
}

#[test]
fn callout_page_link() {
    let (mut sim, mut v) = cottage_layout();
    assert!(v.add_page(&mut sim.app.cx.project, false));
    let p = &mut sim.app.cx.project;
    let mut d = v.page_info_dialog(p).unwrap();
    d.select(2);
    d.info_mut().unwrap().label = "A5.#".into();
    d.info_mut().unwrap().title = "Sections".into();
    assert_eq!(v.apply_page_info(p, &d), Ok(true));
    let target = v.layout().unwrap().pages[2].number;
    let id = v
        .send(p, &plan_spec(PageChoice::Existing(1), None), None)
        .unwrap();
    let mut b = box_of(&v, id);
    b.label = Some("SEE %linked_view_layout_page_label%".into());
    let link = ViewLink {
        kind: ViewKind::LayoutPage,
        id: Id::from(target),
        name: String::new(),
    };
    b.view.label.link = Some(link.clone());
    assert!(v.apply_spec(
        p,
        &BoxSpec {
            layout_box: b,
            page: 1
        }
    ));
    // The plan's callout reports the page's label as Page Information
    // gives it, not the plain sheet number.
    let info = p.resolve_view_link(&link);
    assert!(info.valid);
    assert_eq!(info.page_label, "A5.1");
    assert_eq!(info.view_name, "Sections");
    let pdf = crate::shell::layout_window::print_bytes(v.layout().unwrap(), p, None);
    assert!(pdf_has(&pdf, "SEE A5.1"));
}

#[test]
fn linked_plan_view_and_section() {
    let (mut sim, mut v, cam) = with_camera();
    let p = &mut sim.app.cx.project;
    p.layer_sets
        .sets
        .push(plan_core::layer_sets::LayerSetDef::new("Main Set"));
    let mut view = plan_core::layer_sets::SavedPlanView::new("Main Plan", "Main Set");
    view.floor = Some(0);
    p.plan_views.push(view);
    let mut options = SendOptions::default();
    options.link_saved_view = Some("Main Plan".into());
    let plan = v
        .send_with(
            p,
            &plan_spec(PageChoice::Existing(1), None),
            &details(SendScale::Named(Scale::EighthInch), options),
            None,
        )
        .unwrap();
    assert_eq!(
        box_of(&v, plan).view.saved_view.as_deref(),
        Some("Main Plan")
    );
    // The stair section goes out at 1/2 in = 1 ft on its own page.
    let section = v
        .send_with(
            p,
            &camera_spec(cam, 1),
            &details(SendScale::Named(Scale::HalfInch), SendOptions::default()),
            None,
        )
        .unwrap();
    let b = box_of(&v, section);
    assert_eq!(b.scale, Scale::HalfInch);
    assert!((b.scale.inches_per_foot() - 0.5).abs() < 1e-9);
    // Unlinking the plan keeps it where it is, in one undo step.
    v.selected = Some(plan);
    assert!(v.unlink_saved_view(p));
    assert_eq!(box_of(&v, plan).view.saved_view, None);
}
