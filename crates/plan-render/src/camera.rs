//! Thin-lens camera.

use crate::rng::Rng;
use crate::settings::Projection;
use crate::vec3::V3;
use plan_core::Point;

/// A camera in scene space (X right, Y up, Z toward the viewer; inches).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Eye position.
    pub eye: [f32; 3],
    /// Point looked at.
    pub target: [f32; 3],
    /// Approximate up direction.
    pub up: [f32; 3],
    /// Vertical field of view, degrees.
    pub fov_deg: f32,
    /// Lens diameter, inches; `0.0` is a pinhole (everything in focus).
    pub aperture: f32,
    /// Distance to the plane of perfect focus, inches (`<= 0` focuses on the target).
    pub focus_dist: f32,
}

impl Camera {
    /// A level camera placed from the floor plan.
    ///
    /// `position` is the plan point (inches; scene `Z = -plan y`), `direction_deg`
    /// the view heading measured counter-clockwise from plan +X, `eye_height`
    /// the eye elevation in inches and `fov_deg` the vertical field of view.
    pub fn from_plan(position: Point, direction_deg: f64, eye_height: f64, fov_deg: f64) -> Camera {
        let (sin, cos) = direction_deg.to_radians().sin_cos();
        let eye = [position.x as f32, eye_height as f32, -position.y as f32];
        let reach = 100.0_f32;
        Camera {
            eye,
            target: [
                eye[0] + cos as f32 * reach,
                eye[1],
                eye[2] - sin as f32 * reach,
            ],
            up: [0.0, 1.0, 0.0],
            fov_deg: fov_deg as f32,
            aperture: 0.0,
            focus_dist: reach,
        }
    }
}

/// Precomputed ray generator for one image size.
#[derive(Clone, Debug)]
pub(crate) struct Lens {
    origin: V3,
    forward: V3,
    right: V3,
    up: V3,
    half_h: f32,
    aspect: f32,
    lens_radius: f32,
    focus_dist: f32,
    width: f32,
    height: f32,
    projection: Projection,
}

/// The unit direction of an equirectangular panorama pixel at `(u, v)`
/// (both 0..1, `v` from the top) for a camera with the given basis: the
/// image centre looks along `forward`, `u` turns to the right, `v = 0` is
/// straight up and `v = 1` straight down.
pub(crate) fn equirect_dir(forward: V3, right: V3, up: V3, u: f32, v: f32) -> V3 {
    let lon = (u - 0.5) * std::f32::consts::TAU;
    let lat = (0.5 - v) * std::f32::consts::PI;
    let (sin_lat, cos_lat) = lat.sin_cos();
    let (sin_lon, cos_lon) = lon.sin_cos();
    ((forward * cos_lon + right * sin_lon) * cos_lat + up * sin_lat).normalized()
}

impl Lens {
    pub fn new(cam: &Camera, width: u32, height: u32) -> Lens {
        Lens::with_projection(cam, width, height, Projection::Perspective)
    }

    pub fn with_projection(cam: &Camera, width: u32, height: u32, projection: Projection) -> Lens {
        let origin = V3::from_array(cam.eye);
        let to_target = V3::from_array(cam.target) - origin;
        let forward = if to_target.length_sq() > 0.0 {
            to_target.normalized()
        } else {
            V3::new(0.0, 0.0, -1.0)
        };
        let mut up_hint = V3::from_array(cam.up);
        if up_hint.cross(forward).length_sq() < 1e-10 {
            // Looking straight along `up`: pick any perpendicular hint.
            up_hint = if forward.y.abs() > 0.9 {
                V3::new(0.0, 0.0, -1.0)
            } else {
                V3::new(0.0, 1.0, 0.0)
            };
        }
        let right = forward.cross(up_hint).normalized();
        let up = right.cross(forward);
        let focus = if cam.focus_dist > 0.0 {
            cam.focus_dist
        } else {
            to_target.length().max(1.0)
        };
        Lens {
            origin,
            forward,
            right,
            up,
            half_h: (cam.fov_deg.clamp(1.0, 179.0).to_radians() * 0.5).tan(),
            aspect: width as f32 / height as f32,
            lens_radius: cam.aperture.max(0.0) * 0.5,
            focus_dist: focus,
            width: width as f32,
            height: height as f32,
            projection,
        }
    }

    /// The pinhole ray through `(px, py)`, ignoring the lens aperture.
    pub fn centre_ray(&self, px: f32, py: f32) -> (V3, V3) {
        if self.projection == Projection::Equirectangular {
            let dir = equirect_dir(
                self.forward,
                self.right,
                self.up,
                px / self.width,
                py / self.height,
            );
            return (self.origin, dir);
        }
        let x = (2.0 * px / self.width - 1.0) * self.aspect * self.half_h;
        let y = (1.0 - 2.0 * py / self.height) * self.half_h;
        (
            self.origin,
            (self.forward + self.right * x + self.up * y).normalized(),
        )
    }

    /// Ray through image position `(px, py)` (pixels, origin top-left).
    pub fn ray(&self, px: f32, py: f32, rng: &mut Rng) -> (V3, V3) {
        if self.projection == Projection::Equirectangular {
            // A panorama is a pinhole: no depth of field.
            return self.centre_ray(px, py);
        }
        let x = (2.0 * px / self.width - 1.0) * self.aspect * self.half_h;
        let y = (1.0 - 2.0 * py / self.height) * self.half_h;
        let dir = self.forward + self.right * x + self.up * y;
        if self.lens_radius <= 0.0 {
            return (self.origin, dir.normalized());
        }
        let focus_point = self.origin + dir * self.focus_dist;
        let r = rng.next_f32().sqrt() * self.lens_radius;
        let (sin, cos) = (std::f32::consts::TAU * rng.next_f32()).sin_cos();
        let origin = self.origin + self.right * (r * cos) + self.up * (r * sin);
        (origin, (focus_point - origin).normalized())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_heading_maps_to_scene_axes() {
        let east = Camera::from_plan(Point::new(10.0, 20.0), 0.0, 60.0, 60.0);
        assert_eq!(east.eye, [10.0, 60.0, -20.0]);
        assert!(east.target[0] > east.eye[0] && (east.target[2] - east.eye[2]).abs() < 1e-3);
        let north = Camera::from_plan(Point::ZERO, 90.0, 60.0, 60.0);
        assert!(north.target[2] < north.eye[2]);
    }

    #[test]
    fn centre_ray_points_forward() {
        let cam = Camera::from_plan(Point::ZERO, 0.0, 0.0, 60.0);
        let lens = Lens::new(&cam, 40, 30);
        let (_, d) = lens.ray(20.0, 15.0, &mut Rng::new(1));
        assert!(d.x > 0.999);
    }
}
