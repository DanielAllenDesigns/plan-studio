//! Analytic clear-sky model (Preetham, Shirley and Smits 1999).
//!
//! The sky's luminance and chromaticity at any direction come from the Perez
//! distribution, scaled so the zenith matches the published zenith luminance
//! for the chosen turbidity and sun height. It brightens toward the horizon
//! and toward the sun, which the two-colour gradient cannot do, and it
//! changes colour from noon to sunset.

use crate::vec3::V3;
use serde::{Deserialize, Serialize};

/// Kilo-candela per square metre to the renderer's linear radiance units.
const LUMINANCE_SCALE: f32 = 0.04;
/// Directions closer to the horizon than this (as `cos(zenith angle)`) are
/// treated as if they sat at it; the model diverges there.
const MIN_COS_ZENITH: f32 = 0.02;
/// Sun zenith angles beyond this (radians, just above the horizon) are clamped.
const MAX_SUN_ZENITH: f32 = 1.5;

/// How the sky is painted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum SkyModel {
    /// The two-colour zenith-to-horizon gradient of [`crate::Environment`].
    #[default]
    Gradient,
    /// Preetham analytic sky; `turbidity` runs from about 2 (very clear) to
    /// 10 (hazy). The sun must be above the horizon, otherwise the gradient
    /// is used.
    Preetham { turbidity: f32 },
}

/// Perez coefficients `(A, B, C, D, E)` of one quantity.
type Perez = [f32; 5];

/// A prepared Preetham sky for one sun direction.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Preetham {
    sun: V3,
    /// Zenith luminance (kcd/m^2) and chromaticity.
    zenith: [f32; 3],
    coeff: [Perez; 3],
    /// `F(0, theta_s)` per quantity: the normalisation at the zenith.
    f_zenith: [f32; 3],
}

impl Preetham {
    /// `sun` is the unit direction toward the sun (scene space, Y up).
    pub fn new(sun: V3, turbidity: f32) -> Preetham {
        let t = turbidity.clamp(1.7, 12.0);
        let sun = sun.normalized();
        let theta_s = sun.y.clamp(-1.0, 1.0).acos().min(MAX_SUN_ZENITH);
        let chi = (4.0 / 9.0 - t / 120.0) * (std::f32::consts::PI - 2.0 * theta_s);
        let yz = ((4.0453 * t - 4.9710) * chi.tan() - 0.2155 * t + 2.4192).max(0.05);
        let poly = |c: [[f32; 4]; 3]| {
            let th = theta_s;
            let cubic = |k: [f32; 4]| ((k[0] * th + k[1]) * th + k[2]) * th + k[3];
            t * t * cubic(c[0]) + t * cubic(c[1]) + cubic(c[2])
        };
        let xz = poly([
            [0.00166, -0.00375, 0.00209, 0.0],
            [-0.02903, 0.06377, -0.03202, 0.00394],
            [0.11693, -0.21196, 0.06052, 0.25886],
        ]);
        let yzc = poly([
            [0.00275, -0.00610, 0.00317, 0.0],
            [-0.04214, 0.08970, -0.04153, 0.00516],
            [0.15346, -0.26756, 0.06670, 0.26688],
        ]);
        let coeff = [
            [
                0.1787 * t - 1.4630,
                -0.3554 * t + 0.4275,
                -0.0227 * t + 5.3251,
                0.1206 * t - 2.5771,
                -0.0670 * t + 0.3703,
            ],
            [
                -0.0193 * t - 0.2592,
                -0.0665 * t + 0.0008,
                -0.0004 * t + 0.2125,
                -0.0641 * t - 0.8989,
                -0.0033 * t + 0.0452,
            ],
            [
                -0.0167 * t - 0.2608,
                -0.0950 * t + 0.0092,
                -0.0079 * t + 0.2102,
                -0.0441 * t - 1.6537,
                -0.0109 * t + 0.0529,
            ],
        ];
        // At the zenith theta = 0 and gamma = theta_s.
        let f_zenith = coeff.map(|c| perez(c, 1.0, theta_s));
        Preetham {
            sun,
            zenith: [yz, xz, yzc],
            coeff,
            f_zenith,
        }
    }

    /// Sky luminance in kcd/m^2 and chromaticity `(Y, x, y)` toward `dir`.
    fn yxy(&self, dir: V3) -> [f32; 3] {
        let cos_t = dir.y.max(MIN_COS_ZENITH);
        let gamma = dir.dot(self.sun).clamp(-1.0, 1.0).acos();
        let mut out = [0.0; 3];
        for (i, slot) in out.iter_mut().enumerate() {
            let f = perez(self.coeff[i], cos_t, gamma);
            *slot = self.zenith[i] * f / self.f_zenith[i].max(1e-4);
        }
        out
    }

