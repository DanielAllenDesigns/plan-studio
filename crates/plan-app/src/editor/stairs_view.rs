//! Stairs in the plan (CB-22..CB-34 in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`): storage in the
//! floor's opaque `stairs` slot, plan-symbol drawing, hit-testing, edit
//! handles and the stair commands (Auto Stairwell, Flare/Curve, break line,
//! railing placeholder).
//!
//! Stairs are stored as the JSON of a [`plan_stairs::Stair`] plus one extra
//! key, `"x"`, holding the plan-only settings ([`StairExtras`]: label, line
//! and fill style, materials, landing depth ...). `plan-stairs` ignores the
//! extra key, so the 3D builders read the same objects.

use super::handles::{self, Handle, HandleKind};
use super::{Camera, EditorContext, ObjectRef};
use crate::theme::Palette;
use eframe::egui::{self, Color32, CursorIcon, FontId, Pos2, Shape, Stroke};
use plan_core::geometry::{point_in_polygon, polygon_area, polygon_centroid, Point};
use plan_core::{detect_rooms, Floor, Id, Project, Wall, WallKind};
use plan_stairs::{
    footprint, plan_symbol, solve, top_point, Stair, StairParams, StairShape, StairSolution,
    Stroke as PlanStroke, Turn,
};
use serde_json::{json, Value};
use std::f64::consts::FRAC_PI_2;

/// The layer stairs live on.
pub const LAYER: &str = "Stairs";
/// Fraction of the flight length at which the floor above cuts the stair
/// (Chief's default break line, CB-33).
pub const BREAK_AT: f64 = 2.0 / 3.0;
/// Default stair width (CB-23).
pub const DEFAULT_WIDTH: f64 = 36.0;
/// Floor platform added to the ceiling height when no floor is above (the
/// floor-to-floor rise of a stair that has nowhere to arrive yet).
pub const PLATFORM: f64 = 12.125;
/// Default landing size.
pub const DEFAULT_LANDING: f64 = 36.0;
/// Rise of a ramp (the 30" maximum between landings, CB-34).
pub const RAMP_RISE: f64 = 30.0;
/// Smallest width / landing size the handles allow.
pub const MIN_SIZE: f64 = 12.0;
/// Thickness of the invisible stairwell room-divider walls.
pub const STAIRWELL_THICKNESS: f64 = 0.5;

// ----- the tool variants -----

/// The stair tools of the Build > Stairs flyout (CB-22).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StairKind {
    Draw,
    Straight,
    LShaped,
    UShaped,
    CurveLeft,
    CurveRight,
    Landing,
    Ramp,
}

impl StairKind {
    pub const ALL: [StairKind; 8] = [
        StairKind::Draw,
        StairKind::Straight,
        StairKind::LShaped,
        StairKind::UShaped,
        StairKind::CurveLeft,
        StairKind::CurveRight,
        StairKind::Landing,
        StairKind::Ramp,
    ];

    /// Chief's name (the flyout entry).
    pub fn name(self) -> &'static str {
        match self {
            StairKind::Draw => "Draw Stairs",
            StairKind::Straight => "Straight Stairs",
            StairKind::LShaped => "L-Shaped Stair",
            StairKind::UShaped => "U-Shaped Stair",
            StairKind::CurveLeft => "Curve to Left",
            StairKind::CurveRight => "Curve to Right",
            StairKind::Landing => "Landing",
            StairKind::Ramp => "Draw Ramp",
        }
    }

    pub fn from_name(name: &str) -> Option<StairKind> {
        StairKind::ALL.into_iter().find(|k| k.name() == name)
    }

    /// Status-bar hint.
    pub fn hint(self) -> &'static str {
        match self {
            StairKind::Draw => {
                "Draw Stairs: drag from the bottom of the run to the top; click places a default stair"
            }
            StairKind::Landing => "Landing: drag a rectangle; click places a 3' square landing",
            StairKind::Ramp => "Draw Ramp: drag the run; click places a 1:12 ramp",
            StairKind::LShaped | StairKind::UShaped => {
                "Drag the first flight in the direction of travel; Tab flips the turn"
            }
            StairKind::CurveLeft | StairKind::CurveRight => {
                "Drag the direction of travel; click places a default curved (winder) stair"
            }
            StairKind::Straight => "Straight Stairs: click to place, or drag to set direction and run",
        }
    }
}

// ----- the stored object -----

/// Plan-only settings of a stair, kept beside the [`Stair`] (key `"x"`).
#[derive(Clone, Debug, PartialEq)]
pub struct StairExtras {
    /// `Some(depth)` marks a landing (CB-27): `params.width` is its width.
    pub landing_depth: Option<f64>,
    /// Draw the break line where the floor above cuts the stair (CB-22).
    pub break_line: bool,
    /// Show the number of risers beside the arrow (CB-23).
    pub show_risers: bool,
    pub label: String,
    pub show_label: bool,
    pub line_weight: f32,
    pub dashed: bool,
    pub fill: bool,
    pub fill_gray: u8,
    /// The Run handle leaves the tread depth alone.
    pub lock_tread: bool,
    pub lock_riser: bool,
    pub railing_left: bool,
    pub railing_right: bool,
    /// Component / material pairs of the Materials tab.
    pub materials: Vec<(String, String)>,
    /// Ids of the room-divider walls Auto Stairwell added on the floor above.
    pub stairwell_walls: Vec<Id>,
}

