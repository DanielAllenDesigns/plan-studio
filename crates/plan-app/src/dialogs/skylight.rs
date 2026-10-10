//! Skylights: the Skylight Specification, shapes, the inside hole rim and the
//! ceiling hole (manual pp. 870 to 873; RF-43, RF-85..RF-87, DW-145).
//!
//! A skylight is a hole of a roof plane with a curb and glass
//! (`roof_view::HoleRecord`, `plan_roof::SkylightSpec`). What the roof model
//! cannot hold, the shape, its size, the Inside Hole Rim, the Ceiling Hole
//! and Display in Plan View, is kept in a record of `Floor.roofs`,
//! `{"kind": "skylight_options", "plane", "at", ...}`, found again by the
//! plane and the centroid of the hole (`roof_view` passes the record
//! through; a record whose skylight moved is ignored).
//!
//! * [`place`] makes a skylight: a drag is a rectangle, a click is 2 by 2 feet
//!   ([`plan_roof::DEFAULT_SKYLIGHT_SIZE`]).
//! * The **Skylight Specification** ([`open_spec`]) has the panels General
//!   (shape, width and length, frame width and height, Display in Plan
//!   View, Edit Skylight Shape), Inside Hole Rim (Square, Plumb,
//!   Plumb/Square) and Ceiling Hole (Automatically Generate, Use Manual
//!   Polyline, Do Not Cut). OK is one undo step.
//! * [`move_corner`] is Edit Skylight Shape on the plan: moving a corner
//!   makes the shape Custom.
//! * [`ceiling_holes`] lists the holes the ceiling under the skylights
//!   needs, and [`schedule_rows`] the rows of the skylight schedule.

// The entry points are called from the roof, menu and edit-toolbar owners'
// files once the hooks in docs/integration-queue.md are in; until then
// only the scenario tests use them.
#![allow(dead_code)]

use super::{on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use super::{PV_ACCENT, PV_FAINT, PV_INK};
use crate::editor::roof_view::{self, HoleRecord};
use crate::editor::EditorContext;
use eframe::egui::{self, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::geometry::{polygon_centroid, Point};
use plan_core::{Floor, Id, Project};
use plan_roof::{
    ceiling_hole_outline, move_shape_corner, shape_outline, CeilingHole, HoleRim, SkylightOptions,
    SkylightShape, SkylightSpec, DEFAULT_SKYLIGHT_SIZE, SKYLIGHT_FACETS,
};
use serde_json::{json, Value};
use std::cell::RefCell;

const KIND: &str = "skylight_options";
/// Records and holes whose centroids are this close are the same skylight,
/// inches.
const SAME_AT: f64 = 1.0;

const TABS: &[Tab] = &[on("General"), on("Inside Hole Rim"), on("Ceiling Hole")];

/// Which skylight: the hole `hole` of roof plane `plane`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkylightRef {
    pub plane: Id,
    pub hole: usize,
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

fn is_record(v: &Value) -> bool {
    v.get("kind").and_then(Value::as_str) == Some(KIND)
}

fn record_at(v: &Value) -> Option<(Id, Point)> {
    let plane = v.get("plane")?.as_u64()?;
    let at: Point = serde_json::from_value(v.get("at")?.clone()).ok()?;
    Some((plane, at))
}

/// The skylight's frame in the plan: `across` is along the eave of its roof
/// plane, the length direction its perpendicular.
fn across_of(floor: &Floor, plane: Id) -> Point {
    roof_view::load(floor)
        .plane(plane)
        .map(|p| p.baseline.1.sub(p.baseline.0))
        .filter(|d| d.length() > 1e-9)
        .map_or(Point::new(1.0, 0.0), |d| d.normalized())
}

/// The size of `outline` along and across `across`.
fn extents(outline: &[Point], across: Point) -> (f64, f64) {
    let up = across.perp();
    let range = |dir: Point| {
        let (lo, hi) = outline.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p.dot(dir)), hi.max(p.dot(dir)))
        });
        (hi - lo).max(0.0)
    };
    (range(across), range(up))
}

