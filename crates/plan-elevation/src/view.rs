//! Free-angle views: a section or elevation looking along any plan direction.
//!
//! The axis-aligned [`crate::ViewDir`] pipeline only knows the four compass
//! views. A [`FreeView`] turns the scene about the vertical axis into the
//! camera frame first (the camera line becomes the plane `z = 0` of a Front
//! view), then runs the same hidden-line pipeline and clips the result to the
//! length of the camera line. Drawing space is then *view-local*: X runs along
//! the camera line from its left end (looking along the view direction) to its
//! right end, with 0 at the line's centre; Y is height.

use crate::drawing::{Drawing, Line2, Region};
use crate::hlr;
use crate::projection::{Projection, ViewDir};
use crate::styles::ObjectWeights;
use crate::Options;
use plan_3d::{Mesh, Scene, Vertex};
use plan_core::Point;

/// The camera line of a free-angle section or elevation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FreeView {
    /// Centre of the camera line, plan inches.
    pub origin: Point,
    /// Viewing direction, degrees counter-clockwise from plan +X.
    pub view_deg: f64,
    /// Half the length of the camera line; the drawing is cut off beyond it.
    /// `None` keeps everything.
    pub half_width: Option<f64>,
}

/// `(sin, cos)` of an angle in degrees, exact on the four axes.
fn sin_cos(deg: f64) -> (f64, f64) {
    let quarter = (deg / 90.0).round();
    if (deg - quarter * 90.0).abs() < 1e-9 {
        match (quarter as i64).rem_euclid(4) {
            0 => (0.0, 1.0),
            1 => (1.0, 0.0),
            2 => (0.0, -1.0),
            _ => (-1.0, 0.0),
        }
    } else {
        deg.to_radians().sin_cos()
    }
}

impl FreeView {
    /// A view at `origin` looking along `view_deg`, with no width limit.
    pub fn new(origin: Point, view_deg: f64) -> FreeView {
        FreeView {
            origin,
            view_deg,
            half_width: None,
        }
    }

    /// The same view cut off `half_width` inches either side of the centre.
    pub fn with_half_width(mut self, half_width: f64) -> FreeView {
        self.half_width = Some(half_width.max(0.0));
        self
    }

    /// The unit viewing direction in the plan.
    pub fn dir(&self) -> Point {
        let (s, c) = sin_cos(self.view_deg);
        Point::new(c, s)
    }

    /// The unit vector along the camera line, from its left end to its right
    /// end as the viewer sees it (the direction turned 90 degrees clockwise).
    pub fn tangent(&self) -> Point {
        let d = self.dir();
        Point::new(d.y, -d.x)
    }

    /// The compass [`ViewDir`] this view coincides with, if it is square to an axis.
    pub fn axis(&self) -> Option<ViewDir> {
        let d = self.dir();
        let at = |x: f64, y: f64| (d.x - x).abs() < 1e-9 && (d.y - y).abs() < 1e-9;
        if at(0.0, 1.0) {
            Some(ViewDir::Front)
        } else if at(0.0, -1.0) {
            Some(ViewDir::Back)
        } else if at(1.0, 0.0) {
            Some(ViewDir::Left)
        } else if at(-1.0, 0.0) {
            Some(ViewDir::Right)
        } else {
            None
        }
    }

    /// A plan point in the view frame: `x` along the camera line from its
    /// centre (right is positive), `y` the distance ahead of the line.
    pub fn to_view(&self, p: Point) -> Point {
        let v = p - self.origin;
        Point::new(self.tangent().dot(v), self.dir().dot(v))
    }

    /// Is the plan point `p` kept by the view: ahead of the line, within its
    /// width and within `depth` of it?
    pub fn contains(&self, p: Point, depth: Option<f64>) -> bool {
        let v = self.to_view(p);
        v.y >= -1e-6
            && self.half_width.is_none_or(|hw| v.x.abs() <= hw + 1e-6)
            && depth.is_none_or(|d| v.y <= d + 1e-6)
    }
}

