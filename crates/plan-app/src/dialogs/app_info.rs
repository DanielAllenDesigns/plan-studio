//! Application-level menu commands that are not tied to a plan object: the
//! File menu's Open Recent Documents and View File Information, Tools >
//! Color Chooser and New Plan View, View > Refresh Display, Fill Window
//! Selected Objects, Status Bar, Toolbars and Enter Full Screen, and the Help
//! menu (the manual in `docs/manual`, the project page, System Information).
//!
//! Each command has an id; the menus send it with `Action::Custom(id)` and
//! `EditorContext::run_custom` ends up in [`run_command`]. Commands that need
//! the window (full screen, opening a file, framing the view) leave a request
//! that `main.rs` picks up once a frame.

use crate::dialogs::customize_toolbars::ToolbarDialog;
use crate::dialogs::{help, preferences};
use crate::editor::EditorContext;
use crate::toolbar;
use eframe::egui::{self, Align2};
use plan_core::geometry::Point;
use plan_core::SavedPlanView;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

pub const REFRESH: &str = "app.refresh_display";
pub const FULL_SCREEN: &str = "app.full_screen";
pub const FILE_INFO: &str = "app.file_info";
pub const COLOR_CHOOSER: &str = "app.color_chooser";
pub const SYSTEM_INFO: &str = "app.system_info";
pub const NEW_PLAN_VIEW: &str = "app.new_plan_view";
pub const TOGGLE_STATUS_BAR: &str = "app.toggle_status_bar";
pub const TOGGLE_TOOLBARS: &str = "app.toggle_toolbars";
pub const FILL_SELECTED: &str = "app.fill_selected";
pub const HELP: &str = "help.manual";
pub const HELP_TUTORIAL: &str = "help.tutorial";
pub const HELP_REFERENCE: &str = "help.reference";
pub const HELP_PROJECT: &str = "help.project";
pub const HELP_HOTKEYS: &str = "help.hotkeys";
/// Help > About Plan Studio.
pub const ABOUT: &str = "app.about";
/// Tools > Toolbars and Hotkeys > Customize Toolbars.
pub const CUSTOMIZE_TOOLBARS: &str = "app.customize_toolbars";
pub const PREFS_LIBRARY: &str = "prefs.library";
/// Ids of the recent-file rows: `recent.0` is the newest.
pub const RECENT: [&str; MAX_RECENT] = [
    "recent.0", "recent.1", "recent.2", "recent.3", "recent.4", "recent.5", "recent.6", "recent.7",
    "recent.8", "recent.9",
];
/// How many recent files are kept.
pub const MAX_RECENT: usize = 10;
/// The project page opened by Help > Plan Studio on GitHub.
pub const PROJECT_URL: &str = "https://github.com/DanielAllenDesigns/plan-studio";

/// What the window shell must do for a command.
#[derive(Default)]
struct Requests {
    open_file: Option<PathBuf>,
    full_screen: bool,
    fill_selected: bool,
}

#[derive(Default)]
struct State {
    requests: Requests,
    current_path: Option<PathBuf>,
    file_info: bool,
    color_chooser: Option<[u8; 3]>,
    system_info: bool,
    about: bool,
    /// The window icon has been sent to the window.
    icon_sent: bool,
    /// Customize Toolbars while it is open.
    toolbar_dialog: Option<super::customize_toolbars::ToolbarDialog>,
    /// Recent files in memory (loaded from the settings file on first use).
    recent: Option<Vec<PathBuf>>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

// ----- recent files -----

/// The `recent` key of the settings file.
const RECENT_KEY: &str = "recent_files";

/// The recent files stored in the settings file at `path`, newest first.
pub fn read_recent_at(path: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get(RECENT_KEY).cloned())
        .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .take(MAX_RECENT)
        .collect()
}

