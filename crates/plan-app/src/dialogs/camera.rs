//! Camera dialogs (`docs/parity/3d-views-cameras.md`): the Camera
//! Specification (C-30) and the Ray Trace dialog (C-51, C-52, C-63).
//!
//! The Camera Specification edits a cloned [`CameraObject`] on the shared
//! dialog frame, with Chief's tabs: Camera (position, direction, angle of
//! view, height, tilt, clipping, floors displayed, lock), Backdrop (default
//! sky, a sky colour or a picture from Chief's Backdrops folder), Rendering
//! (technique, Preview or Final View, shadows, ambient and sun overrides)
//! and Label. The tabs write [`CameraObject::view`], which is saved with the
//! plan; [`CameraExtras`] only carries the technique of a camera that has
//! not chosen one yet.
//!
//! Elevation and cross-section cameras (and wall elevations) also get the
//! "Elevation rendering" section on the Rendering tab: hatch materials,
//! shadows (sun azimuth and altitude, optionally from a date, time and
//! latitude through `plan_materials::SunSettings`), the section back-clip
//! depth, line weight by distance and labels. They are stored on the camera
//! (`CameraObject.render`, the back clip in the section) and turned into
//! `plan_elevation::Options` by [`elevation_options`]; [`render_elevation`]
//! draws the camera's 2D drawing with them.
//!
//! Lights live here too: the light list handed to the ray tracer
//! ([`render_lights`], plan lights plus electrical fixtures), the Adjust
//! Lights dialog ([`AdjustLightsDialog`]) and the Add Lights defaults. The
//! camera-backed layout box hook is [`layout_context`] /
//! [`send_camera_to_layout`].
//!
//! [`RayTraceDialog`] is a small state machine around
//! `plan_render::Renderer::render_progressive` running on a background
//! thread: Idle -> Running -> Done / Cancelled / Failed. Everything except the
//! egui drawing is plain Rust and unit tested.

use super::{row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke};
use plan_3d::Scene;
use plan_core::camera::{PlanLight, DEFAULT_CONE_LENGTH};
use plan_core::geometry::Point;
use plan_core::{CameraKind, CameraObject, Id, Project};
use plan_elevation::{
    AnnotateOptions, DimOptions, Drawing, FreeView, ObjectWeights, Options, SunDir,
};
#[cfg(test)]
use plan_elevation::{SectionCut, ViewDir};
use plan_materials::{RenderingTechnique, SunSettings};
use plan_render::{
    AreaLight, Environment, Image, PointLight, RenderSettings, Renderer, SkyModel, Sun, Technique,
};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// Chief's Camera Specification panels (manual pp. 1186 to 1194).
const TABS: &[Tab] = &[
    Tab {
        name: "Camera",
        enabled: true,
    },
    Tab {
        name: "Positioning",
        enabled: true,
    },
    Tab {
        name: "Below Grade",
        enabled: true,
    },
    Tab {
        name: "Selected Defaults",
        enabled: true,
    },
    Tab {
        name: "Plan Display",
        enabled: true,
    },
    Tab {
        name: "Backdrop",
        enabled: true,
    },
    Tab {
        name: "Layer",
        enabled: true,
    },
    Tab {
        name: "Label",
        enabled: true,
    },
];

/// The index of the Selected Defaults tab.
const TAB_SELECTED_DEFAULTS: usize = 3;

mod panels;
pub mod slider;

/// Smallest and largest angle of view Chief accepts (C-7).
pub const MIN_FOV_DEG: f64 = 5.0;
pub const MAX_FOV_DEG: f64 = 170.0;

/// Per-camera settings the model cannot store yet; kept per session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraExtras {
    pub technique: RenderingTechnique,
}

impl Default for CameraExtras {
    fn default() -> Self {
        Self {
            technique: RenderingTechnique::Standard,
        }
    }
}

/// Is this a camera that draws a 2D elevation or cross section?
pub fn is_elevation_camera(c: &CameraObject) -> bool {
    matches!(
        c.kind,
        CameraKind::CrossSection { .. } | CameraKind::WallElevation | CameraKind::Elevation
    )
}

/// Does the camera cut the model at its section line (so a back clip and the
/// section drawing apply)? Exterior elevations (Auto Elevations) do not.
pub fn cuts_model(c: &CameraObject) -> bool {
    match c.kind {
        CameraKind::CrossSection { .. } => true,
        // A wall elevation made by the Wall Elevation tool has its cut line;
        // one without a line draws the whole building from its direction.
        CameraKind::WallElevation => c.section.is_some(),
        _ => false,
    }
}

/// The shadow direction of a sun, or `None` while it is below the horizon.
pub fn sun_dir(s: &SunSettings) -> Option<SunDir> {
    (s.altitude_deg > 0.0).then_some(SunDir {
        azimuth_deg: s.azimuth_deg,
        altitude_deg: s.altitude_deg,
    })
}

/// The orthographic view closest to the camera's viewing direction (the
/// camera stands on the opposite side of the model). The drawing itself now
/// follows the cut line at any angle ([`free_view`]); this is the
/// nearest-axis approximation the tests compare against.
#[cfg(test)]
pub fn elevation_view_dir(c: &CameraObject) -> ViewDir {
    let d = c.direction();
    if d.y.abs() >= d.x.abs() {
        if d.y >= 0.0 {
            ViewDir::Front
        } else {
            ViewDir::Back
        }
    } else if d.x >= 0.0 {
        ViewDir::Left
    } else {
        ViewDir::Right
    }
}

/// The cutting plane of a section camera: through the centre of its cut line,
/// square to the nearest axis (scene Z is -plan y). See [`elevation_view_dir`].
#[cfg(test)]
pub fn section_cut(c: &CameraObject) -> SectionCut {
    let plane_normal = elevation_view_dir(c);
    let offset = match plane_normal {
        ViewDir::Front | ViewDir::Back => -c.position.y,
        _ => c.position.x,
    };
    SectionCut {
        plane_normal,
        offset,
    }
}

/// `plan_elevation::Options` for a camera: hatch, shadows from the stored
/// sun, line weight by distance, and a section's back clip as its depth.
/// (`raster_px` is left at the default; tests lower it.)
pub fn elevation_options(c: &CameraObject) -> Options {
    elevation_options_with_sun(c, None)
}

/// [`elevation_options`] with the shadows taken from `sun` (the plan's Sun
/// Angle) instead of the camera's own sun, when one is given.
pub fn elevation_options_with_sun(c: &CameraObject, sun: Option<SunDir>) -> Options {
    let r = &c.render;
    Options {
        hatch: r.hatch,
        shadows: sun.or(r.shadows.then_some(SunDir {
            azimuth_deg: r.sun_azimuth_deg,
            altitude_deg: r.sun_altitude_deg,
        })),
        depth_weights: r.depth_weights,
        include_hidden_dashed: c.vector.hidden_dashed,
        section_depth: cuts_model(c)
            .then(|| crate::tools::camera::back_clip(c))
            .flatten(),
        ..Options::default()
    }
}

/// The camera line of an elevation or section camera as a free-angle view
/// (C-17, C-20): the centre and direction of its cut line at any angle, and
/// for a camera that cuts the model the line's half length as the width of the
/// drawing. Exterior elevations are not cut off at their ends.
pub fn free_view(c: &CameraObject) -> FreeView {
    let (a, b) = crate::tools::camera::section_line(c);
    let along = b - a;
    // The view looks to the left of A to B (the direction turned 90 degrees
    // clockwise is the line's tangent), so the line fixes the direction.
    let view_deg = if c.section.is_some() && along.length() > 1e-9 {
        along.perp().angle().to_degrees()
    } else {
        c.direction_deg
    };
    let origin = if c.section.is_some() || cuts_model(c) {
        Point::lerp(a, b, 0.5)
    } else {
        c.position
    };
    let view = FreeView::new(origin, view_deg);
    if cuts_model(c) {
        view.with_half_width(crate::tools::camera::section_width(c) * 0.5)
    } else {
        view
    }
}

/// Pen weight classes of the walls and openings of `project`, from the line
/// weights of their layers (walls on their own layer, doors on "Doors", windows
/// on "Windows"): the Vector View line styles per layer.
pub fn layer_weights(project: &Project) -> ObjectWeights {
    let mut w = ObjectWeights::new();
    let pen = |name: &str| project.layers.get(name).map(|l| l.line_weight);
    for f in &project.floors {
        for wall in &f.walls {
            if let Some(p) = pen(&wall.layer) {
                w.set_pen(wall.id, p);
            }
        }
        for o in &f.openings {
            let layer = match o.kind {
                plan_core::OpeningKind::Door => "Doors",
                plan_core::OpeningKind::Window => "Windows",
            };
            if let Some(p) = pen(layer) {
                w.set_pen(o.id, p);
            }
        }
    }
    w
}

/// Which annotations a camera's drawing gets: its labels option gates the
/// title, grade line and roof pitch symbols (and the level callouts when
/// they are on too); the Vector View options add dimension strings and
/// material labels.
pub fn annotate_options(c: &CameraObject) -> AnnotateOptions {
    AnnotateOptions {
        title: c.render.labels,
        grade: c.render.labels,
        levels: c.render.labels && c.vector.level_labels,
        pitch: c.render.labels,
        dimensions: c.vector.dimensions.then(DimOptions::default),
        materials: c.vector.material_labels,
    }
}

/// The camera's 2D drawing with `opts`: a section or wall elevation is cut at
/// its line (at any angle, see [`free_view`]), other elevation cameras draw the
/// whole building from their direction. With the camera's labels option the
/// drawing gets the title (the camera's name), level callouts, grade line and
/// roof pitch symbols; its Vector View options add dimensions, material labels
/// and per-layer line weights.
pub fn render_elevation_with(project: &Project, c: &CameraObject, opts: &Options) -> Drawing {
    render_elevation_parts(project, c, opts).0
}

/// [`render_elevation_with`] and the Cross Section Lines found in the cut
/// (before the poché is dropped, so they exist with Poche off too).
pub fn render_elevation_parts(
    project: &Project,
    c: &CameraObject,
    opts: &Options,
) -> (Drawing, Vec<plan_elevation::CutLine>) {
    let scene = crate::editor::framing_view::elevation_scene(project);
    let mut view = free_view(c);
    let cut = cuts_model(c);
    let weights = c.vector.layer_weights.then(|| layer_weights(project));
    let vol = project.clip_volume(c);
    let mut opts = *opts;
    if opts.depth_cue.is_none() {
        // Depth Cue fogs the lines of a section or elevation (C-140).
        opts.depth_cue = c.view.depth_cue.ramp();
    }
    if cut && c.view.clip.clip_to_room {
        opts.section_depth = vol.back;
    }
    // The Clip Sides switch: off draws the whole width of the model.
    if cut && !c.view.clip.clip_sides && !c.view.clip.clip_to_room {
        view.half_width = None;
    }
    let mut drawing = if cut && vol.plane.is_stepped() {
        render_stepped(&scene, &view, &vol, &opts, weights.as_ref())
    } else {
        plan_elevation::render_free(&scene, &view, cut, &opts, weights.as_ref())
    };
    let mut cut_lines = Vec::new();
    if cut {
        apply_clip(&mut drawing, &vol);
        cut_lines = plan_elevation::cross_section_lines(&drawing, &vol.plane.breaks);
        if !c.view.clip.poche {
            drawing
                .regions
                .retain(|r| r.kind != plan_elevation::RegionKind::Cut);
        }
    }
    if c.view.below_grade.is_active() {
        let bg = &c.view.below_grade;
        plan_elevation::override_below(
            &mut drawing,
            bg.height(grade_height(project, c)),
            &plan_elevation::BelowGradeStyle {
                color: bg.override_color.then_some(bg.color),
                dashed: bg.dashed(),
                weight: bg.override_weight.then_some(bg.weight),
            },
        );
    }
    let notes = annotate_options(c);
    if notes.title || notes.dimensions.is_some() || notes.materials {
        let title = (!c.name.trim().is_empty() && (cut || c.kind == CameraKind::Elevation))
            .then(|| c.name.to_uppercase());
        plan_elevation::annotate_view(
            &mut drawing,
            &scene,
            project,
            &view,
            title.as_deref(),
            &notes,
        );
    }
    drawing.append(annotation_layer_at(c, &cut_lines));
    if cut {
        view_layers(&mut drawing, project, &vol, &cut_lines);
    }
    (drawing, cut_lines)
}

/// The height of the terrain's top surface at the camera, inches: what Below
/// Grade's Terrain Perimeter limit means (the terrain's own elevation at the
/// camera when there is one, else the first floor's level).
pub fn grade_height(project: &Project, c: &CameraObject) -> f64 {
    crate::editor::site_view::terrain_elevation_at(project, c.position)
        .or_else(|| project.floors.first().map(|f| f.elevation))
        .unwrap_or(0.0)
}

/// The Cross Section Lines and Clip Lines of a view as coloured lines, when
/// their layers are on (manual pp. 1167, 1171). They belong to the 3D view
/// only: the Cross Section Lines do not go to layout and neither do the Clip
/// Lines, which are an editing aid.
fn view_layers(
    drawing: &mut Drawing,
    project: &Project,
    vol: &plan_core::camera_view::ClipVolume,
    cut_lines: &[plan_elevation::CutLine],
) {
    use plan_core::camera_view::clip::{CLIP_LINES_LAYER, CROSS_SECTION_LAYER};
    if project.layers.is_visible(CROSS_SECTION_LAYER) {
        let color = project
            .layers
            .get(CROSS_SECTION_LAYER)
            .map_or([0x7A, 0x2E, 0x2E], |l| l.color);
        for l in cut_lines {
            drawing.styled.push(plan_elevation::StyledLine {
                a: l.a,
                b: l.b,
                width: 0.5,
                color,
                dashed: false,
            });
        }
    }
    if let Some(layer) = project.layers.get(CLIP_LINES_LAYER).filter(|l| l.display) {
        let (lo, hi) = drawing.bounds;
        let (x0, x1) = vol.x.unwrap_or((lo.x, hi.x));
        let (y0, y1) = vol.y.unwrap_or((lo.y, hi.y));
        for (a, b) in clip_line_segments(vol, (x0, x1), (y0, y1)) {
            drawing.styled.push(plan_elevation::StyledLine {
                a,
                b,
                width: 0.8,
                color: layer.color,
                dashed: true,
            });
        }
    }
}

/// The Clip Lines of a view: a vertical line at each side clip and a level
/// line at the bottom and top clip elevations (C-136).
pub fn clip_line_segments(
    vol: &plan_core::camera_view::ClipVolume,
    xs: (f64, f64),
    ys: (f64, f64),
) -> Vec<(Point, Point)> {
    let mut v = Vec::new();
    if vol.x.is_some() {
        for x in [xs.0, xs.1] {
            v.push((Point::new(x, ys.0), Point::new(x, ys.1)));
        }
    }
    if vol.y.is_some() {
        for y in [ys.0, ys.1] {
            v.push((Point::new(xs.0, y), Point::new(xs.1, y)));
        }
    }
    v
}

/// Cut a section drawing off at its clip volume: the sides of Clip Sides or
/// the room, and Clip Elevation (C-136, C-138).
pub fn apply_clip(drawing: &mut Drawing, vol: &plan_core::camera_view::ClipVolume) {
    if let Some((lo, hi)) = vol.x {
        plan_elevation::clip_x(drawing, lo, hi);
    }
    if let Some((lo, hi)) = vol.y {
        plan_elevation::clip_y(drawing, lo, hi);
    }
}

/// A stepped cutting plane (C-137): each piece of the plane is its own cut,
/// moved ahead by its offset with its back clip shortened to match, kept to
/// the piece's span of the line, and the pieces share one drawing.
fn render_stepped(
    scene: &plan_3d::Scene,
    view: &FreeView,
    vol: &plan_core::camera_view::ClipVolume,
    opts: &Options,
    weights: Option<&ObjectWeights>,
) -> Drawing {
    let reach = view.half_width.unwrap_or(1.0e5);
    let (lo, hi) = vol.x.unwrap_or((-reach, reach));
    let mut out = Drawing::default();
    for (x0, x1, offset) in vol.spans(lo, hi) {
        let mut piece = *view;
        piece.origin = piece.origin + piece.dir() * offset;
        piece.half_width = None;
        let mut o = *opts;
        o.section_depth = vol.back.map(|b| (b - offset).max(1.0));
        let mut d = plan_elevation::render_free(scene, &piece, true, &o, weights);
        plan_elevation::clip_x(&mut d, x0, x1);
        out.append(d);
    }
    out
}

/// The annotations saved with the view that sit on the drawing itself,
/// as lines and texts of the drawing's own frame (C-129).
#[cfg_attr(not(test), allow(dead_code))]
pub fn annotation_layer(c: &CameraObject) -> Drawing {
    annotation_layer_at(c, &[])
}

/// [`annotation_layer`] with the dimensions that locate a Cross Section Line
/// (and the Point Markers they left) moved to where `cut_lines` stand now.
pub fn annotation_layer_at(c: &CameraObject, cut_lines: &[plan_elevation::CutLine]) -> Drawing {
    let mut d = Drawing::default();
    let drawing_surface = plan_core::camera_view::DrawSurface::drawing();
    for a in c
        .view
        .annotations
        .iter()
        .filter(|a| a.surface == drawing_surface)
    {
        let mut kind = a.kind.clone();
        relocate_on_cut_lines(&mut kind, cut_lines);
        let marks = kind.marks();
        for (p, q) in marks.lines {
            d.lines.push(plan_elevation::Line2 {
                a: Point::new(p[0], p[1]),
                b: Point::new(q[0], q[1]),
                weight: plan_elevation::LineWeight::Light,
                kind: plan_elevation::EdgeKind::Annotation,
            });
        }
        for (at, text, _size) in marks.texts {
            d.texts.push((Point::new(at[0], at[1]), text));
        }
    }
    d.update_bounds();
    d
}

/// Moves the points of `kind` that locate a Cross Section Line to where the
/// line stands now.
fn relocate_on_cut_lines(
    kind: &mut plan_core::camera_view::AnnotKind,
    cut_lines: &[plan_elevation::CutLine],
) {
    if cut_lines.is_empty() {
        return;
    }
    kind.relocate(&|c| plan_elevation::find_cut(cut_lines, c.object, c.edge).map(|l| l.position()));
}

/// How close a dimension end must be to a Cross Section Line to locate it,
/// drawing inches.
pub const CUT_SNAP: f64 = 1.5;

