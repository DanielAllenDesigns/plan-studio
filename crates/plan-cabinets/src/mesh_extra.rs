//! The mesh builders that go beyond boxes: extruded outlines with holes,
//! chamfered countertop edges, corner cabinets, custom tops, fixtures and
//! moldings.

use plan_3d::{Material, Vertex};
use plan_core::geometry::Point;
use std::f64::consts::TAU;

use crate::cabinet::{Cabinet, CabinetKind, CornerStyle, MoldingKind};
use crate::dress::{FootStyle, PilasterStyle};
use crate::geom;
use crate::mesh3d::{build_fronts, pick, Builder, Frame, FrontCtx, FRAME, PANEL, V3};
use crate::top::{CutoutKind, EdgeProfile};

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn at(p: Point, z: f64) -> V3 {
    [p.x, p.y, z]
}

impl Builder {
    /// Adds flat-shaded triangles (each counter-clockwise seen from outside,
    /// local frame) as one mesh. Degenerate triangles are skipped.
    pub(crate) fn add_tris(&mut self, tris: &[[V3; 3]], material: Material) {
        let mut vertices = Vec::with_capacity(tris.len() * 3);
        let mut indices = Vec::with_capacity(tris.len() * 3);
        for t in tris {
            let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len < 1e-9 {
                continue;
            }
            let n = [n[0] / len, n[1] / len, n[2] / len];
            let axis = if n[2].abs() >= n[0].abs() && n[2].abs() >= n[1].abs() {
                2
            } else if n[0].abs() >= n[1].abs() {
                0
            } else {
                1
            };
            let (ua, va) = match axis {
                0 => (1, 2),
                1 => (0, 2),
                _ => (0, 1),
            };
            let base = vertices.len() as u32;
            for p in t {
                vertices.push(Vertex {
                    position: self.point(*p),
                    normal: self.direction(n),
                    uv: [(p[ua] / 12.0) as f32, (p[va] / 12.0) as f32],
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
        self.push(vertices, indices, material);
    }

    /// Extrudes `outer` (minus `holes`) from `z0` to `z1` as one mesh: caps
    /// and side walls, the hole walls facing into the holes.
    pub(crate) fn add_prism(
        &mut self,
        outer: &[Point],
        holes: &[Vec<Point>],
        z0: f64,
        z1: f64,
        material: Material,
    ) {
        if outer.len() < 3 || z1 - z0 <= 1e-9 {
            return;
        }
        let outer = geom::ccw(outer);
        let holes: Vec<Vec<Point>> = holes
            .iter()
            .filter(|h| h.len() >= 3)
            .map(|h| {
                let mut r = geom::ccw(h);
                r.reverse();
                r
            })
            .collect();
        let mut tris: Vec<[V3; 3]> = Vec::new();
        for t in geom::triangulate(&outer, &holes) {
            tris.push([at(t[0], z1), at(t[1], z1), at(t[2], z1)]);
            tris.push([at(t[0], z0), at(t[2], z0), at(t[1], z0)]);
        }
        for ring in std::iter::once(&outer).chain(holes.iter()) {
            wall(&mut tris, ring, z0, z1, ring);
        }
        self.add_tris(&tris, material);
    }

    /// A countertop slab: like [`Builder::add_prism`] but with a chamfered or
    /// rounded top edge along the outer outline.
    pub(crate) fn add_slab(
        &mut self,
        outer: &[Point],
        holes: &[Vec<Point>],
        (z0, z1): (f64, f64),
        edge: EdgeProfile,
        size: f64,
        material: Material,
    ) {
        let s = size.min((z1 - z0) * 0.9).min(3.0);
        if matches!(edge, EdgeProfile::Square | EdgeProfile::Waterfall)
            || s <= 1e-6
            || outer.len() < 3
        {
            self.add_prism(outer, holes, z0, z1, material);
            return;
        }
        let outer = geom::ccw(outer);
        // (inset, z) points of the edge profile from the side wall up to the top.
        let n = edge.steps();
        let profile: Vec<(f64, f64)> = (0..=n)
            .map(|k| match edge {
                EdgeProfile::Bullnose => {
                    let th =
                        std::f64::consts::PI - std::f64::consts::FRAC_PI_2 * k as f64 / n as f64;
                    (s + s * th.cos(), z1 - s + s * th.sin())
                }
                EdgeProfile::Ogee => {
                    // A cove (concave quarter circle) under a bead (convex one).
                    let h = s / 2.0;
                    let half = (n / 2).max(1);
                    if k <= half {
                        let th = std::f64::consts::FRAC_PI_2 * k as f64 / half as f64;
                        (h * th.sin(), h * (1.0 - th.cos()) + (z1 - s))
                    } else {
                        let ph = std::f64::consts::FRAC_PI_2 * (k - half) as f64 / half as f64;
                        (s - h * ph.cos(), h * ph.sin() + (z1 - s + h))
                    }
                }
                _ => (s * k as f64 / n as f64, z1 - s + s * k as f64 / n as f64),
            })
            .collect();
        let rings: Vec<Vec<Point>> = profile
            .iter()
            .map(|(inset, _)| geom::offset_ring(&outer, -inset))
            .collect();
        let top = rings.last().cloned().unwrap_or_default();
        let holes: Vec<Vec<Point>> = holes
            .iter()
            .filter(|h| h.len() >= 3)
            .map(|h| {
                let mut r = geom::ccw(h);
                r.reverse();
                r
            })
            .collect();
        if top.len() < 3 || geom::area(&top) < 1e-3 {
            self.add_prism(&outer, &holes, z0, z1, material);
            return;
        }
        let mut tris: Vec<[V3; 3]> = Vec::new();
        for t in geom::triangulate(&outer, &holes) {
            tris.push([at(t[0], z0), at(t[2], z0), at(t[1], z0)]);
        }
        for t in geom::triangulate(&top, &holes) {
            tris.push([at(t[0], z1), at(t[1], z1), at(t[2], z1)]);
        }
        // Side wall up to the start of the profile, then one band per step.
        wall(&mut tris, &outer, z0, profile[0].1, &outer);
        for k in 0..n {
            wall(
                &mut tris,
                &rings[k],
                profile[k].1,
                profile[k + 1].1,
                &rings[k + 1],
            );
        }
        for hole in &holes {
            wall(&mut tris, hole, z0, z1, hole);
        }
        self.add_tris(&tris, material);
    }

    /// Sink basins and cooktop plates in the countertop holes. `top` is the
    /// local z of the countertop surface.
    pub(crate) fn fixtures(&mut self, cab: &Cabinet, top: f64) {
        for cut in &cab.cutouts {
            let Some((lo, hi)) = geom::bbox(&cut.outline) else {
                continue;
            };
            match cut.kind {
                CutoutKind::Sink => {
                    let (x0, y0, x1, y1) = (lo.x + 0.5, lo.y + 0.5, hi.x - 0.5, hi.y - 0.5);
                    if x1 - x0 < 2.0 || y1 - y0 < 2.0 {
                        continue;
                    }
                    let depth = 7.0_f64.min(top - 1.0).max(1.0);
                    let (zb, t) = (top - depth, 0.25);
                    let m = Material::Metal;
                    self.add_box([x0, y0, zb - t], [x1, y1, zb], m);
                    self.add_box([x0, y0, zb], [x0 + t, y1, top - 0.1], m);
                    self.add_box([x1 - t, y0, zb], [x1, y1, top - 0.1], m);
                    self.add_box([x0 + t, y0, zb], [x1 - t, y0 + t, top - 0.1], m);
                    self.add_box([x0 + t, y1 - t, zb], [x1 - t, y1, top - 0.1], m);
                }
                CutoutKind::Cooktop => {
                    self.add_box(
                        [lo.x - 0.75, lo.y - 0.75, top],
                        [hi.x + 0.75, hi.y + 0.75, top + 0.3],
                        Material::Glass,
                    );
                }
                CutoutKind::Custom => {}
            }
        }
    }

    /// A free-form countertop slab.
    pub(crate) fn custom_countertop(&mut self, cab: &Cabinet) {
        let Some(c) = &cab.custom else { return };
        let (edge, edge_size) = cab.top_effective_profile();
        let material = pick(cab.materials.countertop, Material::Floor);
        self.add_slab(
            &c.treated(),
            &cab.holes_local(),
            (0.0, cab.height),
            edge,
            edge_size,
            material,
        );
        self.fixtures(cab, cab.height);
        // Waterfalls: a vertical slab as thick as the top under each marked
        // edge, down to the floor or by the specified height.
        for i in cab.waterfall_edges() {
            if let Some((ring, drop)) = cab.waterfall_slab(i) {
                self.add_prism(&ring, &[], -drop, 0.0, material);
            }
        }
    }

    /// A backsplash strip along its path.
    pub(crate) fn custom_backsplash(&mut self, cab: &Cabinet) {
        let Some(c) = &cab.custom else { return };
        let strip = geom::thicken_path(&c.outline, c.thickness);
        self.add_prism(
            &strip,
            &[],
            0.0,
            cab.height,
            pick(cab.materials.backsplash, Material::Floor),
        );
    }

    /// Corner base and wall cabinets, built from their L or diagonal outline.
    pub(crate) fn corner(&mut self, cab: &Cabinet) {
        let (w, d, h) = (cab.width, cab.depth, cab.height);
        let spec = cab.corner.unwrap_or_default();
        let a = spec.arm_depth.clamp(1.0, w.min(d));
        let ring = cab.footprint_local();
        let toe = cab.toe_kick.filter(|t| t.height > 0.0);
        let z0 = toe.map_or(0.0, |t| t.height);
        let z1 = h - cab.countertop.map_or(0.0, |c| c.thickness);
        let front_t = cab.door_style.thickness.max(cab.drawer_style.thickness);
        let wi = pick(cab.materials.carcass, Material::WallInterior);

        // Bottom (and top, for a wall cabinet), back walls, arm ends.
        self.add_prism(&ring, &[], z0, z0 + PANEL, wi);
        if !cab.kind.is_base_like() {
            self.add_prism(&ring, &[], z1 - PANEL, z1, wi);
        }
        self.add_box([0.0, 0.0, z0], [w, PANEL, z1], wi);
        self.add_box([0.0, PANEL, z0], [PANEL, d, z1], wi);
        self.add_box(
            [w - PANEL, PANEL, z0],
            [w, (a - front_t).max(PANEL + 0.1), z1],
            wi,
        );
        self.add_box(
            [PANEL, d - PANEL, z0],
            [(a - front_t).max(PANEL + 0.1), d, z1],
            wi,
        );

        // Toe kick along the front, set back by its depth.
        if let Some(tk) = toe {
            let path = match spec.style {
                CornerStyle::PieCut => {
                    let o = tk.depth + FRAME / 2.0;
                    vec![
                        Point::new(w, a - o),
                        Point::new(a - o, a - o),
                        Point::new(a - o, d),
                    ]
                }
                CornerStyle::Diagonal => {
                    let n = Point::new(d - a, w - a).normalized();
                    let s = n.scale(-(tk.depth + FRAME / 2.0));
                    vec![Point::new(w, a).add(s), Point::new(a, d).add(s)]
                }
            };
            let strip = geom::thicken_path(&path, FRAME);
            self.add_prism(
                &strip,
                &[],
                0.0,
                tk.height,
                pick(cab.materials.toe_kick, Material::WallInterior),
            );
        }

        // Countertop and backsplash.
        if cab.countertop.is_some() {
            self.countertop(cab, z1, h);
            if let (Some(bs), Some(top)) =
                (cab.backsplash.filter(|s| s.height > 0.0), cab.top_local())
            {
                // A strip along both back walls.
                let path = vec![
                    Point::new(top[1].x, 0.0),
                    Point::ZERO,
                    Point::new(0.0, top[top.len() - 1].y),
                ];
                let strip = geom::thicken_path(&path, bs.thickness);
                self.add_prism(
                    &strip,
                    &[],
                    h,
                    h + bs.height,
                    pick(cab.materials.backsplash, Material::Floor),
                );
            }
        }

        // Fronts: one frame per straight run of the front.
        let fronts: Vec<Frame> = match spec.style {
            CornerStyle::PieCut => vec![
                Frame::along(Point::new(a, a), Point::new(w, a)),
                Frame::along(Point::new(a, d), Point::new(a, a)),
            ],
            CornerStyle::Diagonal => vec![Frame::along(Point::new(a, d), Point::new(w, a))],
        };
        for frame in fronts {
            if frame.len <= 1.0 || z1 <= z0 {
                continue;
            }
            let ctx = FrontCtx {
                cab,
                layout: &cab.face,
                frame,
                carcass_y: -front_t,
                z0,
                z1,
            };
            build_fronts(self, &ctx, 0.0, frame.len);
        }

        // Lazy susan: two round shelves on a centre pole.
        if spec.style == CornerStyle::PieCut && spec.lazy_susan {
            let c = Point::new(a / 2.0, a / 2.0);
            let r = (a / 2.0 - 1.5).max(1.0);
            let disc: Vec<Point> = (0..24)
                .map(|i| {
                    let th = TAU * f64::from(i) / 24.0;
                    Point::new(c.x + r * th.cos(), c.y + r * th.sin())
                })
                .collect();
            for zs in [z0 + PANEL + 3.0, (z0 + z1) / 2.0] {
                if zs + PANEL < z1 {
                    self.add_prism(&disc, &[], zs, zs + PANEL, wi);
                }
            }
            self.add_box(
                [c.x - 0.5, c.y - 0.5, z0 + PANEL],
                [c.x + 0.5, c.y + 0.5, (z1 - PANEL).max(z0 + PANEL + 1.0)],
                Material::Metal,
            );
        }
    }

    /// Cabinets whose outline is not a rectangle and not a corner: the
    /// special shapes (end, radius end, peninsula, angled and bow fronts)
    /// and boxes with clipped or rounded corners. The box is a shell of the
    /// outline; the fronts run along chords of the front line, one per face
    /// item.
    pub(crate) fn shaped(&mut self, cab: &Cabinet) {
        let (w, d, h) = (cab.width, cab.depth, cab.height);
        let ring = cab.footprint_local();
        let toe = cab.toe_kick.filter(|t| t.height > 0.0);
        let z0 = toe.map_or(0.0, |t| t.height);
        let z1 = h - cab.countertop.map_or(0.0, |c| c.thickness);
        let front_t = cab.door_style.thickness.max(cab.drawer_style.thickness);
        let wi = pick(cab.materials.carcass, Material::WallInterior);
        let st = cab.box_construction.side_thickness.max(0.1);
        let front_at = |x: f64| match &cab.special {
            Some(sp) => sp.depth_at(w, d, x),
            None => d,
        };

        // The shell, the bottom and the top.
        if z1 > z0 {
            let inner = geom::offset_ring(&ring, -st);
            let shell_ok = inner.len() == ring.len() && geom::area(&inner) > 1.0;
            if shell_ok {
                self.add_prism(&ring, &[inner], z0, z1, wi);
            } else {
                self.add_prism(&ring, &[], z0, z1, wi);
            }
            if cab.box_has_bottom() {
                self.add_prism(&ring, &[], z0, z0 + PANEL, wi);
            }
            if cab.box_has_top() {
                self.add_prism(&ring, &[], (z1 - PANEL).max(z0), z1, wi);
            }
        }

        // Toe kick along the front line, set back by its depth.
        if let Some(tk) = toe {
            let front = match &cab.special {
                Some(sp) => sp.front(w, d),
                None => vec![Point::new(0.0, d), Point::new(w, d)],
            };
            let back_off = tk.depth + FRAME / 2.0;
            let n = front.len();
            let path: Vec<Point> = (0..n)
                .map(|i| {
                    let a = front[i.saturating_sub(1)];
                    let b = front[(i + 1).min(n - 1)];
                    let t = b.sub(a).normalized();
                    // Outward is to the front (+y side) of the run.
                    let out = Point::new(-t.y, t.x);
                    front[i].sub(out.scale(back_off))
                })
                .collect();
            let strip = geom::thicken_path(&path, FRAME);
            self.add_prism(
                &strip,
                &[],
                0.0,
                tk.height,
                pick(cab.materials.toe_kick, Material::WallInterior),
            );
        }

        // Countertop and backsplash.
        if cab.countertop.is_some() {
            self.countertop(cab, z1, h);
        }
        self.back_splash(cab, h);

        // Fronts: one chord of the front line per face item.
        let fw = if cab.framed {
            cab.face.frame_width
        } else {
            0.0
        };
        let face_w = w - 2.0 * fw;
        if face_w <= 0.0 || z1 <= z0 {
            return;
        }
        if cab.framed {
            for (xa, xb) in [(0.0, fw), (w - fw, w)] {
                let frame =
                    Frame::along(Point::new(xa, front_at(xa)), Point::new(xb, front_at(xb)));
                if frame.len > 1e-6 {
                    crate::mesh3d::frame_box(
                        self,
                        frame,
                        (0.0, frame.len),
                        (z0, z1),
                        (-FRAME, 0.0),
                        wi,
                    );
                }
            }
        }
        let Ok(items) = cab.face.resolve(z1 - z0, face_w) else {
            return;
        };
        for r in items {
            let (x, y, iw, ih) = r.rect;
            let (xa, xb) = (fw + x, fw + x + iw);
            let frame = Frame::along(Point::new(xa, front_at(xa)), Point::new(xb, front_at(xb)));
            if frame.len <= 1e-6 {
                continue;
            }
            let ctx = FrontCtx {
                cab,
                layout: &cab.face,
                frame,
                carcass_y: -front_t,
                z0,
                z1,
            };
            let rect = (0.0, frame.len, z0 + y, z0 + y + ih);
            crate::mesh3d::build_item(self, &ctx, &r, rect);
        }
    }

    /// Crown molding on top and light rail underneath: a front run and two
    /// returns on rectangular cabinets, a ring around the outline on corner
    /// cabinets.
    pub(crate) fn moldings(&mut self, cab: &Cabinet) {
        let (w, d, h) = (cab.width, cab.depth, cab.height);
        for m in &cab.moldings {
            let (z0, z1) = match m.kind {
                MoldingKind::Crown => (h, h + m.height),
                MoldingKind::LightRail => (-m.height, 0.0),
            };
            let material = pick(cab.materials.molding, Material::Trim);
            let p = m.projection;
            if cab.kind == CabinetKind::CornerBase
                || cab.kind == CabinetKind::CornerWall
                || cab.is_shaped()
            {
                let inner = cab.footprint_local();
                let outer = geom::offset_ring(&inner, p);
                self.add_prism(&outer, &[inner], z0, z1, material);
            } else {
                self.add_box([-p, d, z0], [w + p, d + p, z1], material);
                self.add_box([-p, 0.0, z0], [0.0, d, z1], material);
                self.add_box([w, 0.0, z0], [w + p, d, z1], material);
            }
        }
    }
}

/// Wall quads for every edge of `lower` (ring at `z.0`) up to `upper` (ring
/// at `z.1`), the rings having the same vertex count. Edges run counter-
/// clockwise around material, so the quads face away from it.
fn wall(tris: &mut Vec<[V3; 3]>, lower: &[Point], z_lo: f64, z_hi: f64, upper: &[Point]) {
    let n = lower.len();
    if upper.len() != n {
        return;
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let (a0, b0) = (at(lower[i], z_lo), at(lower[j], z_lo));
        let (a1, b1) = (at(upper[i], z_hi), at(upper[j], z_hi));
        tris.push([a0, b0, b1]);
        tris.push([a0, b1, a1]);
    }
}

impl Builder {
    /// Four feet under a base cabinet in place of the toe kick board
    /// (Accessories tab). `front` is where the kick's front face would be.
    pub(crate) fn feet(&mut self, cab: &Cabinet, front: f64, height: f64) {
        let (w, s) = (cab.width, cab.accessories.foot_size.max(1.0));
        if w < 2.0 * s + 2.0 || front < s + 1.5 {
            return;
        }
        let mat = pick(cab.materials.toe_kick, Material::WallInterior);
        let acc = cab.accessories;
        let (wo, dof) = (
            acc.foot_width_offset.max(0.0),
            acc.foot_depth_offset.max(0.0),
        );
        let xs = [1.0 + wo, w - 1.0 - s - wo];
        let ys = [1.0 + dof, (front - s - dof).max(1.0)];
        // Feet stand under the exposed ends only, unless Always Present.
        let ends = cab.ends.unwrap_or_default();
        let skip = |x: f64, y: f64| {
            !acc.feet_always
                && ((x < w / 2.0 && ends.left)
                    || (x >= w / 2.0 && ends.right)
                    || (y < front / 2.0 && ends.back))
        };
        for &x in &xs {
            for &y in &ys {
                if skip(x, y) {
                    continue;
                }
                match cab.accessories.feet {
                    FootStyle::None => {}
                    FootStyle::Block => {
                        self.add_box([x, y, 0.0], [x + s, y + s, height], mat);
                    }
                    FootStyle::Bun => {
                        // Turned foot: a narrow pad, the swell, a narrow neck.
                        let (cx, cy) = (x + s / 2.0, y + s / 2.0);
                        for (z0, z1, k) in [(0.0, 0.25, 0.6), (0.25, 0.75, 1.0), (0.75, 1.0, 0.7)] {
                            let r = s * k / 2.0;
                            self.add_box(
                                [cx - r, cy - r, height * z0],
                                [cx + r, cy + r, height * z1],
                                mat,
                            );
                        }
                    }
                    FootStyle::Bracket => {
                        // Two thin boards meeting at the outside corner.
                        let t = 0.75_f64.min(s / 2.0);
                        let fx = if x < w / 2.0 { x } else { x + s - t };
                        let fy = if y < front / 2.0 { y } else { y + s - t };
                        self.add_box([x, fy, 0.0], [x + s, fy + t, height], mat);
                        self.add_box([fx, y, 0.0], [fx + t, y + s, height], mat);
                    }
                }
            }
        }
    }

    /// Pilasters on the front corners (Accessories tab): a board standing
    /// proud of the front, fluted ones with two grooves' worth of ribs.
    pub(crate) fn pilasters(&mut self, cab: &Cabinet, z0: f64, z1: f64) {
        let (left, right) = cab.accessories.pilasters();
        let pw = cab
            .accessories
            .pilaster_width
            .clamp(0.5, (cab.width / 3.0).max(0.5));
        let d = cab.depth;
        let mat = pick(cab.materials.door, Material::WallInterior);
        let ends = cab.ends.unwrap_or_default();
        let z0 = if cab.accessories.pilaster_to_bottom {
            0.0
        } else {
            z0
        };
        // Auto pilasters: neighbours share one, centred on their line, built
        // by the cabinet on its left.
        let auto = cab.accessories.pilaster_auto;
        for (on, mut x0, mated, is_left) in [
            (left, 0.0, ends.left, true),
            (right, cab.width - pw, ends.right, false),
        ] {
            if !on || z1 - z0 < 1.0 || (auto && mated && is_left) {
                continue;
            }
            if auto && mated && !is_left {
                x0 = cab.width - pw / 2.0;
            }
            self.add_box([x0, d, z0], [x0 + pw, d + 0.75, z1], mat);
            if cab.accessories.pilaster == PilasterStyle::Fluted {
                for k in 1..=2 {
                    let cx = x0 + pw * f64::from(k) / 3.0;
                    self.add_box(
                        [cx - 0.1, d + 0.75, z0 + 1.0],
                        [cx + 0.1, d + 0.85, z1 - 1.0],
                        mat,
                    );
                }
            }
        }
    }
}
