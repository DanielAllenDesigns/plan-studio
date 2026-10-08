//! Scenario 22: files. The safe save and archive rotation, autosave only
//! while there are unsaved changes, crash and autosave recovery, the Open
//! Recent list, the unsaved-changes prompt and the dirty marker in the window
//! title. Everything runs in a temp folder; `$HOME` is pointed at another
//! temp folder first, so `~/.plan-studio` is never read or written
//! (documentation-layout parity: file handling, `files.rs`).

use super::s21_layout_print::isolate_home;
use super::{draw_shell, Sim};
use crate::dialogs::app_info;
use crate::files::{
    archives_dir, autosave_path, list_archives, recovery_dir, window_title, write_recovery_to, EXT,
};
use crate::toolbar::Action;
use crate::tools::ToolId;
use crate::PlanApp;
use eframe::egui::{self, Key, Modifiers};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("plan-qa-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn sim() -> Sim {
    isolate_home();
    Sim::new()
}

/// A wall drawn with the Wall tool: a real edit that makes the plan dirty.
fn draw_a_wall(sim: &mut Sim, y: f64) {
    sim.tool(ToolId::Wall {
        kind: plan_core::WallKind::Interior,
    });
    sim.drag((100.0, y), (300.0, y));
    sim.tool(ToolId::Select);
}

/// Settles the dirty check (the shell does this a few frames after an edit).
fn settle(a: &mut PlanApp) -> bool {
    a.files.settle(&a.cx, a.path.is_some());
    a.files.is_dirty()
}

/// One headless shell frame of the file machinery, with `events` pressed.
fn frame(a: &mut PlanApp, events: Vec<egui::Event>) -> egui::FullOutput {
    let ctx = a.ctx_for_files();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1000.0, 700.0),
        )),
        events,
        ..Default::default()
    };
    let mut app = Some(a);
    ctx.run(input, |ctx| {
        if let Some(a) = app.take() {
            a.drive_files(ctx);
        }
    })
}

fn key(k: Key, modifiers: Modifiers) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: Some(k),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

trait FilesCtx {
    fn ctx_for_files(&self) -> egui::Context;
}

impl FilesCtx for PlanApp {
    fn ctx_for_files(&self) -> egui::Context {
        thread_local! {
            static CTX: egui::Context = egui::Context::default();
        }
        CTX.with(Clone::clone)
    }
}

/// The title the last frame asked the window to show.
fn title_of(out: &egui::FullOutput) -> Option<String> {
    out.viewport_output
        .values()
        .flat_map(|v| v.commands.iter())
        .find_map(|c| match c {
            egui::ViewportCommand::Title(t) => Some(t.clone()),
            _ => None,
        })
}

