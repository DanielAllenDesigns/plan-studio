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

pub mod annot;
pub mod clip;
pub mod facing;
pub mod spec;

pub use annot::{AnnotKind, DrawSurface, ViewAnnotation};
pub use clip::{ClipVolume, SectionClip, StepPlane};
pub use spec::{
    BelowGrade, BelowGradeLimit, CalloutArrow, CalloutPlacement, CameraOptions, CrossSectionSlider,
    DepthCue, PlanDisplay, SliderPlane, SliderSide, SuperResolution, ViewDefaults, ViewLayer,
};
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

/// What lies below the horizon behind the model (the Backdrop tab's Ground).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GroundKind {
    /// The technique's own ground fade.
    #[default]
    Default,
    /// A flat ground colour below the horizon.
    Color,
    /// No ground: the sky carries on below the horizon.
    None,
}

impl GroundKind {
    pub const ALL: [GroundKind; 3] = [GroundKind::Default, GroundKind::Color, GroundKind::None];

    pub fn label(self) -> &'static str {
        match self {
            GroundKind::Default => "Default ground",
            GroundKind::Color => "Ground color",
            GroundKind::None => "No ground",
        }
    }
}

fn default_sky() -> [u8; 3] {
    [217, 227, 240]
}

fn default_ground() -> [u8; 3] {
    [140, 150, 130]
}

fn default_fog_distance() -> f64 {
    300.0
}

/// Distance haze: surfaces fade toward the fog colour the farther they are
/// from the eye (the Backdrop tab's Fog).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fog {
    pub on: bool,
    /// Distance at which about 63 percent of the colour is fog, feet.
    pub distance_ft: f64,
    /// Fog colour; `None` uses the horizon colour of the sky so the haze
    /// melts into the backdrop.
    pub color: Option<[u8; 3]>,
}

impl Default for Fog {
    fn default() -> Self {
        Self {
            on: false,
            distance_ft: default_fog_distance(),
            color: None,
        }
    }
}

/// Smallest and largest fog distance, feet.
pub const FOG_FEET: (f64, f64) = (10.0, 5000.0);

impl Fog {
    /// The fog density the renderer uses, per inch (0 when the fog is off).
    pub fn density_per_inch(&self) -> f32 {
        if !self.on || !self.distance_ft.is_finite() {
            return 0.0;
        }
        (1.0 / (self.distance_ft.clamp(FOG_FEET.0, FOG_FEET.1) * 12.0)) as f32
    }
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
    /// What lies below the horizon.
    pub ground: GroundKind,
    /// Ground colour of [`GroundKind::Color`].
    #[serde(default = "default_ground")]
    pub ground_color: [u8; 3],
    /// Distance haze.
    pub fog: Fog,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            kind: BackdropKind::Default,
            color: default_sky(),
            image: String::new(),
            ground: GroundKind::Default,
            ground_color: default_ground(),
            fog: Fog::default(),
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
    /// The floors `from` to `to`, both included (0 is the lowest floor): the
    /// per-floor pick of the Camera Specification.
    Picked { from: u8, to: u8 },
}

impl FloorsDisplayed {
    pub const ALL: [FloorsDisplayed; 2] = [FloorsDisplayed::All, FloorsDisplayed::ThisAndBelow];

