//! The 3D view panel (`docs/parity/3d-views-cameras.md`): replaces the plan
//! canvas while a 3D view is active.
//!
//! * A [`plan_view3d::Viewport3d`] shows the scene built by
//!   `plan_3d::build_scene`, rebuilt whenever the plan changed (a hash of the
//!   floors, walls and openings, C-54) or the view scope changed.
//! * Navigation is the viewport's: left-drag orbits (Mouse-Orbit is the
//!   default in the overviews), middle/right-drag or Shift+drag pans, scroll
//!   dollies, and Full Camera walks with W/A/S/D or the arrows and looks
//!   around with a drag (C-34..C-38). Page Up / Page Down raise and lower the
//!   eye (C-38). Esc returns to the plan; Tab cycles cameras.
//! * Overviews, Floor Overview (floors above are left out, C-11), Doll House
//!   (ceilings and roof hidden by the viewport, C-13), Full Camera (C-4) and
//!   cross section / elevation views (C-17..C-19) are entered through
//!   [`ViewRequest`]s from the Camera tool or [`View3dCommand`]s from the
//!   toolbar and menus.
//! * Rendering techniques (C-45) are mapped onto the viewport's settings
//!   (edges, flat shading, background) and a uniform-material scene override;
//!   see [`technique_view`].
//! * "Ray Trace…" opens the progressive ray tracer dialog (C-51) and
//!   `3D > Export > glTF…` writes the model with `plan_3d::gltf`.
//!
//! * **Vector elevations**: when the active camera is an elevation, section or
//!   wall elevation camera and the technique is Vector View or Technical
//!   Illustration, the panel draws the `plan_elevation` [`Drawing`] of the
//!   camera (`dialogs::camera::render_elevation_with`) instead of the GL scene:
//!   weighted lines, poche, shadows, hatch and labels, with pan and zoom. The
//!   drawing is made on a worker thread and remade whenever the project hash,
//!   the camera, the technique or the Sun Angle changes ([`VectorView`]).
//! * **Walkthroughs**: the panel plays a walkthrough camera along its path at
//!   its speed ([`View3dState::play_walkthrough`]) and records the walk as a
//!   numbered PNG sequence ([`record_walkthrough`]) with the path tracer.
//! * **Lights and sun**: plan lights and electrical fixtures feed the ray
//!   tracer's light list; the Sun Angle dialog drives the ray tracer, the GL
//!   key light and the vector elevations' shadows.
//!
//! * **Picking** (C-43, [`pick`]): a click ray-casts the scene and selects the
//!   object under the pointer in the editor's own selection (Shift adds), a
//!   double-click asks for its specification and Delete deletes it. The
//!   selection is tinted in the view. Pictures stay out of the cached scene:
//!   they are added per frame, billboards turned to face the camera.
//!
//! * **Camera view settings** ([`view_settings`], [`backdrop`]): a camera's
//!   Camera Specification tabs are saved on the camera (`CameraObject::view`)
//!   and shown with it: tilt, rendering technique, Preview or Final View,
//!   the backdrop (a colour, or a picture read from Chief's Backdrops folder),
//!   shadows, ambient and sun overrides, Floors Displayed and the Floor
//!   Camera's clip at its ceiling. 3D > Lighting sets the plan's sun and
//!   interior lights. **Save Camera** turns the view on screen into a camera
//!   (Project Browser > Cameras restores it); **Record Walkthrough** writes a
//!   numbered PNG sequence at a chosen frame rate.
//!
//! Roof planes come from `crate::editor::roof_view::roof_meshes`.

pub mod backdrop;
mod drag;
pub mod extras;
pub mod final_view;
pub mod nudge;
mod pick;
pub mod record;
mod stand_in;
mod textures;
pub mod view_settings;
pub mod zoom_history;

use crate::dialogs::camera::{
    elevation_options_with_sun, is_elevation_camera, render_elevation_with, sun_dir,
    AdjustLightsDialog, CameraDialog, CameraExtras, LightingDialog, RayTraceDialog, RecordDialog,
    RecordOutcome,
};
use crate::dialogs::Outcome;
use crate::editor::selection::Selection;
use crate::editor::EditorContext;
use crate::toolbar::ViewFlag;
use crate::tools::camera::{self as camera_tool, CameraVariant};
use crate::tools::{ToolId, ToolSet};
use eframe::egui;
use plan_3d::{build_scene_with, Material, Mesh, Scene, SceneOptions, Vertex};
use plan_core::camera::{DEFAULT_EYE_HEIGHT, DEFAULT_FOV_DEG};
use plan_core::camera_view::{ViewPose, ViewQuality};
use plan_core::geometry::Point;
use plan_core::{CameraKind, CameraObject, Id, PlacedSymbol, Project};
use plan_elevation::{Drawing, EdgeKind, LineWeight, Options, RegionKind, SunDir};
use plan_materials::{settings as technique_settings, FillMode, RenderingTechnique, ShadingModel};
use plan_view3d::{standard_views, CameraMode, Lighting, Look, Quality, ViewSettings, Viewport3d};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

/// Slack in front of a section line so the faces lying on it stay, inches.
const SECTION_TOLERANCE: f64 = 1.0;
/// The viewport's sky-blue background (`Viewport3d::new`).
const DEFAULT_BACKGROUND: [f32; 4] = [0.85, 0.89, 0.94, 1.0];
/// Keyboard eye-height speed in Full Camera, inches per second.
const EYE_RISE_SPEED: f32 = 60.0;

// ----- camera defaults (3D View Defaults, C-68) -----

/// Defaults for new cameras (Default Settings > Camera Tools).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraDefaults {
    /// Eye height above the floor, inches. Chief's default is 66" (C-5).
    pub eye_height: f64,
    /// Horizontal angle of view, degrees (C-7).
    pub fov_deg: f64,
}

static DEFAULTS: Mutex<CameraDefaults> = Mutex::new(CameraDefaults {
    eye_height: DEFAULT_EYE_HEIGHT,
    fov_deg: DEFAULT_FOV_DEG,
});

pub fn camera_defaults() -> CameraDefaults {
    *DEFAULTS.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn set_camera_defaults(d: CameraDefaults) {
    *DEFAULTS.lock().unwrap_or_else(PoisonError::into_inner) = d;
}

/// Back to the built-in camera defaults (tests share the process-wide value).
pub fn reset_camera_defaults() {
    set_camera_defaults(CameraDefaults {
        eye_height: DEFAULT_EYE_HEIGHT,
        fov_deg: DEFAULT_FOV_DEG,
    });
}

// ----- requests and commands -----

/// What the Camera tool (or another plan-side feature) asks the 3D panel to do.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewRequest {
    /// Open an overview in `mode`; `floor` limits it to that floor and below.
    Mode {
        mode: CameraMode,
        floor: Option<usize>,
    },
    /// Open the 3D view of a camera object.
    ShowCamera(Id),
    /// A camera was edited; update its 3D view if it is showing.
    RefreshCamera(Id),
    CameraDeleted(Id),
    /// Open the Camera Specification (C-30).
    OpenCameraSpec(Id),
    /// Open Adjust Lights, highlighting a light.
    OpenAdjustLights(Option<Id>),
    /// Save the view on screen as a camera (Project Browser > Cameras).
    SaveCamera,
    /// Open the Glass House overview.
    GlassHouse,
    /// Open the Lighting dialog.
    OpenLighting,
}

/// A queue of [`ViewRequest`]s shared between the plan tools and the panel.
#[derive(Clone, Default)]
pub struct Outbox(Arc<Mutex<Vec<ViewRequest>>>);

impl Outbox {
    /// The application-wide queue.
    pub fn global() -> Self {
        static GLOBAL: OnceLock<Outbox> = OnceLock::new();
        GLOBAL.get_or_init(Outbox::default).clone()
    }

    pub fn post(&self, r: ViewRequest) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(r);
    }

    pub fn take(&self) -> Vec<ViewRequest> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

/// Toolbar and menu commands of the 3D features (`Action::View3d`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View3dCommand {
    /// Open the 3D view in this mode, framing the whole plan.
    Mode(CameraMode),
    /// Perspective Floor Overview of the active floor (C-11).
    FloorOverview,
    /// Mouse-Orbit Camera (C-34).
    MouseOrbit,
    /// Cross Section Slider (C-23).
    CrossSectionSlider,
    /// Activate a plan tool variant (Full Camera, section camera, ...).
    Tool(CameraVariant),
    /// Rendering Techniques (C-45).
    Technique(RenderingTechnique),
    /// Rebuild 3D (C-54).
    Rebuild,
    /// 3D View Defaults (C-68).
    Defaults,
    /// Create Perspective View > Ray Trace (C-51).
    RayTrace,
    /// 3D > Export > glTF.
    ExportGltf,
    /// Adjust Lights (C-64).
    AdjustLights,
    /// Play the walkthrough camera along its path.
    PlayWalkthrough,
    /// Record the walkthrough as a PNG sequence.
    RecordWalkthrough,
    /// One step of the camera: Move, Orbit, Tilt, View Direction (C-38..C-41).
    Nudge(nudge::Nudge),
    /// Glass House overview (see-through walls).
    GlassHouse,
    /// Save Camera: keep the view on screen as a camera.
    SaveCamera,
    /// 3D > Lighting: the sun and interior lights.
    Lighting,
    /// Create Walkthrough Path from the selected CAD polyline.
    WalkFromCad,
    /// Preview (fast) or Final View (shadows, occlusion) quality.
    Quality(ViewQuality),
    /// Path-traced Final View on or off (C-53).
    FinalViewToggle,
    /// Undo Zoom: back to the camera before the last zoom, pan or move (C-42).
    UndoZoom,
    /// Refresh: redraw the 3D view and its vector drawing from the plan.
    Refresh,
    /// 3D > Export > 360 Panorama (C-77).
    ExportPanorama,
    /// Create Orthographic View: the Full, Floor and Framing Overviews and
    /// the Isometric Views in parallel projection (C-15).
    Parallel(ParallelOverview),
    /// Front, Back, Left or Right Elevation: the Auto Elevation tool for one
    /// side of the building (manual p. 1151).
    AutoSide(camera_tool::AutoSide),
}

/// The corner an Isometric View looks from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IsoCorner {
    SouthWest,
    SouthEast,
    NorthEast,
    NorthWest,
}

impl IsoCorner {
    pub const ALL: [IsoCorner; 4] = [
        IsoCorner::SouthWest,
        IsoCorner::SouthEast,
        IsoCorner::NorthEast,
        IsoCorner::NorthWest,
    ];

    pub fn label(self) -> &'static str {
        match self {
            IsoCorner::SouthWest => "Isometric View SW",
            IsoCorner::SouthEast => "Isometric View SE",
            IsoCorner::NorthEast => "Isometric View NE",
            IsoCorner::NorthWest => "Isometric View NW",
        }
    }

    /// The plan direction the view looks along: from the corner toward the
    /// middle of the building.
    pub fn look(self) -> Point {
        match self {
            IsoCorner::SouthWest => Point::new(1.0, 1.0),
            IsoCorner::SouthEast => Point::new(-1.0, 1.0),
            IsoCorner::NorthEast => Point::new(-1.0, -1.0),
            IsoCorner::NorthWest => Point::new(1.0, -1.0),
        }
    }
}

/// Which parallel-projection overview a [`View3dCommand::Parallel`] opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParallelOverview {
    /// Orthographic Full Overview.
    Full,
    /// Orthographic Floor Overview of the active floor.
    Floor,
    /// Orthographic Framing Overview.
    Framing,
    Isometric(IsoCorner),
}

impl ParallelOverview {
    pub fn label(self) -> &'static str {
        match self {
            ParallelOverview::Full => "Orthographic Full Overview",
            ParallelOverview::Floor => "Orthographic Floor Overview",
            ParallelOverview::Framing => "Orthographic Framing Overview",
            ParallelOverview::Isometric(c) => c.label(),
        }
    }
}

/// The yaw and pitch of an Isometric View: 45 degrees round and 30 degrees
/// down, the conventional angles (manual p. 1153; DECISIONS CS6).
pub fn isometric_angles(corner: IsoCorner) -> (f32, f32) {
    let d = corner.look().normalized();
    let yaw = (-d.x).atan2(d.y) as f32;
    (yaw, 30.0_f32.to_radians())
}

/// Runs a [`View3dCommand`]: the one match arm `main.rs` needs.
pub fn dispatch(
    cmd: View3dCommand,
    cx: &mut EditorContext,
    tools: &mut ToolSet,
    state: &mut View3dState,
) {
    match cmd {
        View3dCommand::Tool(v) => {
            // Cameras are placed in the plan.
            state.active = false;
            tools.set_active(cx, ToolId::CameraVariant(v));
        }
        View3dCommand::Mode(m) => state.open_mode(m, None),
        View3dCommand::FloorOverview => state.open_mode(CameraMode::Orbit, Some(cx.floor)),
        View3dCommand::MouseOrbit => {
            if state.active {
                state.set_mode_user(CameraMode::Orbit);
            } else {
                state.open_mode(CameraMode::Orbit, None);
            }
        }
        View3dCommand::CrossSectionSlider => {
            // In a camera view or overview the command opens the Cross
            // Section Slider dialog (several planes, saved with the camera);
            // elsewhere it opens the elevation slider.
            if state.active && state.section.is_none() && state.slider.is_none() {
                state.slider_dialog = true;
            } else {
                state.toggle_slider();
            }
        }
        View3dCommand::Technique(t) => state.set_technique(t),
        View3dCommand::Rebuild => {
            state.rebuild();
            cx.status = "Rebuilt the 3D model".into();
        }
        View3dCommand::Defaults => state.show_defaults = true,
        View3dCommand::RayTrace => {
            if !state.active {
                state.open_mode(CameraMode::Orbit, None);
            }
            state.raytrace.open = true;
        }
        View3dCommand::ExportGltf => export_gltf(cx),
        View3dCommand::AdjustLights => state.open_adjust_lights(&cx.project, None),
        View3dCommand::PlayWalkthrough => state.play_walkthrough(cx),
        View3dCommand::RecordWalkthrough => state.record_walkthrough_dialog(cx),
        View3dCommand::Nudge(n) => state.nudge_camera(cx, n),
        View3dCommand::GlassHouse => state.open_glass_house(),
        View3dCommand::SaveCamera => state.save_camera(cx),
        View3dCommand::Lighting => state.open_lighting(&cx.project),
        View3dCommand::WalkFromCad => state.walk_from_cad(cx),
        View3dCommand::Quality(q) => state.set_quality(q),
        View3dCommand::FinalViewToggle => {
            state.final_view.enabled = !state.final_view.enabled;
            state.final_view.clear();
            cx.status = if state.final_view.enabled {
                "Final View is path traced when the camera stops moving".into()
            } else {
                "Final View is the shaded view again".into()
            };
        }
        View3dCommand::UndoZoom => state.undo_zoom(cx),
        View3dCommand::Refresh => {
            state.refresh();
            cx.status = "Refreshed the 3D view".into();
        }
        View3dCommand::ExportPanorama => state.panorama_dialog(cx),
        View3dCommand::Parallel(p) => state.open_parallel(p, cx.floor),
        View3dCommand::AutoSide(side) => {
            camera_tool::set_next_auto_side(Some(side));
            state.active = false;
            tools.set_active(cx, ToolId::CameraVariant(CameraVariant::AutoElevation));
        }
    }
}

fn export_gltf(cx: &mut EditorContext) {
    let scene = build_view_scene(&cx.project, &ViewScope::default());
    if scene.meshes.is_empty() {
        cx.status = "Nothing to export: the 3D model is empty".into();
        return;
    }
    let Some(path) = rfd::FileDialog::new()
        .add_filter("glTF 2.0", &["gltf"])
        .set_file_name("model.gltf")
        .save_file()
    else {
        return;
    };
    let stem = path.with_extension("");
    cx.status = match plan_3d::gltf::write_gltf_files(&scene, &stem) {
        Ok(()) => format!("Exported {}.gltf", stem.display()),
        Err(e) => format!("Could not export glTF: {e}"),
    };
}

// ----- rendering techniques (C-45..C-53) -----

/// How a rendering technique drives the interactive viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TechniqueView {
    pub show_edges: bool,
    /// Flat (unlit) shading: no light falloff.
    pub flat: bool,
    pub background: [f32; 4],
    /// Draw every surface in this material (Clay, Line Drawing, Glass House).
    pub fill: Option<Material>,
    /// The interactive renderer's look: sky, shadows, occlusion, post edges
    /// and washes (`plan_view3d::Look`).
    pub look: Look,
}

/// Maps a technique from `plan-materials` onto viewport settings. Standard
/// keeps the edge overlay (C-46, C-69); the bitmap and ray-traced techniques
/// fall back to their closest interactive look.
pub fn technique_view(t: RenderingTechnique) -> TechniqueView {
    let s = technique_settings(t);
    let fill = match s.fill {
        FillMode::Material => None,
        FillMode::SolidColor(rgb) => Some(nearest_material(rgb)),
        FillMode::Transparent(_) => Some(Material::WindowGlass),
    };
    let background = match t {
        RenderingTechnique::VectorView
        | RenderingTechnique::TechnicalIllustration
        | RenderingTechnique::LineDrawing
        | RenderingTechnique::GlassHouse => [0.98, 0.98, 0.98, 1.0],
        RenderingTechnique::Watercolor => [0.97, 0.95, 0.90, 1.0],
        RenderingTechnique::Clay => [0.80, 0.80, 0.80, 1.0],
        RenderingTechnique::Duotone => [0.93, 0.89, 0.80, 1.0],
        RenderingTechnique::Standard | RenderingTechnique::PhysicallyBased => DEFAULT_BACKGROUND,
    };
    let look = match t {
        RenderingTechnique::Standard => Look::Standard,
        RenderingTechnique::PhysicallyBased => Look::Physical,
        RenderingTechnique::Clay => Look::Clay,
        RenderingTechnique::GlassHouse => Look::GlassHouse,
        RenderingTechnique::Watercolor => Look::Watercolor,
        RenderingTechnique::TechnicalIllustration => Look::Technical,
        RenderingTechnique::Duotone => Look::Duotone,
        RenderingTechnique::VectorView | RenderingTechnique::LineDrawing => Look::Flat,
    };
    TechniqueView {
        show_edges: s.edge_lines || t == RenderingTechnique::Standard,
        flat: s.shading != ShadingModel::Lit,
        background,
        fill,
        look,
    }
}

/// The opaque scene material whose colour is closest to `rgb`.
pub fn nearest_material(rgb: [u8; 3]) -> Material {
    const OPAQUE: [Material; 7] = [
        Material::WallExterior,
        Material::WallInterior,
        Material::Floor,
        Material::Ceiling,
        Material::DoorPanel,
        Material::WindowFrame,
        Material::Roof,
    ];
    let want = rgb.map(|c| f32::from(c) / 255.0);
    let dist = |m: &Material| {
        let c = m.color();
        (0..3).map(|i| (c[i] - want[i]).powi(2)).sum::<f32>()
    };
    OPAQUE
        .into_iter()
        .min_by(|a, b| dist(a).total_cmp(&dist(b)))
        .unwrap_or(Material::WallInterior)
}

/// Whether a technique paints textures in the interactive view: Standard and
/// the ray-traced Physically Based preview do; the line, flat, clay and
/// bitmap-filter techniques draw plain colours.
pub fn technique_shows_textures(t: RenderingTechnique) -> bool {
    matches!(
        t,
        RenderingTechnique::Standard | RenderingTechnique::PhysicallyBased
    )
}

/// Puts every mesh in `material`.
pub fn apply_fill(scene: &mut Scene, material: Material) {
    for m in &mut scene.meshes {
        m.material = material;
        m.color = None;
    }
}

fn apply_to_viewport(vp: &mut Viewport3d, tv: &TechniqueView, sun: Option<[f32; 3]>) {
    vp.show_edges = tv.show_edges;
    vp.background = tv.background;
    vp.look = tv.look;
    let base = Lighting::default();
    vp.lighting = if tv.flat {
        Lighting {
            ambient: 1.0,
            key: 0.0,
            ..base
        }
    } else {
        Lighting {
            key_dir: sun.unwrap_or(base.key_dir),
            ..base
        }
    };
}

// ----- sections (C-17..C-19) -----

/// The region a cross section keeps: beyond the cut line, between its ends,
/// and no further than the back clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionCut {
    /// Centre of the cut line.
    pub origin: Point,
    /// Unit viewing direction (away from the viewer, into the model).
    pub dir: Point,
    pub half_width: f64,
    pub back_clip: Option<f64>,
}

impl SectionCut {
    pub fn from_camera(c: &CameraObject) -> Self {
        Self {
            origin: c.position,
            dir: c.direction(),
            half_width: camera_tool::section_width(c) * 0.5,
            back_clip: camera_tool::back_clip(c),
        }
    }

    fn tangent(&self) -> Point {
        Point::new(self.dir.y, -self.dir.x)
    }

    /// Viewport yaw that looks along `dir` (see `plan_view3d::camera`: yaw 0
    /// looks toward plan +Y).
    pub fn yaw(&self) -> f32 {
        self.dir.angle() as f32 - FRAC_PI_2
    }

    /// The orthographic elevation closest to the viewing direction.
    pub fn elevation_mode(&self) -> CameraMode {
        match (self.yaw() / FRAC_PI_2).round() as i64 {
            n if n.rem_euclid(4) == 1 => CameraMode::ElevationRight,
            n if n.rem_euclid(4) == 2 => CameraMode::ElevationBack,
            n if n.rem_euclid(4) == 3 => CameraMode::ElevationLeft,
            _ => CameraMode::ElevationFront,
        }
    }

    /// Is the plan point kept by the cut?
    pub fn contains(&self, p: Point) -> bool {
        let v = p - self.origin;
        let depth = v.dot(self.dir);
        depth >= -SECTION_TOLERANCE
            && v.dot(self.tangent()).abs() <= self.half_width + SECTION_TOLERANCE
            && self.back_clip.is_none_or(|b| depth <= b)
    }

    fn hash_into(&self, h: &mut DefaultHasher) {
        for v in [
            self.origin.x,
            self.origin.y,
            self.dir.x,
            self.dir.y,
            self.half_width,
            self.back_clip.unwrap_or(-1.0),
        ] {
            v.to_bits().hash(h);
        }
    }
}

/// The viewing direction of an elevation mode in the plan.
fn mode_direction(m: CameraMode) -> Point {
    match m {
        CameraMode::ElevationRight => Point::new(-1.0, 0.0),
        CameraMode::ElevationBack => Point::new(0.0, -1.0),
        CameraMode::ElevationLeft => Point::new(1.0, 0.0),
        _ => Point::new(0.0, 1.0),
    }
}

/// Drops the triangles outside `cut`. Triangles are kept or dropped by
/// centroid, so a cut wall ends at its nearest triangle edge (no poche yet).
pub fn clip_scene(scene: &Scene, cut: &SectionCut) -> Scene {
    let mut out = Scene::default();
    for m in &scene.meshes {
        let mut indices = Vec::with_capacity(m.indices.len());
        for tri in m.indices.as_chunks::<3>().0 {
            let (mut x, mut z) = (0.0_f64, 0.0_f64);
            for &i in tri {
                let p = m.vertices[i as usize].position;
                x += f64::from(p[0]);
                z += f64::from(p[2]);
            }
            // Scene Z is the negated plan Y.
            if cut.contains(Point::new(x / 3.0, -z / 3.0)) {
                indices.extend_from_slice(tri);
            }
        }
        if !indices.is_empty() {
            out.meshes.push(Mesh {
                vertices: m.vertices.clone(),
                indices,
                material: m.material,
                object_id: m.object_id,
                color: m.color,
            });
        }
    }
    out
}

// ----- scene building -----

/// What part of the plan a view shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewScope {
    /// Only this floor and the ones below (Floor Overview, C-11).
    pub floor: Option<usize>,
    pub section: Option<SectionCut>,
    /// Draw everything in one material (technique override).
    pub fill: Option<Material>,
    /// Leave the pictures out: the interactive view adds them every frame so
    /// billboards can turn with the camera ([`pick::picture_meshes`]).
    pub no_images: bool,
    /// Nothing above this height (a Floor Camera's ceiling), inches.
    pub clip_above: Option<f64>,
    /// Nothing centred below this height (the lowest floor a camera picks).
    pub clip_below: Option<f64>,
    /// The camera's Cross Section Slider planes (C-141).
    pub slider: view_settings::SliderClip,
    /// Hide the exterior walls that face a camera at this plan point
    /// (C-151).
    pub hide_from: Option<Point>,
    /// Show Color is off: draw everything in grey (C-146).
    pub gray: bool,
}

/// The scene a view shows: the scoped floors, any section cut, and the
/// technique's material override.
pub fn build_view_scene(project: &Project, scope: &ViewScope) -> Scene {
    let scoped;
    let proj = match scope.floor {
        Some(f) if f + 1 < project.floors.len() => {
            scoped = {
                let mut p = project.clone();
                p.floors.truncate(f + 1);
                p
            };
            &scoped
        }
        _ => project,
    };
    // Objects whose height is measured from the ceiling, the roof, the terrain
    // ... (Elevation Reference) stand where that puts them.
    let referenced = crate::dialogs::elevation_ref::effective_project(proj);
    let proj = referenced.as_ref().unwrap_or(proj);
    // Library doors (DW-126): the symbol stands in the opening where the
    // catalog can draw it; the built-in panel door stays where it cannot.
    let library_doors = crate::tools::library::door_library::scene_doors(proj);
    let proj = library_doors.as_ref().map_or(proj, |(p, _)| p);
    // The plan's own opening display: casing, jambs and sills, doors open.
    let mut scene = build_scene_with(proj, &SceneOptions::for_project(proj));
    if let Some((_, meshes)) = &library_doors {
        scene.meshes.extend(meshes.iter().cloned());
    }
    // Walls already follow the roof (`build_scene` reads the roof records);
    // the planes themselves and their eave detail come on top.
    scene
        .meshes
        .extend(crate::editor::roof_view::roof_meshes(proj));
    scene
        .meshes
        .extend(crate::editor::roof_view::roof_detail_meshes(proj));
    scene
        .meshes
        .extend(plan_3d::foundation::foundation_meshes(proj));
    scene
        .meshes
        .extend(crate::editor::framing_view::manual_framing_meshes(proj));
    scene
        .meshes
        .extend(plan_electrical::electrical_meshes(proj));
    scene
        .meshes
        .extend(crate::editor::details_view::detail_meshes(proj));
    scene.meshes.extend(symbol_meshes(proj));
    // Pictures (billboards keep their stored angle here; the interactive view
    // adds them per frame instead), 3D solid features and library solids.
    if !scope.no_images {
        scene
            .meshes
            .extend(crate::editor::placed::image_meshes(proj));
    }
    scene
        .meshes
        .extend(crate::editor::placed::solid_meshes(proj));
    // Terrain surface, roads and landscape objects.
    if let Some(view) = crate::editor::site_view::terrain_view(proj) {
        if let Some(surface) = &view.surface {
            scene.meshes.push(plan_terrain::terrain_mesh_for(
                &view.record.terrain,
                surface,
            ));
            scene
                .meshes
                .extend(plan_terrain::road_meshes(&view.record.terrain, surface));
        }
    }
    scene
        .meshes
        .extend(crate::editor::site_view::terrain_feature_meshes(proj));
    scene.meshes.extend(cabinet_meshes(proj));
    scene.meshes.extend(stair_meshes(proj));
    let scene = match scope.clip_above {
        Some(y) => view_settings::clip_above(&scene, y),
        None => scene,
    };
    let scene = match scope.clip_below {
        Some(y) => view_settings::clip_below(&scene, y),
        None => scene,
    };
    let scene = match scope.hide_from {
        Some(eye) => {
            let hidden = plan_core::camera_view::facing::facing_exterior(project, eye);
            if hidden.is_empty() {
                scene
            } else {
                view_settings::without_objects(&scene, &hidden)
            }
        }
        None => scene,
    };
    let scene = if scope.slider.is_active() {
        scope.slider.apply(&scene)
    } else {
        scene
    };
    let mut scene = match &scope.section {
        Some(cut) => clip_scene(&scene, cut),
        None => scene,
    };
    // Objects painted with the Material Painter / Adjust Materials.
    crate::tools::materials::apply_overrides(project, &mut scene);
    if let Some(m) = scope.fill {
        apply_fill(&mut scene, m);
    }
    if scope.gray {
        view_settings::gray_scene(&mut scene);
    }
    scene
}

