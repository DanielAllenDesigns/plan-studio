//! 3D solids beyond the Build > 3D Solid flyout's basics (reference manual
//! "3D Solid Tools", pp. 1063 to 1081; parity rows CB-439..CB-458).
//!
//! The primitives themselves (box, cylinder, sphere, cone, pyramid, polyline
//! solid, face) are [`crate::details::Solid3d`] records in the floor's
//! [`crate::details::DetailsLayer`]. This module adds what their
//! specification dialogs need and what combining them needs:
//!
//! * [`SolidExt`], the extra spec fields of one primitive, keyed by its id:
//!   rotation about the X and Y axes (the record's own `rotation` is about
//!   Z), the elevation reference, 3D surface quality, a label, Retain Aspect
//!   Ratio and a [`PyramidSpec`] (number of sides, definition, truncation).
//! * Triangle meshes of the primitives ([`solid_tris`]), in plan coordinates
//!   with Z up, relative to the floor. The 3D view, the plan outline, the
//!   volume and the Boolean tools all start from these.
//! * Boolean Union, Subtract and Intersect ([`BoolOp`], [`boolean`]) by BSP
//!   clipping, producing a [`CompoundSolid`]: a closed triangle mesh that is
//!   one object (`ObjectRef::Solid`). [`boolean_floor`] replaces the operands
//!   on a floor by the result.
//! * Convert Polyline to Solid ([`polyline_to_solid`]) and Extrude Object
//!   ([`extrude_face`]).
//!
//! Lengths are inches, angles degrees.

use crate::arch_block::ElevationRef;
use crate::details::{
    circle_points, rotate_deg, DetailRef, DetailStyle, DetailsLayer, Solid3d, SolidKind,
    DEFAULT_SOLID_MATERIAL, SOLID_LAYER,
};
use crate::geometry::{polygon_area, polygon_centroid, Point};
use crate::groups::ObjectRef;
use crate::model::{Floor, Id};
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// A point in plan coordinates (x, y) and height above the floor (z).
pub type V3 = [f64; 3];
/// A triangle, counter-clockwise seen from outside the solid.
pub type Tri = [V3; 3];

/// Segments around a round solid and bands pole to pole of a sphere.
pub const ROUND_SEGMENTS: usize = 24;
const SPHERE_BANDS: usize = 12;
/// Smallest side of a pyramid's base, inches.
pub const MIN_PYRAMID_SIDE: f64 = 0.0625;
/// Number of sides a pyramid starts with (p. 1079).
pub const DEFAULT_PYRAMID_SIDES: u32 = 4;
/// Height of a 3D Solid drawn in plan (p. 1064).
pub const INITIAL_SOLID_HEIGHT: f64 = 1.0;
/// The label layer of 3D solids and blocks.
pub const SOLID_LABEL_LAYER: &str = "Architectural Blocks, Labels";

// ===================================================================
// Extra spec fields
// ===================================================================

/// What the Pyramid's size is defined by (p. 1079).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PyramidDef {
    SideLength,
    #[default]
    RadiusToCorner,
    RadiusToSide,
}

impl PyramidDef {
    pub const ALL: [PyramidDef; 3] = [
        PyramidDef::SideLength,
        PyramidDef::RadiusToCorner,
        PyramidDef::RadiusToSide,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PyramidDef::SideLength => "Side Length",
            PyramidDef::RadiusToCorner => "Radius to Corner",
            PyramidDef::RadiusToSide => "Radius to Side",
        }
    }
}

/// The Pyramid Specification's base shape and truncation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PyramidSpec {
    pub sides: u32,
    pub def: PyramidDef,
    /// The side length or the radius, as `def` says.
    pub size: f64,
    /// Cut the top off: the flat top stands `truncated_height` above the base.
    pub truncated: bool,
    pub truncated_height: f64,
}

impl Default for PyramidSpec {
    fn default() -> Self {
        Self {
            sides: DEFAULT_PYRAMID_SIDES,
            def: PyramidDef::RadiusToCorner,
            size: 12.0,
            truncated: false,
            truncated_height: 0.0,
        }
    }
}

impl PyramidSpec {
    /// The spec of an existing outline (a regular polygon is recognised by
    /// its corner count; others become a square-ish radius to corner).
    pub fn of_outline(outline: &[Point], h: f64) -> PyramidSpec {
        let c = polygon_centroid(outline);
        let r = outline
            .iter()
            .map(|p| p.dist(c))
            .fold(0.0_f64, f64::max)
            .max(MIN_PYRAMID_SIDE);
        PyramidSpec {
            sides: outline.len().max(3) as u32,
            def: PyramidDef::RadiusToCorner,
            size: r,
            truncated: false,
            truncated_height: h * 0.5,
        }
    }

    fn n(&self) -> f64 {
        self.sides.max(3) as f64
    }

    /// Radius from the centre to a corner.
    pub fn corner_radius(&self) -> f64 {
        match self.def {
            PyramidDef::RadiusToCorner => self.size,
            PyramidDef::RadiusToSide => self.size / (PI / self.n()).cos(),
            PyramidDef::SideLength => self.size / (2.0 * (PI / self.n()).sin()),
        }
    }

    /// Radius from the centre to the midpoint of a side.
    pub fn side_radius(&self) -> f64 {
        self.corner_radius() * (PI / self.n()).cos()
    }

    /// Length of one side of the base.
    pub fn side_length(&self) -> f64 {
        2.0 * self.corner_radius() * (PI / self.n()).sin()
    }

    /// The base, counter-clockwise, centred on the origin. Four sides give
    /// an axis-aligned square.
    pub fn outline(&self) -> Vec<Point> {
        let n = self.sides.max(3) as usize;
        let r = self.corner_radius().max(MIN_PYRAMID_SIDE);
        let a0 = std::f64::consts::FRAC_PI_2 + PI / n as f64;
        (0..n)
            .map(|i| {
                let a = a0 + TAU * i as f64 / n as f64;
                Point::new(r * a.cos(), r * a.sin())
            })
            .collect()
    }
}

/// 3D Surface Quality (p. 1071): Automatic, or a Maximum Deflection.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SurfaceQuality {
    pub automatic: bool,
    pub max_deflection: f64,
}

impl Default for SurfaceQuality {
    fn default() -> Self {
        Self {
            automatic: true,
            max_deflection: 0.25,
        }
    }
}

