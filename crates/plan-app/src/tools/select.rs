//! Select Objects (S-1..S-38 in `docs/parity/select-and-edit.md`).
//!
//! * click selects (Shift adds/removes), click on empty space deselects;
//!   Tab cycles through the objects under the pointer;
//! * dragging a wall (body or middle handle) is Chief's perpendicular move:
//!   connected walls keep their directions and stretch, openings travel with
//!   it; Alt moves it freely and connected ends follow;
//! * the end handles stretch a wall with snapping, connected ends follow, and
//!   dropping an end on another wall splits that wall (T-junction);
//! * dragging an opening slides it along its wall (or onto another wall);
//! * CAD items: Move, Rotate, line/vertex/radius handles;
//! * drag on empty space is a marquee: left-to-right selects what it
//!   encloses, right-to-left what it touches;
//! * clicking a temporary dimension value edits it; Enter moves the object;
//! * every drag is one undo step, Esc cancels it;
//! * double-click or Enter opens the specification; Delete deletes;
//! * Alt on the press starts a marquee even over an object (S-30), and the
//!   Marquee Selection setting says whether a marquee encloses, touches or
//!   follows its direction (S-29);
//! * Ctrl (or Cmd) held when a drag starts copies the selection and drags the
//!   copies (S-94); the drag auto-scrolls the view at the edge (S-99);
//! * digits typed during any move or rotate drag set the distance or angle
//!   (S-28, S-23);
//! * Edit Area and Stretch CAD (S-90, S-91) are in [`area`]; the words for the
//!   status bar and the hover tooltip (S-6, S-98, DW-65) in [`describe`].

pub mod area;
pub mod describe;

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::clipboard::Clipboard;
use crate::editor::handles::{self, hit_handle, HandleKind};
use crate::editor::ops::{self, cad_center, JOIN_TOL};
use crate::editor::rooms_edit;
use crate::editor::selection::{expand_groups, extra_in_rect, hit_test_cx, layer_of};
use crate::editor::snap::snap_to_grid;
use crate::editor::stairs_view::{self, StairHandleKind};
use crate::editor::transform;
use crate::editor::{
    behaviors, details_view, foundation_view, framing_view, opening_edit, opening_view, placed,
    roof_view, site_view, tempdim, wall_edit,
};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, EditorRequest, ObjectRef};
use crate::shell::view3d_panel::{Outbox, ViewRequest};
use crate::toolbar::ViewFlag;
use crate::tools::cad::arcs;
use crate::tools::camera::{self as camera_tool, CamHandle};
use eframe::egui::{self, Key, Pos2, Rect, Shape, Stroke};
use plan_core::cad::CadItem;
use plan_core::details::DetailsLayer;
use plan_core::foundation::FoundationLayer;
use plan_core::geometry::{point_in_polygon, segment_intersection, Point};
use plan_core::{Id, Jamb, OpeningKind, Project, WallEnd};

/// Pixels the pointer must travel before a press becomes a drag (S-26).
const DRAG_THRESHOLD_PX: f32 = 3.0;

#[derive(Clone, Copy, Debug)]
enum Op {
    /// Perpendicular move of one wall (free with Alt).
    WallMove(Id),
    WallEnd(Id, WallEnd),
    /// The bulge handle of a curved wall (W-67).
    WallBulge(Id),
    OpeningSlide(Id),
    /// A jamb of an opening: the other jamb stays (DW-26).
    OpeningResize(Id, Jamb),
    /// The label of an opening, moved off its spot (DW-63).
    OpeningLabel(Id),
    /// The depth handle of a bay, box or bow window (manual p. 626).
    BayDepth(Id),
    /// Offset of a dimension line.
    DimOffset(Id),
    CadRotate(Id),
    /// The diamond on an arc edge of a polyline (edge `n`): sets its bulge
    /// (CAD-22).
    CadArcBulge(Id, usize),
    /// Line end, polyline vertex or circle radius.
    CadVertex(Id, HandleKind),
    /// Plain translate of the whole selection.
    Group,
    /// The Rotate handle of a multi-object selection: turns every object
    /// about the point (S-101, S-102).
    GroupRotate(Point),
    /// Click-only handle: flip the door swing.
    Swing(Id),
    /// A handle (or the body, `Move`) of a stair.
    Stair(Id, StairHandleKind),
    Cabinet(Id, HandleKind),
    Symbol(Id, HandleKind),
    DeviceMove(Id),
    RoofMove(Id),
    /// A handle of a roof plane: corner, edge middle, pitch arrow or rotate
    /// knob (RF-38).
    RoofHandle(Id, roof_view::PlaneHandle),
    /// The angle-of-view and tilt handles of a selected camera (C-25).
    CameraWedge(Id, camera_tool::WedgeHandle),
    /// Corner `n` of a slab, slab hole or platform hole.
    FoundationVertex(Id, usize),
    /// One end (`true`: the second point) of a framing member or layout line.
    FramingEnd(Id, bool),
    /// Corner `n` of a Truss Base.
    FramingVertex(Id, usize),
    /// Corner `n` of a deck, floor region, molding or outline solid (a
    /// molding line's two ends are corners 0 and 1).
    DetailVertex(Id, usize),
    Camera(Id, CamHandle),
    /// Vertex `n` of a terrain element.
    TerrainVertex(site_view::TerrainHit, usize),
}

impl Op {
    fn label(self) -> &'static str {
        match self {
            Op::WallMove(_) => "Move Wall",
            Op::WallEnd(..) => "Stretch Wall",
            Op::WallBulge(_) => "Curve Wall",
            Op::OpeningSlide(_) | Op::Swing(_) => "Move Opening",
            Op::OpeningResize(..) => "Resize Opening",
            Op::OpeningLabel(_) => "Move Opening Label",
            Op::BayDepth(_) => "Change Bay Depth",
            Op::DimOffset(_) => "Move Dimension",
            Op::CadRotate(_) => "Rotate",
            Op::CadArcBulge(..) => "Curve Polyline Edge",
            Op::CadVertex(..) => "Reshape",
            Op::Group => "Move Objects",
            Op::GroupRotate(_) => "Rotate Objects",
            Op::Stair(_, k) => stairs_view::drag_label(k),
            Op::Cabinet(..) => "Edit Cabinet",
            Op::Symbol(..) => "Edit Symbol",
            Op::DeviceMove(_) => "Move Device",
            Op::RoofMove(_) => "Move Roof Plane",
            Op::RoofHandle(_, h) => match h {
                roof_view::PlaneHandle::Vertex(_) => "Reshape Roof Plane",
                roof_view::PlaneHandle::Edge(_) => "Move Roof Edge",
                roof_view::PlaneHandle::Pitch => "Change Roof Pitch",
                roof_view::PlaneHandle::Rotate => "Rotate Roof Plane",
            },
            Op::CameraWedge(_, camera_tool::WedgeHandle::Tilt) => "Tilt Camera",
            Op::CameraWedge(..) => "Change Angle of View",
            Op::FoundationVertex(..) => "Reshape Foundation Object",
            Op::FramingEnd(..) => "Stretch Framing",
            Op::FramingVertex(..) => "Reshape Truss Base",
            Op::DetailVertex(..) => "Reshape Detail",
            Op::Camera(..) => "Edit Camera",
            Op::TerrainVertex(..) => "Reshape Terrain Element",
        }
    }
}

struct Active {
    op: Op,
    original: Project,
    start: Point,
    /// Walls the snap engine ignores (the dragged wall and its joined ones).
    exclude: Vec<Id>,
    /// A Ctrl/Cmd drag: the copies were made when the drag began and are what
    /// moves (S-94). `before_copy` is the plan as it stood before them and
    /// `prev_selection` what was selected, for Esc.
    copy: bool,
    before_copy: Option<Project>,
    prev_selection: Vec<ObjectRef>,
}

enum Drag {
    None,
    Armed {
        op: Op,
        start: Point,
        screen: Pos2,
    },
    Active(Box<Active>),
    Marquee {
        start: Point,
        current: Point,
        screen: Pos2,
        add: bool,
    },
}

pub struct SelectTool {
    drag: Drag,
    cursor: egui::CursorIcon,
    /// The room under the press when it landed on empty floor (R-16).
    room_click: Option<usize>,
    /// The marquee under way came from Ctrl/Cmd: it toggles what it covers,
    /// so a selected object leaves the selection (manual p. 260).
    marquee_toggle: bool,
}

impl Default for SelectTool {
    fn default() -> Self {
        Self {
            drag: Drag::None,
            cursor: egui::CursorIcon::Default,
            room_click: None,
            marquee_toggle: false,
        }
    }
}

fn snap_unit_round(v: f64, unit: f64) -> f64 {
    (v / unit).round() * unit
}

/// The rounding unit of a move: the grid, except while a distance is being
/// typed, when the typed number is taken as it is (S-28).
fn unit(cx: &EditorContext) -> f64 {
    if cx.typed_input.has_text() {
        1e-6
    } else {
        cx.snap_unit()
    }
}

// ----- Marquee Selection (S-29, S-30) -----

/// What a marquee picks (Edit > Marquee Selection).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MarqueeMode {
    /// Left to right encloses, right to left touches (Chief's rubber band).
    #[default]
    ByDirection,
    /// Only what lies wholly inside.
    Enclosing,
    /// Everything the rectangle touches.
    Touching,
}

impl MarqueeMode {
    pub const ALL: [MarqueeMode; 3] = [
        MarqueeMode::ByDirection,
        MarqueeMode::Enclosing,
        MarqueeMode::Touching,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MarqueeMode::ByDirection => "By Drag Direction",
            MarqueeMode::Enclosing => "Enclosing",
            MarqueeMode::Touching => "Touching",
        }
    }

    /// The menu command that picks this mode.
    pub fn command(self) -> &'static str {
        match self {
            MarqueeMode::ByDirection => MARQUEE_DIRECTION,
            MarqueeMode::Enclosing => MARQUEE_ENCLOSING,
            MarqueeMode::Touching => MARQUEE_TOUCHING,
        }
    }

    /// Does a marquee dragged from `a` to `b` pick what it touches?
    pub fn crosses(self, a: Point, b: Point) -> bool {
        match self {
            MarqueeMode::ByDirection => b.x < a.x,
            MarqueeMode::Enclosing => false,
            MarqueeMode::Touching => true,
        }
    }
}

pub const MARQUEE_DIRECTION: &str = "select.marquee.direction";
pub const MARQUEE_ENCLOSING: &str = "select.marquee.enclosing";
pub const MARQUEE_TOUCHING: &str = "select.marquee.touching";
pub const EDIT_AREA: &str = "select.area.edit";
pub const EDIT_AREA_VISIBLE: &str = "select.area.visible";
pub const STRETCH_CAD: &str = "select.area.stretch_cad";

thread_local! {
    static MARQUEE: std::cell::Cell<MarqueeMode> =
        const { std::cell::Cell::new(MarqueeMode::ByDirection) };
    /// A drag (move, handle, marquee) is under way: the view scrolls when the
    /// pointer reaches the edge of the canvas.
    static DRAGGING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The Marquee Selection setting in force.
pub fn marquee_mode() -> MarqueeMode {
    MARQUEE.with(|m| m.get())
}

/// Sets the Marquee Selection setting (it lasts for the session).
pub fn set_marquee_mode(m: MarqueeMode) {
    MARQUEE.with(|c| c.set(m));
}

/// Is a Select Objects drag in progress? The shell scrolls the view when the
/// pointer is at the edge of the canvas while this is true (S-99).
pub fn drag_in_progress() -> bool {
    DRAGGING.with(|d| d.get())
}

fn set_dragging(on: bool) {
    DRAGGING.with(|d| d.set(on));
}

/// Width of the strip along the canvas edge in which a drag scrolls the view.
pub const AUTO_SCROLL_MARGIN_PX: f32 = 28.0;
/// Fastest scroll, pixels per second.
pub const AUTO_SCROLL_MAX_PX_PER_S: f32 = 900.0;

/// How far to pan the view (pixels, in the direction the content moves) for a
/// drag whose pointer is at `pos` over the canvas `rect`, after `dt` seconds.
/// Zero while the pointer is well inside; it grows toward the edge and is
/// capped past it (S-99).
pub fn auto_scroll_vector(rect: Rect, pos: Pos2, dt: f32) -> egui::Vec2 {
    let axis = |lo: f32, hi: f32, v: f32| -> f32 {
        // Positive: the pointer is `depth` pixels into the low-edge strip.
        let low = AUTO_SCROLL_MARGIN_PX - (v - lo);
        let high = AUTO_SCROLL_MARGIN_PX - (hi - v);
        let speed = |depth: f32| {
            (depth / AUTO_SCROLL_MARGIN_PX).clamp(0.0, 1.5) / 1.5 * AUTO_SCROLL_MAX_PX_PER_S * dt
        };
        if low > 0.0 {
            speed(low)
        } else if high > 0.0 {
            -speed(high)
        } else {
            0.0
        }
    };
    egui::vec2(
        axis(rect.left(), rect.right(), pos.x),
        axis(rect.top(), rect.bottom(), pos.y),
    )
}

/// Does `id` belong to a Select Objects command (Edit Area, Stretch CAD, the
/// Marquee Selection choices)?
pub fn is_command(id: &str) -> bool {
    id.starts_with("select.")
}

/// Runs one of the commands. True when the Select tool should be the tool
/// from now on (Edit Area and Stretch CAD wait for a rubber band).
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        MARQUEE_DIRECTION => set_marquee_mode(MarqueeMode::ByDirection),
        MARQUEE_ENCLOSING => set_marquee_mode(MarqueeMode::Enclosing),
        MARQUEE_TOUCHING => set_marquee_mode(MarqueeMode::Touching),
        EDIT_AREA => {
            area::begin(
                cx,
                area::AreaKind::Edit {
                    visible_only: false,
                },
            );
            return true;
        }
        EDIT_AREA_VISIBLE => {
            area::begin(cx, area::AreaKind::Edit { visible_only: true });
            return true;
        }
        STRETCH_CAD => {
            area::begin(cx, area::AreaKind::StretchCad);
            return true;
        }
        _ => {}
    }
    false
}

// ----- the move operations (shared by drags and arrow-key nudges) -----

/// Moves a wall by `delta`: perpendicular only, or freely with `free`.
/// A wall dragged by its move handle: square to itself (Default), at the
/// allowed angles (Alternate, Move, or the Polar movement method), or
/// anywhere with Ctrl/Cmd held (`free`, manual p. 237).
fn move_wall(cx: &mut EditorContext, id: Id, delta: Point, free: bool) {
    let fl = cx.floor;
    let unit = unit(cx);
    let polar = if free {
        None
    } else if behaviors::polar_move(cx) {
        behaviors::polar_delta(cx, delta)
    } else {
        None
    };
    if free || polar.is_some() {
        let d = polar.unwrap_or_else(|| {
            Point::new(
                snap_unit_round(delta.x, unit),
                snap_unit_round(delta.y, unit),
            )
        });
        ops::translate_walls_with_followers(&mut cx.project, fl, &[id], d);
    } else {
        move_wall_perpendicular(cx, id, delta);
    }
}

/// The wall moved square to itself by the part of `delta` along its normal.
fn move_wall_perpendicular(cx: &mut EditorContext, id: Id, delta: Point) {
    let fl = cx.floor;
    let unit = unit(cx);
    if let Some(w) = cx.floor().wall(id) {
        let s = snap_unit_round(delta.dot(w.normal()), unit);
        if s.abs() > 1e-9 {
            ops::move_wall_perpendicular(&mut cx.project, fl, id, s);
        }
    }
}

