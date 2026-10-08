//! Texturing logic that needs no OpenGL: which materials a scene needs
//! uploaded, how many to upload per frame, which pictures carry a bitmap, and
//! the GLSL that mirrors [`plan_materials::textures::planar_uv`].
//!
//! The GL calls themselves live in `gpu`; everything here is unit tested
//! without a context.

use std::collections::HashSet;
use std::sync::Arc;

use plan_3d::{Material, Scene};
use plan_core::Id;
use plan_materials::textures::{textured, FLAT_EPSILON};

pub use plan_materials::textures::{planar_uv, projection, Projection};

/// New material textures uploaded per frame (the rest wait for the next one,
/// so a scene with many fresh materials never stalls a single frame).
pub const MAX_UPLOADS_PER_FRAME: usize = 2;
/// Largest picture side uploaded to the GPU; bigger bitmaps are the caller's
/// to shrink (`plan_library::image::Rgba8Image::downscaled`).
pub const MAX_PICTURE_SIDE: u32 = 2048;

/// A decoded picture to draw on the quad(s) whose mesh `object_id` matches.
///
/// The quad's own UVs apply: `(0,0)` is the picture's bottom-left, `(1,1)` its
/// top-right (the layout `plan_3d::images` writes). `key` identifies the
/// pixels (a content hash is ideal): two textures with the same key share one
/// GPU upload.
#[derive(Clone, Debug)]
pub struct ImageTexture {
    pub object_id: Id,
    pub key: u64,
    pub width: u32,
    pub height: u32,
    /// Straight sRGB RGBA8, top row first (`width * height * 4` bytes).
    pub rgba: Arc<Vec<u8>>,
    /// Mirror the picture horizontally.
    pub flip: bool,
}

impl ImageTexture {
    /// Whether any pixel is not opaque (the quad then goes in the blended pass).
    pub fn has_alpha(&self) -> bool {
        self.rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255)
    }

    /// Whether the pixel buffer matches the declared size.
    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= MAX_PICTURE_SIDE
            && self.height <= MAX_PICTURE_SIDE
            && self.rgba.len() == self.width as usize * self.height as usize * 4
    }
}

/// The textured materials `scene` uses, each once, in order of first
/// appearance. The selection tint never counts.
pub fn needed_materials(scene: &Scene) -> Vec<Material> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for mesh in &scene.meshes {
        let m = mesh.material;
        if m != Material::Selection && textured(m) && seen.insert(m) {
            out.push(m);
        }
    }
    out
}

/// Which of `needed` to upload this frame: those not `resident`, at most
/// [`MAX_UPLOADS_PER_FRAME`], in `needed` order.
pub fn next_uploads(needed: &[Material], resident: &HashSet<Material>) -> Vec<Material> {
    needed
        .iter()
        .copied()
        .filter(|m| !resident.contains(m))
        .take(MAX_UPLOADS_PER_FRAME)
        .collect()
}

/// How many of `needed` are still waiting for an upload.
pub fn pending_uploads(needed: &[Material], resident: &HashSet<Material>) -> usize {
    needed.iter().filter(|m| !resident.contains(m)).count()
}

/// GLSL for the surface-to-texture mapping; a line-for-line copy of
/// [`planar_uv`] (`proj` 1 is [`Projection::GroundXz`]).
pub fn glsl_planar_uv() -> String {
    format!(
        r#"
vec2 planar_uv(vec3 pos, vec3 normal, int proj, vec2 inv_scale) {{
    float nl = length(normal);
    vec3 n = nl > 1e-12 ? normal / nl : vec3(0.0, 1.0, 0.0);
    float hl = length(n.xz);
    vec2 ab;
    if (proj == 1 || hl < {FLAT_EPSILON:?}) {{
        ab = pos.xz;
    }} else {{
        vec3 b = vec3(-n.x * n.y / hl, hl, -n.z * n.y / hl);
        vec3 t = cross(b, n);
        ab = vec2(dot(pos, t), -dot(pos, b));
    }}
    return ab * inv_scale;
}}
"#
    )
}

/// The shader's projection code for a material.
pub fn proj_code(m: Material) -> i32 {
    match projection(m) {
        Projection::Auto => 0,
        Projection::GroundXz => 1,
    }
}

#[cfg(test)]
mod tests;
