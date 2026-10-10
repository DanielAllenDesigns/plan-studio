//! Layer Painter, Layer Eyedropper, Object Painter, Object Eyedropper and
//! Match Properties (LAY-18, S-116; Tools menu).
//!
//! * **Layer Eyedropper** loads the layer of the clicked object; the tool then
//!   becomes the **Layer Painter**, and every click moves the clicked object to
//!   that layer (Scope: *Component* is the clicked object alone, *Object* also
//!   its group). One undo step per click.
//! * **Object Eyedropper** loads the attributes of the clicked object: every
//!   specification field except where it is and how big it is. The **Object
//!   Painter** writes them onto the objects it is clicked on, which must be of
//!   the same kind (a wall to walls, a door to doors). Scope (Object Painter
//!   Modes): *Component*, *Object* (the group), *Room*, *Floor* and *Plan*.
//!   With *Apply to all of type* off, the Room, Floor and Plan scopes only
//!   reach objects like the clicked one (same wall type, library symbol,
//!   cabinet kind, ...); on, they reach every object of the kind.
//! * **Match Properties** (Edit toolbar) loads the selected object into the
//!   Object Painter.
//!
//! Which fields are copied, per kind, is [`Attrs`]. The pure part (capture,
//! targets, apply) takes the editor context so it is tested headless.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::selection::{expand_groups, hit_test_cx, layer_of};
use crate::editor::Camera;
use crate::editor::{placed, rooms_edit, EditorContext, EditorRequest, ObjectRef};
use eframe::egui;
use plan_cabinets::Cabinet;
use plan_core::cad::{CadAttrs, CadItem};
use plan_core::fireplace::is_fireplace_symbol;
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::{Dimension, Floor, Id, Opening, OpeningKind, PlacedSymbol, RoomName, Wall};
use std::cell::RefCell;

/// Tools > Object Painter > Object Painter Modes...
pub const MODES: &str = "painters.object_modes";
/// Edit toolbar > Match Properties.
pub const MATCH_PROPERTIES: &str = "painters.match_properties";

/// The tools of the painter family (the Layer Hider rides along: it is the
/// third tool of the Layer Painter, Eyedropper and Hider group).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum PainterMode {
    /// Click objects to move them to the loaded layer.
    #[default]
    LayerPaint,
    /// Click an object to load its layer.
    LayerEyedropper,
    /// Click objects to paint the loaded attributes onto them.
    ObjectPaint,
    /// Click an object to load its attributes.
    ObjectEyedropper,
    /// Click an object to turn off its primary layer in the shown layer set
    /// (LAY-73).
    LayerHider,
}

impl PainterMode {
    pub const ALL: [PainterMode; 5] = [
        PainterMode::LayerPaint,
        PainterMode::LayerEyedropper,
        PainterMode::ObjectPaint,
        PainterMode::ObjectEyedropper,
        PainterMode::LayerHider,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PainterMode::LayerPaint => "Layer Painter",
            PainterMode::LayerEyedropper => "Layer Eyedropper",
            PainterMode::ObjectPaint => "Object Painter",
            PainterMode::ObjectEyedropper => "Object Eyedropper",
            PainterMode::LayerHider => "Layer Hider",
        }
    }

    /// The name of the toolbar button; the 3D toolbar already has an Object
    /// Eyedropper (the materials one).
    pub fn toolbar_name(self) -> &'static str {
        match self {
            PainterMode::ObjectEyedropper => "Object Painter Eyedropper",
            other => other.name(),
        }
    }

    pub fn is_layer(self) -> bool {
        matches!(self, PainterMode::LayerPaint | PainterMode::LayerEyedropper)
    }

    pub fn is_eyedropper(self) -> bool {
        matches!(
            self,
            PainterMode::LayerEyedropper | PainterMode::ObjectEyedropper
        )
    }
}

/// What a Layer Painter click reaches.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LayerScope {
    /// The clicked object alone, even inside a group.
    Component,
    /// The clicked object and the rest of its group.
    #[default]
    Object,
}

impl LayerScope {
    pub const ALL: [LayerScope; 2] = [LayerScope::Component, LayerScope::Object];

    pub fn name(self) -> &'static str {
        match self {
            LayerScope::Component => "Component",
            LayerScope::Object => "Object",
        }
    }
}

/// What an Object Painter click reaches (Object Painter Modes).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ObjectScope {
    /// The clicked object alone, even inside a group.
    Component,
    /// The clicked object and the group members of its kind.
    #[default]
    Object,
    /// The objects of the kind in the clicked room.
    Room,
    /// The objects of the kind on this floor.
    Floor,
    /// The objects of the kind on every floor.
    Plan,
}

