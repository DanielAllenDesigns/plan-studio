//! Camera model: orbit, first-person and orthographic elevation views.
//!
//! Scene space is X right, Y up (inches), Z toward the viewer (= -plan y).
//! `yaw` is a rotation about +Y with `yaw = 0` meaning "viewer standing on +Z
//! looking toward -Z" (the front of the plan). `pitch` is radians above the
//! horizon: for orbit-style cameras it is the elevation of the *eye* above the
//! target, for [`CameraMode::FullCamera`] it is how far the view looks *up*.

use crate::math::{self, Mat4, Vec3};

/// Default orbit yaw, radians (about 34 degrees off the front).
const DEFAULT_YAW: f32 = 0.6;
/// Default pitch for [`CameraMode::Orbit`], radians (30 degrees).
const ORBIT_PITCH: f32 = 30.0 * std::f32::consts::PI / 180.0;
/// Default pitch for [`CameraMode::DollHouse`], radians (55 degrees).
const DOLLHOUSE_PITCH: f32 = 55.0 * std::f32::consts::PI / 180.0;
/// Pitch limit for orbit and look-around, radians (just short of straight up/down).
const MAX_PITCH: f32 = 1.5;
/// Extra room left around the scene when fitting, as a multiplier.
const FIT_MARGIN: f32 = 1.1;
/// Inches travelled by a unit of [`Camera::dolly`] in first-person mode.
const WALK_DOLLY_INCHES: f32 = 600.0;

/// How the scene is viewed. Mirrors Chief Architect's camera views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraMode {
    /// Perspective orbit around the model ("Perspective Full Overview").
    Orbit,
    /// Orbit with ceilings and roof hidden, tilted down ("Doll House View").
    DollHouse,
    /// First-person walkthrough at eye height ("Full Camera").
    FullCamera,
    /// Orthographic elevation seen from the front (+Z).
    ElevationFront,
    /// Orthographic elevation seen from the back (-Z).
    ElevationBack,
    /// Orthographic elevation seen from the left (-X).
    ElevationLeft,
    /// Orthographic elevation seen from the right (+X).
    ElevationRight,
    /// Orthographic top-down view of the whole model.
    PlanOverhead,
}

impl CameraMode {
    /// True for the parallel-projection modes (elevations and plan overhead).
    pub fn is_orthographic(self) -> bool {
        matches!(
            self,
            CameraMode::ElevationFront
                | CameraMode::ElevationBack
                | CameraMode::ElevationLeft
                | CameraMode::ElevationRight
                | CameraMode::PlanOverhead
        )
    }

    /// True for the modes that orbit around a target point.
    pub fn is_orbit_like(self) -> bool {
        matches!(self, CameraMode::Orbit | CameraMode::DollHouse)
    }

    /// True when ceiling and roof meshes should not be drawn.
    pub fn hides_ceiling_and_roof(self) -> bool {
        matches!(self, CameraMode::DollHouse | CameraMode::PlanOverhead)
    }
}

/// Chief-style view names paired with their [`CameraMode`], in menu order.
pub fn standard_views() -> Vec<(&'static str, CameraMode)> {
    vec![
        ("Perspective Full Overview", CameraMode::Orbit),
        ("Doll House View", CameraMode::DollHouse),
        ("Full Camera", CameraMode::FullCamera),
        ("Front Elevation", CameraMode::ElevationFront),
        ("Back Elevation", CameraMode::ElevationBack),
        ("Left Elevation", CameraMode::ElevationLeft),
        ("Right Elevation", CameraMode::ElevationRight),
        ("Plan Overhead", CameraMode::PlanOverhead),
    ]
}