/// Writes the recent files to the settings file at `path`, keeping its other
/// keys.
pub fn write_recent_at(path: &Path, recent: &[PathBuf]) -> Result<(), String> {
    let mut v = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    let list: Vec<String> = recent
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    v[RECENT_KEY] = serde_json::json!(list);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// `list` with `file` moved (or added) to the front, at most
/// [`MAX_RECENT`] long.
pub fn with_recent(list: &[PathBuf], file: &Path) -> Vec<PathBuf> {
    let mut out = vec![file.to_path_buf()];
    out.extend(list.iter().filter(|p| p.as_path() != file).cloned());
    out.truncate(MAX_RECENT);
    out
}

/// The recent files, newest first (files that no longer exist are kept out).
pub fn recent_files() -> Vec<PathBuf> {
    let list = state(|s| {
        s.recent
            .get_or_insert_with(|| {
                #[cfg(test)]
                {
                    Vec::new()
                }
                #[cfg(not(test))]
                {
                    preferences::settings_path()
                        .map(|p| read_recent_at(&p))
                        .unwrap_or_default()
                }
            })
            .clone()
    });
    list.into_iter().filter(|p| p.exists()).collect()
}

/// Remembers a file that was opened or saved.
pub fn push_recent(file: &Path) {
    let list = state(|s| {
        let cur = s.recent.take().unwrap_or_else(|| {
            #[cfg(test)]
            {
                Vec::new()
            }
            #[cfg(not(test))]
            {
                preferences::settings_path()
                    .map(|p| read_recent_at(&p))
                    .unwrap_or_default()
            }
        });
        let next = with_recent(&cur, file);
        s.recent = Some(next.clone());
        next
    });
    #[cfg(not(test))]
    if let Some(p) = preferences::settings_path() {
        let _ = write_recent_at(&p, &list);
    }
    #[cfg(test)]
    let _ = list;
}

/// File > Open Recent Documents > Clear Menu: forgets every recent file.
pub fn clear_recent() {
    state(|s| s.recent = Some(Vec::new()));
    #[cfg(not(test))]
    if let Some(p) = preferences::settings_path() {
        let _ = write_recent_at(&p, &[]);
    }
}

/// The file the current plan was opened from or saved to (set by the shell
/// once a frame, for View File Information).
pub fn set_current_path(p: Option<PathBuf>) {
    state(|s| s.current_path = p);
}

/// A file a recent-file row asked to open; the shell opens it.
pub fn take_open_request() -> Option<PathBuf> {
    state(|s| s.requests.open_file.take())
}

/// Did Enter Full Screen ask for a toggle?
pub fn take_full_screen_request() -> bool {
    state(|s| std::mem::take(&mut s.requests.full_screen))
}

/// Did Fill Window Selected Objects ask for the view to frame the selection?
pub fn take_fill_selected_request() -> bool {
    state(|s| std::mem::take(&mut s.requests.fill_selected))
}

// ----- commands -----

/// Runs a command by id; false when the id is not one of ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    if let Some(i) = RECENT.iter().position(|r| *r == id) {
        match recent_files().get(i) {
            Some(p) => state(|s| s.requests.open_file = Some(p.clone())),
            None => cx.status = "That file is no longer in the recent list".into(),
        }
        return true;
    }
    match id {
        REFRESH => {
            cx.mark_dirty();
            cx.refresh();
            cx.status = "Display refreshed".into();
        }
        FULL_SCREEN => state(|s| s.requests.full_screen = true),
        FILE_INFO => state(|s| s.file_info = true),
        COLOR_CHOOSER => state(|s| {
            s.color_chooser.get_or_insert([200, 200, 200]);
        }),
        SYSTEM_INFO => state(|s| s.system_info = true),
        NEW_PLAN_VIEW => {
            new_plan_view(cx);
        }
        TOGGLE_STATUS_BAR => {
            let mut p = preferences::current();
            p.show_status_bar = !p.show_status_bar;
            preferences::set(p);
        }
        TOGGLE_TOOLBARS => {
            let mut p = preferences::current();
            p.show_toolbars = !p.show_toolbars;
            preferences::set(p);
        }
        FILL_SELECTED => state(|s| s.requests.fill_selected = true),
        HELP => open_help(help::INDEX_FILE, cx),
        HELP_TUTORIAL => open_help(help::TUTORIAL_FILE, cx),
        HELP_REFERENCE => open_help(help::INDEX_FILE, cx),
        HELP_HOTKEYS => open_help(help::HOTKEYS_FILE, cx),
        ABOUT => state(|s| s.about = true),
        CUSTOMIZE_TOOLBARS => state(|s| {
            s.toolbar_dialog.get_or_insert_with(|| {
                ToolbarDialog::new(&toolbar::config::current(), toolbar::config::active_view())
            });
        }),
        HELP_PROJECT => {
            cx.status = match open_external(PROJECT_URL) {
                Ok(()) => format!("Opened {PROJECT_URL}"),
                Err(e) => e,
            };
        }
        PREFS_LIBRARY => preferences::open(preferences::Page::Library),
        _ => return false,
    }
    true
}

