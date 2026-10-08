//! Light sources and the sky environment.

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
}

impl Default for Environment {
    /// A clear afternoon: blue sky, grey-brown ground, sun to the south-west.
    fn default() -> Self {
        Environment {
            sky_color_zenith: [0.22, 0.40, 0.80],
            sky_color_horizon: [0.70, 0.80, 0.90],
            ground_color: [0.28, 0.26, 0.23],
            sun: Some(Sun::from_azimuth_altitude(225.0, 50.0)),
        }
    }
}

/// Sky gradient in renderer form.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sky {
    zenith: V3,
    horizon: V3,
    ground: V3,
}

impl Sky {
    pub fn new(env: &Environment) -> Sky {
        Sky {
            zenith: V3::from_array(env.sky_color_zenith),
            horizon: V3::from_array(env.sky_color_horizon),
            ground: V3::from_array(env.ground_color),
        }
    }

    /// Radiance arriving from direction `d` (unit), excluding the sun.
    pub fn radiance(&self, d: V3) -> V3 {
        if d.y >= 0.0 {
            self.horizon.lerp(self.zenith, d.y.min(1.0).sqrt())
        } else {
            self.ground
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
}
