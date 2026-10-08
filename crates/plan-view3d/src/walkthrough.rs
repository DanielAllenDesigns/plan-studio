//! Walkthroughs: a camera that follows a path in plan (or a list of key
//! frames) at eye height, mirroring Chief's Create Walkthrough Path, Record
//! Walkthrough Along Path and key-framed camera tools.
//!
//! Positions are scene space (inches): `[plan x, elevation, -plan y]`. Angles
//! use the [`Camera`] conventions (radians; `yaw = 0` looks toward -Z, pitch
//! positive looks up).

use plan_core::Point;

use crate::camera::{Camera, CameraMode};

type V3 = [f64; 3];

/// Default walking speed, inches per second (3 ft/s).
const DEFAULT_SPEED: f64 = 36.0;
/// Default distance ahead of the camera it looks toward, inches.
const DEFAULT_LOOK_AHEAD: f64 = 60.0;
/// Default eye height above the path, inches.
const DEFAULT_EYE_HEIGHT: f64 = 66.0;
/// Default vertical field of view, degrees.
const DEFAULT_FOV: f64 = 60.0;
/// Default recording rate, frames per second.
const DEFAULT_FPS: f64 = 24.0;
/// Catmull-Rom samples per control-point span when flattening a smooth path.
const SPAN_SAMPLES: usize = 16;

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn length(a: V3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn lerp(a: V3, b: V3, u: f64) -> V3 {
    add(a, scale(sub(b, a), u))
}

/// Uniform Catmull-Rom point between `p1` (`u = 0`) and `p2` (`u = 1`).
fn catmull_rom(p0: V3, p1: V3, p2: V3, p3: V3, u: f64) -> V3 {
    let (u2, u3) = (u * u, u * u * u);
    let mut out = [0.0; 3];
    for (k, o) in out.iter_mut().enumerate() {
        *o = 0.5
            * (2.0 * p1[k]
                + (p2[k] - p0[k]) * u
                + (2.0 * p0[k] - 5.0 * p1[k] + 4.0 * p2[k] - p3[k]) * u2
                + (3.0 * p1[k] - p0[k] - 3.0 * p2[k] + p3[k]) * u3);
    }
    out
}

/// Spline control points before and after the span `i..i + 1`; past either end
/// the neighbour is mirrored so equally spaced collinear points stay straight.
fn spline_neighbours(pts: &[V3], i: usize) -> (V3, V3) {
    let before = if i == 0 {
        sub(scale(pts[0], 2.0), pts[1])
    } else {
        pts[i - 1]
    };
    let n = pts.len();
    let after = if i + 2 >= n {
        sub(scale(pts[n - 1], 2.0), pts[n - 2])
    } else {
        pts[i + 2]
    };
    (before, after)
}

/// Heading (yaw, radians) of a ground-plane direction `[dx, _, dz]`.
fn heading(d: V3) -> f64 {
    (-d[0]).atan2(-d[2])
}

/// Interpolate angles along the shortest arc.
fn lerp_angle(a: f64, b: f64, u: f64) -> f64 {
    let d = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    a + d * u
}

/// A camera path through the model.
#[derive(Debug, Clone, PartialEq)]
pub struct WalkthroughPath {
    /// Path vertices in scene space: plan x, floor elevation, -plan y.
    pub points: Vec<[f64; 3]>,
    /// Round the corners with a Catmull-Rom spline through the points.
    pub smooth: bool,
    /// Walking speed along the path, inches per second.
    pub speed_in_per_s: f64,
    /// Camera height above the path, inches.
    pub eye_height: f64,
    /// The camera looks toward the point this far ahead along the path, inches.
    pub look_ahead_in: f64,
}

impl Default for WalkthroughPath {
    fn default() -> Self {
        WalkthroughPath {
            points: Vec::new(),
            smooth: true,
            speed_in_per_s: DEFAULT_SPEED,
            eye_height: DEFAULT_EYE_HEIGHT,
            look_ahead_in: DEFAULT_LOOK_AHEAD,
        }
    }
}

impl WalkthroughPath {
    /// A path drawn in plan: each point becomes `[x, floor_elevation, -y]`.
    pub fn from_plan_polyline(points2d: &[Point], floor_elevation: f64, eye_height: f64) -> Self {
        WalkthroughPath {
            points: points2d
                .iter()
                .map(|p| [p.x, floor_elevation, -p.y])
                .collect(),
            eye_height,
            ..Default::default()
        }
    }

    /// Arc length of the (smoothed, if enabled) path, inches.
    pub fn length(&self) -> f64 {
        PathSampler::new(self).length()
    }

    /// Time to walk the whole path at `speed_in_per_s`, seconds.
    pub fn duration_s(&self) -> f64 {
        if self.speed_in_per_s > 0.0 {
            self.length() / self.speed_in_per_s
        } else {
            0.0
        }
    }

    /// Foot position (no eye height) after `t` seconds, clamped to the path.
    pub fn position_at(&self, t: f64) -> [f64; 3] {
        let sampler = PathSampler::new(self);
        sampler.at(t.max(0.0) * self.speed_in_per_s).0
    }
}

/// The path flattened to a polyline with cumulative arc length.
struct PathSampler {
    pts: Vec<V3>,
    cum: Vec<f64>,
}

impl PathSampler {
    fn new(path: &WalkthroughPath) -> Self {
        let mut ctrl: Vec<V3> = Vec::with_capacity(path.points.len());
        for &p in &path.points {
            if p.iter().all(|v| v.is_finite())
                && ctrl.last().is_none_or(|&q| length(sub(p, q)) > 1e-9)
            {
                ctrl.push(p);
            }
        }
        let pts = if path.smooth && ctrl.len() >= 3 {
            let n = ctrl.len();
            let mut out = Vec::with_capacity((n - 1) * SPAN_SAMPLES + 1);
            for i in 0..n - 1 {
                for k in 0..SPAN_SAMPLES {
                    let u = k as f64 / SPAN_SAMPLES as f64;
                    let (before, after) = spline_neighbours(&ctrl, i);
                    out.push(catmull_rom(before, ctrl[i], ctrl[i + 1], after, u));
                }
            }
            out.push(ctrl[n - 1]);
            out
        } else {
            ctrl
        };
        let mut cum = Vec::with_capacity(pts.len());
        let mut total = 0.0;
        for (i, p) in pts.iter().enumerate() {
            if i > 0 {
                total += length(sub(*p, pts[i - 1]));
            }
            cum.push(total);
        }
        PathSampler { pts, cum }
    }

    fn length(&self) -> f64 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    /// Unit direction of the last non-degenerate segment (or +X... -Z when none).
    fn end_tangent(&self) -> V3 {
        for w in self.pts.windows(2).rev() {
            let d = sub(w[1], w[0]);
            let l = length(d);
            if l > 1e-12 {
                return scale(d, 1.0 / l);
            }
        }
        [0.0, 0.0, -1.0]
    }

    /// Point and unit tangent at arc length `s`. Past the end the path is
    /// extended straight along its final direction; before the start it clamps.
    fn at(&self, s: f64) -> (V3, V3) {
        let Some(&first) = self.pts.first() else {
            return ([0.0; 3], [0.0, 0.0, -1.0]);
        };
        let total = self.length();
        if self.pts.len() < 2 || total <= 0.0 {
            return (first, [0.0, 0.0, -1.0]);
        }
        if s >= total {
            let tan = self.end_tangent();
            return (
                add(self.pts[self.pts.len() - 1], scale(tan, s - total)),
                tan,
            );
        }
        let s = s.max(0.0);
        // First index whose cumulative length exceeds `s`; always >= 1 here.
        let hi = self
            .cum
            .partition_point(|&c| c <= s)
            .clamp(1, self.pts.len() - 1);
        let lo = hi - 1;
        let seg = self.cum[hi] - self.cum[lo];
        let u = if seg > 0.0 {
            (s - self.cum[lo]) / seg
        } else {
            0.0
        };
        let d = sub(self.pts[hi], self.pts[lo]);
        let l = length(d);
        let tan = if l > 1e-12 {
            scale(d, 1.0 / l)
        } else {
            self.end_tangent()
        };
        (lerp(self.pts[lo], self.pts[hi], u), tan)
    }
}

/// One key frame of a key-framed camera: an eye pose at a time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyFrame {
    /// Time from the start, seconds.
    pub t_s: f64,
    /// Eye position, scene space inches (`[plan x, elevation, -plan y]`).
    pub position: [f64; 3],
    /// Heading, radians (see the module docs).
    pub yaw: f64,
    /// Elevation angle, radians; positive looks up.
    pub pitch: f64,
    /// Vertical field of view, degrees.
    pub fov: f64,
}

