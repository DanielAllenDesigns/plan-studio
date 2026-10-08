//! The 3D backdrop picture (`docs/parity/3d-views-cameras.md`, C-70): an
//! image drawn behind the model instead of the sky gradient.
//!
//! The pixels are decoded by the caller from a file in Chief's Backdrops
//! folder at run time; nothing here ships a picture. The plain-Rust parts
//! (validation, shrinking, the "cover" fit) are unit tested; the GL upload and
//! the sky pass that samples the texture live in `gpu`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Longest side kept on the GPU, pixels; bigger pictures are shrunk.
pub const MAX_BACKDROP_SIDE: u32 = 2048;

/// A decoded backdrop picture: straight sRGB RGBA8, top row first.
#[derive(Clone, Debug)]
pub struct BackdropImage {
    /// Identifies the pixels: two images with the same key share one upload.
    pub key: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
}

impl PartialEq for BackdropImage {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.width == other.width && self.height == other.height
    }
}

impl BackdropImage {
    /// Wraps decoded pixels, shrinking them (box filter) so the longer side is
    /// at most [`MAX_BACKDROP_SIDE`]. `None` when the buffer does not match
    /// the size or the picture is empty.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
            return None;
        }
        let (width, height, rgba) = shrink(width, height, rgba);
        let mut h = DefaultHasher::new();
        (width, height).hash(&mut h);
        rgba.hash(&mut h);
        Some(BackdropImage {
            key: h.finish(),
            width,
            height,
            rgba: Arc::new(rgba),
        })
    }

    /// Width over height.
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }

    /// The mean colour, display-encoded 0..1 (the viewport's background while
    /// the picture is not on the GPU yet, and the ground tint).
    pub fn average(&self) -> [f32; 3] {
        let mut sum = [0.0_f64; 3];
        let px = self.rgba.as_chunks::<4>().0;
        for p in px {
            for (s, c) in sum.iter_mut().zip(p) {
                *s += f64::from(*c);
            }
        }
        let n = px.len().max(1) as f64 * 255.0;
        [
            (sum[0] / n) as f32,
            (sum[1] / n) as f32,
            (sum[2] / n) as f32,
        ]
    }
}

/// Texture-coordinate scale that makes a picture of `image_aspect` cover a
/// view of `view_aspect` (centred, the overflow cropped): multiply the
/// distance of a screen coordinate from the centre by this.
pub fn cover_scale(image_aspect: f32, view_aspect: f32) -> [f32; 2] {
    if !(image_aspect.is_finite() && view_aspect.is_finite())
        || image_aspect <= 0.0
        || view_aspect <= 0.0
    {
        return [1.0, 1.0];
    }
    if view_aspect > image_aspect {
        // The view is wider: the picture's full width, a band of its height.
        [1.0, image_aspect / view_aspect]
    } else {
        [view_aspect / image_aspect, 1.0]
    }
}

fn shrink(width: u32, height: u32, rgba: Vec<u8>) -> (u32, u32, Vec<u8>) {
    let longest = width.max(height);
    if longest <= MAX_BACKDROP_SIDE {
        return (width, height, rgba);
    }
    let f = (longest as f64 / f64::from(MAX_BACKDROP_SIDE)).ceil() as u32;
    let (nw, nh) = ((width / f).max(1), (height / f).max(1));
    let mut out = Vec::with_capacity(nw as usize * nh as usize * 4);
    for y in 0..nh {
        for x in 0..nw {
            let mut acc = [0_u32; 4];
            let mut n = 0_u32;
            for dy in 0..f {
                for dx in 0..f {
                    let (sx, sy) = (x * f + dx, y * f + dy);
                    if sx < width && sy < height {
                        let i = (sy as usize * width as usize + sx as usize) * 4;
                        for c in 0..4 {
                            acc[c] += u32::from(rgba[i + c]);
                        }
                        n += 1;
                    }
                }
            }
            let n = n.max(1);
            out.extend(acc.iter().map(|a| (a / n) as u8));
        }
    }
    (nw, nh, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> Vec<u8> {
        (0..w * h).flat_map(|_| c).collect()
    }

    #[test]
    fn a_mismatched_buffer_is_refused() {
        assert!(BackdropImage::new(0, 4, vec![]).is_none());
        assert!(BackdropImage::new(2, 2, vec![0; 15]).is_none());
        assert!(BackdropImage::new(2, 2, vec![0; 16]).is_some());
    }

    #[test]
    fn the_key_follows_the_pixels() {
        let a = BackdropImage::new(2, 2, solid(2, 2, [10, 20, 30, 255])).unwrap();
        let b = BackdropImage::new(2, 2, solid(2, 2, [10, 20, 30, 255])).unwrap();
        let c = BackdropImage::new(2, 2, solid(2, 2, [11, 20, 30, 255])).unwrap();
        assert_eq!(a, b);
        assert_ne!(a.key, c.key);
    }

    #[test]
    fn big_pictures_are_shrunk_to_the_gpu_limit() {
        let img = BackdropImage::new(4096, 1024, solid(4096, 1024, [200, 100, 50, 255])).unwrap();
        assert_eq!((img.width, img.height), (2048, 512));
        assert_eq!(img.rgba.len(), 2048 * 512 * 4);
        assert_eq!(&img.rgba[..4], &[200, 100, 50, 255]);
        let avg = img.average();
        assert!((avg[0] - 200.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn cover_fills_the_view_and_crops_the_overflow() {
        // A square picture in a wide view shows a horizontal band.
        let s = cover_scale(1.0, 2.0);
        assert_eq!(s, [1.0, 0.5]);
        // A wide picture in a square view shows its middle.
        let s = cover_scale(2.0, 1.0);
        assert_eq!(s, [0.5, 1.0]);
        assert_eq!(cover_scale(1.5, 1.5), [1.0, 1.0]);
        assert_eq!(cover_scale(f32::NAN, 1.0), [1.0, 1.0]);
    }
}
