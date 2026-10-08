//! Texture lookup for the 3D view and the ray tracer.
//!
//! Every textured [`plan_3d::Material`] resolves to an image, in this order:
//!
//! 1. **Chief's own texture**, read at run time from Daniel's install when it
//!    is there (`~/Documents/Chief Architect Premier X18 Data/Textures`, then
//!    `/Library/Application Support/Chief Architect Premier X18/Referenced
//!    Files`; the `PLAN_STUDIO_TEXTURES` environment variable adds a folder in
//!    front). The files are licensed Chief content: they are never copied into
//!    this repository and never written anywhere. A file named like
//!    `Brick(36).jpg` carries its tile size in inches in the parentheses.
//! 2. A **procedural fallback** generated in code (brick, lap siding, stucco,
//!    shingles, wood grain, concrete, grass, water, ...), so the repo ships no
//!    Chief assets and the view still looks textured without Chief.
//!
//! [`TextureStore`] decodes lazily and caches by bytes (least recently used
//! first out, 512 MB by default). Textures are sRGB RGBA8; [`TextureImage`]
//! samples bilinearly and returns linear light.
//!
//! The surface-to-texture mapping ([`planar_uv`]) is shared by the OpenGL
//! shader (which mirrors it line for line), the ray tracer and the tests, so
//! meshes need no UV field: walls use their face plane, floors and ground use
//! plan XZ, roofs their sloped plane.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use plan_3d::Material;
use plan_library::image::{self, Rgba8Image};

use crate::material::{MaterialDef, ProceduralKind};
use crate::noise::{fbm, value_noise};
use crate::texture::render_texture;

/// Default cache budget for decoded textures.
pub const DEFAULT_CACHE_BYTES: usize = 512 * 1024 * 1024;
/// Side of the generated fallback bitmaps, pixels.
pub const PROCEDURAL_SIZE: u32 = 256;
/// Chief textures larger than this are box-filtered down on load.
pub const MAX_TEXTURE_SIDE: u32 = 1024;
/// Horizontal surface normal length below which a surface counts as flat.
pub const FLAT_EPSILON: f32 = 0.02;

/// Where a texture came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureOrigin {
    /// A file from Chief's install (or any user file).
    File(PathBuf),
    /// Generated in code.
    Procedural,
}

/// A decoded texture ready to upload or sample.
#[derive(Debug, Clone)]
pub struct TextureImage {
    /// sRGB RGBA8 pixels, top row first.
    pub image: Rgba8Image,
    pub origin: TextureOrigin,
    /// Real-world size of one repeat, inches `[across, down]`.
    pub scale_in: [f32; 2],
    /// Mean color in linear light.
    pub average: [f32; 3],
}

fn srgb_lut() -> &'static [f32; 256] {
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = [0.0f32; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            *v = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
        }
        t
    })
}

impl TextureImage {
    /// Wraps a decoded image; `scale_in` is the inches one repeat covers.
    pub fn new(image: Rgba8Image, origin: TextureOrigin, scale_in: [f32; 2]) -> Self {
        let lut = srgb_lut();
        let mut acc = [0.0f64; 3];
        // Sample sparsely: the average only needs to be representative.
        let n = (image.rgba.len() / 4).max(1);
        let step = (n / 4096).max(1);
        let mut count = 0.0f64;
        for p in image.rgba.as_chunks::<4>().0.iter().step_by(step) {
            for k in 0..3 {
                acc[k] += f64::from(lut[usize::from(p[k])]);
            }
            count += 1.0;
        }
        let c = count.max(1.0);
        TextureImage {
            average: [
                (acc[0] / c) as f32,
                (acc[1] / c) as f32,
                (acc[2] / c) as f32,
            ],
            image,
            origin,
            scale_in,
        }
    }

    /// Bytes of pixel data.
    pub fn byte_len(&self) -> usize {
        self.image.byte_len()
    }

