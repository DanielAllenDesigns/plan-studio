//! Chief's "New Plan opens my template" for Plan Studio.
//!
//! * [`TemplateSettings`] (`templates` key of `~/.plan-studio/settings.json`):
//!   the default plan and layout template paths and whether to seed the
//!   defaults from them. On the first run (no `templates` key yet) the paths
//!   come from Chief's own preferences (`plan_config::detect_chief_templates`).
//! * [`SeedCache`] (`~/.plan-studio/template-seed.json`): what was decoded
//!   from the templates, with each file's modification time, so the 9 MB plan
//!   is decoded once and again only when the file changes.
//! * [`overlay`]: the decoded wall types, text styles, dimension sets and
//!   default height laid over a base [`PlanDefaults`] (the template wins).
//! * [`new_layout`]: a layout on the template's sheet with Daniel's title
//!   block.
//!
//! Without a template (CI machines, other users) none of this runs and the
//! embedded Chief X18 template applies.

use plan_chiefplan::bridge::{
    dimension_defaults_from_template, layout_seed, text_style_from_template, wall_type_def,
};
use plan_chiefplan::{
    build_inventory, Category, TemplateDimensionDefaults, TemplateInventory, TemplateTextStyle,
    TemplateWallType,
};
use plan_core::defaults::{DimensionDefaultSet, PlanDefaults};
use plan_core::units::fmt_ft_in_frac;
use plan_docs::SheetSize;
use plan_layout::{Layout, TitleBlockTemplate};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Mutex;

/// The key in `settings.json`.
pub const SETTINGS_KEY: &str = "templates";
/// The decode cache, next to `settings.json`.
pub const CACHE_FILE: &str = "template-seed.json";
/// Bumped when the cache layout changes; an older cache is ignored.
pub const CACHE_VERSION: u32 = 2;

// ===================================================================
// Settings
// ===================================================================

fn yes() -> bool {
    true
}

/// Which templates new plans and layouts start from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSettings {
    /// The default plan template (`.plan`).
    #[serde(default)]
    pub plan: Option<PathBuf>,
    /// The default layout template (`.layout`).
    #[serde(default)]
    pub layout: Option<PathBuf>,
    /// Lay the plan template's values over the shipped defaults.
    #[serde(default = "yes")]
    pub seed_from_chief: bool,
}

impl Default for TemplateSettings {
    fn default() -> Self {
        Self {
            plan: None,
            layout: None,
            seed_from_chief: true,
        }
    }
}

/// `~/.plan-studio/settings.json`.
pub fn settings_path() -> Option<PathBuf> {
    crate::paths::user_file("settings.json")
}

/// `~/.plan-studio/template-seed.json`.
pub fn cache_path() -> Option<PathBuf> {
    crate::paths::user_file(CACHE_FILE)
}

/// The `templates` settings in `path`, or `None` when the file or the key is
/// missing or unreadable.
pub fn read_settings_at(path: &Path) -> Option<TemplateSettings> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    serde_json::from_value(v.get(SETTINGS_KEY)?.clone()).ok()
}

/// Writes the `templates` key of `path`, keeping every other key.
pub fn write_settings_at(path: &Path, s: &TemplateSettings) -> Result<(), String> {
    let mut v = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    v[SETTINGS_KEY] = serde_json::to_value(s).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// The saved settings; on the first run (no `templates` key) Chief's default
/// templates are detected under `home` and saved. The flag is `true` when
/// this call did the detecting. (The app scans through [`begin_detection`];
/// this is the synchronous form.)
#[cfg(test)]
pub fn load_or_detect_at(path: &Path, home: Option<&Path>) -> (TemplateSettings, bool) {
    let (settings, detected, _) = load_or_detect_reporting(path, home);
    (settings, detected)
}

/// [`load_or_detect_at`] that also says why the write failed. A read-only
/// home still gets the detected paths for this run.
fn load_or_detect_reporting(
    path: &Path,
    home: Option<&Path>,
) -> (TemplateSettings, bool, Option<String>) {
    if let Some(s) = read_settings_at(path) {
        return (s, false, None);
    }
    let settings = detect_settings(home);
    let err = write_settings_at(path, &settings).err();
    (settings, true, err)
}

/// The template settings in force. This never scans the home folder (the scan
/// of a first launch takes seconds and runs on a thread, see
/// [`begin_detection`]): the saved `templates` key, else what this session's
/// detection found (even when it could not be saved), else no templates.
pub fn load_settings() -> TemplateSettings {
    settings_path()
        .and_then(|p| read_settings_at(&p))
        .or_else(session_detected)
        .unwrap_or_default()
}

// ===================================================================
// First-launch detection, off the main thread
// ===================================================================

/// What the first-launch scan found.
#[derive(Debug, Clone)]
pub struct Detection {
    /// The templates found (and the settings of this session).
    pub settings: TemplateSettings,
    /// The defaults they seed, decoded on the scan's thread.
    pub defaults: PlanDefaults,
    /// A status-bar note about the seeding.
    pub note: Option<String>,
    /// Why the settings file could not be written; the session still uses
    /// `settings`, and no later call scans again.
    pub save_error: Option<String>,
}

/// The state of this session's first-launch scan: whether it started, the
/// scan in flight, and its result, which is kept even when the settings file
/// could not be written, so a read-only home is scanned once per session, not
/// on every call that wants the settings.
pub struct Startup {
    begun: AtomicBool,
    pending: Mutex<Option<Receiver<Detection>>>,
    session: Mutex<Option<TemplateSettings>>,
}

/// The one of this process.
static STARTUP: Startup = Startup::new();

impl Startup {
    pub const fn new() -> Self {
        Self {
            begun: AtomicBool::new(false),
            pending: Mutex::new(None),
            session: Mutex::new(None),
        }
    }

    /// What the scan found, once it has finished.
    pub fn session(&self) -> Option<TemplateSettings> {
        self.session.lock().ok()?.clone()
    }

    /// Runs `detect` on a thread unless the machine already has saved
    /// settings (`saved`), the scan began, or its result is in. True when a
    /// scan started.
    pub fn begin(&self, saved: bool, detect: impl FnOnce() -> Detection + Send + 'static) -> bool {
        if saved || self.session().is_some() || self.begun.swap(true, Ordering::SeqCst) {
            return false;
        }
        let rx = spawn_detection(detect);
        if let Ok(mut p) = self.pending.lock() {
            *p = Some(rx);
        }
        true
    }

    /// Is a scan running?
    pub fn running(&self) -> bool {
        self.pending.lock().is_ok_and(|p| p.is_some())
    }

    /// The finished scan, once. Remembers its settings for the session.
    pub fn poll(&self) -> Option<Detection> {
        let mut guard = self.pending.lock().ok()?;
        let rx = guard.as_ref()?;
        match rx.try_recv() {
            Ok(d) => {
                *guard = None;
                drop(guard);
                if let Ok(mut s) = self.session.lock() {
                    *s = Some(d.settings.clone());
                }
                Some(d)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                *guard = None;
                None
            }
        }
    }
}

impl Default for Startup {
    fn default() -> Self {
        Self::new()
    }
}

fn session_detected() -> Option<TemplateSettings> {
    STARTUP.session()
}

/// Runs `detect` on its own thread; the result arrives on the channel.
pub fn spawn_detection(detect: impl FnOnce() -> Detection + Send + 'static) -> Receiver<Detection> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(detect());
    });
    rx
}

