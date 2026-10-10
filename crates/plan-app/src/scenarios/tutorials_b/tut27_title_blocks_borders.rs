//! Lesson 27, Title Blocks and Borders (pp. 460-483). Drawing Sheet Setup,
//! the 1/8 in grid unit and concentric borders, title block macros, the
//! revision table, per-page revisions, a renamed page table heading and an
//! imported logo.
use super::support_b::*;
use crate::dialogs::layout::LayoutDefaults;
use crate::shell::layout_window::{LayoutCommand as C, LayoutView};
use plan_core::geometry::Point;
use plan_core::CadItem;
use plan_docs::SheetSize;
use plan_layout::{PageRevision, SheetChoice};

#[test]
fn a_page_takes_its_own_sheet_and_returns_to_the_layouts() {
    let (mut sim, mut v) = cottage_layout();
    let layout_sheet = v.layout().unwrap().sheet_inches();
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    let at = v.page;
    d.select(at);
    d.sheet_mut().unwrap().sheet = Some(SheetChoice::Standard(SheetSize::ArchD));
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    let l = v.layout().unwrap();
    assert_eq!(l.page_sheet_inches(&l.pages[at]), (36.0, 24.0));
    assert_eq!(
        l.sheet_inches(),
        layout_sheet,
        "the layout's sheet is untouched"
    );
    d.sheet_mut().unwrap().sheet = None;
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    let l = v.layout().unwrap();
    assert_eq!(l.page_sheet_inches(&l.pages[at]), layout_sheet);
}

fn pdf_count(pdf: &[u8], needle: &str) -> usize {
    let text: String = pdf.iter().map(|&b| b as char).collect();
    text.matches(needle).count()
}

fn print(sim: &crate::scenarios::Sim, v: &crate::shell::layout_window::LayoutView) -> Vec<u8> {
    crate::shell::layout_window::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None)
}

#[test]
fn border_insets() {
    let (mut sim, mut v) = cottage_layout();
    let p = &mut sim.app.cx.project;
    let defaults = LayoutDefaults {
        snap_grid: true,
        snap_unit_in: 0.125,
    };
    assert!(v.apply_layout_defaults(p, &defaults));
    assert_eq!(v.layout().unwrap().snap_unit_in, 0.125);
    // The outer border on the template page, then two concentric copies.
    v.set_page(0);
    let outer = v
        .add_cad(
            p,
            "Draw Rectangle",
            CadItem::Polyline {
                points: vec![
                    Point::new(0.5, 0.5),
                    Point::new(35.5, 0.5),
                    Point::new(35.5, 23.5),
                    Point::new(0.5, 23.5),
                ],
                closed: true,
            },
        )
        .unwrap();
    let corner = |v: &LayoutView, id| match &v.layout().unwrap().pages[0]
        .cad
        .iter()
        .find(|o| o.id == id)
        .unwrap()
        .item
    {
        CadItem::Polyline { points, .. } => points[0],
        _ => panic!("a rectangle"),
    };
    for (inset, at) in [(0.125, 0.625), (0.25, 0.75)] {
        v.selected_cad = Some(outer);
        let copy = v.concentric_copy(p, inset).expect("a concentric copy");
        assert_eq!(corner(&v, copy), Point::new(at, at));
        assert_eq!(v.undo_label(), Some("Concentric Copy"));
    }
    assert_eq!(v.layout().unwrap().pages[0].cad.len(), 3);
    assert_eq!(v.undo(p).as_deref(), Some("Concentric Copy"));
    assert_eq!(v.layout().unwrap().pages[0].cad.len(), 2);
}

#[test]
fn title_block_macros() {
    let (mut sim, mut v) = cottage_layout();
    sim.app.cx.project.info.date = "10/10/26".into();
    v.edit(&mut sim.app.cx.project, "Draw", |l| {
        l.pages[0].add_text(
            Point::new(1.0, 0.8),
            "TB %layout.title% %layout.label% %date%",
            0.2,
        );
        l.pages[1].add_line(Point::new(2.0, 2.0), Point::new(6.0, 2.0));
        true
    });
    let mut d = v.page_info_dialog(&sim.app.cx.project).unwrap();
    d.select(1);
    d.info_mut().unwrap().title = "Floor Plans".into();
    d.info_mut().unwrap().label = "A1.#".into();
    assert_eq!(v.apply_page_info(&mut sim.app.cx.project, &d), Ok(true));
    let pdf = print(&sim, &v);
    assert_eq!(pdf_count(&pdf, "TB Floor Plans A1.1 10/10/26"), 1);
    assert_eq!(pdf_count(&pdf, "%layout"), 0);
}

#[test]
fn revision_table() {
    let (mut sim, mut v) = cottage_layout();
    v.edit(&mut sim.app.cx.project, "Draw", |l| {
        l.pages[1].add_line(Point::new(2.0, 2.0), Point::new(6.0, 2.0));
        true
    });
    let number = v.layout().unwrap().pages[1].number;
    for n in 1..=7 {
        let rev = PageRevision::new(&n.to_string(), "10/10/26", "DAD", &format!("RevText{n}x"));
        assert!(v.add_layout_revision(&mut sim.app.cx.project, &[number], &rev));
    }
    assert_eq!(v.layout().unwrap().revision_table_rows(number).len(), 7);
    v.set_page(1);
    v.run(&mut sim.app.cx, C::RevisionTable, None);
    v.place_at(&mut sim.app.cx, 12.0, 10.0);
    let pdf = print(&sim, &v);
    for n in 1..=7 {
        assert!(pdf_count(&pdf, &format!("RevText{n}x")) >= 1, "row {n}");
    }
}

