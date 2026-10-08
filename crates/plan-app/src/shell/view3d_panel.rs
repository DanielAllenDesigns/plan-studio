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
//! Roof planes come from `crate::editor::roof_view::roof_meshes`.

use crate::dialogs::camera::{CameraDialog, CameraExtras, RayTraceDialog};
use crate::dialogs::Outcome;
use crate::editor::EditorContext;
use crate::toolbar::ViewFlag;
use crate::tools::camera::{self as camera_tool, CameraVariant};
use crate::tools::{ToolId, ToolSet};
use eframe::egui;
use plan_3d::{build_scene, Material, Mesh, Scene, Vertex};
use plan_core::camera::{DEFAULT_EYE_HEIGHT, DEFAULT_FOV_DEG};
use plan_core::geometry::Point;
use plan_core::{CameraKind, CameraObject, Id, PlacedSymbol, Project};
use plan_materials::{settings as technique_settings, FillMode, RenderingTechnique, ShadingModel};
use plan_view3d::{standard_views, CameraMode, Lighting, Viewport3d};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
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
        View3dCommand::CrossSectionSlider => state.toggle_slider(),
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
    TechniqueView {
        show_edges: s.edge_lines || t == RenderingTechnique::Standard,
        flat: s.shading != ShadingModel::Lit,
        background,
        fill,
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

/// Puts every mesh in `material`.
pub fn apply_fill(scene: &mut Scene, material: Material) {
    for m in &mut scene.meshes {
        m.material = material;
    }
}

fn apply_to_viewport(vp: &mut Viewport3d, tv: &TechniqueView, sun: Option<[f32; 3]>) {
    vp.show_edges = tv.show_edges;
    vp.background = tv.background;
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
    let mut scene = build_scene(proj);
    scene
        .meshes
        .extend(crate::editor::roof_view::roof_meshes(proj));
    scene
        .meshes
        .extend(plan_3d::foundation::foundation_meshes(proj));
    scene
        .meshes
        .extend(crate::editor::framing_view::manual_framing_meshes(proj));
    scene.meshes.extend(symbol_meshes(proj));
    let mut scene = match &scope.section {
        Some(cut) => clip_scene(&scene, cut),
        None => scene,
    };
    if let Some(m) = scope.fill {
        apply_fill(&mut scene, m);
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
/// meshes (or a box when the geometry is missing or partial; nothing when
/// the catalog is not available), everything else is a box so each placed
/// item shows something.
pub fn symbol_meshes(project: &Project) -> Vec<Mesh> {
    use crate::tools::library::chief::{self, Chief3d};
    let mut out = Vec::new();
    for floor in &project.floors {
        for s in &floor.symbols {
            if chief::is_chief_id(&s.catalog_id) {
                match chief::placed_meshes(s, floor.elevation) {
                    Chief3d::Meshes(m) => out.extend(m),
                    Chief3d::Box => out.push(symbol_box(s, floor.elevation)),
                    Chief3d::Missing => {}
                }
            } else {
                out.push(symbol_box(s, floor.elevation));
            }
        }
    }
    out
}

struct HashFmt<'a>(&'a mut DefaultHasher);

impl std::fmt::Write for HashFmt<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

/// A hash of everything that shapes the 3D model: floors, walls, openings,
/// placed symbols, the roofs and the foundation objects.
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
    }
    p.wall_types.len().hash(&mut h);
    h.finish()
}

/// Horizontal to vertical field of view for a viewport of `aspect` (w/h).
pub fn vertical_fov(horizontal_deg: f32, aspect: f32) -> f32 {
    let half = (horizontal_deg.clamp(5.0, 170.0).to_radians() * 0.5).tan();
    (2.0 * (half / aspect.max(0.1)).atan())
        .to_degrees()
        .clamp(10.0, 120.0)
}

// ----- state -----

/// How the viewport is to be aimed once the scene is loaded.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Setup {
    Mode(CameraMode),
    Camera {
        position: Point,
        direction_deg: f64,
        /// Absolute eye height in scene Y, inches.
        eye_y: f64,
        eye_height: f64,
        fov_deg: f64,
    },
    Section(SectionCut),
}

/// The cross section slider (C-23).
#[derive(Clone, Copy, Debug)]
struct Slider {
    /// The cut at offset zero.
    base: SectionCut,
    depth: f64,
    offset: f64,
}