/// The scan itself: Chief's default templates under `home`, saved to the
/// settings file at `path` (best effort), and the defaults they seed.
pub fn detect_and_seed(path: Option<&Path>, home: Option<&Path>, base: PlanDefaults) -> Detection {
    let (settings, save_error) = match path {
        Some(p) => {
            let (s, _, err) = load_or_detect_reporting(p, home);
            (s, err)
        }
        None => (
            detect_settings(home),
            Some(crate::paths::NO_HOME.to_string()),
        ),
    };
    let (defaults, note) = seeded_defaults(&settings, base);
    Detection {
        settings,
        defaults,
        note,
        save_error,
    }
}

fn detect_settings(home: Option<&Path>) -> TemplateSettings {
    let found = home.map(plan_config::detect_chief_templates);
    // Preferences > Folders > Templates, when the user set one, supplies the
    // templates Chief's own preferences did not name.
    let own = custom_templates_folder();
    let from_own = |ext: &str| own.as_deref().and_then(|d| first_with_extension(d, ext));
    TemplateSettings {
        plan: found
            .as_ref()
            .and_then(|f| f.plan.clone())
            .or_else(|| from_own("plan")),
        layout: found
            .as_ref()
            .and_then(|f| f.layout.clone())
            .or_else(|| from_own("layout")),
        seed_from_chief: true,
    }
}

/// The Templates folder of Preferences > Folders when the user changed it
/// from the default.
fn custom_templates_folder() -> Option<PathBuf> {
    use crate::dialogs::preferences::pages::{self, FolderKind};
    pages::current().folders.get(FolderKind::Templates)?;
    pages::folder(FolderKind::Templates)
}

/// The first file (by name) in `dir` with extension `ext`.
fn first_with_extension(dir: &Path, ext: &str) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(ext))
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

/// Starts the first-launch scan on a thread when this machine has no saved
/// `templates` key and the session has not scanned yet. Returns whether a scan
/// started. The window must not wait for it: [`poll_detection`] hands the
/// result over once per frame.
pub fn begin_detection() -> bool {
    let path = settings_path();
    let saved = path.as_deref().and_then(read_settings_at).is_some();
    let home = crate::paths::home_dir();
    STARTUP.begin(saved, move || {
        detect_and_seed(
            path.as_deref(),
            home.as_deref(),
            crate::plan_defaults::embedded(),
        )
    })
}

/// Is the first-launch scan still running? (The window asks to be redrawn
/// while it is, so the result is picked up even when nothing else happens.)
pub fn detection_running() -> bool {
    STARTUP.running()
}

/// The finished scan, once (call every frame; cheap).
pub fn poll_detection() -> Option<Detection> {
    STARTUP.poll()
}

/// Saves `s` to the real settings file.
pub fn save_settings(s: &TemplateSettings) -> Result<(), String> {
    write_settings_at(&settings_path().ok_or(crate::paths::NO_HOME)?, s)
}

// ===================================================================
// Decode cache
// ===================================================================

/// What was read from the plan template.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PlanSeed {
    pub path: String,
    pub file_name: String,
    /// Modification time of the file when it was read, ms since the epoch.
    pub mtime_ms: u64,
    /// When it was read, seconds since the epoch.
    pub read_at: u64,
    pub wall_types: Vec<TemplateWallType>,
    pub text_styles: Vec<TemplateTextStyle>,
    pub dimension_defaults: Vec<TemplateDimensionDefaults>,
    /// The room-type height (109.125 in Daniel's template), inches.
    pub default_height_in: Option<f64>,
    pub layer_sets: usize,
    pub layers: usize,
    pub materials: usize,
    pub plan_views: usize,
    /// The names of the layer sets, saved plan views and Rich Text defaults
    /// (the decode reads names only; each starts as a copy of the active one).
    #[serde(default)]
    pub layer_set_names: Vec<String>,
    #[serde(default)]
    pub plan_view_names: Vec<String>,
    #[serde(default)]
    pub rich_text_defaults: Vec<String>,
}

/// What was read from the layout template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutInfoSeed {
    pub path: String,
    pub file_name: String,
    pub mtime_ms: u64,
    pub read_at: u64,
    /// The listed sheet name, when the file names one.
    pub sheet_name: Option<String>,
    /// Sheet width and height in inches (the `18x24` of the file name when
    /// the numbers are not stored).
    pub sheet_in: Option<(f64, f64)>,
    pub pages: usize,
    pub text_styles: usize,
}

/// Both decodes, as saved in [`CACHE_FILE`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeedCache {
    pub version: u32,
    pub plan: Option<PlanSeed>,
    pub layout: Option<LayoutInfoSeed>,
}

impl Default for SeedCache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            plan: None,
            layout: None,
        }
    }
}

impl SeedCache {
    /// The cache in `path`; empty when missing, unreadable or from another
    /// version.
    pub fn load_at(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<Self>(&t).ok())
            .filter(|c| c.version == CACHE_VERSION)
            .unwrap_or_default()
    }

    pub fn save_at(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }

    /// The cache saved in `~/.plan-studio` (empty if none).
    pub fn load() -> Self {
        cache_path().map_or_else(Self::default, |p| Self::load_at(&p))
    }
}