    pub fn label(self) -> &'static str {
        match self {
            FloorsDisplayed::All => "All floors",
            FloorsDisplayed::ThisAndBelow => "This floor and below",
            FloorsDisplayed::Picked { .. } => "Pick floors",
        }
    }

    /// Is this a per-floor pick?
    pub fn is_picked(self) -> bool {
        matches!(self, FloorsDisplayed::Picked { .. })
    }

    /// The first and last floor shown for a camera on `camera_floor` in a
    /// plan of `floors` floors; `None` shows everything. A pick is brought
    /// into range and put in order.
    pub fn range(self, camera_floor: usize, floors: usize) -> Option<(usize, usize)> {
        let last = floors.saturating_sub(1);
        match self {
            FloorsDisplayed::All => None,
            FloorsDisplayed::ThisAndBelow => Some((0, camera_floor.min(last))),
            FloorsDisplayed::Picked { from, to } => {
                let (a, b) = (usize::from(from).min(last), usize::from(to).min(last));
                Some((a.min(b), a.max(b)))
            }
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

/// A named choice of which lights shine (Chief's light sets in Adjust
/// Lights). The plan's lights and the electrical fixtures are listed apart
/// (their ids come from different counters); anything not listed is off while
/// the set is in use.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LightSet {
    pub name: String,
    /// Ids of the plan's lights ([`crate::Project::lights`]) that shine.
    pub on: Vec<crate::Id>,
    /// Ids of the electrical light fixtures (devices) that shine.
    pub fixtures: Vec<crate::Id>,
}

impl LightSet {
    /// Does the plan light `id` shine in this set?
    pub fn shines(&self, id: crate::Id) -> bool {
        self.on.contains(&id)
    }

    /// Does the electrical light fixture `id` shine in this set?
    pub fn shines_fixture(&self, id: crate::Id) -> bool {
        self.fixtures.contains(&id)
    }
}

/// A saved orbit pose: the eye and the point it looks at, scene space
/// (X, up, -plan Y), inches. Overview cameras and Save Camera keep it so
/// Restore brings back the exact view.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewPose {
    pub eye: [f64; 3],
    pub target: [f64; 3],
    /// The camera's plan symbol places the eye and the target in plan (its
    /// position, direction and clip distance), so moving, aiming or copying
    /// the symbol moves the view; the pose keeps the two heights. Overviews
    /// saved before the symbol was editable read as `false` and keep the
    /// pose as it is.
    #[serde(default)]
    pub symbol: bool,
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
    /// The light set this view uses in place of the plan's active one.
    pub light_set: Option<String>,
    /// A saved orthographic view (an elevation or the plan overhead).
    pub ortho: Option<OrthoView>,
    /// Scene Clipping of a section or elevation (C-136..C-138, C-152, C-157).
    pub clip: SectionClip,
    /// Text, dimensions and CAD drawn on the view, saved with it (C-129).
    pub annotations: Vec<ViewAnnotation>,
    /// Incremental Move Distance of this camera, inches: one pan, dolly or
    /// keyboard move step (C-121, DECISIONS 41).
    #[serde(default = "default_move_step")]
    pub move_step: f64,
    /// Incremental Rotate Angle of this camera, degrees: one orbit, turn,
    /// tilt (a third of it) or side-dolly step.
    #[serde(default = "default_rotate_step")]
    pub rotate_step: f64,
    /// Depth Cue of a section or elevation (C-140).
    pub depth_cue: DepthCue,
    /// The Cross Section Slider planes, saved with the camera (C-141).
    pub slider: CrossSectionSlider,
    /// Below Grade line overrides (C-153).
    pub below_grade: BelowGrade,
    /// Selected Defaults of the view's annotations (C-139).
    pub selected: ViewDefaults,
    /// Plan Display of the camera symbol (C-158).
    pub plan: PlanDisplay,
    /// The Layer panel (C-156).
    pub layer: ViewLayer,
    /// The rendering options of the Camera panel (C-146 to C-151).
    pub options: CameraOptions,
}

/// Incremental Move Distance of a new camera, inches (DECISIONS 41).
pub const DEFAULT_MOVE_STEP: f64 = 24.0;
/// Incremental Rotate Angle of a new camera, degrees (DECISIONS 41).
pub const DEFAULT_ROTATE_STEP: f64 = 15.0;

fn default_move_step() -> f64 {
    DEFAULT_MOVE_STEP
}

fn default_rotate_step() -> f64 {
    DEFAULT_ROTATE_STEP
}

/// Which orthographic view a saved camera restores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrthoKind {
    ElevationFront,
    ElevationBack,
    ElevationLeft,
    ElevationRight,
    PlanOverhead,
}

