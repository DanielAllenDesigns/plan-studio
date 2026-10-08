//! 3D models of user-library items: indexed triangle meshes in the scene
//! frame, a compact binary file format and the transforms the importers use.
//!
//! # Frame
//!
//! Positions are inches in the **natural import frame** of `plan_3d::import`:
//! X right, Y up, and the object's front facing +Z. A normalized model has
//! its bounding-box center at `x = 0`, its bottom at `y = 0` and its back face
//! at `z = 0` (see [`Model3d::normalized`]), which is the symbol-local frame
//! of a placed library symbol.
//!
//! # File format (`.psm`, "Plan Studio model")
//!
//! All numbers little-endian: `"PSM1"`, `u32` part count, then per part
//! `u32` name length + UTF-8 name, `u8` has-color + 3 color bytes, `u32`
//! vertex count + `f32 x 3` per vertex, `u32` index count + `u32` per index.
//! Chief Architect cannot read this format.

use std::fmt;

/// File magic of a `.psm` model.
pub const PSM_MAGIC: &[u8; 4] = b"PSM1";

/// One colored piece of a model (an OBJ group, a glTF primitive).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelPart {
    /// Name from the source file (may be empty).
    pub name: String,
    /// sRGB base color when the source gives one.
    pub color: Option<[u8; 3]>,
    /// Vertex positions, inches.
    pub positions: Vec<[f32; 3]>,
    /// Triangle vertex indices, three per triangle, counter-clockwise from
    /// outside.
    pub indices: Vec<u32>,
}

impl ModelPart {
    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// A model: any number of parts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Model3d {
    /// The parts, in file order.
    pub parts: Vec<ModelPart>,
}

/// Why a `.psm` file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError(pub String);

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ModelError {}

/// Min and max corner.
pub type Box3 = ([f32; 3], [f32; 3]);

impl Model3d {
    /// A model of one part.
    pub fn single(part: ModelPart) -> Model3d {
        Model3d { parts: vec![part] }
    }

    /// Total triangle count.
    pub fn triangle_count(&self) -> usize {
        self.parts.iter().map(ModelPart::triangle_count).sum()
    }

    /// Total vertex count.
    pub fn vertex_count(&self) -> usize {
        self.parts.iter().map(|p| p.positions.len()).sum()
    }

    /// True when there is no triangle at all.
    pub fn is_empty(&self) -> bool {
        self.triangle_count() == 0
    }

