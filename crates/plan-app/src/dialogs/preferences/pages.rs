//! The data behind the Preferences pages beyond the general ones, and the
//! `~/.plan-studio/preferences.json` file that keeps every page.
//!
//! The file is one JSON object: `version`, `general` (the [`Preferences`] of
//! text size, selection color, render defaults and so on) and one key per
//! page ([`PagePrefs`]). Every key has a serde default, so a file written by
//! an older build (or hand-trimmed) fills the rest from the defaults, and a
//! file from a newer build keeps its unknown keys out of the way. `version`
//! is [`FILE_VERSION`]; [`migrate`] is where an older layout is brought up to
//! date.
//!
//! The live copy is [`current`] / [`set`]. [`apply_runtime`] hands what has a
//! setter elsewhere (the marquee choice, the Chief catalog folder) to the
//! editor at once; the rest is read through the accessors at the bottom of
//! this file by the code that acts on it.

// The accessors at the bottom are read by code outside the Preferences dialog
// (the Library Browser, the 3D panel, the folders' readers; see the "Preferences
// round 14" item in docs/integration-queue.md) and not wired yet.
#![allow(dead_code)]

use super::Preferences;
use crate::tools::select::MarqueeMode;
use plan_core::defaults::EditingDefaults;
use plan_core::units::LengthUnit;
use plan_view3d::{Quality, ViewSettings};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The layout version this build writes.
pub const FILE_VERSION: u32 = 1;
/// The file name under `~/.plan-studio/`.
pub const FILE_NAME: &str = "preferences.json";

// ----- small choices -----

/// Size of the toolbar icons (Appearance > Icon size).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IconSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl IconSize {
    pub const ALL: [IconSize; 3] = [IconSize::Small, IconSize::Medium, IconSize::Large];

    pub fn label(self) -> &'static str {
        match self {
            IconSize::Small => "Small",
            IconSize::Medium => "Medium",
            IconSize::Large => "Large",
        }
    }

    /// Side of the icon in points; the button around it is 28.
    pub fn px(self) -> f32 {
        match self {
            IconSize::Small => 16.0,
            IconSize::Medium => 20.0,
            IconSize::Large => 24.0,
        }
    }
}

/// Size of the thumbnails in the Library Browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl PreviewSize {
    pub const ALL: [PreviewSize; 3] = [PreviewSize::Small, PreviewSize::Medium, PreviewSize::Large];

    pub fn label(self) -> &'static str {
        match self {
            PreviewSize::Small => "Small",
            PreviewSize::Medium => "Medium",
            PreviewSize::Large => "Large",
        }
    }

    pub fn px(self) -> f32 {
        match self {
            PreviewSize::Small => 32.0,
            PreviewSize::Medium => 48.0,
            PreviewSize::Large => 72.0,
        }
    }
}

/// Quality of the interactive 3D preview (Render > Preview quality).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewQuality {
    Low,
    #[default]
    Medium,
    High,
}

impl PreviewQuality {
    pub const ALL: [PreviewQuality; 3] = [
        PreviewQuality::Low,
        PreviewQuality::Medium,
        PreviewQuality::High,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PreviewQuality::Low => "Draft",
            PreviewQuality::Medium => "Normal",
            PreviewQuality::High => "High",
        }
    }

    pub fn quality(self) -> Quality {
        match self {
            PreviewQuality::Low => Quality::Low,
            PreviewQuality::Medium => Quality::Medium,
            PreviewQuality::High => Quality::High,
        }
    }
}

/// What a rotation turns about (Edit > Rotate about).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RotateAbout {
    /// The center of the selection.
    #[default]
    Center,
    /// The point the pointer pressed.
    Pointer,
}

impl RotateAbout {
    pub const ALL: [RotateAbout; 2] = [RotateAbout::Center, RotateAbout::Pointer];

    pub fn label(self) -> &'static str {
        match self {
            RotateAbout::Center => "Center of the selection",
            RotateAbout::Pointer => "Point where the pointer pressed",
        }
    }
}

/// What a resize holds still (Edit > Resize about).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizeAbout {
    /// The corner or edge opposite the handle.
    #[default]
    Opposite,
    /// The center of the selection.
    Center,
}

impl ResizeAbout {
    pub const ALL: [ResizeAbout; 2] = [ResizeAbout::Opposite, ResizeAbout::Center];

