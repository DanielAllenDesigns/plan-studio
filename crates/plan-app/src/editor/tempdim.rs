//! Temporary dimensions (S-56..S-64): shown for the selected wall, opening,
//! cabinet or CAD object, never saved. Clicking a value turns it into an edit
//! field; Enter applies it by moving the selected object (the referenced
//! object stays fixed, S-60).
//!
//! CAD objects (S-58): a line shows its length and angle, a box (a closed
//! rectangle) its width and height, a circle its radius and diameter and an
//! arc its radius. Typing a value resizes the object: a line keeps its start
//! and turns or stretches its end, a box keeps its lower left corner, a circle
//! its center.
//!
//! A padlock beside a measuring value locks it (S-63): the temporary
//! dimension becomes a permanent manual dimension on the Dimensions layer,
//! tied to the objects it measures, so it follows them and its number can be
//! typed into later (the Dimension tool's value edit). Clicking the padlock
//! again takes that dimension away.

use super::ops;
use super::selection::{ObjectRef, Selection};
use super::{Camera, EditorContext};
use crate::theme::Palette;
use eframe::egui::{self, FontId, Rect, Shape, Stroke, Vec2};
use plan_cabinets::Cabinet;
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::units::parse_ft_in;
use plan_core::{DimFormat, Floor, Opening, OpeningLocate, Wall, WallEnd, WallLocate};
use std::collections::HashMap;

/// What the temporary dimensions locate: the Temporary group of Dimension
/// Defaults > Locate Objects (DIM-40), separate from the manual and
/// automatic dimensions' own. Walls are measured between their surfaces,
/// their main layers or their centerlines; openings between their sides or
/// their centers.
#[derive(Clone, Debug, PartialEq)]
pub struct TempLocate {
    pub walls: WallLocate,
    pub openings: OpeningLocate,
    /// Setup Temporary, Dimension Row Limit: the most separate gap lines
    /// shown on each side of a wall (DIM-53).
    pub row_limit: usize,
    /// Setup Temporary, Reach: how far past the nearest object a further row
    /// still locates one, inches (DIM-53; DECISIONS DM20).
    pub reach: f64,
    /// Exterior walls: located at their exterior (primary) and interior
    /// (secondary) faces; neither means they are not located.
    pub exterior_sides: [bool; 2],
    /// Interior walls: interior (primary) face, exterior (secondary) face
    /// and centers; none means they are not located.
    pub interior_sides: [bool; 3],
    /// Main-layer spans by wall id (only filled for [`WallLocate::MainLayer`]).
    spans: HashMap<u64, (f64, f64)>,
}

impl Default for TempLocate {
    /// Surfaces and sides: face to face, jamb to jamb.
    fn default() -> Self {
        Self {
            walls: WallLocate::Surfaces,
            openings: OpeningLocate::Sides,
            row_limit: 1,
            reach: f64::INFINITY,
            exterior_sides: [true, true],
            interior_sides: [true, true, false],
            spans: HashMap::new(),
        }
    }
}

impl TempLocate {
    /// The active dimension set's Temporary group.
    pub fn of(cx: &EditorContext) -> Self {
        let g = cx.defaults.dimensions.temp_group();
        let spans = if g.walls == WallLocate::MainLayer {
            cx.floor()
                .walls
                .iter()
                .map(|w| {
                    (
                        w.id,
                        crate::tools::dimension::wall_span(cx, w, WallLocate::MainLayer),
                    )
                })
                .collect()
        } else {
            HashMap::new()
        };
        let st = &cx.defaults.dimensions.setup;
        Self {
            walls: g.walls,
            openings: g.openings,
            row_limit: st.temp_row_limit.max(1) as usize,
            reach: st.temp_reach.max(0.0),
            exterior_sides: [st.temp_exterior_primary, st.temp_exterior_secondary],
            interior_sides: [
                st.temp_interior_primary,
                st.temp_interior_secondary,
                st.temp_interior_centers,
            ],
            spans,
        }
    }

    /// Is `w` located at all (Setup Temporary, Walls options)?
    fn locates_wall(&self, w: &Wall) -> bool {
        match w.kind {
            plan_core::model::WallKind::Exterior => self.exterior_sides.iter().any(|b| *b),
            plan_core::model::WallKind::Interior => self.interior_sides.iter().any(|b| *b),
        }
    }

    /// The span another wall `o` is located at: both faces (as `span`), or
    /// only the faces its kind's side options name; offsets along `o`'s own
    /// normal. A single face gives a zero-width span.
    fn other_span(&self, o: &Wall) -> (f64, f64) {
        let (lo, hi) = self.span(o);
        if self.walls == WallLocate::Centers {
            return (lo, hi);
        }
        // Offsets of the exterior and interior faces along `o.normal()`.
        let ext = o.exterior_side.sign();
        let (ext_off, int_off) = if ext >= 0.0 { (hi, lo) } else { (lo, hi) };
        let mut faces: Vec<f64> = Vec::new();
        match o.kind {
            plan_core::model::WallKind::Exterior => {
                if self.exterior_sides[0] {
                    faces.push(ext_off);
                }
                if self.exterior_sides[1] {
                    faces.push(int_off);
                }
            }
            plan_core::model::WallKind::Interior => {
                if self.interior_sides[0] {
                    faces.push(int_off);
                }
                if self.interior_sides[1] {
                    faces.push(ext_off);
                }
                if self.interior_sides[2] {
                    faces.push((lo + hi) * 0.5);
                }
            }
        }
        let a = faces.iter().copied().fold(f64::INFINITY, f64::min);
        let b = faces.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (a, b)
    }

    /// The lateral span `(lo, hi)` located on `w`, offsets along its normal.
    fn span(&self, w: &Wall) -> (f64, f64) {
        let half = w.thickness * 0.5;
        match self.walls {
            WallLocate::Centers => (0.0, 0.0),
            WallLocate::Surfaces => (-half, half),
            WallLocate::MainLayer => self.spans.get(&w.id).copied().unwrap_or((-half, half)),
        }
    }