impl ObjectScope {
    pub const ALL: [ObjectScope; 5] = [
        ObjectScope::Component,
        ObjectScope::Object,
        ObjectScope::Room,
        ObjectScope::Floor,
        ObjectScope::Plan,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ObjectScope::Component => "Component",
            ObjectScope::Object => "Object",
            ObjectScope::Room => "Room",
            ObjectScope::Floor => "Floor",
            ObjectScope::Plan => "Plan",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            ObjectScope::Component => "Only the object clicked, even when it is in a group.",
            ObjectScope::Object => "The object clicked and the objects of its group.",
            ObjectScope::Room => "Every object of that kind in the room clicked in.",
            ObjectScope::Floor => "Every object of that kind on this floor.",
            ObjectScope::Plan => "Every object of that kind on every floor.",
        }
    }
}

// ----- state shared by the tool, the bar and the Modes dialog -----

#[derive(Default)]
pub struct PainterState {
    /// The layer the Layer Painter paints.
    pub layer: Option<String>,
    pub layer_scope: LayerScope,
    /// "Use Default Layer": the Layer Painter puts each object on its own
    /// system default layer instead of the loaded one.
    pub use_default: bool,
    /// The attributes the Object Painter paints.
    pub source: Option<Attrs>,
    pub scope: ObjectScope,
    pub all_of_type: bool,
    /// The Object Painter Modes window is open.
    pub modes_open: bool,
}

thread_local! {
    static STATE: RefCell<PainterState> = RefCell::new(PainterState::default());
}