/// A camera. Public fields may be edited directly; the helper methods keep
/// pitch and distance within sensible limits.
#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    /// Active view mode.
    pub mode: CameraMode,
    /// Point orbited around / framed by the orthographic views.
    pub target: [f32; 3],
    /// Eye-to-target distance for orbit and orthographic modes, inches.
    pub distance: f32,
    /// Rotation about +Y, radians (see module docs).
    pub yaw: f32,
    /// Elevation angle, radians (see module docs).
    pub pitch: f32,
    /// Eye height above the floor for `FullCamera`, inches (66" default).
    pub eye_height: f32,
    /// Eye position used by `FullCamera`, inches.
    pub position: [f32; 3],
    /// Vertical field of view for perspective modes, degrees.
    pub fov_deg: f32,
    /// Half the visible height of the orthographic views, inches.
    pub ortho_half_height: f32,
    /// Clip Surfaces Within (manual p. 1189): objects nearer than this to
    /// the camera are not drawn. `None` keeps the mode's own near plane.
    pub near: Option<f32>,
    /// Parallel projection for an orbit-like mode: the Orthographic Full,
    /// Floor and Framing Overviews and the Isometric Views (C-15). Zoom
    /// changes `ortho_half_height`, as in the orthographic elevations.
    pub parallel: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            mode: CameraMode::Orbit,
            target: [0.0, 0.0, 0.0],
            distance: 1200.0,
            yaw: DEFAULT_YAW,
            pitch: ORBIT_PITCH,
            eye_height: 66.0,
            position: [0.0, 66.0, 600.0],
            fov_deg: 60.0,
            ortho_half_height: 600.0,
            near: None,
            parallel: false,
        }
    }
}

impl Camera {
    /// Is the projection parallel (an orthographic mode, or an orbit-like
    /// mode with [`Camera::parallel`])?
    pub fn is_parallel(&self) -> bool {
        self.parallel || self.mode.is_orthographic()
    }

    /// Makes an orbit-like view parallel, keeping what it shows: the visible
    /// height is that of the perspective view at the target's distance.
    pub fn make_parallel(&mut self) {
        self.ortho_half_height =
            (self.distance * (self.fov_deg.to_radians() * 0.5).tan()).clamp(6.0, 1.0e6);
        self.parallel = true;
    }

    /// Switch view mode and apply that mode's default orientation.
    ///
    /// Orbit/doll house keep the current yaw when coming from another orbit
    /// view, `FullCamera` keeps the current yaw and position, and the
    /// orthographic modes snap to their fixed direction.
    pub fn set_mode(&mut self, mode: CameraMode) {
        self.parallel = false;
        let was_orbit_like = self.mode.is_orbit_like();
        let was_full = self.mode == CameraMode::FullCamera;
        match mode {
            CameraMode::Orbit | CameraMode::DollHouse => {
                if !was_orbit_like && !was_full {
                    self.yaw = DEFAULT_YAW;
                }
                self.pitch = if mode == CameraMode::DollHouse {
                    DOLLHOUSE_PITCH
                } else {
                    ORBIT_PITCH
                };
            }
            CameraMode::FullCamera => self.pitch = 0.0,
            CameraMode::ElevationFront => (self.yaw, self.pitch) = (0.0, 0.0),
            CameraMode::ElevationBack => (self.yaw, self.pitch) = (std::f32::consts::PI, 0.0),
            CameraMode::ElevationLeft => {
                (self.yaw, self.pitch) = (-std::f32::consts::FRAC_PI_2, 0.0)
            }
            CameraMode::ElevationRight => {
                (self.yaw, self.pitch) = (std::f32::consts::FRAC_PI_2, 0.0)
            }
            CameraMode::PlanOverhead => (self.yaw, self.pitch) = (0.0, std::f32::consts::FRAC_PI_2),
        }
        self.mode = mode;
    }

    /// The equivalent [`plan_render::Camera`] (a pinhole, `up` = +Y).
    ///
    /// `FullCamera` looks along [`Camera::forward`] from `position`; the other
    /// modes look from [`Camera::eye`] at `target`. The ray tracer has no
    /// orthographic projection, so the orthographic modes become perspective
    /// views from the same eye.
    pub fn to_render_camera(&self) -> plan_render::Camera {
        let eye = self.eye();
        let target = if self.mode == CameraMode::FullCamera {
            math::add(eye, math::scale(self.forward(), 100.0))
        } else {
            self.target
        };
        plan_render::Camera {
            eye,
            target,
            up: [0.0, 1.0, 0.0],
            fov_deg: self.fov_deg,
            aperture: 0.0,
            focus_dist: math::length(math::sub(target, eye)).max(1.0),
        }
    }

