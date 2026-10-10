//! 3D meshes for a stair: treads, risers, stringers, landings and handrails.
//!
//! `plan-3d` keeps its mesh builder private, so this module triangulates its
//! own convex prisms directly into [`plan_3d::Mesh`] values. Scene space is
//! X right, Y up, Z = -plan y.

use crate::landing::polygon_slab;
use crate::layout::{Curve, Flight, Frame, Layout, RampArc, Uv};
use crate::railing::{
    bar, landing_railing, stair_half_wall_skipping, stair_railing_skipping, PostSkip,
};
use crate::{RailSide, SideKind, Stair, StairParams, StringerStyle};
use plan_3d::{Material, Mesh, Vertex};
use plan_core::{Id, Point};

pub(crate) type V3 = [f64; 3];

/// Stringer board thickness (curved stairs).
const STRINGER_THICKNESS: f64 = 1.5;
/// Handrail cross-section (square).
const HANDRAIL_SIZE: f64 = 2.0;
/// Handrail height above the nosing line.
const HANDRAIL_HEIGHT: f64 = 34.0;
/// Thickness of a carpet runner.
const RUNNER_THICKNESS: f64 = 0.5;
/// Thickness of the soffit under a stair that is closed underneath.
const SOFFIT_THICKNESS: f64 = 0.75;
/// How far a handrail returns into the wall.
const RETURN_LENGTH: f64 = 3.0;

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
    /// The carpet runner down the treads.
    Runner,
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
    tagged_meshes_skipping(stair, PostSkip::default())
}