    /// Bilinear sample at `(u, v)` measured in repeats (wraps in both
    /// directions); returns linear-light RGB.
    pub fn sample(&self, u: f32, v: f32) -> [f32; 3] {
        let (w, h) = (self.image.width as usize, self.image.height as usize);
        if w == 0 || h == 0 {
            return self.average;
        }
        let x = (u - u.floor()) * w as f32 - 0.5;
        let y = (v - v.floor()) * h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let wrap = |i: f32, n: usize| (i as i64).rem_euclid(n as i64) as usize;
        let (xa, xb) = (wrap(x0, w), wrap(x0 + 1.0, w));
        let (ya, yb) = (wrap(y0, h), wrap(y0 + 1.0, h));
        let lut = srgb_lut();
        let px = |xx: usize, yy: usize, k: usize| {
            lut[usize::from(self.image.rgba[(yy * w + xx) * 4 + k])]
        };
        let mut out = [0.0f32; 3];
        for (k, o) in out.iter_mut().enumerate() {
            let top = px(xa, ya, k) * (1.0 - fx) + px(xb, ya, k) * fx;
            let bot = px(xa, yb, k) * (1.0 - fx) + px(xb, yb, k) * fx;
            *o = top * (1.0 - fy) + bot * fy;
        }
        out
    }
}

// ---- projection ------------------------------------------------------------

/// How a material's surfaces are projected onto their texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    /// By the face: the plane of a wall or roof, plan XZ for flat faces.
    Auto,
    /// Always plan XZ (floors, ceilings, ground, water, paving).
    GroundXz,
}

/// The projection a scene material uses.
pub fn projection(m: Material) -> Projection {
    match m {
        Material::Floor
        | Material::Ceiling
        | Material::Grass
        | Material::Mulch
        | Material::Water
        | Material::Asphalt
        | Material::Gravel => Projection::GroundXz,
        _ => Projection::Auto,
    }
}

/// Texture coordinates, in repeats, of a surface point.
///
/// `pos` and `normal` are in scene axes (inches, Y up). With
/// [`Projection::Auto`] a face that is not flat is mapped in its own plane:
/// `u` runs to the viewer's right looking at the face, `v` runs down the
/// slope (so an image's top points up the wall or up the roof), and distances
/// in the plane map to inches / `scale_in` without stretching. Flat faces and
/// [`Projection::GroundXz`] use `(x, z)`. `inv_scale` is `1 / scale_in`.
///
/// The GLSL in `plan-view3d`'s shader is a line-for-line copy.
pub fn planar_uv(
    pos: [f32; 3],
    normal: [f32; 3],
    proj: Projection,
    inv_scale: [f32; 2],
) -> [f32; 2] {
    let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    let n = if len > 1e-12 {
        [normal[0] / len, normal[1] / len, normal[2] / len]
    } else {
        [0.0, 1.0, 0.0]
    };
    let hl = (n[0] * n[0] + n[2] * n[2]).sqrt();
    let (a, b) = if proj == Projection::GroundXz || hl < FLAT_EPSILON {
        (pos[0], pos[2])
    } else {
        // b: the in-plane direction pointing up the slope; t = b x n points right.
        let bv = [-n[0] * n[1] / hl, hl, -n[2] * n[1] / hl];
        let t = [
            bv[1] * n[2] - bv[2] * n[1],
            bv[2] * n[0] - bv[0] * n[2],
            bv[0] * n[1] - bv[1] * n[0],
        ];
        (
            pos[0] * t[0] + pos[1] * t[1] + pos[2] * t[2],
            -(pos[0] * bv[0] + pos[1] * bv[1] + pos[2] * bv[2]),
        )
    };
    [a * inv_scale[0], b * inv_scale[1]]
}

// ---- which materials have textures -----------------------------------------

/// Whether `m` has a texture (some, like glass and trim, stay flat).
pub fn textured(m: Material) -> bool {
    spec(m).is_some()
}

/// Real-world size of one texture repeat for the procedural fallback, inches.
pub fn default_scale_in(m: Material) -> [f32; 2] {
    spec(m).map_or([48.0, 48.0], |s| s.scale)
}

struct Spec {
    /// Chief texture file names to try, in order.
    chief: &'static [&'static str],
    /// Fallback tile size, inches.
    scale: [f32; 2],
}

