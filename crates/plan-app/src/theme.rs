//! Canvas themes, the chrome (panel) colors, the UI scale and persisted app
//! settings.
//!
//! The canvas default is a low-glare warm gray so the main drawing area is
//! comfortable for people who are sensitive to light and brightness. The
//! chrome is dark with near-white text (pure white in the High Contrast
//! theme); every foreground/background pair of it is checked against the
//! WCAG contrast ratio in the tests below.
//!
//! The UI scale (100 to 175 percent) is egui's zoom factor: text, toolbar
//! buttons, dock widths, dialogs, handles and the icons (which egui_extras
//! rasterizes at points times pixels-per-point, so they stay crisp) all grow
//! together. [`sync_scale`] keeps `AppSettings::ui_scale_pct` and the zoom
//! factor in step, whoever changed it (this module, Preferences > Appearance
//! or eframe's Cmd+Plus / Cmd+Minus).

use eframe::egui::{self, Color32, Stroke};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Panel gray used by the dark Chief-like chrome (before brightness scaling).
const PANEL_GRAY: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3A);
const HOVER_GRAY: Color32 = Color32::from_rgb(0x4A, 0x4A, 0x4A);

pub const BRIGHTNESS_MIN: f32 = 0.6;
pub const BRIGHTNESS_MAX: f32 = 1.0;

/// Which colors the drawing canvas uses.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CanvasTheme {
    Paper,
    LowGlare,
    Dark,
    HighContrast,
}

/// Every color the canvas painter needs.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub background: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub wall_fill_exterior: Color32,
    pub wall_fill_interior: Color32,
    pub wall_stroke: Color32,
    /// Jamb lines, window lines and the door leaf.
    pub opening_line: Color32,
    pub door_arc: Color32,
    pub room_outline: Color32,
    pub room_label: Color32,
    pub selection: Color32,
    pub hover: Color32,
    pub ghost_fill: Color32,
    pub ghost_stroke: Color32,
    pub dimension_text: Color32,
    pub origin_marker: Color32,
    pub text: Color32,
}

impl CanvasTheme {
    pub const ALL: [CanvasTheme; 4] = [
        CanvasTheme::Paper,
        CanvasTheme::LowGlare,
        CanvasTheme::Dark,
        CanvasTheme::HighContrast,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CanvasTheme::Paper => "Paper",
            CanvasTheme::LowGlare => "Low Glare",
            CanvasTheme::Dark => "Dark",
            CanvasTheme::HighContrast => "High Contrast",
        }
    }

    fn key(self) -> &'static str {
        match self {
            CanvasTheme::Paper => "paper",
            CanvasTheme::LowGlare => "low_glare",
            CanvasTheme::Dark => "dark",
            CanvasTheme::HighContrast => "high_contrast",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == key)
    }

    pub fn palette(self) -> Palette {
        let rgb = Color32::from_rgb;
        match self {
            CanvasTheme::Paper => Palette {
                background: rgb(250, 250, 247),
                grid_minor: Color32::from_gray(232),
                grid_major: Color32::from_gray(215),
                wall_fill_exterior: Color32::from_gray(190),
                wall_fill_interior: Color32::from_gray(215),
                wall_stroke: Color32::BLACK,
                opening_line: Color32::BLACK,
                door_arc: Color32::from_gray(90),
                room_outline: rgb(90, 130, 190),
                room_label: rgb(60, 80, 120),
                // #CD5F00: 3.8:1 on the paper (the old #FF8C00 was 2.2:1).
                selection: rgb(205, 95, 0),
                hover: rgb(40, 140, 255),
                ghost_fill: Color32::from_rgba_unmultiplied(80, 140, 255, 90),
                ghost_stroke: rgb(40, 100, 220),
                dimension_text: rgb(20, 60, 160),
                origin_marker: rgb(200, 60, 60),
                text: rgb(30, 30, 30),
            },
            CanvasTheme::LowGlare => Palette {
                background: rgb(196, 192, 184),
                grid_minor: rgb(176, 172, 164),
                grid_major: rgb(160, 156, 148),
                wall_fill_exterior: rgb(120, 116, 110),
                wall_fill_interior: rgb(150, 146, 140),
                wall_stroke: rgb(40, 40, 40),
                opening_line: rgb(40, 40, 40),
                door_arc: rgb(70, 70, 70),
                room_outline: rgb(80, 100, 140),
                room_label: rgb(45, 55, 75),
                // #AA4600: 3.2:1 on the warm gray (the old #C86414 was 2.2:1).
                selection: rgb(170, 70, 0),
                hover: rgb(40, 100, 170),
                ghost_fill: Color32::from_rgba_unmultiplied(60, 100, 170, 80),
                ghost_stroke: rgb(40, 80, 150),
                dimension_text: rgb(30, 50, 110),
                origin_marker: rgb(150, 60, 60),
                text: rgb(30, 30, 30),
            },
            CanvasTheme::Dark => Palette {
                background: rgb(38, 40, 44),
                grid_minor: rgb(52, 54, 58),
                grid_major: rgb(66, 68, 72),
                wall_fill_exterior: rgb(190, 186, 180),
                wall_fill_interior: rgb(150, 146, 140),
                wall_stroke: rgb(230, 230, 230),
                opening_line: rgb(230, 230, 230),
                door_arc: rgb(170, 170, 170),
                room_outline: rgb(110, 140, 190),
                room_label: rgb(150, 175, 215),
                selection: rgb(255, 170, 60),
                hover: rgb(90, 160, 255),
                ghost_fill: Color32::from_rgba_unmultiplied(90, 150, 255, 80),
                ghost_stroke: rgb(110, 170, 255),
                dimension_text: rgb(150, 190, 255),
                origin_marker: rgb(210, 90, 90),
                text: rgb(220, 220, 220),
            },
            CanvasTheme::HighContrast => Palette {
                background: rgb(255, 255, 255),
                grid_minor: rgb(200, 200, 200),
                grid_major: rgb(150, 150, 150),
                wall_fill_exterior: rgb(0, 0, 0),
                wall_fill_interior: rgb(0, 0, 0),
                wall_stroke: rgb(255, 255, 255),
                opening_line: rgb(0, 0, 0),
                door_arc: rgb(0, 0, 0),
                room_outline: rgb(0, 0, 140),
                room_label: rgb(0, 0, 0),
                selection: rgb(255, 0, 0),
                hover: rgb(0, 0, 255),
                ghost_fill: Color32::from_rgba_unmultiplied(0, 0, 255, 70),
                ghost_stroke: rgb(0, 0, 255),
                dimension_text: rgb(0, 0, 0),
                origin_marker: rgb(255, 0, 0),
                text: rgb(0, 0, 0),
            },
        }
    }
}

