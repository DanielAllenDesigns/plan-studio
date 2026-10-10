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
//! * **Floor Camera**: placed like a Full Camera; its 3D view shows only its
//!   own floor, clipped at the floor's ceiling. **Glass House** opens the
//!   overview with the Glass House technique.
//! * **Wedge handles** (C-25): the two far corners of a Full or Floor
//!   Camera's view cone change the angle of view, and a diamond a quarter of
//!   the way along the cone tilts the view ([`WedgeHandle`]). A camera locked
//!   in its Camera Specification ignores every handle.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::{Camera, EditorContext};
use crate::shell::view3d_panel::{camera_defaults, Outbox, ViewRequest};
use eframe::egui::{self, Color32, Pos2, Shape, Stroke, StrokeKind};
use plan_core::camera::{PlanLight, WalkNode, DEFAULT_CONE_LENGTH, DEFAULT_FOV_DEG};
use plan_core::extras::SectionLine;
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::{CameraKind, CameraObject, EditingDefaults, Id, Layer, Project, Wall};
use std::collections::HashMap;
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
/// Narrowest and widest angle of view a cone handle can set, degrees.
const MIN_FOV: f64 = 5.0;
const MAX_FOV: f64 = 170.0;

/// Which camera the tool creates.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CameraVariant {
    #[default]
    FullCamera,
    /// Floor Camera: a Full Camera that shows only its own floor, clipped at
    /// that floor's ceiling.
    FloorCamera,
    /// Glass House (see-through walls) overview.
    GlassHouse,
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
    /// Auto Interior Elevations: click a room, four wall elevations of it.
    AutoInterior,
    /// Create Walkthrough Path.
    Walkthrough,
    /// Add Lights (C-64).
    AddLights,
}

impl CameraVariant {
    pub fn label(self) -> &'static str {
        match self {
            CameraVariant::FullCamera => "Full Camera",
            CameraVariant::FloorCamera => "Floor Camera",
            CameraVariant::GlassHouse => "Glass House View",
            CameraVariant::FullOverview => "Perspective Full Overview",
            CameraVariant::FloorOverview => "Perspective Floor Overview",
            CameraVariant::DollHouse => "Doll House View",
            CameraVariant::CrossSection => "Cross Section/Elevation Camera",
            CameraVariant::BackClippedSection => "Back-Clipped Cross Section",
            CameraVariant::WallElevation => "Wall Elevation Camera",
            CameraVariant::AutoElevation => "Auto Elevations",
            CameraVariant::AutoBackclipped => "Auto Back-Clipped Elevations",
            CameraVariant::AutoInterior => "Auto Interior Elevations",
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
            CameraVariant::FullOverview
                | CameraVariant::FloorOverview
                | CameraVariant::DollHouse
                | CameraVariant::GlassHouse
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

/// One side of the building for the single-side Auto Elevation tools (manual
/// p. 1151): Front is the bottom of the plan on screen, Back the top, Left and
/// Right as seen from the front.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AutoSide {
    Front,
    Back,
    Left,
    Right,
}

impl AutoSide {
    pub const ALL: [AutoSide; 4] = [
        AutoSide::Front,
        AutoSide::Back,
        AutoSide::Left,
        AutoSide::Right,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AutoSide::Front => "Front Elevation",
            AutoSide::Back => "Back Elevation",
            AutoSide::Left => "Left Elevation",
            AutoSide::Right => "Right Elevation",
        }
    }

    /// The compass name Auto Elevations give the camera of this side.
    fn name(self) -> &'static str {
        match self {
            AutoSide::Front => "South",
            AutoSide::Back => "North",
            AutoSide::Left => "West",
            AutoSide::Right => "East",
        }
    }
}

/// The side the next activation of the Auto Elevation tool makes (`None`:
/// all four), read by [`CameraTool::set_variant`].
static NEXT_AUTO_SIDE: Mutex<Option<AutoSide>> = Mutex::new(None);

/// Chooses the side the Auto Elevation tool makes the next time it is
/// activated; `None` makes all four.
pub fn set_next_auto_side(side: Option<AutoSide>) {
    *NEXT_AUTO_SIDE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = side;
}