/// A box of `symbol`'s width x depth x height on its footprint, in the
/// symbol's trim material, for placed items without 3D geometry.
pub fn symbol_box(symbol: &PlacedSymbol, floor_elevation: f64) -> Mesh {
    let foot = symbol.footprint();
    let y0 = (floor_elevation + symbol.elevation) as f32;
    let y1 = y0 + symbol.height.max(0.0) as f32;
    let at = |p: Point, y: f32| [p.x as f32, y, -p.y as f32];
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        material: Material::Trim,
        object_id: Some(symbol.id),
        color: None,
    };
    let mut quad = |c: [[f32; 3]; 4], n: [f32; 3]| {
        let base = mesh.vertices.len() as u32;
        let (a, b, d) = (c[0], c[1], c[2]);
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [d[0] - a[0], d[1] - a[1], d[2] - a[2]];
        let cross = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let flip = cross[0] * n[0] + cross[1] * n[1] + cross[2] * n[2] < 0.0;
        for p in c {
            mesh.vertices.push(Vertex {
                position: p,
                normal: n,
                uv: [0.0, 0.0],
            });
        }
        let order: [u32; 6] = if flip {
            [0, 2, 1, 0, 3, 2]
        } else {
            [0, 1, 2, 0, 2, 3]
        };
        mesh.indices.extend(order.iter().map(|i| base + i));
    };
    quad(
        [
            at(foot[0], y1),
            at(foot[1], y1),
            at(foot[2], y1),
            at(foot[3], y1),
        ],
        [0.0, 1.0, 0.0],
    );
    quad(
        [
            at(foot[0], y0),
            at(foot[1], y0),
            at(foot[2], y0),
            at(foot[3], y0),
        ],
        [0.0, -1.0, 0.0],
    );
    let centre = Point::new(
        foot.iter().map(|p| p.x).sum::<f64>() / 4.0,
        foot.iter().map(|p| p.y).sum::<f64>() / 4.0,
    );
    for i in 0..4 {
        let (p, q) = (foot[i], foot[(i + 1) % 4]);
        // Outward in the plan, then into scene axes (Z = -plan y).
        let mid = Point::new((p.x + q.x) / 2.0 - centre.x, (p.y + q.y) / 2.0 - centre.y);
        let len = mid.length().max(1e-9);
        let n = [(mid.x / len) as f32, 0.0, (-mid.y / len) as f32];
        quad([at(p, y0), at(q, y0), at(q, y1), at(p, y1)], n);
    }
    mesh
}

/// 3D geometry of every placed symbol: Chief objects use their decoded
/// meshes (or a box when the geometry is missing, partial or the catalog is
/// not available), everything else is a box so each placed item shows
/// something.
pub fn symbol_meshes(project: &Project) -> Vec<Mesh> {
    use crate::tools::library::chief::{self, Chief3d};
    let mut out = Vec::new();
    for floor in &project.floors {
        for s in &floor.symbols {
            // Pictures and solids are meshed by `editor::placed`.
            if s.image.is_some() || s.solid {
                continue;
            }
            // User Catalog 3D models (OBJ / glTF imports).
            if let Some(m) = crate::tools::library::user::placed_meshes(s, floor.elevation) {
                out.extend(m);
                continue;
            }
            if chief::is_chief_id(&s.catalog_id) {
                match chief::placed_meshes(s, floor.elevation) {
                    Chief3d::Meshes(m) => out.extend(m),
                    Chief3d::Box => out.push(symbol_box(s, floor.elevation)),
                    // The catalog is not installed: a block (labelled in
                    // the view, `stand_in`) rather than nothing.
                    Chief3d::Missing => out.push(symbol_box(s, floor.elevation)),
                }
            } else {
                out.push(symbol_box(s, floor.elevation));
            }
        }
    }
    out
}

/// 3D geometry of every placed cabinet (QA-05): `plan_cabinets::meshes` for
/// all kinds (base, wall, corner, fillers, custom countertops and
/// backsplashes ...) at each cabinet's stored position and rotation, lifted
/// by the floor's elevation. `plan-cabinets` has no wood or stone materials:
/// its countertop stand-in (`Floor`) becomes `Stone` so it is not mistaken
/// for a room slab, and its handle stand-in (`WindowFrame`) becomes `Metal`.
pub fn cabinet_meshes(project: &Project) -> Vec<Mesh> {
    let mut out = Vec::new();
    for floor in &project.floors {
        for cab in crate::editor::placed::load_cabinets(floor) {
            for mut m in plan_cabinets::meshes(&cab) {
                m.material = match m.material {
                    Material::Floor => Material::Stone,
                    Material::WindowFrame => Material::Metal,
                    other => other,
                };
                lift(&mut m, floor.elevation);
                out.push(m);
            }
        }
    }
    out
}

/// 3D geometry of every stair, ramp and landing (QA-06):
/// `plan_stairs::model3d` on each stair at its floor's elevation. Treads,
/// landings and ramps are `Framing` (lumber) so `Floor` stays the room
/// slabs; risers, stringers and handrails are `Trim`.
pub fn stair_meshes(project: &Project) -> Vec<Mesh> {
    use plan_stairs::StairPart;
    let mut out = Vec::new();
    for floor in &project.floors {
        for mut obj in crate::editor::stairs_view::load(floor) {
            obj.stair.floor_elevation = floor.elevation;
            for (part, mut m) in crate::editor::stairs_view::part_meshes(&obj, floor.elevation) {
                m.material = match part {
                    StairPart::Tread | StairPart::Landing | StairPart::Ramp => Material::Framing,
                    StairPart::Riser
                    | StairPart::Stringer
                    | StairPart::Handrail
                    | StairPart::Runner => Material::Trim,
                };
                out.push(m);
            }
        }
    }
    out
}

/// Does the project have a picture (Create Image / Billboard)?
fn has_pictures(project: &Project) -> bool {
    project
        .floors
        .iter()
        .any(|f| f.symbols.iter().any(|s| s.image.is_some()))
}

/// Raise a mesh by `dy` inches (cabinets are authored relative to the floor).
fn lift(mesh: &mut Mesh, dy: f64) {
    if dy != 0.0 {
        for v in &mut mesh.vertices {
            v.position[1] += dy as f32;
        }
    }
}

struct HashFmt<'a>(&'a mut DefaultHasher);

impl std::fmt::Write for HashFmt<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

/// A hash of everything that shapes the 3D model: floors, walls, openings,
/// placed symbols, room names, cabinets, stairs, the roofs and the
/// foundation objects.
/// Cameras, dimensions and other annotations are not part of it.
pub fn project_hash(p: &Project) -> u64 {
    let mut h = DefaultHasher::new();
    p.floors.len().hash(&mut h);
    for f in &p.floors {
        f.elevation.to_bits().hash(&mut h);
        f.ceiling_height.to_bits().hash(&mut h);
        for w in &f.walls {
            let _ = write!(HashFmt(&mut h), "{w:?}");
        }
        for o in &f.openings {
            let _ = write!(HashFmt(&mut h), "{o:?}");
        }
        // Placed library symbols (position, size, angle, flip, elevation).
        for sym in &f.symbols {
            let _ = write!(HashFmt(&mut h), "{sym:?}");
        }
        // Room names carry the ceiling height and floor offset overrides
        // that shape the platforms (R-23, R-24).
        for n in &f.room_names {
            let _ = write!(HashFmt(&mut h), "{n:?}");
        }
        // Cabinets and stairs live in typed JSON slots (`editor::placed`,
        // `editor::stairs_view`) and are meshed into the scene.
        for c in &f.cabinets {
            let _ = write!(HashFmt(&mut h), "{c}");
        }
        for st in &f.stairs {
            let _ = write!(HashFmt(&mut h), "{st}");
        }
        // Roof planes are stored as tagged CAD records (`editor::roof_view`).
        for c in &f.cad {
            let _ = write!(HashFmt(&mut h), "{c:?}");
        }
        // Roofs live in the floor's typed `roofs` slot (`editor::roof_view`).
        for r in &f.roofs {
            let _ = write!(HashFmt(&mut h), "{r:?}");
        }
        // Slabs, pads and piers live in the typed `foundation` slot.
        if let Some(v) = &f.foundation {
            let _ = write!(HashFmt(&mut h), "{v:?}");
        }
        // Corner boards, moldings, regions, decks and solids (`details`).
        if let Some(v) = &f.details {
            let _ = write!(HashFmt(&mut h), "{v:?}");
        }
        // Built and manual framing, and the electrical layer (both drawn in 3D).
        let _ = write!(HashFmt(&mut h), "{:?}{:?}", f.framing, f.electrical);
        // Floor Defaults: finish thicknesses, default surface materials and
        // the foundation record (footings) shape the platforms and surfaces.
        let _ = write!(HashFmt(&mut h), "{:?}{:?}", f.settings, f.kind);
    }
    p.wall_types.len().hash(&mut h);
    let _ = write!(HashFmt(&mut h), "{:?}", p.opening_display);
    // Painted materials recolor the meshes of their objects.
    for o in &p.object_materials {
        let _ = write!(HashFmt(&mut h), "{o:?}");
    }
    // ...and so do the Materials Defaults of their classes.
    for d in &p.material_defaults {
        let _ = write!(HashFmt(&mut h), "{d:?}");
    }
    // The terrain (surface, roads, landscape objects).
    if let Some(t) = &p.terrain {
        let _ = write!(HashFmt(&mut h), "{t:?}");
    }
    // What each object's height is measured from (Elevation Reference).
    for (k, pages) in &p.props.pages {
        if let Some(e) = pages.elevation {
            let _ = write!(HashFmt(&mut h), "{k}{e:?}");
        }
    }
    h.finish()
}

/// Horizontal to vertical field of view for a viewport of `aspect` (w/h).
pub fn vertical_fov(horizontal_deg: f32, aspect: f32) -> f32 {
    let half = (horizontal_deg.clamp(5.0, 170.0).to_radians() * 0.5).tan();
    (2.0 * (half / aspect.max(0.1)).atan())
        .to_degrees()
        .clamp(10.0, 120.0)
}

/// Vertical to horizontal field of view for a viewport of `aspect` (w/h):
/// the inverse of [`vertical_fov`] (before its limits).
pub fn horizontal_fov(vertical_deg: f32, aspect: f32) -> f64 {
    let half = (vertical_deg.to_radians() * 0.5).tan() * aspect.max(0.1);
    (2.0 * f64::from(half.atan()))
        .to_degrees()
        .clamp(5.0, 170.0)
}

/// The folder Record Walkthrough proposes: a folder named for the camera in
/// the user's Documents.
pub fn default_record_folder(camera_name: &str) -> String {
    let clean: String = camera_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                '-'
            }
        })
        .collect();
    crate::paths::home_dir()
        .map(|h| h.join("Documents").join(format!("{} frames", clean.trim())))
        .map_or_else(
            || format!("{} frames", clean.trim()),
            |p| p.display().to_string(),
        )
}

// ----- vector elevations -----

/// Raster size of the depth buffer behind the on-screen vector drawings: the
/// plan_elevation default is finer than a screen needs and slow to make.
const VECTOR_RASTER_PX: usize = 1024;

/// Is this a technique that draws elevations as vectors?
pub fn is_vector_technique(t: RenderingTechnique) -> bool {
    matches!(
        t,
        RenderingTechnique::VectorView | RenderingTechnique::TechnicalIllustration
    )
}

/// The drawing a vector elevation shows: the camera's drawing with the plan's
/// sun (when given) casting the shadows. Technical Illustration always casts
/// shadows, from the camera's own sun when the plan has none.
pub fn vector_drawing(
    project: &Project,
    cam: &CameraObject,
    technique: RenderingTechnique,
    sun: Option<SunDir>,
    raster_px: usize,
) -> Drawing {
    let mut sun = sun;
    if sun.is_none() && technique == RenderingTechnique::TechnicalIllustration {
        sun = Some(SunDir {
            azimuth_deg: cam.render.sun_azimuth_deg,
            altitude_deg: cam.render.sun_altitude_deg,
        });
    }
    let opts = Options {
        raster_px,
        ..elevation_options_with_sun(cam, sun)
    };
    render_elevation_with(project, cam, &opts)
}

/// Splits a ring into trapezoids between horizontal cuts at every vertex
/// height, filled by the even-odd rule (so the zero-width slits that join
/// holes to the outer ring fill correctly). Each quad is
/// `[left-bottom, right-bottom, right-top, left-top]`.
pub fn trapezoids(ring: &[Point]) -> Vec<[Point; 4]> {
    let n = ring.len();
    if n < 3 {
        return Vec::new();
    }
    let mut ys: Vec<f64> = ring.iter().map(|p| p.y).collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let at = |a: Point, b: Point, y: f64| a.x + (b.x - a.x) * (y - a.y) / (b.y - a.y);
    let mut out = Vec::new();
    for w in ys.windows(2) {
        let (y0, y1) = (w[0], w[1]);
        let mid = (y0 + y1) * 0.5;
        // x at the bottom and top of the slab for every edge crossing it.
        let mut xs: Vec<(f64, f64, f64)> = (0..n)
            .filter_map(|i| {
                let (a, b) = (ring[i], ring[(i + 1) % n]);
                let (lo, hi) = if a.y < b.y { (a, b) } else { (b, a) };
                (lo.y <= mid && mid < hi.y && hi.y > lo.y)
                    .then(|| (at(lo, hi, mid), at(lo, hi, y0), at(lo, hi, y1)))
            })
            .collect();
        xs.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pair in xs.as_chunks::<2>().0 {
            let (l, r) = (pair[0], pair[1]);
            out.push([
                Point::new(l.1, y0),
                Point::new(r.1, y0),
                Point::new(r.2, y1),
                Point::new(l.2, y1),
            ]);
        }
    }
    out
}

/// Area of a trapezoid list.
#[cfg(test)]
pub fn trapezoid_area(quads: &[[Point; 4]]) -> f64 {
    quads
        .iter()
        .map(|q| plan_core::geometry::polygon_area(q).abs())
        .sum()
}

/// A region of the drawing prepared for painting.
struct PaintRegion {
    kind: RegionKind,
    material: Material,
    quads: Vec<[Point; 4]>,
}

/// The vector elevation on screen: the latest drawing, the job making the
/// next one, and the pan and zoom of the view.
pub struct VectorView {
    /// Key of the drawing in [`drawing`](Self::drawing).
    key: u64,
    drawing: Option<Arc<Drawing>>,
    regions: Vec<PaintRegion>,
    /// The job in flight: its key and where it leaves the result.
    pending: Option<(u64, Arc<Mutex<Option<Drawing>>>)>,
    /// Screen offset of the drawing's centre from the middle of the view.
    pan: egui::Vec2,
    /// 1.0 fits the drawing to the view.
    zoom: f32,
    /// Depth buffer size of new drawings (tests lower it).
    pub raster_px: usize,
}

impl Default for VectorView {
    fn default() -> Self {
        Self {
            key: 0,
            drawing: None,
            regions: Vec::new(),
            pending: None,
            pan: egui::Vec2::ZERO,
            zoom: 1.0,
            raster_px: VECTOR_RASTER_PX,
        }
    }
}

impl VectorView {
    /// The drawing on screen, if one has been made.
    pub fn drawing(&self) -> Option<&Drawing> {
        self.drawing.as_deref()
    }

    /// Is a drawing being made?
    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    fn install(&mut self, key: u64, d: Drawing) {
        self.regions = d
            .regions
            .iter()
            .map(|r| PaintRegion {
                kind: r.kind,
                material: r.material,
                quads: trapezoids(&r.polygon),
            })
            .collect();
        self.drawing = Some(Arc::new(d));
        self.key = key;
    }

    /// Takes the result of a finished job.
    fn poll(&mut self) {
        let done = match &self.pending {
            Some((key, slot)) => slot
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .map(|d| (*key, d)),
            None => None,
        };
        if let Some((key, d)) = done {
            self.pending = None;
            self.install(key, d);
        }
    }

    /// Does a drawing for `key` still have to be made? (Polls the job in
    /// flight first, so a finished drawing is taken before answering.)
    fn needs(&mut self, key: u64) -> bool {
        self.poll();
        if self.key == key && self.drawing.is_some() {
            return false;
        }
        !self.pending.as_ref().is_some_and(|(k, _)| *k == key)
    }

    /// Starts a worker thread making the drawing for `key`; the result is
    /// taken by the next [`needs`](Self::needs) after it finishes.
    fn start(&mut self, key: u64, make: impl FnOnce() -> Drawing + Send + 'static) {
        let slot = Arc::new(Mutex::new(None));
        let out = Arc::clone(&slot);
        let spawned = std::thread::Builder::new()
            .name("vector-elevation".into())
            .spawn(move || {
                let d = std::panic::catch_unwind(std::panic::AssertUnwindSafe(make))
                    .unwrap_or_default();
                *out.lock().unwrap_or_else(PoisonError::into_inner) = Some(d);
            });
        if spawned.is_ok() {
            self.pending = Some((key, slot));
        }
    }

    /// Forces the next [`needs`](Self::needs) to ask for a new drawing.
    fn invalidate(&mut self) {
        self.key = 0;
        self.pending = None;
    }

    /// Fit transform for a view rectangle: drawing space (Y up) to screen.
    fn transform(&self, rect: egui::Rect) -> Option<VectorXform> {
        let d = self.drawing.as_deref()?;
        let (lo, hi) = drawing_extent(d);
        let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
        let margin = 0.06;
        let fit = (f64::from(rect.width()) / (w * (1.0 + 2.0 * margin)))
            .min(f64::from(rect.height()) / (h * (1.0 + 2.0 * margin)));
        Some(VectorXform {
            centre: Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5),
            scale: fit * f64::from(self.zoom),
            origin: rect.center() + self.pan,
        })
    }

    /// Zooms by `factor` keeping the drawing point under `at` fixed.
    fn zoom_about(&mut self, rect: egui::Rect, at: egui::Pos2, factor: f32) {
        let Some(before) = self.transform(rect) else {
            return;
        };
        let anchor = before.to_drawing(at);
        self.zoom = (self.zoom * factor).clamp(0.05, 60.0);
        if let Some(after) = self.transform(rect) {
            let now = after.to_screen(anchor);
            self.pan += at - now;
        }
    }

    fn reset_view(&mut self) {
        self.pan = egui::Vec2::ZERO;
        self.zoom = 1.0;
    }
}

/// Drawing space (inches, Y up) to screen pixels.
#[derive(Clone, Copy, Debug)]
pub struct VectorXform {
    centre: Point,
    scale: f64,
    origin: egui::Pos2,
}

impl VectorXform {
    pub fn to_screen(self, p: Point) -> egui::Pos2 {
        egui::pos2(
            self.origin.x + ((p.x - self.centre.x) * self.scale) as f32,
            self.origin.y - ((p.y - self.centre.y) * self.scale) as f32,
        )
    }

    pub fn to_drawing(self, s: egui::Pos2) -> Point {
        Point::new(
            self.centre.x + f64::from(s.x - self.origin.x) / self.scale,
            self.centre.y - f64::from(s.y - self.origin.y) / self.scale,
        )
    }
}

/// Size of the messages painted over the 3D view and the vector view
/// (at least the 15 pt body text of the rest of the interface).
const OVERLAY_TEXT_PX: f32 = 16.0;
/// Height of drawing text, inches (the annotations are about this tall).
const DRAWING_TEXT_IN: f64 = 6.0;

/// Bounds of the lines, regions and text of a drawing.
pub fn drawing_extent(d: &Drawing) -> (Point, Point) {
    let (mut lo, mut hi) = d.bounds;
    let mut grow = |p: Point| {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    };
    for r in &d.regions {
        r.polygon.iter().for_each(|&p| grow(p));
    }
    for (p, t) in &d.texts {
        grow(*p);
        grow(Point::new(
            p.x + 0.6 * DRAWING_TEXT_IN * t.chars().count() as f64,
            p.y + DRAWING_TEXT_IN,
        ));
    }
    (lo, hi)
}

fn color32(c: [f32; 4]) -> egui::Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    egui::Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
}

/// Stroke width in pixels and colour of a line of the drawing.
fn line_style(
    kind: EdgeKind,
    weight: LineWeight,
    technique: RenderingTechnique,
) -> (f32, egui::Color32) {
    let tech = technique == RenderingTechnique::TechnicalIllustration;
    match (kind, weight) {
        (EdgeKind::Cut, _) => (if tech { 3.0 } else { 2.4 }, egui::Color32::BLACK),
        (EdgeKind::Hatch, _) => (0.5, egui::Color32::from_gray(0x70)),
        (EdgeKind::Annotation, _) => (0.8, egui::Color32::from_gray(0x30)),
        (_, LineWeight::Heavy) => (if tech { 2.6 } else { 1.8 }, egui::Color32::BLACK),
        (_, LineWeight::Medium) => (1.1, egui::Color32::from_gray(0x22)),
        (_, LineWeight::Light) => (0.6, egui::Color32::from_gray(0x55)),
    }
}

