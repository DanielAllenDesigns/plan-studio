//! Surface models: Lambert diffuse + GGX specular, thin-sheet glass.

use crate::rng::Rng;
use crate::settings::Technique;
use crate::vec3::{to_world, V3};
use plan_3d::Material;
use std::f32::consts::{FRAC_1_PI, PI, TAU};

/// Number of entries in a surface table (one per [`Material`]).
pub(crate) const MATERIAL_COUNT: usize = Material::ALL.len();

/// Specular reflectance at normal incidence of a dielectric (IOR 1.5).
const F0: f32 = 0.04;
/// Index of refraction of window glass.
const GLASS_IOR: f32 = 1.5;
/// Albedo used by the clay technique.
const CLAY_ALBEDO: f32 = 0.72;
/// GGX roughness per [`Material`], in `Material::ALL` order.
const ROUGHNESS: [f32; MATERIAL_COUNT] = [
    0.85, 0.9, 0.4, 0.95, 0.45, 0.0, 0.35, 0.75, 0.95, 0.8, 0.9, 0.85, 0.9, 0.5, 0.0, 0.35, 0.8,
];

/// How light interacts with a surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Opaque dielectric: diffuse base with a GGX specular coat.
    Opaque,
    /// Thin glass pane: Fresnel reflection, tinted transmission.
    Glass,
    /// Invisible surface: rays pass straight through.
    Clear,
}

/// Shading parameters for one material.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Surface {
    pub kind: Kind,
    pub albedo: V3,
    pub roughness: f32,
    /// Transmission colour for [`Kind::Glass`].
    pub tint: V3,
}

impl Surface {
    /// The per-material table for a technique, indexed by `Material::index()`.
    pub fn table(technique: Technique) -> [Surface; MATERIAL_COUNT] {
        Material::ALL.map(|m| {
            let [r, g, b, a] = m.color();
            let rgb = V3::new(r, g, b);
            let translucent = a < 0.999;
            match technique {
                Technique::PhysicallyBased if translucent => Surface {
                    kind: Kind::Glass,
                    albedo: V3::ZERO,
                    roughness: 0.0,
                    tint: V3::ONE - (V3::ONE - rgb) * a,
                },
                Technique::PhysicallyBased => Surface {
                    kind: Kind::Opaque,
                    albedo: rgb,
                    roughness: ROUGHNESS[m.index()],
                    tint: V3::ONE,
                },
                Technique::Clay | Technique::Ambient => Surface {
                    kind: if translucent {
                        Kind::Clear
                    } else {
                        Kind::Opaque
                    },
                    albedo: V3::splat(CLAY_ALBEDO),
                    roughness: 1.0,
                    tint: V3::ONE,
                },
            }
        })
    }

    fn alpha2(&self) -> f32 {
        let a = (self.roughness * self.roughness).max(0.02);
        a * a
    }

    /// Probability of sampling the specular lobe (glossier means more).
    fn spec_prob(&self) -> f32 {
        0.1 + 0.4 * (1.0 - self.roughness)
    }

    /// BSDF value and the mixture sampling density for `wi` given `wo`.
    ///
    /// All vectors point away from the surface; `n` faces the viewer.
    pub fn eval(&self, n: V3, wo: V3, wi: V3) -> (V3, f32) {
        let cos_i = n.dot(wi);
        let cos_o = n.dot(wo);
        if cos_i <= 0.0 || cos_o <= 0.0 {
            return (V3::ZERO, 0.0);
        }
        let h = (wo + wi).normalized();
        let n_h = n.dot(h).max(0.0);
        let o_h = wo.dot(h).max(1e-4);
        let a2 = self.alpha2();
        let d = ggx_d(n_h, a2);
        let g = smith_g1(cos_i, a2) * smith_g1(cos_o, a2);
        let f = schlick(o_h);
        let spec = d * g * f / (4.0 * cos_i * cos_o);
        let diffuse = self.albedo * ((1.0 - f) * FRAC_1_PI);
        let ps = self.spec_prob();
        let pdf = (1.0 - ps) * cos_i * FRAC_1_PI + ps * d * n_h / (4.0 * o_h);
        (diffuse + V3::splat(spec), pdf)
    }

