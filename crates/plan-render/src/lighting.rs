//! Light sources and the sky environment.

use crate::sky::{Preetham, SkyModel};
use crate::vec3::V3;

/// The sun as a distant disc light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    /// Unit direction *toward* the sun (scene space).
    pub direction: [f32; 3],
    /// Irradiance on a surface facing the sun (linear units).
    pub intensity: f32,
    /// Light colour (linear RGB).
    pub color: [f32; 3],
    /// Angular radius of the disc in degrees; sets the penumbra width.
    pub angular_radius_deg: f32,
}

impl Sun {
    /// Sun from a compass azimuth and altitude, in degrees.
    ///
    /// Azimuth runs clockwise from plan north (+plan Y, scene -Z) through east
    /// (+X); altitude is measured up from the horizon.
    pub fn from_azimuth_altitude(az_deg: f32, alt_deg: f32) -> Sun {
        let (saz, caz) = az_deg.to_radians().sin_cos();
        let (salt, calt) = alt_deg.to_radians().sin_cos();
        Sun {
            direction: [saz * calt, salt, -caz * calt],
            intensity: 8.0,
            color: [1.0, 0.96, 0.88],
            angular_radius_deg: 0.27,
        }
    }
}

/// A spherical point light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLight {
    /// Centre, scene inches.
    pub position: [f32; 3],
    /// Radiant intensity; irradiance at distance `d` inches is `intensity / d^2`.
    pub intensity: f32,
    /// Light colour (linear RGB).
    pub color: [f32; 3],
    /// Sphere radius in inches; sets shadow softness (`0` = hard).
    pub radius: f32,
}

/// A rectangular light panel (a ceiling troffer, a softbox, a lit window).
///
/// It emits from the side its normal `edge_u x edge_v` points to (both sides
/// when `two_sided`). Light is sampled directly (next-event estimation), so a
/// small bright panel renders cleanly with few samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaLight {
    /// One corner, scene inches.
    pub corner: [f32; 3],
    /// First edge vector (corner to the next corner), inches.
    pub edge_u: [f32; 3],
    /// Second edge vector, inches.
    pub edge_v: [f32; 3],
    /// Emitted radiance (linear RGB, colour times intensity).
    pub radiance: [f32; 3],
    /// Emit from both faces.
    pub two_sided: bool,
}

impl AreaLight {
    /// A horizontal panel centred at `center`, `width` along X and `depth`
    /// along Z, shining down (a ceiling light).
    pub fn ceiling_panel(
        center: [f32; 3],
        width: f32,
        depth: f32,
        radiance: [f32; 3],
    ) -> AreaLight {
        AreaLight {
            corner: [center[0] - width * 0.5, center[1], center[2] - depth * 0.5],
            edge_u: [width, 0.0, 0.0],
            edge_v: [0.0, 0.0, depth],
            radiance,
            two_sided: false,
        }
    }
}

/// Sky, ground and optional sun surrounding the model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Environment {
    /// Sky radiance straight up.
    pub sky_color_zenith: [f32; 3],
    /// Sky radiance at the horizon.
    pub sky_color_horizon: [f32; 3],
    /// Radiance of everything below the horizon.
    pub ground_color: [f32; 3],
    /// Sunlight, if any.
    pub sun: Option<Sun>,
    /// How the sky is painted: the gradient above or an analytic clear sky.
    pub sky_model: SkyModel,
}

impl Default for Environment {
    /// A clear afternoon: blue sky, grey-brown ground, sun to the south-west.
    fn default() -> Self {
        Environment {
            sky_color_zenith: [0.22, 0.40, 0.80],
            sky_color_horizon: [0.70, 0.80, 0.90],
            ground_color: [0.28, 0.26, 0.23],
            sun: Some(Sun::from_azimuth_altitude(225.0, 50.0)),
            sky_model: SkyModel::Gradient,
        }
    }
}

/// Sky gradient in renderer form.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sky {
    zenith: V3,
    horizon: V3,
    ground: V3,
    analytic: Option<Preetham>,
}

impl Sky {
    pub fn new(env: &Environment) -> Sky {
        // The analytic sky needs a sun above the horizon to anchor it.
        let analytic = match (env.sky_model, &env.sun) {
            (SkyModel::Preetham { turbidity }, Some(sun)) if sun.direction[1] > 0.0 => {
                Some(Preetham::new(V3::from_array(sun.direction), turbidity))
            }
            _ => None,
        };
        Sky {
            zenith: V3::from_array(env.sky_color_zenith),
            horizon: V3::from_array(env.sky_color_horizon),
            ground: V3::from_array(env.ground_color),
            analytic,
        }
    }

    /// Radiance arriving from direction `d` (unit), excluding the sun.
    pub fn radiance(&self, d: V3) -> V3 {
        if d.y < 0.0 {
            self.ground
        } else if let Some(p) = &self.analytic {
            p.radiance(d)
        } else {
            self.horizon.lerp(self.zenith, d.y.min(1.0).sqrt())
        }
    }
}

/// Sun in renderer form.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SunData {
    pub dir: V3,
    pub irradiance: V3,
    pub cos_max: f32,
}