/// A dimension that meets a Cross Section Line locates the line through a
/// Point Marker, so it stays attached when the lines are made again (manual
/// pp. 1167, 1173; C-131). Looks at every dimension of the camera's drawing
/// that is not yet attached, snaps an end that lies within [`CUT_SNAP`] of a
/// line onto it and adds the marker. Returns how many ends were attached.
pub fn attach_cut_markers(c: &mut CameraObject, cut_lines: &[plan_elevation::CutLine]) -> usize {
    use plan_core::camera_view::annot::CutRef;
    use plan_core::camera_view::AnnotKind;
    let surface = plan_core::camera_view::DrawSurface::drawing();
    let nearest = |p: [f64; 2]| -> Option<(CutRef, [f64; 2])> {
        cut_lines
            .iter()
            .filter_map(|l| {
                let (lo, hi) = (l.a.x.min(l.b.x), l.a.x.max(l.b.x));
                let (ylo, yhi) = (l.a.y.min(l.b.y), l.a.y.max(l.b.y));
                let (off, along_ok, snapped) = if l.is_vertical() {
                    (
                        (p[0] - l.a.x).abs(),
                        p[1] >= ylo - CUT_SNAP && p[1] <= yhi + CUT_SNAP,
                        [l.a.x, p[1]],
                    )
                } else {
                    (
                        (p[1] - l.a.y).abs(),
                        p[0] >= lo - CUT_SNAP && p[0] <= hi + CUT_SNAP,
                        [p[0], l.a.y],
                    )
                };
                (off <= CUT_SNAP && along_ok).then_some((
                    off,
                    CutRef {
                        object: l.object,
                        edge: l.edge,
                    },
                    snapped,
                ))
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .map(|(_, cut, at)| (cut, at))
    };
    let mut attached = Vec::new();
    for a in c
        .view
        .annotations
        .iter_mut()
        .filter(|a| a.surface == surface)
    {
        if let AnnotKind::Dimension {
            a: pa,
            b: pb,
            a_cut,
            b_cut,
            ..
        } = &mut a.kind
        {
            for (p, slot) in [(pa, a_cut), (pb, b_cut)] {
                if slot.is_some() {
                    continue;
                }
                if let Some((cut, at)) = nearest(*p) {
                    *p = at;
                    *slot = Some(cut);
                    attached.push((cut, at));
                }
            }
        }
    }
    let n = attached.len();
    for (cut, at) in attached {
        let has = c
            .view
            .annotations
            .iter()
            .any(|a| matches!(&a.kind, AnnotKind::PointMarker { cut: k, .. } if *k == cut));
        if has {
            continue;
        }
        let id = c.view.annotations.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        let layer = c
            .view
            .annotations
            .iter()
            .find(|a| matches!(a.kind, AnnotKind::Dimension { .. }))
            .map_or_else(String::new, |a| a.layer.clone());
        c.view
            .annotations
            .push(plan_core::camera_view::ViewAnnotation {
                id,
                kind: AnnotKind::PointMarker { at, cut },
                surface,
                layer,
                weight: None,
            });
    }
    n
}

/// The Cross Section Lines of a section camera as the view draws them now.
pub fn cross_section_lines_of(project: &Project, c: &CameraObject) -> Vec<plan_elevation::CutLine> {
    if !cuts_model(c) {
        return Vec::new();
    }
    // The same options the view draws with, so a dimension that locates a
    // line lands exactly where the view puts it.
    render_elevation_parts(project, c, &elevation_options(c)).1
}

/// The camera's drawing as a DXF with its lines on layers by weight (named
/// after the camera), or `None` for an empty view.
#[cfg_attr(not(test), allow(dead_code))]
pub fn camera_dxf(project: &Project, c: &CameraObject) -> Option<String> {
    let prefix = if c.name.trim().is_empty() {
        "Elevation"
    } else {
        c.name.trim()
    };
    render_elevation(project, c).to_dxf(prefix)
}

/// Asks for a file name and writes `drawing` there as a DXF with its lines on
/// layers by weight (named after `name`); returns the status line.
pub fn save_drawing_dxf(drawing: &Drawing, name: &str) -> String {
    let name = if name.trim().is_empty() {
        "Elevation"
    } else {
        name.trim()
    };
    let Some(text) = drawing.to_dxf(name) else {
        return "Nothing to export: the view is empty".into();
    };
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{name}.dxf"))
        .add_filter("dxf", &["dxf"])
        .save_file()
    else {
        return "Export cancelled".into();
    };
    match std::fs::write(&path, text.as_bytes()) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save {}: {e}", path.display()),
    }
}

/// [`save_drawing_dxf`] of the camera's own drawing.
pub fn save_camera_dxf(project: &Project, c: &CameraObject) -> String {
    save_drawing_dxf(&render_elevation(project, c), &c.name)
}

/// [`render_elevation_with`] using [`elevation_options`].
pub fn render_elevation(project: &Project, c: &CameraObject) -> Drawing {
    render_elevation_with(project, c, &elevation_options(c))
}

/// The back clip being edited: the cross section's own, or the one stored on
/// the line of the other section-like cameras.
fn back_clip_of(c: &mut CameraObject) -> Option<&mut Option<f64>> {
    match &mut c.kind {
        CameraKind::CrossSection { back_clip } => Some(back_clip),
        CameraKind::WallElevation | CameraKind::Elevation => {
            c.section.as_mut().map(|s| &mut s.back_clip)
        }
        _ => None,
    }
}

/// Date, time and latitude the "Set sun" button turns into a sun position.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SunInput {
    month: u32,
    day: u32,
    time_hours: f64,
    latitude: f64,
}

impl Default for SunInput {
    fn default() -> Self {
        Self {
            month: 6,
            day: 21,
            time_hours: 15.0,
            latitude: 33.75,
        }
    }
}

/// The Camera Specification dialog (C-30).
pub struct CameraDialog {
    frame: SpecDialog,
    draft: CameraObject,
    extras: CameraExtras,
    floor_name: String,
    fields: Fields,
    /// Length of a section's cut line (the draft's `section` is rebuilt from
    /// the centre, the view direction and this after every edit).
    section_len: f64,
    sun_input: SunInput,
    /// The plan's lighting, the values the overrides start from.
    plan_lighting: plan_core::camera_view::Lighting,
    /// The names of the plan's floors, lowest first (the Floors Displayed
    /// pick).
    floor_names: Vec<String>,
    /// "Export DXF" was clicked; the host takes it with
    /// [`CameraDialog::take_export_request`] (the dialog has no project).
    export_requested: bool,
    /// The lists of the Selected Defaults panel, filled by the host.
    defaults_env: Option<super::default_sets::Env>,
    /// The plan's own active defaults (what a view that chose none uses).
    plan_selected: super::default_sets::Selected,
    /// What the Selected Defaults panel asked the host to do.
    defaults_events: Vec<super::default_sets::Ev>,
    /// The plan's layer names, for the Layer panel.
    layer_names: Vec<String>,
    /// The tab drawn last.
    last_tab: usize,
}

impl CameraDialog {
    pub fn new(camera: &CameraObject, floor_name: &str, extras: CameraExtras) -> Self {
        let mut draft = camera.clone();
        crate::tools::camera::upgrade_section(&mut draft);
        let section_len = crate::tools::camera::section_width(&draft);
        // The camera's own technique (saved with the plan) wins over the one
        // the 3D view happens to be showing.
        let mut extras = extras;
        if let Some(t) = crate::shell::view3d_panel::view_settings::technique_of(&draft) {
            extras.technique = t;
        }
        Self {
            frame: SpecDialog::new("Camera Specification", "camera"),
            draft,
            section_len,
            sun_input: SunInput::default(),
            plan_lighting: plan_core::camera_view::Lighting::default(),
            floor_names: Vec::new(),
            export_requested: false,
            defaults_env: None,
            plan_selected: super::default_sets::Selected::default(),
            defaults_events: Vec::new(),
            layer_names: Vec::new(),
            last_tab: 0,
            extras,
            floor_name: floor_name.to_string(),
            fields: Fields::default(),
        }
    }

    /// The draft the dialog edits, for tests that stand in for the fields.
    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut CameraObject {
        &mut self.draft
    }

    /// The plan's lighting (3D > Lighting), which the Rendering tab's
    /// overrides start from.
    pub fn with_plan_lighting(mut self, l: plan_core::camera_view::Lighting) -> Self {
        self.plan_lighting = l;
        self
    }

    /// The names of the plan's floors, so Floors Displayed can pick some.
    pub fn with_floor_names(mut self, names: Vec<String>) -> Self {
        self.floor_names = names;
        self
    }

    pub fn id(&self) -> Id {
        self.draft.id
    }

    pub fn draft(&self) -> &CameraObject {
        &self.draft
    }

    pub fn extras(&self) -> CameraExtras {
        self.extras
    }

    /// Did the user click "Export drawing as DXF" since the last call? The
    /// host then writes [`camera_dxf`] of [`CameraDialog::draft`].
    pub fn take_export_request(&mut self) -> bool {
        std::mem::take(&mut self.export_requested)
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        // The frame borrows `self` as the page provider, so lift it out.
        let mut frame = std::mem::replace(&mut self.frame, SpecDialog::new("", "camera_tmp"));
        let outcome = frame.show(ctx, self);
        self.frame = frame;
        outcome
    }

    /// Rebuilds a section's cut line from the edited centre, direction,
    /// length and back clip.
    fn sync_section(&mut self) {
        if !self.is_section() {
            return;
        }
        let back = match self.draft.kind {
            CameraKind::CrossSection { back_clip } => back_clip,
            _ => self.draft.section.and_then(|s| s.back_clip),
        };
        let (centre, dir) = (self.draft.position, self.draft.direction_deg);
        crate::tools::camera::set_section_geometry(
            &mut self.draft,
            centre,
            dir,
            self.section_len,
            back,
        );
    }

    /// Cross sections, wall elevations and exterior elevations are placed by
    /// a cut line.
    fn is_section(&self) -> bool {
        crate::tools::camera::is_section(&self.draft)
    }

    /// Keeps a walkthrough's eye position and direction on its first node
    /// and segment after the path was edited.
    fn sync_walkthrough(&mut self) {
        if self.draft.kind != CameraKind::Walkthrough {
            return;
        }
        let n = self.draft.path.len();
        self.draft
            .path_nodes
            .resize(n, plan_core::camera::WalkNode::default());
        if let Some(first) = self.draft.path.first().copied() {
            self.draft.position = first;
            if let Some(next) = self.draft.path.get(1) {
                self.draft.direction_deg = (*next - first).angle().to_degrees();
            }
        }
    }

