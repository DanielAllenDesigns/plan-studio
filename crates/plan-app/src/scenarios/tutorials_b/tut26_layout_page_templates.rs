//! Lesson 26, Layout Page Templates (pp. 451-460). Real: a template page
//! stays out of the printed set, patterned labels number the sheets, an
//! inserted page leaves the others' numbers alone. Open: copy-layout and
//! the per-page template assignment check.
use super::support_b::*;

#[test]
fn patterned_labels_number_the_sheets_and_template_pages_are_not_printed() {
    let (mut sim, mut v) = cottage_layout();
    for _ in 0..5 {
        assert!(v.add_page(&mut sim.app.cx.project, false));
    }
    let n = v.layout().unwrap().pages.len();
    assert_eq!(n, 7, "template page 0 plus six pages");
    let mut d = v.page_info_dialog(&sim.app.cx.project).expect("a dialog");
    d.select(1);
    d.info_mut().unwrap().title = "Cover Sheet Template".into();
    d.info_mut().unwrap().template_page = true;
    for (i, label, title) in [
        (2, "A0.#", "Cover"),
        (3, "A1.#", "Plans"),
        (4, "A1.#", "Elevations"),
        (5, "E1.#", "Electrical"),
    ] {
        d.select(i);
        d.info_mut().unwrap().label = label.into();
        d.info_mut().unwrap().title = title.into();
    }
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    assert_eq!(v.undo_label(), Some("Page Information"));
    let l = v.layout().unwrap();
    let sheet = |i: usize| l.sheet_number_of(&l.pages[i]);
    assert_eq!(
        [sheet(2), sheet(3), sheet(4), sheet(5)],
        ["A0.1", "A1.1", "A1.2", "E1.1"].map(String::from)
    );
    assert_eq!(
        l.content_pages().len(),
        n - 2,
        "both template pages are skipped"
    );
    let pdf = crate::shell::layout_window::print_bytes(l, &sim.app.cx.project, None);
    assert_eq!(
        pdf_pages(&pdf),
        n - 2,
        "the print range is the content pages"
    );
    // Insert Page Before the Elevations page: the labelled pages keep their numbers.
    v.page = 4;
    assert!(v.add_page(&mut sim.app.cx.project, true));
    let l = v.layout().unwrap();
    assert_eq!(l.pages.len(), n + 1);
    assert_eq!(l.pages[5].title, "Elevations");
    assert_eq!(l.sheet_number_of(&l.pages[3]), "A1.1");
    assert_eq!(l.sheet_number_of(&l.pages[5]), "A1.2");
}

#[test]
#[ignore = "T7-26: L-190 (Assign Page Template per page from one Layout Page Information dialog; the page keeps its template)"]
fn assign_template_per_page() {
    let (mut sim, mut v) = cottage_layout();
    assert!(v.add_page(&mut sim.app.cx.project, false));
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    d.select(2);
    d.info_mut().unwrap().template = Some(0);
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    assert_eq!(v.layout().unwrap().pages[2].template, Some(0));
    panic!("L-190: also assert the template's boxes repeat on the page");
}

#[test]
#[ignore = "T7-26: L-192 (copy the layout file keeps the template pages and labels)"]
fn copy_the_layout() {
    crate::scenarios::tutorials_support::assert_ignored_break("L-192");
}