impl Default for StairExtras {
    fn default() -> Self {
        let m = |a: &str, b: &str| (a.to_string(), b.to_string());
        Self {
            landing_depth: None,
            break_line: true,
            show_risers: true,
            label: String::new(),
            show_label: false,
            line_weight: 1.0,
            dashed: false,
            fill: false,
            fill_gray: 235,
            lock_tread: false,
            lock_riser: false,
            railing_left: false,
            railing_right: false,
            materials: vec![
                m("Treads", "Oak"),
                m("Risers", "Painted White"),
                m("Stringers", "Pine"),
                m("Handrail", "Oak"),
                m("Balusters", "Painted White"),
            ],
            stairwell_walls: Vec::new(),
        }
    }
}

impl StairExtras {
    pub fn to_json(&self) -> Value {
        json!({
            "landing_depth": self.landing_depth,
            "break_line": self.break_line,
            "show_risers": self.show_risers,
            "label": self.label,
            "show_label": self.show_label,
            "line_weight": self.line_weight,
            "dashed": self.dashed,
            "fill": self.fill,
            "fill_gray": self.fill_gray,
            "lock_tread": self.lock_tread,
            "lock_riser": self.lock_riser,
            "railing_left": self.railing_left,
            "railing_right": self.railing_right,
            "materials": self.materials.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>(),
            "stairwell_walls": self.stairwell_walls,
        })
    }

    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        let b = |k: &str, dv: bool| v.get(k).and_then(Value::as_bool).unwrap_or(dv);
        let f = |k: &str, dv: f64| v.get(k).and_then(Value::as_f64).unwrap_or(dv);
        let s = |k: &str| {
            v.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let materials: Vec<(String, String)> = v
            .get("materials")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|p| {
                        let p = p.as_array()?;
                        Some((
                            p.first()?.as_str()?.to_string(),
                            p.get(1)?.as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .filter(|m: &Vec<(String, String)>| !m.is_empty())
            .unwrap_or(d.materials);
        Self {
            landing_depth: v.get("landing_depth").and_then(Value::as_f64),
            break_line: b("break_line", true),
            show_risers: b("show_risers", true),
            label: s("label"),
            show_label: b("show_label", false),
            line_weight: f("line_weight", 1.0) as f32,
            dashed: b("dashed", false),
            fill: b("fill", false),
            fill_gray: f("fill_gray", 235.0).clamp(0.0, 255.0) as u8,
            lock_tread: b("lock_tread", false),
            lock_riser: b("lock_riser", false),
            railing_left: b("railing_left", false),
            railing_right: b("railing_right", false),
            materials,
            stairwell_walls: v
                .get("stairwell_walls")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default(),
        }
    }
}

/// A stair or landing of a floor with its plan-only settings.
#[derive(Clone, Debug, PartialEq)]
pub struct StairObj {
    pub stair: Stair,
    pub x: StairExtras,
}

impl StairObj {
    pub fn id(&self) -> Id {
        self.stair.id
    }

    pub fn is_landing(&self) -> bool {
        self.x.landing_depth.is_some()
    }

    pub fn is_ramp(&self) -> bool {
        matches!(self.stair.params.shape, StairShape::Ramp { .. })
    }

    /// Unit vector of the first flight's direction of travel.
    pub fn along(&self) -> Point {
        Point::new(self.stair.direction.cos(), self.stair.direction.sin())
    }

    /// Unit vector to the right of the direction of travel.
    pub fn right(&self) -> Point {
        let a = self.along();
        Point::new(a.y, -a.x)
    }

    /// Centre of the bottom riser line: the pivot of the Rotate handle.
    pub fn bottom_center(&self) -> Point {
        self.stair.origin + self.right() * (self.stair.params.width * 0.5)
    }

    pub fn solution(&self) -> StairSolution {
        solve(&self.stair.params)
    }

    /// Plan polygon of the whole object (landing rectangle for a landing).
    pub fn footprint(&self) -> Vec<Point> {
        if let Some(d) = self.x.landing_depth {
            let o = self.stair.origin;
            let (a, r) = (self.along() * d, self.right() * self.stair.params.width);
            return vec![o, o + a, o + a + r, o + r];
        }
        footprint(&self.stair)
    }

    /// Length of the first flight, measured along the direction of travel.
    pub fn first_flight_len(&self) -> f64 {
        if let Some(d) = self.x.landing_depth {
            return d;
        }
        let sol = self.solution();
        if self.is_ramp() {
            return sol.total_run;
        }
        f64::from(first_flight_treads(&self.stair.params, sol.risers)) * sol.tread_depth
    }

    /// Where the Run handle sits: the top end of the first flight.
    pub fn run_handle_pos(&self) -> Point {
        self.stair.origin
            + self.along() * self.first_flight_len()
            + self.right() * (self.stair.params.width * 0.5)
    }
}

/// Regular treads in the first flight of a stepped stair.
pub fn first_flight_treads(p: &StairParams, risers: u32) -> u32 {
    let (pad, asked) = match p.shape {
        StairShape::LShaped {
            treads_before_landing,
        }
        | StairShape::UShaped {
            treads_before_landing,
        } => (2, Some(treads_before_landing)),
        StairShape::Winder { winders } => (winders.max(1) + 1, None),
        StairShape::Straight | StairShape::Ramp { .. } => return risers.saturating_sub(1),
    };
    if risers < pad {
        return risers.saturating_sub(1);
    }
    let regular = risers - pad;
    asked.unwrap_or(regular / 2).min(regular)
}

// ----- storage -----

fn value_id(v: &Value) -> Option<Id> {
    v.get("id").and_then(Value::as_u64)
}

fn parse(v: &Value) -> Option<StairObj> {
    let stair: Stair = serde_json::from_value(v.clone()).ok()?;
    let x = v.get("x").map(StairExtras::from_json).unwrap_or_default();
    Some(StairObj { stair, x })
}

fn to_json(o: &StairObj) -> Value {
    let mut v = serde_json::to_value(&o.stair).unwrap_or(Value::Null);
    if let Some(m) = v.as_object_mut() {
        m.insert("x".into(), o.x.to_json());
    }
    v
}

/// Every stair and landing of the floor (values that do not parse are
/// skipped, and survive untouched in the file).
pub fn load(floor: &Floor) -> Vec<StairObj> {
    floor.stairs.iter().filter_map(parse).collect()
}

pub fn find(floor: &Floor, id: Id) -> Option<StairObj> {
    floor
        .stairs
        .iter()
        .find(|v| value_id(v) == Some(id))
        .and_then(parse)
}

pub fn exists(floor: &Floor, id: Id) -> bool {
    floor.stairs.iter().any(|v| value_id(v) == Some(id))
}

/// A fresh object id (shared with walls and everything else).
pub fn alloc_id(project: &mut Project) -> Id {
    project.alloc_id()
}

/// Adds `obj` to floor `fl` under a new id and returns the id.
pub fn add(project: &mut Project, fl: usize, mut obj: StairObj) -> Id {
    obj.stair.id = alloc_id(project);
    let id = obj.stair.id;
    project.floors[fl].stairs.push(to_json(&obj));
    id
}

/// Edits one object in place. Returns false if it does not exist.
pub fn update(project: &mut Project, fl: usize, id: Id, f: impl FnOnce(&mut StairObj)) -> bool {
    for v in project.floors[fl].stairs.iter_mut() {
        if value_id(v) == Some(id) {
            if let Some(mut o) = parse(v) {
                f(&mut o);
                *v = to_json(&o);
                return true;
            }
        }
    }
    false
}

pub fn remove(project: &mut Project, fl: usize, id: Id) -> bool {
    let n = project.floors[fl].stairs.len();
    project.floors[fl]
        .stairs
        .retain(|v| value_id(v) != Some(id));
    project.floors[fl].stairs.len() < n
}

// ----- creating -----

/// Floor-to-floor rise from floor `fl` (CB-24): the next floor's elevation
/// minus this one's, or the ceiling height plus a 12 1/8" platform when no
/// floor is above.
pub fn floor_rise(project: &Project, fl: usize) -> f64 {
    let f = &project.floors[fl];
    match project.floors.get(fl + 1) {
        Some(above) if above.elevation - f.elevation > 1.0 => above.elevation - f.elevation,
        _ => f.ceiling_height + PLATFORM,
    }
}

/// A new stair (id 0) for the tool `kind`. `a` is the click or drag start,
/// `b` the drag end: the drag direction is the direction of travel and its
/// length the run (CB-23). The number of risers comes from the floor rise.
pub fn build(
    project: &Project,
    fl: usize,
    kind: StairKind,
    turn: Turn,
    a: Point,
    b: Option<Point>,
) -> StairObj {
    let floor = &project.floors[fl];
    let drag = b.filter(|b| a.dist(*b) > 1e-6);
    let mut extras = StairExtras::default();

    if kind == StairKind::Landing {
        let (lo, hi) = match drag {
            Some(b) => (
                Point::new(a.x.min(b.x), a.y.min(b.y)),
                Point::new(a.x.max(b.x), a.y.max(b.y)),
            ),
            None => (a, Point::new(a.x + DEFAULT_LANDING, a.y + DEFAULT_LANDING)),
        };
        let width = (hi.y - lo.y).max(MIN_SIZE);
        let depth = (hi.x - lo.x).max(MIN_SIZE);
        extras.landing_depth = Some(depth);
        extras.break_line = false;
        extras.show_risers = false;
        let params = StairParams {
            total_rise: 0.0,
            width,
            ..StairParams::default()
        };
        // Facing +x the left edge is +y.
        let mut s = Stair::new(0, Point::new(lo.x, lo.y + width), 0.0, params);
        s.floor_elevation = floor.elevation;
        return StairObj {
            stair: s,
            x: extras,
        };
    }

    let rise = floor_rise(project, fl);
    let mut params = StairParams {
        total_rise: rise,
        width: DEFAULT_WIDTH,
        ..StairParams::default()
    };
    let direction = drag.map_or(FRAC_PI_2, |b| b.sub(a).angle());
    let run = drag.map(|b| a.dist(b));

    match kind {
        StairKind::Draw | StairKind::Straight | StairKind::Landing => {
            if let Some(l) = run {
                let treads = solve(&params).treads;
                if treads > 0 {
                    params.tread_depth = (l / f64::from(treads)).max(1.0);
                }
            }
        }
        StairKind::LShaped | StairKind::UShaped => {
            let regular = solve(&params).risers.saturating_sub(2);
            let t1 = run
                .map_or(regular / 2, |l| (l / params.tread_depth).round() as u32)
                .clamp(1, regular.max(1));
            params.shape = if kind == StairKind::LShaped {
                StairShape::LShaped {
                    treads_before_landing: t1,
                }
            } else {
                StairShape::UShaped {
                    treads_before_landing: t1,
                }
            };
            params.turn = turn;
        }
        StairKind::CurveLeft | StairKind::CurveRight => {
            params.shape = StairShape::Winder { winders: 3 };
            params.turn = if kind == StairKind::CurveLeft {
                Turn::Left
            } else {
                Turn::Right
            };
        }
        StairKind::Ramp => {
            params.total_rise = rise.min(RAMP_RISE);
            let slope = run.map_or(12.0, |l| l / params.total_rise.max(1.0));
            params.shape = StairShape::Ramp {
                slope_1_in: slope.max(1.0),
            };
            extras.break_line = false;
            extras.show_risers = false;
        }
    }

    let right = Point::new(direction.sin(), -direction.cos());
    let origin = a - right * (params.width * 0.5);
    let mut s = Stair::new(0, origin, direction, params);
    s.floor_elevation = floor.elevation;
    StairObj {
        stair: s,
        x: extras,
    }
}

// ----- picking -----

fn edge_distance(poly: &[Point], p: Point) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| plan_core::geometry::dist_to_segment(p, poly[i], poly[(i + 1) % n]))
        .fold(f64::INFINITY, f64::min)
}

/// The topmost stair under `p` (inside it, or within `tol` of its outline).
pub fn pick(floor: &Floor, p: Point, tol: f64) -> Option<Id> {
    load(floor)
        .iter()
        .rev()
        .find(|o| {
            let fp = o.footprint();
            point_in_polygon(p, &fp) || edge_distance(&fp, p) <= tol
        })
        .map(StairObj::id)
}

/// Stairs touched by the rectangle `lo..hi` (marquee selection).
pub fn in_rect(floor: &Floor, lo: Point, hi: Point) -> Vec<Id> {
    load(floor)
        .iter()
        .filter(|o| {
            o.footprint()
                .iter()
                .any(|p| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y)
        })
        .map(StairObj::id)
        .collect()
}

// ----- edit handles (CB-28) -----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StairHandleKind {
    Move,
    /// Rotate about the bottom of the stair.
    Rotate,
    /// Resize the run at the top end.
    Run,
    /// Resize the width from the left side.
    WidthLeft,
    /// Resize the width from the right side.
    WidthRight,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct StairHandle {
    pub kind: StairHandleKind,
    pub pos: Point,
    pub cursor: CursorIcon,
}

/// The handles of a selected stair; `px_per_in` keeps the rotate handle a
/// fixed number of pixels behind the bottom.
pub fn handles(obj: &StairObj, px_per_in: f64) -> Vec<StairHandle> {
    let o = obj.stair.origin;
    let (along, right) = (obj.along(), obj.right());
    let w = obj.stair.params.width;
    let mid = obj.first_flight_len() * 0.5;
    let rotate_off = (24.0 / px_per_in.max(1e-6)).max(10.0);
    let mut out = vec![
        StairHandle {
            kind: StairHandleKind::Move,
            pos: polygon_centroid(&obj.footprint()),
            cursor: CursorIcon::Move,
        },
        StairHandle {
            kind: StairHandleKind::Run,
            pos: obj.run_handle_pos(),
            cursor: CursorIcon::ResizeVertical,
        },
        StairHandle {
            kind: StairHandleKind::WidthLeft,
            pos: o + along * mid,
            cursor: CursorIcon::ResizeHorizontal,
        },
        StairHandle {
            kind: StairHandleKind::WidthRight,
            pos: o + along * mid + right * w,
            cursor: CursorIcon::ResizeHorizontal,
        },
    ];
    out.push(StairHandle {
        kind: StairHandleKind::Rotate,
        pos: obj.bottom_center() - along * rotate_off,
        cursor: CursorIcon::Grab,
    });
    out
}

/// The nearest handle within `tol` inches of `p`.
pub fn hit_handle(hs: &[StairHandle], p: Point, tol: f64) -> Option<StairHandle> {
    hs.iter()
        .filter(|h| h.pos.dist(p) <= tol)
        .min_by(|a, b| a.pos.dist(p).total_cmp(&b.pos.dist(p)))
        .copied()
}

/// The handles as editor handles, for drawing with `handles::draw`.
pub fn editor_handles(obj: &StairObj, px_per_in: f64) -> Vec<Handle> {
    handles(obj, px_per_in)
        .into_iter()
        .map(|h| Handle {
            kind: match h.kind {
                StairHandleKind::Move => HandleKind::Move,
                StairHandleKind::Rotate => HandleKind::Rotate,
                StairHandleKind::Run | StairHandleKind::WidthRight => HandleKind::ResizeEnd,
                StairHandleKind::WidthLeft => HandleKind::ResizeStart,
            },
            pos: h.pos,
            cursor: h.cursor,
            target: ObjectRef::Stair(obj.id()),
        })
        .collect()
}

/// The undo label of a handle drag.
pub fn drag_label(kind: StairHandleKind) -> &'static str {
    match kind {
        StairHandleKind::Move => "Move Stairs",
        StairHandleKind::Rotate => "Rotate Stairs",
        StairHandleKind::Run => "Resize Stair Run",
        StairHandleKind::WidthLeft | StairHandleKind::WidthRight => "Resize Stair Width",
    }
}

