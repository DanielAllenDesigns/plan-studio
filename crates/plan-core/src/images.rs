//! Picture objects and distributed objects (the Image and Distributed Objects
//! flyouts; `docs/chief-x18-subtools.md`).
//!
//! * [`ImageSpec`] turns a [`PlacedSymbol`] into a picture: Create Image (a
//!   flat quad in plan and 3D) and Create Billboard Image (an upright quad
//!   that always faces the camera, see [`billboard_angle`]). The picture is
//!   stored by file path plus size, rotation (the symbol's `angle`) and an
//!   optional transparency key colour; `color` is the picture's average
//!   colour, used where textures are not available (3D).
//! * [`Distribution`] is the record behind Polyline/Spline Distribution Path
//!   and Region: a path or outline plus spacing, offset, scatter and random
//!   rotation/size. It regenerates its copies of a library item
//!   ([`Project::rebuild_distribution`]); the copies are ordinary
//!   [`PlacedSymbol`]s whose `owner` is the record's id.
//!
//! All lengths are inches, angles degrees counter-clockwise. The record of a
//! distribution is a zero-height [`PlacedSymbol`] whose footprint is the
//! bounding box of its path or region (so it can be picked and moved like a
//! symbol; the copies follow on the next rebuild).

use crate::geometry::{dist_to_segment, point_in_polygon, polygon_area, Point};
use crate::model::{Id, Project};
use crate::symbols::PlacedSymbol;
use serde::{Deserialize, Serialize};

/// Catalog id of the record symbol of a distribution.
pub const DISTRIBUTION_CATALOG_ID: &str = "plan.distribution";
/// Catalog id of a picture symbol.
pub const IMAGE_CATALOG_ID: &str = "plan.image";
/// Tag a user-library item carries to place a picture (`image:<path>`).
pub const IMAGE_TAG_PREFIX: &str = "image:";
/// Longest side of a freshly placed picture, inches.
pub const DEFAULT_IMAGE_LONG_SIDE: f64 = 36.0;
/// Height of a billboard when none is given, inches.
pub const DEFAULT_BILLBOARD_HEIGHT: f64 = 72.0;
/// Thickness of a billboard's plan footprint, inches.
pub const BILLBOARD_PLAN_DEPTH: f64 = 2.0;
/// Segments per span when a spline is sampled.
pub const SPLINE_SEGMENTS: usize = 12;
/// Smallest spacing a distribution accepts, inches.
pub const MIN_SPACING: f64 = 1.0;
/// Upper bound on the copies one distribution makes.
pub const MAX_COPIES: usize = 5000;

/// File format of a picture, from its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Other,
}

/// The format a file name suggests.
pub fn format_of(path: &str) -> ImageFormat {
    let l = path.to_ascii_lowercase();
    if l.ends_with(".png") {
        ImageFormat::Png
    } else if l.ends_with(".jpg") || l.ends_with(".jpeg") {
        ImageFormat::Jpeg
    } else {
        ImageFormat::Other
    }
}

/// Pixel size and format read from the file header (PNG IHDR, JPEG SOF).
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32, ImageFormat)> {
    const PNG_SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() >= 24 && bytes[..8] == PNG_SIG && &bytes[12..16] == b"IHDR" {
        let be =
            |i: usize| u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        return Some((be(16), be(20), ImageFormat::Png));
    }
    if bytes.len() > 4 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let m = bytes[i + 1];
            if m == 0xFF {
                i += 1;
                continue;
            }
            if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
                let h = u32::from(u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]));
                let w = u32::from(u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]));
                return Some((w, h, ImageFormat::Jpeg));
            }
            if m == 0xD8 || m == 0x01 || (0xD0..=0xD7).contains(&m) {
                i += 2;
                continue;
            }
            let len = usize::from(u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]));
            i += 2 + len.max(2);
        }
    }
    None
}

/// Shown for a JPEG picture that cannot be decoded (damaged, or a kind the
/// decoder does not read): the plan draws a placeholder frame.
pub const JPEG_NOTE: &str = "This JPEG could not be decoded; drawn as a frame";

fn default_gray() -> [u8; 3] {
    [180, 180, 180]
}

