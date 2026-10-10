//! Undo Zoom in the 3D view (C-42): a stack of the camera before each
//! gesture, so Undo Zoom steps back through zooms, pans and moves like the
//! plan's own zoom history.
//!
//! A gesture is a run of frames in which the camera changes; it ends on the
//! first frame that leaves the camera as it was. The camera from before the
//! run is pushed. Changes the program makes itself (opening another view,
//! framing the model, undoing a zoom) call [`ZoomHistory::reset_to`], which
//! forgets the run in progress without recording it.

use plan_view3d::Camera;

/// Most steps kept.
pub const DEPTH: usize = 50;

#[derive(Debug, Default)]
pub struct ZoomHistory {
    /// The camera before the current gesture began (the last quiet one).
    quiet: Option<Camera>,
    /// The camera at the end of the previous frame.
    last: Option<Camera>,
    stack: Vec<Camera>,
}

impl ZoomHistory {
    /// Watches the camera once per frame.
    pub fn observe(&mut self, now: &Camera) {
        match (&self.last, &self.quiet) {
            // A frame that leaves the camera as it was ends a gesture: the
            // camera from before it is a step. While the camera still changes
            // `quiet` stays the camera from before the gesture.
            (Some(last), Some(quiet)) if last == now && quiet != now => {
                if self.stack.last() != Some(quiet) {
                    self.stack.push(quiet.clone());
                    if self.stack.len() > DEPTH {
                        self.stack.remove(0);
                    }
                }
                self.quiet = Some(now.clone());
            }
            (Some(_), Some(_)) => {}
            _ => self.quiet = Some(now.clone()),
        }
        self.last = Some(now.clone());
    }

    /// Forgets the gesture in progress and takes `cam` as the starting point
    /// (the program moved the camera, not the user).
    pub fn reset_to(&mut self, cam: &Camera) {
        self.quiet = Some(cam.clone());
        self.last = Some(cam.clone());
    }

    /// Drops every step (a different view was opened).
    pub fn clear(&mut self) {
        *self = ZoomHistory::default();
    }

    /// The camera before the last gesture, if there is one.
    pub fn undo(&mut self) -> Option<Camera> {
        let cam = self.stack.pop()?;
        self.reset_to(&cam);
        Some(cam)
    }

    /// How many steps Undo Zoom can go back.
    #[cfg(test)]
    pub fn steps(&self) -> usize {
        self.stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_view3d::CameraMode;

    fn at(distance: f32) -> Camera {
        Camera {
            distance,
            ..Camera::default()
        }
    }

    /// Feeds a run of frames.
    fn feed(h: &mut ZoomHistory, frames: &[f32]) {
        for d in frames {
            h.observe(&at(*d));
        }
    }

    #[test]
    fn a_gesture_is_one_step_back_to_where_it_began() {
        let mut h = ZoomHistory::default();
        // Quiet, then a scroll that changes the camera over four frames.
        feed(
            &mut h,
            &[1200.0, 1200.0, 1100.0, 1000.0, 900.0, 800.0, 800.0],
        );
        assert_eq!(h.steps(), 1);
        assert_eq!(h.undo().unwrap().distance, 1200.0);
        assert_eq!(h.steps(), 0);
        assert!(h.undo().is_none());
    }

    #[test]
    fn two_gestures_undo_in_reverse_order() {
        let mut h = ZoomHistory::default();
        feed(
            &mut h,
            &[1200.0, 1200.0, 1000.0, 1000.0, 1000.0, 700.0, 700.0],
        );
        assert_eq!(h.steps(), 2);
        assert_eq!(h.undo().unwrap().distance, 1000.0);
        assert_eq!(h.undo().unwrap().distance, 1200.0);
    }

    #[test]
    fn a_still_camera_records_nothing() {
        let mut h = ZoomHistory::default();
        feed(&mut h, &[900.0; 10]);
        assert_eq!(h.steps(), 0);
    }

    #[test]
    fn undoing_does_not_record_itself_and_the_next_gesture_starts_there() {
        let mut h = ZoomHistory::default();
        feed(&mut h, &[1200.0, 1200.0, 800.0, 800.0]);
        let back = h.undo().unwrap();
        // The panel sets the camera to `back`; the next frames see it as quiet.
        h.observe(&back);
        h.observe(&back);
        assert_eq!(h.steps(), 0, "undo is not a gesture");
        feed(&mut h, &[1200.0, 1100.0, 1100.0]);
        assert_eq!(h.steps(), 1);
        assert_eq!(h.undo().unwrap().distance, 1200.0);
    }

    #[test]
    fn a_change_by_the_program_is_not_a_step() {
        let mut h = ZoomHistory::default();
        feed(&mut h, &[1200.0, 1200.0]);
        let mut other = at(300.0);
        other.set_mode(CameraMode::DollHouse);
        h.reset_to(&other);
        h.observe(&other);
        h.observe(&other);
        assert_eq!(h.steps(), 0);
        h.clear();
        assert_eq!(h.steps(), 0);
    }

    #[test]
    fn the_stack_is_bounded() {
        let mut h = ZoomHistory::default();
        let mut d = 1000.0;
        h.observe(&at(d));
        h.observe(&at(d));
        for _ in 0..DEPTH + 10 {
            d += 10.0;
            h.observe(&at(d));
            h.observe(&at(d));
        }
        assert_eq!(h.steps(), DEPTH);
    }
}