/// Seconds since the epoch.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Modification time of `path`, ms since the epoch.
pub fn mtime_ms(path: &Path) -> Option<u64> {
    let m = std::fs::metadata(path).ok()?.modified().ok()?;
    let d = m.duration_since(std::time::UNIX_EPOCH).ok()?;
    u64::try_from(d.as_millis()).ok()
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Packs the parts of an inventory the seed needs.
pub fn pack_plan(path: &Path, mtime_ms: u64, now: u64, inv: &TemplateInventory) -> PlanSeed {
    PlanSeed {
        path: path.to_string_lossy().into_owned(),
        file_name: file_name_of(path),
        mtime_ms,
        read_at: now,
        wall_types: inv.summary.wall_types.clone(),
        text_styles: inv.summary.text_styles.clone(),
        dimension_defaults: inv.summary.dimension_defaults.clone(),
        default_height_in: inv.summary.default_heights.room_type_height_in,
        layer_sets: inv.layer_sets.len(),
        layers: inv.layers.len(),
        materials: inv
            .summary
            .materials
            .iter()
            .filter(|m| !m.name.is_empty())
            .count(),
        plan_views: inv.plan_views.len(),
        layer_set_names: inv.layer_sets.iter().map(|e| e.name.clone()).collect(),
        plan_view_names: inv.plan_views.iter().map(|e| e.name.clone()).collect(),
        rich_text_defaults: inv
            .rich_text_defaults
            .iter()
            .map(|e| e.name.clone())
            .collect(),
    }
}

/// Packs the sheet and page vocabulary of a layout inventory.
pub fn pack_layout(
    path: &Path,
    mtime_ms: u64,
    now: u64,
    inv: &TemplateInventory,
) -> LayoutInfoSeed {
    let l = layout_seed(inv);
    LayoutInfoSeed {
        path: path.to_string_lossy().into_owned(),
        file_name: file_name_of(path),
        mtime_ms,
        read_at: now,
        sheet_name: l.sheet_size,
        sheet_in: l.sheet_dimensions_in,
        pages: l.pages.len(),
        text_styles: inv.text_styles.len(),
    }
}

/// Decodes a plan template file.
pub fn decode_plan_file(path: &Path) -> Result<PlanSeed, String> {
    let mtime = mtime_ms(path).unwrap_or(0);
    let inv = build_inventory(path).map_err(|e| e.to_string())?;
    Ok(pack_plan(path, mtime, now_secs(), &inv))
}

/// Decodes a layout template file.
pub fn decode_layout_file(path: &Path) -> Result<LayoutInfoSeed, String> {
    let mtime = mtime_ms(path).unwrap_or(0);
    let inv = build_inventory(path).map_err(|e| e.to_string())?;
    Ok(pack_layout(path, mtime, now_secs(), &inv))
}

/// The outcome of [`refresh_with`].
#[derive(Debug, Clone, PartialEq)]
pub struct Refresh {
    pub cache: SeedCache,
    /// Problems worth showing (a missing file, a decode error).
    pub notes: Vec<String>,
    /// Some template was decoded now (not served from the cache).
    pub decoded: bool,
}

type PlanDecoder<'a> = &'a mut dyn FnMut(&Path) -> Result<PlanSeed, String>;
type LayoutDecoder<'a> = &'a mut dyn FnMut(&Path) -> Result<LayoutInfoSeed, String>;

/// Brings `cache` in line with the settings. A template is decoded again when
/// `force` is set, when its path changed or when its modification time
/// differs from the cached one; otherwise the cached decode is kept. A path
/// that does not exist clears that entry.
pub fn refresh_with(
    mut cache: SeedCache,
    settings: &TemplateSettings,
    force: bool,
    decode_plan: PlanDecoder<'_>,
    decode_layout: LayoutDecoder<'_>,
) -> Refresh {
    let mut notes = Vec::new();
    let mut decoded = false;

    match settings.plan.as_deref() {
        Some(path) if path.is_file() => {
            let mtime = mtime_ms(path).unwrap_or(0);
            let fresh = cache
                .plan
                .as_ref()
                .is_some_and(|c| !force && c.mtime_ms == mtime && Path::new(&c.path) == path);
            if !fresh {
                match decode_plan(path) {
                    Ok(seed) => {
                        cache.plan = Some(seed);
                        decoded = true;
                    }
                    Err(e) => {
                        cache.plan = None;
                        notes.push(format!("Could not read {}: {e}", path.display()));
                    }
                }
            }
        }
        Some(path) => {
            cache.plan = None;
            notes.push(format!("Plan template not found: {}", path.display()));
        }
        None => cache.plan = None,
    }

    match settings.layout.as_deref() {
        Some(path) if path.is_file() => {
            let mtime = mtime_ms(path).unwrap_or(0);
            let fresh = cache
                .layout
                .as_ref()
                .is_some_and(|c| !force && c.mtime_ms == mtime && Path::new(&c.path) == path);
            if !fresh {
                match decode_layout(path) {
                    Ok(seed) => {
                        cache.layout = Some(seed);
                        decoded = true;
                    }
                    Err(e) => {
                        cache.layout = None;
                        notes.push(format!("Could not read {}: {e}", path.display()));
                    }
                }
            }
        }
        Some(path) => {
            cache.layout = None;
            notes.push(format!("Layout template not found: {}", path.display()));
        }
        None => cache.layout = None,
    }

    Refresh {
        cache,
        notes,
        decoded,
    }
}

/// [`refresh_with`] on the real files and the real cache; the cache file is
/// rewritten when something changed.
pub fn refresh(settings: &TemplateSettings, force: bool) -> Refresh {
    let path = cache_path();
    let before = path
        .as_deref()
        .map_or_else(SeedCache::default, SeedCache::load_at);
    let mut out = refresh_with(
        before.clone(),
        settings,
        force,
        &mut decode_plan_file,
        &mut decode_layout_file,
    );
    if out.cache != before {
        if let Some(p) = &path {
            if let Err(e) = out.cache.save_at(p) {
                out.notes
                    .push(format!("Could not save the template cache: {e}"));
            }
        }
    }
    out
}

// ===================================================================
// Seeding
// ===================================================================

fn short_set_name(full: &str) -> String {
    full.strip_suffix(" Dimension Defaults")
        .unwrap_or(full)
        .trim()
        .to_string()
}

/// Lays the plan template over `base`; the template wins.
///
/// * Wall types: each decoded type with its real layer stack and main layer
///   replaces a type of the same name (keeping its kind) or is added.
/// * Text styles: each decoded style (font, weight, plan height) replaces the
///   shipped style of that name or is added. The font is recorded as the
///   template names it (`Avenir`); a machine without it falls back when the
///   text is drawn.
/// * Dimension sets: each decoded set is laid over the same-named saved set
///   (else over the current dimension settings); the active set stays.
/// * Default height (the room-type height, 109.125") for rooms and for the
///   exterior and interior wall defaults.
pub fn overlay(mut d: PlanDefaults, seed: &PlanSeed) -> PlanDefaults {
    for t in seed.wall_types.iter().filter(|t| !t.layers.is_empty()) {
        let mut def = wall_type_def(t);
        match d.wall_types.iter_mut().find(|w| w.name == def.name) {
            Some(existing) => {
                def.kind = existing.kind;
                *existing = def;
            }
            None => d.wall_types.push(def),
        }
    }

    for t in &seed.text_styles {
        let mut style = text_style_from_template(t);
        match d
            .text_styles
            .styles
            .iter_mut()
            .find(|s| s.name == style.name)
        {
            Some(existing) => {
                style.underline = existing.underline;
                *existing = style;
            }
            None => {
                d.text_styles.add(style);
            }
        }
    }
    if let Some(t) = seed
        .text_styles
        .iter()
        .find(|t| t.name == plan_core::text_styles::DEFAULT_TEXT_STYLE_NAME)
    {
        d.text.font = t.font.clone();
        d.text.height = t.height_in;
    }

    for ds in &seed.dimension_defaults {
        let name = short_set_name(&ds.name);
        if name.is_empty() {
            continue;
        }
        let base = d
            .dimension_set(&name)
            .map_or_else(|| d.dimensions.clone(), |s| s.auto.clone());
        let set =
            DimensionDefaultSet::new(name.clone(), dimension_defaults_from_template(ds, &base));
        match d.dimension_sets.iter_mut().find(|s| s.name == name) {
            Some(existing) => *existing = set,
            None => d.dimension_sets.push(set),
        }
    }
    let active = d.active_dimension_set.clone();
    d.set_active_dimension_set(&active);

    if let Some(h) = seed.default_height_in.filter(|h| *h > 0.0) {
        d.rooms.ceiling_height = h;
        d.exterior_wall.height = h;
        d.interior_wall.height = h;
    }
    seed_names(&mut d, seed);
    d
}

/// The name Chief gives a saved default when the template lists it as
/// "<name> Rich Text Defaults".
fn short_rich_name(full: &str) -> String {
    full.strip_suffix(" Rich Text Defaults")
        .unwrap_or(full)
        .trim()
        .to_string()
}

/// Lays the template's layer sets, saved plan views, Rich Text defaults and
/// Default Sets over `d`. The decode reads names, not the values behind them:
/// a layer set starts as a copy of the active one, a Rich Text default as the
/// current Rich Text defaults, and a saved plan view shows the layer set of
/// the same stem ("Plot Plan View" shows "Plot Plan Layer Set") or the
/// active one. A Default Set is made for every name that has both a
/// dimension set and a Rich Text default ("1/4\" Scale"), with the layer set
/// of the same stem when there is one (DECISIONS DS13).
fn seed_names(d: &mut PlanDefaults, seed: &PlanSeed) {
    use plan_core::defaults::saved::{DefaultSet, KindList, SavedDefault, SavedKind, SavedValue};
    for n in seed.layer_set_names.iter().filter(|n| !n.trim().is_empty()) {
        if d.layer_sets.get(n).is_none() {
            let active = d.layer_sets.active.clone();
            d.layer_sets.copy_set(&active, n);
        }
    }
    let stem_set = |d: &PlanDefaults, stem: &str| -> Option<String> {
        [
            format!("{stem} Layer Set"),
            format!("{stem} Plan Layer Set"),
        ]
        .into_iter()
        .find(|n| d.layer_sets.get(n).is_some())
    };
    for v in seed.plan_view_names.iter().filter(|n| !n.trim().is_empty()) {
        if d.plan_views.iter().any(|x| &x.name == v) {
            continue;
        }
        let stem = v
            .trim_end_matches(" Plan View")
            .trim_end_matches(" View")
            .trim();
        let set = stem_set(d, stem).unwrap_or_else(|| d.layer_sets.active.clone());
        d.plan_views
            .push(plan_core::SavedPlanView::new(v.clone(), set));
    }
    if !seed.rich_text_defaults.is_empty() {
        let base = plan_core::callout::TextSpec::default();
        let list = d
            .saved
            .lists
            .entry(SavedKind::RichText.id().to_string())
            .or_insert_with(|| KindList {
                active: plan_core::callout::DEFAULT_SAVED_NAME.to_string(),
                items: vec![SavedDefault {
                    name: plan_core::callout::DEFAULT_SAVED_NAME.to_string(),
                    value: SavedValue::Rich(base.clone()),
                }],
            });
        for n in &seed.rich_text_defaults {
            let name = short_rich_name(n);
            if !name.is_empty() && !list.items.iter().any(|s| s.name == name) {
                list.items.push(SavedDefault {
                    name,
                    value: SavedValue::Rich(base.clone()),
                });
            }
        }
    }
    let dim_names: Vec<String> = seed
        .dimension_defaults
        .iter()
        .map(|ds| short_set_name(&ds.name))
        .filter(|n| !n.is_empty())
        .collect();
    for stem in &dim_names {
        let has_rich = seed
            .rich_text_defaults
            .iter()
            .any(|r| short_rich_name(r) == *stem);
        if !has_rich || d.saved.set(stem).is_some() {
            continue;
        }
        let mut set = DefaultSet {
            name: stem.clone(),
            ..DefaultSet::default()
        };
        set.members
            .insert(SavedKind::ManualDimensions.id().to_string(), stem.clone());
        set.members
            .insert(SavedKind::RichText.id().to_string(), stem.clone());
        if let Some(ls) = stem_set(d, stem) {
            set.layer_set = ls;
        }
        d.saved.sets.push(set);
    }
}

/// `base` with the cached plan template laid over it when seeding is on and
/// a decode exists; else `base` as is.
pub fn defaults_from(
    settings: &TemplateSettings,
    cache: &SeedCache,
    base: PlanDefaults,
) -> PlanDefaults {
    match (&cache.plan, settings.seed_from_chief) {
        (Some(seed), true) => overlay(base, seed),
        _ => base,
    }
}

/// The defaults a new plan starts from when the user has not saved their own:
/// the embedded Chief X18 template, with the plan template laid over it when
/// seeding is on and the template can be read. The note is for the status bar
/// (a fresh decode, or why the template was not used).
pub fn seeded_defaults(
    settings: &TemplateSettings,
    base: PlanDefaults,
) -> (PlanDefaults, Option<String>) {
    if !settings.seed_from_chief || settings.plan.is_none() {
        return (base, None);
    }
    let out = refresh(settings, false);
    let mut note = out.notes.first().cloned();
    if let (true, None, Some(seed)) = (out.decoded, &note, &out.cache.plan) {
        note = Some(format!(
            "Defaults seeded from {} ({} wall types, {} text styles)",
            seed.file_name,
            seed.wall_types.len(),
            seed.text_styles.len()
        ));
    }
    (defaults_from(settings, &out.cache, base), note)
}

// ===================================================================
// Layouts
// ===================================================================

/// The listed sheet whose size is `w` x `h` inches in either orientation.
pub fn sheet_for_inches(w: f64, h: f64) -> Option<SheetSize> {
    let (lo, hi) = (w.min(h), w.max(h));
    SheetSize::ALL.into_iter().find(|s| {
        let (sw, sh) = s.inches();
        (sw.min(sh) - lo).abs() < 0.02 && (sw.max(sh) - hi).abs() < 0.02
    })
}

/// The sheet a new layout uses: the layout template's, else ARCH C
/// (18 x 24, the size in the default template's file name).
pub fn layout_sheet(info: Option<&LayoutInfoSeed>) -> SheetSize {
    info.and_then(|i| i.sheet_in)
        .and_then(|(w, h)| sheet_for_inches(w, h))
        .unwrap_or(SheetSize::ArchC)
}

/// Title of the template page of a new layout (Chief's "Page Template").
pub const TEMPLATE_PAGE_TITLE: &str = "Page Template";

/// A layout on the layout template's sheet with Daniel's title block and
/// Chief's two starting pages: page 0, the Page Template (not printed; its
/// boxes and CAD repeat on every page, and the border and title block are
/// drawn on every page), and an empty page 1. File > New Layout opens it in
/// the layout window (`shell::layout_window`).
pub fn new_layout(name: &str, info: Option<&LayoutInfoSeed>) -> Layout {
    let mut layout = Layout::new(name, layout_sheet(info));
    layout.title_block = TitleBlockTemplate::from_daniel_18x24();
    layout.add_page(0, TEMPLATE_PAGE_TITLE).template_page = true;
    layout.add_page(1, "Page 1");
    layout
}

// ===================================================================
// Summaries
// ===================================================================

/// `2026-10-08 14:03 UTC` for seconds since the epoch.
pub fn format_time(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    // Civil date from a day count (Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rem / 3_600,
        rem % 3_600 / 60
    )
}