/// [`tagged_meshes`] without the newels or balusters `skip` names: the
/// caller puts a library post at each place [`crate::stair_posts`] lists.
pub fn tagged_meshes_skipping(stair: &Stair, skip: PostSkip) -> Vec<(StairPart, Mesh)> {
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
    } else if let Some(arc) = &layout.ramp_arc {
        ctx.curved_ramp(arc, thick);
        if p.handrail {
            ctx.arc_handrails(arc);
        }
    } else if !layout.is_landing {
        let aprons = layout.aprons(p);
        let flared = !p.flare_shape.is_none();
        let nflights = layout.flights.len();
        for (i, f) in layout.flights.iter().enumerate() {
            if layout.is_ramp {
                ctx.ramp(f, thick);
                if p.handrail {
                    ctx.ramp_handrails(f, p, i == 0, i + 1 == nflights);
                }
                continue;
            }
            let t = layout.tread_depth;
            for j in 1..=f.treads {
                let s1 = f64::from(j) * t;
                let s0 = s1 - t - p.nosing;
                let top = f.base + f64::from(j) * h;
                let lo = top - p.tread_thickness;
                let apron = aprons.iter().find(|(n, _)| i == 0 && *n == j);
                if let Some((_, apron)) = apron {
                    let profile: Vec<V3> = apron.iter().map(|&uv| ctx.scene(uv, lo)).collect();
                    ctx.push(
                        StairPart::Tread,
                        Material::Floor,
                        &profile,
                        [0.0, p.tread_thickness, 0.0],
                    );
                } else if flared {
                    let mut ring = layout.across(p, i, s0, layout.bulge(p, i, j - 1));
                    ring.extend(
                        layout
                            .across(p, i, s1, layout.bulge(p, i, j))
                            .into_iter()
                            .rev(),
                    );
                    ctx.slab(StairPart::Tread, Material::Floor, &ring, (lo, top));
                } else {
                    let profile = ctx.flight_rect(f, s0, s1, lo);
                    ctx.push(
                        StairPart::Tread,
                        Material::Floor,
                        &profile,
                        [0.0, p.tread_thickness, 0.0],
                    );
                }
            }
            if !p.open_risers {
                for j in 1..=f.risers {
                    if j == f.risers && i + 1 == nflights && !p.top_landing.riser_surface {
                        continue;
                    }
                    let s0 = f64::from(j - 1) * t;
                    let y0 = f.base + f64::from(j - 1) * h;
                    if flared {
                        let mut ring = layout.across(p, i, s0, layout.bulge(p, i, j - 1));
                        ring.extend(
                            layout
                                .across(p, i, s0 + p.riser_thickness, layout.bulge(p, i, j - 1))
                                .into_iter()
                                .rev(),
                        );
                        ctx.slab(
                            StairPart::Riser,
                            Material::WallInterior,
                            &ring,
                            (y0, y0 + h),
                        );
                    } else {
                        let profile = ctx.flight_rect(f, s0, s0 + p.riser_thickness, y0);
                        ctx.push(
                            StairPart::Riser,
                            Material::WallInterior,
                            &profile,
                            [0.0, h, 0.0],
                        );
                    }
                }
            }
            if f.treads > 0 {
                ctx.stringers(f, i, h, p);
                let (left, right) = (
                    p.handrail || p.left_side == SideKind::Handrail,
                    p.handrail || p.right_side == SideKind::Handrail,
                );
                if left || right {
                    ctx.handrails(f, h, left, right, p, i == 0, i + 1 == nflights);
                }
                if p.runner.width > 1e-9 {
                    ctx.runner(f, h, p, t);
                }
                if p.top_landing.nosing && i + 1 == nflights {
                    ctx.top_nosing(f, h, p);
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

    if layout.is_landing {
        out.extend(
            landing_railing(stair, skip)
                .into_iter()
                .map(|m| (StairPart::Handrail, m)),
        );
    } else {
        for (side, kind) in [
            (RailSide::Left, p.left_side),
            (RailSide::Right, p.right_side),
        ] {
            let railing = p.railing_for(side);
            match kind {
                // A Handrail side is drawn with the flights' handrails.
                SideKind::None | SideKind::Handrail => {}
                SideKind::Railing => out.extend(
                    stair_railing_skipping(stair, side, &railing, skip)
                        .into_iter()
                        .map(|m| (StairPart::Handrail, m)),
                ),
                SideKind::Wall | SideKind::HalfWall => out.extend(
                    stair_half_wall_skipping(stair, side, &railing, kind == SideKind::Wall, skip)
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

    /// A polygon given in the local frame, extruded between two heights.
    fn slab(&mut self, part: StairPart, material: Material, ring: &[Uv], (y0, y1): (f64, f64)) {
        let poly: Vec<Point> = ring.iter().map(|&uv| self.frame.uv(uv)).collect();
        if let Some(m) = polygon_slab(
            &poly,
            self.elevation + y0,
            self.elevation + y1,
            material,
            Some(self.id),
        ) {
            self.out.push((part, m));
        }
    }

    /// A board across the whole flight (or `width` of it from `lat`): the
    /// profile is `(along, height)` pairs, extruded sideways.
    fn across_board(
        &mut self,
        part: StairPart,
        f: &Flight,
        pts: &[(f64, f64)],
        lat: f64,
        width: f64,
    ) {
        let profile: Vec<V3> = pts
            .iter()
            .map(|&(s, h)| self.on_flight(f, s, lat, h))
            .collect();
        let r = f.right();
        let ext = self.vector((r.0 * width, r.1 * width), 0.0);
        self.push(part, Material::WallInterior, &profile, ext);
    }

    /// Stringers along the pitch line through the riser tops: one on each
    /// side by default, plus the centre ones and the skirt and soffit of a
    /// stair that is closed underneath (the Stringers panel).
    fn stringers(&mut self, f: &Flight, index: usize, h: f64, p: &StairParams) {
        let o = &p.stringers;
        let thick = o.thickness.max(0.25);
        let style = if p.stringer == StringerStyle::None && o.centre > 0 {
            StringerStyle::Closed
        } else {
            p.stringer
        };
        let t = f.len / f64::from(f.treads.max(1));
        let slope = h / t;
        let depth = p.stringer_depth;
        let rise = f64::from(f.treads) * h;
        let hyp = f.len.hypot(rise);
        let (cos, sin) = (f.len / hyp, rise / hyp);
        // Where the boards stand: the sides unless left out, then the
        // middle ones.
        let mut lats: Vec<f64> = Vec::new();
        if p.stringer != StringerStyle::None && !o.no_sides {
            lats.extend([0.0, (f.width - thick).max(0.0)]);
        }
        for k in 1..=u32::from(o.centre) {
            lats.push(f.width * f64::from(k) / f64::from(u32::from(o.centre) + 1) - thick / 2.0);
        }
        let s_end = if o.extend_top {
            f.len
        } else {
            (f.len - t).max(t.min(f.len))
        };
        match style {
            StringerStyle::None => {}
            StringerStyle::Closed => {
                let (dx, dy) = (sin * depth, -cos * depth);
                let (a, b) = ((0.0, f.base + h), (s_end, f.base + h + s_end * slope));
                let pts = [a, b, (b.0 + dx, b.1 + dy), (a.0 + dx, a.1 + dy)];
                for &lat in &lats {
                    self.side_board(
                        StairPart::Stringer,
                        Material::WallInterior,
                        f,
                        &pts,
                        lat,
                        thick,
                    );
                }
            }
            StringerStyle::Open => {
                // The board under the notches: between the line through the
                // inside corners of the steps and the bottom edge, at least
                // 4" of throat; then one triangle per step above it.
                let tip = |s: f64| f.base + h + s * slope;
                let throat = (depth / cos - h).max(4.0 / cos);
                let root = |s: f64| tip(s) - h;
                let strip = [
                    (0.0, root(0.0)),
                    (s_end, root(s_end)),
                    (s_end, root(s_end) - throat),
                    (0.0, root(0.0) - throat),
                ];
                let steps = if o.extend_top {
                    f.treads
                } else {
                    f.treads.saturating_sub(1)
                };
                for &lat in &lats {
                    self.side_board(
                        StairPart::Stringer,
                        Material::WallInterior,
                        f,
                        &strip,
                        lat,
                        thick,
                    );
                    for j in 1..=steps {
                        let (s0, s1) = (f64::from(j - 1) * t, f64::from(j) * t);
                        let tri = [(s0, root(s0)), (s0, tip(s0)), (s1, root(s1))];
                        self.side_board(
                            StairPart::Stringer,
                            Material::WallInterior,
                            f,
                            &tri,
                            lat,
                            thick,
                        );
                    }
                }
            }
        }
        // A larger stringer at the base: a block down to the floor under the
        // first steps of the first flight.
        if o.large_base && index == 0 && style != StringerStyle::None {
            let heel = [
                (0.0, f.base),
                (t, f.base),
                (t, f.base + h + t * slope),
                (0.0, f.base + h),
            ];
            for &lat in &lats {
                self.side_board(
                    StairPart::Stringer,
                    Material::WallInterior,
                    f,
                    &heel,
                    lat,
                    thick,
                );
            }
        }
        // Closed underneath: a skirt down to the floor along each side and a
        // soffit across the underside.
        if !o.open_underneath {
            let top_a = f.base + h;
            let top_b = f.base + h + rise;
            let skirt = [(0.0, 0.0), (f.len, 0.0), (f.len, top_b), (0.0, top_a)];
            let inset = o.side_inset.max(0.0).min(f.width / 2.0 - thick);
            for lat in [inset, (f.width - inset - thick).max(0.0)] {
                self.side_board(
                    StairPart::Stringer,
                    Material::WallInterior,
                    f,
                    &skirt,
                    lat,
                    thick,
                );
            }
            let (dx, dy) = (sin * depth, -cos * depth);
            let soffit = [
                (dx, top_a + dy),
                (f.len + dx, top_b + dy),
                (f.len + dx, top_b + dy - SOFFIT_THICKNESS),
                (dx, top_a + dy - SOFFIT_THICKNESS),
            ];
            self.across_board(
                StairPart::Stringer,
                f,
                &soffit,
                inset,
                (f.width - 2.0 * inset).max(0.0),
            );
        }
    }

    /// A handrail on the chosen sides, parallel to the pitch line, with the
    /// Railing panel's extensions and returns.
    #[allow(clippy::too_many_arguments)]
    fn handrails(
        &mut self,
        f: &Flight,
        h: f64,
        left: bool,
        right: bool,
        p: &StairParams,
        first: bool,
        last: bool,
    ) {
        let rise = f64::from(f.treads) * h;
        let slope = rise / f.len.max(1e-9);
        let o = &p.handrail_options;
        let ext_b = if first { o.extend_bottom.max(0.0) } else { 0.0 };
        let ext_t = if last { o.extend_top.max(0.0) } else { 0.0 };
        let y_at = |s: f64| f.base + h + HANDRAIL_HEIGHT + slope * s;
        let (s0, s1) = (-ext_b, f.len + ext_t);
        let pts = [
            (s0, y_at(s0)),
            (s1, y_at(s1)),
            (s1, y_at(s1) - HANDRAIL_SIZE),
            (s0, y_at(s0) - HANDRAIL_SIZE),
        ];
        for (is_left, lat) in [
            left.then_some((true, 0.0)),
            right.then_some((false, (f.width - HANDRAIL_SIZE).max(0.0))),
        ]
        .into_iter()
        .flatten()
        {
            self.side_board(
                StairPart::Handrail,
                Material::WallInterior,
                f,
                &pts,
                lat,
                HANDRAIL_SIZE,
            );
            // A return: a short bar from the end of the rail into the wall.
            let into_wall = if is_left { -1.0 } else { 1.0 };
            for (ret, s) in [(o.return_bottom && first, s0), (o.return_top && last, s1)] {
                if !ret {
                    continue;
                }
                let y = y_at(s) - HANDRAIL_SIZE / 2.0;
                let edge = if is_left { 0.0 } else { f.width };
                let a = self.on_flight(f, s, edge - into_wall * HANDRAIL_SIZE / 2.0, y);
                let b = self.on_flight(f, s, edge + into_wall * RETURN_LENGTH, y);
                let hint = self.vector((f.dir.0, f.dir.1), 0.0);
                self.out.extend(
                    bar(
                        a,
                        b,
                        hint,
                        (HANDRAIL_SIZE, HANDRAIL_SIZE),
                        Material::WallInterior,
                        Some(self.id),
                    )
                    .map(|m| (StairPart::Handrail, m)),
                );
            }
        }
    }

    /// Handrails on both sides of a ramp run, 34" above the surface, flat
    /// over the landing that follows; the first run starts, and the last run
    /// ends, with the Railing panel's extension.
    fn ramp_handrails(&mut self, f: &Flight, p: &StairParams, first: bool, last: bool) {
        let slope = f.rise / f.len.max(1e-9);
        let o = &p.handrail_options;
        let ext_b = if first { o.extend_bottom.max(0.0) } else { 0.0 };
        let ext_t = if last {
            o.extend_top.max(0.0)
        } else {
            crate::RAMP_LANDING
        };
        let y_at = |s: f64| f.base + HANDRAIL_HEIGHT + (slope * s).min(f.rise);
        let pts = [
            (-ext_b, y_at(-ext_b.min(0.0))),
            (f.len, y_at(f.len)),
            (f.len + ext_t, y_at(f.len)),
            (f.len + ext_t, y_at(f.len) - HANDRAIL_SIZE),
            (f.len, y_at(f.len) - HANDRAIL_SIZE),
            (-ext_b, y_at(0.0) - HANDRAIL_SIZE),
        ];
        // A run's rail is not convex once it flattens over the landing:
        // cut it into the sloped part and the level part.
        let sloped = [pts[0], pts[1], pts[4], pts[5]];
        let level = [pts[1], pts[2], pts[3], pts[4]];
        for lat in [0.0, (f.width - HANDRAIL_SIZE).max(0.0)] {
            self.side_board(
                StairPart::Handrail,
                Material::WallInterior,
                f,
                &sloped,
                lat,
                HANDRAIL_SIZE,
            );
            if ext_t > 1e-9 {
                self.side_board(
                    StairPart::Handrail,
                    Material::WallInterior,
                    f,
                    &level,
                    lat,
                    HANDRAIL_SIZE,
                );
            }
        }
    }

    /// Handrails along both edges of a curved ramp.
    fn arc_handrails(&mut self, arc: &RampArc) {
        let c = &arc.curve;
        let sweep = c.sweep();
        let n = ((sweep.to_degrees() / 7.5).ceil() as usize).max(1);
        for lat in [HANDRAIL_SIZE / 2.0, c.width - HANDRAIL_SIZE / 2.0] {
            for i in 0..n {
                let (a0, a1) = (
                    sweep * i as f64 / n as f64,
                    sweep * (i + 1) as f64 / n as f64,
                );
                let pa = self.scene(c.at_lat(a0, lat), arc.height_at(a0) + HANDRAIL_HEIGHT);
                let pb = self.scene(c.at_lat(a1, lat), arc.height_at(a1) + HANDRAIL_HEIGHT);
                let hint = [0.0, 1.0, 0.0];
                let hint = if (pb[0] - pa[0]).abs() + (pb[2] - pa[2]).abs() < 1e-9 {
                    [1.0, 0.0, 0.0]
                } else {
                    hint
                };
                self.out.extend(
                    bar(
                        pa,
                        pb,
                        hint,
                        (HANDRAIL_SIZE, HANDRAIL_SIZE),
                        Material::WallInterior,
                        Some(self.id),
                    )
                    .map(|m| (StairPart::Handrail, m)),
                );
            }
        }
    }

    /// A carpet runner down the middle of the treads; a tucked one also
    /// covers the face of each riser under the nosing.
    fn runner(&mut self, f: &Flight, h: f64, p: &StairParams, t: f64) {
        let rw = p.runner.width.min(f.width);
        let lat0 = (f.width - rw) / 2.0;
        for j in 1..=f.treads {
            let top = f.base + f64::from(j) * h;
            let s1 = f64::from(j) * t;
            let s0 = s1 - t - p.nosing;
            let profile: Vec<V3> = [(s0, lat0), (s1, lat0), (s1, lat0 + rw), (s0, lat0 + rw)]
                .iter()
                .map(|&(s, l)| self.on_flight(f, s, l, top))
                .collect();
            self.push(
                StairPart::Runner,
                Material::Floor,
                &profile,
                [0.0, RUNNER_THICKNESS, 0.0],
            );
            if p.runner.tucked {
                let face = s0 + p.nosing;
                let profile: Vec<V3> = [
                    (face - p.nosing - RUNNER_THICKNESS, lat0),
                    (face - p.nosing, lat0),
                    (face - p.nosing, lat0 + rw),
                    (face - p.nosing - RUNNER_THICKNESS, lat0 + rw),
                ]
                .iter()
                .map(|&(s, l)| self.on_flight(f, s, l, top - h + RUNNER_THICKNESS))
                .collect();
                self.push(
                    StairPart::Runner,
                    Material::Floor,
                    &profile,
                    [0.0, h - RUNNER_THICKNESS - p.tread_thickness, 0.0],
                );
            }
        }
    }

    /// A nosing along the edge of the top landing, over the top riser.
    fn top_nosing(&mut self, f: &Flight, h: f64, p: &StairParams) {
        let top = f.base + f64::from(f.risers) * h;
        let profile = self.flight_rect(
            f,
            f.len - p.nosing,
            f.len + p.riser_thickness,
            top - p.tread_thickness,
        );
        self.push(
            StairPart::Tread,
            Material::Floor,
            &profile,
            [0.0, p.tread_thickness, 0.0],
        );
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

    /// The sloped runs of a curved ramp, in strips of 5 degrees.
    fn curved_ramp(&mut self, arc: &RampArc, thick: f64) {
        let c = &arc.curve;
        for &(a0, a1, _, rise) in &arc.segs {
            if rise.abs() < 1e-9 {
                continue;
            }
            let n = (((a1 - a0).to_degrees() / 5.0).ceil() as usize).max(1);
            for k in 0..n {
                let (b0, b1) = (
                    a0 + (a1 - a0) * k as f64 / n as f64,
                    a0 + (a1 - a0) * (k + 1) as f64 / n as f64,
                );
                let (h0, h1) = (arc.height_at(b0), arc.height_at(b1));
                let profile = [
                    self.scene(c.at_lat(b0, 0.0), h0),
                    self.scene(c.at_lat(b0, c.width), h0),
                    self.scene(c.at_lat(b1, c.width), h1),
                    self.scene(c.at_lat(b1, 0.0), h1),
                ];
                self.push(
                    StairPart::Ramp,
                    Material::WallInterior,
                    &profile,
                    [0.0, -thick, 0.0],
                );
            }
        }
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

    /// The centre pole of a spiral stair: a 16-sided column from the floor
    /// to a guard height above the top step.
    fn pole(&mut self, c: &Curve, top: f64) {
        let r = c.inner.max(1.0);
        let profile: Vec<V3> = (0..16)
            .map(|i| {
                let a = std::f64::consts::TAU * f64::from(i) / 16.0;
                self.scene((c.center.0 + r * a.cos(), c.center.1 + r * a.sin()), 0.0)
            })
            .collect();
        self.push(
            StairPart::Stringer,
            Material::WallInterior,
            &profile,
            [0.0, top, 0.0],
        );
    }

    /// Treads, risers and stringers of a curved stair.
    fn curved(&mut self, c: &Curve, p: &StairParams, h: f64) {
        if p.spiral {
            self.pole(c, f64::from(c.risers) * h + crate::GUARD_HEIGHT);
        }
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
        color: None,
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
