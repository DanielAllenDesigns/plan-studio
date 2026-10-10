//! The rest of the camera and section specification panels (manual pp. 1175,
//! 1176, 1187 to 1200; `docs/parity/3d-views-cameras.md` C-140, C-141, C-146
//! to C-151, C-153, C-156, C-158): Depth Cue, the Cross Section Slider planes,
//! Below Grade line overrides, the Selected Defaults of a view, Plan Display,
//! the Layer panel and the rendering options of the Camera panel.
//!
//! Everything here is plain data with the rules that act on it; the dialogs
//! edit it and the renderers read it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn yes() -> bool {
    true
}

// ---------------------------------------------------------------------
// Depth Cue (manual pp. 1175, 1176)
// ---------------------------------------------------------------------

/// Depth Cue: a partly transparent fog over what is far from the camera.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DepthCue {
    /// Use Depth Cue.
    pub on: bool,
    /// Keep Start/End in Sync: the two distances are always the same, which
    /// gives a sharp border where the fog begins.
    pub keep_in_sync: bool,
    /// Distance from the camera where the fog begins, inches.
    pub start: f64,
    /// Distance from the camera where the fog is at full opacity, inches.
    pub end: f64,
    /// Fog Opacity, 0 (clear) to 1 (nothing can be seen through it).
    pub opacity: f64,
    /// Fog Color.
    pub color: [u8; 3],
}

impl Default for DepthCue {
    fn default() -> Self {
        Self {
            on: false,
            keep_in_sync: false,
            start: 120.0,
            end: 480.0,
            opacity: 0.6,
            color: [255, 255, 255],
        }
    }
}

impl DepthCue {
    /// Types a Start distance. With the sync box checked the End follows; an
    /// End before the Start is pulled up to it.
    pub fn set_start(&mut self, v: f64) {
        self.start = v.max(0.0);
        if self.keep_in_sync || self.end < self.start {
            self.end = self.start;
        }
    }

    /// Types an End distance. With the sync box checked the Start follows; a
    /// Start after the End is pulled down to it.
    pub fn set_end(&mut self, v: f64) {
        self.end = v.max(0.0);
        if self.keep_in_sync || self.start > self.end {
            self.start = self.end;
        }
    }

    /// The sync box: checking it makes the two distances one (the Start
    /// wins).
    pub fn set_keep_in_sync(&mut self, on: bool) {
        self.keep_in_sync = on;
        if on {
            self.end = self.start;
        }
    }

    /// The fog over something `distance` inches from the camera, 0 to the
    /// Fog Opacity: nothing before Start, a linear ramp to End, full after
    /// (a step at Start when the two are the same).
    pub fn fog_at(&self, distance: f64) -> f64 {
        let op = self.opacity.clamp(0.0, 1.0);
        if distance <= self.start {
            0.0
        } else if self.end <= self.start || distance >= self.end {
            op
        } else {
            op * (distance - self.start) / (self.end - self.start)
        }
    }

    /// `(start, end, opacity)` for the drawing code, or `None` when off.
    pub fn ramp(&self) -> Option<(f64, f64, f64)> {
        self.on
            .then_some((self.start, self.end.max(self.start), self.opacity))
    }
}

// ---------------------------------------------------------------------
// Cross Section Slider (manual p. 1176)
// ---------------------------------------------------------------------

/// The side of the model a slider cutting plane starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SliderSide {
    /// The plane starts at the lowest plan X and moves toward higher X.
    Left,
    Right,
    /// Lowest plan Y (the bottom of the plan on screen).
    Front,
    Back,
    /// The highest point of the model, moving down.
    Top,
    Bottom,
}