/// What drives the camera.
#[derive(Debug, Clone, PartialEq)]
pub enum WalkthroughSource {
    /// Follow a path at eye height.
    Path(WalkthroughPath),
    /// Interpolate between key frames (sorted by time).
    KeyFrames(Vec<KeyFrame>),
}

/// A recordable camera animation.
#[derive(Debug, Clone, PartialEq)]
pub struct Walkthrough {
    /// The path or key frames that place the camera.
    pub source: WalkthroughSource,
    /// Recording rate, frames per second.
    pub fps: f64,
    /// Total length, seconds.
    pub duration_s: f64,
    /// Vertical field of view for path walkthroughs, degrees.
    pub fov_deg: f64,
}

impl Walkthrough {
    /// Walk `path` start to end at its own speed, recorded at `fps`.
    pub fn along_path(path: WalkthroughPath, fps: f64) -> Self {
        let duration_s = path.duration_s();
        Walkthrough {
            source: WalkthroughSource::Path(path),
            fps,
            duration_s,
            fov_deg: DEFAULT_FOV,
        }
    }

    /// A key-framed walkthrough; frames are sorted by time and the duration
    /// is the last key frame's time.
    pub fn from_keyframes(mut keyframes: Vec<KeyFrame>, fps: f64) -> Self {
        keyframes.sort_by(|a, b| a.t_s.total_cmp(&b.t_s));
        let duration_s = keyframes.last().map_or(0.0, |k| k.t_s.max(0.0));
        Walkthrough {
            source: WalkthroughSource::KeyFrames(keyframes),
            fps,
            duration_s,
            fov_deg: DEFAULT_FOV,
        }
    }