/// The decode summary of a plan template, one line each.
pub fn plan_summary_lines(p: &PlanSeed) -> Vec<String> {
    let mut lines = vec![
        format!(
            "{} wall types, {} text styles, {} dimension sets",
            p.wall_types.len(),
            p.text_styles.len(),
            p.dimension_defaults.len()
        ),
        format!(
            "{} layer sets, {} layers, {} materials, {} plan views",
            p.layer_sets, p.layers, p.materials, p.plan_views
        ),
    ];
    if let Some(h) = p.default_height_in {
        lines.push(format!("Default height {} ({h})", fmt_ft_in_frac(h, 8)));
    }
    lines
}

/// The decode summary of a layout template.
pub fn layout_summary_lines(l: &LayoutInfoSeed) -> Vec<String> {
    let sheet = match (&l.sheet_name, l.sheet_in) {
        (Some(n), _) => n.clone(),
        (None, Some((w, h))) => format!("{w} x {h} in (from the file name)"),
        (None, None) => "no sheet size found".into(),
    };
    vec![format!(
        "Sheet {sheet}; {} page entries, {} text styles",
        l.pages, l.text_styles
    )]
}

// ===================================================================
// Plan Studio plan templates (File > Templates)
// ===================================================================

/// The extension of a Plan Studio plan template file.
pub const PLAN_TEMPLATE_EXT: &str = "pstemplate";
/// Chief appends this to the names of metric templates (manual p. 111).
pub const METRIC_SUFFIX: &str = " - Metric";
/// The key of [`OwnTemplateSettings`] in `settings.json`.
pub const OWN_SETTINGS_KEY: &str = "plan_templates";

