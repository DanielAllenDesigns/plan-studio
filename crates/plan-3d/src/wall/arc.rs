//! A curved wall's frame: the wall-local `(s, t, h)` coordinates of a straight
//! wall's [`Frame`], with `s` the arc length along the centerline.
//!
//! The wall is a run of vertical facets. Both faces are polylines inscribed in
//! the offset arcs (radial offsets, so every vertex is exactly half the
//! thickness off the centerline) and the stations `s = len * k / n` are shared
//! by the two faces, so a rectangle drawn on a face is split at the facet
//! boundaries and each piece is flat. The first and last station of each face
//! can be moved onto a miter cut line ([`EndCuts`]); everything measured in
//! `s` (holes, roof profile, openings) keeps its meaning.

use crate::builder::{MeshBuilder, V3};
use crate::clip::{clip_half_plane, P2};
use crate::frame::{to_scene, Axis, Frame};
use plan_core::{Point, Wall};

/// Geometric tolerance, inches.
const EPS: f64 = 1e-6;
/// Inches per foot, for UV scaling.
const IN_PER_FT: f64 = 12.0;

/// Where the end of a curved wall is cut by its neighbour's miter: the cut
/// line through two plan points per end (the corner points where the
/// neighbour's faces meet the wall's faces). `None` is a square end.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EndCuts {
    pub start: Option<(Point, Point)>,
    pub end: Option<(Point, Point)>,
}

impl EndCuts {
    /// Square ends.
    pub const NONE: EndCuts = EndCuts {
        start: None,
        end: None,
    };

    /// The cuts the walls of a floor give curved `wall`: its ends mitered
    /// against the straight and curved walls that join it. Square for a
    /// straight wall or a wall that is not in `walls`; for a straight wall
    /// use [`EndCuts::from_outline`].
    pub fn of(walls: &[Wall], wall: &Wall) -> EndCuts {
        match plan_core::joins::curved_end_miters(walls, wall.id, 0.5) {
            Some([start, end]) => EndCuts { start, end },
            None => EndCuts::NONE,
        }
    }

    /// The cuts a straight wall's joined outline gives it
    /// (`plan_core::joins::wall_outlines`: start-left, end-left, end-right,
    /// start-right), so its ends meet the mitred ends of the walls it joins.
    pub fn from_outline(outline: &[Point]) -> EndCuts {
        match outline {
            [sl, el, er, sr] => EndCuts {
                start: Some((*sl, *sr)),
                end: Some((*el, *er)),
            },
            _ => EndCuts::NONE,
        }
    }

    /// Whether both ends are square.
    pub fn is_none(&self) -> bool {
        self.start.is_none() && self.end.is_none()
    }
}

/// The point where the line through `a` and `b` meets the circle of radius
/// `radius` about `center`, the one nearest `near`; `near` when it misses or
/// the hit is implausibly far (a miter cut off a very sharp corner).
fn cut_point(a: Point, b: Point, center: Point, radius: f64, near: Point, limit: f64) -> Point {
    let d = b.sub(a).normalized();
    if d.length() < EPS {
        return near;
    }
    let f = a.sub(center);
    let (half_b, c) = (f.dot(d), f.dot(f) - radius * radius);
    let disc = half_b * half_b - c;
    if disc < 0.0 {
        return near;
    }
    let root = disc.sqrt();
    [-half_b + root, -half_b - root]
        .into_iter()
        .map(|u| a.add(d.scale(u)))
        .min_by(|p, q| p.dist(near).total_cmp(&q.dist(near)))
        .filter(|p| p.dist(near) <= limit)
        .unwrap_or(near)
}

/// Where the cut line through `a` and `b` crosses the face line through
/// `from` along `dir`; `from` when parallel or implausibly far.
fn cut_line_point(a: Point, b: Point, from: Point, dir: Point, limit: f64) -> Point {
    let d = b.sub(a);
    let cross = dir.cross(d);
    if cross.abs() < 1e-9 {
        return from;
    }
    let u = a.sub(from).cross(d) / cross;
    let p = from.add(dir.scale(u));
    if p.dist(from) <= limit {
        p
    } else {
        from
    }
}