    pub fn label(self) -> &'static str {
        match self {
            ResizeAbout::Opposite => "Opposite handle",
            ResizeAbout::Center => "Center of the selection",
        }
    }
}

/// How the end of a CAD line is capped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndCap {
    #[default]
    Flat,
    Round,
    Square,
}

impl EndCap {
    pub const ALL: [EndCap; 3] = [EndCap::Flat, EndCap::Round, EndCap::Square];

    pub fn label(self) -> &'static str {
        match self {
            EndCap::Flat => "Flat",
            EndCap::Round => "Round",
            EndCap::Square => "Square",
        }
    }
}

// ----- the pages -----

/// Appearance: the canvas colors the user sets over the theme, and the icon
/// size. `None` leaves the canvas theme's own color.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearancePrefs {
    pub background: Option<[u8; 3]>,
    pub grid: Option<[u8; 3]>,
    pub text: Option<[u8; 3]>,
    /// The color of dimensions while they are drawn or dragged.
    pub temp_dim: Option<[u8; 3]>,
    pub icon_size: IconSize,
}

/// Library Browser: thumbnail size and what a search looks at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryBrowserPrefs {
    pub preview_size: PreviewSize,
    pub search_names: bool,
    pub search_descriptions: bool,
    pub search_keywords: bool,
    pub search_catalog_names: bool,
    /// A word must match whole, not as part of another word.
    pub whole_words: bool,
    /// Every word typed must match, not any of them.
    pub match_all_words: bool,
}

impl Default for LibraryBrowserPrefs {
    fn default() -> Self {
        Self {
            preview_size: PreviewSize::Medium,
            search_names: true,
            search_descriptions: true,
            search_keywords: true,
            search_catalog_names: false,
            whole_words: false,
            match_all_words: true,
        }
    }
}

/// Render: the 3D preview's quality and the shadow and occlusion switches
/// a new 3D view starts with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderPrefs {
    pub preview_quality: PreviewQuality,
    pub shadows: bool,
    pub ambient_occlusion: bool,
}

impl Default for RenderPrefs {
    fn default() -> Self {
        Self {
            preview_quality: PreviewQuality::Medium,
            shadows: true,
            ambient_occlusion: true,
        }
    }
}

/// Materials List: how the take-off is worked out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialsPrefs {
    /// Add each category's waste factor to the quantities.
    pub apply_waste: bool,
    /// Round the quantity to buy up to whole units.
    pub round_up: bool,
    /// Show the price columns and totals.
    pub show_prices: bool,
    /// List a line for every floor, not only the active one.
    pub all_floors: bool,
}

impl Default for MaterialsPrefs {
    fn default() -> Self {
        Self {
            apply_waste: true,
            round_up: true,
            show_prices: true,
            all_floors: false,
        }
    }
}

/// One of Chief's data folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderKind {
    Library,
    Textures,
    Backdrops,
    Templates,
    Autosave,
    UserLibrary,
}

impl FolderKind {
    pub const ALL: [FolderKind; 6] = [
        FolderKind::Library,
        FolderKind::Textures,
        FolderKind::Backdrops,
        FolderKind::Templates,
        FolderKind::Autosave,
        FolderKind::UserLibrary,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FolderKind::Library => "Library",
            FolderKind::Textures => "Textures",
            FolderKind::Backdrops => "Backdrops",
            FolderKind::Templates => "Templates",
            FolderKind::Autosave => "Autosave",
            FolderKind::UserLibrary => "User library",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            FolderKind::Library => {
                "Chief's install folder with Core, Bonus and Manufacturer Libraries"
            }
            FolderKind::Textures => "Chief's texture files, read when a material uses one",
            FolderKind::Backdrops => "Backdrop images for 3D views",
            FolderKind::Templates => "Plan and layout templates",
            FolderKind::Autosave => "Where autosaves go; empty keeps them with each plan",
            FolderKind::UserLibrary => "The folder holding User_Library.calib",
        }
    }
}

/// Folder overrides; an empty slot means the default for this machine.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderPrefs {
    pub textures: Option<String>,
    pub backdrops: Option<String>,
    pub templates: Option<String>,
    pub autosave: Option<String>,
    pub user_library: Option<String>,
}

