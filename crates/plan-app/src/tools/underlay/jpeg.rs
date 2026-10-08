//! A small baseline JPEG decoder for underlay pictures (no new crates).
//!
//! Decodes sequential Huffman JPEG (SOF0 and SOF1, 8-bit): grayscale and
//! YCbCr (or Adobe RGB) with any 1x or 2x sampling, restart intervals and
//! non-interleaved scans. Progressive, arithmetic-coded, lossless and CMYK
//! files are refused with a message (save them as PNG or a baseline JPEG).
//! Chroma planes are upsampled by replication; the result is straight RGBA8
//! like the PNG decoder's.

use crate::shell::library_browser::png::Rgba;

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Pixels the decoder will build (a guard against absurd headers).
const MAX_PIXELS: usize = 120_000_000;

/// A canonical Huffman table.
#[derive(Clone, Default)]
struct Huff {
    mincode: [i32; 17],
    maxcode: [i32; 17],
    valptr: [i32; 17],
    vals: Vec<u8>,
    present: bool,
}

impl Huff {
    fn new(counts: &[u8; 16], vals: Vec<u8>) -> Huff {
        let mut h = Huff {
            vals,
            present: true,
            ..Huff::default()
        };
        let (mut code, mut k) = (0i32, 0i32);
        for len in 1..=16 {
            let n = i32::from(counts[len - 1]);
            h.valptr[len] = k;
            h.mincode[len] = code;
            code += n;
            k += n;
            h.maxcode[len] = if n > 0 { code - 1 } else { -1 };
            code <<= 1;
        }
        h
    }
}

#[derive(Clone, Default)]
struct Component {
    id: u8,
    h: usize,
    v: usize,
    tq: usize,
    /// Blocks per row and column of the (MCU-padded) plane.
    bw: usize,
    bh: usize,
    plane: Vec<u8>,
    pred: i32,
    dc: usize,
    ac: usize,
}

/// The entropy-coded data of a scan: bytes with `FF00` stuffing.
struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    acc: u32,
    n: u32,
}

impl Bits<'_> {
    fn bit(&mut self) -> u32 {
        if self.n == 0 {
            self.acc = 0;
            if let Some(&b) = self.d.get(self.pos) {
                if b == 0xFF {
                    // `FF 00` is a stuffed byte; any other `FF xx` is a marker
                    // that ends the data, and the rest reads as zeros.
                    if self.d.get(self.pos + 1) == Some(&0) {
                        self.pos += 2;
                        self.acc = 0xFF;
                    }
                } else {
                    self.pos += 1;
                    self.acc = u32::from(b);
                }
            }
            self.n = 8;
        }
        self.n -= 1;
        (self.acc >> self.n) & 1
    }

    fn receive(&mut self, s: u32) -> i32 {
        (0..s).fold(0i32, |v, _| (v << 1) | self.bit() as i32)
    }

    fn huff(&mut self, t: &Huff) -> Result<u8, String> {
        let mut code = 0i32;
        for len in 1..=16 {
            code = (code << 1) | self.bit() as i32;
            if code <= t.maxcode[len] {
                let i = t.valptr[len] + code - t.mincode[len];
                return t
                    .vals
                    .get(usize::try_from(i).map_err(|_| "bad Huffman code")?)
                    .copied()
                    .ok_or_else(|| "bad Huffman code".to_string());
            }
        }
        Err("bad Huffman code".to_string())
    }

    /// Skips to just past the next restart marker.
    fn restart(&mut self) {
        self.n = 0;
        self.acc = 0;
        while self.pos + 1 < self.d.len() {
            if self.d[self.pos] == 0xFF && (0xD0..=0xD7).contains(&self.d[self.pos + 1]) {
                self.pos += 2;
                return;
            }
            if self.d[self.pos] == 0xFF && self.d[self.pos + 1] != 0 && self.d[self.pos + 1] != 0xFF
            {
                // Some other marker: the data ended early.
                return;
            }
            self.pos += 1;
        }
    }
}

