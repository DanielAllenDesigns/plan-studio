//! 3D meshes for a stair: treads, risers, stringers, landings and handrails.
//!
//! `plan-3d` keeps its mesh builder private, so this module triangulates its
//! own convex prisms directly into [`plan_3d::Mesh`] values. Scene space is
//! X right, Y up, Z = -plan y.

use crate::landing::polygon_slab;
use crate::layout::{Curve, Flight, Frame, Layout, Uv};
use crate::railing::{stair_half_wall, stair_railing};
use crate::{RailSide, SideKind, Stair, StairParams, StringerStyle};
use plan_3d::{Material, Mesh, Vertex};
use plan_core::{Id, Point};

pub(crate) type V3 = [f64; 3];

/// Stringer board thickness.
const STRINGER_THICKNESS: f64 = 1.5;
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
    /// A sloped stringer, or a wall / half-wall along a side of the stair.
    Stringer,
    /// A landing or winder slab.
    Landing,
    /// The sloped slab of a ramp.
    Ramp,
    /// A handrail, or the railing of a stair side (newels, balusters, rails).
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
        elevation: stair.bottom_elevation(),
        id: stair.id,
        out: &mut out,
    };
    let p = &stair.params;
    let h = layout.riser_height;
    let thick = p.slab_thickness.max(0.1);

    if let Some(c) = &layout.curve {
        ctx.curved(c, p, h);
    } else if !layout.is_landing {
        for f in &layout.flights {
            if layout.is_ramp {
                ctx.ramp(f, thick);
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
                    let profile = ctx.flight_rect(
                        f,
                        s0,
                        s0 + p.riser_thickness,
                        f.base + f64::from(j - 1) * h,
                    );
                    ctx.push(
                        StairPart::Riser,
                        Material::WallInterior,
                        &profile,
                        [0.0, h, 0.0],
                    );
                }
            }
            if f.treads > 0 {
                ctx.stringers(f, h, p);
                if p.handrail {
                    ctx.handrails(f, h);
                }
            }
        }
    }

    for slab in &layout.slabs {
        let poly: Vec<Point> = slab.poly.iter().map(|&uv| ctx.frame.uv(uv)).collect();
        if let Some(m) = polygon_slab(
            &poly,
            ctx.elevation + slab.top - thick,
            ctx.elevation + slab.top,
            Material::Floor,
            Some(ctx.id),
        ) {
            ctx.out.push((StairPart::Landing, m));
        }
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

    if !layout.is_landing {
        for (side, kind) in [
            (RailSide::Left, p.left_side),
            (RailSide::Right, p.right_side),
        ] {
            match kind {
                SideKind::None => {}
                SideKind::Railing => out.extend(
                    stair_railing(stair, side, &p.railing)
                        .into_iter()
                        .map(|m| (StairPart::Handrail, m)),
                ),
                SideKind::Wall | SideKind::HalfWall => out.extend(
                    stair_half_wall(stair, side, &p.railing, kind == SideKind::Wall)
                        .into_iter()
                        .map(|m| (StairPart::Stringer, m)),
                ),
            }
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
        pts: &[(f64, f64)],
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

    /// Stringers along the pitch line through the riser tops, one on each side.
    fn stringers(&mut self, f: &Flight, h: f64, p: &StairParams) {
        let depth = p.stringer_depth;
        let rise = f64::from(f.treads) * h;
        let hyp = f.len.hypot(rise);
        let (cos, sin) = (f.len / hyp, rise / hyp);
        let lats = [0.0, (f.width - STRINGER_THICKNESS).max(0.0)];
        match p.stringer {
            StringerStyle::None => {}
            StringerStyle::Closed => {
                let (dx, dy) = (sin * depth, -cos * depth);
                let (a, b) = ((0.0, f.base + h), (f.len, f.base + h + rise));
                let pts = [a, b, (b.0 + dx, b.1 + dy), (a.0 + dx, a.1 + dy)];
                for lat in lats {
                    self.side_board(
                        StairPart::Stringer,
                        Material::WallInterior,
                        f,
                        &pts,
                        lat,
                        STRINGER_THICKNESS,
                    );
                }
            }
            StringerStyle::Open => {
                // The board under the notches: between the line through the
                // inside corners of the steps and the bottom edge, at least
                // 4" of throat; then one triangle per step above it.
                let t = f.len / f64::from(f.treads);
                let tip = |s: f64| f.base + h + s * h / t;
                let throat = (depth / cos - h).max(4.0 / cos);
                let root = |s: f64| tip(s) - h;
                let strip = [
                    (0.0, root(0.0)),
                    (f.len, root(f.len)),
                    (f.len, root(f.len) - throat),
                    (0.0, root(0.0) - throat),
                ];
                for lat in lats {
                    self.side_board(
                        StairPart::Stringer,
                        Material::WallInterior,
                        f,
                        &strip,
                        lat,
                        STRINGER_THICKNESS,
                    );
                    for j in 1..=f.treads {
                        let (s0, s1) = (f64::from(j - 1) * t, f64::from(j) * t);
                        let tri = [(s0, root(s0)), (s0, tip(s0)), (s1, root(s1))];
                        self.side_board(
                            StairPart::Stringer,
                            Material::WallInterior,
                            f,
                            &tri,
                            lat,
                            STRINGER_THICKNESS,
                        );
                    }
                }
            }
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
                &pts,
                lat,
                HANDRAIL_SIZE,
            );
        }
    }

    /// One sloped slab rising over the flight length, on top of `f.base`.
    fn ramp(&mut self, f: &Flight, thick: f64) {
        let profile: Vec<V3> = [
            (0.0, f.base),
            (f.len, f.base + f.rise),
            (f.len, f.base + f.rise - thick),
            (0.0, f.base - thick),
        ]
        .iter()
        .map(|&(s, h)| self.on_flight(f, s, 0.0, h))
        .collect();
        let r = f.right();
        let ext = self.vector((r.0 * f.width, r.1 * f.width), 0.0);
        self.push(StairPart::Ramp, Material::WallInterior, &profile, ext);
    }

    /// A horizontal wedge of a curved stair at height `y`.
    fn wedge(&self, c: &Curve, a0: f64, a1: f64, y: f64) -> Vec<V3> {
        let mut pts = vec![
            c.at(a0, c.inner),
            c.at(a0, c.outer()),
            c.at(a1, c.outer()),
            c.at(a1, c.inner),
        ];
        // At a zero inside radius the two inner corners coincide.
        pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
        if pts.len() > 3 {
            let (f, l) = (pts[0], pts[pts.len() - 1]);
            if (f.0 - l.0).abs() < 1e-9 && (f.1 - l.1).abs() < 1e-9 {
                pts.pop();
            }
        }
        pts.iter().map(|&uv| self.scene(uv, y)).collect()
    }

    /// Treads, risers and stringers of a curved stair.
    fn curved(&mut self, c: &Curve, p: &StairParams, h: f64) {
        let nose = p.nosing / c.walk().max(1e-9);
        for j in 1..=c.treads {
            let a1 = c.step * f64::from(j);
            let a0 = a1 - c.step - nose;
            let top = f64::from(j) * h;
            let profile = self.wedge(c, a0, a1, top - p.tread_thickness);
            self.push(
                StairPart::Tread,
                Material::Floor,
                &profile,
                [0.0, p.tread_thickness, 0.0],
            );
        }
        if !p.open_risers {
            for j in 1..=c.risers {
                let a0 = c.step * f64::from(j - 1);
                let a1 = a0 + p.riser_thickness / c.walk().max(1e-9);
                let profile = self.wedge(c, a0, a1, f64::from(j - 1) * h);
                self.push(
                    StairPart::Riser,
                    Material::WallInterior,
                    &profile,
                    [0.0, h, 0.0],
                );
            }
        }
        if p.stringer != StringerStyle::None {
            // The stringers of a curve are stepped skirts, one board per step
            // along the chord, hugging the pitch line (open and closed alike).
            let depth = p.stringer_depth * 1.25;
            for j in 1..=c.treads {
                let (a0, a1) = (c.step * f64::from(j - 1), c.step * f64::from(j));
                let (top0, top1) = (f64::from(j) * h, f64::from(j + 1) * h);
                for lat in [0.0, c.width] {
                    let rho = c.rho(lat);
                    let (p0, p1) = (c.at(a0, rho), c.at(a1, rho));
                    let profile = [
                        self.scene(p0, top0),
                        self.scene(p1, top1),
                        self.scene(p1, top1 - depth),
                        self.scene(p0, top0 - depth),
                    ];
                    let mid = (a0 + a1) / 2.0;
                    let radial = c.at(mid, 1.0);
                    let radial = (radial.0 - c.center.0, radial.1 - c.center.1);
                    let inward = if rho < c.walk() { 1.0 } else { -1.0 };
                    let k = inward * STRINGER_THICKNESS;
                    let ext = self.vector((radial.0 * k, radial.1 * k), 0.0);
                    self.push(StairPart::Stringer, Material::WallInterior, &profile, ext);
                }
            }
        }
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
