//! Per-mesh surface properties of painted materials (the Properties tab of the
//! Material Specification): roughness, metalness, transparency and emissive
//! strength that the viewport shader and the ray tracer read for a mesh that
//! carries its own [`Mesh::color`](crate::Mesh).
//!
//! `Mesh` is built by struct literal in many places, so these values travel in
//! a small process-wide table instead of a field: the app registers them when
//! it paints a scene (`register`) and both renderers ask for them per mesh
//! (`Mesh::paint_surface`). An entry is keyed by the mesh's object id, its
//! scene material and its exact colour, so an entry that outlives its paint
//! is never found by a mesh that is not painted that way.

use crate::{Material, Mesh};
use plan_core::Id;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// How a painted surface responds to light, on the same scales as
/// `plan_materials::MaterialDef`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintSurface {
    /// 0 = mirror, 1 = fully diffuse.
    pub roughness: f32,
    /// 0 = dielectric, 1 = metal.
    pub metallic: f32,
    /// 0 = opaque, 1 = fully clear.
    pub transparency: f32,
    /// Self-illumination, 0..1.
    pub emissive: f32,
}

type Key = (Id, usize, [u8; 3]);

fn table() -> &'static RwLock<HashMap<Key, PaintSurface>> {
    static T: OnceLock<RwLock<HashMap<Key, PaintSurface>>> = OnceLock::new();
    T.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Sets (`Some`) or removes (`None`) the surface of meshes of `object` drawn
/// as `material` in exactly `color`.
pub fn register(object: Id, material: Material, color: [u8; 3], surface: Option<PaintSurface>) {
    let key = (object, material.index(), color);
    let mut t = table().write().unwrap_or_else(|e| e.into_inner());
    match surface {
        Some(s) => {
            t.insert(key, s);
        }
        None => {
            t.remove(&key);
        }
    }
}

/// The registered surface of a painted mesh of `object`, if any.
pub fn lookup(
    object: Option<Id>,
    material: Material,
    color: Option<[u8; 3]>,
) -> Option<PaintSurface> {
    let (object, color) = (object?, color?);
    let t = table().read().unwrap_or_else(|e| e.into_inner());
    t.get(&(object, material.index(), color)).copied()
}

impl Mesh {
    /// The surface properties the Material Painter gave this mesh; `None`
    /// keeps the scene material's own (see `plan_materials::scene_surface`).
    pub fn paint_surface(&self) -> Option<PaintSurface> {
        lookup(self.object_id, self.material, self.color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(object: Id, color: Option<[u8; 3]>) -> Mesh {
        Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            material: Material::Trim,
            object_id: Some(object),
            color,
        }
    }

    #[test]
    fn only_a_mesh_painted_that_way_finds_its_surface() {
        let s = PaintSurface {
            roughness: 0.1,
            metallic: 0.9,
            transparency: 0.0,
            emissive: 0.0,
        };
        register(9_100_001, Material::Trim, [1, 2, 3], Some(s));
        assert_eq!(mesh(9_100_001, Some([1, 2, 3])).paint_surface(), Some(s));
        // Another colour, no colour, or another object finds nothing.
        assert_eq!(mesh(9_100_001, Some([1, 2, 4])).paint_surface(), None);
        assert_eq!(mesh(9_100_001, None).paint_surface(), None);
        assert_eq!(mesh(9_100_002, Some([1, 2, 3])).paint_surface(), None);
        register(9_100_001, Material::Trim, [1, 2, 3], None);
        assert_eq!(mesh(9_100_001, Some([1, 2, 3])).paint_surface(), None);
    }
}
