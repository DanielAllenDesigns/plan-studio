//! Camera tools (`docs/parity/3d-views-cameras.md`, C-4..C-30): Full Camera,
//! the overview cameras and the cross section / elevation camera.
//!
//! * **Full Camera** (C-4, C-5): press at the eye, drag to set the viewing
//!   direction, release creates a [`CameraObject`] on layer "Cameras" and
//!   opens the 3D view from it (the tool posts a [`ViewRequest`]; the 3D
//!   panel in `shell/view3d_panel.rs` drains the queue).
//! * **Overviews** (C-10, C-11, C-13): one click opens the matching 3D view
//!   framing the whole plan. No camera object is created (C-14, verify).
//! * **Cross Section/Elevation** (C-17, C-19): press-drag defines the cut
//!   line; the view looks to the LEFT of the drag direction, so dragging a
//!   line west to east looks north. Release creates a `CrossSection` camera
//!   whose cut line is stored in `CameraObject.section` (a [`SectionLine`]
//!   with both ends and the back clip). `position` mirrors the line centre
//!   and `direction_deg` the viewing direction so every consumer of a camera
//!   keeps working; `fov_deg` is left alone. Files from before the typed
//!   field encoded a section as centre, direction and the line length in
//!   `fov_deg`; they are still read that way when `section` is `None` (see
//!   [`section_line`]) and are converted the first time one is edited
//!   ([`upgrade_section`]).
//! * **Wall Elevation** (C-20): click a wall; the camera stands on the clicked
//!   side and looks at that wall face, cut just in front of the face and
//!   back-clipped to the wall's thickness. **Auto Elevations / Auto
//!   Back-Clipped Elevations** (C-21): one click makes (or updates) the four
//!   North/East/South/West elevation cameras around the building.
//! * **Walkthrough**: click-click-double-click (or Enter) draws a path; press
//!   and drag at a node to fix its look direction. The camera keeps the path,
//!   a height and an optional look direction per node (`CameraObject.path`).
//! * **Add Lights**: a click places a point light at the default height;
//!   clicking a light opens Adjust Lights, Delete removes the selected one.
//! * Camera symbols (C-24) and handles (C-25..C-28): Move, Aim (direction,
//!   Shift snaps to the Editing angle step, 15 degrees by default, C-27),
//!   Clip distance, and for sections the two line ends. Tab cycles cameras, Delete removes one (C-29), a
//!   double-click opens the Camera Specification (C-30).

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::{Camera, EditorContext};
use crate::shell::view3d_panel::{camera_defaults, Outbox, ViewRequest};
use eframe::egui::{self, Color32, Pos2, Shape, Stroke, StrokeKind};
use plan_core::camera::{PlanLight, WalkNode, DEFAULT_CONE_LENGTH, DEFAULT_FOV_DEG};
use plan_core::extras::SectionLine;
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::{CameraKind, CameraObject, EditingDefaults, Id, Layer, Project, Wall};
use std::sync::Mutex;

/// The layer camera symbols live on (C-24).
pub const CAMERA_LAYER: &str = "Cameras";
/// Camera symbol colour, the Camera toolbar group's blue (C-24).
const CAMERA_BLUE: Color32 = Color32::from_rgb(0x2F, 0x6C, 0xB3);
/// Sections shorter than this are not created.
const MIN_SECTION_LEN: f64 = 12.0;
/// A press-release closer than this is a click, not a drag, in inches.
const CLICK_DIST: f64 = 3.0;
/// Back clip given to a new Back-Clipped Cross Section, inches (C-19).
const DEFAULT_BACK_CLIP: f64 = 120.0;
/// Length of the direction arrow on a section symbol, inches.
const ARROW_LEN: f64 = 36.0;
/// Shortest clip distance a handle drag can set, inches.
const MIN_CLIP: f64 = 24.0;
/// Direction used by a Full Camera placed with a click alone (looks up the plan).
const DEFAULT_DIRECTION_DEG: f64 = 90.0;
/// A Wall Elevation cuts this far in front of the wall face, inches.
const WALL_ELEVATION_GAP: f64 = 0.5;
/// A Wall Elevation reaches this far past the wall's far face, inches.
const WALL_ELEVATION_REACH: f64 = 2.0;
/// Auto Elevations stand this far outside the building, inches.
const AUTO_ELEVATION_MARGIN: f64 = 24.0;
/// Auto Back-Clipped Elevations draw this much behind the building face, inches.
const AUTO_BACK_CLIP: f64 = 60.0;
/// Radius of a light's symbol in the plan, inches.
const LIGHT_RADIUS: f64 = 9.0;

/// Which camera the tool creates.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CameraVariant {
    #[default]
    FullCamera,
    /// Perspective Full Overview (C-10).
    FullOverview,
    /// Perspective Floor Overview (C-11).
    FloorOverview,
    /// Doll House View (C-13).
    DollHouse,
    /// Cross Section/Elevation Camera (C-17).
    CrossSection,
    /// Back-Clipped Cross Section (C-19).
    BackClippedSection,
    /// Wall Elevation Camera (C-20): click a wall.
    WallElevation,
    /// Auto Elevations (C-21): one click, four exterior elevations.
    AutoElevation,
    /// Auto Back-Clipped Elevations: four back-clipped sections outside the building.
    AutoBackclipped,
    /// Create Walkthrough Path.
    Walkthrough,
    /// Add Lights (C-64).
    AddLights,
}

impl CameraVariant {
    pub fn label(self) -> &'static str {
        match self {
            CameraVariant::FullCamera => "Full Camera",
            CameraVariant::FullOverview => "Perspective Full Overview",
            CameraVariant::FloorOverview => "Perspective Floor Overview",
            CameraVariant::DollHouse => "Doll House View",
            CameraVariant::CrossSection => "Cross Section/Elevation Camera",
            CameraVariant::BackClippedSection => "Back-Clipped Cross Section",
            CameraVariant::WallElevation => "Wall Elevation Camera",
            CameraVariant::AutoElevation => "Auto Elevations",
            CameraVariant::AutoBackclipped => "Auto Back-Clipped Elevations",
            CameraVariant::Walkthrough => "Create Walkthrough Path",
            CameraVariant::AddLights => "Add Lights",
        }
    }

    fn is_auto(self) -> bool {
        matches!(
            self,
            CameraVariant::AutoElevation | CameraVariant::AutoBackclipped
        )
    }

    fn is_section(self) -> bool {
        matches!(
            self,
            CameraVariant::CrossSection | CameraVariant::BackClippedSection
        )
    }

    fn is_overview(self) -> bool {
        matches!(
            self,
            CameraVariant::FullOverview | CameraVariant::FloorOverview | CameraVariant::DollHouse
        )
    }
}

/// The variant the toolbar or menu picked, read by [`CameraTool::set_variant`]
/// (a `ToolId` cannot carry it).
static NEXT_VARIANT: Mutex<Option<CameraVariant>> = Mutex::new(None);

/// Chooses the variant the Camera tool starts with the next time it is
/// activated or re-activated.
pub fn set_next_variant(v: CameraVariant) {
    *NEXT_VARIANT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(v);
}

fn take_next_variant() -> Option<CameraVariant> {
    NEXT_VARIANT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
}

// ----- geometry shared with the 3D panel -----

/// Is this a camera placed by a cut line (cross section, wall elevation or
/// exterior elevation)?
pub fn is_section(c: &CameraObject) -> bool {
    matches!(
        c.kind,
        CameraKind::CrossSection { .. } | CameraKind::WallElevation | CameraKind::Elevation
    )
}

/// Unit vector along the section line (the viewing direction turned 90
/// degrees clockwise, so the view looks to the left of A to B).
pub fn section_tangent(c: &CameraObject) -> Point {
    if let Some(s) = &c.section {
        let v = s.b - s.a;
        if v.length() > 1e-9 {
            return v.normalized();
        }
    }
    let d = c.direction();
    Point::new(d.y, -d.x)
}

/// Length of a section line.
pub fn section_width(c: &CameraObject) -> f64 {
    match &c.section {
        Some(s) => s.a.dist(s.b).max(MIN_SECTION_LEN),
        // Old encoding: the length rides in `fov_deg`.
        None => c.fov_deg.max(MIN_SECTION_LEN),
    }
}

/// The two ends of a section camera's cut line.
pub fn section_line(c: &CameraObject) -> (Point, Point) {
    if let Some(s) = &c.section {
        return (s.a, s.b);
    }
    let t = section_tangent(c) * (section_width(c) * 0.5);
    (c.position - t, c.position + t)
}

/// Writes a section's geometry: the cut line of `width` centred on `centre`
/// looking along `view_deg`, with back clip `back`. Also keeps `position`,
/// `direction_deg` and the back clip of the camera kind in step.
pub fn set_section_geometry(
    c: &mut CameraObject,
    centre: Point,
    view_deg: f64,
    width: f64,
    back: Option<f64>,
) {
    let a = view_deg.to_radians();
    let view = Point::new(a.cos(), a.sin());
    let half = Point::new(view.y, -view.x) * (width.max(MIN_SECTION_LEN) * 0.5);
    c.position = centre;
    c.direction_deg = view_deg;
    c.section = Some(SectionLine {
        a: centre - half,
        b: centre + half,
        back_clip: back,
    });
    if let CameraKind::CrossSection { back_clip } = &mut c.kind {
        *back_clip = back;
    }
}