    /// A walkthrough along a polyline drawn in plan (inches, Y up): smooth,
    /// 3 ft/s, 66" eye height by default (use `eye_height` to override), 24 fps.
    pub fn from_plan_polyline(points2d: &[Point], floor_elevation: f64, eye_height: f64) -> Self {
        Self::along_path(
            WalkthroughPath::from_plan_polyline(points2d, floor_elevation, eye_height),
            DEFAULT_FPS,
        )
    }

    /// Number of frames to record: `fps * duration`, rounded, at least one.
    pub fn frame_count(&self) -> usize {
        let n = (self.fps * self.duration_s).round();
        if n.is_finite() && n >= 1.0 {
            n as usize
        } else {
            1
        }
    }

    /// Time of frame `index` (0-based), seconds.
    pub fn frame_time(&self, index: usize) -> f64 {
        if self.fps > 0.0 {
            index as f64 / self.fps
        } else {
            0.0
        }
    }

    /// The first-person camera at time `t` seconds (clamped to the walkthrough).
    ///
    /// Paths are walked at constant speed by arc length and the camera looks
    /// toward the point `look_ahead_in` further along (extended straight past
    /// the end), which turns corners smoothly. Key frames use Catmull-Rom for
    /// position and shortest-arc interpolation for yaw; pitch and FOV are linear.
    pub fn camera_at(&self, t: f64) -> Camera {
        let t = if t.is_finite() {
            t.clamp(0.0, self.duration_s.max(0.0))
        } else {
            0.0
        };
        match &self.source {
            WalkthroughSource::Path(path) => self.path_camera(path, t),
            WalkthroughSource::KeyFrames(frames) => self.key_camera(frames, t),
        }
    }

