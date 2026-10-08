//! PNG decoder: every color type and bit depth (1, 2, 4, 8, 16), palettes,
//! `tRNS` transparency (also color-keyed grey and RGB), all five scanline
//! filters and Adam7 interlacing. 16-bit samples keep their high byte. Chunk
//! CRCs are not verified (the source is a local file); gamma/ICC chunks are
//! ignored, so the result is whatever color space the file stored (sRGB for
//! practically every texture).

use super::{corrupt, inflate::zlib_decompress, Error, Rgba8Image, MAX_DIMENSION, MAX_PIXELS};

/// The 8-byte PNG file signature.
pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

fn be32(b: &[u8]) -> usize {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize
}

#[derive(Clone, Copy)]
struct Header {
    w: usize,
    h: usize,
    depth: usize,
    ctype: u8,
    interlace: bool,
    channels: usize,
}

/// Decodes PNG bytes to RGBA8.
pub fn decode(png: &[u8]) -> Result<Rgba8Image, Error> {
    if png.len() < 8 || png[..8] != SIGNATURE {
        return Err(corrupt("missing PNG signature"));
    }
    let mut hdr: Option<Header> = None;
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    let mut pos = 8;
    while pos + 8 <= png.len() {
        let len = be32(&png[pos..]);
        let kind = &png[pos + 4..pos + 8];
        let start = pos + 8;
        let Some(end) = start.checked_add(len).filter(|&e| e <= png.len()) else {
            return Err(corrupt("truncated chunk"));
        };
        let body = &png[start..end];
        match kind {
            b"IHDR" => {
                if body.len() < 13 {
                    return Err(corrupt("short IHDR"));
                }
                let (depth, ctype) = (usize::from(body[8]), body[9]);
                let channels = match ctype {
                    0 | 3 => 1,
                    2 => 3,
                    4 => 2,
                    6 => 4,
                    _ => return Err(corrupt("bad color type")),
                };
                let ok = match ctype {
                    0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
                    3 => matches!(depth, 1 | 2 | 4 | 8),
                    _ => matches!(depth, 8 | 16),
                };
                if !ok {
                    return Err(corrupt("bad bit depth"));
                }
                hdr = Some(Header {
                    w: be32(body),
                    h: be32(&body[4..]),
                    depth,
                    ctype,
                    interlace: body[12] == 1,
                    channels,
                });
            }
            b"PLTE" => {
                palette = body
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|c| [c[0], c[1], c[2]])
                    .collect()
            }
            b"tRNS" => trns = body.to_vec(),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        pos = end + 4;
    }
    let hd = hdr.ok_or_else(|| corrupt("missing IHDR"))?;
    if hd.w == 0
        || hd.h == 0
        || hd.w > MAX_DIMENSION as usize
        || hd.h > MAX_DIMENSION as usize
        || (hd.w as u64) * (hd.h as u64) > MAX_PIXELS
    {
        return Err(corrupt("bad image size"));
    }
    if hd.ctype == 3 && palette.is_empty() {
        return Err(corrupt("palette image without PLTE"));
    }
    let bits_pp = hd.channels * hd.depth;
    let row_bytes = |w: usize| (w * bits_pp).div_ceil(8);
    let expect: usize = if hd.interlace {
        ADAM7
            .iter()
            .map(|&(xs, ys, xo, yo)| {
                let (pw, ph) = pass_dims(hd.w, hd.h, xs, ys, xo, yo);
                if pw == 0 || ph == 0 {
                    0
                } else {
                    ph * (1 + row_bytes(pw))
                }
            })
            .sum()
    } else {
        hd.h * (1 + row_bytes(hd.w))
    };
    let raw = zlib_decompress(&idat, expect, expect + 1024)?;
    if raw.len() < expect {
        return Err(corrupt("image data is short"));
    }
    let mut out = vec![0u8; hd.w * hd.h * 4];
    let ctx = Ctx {
        hd,
        palette: &palette,
        trns: &trns,
    };
    if hd.interlace {
        let mut off = 0;
        for &(xs, ys, xo, yo) in &ADAM7 {
            let (pw, ph) = pass_dims(hd.w, hd.h, xs, ys, xo, yo);
            if pw == 0 || ph == 0 {
                continue;
            }
            let rb = row_bytes(pw);
            let mut pass = raw[off..off + ph * (1 + rb)].to_vec();
            off += ph * (1 + rb);
            unfilter(&mut pass, ph, rb, bits_pp)?;
            for py in 0..ph {
                let row = &pass[py * (1 + rb) + 1..py * (1 + rb) + 1 + rb];
                for px in 0..pw {
                    let rgba = ctx.pixel(row, px);
                    let (x, y) = (xo + px * xs, yo + py * ys);
                    out[(y * hd.w + x) * 4..(y * hd.w + x) * 4 + 4].copy_from_slice(&rgba);
                }
            }
        }
    } else {
        let rb = row_bytes(hd.w);
        let mut data = raw;
        data.truncate(expect);
        unfilter(&mut data, hd.h, rb, bits_pp)?;
        for y in 0..hd.h {
            let row = &data[y * (1 + rb) + 1..y * (1 + rb) + 1 + rb];
            let dst = &mut out[y * hd.w * 4..(y + 1) * hd.w * 4];
            ctx.row(row, dst);
        }
    }
    Ok(Rgba8Image {
        width: hd.w as u32,
        height: hd.h as u32,
        rgba: out,
    })
}