impl SliderSide {
    pub const ALL: [SliderSide; 6] = [
        SliderSide::Left,
        SliderSide::Right,
        SliderSide::Front,
        SliderSide::Back,
        SliderSide::Top,
        SliderSide::Bottom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SliderSide::Left => "Cutting Plane Left",
            SliderSide::Right => "Cutting Plane Right",
            SliderSide::Front => "Cutting Plane Front",
            SliderSide::Back => "Cutting Plane Back",
            SliderSide::Top => "Cutting Plane Top",
            SliderSide::Bottom => "Cutting Plane Bottom",
        }
    }

    /// 0 for the plan X axis, 1 for the plan Y axis, 2 for height.
    fn axis(self) -> usize {
        match self {
            SliderSide::Left | SliderSide::Right => 0,
            SliderSide::Front | SliderSide::Back => 1,
            SliderSide::Top | SliderSide::Bottom => 2,
        }
    }

    /// Does the plane move from the high end of its axis?
    fn starts_high(self) -> bool {
        matches!(
            self,
            SliderSide::Right | SliderSide::Back | SliderSide::Top
        )
    }
}

/// One cutting plane of the slider.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SliderPlane {
    pub side: SliderSide,
    /// The check box: is this plane cutting?
    pub on: bool,
    /// Distance from the edge of the model that the plane cuts first, inches.
    pub position: f64,
}

/// The planes of the Cross Section Slider, saved with the camera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrossSectionSlider {
    pub planes: Vec<SliderPlane>,
}

impl Default for CrossSectionSlider {
    fn default() -> Self {
        Self {
            planes: SliderSide::ALL
                .iter()
                .map(|side| SliderPlane {
                    side: *side,
                    on: false,
                    position: 0.0,
                })
                .collect(),
        }
    }
}

impl CrossSectionSlider {
    /// Is any plane cutting?
    pub fn is_active(&self) -> bool {
        self.planes.iter().any(|p| p.on)
    }

    /// Does the slider cut away the point (plan X, plan Y, height) of a model
    /// that spans `lo` to `hi` (the same three numbers)?
    pub fn removes(&self, p: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> bool {
        self.planes.iter().filter(|pl| pl.on).any(|pl| {
            let a = pl.side.axis();
            if pl.side.starts_high() {
                p[a] > hi[a] - pl.position
            } else {
                p[a] < lo[a] + pl.position
            }
        })
    }

    /// The longest a plane's position may be: the extent of the model along
    /// the plane's axis.
    pub fn max_position(side: SliderSide, lo: [f64; 3], hi: [f64; 3]) -> f64 {
        let a = side.axis();
        (hi[a] - lo[a]).max(0.0)
    }

    /// Turns plane `side` on or off.
    pub fn set_on(&mut self, side: SliderSide, on: bool) {
        if let Some(p) = self.planes.iter_mut().find(|p| p.side == side) {
            p.on = on;
        }
    }

    /// Moves plane `side` (the slider and its typed text box).
    pub fn set_position(&mut self, side: SliderSide, position: f64) {
        if let Some(p) = self.planes.iter_mut().find(|p| p.side == side) {
            p.position = position.max(0.0);
        }
    }
}

// ---------------------------------------------------------------------
// Below Grade (manual pp. 1190, 1197, 1198)
// ---------------------------------------------------------------------

/// Where the Below Grade overrides stop applying.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BelowGradeLimit {
    /// Below the top surface of the terrain (a flat terrain is its elevation;
    /// without terrain, the first floor's level).
    Terrain,
    /// Below a horizontal plane at this height measured from zero, inches.
    Absolute(f64),
}

/// The names of the kinds of object the line overrides touch, shown for
/// reference in the panel (manual p. 1198).
pub const BELOW_GRADE_OBJECTS: [&str; 6] = [
    "Foundation walls and footings",
    "Slabs and foundation floors",
    "Piers, posts and beams below grade",
    "Walls below grade",
    "Stairs below grade",
    "Terrain features",
];

/// Line overrides for what lies below grade in a Vector View.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BelowGrade {
    pub override_color: bool,
    pub color: [u8; 3],
    pub override_style: bool,
    /// A line style name ("Dashed", "Dotted", "Solid"...).
    pub style: String,
    pub override_weight: bool,
    /// Line weight in points.
    pub weight: f64,
    pub limit: BelowGradeLimit,
}

