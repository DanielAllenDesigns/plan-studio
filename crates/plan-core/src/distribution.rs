//! Distribution Region and Path options (reference manual "Distributed
//! Objects", pp. 1091 to 1098; parity rows CB-468..CB-474).
//!
//! [`crate::images::Distribution`] is the record (a path or outline, the
//! library item, spacing). [`DistOptions`] is everything the Distribution
//! Region and Path Specification dialogs add: Show Objects / Show Region,
//! Auto Spacing, the Standard Grid / Alternate Grid / Evenly Scattered
//! distribution style, the Absolute / Relative / Random angle, the X and Y
//! offset from the polyline start or centre, a scaling range, and for paths
//! the number-of-objects mode, minimum and maximum distance, Center
//! Objects, Start Offset and the side-to-side positioning. A record with
//! options (`Distribution::options`) makes its copies with [`copies_with`];
//! one without keeps the older behaviour.
//!
//! [`Project::explode_distribution`] is Explode Distributed Object: the
//! copies stay as ordinary symbols and the record goes, for good.

use crate::geometry::{point_in_polygon, polygon_area, Point};
use crate::images::{DistCopy, DistKind, Distribution, MAX_COPIES, MIN_SPACING};
use crate::model::{Id, Project};
use serde::{Deserialize, Serialize};

/// How the objects of a region are laid out (p. 1094).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RegionStyle {
    /// Rows and columns of the plan's X/Y grid.
    #[default]
    StandardGrid,
    /// Rows and columns that follow the first edge of the region.
    AlternateGrid,
    /// Evenly spread, in staggered rows, not on a grid.
    EvenlyScattered,
}

impl RegionStyle {
    pub const ALL: [RegionStyle; 3] = [
        RegionStyle::StandardGrid,
        RegionStyle::AlternateGrid,
        RegionStyle::EvenlyScattered,
    ];

    pub fn name(self) -> &'static str {
        match self {
            RegionStyle::StandardGrid => "Standard Grid",
            RegionStyle::AlternateGrid => "Alternate Grid",
            RegionStyle::EvenlyScattered => "Evenly Scattered",
        }
    }
}

/// What the Angle is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AngleMode {
    /// From the plan's positive X direction.
    #[default]
    Absolute,
    /// From the first edge of the region, or the edge of the path the object
    /// stands on.
    Relative,
    Random,
}

impl AngleMode {
    pub const ALL: [AngleMode; 3] = [AngleMode::Absolute, AngleMode::Relative, AngleMode::Random];

    pub fn name(self) -> &'static str {
        match self {
            AngleMode::Absolute => "Absolute Angle",
            AngleMode::Relative => "Relative Angle",
            AngleMode::Random => "Random Angle",
        }
    }
}

/// Where a region's X and Y offsets are measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OffsetFrom {
    #[default]
    PolylineStart,
    PolylineCenter,
}

/// Which side(s) of a path the objects offset to (p. 1096).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SideMode {
    /// The left of the path, seen from its start.
    #[default]
    OneSided,
    AlternateSides,
    RandomSides,
}

impl SideMode {
    pub const ALL: [SideMode; 3] = [
        SideMode::OneSided,
        SideMode::AlternateSides,
        SideMode::RandomSides,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SideMode::OneSided => "One Sided",
            SideMode::AlternateSides => "Alternate Sides",
            SideMode::RandomSides => "Random Sides",
        }
    }
}

/// The options of the Distribution Region/Path Specification dialogs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DistOptions {
    /// Show Objects.
    pub show_objects: bool,
    /// Show Region / Show Path.
    pub show_outline: bool,
    /// Spacing becomes 10 percent more than the larger side of the object.
    pub auto_spacing: bool,
    /// Region: how far an object may be randomly offset from its place.
    pub max_offset: f64,
    pub region_style: RegionStyle,
    pub angle: f64,
    pub angle_mode: AngleMode,
    pub offset_x: f64,
    pub offset_y: f64,
    pub offset_from: OffsetFrom,
    pub scaling: bool,
    /// Smallest and largest scale, percent of the object's size.
    pub scale_min: f64,
    pub scale_max: f64,
    /// Path: distribute a number of objects evenly instead of by distance.
    pub by_count: bool,
    pub count: u32,
    /// Path: the greatest distance between centres (the least is the
    /// record's `spacing`); equal or smaller means a fixed distance.
    pub max_distance: f64,
    /// Path: centre the objects along the path.
    pub center_objects: bool,
    /// Path: distance of the first object from the path's start (the
    /// record's `offset`); used when Center Objects is off.
    pub start_offset: f64,
    /// Path: side-to-side positioning.
    pub side_min: f64,
    pub side_max: f64,
    pub side_mode: SideMode,
}