/// A picture placed in the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageSpec {
    /// File the picture is read from.
    pub path: String,
    pub format: ImageFormat,
    /// Pixel size of the file (`0` when it could not be read).
    pub px_w: u32,
    pub px_h: u32,
    /// Pixels of this colour (within `tolerance`) are drawn transparent.
    #[serde(default)]
    pub transparency: Option<[u8; 3]>,
    /// Per-channel tolerance of the transparency key.
    #[serde(default)]
    pub tolerance: u8,
    /// Upright and always facing the camera (Create Billboard Image).
    #[serde(default)]
    pub billboard: bool,
    /// Average colour, the flat colour used in 3D.
    #[serde(default = "default_gray")]
    pub color: [u8; 3],
    /// Why the picture shows a placeholder frame, if it does.
    #[serde(default)]
    pub note: String,
}

impl ImageSpec {
    pub fn new(path: impl Into<String>, px_w: u32, px_h: u32) -> Self {
        let path = path.into();
        let format = format_of(&path);
        let note = if format == ImageFormat::Jpeg {
            JPEG_NOTE.to_string()
        } else {
            String::new()
        };
        Self {
            path,
            format,
            px_w,
            px_h,
            transparency: None,
            tolerance: 0,
            billboard: false,
            color: default_gray(),
            note,
        }
    }

    /// Width over height of the picture (1 when unknown).
    pub fn aspect(&self) -> f64 {
        if self.px_w > 0 && self.px_h > 0 {
            f64::from(self.px_w) / f64::from(self.px_h)
        } else {
            1.0
        }
    }

    /// Size in inches with the longest side [`DEFAULT_IMAGE_LONG_SIDE`].
    pub fn default_size(&self) -> (f64, f64) {
        let a = self.aspect();
        if a >= 1.0 {
            (DEFAULT_IMAGE_LONG_SIDE, DEFAULT_IMAGE_LONG_SIDE / a)
        } else {
            (DEFAULT_IMAGE_LONG_SIDE * a, DEFAULT_IMAGE_LONG_SIDE)
        }
    }
}

impl PlacedSymbol {
    /// A flat picture whose back-center is `position` (Create Image).
    pub fn picture(spec: ImageSpec, position: Point, width: f64, depth: f64) -> Self {
        let mut s = PlacedSymbol::new(IMAGE_CATALOG_ID, position, width, depth, 0.0);
        s.image = Some(spec);
        s
    }

    /// An upright picture facing +Y at `position` (Create Billboard Image).
    pub fn billboard(mut spec: ImageSpec, position: Point, width: f64, height: f64) -> Self {
        spec.billboard = true;
        let mut s = PlacedSymbol::new(
            IMAGE_CATALOG_ID,
            position,
            width,
            BILLBOARD_PLAN_DEPTH,
            height,
        );
        s.image = Some(spec);
        s
    }
}

/// Angle (degrees) that turns a symbol standing at `pos` so its front
/// (local +Y, plan direction `(-sin a, cos a)`) faces `eye`. Returns `None`
/// when the eye is on the spot.
pub fn billboard_angle(pos: Point, eye: Point) -> Option<f64> {
    let d = eye - pos;
    if d.length() < 1e-9 {
        return None;
    }
    Some((-d.x).atan2(d.y).to_degrees().rem_euclid(360.0))
}

// ----- distributions -----

/// Path (copies along a line) or region (copies filling an outline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistKind {
    Path,
    Region,
}

/// How a region is filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionPattern {
    /// Cell centres of a grid of `spacing` squares.
    Grid,
    /// About one copy per `spacing` x `spacing` of area, at random.
    Random,
}