/// Snaps a direction to a multiple of 15 degrees when within 2.5 of one.
fn snap_direction(rad: f64) -> f64 {
    let step = 15f64.to_radians();
    let n = (rad / step).round();
    if (rad - n * step).abs() < 2.5f64.to_radians() {
        n * step
    } else {
        rad
    }
}

/// The object after dragging handle `kind` from `start` to `to`, relative
/// to the object as it was when the drag began.
pub fn drag_handle(orig: &StairObj, kind: StairHandleKind, start: Point, to: Point) -> StairObj {
    let mut o = orig.clone();
    let (along, right) = (orig.along(), orig.right());
    let w = orig.stair.params.width;
    match kind {
        StairHandleKind::Move => o.stair.origin = orig.stair.origin + (to - start),
        StairHandleKind::Rotate => {
            let pivot = orig.bottom_center();
            let v = pivot - to;
            if v.length() > 1e-6 {
                let dir = snap_direction(v.angle());
                o.stair.direction = dir;
                let r = Point::new(dir.sin(), -dir.cos());
                o.stair.origin = pivot - r * (w * 0.5);
            }
        }
        StairHandleKind::Run => {
            let l = (to - orig.bottom_center()).dot(along).max(MIN_SIZE);
            set_run(&mut o, l);
        }
        StairHandleKind::WidthRight => {
            o.stair.params.width = (to - orig.stair.origin).dot(right).max(MIN_SIZE);
        }
        StairHandleKind::WidthLeft => {
            let off = (to - orig.stair.origin).dot(right).min(w - MIN_SIZE);
            o.stair.origin = orig.stair.origin + right * off;
            o.stair.params.width = w - off;
        }
    }
    o
}

