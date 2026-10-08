//! Render configuration.

use crate::tonemap::ToneMap;
use serde::{Deserialize, Serialize};

/// Largest image side a [`RenderSettings::scaled`] result may have, pixels.
pub const MAX_SIDE: u32 = 8192;
/// Largest pixel count a [`RenderSettings::scaled`] result may have.
pub const MAX_PIXELS: u64 = 24_000_000;

/// How surfaces are shaded (Chief's rendering techniques C-45, C-51, C-52).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// Denoise the HDR buffer: a bilateral filter guided by the albedo,
    /// normal and depth of the first hit, so textures and geometry edges stay
    /// sharp while path-tracing noise is smoothed.
    pub denoise: bool,
    /// Shading technique.
    pub technique: Technique,
    /// Seed for the per-pixel random streams; same seed, same image.
    pub seed: u64,
    /// Paint textured materials (brick, siding, shingles, wood...) with their
    /// bitmaps in the physically based technique; off renders flat colors.
    pub textures: bool,
    /// Sample area lights directly (next-event estimation). Off, they are
    /// found only by chance by bounced rays: unbiased but very noisy, kept for
    /// comparison and tests.
    pub next_event: bool,
    /// Report a coarse blocky preview (one sample per 4 x 4 block) through the
    /// progressive callback before the first full pass.
    pub preview_blocks: bool,
}

impl RenderSettings {
    /// The same render at `factor` times the width and height ("Save Image"
    /// at 2x or 4x). The factor is reduced until the image fits
    /// [`MAX_SIDE`] and [`MAX_PIXELS`]; `factor <= 1` returns a copy.
    pub fn scaled(&self, factor: u32) -> RenderSettings {
        let mut k = factor.max(1);
        let (w, h) = (u64::from(self.width.max(1)), u64::from(self.height.max(1)));
        while k > 1
            && (w * u64::from(k) > u64::from(MAX_SIDE)
                || h * u64::from(k) > u64::from(MAX_SIDE)
                || w * h * u64::from(k) * u64::from(k) > MAX_PIXELS)
        {
            k -= 1;
        }
        RenderSettings {
            width: self.width.max(1) * k,
            height: self.height.max(1) * k,
            ..self.clone()
        }
    }
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
            textures: true,
            next_event: true,
            preview_blocks: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_json() {
        let s = RenderSettings {
            width: 321,
            height: 123,
            samples: 17,
            exposure: 1.75,
            denoise: true,
            technique: Technique::Clay,
            tone_map: ToneMap::Reinhard,
            next_event: false,
            preview_blocks: true,
            ..RenderSettings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: RenderSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        // Older files lack the newer fields and still load.
        let old: RenderSettings = serde_json::from_str(r#"{"samples": 9}"#).unwrap();
        assert_eq!(old.samples, 9);
        assert_eq!(old.width, RenderSettings::default().width);
    }

    #[test]
    fn scaling_multiplies_and_is_capped() {
        let s = RenderSettings {
            width: 1280,
            height: 960,
            ..RenderSettings::default()
        };
        assert_eq!(s.scaled(1).width, 1280);
        let two = s.scaled(2);
        assert_eq!((two.width, two.height), (2560, 1920));
        let four = s.scaled(4);
        assert_eq!((four.width, four.height), (5120, 3840));
        // 4000 x 3000 at 2x is already 48 megapixels: over the pixel cap.
        let big = RenderSettings {
            width: 4000,
            height: 3000,
            ..RenderSettings::default()
        };
        let capped = big.scaled(4);
        assert!(capped.width <= MAX_SIDE && capped.height <= MAX_SIDE);
        assert!(u64::from(capped.width) * u64::from(capped.height) <= MAX_PIXELS);
        assert_eq!((capped.width, capped.height), (4000, 3000));
    }
}