/// Paints `view`'s drawing into `rect`: filled regions (faces, poche,
/// shadows), then the lines by weight, then the labels.
fn paint_vector(
    painter: &egui::Painter,
    rect: egui::Rect,
    view: &VectorView,
    technique: RenderingTechnique,
) {
    let (Some(d), Some(xf)) = (view.drawing.as_deref(), view.transform(rect)) else {
        return;
    };
    let tech = technique == RenderingTechnique::TechnicalIllustration;
    let painter = painter.with_clip_rect(rect);
    let mut mesh = egui::Mesh::default();
    for kind in [RegionKind::Face, RegionKind::Cut, RegionKind::Shadow] {
        for r in view.regions.iter().filter(|r| r.kind == kind) {
            let color = match kind {
                RegionKind::Face if tech => color32(r.material.color()),
                RegionKind::Face => egui::Color32::WHITE,
                RegionKind::Cut if tech => egui::Color32::from_gray(0x6E),
                RegionKind::Cut => egui::Color32::from_gray(0x8C),
                RegionKind::Shadow => egui::Color32::from_black_alpha(80),
            };
            for q in &r.quads {
                let base = mesh.vertices.len() as u32;
                for p in q {
                    mesh.colored_vertex(xf.to_screen(*p), color);
                }
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base, base + 2, base + 3);
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
    let view_rect = rect.expand(4.0);
    for l in &d.lines {
        let (a, b) = (xf.to_screen(l.a), xf.to_screen(l.b));
        if !view_rect.intersects(egui::Rect::from_two_pos(a, b)) {
            continue;
        }
        let (width, color) = line_style(l.kind, l.weight, technique);
        let stroke = egui::Stroke::new(width, color);
        if l.is_dashed() {
            painter.extend(egui::Shape::dashed_line(&[a, b], stroke, 5.0, 3.0));
        } else {
            painter.line_segment([a, b], stroke);
        }
    }
    // Lines with their own colour: Cross Section Lines, Clip Lines and the
    // Below Grade colour override.
    for l in &d.styled {
        let (a, b) = (xf.to_screen(l.a), xf.to_screen(l.b));
        if !view_rect.intersects(egui::Rect::from_two_pos(a, b)) {
            continue;
        }
        let stroke = egui::Stroke::new(
            l.width as f32,
            egui::Color32::from_rgb(l.color[0], l.color[1], l.color[2]),
        );
        if l.dashed {
            painter.extend(egui::Shape::dashed_line(&[a, b], stroke, 5.0, 3.0));
        } else {
            painter.line_segment([a, b], stroke);
        }
    }
    let size = ((DRAWING_TEXT_IN * xf.scale) as f32).clamp(7.0, 26.0);
    for (at, text) in &d.texts {
        painter.text(
            xf.to_screen(*at),
            egui::Align2::LEFT_BOTTOM,
            text,
            egui::FontId::proportional(size),
            egui::Color32::from_gray(0x20),
        );
    }
}

// ----- walkthroughs -----

/// The walkthrough being played in the 3D view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkPlay {
    pub camera: Id,
    /// Seconds into the walk.
    pub t_s: f64,
    pub playing: bool,
}

/// Scene-space eye position and viewport yaw of walkthrough camera `c`,
/// `t_s` seconds into the walk.
pub fn walk_view(project: &Project, c: &CameraObject, t_s: f64) -> ([f32; 3], f32) {
    let pose = c.walk_pose_at_time(t_s);
    let elevation = project.floors.get(c.floor).map_or(0.0, |f| f.elevation);
    (
        [
            pose.position.x as f32,
            (elevation + pose.eye_height) as f32,
            -pose.position.y as f32,
        ],
        pose.direction_deg.to_radians() as f32 - FRAC_PI_2,
    )
}

/// The up/down tilt of walkthrough camera `c`, `t_s` seconds into the walk,
/// degrees.
pub fn walk_tilt(c: &CameraObject, t_s: f64) -> f32 {
    c.walk_pose_at_time(t_s).tilt_deg as f32
}

/// The path tracer's settings for a camera's recording settings.
pub fn record_settings_for(w: &plan_core::camera_view::WalkRecord) -> plan_render::RenderSettings {
    let w = w.clamped();
    plan_render::RenderSettings {
        width: w.width,
        height: w.height,
        samples: w.samples,
        denoise: true,
        ..plan_render::RenderSettings::default()
    }
}

/// The frames of a walkthrough: `max(1, duration * fps)` poses spread evenly
/// from the first node to the last.
pub fn walk_frame_count(c: &CameraObject, fps: f64) -> usize {
    ((c.walk_duration_s() * fps).round() as usize).max(1)
}

/// Renders walkthrough `cam` as numbered PNG frames (`frame_0001.png`, ...) in
/// `out_dir` with the path tracer (`plan_view3d` has no offscreen GL target),
/// plus the `make_video.sh` ffmpeg script. `progress(done, total)` returns
/// `false` to stop early; the number of frames written is returned. The movie
/// and the technique looks are [`record::record`].
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn record_walkthrough(
    scene: &Scene,
    cam: &CameraObject,
    elevation: f64,
    lights: &[plan_render::PointLight],
    env: &plan_render::Environment,
    settings: &plan_render::RenderSettings,
    fps: f64,
    out_dir: &std::path::Path,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> std::io::Result<usize> {
    let job = record::RecordJob {
        scene,
        cam,
        elevation,
        lights,
        env,
        settings,
        walk: plan_core::camera_view::WalkRecord {
            fps,
            format: plan_core::camera_view::RecordFormat::Frames,
            ..plan_core::camera_view::WalkRecord::default()
        },
        style: None,
    };
    Ok(record::record(&job, out_dir, progress)?.frames)
}

/// A walkthrough being recorded on a worker thread.
pub struct Recording {
    done: Arc<AtomicUsize>,
    total: usize,
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Result<usize, String>>>>,
    pub dir: PathBuf,
    /// The movie being written, when the format asks for one.
    pub video: Option<PathBuf>,
}

impl Recording {
    /// Starts recording `cam` into `dir` in the camera's own technique.
    #[cfg(test)]
    pub fn start(
        project: &Project,
        cam: &CameraObject,
        sun: Option<plan_render::Sun>,
        dir: PathBuf,
    ) -> Self {
        let technique = view_settings::technique_of(cam).unwrap_or(RenderingTechnique::Standard);
        Self::start_in(project, cam, sun, dir, technique)
    }

    /// Starts recording `cam` into `dir` in `technique` (the path tracer's
    /// Clay or Physically Based, with the Vector View, Technical
    /// Illustration, Line Drawing or Watercolor look applied on top).
    pub fn start_in(
        project: &Project,
        cam: &CameraObject,
        sun: Option<plan_render::Sun>,
        dir: PathBuf,
        technique: RenderingTechnique,
    ) -> Self {
        let scope = view_settings::scope_of(project, cam);
        let (slider, hide_from) = view_settings::extra_scope_of(cam);
        let scene = build_view_scene(
            project,
            &ViewScope {
                floor: scope.floor,
                clip_above: scope.clip_above,
                clip_below: scope.clip_below,
                slider,
                hide_from,
                gray: !cam.view.options.show_color,
                ..ViewScope::default()
            },
        );
        let elevation = project.floors.get(cam.floor).map_or(0.0, |f| f.elevation);
        let lights =
            crate::dialogs::camera::render_lights_in(project, cam.view.light_set.as_deref());
        let (render_technique, style) = view_settings::render_look(technique);
        let cam = cam.clone();
        let walk = cam.view.walk.clamped();
        let video = walk
            .format
            .video()
            .then(|| record::video_path(&dir, &cam.name));
        let total = walk_frame_count(&cam, walk.fps);
        let done = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let result = Arc::new(Mutex::new(None));
        let (d2, c2, r2, out) = (
            Arc::clone(&done),
            Arc::clone(&cancel),
            Arc::clone(&result),
            dir.clone(),
        );
        let spawned = std::thread::Builder::new()
            .name("record-walkthrough".into())
            .spawn(move || {
                let env = plan_render::Environment {
                    sun,
                    ..plan_render::Environment::default()
                };
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let job = record::RecordJob {
                        scene: &scene,
                        cam: &cam,
                        elevation,
                        lights: &lights,
                        env: &env,
                        settings: &plan_render::RenderSettings {
                            technique: render_technique,
                            ..record_settings_for(&walk)
                        },
                        walk,
                        style,
                    };
                    record::record(&job, &out, &mut |n, _| {
                        d2.store(n, Ordering::Relaxed);
                        !c2.load(Ordering::Relaxed)
                    })
                    .map(|r| r.frames)
                    .map_err(|e| e.to_string())
                }))
                .unwrap_or_else(|_| Err("The renderer stopped unexpectedly".into()));
                *r2.lock().unwrap_or_else(PoisonError::into_inner) = Some(res);
            });
        if let Err(e) = spawned {
            *result.lock().unwrap_or_else(PoisonError::into_inner) =
                Some(Err(format!("Could not start the renderer: {e}")));
        }
        Self {
            done,
            total,
            cancel,
            result,
            dir,
            video,
        }
    }

    pub fn progress(&self) -> (usize, usize) {
        (self.done.load(Ordering::Relaxed), self.total)
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The outcome once the worker has finished.
    pub fn finished(&self) -> Option<Result<usize, String>> {
        self.result
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

// ----- state -----

/// How the viewport is to be aimed once the scene is loaded.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Setup {
    Mode(CameraMode),
    /// An orbit-like overview drawn in parallel projection (C-15); an
    /// isometric view also fixes the yaw and pitch.
    Parallel {
        mode: CameraMode,
        iso: Option<(f32, f32)>,
    },
    Camera {
        position: Point,
        direction_deg: f64,
        /// Absolute eye height in scene Y, inches.
        eye_y: f64,
        eye_height: f64,
        fov_deg: f64,
        /// Up/down tilt, degrees.
        tilt_deg: f64,
    },
    /// An overview aimed from a saved eye at a saved target (scene space).
    Pose {
        mode: CameraMode,
        eye: [f32; 3],
        target: [f32; 3],
    },
    Section(SectionCut),
    /// A saved orthographic view.
    Ortho(plan_core::camera_view::OrthoView),
}

/// The cross section slider (C-23).
#[derive(Clone, Copy, Debug)]
struct Slider {
    /// The cut at offset zero.
    base: SectionCut,
    depth: f64,
    offset: f64,
}

/// A pointer event in drawing space for the annotation tools of a vector view.
#[derive(Clone, Copy, Debug)]
pub struct ViewPointer {
    pub kind: ViewPtr,
    pub at: Point,
    pub down: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewPtr {
    Move,
    Down,
    Up,
    Double,
}

pub struct View3dState {
    /// The vector view is annotated: the primary button drives the tools, the
    /// middle button pans (R17-01).
    pub annotate: bool,
    /// Pointer events the vector view collected for the tools this frame.
    pub annot_events: Vec<ViewPointer>,
    /// A 3D view replaces the plan canvas.
    pub active: bool,
    pub viewport: Option<Viewport3d>,
    pub mode: CameraMode,
    pub technique: RenderingTechnique,
    /// The toolbar's Textures toggle: paint materials and pictures with their
    /// bitmaps where the technique shows them ([`technique_shows_textures`]).
    pub textures_on: bool,
    /// Forces a rebuild even when nothing changed (Rebuild 3D).
    pub scene_dirty: bool,
    /// Signature of the model the viewport shows (see [`View3dState::signature`]).
    pub last_project_hash: u64,
    /// Floor Overview: only this floor and below are built.
    pub scope_floor: Option<usize>,
    pub section: Option<SectionCut>,
    /// The camera object the view shows, if any.
    pub active_camera: Option<Id>,
    pub raytrace: RayTraceDialog,
    show_defaults: bool,
    setup: Option<Setup>,
    inbox: Outbox,
    camera_dialog: Option<CameraDialog>,
    extras: HashMap<Id, CameraExtras>,
    slider: Option<Slider>,
    plan_bounds: Option<(Point, Point)>,
    scene_empty: bool,
    sun_light: Option<[f32; 3]>,
    /// The vector elevation on screen (Vector View, Technical Illustration).
    pub vector: VectorView,
    /// The walkthrough being played, if any.
    pub walk: Option<WalkPlay>,
    recording: Option<Recording>,
    adjust_lights: Option<AdjustLightsDialog>,
    /// 3D > Lighting, while it is open.
    lighting: Option<LightingDialog>,
    /// Record Walkthrough, while it is open.
    record_dialog: Option<RecordDialog>,
    /// Preview or Final View ([`View3dState::set_quality`]).
    pub quality: ViewQuality,
    /// Quality presets apply when this changes (the Shading menu may then
    /// adjust the toggles).
    quality_applied: Option<ViewQuality>,
    /// Nothing above this height (a Floor Camera's ceiling), inches.
    pub clip_above: Option<f64>,
    /// Nothing centred below this height (the lowest floor a camera picks).
    pub clip_below: Option<f64>,
    /// The planes of the Cross Section Slider of the view (C-141): the active
    /// camera's, loaded when it is shown, or a live overview's own.
    pub cam_slider: plan_core::camera_view::CrossSectionSlider,
    /// Hide Camera-Facing Exterior Walls looks from this plan point (C-151).
    pub hide_from: Option<Point>,
    /// Show Color is off for the camera on show (C-146).
    pub gray: bool,
    /// The Cross Section Slider dialog is open.
    slider_dialog: bool,
    slider_fields: crate::dialogs::Fields,
    /// The model's extent (plan X, plan Y, height) before any cut, inches.
    model_bounds: Option<([f64; 3], [f64; 3])>,
    /// Save Camera was asked for by a request; done at the next frame.
    save_requested: bool,
    /// The backdrop picture last loaded, by file name.
    backdrop_cache: Option<(String, Option<Arc<plan_view3d::BackdropImage>>)>,
    /// Width over height of the viewport last drawn.
    aspect: f32,
    /// The cached scene as built (no pictures, no selection tint): what clicks
    /// ray-cast and what the overlay is added to.
    base_scene: Scene,
    /// The overlay (pictures, selection and hover tints) the viewport holds,
    /// by [`pick::overlay_key_with_hover`].
    overlay_key: u64,
    /// Is the Select tool the active tool? Only then does a drag on a
    /// selected object move it (the shell sets this before each frame).
    pub select_tool: bool,
    /// A press on a selected object that may become a move ([`drag`]).
    obj_drag: Option<drag::ObjDrag>,
    /// The object under the pointer (its mesh id) and when it was picked.
    hover: Hover,
    /// The path-traced Final View over the live view.
    pub final_view: final_view::FinalView,
    /// Undo Zoom: the camera before each zoom, pan or move.
    pub zoom: zoom_history::ZoomHistory,
    /// Export 360 Panorama, while it is open.
    panorama_dialog: Option<crate::dialogs::panorama::PanoramaDialog>,
    /// A panorama being rendered.
    panorama_job: Option<crate::dialogs::panorama::PanoramaJob>,
}

/// The hover highlight's state: the last pick, throttled by
/// [`pick::hover_due`].
#[derive(Default)]
struct Hover {
    object: Option<Id>,
    last: Option<(egui::Pos2, f64)>,
    /// How many ray casts the hover has made (tests watch the throttle).
    picks: u32,
}

impl Default for View3dState {
    fn default() -> Self {
        Self::with_inbox(Outbox::global())
    }
}

impl View3dState {
    /// A state that reads its requests from `inbox` (tests use their own).
    pub fn with_inbox(inbox: Outbox) -> Self {
        Self {
            active: false,
            viewport: None,
            mode: CameraMode::Orbit,
            technique: RenderingTechnique::Standard,
            textures_on: true,
            scene_dirty: true,
            last_project_hash: 0,
            scope_floor: None,
            section: None,
            annotate: false,
            annot_events: Vec::new(),
            active_camera: None,
            raytrace: RayTraceDialog::default(),
            show_defaults: false,
            setup: None,
            inbox,
            camera_dialog: None,
            extras: HashMap::new(),
            slider: None,
            plan_bounds: None,
            scene_empty: true,
            sun_light: None,
            vector: VectorView::default(),
            walk: None,
            recording: None,
            adjust_lights: None,
            lighting: None,
            record_dialog: None,
            quality: ViewQuality::Final,
            quality_applied: None,
            clip_above: None,
            clip_below: None,
            cam_slider: plan_core::camera_view::CrossSectionSlider::default(),
            hide_from: None,
            gray: false,
            slider_dialog: false,
            slider_fields: crate::dialogs::Fields::default(),
            model_bounds: None,
            save_requested: false,
            backdrop_cache: None,
            aspect: 1.5,
            base_scene: Scene::default(),
            overlay_key: pick::empty_overlay_key(),
            select_tool: true,
            obj_drag: None,
            hover: Hover::default(),
            final_view: final_view::FinalView::default(),
            zoom: zoom_history::ZoomHistory::default(),
            panorama_dialog: None,
            panorama_job: None,
        }
    }

    // ----- requests -----

    /// Applies the queued [`ViewRequest`]s.
    pub fn drain(&mut self, project: &Project) {
        for req in self.inbox.take() {
            match req {
                ViewRequest::Mode { mode, floor } => self.open_mode(mode, floor),
                ViewRequest::ShowCamera(id) => self.show_camera(project, id),
                ViewRequest::RefreshCamera(id) => {
                    if self.active && self.active_camera == Some(id) {
                        self.show_camera(project, id);
                    }
                }
                ViewRequest::CameraDeleted(id) => {
                    self.extras.remove(&id);
                    if self.active_camera == Some(id) {
                        self.active_camera = None;
                    }
                    if self.walk.is_some_and(|w| w.camera == id) {
                        self.walk = None;
                    }
                }
                ViewRequest::OpenAdjustLights(focus) => self.open_adjust_lights(project, focus),
                ViewRequest::SaveCamera => self.save_requested = true,
                ViewRequest::GlassHouse => self.open_glass_house(),
                ViewRequest::OpenLighting => self.open_lighting(project),
                ViewRequest::OpenCameraSpec(id) => {
                    if let Some(c) = project.camera(id) {
                        let floor = project
                            .floors
                            .get(c.floor)
                            .map_or("", |f| f.name.as_str())
                            .to_string();
                        let extras = self.extras.get(&id).copied().unwrap_or(CameraExtras {
                            technique: self.technique,
                        });
                        self.camera_dialog = Some(
                            CameraDialog::new(c, &floor, extras)
                                .with_plan_lighting(project.lighting.clone())
                                .with_floor_names(
                                    project.floors.iter().map(|f| f.name.clone()).collect(),
                                )
                                .with_layer_names(
                                    project
                                        .layers
                                        .layers
                                        .iter()
                                        .map(|l| l.name.clone())
                                        .collect(),
                                ),
                        );
                    }
                }
            }
        }
    }

    /// Opens an overview, framing the whole plan.
    pub fn open_mode(&mut self, mode: CameraMode, floor: Option<usize>) {
        self.active = true;
        self.mode = mode;
        self.scope_floor = floor;
        self.section = None;
        self.slider = None;
        self.active_camera = None;
        self.clip_above = None;
        self.clip_below = None;
        self.hide_from = None;
        self.gray = false;
        self.walk = None;
        self.zoom.clear();
        self.final_view.clear();
        self.setup = Some(Setup::Mode(mode));
    }

    /// Opens an overview in parallel projection (C-15): the Orthographic
    /// Full, Floor or Framing Overview, or an Isometric View.
    pub fn open_parallel(&mut self, kind: ParallelOverview, floor: usize) {
        let floor = (kind == ParallelOverview::Floor).then_some(floor);
        self.open_mode(CameraMode::Orbit, floor);
        let iso = match kind {
            ParallelOverview::Isometric(c) => Some(isometric_angles(c)),
            _ => None,
        };
        self.setup = Some(Setup::Parallel {
            mode: CameraMode::Orbit,
            iso,
        });
    }

    /// Opens the Glass House overview: the Perspective Overview drawn with
    /// the Glass House technique.
    pub fn open_glass_house(&mut self) {
        self.open_mode(CameraMode::Orbit, None);
        self.technique = RenderingTechnique::GlassHouse;
    }

    /// Opens the 3D view of a camera object (C-4, C-17): the camera's saved
    /// technique, quality and floors come with it.
    pub fn show_camera(&mut self, project: &Project, id: Id) {
        let Some(c) = project.camera(id) else { return };
        let elevation = project.floors.get(c.floor).map_or(0.0, |f| f.elevation);
        if let Some(t) = view_settings::technique_of(c) {
            self.technique = t;
        } else if let Some(e) = self.extras.get(&id) {
            self.technique = e.technique;
        }
        self.slider = None;
        let scope = view_settings::scope_of(project, c);
        self.scope_floor = scope.floor;
        self.clip_above = scope.clip_above;
        self.clip_below = scope.clip_below;
        self.cam_slider = c.view.slider.clone();
        self.hide_from = view_settings::hide_facing_from(c);
        self.gray = !c.view.options.show_color;
        // Depth of Field of the camera sets the Ray Trace window's lens.
        if let Some((aperture, focus)) = view_settings::dof_lens(&c.view) {
            self.raytrace.aperture_in = aperture;
            self.raytrace.focus_in = focus;
        }
        self.section = None;
        if c.kind != CameraKind::Walkthrough {
            self.walk = None;
        }
        self.quality = c.view.quality;
        self.quality_applied = None;
        let saved_pose = c.overview_pose().map(|p| Setup::Pose {
            mode: if c.kind == CameraKind::DollHouse {
                CameraMode::DollHouse
            } else {
                CameraMode::Orbit
            },
            eye: p.eye.map(|v| v as f32),
            target: p.target.map(|v| v as f32),
        });
        match c.kind {
            CameraKind::Walkthrough => {
                let pose = c.walk_pose(0.0);
                self.mode = CameraMode::FullCamera;
                self.setup = Some(Setup::Camera {
                    position: pose.position,
                    direction_deg: pose.direction_deg,
                    eye_y: elevation + pose.eye_height,
                    eye_height: pose.eye_height,
                    fov_deg: c.fov_deg,
                    tilt_deg: pose.tilt_deg,
                });
                if !self.walk.is_some_and(|w| w.camera == id) {
                    self.walk = Some(WalkPlay {
                        camera: id,
                        t_s: 0.0,
                        playing: false,
                    });
                }
            }
            CameraKind::FullCamera | CameraKind::FloorCamera => {
                self.mode = CameraMode::FullCamera;
                self.setup = Some(Setup::Camera {
                    position: c.position,
                    direction_deg: c.direction_deg,
                    eye_y: elevation + c.eye_height,
                    eye_height: c.eye_height,
                    fov_deg: c.fov_deg,
                    tilt_deg: c.view.tilt_deg,
                });
            }
            CameraKind::Orthographic if c.view.ortho.is_some() => {
                let view = c.view.ortho.expect("checked");
                self.mode = extras::ortho_mode(view.kind);
                self.setup = Some(Setup::Ortho(view));
            }
            CameraKind::PerspectiveOverview
            | CameraKind::Orthographic
            | CameraKind::GlassHouse
            | CameraKind::FramingOverview => {
                self.mode = CameraMode::Orbit;
                self.setup = Some(saved_pose.unwrap_or(Setup::Mode(CameraMode::Orbit)));
            }
            CameraKind::DollHouse => {
                self.mode = CameraMode::DollHouse;
                self.setup = Some(saved_pose.unwrap_or(Setup::Mode(CameraMode::DollHouse)));
            }
            CameraKind::CrossSection { .. } | CameraKind::WallElevation | CameraKind::Elevation => {
                let cut = SectionCut::from_camera(c);
                self.mode = cut.elevation_mode();
                self.section = Some(cut);
                self.setup = Some(Setup::Section(cut));
            }
        }
        self.active = true;
        self.active_camera = Some(id);
        self.zoom.clear();
        self.final_view.clear();
    }

    /// The open Lighting dialog, for the scenario tests that edit it.
    #[cfg(test)]
    pub fn lighting_dialog_mut(&mut self) -> Option<&mut LightingDialog> {
        self.lighting.as_mut()
    }

    /// The open Adjust Lights dialog, for the scenario tests.
    #[cfg(test)]
    pub fn adjust_lights_mut(&mut self) -> Option<&mut AdjustLightsDialog> {
        self.adjust_lights.as_mut()
    }

    /// The open Record Walkthrough dialog, for the scenario tests.
    #[cfg(test)]
    pub fn record_dialog_mut(&mut self) -> Option<&mut RecordDialog> {
        self.record_dialog.as_mut()
    }

    /// The open Export 360 Panorama dialog, for the scenario tests.
    #[cfg(test)]
    pub fn panorama_dialog_mut(&mut self) -> Option<&mut crate::dialogs::panorama::PanoramaDialog> {
        self.panorama_dialog.as_mut()
    }

    /// Is a panorama being rendered?
    #[cfg(test)]
    pub fn panorama_running(&self) -> bool {
        self.panorama_job.is_some()
    }

    /// The open Camera Specification, for the scenario tests that edit it.
    #[cfg(test)]
    pub fn camera_dialog_mut(&mut self) -> Option<&mut CameraDialog> {
        self.camera_dialog.as_mut()
    }

    /// One camera step from the 3D menu (Move, Orbit, Tilt, View Direction).
    /// With no 3D view up it opens the Perspective Full Overview instead; the
    /// orthographic views, whose direction is fixed, say so.
    pub fn nudge_camera(&mut self, cx: &mut EditorContext, n: nudge::Nudge) {
        let Some(vp) = self.viewport.as_mut().filter(|_| self.active) else {
            self.open_mode(CameraMode::Orbit, None);
            cx.status = "Opened the 3D view; use the camera command again".into();
            return;
        };
        let steps = self
            .active_camera
            .and_then(|id| cx.project.camera(id))
            .map_or_else(nudge::Steps::default, |c| nudge::Steps::of(&c.view));
        if !nudge::apply_with(&mut vp.camera, n, steps) {
            cx.status = "This view looks in a fixed direction; switch to a perspective view".into();
        }
    }

    /// Picks a view from the overlay combo box; leaves any section.
    pub fn set_mode_user(&mut self, mode: CameraMode) {
        self.mode = mode;
        self.section = None;
        self.slider = None;
        self.active_camera = None;
        self.clip_above = None;
        self.clip_below = None;
        self.hide_from = None;
        self.gray = false;
        self.walk = None;
        if let Some(vp) = &mut self.viewport {
            vp.set_mode(mode);
            self.zoom.reset_to(&vp.camera);
        }
    }

    pub fn set_technique(&mut self, t: RenderingTechnique) {
        self.technique = t;
    }

    /// Rebuild 3D (C-54): throws the scene away and builds it again.
    pub fn rebuild(&mut self) {
        self.scene_dirty = true;
    }

    /// Cross Section Slider (C-23): sweeps a cut plane through the model along
    /// the current elevation's viewing direction.
    pub fn toggle_slider(&mut self) {
        if self.slider.take().is_some() {
            self.section = None;
            return;
        }
        let Some((lo, hi)) = self.plan_bounds else {
            self.open_mode(CameraMode::ElevationFront, None);
            return;
        };
        let mode = if matches!(
            self.mode,
            CameraMode::ElevationFront
                | CameraMode::ElevationBack
                | CameraMode::ElevationLeft
                | CameraMode::ElevationRight
        ) {
            self.mode
        } else {
            CameraMode::ElevationFront
        };
        let dir = mode_direction(mode);
        let centre = Point::lerp(lo, hi, 0.5);
        let extent = (hi - lo).dot(Point::new(dir.x.abs(), dir.y.abs()));
        let diag = lo.dist(hi);
        let base = SectionCut {
            origin: centre - dir * (extent * 0.5 + SECTION_TOLERANCE),
            dir,
            half_width: diag,
            back_clip: None,
        };
        self.slider = Some(Slider {
            base,
            depth: extent + 2.0 * SECTION_TOLERANCE,
            offset: 0.0,
        });
        self.active = true;
        self.mode = mode;
        self.active_camera = None;
        self.section = Some(base);
        self.setup = Some(Setup::Section(base));
    }

    // ----- vector elevations -----

    /// The camera whose vector elevation the panel draws instead of the GL
    /// scene: an elevation, section or wall elevation camera while the
    /// technique is Vector View or Technical Illustration.
    pub fn vector_camera<'a>(&self, project: &'a Project) -> Option<&'a CameraObject> {
        if !self.active || !is_vector_technique(self.technique) {
            return None;
        }
        project
            .camera(self.active_camera?)
            .filter(|c| is_elevation_camera(c))
    }

    /// The Cross Section Slider as a camera (C-23): while the slider is on in
    /// a vector technique, the cut at its current distance is drawn as a
    /// cross section, so the vector view follows the slider live.
    fn slider_camera(&self) -> Option<CameraObject> {
        if !self.active || !is_vector_technique(self.technique) || self.slider.is_none() {
            return None;
        }
        let cut = self.section?;
        let half = cut.tangent() * cut.half_width;
        let mut cam = CameraObject::new(
            CameraKind::CrossSection {
                back_clip: cut.back_clip,
            },
            cut.origin,
            cut.dir.angle().to_degrees(),
            "Cross Section Slider",
            0,
        );
        cam.section = Some(plan_core::extras::SectionLine {
            a: cut.origin - half,
            b: cut.origin + half,
            back_clip: cut.back_clip,
        });
        Some(cam)
    }

    /// The camera the vector drawing is made for: the active elevation or
    /// section camera, else the Cross Section Slider's cut.
    pub fn vector_source(&self, project: &Project) -> Option<CameraObject> {
        self.vector_camera(project)
            .cloned()
            .or_else(|| self.slider_camera())
    }

    /// Writes the vector drawing on screen as a DXF (lines by weight class).
    fn export_vector_dxf(&self, cx: &mut EditorContext) {
        let name = self
            .active_camera
            .and_then(|id| cx.project.camera(id))
            .map_or_else(|| "Cross Section".to_string(), |c| c.name.clone());
        cx.status = match self.vector.drawing() {
            Some(d) => crate::dialogs::camera::save_drawing_dxf(d, &name),
            None => "Nothing to export yet".into(),
        };
    }

    /// What the vector drawing depends on: the project hash, the camera, the
    /// technique, the sun and the raster size.
    fn vector_key(&self, project: &Project, cam: &CameraObject, sun: Option<SunDir>) -> u64 {
        let mut h = DefaultHasher::new();
        project_hash(project).hash(&mut h);
        let _ = write!(HashFmt(&mut h), "{cam:?}");
        self.technique.label().hash(&mut h);
        self.vector.raster_px.hash(&mut h);
        match sun {
            Some(s) => {
                1_u8.hash(&mut h);
                s.azimuth_deg.to_bits().hash(&mut h);
                s.altitude_deg.to_bits().hash(&mut h);
            }
            None => 0_u8.hash(&mut h),
        }
        h.finish()
    }

    /// The Sun Angle's shadow direction, while that toggle is on. The compass
    /// azimuth is turned by the plan's North Pointer so shadows fall the way
    /// the real sun casts them.
    pub fn plan_sun(&self, project: &Project, sun_angle_on: bool) -> Option<SunDir> {
        sun_angle_on
            .then(|| sun_dir(&self.raytrace.sun()))
            .flatten()
            .map(|s| SunDir {
                azimuth_deg: crate::editor::site_view::plan_sun_azimuth(project, s.azimuth_deg),
                ..s
            })
    }

    /// Starts a new vector drawing when the project, the camera, the
    /// technique or the sun changed since the one on screen ("Refresh").
    pub fn refresh_vector(&mut self, project: &Project, sun: Option<SunDir>) {
        let Some(cam) = self.vector_source(project) else {
            return;
        };
        let key = self.vector_key(project, &cam, sun);
        if !self.vector.needs(key) {
            return;
        }
        let (project, technique, raster) = (project.clone(), self.technique, self.vector.raster_px);
        self.vector.start(key, move || {
            vector_drawing(&project, &cam, technique, sun, raster)
        });
    }

    /// [`refresh_vector`](Self::refresh_vector) on this thread: the drawing is
    /// on screen when it returns.
    #[cfg(test)]
    pub fn refresh_vector_now(&mut self, project: &Project, sun: Option<SunDir>) {
        let Some(cam) = self.vector_source(project) else {
            return;
        };
        let key = self.vector_key(project, &cam, sun);
        if self.vector.key == key && self.vector.drawing.is_some() {
            return;
        }
        let d = vector_drawing(project, &cam, self.technique, sun, self.vector.raster_px);
        self.vector.pending = None;
        self.vector.install(key, d);
    }

    // ----- walkthroughs -----

    fn walk_camera_id(&self, project: &Project) -> Option<Id> {
        let is_walk = |id: &Id| {
            project
                .camera(*id)
                .is_some_and(|c| c.kind == CameraKind::Walkthrough)
        };
        self.active_camera.filter(is_walk).or_else(|| {
            project
                .cameras
                .iter()
                .find(|c| c.kind == CameraKind::Walkthrough)
                .map(|c| c.id)
        })
    }

    /// Plays the walkthrough (the active one, else the first) from the start.
    pub fn play_walkthrough(&mut self, cx: &mut EditorContext) {
        let Some(id) = self.walk_camera_id(&cx.project) else {
            cx.status = "No walkthrough yet: draw one with Create Walkthrough Path".into();
            return;
        };
        self.show_camera(&cx.project, id);
        self.walk = Some(WalkPlay {
            camera: id,
            t_s: 0.0,
            playing: true,
        });
    }

    /// Moves the playing walkthrough on by `dt` seconds and returns the
    /// viewport pose to apply (`None` when none is shown).
    pub fn tick_walk(&mut self, project: &Project, dt: f64) -> Option<([f32; 3], f32)> {
        let w = self.walk.as_mut()?;
        let c = project.camera(w.camera)?;
        if w.playing {
            w.t_s += dt;
            let end = c.walk_duration_s();
            if w.t_s >= end {
                w.t_s = end;
                w.playing = false;
            }
        }
        Some(walk_view(project, c, w.t_s))
    }

    /// Record Walkthrough: opens the dialog (frame rate, picture size, folder);
    /// its Record button renders the frames ([`View3dState::record_window`]).
    pub fn record_walkthrough_dialog(&mut self, cx: &mut EditorContext) {
        let Some(id) = self.walk_camera_id(&cx.project) else {
            cx.status = "No walkthrough yet: draw one with Create Walkthrough Path".into();
            return;
        };
        if self.recording.is_some() {
            cx.status = "A walkthrough is already being recorded".into();
            return;
        }
        let Some(cam) = cx.project.camera(id) else {
            return;
        };
        self.record_dialog = Some(RecordDialog::new(cam, default_record_folder(&cam.name)));
    }

    /// The Record Walkthrough dialog: OK saves the settings on the camera
    /// (one undo step) and starts the recording.
    fn record_window(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        let Some(mut d) = self.record_dialog.take() else {
            return;
        };
        match d.show(ctx) {
            RecordOutcome::Open => self.record_dialog = Some(d),
            RecordOutcome::Cancel => {}
            RecordOutcome::Record => {
                let (id, settings) = (d.camera, d.clamped());
                if cx
                    .project
                    .camera(id)
                    .is_some_and(|c| c.view.walk != settings)
                {
                    cx.begin_change("Walkthrough Recording Settings");
                    cx.project.update_camera(id, |c| c.view.walk = settings);
                    cx.mark_dirty();
                }
                self.start_recording(cx, id, PathBuf::from(d.folder.trim()));
            }
        }
    }

    /// Starts recording walkthrough camera `id` into `dir`.
    pub fn start_recording(&mut self, cx: &mut EditorContext, id: Id, dir: PathBuf) {
        let Some(cam) = cx.project.camera(id).cloned() else {
            return;
        };
        // The plan's sun (3D > Lighting), as the Final View and the panorama use.
        let sun = final_view::environment_for(&cx.project.lighting, Some(&cam.view)).sun;
        let technique = view_settings::technique_of(&cam).unwrap_or(self.technique);
        let rec = Recording::start_in(&cx.project, &cam, sun, dir, technique);
        cx.status = format!(
            "Recording {} frames to {}",
            rec.progress().1,
            rec.video
                .as_ref()
                .map_or(rec.dir.clone(), Clone::clone)
                .display()
        );
        self.recording = Some(rec);
    }

    // ----- saved cameras, lighting, quality -----

    /// Save Camera: keeps the view on screen as a camera in the plan and the
    /// Project Browser. A Full Camera view becomes a Full Camera at the eye;
    /// an overview or Doll House view keeps its eye and target so Restore
    /// brings the exact view back. One undo step.
    pub fn save_camera(&mut self, cx: &mut EditorContext) {
        let Some(vp) = self.viewport.as_ref().filter(|_| self.active) else {
            cx.status = "Open a 3D view, then use Save Camera".into();
            return;
        };
        let cam = &vp.camera;
        let floor = self.scope_floor.unwrap_or(cx.floor);
        let elevation = cx.project.floors.get(floor).map_or(0.0, |f| f.elevation);
        let mut name_n = cx.project.cameras.len() + 1;
        let name = loop {
            let n = format!("Saved Camera {name_n}");
            if !cx.project.cameras.iter().any(|c| c.name == n) {
                break n;
            }
            name_n += 1;
        };
        let mut obj = match cam.mode {
            CameraMode::FullCamera => {
                let at = Point::new(f64::from(cam.position[0]), -f64::from(cam.position[2]));
                let deg = (f64::from(cam.yaw) + std::f64::consts::FRAC_PI_2)
                    .to_degrees()
                    .rem_euclid(360.0);
                let mut o = CameraObject::new(CameraKind::FullCamera, at, deg, name, floor);
                o.eye_height = (f64::from(cam.position[1]) - elevation).clamp(12.0, 600.0);
                o.fov_deg = horizontal_fov(cam.fov_deg, self.aspect);
                o.view.tilt_deg = f64::from(cam.tilt_deg());
                o
            }
            CameraMode::Orbit | CameraMode::DollHouse => {
                let (eye, target) = cam.pose();
                let kind = if cam.mode == CameraMode::DollHouse {
                    CameraKind::DollHouse
                } else if self.technique == RenderingTechnique::GlassHouse {
                    CameraKind::GlassHouse
                } else {
                    CameraKind::PerspectiveOverview
                };
                // The overview's symbol stands at its eye and looks toward its
                // target, so it can be selected, moved, aimed and copied in the
                // plan (manual pp. 1153, 1157; DECISIONS 166).
                let at = Point::new(f64::from(eye[0]), -f64::from(eye[2]));
                let aim = Point::new(f64::from(target[0]), -f64::from(target[2])) - at;
                let (deg, len) = if aim.length() > 1.0 {
                    (aim.angle().to_degrees().rem_euclid(360.0), aim.length())
                } else {
                    (90.0, plan_core::camera::DEFAULT_CONE_LENGTH)
                };
                let mut o = CameraObject::new(kind, at, deg, name, floor);
                o.fov_deg = horizontal_fov(cam.fov_deg, self.aspect);
                o.clip_distance = Some(len);
                o.view.pose = Some(ViewPose {
                    eye: eye.map(f64::from),
                    target: target.map(f64::from),
                    symbol: true,
                });
                o
            }
            _ => {
                // An elevation or the plan overhead: keep its direction, centre
                // and zoom so Restore brings the same view back.
                let Some(view) = extras::ortho_view_of(cam) else {
                    cx.status = "Save Camera needs a 3D view".into();
                    return;
                };
                let at = Point::new(f64::from(cam.target[0]), -f64::from(cam.target[2]));
                let mut o = CameraObject::new(CameraKind::Orthographic, at, 90.0, name, floor);
                o.view.show_in_plan = false;
                o.view.ortho = Some(view);
                o
            }
        };
        obj.view.technique = Some(self.technique.label().to_string());
        obj.view.quality = self.quality;
        if self.scope_floor.is_some() {
            obj.view.floors = plan_core::camera_view::FloorsDisplayed::ThisAndBelow;
        }
        cx.begin_change("Save Camera");
        if cx.project.layers.get(camera_tool::CAMERA_LAYER).is_none() {
            cx.project.layers.add(plan_core::Layer::new(
                camera_tool::CAMERA_LAYER,
                [0x2F, 0x6C, 0xB3],
                25,
            ));
        }
        let label = obj.name.clone();
        let id = cx.project.add_camera(obj);
        cx.mark_dirty();
        self.active_camera = Some(id);
        cx.status = format!("Saved {label} (Project Browser > Cameras restores it)");
    }

    /// Create Walkthrough Path from the selected CAD polyline or line: each
    /// vertex becomes a key frame node. One undo step.
    pub fn walk_from_cad(&mut self, cx: &mut EditorContext) {
        let cad_id = cx.selection.items.iter().find_map(|o| match o {
            crate::editor::ObjectRef::Cad(id) => Some(*id),
            _ => None,
        });
        let mut n = cx.project.cameras.len() + 1;
        let name = loop {
            let c = format!("Walkthrough {n}");
            if !cx.project.cameras.iter().any(|x| x.name == c) {
                break c;
            }
            n += 1;
        };
        let Some(cam) = cad_id.and_then(|id| {
            camera_tool::walkthrough_from_cad(
                &cx.project,
                cx.floor,
                id,
                camera_defaults().eye_height,
                &name,
            )
        }) else {
            cx.status =
                "Select a CAD polyline or line first, then use Walkthrough Path from CAD".into();
            return;
        };
        cx.begin_change("Create Walkthrough Path");
        if cx.project.layers.get(camera_tool::CAMERA_LAYER).is_none() {
            cx.project.layers.add(plan_core::Layer::new(
                camera_tool::CAMERA_LAYER,
                [0x2F, 0x6C, 0xB3],
                25,
            ));
        }
        let id = cx.project.add_camera(cam);
        cx.mark_dirty();
        cx.status = format!("Walkthrough path made from the CAD line ({name})");
        self.show_camera(&cx.project, id);
    }

    /// 3D > Lighting.
    pub fn open_lighting(&mut self, project: &Project) {
        self.lighting = Some(LightingDialog::new(project));
    }

    /// The Lighting dialog: OK writes the plan's lighting (one undo step).
    fn lighting_window(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        let Some(mut d) = self.lighting.take() else {
            return;
        };
        match d.show(ctx) {
            Outcome::Open => {
                if d.take_adjust_request() {
                    self.open_adjust_lights(&cx.project, None);
                }
                self.lighting = Some(d);
            }
            Outcome::Cancel => {}
            Outcome::Ok => {
                cx.begin_change("Lighting");
                d.apply(&mut cx.project);
                cx.mark_dirty();
                cx.status = "Lighting updated".into();
            }
        }
    }

    /// The picture of an Image backdrop, decoded once per file name.
    fn backdrop_for(
        &mut self,
        view: Option<&plan_core::camera_view::CameraView>,
    ) -> Option<Arc<plan_view3d::BackdropImage>> {
        use plan_core::camera_view::BackdropKind;
        let name = view
            .filter(|v| v.backdrop.kind == BackdropKind::Image)
            .map(|v| v.backdrop.image.clone());
        let Some(name) = name else {
            self.backdrop_cache = None;
            return None;
        };
        if let Some((cached, image)) = &self.backdrop_cache {
            if *cached == name {
                return image.clone();
            }
        }
        let image = backdrop::load(&name);
        self.backdrop_cache = Some((name, image.clone()));
        image
    }

    /// Preview (fast: no shadows or occlusion) or Final View (the full look).
    pub fn set_quality(&mut self, q: ViewQuality) {
        self.quality = q;
        self.quality_applied = None;
    }

    // ----- lights -----

    pub fn open_adjust_lights(&mut self, project: &Project, focus: Option<Id>) {
        let mut d = AdjustLightsDialog::new(project);
        d.focus = focus;
        self.adjust_lights = Some(d);
    }

    // ----- layout -----

    /// Send to Layout: the active elevation or section camera goes to the
    /// project's layout through the Send to Layout dialog.
    pub fn send_to_layout(&mut self, cx: &mut EditorContext) {
        let Some(id) = self
            .active_camera
            .filter(|id| cx.project.camera(*id).is_some_and(is_elevation_camera))
        else {
            cx.status = "Open an elevation or section camera to send it to layout".into();
            return;
        };
        // The open 3D view is offered too, so it can be sent as a picture.
        if let Some(view) = self.snapshot_source() {
            crate::shell::layout_window::offer_snapshot_3d(view);
        }
        crate::shell::layout_window::send_camera(cx, id);
    }

    /// Prints the project's layout to a PDF.
    pub fn export_layout_pdf(&mut self, cx: &mut EditorContext) {
        let Some(bytes) = crate::shell::layout_window::layout_pdf(&cx.project) else {
            cx.status =
                "The layout has no printed pages yet (File > New Layout, then Send to Layout)"
                    .into();
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .set_file_name(format!("{} Layout.pdf", cx.project.name))
            .save_file()
        else {
            return;
        };
        cx.status = match std::fs::write(&path, bytes) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Could not save: {e}"),
        };
    }

    // ----- the scene -----

    fn scope(&self) -> ViewScope {
        ViewScope {
            floor: self.scope_floor,
            section: self.section,
            fill: technique_view(self.technique).fill,
            no_images: true,
            clip_above: self.clip_above,
            clip_below: self.clip_below,
            slider: view_settings::SliderClip::of(&self.cam_slider),
            hide_from: self.hide_from,
            gray: self.gray,
        }
    }

    /// Hash of the model plus everything that scopes the view.
    pub fn signature(&self, project: &Project) -> u64 {
        let mut h = DefaultHasher::new();
        project_hash(project).hash(&mut h);
        self.scope_floor.hash(&mut h);
        match &self.section {
            Some(cut) => {
                1_u8.hash(&mut h);
                cut.hash_into(&mut h);
            }
            None => 0_u8.hash(&mut h),
        }
        self.scope().fill.hash(&mut h);
        self.clip_above.map(f64::to_bits).hash(&mut h);
        self.clip_below.map(f64::to_bits).hash(&mut h);
        view_settings::SliderClip::of(&self.cam_slider).hash_into(&mut h);
        self.hide_from
            .map(|p| (p.x.to_bits(), p.y.to_bits()))
            .hash(&mut h);
        self.gray.hash(&mut h);
        self.textures_on.hash(&mut h);
        // A painted object shows its material as it is now.
        if !project.object_materials.is_empty() || !project.material_defaults.is_empty() {
            crate::tools::materials::library_revision().hash(&mut h);
            // Material packages: the PBR maps switch and their size.
            let r = crate::dialogs::preferences::pages::current().render;
            (r.pbr_maps, r.max_texture_side).hash(&mut h);
        }
        h.finish()
    }

    /// Rebuilds and uploads the scene when the plan or the scope changed.
    pub fn ensure_scene(&mut self, project: &Project) {
        let sig = self.signature(project);
        if !self.scene_dirty && sig == self.last_project_hash && self.viewport.is_some() {
            return;
        }
        // Bounds of the whole model, for the slider (before any cut).
        let base = build_view_scene(
            project,
            &ViewScope {
                floor: self.scope_floor,
                ..ViewScope::default()
            },
        );
        self.model_bounds = base.bounds().map(|(lo, hi)| {
            (
                [lo[0].into(), (-hi[2]).into(), lo[1].into()],
                [hi[0].into(), (-lo[2]).into(), hi[1].into()],
            )
        });
        self.plan_bounds = base.bounds().map(|(lo, hi)| {
            (
                Point::new(lo[0].into(), (-hi[2]).into()),
                Point::new(hi[0].into(), (-lo[2]).into()),
            )
        });
        let scene = build_view_scene(project, &self.scope());
        self.scene_empty = scene.meshes.is_empty() && !has_pictures(project);
        // Keep the user's view across rebuilds, except when about to be re-aimed.
        let keep_view = self.setup.is_none();
        self.queue(&scene, keep_view);
        // Painted materials with a bitmap bring it along (flat ones are just
        // the colour on the mesh).
        if let Some(vp) = &mut self.viewport {
            let store = vp.texture_store();
            vp.set_surface_textures(crate::tools::materials::painted_textures(
                project, &scene, &store,
            ));
        }
        self.base_scene = scene;
        // The viewport holds the bare model: the overlay is added by
        // `refresh_overlay`.
        self.overlay_key = pick::empty_overlay_key();
        self.last_project_hash = sig;
        self.scene_dirty = false;
    }

    /// Hands `scene` to the viewport; `keep_view` leaves the camera where the
    /// user put it (queueing re-frames it on the scene).
    fn queue(&mut self, scene: &Scene, keep_view: bool) {
        let vp = self.viewport.get_or_insert_with(Viewport3d::new);
        let keep = (keep_view && vp.bounds().is_some()).then(|| vp.camera.clone());
        vp.queue_scene(scene);
        if let Some(cam) = keep {
            vp.camera = cam;
        }
    }

    /// The pictures facing the camera, the tint over the selection and the
    /// lighter one under the pointer, drawn over the cached scene. Called every
    /// frame; the viewport's overlay is only replaced when it changed (a
    /// billboard turned by half an inch at its edge, the selection or the
    /// hovered object changed), never for a plain redraw, and the cached scene
    /// is never re-queued for it.
    pub fn refresh_overlay(&mut self, project: &Project, selection: &Selection) {
        let Some(vp) = &self.viewport else {
            return;
        };
        let (eye, hide) = (vp.camera.eye(), vp.camera.mode.hides_ceiling_and_roof());
        let scope = self.scope();
        let pictures = pick::picture_meshes(project, &scope, eye);
        let picture_ids: std::collections::HashSet<Id> =
            pictures.iter().filter_map(|m| m.object_id).collect();
        // The key covers everything the overlay is made of, so an unchanged
        // view costs a hash of the pictures, not a rebuild of the tint.
        let hover = self.hover.object;
        let key = pick::overlay_key_with_hover(selection, hover, &pictures, hide);
        if key == self.overlay_key {
            return;
        }
        self.overlay_key = key;
        let overlay = pick::overlay_with_hover(&self.base_scene, pictures, selection, hover, hide);
        if let Some(vp) = &mut self.viewport {
            vp.set_overlay(overlay);
        }
        // Pictures draw their bitmaps (cached, so cheap when unchanged).
        let floors = scope
            .floor
            .map_or(project.floors.len(), |f| (f + 1).min(project.floors.len()));
        let bitmaps = textures::picture_textures(project, floors, &picture_ids);
        if let Some(vp) = &mut self.viewport {
            vp.set_image_textures(bitmaps);
        }
    }

    /// The object under the screen point `pos` of the viewport `rect`, with its
    /// floor (see [`pick::object_under`]).
    pub fn object_at(
        &self,
        project: &Project,
        floor: usize,
        rect: egui::Rect,
        pos: egui::Pos2,
    ) -> Option<(usize, crate::editor::ObjectRef)> {
        let vp = self.viewport.as_ref()?;
        let pictures = pick::picture_meshes(project, &self.scope(), vp.camera.eye());
        pick::object_under(
            project,
            floor,
            &vp.camera,
            rect,
            pos,
            &self.base_scene,
            &pictures,
        )
    }

    /// A press on a selected object, with the Select tool, may become a drag
    /// along the floor ([`drag`]): arms it, so the viewport leaves the drag
    /// alone instead of orbiting. Called before the viewport sees the frame.
    fn arm_drag(&mut self, ctx: &egui::Context, cx: &EditorContext, rect: egui::Rect) {
        if self.obj_drag.is_some()
            || !self.select_tool
            || cx.selection.is_empty()
            || crate::tools::materials::painter_active()
            || self.walk.is_some_and(|w| w.playing)
        {
            return;
        }
        let (pressed, alt, pos) = ctx.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.modifiers.alt,
                i.pointer.interact_pos(),
            )
        });
        let Some(pos) = pos.filter(|p| pressed && !alt && rect.contains(*p)) else {
            return;
        };
        // A toolbar or window floating over the view keeps its own clicks.
        if ctx
            .layer_id_at(pos)
            .is_some_and(|l| l.order != egui::Order::Background)
        {
            return;
        }
        let Some((floor, obj)) = self.object_at(&cx.project, cx.floor, rect, pos) else {
            return;
        };
        let Some(vp) = &self.viewport else { return };
        if floor == cx.floor {
            self.obj_drag = drag::ObjDrag::arm(cx, obj, &vp.camera, rect, pos, &self.base_scene);
        }
    }

    /// Carries an armed drag on while the button is down and ends it when it
    /// goes up (one undo step); Escape puts everything back.
    fn drive_drag(&mut self, ctx: &egui::Context, cx: &mut EditorContext, rect: egui::Rect) {
        let Some(mut d) = self.obj_drag.take() else {
            return;
        };
        let (down, pos, shift, alt, esc) = ctx.input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.latest_pos(),
                i.modifiers.shift,
                i.modifiers.alt,
                i.key_pressed(egui::Key::Escape),
            )
        });
        if esc {
            d.cancel(cx);
            return;
        }
        if !down {
            d.finish(cx);
            return;
        }
        if let (Some(pos), Some(vp)) = (pos, &self.viewport) {
            d.update(cx, &vp.camera, rect, pos, (shift, alt));
        }
        self.obj_drag = Some(d);
        ctx.request_repaint();
    }

    /// The hover highlight: picks the object under the pointer when the
    /// pointer has moved and the last pick is old enough
    /// ([`pick::hover_due`]); no pick at all while a button is down.
    fn update_hover(&mut self, ctx: &egui::Context, cx: &EditorContext, resp: &egui::Response) {
        let pos = ctx
            .input(|i| {
                (!i.pointer.any_down())
                    .then(|| i.pointer.hover_pos())
                    .flatten()
            })
            .filter(|_| resp.hovered() && self.obj_drag.is_none());
        let Some(pos) = pos else {
            self.hover.object = None;
            self.hover.last = None;
            return;
        };
        let now = ctx.input(|i| i.time);
        if !pick::hover_due(self.hover.last, pos, now) {
            return;
        }
        self.hover.last = Some((pos, now));
        self.hover.picks += 1;
        self.hover.object = pick::hover_id(self.object_at(&cx.project, cx.floor, resp.rect, pos));
    }

    /// Alt-click (C-39): the surface point under `pos` becomes the orbit
    /// centre. Returns whether it did.
    fn set_orbit_center_at(
        &mut self,
        project: &Project,
        rect: egui::Rect,
        pos: egui::Pos2,
    ) -> bool {
        let scope = self.scope();
        let Some(vp) = &mut self.viewport else {
            return false;
        };
        let pictures = pick::picture_meshes(project, &scope, vp.camera.eye());
        pick::surface_point(&vp.camera, rect, pos, &self.base_scene, &pictures)
            .is_some_and(|p| vp.camera.set_orbit_center(p))
    }

    /// Double-click on nothing: frame the whole building again, keeping the
    /// viewing angle.
    fn recenter_on_model(&mut self) -> bool {
        match &mut self.viewport {
            Some(vp) if vp.camera.mode != CameraMode::FullCamera && vp.bounds().is_some() => {
                vp.fit_view();
                true
            }
            _ => false,
        }
    }

    /// Aims the viewport after a request (call after [`ensure_scene`](Self::ensure_scene)).
    pub fn apply_setup(&mut self, aspect: f32) {
        let Some(setup) = self.setup.take() else {
            return;
        };
        let vp = self.viewport.get_or_insert_with(Viewport3d::new);
        match setup {
            Setup::Mode(m) => {
                vp.set_mode(m);
                vp.fit_view();
            }
            Setup::Parallel { mode, iso } => {
                vp.set_mode(mode);
                vp.fit_view();
                if let Some((yaw, pitch)) = iso {
                    vp.camera.yaw = yaw;
                    vp.camera.pitch = pitch;
                }
                vp.camera.make_parallel();
            }
            Setup::Camera {
                position,
                direction_deg,
                eye_y,
                eye_height,
                fov_deg,
                tilt_deg,
            } => {
                vp.set_mode(CameraMode::FullCamera);
                let c = &mut vp.camera;
                c.eye_height = eye_height as f32;
                c.position = [position.x as f32, eye_y as f32, -position.y as f32];
                c.yaw = direction_deg.to_radians() as f32 - FRAC_PI_2;
                c.pitch = 0.0;
                c.set_tilt_deg(tilt_deg as f32);
                c.fov_deg = vertical_fov(fov_deg as f32, aspect);
            }
            Setup::Pose { mode, eye, target } => {
                vp.set_mode(mode);
                vp.fit_view();
                // A saved pose that does not fit (degenerate) keeps the framing.
                vp.camera.look_from(eye, target);
            }
            Setup::Section(cut) => {
                vp.set_mode(cut.elevation_mode());
                vp.camera.yaw = cut.yaw();
            }
            Setup::Ortho(view) => {
                vp.set_mode(extras::ortho_mode(view.kind));
                vp.fit_view();
                extras::apply_ortho(&mut vp.camera, &view);
            }
        }
        // The program aimed the camera: not a step for Undo Zoom.
        self.zoom.reset_to(&vp.camera);
        self.mode = vp.camera.mode;
    }

    /// The camera after `active_camera` on the floor list (Tab, C-29).
    fn next_camera(&self, project: &Project) -> Option<Id> {
        let ids: Vec<Id> = project.cameras.iter().map(|c| c.id).collect();
        let at = self
            .active_camera
            .and_then(|a| ids.iter().position(|i| *i == a));
        match at {
            Some(i) => ids.get((i + 1) % ids.len()).copied(),
            None => ids.first().copied(),
        }
    }

    // ----- frame -----

    /// Per-frame work outside the central area: applies requests and shows the
    /// 3D dialogs. Returns whether the 3D view is active (the caller then
    /// calls [`show`], otherwise it draws the plan).
    pub fn frame(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        self.drain(&cx.project);
        if std::mem::take(&mut self.save_requested) {
            self.save_camera(cx);
        }
        self.windows(ctx, cx);
        self.active
    }

    fn windows(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        if let Some(mut d) = self.camera_dialog.take() {
            if d.wants_defaults_env() {
                let env = crate::dialogs::default_sets::Env::of(cx);
                let plan = crate::dialogs::default_sets::Selected::current(cx);
                d.set_defaults_env(env, plan);
            }
            match d.show(ctx) {
                Outcome::Open => {
                    if d.take_export_request() {
                        cx.status = crate::dialogs::camera::save_camera_dxf(&cx.project, d.draft());
                    }
                    // Add, Edit, Rename and Delete of the Selected Defaults
                    // panel act on the plan's lists.
                    let events = d.take_defaults_events();
                    if !events.is_empty() {
                        let mut sel = d.selected_draft();
                        for ev in events {
                            if let Some(msg) =
                                crate::dialogs::default_sets::handle_event(cx, ev, &mut sel)
                            {
                                cx.status = msg;
                            }
                        }
                        d.set_selected_draft(&sel);
                    }
                    self.camera_dialog = Some(d);
                }
                Outcome::Cancel => {}
                Outcome::Ok => self.apply_camera_dialog(cx, &d),
            }
        }
        self.slider_window(ctx, cx);
        self.lights_window(ctx, cx);
        self.lighting_window(ctx, cx);
        self.record_window(ctx, cx);
        self.recording_window(ctx, cx);
        self.panorama_windows(ctx, cx);
        self.sun_window(ctx, cx);
        self.defaults_window(ctx);
        if self.raytrace.open || self.raytrace.is_running() {
            self.raytrace.lights = crate::dialogs::camera::render_lights_in(
                &cx.project,
                self.active_camera
                    .and_then(|id| cx.project.camera(id))
                    .and_then(|c| c.view.light_set.as_deref()),
            );
            let scope = ViewScope {
                fill: None,
                no_images: false,
                ..self.scope()
            };
            let cam = self.viewport.as_ref().map(|v| v.camera.clone());
            let project = &cx.project;
            let mut source = || {
                let cam = cam.as_ref()?;
                let scene = build_view_scene(project, &scope);
                (!scene.meshes.is_empty()).then(|| (scene, render_camera(cam)))
            };
            self.raytrace.ui(ctx, &mut source);
        } else {
            self.raytrace.poll();
        }
    }

    /// Adjust Lights (C-64): edits a draft, applied with OK.
    fn lights_window(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        let Some(mut d) = self.adjust_lights.take() else {
            return;
        };
        match d.show(ctx) {
            Outcome::Open => self.adjust_lights = Some(d),
            Outcome::Cancel => {}
            Outcome::Ok => {
                cx.begin_change("Adjust Lights");
                d.apply(&mut cx.project);
                cx.mark_dirty();
            }
        }
    }

    /// Progress of a walkthrough recording.
    fn recording_window(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        let Some(rec) = &self.recording else {
            return;
        };
        if let Some(result) = rec.finished() {
            cx.status = match result {
                Ok(n) => match &rec.video {
                    Some(v) => format!("Recorded {n} frames to {}", v.display()),
                    None => format!("Recorded {n} frames to {}", rec.dir.display()),
                },
                Err(e) => format!("Recording failed: {e}"),
            };
            self.recording = None;
            return;
        }
        let (done, total) = rec.progress();
        let mut cancel = false;
        egui::Window::new("Record Walkthrough")
            .id(egui::Id::new("record_walkthrough_window"))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let frac = if total == 0 {
                    0.0
                } else {
                    done as f32 / total as f32
                };
                ui.add(egui::ProgressBar::new(frac).text(format!("{done} / {total} frames")));
                match &rec.video {
                    Some(v) => ui.weak(format!("Writing {}", v.display())),
                    None => ui.weak(format!("Writing frame_NNNN.png to {}", rec.dir.display())),
                };
                cancel = ui.button("Cancel").clicked();
            });
        if cancel {
            rec.cancel();
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    /// The Cross Section Slider dialog (C-141): it works on the view's planes
    /// while it is open; Done saves them with the camera on show, one undo
    /// step.
    fn slider_window(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        if !self.slider_dialog {
            return;
        }
        let (lo, hi) = self
            .model_bounds
            .unwrap_or(([0.0; 3], [1200.0, 1200.0, 300.0]));
        let (done, _changed) = crate::dialogs::camera::slider::show(
            ctx,
            &mut self.slider_fields,
            &mut self.cam_slider,
            lo,
            hi,
        );
        if done {
            self.slider_dialog = false;
            if let Some(id) = self.active_camera {
                let planes = self.cam_slider.clone();
                if cx
                    .project
                    .camera(id)
                    .is_some_and(|c| c.view.slider != planes)
                {
                    cx.begin_change("Cross Section Slider");
                    cx.project.update_camera(id, |c| c.view.slider = planes);
                    cx.mark_dirty();
                }
            }
        }
    }

    fn apply_camera_dialog(&mut self, cx: &mut EditorContext, d: &CameraDialog) {
        let id = d.id();
        cx.begin_change("Camera Specification");
        let draft = d.draft().clone();
        cx.project.update_camera(id, |c| *c = draft);
        cx.mark_dirty();
        self.extras.insert(id, d.extras());
        if self.active_camera == Some(id) {
            self.show_camera(&cx.project, id);
        }
    }

    /// The Sun Angle toggle (C-63): either a date, time and latitude or the
    /// angles themselves. The sun also drives the ray tracer, the GL key light
    /// and the shadows of vector elevations.
    fn sun_window(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        if !cx.view_flags.contains(&ViewFlag::SunAngle) {
            self.sun_light = None;
            return;
        }
        let mut open = true;
        let mut apply = false;
        let rt = &mut self.raytrace;
        egui::Window::new("Sun Angle")
            .id(egui::Id::new("sun_angle_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let mut manual = rt.manual_sun.is_some();
                ui.horizontal(|ui| {
                    ui.radio_value(&mut manual, false, "Date and time");
                    ui.radio_value(&mut manual, true, "Angles");
                });
                if manual != rt.manual_sun.is_some() {
                    let s = rt.sun();
                    rt.manual_sun = manual.then_some((s.azimuth_deg, s.altitude_deg.max(1.0)));
                }
                ui.add_enabled_ui(!manual, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Date");
                        ui.add(
                            egui::DragValue::new(&mut rt.date.0)
                                .range(1..=12)
                                .prefix("month "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut rt.date.1)
                                .range(1..=31)
                                .prefix("day "),
                        );
                    });
                    ui.add(
                        egui::Slider::new(&mut rt.time_hours, 0.0..=24.0)
                            .text("Time")
                            .fixed_decimals(1),
                    );
                    ui.add(
                        egui::DragValue::new(&mut rt.latitude)
                            .range(-66.0..=66.0)
                            .prefix("Latitude ")
                            .suffix("\u{B0}"),
                    );
                });
                if let Some((az, alt)) = &mut rt.manual_sun {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(az)
                                .range(0.0..=360.0)
                                .prefix("Azimuth ")
                                .suffix("\u{B0}"),
                        );
                        ui.add(
                            egui::DragValue::new(alt)
                                .range(-10.0..=90.0)
                                .prefix("Altitude ")
                                .suffix("\u{B0}"),
                        );
                    });
                }
                let s = rt.sun();
                ui.weak(format!(
                    "Azimuth {:.0}\u{B0}, altitude {:.0}\u{B0}",
                    s.azimuth_deg, s.altitude_deg
                ));
                apply = ui
                    .button("Use for elevation shadows")
                    .on_hover_text("Turns shadows on in every elevation and section camera")
                    .clicked();
            });
        if apply {
            let s = self.raytrace.sun();
            cx.begin_change("Sun Angle Shadows");
            for c in cx
                .project
                .cameras
                .iter_mut()
                .filter(|c| is_elevation_camera(c))
            {
                c.render.shadows = true;
                c.render.sun_azimuth_deg = s.azimuth_deg;
                c.render.sun_altitude_deg = s.altitude_deg.max(1.0);
            }
            cx.mark_dirty();
        }
        if !open {
            cx.view_flags.remove(&ViewFlag::SunAngle);
        }
        let s = self.raytrace.sun();
        self.sun_light = (s.altitude_deg > 0.0).then(|| s.direction_to_sun().map(|v| v as f32));
    }

    fn defaults_window(&mut self, ctx: &egui::Context) {
        if !self.show_defaults {
            return;
        }
        let mut d = camera_defaults();
        let mut callout = camera_tool::callout_style();
        let mut open = true;
        let mut tech = self.technique;
        egui::Window::new("3D View Defaults")
            .id(egui::Id::new("view3d_defaults"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("v3d_defaults")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Camera eye height");
                        ui.add(
                            egui::DragValue::new(&mut d.eye_height)
                                .range(12.0..=600.0)
                                .suffix("\""),
                        );
                        ui.end_row();
                        ui.label("Angle of view");
                        ui.add(
                            egui::DragValue::new(&mut d.fov_deg)
                                .range(5.0..=170.0)
                                .suffix("\u{B0}"),
                        );
                        ui.end_row();
                        ui.label("Rendering technique");
                        egui::ComboBox::from_id_salt("v3d_default_technique")
                            .selected_text(tech.label())
                            .show_ui(ui, |ui| {
                                for t in RenderingTechnique::ALL {
                                    ui.selectable_value(&mut tech, t, t.label());
                                }
                            });
                        ui.end_row();
                        ui.label("Callout shape");
                        egui::ComboBox::from_id_salt("v3d_default_callout")
                            .selected_text(callout.shape.label())
                            .show_ui(ui, |ui| {
                                for s in camera_tool::CalloutShape::ALL {
                                    ui.selectable_value(&mut callout.shape, s, s.label());
                                }
                            });
                        ui.end_row();
                        ui.label("Callout size");
                        ui.add(
                            egui::DragValue::new(&mut callout.radius)
                                .range(4.0..=36.0)
                                .suffix("\" radius"),
                        );
                        ui.end_row();
                        ui.label("Callout shows the name");
                        ui.checkbox(&mut callout.show_name, "");
                        ui.end_row();
                    });
            });
        set_camera_defaults(d);
        camera_tool::set_callout_style(callout);
        self.technique = tech;
        self.show_defaults = open;
    }
}