// ----- contrast -----

/// WCAG relative luminance of an sRGB color (alpha ignored).
#[cfg(test)]
pub fn luminance(c: Color32) -> f32 {
    let lin = |v: u8| {
        let s = f32::from(v) / 255.0;
        if s <= 0.039_28 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG contrast ratio of two colors, 1.0 (identical) to 21.0 (black/white).
#[cfg(test)]
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

// ----- chrome colors -----

/// The colors of the panels, toolbars, docks, status bar and dialogs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChromeColors {
    /// Panel and window fill.
    pub panel: Color32,
    /// Fill under the pointer.
    pub hover: Color32,
    /// Fill of the active (pressed, selected) widget; text on it is `text`.
    pub accent: Color32,
    /// Widget outlines and the focus / hover ring.
    pub outline: Color32,
    /// Text and icon ink.
    pub text: Color32,
    /// The status bar strip.
    pub status: Color32,
    /// Fill of the active toolbar button.
    pub active: Color32,
}

impl ChromeColors {
    /// Every foreground / background pair of the chrome with the ratio it has
    /// to reach: `(name, foreground, background, minimum)`.
    #[cfg(test)]
    pub fn pairs(&self) -> Vec<(&'static str, Color32, Color32, f32)> {
        vec![
            ("text on panel", self.text, self.panel, 7.0),
            ("text on hover", self.text, self.hover, 4.5),
            ("text on accent", self.text, self.accent, 3.0),
            ("text on status bar", self.text, self.status, 7.0),
            ("text on active button", self.text, self.active, 4.5),
            ("outline on panel", self.outline, self.panel, 3.0),
            ("accent on panel", self.accent, self.panel, 3.0),
        ]
    }
}

/// Dark, Chief-like chrome. `HighContrast` is pure black with pure white text.
pub fn chrome_colors(theme: CanvasTheme) -> ChromeColors {
    let rgb = Color32::from_rgb;
    match theme {
        CanvasTheme::HighContrast => ChromeColors {
            panel: rgb(0, 0, 0),
            hover: rgb(0x2E, 0x2E, 0x2E),
            accent: rgb(0x0B, 0x57, 0xD0),
            outline: rgb(0xD8, 0xD8, 0xD8),
            text: Color32::WHITE,
            status: rgb(0, 0, 0),
            active: rgb(0x0B, 0x57, 0xD0),
        },
        _ => ChromeColors {
            panel: PANEL_GRAY,
            hover: HOVER_GRAY,
            accent: UI_ACCENT,
            outline: UI_OUTLINE,
            text: UI_TEXT,
            status: rgb(0x2C, 0x2C, 0x2C),
            active: rgb(0x5A, 0x5A, 0x5A),
        },
    }
}

thread_local! {
    static CURRENT_CHROME: Cell<Option<ChromeColors>> = const { Cell::new(None) };
}

/// The chrome colors last applied by [`apply_settings`], for code that paints
/// its own fills (toolbar buttons, the status bar, dock tabs).
pub fn current_chrome() -> ChromeColors {
    CURRENT_CHROME
        .with(Cell::get)
        .unwrap_or_else(|| chrome_colors(CanvasTheme::LowGlare))
}

// ----- UI scale and motion -----

/// Smallest and largest UI scale, percent of normal.
pub const UI_SCALE_MIN: u32 = 100;
pub const UI_SCALE_MAX: u32 = 175;
/// The scale choices the Appearance controls offer.
pub const UI_SCALE_STEPS: [u32; 4] = [100, 125, 150, 175];
/// Body text size in points: the minimum, and the "Larger text" option.
pub const TEXT_PX_BASE: f32 = 15.0;
pub const TEXT_PX_LARGE: f32 = 17.0;

/// Clamps a scale percentage to the supported range.
pub fn clamp_scale(pct: u32) -> u32 {
    pct.clamp(UI_SCALE_MIN, UI_SCALE_MAX)
}

/// Dock widths in points, and their limits.
pub const DOCK_WIDTH_MIN: f32 = 220.0;
pub const DOCK_WIDTH_MAX: f32 = 700.0;

/// The widths the user dragged the side panels to.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DockWidths {
    pub library: f32,
    pub project: f32,
    pub layers: f32,
    pub properties: f32,
}

