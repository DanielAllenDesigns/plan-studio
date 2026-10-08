//! Room selection, labels and fills, the floor commands and the Space
//! Planning boxes (R-16, R-19..R-54, R-55..R-68 in `docs/parity/rooms-floors.md`).
//!
//! `ObjectRef` has no room variant and the shared selection files are not
//! ours to change, so the selected room, the session-only room settings and
//! the Space Planning boxes live in a thread-local [`State`] of this module
//! (the UI runs on one thread; every test thread gets its own). Rooms are
//! derived from walls, so a room is remembered by a point inside it, never by
//! an index.
//!
//! What a Room Specification sets and the file keeps lives in the room's
//! [`RoomName`] (conditioned, stem wall height, fill style, label options,
//! moldings); [`RoomExtras`] is the dialog's view of those plus the few
//! settings that are still per session.

use super::{Camera, EditorContext};
use crate::dialogs::room::RoomInit;
use eframe::egui::{self, Align2, Color32, FontId, Mesh, Pos2, Shape, Stroke};
use plan_core::cad::{CadItem, DEFAULT_CAD_LAYER};
use plan_core::extras::{AreaKind, MoldingKind, MoldingRef, RoomFill, RoomLabelOptions};
use plan_core::geometry::{point_in_polygon, polygon_area, Point};
use plan_core::{detect_rooms, FloorKind, FoundationKind, PlanDefaults, Room, RoomName};
use plan_spaceplan::{bump, plan_symbols, RoomBox, Stroke as SpStroke, GRID};
use std::cell::RefCell;
use std::collections::HashMap;

// ----- per-room settings -----

/// Plan fill pattern of a room (R-35).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FillPattern {
    #[default]
    None,
    Solid,
    Hatch,
    CrossHatch,
    Grid,
}

impl FillPattern {
    pub const ALL: [FillPattern; 5] = [
        FillPattern::None,
        FillPattern::Solid,
        FillPattern::Hatch,
        FillPattern::CrossHatch,
        FillPattern::Grid,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FillPattern::None => "None",
            FillPattern::Solid => "Solid",
            FillPattern::Hatch => "Hatch",
            FillPattern::CrossHatch => "Cross Hatch",
            FillPattern::Grid => "Grid",
        }
    }

    /// The pattern a stored name stands for (unknown names fill solid).
    fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|p| p.name() == name)
            .unwrap_or(FillPattern::Solid)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FillStyle {
    pub pattern: FillPattern,
    pub color: [u8; 3],
    /// Opacity factor, 0..=1, applied to the pattern's own transparency.
    pub alpha: f32,
}

impl Default for FillStyle {
    fn default() -> Self {
        Self {
            pattern: FillPattern::None,
            color: [0xC8, 0xB4, 0x8C],
            alpha: 1.0,
        }
    }
}

impl FillStyle {
    /// The fill a room stores (`None` = no fill).
    pub fn from_room(fill: Option<&RoomFill>) -> Self {
        fill.map_or_else(Self::default, |f| Self {
            pattern: FillPattern::from_name(&f.pattern),
            color: f.color,
            alpha: f.alpha.clamp(0.0, 1.0),
        })
    }

    /// The stored form of this fill; no pattern stores nothing.
    pub fn to_room(self) -> Option<RoomFill> {
        (self.pattern != FillPattern::None).then(|| RoomFill {
            color: self.color,
            pattern: self.pattern.name().to_string(),
            alpha: self.alpha,
        })
    }
}

/// Room Specification > Label (R-45, R-50): the model's label options.
pub type LabelOptions = RoomLabelOptions;

/// Default molding heights when a profile name is first entered, inches.
const BASE_MOLDING_HEIGHT: f64 = 5.25;
const CROWN_MOLDING_HEIGHT: f64 = 3.5;

/// What a Room Specification holds beyond the plain fields of
/// `plan_core::RoomName`. Conditioned, the stem wall, the moldings, the fill
/// and the label options are stored in the `RoomName` (see
/// [`RoomExtras::with_stored`] and [`RoomExtras::store_into`]); the rest is
/// kept for the session.
#[derive(Clone, PartialEq, Debug)]
pub struct RoomExtras {
    /// `None` follows the room type (R-43).
    pub conditioned: Option<bool>,
    pub roof_over: bool,
    pub floor_height_absolute: bool,
    pub ceiling_height_absolute: bool,
    pub floor_finish_thickness: f64,
    pub ceiling_finish_thickness: f64,
    pub stem_wall: bool,
    pub stem_wall_height: f64,
    pub base_molding: String,
    pub crown_molding: String,
    pub wall_covering: String,
    pub fill: FillStyle,
    pub label: LabelOptions,
}

impl RoomExtras {
    pub fn from_defaults(d: &PlanDefaults) -> Self {
        Self {
            conditioned: None,
            roof_over: true,
            floor_height_absolute: false,
            ceiling_height_absolute: false,
            floor_finish_thickness: d.rooms.floor_finish_thickness,
            ceiling_finish_thickness: d.rooms.ceiling_finish_thickness,
            stem_wall: false,
            stem_wall_height: 0.0,
            base_molding: String::new(),
            crown_molding: String::new(),
            wall_covering: String::new(),
            fill: FillStyle::default(),
            label: LabelOptions::default(),
        }
    }

    /// These extras with the stored settings of `name` laid over them.
    pub fn with_stored(mut self, name: &RoomName) -> Self {
        self.conditioned = name.conditioned;
        self.stem_wall = name.stem_wall_height.is_some();
        if let Some(h) = name.stem_wall_height {
            self.stem_wall_height = h;
        }
        self.base_molding = molding_profile(name, MoldingKind::Base);
        self.crown_molding = molding_profile(name, MoldingKind::Crown);
        self.fill = FillStyle::from_room(name.fill_style.as_ref());
        self.label = name.label.clone();
        self
    }

