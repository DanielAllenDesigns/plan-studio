//! The extra maps of a PBR material package at render time.
//!
//! [`PbrSource`] says which files a painted material wants beyond its albedo;
//! [`PbrSet::load`] decodes them into the two data textures the GL view
//! uploads (a tangent-space normal map, and the packed `ORM` map: R ambient
//! occlusion, G roughness, B metallic) plus an opacity cut-out, and the ray
//! tracer samples the same set. Both read the table [`register`] fills when
//! the app paints a scene, keyed like `plan_3d::surface` by object id, scene
//! material and exact colour.
//!
//! Surfaces are mapped planar (`textures::planar_uv`), so the tangent frame
//! follows from the face normal alone ([`tangent_frame`]); no per-vertex
//! tangents are stored. Normal maps use the OpenGL convention (green points up
//! the image); DirectX maps are flipped when they are loaded.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, PoisonError, RwLock};

use plan_3d::Material;
use plan_core::Id;
use plan_library::image::{self, Rgba8Image};

use crate::material::MaterialDef;
use crate::package::MapKind;
use crate::textures::{Projection, FLAT_EPSILON};
use crate::xform::transform_rgba;

/// `PbrSet::flags`: a normal map is present.
pub const PBR_NORMAL: u32 = 1;
/// `PbrSet::flags`: the ORM map's green channel is a measured roughness.
pub const PBR_ROUGH: u32 = 2;
/// `PbrSet::flags`: the ORM map's blue channel is a measured metalness.
pub const PBR_METAL: u32 = 4;
/// `PbrSet::flags`: the ORM map's red channel is ambient occlusion.
pub const PBR_AO: u32 = 8;
/// `PbrSet::flags`: an opacity map cuts the surface out (alpha below one half).
pub const PBR_CUTOUT: u32 = 16;

/// Which maps a painted material wants and how its texture is placed.
#[derive(Debug, Clone)]
pub struct PbrSource {
    /// The material (paths, switches, tile size, offset, angle, bump).
    pub def: MaterialDef,
}

impl PbrSource {
    /// The source of `def`, or `None` when it has no enabled map beyond its
    /// albedo (then it renders as any textured material does).
    pub fn from_def(def: &MaterialDef) -> Option<PbrSource> {
        let any = [
            MapKind::Normal,
            MapKind::Roughness,
            MapKind::Metallic,
            MapKind::Height,
            MapKind::Ao,
            MapKind::Opacity,
        ]
        .into_iter()
        .any(|k| def.map_enabled(k));
        any.then(|| PbrSource { def: def.clone() })
    }

    /// Identifies the files and settings: equal keys decode to equal sets.
    pub fn key(&self) -> u64 {
        let d = &self.def;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for k in MapKind::ALL {
            k.key().hash(&mut h);
            d.map_enabled(k).hash(&mut h);
            d.map_path(k).hash(&mut h);
        }
        d.normal_flip_y.hash(&mut h);
        d.bump.to_bits().hash(&mut h);
        d.texture_scale_in.0.to_bits().hash(&mut h);
        d.texture_scale_in.1.to_bits().hash(&mut h);
        d.texture_offset_in.0.to_bits().hash(&mut h);
        d.texture_offset_in.1.to_bits().hash(&mut h);
        d.texture_angle_deg.to_bits().hash(&mut h);
        h.finish()
    }

    /// Inches `[across, down]` one repeat covers.
    pub fn scale_in(&self) -> [f32; 2] {
        [
            self.def.texture_scale_in.0 as f32,
            self.def.texture_scale_in.1 as f32,
        ]
    }

    /// Strength the normal map is applied with: 0 flat .. 2 doubled, 1 as
    /// authored (the Properties tab's bump slider at 0.5).
    pub fn normal_strength(&self) -> f32 {
        (self.def.bump * 2.0).clamp(0.0, 2.0)
    }
}

/// A square-or-not 8-bit data map (not colour: no sRGB curve applies).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, top row first.
    pub rgba: Vec<u8>,
}

