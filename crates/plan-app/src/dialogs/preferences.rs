//! Edit > Preferences... (Cmd+,): Chief's Preferences dialog, one window with a
//! page list. Pages: Appearance (canvas colors, theme, brightness, icon size,
//! icon halo), Colors (selection color), Text (interface text size, installed
//! fonts, the default text styles), Library Browser (the Chief catalog switch
//! and folder, thumbnail size, search options), Render (ray-trace defaults,
//! preview quality, shadows and occlusion), Materials List, Reset Options
//! (toolbars, dialog sizes, "don't ask again" messages, side windows),
//! Folders (Chief's data folders, each with an "exists" mark), Edit (rotate
//! and resize about, marquee, snap switches), Behaviors (Edit Type, Replicate
//! dialog, camera steps), Snap Properties (every snap kind, sensitivity,
//! angles), Architectural (cabinets, auto rebuild), CAD (arc centers, end
//! caps, line weights), General Plan Defaults (a link) and Unit Conversions.
//!
//! The choices live in `~/.plan-studio/preferences.json` ([`pages::PrefsFile`]:
//! a version, the general [`Preferences`] and one key per page, all with serde
//! defaults); a first run reads the general choices from the older
//! `"preferences"` key of `settings.json`. The canvas theme and the brightness
//! stay at the top level of `settings.json` where `theme::AppSettings` keeps
//! them, and the Chief catalog switch under `"chief_catalogs"`. Every change
//! applies at once and is saved when the mouse is released.

use crate::dialogs::camera::{SamplesPreset, SizePreset};
use crate::editor::{placed, EditorContext};
use crate::theme::{AppSettings, CanvasTheme, BRIGHTNESS_MAX, BRIGHTNESS_MIN};
use crate::toolbar::Action;
use crate::tools::library::chief::{self, ChiefSettings};
use eframe::egui::{self, Align2};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

pub mod pages;
pub mod ui;

/// The key in `settings.json`.
pub const SETTINGS_KEY: &str = "preferences";
/// Command id of Edit > Preferences... (run with `EditorContext::run_custom`).
pub const OPEN: &str = "prefs.open";
/// Smallest and largest text size, percent.
pub const TEXT_SIZE_MIN: u32 = 80;
pub const TEXT_SIZE_MAX: u32 = 150;

fn yes() -> bool {
    true
}

fn pct() -> u32 {
    100
}

fn width() -> u32 {
    1280
}

fn height() -> u32 {
    960
}

fn samples() -> u32 {
    64
}

fn latitude() -> f64 {
    33.75
}

fn fit_tolerance() -> f64 {
    crate::tools::cabinet::FIT_TOLERANCE
}

/// What the Preferences dialog keeps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Appearance: size of the interface text, percent of normal.
    #[serde(default = "pct")]
    pub text_size_pct: u32,
    /// Appearance: a light plate behind the toolbar icons.
    pub icon_halo: bool,
    /// Colors: the selection highlight (None: the canvas theme's).
    pub selection_color: Option<[u8; 3]>,
    /// Render: the Ray Trace dialog's starting image size.
    #[serde(default = "width")]
    pub render_width: u32,
    #[serde(default = "height")]
    pub render_height: u32,
    /// Render: samples per pixel.
    #[serde(default = "samples")]
    pub render_samples: u32,
    /// Render: start in the Clay technique.
    pub render_clay: bool,
    /// Render: latitude of the sun, degrees north.
    #[serde(default = "latitude")]
    pub render_latitude: f64,
    /// View: show the status bar.
    #[serde(default = "yes")]
    pub show_status_bar: bool,
    /// View: show the toolbars.
    #[serde(default = "yes")]
    pub show_toolbars: bool,
    /// Architectural: base cabinets that touch share one countertop.
    #[serde(default = "yes")]
    pub auto_join_countertops: bool,
    /// Architectural: a cabinet dragged into a gap close to its width takes
    /// the gap's width.
    #[serde(default = "yes")]
    pub fit_cabinets_to_gap: bool,
    /// Architectural: how far (inches) a gap may differ from a cabinet's
    /// width and still be filled.
    #[serde(default = "fit_tolerance")]
    pub fit_gap_tolerance: f64,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            text_size_pct: pct(),
            icon_halo: false,
            selection_color: None,
            render_width: width(),
            render_height: height(),
            render_samples: samples(),
            render_clay: false,
            render_latitude: latitude(),
            show_status_bar: true,
            show_toolbars: true,
            auto_join_countertops: true,
            fit_cabinets_to_gap: true,
            fit_gap_tolerance: fit_tolerance(),
        }
    }
}