impl Default for BelowGrade {
    fn default() -> Self {
        Self {
            override_color: false,
            color: [128, 128, 128],
            override_style: false,
            style: "Dashed".into(),
            override_weight: false,
            weight: 0.5,
            limit: BelowGradeLimit::Terrain,
        }
    }
}

impl BelowGrade {
    /// Does any override apply?
    pub fn is_active(&self) -> bool {
        self.override_color || self.override_style || self.override_weight
    }

    /// Is the line style a dashed one?
    pub fn dashed(&self) -> bool {
        self.override_style && !self.style.eq_ignore_ascii_case("solid")
    }

    /// The height below which the overrides apply, given the height of the
    /// terrain's top surface (`grade`).
    pub fn height(&self, grade: f64) -> f64 {
        match self.limit {
            BelowGradeLimit::Terrain => grade,
            BelowGradeLimit::Absolute(h) => h,
        }
    }
}

// ---------------------------------------------------------------------
// Selected Defaults (manual pp. 1190, 1198)
// ---------------------------------------------------------------------

/// The Saved Defaults and layer settings the annotations of a view use; the
/// same fields as the Active Defaults dialog. Empty names mean the plan's own
/// active choice.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewDefaults {
    /// The Default Set in use.
    pub default_set: String,
    /// Saved default name by `SavedKind::id`.
    pub picks: BTreeMap<String, String>,
    pub layer_set: String,
    /// The layer new CAD objects of this view go on.
    pub cad_layer: String,
}

impl ViewDefaults {
    /// Is nothing chosen (the view follows the plan's active defaults)?
    pub fn is_plan_default(&self) -> bool {
        self.default_set.is_empty()
            && self.picks.is_empty()
            && self.layer_set.is_empty()
            && self.cad_layer.is_empty()
    }

    /// The layer the annotations of this view are drawn on: its CAD layer if
    /// chosen, else `fallback`.
    pub fn annotation_layer<'a>(&'a self, fallback: &'a str) -> &'a str {
        if self.cad_layer.is_empty() {
            fallback
        } else {
            &self.cad_layer
        }
    }
}

// ---------------------------------------------------------------------
// Plan Display (manual pp. 1190, 1191, 1198 to 1200)
// ---------------------------------------------------------------------

/// Where the callout of a section sits on its clip plane line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CalloutPlacement {
    #[default]
    Center,
    LeftSide,
    RightSide,
    BothSides,
    /// At the Offset from Center.
    Custom,
}

impl CalloutPlacement {
    pub const ALL: [CalloutPlacement; 5] = [
        CalloutPlacement::Center,
        CalloutPlacement::LeftSide,
        CalloutPlacement::RightSide,
        CalloutPlacement::BothSides,
        CalloutPlacement::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CalloutPlacement::Center => "Center",
            CalloutPlacement::LeftSide => "Left Side",
            CalloutPlacement::RightSide => "Right Side",
            CalloutPlacement::BothSides => "Both Sides",
            CalloutPlacement::Custom => "Custom",
        }
    }

    /// Does the clip plane line show an arrow at its end(s)? Not for Center
    /// or Custom, which draw no clip plane line.
    pub fn draws_line(self) -> bool {
        !matches!(self, CalloutPlacement::Center | CalloutPlacement::Custom)
    }
}

/// The arrow of a callout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CalloutArrow {
    None,
    #[default]
    Small,
    Large,
}

impl CalloutArrow {
    pub const ALL: [CalloutArrow; 3] = [CalloutArrow::None, CalloutArrow::Small, CalloutArrow::Large];

    pub fn label(self) -> &'static str {
        match self {
            CalloutArrow::None => "None",
            CalloutArrow::Small => "Small",
            CalloutArrow::Large => "Large",
        }
    }
}