#[test]
fn page_revision() {
    let (mut sim, mut v) = cottage_layout();
    for _ in 0..3 {
        assert!(v.add_page(&mut sim.app.cx.project, false));
    }
    v.edit(&mut sim.app.cx.project, "Draw", |l| {
        for p in l.pages.iter_mut().skip(1) {
            p.add_line(Point::new(2.0, 2.0), Point::new(6.0, 2.0));
        }
        true
    });
    // The Revision Table sits on the template, so every page shows its own.
    v.set_page(0);
    v.run(&mut sim.app.cx, C::RevisionTable, None);
    v.place_at(&mut sim.app.cx, 12.0, 10.0);
    let numbers: Vec<u32> = v.layout().unwrap().pages.iter().map(|p| p.number).collect();
    let rev = PageRevision::new("A", "10/10/26", "DAD", "AddendumNote");
    assert!(v.add_layout_revision(&mut sim.app.cx.project, &numbers[3..5], &rev));
    let l = v.layout().unwrap();
    assert!(l.revision_table_rows(numbers[1]).is_empty());
    assert!(l.revision_table_rows(numbers[2]).is_empty());
    assert_eq!(l.revision_table_rows(numbers[3]).len(), 1);
    assert_eq!(l.revision_table_rows(numbers[4]).len(), 1);
    let bytes = |range| {
        crate::shell::layout_window::print_bytes(
            v.layout().unwrap(),
            &sim.app.cx.project,
            Some(range),
        )
    };
    assert_eq!(
        pdf_count(&bytes((1, 2)), "AddendumNote"),
        0,
        "pages 1 and 2"
    );
    assert!(
        pdf_count(&bytes((3, 4)), "AddendumNote") >= 2,
        "pages 3 and 4"
    );
}

#[test]
fn sheet_index() {
    let (mut sim, mut v) = cottage_layout();
    v.edit(&mut sim.app.cx.project, "Draw", |l| {
        l.pages[1].add_line(Point::new(2.0, 2.0), Point::new(6.0, 2.0));
        true
    });
    v.set_page(1);
    v.run(&mut sim.app.cx, C::PageTable, None);
    v.place_at(&mut sim.app.cx, 12.0, 6.0);
    let id = v.current_page().unwrap().boxes.last().unwrap().id;
    v.selected = Some(id);
    let before = v.page_tables(&sim.app.cx.project)[0].columns.clone();
    assert_eq!(before[0], "LABEL");
    let titles = vec!["Sheet".to_string(), String::new()];
    assert!(v.set_table_titles(&mut sim.app.cx.project, id, &titles));
    assert_eq!(v.undo_label(), Some("Table Column Titles"));
    let after = v.page_tables(&sim.app.cx.project)[0].columns.clone();
    assert_eq!(after, ["Sheet", &before[1], &before[2]]);
    assert!(
        !v.set_table_titles(&mut sim.app.cx.project, id, &titles),
        "no change"
    );
    assert_eq!(
        pdf_count(&print(&sim, &v), "Sheet"),
        1,
        "the heading prints"
    );
    assert_eq!(
        v.undo(&mut sim.app.cx.project).as_deref(),
        Some("Table Column Titles")
    );
    assert_eq!(v.page_tables(&sim.app.cx.project)[0].columns, before);
}

#[test]
fn logo_image() {
    let (mut sim, mut v) = cottage_layout();
    v.set_page(1);
    let px = (2u32, 2u32, vec![200u8; 16]);
    let p = &mut sim.app.cx.project;
    assert!(v
        .add_image_data_box(p, "logo", (2, 2, vec![0; 3]), (3.0, 3.0), true)
        .is_none());
    let id = v
        .add_image_data_box(p, "logo", px.clone(), (3.0, 3.0), true)
        .expect("the logo box");
    let b = v
        .current_page()
        .unwrap()
        .boxes
        .iter()
        .find(|b| b.id == id)
        .unwrap();
    assert!(
        matches!(b.source, plan_layout::BoxSource::ImageData { .. }),
        "saved in the plan"
    );
    assert_eq!(b.size_in(), (3.0, 3.0));
    assert_eq!(b.label, None, "Suppress Label");
    assert_eq!(v.undo_label(), Some("Import Image"));
    let named = v
        .add_image_data_box(p, "logo", px, (3.0, 3.0), false)
        .unwrap();
    let b = v
        .current_page()
        .unwrap()
        .boxes
        .iter()
        .find(|b| b.id == named)
        .unwrap();
    assert_eq!(b.label.as_deref(), Some("LOGO"));
    assert!(pdf_has(&print(&sim, &v), "/Subtype/Image"));
}