fn spec(m: Material) -> Option<Spec> {
    let s = |chief: &'static [&'static str], w: f32, h: f32| {
        Some(Spec {
            chief,
            scale: [w, h],
        })
    };
    match m {
        Material::WallExterior | Material::Siding => s(&["LapSidingCRCAAB.jpg"], 24.0, 24.0),
        Material::Floor => s(&["OakHardwoodHoney.jpg", "Oak.jpg"], 36.0, 12.0),
        Material::DoorPanel => s(&["Oak.jpg"], 24.0, 24.0),
        Material::Roof => s(
            &["Asphalt Roofing Grey 2016.jpg", "Shingle - Grey.JPG"],
            36.0,
            30.0,
        ),
        Material::Stucco => s(&["Stucco(48).jpg"], 36.0, 36.0),
        Material::Brick => s(&["Brick(36).jpg"], 32.0, 9.0),
        Material::Stone => s(&["StoneVeneer.jpg"], 36.0, 36.0),
        Material::Concrete => s(&["Concrete(72).jpg"], 48.0, 48.0),
        Material::Metal => s(&["BrushedMetal.jpg"], 32.0, 32.0),
        Material::Framing => s(&["Fir(36).jpg"], 48.0, 48.0),
        Material::Grass => s(&["Grass5.jpg"], 48.0, 48.0),
        Material::Mulch => s(&["Mulch(dark).jpg"], 36.0, 36.0),
        Material::Water => s(&["Water3(48).jpg"], 96.0, 96.0),
        Material::Asphalt => s(&["Asphalt-01.jpg"], 48.0, 48.0),
        Material::Gravel => s(&["Gravel.jpg"], 24.0, 24.0),
        _ => None,
    }
}

// ---- procedural fallbacks ---------------------------------------------------