    /// Unit vector from the target toward the eye for orbit-style cameras.
    fn eye_offset(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [sy * cp, sp, cy * cp]
    }

    /// World-space eye position.
    pub fn eye(&self) -> Vec3 {
        if self.mode == CameraMode::FullCamera {
            self.position
        } else {
            math::add(self.target, math::scale(self.eye_offset(), self.distance))
        }
    }

    /// Unit view direction.
    pub fn forward(&self) -> Vec3 {
        if self.mode == CameraMode::FullCamera {
            let (sy, cy) = self.yaw.sin_cos();
            let (sp, cp) = self.pitch.sin_cos();
            [-sy * cp, sp, -cy * cp]
        } else {
            math::scale(self.eye_offset(), -1.0)
        }
    }

    /// Up hint for `look_at`; switches to -Z when looking straight down.
    fn up_hint(&self, forward: Vec3) -> Vec3 {
        if forward[1] < -0.999 {
            [0.0, 0.0, -1.0]
        } else if forward[1] > 0.999 {
            [0.0, 0.0, 1.0]
        } else {
            [0.0, 1.0, 0.0]
        }
    }

    /// Camera right and up unit vectors.
    fn right_up(&self) -> (Vec3, Vec3) {
        let f = self.forward();
        let right = math::normalize(math::cross(f, self.up_hint(f)));
        let up = math::cross(right, f);
        (right, up)
    }

    /// World-to-view matrix (column-major).
    pub fn view_matrix(&self) -> Mat4 {
        let eye = self.eye();
        let f = self.forward();
        math::look_at(eye, math::add(eye, f), self.up_hint(f))
    }

