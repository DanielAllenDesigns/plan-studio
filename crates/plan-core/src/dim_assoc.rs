//! Associative dimensions: a dimension whose end sits on a wall, an opening,
//! a cabinet or a fixture keeps that end tied to the object, so it follows
//! when the object is moved, stretched or reshaped (DIM-3, DIM-29).
//!
//! An end is tied with a [`DimAnchor`]: the object, where on it (a wall's
//! start, end or a fraction of its length; an opening's edges or center; a
//! cabinet's or fixture's local x and y) and how far to the side of a wall's
//! centerline. The point is recomputed from the object by
//! [`Floor::sync_dimension_anchors`], which the editor runs whenever the plan
//! changes. Each anchor remembers the point it last resolved to: if the
//! dimension's end has been moved by hand since (it no longer matches), the
//! tie is dropped instead of dragging the end back, and an automatic
//! dimension edited this way becomes manual (DIM-33).

use crate::cad::{CadItem, CadObject};
use crate::dimension::{AutoGroup, Dimension, DimensionKind};
use crate::geometry::Point;
use crate::model::{Floor, Opening, Wall};
use crate::symbols::PlacedSymbol;
use crate::Id;
use serde::{Deserialize, Serialize};

/// Where on its object an anchor sits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DimAttach {
    /// A wall's start; an opening's start edge.
    Start,
    /// A wall's end; an opening's end edge.
    End,
    /// This fraction (0..1) of the way from the start to the end of a wall,
    /// or across an opening (0.5 is its center).
    Along(f64),
    /// A cabinet or fixture: `u` along its width and `v` from its back,
    /// inches in its own frame. For a CAD object, a stair or an electrical
    /// device, the offset from the object's reference point (a line's start,
    /// a circle's center, a stair's origin, a device's position).
    Local { u: f64, v: f64 },
    /// A polyline's vertex (CAD objects).
    Vertex(u32),
}

/// The kind of object an anchor is tied to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AnchorTarget {
    #[default]
    Wall,
    Opening,
    Cabinet,
    /// A placed library symbol (a fixture).
    Symbol,
    /// A CAD object: a line's ends, a polyline's vertices, a circle's or
    /// arc's center.
    Cad,
    /// A stair (its origin and direction carry the point).
    Stair,
    /// An electrical device.
    Device,
}

impl AnchorTarget {
    pub fn label(self) -> &'static str {
        match self {
            AnchorTarget::Wall => "Wall",
            AnchorTarget::Opening => "Opening",
            AnchorTarget::Cabinet => "Cabinet",
            AnchorTarget::Symbol => "Fixture",
            AnchorTarget::Cad => "CAD Object",
            AnchorTarget::Stair => "Stair",
            AnchorTarget::Device => "Device",
        }
    }
}

/// One tied end of a dimension.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimAnchor {
    /// The id of the object (named `wall` for the files that only knew
    /// walls); [`DimAnchor::target`] says what kind it is.
    pub wall: Id,
    #[serde(default)]
    pub target: AnchorTarget,
    pub at: DimAttach,
    /// Walls and openings: distance to the left of the wall's centerline
    /// (negative: right).
    pub side: f64,
    /// The point the anchor last resolved to.
    pub last: Point,
    /// Which coordinates the object drives.
    #[serde(default)]
    pub axis: AnchorAxis,
    /// Walls: inches along the wall beyond its start (negative) or end
    /// (positive), where a dimension runs to the outer corner of a building.
    #[serde(default)]
    pub extra: f64,
}

/// Which coordinates of its end an anchor drives. A dimension that measures
/// along an axis only follows the object along that axis: its other
/// coordinate is the dimension's own measuring line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AnchorAxis {
    /// Both coordinates: the end lies on the object.
    #[default]
    Both,
    /// Only x (a horizontal dimension).
    X,
    /// Only y (a vertical dimension).
    Y,
}

/// The object a tool located a dimension's end on, and the point on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DimHint {
    pub target: AnchorTarget,
    pub id: Id,
    /// The located point on the object (the dimension's end can be its
    /// projection onto the measuring axis).
    pub point: Point,
}

/// The objects an anchor can be tied to, borrowed from a floor.
pub struct Targets<'a> {
    pub walls: &'a [Wall],
    pub openings: &'a [Opening],
    pub cabinets: &'a [serde_json::Value],
    pub symbols: &'a [PlacedSymbol],
    pub cad: &'a [CadObject],
    pub stairs: &'a [serde_json::Value],
    /// The electrical layer's `devices` records.
    pub devices: &'a [serde_json::Value],
}

/// The `devices` array of a floor's opaque electrical layer.
fn device_records(floor: &Floor) -> &[serde_json::Value] {
    floor
        .electrical
        .as_ref()
        .and_then(|e| e.get("devices"))
        .and_then(serde_json::Value::as_array)
        .map_or(&[], Vec::as_slice)
}

impl<'a> Targets<'a> {
    pub fn of(floor: &'a Floor) -> Self {
        Self {
            walls: &floor.walls,
            openings: &floor.openings,
            cabinets: &floor.cabinets,
            symbols: &floor.symbols,
            cad: &floor.cad,
            stairs: &floor.stairs,
            devices: device_records(floor),
        }
    }

    fn cad_object(&self, id: Id) -> Option<&'a CadObject> {
        self.cad.iter().find(|c| c.id == id)
    }

    fn wall(&self, id: Id) -> Option<&'a Wall> {
        self.walls.iter().find(|w| w.id == id)
    }

    fn opening(&self, id: Id) -> Option<&'a Opening> {
        self.openings.iter().find(|o| o.id == id)
    }

    fn frame(&self, target: AnchorTarget, id: Id) -> Option<Frame> {
        match target {
            AnchorTarget::Cabinet => self
                .cabinets
                .iter()
                .find(|v| v.get("id").and_then(serde_json::Value::as_u64) == Some(id))
                .and_then(cabinet_frame),
            AnchorTarget::Symbol => self.symbols.iter().find(|s| s.id == id).map(|s| {
                let a = s.angle.to_radians();
                let u = Point::new(a.cos(), a.sin());
                Frame {
                    origin: s.position.sub(u.scale(s.width * 0.5)),
                    u,
                    v: u.perp(),
                    w: s.width,
                    d: s.depth,
                }
            }),
            AnchorTarget::Stair => self
                .stairs
                .iter()
                .find(|v| v.get("id").and_then(serde_json::Value::as_u64) == Some(id))
                .and_then(|v| pose_frame(v, "origin", "direction")),
            AnchorTarget::Device => self
                .devices
                .iter()
                .find(|v| v.get("id").and_then(serde_json::Value::as_u64) == Some(id))
                .and_then(|v| pose_frame(v, "position", "angle")),
            _ => None,
        }
    }
}