impl FolderPrefs {
    /// The override of `kind` (the library folder is the Chief catalog
    /// setting, not kept here).
    pub fn get(&self, kind: FolderKind) -> Option<&str> {
        match kind {
            FolderKind::Library => None,
            FolderKind::Textures => self.textures.as_deref(),
            FolderKind::Backdrops => self.backdrops.as_deref(),
            FolderKind::Templates => self.templates.as_deref(),
            FolderKind::Autosave => self.autosave.as_deref(),
            FolderKind::UserLibrary => self.user_library.as_deref(),
        }
        .filter(|s| !s.trim().is_empty())
    }

    /// Sets (or, with empty text, clears) the override of `kind`.
    pub fn set(&mut self, kind: FolderKind, text: &str) {
        let v = (!text.trim().is_empty()).then(|| text.trim().to_string());
        match kind {
            FolderKind::Library => {}
            FolderKind::Textures => self.textures = v,
            FolderKind::Backdrops => self.backdrops = v,
            FolderKind::Templates => self.templates = v,
            FolderKind::Autosave => self.autosave = v,
            FolderKind::UserLibrary => self.user_library = v,
        }
    }
}

/// A folder as the Folders page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderStatus {
    pub kind: FolderKind,
    /// The folder in force; `None` for the autosave folder that follows each
    /// plan.
    pub path: Option<PathBuf>,
    /// The user set this one; otherwise it is the default.
    pub custom: bool,
    /// The folder is there (true when nothing needs to be).
    pub exists: bool,
}

/// Chief's data folder (`~/Documents/Chief Architect Premier X18 Data`, or
/// X17's when only that one exists).
pub fn chief_data_folder(home: &Path) -> PathBuf {
    let docs = home.join("Documents");
    ["X18", "X17"]
        .iter()
        .map(|v| docs.join(format!("Chief Architect Premier {v} Data")))
        .find(|p| p.is_dir())
        .unwrap_or_else(|| docs.join("Chief Architect Premier X18 Data"))
}

/// The default of `kind` on this machine.
pub fn default_folder(kind: FolderKind, home: Option<&Path>) -> Option<PathBuf> {
    match kind {
        FolderKind::Library => {
            let base = Path::new("/Library/Application Support");
            ["X18", "X17"]
                .iter()
                .map(|v| base.join(format!("Chief Architect Premier {v}")))
                .find(|p| p.is_dir())
                .or_else(|| Some(base.join("Chief Architect Premier X18")))
        }
        FolderKind::Textures => home.map(|h| chief_data_folder(h).join("Textures")),
        FolderKind::Backdrops => home.map(|h| chief_data_folder(h).join("Backdrops")),
        FolderKind::Templates => home.map(|h| chief_data_folder(h).join("Templates")),
        FolderKind::Autosave => None,
        FolderKind::UserLibrary => home.map(|h| chief_data_folder(h).join("Database Libraries")),
    }
}

/// The folder in force for `kind`, with its "exists" indicator. `library`
/// is the Chief catalog setting's folder, which the Library page also sets.
pub fn folder_status(
    prefs: &FolderPrefs,
    library: Option<&Path>,
    kind: FolderKind,
    home: Option<&Path>,
) -> FolderStatus {
    let custom_path = if kind == FolderKind::Library {
        library.map(Path::to_path_buf)
    } else {
        prefs.get(kind).map(PathBuf::from)
    };
    let custom = custom_path.is_some();
    let path = custom_path.or_else(|| default_folder(kind, home));
    let exists = path.as_deref().is_none_or(Path::is_dir);
    FolderStatus {
        kind,
        path,
        custom,
        exists,
    }
}

/// Edit: what rotating and resizing hold fixed, the marquee choice and the
/// master switches of the snaps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditPrefs {
    pub rotate_about: RotateAbout,
    pub resize_about: ResizeAbout,
    /// `direction`, `enclosing` or `touching`.
    pub marquee: String,
}

impl Default for EditPrefs {
    fn default() -> Self {
        Self {
            rotate_about: RotateAbout::Center,
            resize_about: ResizeAbout::Opposite,
            marquee: "direction".into(),
        }
    }
}

impl EditPrefs {
    pub fn marquee_mode(&self) -> MarqueeMode {
        match self.marquee.as_str() {
            "enclosing" => MarqueeMode::Enclosing,
            "touching" => MarqueeMode::Touching,
            _ => MarqueeMode::ByDirection,
        }
    }