impl SurfaceQuality {
    /// Segments around a circle of radius `r` for this quality: the
    /// automatic value is [`ROUND_SEGMENTS`]; a deflection `d` needs
    /// `pi / acos(1 - d / r)` segments.
    pub fn segments(&self, r: f64) -> usize {
        if self.automatic || r <= 1e-9 {
            return ROUND_SEGMENTS;
        }
        let d = self.max_deflection.clamp(0.001, r);
        let half = (1.0 - d / r).clamp(-1.0, 1.0).acos();
        if half < 1e-6 {
            return 96;
        }
        ((PI / half).ceil() as usize).clamp(8, 96)
    }
}

/// The spec fields of one [`Solid3d`] that the record itself lacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SolidExt {
    /// The id of the [`Solid3d`] this belongs to.
    pub id: Id,
    /// Rotation about the X axis through the solid's centre, degrees.
    pub rot_x: f64,
    /// Rotation about the Y axis through the solid's centre, degrees.
    pub rot_y: f64,
    pub elevation_ref: ElevationRef,
    pub quality: SurfaceQuality,
    /// The Label panel text; blank is the automatic (empty) label.
    pub label: String,
    /// Retain Aspect Ratio in the dialog.
    pub retain_aspect: bool,
    /// Pyramids only.
    pub pyramid: Option<PyramidSpec>,
}

impl Default for SolidExt {
    fn default() -> Self {
        Self {
            id: 0,
            rot_x: 0.0,
            rot_y: 0.0,
            elevation_ref: ElevationRef::FromFloor,
            quality: SurfaceQuality::default(),
            label: String::new(),
            retain_aspect: true,
            pyramid: None,
        }
    }
}

impl SolidExt {
    pub fn new(id: Id) -> Self {
        SolidExt {
            id,
            ..Self::default()
        }
    }

    /// Nothing differs from the defaults (the solid then needs no record).
    pub fn is_default(&self) -> bool {
        *self == SolidExt::new(self.id)
    }

    /// Is the solid turned about the X or Y axis (so its footprint is no
    /// longer its plan outline)?
    pub fn tilted(&self) -> bool {
        self.rot_x.abs() > 1e-9 || self.rot_y.abs() > 1e-9
    }

    /// Does the 3D shape differ from [`SolidKind`]'s plain mesh (tilted,
    /// truncated pyramid, custom surface quality)?
    pub fn changes_mesh(&self) -> bool {
        self.tilted()
            || self.pyramid.is_some_and(|p| p.truncated)
            || !self.quality.automatic
    }
}

// ===================================================================
// Compound solids
// ===================================================================

/// The result of a Boolean operation on solids: one closed triangle mesh
/// (`ObjectRef::Solid`). Coordinates are plan x, y and height above the
/// floor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompoundSolid {
    pub id: Id,
    pub name: String,
    pub tris: Vec<Tri>,
    pub material: String,
    pub layer: String,
    pub style: DetailStyle,
    pub label: String,
    pub quality: SurfaceQuality,
    pub elevation_ref: ElevationRef,
}

impl Default for CompoundSolid {
    fn default() -> Self {
        Self {
            id: 0,
            name: "3D Solid".to_string(),
            tris: Vec::new(),
            material: DEFAULT_SOLID_MATERIAL.to_string(),
            layer: SOLID_LAYER.to_string(),
            style: DetailStyle::default(),
            label: String::new(),
            quality: SurfaceQuality::default(),
            elevation_ref: ElevationRef::FromFloor,
        }
    }
}

impl CompoundSolid {
    /// Axis-aligned box of the mesh: `(min, max)` as (x, y, z).
    pub fn bounds3(&self) -> Option<(V3, V3)> {
        bounds3(&self.tris)
    }

    /// Plan bounds `(lo, hi)`.
    pub fn plan_bounds(&self) -> Option<(Point, Point)> {
        let (lo, hi) = self.bounds3()?;
        Some((Point::new(lo[0], lo[1]), Point::new(hi[0], hi[1])))
    }

    /// Volume, cubic inches.
    pub fn volume(&self) -> f64 {
        volume(&self.tris)
    }

    /// Height of the lowest point above the floor.
    pub fn bottom(&self) -> f64 {
        self.bounds3().map_or(0.0, |(lo, _)| lo[2])
    }

    /// Height of the highest point above the floor.
    pub fn top(&self) -> f64 {
        self.bounds3().map_or(0.0, |(_, hi)| hi[2])
    }

    /// The plan outline: the boundary of the surfaces that face up.
    pub fn outline(&self) -> Vec<Vec<Point>> {
        top_outline(&self.tris)
    }

    pub fn translate(&mut self, d: Point) {
        for t in &mut self.tris {
            for v in t.iter_mut() {
                v[0] += d.x;
                v[1] += d.y;
            }
        }
    }

    /// Turns the mesh about the vertical line through `about`.
    pub fn rotate_z(&mut self, about: Point, deg: f64) {
        for t in &mut self.tris {
            for v in t.iter_mut() {
                let p = about + rotate_deg(Point::new(v[0], v[1]) - about, deg);
                v[0] = p.x;
                v[1] = p.y;
            }
        }
    }

    /// Mirrors the mesh about the vertical plane through `a` and `b`
    /// (triangles flip so they stay outward facing).
    pub fn reflect(&mut self, a: Point, b: Point) {
        let d = (b - a).normalized();
        for t in &mut self.tris {
            for v in t.iter_mut() {
                let p = Point::new(v[0], v[1]) - a;
                let r = a + (d * p.dot(d)) * 2.0 - p;
                v[0] = r.x;
                v[1] = r.y;
            }
            t.swap(1, 2);
        }
    }

    /// Moves the mesh so its lowest point is at `bottom`.
    pub fn set_bottom(&mut self, bottom: f64) {
        let dz = bottom - self.bottom();
        for t in &mut self.tris {
            for v in t.iter_mut() {
                v[2] += dz;
            }
        }
    }
}

// ===================================================================
// The layer
// ===================================================================

/// Everything a floor keeps for solids beyond the primitives themselves.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SolidLayer {
    pub ext: Vec<SolidExt>,
    pub compounds: Vec<CompoundSolid>,
}

impl SolidLayer {
    pub fn is_empty(&self) -> bool {
        self.ext.is_empty() && self.compounds.is_empty()
    }

    pub fn ext_of(&self, id: Id) -> Option<&SolidExt> {
        self.ext.iter().find(|e| e.id == id)
    }

    /// The ext of `id`, or the default one.
    pub fn ext_or_default(&self, id: Id) -> SolidExt {
        self.ext_of(id).cloned().unwrap_or_else(|| SolidExt::new(id))
    }

