//! JPEG decoder: baseline, extended-sequential and progressive Huffman JPEG,
//! 8-bit, grayscale / YCbCr / RGB / Adobe CMYK and YCCK, any sampling factors
//! (4:4:4, 4:2:2, 4:2:0, 4:4:0, 4:1:1...), restart markers and successive
//! approximation. The EXIF orientation tag is ignored; ICC profiles are
//! ignored. Arithmetic-coded, lossless, hierarchical and 12-bit files are
//! refused with [`Error::Unsupported`].
//!
//! Every scan decodes into per-component coefficient planes, so baseline and
//! progressive files share one dequantize / IDCT / upsample / color path. Chroma
//! planes are upsampled with a triangle filter (like libjpeg's "fancy"
//! upsampling) and converted with the JFIF equations.

use super::{corrupt, unsupported, Error, Rgba8Image, MAX_DIMENSION, MAX_PIXELS};

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Lookahead bits of the fast Huffman table.
const FAST_BITS: u32 = 9;

#[derive(Clone)]
struct Huff {
    present: bool,
    /// `(len << 8) | symbol` for codes up to [`FAST_BITS`] long, 0 otherwise.
    fast: Vec<u16>,
    maxcode: [i32; 17],
    valptr: [i32; 17],
    mincode: [i32; 17],
    vals: Vec<u8>,
}

impl Default for Huff {
    fn default() -> Self {
        Huff {
            present: false,
            fast: Vec::new(),
            maxcode: [-1; 17],
            valptr: [0; 17],
            mincode: [0; 17],
            vals: Vec::new(),
        }
    }
}

impl Huff {
    fn new(counts: &[u8; 16], vals: Vec<u8>) -> Huff {
        let mut h = Huff {
            present: true,
            fast: vec![0; 1 << FAST_BITS],
            vals,
            ..Huff::default()
        };
        let (mut code, mut k) = (0i32, 0i32);
        for len in 1..=16usize {
            let n = i32::from(counts[len - 1]);
            h.valptr[len] = k;
            h.mincode[len] = code;
            if n > 0 {
                h.maxcode[len] = code + n - 1;
                if len as u32 <= FAST_BITS {
                    let shift = FAST_BITS - len as u32;
                    for i in 0..n {
                        let c = (code + i) as usize;
                        let sym = u16::from(h.vals.get((k + i) as usize).copied().unwrap_or(0));
                        if c >= (1usize << len) {
                            continue; // over-subscribed table: ignore the excess
                        }
                        for fill in 0..(1usize << shift) {
                            h.fast[(c << shift) | fill] = ((len as u16) << 8) | sym;
                        }
                    }
                }
            }
            code = (code + n) << 1;
            k += n;
        }
        h
    }
}

/// Entropy-coded segment reader (MSB-first, `FF00` unstuffing, stops at markers).
struct Reader<'a> {
    d: &'a [u8],
    pos: usize,
    acc: u32,
    n: u32,
    hit_marker: bool,
}

impl<'a> Reader<'a> {
    fn new(d: &'a [u8], pos: usize) -> Self {
        Reader {
            d,
            pos,
            acc: 0,
            n: 0,
            hit_marker: false,
        }
    }

    #[inline]
    fn fill(&mut self) {
        while self.n <= 24 {
            let mut b = 0u32;
            if !self.hit_marker {
                match self.d.get(self.pos) {
                    Some(&0xFF) => match self.d.get(self.pos + 1) {
                        Some(&0) => {
                            b = 0xFF;
                            self.pos += 2;
                        }
                        _ => self.hit_marker = true,
                    },
                    Some(&v) => {
                        b = u32::from(v);
                        self.pos += 1;
                    }
                    None => self.hit_marker = true,
                }
            }
            self.acc |= b << (24 - self.n);
            self.n += 8;
        }
    }

    #[inline]
    fn peek(&mut self, k: u32) -> u32 {
        if self.n < k {
            self.fill();
        }
        self.acc >> (32 - k)
    }

    #[inline]
    fn skip(&mut self, k: u32) {
        self.acc <<= k;
        self.n -= k;
    }

    #[inline]
    fn get(&mut self, k: u32) -> u32 {
        if k == 0 {
            return 0;
        }
        let v = self.peek(k);
        self.skip(k);
        v
    }

    #[inline]
    fn bit(&mut self) -> bool {
        self.get(1) == 1
    }

