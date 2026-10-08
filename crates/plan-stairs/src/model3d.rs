//! 3D meshes for a stair: treads, risers, stringers, landings and handrails.
//!
//! `plan-3d` keeps its mesh builder private, so this module triangulates its
//! own convex prisms directly into [`plan_3d::Mesh`] values. Scene space is
//! X right, Y up, Z = -plan y.

use crate::layout::{Flight, Frame, Layout, Uv};
use crate::Stair;
use plan_3d::{Material, Mesh, Vertex};
use plan_core::Id;

pub(crate) type V3 = [f64; 3];

/// Stringer board thickness.
const STRINGER_THICKNESS: f64 = 1.5;
/// Solid thickness of landings, winder treads and ramp slabs.
const SLAB_THICKNESS: f64 = 3.5;
/// Handrail cross-section (square).
const HANDRAIL_SIZE: f64 = 2.0;
/// Handrail height above the nosing line.
const HANDRAIL_HEIGHT: f64 = 34.0;

/// What a mesh of a stair is, since [`plan_3d::Mesh`] carries only a material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StairPart {
    /// A tread box.
    Tread,
    /// A riser board.
    Riser,
    /// A sloped stringer.
    Stringer,
    /// A landing or winder slab.
    Landing,
    /// The sloped slab of a ramp.
    Ramp,
    /// A handrail.
    Handrail,
}

/// All meshes of the stair, each box or slab as its own [`Mesh`].
///
/// Treads and landings use `Material::Floor`; risers, stringers, ramp and
/// handrails use `Material::WallInterior` (plan-3d has no wood material yet).
pub fn meshes(stair: &Stair) -> Vec<Mesh> {
    tagged_meshes(stair).into_iter().map(|(_, m)| m).collect()
}

/// Like [`meshes`], but each mesh is labelled with the [`StairPart`] it is.
pub fn tagged_meshes(stair: &Stair) -> Vec<(StairPart, Mesh)> {
    let layout = Layout::build(stair);
    let mut out = Vec::new();
    let mut ctx = Ctx {
        frame: layout.frame,
        elevation: stair.floor_elevation,
        id: stair.id,
        out: &mut out,
    };
    let p = &stair.params;
    let h = layout.riser_height;

    for f in &layout.flights {
        if layout.is_ramp {
            ctx.ramp(f, layout.total_rise);
            continue;
        }
        for j in 1..=f.treads {
            let s1 = f64::from(j) * layout.tread_depth;
            let s0 = s1 - layout.tread_depth - p.nosing;
            let top = f.base + f64::from(j) * h;
            let profile = ctx.flight_rect(f, s0, s1, top - p.tread_thickness);
            ctx.push(
                StairPart::Tread,
                Material::Floor,
                &profile,
                [0.0, p.tread_thickness, 0.0],
            );
        }
        if !p.open_risers {
            for j in 1..=f.risers {
                let s0 = f64::from(j - 1) * layout.tread_depth;
                let profile =
                    ctx.flight_rect(f, s0, s0 + p.riser_thickness, f.base + f64::from(j - 1) * h);
                ctx.push(
                    StairPart::Riser,
                    Material::WallInterior,
                    &profile,
                    [0.0, h, 0.0],
                );
            }
        }
        if f.treads > 0 {
            ctx.stringers(f, h, p.stringer_depth);
            if p.handrail {
                ctx.handrails(f, h);
            }
        }
    }

    for slab in &layout.slabs {
        let profile: Vec<V3> = slab
            .poly
            .iter()
            .map(|&uv| ctx.scene(uv, slab.top - SLAB_THICKNESS))
            .collect();
        ctx.push(
            StairPart::Landing,
            Material::Floor,
            &profile,
            [0.0, SLAB_THICKNESS, 0.0],
        );
    }
    if !p.open_risers {
        for r in &layout.turn_risers {
            let b = (r.b.0 + r.thickness.0, r.b.1 + r.thickness.1);
            let a = (r.a.0 + r.thickness.0, r.a.1 + r.thickness.1);
            let profile: Vec<V3> = [r.a, r.b, b, a]
                .iter()
                .map(|&uv| ctx.scene(uv, r.base))
                .collect();
            ctx.push(
                StairPart::Riser,
                Material::WallInterior,
                &profile,
                [0.0, h, 0.0],
            );
        }
    }
    out
}

/// Mesh-building context for one stair.
struct Ctx<'a> {
    frame: Frame,
    elevation: f64,
    id: Id,
    out: &'a mut Vec<(StairPart, Mesh)>,
}