/// Sets the length of the first flight: the tread depth follows (the number
/// of treads is fixed by the floor-to-floor rise), a landing grows, a ramp
/// changes its slope.
pub fn set_run(o: &mut StairObj, l: f64) {
    if o.is_landing() {
        o.x.landing_depth = Some(l.max(MIN_SIZE));
    } else if o.is_ramp() {
        let rise = o.stair.params.total_rise.max(1.0);
        o.stair.params.shape = StairShape::Ramp {
            slope_1_in: (l / rise).max(1.0),
        };
    } else if !o.x.lock_tread {
        let treads = first_flight_treads(&o.stair.params, o.solution().risers);
        if treads > 0 {
            o.stair.params.tread_depth = (l / f64::from(treads)).max(1.0);
        }
    }
}

// ----- commands (the stair Edit toolbar) -----

/// Stair-specific Edit toolbar commands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StairCommand {
    /// CB-29, CB-30.
    AutoStairwell,
    /// Flare/Curve Stairs (CB-26): toggles the stair to winders and back.
    FlareCurve,
    /// Add/Remove Stair Breakline (CB-22).
    ToggleBreakLine,
    /// Make Railing (CB-31): placeholder, marks both sides.
    MakeRailing,
}

impl StairCommand {
    pub const ALL: [StairCommand; 4] = [
        StairCommand::AutoStairwell,
        StairCommand::FlareCurve,
        StairCommand::ToggleBreakLine,
        StairCommand::MakeRailing,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StairCommand::AutoStairwell => "Auto Stairwell",
            StairCommand::FlareCurve => "Flare/Curve Stairs",
            StairCommand::ToggleBreakLine => "Add/Remove Stair Breakline",
            StairCommand::MakeRailing => "Make Railing",
        }
    }

    /// An icon id from `icons.rs`, when one fits.
    pub fn icon(self) -> Option<&'static str> {
        match self {
            StairCommand::FlareCurve => Some("stairs_curved"),
            StairCommand::MakeRailing => Some("railing"),
            _ => Some("stairs"),
        }
    }
}