    #[inline]
    fn huff(&mut self, t: &Huff) -> Result<u8, Error> {
        let look = self.peek(FAST_BITS) as usize;
        let e = t.fast[look];
        if e != 0 {
            self.skip(u32::from(e >> 8));
            return Ok(e as u8);
        }
        let v = self.peek(16) as i32;
        for len in (FAST_BITS as usize + 1)..=16 {
            let code = v >> (16 - len);
            if t.maxcode[len] >= 0 && code <= t.maxcode[len] {
                let i = t.valptr[len] + code - t.mincode[len];
                self.skip(len as u32);
                return t
                    .vals
                    .get(usize::try_from(i).map_err(|_| corrupt("bad Huffman code"))?)
                    .copied()
                    .ok_or_else(|| corrupt("bad Huffman code"));
            }
        }
        Err(corrupt("bad Huffman code"))
    }

    /// Discards buffered bits and moves to just after the next RSTn marker.
    fn restart(&mut self) {
        self.acc = 0;
        self.n = 0;
        self.hit_marker = false;
        while self.pos + 1 < self.d.len() {
            if self.d[self.pos] == 0xFF {
                let m = self.d[self.pos + 1];
                if (0xD0..=0xD7).contains(&m) {
                    self.pos += 2;
                    return;
                }
                if m != 0 && m != 0xFF {
                    self.hit_marker = true; // some other marker: data ended early
                    return;
                }
            }
            self.pos += 1;
        }
        self.hit_marker = true;
    }
}

#[inline]
fn extend(v: u32, s: u32) -> i32 {
    if s == 0 {
        0
    } else if v < (1 << (s - 1)) {
        v as i32 - (1 << s) + 1
    } else {
        v as i32
    }
}

#[derive(Clone, Default)]
struct Component {
    id: u8,
    h: usize,
    v: usize,
    tq: usize,
    /// Blocks per row / column of the MCU-padded coefficient plane.
    bw: usize,
    bh: usize,
    /// Blocks actually covering the image (for non-interleaved scans).
    real_w: usize,
    real_h: usize,
    coef: Vec<i16>,
    dc_tbl: usize,
    ac_tbl: usize,
    pred: i32,
    /// Quantization table latched at this component's first scan.
    quant: Option<[u16; 64]>,
}

struct Frame {
    width: usize,
    height: usize,
    progressive: bool,
    comps: Vec<Component>,
    hmax: usize,
    vmax: usize,
    mcux: usize,
    mcuy: usize,
}

fn be16(d: &[u8], i: usize) -> Result<usize, Error> {
    match (d.get(i), d.get(i + 1)) {
        (Some(a), Some(b)) => Ok(usize::from(*a) << 8 | usize::from(*b)),
        _ => Err(corrupt("truncated JPEG")),
    }
}

/// Decodes a JPEG file to RGBA8 (alpha 255).
pub fn decode(d: &[u8]) -> Result<Rgba8Image, Error> {
    if d.len() < 4 || d[0] != 0xFF || d[1] != 0xD8 {
        return Err(corrupt("missing JPEG signature"));
    }
    let mut qt = [[0u16; 64]; 4];
    let mut dc_tables: [Huff; 4] = Default::default();
    let mut ac_tables: [Huff; 4] = Default::default();
    let mut frame: Option<Frame> = None;
    let mut restart_interval = 0usize;
    let mut adobe: Option<u8> = None;
    let mut scans = 0;
    let mut pos = 2;
    loop {
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
        if len < 2 {
            return Err(corrupt("bad JPEG segment length"));
        }
        let body = d
            .get(pos + 2..pos + len)
            .ok_or_else(|| corrupt("truncated JPEG"))?;
        match marker {
            0xC0..=0xC2 => {
                if frame.is_some() {
                    return Err(unsupported("JPEG with several frames"));
                }
                frame = Some(parse_frame(body, marker == 0xC2)?);
            }
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => {
                return Err(unsupported(
                    "lossless, hierarchical or arithmetic-coded JPEG",
                ));
            }
            0xC4 => parse_dht(body, &mut dc_tables, &mut ac_tables)?,
            0xDB => parse_dqt(body, &mut qt)?,
            0xDD => restart_interval = be16(body, 0)?,
            0xEE if body.len() >= 12 && &body[..5] == b"Adobe" => adobe = Some(body[11]),
            0xDA => {
                let fr = frame
                    .as_mut()
                    .ok_or_else(|| corrupt("scan before frame header"))?;
                let scan = ScanCtx {
                    restart_interval,
                    qt: &qt,
                    dc_tables: &dc_tables,
                    ac_tables: &ac_tables,
                };
                pos = decode_scan(d, pos + len, body, fr, &scan)?;
                scans += 1;
                continue;
            }
            _ => {}
        }
        pos += len;
    }
    let fr = frame.ok_or_else(|| corrupt("no JPEG frame header"))?;
    if scans == 0 {
        return Err(corrupt("the JPEG holds no image data"));
    }
    Ok(assemble(&fr, adobe))
}