impl Preferences {
    /// The text size as an egui zoom factor.
    pub fn zoom(&self) -> f32 {
        self.text_size_pct.clamp(TEXT_SIZE_MIN, TEXT_SIZE_MAX) as f32 / 100.0
    }
}

// ----- the settings file -----

/// `~/.plan-studio/settings.json`.
pub fn settings_path() -> Option<PathBuf> {
    crate::paths::user_file("settings.json")
}

/// The `preferences` key of the settings file at `path`; `None` when the file
/// or the key is missing or unreadable.
pub fn read_at(path: &Path) -> Option<Preferences> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    serde_json::from_value(v.get(SETTINGS_KEY)?.clone()).ok()
}

/// Writes the `preferences` key of the settings file at `path`, keeping every
/// other key (the older layout; the live file is `preferences.json`).
#[cfg(test)]
pub fn write_at(path: &Path, p: &Preferences) -> Result<(), String> {
    let mut v = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    v[SETTINGS_KEY] = serde_json::to_value(p).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

// ----- the live copy -----

/// The pages of the dialog, in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Appearance,
    Colors,
    /// Chief's Text page (this build's Fonts page).
    Fonts,
    /// Chief's Library Browser page.
    Library,
    Render,
    MaterialsList,
    ResetOptions,
    Folders,
    Edit,
    Behaviors,
    /// Chief's Snap Properties page.
    Snaps,
    Architectural,
    Cad,
    PlanDefaults,
    UnitConversions,
}

impl Page {
    pub const ALL: [Page; 15] = [
        Page::Appearance,
        Page::Colors,
        Page::Fonts,
        Page::Library,
        Page::Render,
        Page::MaterialsList,
        Page::ResetOptions,
        Page::Folders,
        Page::Edit,
        Page::Behaviors,
        Page::Snaps,
        Page::Architectural,
        Page::Cad,
        Page::PlanDefaults,
        Page::UnitConversions,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Page::Appearance => "Appearance",
            Page::Colors => "Colors",
            Page::Fonts => "Text",
            Page::Library => "Library Browser",
            Page::Render => "Render",
            Page::MaterialsList => "Materials List",
            Page::ResetOptions => "Reset Options",
            Page::Folders => "Folders",
            Page::Edit => "Edit",
            Page::Behaviors => "Behaviors",
            Page::Snaps => "Snap Properties",
            Page::Architectural => "Architectural",
            Page::Cad => "CAD",
            Page::PlanDefaults => "General Plan Defaults",
            Page::UnitConversions => "Unit Conversions",
        }
    }
}

struct Live {
    prefs: Option<Preferences>,
    /// The other pages (`preferences.json`).
    pages: Option<pages::PagePrefs>,
    open: bool,
    page: Page,
    /// A change not yet written to the settings file.
    dirty: bool,
    /// The text size last handed to egui.
    applied_zoom: f32,
    /// Chief catalog settings being edited on the Library page.
    chief: Option<ChiefSettings>,
    chief_folder_text: String,
    note: String,
    /// The editing defaults of the saved pages have been laid over the
    /// plan's (once per run).
    editing_applied: bool,
    /// The converter's input on the Unit Conversions page.
    converter: String,
    /// The Reset Options button waiting for a second click.
    reset_pending: Option<ui::Reset>,
    reset_note: String,
}

impl Default for Live {
    fn default() -> Self {
        Self {
            prefs: None,
            pages: None,
            open: false,
            page: Page::Appearance,
            dirty: false,
            applied_zoom: 1.0,
            chief: None,
            chief_folder_text: String::new(),
            note: String::new(),
            editing_applied: false,
            converter: String::new(),
            reset_pending: None,
            reset_note: String::new(),
        }
    }
}

