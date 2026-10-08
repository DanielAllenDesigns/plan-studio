//! Settings and plain-Rust math of the interactive renderer: quality presets,
//! the "look" of each rendering technique, the sun's shadow-map matrix, the
//! ambient-occlusion sample kernel, the tone curve and the choice of nearby
//! point lights.
//!
//! Nothing here touches OpenGL, so all of it is unit tested; the shader code
//! in `gpu` / `pipeline` mirrors [`tone_map`] and uses these values as uniforms.

use crate::math::{self, Mat4, Vec3};

/// Most point lights the shader evaluates per pixel.
pub const MAX_POINT_LIGHTS: usize = 8;
/// Largest AO kernel the shader declares.
pub const MAX_AO_SAMPLES: usize = 16;

/// How much the interactive view spends on lighting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    /// 1024 shadow map with hard edges, 8 AO samples, no anti-aliasing.
    Low,
    /// 2048 shadow map with 3 x 3 PCF, 12 AO samples, FXAA.
    #[default]
    Medium,
    /// 4096 shadow map with 5 x 5 PCF, 16 AO samples, FXAA.
    High,
}

impl Quality {
    /// Every preset, lowest first.
    pub const ALL: [Quality; 3] = [Quality::Low, Quality::Medium, Quality::High];

    /// Menu name.
    pub fn label(self) -> &'static str {
        match self {
            Quality::Low => "Low",
            Quality::Medium => "Medium",
            Quality::High => "High",
        }
    }

    /// Shadow-map side in texels.
    pub fn shadow_size(self) -> i32 {
        match self {
            Quality::Low => 1024,
            Quality::Medium => 2048,
            Quality::High => 4096,
        }
    }

    /// PCF kernel radius in texels (the kernel is `2r + 1` square).
    pub fn pcf_radius(self) -> i32 {
        match self {
            Quality::Low => 0,
            Quality::Medium => 1,
            Quality::High => 2,
        }
    }

    /// Ambient-occlusion samples per pixel.
    pub fn ao_samples(self) -> usize {
        match self {
            Quality::Low => 8,
            Quality::Medium => 12,
            Quality::High => MAX_AO_SAMPLES,
        }
    }

    /// Whether the fast anti-aliasing pass runs.
    pub fn fxaa(self) -> bool {
        self != Quality::Low
    }

    fn key(self) -> &'static str {
        match self {
            Quality::Low => "low",
            Quality::Medium => "medium",
            Quality::High => "high",
        }
    }

    fn from_key(s: &str) -> Option<Quality> {
        Quality::ALL.into_iter().find(|q| q.key() == s)
    }
}

/// The 3D view options the user toggles (shadows, ambient occlusion, quality).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewSettings {
    /// Cast shadows from the sun (where the technique allows them).
    pub shadows: bool,
    /// Darken creases and contact areas (screen-space ambient occlusion).
    pub ambient_occlusion: bool,
    /// Quality preset for shadows, occlusion and anti-aliasing.
    pub quality: Quality,
    /// Linear exposure applied before the tone curve.
    pub exposure: f32,
}

impl Default for ViewSettings {
    fn default() -> Self {
        ViewSettings {
            shadows: true,
            ambient_occlusion: true,
            quality: Quality::Medium,
            exposure: 1.0,
        }
    }
}

impl ViewSettings {
    /// `key=value` text, one line, for saving with a view.
    pub fn to_text(&self) -> String {
        format!(
            "shadows={} ao={} quality={} exposure={}",
            u8::from(self.shadows),
            u8::from(self.ambient_occlusion),
            self.quality.key(),
            self.exposure
        )
    }

    /// Parses [`ViewSettings::to_text`]; unknown or missing keys keep their
    /// default, so older text still loads.
    pub fn from_text(text: &str) -> ViewSettings {
        let mut s = ViewSettings::default();
        for part in text.split_whitespace() {
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };
            match key {
                "shadows" => s.shadows = value == "1",
                "ao" => s.ambient_occlusion = value == "1",
                "quality" => s.quality = Quality::from_key(value).unwrap_or(s.quality),
                "exposure" => {
                    if let Ok(v) = value.parse::<f32>() {
                        s.exposure = v.clamp(0.1, 8.0);
                    }
                }
                _ => {}
            }
        }
        s
    }
}

