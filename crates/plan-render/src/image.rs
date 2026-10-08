//! Rendered image container.

use crate::tonemap::{srgb_byte, ToneMap};

/// A finished render: display bytes plus the linear HDR buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Tone-mapped sRGB RGBA8, row-major from the top-left (alpha is 255).
    pub rgba: Vec<u8>,
    /// Linear radiance per pixel (after any denoising, before exposure).
    pub hdr: Vec<[f32; 3]>,
}

impl Image {
    /// Tone map a linear buffer into an image.
    pub fn from_hdr(
        width: u32,
        height: u32,
        hdr: Vec<[f32; 3]>,
        tone_map: ToneMap,
        exposure: f32,
    ) -> Image {
        let mut rgba = Vec::with_capacity(hdr.len() * 4);
        for &px in &hdr {
            let [r, g, b] = tone_map.apply(px, exposure);
            rgba.extend_from_slice(&[srgb_byte(r), srgb_byte(g), srgb_byte(b), 255]);
        }
        Image {
            width,
            height,
            rgba,
            hdr,
        }
    }

    /// Linear HDR value at `(x, y)`.
    pub fn hdr_at(&self, x: u32, y: u32) -> [f32; 3] {
        self.hdr[y as usize * self.width as usize + x as usize]
    }
}
