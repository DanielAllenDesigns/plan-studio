//! Plan underlays (L-43, L-46): a raster picture (a scanned survey, an
//! existing-house plan, a PDF page saved as PNG) placed under the plan for
//! tracing. The picture is stored by file path plus pixel size; the placement
//! (lower-left corner, inches per pixel, rotation) is what the two-point
//! calibration edits.
//!
//! Lengths are inches, angles radians counter-clockwise, plan Y up. Pixel
//! coordinates run from the picture's top-left corner (x right, y down), like
//! the image file.

use crate::geometry::Point;
use crate::layers::{Layer, LayerSet};
use crate::model::{Floor, Id, Project};
use serde::{Deserialize, Serialize};

/// The layer underlays are drawn on (hidden and locked with it).
pub const UNDERLAY_LAYER: &str = "Underlays";
/// Longest side of a freshly placed, not yet calibrated underlay, inches.
pub const DEFAULT_LONG_SIDE: f64 = 480.0;
/// Smallest real distance a calibration accepts, inches.
pub const MIN_CALIBRATION_DISTANCE: f64 = 0.01;

/// What the underlay file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UnderlayKind {
    /// A PNG or JPEG file.
    #[default]
    Image,
    /// One page of a PDF (kept for files written by a build that rasterizes
    /// PDF; this build imports pages saved as images).
    Pdf,
}

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

/// One underlay picture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Underlay {
    pub id: Id,
    pub name: String,
    /// Absolute path of the picture file.
    pub path: String,
    #[serde(default)]
    pub kind: UnderlayKind,
    /// Page of a PDF (1-based); 1 for a picture.
    #[serde(default)]
    pub page: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    /// Plan position of the picture's lower-left corner.
    pub origin: Point,
    /// Plan inches per picture pixel.
    pub inches_per_pixel: f64,
    /// Counter-clockwise rotation about `origin`, radians.
    #[serde(default)]
    pub rotation: f64,
    /// 0 (invisible) to 1 (opaque).
    #[serde(default = "one")]
    pub opacity: f32,
    /// A locked underlay cannot be picked or moved.
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "yes")]
    pub visible: bool,
    /// Has a real distance been entered (the scale is not the placeholder)?
    #[serde(default)]
    pub calibrated: bool,
}

impl Underlay {
    /// A picture of `pixel_width` x `pixel_height` centred on `center`, scaled
    /// so its long side is [`DEFAULT_LONG_SIDE`] until it is calibrated.
    pub fn new(
        name: impl Into<String>,
        path: impl Into<String>,
        pixel_width: u32,
        pixel_height: u32,
        center: Point,
    ) -> Self {
        let long = f64::from(pixel_width.max(pixel_height).max(1));
        let ipp = DEFAULT_LONG_SIDE / long;
        let mut u = Self {
            id: 0,
            name: name.into(),
            path: path.into(),
            kind: UnderlayKind::Image,
            page: 1,
            pixel_width,
            pixel_height,
            origin: Point::ZERO,
            inches_per_pixel: ipp,
            rotation: 0.0,
            opacity: 0.6,
            locked: false,
            visible: true,
            calibrated: false,
        };
        let (w, h) = u.size();
        u.origin = Point::new(center.x - w * 0.5, center.y - h * 0.5);
        u
    }

    /// Width and height in plan inches (before rotation).
    pub fn size(&self) -> (f64, f64) {
        (
            f64::from(self.pixel_width) * self.inches_per_pixel,
            f64::from(self.pixel_height) * self.inches_per_pixel,
        )
    }

    fn rot(&self, v: Point) -> Point {
        let (s, c) = self.rotation.sin_cos();
        Point::new(v.x * c - v.y * s, v.x * s + v.y * c)
    }

    fn unrot(&self, v: Point) -> Point {
        let (s, c) = self.rotation.sin_cos();
        Point::new(v.x * c + v.y * s, -v.x * s + v.y * c)
    }