    pub fn set_marquee_mode(&mut self, m: MarqueeMode) {
        self.marquee = match m {
            MarqueeMode::ByDirection => "direction",
            MarqueeMode::Enclosing => "enclosing",
            MarqueeMode::Touching => "touching",
        }
        .into();
    }
}

/// Behaviors: the step sizes of the camera moves, which Chief keeps with the
/// edit behaviors (the Edit Type choice itself is in [`PagePrefs::editing`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorPrefs {
    /// Inches one Move Camera step goes.
    pub camera_move_in: f64,
    /// Degrees one Turn or Orbit step goes.
    pub camera_turn_deg: f64,
    /// Degrees one Tilt step goes.
    pub camera_tilt_deg: f64,
}

impl Default for BehaviorPrefs {
    fn default() -> Self {
        Self {
            camera_move_in: 24.0,
            camera_turn_deg: 15.0,
            camera_tilt_deg: 5.0,
        }
    }
}

/// Architectural: what the program rebuilds on its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArchitecturalPrefs {
    pub auto_rebuild_roofs: bool,
    pub auto_rebuild_walls: bool,
    pub auto_rebuild_foundations: bool,
    pub auto_rebuild_attic_walls: bool,
    /// A rebuild deletes roof planes no wall holds up any more.
    pub delete_unused_roof_planes: bool,
    /// A new plan starts from code-legal defaults (`editor::code`).
    pub seed_code_defaults: bool,
    /// Plan Check re-runs the rule groups an edit touched and shows the count.
    pub check_while_drawing: bool,
}

impl Default for ArchitecturalPrefs {
    fn default() -> Self {
        Self {
            auto_rebuild_roofs: true,
            auto_rebuild_walls: true,
            auto_rebuild_foundations: true,
            auto_rebuild_attic_walls: true,
            delete_unused_roof_planes: false,
            seed_code_defaults: true,
            check_while_drawing: true,
        }
    }
}

/// CAD: arc centers, line end caps and the line weights Chief keeps in its
/// preferences (`... Line Weight`, in 1/100 pt units as the INI has them).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CadPrefs {
    /// Mark the center of a selected arc or circle.
    pub show_arc_centers: bool,
    pub end_caps: EndCap,
    /// Setting name to weight; Daniel's X18 values at first.
    pub line_weights: BTreeMap<String, u32>,
}

impl Default for CadPrefs {
    fn default() -> Self {
        Self {
            show_arc_centers: true,
            end_caps: EndCap::Flat,
            line_weights: plan_config::daniel_x18().line_weights.into_iter().collect(),
        }
    }
}

/// Unit Conversions: the converter's starting unit and its rounding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnitPrefs {
    pub input_unit: LengthUnit,
    /// Smallest fraction (8, 16, 32, 64) the feet-inches result shows.
    pub fraction_denominator: u32,
    /// Decimals in metric and decimal-feet results.
    pub decimals: u32,
}

impl Default for UnitPrefs {
    fn default() -> Self {
        Self {
            input_unit: LengthUnit::FeetInches,
            fraction_denominator: 16,
            decimals: 2,
        }
    }
}

/// Every page but the general one.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PagePrefs {
    pub appearance: AppearancePrefs,
    pub library_browser: LibraryBrowserPrefs,
    pub render: RenderPrefs,
    pub materials: MaterialsPrefs,
    pub folders: FolderPrefs,
    pub edit: EditPrefs,
    pub behaviors: BehaviorPrefs,
    /// The snap kinds and sensitivity, angle snaps, Edit Type and Replicate
    /// choices the user saved. Used only when [`PagePrefs::editing_saved`].
    pub editing: EditingDefaults,
    /// The user has changed an editing default on a page; before that the
    /// plan's own defaults stand.
    pub editing_saved: bool,
    pub architectural: ArchitecturalPrefs,
    pub cad: CadPrefs,
    pub units: UnitPrefs,
    /// Messages the user asked not to be shown again (Reset Options clears).
    pub dont_ask_again: Vec<String>,
}

/// The whole file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrefsFile {
    pub version: u32,
    pub general: Preferences,
    #[serde(flatten)]
    pub pages: PagePrefs,
}

impl Default for PrefsFile {
    fn default() -> Self {
        Self {
            version: FILE_VERSION,
            general: Preferences::default(),
            pages: PagePrefs::default(),
        }
    }
}

