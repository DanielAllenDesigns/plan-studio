//! Wavefront OBJ reader for 3D library symbols.
//!
//! Reads `v`, `f` (any polygon, triangulated by ear clipping), `g` / `o`
//! (parts) and `usemtl`; an optional `.mtl` text supplies each material's
//! diffuse color (`Kd`). Normals, texture coordinates, smoothing groups and
//! free-form curves are ignored: Plan Studio shades imported models flat.
//! Face indices may be absolute, negative (relative) and in any of the
//! `v`, `v/vt`, `v//vn`, `v/vt/vn` forms.
//!
//! The result is converted with [`ModelOptions`] (source unit and up axis) and
//! is in the natural import frame (see [`crate::model`]).

use crate::model::{ImportedModel, ImportedPart, ModelError, ModelOptions};
use std::collections::HashMap;

/// Diffuse colors (`Kd`, 0..1 mapped to 0..255) by material name from the
/// text of a `.mtl` file.
pub fn parse_mtl(text: &str) -> HashMap<String, [u8; 3]> {
    let mut out = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut it = line.split_whitespace();
        match it.next() {
            Some("newmtl") => current = Some(it.collect::<Vec<_>>().join(" ")),
            Some("Kd") => {
                let v: Vec<f32> = it.filter_map(|t| t.parse().ok()).collect();
                if let (Some(name), [r, g, b, ..]) = (&current, v.as_slice()) {
                    let q = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
                    out.insert(name.clone(), [q(*r), q(*g), q(*b)]);
                }
            }
            _ => {}
        }
    }
    out
}

/// The `mtllib` file names an OBJ mentions, so a caller can load them.
pub fn mtl_libraries(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            l.strip_prefix("mtllib").map(|r| r.trim().to_string())
        })
        .filter(|n| !n.is_empty())
        .collect()
}

struct PartBuilder {
    name: String,
    material: Option<String>,
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
    /// Global vertex index -> index in `positions`.
    remap: HashMap<usize, u32>,
}

impl PartBuilder {
    fn new(name: &str, material: Option<String>) -> Self {
        PartBuilder {
            name: name.to_string(),
            material,
            positions: Vec::new(),
            indices: Vec::new(),
            remap: HashMap::new(),
        }
    }

    fn local(&mut self, global: usize, all: &[[f32; 3]]) -> u32 {
        *self.remap.entry(global).or_insert_with(|| {
            self.positions.push(all[global]);
            (self.positions.len() - 1) as u32
        })
    }
}

/// Resolves an OBJ index (1-based, or negative from the end) against `len`.
fn resolve(token: &str, len: usize) -> Option<usize> {
    let n: i64 = token.split('/').next()?.parse().ok()?;
    let idx = if n > 0 {
        n - 1
    } else if n < 0 {
        len as i64 + n
    } else {
        return None;
    };
    (idx >= 0 && (idx as usize) < len).then_some(idx as usize)
}

/// Triangulates a polygon given as 3D points; returns triangles as indices
/// into `pts`. Convex polygons come out as a fan; concave ones are ear-clipped
/// in the plane of the polygon's average normal.
pub fn triangulate_polygon(pts: &[[f32; 3]]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return vec![[0, 1, 2]];
    }
    // Newell normal.
    let mut nrm = [0.0f64; 3];
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        nrm[0] += (a[1] as f64 - b[1] as f64) * (a[2] as f64 + b[2] as f64);
        nrm[1] += (a[2] as f64 - b[2] as f64) * (a[0] as f64 + b[0] as f64);
        nrm[2] += (a[0] as f64 - b[0] as f64) * (a[1] as f64 + b[1] as f64);
    }
    // Drop the dominant axis.
    let axis = (0..3)
        .max_by(|&i, &j| nrm[i].abs().total_cmp(&nrm[j].abs()))
        .unwrap_or(2);
    let sign = if nrm[axis] < 0.0 { -1.0 } else { 1.0 };
    let (iu, iv) = match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let p2: Vec<[f64; 2]> = pts
        .iter()
        .map(|p| [p[iu] as f64, sign * p[iv] as f64])
        .collect();
    let cross = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    let inside = |p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0
    };
    let mut rest: Vec<usize> = (0..n).collect();
    let mut out = Vec::new();
    let mut guard = 0;
    while rest.len() > 3 && guard < n * n {
        guard += 1;
        let m = rest.len();
        let mut clipped = false;
        for k in 0..m {
            let (ia, ib, ic) = (rest[(k + m - 1) % m], rest[k], rest[(k + 1) % m]);
            let (a, b, c) = (p2[ia], p2[ib], p2[ic]);
            if cross(a, b, c) <= 1e-12 {
                continue;
            }
            let blocked = rest
                .iter()
                .any(|&j| j != ia && j != ib && j != ic && inside(p2[j], a, b, c));
            if blocked {
                continue;
            }
            out.push([ia, ib, ic]);
            rest.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            // Degenerate polygon: finish as a fan.
            break;
        }
    }
    if rest.len() >= 3 {
        for k in 1..rest.len() - 1 {
            out.push([rest[0], rest[k], rest[k + 1]]);
        }
    }
    out
}