/// The selected stair (or landing) when exactly one object is selected.
pub fn selected_stair(cx: &EditorContext) -> Option<StairObj> {
    match cx.selection.single()? {
        ObjectRef::Stair(id) => find(cx.floor(), id),
        _ => None,
    }
}

/// The stair commands for the current selection with their enabled flags
/// (empty unless one stair is selected).
pub fn edit_commands(cx: &EditorContext) -> Vec<(StairCommand, bool)> {
    let Some(o) = selected_stair(cx) else {
        return Vec::new();
    };
    let stepped = !o.is_landing() && !o.is_ramp();
    StairCommand::ALL
        .into_iter()
        .map(|c| {
            let on = match c {
                StairCommand::AutoStairwell => {
                    !o.is_landing() && cx.floor + 1 < cx.project.floors.len()
                }
                StairCommand::FlareCurve | StairCommand::ToggleBreakLine => stepped,
                StairCommand::MakeRailing => !o.is_landing(),
            };
            (c, on)
        })
        .collect()
}

/// Runs a stair command on the selected stair. The status bar says what
/// happened; returns true if the model changed.
pub fn run_command(cx: &mut EditorContext, cmd: StairCommand) -> bool {
    let Some(obj) = selected_stair(cx) else {
        cx.status = "Select a stair first".into();
        return false;
    };
    let id = obj.id();
    let fl = cx.floor;
    match cmd {
        StairCommand::AutoStairwell => match auto_stairwell(cx, id) {
            Ok(n) => {
                cx.status =
                    format!("Auto Stairwell: {n} room-divider walls added on the floor above");
                true
            }
            Err(e) => {
                cx.status = e;
                false
            }
        },
        StairCommand::FlareCurve => {
            if obj.is_landing() || obj.is_ramp() {
                cx.status = "Flare/Curve applies to stepped stairs".into();
                return false;
            }
            cx.begin_change("Flare/Curve Stairs");
            let risers = obj.solution().risers;
            update(&mut cx.project, fl, id, |o| {
                o.stair.params.shape = match o.stair.params.shape {
                    StairShape::Winder { .. } => StairShape::LShaped {
                        treads_before_landing: risers.saturating_sub(2) / 2,
                    },
                    _ => StairShape::Winder { winders: 3 },
                };
            });
            cx.mark_dirty();
            cx.status = "Flare/Curve Stairs: toggled winders".into();
            true
        }
        StairCommand::ToggleBreakLine => {
            if obj.is_landing() || obj.is_ramp() {
                cx.status = "Landings and ramps have no break line".into();
                return false;
            }
            cx.begin_change("Stair Breakline");
            update(&mut cx.project, fl, id, |o| {
                o.x.break_line = !o.x.break_line
            });
            cx.mark_dirty();
            true
        }
        StairCommand::MakeRailing => {
            cx.begin_change("Make Railing");
            update(&mut cx.project, fl, id, |o| {
                o.x.railing_left = true;
                o.x.railing_right = true;
            });
            cx.mark_dirty();
            cx.status =
                "Make Railing: railing marked on both sides (railing objects come later)".into();
            true
        }
    }
}

