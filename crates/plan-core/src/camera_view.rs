//! Per-camera view settings and the plan's 3D lighting
//! (`docs/parity/3d-views-cameras.md`, C-6, C-31, C-33, C-44, C-62, C-63, C-70).
//!
//! The Camera Specification of Chief has a Camera tab (position, direction,
//! angle of view, height, tilt, clipping), a Backdrop tab, a Rendering tab
//! (technique, quality, lighting) and a Label tab. [`CameraView`] holds what
//! the tabs add to [`crate::CameraObject`]; every field is optional on load
//! (`#[serde(default)]`) so files saved before the tabs existed still open.
//! [`Lighting`] is the plan-wide sun and interior-light setup of the 3D >
//! Lighting dialog, kept in [`crate::Project::lighting`].
//!
//! The rendering technique is stored by its menu name (plan-core has no
//! dependency on `plan-materials`); the app maps the name back.

use serde::{Deserialize, Serialize};

/// Preview draws fast (no shadows, low quality); Final View is the full look:
/// shadows, occlusion and anti-aliasing (Chief's "Final View" button).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ViewQuality {
    Preview,
    #[default]
    Final,
}

impl ViewQuality {
    pub const ALL: [ViewQuality; 2] = [ViewQuality::Preview, ViewQuality::Final];

    pub fn label(self) -> &'static str {
        match self {
            ViewQuality::Preview => "Preview",
            ViewQuality::Final => "Final View",
        }
    }
}

/// What fills the 3D view behind the model (the Backdrop tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BackdropKind {
    /// The technique's own sky and ground.
    #[default]
    Default,
    /// A flat sky colour.
    Color,
    /// An image file from Chief's Backdrops folder (or any picture file).
    Image,
}

impl BackdropKind {
    pub const ALL: [BackdropKind; 3] = [
        BackdropKind::Default,
        BackdropKind::Color,
        BackdropKind::Image,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BackdropKind::Default => "Default sky",
            BackdropKind::Color => "Sky color",
            BackdropKind::Image => "Image",
        }
    }
}

fn default_sky() -> [u8; 3] {
    [217, 227, 240]
}

/// The Backdrop tab of a camera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Backdrop {
    pub kind: BackdropKind,
    /// Sky colour of [`BackdropKind::Color`].
    pub color: [u8; 3],
    /// File name (inside the Backdrops folder) or full path of the picture of
    /// [`BackdropKind::Image`]. Never the picture itself: it is read from
    /// Chief's install when the view is drawn.
    pub image: String,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            kind: BackdropKind::Default,
            color: default_sky(),
            image: String::new(),
        }
    }
}

/// Which floors a camera shows (C-6, C-44).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FloorsDisplayed {
    /// Every floor of the plan.
    #[default]
    All,
    /// The camera's floor and the ones below it.
    ThisAndBelow,
}

impl FloorsDisplayed {
    pub const ALL: [FloorsDisplayed; 2] = [FloorsDisplayed::All, FloorsDisplayed::ThisAndBelow];

    pub fn label(self) -> &'static str {
        match self {
            FloorsDisplayed::All => "All floors",
            FloorsDisplayed::ThisAndBelow => "This floor and below",
        }
    }
}

/// The Label tab: the text drawn beside the camera symbol in the plan and
/// over the 3D view.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraLabel {
    /// The text; empty uses the camera's name.
    pub text: String,
    pub show_in_plan: bool,
    pub show_in_view: bool,
}

/// A saved orbit pose: the eye and the point it looks at, scene space
/// (X, up, -plan Y), inches. Overview cameras and Save Camera keep it so
/// Restore brings back the exact view.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewPose {
    pub eye: [f64; 3],
    pub target: [f64; 3],
}

fn yes() -> bool {
    true
}

fn default_ambient() -> f64 {
    0.45
}