    /// Bounds of the vertices that a triangle uses.
    pub fn bounds(&self) -> Option<Box3> {
        let mut acc: Option<Box3> = None;
        for part in &self.parts {
            for &i in &part.indices {
                let Some(p) = part.positions.get(i as usize) else {
                    continue;
                };
                let (lo, hi) = acc.get_or_insert((*p, *p));
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
        acc
    }

    /// Width (x), height (y) and depth (z) of the bounds.
    pub fn extent(&self) -> Option<[f32; 3]> {
        let (lo, hi) = self.bounds()?;
        Some([hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]])
    }

    /// Applies `f` to every vertex.
    fn map_points(&self, f: impl Fn([f32; 3]) -> [f32; 3]) -> Model3d {
        Model3d {
            parts: self
                .parts
                .iter()
                .map(|p| ModelPart {
                    positions: p.positions.iter().map(|q| f(*q)).collect(),
                    ..p.clone()
                })
                .collect(),
        }
    }

    /// The model scaled by `factor` about the origin.
    pub fn scaled(&self, factor: f64) -> Model3d {
        let k = factor as f32;
        self.map_points(|p| [p[0] * k, p[1] * k, p[2] * k])
    }

    /// The model moved by `d`.
    pub fn translated(&self, d: [f32; 3]) -> Model3d {
        self.map_points(|p| [p[0] + d[0], p[1] + d[1], p[2] + d[2]])
    }

    /// The model turned by `deg` degrees about the vertical (Y) axis,
    /// counter-clockwise seen from above.
    pub fn rotated_y(&self, deg: f64) -> Model3d {
        let (s, c) = (deg.to_radians().sin() as f32, deg.to_radians().cos() as f32);
        self.map_points(|p| [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c])
    }

    /// The model moved so its bounding-box center is at `x = 0`, its bottom
    /// at `y = 0` and its back face at `z = 0`: the symbol-local frame.
    pub fn normalized(&self) -> Model3d {
        match self.bounds() {
            Some((lo, hi)) => self.translated([-(lo[0] + hi[0]) * 0.5, -lo[1], -lo[2]]),
            None => self.clone(),
        }
    }

    /// A box model of `width` x `depth` x `height` inches in the symbol-local
    /// frame (front at `z = depth`).
    pub fn box_model(width: f32, depth: f32, height: f32, color: Option<[u8; 3]>) -> Model3d {
        let (x0, x1) = (-width * 0.5, width * 0.5);
        let (y0, y1, z0, z1) = (0.0, height, 0.0, depth);
        let positions = vec![
            [x0, y0, z0],
            [x1, y0, z0],
            [x1, y1, z0],
            [x0, y1, z0],
            [x0, y0, z1],
            [x1, y0, z1],
            [x1, y1, z1],
            [x0, y1, z1],
        ];
        // Counter-clockwise seen from outside.
        let quads: [[u32; 4]; 6] = [
            [4, 5, 6, 7], // front (+z)
            [1, 0, 3, 2], // back (-z)
            [5, 1, 2, 6], // right (+x)
            [0, 4, 7, 3], // left (-x)
            [7, 6, 2, 3], // top (+y)
            [0, 1, 5, 4], // bottom (-y)
        ];
        let mut indices = Vec::new();
        for q in quads {
            indices.extend([q[0], q[1], q[2], q[0], q[2], q[3]]);
        }
        Model3d::single(ModelPart {
            name: "box".into(),
            color,
            positions,
            indices,
        })
    }

    /// Drops out-of-range and degenerate triangles and unused vertices, so a
    /// parsed model never carries an index that points nowhere.
    pub fn cleaned(&self) -> Model3d {
        let mut parts = Vec::new();
        for part in &self.parts {
            let n = part.positions.len();
            let mut remap = vec![u32::MAX; n];
            let mut positions = Vec::new();
            let mut indices = Vec::new();
            for t in part.indices.as_chunks::<3>().0 {
                if t.iter().any(|&i| i as usize >= n) {
                    continue;
                }
                if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                    continue;
                }
                let p = [
                    part.positions[t[0] as usize],
                    part.positions[t[1] as usize],
                    part.positions[t[2] as usize],
                ];
                if p.iter().flatten().any(|c| !c.is_finite()) {
                    continue;
                }
                for &i in t {
                    if remap[i as usize] == u32::MAX {
                        remap[i as usize] = positions.len() as u32;
                        positions.push(part.positions[i as usize]);
                    }
                    indices.push(remap[i as usize]);
                }
            }
            if !indices.is_empty() {
                parts.push(ModelPart {
                    name: part.name.clone(),
                    color: part.color,
                    positions,
                    indices,
                });
            }
        }
        Model3d { parts }
    }

    /// Serializes to the `.psm` format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(PSM_MAGIC);
        out.extend_from_slice(&(self.parts.len() as u32).to_le_bytes());
        for p in &self.parts {
            let name = p.name.as_bytes();
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name);
            match p.color {
                Some(c) => out.extend_from_slice(&[1, c[0], c[1], c[2]]),
                None => out.extend_from_slice(&[0, 0, 0, 0]),
            }
            out.extend_from_slice(&(p.positions.len() as u32).to_le_bytes());
            for v in &p.positions {
                for c in v {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
            out.extend_from_slice(&(p.indices.len() as u32).to_le_bytes());
            for i in &p.indices {
                out.extend_from_slice(&i.to_le_bytes());
            }
        }
        out
    }

    /// Parses a `.psm` file.
    pub fn from_bytes(bytes: &[u8]) -> Result<Model3d, ModelError> {
        let bad = |m: &str| ModelError(format!("not a Plan Studio model: {m}"));
        let mut r = Reader { bytes, at: 0 };
        if r.take(4).ok_or_else(|| bad("too short"))? != PSM_MAGIC {
            return Err(bad("wrong signature"));
        }
        let count = r.u32().ok_or_else(|| bad("truncated"))? as usize;
        let mut parts = Vec::new();
        for _ in 0..count.min(1 << 16) {
            let n = r.u32().ok_or_else(|| bad("truncated"))? as usize;
            let name = String::from_utf8_lossy(r.take(n).ok_or_else(|| bad("truncated"))?)
                .into_owned();
            let c = r.take(4).ok_or_else(|| bad("truncated"))?;
            let color = (c[0] == 1).then_some([c[1], c[2], c[3]]);
            let nv = r.u32().ok_or_else(|| bad("truncated"))? as usize;
            let raw = r
                .take(nv.checked_mul(12).ok_or_else(|| bad("size"))?)
                .ok_or_else(|| bad("truncated"))?;
            let positions: Vec<[f32; 3]> = raw
                .as_chunks::<12>().0.iter()
                .map(|b| {
                    let f = |k: usize| f32::from_le_bytes([b[k], b[k + 1], b[k + 2], b[k + 3]]);
                    [f(0), f(4), f(8)]
                })
                .collect();
            let ni = r.u32().ok_or_else(|| bad("truncated"))? as usize;
            let raw = r
                .take(ni.checked_mul(4).ok_or_else(|| bad("size"))?)
                .ok_or_else(|| bad("truncated"))?;
            let indices: Vec<u32> = raw
                .as_chunks::<4>().0.iter()
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            parts.push(ModelPart {
                name,
                color,
                positions,
                indices,
            });
        }
        Ok(Model3d { parts })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let s = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(s)
    }

