//! The path tracer: radiance estimation for one camera ray.

use crate::albedo::{self, TexBinding};
use crate::bvh::Bvh;
use crate::camera::{Camera, Lens};
use crate::lighting::{AreaData, AreaLight, Environment, LightData, PointLight, Sky, SunData};
use crate::rng::Rng;
use crate::settings::{RenderSettings, Technique};
use crate::shading::{reflect, sample_cone, sample_cosine, sheet_reflectance, Kind, Surface};
use crate::vec3::V3;
use plan_materials::textures::TextureStore;

/// Ray-origin offset along the surface normal, inches.
const RAY_EPS: f32 = 0.02;
/// Paths beyond this many pane/clear crossings are abandoned.
const MAX_CROSSINGS: u32 = 32;
/// Per-contribution clamp for indirect light (suppresses fireflies).
const FIREFLY_MAX: f32 = 10.0;
/// Russian roulette starts after this many bounces.
const ROULETTE_DEPTH: u32 = 2;

/// First-hit guide values for one pixel.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Guide {
    pub albedo: V3,
    pub normal: V3,
    pub depth: f32,
}

/// Distance along the image-centre ray to the first opaque surface.
fn centre_distance(bvh: &Bvh, cam: &Camera, surfaces: &[Surface]) -> Option<f32> {
    let pinhole = Camera {
        aperture: 0.0,
        ..*cam
    };
    let lens = Lens::new(&pinhole, 2, 2);
    let (mut o, d) = lens.centre_ray(1.0, 1.0);
    let mut travelled = 0.0;
    for _ in 0..MAX_CROSSINGS {
        let hit = bvh.closest(o, d, f32::INFINITY)?;
        travelled += hit.t;
        let tri = &bvh.tris[hit.tri];
        if surfaces[tri.material as usize].kind == Kind::Opaque {
            return Some(travelled);
        }
        o += d * (hit.t + RAY_EPS);
    }
    None
}