thread_local! {
    static LIVE: RefCell<Live> = RefCell::new(Live::default());
    static HALO: Cell<bool> = const { Cell::new(false) };
}

fn live<R>(f: impl FnOnce(&mut Live) -> R) -> R {
    LIVE.with(|l| f(&mut l.borrow_mut()))
}

/// The preferences in force (read from the settings file on first use).
pub fn current() -> Preferences {
    live(|l| {
        l.prefs
            .get_or_insert_with(|| {
                // Tests start from the defaults, whatever the machine saved.
                #[cfg(test)]
                {
                    Preferences::default()
                }
                #[cfg(not(test))]
                {
                    // preferences.json first; the older settings.json key
                    // for a first run on this layout.
                    pages::file_path()
                        .and_then(|p| pages::read_file_at(&p))
                        .map(|f| f.general)
                        .or_else(|| settings_path().and_then(|p| read_at(&p)))
                        .unwrap_or_default()
                }
            })
            .clone()
    })
}

/// Replaces the preferences and applies them to the running editor; the
/// settings file is written when the dialog next sees the mouse released.
pub fn set(p: Preferences) {
    apply_runtime(&p);
    live(|l| {
        if l.prefs.as_ref() != Some(&p) {
            l.dirty = true;
        }
        l.prefs = Some(p);
    });
}

/// View > Toolbars.
pub fn show_toolbars() -> bool {
    current().show_toolbars
}

/// View > Status Bar.
pub fn show_status_bar() -> bool {
    current().show_status_bar
}

/// Lays the user's own colors (Preferences > Colors and Appearance) over the
/// canvas theme's palette; the canvas calls it each frame after it picks the
/// theme.
pub fn tint_palette(palette: &mut crate::theme::Palette) {
    let rgb = |c: [u8; 3]| egui::Color32::from_rgb(c[0], c[1], c[2]);
    if let Some(c) = current().selection_color {
        palette.selection = rgb(c);
    }
    let a = pages::current().appearance;
    if let Some(c) = a.background {
        palette.background = rgb(c);
    }
    if let Some(c) = a.grid {
        palette.grid_major = rgb(c);
        palette.grid_minor = rgb(c).gamma_multiply(0.6);
    }
    if let Some(c) = a.text {
        palette.text = rgb(c);
        palette.room_label = rgb(c);
    }
    if let Some(c) = a.temp_dim {
        palette.dimension_text = rgb(c);
    }
}

/// The Chief catalog folder the user set (the Library page's), if any.
fn chief_folder() -> Option<PathBuf> {
    #[cfg(test)]
    {
        live(|l| l.chief.as_ref().and_then(|c| c.folder.clone()))
    }
    #[cfg(not(test))]
    {
        chief::ChiefSettings::load().folder
    }
}

/// Is the light plate behind the toolbar icons on?
pub fn icon_halo() -> bool {
    HALO.with(Cell::get)
}

/// Hands the choices to the parts of the editor that act on them.
fn apply_runtime(p: &Preferences) {
    HALO.with(|h| h.set(p.icon_halo));
    placed::set_auto_join(p.auto_join_countertops);
    crate::tools::cabinet::set_fit_to_gap(p.fit_cabinets_to_gap);
    crate::tools::cabinet::set_fit_tolerance(p.fit_gap_tolerance);
}

/// Loads the saved preferences and applies them; call once at startup.
pub fn apply_startup(ctx: &egui::Context) {
    let p = current();
    apply_runtime(&p);
    pages::apply_runtime(&pages::current());
    let z = p.zoom();
    if (z - 1.0).abs() > f32::EPSILON {
        ctx.set_zoom_factor(z);
    }
    live(|l| l.applied_zoom = z);
}

/// The Ray Trace dialog's starting image size.
pub fn render_size_preset() -> SizePreset {
    let p = current();
    SizePreset::ALL
        .into_iter()
        .find(|s| s.dims() == (p.render_width, p.render_height))
        .unwrap_or(SizePreset::P1280x960)
}

/// The Ray Trace dialog's starting sample count.
pub fn render_samples_preset() -> SamplesPreset {
    let p = current();
    SamplesPreset::ALL
        .into_iter()
        .find(|s| s.count() == p.render_samples)
        .unwrap_or(SamplesPreset::S64)
}