    /// The path table of a walkthrough: speed and, per node, the plan
    /// position, camera height and an optional fixed look direction.
    fn walkthrough_page(&mut self, ui: &mut egui::Ui) {
        section(ui, "Walkthrough");
        row(ui, "Walking speed", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.draft.walk_speed)
                    .range(6.0..=600.0)
                    .suffix(" in/s"),
            )
        });
        ui.weak(format!(
            "{} nodes, {:.0}\" long, {:.1} s",
            self.draft.path.len(),
            self.draft.walk_length(),
            self.draft.walk_duration_s()
        ));
        section(ui, "Path nodes");
        let mut remove = None;
        let nodes = self.draft.path.len();
        self.draft
            .path_nodes
            .resize(nodes, plan_core::camera::WalkNode::default());
        for i in 0..nodes {
            ui.horizontal(|ui| {
                ui.label(format!("{}", i + 1));
                let p = &mut self.draft.path[i];
                ui.add(egui::DragValue::new(&mut p.x).speed(1.0).prefix("x "));
                ui.add(egui::DragValue::new(&mut p.y).speed(1.0).prefix("y "));
                let n = &mut self.draft.path_nodes[i];
                ui.add(
                    egui::DragValue::new(&mut n.height)
                        .range(12.0..=600.0)
                        .prefix("h ")
                        .suffix("\""),
                );
                let mut fixed = n.look_deg.is_some();
                if ui.checkbox(&mut fixed, "look").changed() {
                    n.look_deg = fixed.then_some(0.0);
                }
                if let Some(d) = &mut n.look_deg {
                    ui.add(
                        egui::DragValue::new(d)
                            .range(-360.0..=360.0)
                            .suffix("\u{B0}"),
                    );
                }
                ui.add(
                    egui::DragValue::new(&mut n.tilt_deg)
                        .range(
                            -plan_core::camera_view::MAX_TILT_DEG
                                ..=plan_core::camera_view::MAX_TILT_DEG,
                        )
                        .prefix("tilt ")
                        .suffix("\u{B0}"),
                );
                ui.add(
                    egui::DragValue::new(&mut n.hold_s)
                        .range(0.0..=120.0)
                        .speed(0.1)
                        .prefix("hold ")
                        .suffix(" s"),
                );
                if nodes > 2 && ui.small_button("\u{2715}").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.draft.path.remove(i);
            self.draft.path_nodes.remove(i);
        }
        ui.weak("A node without \"look\" faces along the path. Each node is a key frame: the camera tilts and holds as set there.");
        section(ui, "Record Walkthrough");
        let w = &mut self.draft.view.walk;
        row(ui, "Frames per second", |ui| {
            ui.add(
                egui::DragValue::new(&mut w.fps)
                    .range(plan_core::camera_view::MIN_FPS..=plan_core::camera_view::MAX_FPS)
                    .speed(0.2),
            )
        });
        ui.weak(format!(
            "{} frames over {:.1} s",
            (self.draft.walk_duration_s() * self.draft.view.walk.fps)
                .round()
                .max(1.0),
            self.draft.walk_duration_s()
        ));
    }

    fn kind_label(&self) -> &'static str {
        match self.draft.kind {
            CameraKind::FullCamera => "Full Camera",
            CameraKind::FloorCamera => "Floor Camera",
            CameraKind::PerspectiveOverview => "Perspective Overview",
            CameraKind::DollHouse => "Doll House View",
            CameraKind::GlassHouse => "Glass House",
            CameraKind::FramingOverview => "Framing Overview",
            CameraKind::CrossSection { back_clip: None } => "Cross Section/Elevation",
            CameraKind::CrossSection { .. } => "Back-Clipped Cross Section",
            CameraKind::WallElevation => "Wall Elevation",
            CameraKind::Orthographic => "Orthographic",
            CameraKind::Elevation => "Elevation",
            CameraKind::Walkthrough => "Walkthrough",
        }
    }

    /// The Camera tab: name, position, direction, angle of view, height,
    /// tilt, clipping, floors displayed and the plan display options.
    fn general(&mut self, ui: &mut egui::Ui) {
        section(ui, "General");
        row(ui, "Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.draft.name).desired_width(200.0))
        });
        row(ui, "Camera Type", |ui| ui.label(self.kind_label()));
        row(ui, "Floor", |ui| ui.label(&self.floor_name));
        self.general_options(ui);
        if self.draft.kind == CameraKind::Walkthrough {
            self.walkthrough_page(ui);
            self.display_options(ui);
            return;
        }
        self.clipping(ui);
        self.depth_cue_group(ui);
        if !self.is_section() {
            section(ui, "Floors Displayed");
            row(ui, "Show", |ui| {
                if self.draft.kind == CameraKind::FloorCamera {
                    ui.label("This floor, clipped at its ceiling");
                } else {
                    self.floors_combo(ui);
                }
            });
            if self.draft.kind != CameraKind::FloorCamera {
                self.floors_range(ui);
            }
        }
        self.rendering(ui);
        self.render_extras(ui);
        self.view_options(ui);
        self.display_options(ui);
    }

    /// The Floors Displayed choice: all floors, this floor and below, or the
    /// floors between two the user picks.
    fn floors_combo(&mut self, ui: &mut egui::Ui) {
        use plan_core::camera_view::FloorsDisplayed as F;
        let floors = &mut self.draft.view.floors;
        egui::ComboBox::from_id_salt("camera_floors")
            .selected_text(floors.label())
            .show_ui(ui, |ui| {
                for fl in F::ALL {
                    ui.selectable_value(floors, fl, fl.label());
                }
                if ui
                    .selectable_label(floors.is_picked(), "Pick floors")
                    .clicked()
                    && !floors.is_picked()
                {
                    let top = self.floor_names.len().saturating_sub(1).min(255) as u8;
                    let here = self.draft.floor.min(usize::from(top)) as u8;
                    *floors = F::Picked { from: 0, to: here };
                }
            });
    }

    /// The From and To floors of a per-floor pick.
    fn floors_range(&mut self, ui: &mut egui::Ui) {
        use plan_core::camera_view::FloorsDisplayed as F;
        if let F::Picked { from, to } = &mut self.draft.view.floors {
            let names = &self.floor_names;
            let name = |i: u8| {
                names
                    .get(usize::from(i))
                    .cloned()
                    .unwrap_or_else(|| format!("Floor {}", u32::from(i) + 1))
            };
            for (label, v) in [("From floor", &mut *from), ("To floor", &mut *to)] {
                row(ui, label, |ui| {
                    egui::ComboBox::from_id_salt(("camera_floor_pick", label))
                        .selected_text(name(*v))
                        .show_ui(ui, |ui| {
                            for i in 0..names.len().clamp(1, 255) {
                                ui.selectable_value(v, i as u8, name(i as u8));
                            }
                        })
                });
            }
            if *from > *to {
                std::mem::swap(from, to);
            }
        }
    }

    fn clipping(&mut self, ui: &mut egui::Ui) {
        section(ui, "Clipping");
        if self.draft.kind == CameraKind::Walkthrough {
            ui.weak("A walkthrough has no clip planes.");
        } else if let Some(back_clip) = back_clip_of(&mut self.draft) {
            let mut limited = back_clip.is_some();
            if ui.checkbox(&mut limited, "Back-clip the section").changed() {
                *back_clip = limited.then_some(120.0);
            }
            if let Some(v) = back_clip {
                self.fields.length_row(ui, "Back Clip Distance", "back", v);
            }
        } else {
            let d = &mut self.draft;
            let mut limited = d.clip_distance.is_some();
            if ui.checkbox(&mut limited, "Limit view distance").changed() {
                d.clip_distance = limited.then_some(DEFAULT_CONE_LENGTH);
            }
            if let Some(v) = &mut d.clip_distance {
                self.fields.length_row(ui, "Far Clip Distance", "clip", v);
            }
        }
        if is_elevation_camera(&self.draft) {
            self.scene_clipping(ui);
        }
    }

    /// The Scene Clipping group of a section or elevation (C-136..C-138,
    /// C-152, C-157): Poche, Framing Back Clip, Clip Sides, Clip Elevation,
    /// Clip to Room and the stepped cutting plane.
    fn scene_clipping(&mut self, ui: &mut egui::Ui) {
        section(ui, "Scene Clipping");
        let clip = &mut self.draft.view.clip;
        ui.checkbox(&mut clip.poche, "Poche");
        ui.checkbox(&mut clip.framing_back_clip, "Framing Back Clip");
        if clip.framing_back_clip {
            self.fields.length_row(
                ui,
                "Back Clip Framing After",
                "fback",
                &mut clip.framing_back_after,
            );
        }
        ui.checkbox(&mut clip.clip_sides, "Clip Sides")
            .on_hover_text("The view is as wide as the cross section line");
        ui.checkbox(&mut clip.clip_elevation, "Clip Elevation");
        if clip.clip_elevation {
            self.fields
                .length_row(ui, "Bottom Elevation", "cbot", &mut clip.bottom);
            self.fields
                .length_row(ui, "Top Elevation", "ctop", &mut clip.top);
            if clip.top < clip.bottom {
                std::mem::swap(&mut clip.top, &mut clip.bottom);
            }
        }
        ui.checkbox(&mut clip.clip_to_room, "Clip to Room");
        if clip.clip_to_room {
            ui.checkbox(
                &mut clip.ignore_railings,
                "Ignore Railings and Invisible Walls",
            );
            ui.checkbox(&mut clip.ignore_walls_above, "Ignore Walls Above");
        }
        ui.horizontal(|ui| {
            if ui.button("Add Break").clicked() {
                // A break goes in the middle of the widest piece.
                let half = self.section_len * 0.5;
                let (x0, x1, _) = clip
                    .plane
                    .spans(-half, half)
                    .into_iter()
                    .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))
                    .unwrap_or((-half, half, 0.0));
                clip.plane.add_break((x0 + x1) * 0.5);
            }
            ui.weak(format!("{} cutting plane piece(s)", clip.plane.pieces()));
        });
        const KEYS: [&str; 8] = [
            "piece0", "piece1", "piece2", "piece3", "piece4", "piece5", "piece6", "piece7",
        ];
        for (i, key) in KEYS.iter().enumerate().take(clip.plane.pieces()) {
            let mut off = clip.plane.offsets.get(i).copied().unwrap_or(0.0);
            self.fields
                .length_row(ui, &format!("Piece {} Offset", i + 1), key, &mut off);
            clip.plane.set_offset(i, off);
        }
        if clip.plane.is_stepped() {
            ui.horizontal(|ui| {
                if ui.button("Make Parallel").clicked() {
                    clip.plane.make_parallel(0);
                }
                if ui.button("Make Perpendicular").clicked() {
                    clip.plane.make_perpendicular(0);
                }
                if ui.button("Remove Break").clicked() {
                    clip.plane.remove_break(0);
                }
            });
        }
    }

    /// The Navigation group (C-121, DECISIONS 41): the steps this camera's
    /// pan, dolly, orbit, tilt and keyboard commands take.
    fn navigation(&mut self, ui: &mut egui::Ui) {
        section(ui, "Navigation");
        let v = &mut self.draft.view;
        self.fields.length_row(
            ui,
            "Incremental Move Distance",
            "movestep",
            &mut v.move_step,
        );
        self.fields.degrees_row(
            ui,
            "Incremental Rotate Angle",
            "deg_rotstep",
            &mut v.rotate_step,
        );
        v.move_step = v.move_step.max(1.0);
        v.rotate_step = v.rotate_step.clamp(1.0, 90.0);
    }

    fn display_options(&mut self, ui: &mut egui::Ui) {
        section(ui, "Display");
        ui.checkbox(&mut self.draft.view.show_in_plan, "Show camera in plan");
        ui.checkbox(&mut self.draft.view.locked, "Locked camera")
            .on_hover_text(
                "A locked camera cannot be moved, aimed, resized or tilted with its handles",
            );
    }

    /// The Backdrop tab: the default sky, a sky colour, or a picture from
    /// Chief's Backdrops folder (read from the install, never bundled).
    fn backdrop(&mut self, ui: &mut egui::Ui) {
        use plan_core::camera_view::BackdropKind;
        section(ui, "Backdrop");
        row(ui, "Type", |ui| {
            egui::ComboBox::from_id_salt("camera_backdrop_kind")
                .selected_text(self.draft.view.backdrop.kind.label())
                .show_ui(ui, |ui| {
                    for k in BackdropKind::ALL {
                        ui.selectable_value(&mut self.draft.view.backdrop.kind, k, k.label());
                    }
                });
        });
        match self.draft.view.backdrop.kind {
            BackdropKind::Default => {
                ui.weak("The technique's own sky and ground.");
            }
            BackdropKind::Color => {
                row(ui, "Sky color", |ui| {
                    ui.color_edit_button_srgb(&mut self.draft.view.backdrop.color)
                });
            }
            BackdropKind::Image => {
                let found = crate::shell::view3d_panel::backdrop::list_all();
                row(ui, "Picture", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.draft.view.backdrop.image)
                            .hint_text("file name or path")
                            .desired_width(220.0),
                    )
                });
                if found.is_empty() {
                    ui.weak("No Backdrops folder found. Type the path of a JPEG or PNG file.");
                } else {
                    ui.weak("Pictures in Chief's Backdrops folder:");
                    egui::ScrollArea::vertical()
                        .max_height(140.0)
                        .id_salt("camera_backdrop_list")
                        .show(ui, |ui| {
                            for name in &found {
                                let on = self.draft.view.backdrop.image == *name;
                                if ui.selectable_label(on, name).clicked() {
                                    self.draft.view.backdrop.image.clone_from(name);
                                }
                            }
                        });
                }
                let named = self.draft.view.backdrop.image.trim();
                if !named.is_empty()
                    && crate::shell::view3d_panel::backdrop::resolve(
                        named,
                        &crate::shell::view3d_panel::backdrop::backdrop_dirs(),
                    )
                    .is_none()
                {
                    ui.colored_label(
                        egui::Color32::from_rgb(0xC0, 0x40, 0x30),
                        "That picture was not found; the default sky is used.",
                    );
                }
                ui.weak("Shown behind the model by the techniques that draw a sky.");
            }
        }
        self.ground_and_fog(ui);
    }

    /// What lies below the horizon, and the distance haze.
    fn ground_and_fog(&mut self, ui: &mut egui::Ui) {
        use plan_core::camera_view::{GroundKind, FOG_FEET};
        let b = &mut self.draft.view.backdrop;
        section(ui, "Ground");
        row(ui, "Below the horizon", |ui| {
            egui::ComboBox::from_id_salt("camera_ground_kind")
                .selected_text(b.ground.label())
                .show_ui(ui, |ui| {
                    for k in GroundKind::ALL {
                        ui.selectable_value(&mut b.ground, k, k.label());
                    }
                });
        });
        if b.ground == GroundKind::Color {
            row(ui, "Ground color", |ui| {
                ui.color_edit_button_srgb(&mut b.ground_color)
            });
        }
        section(ui, "Fog");
        ui.checkbox(&mut b.fog.on, "Fog");
        if b.fog.on {
            row(ui, "Fog distance", |ui| {
                ui.add(
                    egui::DragValue::new(&mut b.fog.distance_ft)
                        .range(FOG_FEET.0..=FOG_FEET.1)
                        .speed(5.0)
                        .suffix(" ft"),
                )
            });
            let mut own = b.fog.color.is_some();
            if ui.checkbox(&mut own, "Own fog color").changed() {
                b.fog.color = own.then_some([200, 208, 216]);
            }
            if let Some(c) = &mut b.fog.color {
                row(ui, "Fog color", |ui| ui.color_edit_button_srgb(c));
            } else {
                ui.weak("The fog takes the horizon color of the sky.");
            }
            ui.weak("About 63 percent of the color is fog at the fog distance.");
        }
    }

    /// The Rendering tab: technique, Preview or Final View, shadows and the
    /// ambient and sun overrides.
    fn rendering(&mut self, ui: &mut egui::Ui) {
        section(ui, "Rendering");
        row(ui, "Technique", |ui| {
            egui::ComboBox::from_id_salt("camera_technique")
                .selected_text(self.extras.technique.label())
                .show_ui(ui, |ui| {
                    for t in RenderingTechnique::ALL {
                        if ui
                            .selectable_value(&mut self.extras.technique, t, t.label())
                            .changed()
                        {
                            self.draft.view.technique = Some(t.label().to_string());
                        }
                    }
                });
        });
        row(ui, "View quality", |ui| {
            for q in plan_core::camera_view::ViewQuality::ALL {
                ui.selectable_value(&mut self.draft.view.quality, q, q.label());
            }
        });
        ui.checkbox(&mut self.draft.view.shadows, "Cast shadows")
            .on_hover_text("Sun shadows in Final View");
        section(ui, "Lighting");
        let mut own = self.draft.view.ambient.is_some() || self.draft.view.sun_intensity.is_some();
        if ui
            .checkbox(&mut own, "Override the plan's lighting for this camera")
            .changed()
        {
            if own {
                self.draft.view.ambient = Some(self.plan_lighting.ambient);
                self.draft.view.sun_intensity = Some(self.plan_lighting.sun_intensity);
            } else {
                self.draft.view.ambient = None;
                self.draft.view.sun_intensity = None;
            }
        }
        if let Some(a) = &mut self.draft.view.ambient {
            row(ui, "Ambient light", |ui| {
                ui.add(egui::Slider::new(a, 0.0..=1.0))
            });
        }
        if let Some(i) = &mut self.draft.view.sun_intensity {
            row(ui, "Sun intensity", |ui| {
                ui.add(egui::Slider::new(i, 0.0..=2.0))
            });
        }
        if !own {
            ui.weak("The sun and lights come from 3D > Lighting.");
        }
        if !self.plan_lighting.sets.is_empty() {
            row(ui, "Light set", |ui| {
                let current = self
                    .draft
                    .view
                    .light_set
                    .clone()
                    .unwrap_or_else(|| "The plan's".to_string());
                egui::ComboBox::from_id_salt("camera_light_set")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        let none = self.draft.view.light_set.is_none();
                        if ui.selectable_label(none, "The plan's").clicked() {
                            self.draft.view.light_set = None;
                        }
                        for set in &self.plan_lighting.sets {
                            let on = self.draft.view.light_set.as_deref() == Some(&set.name);
                            if ui.selectable_label(on, &set.name).clicked() {
                                self.draft.view.light_set = Some(set.name.clone());
                            }
                        }
                    })
            });
        }
        if is_elevation_camera(&self.draft) {
            self.elevation_rendering(ui);
        }
    }

    /// The Label tab: the text drawn beside the camera in the plan and over
    /// the 3D view.
    fn label_tab(&mut self, ui: &mut egui::Ui) {
        section(ui, "Label");
        row(ui, "Label text", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.draft.view.label.text)
                    .hint_text(self.draft.name.clone())
                    .desired_width(200.0),
            )
        });
        ui.checkbox(
            &mut self.draft.view.label.show_in_plan,
            "Show the label beside the camera in the plan",
        );
        ui.checkbox(
            &mut self.draft.view.label.show_in_view,
            "Show the label over the 3D view",
        );
        ui.weak("An empty label uses the camera's name.");
    }

    /// Sets the sun from the date, time and latitude (`plan_materials`).
    pub fn set_sun_from_date(&mut self) {
        let i = self.sun_input;
        let sun = SunSettings::from_date_time_location((i.month, i.day), i.time_hours, i.latitude);
        self.draft.render.sun_azimuth_deg = sun.azimuth_deg;
        self.draft.render.sun_altitude_deg = sun.altitude_deg.max(1.0);
    }

    fn elevation_rendering(&mut self, ui: &mut egui::Ui) {
        section(ui, "Elevation rendering");
        ui.checkbox(&mut self.draft.render.hatch, "Hatch materials");
        ui.checkbox(&mut self.draft.render.shadows, "Shadows");
        if self.draft.render.shadows {
            let f = &mut self.fields;
            f.degrees_row(
                ui,
                "Sun azimuth (from north)",
                "deg_sun_az",
                &mut self.draft.render.sun_azimuth_deg,
            );
            f.degrees_row(
                ui,
                "Sun height",
                "deg_sun_alt",
                &mut self.draft.render.sun_altitude_deg,
            );
            let i = &mut self.sun_input;
            row(ui, "Date (month, day)", |ui| {
                ui.add(egui::DragValue::new(&mut i.month).range(1..=12));
                ui.add(egui::DragValue::new(&mut i.day).range(1..=31));
            });
            row(ui, "Solar time (hours)", |ui| {
                ui.add(
                    egui::DragValue::new(&mut i.time_hours)
                        .range(0.0..=24.0)
                        .speed(0.1),
                )
            });
            row(ui, "Latitude", |ui| {
                ui.add(
                    egui::DragValue::new(&mut i.latitude)
                        .range(-90.0..=90.0)
                        .speed(0.1),
                )
            });
            if ui.button("Set sun from date and time").clicked() {
                self.set_sun_from_date();
            }
        }
        if let Some(back_clip) = back_clip_of(&mut self.draft) {
            let mut limited = back_clip.is_some();
            if ui
                .checkbox(&mut limited, "Section back-clip depth")
                .changed()
            {
                *back_clip = limited.then_some(120.0);
            }
            if let Some(v) = back_clip {
                self.fields
                    .length_row(ui, "Depth behind the cut", "back_r", v);
            }
        }
        ui.checkbox(
            &mut self.draft.render.depth_weights,
            "Line weight by distance",
        );
        ui.checkbox(
            &mut self.draft.render.labels,
            "Labels (title, levels, roof pitch)",
        );
        ui.add_enabled(
            self.draft.render.labels,
            egui::Checkbox::new(
                &mut self.draft.vector.level_labels,
                "Level callouts (T.O. subfloor, T.O. plate)",
            ),
        );
        section(ui, "Vector View");
        ui.checkbox(
            &mut self.draft.vector.dimensions,
            "Automatic dimensions (floor-to-floor, openings)",
        );
        ui.checkbox(
            &mut self.draft.vector.material_labels,
            "Material labels (siding, brick, roofing)",
        );
        ui.checkbox(
            &mut self.draft.vector.layer_weights,
            "Line weights from the layers",
        );
        ui.checkbox(&mut self.draft.vector.hidden_dashed, "Dashed hidden lines");
        if ui.button("Export drawing as DXF\u{2026}").clicked() {
            self.export_requested = true;
        }
        section(ui, "Plan callout");
        ui.checkbox(&mut self.draft.callout.show, "Show the callout in the plan");
        let mut numbered = self.draft.callout.number.is_some();
        ui.horizontal(|ui| {
            if ui.checkbox(&mut numbered, "View number").changed() {
                self.draft.callout.number = numbered.then_some(1);
            }
            if let Some(n) = &mut self.draft.callout.number {
                ui.add(egui::DragValue::new(n).range(1..=99));
            }
        });
        if !numbered {
            ui.weak("Without a number the cameras are numbered in order.");
        }
    }
}

impl SpecPages for CameraDialog {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.draft.name.trim().is_empty() {
            return Some("Enter a camera name".into());
        }
        if !self.is_section()
            && self.draft.kind != CameraKind::Walkthrough
            && !(MIN_FOV_DEG..=MAX_FOV_DEG).contains(&self.draft.fov_deg)
        {
            return Some(format!(
                "Angle of view must be {MIN_FOV_DEG:.0}\u{B0} to {MAX_FOV_DEG:.0}\u{B0}"
            ));
        }
        if self.fields.any_invalid() {
            return Some("Fix the highlighted fields".into());
        }
        if self.draft.view.tilt_deg.abs() > plan_core::camera_view::MAX_TILT_DEG {
            return Some(format!(
                "Tilt must be within {:.0}\u{B0} either way",
                plan_core::camera_view::MAX_TILT_DEG
            ));
        }
        if self.draft.view.backdrop.kind == plan_core::camera_view::BackdropKind::Image
            && self.draft.view.backdrop.image.trim().is_empty()
        {
            return Some("Choose a backdrop picture or another backdrop type".into());
        }
        let r = &self.draft.render;
        if is_elevation_camera(&self.draft)
            && r.shadows
            && !(0.0..=90.0).contains(&r.sun_altitude_deg)
        {
            return Some("The sun height must be 0\u{B0} to 90\u{B0}".into());
        }
        None
    }

    fn page(&mut self, ui: &mut egui::Ui, tab: usize) {
        self.last_tab = tab;
        match tab {
            0 => self.general(ui),
            1 => self.positioning_tab(ui),
            2 => self.below_grade_tab(ui),
            3 => self.selected_defaults_tab(ui),
            4 => self.plan_display_tab(ui),
            5 => self.backdrop(ui),
            6 => self.layer_tab(ui),
            _ => self.label_tab(ui),
        }
        self.sync_section();
        self.sync_walkthrough();
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let ink = Stroke::new(1.2_f32, Color32::from_rgb(0x2F, 0x6C, 0xB3));
        let mut pts: Vec<Point> = if self.draft.kind == CameraKind::Walkthrough {
            self.draft.path.clone()
        } else if self.is_section() {
            let (a, b) = crate::tools::camera::section_line(&self.draft);
            vec![a, b, self.draft.position + self.draft.direction() * 36.0]
        } else {
            self.draft.symbol_points()
        };
        if pts.is_empty() {
            return;
        }
        let (mut lo, mut hi) = (pts[0], pts[0]);
        for p in &pts {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        let span = (hi.x - lo.x).max(hi.y - lo.y).max(1.0);
        let scale = f64::from(rect.width().min(rect.height())) / span * 0.9;
        let mid = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
        let to_screen = |p: Point| {
            Pos2::new(
                rect.center().x + ((p.x - mid.x) * scale) as f32,
                rect.center().y - ((p.y - mid.y) * scale) as f32,
            )
        };
        let s: Vec<Pos2> = pts.drain(..).map(to_screen).collect();
        if self.draft.kind == CameraKind::Walkthrough {
            painter.add(egui::Shape::line(s.clone(), ink));
            for p in &s {
                painter.circle_filled(*p, 2.5, ink.color);
            }
        } else if self.is_section() {
            painter.line_segment([s[0], s[1]], ink);
            painter.line_segment([s[2], to_screen(self.draft.position)], ink);
        } else {
            painter.add(egui::Shape::convex_polygon(
                s[3..6].to_vec(),
                Color32::from_rgba_unmultiplied(0x2F, 0x6C, 0xB3, 40),
                ink,
            ));
            painter.add(egui::Shape::convex_polygon(
                s[0..3].to_vec(),
                Color32::from_rgb(0x2F, 0x6C, 0xB3),
                Stroke::NONE,
            ));
        }
        painter.text(
            rect.center_bottom(),
            Align2::CENTER_BOTTOM,
            &self.draft.name,
            egui::FontId::proportional(11.0),
            Color32::from_gray(0x2B),
        );
    }
}

// ----- the camera's drawing in a layout -----

/// The 2D drawing of camera `id` for a layout box: the elevation or section
/// with the camera's own options. `None` for cameras that draw no 2D view.
pub fn camera_drawing(project: &Project, id: Id) -> Option<Drawing> {
    let c = project.camera(id)?;
    is_elevation_camera(c).then(|| render_elevation(project, c))
}

/// A layout render context whose camera boxes draw through
/// [`camera_drawing`].
pub fn layout_context(project: &Project) -> plan_layout::LayoutRenderContext<'_> {
    plan_layout::LayoutRenderContext::new(project)
        .with_camera_drawing(move |id| camera_drawing(project, id))
}