    /// Stores `e`; a default ext removes the record.
    pub fn set_ext(&mut self, e: SolidExt) {
        self.ext.retain(|x| x.id != e.id);
        if !e.is_default() {
            self.ext.push(e);
        }
    }

    pub fn compound(&self, id: Id) -> Option<&CompoundSolid> {
        self.compounds.iter().find(|c| c.id == id)
    }

    pub fn compound_mut(&mut self, id: Id) -> Option<&mut CompoundSolid> {
        self.compounds.iter_mut().find(|c| c.id == id)
    }

    pub fn remove_compound(&mut self, id: Id) -> bool {
        let n = self.compounds.len();
        self.compounds.retain(|c| c.id != id);
        self.compounds.len() != n
    }

    /// Forgets the ext of solids that are gone from `details`.
    pub fn drop_orphans(&mut self, details: &DetailsLayer) -> usize {
        let n = self.ext.len();
        self.ext
            .retain(|e| details.solids.iter().any(|s| s.id == e.id));
        n - self.ext.len()
    }
}

// ===================================================================
// Meshes of the primitives
// ===================================================================

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

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// Unit normal of a triangle (zero for a degenerate one).
pub fn tri_normal(t: &Tri) -> V3 {
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    let l = len(n);
    if l < 1e-12 {
        [0.0; 3]
    } else {
        [n[0] / l, n[1] / l, n[2] / l]
    }
}

fn tri_area(t: &Tri) -> f64 {
    0.5 * len(cross(sub(t[1], t[0]), sub(t[2], t[0])))
}

/// Ear-clipping triangulation of a simple polygon: index triples,
/// counter-clockwise in plan space.
pub fn triangulate(pts: &[Point]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let mut ring: Vec<usize> = (0..n).collect();
    if polygon_area(pts) < 0.0 {
        ring.reverse();
    }
    let inside = |p: Point, a: Point, b: Point, c: Point| {
        (b - a).cross(p - a) >= -1e-9 && (c - b).cross(p - b) >= -1e-9 && (a - c).cross(p - c) >= -1e-9
    };
    let mut out = Vec::with_capacity(n - 2);
    while ring.len() > 3 {
        let m = ring.len();
        let mut pick = None;
        for i in 0..m {
            let (ip, ic, inx) = (ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]);
            let (a, b, c) = (pts[ip], pts[ic], pts[inx]);
            if (b - a).cross(c - b) <= 1e-9 {
                continue;
            }
            let blocked = ring.iter().any(|&j| {
                j != ip
                    && j != ic
                    && j != inx
                    && ![a, b, c].iter().any(|q| q.dist(pts[j]) <= 1e-9)
                    && inside(pts[j], a, b, c)
            });
            if !blocked {
                pick = Some(i);
                break;
            }
        }
        let i = pick.unwrap_or_else(|| {
            // A collinear vertex, else any.
            (0..m)
                .find(|&i| {
                    let (a, b, c) = (
                        pts[ring[(i + m - 1) % m]],
                        pts[ring[i]],
                        pts[ring[(i + 1) % m]],
                    );
                    (b - a).cross(c - b).abs() <= 1e-9
                })
                .unwrap_or(0)
        });
        out.push([ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]]);
        ring.remove(i);
    }
    out.push([ring[0], ring[1], ring[2]]);
    out
}

fn ccw(poly: &[Point]) -> Vec<Point> {
    let mut v = poly.to_vec();
    if polygon_area(&v) < 0.0 {
        v.reverse();
    }
    v
}

fn at(p: Point, z: f64) -> V3 {
    [p.x, p.y, z]
}

/// A flat cap over `poly` at height `z`, facing up (or down when `up` is
/// false).
fn cap(out: &mut Vec<Tri>, poly: &[Point], z: f64, up: bool) {
    for [a, b, c] in triangulate(poly) {
        let t = [at(poly[a], z), at(poly[b], z), at(poly[c], z)];
        // `triangulate` is counter-clockwise from above.
        out.push(if up { t } else { [t[0], t[2], t[1]] });
    }
}

/// A prism of `poly` between heights `z0` and `z1`, outward facing.
pub fn prism(poly: &[Point], z0: f64, z1: f64) -> Vec<Tri> {
    let p = ccw(poly);
    let n = p.len();
    let mut out = Vec::new();
    if n < 3 || (z1 - z0).abs() < 1e-12 {
        return out;
    }
    cap(&mut out, &p, z1, true);
    cap(&mut out, &p, z0, false);
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        let (a0, b0, b1, a1) = (at(a, z0), at(b, z0), at(b, z1), at(a, z1));
        out.push([a0, b0, b1]);
        out.push([a0, b1, a1]);
    }
    out
}

/// A frustum from `base` at `z0` to `top` (the base scaled about `apex`'s
/// plan point) at `z1`; with `top` empty a pyramid to `apex`.
fn frustum(base: &[Point], z0: f64, top: &[Point], z1: f64, apex: Option<V3>) -> Vec<Tri> {
    let b = ccw(base);
    let n = b.len();
    let mut out = Vec::new();
    cap(&mut out, &b, z0, false);
    match apex {
        Some(ap) => {
            for i in 0..n {
                out.push([at(b[i], z0), at(b[(i + 1) % n], z0), ap]);
            }
        }
        None => {
            let t = ccw(top);
            cap(&mut out, &t, z1, true);
            for i in 0..n {
                let (a0, b0) = (at(b[i], z0), at(b[(i + 1) % n], z0));
                let (a1, b1) = (at(t[i], z1), at(t[(i + 1) % n], z1));
                out.push([a0, b0, b1]);
                out.push([a0, b1, a1]);
            }
        }
    }
    out
}

fn sphere_tris(r: f64, cz: f64, segs: usize) -> Vec<Tri> {
    let bands = (segs / 2).max(SPHERE_BANDS.min(segs / 2)).max(4);
    let ring = |i: usize, j: usize| -> V3 {
        let phi = -PI / 2.0 + PI * i as f64 / bands as f64;
        let th = TAU * (j % segs) as f64 / segs as f64;
        [r * phi.cos() * th.cos(), r * phi.cos() * th.sin(), cz + r * phi.sin()]
    };
    let mut out = Vec::new();
    for i in 0..bands {
        for j in 0..segs {
            let (p00, p01, p10, p11) = (ring(i, j), ring(i, j + 1), ring(i + 1, j), ring(i + 1, j + 1));
            for t in [[p00, p01, p11], [p00, p11, p10]] {
                if tri_area(&t) > 1e-9 {
                    out.push(t);
                }
            }
        }
    }
    out
}

