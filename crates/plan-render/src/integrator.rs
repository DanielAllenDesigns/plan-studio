//! The path tracer: radiance estimation for one camera ray.

use crate::bvh::Bvh;
use crate::camera::{Camera, Lens};
use crate::lighting::{Environment, LightData, PointLight, Sky, SunData};
use crate::rng::Rng;
use crate::settings::{RenderSettings, Technique};
use crate::shading::{
    reflect, sample_cone, sample_cosine, sheet_reflectance, Kind, Surface, MATERIAL_COUNT,
};
use crate::vec3::V3;

/// Ray-origin offset along the surface normal, inches.
const RAY_EPS: f32 = 0.02;
/// Paths beyond this many pane/clear crossings are abandoned.
const MAX_CROSSINGS: u32 = 32;
/// Per-contribution clamp for indirect light (suppresses fireflies).
const FIREFLY_MAX: f32 = 10.0;
/// Russian roulette starts after this many bounces.
const ROULETTE_DEPTH: u32 = 2;

/// Everything needed to shade rays for one render.
pub(crate) struct Frame<'a> {
    bvh: &'a Bvh,
    surfaces: [Surface; MATERIAL_COUNT],
    sky: Sky,
    sun: Option<SunData>,
    lights: Vec<LightData>,
    technique: Technique,
    max_bounces: u32,
    ao_radius: f32,
    lens: Lens,
    width: u32,
    seed: u64,
}