/// The frame of a curved wall at a floor elevation.
#[derive(Debug, Clone)]
pub struct ArcFrame {
    /// The left (`+t`) and right (`-t`) face polylines, `n + 1` stations each.
    left: Vec<Point>,
    right: Vec<Point>,
    /// Arc length of the centerline.
    len: f64,
    n: usize,
    half: f64,
    elevation: f64,
}

impl ArcFrame {
    /// The frame of curved `wall`, or of a straight wall with cut ends (a
    /// single facet); `None` for a plain straight wall.
    pub fn new(wall: &Wall, elevation: f64, cuts: &EndCuts) -> Option<Self> {
        let Some((curve, (center, r))) = wall
            .curve
            .filter(|c| !c.is_straight())
            .zip(wall.arc_center_radius())
        else {
            return Self::straight(wall, elevation, cuts);
        };
        let n = curve.facet_count(wall.start, wall.end).max(2);
        let half = (wall.thickness * 0.5).max(EPS);
        let mut left = wall.offset_curve(half, n);
        let mut right = wall.offset_curve(-half, n);
        let turn = curve.sweep(wall.start, wall.end).signum();
        let (rl, rr) = ((r - turn * half).max(0.01), (r + turn * half).max(0.01));
        let limit = wall.thickness * 6.0 + 1.0;
        for (cut, at) in [(cuts.start, 0), (cuts.end, n)] {
            if let Some((a, b)) = cut {
                left[at] = cut_point(a, b, center, rl, left[at], limit);
                right[at] = cut_point(a, b, center, rr, right[at], limit);
            }
        }
        Some(Self {
            left,
            right,
            len: wall.path_length(),
            n,
            half,
            elevation,
        })
    }

    /// A straight wall with cut ends: one facet, the faces' ends moved onto
    /// the cut lines.
    fn straight(wall: &Wall, elevation: f64, cuts: &EndCuts) -> Option<Self> {
        if cuts.is_none() || wall.length() <= EPS {
            return None;
        }
        let half = (wall.thickness * 0.5).max(EPS);
        let (dir, nrm) = (wall.direction(), wall.normal());
        let face = |side: f64| {
            let off = nrm.scale(side * half);
            vec![wall.start.add(off), wall.end.add(off)]
        };
        let (mut left, mut right) = (face(1.0), face(-1.0));
        // The plan outline has already applied the mitre limit; a thin layer
        // slab of a thick wall may still be cut far along its length.
        let limit = (wall.thickness * 6.0 + 1.0).max(600.0);
        for (cut, at) in [(cuts.start, 0), (cuts.end, 1)] {
            if let Some((a, b)) = cut {
                left[at] = cut_line_point(a, b, left[at], dir, limit);
                right[at] = cut_line_point(a, b, right[at], dir, limit);
            }
        }
        Some(Self {
            left,
            right,
            len: wall.length(),
            n: 1,
            half,
            elevation,
        })
    }

    /// Arc length of the centerline.
    pub fn length(&self) -> f64 {
        self.len
    }

    /// The facet `s` falls in and how far through it (0 to 1).
    fn locate(&self, s: f64) -> (usize, f64) {
        let u = (s / self.len.max(EPS) * self.n as f64).clamp(0.0, self.n as f64);
        let k = (u.floor() as usize).min(self.n - 1);
        (k, u - k as f64)
    }

    /// Plan position of local `(s, t)`.
    pub fn plan(&self, s: f64, t: f64) -> Point {
        let (k, f) = self.locate(s);
        let l = Point::lerp(self.left[k], self.left[k + 1], f);
        let r = Point::lerp(self.right[k], self.right[k + 1], f);
        Point::lerp(r, l, (t + self.half) / (2.0 * self.half))
    }

    /// Scene-space position of local `(s, t, h)`.
    pub fn point(&self, s: f64, t: f64, h: f64) -> V3 {
        to_scene(self.plan(s, t), self.elevation + h)
    }

    /// `a..b` cut at every facet boundary inside it.
    pub fn spans(&self, a: f64, b: f64) -> Vec<(f64, f64)> {
        let (a, b) = (a.min(b), a.max(b));
        let mut cuts = vec![a];
        for k in 1..self.n {
            let s = self.len * k as f64 / self.n as f64;
            if s > a + EPS && s < b - EPS {
                cuts.push(s);
            }
        }
        cuts.push(b);
        cuts.windows(2).map(|w| (w[0], w[1])).collect()
    }

