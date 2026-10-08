//! PNG decoding for Chief's catalog thumbnails and the pictures tools.
//!
//! The decoder itself is `plan_library::image::png` (every color type and bit
//! depth, `tRNS`, interlacing; no checksums verified, the source is a local
//! file). This module keeps the [`Rgba`] type the Library Browser and the
//! pictures, underlay and material tools share.

use eframe::egui::ColorImage;

/// A decoded image, straight (non-premultiplied) RGBA8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Rgba {
    /// Box-filters down so the longer side is at most `max_side` (no change
    /// when already smaller).
    pub fn downscaled(&self, max_side: usize) -> Rgba {
        let longest = self.width.max(self.height);
        if max_side == 0 || longest <= max_side {
            return self.clone();
        }
        let w = (self.width * max_side / longest).max(1);
        let h = (self.height * max_side / longest).max(1);
        let mut out = vec![0u8; w * h * 4];
        for y in 0..h {
            let (y0, y1) = (
                y * self.height / h,
                ((y + 1) * self.height / h).max(y * self.height / h + 1),
            );
            for x in 0..w {
                let (x0, x1) = (
                    x * self.width / w,
                    ((x + 1) * self.width / w).max(x * self.width / w + 1),
                );
                let mut acc = [0u32; 4];
                let mut n = 0u32;
                for sy in y0..y1.min(self.height) {
                    for sx in x0..x1.min(self.width) {
                        let p = (sy * self.width + sx) * 4;
                        let a = u32::from(self.pixels[p + 3]);
                        // Weight colour by alpha so transparent pixels do not bleed.
                        for (sum, &c) in acc.iter_mut().zip(&self.pixels[p..p + 3]) {
                            *sum += u32::from(c) * a;
                        }
                        acc[3] += a;
                        n += 1;
                    }
                }
                let o = (y * w + x) * 4;
                let alpha = acc[3];
                if alpha == 0 {
                    continue;
                }
                for (dst, &sum) in out[o..o + 3].iter_mut().zip(&acc) {
                    *dst = (sum / alpha) as u8;
                }
                out[o + 3] = (alpha / n.max(1)) as u8;
            }
        }
        Rgba {
            width: w,
            height: h,
            pixels: out,
        }
    }

    /// The egui image.
    pub fn to_color_image(&self) -> ColorImage {
        ColorImage::from_rgba_unmultiplied([self.width, self.height], &self.pixels)
    }
}

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Decodes PNG bytes to RGBA8 with the shared decoder in
/// `plan_library::image` (all color types and depths, palettes, `tRNS` and
/// interlaced files).
pub fn decode(png: &[u8]) -> Result<Rgba, String> {
    if png.len() < 8 || png[..8] != SIGNATURE {
        return Err("not a PNG".into());
    }
    let img = plan_library::image::png::decode(png).map_err(|e| e.to_string())?;
    Ok(Rgba {
        width: img.width as usize,
        height: img.height as usize,
        pixels: img.rgba,
    })
}

