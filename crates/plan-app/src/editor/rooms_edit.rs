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
//! moldings); [`RoomExtras`] is the dialog's view of those plus the
//! misc settings (roof over, absolute heights, finish thicknesses, wall
//! covering), which `RoomName::misc` stores too.

use super::{Camera, EditorContext};
use crate::dialogs::room::RoomInit;
use eframe::egui::{self, Align2, Color32, FontId, Mesh, Pos2, Shape, Stroke};
use plan_core::cad::{CadItem, DEFAULT_CAD_LAYER};
use plan_core::extras::{
    AreaKind, MoldingKind, MoldingRef, RoomFill, RoomLabelOptions, StructureLayer,
};
use plan_core::floors::{
    DeriveFrom, FloorPlacement, FloorSettings, FoundationKind, FoundationOptions, FoundationRooms,
    NewFloorOptions,
};
use plan_core::geometry::{polygon_area, Point};
use plan_core::rooms::{apply_function_defaults, function_defaults, molding_def};
use plan_core::{detect_rooms, FloorKind, PlanDefaults, Room, RoomName};
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
const CHAIR_MOLDING_HEIGHT: f64 = 3.0;

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
    /// Flat Roof over This Room: Build Roof levels the roof over it.
    pub flat_roof: bool,
    pub floor_height_absolute: bool,
    pub ceiling_height_absolute: bool,
    pub floor_finish_thickness: f64,
    pub ceiling_finish_thickness: f64,
    pub stem_wall: bool,
    pub stem_wall_height: f64,
    pub base_molding: String,
    pub crown_molding: String,
    /// Chair rail profile name ("" = none).
    pub chair_molding: String,
    pub wall_covering: String,
    pub fill: FillStyle,
    pub label: LabelOptions,
    /// Floor Structure layers (R-28); empty follows the floor's default.
    pub floor_structure: Vec<StructureLayer>,
    /// Ceiling Structure layers (R-29); empty follows the floor's default.
    pub ceiling_structure: Vec<StructureLayer>,
}