fn parse_frame(body: &[u8], progressive: bool) -> Result<Frame, Error> {
    if body.len() < 6 {
        return Err(corrupt("short SOF"));
    }
    if body[0] != 8 {
        return Err(unsupported("only 8-bit JPEG is supported"));
    }
    let height = be16(body, 1)?;
    let width = be16(body, 3)?;
    let n = usize::from(body[5]);
    if width == 0
        || height == 0
        || width > MAX_DIMENSION as usize
        || height > MAX_DIMENSION as usize
        || (width as u64) * (height as u64) > MAX_PIXELS
    {
        return Err(corrupt("bad JPEG size"));
    }
    if !matches!(n, 1 | 3 | 4) {
        return Err(unsupported("JPEG with an unusual component count"));
    }
    if body.len() < 6 + 3 * n {
        return Err(corrupt("short SOF"));
    }
    let mut comps: Vec<Component> = (0..n)
        .map(|i| {
            let b = &body[6 + 3 * i..9 + 3 * i];
            Component {
                id: b[0],
                h: usize::from(b[1] >> 4),
                v: usize::from(b[1] & 15),
                tq: usize::from(b[2] & 3),
                ..Component::default()
            }
        })
        .collect();
    if comps
        .iter()
        .any(|c| !(1..=4).contains(&c.h) || !(1..=4).contains(&c.v))
    {
        return Err(corrupt("bad JPEG sampling factors"));
    }
    let hmax = comps.iter().map(|c| c.h).max().unwrap_or(1);
    let vmax = comps.iter().map(|c| c.v).max().unwrap_or(1);
    let mcux = width.div_ceil(8 * hmax);
    let mcuy = height.div_ceil(8 * vmax);
    for c in &mut comps {
        c.bw = mcux * c.h;
        c.bh = mcuy * c.v;
        c.real_w = (width * c.h).div_ceil(hmax).div_ceil(8);
        c.real_h = (height * c.v).div_ceil(vmax).div_ceil(8);
        c.coef = vec![0; c.bw * c.bh * 64];
    }
    Ok(Frame {
        width,
        height,
        progressive,
        comps,
        hmax,
        vmax,
        mcux,
        mcuy,
    })
}

fn parse_dht(body: &[u8], dc: &mut [Huff; 4], ac: &mut [Huff; 4]) -> Result<(), Error> {
    let mut i = 0;
    while i + 17 <= body.len() {
        let (class, id) = (body[i] >> 4, usize::from(body[i] & 3));
        let mut counts = [0u8; 16];
        counts.copy_from_slice(&body[i + 1..i + 17]);
        let total: usize = counts.iter().map(|c| usize::from(*c)).sum();
        let vals = body
            .get(i + 17..i + 17 + total)
            .ok_or_else(|| corrupt("truncated DHT"))?
            .to_vec();
        let t = Huff::new(&counts, vals);
        if class == 0 {
            dc[id] = t;
        } else {
            ac[id] = t;
        }
        i += 17 + total;
    }
    Ok(())
}

fn parse_dqt(body: &[u8], qt: &mut [[u16; 64]; 4]) -> Result<(), Error> {
    let mut i = 0;
    while i < body.len() {
        let (prec, id) = (body[i] >> 4, usize::from(body[i] & 3));
        i += 1;
        for &z in &ZIGZAG {
            let v = if prec == 0 {
                u16::from(*body.get(i).ok_or_else(|| corrupt("truncated DQT"))?)
            } else {
                u16::from(*body.get(i).ok_or_else(|| corrupt("truncated DQT"))?) << 8
                    | u16::from(*body.get(i + 1).ok_or_else(|| corrupt("truncated DQT"))?)
            };
            i += if prec == 0 { 1 } else { 2 };
            qt[id][z] = v;
        }
    }
    Ok(())
}