/// Sends camera `camera` to page `page` of `layout` as a camera box at the
/// largest scale that fits. `None` when the camera does not exist or has no
/// 2D view. The layout view's Send to Layout dialog does this for the app;
/// tests use it directly.
#[cfg_attr(not(test), allow(dead_code))]
pub fn send_camera_to_layout(
    layout: &mut plan_layout::Layout,
    project: &Project,
    camera: Id,
    page: u32,
) -> Option<Id> {
    is_elevation_camera(project.camera(camera)?).then_some(())?;
    let cx = layout_context(project);
    plan_layout::send_camera_to_layout(layout, &cx, page, camera, None)
}

// ----- lights (C-64..C-66) -----

/// Radiant intensity of a light of intensity 1.0 in the renderer's units
/// (irradiance is `intensity / distance^2`, inches): about 1.7 on a floor 7'
/// below the light.
pub const LIGHT_UNIT: f32 = 12_000.0;

/// Defaults for lights placed with the Add Lights tool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightDefaults {
    /// Height above the floor, inches; `None` hangs the light 12" below the
    /// ceiling of its floor.
    pub height: Option<f64>,
    pub intensity: f32,
    pub color: [u8; 3],
}

static LIGHT_DEFAULTS: Mutex<LightDefaults> = Mutex::new(LightDefaults {
    height: None,
    intensity: 1.0,
    color: [255, 244, 229],
});

pub fn light_defaults() -> LightDefaults {
    *LIGHT_DEFAULTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

pub fn set_light_defaults(d: LightDefaults) {
    *LIGHT_DEFAULTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = d;
}

/// The light fixtures of the electrical plan (ceiling lights, recessed cans,
/// pendants, sconces and rope lights) as lights; their `id` is the device id.
pub fn electrical_lights(project: &Project) -> Vec<PlanLight> {
    use plan_electrical::{DeviceKind, ElectricalLayer};
    let mut out = Vec::new();
    for (floor, f) in project.floors.iter().enumerate() {
        let Ok(Some(layer)) = f.electrical_as::<ElectricalLayer>() else {
            continue;
        };
        for d in layer.devices.iter().filter(|d| d.kind.is_light()) {
            let hung = matches!(
                d.kind,
                DeviceKind::CeilingLight | DeviceKind::RecessedCan | DeviceKind::PendantLight
            );
            let mut l = PlanLight::new(d.position, if hung { d.height - 6.0 } else { d.height });
            l.height = l.height.max(6.0);
            l.id = d.id;
            l.floor = floor;
            l.name = d.kind.name().to_string();
            l.intensity = match d.kind {
                DeviceKind::RecessedCan => 0.5,
                DeviceKind::WallSconce => 0.4,
                DeviceKind::RopeLight { .. } => 0.3,
                _ => 1.0,
            };
            out.push(l);
        }
    }
    out
}

/// Every light that can shine: the plan's own lights plus, when the plan
/// asks for it, the electrical fixtures. 3D > Lighting can switch every
/// interior light off.
#[cfg(test)]
pub fn all_lights(project: &Project) -> Vec<PlanLight> {
    lights_with_kind(project)
        .into_iter()
        .map(|(l, _)| l)
        .collect()
}

/// [`all_lights`] with a flag for the electrical fixtures (their ids come
/// from another counter than the plan lights').
fn lights_with_kind(project: &Project) -> Vec<(PlanLight, bool)> {
    if !project.lighting.interior_lights {
        return Vec::new();
    }
    let mut v: Vec<(PlanLight, bool)> = project.lights().into_iter().map(|l| (l, false)).collect();
    if project.light_settings().use_electrical {
        v.extend(electrical_lights(project).into_iter().map(|l| (l, true)));
    }
    v
}

/// The lights that shine under the light set a view uses: `view_choice` is a
/// camera's own set, else the plan's active one. With no set in use every
/// light follows its own on/off switch; with one, exactly the lights in the
/// set shine.
pub fn shining_lights(project: &Project, view_choice: Option<&str>) -> Vec<PlanLight> {
    let set = project.lighting.set_for(view_choice);
    lights_with_kind(project)
        .into_iter()
        .filter(|(l, fixture)| match set {
            Some(s) if *fixture => s.shines_fixture(l.id),
            Some(s) => s.shines(l.id),
            None => l.enabled,
        })
        .map(|(l, _)| l)
        .collect()
}

/// One light as the path tracer's point light (scene space: X, up, -plan Y).
/// Lights that do not cast shadows get a large sphere, which blurs their
/// shadows away (the renderer has no per-light shadow switch).
pub fn render_light(project: &Project, l: &PlanLight) -> PointLight {
    let elevation = project.floors.get(l.floor).map_or(0.0, |f| f.elevation);
    PointLight {
        position: [
            l.position.x as f32,
            (elevation + l.height) as f32,
            -l.position.y as f32,
        ],
        intensity: l.intensity.max(0.0) * LIGHT_UNIT,
        color: l.color.map(|c| f32::from(c) / 255.0),
        radius: if l.cast_shadows { 2.0 } else { 24.0 },
    }
}

/// The renderer's light list: every enabled light of [`all_lights`], under
/// the plan's active light set if there is one.
pub fn render_lights(project: &Project) -> Vec<PointLight> {
    render_lights_in(project, None)
}

/// [`render_lights`] for a view that picks its own light set.
pub fn render_lights_in(project: &Project, view_choice: Option<&str>) -> Vec<PointLight> {
    shining_lights(project, view_choice)
        .iter()
        .map(|l| render_light(project, l))
        .collect()
}

/// The Adjust Lights dialog (C-64): the plan's lights with on/off, height,
/// intensity, colour and shadows, the electrical-fixture switch and the
/// defaults of the Add Lights tool. Edits are kept in a draft until OK.
pub struct AdjustLightsDialog {
    draft: Vec<PlanLight>,
    original: Vec<Id>,
    use_electrical: bool,
    electrical: Vec<PlanLight>,
    defaults: LightDefaults,
    /// Light to highlight (the one that was double-clicked).
    pub focus: Option<Id>,
    /// The light sets, edited as a draft; the set in use is an index so a
    /// rename keeps it.
    sets: Vec<plan_core::camera_view::LightSet>,
    active_set: Option<usize>,
    /// The set whose lights are listed for editing.
    editing: Option<usize>,
    new_set: String,
}

impl AdjustLightsDialog {
    pub fn new(project: &Project) -> Self {
        let draft = project.lights();
        Self {
            original: draft.iter().map(|l| l.id).collect(),
            draft,
            use_electrical: project.light_settings().use_electrical,
            electrical: electrical_lights(project),
            defaults: light_defaults(),
            focus: None,
            sets: project.lighting.sets.clone(),
            active_set: project
                .lighting
                .active_set
                .as_deref()
                .and_then(|n| project.lighting.sets.iter().position(|s| s.name == n)),
            editing: None,
            new_set: String::new(),
        }
    }

    /// The light sets of the draft.
    #[cfg(test)]
    pub fn sets(&self) -> &[plan_core::camera_view::LightSet] {
        &self.sets
    }

    /// The ids of the plan lights and of the fixtures that are on right now:
    /// those of the set in use, else the ones whose switch is on.
    fn lights_on_now(&self) -> (Vec<Id>, Vec<Id>) {
        match self.active_set.and_then(|i| self.sets.get(i)) {
            Some(s) => (s.on.clone(), s.fixtures.clone()),
            None => (
                self.draft
                    .iter()
                    .filter(|l| l.enabled)
                    .map(|l| l.id)
                    .collect(),
                if self.use_electrical {
                    self.electrical.iter().map(|l| l.id).collect()
                } else {
                    Vec::new()
                },
            ),
        }
    }

    /// Adds a set called `name` holding the lights that are on now; `false`
    /// when the name is empty or taken.
    pub fn add_set(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || self.sets.iter().any(|s| s.name.eq_ignore_ascii_case(name)) {
            return false;
        }
        let (on, fixtures) = self.lights_on_now();
        self.sets.push(plan_core::camera_view::LightSet {
            name: name.to_string(),
            on,
            fixtures,
        });
        true
    }

    /// Uses set `i` for the whole plan (`None` leaves every light to its own
    /// switch).
    #[cfg(test)]
    pub fn use_set(&mut self, i: Option<usize>) {
        self.active_set = i.filter(|i| *i < self.sets.len());
    }

    /// Deletes set `i`; the set in use and the one being edited follow.
    pub fn delete_set(&mut self, i: usize) {
        if i >= self.sets.len() {
            return;
        }
        self.sets.remove(i);
        let shift = |v: Option<usize>| match v {
            Some(k) if k == i => None,
            Some(k) if k > i => Some(k - 1),
            other => other,
        };
        self.active_set = shift(self.active_set);
        self.editing = shift(self.editing);
    }

    #[cfg(test)]
    pub fn lights_mut(&mut self) -> &mut Vec<PlanLight> {
        &mut self.draft
    }

    #[cfg(test)]
    pub fn sets_mut(&mut self) -> &mut Vec<plan_core::camera_view::LightSet> {
        &mut self.sets
    }

    #[cfg(test)]
    pub fn set_use_electrical(&mut self, on: bool) {
        self.use_electrical = on;
    }

    /// Writes the draft into `project`: edits, removals and the options.
    pub fn apply(&self, project: &mut Project) {
        for id in &self.original {
            if !self.draft.iter().any(|l| l.id == *id) {
                project.remove_light(*id);
            }
        }
        for l in &self.draft {
            let edited = l.clone();
            project.update_light(l.id, |x| {
                let (id, floor) = (x.id, x.floor);
                *x = edited;
                x.id = id;
                x.floor = floor;
            });
        }
        project.set_light_settings(plan_core::camera::LightSettings {
            use_electrical: self.use_electrical,
        });
        set_light_defaults(self.defaults);
        self.apply_sets(project);
    }

    /// Writes the light sets: a deleted light leaves every set, empty names
    /// are dropped and a repeated name gets a number.
    fn apply_sets(&self, project: &mut Project) {
        let alive: Vec<Id> = self.draft.iter().map(|l| l.id).collect();
        let fixtures: Vec<Id> = self.electrical.iter().map(|l| l.id).collect();
        let mut out: Vec<plan_core::camera_view::LightSet> = Vec::new();
        let mut active = None;
        for (i, s) in self.sets.iter().enumerate() {
            let base = s.name.trim();
            if base.is_empty() {
                continue;
            }
            let mut name = base.to_string();
            let mut n = 2;
            while out.iter().any(|o| o.name.eq_ignore_ascii_case(&name)) {
                name = format!("{base} {n}");
                n += 1;
            }
            if self.active_set == Some(i) {
                active = Some(name.clone());
            }
            out.push(plan_core::camera_view::LightSet {
                name,
                on: s
                    .on
                    .iter()
                    .copied()
                    .filter(|id| alive.contains(id))
                    .collect(),
                fixtures: s
                    .fixtures
                    .iter()
                    .copied()
                    .filter(|id| fixtures.contains(id))
                    .collect(),
            });
        }
        project.lighting.sets = out;
        project.lighting.active_set = active;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        egui::Window::new("Adjust Lights")
            .id(egui::Id::new("adjust_lights_dialog"))
            .open(&mut open)
            .collapsible(false)
            .default_width(560.0)
            .show(ctx, |ui| {
                self.contents(ui);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                });
            });
        if !open {
            outcome = Outcome::Cancel;
        }
        // Enter is OK and Escape is Cancel unless a field is being edited.
        if outcome == Outcome::Open && ctx.memory(|m| m.focused()).is_none() {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                outcome = Outcome::Ok;
            } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                outcome = Outcome::Cancel;
            }
        }
        outcome
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        section(ui, "Lights in the plan");
        if self.draft.is_empty() {
            ui.weak("No lights yet. Use Add Lights and click in the plan.");
        }
        let mut remove = None;
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                for (i, l) in self.draft.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut l.enabled, "");
                        let name = egui::TextEdit::singleline(&mut l.name).desired_width(90.0);
                        let r = ui.add(name);
                        if self.focus == Some(l.id) {
                            r.highlight();
                        }
                        ui.add(
                            egui::DragValue::new(&mut l.height)
                                .range(0.0..=600.0)
                                .prefix("h ")
                                .suffix("\""),
                        );
                        ui.add(
                            egui::DragValue::new(&mut l.intensity)
                                .range(0.0..=20.0)
                                .speed(0.05)
                                .prefix("power "),
                        );
                        ui.color_edit_button_srgb(&mut l.color);
                        ui.checkbox(&mut l.cast_shadows, "shadows");
                        if ui.small_button("\u{2715}").clicked() {
                            remove = Some(i);
                        }
                    });
                }
            });
        if let Some(i) = remove {
            self.draft.remove(i);
        }
        section(ui, "Electrical fixtures");
        ui.checkbox(
            &mut self.use_electrical,
            format!(
                "Lighting fixtures of the electrical plan emit light ({})",
                self.electrical.len()
            ),
        );
        self.light_sets(ui);
        section(ui, "New lights");
        let d = &mut self.defaults;
        let mut fixed = d.height.is_some();
        ui.horizontal(|ui| {
            if ui.checkbox(&mut fixed, "Fixed height").changed() {
                d.height = fixed.then_some(84.0);
            }
            if let Some(h) = &mut d.height {
                ui.add(egui::DragValue::new(h).range(0.0..=600.0).suffix("\""));
            } else {
                ui.weak("12\" below the ceiling");
            }
        });
        ui.horizontal(|ui| {
            ui.label("Power");
            ui.add(
                egui::DragValue::new(&mut d.intensity)
                    .range(0.0..=20.0)
                    .speed(0.05),
            );
            ui.color_edit_button_srgb(&mut d.color);
        });
    }
}

impl AdjustLightsDialog {
    /// The Light Sets section: which set is in use, the sets and the lights
    /// each one turns on.
    fn light_sets(&mut self, ui: &mut egui::Ui) {
        section(ui, "Light sets");
        row(ui, "In use", |ui| {
            let current = self
                .active_set
                .and_then(|i| self.sets.get(i))
                .map_or("Each light's own switch".to_string(), |s| s.name.clone());
            egui::ComboBox::from_id_salt("light_set_in_use")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(self.active_set.is_none(), "Each light's own switch")
                        .clicked()
                    {
                        self.active_set = None;
                    }
                    for (i, s) in self.sets.iter().enumerate() {
                        if ui
                            .selectable_label(self.active_set == Some(i), &s.name)
                            .clicked()
                        {
                            self.active_set = Some(i);
                        }
                    }
                });
        });
        let mut delete = None;
        for i in 0..self.sets.len() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.sets[i].name).desired_width(140.0));
                let open = self.editing == Some(i);
                if ui.selectable_label(open, "Lights\u{2026}").clicked() {
                    self.editing = if open { None } else { Some(i) };
                }
                if ui.small_button("\u{2715}").clicked() {
                    delete = Some(i);
                }
            });
            if self.editing == Some(i) {
                self.set_members(ui, i);
            }
        }
        if let Some(i) = delete {
            self.delete_set(i);
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_set)
                    .hint_text("name of a new set")
                    .desired_width(140.0),
            );
            let name = self.new_set.trim().to_string();
            let free =
                !name.is_empty() && !self.sets.iter().any(|s| s.name.eq_ignore_ascii_case(&name));
            if ui
                .add_enabled(free, egui::Button::new("Add Set"))
                .on_hover_text("Makes a set of the lights that are on now")
                .clicked()
                && self.add_set(&name)
            {
                self.new_set.clear();
            }
        });
    }

    /// The check list of the lights in set `i`.
    fn set_members(&mut self, ui: &mut egui::Ui, i: usize) {
        let lights: Vec<(Id, String)> = self.draft.iter().map(|l| (l.id, l.name.clone())).collect();
        let fixtures: Vec<(Id, String)> = self
            .electrical
            .iter()
            .map(|l| (l.id, format!("{} (electrical)", l.name)))
            .collect();
        let set = &mut self.sets[i];
        ui.indent(("light_set_members", i), |ui| {
            if lights.is_empty() && fixtures.is_empty() {
                ui.weak("There are no lights in the plan yet.");
            }
            for (id, name) in &lights {
                let mut on = set.on.contains(id);
                if ui.checkbox(&mut on, name).changed() {
                    if on {
                        set.on.push(*id);
                    } else {
                        set.on.retain(|x| x != id);
                    }
                }
            }
            for (id, name) in &fixtures {
                let mut on = set.fixtures.contains(id);
                if ui.checkbox(&mut on, name).changed() {
                    if on {
                        set.fixtures.push(*id);
                    } else {
                        set.fixtures.retain(|x| x != id);
                    }
                }
            }
        });
    }
}

// ----- 3D > Lighting -----

/// The Lighting dialog (C-62, C-63, C-64): the sun (angle, height, strength,
/// or a date, time and latitude), the ambient light, the interior lights on
/// or off and the plan's lights with their power. Edits stay in a draft until
/// OK, which is one undo step.
pub struct LightingDialog {
    draft: plan_core::camera_view::Lighting,
    lights: Vec<PlanLight>,
    electrical: usize,
    date: SunInput,
    adjust_requested: bool,
}

