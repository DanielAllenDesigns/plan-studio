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

/// GGX roughness of a scene material (shared with the GL view's shader).
fn roughness_of(m: Material) -> f32 {
    plan_materials::scene_surface(m).roughness
}

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
    /// 0 for dielectrics, 1 for metals: a metal has no diffuse lobe and
    /// reflects with its own colour.
    pub metallic: f32,
    /// Transmission colour for [`Kind::Glass`].
    pub tint: V3,
    /// Light the surface gives off (linear radiance), 0 for most materials.
    pub emission: V3,
}

/// A mesh's own look (`Mesh::color`, and the surface the Material Painter
/// gave it): the scene material it is drawn as, its colour and the surface
/// properties that replace the material's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Custom {
    pub material: Material,
    pub color: [u8; 3],
    pub paint: Option<plan_3d::surface::PaintSurface>,
}

/// Radiance of a fully emissive surface, in units of its colour.
const EMISSIVE_SCALE: f32 = 4.0;

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
                    metallic: 0.0,
                    tint: V3::ONE - (V3::ONE - rgb) * a,
                    emission: V3::ZERO,
                },
                Technique::PhysicallyBased => Surface {
                    kind: Kind::Opaque,
                    albedo: rgb,
                    roughness: roughness_of(m),
                    metallic: plan_materials::scene_surface(m).metallic,
                    tint: V3::ONE,
                    emission: V3::ZERO,
                },
                Technique::Clay | Technique::Ambient => Surface {
                    kind: if translucent {
                        Kind::Clear
                    } else {
                        Kind::Opaque
                    },
                    albedo: V3::splat(CLAY_ALBEDO),
                    roughness: 1.0,
                    metallic: 0.0,
                    tint: V3::ONE,
                    emission: V3::ZERO,
                },
            }
        })
    }

    /// The table of [`Surface::table`] followed by one entry for each
    /// [`Custom`] (a mesh's own `color`): the material's surface with its base
    /// colour replaced and, for a painted mesh, its roughness, metalness,
    /// transparency and emission. Clay and ambient occlusion ignore colour
    /// and gloss, so those entries are the material's own.
    pub fn table_with(technique: Technique, custom: &[Custom]) -> Vec<Surface> {
        let mut table = Self::table(technique).to_vec();
        for c in custom {
            let base = table[c.material.index()];
            let coloured = base.recolored(technique, c.material, c.color);
            table.push(match c.paint {
                Some(p) if technique == Technique::PhysicallyBased => {
                    coloured.painted(c.material, c.color, p)
                }
                _ => coloured,
            });
        }
        table
    }

    /// This (recoloured) surface with a painted material's properties: it
    /// turns into a glass sheet when it is mostly transparent, becomes clear
    /// when it is entirely so, and glows when it is emissive.
    fn painted(self, m: Material, rgb: [u8; 3], p: plan_3d::surface::PaintSurface) -> Surface {
        let [r, g, b] = plan_3d::Mesh::linear_rgb(rgb);
        let c = V3::new(r, g, b);
        let opacity = (1.0 - p.transparency).clamp(0.0, 1.0);
        let roughness = p.roughness.clamp(0.0, 1.0);
        let metallic = p.metallic.clamp(0.0, 1.0);
        // The scene material's own translucency (window glass, water) counts
        // too: a clear pane stays clear when it is painted.
        let own = m.color()[3];
        let opacity = opacity.min(own);
        if opacity < 0.02 {
            return Surface {
                kind: Kind::Clear,
                ..self
            };
        }
        if opacity < 0.999 {
            return Surface {
                kind: Kind::Glass,
                albedo: V3::ZERO,
                roughness: 0.0,
                metallic: 0.0,
                tint: V3::ONE - (V3::ONE - c) * opacity,
                emission: V3::ZERO,
            };
        }
        Surface {
            kind: Kind::Opaque,
            albedo: c,
            roughness,
            metallic,
            tint: V3::ONE,
            emission: c * (p.emissive.clamp(0.0, 1.0) * EMISSIVE_SCALE),
        }
    }

    /// This surface with its base colour replaced by `rgb` (sRGB bytes,
    /// converted to linear light).
    fn recolored(self, technique: Technique, m: Material, rgb: [u8; 3]) -> Surface {
        if technique != Technique::PhysicallyBased {
            return self;
        }
        let [r, g, b] = plan_3d::Mesh::linear_rgb(rgb);
        let c = V3::new(r, g, b);
        match self.kind {
            Kind::Opaque => Surface { albedo: c, ..self },
            Kind::Glass => {
                let a = m.color()[3];
                Surface {
                    tint: V3::ONE - (V3::ONE - c) * a,
                    ..self
                }
            }
            Kind::Clear => self,
        }
    }

    fn alpha2(&self) -> f32 {
        let a = (self.roughness * self.roughness).max(0.02);
        a * a
    }

    /// Probability of sampling the specular lobe (glossier means more).
    fn spec_prob(&self) -> f32 {
        let p = 0.1 + 0.4 * (1.0 - self.roughness);
        // A metal has nothing but the specular lobe.
        p + (0.95 - p).max(0.0) * self.metallic
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
        // Fresnel with F0 = 4% for dielectrics and the base colour for metals.
        let f0 = V3::splat(F0).lerp(self.albedo, self.metallic);
        let fw = (1.0 - o_h).clamp(0.0, 1.0).powi(5);
        let f = f0 + (V3::ONE - f0) * fw;
        let spec = f * (d * g / (4.0 * cos_i * cos_o));
        let diffuse = self.albedo * ((1.0 - self.metallic) * (1.0 - schlick(o_h)) * FRAC_1_PI);
        let ps = self.spec_prob();
        let pdf = (1.0 - ps) * cos_i * FRAC_1_PI + ps * d * n_h / (4.0 * o_h);
        (diffuse + spec, pdf)
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
        let t = Surface::table(Technique::PhysicallyBased);
        let s = t[Material::Framing.index()];
        assert_eq!(s.kind, Kind::Opaque);
        assert!(s.roughness > 0.7);
    }

    #[test]
    fn the_landscape_materials_have_surface_entries() {
        let t = Surface::table(Technique::PhysicallyBased);
        for m in [
            Material::Grass,
            Material::Mulch,
            Material::Foliage,
            Material::Asphalt,
            Material::Gravel,
        ] {
            let s = t[m.index()];
            assert_eq!(s.kind, Kind::Opaque, "{m:?}");
            assert_eq!(s.roughness, roughness_of(m), "{m:?}");
            assert!(s.roughness > 0.85, "{m:?} is a matte surface");
        }
        // Water is a translucent sheet, glossier than the lawn around it.
        let water = t[Material::Water.index()];
        assert_eq!(water.kind, Kind::Glass);
        assert!(roughness_of(Material::Water) < roughness_of(Material::Grass));
        // Clay and ambient renders ignore the colours: every opaque entry is
        // the same grey, water and the tint are clear.
        let clay = Surface::table(Technique::Clay);
        assert_eq!(
            clay[Material::Grass.index()].albedo,
            clay[Material::Asphalt.index()].albedo
        );
        assert_eq!(clay[Material::Water.index()].kind, Kind::Clear);
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

    #[test]
    fn metal_reflects_in_its_own_colour_and_has_no_diffuse_lobe() {
        let t = Surface::table(Technique::PhysicallyBased);
        let metal = Surface {
            albedo: V3::new(0.9, 0.5, 0.1),
            ..t[Material::Metal.index()]
        };
        assert!(metal.metallic > 0.5);
        let n = V3::new(0.0, 1.0, 0.0);
        let wo = V3::new(0.4, 0.9, 0.0).normalized();
        let mirror = V3::new(-0.4, 0.9, 0.0).normalized();
        let (f, _) = metal.eval(n, wo, mirror);
        assert!(f.x > f.y && f.y > f.z, "tinted highlight {f:?}");
        // Far from the mirror direction a metal is nearly black; a plaster wall is not.
        let off = V3::new(0.9, 0.3, 0.0).normalized();
        let (dark, _) = metal.eval(n, wo, off);
        let (lit, _) = t[Material::WallInterior.index()].eval(n, wo, off);
        assert!(
            dark.max_comp() < 0.2 * lit.max_comp(),
            "{dark:?} vs {lit:?}"
        );
    }
}