/// Stores a dialog edit (one undo step).
pub fn apply_edit(cx: &mut EditorContext, edited: &StairObj) -> bool {
    let fl = cx.floor;
    if !exists(&cx.project.floors[fl], edited.id()) {
        return false;
    }
    cx.begin_change("Stair Specification");
    let e = edited.clone();
    update(&mut cx.project, fl, edited.id(), |o| *o = e);
    cx.mark_dirty();
    true
}

/// Deletes the selected stairs and the stairwell walls they created
/// (one undo step). Returns how many stairs were removed.
pub fn delete_selected(cx: &mut EditorContext) -> usize {
    let ids: Vec<Id> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Stair(id) => Some(*id),
            _ => None,
        })
        .collect();
    if ids.is_empty() {
        return 0;
    }
    cx.begin_change("Delete Stairs");
    let fl = cx.floor;
    let mut n = 0;
    for id in ids {
        if let Some(o) = find(cx.floor(), id) {
            if let Some(above) = cx.project.floors.get_mut(fl + 1) {
                above.walls.retain(|w| !o.x.stairwell_walls.contains(&w.id));
            }
        }
        n += usize::from(remove(&mut cx.project, fl, id));
    }
    cx.selection
        .items
        .retain(|o| !matches!(o, ObjectRef::Stair(_)));
    cx.mark_dirty();
    n
}

/// Does the top of the stair land inside a room of the floor above? Then
/// Chief offers the Auto Stairwell (CB-29).
pub fn lands_in_room_above(project: &Project, fl: usize, obj: &StairObj) -> bool {
    let Some(above) = project.floors.get(fl + 1) else {
        return false;
    };
    if obj.is_landing() {
        return false;
    }
    let (top, _) = top_point(&obj.stair);
    let centre = polygon_centroid(&obj.footprint());
    let probe = top + (top - centre).normalized() * 6.0;
    detect_rooms(&above.walls, 0.5)
        .iter()
        .any(|r| point_in_polygon(probe, &r.polygon))
}

/// Auto Stairwell (CB-29, CB-30): a closed ring of invisible room-divider
/// walls on the floor above, following the stair's footprint, so a
/// "Stairwell" room forms there. Returns the number of walls added.
pub fn auto_stairwell(cx: &mut EditorContext, id: Id) -> Result<usize, String> {
    let fl = cx.floor;
    let obj = find(cx.floor(), id).ok_or("That stair no longer exists")?;
    if obj.is_landing() {
        return Err("A landing has no stairwell".into());
    }
    if fl + 1 >= cx.project.floors.len() {
        return Err("There is no floor above: build one first".into());
    }
    let above = &cx.project.floors[fl + 1];
    if obj
        .x
        .stairwell_walls
        .iter()
        .any(|w| above.wall(*w).is_some())
    {
        return Err("This stair already has a stairwell".into());
    }
    let poly = obj.footprint();
    cx.begin_change("Auto Stairwell");
    let mut ids = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if a.dist(b) < 0.01 {
            continue;
        }
        let mut w = Wall::new(a, b, STAIRWELL_THICKNESS, 0.0, WallKind::Interior);
        w.id = cx.project.alloc_id();
        w.flags.room_divider = true;
        w.flags.invisible = true;
        ids.push(w.id);
        cx.project.floors[fl + 1].walls.push(w);
    }
    // Name the room that formed.
    let mut anchor = obj.bottom_center() + obj.along() * 6.0;
    if !point_in_polygon(anchor, &poly) {
        anchor = polygon_centroid(&poly);
    }
    let rooms = detect_rooms(&cx.project.floors[fl + 1].walls, 0.5);
    cx.project
        .set_room_name(fl + 1, anchor, "Stairwell", "Stairwell", &rooms);
    let n = ids.len();
    update(&mut cx.project, fl, id, |o| o.x.stairwell_walls = ids);
    cx.mark_dirty();
    Ok(n)
}

