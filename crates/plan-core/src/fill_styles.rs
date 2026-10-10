//! The shared Fill Style (CAD-69, CAD-73..CAD-77, CAD-72; manual pp. 221-226)
//! and the plan's style book: library line styles, fill styles and custom
//! patterns assigned to objects, the User Catalog entries, and the Poché
//! switch.
//!
//! A [`FillStyle`] is the one record every Fill Style panel edits: a pattern
//! type (Use Layer, a system pattern or a library pattern), its scale, row
//! offset, offsets and angle, line weight, colour source (single, layer or
//! background) with transparency, an optional pattern background and
//! gradients. [`fill_geometry`] turns a style and a polygon into the lines
//! and dots a renderer draws, so plan, section and layout share one tiler.
//! Lengths are plan inches.
//!
//! Where a style is applied is a [`FillTarget`] in [`StyleBook::fill_assign`]:
//! the plan is the model, so old files (whose CAD, room, slab and wall layer
//! fills sit in their own fields) stay readable and a style assigned here
//! simply overrides them. [`FillStyle::from_legacy`] reads the old fields in
//! the shared type.

use crate::cad::FillAttr;
use crate::geometry::{point_in_polygon, Point};
use crate::line_styles::{LineStyleDef, LineStyleLibrary, LineTarget};
use crate::model::Id;
use crate::patterns::{CustomPattern, InfiniteLine};
use crate::walls::WallClass;
use crate::Wall;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Colour, gradient
// ---------------------------------------------------------------------------

/// Where a colour comes from (Single Color, Use Layer Color, Use Background
/// Color).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorSource {
    Layer,
    Single([u8; 3]),
    Background,
}

impl ColorSource {
    pub fn resolve(self, layer: [u8; 3], background: [u8; 3]) -> [u8; 3] {
        match self {
            ColorSource::Layer => layer,
            ColorSource::Single(c) => c,
            ColorSource::Background => background,
        }
    }
}

/// Linear (a line) or radial (round a centre).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GradientKind {
    Linear,
    Radial,
}

/// One colour of a gradient.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorPoint {
    pub color: [u8; 3],
    /// 0 is opaque, 1 invisible.
    pub transparency: f64,
    /// 0 to 1: 0 is the far right of a linear gradient (the centre of a
    /// radial one), 1 the far left (the outside).
    pub position: f64,
}

/// The Fill Gradient dialog's record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Gradient {
    pub kind: GradientKind,
    /// Direction of a linear gradient, degrees.
    pub angle_deg: f64,
    pub points: Vec<ColorPoint>,
}

impl Default for Gradient {
    fn default() -> Self {
        Self {
            kind: GradientKind::Linear,
            angle_deg: 0.0,
            points: vec![
                ColorPoint {
                    color: [255, 255, 255],
                    transparency: 0.0,
                    position: 0.0,
                },
                ColorPoint {
                    color: [0, 0, 0],
                    transparency: 0.0,
                    position: 1.0,
                },
            ],
        }
    }
}

impl Gradient {
    /// Reset: the two-colour default.
    pub fn reset(&mut self) {
        *self = Gradient {
            kind: self.kind,
            angle_deg: self.angle_deg,
            ..Gradient::default()
        };
    }

    /// Add Color: a new point halfway along the widest gap.
    pub fn add_color(&mut self) {
        let mut pts = self.points.clone();
        pts.sort_by(|a, b| a.position.total_cmp(&b.position));
        let at = pts
            .windows(2)
            .max_by(|a, b| {
                (a[1].position - a[0].position).total_cmp(&(b[1].position - b[0].position))
            })
            .map(|w| {
                let t = 0.5;
                ColorPoint {
                    color: std::array::from_fn(|i| {
                        (f64::from(w[0].color[i]) * (1.0 - t) + f64::from(w[1].color[i]) * t)
                            .round() as u8
                    }),
                    transparency: (w[0].transparency + w[1].transparency) / 2.0,
                    position: (w[0].position + w[1].position) / 2.0,
                }
            });
        if let Some(p) = at {
            self.points.push(p);
        }
    }

    /// Remove Color: only with three or more colours in the list.
    pub fn remove_color(&mut self, i: usize) -> bool {
        if self.points.len() >= 3 && i < self.points.len() {
            self.points.remove(i);
            true
        } else {
            false
        }
    }

    /// The colour and transparency at `t` (0..1, see [`ColorPoint::position`]).
    pub fn sample(&self, t: f64) -> ([u8; 3], f64) {
        let mut pts = self.points.clone();
        pts.sort_by(|a, b| a.position.total_cmp(&b.position));
        let Some(first) = pts.first() else {
            return ([0, 0, 0], 0.0);
        };
        if t <= first.position {
            return (first.color, first.transparency);
        }
        for w in pts.windows(2) {
            if t <= w[1].position {
                let span = (w[1].position - w[0].position).max(1e-9);
                let u = ((t - w[0].position) / span).clamp(0.0, 1.0);
                let c = std::array::from_fn(|i| {
                    (f64::from(w[0].color[i]) * (1.0 - u) + f64::from(w[1].color[i]) * u).round()
                        as u8
                });
                return (c, w[0].transparency * (1.0 - u) + w[1].transparency * u);
            }
        }
        let last = pts[pts.len() - 1];
        (last.color, last.transparency)
    }

    /// The gradient position of plan point `p` in a fill whose bounds are
    /// `lo`..`hi`.
    pub fn position_at(&self, p: Point, lo: Point, hi: Point) -> f64 {
        match self.kind {
            GradientKind::Linear => {
                let a = self.angle_deg.to_radians();
                let u = Point::new(a.cos(), a.sin());
                let corners = [lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)];
                let (mut mn, mut mx) = (f64::INFINITY, f64::NEG_INFINITY);
                for c in corners {
                    let d = c.dot(u);
                    mn = mn.min(d);
                    mx = mx.max(d);
                }
                // 0 at the far right (+u), 1 at the far left.
                1.0 - ((p.dot(u) - mn) / (mx - mn).max(1e-9)).clamp(0.0, 1.0)
            }
            GradientKind::Radial => {
                let c = Point::lerp(lo, hi, 0.5);
                let r = (hi - lo).length() / 2.0;
                (p.dist(c) / r.max(1e-9)).clamp(0.0, 1.0)
            }
        }
    }
}

/// The pattern background (Background checkbox).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Background {
    pub color: ColorSource,
    pub transparency: f64,
    pub gradient: Option<Gradient>,
}

impl Default for Background {
    fn default() -> Self {
        Self {
            color: ColorSource::Background,
            transparency: 0.0,
            gradient: None,
        }
    }
}

// ---------------------------------------------------------------------------
// The style
// ---------------------------------------------------------------------------

/// The system patterns of the Pattern Type list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemPattern {
    Solid,
    /// Parallel lines `width` apart.
    Lines,
    CrossHatch,
    Grid,
    GridOffset,
    GridStep,
    Brick,
    Herringbone,
    Us,
    Dots,
    Concrete,
    Sand,
}