/// Tools > New Plan View: a saved view named "Plan View N" that starts as a
/// copy of the active one, made the active view (one undo step).
pub fn new_plan_view(cx: &mut EditorContext) -> String {
    let base = cx.project.current_plan_view().cloned();
    let layer_set = base.as_ref().map_or_else(
        || cx.project.layer_sets.active.clone(),
        |v| v.layer_set.clone(),
    );
    let mut n = cx.project.plan_views.len() + 1;
    let name = loop {
        let name = format!("Plan View {n}");
        if cx.project.plan_view(&name).is_none() {
            break name;
        }
        n += 1;
    };
    cx.begin_change("New Plan View");
    let mut view = base.unwrap_or_else(|| SavedPlanView::new(name.clone(), layer_set));
    view.name = name.clone();
    cx.project.plan_views.push(view);
    cx.project.activate_plan_view(&name);
    cx.mark_dirty();
    cx.status = format!("Made {name}");
    name
}

/// The corner points that frame `items` (the Fill Window Selected Objects
/// rectangle), `None` for an empty selection.
pub fn selection_frame(cx: &EditorContext) -> Option<(Point, Point)> {
    crate::editor::transform::items_bounds(cx, &cx.selection.items)
}

// ----- help -----

/// The folder holding the manual: next to the program, under the working
/// folder, or in the source tree this build came from.
pub fn manual_dir() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        for a in exe.ancestors().skip(1).take(5) {
            candidates.push(a.join("docs").join("manual"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("docs").join("manual"));
    }
    candidates.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("docs")
            .join("manual"),
    );
    candidates
        .into_iter()
        .find(|d| d.join("00-index.md").exists())
}

/// Opens the in-app Help viewer on `file`; when this build carries no manual,
/// the files under `docs/manual` open in the system's viewer instead.
fn open_help(file: &str, cx: &mut EditorContext) {
    if help::index().chapters.is_empty() {
        open_manual(file, cx);
    } else {
        help::open(file);
        cx.status = "Help opened".into();
    }
}

fn open_manual(file: &str, cx: &mut EditorContext) {
    cx.status = match manual_dir() {
        Some(d) => {
            let target = d.join(file);
            match open_external(&target.to_string_lossy()) {
                Ok(()) => format!("Opened {}", target.display()),
                Err(e) => e,
            }
        }
        None => "The manual (docs/manual) was not found next to this program".into(),
    };
}

/// Opens a file or URL with the system's default application.
pub fn open_external(target: &str) -> Result<(), String> {
    let (program, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![target])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", "", target])
    } else {
        ("xdg-open", vec![target])
    };
    std::process::Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open {target}: {e}"))
}

// ----- information shown by the windows -----