/// Opens the dialog on `page`.
pub fn open(page: Page) {
    live(|l| {
        l.open = true;
        l.page = page;
        l.chief = None;
    });
}

/// Is the dialog open?
pub fn is_open() -> bool {
    live(|l| l.open)
}

/// The page showing.
pub fn page() -> Page {
    live(|l| l.page)
}

/// Runs a Preferences command by id; false when the id is not ours.
pub fn run_command(_cx: &mut EditorContext, id: &str) -> bool {
    match id {
        OPEN => {
            open(page());
            true
        }
        _ => false,
    }
}

// ----- the window -----

/// Draws the dialog when it is open and keeps the settings file current.
/// `actions` receives the commands its buttons ask for (Default Settings,
/// Customize Hotkeys).
pub fn show_all(
    ctx: &egui::Context,
    cx: &mut EditorContext,
    settings: &mut AppSettings,
    actions: &mut Vec<Action>,
) {
    let z = current().zoom();
    if live(|l| (l.applied_zoom - z).abs() > f32::EPSILON) {
        ctx.set_zoom_factor(z);
        live(|l| l.applied_zoom = z);
    }
    if is_open() {
        let mut open = true;
        egui::Window::new("Preferences")
            .id(egui::Id::new("preferences_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([720.0, 480.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| body(ui, cx, settings, actions));
        if !open {
            live(|l| l.open = false);
        }
    }
    // The saved editing defaults (snaps, Edit Type) go over the plan's once.
    if !live(|l| std::mem::replace(&mut l.editing_applied, true)) {
        let mut editing = cx.defaults.editing.clone();
        if pages::apply_editing(&mut editing) {
            cx.defaults.editing = editing;
            cx.mark_dirty();
        }
    }
    // Write the file once the mouse is up.
    if live(|l| l.dirty) && !ctx.input(|i| i.pointer.any_down()) {
        let res = save_all();
        live(|l| {
            l.dirty = false;
            l.note = res
                .err()
                .map(|e| format!("Could not save: {e}"))
                .unwrap_or_default();
        });
        let note = live(|l| l.note.clone());
        if !note.is_empty() {
            cx.status = note;
        }
    }
}

/// Sets the Chief catalog folder (empty: the default install folder), saves
/// it in `settings.json` and rescans on the next Library Browser open.
fn set_chief_folder(folder: &str) -> Result<(), String> {
    let mut s = live(|l| l.chief.get_or_insert_with(ChiefSettings::load).clone());
    s.folder = (!folder.trim().is_empty()).then(|| PathBuf::from(folder.trim()));
    let res = s.save();
    chief::configure(&s);
    live(|l| {
        l.chief = Some(s);
        l.chief_folder_text = folder.trim().to_string();
        l.note = res.clone().err().unwrap_or_default();
    });
    res
}

/// Writes `preferences.json` from the live copies (nothing under test).
fn save_all() -> Result<(), String> {
    if cfg!(test) {
        return Ok(());
    }
    let path = pages::file_path().ok_or_else(|| crate::paths::NO_HOME.to_string())?;
    let file = pages::PrefsFile {
        version: pages::FILE_VERSION,
        general: current(),
        pages: pages::current(),
    };
    pages::write_file_at(&path, &file)
}

fn body(
    ui: &mut egui::Ui,
    cx: &mut EditorContext,
    settings: &mut AppSettings,
    actions: &mut Vec<Action>,
) {
    let mut prefs = current();
    let before = prefs.clone();
    let active = page();
    ui.horizontal_top(|ui| {
        egui::ScrollArea::vertical()
            .id_salt("prefs_page_list")
            .auto_shrink([true, false])
            .max_height(380.0)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_width(140.0);
                    for p in Page::ALL {
                        if ui.selectable_label(p == active, p.label()).clicked() {
                            live(|l| {
                                l.page = p;
                                l.chief = None;
                            });
                        }
                    }
                });
            });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("prefs_page_body")
            .auto_shrink([false, false])
            .max_height(380.0)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(420.0);
                    ui.heading(active.label());
                    ui.separator();
                    match active {
                        Page::Appearance => {
                            appearance(ui, &mut prefs, settings);
                            ui::appearance_extra(ui, &mut prefs, settings);
                        }
                        Page::Colors => colors(ui, &mut prefs, settings),
                        Page::Fonts => {
                            fonts(ui, &mut prefs);
                            ui::text_links(ui, actions);
                        }
                        Page::Library => {
                            library(ui);
                            ui::library_browser(ui);
                        }
                        Page::Render => {
                            render(ui, &mut prefs);
                            ui::render(ui);
                        }
                        Page::MaterialsList => ui::materials(ui),
                        Page::ResetOptions => ui::reset_options(ui, settings),
                        Page::Folders => ui::folders(ui, actions),
                        Page::Edit => ui::edit(ui, cx, actions),
                        Page::Behaviors => ui::behaviors(ui, cx),
                        Page::Snaps => ui::snaps(ui, cx),
                        Page::Architectural => {
                            architectural(ui, &mut prefs);
                            ui::architectural(ui);
                        }
                        Page::Cad => ui::cad(ui),
                        Page::PlanDefaults => ui::plan_defaults(ui, actions),
                        Page::UnitConversions => ui::units(ui),
                    }
                });
            });
    });
    if prefs != before {
        set(prefs);
    }
}

