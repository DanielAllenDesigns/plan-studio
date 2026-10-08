//! Draws the Plan Studio app icon and writes it in every format the app needs.
//!
//! ```text
//! cargo run -p plan-app --example gen_app_icon            # from the repository root
//! ```
//!
//! The icon is original artwork in the visual language of the toolbars (see
//! `docs/chief-x18-toolbars.md`): a dark rounded tile, a house plan with red
//! walls, a blue door with its swing, a blue window and a yellow dimension
//! string. One list of shapes ([`shapes`]) is the single source; this program
//! writes it as
//!
//! * `assets/icons/app/plan-studio.svg`,
//! * `assets/icons/app/icon-<size>.png` for 16, 32, 64, 128, 256, 512 and 1024
//!   (rasterized here with 4 x 4 supersampling, deflate-compressed by the small
//!   encoder below, so no external tool is involved), and
//! * `assets/icons/app/AppIcon.icns`, when macOS `iconutil` is installed.
//!
//! Run it again after changing the shapes and commit the results.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Design space: the icon is drawn on a 1024 x 1024 canvas.
const SIZE: f64 = 1024.0;
const SIZES: [u32; 7] = [16, 32, 64, 128, 256, 512, 1024];

type Rgb = [f64; 3];

const TILE_TOP: Rgb = [0x4C as f64, 0x4C as f64, 0x4C as f64];
const TILE_BOTTOM: Rgb = [0x2B as f64, 0x2B as f64, 0x2B as f64];
const PAPER: Rgb = [0xF1 as f64, 0xED as f64, 0xE4 as f64];
const RED: Rgb = [0xC8 as f64, 0x36 as f64, 0x2B as f64];
const BLUE: Rgb = [0x2F as f64, 0x6C as f64, 0xB3 as f64];
const LIGHT_BLUE: Rgb = [0x8D as f64, 0xB8 as f64, 0xE8 as f64];
const YELLOW: Rgb = [0xE8 as f64, 0xB8 as f64, 0x2A as f64];
const TAG: Rgb = [0x35 as f64, 0x35 as f64, 0x35 as f64];

#[derive(Clone, Copy)]
enum Fill {
    Flat(Rgb),
    /// Top to bottom over the whole canvas.
    Vertical(Rgb, Rgb),
}

#[derive(Clone, Copy)]
enum Shape {
    RoundRect {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        r: f64,
        fill: Fill,
    },
    /// A stroked segment with butt caps.
    Line {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        w: f64,
        color: Rgb,
    },
    /// A stroked circular arc from angle `a0` to `a1` (degrees, counter-
    /// clockwise on screen from the +x axis through -y, i.e. y points up in
    /// the angle's frame) with butt caps.
    Arc {
        cx: f64,
        cy: f64,
        r: f64,
        a0: f64,
        a1: f64,
        w: f64,
        color: Rgb,
    },
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64, c: Rgb) -> Shape {
    Shape::RoundRect {
        x0,
        y0,
        x1,
        y1,
        r: 0.0,
        fill: Fill::Flat(c),
    }
}