    fn path_camera(&self, path: &WalkthroughPath, t: f64) -> Camera {
        let sampler = PathSampler::new(path);
        let s = t * path.speed_in_per_s.max(0.0);
        let (foot, tangent) = sampler.at(s);
        let look = path.look_ahead_in.max(0.0);
        let (ahead, _) = sampler.at(s + look);
        let mut dir = sub(ahead, foot);
        dir[1] = 0.0;
        if length(dir) < 1e-6 {
            dir = [tangent[0], 0.0, tangent[2]];
        }
        let yaw = if length(dir) < 1e-9 {
            0.0
        } else {
            heading(dir)
        };
        let eye = [foot[0], foot[1] + path.eye_height, foot[2]];
        self.camera(eye, yaw, 0.0, self.fov_deg, path.eye_height)
    }

    fn key_camera(&self, frames: &[KeyFrame], t: f64) -> Camera {
        let (first, last) = match (frames.first(), frames.last()) {
            (Some(a), Some(b)) => (a, b),
            _ => {
                return self.camera(
                    [0.0, DEFAULT_EYE_HEIGHT, 0.0],
                    0.0,
                    0.0,
                    self.fov_deg,
                    DEFAULT_EYE_HEIGHT,
                )
            }
        };
        let pose = if frames.len() == 1 || t <= first.t_s {
            *first
        } else if t >= last.t_s {
            *last
        } else {
            let hi = frames
                .partition_point(|k| k.t_s <= t)
                .clamp(1, frames.len() - 1);
            let i = hi - 1;
            let (a, b) = (&frames[i], &frames[hi]);
            let span = b.t_s - a.t_s;
            let u = if span > 0.0 { (t - a.t_s) / span } else { 0.0 };
            let positions: Vec<V3> = frames.iter().map(|k| k.position).collect();
            let (p0, p3) = spline_neighbours(&positions, i);
            KeyFrame {
                t_s: t,
                position: catmull_rom(p0, a.position, b.position, p3, u),
                yaw: lerp_angle(a.yaw, b.yaw, u),
                pitch: a.pitch + (b.pitch - a.pitch) * u,
                fov: a.fov + (b.fov - a.fov) * u,
            }
        };
        self.camera(
            pose.position,
            pose.yaw,
            pose.pitch,
            pose.fov,
            DEFAULT_EYE_HEIGHT,
        )
    }

    fn camera(&self, eye: V3, yaw: f64, pitch: f64, fov: f64, eye_height: f64) -> Camera {
        let position = [eye[0] as f32, eye[1] as f32, eye[2] as f32];
        let mut cam = Camera {
            mode: CameraMode::FullCamera,
            position,
            yaw: yaw as f32,
            pitch: pitch as f32,
            fov_deg: fov as f32,
            eye_height: eye_height as f32,
            ..Camera::default()
        };
        cam.target = crate::math::add(position, crate::math::scale(cam.forward(), 100.0));
        cam
    }
}

