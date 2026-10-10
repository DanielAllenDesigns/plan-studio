//! Step commands for the 3D camera: the 3D menu's Move Camera, Move Camera
//! with Keyboard, Orbit Camera, Tilt Camera and View Direction entries
//! (C-2, C-34, C-38, C-40, C-41). Each is a small change to the viewport's
//! [`Camera`], made by [`apply`].
//!
//! * Orbit-style cameras (Perspective Full Overview, Doll House) circle their
//!   target: Orbit and Turn change the yaw, Tilt and Orbit up/down the pitch,
//!   Move slides the target along the ground (Raise and Lower move it up and
//!   down).
//! * The Full Camera turns on the spot and walks.
//! * The orthographic views (elevations, plan overhead) have a fixed
//!   direction, so these commands leave them alone ([`apply`] says so).

use plan_view3d::{Camera, CameraMode};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

/// Radians of one Orbit or Turn step (15 degrees).
pub const ORBIT_STEP: f32 = 15.0 * PI / 180.0;
/// Radians of one Tilt step (5 degrees).
pub const TILT_STEP: f32 = 5.0 * PI / 180.0;
/// Inches of one Move step.
pub const MOVE_STEP: f32 = 24.0;

/// A compass direction the view is taken from (View Direction).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Front,
    FrontRight,
    Right,
    BackRight,
    Back,
    BackLeft,
    Left,
    FrontLeft,
}

impl Direction {
    pub const ALL: [Direction; 8] = [
        Direction::Front,
        Direction::FrontRight,
        Direction::Right,
        Direction::BackRight,
        Direction::Back,
        Direction::BackLeft,
        Direction::Left,
        Direction::FrontLeft,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Direction::Front => "Front",
            Direction::FrontRight => "Front Right",
            Direction::Right => "Right",
            Direction::BackRight => "Back Right",
            Direction::Back => "Back",
            Direction::BackLeft => "Back Left",
            Direction::Left => "Left",
            Direction::FrontLeft => "Front Left",
        }
    }

    /// The camera yaw of a viewer standing on this side (0 is the front,
    /// +Z; the right side is +X).
    pub fn yaw(self) -> f32 {
        match self {
            Direction::Front => 0.0,
            Direction::FrontRight => FRAC_PI_4,
            Direction::Right => FRAC_PI_2,
            Direction::BackRight => 3.0 * FRAC_PI_4,
            Direction::Back => PI,
            Direction::BackLeft => -3.0 * FRAC_PI_4,
            Direction::Left => -FRAC_PI_2,
            Direction::FrontLeft => -FRAC_PI_4,
        }
    }
}

/// One step of the camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nudge {
    OrbitLeft,
    OrbitRight,
    OrbitUp,
    OrbitDown,
    TiltUp,
    TiltDown,
    TurnLeft,
    TurnRight,
    Forward,
    Back,
    Left,
    Right,
    Raise,
    Lower,
    Look(Direction),
}

