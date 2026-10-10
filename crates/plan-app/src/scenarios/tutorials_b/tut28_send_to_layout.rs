//! Lesson 28, Sending Views to Layout (pp. 483-500). Real: the site plan is
//! sent at 1 in = 100 ft (1:1200 paper to model) to its own page, every
//! content page prints at 36 x 24 and the PDF has one page per content
//! page. Open: the too-large warning, Plot Lines, the callout page link.
use super::support_b::*;
use crate::dialogs::layout::{PageChoice, Placement, SendSource, SendSpec};
use crate::scenarios::tutorials_support::assert_ignored_break;
use plan_docs::{Scale, SheetSize};
use plan_layout::SheetChoice;

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

#[test]
#[ignore = "T7-28: L-229 (1 ft = 100 ft sent to a small page: the 'too large' warning)"]
fn too_large_warning() {
    assert_ignored_break("L-229");
}

#[test]
#[ignore = "T7-28: L-155 (elevation sent as Plot Lines with Color Fill, then Live View)"]
fn plot_lines_and_live_view() {
    assert_ignored_break("L-155");
}

#[test]
#[ignore = "T7-28: L-232 (callout label with a page link reads the linked page's label)"]
fn callout_page_link() {
    assert_ignored_break("L-232");
}

#[test]
#[ignore = "T7-28: L-161 (Link Saved Plan View on the plan box; stair section at 1/2 in = 1 ft)"]
fn linked_plan_view_and_section() {
    assert_ignored_break("L-161");
}
