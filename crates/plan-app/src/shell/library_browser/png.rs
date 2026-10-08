//! A small dependency-free PNG decoder for Chief's catalog thumbnails.
//!
//! Handles every colour type (grey, RGB, palette, grey+alpha, RGBA) at 8 or
//! 16 bits, plus 1/2/4-bit grey and palette, all five scanline filters and
//! `tRNS`. Interlaced images are rejected (Chief's thumbnails are 256 x 256
//! RGBA, not interlaced). Inflate comes from `plan_calib`; chunk CRCs and the
//! zlib checksum are not verified (the source is a local file).

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

fn be32(b: &[u8]) -> usize {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize
}

/// Decodes PNG bytes to RGBA8.
pub fn decode(png: &[u8]) -> Result<Rgba, String> {
    if png.len() < 8 || png[..8] != SIGNATURE {
        return Err("not a PNG".into());
    }
    let (mut w, mut h, mut depth, mut ctype, mut interlace) = (0, 0, 0u8, 0u8, 0u8);
    let mut have_header = false;
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    let mut pos = 8;
    while pos + 8 <= png.len() {
        let len = be32(&png[pos..]);
        let kind = &png[pos + 4..pos + 8];
        let start = pos + 8;
        let end = start.checked_add(len).filter(|&e| e <= png.len());
        let Some(end) = end else {
            return Err("truncated chunk".into());
        };
        let body = &png[start..end];
        match kind {
            b"IHDR" => {
                if body.len() < 13 {
                    return Err("short IHDR".into());
                }
                w = be32(body);
                h = be32(&body[4..]);
                depth = body[8];
                ctype = body[9];
                interlace = body[12];
                have_header = true;
            }
            b"PLTE" => palette = body.as_chunks::<3>().0.to_vec(),
            b"tRNS" => trns = body.to_vec(),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        pos = end + 4;
    }
    if !have_header || w == 0 || h == 0 || w > 16384 || h > 16384 {
        return Err("bad image size".into());
    }
    if interlace != 0 {
        return Err("interlaced PNG".into());
    }
    let channels = match ctype {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return Err("bad colour type".into()),
    };
    let ok_depth = match ctype {
        0 | 3 => matches!(depth, 1 | 2 | 4 | 8) || (ctype == 0 && depth == 16),
        _ => matches!(depth, 8 | 16),
    };
    if !ok_depth {
        return Err("unsupported bit depth".into());
    }
    if idat.len() < 6 {
        return Err("no image data".into());
    }
    let raw = plan_calib::inflate::inflate_vec(&idat[2..]).map_err(|e| e.to_string())?;
    let bits_per_px = channels * usize::from(depth);
    let stride = (w * bits_per_px).div_ceil(8);
    let bpp = bits_per_px.div_ceil(8).max(1);
    if raw.len() < (stride + 1) * h {
        return Err("image data too short".into());
    }

    // Undo the filters.
    let mut img = vec![0u8; stride * h];
    for y in 0..h {
        let line = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        let (ft, src) = (line[0], &line[1..]);
        let (done, rest) = img.split_at_mut(y * stride);
        let prev: &[u8] = if y == 0 {
            &[]
        } else {
            &done[(y - 1) * stride..]
        };
        let cur = &mut rest[..stride];
        for x in 0..stride {
            let a = if x >= bpp { cur[x - bpp] } else { 0 };
            let b = prev.get(x).copied().unwrap_or(0);
            let c = if x >= bpp {
                prev.get(x - bpp).copied().unwrap_or(0)
            } else {
                0
            };
            let add = match ft {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
                4 => paeth(a, b, c),
                _ => return Err("bad filter".into()),
            };
            cur[x] = src[x].wrapping_add(add);
        }
    }

    // Expand to RGBA8.
    let sample = |row: &[u8], i: usize| -> u8 {
        // The i-th sample of a row, scaled for sub-byte depths only by the
        // caller (palette indices stay raw).
        match depth {
            8 => row[i],
            16 => row[i * 2],
            d => {
                let per = 8 / usize::from(d);
                let byte = row[i / per];
                let shift = 8 - usize::from(d) * (i % per + 1);
                (byte >> shift) & ((1u8 << d) - 1)
            }
        }
    };
    let mut out = vec![0u8; w * h * 4];
    let grey_key = (ctype == 0 && trns.len() >= 2).then(|| u16::from_be_bytes([trns[0], trns[1]]));
    for y in 0..h {
        let row = &img[y * stride..(y + 1) * stride];
        for x in 0..w {
            let o = (y * w + x) * 4;
            let px: [u8; 4] = match ctype {
                0 => {
                    let raw_v = sample(row, x);
                    let v = if depth < 8 {
                        raw_v * (255 / ((1u8 << depth) - 1))
                    } else {
                        raw_v
                    };
                    let key_hit = grey_key.is_some_and(|k| match depth {
                        16 => u16::from(raw_v) == k >> 8,
                        _ => u16::from(raw_v) == k,
                    });
                    [v, v, v, if key_hit { 0 } else { 255 }]
                }
                2 => [
                    sample(row, x * 3),
                    sample(row, x * 3 + 1),
                    sample(row, x * 3 + 2),
                    255,
                ],
                3 => {
                    let i = usize::from(sample(row, x));
                    let c = palette.get(i).copied().unwrap_or([0, 0, 0]);
                    [c[0], c[1], c[2], trns.get(i).copied().unwrap_or(255)]
                }
                4 => {
                    let v = sample(row, x * 2);
                    [v, v, v, sample(row, x * 2 + 1)]
                }
                _ => [
                    sample(row, x * 4),
                    sample(row, x * 4 + 1),
                    sample(row, x * 4 + 2),
                    sample(row, x * 4 + 3),
                ],
            };
            out[o..o + 4].copy_from_slice(&px);
        }
    }
    Ok(Rgba {
        width: w,
        height: h,
        pixels: out,
    })
}

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