fn extend(v: i32, s: u32) -> i32 {
    if s == 0 {
        0
    } else if v < (1 << (s - 1)) {
        v - (1 << s) + 1
    } else {
        v
    }
}

/// `COS[x][u]`: the 1-D inverse DCT basis, scaled so two passes give the 2-D
/// transform.
fn cos_table() -> [[f32; 8]; 8] {
    let mut t = [[0.0f32; 8]; 8];
    for (x, row) in t.iter_mut().enumerate() {
        for (u, c) in row.iter_mut().enumerate() {
            let cu = if u == 0 {
                std::f32::consts::FRAC_1_SQRT_2
            } else {
                1.0
            };
            *c = 0.5 * cu * (((2 * x + 1) * u) as f32 * std::f32::consts::PI / 16.0).cos();
        }
    }
    t
}

fn idct(coef: &[i32; 64], cos: &[[f32; 8]; 8], out: &mut [u8; 64]) {
    let mut tmp = [0.0f32; 64];
    // Rows: tmp[v][x] = sum_u cos[x][u] * F[v][u]
    for v in 0..8 {
        for x in 0..8 {
            let mut s = 0.0;
            for u in 0..8 {
                s += cos[x][u] * coef[v * 8 + u] as f32;
            }
            tmp[v * 8 + x] = s;
        }
    }
    for x in 0..8 {
        for y in 0..8 {
            let mut s = 0.0;
            for v in 0..8 {
                s += cos[y][v] * tmp[v * 8 + x];
            }
            out[y * 8 + x] = (s + 128.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn be16(d: &[u8], i: usize) -> Result<usize, String> {
    match (d.get(i), d.get(i + 1)) {
        (Some(a), Some(b)) => Ok(usize::from(*a) << 8 | usize::from(*b)),
        _ => Err("truncated JPEG".to_string()),
    }
}

/// Decodes a baseline JPEG.
pub fn decode(d: &[u8]) -> Result<Rgba, String> {
    if d.len() < 4 || d[0] != 0xFF || d[1] != 0xD8 {
        return Err("not a JPEG".to_string());
    }
    let cos = cos_table();
    let mut qt = [[0i32; 64]; 4];
    let mut dc_tables: [Huff; 4] = Default::default();
    let mut ac_tables: [Huff; 4] = Default::default();
    let mut comps: Vec<Component> = Vec::new();
    let (mut width, mut height) = (0usize, 0usize);
    let (mut hmax, mut vmax) = (1usize, 1usize);
    let mut restart_interval = 0usize;
    let mut adobe_transform: Option<u8> = None;
    let mut scans = 0;
    let mut pos = 2;
    loop {
        // Find the next marker.
        while pos < d.len() && d[pos] != 0xFF {
            pos += 1;
        }
        while pos < d.len() && d[pos] == 0xFF {
            pos += 1;
        }
        let Some(&marker) = d.get(pos) else { break };
        pos += 1;
        match marker {
            0xD8 | 0x01 | 0xD0..=0xD7 | 0x00 => continue,
            0xD9 => break,
            _ => {}
        }
        let len = be16(d, pos)?;
        let body = d.get(pos + 2..pos + len).ok_or("truncated JPEG")?;
        match marker {
            0xC0 | 0xC1 => {
                if body.len() < 6 || body[0] != 8 {
                    return Err("only 8-bit JPEG is supported".to_string());
                }
                height = usize::from(body[1]) << 8 | usize::from(body[2]);
                width = usize::from(body[3]) << 8 | usize::from(body[4]);
                let n = usize::from(body[5]);
                if width == 0 || height == 0 || width * height > MAX_PIXELS {
                    return Err("bad JPEG size".to_string());
                }
                if !(n == 1 || n == 3) {
                    return Err("only grayscale and 3-component JPEG is supported".to_string());
                }
                if body.len() < 6 + 3 * n {
                    return Err("truncated JPEG".to_string());
                }
                comps = (0..n)
                    .map(|i| {
                        let b = &body[6 + 3 * i..9 + 3 * i];
                        Component {
                            id: b[0],
                            h: usize::from(b[1] >> 4).clamp(1, 4),
                            v: usize::from(b[1] & 15).clamp(1, 4),
                            tq: usize::from(b[2] & 3),
                            ..Component::default()
                        }
                    })
                    .collect();
                hmax = comps.iter().map(|c| c.h).max().unwrap_or(1);
                vmax = comps.iter().map(|c| c.v).max().unwrap_or(1);
                let mcux = width.div_ceil(8 * hmax);
                let mcuy = height.div_ceil(8 * vmax);
                for c in &mut comps {
                    c.bw = mcux * c.h;
                    c.bh = mcuy * c.v;
                    c.plane = vec![0; c.bw * 8 * c.bh * 8];
                }
            }
            0xC2 => {
                return Err(
                    "progressive JPEG is not supported: save it as PNG or a baseline JPEG"
                        .to_string(),
                )
            }
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => {
                return Err(
                    "this kind of JPEG (lossless or arithmetic coded) is not supported".to_string(),
                )
            }
            0xC4 => {
                let mut i = 0;
                while i + 17 <= body.len() {
                    let (class, id) = (body[i] >> 4, usize::from(body[i] & 3));
                    let mut counts = [0u8; 16];
                    counts.copy_from_slice(&body[i + 1..i + 17]);
                    let total: usize = counts.iter().map(|c| usize::from(*c)).sum();
                    let vals = body
                        .get(i + 17..i + 17 + total)
                        .ok_or("truncated JPEG")?
                        .to_vec();
                    let t = Huff::new(&counts, vals);
                    if class == 0 {
                        dc_tables[id] = t;
                    } else {
                        ac_tables[id] = t;
                    }
                    i += 17 + total;
                }
            }
            0xDB => {
                let mut i = 0;
                while i < body.len() {
                    let (prec, id) = (body[i] >> 4, usize::from(body[i] & 3));
                    i += 1;
                    for k in 0..64 {
                        let v = if prec == 0 {
                            i32::from(*body.get(i).ok_or("truncated JPEG")?)
                        } else {
                            i32::from(*body.get(i).ok_or("truncated JPEG")?) << 8
                                | i32::from(*body.get(i + 1).ok_or("truncated JPEG")?)
                        };
                        i += if prec == 0 { 1 } else { 2 };
                        qt[id][ZIGZAG[k]] = v;
                    }
                }
            }
            0xDD => restart_interval = be16(body, 0)?,
            0xEE if body.len() >= 12 && &body[..5] == b"Adobe" => adobe_transform = Some(body[11]),
            0xDA => {
                if comps.is_empty() {
                    return Err("JPEG scan before its frame header".to_string());
                }
                let ns = usize::from(*body.first().ok_or("truncated JPEG")?);
                if body.len() < 1 + 2 * ns || ns == 0 || ns > comps.len() {
                    return Err("bad JPEG scan".to_string());
                }
                let mut order = Vec::new();
                for i in 0..ns {
                    let cid = body[1 + 2 * i];
                    let t = body[2 + 2 * i];
                    let ci = comps
                        .iter()
                        .position(|c| c.id == cid)
                        .ok_or("JPEG scan names an unknown component")?;
                    comps[ci].dc = usize::from(t >> 4) & 3;
                    comps[ci].ac = usize::from(t & 15) & 3;
                    order.push(ci);
                }
                let start = pos + len;
                let used = decode_scan(
                    &d[start.min(d.len())..],
                    &mut comps,
                    &order,
                    (width, height, hmax, vmax),
                    restart_interval,
                    (&qt, &dc_tables, &ac_tables),
                    &cos,
                )?;
                scans += 1;
                pos = start + used;
                continue;
            }
            _ => {}
        }
        pos += len;
    }
    if scans == 0 || comps.is_empty() {
        return Err("the JPEG holds no image data".to_string());
    }
    // Assemble.
    let mut pixels = vec![255u8; width * height * 4];
    let rgb_direct = comps.len() == 3 && adobe_transform == Some(0);
    for y in 0..height {
        for x in 0..width {
            let mut s = [0i32; 3];
            for (k, c) in comps.iter().enumerate() {
                let cx = x * c.h / hmax;
                let cy = y * c.v / vmax;
                s[k] = i32::from(c.plane[cy * c.bw * 8 + cx]);
            }
            let o = (y * width + x) * 4;
            let (r, g, b) = if comps.len() == 1 {
                (s[0], s[0], s[0])
            } else if rgb_direct {
                (s[0], s[1], s[2])
            } else {
                let (yy, cb, cr) = (s[0] as f32, s[1] as f32 - 128.0, s[2] as f32 - 128.0);
                (
                    (yy + 1.402 * cr).round() as i32,
                    (yy - 0.344_136 * cb - 0.714_136 * cr).round() as i32,
                    (yy + 1.772 * cb).round() as i32,
                )
            };
            pixels[o] = r.clamp(0, 255) as u8;
            pixels[o + 1] = g.clamp(0, 255) as u8;
            pixels[o + 2] = b.clamp(0, 255) as u8;
        }
    }
    Ok(Rgba {
        width,
        height,
        pixels,
    })
}

/// Decodes one scan into the component planes; returns how many bytes of `d`
/// it used.
#[allow(clippy::too_many_arguments)]
fn decode_scan(
    d: &[u8],
    comps: &mut [Component],
    order: &[usize],
    (width, height, hmax, vmax): (usize, usize, usize, usize),
    restart_interval: usize,
    (qt, dc_tables, ac_tables): (&[[i32; 64]; 4], &[Huff; 4], &[Huff; 4]),
    cos: &[[f32; 8]; 8],
) -> Result<usize, String> {
    for &ci in order {
        if !dc_tables[comps[ci].dc].present || !ac_tables[comps[ci].ac].present {
            return Err("JPEG scan uses a Huffman table that was not defined".to_string());
        }
    }
    let interleaved = order.len() > 1;
    // MCUs across and down.
    let (cols, rows) = if interleaved {
        (width.div_ceil(8 * hmax), height.div_ceil(8 * vmax))
    } else {
        let c = &comps[order[0]];
        (
            (width * c.h).div_ceil(hmax).div_ceil(8),
            (height * c.v).div_ceil(vmax).div_ceil(8),
        )
    };
    let mut bits = Bits {
        d,
        pos: 0,
        acc: 0,
        n: 0,
    };
    for &ci in order {
        comps[ci].pred = 0;
    }
    let mut coef = [0i32; 64];
    let mut block = [0u8; 64];
    let mut mcu = 0usize;
    for my in 0..rows {
        for mx in 0..cols {
            if restart_interval > 0 && mcu > 0 && mcu.is_multiple_of(restart_interval) {
                bits.restart();
                for &ci in order {
                    comps[ci].pred = 0;
                }
            }
            mcu += 1;
            for &ci in order {
                let (bh_n, bv_n) = if interleaved {
                    (comps[ci].h, comps[ci].v)
                } else {
                    (1, 1)
                };
                for by in 0..bv_n {
                    for bx in 0..bh_n {
                        let (blk_x, blk_y) = if interleaved {
                            (mx * comps[ci].h + bx, my * comps[ci].v + by)
                        } else {
                            (mx, my)
                        };
                        coef.fill(0);
                        let q = &qt[comps[ci].tq];
                        // DC.
                        let s = u32::from(bits.huff(&dc_tables[comps[ci].dc])? & 15);
                        let diff = extend(bits.receive(s), s);
                        comps[ci].pred += diff;
                        coef[0] = comps[ci].pred * q[0];
                        // AC.
                        let mut k = 1;
                        while k < 64 {
                            let rs = bits.huff(&ac_tables[comps[ci].ac])?;
                            let (r, s) = (usize::from(rs >> 4), u32::from(rs & 15));
                            if s == 0 {
                                if r == 15 {
                                    k += 16;
                                    continue;
                                }
                                break;
                            }
                            k += r;
                            if k > 63 {
                                break;
                            }
                            let z = ZIGZAG[k];
                            coef[z] = extend(bits.receive(s), s) * q[z];
                            k += 1;
                        }
                        idct(&coef, cos, &mut block);
                        let c = &mut comps[ci];
                        if blk_x < c.bw && blk_y < c.bh {
                            let stride = c.bw * 8;
                            for yy in 0..8 {
                                let o = (blk_y * 8 + yy) * stride + blk_x * 8;
                                c.plane[o..o + 8].copy_from_slice(&block[yy * 8..yy * 8 + 8]);
                            }
                        }
                    }
                }
            }
        }
    }
    // The scan ends at the next marker (not a restart).
    let mut end = bits.pos.min(d.len());
    while end + 1 < d.len()
        && !(d[end] == 0xFF && d[end + 1] != 0 && !(0xD0..=0xD7).contains(&d[end + 1]))
    {
        end += 1;
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(name: &str, jpg: &[u8], rgb: &[u8], w: usize, h: usize) {
        let img = decode(jpg).unwrap();
        assert_eq!((img.width, img.height), (w, h));
        let mut total = 0u64;
        let mut worst = 0i32;
        for i in 0..w * h {
            for c in 0..3 {
                let a = i32::from(img.pixels[i * 4 + c]);
                let b = i32::from(rgb[i * 3 + c]);
                total += (a - b).unsigned_abs() as u64;
                worst = worst.max((a - b).abs());
            }
            assert_eq!(img.pixels[i * 4 + 3], 255);
        }
        let mean = total as f64 / (w * h * 3) as f64;
        assert!(mean < 2.5, "{name}: mean error {mean}");
        assert!(worst <= 30, "{name}: worst error {worst}");
    }

    #[test]
    fn matches_libjpeg_on_444_420_gray_and_restart_files() {
        check(
            "rgb444",
            include_bytes!("testdata/rgb444.jpg"),
            include_bytes!("testdata/rgb444.rgb"),
            40,
            24,
        );
        // Sizes that are not a multiple of the MCU, chroma at half size.
        check(
            "rgb420",
            include_bytes!("testdata/rgb420.jpg"),
            include_bytes!("testdata/rgb420.rgb"),
            37,
            29,
        );
        check(
            "gray",
            include_bytes!("testdata/gray.jpg"),
            include_bytes!("testdata/gray.rgb"),
            21,
            17,
        );
        // A restart marker every two MCUs.
        check(
            "restart",
            include_bytes!("testdata/restart.jpg"),
            include_bytes!("testdata/restart.rgb"),
            40,
            24,
        );
    }

    #[test]
    fn refuses_progressive_and_junk_without_panicking() {
        let err = decode(include_bytes!("testdata/progressive.jpg")).unwrap_err();
        assert!(err.contains("progressive"), "{err}");
        assert!(decode(b"not a jpeg").is_err());
        assert!(decode(&[0xFF, 0xD8, 0xFF, 0xD9]).is_err());
        // Truncated files fail or decode partially, never panic.
        let good = include_bytes!("testdata/rgb420.jpg");
        for cut in [10, 100, 300, good.len() - 20] {
            let _ = decode(&good[..cut]);
        }
    }

    #[test]
    fn the_idct_of_a_dc_only_block_is_flat() {
        let cos = cos_table();
        let mut coef = [0i32; 64];
        coef[0] = 8 * 10; // DC of 10 levels above mid-grey
        let mut out = [0u8; 64];
        idct(&coef, &cos, &mut out);
        assert!(out.iter().all(|v| *v == 138), "{out:?}");
    }
}
