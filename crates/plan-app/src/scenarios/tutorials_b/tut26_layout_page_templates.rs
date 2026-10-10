//! Lesson 26, Layout Page Templates (pp. 451-460). Real: a template page
//! stays out of the printed set, patterned labels number the sheets, an
//! inserted page leaves the others' numbers alone. Open: copy-layout and
//! the per-page template assignment check.
use super::support_b::*;
use crate::shell::layout_window as lw;
use plan_core::geometry::Point;

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

/// A pdf text with `needle` counted.
fn count_in(pdf: &[u8], needle: &str) -> usize {
    let text: String = pdf.iter().map(|&b| b as char).collect();
    text.matches(needle).count()
}

#[test]
fn assign_template_per_page() {
    let (mut sim, mut v) = cottage_layout();
    assert!(v.add_page(&mut sim.app.cx.project, false));
    assert!(v.add_page(&mut sim.app.cx.project, false));
    // Page 0 is the default template; page 3 becomes a second template.
    v.edit(&mut sim.app.cx.project, "Draw", |l| {
        l.pages[0].add_text(Point::new(1.0, 0.8), "STANDARD %layout.label%", 0.2);
        l.pages[3].add_text(Point::new(1.0, 0.8), "COVERBLOCK %layout.title%", 0.2);
        for i in [1, 2] {
            l.pages[i].add_line(Point::new(2.0, 2.0), Point::new(6.0, 2.0));
        }
        true
    });
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    d.select(3);
    d.info_mut().unwrap().template_page = true;
    d.info_mut().unwrap().title = "Cover Template".into();
    d.select(1);
    d.info_mut().unwrap().label = "A1.#".into();
    d.info_mut().unwrap().title = "Plans".into();
    d.select(2);
    d.info_mut().unwrap().label = "A0.#".into();
    d.info_mut().unwrap().title = "Cover".into();
    d.info_mut().unwrap().template = Some(3);
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    let l = v.layout().unwrap();
    assert_eq!(l.pages[2].template, Some(3));
    assert_eq!(l.template_index_for(2), Some(3));
    assert_eq!(l.template_index_for(1), Some(0), "the default is page zero");
    assert_eq!(l.template_choices().len(), 2);
    // Each template's text repeats on its own pages only, filled from the
    // page's own title block fields (label, title).
    let pdf = crate::shell::layout_window::print_bytes(l, &sim.app.cx.project, None);
    assert_eq!(pdf_pages(&pdf), 2);
    assert_eq!(count_in(&pdf, "STANDARD A1.1"), 1);
    assert_eq!(count_in(&pdf, "COVERBLOCK Cover"), 1);
    assert_eq!(count_in(&pdf, "STANDARD A0.1"), 0, "the cover has its own");
    // One undo step takes the assignment away again.
    assert_eq!(v.undo_label(), Some("Page Information"));
}

#[test]
fn copy_the_layout() {
    let (mut sim, mut v) = cottage_layout();
    assert!(v.add_page(&mut sim.app.cx.project, false));
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    d.select(2);
    d.info_mut().unwrap().label = "A1.#".into();
    d.info_mut().unwrap().title = "Plans".into();
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    let before = v.layout().unwrap().clone();
    let p = &mut sim.app.cx.project;
    assert!(!v.copy_layout_file(p, "  "), "a name is needed");
    assert!(v.copy_layout_file(p, "Permit Set"));
    assert!(!v.copy_layout_file(p, "permit set"), "taken");
    let l = v.layout().unwrap();
    assert_eq!(l.name, "Permit Set");
    assert_eq!(l.pages.len(), before.pages.len());
    assert!(l.pages[0].template_page, "the template page came along");
    assert_eq!(l.page_labels(), before.page_labels());
    assert_eq!(l.pages[2].title, "Plans");
    assert_eq!(lw::layout_names(p).len(), 2, "the original is kept");
    assert_eq!(v.undo_label(), Some("Copy Layout File"));
    assert_eq!(v.undo(p).as_deref(), Some("Copy Layout File"));
    assert_eq!(lw::layout_names(p).len(), 1);
}
