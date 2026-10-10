//! Scenario 60: layout page management (Round 16, brief 01). Page
//! Information with `#` label patterns, a page template per page, page
//! revisions, the Layout Page Table and Layout Revision Table, dragging a
//! page, undo and redo, and the page macros in the PDF (L-188..L-192,
//! L-230, L-236).

use super::s21_layout_print::isolate_home;
use super::Sim;
use crate::shell::layout_window::{self as lw, LayoutCommand as C, LayoutView};
use plan_core::geometry::Point;
use plan_layout::{BoxSource, PageRevision};

fn pdf_text(sim: &Sim, v: &LayoutView) -> String {
    let bytes = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    bytes.iter().map(|&b| b as char).collect()
}

/// A layout with the standard template page 0 and three drawn pages, labelled
/// A0.# / A1.# / A1.# and titled.
fn three_pages(sim: &mut Sim) -> LayoutView {
    isolate_home();
    let mut v = LayoutView::default();
    assert!(v.create(&mut sim.app.cx.project, None));
    assert!(v.add_page(&mut sim.app.cx.project, false));
    assert!(v.add_page(&mut sim.app.cx.project, false));
    assert_eq!(v.layout().unwrap().pages.len(), 4, "template + three pages");
    // Something on every page so they all print; macros on the template.
    v.edit(&mut sim.app.cx.project, "Draw", |l| {
        l.pages[0].add_text(
            Point::new(1.0, 0.8),
            "SHEET %layout.label% PAGE %page.print% OF %numpages% %layout.title%",
            0.2,
        );
        for p in l.pages.iter_mut().skip(1) {
            p.add_line(Point::new(2.0, 2.0), Point::new(6.0, 2.0));
        }
        true
    });
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    for (i, (label, title, desc)) in [
        ("A0.#", "Cover Sheet", "Project data"),
        ("A1.#", "Floor Plans", "Main and upper floors"),
        ("A1.#", "Elevations", "All four sides"),
    ]
    .into_iter()
    .enumerate()
    {
        d.select(i + 1);
        let info = d.info_mut().unwrap();
        info.label = label.into();
        info.title = title.into();
        info.description = desc.into();
    }
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    v
}

fn labels(v: &LayoutView) -> Vec<String> {
    v.layout().unwrap().page_labels()
}

#[test]
fn hash_labels_number_each_prefix_and_duplicates_are_legal() {
    let mut sim = Sim::new();
    let mut v = three_pages(&mut sim);
    assert_eq!(labels(&v), ["A-0", "A0.1", "A1.1", "A1.2"]);
    assert_eq!(v.undo_label(), Some("Page Information"));
    // A fixed label is allowed, even when another page already has it.
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    d.select(2);
    d.info_mut().unwrap().label = "Cover".into();
    d.select(3);
    d.info_mut().unwrap().label = "Cover".into();
    assert_eq!(d.error(), None, "no uniqueness rule");
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    assert_eq!(labels(&v), ["A-0", "A0.1", "Cover", "Cover"]);
    // The Project Browser rows show the labels.
    let rows: Vec<String> = lw::page_list(&sim.app.cx.project)
        .into_iter()
        .map(|r| r.1)
        .collect();
    assert_eq!(rows[1], "A0.1  Cover Sheet");
    assert_eq!(rows[3], "Cover  Elevations");
}

#[test]
fn a_page_template_is_assigned_per_page_and_guarded() {
    let mut sim = Sim::new();
    let mut v = three_pages(&mut sim);
    // Page 1 becomes a second template (no border) for the cover.
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    d.select(1);
    d.info_mut().unwrap().template_page = true;
    d.info_mut().unwrap().title = "Cover Template".into();
    d.sheet_mut().unwrap().no_title_block = true;
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    let cover_template = v.layout().unwrap().pages[1].number;
    // Assign it to the Floor Plans page (index 2); Elevations keep the default.
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    d.select(2);
    d.info_mut().unwrap().template = Some(cover_template);
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    {
        let l = v.layout().unwrap();
        assert_eq!(l.template_index_for(2), Some(1));
        assert_eq!(l.template_index_for(3), Some(0), "the default is page zero");
        assert!(l.effective_no_title_block(2));
        assert!(!l.effective_no_title_block(3));
        assert!(l.delete_blocker(1).is_some(), "assigned: cannot be deleted");
    }
    // It cannot stop being a template while a page uses it.
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    d.select(1);
    d.info_mut().unwrap().template_page = false;
    assert!(d.error().is_some());
    assert!(v.apply_page_info(&mut sim.app.cx.project, &d).is_err());
    assert!(
        v.layout().unwrap().pages[1].template_page,
        "nothing changed"
    );
    // Undo takes the assignment back in one step.
    v.set_page(1);
    assert!(v.delete_current_page(&mut sim.app.cx.project).is_err());
    assert_eq!(
        v.undo(&mut sim.app.cx.project).as_deref(),
        Some("Page Information")
    );
    assert_eq!(v.layout().unwrap().pages[2].template, None);
    assert_eq!(
        v.redo(&mut sim.app.cx.project).as_deref(),
        Some("Page Information")
    );
    assert_eq!(v.layout().unwrap().pages[2].template, Some(cover_template));
}

