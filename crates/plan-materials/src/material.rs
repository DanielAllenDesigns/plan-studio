//! Material definitions: appearance, drawing patterns and cost data.

use serde::{Deserialize, Serialize};

use crate::pattern::Pattern;

/// Which procedural bitmap generator paints a material in 3D.
///
/// Colours not named here come from [`MaterialDef::color`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProceduralKind {
    /// Growth rings and fibres; `ring_spacing` is inches between rings.
    Wood {
        grain_color: [u8; 3],
        ring_spacing: f64,
    },
    /// Running-bond brick with a mortar colour.
    Brick { mortar_color: [u8; 3] },
    /// Sand-float stucco; `grain` is the roughness amplitude, 0..1.
    Stucco { grain: f32 },
    /// Mottled concrete with pits.
    Concrete,
    /// Staggered shingle tabs with shadowed courses.
    Shingles,
    /// Horizontal lap boards with shadow lines.
    LapSiding,
    /// Square or rectangular tile with a grout colour.
    Tile { grout: [u8; 3] },
    /// Short dense fibres.
    Carpet,
    /// Vertical blade streaks.
    Grass,
    /// Irregular fieldstone with mortar gaps.
    Stone,
    /// Smooth glass sheen.
    Glass,
    /// Brushed metal streaks.
    Metal,
}

/// How a material's 3D colour is produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Texture {
    /// Flat [`MaterialDef::color`].
    Solid,
    /// Generated bitmap, see [`crate::render_texture`].
    Procedural(ProceduralKind),
}

/// One entry of the material library (Chief's "Material Definition").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialDef {
    /// Unique display name, e.g. `"Sand Finish – Eggshell"`.
    pub name: String,
    /// Browser path, e.g. `["Siding", "Lap Siding"]`.
    pub category: Vec<String>,
    /// Base colour (sRGB).
    pub color: [u8; 3],
    /// 0 = mirror, 1 = fully diffuse.
    pub roughness: f32,
    /// 0 = dielectric, 1 = metal.
    pub metallic: f32,
    /// 0 = opaque, 1 = fully clear.
    pub transparency: f32,
    /// Self-illumination, 0..1.
    pub emissive: f32,
    /// 2D hatch used in plan and elevation drawings.
    pub pattern: Pattern,
    /// 3D appearance.
    pub texture: Texture,
    /// Real-world size (inches) of one texture tile: (width, height).
    pub texture_scale_in: (f64, f64),
    /// Bump-map strength, 0..1.
    pub bump: f32,
    /// Material cost in dollars per square foot of surface.
    pub cost_per_sq_ft: f64,
    /// Accounting / cost-code reference.
    pub accounting_code: String,
    /// An image file that paints the material in place of the generated
    /// texture (Material Builder); `None` for the library's own materials.
    #[serde(default)]
    pub texture_path: Option<String>,
}

impl MaterialDef {
    /// A matte, opaque, solid-colour material with a 12" x 12" tile size.
    pub fn new(name: &str, category: &[&str], color: [u8; 3]) -> Self {
        Self {
            name: name.to_string(),
            category: category.iter().map(|s| (*s).to_string()).collect(),
            color,
            roughness: 0.8,
            metallic: 0.0,
            transparency: 0.0,
            emissive: 0.0,
            pattern: Pattern::None,
            texture: Texture::Solid,
            texture_scale_in: (12.0, 12.0),
            bump: 0.0,
            cost_per_sq_ft: 0.0,
            accounting_code: String::new(),
            texture_path: None,
        }
    }

    /// Sets roughness and metallic.
    pub fn with_surface(mut self, roughness: f32, metallic: f32) -> Self {
        self.roughness = roughness;
        self.metallic = metallic;
        self
    }

    /// Sets transparency (0 opaque .. 1 clear).
    pub fn with_transparency(mut self, transparency: f32) -> Self {
        self.transparency = transparency;
        self
    }

    /// Sets the 2D drawing pattern.
    pub fn with_pattern(mut self, pattern: Pattern) -> Self {
        self.pattern = pattern;
        self
    }

    /// Sets a procedural texture and its real-world tile size in inches.
    pub fn with_texture(mut self, kind: ProceduralKind, scale_in: (f64, f64)) -> Self {
        self.texture = Texture::Procedural(kind);
        self.texture_scale_in = scale_in;
        self
    }

    /// Sets bump strength.
    pub fn with_bump(mut self, bump: f32) -> Self {
        self.bump = bump;
        self
    }

    /// Sets cost per square foot and the accounting code.
    pub fn with_cost(mut self, cost_per_sq_ft: f64, code: &str) -> Self {
        self.cost_per_sq_ft = cost_per_sq_ft;
        self.accounting_code = code.to_string();
        self
    }

    /// Base colour as RGBA floats in `0..=1` (alpha = `1 - transparency`).
    pub fn rgba_f32(&self) -> [f32; 4] {
        let c = |v: u8| f32::from(v) / 255.0;
        [
            c(self.color[0]),
            c(self.color[1]),
            c(self.color[2]),
            (1.0 - self.transparency).clamp(0.0, 1.0),
        ]
    }
}

/// How a scene material responds to light in the 3D views: the Standard
/// shader and the path tracer both read this one table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneSurface {
    /// 0 = mirror, 1 = fully diffuse (same scale as [`MaterialDef::roughness`]).
    pub roughness: f32,
    /// 0 = dielectric, 1 = metal.
    pub metallic: f32,
}

/// Roughness and metalness of a [`plan_3d::Material`] (the 23 shaded scene
/// materials; the selection tint is fully matte).
pub fn scene_surface(m: plan_3d::Material) -> SceneSurface {
    use plan_3d::Material as M;
    let (roughness, metallic) = match m {
        M::WallExterior => (0.85, 0.0),
        M::WallInterior => (0.9, 0.0),
        M::Floor => (0.4, 0.0),
        M::Ceiling => (0.95, 0.0),
        M::DoorPanel => (0.45, 0.0),
        M::WindowGlass | M::Glass => (0.0, 0.0),
        M::WindowFrame => (0.35, 0.0),
        M::Roof => (0.75, 0.0),
        M::Stucco => (0.95, 0.0),
        M::Siding => (0.8, 0.0),
        M::Brick => (0.9, 0.0),
        M::Stone => (0.85, 0.0),
        M::Concrete => (0.9, 0.0),
        M::Trim => (0.5, 0.0),
        M::Metal => (0.35, 0.9),
        M::Framing => (0.8, 0.0),
        M::Grass => (0.95, 0.0),
        M::Mulch => (0.98, 0.0),
        M::Foliage => (0.9, 0.0),
        M::Water => (0.05, 0.0),
        M::Asphalt => (0.92, 0.0),
        M::Gravel => (0.97, 0.0),
        M::Selection => (1.0, 0.0),
    };
    SceneSurface {
        roughness,
        metallic,
    }
}

#[cfg(test)]
mod scene_surface_tests {
    use super::*;

    #[test]
    fn every_scene_material_has_a_valid_surface() {
        for m in plan_3d::Material::ALL {
            let s = scene_surface(m);
            assert!((0.0..=1.0).contains(&s.roughness), "{m:?}");
            assert!((0.0..=1.0).contains(&s.metallic), "{m:?}");
        }
        assert!(scene_surface(plan_3d::Material::Metal).metallic > 0.5);
        assert!(scene_surface(plan_3d::Material::Stucco).metallic == 0.0);
        assert!(scene_surface(plan_3d::Material::WindowGlass).roughness < 0.1);
    }
}