impl Default for DistOptions {
    fn default() -> Self {
        Self {
            show_objects: true,
            show_outline: true,
            auto_spacing: false,
            max_offset: 0.0,
            region_style: RegionStyle::StandardGrid,
            angle: 0.0,
            angle_mode: AngleMode::Absolute,
            offset_x: 0.0,
            offset_y: 0.0,
            offset_from: OffsetFrom::PolylineStart,
            scaling: false,
            scale_min: 100.0,
            scale_max: 100.0,
            by_count: false,
            count: 5,
            max_distance: 0.0,
            center_objects: false,
            start_offset: 0.0,
            side_min: 0.0,
            side_max: 0.0,
            side_mode: SideMode::OneSided,
        }
    }
}

impl DistOptions {
    /// Options that reproduce a record's older settings, so opening it in
    /// the new dialog changes nothing.
    pub fn from_legacy(d: &Distribution) -> DistOptions {
        let mut o = DistOptions {
            max_offset: d.scatter,
            start_offset: d.offset,
            side_min: d.side_offset,
            side_max: d.side_offset,
            ..DistOptions::default()
        };
        if d.align_to_path && d.kind == DistKind::Path {
            o.angle_mode = AngleMode::Relative;
        }
        if d.random_rotation >= 359.0 {
            o.angle_mode = AngleMode::Random;
        }
        if d.random_size > 0.0 {
            o.scaling = true;
            o.scale_min = (100.0 - d.random_size).max(5.0);
            o.scale_max = 100.0 + d.random_size;
        }
        o.region_style = match d.pattern {
            crate::images::RegionPattern::Grid => RegionStyle::StandardGrid,
            crate::images::RegionPattern::Random => RegionStyle::EvenlyScattered,
        };
        o
    }
}

/// Auto Spacing: 10 percent more than the larger side of the object in
/// plan (p. 1093).
pub fn auto_spacing(item_size: [f64; 3]) -> f64 {
    (item_size[0].max(item_size[1]) * 1.1).max(MIN_SPACING)
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

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        if hi <= lo {
            lo
        } else {
            lo + self.next() * (hi - lo)
        }
    }
}

fn bbox_center(line: &[Point]) -> Point {
    let (mut lo, mut hi) = (line[0], line[0]);
    for p in line {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5)
}

/// Direction of the first edge of `line` in degrees (0 for a degenerate line).
fn first_edge_angle(line: &[Point]) -> f64 {
    line.windows(2)
        .find(|w| w[0].dist(w[1]) > 1e-9)
        .map_or(0.0, |w| (w[1] - w[0]).angle().to_degrees())
}

fn turn(v: Point, deg: f64) -> Point {
    crate::details::rotate_deg(v, deg)
}

fn far_enough(line: &[Point], p: Point, margin: f64) -> bool {
    if margin <= 0.0 {
        return true;
    }
    let n = line.len();
    (0..n)
        .map(|i| crate::geometry::dist_to_segment(p, line[i], line[(i + 1) % n]))
        .fold(f64::INFINITY, f64::min)
        >= margin - 1e-9
}

