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
}

impl Camera {
    pub fn default_view() -> Self {
        Self {
            center: Point::new(240.0, 150.0),
            px_per_in: 2.0,
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
        }
    }

    pub fn world_to_screen(&self, p: Point) -> Pos2 {
        let c = self.rect.center();
        Pos2::new(
            c.x + ((p.x - self.center.x) * self.px_per_in) as f32,
            c.y - ((p.y - self.center.y) * self.px_per_in) as f32,
        )
    }

    pub fn screen_to_world(&self, s: Pos2) -> Point {
        let c = self.rect.center();
        Point::new(
            self.center.x + (s.x - c.x) as f64 / self.px_per_in,
            self.center.y - (s.y - c.y) as f64 / self.px_per_in,
        )
    }

    /// Zoom by `factor` keeping the world point under screen position `at` fixed.
    pub fn zoom_about(&mut self, at: Pos2, factor: f64) {
        let anchor = self.screen_to_world(at);
        self.px_per_in = (self.px_per_in * factor).clamp(0.05, 50.0);
        let c = self.rect.center();
        self.center = Point::new(
            anchor.x - (at.x - c.x) as f64 / self.px_per_in,
            anchor.y + (at.y - c.y) as f64 / self.px_per_in,
        );
    }

    pub fn pan_by_pixels(&mut self, d: Vec2) {
        self.center.x -= d.x as f64 / self.px_per_in;
        self.center.y += d.y as f64 / self.px_per_in;
    }
}