impl MapImage {
    /// Bilinear sample at `(u, v)` in repeats (wraps), channel values `0..=1`.
    pub fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let (w, h) = (self.width as usize, self.height as usize);
        if w == 0 || h == 0 {
            return [0.0; 4];
        }
        let fx = u.rem_euclid(1.0) * w as f32 - 0.5;
        let fy = v.rem_euclid(1.0) * h as f32 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let at = |x: i64, y: i64| -> [f32; 4] {
            let (x, y) = (
                x.rem_euclid(w as i64) as usize,
                y.rem_euclid(h as i64) as usize,
            );
            let o = (y * w + x) * 4;
            [
                f32::from(self.rgba[o]),
                f32::from(self.rgba[o + 1]),
                f32::from(self.rgba[o + 2]),
                f32::from(self.rgba[o + 3]),
            ]
        };
        let (a, b, c, d) = (
            at(x0, y0),
            at(x0 + 1, y0),
            at(x0, y0 + 1),
            at(x0 + 1, y0 + 1),
        );
        let mut out = [0.0; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bot = c[k] + (d[k] - c[k]) * tx;
            out[k] = (top + (bot - top) * ty) / 255.0;
        }
        out
    }
}

/// What one sample of a [`PbrSet`] says about the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PbrSample {
    /// Tangent-space normal (x right, y up the image, z out), unit length.
    pub normal: Option<[f32; 3]>,
    pub roughness: Option<f32>,
    pub metallic: Option<f32>,
    /// Ambient occlusion, 1 = open.
    pub ao: Option<f32>,
    /// Opacity, 1 = solid.
    pub opacity: Option<f32>,
}

/// The decoded extra maps of one material.
#[derive(Debug, Clone, Default)]
pub struct PbrSet {
    /// Tangent-space normals (RGB = `n * 0.5 + 0.5`), strength applied.
    pub normal: Option<MapImage>,
    /// R ambient occlusion, G roughness, B metallic, A 255.
    pub orm: Option<MapImage>,
    /// Opacity in the red channel.
    pub opacity: Option<MapImage>,
    /// The albedo, sRGB with offset / angle / blend applied (only when asked
    /// for: the GL view gets its albedo through the texture store).
    pub albedo: Option<MapImage>,
    /// `PBR_*` bits of what the maps hold.
    pub flags: u32,
}

fn read_map(path: &str, max_side: u32) -> Option<Rgba8Image> {
    let img = image::decode_file(Path::new(path)).ok()?;
    Some(img.downscaled(max_side))
}

fn gray(img: &Rgba8Image) -> Vec<u8> {
    img.rgba.as_chunks::<4>().0.iter().map(|p| p[0]).collect()
}