    /// View-to-clip matrix for a viewport of the given width/height ratio.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        let aspect = if aspect.is_finite() && aspect > 1e-3 {
            aspect
        } else {
            1.0
        };
        if self.is_parallel() {
            let h = self.ortho_half_height.max(1e-3);
            let w = h * aspect;
            // A negative near plane keeps geometry behind the eye plane visible.
            math::ortho(-w, w, -h, h, -self.distance, self.distance * 3.0)
        } else {
            let (near, far) = if self.mode == CameraMode::FullCamera {
                (2.0, 20_000.0_f32.max(self.distance * 10.0))
            } else {
                (
                    (self.distance * 0.01).max(0.5),
                    (self.distance * 20.0).max(20_000.0),
                )
            };
            let near = self.near.map_or(near, |n| n.clamp(0.1, far * 0.5));
            math::perspective(self.fov_deg.to_radians(), aspect, near, far)
        }
    }

    /// `projection * view`.
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        math::mul(&self.projection_matrix(aspect), &self.view_matrix())
    }

    /// Frame the axis-aligned box `min..max` assuming a square viewport.
    ///
    /// Sets `target` to the box centre, `distance` so the bounding sphere fits
    /// the vertical field of view, `ortho_half_height` for the current
    /// orthographic direction, and places the `FullCamera` at the plan centre.
    pub fn fit_to_bounds(&mut self, min: [f32; 3], max: [f32; 3]) {
        self.fit_to_bounds_with_aspect(min, max, 1.0);
    }

    /// Like [`Camera::fit_to_bounds`] but sizes the orthographic views for a
    /// viewport of the given width/height ratio.
    pub fn fit_to_bounds_with_aspect(&mut self, min: [f32; 3], max: [f32; 3], aspect: f32) {
        let center = math::scale(math::add(min, max), 0.5);
        let size = math::sub(max, min);
        let radius = (0.5 * math::length(size)).max(1.0);
        let half_fov = (self.fov_deg.to_radians() * 0.5).max(0.01);
        self.target = center;
        self.distance = radius * FIT_MARGIN / half_fov.sin();
        let (horizontal, vertical) = match self.mode {
            CameraMode::ElevationFront | CameraMode::ElevationBack => (size[0], size[1]),
            CameraMode::ElevationLeft | CameraMode::ElevationRight => (size[2], size[1]),
            CameraMode::PlanOverhead => (size[0], size[2]),
            _ => (size[0].max(size[2]), size[1]),
        };
        let aspect = if aspect > 1e-3 { aspect } else { 1.0 };
        self.ortho_half_height = (0.5 * FIT_MARGIN * vertical.max(horizontal / aspect)).max(1.0);
        self.position = [center[0], min[1] + self.eye_height, center[2]];
    }

    /// Orbit by `dyaw`/`dpitch` radians. Ignored by non-orbit modes.
    pub fn orbit(&mut self, dyaw: f32, dpitch: f32) {
        if !self.mode.is_orbit_like() {
            return;
        }
        let min_pitch = if self.mode == CameraMode::DollHouse {
            0.05
        } else {
            -MAX_PITCH
        };
        self.yaw += dyaw;
        self.pitch = (self.pitch + dpitch).clamp(min_pitch, MAX_PITCH);
    }

    /// Pan by a screen-space drag of `dx`, `dy` pixels in a viewport that is
    /// `viewport_height_px` tall: the scene follows the cursor.
    pub fn pan(&mut self, dx: f32, dy: f32, viewport_height_px: f32) {
        let (right, up) = self.right_up();
        if self.mode == CameraMode::FullCamera {
            self.position = math::add(self.position, math::scale(right, -dx * 0.5));
            return;
        }
        let h = viewport_height_px.max(1.0);
        let per_px = if self.is_parallel() {
            2.0 * self.ortho_half_height / h
        } else {
            2.0 * self.distance * (self.fov_deg.to_radians() * 0.5).tan() / h
        };
        let shift = math::add(
            math::scale(right, -dx * per_px),
            math::scale(up, dy * per_px),
        );
        self.target = math::add(self.target, shift);
    }

    /// Make `center` the point the camera orbits (C-39, Alt-click) without
    /// moving the eye: the view turns to look at the point, and later orbit
    /// drags circle it at the distance it now has. Orbit and doll house
    /// cameras only; returns whether the centre changed. The pitch limit of
    /// the mode applies (a point almost straight above or below the eye moves
    /// the eye slightly).
    pub fn set_orbit_center(&mut self, center: Vec3) -> bool {
        if !self.mode.is_orbit_like() || center.iter().any(|c| !c.is_finite()) {
            return false;
        }
        let eye = self.eye();
        let offset = math::sub(eye, center);
        let distance = math::length(offset);
        if distance < 1.0 {
            return false;
        }
        let min_pitch = if self.mode == CameraMode::DollHouse {
            0.05
        } else {
            -MAX_PITCH
        };
        self.target = center;
        self.distance = distance;
        self.yaw = offset[0].atan2(offset[2]);
        self.pitch = (offset[1] / distance).asin().clamp(min_pitch, MAX_PITCH);
        true
    }

    /// Zoom in (`amount > 0`) or out (`amount < 0`) exponentially. In
    /// `FullCamera` this walks forward/back instead.
    pub fn dolly(&mut self, amount: f32) {
        let factor = (-amount).exp();
        match self.mode {
            CameraMode::FullCamera => self.walk(amount * WALK_DOLLY_INCHES, 0.0),
            m if m.is_orthographic() || self.parallel => {
                self.ortho_half_height = (self.ortho_half_height * factor).clamp(6.0, 1.0e6);
            }
            _ => self.distance = (self.distance * factor).clamp(10.0, 1.0e6),
        }
    }

    /// Move `forward` and `strafe` inches along the ground plane (the Y
    /// component of the view direction is ignored). Moves `position` in
    /// `FullCamera`, and `target` in the other modes.
    pub fn walk(&mut self, forward: f32, strafe: f32) {
        let (sy, cy) = self.yaw.sin_cos();
        // For orbit-style cameras `yaw` points from target to eye, which gives the
        // same ground heading: forward is the opposite of the eye offset.
        let (fwd, right) = ([-sy, 0.0, -cy], [cy, 0.0, -sy]);
        let delta = math::add(math::scale(fwd, forward), math::scale(right, strafe));
        if self.mode == CameraMode::FullCamera {
            self.position = math::add(self.position, delta);
        } else {
            self.target = math::add(self.target, delta);
        }
    }

    /// Turn the first-person view by `dyaw`/`dpitch` radians (pitch positive
    /// looks up). Ignored outside `FullCamera`.
    pub fn turn(&mut self, dyaw: f32, dpitch: f32) {
        if self.mode != CameraMode::FullCamera {
            return;
        }
        self.yaw += dyaw;
        self.pitch = (self.pitch + dpitch).clamp(-MAX_PITCH, MAX_PITCH);
    }

    /// How far the first-person view looks up (negative: down), degrees.
    /// Zero outside `FullCamera`.
    pub fn tilt_deg(&self) -> f32 {
        if self.mode == CameraMode::FullCamera {
            self.pitch.to_degrees()
        } else {
            0.0
        }
    }

    /// Sets how far the first-person view looks up, degrees, limited to
    /// +-85. Returns `false` (and does nothing) outside `FullCamera`.
    pub fn set_tilt_deg(&mut self, deg: f32) -> bool {
        if self.mode != CameraMode::FullCamera {
            return false;
        }
        self.pitch = deg.to_radians().clamp(-MAX_TILT, MAX_TILT);
        true
    }

    /// The eye and the point looked at: the orbit target, or for the
    /// first-person camera the point `distance` inches ahead.
    pub fn pose(&self) -> (Vec3, Vec3) {
        let eye = self.eye();
        if self.mode == CameraMode::FullCamera {
            (
                eye,
                math::add(eye, math::scale(self.forward(), self.distance)),
            )
        } else {
            (eye, self.target)
        }
    }

    /// Aims an orbit-style camera (Perspective Overview, Doll House) from
    /// `eye` at `target`, the inverse of [`Camera::pose`]. Returns `false`
    /// and leaves the camera alone for other modes or a degenerate pose.
    pub fn look_from(&mut self, eye: Vec3, target: Vec3) -> bool {
        if !self.mode.is_orbit_like() {
            return false;
        }
        let offset = math::sub(eye, target);
        let distance = math::length(offset);
        if !distance.is_finite() || distance < 1.0 {
            return false;
        }
        self.target = target;
        self.distance = distance;
        self.yaw = offset[0].atan2(offset[2]);
        self.pitch = (offset[1] / distance).asin().clamp(-MAX_PITCH, MAX_PITCH);
        true
    }
}