    /// Sample an incoming direction; returns it with `f * cos / pdf`.
    pub fn sample(&self, n: V3, wo: V3, rng: &mut Rng) -> Option<(V3, V3)> {
        let (u1, u2) = (rng.next_f32(), rng.next_f32());
        let wi = if rng.next_f32() < self.spec_prob() {
            let a2 = self.alpha2();
            let cos_t = ((1.0 - u1) / (1.0 + (a2 - 1.0) * u1)).sqrt();
            let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
            let (sin_p, cos_p) = (TAU * u2).sin_cos();
            let h = to_world(V3::new(sin_t * cos_p, sin_t * sin_p, cos_t), n);
            h * (2.0 * wo.dot(h)) - wo
        } else {
            sample_cosine(n, u1, u2)
        };
        let (f, pdf) = self.eval(n, wo, wi);
        if pdf <= 0.0 || f.max_comp() <= 0.0 {
            return None;
        }
        Some((wi, f * (n.dot(wi) / pdf)))
    }
}

/// Cosine-weighted direction about `n`.
pub(crate) fn sample_cosine(n: V3, u1: f32, u2: f32) -> V3 {
    let r = u1.sqrt();
    let (sin, cos) = (TAU * u2).sin_cos();
    to_world(V3::new(r * cos, r * sin, (1.0 - u1).max(0.0).sqrt()), n)
}

/// Uniform direction inside the cone of half-angle `acos(cos_max)` about `axis`.
pub(crate) fn sample_cone(axis: V3, cos_max: f32, u1: f32, u2: f32) -> V3 {
    let cos_t = 1.0 - u1 * (1.0 - cos_max);
    let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
    let (sin, cos) = (TAU * u2).sin_cos();
    to_world(V3::new(sin_t * cos, sin_t * sin, cos_t), axis)
}

/// Mirror `d` about the unit normal `n`.
pub(crate) fn reflect(d: V3, n: V3) -> V3 {
    d - n * (2.0 * d.dot(n))
}

fn schlick(cos: f32) -> f32 {
    F0 + (1.0 - F0) * (1.0 - cos.clamp(0.0, 1.0)).powi(5)
}

fn ggx_d(n_h: f32, a2: f32) -> f32 {
    let t = n_h * n_h * (a2 - 1.0) + 1.0;
    a2 / (PI * t * t)
}

fn smith_g1(cos: f32, a2: f32) -> f32 {
    2.0 * cos / (cos + (a2 + (1.0 - a2) * cos * cos).sqrt())
}

/// Reflectance of a thin glass sheet (two interfaces, incoherent) at `cos_i`.
pub(crate) fn sheet_reflectance(cos_i: f32) -> f32 {
    let cos_i = cos_i.clamp(1e-4, 1.0);
    let sin_t2 = (1.0 - cos_i * cos_i) / (GLASS_IOR * GLASS_IOR);
    let cos_t = (1.0 - sin_t2).sqrt();
    let rs = (cos_i - GLASS_IOR * cos_t) / (cos_i + GLASS_IOR * cos_t);
    let rp = (GLASS_IOR * cos_i - cos_t) / (GLASS_IOR * cos_i + cos_t);
    let f = 0.5 * (rs * rs + rp * rp);
    2.0 * f / (1.0 + f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_material_has_a_surface_and_framing_is_rough_lumber() {
        for technique in [
            Technique::PhysicallyBased,
            Technique::Clay,
            Technique::Ambient,
        ] {
            assert_eq!(Surface::table(technique).len(), Material::ALL.len());
        }
        assert_eq!(ROUGHNESS.len(), Material::ALL.len());
        let t = Surface::table(Technique::PhysicallyBased);
        let s = t[Material::Framing.index()];
        assert_eq!(s.kind, Kind::Opaque);
        assert!(s.roughness > 0.7);
    }

    #[test]
    fn glass_reflects_about_eight_percent_head_on_and_more_at_grazing() {
        let head_on = sheet_reflectance(1.0);
        assert!((head_on - 0.0769).abs() < 0.002, "{head_on}");
        assert!(sheet_reflectance(0.05) > 0.5);
    }

    #[test]
    fn diffuse_sampling_conserves_energy() {
        // Mean sampled weight approximates directional albedo: positive, below one.
        let table = Surface::table(Technique::PhysicallyBased);
        let floor = table[Material::Floor.index()];
        let n = V3::new(0.0, 1.0, 0.0);
        let wo = V3::new(0.3, 0.9, 0.1).normalized();
        let mut rng = Rng::new(3);
        let mut sum = V3::ZERO;
        let count = 20_000;
        for _ in 0..count {
            if let Some((_, w)) = floor.sample(n, wo, &mut rng) {
                sum += w;
            }
        }
        let mean = sum / count as f32;
        assert!(mean.x > 0.1 && mean.max_comp() < 1.0, "{mean:?}");
    }
}