/// Brings a file read from an older layout up to [`FILE_VERSION`]. Version 0
/// (a file with no `version` key written by hand) has nothing to change yet;
/// a newer file is left as it is so that saving does not lower its version.
pub fn migrate(file: &mut PrefsFile) {
    if file.version < FILE_VERSION {
        file.version = FILE_VERSION;
    }
}

/// `~/.plan-studio/preferences.json`.
pub fn file_path() -> Option<PathBuf> {
    crate::paths::user_file(FILE_NAME)
}

/// Reads the file at `path`; `None` when it is missing or is not JSON.
pub fn read_file_at(path: &Path) -> Option<PrefsFile> {
    let text = std::fs::read_to_string(path).ok()?;
    parse_file(&text)
}

/// Parses the text of a preferences file.
pub fn parse_file(text: &str) -> Option<PrefsFile> {
    let mut f: PrefsFile = serde_json::from_str(text).ok()?;
    migrate(&mut f);
    Some(f)
}

/// Writes the file at `path`.
pub fn write_file_at(path: &Path, file: &PrefsFile) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

// ----- the live copy -----

/// The page preferences in force.
pub fn current() -> PagePrefs {
    super::live(|l| {
        l.pages
            .get_or_insert_with(|| {
                #[cfg(test)]
                {
                    PagePrefs::default()
                }
                #[cfg(not(test))]
                {
                    file_path()
                        .and_then(|p| read_file_at(&p))
                        .map(|f| f.pages)
                        .unwrap_or_default()
                }
            })
            .clone()
    })
}

/// Replaces the page preferences, applies them to the editor and marks the
/// file for saving.
pub fn set(p: PagePrefs) {
    apply_runtime(&p);
    super::live(|l| {
        if l.pages.as_ref() != Some(&p) {
            l.dirty = true;
        }
        l.pages = Some(p);
    });
}

/// Changes the live page preferences with `f`.
pub fn update(f: impl FnOnce(&mut PagePrefs)) {
    let mut p = current();
    f(&mut p);
    set(p);
}

thread_local! {
    /// The icon side the toolbars draw with; read for every icon every frame.
    static ICON_PX: std::cell::Cell<f32> = const { std::cell::Cell::new(20.0) };
}

/// Hands the choices that have a setter elsewhere to the editor.
pub fn apply_runtime(p: &PagePrefs) {
    crate::tools::select::set_marquee_mode(p.edit.marquee_mode());
    ICON_PX.with(|c| c.set(p.appearance.icon_size.px()));
}

/// Lays the saved editing defaults (snaps, Edit Type, Replicate) over the
/// plan's, when the user has saved any. Call after a plan is opened or
/// started and once at startup; returns true when it changed anything.
pub fn apply_editing(editing: &mut EditingDefaults) -> bool {
    let p = current();
    if !p.editing_saved || *editing == p.editing {
        return false;
    }
    *editing = p.editing;
    true
}

/// Keeps `editing` (what a page just changed) as the saved editing defaults.
pub fn remember_editing(editing: &EditingDefaults) {
    update(|p| {
        p.editing = editing.clone();
        p.editing_saved = true;
    });
}

// ----- accessors for the code that acts on a page -----

/// Icon side in points (Appearance > Icon size).
pub fn icon_px() -> f32 {
    ICON_PX.with(std::cell::Cell::get)
}

/// Thumbnail side in the Library Browser, points.
pub fn library_preview_px() -> f32 {
    current().library_browser.preview_size.px()
}

/// The 3D view options a new view starts with.
pub fn view_settings() -> ViewSettings {
    let r = current().render;
    ViewSettings {
        shadows: r.shadows,
        ambient_occlusion: r.ambient_occlusion,
        quality: r.preview_quality.quality(),
        ..ViewSettings::default()
    }
}

/// The user's color for dimensions being drawn, when set.
pub fn temp_dim_color() -> Option<[u8; 3]> {
    current().appearance.temp_dim
}

/// The folder in force for `kind`, or `None` when there is no home folder
/// (and no override) or the autosave follows each plan.
pub fn folder(kind: FolderKind) -> Option<PathBuf> {
    let library = super::chief_folder();
    folder_status(
        &current().folders,
        library.as_deref(),
        kind,
        crate::paths::home_dir().as_deref(),
    )
    .path
}