/// Reads or changes the painter settings.
pub fn with_state<R>(f: impl FnOnce(&mut PainterState) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

// ----- attributes -----

/// The attributes the Object Eyedropper loaded: everything that specifies an
/// object except its position and size.
#[derive(Clone, Debug)]
pub struct Attrs {
    /// "Wall (Exterior 2x6)" for the bar and the status line.
    pub summary: String,
    data: Data,
}

#[derive(Clone, Debug)]
enum Data {
    Wall(Box<Wall>),
    Opening(Box<Opening>),
    Dimension(Box<Dimension>),
    Cad {
        layer: String,
        attrs: Box<CadAttrs>,
        text_height: Option<f64>,
    },
    Symbol(Box<PlacedSymbol>),
    Cabinet(Box<Cabinet>),
    Room(Box<RoomName>),
}

fn cad_is_text(floor: &Floor, id: Id) -> bool {
    floor
        .cad
        .iter()
        .any(|c| c.id == id && matches!(c.item, CadItem::Text { .. }))
}

/// Does the Object Painter handle this kind of object?
pub fn paintable(o: ObjectRef) -> bool {
    matches!(
        o,
        ObjectRef::Wall(_)
            | ObjectRef::Opening(_)
            | ObjectRef::Dimension(_)
            | ObjectRef::Cad(_)
            | ObjectRef::Text(_)
            | ObjectRef::Symbol(_)
            | ObjectRef::Cabinet(_)
            | ObjectRef::Room(_)
    )
}

impl Attrs {
    /// Loads the attributes of `o` on floor `fl`.
    pub fn capture(cx: &EditorContext, fl: usize, o: ObjectRef) -> Option<Attrs> {
        let floor = cx.project.floors.get(fl)?;
        let (summary, data) = match o {
            ObjectRef::Wall(id) => {
                let w = floor.wall(id)?;
                let what = w
                    .wall_type
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", w.kind));
                (format!("Wall ({what})"), Data::Wall(Box::new(w.clone())))
            }
            ObjectRef::Opening(id) => {
                let op = floor.openings.iter().find(|x| x.id == id)?;
                let what = match op.kind {
                    OpeningKind::Door => "Door",
                    OpeningKind::Window => "Window",
                };
                (what.to_string(), Data::Opening(Box::new(op.clone())))
            }
            ObjectRef::Dimension(id) => {
                let d = floor.dimensions.iter().find(|x| x.id == id)?;
                (
                    "Dimension".to_string(),
                    Data::Dimension(Box::new(d.clone())),
                )
            }
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                let c = floor.cad.iter().find(|x| x.id == id)?;
                let (what, text_height) = match &c.item {
                    CadItem::Text { height, .. } => ("Text", Some(*height)),
                    CadItem::Line { .. } => ("CAD line", None),
                    CadItem::Arc { .. } => ("CAD arc", None),
                    CadItem::Circle { .. } => ("CAD circle", None),
                    CadItem::Polyline { .. } => ("CAD polyline", None),
                };
                let mut attrs = floor.cad_attrs(id).unwrap_or_else(|| CadAttrs::new(id));
                // The words and the shape belong to the object.
                attrs.runs.clear();
                attrs.arc_edges.clear();
                if let Some(f) = attrs.fill.as_mut() {
                    f.lines.clear();
                }
                (
                    what.to_string(),
                    Data::Cad {
                        layer: c.layer.clone(),
                        attrs: Box::new(attrs),
                        text_height,
                    },
                )
            }
            ObjectRef::Symbol(id) => {
                let s = floor.symbol(id)?;
                if !symbol_paintable(s) {
                    return None;
                }
                (
                    format!("Symbol ({})", s.catalog_id),
                    Data::Symbol(Box::new(s.clone())),
                )
            }
            ObjectRef::Cabinet(id) => {
                let c = placed::cabinet_by_id(floor, id)?;
                (
                    format!("Cabinet ({:?})", c.kind),
                    Data::Cabinet(Box::new(c)),
                )
            }
            ObjectRef::Room(i) => {
                if fl != cx.floor {
                    return None;
                }
                let room = cx.rooms.get(i)?;
                let entry = rooms_edit::name_entry(cx, room)
                    .cloned()
                    .unwrap_or_else(|| {
                        RoomName::new(rooms_edit::room_anchor(room), room.label.clone(), "")
                    });
                (
                    format!("Room ({})", entry.room_type),
                    Data::Room(Box::new(entry)),
                )
            }
            _ => return None,
        };
        Some(Attrs { summary, data })
    }

    /// Can these attributes be painted onto `target` of floor `fl`? Doors and
    /// windows do not exchange attributes.
    fn accepts(&self, floor: &Floor, target: ObjectRef) -> bool {
        match (&self.data, target) {
            (Data::Wall(_), ObjectRef::Wall(id)) => floor.wall(id).is_some(),
            (Data::Opening(src), ObjectRef::Opening(id)) => floor
                .openings
                .iter()
                .any(|o| o.id == id && o.kind == src.kind),
            (Data::Dimension(_), ObjectRef::Dimension(id)) => {
                floor.dimensions.iter().any(|d| d.id == id)
            }
            (Data::Cad { .. }, ObjectRef::Cad(id) | ObjectRef::Text(id)) => {
                floor.cad.iter().any(|c| c.id == id)
            }
            (Data::Symbol(_), ObjectRef::Symbol(id)) => {
                floor.symbol(id).is_some_and(symbol_paintable)
            }
            (Data::Cabinet(_), ObjectRef::Cabinet(id)) => {
                placed::cabinet_by_id(floor, id).is_some()
            }
            (Data::Room(_), ObjectRef::Room(_)) => true,
            _ => false,
        }
    }

    /// Does `o` belong to the same kind as these attributes?
    fn same_kind(&self, o: ObjectRef) -> bool {
        matches!(
            (&self.data, o),
            (Data::Wall(_), ObjectRef::Wall(_))
                | (Data::Opening(_), ObjectRef::Opening(_))
                | (Data::Dimension(_), ObjectRef::Dimension(_))
                | (Data::Cad { .. }, ObjectRef::Cad(_) | ObjectRef::Text(_))
                | (Data::Symbol(_), ObjectRef::Symbol(_))
                | (Data::Cabinet(_), ObjectRef::Cabinet(_))
                | (Data::Room(_), ObjectRef::Room(_))
        )
    }

    /// Writes the attributes onto `target`; true when something changed.
    fn apply(&self, cx: &mut EditorContext, fl: usize, target: ObjectRef) -> bool {
        match (&self.data, target) {
            (Data::Wall(src), ObjectRef::Wall(id)) => {
                edit(cx.project.floors[fl].wall_mut(id), |w| {
                    w.thickness = src.thickness;
                    w.height = src.height;
                    w.kind = src.kind;
                    w.layer = src.layer.clone();
                    w.flags = src.flags.clone();
                    w.wall_type = src.wall_type.clone();
                    w.roof = src.roof.clone();
                    w.extras = src.extras.clone();
                    w.class = src.class.clone();
                    w.foundation_height = src.foundation_height;
                    w.is_deck_edge = src.is_deck_edge;
                    w.spec = src.spec.clone();
                })
            }
            (Data::Opening(src), ObjectRef::Opening(id)) => edit(
                cx.project.floors[fl]
                    .openings
                    .iter_mut()
                    .find(|o| o.id == id),
                |o| {
                    o.style = src.style;
                    o.casing = src.casing;
                    o.lites = src.lites;
                    o.egress = src.egress;
                    o.tempered = src.tempered;
                    o.extras = src.extras.clone();
                },
            ),
            (Data::Dimension(src), ObjectRef::Dimension(id)) => edit(
                cx.project.floors[fl]
                    .dimensions
                    .iter_mut()
                    .find(|d| d.id == id),
                |d| {
                    d.text_style = src.text_style.clone();
                    d.look = src.look.clone();
                },
            ),
            (
                Data::Cad {
                    layer,
                    attrs,
                    text_height,
                },
                ObjectRef::Cad(id) | ObjectRef::Text(id),
            ) => self.apply_cad(cx, fl, id, layer, attrs, *text_height),
            (Data::Symbol(src), ObjectRef::Symbol(id)) => edit(
                cx.project.floors[fl]
                    .symbols
                    .iter_mut()
                    .find(|s| s.id == id && symbol_paintable(s)),
                |s| {
                    s.catalog_id = src.catalog_id.clone();
                    s.elevation = src.elevation;
                    s.flip = src.flip;
                    s.solid = src.solid;
                    s.layer = src.layer.clone();
                },
            ),
            (Data::Cabinet(src), ObjectRef::Cabinet(id)) => {
                let Some(mut cab) = placed::cabinet_by_id(&cx.project.floors[fl], id) else {
                    return false;
                };
                let before = serde_json::to_value(&cab).ok();
                cab.door_style = src.door_style.clone();
                cab.drawer_style = src.drawer_style.clone();
                cab.overlay = src.overlay;
                cab.framed = src.framed;
                cab.materials = src.materials;
                cab.indicators = src.indicators;
                cab.indicators_3d = src.indicators_3d;
                cab.moldings = src.moldings.clone();
                cab.fill = src.fill;
                cab.accessories = src.accessories;
                if cab.kind == src.kind {
                    cab.countertop = src.countertop;
                    cab.backsplash = src.backsplash;
                    cab.toe_kick = src.toe_kick;
                }
                let changed = serde_json::to_value(&cab).ok() != before;
                changed && placed::replace_cabinet(&mut cx.project, fl, &cab)
            }
            (Data::Room(src), ObjectRef::Room(i)) => self.apply_room(cx, src, i),
            _ => false,
        }
    }

    fn apply_cad(
        &self,
        cx: &mut EditorContext,
        fl: usize,
        id: Id,
        layer: &str,
        src: &CadAttrs,
        text_height: Option<f64>,
    ) -> bool {
        let before = (
            cx.project.floors[fl].cad_attrs(id),
            cx.project.floors[fl]
                .cad
                .iter()
                .find(|c| c.id == id)
                .map(|c| (c.layer.clone(), c.item.clone())),
        );
        let is_text = cad_is_text(&cx.project.floors[fl], id);
        if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
            c.layer = layer.to_string();
            if let (CadItem::Text { height, .. }, Some(h)) = (&mut c.item, text_height) {
                *height = h;
            }
        }
        cx.project.edit_cad_attrs(fl, id, |a| {
            a.color = src.color;
            a.weight = src.weight;
            a.dash = src.dash;
            a.fill = src.fill.clone();
            a.arrow_start = src.arrow_start;
            a.arrow_end = src.arrow_end;
            a.arrow_size = src.arrow_size;
            a.text_style = src.text_style.clone();
            if is_text {
                // The wrap width and the minimum height are size.
                let (w, h) = (a.text_box.width, a.text_box.height);
                a.text_box = src.text_box;
                a.text_box.width = w;
                a.text_box.height = h;
            }
        });
        let after = (
            cx.project.floors[fl].cad_attrs(id),
            cx.project.floors[fl]
                .cad
                .iter()
                .find(|c| c.id == id)
                .map(|c| (c.layer.clone(), c.item.clone())),
        );
        before != after
    }

    fn apply_room(&self, cx: &mut EditorContext, src: &RoomName, index: usize) -> bool {
        let Some(room) = cx.rooms.get(index).cloned() else {
            return false;
        };
        let fl = cx.floor;
        let current = rooms_edit::name_entry(cx, &room).cloned();
        let before = current.as_ref().and_then(|c| serde_json::to_value(c).ok());
        let anchor = current
            .as_ref()
            .map_or_else(|| rooms_edit::room_anchor(&room), |c| c.anchor);
        let name = current
            .as_ref()
            .map_or_else(|| room.label.clone(), |c| c.name.clone());
        let rooms = cx.rooms.clone();
        cx.project
            .set_room_name(fl, anchor, name, src.room_type.clone(), &rooms);
        let Some(n) = cx.project.floors[fl]
            .room_names
            .iter_mut()
            .find(|n| n.anchor == anchor)
        else {
            return false;
        };
        n.floor_finish = src.floor_finish.clone();
        n.ceiling_finish = src.ceiling_finish.clone();
        n.include_in_living_area = src.include_in_living_area;
        n.has_ceiling = src.has_ceiling;
        n.has_floor = src.has_floor;
        n.conditioned = src.conditioned;
        n.fill_style = src.fill_style.clone();
        n.label = src.label.clone();
        n.moldings = src.moldings.clone();
        n.misc = src.misc.clone();
        n.label_style = src.label_style.clone();
        serde_json::to_value(&*n).ok() != before
    }
}