impl LightingDialog {
    pub fn new(project: &Project) -> Self {
        Self {
            draft: project.lighting.clone(),
            lights: project.lights(),
            electrical: electrical_lights(project).len(),
            date: project
                .lighting
                .from_date
                .map_or_else(SunInput::default, |d| SunInput {
                    month: d.month,
                    day: d.day,
                    time_hours: d.hours,
                    latitude: d.latitude,
                }),
            adjust_requested: false,
        }
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut plan_core::camera_view::Lighting {
        &mut self.draft
    }

    #[cfg(test)]
    pub fn lights_mut(&mut self) -> &mut Vec<PlanLight> {
        &mut self.lights
    }

    /// Did the user click "Adjust Lights..."? The host opens that dialog.
    pub fn take_adjust_request(&mut self) -> bool {
        std::mem::take(&mut self.adjust_requested)
    }

    /// Puts the sun where the date, time and latitude say it is.
    pub fn set_sun_from_date(&mut self) {
        let i = self.date;
        let sun = SunSettings::from_date_time_location((i.month, i.day), i.time_hours, i.latitude);
        self.draft.sun_azimuth_deg = sun.azimuth_deg;
        self.draft.sun_altitude_deg = sun.altitude_deg.clamp(1.0, 90.0);
        self.draft.from_date = Some(plan_core::camera_view::SunDate {
            month: i.month,
            day: i.day,
            hours: i.time_hours,
            latitude: i.latitude,
        });
    }

    /// Writes the draft into `project`: the lighting and each light's power
    /// and on/off switch.
    pub fn apply(&self, project: &mut Project) {
        // The sets themselves are edited in Adjust Lights; here only the one
        // in use is picked.
        let sets = std::mem::take(&mut project.lighting.sets);
        project.lighting = self.draft.clone();
        project.lighting.sets = sets;
        let in_use = project.lighting.active_set.take();
        project.lighting.active_set = in_use.filter(|n| project.lighting.set(n).is_some());
        for l in &self.lights {
            let (enabled, intensity) = (l.enabled, l.intensity);
            project.update_light(l.id, |x| {
                x.enabled = enabled;
                x.intensity = intensity;
            });
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        egui::Window::new("Lighting")
            .id(egui::Id::new("lighting_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                self.contents(ui);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                });
            });
        if !open {
            outcome = Outcome::Cancel;
        }
        // Enter is OK and Escape is Cancel unless a field is being edited.
        if outcome == Outcome::Open && ctx.memory(|m| m.focused()).is_none() {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                outcome = Outcome::Ok;
            } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                outcome = Outcome::Cancel;
            }
        }
        outcome
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        section(ui, "Sun");
        row(ui, "Direction (from north)", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.draft.sun_azimuth_deg)
                    .range(0.0..=360.0)
                    .suffix("\u{B0}"),
            )
        });
        row(ui, "Height above horizon", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.draft.sun_altitude_deg)
                    .range(1.0..=90.0)
                    .suffix("\u{B0}"),
            )
        });
        row(ui, "Sun intensity", |ui| {
            ui.add(egui::Slider::new(&mut self.draft.sun_intensity, 0.0..=2.0))
        });
        row(ui, "Ambient light", |ui| {
            ui.add(egui::Slider::new(&mut self.draft.ambient, 0.0..=1.0))
        });
        ui.collapsing("Set the sun from a date", |ui| {
            let i = &mut self.date;
            row(ui, "Month, day", |ui| {
                ui.add(egui::DragValue::new(&mut i.month).range(1..=12));
                ui.add(egui::DragValue::new(&mut i.day).range(1..=31));
            });
            row(ui, "Solar time (hours)", |ui| {
                ui.add(
                    egui::DragValue::new(&mut i.time_hours)
                        .range(0.0..=24.0)
                        .speed(0.1),
                )
            });
            row(ui, "Latitude", |ui| {
                ui.add(
                    egui::DragValue::new(&mut i.latitude)
                        .range(-90.0..=90.0)
                        .speed(0.1),
                )
            });
            if ui.button("Set sun from date and time").clicked() {
                self.set_sun_from_date();
            }
        });
        section(ui, "Interior lights");
        ui.checkbox(
            &mut self.draft.interior_lights,
            "Interior lights are on (the plan's lights and the electrical fixtures)",
        );
        if !self.draft.sets.is_empty() {
            row(ui, "Light set", |ui| {
                let current = self
                    .draft
                    .active_set
                    .clone()
                    .unwrap_or_else(|| "Each light's own switch".to_string());
                egui::ComboBox::from_id_salt("lighting_light_set")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                self.draft.active_set.is_none(),
                                "Each light's own switch",
                            )
                            .clicked()
                        {
                            self.draft.active_set = None;
                        }
                        let names: Vec<String> =
                            self.draft.sets.iter().map(|s| s.name.clone()).collect();
                        for n in names {
                            let on = self.draft.active_set.as_deref() == Some(n.as_str());
                            if ui.selectable_label(on, &n).clicked() {
                                self.draft.active_set = Some(n);
                            }
                        }
                    })
            });
        }
        ui.add_enabled_ui(self.draft.interior_lights, |ui| {
            if self.lights.is_empty() {
                ui.weak("No lights in the plan yet. Use 3D > Lighting > Add Lights.");
            }
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .id_salt("lighting_list")
                .show(ui, |ui| {
                    for l in &mut self.lights {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut l.enabled, &l.name);
                            ui.add(
                                egui::DragValue::new(&mut l.intensity)
                                    .range(0.0..=20.0)
                                    .speed(0.05)
                                    .prefix("power "),
                            );
                        });
                    }
                });
            if self.electrical > 0 {
                ui.weak(format!(
                    "{} light fixtures of the electrical plan are included.",
                    self.electrical
                ));
            }
        });
        if ui.button("Adjust Lights\u{2026}").clicked() {
            self.adjust_requested = true;
        }
    }
}

// ----- Record Walkthrough -----

/// Picture sizes the Record Walkthrough dialog offers.
pub const RECORD_SIZES: [(&str, u32, u32); 4] = [
    ("640 x 480", 640, 480),
    ("960 x 720", 960, 720),
    ("1280 x 720 (HD)", 1280, 720),
    ("1920 x 1080 (Full HD)", 1920, 1080),
];

/// What the Record Walkthrough dialog decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordOutcome {
    Open,
    Cancel,
    Record,
}

/// The Record Walkthrough dialog: frame rate, picture size, samples, the
/// format (an AVI movie, a PNG sequence or both) and the folder they go to.
pub struct RecordDialog {
    pub camera: Id,
    pub settings: plan_core::camera_view::WalkRecord,
    pub folder: String,
    duration_s: f64,
    name: String,
}

impl RecordDialog {
    pub fn new(camera: &CameraObject, folder: String) -> Self {
        Self {
            camera: camera.id,
            settings: camera.view.walk,
            folder,
            duration_s: camera.walk_duration_s(),
            name: camera.name.clone(),
        }
    }

    /// The settings with every value in range.
    pub fn clamped(&self) -> plan_core::camera_view::WalkRecord {
        self.settings.clamped()
    }

    /// How many frames the walk comes to at the chosen rate.
    pub fn frame_count(&self) -> usize {
        ((self.duration_s * self.clamped().fps).round() as usize).max(1)
    }

    /// Can recording start (a folder is named)?
    pub fn ready(&self) -> bool {
        !self.folder.trim().is_empty()
    }

    pub fn show(&mut self, ctx: &egui::Context) -> RecordOutcome {
        let mut outcome = RecordOutcome::Open;
        let mut open = true;
        egui::Window::new("Record Walkthrough")
            .id(egui::Id::new("record_walkthrough_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("{} ({:.1} s)", self.name, self.duration_s));
                section(ui, "Frames");
                let w = &mut self.settings;
                row(ui, "Frames per second", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut w.fps)
                            .range(
                                plan_core::camera_view::MIN_FPS..=plan_core::camera_view::MAX_FPS,
                            )
                            .speed(0.2),
                    )
                });
                row(ui, "Picture size", |ui| {
                    let current = RECORD_SIZES
                        .iter()
                        .find(|(_, a, b)| (*a, *b) == (w.width, w.height))
                        .map_or("Custom", |(n, _, _)| *n);
                    egui::ComboBox::from_id_salt("record_size")
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for (name, a, b) in RECORD_SIZES {
                                if ui.selectable_label(current == name, name).clicked() {
                                    (w.width, w.height) = (a, b);
                                }
                            }
                        });
                });
                row(ui, "Samples per pixel", |ui| {
                    ui.add(egui::DragValue::new(&mut w.samples).range(1..=256))
                });
                section(ui, "Output");
                row(ui, "Save as", |ui| {
                    egui::ComboBox::from_id_salt("record_format")
                        .selected_text(w.format.label())
                        .show_ui(ui, |ui| {
                            for f in plan_core::camera_view::RecordFormat::ALL {
                                ui.selectable_value(&mut w.format, f, f.label());
                            }
                        })
                });
                if w.format.video() {
                    row(ui, "Video quality", |ui| {
                        ui.add(egui::Slider::new(&mut w.quality, 30..=100))
                    });
                }
                section(ui, "Folder");
                ui.add(
                    egui::TextEdit::singleline(&mut self.folder)
                        .hint_text(if w.format.video() {
                            "folder for the .avi movie"
                        } else {
                            "folder for frame_0001.png, frame_0002.png ..."
                        })
                        .desired_width(320.0),
                );
                if ui.button("Choose Folder\u{2026}").clicked() {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title("Folder for the walkthrough frames")
                        .pick_folder()
                    {
                        self.folder = dir.display().to_string();
                    }
                }
                ui.weak(format!(
                    "{} frames at {:.0} fps, written as {}.",
                    self.frame_count(),
                    self.clamped().fps,
                    match self.clamped().format {
                        plan_core::camera_view::RecordFormat::Video => "one Motion-JPEG .avi movie",
                        plan_core::camera_view::RecordFormat::Frames => "a numbered PNG sequence",
                        plan_core::camera_view::RecordFormat::Both =>
                            "a movie and a numbered PNG sequence",
                    }
                ));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(self.ready(), egui::Button::new("Record"))
                        .clicked()
                    {
                        outcome = RecordOutcome::Record;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = RecordOutcome::Cancel;
                    }
                });
            });
        if !open {
            outcome = RecordOutcome::Cancel;
        }
        outcome
    }
}

// ----- ray tracing (C-51, C-52, C-63) -----

/// Image size presets of the Ray Trace dialog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SizePreset {
    P1280x960,
    P1920x1080,
}

impl SizePreset {
    pub const ALL: [SizePreset; 2] = [SizePreset::P1280x960, SizePreset::P1920x1080];

    pub fn dims(self) -> (u32, u32) {
        match self {
            SizePreset::P1280x960 => (1280, 960),
            SizePreset::P1920x1080 => (1920, 1080),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SizePreset::P1280x960 => "1280 x 960",
            SizePreset::P1920x1080 => "1920 x 1080",
        }
    }
}

/// Samples per pixel presets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SamplesPreset {
    S64,
    S256,
    S1024,
}

impl SamplesPreset {
    pub const ALL: [SamplesPreset; 3] = [
        SamplesPreset::S64,
        SamplesPreset::S256,
        SamplesPreset::S1024,
    ];

    pub fn count(self) -> u32 {
        match self {
            SamplesPreset::S64 => 64,
            SamplesPreset::S256 => 256,
            SamplesPreset::S1024 => 1024,
        }
    }
}

/// The two techniques the ray tracer offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RtTechnique {
    PhysicallyBased,
    Clay,
}

impl RtTechnique {
    pub const ALL: [RtTechnique; 2] = [RtTechnique::PhysicallyBased, RtTechnique::Clay];

    pub fn label(self) -> &'static str {
        match self {
            RtTechnique::PhysicallyBased => "Physically Based",
            RtTechnique::Clay => "Clay",
        }
    }

    fn render_technique(self) -> Technique {
        match self {
            RtTechnique::PhysicallyBased => Technique::PhysicallyBased,
            RtTechnique::Clay => Technique::Clay,
        }
    }
}

/// How the ray tracer paints the sky.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RtSky {
    /// Preetham analytic clear sky: bright near the sun and the horizon.
    Analytic,
    /// The two-colour zenith-to-horizon gradient.
    Gradient,
}

impl RtSky {
    pub const ALL: [RtSky; 2] = [RtSky::Analytic, RtSky::Gradient];

    pub fn label(self) -> &'static str {
        match self {
            RtSky::Analytic => "Clear sky",
            RtSky::Gradient => "Gradient",
        }
    }
}

/// Size of a saved image relative to the render size ("Save Image" at 2x or 4x
/// renders again at that size).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SaveScale {
    X1,
    X2,
    X4,
}

impl SaveScale {
    pub const ALL: [SaveScale; 3] = [SaveScale::X1, SaveScale::X2, SaveScale::X4];

