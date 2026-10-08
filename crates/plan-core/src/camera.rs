//! Camera objects placed in the floor plan (`docs/parity/3d-views-cameras.md`,
//! C-4..C-30). The 3D viewer builds its views from these; here they are plain
//! data plus the plan symbol geometry.

use crate::cad::{CadItem, CadObject};
use crate::geometry::Point;
use crate::layers::Layer;
use crate::model::{Id, Project};
use serde::{Deserialize, Serialize};

/// Length of the view cone drawn in plan when no clip distance is set, inches.
pub const DEFAULT_CONE_LENGTH: f64 = 240.0;
/// Default eye height above the floor, inches (C-5).
pub const DEFAULT_EYE_HEIGHT: f64 = 66.0;
/// Default horizontal angle of view, degrees (C-7).
pub const DEFAULT_FOV_DEG: f64 = 60.0;
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
}

impl Default for WalkNode {
    fn default() -> Self {
        Self {
            height: DEFAULT_EYE_HEIGHT,
            look_deg: None,
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
}

/// Interpolates angles (degrees) along the shortest arc.
fn lerp_deg(a: f64, b: f64, t: f64) -> f64 {
    let d = (b - a + 180.0).rem_euclid(360.0) - 180.0;
    a + d * t
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
                look_deg: None,
            })
            .collect();
        c.path = path;
        c
    }

    /// Node `i` of the walkthrough path (defaults past the stored list).
    pub fn walk_node(&self, i: usize) -> WalkNode {
        self.path_nodes.get(i).copied().unwrap_or(WalkNode {
            height: self.eye_height,
            look_deg: None,
        })
    }

    /// Length of the walkthrough path, inches.
    pub fn walk_length(&self) -> f64 {
        self.path.windows(2).map(|w| w[0].dist(w[1])).sum()
    }

    /// Time to walk the whole path at `walk_speed`, seconds.
    pub fn walk_duration_s(&self) -> f64 {
        if self.walk_speed > 0.0 {
            self.walk_length() / self.walk_speed
        } else {
            0.0
        }
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
    /// the position moves in straight lines between nodes, the eye height is
    /// interpolated between the node heights and the look direction along the
    /// shortest arc between the node directions. A one-node path stays put.
    pub fn walk_pose(&self, u: f64) -> WalkPose {
        let n = self.path.len();
        if n == 0 {
            return WalkPose {
                position: self.position,
                eye_height: self.eye_height,
                direction_deg: self.direction_deg,
            };
        }
        if n == 1 {
            return WalkPose {
                position: self.path[0],
                eye_height: self.walk_node(0).height,
                direction_deg: self.node_look_deg(0),
            };
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
        let (a, b) = (self.walk_node(seg), self.walk_node(seg + 1));
        WalkPose {
            position: Point::lerp(self.path[seg], self.path[seg + 1], t),
            eye_height: a.height + (b.height - a.height) * t,
            direction_deg: lerp_deg(self.node_look_deg(seg), self.node_look_deg(seg + 1), t),
        }
    }

    /// The pose `t_s` seconds into the walk (clamped to the path).
    pub fn walk_pose_at_time(&self, t_s: f64) -> WalkPose {
        let d = self.walk_duration_s();
        self.walk_pose(if d > 0.0 { t_s / d } else { 0.0 })
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
        let tip = self.position + d * (GLYPH_LENGTH * 0.5);
        let back = self.position - d * (GLYPH_LENGTH * 0.5);
        [
            tip,
            back + n * GLYPH_HALF_WIDTH,
            back - n * GLYPH_HALF_WIDTH,
        ]
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

    /// Cameras placed on `floor`.
    pub fn cameras_on(&self, floor: usize) -> impl Iterator<Item = &CameraObject> {
        self.cameras.iter().filter(move |c| c.floor == floor)
    }
}

// ----- lights (C-64..C-66) -----

/// The hidden layer the light records live on. Lights are kept as tagged
/// text records in the floor's CAD list because the project model has no typed
/// slot for them yet; the accessors below are the only code that knows that.
pub const LIGHTS_LAYER: &str = "Lights, Data";
const LIGHT_TAG: &str = "plan-light:";
const LIGHT_SET_TAG: &str = "plan-lightset:";

/// A point light placed in the plan (C-64): position, height above the floor,
/// intensity (1.0 is a typical room light), colour and shadow options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanLight {
    /// The record id (assigned by [`Project::add_light`]; not stored in the JSON).
    #[serde(skip)]
    pub id: Id,
    /// The floor the light was placed on (not stored in the JSON).
    #[serde(skip)]
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

fn data_text<'a>(o: &'a CadObject, tag: &str) -> Option<&'a str> {
    if o.layer != LIGHTS_LAYER {
        return None;
    }
    match &o.item {
        CadItem::Text { text, .. } => text.strip_prefix(tag),
        _ => None,
    }
}

