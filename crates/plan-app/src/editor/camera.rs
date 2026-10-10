//! Maps between world inches (Y up) and screen pixels (Y down).

use eframe::egui::{Pos2, Rect, Vec2};
use plan_core::geometry::Point;

/// The plan camera. `rect` is the canvas rectangle on screen; `main.rs`
/// refreshes it every frame.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// World point shown at the viewport center.
    pub center: Point,
    pub px_per_in: f64,
    pub rect: Rect,
    /// Rotate Plan View: how far the view is turned counter-clockwise, in
    /// radians. Only the view turns, the plan does not.
    pub rotation: f64,
}

impl Camera {
    pub fn default_view() -> Self {
        Self {
            center: Point::new(240.0, 150.0),
            px_per_in: 2.0,
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
            rotation: 0.0,
        }
    }

    /// `v` turned by `by` radians counter-clockwise (plan axes, Y up).
    fn turn(v: Point, by: f64) -> Point {
        let (s, c) = by.sin_cos();
        Point::new(v.x * c - v.y * s, v.x * s + v.y * c)
    }

    pub fn world_to_screen(&self, p: Point) -> Pos2 {
        let c = self.rect.center();
        let d = Self::turn(
            Point::new(
                (p.x - self.center.x) * self.px_per_in,
                (p.y - self.center.y) * self.px_per_in,
            ),
            self.rotation,
        );
        Pos2::new(c.x + d.x as f32, c.y - d.y as f32)
    }

    pub fn screen_to_world(&self, s: Pos2) -> Point {
        let c = self.rect.center();
        let d = Self::turn(
            Point::new((s.x - c.x) as f64, -(s.y - c.y) as f64),
            -self.rotation,
        );
        Point::new(
            self.center.x + d.x / self.px_per_in,
            self.center.y + d.y / self.px_per_in,
        )
    }

    /// Zoom by `factor` keeping the world point under screen position `at` fixed.
    pub fn zoom_about(&mut self, at: Pos2, factor: f64) {
        let anchor = self.screen_to_world(at);
        self.px_per_in = (self.px_per_in * factor).clamp(0.05, 50.0);
        let c = self.rect.center();
        let d = Self::turn(
            Point::new((at.x - c.x) as f64, -(at.y - c.y) as f64),
            -self.rotation,
        );
        self.center = Point::new(
            anchor.x - d.x / self.px_per_in,
            anchor.y - d.y / self.px_per_in,
        );
    }

    /// The four corners of the canvas in plan coordinates.
    pub fn world_corners(&self) -> [Point; 4] {
        let r = self.rect;
        [
            self.screen_to_world(r.left_top()),
            self.screen_to_world(r.right_top()),
            self.screen_to_world(r.right_bottom()),
            self.screen_to_world(r.left_bottom()),
        ]
    }

    pub fn pan_by_pixels(&mut self, d: Vec2) {
        let w = Self::turn(Point::new(d.x as f64, -d.y as f64), -self.rotation);
        self.center.x -= w.x / self.px_per_in;
        self.center.y -= w.y / self.px_per_in;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rotated_view_maps_points_both_ways_and_pans_with_the_pointer() {
        let mut cam = Camera::default_view();
        cam.rotation = std::f64::consts::FRAC_PI_2;
        let p = Point::new(300.0, 200.0);
        let back = cam.screen_to_world(cam.world_to_screen(p));
        assert!((back.x - p.x).abs() < 1e-3 && (back.y - p.y).abs() < 1e-3);
        // The point east of the center is now drawn north of it.
        let c = cam.rect.center();
        let s = cam.world_to_screen(Point::new(cam.center.x + 10.0, cam.center.y));
        assert!((s.x - c.x).abs() < 1e-3 && s.y < c.y);
        // Zooming keeps the point under the pointer; panning drags with it.
        let at = Pos2::new(c.x + 80.0, c.y - 30.0);
        let anchor = cam.screen_to_world(at);
        cam.zoom_about(at, 2.0);
        let again = cam.screen_to_world(at);
        assert!((anchor.x - again.x).abs() < 1e-6 && (anchor.y - again.y).abs() < 1e-6);
        let before = cam.world_to_screen(p);
        cam.pan_by_pixels(Vec2::new(15.0, -9.0));
        let after = cam.world_to_screen(p);
        assert!(
            (after.x - before.x - 15.0).abs() < 1e-3 && (after.y - before.y + 9.0).abs() < 1e-3
        );
    }
}
