//! Picture file writers for Export Picture: PNG (through `plan-render`),
//! baseline JPEG, 32-bit BMP and uncompressed TIFF. No new crates.
//!
//! Input is straight (non-premultiplied) RGBA8, rows top to bottom. JPEG and
//! BMP have no usable alpha and flatten onto white; PNG and TIFF keep it.

/// File format of an exported picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    Bmp,
    Tiff,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Png, Format::Jpeg, Format::Bmp, Format::Tiff];

    pub fn label(self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpeg => "JPEG",
            Format::Bmp => "BMP",
            Format::Tiff => "TIFF",
        }
    }

    /// Extension without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Bmp => "bmp",
            Format::Tiff => "tif",
        }
    }

    /// Whether the format can hold a transparent background.
    pub fn has_alpha(self) -> bool {
        matches!(self, Format::Png | Format::Tiff)
    }

    /// The format a file name's extension names.
    pub fn from_extension(ext: &str) -> Option<Format> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Format::Png),
            "jpg" | "jpeg" => Some(Format::Jpeg),
            "bmp" => Some(Format::Bmp),
            "tif" | "tiff" => Some(Format::Tiff),
            _ => None,
        }
    }
}

/// Encodes `rgba` (`width * height * 4` bytes) as `format`. `quality` is the
/// JPEG quality, 1..=100.
pub fn encode(format: Format, width: u32, height: u32, rgba: &[u8], quality: u8) -> Vec<u8> {
    match format {
        Format::Png => plan_render::encode_png(&plan_render::Image {
            width,
            height,
            rgba: rgba.to_vec(),
            hdr: Vec::new(),
        }),
        Format::Jpeg => encode_jpeg(width, height, rgba, quality),
        Format::Bmp => encode_bmp(width, height, rgba),
        Format::Tiff => encode_tiff(width, height, rgba),
    }
}

/// `c` over white by alpha `a`.
fn over_white(c: u8, a: u8) -> u8 {
    let (c, a) = (u32::from(c), u32::from(a));
    ((c * a + 255 * (255 - a) + 127) / 255) as u8
}

// ----- BMP -----

/// A 32-bit BMP (BITMAPINFOHEADER, bottom-up rows, BGRA), flattened on white.
pub fn encode_bmp(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let data = w * h * 4;
    let mut out = Vec::with_capacity(54 + data);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((54 + data) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(data as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes()); // 72 dpi
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&[0; 8]);
    for row in (0..h).rev() {
        for px in rgba[row * w * 4..(row + 1) * w * 4].chunks_exact(4) {
            let a = px[3];
            out.extend_from_slice(&[
                over_white(px[2], a),
                over_white(px[1], a),
                over_white(px[0], a),
                255,
            ]);
        }
    }
    out
}

// ----- TIFF -----

/// An uncompressed little-endian RGBA TIFF (one strip, unassociated alpha).
pub fn encode_tiff(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let data_len = width as usize * height as usize * 4;
    let mut out = Vec::with_capacity(data_len + 200);
    out.extend_from_slice(b"II");
    out.extend_from_slice(&42u16.to_le_bytes());
    out.extend_from_slice(&8u32.to_le_bytes());
    // 12 entries after the 2-byte count; values that do not fit go after.
    let entries: u16 = 12;
    let ifd_end = 8 + 2 + 12 * u32::from(entries) + 4;
    let bits_at = ifd_end; // 4 x u16
    let data_at = bits_at + 8;
    let mut e: Vec<(u16, u16, u32, u32)> = Vec::new();
    // (tag, type, count, value): type 3 = SHORT, 4 = LONG
    e.push((256, 4, 1, width));
    e.push((257, 4, 1, height));
    e.push((258, 3, 4, bits_at));
    e.push((259, 3, 1, 1)); // no compression
    e.push((262, 3, 1, 2)); // RGB
    e.push((273, 4, 1, data_at)); // strip offset
    e.push((277, 3, 1, 4)); // samples per pixel
    e.push((278, 4, 1, height)); // rows per strip
    e.push((279, 4, 1, data_len as u32)); // strip byte count
    e.push((284, 3, 1, 1)); // chunky
    e.push((296, 3, 1, 2)); // resolution unit: inch
    e.push((338, 3, 1, 2)); // extra samples: unassociated alpha
    debug_assert_eq!(e.len(), usize::from(entries));
    out.extend_from_slice(&entries.to_le_bytes());
    for (tag, ty, count, value) in e {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        if ty == 3 && count == 1 {
            out.extend_from_slice(&(value as u16).to_le_bytes());
            out.extend_from_slice(&[0, 0]);
        } else {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    for _ in 0..4 {
        out.extend_from_slice(&8u16.to_le_bytes());
    }
    out.extend_from_slice(&rgba[..data_len]);
    out
}

// ----- JPEG -----

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

const LUMA_Q: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

const CHROMA_Q: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_LUMA_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROMA_VALS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

/// Huffman codes by symbol: `(code, length)`.
struct Codes([(u16, u8); 256]);

impl Codes {
    fn new(bits: &[u8; 16], vals: &[u8]) -> Codes {
        let mut table = [(0u16, 0u8); 256];
        let (mut code, mut k) = (0u16, 0usize);
        for (len, &n) in bits.iter().enumerate() {
            for _ in 0..n {
                table[usize::from(vals[k])] = (code, len as u8 + 1);
                code += 1;
                k += 1;
            }
            code <<= 1;
        }
        Codes(table)
    }
}

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, code: u16, len: u8) {
        if len == 0 {
            return;
        }
        self.acc = (self.acc << len) | (u32::from(code) & ((1 << len) - 1));
        self.n += u32::from(len);
        while self.n >= 8 {
            let b = (self.acc >> (self.n - 8)) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
        }
    }

    fn finish(&mut self) {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put((1u16 << pad) - 1, pad as u8);
        }
    }
}