/// How the camera's symbol looks in the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanDisplay {
    /// Display on All Floors.
    pub all_floors: bool,
    pub placement: CalloutPlacement,
    /// Offset from Center for a Custom placement, inches along the line.
    pub offset: f64,
    /// The text inside the callout circle.
    pub callout_label: String,
    /// The bottom row of text.
    pub text_below: String,
    /// Text Below Line "Automatic": the camera's name (or the layout page
    /// label once the view is on a sheet).
    pub text_below_auto: bool,
    /// Callout Size, `None` is Automatic.
    pub callout_size: Option<f64>,
    pub arrow: CalloutArrow,
    pub arrow_filled: bool,
    /// Cross Section Line Style: `None` is By Layer.
    pub line_style: Option<String>,
    /// Cross Section Line Weight in points: `None` is By Layer.
    pub line_weight: Option<f64>,
    /// Camera Symbol Size in plan inches, `None` is the default.
    pub symbol_size: Option<f64>,
    pub show_focal_point: bool,
    pub show_fov_indicators: bool,
    /// FOV Indicator Length in plan inches, `None` is the default.
    pub fov_length: Option<f64>,
}

impl Default for PlanDisplay {
    fn default() -> Self {
        Self {
            all_floors: false,
            placement: CalloutPlacement::Center,
            offset: 0.0,
            callout_label: String::new(),
            text_below: String::new(),
            text_below_auto: true,
            callout_size: None,
            arrow: CalloutArrow::Small,
            arrow_filled: true,
            line_style: None,
            line_weight: None,
            symbol_size: None,
            show_focal_point: true,
            show_fov_indicators: true,
            fov_length: None,
        }
    }
}

impl PlanDisplay {
    /// The Text Below Line for a camera called `name`.
    pub fn text_below_for<'a>(&'a self, name: &'a str) -> &'a str {
        if self.text_below_auto {
            name
        } else {
            &self.text_below
        }
    }

    /// Is the camera's symbol shown on `floor` given the camera's own floor?
    pub fn shows_on(&self, camera_floor: usize, floor: usize) -> bool {
        self.all_floors || camera_floor == floor
    }
}

// ---------------------------------------------------------------------
// Layer panel (manual p. 1194)
// ---------------------------------------------------------------------

/// The Layer panel: the layer the camera symbol is on and its drawing group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewLayer {
    pub layer: String,
    /// The Drawing Group number of the symbol (0 to 999), or `None` for the
    /// group of cameras in Default Settings > Drawing Groups.
    pub drawing_group: Option<i32>,
}

/// The layer camera symbols are on.
pub const CAMERA_LAYER: &str = "Cameras";

impl Default for ViewLayer {
    fn default() -> Self {
        Self {
            layer: CAMERA_LAYER.into(),
            drawing_group: None,
        }
    }
}

// ---------------------------------------------------------------------
// Camera panel options (manual pp. 1187 to 1189, 1195, 1196)
// ---------------------------------------------------------------------

/// Super Resolution: render below native resolution, then upscale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SuperResolution {
    #[default]
    Native,
    /// 1.5 times upscaling.
    Quality,
    /// 2 times upscaling.
    Balanced,
    /// 3 times upscaling.
    Performance,
}

impl SuperResolution {
    pub const ALL: [SuperResolution; 4] = [
        SuperResolution::Native,
        SuperResolution::Quality,
        SuperResolution::Balanced,
        SuperResolution::Performance,
    ];

    /// The scaling factor listed after the name.
    pub fn factor(self) -> f64 {
        match self {
            SuperResolution::Native => 1.0,
            SuperResolution::Quality => 1.5,
            SuperResolution::Balanced => 2.0,
            SuperResolution::Performance => 3.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SuperResolution::Native => "Native Resolution (1x)",
            SuperResolution::Quality => "Quality (1.5x)",
            SuperResolution::Balanced => "Balanced (2x)",
            SuperResolution::Performance => "Performance (3x)",
        }
    }
}

