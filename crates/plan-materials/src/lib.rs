//! Chief-style material system as data: material definitions with plan and
//! elevation patterns and 3D appearance, a core library, 2D hatch generators,
//! procedural 3D texture bitmaps, per-component assignments, rendering
//! technique presets and sun / light settings.
//!
//! All lengths are inches.

mod assign;
mod blend;
mod library;
mod material;
mod noise;
mod painter;
mod pattern;
mod sun;
mod takeoff;
mod technique;
mod texture;
pub mod textures;
mod xform;

pub use assign::{default_assignments_for, MaterialAssignment};
pub use blend::{blend_materials, blend_name, parse_blend_name, BLEND_PREFIX};
pub use library::{core_library, MaterialLibrary};
pub use material::{
    scene_surface, MaterialClass, MaterialDef, PriceUnit, ProceduralKind, SceneSurface,
    SurfaceProps, Texture,
};
pub use painter::{build_material, nearest_by_color, scene_material, PaintMode, PaintScope};
pub use pattern::{
    clip_strokes_to_polygon, pattern_strokes, pattern_strokes_turned, Pattern, MAX_STROKES,
};
pub use sun::{default_room_light, LightKind, LightSource, SunSettings};
pub use takeoff::{summarize, to_csv, MaterialQuantity, Region, SurfaceArea, SURFACE_COLUMNS};
pub use technique::{settings, FillMode, RenderingTechnique, ShadingModel, TechniqueSettings};
pub use texture::{render_texture, DEFAULT_TEXTURE_SIZE, MAX_TEXTURE_SIZE};
pub use xform::{
    filter_texture_files, has_texture_transform, list_texture_files, transform_rgba, TextureFile,
};

#[cfg(test)]
mod tests;