impl Default for DockWidths {
    fn default() -> Self {
        Self {
            library: 300.0,
            project: 300.0,
            layers: 300.0,
            properties: 240.0,
        }
    }
}

impl DockWidths {
    /// The saved width of the side panel for `dock`.
    pub fn get(&self, dock: crate::toolbar::Dock) -> f32 {
        use crate::toolbar::Dock;
        match dock {
            Dock::Library => self.library,
            Dock::Project => self.project,
            Dock::LayerDisplay => self.layers,
        }
    }

    /// Stores the width of the side panel for `dock` (clamped); true when it
    /// changed by more than half a point.
    pub fn set(&mut self, dock: crate::toolbar::Dock, width: f32) -> bool {
        use crate::toolbar::Dock;
        let width = Self {
            library: width,
            ..*self
        }
        .clamped()
        .library;
        let slot = match dock {
            Dock::Library => &mut self.library,
            Dock::Project => &mut self.project,
            Dock::LayerDisplay => &mut self.layers,
        };
        let changed = (*slot - width).abs() > 0.5;
        if changed {
            *slot = width;
        }
        changed
    }

    fn clamped(self) -> Self {
        let c = |w: f32| {
            if w.is_finite() {
                w.clamp(DOCK_WIDTH_MIN, DOCK_WIDTH_MAX)
            } else {
                DOCK_WIDTH_MIN
            }
        };
        Self {
            library: c(self.library),
            project: c(self.project),
            layers: c(self.layers),
            properties: c(self.properties),
        }
    }
}

/// Settings persisted between runs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AppSettings {
    pub theme: CanvasTheme,
    /// Global UI brightness, 0.6..=1.0.
    pub brightness: f32,
    /// UI scale in percent, 100..=175 (text, toolbars, docks, dialogs, icons).
    pub ui_scale_pct: u32,
    /// Body text 17 pt instead of 15 pt.
    pub large_text: bool,
    /// No animated transitions (collapsing sections, scrolling, fades).
    pub reduce_motion: bool,
    /// Widths of the side panels.
    pub dock_widths: DockWidths,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: CanvasTheme::LowGlare,
            brightness: 1.0,
            ui_scale_pct: UI_SCALE_MIN,
            large_text: false,
            reduce_motion: false,
            dock_widths: DockWidths::default(),
        }
    }
}

impl AppSettings {
    /// Body text size in points.
    pub fn body_px(&self) -> f32 {
        if self.large_text {
            TEXT_PX_LARGE
        } else {
            TEXT_PX_BASE
        }
    }

    /// `~/.plan-studio/settings.json`, or `None` when no home directory is
    /// known (`HOME`, else `USERPROFILE` on Windows).
    fn path() -> Option<PathBuf> {
        crate::paths::user_file("settings.json")
    }

    /// Loads the saved settings; any problem falls back to the defaults.
    pub fn load() -> Self {
        match Self::path() {
            Some(p) => Self::load_at(&p),
            None => Self::default(),
        }
    }

    /// [`AppSettings::load`] from `path`.
    pub fn load_at(path: &Path) -> Self {
        let mut s = Self::default();
        let Some(text) = std::fs::read_to_string(path).ok() else {
            return s;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            return s;
        };
        if let Some(t) = v
            .get("theme")
            .and_then(|t| t.as_str())
            .and_then(CanvasTheme::from_key)
        {
            s.theme = t;
        }
        if let Some(b) = v.get("brightness").and_then(|b| b.as_f64()) {
            s.brightness = (b as f32).clamp(BRIGHTNESS_MIN, BRIGHTNESS_MAX);
        }
        if let Some(p) = v.get("ui_scale_pct").and_then(|p| p.as_u64()) {
            s.ui_scale_pct = clamp_scale(u32::try_from(p).unwrap_or(UI_SCALE_MAX));
        }
        if let Some(b) = v.get("large_text").and_then(|b| b.as_bool()) {
            s.large_text = b;
        }
        if let Some(b) = v.get("reduce_motion").and_then(|b| b.as_bool()) {
            s.reduce_motion = b;
        }
        if let Some(d) = v.get("dock_widths") {
            let get =
                |k: &str, dflt: f32| d.get(k).and_then(|w| w.as_f64()).map_or(dflt, |w| w as f32);
            let dflt = DockWidths::default();
            s.dock_widths = DockWidths {
                library: get("library", dflt.library),
                project: get("project", dflt.project),
                layers: get("layers", dflt.layers),
                properties: get("properties", dflt.properties),
            }
            .clamped();
        }
        s
    }