/// `(x step, y step, x offset, y offset)` of the seven Adam7 passes.
const ADAM7: [(usize, usize, usize, usize); 7] = [
    (8, 8, 0, 0),
    (8, 8, 4, 0),
    (4, 8, 0, 4),
    (4, 4, 2, 0),
    (2, 4, 0, 2),
    (2, 2, 1, 0),
    (1, 2, 0, 1),
];

fn pass_dims(w: usize, h: usize, xs: usize, ys: usize, xo: usize, yo: usize) -> (usize, usize) {
    (
        w.saturating_sub(xo).div_ceil(xs),
        h.saturating_sub(yo).div_ceil(ys),
    )
}

/// Reverses the scanline filters in place; each row is `1 + rb` bytes.
#[allow(clippy::needless_range_loop)]
fn unfilter(data: &mut [u8], rows: usize, rb: usize, bits_pp: usize) -> Result<(), Error> {
    let bpp = bits_pp.div_ceil(8).max(1);
    let stride = rb + 1;
    for y in 0..rows {
        let (before, cur) = data.split_at_mut(y * stride);
        let prev: Option<&[u8]> = if y == 0 {
            None
        } else {
            Some(&before[(y - 1) * stride + 1..y * stride])
        };
        let filter = cur[0];
        let row = &mut cur[1..stride];
        let up = |i: usize| prev.map_or(0, |p| p[i]);
        match filter {
            0 => {}
            1 => {
                for i in bpp..rb {
                    row[i] = row[i].wrapping_add(row[i - bpp]);
                }
            }
            2 => {
                for i in 0..rb {
                    row[i] = row[i].wrapping_add(up(i));
                }
            }
            3 => {
                for i in 0..rb {
                    let left = if i >= bpp { u16::from(row[i - bpp]) } else { 0 };
                    row[i] = row[i].wrapping_add(((left + u16::from(up(i))) / 2) as u8);
                }
            }
            4 => {
                for i in 0..rb {
                    let a = if i >= bpp { i32::from(row[i - bpp]) } else { 0 };
                    let b = i32::from(up(i));
                    let c = if i >= bpp {
                        prev.map_or(0, |p| i32::from(p[i - bpp]))
                    } else {
                        0
                    };
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                    let pred = if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    };
                    row[i] = row[i].wrapping_add(pred as u8);
                }
            }
            _ => return Err(corrupt("bad PNG filter type")),
        }
    }
    Ok(())
}

struct Ctx<'a> {
    hd: Header,
    palette: &'a [[u8; 3]],
    trns: &'a [u8],
}

