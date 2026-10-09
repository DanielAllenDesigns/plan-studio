//! Material-package maps (Lightbeans) in the path tracer.
//!
//! A mesh painted with a material package carries, besides its flat colour,
//! the package's albedo, normal, roughness, metallic, ambient-occlusion and
//! opacity maps (`plan_materials::pbr`, registered by the app when it paints
//! the scene). The same planar mapping the 3D view uses finds the texel; the
//! tangent frame follows from the face normal, so no tangents are stored.
//! Roughness and metalness replace the scalar values, the normal bends the
//! shading normal, occlusion darkens the diffuse albedo, and an opacity below
//! one half lets the ray through. Without a map the scalar values stay.

use std::sync::Arc;

use crate::settings::Technique;
use crate::shading::{Custom, Surface};
use crate::vec3::V3;
use plan_materials::pbr::{self, PbrSet};
use plan_materials::textures::{planar_uv, projection, Projection};

/// Largest map side the path tracer keeps, pixels.
pub(crate) const MAX_SIDE: u32 = 4096;
/// Brightest albedo a textured surface may reach.
const MAX_ALBEDO: f32 = 0.95;

/// One painted look's maps and how to map them.
pub(crate) struct PbrBinding {
    set: Arc<PbrSet>,
    proj: Projection,
    inv_scale: [f32; 2],
}