thread_local! {
    static DIR_OVERRIDE: std::cell::RefCell<Option<Option<PathBuf>>> =
        const { std::cell::RefCell::new(None) };
    static SETTINGS_OVERRIDE: std::cell::RefCell<Option<OwnTemplateSettings>> =
        const { std::cell::RefCell::new(None) };
}

/// `~/.plan-studio/templates/plans`, where plan templates are kept.
pub fn plan_templates_dir() -> Option<PathBuf> {
    if let Some(o) = DIR_OVERRIDE.with(|d| d.borrow().clone()) {
        return o;
    }
    crate::paths::user_file("templates").map(|d| d.join("plans"))
}

/// Points the plan template folder somewhere else (the tests), and keeps the
/// template settings in memory so no test touches the real ones.
#[cfg(test)]
pub fn set_plan_templates_dir_for_tests(dir: Option<PathBuf>) {
    DIR_OVERRIDE.with(|d| *d.borrow_mut() = dir.map(Some));
    SETTINGS_OVERRIDE.with(|s| *s.borrow_mut() = Some(OwnTemplateSettings::default()));
}

/// A plan template in the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateEntry {
    /// The name as listed (a metric template ends with " - Metric").
    pub name: String,
    pub path: PathBuf,
    pub imperial: bool,
}

/// The file name for a template called `name` in the given units: metric
/// templates carry " - Metric".
pub fn template_file_name(name: &str, imperial: bool) -> String {
    let base = name.trim().trim_end_matches(METRIC_SUFFIX).trim();
    if imperial {
        format!("{base}.{PLAN_TEMPLATE_EXT}")
    } else {
        format!("{base}{METRIC_SUFFIX}.{PLAN_TEMPLATE_EXT}")
    }
}

/// The plan templates in `dir`, sorted by name.
pub fn list_plan_templates_in(dir: &Path) -> Vec<TemplateEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<TemplateEntry> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case(PLAN_TEMPLATE_EXT))
        })
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().into_owned();
            Some(TemplateEntry {
                imperial: !name.ends_with(METRIC_SUFFIX),
                name,
                path: p,
            })
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// The plan templates of the user's folder.
pub fn list_plan_templates() -> Vec<TemplateEntry> {
    plan_templates_dir().map_or_else(Vec::new, |d| list_plan_templates_in(&d))
}

/// Saves `t` in `dir` (created if missing); returns the file.
pub fn save_plan_template_in(
    dir: &Path,
    t: &plan_core::defaults::template::PlanTemplate,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(template_file_name(&t.name, t.imperial));
    let text = t.to_json().map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Saves `t` in the user's template folder.
pub fn save_plan_template(
    t: &plan_core::defaults::template::PlanTemplate,
) -> Result<PathBuf, String> {
    save_plan_template_in(&plan_templates_dir().ok_or(crate::paths::NO_HOME)?, t)
}

/// Reads a plan template file.
pub fn load_plan_template(
    path: &Path,
) -> Result<plan_core::defaults::template::PlanTemplate, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    plan_core::defaults::template::PlanTemplate::from_json(&text).map_err(|e| e.to_string())
}

/// Which Plan Studio templates new plans start from, per unit system, and
/// which kinds the chooser hides.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct OwnTemplateSettings {
    /// The file name of the default plan template for U.S. units.
    #[serde(default)]
    pub plan_us: Option<String>,
    /// ... and for metric.
    #[serde(default)]
    pub plan_metric: Option<String>,
    /// Hide Metric Templates / Hide U.S. Templates in the chooser.
    #[serde(default)]
    pub hide_metric: bool,
    #[serde(default)]
    pub hide_us: bool,
}

impl OwnTemplateSettings {
    /// The default template for the units.
    pub fn default_for(&self, imperial: bool) -> Option<&str> {
        if imperial {
            self.plan_us.as_deref()
        } else {
            self.plan_metric.as_deref()
        }
    }

    /// Makes `file_name` the default for the units.
    pub fn set_default(&mut self, imperial: bool, file_name: &str) {
        let v = Some(file_name.to_string());
        if imperial {
            self.plan_us = v;
        } else {
            self.plan_metric = v;
        }
    }
}

/// The saved settings.
pub fn own_settings() -> OwnTemplateSettings {
    if let Some(s) = SETTINGS_OVERRIDE.with(|s| s.borrow().clone()) {
        return s;
    }
    settings_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| serde_json::from_value(v.get(OWN_SETTINGS_KEY)?.clone()).ok())
        .unwrap_or_default()
}

/// Saves the settings, keeping every other key of `settings.json`.
pub fn save_own_settings(s: &OwnTemplateSettings) -> Result<(), String> {
    if SETTINGS_OVERRIDE.with(|o| o.borrow().is_some()) {
        SETTINGS_OVERRIDE.with(|o| *o.borrow_mut() = Some(s.clone()));
        return Ok(());
    }
    let path = settings_path().ok_or(crate::paths::NO_HOME)?;
    let mut v = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    v[OWN_SETTINGS_KEY] = serde_json::to_value(s).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// What a new plan finds when it looks for the configured template.
#[derive(Debug, Clone, PartialEq)]
pub enum DefaultTemplate {
    /// No Plan Studio template is configured: the Chief template or the
    /// shipped defaults apply.
    NotConfigured,
    /// The configured template file.
    Found(PathBuf),
    /// A template is configured but its file is gone (the prompt offers Load
    /// Installed or Load Custom, manual p. 49).
    Missing(String),
}

/// The default plan template for the units, in the folder `dir`.
pub fn default_plan_template_in(
    dir: &Path,
    settings: &OwnTemplateSettings,
    imperial: bool,
) -> DefaultTemplate {
    match settings.default_for(imperial) {
        None => DefaultTemplate::NotConfigured,
        Some(file) => {
            let p = dir.join(file);
            if p.is_file() {
                DefaultTemplate::Found(p)
            } else {
                DefaultTemplate::Missing(file.to_string())
            }
        }
    }
}

/// The default plan template for the units.
pub fn default_plan_template(imperial: bool) -> DefaultTemplate {
    match plan_templates_dir() {
        Some(dir) => default_plan_template_in(&dir, &own_settings(), imperial),
        None => DefaultTemplate::NotConfigured,
    }
}

/// Decodes `path` for display without touching the cache or the settings.
pub fn preview(path: &Path) -> Result<Preview, String> {
    let mtime = mtime_ms(path).unwrap_or(0);
    let inv = build_inventory(path).map_err(|e| e.to_string())?;
    let is_layout = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("layout"));
    let now = now_secs();
    let lines = if is_layout {
        layout_summary_lines(&pack_layout(path, mtime, now, &inv))
    } else {
        let mut l = plan_summary_lines(&pack_plan(path, mtime, now, &inv));
        let sheets = inv.entries(Category::SheetSize).len();
        if sheets > 0 {
            l.push(format!("{sheets} sheet sizes listed"));
        }
        l
    };
    Ok(Preview {
        is_layout,
        lines,
        inventory: Box::new(inv),
    })
}

/// A template decoded for the Import Chief Template window.
pub struct Preview {
    pub is_layout: bool,
    pub lines: Vec<String>,
    pub inventory: Box<TemplateInventory>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_chiefplan::decode::TemplateWallLayer;