fn appearance(ui: &mut egui::Ui, p: &mut Preferences, settings: &mut AppSettings) {
    ui.label("Canvas theme");
    for t in CanvasTheme::ALL {
        if ui.radio(settings.theme == t, t.label()).clicked() {
            settings.theme = t;
        }
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("UI brightness");
        ui.add(
            egui::Slider::new(&mut settings.brightness, BRIGHTNESS_MIN..=BRIGHTNESS_MAX)
                .fixed_decimals(2),
        );
    });
    ui.horizontal(|ui| {
        ui.label("Text size");
        ui.add(egui::Slider::new(&mut p.text_size_pct, TEXT_SIZE_MIN..=TEXT_SIZE_MAX).suffix("%"));
    });
    ui.checkbox(
        &mut p.icon_halo,
        "Icon halo (a light plate behind toolbar icons)",
    );
}

/// The Fonts page: the interface text size and the switch for installed
/// fonts (plan text, layout text and printed PDFs).
fn fonts(ui: &mut egui::Ui, p: &mut Preferences) {
    ui.horizontal(|ui| {
        ui.label("Interface text size");
        ui.add(egui::Slider::new(&mut p.text_size_pct, TEXT_SIZE_MIN..=TEXT_SIZE_MAX).suffix("%"));
    });
    ui.add_space(6.0);
    let mut on = crate::fonts::use_system_fonts();
    if ui
        .checkbox(&mut on, "Use system fonts for plan, layout and PDF text")
        .changed()
    {
        let saved = crate::fonts::set_use_system_fonts(on).err();
        ui.data_mut(|d| d.insert_temp(egui::Id::new("fonts_save_error"), saved));
    }
    if let Some(Some(e)) =
        ui.data(|d| d.get_temp::<Option<String>>(egui::Id::new("fonts_save_error")))
    {
        ui.weak(format!(
            "The choice holds for this run; it could not be saved ({e})."
        ));
    }
    ui.weak(
        "A text style's font (Avenir, Arial...) is drawn and printed in the installed font of \
         that name. Off: the bundled font on screen, Helvetica on paper.",
    );
    ui.weak(
        "Installed fonts are embedded (subset) only in the PDFs you make here; a font whose \
         licence forbids embedding is printed in Helvetica.",
    );
    ui.add_space(6.0);
    let cat = crate::fonts::catalog();
    ui.label(format!(
        "{} font families found on this computer.",
        cat.families().len()
    ));
    for n in crate::fonts::notes_so_far() {
        ui.weak(n);
    }
}