/// Number of bits of `v`'s magnitude and the bits to write for it.
fn magnitude(v: i32) -> (u8, u16) {
    if v == 0 {
        return (0, 0);
    }
    let a = v.unsigned_abs();
    let size = 32 - a.leading_zeros();
    let bits = if v < 0 { (v - 1) as u32 } else { v as u32 };
    (size as u8, (bits & ((1 << size) - 1)) as u16)
}

fn scaled_table(base: &[u8; 64], quality: u8) -> [u8; 64] {
    let q = i32::from(quality.clamp(1, 100));
    let scale = if q < 50 { 5000 / q } else { 200 - 2 * q };
    let mut out = [0u8; 64];
    for (o, &b) in out.iter_mut().zip(base) {
        *o = ((i32::from(b) * scale + 50) / 100).clamp(1, 255) as u8;
    }
    out
}

/// 8-point DCT-II basis, `cos((2x + 1) u pi / 16)` with the normalisation.
fn dct_basis() -> [[f32; 8]; 8] {
    let mut m = [[0.0f32; 8]; 8];
    for (u, row) in m.iter_mut().enumerate() {
        let cu = if u == 0 { 0.5 / 2f32.sqrt() } else { 0.5 };
        for (x, v) in row.iter_mut().enumerate() {
            *v = cu * (((2 * x + 1) as f32) * u as f32 * std::f32::consts::PI / 16.0).cos();
        }
    }
    m
}

fn forward_dct(block: &[f32; 64], basis: &[[f32; 8]; 8]) -> [f32; 64] {
    let mut tmp = [0.0f32; 64];
    for y in 0..8 {
        for u in 0..8 {
            tmp[y * 8 + u] = (0..8).map(|x| block[y * 8 + x] * basis[u][x]).sum();
        }
    }
    let mut out = [0.0f32; 64];
    for v in 0..8 {
        for u in 0..8 {
            out[v * 8 + u] = (0..8).map(|y| tmp[y * 8 + u] * basis[v][y]).sum();
        }
    }
    out
}

fn segment(out: &mut Vec<u8>, marker: u8, body: &[u8]) {
    out.extend_from_slice(&[0xFF, marker]);
    out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(body);
}

