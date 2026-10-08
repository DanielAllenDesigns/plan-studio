//! Incremental mesh construction with automatic winding correction.

use crate::mesh::{Material, Mesh, Vertex};
use plan_core::Id;

/// A 3D vector in scene space (X right, Y up, Z = -plan y).
pub type V3 = [f32; 3];

pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Accumulates triangles for a single material.
#[derive(Debug, Clone)]
pub struct MeshBuilder {
    material: Material,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    pub fn new(material: Material) -> Self {
        Self {
            material,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Add a triangle whose front side faces `normal`; the winding is flipped if needed.
    pub fn tri(&mut self, p: [V3; 3], uv: [[f32; 2]; 3], normal: V3) {
        let geometric = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        let order = if dot(geometric, normal) < 0.0 {
            [0, 2, 1]
        } else {
            [0, 1, 2]
        };
        let base = self.vertices.len() as u32;
        for k in order {
            self.vertices.push(Vertex {
                position: p[k],
                normal,
                uv: uv[k],
            });
        }
        self.indices.extend([base, base + 1, base + 2]);
    }

    /// Add a planar convex quad (corners in order around the perimeter) facing `normal`.
    pub fn quad(&mut self, p: [V3; 4], uv: [[f32; 2]; 4], normal: V3) {
        self.tri([p[0], p[1], p[2]], [uv[0], uv[1], uv[2]], normal);
        self.tri([p[0], p[2], p[3]], [uv[0], uv[2], uv[3]], normal);
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn finish(self, object_id: Option<Id>) -> Mesh {
        Mesh {
            vertices: self.vertices,
            indices: self.indices,
            material: self.material,
            object_id,
        }
    }
}

/// A group of builders (one per material) for a single source object.
#[derive(Debug, Clone, Default)]
pub struct MeshSet {
    builders: Vec<MeshBuilder>,
}

impl MeshSet {
    /// The builder for `material`, created on first use.
    pub fn material(&mut self, material: Material) -> &mut MeshBuilder {
        let pos = match self.builders.iter().position(|b| b.material == material) {
            Some(i) => i,
            None => {
                self.builders.push(MeshBuilder::new(material));
                self.builders.len() - 1
            }
        };
        &mut self.builders[pos]
    }

    /// Non-empty meshes, tagged with `object_id`.
    pub fn finish(self, object_id: Option<Id>) -> Vec<Mesh> {
        self.builders
            .into_iter()
            .filter(|b| !b.is_empty())
            .map(|b| b.finish(object_id))
            .collect()
    }
}