    /// Writes the settings the file keeps into `name`.
    pub fn store_into(&self, name: &mut RoomName) {
        name.conditioned = self.conditioned;
        name.stem_wall_height = self.stem_wall.then_some(self.stem_wall_height);
        set_molding(
            name,
            MoldingKind::Base,
            &self.base_molding,
            BASE_MOLDING_HEIGHT,
        );
        set_molding(
            name,
            MoldingKind::Crown,
            &self.crown_molding,
            CROWN_MOLDING_HEIGHT,
        );
        name.fill_style = self.fill.to_room();
        name.label = self.label.clone();
    }
}

/// The profile name of the room's `kind` molding ("" = none).
fn molding_profile(name: &RoomName, kind: MoldingKind) -> String {
    name.moldings
        .iter()
        .find(|m| m.kind == kind)
        .map(|m| m.profile.clone())
        .unwrap_or_default()
}

/// Sets, renames or removes the `kind` molding of `name`; other kinds stay,
/// and a renamed molding keeps its height.
fn set_molding(name: &mut RoomName, kind: MoldingKind, profile: &str, height: f64) {
    let profile = profile.trim();
    match name.moldings.iter().position(|m| m.kind == kind) {
        Some(i) if profile.is_empty() => {
            name.moldings.remove(i);
        }
        Some(i) => name.moldings[i].profile = profile.to_string(),
        None if profile.is_empty() => {}
        None => name.moldings.push(MoldingRef {
            kind,
            profile: profile.to_string(),
            height,
        }),
    }
}

type ExtrasKey = (usize, i64, i64);

fn extras_key(floor: usize, anchor: Point) -> ExtrasKey {
    (floor, anchor.x.round() as i64, anchor.y.round() as i64)
}

// ----- session state -----

/// The selected room: a point inside it, on a floor.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RoomSel {
    pub floor: usize,
    pub point: Point,
}

/// A drag of one Space Planning box.
#[derive(Clone, Copy, Debug)]
struct BoxDrag {
    id: u64,
    grab: Point,
    start_min: Point,
}

#[derive(Default)]
struct State {
    selected: Option<RoomSel>,
    extras: HashMap<ExtrasKey, RoomExtras>,
    /// Set by a double-click or Enter; the shell opens the dialog.
    dialog_request: Option<RoomSel>,
    boxes: Vec<RoomBox>,
    drag: Option<BoxDrag>,
    /// Snap distance of the running box drag, inches (`editing.bumping_distance`).
    bump_snap: f64,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

// ----- room lookup -----

/// A point inside `room` (the centroid, or another interior point for
/// concave rooms).
pub fn room_anchor(room: &Room) -> Point {
    let poly = &room.polygon;
    if point_in_polygon(room.centroid, poly) {
        return room.centroid;
    }
    let n = poly.len();
    for i in 0..n {
        let (a, b, c) = (poly[i], poly[(i + 1) % n], poly[(i + 2) % n]);
        let t = Point::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0);
        if point_in_polygon(t, poly) {
            return t;
        }
    }
    room.centroid
}

/// The innermost room containing `p` (nested rooms win over the room
/// around them).
pub fn room_index_at(cx: &EditorContext, p: Point) -> Option<usize> {
    cx.rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| point_in_polygon(p, &r.polygon))
        .min_by(|a, b| a.1.area_sq_in.total_cmp(&b.1.area_sq_in))
        .map(|(i, _)| i)
}

/// The name entry of `room` on the active floor.
pub fn name_entry<'a>(cx: &'a EditorContext, room: &Room) -> Option<&'a RoomName> {
    cx.floor()
        .room_names
        .iter()
        .find(|n| point_in_polygon(n.anchor, &room.polygon))
}

/// The Room Specification values of `room`: the session settings (or the
/// defaults') with the settings stored in its `RoomName` laid over them.
pub fn extras_for(cx: &EditorContext, room: &Room) -> RoomExtras {
    let entry = name_entry(cx, room);
    let key = entry.map(|n| extras_key(cx.floor, n.anchor));
    let base = key
        .and_then(|k| with(|s| s.extras.get(&k).cloned()))
        .unwrap_or_else(|| RoomExtras::from_defaults(&cx.defaults));
    match entry {
        Some(n) => base.with_stored(n),
        None => base,
    }
}

/// The label options stored with `room` (the defaults when it has no name
/// entry yet).
pub fn label_options(cx: &EditorContext, room: &Room) -> RoomLabelOptions {
    name_entry(cx, room).map_or_else(RoomLabelOptions::default, |n| n.label.clone())
}

/// The plan fill stored with `room`.
pub fn fill_style(cx: &EditorContext, room: &Room) -> FillStyle {
    FillStyle::from_room(name_entry(cx, room).and_then(|n| n.fill_style.as_ref()))
}

// ----- selection (R-16) -----

/// Selects room `idx` of `cx.rooms` and deselects the other objects.
pub fn select_room(cx: &mut EditorContext, idx: usize) {
    let Some(room) = cx.rooms.get(idx) else {
        return;
    };
    let sel = RoomSel {
        floor: cx.floor,
        point: room_anchor(room),
    };
    cx.selection.clear();
    with(|s| s.selected = Some(sel));
}

pub fn clear_room_selection() {
    with(|s| s.selected = None);
}