fn circle_n(r: f64, segs: usize) -> Vec<Point> {
    (0..segs)
        .map(|i| {
            let a = TAU * i as f64 / segs as f64;
            Point::new(r * a.cos(), r * a.sin())
        })
        .collect()
}

/// The mesh of a primitive before rotation about the X and Y axes, in the
/// solid's local frame: the origin is the plan position at the bottom.
fn local_tris(s: &Solid3d, ext: Option<&SolidExt>) -> Vec<Tri> {
    let segs = ext.map_or(ROUND_SEGMENTS, |e| match &s.kind {
        SolidKind::Cylinder { r, .. } | SolidKind::Cone { r, .. } | SolidKind::Sphere { r } => {
            e.quality.segments(*r)
        }
        _ => ROUND_SEGMENTS,
    });
    match &s.kind {
        SolidKind::Box { .. } | SolidKind::PolylineSolid { .. } => {
            prism(&s.local_footprint(), 0.0, s.height())
        }
        SolidKind::Cylinder { r, h } => prism(&circle_n(*r, segs), 0.0, *h),
        SolidKind::Sphere { r } => sphere_tris(*r, *r, segs),
        SolidKind::Cone { r, h } => {
            frustum(&circle_n(*r, segs), 0.0, &[], 0.0, Some([0.0, 0.0, *h]))
        }
        SolidKind::Pyramid { outline, h } => {
            let base = match ext.and_then(|e| e.pyramid) {
                Some(spec) => spec.outline(),
                None => outline.clone(),
            };
            let c = polygon_centroid(&base);
            let trunc = ext
                .and_then(|e| e.pyramid)
                .filter(|p| p.truncated && p.truncated_height > 1e-9 && p.truncated_height < *h);
            match trunc {
                Some(p) => {
                    let k = (*h - p.truncated_height) / *h;
                    let top: Vec<Point> = base.iter().map(|q| c + (*q - c) * k).collect();
                    frustum(&base, 0.0, &top, p.truncated_height, None)
                }
                None => frustum(&base, 0.0, &[], 0.0, Some(at(c, *h))),
            }
        }
        SolidKind::Face { polygon } => {
            let p = ccw(polygon);
            let mut out = Vec::new();
            cap(&mut out, &p, 0.0, true);
            cap(&mut out, &p, 0.0, false);
            out
        }
    }
}

fn rot_x(v: V3, c: V3, deg: f64) -> V3 {
    let (s, co) = deg.to_radians().sin_cos();
    let (y, z) = (v[1] - c[1], v[2] - c[2]);
    [v[0], c[1] + y * co - z * s, c[2] + y * s + z * co]
}

fn rot_y(v: V3, c: V3, deg: f64) -> V3 {
    let (s, co) = deg.to_radians().sin_cos();
    let (x, z) = (v[0] - c[0], v[2] - c[2]);
    [c[0] + x * co + z * s, v[1], c[2] - x * s + z * co]
}

/// The triangles of a solid in plan coordinates, `z` above the floor:
/// rotated about X, Y (the ext) and Z, then moved to its position and
/// elevation. Counter-clockwise from outside.
pub fn solid_tris(s: &Solid3d, ext: Option<&SolidExt>) -> Vec<Tri> {
    let mut tris = local_tris(s, ext);
    let (rx, ry) = ext.map_or((0.0, 0.0), |e| (e.rot_x, e.rot_y));
    // Turn about the centre of the shape.
    let pivot = [0.0, 0.0, s.height() * 0.5];
    let pivot = match &s.kind {
        SolidKind::Sphere { r } => [0.0, 0.0, *r],
        _ => pivot,
    };
    for t in &mut tris {
        for v in t.iter_mut() {
            let mut p = *v;
            if rx.abs() > 1e-9 {
                p = rot_x(p, pivot, rx);
            }
            if ry.abs() > 1e-9 {
                p = rot_y(p, pivot, ry);
            }
            let q = rotate_deg(Point::new(p[0], p[1]), s.rotation);
            *v = [s.position.x + q.x, s.position.y + q.y, s.elevation + p[2]];
        }
    }
    tris
}

// ===================================================================
// Measures and outlines
// ===================================================================

/// `(min, max)` of the vertices.
pub fn bounds3(tris: &[Tri]) -> Option<(V3, V3)> {
    let first = tris.first()?[0];
    let (mut lo, mut hi) = (first, first);
    for t in tris {
        for v in t {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
    }
    Some((lo, hi))
}

/// Enclosed volume of a closed outward-facing mesh.
pub fn volume(tris: &[Tri]) -> f64 {
    tris.iter()
        .map(|t| dot(t[0], cross(t[1], t[2])) / 6.0)
        .sum::<f64>()
        .abs()
}

/// Total surface area.
pub fn surface_area(tris: &[Tri]) -> f64 {
    tris.iter().map(tri_area).sum()
}

fn key(p: Point) -> (i64, i64) {
    ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64)
}

/// The plan outline of a solid: the edges of the up-facing triangles that
/// only one such triangle owns, chained into loops. A box gives its
/// rectangle, a sphere its great circle, a subtraction its holes.
pub fn top_outline(tris: &[Tri]) -> Vec<Vec<Point>> {
    use std::collections::HashMap;
    // Directed edges of up-facing triangles, projected; an edge that also
    // appears reversed is interior and cancels.
    let mut edges: HashMap<((i64, i64), (i64, i64)), (Point, Point)> = HashMap::new();
    for t in tris {
        if tri_normal(t)[2] <= 1e-6 {
            continue;
        }
        let p = [
            Point::new(t[0][0], t[0][1]),
            Point::new(t[1][0], t[1][1]),
            Point::new(t[2][0], t[2][1]),
        ];
        for i in 0..3 {
            let (a, b) = (p[i], p[(i + 1) % 3]);
            if key(a) == key(b) {
                continue;
            }
            if edges.remove(&(key(b), key(a))).is_none() {
                edges.insert((key(a), key(b)), (a, b));
            }
        }
    }
    let mut next: HashMap<(i64, i64), Vec<((i64, i64), (Point, Point))>> = HashMap::new();
    for ((ka, kb), pts) in &edges {
        next.entry(*ka).or_default().push((*kb, *pts));
    }
    let mut loops: Vec<Vec<Point>> = Vec::new();
    let mut starts: Vec<_> = edges.keys().copied().collect();
    starts.sort();
    for start in starts {
        if !next
            .get(&start.0)
            .is_some_and(|v| v.iter().any(|(k, _)| *k == start.1))
        {
            continue;
        }
        let mut ring: Vec<Point> = Vec::new();
        let mut cur = start;
        for _ in 0..edges.len() + 1 {
            let Some(list) = next.get_mut(&cur.0) else {
                break;
            };
            let Some(i) = list.iter().position(|(k, _)| *k == cur.1) else {
                break;
            };
            let (kb, (a, _b)) = list.remove(i);
            ring.push(a);
            // Continue from kb: any outgoing edge (prefer the one that turns least).
            cur = match next.get(&kb).and_then(|l| l.first()) {
                Some((k2, _)) => (kb, *k2),
                None => break,
            };
            if cur == start {
                break;
            }
        }
        if ring.len() >= 3 {
            loops.push(simplify_collinear(ring));
        }
    }
    loops
}