/// A frame at the point `pos` of a JSON record turned by its `angle` (radians):
/// the reference of a stair or an electrical device.
fn pose_frame(v: &serde_json::Value, pos: &str, angle: &str) -> Option<Frame> {
    let p = v.get(pos)?;
    let origin = Point::new(p.get("x")?.as_f64()?, p.get("y")?.as_f64()?);
    let a = v
        .get(angle)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);
    let u = Point::new(a.cos(), a.sin());
    Some(Frame {
        origin,
        u,
        v: u.perp(),
        w: 0.0,
        d: 0.0,
    })
}

/// A cabinet's or fixture's rectangle: its back-left corner, the unit
/// vectors along its width and depth, and its size.
#[derive(Debug, Clone, Copy)]
struct Frame {
    origin: Point,
    u: Point,
    v: Point,
    w: f64,
    d: f64,
}

impl Frame {
    fn local(&self, p: Point) -> (f64, f64) {
        let r = p.sub(self.origin);
        (r.dot(self.u), r.dot(self.v))
    }

    fn world(&self, u: f64, v: f64) -> Point {
        self.origin.add(self.u.scale(u)).add(self.v.scale(v))
    }
}

/// The frame of a cabinet stored as JSON (`position` is its back-left
/// corner, `angle` is in radians).
fn cabinet_frame(v: &serde_json::Value) -> Option<Frame> {
    let num = |k: &str| v.get(k).and_then(serde_json::Value::as_f64);
    let pos = v.get("position")?;
    let origin = Point::new(pos.get("x")?.as_f64()?, pos.get("y")?.as_f64()?);
    let a = num("angle").unwrap_or(0.0);
    let u = Point::new(a.cos(), a.sin());
    Some(Frame {
        origin,
        u,
        v: u.perp(),
        w: num("width")?,
        d: num("depth")?,
    })
}

impl DimAnchor {
    /// The point this anchor names on `wall` now (a wall anchor).
    pub fn resolve(&self, wall: &Wall) -> Point {
        let base = match self.at {
            DimAttach::Start => wall.start,
            DimAttach::End => wall.end,
            DimAttach::Along(t) => Point::lerp(wall.start, wall.end, t),
            DimAttach::Local { .. } | DimAttach::Vertex(_) => wall.start,
        };
        base.add(wall.direction().scale(self.extra))
            .add(wall.normal().scale(self.side))
    }

    /// The point this anchor names now, or `None` when its object is gone.
    pub fn resolve_in(&self, t: &Targets) -> Option<Point> {
        match self.target {
            AnchorTarget::Wall => t.wall(self.wall).map(|w| self.resolve(w)),
            AnchorTarget::Opening => {
                let o = t.opening(self.wall)?;
                let w = t.wall(o.wall_id)?;
                let off = match self.at {
                    DimAttach::Start => o.start_offset(),
                    DimAttach::End => o.end_offset(),
                    DimAttach::Along(f) => o.start_offset() + f * o.width,
                    DimAttach::Local { .. } | DimAttach::Vertex(_) => o.center_offset,
                };
                // `off` is arc length on a curved wall (DW-88).
                Some(w.point_along(off).add(w.normal_along(off).scale(self.side)))
            }
            AnchorTarget::Cabinet
            | AnchorTarget::Symbol
            | AnchorTarget::Stair
            | AnchorTarget::Device => {
                let f = t.frame(self.target, self.wall)?;
                match self.at {
                    DimAttach::Local { u, v } => Some(f.world(u, v)),
                    _ => None,
                }
            }
            AnchorTarget::Cad => cad_point(t.cad_object(self.wall)?, self.at),
        }
    }

    /// The point the dimension's end is at now: the object's point, merged
    /// with `current` for an axis-only anchor.
    pub fn resolve_for(&self, t: &Targets, current: Point) -> Option<Point> {
        let p = self.resolve_in(t)?;
        Some(match self.axis {
            AnchorAxis::Both => p,
            AnchorAxis::X => Point::new(p.x, current.y),
            AnchorAxis::Y => Point::new(current.x, p.y),
        })
    }

    /// A short description for the Dimension Specification: `Wall 12 start`,
    /// `Opening 5 center`, `Cabinet 7`.
    pub fn describe(&self) -> String {
        let place = match (self.target, self.at) {
            (AnchorTarget::Wall, DimAttach::Start) => " start",
            (AnchorTarget::Wall, DimAttach::End) => " end",
            (AnchorTarget::Wall, DimAttach::Along(_)) => " along",
            (AnchorTarget::Opening, DimAttach::Start) => " start side",
            (AnchorTarget::Opening, DimAttach::End) => " end side",
            (AnchorTarget::Opening, DimAttach::Along(_)) => " center",
            _ => "",
        };
        let face = match self.target {
            AnchorTarget::Wall | AnchorTarget::Opening if self.side.abs() < 0.01 => ", centerline",
            AnchorTarget::Wall | AnchorTarget::Opening => ", surface",
            _ => "",
        };
        let place = match (self.target, self.at) {
            (AnchorTarget::Cad, DimAttach::Start) => " start",
            (AnchorTarget::Cad, DimAttach::End) => " end",
            (AnchorTarget::Cad, DimAttach::Vertex(i)) => {
                return format!("CAD Object {} point {}", self.wall, i + 1)
            }
            _ => place,
        };
        format!("{} {}{place}{face}", self.target.label(), self.wall)
    }
}

/// The reference point of a CAD item `Local` offsets are measured from: a
/// line's start, a circle's or arc's center, a polyline's first vertex, a
/// text's anchor.
fn cad_reference(item: &CadItem) -> Point {
    match item {
        CadItem::Line { a, .. } => *a,
        CadItem::Arc { center, .. } | CadItem::Circle { center, .. } => *center,
        CadItem::Polyline { points, .. } => points.first().copied().unwrap_or(Point::ZERO),
        CadItem::Text { pos, .. } => *pos,
    }
}

