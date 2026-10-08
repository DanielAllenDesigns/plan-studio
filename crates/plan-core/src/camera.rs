//! Camera objects placed in the floor plan (`docs/parity/3d-views-cameras.md`,
//! C-4..C-30). The 3D viewer builds its views from these; here they are plain
//! data plus the plan symbol geometry.

use crate::geometry::Point;
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
        }
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
}