impl SystemPattern {
    pub const ALL: [SystemPattern; 12] = [
        SystemPattern::Solid,
        SystemPattern::Lines,
        SystemPattern::CrossHatch,
        SystemPattern::Grid,
        SystemPattern::GridOffset,
        SystemPattern::GridStep,
        SystemPattern::Brick,
        SystemPattern::Herringbone,
        SystemPattern::Us,
        SystemPattern::Dots,
        SystemPattern::Concrete,
        SystemPattern::Sand,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SystemPattern::Solid => "Solid",
            SystemPattern::Lines => "Lines",
            SystemPattern::CrossHatch => "Cross Hatch",
            SystemPattern::Grid => "Grid",
            SystemPattern::GridOffset => "Grid, Offset",
            SystemPattern::GridStep => "Grid, Step",
            SystemPattern::Brick => "Brick",
            SystemPattern::Herringbone => "Herringbone",
            SystemPattern::Us => "U's",
            SystemPattern::Dots => "Dots",
            SystemPattern::Concrete => "Concrete",
            SystemPattern::Sand => "Sand",
        }
    }

    /// Row Offset applies (Grid, Offset and Grid, Step).
    pub fn has_row_offset(self) -> bool {
        matches!(self, SystemPattern::GridOffset | SystemPattern::GridStep)
    }

    /// Height applies (the Grid types, Herringbone and U's).
    pub fn has_height(self) -> bool {
        matches!(
            self,
            SystemPattern::Grid
                | SystemPattern::GridOffset
                | SystemPattern::GridStep
                | SystemPattern::Brick
                | SystemPattern::Herringbone
                | SystemPattern::Us
                | SystemPattern::Dots
        )
    }

    /// Width is a single Spacing (Concrete and Sand, and the line sets).
    pub fn width_is_spacing(self) -> bool {
        matches!(
            self,
            SystemPattern::Concrete
                | SystemPattern::Sand
                | SystemPattern::Lines
                | SystemPattern::CrossHatch
        )
    }
}

/// The Type drop-down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternType {
    /// The fill style of the object's layer.
    UseLayer,
    System(SystemPattern),
    /// A custom pattern of the file or the User Catalog, by name.
    Library(String),
}

impl PatternType {
    pub fn label(&self) -> String {
        match self {
            PatternType::UseLayer => "Use Layer".into(),
            PatternType::System(s) => s.label().into(),
            PatternType::Library(n) => n.clone(),
        }
    }
}

/// One fill style: everything a Fill Style panel sets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FillStyle {
    pub pattern: PatternType,
    /// Pattern width (a spacing for lines, Concrete and Sand), plan inches.
    pub width: f64,
    /// Pattern height, plan inches.
    pub height: f64,
    /// Library patterns: scale in X and Y.
    pub x_scale: f64,
    pub y_scale: f64,
    /// Row offset as a fraction of the width (Grid, Offset / Grid, Step).
    pub row_offset: f64,
    /// Horizontal and vertical offsets, plan inches.
    pub h_offset: f64,
    pub v_offset: f64,
    pub angle_deg: f64,
    /// Pattern line weight, hundredths of a millimetre.
    pub line_weight: u32,
    pub color: ColorSource,
    /// 0 is opaque, 1 invisible; also applies to a Solid fill.
    pub transparency: f64,
    pub gradient: Option<Gradient>,
    /// `None` is a transparent pattern background.
    pub background: Option<Background>,
}

impl Default for FillStyle {
    fn default() -> Self {
        Self {
            pattern: PatternType::UseLayer,
            width: 12.0,
            height: 12.0,
            x_scale: 1.0,
            y_scale: 1.0,
            row_offset: 0.5,
            h_offset: 0.0,
            v_offset: 0.0,
            angle_deg: 0.0,
            line_weight: 18,
            color: ColorSource::Layer,
            transparency: 0.0,
            gradient: None,
            background: None,
        }
    }
}

impl FillStyle {
    pub fn use_layer() -> Self {
        Self::default()
    }

    pub fn solid(color: [u8; 3]) -> Self {
        Self {
            pattern: PatternType::System(SystemPattern::Solid),
            color: ColorSource::Single(color),
            ..Self::default()
        }
    }

    pub fn system(p: SystemPattern, width: f64, height: f64) -> Self {
        Self {
            pattern: PatternType::System(p),
            width,
            height,
            ..Self::default()
        }
    }

    /// Parallel lines `spacing` apart at `angle_deg`.
    pub fn hatch(angle_deg: f64, spacing: f64, color: [u8; 3]) -> Self {
        Self {
            angle_deg,
            color: ColorSource::Single(color),
            ..Self::system(SystemPattern::Lines, spacing, spacing)
        }
    }

    pub fn library(name: &str) -> Self {
        Self {
            pattern: PatternType::Library(name.to_string()),
            ..Self::default()
        }
    }

    pub fn is_solid(&self) -> bool {
        self.pattern == PatternType::System(SystemPattern::Solid)
    }

    /// The old plan fills (a name, a colour and an opacity) in the shared
    /// type. `None` for "None" (no fill). Names are the CAD Hatch choices
    /// and the room and slab pattern names.
    pub fn from_legacy(pattern: &str, color: [u8; 3], opacity: f64) -> Option<FillStyle> {
        let transparency = (1.0 - opacity).clamp(0.0, 1.0);
        let base = |p: SystemPattern, w: f64, h: f64, angle: f64| FillStyle {
            angle_deg: angle,
            color: ColorSource::Single(color),
            transparency,
            ..FillStyle::system(p, w, h)
        };
        Some(match pattern {
            "" | "None" => return None,
            "Solid" => FillStyle {
                transparency,
                ..FillStyle::solid(color)
            },
            "Hatch" | "Diagonal Lines" => base(SystemPattern::Lines, 12.0, 12.0, 45.0),
            "Cross Hatch" => base(SystemPattern::CrossHatch, 12.0, 12.0, 45.0),
            "Horizontal Lines" => base(SystemPattern::Lines, 12.0, 12.0, 0.0),
            "Vertical Lines" => base(SystemPattern::Lines, 12.0, 12.0, 90.0),
            "Grid" | "Tile" => base(SystemPattern::Grid, 12.0, 12.0, 0.0),
            "Brick" => base(SystemPattern::Brick, 8.0, 2.25, 0.0),
            "Block" => base(SystemPattern::Brick, 16.0, 8.0, 0.0),
            "Insulation" => base(SystemPattern::Us, 12.0, 6.0, 0.0),
            "Concrete" => base(SystemPattern::Concrete, 12.0, 12.0, 0.0),
            "Earth" => base(SystemPattern::Sand, 12.0, 12.0, 0.0),
            other => FillStyle {
                pattern: PatternType::Library(other.to_string()),
                color: ColorSource::Single(color),
                transparency,
                ..FillStyle::default()
            },
        })
    }

    /// A CAD object's fill (`FillAttr`) in the shared type.
    pub fn from_fill_attr(a: &FillAttr) -> Option<FillStyle> {
        let mut s = FillStyle::from_legacy(
            if a.pattern.is_empty() {
                "Solid"
            } else {
                &a.pattern
            },
            a.color,
            f64::from(a.opacity) / 255.0,
        )?;
        if !a.pattern.is_empty() {
            s.width = a.spacing.max(0.1);
            s.height = a.spacing.max(0.1);
            s.angle_deg = a.angle_deg;
        }
        Some(s)
    }

    /// The solid fill colour with its opacity (`None` when the style is not
    /// solid).
    pub fn solid_rgba(&self, layer: [u8; 3], background: [u8; 3]) -> Option<[u8; 4]> {
        if !self.is_solid() {
            return None;
        }
        let c = self.color.resolve(layer, background);
        Some([c[0], c[1], c[2], alpha(self.transparency)])
    }