fn symbol_paintable(s: &PlacedSymbol) -> bool {
    s.image.is_none() && s.distribution.is_none() && s.owner.is_none() && !is_fireplace_symbol(s)
}

/// Runs `f` on `target` and reports whether its stored form changed.
fn edit<T: serde::Serialize>(target: Option<&mut T>, f: impl FnOnce(&mut T)) -> bool {
    let Some(t) = target else {
        return false;
    };
    let before = serde_json::to_value(&*t).ok();
    f(t);
    serde_json::to_value(&*t).ok() != before
}

// ----- targets -----

/// What "like the clicked object" means for the Room, Floor and Plan scopes
/// when Apply to all of type is off.
fn match_key(cx: &EditorContext, fl: usize, o: ObjectRef) -> String {
    let Some(floor) = cx.project.floors.get(fl) else {
        return String::new();
    };
    match o {
        ObjectRef::Wall(id) => floor
            .wall(id)
            .map(|w| format!("{:?}/{}", w.kind, w.wall_type.clone().unwrap_or_default()))
            .unwrap_or_default(),
        ObjectRef::Opening(id) => floor
            .openings
            .iter()
            .find(|x| x.id == id)
            .map(|x| format!("{:?}", x.kind))
            .unwrap_or_default(),
        ObjectRef::Dimension(id) => floor
            .dimensions
            .iter()
            .find(|x| x.id == id)
            .map(|x| format!("{:?}", x.kind))
            .unwrap_or_default(),
        ObjectRef::Cad(id) | ObjectRef::Text(id) => floor
            .cad
            .iter()
            .find(|x| x.id == id)
            .map(|c| {
                match c.item {
                    CadItem::Line { .. } => "line",
                    CadItem::Arc { .. } => "arc",
                    CadItem::Circle { .. } => "circle",
                    CadItem::Polyline { .. } => "polyline",
                    CadItem::Text { .. } => "text",
                }
                .to_string()
            })
            .unwrap_or_default(),
        ObjectRef::Symbol(id) => floor
            .symbol(id)
            .map(|s| s.catalog_id.clone())
            .unwrap_or_default(),
        ObjectRef::Cabinet(id) => placed::cabinet_by_id(floor, id)
            .map(|c| format!("{:?}", c.kind))
            .unwrap_or_default(),
        ObjectRef::Room(i) if fl == cx.floor => cx
            .rooms
            .get(i)
            .and_then(|r| rooms_edit::name_entry(cx, r))
            .map(|n| n.room_type.clone())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Every object of the kind `like` on floor `fl`, with a point that tells
/// which room it is in.
fn objects_of_kind(cx: &EditorContext, fl: usize, like: ObjectRef) -> Vec<(ObjectRef, Point)> {
    let Some(floor) = cx.project.floors.get(fl) else {
        return Vec::new();
    };
    match like {
        ObjectRef::Wall(_) => floor
            .walls
            .iter()
            .map(|w| (ObjectRef::Wall(w.id), mid(w.start, w.end)))
            .collect(),
        ObjectRef::Opening(_) => floor
            .openings
            .iter()
            .filter_map(|o| {
                let w = floor.wall(o.wall_id)?;
                Some((ObjectRef::Opening(o.id), w.point_at(o.center_offset)))
            })
            .collect(),
        ObjectRef::Dimension(_) => floor
            .dimensions
            .iter()
            .map(|d| (ObjectRef::Dimension(d.id), mid(d.start, d.end)))
            .collect(),
        ObjectRef::Cad(_) | ObjectRef::Text(_) => floor
            .cad
            .iter()
            .map(|c| {
                let (lo, hi) = c.item.bounds();
                let r = if matches!(c.item, CadItem::Text { .. }) {
                    ObjectRef::Text(c.id)
                } else {
                    ObjectRef::Cad(c.id)
                };
                (r, mid(lo, hi))
            })
            .collect(),
        ObjectRef::Symbol(_) => floor
            .symbols
            .iter()
            .map(|s| (ObjectRef::Symbol(s.id), s.position))
            .collect(),
        ObjectRef::Cabinet(_) => placed::load_cabinets(floor)
            .iter()
            .map(|c| (ObjectRef::Cabinet(c.id), c.position))
            .collect(),
        ObjectRef::Room(_) if fl == cx.floor => cx
            .rooms
            .iter()
            .enumerate()
            .map(|(i, r)| (ObjectRef::Room(i), r.centroid))
            .collect(),
        _ => Vec::new(),
    }
}

fn mid(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}

/// How near a wall's middle must be to a room's outline for the wall to count
/// as the room's (inches).
const WALL_ROOM_TOL: f64 = 14.0;

fn near_outline(poly: &[Point], p: Point, tol: f64) -> bool {
    let n = poly.len();
    n >= 2 && (0..n).any(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]) <= tol)
}