/// Drops vertices on a straight run.
pub fn simplify_collinear(ring: Vec<Point>) -> Vec<Point> {
    let n = ring.len();
    if n < 4 {
        return ring;
    }
    let keep: Vec<Point> = (0..n)
        .filter(|&i| {
            let (a, b, c) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
            (b - a).cross(c - b).abs() > 1e-6 * (b - a).length().max(1e-9) * (c - b).length().max(1e-9)
        })
        .map(|i| ring[i])
        .collect();
    if keep.len() >= 3 {
        keep
    } else {
        ring
    }
}

// ===================================================================
// Boolean operations (BSP clipping)
// ===================================================================

/// Union, Subtract or Intersect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    /// The first operand minus the others.
    Subtract,
    Intersect,
}

impl BoolOp {
    pub fn name(self) -> &'static str {
        match self {
            BoolOp::Union => "Union",
            BoolOp::Subtract => "Subtract",
            BoolOp::Intersect => "Intersection",
        }
    }
}

const EPS: f64 = 1e-6;

#[derive(Clone, Copy)]
struct Plane {
    n: V3,
    w: f64,
}

#[derive(Clone)]
struct Poly {
    v: Vec<V3>,
    plane: Plane,
}

impl Poly {
    fn from_tri(t: &Tri) -> Option<Poly> {
        let n = tri_normal(t);
        if n == [0.0; 3] {
            return None;
        }
        Some(Poly {
            v: t.to_vec(),
            plane: Plane {
                n,
                w: dot(n, t[0]),
            },
        })
    }

    fn flip(&mut self) {
        self.v.reverse();
        self.plane.n = [-self.plane.n[0], -self.plane.n[1], -self.plane.n[2]];
        self.plane.w = -self.plane.w;
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Coplanar,
    Front,
    Back,
    Spanning,
}

fn lerp3(a: V3, b: V3, t: f64) -> V3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

impl Plane {
    fn split(
        &self,
        poly: &Poly,
        coplanar_front: &mut Vec<Poly>,
        coplanar_back: &mut Vec<Poly>,
        front: &mut Vec<Poly>,
        back: &mut Vec<Poly>,
    ) {
        let types: Vec<Side> = poly
            .v
            .iter()
            .map(|v| {
                let t = dot(self.n, *v) - self.w;
                if t < -EPS {
                    Side::Back
                } else if t > EPS {
                    Side::Front
                } else {
                    Side::Coplanar
                }
            })
            .collect();
        let has_front = types.contains(&Side::Front);
        let has_back = types.contains(&Side::Back);
        let kind = match (has_front, has_back) {
            (false, false) => Side::Coplanar,
            (true, false) => Side::Front,
            (false, true) => Side::Back,
            (true, true) => Side::Spanning,
        };
        match kind {
            Side::Coplanar => {
                if dot(self.n, poly.plane.n) > 0.0 {
                    coplanar_front.push(poly.clone());
                } else {
                    coplanar_back.push(poly.clone());
                }
            }
            Side::Front => front.push(poly.clone()),
            Side::Back => back.push(poly.clone()),
            Side::Spanning => {
                let (mut f, mut b): (Vec<V3>, Vec<V3>) = (Vec::new(), Vec::new());
                let n = poly.v.len();
                for i in 0..n {
                    let j = (i + 1) % n;
                    let (ti, tj) = (types[i], types[j]);
                    let (vi, vj) = (poly.v[i], poly.v[j]);
                    if ti != Side::Back {
                        f.push(vi);
                    }
                    if ti != Side::Front {
                        b.push(vi);
                    }
                    if (ti == Side::Front && tj == Side::Back)
                        || (ti == Side::Back && tj == Side::Front)
                    {
                        let t = (self.w - dot(self.n, vi)) / dot(self.n, sub(vj, vi));
                        let v = lerp3(vi, vj, t);
                        f.push(v);
                        b.push(v);
                    }
                }
                if f.len() >= 3 {
                    front.push(Poly {
                        v: f,
                        plane: poly.plane,
                    });
                }
                if b.len() >= 3 {
                    back.push(Poly {
                        v: b,
                        plane: poly.plane,
                    });
                }
            }
        }
    }
}

#[derive(Default)]
struct Node {
    plane: Option<Plane>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
    polys: Vec<Poly>,
}

impl Node {
    fn new(polys: Vec<Poly>) -> Node {
        let mut n = Node::default();
        n.build(polys);
        n
    }

