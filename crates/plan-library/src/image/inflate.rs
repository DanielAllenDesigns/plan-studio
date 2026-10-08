//! A small zlib/deflate decoder (RFC 1950/1951) for PNG.
//!
//! `plan-calib` has one too, but it depends on this crate, so PNG decoding
//! carries its own: table-driven Huffman decoding, no checksum verification.

use super::{corrupt, Error};

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    acc: u64,
    n: u32,
    /// Zero bytes fed after the end of the input.
    over: usize,
}

impl<'a> Bits<'a> {
    fn new(d: &'a [u8]) -> Self {
        Bits {
            d,
            pos: 0,
            acc: 0,
            n: 0,
            over: 0,
        }
    }

    fn fill(&mut self) {
        while self.n <= 56 {
            let b = match self.d.get(self.pos) {
                Some(&b) => {
                    self.pos += 1;
                    b
                }
                None => {
                    self.over += 1;
                    0
                }
            };
            self.acc |= u64::from(b) << self.n;
            self.n += 8;
        }
    }

    fn bits(&mut self, k: u32) -> u32 {
        if self.n < k {
            self.fill();
        }
        let v = (self.acc & ((1u64 << k) - 1)) as u32;
        self.acc >>= k;
        self.n -= k;
        v
    }

    fn overrun(&self) -> bool {
        // The accumulator may legitimately hold up to 8 bytes of lookahead.
        self.over > 8
    }

    fn align_byte(&mut self) {
        let drop = self.n % 8;
        self.acc >>= drop;
        self.n -= drop;
    }
}

struct Huff {
    /// Indexed by the next `bits` input bits: `(len << 9) | symbol`, 0 = invalid.
    table: Vec<u16>,
    bits: u32,
}

impl Huff {
    fn new(lengths: &[u8]) -> Result<Huff, Error> {
        let maxlen = u32::from(lengths.iter().copied().max().unwrap_or(0));
        if maxlen == 0 {
            return Ok(Huff {
                table: vec![0; 2],
                bits: 1,
            });
        }
        let mut count = [0u32; 16];
        for &l in lengths {
            count[usize::from(l)] += 1;
        }
        count[0] = 0;
        let mut next = [0u32; 17];
        let mut code = 0u32;
        for len in 1..=15 {
            code = (code + count[len - 1]) << 1;
            next[len] = code;
        }
        let size = 1usize << maxlen;
        let mut table = vec![0u16; size];
        for (sym, &l) in lengths.iter().enumerate() {
            if l == 0 {
                continue;
            }
            let l = u32::from(l);
            let c = next[l as usize];
            next[l as usize] += 1;
            if c >= (1 << l) {
                return Err(corrupt("over-subscribed Huffman code"));
            }
            let rev = (c.reverse_bits() >> (32 - l)) as usize;
            let entry = ((l as u16) << 9) | sym as u16;
            let mut i = rev;
            while i < size {
                table[i] = entry;
                i += 1 << l;
            }
        }
        Ok(Huff {
            table,
            bits: maxlen,
        })
    }

