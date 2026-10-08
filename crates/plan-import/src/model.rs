//! The result of reading a 3D file: parts of indexed triangles in inches.
//!
//! `plan-import` does not depend on `plan-library`; the app turns an
//! [`ImportedModel`] into a `plan_library::Model3d` part by part (the fields
//! match). Coordinates are in the **natural import frame**: X right, Y up,
//! the model's front facing +Z.

use std::fmt;

/// Inches per meter.
pub const INCHES_PER_METER: f64 = 39.370_078_740_157_48;

/// Source units the import dialog offers, with their inches per unit.
pub const UNITS: [(&str, f64); 5] = [
    ("Inches", 1.0),
    ("Feet", 12.0),
    ("Millimeters", INCHES_PER_METER / 1000.0),
    ("Centimeters", INCHES_PER_METER / 100.0),
    ("Meters", INCHES_PER_METER),
];

/// Which source axis points up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpAxis {
    /// Y up, front towards +Z (most OBJ exporters, all glTF).
    #[default]
    Y,
    /// Z up, front towards -Y (CAD and Chief Architect).
    Z,
}

/// How to bring parsed coordinates into inches and the import frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelOptions {
    /// Inches per source unit.
    pub unit_scale: f64,
    /// Which source axis is up.
    pub up_axis: UpAxis,
}

impl Default for ModelOptions {
    fn default() -> Self {
        ModelOptions {
            unit_scale: 1.0,
            up_axis: UpAxis::Y,
        }
    }
}

/// Why a 3D file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError(pub String);

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ModelError {}

/// One colored piece (an OBJ group or material run, a glTF primitive).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedPart {
    /// Name from the file (may be empty).
    pub name: String,
    /// sRGB color when the file gives one.
    pub color: Option<[u8; 3]>,
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Triangle indices, three per triangle.
    pub indices: Vec<u32>,
}

impl ImportedPart {
    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// A parsed 3D file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportedModel {
    /// The parts, in file order.
    pub parts: Vec<ImportedPart>,
}

impl ImportedModel {
    /// Total triangle count.
    pub fn triangle_count(&self) -> usize {
        self.parts.iter().map(|p| p.indices.len() / 3).sum()
    }

    /// True when there is no triangle.
    pub fn is_empty(&self) -> bool {
        self.triangle_count() == 0
    }

    /// Bounds of the vertices a triangle uses.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut acc: Option<([f32; 3], [f32; 3])> = None;
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

    /// Size along x, y and z.
    pub fn extent(&self) -> Option<[f32; 3]> {
        let (lo, hi) = self.bounds()?;
        Some([hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]])
    }

    /// Drops out-of-range, repeated-index, non-finite triangles and unused
    /// vertices; drops parts left empty.
    pub fn cleaned(&self) -> ImportedModel {
        let mut parts = Vec::new();
        for part in &self.parts {
            let n = part.positions.len();
            let mut remap = vec![u32::MAX; n];
            let mut positions = Vec::new();
            let mut indices = Vec::new();
            for t in part.indices.as_chunks::<3>().0 {
                if t.iter().any(|&i| i as usize >= n)
                    || t[0] == t[1]
                    || t[1] == t[2]
                    || t[0] == t[2]
                {
                    continue;
                }
                let finite = t
                    .iter()
                    .all(|&i| part.positions[i as usize].iter().all(|c| c.is_finite()));
                if !finite {
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
                parts.push(ImportedPart {
                    name: part.name.clone(),
                    color: part.color,
                    positions,
                    indices,
                });
            }
        }
        ImportedModel { parts }
    }

    /// Scales by `opts.unit_scale`; a Z-up source is mapped with
    /// `(x, y, z) -> (x, z, -y)` (a rotation: winding is kept).
    pub fn converted(&self, opts: &ModelOptions) -> ImportedModel {
        let k = opts.unit_scale as f32;
        ImportedModel {
            parts: self
                .parts
                .iter()
                .map(|p| ImportedPart {
                    positions: p
                        .positions
                        .iter()
                        .map(|q| match opts.up_axis {
                            UpAxis::Y => [q[0] * k, q[1] * k, q[2] * k],
                            UpAxis::Z => [q[0] * k, q[2] * k, -q[1] * k],
                        })
                        .collect(),
                    ..p.clone()
                })
                .collect(),
        }
    }
}