    /// Scene direction of the plan vector `v`.
    fn scene_dir(v: Point) -> V3 {
        let n = v.normalized();
        [n.x as f32, 0.0, -n.y as f32]
    }

    /// Unit scene direction of the wall's `+t` over `a..b` (left of travel).
    fn across(&self, a: f64, b: f64) -> V3 {
        let d = self.plan(b, 0.0).sub(self.plan(a, 0.0));
        Self::scene_dir(d.perp())
    }

    /// Emit a rectangle perpendicular to `axis` at `at`, as [`Frame::face`]
    /// does for a straight wall: `r1` and `r2` run along the two remaining
    /// axes in the order S, T, H. A face along the arc is cut at the facet
    /// boundaries.
    #[allow(clippy::too_many_arguments)]
    pub fn face(
        &self,
        mesh: &mut MeshBuilder,
        axis: Axis,
        sign: f32,
        at: f64,
        r1: (f64, f64),
        r2: (f64, f64),
    ) {
        let uv = |a: f64, b: f64| [(a / IN_PER_FT) as f32, (b / IN_PER_FT) as f32];
        match axis {
            Axis::T => {
                for (a, b) in self.spans(r1.0, r1.1) {
                    let pts = [
                        self.point(a, at, r2.0),
                        self.point(b, at, r2.0),
                        self.point(b, at, r2.1),
                        self.point(a, at, r2.1),
                    ];
                    let uvs = [uv(a, r2.0), uv(b, r2.0), uv(b, r2.1), uv(a, r2.1)];
                    let n = self.across(a, b).map(|c| c * sign);
                    mesh.quad(pts, uvs, n);
                }
            }
            Axis::H => {
                for (a, b) in self.spans(r1.0, r1.1) {
                    let pts = [
                        self.point(a, r2.0, at),
                        self.point(b, r2.0, at),
                        self.point(b, r2.1, at),
                        self.point(a, r2.1, at),
                    ];
                    let uvs = [uv(a, r2.0), uv(b, r2.0), uv(b, r2.1), uv(a, r2.1)];
                    mesh.quad(pts, uvs, [0.0, sign, 0.0]);
                }
            }
            Axis::S => {
                let pts = [
                    self.point(at, r1.0, r2.0),
                    self.point(at, r1.1, r2.0),
                    self.point(at, r1.1, r2.1),
                    self.point(at, r1.0, r2.1),
                ];
                let uvs = [
                    uv(r1.0, r2.0),
                    uv(r1.1, r2.0),
                    uv(r1.1, r2.1),
                    uv(r1.0, r2.1),
                ];
                // The plane's normal: square to the cut line across the wall,
                // pointing the way the wall runs.
                let (k, _) = self.locate(at);
                let step = self.len / self.n as f64;
                let a = (k as f64 * step).min(self.len - step);
                let run = self.plan(a + step, 0.0).sub(self.plan(a, 0.0));
                let mut normal = self.plan(at, r1.1).sub(self.plan(at, r1.0)).perp();
                if normal.dot(run) < 0.0 {
                    normal = -normal;
                }
                mesh.quad(pts, uvs, Self::scene_dir(normal).map(|c| c * sign));
            }
        }
    }