    /// Opening distances run to the centers (otherwise to the sides).
    fn centers(&self) -> bool {
        self.openings == OpeningLocate::Centers
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TempDimKind {
    /// The wall's own length (Lock: start stays, the end moves).
    WallLength,
    /// Gap to the nearest parallel wall on one side, between the located
    /// surfaces (faces, main layers or centerlines).
    WallGap,
    /// Opening: left jamb to the wall start or the neighbouring opening.
    OpeningToStart,
    /// Opening: right jamb to the wall end or the neighbouring opening.
    OpeningToEnd,
    /// Opening: its width (typing resizes it, DW-12).
    OpeningWidth,
    /// A mulled unit's overall width (read only).
    OpeningUnitWidth,
    /// The wall's angle in degrees counter-clockwise from east (the value is
    /// degrees, not inches). Editing it turns the wall about its start.
    WallAngle,
    /// A curved wall's radius and its arc length along the centerline
    /// (labels only; the length above is the chord). The radius is edited
    /// in the Wall Specification's Curved Wall section.
    WallRadius,
    WallArcLength,
    /// The wall's thickness from face to face (W-28). Typing resizes it about
    /// the reference line the wall keeps fixed (Resize About), so the face
    /// that is not being dimensioned stays where it is.
    WallThickness,
    /// Cabinet: its width (typing resizes it, the left end staying put).
    CabinetWidth,
    /// Cabinet: from its left end to the nearest wall or cabinet, measured
    /// across any opening in the wall behind it (typing slides the cabinet).
    CabinetToLeft,
    /// Cabinet: from its right end to the nearest wall or cabinet.
    CabinetToRight,
    /// Cabinet: from its left end to the near jamb of an opening in the wall
    /// behind it.
    CabinetToOpeningLeft,
    /// Cabinet: from its right end to the near jamb of an opening.
    CabinetToOpeningRight,
    /// CAD line: its length (the start stays, the end moves along the line).
    CadLength,
    /// CAD line: its angle in degrees counter-clockwise from east (the end
    /// turns about the start).
    CadAngle,
    /// CAD box: its width (the lower left corner stays).
    CadWidth,
    /// CAD box: its height.
    CadHeight,
    /// CAD box: the gap from its right side to the nearest parallel wall face
    /// or CAD box side (typing slides the box, S-58).
    CadToRight,
    /// CAD box: the gap from its left side.
    CadToLeft,
    /// CAD box: the gap from its top side.
    CadAbove,
    /// CAD box: the gap from its bottom side.
    CadBelow,
    /// CAD circle or arc: its radius (the center stays).
    CadRadius,
    /// CAD circle: its diameter.
    CadDiameter,
}

impl TempDimKind {
    /// Does this dimension measure a distance between two things? Angles,
    /// radii and arc lengths are labels only, and a mulled unit's width is
    /// read only, so none of those can be locked.
    pub fn lockable(self) -> bool {
        !matches!(
            self,
            TempDimKind::WallAngle
                | TempDimKind::WallRadius
                | TempDimKind::WallArcLength
                | TempDimKind::WallThickness
                | TempDimKind::OpeningUnitWidth
                | TempDimKind::CadAngle
                | TempDimKind::CadRadius
                | TempDimKind::CadDiameter
        )
    }

    /// Is the value degrees rather than inches?
    pub fn is_angle(self) -> bool {
        matches!(self, TempDimKind::WallAngle | TempDimKind::CadAngle)
    }

    /// Drawn as a label only, with no dimension line.
    fn label_only(self) -> bool {
        matches!(
            self,
            TempDimKind::WallAngle
                | TempDimKind::WallRadius
                | TempDimKind::WallArcLength
                | TempDimKind::CadAngle
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TempDim {
    pub kind: TempDimKind,
    /// The measured segment.
    pub a: Point,
    pub b: Point,
    pub value: f64,
    /// Unit direction in which moving the selected object *increases* the value.
    pub axis: Point,
    pub target: ObjectRef,
    /// Where the dimension line is drawn relative to the measured segment:
    /// inches plus screen pixels along the segment's left normal.
    pub offset_in: f64,
    pub offset_px: f32,
    /// The permanent dimension that holds this value locked (S-63), set when
    /// the dimensions are computed.
    pub locked: Option<u64>,
}

impl TempDim {
    /// The drawn dimension line at screen scale `scale` (pixels per inch).
    pub fn line(&self, scale: f64) -> (Point, Point) {
        let n = self.b.sub(self.a).normalized().perp();
        let off = n.scale(self.offset_in + self.offset_px as f64 / scale.max(1e-6));
        (self.a.add(off), self.b.add(off))
    }

    pub fn label_pos(&self, scale: f64) -> Point {
        let (p, q) = self.line(scale);
        Point::lerp(p, q, 0.5)
    }
}

/// The value being typed.
#[derive(Clone, Debug, PartialEq)]
pub struct EditField {
    pub index: usize,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct TempDims {
    pub dims: Vec<TempDim>,
    pub editing: Option<EditField>,
}

impl TempDims {
    /// Rebuilds the dimensions for the current selection (a single wall or
    /// opening). Kept as is while a value is being typed.
    pub fn compute(&mut self, floor: &Floor, selection: &Selection, loc: &TempLocate) {
        if self.editing.is_some() {
            return;
        }
        self.dims.clear();
        match selection.single() {
            Some(ObjectRef::Wall(id)) => wall_dims(floor, id, &mut self.dims, loc),
            Some(ObjectRef::Opening(id)) => opening_dims(floor, id, &mut self.dims, loc),
            Some(ObjectRef::Cabinet(id)) => {
                if let Some(c) = super::placed::cabinet_by_id(floor, id) {
                    self.dims
                        .extend(cabinet_temp_dims(floor, &c, ObjectRef::Cabinet(id), loc));
                }
            }
            Some(ObjectRef::Cad(id)) => cad_dims(floor, id, &mut self.dims),
            _ => {}
        }
        for d in &mut self.dims {
            d.locked = locked_dimension(floor, d);
        }
    }

    pub fn clear(&mut self) {
        self.dims.clear();
        self.editing = None;
    }

    /// Is dimension `index` locked into a permanent dimension?
    pub fn is_locked(&self, index: usize) -> bool {
        self.dims.get(index).is_some_and(|d| d.locked.is_some())
    }

    /// The padlock beside dimension `index`'s value, in plan coordinates.
    pub fn lock_pos(&self, index: usize, scale: f64) -> Option<Point> {
        let d = self.dims.get(index)?;
        let at = d.label_pos(scale);
        Some(Point::new(at.x, at.y + LOCK_DY_PX / scale.max(1e-6)))
    }

    /// The padlock under `p`, if any: only lockable dimensions have one.
    pub fn hit_lock(&self, p: Point, scale: f64) -> Option<usize> {
        let r = LOCK_RADIUS_PX / scale.max(1e-6);
        (0..self.dims.len()).find(|&i| {
            self.dims[i].kind.lockable() && self.lock_pos(i, scale).is_some_and(|q| q.dist(p) <= r)
        })
    }

    /// The value label near `p`, if any.
    pub fn hit_label(&self, p: Point, scale: f64) -> Option<usize> {
        let r = 22.0 / scale.max(1e-6);
        self.dims
            .iter()
            .position(|d| d.label_pos(scale).dist(p) <= r)
    }

    pub fn begin_edit(&mut self, index: usize) -> bool {
        match self.dims.get(index) {
            Some(d) => {
                self.editing = Some(EditField {
                    index,
                    text: format!("{:.3}", d.value)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string(),
                });
                true
            }
            None => false,
        }
    }

    /// Replaces the field with typed text (the first keystroke clears the
    /// old value).
    pub fn type_text(&mut self, s: &str) {
        if let Some(e) = &mut self.editing {
            e.text.push_str(s);
        }
    }

    pub fn backspace(&mut self) {
        if let Some(e) = &mut self.editing {
            e.text.pop();
        }
    }

    /// Tab: the next dimension of the same object.
    pub fn next_field(&mut self) {
        if let Some(e) = &mut self.editing {
            if !self.dims.is_empty() {
                let next = (e.index + 1) % self.dims.len();
                let value = self.dims[next].value;
                *e = EditField {
                    index: next,
                    text: format!("{value:.3}")
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string(),
                };
            }
        }
    }

    pub fn cancel(&mut self) {
        self.editing = None;
    }
}

/// Walls shorter than this show no angle dimension (it would sit on the handles).
const ANGLE_MIN_WALL: f64 = 48.0;
/// The padlock sits this many pixels above the value and is this big.
const LOCK_DY_PX: f64 = 22.0;
const LOCK_RADIUS_PX: f64 = 8.0;

/// The temporary dimensions of a CAD object (S-58).
fn cad_dims(floor: &Floor, id: u64, out: &mut Vec<TempDim>) {
    let Some(c) = floor.cad.iter().find(|c| c.id == id) else {
        return;
    };
    let target = ObjectRef::Cad(id);
    let mut push = |kind, a: Point, b: Point, value: f64, axis: Point, offset_px: f32| {
        out.push(TempDim {
            kind,
            a,
            b,
            value,
            axis,
            target,
            offset_in: 0.0,
            offset_px,
            locked: None,
        });
    };
    match &c.item {
        CadItem::Line { a, b } => {
            let len = a.dist(*b);
            if len < 1e-6 {
                return;
            }
            let u = b.sub(*a).scale(1.0 / len);
            push(TempDimKind::CadLength, *a, *b, len, u, -14.0);
            // The angle sits a quarter of the way along, on the left.
            let at = a.add(u.scale(len * 0.25));
            push(
                TempDimKind::CadAngle,
                at,
                at.add(u),
                super::typed_input::angle_deg(*a, *b),
                u.perp(),
                16.0,
            );
        }
        CadItem::Circle { center, radius } => {
            let e = Point::new(1.0, 0.0);
            push(
                TempDimKind::CadRadius,
                *center,
                Point::new(center.x + radius, center.y),
                *radius,
                e,
                -14.0,
            );
            push(
                TempDimKind::CadDiameter,
                Point::new(center.x - radius, center.y),
                Point::new(center.x + radius, center.y),
                radius * 2.0,
                e,
                16.0,
            );
        }
        CadItem::Arc {
            center,
            radius,
            start_angle,
            ..
        } => {
            let to = Point::new(
                center.x + radius * start_angle.cos(),
                center.y + radius * start_angle.sin(),
            );
            let u = to.sub(*center).normalized();
            push(TempDimKind::CadRadius, *center, to, *radius, u, -14.0);
        }
        CadItem::Polyline {
            points,
            closed: true,
        } => {
            if let Some((lo, hi)) = box_bounds(points) {
                push(
                    TempDimKind::CadWidth,
                    lo,
                    Point::new(hi.x, lo.y),
                    hi.x - lo.x,
                    Point::new(1.0, 0.0),
                    -14.0,
                );
                push(
                    TempDimKind::CadHeight,
                    Point::new(hi.x, lo.y),
                    hi,
                    hi.y - lo.y,
                    Point::new(0.0, 1.0),
                    -14.0,
                );
                box_offsets(floor, id, lo, hi, out);
            }
        }
        _ => {}
    }
}

/// A face that a CAD box's side can face across a gap: a wall face or the
/// side of another CAD box. `vertical` faces stand at `x = at`, horizontal
/// ones at `y = at`; `from..to` is the stretch along the face.
struct Face {
    vertical: bool,
    at: f64,
    from: f64,
    to: f64,
}

fn faces_near(floor: &Floor, skip: u64) -> Vec<Face> {
    let mut v = Vec::new();
    for w in floor.walls.iter().filter(|w| !w.flags.no_locate) {
        let (a, b) = (w.start, w.end);
        let h = w.thickness * 0.5;
        if (a.y - b.y).abs() < 1e-6 {
            let (from, to) = (a.x.min(b.x), a.x.max(b.x));
            for off in [-h, h] {
                v.push(Face {
                    vertical: false,
                    at: a.y + off,
                    from,
                    to,
                });
            }
        } else if (a.x - b.x).abs() < 1e-6 {
            let (from, to) = (a.y.min(b.y), a.y.max(b.y));
            for off in [-h, h] {
                v.push(Face {
                    vertical: true,
                    at: a.x + off,
                    from,
                    to,
                });
            }
        }
    }
    for c in floor.cad.iter().filter(|c| c.id != skip) {
        if let CadItem::Polyline {
            points,
            closed: true,
        } = &c.item
        {
            if let Some((lo, hi)) = box_bounds(points) {
                v.push(Face {
                    vertical: true,
                    at: lo.x,
                    from: lo.y,
                    to: hi.y,
                });
                v.push(Face {
                    vertical: true,
                    at: hi.x,
                    from: lo.y,
                    to: hi.y,
                });
                v.push(Face {
                    vertical: false,
                    at: lo.y,
                    from: lo.x,
                    to: hi.x,
                });
                v.push(Face {
                    vertical: false,
                    at: hi.y,
                    from: lo.x,
                    to: hi.x,
                });
            }
        }
    }
    v
}

/// The offsets from a CAD box to the nearest parallel objects on each side
/// (S-58): the gap to the nearest wall face or CAD box side that faces the
/// side and overlaps it. Typing a gap slides the box.
fn box_offsets(floor: &Floor, id: u64, lo: Point, hi: Point, out: &mut Vec<TempDim>) {
    let faces = faces_near(floor, id);
    let target = ObjectRef::Cad(id);
    let overlap = |f: &Face, a: f64, b: f64| f.from < b - 1e-6 && f.to > a + 1e-6;
    let mid = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
    let mut push = |kind, a: Point, b: Point, value: f64, axis: Point, px: f32| {
        out.push(TempDim {
            kind,
            a,
            b,
            value,
            axis,
            target,
            offset_in: 0.0,
            offset_px: px,
            locked: None,
        });
    };
    // Right: the nearest vertical face at or beyond the right side.
    let right = faces
        .iter()
        .filter(|f| f.vertical && f.at >= hi.x + 1e-6 && overlap(f, lo.y, hi.y))
        .min_by(|a, b| a.at.total_cmp(&b.at));
    if let Some(f) = right {
        push(
            TempDimKind::CadToRight,
            Point::new(hi.x, mid.y),
            Point::new(f.at, mid.y),
            f.at - hi.x,
            Point::new(-1.0, 0.0),
            14.0,
        );
    }
    let left = faces
        .iter()
        .filter(|f| f.vertical && f.at <= lo.x - 1e-6 && overlap(f, lo.y, hi.y))
        .max_by(|a, b| a.at.total_cmp(&b.at));
    if let Some(f) = left {
        push(
            TempDimKind::CadToLeft,
            Point::new(f.at, mid.y),
            Point::new(lo.x, mid.y),
            lo.x - f.at,
            Point::new(1.0, 0.0),
            14.0,
        );
    }
    let above = faces
        .iter()
        .filter(|f| !f.vertical && f.at >= hi.y + 1e-6 && overlap(f, lo.x, hi.x))
        .min_by(|a, b| a.at.total_cmp(&b.at));
    if let Some(f) = above {
        push(
            TempDimKind::CadAbove,
            Point::new(mid.x, hi.y),
            Point::new(mid.x, f.at),
            f.at - hi.y,
            Point::new(0.0, -1.0),
            14.0,
        );
    }
    let below = faces
        .iter()
        .filter(|f| !f.vertical && f.at <= lo.y - 1e-6 && overlap(f, lo.x, hi.x))
        .max_by(|a, b| a.at.total_cmp(&b.at));
    if let Some(f) = below {
        push(
            TempDimKind::CadBelow,
            Point::new(mid.x, f.at),
            Point::new(mid.x, lo.y),
            lo.y - f.at,
            Point::new(0.0, 1.0),
            14.0,
        );
    }
}

/// The corners `(lower left, upper right)` of a closed polyline that is an
/// axis-aligned rectangle (a CAD box), else `None`.
fn box_bounds(points: &[Point]) -> Option<(Point, Point)> {
    if points.len() != 4 {
        return None;
    }
    let tol = 1e-6;
    for i in 0..4 {
        let (p, q) = (points[i], points[(i + 1) % 4]);
        if (p.x - q.x).abs() > tol && (p.y - q.y).abs() > tol {
            return None;
        }
    }
    let lo = Point::new(
        points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
        points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
    );
    let hi = Point::new(
        points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max),
        points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max),
    );
    ((hi.x - lo.x) > tol && (hi.y - lo.y) > tol).then_some((lo, hi))
}

/// The permanent dimension that measures exactly `d`'s segment (a locked
/// temporary dimension, S-63).
pub fn locked_dimension(floor: &Floor, d: &TempDim) -> Option<u64> {
    if !d.kind.lockable() {
        return None;
    }
    let same = |p: Point, q: Point| p.dist(q) < 0.01;
    floor
        .dimensions
        .iter()
        .find(|m| {
            m.kind == plan_core::DimensionKind::Manual
                && ((same(m.start, d.a) && same(m.end, d.b))
                    || (same(m.start, d.b) && same(m.end, d.a)))
        })
        .map(|m| m.id)
}

/// The objects the ends of `d` measure from or to, when the selected object
/// is one of them (an opening's jamb, a cabinet's end): they tie the permanent
/// dimension to that object rather than to the wall it stands in.
fn lock_hints(d: &TempDim) -> [Option<plan_core::dim_assoc::DimHint>; 2] {
    use plan_core::dim_assoc::{AnchorTarget, DimHint};
    let hint = |p: Point| match d.target {
        ObjectRef::Opening(id) => Some(DimHint {
            target: AnchorTarget::Opening,
            id,
            point: p,
        }),
        ObjectRef::Cabinet(id) if id != 0 => Some(DimHint {
            target: AnchorTarget::Cabinet,
            id,
            point: p,
        }),
        _ => None,
    };
    match d.kind {
        TempDimKind::OpeningWidth | TempDimKind::CabinetWidth => [hint(d.a), hint(d.b)],
        TempDimKind::OpeningToStart => [None, hint(d.b)],
        TempDimKind::OpeningToEnd => [hint(d.a), None],
        _ => [None, None],
    }
}

/// Locks temporary dimension `index` into a permanent manual dimension, or
/// takes the locked dimension away again (S-63). One undo step; returns its
/// label.
pub fn toggle_lock(cx: &mut EditorContext, index: usize) -> Result<&'static str, String> {
    let Some(d) = cx.temp.dims.get(index).copied() else {
        return Err("That dimension is gone".into());
    };
    if !d.kind.lockable() {
        return Err("Only a measured distance can be locked".into());
    }
    let fl = cx.floor;
    if cx.layers().is_locked("Dimensions, Manual") {
        return Err("The layer \"Dimensions, Manual\" is locked".into());
    }
    if let Some(id) = locked_dimension(cx.floor(), &d) {
        cx.begin_change("Unlock Dimension");
        cx.project.remove_dimension(fl, id);
        cx.mark_dirty();
        return Ok("Unlock Dimension");
    }
    let offset = d.offset_in + f64::from(d.offset_px) / cx.px_per_in.max(1e-6);
    let mut dim = plan_core::Dimension::new(0, plan_core::DimensionKind::Manual, d.a, d.b, offset);
    let style = cx.defaults.dimensions.text_style.clone();
    if !style.is_empty() {
        dim.text_style = Some(style);
    }
    cx.begin_change("Lock Dimension");
    let id = cx.project.add_dimension(fl, dim);
    // Its ends stay tied to the walls and openings they lie on, and the end
    // that is the selected object stays tied to it.
    cx.project.floors[fl].attach_dimension_hinted(id, lock_hints(&d));
    cx.mark_dirty();
    Ok("Lock Dimension")
}

/// A parallel wall's gap: (gap, overlap midpoint, how far the located
/// surface of the selected wall lies out, overlap start, overlap end).
type GapCandidate = (f64, f64, f64, f64, f64);

/// How far apart along the wall further gap rows sit, inches.
const ROW_SPACING: f64 = 18.0;

fn wall_dims(floor: &Floor, id: u64, out: &mut Vec<TempDim>, loc: &TempLocate) {
    let Some(s) = floor.wall(id) else { return };
    let (u, n, len) = (s.direction(), s.normal(), s.length());
    if len < 1e-6 {
        return;
    }
    // Parallel walls on each side: (gap, overlap midpoint, how far the
    // located surface of `s` lies out on that side, overlap start, end).
    let mut found: [Vec<GapCandidate>; 2] = [Vec::new(), Vec::new()];
    // The located span of `s` along its own normal.
    let (s_lo, s_hi) = loc.span(s);
    for o in &floor.walls {
        // "Parallel" has no meaning along an arc: a curved wall shows no gaps.
        if o.id == id
            || o.length() < 1e-6
            || s.is_curved()
            || o.is_curved()
            || !loc.locates_wall(o)
            || u.cross(o.direction()).abs() > 0.01
        {
            continue;
        }
        let off_a = o.start.sub(s.start).dot(n);
        let off_b = o.end.sub(s.start).dot(n);
        if (off_a - off_b).abs() > 0.05 {
            continue;
        }
        let (s0, s1) = (o.start.sub(s.start).dot(u), o.end.sub(s.start).dot(u));
        let lo = s0.min(s1).max(0.0);
        let hi = s0.max(s1).min(len);
        if hi - lo <= 1.0 {
            continue;
        }
        // The other wall's located span in `s`'s normal direction.
        let (o_lo, o_hi) = loc.other_span(o);
        let (o_lo, o_hi) = if o.normal().dot(n) < 0.0 {
            (-o_hi, -o_lo)
        } else {
            (o_lo, o_hi)
        };
        let side = if off_a < 0.0 { -1.0 } else { 1.0 };
        let (out_s, out_o) = if side > 0.0 {
            (s_hi, -o_lo)
        } else {
            (-s_lo, o_hi)
        };
        let gap = off_a.abs() - out_s - out_o;
        if gap <= 0.01 {
            continue;
        }
        found[usize::from(off_a < 0.0)].push((gap, (lo + hi) * 0.5, out_s, lo, hi));
    }
    for (i, side) in [1.0_f64, -1.0].into_iter().enumerate() {
        // The nearest wall always shows; up to `row_limit - 1` further rows
        // follow when they lie within Reach of the selected wall.
        found[i].sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut rows: Vec<(f64, f64, f64)> = Vec::new();
        for (k, &(gap, mid, out_s, lo, hi)) in found[i].iter().enumerate() {
            if rows.len() >= loc.row_limit || (k > 0 && gap > loc.reach) {
                break;
            }
            // Two rows never share a gap value (a row of the same gap is one
            // line) and further rows sit apart along the wall.
            if rows.iter().any(|r| (r.0 - gap).abs() < 0.01) {
                continue;
            }
            let mut at = mid;
            let r = rows.len() as f64;
            if r > 0.0 {
                at = (mid + r * ROW_SPACING).min(hi).max(lo);
            }
            rows.push((gap, at, out_s));
        }
        for (gap, mid, out_s) in rows {
            let a = s.start + u * mid + n * (side * out_s);
            out.push(TempDim {
                kind: TempDimKind::WallGap,
                a,
                b: a + n * (side * gap),
                value: gap,
                axis: n * -side,
                target: ObjectRef::Wall(id),
                offset_in: 0.0,
                offset_px: 0.0,
                locked: None,
            });
        }
    }
    out.push(TempDim {
        kind: TempDimKind::WallLength,
        a: s.start,
        b: s.end,
        value: len,
        axis: u,
        target: ObjectRef::Wall(id),
        offset_in: -s.thickness * 0.5,
        offset_px: -14.0,
        locked: None,
    });
    // A curved wall reads its radius and arc length at the middle of the arc,
    // on the side it bulges toward; the length above is its chord (W-64, W-74).
    if let Some(r) = s.arc_readout() {
        let (apex, tangent) = s.frame_at(s.path_length() * 0.5);
        for (kind, value, extra) in [
            (TempDimKind::WallRadius, r.radius, 16.0),
            (TempDimKind::WallArcLength, r.arc_length, 34.0),
        ] {
            out.push(TempDim {
                kind,
                a: apex,
                b: apex + tangent,
                value,
                axis: n,
                target: ObjectRef::Wall(id),
                offset_in: s.thickness * 0.5,
                offset_px: extra,
                locked: None,
            });
        }
    }
    // The angle sits a quarter of the way along the wall, on its left, clear
    // of the end handles (a short wall shows none).
    if len >= ANGLE_MIN_WALL {
        let at = s.start + u * (len * 0.25);
        out.push(TempDim {
            kind: TempDimKind::WallAngle,
            a: at,
            b: at + u,
            value: super::typed_input::angle_deg(s.start, s.end),
            axis: n,
            target: ObjectRef::Wall(id),
            offset_in: s.thickness * 0.5,
            offset_px: 16.0,
            locked: None,
        });
    }
    // The thickness across the wall, three quarters of the way along (a short
    // wall or an arc shows none, W-28).
    if len >= ANGLE_MIN_WALL && !s.is_curved() {
        let at = s.start + u * (len * 0.75);
        let a = at - n * (s.thickness * 0.5);
        out.push(TempDim {
            kind: TempDimKind::WallThickness,
            a,
            b: a + n * s.thickness,
            value: s.thickness,
            axis: n,
            target: ObjectRef::Wall(id),
            offset_in: 0.0,
            offset_px: 14.0,
            locked: None,
        });
    }
}

fn opening_dims(floor: &Floor, id: u64, out: &mut Vec<TempDim>, loc: &TempLocate) {
    let Some(o) = floor.openings.iter().find(|o| o.id == id) else {
        return;
    };
    let Some(w) = floor.wall(o.wall_id) else {
        return;
    };
    out.extend(opening_temp_dims(floor, w, o, ObjectRef::Opening(id), loc));
}

/// How far an opening's dimension lines sit from the wall, in pixels. Far
/// enough that a value never lies on a jamb or move handle (a click there must
/// grab the handle, not the value).
const OPENING_DIM_PX: f32 = 30.0;

/// The temporary dimensions of an opening, measured along its wall (DW-11):
/// its width, then from each jamb to the nearest neighbouring opening or the
/// wall end. A mulled window measures its whole unit (and adds the unit's
/// overall width). Also used for the placement ghost.
pub fn opening_temp_dims(
    floor: &Floor,
    w: &Wall,
    o: &Opening,
    target: ObjectRef,
    loc: &TempLocate,
) -> Vec<TempDim> {
    // Along the wall is the arc length on a curved wall (DW-88): the width
    // reads the arc, the line stands at the opening's tangent.
    let len = w.path_length();
    let u = w.tangent_along(o.center_offset);
    let mut out = Vec::new();
    // The unit this opening belongs to: itself, or every member of its group.
    let members: Vec<&Opening> = match o.mull_group {
        Some(g) => floor
            .openings_on(w.id)
            .filter(|m| m.mull_group == Some(g))
            .collect(),
        None => Vec::new(),
    };
    let (lo, hi) = members
        .iter()
        .fold((o.start_offset(), o.end_offset()), |(lo, hi), m| {
            (lo.min(m.start_offset()), hi.max(m.end_offset()))
        });
    let in_unit = |m: &Opening| m.id == o.id || members.iter().any(|x| x.id == m.id);
    // Nearest neighbours outside the unit (and their centers, for the
    // distances that run to centers).
    let mut left = 0.0_f64;
    let mut right = len;
    let (mut left_center, mut right_center) = (0.0_f64, len);
    for m in floor.openings_on(w.id).filter(|m| !in_unit(m)) {
        if m.end_offset() <= lo + 1e-9 {
            if m.end_offset() >= left {
                left_center = m.center_offset;
            }
            left = left.max(m.end_offset());
        } else if m.start_offset() >= hi - 1e-9 {
            if m.start_offset() <= right {
                right_center = m.center_offset;
            }
            right = right.min(m.start_offset());
        }
    }
    // Locate: centers measure from this opening's center to the neighbour's
    // center (or the wall end); a mulled unit keeps its sides.
    let by_center = loc.centers() && members.is_empty();
    let (from_lo, from_hi, left, right) = if by_center {
        (o.center_offset, o.center_offset, left_center, right_center)
    } else {
        (lo, hi, left, right)
    };
    out.push(TempDim {
        kind: TempDimKind::OpeningWidth,
        a: w.point_along(o.start_offset()),
        b: w.point_along(o.end_offset()),
        value: o.width,
        axis: u,
        target,
        offset_in: -w.thickness * 0.5,
        offset_px: -OPENING_DIM_PX,
        locked: None,
    });
    if !members.is_empty() {
        out.push(TempDim {
            kind: TempDimKind::OpeningUnitWidth,
            a: w.point_along(lo),
            b: w.point_along(hi),
            value: hi - lo,
            axis: u,
            target,
            offset_in: -w.thickness * 0.5,
            offset_px: -OPENING_DIM_PX * 1.6,
            locked: None,
        });
    }
    out.push(TempDim {
        kind: TempDimKind::OpeningToStart,
        a: w.point_along(left),
        b: w.point_along(from_lo),
        value: from_lo - left,
        axis: u,
        target,
        offset_in: w.thickness * 0.5,
        offset_px: OPENING_DIM_PX,
        locked: None,
    });
    out.push(TempDim {
        kind: TempDimKind::OpeningToEnd,
        a: w.point_along(from_hi),
        b: w.point_along(right),
        value: right - from_hi,
        axis: u * -1.0,
        target,
        offset_in: w.thickness * 0.5,
        offset_px: OPENING_DIM_PX,
        locked: None,
    });
    out
}

/// How far a cabinet's dimension lines sit past its front, pixels: beyond
/// the rotate handle (24 px) so a click on a value never grabs a handle.
const CABINET_DIM_PX: f32 = 46.0;
/// The dimensions to openings stack outside the ones to walls and cabinets.
const CABINET_OPENING_DIM_PX: f32 = 68.0;

/// The temporary dimensions of a cabinet (a placed one, or the ghost while
/// it is placed or dragged), measured along its width: its width, then from
/// each end to the nearest wall or cabinet (across any opening of the wall
/// behind it) and to the near jamb of an opening in that wall. `target`
/// names the object the values edit; `Cabinet(0)` for a ghost.
pub fn cabinet_temp_dims(
    floor: &Floor,
    cab: &Cabinet,
    target: ObjectRef,
    _loc: &TempLocate,
) -> Vec<TempDim> {
    if cab.kind.is_custom() || cab.width <= 0.0 || cab.depth <= 0.0 {
        return Vec::new();
    }
    let u = Point::new(cab.angle.cos(), cab.angle.sin());
    let v = u.perp();
    let at = |s: f64, t: f64| cab.position + u * s + v * t;
    let mut out = vec![TempDim {
        kind: TempDimKind::CabinetWidth,
        a: at(0.0, cab.depth),
        b: at(cab.width, cab.depth),
        value: cab.width,
        axis: u,
        target,
        offset_in: 0.0,
        offset_px: CABINET_DIM_PX,
        locked: None,
    }];
    // Walls and the cabinets at the same height are what stops a cabinet.
    let mut obstacles: Vec<Vec<Point>> = floor
        .walls
        .iter()
        .filter(|w| !w.flags.invisible && w.length() > 1e-9)
        .map(|w| plan_cabinets::wall_polygon(w.start, w.end, w.thickness))
        .collect();
    let (b0, b1) = (cab.elevation, cab.elevation + cab.height);
    obstacles.extend(
        super::placed::load_cabinets(floor)
            .iter()
            .filter(|o| {
                o.id != cab.id
                    && !o.kind.is_custom()
                    && b0.max(o.elevation) < b1.min(o.elevation + o.height) - 0.5
            })
            .map(Cabinet::footprint),
    );
    // Each end looks outward from its own edge, so a ghost that overlaps a
    // neighbour still measures to the things on its other sides.
    let span = |s0: f64| {
        plan_cabinets::free_span(&obstacles, cab.position, u, v, (0.05, cab.depth - 0.05), s0)
            .unwrap_or((None, None))
    };
    let left = span(0.0).0;
    let right = span(cab.width).1;
    let left_gap = left.map(|l| -l).filter(|g| *g > 0.005);
    let right_gap = right.map(|r| r - cab.width).filter(|g| *g > 0.005);
    // Jambs of the openings in the wall the cabinet's back stands on.
    let (mut open_left, mut open_right) = (None::<f64>, None::<f64>);
    for w in floor.walls.iter().filter(|w| w.length() > 1e-9) {
        let along = w.direction();
        if along.dot(v).abs() > 0.02 {
            continue;
        }
        let t_mid = w.start.sub(cab.position).dot(v);
        if (t_mid.abs() - w.thickness / 2.0).abs() > 1.5 {
            continue;
        }
        for o in floor.openings.iter().filter(|o| o.wall_id == w.id) {
            let sa = w.point_along(o.start_offset()).sub(cab.position).dot(u);
            let sb = w.point_along(o.end_offset()).sub(cab.position).dot(u);
            let (lo, hi) = (sa.min(sb), sa.max(sb));
            if hi <= 1e-6 {
                let g = -hi;
                open_left = Some(open_left.map_or(g, |m| m.min(g)));
            } else if lo >= cab.width - 1e-6 {
                let g = lo - cab.width;
                open_right = Some(open_right.map_or(g, |m| m.min(g)));
            }
        }
    }
    let mut push = |kind, from: f64, to: f64, axis: Point, px: f32| {
        // `from` and `to` are width coordinates; the line runs from the
        // smaller to the larger along `u`.
        let (s0, s1) = (from.min(to), from.max(to));
        out.push(TempDim {
            kind,
            a: at(s0, cab.depth),
            b: at(s1, cab.depth),
            value: s1 - s0,
            axis,
            target,
            offset_in: 0.0,
            offset_px: px,
            locked: None,
        });
    };
    if let Some(g) = left_gap {
        push(TempDimKind::CabinetToLeft, -g, 0.0, u, CABINET_DIM_PX);
    }
    if let Some(g) = right_gap {
        push(
            TempDimKind::CabinetToRight,
            cab.width,
            cab.width + g,
            u * -1.0,
            CABINET_DIM_PX,
        );
    }
    // An opening nearer than the wall or cabinet that stops the cabinet gets
    // its own line outside the other.
    if let Some(g) = open_left.filter(|g| *g > 0.005 && left_gap.is_none_or(|l| *g < l - 0.005)) {
        push(
            TempDimKind::CabinetToOpeningLeft,
            -g,
            0.0,
            u,
            CABINET_OPENING_DIM_PX,
        );
    }
    if let Some(g) = open_right.filter(|g| *g > 0.005 && right_gap.is_none_or(|r| *g < r - 0.005)) {
        push(
            TempDimKind::CabinetToOpeningRight,
            cab.width,
            cab.width + g,
            u * -1.0,
            CABINET_OPENING_DIM_PX,
        );
    }
    out
}

/// Applies `value` (inches) to `dim`: moves the selected object so the
/// dimension reads `value`. One undo step. Returns the undo label.
pub fn apply(cx: &mut EditorContext, dim: &TempDim, value: f64) -> Result<&'static str, String> {
    if value < 0.0 && !dim.kind.is_angle() {
        return Err("A dimension cannot be negative".into());
    }
    let fl = cx.floor;
    let change = value - dim.value;
    match (dim.kind, dim.target) {
        (TempDimKind::WallLength, ObjectRef::Wall(id)) => {
            let Some(w) = cx.floor().wall(id).cloned() else {
                return Err("Wall not found".into());
            };
            if value < 1.0 {
                return Err("A wall must be at least 1\" long".into());
            }
            // A length too short for the wall's openings is held at the
            // shortest that still hosts them, with a warning (W-85).
            let value = super::wall_edit::clamp_length(cx, id, value);
            cx.begin_change("Change Wall Length");
            let to = w.start + w.direction() * value;
            ops::move_wall_end_joined(&mut cx.project, fl, id, WallEnd::End, to);
            cx.mark_dirty();
            Ok("Change Wall Length")
        }
        (TempDimKind::WallThickness, ObjectRef::Wall(id)) => {
            let Some(w) = cx.floor().wall(id).cloned() else {
                return Err("Wall not found".into());
            };
            let least = super::wall_edit::min_thickness(cx, &w);
            if value < least - 1e-9 {
                return Err(format!(
                    "Thickness cannot be less than {} (the layers of the wall type)",
                    cx.fmt_dim(least)
                ));
            }
            cx.begin_change("Change Wall Thickness");
            if cx.project.set_wall_thickness_about(fl, id, value) {
                cx.mark_dirty();
                Ok("Change Wall Thickness")
            } else {
                cx.cancel_change();
                Err("A wall must be at least 1/8\" thick".into())
            }
        }
        (TempDimKind::WallAngle, ObjectRef::Wall(id)) => {
            let Some(w) = cx.floor().wall(id).cloned() else {
                return Err("Wall not found".into());
            };
            cx.begin_change("Change Wall Angle");
            let to = super::typed_input::polar(w.start, w.length(), value.rem_euclid(360.0));
            ops::move_wall_end_joined(&mut cx.project, fl, id, WallEnd::End, to);
            cx.mark_dirty();
            Ok("Change Wall Angle")
        }
        (TempDimKind::WallGap, ObjectRef::Wall(id)) => {
            let Some(w) = cx.floor().wall(id).cloned() else {
                return Err("Wall not found".into());
            };
            cx.begin_change("Move Wall");
            let s = change * dim.axis.dot(w.normal());
            ops::move_wall_perpendicular(&mut cx.project, fl, id, s);
            cx.mark_dirty();
            Ok("Move Wall")
        }
        (TempDimKind::OpeningToStart | TempDimKind::OpeningToEnd, ObjectRef::Opening(id)) => {
            let Some(o) = cx.floor().openings.iter().find(|o| o.id == id).cloned() else {
                return Err("Opening not found".into());
            };
            let sign = if dim.kind == TempDimKind::OpeningToStart {
                1.0
            } else {
                -1.0
            };
            let center = o.center_offset + change * sign;
            cx.begin_change("Move Opening");
            if cx.project.slide_opening(fl, id, center) {
                cx.mark_dirty();
                Ok("Move Opening")
            } else {
                cx.cancel_change();
                Err("The opening does not fit there".into())
            }
        }
        (TempDimKind::OpeningWidth, ObjectRef::Opening(id)) => {
            cx.begin_change("Resize Opening");
            if cx.project.set_opening_width(fl, id, value) {
                cx.mark_dirty();
                Ok("Resize Opening")
            } else {
                cx.cancel_change();
                Err("The opening does not fit that width".into())
            }
        }
        (TempDimKind::OpeningUnitWidth, _) => {
            Err("Resize the windows of the unit one by one".into())
        }
        (
            TempDimKind::CabinetWidth
            | TempDimKind::CabinetToLeft
            | TempDimKind::CabinetToRight
            | TempDimKind::CabinetToOpeningLeft
            | TempDimKind::CabinetToOpeningRight,
            ObjectRef::Cabinet(id),
        ) => {
            let Some(mut c) = super::placed::cabinet_by_id(cx.floor(), id) else {
                return Err("Cabinet not found".into());
            };
            let label = if dim.kind == TempDimKind::CabinetWidth {
                if value < 3.0 {
                    return Err("A cabinet must be at least 3\" wide".into());
                }
                c.width = value;
                "Resize Cabinet"
            } else {
                c.position = c.position + dim.axis * change;
                "Move Cabinet"
            };
            cx.begin_change(label);
            if super::placed::replace_cabinet(&mut cx.project, fl, &c) {
                super::placed::rejoin_if_enabled(cx);
                cx.mark_dirty();
                Ok(label)
            } else {
                cx.cancel_change();
                Err("The plan's cabinets could not be read".into())
            }
        }
        (
            TempDimKind::CadToRight
            | TempDimKind::CadToLeft
            | TempDimKind::CadAbove
            | TempDimKind::CadBelow,
            ObjectRef::Cad(id),
        ) => {
            // Typing a gap slides the box along the axis that grows it.
            if value < 0.0 {
                return Err("A gap cannot be negative".into());
            }
            if !cx.check_unlocked(ObjectRef::Cad(id)) {
                return Err("That object is on a locked layer".into());
            }
            cx.begin_change("Move Box");
            if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                ops::translate_cad(&mut c.item, dim.axis * change);
            }
            cx.mark_dirty();
            Ok("Move Box")
        }
        (
            TempDimKind::CadLength
            | TempDimKind::CadAngle
            | TempDimKind::CadWidth
            | TempDimKind::CadHeight
            | TempDimKind::CadRadius
            | TempDimKind::CadDiameter,
            ObjectRef::Cad(id),
        ) => apply_cad(cx, id, dim.kind, value),
        _ => Err("This dimension cannot be edited".into()),
    }
}

/// Resizes CAD object `id` so the dimension `kind` reads `value` (S-58).
fn apply_cad(
    cx: &mut EditorContext,
    id: u64,
    kind: TempDimKind,
    value: f64,
) -> Result<&'static str, String> {
    if kind != TempDimKind::CadAngle && value < 0.5 {
        return Err("A CAD object must be at least 1/2\" across".into());
    }
    let fl = cx.floor;
    let Some(item) = cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.item.clone())
    else {
        return Err("Object not found".into());
    };
    let (label, new) = match (kind, item) {
        (TempDimKind::CadLength, CadItem::Line { a, b }) => {
            let u = b.sub(a).normalized();
            (
                "Change Line Length",
                CadItem::Line {
                    a,
                    b: a.add(u.scale(value)),
                },
            )
        }
        (TempDimKind::CadAngle, CadItem::Line { a, b }) => (
            "Change Line Angle",
            CadItem::Line {
                a,
                b: super::typed_input::polar(a, a.dist(b), value.rem_euclid(360.0)),
            },
        ),
        (TempDimKind::CadRadius, CadItem::Circle { center, .. }) => (
            "Change Radius",
            CadItem::Circle {
                center,
                radius: value,
            },
        ),
        (TempDimKind::CadDiameter, CadItem::Circle { center, .. }) => (
            "Change Radius",
            CadItem::Circle {
                center,
                radius: value * 0.5,
            },
        ),
        (
            TempDimKind::CadRadius,
            CadItem::Arc {
                center,
                start_angle,
                end_angle,
                ..
            },
        ) => (
            "Change Radius",
            CadItem::Arc {
                center,
                radius: value,
                start_angle,
                end_angle,
            },
        ),
        (TempDimKind::CadWidth | TempDimKind::CadHeight, CadItem::Polyline { points, closed }) => {
            let Some((lo, hi)) = box_bounds(&points) else {
                return Err("This dimension cannot be edited".into());
            };
            let (sx, sy) = if kind == TempDimKind::CadWidth {
                (value / (hi.x - lo.x), 1.0)
            } else {
                (1.0, value / (hi.y - lo.y))
            };
            let points = points
                .iter()
                .map(|p| Point::new(lo.x + (p.x - lo.x) * sx, lo.y + (p.y - lo.y) * sy))
                .collect();
            ("Resize Box", CadItem::Polyline { points, closed })
        }
        _ => return Err("This dimension cannot be edited".into()),
    };
    if !cx.check_unlocked(ObjectRef::Cad(id)) {
        return Err("That object is on a locked layer".into());
    }
    cx.begin_change(label);
    if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
        c.item = new;
    }
    cx.mark_dirty();
    Ok(label)
}

