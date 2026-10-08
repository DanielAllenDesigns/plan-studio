//! Renderable mesh types: vertices, materials, meshes and scenes.

use plan_core::Id;

/// One mesh vertex. Positions are inches; UVs are in feet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

/// Surface material. Each variant maps to one glTF material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Material {
    WallExterior,
    WallInterior,
    Floor,
    Ceiling,
    DoorPanel,
    WindowGlass,
    WindowFrame,
    Roof,
}

impl Material {
    /// Every material, in the order used for glTF material indices.
    pub const ALL: [Material; 8] = [
        Material::WallExterior,
        Material::WallInterior,
        Material::Floor,
        Material::Ceiling,
        Material::DoorPanel,
        Material::WindowGlass,
        Material::WindowFrame,
        Material::Roof,
    ];

    /// Index of this material within [`Material::ALL`].
    pub fn index(&self) -> usize {
        Self::ALL
            .iter()
            .position(|m| m == self)
            .expect("every Material variant is listed in ALL")
    }

    /// Default linear RGBA base color (muted, realistic).
    pub fn color(&self) -> [f32; 4] {
        match self {
            Material::WallExterior => [0.80, 0.74, 0.62, 1.0], // siding beige
            Material::WallInterior => [0.93, 0.92, 0.89, 1.0], // drywall off-white
            Material::Floor => [0.55, 0.39, 0.25, 1.0],        // wood floor
            Material::Ceiling => [0.97, 0.97, 0.96, 1.0],      // white ceiling
            Material::DoorPanel => [0.45, 0.30, 0.18, 1.0],    // door wood
            Material::WindowGlass => [0.62, 0.78, 0.90, 0.35], // light blue, translucent
            Material::WindowFrame => [0.96, 0.96, 0.95, 1.0],  // white frame
            Material::Roof => [0.34, 0.32, 0.31, 1.0],         // charcoal shingle
        }
    }

    /// Human-readable name used in glTF.
    pub fn name(&self) -> &'static str {
        match self {
            Material::WallExterior => "WallExterior",
            Material::WallInterior => "WallInterior",
            Material::Floor => "Floor",
            Material::Ceiling => "Ceiling",
            Material::DoorPanel => "DoorPanel",
            Material::WindowGlass => "WindowGlass",
            Material::WindowFrame => "WindowFrame",
            Material::Roof => "Roof",
        }
    }
}

/// A triangle mesh with a single material.
#[derive(Debug, Clone)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material: Material,
    /// The wall or opening this mesh was generated from, if any.
    pub object_id: Option<Id>,
}

/// Min/max corners of an axis-aligned box.
pub type Bounds = ([f32; 3], [f32; 3]);

fn bounds_of<'a>(vertices: impl Iterator<Item = &'a Vertex>) -> Option<Bounds> {
    let mut acc: Option<Bounds> = None;
    for v in vertices {
        let (lo, hi) = acc.get_or_insert((v.position, v.position));
        for k in 0..3 {
            lo[k] = lo[k].min(v.position[k]);
            hi[k] = hi[k].max(v.position[k]);
        }
    }
    acc
}

impl Mesh {
    /// Axis-aligned bounds, or `None` for an empty mesh.
    pub fn bounds(&self) -> Option<Bounds> {
        bounds_of(self.vertices.iter())
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Everything to draw: a flat list of meshes.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub meshes: Vec<Mesh>,
}

impl Scene {
    /// Axis-aligned bounds of all meshes, or `None` when the scene is empty.
    pub fn bounds(&self) -> Option<Bounds> {
        bounds_of(self.meshes.iter().flat_map(|m| m.vertices.iter()))
    }

    /// Total triangle count across all meshes.
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(Mesh::triangle_count).sum()
    }
}