/// The skylights of `floor`: every skylight hole of every roof plane.
pub fn skylights(floor: &Floor) -> Vec<(SkylightRef, HoleRecord)> {
    let set = roof_view::load(floor);
    set.planes
        .iter()
        .flat_map(|p| {
            p.holes
                .iter()
                .enumerate()
                .filter(|(_, h)| h.is_skylight())
                .map(|(i, h)| {
                    (
                        SkylightRef {
                            plane: p.id,
                            hole: i,
                        },
                        h.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn hole_of(floor: &Floor, r: SkylightRef) -> Option<HoleRecord> {
    roof_view::load(floor)
        .plane(r.plane)?
        .holes
        .get(r.hole)
        .filter(|h| h.is_skylight())
        .cloned()
}

/// The options of skylight `r`: the stored ones, or Rectangle with the size
/// of its outline.
pub fn options_of(floor: &Floor, r: SkylightRef) -> Option<SkylightOptions> {
    let hole = hole_of(floor, r)?;
    let at = polygon_centroid(&hole.outline);
    let stored = floor.roofs.iter().filter(|v| is_record(v)).find_map(|v| {
        let (plane, p) = record_at(v)?;
        (plane == r.plane && p.dist(at) < SAME_AT)
            .then(|| serde_json::from_value::<SkylightOptions>(v.clone()).ok())
            .flatten()
    });
    Some(stored.unwrap_or_else(|| {
        let (w, l) = extents(&hole.outline, across_of(floor, r.plane));
        SkylightOptions {
            width: w,
            length: l,
            shape: if hole.outline.len() == 4 {
                SkylightShape::Rectangle
            } else {
                SkylightShape::Custom
            },
            ..SkylightOptions::default()
        }
    }))
}

fn put_options(floor: &mut Floor, plane: Id, at: Point, opts: &SkylightOptions) {
    floor.roofs.retain(|v| {
        !(is_record(v) && record_at(v).is_some_and(|(p, q)| p == plane && q.dist(at) < SAME_AT))
    });
    if let Ok(Value::Object(mut m)) = serde_json::to_value(opts) {
        m.insert("kind".into(), json!(KIND));
        m.insert("plane".into(), json!(plane));
        m.insert("at".into(), json!(at));
        floor.roofs.push(Value::Object(m));
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// The Skylight tool: a drag from `a` to `b` is a rectangle, a click (the
/// points close together) is 2 by 2 feet centred on `a`, with its sides along
/// and across the slope. One undo step. Returns the skylight made.
pub fn place(cx: &mut EditorContext, a: Point, b: Point) -> Result<SkylightRef, String> {
    let fl = cx.floor;
    let click = a.dist(b) < 4.0;
    let outline = if click {
        let across = {
            let set = roof_view::load(cx.floor());
            set.plane_at(a)
                .map(|id| across_of(cx.floor(), id))
                .unwrap_or(Point::new(1.0, 0.0))
        };
        shape_outline(
            SkylightShape::Rectangle,
            a,
            across,
            DEFAULT_SKYLIGHT_SIZE,
            DEFAULT_SKYLIGHT_SIZE,
            SKYLIGHT_FACETS,
        )
    } else {
        roof_view::rect_polygon(a, b)
    };
    cx.begin_change("Place Skylight");
    match roof_view::add_hole_polygon(&mut cx.project, fl, outline, true) {
        Ok(plane) => {
            cx.mark_dirty();
            let hole = roof_view::load(cx.floor())
                .plane(plane)
                .map_or(0, |p| p.holes.len().saturating_sub(1));
            Ok(SkylightRef { plane, hole })
        }
        Err(e) => {
            cx.cancel_change();
            cx.status = e.clone();
            Err(e)
        }
    }
}

/// Skylight Specification OK: sets the frame (`spec`) and the options of
/// skylight `r`, rebuilding the outline from the shape and size unless the
/// shape is Custom. One undo step; false when the skylight is gone or the new
/// outline would not lie inside its roof plane.
pub fn apply(
    cx: &mut EditorContext,
    r: SkylightRef,
    spec: SkylightSpec,
    opts: SkylightOptions,
) -> bool {
    let fl = cx.floor;
    let mut set = roof_view::load(cx.floor());
    let across = across_of(cx.floor(), r.plane);
    let Some(plane) = set.plane_mut(r.plane) else {
        return false;
    };
    let Some(old) = plane.holes.get(r.hole).filter(|h| h.is_skylight()).cloned() else {
        return false;
    };
    let centre = polygon_centroid(&old.outline);
    let outline = if opts.shape == SkylightShape::Custom {
        old.outline.clone()
    } else {
        shape_outline(
            opts.shape,
            centre,
            across,
            opts.width,
            opts.length,
            SKYLIGHT_FACETS,
        )
    };
    if outline.len() < 3 || roof_view::polygon_self_intersects(&outline) {
        return false;
    }
    let others: Vec<HoleRecord> = plane
        .holes
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != r.hole)
        .map(|(_, h)| h.clone())
        .collect();
    if !plane.encloses(&outline)
        || others.iter().any(|h| {
            h.outline
                .iter()
                .any(|p| plan_core::geometry::point_in_polygon(*p, &outline))
        })
    {
        cx.status = "The skylight must lie completely inside one roof plane".into();
        return false;
    }
    plane.holes[r.hole] = HoleRecord {
        outline: outline.clone(),
        skylight: Some(spec),
    };
    cx.begin_change("Skylight Specification");
    roof_view::store(&mut cx.project, fl, &mut set);
    let at = polygon_centroid(&outline);
    // The record of the skylight as it was goes with the new one.
    cx.project.floors[fl].roofs.retain(|v| {
        !(is_record(v)
            && record_at(v).is_some_and(|(p, q)| p == r.plane && q.dist(centre) < SAME_AT))
    });
    put_options(&mut cx.project.floors[fl], r.plane, at, &opts);
    cx.mark_dirty();
    cx.refresh();
    true
}

/// Edit Skylight Shape on the plan: corner `index` of skylight `r` moves to
/// `to` and the shape becomes Custom. One undo step.
pub fn move_corner(cx: &mut EditorContext, r: SkylightRef, index: usize, to: Point) -> bool {
    let fl = cx.floor;
    let Some(hole) = hole_of(cx.floor(), r) else {
        return false;
    };
    let Some(mut opts) = options_of(cx.floor(), r) else {
        return false;
    };
    let mut outline = hole.outline.clone();
    let mut shape = opts.shape;
    if !move_shape_corner(&mut outline, &mut shape, index, to) {
        return false;
    }
    let mut set = roof_view::load(cx.floor());
    let Some(plane) = set.plane_mut(r.plane) else {
        return false;
    };
    if !plane.encloses(&outline) {
        cx.status = "The skylight must lie completely inside one roof plane".into();
        return false;
    }
    opts.shape = shape;
    let (w, l) = extents(&outline, across_of(cx.floor(), r.plane));
    opts.width = w;
    opts.length = l;
    plane.holes[r.hole].outline = outline.clone();
    let old_at = polygon_centroid(&hole.outline);
    cx.begin_change("Edit Skylight Shape");
    roof_view::store(&mut cx.project, fl, &mut set);
    cx.project.floors[fl].roofs.retain(|v| {
        !(is_record(v)
            && record_at(v).is_some_and(|(p, q)| p == r.plane && q.dist(old_at) < SAME_AT))
    });
    put_options(
        &mut cx.project.floors[fl],
        r.plane,
        polygon_centroid(&outline),
        &opts,
    );
    cx.mark_dirty();
    cx.refresh();
    true
}

/// The holes the ceiling needs under the skylights of floor `fi`, as plan
/// outlines, for the ceiling at `ceiling_y` (inches above the floor datum).
/// `manual` gives the polyline of a Use Manual Polyline hole by skylight.
pub fn ceiling_holes(
    project: &Project,
    fi: usize,
    ceiling_y: f64,
    manual: &dyn Fn(SkylightRef) -> Option<Vec<Point>>,
) -> Vec<Vec<Point>> {
    let floor = &project.floors[fi];
    let set = roof_view::load(floor);
    skylights(floor)
        .into_iter()
        .filter_map(|(r, h)| {
            let opts = options_of(floor, r)?;
            let plane = set.plane(r.plane)?.to_roof_plane(0);
            let m = manual(r);
            ceiling_hole_outline(
                &plane,
                &h.outline,
                opts.rim,
                opts.ceiling_hole,
                ceiling_y,
                m.as_deref(),
            )
        })
        .collect()
}

/// One row of the skylight schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct SkylightRow {
    pub plane: Id,
    pub shape: SkylightShape,
    /// Width across the slope and length along it, inches.
    pub width: f64,
    pub length: f64,
    pub frame_width: f64,
    pub frame_height: f64,
    pub rim: HoleRim,
    pub ceiling_hole: CeilingHole,
    pub display_in_plan: bool,
}

/// The skylights of `floor` as schedule rows.
pub fn schedule_rows(floor: &Floor) -> Vec<SkylightRow> {
    skylights(floor)
        .into_iter()
        .filter_map(|(r, h)| {
            let o = options_of(floor, r)?;
            let spec = h.skylight?;
            Some(SkylightRow {
                plane: r.plane,
                shape: o.shape,
                width: o.width,
                length: o.length,
                frame_width: spec.frame_width,
                frame_height: spec.curb_height,
                rim: o.rim,
                ceiling_hole: o.ceiling_hole,
                display_in_plan: o.display_in_plan,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The dialog
// ---------------------------------------------------------------------------

/// The Skylight Specification.
pub struct SkylightDialog {
    frame: SpecDialog,
    form: Form,
    target: SkylightRef,
}

struct Form {
    spec: SkylightSpec,
    opts: SkylightOptions,
    outline: Vec<Point>,
    fields: Fields,
    /// Edit Skylight Shape was pressed.
    edit_shape: bool,
}

impl SkylightDialog {
    fn new(
        target: SkylightRef,
        spec: SkylightSpec,
        opts: SkylightOptions,
        outline: Vec<Point>,
    ) -> Self {
        Self {
            frame: SpecDialog::new("Skylight Specification", "skylight"),
            form: Form {
                spec,
                opts,
                outline,
                fields: Fields::default(),
                edit_shape: false,
            },
            target,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn options_mut(&mut self) -> &mut SkylightOptions {
        &mut self.form.opts
    }

    pub fn spec_mut(&mut self) -> &mut SkylightSpec {
        &mut self.form.spec
    }

    /// Edit Skylight Shape was pressed: the shape is Custom and its corners
    /// are moved on the plan.
    pub fn editing_shape(&self) -> bool {
        self.form.edit_shape
    }
}

impl Form {
    fn general(&mut self, ui: &mut Ui) {
        section(ui, "Skylight");
        row(ui, "Shape", |ui| {
            egui::ComboBox::from_id_salt("sky_shape")
                .selected_text(self.opts.shape.label())
                .show_ui(ui, |ui| {
                    for s in SkylightShape::ALL {
                        ui.selectable_value(&mut self.opts.shape, s, s.label());
                    }
                });
        });
        let custom = self.opts.shape == SkylightShape::Custom;
        ui.add_enabled_ui(!custom, |ui| {
            self.fields
                .length_row(ui, "Width", "sky_w", &mut self.opts.width);
            if self.opts.shape != SkylightShape::Circle {
                self.fields
                    .length_row(ui, "Length", "sky_l", &mut self.opts.length);
            }
        });
        section(ui, "Frame");
        self.fields
            .length_row(ui, "Frame Width", "sky_fw", &mut self.spec.frame_width);
        self.fields
            .length_row(ui, "Frame Height", "sky_fh", &mut self.spec.curb_height);
        ui.checkbox(&mut self.opts.display_in_plan, "Display in Plan View");
        ui.add_space(6.0);
        if ui
            .button("Edit Skylight Shape")
            .on_hover_text("Move the corners of the opening on the plan")
            .clicked()
        {
            self.opts.shape = SkylightShape::Custom;
            self.edit_shape = true;
        }
    }

    fn rim(&mut self, ui: &mut Ui) {
        section(ui, "Inside Hole Rim");
        for r in HoleRim::ALL {
            ui.radio_value(&mut self.opts.rim, r, r.label());
        }
        ui.weak("Square walls are square to the roof; plumb walls are vertical; Plumb/Square has a plumb sill and a square head.");
    }

    fn ceiling(&mut self, ui: &mut Ui) {
        section(ui, "Ceiling Hole");
        for c in CeilingHole::ALL {
            ui.radio_value(&mut self.opts.ceiling_hole, c, c.label());
        }
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter a valid length".into());
        }
        if self.opts.shape != SkylightShape::Custom
            && (self.opts.width < 6.0 || self.opts.length < 6.0)
        {
            return Some("A skylight is at least 6 inches each way".into());
        }
        if self.spec.curb_height < 0.0 || self.spec.frame_width < 0.0 {
            return Some("The frame cannot be negative".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.general(ui),
            1 => self.rim(ui),
            _ => self.ceiling(ui),
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        let inner = area.shrink(20.0);
        let (w, l) = (self.opts.width.max(1.0), self.opts.length.max(1.0));
        let k = (f64::from(inner.width()) / w).min(f64::from(inner.height()) / l);
        let centre = polygon_centroid(&self.outline);
        let outline = if self.opts.shape == SkylightShape::Custom && self.outline.len() >= 3 {
            self.outline.clone()
        } else {
            shape_outline(
                self.opts.shape,
                Point::ZERO,
                Point::new(1.0, 0.0),
                w,
                l,
                SKYLIGHT_FACETS,
            )
        };
        let origin = if self.opts.shape == SkylightShape::Custom {
            centre
        } else {
            Point::ZERO
        };
        let to = |q: Point| {
            Pos2::new(
                inner.center().x + ((q.x - origin.x) * k) as f32,
                inner.center().y - ((q.y - origin.y) * k) as f32,
            )
        };
        let mut ring: Vec<Pos2> = outline.iter().map(|q| to(*q)).collect();
        if let Some(first) = ring.first().copied() {
            ring.push(first);
        }
        p.add(Shape::line(ring, Stroke::new(2.0_f32, PV_INK)));
        let frame = (self.spec.frame_width * k) as f32;
        if frame > 1.0 {
            p.rect_stroke(
                inner.shrink(frame.min(inner.width() * 0.3)),
                0.0,
                Stroke::new(1.0_f32, PV_FAINT),
                egui::StrokeKind::Inside,
            );
        }
        p.circle_filled(inner.center(), 2.0, PV_ACCENT);
    }
}

thread_local! {
    static HOST: RefCell<Option<SkylightDialog>> = const { RefCell::new(None) };
}

/// Opens the Skylight Specification of skylight `r` on the active floor.
/// False when it is not a skylight.
pub fn open_spec(cx: &EditorContext, r: SkylightRef) -> bool {
    let (Some(hole), Some(opts)) = (hole_of(cx.floor(), r), options_of(cx.floor(), r)) else {
        return false;
    };
    let spec = hole.skylight.unwrap_or_default();
    HOST.with(|h| {
        *h.borrow_mut() = Some(SkylightDialog::new(r, spec, opts, hole.outline.clone()));
    });
    true
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn dialog_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

/// Test access to the open Skylight Specification.
#[cfg(test)]
pub fn with_dialog<R>(f: impl FnOnce(&mut SkylightDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

/// Closes the open Skylight Specification and applies it as OK would.
#[cfg(test)]
pub fn accept_dialog(cx: &mut EditorContext) -> bool {
    let Some(d) = HOST.with(|h| h.borrow_mut().take()) else {
        return false;
    };
    apply(cx, d.target, d.form.spec, d.form.opts)
}

/// Shows the open dialog once a frame and applies its OK.
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    if let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) {
        match d.show(ctx) {
            Outcome::Open => HOST.with(|h| *h.borrow_mut() = Some(d)),
            Outcome::Cancel => {}
            Outcome::Ok => {
                apply(cx, d.target, d.form.spec, d.form.opts);
            }
        }
    }
}