fn colors(ui: &mut egui::Ui, p: &mut Preferences, settings: &AppSettings) {
    let theme_color = settings.theme.palette().selection;
    let mut custom = p.selection_color.is_some();
    if ui
        .checkbox(&mut custom, "Use my own selection color")
        .changed()
    {
        p.selection_color = custom.then(|| [theme_color.r(), theme_color.g(), theme_color.b()]);
    }
    if let Some(c) = &mut p.selection_color {
        ui.horizontal(|ui| {
            ui.label("Selection");
            ui.color_edit_button_srgb(c);
        });
    } else {
        ui.weak("The canvas theme's selection color is used.");
    }
    ui.add_space(6.0);
    ui.weak("Layer colors are set per layer in Tools > Layer Settings > Display Options.");
}

fn library(ui: &mut egui::Ui) {
    let (mut s, mut folder) = live(|l| {
        let s = l.chief.get_or_insert_with(ChiefSettings::load).clone();
        if l.chief_folder_text.is_empty() {
            l.chief_folder_text = s
                .folder
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
        (s, l.chief_folder_text.clone())
    });
    let mut changed = ui
        .checkbox(&mut s.enabled, "Use Chief Architect catalogs")
        .changed();
    ui.horizontal(|ui| {
        ui.label("Catalog folder");
        changed |= ui
            .add(
                egui::TextEdit::singleline(&mut folder)
                    .hint_text("the install folder with Core Libraries")
                    .desired_width(260.0),
            )
            .lost_focus();
        if ui.button("Browse...").clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                folder = dir.to_string_lossy().into_owned();
                changed = true;
            }
        }
    });
    ui.weak("Leave the folder empty to use the default install folder. The Library Browser rescans when it is next opened.");
    if changed {
        s.folder = (!folder.trim().is_empty()).then(|| PathBuf::from(folder.trim()));
        let res = s.save();
        chief::configure(&s);
        live(|l| {
            l.chief = Some(s.clone());
            l.chief_folder_text = folder.clone();
            l.note = res.err().unwrap_or_default();
        });
    }
    let note = live(|l| l.note.clone());
    if !note.is_empty() {
        ui.colored_label(egui::Color32::LIGHT_RED, note);
    }
}