/// A distributed-object record (see the module docs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Distribution {
    pub kind: DistKind,
    /// The points are control points of a spline instead of polyline vertices.
    pub spline: bool,
    /// Path vertices or region outline, plan coordinates.
    pub points: Vec<Point>,
    /// Library item copied.
    pub item: String,
    /// Size of one copy: width, depth, height.
    pub item_size: [f64; 3],
    pub item_elevation: f64,
    /// Picture of an image-library item, copied onto every copy.
    #[serde(default)]
    pub item_image: Option<ImageSpec>,
    /// Layer the copies are placed on.
    pub layer: String,
    /// Distance between copies (path) or grid cell size (region).
    pub spacing: f64,
    /// Path: distance from the start of the path to the first copy.
    /// Region: copies stay this far inside the outline.
    pub offset: f64,
    /// Path only: shift of the copies to the left of the path direction.
    pub side_offset: f64,
    /// Random displacement radius.
    pub scatter: f64,
    /// Random rotation, plus/minus this many degrees (360 is fully random).
    pub random_rotation: f64,
    /// Random size, plus/minus this percent.
    pub random_size: f64,
    /// Copies turn to follow the path direction (path only).
    pub align_to_path: bool,
    pub pattern: RegionPattern,
    /// Seed of the random numbers, so a rebuild gives the same copies.
    pub seed: u64,
    /// Position of the record when the copies were last made; a record that
    /// was moved since is translated into place by the next rebuild.
    pub anchor: Point,
}

/// One copy a distribution makes (centre, angle and size factor).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DistCopy {
    pub center: Point,
    pub angle: f64,
    pub scale: f64,
}

impl Distribution {
    pub fn new(
        kind: DistKind,
        spline: bool,
        points: Vec<Point>,
        item: impl Into<String>,
        item_size: [f64; 3],
    ) -> Self {
        let spacing = (item_size[0].max(item_size[1]) * 1.5).max(12.0);
        Self {
            kind,
            spline,
            points,
            item: item.into(),
            item_size,
            item_elevation: 0.0,
            item_image: None,
            layer: "CAD, Default".to_string(),
            spacing,
            offset: 0.0,
            side_offset: 0.0,
            scatter: 0.0,
            random_rotation: 0.0,
            random_size: 0.0,
            align_to_path: false,
            pattern: RegionPattern::Grid,
            seed: 1,
            anchor: Point::ZERO,
        }
    }

    /// The path or outline as drawn: spline control points sampled, regions
    /// closed implicitly (the last point does not repeat the first).
    pub fn polyline(&self) -> Vec<Point> {
        if self.spline && self.points.len() >= 3 {
            catmull_rom(&self.points, self.kind == DistKind::Region, SPLINE_SEGMENTS)
        } else {
            self.points.clone()
        }
    }

    /// Axis-aligned bounds of the points, `None` when empty.
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let line = self.polyline();
        let first = *line.first()?;
        let (mut lo, mut hi) = (first, first);
        for p in &line {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        Some((lo, hi))
    }

    /// Distance from `p` to the path or outline.
    pub fn distance_to(&self, p: Point) -> f64 {
        let line = self.polyline();
        let n = line.len();
        let segs = if self.kind == DistKind::Region {
            n
        } else {
            n.saturating_sub(1)
        };
        (0..segs)
            .map(|i| dist_to_segment(p, line[i], line[(i + 1) % n]))
            .fold(f64::INFINITY, f64::min)
    }

    /// Length of the (sampled) path.
    pub fn path_length(&self) -> f64 {
        let line = self.polyline();
        line.windows(2).map(|w| w[0].dist(w[1])).sum()
    }

    /// The copies this distribution makes.
    pub fn copies(&self) -> Vec<DistCopy> {
        let mut rng = Rng(self.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32);
        let spacing = self.spacing.max(MIN_SPACING);
        let mut base: Vec<(Point, f64)> = Vec::new();
        let line = self.polyline();
        match self.kind {
            DistKind::Path => {
                for (p, tangent) in path_stations(&line, spacing, self.offset) {
                    let left = Point::new(-tangent.to_radians().sin(), tangent.to_radians().cos());
                    base.push((p + left * self.side_offset, tangent));
                }
            }
            DistKind::Region => {
                for p in region_points(&line, spacing, self.offset, self.pattern, &mut rng) {
                    base.push((p, 0.0));
                }
            }
        }
        base.truncate(MAX_COPIES);
        base.into_iter()
            .map(|(p, tangent)| {
                let mut center = p;
                if self.scatter > 0.0 {
                    let r = self.scatter * rng.next().sqrt();
                    let a = rng.next() * std::f64::consts::TAU;
                    center = center + Point::new(a.cos(), a.sin()) * r;
                }
                let mut angle = if self.align_to_path && self.kind == DistKind::Path {
                    tangent
                } else {
                    0.0
                };
                if self.random_rotation > 0.0 {
                    angle += (rng.next() * 2.0 - 1.0) * self.random_rotation.min(360.0);
                }
                let mut scale = 1.0;
                if self.random_size > 0.0 {
                    scale += (rng.next() * 2.0 - 1.0) * self.random_size / 100.0;
                }
                DistCopy {
                    center,
                    angle: angle.rem_euclid(360.0),
                    scale: scale.max(0.05),
                }
            })
            .collect()
    }