    /// The plan point of a picture pixel (`px` right, `py` down from the
    /// top-left corner; fractions allowed).
    pub fn pixel_to_plan(&self, px: f64, py: f64) -> Point {
        let h = f64::from(self.pixel_height);
        let local = Point::new(px * self.inches_per_pixel, (h - py) * self.inches_per_pixel);
        self.origin.add(self.rot(local))
    }

    /// The picture pixel under a plan point.
    pub fn plan_to_pixel(&self, p: Point) -> (f64, f64) {
        let local = self.unrot(p.sub(self.origin));
        let k = self.inches_per_pixel.max(1e-12);
        (local.x / k, f64::from(self.pixel_height) - local.y / k)
    }

    /// Plan corners in the order lower-left, lower-right, upper-right,
    /// upper-left (the picture's bottom edge first).
    pub fn corners(&self) -> [Point; 4] {
        let (w, h) = self.size();
        [
            self.origin,
            self.origin.add(self.rot(Point::new(w, 0.0))),
            self.origin.add(self.rot(Point::new(w, h))),
            self.origin.add(self.rot(Point::new(0.0, h))),
        ]
    }

    /// Does the picture cover the plan point?
    pub fn contains(&self, p: Point) -> bool {
        let (w, h) = self.size();
        let l = self.unrot(p.sub(self.origin));
        (0.0..=w).contains(&l.x) && (0.0..=h).contains(&l.y)
    }

    /// Two-point calibration: `a` and `b` are plan points the user clicked on
    /// the picture (two ends of a dimension printed on the drawing) and
    /// `real` is the distance between them in the real building, inches. The
    /// picture is scaled about `a`, so `a` stays where it is. False when the
    /// clicks coincide or the distance is not positive.
    pub fn calibrate(&mut self, a: Point, b: Point, real: f64) -> bool {
        let drawn = a.dist(b);
        if drawn < 1e-9 || !real.is_finite() || real < MIN_CALIBRATION_DISTANCE {
            return false;
        }
        let k = real / drawn;
        self.inches_per_pixel *= k;
        self.origin = a.add(self.origin.sub(a).scale(k));
        self.calibrated = true;
        true
    }

    /// Opacity clamped to 0..=1.
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Moves the picture by `delta`.
    pub fn translate(&mut self, delta: Point) {
        self.origin = self.origin.add(delta);
    }
}

/// Adds the [`UNDERLAY_LAYER`] to `layers` when it is missing (a muted blue,
/// thin line); true when it was added.
pub fn ensure_layer(layers: &mut LayerSet) -> bool {
    layers.add(Layer::new(UNDERLAY_LAYER, [110, 130, 160], 13))
}

impl Floor {
    /// The underlay with `id`.
    pub fn underlay(&self, id: Id) -> Option<&Underlay> {
        self.underlays.iter().find(|u| u.id == id)
    }

    pub fn underlay_mut(&mut self, id: Id) -> Option<&mut Underlay> {
        self.underlays.iter_mut().find(|u| u.id == id)
    }
}

impl Project {
    /// Adds `underlay` to `floor` under a fresh id (and makes sure the
    /// Underlays layer exists); returns the id.
    pub fn add_underlay(&mut self, floor: usize, mut underlay: Underlay) -> Id {
        let id = self.alloc_id();
        underlay.id = id;
        ensure_layer(&mut self.layers);
        self.floors[floor].underlays.push(underlay);
        id
    }

