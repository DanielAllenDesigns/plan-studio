//! Stairs in the plan (CB-22..CB-34 in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`): storage in the
//! floor's opaque `stairs` slot, plan-symbol drawing, hit-testing, edit
//! handles, landings that join stair sections, the lock and height rules of
//! the Staircase Specification, and the stair commands (Auto Stairwell,
//! Flare/Curve, break line, Make Railing).
//!
//! The 3D scene gets its stair meshes from [`scene_meshes`].
//!
//! Stairs are stored as the JSON of a [`plan_stairs::Stair`] plus one extra
//! key, `"x"`, holding the plan-only settings ([`StairExtras`]: label, line
//! and fill style, materials, landing depth ...). `plan-stairs` ignores the
//! extra key, so the 3D builders read the same objects.

use super::handles::{self, Handle, HandleKind};
use super::{Camera, EditorContext, ObjectRef};
use crate::theme::Palette;
use eframe::egui::{self, Color32, CursorIcon, FontId, Pos2, Shape, Stroke};
use plan_core::foundation::{FoundationLayer, PlatformHole};
use plan_core::geometry::{point_in_polygon, polygon_area, polygon_centroid, Point};
use plan_core::{detect_rooms, Floor, Id, Project, Wall, WallKind};
use plan_stairs::{
    footprint, plan_symbol, solve, top_point, SideKind, Stair, StairParams, StairShape,
    StairSolution, Stroke as PlanStroke, Turn,
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
/// The hole in the floor platform is the stair's footprint grown by this
/// much on every side, so the faces around the cut lie just outside the
/// footprint (0.05" is a third of a percent of a typical stair's area).
pub const STAIRWELL_HOLE_MARGIN: f64 = 0.05;
/// The divider walls stand this far outside the footprint so the hole lies
/// strictly inside the Stairwell room (a hole that coincides with a room's
/// outline cannot be cut out of its platform).
pub const STAIRWELL_MARGIN: f64 = 0.3;
/// How close a stair end must be to a landing to join it (inches).
pub const JOIN_TOLERANCE: f64 = 6.0;
/// Default distance of the walking line from the centre of a curved stair
/// when it is placed with a click.
pub const DEFAULT_CURVE_RADIUS: f64 = 60.0;
/// Default number of treads of a Click Stairs stair is the solved count; its
/// run is that many default treads.
pub const CLICK_TREAD: f64 = 10.0;

// ----- the tool variants -----

/// The stair tools of the Build > Stairs flyout (CB-22).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StairKind {
    Draw,
    /// Click Stairs: one click places a straight stair of default length.
    Click,
    Straight,
    LShaped,
    UShaped,
    CurveLeft,
    CurveRight,
    /// Curved Stairs: click the centre, drag to the walking radius.
    Curved,
    Landing,
    Ramp,
}

impl StairKind {
    pub const ALL: [StairKind; 10] = [
        StairKind::Draw,
        StairKind::Click,
        StairKind::Straight,
        StairKind::LShaped,
        StairKind::UShaped,
        StairKind::CurveLeft,
        StairKind::CurveRight,
        StairKind::Curved,
        StairKind::Landing,
        StairKind::Ramp,
    ];

    /// Chief's name (the flyout entry).
    pub fn name(self) -> &'static str {
        match self {
            StairKind::Draw => "Draw Stairs",
            StairKind::Click => "Click Stairs",
            StairKind::Straight => "Straight Stairs",
            StairKind::LShaped => "L-Shaped Stair",
            StairKind::UShaped => "U-Shaped Stair",
            StairKind::CurveLeft => "Curve to Left",
            StairKind::CurveRight => "Curve to Right",
            StairKind::Curved => "Curved Stairs",
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
            StairKind::Click => {
                "Click Stairs: click to place a straight stair of default length toward the pointer"
            }
            StairKind::Curved => {
                "Curved Stairs: click the centre, drag to the walking radius; Tab flips the turn"
            }
            StairKind::Landing => {
                "Landing: drag a rectangle, or click the corners and double-click the last; double-click alone places a 3' square"
            }
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
    /// Mirror of the landing depth of a [`StairShape::Landing`] (older files
    /// kept the landing here; [`parse`] converts them).
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
    /// Lock settings: the Run handle and a riser-count change leave the
    /// tread depth alone.
    pub lock_tread: bool,
    /// A change of height keeps the riser height (the count follows).
    pub lock_riser: bool,
    /// A change of height keeps the number of treads (the riser height follows).
    pub lock_count: bool,
    /// Floor-to-floor height at the floor the stair was drawn on, when it was
    /// drawn; 0 when unknown (the Fit to Floor-to-Floor button of the dialog).
    pub story_rise: f64,
    /// Component / material pairs of the Materials tab.
    pub materials: Vec<(String, String)>,
    /// Ids of the room-divider walls Auto Stairwell added on the floor above.
    pub stairwell_walls: Vec<Id>,
    /// Id of the platform hole Auto Stairwell cut in the floor above (CB-29).
    pub stairwell_hole: Option<Id>,
    /// Guard railing around the stairwell opening on the floor above
    /// (CB-29): every side of the hole but the one the stair arrives at.
    pub stairwell_guard: bool,
    /// Ids of the railing walls that guard made on the floor above.
    pub guard_walls: Vec<Id>,
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
            lock_count: false,
            story_rise: 0.0,
            materials: vec![
                m("Treads", "Oak"),
                m("Risers", "Painted White"),
                m("Stringers", "Pine"),
                m("Handrail", "Oak"),
                m("Balusters", "Painted White"),
            ],
            stairwell_walls: Vec::new(),
            stairwell_hole: None,
            stairwell_guard: false,
            guard_walls: Vec::new(),
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
            "lock_count": self.lock_count,
            "story_rise": self.story_rise,
            "materials": self.materials.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>(),
            "stairwell_walls": self.stairwell_walls,
            "stairwell_hole": self.stairwell_hole,
            "stairwell_guard": self.stairwell_guard,
            "guard_walls": self.guard_walls,
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
            lock_count: b("lock_count", false),
            story_rise: f("story_rise", 0.0),
            materials,
            stairwell_walls: v
                .get("stairwell_walls")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default(),
            stairwell_hole: v.get("stairwell_hole").and_then(Value::as_u64),
            stairwell_guard: b("stairwell_guard", false),
            guard_walls: v
                .get("guard_walls")
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
        matches!(self.stair.params.shape, StairShape::Landing { .. })
    }