/// Is `at` in room `idx` (inside it, or on its walls)?
fn in_room(cx: &EditorContext, idx: usize, at: Point) -> bool {
    cx.rooms
        .get(idx)
        .is_some_and(|r| r.contains(at) || near_outline(&r.polygon, at, WALL_ROOM_TOL))
}

/// The room a click on `clicked` at `click` means: the one under the click,
/// else one whose outline the object lies on.
fn room_of(cx: &EditorContext, clicked: ObjectRef, click: Point) -> Option<usize> {
    if let ObjectRef::Room(i) = clicked {
        return Some(i);
    }
    if let Some(i) = rooms_edit::room_index_at(cx, click) {
        return Some(i);
    }
    let at = objects_of_kind(cx, cx.floor, clicked)
        .into_iter()
        .find(|(o, _)| *o == clicked)
        .map(|(_, p)| p)
        .unwrap_or(click);
    (0..cx.rooms.len()).find(|i| in_room(cx, *i, at))
}

/// The objects an Object Painter click at `click` on `clicked` reaches, as
/// (floor, object).
pub fn targets(
    cx: &EditorContext,
    clicked: ObjectRef,
    click: Point,
    src: &Attrs,
    scope: ObjectScope,
    all_of_type: bool,
) -> Vec<(usize, ObjectRef)> {
    let fl = cx.floor;
    let ok = |floor: usize, o: ObjectRef| {
        cx.project
            .floors
            .get(floor)
            .is_some_and(|f| src.accepts(f, o))
    };
    match scope {
        ObjectScope::Component => ok(fl, clicked)
            .then_some((fl, clicked))
            .into_iter()
            .collect(),
        ObjectScope::Object => expand_groups(cx, &[clicked])
            .into_iter()
            .filter(|o| src.same_kind(*o) && ok(fl, *o))
            .map(|o| (fl, o))
            .collect(),
        ObjectScope::Room | ObjectScope::Floor | ObjectScope::Plan => {
            let key = match_key(cx, fl, clicked);
            let like = |floor: usize, o: ObjectRef| all_of_type || match_key(cx, floor, o) == key;
            let room = (scope == ObjectScope::Room)
                .then(|| room_of(cx, clicked, click))
                .flatten();
            if scope == ObjectScope::Room && room.is_none() {
                return Vec::new();
            }
            let floors: Vec<usize> = if scope == ObjectScope::Plan {
                (0..cx.project.floors.len()).collect()
            } else {
                vec![fl]
            };
            let mut out = Vec::new();
            for f in floors {
                for (o, at) in objects_of_kind(cx, f, clicked) {
                    let in_scope = match room {
                        Some(r) => match o {
                            ObjectRef::Room(i) => i == r,
                            _ => in_room(cx, r, at),
                        },
                        None => true,
                    };
                    if in_scope && ok(f, o) && like(f, o) {
                        out.push((f, o));
                    }
                }
            }
            // The clicked object is always painted.
            if !out.contains(&(fl, clicked)) && ok(fl, clicked) {
                out.insert(0, (fl, clicked));
            }
            out
        }
    }
}

