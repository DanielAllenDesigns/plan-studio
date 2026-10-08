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

/// The Properties tab's material class: what kind of surface the material is.
/// A class sets the starting values of the sliders and keeps them in the
/// range that makes the class true (a mirror is always metallic and smooth);
/// the GL view and the ray tracer read the result through
/// [`MaterialDef::surface`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MaterialClass {
    #[default]
    General,
    Plastic,
    Metal,
    Glass,
    Mirror,
    Emissive,
    Transparent,
}

impl MaterialClass {
    /// Every class in the order of the Properties tab's list.
    pub const ALL: [MaterialClass; 7] = [
        MaterialClass::General,
        MaterialClass::Plastic,
        MaterialClass::Metal,
        MaterialClass::Glass,
        MaterialClass::Mirror,
        MaterialClass::Emissive,
        MaterialClass::Transparent,
    ];

    /// The name shown in the dialog.
    pub fn name(self) -> &'static str {
        match self {
            MaterialClass::General => "General",
            MaterialClass::Plastic => "Plastic",
            MaterialClass::Metal => "Metal",
            MaterialClass::Glass => "Glass",
            MaterialClass::Mirror => "Mirror",
            MaterialClass::Emissive => "Emissive",
            MaterialClass::Transparent => "Transparent",
        }
    }

    /// The class called `name` (case-insensitive).
    pub fn from_name(name: &str) -> Option<MaterialClass> {
        Self::ALL
            .into_iter()
            .find(|c| c.name().eq_ignore_ascii_case(name.trim()))
    }

    /// Typical slider values of the class.
    pub fn typical(self) -> SurfaceProps {
        let (roughness, metallic, transparency, emissive) = match self {
            MaterialClass::General => (0.8, 0.0, 0.0, 0.0),
            MaterialClass::Plastic => (0.35, 0.0, 0.0, 0.0),
            MaterialClass::Metal => (0.35, 0.9, 0.0, 0.0),
            MaterialClass::Glass => (0.02, 0.0, 0.85, 0.0),
            MaterialClass::Mirror => (0.0, 1.0, 0.0, 0.0),
            MaterialClass::Emissive => (0.8, 0.0, 0.0, 0.8),
            MaterialClass::Transparent => (0.6, 0.0, 0.5, 0.0),
        };
        SurfaceProps {
            roughness,
            metallic,
            transparency,
            emissive,
        }
    }
}

/// How a material responds to light, after its class has been applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceProps {
    /// 0 = mirror, 1 = fully diffuse.
    pub roughness: f32,
    /// 0 = dielectric, 1 = metal.
    pub metallic: f32,
    /// 0 = opaque, 1 = fully clear.
    pub transparency: f32,
    /// Self-illumination, 0..1.
    pub emissive: f32,
}

/// The unit a material's price is quoted in (Materials List tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PriceUnit {
    #[default]
    SqFt,
    SqYd,
    SqM,
    LinFt,
    Each,
}

impl PriceUnit {
    pub const ALL: [PriceUnit; 5] = [
        PriceUnit::SqFt,
        PriceUnit::SqYd,
        PriceUnit::SqM,
        PriceUnit::LinFt,
        PriceUnit::Each,
    ];

    /// The label of the unit ("sq ft").
    pub fn label(self) -> &'static str {
        match self {
            PriceUnit::SqFt => "sq ft",
            PriceUnit::SqYd => "sq yd",
            PriceUnit::SqM => "sq m",
            PriceUnit::LinFt => "lin ft",
            PriceUnit::Each => "each",
        }
    }

    /// Square feet in one unit of an area unit; `None` for the others.
    pub fn sq_ft(self) -> Option<f64> {
        match self {
            PriceUnit::SqFt => Some(1.0),
            PriceUnit::SqYd => Some(9.0),
            PriceUnit::SqM => Some(10.763_910_416_709_722),
            PriceUnit::LinFt | PriceUnit::Each => None,
        }
    }
}