/// Slides an opening along its wall by `delta`.
fn slide_opening_by(cx: &mut EditorContext, id: Id, delta: Point) {
    let fl = cx.floor;
    let unit = unit(cx);
    let Some(o) = cx.floor().openings.iter().find(|o| o.id == id).cloned() else {
        return;
    };
    let Some(dir) = cx
        .floor()
        .wall(o.wall_id)
        .map(|w| w.tangent_along(o.center_offset))
    else {
        return;
    };
    let center = snap_unit_round(o.center_offset + delta.dot(dir), unit);
    // Slides the whole mulled unit when the window belongs to one.
    cx.project.slide_opening(fl, id, center);
}

/// While an opening is dragged, a typed length sets the gap between its
/// nearer jamb and the wall end or neighbouring opening on that side (DW-12);
/// the center that makes it so, when a length was typed. Measured on the
/// project as it was before the drag (`cx.floor()` is that copy while a drag
/// is applied).
fn typed_slide_center(cx: &EditorContext, id: Id) -> Option<f64> {
    let v = tempdim::typed_value(cx)?;
    let f = cx.floor();
    let o = f.openings.iter().find(|o| o.id == id)?;
    let w = f.wall(o.wall_id)?;
    let dims = tempdim::opening_temp_dims(
        f,
        w,
        o,
        ObjectRef::Opening(id),
        &tempdim::TempLocate::of(cx),
    );
    let gap = |kind| dims.iter().find(|d| d.kind == kind).map(|d| d.value);
    let (start, end) = (
        gap(tempdim::TempDimKind::OpeningToStart)?,
        gap(tempdim::TempDimKind::OpeningToEnd)?,
    );
    // The side the opening is nearer to is the one being set.
    let delta = if start <= end { v - start } else { end - v };
    Some(o.center_offset + delta)
}

/// Translates every selected object by `delta`.
fn move_group(cx: &mut EditorContext, items: &[ObjectRef], delta: Point) {
    move_group_ex(cx, items, delta, true);
}

/// [`move_group`]; without `followers` the walls move alone (the copies of a
/// Ctrl-drag sit on top of the walls they were copied from and must not drag
/// those along).
fn move_group_ex(cx: &mut EditorContext, items: &[ObjectRef], delta: Point, followers: bool) {
    let fl = cx.floor;
    let unit = unit(cx);
    let d = Point::new(
        snap_unit_round(delta.x, unit),
        snap_unit_round(delta.y, unit),
    );
    let walls: Vec<Id> = items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Wall(id) => Some(*id),
            _ => None,
        })
        .collect();
    if followers {
        ops::translate_walls_with_followers(&mut cx.project, fl, &walls, d);
    } else {
        for id in &walls {
            cx.project.translate_wall(fl, *id, d);
        }
    }
    cx.translate_extra(items, d);
    for o in items {
        match *o {
            ObjectRef::Opening(id) => {
                let host_moves = cx
                    .floor()
                    .openings
                    .iter()
                    .find(|x| x.id == id)
                    .is_some_and(|x| walls.contains(&x.wall_id));
                if !host_moves {
                    slide_opening_by(cx, id, delta);
                }
            }
            ObjectRef::Dimension(id) => {
                if let Some(dim) = cx.project.floors[fl]
                    .dimensions
                    .iter_mut()
                    .find(|x| x.id == id)
                {
                    dim.translate(d);
                }
            }
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                    ops::translate_cad(&mut c.item, d);
                }
            }
            _ => {}
        }
    }
}

fn single_op_for_body(o: ObjectRef) -> Op {
    match o {
        ObjectRef::Wall(id) => Op::WallMove(id),
        ObjectRef::Opening(id) => Op::OpeningSlide(id),
        ObjectRef::Dimension(id) => Op::DimOffset(id),
        ObjectRef::Stair(id) => Op::Stair(id, StairHandleKind::Move),
        ObjectRef::Cabinet(id) => Op::Cabinet(id, HandleKind::Move),
        ObjectRef::Symbol(id) => Op::Symbol(id, HandleKind::Move),
        ObjectRef::Device(id) => Op::DeviceMove(id),
        ObjectRef::RoofPlane(id) => Op::RoofMove(id),
        ObjectRef::Camera(id) => Op::Camera(id, CamHandle::Move),
        _ => Op::Group,
    }
}

/// Selects the object a click landed on; a room goes to the room selection.
fn select_hit(cx: &mut EditorContext, o: ObjectRef) {
    match o {
        ObjectRef::Room(i) => rooms_edit::select_room(cx, i),
        other => {
            rooms_edit::clear_room_selection();
            cx.selection.set(other);
        }
    }
}

/// Outlines the selected terrain elements in the selection colour (the plan
/// renderer draws the terrain itself, without a selection state).
fn draw_terrain_selection(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let Some(view) = site_view::terrain_view(&cx.project) else {
        return;
    };
    let stroke = Stroke::new(2.5_f32, cx.palette.selection);
    for o in &cx.selection.items {
        let ObjectRef::TerrainObject(hit) = *o else {
            continue;
        };
        let t = &view.record.terrain;
        let pts = site_view::hit_points(t, hit);
        if let [only] = pts.as_slice() {
            painter.circle_stroke(cam.world_to_screen(*only), 7.0, stroke);
        } else {
            site_view::draw_polyline(
                painter,
                cam,
                &pts,
                site_view::hit_is_closed(t, hit),
                stroke,
                false,
            );
        }
    }
}

/// The wall a selected door or window sits in, softly outlined while its
/// temporary dimensions are shown (S-110).
fn draw_host_walls(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let floor = cx.floor();
    let stroke = Stroke::new(1.0_f32, cx.palette.selection.gamma_multiply(0.55));
    let mut done: Vec<Id> = Vec::new();
    for o in &cx.selection.items {
        let ObjectRef::Opening(id) = *o else { continue };
        let Some(host) = floor
            .openings
            .iter()
            .find(|x| x.id == id)
            .map(|x| x.wall_id)
        else {
            continue;
        };
        if done.contains(&host) || cx.selection.contains(ObjectRef::Wall(host)) {
            continue;
        }
        done.push(host);
        if let Some(w) = floor.wall(host) {
            let pts: Vec<Pos2> = w
                .footprint()
                .iter()
                .map(|p| cam.world_to_screen(*p))
                .collect();
            painter.add(Shape::closed_line(pts, stroke));
        }
    }
}

/// The diamonds on the arc edges of a selected polyline (CAD-22).
fn draw_arc_handles(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let Some(ObjectRef::Cad(id)) = cx.selection.single() else {
        return;
    };
    for (_, apex) in arcs::arc_handles(cx, id) {
        let c = cam.world_to_screen(apex);
        let r = 5.0;
        painter.add(Shape::convex_polygon(
            vec![
                c + egui::vec2(0.0, -r),
                c + egui::vec2(r, 0.0),
                c + egui::vec2(0.0, r),
                c + egui::vec2(-r, 0.0),
            ],
            cx.palette.selection,
            Stroke::new(1.0_f32, cx.palette.ghost_stroke),
        ));
    }
}

/// The wall hosting each selected opening (for S-110 tests).
pub fn host_walls(cx: &EditorContext) -> Vec<Id> {
    let floor = cx.floor();
    let mut out: Vec<Id> = Vec::new();
    for o in &cx.selection.items {
        if let ObjectRef::Opening(id) = *o {
            if let Some(x) = floor.openings.iter().find(|x| x.id == id) {
                if !out.contains(&x.wall_id) {
                    out.push(x.wall_id);
                }
            }
        }
    }
    out
}

// ----- marquee -----

fn rect_corners(lo: Point, hi: Point) -> [Point; 4] {
    [lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)]
}

fn in_rect(p: Point, lo: Point, hi: Point) -> bool {
    p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y
}

/// Does the polygon touch the rectangle?
fn poly_touches_rect(poly: &[Point], lo: Point, hi: Point) -> bool {
    if poly.iter().any(|p| in_rect(*p, lo, hi)) {
        return true;
    }
    let corners = rect_corners(lo, hi);
    if corners.iter().any(|c| point_in_polygon(*c, poly)) {
        return true;
    }
    (0..poly.len()).any(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        (0..4).any(|k| segment_intersection(a, b, corners[k], corners[(k + 1) % 4]).is_some())
    })
}

/// What a marquee dragged from `a` to `b` picks, under the Marquee Selection
/// setting.
fn objects_in_rect(cx: &EditorContext, a: Point, b: Point) -> Vec<ObjectRef> {
    let crossing = marquee_mode().crosses(a, b);
    let lo = Point::new(a.x.min(b.x), a.y.min(b.y));
    let hi = Point::new(a.x.max(b.x), a.y.max(b.y));
    objects_in_box(cx, lo, hi, crossing, false)
}

/// The objects an axis-aligned box encloses (or touches, with `crossing`),
/// on displayed, unlocked layers; `hidden` also takes those on layers that are
/// not displayed (Edit Area).
fn objects_in_box(
    cx: &EditorContext,
    lo: Point,
    hi: Point,
    crossing: bool,
    hidden: bool,
) -> Vec<ObjectRef> {
    let floor = cx.floor();
    let hit = |poly: &[Point]| {
        if crossing {
            poly_touches_rect(poly, lo, hi)
        } else {
            poly.iter().all(|p| in_rect(*p, lo, hi))
        }
    };
    let usable = |o: ObjectRef| {
        layer_of(floor, o)
            .is_none_or(|l| (hidden || cx.layers().is_visible(&l)) && !cx.layers().is_locked(&l))
    };
    let mut out = Vec::new();
    for w in &floor.walls {
        let r = ObjectRef::Wall(w.id);
        if usable(r) && hit(&w.footprint()) {
            out.push(r);
        }
        for o in floor.openings_on(w.id) {
            let half = w.thickness * 0.5;
            let r = ObjectRef::Opening(o.id);
            if usable(r) && hit(&w.band(o.start_offset(), o.end_offset(), -half, half)) {
                out.push(r);
            }
        }
    }
    for d in &floor.dimensions {
        let (p, q) = d.line_points();
        let r = ObjectRef::Dimension(d.id);
        let ok = if crossing {
            poly_touches_rect(&[p, q], lo, hi)
        } else {
            [d.start, d.end, p, q].iter().all(|x| in_rect(*x, lo, hi))
        };
        if usable(r) && ok {
            out.push(r);
        }
    }
    for c in &floor.cad {
        let (blo, bhi) = c.bounds();
        let r = ObjectRef::Cad(c.id);
        let ok = if crossing {
            blo.x <= hi.x && bhi.x >= lo.x && blo.y <= hi.y && bhi.y >= lo.y
        } else {
            in_rect(blo, lo, hi) && in_rect(bhi, lo, hi)
        };
        if usable(r) && ok {
            out.push(r);
        }
    }
    // The other kinds answer to the displayed layers only.
    out.extend(extra_in_rect(cx, lo, hi, crossing));
    out
}

// ----- typed input during a drag (S-28, S-23) -----

/// What the digits typed during a drag mean.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TypedKind {
    /// Nothing is typed for this drag.
    No,
    /// A length (and an angle with Tab): wall ends, openings and every move.
    Length,
    /// Degrees: the Rotate handles.
    Angle,
}

fn typed_kind(op: Op) -> TypedKind {
    match op {
        Op::WallEnd(..)
        | Op::OpeningSlide(_)
        | Op::OpeningResize(..)
        | Op::WallMove(_)
        | Op::Group
        | Op::DeviceMove(_)
        | Op::RoofMove(_)
        | Op::Stair(_, StairHandleKind::Move)
        | Op::Cabinet(_, HandleKind::Move)
        | Op::Symbol(_, HandleKind::Move)
        | Op::Camera(_, CamHandle::Move) => TypedKind::Length,
        Op::CadRotate(_) | Op::GroupRotate(_) => TypedKind::Angle,
        _ => TypedKind::No,
    }
}

/// Is this a drag that carries objects (so Ctrl/Cmd makes it a copy)?
fn is_body_move(op: Op) -> bool {
    matches!(
        op,
        Op::Group
            | Op::WallMove(_)
            | Op::DeviceMove(_)
            | Op::Stair(_, StairHandleKind::Move)
            | Op::Cabinet(_, HandleKind::Move)
            | Op::Symbol(_, HandleKind::Move)
            | Op::Camera(_, CamHandle::Move)
    )
}