fn take_next_auto_side() -> Option<AutoSide> {
    NEXT_AUTO_SIDE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
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

/// A walkthrough along a CAD polyline or line of `floor` (a closed polyline
/// comes back to its start): each vertex becomes a key frame node. `None`
/// when `cad_id` is not a polyline or line with two distinct points.
pub fn walkthrough_from_cad(
    project: &Project,
    floor: usize,
    cad_id: Id,
    eye_height: f64,
    name: &str,
) -> Option<CameraObject> {
    let obj = project
        .floors
        .get(floor)?
        .cad
        .iter()
        .find(|o| o.id == cad_id)?;
    let mut pts: Vec<Point> = match &obj.item {
        plan_core::CadItem::Polyline { points, closed } => {
            let mut v = points.clone();
            if *closed {
                v.extend(points.first().copied());
            }
            v
        }
        plan_core::CadItem::Line { a, b } => vec![*a, *b],
        _ => return None,
    };
    pts.dedup_by(|a, b| a.dist(*b) < 1e-6);
    (pts.len() >= 2).then(|| CameraObject::walkthrough(pts, eye_height, name, floor))
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
    auto_elevation_cameras_for(project, floor, backclipped, None)
}

/// [`auto_elevation_cameras`] for one `side` of the building, or all four.
pub fn auto_elevation_cameras_for(
    project: &Project,
    floor: usize,
    backclipped: bool,
    side: Option<AutoSide>,
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
            .filter(|(name, ..)| side.is_none_or(|s| s.name() == *name))
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
    add_auto_elevations_for(project, floor, backclipped, None)
}

/// [`add_auto_elevations`] for one side of the building (the Front, Back, Left
/// and Right Elevation tools), or all four.
pub fn add_auto_elevations_for(
    project: &mut Project,
    floor: usize,
    backclipped: bool,
    side: Option<AutoSide>,
) -> Vec<Id> {
    let Some(cams) = auto_elevation_cameras_for(project, floor, backclipped, side) else {
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

/// Adds the Auto Interior Elevations of the room of `floor` containing `at`
/// (see `Project::auto_interior_elevations`), updating the ones a previous run
/// made (same name) instead of duplicating them. Returns their ids in North,
/// East, South, West order; empty when no room contains the point.
pub fn add_interior_elevations(project: &mut Project, floor: usize, at: Point) -> Vec<Id> {
    let Some(made) = project.auto_interior_elevations(floor, at) else {
        return Vec::new();
    };
    ensure_camera_layer(project);
    made.cameras
        .into_iter()
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

pub fn ensure_camera_layer(project: &mut Project) {
    use plan_core::camera_view::clip::{CLIP_LINES_LAYER, CROSS_SECTION_LAYER};
    if project.layers.get(CAMERA_LAYER).is_none() {
        project
            .layers
            .add(Layer::new(CAMERA_LAYER, [0x2F, 0x6C, 0xB3], 25));
    }
    // The Cross Section Lines layer is locked and on; the Clip Lines layer is
    // off until the user turns it on to adjust the clipping in the view
    // (manual pp. 1167, 1172).
    if project.layers.get(CROSS_SECTION_LAYER).is_none() {
        let mut l = Layer::new(CROSS_SECTION_LAYER, [0x7A, 0x2E, 0x2E], 13);
        l.locked = true;
        project.layers.add(l);
    }
    if project.layers.get(CLIP_LINES_LAYER).is_none() {
        let mut l = Layer::new(CLIP_LINES_LAYER, [0xD0, 0x70, 0x10], 13);
        l.display = false;
        project.layers.add(l);
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
    /// The diamond handle of break `i` of a stepped cutting plane: drag it
    /// along the line to move the step (C-137).
    Break(usize),
    /// The handle of piece `i` of a stepped plane: drag it square to the line
    /// to set how far ahead of the base line the piece stands (C-137).
    Step(usize),
}

/// The plan point at `x` along a section's cut line (0 at its centre, toward
/// the viewer's right) and `depth` ahead of it.
pub fn plane_point(c: &CameraObject, x: f64, depth: f64) -> Option<Point> {
    let (centre, t, fwd) = plan_core::camera_view::clip::line_frame(c)?;
    Some(centre + t * x + fwd * depth)
}

/// Adds a break to the section `c`'s cutting plane at the plan point `at`
/// (the Add Break edit tool, manual p. 1171): the point is projected onto the
/// cut line. Returns false when `c` has no cut line or a break is too near.
pub fn add_section_break(c: &mut CameraObject, at: Point) -> bool {
    let Some((centre, t, _)) = plan_core::camera_view::clip::line_frame(c) else {
        return false;
    };
    let half = section_width(c) * 0.5;
    let x = (at - centre).dot(t);
    if x.abs() >= half {
        return false;
    }
    c.view.clip.plane.add_break(x).is_some()
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
        v.extend(step_handles(c));
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

/// The handles of a stepped cutting plane: a diamond at every break and a
/// handle in the middle of every piece (manual p. 1172).
fn step_handles(c: &CameraObject) -> Vec<(CamHandle, Point)> {
    let mut plane = c.view.clip.plane.clone();
    plane.normalize();
    if !plane.is_stepped() {
        return Vec::new();
    }
    let half = section_width(c) * 0.5;
    let mut v = Vec::new();
    for (i, x) in plane.breaks.iter().enumerate() {
        let depth = (plane.offsets[i] + plane.offsets[i + 1]) * 0.5;
        if let Some(p) = plane_point(c, *x, depth) {
            v.push((CamHandle::Break(i), p));
        }
    }
    for (i, (x0, x1, off)) in plane.spans(-half, half).into_iter().enumerate() {
        if let Some(p) = plane_point(c, (x0 + x1) * 0.5, off) {
            v.push((CamHandle::Step(i), p));
        }
    }
    v
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
    if !c.view.show_in_plan {
        return false;
    }
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

// ----- wedge handles: angle of view and tilt (C-25) -----

/// The extra handles on a Full or Floor Camera's view cone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WedgeHandle {
    /// The left far corner of the cone: drag to widen or narrow the angle of
    /// view.
    FovLeft,
    /// The right far corner of the cone.
    FovRight,
    /// A diamond on the centre line: drag along it to look up or down.
    Tilt,
}

/// How far along the cone the tilt handle sits, as a fraction of its length.
const TILT_HANDLE_AT: f64 = 0.25;
/// Degrees of tilt per cone length of travel of the tilt handle: its full
/// range (85 degrees either way) spans the first half of the cone.
const TILT_DEG_PER_LENGTH: f64 = 340.0;

/// Do the wedge handles apply to this camera?
pub fn has_wedge(c: &CameraObject) -> bool {
    c.kind.is_eye_level()
}

fn cone_length(c: &CameraObject) -> f64 {
    c.clip_distance.unwrap_or(DEFAULT_CONE_LENGTH)
}

/// The wedge handles of `c` with their plan positions (none for cameras
/// without a view cone).
pub fn wedge_handles_of(c: &CameraObject) -> Vec<(WedgeHandle, Point)> {
    if !has_wedge(c) {
        return Vec::new();
    }
    let cone = c.cone_points();
    let rest = c.position + c.direction() * (cone_length(c) * TILT_HANDLE_AT);
    // The tilt handle slides along the centre line with the tilt.
    let slide = c.view.tilt_deg / TILT_DEG_PER_LENGTH * cone_length(c);
    vec![
        (WedgeHandle::FovLeft, cone[1]),
        (WedgeHandle::FovRight, cone[2]),
        (WedgeHandle::Tilt, rest + c.direction() * slide),
    ]
}

/// The wedge handle of `c` within `tol` of `p`.
pub fn hit_wedge(c: &CameraObject, p: Point, tol: f64) -> Option<WedgeHandle> {
    wedge_handles_of(c)
        .into_iter()
        .filter(|(_, at)| at.dist(p) <= tol)
        .min_by(|a, b| a.1.dist(p).total_cmp(&b.1.dist(p)))
        .map(|(h, _)| h)
}

/// Drags wedge handle `h` of `c` to the plan point `to`: a cone corner sets
/// the angle of view to twice the angle between the view direction and the
/// pointer (5 to 170 degrees); the tilt handle sets the tilt from how far
/// along the view direction it was dragged (limited to the tilt range). A
/// locked camera ignores it.
pub fn apply_wedge(c: &mut CameraObject, h: WedgeHandle, to: Point) {
    if c.view.locked || !has_wedge(c) {
        return;
    }
    let v = to - c.position;
    match h {
        WedgeHandle::FovLeft | WedgeHandle::FovRight => {
            if v.length() < 1e-6 {
                return;
            }
            let along = v.dot(c.direction());
            let across = v.dot(c.direction().perp()).abs();
            let half = across.atan2(along.max(1e-6)).to_degrees();
            c.fov_deg = (half * 2.0).clamp(MIN_FOV, MAX_FOV);
        }
        WedgeHandle::Tilt => {
            let travel = v.dot(c.direction()) / cone_length(c) - TILT_HANDLE_AT;
            c.view.tilt_deg = (travel * TILT_DEG_PER_LENGTH).clamp(
                -plan_core::camera_view::MAX_TILT_DEG,
                plan_core::camera_view::MAX_TILT_DEG,
            );
        }
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
    if c.view.locked {
        return;
    }
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
        CamHandle::Break(i) => {
            if let Some((centre, t, _)) = plan_core::camera_view::clip::line_frame(c) {
                let half = section_width(c) * 0.5;
                c.view
                    .clip
                    .plane
                    .move_break(i, (to - centre).dot(t), -half, half);
            }
        }
        CamHandle::Step(i) => {
            if let Some((centre, _, fwd)) = plan_core::camera_view::clip::line_frame(c) {
                c.view.clip.plane.set_offset(i, (to - centre).dot(fwd));
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
    /// A wedge handle (angle of view, tilt) is being dragged instead.
    wedge: Option<WedgeHandle>,
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
    /// The single side the Auto Elevation tool makes, if not all four.
    auto_side: Option<AutoSide>,
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
            auto_side: None,
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

    /// May camera `id` be edited: its layer is not locked and the camera is
    /// not locked in its Camera Specification?
    fn can_edit(&self, cx: &mut EditorContext, id: Id) -> bool {
        if !self.layer_ok(cx) {
            return false;
        }
        if cx.project.camera(id).is_some_and(|c| c.view.locked) {
            cx.status = "This camera is locked (Camera Specification)".into();
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
        let (kind, base) = if self.variant == CameraVariant::FloorCamera {
            (CameraKind::FloorCamera, "Floor Camera")
        } else {
            (CameraKind::FullCamera, "Camera")
        };
        let name = self.unique_name(cx, base);
        let defaults = camera_defaults();
        let mut cam = CameraObject::new(kind, at, dir, name, cx.floor);
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
        ensure_camera_layer(&mut cx.project);
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
        let label = self.auto_side.map_or(self.variant.label(), AutoSide::label);
        cx.begin_change(label);
        let ids = add_auto_elevations_for(
            &mut cx.project,
            cx.floor,
            self.variant == CameraVariant::AutoBackclipped,
            self.auto_side,
        );
        if let Some(first) = ids.first() {
            self.select(cx, *first);
            self.outbox.post(ViewRequest::ShowCamera(*first));
        }
        cx.status = format!("{label}: {} cameras", ids.len());
        ToolResult {
            switch_to: Some(ToolId::Select),
            ..ToolResult::committed(label)
        }
    }

    /// Auto Interior Elevations: click inside a room.
    fn click_interior(&mut self, cx: &mut EditorContext, at: Point) -> ToolResult {
        if !self.layer_ok(cx) {
            return ToolResult::consumed();
        }
        if cx.project.auto_interior_elevations(cx.floor, at).is_none() {
            cx.status = "Click inside a closed room to make its interior elevations".into();
            return ToolResult::consumed();
        }
        cx.begin_change(self.variant.label());
        let ids = add_interior_elevations(&mut cx.project, cx.floor, at);
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
        if self.variant == CameraVariant::GlassHouse {
            self.outbox.post(ViewRequest::GlassHouse);
        } else {
            self.outbox.post(ViewRequest::Mode { mode, floor });
        }
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
            CameraVariant::FloorCamera => {
                "Floor Camera: press at the eye, drag to aim; the view shows this floor only".into()
            }
            v if v.is_overview() => format!("{}: click in the plan to open the view", v.label()),
            CameraVariant::WallElevation => {
                "Wall Elevation: click the wall face you want to see".into()
            }
            v if v.is_auto() => match self.auto_side {
                Some(side) => format!("{}: click once to make the elevation", side.label()),
                None => format!("{}: click once to make the four elevations", v.label()),
            },
            CameraVariant::AutoInterior => {
                "Auto Interior Elevations: click inside a room to make its four wall elevations"
                    .into()
            }
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
            self.auto_side = take_next_auto_side().filter(|_| v.is_auto());
            self.reset_gestures();
        } else if let Some(v) = take_next_variant() {
            self.variant = v;
            self.auto_side = take_next_auto_side().filter(|_| v.is_auto());
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
            if self.can_edit(cx, id) {
                self.editing = Some(Editing {
                    id,
                    handle,
                    wedge: None,
                    began: false,
                });
            }
            return ToolResult::consumed();
        }
        // The wedge handles of the selected camera: angle of view and tilt.
        let wedge = self
            .selected
            .and_then(|id| cx.project.camera(id))
            .filter(|c| c.floor == cx.floor)
            .and_then(|c| hit_wedge(c, p.world, cx.pick_tol()).map(|h| (c.id, h)));
        if let Some((id, h)) = wedge {
            if self.can_edit(cx, id) {
                self.editing = Some(Editing {
                    id,
                    handle: CamHandle::Aim,
                    wedge: Some(h),
                    began: false,
                });
            }
            return ToolResult::consumed();
        }
        if let Some(id) = self.camera_at(cx, p.world) {
            self.select(cx, id);
            if self.can_edit(cx, id) {
                self.editing = Some(Editing {
                    id,
                    handle: CamHandle::Move,
                    wedge: None,
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
            CameraVariant::AutoInterior => return self.click_interior(cx, p.world),
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
            cx.begin_change(match (ed.wedge, ed.handle) {
                (Some(WedgeHandle::Tilt), _) => "Tilt Camera",
                (Some(_), _) => "Change Angle of View",
                (None, CamHandle::Move) => "Move Camera",
                (None, CamHandle::Aim) => "Rotate Camera",
                (None, CamHandle::Clip) => "Change Camera Clip Distance",
                (None, CamHandle::EndA | CamHandle::EndB) => "Stretch Cross Section",
                (None, CamHandle::Break(_)) => "Move Cross Section Break",
                (None, CamHandle::Step(_)) => "Step Cross Section Line",
            });
            ed.began = true;
        }
        let (id, handle, wedge) = (ed.id, ed.handle, ed.wedge);
        if let Some(w) = wedge {
            let to = p.world;
            cx.project.update_camera(id, |c| apply_wedge(c, w, to));
            self.outbox.post(ViewRequest::RefreshCamera(id));
            return ToolResult::consumed();
        }
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

// ----- callouts (C-17, C-20, C-21, C-24) -----

/// The outline of a callout bubble.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CalloutShape {
    #[default]
    Circle,
    Square,
    Hexagon,
}

impl CalloutShape {
    pub const ALL: [CalloutShape; 3] = [
        CalloutShape::Circle,
        CalloutShape::Square,
        CalloutShape::Hexagon,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CalloutShape::Circle => "Circle",
            CalloutShape::Square => "Square",
            CalloutShape::Hexagon => "Hexagon",
        }
    }
}

/// How section and elevation callouts look in the plan (Default Settings >
/// Camera Tools).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CalloutStyle {
    pub shape: CalloutShape,
    /// Radius of the bubble, plan inches.
    pub radius: f64,
    /// Also write the camera's name beside the bubble.
    pub show_name: bool,
}

impl Default for CalloutStyle {
    fn default() -> Self {
        Self {
            shape: CalloutShape::Circle,
            radius: 9.0,
            show_name: false,
        }
    }
}

static CALLOUT_STYLE: Mutex<CalloutStyle> = Mutex::new(CalloutStyle {
    shape: CalloutShape::Circle,
    radius: 9.0,
    show_name: false,
});

/// The callout style new and existing callouts are drawn with.
pub fn callout_style() -> CalloutStyle {
    *CALLOUT_STYLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn set_callout_style(s: CalloutStyle) {
    *CALLOUT_STYLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = s;
}

/// The plan callout of one section or elevation camera: a bubble behind the
/// middle of its cut line (on the viewer's side) with a stem to the line,
/// the view number and, once the camera's view is on a layout sheet, that
/// sheet's number under a dividing line.
#[derive(Clone, PartialEq, Debug)]
pub struct Callout {
    pub camera: Id,
    pub centre: Point,
    pub radius: f64,
    pub shape: CalloutShape,
    /// The view number.
    pub number: String,
    /// The sheet the view is on ("A-3"), if it is on a layout.
    pub sheet: Option<String>,
    /// The camera's name, when the style asks for it.
    pub name: Option<String>,
    /// The stem from the bubble to the cut line.
    pub stem: (Point, Point),
    /// The Text Below Line the user typed (Plan Display), if any.
    pub below: Option<String>,
    /// Arrows on the clip plane line (Left Side, Right Side, Both Sides):
    /// `(tail, tip, filled)`.
    pub arrows: Vec<(Point, Point, bool)>,
}

/// Gap between the cut line and the callout bubble, plan inches.
const CALLOUT_GAP: f64 = 10.0;

/// The layout sheet each camera's view is on, as `"A-3"`, read from the
/// plan's layout JSON without loading the whole layout: the first non-template
/// page holding a Camera box of that camera.
pub fn camera_sheet_refs(project: &Project) -> HashMap<Id, String> {
    let mut out = HashMap::new();
    let Some(pages) = project
        .layout
        .as_ref()
        .and_then(|l| l.get("pages"))
        .and_then(|p| p.as_array())
    else {
        return out;
    };
    for page in pages {
        if page.get("template_page").and_then(|t| t.as_bool()) == Some(true) {
            continue;
        }
        let Some(number) = page.get("number").and_then(|n| n.as_u64()) else {
            continue;
        };
        let boxes = page.get("boxes").and_then(|b| b.as_array());
        for b in boxes.into_iter().flatten() {
            let id = b
                .get("source")
                .and_then(|s| s.get("Camera"))
                .and_then(|c| c.get("camera_id"))
                .and_then(|i| i.as_u64());
            if let Some(id) = id {
                out.entry(id).or_insert_with(|| format!("A-{number}"));
            }
        }
    }
    out
}

/// The callouts of camera `c` (C-158): one bubble at the placement Plan
/// Display chose (Center, Left Side, Right Side, Both Sides or Custom), with
/// the camera's label, size, Text Below Line and arrows.
pub fn callouts_for(
    project: &Project,
    c: &CameraObject,
    sheets: &HashMap<Id, String>,
    style: &CalloutStyle,
) -> Vec<Callout> {
    use plan_core::camera_view::CalloutPlacement as P;
    if !c.kind.is_section_like() || !c.callout.show {
        return Vec::new();
    }
    let Some(number) = project.callout_number(c.id) else {
        return Vec::new();
    };
    let plan = &c.view.plan;
    let (a, b) = section_line(c);
    let half = section_width(c) * 0.5;
    let dir = c.direction();
    let r = plan.callout_size.map_or(style.radius, |s| s * 0.5).max(2.0);
    let xs: Vec<f64> = match plan.placement {
        P::Center => vec![0.0],
        P::LeftSide => vec![-half],
        P::RightSide => vec![half],
        P::BothSides => vec![-half, half],
        P::Custom => vec![plan.offset.clamp(-half, half)],
    };
    let label = if plan.callout_label.trim().is_empty() {
        number.to_string()
    } else {
        plan.callout_label.trim().to_string()
    };
    let arrow_len = match plan.arrow {
        plan_core::camera_view::CalloutArrow::None => 0.0,
        plan_core::camera_view::CalloutArrow::Small => ARROW_LEN * 0.25,
        plan_core::camera_view::CalloutArrow::Large => ARROW_LEN * 0.5,
    };
    xs.into_iter()
        .map(|x| {
            let on_line = plane_point(c, x, 0.0).unwrap_or_else(|| Point::lerp(a, b, 0.5));
            let centre = on_line - dir * (r + CALLOUT_GAP);
            let arrows = if plan.placement.draws_line() && arrow_len > 0.0 {
                vec![(on_line, on_line + dir * arrow_len, plan.arrow_filled)]
            } else {
                Vec::new()
            };
            Callout {
                camera: c.id,
                centre,
                radius: r,
                shape: style.shape,
                number: label.clone(),
                sheet: sheets.get(&c.id).cloned(),
                name: style.show_name.then(|| c.name.clone()),
                stem: (centre + dir * r, on_line),
                below: (!plan.text_below_auto && !plan.text_below.trim().is_empty())
                    .then(|| plan.text_below.trim().to_string()),
                arrows,
            }
        })
        .collect()
}

/// The callouts of the cameras placed on `floor`.
pub fn callouts_on(project: &Project, floor: usize, style: &CalloutStyle) -> Vec<Callout> {
    let on_floor: Vec<&CameraObject> = project
        .cameras_on(floor)
        .filter(|c| c.kind.is_section_like() && c.callout.show)
        .collect();
    if on_floor.is_empty() {
        return Vec::new();
    }
    let sheets = camera_sheet_refs(project);
    on_floor
        .into_iter()
        .flat_map(|c| callouts_for(project, c, &sheets, style))
        .collect()
}

fn draw_callout(painter: &egui::Painter, cam: &Camera, co: &Callout, selected: bool) {
    let ink = Stroke::new(if selected { 2.0_f32 } else { 1.2_f32 }, CAMERA_BLUE);
    let centre = cam.world_to_screen(co.centre);
    let r = ((co.radius * cam.px_per_in) as f32).clamp(7.0, 22.0);
    painter.line_segment(
        [
            cam.world_to_screen(co.stem.0),
            cam.world_to_screen(co.stem.1),
        ],
        ink,
    );
    let fill = Color32::from_rgba_unmultiplied(255, 255, 255, 235);
    match co.shape {
        CalloutShape::Circle => {
            painter.circle(centre, r, fill, ink);
        }
        CalloutShape::Square => {
            let rect = egui::Rect::from_center_size(centre, egui::Vec2::splat(2.0 * r));
            painter.rect(rect, 0.0, fill, ink, StrokeKind::Inside);
        }
        CalloutShape::Hexagon => {
            let pts: Vec<Pos2> = (0..6)
                .map(|i| {
                    let a = std::f32::consts::FRAC_PI_3 * i as f32;
                    centre + egui::vec2(a.cos(), a.sin()) * r
                })
                .collect();
            painter.add(Shape::convex_polygon(pts, fill, ink));
        }
    }
    let font = |k: f32| egui::FontId::proportional((r * k).max(7.0));
    match &co.sheet {
        Some(sheet) => {
            painter.line_segment(
                [centre - egui::vec2(r, 0.0), centre + egui::vec2(r, 0.0)],
                Stroke::new(1.0_f32, CAMERA_BLUE),
            );
            painter.text(
                centre - egui::vec2(0.0, r * 0.45),
                egui::Align2::CENTER_CENTER,
                &co.number,
                font(0.8),
                CAMERA_BLUE,
            );
            painter.text(
                centre + egui::vec2(0.0, r * 0.45),
                egui::Align2::CENTER_CENTER,
                sheet,
                font(0.5),
                CAMERA_BLUE,
            );
        }
        None => {
            painter.text(
                centre,
                egui::Align2::CENTER_CENTER,
                &co.number,
                font(1.0),
                CAMERA_BLUE,
            );
        }
    }
    for (tail, tip, filled) in &co.arrows {
        let (t, h) = (cam.world_to_screen(*tail), cam.world_to_screen(*tip));
        painter.line_segment([t, h], ink);
        let v = (h - t).normalized();
        let n = egui::vec2(-v.y, v.x);
        let head = [h, h - v * 7.0 + n * 3.5, h - v * 7.0 - n * 3.5];
        let (fill, edge) = if *filled {
            (CAMERA_BLUE, ink)
        } else {
            (Color32::TRANSPARENT, ink)
        };
        painter.add(Shape::convex_polygon(head.to_vec(), fill, edge));
    }
    if let Some(below) = &co.below {
        painter.text(
            centre + egui::vec2(0.0, r + 3.0),
            egui::Align2::CENTER_TOP,
            below,
            font(0.7),
            CAMERA_BLUE,
        );
    }
    if let Some(name) = &co.name {
        painter.text(
            centre + egui::vec2(r + 4.0, 0.0),
            egui::Align2::LEFT_CENTER,
            name,
            font(0.8),
            CAMERA_BLUE,
        );
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

/// The label under a camera symbol (the Label tab), when it is shown in the
/// plan.
fn draw_label(painter: &egui::Painter, cam: &Camera, c: &CameraObject) {
    if !c.view.label.show_in_plan {
        return;
    }
    let at = cam.world_to_screen(c.position) + egui::vec2(0.0, 14.0);
    painter.text(
        at,
        egui::Align2::CENTER_TOP,
        c.view.label_text(&c.name),
        egui::FontId::proportional(11.0),
        CAMERA_BLUE,
    );
}

fn draw_one(painter: &egui::Painter, cam: &Camera, c: &CameraObject, selected: bool) {
    if !c.view.show_in_plan {
        return;
    }
    draw_label(painter, cam, c);
    let mut line = Stroke::new(if selected { 2.0_f32 } else { 1.2_f32 }, CAMERA_BLUE);
    if let Some(w) = c.view.plan.line_weight.filter(|_| is_section(c)) {
        // Cross Section Line Weight (Plan Display), points to screen pixels.
        line.width = (w as f32 * 2.0).clamp(0.5, 6.0);
    }
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
        let half = section_width(c) * 0.5;
        let mut plane = c.view.clip.plane.clone();
        plane.normalize();
        if plane.is_stepped() {
            let pts: Vec<Point> = plane
                .path(-half, half)
                .into_iter()
                .filter_map(|(x, d)| plane_point(c, x, d))
                .collect();
            painter.add(Shape::line(polygon(cam, &pts), line));
        } else if c
            .view
            .plan
            .line_style
            .as_deref()
            .is_some_and(|s| !s.eq_ignore_ascii_case("solid"))
        {
            // A dashed Cross Section Line Style (Plan Display).
            painter.extend(Shape::dashed_line(
                &[cam.world_to_screen(a), cam.world_to_screen(b)],
                line,
                8.0,
                5.0,
            ));
        } else {
            painter.line_segment([cam.world_to_screen(a), cam.world_to_screen(b)], line);
        }
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
    // Show Field of View Indicators off: the cone only shows while selected.
    if c.view.plan.show_fov_indicators || selected {
        let cone = c.cone_points();
        let fill = CAMERA_BLUE.gamma_multiply(if selected { 0.22 } else { 0.12 });
        painter.add(Shape::convex_polygon(
            polygon(cam, &cone),
            fill,
            Stroke::new(1.0_f32, CAMERA_BLUE.gamma_multiply(0.8)),
        ));
    }
    painter.add(Shape::convex_polygon(
        polygon(cam, &c.triangle_points()),
        CAMERA_BLUE,
        Stroke::new(1.0_f32, Color32::WHITE),
    ));
}

/// The Clip Lines of a selected section (C-136): the side clip lines run
/// from the ends of the cut line back to the back clip, and the front clip
/// plane is the cut line itself. Shown while the "CAD, Clip Lines" layer is
/// on.
fn draw_clip_lines(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, c: &CameraObject) {
    use plan_core::camera_view::clip::CLIP_LINES_LAYER;
    if !is_section(c) || !c.view.clip.clip_sides {
        return;
    }
    let layer = cx.project.layers.get(CLIP_LINES_LAYER);
    if !layer.is_some_and(|l| l.display) {
        return;
    }
    let col = layer.map_or([0xD0, 0x70, 0x10], |l| l.color);
    let stroke = Stroke::new(1.0_f32, Color32::from_rgb(col[0], col[1], col[2]));
    let half = section_width(c) * 0.5;
    let reach = back_clip(c).unwrap_or(DEFAULT_BACK_CLIP);
    for x in [-half, half] {
        if let (Some(a), Some(b)) = (plane_point(c, x, 0.0), plane_point(c, x, reach)) {
            painter.extend(Shape::dashed_line(
                &[cam.world_to_screen(a), cam.world_to_screen(b)],
                stroke,
                5.0,
                3.0,
            ));
        }
    }
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
    // Each camera is on the layer its Layer panel names (C-156).
    let shown = |c: &CameraObject| cx.project.layers.is_visible(&c.view.layer.layer);
    for c in cx.project.cameras_on(cx.floor).filter(|c| shown(c)) {
        draw_one(painter, cam, c, selected == Some(c.id));
    }
    for co in callouts_on(&cx.project, cx.floor, &callout_style()) {
        if cx.project.camera(co.camera).is_some_and(shown) {
            draw_callout(painter, cam, &co, selected == Some(co.camera));
        }
    }
    if cx.project.layers.is_visible(CAMERA_LAYER) {
        draw_light_symbols(cx, painter, cam);
    }
    let Some(c) = selected
        .and_then(|id| cx.project.camera(id))
        .filter(|c| shown(c))
    else {
        return;
    };
    if c.floor != cx.floor {
        return;
    }
    if !c.view.show_in_plan {
        return;
    }
    draw_clip_lines(cx, painter, cam, c);
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
    // The cone's corners (angle of view) are round, the tilt handle a diamond.
    for (h, at) in wedge_handles_of(c) {
        let s = cam.world_to_screen(at);
        let color = if c.view.locked {
            Color32::GRAY
        } else {
            cx.palette.selection
        };
        match h {
            WedgeHandle::FovLeft | WedgeHandle::FovRight => {
                painter.circle_filled(s, 5.0, cx.palette.background);
                painter.circle_stroke(s, 5.0, Stroke::new(1.5_f32, color));
            }
            WedgeHandle::Tilt => {
                let d = 6.0_f32;
                let pts = vec![
                    s + egui::vec2(0.0, -d),
                    s + egui::vec2(d, 0.0),
                    s + egui::vec2(0.0, d),
                    s + egui::vec2(-d, 0.0),
                ];
                painter.add(Shape::convex_polygon(
                    pts,
                    cx.palette.background,
                    Stroke::new(1.5_f32, color),
                ));
            }
        }
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
        crate::shell::view3d_panel::reset_camera_defaults();
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
        assert_eq!(c.eye_height, 60.0, "C-5: a new camera starts at 60 in");
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
        crate::shell::view3d_panel::reset_camera_defaults();
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
        assert_eq!(c.path_nodes[2].height, 60.0);
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

    #[test]
    fn auto_interior_elevations_make_four_cameras_and_update_them_on_a_second_run() {
        let (mut cx, mut t, outbox) = setup();
        cx.project = house();
        cx.project.floors[0].room_names.push(plan_core::RoomName {
            anchor: Point::new(100.0, 100.0),
            name: "Den".into(),
            ..Default::default()
        });
        pick(&mut t, CameraVariant::AutoInterior);
        assert!(t.hint().contains("inside a room"));
        // A click outside any room makes nothing.
        let res = down(&mut cx, &mut t, Point::new(600.0, 600.0));
        assert!(res.commit.is_none());
        assert!(cx.project.cameras.is_empty());
        let res = down(&mut cx, &mut t, Point::new(120.0, 96.0));
        assert_eq!(res.commit.as_deref(), Some("Auto Interior Elevations"));
        assert_eq!(cx.project.cameras.len(), 4);
        let names: Vec<&str> = cx.project.cameras.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Den North Wall",
                "Den East Wall",
                "Den South Wall",
                "Den West Wall"
            ]
        );
        // Each looks at its wall from inside the room, back-clipped to the room.
        for (c, deg) in cx.project.cameras.iter().zip([90.0, 0.0, 270.0, 180.0]) {
            assert_eq!(c.kind, CameraKind::WallElevation);
            assert!((c.direction_deg - deg).abs() < 1e-9, "{}", c.name);
            let s = c.section.expect("a cut line");
            let clip = s.back_clip.expect("back-clipped to the room");
            assert!(clip < 240.0 + 20.0, "{clip}");
            assert!(cx.project.floors[0].walls.iter().all(|w| w.thickness > 0.0));
        }
        assert_eq!(outbox.take().len(), 1, "the first camera opens");
        // Running it again updates the same four.
        pick(&mut t, CameraVariant::AutoInterior);
        down(&mut cx, &mut t, Point::new(60.0, 60.0));
        assert_eq!(cx.project.cameras.len(), 4);
        // The drawing of the north wall camera sees the wall's inside face.
        let drawing = crate::dialogs::camera::render_elevation(&cx.project, &cx.project.cameras[0]);
        assert!(!drawing.lines.is_empty());
    }

    #[test]
    fn every_section_camera_gets_a_numbered_callout_behind_its_line() {
        let (mut cx, mut t, _o) = setup();
        for (a, b) in [
            (Point::new(0.0, 0.0), Point::new(120.0, 0.0)),
            (Point::new(0.0, 60.0), Point::new(120.0, 60.0)),
        ] {
            pick(&mut t, CameraVariant::CrossSection);
            drag(&mut cx, &mut t, a, b);
        }
        // A Full Camera makes no callout.
        pick(&mut t, CameraVariant::FullCamera);
        drag(
            &mut cx,
            &mut t,
            Point::new(300.0, 300.0),
            Point::new(300.0, 400.0),
        );
        let style = CalloutStyle::default();
        let cos = callouts_on(&cx.project, 0, &style);
        assert_eq!(cos.len(), 2);
        assert_eq!(cos[0].number, "1");
        assert_eq!(cos[1].number, "2");
        assert!(cos.iter().all(|c| c.sheet.is_none() && c.name.is_none()));
        // West to east looks north; the bubble stands south of the line.
        let first = cx.project.cameras[0].clone();
        let (a, b) = section_line(&first);
        let mid = Point::lerp(a, b, 0.5);
        assert!(cos[0].centre.y < mid.y);
        assert!((cos[0].centre.x - mid.x).abs() < 1e-9);
        assert!(cos[0].stem.1.dist(mid) < 1e-9);
        assert!(cos[0].stem.0.dist(cos[0].centre) <= cos[0].radius + 1e-9);
        // An explicit number wins; switching the callout off removes it.
        let second = cx.project.cameras[1].id;
        cx.project
            .update_camera(second, |c| c.callout.number = Some(7));
        let cos = callouts_on(&cx.project, 0, &style);
        assert_eq!(cos[1].number, "7");
        assert_eq!(cos[0].number, "1");
        cx.project
            .update_camera(first.id, |c| c.callout.show = false);
        assert_eq!(callouts_on(&cx.project, 0, &style).len(), 1);
        // Other floors have none.
        assert!(callouts_on(&cx.project, 1, &style).is_empty());
        // The name shows when the style asks for it.
        let named = CalloutStyle {
            show_name: true,
            shape: CalloutShape::Hexagon,
            ..style
        };
        let cos = callouts_on(&cx.project, 0, &named);
        assert_eq!(cos[0].name.as_deref(), Some("Section 2"));
        assert_eq!(cos[0].shape, CalloutShape::Hexagon);
    }

    #[test]
    fn the_callout_carries_the_sheet_once_the_camera_is_on_a_layout() {
        let mut p = house();
        let mut cam = CameraObject::new(
            CameraKind::Elevation,
            Point::new(120.0, -24.0),
            90.0,
            "South Elevation",
            0,
        );
        set_section_geometry(&mut cam, Point::new(120.0, -24.0), 90.0, 288.0, None);
        let id = p.add_camera(cam);
        let style = CalloutStyle::default();
        assert!(callouts_on(&p, 0, &style)[0].sheet.is_none());
        let mut layout = plan_layout::Layout::new("L", plan_docs::SheetSize::ArchC);
        crate::dialogs::camera::send_camera_to_layout(&mut layout, &p, id, 3).expect("a box");
        crate::shell::layout_window::store(&mut p, &layout);
        let refs = camera_sheet_refs(&p);
        assert_eq!(refs.get(&id).map(String::as_str), Some("A-3"));
        let co = &callouts_on(&p, 0, &style)[0];
        assert_eq!(co.sheet.as_deref(), Some("A-3"));
        assert_eq!(co.number, "1");
        // The layout caption agrees with the plan's view number.
        let b = layout.page(3).unwrap().boxes.last().unwrap();
        assert_eq!(b.label.as_deref(), Some("1 - SOUTH ELEVATION"));
    }

    #[test]
    fn callouts_paint_the_view_number_and_the_sheet() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut cam = CameraObject::new(
            CameraKind::Elevation,
            Point::new(120.0, 100.0),
            90.0,
            "S",
            0,
        );
        set_section_geometry(&mut cam, Point::new(120.0, 100.0), 90.0, 200.0, None);
        let id = cx.project.add_camera(cam);
        let mut layout = plan_layout::Layout::new("L", plan_docs::SheetSize::ArchC);
        let p2 = cx.project.clone();
        crate::dialogs::camera::send_camera_to_layout(&mut layout, &p2, id, 2).expect("a box");
        crate::shell::layout_window::store(&mut cx.project, &layout);
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            draw_camera_symbols(&cx, &painter, &Camera::default_view(), None);
        });
        let texts: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"1".to_string()), "{texts:?}");
        assert!(texts.contains(&"A-2".to_string()), "{texts:?}");
    }

    // ----- wedge handles, lock, Floor Camera, walkthrough from CAD (round 14) -----

    fn placed_full_camera() -> (EditorContext, CameraTool, Outbox, Id) {
        let (mut cx, mut t, o) = setup();
        drag(
            &mut cx,
            &mut t,
            Point::new(100.0, 100.0),
            Point::new(200.0, 100.0),
        );
        let id = cx.project.cameras[0].id;
        o.take();
        (cx, t, o, id)
    }

    #[test]
    fn the_cone_corners_change_the_angle_of_view() {
        crate::shell::view3d_panel::reset_camera_defaults();
        let (mut cx, mut t, o, id) = placed_full_camera();
        let c = cx.project.camera(id).unwrap().clone();
        assert_eq!(c.fov_deg, DEFAULT_FOV_DEG);
        let handles = wedge_handles_of(&c);
        assert_eq!(handles.len(), 3);
        let corner = handles[0].1;
        assert_eq!(hit_wedge(&c, corner, 6.0), Some(WedgeHandle::FovLeft));
        // Drag the left corner out to 45 degrees off the direction (looking +X).
        let to = Point::new(100.0 + 150.0, 100.0 + 150.0);
        let down = PointerEvent::at(&cx, corner).with_down(true);
        t.pointer_down(&mut cx, down);
        let mv = PointerEvent::at(&cx, to).with_down(true);
        t.pointer_move(&mut cx, mv);
        let res = up(&mut cx, &mut t, to);
        assert_eq!(res.commit.as_deref(), Some("Edit Camera"));
        let c = cx.project.camera(id).unwrap();
        assert!((c.fov_deg - 90.0).abs() < 1e-6, "{}", c.fov_deg);
        assert_eq!(
            c.position,
            Point::new(100.0, 100.0),
            "the camera did not move"
        );
        assert_eq!(c.direction_deg, 0.0, "nor turn");
        assert_eq!(o.take(), vec![ViewRequest::RefreshCamera(id)]);
        // One undo step restores the old angle.
        cx.undo();
        assert_eq!(cx.project.camera(id).unwrap().fov_deg, DEFAULT_FOV_DEG);
        // The angle stays within 5 to 170 degrees.
        let mut c = cx.project.camera(id).unwrap().clone();
        apply_wedge(
            &mut c,
            WedgeHandle::FovRight,
            Point::new(100.0 + 200.0, 100.0 - 1.0),
        );
        assert_eq!(c.fov_deg, 5.0);
        apply_wedge(
            &mut c,
            WedgeHandle::FovRight,
            Point::new(100.0 - 50.0, 100.0 - 50.0),
        );
        assert_eq!(c.fov_deg, 170.0);
    }

    #[test]
    fn the_tilt_diamond_slides_along_the_view_direction() {
        let (mut cx, mut t, _o, id) = placed_full_camera();
        let c = cx.project.camera(id).unwrap().clone();
        let tilt_at = |c: &CameraObject| {
            wedge_handles_of(c)
                .into_iter()
                .find(|(h, _)| *h == WedgeHandle::Tilt)
                .unwrap()
                .1
        };
        // At rest it sits a quarter of the way along the cone.
        let rest = tilt_at(&c);
        assert!(rest.dist(Point::new(100.0 + 60.0, 100.0)) < 1e-9);
        assert_eq!(hit_wedge(&c, rest, 6.0), Some(WedgeHandle::Tilt));
        // Drag it forward by 60": 60 / 240 of the cone length is 85 degrees.
        let to = Point::new(100.0 + 120.0, 100.0 + 3.0);
        let down = PointerEvent::at(&cx, rest).with_down(true);
        t.pointer_down(&mut cx, down);
        let mv = PointerEvent::at(&cx, to).with_down(true);
        t.pointer_move(&mut cx, mv);
        let res = up(&mut cx, &mut t, to);
        assert_eq!(res.commit.as_deref(), Some("Edit Camera"));
        let c = cx.project.camera(id).unwrap().clone();
        assert!((c.view.tilt_deg - 85.0).abs() < 1e-6, "{}", c.view.tilt_deg);
        // The handle follows the tilt, and looking down moves it back.
        assert!(tilt_at(&c).dist(Point::new(100.0 + 120.0, 100.0)) < 1e-9);
        let mut down_cam = c.clone();
        apply_wedge(
            &mut down_cam,
            WedgeHandle::Tilt,
            Point::new(100.0 + 30.0, 100.0),
        );
        assert!((down_cam.view.tilt_deg + 42.5).abs() < 1e-6);
        apply_wedge(
            &mut down_cam,
            WedgeHandle::Tilt,
            Point::new(100.0 - 500.0, 100.0),
        );
        assert_eq!(down_cam.view.tilt_deg, -85.0);
        cx.undo();
        assert_eq!(cx.project.camera(id).unwrap().view.tilt_deg, 0.0);
    }

    #[test]
    fn a_locked_camera_ignores_every_handle() {
        let (mut cx, mut t, _o, id) = placed_full_camera();
        cx.project.update_camera(id, |c| c.view.locked = true);
        let before = cx.project.camera(id).unwrap().clone();
        for h in [CamHandle::Move, CamHandle::Aim, CamHandle::Clip] {
            let mut c = before.clone();
            apply_handle(&mut c, h, Point::new(500.0, 500.0), false);
            assert_eq!(c, before, "{h:?}");
        }
        for h in [WedgeHandle::FovLeft, WedgeHandle::Tilt] {
            let mut c = before.clone();
            apply_wedge(&mut c, h, Point::new(300.0, 300.0));
            assert_eq!(c, before, "{h:?}");
        }
        // The tool does not even start a drag, and says why.
        let down = PointerEvent::at(&cx, Point::new(101.0, 100.0)).with_down(true);
        t.pointer_down(&mut cx, down);
        let mv = PointerEvent::at(&cx, Point::new(300.0, 300.0)).with_down(true);
        t.pointer_move(&mut cx, mv);
        let res = up(&mut cx, &mut t, Point::new(300.0, 300.0));
        assert!(res.commit.is_none());
        assert_eq!(cx.project.camera(id).unwrap(), &before);
        assert!(cx.status.contains("locked"));
    }

    #[test]
    fn a_camera_hidden_from_the_plan_has_no_symbol_to_hit() {
        let (mut cx, _t, _o, id) = placed_full_camera();
        let p = Point::new(100.0, 100.0);
        assert!(hit_symbol(cx.project.camera(id).unwrap(), p, 3.0));
        cx.project
            .update_camera(id, |c| c.view.show_in_plan = false);
        assert!(!hit_symbol(cx.project.camera(id).unwrap(), p, 3.0));
    }

    #[test]
    fn only_eye_level_cameras_have_wedge_handles() {
        let full = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "f", 0);
        let floor = CameraObject::new(CameraKind::FloorCamera, Point::ZERO, 0.0, "f", 0);
        let dolls = CameraObject::new(CameraKind::DollHouse, Point::ZERO, 0.0, "d", 0);
        let walk =
            CameraObject::walkthrough(vec![Point::ZERO, Point::new(100.0, 0.0)], 66.0, "w", 0);
        assert_eq!(wedge_handles_of(&full).len(), 3);
        assert_eq!(wedge_handles_of(&floor).len(), 3);
        assert!(wedge_handles_of(&dolls).is_empty());
        assert!(wedge_handles_of(&walk).is_empty());
    }

    #[test]
    fn the_floor_camera_variant_places_a_floor_camera() {
        let (mut cx, mut t, o) = setup();
        pick(&mut t, CameraVariant::FloorCamera);
        let res = drag(
            &mut cx,
            &mut t,
            Point::new(100.0, 100.0),
            Point::new(100.0, 220.0),
        );
        assert_eq!(res.commit.as_deref(), Some("Create Camera"));
        let c = &cx.project.cameras[0];
        assert_eq!(c.kind, CameraKind::FloorCamera);
        assert_eq!(c.name, "Floor Camera 1");
        assert_eq!(o.take(), vec![ViewRequest::ShowCamera(c.id)]);
        // A Glass House click opens the overview instead of placing anything.
        pick(&mut t, CameraVariant::GlassHouse);
        down(&mut cx, &mut t, Point::new(10.0, 10.0));
        assert_eq!(cx.project.cameras.len(), 1);
        assert_eq!(o.take(), vec![ViewRequest::GlassHouse]);
    }

    #[test]
    fn a_walkthrough_is_made_from_a_cad_polyline_line_or_closed_shape() {
        let mut p = Project::new("cad");
        let poly = p.add_cad(
            0,
            "CAD, Default",
            plan_core::CadItem::Polyline {
                points: vec![
                    Point::new(0.0, 0.0),
                    Point::new(100.0, 0.0),
                    Point::new(100.0, 0.0),
                    Point::new(100.0, 80.0),
                ],
                closed: false,
            },
        );
        let ring = p.add_cad(
            0,
            "CAD, Default",
            plan_core::CadItem::Polyline {
                points: vec![
                    Point::new(0.0, 0.0),
                    Point::new(100.0, 0.0),
                    Point::new(100.0, 80.0),
                ],
                closed: true,
            },
        );
        let line = p.add_cad(
            0,
            "CAD, Default",
            plan_core::CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(60.0, 0.0),
            },
        );
        let circle = p.add_cad(
            0,
            "CAD, Default",
            plan_core::CadItem::Circle {
                center: Point::ZERO,
                radius: 10.0,
            },
        );
        let w = walkthrough_from_cad(&p, 0, poly, 70.0, "Walk").unwrap();
        assert_eq!(w.kind, CameraKind::Walkthrough);
        assert_eq!(w.path.len(), 3, "the doubled vertex is dropped");
        assert_eq!(w.path_nodes.len(), 3);
        assert!(w.path_nodes.iter().all(|n| n.height == 70.0));
        assert_eq!(w.position, Point::new(0.0, 0.0));
        assert!((w.direction_deg).abs() < 1e-9);
        let r = walkthrough_from_cad(&p, 0, ring, 66.0, "Ring").unwrap();
        assert_eq!(r.path.len(), 4);
        assert_eq!(r.path.last(), r.path.first());
        assert_eq!(
            walkthrough_from_cad(&p, 0, line, 66.0, "L")
                .unwrap()
                .path
                .len(),
            2
        );
        assert!(walkthrough_from_cad(&p, 0, circle, 66.0, "C").is_none());
        assert!(walkthrough_from_cad(&p, 0, 9999, 66.0, "none").is_none());
        assert!(walkthrough_from_cad(&p, 3, poly, 66.0, "floor").is_none());
    }

    #[test]
    fn the_selected_camera_draws_its_wedge_handles_and_label_and_a_hidden_one_nothing() {
        let (mut cx, _t, _o, id) = placed_full_camera();
        let paint = |cx: &EditorContext, selected: Option<Id>| {
            let ctx = egui::Context::default();
            let out = ctx.run(egui::RawInput::default(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                draw_camera_symbols(cx, &painter, &Camera::default_view(), selected);
            });
            let circles = out
                .shapes
                .iter()
                .filter(|s| matches!(s.shape, Shape::Circle(_)))
                .count();
            let texts: Vec<String> = out
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect();
            (out.shapes.len(), circles, texts)
        };
        let (_, plain, texts) = paint(&cx, None);
        assert!(texts.is_empty());
        let (_, with_handles, _) = paint(&cx, Some(id));
        assert!(with_handles >= plain + 2, "two round angle-of-view handles");
        cx.project.update_camera(id, |c| {
            c.view.label.show_in_plan = true;
            c.view.label.text = "Entry".into();
        });
        assert!(paint(&cx, None).2.contains(&"Entry".to_string()));
        cx.project
            .update_camera(id, |c| c.view.show_in_plan = false);
        let (shapes, _, texts) = paint(&cx, Some(id));
        assert_eq!(shapes, 0, "nothing of a camera hidden from the plan");
        assert!(texts.is_empty());
    }
}
