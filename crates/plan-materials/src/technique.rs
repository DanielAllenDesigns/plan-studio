//! Rendering-technique presets (Chief's 3D "Rendering Techniques").

use serde::{Deserialize, Serialize};

/// A named 3D look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RenderingTechnique {
    Standard,
    VectorView,
    TechnicalIllustration,
    Watercolor,
    LineDrawing,
    GlassHouse,
    PhysicallyBased,
    Clay,
    Duotone,
}

impl RenderingTechnique {
    /// Every technique, in menu order.
    pub const ALL: [RenderingTechnique; 9] = [
        RenderingTechnique::Standard,
        RenderingTechnique::VectorView,
        RenderingTechnique::TechnicalIllustration,
        RenderingTechnique::Watercolor,
        RenderingTechnique::LineDrawing,
        RenderingTechnique::GlassHouse,
        RenderingTechnique::PhysicallyBased,
        RenderingTechnique::Clay,
        RenderingTechnique::Duotone,
    ];

    /// Display name as in Chief's menu.
    pub fn label(self) -> &'static str {
        match self {
            RenderingTechnique::Standard => "Standard",
            RenderingTechnique::VectorView => "Vector View",
            RenderingTechnique::TechnicalIllustration => "Technical Illustration",
            RenderingTechnique::Watercolor => "Watercolor",
            RenderingTechnique::LineDrawing => "Line Drawing",
            RenderingTechnique::GlassHouse => "Glass House",
            RenderingTechnique::PhysicallyBased => "Physically Based",
            RenderingTechnique::Clay => "Clay",
            RenderingTechnique::Duotone => "Duotone",
        }
    }
}

/// How surfaces are shaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShadingModel {
    /// Lit by the scene's lights.
    Lit,
    /// One flat tone per face, no light gradient.
    Flat,
    /// Fill colour only, no shading at all.
    Unlit,
}

/// How faces are filled.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FillMode {
    /// Material colours / textures.
    Material,
    /// One colour for every face.
    SolidColor([u8; 3]),
    /// Translucent neutral-gray fill with the given opacity (0..1).
    Transparent(f32),
}

/// Concrete render settings for a technique.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TechniqueSettings {
    pub shading: ShadingModel,
    /// Draw silhouette / crease edge lines.
    pub edge_lines: bool,
    pub edge_color: [u8; 3],
    /// Edge width in pixels.
    pub edge_width: f32,
    pub fill: FillMode,
    pub shadows: bool,
    /// Ambient light level, 0..1.
    pub ambient: f32,
    /// Dark tone for two-tone looks (Duotone): lit faces blend from `fill`
    /// toward this colour as they fall into shade.
    pub shadow_color: Option<[u8; 3]>,
}

/// The preset for `technique`.
pub fn settings(technique: RenderingTechnique) -> TechniqueSettings {
    let base = TechniqueSettings {
        shading: ShadingModel::Lit,
        edge_lines: false,
        edge_color: [0, 0, 0],
        edge_width: 1.0,
        fill: FillMode::Material,
        shadows: false,
        ambient: 0.3,
        shadow_color: None,
    };
    match technique {
        RenderingTechnique::Standard => TechniqueSettings {
            shadows: true,
            ..base
        },
        RenderingTechnique::VectorView => TechniqueSettings {
            shading: ShadingModel::Flat,
            edge_lines: true,
            ambient: 1.0,
            ..base
        },
        RenderingTechnique::TechnicalIllustration => TechniqueSettings {
            edge_lines: true,
            edge_color: [40, 40, 40],
            ambient: 0.5,
            ..base
        },
        RenderingTechnique::Watercolor => TechniqueSettings {
            edge_lines: true,
            edge_color: [96, 78, 60],
            edge_width: 1.5,
            ambient: 0.6,
            ..base
        },
        RenderingTechnique::LineDrawing => TechniqueSettings {
            shading: ShadingModel::Unlit,
            edge_lines: true,
            fill: FillMode::SolidColor([255, 255, 255]),
            ambient: 1.0,
            ..base
        },
        RenderingTechnique::GlassHouse => TechniqueSettings {
            shading: ShadingModel::Unlit,
            edge_lines: true,
            edge_color: [64, 64, 64],
            fill: FillMode::Transparent(0.2),
            ambient: 1.0,
            ..base
        },
        RenderingTechnique::PhysicallyBased => TechniqueSettings {
            shadows: true,
            ambient: 0.2,
            ..base
        },
        RenderingTechnique::Clay => TechniqueSettings {
            fill: FillMode::SolidColor([200, 200, 200]),
            shadows: true,
            ambient: 0.35,
            ..base
        },
        RenderingTechnique::Duotone => TechniqueSettings {
            fill: FillMode::SolidColor([240, 228, 204]),
            shadows: true,
            ambient: 0.25,
            shadow_color: Some([34, 52, 86]),
            ..base
        },
    }
}
