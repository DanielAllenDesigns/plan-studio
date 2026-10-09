//! Scenario 94 (round 16, brief 27): text tooling and the energy export.
//!
//! * Find/Replace Text replaces a word across the plan's text and the text of
//!   the plan's layout in one undo step (TXT-45, TXT-46).
//! * A text placed inside a room with `%room.name%` stays live and follows the
//!   room's name; a click places a text by its upper-left corner (TXT-59,
//!   TXT-62, DECISIONS 79).
//! * Project Information owners (client, designer, builder...) are macros in
//!   plan text and fill the layout title block (TXT-60, L-52).
//! * Text Macro Management exports user macros from one plan and imports them
//!   into another (TXT-63, TXT-64); Replace Fonts is one undo step (TXT-26).
//! * Thermal Envelope Data and Export to REScheck for a two-story plan: the
//!   totals match the walls and windows (L-83).

use super::{draw_shell, Sim};
use crate::dialogs::find_replace::FindReplaceDialog;
use crate::dialogs::text::macro_manager::{Choice, MacroManager};
use crate::dialogs::text::rescheck;
use crate::shell::layout_window as lw;
use crate::tools::text::TextMode;
use crate::tools::{KeyEvent, ToolId};
use eframe::egui;
use plan_core::cad::CadItem;
use plan_core::find_text::Scope;
use plan_core::geometry::Point;
use plan_core::model::{Floor, OpeningKind, RoomName, WallKind};
use plan_core::text_styles::TextMacros;
use plan_core::Id;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    super::s21_layout_print::isolate_home();
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    sim
}

fn steps(sim: &Sim) -> usize {
    sim.app.cx.action_history().0.len()
}

/// Types `s` into a Text tool click at `(x, y)`.
fn place_text(sim: &mut Sim, mode: TextMode, at: (f64, f64), s: &str) -> Id {
    sim.tool(ToolId::TextVariant(mode));
    sim.click(at.0, at.1);
    sim.key(KeyEvent::text(s));
    if mode == TextMode::RichText {
        sim.key(KeyEvent::key(egui::Key::Tab));
    } else {
        sim.key(KeyEvent::key(egui::Key::Enter));
    }
    sim.tool(ToolId::Select);
    sim.app
        .cx
        .floor()
        .cad
        .iter()
        .rev()
        .find(|c| matches!(c.item, CadItem::Text { .. }))
        .expect("a text")
        .id
}

fn text_of(sim: &Sim, id: Id) -> String {
    match &sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .expect("the text")
        .item
    {
        CadItem::Text { text, .. } => text.clone(),
        _ => panic!("not a text"),
    }
}

fn pos_of(sim: &Sim, id: Id) -> Point {
    match &sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .item
    {
        CadItem::Text { pos, .. } => *pos,
        _ => panic!("not a text"),
    }
}

/// A layout in the plan with a page titled and annotated with `word`.
fn add_layout(sim: &mut Sim, word: &str) {
    let mut layout = plan_layout::Layout::new("Sheets", plan_docs::SheetSize::ArchC);
    let page = layout.add_page(1, format!("{word} plans"));
    page.add_text(Point::new(2.0, 2.0), format!("{word} notes"), 0.125);
    lw::store(&mut sim.app.cx.project, &layout);
}