/// The lines of View File Information.
pub fn file_info_lines(cx: &EditorContext, path: Option<&Path>) -> Vec<(String, String)> {
    let mut v = vec![("Plan".to_string(), cx.project.name.clone())];
    match path {
        Some(p) => {
            v.push(("File".into(), p.display().to_string()));
            if let Ok(meta) = std::fs::metadata(p) {
                v.push(("Size".into(), human_bytes(meta.len())));
                if let Some(secs) = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                {
                    v.push(("Saved".into(), crate::templates::format_time(secs)));
                }
            }
        }
        None => v.push(("File".into(), "Not saved yet".into())),
    }
    let p = &cx.project;
    let sum = |f: &dyn Fn(&plan_core::Floor) -> usize| p.floors.iter().map(f).sum::<usize>();
    v.push(("Floors".into(), p.floors.len().to_string()));
    v.push(("Walls".into(), sum(&|f| f.walls.len()).to_string()));
    v.push((
        "Doors and windows".into(),
        sum(&|f| f.openings.len()).to_string(),
    ));
    v.push(("Cabinets".into(), sum(&|f| f.cabinets.len()).to_string()));
    v.push((
        "Placed symbols".into(),
        sum(&|f| f.symbols.len()).to_string(),
    ));
    v.push(("CAD objects".into(), sum(&|f| f.cad.len()).to_string()));
    v.push(("Underlays".into(), sum(&|f| f.underlays.len()).to_string()));
    v.push(("Layers".into(), p.layers.layers.len().to_string()));
    v.push((
        "Painted objects".into(),
        p.object_materials.len().to_string(),
    ));
    v
}

fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["bytes", "KB", "MB", "GB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u + 1 < UNITS.len() {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} bytes")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

/// The lines of Help > System Information.
pub fn system_info_lines() -> Vec<(String, String)> {
    let dir = crate::paths::user_file("").map(|p| p.display().to_string());
    vec![
        (
            "Plan Studio".into(),
            format!("version {}", env!("CARGO_PKG_VERSION")),
        ),
        (
            "System".into(),
            format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        ),
        (
            "Processors".into(),
            std::thread::available_parallelism()
                .map_or_else(|_| "unknown".to_string(), |n| n.get().to_string()),
        ),
        (
            "Settings folder".into(),
            dir.unwrap_or_else(|| "none".into()),
        ),
        (
            "Manual".into(),
            manual_dir().map_or_else(|| "not found".into(), |d| d.display().to_string()),
        ),
    ]
}

// ----- windows -----

/// Draws the File Information, Color Chooser and System Information windows
/// that are open.
pub fn show_windows(ctx: &egui::Context, cx: &EditorContext) {
    // The window icon, once (Windows and Linux; the Mac app takes its icon
    // from the bundle).
    if state(|s| !std::mem::replace(&mut s.icon_sent, true)) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(std::sync::Arc::new(
            window_icon(),
        ))));
    }
    if state(|s| s.file_info) {
        let mut open = true;
        let path = state(|s| s.current_path.clone());
        egui::Window::new("File Information")
            .id(egui::Id::new("file_information"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                lines_grid(ui, "file_info_grid", &file_info_lines(cx, path.as_deref()))
            });
        if !open {
            state(|s| s.file_info = false);
        }
    }
    if state(|s| s.system_info) {
        let mut open = true;
        let lines = system_info_lines();
        egui::Window::new("System Information")
            .id(egui::Id::new("system_information"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                lines_grid(ui, "system_info_grid", &lines);
                if ui.button("Copy").clicked() {
                    let text: Vec<String> =
                        lines.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                    ctx.copy_text(text.join("\n"));
                }
            });
        if !open {
            state(|s| s.system_info = false);
        }
    }
    if let Some(mut c) = state(|s| s.color_chooser) {
        let mut open = true;
        egui::Window::new("Color Chooser")
            .id(egui::Id::new("color_chooser"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.color_edit_button_srgb(&mut c);
                let hex = format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
                ui.label(format!("{hex}   RGB {} {} {}", c[0], c[1], c[2]));
                if ui.button("Copy hex").clicked() {
                    ctx.copy_text(hex);
                }
            });
        state(|s| s.color_chooser = open.then_some(c));
    }
    help::show(ctx);
    show_about(ctx);
    show_toolbar_dialog(ctx);
}