impl Ctx<'_> {
    /// Scene position of a local point at height `h` above the floor.
    fn scene(&self, uv: Uv, h: f64) -> V3 {
        let p = self.frame.uv(uv);
        [p.x, self.elevation + h, -p.y]
    }

    /// Scene vector of a local displacement plus a vertical component.
    fn vector(&self, d: Uv, dh: f64) -> V3 {
        let v = self.frame.vector(d);
        [v.x, dh, -v.y]
    }

    /// Scene position `s` along `f`, `lat` from its left edge, at height `h`.
    fn on_flight(&self, f: &Flight, s: f64, lat: f64, h: f64) -> V3 {
        self.scene(f.at(s, lat), h)
    }

    /// Horizontal rectangle spanning `s0..s1` along and the full width of `f`.
    fn flight_rect(&self, f: &Flight, s0: f64, s1: f64, h: f64) -> Vec<V3> {
        [(s0, 0.0), (s1, 0.0), (s1, f.width), (s0, f.width)]
            .iter()
            .map(|&(s, lat)| self.on_flight(f, s, lat, h))
            .collect()
    }

    fn push(&mut self, part: StairPart, material: Material, profile: &[V3], ext: V3) {
        self.out
            .push((part, solid(profile, ext, material, Some(self.id))));
    }

    /// A vertical-plane profile (along, height) at lateral `lat`, extruded sideways by `thick`.
    fn side_board(
        &mut self,
        part: StairPart,
        material: Material,
        f: &Flight,
        pts: [(f64, f64); 4],
        lat: f64,
        thick: f64,
    ) {
        let profile: Vec<V3> = pts
            .iter()
            .map(|&(s, h)| self.on_flight(f, s, lat, h))
            .collect();
        let r = f.right();
        let ext = self.vector((r.0 * thick, r.1 * thick), 0.0);
        self.push(part, material, &profile, ext);
    }

    /// Two stringers along the pitch line through the riser tops.
    fn stringers(&mut self, f: &Flight, h: f64, depth: f64) {
        let rise = f64::from(f.treads) * h;
        let hyp = f.len.hypot(rise);
        let (cos, sin) = (f.len / hyp, rise / hyp);
        let (dx, dy) = (sin * depth, -cos * depth);
        let (a, b) = ((0.0, f.base + h), (f.len, f.base + h + rise));
        let pts = [a, b, (b.0 + dx, b.1 + dy), (a.0 + dx, a.1 + dy)];
        for lat in [0.0, (f.width - STRINGER_THICKNESS).max(0.0)] {
            self.side_board(
                StairPart::Stringer,
                Material::WallInterior,
                f,
                pts,
                lat,
                STRINGER_THICKNESS,
            );
        }
    }

    /// A handrail on each side, parallel to the pitch line.
    fn handrails(&mut self, f: &Flight, h: f64) {
        let rise = f64::from(f.treads) * h;
        let (y0, y1) = (
            f.base + h + HANDRAIL_HEIGHT,
            f.base + h + rise + HANDRAIL_HEIGHT,
        );
        let pts = [
            (0.0, y0),
            (f.len, y1),
            (f.len, y1 - HANDRAIL_SIZE),
            (0.0, y0 - HANDRAIL_SIZE),
        ];
        for lat in [0.0, (f.width - HANDRAIL_SIZE).max(0.0)] {
            self.side_board(
                StairPart::Handrail,
                Material::WallInterior,
                f,
                pts,
                lat,
                HANDRAIL_SIZE,
            );
        }
    }

    /// One sloped slab rising `rise` over the flight length.
    fn ramp(&mut self, f: &Flight, rise: f64) {
        let profile: Vec<V3> = [
            (0.0, 0.0),
            (f.len, rise),
            (f.len, rise - SLAB_THICKNESS),
            (0.0, -SLAB_THICKNESS),
        ]
        .iter()
        .map(|&(s, h)| self.on_flight(f, s, 0.0, h))
        .collect();
        let r = f.right();
        let ext = self.vector((r.0 * f.width, r.1 * f.width), 0.0);
        self.push(StairPart::Ramp, Material::WallInterior, &profile, ext);
    }
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn average(pts: &[V3]) -> V3 {
    let n = pts.len() as f64;
    let sum = pts.iter().fold([0.0; 3], |acc, &p| add(acc, p));
    [sum[0] / n, sum[1] / n, sum[2] / n]
}

/// Newell's method: a normal for a (possibly slightly non-planar) polygon.
fn newell(pts: &[V3]) -> V3 {
    let mut n = [0.0; 3];
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

/// A convex prism: `profile` swept along `ext`, with flat-shaded faces and
/// outward normals (decided against the prism centroid, so any winding works).
pub(crate) fn solid(profile: &[V3], ext: V3, material: Material, id: Option<Id>) -> Mesh {
    let moved: Vec<V3> = profile.iter().map(|&p| add(p, ext)).collect();
    let mut all = profile.to_vec();
    all.extend_from_slice(&moved);
    let center = average(&all);

    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        material,
        object_id: id,
    };
    add_face(&mut mesh, profile, center);
    add_face(&mut mesh, &moved, center);
    for i in 0..profile.len() {
        let j = (i + 1) % profile.len();
        add_face(
            &mut mesh,
            &[profile[i], profile[j], moved[j], moved[i]],
            center,
        );
    }
    mesh
}

/// Append a convex polygon as a triangle fan facing away from `center`.
fn add_face(mesh: &mut Mesh, pts: &[V3], center: V3) {
    let mut n = newell(pts);
    let len = dot(n, n).sqrt();
    if len < 1e-12 {
        return;
    }
    n = [n[0] / len, n[1] / len, n[2] / len];
    let mut pts = pts.to_vec();
    if dot(n, sub(average(&pts), center)) < 0.0 {
        n = [-n[0], -n[1], -n[2]];
        pts.reverse();
    }
    let base = mesh.vertices.len() as u32;
    for p in &pts {
        // UVs in feet, projected along the dominant axis of the normal.
        let uv = if n[1].abs() >= n[0].abs() && n[1].abs() >= n[2].abs() {
            [p[0] / 12.0, p[2] / 12.0]
        } else if n[0].abs() >= n[2].abs() {
            [p[2] / 12.0, p[1] / 12.0]
        } else {
            [p[0] / 12.0, p[1] / 12.0]
        };
        mesh.vertices.push(Vertex {
            position: [p[0] as f32, p[1] as f32, p[2] as f32],
            normal: [n[0] as f32, n[1] as f32, n[2] as f32],
            uv: [uv[0] as f32, uv[1] as f32],
        });
    }
    for i in 1..pts.len() as u32 - 1 {
        mesh.indices.extend([base, base + i, base + i + 1]);
    }
}