/// What the Camera Specification stores beyond the camera's placement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraView {
    /// Up/down tilt of the view in degrees (positive looks up); 0 is level.
    pub tilt_deg: f64,
    /// A locked camera cannot be moved, aimed or resized from the plan.
    pub locked: bool,
    /// Draw the camera symbol in the plan.
    #[serde(default = "yes")]
    pub show_in_plan: bool,
    /// The rendering technique's menu name; `None` uses the 3D view's own.
    pub technique: Option<String>,
    pub quality: ViewQuality,
    pub backdrop: Backdrop,
    pub label: CameraLabel,
    pub floors: FloorsDisplayed,
    /// Sun shadows in the Standard technique.
    #[serde(default = "yes")]
    pub shadows: bool,
    /// Ambient light of this view, 0..1; `None` uses the plan's
    /// [`Lighting`].
    pub ambient: Option<f64>,
    /// Sun strength of this view, 0..2; `None` uses the plan's [`Lighting`].
    pub sun_intensity: Option<f64>,
    pub pose: Option<ViewPose>,
    /// How a walkthrough camera is recorded.
    pub walk: WalkRecord,
}

impl Default for CameraView {
    fn default() -> Self {
        Self {
            tilt_deg: 0.0,
            locked: false,
            show_in_plan: true,
            technique: None,
            quality: ViewQuality::Final,
            backdrop: Backdrop::default(),
            label: CameraLabel::default(),
            floors: FloorsDisplayed::All,
            shadows: true,
            ambient: None,
            sun_intensity: None,
            pose: None,
            walk: WalkRecord::default(),
        }
    }
}

impl CameraView {
    /// The ambient light this view uses given the plan's lighting.
    pub fn ambient_in(&self, plan: &Lighting) -> f64 {
        self.ambient.unwrap_or(plan.ambient).clamp(0.0, 1.0)
    }

    /// The sun strength this view uses given the plan's lighting.
    pub fn sun_in(&self, plan: &Lighting) -> f64 {
        self.sun_intensity
            .unwrap_or(plan.sun_intensity)
            .clamp(0.0, 2.0)
    }

    /// The text shown for a camera called `name`.
    pub fn label_text<'a>(&'a self, name: &'a str) -> &'a str {
        if self.label.text.trim().is_empty() {
            name
        } else {
            &self.label.text
        }
    }
}

/// Smallest and largest frame rate of a recorded walkthrough.
pub const MIN_FPS: f64 = 1.0;
pub const MAX_FPS: f64 = 60.0;

fn default_fps() -> f64 {
    WalkRecord::DEFAULT_FPS
}

fn default_width() -> u32 {
    640
}

fn default_height() -> u32 {
    480
}

fn default_samples() -> u32 {
    8
}

/// How Record Walkthrough renders the frames of a walkthrough camera.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WalkRecord {
    /// Frames per second (the PNG sequence is numbered at this rate).
    pub fps: f64,
    /// Picture size, pixels.
    pub width: u32,
    pub height: u32,
    /// Path-tracer samples per pixel.
    pub samples: u32,
}

impl Default for WalkRecord {
    fn default() -> Self {
        Self {
            fps: default_fps(),
            width: default_width(),
            height: default_height(),
            samples: default_samples(),
        }
    }
}

impl WalkRecord {
    /// The default frame rate.
    pub const DEFAULT_FPS: f64 = 12.0;

    /// The same settings with every value brought into range.
    pub fn clamped(self) -> Self {
        Self {
            fps: if self.fps.is_finite() {
                self.fps.clamp(MIN_FPS, MAX_FPS)
            } else {
                default_fps()
            },
            width: self.width.clamp(64, 4096),
            height: self.height.clamp(48, 4096),
            samples: self.samples.clamp(1, 256),
        }
    }
}

/// The largest tilt either way, degrees.
pub const MAX_TILT_DEG: f64 = 85.0;

/// Interior lights and the sun for the whole plan (3D > Lighting).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lighting {
    /// Where the sun is from north, degrees clockwise.
    pub sun_azimuth_deg: f64,
    /// Height of the sun above the horizon, degrees.
    pub sun_altitude_deg: f64,
    /// Sun strength, 0..2 (1 is the plan's lighting).
    pub sun_intensity: f64,
    /// Ambient light, 0..1.
    pub ambient: f64,
    /// The plan's lights and the fixtures of the electrical plan shine. Off
    /// leaves them out of every view and ray trace.
    #[serde(default = "yes")]
    pub interior_lights: bool,
    /// The sun is set by the date: month, day, solar hours and latitude.
    pub from_date: Option<SunDate>,
}

