//! Helpers for the lessons 15 to 28 replays (framing build, terrain lot,
//! layout view). Test-only.
#![allow(dead_code)]

use crate::dialogs::{defaults, framing as framing_dialog};
use crate::editor::framing_view::{self, FramingSettings};
use crate::scenarios::s21_layout_print::isolate_home;
use crate::scenarios::tutorials_support::cottage;
use crate::scenarios::Sim;
use crate::shell::layout_window::{self as lw, LayoutView};
use crate::toolbar::Action;
use crate::tools::terrain::TerrainVariant;
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::Key;
use plan_docs::MasterList;
use plan_framing::MemberKind;

/// Edits the framing settings in place.
pub fn change(sim: &mut Sim, edit: impl FnOnce(&mut FramingSettings)) {
    let mut st = framing_view::settings(&sim.app.cx.project);
    edit(&mut st);
    framing_view::set_settings(sim.cx(), st);
}

/// Build Framing dialog: Build, OK.
pub fn build_dialog_ok(sim: &mut Sim, all_floors: bool) {
    framing_dialog::request_build(all_floors);
    sim.action(Action::Custom(defaults::FRAMING));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.dialog_frame(true);
    sim.dialog_frame(false);
}

pub fn count(sim: &Sim, floor: usize, k: MemberKind) -> usize {
    framing_view::load(&sim.app.cx.project.floors[floor])
        .iter()
        .filter(|m| m.kind == k)
        .count()
}

/// The cottage with a second floor on top.
pub fn two_floor_cottage() -> Sim {
    let mut sim = cottage();
    sim.app.cx.project.build_new_floor(true);
    sim.app.cx.refresh();
    sim
}

/// The cottage with a rectangular terrain perimeter around it.
pub fn cottage_lot() -> Sim {
    let mut sim = cottage();
    sim.tool(ToolId::TerrainVariant(TerrainVariant::Perimeter));
    for (x, y) in [
        (-300.0, -300.0),
        (900.0, -300.0),
        (900.0, 780.0),
        (-300.0, 780.0),
    ] {
        sim.click(x, y);
    }
    sim.key(KeyEvent::key(Key::Enter));
    sim.tool(ToolId::Select);
    sim
}

/// Draws a terrain feature with `v`: one click per point, Enter, then each
/// `typed` answer (empty takes the default) and Enter.
pub fn draw_terrain(sim: &mut Sim, v: TerrainVariant, pts: &[(f64, f64)], typed: &[&str]) {
    sim.tool(ToolId::TerrainVariant(v));
    for &(x, y) in pts {
        sim.click(x, y);
    }
    sim.key(KeyEvent::key(Key::Enter));
    for t in typed {
        if !t.is_empty() {
            sim.key(KeyEvent::text(t));
        }
        sim.key(KeyEvent::key(Key::Enter));
    }
    sim.tool(ToolId::Select);
}

/// The cottage in a fresh layout (template page 0 and page 1).
pub fn cottage_layout() -> (Sim, LayoutView) {
    isolate_home();
    lw::use_memory_master_list(MasterList::default());
    let mut sim = cottage();
    sim.app.cx.project.name = "Chic Cottage".into();
    let mut v = LayoutView::default();
    assert!(v.create(&mut sim.app.cx.project, None));
    (sim, v)
}

/// Number of pages in a PDF byte stream.
pub fn pdf_pages(pdf: &[u8]) -> usize {
    let text = String::from_utf8_lossy(pdf);
    text.match_indices("/Type /Page")
        .filter(|(i, _)| !text[i + "/Type /Page".len()..].starts_with('s'))
        .count()
}

pub fn pdf_has(pdf: &[u8], needle: &str) -> bool {
    let text: String = pdf.iter().map(|&b| b as char).collect();
    text.contains(needle)
}