fn is_locked(cx: &EditorContext, fl: usize, o: ObjectRef) -> bool {
    cx.project
        .floors
        .get(fl)
        .and_then(|f| layer_of(f, o))
        .is_some_and(|l| cx.project.layers.is_locked(&l))
}

// ----- the clicks -----

/// The object under `p` (the topmost one the painters can read).
pub fn pick(cx: &EditorContext, p: Point) -> Option<ObjectRef> {
    hit_test_cx(cx, p, cx.pick_tol()).first().copied()
}

/// Layer Eyedropper: loads the layer of `o`. Returns the layer.
pub fn eyedrop_layer(cx: &mut EditorContext, o: ObjectRef) -> Option<String> {
    let Some(layer) = layer_of(cx.floor(), o) else {
        cx.status = format!("A {} is not on a layer", o.type_name());
        return None;
    };
    with_state(|s| s.layer = Some(layer.clone()));
    cx.status = format!("Layer Eyedropper: {layer}. Click objects to move them to it");
    Some(layer)
}

/// Layer Painter: moves `clicked` (and its group, by scope) to the loaded
/// layer as one undo step. Returns how many objects moved.
pub fn paint_layer(cx: &mut EditorContext, clicked: ObjectRef) -> usize {
    if with_state(|s| s.use_default) {
        let scope = with_state(|s| s.layer_scope);
        let items = match scope {
            LayerScope::Component => vec![clicked],
            LayerScope::Object => expand_groups(cx, &[clicked]),
        };
        return crate::dialogs::select_layer::send_items_to_default(cx, &items, "Layer Painter");
    }
    let Some(layer) = with_state(|s| s.layer.clone()) else {
        cx.status = "Layer Painter: pick a layer first (Layer Eyedropper or the layer list)".into();
        return 0;
    };
    let scope = with_state(|s| s.layer_scope);
    let items = match scope {
        LayerScope::Component => vec![clicked],
        LayerScope::Object => expand_groups(cx, &[clicked]),
    };
    cx.move_objects_to_layer(&items, &layer, "Layer Painter")
}