#[cfg(test)]
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i32::from(a), i32::from(b), i32::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Encodes `raw` (unfiltered samples, `stride` bytes a row) as a PNG with
    /// stored deflate blocks, applying filter `ft` to every row.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode(
        w: u32,
        h: u32,
        depth: u8,
        ctype: u8,
        raw: &[u8],
        stride: usize,
        bpp: usize,
        ft: u8,
        extra: &[(&[u8; 4], Vec<u8>)],
    ) -> Vec<u8> {
        let mut filtered = Vec::new();
        for y in 0..h as usize {
            filtered.push(ft);
            for x in 0..stride {
                let cur = raw[y * stride + x];
                let a = if x >= bpp {
                    raw[y * stride + x - bpp]
                } else {
                    0
                };
                let b = if y > 0 { raw[(y - 1) * stride + x] } else { 0 };
                let c = if y > 0 && x >= bpp {
                    raw[(y - 1) * stride + x - bpp]
                } else {
                    0
                };
                let pred = match ft {
                    0 => 0,
                    1 => a,
                    2 => b,
                    3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
                    _ => paeth(a, b, c),
                };
                filtered.push(cur.wrapping_sub(pred));
            }
        }
        // zlib header + one stored block per 60000 bytes + a dummy adler.
        let mut z = vec![0x78, 0x01];
        let chunks: Vec<&[u8]> = filtered.chunks(60000).collect();
        for (i, c) in chunks.iter().enumerate() {
            z.push(u8::from(i + 1 == chunks.len()));
            z.extend((c.len() as u16).to_le_bytes());
            z.extend((!(c.len() as u16)).to_le_bytes());
            z.extend_from_slice(c);
        }
        z.extend([0, 0, 0, 0]);
        let mut png = SIGNATURE.to_vec();
        let mut chunk = |kind: &[u8; 4], body: &[u8]| {
            png.extend((body.len() as u32).to_be_bytes());
            png.extend(kind);
            png.extend(body);
            png.extend([0, 0, 0, 0]);
        };
        let mut ihdr = Vec::new();
        ihdr.extend(w.to_be_bytes());
        ihdr.extend(h.to_be_bytes());
        ihdr.extend([depth, ctype, 0, 0, 0]);
        chunk(b"IHDR", &ihdr);
        for (k, b) in extra {
            chunk(k, b);
        }
        chunk(b"IDAT", &z);
        chunk(b"IEND", &[]);
        png
    }

    #[test]
    fn decodes_rgba_with_every_filter() {
        let (w, h) = (5usize, 4usize);
        let raw: Vec<u8> = (0..w * h * 4).map(|i| (i * 7 % 251) as u8).collect();
        for ft in 0..=4 {
            let png = encode(w as u32, h as u32, 8, 6, &raw, w * 4, 4, ft, &[]);
            let img = decode(&png).unwrap();
            assert_eq!((img.width, img.height), (w, h), "filter {ft}");
            assert_eq!(img.pixels, raw, "filter {ft}");
        }
    }

    #[test]
    fn decodes_rgb_grey_and_palette() {
        let rgb = [10, 20, 30, 40, 50, 60];
        let img = decode(&encode(2, 1, 8, 2, &rgb, 6, 3, 1, &[])).unwrap();
        assert_eq!(img.pixels, [10, 20, 30, 255, 40, 50, 60, 255]);

        let grey_alpha = [100, 200, 50, 0];
        let img = decode(&encode(2, 1, 8, 4, &grey_alpha, 4, 2, 0, &[])).unwrap();
        assert_eq!(img.pixels, [100, 100, 100, 200, 50, 50, 50, 0]);

        // 2-bit palette, 4 pixels in one byte: indices 0,1,2,1.
        let plte = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let trns = vec![0u8, 128];
        let byte = 0b00_01_10_01;
        let png = encode(
            4,
            1,
            2,
            3,
            &[byte],
            1,
            1,
            0,
            &[(b"PLTE", plte), (b"tRNS", trns)],
        );
        let img = decode(&png).unwrap();
        assert_eq!(
            img.pixels,
            [1, 2, 3, 0, 4, 5, 6, 128, 7, 8, 9, 255, 4, 5, 6, 128]
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(decode(b"nope").is_err());
        let mut png = encode(2, 2, 8, 6, &[0; 16], 8, 4, 0, &[]);
        png[16..20].copy_from_slice(&0u32.to_be_bytes()); // width 0
        assert!(decode(&png).is_err());
        let short = encode(2, 2, 8, 6, &[0; 16], 8, 4, 0, &[]);
        assert!(decode(&short[..short.len() - 30]).is_err());
    }

    #[test]
    fn downscale_keeps_colour_and_size_limits() {
        let img = Rgba {
            width: 8,
            height: 4,
            pixels: [200, 100, 50, 255].repeat(32),
        };
        let small = img.downscaled(4);
        assert_eq!((small.width, small.height), (4, 2));
        assert_eq!(&small.pixels[..4], &[200, 100, 50, 255]);
        assert_eq!(img.downscaled(64), img);
        assert_eq!(small.to_color_image().size, [4, 2]);
    }

    /// Decodes up to 300 real thumbnails from each of a few installed
    /// catalogs; needs Daniel's Chief install, run with `--ignored`.
    #[test]
    #[ignore = "reads the real Chief X18 install"]
    fn real_thumbnails_decode() {
        let lib = crate::tools::library::chief::discover(None);
        let (mut ok, mut bad) = (0, 0);
        let mut catalogs = 0;
        for (i, e) in lib.catalogs().iter().enumerate() {
            if e.path.is_none() || catalogs >= 12 {
                continue;
            }
            let Ok(cat) = lib.open(i) else { continue };
            let Ok(objs) = cat.objects() else { continue };
            catalogs += 1;
            for o in objs.filter(|o| o.has_thumbnail).take(300) {
                let Ok(Some(bytes)) = cat.thumbnail(o.library_object_id) else {
                    continue;
                };
                match decode(&bytes) {
                    Ok(img) if img.width > 0 && img.pixels.len() == img.width * img.height * 4 => {
                        ok += 1
                    }
                    other => {
                        bad += 1;
                        eprintln!("{} / {}: {:?}", e.name, o.name, other.err());
                    }
                }
            }
        }
        eprintln!("{catalogs} catalogs: {ok} thumbnails decoded, {bad} failed");
        assert!(ok > 100 && bad == 0);
    }
}