/// Commits the field being typed. Returns the undo label on success.
pub fn commit_edit(cx: &mut EditorContext) -> Result<&'static str, String> {
    let Some(field) = cx.temp.editing.clone() else {
        return Err("Nothing is being edited".into());
    };
    let Some(dim) = cx.temp.dims.get(field.index).copied() else {
        cx.temp.cancel();
        return Err("That dimension is gone".into());
    };
    let parsed = if dim.kind.is_angle() {
        field
            .text
            .trim()
            .trim_end_matches('\u{b0}')
            .parse::<f64>()
            .ok()
    } else {
        parse_ft_in(&field.text)
    };
    let Some(value) = parsed else {
        return Err(format!("\"{}\" is not a length", field.text));
    };
    cx.temp.editing = None;
    apply(cx, &dim, value)
}

/// The length typed while a tool draws or drags (inches), when there is one.
/// Drags that go through temporary dimensions (wall ends, openings, cabinets)
/// use it in place of the pointer's distance; the shell hands typed
/// characters to the armed [`super::typed_input::TypedInput`].
pub fn typed_value(cx: &EditorContext) -> Option<f64> {
    cx.typed_input.length()
}

/// The angle typed while a tool draws or drags (degrees counter-clockwise
/// from east), when there is one.
pub fn typed_angle(cx: &EditorContext) -> Option<f64> {
    cx.typed_input.angle()
}