#[test]
fn copy_drawings_to_another_page_is_one_undo_step() {
    let mut sim = Sim::new();
    let mut v = three_pages(&mut sim);
    v.set_page(0);
    let to = v.layout().unwrap().pages[3].number;
    let before = v.layout().unwrap().pages[3].cad.len();
    assert_eq!(v.copy_drawings_to(&mut sim.app.cx.project, to), 1);
    assert_eq!(v.layout().unwrap().pages[3].cad.len(), before + 1);
    assert_eq!(
        v.undo(&mut sim.app.cx.project).as_deref(),
        Some("Copy Drawings to Page")
    );
    assert_eq!(v.layout().unwrap().pages[3].cad.len(), before);
}

#[test]
fn revisions_tables_and_dragging_a_page() {
    let mut sim = Sim::new();
    let mut v = three_pages(&mut sim);
    // Add Layout Revision on two pages; one more through Page Information.
    let rev = PageRevision::new("1", "2026-10-02", "DAD", "Moved the stair");
    assert!(v.add_layout_revision(&mut sim.app.cx.project, &[2, 3], &rev));
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    d.select(3);
    d.info_mut().unwrap().revisions.push(PageRevision::new(
        "2",
        "2026-10-05",
        "DAD",
        "Added a skylight",
    ));
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    assert_eq!(v.layout().unwrap().revision_table_rows(3).len(), 2);
    assert_eq!(v.layout().unwrap().revision_table_rows(1).len(), 0);

    // Place the Layout Page Table on page 1 and the Layout Revision Table on
    // page 3, each by a click.
    v.set_page(1);
    v.run(&mut sim.app.cx, C::PageTable, None);
    v.place_at(&mut sim.app.cx, 12.0, 6.0);
    assert!(matches!(
        v.current_page().unwrap().boxes.last().unwrap().source,
        BoxSource::PageTable
    ));
    v.set_page(3);
    v.run(&mut sim.app.cx, C::RevisionTable, None);
    v.place_at(&mut sim.app.cx, 12.0, 10.0);
    assert!(matches!(
        v.current_page().unwrap().boxes.last().unwrap().source,
        BoxSource::RevisionTable
    ));
    assert_eq!(v.undo_label(), Some("Layout Table"));

    let t = pdf_text(&sim, &v);
    // The page table lists the pages with their labels and titles.
    assert!(t.contains("Floor Plans") && t.contains("Main and upper floors"));
    // The revision table shows page three's own rows.
    assert!(t.contains("Moved the stair") && t.contains("Added a skylight"));
    // The macros on the template page are evaluated per page.
    assert!(t.contains("SHEET A0.1 PAGE 1 OF 3 Cover Sheet"), "page one");
    assert!(t.contains("SHEET A1.1 PAGE 2 OF 3 Floor Plans"), "page two");
    assert!(
        t.contains("SHEET A1.2 PAGE 3 OF 3 Elevations"),
        "page three"
    );
    assert!(!t.contains("%layout.label%"));

    // Dragging Elevations (index 3) to the front renumbers every # label.
    assert_eq!(labels(&v), ["A-0", "A0.1", "A1.1", "A1.2"]);
    assert!(v.move_page_to(&mut sim.app.cx.project, 3, 1));
    let titles: Vec<String> = v
        .layout()
        .unwrap()
        .pages
        .iter()
        .map(|p| p.title.clone())
        .collect();
    assert_eq!(titles[1], "Elevations");
    assert_eq!(
        labels(&v),
        ["A-0", "A1.1", "A0.1", "A1.2"].map(String::from)
    );
    assert_eq!(v.layout().unwrap().page_table_rows()[0][1], "Elevations");
    // The page that was shown stays shown; one undo puts it back, redo again.
    assert_eq!(v.current_page().unwrap().title, "Elevations");
    assert_eq!(
        v.undo(&mut sim.app.cx.project).as_deref(),
        Some("Move Page")
    );
    assert_eq!(labels(&v), ["A-0", "A0.1", "A1.1", "A1.2"]);
    assert_eq!(
        v.redo(&mut sim.app.cx.project).as_deref(),
        Some("Move Page")
    );
    assert_eq!(v.layout().unwrap().pages[1].title, "Elevations");
    // After the drag the PDF shows the new order.
    let t = pdf_text(&sim, &v);
    assert!(t.contains("SHEET A1.1 PAGE 1 OF 3 Elevations"));
}

#[test]
fn general_layout_defaults_set_the_snap_and_the_nudge_unit() {
    let mut sim = Sim::new();
    let mut v = three_pages(&mut sim);
    let d = crate::dialogs::layout::LayoutDefaults {
        snap_grid: false,
        snap_unit_in: 0.25,
    };
    assert!(v.apply_layout_defaults(&mut sim.app.cx.project, &d));
    let l = v.layout().unwrap();
    assert!(!l.snap_grid && (l.snap_unit_in - 0.25).abs() < 1e-12);
    assert_eq!(v.undo_label(), Some("General Layout Defaults"));
    assert_eq!(
        v.undo(&mut sim.app.cx.project).as_deref(),
        Some("General Layout Defaults")
    );
    assert!(v.layout().unwrap().snap_grid);
    // The same values again change nothing.
    assert!(!v.apply_layout_defaults(
        &mut sim.app.cx.project,
        &crate::dialogs::layout::LayoutDefaults {
            snap_grid: true,
            snap_unit_in: plan_layout::DEFAULT_SNAP_UNIT_IN,
        }
    ));
}