fn render(ui: &mut egui::Ui, p: &mut Preferences) {
    ui.label("The Ray Trace dialog starts from these.");
    ui.horizontal(|ui| {
        ui.label("Image size");
        let cur = SizePreset::ALL
            .into_iter()
            .find(|s| s.dims() == (p.render_width, p.render_height))
            .unwrap_or(SizePreset::P1280x960);
        egui::ComboBox::from_id_salt("prefs_render_size")
            .selected_text(cur.label())
            .show_ui(ui, |ui| {
                for s in SizePreset::ALL {
                    if ui.selectable_label(s == cur, s.label()).clicked() {
                        (p.render_width, p.render_height) = s.dims();
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Samples per pixel");
        let cur = SamplesPreset::ALL
            .into_iter()
            .find(|s| s.count() == p.render_samples)
            .unwrap_or(SamplesPreset::S64);
        egui::ComboBox::from_id_salt("prefs_render_samples")
            .selected_text(cur.count().to_string())
            .show_ui(ui, |ui| {
                for s in SamplesPreset::ALL {
                    if ui
                        .selectable_label(s == cur, s.count().to_string())
                        .clicked()
                    {
                        p.render_samples = s.count();
                    }
                }
            });
    });
    ui.checkbox(&mut p.render_clay, "Start in the Clay technique");
    ui.horizontal(|ui| {
        ui.label("Sun latitude");
        ui.add(
            egui::DragValue::new(&mut p.render_latitude)
                .speed(0.1)
                .range(-66.0..=66.0)
                .suffix("\u{b0}"),
        );
    });
}

fn architectural(ui: &mut egui::Ui, p: &mut Preferences) {
    ui.checkbox(
        &mut p.auto_join_countertops,
        "Join the countertops of touching base cabinets automatically",
    );
    ui.weak("A joined top is regenerated when a cabinet under it is moved, resized or deleted.");
    ui.add_space(4.0);
    ui.checkbox(
        &mut p.fit_cabinets_to_gap,
        "Fit a cabinet dragged into a gap that is close to its width",
    );
    ui.horizontal(|ui| {
        ui.label("Gap within");
        ui.add(
            egui::DragValue::new(&mut p.fit_gap_tolerance)
                .range(0.0..=12.0)
                .speed(0.1)
                .suffix("\""),
        );
        ui.label("of the cabinet's width");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("plan-studio-prefs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn preferences_round_trip_and_keep_the_other_keys() {
        let file = scratch("settings.json");
        std::fs::write(
            &file,
            r#"{"theme":"low_glare","brightness":0.8,"templates":{"seed_from_chief":true}}"#,
        )
        .unwrap();
        assert_eq!(read_at(&file), None);
        let p = Preferences {
            text_size_pct: 120,
            icon_halo: true,
            selection_color: Some([10, 20, 30]),
            render_width: 1920,
            render_height: 1080,
            render_samples: 256,
            render_clay: true,
            render_latitude: 40.5,
            show_status_bar: false,
            show_toolbars: false,
            auto_join_countertops: false,
            fit_cabinets_to_gap: false,
            fit_gap_tolerance: 3.5,
        };
        write_at(&file, &p).unwrap();
        assert_eq!(read_at(&file), Some(p));
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(v["brightness"], 0.8);
        assert_eq!(v["templates"]["seed_from_chief"], true);
        // A file with only some keys fills the rest from the defaults.
        std::fs::write(&file, r#"{"preferences":{"icon_halo":true}}"#).unwrap();
        let q = read_at(&file).unwrap();
        assert!(q.icon_halo && q.auto_join_countertops && q.text_size_pct == 100);
        assert_eq!(q.render_width, 1280);
    }

    #[test]
    fn setting_preferences_reaches_the_editor() {
        let p = Preferences {
            icon_halo: true,
            auto_join_countertops: true,
            fit_cabinets_to_gap: false,
            ..Preferences::default()
        };
        // `set` also marks the file dirty; nothing writes it here.
        set(p);
        assert!(icon_halo());
        assert!(placed::auto_join_enabled());
        assert!(!crate::tools::cabinet::fit_to_gap_enabled());
        set(Preferences::default());
        assert!(!icon_halo());
        assert!(crate::tools::cabinet::fit_to_gap_enabled());
        assert_eq!(current(), Preferences::default());
    }

    #[test]
    fn render_defaults_choose_the_ray_trace_presets() {
        set(Preferences {
            render_width: 1920,
            render_height: 1080,
            render_samples: 256,
            ..Preferences::default()
        });
        assert_eq!(render_size_preset(), SizePreset::P1920x1080);
        assert_eq!(render_samples_preset(), SamplesPreset::S256);
        // A size no preset has falls back to the first.
        set(Preferences {
            render_width: 7,
            ..Preferences::default()
        });
        assert_eq!(render_size_preset(), SizePreset::P1280x960);
        set(Preferences::default());
    }

    #[test]
    fn the_selection_color_is_laid_over_the_theme() {
        let mut p = CanvasTheme::LowGlare.palette();
        let theme_color = p.selection;
        set(Preferences::default());
        tint_palette(&mut p);
        assert_eq!(p.selection, theme_color);
        set(Preferences {
            selection_color: Some([1, 2, 3]),
            ..Preferences::default()
        });
        tint_palette(&mut p);
        assert_eq!(p.selection, egui::Color32::from_rgb(1, 2, 3));
        set(Preferences::default());
    }

    #[test]
    fn text_size_is_clamped_to_a_zoom_factor() {
        let mut p = Preferences::default();
        assert_eq!(p.zoom(), 1.0);
        p.text_size_pct = 500;
        assert_eq!(p.zoom(), 1.5);
        p.text_size_pct = 1;
        assert_eq!(p.zoom(), 0.8);
    }

    #[test]
    fn the_window_draws_every_page_headlessly() {
        let ctx = egui::Context::default();
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let mut settings = AppSettings::default();
        let mut actions = Vec::new();
        for page in Page::ALL {
            open(page);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                show_all(ctx, &mut cx, &mut settings, &mut actions);
            });
            assert!(is_open());
        }
        live(|l| l.open = false);
        live(|l| l.dirty = false);
    }

    #[test]
    fn run_command_opens_the_dialog() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        live(|l| l.open = false);
        assert!(run_command(&mut cx, OPEN));
        assert!(is_open());
        assert!(!run_command(&mut cx, "nope"));
        live(|l| l.open = false);
    }
}