/// The point an anchor names on a CAD object now.
fn cad_point(c: &CadObject, at: DimAttach) -> Option<Point> {
    match (&c.item, at) {
        (CadItem::Line { a, .. }, DimAttach::Start) => Some(*a),
        (CadItem::Line { b, .. }, DimAttach::End) => Some(*b),
        (CadItem::Line { a, b }, DimAttach::Along(t)) => Some(Point::lerp(*a, *b, t)),
        (CadItem::Polyline { points, .. }, DimAttach::Start) => points.first().copied(),
        (CadItem::Polyline { points, .. }, DimAttach::End) => points.last().copied(),
        (CadItem::Polyline { points, .. }, DimAttach::Vertex(i)) => points.get(i as usize).copied(),
        (item, DimAttach::Local { u, v }) => Some(cad_reference(item).add(Point::new(u, v))),
        _ => None,
    }
}

/// The anchor for `p` on a CAD object: a line's ends or points along it, a
/// polyline's vertices, a circle's or arc's center (or any point of them
/// when `anywhere`).
fn cad_anchor(c: &CadObject, p: Point, anywhere: bool) -> Option<DimAnchor> {
    let near = |q: Point| q.dist(p) <= ATTACH_TOL;
    let at = match &c.item {
        CadItem::Line { a, b } => {
            if near(*a) {
                DimAttach::Start
            } else if near(*b) {
                DimAttach::End
            } else {
                let (t, q) = crate::geometry::project_on_segment(p, *a, *b);
                if near(q) && (anywhere || (0.0..=1.0).contains(&t)) {
                    DimAttach::Along(t)
                } else {
                    return None;
                }
            }
        }
        CadItem::Polyline { points, .. } => {
            let i = points.iter().position(|q| near(*q))?;
            DimAttach::Vertex(i as u32)
        }
        CadItem::Circle { center, radius } | CadItem::Arc { center, radius, .. } => {
            let on = near(*center) || (p.dist(*center) - radius).abs() <= ATTACH_TOL;
            if !on && !anywhere {
                return None;
            }
            let o = p.sub(*center);
            DimAttach::Local { u: o.x, v: o.y }
        }
        CadItem::Text { pos, .. } => {
            if !near(*pos) && !anywhere {
                return None;
            }
            let o = p.sub(*pos);
            DimAttach::Local { u: o.x, v: o.y }
        }
    };
    let last = cad_point(c, at)?;
    Some(DimAnchor {
        wall: c.id,
        target: AnchorTarget::Cad,
        at,
        side: 0.0,
        last,
        axis: AnchorAxis::Both,
        extra: 0.0,
    })
}

/// The anchor for `p` against a record with a pose (a stair or a device): a
/// point of the object, kept in the object's own frame so it follows moves
/// and turns.
fn pose_anchor(target: AnchorTarget, id: Id, f: &Frame, p: Point) -> DimAnchor {
    let (u, v) = f.local(p);
    DimAnchor {
        wall: id,
        target,
        at: DimAttach::Local { u, v },
        side: 0.0,
        last: f.world(u, v),
        axis: AnchorAxis::Both,
        extra: 0.0,
    }
}

/// How close to a device's position a point ties to the device, inches.
const DEVICE_TOL: f64 = 6.0;

/// Ends closer than this to an object point are tied to it, inches.
pub const ATTACH_TOL: f64 = 0.5;

/// The anchor for `p` on `wall`, when `p` lies within the wall's thickness
/// (on its centerline, a face or a layer line) at its start, its end or
/// within its length.
fn anchor_for(wall: &Wall, p: Point) -> Option<DimAnchor> {
    let len = wall.length();
    if len < 1e-6 {
        return None;
    }
    let (dir, normal) = (wall.direction(), wall.normal());
    let rel = p.sub(wall.start);
    let along = rel.dot(dir);
    let side = rel.dot(normal);
    let half = wall.thickness * 0.5;
    // Past an end by up to the half thickness is the outer corner of the
    // wall's footprint, where a building's overall dimension runs.
    if side.abs() > half + ATTACH_TOL
        || along < -(half + ATTACH_TOL)
        || along > len + half + ATTACH_TOL
    {
        return None;
    }
    // Snap the side to the exact line it is on.
    let side = [0.0, half, -half]
        .into_iter()
        .find(|s| (side - s).abs() <= ATTACH_TOL)
        .unwrap_or(side);
    let (at, extra) = if along <= ATTACH_TOL {
        (DimAttach::Start, along.min(0.0))
    } else if along >= len - ATTACH_TOL {
        (DimAttach::End, (along - len).max(0.0))
    } else {
        (DimAttach::Along(along / len), 0.0)
    };
    let mut a = DimAnchor {
        wall: wall.id,
        target: AnchorTarget::Wall,
        at,
        side,
        last: p,
        axis: AnchorAxis::Both,
        extra,
    };
    a.last = a.resolve(wall);
    Some(a)
}

/// The anchor for `p` on `opening` (in `wall`): its start or end side, or
/// its center, within the wall's thickness.
fn opening_anchor(wall: &Wall, o: &Opening, p: Point) -> Option<DimAnchor> {
    if wall.length() < 1e-6 {
        return None;
    }
    // Arc length and distance off the centerline: a curved wall carries its
    // openings along the arc (DW-88).
    let (along, side) = wall.locate(p);
    if side.abs() > wall.thickness * 0.5 + ATTACH_TOL {
        return None;
    }
    let at = if (along - o.start_offset()).abs() <= ATTACH_TOL {
        DimAttach::Start
    } else if (along - o.end_offset()).abs() <= ATTACH_TOL {
        DimAttach::End
    } else if (along - o.center_offset).abs() <= ATTACH_TOL {
        DimAttach::Along(0.5)
    } else {
        return None;
    };
    let half = wall.thickness * 0.5;
    let side = [0.0, half, -half]
        .into_iter()
        .find(|s| (side - s).abs() <= ATTACH_TOL)
        .unwrap_or(side);
    let mut a = DimAnchor {
        wall: o.id,
        target: AnchorTarget::Opening,
        at,
        side,
        last: p,
        axis: AnchorAxis::Both,
        extra: 0.0,
    };
    let targets = Targets {
        walls: std::slice::from_ref(wall),
        openings: std::slice::from_ref(o),
        cabinets: &[],
        symbols: &[],
        cad: &[],
        stairs: &[],
        devices: &[],
    };
    a.last = a.resolve_in(&targets)?;
    Some(a)
}