/// How the lights of a view are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LightChoice {
    /// Automatic, up to a Maximum Number of lights.
    #[default]
    Automatic,
    /// A Light Set (`CameraView::light_set`).
    LightSet,
}

/// The rendering options of the Camera panel that the base view settings do
/// not already hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraOptions {
    /// Uncheck Show Color to draw the view in grey.
    #[serde(default = "yes")]
    pub show_color: bool,
    pub show_watermark: bool,
    /// Ray Casted Sun Shadows.
    pub ray_sun_shadows: bool,
    #[serde(default = "yes")]
    pub reflections: bool,
    #[serde(default = "yes")]
    pub animate_water: bool,
    pub light_bloom: bool,
    /// Ambient Occlusion amount, 0 to 1.
    pub ambient_occlusion: f64,
    /// Sharpening of textures, 0 to 1.
    pub sharpening: f64,
    pub super_resolution: SuperResolution,
    /// Depth of Field Enabled.
    pub dof_on: bool,
    pub f_stop: f64,
    /// Focus Distance, inches.
    pub focus_distance: f64,
    /// Use Sunlight.
    #[serde(default = "yes")]
    pub use_sunlight: bool,
    pub light_choice: LightChoice,
    /// Maximum Number of lights with Automatic lighting.
    pub max_lights: u32,
    /// Objects closer than this to the camera are not drawn, inches.
    pub clip_surfaces_within: f64,
    /// Hide Camera-Facing Exterior Walls.
    pub hide_facing_walls: bool,
}

/// The default Clip Surfaces Within, inches.
pub const DEFAULT_CLIP_WITHIN: f64 = 2.0;
/// The default Maximum Number of lights.
pub const DEFAULT_MAX_LIGHTS: u32 = 8;

impl Default for CameraOptions {
    fn default() -> Self {
        Self {
            show_color: true,
            show_watermark: false,
            ray_sun_shadows: false,
            reflections: true,
            animate_water: true,
            light_bloom: false,
            ambient_occlusion: 0.5,
            sharpening: 0.0,
            super_resolution: SuperResolution::Native,
            dof_on: false,
            f_stop: 5.6,
            focus_distance: 240.0,
            use_sunlight: true,
            light_choice: LightChoice::Automatic,
            max_lights: DEFAULT_MAX_LIGHTS,
            clip_surfaces_within: DEFAULT_CLIP_WITHIN,
            hide_facing_walls: false,
        }
    }
}