/// The icon, bottom layer first.
fn shapes() -> Vec<Shape> {
    // Everything but the tile moves up a little so the plan and its dimension
    // string sit in the optical middle.
    let dy = -24.0;
    let mut v = vec![Shape::RoundRect {
        x0: 0.0,
        y0: 0.0,
        x1: SIZE,
        y1: SIZE,
        r: 230.0,
        fill: Fill::Vertical(TILE_TOP, TILE_BOTTOM),
    }];
    // Exterior walls: a red slab with the paper floor inside it.
    v.push(rect(200.0, 250.0 + dy, 824.0, 700.0 + dy, RED));
    v.push(rect(252.0, 302.0 + dy, 772.0, 648.0 + dy, PAPER));
    // Interior partitions (a room in the top-left corner).
    v.push(rect(252.0, 472.0 + dy, 560.0, 500.0 + dy, RED));
    v.push(rect(532.0, 302.0 + dy, 560.0, 500.0 + dy, RED));
    // The door: an opening in the bottom wall, the leaf and its swing.
    v.push(rect(320.0, 648.0 + dy, 450.0, 700.0 + dy, PAPER));
    v.push(Shape::Line {
        x0: 320.0,
        y0: 648.0 + dy,
        x1: 320.0,
        y1: 528.0 + dy,
        w: 14.0,
        color: BLUE,
    });
    v.push(Shape::Arc {
        cx: 320.0,
        cy: 648.0 + dy,
        r: 120.0,
        a0: 0.0,
        a1: 90.0,
        w: 10.0,
        color: BLUE,
    });
    // The window: an opening in the top wall with glass and sill lines.
    v.push(rect(600.0, 250.0 + dy, 744.0, 302.0 + dy, LIGHT_BLUE));
    for y in [250.0, 276.0, 302.0] {
        v.push(Shape::Line {
            x0: 600.0,
            y0: y + dy,
            x1: 744.0,
            y1: y + dy,
            w: 8.0,
            color: BLUE,
        });
    }
    // The dimension string: line, end ticks and a text tag.
    v.push(Shape::Line {
        x0: 200.0,
        y0: 790.0 + dy,
        x1: 824.0,
        y1: 790.0 + dy,
        w: 18.0,
        color: YELLOW,
    });
    for x in [200.0, 824.0] {
        v.push(Shape::Line {
            x0: x,
            y0: 756.0 + dy,
            x1: x,
            y1: 824.0 + dy,
            w: 18.0,
            color: YELLOW,
        });
    }
    v.push(Shape::RoundRect {
        x0: 416.0,
        y0: 756.0 + dy,
        x1: 608.0,
        y1: 824.0 + dy,
        r: 18.0,
        fill: Fill::Flat(TAG),
    });
    v.push(rect(446.0, 784.0 + dy, 578.0, 796.0 + dy, YELLOW));
    v
}

// ----- coverage and colour -----

fn lerp(a: Rgb, b: Rgb, t: f64) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// The colour of `shape` at the point, `None` when the point is outside it.
fn sample(shape: &Shape, x: f64, y: f64) -> Option<Rgb> {
    match *shape {
        Shape::RoundRect {
            x0,
            y0,
            x1,
            y1,
            r,
            fill,
        } => {
            if x < x0 || x > x1 || y < y0 || y > y1 {
                return None;
            }
            if r > 0.0 {
                let cx = x.clamp(x0 + r, x1 - r);
                let cy = y.clamp(y0 + r, y1 - r);
                if (x - cx).hypot(y - cy) > r {
                    return None;
                }
            }
            Some(match fill {
                Fill::Flat(c) => c,
                Fill::Vertical(a, b) => lerp(a, b, (y / SIZE).clamp(0.0, 1.0)),
            })
        }
        Shape::Line {
            x0,
            y0,
            x1,
            y1,
            w,
            color,
        } => {
            let (dx, dy) = (x1 - x0, y1 - y0);
            let len2 = dx * dx + dy * dy;
            let t = ((x - x0) * dx + (y - y0) * dy) / len2;
            if !(0.0..=1.0).contains(&t) {
                return None;
            }
            let (px, py) = (x0 + t * dx, y0 + t * dy);
            ((x - px).hypot(y - py) <= w / 2.0).then_some(color)
        }
        Shape::Arc {
            cx,
            cy,
            r,
            a0,
            a1,
            w,
            color,
        } => {
            let d = (x - cx).hypot(y - cy);
            if (d - r).abs() > w / 2.0 {
                return None;
            }
            // Screen y grows downwards; angles run counter-clockwise on screen.
            let a = (-(y - cy)).atan2(x - cx).to_degrees();
            let a = if a < 0.0 { a + 360.0 } else { a };
            (a >= a0 && a <= a1).then_some(color)
        }
    }
}

/// Rasterizes the icon at `px` x `px` with 4 x 4 samples per pixel.
fn raster(shapes: &[Shape], px: u32) -> Vec<u8> {
    const N: u32 = 4;
    let scale = SIZE / f64::from(px);
    let mut out = vec![0_u8; (px * px * 4) as usize];
    for py in 0..px {
        for pxx in 0..px {
            let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
            for sy in 0..N {
                for sx in 0..N {
                    let x = (f64::from(pxx) + (f64::from(sx) + 0.5) / f64::from(N)) * scale;
                    let y = (f64::from(py) + (f64::from(sy) + 0.5) / f64::from(N)) * scale;
                    let mut hit: Option<Rgb> = None;
                    for s in shapes {
                        if let Some(c) = sample(s, x, y) {
                            hit = Some(c);
                        }
                    }
                    if let Some(c) = hit {
                        r += c[0];
                        g += c[1];
                        b += c[2];
                        a += 1.0;
                    }
                }
            }
            let i = ((py * px + pxx) * 4) as usize;
            let total = f64::from(N * N);
            if a > 0.0 {
                out[i] = (r / a).round() as u8;
                out[i + 1] = (g / a).round() as u8;
                out[i + 2] = (b / a).round() as u8;
                out[i + 3] = (a / total * 255.0).round() as u8;
            }
        }
    }
    out
}