/// Parses OBJ `text`. `mtl` is the text of the material library, if loaded.
/// Fails when the file holds no face.
pub fn parse_obj(
    text: &str,
    mtl: Option<&str>,
    opts: &ModelOptions,
) -> Result<ImportedModel, ModelError> {
    let colors = mtl.map(parse_mtl).unwrap_or_default();
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut parts: Vec<PartBuilder> = Vec::new();
    let mut name = String::new();
    let mut material: Option<String> = None;
    let mut current: Option<usize> = None;
    let mut faces = 0usize;

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        let mut it = line.split_whitespace();
        let Some(tag) = it.next() else { continue };
        match tag {
            "v" => {
                let c: Vec<f32> = it.take(3).filter_map(|t| t.parse().ok()).collect();
                if c.len() == 3 {
                    verts.push([c[0], c[1], c[2]]);
                } else {
                    return Err(ModelError(format!("Bad vertex line: {line}")));
                }
            }
            "g" | "o" => {
                name = it.collect::<Vec<_>>().join(" ");
                current = None;
            }
            "usemtl" => {
                material = Some(it.collect::<Vec<_>>().join(" "));
                current = None;
            }
            "f" => {
                let tokens: Vec<&str> = it.collect();
                let idx: Vec<usize> = tokens
                    .iter()
                    .filter_map(|t| resolve(t, verts.len()))
                    .collect();
                if idx.len() < 3 || idx.len() != tokens.len() {
                    continue;
                }
                let part = match current {
                    Some(p) => p,
                    None => {
                        let found = parts
                            .iter()
                            .position(|p| p.name == name && p.material == material);
                        let p = found.unwrap_or_else(|| {
                            parts.push(PartBuilder::new(&name, material.clone()));
                            parts.len() - 1
                        });
                        current = Some(p);
                        p
                    }
                };
                let pts: Vec<[f32; 3]> = idx.iter().map(|&i| verts[i]).collect();
                for tri in triangulate_polygon(&pts) {
                    let b = &mut parts[part];
                    for k in tri {
                        let l = b.local(idx[k], &verts);
                        b.indices.push(l);
                    }
                    faces += 1;
                }
            }
            _ => {}
        }
    }
    if faces == 0 {
        return Err(ModelError("The OBJ file has no faces".into()));
    }
    let model = ImportedModel {
        parts: parts
            .into_iter()
            .map(|b| ImportedPart {
                color: b.material.as_ref().and_then(|m| colors.get(m)).copied(),
                name: b.name,
                positions: b.positions,
                indices: b.indices,
            })
            .collect(),
    };
    let model = model.cleaned().converted(opts);
    if model.is_empty() {
        return Err(ModelError("The OBJ file has no usable faces".into()));
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::UpAxis;

    const CUBE: &str = "\
# a unit cube, quads
mtllib cube.mtl
o Cube
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
v 0 0 1
v 1 0 1
v 1 1 1
v 0 1 1
usemtl red
f 5/1/1 6/2/1 7/3/1 8/4/1
f 2 1 4 3
f 6 2 3 7
f 1 5 8 4
usemtl blue
f 4 8 7 3
f 1 2 6 5
";
    const MTL: &str = "newmtl red\nKd 1 0 0\nnewmtl blue\nKd 0 0 0.5\n";

    #[test]
    fn a_cube_of_quads_is_twelve_triangles_in_two_colored_parts() {
        let m = parse_obj(CUBE, Some(MTL), &ModelOptions::default()).unwrap();
        assert_eq!(m.triangle_count(), 12);
        assert_eq!(m.parts.len(), 2);
        assert_eq!(m.parts[0].color, Some([255, 0, 0]));
        assert_eq!(m.parts[1].color, Some([0, 0, 128]));
        assert_eq!(m.parts[0].triangle_count(), 8);
        assert_eq!(m.extent().unwrap(), [1.0, 1.0, 1.0]);
        assert_eq!(mtl_libraries(CUBE), ["cube.mtl"]);
    }

    #[test]
    fn units_up_axis_and_negative_indices() {
        let text = "v 0 0 0\nv 10 0 0\nv 10 20 0\nv 0 20 0\nf -4 -3 -2 -1\n";
        let m = parse_obj(
            text,
            None,
            &ModelOptions {
                unit_scale: 2.0,
                up_axis: UpAxis::Z,
            },
        )
        .unwrap();
        assert_eq!(m.triangle_count(), 2);
        // Z up: the 10 x 20 sheet lies flat; y (up) is 0, z spans -40..0.
        let (lo, hi) = m.bounds().unwrap();
        assert_eq!((hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]), (20.0, 0.0, 40.0));
    }

    #[test]
    fn concave_polygons_are_ear_clipped_not_fanned() {
        // An L shape: the fan from vertex 0 would cover the notch.
        let l = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
        ];
        let tris = triangulate_polygon(&l);
        assert_eq!(tris.len(), 4);
        let area: f32 = tris
            .iter()
            .map(|t| {
                let (a, b, c) = (l[t[0]], l[t[1]], l[t[2]]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0
            })
            .sum();
        assert!((area - 3.0).abs() < 1e-5, "{area}");
        // Clockwise input works too.
        let rev: Vec<[f32; 3]> = l.iter().rev().copied().collect();
        assert_eq!(triangulate_polygon(&rev).len(), 4);
    }

    #[test]
    fn bad_files_are_errors() {
        assert!(parse_obj("", None, &ModelOptions::default()).is_err());
        assert!(parse_obj("v 1 2\n", None, &ModelOptions::default()).is_err());
        assert!(parse_obj("v 0 0 0\nv 1 0 0\nf 1 2 3\n", None, &ModelOptions::default()).is_err());
        // A face that points past the vertex list is skipped.
        let r = parse_obj(
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\nf 1 2 9\n",
            None,
            &ModelOptions::default(),
        )
        .unwrap();
        assert_eq!(r.triangle_count(), 1);
    }
}
