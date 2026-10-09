//! Rope lights: a path with regularly spaced light sources (manual pp. 694,
//! 705; E-22, brief 25).
//!
//! A rope light is drawn by click and drag and edited like an open polyline.
//! It is a record of the electrical layer ([`ElectricalLayer::ropes`]
//! (crate::ElectricalLayer::ropes)), not a [`Device`](crate::Device): a path of
//! points, the [`RopeSpec`] of the Rope Light Specification (Elevation
//! Reference and Height, Distance Between Lights, Center Lights, Show Lights
//! and the Light Display Size) and a Treat as One Object switch that puts the
//! rope in the electrical schedule. The Rope Light Defaults of a plan are a
//! [`RopeSpec`] too ([`ElectricalDefaults::rope`](crate::ElectricalDefaults)).
//!
//! The older strip device ([`DeviceKind::RopeLight`](crate::DeviceKind)) still
//! reads and draws; it is a rope light of two points.

use plan_core::{Id, Point};
use serde::{Deserialize, Serialize};

/// What a rope light's height is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RopeReference {
    /// Up from the finished floor.
    #[default]
    Floor,
    /// Down from the ceiling.
    Ceiling,
}

impl RopeReference {
    pub const ALL: [RopeReference; 2] = [RopeReference::Floor, RopeReference::Ceiling];

    pub fn name(self) -> &'static str {
        match self {
            RopeReference::Floor => "Floor",
            RopeReference::Ceiling => "Ceiling",
        }
    }
}

/// The settings of a rope light (Rope Light Specification, General panel);
/// the Rope Light Defaults hold one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RopeSpec {
    pub reference: RopeReference,
    /// Height of the top of the rope profile from the reference, inches.
    pub height: f64,
    /// Distance between the centers of the light sources, inches.
    pub spacing: f64,
    /// Center the lights along the rope; off puts one at the start.
    pub center_lights: bool,
    /// Draw the light sources as circles in the plan.
    pub show_lights: bool,
    /// Diameter of those circles, inches.
    pub light_size: f64,
    /// The rope is one object in the electrical schedule (Schedule panel).
    pub treat_as_object: bool,
    /// Width and height of the strip profile, inches.
    pub profile_width: f64,
    pub profile_height: f64,
}

impl Default for RopeSpec {
    fn default() -> Self {
        Self {
            reference: RopeReference::Floor,
            height: 84.0,
            spacing: 6.0,
            center_lights: true,
            show_lights: true,
            light_size: 1.0,
            treat_as_object: true,
            profile_width: 0.75,
            profile_height: 0.75,
        }
    }
}

impl RopeSpec {
    /// The spacing to use: at least half an inch so a typo never asks for a
    /// million lights.
    pub fn spacing_clamped(&self) -> f64 {
        self.spacing.max(0.5)
    }
}

/// A rope light in the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RopeLightPath {
    /// Unique among the ropes of one layer; `0` means "not yet added".
    pub id: Id,
    /// The path, plan inches. At least two points.
    pub points: Vec<Point>,
    /// The path closes on itself (a tray ceiling's loop).
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub spec: RopeSpec,
    #[serde(default)]
    pub label: String,
    /// The tray ceiling the path was made from, if any (0 = drawn by hand).
    #[serde(default)]
    pub tray: Id,
}

/// The distance along `points` of every segment end (the first is 0).
fn cumulative(points: &[Point], closed: bool) -> Vec<f64> {
    let mut out = vec![0.0];
    let n = points.len();
    let segs = if closed && n > 2 {
        n
    } else {
        n.saturating_sub(1)
    };
    for i in 0..segs {
        let last = out[i];
        out.push(last + points[i].dist(points[(i + 1) % n]));
    }
    out
}

/// The point `d` inches along the polyline `points`.
fn point_along(points: &[Point], closed: bool, cum: &[f64], d: f64) -> Point {
    let n = points.len();
    let segs = cum.len() - 1;
    for i in 0..segs {
        if d <= cum[i + 1] + 1e-9 || i + 1 == segs {
            let len = cum[i + 1] - cum[i];
            let t = if len < 1e-9 {
                0.0
            } else {
                ((d - cum[i]) / len).clamp(0.0, 1.0)
            };
            let a = points[i];
            let b = points[(i + 1) % n];
            let _ = closed;
            return Point::lerp(a, b, t);
        }
    }
    points[0]
}