/// Where a drag from `fixed` ends when the typed length and angle replace the
/// pointer's: the typed length (else the distance to `toward`) along the typed
/// angle (else the direction of `toward`). `None` when nothing was typed.
pub fn typed_point(cx: &EditorContext, fixed: Point, toward: Point) -> Option<Point> {
    let (len, ang) = (typed_value(cx), typed_angle(cx));
    if len.is_none() && ang.is_none() {
        return None;
    }
    let len = len.unwrap_or_else(|| fixed.dist(toward));
    let ang = ang.unwrap_or_else(|| super::typed_input::angle_deg(fixed, toward));
    Some(super::typed_input::polar(fixed, len, ang))
}

/// Draws the dimensions (and the edit field) over the plan.
pub fn draw(
    dims: &TempDims,
    painter: &egui::Painter,
    cam: &Camera,
    pal: &Palette,
    fmt: &DimFormat,
) {
    let scale = cam.px_per_in;
    let line = Stroke::new(1.0_f32, pal.dimension_text);
    let ext = Stroke::new(0.7_f32, pal.dimension_text.gamma_multiply(0.7));
    for (i, d) in dims.dims.iter().enumerate() {
        let (p, q) = d.line(scale);
        let (sa, sb) = (cam.world_to_screen(p), cam.world_to_screen(q));
        // The angle, radius and arc length are labels only.
        if !d.kind.label_only() {
            painter.line_segment([cam.world_to_screen(d.a), sa], ext);
            painter.line_segment([cam.world_to_screen(d.b), sb], ext);
            painter.line_segment([sa, sb], line);
            let dir = (sb - sa).normalized();
            let tick = Vec2::new(-dir.y, dir.x) * 4.0;
            for s in [sa, sb] {
                painter.line_segment([s - tick, s + tick], line);
            }
        }
        let editing = dims.editing.as_ref().filter(|e| e.index == i);
        let text = match (editing, d.kind) {
            (Some(e), _) => format!("{}|", e.text),
            (None, TempDimKind::WallAngle | TempDimKind::CadAngle) => {
                format!("{:.1}\u{b0}", d.value)
            }
            (None, TempDimKind::WallRadius | TempDimKind::CadRadius) => {
                format!("R {}", fmt.fmt_len(d.value))
            }
            (None, TempDimKind::WallArcLength) => format!("Arc {}", fmt.fmt_len(d.value)),
            (None, _) => fmt.fmt_len(d.value),
        };
        let mid = cam.world_to_screen(d.label_pos(scale));
        let galley = painter.layout_no_wrap(text, FontId::proportional(12.0), pal.dimension_text);
        let rect = Rect::from_center_size(mid, galley.size() + Vec2::new(8.0, 4.0));
        let fill = if editing.is_some() {
            pal.background
        } else {
            pal.background.gamma_multiply(0.85)
        };
        painter.add(Shape::rect_filled(rect, 2.0, fill));
        if editing.is_some() {
            painter.rect_stroke(rect, 2.0, line, egui::StrokeKind::Inside);
        }
        painter.galley(
            rect.center() - galley.size() * 0.5,
            galley,
            pal.dimension_text,
        );
        if d.kind.lockable() {
            if let Some(at) = dims.lock_pos(i, scale) {
                draw_padlock(painter, cam.world_to_screen(at), dims.is_locked(i), pal);
            }
        }
    }
}