/// The ray tracer's camera for the viewport's current view.
fn render_camera(cam: &plan_view3d::Camera) -> plan_render::Camera {
    let eye = cam.eye();
    let f = cam.forward();
    plan_render::Camera {
        eye,
        target: [eye[0] + f[0], eye[1] + f[1], eye[2] + f[2]],
        up: [0.0, 1.0, 0.0],
        fov_deg: cam.fov_deg,
        aperture: 0.0,
        focus_dist: 0.0,
    }
}

// ----- Print Image of the 3D view -----

/// Longest side of a 3D Print Image, pixels.
pub const SNAPSHOT_MAX_PX: u32 = 4096;
/// Samples per pixel of [`View3dState::snapshot_png`].
#[allow(dead_code)]
pub const SNAPSHOT_SAMPLES: u32 = 24;

/// The 3D view as it is: the scene on screen and the camera it looks
/// through, kept so a Print Image can render it later at any size.
///
/// Limits: `plan_view3d` draws through the viewport's GL callback and has no
/// offscreen target to read back, so the picture is ray traced from the
/// viewport's camera. It shows the model's materials with the default clear-day
/// sun and sky, no point lights, no selection tint and no pictures; the
/// vector and technical-illustration techniques print as the shaded view.
#[derive(Clone)]
pub struct Snapshot3d {
    pub scene: Scene,
    pub camera: plan_render::Camera,
}