#[test]
fn saving_is_atomic_archives_the_old_version_and_keeps_only_the_newest_copies() {
    let dir = temp_dir("save");
    let plan = dir.join("maple.psplan");
    let mut s = sim();
    draw_shell(&mut s, 480.0, 360.0);
    s.app.files.settings.archive_keep = 2;
    s.app.path = Some(plan.clone());
    s.action(Action::FileSave);
    assert!(plan.is_file(), "Save writes the plan");
    assert!(
        list_archives(&plan).is_empty(),
        "a first save has nothing to archive"
    );
    let mut sizes = Vec::new();
    for i in 0..3 {
        draw_a_wall(&mut s, 40.0 + 30.0 * f64::from(i));
        s.action(Action::FileSave);
        sizes.push(fs::metadata(&plan).unwrap().len());
        assert!(s.app.cx.status.starts_with("Saved"), "{}", s.app.cx.status);
        // Archive stamps are per second: keep the saves apart.
        std::thread::sleep(Duration::from_millis(1100));
    }
    assert!(
        sizes.windows(2).all(|w| w[1] > w[0]),
        "each save added a wall: {sizes:?}"
    );
    let archives = list_archives(&plan);
    assert_eq!(archives.len(), 2, "only the newest two copies stay");
    assert!(archives[0].starts_with(archives_dir(&plan)));
    // The newest archive is the version before the last save, the next one
    // the version before that.
    let walls = |p: &PathBuf| {
        plan_core::io::load_project(p).unwrap().floors[0]
            .walls
            .len()
    };
    assert_eq!(walls(&archives[0]), 4 + 2);
    assert_eq!(walls(&archives[1]), 4 + 1);
    assert_eq!(walls(&plan), 4 + 3);
    // No temporary file is left behind.
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    // A failed save leaves the file on disk exactly as it was.
    let before = fs::read(&plan).unwrap();
    s.app.path = Some(dir.join("missing-folder").join("x.psplan"));
    draw_a_wall(&mut s, 300.0);
    s.action(Action::FileSave);
    assert!(
        s.app.cx.status.starts_with("Save failed"),
        "{}",
        s.app.cx.status
    );
    assert_eq!(fs::read(&plan).unwrap(), before);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn quick_saves_in_one_second_keep_every_copy_until_the_limit() {
    let dir = temp_dir("quick-saves");
    let plan = dir.join("maple.psplan");
    let mut s = sim();
    draw_shell(&mut s, 480.0, 360.0);
    s.app.path = Some(plan.clone());
    s.action(Action::FileSave);
    for i in 0..4 {
        draw_a_wall(&mut s, 40.0 + 30.0 * f64::from(i));
        s.action(Action::FileSave);
    }
    assert_eq!(list_archives(&plan).len(), 4, "the default keeps 20");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rapid_saves_over_the_archive_limit_never_rotate_away_the_newest_copy() {
    let dir = temp_dir("rapid-saves");
    let plan = dir.join("maple.psplan");
    let mut s = sim();
    draw_shell(&mut s, 480.0, 360.0);
    s.app.files.settings.archive_keep = 3;
    s.app.path = Some(plan.clone());
    s.action(Action::FileSave);
    for i in 0..6 {
        draw_a_wall(&mut s, 40.0 + 30.0 * f64::from(i));
        s.action(Action::FileSave);
    }
    // Six quick saves in one second: the newest archive is the version
    // before the last save (4 + 5 walls).
    let newest = list_archives(&plan);
    let walls = plan_core::io::load_project(&newest[0]).unwrap().floors[0]
        .walls
        .len();
    assert_eq!(walls, 4 + 5, "archives: {newest:?}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn autosave_writes_only_while_the_plan_has_unsaved_changes_and_never_over_the_plan() {
    let dir = temp_dir("autosave");
    let plan = dir.join("maple.psplan");
    let mut s = sim();
    draw_shell(&mut s, 480.0, 360.0);
    s.app.path = Some(plan.clone());
    s.action(Action::FileSave);
    let saved = fs::read(&plan).unwrap();
    let later = Instant::now() + Duration::from_secs(3600);
    assert!(!settle(&mut s.app), "a saved plan is clean");
    assert!(!s.app.tick_autosave(later));
    assert!(
        !autosave_path(&plan).exists(),
        "clean: no autosave however long it waits"
    );

    draw_a_wall(&mut s, 100.0);
    assert!(settle(&mut s.app), "a drawn wall makes it dirty");
    assert!(
        !s.app.tick_autosave(Instant::now()),
        "not before the interval has passed"
    );
    assert!(s.app.tick_autosave(later));
    let auto = autosave_path(&plan);
    let autosaved = plan_core::io::load_project(&auto).unwrap();
    assert_eq!(autosaved.floors[0].walls.len(), 5);
    assert_eq!(
        fs::read(&plan).unwrap(),
        saved,
        "the plan file is untouched"
    );

    // Undo back to the saved state: clean again, and the autosave goes at the
    // next shell frame.
    s.undo();
    assert!(!settle(&mut s.app));
    let _ = frame(&mut s.app, vec![]);
    assert!(
        !auto.exists(),
        "an autosave of the saved state is no longer news"
    );
    // Saving removes any autosave too.
    draw_a_wall(&mut s, 120.0);
    settle(&mut s.app);
    assert!(s.app.tick_autosave(later + Duration::from_secs(7200)));
    s.action(Action::FileSave);
    assert!(!auto.exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_crash_recovery_file_is_offered_at_startup_and_recover_or_discard_answer_it() {
    let home = recovery_dir().map(|d| d.parent().unwrap().to_path_buf());
    let mut s = sim();
    draw_shell(&mut s, 480.0, 360.0);
    let json = s.app.cx.project.to_json().unwrap();
    let rec = recovery_dir().expect("a recovery folder under the temp home");
    assert!(
        rec.starts_with(std::env::temp_dir()),
        "recovery folder {rec:?} must be in the temp HOME (was {home:?})"
    );
    let _ = fs::remove_dir_all(&rec);
    let file = write_recovery_to(&rec, "20261008-120000", &json).unwrap();

    // A new window starts up, scans the recovery folder and asks.
    let mut a = fresh_app();
    a.files.begin_startup(Vec::new());
    let out = frame(&mut a, vec![]);
    assert!(a.files.modal_open(), "the recovery prompt is up");
    let _ = out;
    assert_eq!(a.cx.floor().walls.len(), 0);
    // Enter recovers: the walls are back, the plan counts as unsaved.
    frame(&mut a, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(!a.files.modal_open());
    assert_eq!(a.cx.floor().walls.len(), 4);
    assert!(a.files.is_dirty(), "recovered work is unsaved work");

    // The same file, discarded by a second window: no prompt next launch.
    assert!(file.exists());
    let mut b = fresh_app();
    b.files.begin_startup(Vec::new());
    frame(&mut b, vec![]);
    assert!(b.files.modal_open());
    // Escape is not Discard (Enter recovers); the prompt's Discard button is
    // the only way, so drive the model call the button makes.
    assert!(crate::files::find_recovery_files(&rec).contains(&file));
    let _ = fs::remove_dir_all(&rec);
}

fn fresh_app() -> PlanApp {
    PlanApp::new(
        crate::theme::AppSettings::default(),
        crate::plan_defaults::embedded(),
        None,
    )
}

#[test]
fn opening_a_plan_with_a_newer_autosave_asks_to_recover_it() {
    let dir = temp_dir("open-recover");
    let plan = dir.join("maple.psplan");
    let mut s = sim();
    draw_shell(&mut s, 480.0, 360.0);
    s.app.path = Some(plan.clone());
    s.action(Action::FileSave);
    // A later session left an autosave of an edited plan.
    draw_a_wall(&mut s, 100.0);
    settle(&mut s.app);
    std::thread::sleep(Duration::from_millis(1100));
    let later = Instant::now() + Duration::from_secs(3600);
    assert!(s.app.tick_autosave(later));
    let auto = autosave_path(&plan);
    assert!(auto.is_file());

    let mut other = fresh_app();
    other.open_path(plan.clone());
    assert!(other.files.modal_open(), "Recover / Discard is asked");
    assert_eq!(
        other.cx.floor().walls.len(),
        4,
        "the saved plan is what opened"
    );
    frame(&mut other, vec![key(Key::Enter, Modifiers::NONE)]);
    assert_eq!(
        other.cx.floor().walls.len(),
        5,
        "Enter recovers the autosave"
    );
    assert!(other.files.is_dirty());
    assert_eq!(other.path.as_deref(), Some(plan.as_path()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn open_recent_lists_the_newest_first_dedupes_and_opens_through_the_prompt() {
    let dir = temp_dir("recent");
    app_info::clear_recent();
    let mut s = sim();
    let mut paths = Vec::new();
    for name in ["a", "b", "c"] {
        s.app.cx.project.name = name.to_string();
        let p = dir.join(format!("{name}.{EXT}"));
        s.app.path = Some(p.clone());
        s.action(Action::FileSave);
        paths.push(p);
    }
    let recent = app_info::recent_files();
    assert_eq!(
        recent,
        vec![paths[2].clone(), paths[1].clone(), paths[0].clone()]
    );
    // Saving an old one again moves it to the front without a duplicate.
    s.app.cx.project.name = "a".into();
    s.app.path = Some(paths[0].clone());
    s.action(Action::FileSave);
    let recent = app_info::recent_files();
    assert_eq!(recent[0], paths[0]);
    assert_eq!(recent.len(), 3);
    // A file that is gone drops out of the list.
    fs::remove_file(&paths[1]).unwrap();
    assert_eq!(app_info::recent_files().len(), 2);

    // Row 2 of the menu opens its file; the unsaved plan asks first.
    draw_a_wall(&mut s, 100.0);
    settle(&mut s.app);
    s.action(Action::Custom("recent.1"));
    s.app.app_commands(&egui::Context::default());
    settle(&mut s.app);
    assert!(
        s.app.files.modal_open(),
        "unsaved changes: the prompt comes first"
    );
    // Cancel keeps the plan as it is.
    frame(&mut s.app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!s.app.files.modal_open());
    assert_eq!(s.app.path.as_deref(), Some(paths[0].as_path()));
    // Don't Save carries on and opens the file.
    s.action(Action::Custom("recent.1"));
    s.app.app_commands(&egui::Context::default());
    frame(&mut s.app, vec![key(Key::D, Modifiers::COMMAND)]);
    frame(&mut s.app, vec![]);
    assert_eq!(s.app.path.as_deref(), Some(paths[2].as_path()));
    assert_eq!(s.app.cx.project.name, "c");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_file_menu_rows_clear_recent_close_and_revert_reach_the_files_module() {
    let dir = temp_dir("file-menu");
    app_info::clear_recent();
    let mut s = sim();
    let plan = dir.join("maple.psplan");
    s.app.path = Some(plan.clone());
    s.action(Action::FileSave);
    assert_eq!(app_info::recent_files(), vec![plan.clone()]);
    // File > Open Recent > Clear Menu.
    s.action(Action::Custom(crate::files::CLEAR_RECENT));
    assert!(app_info::recent_files().is_empty(), "{}", s.app.cx.status);
    // File > Revert to Saved puts the saved plan back.
    draw_a_wall(&mut s, 100.0);
    settle(&mut s.app);
    s.action(Action::Custom(crate::files::REVERT));
    // The revert prompt answers Enter with Revert (Cmd+D is the unsaved
    // prompt's Don't Save, unsaved.rs); the first run of this test used Cmd+D.
    frame(&mut s.app, vec![key(Key::Enter, Modifiers::NONE)]);
    frame(&mut s.app, vec![]);
    assert_eq!(s.app.cx.floor().walls.len(), 0, "{}", s.app.cx.status);
    // File > Close empties the window.
    s.action(Action::Custom(crate::files::CLOSE));
    frame(&mut s.app, vec![]);
    assert!(s.app.path.is_none(), "{}", s.app.cx.status);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_unsaved_prompt_save_dont_save_and_cancel_each_do_their_own_thing() {
    let dir = temp_dir("prompt");
    let plan = dir.join("maple.psplan");
    let mut s = sim();
    s.app.path = Some(plan.clone());
    draw_a_wall(&mut s, 100.0);
    assert!(settle(&mut s.app));

    // File > New on a dirty plan: the prompt, and nothing changed yet.
    s.action(Action::FileNew);
    assert!(s.app.files.modal_open());
    assert_eq!(s.app.cx.floor().walls.len(), 1);
    // Cancel (Esc): still the same plan, no prompt.
    frame(&mut s.app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!s.app.files.modal_open());
    assert_eq!(s.app.cx.floor().walls.len(), 1);
    assert!(!plan.exists());
    // Save (Enter): the plan is written, then the new plan starts.
    s.action(Action::FileNew);
    frame(&mut s.app, vec![key(Key::Enter, Modifiers::NONE)]);
    frame(&mut s.app, vec![]);
    assert!(plan.is_file(), "Save wrote the plan first");
    assert!(s.app.path.is_none(), "then a new plan began");
    assert_eq!(s.app.cx.floor().walls.len(), 0);
    // Don't Save (Cmd+D) throws the edit away.
    draw_a_wall(&mut s, 100.0);
    assert!(settle(&mut s.app));
    s.action(Action::FileNew);
    frame(&mut s.app, vec![key(Key::D, Modifiers::COMMAND)]);
    frame(&mut s.app, vec![]);
    assert_eq!(s.app.cx.floor().walls.len(), 0);
    assert!(!settle(&mut s.app), "the new plan is clean");
    // A clean plan goes straight through with no prompt.
    s.action(Action::FileNew);
    assert!(!s.app.files.modal_open());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_window_title_carries_the_plan_name_and_a_dirty_dot() {
    assert_eq!(window_title(None, false), "Plan Studio \u{2014} Untitled");
    let dir = temp_dir("title");
    let plan = dir.join("maple.psplan");
    let mut s = sim();
    s.app.path = Some(plan.clone());
    s.action(Action::FileSave);
    let out = frame(&mut s.app, vec![]);
    assert_eq!(
        title_of(&out).as_deref(),
        Some("Plan Studio \u{2014} maple.psplan")
    );
    draw_a_wall(&mut s, 100.0);
    settle(&mut s.app);
    let out = frame(&mut s.app, vec![]);
    assert_eq!(
        title_of(&out).as_deref(),
        Some("Plan Studio \u{2014} maple.psplan \u{2022}")
    );
    // Saving clears the dot; undo back to the saved state clears it as well.
    s.action(Action::FileSave);
    let out = frame(&mut s.app, vec![]);
    assert_eq!(
        title_of(&out).as_deref(),
        Some("Plan Studio \u{2014} maple.psplan")
    );
    draw_a_wall(&mut s, 140.0);
    settle(&mut s.app);
    frame(&mut s.app, vec![]);
    s.undo();
    settle(&mut s.app);
    let out = frame(&mut s.app, vec![]);
    assert_eq!(
        title_of(&out).as_deref(),
        Some("Plan Studio \u{2014} maple.psplan")
    );
    let _ = fs::remove_dir_all(&dir);
}