/// Turns a height map into a tangent-space normal map. `strength` scales the
/// slope (the central difference of the 0..1 heights is multiplied by it).
pub fn normal_from_height(height: &Rgba8Image, strength: f32) -> Rgba8Image {
    let (w, h) = (height.width as usize, height.height as usize);
    let g = gray(height);
    let at = |x: i64, y: i64| -> f32 {
        let (x, y) = (
            x.rem_euclid(w as i64) as usize,
            y.rem_euclid(h as i64) as usize,
        );
        f32::from(g[y * w + x]) / 255.0
    };
    let mut rgba = vec![255u8; w * h * 4];
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let dx = at(x + 1, y) - at(x - 1, y);
            // Rows run down the image; the map's green points up it.
            let dy_down = at(x, y + 1) - at(x, y - 1);
            let n = norm3([-dx * strength, dy_down * strength, 1.0]);
            let o = (y as usize * w + x as usize) * 4;
            for k in 0..3 {
                rgba[o + k] = ((n[k] * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Rgba8Image {
        width: w as u32,
        height: h as u32,
        rgba,
    }
}

fn norm3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-9 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

/// `img` with the normals' strength applied (`1` leaves them) and the green
/// channel flipped for a DirectX map.
fn adjust_normals(img: &mut Rgba8Image, strength: f32, flip_y: bool) {
    if (strength - 1.0).abs() < 1e-3 && !flip_y {
        return;
    }
    for p in img.rgba.as_chunks_mut::<4>().0 {
        let mut n = [
            f32::from(p[0]) / 127.5 - 1.0,
            f32::from(p[1]) / 127.5 - 1.0,
            f32::from(p[2]) / 127.5 - 1.0,
        ];
        if flip_y {
            n[1] = -n[1];
        }
        n[0] *= strength;
        n[1] *= strength;
        let n = norm3(n);
        for k in 0..3 {
            p[k] = ((n[k] * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// The transform that carries offset and angle only (never the blend tint).
fn placement(def: &MaterialDef) -> MaterialDef {
    let mut d = MaterialDef::new("", &[], [0, 0, 0]);
    d.texture_offset_in = def.texture_offset_in;
    d.texture_angle_deg = def.texture_angle_deg;
    d
}

/// Rotates the xy of every normal by `angle` degrees (the texture turned on
/// the surface; its vectors must turn with it).
fn turn_normals(img: &mut Rgba8Image, angle_deg: f64) {
    let a = angle_deg.rem_euclid(360.0);
    if a < 1e-9 {
        return;
    }
    let (s, c) = (a as f32).to_radians().sin_cos();
    for p in img.rgba.as_chunks_mut::<4>().0 {
        let (x, y) = (f32::from(p[0]) / 127.5 - 1.0, f32::from(p[1]) / 127.5 - 1.0);
        // The bitmap turned counter-clockwise on the surface, y up the image.
        let (rx, ry) = (x * c + y * s, -x * s + y * c);
        p[0] = ((rx * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
        p[1] = ((ry * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
    }
}

fn placed(img: Rgba8Image, def: &MaterialDef, scale_in: [f32; 2]) -> Rgba8Image {
    let rgba = transform_rgba(&img.rgba, img.width, img.height, scale_in, &placement(def));
    Rgba8Image {
        width: img.width,
        height: img.height,
        rgba,
    }
}

impl PbrSet {
    /// Decodes the maps of `src`, every one at most `max_side` pixels on its
    /// longer side. `with_albedo` also keeps the albedo (the ray tracer).
    pub fn load(src: &PbrSource, max_side: u32, with_albedo: bool) -> PbrSet {
        let def = &src.def;
        let scale = src.scale_in();
        let load = |kind: MapKind| -> Option<Rgba8Image> {
            if !def.map_enabled(kind) {
                return None;
            }
            read_map(def.map_path(kind)?, max_side).map(|i| placed(i, def, scale))
        };
        let mut set = PbrSet::default();
        // Normals: the map, else the height map turned into one.
        let strength = src.normal_strength();
        if let Some(mut n) = load(MapKind::Normal) {
            adjust_normals(&mut n, strength, def.normal_flip_y);
            turn_normals(&mut n, def.texture_angle_deg);
            set.normal = Some(MapImage {
                width: n.width,
                height: n.height,
                rgba: n.rgba,
            });
        } else if def.map_enabled(MapKind::Height) {
            if let Some(h) = read_map(def.map_path(MapKind::Height).unwrap_or(""), max_side) {
                let mut n = normal_from_height(&h, 6.0 * strength.max(0.0));
                if strength > 0.0 {
                    n = placed(n, def, scale);
                    turn_normals(&mut n, def.texture_angle_deg);
                    set.normal = Some(MapImage {
                        width: n.width,
                        height: n.height,
                        rgba: n.rgba,
                    });
                }
            }
        }
        if set.normal.is_some() {
            set.flags |= PBR_NORMAL;
        }
        // ORM.
        let rough = load(MapKind::Roughness);
        let metal = load(MapKind::Metallic);
        let ao = load(MapKind::Ao);
        let side = [&rough, &metal, &ao]
            .into_iter()
            .flatten()
            .map(|i| (i.width, i.height))
            .max_by_key(|(w, h)| u64::from(*w) * u64::from(*h));
        if let Some((w, h)) = side {
            let fit = |i: &Option<Rgba8Image>| {
                i.as_ref().map(|i| {
                    if (i.width, i.height) == (w, h) {
                        gray(i)
                    } else {
                        gray(&i.resized_box(w, h))
                    }
                })
            };
            let (r, m, o) = (fit(&rough), fit(&metal), fit(&ao));
            let mut rgba = vec![255u8; w as usize * h as usize * 4];
            for (i, px) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                px[0] = o.as_ref().map_or(255, |v| v[i]);
                px[1] = r.as_ref().map_or(255, |v| v[i]);
                px[2] = m.as_ref().map_or(0, |v| v[i]);
            }
            set.orm = Some(MapImage {
                width: w,
                height: h,
                rgba,
            });
            if rough.is_some() {
                set.flags |= PBR_ROUGH;
            }
            if metal.is_some() {
                set.flags |= PBR_METAL;
            }
            if ao.is_some() {
                set.flags |= PBR_AO;
            }
        }
        // Opacity.
        if let Some(o) = load(MapKind::Opacity) {
            set.opacity = Some(MapImage {
                width: o.width,
                height: o.height,
                rgba: o.rgba,
            });
            set.flags |= PBR_CUTOUT;
        }
        if with_albedo {
            if let Some(path) = def.texture_path.as_deref() {
                if let Some(i) = read_map(path, max_side) {
                    let rgba = transform_rgba(&i.rgba, i.width, i.height, scale, def);
                    set.albedo = Some(MapImage {
                        width: i.width,
                        height: i.height,
                        rgba,
                    });
                }
            }
        }
        set
    }

    /// The maps at `(u, v)` (repeats).
    pub fn sample(&self, u: f32, v: f32) -> PbrSample {
        let normal = self.normal.as_ref().map(|m| {
            let p = m.sample(u, v);
            norm3([p[0] * 2.0 - 1.0, p[1] * 2.0 - 1.0, p[2] * 2.0 - 1.0])
        });
        let orm = self.orm.as_ref().map(|m| m.sample(u, v));
        PbrSample {
            normal,
            roughness: orm.filter(|_| self.flags & PBR_ROUGH != 0).map(|p| p[1]),
            metallic: orm.filter(|_| self.flags & PBR_METAL != 0).map(|p| p[2]),
            ao: orm.filter(|_| self.flags & PBR_AO != 0).map(|p| p[0]),
            opacity: self.opacity.as_ref().map(|m| m.sample(u, v)[0]),
        }
    }

    /// The albedo at `(u, v)` in linear light, when it was loaded.
    pub fn albedo_linear(&self, u: f32, v: f32) -> Option<[f32; 3]> {
        let a = self.albedo.as_ref()?.sample(u, v);
        let lin = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        Some([lin(a[0]), lin(a[1]), lin(a[2])])
    }

    /// Folds the opacity map into the alpha of `rgba` (`w` x `h`, straight
    /// RGBA8) so a texture-sampled alpha cuts the surface out. No-op without
    /// an opacity map.
    pub fn fold_opacity(&self, rgba: &mut [u8], w: u32, h: u32) {
        let Some(o) = &self.opacity else { return };
        if rgba.len() != w as usize * h as usize * 4 || w == 0 || h == 0 {
            return;
        }
        for y in 0..h as usize {
            for x in 0..w as usize {
                let a = o.sample((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32)[0];
                rgba[(y * w as usize + x) * 4 + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// Bytes of pixel data.
    pub fn byte_len(&self) -> usize {
        [&self.normal, &self.orm, &self.opacity, &self.albedo]
            .into_iter()
            .flatten()
            .map(|m| m.rgba.len())
            .sum()
    }
}

// ---- tangent frame -----------------------------------------------------------

/// The surface tangent frame matching `textures::planar_uv`: `(right, up)`,
/// the world directions in which the texture's `u` grows and its image rises,
/// for a face with `normal` (any length). With `right x up = normal` the frame
/// is right handed. The GLSL in plan-view3d's shader is a line-for-line copy.
pub fn tangent_frame(normal: [f32; 3], proj: Projection) -> ([f32; 3], [f32; 3]) {
    let n = norm3(normal);
    let n = if normal.iter().all(|c| c.abs() < 1e-12) {
        [0.0, 1.0, 0.0]
    } else {
        n
    };
    let hl = (n[0] * n[0] + n[2] * n[2]).sqrt();
    if proj == Projection::GroundXz || hl < FLAT_EPSILON {
        // u runs along +x and v along +z: the image rises toward -z.
        return ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
    }
    let up = [-n[0] * n[1] / hl, hl, -n[2] * n[1] / hl];
    let right = [
        up[1] * n[2] - up[2] * n[1],
        up[2] * n[0] - up[0] * n[2],
        up[0] * n[1] - up[1] * n[0],
    ];
    (right, up)
}

/// The shading normal after a tangent-space `ts` normal perturbs the face
/// normal `n` (the unit normal facing the viewer) in `frame`.
pub fn perturb_normal(n: [f32; 3], frame: ([f32; 3], [f32; 3]), ts: [f32; 3]) -> [f32; 3] {
    let (r, u) = frame;
    norm3([
        r[0] * ts[0] + u[0] * ts[1] + n[0] * ts[2],
        r[1] * ts[0] + u[1] * ts[1] + n[1] * ts[2],
        r[2] * ts[0] + u[2] * ts[1] + n[2] * ts[2],
    ])
}

// ---- the table of painted meshes ----------------------------------------------

type Key = (Id, usize, [u8; 3]);

#[derive(Default)]
struct Table {
    by_mesh: HashMap<Key, Arc<PbrSource>>,
    by_key: HashMap<u64, Arc<PbrSource>>,
}

fn table() -> &'static RwLock<Table> {
    static T: OnceLock<RwLock<Table>> = OnceLock::new();
    T.get_or_init(|| RwLock::new(Table::default()))
}

/// Sets (`Some`) or removes (`None`) the maps of meshes of `object` drawn as
/// `material` in exactly `color`.
pub fn register(object: Id, material: Material, color: [u8; 3], src: Option<Arc<PbrSource>>) {
    let key = (object, material.index(), color);
    let mut t = table().write().unwrap_or_else(PoisonError::into_inner);
    match src {
        Some(s) => {
            t.by_key.insert(s.key(), Arc::clone(&s));
            t.by_mesh.insert(key, s);
        }
        None => {
            t.by_mesh.remove(&key);
        }
    }
}

/// The maps registered for a painted mesh, if any.
pub fn lookup(
    object: Option<Id>,
    material: Material,
    color: Option<[u8; 3]>,
) -> Option<Arc<PbrSource>> {
    let (object, color) = (object?, color?);
    let t = table().read().unwrap_or_else(PoisonError::into_inner);
    t.by_mesh.get(&(object, material.index(), color)).cloned()
}

/// A registered source by its [`PbrSource::key`].
pub fn source_by_key(key: u64) -> Option<Arc<PbrSource>> {
    let t = table().read().unwrap_or_else(PoisonError::into_inner);
    t.by_key.get(&key).cloned()
}

/// Most decoded sets kept by [`load_cached`].
const CACHE_SETS: usize = 24;

type CacheKey = (u64, u32, bool);
type CacheEntries = Vec<(CacheKey, Arc<PbrSet>)>;

/// [`PbrSet::load`] through a small process-wide cache.
pub fn load_cached(src: &PbrSource, max_side: u32, with_albedo: bool) -> Arc<PbrSet> {
    static CACHE: OnceLock<Mutex<CacheEntries>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let key = (src.key(), max_side, with_albedo);
    {
        let c = cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, s)) = c.iter().find(|(k, _)| *k == key) {
            return Arc::clone(s);
        }
    }
    let set = Arc::new(PbrSet::load(src, max_side, with_albedo));
    let mut c = cache.lock().unwrap_or_else(PoisonError::into_inner);
    if c.len() >= CACHE_SETS {
        c.remove(0);
    }
    c.push((key, Arc::clone(&set)));
    set
}

#[cfg(test)]
mod tests;
