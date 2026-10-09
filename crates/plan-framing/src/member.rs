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
    /// Extra stud beside the end stud where two walls meet.
    CornerStud,
    /// Backing stud beside a partition that butts into the wall.
    TeeStud,
    TopPlate,
    BottomPlate,
    Header,
    Sill,
    RimJoist,
    Joist,
    /// Doubled joist beside a hole in the floor (a stairwell).
    TrimmerJoist,
    /// Doubled joist across the end of a hole in the floor.
    HeaderJoist,
    Blocking,
    Ledger,
    Rafter,
    Ridge,
    Hip,
    Valley,
    Fascia,
    CollarTie,
    CeilingJoist,
    TrussTopChord,
    TrussBottomChord,
    TrussWeb,
}

impl MemberKind {
    /// Lower-case name used in takeoff lines.
    pub fn name(&self) -> &'static str {
        match self {
            MemberKind::Stud => "stud",
            MemberKind::KingStud => "king stud",
            MemberKind::TrimmerStud => "trimmer",
            MemberKind::CrippleStud => "cripple",
            MemberKind::CornerStud => "corner stud",
            MemberKind::TeeStud => "tee backing",
            MemberKind::TopPlate => "top plate",
            MemberKind::BottomPlate => "bottom plate",
            MemberKind::Header => "header",
            MemberKind::Sill => "sill",
            MemberKind::RimJoist => "rim joist",
            MemberKind::Joist => "joist",
            MemberKind::TrimmerJoist => "trimmer joist",
            MemberKind::HeaderJoist => "header joist",
            MemberKind::Blocking => "blocking",
            MemberKind::Ledger => "ledger",
            MemberKind::Rafter => "rafter",
            MemberKind::Ridge => "ridge",
            MemberKind::Hip => "hip",
            MemberKind::Valley => "valley",
            MemberKind::Fascia => "fascia",
            MemberKind::CollarTie => "collar tie",
            MemberKind::CeilingJoist => "ceiling joist",
            MemberKind::TrussTopChord => "truss top chord",
            MemberKind::TrussBottomChord => "truss bottom chord",
            MemberKind::TrussWeb => "truss web",
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

/// How the lower end of a rafter is cut at the eave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TailCut {
    /// Vertical: the tail ends in a plumb cut (the fascia hangs plumb).
    Plumb,
    /// Horizontal: the tail ends in a level cut.
    Level,
    /// Square to the rafter.
    Square,
}

/// The notch a rafter takes where it bears on the top plate (side view, all
/// distances along the rafter's bottom edge from its lower end).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Birdsmouth {
    /// Where the plumb heel cut meets the bottom edge: the plate's outer
    /// face, measured along the rafter. Equals the overhang's slope length.
    pub heel_at: f64,
    /// Height of the plumb heel cut, inches (at most a third of the depth).
    pub heel_height: f64,
}

/// Cuts a member carries beyond its square-ended box: a rafter's tail cut
/// and birdsmouth. Members without cuts mesh as plain boxes.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct MemberCuts {
    /// Pitch of the member (rise per 12): the plumb and level lines of the cuts
    /// are drawn against it. Zero for a member without cuts.
    pub pitch_in_12: f64,
    pub tail: Option<TailCut>,
    pub birdsmouth: Option<Birdsmouth>,
}

impl MemberCuts {
    pub fn is_empty(&self) -> bool {
        self.tail.is_none() && self.birdsmouth.is_none()
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
    /// Tail cut and birdsmouth of a rafter (none for other members).
    #[serde(default, skip_serializing_if = "MemberCuts::is_empty")]
    pub cuts: MemberCuts,
    /// The cross-section shape the member is drawn with: the shape of its
    /// Framing Type (`catalog::stamp_members` sets it). A plain box when
    /// the type is lumber.
    #[serde(default, skip_serializing_if = "SectionShape::is_box")]
    pub shape: SectionShape,
    /// The Framing Type this member was given by Apply Framing Default
    /// Properties or by a build, by name; empty follows the default of its
    /// Role (`catalog::FramingCatalog::type_of_member`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub framing_type: String,
    /// The Role it was given by Apply Framing Default Properties; `None` is
    /// the Role of its kind (`Role::of_member`). Changes lists only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<crate::catalog::Role>,
}

/// The cross-section of a member (manual p. 887: the Framing Type's shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum SectionShape {
    /// A solid rectangle: lumber, glulam, LVL, PSL, VSL, rectangular
    /// concrete.
    #[default]
    Box,
    /// A wood I-joist: two flanges and a thin web.
    IJoist,
    /// A steel I-beam: thin flanges and web.
    SteelI,
    /// A hollow rectangular steel tube.
    SteelBox,
    /// A steel C channel (studs), open toward the thickness side.
    CChannel,
    /// A steel U channel (the plates of steel studs), open upward.
    UChannel,
    /// A circle or ellipse: round concrete.
    Round,
}