impl Ctx<'_> {
    /// One sample (`idx` counts samples in the row) as its raw integer value.
    fn sample(&self, row: &[u8], idx: usize) -> u16 {
        match self.hd.depth {
            8 => u16::from(row[idx]),
            16 => u16::from(row[idx * 2]) << 8 | u16::from(row[idx * 2 + 1]),
            d => {
                let per = 8 / d;
                let byte = row[idx / per];
                let shift = 8 - d * (idx % per + 1);
                u16::from((byte >> shift) & ((1u8 << d) - 1))
            }
        }
    }

    /// Scales a sample to 8 bits (grey and color samples).
    fn to8(&self, v: u16) -> u8 {
        match self.hd.depth {
            16 => (v >> 8) as u8,
            8 => v as u8,
            d => (u32::from(v) * 255 / ((1u32 << d) - 1)) as u8,
        }
    }

    fn pixel(&self, row: &[u8], x: usize) -> [u8; 4] {
        let ch = self.hd.channels;
        let s = |k: usize| self.sample(row, x * ch + k);
        match self.hd.ctype {
            0 => {
                let v = s(0);
                let g = self.to8(v);
                let a = if self.trns.len() >= 2
                    && v == u16::from_be_bytes([self.trns[0], self.trns[1]])
                {
                    0
                } else {
                    255
                };
                [g, g, g, a]
            }
            2 => {
                let (r, g, b) = (s(0), s(1), s(2));
                let a = if self.trns.len() >= 6
                    && r == u16::from_be_bytes([self.trns[0], self.trns[1]])
                    && g == u16::from_be_bytes([self.trns[2], self.trns[3]])
                    && b == u16::from_be_bytes([self.trns[4], self.trns[5]])
                {
                    0
                } else {
                    255
                };
                [self.to8(r), self.to8(g), self.to8(b), a]
            }
            3 => {
                let i = usize::from(s(0));
                let c = self.palette.get(i).copied().unwrap_or([0, 0, 0]);
                [c[0], c[1], c[2], self.trns.get(i).copied().unwrap_or(255)]
            }
            4 => {
                let g = self.to8(s(0));
                [g, g, g, self.to8(s(1))]
            }
            _ => [
                self.to8(s(0)),
                self.to8(s(1)),
                self.to8(s(2)),
                self.to8(s(3)),
            ],
        }
    }

    fn row(&self, row: &[u8], dst: &mut [u8]) {
        let w = self.hd.w;
        // Fast paths for the overwhelmingly common 8-bit layouts.
        if self.hd.depth == 8 && self.trns.is_empty() {
            match self.hd.ctype {
                6 => {
                    dst.copy_from_slice(&row[..w * 4]);
                    return;
                }
                2 => {
                    for (d, s) in dst
                        .as_chunks_mut::<4>()
                        .0
                        .iter_mut()
                        .zip(row.as_chunks::<3>().0)
                    {
                        d.copy_from_slice(&[s[0], s[1], s[2], 255]);
                    }
                    return;
                }
                0 => {
                    for (d, &g) in dst.as_chunks_mut::<4>().0.iter_mut().zip(row) {
                        d.copy_from_slice(&[g, g, g, 255]);
                    }
                    return;
                }
                _ => {}
            }
        }
        for x in 0..w {
            dst[x * 4..x * 4 + 4].copy_from_slice(&self.pixel(row, x));
        }
    }
}

/// Encodes straight RGBA8 as an (uncompressed, stored-block) PNG. Used by
/// tests and tools; not meant for shipping large images.
pub fn encode_rgba(img: &Rgba8Image) -> Vec<u8> {
    let mut raw = Vec::with_capacity((img.width as usize * 4 + 1) * img.height as usize);
    for y in 0..img.height as usize {
        raw.push(0);
        raw.extend_from_slice(
            &img.rgba[y * img.width as usize * 4..(y + 1) * img.width as usize * 4],
        );
    }
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&img.width.to_be_bytes());
    ihdr.extend_from_slice(&img.height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = SIGNATURE.to_vec();
    write_chunk(&mut out, b"IHDR", &ihdr);
    write_chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    write_chunk(&mut out, b"IEND", &[]);
    out
}

/// zlib stream made of stored deflate blocks.
pub(crate) fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    if raw.is_empty() {
        z.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(c) = chunks.next() {
        z.push(u8::from(chunks.peek().is_none()));
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &v in raw {
        a = (a + u32::from(v)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&(b << 16 | a).to_be_bytes());
    z
}

pub(crate) fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in kind.iter().chain(body) {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                0xEDB8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    out.extend_from_slice(&(!crc).to_be_bytes());
}