/// Gives a section read from an old file (no `section` yet) its typed cut
/// line and frees `fov_deg` from carrying the length. Other cameras are left
/// alone.
pub fn upgrade_section(c: &mut CameraObject) {
    if is_section(c) && c.section.is_none() {
        let (a, b) = section_line(c);
        c.section = Some(SectionLine {
            a,
            b,
            back_clip: back_clip(c),
        });
        c.fov_deg = DEFAULT_FOV_DEG;
    }
}

/// Centre, viewing direction (degrees) and length of the section whose cut
/// line was dragged from `a` to `b`; `None` for a line that is too short.
pub fn section_from_drag(a: Point, b: Point) -> Option<(Point, f64, f64)> {
    let len = a.dist(b);
    if len < MIN_SECTION_LEN {
        return None;
    }
    let t = (b - a).normalized();
    let view = t.perp();
    Some((Point::lerp(a, b, 0.5), view.angle().to_degrees(), len))
}

/// Back clip distance of a section camera, if it has one (C-19).
pub fn back_clip(c: &CameraObject) -> Option<f64> {
    match (&c.section, c.kind) {
        (Some(s), _) => s.back_clip,
        (None, CameraKind::CrossSection { back_clip }) => back_clip,
        _ => None,
    }
}

// ----- elevation cameras made from the plan (C-20, C-21) -----

/// The wall of `floor` whose body (or a pick tolerance beyond it) contains `p`.
pub fn wall_at(project: &Project, floor: usize, p: Point, tol: f64) -> Option<&Wall> {
    project
        .floors
        .get(floor)?
        .walls
        .iter()
        .map(|w| (dist_to_segment(p, w.start, w.end) - w.thickness * 0.5, w))
        .filter(|(d, _)| *d <= tol)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, w)| w)
}

/// A Wall Elevation camera for the face of `wall` that is on the same side as
/// `toward`: it stands on that side and looks at the wall. Its cut line runs
/// the length of the wall just in front of the face and the back clip reaches
/// only through the wall, so the view shows that wall face (with its openings)
/// and nothing of the rest of the building.
pub fn wall_elevation(wall: &Wall, toward: Point, floor: usize, name: &str) -> CameraObject {
    let u = (wall.end - wall.start).normalized();
    let mut n = u.perp();
    if (toward - wall.start).dot(n) < 0.0 {
        n = n * -1.0;
    }
    let half = wall.thickness * 0.5;
    let centre = Point::lerp(wall.start, wall.end, 0.5) + n * (half + WALL_ELEVATION_GAP);
    let view_deg = (n * -1.0).angle().to_degrees();
    let reach = wall.thickness + WALL_ELEVATION_GAP + WALL_ELEVATION_REACH;
    let mut cam = CameraObject::new(CameraKind::WallElevation, centre, view_deg, name, floor);
    set_section_geometry(
        &mut cam,
        centre,
        view_deg,
        wall.start.dist(wall.end),
        Some(reach),
    );
    cam
}