    fn translate(&mut self, d: Point) {
        for p in &mut self.points {
            *p = *p + d;
        }
    }
}

/// Splitmix64 uniform numbers in `[0, 1)`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Catmull-Rom samples through `pts` (`per_span` segments per span).
pub fn catmull_rom(pts: &[Point], closed: bool, per_span: usize) -> Vec<Point> {
    let n = pts.len();
    if n < 3 || per_span == 0 {
        return pts.to_vec();
    }
    let get = |i: isize| -> Point {
        if closed {
            pts[i.rem_euclid(n as isize) as usize]
        } else {
            pts[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(spans * per_span + 1);
    for s in 0..spans {
        let (p0, p1, p2, p3) = (
            get(s as isize - 1),
            get(s as isize),
            get(s as isize + 1),
            get(s as isize + 2),
        );
        for k in 0..per_span {
            let t = k as f64 / per_span as f64;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, c: f64, d: f64| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push(Point::new(
                f(p0.x, p1.x, p2.x, p3.x),
                f(p0.y, p1.y, p2.y, p3.y),
            ));
        }
    }
    if !closed {
        out.push(pts[n - 1]);
    }
    out
}

/// Points every `spacing` along `line`, the first `start` from its beginning
/// (a point falling on the last vertex is kept), each with the direction of
/// the path there in degrees.
pub fn path_stations(line: &[Point], spacing: f64, start: f64) -> Vec<(Point, f64)> {
    let mut out = Vec::new();
    if line.len() < 2 || spacing <= 0.0 {
        return out;
    }
    let total: f64 = line.windows(2).map(|w| w[0].dist(w[1])).sum();
    let mut target = start.max(0.0);
    let mut walked = 0.0;
    for w in line.windows(2) {
        let len = w[0].dist(w[1]);
        if len < 1e-12 {
            continue;
        }
        let dir = (w[1] - w[0]) * (1.0 / len);
        let ang = dir.angle().to_degrees().rem_euclid(360.0);
        while target <= walked + len + 1e-6 && target <= total + 1e-6 {
            let t = (target - walked).clamp(0.0, len);
            out.push((w[0] + dir * t, ang));
            target += spacing;
            if out.len() >= MAX_COPIES {
                return out;
            }
        }
        walked += len;
    }
    out
}

/// Points that fill the polygon `poly`.
fn region_points(
    poly: &[Point],
    spacing: f64,
    inset: f64,
    pattern: RegionPattern,
    rng: &mut Rng,
) -> Vec<Point> {
    let mut out = Vec::new();
    if poly.len() < 3 {
        return out;
    }
    let (mut lo, mut hi) = (poly[0], poly[0]);
    for p in poly {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let inside = |p: Point| {
        point_in_polygon(p, poly)
            && (inset <= 0.0
                || (0..poly.len())
                    .map(|i| dist_to_segment(p, poly[i], poly[(i + 1) % poly.len()]))
                    .fold(f64::INFINITY, f64::min)
                    >= inset - 1e-9)
    };
    match pattern {
        RegionPattern::Grid => {
            let cols = ((hi.x - lo.x) / spacing + 1e-9).floor() as usize;
            let rows = ((hi.y - lo.y) / spacing + 1e-9).floor() as usize;
            // Centre the grid in the bounds so the margins are even.
            let ox = lo.x + ((hi.x - lo.x) - cols as f64 * spacing) * 0.5 + spacing * 0.5;
            let oy = lo.y + ((hi.y - lo.y) - rows as f64 * spacing) * 0.5 + spacing * 0.5;
            for r in 0..rows {
                for c in 0..cols {
                    let p = Point::new(ox + c as f64 * spacing, oy + r as f64 * spacing);
                    if inside(p) {
                        out.push(p);
                        if out.len() >= MAX_COPIES {
                            return out;
                        }
                    }
                }
            }
        }
        RegionPattern::Random => {
            let want =
                ((polygon_area(poly).abs() / (spacing * spacing)).floor() as usize).min(MAX_COPIES);
            let mut tries = 0;
            while out.len() < want && tries < want * 40 + 100 {
                tries += 1;
                let p = Point::new(
                    lo.x + rng.next() * (hi.x - lo.x),
                    lo.y + rng.next() * (hi.y - lo.y),
                );
                if inside(p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

impl Project {
    /// Adds the record of a distribution and its copies; returns the id.
    pub fn add_distribution(&mut self, floor: usize, dist: Distribution) -> Id {
        let mut rec = PlacedSymbol::new(DISTRIBUTION_CATALOG_ID, Point::ZERO, 1.0, 1.0, 0.0);
        rec.layer = dist.layer.clone();
        rec.distribution = Some(dist);
        let id = self.add_symbol(floor, rec);
        self.rebuild_distribution(floor, id);
        id
    }

    /// Re-makes the copies of distribution record `id` and fits the record's
    /// footprint to the path. A record moved since the last rebuild takes
    /// its path along. Returns the number of copies, `None` if `id` is not a
    /// distribution.
    pub fn rebuild_distribution(&mut self, floor: usize, id: Id) -> Option<usize> {
        let f = self.floors.get_mut(floor)?;
        let rec = f
            .symbols
            .iter_mut()
            .find(|s| s.id == id && s.distribution.is_some())?;
        let mut dist = rec.distribution.take()?;
        if rec.position.dist(dist.anchor) > 1e-9 && !dist.points.is_empty() {
            dist.translate(rec.position - dist.anchor);
        }
        if let Some((lo, hi)) = dist.bounds() {
            rec.position = Point::new((lo.x + hi.x) * 0.5, lo.y);
            rec.width = (hi.x - lo.x).max(1.0);
            rec.depth = (hi.y - lo.y).max(1.0);
            rec.angle = 0.0;
        }
        dist.anchor = rec.position;
        rec.layer = dist.layer.clone();
        let layer = rec.layer.clone();
        let copies = dist.copies();
        rec.distribution = Some(dist.clone());
        f.symbols.retain(|s| s.owner != Some(id));
        let n = copies.len();
        let mut made = Vec::with_capacity(n);
        for c in &copies {
            let (w, d, h) = (
                dist.item_size[0] * c.scale,
                dist.item_size[1] * c.scale,
                dist.item_size[2] * c.scale,
            );
            // Copies are centred on the station: the back-center is half a
            // depth behind it.
            let a = c.angle.to_radians();
            let front = Point::new(-a.sin(), a.cos());
            let mut s = PlacedSymbol::new(dist.item.clone(), c.center - front * (d * 0.5), w, d, h);
            s.angle = c.angle;
            s.elevation = dist.item_elevation;
            s.layer = layer.clone();
            s.image = dist.item_image.clone();
            s.owner = Some(id);
            made.push(s);
        }
        for s in made {
            self.add_symbol(floor, s);
        }
        Some(n)
    }

    /// Rebuilds every distribution whose record was moved; returns how many.
    pub fn sync_moved_distributions(&mut self, floor: usize) -> usize {
        let Some(f) = self.floors.get(floor) else {
            return 0;
        };
        let moved: Vec<Id> = f
            .symbols
            .iter()
            .filter(|s| {
                s.distribution
                    .as_ref()
                    .is_some_and(|d| s.position.dist(d.anchor) > 1e-9)
            })
            .map(|s| s.id)
            .collect();
        for id in &moved {
            self.rebuild_distribution(floor, *id);
        }
        moved.len()
    }

    /// The number of copies a distribution record owns.
    pub fn distribution_copies(&self, floor: usize, id: Id) -> usize {
        self.floors.get(floor).map_or(0, |f| {
            f.symbols.iter().filter(|s| s.owner == Some(id)).count()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png(w: u32, h: u32) -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        v
    }

    #[test]
    fn sizes_come_from_png_and_jpeg_headers() {
        assert_eq!(
            image_size(&tiny_png(30, 20)),
            Some((30, 20, ImageFormat::Png))
        );
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xC0, 0, 11, 8, 0, 20, 0, 30, 3, 1, 0x22, 0,
        ];
        assert_eq!(image_size(&jpeg), Some((30, 20, ImageFormat::Jpeg)));
        assert_eq!(image_size(b"nope"), None);
        assert_eq!(format_of("a/B.JPEG"), ImageFormat::Jpeg);
        assert_eq!(format_of("x.png"), ImageFormat::Png);
        let spec = ImageSpec::new("p.jpg", 300, 200);
        assert!(!spec.note.is_empty());
        let (w, d) = spec.default_size();
        assert!((w - 36.0).abs() < 1e-9 && (d - 24.0).abs() < 1e-9);
    }

    #[test]
    fn images_round_trip_through_the_project_file() {
        let mut p = Project::new("img");
        let mut spec = ImageSpec::new("/tmp/pic.png", 64, 32);
        spec.transparency = Some([255, 255, 255]);
        let mut s = PlacedSymbol::picture(spec.clone(), Point::new(10.0, 20.0), 48.0, 24.0);
        s.angle = 30.0;
        let id = p.add_symbol(0, s);
        let q = Project::from_json(&p.to_json().unwrap()).unwrap();
        let back = q.floors[0].symbol(id).unwrap();
        assert_eq!(back.image.as_ref(), Some(&spec));
        assert_eq!((back.width, back.depth, back.angle), (48.0, 24.0, 30.0));
        // Ordinary symbols write none of the new fields.
        let plain =
            serde_json::to_string(&PlacedSymbol::new("a", Point::ZERO, 1.0, 1.0, 1.0)).unwrap();
        assert!(
            !plain.contains("image") && !plain.contains("distribution") && !plain.contains("solid")
        );
    }

    #[test]
    fn a_20_foot_path_at_24_inches_makes_11_copies() {
        let line = [Point::new(0.0, 0.0), Point::new(240.0, 0.0)];
        let st = path_stations(&line, 24.0, 0.0);
        assert_eq!(st.len(), 11);
        assert!((st[10].0.x - 240.0).abs() < 1e-6);
        // An offset of 12" drops the last copy.
        assert_eq!(path_stations(&line, 24.0, 12.0).len(), 10);
        // Around a corner the stations keep walking the path.
        let bent = [
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
        ];
        assert_eq!(path_stations(&bent, 24.0, 0.0).len(), 11);

        let mut p = Project::new("d");
        let mut d = Distribution::new(
            DistKind::Path,
            false,
            line.to_vec(),
            "core.tree",
            [12.0, 12.0, 60.0],
        );
        d.spacing = 24.0;
        let id = p.add_distribution(0, d);
        assert_eq!(p.distribution_copies(0, id), 11);
        // The record is pickable: a footprint around the path.
        let rec = p.floors[0].symbol(id).unwrap();
        assert!((rec.width - 240.0).abs() < 1e-6);
        // Changing the spacing regenerates; no duplicates are left.
        let mut d = rec.distribution.clone().unwrap();
        d.spacing = 48.0;
        p.floors[0]
            .symbols
            .iter_mut()
            .find(|s| s.id == id)
            .unwrap()
            .distribution = Some(d);
        assert_eq!(p.rebuild_distribution(0, id), Some(6));
        assert_eq!(p.distribution_copies(0, id), 6);
        assert_eq!(p.floors[0].symbols.len(), 7);
        // Deleting the record removes its copies.
        assert!(p.remove_symbol(0, id));
        assert!(p.floors[0].symbols.is_empty());
    }

    #[test]
    fn regions_fill_on_a_grid_or_at_random() {
        let rect = vec![
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 120.0),
            Point::new(0.0, 120.0),
        ];
        let mut d = Distribution::new(
            DistKind::Region,
            false,
            rect,
            "core.shrub",
            [12.0, 12.0, 24.0],
        );
        d.spacing = 24.0;
        assert_eq!(d.copies().len(), 50);
        // Cells are centred in their squares.
        assert!(d
            .copies()
            .iter()
            .all(|c| c.center.x > 0.0 && c.center.x < 240.0));
        d.offset = 20.0;
        assert!(d.copies().len() < 50);
        d.offset = 0.0;
        d.pattern = RegionPattern::Random;
        let a = d.copies();
        assert_eq!(a.len(), (240.0f64 * 120.0 / (24.0 * 24.0)).floor() as usize);
        assert_eq!(a, d.copies(), "same seed, same copies");
        d.seed = 7;
        assert_ne!(a, d.copies());
        // An L-shaped region leaves its notch empty.
        let l = vec![
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(120.0, 48.0),
            Point::new(48.0, 48.0),
            Point::new(48.0, 120.0),
            Point::new(0.0, 120.0),
        ];
        let mut d = Distribution::new(DistKind::Region, false, l, "x", [6.0, 6.0, 6.0]);
        d.spacing = 24.0;
        let c = d.copies();
        assert!(!c.is_empty() && c.iter().all(|c| c.center.x < 48.0 || c.center.y < 48.0));
    }

    #[test]
    fn scatter_rotation_and_size_vary_but_stay_bounded() {
        let line = vec![Point::new(0.0, 0.0), Point::new(240.0, 0.0)];
        let mut d = Distribution::new(DistKind::Path, true, line, "x", [10.0, 10.0, 10.0]);
        d.spacing = 24.0;
        d.scatter = 6.0;
        d.random_rotation = 45.0;
        d.random_size = 20.0;
        let c = d.copies();
        assert_eq!(c.len(), 11);
        assert!(c.iter().all(|c| c.center.y.abs() <= 6.0 + 1e-9));
        assert!(c
            .iter()
            .all(|c| (0.8 - 1e-9..=1.2 + 1e-9).contains(&c.scale)));
        assert!(c.iter().any(|c| c.angle != 0.0));
        // A "spline" of two points is just the line.
        assert_eq!(d.polyline().len(), 2);
        // Three points are sampled smoothly.
        d.points = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 50.0),
            Point::new(200.0, 0.0),
        ];
        assert!(d.polyline().len() > 3);
        assert!(d.path_length() > 200.0);
    }

    #[test]
    fn moving_the_record_moves_the_path_on_rebuild() {
        let mut p = Project::new("m");
        let d = Distribution::new(
            DistKind::Path,
            false,
            vec![Point::new(0.0, 0.0), Point::new(120.0, 0.0)],
            "x",
            [6.0, 6.0, 6.0],
        );
        let id = p.add_distribution(0, d);
        let to = p.floors[0].symbol(id).unwrap().position + Point::new(100.0, 40.0);
        p.move_symbol(0, id, to);
        assert_eq!(p.sync_moved_distributions(0), 1);
        let dist = p.floors[0]
            .symbol(id)
            .unwrap()
            .distribution
            .clone()
            .unwrap();
        assert_eq!(dist.points[0], Point::new(100.0, 40.0));
        assert_eq!(p.sync_moved_distributions(0), 0);
    }

    #[test]
    fn billboards_face_the_eye() {
        let pos = Point::new(100.0, 100.0);
        let front = |a: f64| Point::new(-a.to_radians().sin(), a.to_radians().cos());
        for eye in [
            Point::new(100.0, 400.0),
            Point::new(400.0, 100.0),
            Point::new(-50.0, -300.0),
            Point::new(-200.0, 180.0),
        ] {
            let a = billboard_angle(pos, eye).unwrap();
            let want = (eye - pos) * (1.0 / eye.dist(pos));
            assert!(front(a).dist(want) < 1e-9, "{eye:?} {a}");
        }
        assert_eq!(billboard_angle(pos, pos), None);
        assert_eq!(billboard_angle(pos, Point::new(100.0, 400.0)), Some(0.0));
    }
}