impl Project {
    fn ensure_lights_layer(&mut self) {
        if self.layers.get(LIGHTS_LAYER).is_none() {
            let mut l = Layer::new(LIGHTS_LAYER, [0xC8, 0xA0, 0x20], 18);
            l.display = false;
            self.layers.add(l);
        }
    }

    /// Every light of the plan, floor by floor.
    pub fn lights(&self) -> Vec<PlanLight> {
        let mut out = Vec::new();
        for (floor, f) in self.floors.iter().enumerate() {
            for o in &f.cad {
                if let Some(json) = data_text(o, LIGHT_TAG) {
                    if let Ok(mut l) = serde_json::from_str::<PlanLight>(json) {
                        l.id = o.id;
                        l.floor = floor;
                        out.push(l);
                    }
                }
            }
        }
        out
    }

    pub fn light(&self, id: Id) -> Option<PlanLight> {
        self.lights().into_iter().find(|l| l.id == id)
    }

    /// Adds a light to `floor`; `None` if the floor does not exist.
    pub fn add_light(&mut self, floor: usize, light: PlanLight) -> Option<Id> {
        if floor >= self.floors.len() {
            return None;
        }
        self.ensure_lights_layer();
        let id = self.alloc_id();
        let json = serde_json::to_string(&light).ok()?;
        self.floors[floor].cad.push(CadObject {
            id,
            layer: LIGHTS_LAYER.to_string(),
            item: CadItem::Text {
                pos: light.position,
                text: format!("{LIGHT_TAG}{json}"),
                height: 1.0,
                angle: 0.0,
            },
        });
        Some(id)
    }

    /// Edits a light in place; `false` if there is no such light. Its id and
    /// floor cannot be changed by `edit`.
    pub fn update_light(&mut self, id: Id, edit: impl FnOnce(&mut PlanLight)) -> bool {
        for f in &mut self.floors {
            let Some(o) = f
                .cad
                .iter_mut()
                .find(|o| o.id == id && data_text(o, LIGHT_TAG).is_some())
            else {
                continue;
            };
            let Some(mut l) =
                data_text(o, LIGHT_TAG).and_then(|j| serde_json::from_str::<PlanLight>(j).ok())
            else {
                return false;
            };
            edit(&mut l);
            if let Ok(json) = serde_json::to_string(&l) {
                o.item = CadItem::Text {
                    pos: l.position,
                    text: format!("{LIGHT_TAG}{json}"),
                    height: 1.0,
                    angle: 0.0,
                };
                return true;
            }
            return false;
        }
        false
    }

    /// Removes a light; returns whether it existed.
    pub fn remove_light(&mut self, id: Id) -> bool {
        let mut found = false;
        for f in &mut self.floors {
            let n = f.cad.len();
            f.cad
                .retain(|o| !(o.id == id && data_text(o, LIGHT_TAG).is_some()));
            found |= f.cad.len() != n;
        }
        found
    }

    /// The plan-wide light options (defaults when none are stored).
    pub fn light_settings(&self) -> LightSettings {
        self.floors
            .iter()
            .flat_map(|f| f.cad.iter())
            .find_map(|o| data_text(o, LIGHT_SET_TAG))
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default()
    }

    pub fn set_light_settings(&mut self, settings: LightSettings) {
        if self.floors.is_empty() {
            return;
        }
        self.ensure_lights_layer();
        for f in &mut self.floors {
            f.cad.retain(|o| data_text(o, LIGHT_SET_TAG).is_none());
        }
        let id = self.alloc_id();
        if let Ok(json) = serde_json::to_string(&settings) {
            self.floors[0].cad.push(CadObject {
                id,
                layer: LIGHTS_LAYER.to_string(),
                item: CadItem::Text {
                    pos: Point::ZERO,
                    text: format!("{LIGHT_SET_TAG}{json}"),
                    height: 1.0,
                    angle: 0.0,
                },
            });
        }
    }
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
        // The data layer is hidden.
        assert!(!back.layers.is_visible(LIGHTS_LAYER));
        let mut back = back;
        assert!(back.remove_light(id));
        assert!(!back.remove_light(id));
        assert_eq!(back.lights().len(), 1);
        // Settings are replaced, not duplicated.
        back.set_light_settings(LightSettings::default());
        back.set_light_settings(LightSettings::default());
        assert!(back.light_settings().use_electrical);
        assert_eq!(back.floors[0].cad.len(), 2);
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
}