    /// `self` with `UseLayer` replaced by the layer's style.
    pub fn resolved<'a>(&'a self, layer_style: &'a FillStyle) -> &'a FillStyle {
        if self.pattern == PatternType::UseLayer {
            layer_style
        } else {
            self
        }
    }

    /// The text beside the preview ("Brick, 8 x 2.25").
    pub fn summary(&self) -> String {
        match &self.pattern {
            PatternType::UseLayer => "Use Layer".into(),
            PatternType::System(s) if s.has_height() => {
                format!(
                    "{}, {} x {}",
                    s.label(),
                    trim(self.width),
                    trim(self.height)
                )
            }
            PatternType::System(s) if *s != SystemPattern::Solid => {
                format!("{}, {}", s.label(), trim(self.width))
            }
            PatternType::System(s) => s.label().into(),
            PatternType::Library(n) => {
                format!("{n} ({} x {})", trim(self.x_scale), trim(self.y_scale))
            }
        }
    }
}

fn trim(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// 0..1 transparency to an alpha byte.
pub fn alpha(transparency: f64) -> u8 {
    ((1.0 - transparency.clamp(0.0, 1.0)) * 255.0).round() as u8
}

// ---------------------------------------------------------------------------
// Tiling
// ---------------------------------------------------------------------------

/// Upper bound on lines per fill.
pub const MAX_SEGMENTS: usize = 40_000;
/// Upper bound on dots per fill.
pub const MAX_DOTS: usize = 40_000;

/// What a fill draws.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FillGeometry {
    /// A solid fill of the whole area.
    pub solid: bool,
    /// The pattern's lines, already clipped to the area.
    pub lines: Vec<(Point, Point)>,
    /// The pattern's dots.
    pub dots: Vec<Point>,
    /// The limits cut the pattern short.
    pub truncated: bool,
}

/// A tile lattice: cells of `w` x `h` laid from the plan origin, the rows
/// shifted by `row_shift` (alternately or progressively), turned `angle` and
/// moved by `off`.
#[derive(Debug, Clone, Copy)]
pub struct Lattice {
    pub w: f64,
    pub h: f64,
    pub row_shift: f64,
    /// Alternate rows shift (Grid, Offset); otherwise each row shifts one
    /// more (Grid, Step).
    pub alternate: bool,
    /// Columns shift instead of rows (a custom pattern's Vertical Shift).
    pub col_shift: f64,
    pub off: Point,
    pub angle_deg: f64,
}

impl Lattice {
    fn rot(&self) -> (f64, f64) {
        let a = self.angle_deg.to_radians();
        (a.cos(), a.sin())
    }

    pub fn to_world(&self, l: Point) -> Point {
        let (c, s) = self.rot();
        Point::new(
            l.x * c - l.y * s + self.off.x,
            l.x * s + l.y * c + self.off.y,
        )
    }

    pub fn to_local(&self, p: Point) -> Point {
        let (c, s) = self.rot();
        let q = p - self.off;
        Point::new(q.x * c + q.y * s, -q.x * s + q.y * c)
    }

    /// Origin of the cell in column `i`, row `j` (local coordinates).
    pub fn cell_origin(&self, i: i64, j: i64) -> Point {
        let rs = if self.row_shift != 0.0 {
            if self.alternate {
                if j.rem_euclid(2) == 1 {
                    self.row_shift
                } else {
                    0.0
                }
            } else {
                j as f64 * self.row_shift
            }
        } else {
            0.0
        };
        let cs = if self.row_shift == 0.0 && self.col_shift != 0.0 {
            i as f64 * self.col_shift
        } else {
            0.0
        };
        Point::new(i as f64 * self.w + rs, j as f64 * self.h + cs)
    }

    /// The cells that can touch the points `pts` (world coordinates), as
    /// `(i, j)`. At most `limit` cells; the bool says the list was cut.
    pub fn cells(&self, pts: &[Point], limit: usize) -> (Vec<(i64, i64)>, bool) {
        if !(self.w > 1e-6 && self.h > 1e-6) || pts.is_empty() {
            return (Vec::new(), false);
        }
        let (mut lo, mut hi) = (
            Point::new(f64::INFINITY, f64::INFINITY),
            Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
        );
        for p in pts {
            let l = self.to_local(*p);
            lo = Point::new(lo.x.min(l.x), lo.y.min(l.y));
            hi = Point::new(hi.x.max(l.x), hi.y.max(l.y));
        }
        let mut out = Vec::new();
        let mut cut = false;
        if self.row_shift == 0.0 && self.col_shift != 0.0 {
            // Columns shift: walk columns, then the rows that reach the box.
            let i0 = (lo.x / self.w).floor() as i64 - 1;
            let i1 = (hi.x / self.w).ceil() as i64 + 1;
            for i in i0..=i1 {
                let base = self.cell_origin(i, 0).y;
                let j0 = ((lo.y - base) / self.h).floor() as i64 - 1;
                let j1 = ((hi.y - base) / self.h).ceil() as i64 + 1;
                for j in j0..=j1 {
                    if out.len() >= limit {
                        return (out, true);
                    }
                    out.push((i, j));
                }
            }
        } else {
            let j0 = (lo.y / self.h).floor() as i64 - 1;
            let j1 = (hi.y / self.h).ceil() as i64 + 1;
            for j in j0..=j1 {
                let base = self.cell_origin(0, j).x;
                let i0 = ((lo.x - base) / self.w).floor() as i64 - 1;
                let i1 = ((hi.x - base) / self.w).ceil() as i64 + 1;
                for i in i0..=i1 {
                    if out.len() >= limit {
                        cut = true;
                        break;
                    }
                    out.push((i, j));
                }
                if cut {
                    break;
                }
            }
        }
        (out, cut)
    }
}

/// Even-odd inside test across an outer ring and its holes.
fn inside(p: Point, rings: &[&[Point]]) -> bool {
    rings.iter().filter(|r| point_in_polygon(p, r)).count() % 2 == 1
}

/// The parts of segment `a`-`b` inside the rings.
pub fn clip_segment(a: Point, b: Point, rings: &[&[Point]]) -> Vec<(Point, Point)> {
    let mut ts = vec![0.0, 1.0];
    let d = b - a;
    for r in rings {
        let n = r.len();
        for i in 0..n {
            let (p, q) = (r[i], r[(i + 1) % n]);
            let e = q - p;
            let den = d.cross(e);
            if den.abs() < 1e-12 {
                continue;
            }
            let t = (p - a).cross(e) / den;
            let u = (p - a).cross(d) / den;
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                ts.push(t);
            }
        }
    }
    ts.sort_by(f64::total_cmp);
    let mut out: Vec<(Point, Point)> = Vec::new();
    for w in ts.windows(2) {
        if w[1] - w[0] < 1e-9 {
            continue;
        }
        let m = Point::lerp(a, b, (w[0] + w[1]) / 2.0);
        if inside(m, rings) {
            let seg = (Point::lerp(a, b, w[0]), Point::lerp(a, b, w[1]));
            match out.last_mut() {
                Some(last) if last.1.dist(seg.0) < 1e-7 => last.1 = seg.1,
                _ => out.push(seg),
            }
        }
    }
    out
}