    fn temp_dir(tag: &str) -> PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let p = std::env::temp_dir().join(format!("plan-app-tpl-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn wall(name: &str, layers: &[(&str, f64, bool)]) -> TemplateWallType {
        TemplateWallType {
            name: name.into(),
            layers: layers
                .iter()
                .map(|(m, t, b)| ((*m).into(), *t, *b))
                .collect(),
            layer_details: Vec::<TemplateWallLayer>::new(),
            total_thickness_in: layers.iter().map(|l| l.1).sum(),
            offset: 0,
        }
    }

    fn style(name: &str, font: &str, height: f64, bold: bool) -> TemplateTextStyle {
        TemplateTextStyle {
            name: name.into(),
            font: font.into(),
            font_style: if bold { "Heavy" } else { "Book" }.into(),
            height_in: height,
            bold,
            italic: false,
            color: None,
            guid: String::new(),
            offset: 0,
        }
    }

    /// A synthetic decode shaped like Daniel's plan template.
    fn synthetic_seed() -> PlanSeed {
        PlanSeed {
            path: "/t/x17.plan".into(),
            file_name: "x17.plan".into(),
            mtime_ms: 1000,
            read_at: 5,
            wall_types: vec![
                wall(
                    "Stucco-6",
                    &[
                        ("Stucco", 1.0, false),
                        ("Fir Framing", 5.5, true),
                        ("Drywall", 0.5, false),
                    ],
                ),
                wall(
                    "Brand New-4",
                    &[("Plywood", 0.5, false), ("Studs", 3.5, true)],
                ),
            ],
            text_styles: vec![
                style("1/4\" Text Style", "Avenir", 4.5, false),
                style("Room Label Style", "Avenir", 6.0, true),
                style("Template Only Style", "Avenir", 9.0, false),
            ],
            dimension_defaults: vec![
                TemplateDimensionDefaults {
                    name: "1/4\" Scale Dimension Defaults".into(),
                    text_style: Some("Dimension Text Style".into()),
                    arrow_size_in: Some(3.0),
                    extension_length_towards_in: Some(1.5),
                    extension_proximity_in: Some(2.0),
                    baseline_separation_in: Some(12.0),
                    reach_in: Some(24.0),
                    decimal_places: Some(2),
                    smallest_fraction: Some(16),
                    ..TemplateDimensionDefaults::default()
                },
                TemplateDimensionDefaults {
                    name: "Brand New Dimension Defaults".into(),
                    arrow_size_in: Some(1.0),
                    ..TemplateDimensionDefaults::default()
                },
            ],
            default_height_in: Some(109.125),
            layer_sets: 3,
            layers: 100,
            materials: 40,
            plan_views: 7,
            ..PlanSeed::default()
        }
    }

    #[test]
    fn settings_round_trip_keeps_other_keys() {
        let dir = temp_dir("settings");
        let file = dir.join("settings.json");
        std::fs::write(&file, r#"{"theme":"dark","brightness":0.8}"#).unwrap();
        assert_eq!(read_settings_at(&file), None);
        let s = TemplateSettings {
            plan: Some(PathBuf::from("/a/b.plan")),
            layout: None,
            seed_from_chief: false,
        };
        write_settings_at(&file, &s).unwrap();
        assert_eq!(read_settings_at(&file), Some(s));
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["brightness"], 0.8);
        // A partial key reads with the defaults filled in.
        std::fs::write(&file, r#"{"templates":{"plan":"/x.plan"}}"#).unwrap();
        let p = read_settings_at(&file).unwrap();
        assert_eq!(
            (p.plan, p.layout, p.seed_from_chief),
            (Some("/x.plan".into()), None, true)
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn first_run_detects_once_then_keeps_the_saved_choice() {
        let home = temp_dir("home");
        let tpl = plan_config::templates_folder(&home, "X18");
        std::fs::create_dir_all(&tpl).unwrap();
        std::fs::write(tpl.join(plan_config::FALLBACK_PLAN_TEMPLATE), b"x").unwrap();
        let file = home.join(".plan-studio").join("settings.json");
        let (first, detected) = load_or_detect_at(&file, Some(&home));
        assert!(detected);
        assert_eq!(
            first.plan,
            Some(tpl.join(plan_config::FALLBACK_PLAN_TEMPLATE))
        );
        assert_eq!(first.layout, None);
        assert!(first.seed_from_chief);
        // The detection was saved; a changed choice is not detected over.
        let mut edited = first.clone();
        edited.plan = None;
        edited.seed_from_chief = false;
        write_settings_at(&file, &edited).unwrap();
        let (second, again) = load_or_detect_at(&file, Some(&home));
        assert!(!again);
        assert_eq!(second, edited);
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn overlay_gives_real_wall_stacks_and_main_layers() {
        let base = PlanDefaults::chief_x18_daniel();
        let n = base.wall_types.len();
        let d = overlay(base.clone(), &synthetic_seed());
        let stucco = d.wall_type("Stucco-6").unwrap();
        assert_eq!(stucco.layers.len(), 3);
        assert_eq!(stucco.thickness(), 7.0);
        assert_eq!(stucco.main_layer().unwrap().material, "Fir Framing");
        assert_eq!(stucco.main_layer_offset(), 1.0);
        // The kind of an existing type is kept; new types are added.
        assert_eq!(stucco.kind, base.wall_type("Stucco-6").unwrap().kind);
        assert_eq!(d.wall_types.len(), n + 1);
        let fresh = d.wall_type("Brand New-4").unwrap();
        assert_eq!(fresh.thickness(), 4.0);
        assert_eq!(fresh.layers.len(), 2);
        // The default wall type now measures what the template says.
        assert_eq!(d.exterior_thickness(), 7.0);
    }

    #[test]
    fn overlay_text_styles_use_avenir_and_the_template_heights() {
        let d = overlay(PlanDefaults::chief_x18_daniel(), &synthetic_seed());
        let q = d.text_styles.get("1/4\" Text Style").unwrap();
        assert_eq!(
            (q.font.as_str(), q.height_in, q.bold),
            ("Avenir", 4.5, false)
        );
        let room = d.text_styles.get("Room Label Style").unwrap();
        assert_eq!((room.font.as_str(), room.bold), ("Avenir", true));
        assert!(d.text_styles.get("Template Only Style").is_some());
        // Styles the template does not mention keep the shipped Arial.
        assert_eq!(d.text_styles.get("Schedule Style").unwrap().font, "Arial");
    }

    #[test]
    fn overlay_fills_dimension_slots_and_the_height() {
        let base = PlanDefaults::chief_x18_daniel();
        let sets = base.dimension_sets.len();
        let d = overlay(base, &synthetic_seed());
        let q = d.dimension_set("1/4\" Scale").unwrap();
        assert_eq!(q.auto.text_style, "Dimension Text Style");
        assert_eq!(
            (
                q.auto.arrow_size,
                q.auto.extension_toward,
                q.auto.extension_proximity,
                q.auto.baseline_separation,
                q.auto.reach,
                q.auto.decimals,
                q.auto.smallest_fraction
            ),
            (3.0, 1.5, 2.0, 12.0, 24.0, 2, 16)
        );
        assert_eq!(q.auto.set_name, "1/4\" Scale Dimension Defaults");
        // The active set's settings are mirrored into `dimensions`.
        assert_eq!(d.active_dimension_set, "1/4\" Scale");
        assert_eq!(d.dimensions.arrow_size, 3.0);
        assert_eq!(d.dimension_sets.len(), sets + 1);
        assert!(d.dimension_set("Brand New").is_some());
        for h in [
            d.rooms.ceiling_height,
            d.exterior_wall.height,
            d.interior_wall.height,
        ] {
            assert_eq!(h, 109.125);
        }
    }

    #[test]
    fn overlay_without_a_decode_changes_nothing() {
        let base = PlanDefaults::chief_x18_daniel();
        let empty = PlanSeed {
            wall_types: vec![],
            text_styles: vec![],
            dimension_defaults: vec![],
            default_height_in: None,
            ..synthetic_seed()
        };
        assert_eq!(overlay(base.clone(), &empty), base);
    }

    #[test]
    fn no_template_means_the_embedded_defaults() {
        let base = crate::plan_defaults::embedded();
        for settings in [
            TemplateSettings::default(),
            TemplateSettings {
                plan: Some(PathBuf::from("/definitely/not/here.plan")),
                seed_from_chief: false,
                layout: None,
            },
        ] {
            let (d, note) = seeded_defaults(&settings, base.clone());
            assert_eq!(d, base);
            assert_eq!(note, None);
        }
        assert_eq!(base, PlanDefaults::chief_x18_daniel());
    }

    #[test]
    fn cache_is_reused_until_the_file_changes() {
        let dir = temp_dir("cache");
        let plan = dir.join("t.plan");
        std::fs::write(&plan, b"one").unwrap();
        let settings = TemplateSettings {
            plan: Some(plan.clone()),
            layout: None,
            seed_from_chief: true,
        };
        let mut plan_calls = 0;
        let mut dec_plan = |p: &Path| -> Result<PlanSeed, String> {
            plan_calls += 1;
            Ok(PlanSeed {
                path: p.to_string_lossy().into_owned(),
                mtime_ms: mtime_ms(p).unwrap(),
                ..synthetic_seed()
            })
        };
        let mut dec_layout = |_: &Path| -> Result<LayoutInfoSeed, String> { Err("unused".into()) };

        let first = refresh_with(
            SeedCache::default(),
            &settings,
            false,
            &mut dec_plan,
            &mut dec_layout,
        );
        assert!(first.decoded);
        // Same file, same mtime: served from the cache.
        let second = refresh_with(
            first.cache.clone(),
            &settings,
            false,
            &mut dec_plan,
            &mut dec_layout,
        );
        assert!(!second.decoded);
        assert_eq!(second.cache, first.cache);
        // Forcing decodes again.
        let forced = refresh_with(
            second.cache.clone(),
            &settings,
            true,
            &mut dec_plan,
            &mut dec_layout,
        );
        assert!(forced.decoded);
        // A new modification time invalidates the cache.
        let mut stale = forced.cache.clone();
        stale.plan.as_mut().unwrap().mtime_ms += 1;
        let changed = refresh_with(stale, &settings, false, &mut dec_plan, &mut dec_layout);
        assert!(changed.decoded);
        // So does a different path.
        let other = dir.join("other.plan");
        std::fs::write(&other, b"two").unwrap();
        let moved = TemplateSettings {
            plan: Some(other),
            ..settings.clone()
        };
        assert!(
            refresh_with(
                changed.cache.clone(),
                &moved,
                false,
                &mut dec_plan,
                &mut dec_layout
            )
            .decoded
        );
        // A file that went away clears the entry and says so.
        std::fs::remove_file(&plan).unwrap();
        let gone = refresh_with(
            changed.cache,
            &settings,
            false,
            &mut dec_plan,
            &mut dec_layout,
        );
        assert!(gone.cache.plan.is_none());
        assert!(gone.notes[0].contains("not found"));
        assert_eq!(plan_calls, 4);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_file_round_trip_and_version_gate() {
        let dir = temp_dir("cachefile");
        let file = dir.join(CACHE_FILE);
        let cache = SeedCache {
            plan: Some(synthetic_seed()),
            ..SeedCache::default()
        };
        cache.save_at(&file).unwrap();
        assert_eq!(SeedCache::load_at(&file), cache);
        std::fs::write(&file, r#"{"version":0,"plan":null,"layout":null}"#).unwrap();
        assert_eq!(SeedCache::load_at(&file), SeedCache::default());
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(SeedCache::load_at(&file), SeedCache::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn layouts_use_the_template_sheet_and_daniels_title_block() {
        let arch_c = LayoutInfoSeed {
            path: "/t/18x24.layout".into(),
            file_name: "18x24.layout".into(),
            mtime_ms: 1,
            read_at: 1,
            sheet_name: None,
            sheet_in: Some((18.0, 24.0)),
            pages: 4,
            text_styles: 7,
        };
        assert_eq!(layout_sheet(Some(&arch_c)), SheetSize::ArchC);
        assert_eq!(layout_sheet(None), SheetSize::ArchC);
        let b = LayoutInfoSeed {
            sheet_in: Some((11.0, 17.0)),
            ..arch_c.clone()
        };
        assert_eq!(layout_sheet(Some(&b)), SheetSize::Tabloid);
        let l = new_layout("Cottage", Some(&arch_c));
        assert_eq!(l.sheet, SheetSize::ArchC);
        assert_eq!(l.title_block, TitleBlockTemplate::from_daniel_18x24());
        assert_eq!(sheet_for_inches(7.0, 7.0), None);
        // Page 0 is the unprinted template page, page 1 the first sheet.
        assert_eq!(l.pages.len(), 2);
        assert!(l.pages[0].template_page && l.pages[0].number == 0);
        assert!(!l.pages[1].template_page && l.pages[1].number == 1);
        assert_eq!(l.content_pages().len(), 1);
    }

    #[test]
    fn summaries_and_times() {
        let lines = plan_summary_lines(&synthetic_seed());
        assert_eq!(lines[0], "2 wall types, 3 text styles, 2 dimension sets");
        assert!(lines[2].starts_with("Default height 9'-1 1/8\""));
        assert_eq!(format_time(0), "1970-01-01 00:00 UTC");
        assert_eq!(format_time(1_791_000_000), "2026-10-03 04:00 UTC");
    }

    // ----- the real files (Daniel's machine only) -----

    fn real_templates() -> Option<(PathBuf, PathBuf)> {
        let home = crate::paths::home_dir()?;
        let found = plan_config::detect_chief_templates(&home);
        Some((found.plan?, found.layout?))
    }

    #[test]
    #[ignore = "reads Daniel's Chief templates"]
    fn real_plan_template_seeds_the_defaults() {
        let (plan, _) = real_templates().expect("Chief templates are installed");
        let seed = decode_plan_file(&plan).unwrap();
        let d = overlay(PlanDefaults::chief_x18_daniel(), &seed);
        assert!(seed.wall_types.len() >= 100, "{}", seed.wall_types.len());
        assert!(seed.text_styles.len() >= 12);
        assert!(seed.dimension_defaults.len() >= 14);
        assert_eq!(d.rooms.ceiling_height, 109.125);
        let quarter = d.text_styles.get("1/4\" Text Style").unwrap();
        assert_eq!((quarter.font.as_str(), quarter.height_in), ("Avenir", 4.5));
        assert!(d.dimension_set("1/4\" Scale").is_some());
        // DECISIONS DT1: Daniel's x17 working template brings 34 layer sets,
        // 14 dimension sets, 13 Rich Text sets and 20 saved plan views.
        if seed.file_name.starts_with("x17 Working Template") {
            assert_eq!(seed.layer_set_names.len(), 34, "{:?}", seed.layer_set_names);
            assert_eq!(seed.dimension_defaults.len(), 14);
            assert_eq!(seed.rich_text_defaults.len(), 13);
            assert_eq!(seed.plan_view_names.len(), 20);
            for n in &seed.layer_set_names {
                assert!(d.layer_sets.get(n).is_some(), "layer set {n}");
            }
            for n in &seed.plan_view_names {
                assert!(d.plan_views.iter().any(|v| &v.name == n), "view {n}");
            }
            let rich = d.saved.lists.get("rich_text").map_or(0, |l| l.items.len());
            assert!(rich >= 13, "{rich} Rich Text defaults");
        }
    }

    #[test]
    #[ignore = "reads Daniel's Chief templates"]
    fn real_first_run_detects_and_seeds_end_to_end() {
        let home = crate::paths::home_dir().expect("home");
        let scratch = temp_dir("e2e");
        let (settings, detected) = load_or_detect_at(&scratch.join("settings.json"), Some(&home));
        assert!(detected);
        let plan = settings.plan.clone().expect("plan template detected");
        let layout = settings.layout.clone().expect("layout template detected");
        println!("plan   = {}", plan.display());
        println!("layout = {}", layout.display());
        let first = refresh_with(
            SeedCache::default(),
            &settings,
            false,
            &mut decode_plan_file,
            &mut decode_layout_file,
        );
        assert!(first.decoded && first.notes.is_empty(), "{:?}", first.notes);
        let again = refresh_with(
            first.cache.clone(),
            &settings,
            false,
            &mut decode_plan_file,
            &mut decode_layout_file,
        );
        assert!(!again.decoded, "second pass must come from the cache");
        let p = first.cache.plan.as_ref().unwrap();
        for l in plan_summary_lines(p) {
            println!("{l}");
        }
        for l in layout_summary_lines(first.cache.layout.as_ref().unwrap()) {
            println!("{l}");
        }
        let d = defaults_from(&settings, &first.cache, PlanDefaults::chief_x18_daniel());
        let s = d.wall_type("Stucco-6").unwrap();
        println!("Stucco-6: {} layers, {} in", s.layers.len(), s.thickness());
        let t = d.text_styles.get("1/4\" Text Style").unwrap();
        println!("1/4\" Text Style: {} {} in", t.font, t.height_in);
        assert_eq!(d.rooms.ceiling_height, 109.125);
        assert_eq!(layout_sheet(first.cache.layout.as_ref()), SheetSize::ArchC);
        std::fs::remove_dir_all(&scratch).ok();
    }

    #[test]
    #[ignore = "reads Daniel's Chief templates"]
    fn real_layout_template_is_arch_c() {
        let (_, layout) = real_templates().expect("Chief templates are installed");
        let info = decode_layout_file(&layout).unwrap();
        assert_eq!(layout_sheet(Some(&info)), SheetSize::ArchC);
    }

    fn empty_detection(plan: Option<&str>) -> Detection {
        Detection {
            settings: TemplateSettings {
                plan: plan.map(PathBuf::from),
                layout: None,
                seed_from_chief: true,
            },
            defaults: PlanDefaults::chief_x18_daniel(),
            note: None,
            save_error: None,
        }
    }

    fn wait(start: &Startup) -> Detection {
        for _ in 0..500 {
            if let Some(d) = start.poll() {
                return d;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the scan did not finish");
    }

    #[test]
    fn the_first_launch_scan_runs_on_its_own_thread_and_only_once() {
        let start = Startup::new();
        let main_thread = std::thread::current().id();
        let seen = std::sync::Arc::new(Mutex::new(None));
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (seen2, calls2) = (seen.clone(), calls.clone());
        // The window does not wait: `begin` returns before the scan is done.
        let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
        let gate2 = gate.clone();
        assert!(start.begin(false, move || {
            gate2.wait();
            calls2.fetch_add(1, Ordering::SeqCst);
            *seen2.lock().unwrap() = Some(std::thread::current().id());
            empty_detection(Some("/found/plan.plan"))
        }));
        assert!(start.running());
        assert!(start.poll().is_none(), "nothing yet while the scan waits");
        gate.wait();
        let d = wait(&start);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_ne!(seen.lock().unwrap().unwrap(), main_thread);
        assert_eq!(
            d.settings.plan.as_deref(),
            Some(Path::new("/found/plan.plan"))
        );
        assert!(!start.running());
        // The result is the session's settings, and a second start is refused.
        assert_eq!(start.session(), Some(d.settings));
        assert!(!start.begin(false, || panic!("must not scan again")));
    }

    #[test]
    fn saved_settings_skip_the_scan() {
        let start = Startup::new();
        assert!(!start.begin(true, || panic!("saved settings need no scan")));
        assert!(!start.running());
        assert!(start.session().is_none());
    }

    #[test]
    fn a_failed_settings_write_does_not_scan_again_this_session() {
        let home = temp_dir("readonly-home");
        // The settings path is "inside" a file, so it can never be written.
        let blocker = home.join("blocker");
        std::fs::write(&blocker, "x").unwrap();
        let path = blocker.join("settings.json");
        let start = Startup::new();
        let p2 = path.clone();
        let h2 = home.clone();
        assert!(start.begin(false, move || {
            detect_and_seed(Some(&p2), Some(&h2), PlanDefaults::chief_x18_daniel())
        }));
        let d = wait(&start);
        assert!(d.save_error.is_some(), "the write must have failed");
        // The session keeps the result; nothing scans again.
        assert!(start.session().is_some());
        assert!(!start.begin(false, || panic!("rescanned after a failed write")));
        assert!(read_settings_at(&path).is_none());
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn the_detection_function_reads_templates_without_touching_the_main_thread() {
        // Chief's templates under a scratch home: detect_and_seed finds the
        // layout and plan files by name and saves the settings.
        let home = temp_dir("detect-home");
        let file = home.join(".plan-studio").join("settings.json");
        let d = detect_and_seed(Some(&file), Some(&home), PlanDefaults::chief_x18_daniel());
        assert!(d.save_error.is_none());
        assert_eq!(read_settings_at(&file), Some(d.settings.clone()));
        // With no Chief install nothing is found and the defaults are the base.
        assert!(d.settings.plan.is_none());
        assert_eq!(d.defaults, PlanDefaults::chief_x18_daniel());
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn the_templates_folder_preference_supplies_templates_chief_did_not_name() {
        use crate::dialogs::preferences::pages::{self, FolderKind};
        let home = temp_dir("own-home");
        let own = temp_dir("own-folder");
        std::fs::write(own.join("b.plan"), "x").unwrap();
        std::fs::write(own.join("a.plan"), "x").unwrap();
        std::fs::write(own.join("page.layout"), "x").unwrap();
        // Default folder: nothing found on a machine without Chief.
        assert!(detect_settings(Some(&home)).plan.is_none());
        pages::update(|p| p.folders.set(FolderKind::Templates, &own.to_string_lossy()));
        let s = detect_settings(Some(&home));
        assert_eq!(s.plan, Some(own.join("a.plan")));
        assert_eq!(s.layout, Some(own.join("page.layout")));
        pages::update(|p| p.folders.set(FolderKind::Templates, ""));
        assert!(detect_settings(Some(&home)).plan.is_none());
        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&own).ok();
    }
}