// ----- SVG -----

fn hex(c: Rgb) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        c[0].round() as u8,
        c[1].round() as u8,
        c[2].round() as u8
    )
}

fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn svg(shapes: &[Shape]) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1024 1024\" width=\"1024\" height=\"1024\">"
    );
    let _ = writeln!(s, "  <title>Plan Studio</title>");
    let _ = writeln!(
        s,
        "  <defs><linearGradient id=\"tile\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient></defs>",
        hex(TILE_TOP),
        hex(TILE_BOTTOM)
    );
    for shape in shapes {
        match *shape {
            Shape::RoundRect {
                x0,
                y0,
                x1,
                y1,
                r,
                fill,
            } => {
                let fill = match fill {
                    Fill::Flat(c) => hex(c),
                    Fill::Vertical(..) => "url(#tile)".to_string(),
                };
                let rx = if r > 0.0 {
                    format!(" rx=\"{}\"", num(r))
                } else {
                    String::new()
                };
                let _ = writeln!(
                    s,
                    "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx} fill=\"{fill}\"/>",
                    num(x0),
                    num(y0),
                    num(x1 - x0),
                    num(y1 - y0)
                );
            }
            Shape::Line {
                x0,
                y0,
                x1,
                y1,
                w,
                color,
            } => {
                let _ = writeln!(
                    s,
                    "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
                    num(x0),
                    num(y0),
                    num(x1),
                    num(y1),
                    hex(color),
                    num(w)
                );
            }
            Shape::Arc {
                cx,
                cy,
                r,
                a0,
                a1,
                w,
                color,
            } => {
                let pt = |a: f64| (cx + r * a.to_radians().cos(), cy - r * a.to_radians().sin());
                let (sx, sy) = pt(a0);
                let (ex, ey) = pt(a1);
                // Counter-clockwise on screen is sweep-flag 0.
                let large = u8::from(a1 - a0 > 180.0);
                let _ = writeln!(
                    s,
                    "  <path d=\"M{} {} A{} {} 0 {large} 0 {} {}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
                    num(sx),
                    num(sy),
                    num(r),
                    num(r),
                    num(ex),
                    num(ey),
                    hex(color),
                    num(w)
                );
            }
        }
    }
    s.push_str("</svg>\n");
    s
}