/// The positions of the light sources along a path: `spacing` apart, either
/// centered on the path (the margins at both ends equal) or from the start.
pub fn light_positions(points: &[Point], closed: bool, spacing: f64, center: bool) -> Vec<Point> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let spacing = spacing.max(0.5);
    let cum = cumulative(points, closed);
    let total = cum[cum.len() - 1];
    if total < 1e-9 {
        return vec![points[0]];
    }
    let ring = closed && points.len() > 2;
    let distances: Vec<f64> = if ring && center {
        // A loop has no ends: share the length out evenly.
        let n = ((total / spacing).round() as usize).max(1);
        let step = total / n as f64;
        (0..n).map(|i| step * (i as f64 + 0.5)).collect()
    } else if center {
        let n = ((total / spacing + 1e-9).floor() as usize).max(1);
        let start = (total - spacing * (n - 1) as f64) * 0.5;
        (0..n).map(|i| start + spacing * i as f64).collect()
    } else {
        let limit = if ring { total - 1e-9 } else { total + 1e-9 };
        let mut v = Vec::new();
        let mut d = 0.0;
        while d <= limit {
            v.push(d);
            d += spacing;
        }
        v
    };
    distances
        .into_iter()
        .map(|d| point_along(points, closed, &cum, d))
        .collect()
}

impl RopeLightPath {
    /// A rope light along `points`.
    pub fn new(points: Vec<Point>, spec: RopeSpec) -> Self {
        Self {
            id: 0,
            points,
            closed: false,
            spec,
            label: String::new(),
            tray: 0,
        }
    }

    /// Length of the path, inches.
    pub fn length(&self) -> f64 {
        let cum = cumulative(&self.points, self.closed);
        cum[cum.len() - 1]
    }

    /// The light sources, plan inches.
    pub fn lights(&self) -> Vec<Point> {
        light_positions(
            &self.points,
            self.closed,
            self.spec.spacing_clamped(),
            self.spec.center_lights,
        )
    }

    /// Height of the top of the rope above the finished floor, inches, in a
    /// room whose ceiling is `ceiling` high.
    pub fn top_elevation(&self, ceiling: f64) -> f64 {
        match self.spec.reference {
            RopeReference::Floor => self.spec.height,
            RopeReference::Ceiling => ceiling - self.spec.height,
        }
    }

    /// The segments as `(start, end)` pairs.
    pub fn segments(&self) -> Vec<(Point, Point)> {
        let n = self.points.len();
        let count = if self.closed && n > 2 {
            n
        } else {
            n.saturating_sub(1)
        };
        (0..count)
            .map(|i| (self.points[i], self.points[(i + 1) % n]))
            .collect()
    }

    /// The vertex nearest `p` within `tol` inches.
    pub fn vertex_at(&self, p: Point, tol: f64) -> Option<usize> {
        self.points
            .iter()
            .enumerate()
            .map(|(i, v)| (i, v.dist(p)))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// The segment whose midpoint handle is nearest `p` within `tol` inches
    /// (the handle that inserts a vertex).
    pub fn midpoint_at(&self, p: Point, tol: f64) -> Option<usize> {
        self.segments()
            .iter()
            .enumerate()
            .map(|(i, (a, b))| (i, Point::lerp(*a, *b, 0.5).dist(p)))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Distance from `p` to the path.
    pub fn distance_to(&self, p: Point) -> f64 {
        self.segments()
            .iter()
            .map(|(a, b)| plan_core::geometry::dist_to_segment(p, *a, *b))
            .fold(f64::INFINITY, f64::min)
    }

    /// Inserts a vertex at `at` after vertex `segment`; returns its index.
    pub fn insert_vertex(&mut self, segment: usize, at: Point) -> usize {
        let i = (segment + 1).min(self.points.len());
        self.points.insert(i, at);
        i
    }

    /// Removes vertex `i` while at least two remain.
    pub fn remove_vertex(&mut self, i: usize) -> bool {
        if self.points.len() <= 2 || i >= self.points.len() {
            return false;
        }
        self.points.remove(i);
        true
    }

    /// Moves vertex `i` to `to`.
    pub fn move_vertex(&mut self, i: usize, to: Point) -> bool {
        match self.points.get_mut(i) {
            Some(p) => {
                *p = to;
                true
            }
            None => false,
        }
    }

    /// Moves the whole path by `d`.
    pub fn translate(&mut self, d: Point) {
        for p in &mut self.points {
            *p = *p + d;
        }
    }

    /// A rope light along a tray ceiling's rope path (`points` is its loop at
    /// `elevation` above the floor, from `plan_core::tray::rope_light_paths`).
    pub fn from_tray_path(
        tray: Id,
        name: &str,
        points: &[Point],
        elevation: f64,
        defaults: &RopeSpec,
    ) -> Self {
        let mut spec = defaults.clone();
        spec.reference = RopeReference::Floor;
        spec.height = elevation;
        let mut r = RopeLightPath::new(points.to_vec(), spec);
        r.closed = points.len() > 2;
        r.label = name.to_string();
        r.tray = tray;
        r
    }
}