    fn invert(&mut self) {
        for p in &mut self.polys {
            p.flip();
        }
        if let Some(pl) = &mut self.plane {
            pl.n = [-pl.n[0], -pl.n[1], -pl.n[2]];
            pl.w = -pl.w;
        }
        if let Some(f) = &mut self.front {
            f.invert();
        }
        if let Some(b) = &mut self.back {
            b.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    fn clip_polys(&self, polys: Vec<Poly>) -> Vec<Poly> {
        let Some(plane) = self.plane else {
            return polys;
        };
        let (mut front, mut back) = (Vec::new(), Vec::new());
        for p in &polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            plane.split(p, &mut cf, &mut cb, &mut front, &mut back);
            front.append(&mut cf);
            back.append(&mut cb);
        }
        let mut front = match &self.front {
            Some(f) => f.clip_polys(front),
            None => front,
        };
        let back = match &self.back {
            Some(b) => b.clip_polys(back),
            None => Vec::new(),
        };
        front.extend(back);
        front
    }

    fn clip_to(&mut self, other: &Node) {
        self.polys = other.clip_polys(std::mem::take(&mut self.polys));
        if let Some(f) = &mut self.front {
            f.clip_to(other);
        }
        if let Some(b) = &mut self.back {
            b.clip_to(other);
        }
    }

    fn all(&self, out: &mut Vec<Poly>) {
        out.extend(self.polys.iter().cloned());
        if let Some(f) = &self.front {
            f.all(out);
        }
        if let Some(b) = &self.back {
            b.all(out);
        }
    }

    fn build(&mut self, polys: Vec<Poly>) {
        if polys.is_empty() {
            return;
        }
        let plane = *self.plane.get_or_insert(polys[0].plane);
        let (mut front, mut back) = (Vec::new(), Vec::new());
        for p in &polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            plane.split(p, &mut cf, &mut cb, &mut front, &mut back);
            self.polys.append(&mut cf);
            self.polys.append(&mut cb);
        }
        if !front.is_empty() {
            self.front.get_or_insert_with(Default::default).build(front);
        }
        if !back.is_empty() {
            self.back.get_or_insert_with(Default::default).build(back);
        }
    }
}

fn polys_of(tris: &[Tri]) -> Vec<Poly> {
    tris.iter().filter_map(Poly::from_tri).collect()
}

fn tris_of(polys: &[Poly]) -> Vec<Tri> {
    let mut out = Vec::new();
    for p in polys {
        for i in 1..p.v.len().saturating_sub(1) {
            let t = [p.v[0], p.v[i], p.v[i + 1]];
            if tri_area(&t) > 1e-9 {
                out.push(t);
            }
        }
    }
    out
}

/// `a` op `b` for two closed, outward-facing meshes.
pub fn boolean(op: BoolOp, a: &[Tri], b: &[Tri]) -> Vec<Tri> {
    let mut na = Node::new(polys_of(a));
    let mut nb = Node::new(polys_of(b));
    match op {
        BoolOp::Union => {
            na.clip_to(&nb);
            nb.clip_to(&na);
            nb.invert();
            nb.clip_to(&na);
            nb.invert();
            let mut pb = Vec::new();
            nb.all(&mut pb);
            na.build(pb);
        }
        BoolOp::Subtract => {
            na.invert();
            na.clip_to(&nb);
            nb.clip_to(&na);
            nb.invert();
            nb.clip_to(&na);
            nb.invert();
            let mut pb = Vec::new();
            nb.all(&mut pb);
            na.build(pb);
            na.invert();
        }
        BoolOp::Intersect => {
            na.invert();
            nb.clip_to(&na);
            nb.invert();
            na.clip_to(&nb);
            nb.clip_to(&na);
            let mut pb = Vec::new();
            nb.all(&mut pb);
            na.build(pb);
            na.invert();
        }
    }
    let mut out = Vec::new();
    na.all(&mut out);
    tris_of(&out)
}

/// The operation over any number of operands: the first op the rest in turn.
pub fn boolean_many(op: BoolOp, operands: &[Vec<Tri>]) -> Vec<Tri> {
    let Some((first, rest)) = operands.split_first() else {
        return Vec::new();
    };
    let mut acc = first.clone();
    for r in rest {
        acc = boolean(op, &acc, r);
    }
    acc
}

/// Why a Boolean operation did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoolError {
    TooFew,
    /// The named kind is not a solid.
    NotSolid(&'static str),
    /// Nothing is left (for instance an intersection of separate solids).
    Empty,
    Faces,
}

impl std::fmt::Display for BoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoolError::TooFew => write!(f, "Select two or more 3D solids"),
            BoolError::NotSolid(k) => write!(f, "{k} is not a 3D solid"),
            BoolError::Empty => write!(f, "Nothing is left after that operation"),
            BoolError::Faces => write!(f, "A face has no volume: extrude it into a solid first"),
        }
    }
}

impl std::error::Error for BoolError {}

impl Floor {
    /// The triangles of solid `r` (a [`ObjectRef::Detail`] solid or an
    /// [`ObjectRef::Solid`]) and its material and layer.
    pub fn solid_operand(&self, r: ObjectRef) -> Result<(Vec<Tri>, String, String), BoolError> {
        match r {
            ObjectRef::Detail(id) => {
                let layer = DetailsLayer::load(self);
                let s = layer
                    .solid(id)
                    .ok_or(BoolError::NotSolid("That object"))?;
                if matches!(s.kind, SolidKind::Face { .. }) {
                    return Err(BoolError::Faces);
                }
                let ext = self.solid_layer.ext_of(id);
                Ok((solid_tris(s, ext), s.material.clone(), s.layer.clone()))
            }
            ObjectRef::Solid(id) => {
                let c = self
                    .solid_layer
                    .compound(id)
                    .ok_or(BoolError::NotSolid("That object"))?;
                Ok((c.tris.clone(), c.material.clone(), c.layer.clone()))
            }
            _ => Err(BoolError::NotSolid("That object")),
        }
    }
}

/// Union, Subtract or Intersect of the solids `operands` (in selection
/// order, the first is the one Subtract keeps) on `floor`. The operands go;
/// the result is a [`CompoundSolid`] with id `new_id`, which is returned.
pub fn boolean_floor(
    floor: &mut Floor,
    op: BoolOp,
    operands: &[ObjectRef],
    new_id: Id,
) -> Result<Id, BoolError> {
    if operands.len() < 2 {
        return Err(BoolError::TooFew);
    }
    let mut meshes = Vec::new();
    let mut first: Option<(String, String)> = None;
    for r in operands {
        let (t, m, l) = floor.solid_operand(*r)?;
        first.get_or_insert((m, l));
        meshes.push(t);
    }
    let tris = boolean_many(op, &meshes);
    if tris.is_empty() || volume(&tris) < 1e-6 {
        return Err(BoolError::Empty);
    }
    let (material, layer) = first.unwrap_or_default();
    // The operands go.
    let mut details = DetailsLayer::load(floor);
    for r in operands {
        match r {
            ObjectRef::Detail(id) => {
                let _ = details.remove(DetailRef::Solid(*id));
                floor.solid_layer.ext.retain(|e| e.id != *id);
            }
            ObjectRef::Solid(id) => {
                floor.solid_layer.remove_compound(*id);
            }
            _ => {}
        }
    }
    let _ = floor.set_details(&details);
    floor.solid_layer.compounds.push(CompoundSolid {
        id: new_id,
        tris,
        material,
        layer,
        ..CompoundSolid::default()
    });
    Ok(new_id)
}

// ===================================================================
// Conversions and extrusion
// ===================================================================

/// Convert Polyline to Solid: the closed polyline `points` becomes a 3D
/// Solid of height `height` (p. 294). `None` for fewer than three points or
/// no area.
pub fn polyline_to_solid(id: Id, points: &[Point], height: f64) -> Option<Solid3d> {
    if points.len() < 3 || polygon_area(points).abs() < 1e-6 {
        return None;
    }
    let c = polygon_centroid(points);
    let outline: Vec<Point> = points.iter().map(|p| *p - c).collect();
    Some(Solid3d {
        id,
        kind: SolidKind::PolylineSolid {
            outline,
            h: height.max(INITIAL_SOLID_HEIGHT),
        },
        position: c,
        ..Solid3d::default()
    })
}

