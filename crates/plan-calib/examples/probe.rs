//! Scratch probe used to reverse engineer the Chief blobs.
//!
//! ```text
//! cargo run -p plan-calib --release --example probe -- <catalog.calib> <out-dir> [N] [name-filter]
//! ```
//!
//! For the first `N` objects (default 20) whose name contains the filter it
//! writes `<id>-<name>.txt` into `out-dir` with the hex dump and an f32/f64
//! view of the `Data` blob, the `symDxf`/`symBlock` blobs, the `SymbolData`
//! head and the `AssociatedData` binary tail, then prints a summary, including
//! how many `symDxf` blobs parse as AutoCAD-style binary DXF (they do not; they
//! are polygon face streams, see `decode::parse_face_stream`). The dumps are
//! derived from licensed Chief content: write them outside the repository.

use plan_calib::decode::{self, split_associated};
use plan_calib::ChiefCatalog;
use std::fmt::Write as _;
use std::path::Path;

fn hexdump(b: &[u8], max: usize) -> String {
    let mut s = String::new();
    for (i, row) in b.chunks(16).take(max / 16).enumerate() {
        let hex: Vec<String> = row.iter().map(|x| format!("{x:02x}")).collect();
        let asc: String = row
            .iter()
            .map(|&c| {
                if (32..127).contains(&c) {
                    c as char
                } else {
                    '.'
                }
            })
            .collect();
        let _ = writeln!(s, "{:06x}  {:<47}  {asc}", i * 16, hex.join(" "));
    }
    s
}

/// Plausible numbers at every 4-byte offset: (offset, f32, f64 when 8-aligned).
fn float_view(b: &[u8], max: usize) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i + 4 <= b.len().min(max) {
        let f = f32::from_le_bytes(b[i..i + 4].try_into().unwrap());
        let d = b
            .get(i..i + 8)
            .map(|x| f64::from_le_bytes(x.try_into().unwrap()));
        let f_ok = f.is_finite() && (0.01..5000.0).contains(&f.abs());
        let d_ok = d.is_some_and(|d| d.is_finite() && (0.01..5000.0).contains(&d.abs()));
        if f_ok || d_ok {
            let _ = writeln!(
                s,
                "{i:06x}  f32={f:<12.4} f64={}",
                d.map_or(String::new(), |d| format!("{d:.4}"))
            );
        }
        i += 4;
    }
    s
}

/// Fraction of the blob consumed by a strict binary-DXF reader (2-byte group
/// code, typed value) starting at `start`.
fn binary_dxf_fraction(b: &[u8], start: usize) -> f64 {
    let mut p = start;
    while p + 2 <= b.len() {
        let code = u16::from_le_bytes([b[p], b[p + 1]]);
        let step = match code {
            0..=9 | 100..=102 | 300..=369 | 1000 => match b[p + 2..].iter().position(|&c| c == 0) {
                Some(n) => 2 + n + 1,
                None => break,
            },
            10..=59 | 110..=149 | 210..=239 | 1010..=1059 => 10,
            60..=79 | 170..=179 | 270..=289 | 370..=389 | 1060..=1070 => 4,
            90..=99 | 160..=169 | 400..=409 | 1071 => 6,
            _ => break,
        };
        p += step;
    }
    p as f64 / b.len().max(1) as f64
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: probe <catalog.calib> <out-dir> [N] [name-filter]");
        return;
    }
    let cat = ChiefCatalog::open(&args[1]).expect("open");
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out).expect("out dir");
    let n: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20);
    let filter = args.get(4).map(|s| s.to_lowercase()).unwrap_or_default();

    let (mut dxf_blobs, mut dxf_binary_ok, mut faces_ok) = (0, 0, 0);
    let mut dumped = 0;
    for obj in cat.objects().expect("objects") {
        let blobs = cat.object_blobs(obj.library_object_id).expect("blobs");
        if let Some(dx) = &blobs.sym_dxf {
            dxf_blobs += 1;
            if [0, 2, 6].iter().any(|&s| binary_dxf_fraction(dx, s) >= 0.9) {
                dxf_binary_ok += 1;
            }
            if decode::parse_face_stream(dx).is_some() {
                faces_ok += 1;
            }
        }
        if dumped >= n || !obj.name.to_lowercase().contains(&filter) {
            continue;
        }
        dumped += 1;
        let mut t = format!(
            "# {} (id {}, type {})\n",
            obj.name, obj.library_object_id, obj.type_code
        );
        let mut section = |title: &str, b: &[u8], dump: usize| {
            let _ = writeln!(t, "\n## {title}: {} bytes\n{}", b.len(), hexdump(b, dump));
            let _ = writeln!(t, "### number view\n{}", float_view(b, dump));
        };
        if let Some(b) = &blobs.data {
            section("Data", b, 4096);
        }
        if let Some(b) = &blobs.sym_dxf {
            section("symDxf (head)", b, 2048);
        }
        if let Some(b) = &blobs.sym_block {
            section("symBlock (head)", b, 512);
        }
        if let Some(b) = &blobs.symbol_data {
            section("SymbolData (head)", b, 1024);
        }
        if let Some(b) = &blobs.associated {
            section("AssociatedData tail (head)", split_associated(b).1, 2048);
        }
        let safe: String = obj
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        std::fs::write(out.join(format!("{}-{safe}.txt", obj.library_object_id)), t)
            .expect("write");
    }
    println!("dumped {dumped} objects to {}", out.display());
    println!(
        "symDxf blobs: {dxf_blobs}; parse as binary DXF (>=90% consumed): {dxf_binary_ok}; parse as face stream: {faces_ok}"
    );
}