/// Object Eyedropper: loads the attributes of `o` on the active floor.
pub fn eyedrop_object(cx: &mut EditorContext, o: ObjectRef) -> bool {
    match Attrs::capture(cx, cx.floor, o) {
        Some(a) => {
            cx.status = format!(
                "Object Eyedropper: {}. Click objects of the same kind to paint them",
                a.summary
            );
            with_state(|s| s.source = Some(a));
            true
        }
        None => {
            cx.status = format!("The Object Painter does not copy a {}", o.type_name());
            false
        }
    }
}

/// Object Painter: paints the loaded attributes onto the objects `clicked`
/// reaches (one undo step). Returns how many objects changed.
pub fn paint_object(cx: &mut EditorContext, clicked: ObjectRef, click: Point) -> usize {
    let Some((src, scope, all)) =
        with_state(|s| s.source.clone().map(|a| (a, s.scope, s.all_of_type)))
    else {
        cx.status = "Object Painter: use the Object Eyedropper on an object first".into();
        return 0;
    };
    if !src.same_kind(clicked) {
        cx.status = format!(
            "The Object Painter holds a {}; it cannot paint a {}",
            src.summary,
            clicked.type_name().to_lowercase()
        );
        return 0;
    }
    let found = targets(cx, clicked, click, &src, scope, all);
    if found.is_empty() {
        cx.status = format!(
            "Object Painter: the loaded {} does not fit that {}",
            src.summary,
            clicked.type_name().to_lowercase()
        );
        return 0;
    }
    let todo: Vec<(usize, ObjectRef)> = found
        .into_iter()
        .filter(|(f, o)| !is_locked(cx, *f, *o))
        .collect();
    if todo.is_empty() {
        cx.status = "Object Painter: those objects are on a locked layer".into();
        return 0;
    }
    cx.begin_change("Object Painter");
    let mut n = 0;
    for (f, o) in todo {
        if src.apply(cx, f, o) {
            n += 1;
        }
    }
    if n == 0 {
        cx.cancel_change();
        cx.status = "Object Painter: the objects already match".into();
        return 0;
    }
    cx.mark_dirty();
    cx.status = format!(
        "Object Painter: painted {n} {} object{}",
        clicked.type_name().to_lowercase(),
        if n == 1 { "" } else { "s" }
    );
    n
}

/// Match Properties: loads the selected object and switches to the Object
/// Painter. False when the selection is not one object the painter reads.
pub fn match_properties(cx: &mut EditorContext) -> bool {
    let Some(o) = cx.selection.single() else {
        cx.status = "Match Properties: select one object to copy from".into();
        return false;
    };
    if !eyedrop_object(cx, o) {
        return false;
    }
    cx.status = format!(
        "Match Properties: {}. Click objects to match them",
        with_state(|s| s.source.as_ref().map(|a| a.summary.clone())).unwrap_or_default()
    );
    cx.requests
        .push(EditorRequest::SetTool(ToolId::PainterVariant(
            PainterMode::ObjectPaint,
        )));
    true
}

/// Can the Edit toolbar offer Match Properties for the selection?
pub fn can_match(cx: &EditorContext) -> bool {
    cx.selection
        .single()
        .is_some_and(|o| paintable(o) && Attrs::capture(cx, cx.floor, o).is_some())
}

/// Runs a painter menu or toolbar command by id; false when it is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        MODES => {
            with_state(|s| s.modes_open = true);
            cx.requests
                .push(EditorRequest::SetTool(ToolId::PainterVariant(
                    PainterMode::ObjectPaint,
                )));
            true
        }
        MATCH_PROPERTIES => {
            match_properties(cx);
            true
        }
        // Line styles, Fill Styles (the Fill Style Painter reads and writes
        // through `dialogs::fill_style::{read_fill, apply_fill}`) and the
        // custom pattern commands.
        _ => {
            crate::dialogs::line_style::run_command(cx, id)
                || crate::dialogs::fill_style::run_command(cx, id)
                || crate::dialogs::pattern_editor::run_command(cx, id)
        }
    }
}

// ----- the tool -----

pub struct PaintersTool {
    mode: PainterMode,
}

impl Default for PaintersTool {
    fn default() -> Self {
        Self {
            mode: PainterMode::LayerPaint,
        }
    }
}

impl Tool for PaintersTool {
    fn id(&self) -> ToolId {
        ToolId::PainterVariant(self.mode)
    }

