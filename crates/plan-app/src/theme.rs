//! Canvas themes, the global UI brightness, and persisted app settings.
//!
//! The canvas default is a low-glare warm gray so the main drawing area is
//! comfortable for people who are sensitive to light and brightness.

use eframe::egui::{self, Color32, Stroke};
use std::path::PathBuf;

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
                selection: rgb(255, 140, 0),
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
                selection: rgb(200, 100, 20),
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

/// Settings persisted between runs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AppSettings {
    pub theme: CanvasTheme,
    /// Global UI brightness, 0.6..=1.0.
    pub brightness: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: CanvasTheme::LowGlare,
            brightness: 1.0,
        }
    }
}

impl AppSettings {
    /// `~/.plan-studio/settings.json`, or `None` when no home directory is
    /// known (`HOME`, else `USERPROFILE` on Windows).
    fn path() -> Option<PathBuf> {
        crate::paths::user_file("settings.json")
    }

    /// Loads the saved settings; any problem falls back to the defaults.
    pub fn load() -> Self {
        let mut s = Self::default();
        let Some(text) = Self::path().and_then(|p| std::fs::read_to_string(p).ok()) else {
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
        s
    }

    /// Writes the settings; failures are returned so the caller can show them.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or(crate::paths::NO_HOME)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let v = serde_json::json!({
            "theme": self.theme.key(),
            "brightness": self.brightness,
        });
        let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

/// Multiplies the RGB channels of `c` by `b`.
pub fn scale(c: Color32, b: f32) -> Color32 {
    let m = |v: u8| (f32::from(v) * b).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(m(c.r()), m(c.g()), m(c.b()), c.a())
}

/// UI text: near-white for readability (light-sensitive, low-vision friendly).
pub const UI_TEXT: Color32 = Color32::from_rgb(0xF2, 0xF2, 0xF2);
/// Widget outlines.
pub const UI_OUTLINE: Color32 = Color32::from_rgb(0x6A, 0x6A, 0x6A);
/// Selected / active highlight (text on it is white).
pub const UI_ACCENT: Color32 = Color32::from_rgb(0x4D, 0x8E, 0xDC);
/// egui grays disabled and weak text by averaging it with this color. With
/// `UI_TEXT` the result is #B9 or lighter, so disabled text stays readable.
const UI_FADE_TARGET: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);

/// Dark, Chief-like chrome with panel fills and text scaled by `brightness`
/// (1.0 gives exactly the colors above; lower values dim proportionally).
/// Text is 15 px body / 18 px headings.
pub fn apply_chrome(ctx: &egui::Context, brightness: f32) {
    let b = brightness.clamp(BRIGHTNESS_MIN, BRIGHTNESS_MAX);
    let mut v = egui::Visuals::dark();
    v.panel_fill = scale(PANEL_GRAY, b);
    v.window_fill = scale(PANEL_GRAY, b);
    v.extreme_bg_color = scale(v.extreme_bg_color, b);
    v.faint_bg_color = scale(v.faint_bg_color, b);
    v.override_text_color = Some(scale(UI_TEXT, b));
    v.selection.bg_fill = scale(UI_ACCENT, b);
    v.selection.stroke = Stroke::new(1.0_f32, scale(Color32::WHITE, b));
    v.window_stroke = Stroke::new(1.0_f32, scale(UI_OUTLINE, b));
    for (w, c) in [
        (&mut v.widgets.inactive, PANEL_GRAY),
        (&mut v.widgets.hovered, HOVER_GRAY),
        (&mut v.widgets.active, UI_ACCENT),
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
        state.fg_stroke.color = scale(UI_TEXT, b);
        state.bg_stroke.color = scale(UI_OUTLINE, b);
    }
    w.active.fg_stroke.color = scale(Color32::WHITE, b);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        use egui::TextStyle::{Body, Button, Heading, Monospace, Small};
        for (style, size) in [
            (Small, 11.0),
            (Body, 15.0),
            (Button, 15.0),
            (Monospace, 15.0),
            (Heading, 18.0),
        ] {
            if let Some(f) = s.text_styles.get_mut(&style) {
                f.size = size;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
        apply_chrome(&ctx, 1.0);
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
        apply_chrome(&ctx, 0.6);
        assert!(ctx.style().visuals.text_color().r() < 0xF2);
    }
}