    pub fn factor(self) -> u32 {
        match self {
            SaveScale::X1 => 1,
            SaveScale::X2 => 2,
            SaveScale::X4 => 4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SaveScale::X1 => "Same size",
            SaveScale::X2 => "2x size",
            SaveScale::X4 => "4x size",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RtPhase {
    Idle,
    Running,
    Done,
    /// Stopped early; the image so far can still be saved.
    Cancelled,
    Failed(String),
}

/// A progressive image as display bytes.
#[derive(Clone, Debug)]
struct Preview {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

struct Shared {
    done: AtomicU32,
    total: u32,
    cancel: AtomicBool,
    latest: Mutex<Option<Preview>>,
    finished: Mutex<Option<Result<Image, String>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct RayTraceDialog {
    pub open: bool,
    pub size: SizePreset,
    pub samples: SamplesPreset,
    pub technique: RtTechnique,
    /// Sun date `(month, day)`, local solar time in hours and latitude (C-63).
    pub date: (u32, u32),
    pub time_hours: f64,
    pub latitude: f64,
    /// Replaces the size preset (tests render tiny images).
    pub size_override: Option<(u32, u32)>,
    /// Sun Angle in manual mode: this azimuth and altitude replace the ones
    /// worked out from the date, time and latitude.
    pub manual_sun: Option<(f64, f64)>,
    /// Point lights of the plan, set by the 3D panel before a render starts.
    pub lights: Vec<PointLight>,
    /// Rectangular area lights (sampled directly, so small bright panels are
    /// clean at low sample counts).
    pub areas: Vec<AreaLight>,
    /// Sky model and, for the clear sky, its haze (turbidity, 2 clear to 10 hazy).
    pub sky: RtSky,
    pub turbidity: f32,
    /// Exposure compensation in stops (EV) applied before tone mapping.
    pub exposure_ev: f32,
    /// Smooth the noise with the albedo/normal-guided filter.
    pub denoise: bool,
    /// Lens diameter in inches for depth of field (0 is a pinhole) and the
    /// focus distance in inches (0 focuses on what is at the image centre).
    pub aperture_in: f32,
    pub focus_in: f32,
    /// Size of the saved image.
    pub save_scale: SaveScale,
    pub message: String,
    phase: RtPhase,
    /// The scene and camera of the last render, kept for a bigger Save Image.
    job: Option<(Scene, plan_render::Camera)>,
    /// A larger render in progress that writes its PNG here when done.
    pending_save: Option<std::path::PathBuf>,
    shared: Option<Arc<Shared>>,
    preview: Option<Preview>,
    preview_dirty: bool,
    image: Option<Image>,
    texture: Option<egui::TextureHandle>,
}

impl Default for RayTraceDialog {
    fn default() -> Self {
        Self {
            open: false,
            // Preferences > Render.
            size: super::preferences::render_size_preset(),
            samples: super::preferences::render_samples_preset(),
            technique: if super::preferences::current().render_clay {
                RtTechnique::Clay
            } else {
                RtTechnique::PhysicallyBased
            },
            // Mid-afternoon at the summer solstice in Metro Atlanta.
            date: (6, 21),
            time_hours: 15.0,
            latitude: super::preferences::current().render_latitude,
            size_override: None,
            manual_sun: None,
            lights: Vec::new(),
            areas: Vec::new(),
            sky: RtSky::Analytic,
            turbidity: 2.5,
            exposure_ev: 0.0,
            denoise: false,
            aperture_in: 0.0,
            focus_in: 0.0,
            save_scale: SaveScale::X1,
            message: String::new(),
            phase: RtPhase::Idle,
            job: None,
            pending_save: None,
            shared: None,
            preview: None,
            preview_dirty: false,
            image: None,
            texture: None,
        }
    }
}

impl RayTraceDialog {
    pub fn phase(&self) -> &RtPhase {
        &self.phase
    }

    pub fn is_running(&self) -> bool {
        self.phase == RtPhase::Running
    }

    /// The finished (or cancelled) image.
    pub fn image(&self) -> Option<&Image> {
        self.image.as_ref()
    }

    /// `(samples done, samples total)` of the current or last render.
    pub fn progress(&self) -> (u32, u32) {
        self.shared
            .as_ref()
            .map_or((0, 0), |s| (s.done.load(Ordering::Relaxed), s.total))
    }

    /// The sun for the chosen date, time and latitude.
    pub fn sun(&self) -> SunSettings {
        let mut s = SunSettings::from_date_time_location(self.date, self.time_hours, self.latitude);
        if let Some((azimuth, altitude)) = self.manual_sun {
            s.azimuth_deg = azimuth;
            s.altitude_deg = altitude;
            if altitude <= 0.0 {
                s.intensity = 0.0;
            } else if s.intensity <= 0.0 {
                s.intensity = (altitude.to_radians().sin() as f32).sqrt();
            }
        }
        s
    }

    /// The render settings the dialog describes (always valid: sizes and
    /// sample counts come from the presets).
    pub fn settings(&self) -> RenderSettings {
        let (width, height) = self.size_override.unwrap_or_else(|| self.size.dims());
        RenderSettings {
            width: width.max(1),
            height: height.max(1),
            samples: self.samples.count(),
            technique: self.technique.render_technique(),
            exposure: self.exposure_ev.clamp(-6.0, 6.0).exp2(),
            denoise: self.denoise,
            preview_blocks: true,
            ..RenderSettings::default()
        }
    }

    /// Sky plus the sun (none when it is below the horizon).
    pub fn environment(&self) -> Environment {
        let s = self.sun();
        let sun = (s.altitude_deg > 0.0).then(|| {
            let mut sun = Sun::from_azimuth_altitude(s.azimuth_deg as f32, s.altitude_deg as f32);
            sun.intensity *= s.intensity.clamp(0.1, 1.0);
            sun.color = s.color.map(|c| f32::from(c) / 255.0);
            sun
        });
        Environment {
            sun,
            sky_model: match self.sky {
                RtSky::Analytic => SkyModel::Preetham {
                    turbidity: self.turbidity.clamp(2.0, 10.0),
                },
                RtSky::Gradient => SkyModel::Gradient,
            },
            ..Environment::default()
        }
    }

    /// Starts rendering `scene` from `camera` on a background thread.
    pub fn start(&mut self, scene: Scene, camera: plan_render::Camera) {
        self.pending_save = None;
        self.start_scaled(scene, camera, 1);
    }

    /// Renders `scene` at `scale` times the chosen size (a larger Save Image).
    /// The progressive preview is only shown for the normal size.
    fn start_scaled(&mut self, scene: Scene, camera: plan_render::Camera, scale: u32) {
        if self.is_running() {
            return;
        }
        let settings = self.settings().scaled(scale);
        let hires = settings.width > self.settings().width;
        let env = self.environment();
        let lights = self.lights.clone();
        let areas = self.areas.clone();
        // Depth of field: the dialog's lens and focus override the camera's.
        let camera = plan_render::Camera {
            aperture: self.aperture_in.max(0.0),
            focus_dist: self.focus_in.max(0.0),
            ..camera
        };
        self.job = Some((scene.clone(), camera));
        let shared = Arc::new(Shared {
            done: AtomicU32::new(0),
            total: settings.samples,
            cancel: AtomicBool::new(false),
            latest: Mutex::new(None),
            finished: Mutex::new(None),
        });
        let worker = Arc::clone(&shared);
        let spawned = std::thread::Builder::new()
            .name("ray-trace".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    let renderer = Renderer::new(&scene);
                    let mut progress = |img: &Image, done: u32| {
                        worker.done.store(done, Ordering::Relaxed);
                        if !hires {
                            *lock(&worker.latest) = Some(Preview {
                                width: img.width,
                                height: img.height,
                                rgba: img.rgba.clone(),
                            });
                        }
                        !worker.cancel.load(Ordering::Relaxed)
                    };
                    renderer.render_progressive_with_areas(
                        &camera,
                        &env,
                        (&lights, &areas),
                        &settings,
                        &mut progress,
                    )
                }));
                *lock(&worker.finished) =
                    Some(result.map_err(|_| "The renderer stopped unexpectedly".to_string()));
            });
        match spawned {
            Ok(_) => {
                self.shared = Some(shared);
                self.phase = RtPhase::Running;
                self.image = None;
                self.preview = None;
                self.message.clear();
            }
            Err(e) => self.phase = RtPhase::Failed(format!("Could not start the renderer: {e}")),
        }
    }

    /// Asks a running render to stop after its current pass.
    pub fn cancel(&mut self) {
        if let Some(s) = &self.shared {
            s.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Moves what the worker produced into the dialog; call every frame.
    pub fn poll(&mut self) {
        let Some(shared) = self.shared.clone() else {
            return;
        };
        if let Some(p) = lock(&shared.latest).take() {
            self.preview = Some(p);
            self.preview_dirty = true;
        }
        let finished = lock(&shared.finished).take();
        match finished {
            Some(Ok(image)) => {
                if self.pending_save.is_none() {
                    self.preview = Some(Preview {
                        width: image.width,
                        height: image.height,
                        rgba: image.rgba.clone(),
                    });
                    self.preview_dirty = true;
                }
                self.image = Some(image);
                let cancelled = shared.cancel.load(Ordering::Relaxed);
                self.phase = if cancelled {
                    RtPhase::Cancelled
                } else {
                    RtPhase::Done
                };
                if let Some(path) = self.pending_save.take() {
                    self.message = match self.save_png(&path) {
                        Ok(()) => format!("Saved {}", path.display()),
                        Err(e) => format!("Could not save: {e}"),
                    };
                }
            }
            Some(Err(e)) => {
                self.pending_save = None;
                self.phase = RtPhase::Failed(e);
            }
            None => {}
        }
    }

    /// Saves the image to `path` at the chosen [`SaveScale`]: the finished
    /// image as it is for the same size, otherwise the last scene is rendered
    /// again that many times larger in the background and written when done.
    pub fn save_image(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        let factor = self.save_scale.factor();
        if factor <= 1 {
            return self.save_png(path);
        }
        let Some((scene, camera)) = self.job.clone() else {
            return Err(std::io::Error::other("nothing has been rendered yet"));
        };
        self.pending_save = Some(path.to_path_buf());
        self.message = format!("Rendering the {factor}x image...");
        self.start_scaled(scene, camera, factor);
        if self.is_running() {
            Ok(())
        } else {
            self.pending_save = None;
            Err(std::io::Error::other("could not start the renderer"))
        }
    }

    /// Writes the image to `path` as a PNG.
    pub fn save_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        match &self.image {
            Some(img) => std::fs::write(path, plan_render::encode_png(img)),
            None => Err(std::io::Error::other("nothing has been rendered yet")),
        }
    }

    /// Draws the dialog. `source` supplies the scene and camera when
    /// Render is pressed (`None` if there is nothing to render).
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        source: &mut dyn FnMut() -> Option<(Scene, plan_render::Camera)>,
    ) {
        self.poll();
        if !self.open {
            return;
        }
        let mut open = true;
        egui::Window::new("Ray Trace")
            .id(egui::Id::new("ray_trace_dialog"))
            .open(&mut open)
            .collapsible(false)
            .default_width(460.0)
            .show(ctx, |ui| self.contents(ui, ctx, source));
        self.open = open;
        if self.is_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn contents(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        source: &mut dyn FnMut() -> Option<(Scene, plan_render::Camera)>,
    ) {
        let idle = !self.is_running();
        ui.add_enabled_ui(idle, |ui| {
            egui::Grid::new("rt_grid").num_columns(2).show(ui, |ui| {
                ui.label("Image size");
                egui::ComboBox::from_id_salt("rt_size")
                    .selected_text(self.size.label())
                    .show_ui(ui, |ui| {
                        for s in SizePreset::ALL {
                            ui.selectable_value(&mut self.size, s, s.label());
                        }
                    });
                ui.end_row();
                ui.label("Samples per pixel");
                egui::ComboBox::from_id_salt("rt_samples")
                    .selected_text(self.samples.count().to_string())
                    .show_ui(ui, |ui| {
                        for s in SamplesPreset::ALL {
                            ui.selectable_value(&mut self.samples, s, s.count().to_string());
                        }
                    });
                ui.end_row();
                ui.label("Technique");
                egui::ComboBox::from_id_salt("rt_technique")
                    .selected_text(self.technique.label())
                    .show_ui(ui, |ui| {
                        for t in RtTechnique::ALL {
                            ui.selectable_value(&mut self.technique, t, t.label());
                        }
                    });
                ui.end_row();
                ui.label("Sun date");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.date.0)
                            .range(1..=12)
                            .prefix("month "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.date.1)
                            .range(1..=31)
                            .prefix("day "),
                    );
                });
                ui.end_row();
                ui.label("Sun time");
                ui.add(
                    egui::Slider::new(&mut self.time_hours, 0.0..=24.0)
                        .suffix(" h")
                        .fixed_decimals(1),
                );
                ui.end_row();
                ui.label("Latitude");
                ui.add(
                    egui::DragValue::new(&mut self.latitude)
                        .range(-66.0..=66.0)
                        .suffix("\u{B0}"),
                );
                ui.end_row();
                ui.label("Sky");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("rt_sky")
                        .selected_text(self.sky.label())
                        .show_ui(ui, |ui| {
                            for s in RtSky::ALL {
                                ui.selectable_value(&mut self.sky, s, s.label());
                            }
                        });
                    if self.sky == RtSky::Analytic {
                        ui.add(
                            egui::Slider::new(&mut self.turbidity, 2.0..=10.0)
                                .text("haze")
                                .fixed_decimals(1),
                        );
                    }
                });
                ui.end_row();
                ui.label("Exposure");
                ui.add(
                    egui::Slider::new(&mut self.exposure_ev, -3.0..=3.0)
                        .suffix(" EV")
                        .fixed_decimals(1),
                );
                ui.end_row();
                ui.label("Depth of field");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.aperture_in)
                            .range(0.0..=24.0)
                            .speed(0.1)
                            .suffix("\" lens"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.focus_in)
                            .range(0.0..=20000.0)
                            .speed(2.0)
                            .custom_formatter(|v, _| {
                                if v <= 0.0 {
                                    "auto focus".to_string()
                                } else {
                                    format!("focus {v:.0}\"")
                                }
                            }),
                    );
                });
                ui.end_row();
                ui.label("Noise");
                ui.checkbox(&mut self.denoise, "Denoise (keeps edges and textures)");
                ui.end_row();
            });
        });
        let sun = self.sun();
        ui.weak(format!(
            "Sun: azimuth {:.0}\u{B0}, altitude {:.0}\u{B0}",
            sun.azimuth_deg, sun.altitude_deg
        ));
        ui.separator();
        ui.horizontal(|ui| {
            if self.is_running() {
                if ui.button("Cancel").clicked() {
                    self.cancel();
                }
            } else if ui.button("Render").clicked() {
                match source() {
                    Some((scene, camera)) => self.start(scene, camera),
                    None => self.message = "There is nothing to render".into(),
                }
            }
            let can_save = self.image().is_some() && !self.is_running();
            egui::ComboBox::from_id_salt("rt_save_scale")
                .selected_text(self.save_scale.label())
                .show_ui(ui, |ui| {
                    for s in SaveScale::ALL {
                        ui.selectable_value(&mut self.save_scale, s, s.label());
                    }
                });
            if ui
                .add_enabled(can_save, egui::Button::new("Save Image\u{2026}"))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG image", &["png"])
                    .set_file_name("render.png")
                    .save_file()
                {
                    self.message = match self.save_image(&path) {
                        Ok(()) if self.save_scale == SaveScale::X1 => {
                            format!("Saved {}", path.display())
                        }
                        Ok(()) => self.message.clone(),
                        Err(e) => format!("Could not save: {e}"),
                    };
                }
            }
        });
        let (done, total) = self.progress();
        match self.phase() {
            RtPhase::Idle => {}
            RtPhase::Running => {
                let frac = if total == 0 {
                    0.0
                } else {
                    done as f32 / total as f32
                };
                ui.add(egui::ProgressBar::new(frac).text(format!("{done} / {total} samples")));
            }
            RtPhase::Done => {
                ui.label(format!("Finished: {total} samples"));
            }
            RtPhase::Cancelled => {
                ui.label(format!("Stopped after {done} of {total} samples"));
            }
            RtPhase::Failed(e) => {
                ui.colored_label(Color32::from_rgb(0xE0, 0x4B, 0x4B), e);
            }
        }
        if !self.message.is_empty() {
            ui.weak(&self.message);
        }
        self.show_preview(ui, ctx);
    }

    fn show_preview(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(p) = &self.preview else { return };
        if self.preview_dirty || self.texture.is_none() {
            let img = egui::ColorImage::from_rgba_unmultiplied(
                [p.width as usize, p.height as usize],
                &p.rgba,
            );
            match &mut self.texture {
                Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                None => {
                    self.texture =
                        Some(ctx.load_texture("ray_trace", img, egui::TextureOptions::LINEAR));
                }
            }
            self.preview_dirty = false;
        }
        if let Some(t) = &self.texture {
            let w = ui.available_width().clamp(160.0, 640.0);
            let h = w * p.height as f32 / p.width.max(1) as f32;
            ui.image((t.id(), egui::vec2(w, h)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_3d::{Material, Mesh, Vertex};

    #[test]
    fn the_dialog_edits_a_sections_cut_line_not_its_fov() {
        // An old file: length in `fov_deg`, no typed section.
        let old = CameraObject {
            fov_deg: 200.0,
            ..CameraObject::new(
                CameraKind::CrossSection { back_clip: None },
                Point::new(100.0, 0.0),
                90.0,
                "Section 1",
                0,
            )
        };
        let mut d = CameraDialog::new(&old, "1st Floor", CameraExtras::default());
        let s = d.draft().section.expect("upgraded on open");
        assert!((s.a.dist(s.b) - 200.0).abs() < 1e-9);
        assert_eq!(d.draft().fov_deg, plan_core::camera::DEFAULT_FOV_DEG);
        // Editing the length, centre and back clip rebuilds the typed line.
        d.section_len = 100.0;
        d.draft.position = Point::new(150.0, 10.0);
        d.draft.kind = CameraKind::CrossSection {
            back_clip: Some(60.0),
        };
        d.sync_section();
        let s = d.draft().section.unwrap();
        assert!(s.a.dist(Point::new(100.0, 10.0)) < 1e-9);
        assert!(s.b.dist(Point::new(200.0, 10.0)) < 1e-9);
        assert_eq!(s.back_clip, Some(60.0));
    }

    fn house() -> Project {
        let mut p = Project::new("House");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        let mut front = 0;
        for i in 0..4 {
            let id = p.add_wall(
                0,
                c[i],
                c[(i + 1) % 4],
                6.5,
                109.125,
                plan_core::WallKind::Exterior,
            );
            if i == 0 {
                front = id;
            }
        }
        // Glass hatches in the elevation.
        p.add_opening(0, front, 120.0, plan_core::OpeningKind::Window)
            .unwrap();
        p
    }

    fn section_camera() -> CameraObject {
        let mut c = CameraObject::new(
            CameraKind::CrossSection {
                back_clip: Some(80.0),
            },
            Point::new(120.0, 96.0),
            90.0,
            "Section 1",
            0,
        );
        crate::tools::camera::upgrade_section(&mut c);
        c
    }

    #[test]
    fn rendering_toggles_persist_on_the_camera_and_reach_the_options() {
        let cam = section_camera();
        let mut d = CameraDialog::new(&cam, "1st Floor", CameraExtras::default());
        let o = elevation_options(d.draft());
        assert!(!o.hatch && o.shadows.is_none() && !o.depth_weights);
        assert_eq!(o.section_depth, Some(80.0));
        assert!(d.draft().render.labels, "labels are on by default");

        d.draft.render.hatch = true;
        d.draft.render.shadows = true;
        d.draft.render.depth_weights = true;
        d.draft.render.labels = false;
        d.set_sun_from_date();
        let (az, alt) = (
            d.draft.render.sun_azimuth_deg,
            d.draft.render.sun_altitude_deg,
        );
        assert!(alt > 10.0 && alt < 90.0, "afternoon sun: {alt}");
        assert!((az - 135.0).abs() > 1.0, "the sun moved: {az}");
        d.draft.kind = CameraKind::CrossSection {
            back_clip: Some(60.0),
        };
        d.sync_section();
        assert!(d.error().is_none());

        let o = elevation_options(d.draft());
        assert!(o.hatch && o.depth_weights);
        assert_eq!(o.section_depth, Some(60.0));
        assert_eq!(
            o.shadows,
            Some(SunDir {
                azimuth_deg: az,
                altitude_deg: alt
            })
        );

        // The draft is what the 3D panel stores back; it round-trips.
        let json = serde_json::to_string(d.draft()).unwrap();
        let back: CameraObject = serde_json::from_str(&json).unwrap();
        assert_eq!(&back, d.draft());
        let reopened = CameraDialog::new(&back, "1st Floor", CameraExtras::default());
        assert!(reopened.draft().render.hatch && reopened.draft().render.shadows);
        assert!(!reopened.draft().render.labels);

        // A sun below the horizon blocks OK while shadows are on.
        d.draft.render.sun_altitude_deg = 120.0;
        assert!(d.error().is_some());

        // The dialog draws its Rendering tab for sections only.
        let ctx = egui::Context::default();
        let mut sec = CameraDialog::new(&cam, "1st Floor", CameraExtras::default());
        let mut plain = CameraDialog::new(
            &CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "C", 0),
            "1st Floor",
            CameraExtras::default(),
        );
        for dialog in [&mut sec, &mut plain] {
            dialog.draft.render.shadows = true;
            for tab in 0..3 {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| dialog.page(ui, tab));
                });
            }
        }
        assert!(!is_elevation_camera(plain.draft()));
    }

    #[test]
    fn the_options_change_what_the_camera_renders() {
        let p = house();
        let mut cam = CameraObject::new(
            CameraKind::WallElevation,
            Point::new(120.0, -300.0),
            90.0,
            "Front",
            0,
        );
        assert_eq!(elevation_view_dir(&cam), ViewDir::Front);
        let fast = |c: &CameraObject| Options {
            raster_px: 256,
            ..elevation_options(c)
        };
        let plain = render_elevation_with(&p, &cam, &fast(&cam));
        assert!(plain.texts.iter().any(|(_, t)| t == "FRONT ELEVATION"));
        assert!(!plain
            .lines
            .iter()
            .any(|l| l.kind == plan_elevation::EdgeKind::Hatch));

        cam.render.hatch = true;
        cam.render.labels = false;
        let hatched = render_elevation_with(&p, &cam, &fast(&cam));
        assert!(hatched
            .lines
            .iter()
            .any(|l| l.kind == plan_elevation::EdgeKind::Hatch));
        assert!(hatched.texts.is_empty(), "labels off");

        // Facing the other way picks another side.
        cam.direction_deg = 270.0;
        assert_eq!(elevation_view_dir(&cam), ViewDir::Back);
        cam.direction_deg = 0.0;
        assert_eq!(elevation_view_dir(&cam), ViewDir::Left);
        cam.direction_deg = 180.0;
        assert_eq!(elevation_view_dir(&cam), ViewDir::Right);

        // A section is cut at its line and titled with the camera's name.
        let sec = section_camera();
        let cut = section_cut(&sec);
        assert_eq!(cut.plane_normal, ViewDir::Front);
        assert!((cut.offset + 96.0).abs() < 1e-9);
        let d = render_elevation_with(&p, &sec, &fast(&sec));
        assert!(d.cut_regions().count() > 0, "the cut walls are poche");
        assert!(d.texts.iter().any(|(_, t)| t == "SECTION 1"));
    }

    fn tiny_scene() -> Scene {
        let v = |x, y, z| Vertex {
            position: [x, y, z],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        };
        Scene {
            meshes: vec![Mesh {
                vertices: vec![
                    v(-100.0, 0.0, -100.0),
                    v(100.0, 0.0, -100.0),
                    v(100.0, 0.0, 100.0),
                    v(-100.0, 0.0, 100.0),
                ],
                indices: vec![0, 2, 1, 0, 3, 2],
                material: Material::Floor,
                object_id: None,
                color: None,
            }],
        }
    }

    #[test]
    fn presets_build_valid_render_settings() {
        let mut d = RayTraceDialog::default();
        let s = d.settings();
        assert_eq!((s.width, s.height, s.samples), (1280, 960, 64));
        assert_eq!(s.technique, Technique::PhysicallyBased);
        d.size = SizePreset::P1920x1080;
        d.samples = SamplesPreset::S1024;
        d.technique = RtTechnique::Clay;
        let s = d.settings();
        assert_eq!((s.width, s.height, s.samples), (1920, 1080, 1024));
        assert_eq!(s.technique, Technique::Clay);
        for size in SizePreset::ALL {
            for samples in SamplesPreset::ALL {
                d.size = size;
                d.samples = samples;
                let s = d.settings();
                assert!(s.width > 0 && s.height > 0 && s.samples > 0 && s.max_bounces > 0);
            }
        }
    }

    #[test]
    fn sun_follows_the_date_and_time() {
        let mut d = RayTraceDialog::default();
        let noon = {
            d.time_hours = 12.0;
            d.environment().sun.expect("sun is up at noon")
        };
        assert!(noon.direction[1] > 0.5);
        d.time_hours = 0.0;
        assert!(d.environment().sun.is_none(), "no sun at midnight");
        // Summer afternoon: the sun is in the west half of the sky (scene +X is east).
        d.time_hours = 16.0;
        let pm = d.environment().sun.unwrap();
        assert!(pm.direction[0] < 0.0);
    }

    #[test]
    fn render_runs_to_done_on_a_background_thread() {
        let mut d = RayTraceDialog {
            size_override: Some((16, 12)),
            ..RayTraceDialog::default()
        };
        d.samples = SamplesPreset::S64;
        assert_eq!(d.phase(), &RtPhase::Idle);
        let cam = plan_render::Camera::from_plan(Point::new(0.0, -300.0), 90.0, 120.0, 60.0);
        d.start(tiny_scene(), cam);
        assert_eq!(d.phase(), &RtPhase::Running);
        let t0 = std::time::Instant::now();
        while d.is_running() && t0.elapsed().as_secs() < 60 {
            d.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(d.phase(), &RtPhase::Done);
        assert_eq!(d.progress(), (64, 64));
        let img = d.image().expect("image");
        assert_eq!((img.width, img.height), (16, 12));
        let dir = std::env::temp_dir().join(format!("plan_rt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r.png");
        d.save_png(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cancel_stops_early_and_keeps_the_image() {
        let mut d = RayTraceDialog {
            size_override: Some((64, 48)),
            samples: SamplesPreset::S1024,
            ..RayTraceDialog::default()
        };
        let cam = plan_render::Camera::from_plan(Point::new(0.0, -300.0), 90.0, 120.0, 60.0);
        d.start(tiny_scene(), cam);
        d.cancel();
        let t0 = std::time::Instant::now();
        while d.is_running() && t0.elapsed().as_secs() < 60 {
            d.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(d.phase(), &RtPhase::Cancelled);
        assert!(d.progress().0 < 1024);
        assert!(d.image().is_some());
    }

    #[test]
    fn saving_without_an_image_fails() {
        let d = RayTraceDialog::default();
        assert!(d
            .save_png(std::path::Path::new("/nonexistent/x.png"))
            .is_err());
    }

    #[test]
    fn ray_trace_options_reach_the_renderer() {
        let mut d = RayTraceDialog {
            exposure_ev: 1.0,
            denoise: true,
            turbidity: 4.0,
            ..RayTraceDialog::default()
        };
        let s = d.settings();
        assert!(
            (s.exposure - 2.0).abs() < 1e-5,
            "+1 EV doubles the exposure"
        );
        assert!(s.denoise && s.preview_blocks);
        assert_eq!(
            d.environment().sky_model,
            SkyModel::Preetham { turbidity: 4.0 }
        );
        d.sky = RtSky::Gradient;
        assert_eq!(d.environment().sky_model, SkyModel::Gradient);
        d.exposure_ev = 99.0;
        assert!(d.settings().exposure.is_finite() && d.settings().exposure <= 64.0);
    }

    #[test]
    fn depth_of_field_and_area_lights_are_passed_to_the_render() {
        let mut d = RayTraceDialog {
            size_override: Some((16, 12)),
            samples: SamplesPreset::S64,
            aperture_in: 4.0,
            focus_in: 120.0,
            ..RayTraceDialog::default()
        };
        d.areas = vec![AreaLight::ceiling_panel(
            [5.0, 90.0, -5.0],
            24.0,
            24.0,
            [4.0; 3],
        )];
        d.start(
            tiny_scene(),
            plan_render::Camera::from_plan(Point::new(0.0, -300.0), 90.0, 120.0, 60.0),
        );
        let (_, cam) = d.job.clone().expect("the job is kept");
        assert_eq!((cam.aperture, cam.focus_dist), (4.0, 120.0));
        let t0 = std::time::Instant::now();
        while d.is_running() && t0.elapsed().as_secs() < 60 {
            d.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(d.phase(), &RtPhase::Done);
    }

    #[test]
    fn save_image_at_two_times_renders_again_and_writes_the_bigger_png() {
        let mut d = RayTraceDialog {
            size_override: Some((16, 12)),
            samples: SamplesPreset::S64,
            ..RayTraceDialog::default()
        };
        let dir = std::env::temp_dir().join(format!("plan_rt2x_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big.png");
        // Nothing rendered yet: a bigger save has nothing to repeat.
        d.save_scale = SaveScale::X2;
        assert!(d.save_image(&path).is_err());
        d.start(
            tiny_scene(),
            plan_render::Camera::from_plan(Point::new(0.0, -300.0), 90.0, 120.0, 60.0),
        );
        let wait = |d: &mut RayTraceDialog| {
            let t0 = std::time::Instant::now();
            while d.is_running() && t0.elapsed().as_secs() < 60 {
                d.poll();
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        };
        wait(&mut d);
        assert_eq!(d.image().map(|i| (i.width, i.height)), Some((16, 12)));
        d.save_image(&path).expect("the bigger render starts");
        assert!(d.is_running(), "the 2x render runs in the background");
        wait(&mut d);
        assert_eq!(d.phase(), &RtPhase::Done);
        assert_eq!(d.image().map(|i| (i.width, i.height)), Some((32, 24)));
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        assert_eq!((width, height), (32, 24));
        assert!(d.message.starts_with("Saved"), "{}", d.message);
        // The same size just writes what is there.
        d.save_scale = SaveScale::X1;
        let same = dir.join("same.png");
        d.save_image(&same).unwrap();
        assert!(same.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn camera_dialog_validates_name_and_fov() {
        let cam = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "Camera 1", 0);
        let mut d = CameraDialog::new(&cam, "1st Floor", CameraExtras::default());
        assert!(d.error().is_none());
        d.draft.name = "  ".into();
        assert!(d.error().is_some());
        d.draft.name = "A".into();
        d.draft.fov_deg = 3.0;
        assert!(d.error().unwrap().contains("Angle of view"));
        d.draft.fov_deg = 90.0;
        assert!(d.error().is_none());
        assert_eq!(d.id(), cam.id);
    }

    #[test]
    fn a_camera_box_prints_the_elevation_through_the_hook() {
        let mut p = house();
        let id = p.add_camera(section_camera());
        let full = p.add_camera(CameraObject::new(
            CameraKind::FullCamera,
            Point::ZERO,
            0.0,
            "c",
            0,
        ));
        let mut layout = plan_layout::Layout::new("Camera Views", plan_docs::SheetSize::ArchC);
        let box_id = send_camera_to_layout(&mut layout, &p, id, 1).expect("a camera box");
        let b = layout
            .page(1)
            .unwrap()
            .boxes
            .iter()
            .find(|b| b.id == box_id)
            .unwrap();
        assert!(
            matches!(b.source, plan_layout::BoxSource::Camera { camera_id } if camera_id == id)
        );
        assert_eq!(b.label.as_deref(), Some("1 - SECTION 1"));
        let drawing = camera_drawing(&p, id).expect("a section draws");
        assert!(drawing.cut_regions().count() > 0);
        let hooked = layout_context(&p);
        let plain = plan_layout::LayoutRenderContext::new(&p);
        let with = plan_layout::render_box_lines(b, &hooked);
        let without = plan_layout::render_box_lines(b, &plain);
        // Without the hook only the box border (4 edges) is drawn.
        assert_eq!(without.len(), 4);
        assert!(
            with.len() >= drawing.lines.len(),
            "{} box lines for {} elevation lines",
            with.len(),
            drawing.lines.len()
        );
        let pdf = plan_layout::render_pdf(&layout, &hooked);
        assert!(pdf.starts_with(b"%PDF"));
        assert!(pdf.len() > plan_layout::render_pdf(&layout, &plain).len() + 500);
        // Cameras that draw no 2D view are refused.
        assert!(send_camera_to_layout(&mut layout, &p, full, 1).is_none());
        assert!(camera_drawing(&p, full).is_none());
    }

    #[test]
    fn exterior_and_wall_elevations_draw_with_their_names() {
        let p = house();
        let cams = crate::tools::camera::auto_elevation_cameras(&p, 0, false).unwrap();
        let south = cams.iter().find(|c| c.name == "South Elevation").unwrap();
        let opts = Options {
            raster_px: 192,
            ..elevation_options(south)
        };
        let d = render_elevation_with(&p, south, &opts);
        assert!(!d.lines.is_empty());
        assert!(
            d.texts.iter().any(|(_, t)| t == "SOUTH ELEVATION"),
            "{:?}",
            d.texts
        );
        // A wall elevation is a section: only that wall is in the picture.
        let wall = p.floors[0].walls[0].clone();
        let cam =
            crate::tools::camera::wall_elevation(&wall, Point::new(120.0, 30.0), 0, "Kitchen Wall");
        assert!(cuts_model(&cam) && is_elevation_camera(&cam));
        assert_eq!(elevation_view_dir(&cam), ViewDir::Back);
        let o = Options {
            raster_px: 192,
            ..elevation_options(&cam)
        };
        assert_eq!(o.section_depth, Some(wall.thickness + 2.5));
        let wall_d = render_elevation_with(&p, &cam, &o);
        let full_d = render_elevation_with(&p, south, &opts);
        assert!(!wall_d.lines.is_empty());
        assert!(wall_d.total_length() < full_d.total_length() * 1.5);
        assert!(wall_d.texts.iter().any(|(_, t)| t == "KITCHEN WALL"));
    }

    #[test]
    fn the_plan_sun_overrides_the_cameras_own_shadow_setting() {
        let cam = section_camera();
        assert!(elevation_options_with_sun(&cam, None).shadows.is_none());
        let sun = SunDir {
            azimuth_deg: 200.0,
            altitude_deg: 30.0,
        };
        assert_eq!(
            elevation_options_with_sun(&cam, Some(sun)).shadows,
            Some(sun)
        );
        let low = SunSettings::from_date_time_location((6, 21), 3.0, 33.75);
        assert!(sun_dir(&low).is_none(), "no shadows at night");
        let noon = SunSettings::from_date_time_location((6, 21), 12.0, 33.75);
        assert!(sun_dir(&noon).unwrap().altitude_deg > 60.0);
    }

    #[test]
    fn lights_reach_the_render_light_list() {
        use plan_electrical::{place_free, DeviceKind, ElectricalLayer};
        let mut p = house();
        p.floors[0].elevation = 10.0;
        let mut on = PlanLight::new(Point::new(100.0, 50.0), 84.0);
        on.intensity = 2.0;
        on.color = [255, 0, 0];
        let a = p.add_light(0, on).unwrap();
        let mut off = PlanLight::new(Point::new(0.0, 0.0), 84.0);
        off.enabled = false;
        p.add_light(0, off).unwrap();
        let lights = render_lights(&p);
        assert_eq!(lights.len(), 1, "the disabled light is left out");
        let l = lights[0];
        assert_eq!(
            l.position,
            [100.0, 94.0, -50.0],
            "x, elevation + height, -y"
        );
        assert!((l.intensity - 2.0 * LIGHT_UNIT).abs() < 1e-3);
        assert_eq!(l.color, [1.0, 0.0, 0.0]);
        assert!(l.radius < 5.0, "shadow casting lights are small spheres");
        p.update_light(a, |x| x.cast_shadows = false);
        assert!(render_lights(&p)[0].radius > 5.0);

        // Electrical lighting fixtures emit light unless the plan says not to.
        let mut layer = ElectricalLayer::default();
        layer.add(place_free(DeviceKind::CeilingLight, Point::new(60.0, 60.0)));
        layer.add(place_free(DeviceKind::Switch, Point::new(10.0, 10.0)));
        crate::editor::site_view::save_electrical(&mut p, 0, &layer);
        assert_eq!(electrical_lights(&p).len(), 1, "switches do not shine");
        let all = render_lights(&p);
        assert_eq!(all.len(), 2);
        assert!(all
            .iter()
            .any(|l| l.position[0] == 60.0 && l.position[2] == -60.0));
        p.set_light_settings(plan_core::camera::LightSettings {
            use_electrical: false,
        });
        assert_eq!(render_lights(&p).len(), 1);
    }

    #[test]
    fn the_ray_tracer_gets_the_plan_lights_and_the_manual_sun() {
        let mut rt = RayTraceDialog::default();
        let auto = rt.sun();
        rt.manual_sun = Some((270.0, 20.0));
        let s = rt.sun();
        assert_eq!((s.azimuth_deg, s.altitude_deg), (270.0, 20.0));
        assert!(s.intensity > 0.0);
        assert_ne!(auto.azimuth_deg, s.azimuth_deg);
        let e = rt.environment();
        assert!(e.sun.is_some());
        rt.manual_sun = Some((270.0, -5.0));
        assert!(
            rt.environment().sun.is_none(),
            "a sun under the horizon is off"
        );
        // A light in the list is used by the render.
        rt.manual_sun = None;
        rt.size_override = Some((8, 8));
        rt.samples = SamplesPreset::S64;
        rt.lights = vec![PointLight {
            position: [5.0, 40.0, -5.0],
            intensity: 40_000.0,
            color: [1.0, 1.0, 1.0],
            radius: 2.0,
        }];
        rt.start(
            tiny_scene(),
            plan_render::Camera::from_plan(Point::new(5.0, 5.0), 0.0, 20.0, 60.0),
        );
        for _ in 0..600 {
            rt.poll();
            if !rt.is_running() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(rt.phase(), &RtPhase::Done);
    }

    #[test]
    fn adjust_lights_applies_edits_removals_and_the_electrical_switch() {
        let mut p = house();
        let a = p
            .add_light(0, PlanLight::new(Point::new(10.0, 10.0), 84.0))
            .unwrap();
        let b = p
            .add_light(0, PlanLight::new(Point::new(50.0, 10.0), 84.0))
            .unwrap();
        let mut d = AdjustLightsDialog::new(&p);
        assert_eq!(d.draft.len(), 2);
        d.lights_mut()[0].intensity = 3.5;
        d.lights_mut()[0].color = [10, 20, 30];
        d.lights_mut()[0].enabled = false;
        d.lights_mut().remove(1);
        d.set_use_electrical(false);
        // Nothing changes until OK.
        assert_eq!(p.light(a).unwrap().intensity, 1.0);
        d.apply(&mut p);
        let got = p.light(a).unwrap();
        assert_eq!(
            (got.intensity, got.color, got.enabled),
            (3.5, [10, 20, 30], false)
        );
        assert_eq!(
            got.position,
            Point::new(10.0, 10.0),
            "position and floor are kept"
        );
        assert!(p.light(b).is_none());
        assert!(!p.light_settings().use_electrical);
    }

    #[test]
    fn walkthrough_cameras_are_edited_node_by_node() {
        let cam = CameraObject::walkthrough(
            vec![
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(100.0, 80.0),
            ],
            66.0,
            "Walk",
            0,
        );
        let mut d = CameraDialog::new(&cam, "1st Floor", CameraExtras::default());
        assert_eq!(d.kind_label(), "Walkthrough");
        assert!(
            d.error().is_none(),
            "no field of view check on a walkthrough"
        );
        // The eye follows the first node after the path is edited.
        d.draft.path[0] = Point::new(-20.0, 5.0);
        d.sync_walkthrough();
        assert_eq!(d.draft.position, Point::new(-20.0, 5.0));
        assert!(
            (d.draft.direction_deg - (Point::new(120.0, -5.0)).angle().to_degrees()).abs() < 1e-9
        );
        assert_eq!(d.draft.path_nodes.len(), 3);
    }

    /// A wall of `len` at `deg` from the origin on a fresh plan.
    fn skewed(deg: f64, len: f64) -> (Project, Id) {
        let mut p = Project::new("skew");
        let u = Point::new(deg.to_radians().cos(), deg.to_radians().sin());
        let id = p.add_wall(
            0,
            Point::ZERO,
            u * len,
            4.5,
            109.125,
            plan_core::WallKind::Exterior,
        );
        (p, id)
    }

    fn face_bounds(d: &Drawing, id: Id) -> (Point, Point) {
        let (mut lo, mut hi) = (Point::new(1e9, 1e9), Point::new(-1e9, -1e9));
        for r in d
            .regions
            .iter()
            .filter(|r| r.object_id == Some(id) && r.kind == plan_elevation::RegionKind::Face)
        {
            for q in &r.polygon {
                lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
                hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
            }
        }
        (lo, hi)
    }

    #[test]
    fn a_wall_elevation_at_30_degrees_cuts_exactly_along_its_line() {
        let (p, id) = skewed(30.0, 144.0);
        let wall = p.floors[0].walls[0].clone();
        let toward = Point::new(-50.0, 100.0); // the left (north-west) face
        let cam = crate::tools::camera::wall_elevation(&wall, toward, 0, "Skew Wall");
        // The camera line is the wall's own: not snapped to an axis.
        assert!((cam.direction_deg.rem_euclid(360.0) - 300.0).abs() < 1e-6);
        let view = free_view(&cam);
        assert!(view.axis().is_none());
        assert!((view.half_width.unwrap() - 72.0).abs() < 1e-9);
        let o = Options {
            raster_px: 512,
            ..elevation_options(&cam)
        };
        let d = render_elevation_with(&p, &cam, &o);
        let (lo, hi) = face_bounds(&d, id);
        assert!(lo.x.abs() > 1.0, "the face region exists: {lo:?} {hi:?}");
        assert!(
            (lo.x + 72.0).abs() < 1.5 && (hi.x - 72.0).abs() < 1.5,
            "{lo:?} {hi:?}"
        );
        assert!(lo.y.abs() < 1.5 && (hi.y - 109.125).abs() < 1.5);
        // Square to an axis the same wall gives the same face (the old snapping
        // drew a 30 degree wall as a 120 x 72 shortened face).
        let (p0, id0) = skewed(0.0, 144.0);
        let w0 = p0.floors[0].walls[0].clone();
        let cam0 = crate::tools::camera::wall_elevation(&w0, Point::new(70.0, 50.0), 0, "Flat");
        let d0 = render_elevation_with(&p0, &cam0, &o);
        let (l0, h0) = face_bounds(&d0, id0);
        assert!(((h0.x - l0.x) - (hi.x - lo.x)).abs() < 2.0);
    }

    #[test]
    fn a_section_dialog_direction_of_30_degrees_draws_that_section() {
        // The dialog's View Direction field takes any angle and rebuilds the cut line square to it.
        let (p, _) = skewed(60.0, 200.0);
        let mut cam = section_camera();
        cam.position = Point::new(50.0, 87.0);
        let mut d = CameraDialog::new(&cam, "1st Floor", CameraExtras::default());
        d.draft.direction_deg = 150.0;
        d.section_len = 160.0;
        d.sync_section();
        let c = d.draft().clone();
        let (a, b) = crate::tools::camera::section_line(&c);
        assert!((a.dist(b) - 160.0).abs() < 1e-9);
        let along = (b - a).normalized();
        // The view looks to the left of A to B: 90 degrees counter-clockwise from the line.
        assert!((along.perp().angle().to_degrees() - 150.0).abs() < 1e-6);
        let view = free_view(&c);
        assert!((view.view_deg - 150.0).abs() < 1e-6 && view.axis().is_none());
        let o = Options {
            raster_px: 256,
            ..elevation_options(&c)
        };
        let drawn = render_elevation_with(&p, &c, &o);
        // The 60 degree wall crosses the line, so there is poche.
        assert!(drawn.cut_regions().count() > 0);
        // The drawing is cut off at the ends of the line (grade lines and notes aside).
        assert!(drawn
            .regions
            .iter()
            .flat_map(|r| r.polygon.iter())
            .all(|q| q.x.abs() <= 80.0 + 1e-6));
    }

    fn two_storey_house() -> Project {
        let mut p = house();
        p.build_new_floor(true);
        p
    }

    #[test]
    fn vector_options_add_dimensions_material_labels_and_dashed_lines() {
        let p = two_storey_house();
        let cams = crate::tools::camera::auto_elevation_cameras(&p, 0, false).unwrap();
        let mut south = cams
            .into_iter()
            .find(|c| c.name == "South Elevation")
            .unwrap();
        let plain = Options {
            raster_px: 256,
            ..elevation_options(&south)
        };
        let d = render_elevation_with(&p, &south, &plain);
        assert!(d.dims.is_empty());
        assert!(d.texts.iter().any(|(_, t)| t.starts_with("T.O. PLATE")));
        south.vector.dimensions = true;
        south.vector.level_labels = false;
        south.vector.material_labels = true;
        south.vector.hidden_dashed = true;
        let o = Options {
            raster_px: 256,
            ..elevation_options(&south)
        };
        assert!(o.include_hidden_dashed);
        let d = render_elevation_with(&p, &south, &o);
        let f2f = d
            .dims
            .iter()
            .find(|x| x.kind == plan_elevation::DimKind::FloorToFloor)
            .expect("a floor-to-floor dimension");
        let structure = plan_core::floors::FLOOR_PLATFORM_THICKNESS;
        assert!((f2f.value() - (109.125 + structure)).abs() < 1e-9);
        assert!(d
            .dims
            .iter()
            .any(|x| x.kind == plan_elevation::DimKind::Opening));
        assert!(!d.texts.iter().any(|(_, t)| t.starts_with("T.O.")));
        assert!(d.texts.iter().any(|(_, t)| t == "SOUTH ELEVATION"));
        assert!(d.lines.iter().any(|l| l.is_dashed()));
        // The house is clad in the generic wall material, which gets no label;
        // the option itself reaches the annotations.
        assert!(annotate_options(&south).materials);
        // The Labels option off removes the title and callouts but not the dimensions.
        south.render.labels = false;
        let d = render_elevation_with(&p, &south, &o);
        assert!(!d.texts.iter().any(|(_, t)| t == "SOUTH ELEVATION"));
        assert!(!d.dims.is_empty());
    }

    #[test]
    fn layer_pens_become_the_line_weights_of_the_vector_view() {
        let mut p = house();
        let win = p.floors[0].openings[0].id;
        let wall = p.floors[0].walls[0].id;
        let w = layer_weights(&p);
        assert_eq!(w.class_of(wall), Some(plan_elevation::LineWeight::Heavy));
        assert_eq!(w.class_of(win), Some(plan_elevation::LineWeight::Medium));
        // Thin pens draw light.
        p.layers.layers.iter_mut().for_each(|l| {
            if l.name == "Windows" {
                l.line_weight = 13;
            }
        });
        assert_eq!(
            layer_weights(&p).class_of(win),
            Some(plan_elevation::LineWeight::Light)
        );
        let cams = crate::tools::camera::auto_elevation_cameras(&p, 0, false).unwrap();
        let mut south = cams
            .into_iter()
            .find(|c| c.name == "South Elevation")
            .unwrap();
        let o = Options {
            raster_px: 256,
            ..elevation_options(&south)
        };
        let heavy = |d: &Drawing| {
            d.lines
                .iter()
                .filter(|l| l.weight == plan_elevation::LineWeight::Heavy)
                .map(plan_elevation::Line2::length)
                .sum::<f64>()
        };
        let plain = render_elevation_with(&p, &south, &o);
        south.vector.layer_weights = true;
        let styled = render_elevation_with(&p, &south, &o);
        assert!(heavy(&styled) < heavy(&plain));
    }

    #[test]
    fn the_camera_drawing_exports_as_a_dxf_with_layers_by_weight() {
        let p = house();
        let cams = crate::tools::camera::auto_elevation_cameras(&p, 0, false).unwrap();
        let south = cams
            .into_iter()
            .find(|c| c.name == "South Elevation")
            .unwrap();
        let dxf = camera_dxf(&p, &south).expect("a drawing");
        for layer in ["South Elevation, Heavy", "South Elevation, Annotation"] {
            assert!(dxf.contains(layer), "{layer}");
        }
        assert!(dxf.contains("SOUTH ELEVATION"));
        let empty = Project::new("empty");
        assert!(camera_dxf(&empty, &south).is_none());
        // The dialog only raises a request; the host has the project.
        let mut d = CameraDialog::new(&south, "1st Floor", CameraExtras::default());
        assert!(!d.take_export_request());
        d.export_requested = true;
        assert!(d.take_export_request() && !d.take_export_request());
    }

    // ----- Camera Specification tabs, Lighting and Record Walkthrough (round 14) -----

    fn draw_tabs(d: &mut CameraDialog) {
        let ctx = egui::Context::default();
        for tab in 0..TABS.len() {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| d.page(ui, tab));
            });
        }
    }

    #[test]
    fn the_tabs_are_the_manuals_camera_panels() {
        let cam = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "C", 0);
        let d = CameraDialog::new(&cam, "1st Floor", CameraExtras::default());
        let names: Vec<_> = d.tabs().iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            [
                "Camera",
                "Positioning",
                "Below Grade",
                "Selected Defaults",
                "Plan Display",
                "Backdrop",
                "Layer",
                "Label"
            ]
        );
        assert!(d.tabs().iter().all(|t| t.enabled));
    }

    #[test]
    fn every_tab_draws_for_every_kind_of_camera() {
        use plan_core::camera_view::BackdropKind;
        let mut cams = vec![
            CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "full", 0),
            CameraObject::new(CameraKind::FloorCamera, Point::ZERO, 0.0, "floor", 0),
            CameraObject::new(CameraKind::PerspectiveOverview, Point::ZERO, 0.0, "ov", 0),
            CameraObject::new(CameraKind::DollHouse, Point::ZERO, 0.0, "doll", 0),
            CameraObject::new(CameraKind::GlassHouse, Point::ZERO, 0.0, "glass", 0),
            CameraObject::new(CameraKind::FramingOverview, Point::ZERO, 0.0, "fr", 0),
            CameraObject::walkthrough(vec![Point::ZERO, Point::new(100.0, 0.0)], 66.0, "walk", 0),
            section_camera(),
        ];
        for kind in BackdropKind::ALL {
            for c in &mut cams {
                c.view.backdrop.kind = kind;
                c.view.ambient = Some(0.3);
                c.view.sun_intensity = Some(1.0);
                let mut d = CameraDialog::new(c, "1st Floor", CameraExtras::default());
                draw_tabs(&mut d);
                assert!(d.kind_label().len() > 3);
            }
        }
    }

    #[test]
    fn a_cameras_saved_technique_wins_and_the_rendering_tab_edits_it() {
        let mut cam = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "C", 0);
        cam.view.technique = Some("Watercolor".into());
        let d = CameraDialog::new(&cam, "1st Floor", CameraExtras::default());
        assert_eq!(d.extras().technique, RenderingTechnique::Watercolor);
        // A Glass House camera starts as a Glass House.
        let glass = CameraObject::new(CameraKind::GlassHouse, Point::ZERO, 0.0, "G", 0);
        let d = CameraDialog::new(&glass, "1st Floor", CameraExtras::default());
        assert_eq!(d.extras().technique, RenderingTechnique::GlassHouse);
        // With no choice the 3D view's technique is offered.
        let plain = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "P", 0);
        let d = CameraDialog::new(
            &plain,
            "1st Floor",
            CameraExtras {
                technique: RenderingTechnique::Clay,
            },
        );
        assert_eq!(d.extras().technique, RenderingTechnique::Clay);
        assert_eq!(d.draft().view.technique, None);
    }

    #[test]
    fn tilt_backdrop_and_label_are_checked() {
        use plan_core::camera_view::BackdropKind;
        let cam = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "C", 0);
        let mut d = CameraDialog::new(&cam, "1st Floor", CameraExtras::default());
        assert!(d.error().is_none());
        d.draft.view.tilt_deg = 90.0;
        assert!(d.error().unwrap().contains("Tilt"));
        d.draft.view.tilt_deg = -85.0;
        assert!(d.error().is_none());
        d.draft.view.backdrop.kind = BackdropKind::Image;
        assert!(d.error().unwrap().contains("backdrop"));
        d.draft.view.backdrop.image = "Rolling Hills.jpg".into();
        assert!(d.error().is_none());
        // The label and lock edits stay on the draft the host applies.
        d.draft.view.label.text = "Entry".into();
        d.draft.view.locked = true;
        d.draft.view.show_in_plan = false;
        d.draft.view.floors = plan_core::camera_view::FloorsDisplayed::ThisAndBelow;
        draw_tabs(&mut d);
        let v = &d.draft().view;
        assert!(v.locked && !v.show_in_plan);
        assert_eq!(v.label_text("C"), "Entry");
    }

    #[test]
    fn the_overrides_start_from_the_plans_lighting() {
        let cam = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "C", 0);
        let plan = plan_core::camera_view::Lighting {
            ambient: 0.7,
            sun_intensity: 1.5,
            ..plan_core::camera_view::Lighting::default()
        };
        let d = CameraDialog::new(&cam, "1st Floor", CameraExtras::default())
            .with_plan_lighting(plan.clone());
        assert_eq!(d.plan_lighting, plan);
        assert_eq!(
            (d.draft().view.ambient, d.draft().view.sun_intensity),
            (None, None)
        );
    }

    #[test]
    fn the_lighting_dialog_sets_the_sun_the_interior_lights_and_each_light() {
        let mut p = house();
        let a = p
            .add_light(0, PlanLight::new(Point::new(40.0, 40.0), 84.0))
            .unwrap();
        let b = p
            .add_light(0, PlanLight::new(Point::new(80.0, 40.0), 84.0))
            .unwrap();
        let mut d = LightingDialog::new(&p);
        let ids: Vec<Id> = d.lights_mut().iter().map(|l| l.id).collect();
        assert_eq!(ids, [a, b]);
        // Noon on the summer solstice at 33.75 degrees north: high and south.
        d.date = SunInput {
            month: 6,
            day: 21,
            time_hours: 12.0,
            latitude: 33.75,
        };
        d.set_sun_from_date();
        assert!(d.draft_mut().sun_altitude_deg > 70.0, "{:?}", d.draft_mut());
        assert!((d.draft_mut().sun_azimuth_deg - 180.0).abs() < 10.0);
        assert_eq!(
            d.draft_mut().from_date.map(|x| (x.month, x.day)),
            Some((6, 21))
        );
        d.draft_mut().interior_lights = false;
        d.draft_mut().ambient = 0.3;
        d.lights_mut()[0].enabled = false;
        d.lights_mut()[1].intensity = 2.5;
        // Nothing changes until OK.
        assert!(p.lighting.interior_lights);
        d.apply(&mut p);
        assert!(!p.lighting.interior_lights);
        assert_eq!(p.lighting.ambient, 0.3);
        assert!(!p.light(a).unwrap().enabled);
        assert_eq!(p.light(b).unwrap().intensity, 2.5);
        // With the interior lights off nothing reaches the ray tracer or the
        // viewport, and the plan keeps its lights for when they come back on.
        assert!(all_lights(&p).is_empty() && render_lights(&p).is_empty());
        assert_eq!(p.lights().len(), 2);
        p.lighting.interior_lights = true;
        assert_eq!(
            render_lights(&p).len(),
            1,
            "the switched-off light stays off"
        );
        // The dialog draws, and asks for Adjust Lights on request.
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                let _ = d.show(ctx);
            });
        }
        assert!(!d.take_adjust_request());
        d.adjust_requested = true;
        assert!(d.take_adjust_request() && !d.take_adjust_request());
    }

    #[test]
    fn the_record_dialog_counts_frames_and_needs_a_folder() {
        let mut cam =
            CameraObject::walkthrough(vec![Point::ZERO, Point::new(360.0, 0.0)], 66.0, "Walk", 0);
        cam.id = 7;
        cam.walk_speed = 36.0; // ten seconds
        let mut d = RecordDialog::new(&cam, String::new());
        assert_eq!(d.camera, 7);
        assert_eq!(d.settings, plan_core::camera_view::WalkRecord::default());
        assert_eq!(d.frame_count(), 120, "10 s at the default 12 fps");
        d.settings.fps = 24.0;
        assert_eq!(d.frame_count(), 240);
        d.settings.fps = 1000.0;
        assert_eq!(d.frame_count(), 600, "the rate is limited to 60 fps");
        assert!(!d.ready());
        d.folder = "  ".into();
        assert!(!d.ready());
        d.folder = "/tmp/frames".into();
        assert!(d.ready());
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(d.show(ctx), RecordOutcome::Open);
        });
    }

    #[test]
    fn the_walkthrough_tab_edits_tilt_hold_and_the_recording_rate() {
        let mut cam = CameraObject::walkthrough(
            vec![
                Point::ZERO,
                Point::new(100.0, 0.0),
                Point::new(100.0, 100.0),
            ],
            66.0,
            "Walk",
            0,
        );
        cam.path_nodes[1].tilt_deg = 10.0;
        cam.path_nodes[1].hold_s = 2.0;
        cam.view.walk.fps = 24.0;
        let mut d = CameraDialog::new(&cam, "1st Floor", CameraExtras::default());
        draw_tabs(&mut d);
        assert_eq!(d.draft().path_nodes[1].tilt_deg, 10.0);
        assert_eq!(d.draft().view.walk.fps, 24.0);
        assert!((d.draft().walk_duration_s() - (200.0 / 36.0 + 2.0)).abs() < 1e-9);
        assert!(d.error().is_none());
    }

    #[test]
    fn a_light_set_lists_plan_lights_and_fixtures_apart() {
        let mut p = Project::new("Lights");
        let a = p
            .add_light(0, PlanLight::new(Point::new(10.0, 10.0), 84.0))
            .unwrap();
        // The id names a fixture, so the plan light with that id stays off.
        p.lighting.add_set("Fixtures only", vec![], vec![a]);
        assert!(shining_lights(&p, Some("Fixtures only")).is_empty());
        p.lighting.add_set("Plain", vec![a], vec![]);
        assert_eq!(shining_lights(&p, Some("Plain")).len(), 1);
        // Without a set each light follows its own switch; a set overrides it.
        p.update_light(a, |l| l.enabled = false);
        assert!(shining_lights(&p, None).is_empty());
        assert_eq!(shining_lights(&p, Some("Plain")).len(), 1);
        // The plan's active set applies to every view that names none.
        p.lighting.active_set = Some("Plain".into());
        assert_eq!(render_lights(&p).len(), 1);
        assert!(render_lights_in(&p, Some("Fixtures only")).is_empty());
        // A camera whose set was deleted uses the plan's.
        assert_eq!(shining_lights(&p, Some("Gone")).len(), 1);
        // Interior lights off silences everything.
        p.lighting.interior_lights = false;
        assert!(shining_lights(&p, Some("Plain")).is_empty());
    }

    #[test]
    fn adjust_lights_makes_renames_uses_and_deletes_sets_as_a_draft() {
        let mut p = Project::new("Sets");
        let a = p
            .add_light(0, PlanLight::new(Point::new(10.0, 10.0), 84.0))
            .unwrap();
        let b = p
            .add_light(0, PlanLight::new(Point::new(50.0, 50.0), 84.0))
            .unwrap();
        p.update_light(b, |l| l.enabled = false);
        let mut d = AdjustLightsDialog::new(&p);
        // A new set starts from what is on now: only the light a.
        assert!(d.add_set("Day"));
        assert_eq!(d.sets()[0].on, vec![a]);
        assert!(!d.add_set("DAY") && !d.add_set(" "));
        d.sets_mut()[0].on.push(b);
        assert!(d.add_set("Night"));
        // The set in use is what is on now for the next set.
        d.use_set(Some(0));
        assert!(d.add_set("Copy of day"));
        assert_eq!(d.sets()[2].on.len(), 2);
        // Deleting the set in use clears it; the others keep their place.
        d.delete_set(0);
        assert_eq!(
            d.sets().iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["Night", "Copy of day"]
        );
        d.use_set(Some(1));
        d.sets_mut()[1].name = "  Dinner ".into();
        d.apply(&mut p);
        assert_eq!(
            p.lighting.active_set.as_deref(),
            Some("Dinner"),
            "a rename follows the set in use"
        );
        assert_eq!(p.lighting.sets.len(), 2);
        // A repeated or empty name is made unique or dropped on OK.
        let mut d = AdjustLightsDialog::new(&p);
        d.sets_mut()[0].name = "dinner".into();
        d.sets_mut()[1].name = String::new();
        d.apply(&mut p);
        assert_eq!(p.lighting.sets.len(), 1);
        assert_eq!(p.lighting.sets[0].name, "dinner");
        assert_eq!(
            p.lighting.active_set, None,
            "the set in use was the nameless one"
        );
    }

    #[test]
    fn the_lighting_dialog_picks_the_set_in_use_but_keeps_the_sets() {
        let mut p = Project::new("Sets");
        p.lighting.add_set("A", vec![], vec![]);
        p.lighting.add_set("B", vec![], vec![]);
        let mut d = LightingDialog::new(&p);
        d.draft_mut().active_set = Some("B".into());
        d.draft_mut().sets.clear(); // the draft's copy of the sets is not stored
        d.apply(&mut p);
        assert_eq!(p.lighting.sets.len(), 2);
        assert_eq!(p.lighting.active_set.as_deref(), Some("B"));
        let mut d = LightingDialog::new(&p);
        d.draft_mut().active_set = Some("Gone".into());
        d.apply(&mut p);
        assert_eq!(p.lighting.active_set, None);
    }

    #[test]
    fn the_record_dialog_offers_the_format_and_defaults_to_a_movie() {
        let cam =
            CameraObject::walkthrough(vec![Point::ZERO, Point::new(120.0, 0.0)], 66.0, "Walk", 0);
        let mut d = RecordDialog::new(&cam, "/tmp/x".into());
        assert_eq!(
            d.settings.format,
            plan_core::camera_view::RecordFormat::Video
        );
        d.settings.format = plan_core::camera_view::RecordFormat::Both;
        d.settings.quality = 200;
        let w = d.clamped();
        assert_eq!(
            (w.format, w.quality),
            (plan_core::camera_view::RecordFormat::Both, 100)
        );
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(d.show(ctx), RecordOutcome::Open);
        });
    }

    #[test]
    fn the_backdrop_and_camera_tabs_draw_ground_fog_and_the_floor_pick() {
        let mut cam = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "C", 1);
        cam.view.backdrop.fog.on = true;
        cam.view.backdrop.ground = plan_core::camera_view::GroundKind::Color;
        cam.view.floors = plan_core::camera_view::FloorsDisplayed::Picked { from: 0, to: 1 };
        let mut lighting = plan_core::camera_view::Lighting::default();
        lighting.add_set("Night", vec![], vec![]);
        let mut d = CameraDialog::new(&cam, "2nd Floor", CameraExtras::default())
            .with_plan_lighting(lighting)
            .with_floor_names(vec!["1st Floor".into(), "2nd Floor".into()]);
        draw_tabs(&mut d);
        assert_eq!(
            d.draft().view.floors,
            plan_core::camera_view::FloorsDisplayed::Picked { from: 0, to: 1 }
        );
        assert!(d.draft().view.backdrop.fog.on);
        assert!(d.error().is_none());
    }
}
