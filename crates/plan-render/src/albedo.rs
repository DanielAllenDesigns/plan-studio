//! Textured albedo for the path tracer.
//!
//! A textured [`Material`] looks its base color up in the same bitmap the 3D
//! view uses (`plan_materials::textures`), through the same surface-to-texture
//! mapping ([`planar_uv`]): bilinear, repeating at the material's real-world
//! tile size. The bitmap's brightness is matched to the material's flat
//! albedo so switching textures on changes the pattern, not the exposure.

use std::sync::Arc;

use crate::settings::Technique;
use crate::shading::{Kind, Surface, MATERIAL_COUNT};
use crate::vec3::V3;
use plan_3d::Material;
use plan_materials::textures::{planar_uv, projection, Projection, TextureImage, TextureStore};

/// Brightest albedo a textured surface may reach (keeps energy conserved).
const MAX_ALBEDO: f32 = 0.95;
/// Limits of the brightness match between a bitmap and the flat albedo.
const GAIN_RANGE: (f32, f32) = (0.25, 4.0);

/// One material's bitmap and how to map it.
pub(crate) struct TexBinding {
    tex: Arc<TextureImage>,
    proj: Projection,
    inv_scale: [f32; 2],
    gain: f32,
}

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

impl TexBinding {
    /// Linear albedo at world point `p` on a face with geometric normal `ng`.
    pub fn albedo(&self, p: V3, ng: V3) -> V3 {
        let uv = planar_uv(
            [p.x, p.y, p.z],
            [ng.x, ng.y, ng.z],
            self.proj,
            self.inv_scale,
        );
        let c = self.tex.sample(uv[0], uv[1]);
        V3::new(
            (c[0] * self.gain).min(MAX_ALBEDO),
            (c[1] * self.gain).min(MAX_ALBEDO),
            (c[2] * self.gain).min(MAX_ALBEDO),
        )
    }
}

/// The per-material bindings (index = `Material::index()`); all `None` when
/// textures are off or the technique ignores colors.
pub(crate) fn table(
    store: &TextureStore,
    enabled: bool,
    technique: Technique,
    surfaces: &[Surface; MATERIAL_COUNT],
) -> [Option<TexBinding>; MATERIAL_COUNT] {
    Material::ALL.map(|m| {
        if !enabled
            || technique != Technique::PhysicallyBased
            || surfaces[m.index()].kind != Kind::Opaque
        {
            return None;
        }
        let tex = store.material(m)?;
        let flat = m.color();
        let flat_luma = luma([flat[0], flat[1], flat[2]]);
        let gain = (flat_luma / luma(tex.average).max(1e-4)).clamp(GAIN_RANGE.0, GAIN_RANGE.1);
        Some(TexBinding {
            inv_scale: [
                1.0 / tex.scale_in[0].max(1e-3),
                1.0 / tex.scale_in[1].max(1e-3),
            ],
            proj: projection(m),
            gain,
            tex,
        })
    })
}