/// The look a rendering technique gives the interactive view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Look {
    /// Shaded materials with sky light, sun shadows and soft occlusion.
    #[default]
    Standard,
    /// Standard with glossier highlights and sky reflections.
    Physical,
    /// One matte material, strong occlusion and soft shadows.
    Clay,
    /// Everything translucent with emphasised edges.
    GlassHouse,
    /// Pigment washes: soft colour, darkened edges and paper grain.
    Watercolor,
    /// Flat banded colour with bold silhouette and crease lines.
    Technical,
    /// Two tones from light to shade.
    Duotone,
    /// Unlit flat colour (line drawing).
    Flat,
}

/// Per-look constants for the shaders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookParams {
    /// Shader code (`u_look`).
    pub code: i32,
    /// Paint a sky gradient and ground fade behind the model.
    pub sky: bool,
    /// Sun shadows are drawn.
    pub shadows: bool,
    /// Strength of the ambient occlusion, 0 for none.
    pub ao_strength: f32,
    /// Specular highlights and sky reflections, 0 for none.
    pub specular: f32,
    /// Strength of the depth/normal edge lines in the final pass, 0 for none.
    pub edge_lines: f32,
    /// Line width in points.
    pub edge_width: f32,
    /// Line colour (linear).
    pub edge_color: [f32; 3],
    /// Pigment wash and paper grain strength, 0 for none.
    pub wash: f32,
}

impl Look {
    /// The constants for this look.
    pub fn params(self) -> LookParams {
        let base = LookParams {
            code: 0,
            sky: false,
            shadows: false,
            ao_strength: 0.0,
            specular: 0.0,
            edge_lines: 0.0,
            edge_width: 1.0,
            edge_color: [0.08, 0.08, 0.09],
            wash: 0.0,
        };
        match self {
            Look::Standard => LookParams {
                sky: true,
                shadows: true,
                ao_strength: 0.8,
                specular: 0.6,
                ..base
            },
            Look::Physical => LookParams {
                code: 1,
                sky: true,
                shadows: true,
                ao_strength: 0.9,
                specular: 1.0,
                ..base
            },
            Look::Clay => LookParams {
                code: 2,
                shadows: true,
                ao_strength: 1.0,
                ..base
            },
            Look::GlassHouse => LookParams {
                code: 3,
                edge_width: 1.5,
                edge_color: [0.15, 0.17, 0.2],
                ..base
            },
            Look::Watercolor => LookParams {
                code: 4,
                shadows: true,
                ao_strength: 0.4,
                edge_lines: 0.8,
                edge_width: 1.6,
                edge_color: [0.38, 0.30, 0.22],
                wash: 1.0,
                ..base
            },
            Look::Technical => LookParams {
                code: 5,
                ao_strength: 0.0,
                edge_lines: 1.0,
                edge_width: 1.6,
                edge_color: [0.04, 0.04, 0.05],
                ..base
            },
            Look::Duotone => LookParams {
                code: 6,
                shadows: true,
                ao_strength: 0.6,
                ..base
            },
            Look::Flat => LookParams {
                code: 7,
                edge_lines: 0.7,
                edge_color: [0.05, 0.05, 0.06],
                ..base
            },
        }
    }
}

/// A point light as the shader sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewLight {
    /// Scene position, inches.
    pub position: Vec3,
    /// Linear colour times intensity (already divided by pi: add straight to
    /// the diffuse term, divide by squared distance).
    pub color: Vec3,
}