    /// Writes the settings; failures are returned so the caller can show them.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or(crate::paths::NO_HOME)?;
        self.save_at(&path)
    }

    /// [`AppSettings::save`] to `path`.
    pub fn save_at(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        // Keep keys other modules store in the same file (Chief catalogs).
        let mut v = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        v["theme"] = serde_json::json!(self.theme.key());
        v["brightness"] = serde_json::json!(self.brightness);
        v["ui_scale_pct"] = serde_json::json!(clamp_scale(self.ui_scale_pct));
        v["large_text"] = serde_json::json!(self.large_text);
        v["reduce_motion"] = serde_json::json!(self.reduce_motion);
        let w = self.dock_widths.clamped();
        v["dock_widths"] = serde_json::json!({
            "library": w.library,
            "project": w.project,
            "layers": w.layers,
            "properties": w.properties,
        });
        let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

// ----- remembered dialog geometry -----

/// Where a dialog was last put and how large the user made it, in points.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DialogGeometry {
    pub pos: [f32; 2],
    pub size: [f32; 2],
}

/// The settings.json key of the per-dialog geometry table.
pub const DIALOG_KEY: &str = "dialog_geometry";

/// Dialog geometry by dialog type (its title), as kept in settings.json.
#[derive(Default, Debug, PartialEq)]
pub struct DialogGeometries {
    map: BTreeMap<String, DialogGeometry>,
}

impl DialogGeometries {
    pub fn get(&self, key: &str) -> Option<DialogGeometry> {
        self.map.get(key).copied()
    }

