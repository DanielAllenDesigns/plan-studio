//! Camera objects placed in the floor plan (`docs/parity/3d-views-cameras.md`,
//! C-4..C-30). The 3D viewer builds its views from these; here they are plain
//! data plus the plan symbol geometry.

use crate::cad::CadItem;
use crate::geometry::Point;
use crate::model::{Id, Project};
use serde::{Deserialize, Serialize};

/// Length of the view cone drawn in plan when no clip distance is set, inches.
pub const DEFAULT_CONE_LENGTH: f64 = 240.0;
/// Default eye height above the floor, inches: 60 in per the manual (C-5,
/// DECISIONS CS1).
pub const DEFAULT_EYE_HEIGHT: f64 = 60.0;
/// Default horizontal angle of view, degrees (C-7).
pub const DEFAULT_FOV_DEG: f64 = 55.0;
/// Eye height of a new walkthrough node, inches; DECISIONS CS1 leaves it.
pub const DEFAULT_WALK_HEIGHT: f64 = 66.0;
/// Size of the camera triangle in plan, inches.
const GLYPH_LENGTH: f64 = 18.0;
const GLYPH_HALF_WIDTH: f64 = 7.0;

/// The kind of view a camera produces (C-4, C-10..C-20).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CameraKind {
    FullCamera,
    PerspectiveOverview,
    DollHouse,
    /// Cross section / elevation; `back_clip` is the rear clip distance for a
    /// back-clipped section (C-19).
    CrossSection {
        back_clip: Option<f64>,
    },
    WallElevation,
    Orthographic,
    /// Exterior elevation made by Auto Elevations (C-21): the section line
    /// only places the camera, nothing is cut away.
    Elevation,
    /// A camera that follows `path` (C-30 family, Create Walkthrough Path).
    Walkthrough,
    /// A Full Camera that shows only its own floor: everything above the
    /// floor's ceiling is clipped away (Floor Camera).
    FloorCamera,
    /// A Perspective Overview drawn as a Glass House (see-through walls,
    /// every edge visible).
    GlassHouse,
    /// A Perspective Overview of the framing (the camera kind only; the
    /// framing view fills the scene).
    FramingOverview,
}

/// Default walking speed of a walkthrough, inches per second (3 ft/s).
pub const DEFAULT_WALK_SPEED: f64 = 36.0;

fn default_walk_speed() -> f64 {
    DEFAULT_WALK_SPEED
}

/// Per-node settings of a walkthrough path: the camera height above the floor
/// and, optionally, a fixed look direction (`None` = look along the path).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WalkNode {
    /// Eye height above the floor at this node, inches.
    pub height: f64,
    /// Look direction in degrees counter-clockwise from +X.
    #[serde(default)]
    pub look_deg: Option<f64>,
    /// Up/down tilt of the view at this node, degrees (positive looks up).
    #[serde(default)]
    pub tilt_deg: f64,
    /// Seconds the camera stands still at this node (a key frame held).
    #[serde(default)]
    pub hold_s: f64,
}

impl Default for WalkNode {
    fn default() -> Self {
        Self {
            height: DEFAULT_WALK_HEIGHT,
            look_deg: None,
            tilt_deg: 0.0,
            hold_s: 0.0,
        }
    }
}

/// The camera pose on a walkthrough at one point of the path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WalkPose {
    pub position: Point,
    /// Eye height above the floor, inches.
    pub eye_height: f64,
    /// Look direction in degrees counter-clockwise from +X.
    pub direction_deg: f64,
    /// Up/down tilt in degrees (positive looks up).
    pub tilt_deg: f64,
}

/// Interpolates angles (degrees) along the shortest arc.
fn lerp_deg(a: f64, b: f64, t: f64) -> f64 {
    let d = (b - a + 180.0).rem_euclid(360.0) - 180.0;
    a + d * t
}

/// Vector View options of an elevation or section camera: which extra
/// annotations the 2D drawing gets and how its lines are weighted.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VectorOptions {
    /// Automatic dimension strings left of the building: floor-to-floor and
    /// the sills and heads of the openings in view.
    pub dimensions: bool,
    /// The "T.O. SUBFLOOR" / "T.O. PLATE" level callouts (with the camera's
    /// labels option on).
    pub level_labels: bool,
    /// Text leaders naming the siding, brick, roofing... of each region.
    pub material_labels: bool,
    /// Draw each wall's and opening's lines at the pen weight of its layer.
    pub layer_weights: bool,
    /// Draw hidden edges dashed.
    pub hidden_dashed: bool,
}

impl Default for VectorOptions {
    fn default() -> Self {
        Self {
            dimensions: false,
            level_labels: true,
            material_labels: false,
            layer_weights: false,
            hidden_dashed: false,
        }
    }
}

/// The plan callout of a section or elevation camera: the bubble with its
/// view number (and, once the view is on a layout sheet, the sheet reference).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CalloutOptions {
    /// Draw the callout in the plan.
    pub show: bool,
    /// The view number; `None` numbers the cameras automatically.
    pub number: Option<u32>,
}

impl Default for CalloutOptions {
    fn default() -> Self {
        Self {
            show: true,
            number: None,
        }
    }
}