/// The anchor for `p` on a cabinet or fixture frame: within tolerance of its
/// outline (`anywhere` also takes a point inside it).
fn frame_anchor(
    target: AnchorTarget,
    id: Id,
    f: &Frame,
    p: Point,
    anywhere: bool,
) -> Option<DimAnchor> {
    let (u, v) = f.local(p);
    let inside =
        u >= -ATTACH_TOL && u <= f.w + ATTACH_TOL && v >= -ATTACH_TOL && v <= f.d + ATTACH_TOL;
    let on_edge = [u, f.w - u, v, f.d - v]
        .iter()
        .any(|d| d.abs() <= ATTACH_TOL);
    if !inside || !(anywhere || on_edge) {
        return None;
    }
    let a = DimAnchor {
        wall: id,
        target,
        at: DimAttach::Local { u, v },
        side: 0.0,
        last: p,
        axis: AnchorAxis::Both,
        extra: 0.0,
    };
    Some(DimAnchor {
        last: f.world(u, v),
        ..a
    })
}

/// The anchor for `p` on whichever object it lies on: opening sides and
/// centers first, then walls, then cabinets and fixtures.
pub fn find_anchor(t: &Targets, p: Point) -> Option<DimAnchor> {
    for o in t.openings {
        if let Some(a) = t.wall(o.wall_id).and_then(|w| opening_anchor(w, o, p)) {
            return Some(a);
        }
    }
    if let Some(a) = t.walls.iter().find_map(|w| anchor_for(w, p)) {
        return Some(a);
    }
    for c in t.cabinets {
        let Some(id) = c.get("id").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        if let Some(a) =
            cabinet_frame(c).and_then(|f| frame_anchor(AnchorTarget::Cabinet, id, &f, p, false))
        {
            return Some(a);
        }
    }
    for s in t.symbols {
        let a = t
            .frame(AnchorTarget::Symbol, s.id)
            .and_then(|f| frame_anchor(AnchorTarget::Symbol, s.id, &f, p, false));
        if a.is_some() {
            return a;
        }
    }
    for c in t.cad {
        if let Some(a) = cad_anchor(c, p, false) {
            return Some(a);
        }
    }
    for d in t.devices {
        let Some(id) = d.get("id").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        if let Some(f) =
            pose_frame(d, "position", "angle").filter(|f| f.origin.dist(p) <= DEVICE_TOL)
        {
            return Some(pose_anchor(AnchorTarget::Device, id, &f, p));
        }
    }
    None
}

/// The anchor for `p` on the object the tool located it on (`hint`), when it
/// really lies there.
fn hinted_anchor(t: &Targets, hint: (AnchorTarget, Id), p: Point) -> Option<DimAnchor> {
    let (target, id) = hint;
    match target {
        AnchorTarget::Wall => t.wall(id).and_then(|w| anchor_for(w, p)),
        AnchorTarget::Opening => {
            let o = t.opening(id)?;
            opening_anchor(t.wall(o.wall_id)?, o, p)
        }
        AnchorTarget::Cabinet | AnchorTarget::Symbol => t
            .frame(target, id)
            .and_then(|f| frame_anchor(target, id, &f, p, true)),
        AnchorTarget::Cad => t.cad_object(id).and_then(|c| cad_anchor(c, p, true)),
        AnchorTarget::Stair | AnchorTarget::Device => {
            t.frame(target, id).map(|f| pose_anchor(target, id, &f, p))
        }
    }
}

impl Floor {
    /// Ties the ends of dimension `id` to the objects they lie on (an end
    /// that lies on nothing stays free). Returns how many ends were tied.
    /// See [`Floor::attach_dimension_hinted`].
    pub fn attach_dimension(&mut self, id: Id) -> usize {
        self.attach_dimension_hinted(id, [None, None])
    }

    /// Like [`Floor::attach_dimension`], preferring for each end the object
    /// the tool located it on. Manual dimensions and the automatic
    /// exterior and interior strings are tied; a typed text (and a baseline
    /// string's computed one) would go stale as the ends move, so those are
    /// not.
    pub fn attach_dimension_hinted(&mut self, id: Id, hints: [Option<DimHint>; 2]) -> usize {
        let Some(i) = self.dimensions.iter().position(|d| d.id == id) else {
            return 0;
        };
        let d = &self.dimensions[i];
        let tieable = d.kind == DimensionKind::Manual
            || (d.kind == DimensionKind::AutoExterior
                && matches!(
                    d.auto_group,
                    AutoGroup::Exterior | AutoGroup::Interior | AutoGroup::Nkba
                ));
        if !tieable || d.text_override.is_some() {
            return 0;
        }
        let ends = [d.start, d.end];
        // A dimension measuring along an axis keeps its measuring line.
        let axis = if (d.start.y - d.end.y).abs() < 1e-6 {
            AnchorAxis::X
        } else if (d.start.x - d.end.x).abs() < 1e-6 {
            AnchorAxis::Y
        } else {
            AnchorAxis::Both
        };
        let t = Targets::of(self);
        let mut anchors = [None, None];
        for (k, p) in ends.into_iter().enumerate() {
            anchors[k] = hints[k]
                .and_then(|h| {
                    let on = (h.target, h.id);
                    hinted_anchor(&t, on, p).or_else(|| {
                        // The end is the located point projected onto the
                        // measuring axis: the object drives that axis.
                        hinted_anchor(&t, on, h.point)
                            .filter(|_| axis != AnchorAxis::Both)
                            .map(|a| DimAnchor { last: p, axis, ..a })
                    })
                })
                .or_else(|| find_anchor(&t, p));
        }
        // An end snaps to the object feature it is tied to, so the tie holds
        // from the start (an end that differs from its feature is taken for
        // one moved by hand).
        let mut pts = ends;
        for k in 0..2 {
            if let Some(a) = anchors[k] {
                if let Some(r) = a.resolve_for(&t, ends[k]) {
                    pts[k] = r;
                    anchors[k] = Some(DimAnchor { last: r, ..a });
                }
            }
        }
        let n = anchors.iter().flatten().count();
        let d = &mut self.dimensions[i];
        d.anchors = anchors;
        d.start = pts[0];
        d.end = pts[1];
        n
    }