impl OrthoKind {
    pub const ALL: [OrthoKind; 5] = [
        OrthoKind::ElevationFront,
        OrthoKind::ElevationBack,
        OrthoKind::ElevationLeft,
        OrthoKind::ElevationRight,
        OrthoKind::PlanOverhead,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OrthoKind::ElevationFront => "Front Elevation",
            OrthoKind::ElevationBack => "Back Elevation",
            OrthoKind::ElevationLeft => "Left Elevation",
            OrthoKind::ElevationRight => "Right Elevation",
            OrthoKind::PlanOverhead => "Plan Overhead",
        }
    }
}

/// A saved orthographic view: the direction, the point it is centred on
/// (scene space) and how much it shows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrthoView {
    pub kind: OrthoKind,
    pub target: [f64; 3],
    /// Half the visible height, inches.
    pub half_height: f64,
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
            light_set: None,
            ortho: None,
            clip: SectionClip::legacy(),
            annotations: Vec::new(),
            move_step: DEFAULT_MOVE_STEP,
            rotate_step: DEFAULT_ROTATE_STEP,
            depth_cue: DepthCue::default(),
            slider: CrossSectionSlider::default(),
            below_grade: BelowGrade::default(),
            selected: ViewDefaults::default(),
            plan: PlanDisplay::default(),
            layer: ViewLayer::default(),
            options: CameraOptions::default(),
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

/// What Record Walkthrough writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RecordFormat {
    /// One Motion-JPEG `.avi` movie.
    #[default]
    Video,
    /// A numbered PNG sequence and the `make_video.sh` script.
    Frames,
    /// Both of them.
    Both,
}

impl RecordFormat {
    pub const ALL: [RecordFormat; 3] = [
        RecordFormat::Video,
        RecordFormat::Frames,
        RecordFormat::Both,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RecordFormat::Video => "Video (AVI, Motion-JPEG)",
            RecordFormat::Frames => "PNG sequence",
            RecordFormat::Both => "Video and PNG sequence",
        }
    }

    /// Does this format write the movie?
    pub fn video(self) -> bool {
        matches!(self, RecordFormat::Video | RecordFormat::Both)
    }

    /// Does this format write the PNG frames?
    pub fn frames(self) -> bool {
        matches!(self, RecordFormat::Frames | RecordFormat::Both)
    }
}

fn default_jpeg_quality() -> u8 {
    85
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
    /// A movie, a PNG sequence or both.
    pub format: RecordFormat,
    /// JPEG quality of the movie's frames, 1..=100.
    #[serde(default = "default_jpeg_quality")]
    pub quality: u8,
}

impl Default for WalkRecord {
    fn default() -> Self {
        Self {
            fps: default_fps(),
            width: default_width(),
            height: default_height(),
            samples: default_samples(),
            format: RecordFormat::Video,
            quality: default_jpeg_quality(),
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
            format: self.format,
            quality: self.quality.clamp(1, 100),
        }
    }
}

/// The largest tilt either way, degrees.
pub const MAX_TILT_DEG: f64 = 85.0;

/// Interior lights and the sun for the whole plan (3D > Lighting).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// The named light sets of Adjust Lights.
    pub sets: Vec<LightSet>,
    /// The set in use for the whole plan; `None` leaves every light to its
    /// own on/off switch.
    pub active_set: Option<String>,
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
            sets: Vec::new(),
            active_set: None,
        }
    }
}