/// A camera shown in the plan on one floor (C-24).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraObject {
    pub id: Id,
    pub kind: CameraKind,
    /// Eye position in the plan, inches.
    pub position: Point,
    /// View direction in degrees counter-clockwise from +X.
    pub direction_deg: f64,
    /// Eye height above the floor it sits on, inches (C-6).
    pub eye_height: f64,
    /// Horizontal field of view, degrees (C-7).
    pub fov_deg: f64,
    /// Far clip distance, inches (`None` = unlimited).
    pub clip_distance: Option<f64>,
    pub name: String,
    /// Index of the floor the camera was placed on.
    pub floor: usize,
    /// Cut line of a cross section; replaces overloading `fov_deg`.
    #[serde(default)]
    pub section: Option<crate::extras::SectionLine>,
    /// Hatch, shadow, depth-weight and label options of an elevation or
    /// section camera.
    #[serde(default)]
    pub render: crate::extras::ElevationRender,
    /// Walkthrough path in the plan (kind [`CameraKind::Walkthrough`]).
    #[serde(default)]
    pub path: Vec<Point>,
    /// Per-node height and look direction of `path`; nodes past the end of
    /// this list use [`WalkNode::default`].
    #[serde(default)]
    pub path_nodes: Vec<WalkNode>,
    /// Walking speed along `path`, inches per second.
    #[serde(default = "default_walk_speed")]
    pub walk_speed: f64,
    /// Vector View annotations and line weights of an elevation or section.
    #[serde(default)]
    pub vector: VectorOptions,
    /// The plan callout of a section or elevation.
    #[serde(default)]
    pub callout: CalloutOptions,
    /// Tilt, lock, backdrop, rendering, label and floors of the view (the
    /// Camera Specification tabs).
    #[serde(default)]
    pub view: crate::camera_view::CameraView,
}

impl CameraObject {
    /// A camera with Chief-like defaults (eye height 66", 60 degree view).
    /// The id is assigned by [`Project::add_camera`].
    pub fn new(
        kind: CameraKind,
        position: Point,
        direction_deg: f64,
        name: impl Into<String>,
        floor: usize,
    ) -> Self {
        Self {
            id: 0,
            kind,
            position,
            direction_deg,
            eye_height: DEFAULT_EYE_HEIGHT,
            fov_deg: DEFAULT_FOV_DEG,
            clip_distance: None,
            name: name.into(),
            floor,
            section: None,
            render: crate::extras::ElevationRender::default(),
            path: Vec::new(),
            path_nodes: Vec::new(),
            walk_speed: DEFAULT_WALK_SPEED,
            vector: VectorOptions::default(),
            callout: CalloutOptions::default(),
            view: crate::camera_view::CameraView {
                clip: crate::camera_view::SectionClip::for_kind(kind),
                ..crate::camera_view::CameraView::default()
            },
        }
    }

    /// A walkthrough along `path` (at least one node): the camera sits at the
    /// first node looking along the first segment. Node heights start at the
    /// eye height `eye_height`.
    pub fn walkthrough(
        path: Vec<Point>,
        eye_height: f64,
        name: impl Into<String>,
        floor: usize,
    ) -> Self {
        let first = path.first().copied().unwrap_or(Point::ZERO);
        let dir = path
            .get(1)
            .map_or(90.0, |p| (*p - first).angle().to_degrees());
        let mut c = Self::new(CameraKind::Walkthrough, first, dir, name, floor);
        c.eye_height = eye_height;
        c.path_nodes = path
            .iter()
            .map(|_| WalkNode {
                height: eye_height,
                ..WalkNode::default()
            })
            .collect();
        c.path = path;
        c
    }

    /// Node `i` of the walkthrough path (defaults past the stored list).
    pub fn walk_node(&self, i: usize) -> WalkNode {
        self.path_nodes.get(i).copied().unwrap_or(WalkNode {
            height: self.eye_height,
            ..WalkNode::default()
        })
    }

    /// Length of the walkthrough path, inches.
    pub fn walk_length(&self) -> f64 {
        self.path.windows(2).map(|w| w[0].dist(w[1])).sum()
    }

    /// Seconds spent standing at the nodes (the key frames' holds).
    pub fn walk_hold_s(&self) -> f64 {
        (0..self.path.len())
            .map(|i| self.walk_node(i).hold_s.max(0.0))
            .sum()
    }

    /// Time to walk the whole path at `walk_speed`, plus the holds, seconds.
    pub fn walk_duration_s(&self) -> f64 {
        let travel = if self.walk_speed > 0.0 {
            self.walk_length() / self.walk_speed
        } else {
            0.0
        };
        travel + self.walk_hold_s()
    }

    /// Look direction at node `i`: its own, or along the path (the mean of
    /// the segments on either side, the end nodes use their one segment).
    fn node_look_deg(&self, i: usize) -> f64 {
        if let Some(d) = self.walk_node(i).look_deg {
            return d;
        }
        let n = self.path.len();
        let seg = |a: usize, b: usize| (self.path[b] - self.path[a]).normalized();
        let dir = match (i > 0, i + 1 < n) {
            (true, true) => seg(i - 1, i) + seg(i, i + 1),
            (false, true) => seg(i, i + 1),
            (true, false) => seg(i - 1, i),
            (false, false) => return self.direction_deg,
        };
        if dir.length() < 1e-9 {
            self.direction_deg
        } else {
            dir.angle().to_degrees()
        }
    }