    fn u32(&mut self) -> Option<u32> {
        let b = self.take(4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri() -> Model3d {
        Model3d::single(ModelPart {
            name: "t".into(),
            color: Some([10, 20, 30]),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]],
            indices: vec![0, 1, 2],
        })
    }

    #[test]
    fn bytes_round_trip_exactly() {
        let m = Model3d::box_model(10.0, 20.0, 30.0, None);
        let mut m2 = m.clone();
        m2.parts.push(tri().parts[0].clone());
        let back = Model3d::from_bytes(&m2.to_bytes()).unwrap();
        assert_eq!(back, m2);
        assert_eq!(back.triangle_count(), 13);
    }

    #[test]
    fn damaged_files_are_errors_not_panics() {
        assert!(Model3d::from_bytes(b"").is_err());
        assert!(Model3d::from_bytes(b"XXXX\0\0\0\0").is_err());
        let bytes = tri().to_bytes();
        for cut in 0..bytes.len() {
            let _ = Model3d::from_bytes(&bytes[..cut]);
        }
        let mut huge = bytes.clone();
        huge[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let _ = Model3d::from_bytes(&huge);
    }

    #[test]
    fn box_model_is_symbol_local_and_closed() {
        let m = Model3d::box_model(36.0, 24.0, 30.0, None);
        let (lo, hi) = m.bounds().unwrap();
        assert_eq!(lo, [-18.0, 0.0, 0.0]);
        assert_eq!(hi, [18.0, 30.0, 24.0]);
        assert_eq!(m.triangle_count(), 12);
        // Every face points out: the normal agrees with the face center.
        let p = &m.parts[0];
        for t in p.indices.as_chunks::<3>().0 {
            let a = p.positions[t[0] as usize];
            let b = p.positions[t[1] as usize];
            let c = p.positions[t[2] as usize];
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let mid = [
                (a[0] + b[0] + c[0]) / 3.0 - 0.0,
                (a[1] + b[1] + c[1]) / 3.0 - 15.0,
                (a[2] + b[2] + c[2]) / 3.0 - 12.0,
            ];
            assert!(n[0] * mid[0] + n[1] * mid[1] + n[2] * mid[2] > 0.0);
        }
    }

    #[test]
    fn normalizing_centers_x_and_sits_on_the_floor() {
        let m = tri().translated([5.0, 3.0, -7.0]).normalized();
        let (lo, hi) = m.bounds().unwrap();
        assert!((lo[0] + hi[0]).abs() < 1e-6);
        assert_eq!((lo[1], lo[2]), (0.0, 0.0));
    }

    #[test]
    fn rotation_turns_about_the_vertical_axis() {
        let m = Model3d::box_model(10.0, 4.0, 2.0, None).rotated_y(90.0);
        let e = m.extent().unwrap();
        assert!((e[0] - 4.0).abs() < 1e-4 && (e[2] - 10.0).abs() < 1e-4 && (e[1] - 2.0).abs() < 1e-4);
    }

    #[test]
    fn cleaning_drops_bad_triangles_and_unused_vertices() {
        let m = Model3d::single(ModelPart {
            name: String::new(),
            color: None,
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [9.0, 9.0, 9.0],
                [f32::NAN, 0.0, 0.0],
            ],
            indices: vec![0, 1, 2, 0, 0, 1, 0, 1, 99, 0, 1, 4],
        });
        let c = m.cleaned();
        assert_eq!(c.triangle_count(), 1);
        assert_eq!(c.vertex_count(), 3);
    }
}