    /// Depth of a landing along its direction (`None` for a stair).
    pub fn landing_depth(&self) -> Option<f64> {
        match self.stair.params.shape {
            StairShape::Landing { depth } => Some(depth),
            _ => None,
        }
    }

    /// Resizes a landing's depth (the rectangle; a polygon landing keeps its outline).
    pub fn set_landing_depth(&mut self, depth: f64) {
        self.stair.params.shape = StairShape::Landing {
            depth: depth.max(MIN_SIZE),
        };
        self.x.landing_depth = Some(depth.max(MIN_SIZE));
    }

    /// A landing drawn as a polygon rather than a rectangle.
    pub fn is_polygon_landing(&self) -> bool {
        self.is_landing() && self.stair.params.outline.len() >= 3
    }

    pub fn is_curved(&self) -> bool {
        matches!(self.stair.params.shape, StairShape::Curved { .. })
    }

    /// Height of the platform at the top of a landing, above its floor.
    pub fn landing_height(&self) -> f64 {
        self.stair.params.total_rise
    }

    /// Distance of the bottom of the stair above the floor it was drawn on
    /// (non-zero for a section that starts on a landing): the Bottom Height.
    pub fn bottom_height(&self) -> f64 {
        self.stair.base
    }

    /// Height of the top of the stair above that floor: the Top Height.
    pub fn top_height(&self) -> f64 {
        self.bottom_height() + self.stair.params.total_rise
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

    /// Plan polygon of the whole object (the outline of a landing).
    pub fn footprint(&self) -> Vec<Point> {
        footprint(&self.stair)
    }

    /// Length of the first flight, measured along the direction of travel.
    pub fn first_flight_len(&self) -> f64 {
        if let Some(d) = self.landing_depth() {
            return d;
        }
        let sol = self.solution();
        if self.is_ramp() {
            // The first run of a ramp, before its first landing.
            let gaps = f64::from(sol.landings) * plan_stairs::RAMP_LANDING;
            return (sol.total_run - gaps) / f64::from(sol.landings + 1);
        }
        f64::from(first_flight_treads(&self.stair.params, sol.risers)) * sol.tread_depth
    }

    /// Where the Run handle sits: the top end of the first flight (the top
    /// of the whole curve for a curved stair).
    pub fn run_handle_pos(&self) -> Point {
        if self.is_curved() {
            return top_point(&self.stair).0;
        }
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
        StairShape::Straight
        | StairShape::Ramp { .. }
        | StairShape::Curved { .. }
        | StairShape::Landing { .. } => return risers.saturating_sub(1),
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
    let mut stair: Stair = serde_json::from_value(v.clone()).ok()?;
    let xv = v.get("x");
    let mut x = xv.map(StairExtras::from_json).unwrap_or_default();
    // Older files kept a landing as a straight stair plus `landing_depth`
    // and the railing sides as two flags.
    if let (Some(d), false) = (
        x.landing_depth,
        matches!(stair.params.shape, StairShape::Landing { .. }),
    ) {
        stair.params.shape = StairShape::Landing { depth: d };
    }
    if let StairShape::Landing { depth } = stair.params.shape {
        x.landing_depth = Some(depth);
    }
    let legacy = |k: &str| {
        xv.and_then(|x| x.get(k))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    if legacy("railing_left") && stair.params.left_side == SideKind::None {
        stair.params.left_side = SideKind::Railing;
    }
    if legacy("railing_right") && stair.params.right_side == SideKind::None {
        stair.params.right_side = SideKind::Railing;
    }
    Some(StairObj { stair, x })
}

fn to_json(o: &StairObj) -> Value {
    let mut v = serde_json::to_value(&o.stair).unwrap_or(Value::Null);
    if let Some(m) = v.as_object_mut() {
        let mut x = o.x.clone();
        x.landing_depth = o.landing_depth();
        m.insert("x".into(), x.to_json());
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

/// Edits one object in place. Returns false if it does not exist. A
/// stairwell the stair carries (hole and divider walls on the floor above)
/// follows the edit.
pub fn update(project: &mut Project, fl: usize, id: Id, f: impl FnOnce(&mut StairObj)) -> bool {
    let Some(idx) = project.floors[fl]
        .stairs
        .iter()
        .position(|v| value_id(v) == Some(id))
    else {
        return false;
    };
    let Some(mut o) = parse(&project.floors[fl].stairs[idx]) else {
        return false;
    };
    f(&mut o);
    resync_stairwell(project, fl, &mut o);
    project.floors[fl].stairs[idx] = to_json(&o);
    true
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

/// A rectangular landing (id 0) from the corner `a` to the corner `b` (a
/// 3' square when `b` is `None`). Its top is at the floor until a stair
/// section joins it ([`connect`]).
fn rect_landing(project: &Project, fl: usize, a: Point, b: Option<Point>) -> StairObj {
    let floor = &project.floors[fl];
    let (lo, hi) = match b.filter(|b| a.dist(*b) > 1e-6) {
        Some(b) => (
            Point::new(a.x.min(b.x), a.y.min(b.y)),
            Point::new(a.x.max(b.x), a.y.max(b.y)),
        ),
        None => (a, Point::new(a.x + DEFAULT_LANDING, a.y + DEFAULT_LANDING)),
    };
    let width = (hi.y - lo.y).max(MIN_SIZE);
    let depth = (hi.x - lo.x).max(MIN_SIZE);
    let mut extras = StairExtras {
        landing_depth: Some(depth),
        break_line: false,
        show_risers: false,
        story_rise: floor_rise(project, fl),
        ..StairExtras::default()
    };
    extras.landing_depth = Some(depth);
    let params = StairParams {
        total_rise: 0.0,
        width,
        shape: StairShape::Landing { depth },
        ..StairParams::default()
    };
    // Facing +x the left edge is +y.
    let mut s = Stair::new(0, Point::new(lo.x, lo.y + width), 0.0, params);
    s.floor_elevation = floor.elevation;
    StairObj {
        stair: s,
        x: extras,
    }
}

/// A polygon landing (id 0) from at least three corners: the Landing tool's
/// click-click-double-click. The depth and width are the outline's extents
/// along and across the first edge.
pub fn build_polygon_landing(project: &Project, fl: usize, pts: &[Point]) -> StairObj {
    let mut o = rect_landing(project, fl, pts[0], None);
    let (lo, hi) = pts.iter().fold(
        (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        ),
        |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        },
    );
    o.stair.origin = pts[0];
    o.stair.params.outline = pts.to_vec();
    o.stair.params.width = (hi.y - lo.y).max(MIN_SIZE);
    o.stair.params.shape = StairShape::Landing {
        depth: (hi.x - lo.x).max(MIN_SIZE),
    };
    o.x.landing_depth = o.landing_depth();
    o
}

/// A new stair (id 0) for the tool `kind`. `a` is the click or drag start,
/// `b` the drag end: the drag direction is the direction of travel and its
/// length the run (CB-23). The number of risers comes from the floor rise.
/// For Curved Stairs `a` is the centre and `b` a point on the walking line.
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

    if kind == StairKind::Landing {
        return rect_landing(project, fl, a, b);
    }

    let rise = floor_rise(project, fl);
    let mut extras = StairExtras {
        story_rise: rise,
        ..StairExtras::default()
    };
    let mut params = StairParams {
        total_rise: rise,
        width: DEFAULT_WIDTH,
        ..StairParams::default()
    };
    let mut direction = drag.map_or(FRAC_PI_2, |b| b.sub(a).angle());
    let run = drag.map(|b| a.dist(b));
    let mut anchor = a;

    match kind {
        StairKind::Draw | StairKind::Click | StairKind::Straight | StairKind::Landing => {
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
        StairKind::Curved => {
            // `a` is the centre; the stair starts at `b` on the walking line
            // (60" out and below the centre when only clicked).
            let start = b
                .filter(|b| a.dist(*b) > 1e-6)
                .unwrap_or_else(|| Point::new(a.x, a.y - DEFAULT_CURVE_RADIUS));
            let walk = a.dist(start).max(1.0);
            let phi = (start - a).angle();
            params.turn = turn;
            params.shape = StairShape::Curved {
                inner_radius: (walk - params.width / 2.0).max(0.0),
            };
            direction = match turn {
                Turn::Left => phi + FRAC_PI_2,
                Turn::Right => phi - FRAC_PI_2,
            };
            anchor = start;
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
    let origin = anchor - right * (params.width * 0.5);
    let mut s = Stair::new(0, origin, direction, params);
    s.floor_elevation = floor.elevation;
    StairObj {
        stair: s,
        x: extras,
    }
}

/// The run of a Click Stairs stair: the solved number of default treads.
pub fn click_run(project: &Project, fl: usize) -> f64 {
    let params = StairParams {
        total_rise: floor_rise(project, fl),
        ..StairParams::default()
    };
    f64::from(solve(&params).treads) * CLICK_TREAD
}

// ----- landings that join stair sections -----

/// Which end of a stair touches a landing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Joint {
    /// The top of the stair arrives on the landing.
    Top,
    /// The bottom of the stair starts on the landing.
    Bottom,
}

fn touches(poly: &[Point], p: Point) -> bool {
    point_in_polygon(p, poly) || edge_distance(poly, p) <= JOIN_TOLERANCE
}

/// Does an end of `stair` meet the landing's outline?
pub fn joint(stair: &StairObj, landing: &StairObj) -> Option<Joint> {
    if stair.is_landing() || !landing.is_landing() {
        return None;
    }
    let poly = landing.footprint();
    let (top, _) = top_point(&stair.stair);
    if touches(&poly, top) {
        return Some(Joint::Top);
    }
    if touches(&poly, stair.bottom_center()) {
        return Some(Joint::Bottom);
    }
    None
}

/// Joins `id` (a stair or a landing) to what it touches (CB-27): a landing
/// takes the height of the stair that arrives on it, and a stair that starts
/// on a landing begins at the landing's height and rises the rest of the
/// floor-to-floor height. Returns how many joints were made.
pub fn connect(project: &mut Project, fl: usize, id: Id) -> usize {
    let objs = load(&project.floors[fl]);
    let Some(me) = objs.iter().find(|o| o.id() == id).cloned() else {
        return 0;
    };
    let pairs: Vec<(StairObj, StairObj)> = if me.is_landing() {
        objs.iter()
            .filter(|o| !o.is_landing())
            .map(|s| (s.clone(), me.clone()))
            .collect()
    } else {
        objs.iter()
            .filter(|o| o.is_landing())
            .map(|l| (me.clone(), l.clone()))
            .collect()
    };
    let story = floor_rise(project, fl);
    let joints: Vec<(StairObj, StairObj, Joint)> = pairs
        .into_iter()
        .filter_map(|(s, l)| joint(&s, &l).map(|j| (s, l, j)))
        .collect();
    let mut n = 0;
    // A landing takes the height of the stair that arrives on it ...
    for (stair, landing, _) in joints.iter().filter(|j| j.2 == Joint::Top) {
        let h = stair.top_height();
        update(project, fl, landing.id(), |l| l.stair.params.total_rise = h);
        n += 1;
    }
    // ... and a stair that starts on it begins there.
    for (stair, landing, _) in joints.iter().filter(|j| j.2 == Joint::Bottom) {
        let h = find(&project.floors[fl], landing.id()).map_or(0.0, |l| l.landing_height());
        let rest = story - h;
        if rest > plan_stairs::MIN_RISER {
            update(project, fl, stair.id(), |s| {
                s.stair.base = h;
                s.stair.params.total_rise = rest;
            });
            n += 1;
        }
    }
    n
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
    /// Move corner `i` of a polygon landing's outline.
    Corner(usize),
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
    // On a curve the width handles sit just past the first riser.
    let mid = if obj.is_curved() {
        6.0
    } else {
        obj.first_flight_len() * 0.5
    };
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
    // A polygon landing moves, and each corner of its outline reshapes it.
    if obj.is_polygon_landing() {
        out.retain(|h| h.kind == StairHandleKind::Move);
        for (i, p) in obj.stair.params.outline.iter().enumerate() {
            out.push(StairHandle {
                kind: StairHandleKind::Corner(i),
                pos: *p,
                cursor: CursorIcon::Crosshair,
            });
        }
    }
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
                StairHandleKind::Corner(i) => HandleKind::Reshape(i),
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
        StairHandleKind::Corner(_) => "Reshape Landing",
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
        StairHandleKind::Move => {
            let d = to - start;
            o.stair.origin = orig.stair.origin + d;
            for p in &mut o.stair.params.outline {
                *p = *p + d;
            }
        }
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
            if let Some(c) = plan_stairs::curve_center(&orig.stair) {
                set_curve_sweep(&mut o, c, to);
            } else {
                let l = (to - orig.bottom_center()).dot(along).max(MIN_SIZE);
                set_run(&mut o, l);
            }
        }
        StairHandleKind::WidthRight => {
            o.stair.params.width = (to - orig.stair.origin).dot(right).max(MIN_SIZE);
        }
        StairHandleKind::WidthLeft => {
            let off = (to - orig.stair.origin).dot(right).min(w - MIN_SIZE);
            o.stair.origin = orig.stair.origin + right * off;
            o.stair.params.width = w - off;
        }
        StairHandleKind::Corner(i) => {
            if i < o.stair.params.outline.len() {
                o.stair.params.outline[i] = to;
                reshape_polygon_landing(&mut o);
            }
        }
    }
    o
}

/// Brings a polygon landing's width, depth and origin back in line with its
/// outline after a corner moved: the origin stays the first corner, the
/// depth and width are the outline's extents (as when it was drawn).
fn reshape_polygon_landing(o: &mut StairObj) {
    let pts = &o.stair.params.outline;
    if pts.len() < 3 {
        return;
    }
    let (lo, hi) = pts.iter().fold(
        (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        ),
        |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        },
    );
    o.stair.origin = pts[0];
    o.stair.params.width = (hi.y - lo.y).max(MIN_SIZE);
    let depth = (hi.x - lo.x).max(MIN_SIZE);
    o.stair.params.shape = StairShape::Landing { depth };
    o.x.landing_depth = Some(depth);
}

/// Sets the length of the first flight: the tread depth follows (the number
/// of treads is fixed by the floor-to-floor rise), a landing grows, a ramp
/// changes its slope.
pub fn set_run(o: &mut StairObj, l: f64) {
    if o.is_landing() {
        o.set_landing_depth(l);
    } else if o.is_ramp() {
        let rise = o.stair.params.total_rise.max(1.0);
        let runs = f64::from(plan_stairs::ramp_runs(rise));
        o.stair.params.shape = StairShape::Ramp {
            slope_1_in: (l / (rise / runs)).max(1.0),
        };
    } else if !o.x.lock_tread {
        let treads = first_flight_treads(&o.stair.params, o.solution().risers);
        if treads > 0 {
            o.stair.params.tread_depth = (l / f64::from(treads)).max(1.0);
        }
    }
}

/// Dragging the top of a curved stair to `to` sets how far round it goes:
/// the tread depth on the walking line follows the angle (unless locked).
fn set_curve_sweep(o: &mut StairObj, centre: Point, to: Point) {
    if o.x.lock_tread {
        return;
    }
    let treads = o.solution().treads;
    let StairShape::Curved { inner_radius } = o.stair.params.shape else {
        return;
    };
    if treads == 0 {
        return;
    }
    let walk = inner_radius + o.stair.params.width / 2.0;
    let start = o.bottom_center() - centre;
    let a0 = start.angle();
    let a1 = (to - centre).angle();
    // Counter-clockwise for a left turn.
    let mut sweep = if o.stair.params.turn == Turn::Left {
        a1 - a0
    } else {
        a0 - a1
    };
    while sweep < 0.2 {
        sweep += std::f64::consts::TAU;
    }
    while sweep > std::f64::consts::TAU + 0.2 {
        sweep -= std::f64::consts::TAU;
    }
    o.stair.params.tread_depth = (sweep * walk / f64::from(treads)).max(1.0);
}

// ----- the heights and locks of the Staircase Specification -----

/// Changes the floor-to-floor rise of a stair, honouring the lock settings
/// (CB-24): with the number of treads locked the riser height follows; with
/// the riser height locked the count follows; unlocked, the count is rounded
/// from the riser height and the riser height then shows the actual value.
/// Unless the tread depth is locked, a straight stair keeps its total run
/// when the count changes.
pub fn set_total_rise(o: &mut StairObj, rise: f64) {
    if o.is_landing() || o.is_ramp() {
        o.stair.params.total_rise = rise.max(0.0);
        return;
    }
    let rise = rise.max(1.0);
    let before = o.solution();
    let run = f64::from(before.treads) * o.stair.params.tread_depth;
    o.stair.params.total_rise = rise;
    if o.x.lock_count {
        o.stair.params.riser_height_target = rise / f64::from(before.risers.max(1));
    } else {
        let sol = solve(&o.stair.params);
        if !o.x.lock_riser {
            o.stair.params.riser_height_target = sol.riser_height;
        }
        keep_run(o, run, before.treads, sol.treads);
    }
}

/// Sets the number of risers (the treads are one fewer); the riser height
/// follows from the rise.
pub fn set_risers(o: &mut StairObj, risers: u32) {
    if o.is_landing() || o.is_ramp() {
        return;
    }
    let before = o.solution();
    let run = f64::from(before.treads) * o.stair.params.tread_depth;
    let n = risers.clamp(2, 99);
    o.stair.params.riser_height_target = o.stair.params.total_rise.max(1.0) / f64::from(n);
    keep_run(o, run, before.treads, n - 1);
}

/// Sets the number of treads (risers = treads + 1).
pub fn set_treads(o: &mut StairObj, treads: u32) {
    set_risers(o, treads + 1);
}

/// Moves the bottom of the stair to `height` above the floor it was drawn
/// on, keeping the top where it is (so the rise changes).
pub fn set_bottom_height(o: &mut StairObj, height: f64) {
    let top = o.top_height();
    o.stair.base = height;
    set_total_rise(o, top - height);
}

/// Moves the top of the stair to `height` above the floor it was drawn on.
pub fn set_top_height(o: &mut StairObj, height: f64) {
    let rise = height - o.bottom_height();
    set_total_rise(o, rise);
}

/// Makes the stair rise from its bottom to the floor above (the Top Height
/// from the floors): the story height recorded when it was drawn.
pub fn fit_to_story(o: &mut StairObj) -> bool {
    if o.x.story_rise <= 0.0 {
        return false;
    }
    let top = o.x.story_rise;
    set_top_height(o, top);
    true
}

/// A straight stair keeps its total run when the number of treads changes
/// (the tread depth is what moves) unless the tread depth is locked.
fn keep_run(o: &mut StairObj, run: f64, was: u32, now: u32) {
    if o.x.lock_tread || was == now || now == 0 {
        return;
    }
    if matches!(o.stair.params.shape, StairShape::Straight) {
        o.stair.params.tread_depth = (run / f64::from(now)).max(1.0);
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
    /// Make Railing (CB-31): a railing on both sides of the stair.
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
                cx.status = format!(
                    "Auto Stairwell: {n} room-divider walls and a hole in the floor platform added on the floor above"
                );
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
                    StairShape::Curved { .. } => StairShape::Straight,
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
                o.stair.params.left_side = SideKind::Railing;
                o.stair.params.right_side = SideKind::Railing;
            });
            cx.mark_dirty();
            cx.status = "Make Railing: railings with newels and balusters on both sides".into();
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
    connect(&mut cx.project, fl, edited.id());
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
            remove_stairwell(&mut cx.project, fl, &o);
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

/// `poly` grown by `m` on every side.
fn grow(poly: &[Point], m: f64) -> Vec<Point> {
    // `offset_ring` moves a counter-clockwise ring inward for positive `d`.
    let d = if polygon_area_signed(poly) > 0.0 {
        -m
    } else {
        m
    };
    plan_core::foundation::offset_ring(poly, d)
}

/// The outline of the Stairwell room: the stair's footprint grown by
/// [`STAIRWELL_MARGIN`] on every side.
fn room_ring(poly: &[Point]) -> Vec<Point> {
    grow(poly, STAIRWELL_MARGIN)
}

/// The outline of the hole in the floor platform of the floor above.
pub fn hole_outline(o: &StairObj) -> Vec<Point> {
    grow(&o.footprint(), STAIRWELL_HOLE_MARGIN)
}

fn polygon_area_signed(poly: &[Point]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

/// Adds the closed ring of invisible room-divider walls for the outline
/// `poly` to floor `above`, returning their ids.
fn add_ring(project: &mut Project, above: usize, poly: &[Point]) -> Vec<Id> {
    let mut ids = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if a.dist(b) < 0.01 {
            continue;
        }
        let mut w = Wall::new(a, b, STAIRWELL_THICKNESS, 0.0, WallKind::Interior);
        w.id = project.alloc_id();
        w.flags.room_divider = true;
        w.flags.invisible = true;
        ids.push(w.id);
        project.floors[above].walls.push(w);
    }
    ids
}

/// Names the room the stairwell ring formed "Stairwell".
fn name_stairwell(project: &mut Project, above: usize, o: &StairObj, poly: &[Point]) {
    let mut anchor = o.bottom_center() + o.along() * 6.0;
    if !point_in_polygon(anchor, poly) {
        anchor = polygon_centroid(poly);
    }
    let rooms = detect_rooms(&project.floors[above].walls, 0.5);
    project.set_room_name(above, anchor, "Stairwell", "Stairwell", &rooms);
    // The room is open to below: no floor under it (R-30).
    if let Some(n) = project.floors[above]
        .room_names
        .iter_mut()
        .find(|n| n.anchor == anchor)
    {
        n.has_floor = false;
    }
}

/// Cuts (or moves) the stairwell hole `stair` owns in the floor platform of
/// floor `above`; returns its id.
fn put_hole(project: &mut Project, above: usize, stair: Id, poly: &[Point]) -> Id {
    let mut layer = FoundationLayer::load(&project.floors[above]);
    let id = match layer
        .platform_holes
        .iter_mut()
        .find(|h| h.owner == Some(stair))
    {
        Some(h) => {
            h.outline = poly.to_vec();
            h.id
        }
        None => {
            let id = project.alloc_id();
            layer
                .platform_holes
                .push(PlatformHole::stairwell(id, poly.to_vec(), stair));
            id
        }
    };
    layer.store(&mut project.floors[above]);
    id
}

/// Removes everything Auto Stairwell added for `o` on the floor above `fl`:
/// the divider walls and the platform hole.
fn remove_stairwell(project: &mut Project, fl: usize, o: &StairObj) {
    let Some(above) = project.floors.get_mut(fl + 1) else {
        return;
    };
    above.walls.retain(|w| !o.x.stairwell_walls.contains(&w.id));
    above.walls.retain(|w| !o.x.guard_walls.contains(&w.id));
    let mut layer = FoundationLayer::load(above);
    if layer.remove_owned(o.id()) > 0 {
        layer.store(above);
    }
}

/// Keeps the stairwell of a stair that moved or changed shape in step with
/// it: the hole follows the footprint, the divider walls are moved (or
/// rebuilt when the outline has a different number of corners).
fn resync_stairwell(project: &mut Project, fl: usize, o: &mut StairObj) {
    let has_walls = !o.x.stairwell_walls.is_empty();
    if (o.x.stairwell_hole.is_none() && !has_walls) || fl + 1 >= project.floors.len() {
        return;
    }
    resync_stairwell_parts(project, fl, o);
    sync_guard(project, fl, o);
}

/// Thickness and height of the guard railing around a stairwell, inches.
pub const GUARD_THICKNESS: f64 = 4.0;
pub const GUARD_HEIGHT: f64 = 36.0;

/// The sides of the stairwell opening a guard railing runs along: the hole's
/// outline without the side the stair arrives at.
pub fn guard_edges(o: &StairObj) -> Vec<(Point, Point)> {
    let ring = hole_outline(o);
    let n = ring.len();
    let edges: Vec<(Point, Point)> = (0..n)
        .map(|i| (ring[i], ring[(i + 1) % n]))
        .filter(|(a, b)| a.dist(*b) >= 1.0)
        .collect();
    let (top, _) = top_point(&o.stair);
    let arrival = edges
        .iter()
        .enumerate()
        .min_by(|a, b| {
            let da = plan_core::geometry::dist_to_segment(top, a.1 .0, a.1 .1);
            let db = plan_core::geometry::dist_to_segment(top, b.1 .0, b.1 .1);
            da.total_cmp(&db)
        })
        .map(|(i, _)| i);
    edges
        .into_iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != arrival)
        .map(|(_, e)| e)
        .collect()
}

/// Makes the railing walls of the stair's guard on the floor above `fl`
/// match its settings: none when the guard is off or there is no opening,
/// else one railing wall per side of the opening but the arrival side.
fn sync_guard(project: &mut Project, fl: usize, o: &mut StairObj) {
    if fl + 1 >= project.floors.len() {
        return;
    }
    let old = std::mem::take(&mut o.x.guard_walls);
    project.floors[fl + 1]
        .walls
        .retain(|w| !old.contains(&w.id));
    if !o.x.stairwell_guard || o.x.stairwell_hole.is_none() || o.is_landing() {
        return;
    }
    for (a, b) in guard_edges(o) {
        let mut w = Wall::new(a, b, GUARD_THICKNESS, GUARD_HEIGHT, WallKind::Interior);
        w.id = project.alloc_id();
        w.wall_type = Some("Railing-4".to_string());
        w.set_class(plan_core::WallClass::Railing);
        o.x.guard_walls.push(w.id);
        project.floors[fl + 1].walls.push(w);
    }
}

fn resync_stairwell_parts(project: &mut Project, fl: usize, o: &mut StairObj) {
    let has_walls = !o.x.stairwell_walls.is_empty();
    let poly = o.footprint();
    if o.x.stairwell_hole.is_some() {
        let hole = hole_outline(o);
        o.x.stairwell_hole = Some(put_hole(project, fl + 1, o.id(), &hole));
    }
    if has_walls {
        let ring = room_ring(&poly);
        let edges: Vec<(Point, Point)> = (0..ring.len())
            .map(|i| (ring[i], ring[(i + 1) % ring.len()]))
            .filter(|(a, b)| a.dist(*b) >= 0.01)
            .collect();
        let above = &mut project.floors[fl + 1];
        let present =
            o.x.stairwell_walls
                .iter()
                .all(|id| above.wall(*id).is_some());
        if present && edges.len() == o.x.stairwell_walls.len() {
            for (id, (a, b)) in o.x.stairwell_walls.iter().zip(&edges) {
                if let Some(w) = above.walls.iter_mut().find(|w| w.id == *id) {
                    w.start = *a;
                    w.end = *b;
                }
            }
        } else {
            above.walls.retain(|w| !o.x.stairwell_walls.contains(&w.id));
            o.x.stairwell_walls = add_ring(project, fl + 1, &ring);
            name_stairwell(project, fl + 1, o, &poly);
        }
    }
}

/// Auto Stairwell (CB-29, CB-30): a hole in the platform of the floor above
/// where the stair passes through, plus a closed ring of invisible
/// room-divider walls following the stair's footprint so a "Stairwell" room
/// forms there. Returns the number of walls added. The hole and the walls
/// are remembered with the stair: deleting the stair, or undoing the
/// command, takes them away.
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
    let hole_there = FoundationLayer::load(above).owned_hole(id).is_some();
    if hole_there
        || obj
            .x
            .stairwell_walls
            .iter()
            .any(|w| above.wall(*w).is_some())
    {
        return Err("This stair already has a stairwell".into());
    }
    let poly = obj.footprint();
    cx.begin_change("Auto Stairwell");
    let ids = add_ring(&mut cx.project, fl + 1, &room_ring(&poly));
    name_stairwell(&mut cx.project, fl + 1, &obj, &poly);
    let hole = put_hole(&mut cx.project, fl + 1, id, &hole_outline(&obj));
    let n = ids.len();
    // `update` brings the guard railing (when the stair asks for one) with it.
    update(&mut cx.project, fl, id, |o| {
        o.x.stairwell_walls = ids;
        o.x.stairwell_hole = Some(hole);
    });
    cx.mark_dirty();
    Ok(n)
}

/// Area of the stair footprint in square inches.
pub fn footprint_area(obj: &StairObj) -> f64 {
    polygon_area(&obj.footprint()).abs()
}

// ----- symbols -----

/// The plan symbol on the stair's own floor: treads, outline, the UP arrow
/// and, where the floor above cuts it, the break line at 2/3 (CB-33).
pub fn symbol_strokes(o: &StairObj) -> Vec<PlanStroke> {
    if o.is_landing() {
        return plan_symbol(&o.stair, None);
    }
    let cut = (o.x.break_line && !o.is_ramp()).then_some(BREAK_AT);
    let mut out = plan_symbol(&o.stair, cut);
    if o.x.show_risers && !o.is_ramp() {
        let risers = o.solution().risers;
        let u = if o.is_curved() {
            10.0
        } else {
            o.first_flight_len() * 0.3
        };
        out.push(PlanStroke::Text {
            pos: o.stair.origin + o.along() * u + o.right() * (o.stair.params.width * 0.12),
            text: format!("{risers}R"),
            height: 4.5,
            angle: o.stair.direction.to_degrees().rem_euclid(360.0),
        });
    }
    out
}

/// The treads beyond the break line, which the stair's own floor leaves out
/// of its symbol: drawn dashed so the whole run is still readable (CB-33).
/// Empty for landings, ramps and stairs without a break line.
pub fn hidden_strokes(o: &StairObj) -> Vec<PlanStroke> {
    if o.is_landing() || o.is_ramp() || !o.x.break_line {
        return Vec::new();
    }
    let full = plan_symbol(&o.stair, None);
    let lower = plan_symbol(&o.stair, Some(BREAK_AT));
    full.into_iter()
        .filter(|s| match s {
            PlanStroke::Line(a, b) => !lower
                .iter()
                .any(|l| matches!(l, PlanStroke::Line(c, d) if c == a && d == b)),
            _ => false,
        })
        .collect()
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
    if let StairShape::Landing { depth } = p.shape {
        return vec![(0.0, 0.0), (depth, 0.0)];
    }
    if let StairShape::Ramp { slope_1_in } = p.shape {
        let runs = plan_stairs::ramp_runs(p.total_rise);
        let rise = p.total_rise.max(0.0) / f64::from(runs);
        let (mut x, mut y) = (0.0, 0.0);
        let mut pts = vec![(x, y)];
        for k in 0..runs {
            x += rise * slope_1_in;
            y += rise;
            pts.push((x, y));
            if k + 1 < runs {
                x += plan_stairs::RAMP_LANDING;
                pts.push((x, y));
            }
        }
        return pts;
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

/// The meshes of every stair, landing and ramp of the floor for the 3D
/// scene: treads, risers, stringers, landings, ramp slabs, newels, balusters
/// and rails, side walls and half-walls, with `object_id` set to the stair.
/// Positions carry the stair's own floor elevation. Objects that do not
/// parse are skipped.
pub fn scene_meshes(floor: &Floor) -> Vec<plan_3d::Mesh> {
    load(floor)
        .iter()
        .flat_map(|o| plan_stairs::meshes(&o.stair))
        .collect()
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
    draw_strokes(
        painter,
        cam,
        &hidden_strokes(o),
        pal.text.gamma_multiply(0.5),
        o.x.line_weight.max(0.25),
        true,
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