impl RoomExtras {
    pub fn from_defaults(d: &PlanDefaults) -> Self {
        Self {
            conditioned: None,
            roof_over: true,
            flat_roof: false,
            floor_height_absolute: false,
            ceiling_height_absolute: false,
            floor_finish_thickness: d.rooms.floor_finish_thickness,
            ceiling_finish_thickness: d.rooms.ceiling_finish_thickness,
            stem_wall: false,
            stem_wall_height: 0.0,
            base_molding: String::new(),
            crown_molding: String::new(),
            chair_molding: String::new(),
            wall_covering: String::new(),
            fill: FillStyle::default(),
            label: LabelOptions::default(),
            floor_structure: Vec::new(),
            ceiling_structure: Vec::new(),
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
        self.chair_molding = molding_profile(name, MoldingKind::Chair);
        self.fill = FillStyle::from_room(name.fill_style.as_ref());
        self.label = name.label.clone();
        if let Some(m) = &name.misc {
            self.roof_over = m.roof_over;
            self.flat_roof = m.flat_roof;
            self.floor_height_absolute = m.floor_height_absolute;
            self.ceiling_height_absolute = m.ceiling_height_absolute;
            self.floor_finish_thickness = m.floor_finish_thickness;
            self.ceiling_finish_thickness = m.ceiling_finish_thickness;
            self.wall_covering = m.wall_covering.clone();
            self.floor_structure = m.floor_structure.clone();
            self.ceiling_structure = m.ceiling_structure.clone();
        }
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
        set_molding(
            name,
            MoldingKind::Chair,
            &self.chair_molding,
            CHAIR_MOLDING_HEIGHT,
        );
        name.fill_style = self.fill.to_room();
        name.label = self.label.clone();
        name.misc = Some(plan_core::extras::RoomMisc {
            roof_over: self.roof_over,
            flat_roof: self.flat_roof,
            floor_height_absolute: self.floor_height_absolute,
            ceiling_height_absolute: self.ceiling_height_absolute,
            floor_finish_thickness: self.floor_finish_thickness,
            ceiling_finish_thickness: self.ceiling_finish_thickness,
            wall_covering: self.wall_covering.clone(),
            floor_structure: self.floor_structure.clone(),
            ceiling_structure: self.ceiling_structure.clone(),
        });
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

/// Sets, renames or removes the `kind` molding of `name`; other kinds stay.
/// A profile from the molding library brings its own height (R-34); a name
/// the library does not know keeps the height the molding had, or `height`.
fn set_molding(name: &mut RoomName, kind: MoldingKind, profile: &str, height: f64) {
    let profile = profile.trim();
    let library = molding_def(profile).map(|d| d.height());
    match name.moldings.iter().position(|m| m.kind == kind) {
        Some(i) if profile.is_empty() => {
            name.moldings.remove(i);
        }
        Some(i) => {
            let m = &mut name.moldings[i];
            if m.profile != profile {
                m.height = library.unwrap_or(m.height);
            }
            m.profile = profile.to_string();
        }
        None if profile.is_empty() => {}
        None => name.moldings.push(MoldingRef {
            kind,
            profile: profile.to_string(),
            height: library.unwrap_or(height),
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
    /// A room label being dragged (R-44).
    label_drag: Option<LabelDrag>,
}

/// A drag of one room label: the room is remembered by a point inside it.
#[derive(Clone, Copy, Debug)]
struct LabelDrag {
    floor: usize,
    room_point: Point,
    grab: Point,
    start_offset: Point,
    begun: bool,
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
    if room.contains(room.centroid) {
        return room.centroid;
    }
    let n = poly.len();
    for i in 0..n {
        let (a, b, c) = (poly[i], poly[(i + 1) % n], poly[(i + 2) % n]);
        let t = Point::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0);
        if room.contains(t) {
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
        .filter(|(_, r)| r.contains(p))
        .min_by(|a, b| a.1.area_sq_in.total_cmp(&b.1.area_sq_in))
        .map(|(i, _)| i)
}

/// The name entry of `room` on the active floor.
pub fn name_entry<'a>(cx: &'a EditorContext, room: &Room) -> Option<&'a RoomName> {
    room.name_entry(&cx.floor().room_names)
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

/// The values the label macros stand for (R-47).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LabelValues {
    pub name: String,
    pub room_type: String,
    pub area: String,
    pub standard_area: String,
    pub centerline_area: String,
    pub dims: String,
    pub ceiling: String,
    pub floor: String,
    pub perimeter: String,
}

/// The macros a label template understands, with what each stands for.
pub const LABEL_MACROS: [(&str, &str); 9] = [
    ("<name>", "room name"),
    ("<type>", "room type"),
    ("<area>", "interior area"),
    ("<std_area>", "standard area"),
    ("<cl_area>", "centerline area"),
    ("<dims>", "interior dimensions"),
    ("<ceiling>", "ceiling height"),
    ("<floor>", "floor name"),
    ("<perimeter>", "interior perimeter"),
];

/// Replaces the label macros of `template` with `vals`. Unknown text stays
/// as typed; `\n` in the template starts a new line.
pub fn expand_label_macros(template: &str, vals: &LabelValues) -> String {
    let pairs = [
        ("<name>", &vals.name),
        ("<type>", &vals.room_type),
        ("<area>", &vals.area),
        ("<std_area>", &vals.standard_area),
        ("<cl_area>", &vals.centerline_area),
        ("<dims>", &vals.dims),
        ("<ceiling>", &vals.ceiling),
        ("<floor>", &vals.floor),
        ("<perimeter>", &vals.perimeter),
    ];
    let mut out = template.replace("\\n", "\n");
    for (token, value) in pairs {
        out = out.replace(token, value);
    }
    out
}

fn sq_ft_text(v: f64) -> String {
    format!("{} sq ft", v.round())
}

/// The macro values of `room` on the active floor.
pub fn label_values(cx: &EditorContext, room: &Room) -> LabelValues {
    let entry = name_entry(cx, room);
    let outline = if room.inner_polygon.len() >= 3 {
        &room.inner_polygon
    } else {
        &room.polygon
    };
    let perimeter: f64 = (0..outline.len())
        .map(|i| outline[i].dist(outline[(i + 1) % outline.len()]))
        .sum();
    LabelValues {
        name: cx.room_name(room),
        room_type: entry.map_or_else(String::new, |n| n.room_type.clone()),
        area: sq_ft_text(room.interior_area_sq_ft()),
        standard_area: sq_ft_text(room.standard_area_sq_ft()),
        centerline_area: sq_ft_text(room.area_sq_ft()),
        dims: interior_dims_text(cx, room),
        ceiling: cx.fmt_dim(
            entry
                .and_then(|n| n.ceiling_height)
                .unwrap_or(cx.floor().ceiling_height),
        ),
        floor: cx.floor().name.clone(),
        perimeter: cx.fmt_dim(perimeter),
    }
}

/// The label text of a room per the label options stored with it: a macro
/// template when it has one, else the name, then the interior dimensions and
/// the area (interior, standard or centerline) when checked. Empty when
/// nothing is shown (R-50).
pub fn room_label_text(cx: &EditorContext, room: &Room) -> String {
    let opts = label_options(cx, room);
    if !opts.template.trim().is_empty() {
        return expand_label_macros(&opts.template, &label_values(cx, room));
    }
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

// ----- draggable labels (R-44) -----

/// Plan label font size, points (the size `draw_rooms` uses).
pub const LABEL_FONT_PX: f64 = 13.0;

/// Where `room`'s label is drawn: the room's label point plus the offset the
/// label was dragged by.
pub fn label_position(cx: &EditorContext, room: &Room) -> Point {
    match name_entry(cx, room) {
        Some(n) => room.label_point(n.label_style.placement, LABEL_INSET) + n.label.offset,
        None => room_anchor(room),
    }
}

/// How far in from the wall a label placed near a wall sits, inches.
pub const LABEL_INSET: f64 = 18.0;

/// The text style `room`'s label is drawn in: the one its Label tab names,
/// else the Room Label Style (R-46).
pub fn label_text_style(cx: &EditorContext, room: &Room) -> String {
    name_entry(cx, room).map_or_else(
        || plan_core::rooms::ROOM_LABEL_TEXT_STYLE.to_string(),
        |n| n.label_style.style_name().to_string(),
    )
}

/// Half the width and height of `room`'s label box in plan inches at the
/// current zoom.
fn label_half_extent(cx: &EditorContext, text: &str) -> (f64, f64) {
    let ppi = cx.px_per_in.max(1e-6);
    let lines = text.lines().count().max(1) as f64;
    let chars = text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64;
    (
        (chars * LABEL_FONT_PX * 0.28 + 3.0) / ppi,
        (lines * LABEL_FONT_PX * 0.6 + 2.0) / ppi,
    )
}

/// The room whose label is under `p`, if any (nested rooms first).
pub fn label_at(cx: &EditorContext, p: Point) -> Option<usize> {
    if !cx.layers().is_visible("Room Labels") {
        return None;
    }
    let mut best: Option<(usize, f64)> = None;
    for (i, room) in cx.rooms.iter().enumerate() {
        let text = room_label_text(cx, room);
        if text.trim().is_empty() {
            continue;
        }
        let c = label_position(cx, room);
        let (hw, hh) = label_half_extent(cx, &text);
        if (p.x - c.x).abs() <= hw
            && (p.y - c.y).abs() <= hh
            && best.is_none_or(|(_, a)| room.area_sq_in < a)
        {
            best = Some((i, room.area_sq_in));
        }
    }
    best.map(|(i, _)| i)
}

/// Index into the active floor's `room_names` of room `idx`'s entry, creating
/// the entry (the floor's default room type) when the room has none.
fn ensure_name_entry(cx: &mut EditorContext, idx: usize) -> Option<usize> {
    let room = cx.rooms.get(idx)?.clone();
    if let Some(e) = name_entry(cx, &room) {
        let anchor = e.anchor;
        return cx
            .floor()
            .room_names
            .iter()
            .position(|n| n.anchor == anchor);
    }
    let anchor = room_anchor(&room);
    let ty = draft_room_type(cx);
    let (fl, rooms) = (cx.floor, cx.rooms.clone());
    cx.project
        .set_room_name(fl, anchor, room.label.clone(), ty, &rooms);
    cx.project.floors[fl]
        .room_names
        .iter()
        .position(|n| n.anchor == anchor)
}

/// Moves room `idx`'s label to `offset` from its label point as one undo step
/// (the name entry is created when the room has none).
#[cfg_attr(not(test), allow(dead_code))]
pub fn set_label_offset(cx: &mut EditorContext, idx: usize, offset: Point) -> bool {
    cx.begin_change("Move Room Label");
    let Some(i) = ensure_name_entry(cx, idx) else {
        cx.cancel_change();
        return false;
    };
    let fl = cx.floor;
    cx.project.floors[fl].room_names[i].label.offset = offset;
    cx.mark_dirty();
    true
}

/// Pointer pressed on a label: selects its room and starts the drag. False
/// when no label is under `world`.
pub fn label_pointer_down(cx: &mut EditorContext, world: Point) -> bool {
    let Some(idx) = label_at(cx, world) else {
        return false;
    };
    let room = cx.rooms[idx].clone();
    let drag = LabelDrag {
        floor: cx.floor,
        room_point: room_anchor(&room),
        grab: world,
        start_offset: name_entry(cx, &room).map_or(Point::ZERO, |n| n.label.offset),
        begun: false,
    };
    select_room(cx, idx);
    with(|s| s.label_drag = Some(drag));
    true
}

/// Is a label drag running?
pub fn label_dragging() -> bool {
    with(|s| s.label_drag.is_some())
}

/// Pointer moved during a label drag. The first real movement opens the undo
/// step.
pub fn label_pointer_move(cx: &mut EditorContext, world: Point) -> bool {
    let Some(mut d) = with(|s| s.label_drag) else {
        return false;
    };
    let delta = world - d.grab;
    if !d.begun && delta.length() < 2.0 / cx.px_per_in.max(1e-6) {
        return true;
    }
    let Some(idx) = (d.floor == cx.floor)
        .then(|| room_index_at(cx, d.room_point))
        .flatten()
    else {
        with(|s| s.label_drag = None);
        return false;
    };
    if !d.begun {
        cx.begin_change("Move Room Label");
        d.begun = true;
        with(|s| s.label_drag = Some(d));
    }
    if let Some(i) = ensure_name_entry(cx, idx) {
        let fl = cx.floor;
        cx.project.floors[fl].room_names[i].label.offset = d.start_offset + delta;
        cx.mark_dirty();
    }
    true
}

/// Pointer released: ends a label drag. True when one was running.
pub fn label_pointer_up() -> bool {
    with(|s| s.label_drag.take().is_some())
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

/// Living Area of each floor, square feet (interior areas), in floor order.
pub fn living_area_by_floor(cx: &EditorContext) -> Vec<(String, f64)> {
    cx.project
        .floors
        .iter()
        .map(|f| {
            let area: f64 = detect_rooms(&f.walls, 0.5)
                .iter()
                .filter(|r| counts_as_living(&cx.defaults, r.name_entry(&f.room_names)))
                .map(Room::interior_area_sq_ft)
                .sum();
            (f.name.clone(), area)
        })
        .collect()
}

/// Total Living Area over all floors, square feet (interior areas).
pub fn living_area_total_sq_ft(cx: &EditorContext) -> f64 {
    living_area_by_floor(cx).iter().map(|(_, a)| a).sum()
}

/// The total living area readout (R-52): the total, then each floor that has
/// any, as in "Total living area 1,820 sq ft (1st Floor 1,200, 2nd Floor 620)".
pub fn living_area_report(cx: &EditorContext) -> String {
    let by_floor = living_area_by_floor(cx);
    let total: f64 = by_floor.iter().map(|(_, a)| a).sum();
    let parts: Vec<String> = by_floor
        .iter()
        .filter(|(_, a)| *a >= 0.5)
        .map(|(n, a)| format!("{n} {}", group_thousands(a.round() as i64)))
        .collect();
    let mut out = format!(
        "Total living area {} sq ft",
        group_thousands(total.round() as i64)
    );
    if parts.len() > 1 {
        out.push_str(&format!(" ({})", parts.join(", ")));
    }
    out
}

/// 1820 as "1,820".
fn group_thousands(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

// ----- the Room Specification (R-19..R-36) -----

/// The room type a new room on the active floor starts with: the floor's
/// default room type (Floor Defaults) when it names a known type, else the
/// first of the plan's types.
pub fn draft_room_type(cx: &EditorContext) -> String {
    let wanted = &cx.floor().settings.default_room_type;
    if !wanted.is_empty() && cx.defaults.room_type(wanted).is_some() {
        return wanted.clone();
    }
    cx.defaults
        .rooms
        .room_types
        .first()
        .map_or(String::new(), |t| t.name.clone())
}

/// The draft a room without a name entry starts the Room Specification from:
/// the floor's default type and materials and finish thicknesses (Floor
/// Defaults, R-56); when the floor names a default type its function sets the
/// platform defaults too (R-41).
fn new_room_draft(cx: &EditorContext, room: &Room) -> (RoomName, RoomExtras) {
    let st = &cx.floor().settings;
    let ty = draft_room_type(cx);
    let mut name = RoomName::new(room_anchor(room), room.label.clone(), ty.clone());
    if !st.floor_material.is_empty() {
        name.floor_finish = Some(st.floor_material.clone());
    }
    if !st.ceiling_material.is_empty() {
        name.ceiling_finish = Some(st.ceiling_material.clone());
    }
    let mut extras = extras_for(cx, room);
    extras.floor_finish_thickness = st.floor_finish_thickness;
    extras.ceiling_finish_thickness = st.ceiling_finish_thickness;
    if !st.default_room_type.is_empty() {
        if let Some(def) = cx.defaults.room_type(&ty) {
            let fd = function_defaults(&def.function, &ty);
            apply_function_defaults(&mut name, &fd, st.floor_finish_thickness);
            if let Some(m) = name.misc.take() {
                extras.floor_finish_thickness = m.floor_finish_thickness;
                extras.floor_structure = m.floor_structure;
            }
        }
    }
    (name, extras)
}

/// The ceiling height a room on the active floor gets when it names none,
/// measured from its own floor `offset` above the floor datum: the floor's
/// ceiling height, or on a foundation floor with a basement the clear height
/// under the first floor's platform.
fn default_ceiling_height(cx: &EditorContext, offset: f64) -> f64 {
    let f = cx.floor();
    if f.kind == FloorKind::Foundation && f.settings.ceiling_structure_thickness > 0.0 {
        (f.ceiling_height - f.settings.ceiling_structure_thickness - offset).max(1.0)
    } else {
        f.ceiling_height
    }
}

/// Everything the Room Specification dialog starts from.
pub fn room_dialog_init(cx: &EditorContext, idx: usize) -> Option<RoomInit> {
    let room = cx.rooms.get(idx)?;
    let (name, extras) = match name_entry(cx, room) {
        Some(e) => (e.clone(), extras_for(cx, room)),
        None => new_room_draft(cx, room),
    };
    let outline = if room.inner_polygon.len() >= 3 {
        room.inner_polygon.clone()
    } else {
        room.polygon.clone()
    };
    let perimeter: f64 = (0..outline.len())
        .map(|i| outline[i].dist(outline[(i + 1) % outline.len()]))
        .sum();
    let ceiling_default = default_ceiling_height(cx, name.floor_height_offset);
    Some(RoomInit {
        room_index: idx,
        name,
        extras,
        types: cx.defaults.rooms.room_types.clone(),
        polygon: outline,
        interior_dims: interior_dims_text(cx, room),
        interior_area_sq_ft: room.interior_area_sq_ft(),
        standard_area_sq_ft: room.standard_area_sq_ft(),
        perimeter_in: perimeter,
        floor_elevation: cx.floor().elevation,
        floor_ceiling_height: ceiling_default,
        default_floor_finish: cx.floor().settings.floor_finish_thickness,
        default_name: room.label.clone(),
        total_living_sq_ft: living_area_total_sq_ft(cx),
        floor_name: cx.floor().name.clone(),
        text_styles: cx
            .project
            .text_styles
            .names()
            .into_iter()
            .map(String::from)
            .collect(),
        slab_allowed: cx.floor().kind == FloorKind::Normal,
        default_floor_material: cx.floor().settings.floor_material.clone(),
        default_ceiling_material: cx.floor().settings.ceiling_material.clone(),
        default_wall_material: cx.floor().settings.wall_material.clone(),
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
    let anchor = if room.contains(draft.anchor) {
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
        n.monolithic_slab = draft.monolithic_slab;
        n.label_style = draft.label_style.clone();
        // Roof Group (R-114): which building the room's roof belongs to.
        n.roof_group = draft.roof_group;
        // The Deck tab (CB-86).
        n.deck = draft.deck.clone();
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
    cx.status = format!("Updated the room. {}", living_area_report(cx));
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

/// The hatch segments the plan draws (see [`hatch_segments`]), for tests.
#[cfg(test)]
pub fn hatch_lines(
    poly: &[Point],
    holes: &[Vec<Point>],
    n: Point,
    spacing: f64,
) -> Vec<(Point, Point)> {
    hatch_segments(poly, holes, n, spacing)
}

/// Parallel hatch segments across `poly` minus `holes`: lines where `n . p`
/// is a multiple of `spacing`, clipped to the polygon by pairing the edge
/// crossings (the holes' edges make the gaps).
fn hatch_segments(
    poly: &[Point],
    holes: &[Vec<Point>],
    n: Point,
    spacing: f64,
) -> Vec<(Point, Point)> {
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
        for ring in std::iter::once(poly).chain(holes.iter().map(Vec::as_slice)) {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                let (fa, fb) = (f(a), f(b));
                if (fa - c) * (fb - c) < 0.0 {
                    hits.push(Point::lerp(a, b, (c - fa) / (fb - fa)));
                }
            }
        }
        hits.sort_by(|p, q| g(*p).total_cmp(&g(*q)));
        for (a, b) in hits.iter().step_by(2).zip(hits.iter().skip(1).step_by(2)) {
            out.push((*a, *b));
        }
    }
    out
}

fn draw_fill(
    painter: &egui::Painter,
    cam: &Camera,
    poly: &[Point],
    holes: &[Vec<Point>],
    fill: FillStyle,
) {
    if fill.pattern == FillPattern::None || poly.len() < 3 {
        return;
    }
    let [r, g, b] = fill.color;
    let alpha = |base: f32| (base * fill.alpha.clamp(0.0, 1.0)).round() as u8;
    match fill.pattern {
        FillPattern::Solid => {
            let col = Color32::from_rgba_unmultiplied(r, g, b, alpha(70.0));
            let mut mesh = Mesh::default();
            if holes.is_empty() {
                let pts = screen_poly(cam, poly);
                for p in &pts {
                    mesh.colored_vertex(*p, col);
                }
                for t in triangulate(poly) {
                    mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
                }
            } else {
                // Nested rooms (R-11) are left open.
                for tri in plan_3d::foundation::cut_platform(poly, holes) {
                    let base = mesh.vertices.len() as u32;
                    for p in tri {
                        mesh.colored_vertex(cam.world_to_screen(p), col);
                    }
                    mesh.add_triangle(base, base + 1, base + 2);
                }
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
                for (a, b) in hatch_segments(poly, holes, *d, spacing) {
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
        draw_fill(painter, cam, poly, &room.holes, fill);
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
    /// Grade Beams on Piers.
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
            FoundationType::Piers => "Grade Beams on Piers",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FoundationSpec {
    pub kind: FoundationType,
    /// Walls with Footings: the height of the foundation walls, bottom to the
    /// first floor, inches.
    pub stem_height: f64,
    pub min_stem_height: f64,
    /// Not stored by the model yet.
    pub garage_floor: bool,
    /// Walls with Footings: a footing under the walls.
    pub footing: bool,
    pub footing_width: f64,
    pub footing_depth: f64,
    /// Monolithic Slab: the slab's thickness and the stem wall (thickened
    /// edge) height, inches.
    pub slab_thickness: f64,
    pub slab_stem_height: f64,
    /// Grade Beams on Piers: beam height, pier height and the spacing of the
    /// piers along the walls, inches.
    pub beam_height: f64,
    pub pier_height: f64,
    pub pier_spacing: f64,
    /// Walls with Footings: the room made inside (a basement from the
    /// wall height up, else a crawl space).
    pub rooms: FoundationRooms,
}

impl FoundationSpec {
    pub fn from_defaults(d: &PlanDefaults) -> Self {
        let o = FoundationOptions::new(FoundationKind::MonolithicSlab);
        Self {
            kind: FoundationType::WallsWithFootings,
            stem_height: d.foundation_wall.height,
            min_stem_height: 12.0,
            garage_floor: true,
            footing: true,
            // The footing starts at the code-legal size of the plan's defaults.
            footing_width: d.code.footing_width,
            footing_depth: d.code.footing_thickness,
            slab_thickness: o.slab_thickness,
            slab_stem_height: o.edge_height,
            beam_height: plan_core::floors::GRADE_BEAM_HEIGHT,
            pier_height: o.pier_height,
            pier_spacing: o.pier_spacing,
            rooms: FoundationRooms::Auto,
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

    /// What the model builds from these choices.
    pub fn to_options(self) -> FoundationOptions {
        let footed = self.footing && self.kind == FoundationType::WallsWithFootings;
        FoundationOptions {
            kind: self.to_kind(),
            footing_width: if footed { self.footing_width } else { 0.0 },
            footing_depth: if footed { self.footing_depth } else { 0.0 },
            slab_thickness: self.slab_thickness,
            edge_height: match self.kind {
                FoundationType::Piers => self.beam_height,
                _ => self.slab_stem_height,
            },
            pier_height: self.pier_height,
            pier_spacing: self.pier_spacing,
            rooms: self.rooms,
        }
    }

    /// Why the dialog cannot be accepted, if so.
    pub fn error(&self) -> Option<&'static str> {
        match self.kind {
            FoundationType::WallsWithFootings => {
                if self.stem_height <= 0.0 {
                    Some("Stem wall height must be greater than zero")
                } else if self.footing && (self.footing_width <= 0.0 || self.footing_depth <= 0.0) {
                    Some("The footing needs a width and a depth")
                } else {
                    None
                }
            }
            FoundationType::MonolithicSlab => {
                if self.slab_thickness <= 0.0 {
                    Some("The slab thickness must be greater than zero")
                } else if self.slab_stem_height <= 0.0 {
                    Some("The stem wall height must be greater than zero")
                } else {
                    None
                }
            }
            FoundationType::Piers => {
                if self.beam_height <= 0.0 || self.pier_height <= 0.0 {
                    Some("The beams and piers need a height")
                } else if self.pier_spacing < 12.0 {
                    Some("The piers need at least a foot between them")
                } else {
                    None
                }
            }
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

/// The choices of the Build New Floor dialog (R-59, R-61).
#[derive(Clone, Debug, PartialEq)]
pub struct NewFloorSpec {
    /// What to copy from the current floor.
    pub derive: DeriveFrom,
    pub place: FloorPlacement,
    /// Copy the current floor's room name entries.
    pub copy_rooms: bool,
    /// Copy the current floor's slab, pad and pier data.
    pub copy_foundation: bool,
    /// Take the ceiling height and Floor Defaults from the plan defaults
    /// instead of the current floor.
    pub heights_from_defaults: bool,
    /// Build this foundation too when the plan has none.
    pub foundation: Option<FoundationSpec>,
    /// Build the attic floor too (R-68).
    pub attic: bool,
}

impl NewFloorSpec {
    /// Chief's starting choices: derive the exterior walls, put the floor
    /// above, heights from the Floor Defaults.
    pub fn new() -> Self {
        Self {
            derive: DeriveFrom::ExteriorWalls,
            place: FloorPlacement::Above,
            copy_rooms: false,
            copy_foundation: false,
            heights_from_defaults: true,
            foundation: None,
            attic: false,
        }
    }
}

impl Default for NewFloorSpec {
    fn default() -> Self {
        Self::new()
    }
}

/// Does the plan have a foundation floor?
pub fn has_foundation(cx: &EditorContext) -> bool {
    cx.project
        .floors
        .first()
        .is_some_and(|f| f.kind == FloorKind::Foundation)
}

/// Build New Floor with the dialog's full set of choices (R-59): derives from
/// the current floor (the top normal floor when that is the foundation),
/// places the new floor above or below it, takes heights from the Floor
/// Defaults when asked and builds a foundation when the plan has none and one
/// was chosen. One undo step; the new floor becomes the active one.
pub fn build_new_floor_with(cx: &mut EditorContext, spec: &NewFloorSpec) -> Option<usize> {
    let cur = cx.floor;
    let source = (cx.project.floors.get(cur)?.kind != FloorKind::Foundation).then_some(cur);
    let mut opts = NewFloorOptions {
        source,
        place: spec.place,
        derive: spec.derive,
        copy_rooms: spec.copy_rooms,
        copy_foundation: spec.copy_foundation,
        ceiling_height: None,
        settings: None,
    };
    if spec.heights_from_defaults {
        opts.ceiling_height = Some(cx.defaults.rooms.ceiling_height);
        opts.settings = Some(cx.defaults.rooms.floor.clone());
    }
    let build_foundation = spec.foundation.filter(|_| !has_foundation(cx));
    cx.begin_change("Build New Floor");
    let Some(mut idx) = cx.project.build_new_floor_with(&opts) else {
        cx.cancel_change();
        cx.status = match spec.place {
            FloorPlacement::Above => "Cannot build a floor above the attic".into(),
            FloorPlacement::Below => "Cannot build a floor below the foundation".into(),
        };
        return None;
    };
    if let Some(f) = build_foundation {
        cx.project.build_foundation_with(&f.to_options());
        add_foundation_room_types(cx);
        idx += 1;
    }
    if spec.attic {
        cx.project.build_attic_floor();
    }
    after_floor_change(cx, idx);
    cx.status = format!("Built {}", cx.project.floors[idx].name);
    Some(idx)
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

/// Insert New Floor Below (R-60): an empty floor under the current one.
pub fn insert_floor_below(cx: &mut EditorContext) -> Option<usize> {
    cx.begin_change("Insert New Floor Below");
    match cx.project.insert_floor_below(cx.floor) {
        Some(idx) => {
            after_floor_change(cx, idx);
            cx.status = format!("Inserted {}", cx.project.floors[idx].name);
            Some(idx)
        }
        None => {
            cx.cancel_change();
            cx.status = "Cannot insert a floor below the foundation".into();
            None
        }
    }
}

/// Floor Defaults (R-56, R-58): sets the ceiling height and settings of the
/// active floor as one undo step; walls at the old ceiling height follow and
/// the floors above move by the change. With `as_plan_default` the plan
/// defaults take them too, for floors built from now on.
pub fn apply_floor_defaults(
    cx: &mut EditorContext,
    ceiling_height: f64,
    settings: FloorSettings,
    as_plan_default: bool,
) -> bool {
    cx.begin_change("Floor Defaults");
    let fl = cx.floor;
    if !cx
        .project
        .apply_floor_settings(fl, ceiling_height, settings.clone())
    {
        cx.cancel_change();
        return false;
    }
    if as_plan_default {
        set_plan_floor_defaults(cx, ceiling_height, settings);
    }
    cx.mark_dirty();
    cx.refresh();
    cx.status = "Updated the floor defaults".into();
    true
}

/// Edit > Default Settings > Floors and Rooms > Floor Defaults: the ceiling
/// height and settings every floor built from now on starts with.
pub fn set_plan_floor_defaults(
    cx: &mut EditorContext,
    ceiling_height: f64,
    settings: FloorSettings,
) {
    cx.defaults.rooms.ceiling_height = ceiling_height;
    cx.defaults.rooms.floor_finish_thickness = settings.floor_finish_thickness;
    cx.defaults.rooms.ceiling_finish_thickness = settings.ceiling_finish_thickness;
    cx.defaults.rooms.floor = settings.without_foundation();
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
    cx.project.build_foundation_with(&spec.to_options());
    add_foundation_room_types(cx);
    let next = if had { cx.floor } else { cx.floor + 1 };
    after_floor_change(cx, next);
    let rooms = cx
        .project
        .floors
        .first()
        .and_then(|f| f.room_names.first())
        .map(|n| format!(" with a {}", n.room_type.to_lowercase()))
        .unwrap_or_default();
    cx.status = format!("Built the foundation ({}){rooms}", spec.kind.name());
}

/// Makes sure the room types the foundation floor's rooms use (Basement,
/// Crawl Space) are in the plan's Room Types list, so the Room Specification
/// can show them (R-18).
fn add_foundation_room_types(cx: &mut EditorContext) {
    let wanted: Vec<String> = cx
        .project
        .floors
        .iter()
        .filter(|f| f.kind == FloorKind::Foundation)
        .flat_map(|f| f.room_names.iter().map(|n| n.room_type.clone()))
        .collect();
    for t in wanted {
        if cx.defaults.room_type(&t).is_none() {
            cx.defaults
                .rooms
                .room_types
                .push(plan_core::defaults::RoomTypeDef {
                    name: t.clone(),
                    function: t.clone(),
                    include_in_living_area: false,
                    conditioned: t == "Basement",
                    default_floor_finish: String::new(),
                });
        }
    }
}

/// Build Attic Floor (R-68): the attic floor above the top floor with attic
/// walls over its exterior walls and an Attic room inside them, or the
/// existing one brought up to date. One undo step; the attic becomes the
/// active floor.
pub fn build_attic_floor(cx: &mut EditorContext) -> Option<usize> {
    cx.begin_change("Build Attic Floor");
    match cx.project.build_attic_floor() {
        Some(idx) => {
            after_floor_change(cx, idx);
            cx.status = "Built the attic floor".into();
            Some(idx)
        }
        None => {
            cx.cancel_change();
            cx.status = "The attic needs a floor with exterior walls to sit on".into();
            None
        }
    }
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
    // One undo step when the restack changes the plan, none when it does not
    // (QA-25).
    cx.undo_group(|cx| {
        cx.begin_change("Rebuild All");
        cx.project.restack_floors();
    });
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
        "Plan Footprint: {:.0} sq ft, {:.1} ft around. {}",
        fp.area_sq_ft,
        fp.perimeter_ft,
        living_area_report(cx)
    );
    Some(fp.area_sq_ft)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::rooms::LabelPlacement;
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
        extras.roof_over = false;
        extras.flat_roof = true;
        extras.ceiling_height_absolute = true;
        extras.floor_finish_thickness = 0.75;
        extras.wall_covering = "Wainscot".into();
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
        assert!(!back.roof_over && back.flat_roof && back.ceiling_height_absolute);
        assert_eq!(back.floor_finish_thickness, 0.75);
        assert_eq!(back.wall_covering, "Wainscot");
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

    /// The highest and lowest vertex height of the floor platform meshes at
    /// plan x in `[x0, x1]`.
    fn floor_span(cx: &EditorContext, x0: f32, x1: f32) -> (f32, f32) {
        let scene = plan_3d::build_scene(&cx.project);
        let ys: Vec<f32> = scene
            .meshes
            .iter()
            .filter(|m| m.material == plan_3d::Material::Floor)
            .flat_map(|m| m.vertices.iter())
            .filter(|v| v.position[0] >= x0 && v.position[0] <= x1)
            .map(|v| v.position[1])
            .collect();
        (
            ys.iter().copied().fold(f32::MIN, f32::max),
            ys.iter().copied().fold(f32::MAX, f32::min),
        )
    }

    /// Names room 0 through the Room Specification dialog as `ty`.
    fn name_room_through_dialog(cx: &mut EditorContext, ty: &str) {
        let init = room_dialog_init(cx, 0).unwrap();
        let mut d = crate::dialogs::room::RoomDialog::new(init);
        d.set_room_type(ty);
        let draft = d.room_name().clone();
        assert!(apply_room_spec(cx, 0, &draft, d.extras()));
    }

    #[test]
    fn a_garage_drops_its_3d_floor_24_inches_through_the_dialog() {
        let mut cx = house();
        let (house_top, _) = floor_span(&cx, 0.0, 240.0);
        name_room_through_dialog(&mut cx, "Garage");
        let (top, low) = floor_span(&cx, 0.0, 240.0);
        // 24" down, and the finish layer goes (the slab is bare concrete).
        assert!((top - (house_top - 24.0 - 0.75)).abs() < 0.01, "{top}");
        assert!((top - low - 4.0).abs() < 0.01, "a 4 in slab");
        // The values are the room's own, editable afterwards.
        let n = name_entry(&cx, &cx.rooms[0].clone()).unwrap().clone();
        assert_eq!(n.floor_height_offset, -24.0);
        name_room_through_dialog(&mut cx, "Bedroom");
        let (top, _) = floor_span(&cx, 0.0, 240.0);
        assert!((top - house_top).abs() < 0.01, "back to the house floor");
    }

    #[test]
    fn a_deck_has_no_ceiling_and_open_below_has_no_floor_in_3d() {
        let count = |cx: &EditorContext, m: plan_3d::Material| {
            plan_3d::build_scene(&cx.project)
                .meshes
                .iter()
                .filter(|x| x.material == m)
                .count()
        };
        let mut cx = house();
        assert_eq!(count(&cx, plan_3d::Material::Ceiling), 1);
        name_room_through_dialog(&mut cx, "Deck");
        assert_eq!(
            count(&cx, plan_3d::Material::Ceiling),
            0,
            "no ceiling platform"
        );
        assert_eq!(count(&cx, plan_3d::Material::Floor), 1, "a deck platform");
        name_room_through_dialog(&mut cx, "Open Below");
        assert_eq!(count(&cx, plan_3d::Material::Floor), 0, "no floor platform");
        assert_eq!(count(&cx, plan_3d::Material::Ceiling), 1);
        // The Structure switches are the dialog's to flip afterwards.
        let init = room_dialog_init(&cx, 0).unwrap();
        let mut draft = init.name.clone();
        draft.has_floor = true;
        assert!(apply_room_spec(&mut cx, 0, &draft, &init.extras));
        assert_eq!(count(&cx, plan_3d::Material::Floor), 1);
    }

    #[test]
    fn the_structure_define_changes_the_platform_thickness() {
        let mut cx = house();
        let init = room_dialog_init(&cx, 0).unwrap();
        let mut d = crate::dialogs::room::RoomDialog::new(init);
        d.structure_mut(crate::dialogs::room::Define::Floor)
            .extend([
                StructureLayer::new("Subfloor", 0.75),
                StructureLayer::new("Joist", 9.25),
            ]);
        let draft = d.room_name().clone();
        assert!(apply_room_spec(&mut cx, 0, &draft, d.extras()));
        let (top, low) = floor_span(&cx, 0.0, 240.0);
        assert!((top - low - 10.0).abs() < 0.01, "{}", top - low);
        // The layers come back in the dialog and survive a save.
        let again = room_dialog_init(&cx, 0).unwrap();
        assert_eq!(again.extras.floor_structure.len(), 2);
        let json = serde_json::to_string(&cx.project).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        let misc = back.floors[0].room_names[0].misc.clone().unwrap();
        assert_eq!(misc.floor_structure.len(), 2);
    }

    #[test]
    fn floor_defaults_change_what_new_rooms_start_with() {
        let mut cx = house();
        let before = room_dialog_init(&cx, 0).unwrap();
        assert_eq!(before.floor_ceiling_height, cx.floor().ceiling_height);
        let settings = FloorSettings {
            default_room_type: "Garage".into(),
            floor_finish_thickness: 0.5,
            floor_material: "Oak".into(),
            ..FloorSettings::default()
        };
        assert!(apply_floor_defaults(&mut cx, 96.0, settings, false));
        let init = room_dialog_init(&cx, 0).unwrap();
        assert_eq!(init.floor_ceiling_height, 96.0);
        assert_eq!(init.name.room_type, "Garage");
        assert_eq!(init.name.floor_finish.as_deref(), Some("Oak"));
        assert_eq!(init.default_floor_finish, 0.5);
        // The Garage function brought its platform defaults along.
        assert_eq!(init.name.floor_height_offset, -24.0);
        // Walls at the old ceiling height came down with it.
        let scene = plan_3d::build_scene(&cx.project);
        let ceil_y = scene
            .meshes
            .iter()
            .filter(|m| m.material == plan_3d::Material::Ceiling)
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .fold(f32::MAX, f32::min);
        assert!((ceil_y - 96.0).abs() < 0.01, "{ceil_y}");
        // One undo step brings the old values back.
        assert!(cx.can_undo());
        cx.undo();
        assert_eq!(cx.floor().ceiling_height, before.floor_ceiling_height);
    }

    #[test]
    fn plan_floor_defaults_set_the_next_floor_built() {
        let mut cx = house();
        let st = FloorSettings {
            floor_structure_thickness: 12.0,
            ..FloorSettings::default()
        };
        set_plan_floor_defaults(&mut cx, 120.0, st.clone());
        let idx = build_new_floor_with(&mut cx, &NewFloorSpec::new()).unwrap();
        assert_eq!(cx.project.floors[idx].ceiling_height, 120.0);
        assert_eq!(cx.project.floors[idx].settings, st);
        let want = cx.project.floors[0].ceiling_height + 12.0;
        assert!((cx.project.floors[idx].elevation - want).abs() < 1e-9);
        // Same as the floor below keeps the heights of the floor below.
        let spec = NewFloorSpec {
            heights_from_defaults: false,
            ..NewFloorSpec::new()
        };
        cx.floor = 0;
        let next = build_new_floor_with(&mut cx, &spec).unwrap();
        assert_eq!(
            cx.project.floors[next].ceiling_height,
            cx.project.floors[0].ceiling_height
        );
    }

    #[test]
    fn build_new_floor_derives_exterior_only_all_walls_or_blank() {
        let mut cx = house();
        cx.project.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 180.0),
            4.5,
            109.0,
            WallKind::Interior,
        );
        let exterior = build_new_floor_with(&mut cx, &NewFloorSpec::new()).unwrap();
        assert_eq!(cx.project.floors[exterior].walls.len(), 4);
        assert_eq!(cx.floor, exterior);
        cx.floor = 0;
        let all = build_new_floor_with(
            &mut cx,
            &NewFloorSpec {
                derive: DeriveFrom::AllWalls,
                ..NewFloorSpec::new()
            },
        )
        .unwrap();
        assert_eq!(cx.project.floors[all].walls.len(), 5);
        cx.floor = 0;
        let blank = build_new_floor_with(
            &mut cx,
            &NewFloorSpec {
                derive: DeriveFrom::Blank,
                ..NewFloorSpec::new()
            },
        )
        .unwrap();
        assert!(cx.project.floors[blank].walls.is_empty());
        // One undo step per command.
        cx.undo();
        assert_eq!(cx.project.floors.len(), 3);
    }

    #[test]
    fn build_new_floor_below_and_with_a_foundation() {
        let mut cx = house();
        let below = build_new_floor_with(
            &mut cx,
            &NewFloorSpec {
                place: FloorPlacement::Below,
                derive: DeriveFrom::Blank,
                ..NewFloorSpec::new()
            },
        )
        .unwrap();
        assert_eq!(below, 0);
        assert_eq!(cx.project.floors.len(), 2);
        assert!(
            cx.project.floors[1].walls.len() == 4,
            "the old floor moved up"
        );
        // A foundation is built too when the plan has none and one was chosen.
        let mut cx = house();
        let spec = NewFloorSpec {
            foundation: Some(FoundationSpec::from_defaults(&cx.defaults)),
            ..NewFloorSpec::new()
        };
        let idx = build_new_floor_with(&mut cx, &spec).unwrap();
        assert!(has_foundation(&cx));
        assert_eq!(cx.project.floors.len(), 3);
        assert_eq!(idx, 2);
        assert_eq!(cx.floor, 2);
        // With a foundation already there, the option does nothing.
        let again = build_new_floor_with(&mut cx, &spec).unwrap();
        assert_eq!(cx.project.floors.len(), 4);
        assert_eq!(again, 3);
        // Nothing is built below the foundation floor itself.
        cx.floor = 0;
        assert!(insert_floor_below(&mut cx).is_none());
        cx.floor = 1;
        assert_eq!(insert_floor_below(&mut cx), Some(1));
    }

    /// A 240 x 180 room with a free-standing 60 x 36 closet loop inside it.
    fn house_with_closet() -> EditorContext {
        let mut cx = house();
        let k = [
            Point::new(60.0, 60.0),
            Point::new(120.0, 60.0),
            Point::new(120.0, 96.0),
            Point::new(60.0, 96.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, k[i], k[(i + 1) % 4], 4.5, 109.0, WallKind::Interior);
        }
        cx.mark_dirty();
        cx.refresh();
        cx
    }

    #[test]
    fn a_nested_closet_subtracts_from_the_room_around_it() {
        let cx = house_with_closet();
        assert_eq!(cx.rooms.len(), 2);
        let outer = cx.rooms.iter().find(|r| !r.holes.is_empty()).unwrap();
        let closet = cx.rooms.iter().find(|r| r.holes.is_empty()).unwrap();
        assert!((outer.area_sq_in - (240.0 * 180.0 - 60.0 * 36.0)).abs() < 1e-6);
        assert!((closet.area_sq_in - 60.0 * 36.0).abs() < 1e-6);
        // Picking inside the closet gives the closet, outside it the room.
        let (inside, around) = (Point::new(90.0, 78.0), Point::new(30.0, 30.0));
        let at = |p| room_index_at(&cx, p).map(|i| cx.rooms[i].area_sq_in);
        assert_eq!(at(inside), Some(closet.area_sq_in));
        assert_eq!(at(around), Some(outer.area_sq_in));
        // The living area counts the closet once.
        let total = living_area_total_sq_ft(&cx);
        let box_inner = (240.0 - 6.0) * (180.0 - 6.0) / 144.0;
        assert!(total < box_inner + 0.01, "{total} <= {box_inner}");
    }

    #[test]
    fn a_name_in_the_closet_belongs_to_the_closet() {
        let mut cx = house_with_closet();
        let outer_i = cx.rooms.iter().position(|r| !r.holes.is_empty()).unwrap();
        let closet_i = 1 - outer_i;
        let closet = cx.rooms[closet_i].clone();
        let draft = RoomName::new(room_anchor(&closet), "Walk-in", "Closet");
        let extras = extras_for(&cx, &closet);
        assert!(apply_room_spec(&mut cx, closet_i, &draft, &extras));
        let (outer, closet) = (cx.rooms[outer_i].clone(), cx.rooms[closet_i].clone());
        assert_eq!(name_entry(&cx, &closet).unwrap().name, "Walk-in");
        assert!(name_entry(&cx, &outer).is_none());
        assert_eq!(cx.room_name(&closet), "Walk-in");
        // Naming the room around it leaves the closet's name alone.
        let draft = RoomName::new(room_anchor(&outer), "Bedroom", "Bedroom");
        let extras = extras_for(&cx, &outer);
        assert!(apply_room_spec(&mut cx, outer_i, &draft, &extras));
        assert_eq!(cx.floor().room_names.len(), 2);
        let (outer, closet) = (cx.rooms[outer_i].clone(), cx.rooms[closet_i].clone());
        assert_eq!(cx.room_name(&closet), "Walk-in");
        assert_eq!(cx.room_name(&outer), "Bedroom");
    }

    #[test]
    fn a_dragged_label_offset_round_trips_and_is_picked() {
        let mut cx = house();
        cx.px_per_in = 1.0;
        let room = cx.rooms[0].clone();
        let home = label_position(&cx, &room);
        assert_eq!(home, room_anchor(&room));
        assert_eq!(label_at(&cx, home), Some(0));
        // Dragging with the pointer: press on the label, move, release.
        assert!(label_pointer_down(&mut cx, home));
        assert_eq!(selected_room(&cx), Some(0), "the press picks the room");
        assert!(label_dragging());
        assert!(label_pointer_move(
            &mut cx,
            Point::new(home.x + 40.0, home.y - 25.0)
        ));
        assert!(label_pointer_up());
        assert!(!label_dragging());
        let room = cx.rooms[0].clone();
        let moved = label_position(&cx, &room);
        assert!((moved.x - (home.x + 40.0)).abs() < 1e-9);
        assert!((moved.y - (home.y - 25.0)).abs() < 1e-9);
        assert_eq!(label_at(&cx, moved), Some(0));
        assert_eq!(label_at(&cx, Point::new(5000.0, 5000.0)), None);
        // The offset is part of the file.
        let json = serde_json::to_string(&cx.project).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        let n = &back.floors[0].room_names[0];
        assert_eq!(n.label.offset, Point::new(40.0, -25.0));
        // One undo step puts the label back.
        cx.undo();
        let room = cx.rooms[0].clone();
        assert_eq!(label_position(&cx, &room), home);
        // A press without a move changes nothing.
        assert!(label_pointer_down(&mut cx, home));
        assert!(label_pointer_up());
        assert!(cx.floor().room_names.is_empty());
        // Setting it directly, and resetting through the dialog's extras.
        assert!(set_label_offset(&mut cx, 0, Point::new(-10.0, 12.0)));
        let room = cx.rooms[0].clone();
        assert_eq!(
            label_position(&cx, &room),
            Point::new(home.x - 10.0, home.y + 12.0)
        );
        let mut extras = extras_for(&cx, &room);
        assert_eq!(extras.label.offset, Point::new(-10.0, 12.0));
        extras.label.offset = Point::ZERO;
        let draft = name_entry(&cx, &room).unwrap().clone();
        assert!(apply_room_spec(&mut cx, 0, &draft, &extras));
        let room = cx.rooms[0].clone();
        assert_eq!(label_position(&cx, &room), home);
    }

    #[test]
    fn label_macros_expand_to_the_rooms_values() {
        let mut cx = house();
        let room = cx.rooms[0].clone();
        let mut extras = extras_for(&cx, &room);
        extras.label.template = "<name> (<type>)\\n<dims> <area> h=<ceiling> <floor>".into();
        let mut draft = RoomName::new(room_anchor(&room), "Study", "Study");
        draft.ceiling_height = Some(120.0);
        assert!(apply_room_spec(&mut cx, 0, &draft, &extras));
        let room = cx.rooms[0].clone();
        let text = room_label_text(&cx, &room);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert_eq!(lines[0], "Study (Study)");
        assert!(
            lines[1].contains(" x ") && lines[1].contains("sq ft"),
            "{text}"
        );
        assert!(lines[1].contains(&cx.fmt_dim(120.0)), "{text}");
        assert!(lines[1].contains(&cx.floor().name), "{text}");
        assert!(!text.contains('<'), "{text}");
        // Unknown text stays, and an empty template falls back to the lines.
        let vals = label_values(&cx, &room);
        assert_eq!(expand_label_macros("<foo> <name>", &vals), "<foo> Study");
        extras.label.template.clear();
        assert!(apply_room_spec(&mut cx, 0, &draft, &extras));
        let room = cx.rooms[0].clone();
        assert!(room_label_text(&cx, &room).starts_with("Study\n"));
    }

    fn spec(kind: FoundationType) -> FoundationSpec {
        let mut s = FoundationSpec::from_defaults(&PlanDefaults::chief_x18_daniel());
        s.kind = kind;
        s
    }

    #[test]
    fn the_foundation_spec_hands_its_choices_to_the_model() {
        let mut s = spec(FoundationType::WallsWithFootings);
        s.stem_height = 96.0;
        s.footing_width = 24.0;
        s.footing_depth = 12.0;
        let o = s.to_options();
        assert_eq!(o.kind, FoundationKind::StemWall { height: 96.0 });
        assert_eq!((o.footing_width, o.footing_depth), (24.0, 12.0));
        s.footing = false;
        assert_eq!(s.to_options().footing_width, 0.0);
        s.kind = FoundationType::MonolithicSlab;
        s.slab_thickness = 5.0;
        s.slab_stem_height = 14.0;
        let o = s.to_options();
        assert_eq!(o.kind, FoundationKind::MonolithicSlab);
        assert_eq!((o.slab_thickness, o.edge_height), (5.0, 14.0));
        s.kind = FoundationType::Piers;
        s.beam_height = 20.0;
        s.pier_spacing = 72.0;
        let o = s.to_options();
        assert_eq!(o.kind, FoundationKind::Pier);
        assert_eq!((o.edge_height, o.pier_spacing), (20.0, 72.0));
        // The names Chief's dialog uses.
        let names: Vec<&str> = FoundationType::ALL.iter().map(|t| t.name()).collect();
        assert_eq!(
            names,
            [
                "Walls with Footings",
                "Monolithic Slab",
                "Grade Beams on Piers"
            ]
        );
    }

    #[test]
    fn the_foundation_spec_refuses_nonsense() {
        let mut s = spec(FoundationType::WallsWithFootings);
        assert!(s.error().is_none());
        s.stem_height = 0.0;
        assert!(s.error().is_some());
        s.stem_height = 36.0;
        s.footing_width = 0.0;
        assert!(s.error().unwrap().contains("footing"));
        s.footing = false;
        assert!(s.error().is_none());
        s.kind = FoundationType::MonolithicSlab;
        s.slab_thickness = 0.0;
        assert!(s.error().is_some());
        s.slab_thickness = 4.0;
        s.slab_stem_height = 0.0;
        assert!(s.error().is_some());
        s.kind = FoundationType::Piers;
        s.pier_spacing = 6.0;
        assert!(s.error().is_some());
        s.pier_spacing = 96.0;
        assert!(s.error().is_none());
    }

    #[test]
    fn build_foundation_makes_a_basement_and_its_room_type() {
        let mut cx = house();
        assert!(cx.defaults.room_type("Basement").is_none());
        let mut s = spec(FoundationType::WallsWithFootings);
        s.stem_height = 108.0;
        build_foundation(&mut cx, s);
        assert_eq!(cx.floor, 1, "the active floor moved up with the plan");
        let f = &cx.project.floors[0];
        assert_eq!(f.kind, FloorKind::Foundation);
        assert_eq!(f.room_names[0].room_type, "Basement");
        let t = cx.defaults.room_type("Basement").expect("type added");
        assert_eq!(t.function, "Basement");
        assert!(!t.include_in_living_area);
        assert!(cx.status.contains("basement"), "{}", cx.status);
        // One undo step.
        assert_eq!(cx.undo_label(), Some("Build Foundation"));
        cx.undo();
        assert_eq!(cx.project.floors.len(), 1);
        // A short wall makes a crawl space, which the template already has.
        let mut s = spec(FoundationType::WallsWithFootings);
        s.stem_height = 30.0;
        build_foundation(&mut cx, s);
        assert_eq!(cx.project.floors[0].room_names[0].room_type, "Crawl Space");
        assert!(!cx.project.floors[0].room_names[0].has_floor);
        // Choosing no room leaves the foundation floor unnamed.
        s.rooms = FoundationRooms::None;
        build_foundation(&mut cx, s);
        assert!(cx.project.floors[0].room_names.is_empty());
    }

    #[test]
    fn the_other_foundation_types_build_their_parts() {
        let mut cx = house();
        build_foundation(&mut cx, spec(FoundationType::MonolithicSlab));
        let layer = plan_core::foundation::FoundationLayer::load(&cx.project.floors[0]);
        assert_eq!(layer.slabs.len(), 1);
        assert!(cx.project.floors[0].room_names.is_empty());
        build_foundation(&mut cx, spec(FoundationType::Piers));
        let layer = plan_core::foundation::FoundationLayer::load(&cx.project.floors[0]);
        assert!(layer.slabs.is_empty() && layer.piers.len() >= 4);
        assert_eq!(cx.project.floors.len(), 2);
    }

    #[test]
    fn build_new_floor_can_build_the_attic_too() {
        let mut cx = house();
        let spec = NewFloorSpec {
            attic: true,
            ..NewFloorSpec::new()
        };
        let idx = build_new_floor_with(&mut cx, &spec).unwrap();
        assert_eq!(cx.project.floors.len(), 3);
        assert_eq!(cx.project.floors[2].kind, FloorKind::Attic);
        assert_eq!(cx.floor, idx, "the new floor is the active one");
        cx.undo();
        assert_eq!(cx.project.floors.len(), 1);
    }

    #[test]
    fn build_attic_floor_is_one_undo_step_and_needs_walls() {
        let mut cx = house();
        let a = build_attic_floor(&mut cx).unwrap();
        assert_eq!(cx.floor, a);
        assert_eq!(cx.project.floors[a].kind, FloorKind::Attic);
        assert_eq!(cx.project.floors[a].room_names[0].room_type, "Attic");
        assert_eq!(cx.undo_label(), Some("Build Attic Floor"));
        cx.undo();
        assert_eq!(cx.project.floors.len(), 1);
        let mut blank = EditorContext::new(plan_defaults::embedded());
        assert!(build_attic_floor(&mut blank).is_none());
        assert!(blank.status.contains("exterior walls"), "{}", blank.status);
    }

    #[test]
    fn the_living_area_readout_names_the_floors() {
        let mut cx = house();
        cx.project.build_new_floor(true);
        cx.refresh();
        let report = living_area_report(&cx);
        assert!(report.starts_with("Total living area "), "{report}");
        assert!(
            report.contains("1st Floor") && report.contains("2nd Floor"),
            "{report}"
        );
        let total = living_area_total_sq_ft(&cx);
        let per: f64 = living_area_by_floor(&cx).iter().map(|(_, a)| a).sum();
        assert!((total - per).abs() < 1e-9);
        assert_eq!(group_thousands(1820), "1,820");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1_234_567), "1,234,567");
        // A Room Specification OK leaves the readout in the status line.
        let room = cx.rooms[0].clone();
        let extras = extras_for(&cx, &room);
        let draft = RoomName::new(room_anchor(&room), "Den", "Den");
        assert!(apply_room_spec(&mut cx, 0, &draft, &extras));
        assert!(cx.status.contains("Total living area"), "{}", cx.status);
    }

    #[test]
    fn a_label_sits_where_its_placement_says_and_keeps_its_drag() {
        let mut cx = house();
        let room = cx.rooms[0].clone();
        let centre = label_position(&cx, &room);
        let extras = extras_for(&cx, &room);
        let mut draft = RoomName::new(room_anchor(&room), "Den", "Den");
        draft.label_style.placement = LabelPlacement::Bottom;
        assert!(apply_room_spec(&mut cx, 0, &draft, &extras));
        let room = cx.rooms[0].clone();
        let low = label_position(&cx, &room);
        assert!(low.y < centre.y - 10.0, "{low:?} vs {centre:?}");
        assert!(room.contains(low));
        // A drag adds to the placement.
        assert!(set_label_offset(&mut cx, 0, Point::new(10.0, 0.0)));
        assert!((label_position(&cx, &room).x - (low.x + 10.0)).abs() < 1e-9);
        // The text style comes from the Label tab, else the Room Label Style.
        assert_eq!(label_text_style(&cx, &room), "Room Label Style");
        let mut draft = cx.floor().room_names[0].clone();
        draft.label_style.text_style = "Schedule Style".into();
        let extras = extras_for(&cx, &room);
        assert!(apply_room_spec(&mut cx, 0, &draft, &extras));
        let room = cx.rooms[0].clone();
        assert_eq!(label_text_style(&cx, &room), "Schedule Style");
    }

    #[test]
    fn the_monolithic_slab_flag_and_the_chair_rail_are_stored_with_the_room() {
        let mut cx = house();
        let room = cx.rooms[0].clone();
        let mut extras = extras_for(&cx, &room);
        extras.base_molding = "Base - Colonial 5 1/4".into();
        extras.chair_molding = "Chair Rail - Simple 2 1/2".into();
        extras.crown_molding = "Crown - Cove 3 5/8".into();
        let mut draft = RoomName::new(room_anchor(&room), "Den", "Den");
        draft.monolithic_slab = Some(plan_core::rooms::RoomSlab {
            thickness: 5.0,
            stem_height: 15.0,
        });
        assert!(apply_room_spec(&mut cx, 0, &draft, &extras));
        let n = &cx.floor().room_names[0];
        assert_eq!(n.monolithic_slab.unwrap().stem_height, 15.0);
        // The library gives each molding its own height.
        let h = |k: MoldingKind| n.moldings.iter().find(|m| m.kind == k).map(|m| m.height);
        assert_eq!(h(MoldingKind::Base), Some(5.25));
        assert_eq!(h(MoldingKind::Chair), Some(2.5));
        assert_eq!(h(MoldingKind::Crown), Some(3.625));
        // The dialog reads them back.
        let again = extras_for(&cx, &room);
        assert_eq!(again.chair_molding, "Chair Rail - Simple 2 1/2");
        // Clearing a profile removes the molding; a name the library lacks
        // keeps the height it had.
        let mut extras = again;
        extras.chair_molding.clear();
        extras.base_molding = "My Own Base".into();
        let draft = cx.floor().room_names[0].clone();
        assert!(apply_room_spec(&mut cx, 0, &draft, &extras));
        let n = &cx.floor().room_names[0];
        assert_eq!(n.moldings.len(), 2);
        assert_eq!(
            n.moldings
                .iter()
                .find(|m| m.kind == MoldingKind::Base)
                .unwrap()
                .height,
            5.25
        );
    }

    #[test]
    fn footprint_adds_cad() {
        let mut cx = house();
        let area = add_plan_footprint(&mut cx).unwrap();
        assert!(area > 250.0, "{area}");
        assert_eq!(cx.floor().cad.len(), 2);
    }
}