/// Area of the stair footprint in square inches.
pub fn footprint_area(obj: &StairObj) -> f64 {
    polygon_area(&obj.footprint()).abs()
}

// ----- symbols -----

fn landing_strokes(o: &StairObj) -> Vec<PlanStroke> {
    let fp = o.footprint();
    let mut out = vec![PlanStroke::Polyline(fp.clone(), true)];
    if fp.len() == 4 {
        out.push(PlanStroke::Line(fp[0], fp[2]));
        out.push(PlanStroke::Line(fp[1], fp[3]));
    }
    out
}

/// The plan symbol on the stair's own floor: treads, outline, the UP arrow
/// and, where the floor above cuts it, the break line at 2/3 (CB-33).
pub fn symbol_strokes(o: &StairObj) -> Vec<PlanStroke> {
    if o.is_landing() {
        return landing_strokes(o);
    }
    let cut = (o.x.break_line && !o.is_ramp()).then_some(BREAK_AT);
    let mut out = plan_symbol(&o.stair, cut);
    if o.x.show_risers && !o.is_ramp() {
        let risers = o.solution().risers;
        let u = o.first_flight_len() * 0.3;
        out.push(PlanStroke::Text {
            pos: o.stair.origin + o.along() * u + o.right() * (o.stair.params.width * 0.12),
            text: format!("{risers}R"),
            height: 4.5,
            angle: o.stair.direction.to_degrees().rem_euclid(360.0),
        });
    }
    out
}

/// What a stair of the floor below shows on this floor: the part beyond
/// the break line (the treads the lower floor's symbol leaves out), the
/// outline and a "DN" arrow pointing back down (CB-33).
pub fn upper_strokes(o: &StairObj) -> Vec<PlanStroke> {
    if o.is_landing() {
        return Vec::new();
    }
    let full = plan_symbol(&o.stair, None);
    let lower = plan_symbol(&o.stair, Some(BREAK_AT));
    let mut out = Vec::new();
    for s in &full {
        if let PlanStroke::Line(a, b) = s {
            let drawn_below = lower
                .iter()
                .any(|l| matches!(l, PlanStroke::Line(c, d) if c == a && d == b));
            if !drawn_below {
                out.push(s.clone());
            }
        }
    }
    out.push(PlanStroke::Polyline(footprint(&o.stair), true));
    if let Some(brk) = lower
        .iter()
        .find(|s| matches!(s, PlanStroke::Polyline(p, false) if p.len() == 6))
    {
        out.push(brk.clone());
    }
    // The arrow path of the full symbol ends at the top of the stair.
    let path = full.iter().rev().find_map(|s| match s {
        PlanStroke::Polyline(p, false) if p.len() >= 2 => Some(p),
        _ => None,
    });
    if let Some(p) = path {
        let (a, b) = (p[p.len() - 2], p[p.len() - 1]);
        let dir = (b - a).normalized();
        let side = dir.perp();
        let len = a.dist(b).min(48.0);
        let end = b - dir * len;
        out.push(PlanStroke::Line(b, end));
        out.push(PlanStroke::Polyline(
            vec![
                end,
                end + dir * 6.0 + side * 2.7,
                end + dir * 6.0 - side * 2.7,
            ],
            true,
        ));
        out.push(PlanStroke::Text {
            pos: b - dir * 14.0 + side * 4.0,
            text: "DN".into(),
            height: 6.0,
            angle: dir.angle().to_degrees().rem_euclid(360.0),
        });
    }
    out
}

/// The cross-section stick figure of a stair as `(x, y)` inches, y up
/// (the preview of the specification dialog): risers and treads from the
/// solved layout, a landing as one long tread.
pub fn elevation_points(o: &StairObj) -> Vec<(f64, f64)> {
    let p = &o.stair.params;
    let sol = solve(p);
    if let StairShape::Ramp { .. } = p.shape {
        return vec![(0.0, 0.0), (sol.total_run, p.total_rise.max(0.0))];
    }
    let (h, t) = (sol.riser_height, sol.tread_depth);
    let t1 = first_flight_treads(p, sol.risers);
    let landing = match p.shape {
        StairShape::LShaped { .. } | StairShape::UShaped { .. } if sol.risers >= 2 => {
            Some(p.landing_depth.max(p.width))
        }
        _ => None,
    };
    let (mut x, mut y) = (0.0, 0.0);
    let mut pts = vec![(x, y)];
    for i in 0..sol.risers {
        y += h;
        pts.push((x, y));
        if i + 1 < sol.risers {
            x += match landing {
                Some(l) if i == t1 => l,
                _ => t,
            };
            pts.push((x, y));
        }
    }
    pts.push((x + t * 0.6, y));
    pts
}