/// Month, day, solar time and latitude the sun position came from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SunDate {
    pub month: u32,
    pub day: u32,
    pub hours: f64,
    pub latitude: f64,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            sun_azimuth_deg: 225.0,
            sun_altitude_deg: 45.0,
            sun_intensity: 1.0,
            ambient: default_ambient(),
            interior_lights: true,
            from_date: None,
        }
    }
}

impl Lighting {
    /// The unit direction toward the sun in scene space (X east, Y up, Z
    /// south: plan +Y is north). Azimuth is measured clockwise from north.
    pub fn to_sun(&self) -> [f64; 3] {
        let az = self.sun_azimuth_deg.to_radians();
        let alt = self.sun_altitude_deg.to_radians();
        let horizontal = alt.cos();
        // East = +X, north = +plan Y = -scene Z.
        [az.sin() * horizontal, alt.sin(), -az.cos() * horizontal]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_files_without_the_view_load_with_defaults() {
        let v: CameraView = serde_json::from_str("{}").unwrap();
        assert_eq!(v, CameraView::default());
        assert!(v.show_in_plan && v.shadows);
        let l: Lighting = serde_json::from_str("{}").unwrap();
        assert_eq!(l, Lighting::default());
        assert!(l.interior_lights);
    }

    #[test]
    fn a_view_round_trips_through_json() {
        let mut v = CameraView::default();
        v.tilt_deg = -12.5;
        v.locked = true;
        v.technique = Some("Glass House".into());
        v.quality = ViewQuality::Preview;
        v.backdrop.kind = BackdropKind::Image;
        v.backdrop.image = "Rolling Hills.jpg".into();
        v.label.text = "Entry".into();
        v.floors = FloorsDisplayed::ThisAndBelow;
        v.pose = Some(ViewPose {
            eye: [1.0, 2.0, 3.0],
            target: [4.0, 5.0, 6.0],
        });
        let back: CameraView = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn the_recording_settings_are_brought_into_range() {
        let w = WalkRecord {
            fps: 500.0,
            width: 10,
            height: 99_999,
            samples: 0,
        }
        .clamped();
        assert_eq!((w.fps, w.width, w.height, w.samples), (60.0, 64, 4096, 1));
        let nan = WalkRecord {
            fps: f64::NAN,
            ..WalkRecord::default()
        };
        assert_eq!(nan.clamped().fps, 12.0);
    }

    #[test]
    fn the_label_falls_back_to_the_name() {
        let mut v = CameraView::default();
        let plan = Lighting {
            ambient: 0.3,
            sun_intensity: 1.5,
            ..Lighting::default()
        };
        assert_eq!((v.ambient_in(&plan), v.sun_in(&plan)), (0.3, 1.5));
        v.ambient = Some(0.8);
        v.sun_intensity = Some(5.0);
        assert_eq!((v.ambient_in(&plan), v.sun_in(&plan)), (0.8, 2.0));
        let mut v = CameraView::default();
        assert_eq!(v.label_text("Camera 1"), "Camera 1");
        v.label.text = "  ".into();
        assert_eq!(v.label_text("Camera 1"), "Camera 1");
        v.label.text = "Entry".into();
        assert_eq!(v.label_text("Camera 1"), "Entry");
    }

    #[test]
    fn the_sun_direction_follows_azimuth_and_altitude() {
        let mut l = Lighting::default();
        // Due south at the horizon is +scene Z; due east is +X.
        l.sun_azimuth_deg = 180.0;
        l.sun_altitude_deg = 0.0;
        let s = l.to_sun();
        assert!((s[2] - 1.0).abs() < 1e-9 && s[0].abs() < 1e-9);
        l.sun_azimuth_deg = 90.0;
        let e = l.to_sun();
        assert!((e[0] - 1.0).abs() < 1e-9);
        l.sun_altitude_deg = 90.0;
        assert!((l.to_sun()[1] - 1.0).abs() < 1e-9);
    }
}