    /// Removes the underlay; false when there is none with that id.
    pub fn remove_underlay(&mut self, floor: usize, id: Id) -> bool {
        let list = &mut self.floors[floor].underlays;
        let n = list.len();
        list.retain(|u| u.id != id);
        list.len() != n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Underlay {
        Underlay::new("survey", "/tmp/survey.png", 1000, 500, Point::new(0.0, 0.0))
    }

    #[test]
    fn a_new_underlay_is_centred_and_scaled_to_the_default_long_side() {
        let u = sample();
        let (w, h) = u.size();
        assert!((w - DEFAULT_LONG_SIDE).abs() < 1e-9);
        assert!((h - DEFAULT_LONG_SIDE / 2.0).abs() < 1e-9);
        let c = u.corners();
        assert!((c[0].x + w / 2.0).abs() < 1e-9 && (c[0].y + h / 2.0).abs() < 1e-9);
        assert!(u.contains(Point::new(0.0, 0.0)));
        assert!(!u.contains(Point::new(w, 0.0)));
        assert!(!u.calibrated);
    }

    #[test]
    fn pixels_and_plan_points_round_trip_with_rotation() {
        let mut u = sample();
        u.rotation = 0.7;
        u.origin = Point::new(40.0, -12.0);
        for (px, py) in [(0.0, 0.0), (1000.0, 500.0), (250.5, 100.25)] {
            let p = u.pixel_to_plan(px, py);
            let (qx, qy) = u.plan_to_pixel(p);
            assert!(
                (qx - px).abs() < 1e-6 && (qy - py).abs() < 1e-6,
                "{px} {py}"
            );
        }
        // The top-left pixel is the upper-left corner.
        assert!(u.pixel_to_plan(0.0, 0.0).dist(u.corners()[3]) < 1e-9);
    }

    #[test]
    fn two_point_calibration_sets_the_scale_and_keeps_the_first_point() {
        let mut u = sample();
        // The user clicks two ends of a dimension that reads 20'-0" (240")
        // but are 120" apart at the placeholder scale.
        let a = u.pixel_to_plan(100.0, 250.0);
        let b = u.pixel_to_plan(100.0 + 250.0, 250.0);
        let drawn = a.dist(b);
        assert!((drawn - 125.0 * (DEFAULT_LONG_SIDE / 1000.0) * 2.0).abs() < 1e-6);
        assert!(u.calibrate(a, b, 240.0));
        assert!(u.calibrated);
        // The same pixels are now 240" apart in the plan, and `a` did not move.
        let a2 = u.pixel_to_plan(100.0, 250.0);
        let b2 = u.pixel_to_plan(350.0, 250.0);
        assert!(a2.dist(a) < 1e-9);
        assert!((a2.dist(b2) - 240.0).abs() < 1e-9);
        assert!((u.inches_per_pixel - 240.0 / 250.0).abs() < 1e-9);
    }

    #[test]
    fn calibration_refuses_nonsense() {
        let mut u = sample();
        let before = u.clone();
        assert!(!u.calibrate(Point::ZERO, Point::ZERO, 10.0));
        assert!(!u.calibrate(Point::ZERO, Point::new(5.0, 0.0), 0.0));
        assert!(!u.calibrate(Point::ZERO, Point::new(5.0, 0.0), f64::NAN));
        assert_eq!(u, before);
    }

    #[test]
    fn project_slot_adds_a_layer_and_survives_json() {
        let mut p = Project::new("t");
        assert!(p.layers.get(UNDERLAY_LAYER).is_none());
        let id = p.add_underlay(0, sample());
        assert!(p.layers.get(UNDERLAY_LAYER).is_some());
        p.floors[0].underlay_mut(id).unwrap().locked = true;
        p.floors[0].underlay_mut(id).unwrap().set_opacity(2.0);
        let json = p.to_json().unwrap();
        let back = Project::from_json(&json).unwrap();
        let u = back.floors[0].underlay(id).unwrap();
        assert!(u.locked);
        assert_eq!(u.opacity, 1.0);
        assert_eq!(u.name, "survey");
        assert!(p.remove_underlay(0, id));
        assert!(!p.remove_underlay(0, id));
    }

    #[test]
    fn older_files_without_underlays_load() {
        let mut v = serde_json::to_value(Project::new("t")).unwrap();
        v["floors"][0].as_object_mut().unwrap().remove("underlays");
        let p: Project = serde_json::from_value(v).unwrap();
        assert!(p.floors[0].underlays.is_empty());
    }
}