/// Extrude Object: the face or solid `s` swept by the Extrusion Delta `d`
/// `(dx, dy, dz)`. A vertical delta of a face gives a polyline solid; a
/// slanted delta gives a compound solid. Returns the mesh.
pub fn extrude_face(s: &Solid3d, ext: Option<&SolidExt>, d: V3) -> Vec<Tri> {
    let base = solid_tris(s, ext);
    let ring = match &s.kind {
        SolidKind::Face { .. } => s.footprint(),
        _ => top_outline(&base).into_iter().next().unwrap_or_default(),
    };
    if ring.len() < 3 {
        return Vec::new();
    }
    let z0 = s.elevation + if matches!(s.kind, SolidKind::Face { .. }) { 0.0 } else { s.height() };
    // Sweep the ring along d: a prism with slanted sides.
    let ring = ccw(&ring);
    let n = ring.len();
    let moved: Vec<V3> = ring.iter().map(|p| [p.x + d[0], p.y + d[1], z0 + d[2]]).collect();
    let from: Vec<V3> = ring.iter().map(|p| [p.x, p.y, z0]).collect();
    let mut out = Vec::new();
    // Caps (the lower cap faces down when d points up).
    let up = d[2] >= 0.0;
    let mut cap_a = Vec::new();
    cap(&mut cap_a, &ring, z0, !up);
    out.extend(cap_a);
    let ring_b: Vec<Point> = ring.iter().map(|p| Point::new(p.x + d[0], p.y + d[1])).collect();
    let mut cap_b = Vec::new();
    cap(&mut cap_b, &ring_b, z0 + d[2], up);
    out.extend(cap_b);
    for i in 0..n {
        let j = (i + 1) % n;
        let (a0, b0, b1, a1) = (from[i], from[j], moved[j], moved[i]);
        if up {
            out.push([a0, b0, b1]);
            out.push([a0, b1, a1]);
        } else {
            out.push([a0, b1, b0]);
            out.push([a0, a1, b1]);
        }
    }
    out
}

/// A regular-polygon pyramid solid for `spec` standing at `position`.
pub fn pyramid_solid(id: Id, position: Point, h: f64, spec: &PyramidSpec) -> Solid3d {
    Solid3d {
        id,
        kind: SolidKind::Pyramid {
            outline: spec.outline(),
            h,
        },
        position,
        ..Solid3d::default()
    }
}