    /// A convex polygon given in `(s, h)` on the face at `t`, facing `sign`
    /// along the wall's cross axis: cut at the facet boundaries, each piece
    /// fan-triangulated.
    pub fn poly_face(&self, mesh: &mut MeshBuilder, poly: &[P2], t: f64, sign: f32) {
        let (lo, hi) = poly.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p.0), hi.max(p.0))
        });
        for (a, b) in self.spans(lo, hi) {
            // The part of the polygon between the station lines `a` and `b`.
            let strip = clip_half_plane(poly, 1.0, 0.0, -a);
            let strip = clip_half_plane(&strip, -1.0, 0.0, b);
            if strip.len() < 3 {
                continue;
            }
            let normal = self.across(a, b).map(|c| c * sign);
            let pts: Vec<V3> = strip.iter().map(|&(s, h)| self.point(s, t, h)).collect();
            let uv: Vec<[f32; 2]> = strip
                .iter()
                .map(|&(s, h)| [(s / IN_PER_FT) as f32, (h / IN_PER_FT) as f32])
                .collect();
            for i in 1..strip.len() - 1 {
                mesh.tri(
                    [pts[0], pts[i], pts[i + 1]],
                    [uv[0], uv[i], uv[i + 1]],
                    normal,
                );
            }
        }
    }

    /// A sloped band along a top or bottom profile piece, from height `h0` at
    /// `s0` to `h1` at `s1`, spanning the thickness, facing up or down.
    pub fn band(&self, cap: &mut MeshBuilder, piece: (f64, f64, f64, f64), up: bool) {
        let (s0, s1, h0, h1) = piece;
        let height = |s: f64| {
            if (s1 - s0).abs() <= EPS {
                h0
            } else {
                h0 + (h1 - h0) * (s - s0) / (s1 - s0)
            }
        };
        let uv = |s: f64, t: f64| [(s / IN_PER_FT) as f32, (t / IN_PER_FT) as f32];
        for (a, b) in self.spans(s0, s1) {
            let (ha, hb) = (height(a), height(b));
            let pts = [
                self.point(a, -self.half, ha),
                self.point(b, -self.half, hb),
                self.point(b, self.half, hb),
                self.point(a, self.half, ha),
            ];
            let mut n = crate::builder::cross(
                crate::builder::sub(pts[1], pts[0]),
                crate::builder::sub(pts[3], pts[0]),
            );
            if (n[1] < 0.0) == up {
                n = n.map(|c| -c);
            }
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
            cap.quad(
                pts,
                [
                    uv(a, -self.half),
                    uv(b, -self.half),
                    uv(b, self.half),
                    uv(a, self.half),
                ],
                n.map(|c| c / len),
            );
        }
    }
}

/// A wall's local frame, straight or following an arc.
#[derive(Debug, Clone)]
pub enum WallFrame {
    Straight(Frame, f64),
    Arc(ArcFrame),
}

impl WallFrame {
    /// The frame of `wall` at `elevation`; a curved wall follows its arc, with
    /// its ends cut by `cuts`.
    pub fn new(wall: &Wall, elevation: f64, cuts: &EndCuts) -> Self {
        match ArcFrame::new(wall, elevation, cuts) {
            Some(arc) => WallFrame::Arc(arc),
            None => WallFrame::Straight(Frame::new(wall, elevation), wall.length()),
        }
    }

    /// Length along the wall: the chord, or the arc length of a curved wall.
    pub fn length(&self) -> f64 {
        match self {
            WallFrame::Straight(_, len) => *len,
            WallFrame::Arc(a) => a.length(),
        }
    }

    /// See [`Frame::cuboid`]. On an arc the box is bent along it: its two
    /// side faces and its top and bottom are cut at the facets.
    pub fn cuboid(&self, mesh: &mut MeshBuilder, s: (f64, f64), t: (f64, f64), h: (f64, f64)) {
        match self {
            WallFrame::Straight(f, _) => f.cuboid(mesh, s, t, h),
            WallFrame::Arc(a) => {
                a.face(mesh, Axis::S, -1.0, s.0, t, h);
                a.face(mesh, Axis::S, 1.0, s.1, t, h);
                a.face(mesh, Axis::T, -1.0, t.0, s, h);
                a.face(mesh, Axis::T, 1.0, t.1, s, h);
                a.face(mesh, Axis::H, -1.0, h.0, s, t);
                a.face(mesh, Axis::H, 1.0, h.1, s, t);
            }
        }
    }

    /// See [`Frame::face`].
    #[allow(clippy::too_many_arguments)]
    pub fn face(
        &self,
        mesh: &mut MeshBuilder,
        axis: Axis,
        sign: f32,
        at: f64,
        r1: (f64, f64),
        r2: (f64, f64),
    ) {
        match self {
            WallFrame::Straight(f, _) => f.face(mesh, axis, sign, at, r1, r2),
            WallFrame::Arc(a) => a.face(mesh, axis, sign, at, r1, r2),
        }
    }
}