impl CameraOptions {
    /// Keeps every number inside what its slider or field allows.
    pub fn clamp(&mut self) {
        self.ambient_occlusion = self.ambient_occlusion.clamp(0.0, 1.0);
        self.sharpening = self.sharpening.clamp(0.0, 1.0);
        self.f_stop = self.f_stop.clamp(1.0, 32.0);
        self.focus_distance = self.focus_distance.max(1.0);
        self.max_lights = self.max_lights.clamp(1, 64);
        self.clip_surfaces_within = self.clip_surfaces_within.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_cue_start_and_end_stay_in_order_and_in_sync() {
        let mut d = DepthCue::default();
        d.set_start(300.0);
        assert!(d.end >= d.start, "the end is pulled up to the start");
        d.set_end(100.0);
        assert!(d.start <= d.end, "the start is pulled down to the end");
        d.set_keep_in_sync(true);
        assert_eq!(d.start, d.end);
        d.set_end(200.0);
        assert_eq!((d.start, d.end), (200.0, 200.0));
        d.set_start(50.0);
        assert_eq!((d.start, d.end), (50.0, 50.0));
    }

    #[test]
    fn depth_cue_fog_ramps_to_the_opacity() {
        let d = DepthCue {
            on: true,
            start: 100.0,
            end: 300.0,
            opacity: 0.5,
            ..DepthCue::default()
        };
        assert_eq!(d.fog_at(50.0), 0.0);
        assert!((d.fog_at(200.0) - 0.25).abs() < 1e-9);
        assert_eq!(d.fog_at(900.0), 0.5);
        assert_eq!(d.ramp(), Some((100.0, 300.0, 0.5)));
        let sharp = DepthCue {
            keep_in_sync: true,
            start: 100.0,
            end: 100.0,
            ..d
        };
        assert_eq!(sharp.fog_at(99.0), 0.0);
        assert_eq!(sharp.fog_at(101.0), 0.5);
        assert_eq!(DepthCue::default().ramp(), None);
    }

    #[test]
    fn slider_planes_remove_what_lies_between_the_edge_and_the_position() {
        let (lo, hi) = ([0.0, 0.0, 0.0], [400.0, 300.0, 200.0]);
        let mut s = CrossSectionSlider::default();
        assert!(!s.is_active());
        assert!(!s.removes([1.0, 1.0, 1.0], lo, hi));
        s.set_on(SliderSide::Left, true);
        s.set_position(SliderSide::Left, 100.0);
        assert!(s.removes([50.0, 150.0, 10.0], lo, hi));
        assert!(!s.removes([150.0, 150.0, 10.0], lo, hi));
        s.set_on(SliderSide::Top, true);
        s.set_position(SliderSide::Top, 80.0);
        // The top 80 inches are gone too: a second plane works with the first.
        assert!(s.removes([300.0, 100.0, 150.0], lo, hi));
        assert!(!s.removes([300.0, 100.0, 100.0], lo, hi));
        s.set_on(SliderSide::Right, true);
        s.set_position(SliderSide::Right, 40.0);
        assert!(s.removes([380.0, 100.0, 10.0], lo, hi));
        assert_eq!(CrossSectionSlider::max_position(SliderSide::Back, lo, hi), 300.0);
    }

    #[test]
    fn below_grade_overrides_apply_below_the_terrain_or_a_height() {
        let mut b = BelowGrade::default();
        assert!(!b.is_active());
        b.override_style = true;
        assert!(b.is_active() && b.dashed());
        b.style = "Solid".into();
        assert!(!b.dashed());
        assert_eq!(b.height(-6.0), -6.0);
        b.limit = BelowGradeLimit::Absolute(-18.0);
        assert_eq!(b.height(-6.0), -18.0);
    }

    #[test]
    fn plan_display_text_below_follows_the_name_until_it_is_typed() {
        let mut p = PlanDisplay::default();
        assert_eq!(p.text_below_for("Section A"), "Section A");
        p.text_below_auto = false;
        p.text_below = "SEE A-3".into();
        assert_eq!(p.text_below_for("Section A"), "SEE A-3");
        assert!(p.shows_on(0, 0) && !p.shows_on(0, 1));
        p.all_floors = true;
        assert!(p.shows_on(0, 1));
        assert!(CalloutPlacement::LeftSide.draws_line());
        assert!(!CalloutPlacement::Center.draws_line());
    }

    #[test]
    fn camera_options_clamp_and_old_files_get_the_defaults() {
        let mut o = CameraOptions {
            ambient_occlusion: 4.0,
            f_stop: 0.1,
            max_lights: 0,
            ..CameraOptions::default()
        };
        o.clamp();
        assert_eq!((o.ambient_occlusion, o.f_stop, o.max_lights), (1.0, 1.0, 1));
        let loaded: CameraOptions = serde_json::from_str("{}").unwrap();
        assert!(loaded.show_color && loaded.reflections && loaded.use_sunlight);
        assert_eq!(loaded.super_resolution.factor(), 1.0);
        let view: ViewLayer = serde_json::from_str("{}").unwrap();
        assert_eq!(view.layer, CAMERA_LAYER);
    }

    #[test]
    fn view_defaults_pick_the_annotation_layer() {
        let mut d = ViewDefaults::default();
        assert!(d.is_plan_default());
        assert_eq!(d.annotation_layer("Text"), "Text");
        d.cad_layer = "Detail".into();
        assert!(!d.is_plan_default());
        assert_eq!(d.annotation_layer("Text"), "Detail");
    }
}