/// Index into `cx.rooms` of the selected room, if it still exists on the
/// active floor.
pub fn selected_room(cx: &EditorContext) -> Option<usize> {
    let sel = with(|s| s.selected)?;
    if sel.floor != cx.floor {
        return None;
    }
    room_index_at(cx, sel.point)
}

/// Asks the shell to open the Room Specification of room `idx`.
pub fn request_room_dialog(cx: &EditorContext, idx: usize) {
    if let Some(room) = cx.rooms.get(idx) {
        let sel = RoomSel {
            floor: cx.floor,
            point: room_anchor(room),
        };
        with(|s| s.dialog_request = Some(sel));
    }
}

/// The pending dialog request as a room index on the active floor.
pub fn take_room_dialog_request(cx: &EditorContext) -> Option<usize> {
    let sel = with(|s| s.dialog_request.take())?;
    (sel.floor == cx.floor)
        .then(|| room_index_at(cx, sel.point))
        .flatten()
}

// ----- labels (R-44..R-50) -----

/// The bounding-rectangle interior dimensions of a room (R-48): `W x L`.
pub fn interior_dims_text(cx: &EditorContext, room: &Room) -> String {
    let poly = if room.inner_polygon.len() >= 3 {
        &room.inner_polygon
    } else {
        &room.polygon
    };
    let (mut lo, mut hi) = (poly[0], poly[0]);
    for p in poly {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    format!("{} x {}", cx.fmt_dim(hi.x - lo.x), cx.fmt_dim(hi.y - lo.y))
}

/// The label text of a room per the label options stored with it: the name,
/// then the interior dimensions and the area (interior, standard or
/// centerline) when checked. Empty when nothing is shown (R-50).
pub fn room_label_text(cx: &EditorContext, room: &Room) -> String {
    let opts = label_options(cx, room);
    let mut lines = Vec::new();
    if opts.show_name {
        lines.push(cx.room_name(room));
    }
    if opts.show_dimensions {
        lines.push(interior_dims_text(cx, room));
    }
    if opts.show_area {
        lines.push(match opts.area_kind {
            AreaKind::Interior => format!("{} sq ft", room.interior_area_sq_ft().round()),
            AreaKind::Standard => format!("{} sq ft std", room.standard_area_sq_ft().round()),
            AreaKind::Centerline => format!("{} sq ft c/l", room.area_sq_ft().round()),
        });
    }
    lines.join("\n")
}

// ----- living area (R-51..R-54) -----

/// Does the room count toward the living area? Its own setting wins, else the
/// room type's, else it counts.
pub fn counts_as_living(defaults: &PlanDefaults, entry: Option<&RoomName>) -> bool {
    match entry {
        Some(e) => e.include_in_living_area.unwrap_or_else(|| {
            defaults
                .room_type(&e.room_type)
                .is_none_or(|t| t.include_in_living_area)
        }),
        None => true,
    }
}

/// Total Living Area over all floors, square feet (interior areas).
pub fn living_area_total_sq_ft(cx: &EditorContext) -> f64 {
    let mut total = 0.0;
    for f in &cx.project.floors {
        for room in detect_rooms(&f.walls, 0.5) {
            let entry = f
                .room_names
                .iter()
                .find(|n| point_in_polygon(n.anchor, &room.polygon));
            if counts_as_living(&cx.defaults, entry) {
                total += room.interior_area_sq_ft();
            }
        }
    }
    total
}

// ----- the Room Specification (R-19..R-36) -----

/// Everything the Room Specification dialog starts from.
pub fn room_dialog_init(cx: &EditorContext, idx: usize) -> Option<RoomInit> {
    let room = cx.rooms.get(idx)?;
    let default_type = cx
        .defaults
        .rooms
        .room_types
        .first()
        .map_or(String::new(), |t| t.name.clone());
    let name = match name_entry(cx, room) {
        Some(e) => e.clone(),
        None => RoomName::new(room_anchor(room), room.label.clone(), default_type),
    };
    let outline = if room.inner_polygon.len() >= 3 {
        room.inner_polygon.clone()
    } else {
        room.polygon.clone()
    };
    let perimeter: f64 = (0..outline.len())
        .map(|i| outline[i].dist(outline[(i + 1) % outline.len()]))
        .sum();
    Some(RoomInit {
        room_index: idx,
        name,
        extras: extras_for(cx, room),
        types: cx.defaults.rooms.room_types.clone(),
        polygon: outline,
        interior_dims: interior_dims_text(cx, room),
        interior_area_sq_ft: room.interior_area_sq_ft(),
        standard_area_sq_ft: room.standard_area_sq_ft(),
        perimeter_in: perimeter,
        floor_elevation: cx.floor().elevation,
        floor_ceiling_height: cx.floor().ceiling_height,
        default_name: room.label.clone(),
        total_living_sq_ft: living_area_total_sq_ft(cx),
        floor_name: cx.floor().name.clone(),
    })
}

/// Writes an accepted Room Specification as one undo step.
pub fn apply_room_spec(
    cx: &mut EditorContext,
    room_index: usize,
    draft: &RoomName,
    extras: &RoomExtras,
) -> bool {
    let Some(room) = cx.rooms.get(room_index).cloned() else {
        return false;
    };
    let old_key = name_entry(cx, &room).map(|n| extras_key(cx.floor, n.anchor));
    let anchor = if point_in_polygon(draft.anchor, &room.polygon) {
        draft.anchor
    } else {
        room_anchor(&room)
    };
    cx.begin_change("Room Specification");
    let fl = cx.floor;
    let rooms = cx.rooms.clone();
    cx.project.set_room_name(
        fl,
        anchor,
        draft.name.clone(),
        draft.room_type.clone(),
        &rooms,
    );
    if let Some(n) = cx.project.floors[fl]
        .room_names
        .iter_mut()
        .find(|n| n.anchor == anchor)
    {
        n.floor_height_offset = draft.floor_height_offset;
        n.ceiling_height = draft.ceiling_height;
        n.floor_finish = draft.floor_finish.clone();
        n.ceiling_finish = draft.ceiling_finish.clone();
        n.include_in_living_area = draft.include_in_living_area;
        n.has_ceiling = draft.has_ceiling;
        n.has_floor = draft.has_floor;
        n.rough_ceiling = draft.rough_ceiling;
        extras.store_into(n);
    }
    with(|s| {
        if let Some(k) = old_key {
            s.extras.remove(&k);
        }
        s.extras.insert(extras_key(fl, anchor), extras.clone());
    });
    cx.mark_dirty();
    cx.refresh();
    true
}

// ----- drawing -----

fn screen_poly(cam: &Camera, poly: &[Point]) -> Vec<Pos2> {
    poly.iter().map(|p| cam.world_to_screen(*p)).collect()
}

/// Ear-clipping triangulation of a simple polygon (any winding).
fn triangulate(poly: &[Point]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let mut idx: Vec<usize> = (0..n).collect();
    if polygon_area(poly) < 0.0 {
        idx.reverse();
    }
    let cross = |a: Point, b: Point, c: Point| (b - a).cross(c - a);
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if cross(a, b, c) <= 1e-9 {
                continue;
            }
            let inside = idx.iter().any(|&k| {
                k != ia && k != ib && k != ic && {
                    let p = poly[k];
                    cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0
                }
            });
            if !inside {
                out.push([ia, ib, ic]);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

/// Parallel hatch segments across `poly`: lines where `n . p` is a multiple of
/// `spacing`, clipped to the polygon by pairing the edge crossings.
fn hatch_segments(poly: &[Point], n: Point, spacing: f64) -> Vec<(Point, Point)> {
    let f = |p: Point| n.x * p.x + n.y * p.y;
    let g = |p: Point| -n.y * p.x + n.x * p.y;
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for p in poly {
        lo = lo.min(f(*p));
        hi = hi.max(f(*p));
    }
    let mut out = Vec::new();
    let first = (lo / spacing).ceil() as i64;
    let last = (hi / spacing).floor() as i64;
    if last - first > 600 {
        return out;
    }
    for k in first..=last {
        let c = k as f64 * spacing + 1e-7;
        let mut hits: Vec<Point> = Vec::new();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let (fa, fb) = (f(a), f(b));
            if (fa - c) * (fb - c) < 0.0 {
                hits.push(Point::lerp(a, b, (c - fa) / (fb - fa)));
            }
        }
        hits.sort_by(|p, q| g(*p).total_cmp(&g(*q)));
        for (a, b) in hits.iter().step_by(2).zip(hits.iter().skip(1).step_by(2)) {
            out.push((*a, *b));
        }
    }
    out
}

fn draw_fill(painter: &egui::Painter, cam: &Camera, poly: &[Point], fill: FillStyle) {
    if fill.pattern == FillPattern::None || poly.len() < 3 {
        return;
    }
    let [r, g, b] = fill.color;
    let alpha = |base: f32| (base * fill.alpha.clamp(0.0, 1.0)).round() as u8;
    match fill.pattern {
        FillPattern::Solid => {
            let col = Color32::from_rgba_unmultiplied(r, g, b, alpha(70.0));
            let pts = screen_poly(cam, poly);
            let mut mesh = Mesh::default();
            for p in &pts {
                mesh.colored_vertex(*p, col);
            }
            for t in triangulate(poly) {
                mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
            }
            painter.add(Shape::mesh(mesh));
        }
        pattern => {
            let col = Color32::from_rgba_unmultiplied(r, g, b, alpha(150.0));
            let stroke = Stroke::new(1.0_f32, col);
            let spacing = 12.0_f64.max(6.0 / cam.px_per_in);
            let s = std::f64::consts::FRAC_1_SQRT_2;
            let dirs: Vec<Point> = match pattern {
                FillPattern::Hatch => vec![Point::new(s, s)],
                FillPattern::CrossHatch => vec![Point::new(s, s), Point::new(s, -s)],
                _ => vec![Point::new(1.0, 0.0), Point::new(0.0, 1.0)],
            };
            let spacing = if pattern == FillPattern::Grid {
                spacing * 2.0
            } else {
                spacing
            };
            for d in &dirs {
                for (a, b) in hatch_segments(poly, *d, spacing) {
                    painter.line_segment([cam.world_to_screen(a), cam.world_to_screen(b)], stroke);
                }
            }
        }
    }
}

/// Fills of every room, then the outline and corner handles of the selected
/// room (R-16, R-35). Called once from the room drawing.
pub fn draw_room_selection(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    for room in &cx.rooms {
        let fill = fill_style(cx, room);
        let poly = if room.inner_polygon.len() >= 3 {
            &room.inner_polygon
        } else {
            &room.polygon
        };
        draw_fill(painter, cam, poly, fill);
    }
    let Some(room) = selected_room(cx).and_then(|i| cx.rooms.get(i)) else {
        return;
    };
    let poly = if room.inner_polygon.len() >= 3 {
        &room.inner_polygon
    } else {
        &room.polygon
    };
    let col = cx.palette.selection;
    let pts = screen_poly(cam, poly);
    painter.add(Shape::closed_line(pts.clone(), Stroke::new(3.0_f32, col)));
    for p in pts {
        let r = egui::Rect::from_center_size(p, egui::vec2(7.0, 7.0));
        painter.rect_filled(r, 0.0, col);
    }
}

// ----- Space Planning boxes -----

/// Replaces the boxes shown on the plan.
pub fn set_space_boxes(boxes: Vec<RoomBox>) {
    with(|s| {
        s.boxes = boxes;
        s.drag = None;
    });
}

pub fn space_boxes() -> Vec<RoomBox> {
    with(|s| s.boxes.clone())
}

pub fn clear_space_boxes() {
    set_space_boxes(Vec::new());
}

fn box_at(boxes: &[RoomBox], floor: usize, p: Point) -> Option<usize> {
    boxes.iter().rposition(|b| {
        b.floor == floor
            && p.x >= b.rect.0.x
            && p.x <= b.rect.1.x
            && p.y >= b.rect.0.y
            && p.y <= b.rect.1.y
    })
}

/// A press on a box starts dragging it. Returns true when the box took it.
pub fn space_pointer_down(cx: &EditorContext, world: Point) -> bool {
    let editing = &cx.defaults.editing;
    let snap = if editing.bumping {
        editing.bumping_distance
    } else {
        0.0
    };
    with(|s| {
        let Some(i) = box_at(&s.boxes, cx.floor, world) else {
            return false;
        };
        s.bump_snap = snap;
        let b = &s.boxes[i];
        s.drag = Some(BoxDrag {
            id: b.id,
            grab: world,
            start_min: b.rect.0,
        });
        true
    })
}

/// Is a box being dragged?
pub fn space_dragging() -> bool {
    with(|s| s.drag.is_some())
}

/// Moves the dragged box to follow the pointer, bumped against its neighbors.
pub fn space_pointer_move(world: Point) -> bool {
    with(|s| {
        let Some(d) = s.drag else {
            return false;
        };
        let Some(b) = s.boxes.iter().find(|b| b.id == d.id) else {
            return false;
        };
        let (w, h) = (b.width(), b.height());
        let snap = |v: f64| (v / GRID).round() * GRID;
        let min = Point::new(
            snap(d.start_min.x + world.x - d.grab.x),
            snap(d.start_min.y + world.y - d.grab.y),
        );
        let proposed = (min, Point::new(min.x + w, min.y + h));
        let rect = bump(&s.boxes, d.id, proposed, s.bump_snap);
        if let Some(b) = s.boxes.iter_mut().find(|b| b.id == d.id) {
            b.rect = rect;
        }
        true
    })
}

/// Ends a box drag. Returns true when one was in progress.
pub fn space_pointer_up() -> bool {
    with(|s| s.drag.take().is_some())
}

/// The boxes of the active floor as colored rectangles with names and areas
/// (the plan_symbols drawing hook).
pub fn draw_space_boxes(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let boxes: Vec<RoomBox> = with(|s| {
        s.boxes
            .iter()
            .filter(|b| b.floor == cx.floor)
            .cloned()
            .collect()
    });
    if boxes.is_empty() {
        return;
    }
    let dragged = with(|s| s.drag.map(|d| d.id));
    for stroke in plan_symbols(&boxes) {
        match stroke {
            SpStroke::Polygon { points, fill } => {
                let pts = screen_poly(cam, &points);
                let col = Color32::from_rgba_unmultiplied(fill[0], fill[1], fill[2], 150);
                painter.add(Shape::convex_polygon(
                    pts,
                    col,
                    Stroke::new(1.5_f32, cx.palette.room_outline),
                ));
            }
            SpStroke::Text { at, text, height } => {
                let size = (height * cam.px_per_in) as f32;
                painter.text(
                    cam.world_to_screen(at),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(size.clamp(9.0, 18.0)),
                    cx.palette.room_label,
                );
            }
        }
    }
    if let Some(b) = dragged.and_then(|id| boxes.iter().find(|b| b.id == id)) {
        let r =
            egui::Rect::from_two_pos(cam.world_to_screen(b.rect.0), cam.world_to_screen(b.rect.1));
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(3.0_f32, cx.palette.selection),
            egui::StrokeKind::Outside,
        );
    }
}

// ----- floor commands (R-55..R-68) -----

/// The foundation types of the Build Foundation dialog (R-61, R-62).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FoundationType {
    WallsWithFootings,
    MonolithicSlab,
    Piers,
}