    fn name(&self) -> &'static str {
        self.mode.name()
    }

    fn hint(&self) -> String {
        match self.mode {
            PainterMode::LayerPaint => {
                "Click objects to move them to the loaded layer. Esc ends the painter".into()
            }
            PainterMode::LayerEyedropper => {
                "Click an object to load its layer, then click others to move them there".into()
            }
            PainterMode::ObjectPaint => {
                "Click objects of the same kind to paint the loaded attributes onto them. Esc ends the painter"
                    .into()
            }
            PainterMode::ObjectEyedropper => {
                "Click an object to load its attributes, then click others to paint them".into()
            }
            PainterMode::LayerHider => {
                "Click an object to turn off its primary layer in the active layer set. Esc ends the tool"
                    .into()
            }
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::PainterVariant(m) = id {
            self.mode = m;
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status.clear();
        match self.mode {
            PainterMode::LayerPaint => {
                // Start from the layer of the selection.
                if with_state(|s| s.layer.is_none() && !s.use_default) {
                    if let Some(l) = cx.selection_layers().into_iter().next() {
                        with_state(|s| s.layer = Some(l));
                    }
                }
                if with_state(|s| s.layer.is_none() && !s.use_default) {
                    cx.status =
                        "Layer Painter: pick a layer in the bar or use the Layer Eyedropper".into();
                }
            }
            PainterMode::ObjectPaint if with_state(|s| s.source.is_none()) => {
                cx.status =
                    "Object Painter: use the Object Eyedropper (or Match Properties) first".into();
            }
            _ => {}
        }
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        cx.hover = None;
    }

    fn frame(&mut self, cx: &mut EditorContext, _ctx: &egui::Context) {
        // The bar (drawn with the overlay) asked for another painter.
        if let Some(next) = crate::dialogs::painters::take_switch() {
            cx.requests.push(EditorRequest::SetTool(next));
        }
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, _cam: &Camera) {
        crate::dialogs::painters::show(painter, cx, self.mode);
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        cx.hover = pick(cx, p.world);
        // The Status Bar says which layer a click would turn off.
        if self.mode == PainterMode::LayerHider {
            if let Some(layer) = cx.hover.and_then(|o| layer_of(cx.floor(), o)) {
                cx.status = format!("Layer Hider: click to turn off {layer}");
            }
        }
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if p.button != egui::PointerButton::Primary {
            return ToolResult::ignored();
        }
        let Some(clicked) = pick(cx, p.world) else {
            cx.status = format!("{}: nothing under the pointer", self.mode.name());
            return ToolResult::consumed();
        };
        match self.mode {
            PainterMode::LayerEyedropper => {
                if eyedrop_layer(cx, clicked).is_some() {
                    return ToolResult {
                        consumed: true,
                        repaint: true,
                        switch_to: Some(ToolId::PainterVariant(PainterMode::LayerPaint)),
                        commit: None,
                    };
                }
                ToolResult::consumed()
            }
            PainterMode::LayerPaint => {
                if paint_layer(cx, clicked) > 0 {
                    ToolResult::committed("Layer Painter")
                } else {
                    ToolResult::consumed()
                }
            }
            PainterMode::ObjectEyedropper => {
                if eyedrop_object(cx, clicked) {
                    return ToolResult {
                        consumed: true,
                        repaint: true,
                        switch_to: Some(ToolId::PainterVariant(PainterMode::ObjectPaint)),
                        commit: None,
                    };
                }
                ToolResult::consumed()
            }
            PainterMode::ObjectPaint => {
                if paint_object(cx, clicked, p.world) > 0 {
                    ToolResult::committed("Object Painter")
                } else {
                    ToolResult::consumed()
                }
            }
            PainterMode::LayerHider => {
                if crate::dialogs::object_layers::hide_primary_layer(cx, clicked).is_some() {
                    ToolResult::committed("Layer Hider")
                } else {
                    ToolResult::consumed()
                }
            }
        }
    }

    fn key(&mut self, _cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(egui::Key::Escape) {
            return ToolResult {
                consumed: true,
                repaint: true,
                switch_to: Some(ToolId::Select),
                commit: None,
            };
        }
        ToolResult::ignored()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_have_chiefs_names_and_a_family_split() {
        let names: Vec<&str> = PainterMode::ALL.iter().map(|m| m.name()).collect();
        assert_eq!(
            names,
            [
                "Layer Painter",
                "Layer Eyedropper",
                "Object Painter",
                "Object Eyedropper",
                "Layer Hider"
            ]
        );
        assert!(PainterMode::LayerPaint.is_layer() && !PainterMode::ObjectPaint.is_layer());
        assert!(PainterMode::ObjectEyedropper.is_eyedropper());
        assert_eq!(
            ObjectScope::ALL.map(ObjectScope::name),
            ["Component", "Object", "Room", "Floor", "Plan"]
        );
    }

    #[test]
    fn the_tool_follows_its_variant() {
        let mut t = PaintersTool::default();
        t.set_variant(ToolId::PainterVariant(PainterMode::ObjectEyedropper));
        assert_eq!(
            t.id(),
            ToolId::PainterVariant(PainterMode::ObjectEyedropper)
        );
        assert_eq!(t.name(), "Object Eyedropper");
    }
}