/// The up to [`MAX_POINT_LIGHTS`] lights closest to `eye`, nearest first.
///
/// Lights with no energy are dropped first.
pub fn nearest_lights(lights: &[ViewLight], eye: Vec3, max: usize) -> Vec<ViewLight> {
    let mut v: Vec<ViewLight> = lights
        .iter()
        .copied()
        .filter(|l| l.color.iter().any(|c| *c > 0.0) && l.position.iter().all(|c| c.is_finite()))
        .collect();
    let dist = |l: &ViewLight| math::length(math::sub(l.position, eye));
    v.sort_by(|a, b| dist(a).total_cmp(&dist(b)));
    v.truncate(max.min(MAX_POINT_LIGHTS));
    v
}

/// Where the sun's shadow map looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowMap {
    /// World to the light's clip space (orthographic).
    pub view_proj: Mat4,
    /// World size of one texel for a map of `size` texels, inches.
    pub texel_world: f32,
    /// Depth of the map's volume along the light, inches.
    pub depth_range: f32,
}

/// Orthographic shadow map fitted to the box `min..max`, looking along
/// `-to_sun` for a map of `size` texels per side.
///
/// The map covers exactly the box (plus a one texel margin) so no resolution
/// is wasted; every corner of the box lands inside the clip cube.
pub fn shadow_map(to_sun: Vec3, min: Vec3, max: Vec3, size: i32) -> ShadowMap {
    let dir = {
        let d = math::normalize(to_sun);
        if math::length(d) < 0.5 {
            [0.0, 1.0, 0.0]
        } else {
            d
        }
    };
    let centre = math::scale(math::add(min, max), 0.5);
    let up = if dir[1].abs() > 0.98 {
        [0.0, 0.0, -1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let radius = math::length(math::sub(max, min)) * 0.5;
    let view = math::look_at(
        math::add(centre, math::scale(dir, radius.max(1.0))),
        centre,
        up,
    );
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for i in 0..8 {
        let corner = [
            if i & 1 == 0 { min[0] } else { max[0] },
            if i & 2 == 0 { min[1] } else { max[1] },
            if i & 4 == 0 { min[2] } else { max[2] },
        ];
        let p = math::transform_point(&view, corner);
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let extent = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1.0);
    let texel = extent / size.max(1) as f32;
    let pad = texel * 2.0;
    // View space looks down -Z: the nearest corner has the largest z.
    let proj = math::ortho(
        lo[0] - pad,
        hi[0] + pad,
        lo[1] - pad,
        hi[1] + pad,
        -hi[2] - pad,
        -lo[2] + pad,
    );
    ShadowMap {
        view_proj: math::mul(&proj, &view),
        texel_world: texel,
        depth_range: (hi[2] - lo[2]) + 2.0 * pad,
    }
}

/// Hemisphere sample kernel for ambient occlusion: `n` offsets in tangent
/// space (+Z up out of the surface), all inside the unit hemisphere and
/// denser near the centre. The same `seed` always gives the same kernel.
pub fn ssao_kernel(n: usize, seed: u32) -> Vec<Vec3> {
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9);
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state >> 8) as f32 / 16_777_216.0
    };
    (0..n)
        .map(|i| {
            // Cosine-ish hemisphere direction.
            let (u, v) = (next(), next());
            let r = u.sqrt();
            let phi = std::f32::consts::TAU * v;
            let dir = [r * phi.cos(), r * phi.sin(), (1.0 - u).max(0.02).sqrt()];
            let t = (i as f32 + 0.5) / n.max(1) as f32;
            // Accelerating interpolation: most samples close to the origin.
            let scale = 0.1 + 0.9 * t * t;
            math::scale(math::normalize(dir), scale)
        })
        .collect()
}

/// The tone curve of the Standard shader: linear below the knee, then a soft
/// exponential shoulder that approaches 1.
pub fn tone_map(x: f32) -> f32 {
    const KNEE: f32 = 0.6;
    let x = x.max(0.0);
    if x <= KNEE {
        x
    } else {
        KNEE + (1.0 - KNEE) * (1.0 - (-(x - KNEE) / (1.0 - KNEE)).exp())
    }
}