impl FoundationType {
    pub const ALL: [FoundationType; 3] = [
        FoundationType::WallsWithFootings,
        FoundationType::MonolithicSlab,
        FoundationType::Piers,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FoundationType::WallsWithFootings => "Walls with Footings",
            FoundationType::MonolithicSlab => "Monolithic Slab",
            FoundationType::Piers => "Piers",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FoundationSpec {
    pub kind: FoundationType,
    pub stem_height: f64,
    pub min_stem_height: f64,
    /// Not stored by the model yet.
    pub garage_floor: bool,
}

impl FoundationSpec {
    pub fn from_defaults(d: &PlanDefaults) -> Self {
        Self {
            kind: FoundationType::WallsWithFootings,
            stem_height: d.foundation_wall.height,
            min_stem_height: 12.0,
            garage_floor: true,
        }
    }

    pub fn to_kind(self) -> FoundationKind {
        match self.kind {
            FoundationType::WallsWithFootings => FoundationKind::StemWall {
                height: self.stem_height.max(self.min_stem_height),
            },
            FoundationType::MonolithicSlab => FoundationKind::MonolithicSlab,
            FoundationType::Piers => FoundationKind::Pier,
        }
    }
}

fn after_floor_change(cx: &mut EditorContext, floor: usize) {
    cx.floor = floor.min(cx.project.floors.len() - 1);
    cx.reset_view_state();
    clear_room_selection();
    clear_space_boxes();
    cx.mark_dirty();
    cx.refresh();
}

/// Build New Floor (R-59): derive the walls of the top floor or start blank.
pub fn build_new_floor(cx: &mut EditorContext, derive: bool) -> usize {
    cx.begin_change("Build New Floor");
    let idx = cx.project.build_new_floor(derive);
    after_floor_change(cx, idx);
    cx.status = format!("Built {}", cx.project.floors[idx].name);
    idx
}

/// Insert New Floor (R-60): an empty floor above the current one.
pub fn insert_floor(cx: &mut EditorContext) -> Option<usize> {
    cx.begin_change("Insert New Floor");
    match cx.project.insert_floor_above(cx.floor) {
        Some(idx) => {
            after_floor_change(cx, idx);
            cx.status = format!("Inserted {}", cx.project.floors[idx].name);
            Some(idx)
        }
        None => {
            cx.cancel_change();
            cx.status = "Cannot insert a floor above the attic".into();
            None
        }
    }
}

/// Delete Current Floor (R-60).
pub fn delete_floor(cx: &mut EditorContext) -> bool {
    let idx = cx.floor;
    let name = cx.floor().name.clone();
    cx.begin_change("Delete Current Floor");
    if !cx.project.delete_floor(idx) {
        cx.cancel_change();
        cx.status = "Cannot delete the only floor".into();
        return false;
    }
    after_floor_change(cx, idx.saturating_sub(1));
    cx.status = format!("Deleted {name}");
    true
}

/// Exchange With Floor Above/Below (R-64); the view stays on this floor.
pub fn exchange_floor(cx: &mut EditorContext, above: bool) -> bool {
    let other = if above {
        cx.floor + 1
    } else {
        match cx.floor.checked_sub(1) {
            Some(o) => o,
            None => {
                cx.status = "There is no floor below".into();
                return false;
            }
        }
    };
    cx.begin_change(if above {
        "Exchange With Floor Above"
    } else {
        "Exchange With Floor Below"
    });
    if !cx.project.exchange_floors(cx.floor, other) {
        cx.cancel_change();
        cx.status = format!(
            "There is no floor {}",
            if above { "above" } else { "below" }
        );
        return false;
    }
    let f = cx.floor;
    after_floor_change(cx, f);
    cx.status = "Exchanged the floors".into();
    true
}

/// Build Foundation (R-61, R-62). A newly created foundation becomes floor 0,
/// so the active floor index moves up by one to stay on the same floor.
pub fn build_foundation(cx: &mut EditorContext, spec: FoundationSpec) {
    let had = cx
        .project
        .floors
        .first()
        .is_some_and(|f| f.kind == FloorKind::Foundation);
    cx.begin_change("Build Foundation");
    cx.project.build_foundation(spec.to_kind());
    let next = if had { cx.floor } else { cx.floor + 1 };
    after_floor_change(cx, next);
    cx.status = format!("Built the foundation ({})", spec.kind.name());
}

/// Delete Foundation (R-63).
pub fn delete_foundation(cx: &mut EditorContext) -> bool {
    let has = cx
        .project
        .floors
        .first()
        .is_some_and(|f| f.kind == FloorKind::Foundation);
    if !has || cx.project.floors.len() < 2 {
        cx.status = "There is no foundation".into();
        return false;
    }
    cx.begin_change("Delete Foundation");
    cx.project.delete_floor(0);
    let next = cx.floor.saturating_sub(1);
    after_floor_change(cx, next);
    cx.status = "Deleted the foundation".into();
    true
}

/// Rebuild Walls/Floors/Ceilings (R-33): restack the floors and redo the
/// derived data.
pub fn rebuild_all(cx: &mut EditorContext) {
    cx.project.restack_floors();
    cx.mark_dirty();
    cx.refresh();
    cx.status = "Rebuilt walls, floors and ceilings".into();
}

/// Adds the footprint of the active floor as a closed CAD polyline with an
/// area note (Tools > Checks > Plan Footprint). Returns the area in sq ft.
pub fn add_plan_footprint(cx: &mut EditorContext) -> Option<f64> {
    cx.refresh();
    let fp = plan_check::plan_footprint(&cx.project, cx.floor, &cx.rooms);
    if fp.polygon.len() < 3 {
        cx.status = "Plan Footprint: there are no rooms on this floor".into();
        return None;
    }
    let (mut lo, mut hi) = (fp.polygon[0], fp.polygon[0]);
    for p in &fp.polygon {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let height = cx.defaults.text.height;
    cx.begin_change("Plan Footprint");
    let fl = cx.floor;
    cx.project.add_cad(
        fl,
        DEFAULT_CAD_LAYER,
        CadItem::Polyline {
            points: fp.polygon.clone(),
            closed: true,
        },
    );
    cx.project.add_cad(
        fl,
        DEFAULT_CAD_LAYER,
        CadItem::Text {
            pos: Point::new(lo.x, lo.y - height * 2.0),
            text: format!("Footprint: {:.0} sq ft", fp.area_sq_ft),
            height,
            angle: 0.0,
        },
    );
    cx.mark_dirty();
    cx.status = format!(
        "Plan Footprint: {:.0} sq ft, {:.1} ft around",
        fp.area_sq_ft, fp.perimeter_ft
    );
    Some(fp.area_sq_ft)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{Project, WallKind};

    fn house() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
        }
        cx.mark_dirty();
        cx.refresh();
        cx
    }

    #[test]
    fn room_is_found_and_selected_by_point() {
        let mut cx = house();
        assert_eq!(room_index_at(&cx, Point::new(120.0, 90.0)), Some(0));
        assert_eq!(room_index_at(&cx, Point::new(500.0, 90.0)), None);
        select_room(&mut cx, 0);
        assert_eq!(selected_room(&cx), Some(0));
        clear_room_selection();
        assert_eq!(selected_room(&cx), None);
    }

    #[test]
    fn selection_is_dropped_on_another_floor() {
        let mut cx = house();
        select_room(&mut cx, 0);
        build_new_floor(&mut cx, false);
        assert_eq!(selected_room(&cx), None);
    }

    #[test]
    fn label_follows_the_label_options() {
        let mut cx = house();
        let room = cx.rooms[0].clone();
        // A room that was never named shows its name and interior area.
        let text = room_label_text(&cx, &room);
        assert!(text.contains("sq ft") && !text.contains(" x "), "{text}");
        let mut extras = extras_for(&cx, &room);
        extras.label.show_dimensions = true;
        extras.label.area_kind = AreaKind::Standard;
        let draft = RoomName::new(room_anchor(&room), "Study", "Study");
        assert!(apply_room_spec(&mut cx, 0, &draft, &extras));
        let room = cx.rooms[0].clone();
        let text = room_label_text(&cx, &room);
        assert!(text.starts_with("Study"), "{text}");
        assert!(text.contains(" x "), "{text}");
        assert!(text.contains("std"), "{text}");
        let mut extras = extras_for(&cx, &room);
        extras.label.show_area = false;
        extras.label.show_dimensions = false;
        let draft = name_entry(&cx, &room).cloned().unwrap();
        apply_room_spec(&mut cx, 0, &draft, &extras);
        let room = cx.rooms[0].clone();
        assert_eq!(room_label_text(&cx, &room), "Study");
        let mut extras = extras_for(&cx, &room);
        extras.label.show_name = false;
        let draft = name_entry(&cx, &room).cloned().unwrap();
        apply_room_spec(&mut cx, 0, &draft, &extras);
        let room = cx.rooms[0].clone();
        assert_eq!(room_label_text(&cx, &room), "");
    }

    #[test]
    fn centerline_area_kind_shows_the_centerline_area() {
        let mut cx = house();
        let room = cx.rooms[0].clone();
        let mut extras = extras_for(&cx, &room);
        extras.label.area_kind = AreaKind::Centerline;
        let draft = RoomName::new(room_anchor(&room), "Hall", "Hall");
        apply_room_spec(&mut cx, 0, &draft, &extras);
        let room = cx.rooms[0].clone();
        let want = format!("{} sq ft c/l", room.area_sq_ft().round());
        assert!(room_label_text(&cx, &room).ends_with(&want));
    }

    #[test]
    fn room_specification_persists_in_the_room_name_and_the_file() {
        let mut cx = house();
        let room = cx.rooms[0].clone();
        let mut extras = extras_for(&cx, &room);
        extras.conditioned = Some(false);
        extras.stem_wall = true;
        extras.stem_wall_height = 30.0;
        extras.base_molding = "Colonial".into();
        extras.crown_molding = "Cove".into();
        extras.fill = FillStyle {
            pattern: FillPattern::Hatch,
            color: [1, 2, 3],
            alpha: 0.5,
        };
        extras.label.show_dimensions = true;
        extras.label.area_kind = AreaKind::Centerline;
        let draft = RoomName::new(room_anchor(&room), "Den", "Den");
        assert!(apply_room_spec(&mut cx, 0, &draft, &extras));

        let n = &cx.floor().room_names[0];
        assert_eq!(n.conditioned, Some(false));
        assert_eq!(n.stem_wall_height, Some(30.0));
        assert_eq!(n.fill_style.as_ref().unwrap().pattern, "Hatch");
        assert_eq!(n.fill_style.as_ref().unwrap().color, [1, 2, 3]);
        assert!(n.label.show_dimensions);
        assert_eq!(n.label.area_kind, AreaKind::Centerline);
        assert_eq!(n.moldings.len(), 2);

        // A fresh session (a reloaded file) sees the same Room Specification:
        // the stored values do not depend on the session extras.
        let json = cx.project.to_json().unwrap();
        let mut cx2 = EditorContext::new(crate::plan_defaults::embedded());
        cx2.set_project(Project::from_json(&json).unwrap());
        cx2.refresh();
        let room2 = cx2.rooms[0].clone();
        with(|s| s.extras.clear());
        let back = extras_for(&cx2, &room2);
        assert_eq!(back.conditioned, Some(false));
        assert!(back.stem_wall && back.stem_wall_height == 30.0);
        assert_eq!(back.base_molding, "Colonial");
        assert_eq!(back.crown_molding, "Cove");
        assert_eq!(back.fill, extras.fill);
        assert_eq!(back.label, extras.label);
        assert_eq!(fill_style(&cx2, &room2).pattern, FillPattern::Hatch);
        assert!(room_label_text(&cx2, &room2).contains(" x "));

        // Clearing them stores nothing.
        let mut cleared = back;
        cleared.stem_wall = false;
        cleared.base_molding.clear();
        cleared.fill.pattern = FillPattern::None;
        let draft = name_entry(&cx2, &room2).cloned().unwrap();
        apply_room_spec(&mut cx2, 0, &draft, &cleared);
        let n = &cx2.floor().room_names[0];
        assert_eq!(n.stem_wall_height, None);
        assert!(n.fill_style.is_none());
        assert_eq!(n.moldings.len(), 1);
        assert_eq!(n.moldings[0].kind, MoldingKind::Crown);
    }

    #[test]
    fn bumping_distance_comes_from_the_editing_defaults() {
        use plan_spaceplan::{generate_boxes, Questionnaire};
        let mut cx = house();
        cx.defaults.editing.bumping = false;
        set_space_boxes(generate_boxes(&Questionnaire::default()));
        let c = space_boxes()[0].center();
        assert!(space_pointer_down(&cx, c));
        assert_eq!(with(|s| s.bump_snap), 0.0);
        space_pointer_up();
        cx.defaults.editing.bumping = true;
        cx.defaults.editing.bumping_distance = 7.0;
        assert!(space_pointer_down(&cx, c));
        assert_eq!(with(|s| s.bump_snap), 7.0);
        space_pointer_up();
        clear_space_boxes();
    }

    #[test]
    fn spec_is_one_undo_step() {
        let mut cx = house();
        let room = cx.rooms[0].clone();
        let mut draft = RoomName::new(room_anchor(&room), "Den", "Den");
        draft.has_ceiling = false;
        draft.include_in_living_area = Some(false);
        let extras = extras_for(&cx, &room);
        apply_room_spec(&mut cx, 0, &draft, &extras);
        let entry = &cx.floor().room_names[0];
        assert_eq!(entry.name, "Den");
        assert!(!entry.has_ceiling);
        assert_eq!(cx.undo().as_deref(), Some("Room Specification"));
        assert!(cx.floor().room_names.is_empty());
    }

    #[test]
    fn living_area_honors_exclusion() {
        let mut cx = house();
        let all = living_area_total_sq_ft(&cx);
        assert!(all > 200.0, "{all}");
        let room = cx.rooms[0].clone();
        let mut draft = RoomName::new(room_anchor(&room), "Garage", "Garage");
        draft.include_in_living_area = None;
        let extras = extras_for(&cx, &room);
        apply_room_spec(&mut cx, 0, &draft, &extras);
        assert!(living_area_total_sq_ft(&cx) < 1.0);
    }

    #[test]
    fn new_floor_copies_exterior_walls() {
        let mut cx = house();
        let idx = build_new_floor(&mut cx, true);
        assert_eq!(cx.project.floors.len(), 2);
        assert_eq!(idx, 1);
        assert_eq!(cx.floor, 1);
        assert_eq!(cx.project.floors[1].walls.len(), 4);
        assert_eq!(cx.undo().as_deref(), Some("Build New Floor"));
        assert_eq!(cx.project.floors.len(), 1);
        build_new_floor(&mut cx, false);
        assert!(cx.project.floors[1].walls.is_empty());
    }

    #[test]
    fn foundation_shifts_the_active_floor() {
        let mut cx = house();
        let spec = FoundationSpec::from_defaults(&cx.defaults);
        build_foundation(&mut cx, spec);
        assert_eq!(cx.project.floors[0].kind, FloorKind::Foundation);
        assert_eq!(cx.floor, 1);
        assert_eq!(cx.project.floors[0].walls.len(), 4);
        // Rebuilding keeps the floor index.
        build_foundation(&mut cx, spec);
        assert_eq!(cx.floor, 1);
        assert_eq!(cx.project.floors.len(), 2);
        assert!(delete_foundation(&mut cx));
        assert_eq!(cx.floor, 0);
    }

    #[test]
    fn insert_delete_and_exchange() {
        let mut cx = house();
        build_new_floor(&mut cx, false);
        cx.floor = 0;
        assert!(exchange_floor(&mut cx, true));
        assert!(cx.project.floors[0].walls.is_empty());
        assert_eq!(cx.project.floors[1].walls.len(), 4);
        assert!(!exchange_floor(&mut cx, false));
        let at = insert_floor(&mut cx).unwrap();
        assert_eq!(at, 1);
        assert_eq!(cx.project.floors.len(), 3);
        assert!(delete_floor(&mut cx));
        assert_eq!(cx.project.floors.len(), 2);
        assert_eq!(cx.floor, 0);
    }

    #[test]
    fn space_boxes_drag_and_bump() {
        use plan_spaceplan::{generate_boxes, Questionnaire};
        let cx = house();
        let boxes = generate_boxes(&Questionnaire::default());
        assert!(!boxes.is_empty());
        let first = boxes[0].clone();
        set_space_boxes(boxes);
        let c = first.center();
        assert!(space_pointer_down(&cx, c));
        assert!(space_dragging());
        assert!(space_pointer_move(Point::new(c.x + 300.0, c.y + 300.0)));
        assert!(space_pointer_up());
        let moved = space_boxes()
            .into_iter()
            .find(|b| b.id == first.id)
            .unwrap();
        assert_ne!(moved.rect, first.rect);
        assert!(plan_spaceplan::validate(&space_boxes())
            .iter()
            .all(|i| !i.message.contains("overlap")));
        clear_space_boxes();
    }

    #[test]
    fn triangulation_covers_a_concave_polygon() {
        let l = [
            Point::new(0.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(20.0, 10.0),
            Point::new(10.0, 10.0),
            Point::new(10.0, 20.0),
            Point::new(0.0, 20.0),
        ];
        let tris = triangulate(&l);
        assert_eq!(tris.len(), 4);
        let total: f64 = tris
            .iter()
            .map(|t| polygon_area(&[l[t[0]], l[t[1]], l[t[2]]]).abs())
            .sum();
        assert!((total - 300.0).abs() < 1e-6, "{total}");
    }

    #[test]
    fn footprint_adds_cad() {
        let mut cx = house();
        let area = add_plan_footprint(&mut cx).unwrap();
        assert!(area > 250.0, "{area}");
        assert_eq!(cx.floor().cad.len(), 2);
    }
}