/// Decoder state shared by every scan.
struct ScanCtx<'a> {
    restart_interval: usize,
    qt: &'a [[u16; 64]; 4],
    dc_tables: &'a [Huff; 4],
    ac_tables: &'a [Huff; 4],
}

/// Decodes one scan into the coefficient planes; returns the offset just past
/// its entropy-coded data (at the next marker).
fn decode_scan(
    d: &[u8],
    start: usize,
    header: &[u8],
    fr: &mut Frame,
    cx: &ScanCtx,
) -> Result<usize, Error> {
    let ns = usize::from(*header.first().ok_or_else(|| corrupt("short SOS"))?);
    if ns == 0 || ns > fr.comps.len() || header.len() < 1 + 2 * ns + 3 {
        return Err(corrupt("bad JPEG scan header"));
    }
    let mut order = Vec::with_capacity(ns);
    for i in 0..ns {
        let cid = header[1 + 2 * i];
        let t = header[2 + 2 * i];
        let ci = fr
            .comps
            .iter()
            .position(|c| c.id == cid)
            .ok_or_else(|| corrupt("scan names an unknown component"))?;
        fr.comps[ci].dc_tbl = usize::from(t >> 4) & 3;
        fr.comps[ci].ac_tbl = usize::from(t & 15) & 3;
        if fr.comps[ci].quant.is_none() {
            fr.comps[ci].quant = Some(cx.qt[fr.comps[ci].tq]);
        }
        order.push(ci);
    }
    let (mut ss, mut se) = (
        usize::from(header[1 + 2 * ns]),
        usize::from(header[2 + 2 * ns]),
    );
    let (ah, al) = (
        u32::from(header[3 + 2 * ns] >> 4),
        u32::from(header[3 + 2 * ns] & 15),
    );
    let progressive = fr.progressive;
    if !progressive {
        ss = 0;
        se = 63;
    } else if ss > se || se > 63 || al > 13 || (ss == 0 && se != 0) {
        return Err(corrupt("bad progressive scan parameters"));
    }
    let dc_scan = progressive && ss == 0;
    if progressive && !dc_scan && ns != 1 {
        return Err(corrupt("progressive AC scan with several components"));
    }
    for &ci in &order {
        let c = &fr.comps[ci];
        let need_dc = !progressive || (dc_scan && ah == 0);
        let need_ac = !progressive || !dc_scan;
        if need_dc && !cx.dc_tables[c.dc_tbl].present {
            return Err(corrupt("scan uses an undefined DC Huffman table"));
        }
        if need_ac && !cx.ac_tables[c.ac_tbl].present {
            return Err(corrupt("scan uses an undefined AC Huffman table"));
        }
    }

    let interleaved = ns > 1;
    let (cols, rows) = if interleaved {
        (fr.mcux, fr.mcuy)
    } else {
        let c = &fr.comps[order[0]];
        (c.real_w, c.real_h)
    };
    let mut rd = Reader::new(d, start);
    for &ci in &order {
        fr.comps[ci].pred = 0;
    }
    let mut eobrun: i32 = 0;
    let mut unit = 0usize;
    for ry in 0..rows {
        for rx in 0..cols {
            if cx.restart_interval > 0 && unit > 0 && unit.is_multiple_of(cx.restart_interval) {
                rd.restart();
                for &ci in &order {
                    fr.comps[ci].pred = 0;
                }
                eobrun = 0;
            }
            unit += 1;
            for &ci in &order {
                let (nh, nv) = if interleaved {
                    (fr.comps[ci].h, fr.comps[ci].v)
                } else {
                    (1, 1)
                };
                for by in 0..nv {
                    for bx in 0..nh {
                        let (blk_x, blk_y) = if interleaved {
                            (rx * fr.comps[ci].h + bx, ry * fr.comps[ci].v + by)
                        } else {
                            (rx, ry)
                        };
                        let c = &mut fr.comps[ci];
                        let o = (blk_y * c.bw + blk_x) * 64;
                        let block: &mut [i16] = &mut c.coef[o..o + 64];
                        let dct = &cx.dc_tables[c.dc_tbl];
                        let act = &cx.ac_tables[c.ac_tbl];
                        if !progressive {
                            baseline_block(&mut rd, block, &mut c.pred, dct, act)?;
                        } else if dc_scan {
                            if ah == 0 {
                                let s = u32::from(rd.huff(dct)? & 15);
                                c.pred = c.pred.wrapping_add(extend(rd.get(s), s));
                                block[0] = c.pred.wrapping_shl(al) as i16;
                            } else if rd.bit() {
                                block[0] |= (1 << al) as i16;
                            }
                        } else if ah == 0 {
                            ac_first(&mut rd, block, (ss, se, al), act, &mut eobrun)?;
                        } else {
                            ac_refine(&mut rd, block, (ss, se, al), act, &mut eobrun)?;
                        }
                    }
                }
            }
        }
    }
    // The scan ends at the next real marker.
    let mut end = rd.pos.min(d.len());
    while end + 1 < d.len()
        && !(d[end] == 0xFF
            && d[end + 1] != 0
            && d[end + 1] != 0xFF
            && !(0xD0..=0xD7).contains(&d[end + 1]))
    {
        end += 1;
    }
    Ok(end)
}