impl SectionShape {
    pub fn is_box(&self) -> bool {
        *self == SectionShape::Box
    }

    /// The shape's closed outline in the section plane, counter-clockwise in
    /// `(u, v)` with `u` across the thickness and `v` across the depth, for
    /// a section `thickness x depth`. `None` for a box and for the hollow
    /// tube (see [`SectionShape::tube`]).
    pub fn outline(&self, thickness: f64, depth: f64) -> Option<Vec<(f64, f64)>> {
        let (ht, hd) = (thickness / 2.0, depth / 2.0);
        let flange = |frac: f64, lo: f64, hi: f64| (depth * frac).clamp(lo, hi).min(hd * 0.8);
        match self {
            SectionShape::Box | SectionShape::SteelBox => None,
            SectionShape::IJoist | SectionShape::SteelI => {
                let (fh, w) = if *self == SectionShape::IJoist {
                    (flange(0.18, 1.0, 1.5), (ht * 0.18).max(0.1875))
                } else {
                    (flange(0.08, 0.25, 0.75), (ht * 0.06).max(0.125))
                };
                let w = w.min(ht * 0.9);
                Some(vec![
                    (-ht, -hd),
                    (ht, -hd),
                    (ht, -hd + fh),
                    (w, -hd + fh),
                    (w, hd - fh),
                    (ht, hd - fh),
                    (ht, hd),
                    (-ht, hd),
                    (-ht, hd - fh),
                    (-w, hd - fh),
                    (-w, -hd + fh),
                    (-ht, -hd + fh),
                ])
            }
            SectionShape::CChannel => {
                let t = (thickness.min(depth) * 0.12).clamp(0.06, 0.25);
                Some(vec![
                    (-ht, -hd),
                    (ht, -hd),
                    (ht, -hd + t),
                    (-ht + t, -hd + t),
                    (-ht + t, hd - t),
                    (ht, hd - t),
                    (ht, hd),
                    (-ht, hd),
                ])
            }
            SectionShape::UChannel => {
                let t = (thickness.min(depth) * 0.12).clamp(0.06, 0.25);
                Some(vec![
                    (-ht, -hd),
                    (ht, -hd),
                    (ht, hd),
                    (ht - t, hd),
                    (ht - t, -hd + t),
                    (-ht + t, -hd + t),
                    (-ht + t, hd),
                    (-ht, hd),
                ])
            }
            SectionShape::Round => Some(
                (0..16)
                    .map(|i| {
                        let a = std::f64::consts::TAU * f64::from(i) / 16.0;
                        (ht * a.cos(), hd * a.sin())
                    })
                    .collect(),
            ),
        }
    }

    /// The wall thickness of the hollow tube; `None` for the other shapes.
    pub fn tube(&self, thickness: f64, depth: f64) -> Option<f64> {
        (*self == SectionShape::SteelBox)
            .then(|| (thickness.min(depth) * 0.1).clamp(0.06, 0.25))
    }

    pub fn name(&self) -> &'static str {
        match self {
            SectionShape::Box => "Box",
            SectionShape::IJoist => "I-joist",
            SectionShape::SteelI => "Steel I",
            SectionShape::SteelBox => "Steel box",
            SectionShape::CChannel => "C channel",
            SectionShape::UChannel => "U channel",
            SectionShape::Round => "Round",
        }
    }
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
            cuts: MemberCuts::default(),
            shape: SectionShape::Box,
            framing_type: String::new(),
            role: None,
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
    /// A member with [`MemberCuts`] (a rafter with its tail cut and
    /// birdsmouth) meshes as its side profile extruded across the thickness.
    /// The mesh `object_id` is the member's wall, if any.
    pub fn mesh(&self) -> Mesh {
        if !self.cuts.is_empty() {
            return self.cut_mesh();
        }
        if !self.shape.is_box() {
            return self.section_mesh();
        }
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
            material: Material::Framing,
            object_id: self.wall_id,
            color: None,
        }
    }
}