fn hash(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit(i: i64, j: i64, k: u64) -> f64 {
    let h = hash(hash(i as u64 ^ k) ^ (j as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93));
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// The corners of the rings' bounding box.
fn bounds(rings: &[&[Point]]) -> Option<(Point, Point)> {
    let mut it = rings.iter().flat_map(|r| r.iter().copied());
    let first = it.next()?;
    Some(it.fold((first, first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}

/// A family of parallel infinite lines through `origin` at `angle_deg`,
/// `spacing` apart, each shifted `shift` along itself from the one before,
/// clipped to the rings. `dashes` (positive dash, negative gap, zero dot)
/// breaks the lines up.
pub fn line_family(
    rings: &[&[Point]],
    origin: Point,
    angle_deg: f64,
    spacing: f64,
    shift: f64,
    dashes: &[f64],
    out: &mut FillGeometry,
) {
    let Some((lo, hi)) = bounds(rings) else {
        return;
    };
    if spacing.is_nan() || spacing.abs() <= 1e-4 {
        return;
    }
    let spacing = spacing.abs();
    let a = angle_deg.to_radians();
    let (u, n) = (Point::new(a.cos(), a.sin()), Point::new(-a.sin(), a.cos()));
    let corners = [lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)];
    let (mut tmin, mut tmax, mut smin, mut smax) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for c in corners {
        let q = c - origin;
        tmin = tmin.min(q.dot(n));
        tmax = tmax.max(q.dot(n));
        smin = smin.min(q.dot(u));
        smax = smax.max(q.dot(u));
    }
    let k0 = (tmin / spacing).ceil() as i64;
    let k1 = (tmax / spacing).floor() as i64;
    if k1 - k0 > 20_000 {
        out.truncated = true;
        return;
    }
    let period: f64 = dashes.iter().map(|d| d.abs()).sum();
    for k in k0..=k1 {
        let base = origin + n * (k as f64 * spacing);
        let s_shift = k as f64 * shift;
        // Parameter range along the line, measured from its own origin.
        let (s0, s1) = (smin - s_shift - 1.0, smax - s_shift + 1.0);
        let at = |s: f64| base + u * (s + s_shift);
        if dashes.is_empty() || period < 1e-6 {
            for (p, q) in clip_segment(at(s0), at(s1), rings) {
                if out.lines.len() >= MAX_SEGMENTS {
                    out.truncated = true;
                    return;
                }
                out.lines.push((p, q));
            }
            continue;
        }
        let mut s = (s0 / period).floor() * period;
        let mut guard = 0usize;
        while s < s1 {
            for d in dashes {
                guard += 1;
                if guard > 400_000 || out.lines.len() >= MAX_SEGMENTS {
                    out.truncated = true;
                    return;
                }
                let len = d.abs();
                if *d > 1e-9 {
                    for (p, q) in clip_segment(at(s), at(s + len), rings) {
                        out.lines.push((p, q));
                    }
                } else if d.abs() <= 1e-9 && inside(at(s), rings) && out.dots.len() < MAX_DOTS {
                    out.dots.push(at(s));
                }
                s += len;
            }
        }
    }
}

/// Segments of one cell of a system pattern in cell coordinates, and dots.
fn system_tile(
    p: SystemPattern,
    w: f64,
    h: f64,
    i: i64,
    j: i64,
) -> (Vec<(Point, Point)>, Vec<Point>) {
    let pt = Point::new;
    match p {
        SystemPattern::Grid
        | SystemPattern::GridOffset
        | SystemPattern::GridStep
        | SystemPattern::Brick => (
            vec![(pt(0.0, 0.0), pt(w, 0.0)), (pt(0.0, 0.0), pt(0.0, h))],
            Vec::new(),
        ),
        SystemPattern::Herringbone => (
            vec![(pt(0.0, 0.0), pt(w / 2.0, h)), (pt(w / 2.0, h), pt(w, 0.0))],
            Vec::new(),
        ),
        SystemPattern::Us => (
            vec![
                (pt(0.25 * w, 0.75 * h), pt(0.25 * w, 0.25 * h)),
                (pt(0.25 * w, 0.25 * h), pt(0.75 * w, 0.25 * h)),
                (pt(0.75 * w, 0.25 * h), pt(0.75 * w, 0.75 * h)),
            ],
            Vec::new(),
        ),
        SystemPattern::Dots => (Vec::new(), vec![pt(w / 2.0, h / 2.0)]),
        SystemPattern::Concrete | SystemPattern::Sand => {
            let n = if p == SystemPattern::Concrete { 3 } else { 7 };
            (
                Vec::new(),
                (0..n)
                    .map(|k| pt(unit(i, j, 11 + k) * w, unit(i, j, 91 + k) * h))
                    .collect(),
            )
        }
        SystemPattern::Solid | SystemPattern::Lines | SystemPattern::CrossHatch => {
            (Vec::new(), Vec::new())
        }
    }
}

/// The lines and dots of `style` over `outer` minus `holes`. `patterns`
/// resolves library patterns. A Use Layer style draws nothing here: resolve
/// it against the layer's style first ([`FillStyle::resolved`]).
pub fn fill_geometry(
    style: &FillStyle,
    outer: &[Point],
    holes: &[Vec<Point>],
    patterns: &[CustomPattern],
) -> FillGeometry {
    let mut g = FillGeometry::default();
    if outer.len() < 3 {
        return g;
    }
    let mut rings: Vec<&[Point]> = vec![outer];
    rings.extend(holes.iter().map(|h| h.as_slice()));
    let off = Point::new(style.h_offset, style.v_offset);
    match &style.pattern {
        PatternType::UseLayer => {}
        PatternType::System(SystemPattern::Solid) => g.solid = true,
        PatternType::System(SystemPattern::Lines) => {
            // The lines stay `width` apart whatever the angle; offsets move
            // the family across itself.
            line_family(&rings, off, style.angle_deg, style.width, 0.0, &[], &mut g);
        }
        PatternType::System(SystemPattern::CrossHatch) => {
            line_family(&rings, off, style.angle_deg, style.width, 0.0, &[], &mut g);
            line_family(
                &rings,
                off,
                style.angle_deg + 90.0,
                style.width,
                0.0,
                &[],
                &mut g,
            );
        }
        PatternType::System(sp) => {
            let (w, h) = match sp {
                SystemPattern::Concrete | SystemPattern::Sand => (style.width, style.width),
                _ => (style.width, style.height),
            };
            let lat = Lattice {
                w,
                h,
                row_shift: match sp {
                    SystemPattern::GridOffset | SystemPattern::GridStep | SystemPattern::Brick => {
                        style.row_offset * w
                    }
                    _ => 0.0,
                },
                alternate: !matches!(sp, SystemPattern::GridStep),
                col_shift: 0.0,
                off,
                angle_deg: style.angle_deg,
            };
            tile_into(
                &lat,
                &rings,
                1.0,
                1.0,
                |i, j| system_tile(*sp, w, h, i, j),
                &mut g,
            );
        }
        PatternType::Library(name) => {
            if let Some(p) = patterns.iter().find(|p| &p.name == name) {
                custom_into(p, style, &rings, &mut g);
            }
        }
    }
    g
}

/// Lays `tile` (cell coordinates scaled by `sx`, `sy`) on the lattice.
fn tile_into(
    lat: &Lattice,
    rings: &[&[Point]],
    sx: f64,
    sy: f64,
    tile: impl Fn(i64, i64) -> (Vec<(Point, Point)>, Vec<Point>),
    g: &mut FillGeometry,
) {
    let all: Vec<Point> = rings.iter().flat_map(|r| r.iter().copied()).collect();
    let (cells, cut) = lat.cells(&all, 60_000);
    g.truncated |= cut;
    let Some((lo, hi)) = bounds(rings) else {
        return;
    };
    let pad = lat.w.max(lat.h) * 2.0;
    for (i, j) in cells {
        let (segs, dots) = tile(i, j);
        let o = lat.cell_origin(i, j);
        for (a, b) in segs {
            let wa = lat.to_world(Point::new(o.x + a.x * sx, o.y + a.y * sy));
            let wb = lat.to_world(Point::new(o.x + b.x * sx, o.y + b.y * sy));
            if wa.x.max(wb.x) < lo.x - pad
                || wa.x.min(wb.x) > hi.x + pad
                || wa.y.max(wb.y) < lo.y - pad
                || wa.y.min(wb.y) > hi.y + pad
            {
                continue;
            }
            for s in clip_segment(wa, wb, rings) {
                if g.lines.len() >= MAX_SEGMENTS {
                    g.truncated = true;
                    return;
                }
                g.lines.push(s);
            }
        }
        for d in dots {
            let wd = lat.to_world(Point::new(o.x + d.x * sx, o.y + d.y * sy));
            if inside(wd, rings) {
                if g.dots.len() >= MAX_DOTS {
                    g.truncated = true;
                    return;
                }
                g.dots.push(wd);
            }
        }
    }
}

/// The custom pattern `p` over the rings, scaled and turned by `style`.
fn custom_into(p: &CustomPattern, style: &FillStyle, rings: &[&[Point]], g: &mut FillGeometry) {
    let (sx, sy) = (style.x_scale.max(1e-3), style.y_scale.max(1e-3));
    let off = Point::new(style.h_offset, style.v_offset);
    for grp in &p.groups {
        let lat = Lattice {
            w: (grp.width * sx).max(1e-3),
            h: (grp.height * sy).max(1e-3),
            row_shift: grp.h_shift * sx,
            alternate: false,
            col_shift: grp.v_shift * sy,
            off,
            angle_deg: style.angle_deg,
        };
        if !grp.tile.is_empty() || !grp.dots.is_empty() {
            tile_into(
                &lat,
                rings,
                sx,
                sy,
                |_, _| (grp.tile.clone(), grp.dots.clone()),
                g,
            );
        }
        for l in &grp.infinite {
            infinite_into(l, &lat, sx.min(sy), rings, g);
        }
    }
}

/// An infinite pattern line of a group, in the group's frame.
fn infinite_into(
    l: &InfiniteLine,
    lat: &Lattice,
    k: f64,
    rings: &[&[Point]],
    g: &mut FillGeometry,
) {
    let origin = lat.to_world(Point::new(l.x * k, l.y * k));
    let dashes: Vec<f64> = l.dashes.iter().map(|d| d * k).collect();
    line_family(
        rings,
        origin,
        l.angle_deg + lat.angle_deg,
        l.spacing * k,
        l.shift * k,
        &dashes,
        g,
    );
}

// ---------------------------------------------------------------------------
// Assignments
// ---------------------------------------------------------------------------

/// Where a fill style is applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FillTarget {
    /// A CAD object (a closed polyline, a circle...) by id.
    Cad(Id),
    /// A slab by id.
    Slab(Id),
    /// A room, by its anchor point.
    Room(Point),
    /// Layer `index` (exterior first) of a wall type definition.
    WallLayer { wall_type: String, index: usize },
    /// The fill style of a display layer.
    Layer(String),
    /// A defaults dialog or tool.
    Default(String),
}

/// A named fill style (Fill Style Specification, library).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedFill {
    pub name: String,
    pub style: FillStyle,
}

/// Poché: which kind of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PocheView {
    Plan,
    Section,
    Overview,
}

/// The Poché switch of each view (View Specification dialogs, Layout Box
/// Specification).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PocheSettings {
    /// A view's own choice by name (a saved plan view, a section camera, a
    /// layout box).
    pub views: Vec<(String, bool)>,
    /// The colour of poché that has no wall fill style.
    pub color: Option<[u8; 3]>,
}

