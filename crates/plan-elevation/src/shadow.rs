//! Cast shadows for the Technical Illustration look.
//!
//! A second orthographic depth render is made looking along the sun's rays,
//! at the same raster size as the view. Every visible pixel is lifted back to
//! its view-space point, moved into light space and compared with the shadow
//! map: a point with something nearer the sun is in shadow. This also marks
//! faces turned away from the sun (self-shadow), as Chief does.
//!
//! Simplifications: the shadow map is a plain depth buffer with a constant
//! bias (3 px + 0.1"), so surfaces at grazing incidence may speckle; casters
//! are the kept (section-clipped) triangles, front and back faces alike; the
//! ground is an implicit plane at the lowest scene point and is visible only
//! in the Top view (it is edge-on in elevations), where background pixels
//! inside the drawing extent are tested against it.

use crate::hlr::{DepthBuffer, Tri, V3};
use crate::projection::{Projection, ViewDir};
use plan_3d::Material;
use serde::{Deserialize, Serialize};

/// Shadow bias in light-map pixels.
const BIAS_PX: f64 = 3.0;
/// Shadow bias, inches.
const BIAS_IN: f64 = 0.1;

/// Where the sun is: compass bearing and height, like `plan_materials::SunSettings`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SunDir {
    /// Bearing of the sun, degrees clockwise from plan north (+plan y); east is +x.
    pub azimuth_deg: f64,
    /// Height above the horizon, degrees.
    pub altitude_deg: f64,
}

impl SunDir {
    /// Direction the light travels, in scene space (X east, Y up, Z = -plan y).
    pub fn light_direction(&self) -> [f64; 3] {
        let (az, alt) = (
            self.azimuth_deg.to_radians(),
            self.altitude_deg.to_radians(),
        );
        [-az.sin() * alt.cos(), -alt.sin(), az.cos() * alt.cos()]
    }
}

impl From<&plan_materials::SunSettings> for SunDir {
    fn from(s: &plan_materials::SunSettings) -> SunDir {
        SunDir {
            azimuth_deg: s.azimuth_deg,
            altitude_deg: s.altitude_deg,
        }
    }
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A pixel in shadow and the material it falls on.
pub(crate) struct Shadowed {
    pub index: usize,
    pub material: Option<Material>,
}

/// Shadowed pixels of the view buffer.
///
/// `surface(i)` says what lies under pixel `i`; background pixels are only
/// considered (against the ground plane) in the Top view.
pub(crate) fn shadowed_pixels(
    tris: &[Tri],
    buf: &DepthBuffer,
    proj: &Projection,
    sun: SunDir,
    raster_px: usize,
    surface: &dyn Fn(usize) -> Surface,
) -> Vec<Shadowed> {
    if tris.is_empty() {
        return Vec::new();
    }
    let (dir_uv, dir_d) = proj.project(sun.light_direction());
    let light = normalize([dir_uv.x, dir_uv.y, dir_d]);
    let helper = if light[1].abs() > 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let e1 = normalize(cross(helper, light));
    let e2 = cross(light, e1);
    let to_light = |p: V3| -> V3 { [dot(p, e1), dot(p, e2), -dot(p, light)] };

    let lit_tris: Vec<[V3; 3]> = tris.iter().map(|t| t.p.map(to_light)).collect();
    let (mut lo, mut hi) = (
        plan_core::Point::new(f64::INFINITY, f64::INFINITY),
        plan_core::Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
    );
    for p in lit_tris.iter().flatten() {
        lo = plan_core::Point::new(lo.x.min(p[0]), lo.y.min(p[1]));
        hi = plan_core::Point::new(hi.x.max(p[0]), hi.y.max(p[1]));
    }
    let mut map = DepthBuffer::new((lo, hi), raster_px);
    for t in &lit_tris {
        map.rasterize(t, 0);
    }
    let bias = BIAS_PX * map.pixel_size() + BIAS_IN;

    let ground_depth = proj.depth_range().0;
    let top = proj.dir() == ViewDir::Top;
    let mut out = Vec::new();
    for y in 0..buf.h {
        for x in 0..buf.w {
            let i = y * buf.w + x;
            let (material, d) = match surface(i) {
                Surface::Skip => continue,
                Surface::Material(m) => (Some(m), f64::from(buf.data[i])),
                Surface::Background if top => (None, ground_depth),
                Surface::Background => continue,
            };
            let u = buf.origin.x + (x as f64 + 0.5 - crate::hlr::MARGIN_PX) / buf.scale;
            let v = buf.origin.y + (y as f64 + 0.5 - crate::hlr::MARGIN_PX) / buf.scale;
            let q = to_light([u, v, d]);
            let (px, py) = map.to_px(plan_core::Point::new(q[0], q[1]));
            if px < 0.0 || py < 0.0 || px >= map.w as f64 || py >= map.h as f64 {
                continue;
            }
            let nearest = f64::from(map.data[map.pixel_index(px, py)]);
            if nearest > q[2] + bias {
                out.push(Shadowed { index: i, material });
            }
        }
    }
    out
}

/// What lies under a view pixel, as far as shadows care.
pub(crate) enum Surface {
    /// Nothing drawn here.
    Background,
    /// A surface that receives shadow.
    Material(Material),
    /// A pixel that never shows shadow (section poche).
    Skip,
}