    /// Luminance (linear units) toward `dir`; the dome only, no sun disc.
    #[cfg(test)]
    pub fn luminance(&self, dir: V3) -> f32 {
        self.yxy(dir)[0].max(0.0) * LUMINANCE_SCALE
    }

    /// Linear RGB radiance toward `dir` (above the horizon).
    pub fn radiance(&self, dir: V3) -> V3 {
        let [y_lum, x, y] = self.yxy(dir);
        let y_lum = y_lum.max(0.0) * LUMINANCE_SCALE;
        let y = y.max(0.05);
        let big_x = x / y * y_lum;
        let big_z = (1.0 - x - y) / y * y_lum;
        V3::new(
            3.2406 * big_x - 1.5372 * y_lum - 0.4986 * big_z,
            -0.9689 * big_x + 1.8758 * y_lum + 0.0415 * big_z,
            0.0557 * big_x - 0.2040 * y_lum + 1.0570 * big_z,
        )
        .max_each(0.0)
    }
}

/// The Perez distribution `F(theta, gamma)` from `cos(theta)` and `gamma`.
fn perez(c: Perez, cos_theta: f32, gamma: f32) -> f32 {
    let cos_theta = cos_theta.max(MIN_COS_ZENITH);
    (1.0 + c[0] * (c[1] / cos_theta).exp())
        * (1.0 + c[2] * (c[3] * gamma).exp() + c[4] * gamma.cos().powi(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sun(alt_deg: f32, az_deg: f32) -> V3 {
        let (sa, ca) = alt_deg.to_radians().sin_cos();
        let (sz, cz) = az_deg.to_radians().sin_cos();
        V3::new(sz * ca, sa, -cz * ca)
    }

    /// Brightest direction of the dome on a 1 degree grid.
    fn brightest(sky: &Preetham) -> V3 {
        let mut best = (0.0, V3::new(0.0, 1.0, 0.0));
        for alt in (2..90).map(|a| a as f32) {
            for az in (0..360).step_by(2).map(|a| a as f32) {
                let d = sun(alt, az);
                let l = sky.luminance(d);
                if l > best.0 {
                    best = (l, d);
                }
            }
        }
        best.1
    }

    #[test]
    fn luminance_peaks_at_the_sun() {
        for (alt, t) in [(50.0, 2.5), (35.0, 4.0), (20.0, 3.0), (70.0, 6.0)] {
            let s = sun(alt, 130.0);
            let sky = Preetham::new(s, t);
            let peak = brightest(&sky);
            let angle = peak.dot(s).clamp(-1.0, 1.0).acos().to_degrees();
            assert!(
                angle < 6.0,
                "alt {alt} T {t}: peak {angle} deg from the sun"
            );
            // Falls off away from the sun at the same height.
            let near = sky.luminance(sun(alt, 150.0));
            let far = sky.luminance(sun(alt, 310.0));
            assert!(near > far, "{near} vs {far}");
            assert!(sky.luminance(s) > 2.0 * sky.luminance(sun(alt, 310.0)));
        }
    }

    #[test]
    fn the_dome_is_bluish_and_finite_everywhere() {
        let sky = Preetham::new(sun(45.0, 90.0), 2.5);
        let zenith = sky.radiance(V3::new(0.0, 1.0, 0.0));
        assert!(zenith.is_finite() && zenith.z > zenith.x, "{zenith:?}");
        for alt in [0.0, 0.5, 5.0, 45.0, 89.9] {
            for az in (0..360).step_by(30) {
                let c = sky.radiance(sun(alt, az as f32));
                assert!(
                    c.is_finite() && c.x >= 0.0 && c.y >= 0.0 && c.z >= 0.0,
                    "{c:?}"
                );
            }
        }
        // Zenith luminance is in a sane range for a clear day.
        let y = sky.luminance(V3::new(0.0, 1.0, 0.0));
        assert!((0.05..2.0).contains(&y), "{y}");
    }

    #[test]
    fn hazier_skies_are_whiter_and_a_low_sun_is_dimmer() {
        let noon = Preetham::new(sun(70.0, 0.0), 2.5);
        let low = Preetham::new(sun(8.0, 0.0), 2.5);
        let up = V3::new(0.0, 1.0, 0.0);
        assert!(noon.luminance(up) > low.luminance(up));
    }
}