impl<'a> Frame<'a> {
    pub fn new(
        bvh: &'a Bvh,
        scene_diagonal: f32,
        cam: &Camera,
        env: &Environment,
        lights: &[PointLight],
        settings: &RenderSettings,
    ) -> Frame<'a> {
        let (width, height) = (settings.width.max(1), settings.height.max(1));
        Frame {
            bvh,
            surfaces: Surface::table(settings.technique),
            sky: Sky::new(env),
            sun: env.sun.as_ref().map(SunData::new),
            lights: lights.iter().map(LightData::new).collect(),
            technique: settings.technique,
            max_bounces: settings.max_bounces,
            ao_radius: (scene_diagonal * 0.1).max(12.0),
            lens: Lens::new(cam, width, height),
            width,
            seed: settings.seed,
        }
    }

    /// One radiance sample for pixel `(x, y)`; deterministic in `(seed, x, y, sample)`.
    pub fn sample(&self, x: u32, y: u32, sample: u32) -> V3 {
        let pixel = u64::from(y) * u64::from(self.width) + u64::from(x);
        let mut rng = Rng::for_sample(self.seed, pixel, sample);
        let (jx, jy) = (rng.next_f32(), rng.next_f32());
        let (origin, dir) = self.lens.ray(x as f32 + jx, y as f32 + jy, &mut rng);
        let c = match self.technique {
            Technique::Ambient => self.ambient(origin, dir, &mut rng),
            _ => self.radiance(origin, dir, &mut rng),
        };
        if c.is_finite() {
            c
        } else {
            V3::ZERO
        }
    }

    /// Ambient occlusion: one cosine-weighted visibility probe at the first solid hit.
    fn ambient(&self, mut o: V3, d: V3, rng: &mut Rng) -> V3 {
        for _ in 0..MAX_CROSSINGS {
            let Some(hit) = self.bvh.closest(o, d, f32::INFINITY) else {
                return V3::ONE;
            };
            let tri = &self.bvh.tris[hit.tri];
            let p = o + d * hit.t;
            if self.surfaces[tri.material as usize].kind != Kind::Opaque {
                o = p + d * RAY_EPS;
                continue;
            }
            let ng = tri.normal();
            let n = if ng.dot(d) > 0.0 { -ng } else { ng };
            let wi = sample_cosine(n, rng.next_f32(), rng.next_f32());
            return self.transmittance(p + n * RAY_EPS, wi, self.ao_radius);
        }
        V3::ONE
    }

    /// Radiance along a camera ray.
    fn radiance(&self, mut o: V3, mut d: V3, rng: &mut Rng) -> V3 {
        let mut throughput = V3::ONE;
        let mut sum = V3::ZERO;
        let mut bounce = 0;
        let mut crossings = 0;
        // True while the path has only seen the camera and perfect specular
        // events; only then is the sun disc visible (otherwise NEE covers it).
        let mut specular = true;
        loop {
            let Some(hit) = self.bvh.closest(o, d, f32::INFINITY) else {
                sum += self.contribution(throughput * self.background(d, specular), bounce);
                break;
            };
            let tri = &self.bvh.tris[hit.tri];
            let surface = &self.surfaces[tri.material as usize];
            let p = o + d * hit.t;
            let ng = tri.normal();
            let n = if ng.dot(d) > 0.0 { -ng } else { ng };
            match surface.kind {
                Kind::Clear => {
                    crossings += 1;
                    o = p + d * RAY_EPS;
                }
                Kind::Glass => {
                    crossings += 1;
                    if rng.next_f32() < sheet_reflectance(-d.dot(n)) {
                        d = reflect(d, n);
                        o = p + n * RAY_EPS;
                    } else {
                        throughput *= surface.tint;
                        o = p + d * RAY_EPS;
                    }
                    specular = true;
                }
                Kind::Opaque => {
                    let wo = -d;
                    let direct = self.direct(p, n, wo, surface, rng);
                    sum += self.contribution(throughput * direct, bounce);
                    if bounce >= self.max_bounces {
                        break;
                    }
                    if bounce >= ROULETTE_DEPTH {
                        let q = throughput.max_comp().clamp(0.05, 0.95);
                        if rng.next_f32() >= q {
                            break;
                        }
                        throughput = throughput / q;
                    }
                    let Some((wi, weight)) = surface.sample(n, wo, rng) else {
                        break;
                    };
                    throughput *= weight;
                    d = wi;
                    o = p + n * RAY_EPS;
                    bounce += 1;
                    specular = false;
                }
            }
            if crossings > MAX_CROSSINGS {
                break;
            }
        }
        sum
    }

    /// Clamp fireflies on indirect contributions.
    fn contribution(&self, c: V3, bounce: u32) -> V3 {
        if bounce == 0 {
            c
        } else {
            c.min_each(FIREFLY_MAX)
        }
    }

    /// Light arriving from beyond the scene along `d`.
    fn background(&self, d: V3, specular: bool) -> V3 {
        if self.technique == Technique::Ambient {
            return V3::ONE;
        }
        let mut c = self.sky.radiance(d);
        if let (true, Some(sun)) = (specular, &self.sun) {
            if d.dot(sun.dir) >= sun.cos_max {
                c = sun.disc_radiance();
            }
        }
        c
    }

    /// Next-event estimation toward the sun and the point lights.
    fn direct(&self, p: V3, n: V3, wo: V3, surface: &Surface, rng: &mut Rng) -> V3 {
        let origin = p + n * RAY_EPS;
        let mut sum = V3::ZERO;
        if let Some(sun) = &self.sun {
            let wi = sample_cone(sun.dir, sun.cos_max, rng.next_f32(), rng.next_f32());
            let cos_i = n.dot(wi);
            if cos_i > 0.0 {
                let (f, _) = surface.eval(n, wo, wi);
                if f.max_comp() > 0.0 {
                    let t = self.transmittance(origin, wi, f32::INFINITY);
                    sum += f * t * sun.irradiance * cos_i;
                }
            }
        }
        for light in &self.lights {
            let jitter = sample_cone(V3::new(0.0, 1.0, 0.0), -1.0, rng.next_f32(), rng.next_f32());
            let to = light.pos + jitter * light.radius - origin;
            let d2 = to.length_sq().max(1e-4);
            let dist = d2.sqrt();
            let wi = to / dist;
            let cos_i = n.dot(wi);
            if cos_i > 0.0 {
                let (f, _) = surface.eval(n, wo, wi);
                if f.max_comp() > 0.0 {
                    let t = self.transmittance(origin, wi, dist - RAY_EPS);
                    sum += f * t * light.intensity * (cos_i / d2);
                }
            }
        }
        sum
    }

    /// Fraction of light surviving from `o` along `d` for at most `tmax`.
    ///
    /// Opaque surfaces block; glass attenuates by its tint and reflectance.
    fn transmittance(&self, mut o: V3, d: V3, mut tmax: f32) -> V3 {
        let mut t = V3::ONE;
        for _ in 0..MAX_CROSSINGS {
            let Some(hit) = self.bvh.closest(o, d, tmax) else {
                return t;
            };
            let tri = &self.bvh.tris[hit.tri];
            let surface = &self.surfaces[tri.material as usize];
            match surface.kind {
                Kind::Opaque => return V3::ZERO,
                Kind::Glass => {
                    let cos = tri.normal().dot(d).abs();
                    t = t * surface.tint * (1.0 - sheet_reflectance(cos));
                }
                Kind::Clear => {}
            }
            let step = hit.t + RAY_EPS;
            o += d * step;
            tmax -= step;
            if tmax <= 0.0 {
                return t;
            }
        }
        V3::ZERO
    }
}