/// `v` turned by `deg` degrees counter-clockwise.
fn turned(v: Point, deg: f64) -> Point {
    let (s, c) = deg.to_radians().sin_cos();
    Point::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Where the pointer of a move drag stands for the typed distance (and angle)
/// when the drag has no temporary-dimension logic of its own.
pub(super) fn typed_move_target(cx: &EditorContext, start: Point, raw: Point) -> Option<Point> {
    if !cx.typed_input.has_text() {
        return None;
    }
    tempdim::typed_point(cx, start, raw)
}

/// Where the pointer of a rotate drag stands for the typed angle: the press
/// point turned about `center`.
pub(super) fn typed_rotate_target(
    cx: &EditorContext,
    center: Point,
    start: Point,
) -> Option<Point> {
    let deg = tempdim::typed_angle(cx)?;
    let mut v = start - center;
    if v.length() < 1e-9 {
        v = Point::new(10.0, 0.0);
    }
    Some(center + turned(v, deg))
}

/// The pointer event a drag is applied with: the real one, or one whose
/// position stands for the typed number (with the snaps off, so the number is
/// taken as typed).
fn typed_pointer(cx: &EditorContext, a: &Active, p: &PointerEvent) -> Option<PointerEvent> {
    if !cx.typed_input.has_text() {
        return None;
    }
    let fl = cx.floor;
    let world = match a.op {
        // Wall ends and openings read the typed values themselves.
        Op::WallEnd(..) | Op::OpeningSlide(_) | Op::OpeningResize(..) => return None,
        Op::GroupRotate(center) => typed_rotate_target(cx, center, a.start)?,
        Op::CadRotate(id) => {
            let c = a.original.floors[fl].cad.iter().find(|c| c.id == id)?;
            typed_rotate_target(cx, cad_center(&c.item), a.start)?
        }
        Op::WallMove(id) => {
            // The wall only goes sideways: the typed length is the distance
            // across, on the side the pointer is on.
            let w = a.original.floors[fl].wall(id)?;
            let n = w.normal();
            let sign = if (p.world - a.start).dot(n) < 0.0 {
                -1.0
            } else {
                1.0
            };
            a.start + n * (sign * tempdim::typed_value(cx)?)
        }
        op if typed_kind(op) == TypedKind::Length => typed_move_target(cx, a.start, p.world)?,
        _ => return None,
    };
    let mut e = *p;
    e.world = world;
    e.snapped = world;
    e.modifiers.ctrl = true;
    Some(e)
}

/// The status-bar words while a number is typed into a drag.
fn typed_readout(cx: &mut EditorContext, a: &Active, world: Point) {
    use crate::editor::typed_input::{angle_deg, TypedField};
    if !cx.typed_input.is_armed()
        || matches!(
            a.op,
            Op::WallEnd(..) | Op::OpeningSlide(_) | Op::OpeningResize(..)
        )
    {
        return;
    }
    let ti = &cx.typed_input;
    let on = |f: TypedField| ti.has_text() && ti.field() == f;
    if typed_kind(a.op) == TypedKind::Angle {
        let live = format!("{:.1}\u{b0}", {
            let center = match a.op {
                Op::GroupRotate(c) => c,
                Op::CadRotate(id) => a.original.floors[cx.floor]
                    .cad
                    .iter()
                    .find(|c| c.id == id)
                    .map_or(a.start, |c| cad_center(&c.item)),
                _ => a.start,
            };
            (angle_deg(center, world) - angle_deg(center, a.start)).rem_euclid(360.0)
        });
        cx.readout = Some(format!(
            "Rotate: {}",
            if on(TypedField::Angle) {
                format!("{}|", ti.angle_text())
            } else {
                live
            }
        ));
        return;
    }
    let live_len = cx.fmt_dim(a.start.dist(world));
    let live_ang = format!("{:.1}\u{b0}", angle_deg(a.start, world));
    cx.readout = Some(format!(
        "Distance: {}   Angle: {}",
        if on(TypedField::Length) {
            format!("{}|", ti.length_text())
        } else {
            live_len
        },
        if on(TypedField::Angle) {
            format!("{}|", ti.angle_text())
        } else {
            live_ang
        },
    ));
}

impl SelectTool {
    fn update_hover(&mut self, cx: &mut EditorContext, p: &PointerEvent) {
        let tol = cx.pick_tol();
        let hs = handles::handles_for(cx, cx.px_per_in);
        if let Some(h) = hit_handle(&hs, p.world, tol) {
            self.cursor = h.cursor;
            cx.hover = None;
            return;
        }
        let top = hit_test_cx(cx, p.world, tol).first().copied();
        self.cursor = if top.is_some() {
            egui::CursorIcon::PointingHand
        } else {
            egui::CursorIcon::Default
        };
        cx.hover = top;
    }

    /// The drag a press at `at` starts on a handle of the selected object.
    fn handle_op(cx: &EditorContext, at: Point, tol: f64) -> Option<Op> {
        // A selected callout, marker or note (a group) has its own handles.
        if let Some((id, kind)) = crate::tools::text::annot_handle_at(cx, at, tol) {
            return Some(Op::CadVertex(id, kind));
        }
        match cx.selection.single()? {
            ObjectRef::Stair(id) => {
                let o = stairs_view::find(cx.floor(), id)?;
                let hs = stairs_view::handles(&o, cx.px_per_in);
                stairs_view::hit_handle(&hs, at, tol).map(|h| Op::Stair(id, h.kind))
            }
            ObjectRef::Camera(id) => {
                let c = cx.project.camera(id)?;
                camera_tool::hit_handle(c, at, tol)
                    .map(|h| Op::Camera(id, h))
                    .or_else(|| camera_tool::hit_wedge(c, at, tol).map(|w| Op::CameraWedge(id, w)))
            }
            ObjectRef::Cad(id) => {
                // The bulge diamonds of a polyline with arc edges come first.
                let arc = arcs::arc_handles(cx, id)
                    .into_iter()
                    .find(|(_, apex)| apex.dist(at) <= tol)
                    .map(|(edge, _)| Op::CadArcBulge(id, edge));
                arc.or_else(|| {
                    let hs = handles::handles_for(cx, cx.px_per_in);
                    hit_handle(&hs, at, tol)
                        .as_ref()
                        .and_then(Self::op_for_handle)
                })
            }
            _ => {
                let hs = handles::handles_for(cx, cx.px_per_in);
                hit_handle(&hs, at, tol)
                    .as_ref()
                    .and_then(Self::op_for_handle)
            }
        }
    }

    /// The operation a handle starts, if it is a draggable one.
    fn op_for_handle(h: &handles::Handle) -> Option<Op> {
        Some(match (h.target, h.kind) {
            (ObjectRef::Cabinet(id), k) => Op::Cabinet(id, k),
            (ObjectRef::Symbol(id), k) => Op::Symbol(id, k),
            (ObjectRef::Device(id), HandleKind::Move) => Op::DeviceMove(id),
            (ObjectRef::RoofPlane(id), HandleKind::Move) => Op::RoofMove(id),
            (ObjectRef::RoofPlane(id), k) => Op::RoofHandle(id, handles::roof_plane_handle(k)?),
            (ObjectRef::Camera(id), k) => Op::CameraWedge(id, handles::camera_wedge_handle(k)?),
            (ObjectRef::Foundation(id), HandleKind::Reshape(i)) => Op::FoundationVertex(id, i),
            (ObjectRef::Framing(id), HandleKind::ResizeStart) => Op::FramingEnd(id, false),
            (ObjectRef::Framing(id), HandleKind::ResizeEnd) => Op::FramingEnd(id, true),
            (ObjectRef::Framing(id), HandleKind::Reshape(i)) => Op::FramingVertex(id, i),
            (ObjectRef::Detail(id), HandleKind::Reshape(i)) => Op::DetailVertex(id, i),
            (ObjectRef::TerrainObject(hit), HandleKind::Reshape(i)) => Op::TerrainVertex(hit, i),
            // Only a two-point molding line has end handles.
            (ObjectRef::Detail(id), HandleKind::ResizeStart) => Op::DetailVertex(id, 0),
            (ObjectRef::Detail(id), HandleKind::ResizeEnd) => Op::DetailVertex(id, 1),
            (ObjectRef::Wall(id), HandleKind::ResizeStart) => Op::WallEnd(id, WallEnd::Start),
            (ObjectRef::Wall(id), HandleKind::ResizeEnd) => Op::WallEnd(id, WallEnd::End),
            (ObjectRef::Wall(id), HandleKind::PerpendicularMove) => Op::WallMove(id),
            (ObjectRef::Wall(id), HandleKind::Bulge) => Op::WallBulge(id),
            (ObjectRef::Opening(id), HandleKind::PerpendicularMove) => Op::OpeningSlide(id),
            (ObjectRef::Opening(id), HandleKind::Swing) => Op::Swing(id),
            (ObjectRef::Opening(id), HandleKind::Label) => Op::OpeningLabel(id),
            (ObjectRef::Opening(id), HandleKind::BayDepth) => Op::BayDepth(id),
            (ObjectRef::Opening(id), HandleKind::ResizeStart) => Op::OpeningResize(id, Jamb::Start),
            (ObjectRef::Opening(id), HandleKind::ResizeEnd) => Op::OpeningResize(id, Jamb::End),
            (ObjectRef::Dimension(id), HandleKind::PerpendicularMove) => Op::DimOffset(id),
            (ObjectRef::Cad(_) | ObjectRef::Text(_), HandleKind::Move) => Op::Group,
            (ObjectRef::Cad(id) | ObjectRef::Text(id), HandleKind::Rotate) => Op::CadRotate(id),
            (
                ObjectRef::Cad(id) | ObjectRef::Text(id),
                k @ (HandleKind::ResizeStart | HandleKind::ResizeEnd | HandleKind::Reshape(_)),
            ) => Op::CadVertex(id, k),
            _ => return None,
        })
    }

    /// Re-applies `a`'s operation for the pointer position, starting from the
    /// original project (so the result never accumulates error).
    fn apply(&self, cx: &mut EditorContext, a: &Active, p0: &PointerEvent) {
        cx.project = a.original.clone();
        let fl = cx.floor;
        // A typed distance or angle stands in for the pointer (S-28).
        let typed = typed_pointer(cx, a, p0);
        let p = typed.as_ref().unwrap_or(p0);
        typed_readout(cx, a, p.world);
        // Alt (or the right button, or a summon key) picks the behavior for
        // this drag; Ctrl/Cmd overrides snaps and move restrictions.
        behaviors::summon(p0);
        let alt = p.overrides();
        let total = p.world - a.start;
        // Edit Behaviors (S-65) that replace the plain move or reshape.
        let handled = match a.op {
            Op::Group if !a.copy => {
                let items = cx.selection.items.clone();
                behaviors::apply_group(cx, &items, a.start, p.world, p.modifiers.shift)
            }
            // A text's width and height handles size its box (S-25, TXT-3).
            Op::CadVertex(id, kind) => {
                behaviors::apply_vertex(cx, id, kind, a.start, p.world)
                    || crate::tools::text::drag_box_handle(cx, id, kind, p.world, alt)
            }
            _ => false,
        };
        if handled {
            cx.mark_dirty();
            return;
        }
        match a.op {
            Op::WallMove(id) => move_wall(cx, id, total, alt),
            Op::WallBulge(id) => {
                let Some(w) = cx.floor().wall(id).cloned() else {
                    return;
                };
                // The bulge is the pointer's distance from the chord midpoint
                // along the wall normal, on the grid unless Alt is held.
                let mid = Point::lerp(w.start, w.end, 0.5);
                let mut b = (p.world - mid).dot(w.normal());
                if !alt {
                    b = snap_unit_round(b, unit(cx));
                }
                let max = w.length() * 0.5;
                b = b.clamp(-max, max);
                let curve = (b.abs() >= 0.5).then_some(plan_core::WallCurve { bulge: b });
                cx.project.set_wall_curve(fl, id, curve);
            }
            Op::WallEnd(id, end) => {
                let Some(w) = cx.floor().wall(id).cloned() else {
                    return;
                };
                let fixed = if end == WallEnd::Start {
                    w.end
                } else {
                    w.start
                };
                let s = cx.snap_at(p.world, Some(fixed), alt, &a.exclude);
                cx.last_snap = Some(s);
                // Shift holds the angle increment; a typed length and angle
                // replace the pointer's (W-15..W-18).
                let to = wall_edit::drag_end(cx, fixed, p.world, s.point, p.modifiers.shift, alt);
                // A typed length too short for the wall's openings is held at
                // the shortest that hosts them (W-85).
                let to = if cx.typed_input.has_text() {
                    wall_edit::clamp_end_for_openings(cx, id, end, to)
                } else {
                    to
                };
                if to.dist(fixed) >= 1.0 {
                    ops::move_wall_end_joined(&mut cx.project, fl, id, end, to);
                }
            }
            Op::OpeningSlide(id) => {
                let unit = unit(cx);
                let Some(o) = cx.floor().openings.iter().find(|o| o.id == id).cloned() else {
                    return;
                };
                // Over another wall: re-host there (DW-18).
                let tol = cx.pick_tol();
                let other = cx
                    .floor()
                    .walls
                    .iter()
                    .filter(|w| w.id != o.wall_id && cx.layers().is_visible(&w.layer))
                    .map(|w| (w, w.closest_point(p.world).0.dist(p.world)))
                    .filter(|(_, d)| *d <= tol)
                    .min_by(|x, y| x.1.total_cmp(&y.1))
                    .map(|(w, _)| (w.id, w.locate(p.world).0));
                if let Some((wid, along)) = other {
                    let center = snap_unit_round(along, unit);
                    ops::place_opening_at(&mut cx.project, fl, id, wid, center);
                } else if let Some(center) = typed_slide_center(cx, id) {
                    cx.project.slide_opening(fl, id, center);
                } else if let Some(along) = cx
                    .floor()
                    .wall(o.wall_id)
                    .filter(|w| w.is_curved())
                    .map(|w| w.locate(p.world).0 - w.locate(a.start).0)
                {
                    // On a curved wall the opening follows the pointer along the arc.
                    let center = snap_unit_round(o.center_offset + along, unit);
                    cx.project.slide_opening(fl, id, center);
                } else {
                    slide_opening_by(cx, id, total);
                }
            }
            Op::OpeningResize(id, jamb) => {
                let unit = unit(cx);
                let Some(o) = cx.floor().openings.iter().find(|o| o.id == id).cloned() else {
                    return;
                };
                let Some(w) = cx.floor().wall(o.wall_id).cloned() else {
                    return;
                };
                let t = w.locate(p.world).0;
                let mut edge = if alt { t } else { snap_unit_round(t, unit) };
                // With the standard widths on, the width lands on the nearest
                // manufacturer width of the style (DW-27); Alt skips it.
                if !alt {
                    edge = opening_edit::standard_widths(cx, o.kind).snap_edge(&o, jamb, edge);
                }
                // A typed number is the new width (the other jamb stays).
                if let Some(width) = tempdim::typed_value(cx) {
                    edge = match jamb {
                        Jamb::Start => o.end_offset() - width,
                        Jamb::End => o.start_offset() + width,
                    };
                }
                cx.project.resize_opening(fl, id, jamb, edge);
            }
            Op::OpeningLabel(id) => {
                opening_view::drag_label(cx, id, p.world);
            }
            Op::BayDepth(id) => {
                opening_edit::drag_bay_depth(cx, id, p.world);
            }
            Op::DimOffset(id) => {
                let unit = unit(cx);
                // The whole string moves its line; a curve sets its distance.
                cx.project.floors[fl].drag_dimension_line(id, p.world, Some(unit));
            }
            Op::CadRotate(id) => {
                let Some(c) = a.original.floors[fl].cad.iter().find(|c| c.id == id) else {
                    return;
                };
                let center = cad_center(&c.item);
                let mut angle = p.world.sub(center).angle() - a.start.sub(center).angle();
                if !alt {
                    let inc = cx.defaults.grid.angle_snap_deg.max(1.0).to_radians();
                    angle = (angle / inc).round() * inc;
                }
                if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                    ops::rotate_cad(&mut c.item, center, angle);
                }
            }
            Op::CadArcBulge(id, edge) => {
                if let Some(mut l) = arcs::logical_of(cx, id) {
                    if l.set_bulge_through(edge, p.world).is_ok() {
                        arcs::replace_polyline(cx, id, &l);
                    }
                }
            }
            Op::CadVertex(id, kind) => {
                let to = cx.snap_at(p.world, None, alt, &[]).point;
                if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                    match (&mut c.item, kind) {
                        (CadItem::Line { a, .. }, HandleKind::ResizeStart) => *a = to,
                        (CadItem::Line { b, .. }, HandleKind::ResizeEnd) => *b = to,
                        (CadItem::Circle { center, radius }, HandleKind::ResizeEnd) => {
                            *radius = center.dist(to).max(0.5);
                        }
                        (CadItem::Polyline { points, .. }, HandleKind::Reshape(i)) => {
                            if let Some(v) = points.get_mut(i) {
                                *v = to;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Op::Group => {
                let items = cx.selection.items.clone();
                move_group_ex(cx, &items, behaviors::group_delta(cx, total, alt), !a.copy);
            }
            Op::GroupRotate(center) => {
                let mut angle = (p.world - center).angle() - (a.start - center).angle();
                if !alt {
                    let inc = cx.defaults.grid.angle_snap_deg.max(1.0).to_radians();
                    angle = (angle / inc).round() * inc;
                }
                let items = cx.selection.items.clone();
                transform::apply_xform(
                    cx,
                    &items,
                    &plan_core::transform::Xform::rotate(center, angle),
                );
            }
            Op::Swing(_) => {}
            Op::Stair(id, kind) => {
                if let Some(orig) = stairs_view::find(&a.original.floors[fl], id) {
                    let to = match kind {
                        StairHandleKind::Rotate | StairHandleKind::Run => {
                            cx.snap_at(p.world, Some(orig.bottom_center()), alt, &[])
                                .point
                        }
                        _ => p.world,
                    };
                    let mut n = stairs_view::drag_handle(&orig, kind, a.start, to);
                    if kind == StairHandleKind::Move && !alt {
                        n.stair.origin = snap_to_grid(n.stair.origin, unit(cx));
                    }
                    stairs_view::update(&mut cx.project, fl, id, |o| *o = n);
                }
            }
            Op::Cabinet(id, kind) => {
                if let Some(orig) = placed::cabinet_by_id(&a.original.floors[fl], id) {
                    use crate::tools::cabinet::{apply_edit_mode, bump_mode, BumpMode};
                    if kind == HandleKind::Move && bump_mode() == BumpMode::Push {
                        // Push (Edit > Neighbors): a run the cabinet meets is
                        // pushed ahead of it; every step starts from the
                        // cabinets as they were when the drag began.
                        let before = placed::load_cabinets(&a.original.floors[fl]);
                        let now = placed::load_cabinets(&cx.project.floors[fl]);
                        for o in &before {
                            if now.iter().find(|c| c.id == o.id) != Some(o) {
                                placed::replace_cabinet(&mut cx.project, fl, o);
                            }
                        }
                        let e = apply_edit_mode(
                            cx,
                            BumpMode::Push,
                            kind,
                            &orig,
                            a.start,
                            p,
                            Some(before.as_slice()),
                        );
                        placed::replace_cabinet(&mut cx.project, fl, &e.cab);
                        for q in &e.pushed {
                            placed::replace_cabinet(&mut cx.project, fl, q);
                        }
                    } else {
                        let c = crate::tools::cabinet::apply_edit(cx, kind, &orig, a.start, p);
                        placed::replace_cabinet(&mut cx.project, fl, &c);
                    }
                }
            }
            Op::Symbol(id, kind) => {
                if let Some(orig) = a.original.floors[fl].symbol(id).cloned() {
                    let s = crate::tools::library::apply_drag(cx, kind, &orig, a.start, p);
                    if let Some(slot) = cx.project.floors[fl]
                        .symbols
                        .iter_mut()
                        .find(|x| x.id == id)
                    {
                        *slot = s;
                    }
                }
            }
            Op::DeviceMove(id) => {
                let unit = unit(cx);
                let mut layer = site_view::load_electrical(&a.original.floors[fl]);
                if let Some(d) = layer.device_mut(id) {
                    match d.wall_id.and_then(|w| cx.floor().wall(w)).cloned() {
                        Some(w) => site_view::slide_on_wall(d, &w, p.world, unit),
                        None => {
                            d.position = Point::new(
                                snap_unit_round(d.position.x + total.x, unit),
                                snap_unit_round(d.position.y + total.y, unit),
                            );
                        }
                    }
                }
                site_view::save_electrical(&mut cx.project, fl, &layer);
            }
            Op::RoofMove(id) => {
                let unit = unit(cx);
                let mut set = roof_view::load(&a.original.floors[fl]);
                let d = Point::new(
                    snap_unit_round(total.x, unit),
                    snap_unit_round(total.y, unit),
                );
                if roof_view::translate_record(&mut set, id, d) {
                    roof_view::store(&mut cx.project, fl, &mut set);
                }
            }
            Op::RoofHandle(id, h) => {
                // Corners follow the snapped pointer; the pitch arrow, edge
                // and rotate handles work from the raw one.
                let to = if matches!(h, roof_view::PlaneHandle::Vertex(_)) {
                    cx.snap_at(p.world, None, alt, &[]).point
                } else {
                    p.world
                };
                if let Some(d) =
                    roof_view::apply_handle_drag(&mut cx.project, fl, id, h, a.start, to)
                {
                    cx.readout = Some(d.readout);
                }
            }
            Op::CameraWedge(id, w) => {
                if let Some(orig) = a.original.camera(id).cloned() {
                    cx.project.update_camera(id, |c| {
                        *c = orig.clone();
                        camera_tool::apply_wedge(c, w, p.world);
                    });
                }
            }
            Op::FoundationVertex(id, i) => {
                let to = cx.snap_at(p.world, None, alt, &[]).point;
                let mut layer = FoundationLayer::load(&a.original.floors[fl]);
                if let Some(r) = layer.find(id) {
                    if foundation_view::move_vertex_in(&mut layer, r, i, to) {
                        foundation_view::save(&mut cx.project, fl, &layer);
                    }
                }
            }
            Op::FramingEnd(id, at_end) => {
                let other = framing_view::find(&a.original.floors[fl], id)
                    .and_then(|r| r.line_ends())
                    .map(|(s, e)| if at_end { s } else { e });
                if let Some(fixed) = other {
                    let to = cx.snap_at(p.world, Some(fixed), alt, &[]).point;
                    if to.dist(fixed) >= framing_view::MIN_MEMBER {
                        framing_view::move_end_in(&mut cx.project.floors[fl], id, at_end, to);
                    }
                }
            }
            Op::FramingVertex(id, i) => {
                let to = cx.snap_at(p.world, None, alt, &[]).point;
                framing_view::move_vertex_in(&mut cx.project.floors[fl], id, i, to);
            }
            Op::DetailVertex(id, i) => {
                let mut layer = DetailsLayer::load(&a.original.floors[fl]);
                if let Some(r) = layer.find(id) {
                    // A molding line's end snaps from the other end.
                    let from = (i < 2)
                        .then(|| layer.vertices(r))
                        .flatten()
                        .filter(|v| v.len() == 2)
                        .map(|v| v[1 - i]);
                    let to = cx.snap_at(p.world, from, alt, &[]).point;
                    let too_short = from.is_some_and(|f| f.dist(to) < 1.0);
                    if !too_short && details_view::move_vertex_in(&mut layer, r, i, to) {
                        details_view::save(&mut cx.project, fl, &layer);
                    }
                }
            }
            Op::TerrainVertex(hit, i) => {
                let to = cx.snap_at(p.world, None, alt, &[]).point;
                if let Some(mut rec) = site_view::load_terrain(&a.original) {
                    if site_view::move_terrain_vertex(&mut rec.terrain, hit, i, to) {
                        site_view::save_terrain(&mut cx.project, &rec);
                    }
                }
            }
            Op::Camera(id, h) => {
                if let Some(orig) = a.original.camera(id).cloned() {
                    let unit = unit(cx);
                    let to = if h == CamHandle::Move {
                        Point::new(
                            snap_unit_round(orig.position.x + total.x, unit),
                            snap_unit_round(orig.position.y + total.y, unit),
                        )
                    } else {
                        p.world
                    };
                    cx.project.update_camera(id, |c| {
                        *c = orig.clone();
                        camera_tool::apply_handle_with(
                            c,
                            h,
                            to,
                            p.modifiers
                                .shift
                                .then_some(cx.defaults.editing.angle_snap_deg),
                        );
                    });
                }
            }
        }
        if matches!(a.op, Op::WallMove(_) | Op::WallEnd(..) | Op::Group) {
            details_view::follow_walls(&mut cx.project, fl, &a.original.floors[fl].walls);
        }
        // A moved distribution record carries its copies along.
        if matches!(a.op, Op::Group | Op::Symbol(..)) {
            crate::editor::placed::sync_distributions(cx);
        }
        cx.mark_dirty();
    }

    fn start_drag(&mut self, cx: &mut EditorContext, op: Op, start: Point, copy: bool) {
        let mut op = op;
        let mut exclude = Vec::new();
        if let Op::WallEnd(id, end) = op {
            exclude.push(id);
            if let Some(w) = cx.floor().wall(id) {
                let at = if end == WallEnd::Start {
                    w.start
                } else {
                    w.end
                };
                exclude.extend(
                    ops::walls_at(&cx.project, cx.floor, at, JOIN_TOL, Some(id))
                        .into_iter()
                        .map(|(i, _)| i),
                );
            }
        }
        // Ctrl/Cmd: the copies are made now and are what the drag carries
        // (S-94); one undo step holds the copy and the move.
        let clip = (copy && is_body_move(op))
            .then(|| Clipboard::capture(cx))
            .filter(|c| !c.is_empty());
        let copy = clip.is_some();
        let label = if copy {
            "Copy Objects"
        } else if matches!(op, Op::Group) {
            behaviors::group_label(cx)
        } else {
            op.label()
        };
        cx.begin_change(label);
        let before_copy = copy.then(|| cx.project.clone());
        let prev_selection = cx.selection.items.clone();
        let mut copied = false;
        if let Some(clip) = clip {
            let made = clip.paste(cx, Point::ZERO, false);
            if !made.is_empty() {
                cx.selection.items = made;
                op = Op::Group;
                copied = true;
            }
        }
        match typed_kind(op) {
            TypedKind::Length => cx.typed_input.arm(),
            TypedKind::Angle => cx.typed_input.arm_angle(),
            TypedKind::No => {}
        }
        self.drag = Drag::Active(Box::new(Active {
            op,
            original: cx.project.clone(),
            start,
            exclude,
            copy: copied,
            before_copy: copied.then_some(before_copy).flatten(),
            prev_selection,
        }));
    }

    fn finish(&mut self, cx: &mut EditorContext, a: Active) -> ToolResult {
        let fl = cx.floor;
        cx.typed_input.disarm();
        cx.readout = None;
        // Dropping a wall end on the middle of another wall splits it there.
        if let Op::WallEnd(id, end) = a.op {
            if let Some(w) = cx.floor().wall(id) {
                let p = if end == WallEnd::Start {
                    w.start
                } else {
                    w.end
                };
                let mut skip = vec![id];
                skip.extend(
                    ops::walls_at(&cx.project, fl, p, JOIN_TOL, None)
                        .iter()
                        .map(|x| x.0),
                );
                ops::split_walls_at_point(&mut cx.project, fl, p, &skip);
            }
        }
        // Chief auto-connects a wall whose end or body was dragged near other
        // walls (W-31..W-36); run inside the drag's own undo step.
        if let Op::WallEnd(id, _) | Op::WallMove(id) = a.op {
            let rooms_before = wall_edit::room_count(cx);
            crate::editor::connect::auto_connect(cx, id);
            if matches!(a.op, Op::WallEnd(..)) {
                wall_edit::merge_collinear_at(cx, id);
            }
            wall_edit::auto_reverse_if_closed(cx, id, rooms_before);
            // Joining may have moved the wall again: trim follows it.
            details_view::follow_walls(&mut cx.project, fl, &a.original.floors[fl].walls);
        }
        // A moved distribution record carries its copies along.
        if matches!(a.op, Op::Group | Op::Symbol(..)) {
            crate::editor::placed::sync_distributions(cx);
        }
        // Moved or resized cabinets regenerate the countertop they join (CB-14).
        if matches!(a.op, Op::Group | Op::Cabinet(..)) {
            crate::editor::placed::rejoin_if_enabled(cx);
        }
        cx.last_snap = None;
        cx.mark_dirty();
        // A Replicate drag may hand its move to Transform/Replicate Object.
        if matches!(a.op, Op::Group) && !a.copy {
            behaviors::finish_group(cx);
        }
        let reference = a.before_copy.as_ref().unwrap_or(&a.original);
        if reference.to_json().ok() == cx.project.to_json().ok() {
            cx.cancel_change();
            return ToolResult::consumed();
        }
        if let Op::Camera(id, _) | Op::CameraWedge(id, _) = a.op {
            Outbox::global().post(ViewRequest::RefreshCamera(id));
        }
        ToolResult::committed(if a.copy {
            "Copy Objects"
        } else if matches!(a.op, Op::Group) {
            behaviors::group_label(cx)
        } else {
            a.op.label()
        })
    }

    fn cancel_drag(&mut self, cx: &mut EditorContext) -> bool {
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Active(a) => {
                set_dragging(false);
                // A Ctrl-drag goes back past the copies it made.
                if a.copy {
                    cx.selection.items = a.prev_selection;
                }
                cx.project = a.before_copy.unwrap_or(a.original);
                cx.cancel_change();
                cx.typed_input.disarm();
                cx.readout = None;
                cx.last_snap = None;
                cx.mark_dirty();
                true
            }
            Drag::None => false,
            _ => true,
        }
    }

    fn cycle(&mut self, cx: &mut EditorContext, backwards: bool) -> ToolResult {
        // With one door or window selected and no pointer over the plan, Tab
        // walks the stack under it like Select Next Object (manual p. 610).
        if cx.cursor_world.is_none()
            && matches!(cx.selection.items.as_slice(), [ObjectRef::Opening(_)])
        {
            crate::editor::opening_edit::select_next(cx);
            return ToolResult::consumed();
        }
        let Some(at) = cx.cursor_world else {
            return ToolResult::ignored();
        };
        let hits = hit_test_cx(cx, at, cx.pick_tol());
        // The Exterior Room comes after the objects under the pointer when
        // it is just outside an exterior wall (R-106, manual p. 449).
        match rooms_edit::exterior_cycle(cx, at, &hits, backwards) {
            rooms_edit::Next::Hit(o) => {
                select_hit(cx, o);
                ToolResult::consumed()
            }
            rooms_edit::Next::Exterior => ToolResult::consumed(),
            rooms_edit::Next::Nothing => ToolResult::ignored(),
        }
    }

    fn nudge(&mut self, cx: &mut EditorContext, dir: Point, big: bool) -> ToolResult {
        if cx.selection.is_empty() {
            return ToolResult::ignored();
        }
        let delta = dir * (cx.snap_unit() * if big { 10.0 } else { 1.0 });
        let items = cx.selection.items.clone();
        if items.iter().any(|o| !cx.check_unlocked(*o)) {
            return ToolResult::consumed();
        }
        // A group: a nudge that moves nothing (a terrain point, say) leaves no
        // undo step (QA-26).
        cx.undo_group(|cx| {
            cx.begin_change("Nudge");
            let before = cx.floor().walls.clone();
            match cx.selection.single() {
                Some(ObjectRef::Wall(id)) => move_wall_perpendicular(cx, id, delta),
                Some(ObjectRef::Opening(id)) => slide_opening_by(cx, id, delta),
                _ => move_group(cx, &items, delta),
            }
            let fl = cx.floor;
            details_view::follow_walls(&mut cx.project, fl, &before);
            // A nudged distribution record carries its copies along.
            crate::editor::placed::sync_distributions(cx);
        });
        cx.mark_dirty();
        ToolResult::committed("Nudge")
    }

    /// Typed length and angle while a wall end is dragged (W-15, W-16): the
    /// drag follows the typed values, Enter drops the end there.
    fn typed_key(&mut self, cx: &mut EditorContext, k: &KeyEvent) -> Option<ToolResult> {
        use crate::editor::typed_input::TypedKey;
        let Drag::Active(a) = &self.drag else {
            return None;
        };
        if typed_kind(a.op) == TypedKind::No || !cx.typed_input.is_armed() {
            return None;
        }
        let res = cx.typed_input.handle(k.key, k.text.as_deref());
        if res == TypedKey::Ignored {
            return None;
        }
        if let Drag::Active(a) = std::mem::replace(&mut self.drag, Drag::None) {
            let at = cx.cursor_world.unwrap_or(a.start);
            let p = PointerEvent::at(cx, at)
                .with_modifiers(k.modifiers)
                .with_down(true);
            self.apply(cx, &a, &p);
            if res == TypedKey::Commit {
                return Some(self.finish(cx, *a));
            }
            self.drag = Drag::Active(a);
        }
        Some(ToolResult::consumed())
    }

    fn key_while_editing(&mut self, cx: &mut EditorContext, k: &KeyEvent) -> ToolResult {
        if let Some(t) = &k.text {
            let ok: String = t
                .chars()
                .filter(|c| c.is_ascii_digit() || " '\"-/.".contains(*c))
                .collect();
            cx.temp.type_text(&ok);
        } else if k.is(Key::Backspace) {
            cx.temp.backspace();
        } else if k.is(Key::Tab) {
            cx.temp.next_field();
        } else if k.is(Key::Escape) {
            cx.temp.cancel();
        } else if k.is(Key::Enter) {
            match tempdim::commit_edit(cx) {
                Ok(label) => return ToolResult::committed(label),
                Err(e) => cx.status = e,
            }
        }
        ToolResult::consumed()
    }
}

impl Tool for SelectTool {
    fn id(&self) -> ToolId {
        ToolId::Select
    }

    /// While a move or a wall end is dragged: where it began, so Tab or
    /// Enter can ask for the new location (manual p. 197).
    fn coordinate_origin(&self, cx: &EditorContext) -> Option<Point> {
        let Drag::Active(a) = &self.drag else {
            return None;
        };
        if !cx.typed_input.is_armed() {
            return None;
        }
        match a.op {
            Op::WallEnd(id, end) => a.original.floors[cx.floor].wall(id).map(|w| {
                if end == WallEnd::Start {
                    w.end
                } else {
                    w.start
                }
            }),
            Op::Group | Op::WallMove(_) => Some(a.start),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        "Select Objects"
    }

    fn hint(&self) -> String {
        "Select: click an object; drag to move or marquee (Alt starts a marquee over objects, Ctrl/Cmd copies); Tab cycles; Delete removes".into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        self.cursor
    }

    fn frame(&mut self, cx: &mut EditorContext, _ctx: &egui::Context) {
        // A Copy made in another window or file shows up here (S-85).
        crate::editor::clipboard::poll_file(cx);
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.cancel_drag(cx);
        area::cancel(cx);
        transform::cancel_mode(cx);
        wall_edit::cancel_break();
        cx.temp.cancel();
        cx.hover = None;
        set_dragging(false);
        // An Edit Behavior lasts only while Select Objects is the tool (S-66).
        behaviors::reset(cx);
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        // The rubber band and the region of Edit Area / Stretch CAD.
        if let Some(res) = area::pointer_down(cx, &p) {
            return res;
        }
        // A hanging Paste, Point to Point Move, Reflect, Center or Make
        // Parallel takes the click.
        if let Some(res) = transform::mode_pointer_down(cx, &p) {
            return res;
        }
        // The Rotate handle of a multi-object selection (S-101).
        if let Some((center, handle)) = transform::group_rotate_handle(cx) {
            if p.world.dist(handle) <= cx.pick_tol() * 1.5 {
                self.drag = Drag::Armed {
                    op: Op::GroupRotate(center),
                    start: p.world,
                    screen: p.screen,
                };
                return ToolResult::consumed();
            }
        }
        // Break Wall waits for the click that sets the break point (W-43).
        if wall_edit::break_pending() {
            return if wall_edit::break_click(cx, p.world) {
                ToolResult::committed("Break Wall")
            } else {
                ToolResult::consumed()
            };
        }
        // Space Planning boxes sit on top of the plan and are dragged first.
        if rooms_edit::space_pointer_down(cx, p.world) {
            return ToolResult::consumed();
        }
        // A room label picks its room and drags (R-44).
        if rooms_edit::label_pointer_down(cx, p.world) {
            return ToolResult::consumed();
        }
        // An edge grip of the selected Exterior Room sets the level's default
        // heights (R-106).
        if rooms_edit::exterior_drag_down(cx, p.world) {
            return ToolResult::consumed();
        }
        self.room_click = None;
        let tol = cx.pick_tol();
        // The depth handle of a bay window can stand where the temporary
        // width reads; the handle of the selected object wins.
        if let Some(op @ Op::BayDepth(_)) = Self::handle_op(cx, p.world, tol) {
            cx.temp.cancel();
            self.drag = Drag::Armed {
                op,
                start: p.world,
                screen: p.screen,
            };
            return ToolResult::consumed();
        }
        // The padlock beside a measured value locks it into a permanent
        // dimension (S-63).
        if let Some(i) = cx.temp.hit_lock(p.world, cx.px_per_in) {
            cx.temp.cancel();
            return match tempdim::toggle_lock(cx, i) {
                Ok(label) => ToolResult::committed(label),
                Err(e) => {
                    cx.status = e;
                    ToolResult::consumed()
                }
            };
        }
        if let Some(i) = cx.temp.hit_label(p.world, cx.px_per_in) {
            cx.temp.cancel();
            cx.temp.begin_edit(i);
            return ToolResult::consumed();
        }
        cx.temp.cancel();
        let shift = p.modifiers.shift;
        // An edit handle of a selected schedule (side Resize, Rotate, Resize
        // Column, Move Row, Move Column, Sort by Column, Wrap).
        if let Some(res) = crate::tools::schedule::handle_press(cx, p.world) {
            return res;
        }
        if let Some(op) = Self::handle_op(cx, p.world, tol) {
            if let Op::Camera(id, CamHandle::Break(i)) = op {
                crate::editor::camera_edit::select_break(id, i);
            }
            self.drag = Drag::Armed {
                op,
                start: p.world,
                screen: p.screen,
            };
            return ToolResult::consumed();
        }
        let hits = hit_test_cx(cx, p.world, tol);
        // A room on top means empty floor: the click selects the room.
        let top_hit = hits
            .first()
            .copied()
            .filter(|o| !matches!(o, ObjectRef::Room(_)));
        // Ctrl/Cmd on the press marquees from on top of an object and toggles
        // the objects it covers (S-30, manual p. 260).
        let marquee_over = p.overrides() && !p.modifiers.alt;
        let top_hit = top_hit.filter(|_| !marquee_over);
        if let Some(top) = top_hit {
            rooms_edit::clear_room_selection();
            // A click on a schedule also picks the row it falls on.
            if let ObjectRef::Schedule(sid) = top {
                crate::editor::schedule_view::note_click(cx, sid, p.world);
            }
            // A click on a group member names the whole group (S-35).
            let members = expand_groups(cx, &[top]);
            if shift {
                if cx.selection.contains(top) {
                    cx.selection.items.retain(|o| !members.contains(o));
                } else {
                    for m in members {
                        cx.selection.add(m);
                    }
                }
                return ToolResult::consumed();
            }
            if !cx.selection.contains(top) {
                cx.selection.items = members;
            }
            let op = if cx.selection.len() == 1 {
                single_op_for_body(top)
            } else {
                Op::Group
            };
            self.drag = Drag::Armed {
                op,
                start: p.world,
                screen: p.screen,
            };
            return ToolResult::consumed();
        }
        if !shift && !marquee_over {
            cx.selection.clear();
            rooms_edit::clear_room_selection();
        }
        self.marquee_toggle = marquee_over && !shift;
        self.room_click = rooms_edit::room_index_at(cx, p.world);
        self.drag = Drag::Marquee {
            start: p.world,
            current: p.world,
            screen: p.screen,
            add: shift || marquee_over,
        };
        ToolResult::consumed()
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        behaviors::summon(&p);
        if area::pointer_move(cx, &p) {
            return ToolResult::consumed();
        }
        transform::mode_pointer_move(&p);
        if p.down && rooms_edit::space_dragging() {
            rooms_edit::space_pointer_move(p.world);
            return ToolResult::consumed();
        }
        if p.down && rooms_edit::label_dragging() {
            rooms_edit::label_pointer_move(cx, p.world);
            return ToolResult::consumed();
        }
        if p.down && rooms_edit::exterior_dragging() {
            rooms_edit::exterior_drag_move(cx, p.world);
            return ToolResult::consumed();
        }
        if p.down && crate::tools::schedule::handle_move(cx, p.world) {
            return ToolResult::consumed();
        }
        if !p.down {
            self.update_hover(cx, &p);
            return ToolResult {
                repaint: true,
                ..ToolResult::default()
            };
        }
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Armed { op, start, screen } => {
                if (p.screen - screen).length() < DRAG_THRESHOLD_PX || matches!(op, Op::Swing(_)) {
                    self.drag = Drag::Armed { op, start, screen };
                    return ToolResult::consumed();
                }
                // Locked layers refuse the edit (S-5).
                let items: Vec<ObjectRef> = cx.selection.items.clone();
                if items.iter().any(|o| !cx.check_unlocked(*o)) {
                    return ToolResult::consumed();
                }
                // Ctrl/Cmd alone overrides (manual p. 237); with Alt too, the
                // drag copies (S-94).
                let copy = p.overrides() && p.modifiers.alt;
                self.start_drag(cx, op, start, copy);
                if let Drag::Active(a) = std::mem::replace(&mut self.drag, Drag::None) {
                    self.apply(cx, &a, &p);
                    self.drag = Drag::Active(a);
                }
            }
            Drag::Active(a) => {
                self.apply(cx, &a, &p);
                self.drag = Drag::Active(a);
            }
            Drag::Marquee {
                start, screen, add, ..
            } => {
                self.drag = Drag::Marquee {
                    start,
                    current: p.world,
                    screen,
                    add,
                };
            }
            Drag::None => {}
        }
        // The view scrolls while the pointer sits at the canvas edge (S-99).
        set_dragging(match &self.drag {
            Drag::Active(_) => true,
            Drag::Marquee { screen, .. } => (p.screen - *screen).length() >= DRAG_THRESHOLD_PX,
            _ => false,
        });
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        set_dragging(false);
        if let Some(res) = area::pointer_up(cx, &p) {
            return res;
        }
        if rooms_edit::space_pointer_up() {
            return ToolResult::consumed();
        }
        if rooms_edit::label_pointer_up() {
            return ToolResult::consumed();
        }
        match rooms_edit::exterior_drag_up(cx) {
            Some(true) => return ToolResult::committed("Exterior Room"),
            Some(false) => return ToolResult::consumed(),
            None => {}
        }
        if let Some(res) = crate::tools::schedule::handle_release(cx, p.world) {
            return res;
        }
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Active(a) => self.finish(cx, *a),
            Drag::Armed {
                op: Op::Swing(id), ..
            } => {
                // Click reverses the swing; Shift-click moves the hinge (DW-33).
                let hinge = p.modifiers.shift;
                if cx.check_unlocked(ObjectRef::Opening(id)) {
                    cx.selection.set(ObjectRef::Opening(id));
                    if hinge {
                        cx.begin_change("Flip Hinge");
                        let fl = cx.floor;
                        cx.project.flip_hinge(fl, id);
                        cx.mark_dirty();
                    } else {
                        cx.reverse_swing();
                    }
                }
                ToolResult::committed(if hinge { "Flip Hinge" } else { "Reverse Swing" })
            }
            Drag::Marquee {
                start, screen, add, ..
            } => {
                if (p.screen - screen).length() >= DRAG_THRESHOLD_PX {
                    let found = objects_in_rect(cx, start, p.world);
                    if self.marquee_toggle {
                        // Ctrl/Cmd: what is selected leaves, what is not joins.
                        for o in found {
                            if cx.selection.contains(o) {
                                cx.selection.items.retain(|x| *x != o);
                            } else {
                                cx.selection.add(o);
                            }
                        }
                    } else {
                        if !add {
                            cx.selection.clear();
                        }
                        for o in found {
                            cx.selection.add(o);
                        }
                    }
                } else if let Some(room) = self.room_click.take() {
                    // A plain click on empty floor selects the room.
                    rooms_edit::select_room(cx, room);
                } else if let Some(ext) = rooms_edit::exterior_at(cx, p.world) {
                    // Just outside an exterior wall: the Exterior Room (R-106).
                    rooms_edit::select_exterior(cx, ext);
                }
                ToolResult::consumed()
            }
            _ => ToolResult::ignored(),
        }
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        self.drag = Drag::None;
        let hits = hit_test_cx(cx, p.world, cx.pick_tol());
        let top = hits
            .first()
            .copied()
            .filter(|o| !matches!(o, ObjectRef::Room(_)));
        match top {
            Some(o) => {
                select_hit(cx, o);
                cx.requests.push(EditorRequest::OpenSpec(o));
                ToolResult::consumed()
            }
            None => match rooms_edit::room_index_at(cx, p.world) {
                // Double-click inside a room opens the Room Specification (R-19).
                Some(room) => {
                    rooms_edit::select_room(cx, room);
                    rooms_edit::request_room_dialog(cx, room);
                    ToolResult::consumed()
                }
                // Just outside an exterior wall: the Exterior Room's.
                None => match rooms_edit::exterior_at(cx, p.world) {
                    Some(ext) => {
                        rooms_edit::select_exterior(cx, ext);
                        rooms_edit::request_exterior_dialog(cx, ext);
                        ToolResult::consumed()
                    }
                    None => ToolResult::ignored(),
                },
            },
        }
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if let Some(res) = area::key(cx, &k) {
            return res;
        }
        if cx.temp.editing.is_some() {
            return self.key_while_editing(cx, &k);
        }
        if let Some(r) = self.typed_key(cx, &k) {
            return r;
        }
        // Esc ends a hanging Paste or a click-driven edit mode.
        if k.is(Key::Escape) && transform::mode_escape(cx) {
            return ToolResult::consumed();
        }
        if k.is(Key::Escape) && wall_edit::cancel_break() {
            cx.status.clear();
            return ToolResult::consumed();
        }
        if k.is(Key::Escape) {
            if self.cancel_drag(cx) {
                return ToolResult::consumed();
            }
            if rooms_edit::selected_room(cx).is_some()
                || rooms_edit::selected_exterior(cx).is_some()
            {
                rooms_edit::clear_room_selection();
                return ToolResult::consumed();
            }
            if !cx.selection.is_empty() {
                cx.selection.clear();
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        if k.is(Key::Delete) || k.is(Key::Backspace) {
            if cx.selection.is_empty() {
                return ToolResult::ignored();
            }
            cx.delete_selection();
            return ToolResult::committed("Delete");
        }
        if k.is(Key::Enter) {
            if let (true, Some(room)) = (cx.selection.is_empty(), rooms_edit::selected_room(cx)) {
                rooms_edit::request_room_dialog(cx, room);
                return ToolResult::consumed();
            }
            if let (true, Some(ext)) = (cx.selection.is_empty(), rooms_edit::selected_exterior(cx))
            {
                rooms_edit::request_exterior_dialog(cx, ext);
                return ToolResult::consumed();
            }
            return match cx.selection.single() {
                Some(o) => {
                    cx.requests.push(EditorRequest::OpenSpec(o));
                    ToolResult::consumed()
                }
                None => ToolResult::ignored(),
            };
        }
        if k.is(Key::Tab) {
            return self.cycle(cx, k.modifiers.shift);
        }
        let big = k.modifiers.shift;
        match k.key {
            Some(Key::ArrowLeft) => self.nudge(cx, Point::new(-1.0, 0.0), big),
            Some(Key::ArrowRight) => self.nudge(cx, Point::new(1.0, 0.0), big),
            Some(Key::ArrowUp) => self.nudge(cx, Point::new(0.0, 1.0), big),
            Some(Key::ArrowDown) => self.nudge(cx, Point::new(0.0, -1.0), big),
            _ => ToolResult::ignored(),
        }
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        if let Drag::Marquee { start, current, .. } = &self.drag {
            let r = Rect::from_two_pos(cam.world_to_screen(*start), cam.world_to_screen(*current));
            let crossing = marquee_mode().crosses(*start, *current);
            let col = if crossing { pal.hover } else { pal.selection };
            painter.add(Shape::rect_filled(r, 0.0, col.gamma_multiply(0.12)));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, col), egui::StrokeKind::Inside);
        }
        if cx.view_flags.contains(&ViewFlag::TemporaryDimensions) {
            tempdim::draw(&cx.temp, painter, cam, pal, &cx.dim_format());
        }
        draw_terrain_selection(cx, painter, cam);
        draw_host_walls(cx, painter, cam);
        draw_arc_handles(cx, painter, cam);
        area::draw_overlay(cx, painter, cam);
        transform::draw_mode_overlay(cx, painter, cam);
        if !transform::mode_active() {
            transform::draw_group_rotate_handle(cx, painter, cam);
        }
        let hs = handles::handles_for(cx, cam.px_per_in);
        handles::draw(&hs, painter, cam, pal);
        if let (Drag::Active(_), Some(s)) = (&self.drag, cx.last_snap) {
            crate::editor::render::draw_snap_marker(painter, cam, &s, pal.ghost_stroke);
        }
        // The number being typed into the drag, at the pointer (S-28).
        if let (Drag::Active(_), true, Some(at), Some(text)) = (
            &self.drag,
            cx.typed_input.has_text(),
            cx.cursor_world,
            cx.readout.as_ref(),
        ) {
            let pos = cam.world_to_screen(at) + egui::vec2(14.0, -18.0);
            painter.text(
                pos,
                egui::Align2::LEFT_BOTTOM,
                text,
                egui::FontId::proportional(13.0),
                pal.selection,
            );
        }
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction> {
        let mut v = cx.common_edit_actions();
        v.extend(cx.extra_edit_actions());
        let floor = cx.floor();
        let has_door = cx.selection.items.iter().any(|o| match o {
            ObjectRef::Opening(id) => floor
                .openings
                .iter()
                .any(|x| x.id == *id && x.kind == OpeningKind::Door),
            _ => false,
        });
        if has_door {
            v.push(EditAction::new(EditActionKind::ReverseSwing));
        }
        if cx
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Wall(_)))
        {
            v.push(EditAction::new(EditActionKind::FixWallConnections));
        }
        if crate::editor::placed::selection_has_closed_polyline(cx) {
            v.push(EditAction::new(EditActionKind::Custom {
                id: crate::editor::placed::SOFFIT_FROM_POLYLINE,
                label: "Convert Polyline to Soffit",
                icon: "",
            }));
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::WallKind;

    fn room() -> (EditorContext, [Id; 4]) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(120.0, 96.0),
            Point::new(0.0, 96.0),
        ];
        let mut ids = [0; 4];
        for i in 0..4 {
            ids[i] = cx
                .project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 100.0, WallKind::Exterior);
        }
        cx.refresh();
        (cx, ids)
    }

    fn ev(cx: &EditorContext, x: f64, y: f64) -> PointerEvent {
        PointerEvent::at(cx, Point::new(x, y))
    }

    fn drag(t: &mut SelectTool, cx: &mut EditorContext, from: (f64, f64), to: (f64, f64)) {
        let a = ev(cx, from.0, from.1);
        t.pointer_move(cx, a);
        t.pointer_down(cx, a.with_down(true));
        let b = ev(cx, to.0, to.1).with_down(true);
        t.pointer_move(cx, b);
        t.pointer_up(cx, b);
    }

    #[test]
    fn click_selects_and_empty_click_deselects() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        let p = ev(&cx, 60.0, 1.0);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(ids[0])));
        let q = ev(&cx, 60.0, 48.0);
        t.pointer_down(&mut cx, q.with_down(true));
        t.pointer_up(&mut cx, q);
        assert!(cx.selection.is_empty());
    }

    #[test]
    fn perpendicular_drag_moves_a_wall_and_keeps_neighbours_attached() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        // Drag the top wall down 20", sideways drift must be ignored.
        drag(&mut t, &mut cx, (60.0, 96.0), (75.0, 76.0));
        let f = cx.floor();
        let top = f.wall(ids[2]).unwrap();
        assert_eq!(
            (top.start, top.end),
            (Point::new(120.0, 76.0), Point::new(0.0, 76.0))
        );
        assert_eq!(f.wall(ids[1]).unwrap().end, Point::new(120.0, 76.0));
        assert_eq!(f.wall(ids[3]).unwrap().start, Point::new(0.0, 76.0));
        cx.refresh();
        assert_eq!(cx.rooms.len(), 1);
        // One undo step restores everything.
        assert_eq!(cx.undo().as_deref(), Some("Move Wall"));
        assert_eq!(
            cx.floor().wall(ids[2]).unwrap().start,
            Point::new(120.0, 96.0)
        );
        assert_eq!(
            cx.floor().wall(ids[1]).unwrap().end,
            Point::new(120.0, 96.0)
        );
        assert!(!cx.can_undo());
    }

    #[test]
    fn escape_cancels_a_drag() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        let a = ev(&cx, 60.0, 96.0);
        t.pointer_down(&mut cx, a.with_down(true));
        let b = ev(&cx, 60.0, 60.0).with_down(true);
        t.pointer_move(&mut cx, b);
        assert_ne!(cx.floor().wall(ids[2]).unwrap().start.y, 96.0);
        t.key(&mut cx, KeyEvent::escape());
        assert_eq!(cx.floor().wall(ids[2]).unwrap().start.y, 96.0);
        assert!(!cx.can_undo());
    }

    #[test]
    fn end_handle_stretches_with_the_connected_wall_and_splits_on_drop() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        // Select the bottom wall, then drag its end handle up the right wall.
        let p = ev(&cx, 30.0, 1.0);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        // Alt suspends the 15 degree angle snap so the end lands exactly.
        let alt = eframe::egui::Modifiers {
            ctrl: true,
            ..eframe::egui::Modifiers::NONE
        };
        let a = ev(&cx, 120.0, 0.0);
        t.pointer_down(&mut cx, a.with_down(true));
        let b = ev(&cx, 120.0, 40.0).with_down(true).with_modifiers(alt);
        t.pointer_move(&mut cx, b);
        t.pointer_up(&mut cx, b);
        // The bottom wall's end moved; the right wall's start followed.
        assert_eq!(
            cx.floor().wall(ids[0]).unwrap().end,
            Point::new(120.0, 40.0)
        );
        assert_eq!(
            cx.floor().wall(ids[1]).unwrap().start,
            Point::new(120.0, 40.0)
        );

        // Dropping an end in the middle of another wall splits that wall.
        let (mut cx, ids) = room();
        let inner = cx.project.add_wall(
            0,
            Point::new(60.0, 40.0),
            Point::new(60.0, 70.0),
            4.5,
            100.0,
            WallKind::Interior,
        );
        cx.selection.set(ObjectRef::Wall(inner));
        drag(&mut t, &mut cx, (60.0, 70.0), (60.0, 96.0));
        let f = cx.floor();
        assert_eq!(f.walls.len(), 6);
        assert_eq!(f.wall(ids[2]).unwrap().end, Point::new(60.0, 96.0));
    }

    #[test]
    fn dragging_an_opening_slides_it() {
        let (mut cx, ids) = room();
        let o = cx
            .project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        let mut t = SelectTool::default();
        drag(&mut t, &mut cx, (60.0, 0.0), (80.4, 2.0));
        assert_eq!(cx.selection.single(), Some(ObjectRef::Opening(o)));
        let op = cx.floor().openings.iter().find(|x| x.id == o).unwrap();
        assert_eq!(op.center_offset, 80.0);
    }

    #[test]
    fn a_jamb_drag_lands_on_a_standard_width_when_snapping_is_on() {
        let (mut cx, ids) = room();
        let o = cx
            .project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        let span = |cx: &EditorContext| {
            let o = cx.floor().openings.iter().find(|x| x.id == o).unwrap();
            (o.start_offset(), o.end_offset())
        };
        let mut t = SelectTool::default();
        // Spans 42..78 (36 wide). Off: the start jamb follows the grid.
        cx.selection.set(ObjectRef::Opening(o));
        drag(&mut t, &mut cx, (42.0, 0.0), (53.0, 0.0));
        assert_eq!(span(&cx), (53.0, 78.0));
        cx.undo();
        // On: 78 - 53 = 25" snaps to the 24" hinged-door width.
        cx.defaults.opening_variants.widths.snap = true;
        cx.selection.set(ObjectRef::Opening(o));
        drag(&mut t, &mut cx, (42.0, 0.0), (53.0, 0.0));
        assert_eq!(span(&cx), (54.0, 78.0));
        cx.undo();
        // Windows use the window list: a 36" window dragged to 44" wide
        // lands on the 48" size.
        let win = cx
            .project
            .add_opening(0, ids[2], 60.0, OpeningKind::Window)
            .unwrap();
        let width = |cx: &EditorContext| {
            cx.floor()
                .openings
                .iter()
                .find(|x| x.id == win)
                .unwrap()
                .width
        };
        assert_eq!(width(&cx), 36.0);
        cx.selection.set(ObjectRef::Opening(win));
        // The wall runs the other way; drag the end jamb along it.
        let w = cx.floor().wall(ids[2]).unwrap().clone();
        let hs = handles::handles_for(&cx, cx.px_per_in);
        let end = hs
            .iter()
            .find(|h| h.kind == HandleKind::ResizeEnd)
            .unwrap()
            .pos;
        let along = w.direction();
        let to = Point::new(end.x + along.x * 8.0, end.y + along.y * 8.0);
        drag(&mut t, &mut cx, (end.x, end.y), (to.x, to.y));
        assert_eq!(width(&cx), 48.0);
        cx.undo();
        cx.defaults.opening_variants.widths.snap = false;
        cx.selection.set(ObjectRef::Opening(win));
        drag(&mut t, &mut cx, (end.x, end.y), (to.x, to.y));
        assert_eq!(width(&cx), 44.0);
    }

    #[test]
    fn the_label_handle_drags_the_label_as_one_undo_step() {
        let (mut cx, ids) = room();
        cx.project.ensure_opening_label_layers();
        let o = cx
            .project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        cx.refresh();
        cx.selection.set(ObjectRef::Opening(o));
        let label_at = |cx: &EditorContext| {
            handles::handles_for(cx, cx.px_per_in)
                .into_iter()
                .find(|h| h.kind == HandleKind::Label)
                .map(|h| h.pos)
        };
        let home = label_at(&cx).expect("a label handle while the label is shown");
        let mut t = SelectTool::default();
        drag(
            &mut t,
            &mut cx,
            (home.x, home.y),
            (home.x + 18.0, home.y - 7.0),
        );
        assert_eq!(cx.undo_label(), Some("Move Opening Label"));
        cx.refresh();
        let there = label_at(&cx).unwrap();
        assert!(
            (there.x - (home.x + 18.0)).abs() < 1e-6 && (there.y - (home.y - 7.0)).abs() < 1e-6
        );
        let off = cx.floor().openings[0].extras.spec.label_offset;
        assert!(off.0.abs() > 1.0 || off.1.abs() > 1.0);
        // The opening itself did not move.
        assert_eq!(cx.floor().openings[0].center_offset, 60.0);
        cx.undo();
        assert_eq!(cx.floor().openings[0].extras.spec.label_offset, (0.0, 0.0));
        // No handle while the label layer is hidden.
        cx.project.layers.set_display("Doors, Labels", false);
        cx.mark_dirty();
        cx.refresh();
        assert!(label_at(&cx).is_none());
    }

    #[test]
    fn marquee_window_and_crossing() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        // Left to right around the bottom wall only.
        drag(&mut t, &mut cx, (-20.0, -20.0), (140.0, 15.0));
        assert_eq!(cx.selection.items, vec![ObjectRef::Wall(ids[0])]);
        // Right to left touching bottom, right and left walls.
        drag(&mut t, &mut cx, (140.0, 15.0), (-20.0, -20.0));
        assert_eq!(cx.selection.len(), 3);
    }

    #[test]
    fn delete_removes_the_selection_with_its_openings_and_undoes() {
        let (mut cx, ids) = room();
        cx.project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        cx.selection.set(ObjectRef::Wall(ids[0]));
        let mut t = SelectTool::default();
        let r = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete"));
        assert_eq!(cx.floor().walls.len(), 3);
        assert!(cx.floor().openings.is_empty());
        cx.undo();
        assert_eq!(cx.floor().walls.len(), 4);
        assert_eq!(cx.floor().openings.len(), 1);
    }

    #[test]
    fn tab_cycles_through_the_objects_under_the_pointer() {
        let (mut cx, ids) = room();
        let o = cx
            .project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        cx.cursor_world = Some(Point::new(60.0, 0.0));
        let mut t = SelectTool::default();
        t.key(&mut cx, KeyEvent::key(Key::Tab));
        assert_eq!(cx.selection.single(), Some(ObjectRef::Opening(o)));
        t.key(&mut cx, KeyEvent::key(Key::Tab));
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(ids[0])));
    }

    #[test]
    fn typing_a_temporary_dimension_moves_the_wall() {
        let (mut cx, ids) = room();
        // A partition in the middle of the room, 40" from the bottom wall.
        let part = cx.project.add_wall(
            0,
            Point::new(10.0, 40.0),
            Point::new(110.0, 40.0),
            4.0,
            100.0,
            WallKind::Interior,
        );
        cx.selection.set(ObjectRef::Wall(part));
        cx.refresh();
        let mut t = SelectTool::default();
        let gap = cx
            .temp
            .dims
            .iter()
            .position(|d| d.value < 40.0 && d.kind == tempdim::TempDimKind::WallGap)
            .unwrap();
        // Click the value, type 3'-0" and press Enter.
        let at = cx.temp.dims[gap].label_pos(cx.px_per_in);
        let p = ev(&cx, at.x, at.y);
        t.pointer_down(&mut cx, p.with_down(true));
        assert!(cx.temp.editing.is_some());
        cx.temp.editing.as_mut().unwrap().text.clear();
        t.key(&mut cx, KeyEvent::text("3'"));
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Move Wall"));
        let y = cx.floor().wall(part).unwrap().start.y;
        // Gap to the bottom wall's face is now 36": 3 + 2 + 36 = 41.
        assert!((y - 41.0).abs() < 1e-9, "{y}");
        let _ = ids;
    }

    #[test]
    fn copy_and_paste_in_place_duplicates_walls_and_openings() {
        let (mut cx, ids) = room();
        cx.project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        cx.selection.set(ObjectRef::Wall(ids[0]));
        cx.apply_edit_action(EditActionKind::Copy);
        cx.apply_edit_action(EditActionKind::PasteInPlace);
        assert_eq!(cx.floor().walls.len(), 5);
        assert_eq!(cx.floor().openings.len(), 2);
        assert_eq!(cx.selection.len(), 1);
        cx.undo();
        assert_eq!(cx.floor().walls.len(), 4);
    }

    #[test]
    fn reverse_swing_is_offered_for_doors_only() {
        let (mut cx, ids) = room();
        let o = cx
            .project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        let t = SelectTool::default();
        cx.selection.set(ObjectRef::Opening(o));
        let kinds: Vec<_> = t.edit_toolbar(&cx).iter().map(|a| a.kind).collect();
        assert!(kinds.contains(&EditActionKind::ReverseSwing));
        cx.apply_edit_action(EditActionKind::ReverseSwing);
        assert!(cx.floor().openings[0].swing_flipped);
        cx.selection.set(ObjectRef::Wall(ids[0]));
        let kinds: Vec<_> = t.edit_toolbar(&cx).iter().map(|a| a.kind).collect();
        assert!(!kinds.contains(&EditActionKind::ReverseSwing));
        assert!(kinds.contains(&EditActionKind::FixWallConnections));
    }

    #[test]
    fn clicking_inside_a_room_selects_the_room_and_double_click_opens_it() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        let q = ev(&cx, 60.0, 48.0);
        t.pointer_down(&mut cx, q.with_down(true));
        t.pointer_up(&mut cx, q);
        assert!(cx.selection.is_empty());
        assert_eq!(rooms_edit::selected_room(&cx), Some(0));
        // Clicking an object takes the selection back from the room.
        let w = ev(&cx, 60.0, 1.0);
        t.pointer_down(&mut cx, w.with_down(true));
        t.pointer_up(&mut cx, w);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(ids[0])));
        assert_eq!(rooms_edit::selected_room(&cx), None);
        // Double-click inside the room asks for the Room Specification.
        let dc = ev(&cx, 60.0, 48.0);
        let r = t.double_click(&mut cx, dc);
        assert!(r.consumed);
        assert_eq!(rooms_edit::selected_room(&cx), Some(0));
        assert_eq!(rooms_edit::take_room_dialog_request(&cx), Some(0));
        // Esc drops the room selection.
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert_eq!(rooms_edit::selected_room(&cx), None);
        // Outside every room a double-click does nothing.
        let far = ev(&cx, 500.0, 500.0);
        assert!(!t.double_click(&mut cx, far).consumed);
    }

    #[test]
    fn dragging_a_space_planning_box_moves_it() {
        use plan_spaceplan::{generate_boxes, Questionnaire};
        let (mut cx, _) = room();
        let mut t = SelectTool::default();
        let boxes = generate_boxes(&Questionnaire::default());
        let first = boxes[0].clone();
        rooms_edit::set_space_boxes(boxes);
        let c = first.center();
        drag(&mut t, &mut cx, (c.x, c.y), (c.x + 240.0, c.y + 240.0));
        let moved = rooms_edit::space_boxes()
            .into_iter()
            .find(|b| b.id == first.id)
            .unwrap();
        assert_ne!(moved.rect, first.rect);
        assert!(cx.selection.is_empty());
        rooms_edit::clear_space_boxes();
    }

    #[test]
    fn select_picks_moves_and_deletes_a_pad_with_undo() {
        use crate::editor::foundation_view;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = foundation_view::add_pad(&mut cx, Point::new(100.0, 100.0));
        cx.refresh();
        let mut t = SelectTool::default();
        // A click selects it as a Foundation object.
        let p = ev(&cx, 104.0, 98.0);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Foundation(id)));
        assert!(ObjectRef::Foundation(id).exists(cx.floor()));
        assert_eq!(
            layer_of(cx.floor(), ObjectRef::Foundation(id)).as_deref(),
            Some("Piers/Pads")
        );
        // Dragging its body moves it, as one undo step.
        drag(&mut t, &mut cx, (104.0, 98.0), (128.0, 98.0));
        let moved = foundation_view::load(&cx).pad(id).unwrap().center;
        assert!(
            (moved.x - 124.0).abs() < 1e-6 && (moved.y - 100.0).abs() < 1e-6,
            "{moved:?}"
        );
        // Double-click asks for the specification.
        cx.requests.clear();
        let at = ev(&cx, 126.0, 100.0);
        assert!(t.double_click(&mut cx, at).consumed);
        assert!(cx
            .requests
            .iter()
            .any(|r| matches!(r, EditorRequest::OpenSpec(ObjectRef::Foundation(i)) if *i == id)));
        // Delete removes it; undo brings it back; box select finds it.
        assert!(t.key(&mut cx, KeyEvent::key(Key::Delete)).consumed);
        assert!(foundation_view::load(&cx).is_empty());
        assert!(cx.selection.is_empty());
        cx.undo();
        assert!(foundation_view::load(&cx).pad(id).is_some());
        let boxed = objects_in_rect(&cx, Point::new(0.0, 0.0), Point::new(300.0, 300.0));
        assert!(boxed.contains(&ObjectRef::Foundation(id)));
    }

    fn terrain_with_a_wall_and_a_bed() -> EditorContext {
        use plan_terrain::{
            Landscape, LandscapeKind, ShapeKind, TerrainWall, WallKind as TerrainWallKind,
        };
        let mut cx = EditorContext::new(plan_defaults::embedded());
        site_view::edit_terrain(&mut cx, "Draw", |r| {
            r.terrain.walls.push(TerrainWall::new(
                TerrainWallKind::Wall,
                vec![Point::new(0.0, 0.0), Point::new(240.0, 0.0)],
                false,
            ));
            r.terrain.landscape.push(Landscape::new(
                LandscapeKind::GardenBed,
                ShapeKind::Polyline,
                vec![
                    Point::new(300.0, 300.0),
                    Point::new(500.0, 300.0),
                    Point::new(500.0, 500.0),
                    Point::new(300.0, 500.0),
                ],
            ));
        });
        site_view::ensure_landscape_layers(&mut cx.project);
        cx.refresh();
        cx
    }

    fn terrain_of(cx: &EditorContext) -> plan_terrain::Terrain {
        site_view::load_terrain(&cx.project).unwrap().terrain
    }

    #[test]
    fn select_picks_moves_reshapes_opens_and_deletes_terrain_objects_with_undo() {
        use site_view::TerrainHit;
        let mut cx = terrain_with_a_wall_and_a_bed();
        let mut t = SelectTool::default();
        // A click selects the terrain wall, not the whole terrain.
        let p = ev(&cx, 100.0, 1.0);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        let wall = ObjectRef::TerrainObject(TerrainHit::Wall(0));
        assert_eq!(cx.selection.single(), Some(wall));
        assert_eq!(wall.type_name(), "Terrain Wall");
        assert!(wall.exists_in(&cx.project, 0));
        assert!(!ObjectRef::TerrainObject(TerrainHit::Wall(3)).exists_in(&cx.project, 0));

        // Dragging the body moves it as one undo step.
        let steps = cx.undo_label().map(str::to_string);
        drag(&mut t, &mut cx, (100.0, 1.0), (100.0, 49.0));
        let moved = terrain_of(&cx).walls[0].points.clone();
        assert!(
            (moved[0].y - 48.0).abs() < 1e-6 && (moved[1].y - 48.0).abs() < 1e-6,
            "{moved:?}"
        );
        assert_eq!(cx.undo_label(), Some("Move Objects"));
        cx.undo();
        assert_eq!(terrain_of(&cx).walls[0].points[0].y, 0.0);
        assert_eq!(cx.undo_label().map(str::to_string), steps);

        // A vertex handle per point; dragging one reshapes the wall.
        let hs = handles::handles_for(&cx, 2.0);
        assert_eq!(hs.len(), 2);
        assert!(hs.iter().all(|h| matches!(h.kind, HandleKind::Reshape(_))));
        drag(&mut t, &mut cx, (240.0, 0.0), (300.0, 0.0));
        assert_eq!(cx.undo_label(), Some("Reshape Terrain Element"));
        let pts = terrain_of(&cx).walls[0].points.clone();
        assert_eq!(
            (pts[0], pts[1]),
            (Point::new(0.0, 0.0), Point::new(300.0, 0.0))
        );
        cx.undo();

        // Arrow keys nudge it.
        assert!(t.key(&mut cx, KeyEvent::key(Key::ArrowUp)).commit.is_some());
        assert!(terrain_of(&cx).walls[0].points[0].y > 0.0);
        cx.undo();

        // Double-click asks for the specification.
        cx.requests.clear();
        let at = ev(&cx, 100.0, 1.0);
        assert!(t.double_click(&mut cx, at).consumed);
        assert!(cx
            .requests
            .iter()
            .any(|r| matches!(r, EditorRequest::OpenSpec(o) if *o == wall)));

        // Delete removes it; undo brings it back.
        assert!(t.key(&mut cx, KeyEvent::key(Key::Delete)).consumed);
        assert!(terrain_of(&cx).walls.is_empty());
        assert_eq!(terrain_of(&cx).landscape.len(), 1, "the bed stays");
        assert!(cx.selection.is_empty());
        assert_eq!(cx.undo_label(), Some("Delete Terrain Element"));
        cx.undo();
        assert_eq!(terrain_of(&cx).walls.len(), 1);
    }

    #[test]
    fn a_marquee_selects_terrain_objects_and_delete_takes_them_all() {
        use site_view::TerrainHit;
        let mut cx = terrain_with_a_wall_and_a_bed();
        let mut t = SelectTool::default();
        // Window: only what it encloses; crossing: what it touches.
        let inside = objects_in_rect(&cx, Point::new(-10.0, -10.0), Point::new(260.0, 20.0));
        assert!(inside.contains(&ObjectRef::TerrainObject(TerrainHit::Wall(0))));
        assert!(!inside.contains(&ObjectRef::TerrainObject(TerrainHit::Landscape(0))));
        let touching = objects_in_rect(&cx, Point::new(400.0, 400.0), Point::new(350.0, 350.0));
        assert!(touching.contains(&ObjectRef::TerrainObject(TerrainHit::Landscape(0))));
        // A box over both, then Delete: the indices of the second one must not
        // be disturbed by removing the first.
        drag(&mut t, &mut cx, (-20.0, -20.0), (600.0, 600.0));
        assert_eq!(cx.selection.len(), 2, "{:?}", cx.selection.items);
        assert!(t.key(&mut cx, KeyEvent::key(Key::Delete)).consumed);
        let rec = terrain_of(&cx);
        assert!(rec.walls.is_empty() && rec.landscape.is_empty());
        cx.undo();
        let rec = terrain_of(&cx);
        assert_eq!((rec.walls.len(), rec.landscape.len()), (1, 1));
    }

    #[test]
    fn a_group_move_carries_terrain_objects_with_plan_objects() {
        use site_view::TerrainHit;
        let mut cx = terrain_with_a_wall_and_a_bed();
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 200.0),
            Point::new(200.0, 200.0),
            6.0,
            100.0,
            plan_core::WallKind::Exterior,
        );
        cx.refresh();
        cx.selection.items = vec![
            ObjectRef::Wall(w),
            ObjectRef::TerrainObject(TerrainHit::Landscape(0)),
        ];
        let mut t = SelectTool::default();
        assert_eq!(
            t.key(&mut cx, KeyEvent::key(Key::ArrowRight))
                .commit
                .as_deref(),
            Some("Nudge")
        );
        let bed = terrain_of(&cx).landscape[0].points[0];
        assert!(bed.x > 300.0, "{bed:?}");
        assert!(cx.floor().wall(w).unwrap().start.x > 0.0);
        cx.undo();
        assert_eq!(
            terrain_of(&cx).landscape[0].points[0],
            Point::new(300.0, 300.0)
        );
        assert_eq!(cx.floor().wall(w).unwrap().start.x, 0.0);
    }

    #[test]
    fn the_perimeter_is_still_the_whole_terrain() {
        let mut cx = terrain_with_a_wall_and_a_bed();
        site_view::edit_terrain(&mut cx, "Perimeter", |r| {
            r.terrain.perimeter = vec![
                Point::new(-600.0, -600.0),
                Point::new(1200.0, -600.0),
                Point::new(1200.0, 1200.0),
                Point::new(-600.0, 1200.0),
            ];
        });
        let hits = hit_test_cx(&cx, Point::new(0.0, -600.0), 4.0);
        assert!(hits.contains(&ObjectRef::Terrain), "{hits:?}");
        let on_wall = hit_test_cx(&cx, Point::new(100.0, 1.0), 4.0);
        assert_eq!(
            on_wall.first(),
            Some(&ObjectRef::TerrainObject(site_view::TerrainHit::Wall(0)))
        );
    }

    #[test]
    fn moving_a_distribution_record_takes_its_copies_along() {
        use plan_core::images::{DistKind, Distribution};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let dist = Distribution::new(
            DistKind::Path,
            false,
            vec![Point::new(0.0, 0.0), Point::new(240.0, 0.0)],
            "shrub",
            [24.0, 24.0, 30.0],
        );
        let id = cx.project.add_distribution(0, dist);
        cx.refresh();
        let copies = |cx: &EditorContext| -> Vec<Point> {
            cx.floor()
                .symbols
                .iter()
                .filter(|s| s.owner == Some(id))
                .map(|s| s.position)
                .collect()
        };
        let before = copies(&cx);
        assert!(before.len() >= 3);
        let mut t = SelectTool::default();
        cx.selection.set(ObjectRef::Symbol(id));
        // An arrow-key nudge moves the record and rebuilds the copies there.
        assert!(t.key(&mut cx, KeyEvent::key(Key::ArrowUp)).commit.is_some());
        let nudged = copies(&cx);
        assert_eq!(nudged.len(), before.len());
        assert!(nudged[0].y > before[0].y, "{:?} {:?}", before[0], nudged[0]);
        cx.undo();
        assert_eq!(copies(&cx)[0], before[0]);
        // So does dragging the selected record by its move handle.
        cx.selection.set(ObjectRef::Symbol(id));
        let handle = handles::handles_for(&cx, 2.0)
            .into_iter()
            .find(|h| h.kind == HandleKind::Move)
            .expect("a move handle")
            .pos;
        drag(
            &mut t,
            &mut cx,
            (handle.x, handle.y),
            (handle.x, handle.y + 48.0),
        );
        let dragged = copies(&cx);
        assert_eq!(dragged.len(), before.len());
        assert!(dragged[0].y > before[0].y + 40.0, "{:?}", dragged[0]);
    }

    #[test]
    fn select_picks_moves_opens_and_deletes_a_schedule_with_undo() {
        use crate::editor::schedule_view as sv;
        use plan_core::schedules::ScheduleKind;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = sv::add(&mut cx, ScheduleKind::General, Point::new(100.0, 300.0));
        cx.refresh();
        let def = sv::find(&cx, id).unwrap();
        let (lo, hi) = sv::layout_of(&cx, &def, 0).bounds(def.position);
        let inside = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
        let mut t = SelectTool::default();
        // A click selects it as a Schedule object on its own layer.
        let p = ev(&cx, inside.x, inside.y);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Schedule(id)));
        assert_eq!(sv::selected(&cx), Some(id));
        assert!(ObjectRef::Schedule(id).exists(cx.floor()));
        assert_eq!(
            layer_of(cx.floor(), ObjectRef::Schedule(id)).as_deref(),
            Some(def.layer.as_str())
        );
        // Dragging its body moves it, as one undo step.
        drag(
            &mut t,
            &mut cx,
            (inside.x, inside.y),
            (inside.x + 48.0, inside.y + 24.0),
        );
        let moved = sv::find(&cx, id).unwrap().position;
        assert!(
            (moved.x - 148.0).abs() < 1e-6 && (moved.y - 324.0).abs() < 1e-6,
            "{moved:?}"
        );
        cx.undo();
        assert_eq!(sv::find(&cx, id).unwrap().position, def.position);
        // Double-click asks for the specification.
        cx.requests.clear();
        let at = ev(&cx, inside.x, inside.y);
        assert!(t.double_click(&mut cx, at).consumed);
        assert!(cx
            .requests
            .iter()
            .any(|r| matches!(r, EditorRequest::OpenSpec(ObjectRef::Schedule(i)) if *i == id)));
        // Box select finds it; Delete removes it and undo brings it back.
        let boxed = objects_in_rect(&cx, Point::new(-500.0, -500.0), Point::new(3000.0, 3000.0));
        assert!(boxed.contains(&ObjectRef::Schedule(id)));
        cx.selection.set(ObjectRef::Schedule(id));
        assert!(t.key(&mut cx, KeyEvent::key(Key::Delete)).consumed);
        assert!(sv::find(&cx, id).is_none());
        assert!(cx.selection.is_empty());
        assert_eq!(cx.undo_label(), Some("Delete Schedule"));
        cx.undo();
        assert!(sv::find(&cx, id).is_some());
    }

    #[test]
    fn roof_dormers_and_ceiling_planes_are_selected_as_roof_records() {
        use crate::editor::roof_view;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let fl = cx.floor;
        let id = roof_view::add_ceiling(
            &mut cx.project,
            fl,
            (
                Point::new(0.0, 0.0),
                Point::new(240.0, 0.0),
                Point::new(120.0, 90.0),
            ),
            100.0,
            4.0,
        )
        .unwrap();
        cx.mark_dirty();
        cx.refresh();
        let mut t = SelectTool::default();
        let p = ev(&cx, 120.0, 45.0);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        assert_eq!(cx.selection.single(), Some(ObjectRef::RoofPlane(id)));
        // Moving it moves the ceiling plane.
        drag(&mut t, &mut cx, (120.0, 45.0), (144.0, 45.0));
        let c = &roof_view::load(cx.floor()).ceilings[0];
        assert!((c.outline[0].x - 24.0).abs() < 1e-6, "{:?}", c.outline[0]);
        // Delete removes it.
        t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert!(roof_view::load(cx.floor()).ceilings.is_empty());
    }

    #[test]
    fn select_picks_stretches_moves_and_deletes_framing() {
        use crate::editor::framing_view::{self, Record};
        use plan_framing::{FramingMember, ManualMemberKind};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let m = framing_view::new_member(
            cx.floor(),
            ManualMemberKind::Joist,
            Point::new(0.0, 48.0),
            Point::new(192.0, 48.0),
        );
        let id = framing_view::add_record(&mut cx, "Place Joist", |id| {
            Record::Manual(FramingMember { id, ..m })
        });
        cx.selection.clear();
        cx.refresh();
        let mut t = SelectTool::default();
        // A click selects it as a Framing object on its framing layer.
        let p = ev(&cx, 96.0, 48.0);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Framing(id)));
        assert!(ObjectRef::Framing(id).exists(cx.floor()));
        assert_eq!(
            layer_of(cx.floor(), ObjectRef::Framing(id)).as_deref(),
            Some("Framing, Floor Joists")
        );
        // The tools share the selection with framing_view.
        assert_eq!(framing_view::selected(&cx), vec![id]);
        // It has an end handle at each end.
        let hs = handles::handles_for(&cx, cx.px_per_in);
        assert_eq!(hs.len(), 2);
        assert_eq!(hs[1].kind, HandleKind::ResizeEnd);
        assert_eq!(hs[1].pos, Point::new(192.0, 48.0));
        // Dragging the end handle stretches the joist, as one undo step.
        drag(&mut t, &mut cx, (192.0, 48.0), (264.0, 48.0));
        let len = |cx: &EditorContext| match framing_view::find(cx.floor(), id) {
            Some(Record::Manual(m)) => m.plan_length(),
            other => panic!("{other:?}"),
        };
        assert!((len(&cx) - 264.0).abs() < 1e-6, "{}", len(&cx));
        assert_eq!(cx.undo_label(), Some("Stretch Framing"));
        // Dragging the body moves it.
        drag(&mut t, &mut cx, (120.0, 48.0), (120.0, 96.0));
        let Some(Record::Manual(m)) = framing_view::find(cx.floor(), id) else {
            panic!("the joist is gone");
        };
        assert!((m.start.y - 96.0).abs() < 1e-6 && (m.end.y - 96.0).abs() < 1e-6);
        assert_eq!(cx.undo_label(), Some("Move Objects"));
        // Double-click asks for the specification.
        cx.requests.clear();
        let at = ev(&cx, 120.0, 96.0);
        assert!(t.double_click(&mut cx, at).consumed);
        assert!(cx
            .requests
            .iter()
            .any(|r| matches!(r, EditorRequest::OpenSpec(ObjectRef::Framing(i)) if *i == id)));
        // Box select finds it; Delete removes it and undo brings it back.
        let boxed = objects_in_rect(&cx, Point::new(-10.0, 0.0), Point::new(400.0, 200.0));
        assert!(boxed.contains(&ObjectRef::Framing(id)));
        assert!(t.key(&mut cx, KeyEvent::key(Key::Delete)).consumed);
        assert!(framing_view::find(cx.floor(), id).is_none());
        assert!(cx.selection.is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Delete Framing"));
        assert!(framing_view::find(cx.floor(), id).is_some());
    }

    #[test]
    fn truss_base_corners_and_posts_in_the_selection() {
        use crate::editor::framing_view::{self, Record};
        use plan_framing::{FramingMember, ManualMemberKind, TrussBase};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let base = framing_view::add_record(&mut cx, "Place Truss Base", |id| Record::TrussBase {
            id,
            base: TrussBase::new(
                vec![
                    Point::new(0.0, 0.0),
                    Point::new(240.0, 0.0),
                    Point::new(240.0, 144.0),
                    Point::new(0.0, 144.0),
                ],
                109.0,
            ),
        });
        let post = framing_view::new_member(
            cx.floor(),
            ManualMemberKind::Post,
            Point::new(300.0, 72.0),
            Point::new(300.0, 72.0),
        );
        let post_id = framing_view::add_record(&mut cx, "Place Post", |id| {
            Record::Manual(FramingMember { id, ..post })
        });
        cx.selection.set(ObjectRef::Framing(base));
        cx.refresh();
        // One corner handle per base corner; a post has none.
        let hs = handles::handles_for(&cx, cx.px_per_in);
        assert_eq!(hs.len(), 4);
        assert!(hs.iter().all(|h| matches!(h.kind, HandleKind::Reshape(_))));
        let mut t = SelectTool::default();
        drag(&mut t, &mut cx, (240.0, 144.0), (288.0, 168.0));
        let Some(Record::TrussBase { base: b, .. }) = framing_view::find(cx.floor(), base) else {
            panic!("the base is gone");
        };
        assert_eq!(b.points[2], Point::new(288.0, 168.0));
        assert_eq!(cx.undo_label(), Some("Reshape Truss Base"));
        cx.selection.set(ObjectRef::Framing(post_id));
        assert!(handles::handles_for(&cx, cx.px_per_in).is_empty());
    }

    #[test]
    fn slab_corners_are_dragged_with_handles() {
        use plan_core::foundation::rect_outline;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let slab = foundation_view::add_slab(
            &mut cx,
            rect_outline(Point::new(0.0, 0.0), Point::new(240.0, 180.0)),
            false,
        );
        let hole = foundation_view::add_platform_hole(
            &mut cx,
            rect_outline(Point::new(60.0, 60.0), Point::new(108.0, 108.0)),
            plan_core::foundation::PlatformKind::Floor,
        );
        cx.selection.set(ObjectRef::Foundation(slab));
        cx.refresh();
        let hs = handles::handles_for(&cx, cx.px_per_in);
        assert_eq!(hs.len(), 4);
        let corner = hs[2].pos;
        let mut t = SelectTool::default();
        drag(
            &mut t,
            &mut cx,
            (corner.x, corner.y),
            (corner.x + 48.0, corner.y + 24.0),
        );
        let moved = foundation_view::load(&cx).slab(slab).unwrap().outline[2];
        assert!(
            (moved.x - corner.x - 48.0).abs() < 1e-6 && (moved.y - corner.y - 24.0).abs() < 1e-6,
            "{moved:?}"
        );
        assert_eq!(cx.undo_label(), Some("Reshape Foundation Object"));
        cx.undo();
        assert_eq!(
            foundation_view::load(&cx).slab(slab).unwrap().outline[2],
            corner
        );
        // Platform holes have corner handles too.
        cx.selection.set(ObjectRef::Foundation(hole));
        assert_eq!(handles::handles_for(&cx, cx.px_per_in).len(), 4);
    }
    fn square(x: f64, y: f64, s: f64) -> Vec<Point> {
        vec![
            Point::new(x, y),
            Point::new(x + s, y),
            Point::new(x + s, y + s),
            Point::new(x, y + s),
        ]
    }

    #[test]
    fn select_picks_moves_opens_and_deletes_details_with_undo() {
        use plan_core::details::{DetailRef, MoldingProfile};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let deck = details_view::add_deck(&mut cx, square(100.0, 100.0, 96.0));
        let mold = details_view::add_molding(
            &mut cx,
            vec![Point::new(0.0, 400.0), Point::new(100.0, 400.0)],
            MoldingProfile::Base,
        );
        cx.refresh();
        let mut t = SelectTool::default();
        // A click selects the deck as a Detail object on its layer.
        let p = ev(&cx, 150.0, 150.0);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Detail(deck)));
        assert!(ObjectRef::Detail(deck).exists(cx.floor()));
        assert_eq!(
            layer_of(cx.floor(), ObjectRef::Detail(deck)).as_deref(),
            Some("Decks")
        );
        // The details tool reads the same selection.
        assert_eq!(details_view::selected(&cx), Some(DetailRef::Deck(deck)));
        // Dragging the body moves it, as one undo step.
        drag(&mut t, &mut cx, (150.0, 150.0), (174.0, 150.0));
        assert_eq!(
            details_view::load(&cx).deck(deck).unwrap().outline[0],
            Point::new(124.0, 100.0)
        );
        assert_eq!(cx.undo_label(), Some("Move Objects"));
        // Double-click asks for the specification.
        cx.requests.clear();
        let at = ev(&cx, 170.0, 150.0);
        assert!(t.double_click(&mut cx, at).consumed);
        assert!(cx
            .requests
            .iter()
            .any(|r| matches!(r, EditorRequest::OpenSpec(ObjectRef::Detail(i)) if *i == deck)));
        // Box select finds both; crossing from the right finds the deck too.
        let boxed = objects_in_rect(&cx, Point::new(0.0, 0.0), Point::new(500.0, 500.0));
        assert!(boxed.contains(&ObjectRef::Detail(deck)));
        assert!(boxed.contains(&ObjectRef::Detail(mold)));
        // A molding line is picked by its stroke and has two end handles.
        let q = ev(&cx, 50.0, 401.0);
        t.pointer_down(&mut cx, q.with_down(true));
        t.pointer_up(&mut cx, q);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Detail(mold)));
        let hs = handles::handles_for(&cx, cx.px_per_in);
        assert_eq!(hs.len(), 2);
        // Dragging its end stretches it: one undo step.
        let end = hs[1].pos;
        drag(&mut t, &mut cx, (end.x, end.y), (end.x + 60.0, end.y));
        let line = details_view::load(&cx)
            .molding(mold)
            .unwrap()
            .polyline
            .clone();
        assert!(
            (line[1].x - 160.0).abs() < 1e-6 && line[1].y == 400.0,
            "{line:?}"
        );
        assert_eq!(cx.undo_label(), Some("Reshape Detail"));
        cx.undo();
        assert_eq!(
            details_view::load(&cx).molding(mold).unwrap().polyline[1],
            Point::new(100.0, 400.0)
        );
        // Delete removes the selected molding; undo brings it back.
        assert!(t.key(&mut cx, KeyEvent::key(Key::Delete)).consumed);
        assert!(details_view::load(&cx).molding(mold).is_none());
        assert!(cx.selection.is_empty());
        cx.undo();
        assert!(details_view::load(&cx).molding(mold).is_some());
    }

    #[test]
    fn a_deck_corner_handle_reshapes_it() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let deck = details_view::add_deck(&mut cx, square(100.0, 100.0, 96.0));
        cx.selection.set(ObjectRef::Detail(deck));
        cx.refresh();
        let hs = handles::handles_for(&cx, cx.px_per_in);
        assert_eq!(hs.len(), 4);
        let corner = hs[2].pos;
        let mut t = SelectTool::default();
        drag(
            &mut t,
            &mut cx,
            (corner.x, corner.y),
            (corner.x + 48.0, corner.y + 24.0),
        );
        let moved = details_view::load(&cx).deck(deck).unwrap().outline[2];
        assert!(
            (moved.x - corner.x - 48.0).abs() < 1e-6 && (moved.y - corner.y - 24.0).abs() < 1e-6,
            "{moved:?}"
        );
        assert_eq!(cx.undo_label(), Some("Reshape Detail"));
    }

    #[test]
    fn deleting_a_wall_drops_its_material_regions_and_hatches_in_one_step() {
        let (mut cx, ids) = room();
        let region = details_view::add_wall_region(
            &mut cx,
            ids[0],
            plan_core::walls::Side::Left,
            (10.0, 60.0),
            (0.0, 96.0),
        );
        details_view::wall_hatch(&mut cx, ids[0]);
        let other = details_view::add_wall_region(
            &mut cx,
            ids[1],
            plan_core::walls::Side::Left,
            (10.0, 60.0),
            (0.0, 96.0),
        );
        cx.selection.set(ObjectRef::Wall(ids[0]));
        cx.delete_selection();
        let l = details_view::load(&cx);
        assert!(l.region(region).is_none() && l.hatches.is_empty());
        assert!(l.region(other).is_some(), "other walls keep theirs");
        // Undo restores the wall and its regions together.
        let steps = std::iter::from_fn(|| cx.undo()).count();
        assert!(steps >= 1);
        assert!(cx.floor().wall(ids[0]).is_some());
    }

    #[test]
    fn moving_a_wall_moves_its_corner_trim_and_keeps_its_regions() {
        let (mut cx, ids) = room();
        cx.refresh();
        details_view::auto_corner_boards(&mut cx);
        details_view::auto_quoins(&mut cx);
        let region = details_view::add_wall_region(
            &mut cx,
            ids[2],
            plan_core::walls::Side::Left,
            (10.0, 60.0),
            (0.0, 96.0),
        );
        let top = |cx: &EditorContext| -> Vec<f64> {
            details_view::load(cx)
                .corner_boards
                .iter()
                .map(|b| b.wall_corner.y)
                .filter(|y| *y > 50.0)
                .collect()
        };
        assert_eq!(top(&cx), vec![99.0, 99.0]);
        let mut t = SelectTool::default();
        // Drag the top wall (y = 96) down 20".
        drag(&mut t, &mut cx, (60.0, 96.0), (75.0, 76.0));
        assert_eq!(cx.undo_label(), Some("Move Wall"));
        assert_eq!(top(&cx), vec![79.0, 79.0]);
        let l = details_view::load(&cx);
        assert_eq!(
            l.quoins
                .iter()
                .filter(|q| (q.corner.y - 79.0).abs() < 1e-6)
                .count(),
            2
        );
        // The wall region is measured along the wall, so it moved with it.
        let strip = l.region(region).unwrap().plan_polygon(cx.floor()).unwrap();
        assert!(
            strip.iter().all(|q| (q.y - 76.0).abs() <= 3.0 + 1e-9),
            "{strip:?}"
        );
        // The bottom corners stay; one undo puts everything back.
        assert_eq!(
            l.corner_boards
                .iter()
                .filter(|b| b.wall_corner.y < 0.0)
                .count(),
            2
        );
        cx.undo();
        assert_eq!(top(&cx), vec![99.0, 99.0]);
        // A nudge moves trim too.
        cx.selection.set(ObjectRef::Wall(ids[2]));
        cx.refresh();
        let snap = cx.snap_unit();
        t.key(&mut cx, KeyEvent::key(Key::ArrowDown));
        assert_eq!(top(&cx), vec![99.0 - snap, 99.0 - snap]);
    }

    #[test]
    fn moving_a_cabinet_with_select_takes_its_joined_top_apart() {
        use plan_cabinets::Cabinet;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        placed::set_auto_join(true);
        for x in [0.0, 24.0] {
            let mut c = Cabinet::base(24.0);
            c.position = Point::new(x, 0.0);
            placed::add_cabinet(&mut cx.project, 0, c).unwrap();
        }
        assert_eq!(placed::rejoin_countertops(&mut cx), 1);
        let mut t = SelectTool::default();
        // Pick the first base cabinet (not the top over it) and drag it away.
        drag(&mut t, &mut cx, (12.0, 12.0), (12.0, 212.0));
        let list = placed::load_cabinets(cx.floor());
        assert_eq!(list.len(), 2, "the top is gone: {list:?}");
        assert!(list
            .iter()
            .all(|c| c.joined.is_empty() && c.countertop.is_some()));
        assert!(list.iter().any(|c| c.position.y > 100.0));
        assert_eq!(cx.undo_label(), Some("Edit Cabinet"));
        cx.undo();
        assert_eq!(
            placed::load_cabinets(cx.floor()).len(),
            3,
            "undo brings the top back"
        );
        placed::set_auto_join(false);
    }
}