/// The lock glyph beside a value (S-63): closed and filled when the value is
/// locked into a permanent dimension, open outline otherwise.
fn draw_padlock(painter: &egui::Painter, c: egui::Pos2, locked: bool, pal: &Palette) {
    let color = if locked {
        pal.selection
    } else {
        pal.dimension_text.gamma_multiply(0.8)
    };
    let stroke = Stroke::new(1.3_f32, color);
    let body = Rect::from_center_size(c + Vec2::new(0.0, 2.5), Vec2::new(9.0, 6.5));
    if locked {
        painter.rect_filled(body, 1.5, color);
    } else {
        painter.rect_stroke(body, 1.5, stroke, egui::StrokeKind::Inside);
    }
    // The shackle: both legs down when locked, the right leg lifted when not.
    let (l, r) = (body.left() + 2.0, body.right() - 2.0);
    let top = body.top() - 4.0;
    painter.line_segment([egui::pos2(l, body.top()), egui::pos2(l, top)], stroke);
    painter.line_segment([egui::pos2(l, top), egui::pos2(r, top)], stroke);
    let foot = if locked { body.top() } else { top + 1.5 };
    painter.line_segment([egui::pos2(r, top), egui::pos2(r, foot)], stroke);
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{OpeningKind, Project, WallKind};

    fn plan() -> (Project, u64, u64) {
        let mut p = Project::new("t");
        let a = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let mid = p.add_wall(
            0,
            Point::new(0.0, 100.0),
            Point::new(240.0, 100.0),
            4.0,
            100.0,
            WallKind::Interior,
        );
        p.add_wall(
            0,
            Point::new(0.0, 200.0),
            Point::new(240.0, 200.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        (p, a, mid)
    }

    #[test]
    fn wall_shows_gaps_to_both_sides_and_its_length() {
        let (p, _, mid) = plan();
        let mut sel = Selection::default();
        sel.set(ObjectRef::Wall(mid));
        let mut t = TempDims::default();
        t.compute(&p.floors[0], &sel, &TempLocate::default());
        let kinds: Vec<_> = t.dims.iter().map(|d| d.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TempDimKind::WallGap,
                TempDimKind::WallGap,
                TempDimKind::WallLength,
                TempDimKind::WallAngle,
                TempDimKind::WallThickness
            ]
        );
        // 100" centerline spacing minus half of each thickness.
        assert!((t.dims[0].value - 95.0).abs() < 1e-9);
        assert!((t.dims[1].value - 95.0).abs() < 1e-9);
        assert_eq!(t.dims[2].value, 240.0);
        assert_eq!(t.dims[3].value, 0.0);
    }

    /// The gap values on the `up` (+y) or down side of wall `id`.
    fn gaps(p: &Project, id: u64, loc: &TempLocate, up: bool) -> Vec<f64> {
        let mut sel = Selection::default();
        sel.set(ObjectRef::Wall(id));
        let mut t = TempDims::default();
        t.compute(&p.floors[0], &sel, loc);
        t.dims
            .iter()
            .filter(|d| d.kind == TempDimKind::WallGap && (d.b.y > d.a.y) == up)
            .map(|d| d.value)
            .collect()
    }

    #[test]
    fn row_limit_and_reach_decide_how_many_gap_rows_show() {
        let (mut p, _, mid) = plan();
        p.add_wall(
            0,
            Point::new(0.0, 300.0),
            Point::new(240.0, 300.0),
            4.0,
            100.0,
            WallKind::Interior,
        );
        let mut loc = TempLocate::default();
        // One row: the nearest wall on each side.
        assert_eq!(gaps(&p, mid, &loc, true), vec![95.0]);
        // Two rows reach the wall behind it.
        loc.row_limit = 2;
        assert_eq!(gaps(&p, mid, &loc, true), vec![95.0, 196.0]);
        // The second row must lie within Reach; the nearest always shows.
        loc.reach = 100.0;
        assert_eq!(gaps(&p, mid, &loc, true), vec![95.0]);
        loc.reach = 0.0;
        assert_eq!(gaps(&p, mid, &loc, true), vec![95.0]);
        assert_eq!(gaps(&p, mid, &loc, false), vec![95.0]);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn the_wall_options_choose_the_faces_other_walls_are_located_at() {
        let (p, _, mid) = plan();
        let mut loc = TempLocate::default();
        // The wall at y = 200 is exterior and its exterior side is +y:
        // Primary locates the outer face, Secondary the inner one.
        loc.exterior_sides = [true, false];
        assert_eq!(gaps(&p, mid, &loc, true), vec![101.0]);
        loc.exterior_sides = [false, true];
        assert_eq!(gaps(&p, mid, &loc, true), vec![95.0]);
        // Neither: exterior walls are not located at all.
        loc.exterior_sides = [false, false];
        assert!(gaps(&p, mid, &loc, true).is_empty());
        assert!(gaps(&p, mid, &loc, false).is_empty());
    }

    #[test]
    fn a_curved_wall_shows_its_chord_radius_and_arc_length() {
        let (mut p, a, _) = plan();
        // A semicircle over the 240" wall: radius 120", arc pi * 120".
        p.floors[0].wall_mut(a).unwrap().curve =
            plan_core::WallCurve::from_radius(240.0, 120.0, true);
        let mut sel = Selection::default();
        sel.set(ObjectRef::Wall(a));
        let mut t = TempDims::default();
        t.compute(&p.floors[0], &sel, &TempLocate::default());
        let value = |k: TempDimKind| t.dims.iter().find(|d| d.kind == k).map(|d| d.value);
        assert_eq!(value(TempDimKind::WallLength), Some(240.0), "the chord");
        assert!((value(TempDimKind::WallRadius).unwrap() - 120.0).abs() < 1e-9);
        let arc = value(TempDimKind::WallArcLength).unwrap();
        assert!((arc - std::f64::consts::PI * 120.0).abs() < 1e-6);
        // No parallel-wall gaps along an arc, and the labels are read only.
        assert!(t.dims.iter().all(|d| d.kind != TempDimKind::WallGap));
        // The labels sit on the arc's middle (its apex, 120" off the chord).
        let r = t
            .dims
            .iter()
            .find(|d| d.kind == TempDimKind::WallRadius)
            .unwrap();
        assert!(
            (r.a.y - 120.0).abs() < 1e-6 && (r.a.x - 120.0).abs() < 1e-6,
            "{:?}",
            r.a
        );
        // A straight wall shows neither.
        let (q, _, mid) = plan();
        let mut sel = Selection::default();
        sel.set(ObjectRef::Wall(mid));
        t.compute(&q.floors[0], &sel, &TempLocate::default());
        assert!(t.dims.iter().all(|d| d.kind != TempDimKind::WallRadius));
    }

    #[test]
    fn typing_an_angle_turns_the_wall_about_its_start() {
        let (p, _, mid) = plan();
        let mut cx = EditorContext::with_project(p, plan_core::PlanDefaults::chief_x18_daniel());
        cx.selection.set(ObjectRef::Wall(mid));
        cx.refresh();
        let i = cx
            .temp
            .dims
            .iter()
            .position(|d| d.kind == TempDimKind::WallAngle)
            .unwrap();
        cx.temp.begin_edit(i);
        cx.temp.editing.as_mut().unwrap().text = "90".into();
        assert_eq!(commit_edit(&mut cx).unwrap(), "Change Wall Angle");
        let w = cx.floor().wall(mid).unwrap();
        assert_eq!(w.start, Point::new(0.0, 100.0));
        assert_eq!(w.end, Point::new(0.0, 340.0));
        cx.undo();
        assert_eq!(cx.floor().wall(mid).unwrap().end, Point::new(240.0, 100.0));
    }

    #[test]
    fn typed_values_come_from_the_typed_input() {
        let (p, _, _) = plan();
        let mut cx = EditorContext::with_project(p, plan_core::PlanDefaults::chief_x18_daniel());
        assert_eq!(typed_value(&cx), None);
        assert_eq!(typed_point(&cx, Point::ZERO, Point::new(5.0, 5.0)), None);
        cx.typed_input.arm();
        cx.typed_input.handle(None, Some("12'"));
        assert_eq!(typed_value(&cx), Some(144.0));
        // Only the length typed: the pointer still picks the direction.
        let pt = typed_point(&cx, Point::ZERO, Point::new(0.0, 10.0)).unwrap();
        assert_eq!(pt, Point::new(0.0, 144.0));
        cx.typed_input.handle(Some(egui::Key::Tab), None);
        cx.typed_input.handle(None, Some("180"));
        assert_eq!(typed_angle(&cx), Some(180.0));
        let pt = typed_point(&cx, Point::ZERO, Point::new(0.0, 10.0)).unwrap();
        assert_eq!(pt, Point::new(-144.0, 0.0));
    }

    #[test]
    fn opening_shows_distances_to_both_wall_ends() {
        let (mut p, a, _) = plan();
        let o = p.add_opening(0, a, 100.0, OpeningKind::Door).unwrap();
        let mut sel = Selection::default();
        sel.set(ObjectRef::Opening(o));
        let mut t = TempDims::default();
        t.compute(&p.floors[0], &sel, &TempLocate::default());
        // The width, then the distance from each jamb to the wall end.
        assert_eq!(t.dims.len(), 3);
        let value = |k| t.dims.iter().find(|d| d.kind == k).unwrap().value;
        assert_eq!(value(TempDimKind::OpeningWidth), 36.0);
        assert_eq!(value(TempDimKind::OpeningToStart), 82.0);
        assert_eq!(value(TempDimKind::OpeningToEnd), 240.0 - 118.0);
    }

    #[test]
    fn a_neighbouring_opening_replaces_the_wall_end_in_the_gap() {
        let (mut p, a, _) = plan();
        let left = p.add_opening(0, a, 40.0, OpeningKind::Window).unwrap();
        let o = p.add_opening(0, a, 120.0, OpeningKind::Door).unwrap();
        let right = p.add_opening(0, a, 200.0, OpeningKind::Window).unwrap();
        let f = &p.floors[0];
        let get = |id| f.openings.iter().find(|x| x.id == id).unwrap().clone();
        let (l, m, r) = (get(left), get(o), get(right));
        let w = f.wall(a).unwrap();
        let dims = opening_temp_dims(f, w, &m, ObjectRef::Opening(o), &TempLocate::default());
        let value = |k| dims.iter().find(|d| d.kind == k).unwrap().value;
        assert_eq!(
            value(TempDimKind::OpeningToStart),
            m.start_offset() - l.end_offset()
        );
        assert_eq!(
            value(TempDimKind::OpeningToEnd),
            r.start_offset() - m.end_offset()
        );
        // The measured segments run along the wall between the two jambs.
        let start = dims
            .iter()
            .find(|d| d.kind == TempDimKind::OpeningToStart)
            .unwrap();
        assert_eq!(start.a, w.point_along(l.end_offset()));
        assert_eq!(start.b, w.point_along(m.start_offset()));
    }

    #[test]
    fn typing_a_width_resizes_and_a_unit_adds_its_overall_width() {
        let (mut p, a, _) = plan();
        let w1 = p.add_opening(0, a, 80.0, OpeningKind::Window).unwrap();
        let w2 = p.add_opening(0, a, 120.0, OpeningKind::Window).unwrap();
        p.mull_openings(0, &[w1, w2]).unwrap();
        let mut cx = EditorContext::with_project(p, plan_core::PlanDefaults::chief_x18_daniel());
        cx.selection.set(ObjectRef::Opening(w1));
        cx.refresh();
        let unit = cx
            .temp
            .dims
            .iter()
            .find(|d| d.kind == TempDimKind::OpeningUnitWidth)
            .expect("a mulled unit reads its overall width");
        assert_eq!(unit.value, 72.0);
        // Typing into it is refused; the member's own width works.
        let i = cx
            .temp
            .dims
            .iter()
            .position(|d| d.kind == TempDimKind::OpeningUnitWidth)
            .unwrap();
        cx.temp.begin_edit(i);
        cx.temp.editing.as_mut().unwrap().text = "80".into();
        assert!(commit_edit(&mut cx).is_err());
        let j = cx
            .temp
            .dims
            .iter()
            .position(|d| d.kind == TempDimKind::OpeningWidth)
            .unwrap();
        cx.temp.cancel();
        cx.temp.begin_edit(j);
        cx.temp.editing.as_mut().unwrap().text = "30".into();
        assert_eq!(commit_edit(&mut cx).unwrap(), "Resize Opening");
        let o = cx.floor().openings.iter().find(|x| x.id == w1).unwrap();
        assert_eq!(o.width, 30.0);
    }

    #[test]
    fn typing_a_gap_moves_the_wall_and_undoes() {
        let (p, _, mid) = plan();
        let mut cx = EditorContext::with_project(p, plan_core::PlanDefaults::chief_x18_daniel());
        cx.selection.set(ObjectRef::Wall(mid));
        cx.refresh();
        assert!(cx.temp.begin_edit(0));
        cx.temp.editing.as_mut().unwrap().text = "5'".into();
        // The first gap is on the +normal side (the wall runs +x, so +y).
        let label = commit_edit(&mut cx).unwrap();
        assert_eq!(label, "Move Wall");
        let w = cx.floor().wall(mid).unwrap();
        // The gap to the wall above shrinks from 95" to 60": the selected wall
        // moves 35" toward the fixed reference wall.
        assert!((w.start.y - 135.0).abs() < 1e-9, "{}", w.start.y);
        cx.undo();
        assert_eq!(cx.floor().wall(mid).unwrap().start.y, 100.0);
    }

    #[test]
    fn typing_an_opening_distance_slides_it() {
        let (mut p, a, _) = plan();
        let o = p.add_opening(0, a, 100.0, OpeningKind::Door).unwrap();
        let mut cx = EditorContext::with_project(p, plan_core::PlanDefaults::chief_x18_daniel());
        cx.selection.set(ObjectRef::Opening(o));
        cx.refresh();
        let i = cx
            .temp
            .dims
            .iter()
            .position(|d| d.kind == TempDimKind::OpeningToStart)
            .unwrap();
        cx.temp.begin_edit(i);
        cx.temp.editing.as_mut().unwrap().text = "60".into();
        commit_edit(&mut cx).unwrap();
        let op = cx.floor().openings.iter().find(|x| x.id == o).unwrap();
        assert_eq!(op.start_offset(), 60.0);
    }

    #[test]
    fn temporary_dimensions_locate_by_their_own_group() {
        use plan_core::LocateGroup;
        let (mut p, a, mid) = plan();
        let o = p.add_opening(0, a, 100.0, OpeningKind::Door).unwrap();
        let w = p.add_opening(0, a, 200.0, OpeningKind::Window).unwrap();
        let mut cx = EditorContext::with_project(p, plan_core::PlanDefaults::chief_x18_daniel());
        let gaps = |cx: &mut EditorContext| -> Vec<f64> {
            cx.selection.set(ObjectRef::Wall(mid));
            cx.refresh();
            cx.temp
                .dims
                .iter()
                .filter(|d| d.kind == TempDimKind::WallGap)
                .map(|d| d.value)
                .collect()
        };
        let jambs = |cx: &mut EditorContext| -> (f64, f64) {
            cx.selection.set(ObjectRef::Opening(o));
            cx.refresh();
            let v = |k| cx.temp.dims.iter().find(|d| d.kind == k).unwrap().value;
            (v(TempDimKind::OpeningToStart), v(TempDimKind::OpeningToEnd))
        };
        // The manual and automatic group (Centers, Centers) does not touch
        // the temporary dimensions: face to face, jamb to jamb.
        cx.defaults.dimensions.locate_walls = plan_core::WallLocate::Centers;
        cx.defaults
            .dimensions
            .set_opening_locate(OpeningLocate::Centers);
        assert_eq!(gaps(&mut cx), vec![95.0, 95.0]);
        let (start, end) = jambs(&mut cx);
        let door = cx
            .floor()
            .openings
            .iter()
            .find(|x| x.id == o)
            .unwrap()
            .clone();
        let window = cx
            .floor()
            .openings
            .iter()
            .find(|x| x.id == w)
            .unwrap()
            .clone();
        assert!((start - door.start_offset()).abs() < 1e-9);
        assert!((end - (window.start_offset() - door.end_offset())).abs() < 1e-9);
        // The temporary group on centerlines and centers.
        cx.defaults.dimensions.temp_locate = Some(LocateGroup {
            walls: plan_core::WallLocate::Centers,
            openings: OpeningLocate::Centers,
            ..LocateGroup::default()
        });
        assert_eq!(gaps(&mut cx), vec![100.0, 100.0]);
        let (start, end) = jambs(&mut cx);
        assert!((start - door.center_offset).abs() < 1e-9);
        assert!((end - (window.center_offset - door.center_offset)).abs() < 1e-9);
        // Typing a center-to-center distance still slides the opening.
        let i = cx
            .temp
            .dims
            .iter()
            .position(|d| d.kind == TempDimKind::OpeningToStart)
            .unwrap();
        cx.temp.begin_edit(i);
        cx.temp.editing.as_mut().unwrap().text = "60".into();
        commit_edit(&mut cx).unwrap();
        let op = cx.floor().openings.iter().find(|x| x.id == o).unwrap();
        assert_eq!(op.center_offset, 60.0);
        // And the main layer of a wall is its span (the plan's wall types).
        cx.defaults.dimensions.temp_locate = Some(LocateGroup {
            walls: plan_core::WallLocate::MainLayer,
            ..LocateGroup::default()
        });
        let main = gaps(&mut cx);
        assert_eq!(main.len(), 2);
        assert!(main.iter().all(|g| *g > 90.0 && *g <= 100.0), "{main:?}");
    }

    // ----- cabinets -----

    /// A back wall along y = 0 (face at y = 3) closed by two side walls
    /// 54" apart on centre (faces 48" apart), and a 24" base cabinet 12" from
    /// each side wall.
    fn kitchen() -> (EditorContext, u64) {
        let mut p = Project::new("t");
        p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        for x in [0.0, 54.0] {
            p.add_wall(
                0,
                Point::new(x, 0.0),
                Point::new(x, 100.0),
                6.0,
                96.0,
                WallKind::Interior,
            );
        }
        let mut cab = Cabinet::base(24.0);
        cab.position = Point::new(15.0, 3.0);
        let id = super::super::placed::add_cabinet(&mut p, 0, cab).unwrap();
        let mut cx = EditorContext::with_project(p, plan_core::PlanDefaults::chief_x18_daniel());
        cx.selection.set(ObjectRef::Cabinet(id));
        cx.refresh();
        (cx, id)
    }

    fn dim(cx: &EditorContext, kind: TempDimKind) -> Option<f64> {
        cx.temp
            .dims
            .iter()
            .find(|d| d.kind == kind)
            .map(|d| d.value)
    }

    #[test]
    fn a_cabinet_in_a_48_inch_gap_shows_its_width_and_both_gaps() {
        let (cx, _) = kitchen();
        assert_eq!(dim(&cx, TempDimKind::CabinetWidth), Some(24.0));
        assert!((dim(&cx, TempDimKind::CabinetToLeft).unwrap() - 12.0).abs() < 1e-9);
        assert!((dim(&cx, TempDimKind::CabinetToRight).unwrap() - 12.0).abs() < 1e-9);
        // 12 + 24 + 12 is the 48" between the wall faces.
        let total: f64 = [
            TempDimKind::CabinetToLeft,
            TempDimKind::CabinetWidth,
            TempDimKind::CabinetToRight,
        ]
        .iter()
        .map(|k| dim(&cx, *k).unwrap())
        .sum();
        assert!((total - 48.0).abs() < 1e-9);
        // Nothing hangs in the wall behind: no opening dimensions.
        assert!(dim(&cx, TempDimKind::CabinetToOpeningLeft).is_none());
    }

    #[test]
    fn a_neighbour_cabinet_stops_the_gap() {
        let (mut cx, id) = kitchen();
        let mut next = Cabinet::base(12.0);
        next.position = Point::new(15.0 + 24.0 + 4.0, 3.0);
        super::super::placed::add_cabinet(&mut cx.project, 0, next).unwrap();
        cx.selection.set(ObjectRef::Cabinet(id));
        cx.refresh();
        assert!((dim(&cx, TempDimKind::CabinetToRight).unwrap() - 4.0).abs() < 1e-9);
        // A wall cabinet above does not stop a base cabinet.
        let mut wall = Cabinet::wall(12.0);
        wall.position = Point::new(15.0 + 24.0 + 1.0, 3.0);
        super::super::placed::add_cabinet(&mut cx.project, 0, wall).unwrap();
        cx.refresh();
        assert!((dim(&cx, TempDimKind::CabinetToRight).unwrap() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn typing_a_gap_slides_the_cabinet_and_a_width_resizes_it() {
        let (mut cx, id) = kitchen();
        let i = cx
            .temp
            .dims
            .iter()
            .position(|d| d.kind == TempDimKind::CabinetToLeft)
            .unwrap();
        cx.temp.begin_edit(i);
        cx.temp.editing.as_mut().unwrap().text = "6".into();
        assert_eq!(commit_edit(&mut cx), Ok("Move Cabinet"));
        let c = super::super::placed::cabinet_by_id(cx.floor(), id).unwrap();
        assert!((c.position.x - 9.0).abs() < 1e-9 && c.width == 24.0);
        cx.refresh();
        assert!((dim(&cx, TempDimKind::CabinetToRight).unwrap() - 18.0).abs() < 1e-9);
        assert_eq!(cx.undo_label(), Some("Move Cabinet"));
        // The right gap slides the other way.
        let j = cx
            .temp
            .dims
            .iter()
            .position(|d| d.kind == TempDimKind::CabinetToRight)
            .unwrap();
        cx.temp.begin_edit(j);
        cx.temp.editing.as_mut().unwrap().text = "10".into();
        commit_edit(&mut cx).unwrap();
        let c = super::super::placed::cabinet_by_id(cx.floor(), id).unwrap();
        assert!((c.position.x - 17.0).abs() < 1e-9);
        // Typing the width keeps the left end.
        cx.refresh();
        let w = cx
            .temp
            .dims
            .iter()
            .position(|d| d.kind == TempDimKind::CabinetWidth)
            .unwrap();
        cx.temp.begin_edit(w);
        cx.temp.editing.as_mut().unwrap().text = "30".into();
        assert_eq!(commit_edit(&mut cx), Ok("Resize Cabinet"));
        let c = super::super::placed::cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!((c.width, c.position.x), (30.0, 17.0));
        cx.temp.begin_edit(w);
        cx.temp.editing.as_mut().unwrap().text = "1".into();
        assert!(commit_edit(&mut cx).is_err(), "too narrow");
    }

    #[test]
    fn gaps_are_measured_across_an_opening_and_to_its_jamb() {
        let mut p = Project::new("t");
        let back = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        for x in [0.0, 120.0] {
            p.add_wall(
                0,
                Point::new(x, 0.0),
                Point::new(x, 100.0),
                6.0,
                96.0,
                WallKind::Interior,
            );
        }
        let o = p.add_opening(0, back, 30.0, OpeningKind::Window).unwrap();
        let jamb = p.floors[0]
            .openings
            .iter()
            .find(|x| x.id == o)
            .unwrap()
            .end_offset();
        let mut cab = Cabinet::base(24.0);
        cab.position = Point::new(60.0, 3.0);
        let id = super::super::placed::add_cabinet(&mut p, 0, cab).unwrap();
        let mut cx = EditorContext::with_project(p, plan_core::PlanDefaults::chief_x18_daniel());
        cx.selection.set(ObjectRef::Cabinet(id));
        cx.refresh();
        // The wall at the left end is 57" away, the window's jamb nearer.
        assert!((dim(&cx, TempDimKind::CabinetToLeft).unwrap() - 57.0).abs() < 1e-9);
        let to_jamb = dim(&cx, TempDimKind::CabinetToOpeningLeft).unwrap();
        assert!((to_jamb - (60.0 - jamb)).abs() < 1e-9, "{to_jamb} {jamb}");
        assert!(dim(&cx, TempDimKind::CabinetToOpeningRight).is_none());
    }

    #[test]
    fn a_ghost_cabinet_has_dimensions_too() {
        let (cx, _) = kitchen();
        let mut ghost = Cabinet::base(30.0);
        ghost.position = Point::new(10.0, 3.0);
        let dims = cabinet_temp_dims(
            cx.floor(),
            &ghost,
            ObjectRef::Cabinet(0),
            &TempLocate::default(),
        );
        let get = |k| dims.iter().find(|d| d.kind == k).map(|d| d.value);
        assert_eq!(get(TempDimKind::CabinetWidth), Some(30.0));
        assert!((get(TempDimKind::CabinetToLeft).unwrap() - 7.0).abs() < 1e-9);
        // The placed cabinet overlaps the ghost, so it stops nothing; the
        // wall 11" past the ghost's right end does.
        assert!((get(TempDimKind::CabinetToRight).unwrap() - 11.0).abs() < 1e-9);
        let custom = {
            let mut c = Cabinet::new(plan_cabinets::CabinetKind::CustomCountertop, 24.0);
            c.position = Point::new(10.0, 3.0);
            c
        };
        assert!(cabinet_temp_dims(
            cx.floor(),
            &custom,
            ObjectRef::Cabinet(0),
            &TempLocate::default()
        )
        .is_empty());
    }
}