impl SunData {
    pub fn new(sun: &Sun) -> SunData {
        SunData {
            dir: V3::from_array(sun.direction).normalized(),
            irradiance: V3::from_array(sun.color) * sun.intensity.max(0.0),
            cos_max: sun.angular_radius_deg.max(0.0).to_radians().cos(),
        }
    }

    /// Radiance of the visible disc (capped so it cannot swamp averages).
    pub fn disc_radiance(&self) -> V3 {
        let solid_angle = std::f32::consts::TAU * (1.0 - self.cos_max);
        if solid_angle <= 0.0 {
            return V3::ZERO;
        }
        (self.irradiance / solid_angle).min_each(100.0)
    }
}

/// Point light in renderer form.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LightData {
    pub pos: V3,
    pub intensity: V3,
    pub radius: f32,
}

impl LightData {
    pub fn new(l: &PointLight) -> LightData {
        LightData {
            pos: V3::from_array(l.position),
            intensity: V3::from_array(l.color) * l.intensity.max(0.0),
            radius: l.radius.max(0.0),
        }
    }
}

/// Area light in renderer form.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AreaData {
    corner: V3,
    u: V3,
    v: V3,
    normal: V3,
    pub area: f32,
    pub radiance: V3,
    two_sided: bool,
}

impl AreaData {
    /// `None` for a degenerate (zero-area) panel.
    pub fn new(a: &AreaLight) -> Option<AreaData> {
        let (u, v) = (V3::from_array(a.edge_u), V3::from_array(a.edge_v));
        let cross = u.cross(v);
        let area = cross.length();
        (area.is_finite() && area > 1e-6).then(|| AreaData {
            corner: V3::from_array(a.corner),
            u,
            v,
            normal: cross / area,
            area,
            radiance: V3::from_array(a.radiance).max_each(0.0),
            two_sided: a.two_sided,
        })
    }

    /// Point at panel coordinates `(s, t)` in `[0, 1]`.
    pub fn point(&self, s: f32, t: f32) -> V3 {
        self.corner + self.u * s + self.v * t
    }

    /// Cosine between the panel normal and the direction light leaves along
    /// `-wi` (0 when the emitting side faces away).
    pub fn emit_cos(&self, wi: V3) -> f32 {
        let c = -self.normal.dot(wi);
        if self.two_sided {
            c.abs()
        } else {
            c.max(0.0)
        }
    }

    /// Distance along a ray to the panel's emitting face, if it is hit within `tmax`.
    pub fn hit(&self, o: V3, d: V3, tmax: f32) -> Option<f32> {
        let denom = self.normal.dot(d);
        if denom.abs() < 1e-7 || (!self.two_sided && denom > 0.0) {
            return None;
        }
        let t = self.normal.dot(self.corner - o) / denom;
        if !(1e-3..tmax).contains(&t) {
            return None;
        }
        let rel = o + d * t - self.corner;
        let (uu, vv) = (self.u.length_sq(), self.v.length_sq());
        let s = rel.dot(self.u) / uu;
        let r = rel.dot(self.v) / vv;
        ((0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&r)).then_some(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_direction_is_unit_and_compass_correct() {
        let s = Sun::from_azimuth_altitude(90.0, 30.0);
        let d = V3::from_array(s.direction);
        assert!((d.length() - 1.0).abs() < 1e-5);
        assert!(d.x > 0.8 && d.y > 0.49 && d.y < 0.51); // east, 30 degrees up
        let n = V3::from_array(Sun::from_azimuth_altitude(0.0, 0.0).direction);
        assert!(n.z < -0.99); // north is scene -Z
    }

    #[test]
    fn a_ceiling_panel_shines_down_and_is_hit_from_below_only() {
        let a = AreaData::new(&AreaLight::ceiling_panel(
            [0.0, 96.0, 0.0],
            24.0,
            48.0,
            [5.0; 3],
        ))
        .unwrap();
        assert!((a.area - 24.0 * 48.0).abs() < 1e-2);
        let below = V3::new(0.0, 0.0, 0.0);
        let t = a.hit(below, V3::new(0.0, 1.0, 0.0), f32::INFINITY).unwrap();
        assert!((t - 96.0).abs() < 1e-3);
        // From above the panel is dark (one sided) and a miss to the side.
        assert!(a
            .hit(
                V3::new(0.0, 200.0, 0.0),
                V3::new(0.0, -1.0, 0.0),
                f32::INFINITY
            )
            .is_none());
        assert!(a.hit(below, V3::new(0.0, 1.0, 0.0), 50.0).is_none());
        assert!(a
            .hit(
                V3::new(40.0, 0.0, 0.0),
                V3::new(0.0, 1.0, 0.0),
                f32::INFINITY
            )
            .is_none());
        // `wi` points from the lit surface toward the panel.
        assert!(a.emit_cos(V3::new(0.0, 1.0, 0.0)) > 0.99);
        assert_eq!(a.emit_cos(V3::new(0.0, -1.0, 0.0)), 0.0);
        assert!(AreaData::new(&AreaLight {
            edge_v: [0.0; 3],
            ..AreaLight::ceiling_panel([0.0; 3], 1.0, 1.0, [1.0; 3])
        })
        .is_none());
    }
}