    /// Moves the tied ends of every dimension to where their objects are
    /// now. A tie to an object that no longer exists, or to an end that was
    /// moved by hand, is dropped (and an automatic dimension edited that way
    /// becomes manual). Returns whether anything changed.
    pub fn sync_dimension_anchors(&mut self) -> bool {
        let Floor {
            walls,
            openings,
            cabinets,
            symbols,
            cad,
            stairs,
            electrical,
            dimensions,
            ..
        } = self;
        let devices = electrical
            .as_ref()
            .and_then(|e| e.get("devices"))
            .and_then(serde_json::Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let t = Targets {
            walls,
            openings,
            cabinets,
            symbols,
            cad,
            stairs,
            devices,
        };
        let mut changed = false;
        for d in dimensions.iter_mut() {
            if d.anchors.iter().all(Option::is_none) {
                continue;
            }
            changed |= sync_dimension(d, &t);
        }
        changed
    }
}

fn sync_dimension(d: &mut Dimension, t: &Targets) -> bool {
    let mut changed = false;
    let mut dropped = false;
    let moved_from = d.line_points();
    for k in 0..2 {
        let Some(a) = d.anchors[k] else { continue };
        let current = if k == 0 { d.start } else { d.end };
        let target = a.resolve_for(t, current);
        // The object is gone, or the end was moved by hand since the last
        // sync: let go.
        let Some(target) = target.filter(|_| current.dist(a.last) <= 1e-6) else {
            d.anchors[k] = None;
            changed = true;
            dropped = true;
            continue;
        };
        if target.dist(current) > 1e-9 {
            if k == 0 {
                d.start = target;
            } else {
                d.end = target;
            }
            d.anchors[k] = Some(DimAnchor { last: target, ..a });
            changed = true;
        }
    }
    // DIM-35: a manual dimension line stays where it was when the object it
    // measures moves; only the value changes. An automatic string keeps its
    // distance from the wall.
    if changed && d.kind == DimensionKind::Manual && d.length() > 1e-9 {
        let mid = Point::lerp(moved_from.0, moved_from.1, 0.5);
        d.offset = mid.sub(d.start).dot(d.end.sub(d.start).normalized().perp());
    }
    if dropped && d.kind == DimensionKind::AutoExterior && d.auto_group != AutoGroup::None {
        d.kind = DimensionKind::Manual;
        d.auto_group = AutoGroup::None;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, WallKind};

    fn house() -> Project {
        let mut p = Project::new("t");
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 144.0),
            Point::new(0.0, 144.0),
        ];
        for i in 0..4 {
            p.add_wall(0, pts[i], pts[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
        p
    }

    fn dim(p: &mut Project, a: Point, b: Point) -> Id {
        let id = p.add_dimension(0, Dimension::new(0, DimensionKind::Manual, a, b, 24.0));
        p.floors[0].attach_dimension(id);
        id
    }

    fn hint(target: AnchorTarget, id: Id, point: Point) -> Option<DimHint> {
        Some(DimHint { target, id, point })
    }

    fn get(p: &Project, id: Id) -> &Dimension {
        p.floors[0].dimensions.iter().find(|d| d.id == id).unwrap()
    }

    #[test]
    fn a_dimension_between_wall_corners_follows_a_moved_wall() {
        let mut p = house();
        // Outer faces of the south wall's two ends: x 0..240 at y = -3.
        let id = dim(&mut p, Point::new(0.0, -3.0), Point::new(240.0, -3.0));
        let a = get(&p, id).anchors;
        assert!(a[0].is_some() && a[1].is_some());
        assert_eq!(a[0].unwrap().at, DimAttach::Start);
        assert_eq!(a[1].unwrap().at, DimAttach::End);
        // Nothing moved: nothing changes.
        assert!(!p.floors[0].sync_dimension_anchors());
        // Stretch the east end of the south wall by 60".
        let south = p.floors[0].walls[0].id;
        p.floors[0].wall_mut(south).unwrap().end = Point::new(300.0, 0.0);
        assert!(p.floors[0].sync_dimension_anchors());
        let d = get(&p, id);
        assert_eq!(d.start, Point::new(0.0, -3.0));
        assert!(d.end.dist(Point::new(300.0, -3.0)) < 1e-9, "{:?}", d.end);
        assert!((d.length() - 300.0).abs() < 1e-9);
        // Move the whole wall up: both ends follow.
        let w = p.floors[0].wall_mut(south).unwrap();
        w.start = Point::new(0.0, 10.0);
        w.end = Point::new(300.0, 10.0);
        p.floors[0].sync_dimension_anchors();
        let d = get(&p, id);
        assert!(d.start.dist(Point::new(0.0, 7.0)) < 1e-9);
        assert!(d.end.dist(Point::new(300.0, 7.0)) < 1e-9);
        // Idempotent.
        assert!(!p.floors[0].sync_dimension_anchors());
    }

    #[test]
    fn a_point_along_a_wall_keeps_its_fraction() {
        let mut p = house();
        let id = dim(&mut p, Point::new(60.0, 0.0), Point::new(240.0, 144.0));
        let a = get(&p, id).anchors;
        assert_eq!(a[0].unwrap().at, DimAttach::Along(0.25));
        let south = p.floors[0].walls[0].id;
        p.floors[0].wall_mut(south).unwrap().end = Point::new(480.0, 0.0);
        p.floors[0].sync_dimension_anchors();
        assert!(get(&p, id).start.dist(Point::new(120.0, 0.0)) < 1e-9);
    }

    #[test]
    fn a_hand_moved_end_and_a_deleted_wall_let_go() {
        let mut p = house();
        let id = dim(&mut p, Point::new(0.0, 0.0), Point::new(240.0, 0.0));
        // The user drags the start by hand.
        p.floors[0]
            .dimensions
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap()
            .start = Point::new(10.0, 5.0);
        let south = p.floors[0].walls[0].id;
        p.floors[0].wall_mut(south).unwrap().end = Point::new(300.0, 0.0);
        p.floors[0].sync_dimension_anchors();
        let d = get(&p, id);
        assert_eq!(d.start, Point::new(10.0, 5.0), "not dragged back");
        assert!(d.anchors[0].is_none());
        assert!(d.anchors[1].is_some());
        assert!(d.end.dist(Point::new(300.0, 0.0)) < 1e-9);
        // The wall goes away: the end stays where it was and is free.
        p.floors[0].walls.retain(|w| w.id != south);
        p.floors[0].sync_dimension_anchors();
        let d = get(&p, id);
        assert!(d.anchors.iter().all(Option::is_none));
        assert!(d.end.dist(Point::new(300.0, 0.0)) < 1e-9);
    }

    #[test]
    fn free_ends_and_other_kinds_are_not_tied() {
        let mut p = house();
        let id = dim(&mut p, Point::new(500.0, 500.0), Point::new(600.0, 500.0));
        assert!(get(&p, id).anchors.iter().all(Option::is_none));
        let auto = p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::AutoExterior,
                Point::new(0.0, 0.0),
                Point::new(240.0, 0.0),
                24.0,
            ),
        );
        assert_eq!(p.floors[0].attach_dimension(auto), 0);
        // Old files without the field load.
        let json = serde_json::to_string(get(&p, id)).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut().unwrap().remove("anchors");
        let back: Dimension = serde_json::from_value(v).unwrap();
        assert!(back.anchors.iter().all(Option::is_none));
    }