fn one() -> f64 {
    1.0
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
    /// Material class (Properties tab).
    #[serde(default)]
    pub class: MaterialClass,
    /// Size of the plan / elevation hatch relative to its usual size (Pattern
    /// tab); 1 = as drawn by [`Pattern`].
    #[serde(default = "one")]
    pub pattern_scale: f64,
    /// Rotation of the plan / elevation hatch, degrees counter-clockwise.
    #[serde(default)]
    pub pattern_angle: f64,
    /// Where the texture starts, inches `(across, down)` (Texture tab).
    #[serde(default)]
    pub texture_offset_in: (f64, f64),
    /// Texture rotation, degrees.
    #[serde(default)]
    pub texture_angle_deg: f64,
    /// Colour mixed into the texture; used when `blend_amount` > 0.
    #[serde(default)]
    pub blend_color: Option<[u8; 3]>,
    /// How much of `blend_color` the texture takes, 0..1.
    #[serde(default)]
    pub blend_amount: f32,
    /// Materials List tab: who makes it.
    #[serde(default)]
    pub manufacturer: String,
    /// Materials List tab: who sells it.
    #[serde(default)]
    pub supplier: String,
    /// Materials List tab: price per `unit`. 0 falls back to
    /// `cost_per_sq_ft`.
    #[serde(default)]
    pub price: f64,
    /// Materials List tab: the unit of `price`.
    #[serde(default)]
    pub unit: PriceUnit,
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
            class: MaterialClass::General,
            pattern_scale: 1.0,
            pattern_angle: 0.0,
            texture_offset_in: (0.0, 0.0),
            texture_angle_deg: 0.0,
            blend_color: None,
            blend_amount: 0.0,
            manufacturer: String::new(),
            supplier: String::new(),
            price: 0.0,
            unit: PriceUnit::SqFt,
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

    /// Sets the material class and moves the sliders to its typical values.
    pub fn with_class(mut self, class: MaterialClass) -> Self {
        self.set_class(class);
        self
    }

    /// Changes the class; the surface sliders take the class's typical
    /// values (the Properties tab does this when a class is picked).
    pub fn set_class(&mut self, class: MaterialClass) {
        self.class = class;
        let t = class.typical();
        self.roughness = t.roughness;
        self.metallic = t.metallic;
        self.transparency = t.transparency;
        self.emissive = t.emissive;
    }

    /// The surface the renderers use: the sliders, held to what the class
    /// requires (a mirror is metallic and smooth, glass is clear and smooth,
    /// plastic is glossy, an emissive material glows).
    pub fn surface(&self) -> SurfaceProps {
        let mut s = SurfaceProps {
            roughness: self.roughness.clamp(0.0, 1.0),
            metallic: self.metallic.clamp(0.0, 1.0),
            transparency: self.transparency.clamp(0.0, 1.0),
            emissive: self.emissive.clamp(0.0, 1.0),
        };
        match self.class {
            MaterialClass::General => {}
            MaterialClass::Plastic => {
                s.metallic = 0.0;
                s.roughness = s.roughness.min(0.5);
            }
            MaterialClass::Metal => s.metallic = s.metallic.max(0.8),
            MaterialClass::Glass => {
                s.metallic = 0.0;
                s.roughness = s.roughness.min(0.1);
                s.transparency = s.transparency.max(0.5);
            }
            MaterialClass::Mirror => {
                s.metallic = 1.0;
                s.roughness = s.roughness.min(0.05);
                s.transparency = 0.0;
            }
            MaterialClass::Emissive => s.emissive = s.emissive.max(0.3),
            MaterialClass::Transparent => s.transparency = s.transparency.max(0.2),
        }
        s
    }

    /// The price and unit the Materials List uses: the quoted `price` and
    /// `unit`, else the older per-square-foot cost.
    pub fn quoted_price(&self) -> (f64, PriceUnit) {
        if self.price > 0.0 {
            (self.price, self.unit)
        } else {
            (self.cost_per_sq_ft, PriceUnit::SqFt)
        }
    }

    /// Cost of one square foot of the surface, 0 for a unit that is not an
    /// area.
    pub fn price_per_sq_ft(&self) -> f64 {
        let (p, u) = self.quoted_price();
        u.sq_ft().map_or(0.0, |sq| p / sq)
    }

    /// The plan / elevation hatch of this material: the pattern at the Pattern
    /// tab's scale, turned by its angle, over `rect` (inches), for a drawing
    /// at `scale_in_per_ft` paper inches per foot. Clip the strokes to the
    /// polygon they fill ([`crate::clip_strokes_to_polygon`]).
    pub fn hatch_strokes(
        &self,
        rect: (plan_core::Point, plan_core::Point),
        scale_in_per_ft: f64,
    ) -> Vec<(plan_core::Point, plan_core::Point)> {
        crate::pattern_strokes_turned(
            &self.pattern,
            rect,
            scale_in_per_ft,
            self.pattern_scale,
            self.pattern_angle,
        )
    }

    /// Base colour as RGBA floats in `0..=1` (alpha = `1 - transparency`).
    pub fn rgba_f32(&self) -> [f32; 4] {
        let c = |v: u8| f32::from(v) / 255.0;
        [
            c(self.color[0]),
            c(self.color[1]),
            c(self.color[2]),
            (1.0 - self.surface().transparency).clamp(0.0, 1.0),
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