impl Lighting {
    /// The light set called `name` (case-insensitive).
    pub fn set(&self, name: &str) -> Option<&LightSet> {
        self.sets
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name.trim()))
    }

    /// The set a view uses: its own choice, else the plan's active set. A
    /// choice that names no set (deleted since) falls back to the plan's.
    pub fn set_for(&self, view_choice: Option<&str>) -> Option<&LightSet> {
        view_choice
            .and_then(|n| self.set(n))
            .or_else(|| self.active_set.as_deref().and_then(|n| self.set(n)))
    }

    /// Adds a set `name` listing the lights `on`; `false` (nothing added) when
    /// the name is empty or taken. An existing set of the name is replaced
    /// only by [`Lighting::update_set`].
    pub fn add_set(&mut self, name: &str, on: Vec<crate::Id>, fixtures: Vec<crate::Id>) -> bool {
        let name = name.trim();
        if name.is_empty() || self.set(name).is_some() {
            return false;
        }
        self.sets.push(LightSet {
            name: name.to_string(),
            on,
            fixtures,
        });
        true
    }

    /// Replaces the lights of set `name`; `false` if there is no such set.
    pub fn update_set(&mut self, name: &str, on: Vec<crate::Id>, fixtures: Vec<crate::Id>) -> bool {
        match self
            .sets
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(name.trim()))
        {
            Some(s) => {
                s.on = on;
                s.fixtures = fixtures;
                true
            }
            None => false,
        }
    }

    /// Renames a set; `false` if the new name is empty or taken by another
    /// set, or there is no set `from`. The active set follows the rename.
    pub fn rename_set(&mut self, from: &str, to: &str) -> bool {
        let to = to.trim();
        if to.is_empty() {
            return false;
        }
        if self
            .set(to)
            .is_some_and(|s| !s.name.eq_ignore_ascii_case(from.trim()))
        {
            return false;
        }
        let Some(s) = self
            .sets
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(from.trim()))
        else {
            return false;
        };
        let old = std::mem::replace(&mut s.name, to.to_string());
        if self.active_set.as_deref() == Some(old.as_str()) {
            self.active_set = Some(to.to_string());
        }
        true
    }

    /// Deletes a set (and stops using it when it was the active one).
    pub fn remove_set(&mut self, name: &str) -> bool {
        let n = self.sets.len();
        self.sets
            .retain(|s| !s.name.eq_ignore_ascii_case(name.trim()));
        if self.set_for(None).is_none() {
            self.active_set = None;
        }
        self.sets.len() != n
    }

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
    #![allow(clippy::field_reassign_with_default)]
    use super::*;

    #[test]
    fn clip_annotations_and_steps_round_trip_and_old_files_read_with_defaults() {
        let mut v = CameraView::default();
        v.clip.clip_elevation = true;
        v.clip.plane.add_break(10.0).unwrap();
        v.clip.plane.set_offset(1, 18.0);
        v.move_step = 6.0;
        v.rotate_step = 30.0;
        v.annotations.push(ViewAnnotation {
            id: 1,
            kind: AnnotKind::Line {
                a: [0.0, 0.0],
                b: [10.0, 0.0],
            },
            surface: DrawSurface::drawing(),
            layer: "CAD".into(),
            weight: None,
        });
        let back: CameraView = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(back, v);
        // A view saved before these fields existed keeps clipping at its
        // line and starts with the usual steps.
        let old: CameraView = serde_json::from_str(r#"{"tilt_deg":0.0}"#).unwrap();
        assert!(old.clip.clip_sides && old.clip.poche);
        assert!(old.annotations.is_empty());
        assert_eq!((old.move_step, old.rotate_step), (24.0, 15.0));
    }

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
            symbol: true,
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
            quality: 0,
            ..WalkRecord::default()
        }
        .clamped();
        assert_eq!((w.fps, w.width, w.height, w.samples), (60.0, 64, 4096, 1));
        assert_eq!(w.quality, 1);
        assert_eq!(WalkRecord::default().format, RecordFormat::Video);
        assert!(RecordFormat::Both.video() && RecordFormat::Both.frames());
        assert!(!RecordFormat::Video.frames() && !RecordFormat::Frames.video());
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

    #[test]
    fn light_sets_are_named_unique_and_follow_a_rename() {
        let mut l = Lighting::default();
        assert!(l.add_set("Evening", vec![3, 4], vec![8]));
        assert!(
            !l.add_set("evening", vec![], vec![]),
            "names are unique, any case"
        );
        assert!(!l.add_set("  ", vec![], vec![]));
        assert!(l.add_set("Movie", vec![], vec![]));
        assert!(l.set("EVENING").unwrap().shines(4));
        assert!(!l.set("Evening").unwrap().shines(5));
        let evening = l.set("Evening").unwrap();
        assert!(
            evening.shines_fixture(8) && !evening.shines_fixture(3),
            "lights and fixtures are apart"
        );
        l.active_set = Some("Evening".into());
        assert_eq!(l.set_for(None).unwrap().name, "Evening");
        // A camera's own choice wins over the plan's.
        assert_eq!(l.set_for(Some("Movie")).unwrap().name, "Movie");
        assert_eq!(
            l.set_for(Some("Gone")).unwrap().name,
            "Evening",
            "a deleted choice falls back to the plan's set"
        );
        assert!(l.rename_set("Evening", "Dinner"));
        assert_eq!(l.active_set.as_deref(), Some("Dinner"));
        assert!(!l.rename_set("Dinner", "Movie"), "taken");
        assert!(l.update_set("dinner", vec![9], vec![]));
        assert!(l.set("Dinner").unwrap().shines(9));
        assert!(l.remove_set("Dinner"));
        assert_eq!(l.active_set, None, "the active set was deleted");
        assert!(!l.remove_set("Dinner"));
    }

    #[test]
    fn lighting_with_sets_round_trips_and_old_files_still_load() {
        let mut l = Lighting::default();
        l.add_set("Night", vec![1, 2, 3], vec![5]);
        l.active_set = Some("Night".into());
        let back: Lighting = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(back, l);
        let old: Lighting = serde_json::from_str(r#"{"ambient":0.2}"#).unwrap();
        assert!(old.sets.is_empty() && old.active_set.is_none());
    }

    #[test]
    fn fog_density_follows_the_distance_and_the_switch() {
        let mut f = Fog::default();
        assert_eq!(f.density_per_inch(), 0.0);
        f.on = true;
        f.distance_ft = 100.0;
        assert!((f.density_per_inch() - 1.0 / 1200.0).abs() < 1e-9);
        f.distance_ft = 1.0;
        assert!(
            (f.density_per_inch() - 1.0 / 120.0).abs() < 1e-9,
            "clamped to 10 ft"
        );
        f.distance_ft = f64::NAN;
        assert_eq!(f.density_per_inch(), 0.0);
    }

    #[test]
    fn ground_fog_floors_and_ortho_view_round_trip() {
        let mut v = CameraView::default();
        v.backdrop.ground = GroundKind::Color;
        v.backdrop.ground_color = [10, 20, 30];
        v.backdrop.fog = Fog {
            on: true,
            distance_ft: 250.0,
            color: Some([200, 210, 220]),
        };
        v.floors = FloorsDisplayed::Picked { from: 1, to: 2 };
        v.light_set = Some("Night".into());
        v.ortho = Some(OrthoView {
            kind: OrthoKind::ElevationLeft,
            target: [1.0, 2.0, 3.0],
            half_height: 480.0,
        });
        let back: CameraView = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(back, v);
        // A file from before the ground and fog options opens unchanged.
        let old: Backdrop = serde_json::from_str(r#"{"kind":"Color","color":[1,2,3]}"#).unwrap();
        assert_eq!(old.ground, GroundKind::Default);
        assert!(!old.fog.on);
    }

    #[test]
    fn picked_floors_come_back_in_order_and_in_range() {
        assert_eq!(FloorsDisplayed::All.range(1, 3), None);
        assert_eq!(FloorsDisplayed::ThisAndBelow.range(1, 3), Some((0, 1)));
        assert_eq!(FloorsDisplayed::ThisAndBelow.range(9, 3), Some((0, 2)));
        let pick = FloorsDisplayed::Picked { from: 2, to: 1 };
        assert_eq!(pick.range(0, 3), Some((1, 2)));
        assert_eq!(
            FloorsDisplayed::Picked { from: 0, to: 9 }.range(0, 2),
            Some((0, 1))
        );
        assert!(pick.is_picked() && !FloorsDisplayed::All.is_picked());
    }
}