/// Everything needed to shade rays for one render.
pub(crate) struct Frame<'a> {
    bvh: &'a Bvh,
    /// One entry per material, then one per recoloured `(material, colour)`.
    surfaces: Vec<Surface>,
    textures: Vec<Option<TexBinding>>,
    sky: Sky,
    sun: Option<SunData>,
    lights: Vec<LightData>,
    areas: Vec<AreaData>,
    next_event: bool,
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
        (lights, areas): (&[PointLight], &[AreaLight]),
        settings: &RenderSettings,
        (store, custom): (&TextureStore, &[crate::shading::Custom]),
    ) -> Frame<'a> {
        let (width, height) = (settings.width.max(1), settings.height.max(1));
        let surfaces = Surface::table_with(settings.technique, custom);
        // Depth of field with no focus distance focuses on whatever is at the
        // image centre.
        let mut cam = *cam;
        if cam.aperture > 0.0 && cam.focus_dist <= 0.0 {
            cam.focus_dist = centre_distance(bvh, &cam, &surfaces).unwrap_or(0.0);
        }
        let cam = &cam;
        Frame {
            bvh,
            textures: albedo::table(store, settings.textures, settings.technique, &surfaces),
            surfaces,
            sky: Sky::new(env),
            sun: env.sun.as_ref().map(SunData::new),
            lights: lights.iter().map(LightData::new).collect(),
            areas: areas.iter().filter_map(AreaData::new).collect(),
            next_event: settings.next_event,
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

    /// Albedo, shading normal and distance of the first opaque surface seen
    /// through pixel `(x, y)`'s centre (the denoiser's guide buffers). Glass
    /// and clear surfaces are looked through. Sky pixels have zero albedo and
    /// normal and an infinite distance.
    pub fn guide(&self, x: u32, y: u32) -> Guide {
        let (mut o, d) = self.lens.centre_ray(x as f32 + 0.5, y as f32 + 0.5);
        let mut travelled = 0.0;
        for _ in 0..MAX_CROSSINGS {
            let Some(hit) = self.bvh.closest(o, d, f32::INFINITY) else {
                break;
            };
            let tri = &self.bvh.tris[hit.tri];
            let surface = &self.surfaces[tri.material as usize];
            let p = o + d * hit.t;
            travelled += hit.t;
            if surface.kind != Kind::Opaque {
                o = p + d * RAY_EPS;
                continue;
            }
            let ng = tri.normal();
            let n = if ng.dot(d) > 0.0 { -ng } else { ng };
            let albedo = match &self.textures[tri.material as usize] {
                Some(binding) => binding.albedo(p, ng),
                None => surface.albedo,
            };
            return Guide {
                albedo,
                normal: n,
                depth: travelled,
            };
        }
        Guide {
            albedo: V3::ZERO,
            normal: V3::ZERO,
            depth: f32::INFINITY,
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
            let found = self.bvh.closest(o, d, f32::INFINITY);
            let reach = found.as_ref().map_or(f32::INFINITY, |h| h.t);
            sum += self.emitted(o, d, reach, throughput, (bounce, specular));
            let Some(hit) = found else {
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
                    let textured;
                    let surface = match &self.textures[tri.material as usize] {
                        Some(binding) => {
                            textured = Surface {
                                albedo: binding.albedo(p, ng),
                                ..*surface
                            };
                            &textured
                        }
                        None => surface,
                    };
                    let direct = self.direct(p, n, wo, surface, rng);
                    sum += self.contribution(throughput * (direct + surface.emission), bounce);
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

    /// Light from the area panels seen along the segment `o + t d`, `t < reach`.
    ///
    /// With next-event estimation on, panels are only counted when seen
    /// directly (camera rays and mirror/glass chains); bounced rays leave the
    /// panels to [`Frame::direct`] so nothing is counted twice.
    fn emitted(
        &self,
        o: V3,
        d: V3,
        reach: f32,
        throughput: V3,
        (bounce, specular): (u32, bool),
    ) -> V3 {
        if self.areas.is_empty() || (self.next_event && !specular) {
            return V3::ZERO;
        }
        let mut sum = V3::ZERO;
        for a in &self.areas {
            if a.hit(o, d, reach).is_some() {
                sum += self.contribution(throughput * a.radiance, bounce);
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
        if self.next_event {
            for a in &self.areas {
                let to = a.point(rng.next_f32(), rng.next_f32()) - origin;
                let d2 = to.length_sq().max(1.0);
                let dist = d2.sqrt();
                let wi = to / dist;
                let (cos_i, cos_l) = (n.dot(wi), a.emit_cos(wi));
                if cos_i > 0.0 && cos_l > 0.0 {
                    let (f, _) = surface.eval(n, wo, wi);
                    if f.max_comp() > 0.0 {
                        let t = self.transmittance(origin, wi, dist - RAY_EPS);
                        sum += f * t * a.radiance * (cos_i * cos_l * a.area / d2);
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bvh::Tri;
    use plan_3d::Material;

    #[test]
    fn centre_distance_finds_the_surface_ahead_and_skips_glass() {
        let wall = |z: f32, m: Material| {
            let (a, b, c, d) = (
                V3::new(-50.0, -50.0, z),
                V3::new(50.0, -50.0, z),
                V3::new(50.0, 50.0, z),
                V3::new(-50.0, 50.0, z),
            );
            [
                Tri::new(a, b, c, m.index() as u32).unwrap(),
                Tri::new(a, c, d, m.index() as u32).unwrap(),
            ]
        };
        let mut tris = wall(-100.0, Material::WallInterior).to_vec();
        tris.extend(wall(-40.0, Material::WindowGlass));
        let bvh = Bvh::build(tris);
        let cam = Camera {
            eye: [0.0; 3],
            target: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 50.0,
            aperture: 1.0,
            focus_dist: 0.0,
        };
        let surfaces = Surface::table(Technique::PhysicallyBased);
        let d = centre_distance(&bvh, &cam, &surfaces).unwrap();
        assert!((d - 100.0).abs() < 0.5, "{d}");
    }
}