/// The scene turned into the view frame: scene X along the camera line, scene
/// Z the negated distance ahead of it, so the view is a plain Front view with
/// its cutting plane at `z = 0`.
pub fn view_scene(scene: &Scene, view: &FreeView) -> Scene {
    let (t, d) = (view.tangent(), view.dir());
    let o = view.origin;
    let meshes = scene
        .meshes
        .iter()
        .map(|m| Mesh {
            vertices: m
                .vertices
                .iter()
                .map(|v| {
                    // Scene Z is the negated plan Y.
                    let p = Point::new(f64::from(v.position[0]), -f64::from(v.position[2])) - o;
                    let n = Point::new(f64::from(v.normal[0]), -f64::from(v.normal[2]));
                    Vertex {
                        position: [t.dot(p) as f32, v.position[1], -(d.dot(p)) as f32],
                        normal: [t.dot(n) as f32, v.normal[1], -(d.dot(n)) as f32],
                        ..*v
                    }
                })
                .collect(),
            ..m.clone()
        })
        .collect();
    Scene { meshes }
}

/// Keep the part of a ring on one side of the vertical line `x = edge`.
fn clip_half(ring: &[Point], edge: f64, keep_right: bool) -> Vec<Point> {
    let inside = |p: Point| if keep_right { p.x >= edge } else { p.x <= edge };
    let mut out = Vec::with_capacity(ring.len() + 2);
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let (ka, kb) = (inside(a), inside(b));
        if ka {
            out.push(a);
        }
        if ka != kb {
            let t = (edge - a.x) / (b.x - a.x);
            out.push(Point::new(edge, a.y + (b.y - a.y) * t));
        }
    }
    out
}

/// Clip a polygon ring to `lo <= x <= hi` (Sutherland-Hodgman); empty when
/// nothing is left.
fn clip_ring(ring: &[Point], lo: f64, hi: f64) -> Vec<Point> {
    let right = clip_half(ring, lo, true);
    if right.len() < 3 {
        return Vec::new();
    }
    let both = clip_half(&right, hi, false);
    if both.len() < 3 {
        Vec::new()
    } else {
        both
    }
}

/// Cut a drawing off outside `lo <= x <= hi`: lines are trimmed, regions
/// clipped, text anchored outside is dropped.
pub fn clip_x(drawing: &mut Drawing, lo: f64, hi: f64) {
    let mut lines: Vec<Line2> = Vec::with_capacity(drawing.lines.len());
    for l in &drawing.lines {
        let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
        let dx = l.b.x - l.a.x;
        let mut keep = true;
        for (p, q) in [(-dx, l.a.x - lo), (dx, hi - l.a.x)] {
            if p.abs() < 1e-12 {
                keep &= q >= -1e-9;
            } else {
                let r = q / p;
                if p < 0.0 {
                    t0 = t0.max(r);
                } else {
                    t1 = t1.min(r);
                }
            }
        }
        if !keep || t1 - t0 <= 1e-9 {
            continue;
        }
        lines.push(Line2 {
            a: Point::lerp(l.a, l.b, t0),
            b: Point::lerp(l.a, l.b, t1),
            ..*l
        });
    }
    drawing.lines = lines;
    let regions: Vec<Region> = drawing
        .regions
        .drain(..)
        .filter_map(|r| {
            let polygon = clip_ring(&r.polygon, lo, hi);
            (polygon.len() >= 3).then_some(Region { polygon, ..r })
        })
        .collect();
    drawing.regions = regions;
    drawing.texts.retain(|(p, _)| p.x >= lo && p.x <= hi);
    drawing.update_bounds();
}

/// How much finer the depth buffer gets when the view window is much narrower
/// than the scene it is cut from.
const MAX_RASTER_BOOST: f64 = 2.0;