/// Customize Toolbars, while it is open.
fn show_toolbar_dialog(ctx: &egui::Context) {
    let Some(mut dialog) = state(|s| s.toolbar_dialog.take()) else {
        return;
    };
    if dialog.show(ctx) == super::Outcome::Open {
        state(|s| s.toolbar_dialog = Some(dialog));
    }
}

// ----- About -----

/// The lines of the About window: (label, text).
pub fn about_lines() -> Vec<(String, String)> {
    vec![
        (
            "Version".into(),
            format!(
                "{} ({} {})",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        ),
        ("License".into(), "MIT License".into()),
        (
            "Copyright".into(),
            "Daniel Sievers / Daniel Allen Designs".into(),
        ),
        ("Project".into(), PROJECT_URL.into()),
    ]
}

/// The paragraph about Chief Architect that the About window and the README
/// both carry: what is read from the user's install and what is not shipped.
pub const CHIEF_NOTICE: &str =
    "Plan Studio is an independent open-source program. It is not made by, \
affiliated with or endorsed by Chief Architect, Inc. No Chief Architect code, assets or catalog \
content ships with it; Chief catalogs and templates are read in place from your own Chief install. \
Chief Architect is a trademark of its owner and is named here only to describe compatibility.";

fn show_about(ctx: &egui::Context) {
    if !state(|s| s.about) {
        return;
    }
    let mut open = true;
    egui::Window::new("About Plan Studio")
        .id(egui::Id::new("about_plan_studio"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::new(egui::include_image!(
                        "../../assets/icons/app/plan-studio.svg"
                    ))
                    .fit_to_exact_size(egui::Vec2::splat(72.0)),
                );
                ui.vertical(|ui| {
                    ui.heading("Plan Studio");
                    ui.label("A residential design CAD program for the floor plan, 3D, elevations and layout.");
                });
            });
            ui.separator();
            egui::Grid::new("about_grid")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for (k, v) in about_lines() {
                        ui.label(k.as_str());
                        if v.starts_with("https://") {
                            if ui.link(&v).clicked() {
                                let _ = open_external(&v);
                            }
                        } else {
                            ui.label(v);
                        }
                        ui.end_row();
                    }
                });
            ui.separator();
            ui.add(egui::Label::new(egui::RichText::new(CHIEF_NOTICE).weak()).wrap());
        });
    if !open {
        state(|s| s.about = false);
    }
}

// ----- the window icon -----

/// The 256 pixel app icon, for the window (`ViewportBuilder::with_icon`).
pub fn window_icon() -> egui::IconData {
    static PNG: &[u8] = include_bytes!("../../assets/icons/app/icon-256.png");
    match plan_library::image::png::decode(PNG) {
        Ok(img) => egui::IconData {
            rgba: img.rgba,
            width: img.width,
            height: img.height,
        },
        // The icon is a compiled-in asset checked by a test; an empty icon
        // makes the window system use its own.
        Err(_) => egui::IconData::default(),
    }
}