/// `n` evenly spaced cameras from start to end (both included) for a quick
/// Walkthrough Preview. `n = 1` gives the start pose; `n = 0` gives none.
pub fn preview_poses(wt: &Walkthrough, n: usize) -> Vec<Camera> {
    (0..n)
        .map(|i| {
            let u = if n > 1 {
                i as f64 / (n - 1) as f64
            } else {
                0.0
            };
            wt.camera_at(u * wt.duration_s)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist(a: [f32; 3], b: [f32; 3]) -> f64 {
        length(sub(
            [a[0] as f64, a[1] as f64, a[2] as f64],
            [b[0] as f64, b[1] as f64, b[2] as f64],
        ))
    }

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn polyline(smooth: bool) -> Walkthrough {
        let mut wt = Walkthrough::from_plan_polyline(
            &[pt(0.0, 0.0), pt(240.0, 0.0), pt(240.0, 240.0)],
            0.0,
            66.0,
        );
        if let WalkthroughSource::Path(p) = &mut wt.source {
            p.smooth = smooth;
            wt.duration_s = p.duration_s();
        }
        wt
    }

    #[test]
    fn plan_points_map_to_scene_space() {
        let wt = polyline(false);
        let WalkthroughSource::Path(p) = &wt.source else {
            panic!()
        };
        assert_eq!(p.points[2], [240.0, 0.0, -240.0]);
        assert_eq!(p.speed_in_per_s, 36.0);
        assert_eq!(p.look_ahead_in, 60.0);
        assert!((wt.duration_s - 480.0 / 36.0).abs() < 1e-9);
    }

    #[test]
    fn camera_at_zero_starts_at_first_point_facing_second() {
        let wt = polyline(false);
        let cam = wt.camera_at(0.0);
        assert_eq!(cam.mode, CameraMode::FullCamera);
        assert_eq!(cam.position, [0.0, 66.0, 0.0]);
        let f = cam.forward();
        // The second point is +X, so the view direction is +X.
        assert!(
            (f[0] - 1.0).abs() < 1e-5 && f[1].abs() < 1e-6 && f[2].abs() < 1e-5,
            "{f:?}"
        );
        // A collinear smooth path also starts toward the second point.
        let straight = Walkthrough::from_plan_polyline(
            &[pt(0.0, 0.0), pt(100.0, 0.0), pt(200.0, 0.0)],
            0.0,
            66.0,
        );
        let f = straight.camera_at(0.0).forward();
        assert!((f[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn constant_speed_by_arc_length() {
        let wt = polyline(false);
        // Corner is at 240" = 6.667 s: position there is the second vertex.
        let c = wt.camera_at(240.0 / 36.0);
        assert!(dist(c.position, [240.0, 66.0, 0.0]) < 1e-3);
        // Equal time steps on one straight leg cover equal distances.
        let a = wt.camera_at(1.0).position;
        let b = wt.camera_at(2.0).position;
        assert!((dist(a, b) - 36.0).abs() < 1e-3);
        // After the corner the camera walks the second leg at the same speed.
        let c = wt.camera_at(8.0).position;
        let d = wt.camera_at(9.0).position;
        assert!((dist(c, d) - 36.0).abs() < 1e-3);
        assert!((c[0] - 240.0).abs() < 1e-3);
        // Ends at the last point.
        let end = wt.camera_at(wt.duration_s + 5.0).position;
        assert!(dist(end, [240.0, 66.0, -240.0]) < 1e-3);
    }

    #[test]
    fn smooth_path_keeps_near_constant_speed() {
        let wt = polyline(true);
        let dt = 0.25;
        let n = (wt.duration_s / dt) as usize;
        let mut prev = wt.camera_at(0.0).position;
        for i in 1..n {
            let cur = wt.camera_at(i as f64 * dt).position;
            let d = dist(prev, cur);
            assert!((d - 36.0 * dt).abs() < 0.05 * 36.0 * dt, "step {i}: {d}");
            prev = cur;
        }
        // It passes through the corner control point's neighbourhood but cuts the corner.
        let mid = wt.camera_at(wt.duration_s * 0.5).position;
        assert!(mid[0] <= 240.0 + 1e-3 && mid[2] <= 1e-3);
    }

    #[test]
    fn yaw_turns_smoothly_through_the_corner() {
        let wt = polyline(false);
        let before = wt.camera_at(240.0 / 36.0 - 3.0).forward(); // 108" before, look-ahead short of corner
        let at = wt.camera_at(240.0 / 36.0).forward();
        let after = wt.camera_at(240.0 / 36.0 + 3.0).forward();
        assert!(before[0] > 0.99);
        assert!(after[2] < -0.99);
        // Right on the corner the look-ahead point is down the second leg.
        assert!(at[2] < -0.99);
        // Mid-turn the heading is between the two legs.
        let mid = wt.camera_at(240.0 / 36.0 - 0.8).forward();
        assert!(mid[0] > 0.1 && mid[2] < -0.1, "{mid:?}");
    }

    #[test]
    fn frame_count_is_fps_times_duration() {
        let mut wt = polyline(false);
        wt.fps = 30.0;
        wt.duration_s = 4.0;
        assert_eq!(wt.frame_count(), 120);
        wt.fps = 24.0;
        wt.duration_s = 2.5;
        assert_eq!(wt.frame_count(), 60);
        assert!((wt.frame_time(24) - 1.0).abs() < 1e-12);
    }

    fn kf(t: f64, x: f64, yaw: f64) -> KeyFrame {
        KeyFrame {
            t_s: t,
            position: [x, 66.0, 0.0],
            yaw,
            pitch: 0.1 * t,
            fov: 50.0 + 10.0 * t,
        }
    }

    #[test]
    fn keyframes_interpolate_midpoints() {
        let wt = Walkthrough::from_keyframes(
            vec![kf(2.0, 200.0, 1.0), kf(0.0, 0.0, 0.0), kf(1.0, 100.0, 0.5)],
            24.0,
        );
        assert!((wt.duration_s - 2.0).abs() < 1e-12);
        let cam = wt.camera_at(0.5);
        assert!((cam.position[0] - 50.0).abs() < 1e-3, "{:?}", cam.position);
        assert!((cam.yaw - 0.25).abs() < 1e-5);
        assert!((cam.pitch - 0.05).abs() < 1e-5);
        assert!((cam.fov_deg - 55.0).abs() < 1e-4);
        let cam = wt.camera_at(1.5);
        assert!((cam.position[0] - 150.0).abs() < 1e-3);
        // Exactly on a key frame.
        assert!((wt.camera_at(1.0).position[0] - 100.0).abs() < 1e-4);
    }

    #[test]
    fn keyframe_yaw_takes_the_shortest_arc() {
        let near_pi = 170f64.to_radians();
        let wt =
            Walkthrough::from_keyframes(vec![kf(0.0, 0.0, near_pi), kf(1.0, 10.0, -near_pi)], 24.0);
        let yaw = wt.camera_at(0.5).yaw as f64;
        // Midway across the +/-180 seam, not through 0.
        assert!((yaw.abs() - std::f64::consts::PI).abs() < 1e-4, "{yaw}");
    }

    #[test]
    fn preview_poses_span_the_walkthrough() {
        let wt = polyline(false);
        let poses = preview_poses(&wt, 5);
        assert_eq!(poses.len(), 5);
        assert!(dist(poses[0].position, [0.0, 66.0, 0.0]) < 1e-3);
        assert!(dist(poses[4].position, [240.0, 66.0, -240.0]) < 1e-3);
        assert!(preview_poses(&wt, 0).is_empty());
        assert_eq!(preview_poses(&wt, 1).len(), 1);
    }

    #[test]
    fn degenerate_inputs_do_not_panic() {
        let empty = Walkthrough::along_path(WalkthroughPath::default(), 24.0);
        assert_eq!(empty.frame_count(), 1);
        assert!(empty.camera_at(3.0).position.iter().all(|v| v.is_finite()));
        let one = Walkthrough::from_plan_polyline(&[pt(5.0, 5.0)], 0.0, 66.0);
        assert!(one.camera_at(1.0).forward().iter().all(|v| v.is_finite()));
        let dup = Walkthrough::from_plan_polyline(
            &[pt(0.0, 0.0), pt(0.0, 0.0), pt(50.0, 0.0)],
            0.0,
            66.0,
        );
        assert!(dup.duration_s > 0.0);
        let none = Walkthrough::from_keyframes(Vec::new(), 24.0);
        assert!(none.camera_at(1.0).position.iter().all(|v| v.is_finite()));
    }
}
