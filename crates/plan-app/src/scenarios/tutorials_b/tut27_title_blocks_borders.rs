//! Lesson 27, Title Blocks and Borders (pp. 460-483). Real: Drawing Sheet
//! Setup, a page on its own sheet and back to the layout's sheet. Open:
//! the 1/8 in grid unit, concentric borders, macros, revision and sheet
//! index tables.
use super::support_b::*;
use crate::scenarios::tutorials_support::assert_ignored_break;
use plan_docs::SheetSize;
use plan_layout::SheetChoice;

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

#[test]
#[ignore = "T7-27: L-230 (General Layout Defaults: grid snap 1/8 in; concentric border insets 1/8 and 1/4)"]
fn border_insets() {
    assert_ignored_break("L-230");
}

#[test]
#[ignore = "T7-27: L-231 (title block text %layout.title% / %layout.label% / short date macros)"]
fn title_block_macros() {
    assert_ignored_break("L-231");
}

#[test]
#[ignore = "T7-27: L-189 (Layout Revision Table, 7 rows)"]
fn revision_table() {
    assert_ignored_break("L-189");
}

#[test]
#[ignore = "T7-27: L-195 (page revision shows only on pages 3 and 4)"]
fn page_revision() {
    assert_ignored_break("L-195");
}

#[test]
#[ignore = "T7-27: L-236 (Layout Page Table 'Sheet Index' with a renamed Label column)"]
fn sheet_index() {
    assert_ignored_break("L-236");
}

#[test]
#[ignore = "T7-27: L-193 (import a logo: Save in Plan, 3 x 3, Suppress Label)"]
fn logo_image() {
    assert_ignored_break("L-193");
}
