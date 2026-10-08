//! Framing members: a lumber box with a position, an orientation and a mesh.

use crate::lumber::{format_inches, Lumber};
use plan_3d::{Material, Mesh, Vertex};
use plan_core::Id;
use serde::{Deserialize, Serialize};

/// A 3D vector in the 3D frame (X right, Y up, Z = -plan y), inches.
pub type Vec3 = [f64; 3];

pub(crate) fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(crate) fn scale(a: Vec3, k: f64) -> Vec3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub(crate) fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// What a member is for. Mirrors Chief's framing member types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemberKind {
    Stud,
    KingStud,
    TrimmerStud,
    CrippleStud,
    TopPlate,
    BottomPlate,
    Header,
    Sill,
    RimJoist,
    Joist,
    Blocking,
    Ledger,
}

impl MemberKind {
    /// Lower-case name used in takeoff lines.
    pub fn name(&self) -> &'static str {
        match self {
            MemberKind::Stud => "stud",
            MemberKind::KingStud => "king stud",
            MemberKind::TrimmerStud => "trimmer",
            MemberKind::CrippleStud => "cripple",
            MemberKind::TopPlate => "top plate",
            MemberKind::BottomPlate => "bottom plate",
            MemberKind::Header => "header",
            MemberKind::Sill => "sill",
            MemberKind::RimJoist => "rim joist",
            MemberKind::Joist => "joist",
            MemberKind::Blocking => "blocking",
            MemberKind::Ledger => "ledger",
        }
    }
}

/// Position and orientation of a member's box.
///
/// `origin` is the centre of the member's start end face. `axis_x` is the unit
/// length direction, `axis_y` the unit depth direction; the thickness direction
/// is [`Transform3::axis_z`] = `axis_x × axis_y`. Depth and thickness are both
/// centred on `origin`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform3 {
    pub origin: Vec3,
    pub axis_x: Vec3,
    pub axis_y: Vec3,
}

impl Transform3 {
    /// Unit thickness direction, `axis_x × axis_y`.
    pub fn axis_z(&self) -> Vec3 {
        cross(self.axis_x, self.axis_y)
    }
}

/// One piece of lumber: a `length × lumber.depth × lumber.thickness` box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub kind: MemberKind,
    pub lumber: Lumber,
    pub length: f64,
    pub transform: Transform3,
    /// The wall this member frames, if any (floor members have none).
    pub wall_id: Option<Id>,
    /// Cut-list label, e.g. `"2x6 x 92 5/8"`.
    pub label: String,
}

impl Member {
    /// Build a member, deriving its label from the lumber and length.
    pub fn new(
        kind: MemberKind,
        lumber: Lumber,
        length: f64,
        transform: Transform3,
        wall_id: Option<Id>,
    ) -> Self {
        let label = format!("{} x {}", lumber.nominal_name(), format_inches(length));
        Self {
            kind,
            lumber,
            length,
            transform,
            wall_id,
            label,
        }
    }

    /// The eight box corners. Bit 0 of the index selects the far end of the
    /// length, bit 1 the +depth side, bit 2 the +thickness side.
    pub fn corners(&self) -> [Vec3; 8] {
        let t = &self.transform;
        let (az, hd, ht) = (
            t.axis_z(),
            self.lumber.depth / 2.0,
            self.lumber.thickness / 2.0,
        );
        std::array::from_fn(|i| {
            let along = if i & 1 == 0 { 0.0 } else { self.length };
            let up = if i & 2 == 0 { -hd } else { hd };
            let out = if i & 4 == 0 { -ht } else { ht };
            add(
                add(t.origin, scale(t.axis_x, along)),
                add(scale(t.axis_y, up), scale(az, out)),
            )
        })
    }

    /// A 24-vertex, 12-triangle box mesh with outward unit normals.
    ///
    /// Deviation: [`Material`] has no wood variant, so `WallExterior` (a tan
    /// colour) stands in. The mesh `object_id` is the member's wall, if any.
    pub fn mesh(&self) -> Mesh {
        let t = &self.transform;
        let axes = [t.axis_x, t.axis_y, t.axis_z()];
        let size = [self.length, self.lumber.depth, self.lumber.thickness];
        let centre = add(t.origin, scale(t.axis_x, self.length / 2.0));
        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for face in 0..6 {
            let k = face / 2;
            let sign = if face % 2 == 0 { 1.0 } else { -1.0 };
            // (x, y, z) is right-handed, so a×b = n for the cyclic tangents.
            let (mut ia, mut ib) = ((k + 1) % 3, (k + 2) % 3);
            if sign < 0.0 {
                std::mem::swap(&mut ia, &mut ib);
            }
            let (a, b, sa, sb) = (axes[ia], axes[ib], size[ia], size[ib]);
            let normal = scale(axes[k], sign);
            let fc = add(centre, scale(axes[k], sign * size[k] / 2.0));
            let base = vertices.len() as u32;
            for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let p = add(fc, add(scale(a, (u - 0.5) * sa), scale(b, (v - 0.5) * sb)));
                vertices.push(Vertex {
                    position: [p[0] as f32, p[1] as f32, p[2] as f32],
                    normal: [normal[0] as f32, normal[1] as f32, normal[2] as f32],
                    uv: [(u * sa / 12.0) as f32, (v * sb / 12.0) as f32],
                });
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Mesh {
            vertices,
            indices,
            material: Material::WallExterior,
            object_id: self.wall_id,
        }
    }
}