/// Region centres for `style`: a grid anchored at `origin`, in the frame
/// rotated by `frame` degrees, kept inside `poly` (and `margin` from its
/// edges).
fn region_centres(
    poly: &[Point],
    style: RegionStyle,
    spacing: f64,
    origin: Point,
    frame: f64,
    margin: f64,
) -> Vec<Point> {
    let mut out = Vec::new();
    // The polygon in the grid frame: its box says how many steps reach it.
    let local: Vec<Point> = poly.iter().map(|p| turn(*p - origin, -frame)).collect();
    let (mut lo, mut hi) = (local[0], local[0]);
    for p in &local {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let (row_step, stagger) = match style {
        RegionStyle::EvenlyScattered => (spacing * 0.866_025_403_8, true),
        _ => (spacing, false),
    };
    let (c0, c1) = (
        (lo.x / spacing).floor() as i64 - 1,
        (hi.x / spacing).ceil() as i64 + 1,
    );
    let (r0, r1) = (
        (lo.y / row_step).floor() as i64 - 1,
        (hi.y / row_step).ceil() as i64 + 1,
    );
    for r in r0..=r1 {
        for c in c0..=c1 {
            let shift = if stagger && r.rem_euclid(2) == 1 {
                spacing * 0.5
            } else {
                0.0
            };
            let l = Point::new(c as f64 * spacing + shift, r as f64 * row_step);
            let p = origin + turn(l, frame);
            if point_in_polygon(p, poly) && far_enough(poly, p, margin) {
                out.push(p);
                if out.len() >= MAX_COPIES {
                    return out;
                }
            }
        }
    }
    out
}

/// The copies a record makes with `o`.
pub fn copies_with(d: &Distribution, o: &DistOptions) -> Vec<DistCopy> {
    let mut rng = Rng(d.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA076_1D64);
    let line = d.polyline();
    if line.len() < 2 {
        return Vec::new();
    }
    let spacing = if o.auto_spacing {
        auto_spacing(d.item_size)
    } else {
        d.spacing
    }
    .max(MIN_SPACING);
    let first = first_edge_angle(&line);
    // (centre, direction of the path there), before the scatter and the
    // angle options.
    let mut base: Vec<(Point, f64)> = Vec::new();
    match d.kind {
        DistKind::Region => {
            if line.len() < 3 || polygon_area(&line).abs() < 1e-9 {
                return Vec::new();
            }
            let start = match o.offset_from {
                OffsetFrom::PolylineStart => line[0],
                OffsetFrom::PolylineCenter => bbox_center(&line),
            };
            let (origin, frame) = match o.region_style {
                RegionStyle::AlternateGrid => (start, first),
                _ => (start, 0.0),
            };
            // The offsets are in plan X and Y (not the frame's).
            let origin = origin + Point::new(o.offset_x, o.offset_y);
            for p in region_centres(&line, o.region_style, spacing, origin, frame, d.offset) {
                base.push((p, first));
            }
        }
        DistKind::Path => {
            let total: f64 = line.windows(2).map(|w| w[0].dist(w[1])).sum();
            if total < 1e-9 {
                return Vec::new();
            }
            let mut stations: Vec<f64> = Vec::new();
            if o.by_count {
                let n = o.count.clamp(1, MAX_COPIES as u32) as usize;
                if n == 1 {
                    stations.push(total * 0.5);
                } else {
                    for i in 0..n {
                        stations.push(total * i as f64 / (n - 1) as f64);
                    }
                }
            } else {
                // Distances between centres: fixed, or random between the
                // least and the greatest.
                let mut gaps: Vec<f64> = Vec::new();
                let mut sum = 0.0;
                let step_max = o.max_distance.max(spacing);
                while sum + spacing <= total + 1e-9 && gaps.len() < MAX_COPIES {
                    let g = rng.range(spacing, step_max);
                    if sum + g > total + 1e-9 {
                        break;
                    }
                    gaps.push(g);
                    sum += g;
                }
                let span = sum;
                let start = if o.center_objects {
                    (total - span) * 0.5
                } else {
                    o.start_offset.max(0.0)
                };
                let mut at = start;
                stations.push(at);
                for g in gaps {
                    at += g;
                    stations.push(at);
                }
                stations.retain(|s| *s <= total + 1e-6);
            }
            for (i, s) in stations.iter().enumerate() {
                let (p, tangent) = station_at(&line, *s);
                let side = rng.range(o.side_min, o.side_max.max(o.side_min));
                let sign = match o.side_mode {
                    SideMode::OneSided => 1.0,
                    SideMode::AlternateSides => {
                        if i % 2 == 0 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    SideMode::RandomSides => {
                        if rng.next() < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                };
                let left = Point::new(-tangent.to_radians().sin(), tangent.to_radians().cos());
                base.push((p + left * (side * sign), tangent));
            }
        }
    }
    base.truncate(MAX_COPIES);
    base.into_iter()
        .map(|(p, tangent)| {
            let mut center = p;
            if d.kind == DistKind::Region
                && o.max_offset > 0.0
                && o.region_style != RegionStyle::EvenlyScattered
            {
                let r = o.max_offset * rng.next().sqrt();
                let a = rng.next() * std::f64::consts::TAU;
                center = center + Point::new(a.cos(), a.sin()) * r;
            }
            let angle = match o.angle_mode {
                AngleMode::Absolute => o.angle,
                AngleMode::Relative => tangent + o.angle,
                AngleMode::Random => rng.next() * 360.0,
            };
            let scale = if o.scaling {
                rng.range(o.scale_min.min(o.scale_max), o.scale_max.max(o.scale_min)) / 100.0
            } else {
                1.0
            };
            DistCopy {
                center,
                angle: angle.rem_euclid(360.0),
                scale: scale.max(0.05),
            }
        })
        .collect()
}

/// The point `s` along `line` and the path's direction there, degrees.
fn station_at(line: &[Point], s: f64) -> (Point, f64) {
    let mut walked = 0.0;
    let mut last = (line[0], 0.0);
    for w in line.windows(2) {
        let len = w[0].dist(w[1]);
        if len < 1e-12 {
            continue;
        }
        let dir = (w[1] - w[0]) * (1.0 / len);
        let ang = dir.angle().to_degrees().rem_euclid(360.0);
        last = (w[1], ang);
        if s <= walked + len + 1e-9 {
            return (w[0] + dir * (s - walked).clamp(0.0, len), ang);
        }
        walked += len;
    }
    last
}

impl Project {
    /// Explode Distributed Object (p. 1092): the record goes, its copies stay
    /// as independent symbols. Returns how many copies were released, `None`
    /// when `id` is not a distribution record.
    pub fn explode_distribution(&mut self, floor: usize, id: Id) -> Option<usize> {
        let f = self.floors.get_mut(floor)?;
        if !f
            .symbols
            .iter()
            .any(|s| s.id == id && s.distribution.is_some())
        {
            return None;
        }
        f.symbols.retain(|s| s.id != id);
        let mut n = 0;
        for s in &mut f.symbols {
            if s.owner == Some(id) {
                s.owner = None;
                n += 1;
            }
        }
        f.groups
            .retain(|g| !g.members.contains(&crate::groups::ObjectRef::Symbol(id)));
        Some(n)
    }

    /// Replace the object a record distributes (Replace From Library): the
    /// new item and size, then a rebuild. Returns the number of copies.
    pub fn replace_distributed_item(
        &mut self,
        floor: usize,
        id: Id,
        item: &str,
        size: [f64; 3],
    ) -> Option<usize> {
        let f = self.floors.get_mut(floor)?;
        let rec = f.symbols.iter_mut().find(|s| s.id == id)?;
        let d = rec.distribution.as_mut()?;
        d.item = item.to_string();
        d.item_size = size;
        self.rebuild_distribution(floor, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(side: f64) -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(side, 0.0),
            Point::new(side, side),
            Point::new(0.0, side),
        ]
    }

    fn region(side: f64, spacing: f64) -> Distribution {
        let mut d = Distribution::new(
            DistKind::Region,
            false,
            square(side),
            "plant",
            [12.0, 12.0, 24.0],
        );
        d.spacing = spacing;
        d
    }

    fn path(len: f64, spacing: f64) -> Distribution {
        let mut d = Distribution::new(
            DistKind::Path,
            false,
            vec![Point::new(0.0, 0.0), Point::new(len, 0.0)],
            "post",
            [6.0, 6.0, 36.0],
        );
        d.spacing = spacing;
        d
    }

    #[test]
    fn auto_spacing_is_ten_percent_over_the_larger_side() {
        assert!((auto_spacing([20.0, 30.0, 5.0]) - 33.0).abs() < 1e-9);
        let d = region(120.0, 1.0);
        let o = DistOptions {
            auto_spacing: true,
            ..DistOptions::default()
        };
        let n = copies_with(&d, &o).len();
        // 12 x 12 object, 13.2 spacing: about 9 across.
        assert!((64..=100).contains(&n), "{n}");
    }

    #[test]
    fn standard_and_alternate_grids_differ_for_a_turned_region() {
        let turned: Vec<Point> = square(120.0)
            .into_iter()
            .map(|p| crate::details::rotate_deg(p, 30.0))
            .collect();
        let mut d = region(120.0, 24.0);
        d.points = turned;
        let std = copies_with(&d, &DistOptions::default());
        let alt = copies_with(
            &d,
            &DistOptions {
                region_style: RegionStyle::AlternateGrid,
                ..DistOptions::default()
            },
        );
        let sc = copies_with(
            &d,
            &DistOptions {
                region_style: RegionStyle::EvenlyScattered,
                ..DistOptions::default()
            },
        );
        assert!(!std.is_empty() && !alt.is_empty() && !sc.is_empty());
        assert_ne!(std.len(), 0);
        // The alternate grid lines up with the first edge: all centres sit on
        // multiples of the spacing in that frame.
        let o = d.points[0];
        let ang = (d.points[1] - d.points[0]).angle().to_degrees();
        for c in &alt {
            let l = crate::details::rotate_deg(c.center - o, -ang);
            assert!((l.x / 24.0 - (l.x / 24.0).round()).abs() < 1e-6, "{l:?}");
        }
        // Scattered rows are staggered: not every x is on the same lattice.
        let off_lattice = sc
            .iter()
            .any(|c| ((c.center.x - d.points[0].x) / 24.0).fract().abs() > 0.1);
        assert!(off_lattice);
    }

    #[test]
    fn offsets_move_the_grid_from_the_start_or_the_centre() {
        let d = region(100.0, 20.0);
        let a = copies_with(&d, &DistOptions::default());
        let b = copies_with(
            &d,
            &DistOptions {
                offset_x: 5.0,
                offset_y: 5.0,
                ..DistOptions::default()
            },
        );
        let c = copies_with(
            &d,
            &DistOptions {
                offset_from: OffsetFrom::PolylineCenter,
                ..DistOptions::default()
            },
        );
        assert_ne!(a[0].center, b[0].center);
        assert!(a.iter().all(|k| (k.center.x / 20.0).fract().abs() < 1e-9));
        assert!(c
            .iter()
            .all(|k| ((k.center.x - 50.0) / 20.0).fract().abs() < 1e-9));
    }

    #[test]
    fn angles_and_scales_follow_their_options() {
        let d = path(100.0, 25.0);
        let abs = copies_with(
            &d,
            &DistOptions {
                angle: 30.0,
                ..DistOptions::default()
            },
        );
        assert!(abs.iter().all(|c| (c.angle - 30.0).abs() < 1e-9));
        let mut bent = path(100.0, 25.0);
        bent.points = vec![
            Point::new(0.0, 0.0),
            Point::new(50.0, 0.0),
            Point::new(50.0, 50.0),
        ];
        let rel = copies_with(
            &bent,
            &DistOptions {
                angle_mode: AngleMode::Relative,
                angle: 10.0,
                ..DistOptions::default()
            },
        );
        assert!(rel.iter().any(|c| (c.angle - 10.0).abs() < 1e-6));
        assert!(
            rel.iter().any(|c| (c.angle - 100.0).abs() < 1e-6),
            "past the corner it follows the second edge"
        );
        let random = copies_with(
            &d,
            &DistOptions {
                angle_mode: AngleMode::Random,
                scaling: true,
                scale_min: 50.0,
                scale_max: 150.0,
                ..DistOptions::default()
            },
        );
        assert!(random
            .windows(2)
            .any(|w| (w[0].angle - w[1].angle).abs() > 1e-6));
        assert!(random.iter().all(|c| (0.5..=1.5).contains(&c.scale)));
        assert!(random
            .windows(2)
            .any(|w| (w[0].scale - w[1].scale).abs() > 1e-9));
        // Same seed, same result.
        let again = copies_with(
            &d,
            &DistOptions {
                angle_mode: AngleMode::Random,
                scaling: true,
                scale_min: 50.0,
                scale_max: 150.0,
                ..DistOptions::default()
            },
        );
        assert_eq!(random, again);
    }

    #[test]
    fn path_counts_centres_and_sides() {
        let d = path(100.0, 30.0);
        // A number of objects, ends included.
        let five = copies_with(
            &d,
            &DistOptions {
                by_count: true,
                count: 5,
                ..DistOptions::default()
            },
        );
        assert_eq!(five.len(), 5);
        assert!((five[0].center.x).abs() < 1e-9 && (five[4].center.x - 100.0).abs() < 1e-9);
        // Distance: 0, 30, 60, 90.
        let dist = copies_with(&d, &DistOptions::default());
        assert_eq!(dist.len(), 4);
        // Centred: the 90 inch run sits in the middle of 100.
        let centred = copies_with(
            &d,
            &DistOptions {
                center_objects: true,
                ..DistOptions::default()
            },
        );
        assert!((centred[0].center.x - 5.0).abs() < 1e-9);
        let started = copies_with(
            &d,
            &DistOptions {
                start_offset: 10.0,
                ..DistOptions::default()
            },
        );
        assert!((started[0].center.x - 10.0).abs() < 1e-9);
        assert_eq!(started.len(), 4, "10, 40, 70, 100");
        // Alternate sides step left and right of the path.
        let alt = copies_with(
            &d,
            &DistOptions {
                side_min: 6.0,
                side_max: 6.0,
                side_mode: SideMode::AlternateSides,
                ..DistOptions::default()
            },
        );
        assert!((alt[0].center.y - 6.0).abs() < 1e-9 && (alt[1].center.y + 6.0).abs() < 1e-9);
        let one = copies_with(
            &d,
            &DistOptions {
                side_min: 6.0,
                side_max: 6.0,
                ..DistOptions::default()
            },
        );
        assert!(one.iter().all(|c| (c.center.y - 6.0).abs() < 1e-9));
        // A maximum distance makes the gaps vary but never leave the range.
        let varied = copies_with(
            &d,
            &DistOptions {
                max_distance: 45.0,
                ..DistOptions::default()
            },
        );
        for w in varied.windows(2) {
            let g = w[1].center.x - w[0].center.x;
            assert!((30.0 - 1e-9..=45.0 + 1e-9).contains(&g), "{g}");
        }
    }

    #[test]
    fn the_record_uses_its_options_and_can_be_exploded() {
        let mut p = Project::new("d");
        let mut d = region(100.0, 25.0);
        d.options = Some(DistOptions {
            region_style: RegionStyle::EvenlyScattered,
            ..DistOptions::default()
        });
        let id = p.add_distribution(0, d);
        let n = p.distribution_copies(0, id);
        assert!(n > 0);
        let json = p.to_json().unwrap();
        let back = Project::from_json(&json).unwrap();
        assert_eq!(back.distribution_copies(0, id), n);
        let mut q = back;
        assert_eq!(q.explode_distribution(0, id), Some(n));
        assert_eq!(q.distribution_copies(0, id), 0);
        assert_eq!(q.floors[0].symbols.len(), n, "the copies stay");
        assert!(q.floors[0].symbols.iter().all(|s| s.owner.is_none()));
        assert_eq!(q.explode_distribution(0, id), None);
        // Legacy settings carry over into the options.
        let mut old = path(100.0, 20.0);
        old.align_to_path = true;
        old.side_offset = 4.0;
        let o = DistOptions::from_legacy(&old);
        assert_eq!((o.angle_mode, o.side_min), (AngleMode::Relative, 4.0));
    }
}