fn baseline_block(
    rd: &mut Reader,
    block: &mut [i16],
    pred: &mut i32,
    dct: &Huff,
    act: &Huff,
) -> Result<(), Error> {
    let s = u32::from(rd.huff(dct)? & 15);
    *pred = pred.wrapping_add(extend(rd.get(s), s));
    block[0] = *pred as i16;
    let mut k = 1;
    while k < 64 {
        let rs = rd.huff(act)?;
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
        block[ZIGZAG[k]] = extend(rd.get(s), s) as i16;
        k += 1;
    }
    Ok(())
}

fn ac_first(
    rd: &mut Reader,
    block: &mut [i16],
    (ss, se, al): (usize, usize, u32),
    act: &Huff,
    eobrun: &mut i32,
) -> Result<(), Error> {
    if *eobrun > 0 {
        *eobrun -= 1;
        return Ok(());
    }
    let mut k = ss;
    while k <= se {
        let rs = rd.huff(act)?;
        let (r, s) = (u32::from(rs >> 4), u32::from(rs & 15));
        if s == 0 {
            if r < 15 {
                *eobrun = (1 << r) - 1;
                if r > 0 {
                    *eobrun += rd.get(r) as i32;
                }
                break;
            }
            k += 16;
            continue;
        }
        k += r as usize;
        if k > 63 {
            break;
        }
        block[ZIGZAG[k]] = extend(rd.get(s), s).wrapping_shl(al) as i16;
        k += 1;
    }
    Ok(())
}

fn ac_refine(
    rd: &mut Reader,
    block: &mut [i16],
    (ss, se, al): (usize, usize, u32),
    act: &Huff,
    eobrun: &mut i32,
) -> Result<(), Error> {
    let p1: i16 = 1 << al;
    let m1: i16 = -(1 << al);
    let mut k = ss;
    if *eobrun <= 0 {
        while k <= se {
            let rs = rd.huff(act)?;
            let (mut r, s) = (i32::from(rs >> 4), u32::from(rs & 15));
            let mut val: i16 = 0;
            if s == 0 {
                if r < 15 {
                    *eobrun = 1 << r;
                    if r > 0 {
                        *eobrun += rd.get(r as u32) as i32;
                    }
                    break;
                }
                // r == 15: skip 16 zero coefficients.
            } else {
                val = if rd.bit() { p1 } else { m1 };
            }
            while k <= se {
                let c = &mut block[ZIGZAG[k]];
                if *c != 0 {
                    if rd.bit() && (*c & p1) == 0 {
                        *c = c.wrapping_add(if *c >= 0 { p1 } else { m1 });
                    }
                } else {
                    r -= 1;
                    if r < 0 {
                        break;
                    }
                }
                k += 1;
            }
            if val != 0 && k <= se {
                block[ZIGZAG[k]] = val;
            }
            k += 1;
        }
    }
    if *eobrun > 0 {
        while k <= se {
            let c = &mut block[ZIGZAG[k]];
            if *c != 0 && rd.bit() && (*c & p1) == 0 {
                *c = c.wrapping_add(if *c >= 0 { p1 } else { m1 });
            }
            k += 1;
        }
        *eobrun -= 1;
    }
    Ok(())
}

// ---- IDCT -----------------------------------------------------------------

const fn f2f(x: f32) -> i64 {
    (x * 4096.0 + 0.5) as i64
}

