//! A small zlib/DEFLATE decoder (RFC 1950 and 1951) for the PDF reader: the
//! object streams, cross-reference streams and `FlateDecode` pictures of a
//! PDF are compressed with it. Port of the structure of zlib's `puff.c`:
//! canonical Huffman codes decoded bit by bit, fixed and dynamic blocks,
//! stored blocks. Output is capped so a damaged or hostile stream cannot use
//! all the memory.

/// Why a stream could not be inflated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InflateError {
    /// The stream ended before the last block did.
    Truncated,
    /// The bits do not describe a valid DEFLATE stream.
    Corrupt(&'static str),
    /// The output would pass the limit given by the caller.
    TooLarge,
}

impl std::fmt::Display for InflateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InflateError::Truncated => write!(f, "the compressed data ends early"),
            InflateError::Corrupt(why) => write!(f, "damaged compressed data ({why})"),
            InflateError::TooLarge => write!(f, "the picture is too large"),
        }
    }
}

const MAX_BITS: usize = 15;
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
const CLEN_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// A canonical Huffman code: how many codes of each length, and the symbols
/// in code order.
struct Huffman {
    count: [u16; MAX_BITS + 1],
    symbol: Vec<u16>,
}

impl Huffman {
    /// Builds the code from the code length of every symbol. An incomplete
    /// code is accepted (a block of one distance code is legal), an
    /// over-subscribed one is not.
    fn new(lengths: &[u8]) -> Result<Huffman, InflateError> {
        let mut count = [0u16; MAX_BITS + 1];
        for &l in lengths {
            count[usize::from(l)] += 1;
        }
        let mut left: i32 = 1;
        for &n in count.iter().skip(1) {
            left <<= 1;
            left -= i32::from(n);
            if left < 0 {
                return Err(InflateError::Corrupt("over-subscribed code"));
            }
        }
        let mut offs = [0u16; MAX_BITS + 2];
        for len in 1..=MAX_BITS {
            offs[len + 1] = offs[len] + count[len];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[usize::from(offs[usize::from(l)])] = sym as u16;
                offs[usize::from(l)] += 1;
            }
        }
        Ok(Huffman { count, symbol })
    }
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
    nbits: u32,
}

impl Bits<'_> {
    fn need(&mut self, n: u32) -> Result<u32, InflateError> {
        while self.nbits < n {
            let b = *self.data.get(self.pos).ok_or(InflateError::Truncated)?;
            self.pos += 1;
            self.bit |= u32::from(b) << self.nbits;
            self.nbits += 8;
        }
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.nbits -= n;
        Ok(v)
    }

    fn decode(&mut self, h: &Huffman) -> Result<usize, InflateError> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..=MAX_BITS {
            code |= self.need(1)? as i32;
            let count = i32::from(h.count[len]);
            if code - count < first {
                return Ok(usize::from(h.symbol[(index + (code - first)) as usize]));
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(InflateError::Corrupt("bad code"))
    }
}

fn fixed_codes() -> Result<(Huffman, Huffman), InflateError> {
    let mut l = [0u8; 288];
    l[..144].fill(8);
    l[144..256].fill(9);
    l[256..280].fill(7);
    l[280..].fill(8);
    Ok((Huffman::new(&l)?, Huffman::new(&[5u8; 30])?))
}

fn codes(
    bits: &mut Bits,
    out: &mut Vec<u8>,
    lit: &Huffman,
    dist: &Huffman,
    limit: usize,
) -> Result<(), InflateError> {
    loop {
        let sym = bits.decode(lit)?;
        match sym {
            0..=255 => {
                if out.len() >= limit {
                    return Err(InflateError::TooLarge);
                }
                out.push(sym as u8);
            }
            256 => return Ok(()),
            257..=285 => {
                let i = sym - 257;
                let len = usize::from(LEN_BASE[i]) + bits.need(u32::from(LEN_EXTRA[i]))? as usize;
                let ds = bits.decode(dist)?;
                if ds >= 30 {
                    return Err(InflateError::Corrupt("bad distance code"));
                }
                let d = usize::from(DIST_BASE[ds]) + bits.need(u32::from(DIST_EXTRA[ds]))? as usize;
                if d > out.len() {
                    return Err(InflateError::Corrupt("distance too far back"));
                }
                if out.len() + len > limit {
                    return Err(InflateError::TooLarge);
                }
                let start = out.len() - d;
                for k in 0..len {
                    let b = out[start + k];
                    out.push(b);
                }
            }
            _ => return Err(InflateError::Corrupt("bad length code")),
        }
    }
}

fn dynamic_codes(bits: &mut Bits) -> Result<(Huffman, Huffman), InflateError> {
    let nlen = bits.need(5)? as usize + 257;
    let ndist = bits.need(5)? as usize + 1;
    let ncode = bits.need(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err(InflateError::Corrupt("too many codes"));
    }
    let mut lengths = [0u8; 320];
    for &o in CLEN_ORDER.iter().take(ncode) {
        lengths[o] = bits.need(3)? as u8;
    }
    let clen = Huffman::new(&lengths[..19])?;
    let mut lengths = [0u8; 320];
    let mut i = 0;
    while i < nlen + ndist {
        let sym = bits.decode(&clen)?;
        if sym < 16 {
            lengths[i] = sym as u8;
            i += 1;
            continue;
        }
        let (prev, rep) = match sym {
            16 => {
                if i == 0 {
                    return Err(InflateError::Corrupt("repeat with nothing before"));
                }
                (lengths[i - 1], 3 + bits.need(2)? as usize)
            }
            17 => (0, 3 + bits.need(3)? as usize),
            _ => (0, 11 + bits.need(7)? as usize),
        };
        if i + rep > nlen + ndist {
            return Err(InflateError::Corrupt("too many lengths"));
        }
        lengths[i..i + rep].fill(prev);
        i += rep;
    }
    if lengths[256] == 0 {
        return Err(InflateError::Corrupt("no end-of-block code"));
    }
    Ok((
        Huffman::new(&lengths[..nlen])?,
        Huffman::new(&lengths[nlen..nlen + ndist])?,
    ))
}