// ----- PNG with a small deflate encoder -----

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFF_u32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for &x in data {
        a = (a + u32::from(x)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    /// Writes `count` bits of `value`, least significant first.
    fn put(&mut self, value: u32, count: u32) {
        self.acc |= value << self.n;
        self.n += count;
        while self.n >= 8 {
            self.out.push((self.acc & 0xFF) as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// Writes a Huffman code (most significant bit first).
    fn code(&mut self, code: u32, len: u32) {
        let mut rev = 0;
        for i in 0..len {
            rev |= ((code >> i) & 1) << (len - 1 - i);
        }
        self.put(rev, len);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push((self.acc & 0xFF) as u8);
        }
        self.out
    }
}

fn fixed_literal(bits: &mut Bits, sym: u32) {
    match sym {
        0..=143 => bits.code(0x30 + sym, 8),
        144..=255 => bits.code(0x190 + (sym - 144), 9),
        256..=279 => bits.code(sym - 256, 7),
        _ => bits.code(0xC0 + (sym - 280), 8),
    }
}

const LEN_BASE: [u32; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u32; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// zlib stream: one fixed-Huffman deflate block with greedy LZ77 matching.
fn zlib_compress(data: &[u8]) -> Vec<u8> {
    const WINDOW: usize = 32_768;
    const HASH: usize = 1 << 15;
    let mut bits = Bits {
        out: vec![0x78, 0x9C],
        acc: 0,
        n: 0,
    };
    bits.put(1, 1); // final block
    bits.put(1, 2); // fixed Huffman
    let mut head = vec![usize::MAX; HASH];
    let hash = |i: usize| -> usize {
        ((usize::from(data[i]) << 10) ^ (usize::from(data[i + 1]) << 5) ^ usize::from(data[i + 2]))
            & (HASH - 1)
    };
    let mut i = 0;
    while i < data.len() {
        let mut best = (0_usize, 0_usize);
        if i + 3 <= data.len() {
            let h = hash(i);
            let cand = head[h];
            if cand != usize::MAX && i - cand <= WINDOW {
                let max = (data.len() - i).min(258);
                let mut l = 0;
                while l < max && data[cand + l] == data[i + l] {
                    l += 1;
                }
                if l >= 3 {
                    best = (l, i - cand);
                }
            }
            head[h] = i;
        }
        if best.0 >= 3 {
            let (len, dist) = (best.0 as u32, best.1 as u32);
            let li = LEN_BASE.iter().rposition(|&b| b <= len).unwrap_or(0);
            fixed_literal(&mut bits, 257 + li as u32);
            bits.put(len - LEN_BASE[li], LEN_EXTRA[li]);
            let di = DIST_BASE.iter().rposition(|&b| b <= dist).unwrap_or(0);
            bits.code(di as u32, 5);
            bits.put(dist - DIST_BASE[di], DIST_EXTRA[di]);
            // Index the skipped positions so later matches can find them.
            for k in 1..best.0 {
                if i + k + 3 <= data.len() {
                    head[hash(i + k)] = i + k;
                }
            }
            i += best.0;
        } else {
            fixed_literal(&mut bits, u32::from(data[i]));
            i += 1;
        }
    }
    fixed_literal(&mut bits, 256);
    let mut z = bits.finish();
    z.extend_from_slice(&adler32(data).to_be_bytes());
    z
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn encode_png(rgba: &[u8], px: u32) -> Vec<u8> {
    let stride = px as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * px as usize);
    for row in rgba.chunks(stride) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&px.to_be_bytes());
    ihdr.extend_from_slice(&px.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_compress(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

// ----- main -----

fn icon_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("icons")
        .join("app")
}

/// `iconutil` names: (file name inside the .iconset, pixel size).
const ICONSET: [(&str, u32); 10] = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
];

fn main() -> std::io::Result<()> {
    let dir = icon_dir();
    std::fs::create_dir_all(&dir)?;
    let shapes = shapes();
    std::fs::write(dir.join("plan-studio.svg"), svg(&shapes))?;
    println!("wrote {}", dir.join("plan-studio.svg").display());

    for px in SIZES {
        let png = encode_png(&raster(&shapes, px), px);
        let path = dir.join(format!("icon-{px}.png"));
        std::fs::write(&path, &png)?;
        println!("wrote {} ({} bytes)", path.display(), png.len());
    }

    // The macOS icon: only when iconutil is there.
    let set = std::env::temp_dir().join(format!("PlanStudio-{}.iconset", std::process::id()));
    let _ = std::fs::remove_dir_all(&set);
    std::fs::create_dir_all(&set)?;
    for (name, px) in ICONSET {
        std::fs::copy(dir.join(format!("icon-{px}.png")), set.join(name))?;
    }
    let icns = dir.join("AppIcon.icns");
    match Command::new("iconutil")
        .args(["-c", "icns", "-o"])
        .arg(&icns)
        .arg(&set)
        .status()
    {
        Ok(s) if s.success() => println!("wrote {}", icns.display()),
        Ok(s) => println!("iconutil failed ({s}); AppIcon.icns not written"),
        Err(_) => println!("iconutil not found; AppIcon.icns not written (the PNGs are enough)"),
    }
    let _ = std::fs::remove_dir_all(&set);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deflate_round_trips_through_the_apps_png_decoder() {
        let shapes = shapes();
        for px in [16_u32, 64] {
            let rgba = raster(&shapes, px);
            let png = encode_png(&rgba, px);
            let img = plan_library::image::png::decode(&png).expect("valid PNG");
            assert_eq!((img.width, img.height), (px, px));
            assert_eq!(img.rgba, rgba);
        }
    }
}