pub struct View3dState {
    /// A 3D view replaces the plan canvas.
    pub active: bool,
    pub viewport: Option<Viewport3d>,
    pub mode: CameraMode,
    pub technique: RenderingTechnique,
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
            scene_dirty: true,
            last_project_hash: 0,
            scope_floor: None,
            section: None,
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
                }
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
                        self.camera_dialog = Some(CameraDialog::new(c, &floor, extras));
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
        self.setup = Some(Setup::Mode(mode));
    }

    /// Opens the 3D view of a camera object (C-4, C-17).
    pub fn show_camera(&mut self, project: &Project, id: Id) {
        let Some(c) = project.camera(id) else { return };
        let elevation = project.floors.get(c.floor).map_or(0.0, |f| f.elevation);
        if let Some(e) = self.extras.get(&id) {
            self.technique = e.technique;
        }
        self.slider = None;
        self.scope_floor = None;
        self.section = None;
        match c.kind {
            CameraKind::FullCamera => {
                self.mode = CameraMode::FullCamera;
                self.setup = Some(Setup::Camera {
                    position: c.position,
                    direction_deg: c.direction_deg,
                    eye_y: elevation + c.eye_height,
                    eye_height: c.eye_height,
                    fov_deg: c.fov_deg,
                });
            }
            CameraKind::PerspectiveOverview | CameraKind::Orthographic => {
                self.mode = CameraMode::Orbit;
                self.setup = Some(Setup::Mode(CameraMode::Orbit));
            }
            CameraKind::DollHouse => {
                self.mode = CameraMode::DollHouse;
                self.setup = Some(Setup::Mode(CameraMode::DollHouse));
            }
            CameraKind::CrossSection { .. } | CameraKind::WallElevation => {
                let cut = SectionCut::from_camera(c);
                self.mode = cut.elevation_mode();
                self.section = Some(cut);
                self.setup = Some(Setup::Section(cut));
            }
        }
        self.active = true;
        self.active_camera = Some(id);
    }

    /// Picks a view from the overlay combo box; leaves any section.
    pub fn set_mode_user(&mut self, mode: CameraMode) {
        self.mode = mode;
        self.section = None;
        self.slider = None;
        self.active_camera = None;
        if let Some(vp) = &mut self.viewport {
            vp.set_mode(mode);
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

    // ----- the scene -----

    fn scope(&self) -> ViewScope {
        ViewScope {
            floor: self.scope_floor,
            section: self.section,
            fill: technique_view(self.technique).fill,
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
        self.plan_bounds = base.bounds().map(|(lo, hi)| {
            (
                Point::new(lo[0].into(), (-hi[2]).into()),
                Point::new(hi[0].into(), (-lo[2]).into()),
            )
        });
        let scene = build_view_scene(project, &self.scope());
        self.scene_empty = scene.meshes.is_empty();
        let vp = self.viewport.get_or_insert_with(Viewport3d::new);
        // Keep the user's view across rebuilds, except when about to be re-aimed.
        let keep = (self.setup.is_none() && vp.bounds().is_some()).then(|| vp.camera.clone());
        vp.queue_scene(&scene);
        if let Some(cam) = keep {
            vp.camera = cam;
        }
        self.last_project_hash = sig;
        self.scene_dirty = false;
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
            Setup::Camera {
                position,
                direction_deg,
                eye_y,
                eye_height,
                fov_deg,
            } => {
                vp.set_mode(CameraMode::FullCamera);
                let c = &mut vp.camera;
                c.eye_height = eye_height as f32;
                c.position = [position.x as f32, eye_y as f32, -position.y as f32];
                c.yaw = direction_deg.to_radians() as f32 - FRAC_PI_2;
                c.pitch = 0.0;
                c.fov_deg = vertical_fov(fov_deg as f32, aspect);
            }
            Setup::Section(cut) => {
                vp.set_mode(cut.elevation_mode());
                vp.camera.yaw = cut.yaw();
            }
        }
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
        self.windows(ctx, cx);
        self.active
    }

    fn windows(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        if let Some(mut d) = self.camera_dialog.take() {
            match d.show(ctx) {
                Outcome::Open => self.camera_dialog = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => self.apply_camera_dialog(cx, &d),
            }
        }
        self.sun_window(ctx, cx);
        self.defaults_window(ctx);
        if self.raytrace.open || self.raytrace.is_running() {
            let scope = ViewScope {
                fill: None,
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

    /// The Sun Angle toggle (C-63): date, time and latitude, also used by the
    /// ray tracer and as the viewport's key light direction.
    fn sun_window(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        if !cx.view_flags.contains(&ViewFlag::SunAngle) {
            self.sun_light = None;
            return;
        }
        let mut open = true;
        let rt = &mut self.raytrace;
        egui::Window::new("Sun Angle")
            .id(egui::Id::new("sun_angle_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
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
                let s = rt.sun();
                ui.weak(format!(
                    "Azimuth {:.0}\u{B0}, altitude {:.0}\u{B0}",
                    s.azimuth_deg, s.altitude_deg
                ));
            });
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
                    });
            });
        set_camera_defaults(d);
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

// ----- drawing -----

/// Draws the 3D view in the central area.
pub fn show(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut View3dState) {
    let ctx = ui.ctx().clone();
    st.ensure_scene(&cx.project);
    if let Some(note) = crate::tools::library::chief::take_partial_notice() {
        cx.status = note;
    }
    let rect = ui.available_rect_before_wrap();
    let size = ui.available_size();
    st.apply_setup(size.x / size.y.max(1.0));

    let tv = technique_view(st.technique);
    let sun = st.sun_light;
    let vp = st.viewport.get_or_insert_with(Viewport3d::new);
    apply_to_viewport(vp, &tv, sun);
    let resp = vp.ui(ui, size);
    st.mode = vp.camera.mode;

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
    if let Some(e) = vp.gl_error() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("3D view unavailable: {e}"),
            egui::FontId::proportional(14.0),
            egui::Color32::from_rgb(0xE0, 0x4B, 0x4B),
        );
    } else if st.scene_empty {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Nothing to show yet. Draw some walls in the plan.",
            egui::FontId::proportional(14.0),
            egui::Color32::from_gray(0x60),
        );
    }

    let mut want_mode = None;
    let mut want_technique = None;
    let (mut rebuild, mut ray, mut back) = (false, false, false);
    let mut slider_change = None;
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
                                    want_mode = Some(*m);
                                }
                            }
                        });
                    egui::ComboBox::from_id_salt("view3d_technique")
                        .selected_text(st.technique.label())
                        .show_ui(ui, |ui| {
                            for t in RenderingTechnique::ALL {
                                if ui.selectable_label(st.technique == t, t.label()).clicked() {
                                    want_technique = Some(t);
                                }
                            }
                        });
                    rebuild = ui.button("Rebuild 3D").clicked();
                    ray = ui.button("Ray Trace\u{2026}").clicked();
                    back = ui.button("Back to Plan").clicked();
                });
                if let Some(s) = &st.slider {
                    let mut off = s.offset;
                    let r = ui.add(
                        egui::Slider::new(&mut off, 0.0..=s.depth)
                            .text("Cross section depth")
                            .suffix("\""),
                    );
                    if r.changed() {
                        slider_change = Some(off);
                    }
                }
            });
        });
    if let Some(m) = want_mode {
        st.set_mode_user(m);
    }
    if let Some(t) = want_technique {
        st.set_technique(t);
    }
    if let Some(off) = slider_change {
        if let Some(s) = &mut st.slider {
            s.offset = off;
            let mut cut = s.base;
            cut.origin = cut.origin + cut.dir * off;
            st.section = Some(cut);
        }
    }
    if rebuild {
        st.rebuild();
        cx.status = "Rebuilt the 3D model".into();
    }
    if ray {
        st.raytrace.open = true;
    }
    let tab = ctx.input(|i| i.key_pressed(egui::Key::Tab)) && (resp.hovered() || resp.has_focus());
    if tab {
        if let Some(id) = st.next_camera(&cx.project) {
            st.show_camera(&cx.project, id);
        }
    }
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if back || esc {
        st.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!((c.fov_deg - vertical_fov(60.0, 4.0 / 3.0)).abs() < 1e-4);
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
        assert_eq!(camera_defaults().eye_height, 66.0);
        assert_eq!(camera_defaults().fov_deg, 60.0);
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
    fn a_chief_symbol_without_its_catalog_adds_nothing() {
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
            before
        );
    }
}