/// The default dark poché colour.
pub const POCHE_COLOR: [u8; 3] = [55, 55, 55];

impl PocheSettings {
    /// Whether poché shows in view `name` of kind `kind`. Plans and floor
    /// overviews start with it off, sections (which always had it) on.
    pub fn is_on(&self, name: &str, kind: PocheView) -> bool {
        self.views
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, on)| *on)
            .unwrap_or(matches!(kind, PocheView::Section))
    }

    pub fn set(&mut self, name: &str, on: bool) {
        match self.views.iter_mut().find(|(n, _)| n == name) {
            Some(v) => v.1 = on,
            None => self.views.push((name.to_string(), on)),
        }
    }

    pub fn color(&self) -> [u8; 3] {
        self.color.unwrap_or(POCHE_COLOR)
    }
}

/// Does a cut wall get poché (manual p. 226): not glass walls, invisible
/// walls, room dividers, railings, fencing, deck edges or raised walls.
pub fn wall_gets_poche(w: &Wall) -> bool {
    if w.flags.invisible || w.flags.room_divider || w.flags.railing {
        return false;
    }
    if w.bottom_offset >= crate::walls::ROOM_BOUNDARY_MAX_BOTTOM {
        return false;
    }
    !matches!(
        w.class,
        WallClass::Glass
            | WallClass::RoomDivider
            | WallClass::Railing
            | WallClass::DeckRailing
            | WallClass::DeckEdge
            | WallClass::Fencing { .. }
    )
}

/// Everything the Style tools keep in the plan.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleBook {
    /// The line styles saved in the file (Line Style Management).
    pub line_styles: LineStyleLibrary,
    /// Library line styles applied to objects, layers and defaults.
    pub line_assign: Vec<(LineTarget, String)>,
    /// Named fill styles saved in the file.
    pub fill_styles: Vec<NamedFill>,
    /// Fill styles applied to objects.
    pub fill_assign: Vec<(FillTarget, FillStyle)>,
    /// Custom patterns saved in the file.
    pub patterns: Vec<CustomPattern>,
    /// The User Catalog entries (Add to Library) kept with the plan until
    /// the library browser has a writer for them.
    pub user_lines: Vec<LineStyleDef>,
    pub user_fills: Vec<NamedFill>,
    pub user_patterns: Vec<CustomPattern>,
    pub poche: PocheSettings,
}

impl StyleBook {
    pub fn is_default(&self) -> bool {
        *self == StyleBook::default()
    }

    // ----- line styles -----

    /// The library style named `name`: the file's, else the catalog's.
    pub fn line_style(&self, name: &str) -> Option<LineStyleDef> {
        self.line_styles
            .get(name)
            .cloned()
            .or_else(|| self.user_lines.iter().find(|s| s.name == name).cloned())
            .or_else(|| {
                crate::line_styles::catalog()
                    .into_iter()
                    .find(|s| s.name == name)
            })
    }

    /// The style assigned to `target`.
    pub fn assigned_line(&self, target: &LineTarget) -> Option<&str> {
        self.line_assign
            .iter()
            .find(|(t, _)| t == target)
            .map(|(_, n)| n.as_str())
    }

    /// Assigns library style `def` to `target`, saving it in the file when
    /// it came from the catalog. `None` clears the assignment.
    pub fn set_line(&mut self, target: LineTarget, def: Option<&LineStyleDef>) {
        self.line_assign.retain(|(t, _)| *t != target);
        if let Some(def) = def {
            let name = if self.line_styles.get(&def.name).is_some_and(|s| s.system)
                && self.line_styles.get(&def.name).map(|s| &s.components) == Some(&def.components)
            {
                def.name.clone()
            } else {
                self.line_styles.ensure(def)
            };
            self.line_assign.push((target, name));
        }
        if self.line_styles.auto_purge {
            self.purge_line_styles();
        }
    }