/// Applies `n` to `cam`. Returns false (and leaves the camera alone) in the
/// orthographic views.
pub fn apply(cam: &mut Camera, n: Nudge) -> bool {
    if cam.mode.is_orthographic() {
        return false;
    }
    let full = cam.mode == CameraMode::FullCamera;
    let yaw = |cam: &mut Camera, d: f32| {
        if full {
            cam.turn(d, 0.0);
        } else {
            cam.orbit(d, 0.0);
        }
    };
    let pitch = |cam: &mut Camera, d: f32| {
        if full {
            cam.turn(0.0, d);
        } else {
            cam.orbit(0.0, d);
        }
    };
    match n {
        // An overview's eye circles its target (the yaw grows toward the
        // right); the Full Camera turns left as its yaw grows.
        Nudge::OrbitLeft | Nudge::TurnLeft => yaw(cam, if full { ORBIT_STEP } else { -ORBIT_STEP }),
        Nudge::OrbitRight | Nudge::TurnRight => {
            yaw(cam, if full { -ORBIT_STEP } else { ORBIT_STEP })
        }
        Nudge::OrbitUp => pitch(cam, ORBIT_STEP),
        Nudge::OrbitDown => pitch(cam, -ORBIT_STEP),
        Nudge::TiltUp => pitch(cam, TILT_STEP),
        Nudge::TiltDown => pitch(cam, -TILT_STEP),
        Nudge::Forward => cam.walk(MOVE_STEP, 0.0),
        Nudge::Back => cam.walk(-MOVE_STEP, 0.0),
        Nudge::Left => cam.walk(0.0, -MOVE_STEP),
        Nudge::Right => cam.walk(0.0, MOVE_STEP),
        Nudge::Raise | Nudge::Lower => {
            let d = if n == Nudge::Raise {
                MOVE_STEP
            } else {
                -MOVE_STEP
            };
            if full {
                cam.position[1] += d;
            } else {
                cam.target[1] += d;
            }
        }
        Nudge::Look(dir) => cam.yaw = dir.yaw(),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam(mode: CameraMode) -> Camera {
        let mut c = Camera::default();
        c.set_mode(mode);
        c
    }

    #[test]
    fn orbit_steps_turn_and_tilt_the_overview() {
        let mut c = cam(CameraMode::Orbit);
        let (y, p) = (c.yaw, c.pitch);
        assert!(apply(&mut c, Nudge::OrbitRight));
        assert!((c.yaw - (y + ORBIT_STEP)).abs() < 1e-6);
        assert!(apply(&mut c, Nudge::OrbitLeft));
        assert!((c.yaw - y).abs() < 1e-6);
        assert!(apply(&mut c, Nudge::TiltUp));
        assert!((c.pitch - (p + TILT_STEP)).abs() < 1e-6);
        assert!(apply(&mut c, Nudge::TiltDown));
        assert!((c.pitch - p).abs() < 1e-6);
        // The pitch stops short of straight up.
        for _ in 0..100 {
            apply(&mut c, Nudge::OrbitUp);
        }
        assert!(c.pitch <= 1.5 + 1e-6);
    }

    #[test]
    fn moving_slides_the_target_or_the_full_camera() {
        let mut c = cam(CameraMode::Orbit);
        let t = c.target;
        assert!(apply(&mut c, Nudge::Raise));
        assert!((c.target[1] - (t[1] + MOVE_STEP)).abs() < 1e-4);
        assert!(apply(&mut c, Nudge::Forward));
        assert_ne!(c.target[0], t[0]);
        let mut f = cam(CameraMode::FullCamera);
        let p = f.position;
        assert!(apply(&mut f, Nudge::Lower));
        assert!((f.position[1] - (p[1] - MOVE_STEP)).abs() < 1e-4);
        let before = f.position;
        assert!(apply(&mut f, Nudge::Forward));
        let moved = (0..3)
            .map(|i| (f.position[i] - before[i]).powi(2))
            .sum::<f32>();
        assert!((moved.sqrt() - MOVE_STEP).abs() < 1e-3);
    }

    #[test]
    fn the_full_camera_turns_on_the_spot() {
        let mut f = cam(CameraMode::FullCamera);
        let (pos, yaw) = (f.position, f.yaw);
        assert!(apply(&mut f, Nudge::TurnLeft));
        assert_eq!(f.position, pos);
        assert!((f.yaw - (yaw + ORBIT_STEP)).abs() < 1e-6);
        assert!(apply(&mut f, Nudge::TiltUp));
        assert!(f.pitch > 0.0);
    }

    #[test]
    fn view_direction_snaps_the_yaw_and_keeps_the_target() {
        let mut c = cam(CameraMode::Orbit);
        c.target = [10.0, 20.0, 30.0];
        let d = c.distance;
        for dir in Direction::ALL {
            assert!(apply(&mut c, Nudge::Look(dir)));
            assert_eq!(c.target, [10.0, 20.0, 30.0]);
            assert_eq!(c.distance, d);
            assert_eq!(c.yaw, dir.yaw());
        }
        // The right side puts the eye on +X.
        apply(&mut c, Nudge::Look(Direction::Right));
        assert!(c.eye()[0] > c.target[0]);
        apply(&mut c, Nudge::Look(Direction::Front));
        assert!(c.eye()[2] > c.target[2]);
    }

    #[test]
    fn the_orthographic_views_ignore_the_steps() {
        let mut c = cam(CameraMode::ElevationFront);
        let before = c.clone();
        for n in [
            Nudge::OrbitLeft,
            Nudge::Forward,
            Nudge::Look(Direction::Back),
        ] {
            assert!(!apply(&mut c, n));
        }
        assert_eq!(c, before);
    }
}