fn base_rgb(m: Material) -> [u8; 3] {
    let c = m.color();
    [0, 1, 2].map(|i| (c[i].clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
}

fn darker(c: [u8; 3], k: f32) -> [u8; 3] {
    c.map(|v| (f32::from(v) * k) as u8)
}

fn procedural_def(m: Material) -> Option<MaterialDef> {
    use ProceduralKind as K;
    let rgb = base_rgb(m);
    let sc = spec(m)?.scale;
    let wood = |ring: f64| K::Wood {
        grain_color: darker(rgb, 0.62),
        ring_spacing: ring,
    };
    let kind = match m {
        Material::WallExterior | Material::Siding => K::LapSiding,
        Material::Floor => wood(0.8),
        Material::DoorPanel => wood(1.0),
        Material::Framing => wood(0.6),
        Material::Roof => K::Shingles,
        Material::Stucco => K::Stucco { grain: 0.6 },
        Material::Brick => K::Brick {
            mortar_color: [190, 186, 176],
        },
        Material::Stone | Material::Gravel => K::Stone,
        Material::Concrete | Material::Asphalt => K::Concrete,
        Material::Metal => K::Metal,
        Material::Grass => K::Grass,
        Material::Mulch => K::Carpet,
        // Water is generated separately (see `water_pixels`).
        _ => K::Glass,
    };
    Some(
        MaterialDef::new(m.name(), &["Scene"], rgb)
            .with_texture(kind, (f64::from(sc[0]), f64::from(sc[1]))),
    )
}

/// Tileable water ripples around `rgb`.
fn water_pixels(rgb: [u8; 3], size: u32) -> Vec<u8> {
    let n = size as usize;
    let base = rgb.map(|v| f32::from(v) / 255.0);
    let mut out = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        let v = (y as f32 + 0.5) / n as f32;
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32;
            let warp = fbm(u, v, 4, 4, 7, 3);
            let ripple = ((u * 6.0 + warp * 2.5) * std::f32::consts::TAU).sin()
                * ((v * 5.0 - warp * 2.0) * std::f32::consts::TAU).sin();
            let glint = value_noise(u * 32.0, v * 32.0, 32, 32, 11);
            let shade = 0.88 + 0.12 * ripple + 0.10 * (glint - 0.5);
            for c in base {
                out.push(((c * shade).clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
            out.push(255);
        }
    }
    out
}

/// The generated fallback for `m`, or `None` for untextured materials.
pub fn procedural(m: Material) -> Option<TextureImage> {
    let sc = spec(m)?.scale;
    let pixels = if m == Material::Water {
        water_pixels(base_rgb(m), PROCEDURAL_SIZE)
    } else {
        let mut px = render_texture(&procedural_def(m)?, PROCEDURAL_SIZE);
        for a in px.as_chunks_mut::<4>().0 {
            a[3] = 255; // the mesh carries the material's transparency
        }
        px
    };
    let img = Rgba8Image {
        width: PROCEDURAL_SIZE,
        height: PROCEDURAL_SIZE,
        rgba: pixels,
    };
    Some(TextureImage::new(img, TextureOrigin::Procedural, sc))
}

// ---- Chief's install --------------------------------------------------------

/// The folders searched for Chief texture files, most specific first.
pub fn default_texture_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(extra) = std::env::var_os("PLAN_STUDIO_TEXTURES") {
        dirs.push(PathBuf::from(extra));
    }
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(Path::new(&home).join("Documents/Chief Architect Premier X18 Data/Textures"));
    }
    dirs.push(PathBuf::from(
        "/Library/Application Support/Chief Architect Premier X18/Referenced Files",
    ));
    dirs.retain(|d| d.is_dir());
    dirs
}

/// The tile size in inches a Chief file name announces: `Brick(36).jpg` is 36.
pub fn tile_inches_from_name(file_name: &str) -> Option<f32> {
    let open = file_name.rfind('(')?;
    let close = file_name[open..].find(')')? + open;
    let n: f32 = file_name[open + 1..close].trim().parse().ok()?;
    (6.0..=240.0).contains(&n).then_some(n)
}

fn load_file(path: &Path, default_scale: [f32; 2]) -> Option<TextureImage> {
    let img = image::decode_file(path).ok()?;
    let img = img.downscaled(MAX_TEXTURE_SIDE);
    let name = path.file_name()?.to_string_lossy();
    let scale = tile_inches_from_name(&name).map_or(default_scale, |n| [n, n]);
    Some(TextureImage::new(
        img,
        TextureOrigin::File(path.to_path_buf()),
        scale,
    ))
}

// ---- the cache ---------------------------------------------------------------

struct Slot {
    tex: Option<Arc<TextureImage>>,
    bytes: usize,
    used: u64,
}

struct Inner {
    map: HashMap<String, Slot>,
    bytes: usize,
    cap: usize,
    tick: u64,
}

/// Decodes and caches textures; safe to share between threads.
pub struct TextureStore {
    dirs: Vec<PathBuf>,
    inner: Mutex<Inner>,
}

impl Default for TextureStore {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TextureStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextureStore")
            .field("dirs", &self.dirs)
            .field("cached_bytes", &self.cached_bytes())
            .finish()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl TextureStore {
    /// A store that searches the default Chief folders.
    pub fn new() -> Self {
        Self::with_dirs(default_texture_dirs())
    }

    /// A store searching `dirs` (empty: procedural textures only).
    pub fn with_dirs(dirs: Vec<PathBuf>) -> Self {
        TextureStore {
            dirs,
            inner: Mutex::new(Inner {
                map: HashMap::new(),
                bytes: 0,
                cap: DEFAULT_CACHE_BYTES,
                tick: 0,
            }),
        }
    }

    /// Sets the cache budget in bytes (builder style).
    pub fn with_capacity(self, bytes: usize) -> Self {
        lock(&self.inner).cap = bytes;
        self
    }

    /// The process-wide store the viewport and the ray tracer share.
    pub fn shared() -> Arc<TextureStore> {
        static SHARED: OnceLock<Arc<TextureStore>> = OnceLock::new();
        Arc::clone(SHARED.get_or_init(|| Arc::new(TextureStore::new())))
    }

    /// The folders searched for Chief files.
    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// Decoded bytes currently cached.
    pub fn cached_bytes(&self) -> usize {
        lock(&self.inner).bytes
    }

    /// Textures currently cached.
    pub fn cached_count(&self) -> usize {
        lock(&self.inner)
            .map
            .values()
            .filter(|s| s.tex.is_some())
            .count()
    }

    /// Drops everything cached.
    pub fn clear(&self) {
        let mut g = lock(&self.inner);
        g.map.clear();
        g.bytes = 0;
    }

    /// Whether `key` is cached.
    pub fn is_cached(&self, key: &str) -> bool {
        lock(&self.inner)
            .map
            .get(key)
            .is_some_and(|s| s.tex.is_some())
    }

    /// The cache key of a scene material's texture.
    pub fn material_key(m: Material) -> String {
        format!("material:{}", m.name())
    }

    fn cached_or_load(
        &self,
        key: &str,
        load: impl FnOnce() -> Option<TextureImage>,
    ) -> Option<Arc<TextureImage>> {
        {
            let mut g = lock(&self.inner);
            g.tick += 1;
            let tick = g.tick;
            if let Some(slot) = g.map.get_mut(key) {
                slot.used = tick;
                return slot.tex.clone();
            }
        }
        // Decode without the lock: JPEG decoding takes tens of milliseconds.
        let tex = load().map(Arc::new);
        let mut g = lock(&self.inner);
        g.tick += 1;
        let tick = g.tick;
        if let Some(slot) = g.map.get_mut(key) {
            slot.used = tick;
            return slot.tex.clone(); // another thread won the race
        }
        let bytes = tex.as_ref().map_or(0, |t| t.byte_len());
        g.bytes += bytes;
        g.map.insert(
            key.to_string(),
            Slot {
                tex: tex.clone(),
                bytes,
                used: tick,
            },
        );
        while g.bytes > g.cap && g.map.len() > 1 {
            let victim = g
                .map
                .iter()
                .filter(|(k, _)| k.as_str() != key)
                .min_by_key(|(_, s)| s.used)
                .map(|(k, _)| k.clone());
            let Some(victim) = victim else { break };
            if let Some(slot) = g.map.remove(&victim) {
                g.bytes -= slot.bytes;
            }
        }
        tex
    }

    /// The scene material's texture if it is already cached (never loads).
    pub fn peek_material(&self, m: Material) -> Option<Arc<TextureImage>> {
        let mut g = lock(&self.inner);
        g.tick += 1;
        let tick = g.tick;
        let slot = g.map.get_mut(&Self::material_key(m))?;
        slot.used = tick;
        slot.tex.clone()
    }

    /// The texture for a scene material: Chief's file when found, else the
    /// procedural fallback; `None` for materials that stay flat.
    pub fn material(&self, m: Material) -> Option<Arc<TextureImage>> {
        let sp = spec(m)?;
        self.cached_or_load(&Self::material_key(m), || {
            for name in sp.chief {
                for dir in &self.dirs {
                    let p = dir.join(name);
                    if p.is_file() {
                        if let Some(t) = load_file(&p, sp.scale) {
                            return Some(t);
                        }
                    }
                }
            }
            procedural(m)
        })
    }

    /// The texture of a library material: its own image file when it names
    /// one that loads, else its generated bitmap.
    pub fn definition(&self, def: &MaterialDef) -> Arc<TextureImage> {
        let key = format!("def:{}:{:?}", def.name, def.texture_path);
        self.cached_or_load(&key, || {
            let scale = [def.texture_scale_in.0 as f32, def.texture_scale_in.1 as f32];
            if let Some(p) = &def.texture_path {
                if let Some(t) = load_file(Path::new(p), scale) {
                    return Some(t);
                }
            }
            let size = crate::DEFAULT_TEXTURE_SIZE;
            let img = Rgba8Image {
                width: size,
                height: size,
                rgba: render_texture(def, size),
            };
            Some(TextureImage::new(img, TextureOrigin::Procedural, scale))
        })
        .expect("a generated texture always exists")
    }

    /// Any image file as a texture (tile size `default_scale_in`, or the
    /// `Name(36).jpg` convention); cached by path.
    pub fn file(&self, path: &Path, default_scale_in: [f32; 2]) -> Option<Arc<TextureImage>> {
        self.cached_or_load(&format!("file:{}", path.display()), || {
            load_file(path, default_scale_in)
        })
    }
}

#[cfg(test)]
mod tests;