    /// Where `name` is used, for the Used column.
    pub fn line_usage(
        &self,
        enum_uses: &[crate::layers::LineStyle],
        cad_exists: &dyn Fn(Id) -> bool,
    ) -> std::collections::BTreeMap<String, crate::line_styles::UseMark> {
        crate::line_styles::usage(&self.line_assign, enum_uses, cad_exists, &self.line_styles)
    }

    /// Purge: drops the unused styles of the file.
    pub fn purge_line_styles(&mut self) -> Vec<String> {
        let used: Vec<String> = self.line_assign.iter().map(|(_, n)| n.clone()).collect();
        self.line_styles.purge(&|n| used.iter().any(|u| u == n))
    }

    /// Merge: keeps the topmost of `rows`; the dropped styles' instances now
    /// use it.
    pub fn merge_line_styles(&mut self, rows: &[usize]) -> Result<String, String> {
        let (kept, dropped) = self.line_styles.merge(rows)?;
        for (_, n) in &mut self.line_assign {
            if dropped.contains(n) {
                n.clone_from(&kept);
            }
        }
        Ok(kept)
    }

    /// Renames line style `index`, repointing its instances.
    pub fn rename_line_style(&mut self, index: usize, def: LineStyleDef) -> Result<(), String> {
        let old = self
            .line_styles
            .styles
            .get(index)
            .map(|s| s.name.clone())
            .ok_or("No such line style")?;
        let new = def.name.clone();
        self.line_styles.edit(index, def)?;
        for (_, n) in &mut self.line_assign {
            if *n == old {
                n.clone_from(&new);
            }
        }
        Ok(())
    }

    /// Add to Library: puts the style in the User Catalog.
    pub fn add_line_to_library(&mut self, def: &LineStyleDef) {
        match self.user_lines.iter_mut().find(|s| s.name == def.name) {
            Some(s) => {
                *s = LineStyleDef {
                    system: false,
                    ..def.clone()
                }
            }
            None => self.user_lines.push(LineStyleDef {
                system: false,
                ..def.clone()
            }),
        }
    }

    /// File > Import > Import Line Styles: the styles go to the User Catalog.
    pub fn import_lin(&mut self, text: &str, unit: f64) -> crate::line_styles::LinImport {
        let imp = crate::line_styles::parse_lin(text, unit);
        for s in &imp.styles {
            self.add_line_to_library(s);
        }
        imp
    }

    // ----- fill styles -----

    pub fn fill_for(&self, target: &FillTarget) -> Option<&FillStyle> {
        self.fill_assign
            .iter()
            .find(|(t, _)| t == target)
            .map(|(_, s)| s)
    }

    /// Fill Style Painter: reads the fill style an object has (Eyedropper).
    pub fn read_fill(&self, target: &FillTarget) -> Option<FillStyle> {
        self.fill_for(target).cloned()
    }

    /// Fill Style Painter: paints `style` onto `target` (`None` removes the
    /// assignment, so the object goes back to its own fill).
    pub fn apply_fill(&mut self, target: FillTarget, style: Option<FillStyle>) {
        self.fill_assign.retain(|(t, _)| *t != target);
        if let Some(s) = style {
            self.fill_assign.push((target, s));
        }
    }

    /// The pattern `name` of the file, else the User Catalog's.
    pub fn pattern(&self, name: &str) -> Option<&CustomPattern> {
        self.patterns
            .iter()
            .find(|p| p.name == name)
            .or_else(|| self.user_patterns.iter().find(|p| p.name == name))
    }

    /// Every pattern a fill can name (file first).
    pub fn all_patterns(&self) -> Vec<CustomPattern> {
        let mut v = self.patterns.clone();
        for p in &self.user_patterns {
            if !v.iter().any(|q| q.name == p.name) {
                v.push(p.clone());
            }
        }
        v
    }

    /// Saves a pattern in the file under a free name; returns the name.
    pub fn add_pattern(&mut self, mut p: CustomPattern) -> String {
        let base = if p.name.trim().is_empty() {
            "New Pattern".to_string()
        } else {
            p.name.clone()
        };
        let mut name = base.clone();
        let mut n = 2;
        while self.patterns.iter().any(|q| q.name == name) {
            name = format!("{base} {n}");
            n += 1;
        }
        p.name = name.clone();
        self.patterns.push(p);
        name
    }

    /// Replaces the pattern called `name` (Save Active View in a Pattern
    /// window).
    pub fn update_pattern(&mut self, name: &str, p: CustomPattern) -> bool {
        match self.patterns.iter_mut().find(|q| q.name == name) {
            Some(q) => {
                *q = CustomPattern {
                    name: name.to_string(),
                    ..p
                };
                true
            }
            None => false,
        }
    }

    /// Add to Library for a fill style panel: the style as defined, named.
    pub fn add_fill_to_library(&mut self, name: &str, style: FillStyle) {
        match self.user_fills.iter_mut().find(|f| f.name == name) {
            Some(f) => f.style = style,
            None => self.user_fills.push(NamedFill {
                name: name.to_string(),
                style,
            }),
        }
    }

    /// File > Import > Import Patterns (.pat): patterns go to the User Catalog.
    pub fn import_pat(&mut self, text: &str, unit: f64) -> crate::patterns::PatImport {
        let imp = crate::patterns::parse_pat(text, unit);
        for p in &imp.patterns {
            match self.user_patterns.iter_mut().find(|q| q.name == p.name) {
                Some(q) => *q = p.clone(),
                None => self.user_patterns.push(p.clone()),
            }
        }
        imp
    }

    /// Drops assignments to objects that are gone.
    pub fn prune(&mut self, cad_exists: &dyn Fn(Id) -> bool, slab_exists: &dyn Fn(Id) -> bool) {
        self.line_assign.retain(|(t, _)| match t {
            LineTarget::Cad(id) => cad_exists(*id),
            _ => true,
        });
        self.fill_assign.retain(|(t, _)| match t {
            FillTarget::Cad(id) => cad_exists(*id),
            FillTarget::Slab(id) => slab_exists(*id),
            _ => true,
        });
    }
}

impl crate::Project {
    /// The library style the CAD object `id` on `layer` is assigned: its
    /// own, else its layer's. `None` leaves the drawing to the layer and
    /// object `LineStyle` fields.
    pub fn assigned_line_style(&self, id: Id, layer: &str) -> Option<LineStyleDef> {
        self.styles
            .assigned_line(&LineTarget::Cad(id))
            .or_else(|| {
                self.styles
                    .assigned_line(&LineTarget::Layer(layer.to_string()))
            })
            .and_then(|n| self.styles.line_style(n))
    }

    /// The pieces to draw CAD object `id` (on `layer`, shape `item`) with
    /// its library line style, for a view of `plan_per_paper` plan inches per
    /// paper inch. `None` when the object has no library style (or no path):
    /// draw it the usual way. Plan, layout and PDF renderers all call this.
    pub fn styled_line_pieces(
        &self,
        id: Id,
        layer: &str,
        item: &crate::cad::CadItem,
        plan_per_paper: f64,
    ) -> Option<Vec<crate::line_styles::Piece>> {
        let def = self.assigned_line_style(id, layer)?;
        let (pts, closed) = crate::line_styles::cad_item_path(item)?;
        Some(crate::line_styles::stroke_path(
            &def,
            &pts,
            closed,
            plan_per_paper,
        ))
    }