/// Sky-gradient colours from the view's horizon (background) colour:
/// `(zenith, horizon, ground)`.
pub fn sky_colors(horizon: [f32; 4]) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let h = [horizon[0], horizon[1], horizon[2]];
    let zenith = [h[0] * 0.62, h[1] * 0.76, (h[2] * 1.02).min(1.0)];
    let earth = [0.46, 0.44, 0.39];
    let ground = [
        h[0] * 0.35 + earth[0] * 0.65,
        h[1] * 0.35 + earth[1] * 0.65,
        h[2] * 0.35 + earth[2] * 0.65,
    ];
    (zenith, h, ground)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_map_fits_the_scene_bounds() {
        let (min, max) = ([-120.0, 0.0, -600.0], [1200.0, 330.0, 90.0]);
        for sun in [
            [-0.4, 0.8, 0.5],
            [0.0, 1.0, 0.0],
            [0.9, 0.1, -0.3],
            [1.0, 0.0, 0.0],
        ] {
            let sm = shadow_map(sun, min, max, 2048);
            for i in 0..8 {
                let c = [
                    if i & 1 == 0 { min[0] } else { max[0] },
                    if i & 2 == 0 { min[1] } else { max[1] },
                    if i & 4 == 0 { min[2] } else { max[2] },
                ];
                let p = math::transform_point(&sm.view_proj, c);
                for (k, v) in p.iter().enumerate() {
                    assert!(v.abs() <= 1.0, "sun {sun:?} corner {c:?} axis {k}: {v}");
                }
            }
            // Points well outside the box miss the map.
            let far = math::transform_point(&sm.view_proj, [5000.0, 100.0, 0.0]);
            assert!(far.iter().any(|v| v.abs() > 1.0), "{far:?}");
            assert!(sm.texel_world > 0.0 && sm.depth_range > 0.0);
        }
    }

    #[test]
    fn shadow_map_is_tight_not_wasteful() {
        // A flat 100 x 100 slab lit from straight above fills the map.
        let sm = shadow_map([0.0, 1.0, 0.0], [0.0, 0.0, 0.0], [100.0, 0.0, 100.0], 1024);
        let a = math::transform_point(&sm.view_proj, [0.0, 0.0, 0.0]);
        let b = math::transform_point(&sm.view_proj, [100.0, 0.0, 100.0]);
        let span = (a[0] - b[0]).abs().max((a[1] - b[1]).abs());
        assert!(span > 1.9, "the slab covers the map ({span})");
        assert!((sm.texel_world - 100.0 / 1024.0).abs() < 0.01);
    }

    #[test]
    fn a_shadow_caster_lands_in_front_of_the_floor_it_shades() {
        let sm = shadow_map(
            [-0.4, 0.8, 0.5],
            [0.0, 0.0, 0.0],
            [240.0, 100.0, 240.0],
            2048,
        );
        let occluder = [120.0, 90.0, 120.0];
        // The floor point on the sun's ray through the occluder.
        let d = math::normalize([-0.4, 0.8, 0.5]);
        let t = occluder[1] / d[1];
        let floor_pt = math::sub(occluder, math::scale(d, t));
        let (po, pf) = (
            math::transform_point(&sm.view_proj, occluder),
            math::transform_point(&sm.view_proj, floor_pt),
        );
        assert!((po[0] - pf[0]).abs() < 1e-3 && (po[1] - pf[1]).abs() < 1e-3);
        assert!(
            po[2] < pf[2],
            "occluder is nearer the light: {} < {}",
            po[2],
            pf[2]
        );
    }

    #[test]
    fn ssao_kernel_samples_a_denser_unit_hemisphere() {
        for n in [8, 12, 16] {
            let k = ssao_kernel(n, 7);
            assert_eq!(k.len(), n);
            for s in &k {
                let len = math::length(*s);
                assert!(len > 0.05 && len <= 1.0 + 1e-5, "{len}");
                assert!(s[2] > 0.0, "above the surface: {s:?}");
            }
            let near = k.iter().filter(|s| math::length(**s) < 0.5).count();
            assert!(near * 2 >= n, "{near} of {n} close to the origin");
        }
        assert_eq!(ssao_kernel(12, 3), ssao_kernel(12, 3));
        assert_ne!(ssao_kernel(12, 3), ssao_kernel(12, 4));
    }

    #[test]
    fn tone_mapping_is_monotonic_bounded_and_linear_in_the_shadows() {
        let mut last = -1.0;
        for i in 0..=2000 {
            let x = i as f32 * 0.005;
            let y = tone_map(x);
            assert!(y >= last, "monotonic at {x}");
            assert!(
                (0.0..1.0).contains(&y) || (y - 1.0).abs() < 1e-3,
                "{x} -> {y}"
            );
            last = y;
        }
        assert_eq!(tone_map(0.3), 0.3);
        assert_eq!(tone_map(-1.0), 0.0);
        // Continuous across the knee.
        assert!((tone_map(0.6001) - tone_map(0.5999)).abs() < 1e-3);
        // Highlights compress: 1.1 maps below 1.
        assert!(tone_map(1.1) < 0.95 && tone_map(1.1) > tone_map(0.9));
    }

    #[test]
    fn settings_round_trip_and_old_text_loads() {
        let s = ViewSettings {
            shadows: false,
            ambient_occlusion: true,
            quality: Quality::High,
            exposure: 1.5,
        };
        assert_eq!(ViewSettings::from_text(&s.to_text()), s);
        let d = ViewSettings::default();
        assert_eq!(ViewSettings::from_text(&d.to_text()), d);
        let partial = ViewSettings::from_text("quality=low bogus=1 shadows=0");
        assert_eq!(partial.quality, Quality::Low);
        assert!(!partial.shadows && partial.ambient_occlusion);
        assert_eq!(ViewSettings::from_text(""), d);
    }

    #[test]
    fn quality_presets_scale_up() {
        let sizes: Vec<i32> = Quality::ALL.iter().map(|q| q.shadow_size()).collect();
        assert!(sizes.windows(2).all(|w| w[0] < w[1]));
        let ao: Vec<usize> = Quality::ALL.iter().map(|q| q.ao_samples()).collect();
        assert!(ao.windows(2).all(|w| w[0] < w[1]) && ao[2] <= MAX_AO_SAMPLES);
        assert!(!Quality::Low.fxaa() && Quality::High.fxaa());
    }

    #[test]
    fn nearest_lights_are_sorted_capped_and_energetic() {
        let l = |x: f32, e: f32| ViewLight {
            position: [x, 80.0, 0.0],
            color: [e; 3],
        };
        let lights: Vec<ViewLight> = (0..12).map(|i| l(i as f32 * 100.0, 1.0)).collect();
        let near = nearest_lights(&lights, [450.0, 60.0, 0.0], MAX_POINT_LIGHTS);
        assert_eq!(near.len(), MAX_POINT_LIGHTS);
        assert!(near[0].position[0] == 400.0 || near[0].position[0] == 500.0);
        assert!(!near
            .iter()
            .any(|n| n.position[0] > 800.0 || n.position[0] < 100.0));
        let dead = nearest_lights(&[l(0.0, 0.0)], [0.0; 3], 8);
        assert!(dead.is_empty());
    }

    #[test]
    fn every_look_has_distinct_codes_and_technical_draws_edges() {
        let looks = [
            Look::Standard,
            Look::Physical,
            Look::Clay,
            Look::GlassHouse,
            Look::Watercolor,
            Look::Technical,
            Look::Duotone,
            Look::Flat,
        ];
        let mut codes: Vec<i32> = looks.iter().map(|l| l.params().code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), looks.len());
        assert!(Look::Technical.params().edge_lines > 0.9);
        assert!(Look::Watercolor.params().wash > 0.0);
        assert!(Look::Standard.params().sky && !Look::Clay.params().sky);
        assert!(!Look::Flat.params().shadows);
    }

    #[test]
    fn sky_colours_darken_toward_the_zenith() {
        let (z, h, g) = sky_colors([0.85, 0.89, 0.94, 1.0]);
        assert!(z[0] < h[0] && z[1] < h[1]);
        assert!(g.iter().all(|c| (0.0..=1.0).contains(c)));
    }
}