/// The free-angle pipeline: turn the scene into the view frame, optionally cut
/// at the camera line (everything in front of it removed, `opts.section_depth`
/// limiting how far behind it is drawn), draw with hidden lines removed and
/// cut the result off at the line's ends.
pub fn render_free(
    scene: &Scene,
    view: &FreeView,
    cut: bool,
    opts: &Options,
    weights: Option<&ObjectWeights>,
) -> Drawing {
    let rotated = view_scene(scene, view);
    let Some(bounds) = rotated.bounds() else {
        return Drawing::default();
    };
    let proj = Projection::for_view(ViewDir::Front, bounds);
    let mut opts = *opts;
    if let Some(hw) = view.half_width.filter(|hw| *hw > 1e-6) {
        let (lo, hi) = proj.extent();
        let boost = ((hi.x - lo.x) / (2.0 * hw)).clamp(1.0, MAX_RASTER_BOOST);
        opts.raster_px = (opts.raster_px as f64 * boost).round() as usize;
    }
    let cut_depth = cut.then(|| proj.depth_of_offset(0.0));
    let mut drawing = hlr::render(&rotated, &proj, cut_depth, &opts, weights);
    if let Some(hw) = view.half_width {
        clip_x(&mut drawing, -hw, hw);
    }
    drawing
}

/// Cross section along `view`'s camera line: what lies ahead of the line is
/// drawn, cut faces as poche (see [`crate::section`]).
pub fn section_free(scene: &Scene, view: &FreeView, opts: &Options) -> Drawing {
    render_free(scene, view, true, opts, None)
}

/// The whole scene seen along `view`'s direction, nothing cut away.
pub fn elevation_free(scene: &Scene, view: &FreeView, opts: &Options) -> Drawing {
    render_free(scene, view, false, opts, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_views_are_exact() {
        for (deg, axis) in [
            (90.0, ViewDir::Front),
            (270.0, ViewDir::Back),
            (0.0, ViewDir::Left),
            (180.0, ViewDir::Right),
            (-90.0, ViewDir::Back),
        ] {
            assert_eq!(FreeView::new(Point::ZERO, deg).axis(), Some(axis), "{deg}");
        }
        assert_eq!(FreeView::new(Point::ZERO, 30.0).axis(), None);
    }

    #[test]
    fn the_view_frame_has_x_along_the_line_and_y_ahead() {
        let v = FreeView::new(Point::new(10.0, 20.0), 90.0);
        let p = v.to_view(Point::new(15.0, 50.0));
        assert!((p.x - 5.0).abs() < 1e-9 && (p.y - 30.0).abs() < 1e-9);
        // Looking along +X the right-hand side is -Y.
        let v = FreeView::new(Point::ZERO, 0.0);
        let p = v.to_view(Point::new(4.0, -6.0));
        assert!((p.x - 6.0).abs() < 1e-9 && (p.y - 4.0).abs() < 1e-9);
    }

    #[test]
    fn contains_applies_width_and_depth() {
        let v = FreeView::new(Point::ZERO, 90.0).with_half_width(50.0);
        assert!(v.contains(Point::new(40.0, 10.0), Some(60.0)));
        assert!(!v.contains(Point::new(60.0, 10.0), None));
        assert!(!v.contains(Point::new(0.0, -5.0), None));
        assert!(!v.contains(Point::new(0.0, 70.0), Some(60.0)));
    }

    #[test]
    fn clip_x_trims_lines_and_regions() {
        use crate::drawing::{EdgeKind, LineWeight, RegionKind};
        let mut d = Drawing::new(vec![Line2 {
            a: Point::new(-20.0, 0.0),
            b: Point::new(20.0, 0.0),
            weight: LineWeight::Heavy,
            kind: EdgeKind::Silhouette,
        }]);
        d.regions.push(Region {
            polygon: vec![
                Point::new(-20.0, 0.0),
                Point::new(20.0, 0.0),
                Point::new(20.0, 10.0),
                Point::new(-20.0, 10.0),
            ],
            material: plan_3d::Material::Siding,
            object_id: None,
            kind: RegionKind::Face,
        });
        clip_x(&mut d, -5.0, 8.0);
        assert!((d.lines[0].a.x + 5.0).abs() < 1e-9 && (d.lines[0].b.x - 8.0).abs() < 1e-9);
        assert!((d.regions[0].area() - 130.0).abs() < 1e-9);
        clip_x(&mut d, 30.0, 40.0);
        assert!(d.lines.is_empty() && d.regions.is_empty());
    }
}