    /// The pose at fraction `u` (0..=1) of the way along the path by length:
    /// the position moves in straight lines between nodes, the eye height and
    /// tilt are interpolated between the node values and the look direction
    /// along the shortest arc between the node directions. A one-node path
    /// stays put. Holds are not part of the length: see
    /// [`CameraObject::walk_pose_at_time`].
    pub fn walk_pose(&self, u: f64) -> WalkPose {
        let n = self.path.len();
        if n < 2 {
            return self.pose_on(0, 0.0);
        }
        let total = self.walk_length();
        let want = u.clamp(0.0, 1.0) * total;
        let mut run = 0.0;
        let mut seg = n - 2;
        for i in 0..n - 1 {
            let len = self.path[i].dist(self.path[i + 1]);
            if want <= run + len || i == n - 2 {
                seg = i;
                break;
            }
            run += len;
        }
        let len = self.path[seg].dist(self.path[seg + 1]);
        let t = if len > 1e-9 {
            ((want - run) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.pose_on(seg, t)
    }

    /// The pose a fraction `t` of the way along segment `seg`.
    fn pose_on(&self, seg: usize, t: f64) -> WalkPose {
        let n = self.path.len();
        if n == 0 {
            return WalkPose {
                position: self.position,
                eye_height: self.eye_height,
                direction_deg: self.direction_deg,
                tilt_deg: 0.0,
            };
        }
        if n == 1 {
            let node = self.walk_node(0);
            return WalkPose {
                position: self.path[0],
                eye_height: node.height,
                direction_deg: self.node_look_deg(0),
                tilt_deg: node.tilt_deg,
            };
        }
        let seg = seg.min(n - 2);
        let (a, b) = (self.walk_node(seg), self.walk_node(seg + 1));
        WalkPose {
            position: Point::lerp(self.path[seg], self.path[seg + 1], t),
            eye_height: a.height + (b.height - a.height) * t,
            direction_deg: lerp_deg(self.node_look_deg(seg), self.node_look_deg(seg + 1), t),
            tilt_deg: a.tilt_deg + (b.tilt_deg - a.tilt_deg) * t,
        }
    }

    /// Seconds into the walk at which the camera arrives at node `i` (the
    /// key frames' times, for jumping between them). Clamped to the last
    /// node.
    pub fn walk_node_time(&self, i: usize) -> f64 {
        let n = self.path.len();
        if n < 2 || self.walk_speed <= 0.0 {
            return 0.0;
        }
        let i = i.min(n - 1);
        let mut t = 0.0;
        for seg in 0..i {
            t += self.walk_node(seg).hold_s.max(0.0);
            t += self.path[seg].dist(self.path[seg + 1]) / self.walk_speed;
        }
        t
    }

    /// The pose `t_s` seconds into the walk (clamped to the path): the camera
    /// walks each segment at `walk_speed` and stands still for each node's
    /// `hold_s` before it sets off.
    pub fn walk_pose_at_time(&self, t_s: f64) -> WalkPose {
        let n = self.path.len();
        if n < 2 || self.walk_speed <= 0.0 {
            return self.pose_on(0, 0.0);
        }
        let mut t = t_s.max(0.0);
        for seg in 0..n - 1 {
            let hold = self.walk_node(seg).hold_s.max(0.0);
            if t < hold {
                return self.pose_on(seg, 0.0);
            }
            t -= hold;
            let travel = self.path[seg].dist(self.path[seg + 1]) / self.walk_speed;
            if t < travel {
                return self.pose_on(seg, t / travel);
            }
            t -= travel;
        }
        self.pose_on(n - 2, 1.0)
    }

    /// Unit view direction in the plan.
    pub fn direction(&self) -> Point {
        let a = self.direction_deg.to_radians();
        Point::new(a.cos(), a.sin())
    }

    /// The camera glyph: a triangle whose tip points along the view
    /// direction, centered on the eye. Three points.
    pub fn triangle_points(&self) -> [Point; 3] {
        let d = self.direction();
        let n = d.perp();
        // Plan Display's Camera Symbol Size is the glyph's length.
        let len = self.view.plan.symbol_size.map_or(GLYPH_LENGTH, |s| s.max(1.0));
        let half_width = GLYPH_HALF_WIDTH * len / GLYPH_LENGTH;
        let tip = self.position + d * (len * 0.5);
        let back = self.position - d * (len * 0.5);
        [tip, back + n * half_width, back - n * half_width]
    }

    /// The view cone: `[apex, left far corner, right far corner]`, with the
    /// far edge at the clip distance (or [`DEFAULT_CONE_LENGTH`]) and the edge
    /// lines half the field of view off the view direction (C-24, C-25).
    pub fn cone_points(&self) -> [Point; 3] {
        let len = self.clip_distance.unwrap_or(DEFAULT_CONE_LENGTH);
        let half = (self.fov_deg * 0.5).to_radians();
        let base = self.direction_deg.to_radians();
        let edge = |a: f64| self.position + Point::new(a.cos(), a.sin()) * (len / half.cos());
        [self.position, edge(base + half), edge(base - half)]
    }

    /// The eye and target an overview restores (C-14). With a plan symbol
    /// (`ViewPose::symbol`) the symbol's position and line of sight place them
    /// in plan: the eye is the camera, the target the end of its line of
    /// sight; the pose keeps the heights.
    pub fn overview_pose(&self) -> Option<crate::camera_view::ViewPose> {
        let p = self.view.pose?;
        if !p.symbol {
            return Some(p);
        }
        let d = self.direction();
        let len = self.clip_distance.unwrap_or(DEFAULT_CONE_LENGTH);
        Some(crate::camera_view::ViewPose {
            eye: [self.position.x, p.eye[1], -self.position.y],
            target: [
                self.position.x + d.x * len,
                p.target[1],
                -(self.position.y + d.y * len),
            ],
            symbol: true,
        })
    }

    /// The plan symbol (C-24): the three triangle points followed by the three
    /// cone points (`apex, left, right`), six points in all.
    pub fn symbol_points(&self) -> Vec<Point> {
        let mut v = self.triangle_points().to_vec();
        v.extend(self.cone_points());
        v
    }
}

impl Project {
    /// Add a camera (its `id` is replaced with a fresh one) and return the id.
    pub fn add_camera(&mut self, mut camera: CameraObject) -> Id {
        let id = self.alloc_id();
        camera.id = id;
        self.cameras.push(camera);
        id
    }

    /// Remove a camera; returns whether it existed.
    pub fn remove_camera(&mut self, id: Id) -> bool {
        let n = self.cameras.len();
        self.cameras.retain(|c| c.id != id);
        self.cameras.len() != n
    }

    /// Edit a camera in place; returns `false` if it does not exist. The id
    /// cannot be changed by `edit`.
    pub fn update_camera(&mut self, id: Id, edit: impl FnOnce(&mut CameraObject)) -> bool {
        match self.cameras.iter_mut().find(|c| c.id == id) {
            Some(c) => {
                edit(c);
                c.id = id;
                true
            }
            None => false,
        }
    }

    pub fn camera(&self, id: Id) -> Option<&CameraObject> {
        self.cameras.iter().find(|c| c.id == id)
    }

    /// Cameras shown on `floor`: those placed on it and those whose Plan
    /// Display says Display on All Floors (manual pp. 1191, 1198).
    pub fn cameras_on(&self, floor: usize) -> impl Iterator<Item = &CameraObject> {
        self.cameras
            .iter()
            .filter(move |c| c.view.plan.shows_on(c.floor, floor))
    }
}

// ----- callouts and interior elevations (C-17, C-20, C-21) -----

impl CameraKind {
    /// Is this a camera placed by a cut line (section, wall or exterior
    /// elevation), the cameras that get a plan callout?
    pub fn is_section_like(self) -> bool {
        matches!(
            self,
            CameraKind::CrossSection { .. } | CameraKind::WallElevation | CameraKind::Elevation
        )
    }

    /// Does the camera stand in the plan at an eye height and look along its
    /// direction (Full Camera and Floor Camera)?
    pub fn is_eye_level(self) -> bool {
        matches!(self, CameraKind::FullCamera | CameraKind::FloorCamera)
    }

    /// Is this a camera that orbits the whole building (the overviews, Doll
    /// House and Glass House)?
    pub fn is_overview(self) -> bool {
        matches!(
            self,
            CameraKind::PerspectiveOverview
                | CameraKind::DollHouse
                | CameraKind::GlassHouse
                | CameraKind::FramingOverview
        )
    }
}

/// The back clip of a section-like camera: a cross section keeps its own, an
/// elevation the one stored on its cut line; `None` means unlimited.
pub fn back_clip_of(c: &CameraObject) -> Option<f64> {
    match (&c.section, c.kind) {
        (Some(s), _) => s.back_clip,
        (None, CameraKind::CrossSection { back_clip }) => back_clip,
        _ => None,
    }
}

/// How far in front of the wall an interior elevation's cut line stands, inches.
pub const INTERIOR_CUT_INSET: f64 = 1.0;
/// How far an interior elevation reaches past the room's far surface, inches
/// (through the wall it looks at).
pub const INTERIOR_BACK_EXTRA: f64 = 8.0;
/// Shortest room side that gets an interior elevation, inches.
const MIN_INTERIOR_SIDE: f64 = 12.0;

/// The cameras of Auto Interior Elevations for one room.
#[derive(Debug, Clone, PartialEq)]
pub struct InteriorElevations {
    /// The room's name ("Room" without one).
    pub room: String,
    /// North, East, South, West wall elevations, in that order.
    pub cameras: Vec<CameraObject>,
}

impl Project {
    /// The cameras that get a plan callout, in project order.
    pub fn callout_cameras(&self) -> Vec<&CameraObject> {
        self.cameras
            .iter()
            .filter(|c| c.kind.is_section_like() && c.callout.show)
            .collect()
    }

    /// The view number of a camera's callout: its own number, or the lowest
    /// number no other callout claims, handed out in project order.
    pub fn callout_number(&self, id: Id) -> Option<u32> {
        let cams = self.callout_cameras();
        cams.iter().find(|c| c.id == id)?;
        let taken: Vec<u32> = cams.iter().filter_map(|c| c.callout.number).collect();
        let mut next = 0;
        for c in cams {
            let n = match c.callout.number {
                Some(n) => n,
                None => loop {
                    next += 1;
                    if !taken.contains(&next) {
                        break next;
                    }
                },
            };
            if c.id == id {
                return Some(n);
            }
        }
        None
    }

    /// Auto Interior Elevations for the room of `floor` that contains `at`
    /// (the smallest one when rooms nest): a Wall Elevation camera for each of
    /// its four sides, standing just inside the opposite side, looking at the
    /// wall and reaching only to the wall's far face. They are named
    /// "{room} North Wall" and so on, after the wall they look at (north is
    /// plan +Y); the cameras are returned, not added. `None` when no room
    /// contains the point or it is too small.
    pub fn auto_interior_elevations(&self, floor: usize, at: Point) -> Option<InteriorElevations> {
        let f = self.floors.get(floor)?;
        let rooms = crate::rooms::detect_rooms(&f.walls, 1.0);
        let room = rooms
            .iter()
            .filter(|r| r.contains(at))
            .min_by(|a, b| a.area_sq_in.total_cmp(&b.area_sq_in))?;
        let ring = if room.inner_polygon.len() >= 3 {
            &room.inner_polygon
        } else {
            &room.polygon
        };
        let first = *ring.first()?;
        let (lo, hi) = ring.iter().fold((first, first), |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        });
        let (w, h) = (hi.x - lo.x, hi.y - lo.y);
        if w < MIN_INTERIOR_SIDE || h < MIN_INTERIOR_SIDE {
            return None;
        }
        let name = room
            .name_entry(&f.room_names)
            .map(|n| n.name.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Room".to_string());
        let (cx, cy) = ((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
        let inset = INTERIOR_CUT_INSET;
        // (wall, view direction, line centre, line length, room depth along the view)
        let sides = [
            ("North", 90.0, Point::new(cx, lo.y + inset), w, h),
            ("East", 0.0, Point::new(lo.x + inset, cy), h, w),
            ("South", 270.0, Point::new(cx, hi.y - inset), w, h),
            ("West", 180.0, Point::new(hi.x - inset, cy), h, w),
        ];
        let cameras = sides
            .into_iter()
            .map(|(side, view_deg, centre, width, depth)| {
                let back = depth - 2.0 * inset + INTERIOR_BACK_EXTRA;
                let mut cam = CameraObject::new(
                    CameraKind::WallElevation,
                    centre,
                    view_deg,
                    format!("{name} {side} Wall"),
                    floor,
                );
                let t = Point::new(cam.direction().y, -cam.direction().x) * (width * 0.5);
                cam.section = Some(crate::extras::SectionLine {
                    a: centre - t,
                    b: centre + t,
                    back_clip: Some(back),
                });
                cam
            })
            .collect();
        Some(InteriorElevations {
            room: name,
            cameras,
        })
    }
}

// ----- lights (C-64..C-66) -----

/// The hidden layer the light records used to live on. Lights are now the
/// typed slots [`Project::lights`] and [`Project::light_options`]; files saved
/// before that hold them as tagged text records in the floors' CAD lists on
/// this layer, and [`migrate_legacy`] converts them once on load.
pub const LIGHTS_LAYER: &str = "Lights, Data";
const LIGHT_TAG: &str = "plan-light:";
const LIGHT_SET_TAG: &str = "plan-lightset:";

/// A point light placed in the plan (C-64): position, height above the floor,
/// intensity (1.0 is a typical room light), colour and shadow options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanLight {
    /// The record id (assigned by [`Project::add_light`]).
    #[serde(default)]
    pub id: Id,
    /// The floor the light was placed on.
    #[serde(default)]
    pub floor: usize,
    pub name: String,
    pub position: Point,
    /// Height above the floor it sits on, inches.
    pub height: f64,
    /// Relative intensity; 1.0 lights a room from a 7' ceiling.
    pub intensity: f32,
    pub color: [u8; 3],
    pub enabled: bool,
    pub cast_shadows: bool,
}

impl PlanLight {
    /// A warm light at `position`, `height` inches above the floor.
    pub fn new(position: Point, height: f64) -> Self {
        Self {
            id: 0,
            floor: 0,
            name: "Light".into(),
            position,
            height,
            intensity: 1.0,
            color: [255, 244, 229],
            enabled: true,
            cast_shadows: true,
        }
    }
}

/// Plan-wide light options.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LightSettings {
    /// Light fixtures of the electrical plan (ceiling lights, recessed cans,
    /// pendants, sconces) emit light too.
    pub use_electrical: bool,
}

impl Default for LightSettings {
    fn default() -> Self {
        Self {
            use_electrical: true,
        }
    }
}

impl Project {
    /// Every light of the plan, in the order they were added.
    pub fn lights(&self) -> Vec<PlanLight> {
        self.lights.clone()
    }

    pub fn light(&self, id: Id) -> Option<PlanLight> {
        self.lights.iter().find(|l| l.id == id).cloned()
    }

    /// Adds a light to `floor`; `None` if the floor does not exist.
    pub fn add_light(&mut self, floor: usize, mut light: PlanLight) -> Option<Id> {
        if floor >= self.floors.len() {
            return None;
        }
        let id = self.alloc_id();
        light.id = id;
        light.floor = floor;
        self.lights.push(light);
        Some(id)
    }

    /// Edits a light in place; `false` if there is no such light. Its id and
    /// floor cannot be changed by `edit`.
    pub fn update_light(&mut self, id: Id, edit: impl FnOnce(&mut PlanLight)) -> bool {
        let Some(l) = self.lights.iter_mut().find(|l| l.id == id) else {
            return false;
        };
        let floor = l.floor;
        edit(l);
        l.id = id;
        l.floor = floor;
        true
    }

    /// Removes a light; returns whether it existed.
    pub fn remove_light(&mut self, id: Id) -> bool {
        let n = self.lights.len();
        self.lights.retain(|l| l.id != id);
        self.lights.len() != n
    }

    /// The plan-wide light options (defaults when none are stored).
    pub fn light_settings(&self) -> LightSettings {
        self.light_options
    }

    pub fn set_light_settings(&mut self, settings: LightSettings) {
        self.light_options = settings;
    }
}

/// Moves lights stored the old way (tagged text records on the hidden
/// [`LIGHTS_LAYER`] of the floors' CAD lists) into [`Project::lights`] and
/// [`Project::light_options`], removes the records and the layer. Runs once
/// when a project is loaded; returns whether anything changed.
pub fn migrate_legacy(project: &mut Project) -> bool {
    let mut changed = false;
    let mut found: Vec<PlanLight> = Vec::new();
    let mut options = None;
    for (floor, f) in project.floors.iter_mut().enumerate() {
        let n = f.cad.len();
        f.cad.retain(|o| {
            if o.layer != LIGHTS_LAYER {
                return true;
            }
            if let CadItem::Text { text, .. } = &o.item {
                if let Some(json) = text.strip_prefix(LIGHT_TAG) {
                    if let Ok(mut l) = serde_json::from_str::<PlanLight>(json) {
                        l.id = o.id;
                        l.floor = floor;
                        found.push(l);
                    }
                } else if let Some(json) = text.strip_prefix(LIGHT_SET_TAG) {
                    if let Ok(s) = serde_json::from_str::<LightSettings>(json) {
                        options = Some(s);
                    }
                }
            }
            false
        });
        changed |= f.cad.len() != n;
    }
    for l in found {
        if !project.lights.iter().any(|x| x.id == l.id) {
            project.lights.push(l);
        }
    }
    if let Some(s) = options {
        project.light_options = s;
    }
    let n = project.layers.layers.len();
    project.layers.layers.retain(|l| l.name != LIGHTS_LAYER);
    changed |= project.layers.layers.len() != n;
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_crud() {
        let mut p = Project::new("c");
        let id = p.add_camera(CameraObject::new(
            CameraKind::CrossSection {
                back_clip: Some(120.0),
            },
            Point::new(10.0, 20.0),
            90.0,
            "Section A",
            0,
        ));
        assert_eq!(p.camera(id).unwrap().name, "Section A");
        assert!(p.update_camera(id, |c| {
            c.eye_height = 72.0;
            c.id = 12345;
        }));
        assert_eq!(p.camera(id).unwrap().eye_height, 72.0);
        assert_eq!(p.cameras_on(0).count(), 1);
        assert_eq!(p.cameras_on(1).count(), 0);
        assert!(!p.update_camera(999, |_| {}));
        assert!(p.remove_camera(id));
        assert!(!p.remove_camera(id));
        let json = serde_json::to_string(&CameraKind::CrossSection { back_clip: None }).unwrap();
        assert!(json.contains("CrossSection"));
    }

    #[test]
    fn symbol_is_triangle_plus_cone() {
        let mut c = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "c", 0);
        c.fov_deg = 90.0;
        c.clip_distance = Some(100.0);
        let tri = c.triangle_points();
        assert!(tri[0].x > 0.0 && tri[1].x < 0.0 && tri[2].x < 0.0);
        assert!((tri[1].y + tri[2].y).abs() < 1e-9);
        let cone = c.cone_points();
        assert_eq!(cone[0], Point::ZERO);
        // 45 degree edges; the far corners sit 100" ahead of the eye.
        assert!((cone[1].x - 100.0).abs() < 1e-9 && (cone[1].y - 100.0).abs() < 1e-9);
        assert!((cone[2].x - 100.0).abs() < 1e-9 && (cone[2].y + 100.0).abs() < 1e-9);
        assert_eq!(c.symbol_points().len(), 6);
        // Rotating the camera rotates the glyph.
        c.direction_deg = 90.0;
        assert!(c.triangle_points()[0].y > 0.0);
    }

    #[test]
    fn old_project_json_has_no_cameras() {
        let p = Project::from_json(
            r#"{"name":"x","floors":[{"name":"F","elevation":0.0,"ceiling_height":96.0,
                "walls":[],"openings":[]}],"next_id":1}"#,
        )
        .unwrap();
        assert!(p.cameras.is_empty() && p.wall_types.is_empty());
        assert!(p.floors[0].symbols.is_empty() && p.floors[0].groups.is_empty());
    }

    #[test]
    fn walk_pose_interpolates_position_height_and_look() {
        let mut c = CameraObject::walkthrough(
            vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)],
            60.0,
            "Walk",
            0,
        );
        c.path_nodes[1].height = 80.0;
        let mid = c.walk_pose(0.5);
        assert!((mid.position.x - 50.0).abs() < 1e-9 && mid.position.y.abs() < 1e-9);
        assert!((mid.eye_height - 70.0).abs() < 1e-9);
        assert!(mid.direction_deg.abs() < 1e-9);
        assert_eq!(c.walk_pose(0.0).position, Point::new(0.0, 0.0));
        assert_eq!(c.walk_pose(2.0).position, Point::new(100.0, 0.0));
        // 36 in/s: 100" takes a bit under three seconds.
        assert!((c.walk_duration_s() - 100.0 / 36.0).abs() < 1e-9);
        assert_eq!(c.walk_pose_at_time(c.walk_duration_s()).position.x, 100.0);
    }

    #[test]
    fn walk_pose_follows_corners_and_fixed_looks() {
        let mut c = CameraObject::walkthrough(
            vec![
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(100.0, 100.0),
            ],
            66.0,
            "Walk",
            0,
        );
        // Halfway along the path is the corner; the look turns 45 degrees.
        let corner = c.walk_pose(0.5);
        assert!((corner.position.x - 100.0).abs() < 1e-9 && corner.position.y.abs() < 1e-9);
        assert!((corner.direction_deg - 45.0).abs() < 1e-6);
        // A node can pin its look direction; interpolation takes the short arc.
        c.path_nodes[0].look_deg = Some(350.0);
        c.path_nodes[1].look_deg = Some(10.0);
        let q = c.walk_pose(0.25);
        assert!(q.direction_deg.rem_euclid(360.0) < 1e-6 || (q.direction_deg - 360.0).abs() < 1e-6);
    }

    #[test]
    fn key_frames_hold_and_tilt_along_the_walk() {
        let mut c = CameraObject::walkthrough(
            vec![
                Point::new(0.0, 0.0),
                Point::new(72.0, 0.0),
                Point::new(72.0, 72.0),
            ],
            66.0,
            "Walk",
            0,
        );
        c.walk_speed = 36.0;
        // Two seconds standing at the corner, looking up 20 degrees there.
        c.path_nodes[1].hold_s = 2.0;
        c.path_nodes[1].tilt_deg = 20.0;
        assert!((c.walk_hold_s() - 2.0).abs() < 1e-9);
        // 144" of path at 3 ft/s is four seconds, plus the hold.
        assert!((c.walk_duration_s() - 6.0).abs() < 1e-9);
        // Halfway up the first leg (one second in) the tilt is half way.
        let q = c.walk_pose_at_time(1.0);
        assert!((q.position.x - 36.0).abs() < 1e-9 && (q.tilt_deg - 10.0).abs() < 1e-9);
        // The corner is reached at 2 s and held until 4 s.
        for t in [2.0, 3.0, 3.99] {
            let p = c.walk_pose_at_time(t);
            assert!(
                (p.position.x - 72.0).abs() < 1e-9 && p.position.y.abs() < 1e-9,
                "t={t}"
            );
            assert!((p.tilt_deg - 20.0).abs() < 1e-9);
        }
        // It sets off again after the hold and ends at the last node.
        let p = c.walk_pose_at_time(5.0);
        assert!((p.position.y - 36.0).abs() < 1e-9, "{p:?}");
        assert_eq!(c.walk_pose_at_time(99.0).position, Point::new(72.0, 72.0));
        // The key frames' arrival times.
        assert_eq!(c.walk_node_time(0), 0.0);
        assert!((c.walk_node_time(1) - 2.0).abs() < 1e-9);
        assert!((c.walk_node_time(2) - 6.0).abs() < 1e-9);
        assert_eq!(c.walk_node_time(9), c.walk_node_time(2));
        assert_eq!(c.walk_pose_at_time(-3.0).position, Point::new(0.0, 0.0));
    }

    #[test]
    fn old_walkthrough_nodes_load_without_hold_or_tilt() {
        let n: WalkNode = serde_json::from_str(r#"{"height":66.0,"look_deg":null}"#).unwrap();
        assert_eq!(n, WalkNode::default());
        let cam = CameraObject::new(CameraKind::FloorCamera, Point::ZERO, 90.0, "Floor", 1);
        let back: CameraObject =
            serde_json::from_str(&serde_json::to_string(&cam).unwrap()).unwrap();
        assert_eq!(back, cam);
    }

    #[test]
    fn lights_round_trip_through_the_project_file() {
        let mut p = Project::new("lights");
        assert!(p.lights().is_empty());
        let mut l = PlanLight::new(Point::new(120.0, 90.0), 84.0);
        l.intensity = 2.5;
        l.color = [200, 220, 255];
        l.cast_shadows = false;
        let id = p.add_light(0, l.clone()).unwrap();
        let other = p
            .add_light(0, PlanLight::new(Point::new(0.0, 0.0), 60.0))
            .unwrap();
        assert!(p.add_light(5, l.clone()).is_none());
        assert!(p.update_light(other, |x| x.enabled = false));
        p.set_light_settings(LightSettings {
            use_electrical: false,
        });
        let back = Project::from_json(&p.to_json().unwrap()).unwrap();
        let lights = back.lights();
        assert_eq!(lights.len(), 2);
        let got = back.light(id).unwrap();
        assert_eq!(got.position, l.position);
        assert!((got.intensity - 2.5).abs() < 1e-6 && !got.cast_shadows);
        assert_eq!(got.color, [200, 220, 255]);
        assert!(!back.light(other).unwrap().enabled);
        assert!(!back.light_settings().use_electrical);
        // Typed slot: no data layer, no CAD records.
        assert!(back.layers.get(LIGHTS_LAYER).is_none());
        assert!(back.floors[0].cad.is_empty());
        let mut back = back;
        assert!(back.remove_light(id));
        assert!(!back.remove_light(id));
        assert_eq!(back.lights().len(), 1);
        // Settings are replaced, not duplicated.
        back.set_light_settings(LightSettings::default());
        back.set_light_settings(LightSettings::default());
        assert!(back.light_settings().use_electrical);
        assert!(back.floors[0].cad.is_empty());
    }

    #[test]
    fn legacy_light_records_move_into_the_typed_slot_once() {
        use crate::cad::CadObject;
        let mut p = Project::new("old");
        let rec = |id, text: String| CadObject {
            id,
            layer: LIGHTS_LAYER.to_string(),
            item: CadItem::Text {
                pos: Point::ZERO,
                text,
                height: 1.0,
                angle: 0.0,
            },
        };
        let mut l = PlanLight::new(Point::new(30.0, 40.0), 80.0);
        l.name = "Hall".into();
        // The old format kept neither id nor floor in the JSON.
        let mut json: serde_json::Value = serde_json::to_value(&l).unwrap();
        json.as_object_mut().unwrap().remove("id");
        json.as_object_mut().unwrap().remove("floor");
        p.floors[0].cad.push(rec(7, format!("{LIGHT_TAG}{json}")));
        p.floors[0].cad.push(rec(
            8,
            format!("{LIGHT_SET_TAG}{}", r#"{"use_electrical":false}"#),
        ));
        p.floors[0].cad.push(CadObject {
            id: 9,
            layer: "CAD, Default".into(),
            item: CadItem::Line {
                a: Point::ZERO,
                b: Point::new(1.0, 0.0),
            },
        });
        let mut layer = crate::layers::Layer::new(LIGHTS_LAYER, [0, 0, 0], 1);
        layer.display = false;
        p.layers.add(layer);
        assert!(migrate_legacy(&mut p));
        assert_eq!(p.lights.len(), 1);
        assert_eq!((p.lights[0].id, p.lights[0].floor), (7, 0));
        assert_eq!(p.lights[0].name, "Hall");
        assert!(!p.light_settings().use_electrical);
        // Only the unrelated CAD object stays; the data layer is gone.
        assert_eq!(p.floors[0].cad.len(), 1);
        assert!(p.layers.get(LIGHTS_LAYER).is_none());
        assert!(!migrate_legacy(&mut p));
        assert_eq!(p.lights.len(), 1);
    }

    #[test]
    fn old_cameras_have_no_path() {
        let c: CameraObject = serde_json::from_str(
            r#"{"id":1,"kind":"FullCamera","position":{"x":0.0,"y":0.0},"direction_deg":0.0,
                "eye_height":66.0,"fov_deg":60.0,"clip_distance":null,"name":"c","floor":0}"#,
        )
        .unwrap();
        assert!(c.path.is_empty() && c.path_nodes.is_empty());
        assert_eq!(c.walk_speed, DEFAULT_WALK_SPEED);
    }

    #[test]
    fn new_camera_options_default_and_round_trip() {
        let c: CameraObject = serde_json::from_str(
            r#"{"id":1,"kind":"Elevation","position":{"x":0.0,"y":0.0},"direction_deg":90.0,
                "eye_height":66.0,"fov_deg":60.0,"clip_distance":null,"name":"c","floor":0}"#,
        )
        .unwrap();
        assert_eq!(c.vector, VectorOptions::default());
        assert!(c.callout.show && c.callout.number.is_none());
        assert!(c.vector.level_labels && !c.vector.dimensions);
        let mut d = c.clone();
        d.vector.dimensions = true;
        d.vector.material_labels = true;
        d.callout.number = Some(4);
        let back: CameraObject = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn callouts_number_themselves_around_explicit_numbers() {
        let mut p = Project::new("n");
        let mk = |p: &mut Project, kind, number: Option<u32>, show: bool| {
            let mut c = CameraObject::new(kind, Point::ZERO, 90.0, "c", 0);
            c.callout.number = number;
            c.callout.show = show;
            p.add_camera(c)
        };
        let full = mk(&mut p, CameraKind::FullCamera, None, true);
        let a = mk(&mut p, CameraKind::Elevation, None, true);
        let b = mk(&mut p, CameraKind::WallElevation, Some(1), true);
        let hidden = mk(&mut p, CameraKind::Elevation, None, false);
        let c = mk(
            &mut p,
            CameraKind::CrossSection { back_clip: None },
            None,
            true,
        );
        // Only section-like cameras with a callout are numbered; 1 is taken.
        assert_eq!(p.callout_number(full), None);
        assert_eq!(p.callout_number(hidden), None);
        assert_eq!(p.callout_number(b), Some(1));
        assert_eq!(p.callout_number(a), Some(2));
        assert_eq!(p.callout_number(c), Some(3));
        assert_eq!(p.callout_cameras().len(), 3);
    }

    fn room_project(w: f64, h: f64) -> Project {
        use crate::model::{WallKind, DEFAULT_CEILING_HEIGHT};
        let mut p = Project::new("room");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, h),
            Point::new(0.0, h),
        ];
        for i in 0..4 {
            p.add_wall(
                0,
                c[i],
                c[(i + 1) % 4],
                6.0,
                DEFAULT_CEILING_HEIGHT,
                WallKind::Exterior,
            );
        }
        p
    }

    #[test]
    fn interior_elevations_make_four_wall_cameras_named_by_direction() {
        let mut p = room_project(240.0, 144.0);
        p.floors[0].room_names.push(crate::model::RoomName {
            anchor: Point::new(120.0, 72.0),
            name: "Kitchen".into(),
            ..Default::default()
        });
        let r = p
            .auto_interior_elevations(0, Point::new(100.0, 60.0))
            .unwrap();
        assert_eq!(r.room, "Kitchen");
        let names: Vec<&str> = r.cameras.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Kitchen North Wall",
                "Kitchen East Wall",
                "Kitchen South Wall",
                "Kitchen West Wall"
            ]
        );
        assert!(r
            .cameras
            .iter()
            .all(|c| c.kind == CameraKind::WallElevation));
        // The north-wall camera stands at the room's south side looking at +Y, as wide as the
        // room (240 - 2 x 3 inside), reaching the room's depth plus the wall.
        let n = &r.cameras[0];
        let s = n.section.unwrap();
        assert!((n.direction_deg - 90.0).abs() < 1e-9);
        assert!((s.a.dist(s.b) - 234.0).abs() < 1e-6);
        assert!((n.position.y - (3.0 + INTERIOR_CUT_INSET)).abs() < 1e-6);
        let depth = 144.0 - 6.0 - 2.0 * INTERIOR_CUT_INSET + INTERIOR_BACK_EXTRA;
        assert!((s.back_clip.unwrap() - depth).abs() < 1e-6);
        // Looking east: the line is the room's height long, at the west side.
        let e = r.cameras[1].section.unwrap();
        assert!((e.a.dist(e.b) - 138.0).abs() < 1e-6);
        assert!((r.cameras[1].position.x - (3.0 + INTERIOR_CUT_INSET)).abs() < 1e-6);
        // A click outside any room, or on a floor without walls, finds nothing.
        assert!(p
            .auto_interior_elevations(0, Point::new(500.0, 500.0))
            .is_none());
        assert!(Project::new("e")
            .auto_interior_elevations(0, Point::ZERO)
            .is_none());
    }
}
