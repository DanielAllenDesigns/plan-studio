//! plan-elevation: Chief-style 2D vector drawings from a 3D [`Scene`].
//!
//! Exterior elevations, cross sections and the plan "overhead" view are
//! produced as weighted line segments with hidden lines removed, ready for
//! DXF, PDF or layout output (see [`Drawing::to_cad`]).
//!
//! * [`elevation`] / [`elevation_from_project`]: Front, Back, Left, Right and Top views.
//! * [`section`]: cut the scene with a plane and draw what lies beyond it.
//! * [`plan_overhead`]: the Top view, where ceilings and roofs occlude.
//!
//! Drawing space is inches of the building, X right and Y up, unshifted from
//! the view's scene coordinates (see [`Projection`]). The algorithm is a
//! dependency-free software depth-buffer test, described in [`Options`] and
//! the crate README.

mod drawing;
mod hlr;
mod projection;

pub use drawing::{Drawing, EdgeKind, Line2, LineWeight};
pub use projection::{Projection, ViewDir};

use plan_3d::{build_scene, Scene};
use plan_core::Project;

/// A cutting plane for [`section`].
///
/// The camera stands on the `plane_normal` side looking at the model; the
/// plane sits at scene coordinate `offset` along the view axis (Z for
/// Front/Back, X for Left/Right, Y for Top) and everything between the camera
/// and the plane is removed. Remember scene Z is `-plan y`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionCut {
    pub plane_normal: ViewDir,
    /// Position of the cutting plane along that axis, scene units (inches).
    pub offset: f64,
}

/// Tuning knobs for the hidden-line pipeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Faces folding by more than this many degrees get a Medium crease line (default 30).
    pub crease_angle_deg: f64,
    /// Depth-buffer resolution along the longer drawing axis (default 2048).
    /// Visible runs shorter than 2 px are dropped, so detail finer than
    /// `extent / raster_px` is lost.
    pub raster_px: usize,
    /// Also emit hidden edges as [`EdgeKind::Hidden`] lines for dashed output (default false).
    pub include_hidden_dashed: bool,
    /// Back-facing triangles do not occlude (default true; fine for closed solids).
    pub cull_backfaces: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            crease_angle_deg: 30.0,
            raster_px: 2048,
            include_hidden_dashed: false,
            cull_backfaces: true,
        }
    }
}

/// Hidden-line elevation of `scene` seen from `dir` (`Top` gives the plan view).
pub fn elevation(scene: &Scene, dir: ViewDir, opts: &Options) -> Drawing {
    match scene.bounds() {
        Some(bounds) => hlr::render(scene, &Projection::for_view(dir, bounds), None, opts),
        None => Drawing::default(),
    }
}

/// Cross section: clip `scene` at `cut`, draw the cut as Heavy [`EdgeKind::Cut`]
/// lines and the remaining model beyond it with hidden lines removed.
///
/// Cut faces are treated as solid and hide what is behind them. This assumes
/// closed meshes, which is what plan-3d produces.
pub fn section(scene: &Scene, cut: SectionCut, opts: &Options) -> Drawing {
    let Some(bounds) = scene.bounds() else {
        return Drawing::default();
    };
    let proj = Projection::for_view(cut.plane_normal, bounds);
    let depth = proj.depth_of_offset(cut.offset);
    hlr::render(scene, &proj, Some(depth), opts)
}

/// The plan "overhead" line drawing: the Top view, with roofs and ceilings occluding.
pub fn plan_overhead(scene: &Scene, opts: &Options) -> Drawing {
    elevation(scene, ViewDir::Top, opts)
}

/// Build the scene for `project` with plan-3d, then draw its elevation.
pub fn elevation_from_project(project: &Project, dir: ViewDir, opts: &Options) -> Drawing {
    elevation(&build_scene(project), dir, opts)
}