/// One 8-point inverse DCT pass (the classic `jidctint` butterfly with 12
/// fractional bits, in 64-bit so corrupt coefficients cannot overflow).
/// Returns outputs 0..4 and 4..8.
#[inline(always)]
fn idct_1d(s: [i64; 8]) -> ([i64; 4], [i64; 4]) {
    let p1 = (s[2] + s[6]) * f2f(0.541_196_1);
    let t2 = p1 + s[6] * f2f(-1.847_759_1);
    let t3 = p1 + s[2] * f2f(0.765_366_86);
    let t0 = (s[0] + s[4]) << 12;
    let t1 = (s[0] - s[4]) << 12;
    let x0 = t0 + t3;
    let x3 = t0 - t3;
    let x1 = t1 + t2;
    let x2 = t1 - t2;
    let (mut t0, mut t1, mut t2, mut t3) = (s[7], s[5], s[3], s[1]);
    let mut p3 = t0 + t2;
    let mut p4 = t1 + t3;
    let mut p1 = t0 + t3;
    let mut p2 = t1 + t2;
    let p5 = (p3 + p4) * f2f(1.175_875_6);
    t0 *= f2f(0.298_631_34);
    t1 *= f2f(2.053_12);
    t2 *= f2f(3.072_711);
    t3 *= f2f(1.501_321_1);
    p1 = p5 + p1 * f2f(-0.899_976_2);
    p2 = p5 + p2 * f2f(-2.562_915_5);
    p3 *= f2f(-1.961_570_6);
    p4 *= f2f(-0.390_180_64);
    t3 += p1 + p4;
    t2 += p2 + p3;
    t1 += p2 + p4;
    t0 += p1 + p3;
    (
        [x0 + t3, x1 + t2, x2 + t1, x3 + t0],
        [x3 - t0, x2 - t1, x1 - t2, x0 - t3],
    )
}

/// Dequantizes and inverse-transforms one block into 8 bits per sample.
fn idct_block(coef: &[i16], q: &[u16; 64], out: &mut [u8], stride: usize) {
    if coef[1..].iter().all(|&c| c == 0) {
        // DC only: a flat block.
        let v = ((i32::from(coef[0]) * i32::from(q[0]) + 4) >> 3) + 128;
        let v = v.clamp(0, 255) as u8;
        for y in 0..8 {
            out[y * stride..y * stride + 8].fill(v);
        }
        return;
    }
    let mut tmp = [0i64; 64];
    for x in 0..8 {
        let s = |r: usize| i64::from(coef[r * 8 + x]) * i64::from(q[r * 8 + x]);
        let (lo, hi) = idct_1d([s(0), s(1), s(2), s(3), s(4), s(5), s(6), s(7)]);
        for r in 0..4 {
            tmp[r * 8 + x] = (lo[r] + 512) >> 10;
            tmp[(7 - r) * 8 + x] = (hi[3 - r] + 512) >> 10;
        }
    }
    let bias = 65536 + (128 << 17);
    for y in 0..8 {
        let r = &tmp[y * 8..y * 8 + 8];
        let (lo, hi) = idct_1d([r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7]]);
        let row = &mut out[y * stride..y * stride + 8];
        for k in 0..4 {
            row[k] = ((lo[k] + bias) >> 17).clamp(0, 255) as u8;
            row[7 - k] = ((hi[3 - k] + bias) >> 17).clamp(0, 255) as u8;
        }
    }
}

// ---- assembly ----------------------------------------------------------------

struct Plane {
    data: Vec<u8>,
    stride: usize,
    /// Samples that carry image data (before MCU padding).
    w: usize,
    h: usize,
}

fn component_plane(c: &Component, quant: &[u16; 64], w: usize, h: usize) -> Plane {
    let stride = c.bw * 8;
    let mut data = vec![0u8; stride * c.bh * 8];
    for by in 0..c.bh {
        for bx in 0..c.bw {
            let o = (by * c.bw + bx) * 64;
            idct_block(
                &c.coef[o..o + 64],
                quant,
                &mut data[by * 8 * stride + bx * 8..],
                stride,
            );
        }
    }
    Plane { data, stride, w, h }
}