fn lines_grid(ui: &mut egui::Ui, id: &str, lines: &[(String, String)]) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            for (k, v) in lines {
                ui.label(k);
                ui.label(v);
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    #[test]
    fn recent_files_go_to_the_front_without_duplicates_and_are_capped() {
        let mut list: Vec<PathBuf> = Vec::new();
        for i in 0..12 {
            list = with_recent(&list, Path::new(&format!("/plans/p{i}.psplan")));
        }
        assert_eq!(list.len(), MAX_RECENT);
        assert_eq!(list[0], Path::new("/plans/p11.psplan"));
        let again = with_recent(&list, Path::new("/plans/p5.psplan"));
        assert_eq!(again[0], Path::new("/plans/p5.psplan"));
        assert_eq!(again.iter().filter(|p| p.ends_with("p5.psplan")).count(), 1);
        assert_eq!(again.len(), MAX_RECENT);
    }

    #[test]
    fn recent_files_round_trip_through_the_settings_file_keeping_other_keys() {
        let dir = std::env::temp_dir().join(format!("plan-studio-recent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        std::fs::write(&file, r#"{"theme":"low_glare"}"#).unwrap();
        let list = vec![
            PathBuf::from("/a/one.psplan"),
            PathBuf::from("/b/two.psplan"),
        ];
        write_recent_at(&file, &list).unwrap();
        assert_eq!(read_recent_at(&file), list);
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(v["theme"], "low_glare");
        assert!(read_recent_at(&dir.join("missing.json")).is_empty());
    }

    #[test]
    fn a_recent_row_asks_the_shell_to_open_the_file() {
        let dir = std::env::temp_dir().join(format!("plan-studio-recent2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("house.psplan");
        std::fs::write(&f, "{}").unwrap();
        push_recent(&f);
        let mut cx = cx();
        assert_eq!(recent_files(), vec![f.clone()]);
        assert!(run_command(&mut cx, "recent.0"));
        assert_eq!(take_open_request(), Some(f));
        assert!(take_open_request().is_none());
        // A row past the end explains itself.
        assert!(run_command(&mut cx, "recent.9"));
        assert!(cx.status.contains("no longer"));
        assert!(!run_command(&mut cx, "recent.10"));
    }

    #[test]
    fn new_plan_view_copies_the_active_view_as_one_undo_step() {
        let mut cx = cx();
        let before = cx.project.plan_views.len();
        let name = new_plan_view(&mut cx);
        assert_eq!(cx.project.plan_views.len(), before + 1);
        assert_eq!(cx.project.active_plan_view, name);
        assert!(cx.project.plan_view(&name).is_some());
        assert_eq!(cx.undo_label(), Some("New Plan View"));
        let second = new_plan_view(&mut cx);
        assert_ne!(name, second);
        cx.undo();
        cx.undo();
        assert_eq!(cx.project.plan_views.len(), before);
    }

    #[test]
    fn requests_and_toggles_reach_the_shell() {
        let mut cx = cx();
        assert!(run_command(&mut cx, FULL_SCREEN));
        assert!(take_full_screen_request());
        assert!(!take_full_screen_request());
        assert!(run_command(&mut cx, FILL_SELECTED));
        assert!(take_fill_selected_request());
        let shown = preferences::current().show_toolbars;
        assert!(run_command(&mut cx, TOGGLE_TOOLBARS));
        assert_eq!(preferences::current().show_toolbars, !shown);
        assert!(run_command(&mut cx, TOGGLE_TOOLBARS));
        assert_eq!(preferences::current().show_toolbars, shown);
        assert!(run_command(&mut cx, REFRESH));
        assert!(run_command(&mut cx, PREFS_LIBRARY));
        assert_eq!(preferences::page(), preferences::Page::Library);
        assert!(!run_command(&mut cx, "app.nope"));
    }

    #[test]
    fn file_information_describes_the_plan_and_the_file() {
        let mut cx = cx();
        cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            plan_core::WallKind::Exterior,
        );
        let unsaved = file_info_lines(&cx, None);
        assert!(unsaved
            .iter()
            .any(|(k, v)| k == "File" && v == "Not saved yet"));
        assert!(unsaved.iter().any(|(k, v)| k == "Walls" && v == "1"));
        let dir = std::env::temp_dir().join(format!("plan-studio-info-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.psplan");
        std::fs::write(&f, vec![b'x'; 2048]).unwrap();
        let saved = file_info_lines(&cx, Some(&f));
        assert!(saved.iter().any(|(k, v)| k == "Size" && v == "2.0 KB"));
        assert_eq!(human_bytes(10), "10 bytes");
        assert!(system_info_lines().iter().any(|(k, _)| k == "Plan Studio"));
    }

    #[test]
    fn the_windows_draw_headlessly() {
        let mut cx = cx();
        for id in [FILE_INFO, SYSTEM_INFO, COLOR_CHOOSER] {
            assert!(run_command(&mut cx, id));
        }
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_windows(ctx, &cx));
    }

    #[test]
    fn the_help_commands_open_the_in_app_viewer() {
        let mut cx = cx();
        for (id, file) in [
            (HELP, help::INDEX_FILE),
            (HELP_TUTORIAL, help::TUTORIAL_FILE),
            (HELP_REFERENCE, help::INDEX_FILE),
            (HELP_HOTKEYS, help::HOTKEYS_FILE),
        ] {
            assert!(run_command(&mut cx, id));
            assert!(help::is_open(), "{id} opens the viewer");
            let ctx = egui::Context::default();
            let _ = ctx.run(egui::RawInput::default(), |ctx| show_windows(ctx, &cx));
            assert!(cx.status.contains("Help"), "{}", cx.status);
            // The viewer is on the chapter the command names.
            let idx = help::index();
            assert!(idx.chapter_by_file(file).is_some());
        }
    }

    #[test]
    fn about_carries_the_version_license_notice_and_project_link() {
        let lines = about_lines();
        let get = |k: &str| {
            lines
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        assert!(get("Version").starts_with(env!("CARGO_PKG_VERSION")));
        assert_eq!(get("License"), "MIT License");
        assert_eq!(get("Project"), PROJECT_URL);
        assert!(get("Project").starts_with("https://github.com/"));
        assert!(CHIEF_NOTICE.contains("No Chief Architect code, assets or catalog content ships"));
        assert!(CHIEF_NOTICE.contains("independent"));
        // The window opens from its command and draws.
        let mut cx = cx();
        assert!(run_command(&mut cx, ABOUT));
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_windows(ctx, &cx));
        assert!(state(|s| s.about));
    }

    #[test]
    fn customize_toolbars_opens_from_its_command_and_closes_on_cancel() {
        let mut cx = cx();
        assert!(run_command(&mut cx, CUSTOMIZE_TOOLBARS));
        assert!(state(|s| s.toolbar_dialog.is_some()));
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_windows(ctx, &cx));
        assert!(state(|s| s.toolbar_dialog.is_some()), "still open");
        state(|s| s.toolbar_dialog = None);
    }

    #[test]
    fn the_app_icon_files_exist_and_are_valid_pngs() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons/app");
        for px in [16_u32, 32, 64, 128, 256, 512, 1024] {
            let bytes = std::fs::read(dir.join(format!("icon-{px}.png")))
                .unwrap_or_else(|e| panic!("icon-{px}.png: {e}"));
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "icon-{px}.png signature");
            let img = plan_library::image::png::decode(&bytes)
                .unwrap_or_else(|e| panic!("icon-{px}.png does not decode: {e:?}"));
            assert_eq!((img.width, img.height), (px, px));
            // A rounded tile: the corner is transparent, the middle is not.
            assert_eq!(img.rgba[3], 0, "corner of icon-{px}.png is clear");
            let mid = ((px / 2 * px + px / 2) * 4 + 3) as usize;
            assert_eq!(img.rgba[mid], 255, "middle of icon-{px}.png is solid");
        }
        let svg = std::fs::read_to_string(dir.join("plan-studio.svg")).unwrap();
        assert!(svg.contains("<svg") && svg.contains("viewBox=\"0 0 1024 1024\""));
        // The macOS icon, when the generator could run iconutil.
        if let Ok(icns) = std::fs::read(dir.join("AppIcon.icns")) {
            assert_eq!(&icns[..4], b"icns");
        }
        let icon = window_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn the_manual_is_found_in_the_source_tree() {
        let d = manual_dir().expect("docs/manual exists in the repository");
        assert!(d.join("01-getting-started.md").exists());
    }
}
