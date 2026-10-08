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
//! Roof planes come from `crate::editor::roof_view::roof_meshes`.

use crate::dialogs::camera::{
    elevation_options_with_sun, is_elevation_camera, render_elevation_with, render_lights, sun_dir,
    AdjustLightsDialog, CameraDialog, CameraExtras, RayTraceDialog,
};
use crate::dialogs::Outcome;
use crate::editor::EditorContext;
use crate::toolbar::ViewFlag;
use crate::tools::camera::{self as camera_tool, CameraVariant};
use crate::tools::{ToolId, ToolSet};
use eframe::egui;
use plan_3d::{build_scene, Material, Mesh, Scene, Vertex};
use plan_core::camera::LIGHTS_LAYER;
use plan_core::camera::{DEFAULT_EYE_HEIGHT, DEFAULT_FOV_DEG};
use plan_core::geometry::Point;
use plan_core::{CameraKind, CameraObject, Id, PlacedSymbol, Project};
use plan_elevation::{Drawing, EdgeKind, LineWeight, Options, RegionKind, SunDir};
use plan_materials::{settings as technique_settings, FillMode, RenderingTechnique, ShadingModel};
use plan_view3d::{standard_views, CameraMode, Lighting, Viewport3d};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
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
        View3dCommand::AdjustLights => state.open_adjust_lights(&cx.project, None),
        View3dCommand::PlayWalkthrough => state.play_walkthrough(cx),
        View3dCommand::RecordWalkthrough => state.record_walkthrough_dialog(cx),
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
    scene
        .meshes
        .extend(crate::editor::details_view::detail_meshes(proj));
    scene.meshes.extend(symbol_meshes(proj));
    // Pictures, 3D solid features and library solids (billboards keep their
    // stored angle in the cached scene).
    scene
        .meshes
        .extend(crate::editor::placed::image_meshes(proj));
    scene
        .meshes
        .extend(crate::editor::placed::solid_meshes(proj));
    // Terrain surface, roads and landscape objects.
    if let Some(view) = crate::editor::site_view::terrain_view(proj) {
        if let Some(surface) = &view.surface {
            scene.meshes.push(plan_terrain::terrain_mesh(surface));
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
            // Pictures and solids are meshed by `editor::placed`.
            if s.image.is_some() || s.solid {
                continue;
            }
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
            for (part, mut m) in plan_stairs::tagged_meshes(&obj.stair) {
                m.material = match part {
                    StairPart::Tread | StairPart::Landing | StairPart::Ramp => Material::Framing,
                    StairPart::Riser | StairPart::Stringer | StairPart::Handrail => Material::Trim,
                };
                out.push(m);
            }
        }
    }
    out
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
        for c in f.cad.iter().filter(|c| c.layer != LIGHTS_LAYER) {
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
    }
    p.wall_types.len().hash(&mut h);
    // The terrain (surface, roads, landscape objects).
    if let Some(t) = &p.terrain {
        let _ = write!(HashFmt(&mut h), "{t:?}");
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

/// Frame rate of recorded walkthroughs.
pub const RECORD_FPS: f64 = 12.0;

/// Path-traced settings of a recorded frame: small and quick, since a walk of
/// a minute is several hundred frames.
pub fn record_settings() -> plan_render::RenderSettings {
    plan_render::RenderSettings {
        width: 640,
        height: 480,
        samples: 8,
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
/// `false` to stop early; the number of frames written is returned.
#[allow(clippy::too_many_arguments)]
pub fn record_walkthrough(
    scene: &Scene,
    cam: &CameraObject,
    elevation: f64,
    lights: &[plan_render::PointLight],
    env: &plan_render::Environment,
    settings: &plan_render::RenderSettings,
    fps: f64,
    out_dir: &Path,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> std::io::Result<usize> {
    std::fs::create_dir_all(out_dir)?;
    let renderer = plan_render::Renderer::new(scene);
    let total = walk_frame_count(cam, fps);
    let aspect = settings.width as f32 / settings.height.max(1) as f32;
    let fov = vertical_fov(cam.fov_deg as f32, aspect);
    for i in 0..total {
        let u = if total > 1 {
            i as f64 / (total - 1) as f64
        } else {
            0.0
        };
        let pose = cam.walk_pose(u);
        let camera = plan_render::Camera::from_plan(
            pose.position,
            pose.direction_deg,
            elevation + pose.eye_height,
            f64::from(fov),
        );
        let image = renderer.render(&camera, env, lights, settings);
        std::fs::write(
            out_dir.join(plan_view3d::export::frame_file_name(i + 1)),
            plan_render::encode_png(&image),
        )?;
        if !progress(i + 1, total) {
            return Ok(i + 1);
        }
    }
    plan_view3d::export::write_ffmpeg_script(out_dir, fps)?;
    Ok(total)
}

/// A walkthrough being recorded on a worker thread.
pub struct Recording {
    done: Arc<AtomicUsize>,
    total: usize,
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Result<usize, String>>>>,
    pub dir: PathBuf,
}

impl Recording {
    /// Starts recording `cam` into `dir`.
    pub fn start(
        project: &Project,
        cam: &CameraObject,
        sun: Option<plan_render::Sun>,
        dir: PathBuf,
    ) -> Self {
        let scene = build_view_scene(project, &ViewScope::default());
        let elevation = project.floors.get(cam.floor).map_or(0.0, |f| f.elevation);
        let lights = render_lights(project);
        let cam = cam.clone();
        let total = walk_frame_count(&cam, RECORD_FPS);
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
                    record_walkthrough(
                        &scene,
                        &cam,
                        elevation,
                        &lights,
                        &env,
                        &record_settings(),
                        RECORD_FPS,
                        &out,
                        &mut |n, _| {
                            d2.store(n, Ordering::Relaxed);
                            !c2.load(Ordering::Relaxed)
                        },
                    )
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
    /// The vector elevation on screen (Vector View, Technical Illustration).
    pub vector: VectorView,
    /// The walkthrough being played, if any.
    pub walk: Option<WalkPlay>,
    recording: Option<Recording>,
    adjust_lights: Option<AdjustLightsDialog>,
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
            vector: VectorView::default(),
            walk: None,
            recording: None,
            adjust_lights: None,
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
        self.walk = None;
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
        if c.kind != CameraKind::Walkthrough {
            self.walk = None;
        }
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
                });
                if !self.walk.is_some_and(|w| w.camera == id) {
                    self.walk = Some(WalkPlay {
                        camera: id,
                        t_s: 0.0,
                        playing: false,
                    });
                }
            }
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
            CameraKind::CrossSection { .. } | CameraKind::WallElevation | CameraKind::Elevation => {
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
        self.walk = None;
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

    /// The Sun Angle's shadow direction, while that toggle is on.
    pub fn plan_sun(&self, sun_angle_on: bool) -> Option<SunDir> {
        sun_angle_on
            .then(|| sun_dir(&self.raytrace.sun()))
            .flatten()
    }

    /// Starts a new vector drawing when the project, the camera, the
    /// technique or the sun changed since the one on screen ("Refresh").
    pub fn refresh_vector(&mut self, project: &Project, sun: Option<SunDir>) {
        let Some(cam) = self.vector_camera(project).cloned() else {
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
        let Some(cam) = self.vector_camera(project).cloned() else {
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

    /// Record Walkthrough: asks for a folder and renders the frames there.
    pub fn record_walkthrough_dialog(&mut self, cx: &mut EditorContext) {
        let Some(id) = self.walk_camera_id(&cx.project) else {
            cx.status = "No walkthrough yet: draw one with Create Walkthrough Path".into();
            return;
        };
        if self.recording.is_some() {
            cx.status = "A walkthrough is already being recorded".into();
            return;
        }
        let Some(dir) = rfd::FileDialog::new()
            .set_title("Folder for the walkthrough frames")
            .pick_folder()
        else {
            return;
        };
        self.start_recording(cx, id, dir);
    }

    /// Starts recording walkthrough camera `id` into `dir`.
    pub fn start_recording(&mut self, cx: &mut EditorContext, id: Id, dir: PathBuf) {
        let Some(cam) = cx.project.camera(id).cloned() else {
            return;
        };
        let sun = self.raytrace.environment().sun;
        let rec = Recording::start(&cx.project, &cam, sun, dir);
        cx.status = format!(
            "Recording {} frames to {}",
            rec.progress().1,
            rec.dir.display()
        );
        self.recording = Some(rec);
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
        self.lights_window(ctx, cx);
        self.recording_window(ctx, cx);
        self.sun_window(ctx, cx);
        self.defaults_window(ctx);
        if self.raytrace.open || self.raytrace.is_running() {
            self.raytrace.lights = render_lights(&cx.project);
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
                Ok(n) => format!("Recorded {n} frames to {}", rec.dir.display()),
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
                ui.weak(format!("Writing frame_NNNN.png to {}", rec.dir.display()));
                cancel = ui.button("Cancel").clicked();
            });
        if cancel {
            rec.cancel();
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
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
    walk_scrub: Option<f64>,
    walk_speed: Option<f64>,
    record: bool,
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

/// Draws the 3D view in the central area.
pub fn show(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut View3dState) {
    let ctx = ui.ctx().clone();
    if st.vector_camera(&cx.project).is_some() {
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

    let tv = technique_view(st.technique);
    let sun = st.sun_light;
    let vp = st.viewport.get_or_insert_with(Viewport3d::new);
    apply_to_viewport(vp, &tv, sun);
    if let Some((position, yaw)) = walk_pose {
        vp.camera.position = position;
        vp.camera.yaw = yaw;
        vp.camera.pitch = 0.0;
    }
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

    let mut out = BarOut::default();
    let walk_info = st.walk.and_then(|w| {
        cx.project
            .camera(w.camera)
            .map(|c| (w, c.walk_duration_s(), c.walk_speed))
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
                    out.rebuild = ui.button("Rebuild 3D").clicked();
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
                if let Some((w, duration, speed)) = walk_info {
                    ui.horizontal(|ui| {
                        let label = if w.playing {
                            "Pause"
                        } else {
                            "Play Walkthrough"
                        };
                        out.walk_toggle = ui.button(label).clicked();
                        out.walk_stop = ui.button("Stop").clicked();
                        let mut t = w.t_s;
                        let r = ui.add(
                            egui::Slider::new(&mut t, 0.0..=duration.max(0.01))
                                .text("s")
                                .fixed_decimals(1),
                        );
                        if r.changed() {
                            out.walk_scrub = Some(t);
                        }
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
        st.vector.invalidate();
        st.rebuild();
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
    }
    if out.walk_stop || out.walk_scrub.is_some() {
        if let (Some(w), Some(vp)) = (st.walk, st.viewport.as_mut()) {
            if let Some(c) = cx.project.camera(w.camera) {
                let (position, yaw) = walk_view(&cx.project, c, w.t_s);
                vp.camera.position = position;
                vp.camera.yaw = yaw;
                vp.camera.pitch = 0.0;
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
    let sun = st.plan_sun(cx.view_flags.contains(&ViewFlag::SunAngle));
    st.refresh_vector(&cx.project, sun);
    let rect = ui.available_rect_before_wrap();
    let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
    ui.painter()
        .rect_filled(rect, 0.0, egui::Color32::from_rgb(0xFA, 0xFA, 0xF8));
    if resp.dragged() {
        st.vector.pan += resp.drag_delta();
    }
    if resp.hovered() {
        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
        if let (true, Some(at)) = (scroll != 0.0, resp.hover_pos()) {
            st.vector.zoom_about(rect, at, (scroll / 240.0).exp());
        }
    }
    if resp.double_clicked() {
        st.vector.reset_view();
    }
    paint_vector(ui.painter(), rect, &st.vector, st.technique);
    let name = st
        .active_camera
        .and_then(|id| cx.project.camera(id))
        .map_or(String::new(), |c| c.name.clone());
    if st.vector.drawing().is_none() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Drawing the view\u{2026}",
            egui::FontId::proportional(14.0),
            egui::Color32::from_gray(0x60),
        );
    } else if st.vector.drawing().is_some_and(|d| d.lines.is_empty()) {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Nothing to draw: the view is empty.",
            egui::FontId::proportional(14.0),
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
                    out.to_layout = ui.button("Send to Layout").clicked();
                    out.layout_pdf = ui.button("Layout PDF\u{2026}").clicked();
                    out.ray = ui.button("Ray Trace\u{2026}").clicked();
                    out.back = ui.button("Back to Plan").clicked();
                    if busy {
                        ui.spinner();
                    }
                });
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
        assert!(st.plan_sun(false).is_none());
        let s = st
            .plan_sun(true)
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
}