/// Plan bounds `(min, max)` of every wall of every floor.
pub fn building_bounds(project: &Project) -> Option<(Point, Point)> {
    let mut it = project
        .floors
        .iter()
        .flat_map(|f| f.walls.iter().flat_map(|w| w.footprint()));
    let first = it.next()?;
    Some(it.fold((first, first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}

/// The four elevation cameras around the building (South, North, West, East,
/// each standing outside and looking at its face). With `backclipped` they
/// are Back-Clipped Cross Sections drawing [`AUTO_BACK_CLIP`] behind the face
/// instead of plain exterior elevations. `None` without walls.
pub fn auto_elevation_cameras(
    project: &Project,
    floor: usize,
    backclipped: bool,
) -> Option<Vec<CameraObject>> {
    let (lo, hi) = building_bounds(project)?;
    let m = AUTO_ELEVATION_MARGIN;
    let (cx, cy) = ((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
    // (name, view direction, centre of the cut line, its length)
    let sides = [
        (
            "South",
            90.0,
            Point::new(cx, lo.y - m),
            hi.x - lo.x + 2.0 * m,
        ),
        (
            "North",
            270.0,
            Point::new(cx, hi.y + m),
            hi.x - lo.x + 2.0 * m,
        ),
        ("West", 0.0, Point::new(lo.x - m, cy), hi.y - lo.y + 2.0 * m),
        (
            "East",
            180.0,
            Point::new(hi.x + m, cy),
            hi.y - lo.y + 2.0 * m,
        ),
    ];
    Some(
        sides
            .into_iter()
            .map(|(side, dir, centre, width)| {
                let (kind, name) = if backclipped {
                    (
                        CameraKind::CrossSection {
                            back_clip: Some(m + AUTO_BACK_CLIP),
                        },
                        format!("{side} Back-Clipped Elevation"),
                    )
                } else {
                    (CameraKind::Elevation, format!("{side} Elevation"))
                };
                let back = backclipped.then_some(m + AUTO_BACK_CLIP);
                let mut cam = CameraObject::new(kind, centre, dir, name, floor);
                set_section_geometry(&mut cam, centre, dir, width, back);
                cam
            })
            .collect(),
    )
}

/// Adds the Auto Elevation cameras to `project`, updating the ones a previous
/// run made (same name) instead of duplicating them. Returns their ids in
/// South, North, West, East order; empty without walls.
pub fn add_auto_elevations(project: &mut Project, floor: usize, backclipped: bool) -> Vec<Id> {
    let Some(cams) = auto_elevation_cameras(project, floor, backclipped) else {
        return Vec::new();
    };
    ensure_camera_layer(project);
    cams.into_iter()
        .map(|cam| {
            let existing = project
                .cameras
                .iter()
                .find(|c| c.name == cam.name && is_section(c))
                .map(|c| c.id);
            match existing {
                Some(id) => {
                    project.update_camera(id, |c| {
                        c.kind = cam.kind;
                        c.position = cam.position;
                        c.direction_deg = cam.direction_deg;
                        c.section = cam.section;
                        c.floor = cam.floor;
                    });
                    id
                }
                None => project.add_camera(cam),
            }
        })
        .collect()
}

fn ensure_camera_layer(project: &mut Project) {
    if project.layers.get(CAMERA_LAYER).is_none() {
        project
            .layers
            .add(Layer::new(CAMERA_LAYER, [0x2F, 0x6C, 0xB3], 25));
    }
}

// ----- handles (C-25..C-28) -----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CamHandle {
    /// Move the whole camera (the glyph, or the section's centre).
    Move,
    /// Rotate the viewing direction.
    Aim,
    /// Clip distance (far clip, or a section's back clip).
    Clip,
    /// Section line ends: stretch the width.
    EndA,
    EndB,
}

/// The handles of a selected camera with their plan positions.
pub fn handles_of(c: &CameraObject) -> Vec<(CamHandle, Point)> {
    let d = c.direction();
    if c.kind == CameraKind::Walkthrough {
        // The whole path moves with the first node; nodes are edited in the
        // Camera Specification.
        vec![(CamHandle::Move, c.position)]
    } else if is_section(c) {
        let (a, b) = section_line(c);
        let mut v = vec![
            (CamHandle::Move, c.position),
            (CamHandle::EndA, a),
            (CamHandle::EndB, b),
            (CamHandle::Aim, c.position + d * ARROW_LEN),
        ];
        if let Some(bc) = back_clip(c) {
            v.push((CamHandle::Clip, c.position + d * bc));
        }
        v
    } else {
        let len = c.clip_distance.unwrap_or(DEFAULT_CONE_LENGTH);
        vec![
            (CamHandle::Move, c.position),
            (CamHandle::Aim, c.position + d * (len * 0.5)),
            (CamHandle::Clip, c.position + d * len),
        ]
    }
}

/// The handle of `c` within `tol` of `p`, nearest first (Move wins ties).
pub fn hit_handle(c: &CameraObject, p: Point, tol: f64) -> Option<CamHandle> {
    handles_of(c)
        .into_iter()
        .filter(|(_, at)| at.dist(p) <= tol)
        .min_by(|a, b| {
            let da = a.1.dist(p) + f64::from(a.0 != CamHandle::Move) * 1e-6;
            let db = b.1.dist(p) + f64::from(b.0 != CamHandle::Move) * 1e-6;
            da.total_cmp(&db)
        })
        .map(|(h, _)| h)
}

/// Does `p` touch the camera's symbol (glyph, or the section line)?
pub fn hit_symbol(c: &CameraObject, p: Point, tol: f64) -> bool {
    if c.kind == CameraKind::Walkthrough {
        c.position.dist(p) <= tol + 9.0
            || c.path
                .windows(2)
                .any(|w| dist_to_segment(p, w[0], w[1]) <= tol)
    } else if is_section(c) {
        let (a, b) = section_line(c);
        dist_to_segment(p, a, b) <= tol
    } else {
        c.position.dist(p) <= tol + 9.0
    }
}

/// Drags handle `h` of `c` to the plan point `to`. `snap_angle` rounds a
/// direction to the default angle step (15 degrees, C-27); the tool passes the
/// plan's Editing step through [`apply_handle_with`].
pub fn apply_handle(c: &mut CameraObject, h: CamHandle, to: Point, snap_angle: bool) {
    apply_handle_with(
        c,
        h,
        to,
        snap_angle.then(|| EditingDefaults::default().angle_snap_deg),
    );
}

/// [`apply_handle`] with the angle step to round a direction to, if any.
pub fn apply_handle_with(c: &mut CameraObject, h: CamHandle, to: Point, snap_deg: Option<f64>) {
    upgrade_section(c);
    match h {
        CamHandle::Move => {
            let delta = to - c.position;
            c.position = to;
            if let Some(s) = &mut c.section {
                s.a = s.a + delta;
                s.b = s.b + delta;
            }
            for n in &mut c.path {
                *n = *n + delta;
            }
        }
        CamHandle::Aim => {
            let v = to - c.position;
            if v.length() > 1e-6 {
                let mut deg = v.angle().to_degrees();
                if let Some(step) = snap_deg.filter(|s| *s > 0.0) {
                    deg = (deg / step).round() * step;
                }
                if is_section(c) {
                    let (centre, width, back) = (c.position, section_width(c), back_clip(c));
                    set_section_geometry(c, centre, deg, width, back);
                } else {
                    c.direction_deg = deg;
                }
            }
        }
        CamHandle::Clip => {
            let along = ((to - c.position).dot(c.direction())).max(MIN_CLIP);
            if is_section(c) {
                if let CameraKind::CrossSection { back_clip } = &mut c.kind {
                    *back_clip = Some(along);
                }
                if let Some(s) = &mut c.section {
                    s.back_clip = Some(along);
                }
            } else {
                c.clip_distance = Some(along);
            }
        }
        CamHandle::EndA | CamHandle::EndB => {
            if is_section(c) {
                let half = (to - c.position).dot(section_tangent(c)).abs();
                let (centre, dir, back) = (c.position, c.direction_deg, back_clip(c));
                set_section_geometry(c, centre, dir, half * 2.0, back);
            }
        }
    }
}

// ----- the tool -----

struct Placing {
    start: Point,
    current: Point,
}

struct Editing {
    id: Id,
    handle: CamHandle,
    /// The undo step has been taken (the pointer actually moved).
    began: bool,
}

/// A walkthrough path being drawn.
struct WalkPlacing {
    nodes: Vec<Point>,
    /// Fixed look direction per node (set by pressing and dragging at it).
    looks: Vec<Option<f64>>,
    /// The pointer is down on the last node (a drag aims it).
    pressing: bool,
    /// Where the pointer is, for the rubber band.
    cursor: Point,
}

/// A light being dragged.
struct LightEdit {
    id: Id,
    began: bool,
}

pub struct CameraTool {
    pub variant: CameraVariant,
    placing: Option<Placing>,
    editing: Option<Editing>,
    walk: Option<WalkPlacing>,
    selected_light: Option<Id>,
    light_edit: Option<LightEdit>,
    /// The selected camera. Kept here because the shared selection only keeps
    /// object kinds the model stores per floor.
    selected: Option<Id>,
    outbox: Outbox,
}

impl Default for CameraTool {
    fn default() -> Self {
        Self::with_outbox(Outbox::global())
    }
}

impl CameraTool {
    /// A tool that posts its view requests to `outbox` (tests use their own).
    pub fn with_outbox(outbox: Outbox) -> Self {
        Self {
            variant: CameraVariant::default(),
            placing: None,
            editing: None,
            walk: None,
            selected_light: None,
            light_edit: None,
            selected: None,
            outbox,
        }
    }

    pub fn selected(&self) -> Option<Id> {
        self.selected
    }

    /// The selected light (Add Lights).
    pub fn selected_light(&self) -> Option<Id> {
        self.selected_light
    }

    fn reset_gestures(&mut self) {
        self.placing = None;
        self.editing = None;
        self.walk = None;
        self.light_edit = None;
    }

    fn layer_ok(&self, cx: &mut EditorContext) -> bool {
        if cx.project.layers.is_locked(CAMERA_LAYER) {
            cx.status = "The Cameras layer is locked".into();
            return false;
        }
        true
    }

    fn unique_name(&self, cx: &EditorContext, base: &str) -> String {
        let mut n = cx.project.cameras.len() + 1;
        loop {
            let name = format!("{base} {n}");
            if !cx.project.cameras.iter().any(|c| c.name == name) {
                return name;
            }
            n += 1;
        }
    }

    /// The camera under `p` on the current floor, selected one first.
    fn camera_at(&self, cx: &EditorContext, p: Point) -> Option<Id> {
        let tol = cx.pick_tol();
        let sel = self
            .selected
            .and_then(|id| cx.project.camera(id))
            .filter(|c| c.floor == cx.floor);
        if let Some(c) = sel {
            if hit_symbol(c, p, tol) || hit_handle(c, p, tol).is_some() {
                return Some(c.id);
            }
        }
        cx.project
            .cameras_on(cx.floor)
            .filter(|c| hit_symbol(c, p, tol))
            .min_by(|a, b| a.position.dist(p).total_cmp(&b.position.dist(p)))
            .map(|c| c.id)
    }

    fn select(&mut self, cx: &mut EditorContext, id: Id) {
        self.selected = Some(id);
        if let Some(c) = cx.project.camera(id) {
            cx.status = format!("Camera: {}", c.name);
        }
    }

    /// Tab / Shift+Tab: the next camera on the floor (C-29).
    fn cycle(&mut self, cx: &mut EditorContext, back: bool) -> bool {
        let ids: Vec<Id> = cx.project.cameras_on(cx.floor).map(|c| c.id).collect();
        if ids.is_empty() {
            return false;
        }
        let at = self.selected.and_then(|s| ids.iter().position(|i| *i == s));
        let next = match (at, back) {
            (None, false) => 0,
            (None, true) => ids.len() - 1,
            (Some(i), false) => (i + 1) % ids.len(),
            (Some(i), true) => (i + ids.len() - 1) % ids.len(),
        };
        self.select(cx, ids[next]);
        true
    }

    fn create_full_camera(&mut self, cx: &mut EditorContext, at: Point, to: Point) -> ToolResult {
        if !self.layer_ok(cx) {
            return ToolResult::consumed();
        }
        let dir = if at.dist(to) < CLICK_DIST {
            DEFAULT_DIRECTION_DEG
        } else {
            (to - at).angle().to_degrees()
        };
        let name = self.unique_name(cx, "Camera");
        let defaults = camera_defaults();
        let mut cam = CameraObject::new(CameraKind::FullCamera, at, dir, name, cx.floor);
        cam.eye_height = defaults.eye_height;
        cam.fov_deg = defaults.fov_deg;
        self.add_camera(cx, cam)
    }

    fn create_section(&mut self, cx: &mut EditorContext, a: Point, b: Point) -> ToolResult {
        let Some((centre, dir, _)) = section_from_drag(a, b) else {
            cx.status = "Drag a longer line to define the cross section".into();
            return ToolResult::consumed();
        };
        if !self.layer_ok(cx) {
            return ToolResult::consumed();
        }
        let back = (self.variant == CameraVariant::BackClippedSection).then_some(DEFAULT_BACK_CLIP);
        let name = self.unique_name(cx, "Section");
        let mut cam = CameraObject::new(
            CameraKind::CrossSection { back_clip: back },
            centre,
            dir,
            name,
            cx.floor,
        );
        cam.section = Some(SectionLine {
            a,
            b,
            back_clip: back,
        });
        self.add_camera(cx, cam)
    }

    fn add_camera(&mut self, cx: &mut EditorContext, cam: CameraObject) -> ToolResult {
        cx.begin_change("Create Camera");
        if cx.project.layers.get(CAMERA_LAYER).is_none() {
            cx.project
                .layers
                .add(Layer::new(CAMERA_LAYER, [0x2F, 0x6C, 0xB3], 25));
        }
        let id = cx.project.add_camera(cam);
        self.select(cx, id);
        self.outbox.post(ViewRequest::ShowCamera(id));
        ToolResult {
            switch_to: Some(ToolId::Select),
            ..ToolResult::committed("Create Camera")
        }
    }

    /// Wall Elevation (C-20): click a wall.
    fn click_wall_elevation(&mut self, cx: &mut EditorContext, at: Point) -> ToolResult {
        let tol = cx.pick_tol();
        let Some(wall) = wall_at(&cx.project, cx.floor, at, tol).cloned() else {
            cx.status = "Click a wall to make its elevation".into();
            return ToolResult::consumed();
        };
        if !self.layer_ok(cx) {
            return ToolResult::consumed();
        }
        let name = self.unique_name(cx, "Wall Elevation");
        let cam = wall_elevation(&wall, at, cx.floor, &name);
        self.add_camera(cx, cam)
    }

    /// Auto Elevations and Auto Back-Clipped Elevations (C-21).
    fn click_auto(&mut self, cx: &mut EditorContext) -> ToolResult {
        if building_bounds(&cx.project).is_none() {
            cx.status = "Draw some walls first".into();
            return ToolResult::consumed();
        }
        if !self.layer_ok(cx) {
            return ToolResult::consumed();
        }
        cx.begin_change(self.variant.label());
        let ids = add_auto_elevations(
            &mut cx.project,
            cx.floor,
            self.variant == CameraVariant::AutoBackclipped,
        );
        if let Some(first) = ids.first() {
            self.select(cx, *first);
            self.outbox.post(ViewRequest::ShowCamera(*first));
        }
        cx.status = format!("{}: {} cameras", self.variant.label(), ids.len());
        ToolResult {
            switch_to: Some(ToolId::Select),
            ..ToolResult::committed(self.variant.label())
        }
    }

    /// Adds a node to the walkthrough being drawn (a click on the last node
    /// is the second half of a double click and is ignored).
    fn walk_click(&mut self, cx: &mut EditorContext, at: Point) -> ToolResult {
        let tol = cx.pick_tol();
        match &mut self.walk {
            Some(w) => {
                if w.nodes.last().is_some_and(|l| l.dist(at) <= tol) {
                    return ToolResult::consumed();
                }
                w.nodes.push(at);
                w.looks.push(None);
                w.pressing = true;
                w.cursor = at;
            }
            None => {
                self.walk = Some(WalkPlacing {
                    nodes: vec![at],
                    looks: vec![None],
                    pressing: true,
                    cursor: at,
                });
            }
        }
        cx.status = "Walkthrough: click the next node, double-click to finish".into();
        ToolResult::consumed()
    }

    /// Finishes the walkthrough path (double click or Enter).
    fn finish_walk(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(w) = self.walk.take() else {
            return ToolResult::ignored();
        };
        if w.nodes.len() < 2 {
            cx.status = "A walkthrough needs at least two nodes".into();
            return ToolResult::consumed();
        }
        if !self.layer_ok(cx) {
            return ToolResult::consumed();
        }
        let name = self.unique_name(cx, "Walkthrough");
        let mut cam =
            CameraObject::walkthrough(w.nodes, camera_defaults().eye_height, name, cx.floor);
        for (n, look) in cam.path_nodes.iter_mut().zip(w.looks) {
            *n = WalkNode {
                look_deg: look,
                ..*n
            };
        }
        self.add_camera(cx, cam)
    }

    fn light_at(&self, cx: &EditorContext, p: Point) -> Option<Id> {
        let tol = cx.pick_tol() + LIGHT_RADIUS;
        cx.project
            .lights()
            .into_iter()
            .filter(|l| l.floor == cx.floor && l.position.dist(p) <= tol)
            .min_by(|a, b| a.position.dist(p).total_cmp(&b.position.dist(p)))
            .map(|l| l.id)
    }

    /// Add Lights: a click on a light selects it (and a second click opens
    /// Adjust Lights); a click elsewhere places a light.
    fn click_light(&mut self, cx: &mut EditorContext, at: Point, snapped: Point) -> ToolResult {
        if let Some(id) = self.light_at(cx, at) {
            self.selected_light = Some(id);
            self.light_edit = Some(LightEdit { id, began: false });
            cx.status =
                "Light selected: drag to move, Delete removes it, double-click to adjust".into();
            return ToolResult::consumed();
        }
        let d = crate::dialogs::camera::light_defaults();
        let ceiling = cx.floor().ceiling_height;
        let n = cx.project.lights().len() + 1;
        let mut light = PlanLight::new(snapped, d.height.unwrap_or((ceiling - 12.0).max(12.0)));
        light.intensity = d.intensity;
        light.color = d.color;
        light.name = format!("Light {n}");
        cx.begin_change("Add Light");
        let floor = cx.floor;
        let id = cx.project.add_light(floor, light);
        self.selected_light = id;
        cx.status = "Light added".into();
        ToolResult::committed("Add Light")
    }

    fn click_overview(&mut self, cx: &mut EditorContext) -> ToolResult {
        let (mode, floor) = match self.variant {
            CameraVariant::FloorOverview => (plan_view3d::CameraMode::Orbit, Some(cx.floor)),
            CameraVariant::DollHouse => (plan_view3d::CameraMode::DollHouse, None),
            _ => (plan_view3d::CameraMode::Orbit, None),
        };
        self.outbox.post(ViewRequest::Mode { mode, floor });
        cx.status = format!("{} opened", self.variant.label());
        ToolResult {
            switch_to: Some(ToolId::Select),
            ..ToolResult::consumed()
        }
    }
}

impl Tool for CameraTool {
    fn id(&self) -> ToolId {
        ToolId::Camera
    }

    fn name(&self) -> &'static str {
        self.variant.label()
    }

    fn hint(&self) -> String {
        match self.variant {
            CameraVariant::FullCamera => {
                "Full Camera: press at the eye, drag to aim, release to open the view".into()
            }
            v if v.is_overview() => format!("{}: click in the plan to open the view", v.label()),
            CameraVariant::WallElevation => {
                "Wall Elevation: click the wall face you want to see".into()
            }
            v if v.is_auto() => format!("{}: click once to make the four elevations", v.label()),
            CameraVariant::Walkthrough => {
                "Walkthrough: click each node, double-click to finish; drag at a node to aim it"
                    .into()
            }
            CameraVariant::AddLights => "Add Lights: click to place a light".into(),
            _ => "Cross Section: drag the cut line; the view looks to the left of the drag".into(),
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::CameraVariant(v) = id {
            self.variant = v;
            self.reset_gestures();
        } else if let Some(v) = take_next_variant() {
            self.variant = v;
            self.reset_gestures();
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.reset_gestures();
        cx.status = self.hint();
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.reset_gestures();
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        // A walkthrough in progress takes every click; Add Lights places lights.
        if self.walk.is_some() {
            return self.walk_click(cx, p.snapped);
        }
        if self.variant == CameraVariant::AddLights {
            return self.click_light(cx, p.world, p.snapped);
        }
        // Handles of the selected camera, then any camera symbol, then place.
        let handle = self
            .selected
            .and_then(|id| cx.project.camera(id))
            .filter(|c| c.floor == cx.floor)
            .and_then(|c| hit_handle(c, p.world, cx.pick_tol()).map(|h| (c.id, h)));
        if let Some((id, handle)) = handle {
            if self.layer_ok(cx) {
                self.editing = Some(Editing {
                    id,
                    handle,
                    began: false,
                });
            }
            return ToolResult::consumed();
        }
        if let Some(id) = self.camera_at(cx, p.world) {
            self.select(cx, id);
            if self.layer_ok(cx) {
                self.editing = Some(Editing {
                    id,
                    handle: CamHandle::Move,
                    began: false,
                });
            }
            return ToolResult::consumed();
        }
        self.selected = None;
        if self.variant.is_overview() {
            return self.click_overview(cx);
        }
        match self.variant {
            CameraVariant::WallElevation => return self.click_wall_elevation(cx, p.world),
            CameraVariant::AutoElevation | CameraVariant::AutoBackclipped => {
                return self.click_auto(cx)
            }
            CameraVariant::Walkthrough => return self.walk_click(cx, p.snapped),
            _ => {}
        }
        self.placing = Some(Placing {
            start: p.snapped,
            current: p.snapped,
        });
        ToolResult::consumed()
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(pl) = &mut self.placing {
            pl.current = p.snapped;
            let v = p.snapped - pl.start;
            cx.readout = Some(if self.variant.is_section() {
                format!("Length: {}", cx.fmt_dim(v.length()))
            } else {
                format!("Direction: {:.0}\u{B0}", v.angle().to_degrees())
            });
            return ToolResult::consumed();
        }
        if let Some(w) = &mut self.walk {
            w.cursor = p.snapped;
            if w.pressing && p.down {
                let far = cx.pick_tol() * 2.0;
                if let (Some(last), Some(look)) = (w.nodes.last().copied(), w.looks.last_mut()) {
                    if last.dist(p.world) > far {
                        *look = Some((p.world - last).angle().to_degrees());
                    }
                }
            }
            return ToolResult::consumed();
        }
        if let Some(le) = &mut self.light_edit {
            if !p.down {
                return ToolResult::ignored();
            }
            if !le.began {
                cx.begin_change("Move Light");
                le.began = true;
            }
            let (id, to) = (le.id, p.snapped);
            cx.project.update_light(id, |l| l.position = to);
            return ToolResult::consumed();
        }
        let Some(ed) = &mut self.editing else {
            return ToolResult::ignored();
        };
        if !p.down {
            return ToolResult::ignored();
        }
        if !ed.began {
            cx.begin_change(match ed.handle {
                CamHandle::Move => "Move Camera",
                CamHandle::Aim => "Rotate Camera",
                CamHandle::Clip => "Change Camera Clip Distance",
                CamHandle::EndA | CamHandle::EndB => "Stretch Cross Section",
            });
            ed.began = true;
        }
        let (id, handle) = (ed.id, ed.handle);
        let to = if handle == CamHandle::Move {
            p.snapped
        } else {
            p.world
        };
        let snap_deg = p
            .modifiers
            .shift
            .then_some(cx.defaults.editing.angle_snap_deg);
        cx.project
            .update_camera(id, |c| apply_handle_with(c, handle, to, snap_deg));
        // Update a 3D view of this camera live while the mouse is down (C-26).
        self.outbox.post(ViewRequest::RefreshCamera(id));
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        cx.readout = None;
        if let Some(w) = &mut self.walk {
            w.pressing = false;
            return ToolResult::consumed();
        }
        if let Some(le) = self.light_edit.take() {
            if le.began {
                cx.mark_dirty();
                return ToolResult::committed("Move Light");
            }
            return ToolResult::consumed();
        }
        if let Some(ed) = self.editing.take() {
            if ed.began {
                cx.mark_dirty();
                return ToolResult::committed("Edit Camera");
            }
            return ToolResult::consumed();
        }
        let Some(pl) = self.placing.take() else {
            return ToolResult::ignored();
        };
        let end = p.snapped;
        if self.variant.is_section() {
            self.create_section(cx, pl.start, end)
        } else {
            self.create_full_camera(cx, pl.start, end)
        }
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.walk.is_some() {
            return self.finish_walk(cx);
        }
        if self.variant == CameraVariant::AddLights {
            return match self.light_at(cx, p.world) {
                Some(id) => {
                    self.selected_light = Some(id);
                    self.light_edit = None;
                    self.outbox.post(ViewRequest::OpenAdjustLights(Some(id)));
                    ToolResult::consumed()
                }
                None => ToolResult::ignored(),
            };
        }
        match self.camera_at(cx, p.world) {
            Some(id) => {
                self.select(cx, id);
                self.editing = None;
                self.outbox.post(ViewRequest::OpenCameraSpec(id));
                ToolResult::consumed()
            }
            None => ToolResult::ignored(),
        }
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if self.walk.is_some() {
            if k.is(egui::Key::Escape) {
                self.walk = None;
                return ToolResult::consumed();
            }
            if k.is(egui::Key::Enter) {
                return self.finish_walk(cx);
            }
            if k.is(egui::Key::Backspace) || k.is(egui::Key::Delete) {
                if let Some(w) = &mut self.walk {
                    w.nodes.pop();
                    w.looks.pop();
                    if w.nodes.is_empty() {
                        self.walk = None;
                    }
                }
                return ToolResult::consumed();
            }
        }
        if (k.is(egui::Key::Delete) || k.is(egui::Key::Backspace)) && self.selected_light.is_some()
        {
            if let Some(id) = self.selected_light.take() {
                if cx.project.light(id).is_some() {
                    cx.begin_change("Delete Light");
                    cx.project.remove_light(id);
                    return ToolResult::committed("Delete Light");
                }
            }
        }
        if k.is(egui::Key::Escape) {
            if self.light_edit.take().is_some() || self.selected_light.take().is_some() {
                return ToolResult::consumed();
            }
            if self.placing.take().is_some() {
                cx.readout = None;
                return ToolResult::consumed();
            }
            if let Some(ed) = self.editing.take() {
                if ed.began {
                    cx.undo();
                }
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        if k.is(egui::Key::Tab) {
            return if self.cycle(cx, k.modifiers.shift) {
                ToolResult::consumed()
            } else {
                ToolResult::ignored()
            };
        }
        if k.is(egui::Key::Delete) || k.is(egui::Key::Backspace) {
            let Some(id) = self.selected.filter(|id| cx.project.camera(*id).is_some()) else {
                return ToolResult::ignored();
            };
            if !self.layer_ok(cx) {
                return ToolResult::consumed();
            }
            cx.begin_change("Delete Camera");
            cx.project.remove_camera(id);
            self.selected = None;
            self.outbox.post(ViewRequest::CameraDeleted(id));
            return ToolResult::committed("Delete Camera");
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        draw_camera_symbols(cx, painter, cam, self.selected);
        if let Some(l) = self.selected_light.and_then(|id| cx.project.light(id)) {
            if l.floor == cx.floor {
                let c = cam.world_to_screen(l.position);
                let r = (LIGHT_RADIUS * cam.px_per_in) as f32 + 4.0;
                painter.circle_stroke(c, r, Stroke::new(1.5_f32, cx.palette.selection));
            }
        }
        if let Some(w) = &self.walk {
            let mut pts: Vec<Point> = w.nodes.clone();
            pts.push(w.cursor);
            let ghost = Stroke::new(1.5_f32, CAMERA_BLUE);
            painter.add(Shape::line(polygon(cam, &pts), ghost));
            for (n, look) in w.nodes.iter().zip(&w.looks) {
                painter.circle_filled(cam.world_to_screen(*n), 3.5, CAMERA_BLUE);
                if let Some(d) = look {
                    let a = d.to_radians();
                    arrow(painter, cam, *n, Point::new(a.cos(), a.sin()), ghost);
                }
            }
            return;
        }
        let Some(pl) = &self.placing else { return };
        let a = cam.world_to_screen(pl.start);
        let b = cam.world_to_screen(pl.current);
        let ghost = Stroke::new(1.5_f32, CAMERA_BLUE);
        if self.variant.is_section() {
            painter.line_segment([a, b], ghost);
            if let Some((centre, dir, _)) = section_from_drag(pl.start, pl.current) {
                let d = Point::new(dir.to_radians().cos(), dir.to_radians().sin());
                arrow(painter, cam, centre, d, ghost);
            }
        } else {
            let dir = if pl.start.dist(pl.current) < CLICK_DIST {
                DEFAULT_DIRECTION_DEG
            } else {
                (pl.current - pl.start).angle().to_degrees()
            };
            let ghost_cam = CameraObject::new(CameraKind::FullCamera, pl.start, dir, "", cx.floor);
            draw_one(painter, cam, &ghost_cam, false);
        }
    }

    fn edit_toolbar(&self, _cx: &EditorContext) -> Vec<crate::editor::EditAction> {
        Vec::new()
    }
}

// ----- drawing (C-24) -----

fn arrow(painter: &egui::Painter, cam: &Camera, from: Point, dir: Point, stroke: Stroke) {
    let tip = from + dir * ARROW_LEN;
    let (a, b) = (cam.world_to_screen(from), cam.world_to_screen(tip));
    painter.line_segment([a, b], stroke);
    let back = dir.perp();
    for side in [1.0, -1.0] {
        let wing = tip - dir * 8.0 + back * (4.0 * side);
        painter.line_segment([b, cam.world_to_screen(wing)], stroke);
    }
}

fn polygon(cam: &Camera, pts: &[Point]) -> Vec<Pos2> {
    pts.iter().map(|p| cam.world_to_screen(*p)).collect()
}

fn draw_one(painter: &egui::Painter, cam: &Camera, c: &CameraObject, selected: bool) {
    let line = Stroke::new(if selected { 2.0_f32 } else { 1.2_f32 }, CAMERA_BLUE);
    if c.kind == CameraKind::Walkthrough {
        painter.add(Shape::line(polygon(cam, &c.path), line));
        for n in &c.path {
            painter.circle_filled(cam.world_to_screen(*n), 3.0, CAMERA_BLUE);
        }
        painter.add(Shape::convex_polygon(
            polygon(cam, &c.triangle_points()),
            CAMERA_BLUE,
            Stroke::new(1.0_f32, Color32::WHITE),
        ));
        return;
    }
    if is_section(c) {
        let (a, b) = section_line(c);
        painter.line_segment([cam.world_to_screen(a), cam.world_to_screen(b)], line);
        let t = section_tangent(c);
        for end in [a, b] {
            let tick = c.direction() * 6.0;
            painter.line_segment(
                [
                    cam.world_to_screen(end - tick),
                    cam.world_to_screen(end + tick),
                ],
                line,
            );
        }
        arrow(painter, cam, c.position, c.direction(), line);
        if let Some(bc) = back_clip(c) {
            let off = c.direction() * bc;
            let half = t * (section_width(c) * 0.5);
            painter.line_segment(
                [
                    cam.world_to_screen(c.position + off - half),
                    cam.world_to_screen(c.position + off + half),
                ],
                Stroke::new(1.0_f32, CAMERA_BLUE.gamma_multiply(0.6)),
            );
        }
        return;
    }
    let cone = c.cone_points();
    let fill = CAMERA_BLUE.gamma_multiply(if selected { 0.22 } else { 0.12 });
    painter.add(Shape::convex_polygon(
        polygon(cam, &cone),
        fill,
        Stroke::new(1.0_f32, CAMERA_BLUE.gamma_multiply(0.8)),
    ));
    painter.add(Shape::convex_polygon(
        polygon(cam, &c.triangle_points()),
        CAMERA_BLUE,
        Stroke::new(1.0_f32, Color32::WHITE),
    ));
}

/// The plan symbol of a light: a ring with eight rays, grayed when off.
fn draw_light_symbols(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    for l in cx.project.lights().iter().filter(|l| l.floor == cx.floor) {
        let ink = Color32::from_rgb(0xC8, 0xA0, 0x20);
        let ink = if l.enabled {
            ink
        } else {
            ink.gamma_multiply(0.4)
        };
        let stroke = Stroke::new(1.2_f32, ink);
        let c = cam.world_to_screen(l.position);
        let r = (LIGHT_RADIUS * 0.5 * cam.px_per_in) as f32;
        painter.circle_stroke(c, r.max(2.5), stroke);
        for i in 0..8 {
            let a = f64::from(i) * std::f64::consts::FRAC_PI_4;
            let d = egui::vec2(a.cos() as f32, -a.sin() as f32);
            painter.line_segment(
                [c + d * (r.max(2.5) + 1.5), c + d * (r.max(2.5) * 1.9 + 2.0)],
                stroke,
            );
        }
    }
}

/// Draws every camera symbol of the current floor on layer "Cameras" (C-24),
/// with the handles of `selected` (C-25..C-28). The Camera tool calls this
/// from its overlay; the plan renderer may call it too so symbols also show
/// while another tool is active.
pub fn draw_camera_symbols(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    selected: Option<Id>,
) {
    if !cx.project.layers.is_visible(CAMERA_LAYER) {
        return;
    }
    for c in cx.project.cameras_on(cx.floor) {
        draw_one(painter, cam, c, selected == Some(c.id));
    }
    draw_light_symbols(cx, painter, cam);
    let Some(c) = selected.and_then(|id| cx.project.camera(id)) else {
        return;
    };
    if c.floor != cx.floor {
        return;
    }
    for (h, at) in handles_of(c) {
        let s = cam.world_to_screen(at);
        let r = egui::Rect::from_center_size(s, egui::Vec2::splat(9.0));
        painter.rect_filled(r, 0.0, cx.palette.background);
        let color = if h == CamHandle::Move {
            CAMERA_BLUE
        } else {
            cx.palette.selection
        };
        painter.rect_stroke(r, 0.0, Stroke::new(1.5_f32, color), StrokeKind::Inside);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use crate::shell::view3d_panel::View3dState;
    use plan_view3d::CameraMode;

    fn setup() -> (EditorContext, CameraTool, Outbox) {
        let cx = EditorContext::new(plan_defaults::embedded());
        let outbox = Outbox::default();
        (cx, CameraTool::with_outbox(outbox.clone()), outbox)
    }

    fn down(cx: &mut EditorContext, t: &mut CameraTool, p: Point) -> ToolResult {
        let ev = PointerEvent::at(cx, p);
        t.pointer_down(cx, ev)
    }

    fn up(cx: &mut EditorContext, t: &mut CameraTool, p: Point) -> ToolResult {
        let ev = PointerEvent::at(cx, p);
        t.pointer_up(cx, ev)
    }

    fn double(cx: &mut EditorContext, t: &mut CameraTool, p: Point) -> ToolResult {
        let ev = PointerEvent::at(cx, p);
        t.double_click(cx, ev)
    }

    fn drag(cx: &mut EditorContext, t: &mut CameraTool, a: Point, b: Point) -> ToolResult {
        let down = PointerEvent::at(cx, a).with_down(true);
        t.pointer_down(cx, down);
        let mid = PointerEvent::at(cx, b).with_down(true);
        t.pointer_move(cx, mid);
        t.pointer_up(cx, PointerEvent::at(cx, b))
    }

    #[test]
    fn full_camera_drag_sets_position_and_direction() {
        let (mut cx, mut t, outbox) = setup();
        let res = drag(
            &mut cx,
            &mut t,
            Point::new(100.0, 100.0),
            Point::new(100.0, 220.0),
        );
        assert_eq!(res.commit.as_deref(), Some("Create Camera"));
        assert_eq!(res.switch_to, Some(ToolId::Select));
        let c = &cx.project.cameras[0];
        assert_eq!(c.kind, CameraKind::FullCamera);
        assert_eq!(c.position, Point::new(100.0, 100.0));
        assert!((c.direction_deg - 90.0).abs() < 1e-9);
        assert_eq!(c.eye_height, 66.0);
        assert_eq!(c.name, "Camera 1");
        assert!(cx.project.layers.get(CAMERA_LAYER).is_some());
        assert_eq!(outbox.take(), vec![ViewRequest::ShowCamera(c.id)]);
        // The release is one undo step.
        cx.undo();
        assert!(cx.project.cameras.is_empty());
    }

    #[test]
    fn full_camera_click_uses_the_default_direction() {
        let (mut cx, mut t, _o) = setup();
        let p = Point::new(40.0, 40.0);
        down(&mut cx, &mut t, p);
        up(&mut cx, &mut t, p);
        assert_eq!(cx.project.cameras[0].direction_deg, DEFAULT_DIRECTION_DEG);
    }

    #[test]
    fn full_camera_opens_the_3d_view_at_the_eye() {
        let (mut cx, mut t, outbox) = setup();
        drag(
            &mut cx,
            &mut t,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
        );
        let mut view = View3dState::with_inbox(outbox);
        view.drain(&cx.project);
        assert!(view.active);
        assert_eq!(view.mode, CameraMode::FullCamera);
        assert_eq!(view.active_camera, Some(cx.project.cameras[0].id));
    }

    #[test]
    fn overview_click_sets_the_view_mode() {
        for (variant, mode, floor) in [
            (CameraVariant::FullOverview, CameraMode::Orbit, None),
            (CameraVariant::DollHouse, CameraMode::DollHouse, None),
            (CameraVariant::FloorOverview, CameraMode::Orbit, Some(0)),
        ] {
            let (mut cx, mut t, outbox) = setup();
            t.variant = variant;
            let res = down(&mut cx, &mut t, Point::new(5.0, 5.0));
            assert_eq!(res.switch_to, Some(ToolId::Select));
            assert!(cx.project.cameras.is_empty(), "overviews create no camera");
            let mut view = View3dState::with_inbox(outbox);
            assert!(!view.active);
            view.drain(&cx.project);
            assert!(view.active);
            assert_eq!(view.mode, mode);
            assert_eq!(view.scope_floor, floor);
        }
    }

    #[test]
    fn cross_section_drag_picks_the_elevation_mode() {
        // West to east looks north: the front elevation (viewer on the south).
        let cases = [
            ((0.0, 0.0), (100.0, 0.0), CameraMode::ElevationFront, 90.0),
            ((100.0, 0.0), (0.0, 0.0), CameraMode::ElevationBack, -90.0),
            ((0.0, 0.0), (0.0, 100.0), CameraMode::ElevationRight, 180.0),
            ((0.0, 100.0), (0.0, 0.0), CameraMode::ElevationLeft, 0.0),
        ];
        for (a, b, mode, dir) in cases {
            let (mut cx, mut t, outbox) = setup();
            t.variant = CameraVariant::CrossSection;
            drag(&mut cx, &mut t, Point::new(a.0, a.1), Point::new(b.0, b.1));
            let c = &cx.project.cameras[0];
            assert_eq!(c.kind, CameraKind::CrossSection { back_clip: None });
            assert!((c.direction_deg - dir).abs() < 1e-6, "{a:?}->{b:?}");
            assert!((section_width(c) - 100.0).abs() < 1e-9);
            let (la, lb) = section_line(c);
            assert!(la.dist(Point::new(a.0, a.1)) < 1e-6 && lb.dist(Point::new(b.0, b.1)) < 1e-6);
            let mut view = View3dState::with_inbox(outbox);
            view.drain(&cx.project);
            assert_eq!(view.mode, mode, "{a:?}->{b:?}");
            assert!(view.section.is_some());
        }
    }

    #[test]
    fn back_clipped_section_has_a_back_clip_handle() {
        let (mut cx, mut t, _o) = setup();
        t.variant = CameraVariant::BackClippedSection;
        drag(
            &mut cx,
            &mut t,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
        );
        let c = &cx.project.cameras[0];
        assert_eq!(back_clip(c), Some(DEFAULT_BACK_CLIP));
        assert!(handles_of(c).iter().any(|(h, _)| *h == CamHandle::Clip));
    }

    #[test]
    fn a_short_section_drag_creates_nothing() {
        let (mut cx, mut t, _o) = setup();
        t.variant = CameraVariant::CrossSection;
        let res = drag(&mut cx, &mut t, Point::ZERO, Point::new(4.0, 0.0));
        assert!(res.commit.is_none());
        assert!(cx.project.cameras.is_empty());
    }

    #[test]
    fn handles_move_aim_and_clip() {
        let mut c = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "c", 0);
        apply_handle(&mut c, CamHandle::Move, Point::new(10.0, 10.0), false);
        assert_eq!(c.position, Point::new(10.0, 10.0));
        apply_handle(&mut c, CamHandle::Aim, Point::new(10.0, 110.0), false);
        assert!((c.direction_deg - 90.0).abs() < 1e-9);
        // Shift snaps the aim to 15 degrees (C-27).
        apply_handle(&mut c, CamHandle::Aim, Point::new(110.0, 10.0 + 38.0), true);
        assert!((c.direction_deg - 15.0).abs() < 1e-9);
        c.direction_deg = 0.0;
        apply_handle(&mut c, CamHandle::Clip, Point::new(210.0, 10.0), false);
        assert_eq!(c.clip_distance, Some(200.0));
        apply_handle(&mut c, CamHandle::Clip, Point::new(12.0, 10.0), false);
        assert_eq!(c.clip_distance, Some(MIN_CLIP));
    }

    #[test]
    fn dragging_a_selected_camera_handle_edits_it_with_one_undo_step() {
        let (mut cx, mut t, _o) = setup();
        drag(
            &mut cx,
            &mut t,
            Point::new(100.0, 100.0),
            Point::new(200.0, 100.0),
        );
        let id = cx.project.cameras[0].id;
        assert_eq!(t.selected(), Some(id));
        // Grab the glyph centre and drag it.
        let down = PointerEvent::at(&cx, Point::new(101.0, 100.0)).with_down(true);
        t.pointer_down(&mut cx, down);
        let mv = PointerEvent::at(&cx, Point::new(150.0, 160.0)).with_down(true);
        t.pointer_move(&mut cx, mv);
        let res = up(&mut cx, &mut t, Point::new(150.0, 160.0));
        assert_eq!(res.commit.as_deref(), Some("Edit Camera"));
        assert_eq!(
            cx.project.camera(id).unwrap().position,
            Point::new(150.0, 160.0)
        );
        cx.undo();
        assert_eq!(
            cx.project.camera(id).unwrap().position,
            Point::new(100.0, 100.0)
        );
    }

    #[test]
    fn tab_cycles_and_delete_removes() {
        let (mut cx, mut t, _o) = setup();
        for x in [0.0, 100.0, 200.0] {
            drag(&mut cx, &mut t, Point::new(x, 0.0), Point::new(x, 50.0));
        }
        let ids: Vec<Id> = cx.project.cameras.iter().map(|c| c.id).collect();
        t.selected = None;
        let tab = KeyEvent::key(egui::Key::Tab);
        assert!(t.key(&mut cx, tab.clone()).consumed);
        assert_eq!(t.selected(), Some(ids[0]));
        t.key(&mut cx, tab.clone());
        t.key(&mut cx, tab.clone());
        t.key(&mut cx, tab);
        assert_eq!(t.selected(), Some(ids[0]), "wraps around");
        let res = t.key(&mut cx, KeyEvent::key(egui::Key::Delete));
        assert_eq!(res.commit.as_deref(), Some("Delete Camera"));
        assert_eq!(cx.project.cameras.len(), 2);
        assert!(cx.project.camera(ids[0]).is_none());
        // With nothing selected Delete is left to the shell.
        assert!(!t.key(&mut cx, KeyEvent::key(egui::Key::Delete)).consumed);
    }

    #[test]
    fn double_click_opens_the_camera_specification() {
        let (mut cx, mut t, outbox) = setup();
        drag(
            &mut cx,
            &mut t,
            Point::new(100.0, 100.0),
            Point::new(200.0, 100.0),
        );
        outbox.take();
        let id = cx.project.cameras[0].id;
        let res = double(&mut cx, &mut t, Point::new(100.0, 101.0));
        assert!(res.consumed);
        assert_eq!(outbox.take(), vec![ViewRequest::OpenCameraSpec(id)]);
        let miss = double(&mut cx, &mut t, Point::new(900.0, 900.0));
        assert!(!miss.consumed);
    }

    #[test]
    fn escape_cancels_a_drag() {
        let (mut cx, mut t, _o) = setup();
        let down = PointerEvent::at(&cx, Point::ZERO).with_down(true);
        t.pointer_down(&mut cx, down);
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        let res = up(&mut cx, &mut t, Point::new(0.0, 50.0));
        assert!(!res.consumed && cx.project.cameras.is_empty());
        // Nothing to cancel: the shell gets Esc.
        assert!(!t.key(&mut cx, KeyEvent::escape()).consumed);
    }

    #[test]
    fn locked_camera_layer_blocks_placement() {
        let (mut cx, mut t, _o) = setup();
        cx.project
            .layers
            .add(Layer::new(CAMERA_LAYER, [0, 0, 255], 25));
        cx.project.layers.set_locked(CAMERA_LAYER, true);
        drag(&mut cx, &mut t, Point::ZERO, Point::new(0.0, 80.0));
        assert!(cx.project.cameras.is_empty());
        assert!(cx.status.contains("locked"));
    }

    #[test]
    fn variant_is_chosen_through_set_variant() {
        let mut t = CameraTool::with_outbox(Outbox::default());
        set_next_variant(CameraVariant::DollHouse);
        t.set_variant(ToolId::Camera);
        assert_eq!(t.variant, CameraVariant::DollHouse);
        // Without a pending choice the variant stays.
        t.set_variant(ToolId::Camera);
        assert_eq!(t.variant, CameraVariant::DollHouse);
    }

    fn legacy_section() -> CameraObject {
        // West to east along y = -50, 400 long, looking north: the old
        // encoding (length in `fov_deg`, no typed section).
        CameraObject {
            fov_deg: 400.0,
            ..CameraObject::new(
                CameraKind::CrossSection {
                    back_clip: Some(80.0),
                },
                Point::new(120.0, -50.0),
                90.0,
                "Section 1",
                0,
            )
        }
    }

    #[test]
    fn a_new_section_stores_its_cut_line_and_leaves_fov_alone() {
        let (mut cx, mut t, _o) = setup();
        t.variant = CameraVariant::BackClippedSection;
        drag(
            &mut cx,
            &mut t,
            Point::new(10.0, 20.0),
            Point::new(110.0, 20.0),
        );
        let c = &cx.project.cameras[0];
        let s = c.section.expect("typed cut line");
        assert_eq!(
            (s.a, s.b),
            (Point::new(10.0, 20.0), Point::new(110.0, 20.0))
        );
        assert_eq!(s.back_clip, Some(DEFAULT_BACK_CLIP));
        assert_eq!(
            c.kind,
            CameraKind::CrossSection {
                back_clip: Some(DEFAULT_BACK_CLIP)
            }
        );
        assert_eq!(
            c.fov_deg, DEFAULT_FOV_DEG,
            "fov_deg no longer carries the length"
        );
        assert_eq!(c.position, Point::new(60.0, 20.0));
        assert!((c.direction_deg - 90.0).abs() < 1e-9);
        assert!((section_width(c) - 100.0).abs() < 1e-9);
        // The section survives the file.
        let back = plan_core::Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert_eq!(back.cameras[0].section, c.section);
    }

    #[test]
    fn the_old_section_encoding_is_still_read() {
        let c = legacy_section();
        assert!(c.section.is_none());
        assert!((section_width(&c) - 400.0).abs() < 1e-9);
        let (a, b) = section_line(&c);
        assert!(a.dist(Point::new(-80.0, -50.0)) < 1e-9 && b.dist(Point::new(320.0, -50.0)) < 1e-9);
        assert_eq!(back_clip(&c), Some(80.0));
        assert!(handles_of(&c).iter().any(|(h, _)| *h == CamHandle::Clip));
    }

    #[test]
    fn editing_an_old_section_upgrades_it_without_moving_it() {
        let mut c = legacy_section();
        let before = section_line(&c);
        upgrade_section(&mut c);
        let s = c.section.unwrap();
        assert_eq!((s.a, s.b), before);
        assert_eq!(s.back_clip, Some(80.0));
        assert_eq!(c.fov_deg, DEFAULT_FOV_DEG);
        assert_eq!(section_line(&c), before);

        // Handles on an old camera upgrade it first, then edit.
        let mut c = legacy_section();
        apply_handle(&mut c, CamHandle::Move, Point::new(130.0, -40.0), false);
        let (a, b) = section_line(&c);
        assert!(a.dist(Point::new(-70.0, -40.0)) < 1e-9 && b.dist(Point::new(330.0, -40.0)) < 1e-9);
        assert!(c.section.is_some());
    }

    #[test]
    fn section_handles_keep_the_line_and_the_camera_in_step() {
        let mut c = legacy_section();
        // Stretch: the line grows symmetrically about the centre.
        apply_handle(&mut c, CamHandle::EndB, Point::new(170.0, -50.0), false);
        assert!((section_width(&c) - 100.0).abs() < 1e-9);
        let (a, b) = section_line(&c);
        assert!(a.dist(Point::new(70.0, -50.0)) < 1e-9 && b.dist(Point::new(170.0, -50.0)) < 1e-9);
        // Aim: the line turns about the centre, keeping its length; the view
        // direction follows and the stored line stays perpendicular to it.
        apply_handle(&mut c, CamHandle::Aim, Point::new(120.0, 100.0), false);
        assert!((c.direction_deg - 90.0).abs() < 1e-9);
        apply_handle(&mut c, CamHandle::Aim, Point::new(220.0, -50.0), false);
        assert!(c.direction_deg.abs() < 1e-9);
        let (a, b) = section_line(&c);
        assert!((a.dist(b) - 100.0).abs() < 1e-9);
        assert!((a.x - 120.0).abs() < 1e-9 && (b.x - 120.0).abs() < 1e-9);
        assert_eq!(c.position, Point::new(120.0, -50.0));
        // Clip: both the kind and the typed line carry it.
        apply_handle(&mut c, CamHandle::Clip, Point::new(320.0, -50.0), false);
        assert_eq!(back_clip(&c), Some(200.0));
        assert_eq!(
            c.kind,
            CameraKind::CrossSection {
                back_clip: Some(200.0)
            }
        );
        assert_eq!(c.section.unwrap().back_clip, Some(200.0));
    }

    #[test]
    fn the_aim_snap_step_comes_from_the_caller() {
        let mut c = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "c", 0);
        // 38 degrees snaps to 30 with a 30 degree step, to 45 with 45.
        let to = Point::new(100.0, 100.0 * 38.0_f64.to_radians().tan());
        apply_handle_with(&mut c, CamHandle::Aim, to, Some(30.0));
        assert!((c.direction_deg - 30.0).abs() < 1e-9);
        apply_handle_with(&mut c, CamHandle::Aim, to, Some(45.0));
        assert!((c.direction_deg - 45.0).abs() < 1e-9);
        apply_handle_with(&mut c, CamHandle::Aim, to, None);
        assert!((c.direction_deg - 38.0).abs() < 1e-6);
    }

    fn house() -> Project {
        let mut p = Project::new("House");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        for i in 0..4 {
            p.add_wall(
                0,
                c[i],
                c[(i + 1) % 4],
                6.0,
                96.0,
                plan_core::WallKind::Exterior,
            );
        }
        p
    }

    fn pick(t: &mut CameraTool, v: CameraVariant) {
        t.set_variant(ToolId::CameraVariant(v));
    }

    #[test]
    fn a_wall_elevation_faces_the_clicked_side_of_the_wall() {
        let (mut cx, mut t, outbox) = setup();
        cx.project = house();
        pick(&mut t, CameraVariant::WallElevation);
        // North side of the south wall (the interior of the house).
        let res = down(&mut cx, &mut t, Point::new(120.0, 24.0));
        assert!(!res.consumed || res.commit.is_none(), "{res:?}");
        // The pick tolerance is small: 24" off the wall's face is a miss.
        assert!(cx.project.cameras.is_empty());
        let res = down(&mut cx, &mut t, Point::new(120.0, 2.0));
        assert_eq!(res.commit.as_deref(), Some("Create Camera"));
        let c = &cx.project.cameras[0];
        assert_eq!(c.kind, CameraKind::WallElevation);
        // The wall runs along +X; its interior normal is +Y, so the camera
        // looks along -Y (270 degrees) at the wall.
        let wall_normal = Point::new(0.0, 1.0);
        assert!(
            (c.direction().dot(wall_normal) + 1.0).abs() < 1e-9,
            "{}",
            c.direction_deg
        );
        // The cut line sits just inside the room, the length of the wall, and
        // the back clip reaches only through the wall.
        let s = c.section.expect("a cut line");
        assert!((s.a.y - 3.5).abs() < 1e-9 && (s.b.y - 3.5).abs() < 1e-9);
        assert!((s.a.dist(s.b) - 240.0).abs() < 1e-9);
        assert_eq!(
            s.back_clip,
            Some(6.0 + WALL_ELEVATION_GAP + WALL_ELEVATION_REACH)
        );
        assert!(c.position.dist(Point::new(120.0, 3.5)) < 1e-9);
        assert_eq!(outbox.take(), vec![ViewRequest::ShowCamera(c.id)]);
        // The other face of the same wall looks the other way.
        let far = wall_elevation(
            &cx.project.floors[0].walls[0],
            Point::new(120.0, -50.0),
            0,
            "x",
        );
        assert!((far.direction_deg - 90.0).abs() < 1e-9);
        assert!((far.position.y + 3.5).abs() < 1e-9);
    }

    #[test]
    fn auto_elevations_make_four_cameras_and_update_them_on_a_second_run() {
        let (mut cx, mut t, outbox) = setup();
        cx.project = house();
        pick(&mut t, CameraVariant::AutoElevation);
        let res = down(&mut cx, &mut t, Point::new(-200.0, -200.0));
        assert_eq!(res.commit.as_deref(), Some("Auto Elevations"));
        assert_eq!(cx.project.cameras.len(), 4);
        let by = |name: &str| {
            cx.project
                .cameras
                .iter()
                .find(|c| c.name == name)
                .unwrap_or_else(|| panic!("no {name}"))
                .clone()
        };
        let south = by("South Elevation");
        assert_eq!(south.kind, CameraKind::Elevation);
        assert!((south.direction_deg - 90.0).abs() < 1e-9);
        assert!(
            south.position.y < -3.0,
            "outside the building: {:?}",
            south.position
        );
        assert!((by("North Elevation").direction_deg - 270.0).abs() < 1e-9);
        assert!(by("West Elevation").direction_deg.abs() < 1e-9);
        assert!((by("East Elevation").direction_deg - 180.0).abs() < 1e-9);
        assert!(by("East Elevation").position.x > 243.0);
        assert!(matches!(outbox.take()[..], [ViewRequest::ShowCamera(_)]));
        // Growing the house and running again moves the cameras, no copies.
        cx.project.add_wall(
            0,
            Point::new(240.0, 192.0),
            Point::new(480.0, 192.0),
            6.0,
            96.0,
            plan_core::WallKind::Exterior,
        );
        pick(&mut t, CameraVariant::AutoElevation);
        down(&mut cx, &mut t, Point::new(-200.0, -200.0));
        assert_eq!(cx.project.cameras.len(), 4);
        assert!(by_name(&cx.project, "East Elevation").position.x > 483.0);
        // One undo step undoes the whole set.
        cx.undo();
        assert_eq!(cx.project.cameras.len(), 4);
        cx.undo();
        assert!(cx.project.cameras.is_empty());
        // Back-clipped elevations are sections with a back clip.
        pick(&mut t, CameraVariant::AutoBackclipped);
        down(&mut cx, &mut t, Point::new(-200.0, -200.0));
        assert_eq!(cx.project.cameras.len(), 4);
        let c = by_name(&cx.project, "South Back-Clipped Elevation");
        assert!(matches!(
            c.kind,
            CameraKind::CrossSection { back_clip: Some(_) }
        ));
        assert_eq!(back_clip(&c), Some(AUTO_ELEVATION_MARGIN + AUTO_BACK_CLIP));
    }

    fn by_name(p: &Project, name: &str) -> CameraObject {
        p.cameras.iter().find(|c| c.name == name).unwrap().clone()
    }

    #[test]
    fn auto_elevations_need_walls() {
        let (mut cx, mut t, outbox) = setup();
        pick(&mut t, CameraVariant::AutoElevation);
        down(&mut cx, &mut t, Point::new(0.0, 0.0));
        assert!(cx.project.cameras.is_empty() && outbox.take().is_empty());
        assert!(cx.status.contains("walls"));
    }

    #[test]
    fn the_walkthrough_tool_draws_a_path_with_node_looks() {
        let (mut cx, mut t, outbox) = setup();
        pick(&mut t, CameraVariant::Walkthrough);
        // Click, click-and-drag (aims the second node), then double-click.
        down(&mut cx, &mut t, Point::new(0.0, 0.0));
        up(&mut cx, &mut t, Point::new(0.0, 0.0));
        let press = PointerEvent::at(&cx, Point::new(120.0, 0.0)).with_down(true);
        t.pointer_down(&mut cx, press);
        let aim = PointerEvent::at(&cx, Point::new(120.0, 60.0)).with_down(true);
        t.pointer_move(&mut cx, aim);
        up(&mut cx, &mut t, Point::new(120.0, 60.0));
        down(&mut cx, &mut t, Point::new(120.0, 120.0));
        up(&mut cx, &mut t, Point::new(120.0, 120.0));
        // The second half of the double click lands on the last node.
        down(&mut cx, &mut t, Point::new(120.0, 120.0));
        let res = double(&mut cx, &mut t, Point::new(120.0, 120.0));
        assert_eq!(res.commit.as_deref(), Some("Create Camera"));
        assert_eq!(cx.project.cameras.len(), 1);
        let c = &cx.project.cameras[0];
        assert_eq!(c.kind, CameraKind::Walkthrough);
        assert_eq!(
            c.path,
            vec![
                Point::new(0.0, 0.0),
                Point::new(120.0, 0.0),
                Point::new(120.0, 120.0)
            ]
        );
        assert_eq!(c.position, c.path[0]);
        assert_eq!(c.path_nodes.len(), 3);
        assert_eq!(c.path_nodes[0].look_deg, None);
        assert!((c.path_nodes[1].look_deg.unwrap() - 90.0).abs() < 1e-9);
        assert_eq!(c.path_nodes[2].height, 66.0);
        assert!(c.direction_deg.abs() < 1e-9, "faces the first segment");
        assert_eq!(outbox.take(), vec![ViewRequest::ShowCamera(c.id)]);
    }

    #[test]
    fn a_walkthrough_needs_two_nodes_and_escape_cancels() {
        let (mut cx, mut t, _o) = setup();
        pick(&mut t, CameraVariant::Walkthrough);
        down(&mut cx, &mut t, Point::new(0.0, 0.0));
        up(&mut cx, &mut t, Point::new(0.0, 0.0));
        let res = t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert!(res.consumed && cx.project.cameras.is_empty());
        down(&mut cx, &mut t, Point::new(0.0, 0.0));
        down(&mut cx, &mut t, Point::new(60.0, 0.0));
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(cx.project.cameras.is_empty());
        // Enter finishes a path, Backspace removes the last node.
        for p in [(0.0, 0.0), (60.0, 0.0), (60.0, 60.0)] {
            down(&mut cx, &mut t, Point::new(p.0, p.1));
            up(&mut cx, &mut t, Point::new(p.0, p.1));
        }
        t.key(&mut cx, KeyEvent::key(egui::Key::Backspace));
        let res = t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert_eq!(res.commit.as_deref(), Some("Create Camera"));
        assert_eq!(cx.project.cameras[0].path.len(), 2);
    }

    #[test]
    fn moving_a_walkthrough_moves_its_whole_path() {
        let mut c = CameraObject::walkthrough(
            vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)],
            66.0,
            "W",
            0,
        );
        apply_handle(&mut c, CamHandle::Move, Point::new(10.0, 20.0), false);
        assert_eq!(
            c.path,
            vec![Point::new(10.0, 20.0), Point::new(110.0, 20.0)]
        );
        assert!(hit_symbol(&c, Point::new(60.0, 21.0), 2.0));
        assert!(!hit_symbol(&c, Point::new(60.0, 60.0), 2.0));
    }

    #[test]
    fn add_lights_places_selects_moves_and_deletes_lights() {
        let (mut cx, mut t, outbox) = setup();
        pick(&mut t, CameraVariant::AddLights);
        let res = down(&mut cx, &mut t, Point::new(120.0, 96.0));
        assert_eq!(res.commit.as_deref(), Some("Add Light"));
        let lights = cx.project.lights();
        assert_eq!(lights.len(), 1);
        let l = &lights[0];
        let ceiling = cx.floor().ceiling_height;
        assert_eq!(l.position, Point::new(120.0, 96.0));
        assert!((l.height - (ceiling - 12.0)).abs() < 1e-9);
        assert!(l.enabled && l.cast_shadows);
        assert_eq!(t.selected_light(), Some(l.id));
        // A click on the light selects it instead of adding another.
        let id = l.id;
        down(&mut cx, &mut t, Point::new(121.0, 96.0));
        up(&mut cx, &mut t, Point::new(121.0, 96.0));
        assert_eq!(cx.project.lights().len(), 1);
        // Dragging it moves it in one undo step.
        let press = PointerEvent::at(&cx, Point::new(120.0, 96.0)).with_down(true);
        t.pointer_down(&mut cx, press);
        let to = PointerEvent::at(&cx, Point::new(180.0, 96.0)).with_down(true);
        t.pointer_move(&mut cx, to);
        let res = up(&mut cx, &mut t, Point::new(180.0, 96.0));
        assert_eq!(res.commit.as_deref(), Some("Move Light"));
        assert_eq!(
            cx.project.light(id).unwrap().position,
            Point::new(180.0, 96.0)
        );
        cx.undo();
        assert_eq!(
            cx.project.light(id).unwrap().position,
            Point::new(120.0, 96.0)
        );
        // Double-click opens Adjust Lights on it.
        double(&mut cx, &mut t, Point::new(120.0, 96.0));
        assert_eq!(outbox.take(), vec![ViewRequest::OpenAdjustLights(Some(id))]);
        // Delete removes the selected light.
        t.selected_light = Some(id);
        let res = t.key(&mut cx, KeyEvent::key(egui::Key::Delete));
        assert_eq!(res.commit.as_deref(), Some("Delete Light"));
        assert!(cx.project.lights().is_empty());
        cx.undo();
        assert_eq!(cx.project.lights().len(), 1);
    }
}