/// Is "don't ask again" set for the message `key`?
pub fn dont_ask(key: &str) -> bool {
    current().dont_ask_again.iter().any(|k| k == key)
}

/// Sets "don't ask again" for the message `key`.
pub fn set_dont_ask(key: &str) {
    update(|p| {
        if !p.dont_ask_again.iter().any(|k| k == key) {
            p.dont_ask_again.push(key.to_string());
        }
    });
}

/// Reset Options: every message shows again. Returns how many were hidden.
pub fn reset_dont_ask() -> usize {
    let n = current().dont_ask_again.len();
    update(|p| p.dont_ask_again.clear());
    n
}

/// The weight of the CAD line-weight setting `name`.
pub fn line_weight(name: &str) -> Option<u32> {
    current().cad.line_weights.get(name).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::defaults::EditBehavior;

    #[test]
    fn the_file_round_trips_every_page() {
        let mut f = PrefsFile::default();
        f.general.text_size_pct = 120;
        f.pages.appearance.background = Some([1, 2, 3]);
        f.pages.appearance.icon_size = IconSize::Large;
        f.pages.library_browser.preview_size = PreviewSize::Large;
        f.pages.library_browser.match_all_words = false;
        f.pages.render.preview_quality = PreviewQuality::High;
        f.pages.render.shadows = false;
        f.pages.materials.round_up = false;
        f.pages.folders.set(FolderKind::Textures, "/tmp/tex");
        f.pages.edit.set_marquee_mode(MarqueeMode::Touching);
        f.pages.edit.rotate_about = RotateAbout::Pointer;
        f.pages.behaviors.camera_move_in = 12.0;
        f.pages.editing.snap_distance_px = 17.0;
        f.pages.editing.behavior.mode = EditBehavior::Replicate;
        f.pages.editing_saved = true;
        f.pages.architectural.delete_unused_roof_planes = true;
        f.pages.cad.end_caps = EndCap::Round;
        f.pages
            .cad
            .line_weights
            .insert("Layout Edge Line Weight".into(), 30);
        f.pages.units.input_unit = LengthUnit::Millimeters;
        f.pages.dont_ask_again.push("delete.floor".into());
        let text = serde_json::to_string_pretty(&f).unwrap();
        assert_eq!(parse_file(&text), Some(f));
    }

    #[test]
    fn a_partial_or_old_file_fills_the_rest_from_the_defaults() {
        let f = parse_file(r#"{"render": {"shadows": false}}"#).unwrap();
        assert_eq!(
            f.version, FILE_VERSION,
            "a file with no version is brought up"
        );
        assert!(!f.pages.render.shadows && f.pages.render.ambient_occlusion);
        assert_eq!(f.pages.library_browser, LibraryBrowserPrefs::default());
        assert_eq!(f.general, Preferences::default());
        assert!(!f.pages.editing_saved);
        // Unknown keys (from a newer build) are ignored; its version stays.
        let g =
            parse_file(r#"{"version": 9, "future": {"x": 1}, "units": {"decimals": 4}}"#).unwrap();
        assert_eq!(g.version, 9);
        assert_eq!(g.pages.units.decimals, 4);
        assert_eq!(g.pages.units.fraction_denominator, 16);
        assert_eq!(parse_file("not json"), None);
    }

    #[test]
    fn the_file_is_written_and_read_back() {
        let dir =
            std::env::temp_dir().join(format!("plan-studio-prefs-r14-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sub").join(FILE_NAME);
        assert_eq!(read_file_at(&path), None);
        let mut f = PrefsFile::default();
        f.pages.cad.show_arc_centers = false;
        write_file_at(&path, &f).unwrap();
        assert_eq!(read_file_at(&path), Some(f));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn marquee_choice_round_trips_through_its_key() {
        let mut e = EditPrefs::default();
        for m in MarqueeMode::ALL {
            e.set_marquee_mode(m);
            assert_eq!(e.marquee_mode(), m);
        }
        e.marquee = "nonsense".into();
        assert_eq!(e.marquee_mode(), MarqueeMode::ByDirection);
    }

    #[test]
    fn folders_show_the_default_or_the_override_and_whether_it_exists() {
        let dir = std::env::temp_dir().join(format!("plan-studio-folders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let data = dir.join("Documents/Chief Architect Premier X18 Data");
        std::fs::create_dir_all(data.join("Textures")).unwrap();
        let mut prefs = FolderPrefs::default();
        let tex = folder_status(&prefs, None, FolderKind::Textures, Some(&dir));
        assert!(!tex.custom && tex.exists);
        assert_eq!(tex.path, Some(data.join("Textures")));
        let back = folder_status(&prefs, None, FolderKind::Backdrops, Some(&dir));
        assert!(!back.exists, "no Backdrops folder here");
        // An override wins and has its own indicator.
        prefs.set(
            FolderKind::Backdrops,
            &format!("  {}  ", data.join("Textures").display()),
        );
        let back = folder_status(&prefs, None, FolderKind::Backdrops, Some(&dir));
        assert!(back.custom && back.exists);
        prefs.set(FolderKind::Backdrops, "");
        assert_eq!(prefs.get(FolderKind::Backdrops), None);
        // The autosave follows each plan, so nothing needs to exist.
        let auto = folder_status(&prefs, None, FolderKind::Autosave, Some(&dir));
        assert!(auto.path.is_none() && auto.exists);
        // The library folder is the Chief catalog setting's.
        let lib = folder_status(
            &prefs,
            Some(&dir.join("nowhere")),
            FolderKind::Library,
            None,
        );
        assert!(lib.custom && !lib.exists);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_live_copy_applies_the_marquee_and_marks_the_file_dirty() {
        super::super::live(|l| l.dirty = false);
        update(|p| p.edit.set_marquee_mode(MarqueeMode::Enclosing));
        assert_eq!(crate::tools::select::marquee_mode(), MarqueeMode::Enclosing);
        assert!(super::super::live(|l| l.dirty));
        set(PagePrefs::default());
        assert_eq!(
            crate::tools::select::marquee_mode(),
            MarqueeMode::ByDirection
        );
        super::super::live(|l| l.dirty = false);
    }

    #[test]
    fn saved_editing_defaults_go_over_the_plans_only_once_saved() {
        set(PagePrefs::default());
        let mut plan = EditingDefaults {
            snap_distance_px: 25.0,
            ..EditingDefaults::default()
        };
        assert!(!apply_editing(&mut plan), "nothing saved: the plan stands");
        assert_eq!(plan.snap_distance_px, 25.0);
        let mine = EditingDefaults {
            snap_distance_px: 6.0,
            angle_snap_deg: 45.0,
            ..EditingDefaults::default()
        };
        remember_editing(&mine);
        assert!(apply_editing(&mut plan));
        assert_eq!(plan, mine);
        assert!(!apply_editing(&mut plan), "already the same");
        set(PagePrefs::default());
        super::super::live(|l| l.dirty = false);
    }

    #[test]
    fn dont_ask_again_is_kept_and_reset() {
        set(PagePrefs::default());
        assert!(!dont_ask("delete.floor"));
        set_dont_ask("delete.floor");
        set_dont_ask("delete.floor");
        set_dont_ask("close.unsaved");
        assert!(dont_ask("delete.floor") && dont_ask("close.unsaved"));
        assert_eq!(current().dont_ask_again.len(), 2);
        assert_eq!(reset_dont_ask(), 2);
        assert!(!dont_ask("delete.floor"));
        set(PagePrefs::default());
        super::super::live(|l| l.dirty = false);
    }

    #[test]
    fn accessors_follow_the_pages() {
        set(PagePrefs::default());
        assert_eq!(icon_px(), 20.0);
        assert_eq!(library_preview_px(), 48.0);
        assert_eq!(view_settings(), ViewSettings::default());
        update(|p| {
            p.appearance.icon_size = IconSize::Small;
            p.library_browser.preview_size = PreviewSize::Large;
            p.render.preview_quality = PreviewQuality::High;
            p.render.shadows = false;
            p.appearance.temp_dim = Some([9, 8, 7]);
        });
        assert_eq!(icon_px(), 16.0);
        assert_eq!(library_preview_px(), 72.0);
        let v = view_settings();
        assert!(!v.shadows && v.ambient_occlusion && v.quality == Quality::High);
        assert_eq!(temp_dim_color(), Some([9, 8, 7]));
        assert_eq!(line_weight("Minimum Display Line Weight"), Some(1));
        set(PagePrefs::default());
        super::super::live(|l| l.dirty = false);
    }
}