    /// Stores `g`; true when it differs from what was kept.
    pub fn set(&mut self, key: &str, g: DialogGeometry) -> bool {
        let changed = self.map.get(key) != Some(&g);
        if changed {
            self.map.insert(key.to_string(), g);
        }
        changed
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Reads the table from the settings file at `path` (empty on any problem).
    pub fn read_at(path: &Path) -> Self {
        let mut out = Self::default();
        let Some(v) = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        else {
            return out;
        };
        let Some(obj) = v.get(DIALOG_KEY).and_then(|d| d.as_object()) else {
            return out;
        };
        for (k, e) in obj {
            let n = |name: &str, i: usize| {
                e.get(name)
                    .and_then(|a| a.get(i))
                    .and_then(|x| x.as_f64())
                    .map(|x| x as f32)
                    .filter(|x| x.is_finite())
            };
            if let (Some(x), Some(y), Some(w), Some(h)) =
                (n("pos", 0), n("pos", 1), n("size", 0), n("size", 1))
            {
                if w >= 100.0 && h >= 80.0 {
                    out.map.insert(
                        k.clone(),
                        DialogGeometry {
                            pos: [x, y],
                            size: [w, h],
                        },
                    );
                }
            }
        }
        out
    }

    /// Writes the table into the settings file at `path`, keeping every other key.
    pub fn write_at(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut v = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        let mut obj = serde_json::Map::new();
        for (k, g) in &self.map {
            obj.insert(
                k.clone(),
                serde_json::json!({ "pos": g.pos, "size": g.size }),
            );
        }
        v[DIALOG_KEY] = serde_json::Value::Object(obj);
        let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

struct DialogStore {
    geoms: Option<DialogGeometries>,
    dirty: bool,
}

thread_local! {
    static DIALOGS: std::cell::RefCell<DialogStore> =
        const { std::cell::RefCell::new(DialogStore { geoms: None, dirty: false }) };
}

fn with_dialogs<R>(f: impl FnOnce(&mut DialogGeometries, &mut bool) -> R) -> R {
    DIALOGS.with(|d| {
        let mut d = d.borrow_mut();
        let st = &mut *d;
        let geoms = st.geoms.get_or_insert_with(|| {
            // Tests start empty, whatever the machine saved.
            #[cfg(test)]
            {
                DialogGeometries::default()
            }
            #[cfg(not(test))]
            {
                crate::paths::user_file("settings.json")
                    .map(|p| DialogGeometries::read_at(&p))
                    .unwrap_or_default()
            }
        });
        f(geoms, &mut st.dirty)
    })
}

/// The remembered geometry of the dialog type `key`.
pub fn dialog_geometry(key: &str) -> Option<DialogGeometry> {
    with_dialogs(|g, _| g.get(key))
}

/// Remembers the geometry of the dialog type `key`; it is written to the
/// settings file by [`flush_dialog_geometry`].
pub fn remember_dialog(key: &str, g: DialogGeometry) {
    with_dialogs(|geoms, dirty| {
        if geoms.set(key, g) {
            *dirty = true;
        }
    });
}

/// Writes pending dialog geometry once the mouse is up (not while a window is
/// being dragged or resized).
pub fn flush_dialog_geometry(ctx: &egui::Context) {
    if ctx.input(|i| i.pointer.any_down()) {
        return;
    }
    #[cfg(not(test))]
    with_dialogs(|geoms, dirty| {
        if *dirty {
            *dirty = false;
            if let Some(path) = crate::paths::user_file("settings.json") {
                // A failed write only loses the remembered position.
                let _ = geoms.write_at(&path);
            }
        }
    });
    #[cfg(test)]
    let _ = ctx;
}

/// Multiplies the RGB channels of `c` by `b`.
pub fn scale(c: Color32, b: f32) -> Color32 {
    let m = |v: u8| (f32::from(v) * b).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(m(c.r()), m(c.g()), m(c.b()), c.a())
}

/// UI text: near-white for readability (light-sensitive, low-vision friendly).
pub const UI_TEXT: Color32 = Color32::from_rgb(0xF2, 0xF2, 0xF2);
/// Widget outlines and focus rings: 3.3:1 on the panel gray (WCAG 1.4.11).
pub const UI_OUTLINE: Color32 = Color32::from_rgb(0x8A, 0x8A, 0x8A);
/// Selected / active highlight (text on it is white).
pub const UI_ACCENT: Color32 = Color32::from_rgb(0x4D, 0x8E, 0xDC);
/// egui grays disabled and weak text by averaging it with this color. With
/// `UI_TEXT` the result is #B9 or lighter, so disabled text stays readable.
const UI_FADE_TARGET: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);

/// Applies the chrome (colors, text sizes, motion) and the UI scale for
/// `settings` to egui. Cheap enough to call when anything changed.
pub fn apply_settings(ctx: &egui::Context, settings: &AppSettings) {
    let colors = chrome_colors(settings.theme);
    CURRENT_CHROME.with(|c| c.set(Some(colors)));
    let b = settings.brightness.clamp(BRIGHTNESS_MIN, BRIGHTNESS_MAX);
    let mut v = egui::Visuals::dark();
    v.panel_fill = scale(colors.panel, b);
    v.window_fill = scale(colors.panel, b);
    v.extreme_bg_color = scale(v.extreme_bg_color, b);
    v.faint_bg_color = scale(v.faint_bg_color, b);
    v.override_text_color = Some(scale(colors.text, b));
    v.selection.bg_fill = scale(colors.accent, b);
    v.selection.stroke = Stroke::new(1.0_f32, scale(Color32::WHITE, b));
    v.window_stroke = Stroke::new(1.0_f32, scale(colors.outline, b));
    for (w, c) in [
        (&mut v.widgets.inactive, colors.panel),
        (&mut v.widgets.hovered, colors.hover),
        (&mut v.widgets.active, colors.accent),
    ] {
        w.bg_fill = scale(c, b);
        w.weak_bg_fill = scale(c, b);
    }
    // Disabled and weak text is the text color averaged with this fill.
    v.widgets.noninteractive.weak_bg_fill = scale(UI_FADE_TARGET, b);
    let w = &mut v.widgets;
    for state in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        state.fg_stroke.color = scale(colors.text, b);
        state.bg_stroke.color = scale(colors.outline, b);
    }
    w.active.fg_stroke.color = scale(Color32::WHITE, b);
    ctx.set_visuals(v);
    let body = settings.body_px().max(TEXT_PX_BASE);
    let reduce = settings.reduce_motion;
    ctx.style_mut(|s| {
        use egui::TextStyle::{Body, Button, Heading, Monospace, Small};
        for (style, size) in [
            (Small, (body - 4.0).max(11.0)),
            (Body, body),
            (Button, body),
            (Monospace, body),
            (Heading, body + 3.0),
        ] {
            if let Some(f) = s.text_styles.get_mut(&style) {
                f.size = size;
            }
        }
        // Bigger rows and hit targets: 24 pt rows, 6 pt gaps.
        s.spacing.interact_size.y = 24.0;
        s.spacing.item_spacing.y = 5.0;
        if reduce {
            s.animation_time = 0.0;
            s.scroll_animation = egui::style::ScrollAnimation::none();
        } else {
            let d = egui::Style::default();
            s.animation_time = d.animation_time;
            s.scroll_animation = d.scroll_animation;
        }
    });
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("chrome_key"), chrome_key(settings)));
}