// ----- members with cuts -----

/// Triangulate a simple polygon (counter-clockwise) by ear clipping.
fn ear_clip(poly: &[(f64, f64)]) -> Vec<[usize; 3]> {
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let inside = |p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
        cross(a, b, p) >= -1e-12 && cross(b, c, p) >= -1e-12 && cross(c, a, p) >= -1e-12
    };
    let mut idx: Vec<usize> = (0..poly.len()).collect();
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < 1000 {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for i in 0..n {
            let (a, b, c) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            if cross(poly[a], poly[b], poly[c]) <= 1e-12 {
                continue;
            }
            let ear = idx
                .iter()
                .filter(|&&k| k != a && k != b && k != c)
                .all(|&k| !inside(poly[k], poly[a], poly[b], poly[c]));
            if ear {
                out.push([a, b, c]);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

impl Member {
    /// The member's side profile in its own frame (`x` along the length from
    /// the start face, `y` across the depth, both centred on the start-face
    /// centre), counter-clockwise: the square-ended rectangle with the tail
    /// cut at the start and the birdsmouth notch on the bottom edge.
    pub fn profile(&self) -> Vec<(f64, f64)> {
        let (l, hd) = (self.length, self.lumber.depth / 2.0);
        let theta = (self.cuts.pitch_in_12 / 12.0).atan();
        let (sn, cs) = theta.sin_cos();
        let half = l / 2.0;
        let (mut bl, mut tl) = (0.0, 0.0);
        if sn > 1e-6 {
            match self.cuts.tail {
                // The vertical line leans toward the ridge going up.
                Some(TailCut::Plumb) => tl = (2.0 * hd * theta.tan()).min(half),
                // A level line: the top corner is the outermost point.
                Some(TailCut::Level) => bl = (2.0 * hd * cs / sn).min(half),
                Some(TailCut::Square) | None => {}
            }
        }
        let mut pts = vec![(bl, -hd)];
        if let (Some(b), true) = (self.cuts.birdsmouth, sn > 1e-6) {
            // D on the bottom edge, C above it (the heel), E on the bottom edge
            // again along the level seat.
            let (dx, hv) = (b.heel_at.max(bl), b.heel_height);
            let c = (dx + hv * sn, -hd + hv * cs);
            let e = (dx + hv / sn, -hd);
            if hv > 1e-6 && e.0 < l - 0.1 && c.1 < hd {
                pts.extend([(dx, -hd), c, e]);
            }
        }
        pts.extend([(l, -hd), (l, hd), (tl, hd)]);
        // Drop repeated points (a heel cut right at the tail's corner).
        pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
        pts
    }

    /// The member meshed as its [`SectionShape`] outline extruded along its
    /// length (an I-joist, a channel, a tube or a round member).
    fn section_mesh(&self) -> Mesh {
        let t = &self.transform;
        let az = t.axis_z();
        let (th, dp) = (self.lumber.thickness, self.lumber.depth);
        let at = |p: (f64, f64), along: f64| {
            add(
                add(t.origin, scale(t.axis_x, along)),
                add(scale(t.axis_y, p.1), scale(az, p.0)),
            )
        };
        let f32v = |v: Vec3| [v[0] as f32, v[1] as f32, v[2] as f32];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut push_tri = |a: Vec3, b: Vec3, c: Vec3, n: Vec3| {
            let e = cross(
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            );
            let (b, c) = if dot(e, n) < 0.0 { (c, b) } else { (b, c) };
            let base = vertices.len() as u32;
            for p in [a, b, c] {
                vertices.push(Vertex {
                    position: f32v(p),
                    normal: f32v(n),
                    uv: [(p[0] / 12.0) as f32, (p[2] / 12.0) as f32],
                });
            }
            indices.extend([base, base + 1, base + 2]);
        };
        let mut rings: Vec<Vec<(f64, f64)>> = Vec::new();
        if let Some(w) = self.shape.tube(th, dp) {
            let (ht, hd) = (th / 2.0, dp / 2.0);
            let outer = vec![(-ht, -hd), (ht, -hd), (ht, hd), (-ht, hd)];
            let inner = vec![
                (-ht + w, -hd + w),
                (ht - w, -hd + w),
                (ht - w, hd - w),
                (-ht + w, hd - w),
            ];
            // The end caps are the four trapezoids between the rings.
            for i in 0..4 {
                let j = (i + 1) % 4;
                for along in [0.0, self.length] {
                    let n = scale(t.axis_x, if along == 0.0 { -1.0 } else { 1.0 });
                    push_tri(at(outer[i], along), at(outer[j], along), at(inner[j], along), n);
                    push_tri(at(outer[i], along), at(inner[j], along), at(inner[i], along), n);
                }
            }
            rings.push(outer);
            // The inner ring faces inward.
            rings.push(inner.into_iter().rev().collect());
        } else if let Some(outline) = self.shape.outline(th, dp) {
            for tri in ear_clip(&outline) {
                let p = |k: usize| outline[tri[k]];
                push_tri(at(p(0), 0.0), at(p(1), 0.0), at(p(2), 0.0), scale(t.axis_x, -1.0));
                push_tri(
                    at(p(0), self.length),
                    at(p(1), self.length),
                    at(p(2), self.length),
                    t.axis_x,
                );
            }
            rings.push(outline);
        }
        for ring in &rings {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                let (du, dv) = (b.0 - a.0, b.1 - a.1);
                let len = (du * du + dv * dv).sqrt();
                if len < 1e-9 {
                    continue;
                }
                // Outward normal of a counter-clockwise edge: (dv, -du).
                let n = add(scale(az, dv / len), scale(t.axis_y, -du / len));
                push_tri(at(a, 0.0), at(b, 0.0), at(b, self.length), n);
                push_tri(at(a, 0.0), at(b, self.length), at(a, self.length), n);
            }
        }
        Mesh {
            vertices,
            indices,
            material: Material::Framing,
            object_id: self.wall_id,
            color: None,
        }
    }

    fn cut_mesh(&self) -> Mesh {
        let t = &self.transform;
        let az = t.axis_z();
        let ht = self.lumber.thickness / 2.0;
        let prof = self.profile();
        let at = |p: (f64, f64), z: f64| {
            add(
                add(t.origin, scale(t.axis_x, p.0)),
                add(scale(t.axis_y, p.1), scale(az, z)),
            )
        };
        let f32v = |v: Vec3| [v[0] as f32, v[1] as f32, v[2] as f32];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut push_tri = |a: Vec3, b: Vec3, c: Vec3, n: Vec3| {
            // Wind the triangle so its geometric normal agrees with `n`.
            let e = cross(
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            );
            let (b, c) = if dot(e, n) < 0.0 { (c, b) } else { (b, c) };
            let base = vertices.len() as u32;
            for p in [a, b, c] {
                vertices.push(Vertex {
                    position: f32v(p),
                    normal: f32v(n),
                    uv: [(p[0] / 12.0) as f32, (p[2] / 12.0) as f32],
                });
            }
            indices.extend([base, base + 1, base + 2]);
        };
        for tri in ear_clip(&prof) {
            let p = |k: usize| prof[tri[k]];
            push_tri(at(p(0), ht), at(p(1), ht), at(p(2), ht), az);
            push_tri(at(p(0), -ht), at(p(1), -ht), at(p(2), -ht), scale(az, -1.0));
        }
        for i in 0..prof.len() {
            let (a, b) = (prof[i], prof[(i + 1) % prof.len()]);
            let (ex, ey) = (b.0 - a.0, b.1 - a.1);
            let len = (ex * ex + ey * ey).sqrt();
            if len < 1e-9 {
                continue;
            }
            // Outward normal of a counter-clockwise edge: (ey, -ex).
            let n = add(scale(t.axis_x, ey / len), scale(t.axis_y, -ex / len));
            push_tri(at(a, -ht), at(b, -ht), at(b, ht), n);
            push_tri(at(a, -ht), at(b, ht), at(a, ht), n);
        }
        Mesh {
            vertices,
            indices,
            material: Material::Framing,
            object_id: self.wall_id,
            color: None,
        }
    }
}