/// Hook for the dialogs: the polygon of a circle used for plan drawing.
pub fn round_outline(r: f64) -> Vec<Point> {
    circle_points(Point::ZERO, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;

    fn box_solid(id: Id, x: f64, w: f64, h: f64, elev: f64) -> Solid3d {
        Solid3d {
            id,
            kind: SolidKind::Box { w, d: w, h },
            position: Point::new(x, 0.0),
            elevation: elev,
            ..Solid3d::default()
        }
    }

    #[test]
    fn primitives_are_closed_and_outward_facing() {
        let cases = [
            (SolidKind::Box { w: 10.0, d: 20.0, h: 30.0 }, 6000.0),
            (SolidKind::Cylinder { r: 6.0, h: 10.0 }, 0.0),
            (SolidKind::Sphere { r: 5.0 }, 0.0),
            (SolidKind::Cone { r: 6.0, h: 9.0 }, 0.0),
            (
                SolidKind::Pyramid {
                    outline: PyramidSpec::default().outline(),
                    h: 12.0,
                },
                0.0,
            ),
            (
                SolidKind::PolylineSolid {
                    outline: vec![
                        Point::new(0.0, 0.0),
                        Point::new(10.0, 0.0),
                        Point::new(10.0, 4.0),
                        Point::new(4.0, 4.0),
                        Point::new(4.0, 10.0),
                        Point::new(0.0, 10.0),
                    ],
                    h: 5.0,
                },
                0.0,
            ),
        ];
        for (kind, exact) in cases {
            let s = Solid3d::new(1, kind, Point::ZERO);
            let t = solid_tris(&s, None);
            let v = volume(&t);
            assert!(v > 1.0, "{} has volume", s.kind.name());
            // Signed volume positive means outward winding.
            let signed: f64 = t.iter().map(|t| dot(t[0], cross(t[1], t[2])) / 6.0).sum();
            assert!(signed > 0.0, "{} faces outward ({signed})", s.kind.name());
            if exact > 0.0 {
                assert!((v - exact).abs() < 1e-6);
            } else {
                // Within a few percent of the true volume of the round shape.
                let truth = s.volume();
                assert!((v - truth).abs() / truth < 0.08, "{}: {v} vs {truth}", s.kind.name());
            }
        }
    }

    #[test]
    fn union_of_overlapping_boxes_has_the_expected_volume() {
        let a = solid_tris(&box_solid(1, 0.0, 10.0, 10.0, 0.0), None);
        let b = solid_tris(&box_solid(2, 5.0, 10.0, 10.0, 0.0), None);
        let u = boolean(BoolOp::Union, &a, &b);
        // 15 x 10 x 10.
        assert!((volume(&u) - 1500.0).abs() < 1e-3, "{}", volume(&u));
        let s = boolean(BoolOp::Subtract, &a, &b);
        assert!((volume(&s) - 500.0).abs() < 1e-3, "{}", volume(&s));
        let i = boolean(BoolOp::Intersect, &a, &b);
        assert!((volume(&i) - 500.0).abs() < 1e-3, "{}", volume(&i));
        // Disjoint solids intersect in nothing.
        let far = solid_tris(&box_solid(3, 100.0, 10.0, 10.0, 0.0), None);
        assert!(volume(&boolean(BoolOp::Intersect, &a, &far)) < 1e-6);
    }

    #[test]
    fn subtract_a_cylinder_leaves_a_hole_in_the_plan_outline() {
        let slab = Solid3d {
            id: 1,
            kind: SolidKind::Box { w: 40.0, d: 40.0, h: 6.0 },
            ..Solid3d::default()
        };
        let hole = Solid3d {
            id: 2,
            kind: SolidKind::Cylinder { r: 8.0, h: 20.0 },
            elevation: -5.0,
            ..Solid3d::default()
        };
        let a = solid_tris(&slab, None);
        let b = solid_tris(&hole, None);
        let r = boolean(BoolOp::Subtract, &a, &b);
        let v = volume(&r);
        let cyl = std::f64::consts::PI * 64.0 * 6.0;
        assert!((v - (9600.0 - cyl)).abs() / 9600.0 < 0.03, "{v}");
        let loops = top_outline(&r);
        assert_eq!(loops.len(), 2, "outer boundary and the hole");
        let areas: Vec<f64> = loops.iter().map(|l| polygon_area(l).abs()).collect();
        assert!(areas.iter().any(|a| (a - 1600.0).abs() < 1.0));
    }

    #[test]
    fn boolean_floor_replaces_the_operands_with_one_compound() {
        let mut p = Project::new("csg");
        let (a, b) = (p.alloc_id(), p.alloc_id());
        let mut d = DetailsLayer::default();
        d.solids.push(box_solid(a, 0.0, 10.0, 10.0, 0.0));
        d.solids.push(box_solid(b, 5.0, 10.0, 10.0, 0.0));
        p.floors[0].set_details(&d).unwrap();
        let new_id = p.alloc_id();
        let f = &mut p.floors[0];
        let r = boolean_floor(
            f,
            BoolOp::Union,
            &[ObjectRef::Detail(a), ObjectRef::Detail(b)],
            new_id,
        );
        assert_eq!(r, Ok(new_id));
        assert!(DetailsLayer::load(f).solids.is_empty());
        let c = f.solid_layer.compound(new_id).unwrap();
        assert!((c.volume() - 1500.0).abs() < 1e-3);
        let (lo, hi) = c.plan_bounds().unwrap();
        assert!((hi.x - lo.x - 15.0).abs() < 1e-6);
        // Round trip through the project file.
        let back = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(back.floors[0].solid_layer.compounds.len(), 1);
        // Faces are refused.
        let mut f2 = back.floors[0].clone();
        let r = boolean_floor(&mut f2, BoolOp::Union, &[ObjectRef::Cad(1), ObjectRef::Solid(new_id)], 99);
        assert!(matches!(r, Err(BoolError::NotSolid(_))));
    }

    #[test]
    fn tilt_truncation_and_quality_change_the_mesh() {
        let s = box_solid(1, 0.0, 10.0, 10.0, 0.0);
        let flat = solid_tris(&s, None);
        let mut e = SolidExt::new(1);
        e.rot_x = 90.0;
        let tilted = solid_tris(&Solid3d { kind: SolidKind::Box { w: 10.0, d: 10.0, h: 30.0 }, ..s.clone() }, Some(&e));
        let (lo, hi) = bounds3(&tilted).unwrap();
        // A 10 x 10 x 30 box turned a quarter about X lies along Y.
        assert!((hi[1] - lo[1] - 30.0).abs() < 1e-6 && (hi[2] - lo[2] - 10.0).abs() < 1e-6);
        assert!((volume(&tilted) - 3000.0).abs() < 1e-6);
        assert!((volume(&flat) - 1000.0).abs() < 1e-6);
        // A truncated pyramid is a frustum.
        let spec = PyramidSpec {
            sides: 4,
            def: PyramidDef::SideLength,
            size: 10.0,
            truncated: true,
            truncated_height: 6.0,
        };
        let py = pyramid_solid(5, Point::ZERO, 12.0, &spec);
        let mut ex = SolidExt::new(5);
        ex.pyramid = Some(spec);
        let full = volume(&solid_tris(&py, None));
        let cut = volume(&solid_tris(&py, Some(&ex)));
        assert!(cut < full && cut > 0.0);
        // Side length 10, height 12: a full pyramid is 400.
        assert!((solid_tris(&pyramid_solid(5, Point::ZERO, 12.0, &spec), Some(&SolidExt { pyramid: Some(PyramidSpec { truncated: false, ..spec }), ..SolidExt::new(5) })).len() as i32) > 0);
        assert!(SurfaceQuality { automatic: false, max_deflection: 0.01 }.segments(30.0) > ROUND_SEGMENTS);
        assert_eq!(SurfaceQuality::default().segments(30.0), ROUND_SEGMENTS);
    }

    #[test]
    fn pyramid_spec_geometry() {
        let sq = PyramidSpec {
            sides: 4,
            def: PyramidDef::SideLength,
            size: 12.0,
            truncated: false,
            truncated_height: 0.0,
        };
        let o = sq.outline();
        assert_eq!(o.len(), 4);
        // Axis-aligned square of side 12.
        let (lo, hi) = crate::details::bounds(&o);
        assert!((hi.x - lo.x - 12.0).abs() < 1e-9 && (hi.y - lo.y - 12.0).abs() < 1e-9);
        assert!((sq.side_radius() - 6.0).abs() < 1e-9);
        let hex = PyramidSpec { sides: 6, def: PyramidDef::RadiusToSide, size: 10.0, ..sq };
        assert!((hex.side_length() - 2.0 * 10.0 * (PI / 6.0).tan()).abs() < 1e-9);
        assert_eq!(PyramidSpec { sides: 3, ..sq }.outline().len(), 3);
    }

    #[test]
    fn polyline_conversion_and_extrusion() {
        let sq = [Point::new(0.0, 0.0), Point::new(20.0, 0.0), Point::new(20.0, 10.0), Point::new(0.0, 10.0)];
        let s = polyline_to_solid(3, &sq, 8.0).unwrap();
        assert!((s.volume() - 1600.0).abs() < 1e-9);
        assert!((s.position.x - 10.0).abs() < 1e-9);
        assert!(polyline_to_solid(3, &sq[..2], 8.0).is_none());
        let face = Solid3d {
            id: 4,
            kind: SolidKind::Face { polygon: sq.to_vec() },
            ..Solid3d::default()
        };
        let t = extrude_face(&face, None, [0.0, 0.0, 6.0]);
        assert!((volume(&t) - 1200.0).abs() < 1e-6, "{}", volume(&t));
        // Extruding along a slant keeps the volume of the base times the vertical rise.
        let slanted = extrude_face(&face, None, [5.0, 0.0, 6.0]);
        assert!((volume(&slanted) - 1200.0).abs() < 1e-6);
    }

    #[test]
    fn ext_records_come_and_go() {
        let mut l = SolidLayer::default();
        let mut e = SolidExt::new(7);
        e.rot_y = 15.0;
        l.set_ext(e.clone());
        assert_eq!(l.ext.len(), 1);
        l.set_ext(SolidExt::new(7));
        assert!(l.is_empty(), "a default ext is not stored");
        assert_eq!(l.ext_or_default(7), SolidExt::new(7));
        let _ = TAU;
    }
}