impl Snapshot3d {
    /// Ray traces the view `width` x `height` pixels (each clamped to
    /// 16..=[`SNAPSHOT_MAX_PX`]) at `samples` per pixel and encodes it as PNG.
    /// `None` when the scene is empty.
    pub fn render_png(&self, width: u32, height: u32, samples: u32) -> Option<Vec<u8>> {
        if self.scene.meshes.is_empty() {
            return None;
        }
        let side = |v: u32| v.clamp(16, SNAPSHOT_MAX_PX);
        let settings = plan_render::RenderSettings {
            width: side(width),
            height: side(height),
            samples: samples.clamp(1, 512),
            ..plan_render::RenderSettings::default()
        };
        let renderer = plan_render::Renderer::new(&self.scene);
        let image = renderer.render(
            &self.camera,
            &plan_render::Environment::default(),
            &[],
            &settings,
        );
        Some(plan_render::encode_png(&image))
    }
}

impl View3dState {
    /// The view as Print Image needs it; `None` without a viewport or a model.
    pub fn snapshot_source(&self) -> Option<Snapshot3d> {
        let cam = &self.viewport.as_ref()?.camera;
        (!self.base_scene.meshes.is_empty()).then(|| Snapshot3d {
            scene: self.base_scene.clone(),
            camera: render_camera(cam),
        })
    }

    /// Print Image of the 3D view: the view as a PNG `width` x `height`
    /// pixels (see [`Snapshot3d`] for what it shows and what it leaves out).
    #[allow(dead_code)] // The headless Print Image: same render, no dialog.
    pub fn snapshot_png(&self, width: u32, height: u32) -> Option<Vec<u8>> {
        self.snapshot_source()?
            .render_png(width, height, SNAPSHOT_SAMPLES)
    }
}

/// The time of the key frame (path node) before (`dir` < 0) or after (`dir`
/// > 0) `t_s` on walkthrough `c`; the ends when there is none.
pub fn jump_key_frame(c: &CameraObject, t_s: f64, dir: i32) -> f64 {
    const EPS: f64 = 1e-6;
    let times: Vec<f64> = (0..c.path.len()).map(|i| c.walk_node_time(i)).collect();
    if dir < 0 {
        times
            .iter()
            .rev()
            .find(|t| **t < t_s - EPS)
            .copied()
            .unwrap_or(0.0)
    } else {
        times
            .iter()
            .find(|t| **t > t_s + EPS)
            .copied()
            .unwrap_or_else(|| c.walk_duration_s())
    }
}

// ----- drawing -----

/// What the user asked for in a view's toolbar this frame.
#[derive(Default)]
struct BarOut {
    mode: Option<CameraMode>,
    technique: Option<RenderingTechnique>,
    rebuild: bool,
    ray: bool,
    back: bool,
    refresh: bool,
    to_layout: bool,
    layout_pdf: bool,
    slider: Option<f64>,
    walk_toggle: bool,
    walk_stop: bool,
    quality: Option<ViewQuality>,
    walk_scrub: Option<f64>,
    /// Jump to the previous (-1) or next (+1) key frame.
    walk_jump: Option<i32>,
    walk_speed: Option<f64>,
    record: bool,
    /// Export the vector drawing as a DXF.
    dxf: bool,
}

/// The technique combo box shared by both views.
fn technique_combo(ui: &mut egui::Ui, current: RenderingTechnique, out: &mut BarOut) {
    egui::ComboBox::from_id_salt("view3d_technique")
        .selected_text(current.label())
        .show_ui(ui, |ui| {
            for t in RenderingTechnique::ALL {
                if ui.selectable_label(current == t, t.label()).clicked() {
                    out.technique = Some(t);
                }
            }
        });
}

/// The Shading menu: shadows, ambient occlusion, quality and exposure of the
/// interactive view (`plan_view3d::ViewSettings`).
fn shading_menu(ui: &mut egui::Ui, settings: Option<&mut ViewSettings>, final_on: &mut bool) {
    let Some(s) = settings else { return };
    ui.menu_button("Shading", |ui| {
        ui.checkbox(final_on, "Path-traced Final View")
            .on_hover_text("When the camera stops, ray trace the view and refine it until Stop");
        ui.checkbox(&mut s.shadows, "Shadows");
        ui.checkbox(&mut s.ambient_occlusion, "Ambient occlusion");
        ui.horizontal(|ui| {
            ui.label("Quality");
            for q in Quality::ALL {
                ui.selectable_value(&mut s.quality, q, q.label());
            }
        });
        ui.add(egui::Slider::new(&mut s.exposure, 0.5..=2.0).text("Exposure"));
    });
}

/// Draws the 3D view in the central area.
pub fn show(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut View3dState) {
    let ctx = ui.ctx().clone();
    if st.vector_source(&cx.project).is_some() {
        show_vector(ui, cx, st);
        return;
    }
    st.ensure_scene(&cx.project);
    if let Some(note) = crate::tools::library::chief::take_partial_notice() {
        cx.status = note;
    }
    let rect = ui.available_rect_before_wrap();
    let size = ui.available_size();
    st.apply_setup(size.x / size.y.max(1.0));

    // A playing walkthrough moves the eye along its path.
    let dt = ctx.input(|i| f64::from(i.stable_dt.min(0.1)));
    let walk_pose = if st.walk.is_some_and(|w| w.playing) {
        ctx.request_repaint();
        st.tick_walk(&cx.project, dt)
    } else {
        None
    };
    let walk_tilt = st
        .walk
        .and_then(|w| cx.project.camera(w.camera).map(|c| walk_tilt(c, w.t_s)));

    let tv = technique_view(st.technique);
    let sun = st.sun_light;
    // The camera on show brings its own backdrop, lighting and quality.
    let shown = st.active_camera.and_then(|id| cx.project.camera(id));
    let view = shown.map(|c| c.view.clone());
    let view_label = shown
        .filter(|c| c.view.label.show_in_view)
        .map(|c| c.view.label_text(&c.name).to_string());
    let backdrop = st.backdrop_for(view.as_ref());
    let plan_lighting = cx.project.lighting.clone();
    let (quality, quality_due) = (st.quality, st.quality_applied != Some(st.quality));
    st.quality_applied = Some(quality);
    st.aspect = size.x / size.y.max(1.0);
    st.arm_drag(&ctx, cx, rect);
    let vp = st.viewport.get_or_insert_with(Viewport3d::new);
    // A drag that moves an object must not also orbit the camera.
    vp.drag_locked = st.obj_drag.is_some();
    apply_to_viewport(vp, &tv, sun);
    vp.lighting = view_settings::rig(&plan_lighting, view.as_ref(), sun, tv.flat);
    // The camera's own rendering options (C-147, C-150, C-151).
    vp.ao_scale = view_settings::ao_scale(view.as_ref());
    vp.camera.near =
        view_settings::near_of(view.as_ref(), vp.camera.mode == CameraMode::FullCamera);
    if let Some(sky) = view.as_ref().and_then(view_settings::sky_color) {
        vp.background = sky;
    }
    vp.backdrop = backdrop;
    vp.ground = view_settings::ground_of(view.as_ref());
    vp.fog = view_settings::fog_of(view.as_ref());
    if quality_due {
        vp.settings = view_settings::settings_for(
            quality,
            vp.settings.exposure,
            view.as_ref()
                .is_none_or(|v| v.shadows && v.options.use_sunlight),
        );
    }
    let mut lights = crate::dialogs::camera::render_lights_in(
        &cx.project,
        view.as_ref().and_then(|v| v.light_set.as_deref()),
    );
    // Automatic lighting uses at most the camera's Maximum Number of lights.
    if let Some(v) = view
        .as_ref()
        .filter(|v| v.options.light_choice == plan_core::camera_view::spec::LightChoice::Automatic)
    {
        lights.truncate(v.options.max_lights as usize);
    }
    vp.set_point_lights(&lights);
    vp.textures_enabled = st.textures_on && technique_shows_textures(st.technique);
    if let Some((position, yaw)) = walk_pose {
        vp.camera.position = position;
        vp.camera.yaw = yaw;
        vp.camera.pitch = 0.0;
        if let Some(t) = walk_tilt {
            vp.camera.set_tilt_deg(t);
        }
    }
    let resp = vp.ui(ui, size);
    st.mode = vp.camera.mode;
    let stand_cam = vp.camera.clone();
    // Undo Zoom: a playing walkthrough moves the camera by itself.
    if st.walk.is_some_and(|w| w.playing) {
        st.zoom.reset_to(&vp.camera);
    } else {
        st.zoom.observe(&vp.camera);
    }

    // Page Up / Page Down raise and lower the eye (C-38).
    if vp.camera.mode == CameraMode::FullCamera && (resp.hovered() || resp.has_focus()) {
        let (dt, rise) = ctx.input(|i| {
            let k = |key| f32::from(i.key_down(key));
            (
                i.stable_dt.min(0.1),
                k(egui::Key::PageUp) - k(egui::Key::PageDown),
            )
        });
        if rise != 0.0 {
            vp.camera.position[1] += rise * EYE_RISE_SPEED * dt;
            ctx.request_repaint();
        }
    }
    let gl_error = vp.gl_error();
    st.drive_drag(&ctx, cx, resp.rect);
    handle_pointer_and_keys(&ctx, cx, st, &resp);
    st.update_hover(&ctx, cx, &resp);
    st.refresh_overlay(&cx.project, &cx.selection);
    stand_in::paint(ui.painter(), &stand_cam, resp.rect, &cx.project);
    st.final_view_frame(ui, cx, resp.rect);
    if view.as_ref().is_some_and(|v| v.options.show_watermark) {
        extras::paint_watermark(
            ui.painter(),
            resp.rect,
            &cx.project.print_setup.watermark.spec,
        );
    }
    if let Some(text) = view_label {
        ui.painter().text(
            resp.rect.left_bottom() + egui::vec2(12.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            text,
            egui::FontId::proportional(OVERLAY_TEXT_PX),
            egui::Color32::from_gray(0x30),
        );
    }
    if let Some(e) = gl_error {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("3D view unavailable: {e}"),
            egui::FontId::proportional(OVERLAY_TEXT_PX),
            egui::Color32::from_rgb(0xE0, 0x4B, 0x4B),
        );
    } else if st.scene_empty {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Nothing to show yet. Draw some walls in the plan.",
            egui::FontId::proportional(OVERLAY_TEXT_PX),
            egui::Color32::from_gray(0x60),
        );
    }

    let mut out = BarOut::default();
    let walk_info = st.walk.and_then(|w| {
        cx.project.camera(w.camera).map(|c| {
            (
                w,
                c.walk_duration_s(),
                c.walk_speed,
                c.view.walk.clamped().fps,
            )
        })
    });
    egui::Area::new(egui::Id::new("view3d_toolbar"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min + egui::vec2(8.0, 8.0))
        .show(&ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let views = standard_views();
                    let label = views
                        .iter()
                        .find(|(_, m)| *m == st.mode)
                        .map_or("View", |(n, _)| *n);
                    egui::ComboBox::from_id_salt("view3d_mode")
                        .selected_text(label)
                        .show_ui(ui, |ui| {
                            for (name, m) in &views {
                                if ui.selectable_label(st.mode == *m, *name).clicked() {
                                    out.mode = Some(*m);
                                }
                            }
                        });
                    technique_combo(ui, st.technique, &mut out);
                    ui.checkbox(&mut st.textures_on, "Textures");
                    shading_menu(
                        ui,
                        st.viewport.as_mut().map(|v| &mut v.settings),
                        &mut st.final_view.enabled,
                    );
                    out.rebuild = ui.button("Rebuild 3D").clicked();
                    for q in ViewQuality::ALL {
                        if ui
                            .selectable_label(st.quality == q, q.label())
                            .on_hover_text(match q {
                                ViewQuality::Preview => "Fast: no shadows or occlusion",
                                ViewQuality::Final => "Shadows, occlusion and anti-aliasing",
                            })
                            .clicked()
                        {
                            out.quality = Some(q);
                        }
                    }
                    out.ray = ui.button("Ray Trace\u{2026}").clicked();
                    out.back = ui.button("Back to Plan").clicked();
                });
                if let Some(s) = &st.slider {
                    let mut off = s.offset;
                    let r = ui.add(
                        egui::Slider::new(&mut off, 0.0..=s.depth)
                            .text("Cross section depth")
                            .suffix("\""),
                    );
                    if r.changed() {
                        out.slider = Some(off);
                    }
                }
                if let Some((w, duration, speed, fps)) = walk_info {
                    ui.horizontal(|ui| {
                        let label = if w.playing {
                            "Pause"
                        } else {
                            "Play Walkthrough"
                        };
                        out.walk_toggle = ui.button(label).clicked();
                        out.walk_stop = ui.button("Stop").clicked();
                        if ui
                            .button("|<")
                            .on_hover_text("Previous key frame")
                            .clicked()
                        {
                            out.walk_jump = Some(-1);
                        }
                        if ui.button(">|").on_hover_text("Next key frame").clicked() {
                            out.walk_jump = Some(1);
                        }
                        let mut t = w.t_s;
                        let r = ui.add(
                            egui::Slider::new(&mut t, 0.0..=duration.max(0.01))
                                .text("s")
                                .fixed_decimals(1),
                        );
                        if r.changed() {
                            out.walk_scrub = Some(t);
                        }
                        ui.weak(format!(
                            "frame {} of {}",
                            (w.t_s * fps).round() as usize + 1,
                            (duration * fps).round().max(1.0) as usize
                        ));
                        let mut v = speed;
                        let r = ui.add(
                            egui::DragValue::new(&mut v)
                                .range(6.0..=600.0)
                                .prefix("Speed ")
                                .suffix(" in/s"),
                        );
                        if r.changed() {
                            out.walk_speed = Some(v);
                        }
                        out.record = ui.button("Record Walkthrough\u{2026}").clicked();
                    });
                }
            });
        });
    apply_bar(&ctx, cx, st, out, Some(&resp));
}

/// Selection in the 3D view (C-43): a click selects the object under the
/// pointer (Shift adds), a double-click also asks for its specification (on
/// nothing it frames the whole building again), Alt-click makes the clicked
/// surface point the orbit centre (C-39), Delete deletes the selection and
/// the arrow keys nudge it.
fn handle_pointer_and_keys(
    ctx: &egui::Context,
    cx: &mut EditorContext,
    st: &mut View3dState,
    resp: &egui::Response,
) {
    let clicked = resp.double_clicked() || resp.clicked();
    if clicked {
        if let Some(pos) = resp.interact_pointer_pos() {
            let (add, alt) = ctx.input(|i| (i.modifiers.shift, i.modifiers.alt));
            if alt {
                cx.status = if st.set_orbit_center_at(&cx.project, resp.rect, pos) {
                    "Orbit center set to the clicked surface".into()
                } else {
                    "Alt-click a surface in the overview or doll house view to orbit it".into()
                };
            } else {
                let hit = st.object_at(&cx.project, cx.floor, resp.rect, pos);
                if resp.double_clicked() {
                    if hit.is_none() && st.recenter_on_model() {
                        cx.status = "Centered on the building".into();
                    }
                    pick::apply_open(cx, hit);
                } else {
                    pick::apply_pick(cx, hit, add);
                }
            }
            // The request is run, and the selection shown, next frame.
            ctx.request_repaint();
        }
    }
    // The view itself holds the keyboard focus after a click (for the walking
    // keys); a text field somewhere else keeps its Delete.
    let other_has_focus = ctx
        .memory(|m| m.focused())
        .is_some_and(|focused| focused != resp.id);
    let wants_delete = (resp.hovered() || resp.has_focus())
        && !other_has_focus
        && ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
    if wants_delete && !cx.selection.is_empty() {
        cx.delete_selection();
        ctx.request_repaint();
    }
    // Arrow keys nudge the selection by a snap unit (ten with Shift) along the
    // plan axis nearest the arrow on screen; Full Camera walks with them.
    let walking = st
        .viewport
        .as_ref()
        .is_none_or(|v| v.camera.mode == CameraMode::FullCamera);
    if (resp.hovered() || resp.has_focus())
        && !other_has_focus
        && !walking
        && st.select_tool
        && st.obj_drag.is_none()
        && !cx.selection.is_empty()
    {
        let presses: Vec<(egui::Key, bool)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } if !modifiers.command => Some((*key, modifiers.shift)),
                    _ => None,
                })
                .collect()
        });
        for (key, big) in presses {
            let Some(cam) = st.viewport.as_ref().map(|v| &v.camera) else {
                break;
            };
            if let Some(dir) = drag::nudge_dir(cam, key) {
                let step = cx.snap_unit() * if big { 10.0 } else { 1.0 };
                if drag::nudge(cx, dir * step) {
                    ctx.request_repaint();
                }
            }
        }
    }
}

/// Applies a toolbar's output to the state (both views).
fn apply_bar(
    ctx: &egui::Context,
    cx: &mut EditorContext,
    st: &mut View3dState,
    out: BarOut,
    resp: Option<&egui::Response>,
) {
    if let Some(m) = out.mode {
        st.set_mode_user(m);
    }
    if let Some(t) = out.technique {
        st.set_technique(t);
    }
    if let Some(q) = out.quality {
        st.set_quality(q);
    }
    if let Some(off) = out.slider {
        if let Some(s) = &mut st.slider {
            s.offset = off;
            let mut cut = s.base;
            cut.origin = cut.origin + cut.dir * off;
            st.section = Some(cut);
        }
    }
    if out.rebuild {
        st.rebuild();
        cx.status = "Rebuilt the 3D model".into();
    }
    if out.refresh {
        st.refresh();
    }
    if out.dxf {
        st.export_vector_dxf(cx);
    }
    if out.ray {
        st.raytrace.open = true;
    }
    if out.to_layout {
        st.send_to_layout(cx);
    }
    if out.layout_pdf {
        st.export_layout_pdf(cx);
    }
    if out.record {
        st.record_walkthrough_dialog(cx);
    }
    // Walkthrough transport.
    if let Some(w) = &mut st.walk {
        let end = cx
            .project
            .camera(w.camera)
            .map_or(0.0, CameraObject::walk_duration_s);
        if out.walk_toggle {
            if w.t_s >= end {
                w.t_s = 0.0;
            }
            w.playing = !w.playing;
        }
        if out.walk_stop {
            w.playing = false;
            w.t_s = 0.0;
        }
        if let Some(t) = out.walk_scrub {
            w.playing = false;
            w.t_s = t.clamp(0.0, end);
        }
        if let Some(dir) = out.walk_jump {
            w.playing = false;
            if let Some(c) = cx.project.camera(w.camera) {
                w.t_s = jump_key_frame(c, w.t_s, dir);
            }
        }
    }
    if out.walk_stop || out.walk_scrub.is_some() || out.walk_jump.is_some() {
        if let (Some(w), Some(vp)) = (st.walk, st.viewport.as_mut()) {
            if let Some(c) = cx.project.camera(w.camera) {
                let (position, yaw) = walk_view(&cx.project, c, w.t_s);
                vp.camera.position = position;
                vp.camera.yaw = yaw;
                vp.camera.pitch = 0.0;
                vp.camera.set_tilt_deg(walk_tilt(c, w.t_s));
            }
        }
    }
    if let (Some(speed), Some(id)) = (out.walk_speed, st.walk.map(|w| w.camera)) {
        cx.begin_change_merged("Walking Speed");
        cx.project.update_camera(id, |c| c.walk_speed = speed);
        cx.mark_dirty();
    }
    if !ctx.input(|i| i.pointer.any_down()) {
        cx.end_merge();
    }
    let hot = resp.is_none_or(|r| r.hovered() || r.has_focus());
    let tab = ctx.input(|i| i.key_pressed(egui::Key::Tab)) && hot;
    if tab {
        if let Some(id) = st.next_camera(&cx.project) {
            st.show_camera(&cx.project, id);
        }
    }
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if out.back || esc {
        st.active = false;
    }
}

