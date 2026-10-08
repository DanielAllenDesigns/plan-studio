//! Render configuration.

use crate::tonemap::ToneMap;

/// How surfaces are shaded (Chief's rendering techniques C-45, C-51, C-52).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Technique {
    /// Full path tracing with per-material roughness and glass.
    #[default]
    PhysicallyBased,
    /// Every surface light-grey diffuse; glass is clear.
    Clay,
    /// Ambient occlusion only.
    Ambient,
}

/// Parameters of one render.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSettings {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Samples per pixel.
    pub samples: u32,
    /// Maximum diffuse/glossy bounces per path.
    pub max_bounces: u32,
    /// Worker threads; `0` uses all available cores.
    pub threads: usize,
    /// Display transform.
    pub tone_map: ToneMap,
    /// Linear exposure multiplier applied before tone mapping.
    pub exposure: f32,
    /// Run a light bilateral filter on the HDR buffer.
    pub denoise: bool,
    /// Shading technique.
    pub technique: Technique,
    /// Seed for the per-pixel random streams; same seed, same image.
    pub seed: u64,
}

impl Default for RenderSettings {
    fn default() -> Self {
        RenderSettings {
            width: 640,
            height: 480,
            samples: 64,
            max_bounces: 4,
            threads: 0,
            tone_map: ToneMap::Aces,
            exposure: 1.0,
            denoise: false,
            technique: Technique::PhysicallyBased,
            seed: 0x5EED_CAFE,
        }
    }
}