/// What [`apply_settings`] depends on, to notice a change cheaply.
fn chrome_key(s: &AppSettings) -> (CanvasTheme, u32, bool, bool) {
    (
        s.theme,
        (s.brightness * 1000.0).round() as u32,
        s.large_text,
        s.reduce_motion,
    )
}

/// Re-applies the chrome when the theme, brightness, text size or motion
/// setting changed since the last call (and on the first call).
pub fn sync_chrome(ctx: &egui::Context, settings: &AppSettings) {
    let applied: Option<(CanvasTheme, u32, bool, bool)> =
        ctx.data(|d| d.get_temp(egui::Id::new("chrome_key")));
    if applied != Some(chrome_key(settings)) {
        apply_settings(ctx, settings);
    }
}

/// Keeps `settings.ui_scale_pct` and egui's zoom factor in step. A change of
/// the setting sets the zoom; a change of the zoom (Preferences, Cmd+Plus,
/// Cmd+Minus) is adopted into the setting, clamped to 100..=175.
pub fn sync_scale(ctx: &egui::Context, settings: &mut AppSettings) {
    let id = egui::Id::new("scale_applied");
    let applied: Option<u32> = ctx.data(|d| d.get_temp(id));
    let zoom_pct = (ctx.zoom_factor() * 100.0).round() as u32;
    let want = clamp_scale(settings.ui_scale_pct);
    let now = match applied {
        None if want == UI_SCALE_MIN && zoom_pct != UI_SCALE_MIN => {
            // First call: something (the old text-size preference) already
            // chose a zoom; keep it.
            clamp_scale(zoom_pct)
        }
        Some(a) if want == a && zoom_pct != a => clamp_scale(zoom_pct),
        _ => want,
    };
    if now != zoom_pct {
        ctx.set_zoom_factor(now as f32 / 100.0);
    }
    settings.ui_scale_pct = now;
    ctx.data_mut(|d| d.insert_temp(id, now));
}