/// A baseline JPEG (YCbCr 4:4:4, standard Huffman tables) of `rgba` flattened
/// on white. `quality` is 1..=100 like libjpeg's.
pub fn encode_jpeg(width: u32, height: u32, rgba: &[u8], quality: u8) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let (ql, qc) = (scaled_table(&LUMA_Q, quality), scaled_table(&CHROMA_Q, quality));
    let mut out = vec![0xFF, 0xD8];
    segment(
        &mut out,
        0xE0,
        &[b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0],
    );
    for (id, table) in [(0u8, &ql), (1u8, &qc)] {
        let mut body = vec![id];
        body.extend(ZIGZAG.iter().map(|&z| table[z]));
        segment(&mut out, 0xDB, &body);
    }
    let mut sof = vec![8];
    sof.extend_from_slice(&(height as u16).to_be_bytes());
    sof.extend_from_slice(&(width as u16).to_be_bytes());
    sof.extend_from_slice(&[3, 1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1]);
    segment(&mut out, 0xC0, &sof);
    for (class_id, bits, vals) in [
        (0x00u8, &DC_LUMA_BITS, &DC_VALS[..]),
        (0x10, &AC_LUMA_BITS, &AC_LUMA_VALS[..]),
        (0x01, &DC_CHROMA_BITS, &DC_VALS[..]),
        (0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALS[..]),
    ] {
        let mut body = vec![class_id];
        body.extend_from_slice(bits);
        body.extend_from_slice(vals);
        segment(&mut out, 0xC4, &body);
    }
    segment(&mut out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);

    let dc = [
        Codes::new(&DC_LUMA_BITS, &DC_VALS),
        Codes::new(&DC_CHROMA_BITS, &DC_VALS),
    ];
    let ac = [
        Codes::new(&AC_LUMA_BITS, &AC_LUMA_VALS),
        Codes::new(&AC_CHROMA_BITS, &AC_CHROMA_VALS),
    ];
    let quant = [&ql, &qc, &qc];
    let basis = dct_basis();
    let mut bw = BitWriter {
        out,
        acc: 0,
        n: 0,
    };
    let mut last_dc = [0i32; 3];
    for by in (0..h).step_by(8) {
        for bx in (0..w).step_by(8) {
            // YCbCr planes of this block, edge pixels repeated past the image.
            let mut planes = [[0.0f32; 64]; 3];
            for y in 0..8 {
                for x in 0..8 {
                    let (sx, sy) = ((bx + x).min(w - 1), (by + y).min(h - 1));
                    let i = (sy * w + sx) * 4;
                    let a = rgba[i + 3];
                    let r = f32::from(over_white(rgba[i], a));
                    let g = f32::from(over_white(rgba[i + 1], a));
                    let b = f32::from(over_white(rgba[i + 2], a));
                    planes[0][y * 8 + x] = 0.299 * r + 0.587 * g + 0.114 * b - 128.0;
                    planes[1][y * 8 + x] = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
                    planes[2][y * 8 + x] = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
                }
            }
            for (c, plane) in planes.iter().enumerate() {
                let t = usize::from(c > 0);
                let coef = forward_dct(plane, &basis);
                let mut q = [0i32; 64];
                for (k, &z) in ZIGZAG.iter().enumerate() {
                    q[k] = (coef[z] / f32::from(quant[c][z])).round() as i32;
                }
                // DC difference.
                let diff = q[0] - last_dc[c];
                last_dc[c] = q[0];
                let (size, bits) = magnitude(diff);
                let (code, len) = dc[t].0[usize::from(size)];
                bw.put(code, len);
                bw.put(bits, size);
                // AC run lengths.
                let mut run = 0u8;
                for &v in &q[1..] {
                    if v == 0 {
                        run += 1;
                        continue;
                    }
                    while run > 15 {
                        let (code, len) = ac[t].0[0xF0];
                        bw.put(code, len);
                        run -= 16;
                    }
                    let (size, bits) = magnitude(v);
                    let (code, len) = ac[t].0[usize::from(run << 4 | size)];
                    bw.put(code, len);
                    bw.put(bits, size);
                    run = 0;
                }
                if run > 0 {
                    let (code, len) = ac[t].0[0x00];
                    bw.put(code, len);
                }
            }
        }
    }
    bw.finish();
    let mut out = bw.out;
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_library::image::{jpeg, png};

    /// A gradient with a hard edge and a translucent corner.
    fn sample(w: u32, h: u32) -> Vec<u8> {
        let mut v = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let edge = if x > w / 2 { 255 } else { 20 };
                let a = if x < 3 && y < 3 { 0 } else { 255 };
                v.extend_from_slice(&[edge, (y * 255 / h) as u8, (x * 255 / w) as u8, a]);
            }
        }
        v
    }

    #[test]
    fn huffman_tables_cover_every_symbol() {
        for (bits, vals) in [
            (&AC_LUMA_BITS, &AC_LUMA_VALS[..]),
            (&AC_CHROMA_BITS, &AC_CHROMA_VALS[..]),
        ] {
            assert_eq!(bits.iter().map(|&b| usize::from(b)).sum::<usize>(), vals.len());
            let mut want: Vec<u8> = (1..=10u8)
                .flat_map(|s| (0..16u8).map(move |r| r << 4 | s))
                .collect();
            want.extend([0x00, 0xF0]);
            want.sort_unstable();
            let mut got = vals.to_vec();
            got.sort_unstable();
            assert_eq!(got, want);
        }
        assert_eq!(DC_LUMA_BITS.iter().map(|&b| usize::from(b)).sum::<usize>(), 12);
        assert_eq!(DC_CHROMA_BITS.iter().map(|&b| usize::from(b)).sum::<usize>(), 12);
    }

    #[test]
    fn jpeg_round_trips_through_the_decoder() {
        let (w, h) = (37u32, 21u32); // not multiples of 8
        let src = sample(w, h);
        let bytes = encode_jpeg(w, h, &src, 92);
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
        assert_eq!(&bytes[bytes.len() - 2..], &[0xFF, 0xD9]);
        let back = jpeg::decode(&bytes).expect("our JPEG decodes");
        assert_eq!((back.width, back.height), (w, h));
        // Close to the source (lossy), away from the edge column.
        let mut worst = 0i32;
        for y in 4..h as usize {
            for x in 0..w as usize {
                if (x as i32 - (w / 2) as i32).abs() <= 2 {
                    continue;
                }
                for c in 0..3 {
                    let d = i32::from(src[(y * w as usize + x) * 4 + c])
                        - i32::from(back.rgba[(y * w as usize + x) * 4 + c]);
                    worst = worst.max(d.abs());
                }
            }
        }
        assert!(worst <= 24, "worst channel error {worst}");
        // The transparent corner became white.
        assert!(back.rgba[..3].iter().all(|&c| c > 200), "{:?}", &back.rgba[..4]);
    }

    #[test]
    fn lower_quality_is_smaller() {
        let src = sample(64, 64);
        let hi = encode_jpeg(64, 64, &src, 95).len();
        let lo = encode_jpeg(64, 64, &src, 20).len();
        assert!(lo < hi, "{lo} !< {hi}");
    }

    #[test]
    fn bmp_layout_and_bottom_up_rows() {
        let src = sample(5, 3);
        let b = encode_bmp(5, 3, &src);
        assert_eq!(&b[..2], b"BM");
        assert_eq!(u32::from_le_bytes([b[2], b[3], b[4], b[5]]) as usize, b.len());
        assert_eq!(b.len(), 54 + 5 * 3 * 4);
        assert_eq!(i32::from_le_bytes([b[18], b[19], b[20], b[21]]), 5);
        assert_eq!(i32::from_le_bytes([b[22], b[23], b[24], b[25]]), 3);
        // The first stored row is the image's last row: BGR of (0, last row).
        let last = &src[2 * 5 * 4..2 * 5 * 4 + 4];
        assert_eq!(&b[54..57], &[last[2], last[1], last[0]]);
    }

    #[test]
    fn tiff_header_and_pixels() {
        let src = sample(4, 2);
        let t = encode_tiff(4, 2, &src);
        assert_eq!(&t[..4], &[b'I', b'I', 42, 0]);
        let ifd = u32::from_le_bytes([t[4], t[5], t[6], t[7]]) as usize;
        let n = u16::from_le_bytes([t[ifd], t[ifd + 1]]) as usize;
        assert_eq!(n, 12);
        // Find the strip offset (tag 273) and read the pixels back.
        let mut offset = 0usize;
        for k in 0..n {
            let e = ifd + 2 + k * 12;
            if u16::from_le_bytes([t[e], t[e + 1]]) == 273 {
                offset = u32::from_le_bytes([t[e + 8], t[e + 9], t[e + 10], t[e + 11]]) as usize;
            }
        }
        assert_eq!(&t[offset..], &src[..]);
    }

    #[test]
    fn format_helpers_and_png_dispatch() {
        assert_eq!(Format::from_extension("JPEG"), Some(Format::Jpeg));
        assert_eq!(Format::from_extension("tiff"), Some(Format::Tiff));
        assert_eq!(Format::from_extension("gif"), None);
        assert!(Format::Png.has_alpha() && !Format::Jpeg.has_alpha() && !Format::Bmp.has_alpha());
        let src = sample(8, 8);
        let p = encode(Format::Png, 8, 8, &src, 90);
        let back = png::decode(&p).unwrap();
        assert_eq!((back.width, back.height, back.rgba), (8, 8, src));
    }
}