#[test]
fn replace_a_word_across_plan_text_and_layout_text_in_one_undo_step() {
    let mut sim = house();
    let a = place_text(&mut sim, TextMode::Text, (100.0, 100.0), "Master Bath");
    let b = place_text(&mut sim, TextMode::Text, (100.0, 200.0), "Bath fan");
    let hall = place_text(&mut sim, TextMode::Text, (300.0, 200.0), "Hall");
    add_layout(&mut sim, "Bath");
    let before = steps(&sim);

    let cx = &mut sim.app.cx;
    let mut d = FindReplaceDialog::new();
    d.search.find = "bath".into();
    d.search.replace = "Suite".into();
    d.scope = Scope::AllOpenFiles;
    d.find(cx);
    // Two plan texts, the layout's page title and its text.
    assert_eq!(d.results.len(), 4, "{:?}", d.result);
    // The current view alone sees only the plan's two.
    d.scope = Scope::CurrentView;
    d.find(cx);
    assert_eq!(d.results.len(), 2);
    d.scope = Scope::AllOpenFiles;
    d.replace_all(cx);
    assert_eq!(d.result, "Replaced 4 in 4 text object(s)");
    assert_eq!(steps(&sim), before + 1, "one undo step");
    assert_eq!(text_of(&sim, a), "Master Suite");
    assert_eq!(text_of(&sim, b), "Suite fan");
    assert_eq!(text_of(&sim, hall), "Hall");
    let layout = lw::load(&sim.app.cx.project).unwrap();
    assert_eq!(layout.pages[0].title, "Suite plans");
    let notes: Vec<String> = layout.pages[0]
        .cad
        .iter()
        .filter_map(|o| match &o.item {
            CadItem::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(notes, ["Suite notes"]);

    // One undo brings the plan and the layout back together.
    assert_eq!(sim.undo().as_deref(), Some("Replace Text"));
    assert_eq!(text_of(&sim, a), "Master Bath");
    let layout = lw::load(&sim.app.cx.project).unwrap();
    assert_eq!(layout.pages[0].title, "Bath plans");
    assert_eq!(sim.redo().as_deref(), Some("Replace Text"));
    assert_eq!(text_of(&sim, b), "Suite fan");
}

#[test]
fn a_click_is_the_upper_left_corner_and_a_room_macro_follows_the_room() {
    let mut sim = house();
    sim.app.cx.project.floors[0].room_names.push(RoomName {
        anchor: Point::new(240.0, 180.0),
        name: "Kitchen".into(),
        room_type: "Kitchen".into(),
        ..RoomName::default()
    });
    sim.app.cx.refresh();
    let id = place_text(
        &mut sim,
        TextMode::Text,
        (200.0, 150.0),
        "%room.name% %floor%",
    );
    assert_eq!(text_of(&sim, id), "Kitchen 1st Floor");
    // The click is the upper-left corner: the block hangs below it.
    let p = pos_of(&sim, id);
    assert!((p.x - 200.0).abs() < 6.0, "{p:?}");
    assert!(p.y < 150.0 && p.y > 130.0, "{p:?}");
    // The room is renamed: the text follows.
    sim.app.cx.project.floors[0].room_names[0].name = "Great Room".into();
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert_eq!(text_of(&sim, id), "Great Room 1st Floor");
    // The text was typed with macros: Find sees the macro names, not values.
    let cx = &mut sim.app.cx;
    let mut d = FindReplaceDialog::new();
    d.search.find = "great".into();
    d.find(cx);
    assert!(d.results.is_empty(), "{:?}", d.result);
    d.search.find = "room.name".into();
    d.search.macros = plan_core::find_text::MacroMode::MacrosOnly;
    d.find(cx);
    assert_eq!(d.results.len(), 1);
    // One undo step takes the text away.
    assert_eq!(sim.undo().as_deref(), Some("Place Text"));
}

#[test]
fn project_information_owners_are_macros_and_fill_the_title_block() {
    use crate::dialogs::project_info::{self, ProjectInfoDialog};
    let mut sim = house();
    let mut d = ProjectInfoDialog::new(&sim.app.cx.project.info);
    {
        let (_, info) = d.owners_mut();
        info.client_name = "Jane Smith".into();
        info.project_number = "26-041".into();
        info.designer = "Daniel".into();
        info.company = "Daniel Allen Designs".into();
    }
    d.add_owner("Builder").unwrap();
    d.add_field("License No").unwrap();
    assert!(d.set_value("License No", "GA-5521"));
    d.select_owner("Project");
    d.add_field("Lot")
        .expect_err("Lot is a system name already");
    assert!(d.set_value("Lot", "Lot 7"));
    let before = steps(&sim);
    assert!(project_info::apply(&mut sim.app.cx, d.draft()));
    assert_eq!(steps(&sim), before + 1);

    // Plan text uses any owner.
    let id = place_text(
        &mut sim,
        TextMode::Text,
        (100.0, 100.0),
        "%client.name% / %project.number% / %project.lot% / %builder.license_no% / %designer.company%",
    );
    assert_eq!(
        text_of(&sim, id),
        "Jane Smith / 26-041 / Lot 7 / GA-5521 / Daniel Allen Designs"
    );
    // The title block of a layout reads the same information.
    let layout = plan_layout::Layout::new("Sheets", plan_docs::SheetSize::ArchC);
    let ctx = lw::macro_context(&sim.app.cx.project);
    let fields = layout.title_block.expand_macros(&ctx);
    assert!(
        fields.iter().any(|(_, v)| v.contains("Jane Smith")),
        "{fields:?}"
    );
    assert!(
        fields.iter().any(|(_, v)| v.contains("Daniel")),
        "{fields:?}"
    );
    // The owners survive a save and a load.
    let back = plan_core::Project::from_json(&sim.app.cx.project.to_json().unwrap()).unwrap();
    assert_eq!(
        plan_core::macros::MacroOwners::from_info(&back.info).value(
            &back.info,
            "Builder",
            "License No"
        ),
        "GA-5521"
    );
    // The live text survives too.
    assert!(back.live_text(id).is_some());
}

#[test]
fn macros_export_from_one_plan_and_import_into_another() {
    let mut a = TextMacros::default();
    a.add("firm", "Daniel Allen Designs");
    a.add("stamp", "Drawn by %firm% on %date.short%");
    let src = MacroManager::new(a);
    let file = src.export_text(&[]);
    let mut b = TextMacros::default();
    b.add("firm", "Other Office");
    let mut dst = MacroManager::new(b);
    assert_eq!(dst.begin_import(&file), Ok(1));
    dst.choose("firm", Choice::Rename);
    let report = dst.finish_import().unwrap();
    assert_eq!(report.added, ["stamp"]);
    assert_eq!(report.renamed, [("firm".to_string(), "firm_2".to_string())]);
    // The importing plan uses the imported macro in a text.
    let mut sim = house();
    sim.app.cx.begin_change("Change Text Macros");
    sim.app.cx.project.set_text_macros(dst.draft());
    sim.app.cx.mark_dirty();
    let id = place_text(&mut sim, TextMode::Text, (100.0, 100.0), "%stamp%");
    let t = text_of(&sim, id);
    assert!(t.starts_with("Drawn by Other Office on "), "{t}");
    assert!(!t.contains('%'));
}

#[test]
fn replace_fonts_is_one_undo_step_and_the_prompt_lists_missing_fonts() {
    let mut sim = house();
    sim.app.cx.project.text_styles.styles[0].font = "Zzyzx Display".into();
    sim.app
        .cx
        .run_custom(crate::dialogs::find_replace::REPLACE_FONTS_PROMPT);
    assert!(crate::dialogs::find_replace::replace_fonts_open());
    let rows = crate::dialogs::find_replace::replace_fonts_rows();
    assert!(rows.iter().any(|r| r.font == "Zzyzx Display" && r.missing));
    crate::dialogs::find_replace::set_replacement("Zzyzx Display", "Arial", "Bold");
    let before = steps(&sim);
    sim.app
        .cx
        .run_custom(crate::dialogs::find_replace::REPLACE_FONTS_APPLY);
    assert_eq!(steps(&sim), before + 1);
    assert_eq!(sim.app.cx.project.text_styles.styles[0].font, "Arial");
    assert!(!crate::dialogs::find_replace::replace_fonts_open());
    sim.undo();
    assert_eq!(
        sim.app.cx.project.text_styles.styles[0].font,
        "Zzyzx Display"
    );
}

/// A 20' x 15' two-story box: windows north and east, a door south.
fn two_story() -> Sim {
    super::s21_layout_print::isolate_home();
    let mut sim = Sim::new();
    let cx = &mut sim.app.cx;
    cx.project.floors.push(Floor::new("2nd Floor", 109.0));
    let pts = [(0.0, 0.0), (240.0, 0.0), (240.0, 180.0), (0.0, 180.0)];
    for fl in 0..2 {
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            cx.project.add_wall(
                fl,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
    }
    let ids: Vec<Vec<Id>> = (0..2)
        .map(|fl| cx.project.floors[fl].walls.iter().map(|w| w.id).collect())
        .collect();
    cx.project
        .add_opening(0, ids[0][0], 120.0, OpeningKind::Door)
        .unwrap();
    cx.project
        .add_opening(0, ids[0][1], 90.0, OpeningKind::Window)
        .unwrap();
    for off in [60.0, 180.0] {
        cx.project
            .add_opening(0, ids[0][2], off, OpeningKind::Window)
            .unwrap();
    }
    cx.project
        .add_opening(1, ids[1][2], 120.0, OpeningKind::Window)
        .unwrap();
    cx.project
        .add_opening(1, ids[1][3], 90.0, OpeningKind::Window)
        .unwrap();
    cx.project.info.client_name = "Jane & Joe Smith".into();
    cx.project.info.designer = "Daniel".into();
    cx.project.info.company = "Daniel Allen Designs".into();
    cx.refresh();
    sim
}

fn number_after(line: &str, column: usize) -> f64 {
    line.split(',').nth(column).unwrap().parse().unwrap()
}

/// The sum of `<tag>` values inside every `<block>` element of `xml`.
fn sum_in(xml: &str, block: &str, tag: &str) -> f64 {
    let (open, close) = (format!("<{block}>"), format!("</{block}>"));
    let mut total = 0.0;
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i..];
        let j = after.find(&close).expect("closed");
        let body = &after[..j];
        let (t_open, t_close) = (format!("<{tag}>"), format!("</{tag}>"));
        if let Some(a) = body.find(&t_open) {
            let v = &body[a + t_open.len()..];
            total += v[..v.find(&t_close).unwrap()].parse::<f64>().unwrap();
        }
        rest = &after[j..];
    }
    total
}

#[test]
fn the_thermal_envelope_csv_and_rescheck_file_match_the_walls_and_windows() {
    let mut sim = two_story();
    let cx = &mut sim.app.cx;
    // What the geometry says: 8 ft walls, 20 ft north and south, 15 ft east
    // and west, on two floors.
    let south = 2.0 * 20.0 * 8.0;
    let east = 2.0 * 15.0 * 8.0;
    let windows = 5.0 * (36.0 * 60.0) / 144.0;
    let doors = (36.0 * 80.0) / 144.0;

    let csv = rescheck::thermal_csv(cx);
    let rows: Vec<&str> = csv.lines().collect();
    assert!(rows[0].starts_with("Floor Level,Component,Assembly,Direction,Area (sq ft)"));
    let total = |kind: &str, dir: &str| -> f64 {
        rows.iter()
            .find(|l| l.starts_with(&format!("Total,{kind},")) && l.split(',').nth(3) == Some(dir))
            .map_or(0.0, |l| number_after(l, 4))
    };
    assert!((total("Wall", "South") - south).abs() < 0.01);
    assert!((total("Wall", "North") - south).abs() < 0.01);
    assert!((total("Wall", "East") - east).abs() < 0.01);
    assert!((total("Wall", "West") - east).abs() < 0.01);
    let win_total: f64 = ["North", "East", "South", "West"]
        .iter()
        .map(|d| total("Window", d))
        .sum();
    assert!(
        (win_total - windows).abs() < 0.05,
        "{win_total} vs {windows}"
    );
    assert!((total("Door", "South") - doors).abs() < 0.01);
    // By floor level: eight walls, one row each.
    assert_eq!(
        rows.iter()
            .filter(|l| l.starts_with("1st Floor,Wall,"))
            .count(),
        4
    );
    assert_eq!(
        rows.iter()
            .filter(|l| l.starts_with("2nd Floor,Wall,"))
            .count(),
        4
    );
    // The windows face where their walls do.
    assert!(rows
        .iter()
        .any(|l| l.starts_with("2nd Floor,Window,") && l.contains(",West,")));

    // REScheck: grouped, then each wall and window on its own.
    let dir = std::env::temp_dir().join(format!("s94_rescheck_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let csv_path = dir.join("env.csv");
    rescheck::export_thermal_to(cx, &csv_path).unwrap();
    assert_eq!(std::fs::read_to_string(&csv_path).unwrap(), csv);
    let grouped = rescheck::rescheck_rxl(cx, &plan_docs::rescheck::Options::default());
    plan_docs::rescheck::well_formed(&grouped).unwrap();
    assert_eq!(grouped.matches("<Wall>").count(), 4);
    assert_eq!(grouped.matches("<Window>").count(), 3);
    assert!((sum_in(&grouped, "Wall", "GrossArea") - (2.0 * south + 2.0 * east)).abs() < 0.01);
    assert!((sum_in(&grouped, "Window", "GrossArea") - windows).abs() < 0.05);
    assert!((sum_in(&grouped, "Door", "GrossArea") - doors).abs() < 0.01);
    assert!(grouped.contains("<FrontFaces>South</FrontFaces>"));
    assert!(grouped.contains("<Name>Jane &amp; Joe Smith</Name>"));
    assert!(grouped.contains("<Company>Daniel Allen Designs</Company>"));
    let alone = rescheck::rescheck_rxl(
        cx,
        &plan_docs::rescheck::Options {
            group_walls: false,
            group_openings: false,
            ..plan_docs::rescheck::Options::default()
        },
    );
    plan_docs::rescheck::well_formed(&alone).unwrap();
    assert_eq!(alone.matches("<Wall>").count(), 8);
    assert_eq!(alone.matches("<Window>").count(), 5);
    assert!(
        (sum_in(&alone, "Wall", "GrossArea") - sum_in(&grouped, "Wall", "GrossArea")).abs() < 0.01
    );
    let rxl_path = dir.join("plan.rxl");
    rescheck::export_rescheck_to(cx, &rxl_path, &plan_docs::rescheck::Options::default()).unwrap();
    assert_eq!(std::fs::read_to_string(&rxl_path).unwrap(), grouped);
    std::fs::remove_dir_all(&dir).ok();

    // The menu commands open the dialog the choices live in.
    cx.run_custom(rescheck::EXPORT_RESCHECK);
    assert!(rescheck::dialog_open());
    rescheck::set_dialog_options(false, true);
    assert!(!rescheck::dialog_options().group_walls);
    rescheck::close_dialog();
}

#[test]
fn a_north_pointer_turns_the_directions_of_the_export() {
    let mut sim = two_story();
    // North is to the right of the plan.
    sim.app.cx.project.terrain = Some(serde_json::json!({ "terrain": { "north_angle": 90.0 } }));
    let csv = rescheck::thermal_csv(&sim.app.cx);
    let north: f64 = csv
        .lines()
        .find(|l| l.starts_with("Total,Wall,") && l.split(',').nth(3) == Some("North"))
        .map(|l| number_after(l, 4))
        .unwrap();
    // The east walls (15 ft, two floors) now face north.
    assert!((north - 2.0 * 15.0 * 8.0).abs() < 0.01, "{north}");
}
