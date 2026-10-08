//! Sun position and light sources.

use plan_core::Point;
use serde::{Deserialize, Serialize};

/// Sun parameters for shadows and lighting.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SunSettings {
    /// Compass bearing of the sun, degrees clockwise from north.
    pub azimuth_deg: f64,
    /// Height above the horizon, degrees.
    pub altitude_deg: f64,
    /// Relative intensity, 0..1.
    pub intensity: f32,
    pub color: [u8; 3],
    /// `(month, day)`, 1-based.
    pub date: (u32, u32),
    /// Local solar time in hours (12.0 = solar noon).
    pub time_hours: f64,
    /// Latitude in degrees, north positive.
    pub latitude: f64,
}

impl SunSettings {
    /// Computes the sun position for a date, local solar time and latitude.
    ///
    /// Uses Cooper's declination formula and the standard hour-angle
    /// relations; the equation of time and longitude offset are ignored, so
    /// `time_hours` is solar time. Below the horizon the intensity is 0.
    pub fn from_date_time_location(date: (u32, u32), time_hours: f64, latitude: f64) -> Self {
        const CUM_DAYS: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        let month = date.0.clamp(1, 12);
        let day_of_year = f64::from(CUM_DAYS[month as usize - 1] + date.1.clamp(1, 31));
        let decl =
            (23.44_f64 * (360.0 / 365.0 * (284.0 + day_of_year)).to_radians().sin()).to_radians();
        let hour_angle = (15.0 * (time_hours - 12.0)).to_radians();
        let lat = latitude.to_radians();

        let sin_alt =
            (lat.sin() * decl.sin() + lat.cos() * decl.cos() * hour_angle.cos()).clamp(-1.0, 1.0);
        let alt = sin_alt.asin();
        let denom = alt.cos() * lat.cos();
        let mut az = if denom.abs() < 1e-9 {
            180.0
        } else {
            ((decl.sin() - sin_alt * lat.sin()) / denom)
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees()
        };
        if hour_angle > 0.0 {
            az = 360.0 - az;
        }
        let altitude_deg = alt.to_degrees();
        // Warm near the horizon, neutral daylight above ~45 degrees.
        let k = (altitude_deg / 45.0).clamp(0.0, 1.0) as f32;
        let lerp = |a: f32, b: f32| (a + (b - a) * k).round() as u8;
        Self {
            azimuth_deg: az,
            altitude_deg,
            intensity: if altitude_deg > 0.0 {
                sin_alt.sqrt() as f32
            } else {
                0.0
            },
            color: [lerp(255.0, 255.0), lerp(168.0, 244.0), lerp(96.0, 229.0)],
            date,
            time_hours,
            latitude,
        }
    }

    /// Unit vector pointing toward the sun in the 3D scene frame
    /// (X east, Y up, -Z north).
    pub fn direction_to_sun(&self) -> [f64; 3] {
        let (alt, az) = (
            self.altitude_deg.to_radians(),
            self.azimuth_deg.to_radians(),
        );
        [alt.cos() * az.sin(), alt.sin(), -alt.cos() * az.cos()]
    }
}

/// Kind of artificial light.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LightKind {
    /// Omnidirectional.
    Point,
    /// Cone of `cone_deg` full angle along `direction`.
    Spot { cone_deg: f64 },
    /// Parallel rays along `direction` (like distant sun).
    Parallel,
}

/// An artificial light. Positions are plan inches with Z up.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LightSource {
    pub kind: LightKind,
    pub position: [f64; 3],
    /// Unit aim vector for spot and parallel lights.
    pub direction: [f64; 3],
    pub intensity: f32,
    pub color: [u8; 3],
    pub on: bool,
}

/// Chief-style default room light: a warm point light 12" below the ceiling
/// at the room centre.
pub fn default_room_light(room_center: Point, ceiling_height: f64) -> LightSource {
    LightSource {
        kind: LightKind::Point,
        position: [
            room_center.x,
            room_center.y,
            (ceiling_height - 12.0).max(0.0),
        ],
        direction: [0.0, 0.0, -1.0],
        intensity: 1.0,
        color: [255, 244, 229],
        on: true,
    }
}