    /// The fill style of wall layer `index` of type `wall_type`.
    pub fn wall_layer_fill(&self, wall_type: &str, index: usize) -> Option<&FillStyle> {
        self.styles.fill_for(&FillTarget::WallLayer {
            wall_type: wall_type.to_string(),
            index,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::PatternTileGroup;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn square(s: f64) -> Vec<Point> {
        vec![p(0.0, 0.0), p(s, 0.0), p(s, s), p(0.0, s)]
    }

    #[test]
    fn a_solid_style_fills_with_its_colour_and_transparency() {
        let mut s = FillStyle::solid([10, 20, 30]);
        s.transparency = 0.5;
        assert_eq!(s.solid_rgba([0; 3], [255; 3]), Some([10, 20, 30, 128]));
        let g = fill_geometry(&s, &square(48.0), &[], &[]);
        assert!(g.solid && g.lines.is_empty());
        // Use Layer draws nothing until it is resolved against the layer.
        let ul = FillStyle::use_layer();
        assert_eq!(
            fill_geometry(&ul, &square(48.0), &[], &[]),
            FillGeometry::default()
        );
        assert_eq!(ul.resolved(&s), &s);
        assert_eq!(ColorSource::Layer.resolve([1, 2, 3], [9, 9, 9]), [1, 2, 3]);
        assert_eq!(
            ColorSource::Background.resolve([1, 2, 3], [9, 9, 9]),
            [9, 9, 9]
        );
    }

    #[test]
    fn a_line_pattern_keeps_its_spacing_at_any_angle_and_follows_the_offset() {
        let s = FillStyle::hatch(0.0, 12.0, [0; 3]);
        let g = fill_geometry(&s, &square(48.0), &[], &[]);
        // Lines at y = 12, 24, 36 (0 and 48 touch the edge) fall inside.
        let mut ys: Vec<f64> = g.lines.iter().map(|l| l.0.y).collect();
        ys.sort_by(f64::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        assert!(
            ys.contains(&12.0) && ys.contains(&24.0) && ys.contains(&36.0),
            "{ys:?}"
        );
        for (a, b) in &g.lines {
            assert!((a.y - b.y).abs() < 1e-9, "horizontal");
            assert!(a.x >= -1e-9 && b.x <= 48.0 + 1e-9);
        }
        // A vertical offset of 4 moves every line up 4.
        let mut off = s.clone();
        off.v_offset = 4.0;
        let g2 = fill_geometry(&off, &square(48.0), &[], &[]);
        let ys2: Vec<f64> = g2.lines.iter().map(|l| l.0.y).collect();
        assert!(ys2.iter().any(|y| (y - 16.0).abs() < 1e-6));
        // At 45 degrees the perpendicular spacing is still 12.
        let d = fill_geometry(
            &FillStyle::hatch(45.0, 12.0, [0; 3]),
            &square(120.0),
            &[],
            &[],
        );
        let offs: Vec<f64> = d
            .lines
            .iter()
            .map(|(a, _)| (a.y - a.x) / std::f64::consts::SQRT_2)
            .collect();
        let mut sorted = offs.clone();
        sorted.sort_by(f64::total_cmp);
        sorted.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        for w in sorted.windows(2) {
            assert!((w[1] - w[0] - 12.0).abs() < 1e-6, "{sorted:?}");
        }
    }

    #[test]
    fn grid_offset_shifts_alternate_rows_and_grid_step_shifts_each_row_more() {
        let mut off = FillStyle::system(SystemPattern::GridOffset, 8.0, 4.0);
        off.row_offset = 0.25;
        let g = fill_geometry(&off, &square(32.0), &[], &[]);
        let verticals = |g: &FillGeometry, y0: f64| -> Vec<f64> {
            let mut v: Vec<f64> = g
                .lines
                .iter()
                .filter(|(a, b)| (a.x - b.x).abs() < 1e-9 && (a.y.min(b.y) - y0).abs() < 1e-6)
                .map(|(a, _)| a.x)
                .collect();
            v.sort_by(f64::total_cmp);
            v.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            v
        };
        let row0 = verticals(&g, 0.0);
        let row1 = verticals(&g, 4.0);
        let row2 = verticals(&g, 8.0);
        assert!(row0.contains(&8.0) && row1.contains(&2.0) && row1.contains(&10.0));
        assert_eq!(row0, row2, "alternate rows repeat");
        let mut step = off.clone();
        step.pattern = PatternType::System(SystemPattern::GridStep);
        let gs = fill_geometry(&step, &square(32.0), &[], &[]);
        let r0 = verticals(&gs, 0.0);
        let r1 = verticals(&gs, 4.0);
        let r2 = verticals(&gs, 8.0);
        assert!(
            r0.contains(&8.0) && r1.contains(&2.0) && r2.contains(&4.0),
            "{r0:?} {r1:?} {r2:?}"
        );
    }

    #[test]
    fn scale_angle_and_offset_move_a_library_tile_as_the_panel_says() {
        // A tile with a diagonal in a 10 x 10 repeat box.
        let mut pat = CustomPattern::new("Slash");
        pat.groups[0] = PatternTileGroup {
            tile: vec![(p(0.0, 0.0), p(10.0, 10.0))],
            width: 10.0,
            height: 10.0,
            ..PatternTileGroup::default()
        };
        let lib = vec![pat];
        let mut s = FillStyle::library("Slash");
        let g = fill_geometry(&s, &square(40.0), &[], &lib);
        assert_eq!(g.lines.len(), 16, "4 x 4 tiles");
        s.x_scale = 2.0;
        s.y_scale = 2.0;
        let g2 = fill_geometry(&s, &square(40.0), &[], &lib);
        assert_eq!(g2.lines.len(), 4, "2 x 2 tiles at twice the size");
        for (a, b) in &g2.lines {
            assert!((b.x - a.x - 20.0).abs() < 1e-9);
        }
        // Turned 90 degrees the slash leans the other way.
        s.x_scale = 1.0;
        s.y_scale = 1.0;
        s.angle_deg = 90.0;
        let g3 = fill_geometry(&s, &square(40.0), &[], &lib);
        assert!(!g3.lines.is_empty());
        for (a, b) in &g3.lines {
            assert!(((b.x - a.x) * (b.y - a.y)) < 0.0, "negative slope");
        }
        // The horizontal offset shifts the tile phase.
        s.angle_deg = 0.0;
        s.h_offset = 5.0;
        let g4 = fill_geometry(&s, &square(40.0), &[], &lib);
        let min_x = g4
            .lines
            .iter()
            .map(|l| l.0.x.min(l.1.x))
            .fold(f64::INFINITY, f64::min);
        assert!(min_x.abs() < 1e-9);
        assert!(g4
            .lines
            .iter()
            .any(|(a, _)| (a.x - 5.0).abs() < 1e-6 || (a.x - 0.0).abs() < 1e-6));
    }

    #[test]
    fn holes_and_concave_areas_are_not_hatched() {
        let outer = square(48.0);
        let hole = vec![p(12.0, 12.0), p(36.0, 12.0), p(36.0, 36.0), p(12.0, 36.0)];
        let g = fill_geometry(
            &FillStyle::hatch(0.0, 6.0, [0; 3]),
            &outer,
            std::slice::from_ref(&hole),
            &[],
        );
        for (a, b) in &g.lines {
            let mid = Point::lerp(*a, *b, 0.5);
            assert!(!point_in_polygon(mid, &hole), "{a:?}");
        }
        assert!(g.lines.len() > 6);
        // Dots (Concrete) stay inside too, and are stable between calls.
        let c = FillStyle::system(SystemPattern::Concrete, 6.0, 6.0);
        let d1 = fill_geometry(&c, &outer, std::slice::from_ref(&hole), &[]);
        assert_eq!(
            d1,
            fill_geometry(&c, &outer, std::slice::from_ref(&hole), &[])
        );
        assert!(d1.dots.len() > 20);
        assert!(d1.dots.iter().all(|d| !point_in_polygon(*d, &hole)));
    }

    #[test]
    fn legacy_names_map_into_the_shared_type() {
        assert!(FillStyle::from_legacy("None", [0; 3], 1.0).is_none());
        let s = FillStyle::from_legacy("Solid", [1, 2, 3], 0.5).unwrap();
        assert!(s.is_solid() && (s.transparency - 0.5).abs() < 1e-9);
        let h = FillStyle::from_legacy("Hatch", [1, 2, 3], 1.0).unwrap();
        assert_eq!(h.pattern, PatternType::System(SystemPattern::Lines));
        assert_eq!(h.angle_deg, 45.0);
        let b = FillStyle::from_legacy("Brick", [0; 3], 1.0).unwrap();
        assert_eq!((b.width, b.height), (8.0, 2.25));
        let custom = FillStyle::from_legacy("My Tile", [0; 3], 1.0).unwrap();
        assert_eq!(custom.pattern, PatternType::Library("My Tile".into()));
        // A CAD fill with a pattern keeps its spacing and angle.
        let a = FillAttr {
            pattern: "Cross Hatch".into(),
            spacing: 9.0,
            angle_deg: 30.0,
            ..FillAttr::default()
        };
        let f = FillStyle::from_fill_attr(&a).unwrap();
        assert_eq!((f.width, f.angle_deg), (9.0, 30.0));
        assert!(FillStyle::from_fill_attr(&FillAttr::default())
            .unwrap()
            .is_solid());
        assert_eq!(h.summary(), "Lines, 12");
    }

    #[test]
    fn gradients_interpolate_and_keep_at_least_two_colours() {
        let mut g = Gradient::default();
        assert_eq!(g.sample(0.0).0, [255, 255, 255]);
        assert_eq!(g.sample(1.0).0, [0, 0, 0]);
        assert_eq!(g.sample(0.5).0, [128, 128, 128]);
        assert!(!g.remove_color(0), "two colours stay");
        g.add_color();
        assert_eq!(g.points.len(), 3);
        assert!(g.remove_color(2));
        // A linear gradient runs along its angle; a radial one from the centre.
        let (lo, hi) = (p(0.0, 0.0), p(100.0, 100.0));
        assert!(g.position_at(p(100.0, 50.0), lo, hi) < 0.01);
        assert!(g.position_at(p(0.0, 50.0), lo, hi) > 0.99);
        g.kind = GradientKind::Radial;
        assert!(g.position_at(p(50.0, 50.0), lo, hi) < 0.01);
        assert!(g.position_at(p(100.0, 100.0), lo, hi) > 0.99);
        g.reset();
        assert_eq!(g.points.len(), 2);
    }

    #[test]
    fn poche_goes_to_cut_walls_only() {
        let wall = |class: WallClass| {
            let mut w = Wall::new(
                p(0.0, 0.0),
                p(100.0, 0.0),
                6.0,
                96.0,
                crate::WallKind::Exterior,
            );
            w.class = class;
            w
        };
        assert!(wall_gets_poche(&wall(WallClass::Standard)));
        assert!(wall_gets_poche(&wall(WallClass::Foundation)));
        assert!(wall_gets_poche(&wall(WallClass::HalfWall { height: 42.0 })));
        assert!(!wall_gets_poche(&wall(WallClass::Glass)));
        assert!(!wall_gets_poche(&wall(WallClass::Railing)));
        assert!(!wall_gets_poche(&wall(WallClass::DeckRailing)));
        assert!(!wall_gets_poche(&wall(WallClass::RoomDivider)));
        assert!(!wall_gets_poche(&wall(WallClass::Fencing {
            style: crate::walls::FenceStyle::default()
        })));
        let mut invisible = wall(WallClass::Standard);
        invisible.flags.invisible = true;
        assert!(!wall_gets_poche(&invisible));
        let mut raised = wall(WallClass::Standard);
        raised.bottom_offset = 60.0;
        assert!(!wall_gets_poche(&raised));
        // The view switch: plans start off, sections on, a view can change.
        let mut ps = PocheSettings::default();
        assert!(!ps.is_on("Floor Plan", PocheView::Plan));
        assert!(ps.is_on("Section 1", PocheView::Section));
        ps.set("Floor Plan", true);
        ps.set("Section 1", false);
        assert!(ps.is_on("Floor Plan", PocheView::Plan));
        assert!(!ps.is_on("Section 1", PocheView::Section));
        assert_eq!(ps.color(), POCHE_COLOR);
    }

    #[test]
    fn the_book_assigns_reads_and_clears_fills_and_lines() {
        let mut b = StyleBook::default();
        assert!(b.is_default());
        let t = FillTarget::WallLayer {
            wall_type: "Ext 2x6".into(),
            index: 1,
        };
        assert!(b.read_fill(&t).is_none());
        b.apply_fill(t.clone(), Some(FillStyle::hatch(45.0, 6.0, [0; 3])));
        assert_eq!(b.read_fill(&t).unwrap().angle_deg, 45.0);
        b.apply_fill(t.clone(), Some(FillStyle::solid([9; 3])));
        assert_eq!(b.fill_assign.len(), 1, "one assignment per target");
        b.apply_fill(t.clone(), None);
        assert!(b.read_fill(&t).is_none());
        // A catalog line style is saved in the file when it is applied.
        let gas = b.line_style("Gas Line").unwrap();
        b.set_line(LineTarget::Cad(7), Some(&gas));
        assert!(b.line_styles.get("Gas Line").is_some());
        assert_eq!(b.assigned_line(&LineTarget::Cad(7)), Some("Gas Line"));
        b.set_line(LineTarget::Layer("CAD".into()), Some(&gas));
        assert_eq!(
            b.line_styles
                .styles
                .iter()
                .filter(|s| s.name == "Gas Line")
                .count(),
            1
        );
        // Merge repoints the instances of the dropped style.
        let mut mine = gas.clone();
        mine.name = "Mine".into();
        b.set_line(LineTarget::Cad(8), Some(&mine));
        let (a, c) = (
            b.line_styles.index_of("Gas Line").unwrap(),
            b.line_styles.index_of("Mine").unwrap(),
        );
        assert_eq!(b.merge_line_styles(&[a, c]).unwrap(), "Gas Line");
        assert_eq!(b.assigned_line(&LineTarget::Cad(8)), Some("Gas Line"));
        // Gone objects lose their assignments.
        b.prune(&|id| id != 7, &|_| true);
        assert!(b.assigned_line(&LineTarget::Cad(7)).is_none());
        // The book survives a save and load.
        let json = serde_json::to_string(&b).unwrap();
        let back: StyleBook = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }
}
