//! Orthographic view directions and the scene-to-drawing mapping.

use plan_3d::Bounds;
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// Where the camera stands. Scene space is X right, Y up, Z toward the viewer
/// of the plan (Z = -plan y).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViewDir {
    /// Camera on +Z looking toward -Z: the elevation seen from the -plan-y side.
    Front,
    /// Camera on -Z looking toward +Z.
    Back,
    /// Camera on -X looking toward +X (sees the left end of the plan).
    Left,
    /// Camera on +X looking toward -X (sees the right end of the plan).
    Right,
    /// Camera above looking down: the plan frame (drawing x = plan x, y = plan y).
    Top,
}

/// One drawing axis: which scene axis feeds it and with which sign.
#[derive(Debug, Clone, Copy)]
struct AxisMap {
    axis: usize,
    sign: f64,
}

const fn ax(axis: usize, sign: f64) -> AxisMap {
    AxisMap { axis, sign }
}

/// Orthographic mapping from scene space to drawing space.
///
/// Drawing coordinates are inches of the building (not scaled, not shifted),
/// X right and Y up. The third output, *depth*, grows toward the viewer, so a
/// larger depth means nearer to the camera. Every view is a proper rotation of
/// the scene, so triangle winding is preserved.
#[derive(Debug, Clone, Copy)]
pub struct Projection {
    dir: ViewDir,
    u: AxisMap,
    v: AxisMap,
    d: AxisMap,
    min: Point,
    max: Point,
    depth: (f64, f64),
}

impl Projection {
    /// Build the mapping for `dir`, remembering the 2D extent of `bounds`.
    pub fn for_view(dir: ViewDir, bounds: Bounds) -> Projection {
        let (u, v, d) = match dir {
            ViewDir::Front => (ax(0, 1.0), ax(1, 1.0), ax(2, 1.0)),
            ViewDir::Back => (ax(0, -1.0), ax(1, 1.0), ax(2, -1.0)),
            ViewDir::Left => (ax(2, 1.0), ax(1, 1.0), ax(0, -1.0)),
            ViewDir::Right => (ax(2, -1.0), ax(1, 1.0), ax(0, 1.0)),
            ViewDir::Top => (ax(0, 1.0), ax(2, -1.0), ax(1, 1.0)),
        };
        let mut p = Projection {
            dir,
            u,
            v,
            d,
            min: Point::ZERO,
            max: Point::ZERO,
            depth: (0.0, 0.0),
        };
        let (lo, hi) = (bounds.0.map(f64::from), bounds.1.map(f64::from));
        let (a, da) = p.project(lo);
        let (b, db) = p.project(hi);
        p.min = Point::new(a.x.min(b.x), a.y.min(b.y));
        p.max = Point::new(a.x.max(b.x), a.y.max(b.y));
        p.depth = (da.min(db), da.max(db));
        p
    }

    /// The view direction this projection was built for.
    pub fn dir(&self) -> ViewDir {
        self.dir
    }

    /// Map a scene point to `(drawing point, depth toward the viewer)`.
    pub fn project(&self, p: [f64; 3]) -> (Point, f64) {
        let pick = |m: AxisMap| m.sign * p[m.axis];
        (Point::new(pick(self.u), pick(self.v)), pick(self.d))
    }

    /// Depth of the plane whose scene coordinate along the view axis is `offset`.
    pub fn depth_of_offset(&self, offset: f64) -> f64 {
        self.d.sign * offset
    }

    /// Drawing-space extent `(min, max)` of the bounds this projection was built from.
    pub fn extent(&self) -> (Point, Point) {
        (self.min, self.max)
    }

    /// Depth range `(far, near)` of the bounds this projection was built from.
    pub fn depth_range(&self) -> (f64, f64) {
        self.depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Bounds = ([0.0, 0.0, -50.0], [100.0, 90.0, 10.0]);

    #[test]
    fn front_keeps_x_and_y_and_depth_is_z() {
        let p = Projection::for_view(ViewDir::Front, BOUNDS);
        let (pt, d) = p.project([5.0, 7.0, 3.0]);
        assert_eq!((pt, d), (Point::new(5.0, 7.0), 3.0));
        let (lo, hi) = p.extent();
        assert_eq!((lo, hi), (Point::new(0.0, 0.0), Point::new(100.0, 90.0)));
    }

    #[test]
    fn top_uses_plan_frame() {
        let p = Projection::for_view(ViewDir::Top, BOUNDS);
        let (pt, d) = p.project([5.0, 7.0, -3.0]);
        assert_eq!((pt, d), (Point::new(5.0, 3.0), 7.0));
    }

    #[test]
    fn opposite_views_mirror_each_other() {
        let f = Projection::for_view(ViewDir::Front, BOUNDS);
        let b = Projection::for_view(ViewDir::Back, BOUNDS);
        let q = [12.0, 4.0, -8.0];
        assert_eq!(f.project(q).0.x, -b.project(q).0.x);
        assert_eq!(f.project(q).1, -b.project(q).1);
    }
}