/// The Appearance controls shared by the status bar menu and Preferences:
/// UI scale, larger text and reduce motion. True when something changed.
pub fn appearance_controls(ui: &mut egui::Ui, s: &mut AppSettings) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("UI scale");
        for pct in UI_SCALE_STEPS {
            if ui
                .selectable_label(s.ui_scale_pct == pct, format!("{pct}%"))
                .clicked()
            {
                s.ui_scale_pct = pct;
                changed = true;
            }
        }
    });
    changed |= ui
        .checkbox(&mut s.large_text, "Larger text (17 pt)")
        .changed();
    changed |= ui
        .checkbox(&mut s.reduce_motion, "Reduce motion")
        .on_hover_text("No animated transitions")
        .changed();
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("plan-studio-theme-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn low_glare_is_default_and_has_no_pure_white() {
        assert_eq!(AppSettings::default().theme, CanvasTheme::LowGlare);
        let p = CanvasTheme::LowGlare.palette();
        for c in [
            p.background,
            p.grid_minor,
            p.grid_major,
            p.wall_fill_exterior,
            p.wall_fill_interior,
            p.wall_stroke,
            p.text,
        ] {
            assert_ne!(c, Color32::WHITE);
        }
    }

    #[test]
    fn theme_keys_round_trip() {
        for t in CanvasTheme::ALL {
            assert_eq!(CanvasTheme::from_key(t.key()), Some(t));
        }
    }

    #[test]
    fn chrome_text_is_near_white_and_disabled_text_stays_readable() {
        let ctx = egui::Context::default();
        apply_settings(&ctx, &AppSettings::default());
        let style = ctx.style();
        let v = &style.visuals;
        assert_eq!(v.text_color(), UI_TEXT);
        let disabled = v.gray_out(v.text_color());
        assert!(disabled.r() >= 0xB8 && disabled.g() >= 0xB8 && disabled.b() >= 0xB8);
        assert_eq!(v.panel_fill, PANEL_GRAY);
        assert_eq!(v.widgets.noninteractive.bg_stroke.color, UI_OUTLINE);
        assert_eq!(v.selection.bg_fill, UI_ACCENT);
        assert_eq!(style.text_styles[&egui::TextStyle::Body].size, 15.0);
        assert_eq!(style.text_styles[&egui::TextStyle::Heading].size, 18.0);
        // Dimming is proportional.
        let dim = AppSettings {
            brightness: 0.6,
            ..AppSettings::default()
        };
        apply_settings(&ctx, &dim);
        assert!(ctx.style().visuals.text_color().r() < 0xF2);
    }

    #[test]
    fn high_contrast_chrome_has_pure_white_text() {
        let ctx = egui::Context::default();
        let s = AppSettings {
            theme: CanvasTheme::HighContrast,
            ..AppSettings::default()
        };
        apply_settings(&ctx, &s);
        let v = ctx.style().visuals.clone();
        assert_eq!(v.text_color(), Color32::WHITE);
        assert_eq!(v.panel_fill, Color32::BLACK);
        assert_eq!(current_chrome(), chrome_colors(CanvasTheme::HighContrast));
    }

    /// Every chrome pair of every theme reaches its WCAG ratio; the High
    /// Contrast theme also clears 3:1 on every pair and 7:1 on its text.
    #[test]
    fn chrome_colors_meet_their_contrast_ratios() {
        for theme in CanvasTheme::ALL {
            let c = chrome_colors(theme);
            for (name, fg, bg, min) in c.pairs() {
                let r = contrast_ratio(fg, bg);
                assert!(r >= min, "{theme:?}: {name} is {r:.2}:1, needs {min}:1");
                assert!(r >= 3.0, "{theme:?}: {name} is {r:.2}:1, below 3:1");
            }
        }
        let hc = chrome_colors(CanvasTheme::HighContrast);
        assert_eq!(hc.text, Color32::WHITE);
        assert!(contrast_ratio(hc.text, hc.panel) >= 7.0);
        assert!(contrast_ratio(hc.text, hc.hover) >= 7.0);
        assert!(contrast_ratio(hc.text, hc.accent) >= 4.5);
        assert!(contrast_ratio(hc.outline, hc.panel) >= 7.0);
    }

    #[test]
    fn contrast_math_matches_wcag() {
        assert!((contrast_ratio(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn canvas_ink_reads_on_every_canvas_background() {
        for theme in CanvasTheme::ALL {
            let p = theme.palette();
            for (name, fg, min) in [
                ("wall stroke", p.wall_stroke, 4.5),
                ("opening line", p.opening_line, 4.5),
                ("text", p.text, 4.5),
                ("room label", p.room_label, 4.5),
                ("dimension text", p.dimension_text, 4.5),
                ("selection", p.selection, 3.0),
                ("hover", p.hover, 3.0),
            ] {
                // Walls draw their stroke over the fill, not the background.
                if name == "wall stroke" && theme == CanvasTheme::HighContrast {
                    continue;
                }
                let r = contrast_ratio(fg, p.background);
                assert!(r >= min, "{theme:?}: {name} is {r:.2}:1 on the canvas");
            }
        }
    }

    /// The low-glare canvas: warm gray #C4C0B8 (L 0.52), grid lines only just
    /// darker than the paper so they never compete with the walls.
    #[test]
    fn low_glare_canvas_values_are_documented_and_gentle() {
        let p = CanvasTheme::LowGlare.palette();
        assert_eq!(p.background, Color32::from_rgb(0xC4, 0xC0, 0xB8));
        assert_eq!(p.grid_minor, Color32::from_rgb(0xB0, 0xAC, 0xA4));
        assert_eq!(p.grid_major, Color32::from_rgb(0xA0, 0x9C, 0x94));
        assert!(luminance(p.background) < 0.6, "background is too bright");
        let minor = contrast_ratio(p.grid_minor, p.background);
        let major = contrast_ratio(p.grid_major, p.background);
        assert!((1.1..1.4).contains(&minor), "minor grid {minor:.2}:1");
        assert!((1.4..2.0).contains(&major), "major grid {major:.2}:1");
        // And much dimmer than the paper theme.
        assert!(luminance(p.background) < 0.6 * luminance(CanvasTheme::Paper.palette().background));
    }

    #[test]
    fn body_text_is_never_below_15_and_large_text_is_17() {
        let ctx = egui::Context::default();
        let mut s = AppSettings::default();
        apply_settings(&ctx, &s);
        assert_eq!(ctx.style().text_styles[&egui::TextStyle::Body].size, 15.0);
        s.large_text = true;
        apply_settings(&ctx, &s);
        let style = ctx.style();
        assert_eq!(style.text_styles[&egui::TextStyle::Body].size, 17.0);
        assert_eq!(style.text_styles[&egui::TextStyle::Button].size, 17.0);
        assert_eq!(style.text_styles[&egui::TextStyle::Heading].size, 20.0);
        assert!(style.text_styles[&egui::TextStyle::Small].size >= 11.0);
    }

    #[test]
    fn reduce_motion_turns_animations_off() {
        let ctx = egui::Context::default();
        let mut s = AppSettings::default();
        apply_settings(&ctx, &s);
        assert!(ctx.style().animation_time > 0.0);
        s.reduce_motion = true;
        apply_settings(&ctx, &s);
        assert_eq!(ctx.style().animation_time, 0.0);
        s.reduce_motion = false;
        apply_settings(&ctx, &s);
        assert!(ctx.style().animation_time > 0.0);
    }

    #[test]
    fn settings_round_trip_scale_docks_and_keep_foreign_keys() {
        let path = temp_file("settings-round-trip.json");
        std::fs::write(&path, r#"{"chief_catalogs":{"on":true}}"#).unwrap();
        let s = AppSettings {
            theme: CanvasTheme::HighContrast,
            brightness: 0.8,
            ui_scale_pct: 150,
            large_text: true,
            reduce_motion: true,
            dock_widths: DockWidths {
                library: 340.0,
                project: 310.0,
                layers: 280.0,
                properties: 260.0,
            },
        };
        s.save_at(&path).unwrap();
        assert_eq!(AppSettings::load_at(&path), s);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("chief_catalogs"));
        // Out-of-range values are clamped on load.
        std::fs::write(
            &path,
            r#"{"ui_scale_pct": 400, "dock_widths": {"library": 5, "layers": 99999}}"#,
        )
        .unwrap();
        let q = AppSettings::load_at(&path);
        assert_eq!(q.ui_scale_pct, UI_SCALE_MAX);
        assert_eq!(q.dock_widths.library, DOCK_WIDTH_MIN);
        assert_eq!(q.dock_widths.layers, DOCK_WIDTH_MAX);
        assert_eq!(q.dock_widths.project, DockWidths::default().project);
        // A missing or broken file gives the defaults.
        assert_eq!(
            AppSettings::load_at(&temp_file("does-not-exist.json")),
            AppSettings::default()
        );
    }

    #[test]
    fn dialog_geometry_round_trips_per_dialog_type() {
        let path = temp_file("settings-dialogs.json");
        std::fs::write(&path, r#"{"theme":"dark"}"#).unwrap();
        let mut g = DialogGeometries::default();
        let wall = DialogGeometry {
            pos: [120.0, 80.0],
            size: [900.0, 600.0],
        };
        let door = DialogGeometry {
            pos: [10.0, 20.0],
            size: [700.0, 500.0],
        };
        assert!(g.set("Wall Specification", wall));
        assert!(!g.set("Wall Specification", wall));
        assert!(g.set("Door Specification", door));
        g.write_at(&path).unwrap();
        let back = DialogGeometries::read_at(&path);
        assert_eq!(back, g);
        assert_eq!(back.get("Door Specification"), Some(door));
        assert_eq!(back.len(), 2);
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("\"theme\""));
        // AppSettings keeps the table too.
        AppSettings::default().save_at(&path).unwrap();
        assert_eq!(DialogGeometries::read_at(&path), g);
        // The live store.
        remember_dialog("Roof Specification", wall);
        assert_eq!(dialog_geometry("Roof Specification"), Some(wall));
        assert_eq!(dialog_geometry("Nothing"), None);
    }

    fn run_frame(ctx: &egui::Context) {
        let _ = ctx.run(egui::RawInput::default(), |_| {});
    }

    #[test]
    fn scale_setting_drives_the_zoom_and_adopts_outside_changes() {
        let ctx = egui::Context::default();
        let mut s = AppSettings::default();
        sync_scale(&ctx, &mut s);
        assert_eq!(ctx.zoom_factor(), 1.0);
        // The setting changes: the zoom follows (egui applies it at the next
        // frame, which is where the app calls `sync_scale` again).
        s.ui_scale_pct = 150;
        sync_scale(&ctx, &mut s);
        run_frame(&ctx);
        assert_eq!(ctx.zoom_factor(), 1.5);
        assert_eq!(ctx.pixels_per_point(), 1.5);
        sync_scale(&ctx, &mut s);
        assert_eq!(s.ui_scale_pct, 150);
        // Something else (Cmd+Plus) changes the zoom: the setting adopts it.
        ctx.set_zoom_factor(1.25);
        run_frame(&ctx);
        sync_scale(&ctx, &mut s);
        assert_eq!(s.ui_scale_pct, 125);
        // Out of range zooms are pulled back into 100..=175.
        ctx.set_zoom_factor(0.8);
        run_frame(&ctx);
        sync_scale(&ctx, &mut s);
        assert_eq!(s.ui_scale_pct, 100);
        run_frame(&ctx);
        assert_eq!(ctx.zoom_factor(), 1.0);
        ctx.set_zoom_factor(3.0);
        run_frame(&ctx);
        sync_scale(&ctx, &mut s);
        assert_eq!(s.ui_scale_pct, 175);
        run_frame(&ctx);
        assert_eq!(ctx.zoom_factor(), 1.75);
    }

    #[test]
    fn a_zoom_chosen_before_the_first_sync_is_kept() {
        let ctx = egui::Context::default();
        ctx.set_zoom_factor(1.2);
        run_frame(&ctx);
        let mut s = AppSettings::default();
        sync_scale(&ctx, &mut s);
        assert_eq!(s.ui_scale_pct, 120);
        run_frame(&ctx);
        assert_eq!(ctx.zoom_factor(), 1.2);
    }

    #[test]
    fn sync_chrome_reapplies_only_on_change() {
        let ctx = egui::Context::default();
        let mut s = AppSettings::default();
        sync_chrome(&ctx, &s);
        assert_eq!(ctx.style().text_styles[&egui::TextStyle::Body].size, 15.0);
        s.large_text = true;
        sync_chrome(&ctx, &s);
        assert_eq!(ctx.style().text_styles[&egui::TextStyle::Body].size, 17.0);
    }
}