// ----- drawing -----

fn screen_pts(cam: &Camera, pts: &[Point]) -> Vec<Pos2> {
    pts.iter().map(|p| cam.world_to_screen(*p)).collect()
}

fn is_convex(pts: &[Point]) -> bool {
    let n = pts.len();
    if n < 4 {
        return true;
    }
    let mut sign = 0.0f64;
    for i in 0..n {
        let c = (pts[(i + 1) % n] - pts[i]).cross(pts[(i + 2) % n] - pts[(i + 1) % n]);
        if c.abs() > 1e-9 {
            if sign == 0.0 {
                sign = c.signum();
            } else if c.signum() != sign {
                return false;
            }
        }
    }
    true
}

fn draw_text(
    painter: &egui::Painter,
    cam: &Camera,
    pos: Point,
    text: &str,
    height: f64,
    angle_deg: f64,
    color: Color32,
) {
    let size = (height * cam.px_per_in) as f32 * 1.15;
    if size < 5.0 {
        return;
    }
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(size), color);
    let a = -(angle_deg.to_radians() as f32);
    let (s, c) = a.sin_cos();
    let ascent = size * 0.8;
    let top_left = cam.world_to_screen(pos) + egui::vec2(s * ascent, -c * ascent);
    painter.add(egui::epaint::TextShape::new(top_left, galley, color).with_angle(a));
}

/// Draws plan-symbol strokes (inches, Y up) through `cam`.
pub fn draw_strokes(
    painter: &egui::Painter,
    cam: &Camera,
    strokes: &[PlanStroke],
    color: Color32,
    weight: f32,
    dashed: bool,
) {
    let st = Stroke::new(weight, color);
    let line = |pts: Vec<Pos2>, closed: bool| {
        let mut pts = pts;
        if closed && pts.len() > 2 {
            pts.push(pts[0]);
        }
        if dashed {
            painter.extend(Shape::dashed_line(&pts, st, 6.0, 4.0));
        } else {
            painter.add(Shape::line(pts, st));
        }
    };
    for s in strokes {
        match s {
            PlanStroke::Line(a, b) => line(screen_pts(cam, &[*a, *b]), false),
            PlanStroke::Polyline(pts, closed) => line(screen_pts(cam, pts), *closed),
            PlanStroke::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let n = 24;
                let pts: Vec<Point> = (0..=n)
                    .map(|i| {
                        let t = start_deg + (end_deg - start_deg) * f64::from(i) / f64::from(n);
                        let (sn, cs) = t.to_radians().sin_cos();
                        Point::new(center.x + radius * cs, center.y + radius * sn)
                    })
                    .collect();
                painter.add(Shape::line(screen_pts(cam, &pts), st));
            }
            PlanStroke::Text {
                pos,
                text,
                height,
                angle,
            } => draw_text(painter, cam, *pos, text, *height, *angle, color),
        }
    }
}

/// Draws a stair or landing in the style of its settings.
pub fn draw_object(painter: &egui::Painter, cam: &Camera, pal: &Palette, o: &StairObj) {
    let fp = o.footprint();
    if o.x.fill && is_convex(&fp) && fp.len() >= 3 {
        painter.add(Shape::convex_polygon(
            screen_pts(cam, &fp),
            Color32::from_gray(o.x.fill_gray).gamma_multiply(0.45),
            Stroke::NONE,
        ));
    }
    draw_strokes(
        painter,
        cam,
        &symbol_strokes(o),
        pal.text,
        o.x.line_weight.max(0.25),
        o.x.dashed,
    );
    if o.x.show_label && !o.x.label.is_empty() {
        draw_text(
            painter,
            cam,
            polygon_centroid(&fp),
            &o.x.label,
            6.0,
            0.0,
            pal.text,
        );
    }
}

/// Draws every stair of the floor, the arriving part of the floor below's
/// stairs ("DN"), and the highlight and handles of the selection. Called
/// from `render::draw_plan`.
pub fn draw_stairs(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    if !cx.layers().is_visible(LAYER) {
        return;
    }
    let pal = &cx.palette;
    if cx.floor > 0 {
        for o in load(&cx.project.floors[cx.floor - 1]) {
            draw_strokes(
                painter,
                cam,
                &upper_strokes(&o),
                pal.text.gamma_multiply(0.6),
                1.0,
                true,
            );
        }
    }
    let objs = load(cx.floor());
    for o in &objs {
        draw_object(painter, cam, pal, o);
    }
    for o in &objs {
        let r = ObjectRef::Stair(o.id());
        let stroke = if cx.selection.contains(r) {
            Stroke::new(3.0_f32, pal.selection)
        } else if cx.hover == Some(r) {
            Stroke::new(2.0_f32, pal.hover)
        } else {
            continue;
        };
        painter.add(Shape::closed_line(screen_pts(cam, &o.footprint()), stroke));
    }
    if let Some(o) = selected_stair(cx) {
        handles::draw(&editor_handles(&o, cam.px_per_in), painter, cam, pal);
    }
}
