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
    Stucco,
    Siding,
    Brick,
    Stone,
    Concrete,
    /// Casing, jambs, sills, thresholds, posts.
    Trim,
    /// Door and shower glass (windows keep [`Material::WindowGlass`]).
    Glass,
    /// Rails, balusters, tracks, garage-door rails.
    Metal,
    /// Dimensional lumber: framing members and the structure above ceilings.
    Framing,
    /// Lawn, grass regions and the terrain surface.
    Grass,
    /// Mulch and soil of garden beds.
    Mulch,
    /// Plant canopies.
    Foliage,
    /// Water features (translucent).
    Water,
    /// Roads and driveways.
    Asphalt,
    /// Gravel paths and drives.
    Gravel,
    /// The tint drawn over a selected object in the 3D view. Never part of a
    /// model scene or an export: the viewport adds it on top.
    Selection,
}

impl Material {
    /// Every material, in the order used for glTF material indices.
    pub const ALL: [Material; 24] = [
        Material::WallExterior,
        Material::WallInterior,
        Material::Floor,
        Material::Ceiling,
        Material::DoorPanel,
        Material::WindowGlass,
        Material::WindowFrame,
        Material::Roof,
        Material::Stucco,
        Material::Siding,
        Material::Brick,
        Material::Stone,
        Material::Concrete,
        Material::Trim,
        Material::Glass,
        Material::Metal,
        Material::Framing,
        Material::Grass,
        Material::Mulch,
        Material::Foliage,
        Material::Water,
        Material::Asphalt,
        Material::Gravel,
        Material::Selection,
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
            Material::Stucco => [0.86, 0.82, 0.72, 1.0],       // eggshell stucco
            Material::Siding => [0.72, 0.70, 0.64, 1.0],       // painted lap siding
            Material::Brick => [0.62, 0.30, 0.22, 1.0],        // red brick
            Material::Stone => [0.58, 0.56, 0.52, 1.0],        // stone veneer
            Material::Concrete => [0.66, 0.66, 0.65, 1.0],     // poured concrete
            Material::Trim => [0.97, 0.97, 0.95, 1.0],         // painted trim
            Material::Glass => [0.70, 0.85, 0.92, 0.30],       // translucent glass
            Material::Metal => [0.55, 0.57, 0.60, 1.0],        // painted steel
            Material::Framing => [0.76, 0.60, 0.38, 1.0],      // raw lumber
            Material::Grass => [0.30, 0.52, 0.22, 1.0],        // lawn
            Material::Mulch => [0.33, 0.22, 0.14, 1.0],        // dark mulch
            Material::Foliage => [0.18, 0.40, 0.17, 1.0],      // leaf canopy
            Material::Water => [0.20, 0.45, 0.65, 0.55],       // blue, translucent
            Material::Asphalt => [0.20, 0.20, 0.21, 1.0],      // paving
            Material::Gravel => [0.62, 0.58, 0.50, 1.0],       // crushed stone
            Material::Selection => [0.98, 0.62, 0.10, 0.45],   // orange tint
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
            Material::Stucco => "Stucco",
            Material::Siding => "Siding",
            Material::Brick => "Brick",
            Material::Stone => "Stone",
            Material::Concrete => "Concrete",
            Material::Trim => "Trim",
            Material::Glass => "Glass",
            Material::Metal => "Metal",
            Material::Framing => "Framing",
            Material::Grass => "Grass",
            Material::Mulch => "Mulch",
            Material::Foliage => "Foliage",
            Material::Water => "Water",
            Material::Asphalt => "Asphalt",
            Material::Gravel => "Gravel",
            Material::Selection => "Selection",
        }
    }

    /// Map a wall-layer name or material string to a surface material
    /// (case-insensitive substring match); `None` when nothing matches.
    pub fn from_layer_name(name: &str) -> Option<Material> {
        let n = name.to_ascii_lowercase();
        const TABLE: [(&str, Material); 6] = [
            ("stucco", Material::Stucco),
            ("siding", Material::Siding),
            ("brick", Material::Brick),
            ("stone", Material::Stone),
            ("concrete", Material::Concrete),
            ("drywall", Material::WallInterior),
        ];
        TABLE.iter().find(|(k, _)| n.contains(k)).map(|&(_, m)| m)
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
    /// Exact sRGB colour for this mesh (Material Painter, Adjust Materials,
    /// Material Builder). When set, the viewport and the ray tracer use it as
    /// the albedo in place of [`Material::color`]; `None` keeps the material's
    /// own colour.
    pub color: Option<[u8; 3]>,
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
    /// Linear-light value of an sRGB colour (what the shader and the ray
    /// tracer use as albedo; [`Material::color`] is linear already).
    pub fn linear_rgb(rgb: [u8; 3]) -> [f32; 3] {
        rgb.map(|c| {
            let c = f32::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
    }

    /// This mesh's own colour as linear light, if it has one.
    pub fn color_linear(&self) -> Option<[f32; 3]> {
        self.color.map(Self::linear_rgb)
    }

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