fn arr(v: V3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

impl PbrBinding {
    fn uv(&self, p: V3, ng: V3) -> [f32; 2] {
        planar_uv(arr(p), arr(ng), self.proj, self.inv_scale)
    }

    /// Is the surface cut out (opacity below one half) at `p`?
    pub fn cut_out(&self, p: V3, ng: V3) -> bool {
        let uv = self.uv(p, ng);
        self.set
            .sample(uv[0], uv[1])
            .opacity
            .is_some_and(|o| o < 0.5)
    }

    /// The surface at `p` and its shading normal: `base` with the maps'
    /// albedo, roughness, metalness and occlusion, and `n` (the face normal
    /// toward the viewer; `ng` is the geometric one) bent by the normal map.
    pub fn shade(&self, p: V3, ng: V3, n: V3, wo: V3, base: &Surface) -> (Surface, V3) {
        let uv = self.uv(p, ng);
        let s = self.set.sample(uv[0], uv[1]);
        let mut surface = *base;
        if let Some(a) = self.set.albedo_linear(uv[0], uv[1]) {
            surface.albedo = V3::new(
                a[0].min(MAX_ALBEDO),
                a[1].min(MAX_ALBEDO),
                a[2].min(MAX_ALBEDO),
            );
        }
        if let Some(r) = s.roughness {
            surface.roughness = r.clamp(0.0, 1.0);
        }
        if let Some(m) = s.metallic {
            surface.metallic = m.clamp(0.0, 1.0);
        }
        if let Some(ao) = s.ao {
            surface.albedo = surface.albedo * ao.clamp(0.0, 1.0);
        }
        let mut normal = n;
        if let Some(ts) = s.normal {
            let frame = pbr::tangent_frame(arr(ng), self.proj);
            let bent = V3::from_array(pbr::perturb_normal(arr(n), frame, ts)).normalized();
            // A grazing view must not see the back of the bent normal.
            if bent.dot(wo) > 0.0 {
                normal = bent;
            }
        }
        (surface, normal)
    }
}

/// The bindings of the `custom` looks, indexed like the surface table
/// (`MATERIAL_COUNT + i`); empty when textures are off or the technique
/// ignores colours.
pub(crate) fn table(
    custom: &[Custom],
    enabled: bool,
    technique: Technique,
    offset: usize,
) -> Vec<Option<PbrBinding>> {
    let mut out: Vec<Option<PbrBinding>> = Vec::new();
    if !enabled || technique != Technique::PhysicallyBased {
        return out;
    }
    out.resize_with(offset, || None);
    for c in custom {
        out.push(c.maps.and_then(pbr::source_by_key).map(|src| {
            let [w, h] = src.scale_in();
            PbrBinding {
                set: pbr::load_cached(&src, MAX_SIDE, true),
                proj: projection(c.material),
                inv_scale: [1.0 / w.max(1e-3), 1.0 / h.max(1e-3)],
            }
        }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shading::Kind;
    use plan_3d::Material;
    use plan_library::image::{png, Rgba8Image};
    use plan_materials::MaterialDef;

    fn write(dir: &std::path::Path, name: &str, w: u32, h: u32, px: [u8; 4]) -> String {
        let p = dir.join(name);
        std::fs::write(&p, png::encode_rgba(&Rgba8Image::filled(w, h, px))).unwrap();
        p.display().to_string()
    }

    fn base() -> Surface {
        Surface {
            kind: Kind::Opaque,
            albedo: V3::new(0.5, 0.5, 0.5),
            roughness: 0.9,
            metallic: 0.0,
            tint: V3::ONE,
            emission: V3::ZERO,
        }
    }

    fn binding(def: &MaterialDef) -> PbrBinding {
        let src = pbr::PbrSource::from_def(def).unwrap();
        PbrBinding {
            set: Arc::new(PbrSet::load(&src, 64, true)),
            proj: Projection::Auto,
            inv_scale: [1.0 / 12.0, 1.0 / 12.0],
        }
    }

    #[test]
    fn maps_replace_roughness_metalness_albedo_and_bend_the_normal() {
        let dir = std::env::temp_dir().join(format!("ps-render-pbr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut def = MaterialDef::new("Slab", &["T"], [10, 10, 10]);
        def.bump = 0.5;
        def.texture_path = Some(write(&dir, "albedo.png", 2, 2, [255, 255, 255, 255]));
        def.roughness_map = Some(write(&dir, "r.png", 2, 2, [51, 51, 51, 255]));
        def.metallic_map = Some(write(&dir, "m.png", 2, 2, [255, 255, 255, 255]));
        def.ao_map = Some(write(&dir, "a.png", 2, 2, [128, 128, 128, 255]));
        // A normal leaning toward +x (right).
        def.normal_map = Some(write(&dir, "n.png", 2, 2, [218, 128, 218, 255]));
        let b = binding(&def);
        // A wall facing +z, seen head-on.
        let (p, ng) = (V3::new(3.0, 4.0, 0.0), V3::new(0.0, 0.0, 1.0));
        let wo = V3::new(0.0, 0.0, 1.0);
        let (s, n) = b.shade(p, ng, ng, wo, &base());
        assert!((s.roughness - 0.2).abs() < 0.02, "{}", s.roughness);
        assert!(s.metallic > 0.99);
        // White albedo, halved by occlusion (about half, in linear light).
        assert!(s.albedo.x > 0.4 && s.albedo.x < 0.6, "{:?}", s.albedo);
        // The face looks down -z... its right is +x for a +z facing wall? The
        // bent normal must have leaned along the frame's right axis.
        let (right, _) = pbr::tangent_frame([0.0, 0.0, 1.0], Projection::Auto);
        assert!(n.dot(V3::from_array(right)) > 0.5, "{n:?} vs {right:?}");
        assert!((n.length() - 1.0).abs() < 1e-4);
        // No cut-out without an opacity map.
        assert!(!b.cut_out(p, ng));
        // A grazing view keeps the face normal when the bent one turns away.
        let graze = V3::new(-1.0, 0.0, 0.05).normalized();
        let (_, kept) = b.shade(p, ng, ng, graze, &base());
        assert!(kept.dot(graze) > 0.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_opacity_map_cuts_the_surface_out() {
        let dir = std::env::temp_dir().join(format!("ps-render-cut-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut def = MaterialDef::new("Lace", &["T"], [10, 10, 10]);
        def.opacity_map = Some(write(&dir, "o.png", 2, 2, [0, 0, 0, 255]));
        let b = binding(&def);
        assert!(b.cut_out(V3::new(1.0, 1.0, 0.0), V3::new(0.0, 0.0, 1.0)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_table_binds_only_registered_looks_and_only_for_physically_based() {
        let dir = std::env::temp_dir().join(format!("ps-render-tbl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut def = MaterialDef::new("Tbl", &["T"], [20, 30, 40]);
        def.roughness_map = Some(write(&dir, "r.png", 2, 2, [100, 100, 100, 255]));
        let src = Arc::new(pbr::PbrSource::from_def(&def).unwrap());
        pbr::register(
            990_777,
            Material::Floor,
            [20, 30, 40],
            Some(Arc::clone(&src)),
        );
        let custom = [
            Custom {
                material: Material::Floor,
                color: [20, 30, 40],
                paint: None,
                maps: Some(src.key()),
            },
            Custom {
                material: Material::Floor,
                color: [1, 2, 3],
                paint: None,
                maps: None,
            },
        ];
        let t = table(&custom, true, Technique::PhysicallyBased, 3);
        assert_eq!(t.len(), 5);
        assert!(t[3].is_some() && t[4].is_none() && t[0].is_none());
        assert!(table(&custom, false, Technique::PhysicallyBased, 3).is_empty());
        assert!(table(&custom, true, Technique::Clay, 3).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