/// Inflates raw DEFLATE data. `limit` caps the output.
pub fn inflate_raw(data: &[u8], limit: usize) -> Result<Vec<u8>, InflateError> {
    let mut bits = Bits {
        data,
        pos: 0,
        bit: 0,
        nbits: 0,
    };
    let mut out = Vec::with_capacity(data.len().saturating_mul(3).min(limit));
    loop {
        let last = bits.need(1)?;
        match bits.need(2)? {
            0 => {
                bits.bit = 0;
                bits.nbits = 0;
                let head = data
                    .get(bits.pos..bits.pos + 4)
                    .ok_or(InflateError::Truncated)?;
                let len = usize::from(u16::from_le_bytes([head[0], head[1]]));
                let nlen = u16::from_le_bytes([head[2], head[3]]);
                if nlen != !(len as u16) {
                    return Err(InflateError::Corrupt("stored block length"));
                }
                bits.pos += 4;
                let block = data
                    .get(bits.pos..bits.pos + len)
                    .ok_or(InflateError::Truncated)?;
                if out.len() + len > limit {
                    return Err(InflateError::TooLarge);
                }
                out.extend_from_slice(block);
                bits.pos += len;
            }
            1 => {
                let (l, d) = fixed_codes()?;
                codes(&mut bits, &mut out, &l, &d, limit)?;
            }
            2 => {
                let (l, d) = dynamic_codes(&mut bits)?;
                codes(&mut bits, &mut out, &l, &d, limit)?;
            }
            _ => return Err(InflateError::Corrupt("reserved block type")),
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

/// Inflates a zlib stream (the two header bytes, the DEFLATE data; the
/// Adler-32 check at the end is not verified because PDF writers sometimes
/// leave it off or damage it and viewers read the data anyway).
pub fn inflate_zlib(data: &[u8], limit: usize) -> Result<Vec<u8>, InflateError> {
    if data.len() < 2 {
        return Err(InflateError::Truncated);
    }
    let (cmf, flg) = (data[0], data[1]);
    let header = cmf & 0x0F == 8 && (u16::from(cmf) << 8 | u16::from(flg)) % 31 == 0;
    let skip = if header {
        if flg & 0x20 != 0 {
            6
        } else {
            2
        }
    } else {
        0
    };
    inflate_raw(&data[skip.min(data.len())..], limit)
}

/// Adler-32 checksum.
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    (b << 16) | a
}

/// `raw` as a zlib stream of stored (uncompressed) blocks.
pub fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    if raw.is_empty() {
        out.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    let mut chunks = raw.chunks(65_535).peekable();
    while let Some(c) = chunks.next() {
        out.push(u8::from(chunks.peek().is_none()));
        out.extend_from_slice(&(c.len() as u16).to_le_bytes());
        out.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        out.extend_from_slice(c);
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "hello hello hello hello\n" deflated with fixed Huffman codes and a
    /// back reference (made with zlib, level 9).
    const FIXED: &[u8] = &[
        0x78, 0xDA, 0xCB, 0x48, 0xCD, 0xC9, 0xC9, 0x57, 0xC8, 0x40, 0x27, 0xB9, 0x00, 0x70, 0xBE,
        0x08, 0xBB,
    ];

    #[test]
    fn a_fixed_huffman_stream_with_a_back_reference_inflates() {
        let out = inflate_zlib(FIXED, 1000).unwrap();
        assert_eq!(out, b"hello hello hello hello\n");
    }

    #[test]
    fn stored_blocks_round_trip_through_the_writer() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let z = zlib_stored(&data);
        assert_eq!(inflate_zlib(&z, usize::MAX).unwrap(), data);
        assert_eq!(
            inflate_zlib(&zlib_stored(&[]), 10).unwrap(),
            Vec::<u8>::new()
        );
    }

    #[test]
    fn dynamic_blocks_inflate() {
        // A dynamic-Huffman stream of words and noise (made with zlib).
        let z = include_bytes!("testdata/dynamic.zlib");
        let want = include_bytes!("testdata/dynamic.raw");
        assert_eq!(inflate_zlib(z, 1 << 20).unwrap(), want);
    }

    #[test]
    fn bad_streams_are_refused_without_panicking() {
        assert_eq!(inflate_zlib(&[], 10), Err(InflateError::Truncated));
        assert!(inflate_zlib(&FIXED[..8], 100).is_err());
        assert!(inflate_zlib(&[0x78, 0x9C, 0xFF, 0xFF, 0xFF], 100).is_err());
        let z = zlib_stored(&[1u8; 100]);
        assert_eq!(inflate_zlib(&z, 50), Err(InflateError::TooLarge));
    }
}
