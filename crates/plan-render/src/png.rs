//! Dependency-free PNG writer (RGBA8, zlib "stored" blocks, no compression).

use crate::image::Image;
use std::io;
use std::path::Path;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
/// Largest payload of one stored DEFLATE block.
const STORED_MAX: usize = 65_535;

const fn crc_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = crc_table();

/// CRC-32 (IEEE) as used by PNG chunks.
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFF_u32;
    for &b in data {
        c = CRC_TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Adler-32 checksum as used by zlib.
pub fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let (mut a, mut b) = (1_u32, 0_u32);
    // 5552 bytes is the longest run before `b` can overflow a u32.
    for block in data.chunks(5552) {
        for &byte in block {
            a += u32::from(byte);
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

/// Wrap `raw` in a zlib stream made of uncompressed DEFLATE blocks.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + raw.len() / STORED_MAX * 5 + 16);
    out.extend_from_slice(&[0x78, 0x01]);
    if raw.is_empty() {
        out.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    let blocks = raw.chunks(STORED_MAX).count();
    for (i, block) in raw.chunks(STORED_MAX).enumerate() {
        let len = block.len() as u16;
        out.push(u8::from(i + 1 == blocks)); // BFINAL, BTYPE = 00
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Encode an image as a PNG file (RGBA, 8 bits per channel).
pub fn encode_png(image: &Image) -> Vec<u8> {
    let (w, h) = (image.width as usize, image.height as usize);
    let mut raw = Vec::with_capacity(h * (w * 4 + 1));
    for row in image.rgba.chunks(w.max(1) * 4).take(h) {
        raw.push(0); // filter type: None
        raw.extend_from_slice(row);
    }
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&image.width.to_be_bytes());
    ihdr.extend_from_slice(&image.height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA, deflate, no filter, no interlace

    let mut out = Vec::with_capacity(raw.len() + 64);
    out.extend_from_slice(&SIGNATURE);
    push_chunk(&mut out, b"IHDR", &ihdr);
    push_chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    push_chunk(&mut out, b"IEND", &[]);
    out
}

/// Write an image to `path` as a PNG.
pub fn write_png(path: impl AsRef<Path>, image: &Image) -> io::Result<()> {
    std::fs::write(path, encode_png(image))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_checksums() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        let big = vec![0xFF_u8; 100_000];
        assert_eq!(adler32(&big), {
            let (mut a, mut b) = (1_u64, 0_u64);
            for &x in &big {
                a = (a + u64::from(x)) % 65_521;
                b = (b + a) % 65_521;
            }
            ((b << 16) | a) as u32
        });
    }
}
