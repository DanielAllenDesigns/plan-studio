//! plan-elevation: Chief-style 2D vector drawings from a 3D [`Scene`].
//!
//! Exterior elevations, cross sections and the plan "overhead" view are
//! produced as weighted line segments with hidden lines removed, ready for
//! DXF, PDF or layout output (see [`Drawing::to_cad`]).
//!
//! * [`elevation`] / [`elevation_from_project`]: Front, Back, Left, Right and Top views.
//! * [`section`]: cut the scene with a plane and draw what lies beyond it.
//! * [`plan_overhead`]: the Top view, where ceilings and roofs occlude.
//! * [`elevation_with_labels`] / [`annotate`]: title, grade line, level
//!   callouts and roof pitch symbols.
//! * [`section_free`] / [`elevation_free`]: the same for a camera line at any
//!   angle ([`FreeView`]), with [`annotate_view`] adding automatic elevation
//!   dimensions ([`ElevDim`]) and material labels; [`ObjectWeights`] restyles
//!   lines by the pen weight of each object's layer; [`Drawing::to_dxf`]
//!   writes the lines on layers by weight.
//!
//! Besides lines a [`Drawing`] carries filled [`Region`]s (visible faces per
//! material, section poche, cast shadows) and, with [`Options::hatch`], the
//! material hatch strokes ([`EdgeKind::Hatch`]).
//!
//! Drawing space is inches of the building, X right and Y up, unshifted from
//! the view's scene coordinates (see [`Projection`]). The algorithm is a
//! dependency-free software depth-buffer test, described in [`Options`] and
//! the crate README.

mod belowgrade;
mod cutlines;
mod dims;
mod drawing;
mod dxf;
mod hatch;
mod hlr;
mod labels;
mod mlabels;
mod projection;
mod regions;
mod shadow;
mod styles;
mod view;

pub use belowgrade::{override_below, weight_class, BelowGradeStyle};
pub use cutlines::{cross_section_lines, find_cut, CutLine};
pub use dims::{DimKind, DimOptions, ElevDim};
pub use drawing::{Drawing, EdgeKind, Line2, LineWeight, Region, RegionKind, StyledLine};
pub use hatch::{poche_hatch_lines, without_poche, DEFAULT_HATCH_SCALE, MAX_POCHE_LINES};
pub use labels::{annotate, annotate_view, annotate_with, AnnotateOptions};
pub use mlabels::{interior_point, material_label};
pub use projection::{Projection, ViewDir};
pub use shadow::SunDir;
pub use styles::{pen_class, ObjectWeights, HEAVY_PEN, MEDIUM_PEN};
pub use view::{clip_x, clip_y, elevation_free, render_free, section_free, view_scene, FreeView};

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
    /// Fill [`Drawing::regions`] with the visible faces (and section poche),
    /// extracted from the id buffer (default true). Implied by `hatch` and `shadows`.
    pub regions: bool,
    /// Add material hatches to the face regions as Light [`EdgeKind::Hatch`]
    /// lines (default false: it costs time and adds many segments).
    pub hatch: bool,
    /// The drawing scale the hatches are made for, paper inches per foot of
    /// the building (default 0.25: 1/4" = 1'-0"). Patterns that would be
    /// denser than 1/32" on paper at that scale are coarsened, so a hatch
    /// drawn for a 1/8" sheet is not the one drawn for a 1/2" detail.
    pub hatch_scale: f64,
    /// Sections only: draw geometry at most this far beyond the cut plane,
    /// inches (Chief's back-clipped cross section). `None` draws everything.
    pub section_depth: Option<f64>,
    /// Cast shadows from this sun as [`RegionKind::Shadow`] regions (Technical
    /// Illustration look). Implemented as a shadow map at the raster size.
    pub shadows: Option<SunDir>,
    /// Approximation of Chief's "line weight by distance": lines more than 12"
    /// behind the nearest drawn line step down one weight class (default false).
    pub depth_weights: bool,
    /// Depth Cue (C-140): `(start, end, opacity)`, distances behind the
    /// nearest drawn line in inches and the fog opacity 0..1 reached at
    /// `end`; lines fade to lighter weights through the ramp. `None` is off.
    pub depth_cue: Option<(f64, f64, f64)>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            crease_angle_deg: 30.0,
            raster_px: 2048,
            include_hidden_dashed: false,
            cull_backfaces: true,
            regions: true,
            hatch: false,
            hatch_scale: DEFAULT_HATCH_SCALE,
            section_depth: None,
            shadows: None,
            depth_weights: false,
            depth_cue: None,
        }
    }
}

/// Hidden-line elevation of `scene` seen from `dir` (`Top` gives the plan view).
pub fn elevation(scene: &Scene, dir: ViewDir, opts: &Options) -> Drawing {
    match scene.bounds() {
        Some(bounds) => hlr::render(scene, &Projection::for_view(dir, bounds), None, opts, None),
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
    hlr::render(scene, &proj, Some(depth), opts, None)
}

/// The plan "overhead" line drawing: the Top view, with roofs and ceilings occluding.
pub fn plan_overhead(scene: &Scene, opts: &Options) -> Drawing {
    elevation(scene, ViewDir::Top, opts)
}

/// Build the scene for `project` with plan-3d, then draw its elevation.
pub fn elevation_from_project(project: &Project, dir: ViewDir, opts: &Options) -> Drawing {
    elevation(&build_scene(project), dir, opts)
}

/// [`elevation_from_project`] plus [`annotate`]: title, grade line, level
/// callouts and roof pitch symbols as [`Drawing::texts`] and annotation lines.
pub fn elevation_with_labels(project: &Project, dir: ViewDir, opts: &Options) -> Drawing {
    let scene = build_scene(project);
    let mut drawing = elevation(&scene, dir, opts);
    annotate(&mut drawing, &scene, project, dir);
    drawing
}