/// Tilt limit of the first-person view, radians (85 degrees).
const MAX_TILT: f32 = 85.0 * std::f32::consts::PI / 180.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilt_only_applies_to_the_first_person_camera() {
        let mut cam = Camera::default();
        assert!(!cam.set_tilt_deg(10.0));
        assert_eq!(cam.tilt_deg(), 0.0);
        cam.set_mode(CameraMode::FullCamera);
        assert!(cam.set_tilt_deg(20.0));
        assert!((cam.tilt_deg() - 20.0).abs() < 1e-4);
        assert!(cam.forward()[1] > 0.3, "tilting up raises the view");
        assert!(cam.set_tilt_deg(120.0));
        assert!((cam.tilt_deg() - 85.0).abs() < 1e-3);
        assert!(cam.set_tilt_deg(-120.0));
        assert!((cam.tilt_deg() + 85.0).abs() < 1e-3);
    }

    #[test]
    fn look_from_is_the_inverse_of_pose() {
        let mut cam = Camera::default();
        let (eye, target) = ([900.0, 700.0, 1500.0], [400.0, 60.0, -300.0]);
        assert!(cam.look_from(eye, target));
        let (e, t) = cam.pose();
        for i in 0..3 {
            assert!((e[i] - eye[i]).abs() < 0.05, "eye {i}: {e:?}");
            assert!((t[i] - target[i]).abs() < 0.05, "target {i}: {t:?}");
        }
        // Degenerate and first-person cameras are left alone.
        assert!(!cam.look_from(target, target));
        cam.set_mode(CameraMode::FullCamera);
        assert!(!cam.look_from(eye, target));
    }

    const MIN: [f32; 3] = [0.0, 0.0, -600.0];
    const MAX: [f32; 3] = [1200.0, 108.0, 0.0];

    fn corners() -> Vec<Vec3> {
        let mut out = Vec::new();
        for x in [MIN[0], MAX[0]] {
            for y in [MIN[1], MAX[1]] {
                for z in [MIN[2], MAX[2]] {
                    out.push([x, y, z]);
                }
            }
        }
        out
    }

    #[test]
    fn fit_to_bounds_contains_bounding_sphere() {
        let mut cam = Camera::default();
        cam.fit_to_bounds(MIN, MAX);
        let radius = 0.5 * math::length(math::sub(MAX, MIN));
        let half_fov = 30f32.to_radians();
        assert!(
            cam.distance * half_fov.sin() >= radius,
            "sphere must fit the FOV"
        );
        assert!(cam.distance > radius);
        // Every box corner lands inside NDC for a square viewport.
        let vp = cam.view_projection(1.0);
        for c in corners() {
            let p = math::transform_point(&vp, c);
            assert!(
                p[0].abs() <= 1.0 && p[1].abs() <= 1.0,
                "corner {c:?} -> {p:?}"
            );
            assert!(p[2].abs() <= 1.0);
        }
        // The target is centred.
        let t = math::transform_point(&vp, cam.target);
        assert!(t[0].abs() < 1e-3 && t[1].abs() < 1e-3);
    }

    #[test]
    fn elevation_projection_is_parallel() {
        let mut cam = Camera::default();
        cam.set_mode(CameraMode::ElevationFront);
        cam.fit_to_bounds(MIN, MAX);
        let vp = cam.view_projection(1.6);
        let near_pt = math::transform_point(&vp, [300.0, 50.0, 0.0]);
        let far_pt = math::transform_point(&vp, [300.0, 50.0, -500.0]);
        assert!((near_pt[0] - far_pt[0]).abs() < 1e-4);
        assert!((near_pt[1] - far_pt[1]).abs() < 1e-4);
        assert!(
            (near_pt[2] - far_pt[2]).abs() > 1e-4,
            "depth must still differ"
        );

        // A perspective camera does not behave that way.
        let mut persp = Camera::default();
        persp.fit_to_bounds(MIN, MAX);
        let vp = persp.view_projection(1.6);
        let a = math::transform_point(&vp, [300.0, 50.0, 0.0]);
        let b = math::transform_point(&vp, [300.0, 50.0, -500.0]);
        assert!((a[0] - b[0]).abs() > 1e-3 || (a[1] - b[1]).abs() > 1e-3);
    }

    #[test]
    fn elevations_face_the_expected_sides() {
        let mut cam = Camera::default();
        cam.fit_to_bounds(MIN, MAX);
        let cases = [
            (CameraMode::ElevationFront, [0.0, 0.0, 1.0]),
            (CameraMode::ElevationBack, [0.0, 0.0, -1.0]),
            (CameraMode::ElevationLeft, [-1.0, 0.0, 0.0]),
            (CameraMode::ElevationRight, [1.0, 0.0, 0.0]),
        ];
        for (mode, toward_eye) in cases {
            cam.set_mode(mode);
            let dir = math::normalize(math::sub(cam.eye(), cam.target));
            for k in 0..3 {
                assert!((dir[k] - toward_eye[k]).abs() < 1e-4, "{mode:?} axis {k}");
            }
        }
    }

    #[test]
    fn plan_overhead_looks_straight_down_with_finite_matrix() {
        let mut cam = Camera::default();
        cam.set_mode(CameraMode::PlanOverhead);
        cam.fit_to_bounds(MIN, MAX);
        let view = cam.view_matrix();
        assert!(view.iter().all(|v| v.is_finite()));
        // Plan +x is screen right; plan +y (= -Z) is screen up.
        let right = math::transform_point(&view, math::add(cam.target, [100.0, 0.0, 0.0]));
        let up = math::transform_point(&view, math::add(cam.target, [0.0, 0.0, -100.0]));
        assert!(right[0] > 50.0 && up[1] > 50.0);
    }

    #[test]
    fn dollhouse_defaults_to_55_degrees_and_hides_roof() {
        let mut cam = Camera::default();
        cam.set_mode(CameraMode::DollHouse);
        assert!((cam.pitch.to_degrees() - 55.0).abs() < 0.01);
        assert!(cam.mode.hides_ceiling_and_roof());
        assert!(!CameraMode::Orbit.hides_ceiling_and_roof());
    }

    #[test]
    fn full_camera_walks_along_its_heading() {
        let mut cam = Camera::default();
        cam.set_mode(CameraMode::FullCamera);
        cam.yaw = 0.0;
        cam.position = [0.0, 66.0, 0.0];
        cam.walk(100.0, 0.0);
        assert!(
            (cam.position[2] + 100.0).abs() < 1e-3,
            "yaw 0 walks toward -Z"
        );
        cam.walk(0.0, 50.0);
        assert!((cam.position[0] - 50.0).abs() < 1e-3, "strafe right is +X");
        assert!(
            (cam.position[1] - 66.0).abs() < 1e-6,
            "eye height is preserved"
        );
    }

    #[test]
    fn standard_views_match_chief_names() {
        let views = standard_views();
        assert_eq!(views.len(), 8);
        assert_eq!(views[0], ("Perspective Full Overview", CameraMode::Orbit));
        assert_eq!(views[7], ("Plan Overhead", CameraMode::PlanOverhead));
    }

    #[test]
    fn setting_the_orbit_center_keeps_the_eye_and_orbits_the_new_point() {
        let mut cam = Camera::default();
        cam.fit_to_bounds(MIN, MAX);
        let eye = cam.eye();
        let point = [900.0, 40.0, -100.0];
        assert!(cam.set_orbit_center(point));
        assert_eq!(cam.target, point);
        let now = cam.eye();
        for k in 0..3 {
            assert!((now[k] - eye[k]).abs() < 0.05, "{now:?} vs {eye:?}");
        }
        // The view now looks at the point.
        let f = cam.forward();
        let to = math::normalize(math::sub(point, now));
        assert!(math::dot(f, to) > 0.9999);
        // Orbiting circles the point at the same distance.
        let d = cam.distance;
        cam.orbit(0.7, 0.1);
        assert_eq!(cam.target, point);
        assert!((cam.distance - d).abs() < 1e-3);
        // Walk-through and orthographic cameras have no orbit centre; a point
        // on the eye is refused.
        let mut full = Camera::default();
        full.set_mode(CameraMode::FullCamera);
        assert!(!full.set_orbit_center(point));
        let mut ortho = Camera::default();
        ortho.set_mode(CameraMode::ElevationFront);
        assert!(!ortho.set_orbit_center(point));
        let mut cam = Camera::default();
        assert!(!cam.set_orbit_center(cam.eye()));
        assert!(!cam.set_orbit_center([f32::NAN, 0.0, 0.0]));
    }

    #[test]
    fn dolly_zooms_orbit_distance_and_ortho_height() {
        let mut cam = Camera::default();
        let d = cam.distance;
        cam.dolly(0.5);
        assert!(cam.distance < d);
        cam.set_mode(CameraMode::ElevationFront);
        let h = cam.ortho_half_height;
        cam.dolly(-0.5);
        assert!(cam.ortho_half_height > h);
    }
}