    #[test]
    fn anchors_survive_the_json() {
        let mut p = house();
        let id = dim(&mut p, Point::new(0.0, 0.0), Point::new(240.0, 0.0));
        let json = serde_json::to_string(get(&p, id)).unwrap();
        let back: Dimension = serde_json::from_str(&json).unwrap();
        assert_eq!(&back, get(&p, id));
    }

    #[test]
    fn an_opening_side_anchor_follows_a_moved_wall_and_a_moved_opening() {
        let mut p = house();
        let south = p.floors[0].walls[0].id;
        let win = p
            .add_opening(0, south, 100.0, crate::model::OpeningKind::Window)
            .unwrap();
        let w = p.floors[0].openings.iter().find(|o| o.id == win).unwrap();
        let (a, b) = (w.start_offset(), w.end_offset());
        // The window's two sides on the south wall's outer face.
        let id = dim(&mut p, Point::new(a, -3.0), Point::new(b, -3.0));
        let anchors = get(&p, id).anchors;
        for (k, at) in [(0, DimAttach::Start), (1, DimAttach::End)] {
            let an = anchors[k].unwrap();
            assert_eq!(
                (an.target, an.wall, an.at),
                (AnchorTarget::Opening, win, at)
            );
        }
        assert_eq!(
            anchors[0].unwrap().describe(),
            format!("Opening {win} start side, surface")
        );
        // Stretching the wall leaves the window where it is on the wall.
        p.floors[0].wall_mut(south).unwrap().end = Point::new(480.0, 0.0);
        p.floors[0].sync_dimension_anchors();
        assert!(get(&p, id).start.dist(Point::new(a, -3.0)) < 1e-9);
        // Moving the wall moves the string.
        let w = p.floors[0].wall_mut(south).unwrap();
        w.start = Point::new(0.0, 12.0);
        w.end = Point::new(480.0, 12.0);
        assert!(p.floors[0].sync_dimension_anchors());
        let d = get(&p, id);
        assert!(d.start.dist(Point::new(a, 9.0)) < 1e-9, "{:?}", d.start);
        assert!(d.end.dist(Point::new(b, 9.0)) < 1e-9);
        assert!((d.length() - (b - a)).abs() < 1e-9);
        // DIM-35: the manual dimension line stays where it was (y = 21).
        assert!(
            (d.line_points().0.y - 21.0).abs() < 1e-9,
            "{:?}",
            d.line_points()
        );
        // Sliding the window along its wall moves the string with it.
        p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == win)
            .unwrap()
            .center_offset = 200.0;
        p.floors[0].sync_dimension_anchors();
        let d = get(&p, id);
        assert!(d.start.dist(Point::new(a + 100.0, 9.0)) < 1e-9);
        // The window goes away: both ends are free where they were.
        p.floors[0].openings.retain(|o| o.id != win);
        p.floors[0].sync_dimension_anchors();
        let d = get(&p, id);
        assert!(d.anchors.iter().all(Option::is_none));
        assert!(d.start.dist(Point::new(a + 100.0, 9.0)) < 1e-9);
    }

    #[test]
    fn a_cabinet_side_anchor_follows_the_cabinet() {
        let mut p = house();
        p.floors[0].cabinets.push(serde_json::json!({
            "id": 900,
            "position": {"x": 10.0, "y": 20.0},
            "angle": 0.0,
            "width": 30.0,
            "depth": 24.0
        }));
        // The cabinet's left side, at 10" from its back, and a free point.
        let id = dim(&mut p, Point::new(10.0, 30.0), Point::new(100.0, 30.0));
        let a = get(&p, id).anchors;
        let an = a[0].unwrap();
        assert_eq!((an.target, an.wall), (AnchorTarget::Cabinet, 900));
        assert!(a[1].is_none());
        p.floors[0].cabinets[0]["position"]["x"] = serde_json::json!(50.0);
        assert!(p.floors[0].sync_dimension_anchors());
        assert!(get(&p, id).start.dist(Point::new(50.0, 30.0)) < 1e-9);
        // Turned a quarter: the side swings with it.
        p.floors[0].cabinets[0]["angle"] = serde_json::json!(std::f64::consts::FRAC_PI_2);
        p.floors[0].sync_dimension_anchors();
        assert!(
            get(&p, id).start.dist(Point::new(40.0, 20.0)) < 1e-9,
            "{:?}",
            get(&p, id).start
        );
    }

    #[test]
    fn a_hint_wins_and_main_layer_faces_are_tied_to_the_wall() {
        let mut p = house();
        // A point on a main-layer line, 2" off the centerline.
        let id = p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(60.0, -2.0),
                Point::new(180.0, -2.0),
                24.0,
            ),
        );
        let south = p.floors[0].walls[0].id;
        let north = p.floors[0].walls[2].id;
        assert_eq!(
            p.floors[0].attach_dimension_hinted(
                id,
                [
                    hint(AnchorTarget::Wall, south, Point::new(60.0, -2.0)),
                    None
                ]
            ),
            2
        );
        assert_eq!(get(&p, id).anchors[0].unwrap().side, -2.0);
        p.floors[0].wall_mut(south).unwrap().start = Point::new(0.0, 6.0);
        p.floors[0].wall_mut(south).unwrap().end = Point::new(240.0, 6.0);
        p.floors[0].sync_dimension_anchors();
        assert!((get(&p, id).start.y - 4.0).abs() < 1e-9);
        // A hint at a wall the point is not on falls back to the one it is on.
        let id2 = dim(&mut p, Point::new(0.0, 144.0), Point::new(240.0, 144.0));
        p.floors[0].attach_dimension_hinted(
            id2,
            [hint(AnchorTarget::Wall, south, Point::new(0.0, 0.0)), None],
        );
        assert_eq!(get(&p, id2).anchors[0].unwrap().wall, north);
    }

    #[test]
    fn deleting_a_located_cabinet_or_opening_frees_the_point_where_it_was() {
        let mut p = house();
        p.floors[0].cabinets.push(serde_json::json!({
            "id": 900,
            "position": {"x": 10.0, "y": 20.0},
            "angle": 0.0,
            "width": 30.0,
            "depth": 24.0
        }));
        // Cabinet face points of an automatic NKBA string.
        let mut d = Dimension::new(
            0,
            DimensionKind::AutoExterior,
            Point::new(10.0, 44.0),
            Point::new(40.0, 44.0),
            12.0,
        );
        d.auto_group = AutoGroup::Nkba;
        let id = p.add_dimension(0, d);
        assert_eq!(p.floors[0].attach_dimension(id), 2, "NKBA strings are tied");
        // The cabinet moves: the string follows it.
        p.floors[0].cabinets[0]["position"]["x"] = serde_json::json!(50.0);
        p.floors[0].sync_dimension_anchors();
        assert!(get(&p, id).start.dist(Point::new(50.0, 44.0)) < 1e-9);
        // Deleted: both points stay where they were, free; the automatic
        // string is manual from then on.
        p.floors[0].cabinets.clear();
        p.floors[0].sync_dimension_anchors();
        let got = get(&p, id);
        assert!(got.anchors.iter().all(Option::is_none));
        assert!(got.start.dist(Point::new(50.0, 44.0)) < 1e-9);
        assert!(got.end.dist(Point::new(80.0, 44.0)) < 1e-9);
        assert_eq!(
            (got.kind, got.auto_group),
            (DimensionKind::Manual, AutoGroup::None)
        );
        // An opening: its sides are tied, deleting it frees them.
        let south = p.floors[0].walls[0].id;
        let o = p
            .add_opening(0, south, 120.0, crate::model::OpeningKind::Window)
            .unwrap();
        let (a, b) = {
            let f = &p.floors[0];
            let o = f.openings.iter().find(|x| x.id == o).unwrap();
            let w = f.wall(south).unwrap();
            (
                w.point_along(o.start_offset()),
                w.point_along(o.end_offset()),
            )
        };
        let id = dim(&mut p, a, b);
        assert_eq!(get(&p, id).anchors.iter().flatten().count(), 2);
        p.remove_opening(0, o);
        p.floors[0].sync_dimension_anchors();
        let got = get(&p, id);
        assert!(got
            .anchors
            .iter()
            .flatten()
            .all(|a| a.target != AnchorTarget::Opening));
        assert!(got.start.dist(a) < 1e-9 && got.end.dist(b) < 1e-9);
    }

    #[test]
    fn automatic_strings_follow_their_walls_until_they_are_edited() {
        let mut p = house();
        let mut d = Dimension::new(
            0,
            DimensionKind::AutoExterior,
            Point::new(0.0, -3.0),
            Point::new(240.0, -3.0),
            32.0,
        );
        d.auto_group = AutoGroup::Exterior;
        let id = p.add_dimension(0, d);
        assert_eq!(p.floors[0].attach_dimension(id), 2);
        let south = p.floors[0].walls[0].id;
        p.floors[0].wall_mut(south).unwrap().end = Point::new(300.0, 0.0);
        p.floors[0].sync_dimension_anchors();
        let got = get(&p, id);
        assert_eq!(got.kind, DimensionKind::AutoExterior);
        assert!((got.length() - 300.0).abs() < 1e-9);
        // Edited by hand: the tie lets go and the string becomes manual.
        p.floors[0]
            .dimensions
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap()
            .end = Point::new(310.0, -3.0);
        p.floors[0].sync_dimension_anchors();
        let got = get(&p, id);
        assert_eq!(
            (got.kind, got.auto_group),
            (DimensionKind::Manual, AutoGroup::None)
        );
    }

    #[test]
    fn a_projected_end_follows_its_object_along_the_measuring_axis() {
        let mut p = house();
        let east = p.floors[0].walls[1].id;
        // A horizontal dimension from the west face of the house to a point
        // located on the east wall's outer face but projected onto y = 30.
        let id = p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(-3.0, 300.0),
                Point::new(243.0, 300.0),
                24.0,
            ),
        );
        // The located point itself was at y = 100 on that face.
        let n = p.floors[0].attach_dimension_hinted(
            id,
            [
                None,
                hint(AnchorTarget::Wall, east, Point::new(243.0, 100.0)),
            ],
        );
        assert_eq!(n, 1, "the west end is free");
        let a = get(&p, id).anchors;
        assert_eq!(a[1].unwrap().axis, AnchorAxis::X);
        // The east wall moves 40" out: the end follows along x only.
        let w = p.floors[0].wall_mut(east).unwrap();
        w.start = Point::new(280.0, 0.0);
        w.end = Point::new(280.0, 144.0);
        assert!(p.floors[0].sync_dimension_anchors());
        let d = get(&p, id);
        assert!(d.end.dist(Point::new(283.0, 300.0)) < 1e-9, "{:?}", d.end);
        assert_eq!(d.start, Point::new(-3.0, 300.0));
    }
    #[test]
    fn a_dimension_tied_to_an_opening_on_a_curved_wall_follows_it_along_the_arc() {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        // A semicircular bay.
        p.floors[0].wall_mut(w).unwrap().curve = crate::WallCurve::from_radius(240.0, 120.0, true);
        let o = p
            .add_opening(0, w, 100.0, crate::model::OpeningKind::Window)
            .unwrap();
        let (a, b) = {
            let f = &p.floors[0];
            let wall = f.wall(w).unwrap();
            let op = f.openings.iter().find(|x| x.id == o).unwrap();
            (
                wall.point_along(op.start_offset()),
                wall.point_along(op.end_offset()),
            )
        };
        // The chord and the arc differ, so a chord-based tie would miss.
        assert!(a.dist(p.floors[0].wall(w).unwrap().point_at(100.0 - 18.0)) > 5.0);
        let id = dim(&mut p, a, b);
        let anchors = get(&p, id).anchors;
        assert_eq!(anchors.iter().flatten().count(), 2, "{anchors:?}");
        assert!(anchors
            .iter()
            .flatten()
            .any(|x| x.target == AnchorTarget::Opening));
        // Slide the window along the arc: the dimension goes with it.
        assert!(p.slide_opening(0, o, 160.0));
        p.floors[0].sync_dimension_anchors();
        let (na, nb) = {
            let f = &p.floors[0];
            let wall = f.wall(w).unwrap();
            let op = f.openings.iter().find(|x| x.id == o).unwrap();
            (
                wall.point_along(op.start_offset()),
                wall.point_along(op.end_offset()),
            )
        };
        let d = get(&p, id);
        assert!(
            d.start.dist(na) < 0.6 && d.end.dist(nb) < 0.6,
            "{d:?} {na:?} {nb:?}"
        );
    }

    #[test]
    fn a_dimension_follows_the_ends_of_a_cad_line_and_a_polyline_vertex() {
        let mut p = house();
        let line = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(300.0, 0.0),
                b: Point::new(400.0, 0.0),
            },
        );
        let id = dim(&mut p, Point::new(300.0, 0.0), Point::new(400.0, 0.0));
        let anchors = get(&p, id).anchors;
        assert!(anchors
            .iter()
            .flatten()
            .all(|a| a.target == AnchorTarget::Cad && a.wall == line));
        assert_eq!(anchors[0].unwrap().at, DimAttach::Start);
        assert_eq!(anchors[1].unwrap().at, DimAttach::End);
        // Stretch the line's end, then slide both ends.
        if let Some(c) = p.floors[0].cad.iter_mut().find(|c| c.id == line) {
            c.item = CadItem::Line {
                a: Point::new(300.0, 20.0),
                b: Point::new(450.0, 20.0),
            };
        }
        assert!(p.floors[0].sync_dimension_anchors());
        let d = get(&p, id);
        assert!(d.start.dist(Point::new(300.0, 20.0)) < 1e-9);
        assert!(d.end.dist(Point::new(450.0, 20.0)) < 1e-9);
        // A polyline vertex keeps its index when the polyline moves.
        let poly = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Polyline {
                points: vec![
                    Point::new(0.0, 500.0),
                    Point::new(100.0, 500.0),
                    Point::new(100.0, 560.0),
                ],
                closed: false,
            },
        );
        let id2 = dim(&mut p, Point::new(100.0, 500.0), Point::new(100.0, 560.0));
        let a = get(&p, id2).anchors;
        assert_eq!(a[0].unwrap().at, DimAttach::Vertex(1));
        assert_eq!(a[1].unwrap().at, DimAttach::End);
        assert!(a[0].unwrap().describe().contains("point 2"));
        if let Some(c) = p.floors[0].cad.iter_mut().find(|c| c.id == poly) {
            if let CadItem::Polyline { points, .. } = &mut c.item {
                for q in points.iter_mut() {
                    q.y += 40.0;
                }
            }
        }
        assert!(p.floors[0].sync_dimension_anchors());
        let d = get(&p, id2);
        assert!(d.start.dist(Point::new(100.0, 540.0)) < 1e-9, "{:?}", d.start);
        assert!(d.end.dist(Point::new(100.0, 600.0)) < 1e-9);
    }

    #[test]
    fn a_circle_center_a_stair_and_an_electrical_device_keep_their_dimensions() {
        let mut p = house();
        let circle = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::new(60.0, 60.0),
                radius: 10.0,
            },
        );
        // A dimension from the circle's center to the south wall's face.
        let id = dim(&mut p, Point::new(60.0, 60.0), Point::new(60.0, 3.0));
        let a = get(&p, id).anchors[0].unwrap();
        assert_eq!((a.target, a.wall), (AnchorTarget::Cad, circle));
        if let Some(c) = p.floors[0].cad.iter_mut().find(|c| c.id == circle) {
            c.item = CadItem::Circle {
                center: Point::new(90.0, 80.0),
                radius: 10.0,
            };
        }
        assert!(p.floors[0].sync_dimension_anchors());
        assert!(get(&p, id).start.dist(Point::new(90.0, 80.0)) < 1e-9);

        // An electrical device: a record with a position.
        p.floors[0].electrical = Some(serde_json::json!({
            "devices": [{"id": 77, "position": {"x": 120.0, "y": 100.0}, "angle": 0.0}]
        }));
        let did = dim(&mut p, Point::new(120.0, 100.0), Point::new(120.0, 3.0));
        let a = get(&p, did).anchors[0].unwrap();
        assert_eq!((a.target, a.wall), (AnchorTarget::Device, 77));
        p.floors[0].electrical = Some(serde_json::json!({
            "devices": [{"id": 77, "position": {"x": 150.0, "y": 90.0}, "angle": 0.0}]
        }));
        assert!(p.floors[0].sync_dimension_anchors());
        assert!(get(&p, did).start.dist(Point::new(150.0, 90.0)) < 1e-9);

        // A stair is tied through a hint (no outline is known here); the
        // point turns with it.
        p.floors[0].stairs = vec![serde_json::json!({
            "id": 9, "origin": {"x": 200.0, "y": 50.0}, "direction": 0.0
        })];
        let sid = p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(230.0, 50.0),
                Point::new(230.0, 3.0),
                20.0,
            ),
        );
        p.floors[0].attach_dimension_hinted(
            sid,
            [
                hint(AnchorTarget::Stair, 9, Point::new(230.0, 50.0)),
                None,
            ],
        );
        let a = get(&p, sid).anchors[0].unwrap();
        assert_eq!((a.target, a.wall), (AnchorTarget::Stair, 9));
        p.floors[0].stairs = vec![serde_json::json!({
            "id": 9, "origin": {"x": 200.0, "y": 50.0}, "direction": std::f64::consts::FRAC_PI_2
        })];
        assert!(p.floors[0].sync_dimension_anchors());
        // 30 inches along the stair's direction now points north.
        assert!(get(&p, sid).start.dist(Point::new(200.0, 80.0)) < 1e-9, "{:?}", get(&p, sid).start);
    }
}
