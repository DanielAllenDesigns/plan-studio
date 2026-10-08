//! Chief-style material system as data: material definitions with plan and
//! elevation patterns and 3D appearance, a core library, 2D hatch generators,
//! procedural 3D texture bitmaps, per-component assignments, rendering
//! technique presets and sun / light settings.
//!
//! All lengths are inches.

mod assign;
mod library;
mod material;
mod noise;
mod painter;
mod pattern;
mod sun;
mod technique;
mod texture;
pub mod textures;

pub use assign::{default_assignments_for, MaterialAssignment};
pub use library::{core_library, MaterialLibrary};
pub use material::{scene_surface, MaterialDef, ProceduralKind, SceneSurface, Texture};
pub use painter::{build_material, nearest_by_color, scene_material};
pub use pattern::{clip_strokes_to_polygon, pattern_strokes, Pattern, MAX_STROKES};
pub use sun::{default_room_light, LightKind, LightSource, SunSettings};
pub use technique::{settings, FillMode, RenderingTechnique, ShadingModel, TechniqueSettings};
pub use texture::{render_texture, DEFAULT_TEXTURE_SIZE, MAX_TEXTURE_SIZE};

#[cfg(test)]
mod tests;