/// Per-output-sample interpolation taps `(i0, i1, weight of i1 in 0..=256)`.
fn axis_taps(out_len: usize, src_len: usize, num: usize, den: usize) -> Vec<(u32, u32, u32)> {
    // Output sample x sits at source coordinate (x + 0.5) * num / den - 0.5.
    (0..out_len)
        .map(|x| {
            let pos = ((2 * x + 1) * num) as f32 / (2 * den) as f32 - 0.5;
            let p = pos.max(0.0);
            let i0 = (p.floor() as usize).min(src_len - 1);
            let i1 = (i0 + 1).min(src_len - 1);
            let w = ((p - i0 as f32) * 256.0).round().clamp(0.0, 256.0) as u32;
            (i0 as u32, i1 as u32, w)
        })
        .collect()
}

/// The component upsampled to `width x height`, one byte per pixel.
fn upsample(p: &Plane, c: &Component, fr: &Frame) -> Vec<u8> {
    let (width, height) = (fr.width, fr.height);
    let mut out = vec![0u8; width * height];
    if c.h == fr.hmax && c.v == fr.vmax {
        for y in 0..height {
            out[y * width..(y + 1) * width]
                .copy_from_slice(&p.data[y * p.stride..y * p.stride + width]);
        }
        return out;
    }
    let xt = axis_taps(width, p.w.max(1), c.h, fr.hmax);
    let yt = axis_taps(height, p.h.max(1), c.v, fr.vmax);
    for y in 0..height {
        let (y0, y1, wy) = yt[y];
        let r0 = &p.data[y0 as usize * p.stride..];
        let r1 = &p.data[y1 as usize * p.stride..];
        let dst = &mut out[y * width..(y + 1) * width];
        for (d, &(x0, x1, wx)) in dst.iter_mut().zip(&xt) {
            let (x0, x1) = (x0 as usize, x1 as usize);
            let a = u32::from(r0[x0]) * (256 - wy) + u32::from(r1[x0]) * wy;
            let b = u32::from(r0[x1]) * (256 - wy) + u32::from(r1[x1]) * wy;
            *d = ((a * (256 - wx) + b * wx + (1 << 15)) >> 16) as u8;
        }
    }
    out
}

fn assemble(fr: &Frame, adobe: Option<u8>) -> Rgba8Image {
    let (width, height) = (fr.width, fr.height);
    let planes: Vec<Vec<u8>> = fr
        .comps
        .iter()
        .map(|c| {
            let quant = c.quant.unwrap_or([1; 64]);
            let pw = (width * c.h).div_ceil(fr.hmax);
            let ph = (height * c.v).div_ceil(fr.vmax);
            upsample(&component_plane(c, &quant, pw, ph), c, fr)
        })
        .collect();
    let mut rgba = vec![255u8; width * height * 4];
    let n = planes.len();
    let ids_rgb =
        n == 3 && fr.comps[0].id == b'R' && fr.comps[1].id == b'G' && fr.comps[2].id == b'B';
    let rgb_direct = n == 3 && (adobe == Some(0) || ids_rgb);
    let ycc = |y: i32, cb: i32, cr: i32| -> [i32; 3] {
        let (cb, cr) = (cb - 128, cr - 128);
        [
            y + ((91_881 * cr + 32_768) >> 16),
            y + ((-22_554 * cb - 46_802 * cr + 32_768) >> 16),
            y + ((116_130 * cb + 32_768) >> 16),
        ]
    };
    for (i, px) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let v = |k: usize| i32::from(planes[k][i]);
        let rgb: [i32; 3] = match n {
            1 => [v(0); 3],
            3 if rgb_direct => [v(0), v(1), v(2)],
            3 => ycc(v(0), v(1), v(2)),
            _ => {
                // Adobe CMYK / YCCK stores inverted samples.
                let cmy = if adobe == Some(2) {
                    ycc(v(0), v(1), v(2))
                } else {
                    [v(0), v(1), v(2)]
                };
                let k = v(3);
                [cmy[0] * k / 255, cmy[1] * k / 255, cmy[2] * k / 255]
            }
        };
        for (dst, c) in px.iter_mut().zip(rgb) {
            *dst = c.clamp(0, 255) as u8;
        }
    }
    Rgba8Image {
        width: width as u32,
        height: height as u32,
        rgba,
    }
}

#[cfg(test)]
pub(super) fn idct_block_for_test(coef: &[i16; 64], q: &[u16; 64], out: &mut [u8; 64]) {
    idct_block(coef, q, out, 8);
}