    fn decode(&self, b: &mut Bits) -> Result<usize, Error> {
        if b.n < self.bits {
            b.fill();
        }
        let e = self.table[(b.acc & ((1u64 << self.bits) - 1)) as usize];
        if e == 0 {
            return Err(corrupt("bad Huffman code"));
        }
        let len = u32::from(e >> 9);
        b.acc >>= len;
        b.n -= len;
        Ok(usize::from(e & 0x1FF))
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CL_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Inflates a zlib stream (2-byte header, deflate data, ignored Adler-32).
/// `limit` caps the output size.
pub(crate) fn zlib_decompress(
    data: &[u8],
    size_hint: usize,
    limit: usize,
) -> Result<Vec<u8>, Error> {
    if data.len() < 2
        || data[0] & 0x0F != 8
        || (u32::from(data[0]) << 8 | u32::from(data[1])) % 31 != 0
    {
        return Err(corrupt("bad zlib header"));
    }
    if data[1] & 0x20 != 0 {
        return Err(corrupt("zlib preset dictionary"));
    }
    inflate(&data[2..], size_hint, limit)
}

pub(crate) fn inflate(data: &[u8], size_hint: usize, limit: usize) -> Result<Vec<u8>, Error> {
    let mut out: Vec<u8> = Vec::with_capacity(size_hint.min(limit));
    let mut b = Bits::new(data);
    let (mut fixed_l, mut fixed_d): (Option<Huff>, Option<Huff>) = (None, None);
    loop {
        let last = b.bits(1) == 1;
        match b.bits(2) {
            0 => {
                b.align_byte();
                let len = b.bits(16) as usize;
                let nlen = b.bits(16) as usize;
                if len != (!nlen & 0xFFFF) {
                    return Err(corrupt("bad stored block length"));
                }
                if out.len() + len > limit {
                    return Err(corrupt("inflated data too large"));
                }
                for _ in 0..len {
                    out.push(b.bits(8) as u8);
                }
            }
            t @ (1 | 2) => {
                let (lit, dist);
                let (lit_ref, dist_ref): (&Huff, &Huff);
                if t == 1 {
                    if fixed_l.is_none() {
                        let mut l = [0u8; 288];
                        l[..144].fill(8);
                        l[144..256].fill(9);
                        l[256..280].fill(7);
                        l[280..].fill(8);
                        fixed_l = Some(Huff::new(&l)?);
                        fixed_d = Some(Huff::new(&[5u8; 30])?);
                    }
                    lit_ref = fixed_l.as_ref().expect("just built");
                    dist_ref = fixed_d.as_ref().expect("just built");
                } else {
                    let hlit = b.bits(5) as usize + 257;
                    let hdist = b.bits(5) as usize + 1;
                    let hclen = b.bits(4) as usize + 4;
                    if hlit > 286 || hdist > 30 {
                        return Err(corrupt("bad dynamic block header"));
                    }
                    let mut cl = [0u8; 19];
                    for &o in CL_ORDER.iter().take(hclen) {
                        cl[o] = b.bits(3) as u8;
                    }
                    let clh = Huff::new(&cl)?;
                    let mut lens = vec![0u8; hlit + hdist];
                    let mut i = 0;
                    while i < lens.len() {
                        let sym = clh.decode(&mut b)?;
                        if b.overrun() {
                            return Err(corrupt("truncated deflate data"));
                        }
                        match sym {
                            0..=15 => {
                                lens[i] = sym as u8;
                                i += 1;
                            }
                            16 => {
                                if i == 0 {
                                    return Err(corrupt("repeat with no previous length"));
                                }
                                let prev = lens[i - 1];
                                let n = 3 + b.bits(2) as usize;
                                if i + n > lens.len() {
                                    return Err(corrupt("code lengths overflow"));
                                }
                                lens[i..i + n].fill(prev);
                                i += n;
                            }
                            17 | 18 => {
                                let n = if sym == 17 {
                                    3 + b.bits(3) as usize
                                } else {
                                    11 + b.bits(7) as usize
                                };
                                if i + n > lens.len() {
                                    return Err(corrupt("code lengths overflow"));
                                }
                                i += n; // already zero
                            }
                            _ => return Err(corrupt("bad code length symbol")),
                        }
                    }
                    lit = Huff::new(&lens[..hlit])?;
                    dist = Huff::new(&lens[hlit..])?;
                    lit_ref = &lit;
                    dist_ref = &dist;
                }
                loop {
                    let sym = lit_ref.decode(&mut b)?;
                    if b.overrun() {
                        return Err(corrupt("truncated deflate data"));
                    }
                    match sym {
                        0..=255 => {
                            if out.len() >= limit {
                                return Err(corrupt("inflated data too large"));
                            }
                            out.push(sym as u8);
                        }
                        256 => break,
                        257..=285 => {
                            let li = sym - 257;
                            let len = usize::from(LEN_BASE[li])
                                + b.bits(u32::from(LEN_EXTRA[li])) as usize;
                            let ds = dist_ref.decode(&mut b)?;
                            if ds >= 30 {
                                return Err(corrupt("bad distance symbol"));
                            }
                            let d = usize::from(DIST_BASE[ds])
                                + b.bits(u32::from(DIST_EXTRA[ds])) as usize;
                            if d > out.len() {
                                return Err(corrupt("distance too far back"));
                            }
                            if out.len() + len > limit {
                                return Err(corrupt("inflated data too large"));
                            }
                            let start = out.len() - d;
                            if d >= len {
                                out.extend_from_within(start..start + len);
                            } else {
                                for k in 0..len {
                                    let v = out[start + k];
                                    out.push(v);
                                }
                            }
                        }
                        _ => return Err(corrupt("bad literal/length symbol")),
                    }
                }
            }
            _ => return Err(corrupt("bad deflate block type")),
        }
        if b.overrun() {
            return Err(corrupt("truncated deflate data"));
        }
        if last {
            break;
        }
    }
    Ok(out)
}
