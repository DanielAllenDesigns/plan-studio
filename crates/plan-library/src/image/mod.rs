//! Dependency-free image decoding for textures and pictures.
//!
//! [`decode`] reads PNG (every color type and bit depth, palette, `tRNS`,
//! Adam7) and JPEG (baseline and progressive Huffman; grayscale, YCbCr, RGB
//! and Adobe CMYK/YCCK; 4:4:4, 4:2:2, 4:2:0 and other sampling; restart
//! markers; the EXIF orientation tag is ignored) into straight, non-premultiplied
//! [`Rgba8Image`]s. [`Rgba8Image::downscaled`] and [`mipmaps`] are box filters.
//!
//! Decoding is single-threaded. A 2048 x 2048 4:2:0 JPEG decodes in well under
//! 150 ms in a release build and under about a second in a dev build
//! (`opt-level = 1`); see the `#[ignore]`d test `chief_textures_decode_fast`.

use std::fmt;

mod inflate;
pub mod jpeg;
pub mod png;

/// Largest width or height a decoder accepts.
pub const MAX_DIMENSION: u32 = 16_384;
/// Largest pixel count a decoder accepts (guards absurd headers).
pub const MAX_PIXELS: u64 = 120_000_000;

/// Why an image could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Neither a PNG nor a JPEG signature.
    UnknownFormat,
    /// A valid file using a feature this decoder does not implement.
    Unsupported(String),
    /// Damaged or truncated data.
    Corrupt(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownFormat => write!(f, "not a PNG or JPEG image"),
            Error::Unsupported(m) => write!(f, "unsupported image: {m}"),
            Error::Corrupt(m) => write!(f, "damaged image: {m}"),
        }
    }
}

impl std::error::Error for Error {}

pub(crate) fn corrupt(msg: &str) -> Error {
    Error::Corrupt(msg.to_string())
}

pub(crate) fn unsupported(msg: &str) -> Error {
    Error::Unsupported(msg.to_string())
}

/// A decoded image: straight (non-premultiplied) sRGB RGBA8, row-major, top
/// row first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba8Image {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// Decodes PNG or JPEG bytes (the format is sniffed from the signature).
pub fn decode(bytes: &[u8]) -> Result<Rgba8Image, Error> {
    if bytes.starts_with(&png::SIGNATURE) {
        png::decode(bytes)
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        jpeg::decode(bytes)
    } else {
        Err(Error::UnknownFormat)
    }
}

/// Reads and decodes an image file.
pub fn decode_file(path: &std::path::Path) -> Result<Rgba8Image, Error> {
    let bytes =
        std::fs::read(path).map_err(|e| Error::Corrupt(format!("{}: {e}", path.display())))?;
    decode(&bytes)
}

impl Rgba8Image {
    /// A `width` x `height` image filled with one color.
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let mut data = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..width as usize * height as usize {
            data.extend_from_slice(&rgba);
        }
        Rgba8Image {
            width,
            height,
            rgba: data,
        }
    }

    /// The pixel at `(x, y)` (clamped to the image).
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let x = x.min(self.width.saturating_sub(1)) as usize;
        let y = y.min(self.height.saturating_sub(1)) as usize;
        let o = (y * self.width as usize + x) * 4;
        [
            self.rgba[o],
            self.rgba[o + 1],
            self.rgba[o + 2],
            self.rgba[o + 3],
        ]
    }

    /// Whether any pixel is not fully opaque.
    pub fn has_alpha(&self) -> bool {
        self.rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255)
    }

    /// Bytes of pixel data held.
    pub fn byte_len(&self) -> usize {
        self.rgba.len()
    }

    /// The alpha-weighted average color (sRGB bytes, alpha = mean alpha).
    pub fn average_color(&self) -> [u8; 4] {
        let (mut acc, mut wsum, mut asum) = ([0u64; 3], 0u64, 0u64);
        for p in self.rgba.as_chunks::<4>().0 {
            let a = u64::from(p[3]);
            for k in 0..3 {
                acc[k] += u64::from(p[k]) * a;
            }
            wsum += a;
            asum += a;
        }
        let n = (self.rgba.len() / 4).max(1) as u64;
        if wsum == 0 {
            return [0, 0, 0, 0];
        }
        [
            (acc[0] / wsum) as u8,
            (acc[1] / wsum) as u8,
            (acc[2] / wsum) as u8,
            (asum / n) as u8,
        ]
    }

    /// Box-filters down so the longer side is at most `max_side` (a clone when
    /// already small enough). The aspect ratio is kept.
    pub fn downscaled(&self, max_side: u32) -> Rgba8Image {
        let longest = self.width.max(self.height);
        if max_side == 0 || longest <= max_side {
            return self.clone();
        }
        let w = ((u64::from(self.width) * u64::from(max_side) / u64::from(longest)) as u32).max(1);
        let h = ((u64::from(self.height) * u64::from(max_side) / u64::from(longest)) as u32).max(1);
        self.resized_box(w, h)
    }

    /// Box-filters to exactly `w` x `h` (either larger or smaller than the
    /// source is allowed; enlarging repeats pixels). Color is averaged in
    /// linear light and weighted by alpha so transparent pixels do not bleed.
    pub fn resized_box(&self, w: u32, h: u32) -> Rgba8Image {
        let (w, h) = (w.max(1), h.max(1));
        if w == self.width && h == self.height {
            return self.clone();
        }
        let lut = srgb_to_linear_lut();
        let (sw, sh) = (self.width as usize, self.height as usize);
        let mut out = vec![0u8; w as usize * h as usize * 4];
        for y in 0..h as usize {
            let y0 = y * sh / h as usize;
            let y1 = ((y + 1) * sh / h as usize).max(y0 + 1).min(sh.max(1));
            for x in 0..w as usize {
                let x0 = x * sw / w as usize;
                let x1 = ((x + 1) * sw / w as usize).max(x0 + 1).min(sw.max(1));
                let mut acc = [0.0f32; 3];
                let (mut asum, mut n) = (0.0f32, 0.0f32);
                for sy in y0..y1 {
                    for sx in x0..x1 {
                        let p = (sy * sw + sx) * 4;
                        let a = f32::from(self.rgba[p + 3]);
                        for k in 0..3 {
                            acc[k] += lut[usize::from(self.rgba[p + k])] * a;
                        }
                        asum += a;
                        n += 1.0;
                    }
                }
                let o = (y * w as usize + x) * 4;
                if asum > 0.0 {
                    for k in 0..3 {
                        out[o + k] = linear_to_srgb(acc[k] / asum);
                    }
                }
                out[o + 3] = (asum / n + 0.5) as u8;
            }
        }
        Rgba8Image {
            width: w,
            height: h,
            rgba: out,
        }
    }
}

/// The mip chain of `base` (level 0 is a clone of it), halving until 1 x 1.
pub fn mipmaps(base: &Rgba8Image) -> Vec<Rgba8Image> {
    let mut chain = vec![base.clone()];
    loop {
        let last = chain.last().expect("chain is never empty");
        if last.width <= 1 && last.height <= 1 {
            break;
        }
        let next = last.resized_box((last.width / 2).max(1), (last.height / 2).max(1));
        chain.push(next);
    }
    chain
}

fn srgb_to_linear_lut() -> [f32; 256] {
    let mut t = [0.0f32; 256];
    for (i, v) in t.iter_mut().enumerate() {
        let c = i as f32 / 255.0;
        *v = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
    }
    t
}

fn linear_to_srgb(l: f32) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let c = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests;
