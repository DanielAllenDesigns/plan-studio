//! STL reader (binary and ASCII) for 3D library symbols.
//!
//! STL is a bag of triangles with no units, no colors (a few exporters put a
//! `COLOR=r g b a` in the 80-byte header of the binary form, which is used as
//! the part color) and no shared vertices; equal vertices are welded here so
//! the model has indices. The result is converted with [`ModelOptions`]
//! (source unit and up axis; STL is conventionally Z up) like the other
//! readers and is in the natural import frame (see [`crate::model`]).

use crate::model::{ImportedModel, ImportedPart, ModelError, ModelOptions};
use std::collections::HashMap;

/// Welds equal vertices of a triangle soup.
#[derive(Default)]
struct Welder {
    index: HashMap<[u32; 3], u32>,
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl Welder {
    fn push(&mut self, p: [f32; 3]) {
        // `-0.0` and `0.0` are the same vertex.
        let key = p.map(|c| (c + 0.0).to_bits());
        let positions = &mut self.positions;
        let i = *self.index.entry(key).or_insert_with(|| {
            positions.push(p);
            (positions.len() - 1) as u32
        });
        self.indices.push(i);
    }
}

/// True when `bytes` is laid out as a binary STL (header, count, 50 bytes per
/// facet).
pub fn is_binary_stl(bytes: &[u8]) -> bool {
    if bytes.len() < 84 {
        return false;
    }
    let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    n.checked_mul(50)
        .and_then(|b| b.checked_add(84))
        .is_some_and(|total| total == bytes.len())
}

/// The `COLOR=r g b a` of a binary header, when present.
fn header_color(header: &[u8]) -> Option<[u8; 3]> {
    let at = header.windows(6).position(|w| w == b"COLOR=")?;
    let rgb = header.get(at + 6..at + 9)?;
    Some([rgb[0], rgb[1], rgb[2]])
}

fn parse_binary(bytes: &[u8]) -> Result<(Welder, Option<[u8; 3]>), ModelError> {
    let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let mut w = Welder::default();
    let f = |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    for k in 0..n {
        let base = 84 + k * 50 + 12; // skip the normal
        for v in 0..3 {
            let o = base + v * 12;
            w.push([f(o), f(o + 4), f(o + 8)]);
        }
    }
    Ok((w, header_color(&bytes[..80])))
}

fn parse_ascii(text: &str) -> Result<(Welder, String), ModelError> {
    let mut w = Welder::default();
    let mut name = String::new();
    let mut tri: Vec<[f32; 3]> = Vec::with_capacity(3);
    for raw in text.lines() {
        let mut it = raw.split_whitespace();
        match it.next().map(str::to_ascii_lowercase).as_deref() {
            Some("solid") if name.is_empty() => name = it.collect::<Vec<_>>().join(" "),
            Some("vertex") => {
                let c: Vec<f32> = it.take(3).filter_map(|t| t.parse().ok()).collect();
                if c.len() != 3 {
                    return Err(ModelError(format!("Bad STL vertex line: {}", raw.trim())));
                }
                tri.push([c[0], c[1], c[2]]);
            }
            Some("endfacet") => {
                if tri.len() == 3 {
                    for p in tri.drain(..) {
                        w.push(p);
                    }
                }
                tri.clear();
            }
            _ => {}
        }
    }
    Ok((w, name))
}

/// Parses a binary or ASCII STL file. Fails when it holds no triangle.
pub fn parse_stl(bytes: &[u8], opts: &ModelOptions) -> Result<ImportedModel, ModelError> {
    let (w, name, color) = if is_binary_stl(bytes) {
        let (w, c) = parse_binary(bytes)?;
        (w, String::new(), c)
    } else {
        let text = String::from_utf8_lossy(bytes);
        if !text.trim_start().to_ascii_lowercase().starts_with("solid") {
            return Err(ModelError("Not an STL file".into()));
        }
        let (w, name) = parse_ascii(&text)?;
        (w, name, None)
    };
    if w.indices.is_empty() {
        return Err(ModelError("The STL file has no triangles".into()));
    }
    let model = ImportedModel {
        parts: vec![ImportedPart {
            name,
            color,
            positions: w.positions,
            indices: w.indices,
            ..ImportedPart::default()
        }],
    };
    let model = model.cleaned().converted(opts);
    if model.is_empty() {
        return Err(ModelError("The STL file has no usable triangles".into()));
    }
    Ok(model)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::UpAxis;

    /// The 12 triangles of an axis-aligned box `w x d x h` from the origin,
    /// counter-clockwise from outside.
    pub(crate) fn box_tris(w: f32, d: f32, h: f32) -> Vec<[[f32; 3]; 3]> {
        let v = [
            [0.0, 0.0, 0.0],
            [w, 0.0, 0.0],
            [w, d, 0.0],
            [0.0, d, 0.0],
            [0.0, 0.0, h],
            [w, 0.0, h],
            [w, d, h],
            [0.0, d, h],
        ];
        let quads: [[usize; 4]; 6] = [
            [3, 2, 1, 0], // bottom
            [4, 5, 6, 7], // top
            [0, 1, 5, 4], // -y
            [1, 2, 6, 5], // +x
            [2, 3, 7, 6], // +y
            [3, 0, 4, 7], // -x
        ];
        quads
            .iter()
            .flat_map(|q| [[v[q[0]], v[q[1]], v[q[2]]], [v[q[0]], v[q[2]], v[q[3]]]])
            .collect()
    }

    /// Binary STL bytes of triangles.
    pub(crate) fn binary(tris: &[[[f32; 3]; 3]], header_text: &str) -> Vec<u8> {
        let mut out = vec![0u8; 80];
        out[..header_text.len()].copy_from_slice(header_text.as_bytes());
        out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
        for t in tris {
            out.extend_from_slice(&[0u8; 12]);
            for p in t {
                for c in p {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
            out.extend_from_slice(&[0u8; 2]);
        }
        out
    }

    /// ASCII STL text of triangles.
    pub(crate) fn ascii(tris: &[[[f32; 3]; 3]], name: &str) -> String {
        let mut s = format!("solid {name}\n");
        for t in tris {
            s.push_str("  facet normal 0 0 0\n    outer loop\n");
            for p in t {
                s.push_str(&format!("      vertex {} {} {}\n", p[0], p[1], p[2]));
            }
            s.push_str("    endloop\n  endfacet\n");
        }
        s.push_str(&format!("endsolid {name}\n"));
        s
    }

    #[test]
    fn binary_box_is_welded_to_eight_vertices() {
        let bytes = binary(&box_tris(10.0, 20.0, 30.0), "Plan Studio test");
        assert!(is_binary_stl(&bytes));
        let m = parse_stl(&bytes, &ModelOptions::default()).unwrap();
        assert_eq!(m.triangle_count(), 12);
        assert_eq!(m.parts[0].positions.len(), 8);
        assert_eq!(m.extent().unwrap(), [10.0, 20.0, 30.0]);
    }

    #[test]
    fn ascii_matches_binary() {
        let tris = box_tris(4.0, 5.0, 6.0);
        let a = parse_stl(ascii(&tris, "crate").as_bytes(), &ModelOptions::default()).unwrap();
        let b = parse_stl(&binary(&tris, ""), &ModelOptions::default()).unwrap();
        assert_eq!(a.parts[0].name, "crate");
        assert_eq!(a.parts[0].positions, b.parts[0].positions);
        assert_eq!(a.parts[0].indices, b.parts[0].indices);
    }

    #[test]
    fn a_binary_file_whose_header_says_solid_is_still_binary() {
        let bytes = binary(&box_tris(1.0, 1.0, 1.0), "solid looks ascii");
        assert!(is_binary_stl(&bytes));
        assert_eq!(
            parse_stl(&bytes, &ModelOptions::default())
                .unwrap()
                .triangle_count(),
            12
        );
    }

    #[test]
    fn z_up_and_units_are_applied() {
        let bytes = binary(&box_tris(1000.0, 500.0, 2000.0), "");
        let opts = ModelOptions {
            unit_scale: 1.0 / 25.4,
            up_axis: UpAxis::Z,
        };
        let m = parse_stl(&bytes, &opts).unwrap();
        let e = m.extent().unwrap();
        // width 1000 mm, depth 500 mm, height 2000 mm in inches.
        assert!((e[0] - 39.37).abs() < 0.01, "{e:?}");
        assert!((e[1] - 78.74).abs() < 0.01, "{e:?}");
        assert!((e[2] - 19.685).abs() < 0.01, "{e:?}");
    }

    #[test]
    fn header_color_becomes_the_part_color() {
        let mut head = b"COLOR=".to_vec();
        head.extend_from_slice(&[200, 100, 50, 255]);
        let mut bytes = binary(&box_tris(1.0, 1.0, 1.0), "");
        bytes[..head.len()].copy_from_slice(&head);
        let m = parse_stl(&bytes, &ModelOptions::default()).unwrap();
        assert_eq!(m.parts[0].color, Some([200, 100, 50]));
    }

    #[test]
    fn junk_and_empty_files_are_refused() {
        assert!(parse_stl(b"hello", &ModelOptions::default()).is_err());
        assert!(parse_stl(b"solid x\nendsolid x\n", &ModelOptions::default()).is_err());
        assert!(parse_stl(&binary(&[], ""), &ModelOptions::default()).is_err());
        // Truncated binary: the count promises more than the file holds.
        let mut bytes = binary(&box_tris(1.0, 1.0, 1.0), "");
        bytes.truncate(bytes.len() - 10);
        assert!(parse_stl(&bytes, &ModelOptions::default()).is_err());
    }
}