/// The vector elevation of the active camera in the central area: pan with a
/// drag, zoom with the wheel, double-click to fit.
fn show_vector(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut View3dState) {
    let ctx = ui.ctx().clone();
    let sun = st.plan_sun(&cx.project, cx.view_flags.contains(&ViewFlag::SunAngle));
    st.refresh_vector(&cx.project, sun);
    let rect = ui.available_rect_before_wrap();
    let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
    ui.painter()
        .rect_filled(rect, 0.0, egui::Color32::from_rgb(0xFA, 0xFA, 0xF8));
    let xform = st.vector.transform(rect).filter(|_| st.annotate);
    if let Some(xf) = xform {
        let (pressed, released, dbl, held, pos) = ctx.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.button_released(egui::PointerButton::Primary),
                i.pointer
                    .button_double_clicked(egui::PointerButton::Primary),
                i.pointer.button_down(egui::PointerButton::Primary),
                i.pointer.latest_pos(),
            )
        });
        if let Some(at) = pos
            .filter(|_| resp.hovered() || held)
            .map(|p| xf.to_drawing(p))
        {
            let mk = |kind, down| ViewPointer { kind, at, down };
            if pressed && resp.hovered() {
                if dbl {
                    st.annot_events.push(mk(ViewPtr::Double, true));
                }
                st.annot_events.push(mk(ViewPtr::Down, true));
            } else if released {
                st.annot_events.push(mk(ViewPtr::Up, false));
            } else {
                st.annot_events.push(mk(ViewPtr::Move, held));
            }
        }
        if resp.dragged_by(egui::PointerButton::Middle) {
            st.vector.pan += resp.drag_delta();
        }
    } else if resp.dragged() {
        st.vector.pan += resp.drag_delta();
    }
    if resp.hovered() {
        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
        if let (true, Some(at)) = (scroll != 0.0, resp.hover_pos()) {
            st.vector.zoom_about(rect, at, (scroll / 240.0).exp());
        }
    }
    if resp.double_clicked() && !st.annotate {
        st.vector.reset_view();
    }
    paint_vector(ui.painter(), rect, &st.vector, st.technique);
    let name = st
        .active_camera
        .and_then(|id| cx.project.camera(id))
        .map_or_else(
            || {
                if st.slider.is_some() {
                    "Cross Section Slider".to_string()
                } else {
                    String::new()
                }
            },
            |c| c.name.clone(),
        );
    if st.vector.drawing().is_none() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Drawing the view\u{2026}",
            egui::FontId::proportional(OVERLAY_TEXT_PX),
            egui::Color32::from_gray(0x60),
        );
    } else if st.vector.drawing().is_some_and(|d| d.lines.is_empty()) {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Nothing to draw: the view is empty.",
            egui::FontId::proportional(OVERLAY_TEXT_PX),
            egui::Color32::from_gray(0x60),
        );
    }
    let busy = st.vector.is_busy();
    let mut out = BarOut::default();
    egui::Area::new(egui::Id::new("view3d_toolbar"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min + egui::vec2(8.0, 8.0))
        .show(&ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(&name);
                    technique_combo(ui, st.technique, &mut out);
                    out.refresh = ui.button("Refresh").clicked();
                    out.dxf = ui.button("Export DXF\u{2026}").clicked();
                    out.to_layout = ui.button("Send to Layout").clicked();
                    out.layout_pdf = ui.button("Layout PDF\u{2026}").clicked();
                    out.ray = ui.button("Ray Trace\u{2026}").clicked();
                    out.back = ui.button("Back to Plan").clicked();
                    if busy {
                        ui.spinner();
                    }
                });
                if let Some(s) = &st.slider {
                    let mut off = s.offset;
                    let r = ui.add(
                        egui::Slider::new(&mut off, 0.0..=s.depth)
                            .text("Cross section depth")
                            .suffix("\""),
                    );
                    if r.changed() {
                        out.slider = Some(off);
                    }
                }
            });
        });
    if busy {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    apply_bar(&ctx, cx, st, out, Some(&resp));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::camera::render_lights;
    use crate::plan_defaults;
    use plan_3d::Vertex;
    use plan_core::WallKind;

    fn project_with_wall() -> Project {
        let mut p = Project::new("t");
        p.add_wall(
            0,
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        p
    }

    fn quad_at(plan_y: f32) -> Mesh {
        // A small wall-like quad at a plan Y (scene Z = -y).
        let v = |x, y| Vertex {
            position: [x, y, -plan_y],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
        };
        Mesh {
            vertices: vec![v(0.0, 0.0), v(10.0, 0.0), v(10.0, 10.0), v(0.0, 10.0)],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: Material::WallExterior,
            object_id: None,
            color: None,
        }
    }

    #[test]
    fn roofs_are_part_of_the_view_scene_and_the_hash() {
        use crate::editor::roof_view::{rebuild, RoofSettings};
        let mut p = Project::new("roof");
        let (w, d) = (480.0, 288.0);
        let corners = [
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, d),
            Point::new(0.0, d),
        ];
        for i in 0..4 {
            p.add_wall(
                0,
                corners[i],
                corners[(i + 1) % 4],
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        let before = project_hash(&p);
        let plain = build_view_scene(&p, &ViewScope::default());
        assert!(plain.meshes.iter().all(|m| m.material != Material::Roof));
        let defaults = plan_defaults::embedded();
        rebuild(&mut p, 0, RoofSettings::from_defaults(&defaults), false).unwrap();
        assert_ne!(before, project_hash(&p), "a new roof rebuilds the 3D view");
        let roofed = build_view_scene(&p, &ViewScope::default());
        assert!(roofed.meshes.iter().any(|m| m.material == Material::Roof));
    }

    #[test]
    fn a_placed_slab_joins_the_3d_scene_and_the_hash() {
        use plan_core::foundation::{rect_outline, FoundationLayer, Slab};
        let mut p = project_with_wall();
        let before = project_hash(&p);
        let plain = build_view_scene(&p, &ViewScope::default());
        let mut layer = FoundationLayer::default();
        layer.slabs.push(Slab::new(
            900,
            rect_outline(Point::new(0.0, 0.0), Point::new(240.0, 180.0)),
        ));
        layer
            .pads
            .push(plan_core::foundation::Pad::new(901, Point::new(300.0, 0.0)));
        layer.store(&mut p.floors[0]);
        assert_ne!(before, project_hash(&p), "a slab rebuilds the 3D view");
        let with_slab = build_view_scene(&p, &ViewScope::default());
        assert!(with_slab.meshes.len() > plain.meshes.len());
        assert!(with_slab
            .meshes
            .iter()
            .any(|m| m.material == Material::Concrete && m.object_id == Some(900)));
        // Editing the slab changes the hash again.
        let h = project_hash(&p);
        layer.slabs[0].thickness = 8.0;
        layer.store(&mut p.floors[0]);
        assert_ne!(h, project_hash(&p));
    }

    #[test]
    fn scene_hash_changes_when_a_wall_moves() {
        let mut p = project_with_wall();
        let h = project_hash(&p);
        assert_eq!(h, project_hash(&p), "the hash is stable");
        p.floors[0].walls[0].end = Point::new(300.0, 0.0);
        let moved = project_hash(&p);
        assert_ne!(h, moved);
        // Opening hosted on the wall changes it too; cameras do not.
        let wall = p.floors[0].walls[0].id;
        p.add_camera(CameraObject::new(
            CameraKind::FullCamera,
            Point::ZERO,
            0.0,
            "c",
            0,
        ));
        assert_eq!(moved, project_hash(&p));
        let _ = p.add_opening(0, wall, 100.0, plan_core::OpeningKind::Door);
        assert_ne!(moved, project_hash(&p));
    }

    #[test]
    fn signature_follows_scope_and_section() {
        let p = project_with_wall();
        let mut st = View3dState::with_inbox(Outbox::default());
        let base = st.signature(&p);
        st.scope_floor = Some(0);
        let scoped = st.signature(&p);
        assert_ne!(base, scoped);
        st.section = Some(SectionCut {
            origin: Point::ZERO,
            dir: Point::new(0.0, 1.0),
            half_width: 50.0,
            back_clip: None,
        });
        assert_ne!(scoped, st.signature(&p));
        // Fill technique changes the scene, plain ones do not.
        let before = st.signature(&p);
        st.technique = RenderingTechnique::VectorView;
        assert_eq!(before, st.signature(&p));
        st.technique = RenderingTechnique::Clay;
        assert_ne!(before, st.signature(&p));
    }

    #[test]
    fn ensure_scene_rebuilds_only_when_the_plan_changes() {
        let mut p = project_with_wall();
        let mut st = View3dState::with_inbox(Outbox::default());
        st.ensure_scene(&p);
        let vp = st.viewport.as_ref().unwrap();
        assert!(vp.bounds().is_some());
        assert!(!st.scene_dirty);
        let first = st.last_project_hash;
        st.ensure_scene(&p);
        assert_eq!(st.last_project_hash, first);
        p.floors[0].walls[0].end = Point::new(480.0, 0.0);
        st.ensure_scene(&p);
        assert_ne!(st.last_project_hash, first);
        let w = st.viewport.as_ref().unwrap().bounds().unwrap();
        assert!(w.1[0] > 400.0, "new wall length is in the scene");
        st.rebuild();
        assert!(st.scene_dirty);
        st.ensure_scene(&p);
        assert!(!st.scene_dirty);
    }

    #[test]
    fn technique_mapping() {
        let std = technique_view(RenderingTechnique::Standard);
        assert!(std.show_edges && !std.flat && std.fill.is_none());
        let vec = technique_view(RenderingTechnique::VectorView);
        assert!(vec.show_edges && vec.flat && vec.fill.is_none());
        let line = technique_view(RenderingTechnique::LineDrawing);
        assert!(line.show_edges && line.flat && line.fill.is_some());
        assert!(line.background[0] > 0.95);
        let glass = technique_view(RenderingTechnique::GlassHouse);
        assert_eq!(glass.fill, Some(Material::WindowGlass));
        let clay = technique_view(RenderingTechnique::Clay);
        assert!(!clay.flat && !clay.show_edges);
        let fill = clay.fill.expect("clay is one material");
        assert_ne!(fill, Material::WindowGlass);
        let duo = technique_view(RenderingTechnique::Duotone);
        assert!(duo.fill.is_some());
        for t in RenderingTechnique::ALL {
            let v = technique_view(t);
            assert!(v.background.iter().all(|c| (0.0..=1.0).contains(c)));
        }
    }

    #[test]
    fn every_technique_has_its_own_gl_look() {
        let look = |t| technique_view(t).look;
        assert_eq!(look(RenderingTechnique::Standard), Look::Standard);
        assert_eq!(look(RenderingTechnique::PhysicallyBased), Look::Physical);
        assert_eq!(look(RenderingTechnique::Clay), Look::Clay);
        assert_eq!(look(RenderingTechnique::GlassHouse), Look::GlassHouse);
        assert_eq!(look(RenderingTechnique::Watercolor), Look::Watercolor);
        assert_eq!(
            look(RenderingTechnique::TechnicalIllustration),
            Look::Technical
        );
        assert_eq!(look(RenderingTechnique::Duotone), Look::Duotone);
        assert_eq!(look(RenderingTechnique::LineDrawing), Look::Flat);
        // The looks that draw edge lines in the composite pass.
        assert!(
            look(RenderingTechnique::TechnicalIllustration)
                .params()
                .edge_lines
                > 0.0
        );
        assert!(look(RenderingTechnique::Standard).params().sky);
        let mut vp = Viewport3d::new();
        apply_to_viewport(
            &mut vp,
            &technique_view(RenderingTechnique::Watercolor),
            None,
        );
        assert_eq!(vp.look, Look::Watercolor);
        assert!(vp.settings.shadows && vp.settings.ambient_occlusion);
    }

    #[test]
    fn nearest_material_matches_colours() {
        assert_eq!(nearest_material([247, 247, 245]), Material::Ceiling);
        assert_eq!(nearest_material([140, 99, 64]), Material::Floor);
    }

    #[test]
    fn fill_override_makes_one_material() {
        let p = project_with_wall();
        let plain = build_view_scene(&p, &ViewScope::default());
        let kinds: std::collections::HashSet<_> = plain.meshes.iter().map(|m| m.material).collect();
        assert!(kinds.len() > 1);
        let clay = build_view_scene(
            &p,
            &ViewScope {
                fill: Some(Material::WallInterior),
                ..ViewScope::default()
            },
        );
        assert!(clay
            .meshes
            .iter()
            .all(|m| m.material == Material::WallInterior));
    }

    #[test]
    fn floor_overview_leaves_out_the_floors_above() {
        let mut p = project_with_wall();
        let upper = p.build_new_floor(false);
        p.add_wall(
            upper,
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let all = build_view_scene(&p, &ViewScope::default());
        let floor0 = build_view_scene(
            &p,
            &ViewScope {
                floor: Some(0),
                ..ViewScope::default()
            },
        );
        assert!(floor0.triangle_count() < all.triangle_count());
        let (_, hi_all) = all.bounds().unwrap();
        let (_, hi_0) = floor0.bounds().unwrap();
        assert!(hi_0[1] < hi_all[1]);
    }

    #[test]
    fn section_keeps_what_is_beyond_the_cut() {
        // Cut at plan y = 100 looking north (+Y): keep y >= 100, drop y < 100.
        let scene = Scene {
            meshes: vec![quad_at(20.0), quad_at(200.0), quad_at(100.0)],
        };
        let cut = SectionCut {
            origin: Point::new(5.0, 100.0),
            dir: Point::new(0.0, 1.0),
            half_width: 100.0,
            back_clip: None,
        };
        let clipped = clip_scene(&scene, &cut);
        assert_eq!(clipped.meshes.len(), 2, "the near quad is removed");
        let ys: Vec<f32> = clipped
            .meshes
            .iter()
            .map(|m| -m.vertices[0].position[2])
            .collect();
        assert!(ys.contains(&200.0) && ys.contains(&100.0));
        // Back clip drops what is further than 50" behind the cut.
        let clipped = clip_scene(
            &scene,
            &SectionCut {
                back_clip: Some(50.0),
                ..cut
            },
        );
        assert_eq!(clipped.meshes.len(), 1);
        // Outside the cut line's ends nothing is kept.
        let narrow = SectionCut {
            half_width: 1.0,
            origin: Point::new(500.0, 100.0),
            ..cut
        };
        assert!(clip_scene(&scene, &narrow).meshes.is_empty());
    }

    #[test]
    fn section_direction_selects_the_elevation() {
        let at = |x: f64, y: f64| SectionCut {
            origin: Point::ZERO,
            dir: Point::new(x, y),
            half_width: 10.0,
            back_clip: None,
        };
        assert_eq!(at(0.0, 1.0).elevation_mode(), CameraMode::ElevationFront);
        assert_eq!(at(-1.0, 0.0).elevation_mode(), CameraMode::ElevationRight);
        assert_eq!(at(0.0, -1.0).elevation_mode(), CameraMode::ElevationBack);
        assert_eq!(at(1.0, 0.0).elevation_mode(), CameraMode::ElevationLeft);
        // 20 degrees off north is still the front elevation.
        let d = 70.0_f64.to_radians();
        assert_eq!(
            at(d.cos(), d.sin()).elevation_mode(),
            CameraMode::ElevationFront
        );
        assert!(at(0.0, 1.0).yaw().abs() < 1e-6);
    }

    #[test]
    fn full_camera_aims_the_viewport() {
        let mut p = project_with_wall();
        let mut cam = CameraObject::new(
            CameraKind::FullCamera,
            Point::new(120.0, 60.0),
            90.0,
            "Camera 1",
            0,
        );
        cam.eye_height = 60.0;
        let id = p.add_camera(cam);
        let mut st = View3dState::with_inbox(Outbox::default());
        st.show_camera(&p, id);
        assert!(st.active && st.mode == CameraMode::FullCamera);
        st.ensure_scene(&p);
        st.apply_setup(4.0 / 3.0);
        let c = &st.viewport.as_ref().unwrap().camera;
        assert_eq!(c.mode, CameraMode::FullCamera);
        assert_eq!(c.position, [120.0, 60.0, -60.0]);
        // Facing plan north is scene -Z.
        let f = c.forward();
        assert!(f[2] < -0.99 && f[0].abs() < 1e-4);
        assert!((c.fov_deg - vertical_fov(55.0, 4.0 / 3.0)).abs() < 1e-4);
        // East-facing camera looks along +X.
        p.update_camera(id, |c| c.direction_deg = 0.0);
        st.show_camera(&p, id);
        st.ensure_scene(&p);
        st.apply_setup(1.0);
        assert!(st.viewport.as_ref().unwrap().camera.forward()[0] > 0.99);
    }

    #[test]
    fn cross_section_camera_clips_and_looks_the_right_way() {
        let mut p = project_with_wall();
        // West to east along y = -50, looking north into the wall at y = 0.
        let cam = CameraObject {
            fov_deg: 400.0,
            ..CameraObject::new(
                CameraKind::CrossSection { back_clip: None },
                Point::new(120.0, -50.0),
                90.0,
                "Section 1",
                0,
            )
        };
        let id = p.add_camera(cam);
        let mut st = View3dState::with_inbox(Outbox::default());
        st.show_camera(&p, id);
        assert_eq!(st.mode, CameraMode::ElevationFront);
        st.ensure_scene(&p);
        st.apply_setup(1.5);
        let c = &st.viewport.as_ref().unwrap().camera;
        assert_eq!(c.mode, CameraMode::ElevationFront);
        assert!(c.forward()[2] < -0.99, "front elevation looks toward -Z");
        // A section on the far side of the wall removes it from the scene.
        p.update_camera(id, |c| c.position = Point::new(120.0, 50.0));
        st.show_camera(&p, id);
        st.ensure_scene(&p);
        assert!(st.scene_empty, "the wall is in front of the cut");
    }

    #[test]
    fn commands_switch_views_and_tools() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut tools = ToolSet::new();
        let mut st = View3dState::with_inbox(Outbox::default());
        dispatch(
            View3dCommand::Mode(CameraMode::DollHouse),
            &mut cx,
            &mut tools,
            &mut st,
        );
        assert!(st.active && st.mode == CameraMode::DollHouse);
        dispatch(View3dCommand::FloorOverview, &mut cx, &mut tools, &mut st);
        assert_eq!(st.scope_floor, Some(0));
        assert_eq!(st.mode, CameraMode::Orbit);
        dispatch(
            View3dCommand::Technique(RenderingTechnique::Clay),
            &mut cx,
            &mut tools,
            &mut st,
        );
        assert_eq!(st.technique, RenderingTechnique::Clay);
        dispatch(View3dCommand::RayTrace, &mut cx, &mut tools, &mut st);
        assert!(st.raytrace.open);
        // Placing a camera goes back to the plan with the Camera tool active.
        dispatch(
            View3dCommand::Tool(CameraVariant::FullCamera),
            &mut cx,
            &mut tools,
            &mut st,
        );
        assert!(!st.active);
        assert_eq!(
            tools.active_id(),
            ToolId::CameraVariant(CameraVariant::FullCamera)
        );
    }

    #[test]
    fn slider_cuts_progressively() {
        let p = project_with_wall();
        let mut st = View3dState::with_inbox(Outbox::default());
        st.ensure_scene(&p);
        st.toggle_slider();
        assert!(st.active && st.slider.is_some());
        let base = st.section.unwrap();
        assert_eq!(st.mode, CameraMode::ElevationFront);
        // Deep into the model everything lies in front of the cut.
        let mut deep = base;
        deep.origin = deep.origin + deep.dir * 1000.0;
        let scene = build_view_scene(
            &p,
            &ViewScope {
                section: Some(deep),
                ..ViewScope::default()
            },
        );
        assert!(scene.meshes.is_empty());
        st.toggle_slider();
        assert!(st.section.is_none() && st.slider.is_none());
    }

    #[test]
    fn vertical_fov_shrinks_with_aspect() {
        assert!((vertical_fov(90.0, 1.0) - 90.0).abs() < 1e-3);
        assert!(vertical_fov(90.0, 2.0) < 60.0);
        assert!(vertical_fov(1.0, 1.0) >= 10.0);
    }

    #[test]
    fn camera_defaults_roundtrip() {
        reset_camera_defaults();
        assert_eq!(camera_defaults().eye_height, 60.0);
        assert_eq!(camera_defaults().fov_deg, 55.0);
    }

    #[test]
    fn requests_open_and_refresh_the_camera_dialog() {
        let mut p = project_with_wall();
        let id = p.add_camera(CameraObject::new(
            CameraKind::FullCamera,
            Point::ZERO,
            0.0,
            "Camera 1",
            0,
        ));
        let outbox = Outbox::default();
        let mut st = View3dState::with_inbox(outbox.clone());
        outbox.post(ViewRequest::OpenCameraSpec(id));
        st.drain(&p);
        assert!(st.camera_dialog.is_some());
        assert!(!st.active, "the dialog opens in the plan");
        outbox.post(ViewRequest::CameraDeleted(id));
        st.drain(&p);
        assert!(st.active_camera.is_none());
    }

    #[test]
    fn placed_symbols_appear_in_the_scene_and_the_hash() {
        use plan_core::PlacedSymbol;
        let mut p = project_with_wall();
        let before_hash = project_hash(&p);
        let before = build_view_scene(&p, &ViewScope::default()).meshes.len();
        // A non-Chief library item: 24 x 30 x 40 box, 6 in off the floor.
        let mut s = PlacedSymbol::new(
            "core.plumbing.toilet_elongated",
            Point::new(100.0, 50.0),
            24.0,
            30.0,
            40.0,
        );
        s.elevation = 6.0;
        p.add_symbol(0, s);
        assert_ne!(
            before_hash,
            project_hash(&p),
            "a placed symbol rebuilds the 3D view"
        );
        let scene = build_view_scene(&p, &ViewScope::default());
        assert_eq!(scene.meshes.len(), before + 1);
        let id = p.floors[0].symbols[0].id;
        let boxed = scene
            .meshes
            .iter()
            .find(|m| m.object_id == Some(id))
            .expect("a mesh for the symbol");
        assert_eq!(boxed.material, Material::Trim);
        assert_eq!(boxed.triangle_count(), 12);
        let (lo, hi) = boxed.bounds().unwrap();
        // x 88..112, up 6..46, scene Z = -plan y: -80..-50 (front is +plan y).
        assert_eq!((lo, hi), ([88.0, 6.0, -80.0], [112.0, 46.0, -50.0]));
        // Every face points away from the box centre.
        let c = [100.0, 26.0, -65.0];
        for tri in boxed.indices.as_chunks::<3>().0 {
            let (a, b, d) = (
                boxed.vertices[tri[0] as usize],
                boxed.vertices[tri[1] as usize],
                boxed.vertices[tri[2] as usize],
            );
            let e1 = [
                b.position[0] - a.position[0],
                b.position[1] - a.position[1],
                b.position[2] - a.position[2],
            ];
            let e2 = [
                d.position[0] - a.position[0],
                d.position[1] - a.position[1],
                d.position[2] - a.position[2],
            ];
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let out: f32 = (0..3).map(|k| (a.position[k] - c[k]) * n[k]).sum();
            assert!(out > 0.0, "winding faces outward: {out}");
        }

        // Moving the symbol changes the hash, so the view rebuilds.
        let h = project_hash(&p);
        p.floors[0].symbols[0].position = Point::new(130.0, 50.0);
        assert_ne!(h, project_hash(&p));
        let moved = build_view_scene(&p, &ViewScope::default());
        let (lo, _) = moved
            .meshes
            .iter()
            .find(|m| m.object_id == Some(id))
            .unwrap()
            .bounds()
            .unwrap();
        assert_eq!(lo[0], 118.0);

        // The ensure_scene signature follows it too.
        let mut st = View3dState::with_inbox(Outbox::default());
        st.ensure_scene(&p);
        let first = st.last_project_hash;
        p.floors[0].symbols[0].width = 30.0;
        st.ensure_scene(&p);
        assert_ne!(st.last_project_hash, first);
    }

    #[test]
    fn a_chief_symbol_without_its_catalog_is_a_block() {
        use plan_core::PlacedSymbol;
        let mut p = project_with_wall();
        let before = build_view_scene(&p, &ViewScope::default()).meshes.len();
        p.add_symbol(
            0,
            PlacedSymbol::new(
                "chief.no-such-catalog.9",
                Point::new(10.0, 10.0),
                20.0,
                20.0,
                20.0,
            ),
        );
        assert_eq!(
            build_view_scene(&p, &ViewScope::default()).meshes.len(),
            before + 1
        );
    }

    // ----- vector elevations, walkthroughs, lights -----

    fn house() -> Project {
        let mut p = Project::new("House");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
        }
        p
    }

    fn vector_state(p: &mut Project, cam: CameraObject) -> (View3dState, Id) {
        let id = p.add_camera(cam);
        let mut st = View3dState::with_inbox(Outbox::default());
        st.technique = RenderingTechnique::VectorView;
        st.vector.raster_px = 192;
        st.show_camera(p, id);
        (st, id)
    }

    fn middle_section() -> CameraObject {
        // Through the middle of the house, looking up the plan (+Y).
        let mut c = CameraObject::new(
            CameraKind::CrossSection { back_clip: None },
            Point::new(120.0, 96.0),
            90.0,
            "Section 1",
            0,
        );
        crate::tools::camera::upgrade_section(&mut c);
        c
    }

    #[test]
    fn a_section_camera_in_vector_view_draws_cut_regions() {
        let mut p = house();
        let (mut st, id) = vector_state(&mut p, middle_section());
        assert_eq!(st.vector_camera(&p).map(|c| c.id), Some(id));
        st.refresh_vector_now(&p, None);
        let d = st.vector.drawing().expect("a drawing");
        assert!(!d.lines.is_empty());
        assert!(d.cut_regions().count() > 0, "the cut walls have poche");
        assert!(d.lines.iter().any(|l| l.kind == EdgeKind::Cut));
        // Standard shows the GL scene instead; Technical Illustration also draws vectors.
        st.technique = RenderingTechnique::Standard;
        assert!(st.vector_camera(&p).is_none());
        st.technique = RenderingTechnique::TechnicalIllustration;
        assert!(st.vector_camera(&p).is_some());
        // A full camera never draws a vector view.
        let full = p.add_camera(CameraObject::new(
            CameraKind::FullCamera,
            Point::new(10.0, 10.0),
            90.0,
            "c",
            0,
        ));
        st.show_camera(&p, full);
        assert!(st.vector_camera(&p).is_none());
    }

    #[test]
    fn the_vector_drawing_refreshes_when_the_project_or_the_sun_change() {
        let mut p = house();
        let (mut st, id) = vector_state(&mut p, middle_section());
        let cam = p.camera(id).unwrap().clone();
        let key = st.vector_key(&p, &cam, None);
        assert_eq!(
            key,
            st.vector_key(&p, &cam, None),
            "stable while nothing changes"
        );
        st.refresh_vector_now(&p, None);
        assert!(!st.vector.needs(key));
        // A moved wall changes the project hash, so a new drawing is needed.
        p.add_wall(
            0,
            Point::new(0.0, 96.0),
            Point::new(240.0, 96.0),
            4.0,
            109.0,
            WallKind::Interior,
        );
        let newer = st.vector_key(&p, &cam, None);
        assert_ne!(key, newer);
        assert!(st.vector.needs(newer));
        // So do the technique, the camera and the plan's sun.
        let sun = Some(SunDir {
            azimuth_deg: 200.0,
            altitude_deg: 40.0,
        });
        assert_ne!(newer, st.vector_key(&p, &cam, sun));
        let mut moved = cam.clone();
        moved.position.x += 12.0;
        assert_ne!(newer, st.vector_key(&p, &moved, None));
        st.technique = RenderingTechnique::TechnicalIllustration;
        assert_ne!(newer, st.vector_key(&p, &cam, None));
        // The worker thread delivers the new drawing.
        st.refresh_vector(&p, None);
        assert!(st.vector.is_busy() || st.vector.drawing().is_some());
        for _ in 0..1000 {
            st.refresh_vector(&p, None);
            if !st.vector.is_busy() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!st.vector.is_busy());
        let key = st.vector_key(&p, &cam, None);
        assert!(
            !st.vector.needs(key),
            "the finished drawing is the current one"
        );
    }

    #[test]
    fn technical_illustration_casts_shadows_from_the_camera_sun() {
        let mut p = house();
        let cam = middle_section();
        p.add_camera(cam.clone());
        let plain = vector_drawing(&p, &cam, RenderingTechnique::VectorView, None, 160);
        let tech = vector_drawing(
            &p,
            &cam,
            RenderingTechnique::TechnicalIllustration,
            None,
            160,
        );
        assert_eq!(plain.regions_of(RegionKind::Shadow).count(), 0);
        let lit = vector_drawing(
            &p,
            &cam,
            RenderingTechnique::VectorView,
            Some(SunDir {
                azimuth_deg: 90.0,
                altitude_deg: 25.0,
            }),
            160,
        );
        assert!(
            lit.regions_of(RegionKind::Shadow).count() > 0,
            "the plan's sun shades faces"
        );
        assert!(!tech.lines.is_empty());
    }

    #[test]
    fn trapezoids_fill_concave_rings_and_holes_by_even_odd() {
        let pts =
            |v: &[(f64, f64)]| -> Vec<Point> { v.iter().map(|&(x, y)| Point::new(x, y)).collect() };
        let l_shape = pts(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 4.0),
            (4.0, 4.0),
            (4.0, 10.0),
            (0.0, 10.0),
        ]);
        assert!((trapezoid_area(&trapezoids(&l_shape)) - 64.0).abs() < 1e-9);
        // A square with a square hole joined to the outside by a zero-width slit.
        let holed = pts(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 5.0),
            (4.0, 5.0),
            (4.0, 4.0),
            (6.0, 4.0),
            (6.0, 6.0),
            (4.0, 6.0),
            (4.0, 5.0),
            (0.0, 5.0),
        ]);
        assert!((trapezoid_area(&trapezoids(&holed)) - 96.0).abs() < 1e-9);
        // A sloped edge (a gable) is cut exactly.
        let gable = pts(&[(0.0, 0.0), (10.0, 0.0), (5.0, 5.0)]);
        assert!((trapezoid_area(&trapezoids(&gable)) - 25.0).abs() < 1e-9);
        assert!(trapezoids(&pts(&[(0.0, 0.0), (1.0, 1.0)])).is_empty());
    }

    #[test]
    fn the_vector_view_pans_and_zooms_about_the_cursor() {
        let mut p = house();
        let (mut st, _id) = vector_state(&mut p, middle_section());
        st.refresh_vector_now(&p, None);
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let before = st.vector.transform(rect).unwrap();
        // Fitted: the drawing sits inside the view.
        let d = st.vector.drawing().unwrap();
        let (lo, hi) = drawing_extent(d);
        for pt in [lo, hi] {
            assert!(rect.contains(before.to_screen(pt)), "{pt:?}");
        }
        // Zooming about a point keeps that point of the drawing under the cursor.
        let at = egui::pos2(300.0, 200.0);
        let anchor = before.to_drawing(at);
        st.vector.zoom_about(rect, at, 2.0);
        let after = st.vector.transform(rect).unwrap();
        let moved = after.to_screen(anchor);
        assert!((moved - at).length() < 0.01, "{moved:?} vs {at:?}");
        assert!(after.scale > before.scale * 1.99);
        st.vector.pan += egui::vec2(15.0, -5.0);
        assert!(st.vector.transform(rect).unwrap().origin != after.origin);
        st.vector.reset_view();
        assert_eq!(st.vector.transform(rect).unwrap().origin, before.origin);
    }

    fn walk_camera() -> CameraObject {
        let mut c = CameraObject::walkthrough(
            vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)],
            66.0,
            "Walk",
            0,
        );
        c.walk_speed = 50.0;
        c
    }

    #[test]
    fn a_walkthrough_plays_along_its_path_at_its_speed() {
        let mut p = house();
        p.floors[0].elevation = 10.0;
        let id = p.add_camera(walk_camera());
        let mut st = View3dState::with_inbox(Outbox::default());
        st.show_camera(&p, id);
        assert!(st.active && st.mode == CameraMode::FullCamera);
        let w = st.walk.expect("the walkthrough is ready to play");
        assert_eq!((w.camera, w.t_s, w.playing), (id, 0.0, false));
        let c = p.camera(id).unwrap();
        // Two seconds long; at one second the eye is at the midpoint.
        assert!((c.walk_duration_s() - 2.0).abs() < 1e-9);
        let ([x, y, z], yaw) = walk_view(&p, c, 1.0);
        assert!((x - 50.0).abs() < 1e-4 && (y - 76.0).abs() < 1e-4 && z.abs() < 1e-4);
        assert!((yaw + FRAC_PI_2).abs() < 1e-5, "looks along +X");
        // The player advances with time and stops at the end.
        st.walk.as_mut().unwrap().playing = true;
        let (pos, _) = st.tick_walk(&p, 0.5).unwrap();
        assert!((pos[0] - 25.0).abs() < 1e-3);
        assert!(st.walk.unwrap().playing);
        let (pos, _) = st.tick_walk(&p, 10.0).unwrap();
        assert!((pos[0] - 100.0).abs() < 1e-3);
        let w = st.walk.unwrap();
        assert!(!w.playing && (w.t_s - 2.0).abs() < 1e-9);
        // The Play command rewinds and starts.
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        st.play_walkthrough(&mut cx);
        let w = st.walk.unwrap();
        assert!(w.playing && w.t_s == 0.0);
        // Leaving for an overview drops the walkthrough.
        st.open_mode(CameraMode::Orbit, None);
        assert!(st.walk.is_none());
    }

    #[test]
    fn recording_writes_a_numbered_png_sequence_and_a_video_script() {
        let mut p = house();
        p.floors[0].elevation = 0.0;
        let mut cam = walk_camera();
        cam.path = vec![Point::new(60.0, 40.0), Point::new(180.0, 40.0)];
        cam.position = cam.path[0];
        cam.direction_deg = 0.0;
        cam.walk_speed = 120.0; // one second: 12 frames at the default rate
        let scene = build_view_scene(&p, &ViewScope::default());
        let dir = std::env::temp_dir().join(format!("plan_walk_frames_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let settings = plan_render::RenderSettings {
            width: 24,
            height: 18,
            samples: 1,
            ..plan_render::RenderSettings::default()
        };
        let env = plan_render::Environment::default();
        let mut seen = Vec::new();
        let n = record_walkthrough(
            &scene,
            &cam,
            0.0,
            &[],
            &env,
            &settings,
            4.0,
            &dir,
            &mut |d, t| {
                seen.push((d, t));
                true
            },
        )
        .unwrap();
        assert_eq!(n, 4, "one second at 4 fps");
        assert_eq!(seen, vec![(1, 4), (2, 4), (3, 4), (4, 4)]);
        for i in 1..=4 {
            let bytes = std::fs::read(dir.join(format!("frame_{i:04}.png"))).unwrap();
            assert!(bytes.starts_with(b"\x89PNG"), "frame {i}");
        }
        assert!(dir.join("make_video.sh").exists());
        // Stopping early keeps what was written.
        let n = record_walkthrough(
            &scene,
            &cam,
            0.0,
            &[],
            &env,
            &settings,
            4.0,
            &dir,
            &mut |d, _| d < 2,
        )
        .unwrap();
        assert_eq!(n, 2);
        // The first and last frames are the path's ends.
        assert_eq!(walk_frame_count(&cam, 4.0), 4);
        assert!((cam.walk_pose(0.0).position.x - 60.0).abs() < 1e-9);
        assert!((cam.walk_pose(1.0).position.x - 180.0).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_recording_thread_reports_progress_and_finishes() {
        let p = house();
        let mut cam = walk_camera();
        cam.path = vec![Point::new(60.0, 40.0), Point::new(72.0, 40.0)];
        cam.walk_speed = 48.0; // a quarter second: 3 frames
        cam.view.walk.format = plan_core::camera_view::RecordFormat::Frames;
        let dir = std::env::temp_dir().join(format!("plan_walk_thread_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let rec = Recording::start(&p, &cam, None, dir.clone());
        assert_eq!(rec.progress().1, 3);
        let mut result = None;
        for _ in 0..3000 {
            result = rec.finished();
            if result.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(result, Some(Ok(3)));
        assert_eq!(rec.progress(), (3, 3));
        assert!(dir.join("frame_0003.png").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_plan_sun_is_only_used_while_sun_angle_is_on() {
        let st = View3dState::with_inbox(Outbox::default());
        let project = Project::from_defaults("Sun", &crate::plan_defaults::embedded());
        assert!(st.plan_sun(&project, false).is_none());
        let s = st
            .plan_sun(&project, true)
            .expect("mid-afternoon in June is above the horizon");
        assert!(s.altitude_deg > 10.0);
    }

    #[test]
    fn lights_do_not_rebuild_the_3d_model() {
        let mut p = house();
        let before = project_hash(&p);
        p.add_light(
            0,
            plan_core::camera::PlanLight::new(Point::new(50.0, 50.0), 84.0),
        )
        .unwrap();
        assert_eq!(before, project_hash(&p));
    }

    fn run_frames(cx: &mut EditorContext, st: &mut View3dState, n: usize) {
        let ctx = egui::Context::default();
        for _ in 0..n {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                if st.frame(ctx, cx) {
                    egui::CentralPanel::default().show(ctx, |ui| show(ui, cx, st));
                }
            });
        }
    }

    #[test]
    fn the_vector_view_and_the_light_and_sun_windows_draw_frames() {
        let mut p = house();
        let (mut st, id) = vector_state(&mut p, middle_section());
        p.cameras[0].render.hatch = true;
        p.add_light(
            0,
            plan_core::camera::PlanLight::new(Point::new(50.0, 50.0), 84.0),
        )
        .unwrap();
        st.refresh_vector_now(&p, None);
        assert!(st.vector.drawing().is_some());
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        cx.view_flags.insert(ViewFlag::SunAngle);
        st.open_adjust_lights(&cx.project, None);
        st.active = true;
        run_frames(&mut cx, &mut st, 3);
        assert!(st.active && st.active_camera == Some(id));
        // The drawing is painted: regions, lines and labels.
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(640.0, 480.0));
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                paint_vector(
                    ui.painter(),
                    rect,
                    &st.vector,
                    RenderingTechnique::VectorView,
                );
                paint_vector(
                    ui.painter(),
                    rect,
                    &st.vector,
                    RenderingTechnique::TechnicalIllustration,
                );
            });
        });
        // Apply from the Sun Angle window makes every elevation cast shadows.
        assert!(!cx.project.cameras[0].render.shadows);
    }

    #[test]
    fn walkthrough_controls_draw_frames_in_the_3d_view() {
        let mut p = house();
        let id = p.add_camera(walk_camera());
        let mut st = View3dState::with_inbox(Outbox::default());
        st.show_camera(&p, id);
        st.walk.as_mut().unwrap().playing = true;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        run_frames(&mut cx, &mut st, 2);
        let w = st.walk.expect("still a walkthrough");
        assert!(w.t_s > 0.0 || !w.playing);
    }

    // ----- clicking in the 3D view -----

    /// Runs one frame of the 3D view on an 800x600 screen with `events`.
    fn frame_with(
        ctx: &egui::Context,
        cx: &mut EditorContext,
        st: &mut View3dState,
        events: Vec<egui::Event>,
        time: f64,
    ) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            time: Some(time),
            ..egui::RawInput::default()
        };
        let _ = ctx.run(raw, |ctx| {
            if st.frame(ctx, cx) {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| show(ui, cx, st));
            }
        });
    }

    fn press(pos: egui::Pos2, down: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: down,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// The pixel of a scene point in the 800x600 view of `st`.
    fn pixel(st: &View3dState, p: [f32; 3]) -> egui::Pos2 {
        let cam = &st.viewport.as_ref().unwrap().camera;
        let ndc = plan_view3d::math::transform_point(&cam.view_projection(800.0 / 600.0), p);
        egui::pos2((ndc[0] + 1.0) * 400.0, (1.0 - ndc[1]) * 300.0)
    }

    /// A click (move, press, release on separate frames); `t` is the time.
    fn click(
        ctx: &egui::Context,
        cx: &mut EditorContext,
        st: &mut View3dState,
        at: egui::Pos2,
        t: f64,
    ) {
        frame_with(ctx, cx, st, vec![egui::Event::PointerMoved(at)], t);
        frame_with(ctx, cx, st, vec![press(at, true)], t + 0.01);
        frame_with(ctx, cx, st, vec![press(at, false)], t + 0.02);
    }

    fn view_of_a_wall() -> (EditorContext, View3dState, egui::Context, Id, egui::Pos2) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project_with_wall();
        let wall = cx.project.floors[0].walls[0].id;
        let mut st = View3dState::with_inbox(Outbox::default());
        st.active = true;
        let ctx = egui::Context::default();
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 0.0);
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 0.1);
        // The wall's top face is up in the overview.
        let on_wall = pixel(&st, [120.0, 96.0, 0.0]);
        (cx, st, ctx, wall, on_wall)
    }

    #[test]
    fn a_click_in_the_3d_view_selects_the_object_and_tints_it() {
        let (mut cx, mut st, ctx, wall, on_wall) = view_of_a_wall();
        assert!(cx.selection.is_empty());
        click(&ctx, &mut cx, &mut st, on_wall, 1.0);
        assert_eq!(
            cx.selection.single(),
            Some(crate::editor::ObjectRef::Wall(wall))
        );
        // The next frames show the selection tint with the model.
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 1.2);
        assert_ne!(st.overlay_key, pick::empty_overlay_key());
        // A click on empty space clears the selection and the tint.
        click(&ctx, &mut cx, &mut st, egui::pos2(5.0, 5.0), 2.0);
        assert!(cx.selection.is_empty());
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 2.2);
        assert_eq!(st.overlay_key, pick::empty_overlay_key());
    }

    #[test]
    fn a_double_click_in_the_3d_view_asks_for_the_specification() {
        let (mut cx, mut st, ctx, wall, on_wall) = view_of_a_wall();
        cx.requests.clear();
        click(&ctx, &mut cx, &mut st, on_wall, 1.0);
        click(&ctx, &mut cx, &mut st, on_wall, 1.1);
        assert!(
            cx.requests.iter().any(|r| matches!(
                r,
                crate::editor::EditorRequest::OpenSpec(crate::editor::ObjectRef::Wall(i)) if *i == wall
            )),
            "{:?}",
            cx.requests
        );
        assert_eq!(
            cx.selection.single(),
            Some(crate::editor::ObjectRef::Wall(wall))
        );
    }

    #[test]
    fn delete_in_the_3d_view_deletes_the_selection_and_undo_restores_it() {
        let (mut cx, mut st, ctx, wall, on_wall) = view_of_a_wall();
        click(&ctx, &mut cx, &mut st, on_wall, 1.0);
        assert_eq!(cx.selection.len(), 1);
        let key = |k| egui::Event::Key {
            key: k,
            physical_key: Some(k),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        // The pointer is over the view; Delete removes the wall.
        frame_with(&ctx, &mut cx, &mut st, vec![key(egui::Key::Delete)], 3.0);
        assert!(cx.project.floors[0].wall(wall).is_none());
        assert!(cx.selection.is_empty());
        // The view rebuilds without it.
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 3.1);
        assert!(st
            .base_scene
            .meshes
            .iter()
            .all(|m| m.object_id != Some(wall)));
        cx.undo();
        assert!(cx.project.floors[0].wall(wall).is_some());
    }

    // ----- pictures and billboards in the view -----

    fn house_with_a_billboard() -> (Project, Id) {
        let mut p = house();
        let mut spec = plan_core::ImageSpec::new("tree.png", 10, 10);
        spec.color = [100, 120, 90];
        let id = p.add_symbol(
            0,
            PlacedSymbol::billboard(spec, Point::new(400.0, 100.0), 40.0, 72.0),
        );
        (p, id)
    }

    #[test]
    fn pictures_stay_out_of_the_cached_scene_and_are_added_per_view() {
        let (p, pic) = house_with_a_billboard();
        let mut st = View3dState::with_inbox(Outbox::default());
        st.ensure_scene(&p);
        assert!(
            st.base_scene
                .meshes
                .iter()
                .all(|m| m.object_id != Some(pic)),
            "the cached scene holds solids only"
        );
        assert!(!st.base_scene.meshes.is_empty());
        // Exports and the ray tracer still get them (stored angle).
        let full = build_view_scene(&p, &ViewScope::default());
        assert!(full.meshes.iter().any(|m| m.object_id == Some(pic)));
        // The panel adds them next, and the bare model is not what it holds.
        assert_eq!(st.overlay_key, pick::empty_overlay_key());
        st.refresh_overlay(&p, &Selection::default());
        assert_ne!(st.overlay_key, pick::empty_overlay_key());
    }

    #[test]
    fn the_billboard_follows_the_orbiting_camera_without_rebuilding_the_model() {
        let (p, _) = house_with_a_billboard();
        let mut st = View3dState::with_inbox(Outbox::default());
        st.ensure_scene(&p);
        st.refresh_overlay(&p, &Selection::default());
        let hash = st.last_project_hash;
        let first = st.overlay_key;
        // The same view again: nothing to upload.
        st.refresh_overlay(&p, &Selection::default());
        assert_eq!(st.overlay_key, first);
        // Orbit a quarter turn: the billboard turns, the model is untouched.
        st.viewport.as_mut().unwrap().camera.yaw += FRAC_PI_2;
        st.refresh_overlay(&p, &Selection::default());
        assert_ne!(st.overlay_key, first);
        st.ensure_scene(&p);
        assert_eq!(st.last_project_hash, hash);
        assert!(!st.scene_dirty);
        // The user's view survives the re-queue.
        let yaw = st.viewport.as_ref().unwrap().camera.yaw;
        st.viewport.as_mut().unwrap().camera.yaw += 0.3;
        st.refresh_overlay(&p, &Selection::default());
        assert!((st.viewport.as_ref().unwrap().camera.yaw - yaw - 0.3).abs() < 1e-5);
    }

    #[test]
    fn the_vector_elevation_ignores_billboards() {
        let mut p = house();
        let (with, _) = vector_state(&mut p, middle_section());
        let without = vector_drawing(
            &p,
            with.vector_camera(&p).unwrap(),
            RenderingTechnique::VectorView,
            None,
            256,
        );
        let mut spec = plan_core::ImageSpec::new("tree.png", 10, 10);
        spec.color = [100, 120, 90];
        p.add_symbol(
            0,
            PlacedSymbol::billboard(spec, Point::new(100.0, 100.0), 80.0, 90.0),
        );
        let cam = with.vector_camera(&p).unwrap().clone();
        let with_billboard = vector_drawing(&p, &cam, RenderingTechnique::VectorView, None, 256);
        assert_eq!(without, with_billboard);
    }

    #[test]
    fn a_click_on_a_terrain_wall_in_the_3d_view_selects_the_terrain_element() {
        use crate::editor::site_view::{self, TerrainHit};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project_with_wall();
        let mut rec = site_view::TerrainRecord::new();
        rec.terrain.walls.push(plan_terrain::TerrainWall::new(
            plan_terrain::WallKind::Wall,
            vec![Point::new(0.0, -300.0), Point::new(300.0, -300.0)],
            false,
        ));
        site_view::save_terrain(&mut cx.project, &rec);
        let mut st = View3dState::with_inbox(Outbox::default());
        st.active = true;
        let ctx = egui::Context::default();
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 0.0);
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 0.1);
        let id = site_view::hit_mesh_id(TerrainHit::Wall(0));
        let mesh = st
            .base_scene
            .meshes
            .iter()
            .find(|m| m.object_id == Some(id))
            .expect("the terrain wall is in the scene");
        let (lo, hi) = mesh.bounds().unwrap();
        let top = pixel(&st, [(lo[0] + hi[0]) / 2.0, hi[1], (lo[2] + hi[2]) / 2.0]);
        click(&ctx, &mut cx, &mut st, top, 1.0);
        assert_eq!(
            cx.selection.single(),
            Some(crate::editor::ObjectRef::TerrainObject(TerrainHit::Wall(0)))
        );
        // It is tinted, and Delete removes it as a terrain edit.
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 1.2);
        assert_ne!(st.overlay_key, pick::empty_overlay_key());
        let delete = egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: Some(egui::Key::Delete),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame_with(&ctx, &mut cx, &mut st, vec![delete], 2.0);
        assert!(site_view::load_terrain(&cx.project)
            .unwrap()
            .terrain
            .walls
            .is_empty());
        cx.undo();
        assert_eq!(
            site_view::load_terrain(&cx.project)
                .unwrap()
                .terrain
                .walls
                .len(),
            1
        );
    }

    #[test]
    fn print_image_of_the_3d_view_renders_the_viewport_camera_as_a_png() {
        let p = house();
        let mut st = View3dState::default();
        // No viewport yet: nothing to print.
        assert!(st.snapshot_png(32, 24).is_none());
        assert!(st.snapshot_source().is_none());
        // A view with a model: the snapshot keeps scene and camera.
        st.viewport = Some(Viewport3d::new());
        assert!(
            st.snapshot_png(32, 24).is_none(),
            "an empty scene prints nothing"
        );
        st.base_scene = build_view_scene(&p, &ViewScope::default());
        assert!(!st.base_scene.meshes.is_empty());
        let snap = st.snapshot_source().expect("a view with a model");
        let png = snap.render_png(48, 36, 1).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // The size is what was asked (IHDR width and height), clamped to the limits.
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 48);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 36);
        let small = snap.render_png(1, 1, 1).unwrap();
        assert_eq!(u32::from_be_bytes(small[16..20].try_into().unwrap()), 16);
        // snapshot_png is the same render at the default quality.
        let direct = st.snapshot_png(32, 24).unwrap();
        assert_eq!(&direct[..4], b"\x89PNG");
    }

    #[test]
    fn the_slider_drives_the_vector_view_live() {
        let p = house();
        let mut st = View3dState::with_inbox(Outbox::default());
        st.technique = RenderingTechnique::VectorView;
        st.vector.raster_px = 192;
        st.ensure_scene(&p);
        assert!(st.vector_source(&p).is_none());
        st.toggle_slider();
        // No camera object, but the slider is drawn as a cross section.
        assert!(st.vector_camera(&p).is_none());
        let cam = st.vector_source(&p).expect("the slider's cut");
        assert!(matches!(cam.kind, CameraKind::CrossSection { .. }));
        st.refresh_vector_now(&p, None);
        let start = st.vector.drawing().cloned().expect("a drawing");
        assert_eq!(
            start.cut_regions().count(),
            0,
            "the cut is in front of the house"
        );
        // Moving the slider through the house cuts the side walls.
        let (base, depth) = {
            let s = st.slider.as_ref().unwrap();
            (s.base, s.depth)
        };
        let off = depth * 0.5;
        st.slider.as_mut().unwrap().offset = off;
        let mut cut = base;
        cut.origin = cut.origin + cut.dir * off;
        st.section = Some(cut);
        st.refresh_vector_now(&p, None);
        let mid = st.vector.drawing().cloned().expect("a drawing");
        assert!(mid.cut_regions().count() > 0, "the side walls have poche");
        assert_ne!(mid, start);
        // The standard technique shows the GL scene again.
        st.technique = RenderingTechnique::Standard;
        assert!(st.vector_source(&p).is_none());
    }

    // ----- moving objects, the orbit centre, hover, the overlay (C-39, C-43) -----

    /// Like `frame_with` with keyboard modifiers held.
    fn frame_mods(
        ctx: &egui::Context,
        cx: &mut EditorContext,
        st: &mut View3dState,
        events: Vec<egui::Event>,
        time: f64,
        modifiers: egui::Modifiers,
    ) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            time: Some(time),
            modifiers,
            ..egui::RawInput::default()
        };
        let _ = ctx.run(raw, |ctx| {
            if st.frame(ctx, cx) {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| show(ui, cx, st));
            }
        });
    }

    /// A 24" base cabinet selected in an empty plan, seen in the overview.
    fn view_of_a_cabinet() -> (EditorContext, View3dState, egui::Context, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = crate::editor::placed::add_cabinet(
            &mut cx.project,
            0,
            plan_cabinets::Cabinet::base(24.0),
        )
        .unwrap();
        cx.selection.set(crate::editor::ObjectRef::Cabinet(id));
        let mut st = View3dState::with_inbox(Outbox::default());
        st.active = true;
        let ctx = egui::Context::default();
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 0.0);
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 0.1);
        (cx, st, ctx, id)
    }

    /// The scene bounds of the meshes of `id`.
    fn bounds_of(st: &View3dState, id: Id) -> ([f32; 3], [f32; 3]) {
        let scene = Scene {
            meshes: st
                .base_scene
                .meshes
                .iter()
                .filter(|m| m.object_id == Some(id))
                .cloned()
                .collect(),
        };
        scene.bounds().expect("the object has meshes")
    }

    #[test]
    fn dragging_a_selected_cabinet_in_3d_moves_it_along_the_floor_in_one_undo_step() {
        let (mut cx, mut st, ctx, id) = view_of_a_cabinet();
        let before = placed_cabinet(&cx, id);
        let (lo, hi) = bounds_of(&st, id);
        let top = [(lo[0] + hi[0]) * 0.5, hi[1], (lo[2] + hi[2]) * 0.5];
        let from = pixel(&st, top);
        // Where the press ray meets the cabinet's base plane, and the pixel
        // 60" further along plan x on that plane.
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let cam = st.viewport.as_ref().unwrap().camera.clone();
        let p0 = drag::floor_point_at(&cam, rect, from, lo[1]).unwrap();
        let to = pixel(&st, [(p0.x + 60.0) as f32, lo[1], -p0.y as f32]);
        let mid = from + (to - from) * 0.5;
        frame_with(
            &ctx,
            &mut cx,
            &mut st,
            vec![egui::Event::PointerMoved(from)],
            1.0,
        );
        frame_with(&ctx, &mut cx, &mut st, vec![press(from, true)], 1.05);
        for (k, at) in [mid, to].into_iter().enumerate() {
            frame_with(
                &ctx,
                &mut cx,
                &mut st,
                vec![egui::Event::PointerMoved(at)],
                1.1 + k as f64 * 0.05,
            );
        }
        assert!(st.obj_drag.is_some(), "the press armed a drag");
        frame_with(&ctx, &mut cx, &mut st, vec![press(to, false)], 1.3);
        assert!(st.obj_drag.is_none());
        let after = placed_cabinet(&cx, id);
        let moved = after.position - before.position;
        let unit = cx.snap_unit();
        assert!(
            (moved.x - 60.0).abs() <= unit && moved.y.abs() <= unit,
            "moved {moved:?}"
        );
        // The camera did not orbit, the selection is the cabinet, and the
        // whole drag is one undo step.
        let yaw = st.viewport.as_ref().unwrap().camera.yaw;
        assert_eq!(yaw, cam.yaw);
        assert_eq!(
            cx.selection.single(),
            Some(crate::editor::ObjectRef::Cabinet(id))
        );
        assert_eq!(cx.undo_label(), Some(drag::LABEL));
        cx.undo();
        assert_eq!(placed_cabinet(&cx, id).position, before.position);
        assert_ne!(cx.undo_label(), Some(drag::LABEL));
    }

    fn placed_cabinet(cx: &EditorContext, id: Id) -> plan_cabinets::Cabinet {
        crate::editor::placed::cabinet_by_id(cx.floor(), id).unwrap()
    }

    #[test]
    fn a_press_without_movement_or_off_the_selection_does_not_move_anything() {
        let (mut cx, mut st, ctx, id) = view_of_a_cabinet();
        let before = placed_cabinet(&cx, id);
        let (lo, hi) = bounds_of(&st, id);
        let top = pixel(&st, [(lo[0] + hi[0]) * 0.5, hi[1], (lo[2] + hi[2]) * 0.5]);
        // A click on the selected cabinet: no move, no undo step.
        let steps = cx.can_undo();
        click(&ctx, &mut cx, &mut st, top, 1.0);
        assert_eq!(placed_cabinet(&cx, id).position, before.position);
        assert_eq!(cx.can_undo(), steps);
        // With another tool active the drag orbits as it always did.
        cx.selection.set(crate::editor::ObjectRef::Cabinet(id));
        st.select_tool = false;
        let yaw = st.viewport.as_ref().unwrap().camera.yaw;
        frame_with(
            &ctx,
            &mut cx,
            &mut st,
            vec![egui::Event::PointerMoved(top)],
            2.0,
        );
        frame_with(&ctx, &mut cx, &mut st, vec![press(top, true)], 2.05);
        let away = top + egui::vec2(80.0, 0.0);
        for (k, at) in [top + egui::vec2(40.0, 0.0), away].into_iter().enumerate() {
            frame_with(
                &ctx,
                &mut cx,
                &mut st,
                vec![egui::Event::PointerMoved(at)],
                2.1 + k as f64 * 0.05,
            );
        }
        frame_with(&ctx, &mut cx, &mut st, vec![press(away, false)], 2.3);
        assert_eq!(placed_cabinet(&cx, id).position, before.position);
        assert_ne!(st.viewport.as_ref().unwrap().camera.yaw, yaw, "it orbited");
    }

    #[test]
    fn arrow_keys_nudge_the_selection_in_3d_as_one_undo_step() {
        let (mut cx, mut st, ctx, id) = view_of_a_cabinet();
        let before = placed_cabinet(&cx, id).position;
        let (lo, hi) = bounds_of(&st, id);
        let over = pixel(&st, [(lo[0] + hi[0]) * 0.5, hi[1], (lo[2] + hi[2]) * 0.5]);
        frame_with(
            &ctx,
            &mut cx,
            &mut st,
            vec![egui::Event::PointerMoved(over)],
            1.0,
        );
        let key = |k, shift: bool| egui::Event::Key {
            key: k,
            physical_key: Some(k),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                shift,
                ..egui::Modifiers::NONE
            },
        };
        let unit = cx.snap_unit();
        // The default overview looks toward plan +y at an angle: "right" is
        // the nearest axis to the screen's right, "up" the one away from us.
        let cam = st.viewport.as_ref().unwrap().camera.clone();
        let right = drag::nudge_dir(&cam, egui::Key::ArrowRight).unwrap();
        frame_with(
            &ctx,
            &mut cx,
            &mut st,
            vec![key(egui::Key::ArrowRight, false)],
            1.1,
        );
        let moved = placed_cabinet(&cx, id).position - before;
        assert!(
            (moved.x - right.x * unit).abs() < 1e-6 && (moved.y - right.y * unit).abs() < 1e-6,
            "{moved:?} for {right:?}"
        );
        assert_eq!(cx.undo_label(), Some(drag::LABEL));
        frame_with(
            &ctx,
            &mut cx,
            &mut st,
            vec![key(egui::Key::ArrowRight, true)],
            1.2,
        );
        let moved = placed_cabinet(&cx, id).position - before;
        assert!(
            (moved.x - right.x * unit * 11.0).abs() < 1e-6,
            "Shift is ten units"
        );
        cx.undo();
        cx.undo();
        assert_eq!(placed_cabinet(&cx, id).position, before);
    }

    #[test]
    fn alt_click_sets_the_orbit_centre_to_the_clicked_surface() {
        let (mut cx, mut st, ctx, wall, on_wall) = view_of_a_wall();
        let eye = st.viewport.as_ref().unwrap().camera.eye();
        let mods = egui::Modifiers {
            alt: true,
            ..egui::Modifiers::NONE
        };
        frame_mods(
            &ctx,
            &mut cx,
            &mut st,
            vec![egui::Event::PointerMoved(on_wall)],
            1.0,
            mods,
        );
        let down = egui::Event::PointerButton {
            pos: on_wall,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: mods,
        };
        let up = egui::Event::PointerButton {
            pos: on_wall,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: mods,
        };
        frame_mods(&ctx, &mut cx, &mut st, vec![down], 1.05, mods);
        frame_mods(&ctx, &mut cx, &mut st, vec![up], 1.1, mods);
        let cam = &st.viewport.as_ref().unwrap().camera;
        let hit = [120.0, 96.0, 0.0];
        let off = plan_view3d::math::length(plan_view3d::math::sub(cam.target, hit));
        assert!(
            off < 1.5,
            "target {:?} is {off} from the wall top",
            cam.target
        );
        let now = cam.eye();
        for k in 0..3 {
            assert!((now[k] - eye[k]).abs() < 0.5, "the eye stays put");
        }
        // Nothing was selected by it.
        assert!(cx.selection.is_empty(), "{wall}");
        assert!(cx.status.contains("Orbit center"), "{}", cx.status);
    }

    #[test]
    fn double_clicking_nothing_frames_the_building_again() {
        let (mut cx, mut st, ctx, _wall, _on) = view_of_a_wall();
        let centre = st.viewport.as_ref().unwrap().camera.target;
        {
            let cam = &mut st.viewport.as_mut().unwrap().camera;
            cam.target = [900.0, 10.0, 400.0];
            cam.distance *= 0.3;
        }
        click(&ctx, &mut cx, &mut st, egui::pos2(5.0, 5.0), 1.0);
        click(&ctx, &mut cx, &mut st, egui::pos2(5.0, 5.0), 1.1);
        let cam = &st.viewport.as_ref().unwrap().camera;
        for (got, want) in cam.target.iter().zip(centre) {
            assert!((got - want).abs() < 0.5, "{:?}", cam.target);
        }
    }

    #[test]
    fn the_hover_picks_are_throttled_and_tint_the_object_under_the_pointer() {
        let (mut cx, mut st, ctx, wall, on_wall) = view_of_a_wall();
        // Two spots on the wall's top, well apart on screen.
        let along = pixel(&st, [170.0, 96.0, 0.0]);
        assert!((along - on_wall).length() > 10.0);
        let base = st.hover.picks;
        let at = |p: egui::Pos2, t: f64, cx: &mut EditorContext, st: &mut View3dState| {
            frame_with(&ctx, cx, st, vec![egui::Event::PointerMoved(p)], t);
        };
        at(on_wall, 1.0, &mut cx, &mut st);
        assert_eq!(st.hover.picks, base + 1);
        assert_eq!(st.hover.object, Some(wall));
        // Still, then a one pixel wobble, then a real move too soon: no picks.
        at(on_wall, 1.01, &mut cx, &mut st);
        at(on_wall + egui::vec2(1.0, 0.0), 1.02, &mut cx, &mut st);
        at(along, 1.03, &mut cx, &mut st);
        assert_eq!(st.hover.picks, base + 1);
        // The move is picked once the interval has passed, then a pointer
        // that stays costs nothing more.
        at(along, 1.2, &mut cx, &mut st);
        assert_eq!(st.hover.picks, base + 2);
        at(along, 1.3, &mut cx, &mut st);
        at(along, 1.4, &mut cx, &mut st);
        assert_eq!(st.hover.picks, base + 2, "a still pointer costs no pick");
        at(along + egui::vec2(0.0, 4.0), 1.5, &mut cx, &mut st);
        assert_eq!(st.hover.picks, base + 3);
        // The tint is in the viewport's overlay, light blue, over the wall.
        let tinted = |st: &View3dState| {
            st.viewport
                .as_ref()
                .unwrap()
                .overlay()
                .iter()
                .any(|m| m.object_id == Some(wall) && m.color == Some(pick::HOVER_RGB))
        };
        assert_eq!(st.hover.object, Some(wall));
        assert!(tinted(&st));
        // A selected object shows the selection tint, not the hover one.
        cx.selection.set(crate::editor::ObjectRef::Wall(wall));
        at(on_wall, 1.7, &mut cx, &mut st);
        assert!(!tinted(&st));
        // The pointer on nothing clears the highlight.
        cx.selection.clear();
        at(egui::pos2(5.0, 5.0), 2.0, &mut cx, &mut st);
        assert_eq!(st.hover.object, None);
        assert!(st.viewport.as_ref().unwrap().overlay().is_empty());
    }

    #[test]
    fn the_selection_tint_goes_in_the_overlay_without_requeueing_the_scene() {
        let (mut cx, mut st, ctx, wall, on_wall) = view_of_a_wall();
        let meshes = st.viewport.as_ref().unwrap().scene_mesh_count();
        assert_eq!(meshes, st.base_scene.meshes.len());
        click(&ctx, &mut cx, &mut st, on_wall, 1.0);
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 1.2);
        let vp = st.viewport.as_ref().unwrap();
        assert_eq!(
            vp.scene_mesh_count(),
            meshes,
            "the cached scene is untouched"
        );
        assert!(vp
            .overlay()
            .iter()
            .any(|m| m.material == Material::Selection && m.object_id == Some(wall)));
        // The camera was not re-framed by it either.
        let cam = vp.camera.clone();
        frame_with(&ctx, &mut cx, &mut st, Vec::new(), 1.4);
        assert_eq!(st.viewport.as_ref().unwrap().camera, cam);
    }

    #[test]
    fn a_painted_object_shows_the_exact_colour_and_its_bitmap() {
        use plan_core::object_materials::WHOLE_OBJECT;
        let mut p = project_with_wall();
        let wall = p.floors[0].walls[0].id;
        // A user material: a flat colour, and one with an image file.
        let flat = plan_materials::MaterialDef::new("Test Teal", &["Custom"], [12, 140, 150]);
        let mut user = plan_materials::MaterialLibrary::default();
        user.add(flat);
        crate::tools::materials::set_user_library_for_test(user);
        p.set_object_material(wall, WHOLE_OBJECT, "Test Teal");
        let scene = build_view_scene(&p, &ViewScope::default());
        let painted: Vec<&Mesh> = scene
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(wall) && m.color.is_some())
            .collect();
        assert!(!painted.is_empty());
        assert!(painted.iter().all(|m| m.color == Some([12, 140, 150])));
        crate::tools::materials::set_user_library_for_test(
            plan_materials::MaterialLibrary::default(),
        );
    }

    // ----- camera views, saved cameras, lighting, key frames (round 14) -----

    fn two_storey() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = house();
        cx.project.build_new_floor(true);
        cx
    }

    fn png_bytes(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
        plan_render::encode_png(&plan_render::Image {
            width: w,
            height: h,
            rgba: (0..w * h)
                .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
                .collect(),
            hdr: Vec::new(),
        })
    }

    #[test]
    fn a_floor_camera_shows_only_its_floor_clipped_at_the_ceiling() {
        let mut cx = two_storey();
        assert!(cx.project.floors.len() >= 2);
        let floor_cam = cx.project.add_camera(CameraObject::new(
            CameraKind::FloorCamera,
            Point::new(120.0, 96.0),
            90.0,
            "Floor Camera 1",
            0,
        ));
        let full_cam = cx.project.add_camera(CameraObject::new(
            CameraKind::FullCamera,
            Point::new(120.0, 96.0),
            90.0,
            "Camera 1",
            0,
        ));
        let mut st = View3dState::with_inbox(Outbox::default());
        st.show_camera(&cx.project, full_cam);
        assert_eq!((st.scope_floor, st.clip_above), (None, None));
        let all = build_view_scene(&cx.project, &st.scope());
        st.show_camera(&cx.project, floor_cam);
        let f = &cx.project.floors[0];
        let limit = f.elevation + f.ceiling_height + view_settings::CEILING_SLACK;
        assert_eq!((st.scope_floor, st.clip_above), (Some(0), Some(limit)));
        assert!(st.mode == CameraMode::FullCamera);
        let clipped = build_view_scene(&cx.project, &st.scope());
        assert!(clipped.triangle_count() < all.triangle_count());
        // Nothing lies wholly above the ceiling.
        for m in &clipped.meshes {
            for tri in m.indices.as_chunks::<3>().0 {
                let low = tri
                    .iter()
                    .map(|&i| m.vertices[i as usize].position[1])
                    .fold(f32::INFINITY, f32::min);
                assert!(
                    f64::from(low) < limit,
                    "a triangle at {low} is above {limit}"
                );
            }
        }
        // The signature follows the clip, so the view rebuilds.
        let sig = st.signature(&cx.project);
        st.clip_above = None;
        assert_ne!(sig, st.signature(&cx.project));
    }

    #[test]
    fn a_cameras_saved_view_settings_reach_the_viewport() {
        use plan_core::camera_view::{BackdropKind, ViewQuality};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = house();
        let mut cam =
            CameraObject::new(CameraKind::FullCamera, Point::new(60.0, 60.0), 0.0, "C", 0);
        cam.view.tilt_deg = 20.0;
        cam.view.technique = Some("Clay".into());
        cam.view.quality = ViewQuality::Preview;
        cam.view.backdrop.kind = BackdropKind::Color;
        cam.view.backdrop.color = [255, 0, 0];
        cam.view.ambient = Some(0.2);
        cam.view.sun_intensity = Some(0.0);
        let id = cx.project.add_camera(cam);
        let mut st = View3dState::with_inbox(Outbox::default());
        st.show_camera(&cx.project, id);
        assert_eq!(st.technique, RenderingTechnique::Clay);
        run_frames(&mut cx, &mut st, 2);
        let vp = st.viewport.as_ref().unwrap();
        assert!(
            (vp.camera.tilt_deg() - 20.0).abs() < 0.01,
            "{}",
            vp.camera.tilt_deg()
        );
        assert!(
            !vp.settings.shadows && !vp.settings.ambient_occlusion,
            "Preview"
        );
        assert_eq!(vp.background[..3], [1.0, 0.0, 0.0]);
        assert!((vp.lighting.ambient - 0.2).abs() < 1e-6);
        // Clay is lit, but this camera's sun is off.
        assert_eq!(vp.lighting.key, 0.0);
        // Final View turns the shadows on again.
        st.set_quality(ViewQuality::Final);
        run_frames(&mut cx, &mut st, 1);
        let vp = st.viewport.as_ref().unwrap();
        assert!(vp.settings.shadows && vp.settings.ambient_occlusion);
    }

    #[test]
    fn the_plans_lighting_sets_the_sun_and_ambient_of_every_view() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = house();
        cx.project.lighting.sun_azimuth_deg = 90.0;
        cx.project.lighting.sun_altitude_deg = 10.0;
        cx.project.lighting.ambient = 0.2;
        cx.project.lighting.sun_intensity = 0.5;
        let mut st = View3dState::with_inbox(Outbox::default());
        st.open_mode(CameraMode::Orbit, None);
        run_frames(&mut cx, &mut st, 2);
        let l = st.viewport.as_ref().unwrap().lighting;
        assert!(
            l.key_dir[0] > 0.9,
            "the sun is in the east: {:?}",
            l.key_dir
        );
        assert!((l.ambient - 0.2).abs() < 1e-6);
        assert!((l.key - 0.65 * 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_picture_backdrop_is_read_from_its_file_and_dropped_when_it_is_gone() {
        use plan_core::camera_view::BackdropKind;
        let dir = std::env::temp_dir().join(format!("plan_backdrop_view_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sky.png");
        std::fs::write(&file, png_bytes(8, 4, [30, 90, 200])).unwrap();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = house();
        let mut cam = CameraObject::new(CameraKind::PerspectiveOverview, Point::ZERO, 0.0, "O", 0);
        cam.view.backdrop.kind = BackdropKind::Image;
        cam.view.backdrop.image = file.display().to_string();
        let id = cx.project.add_camera(cam);
        let mut st = View3dState::with_inbox(Outbox::default());
        st.show_camera(&cx.project, id);
        run_frames(&mut cx, &mut st, 2);
        let img = st
            .viewport
            .as_ref()
            .unwrap()
            .backdrop
            .clone()
            .expect("picture");
        assert_eq!((img.width, img.height), (8, 4));
        // The default sky again once the camera asks for none.
        cx.project
            .update_camera(id, |c| c.view.backdrop.kind = BackdropKind::Default);
        run_frames(&mut cx, &mut st, 1);
        assert!(st.viewport.as_ref().unwrap().backdrop.is_none());
        // A missing file falls back to the sky instead of failing.
        cx.project.update_camera(id, |c| {
            c.view.backdrop.kind = BackdropKind::Image;
            c.view.backdrop.image = dir.join("gone.png").display().to_string();
        });
        run_frames(&mut cx, &mut st, 1);
        assert!(st.viewport.as_ref().unwrap().backdrop.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_glass_house_camera_opens_as_a_glass_house() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = house();
        let id = cx.project.add_camera(CameraObject::new(
            CameraKind::GlassHouse,
            Point::ZERO,
            0.0,
            "Glass",
            0,
        ));
        let mut st = View3dState::with_inbox(Outbox::default());
        st.show_camera(&cx.project, id);
        assert_eq!(st.technique, RenderingTechnique::GlassHouse);
        assert_eq!(st.mode, CameraMode::Orbit);
        let mut other = View3dState::with_inbox(Outbox::default());
        other.open_glass_house();
        assert_eq!(other.technique, RenderingTechnique::GlassHouse);
        assert!(other.active);
    }

    #[test]
    fn save_camera_keeps_a_full_camera_view_as_a_camera_in_one_undo_step() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = house();
        let src = cx.project.add_camera(CameraObject::new(
            CameraKind::FullCamera,
            Point::new(100.0, 50.0),
            90.0,
            "Camera 1",
            0,
        ));
        let mut st = View3dState::with_inbox(Outbox::default());
        st.show_camera(&cx.project, src);
        run_frames(&mut cx, &mut st, 2);
        // The user walked and looked up.
        {
            let vp = st.viewport.as_mut().unwrap();
            vp.camera.position = [150.0, 70.0, -80.0];
            vp.camera.yaw = 0.0;
            vp.camera.set_tilt_deg(15.0);
        }
        let n = cx.project.cameras.len();
        st.save_camera(&mut cx);
        assert_eq!(cx.project.cameras.len(), n + 1);
        let saved = cx.project.cameras.last().unwrap().clone();
        assert_eq!(saved.kind, CameraKind::FullCamera);
        assert!(saved.name.starts_with("Saved Camera"));
        assert!(saved.position.dist(Point::new(150.0, 80.0)) < 1e-3);
        assert!(
            (saved.direction_deg - 90.0).abs() < 1e-3,
            "{}",
            saved.direction_deg
        );
        assert!((saved.eye_height - 70.0).abs() < 1e-3);
        assert!((saved.view.tilt_deg - 15.0).abs() < 0.01);
        // New cameras start at 55 degrees (DECISIONS CS1); the saved view
        // keeps the viewport's field of view within a degree.
        assert!((saved.fov_deg - 55.0).abs() < 1.0, "{}", saved.fov_deg);
        assert_eq!(saved.view.technique.as_deref(), Some("Standard"));
        // Restore brings the same view back.
        st.show_camera(&cx.project, saved.id);
        run_frames(&mut cx, &mut st, 2);
        let cam = &st.viewport.as_ref().unwrap().camera;
        assert!((cam.position[0] - 150.0).abs() < 0.01 && (cam.position[2] + 80.0).abs() < 0.01);
        assert!((cam.tilt_deg() - 15.0).abs() < 0.01);
        // One undo step takes it away again.
        cx.undo();
        assert_eq!(cx.project.cameras.len(), n);
    }

    #[test]
    fn orthographic_overviews_and_isometric_views_are_parallel_orbits() {
        let mut st = View3dState::with_inbox(Outbox::default());
        st.open_parallel(ParallelOverview::Full, 0);
        st.apply_setup(1.5);
        let cam = &st.viewport.as_ref().unwrap().camera;
        assert!(cam.parallel && cam.is_parallel());
        assert_eq!(cam.mode, CameraMode::Orbit, "it still orbits");
        // An Isometric View looks from a corner, 45 degrees round, 30 down.
        st.open_parallel(ParallelOverview::Isometric(IsoCorner::SouthWest), 0);
        st.apply_setup(1.5);
        let cam = &st.viewport.as_ref().unwrap().camera;
        assert!(
            (cam.yaw + std::f32::consts::FRAC_PI_4).abs() < 1e-5,
            "{}",
            cam.yaw
        );
        assert!((cam.pitch - 30.0_f32.to_radians()).abs() < 1e-5);
        let ne = isometric_angles(IsoCorner::NorthEast);
        assert!(
            (ne.0 - 3.0 * std::f32::consts::FRAC_PI_4).abs() < 1e-5,
            "{}",
            ne.0
        );
        // Any ordinary view is perspective again.
        st.open_mode(CameraMode::Orbit, None);
        st.apply_setup(1.5);
        assert!(!st.viewport.as_ref().unwrap().camera.parallel);
        // The Floor Overview is limited to the active floor.
        st.open_parallel(ParallelOverview::Floor, 2);
        assert_eq!(st.scope_floor, Some(2));
    }

    #[test]
    fn save_camera_keeps_an_overviews_pose_and_restore_brings_it_back() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = house();
        let mut st = View3dState::with_inbox(Outbox::default());
        st.save_camera(&mut cx);
        assert!(cx.project.cameras.is_empty(), "no 3D view yet");
        assert!(cx.status.contains("3D view"));
        st.open_mode(CameraMode::DollHouse, None);
        run_frames(&mut cx, &mut st, 2);
        st.viewport.as_mut().unwrap().camera.orbit(0.7, 0.1);
        let (eye, target) = st.viewport.as_ref().unwrap().camera.pose();
        st.save_camera(&mut cx);
        let saved = cx.project.cameras.last().unwrap().clone();
        assert_eq!(saved.kind, CameraKind::DollHouse);
        assert!(
            saved.view.show_in_plan,
            "an overview has a symbol in the plan (DECISIONS 166)"
        );
        let (derived, raw) = (saved.overview_pose().unwrap(), saved.view.pose.unwrap());
        for i in 0..3 {
            assert!((derived.eye[i] - raw.eye[i]).abs() < 1e-6, "eye {i}");
            assert!(
                (derived.target[i] - raw.target[i]).abs() < 1e-6,
                "target {i}"
            );
        }
        let pose = saved.view.pose.expect("pose");
        for i in 0..3 {
            assert!((pose.eye[i] - f64::from(eye[i])).abs() < 0.01);
            assert!((pose.target[i] - f64::from(target[i])).abs() < 0.01);
        }
        // Another view, then restore the saved one.
        st.open_mode(CameraMode::Orbit, None);
        run_frames(&mut cx, &mut st, 2);
        st.show_camera(&cx.project, saved.id);
        run_frames(&mut cx, &mut st, 2);
        let (e, t) = st.viewport.as_ref().unwrap().camera.pose();
        assert_eq!(
            st.viewport.as_ref().unwrap().camera.mode,
            CameraMode::DollHouse
        );
        for i in 0..3 {
            assert!((e[i] - eye[i]).abs() < 0.05, "eye {i}: {e:?} vs {eye:?}");
            assert!((t[i] - target[i]).abs() < 0.05, "target {i}");
        }
    }

    #[test]
    fn save_camera_keeps_an_orthographic_view_as_an_orthographic_camera() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = house();
        let mut st = View3dState::with_inbox(Outbox::default());
        st.open_mode(CameraMode::ElevationFront, None);
        run_frames(&mut cx, &mut st, 2);
        st.save_camera(&mut cx);
        let saved = cx.project.cameras.last().expect("a saved camera");
        assert_eq!(saved.kind, CameraKind::Orthographic);
        assert_eq!(
            saved.view.ortho.map(|o| o.kind),
            Some(plan_core::camera_view::OrthoKind::ElevationFront)
        );
    }

    #[test]
    fn a_walkthrough_path_is_made_from_a_selected_cad_polyline() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = house();
        let mut st = View3dState::with_inbox(Outbox::default());
        st.walk_from_cad(&mut cx);
        assert!(cx.project.cameras.is_empty());
        assert!(cx.status.contains("CAD"));
        let pts = vec![
            Point::new(10.0, 10.0),
            Point::new(200.0, 10.0),
            Point::new(200.0, 150.0),
        ];
        let id = cx.project.add_cad(
            0,
            "CAD, Default",
            plan_core::CadItem::Polyline {
                points: pts.clone(),
                closed: false,
            },
        );
        cx.selection.set(crate::editor::ObjectRef::Cad(id));
        st.walk_from_cad(&mut cx);
        let cam = cx.project.cameras.last().expect("a walkthrough");
        assert_eq!(cam.kind, CameraKind::Walkthrough);
        assert_eq!(cam.path, pts);
        assert!(
            st.active && st.walk.is_some(),
            "the walk opens in the 3D view"
        );
        cx.undo();
        assert!(cx.project.cameras.is_empty());
    }

    #[test]
    fn the_key_frame_buttons_jump_between_the_nodes() {
        let mut c = CameraObject::walkthrough(
            vec![
                Point::new(0.0, 0.0),
                Point::new(72.0, 0.0),
                Point::new(72.0, 72.0),
            ],
            66.0,
            "W",
            0,
        );
        c.walk_speed = 36.0;
        c.path_nodes[1].hold_s = 1.0;
        // Arrivals at 0 s, 2 s and 5 s (two seconds, a one second hold, two more).
        assert_eq!(jump_key_frame(&c, 0.0, 1), 2.0);
        assert_eq!(jump_key_frame(&c, 2.0, 1), 5.0);
        assert_eq!(jump_key_frame(&c, 5.0, 1), c.walk_duration_s());
        assert_eq!(jump_key_frame(&c, 5.0, -1), 2.0);
        assert_eq!(jump_key_frame(&c, 1.0, -1), 0.0);
        assert_eq!(jump_key_frame(&c, 0.0, -1), 0.0);
    }

    #[test]
    fn recording_follows_the_cameras_frame_rate_and_picture_size() {
        let mut p = house();
        p.floors[0].elevation = 0.0;
        let mut cam = walk_camera();
        cam.path = vec![Point::new(60.0, 40.0), Point::new(180.0, 40.0)];
        cam.position = cam.path[0];
        cam.direction_deg = 0.0;
        cam.walk_speed = 120.0; // one second
        cam.path_nodes[1].tilt_deg = 20.0;
        cam.view.walk = plan_core::camera_view::WalkRecord {
            fps: 3.0,
            width: 64,
            height: 48,
            samples: 1,
            format: plan_core::camera_view::RecordFormat::Frames,
            ..plan_core::camera_view::WalkRecord::default()
        };
        assert_eq!(walk_frame_count(&cam, 3.0), 3);
        let id = p.add_camera(cam);
        let dir = std::env::temp_dir().join(format!("plan_walk_fps_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let rec = Recording::start(&p, p.camera(id).unwrap(), None, dir.clone());
        let started = std::time::Instant::now();
        let result = loop {
            if let Some(r) = rec.finished() {
                break r;
            }
            assert!(
                started.elapsed().as_secs() < 120,
                "recording never finished"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert_eq!(result, Ok(3));
        let mut frames: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".png"))
            .collect();
        assert_eq!(frames.len(), 3, "a numbered PNG sequence at 3 fps");
        frames.sort_by_key(std::fs::DirEntry::file_name);
        let bytes = std::fs::read(frames[0].path()).unwrap();
        // The PNG header names the picture size.
        assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), 64);
        assert_eq!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()), 48);
        let script = std::fs::read_to_string(dir.join("make_video.sh")).unwrap();
        assert!(
            script.contains('3'),
            "the video script uses the chosen rate: {script}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_walkthrough_tilts_and_holds_while_it_plays() {
        let mut p = house();
        let mut cam = walk_camera();
        cam.walk_speed = 50.0;
        cam.path_nodes[1].tilt_deg = 30.0;
        cam.path_nodes[1].hold_s = 1.0;
        let id = p.add_camera(cam);
        let c = p.camera(id).unwrap();
        // Two seconds of walking and a second held at the end.
        assert!((c.walk_duration_s() - 3.0).abs() < 1e-9);
        assert!((walk_tilt(c, 1.0) - 15.0).abs() < 1e-4);
        assert!((walk_tilt(c, 2.5) - 30.0).abs() < 1e-4);
        let ([x1, ..], _) = walk_view(&p, c, 2.0);
        let ([x2, ..], _) = walk_view(&p, c, 2.9);
        assert!(
            (x1 - 100.0).abs() < 1e-3 && (x2 - 100.0).abs() < 1e-3,
            "held at the node"
        );
    }

    #[test]
    fn lighting_and_record_dialogs_draw_frames_and_apply() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = house();
        let light = cx
            .project
            .add_light(
                0,
                plan_core::camera::PlanLight::new(Point::new(50.0, 50.0), 84.0),
            )
            .unwrap();
        let cam = cx.project.add_camera(walk_camera());
        let mut st = View3dState::with_inbox(Outbox::default());
        st.open_lighting(&cx.project);
        st.record_walkthrough_dialog(&mut cx);
        assert!(st.record_dialog.is_some() && st.lighting.is_some());
        st.active = true;
        run_frames(&mut cx, &mut st, 3);
        assert!(
            st.record_dialog.is_some() && st.lighting.is_some(),
            "both stay open"
        );
        // The camera under the record dialog is the walkthrough.
        assert_eq!(st.record_dialog.as_ref().unwrap().camera, cam);
        // OK in the Lighting dialog writes the plan and the light, in one step.
        {
            let d = st.lighting.as_mut().unwrap();
            d.draft_mut().interior_lights = false;
            d.draft_mut().sun_azimuth_deg = 135.0;
            d.lights_mut()[0].intensity = 3.0;
        }
        let d = st.lighting.take().unwrap();
        cx.begin_change("Lighting");
        d.apply(&mut cx.project);
        assert_eq!(cx.project.lighting.sun_azimuth_deg, 135.0);
        assert!(!cx.project.lighting.interior_lights);
        assert_eq!(cx.project.light(light).unwrap().intensity, 3.0);
        // Interior lights off: nothing lights the ray tracer.
        assert!(render_lights(&cx.project).is_empty());
        cx.project.lighting.interior_lights = true;
        assert_eq!(render_lights(&cx.project).len(), 1);
    }
}
