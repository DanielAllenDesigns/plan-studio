//! NKBA Kitchen Planning Guidelines and Bathroom Planning Guidelines, the rule
//! group "NKBA" of Plan Check.
//!
//! The model is read the way `rules_fixtures.rs` reads it: cabinets from the
//! floor's JSON (`kind`, `position`, `angle`, `width`, `depth`, `height`,
//! `elevation`, `countertop`, `cutouts`, `appliance`, `label`) and fixtures and
//! appliances from the placed library symbols (their catalog id names them).
//! A sink or a cooktop is also found as a cutout in a countertop, and an
//! appliance as the `appliance` of a cabinet. The electrical rules read the
//! placed devices like `rules_mep.rs`.
//!
//! Each guideline has a rule id in [`GUIDELINES`]. [`evaluate`] runs them and
//! also records which guidelines it could test in which room, so the Kitchen
//! and Bath report can say "met", "not met" or "not in the plan" for every
//! guideline, not only list the failures.
//!
//! All lengths are inches. Distances between work centres are taken between
//! the centres of their fronts. Rooms are measured to wall centerlines, as in
//! the rest of the crate.

use plan_core::geometry::{dist_to_segment, point_in_polygon, segment_intersection};
use plan_core::units::fmt_ft_in;
use plan_core::{Id, OpeningKind, OpeningStyle, Point, Project, Room};
use plan_stairs::Stair;
use serde_json::Value;

use crate::ctx::{Ctx, OpInfo};
use crate::geom::polygons_overlap;
use crate::rules::swing_polygon;
use crate::rules_mep::{devices, Dev};
use crate::settings::{CheckSettings, RuleInfo};
use crate::{finding, CheckOptions, Finding, ReportTable, Severity, Target};

const EPS: f64 = 1e-6;

// ----- the guidelines -----

/// Kitchen or bathroom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Kitchen,
    Bath,
}

impl Area {
    fn word(self) -> &'static str {
        match self {
            Area::Kitchen => "Kitchen",
            Area::Bath => "Bathroom",
        }
    }
}

/// One guideline the plan can be tested against.
pub(crate) struct Guideline {
    /// The `rule` text of its findings.
    pub id: &'static str,
    pub area: Area,
    pub severity: Severity,
    /// What the guideline asks, for the settings list and the report.
    pub requirement: &'static str,
}

const fn g(id: &'static str, area: Area, severity: Severity, requirement: &'static str) -> Guideline {
    Guideline {
        id,
        area,
        severity,
        requirement,
    }
}

use Area::{Bath as B, Kitchen as K};
use Severity::{Info as I, Warning as W};

pub(crate) const K_DOOR: &str = "NKBA kitchen entry door";
pub(crate) const K_SWING: &str = "NKBA kitchen door swing";
pub(crate) const K_TRIANGLE: &str = "NKBA kitchen work triangle";
pub(crate) const K_AISLE2: &str = "NKBA kitchen two-cook work aisle";
pub(crate) const K_SEAT_KNEE: &str = "NKBA kitchen seating space";
pub(crate) const K_SEAT_CLEAR: &str = "NKBA kitchen seating clearance";
pub(crate) const K_SINK: &str = "NKBA kitchen sink landing";
pub(crate) const K_COOKTOP: &str = "NKBA kitchen cooktop landing";
pub(crate) const K_FRIDGE: &str = "NKBA kitchen refrigerator landing";
pub(crate) const K_OVEN: &str = "NKBA kitchen oven landing";
pub(crate) const K_MICRO: &str = "NKBA kitchen microwave landing";
pub(crate) const K_DISHWASHER: &str = "NKBA kitchen dishwasher";
pub(crate) const K_COOK_CLEAR: &str = "NKBA kitchen cooking surface clearance";
pub(crate) const K_COOK_SAFE: &str = "NKBA kitchen cooking safety";
pub(crate) const K_VENT: &str = "NKBA kitchen ventilation";
pub(crate) const K_WASTE: &str = "NKBA kitchen waste receptacles";
pub(crate) const K_FRONTAGE: &str = "NKBA kitchen counter frontage";
pub(crate) const K_EDGES: &str = "NKBA kitchen countertop edges";
pub(crate) const K_RECEPT: &str = "NKBA kitchen receptacles";
pub(crate) const K_GFCI: &str = "NKBA kitchen GFCI protection";
pub(crate) const B_DOOR: &str = "NKBA bath entry door";
pub(crate) const B_CLEAR: &str = "NKBA bath clear floor space";
pub(crate) const B_TOILET: &str = "NKBA bath toilet space";
pub(crate) const B_SHOWER: &str = "NKBA bath shower size";
pub(crate) const B_CONTROLS: &str = "NKBA bath shower controls";
pub(crate) const B_GRAB: &str = "NKBA bath grab bar blocking";
pub(crate) const B_LAV_HEIGHT: &str = "NKBA bath lavatory height";
pub(crate) const B_LAV_SPACE: &str = "NKBA bath lavatory spacing";
pub(crate) const B_MIRROR: &str = "NKBA bath mirror";
pub(crate) const B_VENT: &str = "NKBA bath ventilation";
pub(crate) const B_RECEPT: &str = "NKBA bath receptacle at lavatory";
pub(crate) const B_GFCI: &str = "NKBA bath GFCI protection";

/// Every guideline Plan Check tests, kitchen first.
pub(crate) static GUIDELINES: &[Guideline] = &[
    g(K_DOOR, K, W, "The doorway into the kitchen is 32\" clear"),
    g(K_SWING, K, W, "A door does not swing into a cabinet or appliance"),
    g(K_TRIANGLE, K, W, "Work triangle: legs 4' to 9', total 13' to 26', no cabinet cutting a leg by more than 12\""),
    g(K_AISLE2, K, I, "A work aisle is 48\" when two cooks work (42\" for one)"),
    g(K_SEAT_KNEE, K, W, "Seating has knee space: 15\" at a 36\" counter, 12\" at 42\", 18\" at 30\""),
    g(K_SEAT_CLEAR, K, W, "36\" clear behind seating (44\" where people pass behind it)"),
    g(K_SINK, K, W, "Sink landing: 24\" of counter on one side and 18\" on the other"),
    g(K_COOKTOP, K, W, "Cooking surface landing: 15\" on one side and 12\" on the other"),
    g(K_FRIDGE, K, W, "Refrigerator landing: 15\" beside it, or 15\" across within 48\""),
    g(K_OVEN, K, W, "Oven landing: 15\" beside it, or 15\" across within 48\""),
    g(K_MICRO, K, W, "Microwave landing: 15\" beside it, or 15\" across within 48\""),
    g(K_DISHWASHER, K, W, "The dishwasher is within 36\" of the sink with 21\" of standing space"),
    g(K_COOK_CLEAR, K, W, "24\" (protected) or 30\" between the cooking surface and a cabinet above"),
    g(K_COOK_SAFE, K, I, "The cooking surface is not under or beside an operable window"),
    g(K_VENT, K, W, "A hood or exhaust fan over the cooking surface, as wide as the cooktop"),
    g(K_WASTE, K, I, "A waste receptacle in the kitchen"),
    g(K_FRONTAGE, K, W, "Counter frontage: 158\" (198\" in a kitchen of 150 sq ft or more)"),
    g(K_EDGES, K, I, "Island and peninsula countertops have clipped or rounded corners"),
    g(K_RECEPT, K, W, "A receptacle within 24\" of every counter point, and one at an island"),
    g(K_GFCI, K, W, "GFCI protection for receptacles within 6' of a sink"),
    g(B_DOOR, B, W, "The doorway into the bathroom is 32\" clear"),
    g(B_CLEAR, B, W, "30\" of clear floor space in front of the lavatory, tub and shower"),
    g(B_TOILET, B, I, "Toilet: 16\" from its centre to a side obstruction and 30\" clear in front"),
    g(B_SHOWER, B, W, "A shower is at least 36\" x 36\" inside"),
    g(B_CONTROLS, B, I, "Shower controls are 38\" to 48\" above the floor"),
    g(B_GRAB, B, I, "Blocking in the walls at the toilet, tub and shower for grab bars"),
    g(B_LAV_HEIGHT, B, I, "The lavatory top is 32\" to 43\" above the floor"),
    g(B_LAV_SPACE, B, W, "Lavatories are 30\" apart (centre to centre) and 15\" from a side obstruction"),
    g(B_MIRROR, B, I, "A mirror at the lavatory"),
    g(B_VENT, B, W, "An exhaust fan, or a window of 3 sq ft or more"),
    g(B_RECEPT, B, W, "A receptacle within 36\" of each lavatory"),
    g(B_GFCI, B, W, "GFCI protection for receptacles near the lavatory, tub and shower"),
];

/// Guidelines that need information the plan does not hold, listed in the
/// report as "not checked".
pub(crate) static NOT_CHECKABLE: &[(Area, &str, &str)] = &[
    (K, "Work triangle traffic", "No major traffic through the work triangle (the plan has no traffic paths)"),
    (K, "Storage", "Shelf and drawer storage totals by cabinet type (cabinet contents are not modelled)"),
    (K, "Countertop heights", "Counter heights suit the cooks (the cooks are not in the plan)"),
    (K, "Cooking exhaust volume", "Hood exhaust of 150 cfm or more, ducted outdoors (the plan holds no airflow)"),
    (K, "Fire extinguisher and smoke alarm", "A fire extinguisher in the kitchen and a smoke alarm near it"),
    (K, "Lighting", "Task and general lighting (illumination levels are not computed)"),
    (B, "Toilet compartment", "A toilet compartment of 36\" x 66\" or more when it is enclosed"),
    (B, "Tub and shower surfaces", "Slip-resistant floors, seat and wall surfaces (finishes are not tested)"),
    (B, "Lighting", "Lighting at the mirror and in the shower (illumination is not computed)"),
    (B, "Shower and tub valves", "Temperature control (anti-scald) valves (valves are not modelled)"),
];

/// The settings-list entries of the "NKBA" group.
pub(crate) fn catalog() -> Vec<RuleInfo> {
    GUIDELINES
        .iter()
        .map(|gl| RuleInfo {
            id: gl.id,
            group: "NKBA",
            severity: gl.severity,
            summary: gl.requirement,
        })
        .collect()
}

// ----- what the rules read -----

/// What a piece of the model is, for the NKBA rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Sink,
    Cooktop,
    Range,
    Oven,
    Microwave,
    Fridge,
    Dishwasher,
    Hood,
    Fan,
    Trash,
    Toilet,
    Tub,
    Shower,
    Valve,
    Grab,
    Mirror,
}

impl Kind {
    fn is_cook(self) -> bool {
        matches!(self, Kind::Cooktop | Kind::Range)
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Sink => "sink",
            Kind::Cooktop => "cooktop",
            Kind::Range => "range",
            Kind::Oven => "oven",
            Kind::Microwave => "microwave",
            Kind::Fridge => "refrigerator",
            Kind::Dishwasher => "dishwasher",
            Kind::Hood => "hood",
            Kind::Fan => "exhaust fan",
            Kind::Trash => "waste receptacle",
            Kind::Toilet => "toilet",
            Kind::Tub => "tub",
            Kind::Shower => "shower",
            Kind::Valve => "shower valve",
            Kind::Grab => "grab bar",
            Kind::Mirror => "mirror",
        }
    }
}

/// Names a library symbol, appliance or cutout. `None` for everything else.
fn classify(text: &str) -> Option<Kind> {
    let t = text.to_lowercase();
    let has = |ks: &[&str]| ks.iter().any(|k| t.contains(k));
    if has(&["hot_tub", "hot tub", "wine", "plants"]) || (has(&["lighting"]) && !has(&["exhaust"])) {
        return None;
    }
    Some(if has(&["hood"]) {
        Kind::Hood
    } else if has(&["exhaust"]) {
        Kind::Fan
    } else if has(&["cooktop", "cook top"]) {
        Kind::Cooktop
    } else if has(&["microwave"]) {
        Kind::Microwave
    } else if has(&["range", "stove"]) {
        Kind::Range
    } else if has(&["oven"]) {
        Kind::Oven
    } else if has(&["dishwasher"]) {
        Kind::Dishwasher
    } else if has(&["refrigerator", "fridge"]) {
        Kind::Fridge
    } else if has(&["trash", "compactor", "waste", "recycl"]) {
        Kind::Trash
    } else if has(&["toilet", "water_closet", "water closet"]) {
        Kind::Toilet
    } else if has(&["shower_valve", "shower valve", "shower_control", "shower control", "shower_trim"]) {
        Kind::Valve
    } else if has(&["shower"]) {
        Kind::Shower
    } else if has(&["bathtub", "tub_", ".tub", "_tub", "bath tub"]) {
        Kind::Tub
    } else if has(&["grab"]) {
        Kind::Grab
    } else if has(&["mirror"]) {
        Kind::Mirror
    } else if has(&["sink", "lavatory", "vanity"]) {
        Kind::Sink
    } else {
        return None;
    })
}

/// A cabinet read from the floor's JSON.
struct Cab {
    id: Id,
    kind: String,
    origin: Point,
    u: Point,
    v: Point,
    width: f64,
    depth: f64,
    height: f64,
    elevation: f64,
    front_over: f64,
    corner: String,
    has_top: bool,
    label: String,
    appliance: Option<String>,
    /// The cabinet footprint.
    poly: Vec<Point>,
    /// The countertop footprint with its overhangs.
    top: Vec<Point>,
    /// Cutouts as (kind, local bounds x0, y0, x1, y1, plan outline).
    cutouts: Vec<(String, [f64; 4], Vec<Point>)>,
}

impl Cab {
    fn base_like(&self) -> bool {
        matches!(self.kind.as_str(), "Base" | "CornerBase" | "BlindBase")
    }

    fn tall(&self) -> bool {
        self.kind == "FullHeight"
    }

    fn wall_unit(&self) -> bool {
        matches!(self.kind.as_str(), "Wall" | "CornerWall" | "BlindWall")
    }

    /// Stands on the floor in the way.
    fn blocks(&self) -> bool {
        self.base_like() || self.tall()
    }

    fn local(&self, x: f64, y: f64) -> Point {
        self.origin + self.u * x + self.v * y
    }

    /// Top of the countertop above the floor.
    fn counter_top(&self) -> f64 {
        self.elevation + self.height
    }
}

fn rect(p: Point, u: Point, v: Point, w: f64, d: f64) -> Vec<Point> {
    vec![p, p + u * w, p + u * w + v * d, p + v * d]
}

fn cab_from_json(v: &Value) -> Option<Cab> {
    let num = |o: &Value, k: &str| o.get(k).and_then(Value::as_f64);
    let kind = v.get("kind")?.as_str()?.to_string();
    let origin: Point = serde_json::from_value(v.get("position")?.clone()).ok()?;
    let (width, depth) = (num(v, "width")?, num(v, "depth")?);
    let angle = num(v, "angle").unwrap_or(0.0);
    let u = Point::new(angle.cos(), angle.sin());
    let vv = u.perp();
    let top = v.get("countertop").filter(|c| c.is_object());
    let over = |k: &str| top.and_then(|c| num(c, k)).unwrap_or(0.0);
    let (front, sides, back) = (over("overhang_front"), over("overhang_sides"), over("overhang_back"));
    let corner = top
        .and_then(|c| c.get("corner"))
        .and_then(Value::as_str)
        .unwrap_or("None")
        .to_string();
    let mut cab = Cab {
        id: v.get("id")?.as_u64()?,
        kind,
        origin,
        u,
        v: vv,
        width,
        depth,
        height: num(v, "height").unwrap_or(36.0),
        elevation: num(v, "elevation").unwrap_or(0.0),
        front_over: front,
        corner,
        has_top: top.is_some(),
        label: v
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase(),
        appliance: v
            .get("appliance")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_lowercase),
        poly: rect(origin, u, vv, width, depth),
        top: rect(
            origin - u * sides - vv * back,
            u,
            vv,
            width + 2.0 * sides,
            depth + front + back,
        ),
        cutouts: Vec::new(),
    };
    if let Some(list) = v.get("cutouts").and_then(Value::as_array) {
        for c in list {
            let kind = c.get("kind").and_then(Value::as_str).unwrap_or("").to_string();
            let Some(pts) = c
                .get("outline")
                .and_then(|o| serde_json::from_value::<Vec<Point>>(o.clone()).ok())
            else {
                continue;
            };
            if pts.len() < 3 {
                continue;
            }
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for p in &pts {
                x0 = x0.min(p.x);
                y0 = y0.min(p.y);
                x1 = x1.max(p.x);
                y1 = y1.max(p.y);
            }
            let plan = pts.iter().map(|p| cab.local(p.x, p.y)).collect();
            cab.cutouts.push((kind, [x0, y0, x1, y1], plan));
        }
    }
    Some(cab)
}

/// A sink, appliance or fixture of the plan.
struct Item {
    kind: Kind,
    label: String,
    /// Centre of the footprint.
    center: Point,
    /// Middle of the back edge, where the counter probes for landing start.
    back: Point,
    /// Middle of the front edge (the counter front for a sink in a counter).
    front: Point,
    u: Point,
    v: Point,
    width: f64,
    depth: f64,
    poly: Vec<Point>,
    target: Target,
    elevation: f64,
    /// Floor to the top of the item or of the counter it sits in.
    top: f64,
    /// Comes from a symbol that stands on the floor in the way.
    obstacle: bool,
    /// The cabinet that holds it (a cutout or an appliance bay).
    cabinet: Option<Id>,
}

impl Item {
    fn label(&self) -> String {
        if self.label.trim().is_empty() {
            self.kind.name().to_string()
        } else {
            self.label.clone()
        }
    }
}

/// What the rules measure against.
struct Env<'a> {
    ctx: &'a Ctx<'a>,
    cabs: Vec<Cab>,
    items: Vec<Item>,
    walls: Vec<(Id, Vec<Point>)>,
    devs: Vec<Dev>,
}

impl<'a> Env<'a> {
    fn new(ctx: &'a Ctx<'a>) -> Self {
        let cabs: Vec<Cab> = ctx.floor.cabinets.iter().filter_map(cab_from_json).collect();
        let mut items = Vec::new();
        for s in ctx
            .floor
            .symbols
            .iter()
            .filter(|s| s.image.is_none() && s.owner.is_none())
        {
            let Some(kind) = classify(&format!("{} {}", s.catalog_id, s.label)) else {
                continue;
            };
            let a = s.angle.to_radians();
            let u = Point::new(a.cos(), a.sin());
            let v = u.perp();
            let hw = s.width * 0.5;
            items.push(Item {
                kind,
                label: s.label.clone(),
                center: s.position + v * (s.depth * 0.5),
                back: s.position,
                front: s.position + v * s.depth,
                u,
                v,
                width: s.width,
                depth: s.depth,
                poly: vec![
                    s.position - u * hw,
                    s.position + u * hw,
                    s.position + u * hw + v * s.depth,
                    s.position - u * hw + v * s.depth,
                ],
                target: Target::Symbol(s.id),
                elevation: s.elevation,
                top: s.elevation + s.height,
                obstacle: s.elevation < 12.0
                    && !matches!(kind, Kind::Hood | Kind::Fan | Kind::Mirror | Kind::Grab | Kind::Valve),
                cabinet: None,
            });
        }
        // Cutouts and appliance bays of cabinets, unless a symbol already
        // stands for the same thing.
        for c in &cabs {
            for (k, [x0, y0, x1, y1], outline) in &c.cutouts {
                let kind = match k.as_str() {
                    "Sink" => Kind::Sink,
                    "Cooktop" => Kind::Cooktop,
                    _ => continue,
                };
                let center = c.local((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                if items
                    .iter()
                    .any(|i| i.kind == kind && i.center.dist(center) < 18.0)
                {
                    continue;
                }
                items.push(Item {
                    kind,
                    label: String::new(),
                    center,
                    back: c.local((x0 + x1) / 2.0, *y0),
                    front: c.local((x0 + x1) / 2.0, c.depth + c.front_over),
                    u: c.u,
                    v: c.v,
                    width: x1 - x0,
                    depth: y1 - y0,
                    poly: outline.clone(),
                    target: Target::Cabinet(c.id),
                    elevation: c.elevation,
                    top: c.counter_top(),
                    obstacle: false,
                    cabinet: Some(c.id),
                });
            }
            let named = c
                .appliance
                .as_deref()
                .and_then(classify)
                .or_else(|| classify(&c.label).filter(|k| *k == Kind::Trash));
            if let Some(kind) = named {
                if items
                    .iter()
                    .any(|i| i.kind == kind && i.center.dist(c.local(c.width / 2.0, c.depth / 2.0)) < 18.0)
                {
                    continue;
                }
                items.push(Item {
                    kind,
                    label: String::new(),
                    center: c.local(c.width / 2.0, c.depth / 2.0),
                    back: c.local(c.width / 2.0, 0.0),
                    front: c.local(c.width / 2.0, c.depth),
                    u: c.u,
                    v: c.v,
                    width: c.width,
                    depth: c.depth,
                    poly: c.poly.clone(),
                    target: Target::Cabinet(c.id),
                    elevation: c.elevation,
                    top: c.counter_top(),
                    obstacle: false,
                    cabinet: Some(c.id),
                });
            }
        }
        let walls = ctx
            .floor
            .walls
            .iter()
            .filter(|w| !w.flags.invisible && !w.flags.room_divider && !w.flags.railing)
            .map(|w| (w.id, w.plan_polygon()))
            .collect();
        Self {
            ctx,
            cabs,
            items,
            walls,
            devs: devices(ctx),
        }
    }

    fn in_room(&self, i: usize, p: Point) -> bool {
        point_in_polygon(p, &self.ctx.rooms[i].polygon)
    }

    fn room_items(&self, i: usize, pred: impl Fn(Kind) -> bool) -> Vec<&Item> {
        self.items
            .iter()
            .filter(|it| pred(it.kind) && self.in_room(i, it.center))
            .collect()
    }

    fn room_cabs(&self, i: usize) -> Vec<&Cab> {
        self.cabs
            .iter()
            .filter(|c| self.in_room(i, c.local(c.width / 2.0, c.depth / 2.0)))
            .collect()
    }

    /// Distance along the ray to the nearest thing that stands in the way
    /// (not the item `skip_item` or the cabinet `skip_cab`, doorways are
    /// open), up to `max`.
    fn ray(&self, from: Point, dir: Point, max: f64, skip_item: Option<usize>, skip_cab: Option<Id>) -> Option<f64> {
        let end = from + dir * max;
        let hit = |poly: &[Point]| {
            (0..poly.len())
                .filter_map(|k| {
                    segment_intersection(from, end, poly[k], poly[(k + 1) % poly.len()])
                        .map(|(t, _)| t * max)
                })
                .min_by(f64::total_cmp)
        };
        let mut best: Option<f64> = None;
        let mut note = |d: f64| best = Some(best.map_or(d, |b| b.min(d)));
        for c in self.cabs.iter().filter(|c| c.blocks() && Some(c.id) != skip_cab) {
            if let Some(d) = hit(&c.poly) {
                note(d);
            }
        }
        for (k, it) in self.items.iter().enumerate() {
            if it.obstacle && Some(k) != skip_item {
                if let Some(d) = hit(&it.poly) {
                    note(d);
                }
            }
        }
        for (wid, poly) in &self.walls {
            let Some(d) = hit(poly) else {
                continue;
            };
            let at = from + dir * d;
            let in_doorway = self
                .ctx
                .ops
                .iter()
                .filter(|o| o.op.kind == OpeningKind::Door && o.op.wall_id == *wid)
                .filter_map(|o| Some((o.op, o.wall?)))
                .any(|(op, w)| {
                    let along = at.sub(w.start).dot(w.direction());
                    along >= op.start_offset() && along <= op.end_offset()
                });
            if !in_doorway {
                note(d);
            }
        }
        best
    }

    /// Clear distance in front of `it` (three rays across its width).
    fn clear_front(&self, k: usize, max: f64) -> Option<f64> {
        let it = &self.items[k];
        [-0.35, 0.0, 0.35]
            .into_iter()
            .filter_map(|f| {
                let from = it.front + it.u * (it.width * f) + it.v * 0.05;
                self.ray(from, it.v, max, Some(k), it.cabinet).map(|d| d + 0.05)
            })
            .min_by(f64::total_cmp)
    }

    /// Is `p` on a usable countertop (not under a sink, a cooktop or an
    /// appliance standing in the run)?
    fn counter_at(&self, p: Point) -> bool {
        // A dishwasher stands under the counter, so its width counts.
        if self
            .items
            .iter()
            .any(|i| i.kind == Kind::Dishwasher && i.cabinet.is_none() && point_in_polygon(p, &i.poly))
        {
            return true;
        }
        let on_top = self
            .cabs
            .iter()
            .any(|c| c.base_like() && c.has_top && point_in_polygon(p, &c.top));
        if !on_top {
            return false;
        }
        let cut = self
            .cabs
            .iter()
            .flat_map(|c| c.cutouts.iter())
            .any(|(_, _, o)| point_in_polygon(p, o));
        // A range or refrigerator interrupts the counter.
        let appliance = self.items.iter().any(|i| {
            i.cabinet.is_none()
                && i.obstacle
                && point_in_polygon(p, &i.poly)
        });
        !cut && !appliance
    }

    /// Length of unbroken counter beside `it` on `side` (+1 or -1 along its
    /// width), measured from its edge on a line near the back, up to `max`.
    fn landing(&self, it: &Item, side: f64, max: f64) -> f64 {
        let y = (it.depth * 0.5).min(10.0);
        let start = it.back + it.v * y + it.u * (side * it.width * 0.5);
        // Cells of a quarter inch, sampled at their middles, so a counter
        // that ends exactly on a whole inch measures a whole inch.
        let mut run = 0.0;
        let mut gap = 0.0;
        let mut k = 0;
        loop {
            let edge = f64::from(k + 1) * 0.25;
            if edge > max + 1e-9 {
                break;
            }
            if self.counter_at(start + it.u * (side * (edge - 0.125))) {
                run = edge;
                gap = 0.0;
            } else {
                gap += 0.25;
                if gap > 1.5 {
                    break;
                }
            }
            k += 1;
        }
        run
    }

    /// Is there a counter 15" wide across from `it` within `reach` inches?
    fn counter_across(&self, it: &Item, reach: f64, wide: f64) -> bool {
        let mut t = 6.0;
        while t <= reach {
            let p = it.front + it.v * t;
            if self.counter_at(p + it.u * (wide / 2.0 - 0.5))
                && self.counter_at(p - it.u * (wide / 2.0 - 0.5))
            {
                return true;
            }
            t += 1.0;
        }
        false
    }

    /// A wall within `d` of `p`?
    fn wall_near(&self, p: Point, d: f64) -> bool {
        self.ctx.floor.walls.iter().any(|w| {
            !w.flags.invisible
                && dist_to_segment(p, w.start, w.end) - w.thickness * 0.5 <= d
        })
    }

    fn receptacles(&self) -> Vec<&Dev> {
        self.devs
            .iter()
            .filter(|d| matches!(d.kind.as_str(), "Outlet110" | "Outlet110Quad" | "Gfci" | "OutletFloor"))
            .collect()
    }
}

/// Distance from a point to a polygon (0 inside it).
fn point_poly_dist(p: Point, poly: &[Point]) -> f64 {
    if point_in_polygon(p, poly) {
        return 0.0;
    }
    (0..poly.len())
        .map(|k| dist_to_segment(p, poly[k], poly[(k + 1) % poly.len()]))
        .fold(f64::MAX, f64::min)
}

/// Distance between two polygons (0 when they touch or overlap).
fn poly_dist(a: &[Point], b: &[Point]) -> f64 {
    if polygons_overlap(a, b) {
        return 0.0;
    }
    let one = |p: &[Point], q: &[Point]| {
        p.iter()
            .flat_map(|&pt| (0..q.len()).map(move |k| dist_to_segment(pt, q[k], q[(k + 1) % q.len()])))
            .fold(f64::MAX, f64::min)
    };
    one(a, b).min(one(b, a))
}

// ----- results -----

/// What a run found and what it could test.
#[derive(Default)]
pub(crate) struct Eval {
    pub findings: Vec<Finding>,
    /// (room index, rule) pairs the run was able to test.
    pub checked: Vec<(usize, &'static str)>,
    /// (room index, rule, message) of every failed one.
    pub failed: Vec<(usize, &'static str, String)>,
}

impl Eval {
    fn tested(&mut self, room: usize, rule: &'static str) {
        if !self.checked.contains(&(room, rule)) {
            self.checked.push((room, rule));
        }
    }

    fn fail(&mut self, room: usize, f: Finding) {
        self.tested(room, f.rule);
        self.failed.push((room, f.rule, f.message.clone()));
        self.findings.push(f);
    }
}

/// Runs every NKBA rule on the floor of `ctx`.
pub(crate) fn evaluate(ctx: &Ctx) -> Eval {
    let env = Env::new(ctx);
    let mut ev = Eval::default();
    for i in 0..ctx.rooms.len() {
        if ctx.types[i].contains("kitchen") {
            kitchen(&env, i, &mut ev);
        } else if ctx.is_bath(i) {
            bath(&env, i, &mut ev);
        }
    }
    ev
}

/// Adds the findings of the NKBA rules.
pub(crate) fn run(ctx: &Ctx, out: &mut Vec<Finding>) {
    out.extend(evaluate(ctx).findings);
}

fn feet(inches: f64) -> String {
    fmt_ft_in(inches)
}

/// Clear width of the opening of a door, inches.
fn clear_width(oi: &OpInfo) -> f64 {
    let w = oi.op.width;
    match oi.op.style {
        OpeningStyle::Doorway | OpeningStyle::Barn | OpeningStyle::PassThrough => w,
        OpeningStyle::Pocket => w - 2.0,
        OpeningStyle::Sliding | OpeningStyle::Bifold => w / 2.0,
        _ => w - 4.0,
    }
}

fn is_closet(t: &str) -> bool {
    t.contains("closet") || t.contains("pantry")
}

/// NKBA 32" clear at the doors into a kitchen or bathroom.
fn entry_doors(env: &Env, i: usize, rule: &'static str, area: Area, ev: &mut Eval) {
    let ctx = env.ctx;
    for oi in ctx
        .ops
        .iter()
        .filter(|o| o.op.kind == OpeningKind::Door && o.touches(i))
    {
        if oi.other_side(i).is_some_and(|r| is_closet(&ctx.types[r])) {
            continue;
        }
        ev.tested(i, rule);
        let clear = clear_width(oi);
        if clear < 32.0 - EPS {
            let mut f = finding(
                rule,
                Severity::Warning,
                format!(
                    "A door into {} is {:.0}\" wide, {:.0}\" clear; {} needs 32\" clear.",
                    ctx.name(i),
                    oi.op.width,
                    clear,
                    area.word().to_lowercase()
                ),
                "Use a 3'-0\" door (32\" clear) or a cased opening.",
            )
            .on(Target::Opening(oi.op.id));
            if let Some(c) = oi.center {
                f = f.at(c);
            }
            ev.fail(i, f);
        }
    }
}

// ----- the kitchen -----

fn kitchen(env: &Env, i: usize, ev: &mut Eval) {
    let ctx = env.ctx;
    let name = ctx.name(i);
    entry_doors(env, i, K_DOOR, K, ev);
    door_swings(env, i, &name, ev);
    work_triangle(env, i, &name, ev);
    two_cook_aisle(env, i, &name, ev);
    seating(env, i, &name, ev);
    landings(env, i, &name, ev);
    dishwashers(env, i, &name, ev);
    cooking_surface(env, i, &name, ev);
    waste(env, i, &name, ev);
    counters(env, i, &name, ev);
    kitchen_power(env, i, &name, ev);
}

/// A door must not swing into a cabinet or an appliance.
fn door_swings(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    for oi in env.ctx.ops.iter().filter(|o| {
        o.op.kind == OpeningKind::Door
            && o.swing_room() == Some(i)
            && matches!(o.op.style, OpeningStyle::Hinged | OpeningStyle::DoubleDoor)
    }) {
        let Some(mut arc) = swing_polygon(oi) else {
            continue;
        };
        ev.tested(i, K_SWING);
        let c = plan_core::geometry::polygon_centroid(&arc);
        for p in &mut arc {
            *p = c + (*p - c) * 0.9;
        }
        let hit = env
            .cabs
            .iter()
            .filter(|cb| cb.blocks() && env.in_room(i, cb.local(cb.width / 2.0, cb.depth / 2.0)))
            .find(|cb| polygons_overlap(&arc, &cb.poly))
            .map(|cb| ("cabinet".to_string(), cb.id))
            .or_else(|| {
                env.items
                    .iter()
                    .filter(|it| it.obstacle && it.cabinet.is_none() && env.in_room(i, it.center))
                    .find(|it| polygons_overlap(&arc, &it.poly))
                    .map(|it| (it.label(), 0))
            });
        if let Some((what, _)) = hit {
            let mut f = finding(
                K_SWING,
                Severity::Warning,
                format!("The swing of door {} reaches a {what} in {name}.", oi.op.id),
                "Flip the swing, use a pocket or outswing door, or move the cabinet or appliance.",
            )
            .on(Target::Opening(oi.op.id));
            if let Some(c) = oi.center {
                f = f.at(c);
            }
            ev.fail(i, f);
        }
    }
}

/// Length of the segment `a`-`b` that lies inside cabinets or appliances
/// other than the ones that hold the end points.
fn blocked_length(env: &Env, a: Point, b: Point, skip: &[Option<Id>], skip_items: &[usize]) -> f64 {
    let len = a.dist(b);
    if len < EPS {
        return 0.0;
    }
    let dir = (b - a) * (1.0 / len);
    let mut blocked = 0.0;
    let mut t = 0.25;
    while t < len {
        let p = a + dir * t;
        let inside = env
            .cabs
            .iter()
            .filter(|c| c.blocks() && !skip.contains(&Some(c.id)))
            .any(|c| point_in_polygon(p, &c.poly))
            || env
                .items
                .iter()
                .enumerate()
                .filter(|(k, it)| it.obstacle && !skip_items.contains(k))
                .any(|(_, it)| point_in_polygon(p, &it.poly));
        if inside {
            blocked += 0.25;
        }
        t += 0.25;
    }
    blocked
}

/// NKBA work triangle: each leg 4' to 9', the three legs 13' to 26', and no
/// cabinet or appliance cutting a leg by more than 12".
fn work_triangle(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    let idx: Vec<usize> = env
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| env.in_room(i, it.center))
        .map(|(k, _)| k)
        .collect();
    let pick = |pred: &dyn Fn(Kind) -> bool| {
        idx.iter()
            .copied()
            .filter(|&k| pred(env.items[k].kind))
            .max_by(|&a, &b| {
                let area = |k: usize| env.items[k].width * env.items[k].depth;
                area(a).total_cmp(&area(b))
            })
    };
    let (Some(s), Some(c), Some(f)) = (
        pick(&|k| k == Kind::Sink),
        pick(&|k| k.is_cook()),
        pick(&|k| k == Kind::Fridge),
    ) else {
        return;
    };
    ev.tested(i, K_TRIANGLE);
    let pt = |k: usize| env.items[k].front + env.items[k].v * 1.0;
    let legs = [
        ("sink to cooktop", s, c),
        ("cooktop to refrigerator", c, f),
        ("refrigerator to sink", f, s),
    ];
    let mut why = Vec::new();
    let mut total = 0.0;
    for (label, a, b) in legs {
        let d = pt(a).dist(pt(b));
        total += d;
        if d < 48.0 - EPS {
            why.push(format!("the {label} leg is {} (at least 4')", feet(d)));
        } else if d > 108.0 + EPS {
            why.push(format!("the {label} leg is {} (at most 9')", feet(d)));
        }
        let cut = blocked_length(
            env,
            pt(a),
            pt(b),
            &[env.items[a].cabinet, env.items[b].cabinet],
            &[a, b],
        );
        if cut > 12.0 + EPS {
            why.push(format!(
                "a cabinet or appliance cuts the {label} leg by {cut:.0}\" (at most 12\")"
            ));
        }
    }
    if total > 312.0 + EPS {
        why.push(format!("the legs total {} (at most 26')", feet(total)));
    } else if total < 156.0 - EPS {
        why.push(format!("the legs total {} (at least 13')", feet(total)));
    }
    if !why.is_empty() {
        let centre = Point::new(
            (pt(s).x + pt(c).x + pt(f).x) / 3.0,
            (pt(s).y + pt(c).y + pt(f).y) / 3.0,
        );
        ev.fail(
            i,
            finding(
                K_TRIANGLE,
                Severity::Warning,
                format!("In {name}: {}.", why.join("; ")),
                "Move the sink, cooktop or refrigerator so the legs are 4' to 9' and total 13' to 26', or open the leg through an island.",
            )
            .at(centre)
            .on(env.items[s].target),
        );
    }
}

/// Two cooks need a 48" work aisle (the generic 36" and 42" aisles are the
/// "NKBA kitchen aisle width" rule).
fn two_cook_aisle(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    let sinks = env.room_items(i, |k| k == Kind::Sink).len();
    if sinks < 2 {
        return;
    }
    let mut worst: Option<(f64, usize)> = None;
    for (k, it) in env.items.iter().enumerate() {
        let work = matches!(it.kind, Kind::Sink | Kind::Cooktop | Kind::Range | Kind::Fridge | Kind::Dishwasher);
        if !work || !env.in_room(i, it.center) {
            continue;
        }
        let from = it.front + it.v * 0.05;
        if let Some(d) = env.ray(from, it.v, 60.0, Some(k), it.cabinet) {
            if worst.is_none_or(|(w, _)| d < w) {
                worst = Some((d, k));
            }
        }
    }
    let Some((d, k)) = worst else {
        return;
    };
    ev.tested(i, K_AISLE2);
    if d < 48.0 - EPS {
        ev.fail(
            i,
            finding(
                K_AISLE2,
                Severity::Info,
                format!("{name} has {sinks} sinks, so two cooks work in it, but the work aisle in front of the {} is {d:.0}\".", env.items[k].label()),
                "Open the aisle to 48\" where two cooks work.",
            )
            .at(env.items[k].front + env.items[k].v * (d / 2.0))
            .on(env.items[k].target),
        );
    }
}

/// Seating: the knee space under the overhang and the clearance behind it.
fn seating(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    for c in env
        .room_cabs(i)
        .into_iter()
        .filter(|c| c.base_like() && c.has_top && c.front_over >= 6.0)
    {
        let need = if c.height >= 40.0 {
            12.0
        } else if c.height <= 32.0 {
            18.0
        } else {
            15.0
        };
        ev.tested(i, K_SEAT_KNEE);
        let mid = c.local(c.width / 2.0, c.depth + c.front_over);
        if c.front_over < need - EPS {
            ev.fail(
                i,
                finding(
                    K_SEAT_KNEE,
                    Severity::Warning,
                    format!(
                        "Seating at a {:.0}\" high counter in {name} has a {:.0}\" overhang; {need:.0}\" of knee space is needed.",
                        c.height, c.front_over
                    ),
                    "Extend the countertop overhang over the seating side.",
                )
                .at(mid)
                .on(Target::Cabinet(c.id)),
            );
        }
        let clear = [0.2, 0.5, 0.8]
            .into_iter()
            .filter_map(|f| {
                let from = c.local(c.width * f, c.depth + c.front_over + 0.05);
                env.ray(from, c.v, 44.0, None, Some(c.id))
            })
            .min_by(f64::total_cmp);
        ev.tested(i, K_SEAT_CLEAR);
        if let Some(d) = clear {
            if d < 36.0 - EPS {
                ev.fail(
                    i,
                    finding(
                        K_SEAT_CLEAR,
                        Severity::Warning,
                        format!("Behind the seating at the counter in {name} there is {d:.0}\" clear; 36\" is the least."),
                        "Move the seating or what stands behind it so 36\" (44\" where people walk behind) is clear.",
                    )
                    .at(mid)
                    .on(Target::Cabinet(c.id)),
                );
            } else if d < 44.0 - EPS {
                ev.fail(
                    i,
                    finding(
                        K_SEAT_CLEAR,
                        Severity::Info,
                        format!("Behind the seating at the counter in {name} there is {d:.0}\" clear; 44\" is needed where people walk behind the seat."),
                        "Open the space to 44\" if this is a path.",
                    )
                    .at(mid)
                    .on(Target::Cabinet(c.id)),
                );
            }
        }
    }
}

fn landing_finding(
    env: &Env,
    it: &Item,
    rule: &'static str,
    name: &str,
    what: &str,
    left: f64,
    right: f64,
    need: (f64, f64),
) -> Finding {
    let _ = env;
    finding(
        rule,
        Severity::Warning,
        format!(
            "The {what} in {name} has {left:.0}\" of counter on one side and {right:.0}\" on the other; {:.0}\" and {:.0}\" are needed.",
            need.0, need.1
        ),
        "Add counter beside it, at the same height.",
    )
    .at(it.front)
    .on(it.target)
}

/// Landing areas at the sink, cooking surface, refrigerator, oven and
/// microwave.
fn landings(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    let sinks = env.room_items(i, |k| k == Kind::Sink);
    let primary = sinks
        .iter()
        .map(|s| s.width * s.depth)
        .fold(0.0_f64, f64::max);
    for it in &sinks {
        let (l, r) = (env.landing(it, -1.0, 60.0), env.landing(it, 1.0, 60.0));
        let is_primary = it.width * it.depth >= primary - EPS;
        let need = if is_primary { (24.0, 18.0) } else { (18.0, 3.0) };
        ev.tested(i, K_SINK);
        let (hi, lo) = (l.max(r), l.min(r));
        if hi < need.0 - EPS || lo < need.1 - EPS {
            let what = if is_primary { "sink" } else { "second sink" };
            ev.fail(i, landing_finding(env, it, K_SINK, name, what, l, r, need));
        }
    }
    for it in env.room_items(i, Kind::is_cook) {
        let (l, r) = (env.landing(it, -1.0, 40.0), env.landing(it, 1.0, 40.0));
        ev.tested(i, K_COOKTOP);
        if l.max(r) < 15.0 - EPS || l.min(r) < 12.0 - EPS {
            ev.fail(i, landing_finding(env, it, K_COOKTOP, name, it.kind.name(), l, r, (15.0, 12.0)));
        }
    }
    for (kind, rule) in [
        (Kind::Fridge, K_FRIDGE),
        (Kind::Oven, K_OVEN),
        (Kind::Microwave, K_MICRO),
    ] {
        for it in env.room_items(i, |k| k == kind) {
            ev.tested(i, rule);
            let (l, r) = (env.landing(it, -1.0, 30.0), env.landing(it, 1.0, 30.0));
            if l.max(r) >= 15.0 - EPS || env.counter_across(it, 48.0, 15.0) {
                continue;
            }
            ev.fail(
                i,
                finding(
                    rule,
                    Severity::Warning,
                    format!(
                        "The {} in {name} has {:.0}\" of counter beside it and none across within 48\"; 15\" is needed.",
                        it.kind.name(),
                        l.max(r)
                    ),
                    "Add 15\" of counter beside the appliance, or across from it within 48\".",
                )
                .at(it.front)
                .on(it.target),
            );
        }
    }
}

/// The dishwasher is within 36" of the sink and has 21" of standing space.
fn dishwashers(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    let sinks = env.room_items(i, |k| k == Kind::Sink);
    for (k, it) in env.items.iter().enumerate() {
        if it.kind != Kind::Dishwasher || !env.in_room(i, it.center) {
            continue;
        }
        ev.tested(i, K_DISHWASHER);
        let mut why = Vec::new();
        if let Some(d) = sinks
            .iter()
            .map(|s| poly_dist(&it.poly, &s.poly))
            .min_by(f64::total_cmp)
        {
            if d > 36.0 + EPS {
                why.push(format!("{d:.0}\" from the sink (at most 36\")"));
            }
        }
        if let Some(d) = env.clear_front(k, 21.0) {
            if d < 21.0 - EPS {
                why.push(format!("{d:.0}\" of standing space in front (21\" is needed)"));
            }
        }
        if !why.is_empty() {
            ev.fail(
                i,
                finding(
                    K_DISHWASHER,
                    Severity::Warning,
                    format!("The dishwasher in {name} is {}.", why.join(" and has ")),
                    "Put the dishwasher within 36\" of the sink and keep 21\" clear in front of it.",
                )
                .at(it.front)
                .on(it.target),
            );
        }
    }
}

/// Clearance above the cooking surface, windows beside it and ventilation.
fn cooking_surface(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    let cooks = env.room_items(i, Kind::is_cook);
    for it in &cooks {
        // Cabinets and microwaves above the cooking surface.
        let mut above: Vec<(f64, String, Target)> = env
            .cabs
            .iter()
            .filter(|c| c.wall_unit() && polygons_overlap(&c.poly, &it.poly))
            .map(|c| (c.elevation - it.top, "wall cabinet".to_string(), Target::Cabinet(c.id)))
            .collect();
        above.extend(
            env.items
                .iter()
                .filter(|m| m.kind == Kind::Microwave && m.elevation > 30.0 && polygons_overlap(&m.poly, &it.poly))
                .map(|m| (m.elevation - it.top, "microwave".to_string(), m.target)),
        );
        for (gap, what, target) in above {
            ev.tested(i, K_COOK_CLEAR);
            if gap < 24.0 - EPS {
                ev.fail(
                    i,
                    finding(
                        K_COOK_CLEAR,
                        Severity::Warning,
                        format!("A {what} is {gap:.0}\" above the {} in {name}; 24\" is the least, and only with a protected surface.", it.kind.name()),
                        "Raise the cabinet to at least 30\" (24\" with a noncombustible, protected underside).",
                    )
                    .at(it.center)
                    .on(target),
                );
            } else if gap < 30.0 - EPS {
                ev.fail(
                    i,
                    finding(
                        K_COOK_CLEAR,
                        Severity::Info,
                        format!("A {what} is {gap:.0}\" above the {} in {name}; that is allowed only with a protected, noncombustible surface (30\" if unprotected).", it.kind.name()),
                        "Raise it to 30\" or protect its underside.",
                    )
                    .at(it.center)
                    .on(target),
                );
            }
        }
        // An operable window over or beside the cooking surface.
        for oi in env.ctx.ops.iter().filter(|o| {
            o.op.kind == OpeningKind::Window && o.op.sill_height < 66.0 && o.center.is_some()
        }) {
            let (Some(w), Some(c)) = (oi.wall, oi.center) else {
                continue;
            };
            if dist_to_segment(it.back, w.start, w.end) - w.thickness * 0.5 > 12.0 {
                continue;
            }
            let along = (it.back - c).dot(w.direction()).abs();
            ev.tested(i, K_COOK_SAFE);
            if along <= it.width / 2.0 + oi.op.width / 2.0 + 12.0 {
                ev.fail(
                    i,
                    finding(
                        K_COOK_SAFE,
                        Severity::Info,
                        format!("Window {} is over or within 12\" of the {} in {name}.", oi.op.id, it.kind.name()),
                        "Avoid an operable window over the cooking surface; curtains near a flame are a hazard.",
                    )
                    .at(c)
                    .on(Target::Opening(oi.op.id)),
                );
            }
        }
        // Ventilation.
        ev.tested(i, K_VENT);
        let vent = env.items.iter().find(|h| {
            (h.kind == Kind::Hood && (poly_dist(&h.poly, &it.poly) <= 12.0 || h.center.dist(it.center) <= 36.0))
                || (h.kind == Kind::Fan && h.center.dist(it.center) <= 60.0)
                || (h.kind == Kind::Microwave && h.elevation > 30.0 && polygons_overlap(&h.poly, &it.poly))
        });
        match vent {
            None => ev.fail(
                i,
                finding(
                    K_VENT,
                    Severity::Warning,
                    format!("The {} in {name} has no hood or exhaust fan over it.", it.kind.name()),
                    "Add a range hood (150 cfm or more, ducted to the outdoors) over the cooking surface.",
                )
                .at(it.center)
                .on(it.target),
            ),
            Some(h) if h.kind == Kind::Hood && h.width < it.width - 1.0 - EPS => ev.fail(
                i,
                finding(
                    K_VENT,
                    Severity::Info,
                    format!("The hood over the {} in {name} is {:.0}\" wide, narrower than the {:.0}\" cooking surface.", it.kind.name(), h.width, it.width),
                    "Use a hood at least as wide as the cooking surface.",
                )
                .at(h.center)
                .on(h.target),
            ),
            Some(_) => {}
        }
    }
}

fn waste(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    let has_cabinets = env.room_cabs(i).iter().any(|c| c.base_like());
    if !has_cabinets && env.room_items(i, |_| true).is_empty() {
        return;
    }
    ev.tested(i, K_WASTE);
    if env.room_items(i, |k| k == Kind::Trash).is_empty() {
        ev.fail(
            i,
            finding(
                K_WASTE,
                Severity::Info,
                format!("{name} has no waste receptacle."),
                "Place a pull-out waste and recycling cabinet near the sink.",
            )
            .at(ev_centre(env, i))
            .on(Target::Room(i)),
        );
    }
}

fn ev_centre(env: &Env, i: usize) -> Point {
    env.ctx.rooms[i].centroid
}

/// Counter frontage and the corners of island countertops.
fn counters(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    let tops: Vec<&Cab> = env
        .room_cabs(i)
        .into_iter()
        .filter(|c| c.base_like() && c.has_top)
        .collect();
    if tops.is_empty() {
        return;
    }
    let mut frontage = 0.0;
    for c in tops.iter().filter(|c| c.kind != "CornerBase") {
        let cut: f64 = c
            .cutouts
            .iter()
            .filter(|(k, _, _)| k == "Sink" || k == "Cooktop")
            .map(|(_, [x0, _, x1, _], _)| x1 - x0)
            .sum();
        frontage += (c.width - cut).max(0.0);
    }
    let area = env.ctx.rooms[i].area_sq_in / 144.0;
    let need = if area >= 150.0 { 198.0 } else { 158.0 };
    ev.tested(i, K_FRONTAGE);
    if frontage < need - EPS {
        ev.fail(
            i,
            finding(
                K_FRONTAGE,
                Severity::Warning,
                format!(
                    "{name} ({area:.0} sq ft) has {frontage:.0}\" of counter frontage, not counting corners, sinks and cooktops; {need:.0}\" is needed."
                ),
                "Add base cabinets with a countertop (at least 15\" deep).",
            )
            .at(ev_centre(env, i))
            .on(Target::Room(i)),
        );
    }
    // Island and peninsula corners.
    let sharp: Vec<&&Cab> = tops
        .iter()
        .filter(|c| c.corner == "None" && c.width >= 12.0 && !env.wall_near(c.local(c.width / 2.0, c.depth / 2.0), c.depth / 2.0 + 6.0))
        .collect();
    let islands = tops
        .iter()
        .filter(|c| !env.wall_near(c.local(c.width / 2.0, c.depth / 2.0), c.depth / 2.0 + 6.0))
        .count();
    if islands > 0 {
        ev.tested(i, K_EDGES);
    }
    if let Some(c) = sharp.first() {
        ev.fail(
            i,
            finding(
                K_EDGES,
                Severity::Info,
                format!(
                    "{} island or peninsula countertop{} in {name} ha{} square corners.",
                    sharp.len(),
                    if sharp.len() == 1 { "" } else { "s" },
                    if sharp.len() == 1 { "s" } else { "ve" }
                ),
                "Clip or round the exposed corners (Countertop > Corner).",
            )
            .at(c.local(c.width / 2.0, c.depth / 2.0))
            .on(Target::Cabinet(c.id)),
        );
    }
}

/// Receptacles along the counters and GFCI protection near the sink.
fn kitchen_power(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    if env.devs.is_empty() {
        return;
    }
    let recs = env.receptacles();
    let tops: Vec<&Cab> = env
        .room_cabs(i)
        .into_iter()
        .filter(|c| c.base_like() && c.has_top && c.width >= 12.0)
        .collect();
    let mut gaps: Vec<(Point, Id, bool)> = Vec::new();
    for c in &tops {
        let centre = c.local(c.width / 2.0, c.depth / 2.0);
        let free = !env.wall_near(centre, c.depth / 2.0 + 6.0);
        if free {
            // An island or peninsula of 12" x 24" or more needs one.
            if c.width * c.depth >= 12.0 * 24.0 - EPS
                && !recs
                    .iter()
                    .any(|d| point_poly_dist(d.at, &c.top) <= 24.0 + 6.0)
            {
                gaps.push((centre, c.id, true));
            }
            continue;
        }
        let steps = (c.width / 6.0).ceil().max(1.0) as usize;
        for k in 0..=steps {
            let p = c.local(c.width * k as f64 / steps as f64, 0.0);
            if !recs.iter().any(|d| d.at.dist(p) <= 24.0 + 6.0) {
                gaps.push((p, c.id, false));
                break;
            }
        }
    }
    if !tops.is_empty() {
        ev.tested(i, K_RECEPT);
    }
    if let Some((p, id, island)) = gaps.first() {
        ev.fail(
            i,
            finding(
                K_RECEPT,
                Severity::Warning,
                if *island {
                    format!("The island or peninsula counter in {name} has no receptacle.")
                } else {
                    format!("The counter in {name} has no receptacle within 24\" of this point ({} places in all).", gaps.len())
                },
                "Add a GFCI receptacle on the backsplash, no point of the counter more than 24\" from one.",
            )
            .at(*p)
            .on(Target::Cabinet(*id)),
        );
    }
    // GFCI near the sink: a plain receptacle within 6'.
    let sinks = env.room_items(i, |k| k == Kind::Sink);
    if sinks.is_empty() {
        return;
    }
    ev.tested(i, K_GFCI);
    let plain: Vec<&Dev> = env
        .devs
        .iter()
        .filter(|d| matches!(d.kind.as_str(), "Outlet110" | "Outlet110Quad"))
        .filter(|d| sinks.iter().any(|s| point_poly_dist(d.at, &s.poly) <= 72.0))
        .collect();
    if let Some(d) = plain.first() {
        ev.fail(
            i,
            finding(
                K_GFCI,
                Severity::Warning,
                format!(
                    "{name} has {} receptacle{} within 6' of the sink without GFCI protection.",
                    plain.len(),
                    if plain.len() == 1 { "" } else { "s" }
                ),
                "Use GFCI receptacles within 6' of a sink.",
            )
            .at(d.at)
            .on(Target::Room(i)),
        );
    }
}

// ----- the bathroom -----

fn bath(env: &Env, i: usize, ev: &mut Eval) {
    let name = env.ctx.name(i);
    entry_doors(env, i, B_DOOR, B, ev);
    clear_floor(env, i, &name, ev);
    toilets(env, i, &name, ev);
    showers(env, i, &name, ev);
    grab_bars(env, i, &name, ev);
    lavatories(env, i, &name, ev);
    ventilation(env, i, &name, ev);
    bath_power(env, i, &name, ev);
}

/// 30" of clear floor in front of the lavatory, tub and shower.
fn clear_floor(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    for (k, it) in env.items.iter().enumerate() {
        if !matches!(it.kind, Kind::Sink | Kind::Tub | Kind::Shower) || !env.in_room(i, it.center) {
            continue;
        }
        ev.tested(i, B_CLEAR);
        if let Some(d) = env.clear_front(k, 30.0) {
            if d < 30.0 - EPS {
                ev.fail(
                    i,
                    finding(
                        B_CLEAR,
                        Severity::Warning,
                        format!("The {} in {name} has {d:.0}\" of clear floor in front of it; 30\" is needed.", it.kind.name()),
                        "Move the fixture or the obstruction so 30\" is clear in front.",
                    )
                    .at(it.front + it.v * (d / 2.0))
                    .on(it.target),
                );
            }
        }
    }
}

/// Toilet: 16" from its centre to a side obstruction and 30" in front (the
/// 15" and 21" minimums are the IRC rule).
fn toilets(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    for (k, it) in env.items.iter().enumerate() {
        if it.kind != Kind::Toilet || !env.in_room(i, it.center) {
            continue;
        }
        ev.tested(i, B_TOILET);
        let side = |dir: Point| {
            [0.25, 0.5, 0.75]
                .into_iter()
                .filter_map(|f| env.ray(it.back + it.v * (it.depth * f), dir, 16.0, Some(k), None))
                .min_by(f64::total_cmp)
        };
        let mut why = Vec::new();
        for (word, d) in [("left", side(it.u * -1.0)), ("right", side(it.u))] {
            if let Some(d) = d.filter(|d| (15.0 - EPS..16.0 - EPS).contains(d)) {
                why.push(format!("{d:.1}\" to the {word}, 16\" is advised"));
            }
        }
        if let Some(d) = env
            .clear_front(k, 30.0)
            .filter(|d| (21.0 - EPS..30.0 - EPS).contains(d))
        {
            why.push(format!("{d:.0}\" clear in front, 30\" is advised"));
        }
        if !why.is_empty() {
            ev.fail(
                i,
                finding(
                    B_TOILET,
                    Severity::Info,
                    format!("The toilet in {name} meets the code minimum but has {}.", why.join(" and ")),
                    "Allow 16\" from the toilet centreline to each side obstruction and 30\" in front.",
                )
                .at(it.center)
                .on(it.target),
            );
        }
    }
}

/// Shower size and controls.
fn showers(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    for it in env.room_items(i, |k| k == Kind::Shower) {
        ev.tested(i, B_SHOWER);
        let small = it.width.min(it.depth);
        if small < 36.0 - EPS {
            ev.fail(
                i,
                finding(
                    B_SHOWER,
                    Severity::Warning,
                    format!("The shower in {name} is {:.0}\" x {:.0}\"; 36\" x 36\" inside is the NKBA minimum.", it.width, it.depth),
                    "Enlarge the shower to at least 36\" x 36\".",
                )
                .at(it.center)
                .on(it.target),
            );
        }
        // Controls: a valve symbol is measured, otherwise a reminder.
        let valve = env
            .items
            .iter()
            .find(|v| v.kind == Kind::Valve && v.center.dist(it.center) <= it.width.max(it.depth) + 36.0);
        match valve {
            Some(v) => {
                ev.tested(i, B_CONTROLS);
                
                let h = (v.elevation + v.top) / 2.0;
                if !(38.0 - EPS..=48.0 + EPS).contains(&h) {
                    ev.fail(
                        i,
                        finding(
                            B_CONTROLS,
                            Severity::Warning,
                            format!("The shower controls in {name} are {h:.0}\" above the floor; 38\" to 48\" is the range."),
                            "Set the valve 38\" to 48\" above the floor.",
                        )
                        .at(v.center)
                        .on(v.target),
                    );
                }
            }
            None => {
                ev.tested(i, B_CONTROLS);
                ev.fail(
                    i,
                    finding(
                        B_CONTROLS,
                        Severity::Info,
                        format!("The shower in {name} has no control placed; set the controls 38\" to 48\" above the floor."),
                        "Place a shower valve symbol, or note the control height.",
                    )
                    .at(it.center)
                    .on(it.target),
                );
            }
        }
    }
}

/// Blocking for grab bars at the toilet, tub and shower: a grab bar symbol
/// within 36" of the fixture stands for it.
fn grab_bars(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    let fixtures = env.room_items(i, |k| matches!(k, Kind::Toilet | Kind::Tub | Kind::Shower));
    if fixtures.is_empty() {
        return;
    }
    ev.tested(i, B_GRAB);
    let bare: Vec<&&Item> = fixtures
        .iter()
        .filter(|f| {
            !env.items
                .iter()
                .any(|gb| gb.kind == Kind::Grab && poly_dist(&gb.poly, &f.poly) <= 36.0)
        })
        .collect();
    if let Some(first) = bare.first() {
        let list: Vec<&str> = bare.iter().map(|f| f.kind.name()).collect();
        ev.fail(
            i,
            finding(
                B_GRAB,
                Severity::Info,
                format!("{name}: no grab bar or blocking is shown at the {}.", list.join(", the ")),
                "Frame blocking in the walls at the toilet, tub and shower, 33\" to 36\" above the floor, or place grab bars.",
            )
            .at(first.center)
            .on(first.target),
        );
    }
}

/// Lavatory height, spacing, mirror.
fn lavatories(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    let sinks: Vec<(usize, &Item)> = env
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| it.kind == Kind::Sink && env.in_room(i, it.center))
        .collect();
    if sinks.is_empty() {
        return;
    }
    for (_, it) in &sinks {
        let known = it.cabinet.is_some() || it.top >= 20.0;
        if !known {
            continue;
        }
        ev.tested(i, B_LAV_HEIGHT);
        if !(32.0 - EPS..=43.0 + EPS).contains(&it.top) {
            ev.fail(
                i,
                finding(
                    B_LAV_HEIGHT,
                    Severity::Info,
                    format!("The lavatory top in {name} is {:.0}\" above the floor; 32\" to 43\" is the range.", it.top),
                    "Set the vanity height between 32\" and 43\".",
                )
                .at(it.center)
                .on(it.target),
            );
        }
    }
    // Spacing between basins and from a side obstruction.
    ev.tested(i, B_LAV_SPACE);
    let mut why = Vec::new();
    let mut at = None;
    for (a, (_, s)) in sinks.iter().enumerate() {
        for (_, t) in sinks.iter().skip(a + 1) {
            let d = s.center.dist(t.center);
            if d < 30.0 - EPS {
                why.push(format!("two basins are {d:.0}\" apart centre to centre (30\" is needed)"));
                at = at.or(Some(s.target));
            }
        }
    }
    for (k, s) in &sinks {
        for (word, dir) in [("left", s.u * -1.0), ("right", s.u)] {
            let from = s.center + s.v * 0.05;
            let hit = env.ray(from, dir, 15.0, Some(*k), s.cabinet).filter(|d| *d < 15.0 - EPS);
            if let Some(d) = hit {
                why.push(format!("a basin is {d:.0}\" from the {word} obstruction (15\" is needed)"));
                at = at.or(Some(s.target));
            }
        }
    }
    if !why.is_empty() {
        let first = sinks[0].1;
        ev.fail(
            i,
            finding(
                B_LAV_SPACE,
                Severity::Warning,
                format!("In {name}, {}.", why.join("; ")),
                "Space double basins 30\" centre to centre and keep each 15\" from a wall or fixture.",
            )
            .at(first.center)
            .on(at.unwrap_or(first.target)),
        );
    }
    ev.tested(i, B_MIRROR);
    if !env.items.iter().any(|m| m.kind == Kind::Mirror && env.in_room(i, m.center)) {
        ev.fail(
            i,
            finding(
                B_MIRROR,
                Severity::Info,
                format!("{name} has a lavatory but no mirror."),
                "Place a mirror over the lavatory.",
            )
            .at(sinks[0].1.center)
            .on(sinks[0].1.target),
        );
    }
}

/// An exhaust fan, or a window of 3 sq ft or more.
fn ventilation(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    ev.tested(i, B_VENT);
    if env.items.iter().any(|f| f.kind == Kind::Fan && env.in_room(i, f.center)) {
        return;
    }
    let windows: Vec<f64> = env
        .ctx
        .exterior_openings(i, OpeningKind::Window)
        .iter()
        .map(|o| o.op.width * o.op.height / 144.0)
        .collect();
    let best = windows.iter().copied().fold(0.0_f64, f64::max);
    if best >= 3.0 - EPS {
        return;
    }
    let message = if windows.is_empty() {
        format!("{name} has no exhaust fan and no window.")
    } else {
        format!("{name} has no exhaust fan and its window is {best:.1} sq ft; 3 sq ft is needed.")
    };
    ev.fail(
        i,
        finding(
            B_VENT,
            Severity::Warning,
            message,
            "Add an exhaust fan (50 cfm intermittent) ducted to the outdoors, or a window of 3 sq ft with half of it openable.",
        )
        .at(env.ctx.rooms[i].centroid)
        .on(Target::Room(i)),
    );
}

/// A receptacle at each lavatory, and GFCI near the water.
fn bath_power(env: &Env, i: usize, name: &str, ev: &mut Eval) {
    if env.devs.is_empty() {
        return;
    }
    let recs = env.receptacles();
    let sinks = env.room_items(i, |k| k == Kind::Sink);
    for s in &sinks {
        ev.tested(i, B_RECEPT);
        // The basin's edge to the receptacle on the wall; the wall's own
        // thickness is left out of the 36".
        if !recs.iter().any(|d| point_poly_dist(d.at, &s.poly) <= 36.0 + 3.0) {
            ev.fail(
                i,
                finding(
                    B_RECEPT,
                    Severity::Warning,
                    format!("No receptacle is within 36\" of the lavatory in {name}."),
                    "Add a GFCI receptacle on the wall within 36\" of the basin.",
                )
                .at(s.center)
                .on(s.target),
            );
        }
    }
    let wet = env.room_items(i, |k| matches!(k, Kind::Sink | Kind::Tub | Kind::Shower));
    if wet.is_empty() {
        return;
    }
    ev.tested(i, B_GFCI);
    let plain: Vec<&Dev> = env
        .devs
        .iter()
        .filter(|d| matches!(d.kind.as_str(), "Outlet110" | "Outlet110Quad"))
        .filter(|d| env.in_room(i, d.at) || crate::rules_mep::in_room(env.ctx, i, d.at))
        .filter(|d| wet.iter().any(|w| point_poly_dist(d.at, &w.poly) <= 72.0))
        .collect();
    if let Some(d) = plain.first() {
        ev.fail(
            i,
            finding(
                B_GFCI,
                Severity::Warning,
                format!(
                    "{name} has {} receptacle{} near the water without GFCI protection.",
                    plain.len(),
                    if plain.len() == 1 { "" } else { "s" }
                ),
                "Use GFCI receptacles in the bathroom.",
            )
            .at(d.at)
            .on(Target::Room(i)),
        );
    }
}

// ----- the report -----

/// How a guideline came out in a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NkbaStatus {
    /// Tested, and the plan meets it.
    Met,
    /// Tested, and the plan does not meet it.
    NotMet,
    /// The plan has nothing the guideline applies to.
    NotInPlan,
    /// The plan holds no data to test it.
    NotChecked,
    /// The rule is switched off in Plan Check Settings.
    SwitchedOff,
}

impl NkbaStatus {
    /// Report text.
    pub fn text(self) -> &'static str {
        match self {
            NkbaStatus::Met => "Met",
            NkbaStatus::NotMet => "Not met",
            NkbaStatus::NotInPlan => "Not in the plan",
            NkbaStatus::NotChecked => "Not checked",
            NkbaStatus::SwitchedOff => "Switched off",
        }
    }
}

/// One line of the Kitchen and Bath report.
#[derive(Debug, Clone, PartialEq)]
pub struct NkbaRow {
    /// The room, e.g. `Kitchen`.
    pub room: String,
    /// `Kitchen` or `Bathroom`.
    pub area: &'static str,
    /// The rule id, or the topic of an unchecked guideline.
    pub guideline: String,
    pub requirement: String,
    pub status: NkbaStatus,
    /// The finding's message when not met.
    pub detail: String,
}

/// The Kitchen and Bath report: every NKBA guideline in every kitchen and
/// bathroom of a floor.
#[derive(Debug, Clone, PartialEq)]
pub struct NkbaReport {
    pub rows: Vec<NkbaRow>,
}

impl NkbaReport {
    /// `(met, not met)` over the tested guidelines.
    pub fn counts(&self) -> (usize, usize) {
        let n = |s| self.rows.iter().filter(|r| r.status == s).count();
        (n(NkbaStatus::Met), n(NkbaStatus::NotMet))
    }

    /// A one-line summary, e.g. `NKBA: 14 met, 3 not met in 2 rooms`.
    pub fn summary(&self) -> String {
        let (met, not_met) = self.counts();
        let mut rooms: Vec<&str> = self.rows.iter().map(|r| r.room.as_str()).collect();
        rooms.sort_unstable();
        rooms.dedup();
        format!(
            "NKBA: {met} met, {not_met} not met in {} room{}",
            rooms.len(),
            if rooms.len() == 1 { "" } else { "s" }
        )
    }

    /// The report as a table for the layout, PDF and spreadsheet outputs.
    pub fn table(&self, floor_name: &str) -> ReportTable {
        ReportTable {
            title: format!("Kitchen and Bath Report - {floor_name}"),
            columns: ["Room", "Guideline", "Requirement", "Result", "Detail"]
                .iter()
                .map(|c| (*c).to_string())
                .collect(),
            rows: self
                .rows
                .iter()
                .map(|r| {
                    vec![
                        r.room.clone(),
                        r.guideline.clone(),
                        r.requirement.clone(),
                        r.status.text().to_string(),
                        r.detail.clone(),
                    ]
                })
                .collect(),
        }
    }

    /// The report as Markdown: a count line, then a table per room.
    pub fn markdown(&self) -> String {
        let mut md = String::from("# Kitchen and Bath Report\n\n");
        if self.rows.is_empty() {
            md.push_str("No kitchen or bathroom was found. Name the rooms Kitchen and Bath.\n");
            return md;
        }
        md.push_str(&format!("{}\n", self.summary()));
        let mut last = "";
        for r in &self.rows {
            if r.room != last {
                md.push_str(&format!("\n## {} ({})\n\n| Guideline | Result | Detail |\n|---|---|---|\n", r.room, r.area));
                last = &r.room;
            }
            md.push_str(&format!("| {} | {} | {} |\n", r.guideline, r.status.text(), r.detail));
        }
        md
    }
}

/// Tests every kitchen and bathroom of `floor` against the NKBA guidelines
/// (rules switched off in the plan's Plan Check Settings are listed as such).
///
/// # Panics
/// Panics if `floor` is not a valid floor index.
pub fn nkba_report(
    project: &Project,
    floor: usize,
    rooms: &[Room],
    room_types: &[(usize, String)],
    stairs: &[Stair],
) -> NkbaReport {
    let settings = CheckSettings::load(project);
    let opts: &CheckOptions = &settings.options;
    let ctx = Ctx::new(&project.floors[floor], rooms, room_types, stairs, opts).with_project(project);
    let ev = evaluate(&ctx);
    let mut rows = Vec::new();
    for i in 0..rooms.len() {
        let area = if ctx.types[i].contains("kitchen") {
            Area::Kitchen
        } else if ctx.is_bath(i) {
            Area::Bath
        } else {
            continue;
        };
        for gl in GUIDELINES.iter().filter(|gl| gl.area == area) {
            let failed: Vec<&String> = ev
                .failed
                .iter()
                .filter(|(r, id, _)| *r == i && *id == gl.id)
                .map(|(_, _, m)| m)
                .collect();
            let (status, detail) = if !settings.is_enabled(gl.id) {
                (NkbaStatus::SwitchedOff, String::new())
            } else if !failed.is_empty() {
                (
                    NkbaStatus::NotMet,
                    failed.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" "),
                )
            } else if ev.checked.contains(&(i, gl.id)) {
                (NkbaStatus::Met, String::new())
            } else {
                (NkbaStatus::NotInPlan, String::new())
            };
            rows.push(NkbaRow {
                room: ctx.name(i),
                area: area.word(),
                guideline: gl.id.to_string(),
                requirement: gl.requirement.to_string(),
                status,
                detail,
            });
        }
        for (a, topic, text) in NOT_CHECKABLE.iter().filter(|(a, _, _)| *a == area) {
            rows.push(NkbaRow {
                room: ctx.name(i),
                area: a.word(),
                guideline: (*topic).to_string(),
                requirement: (*text).to_string(),
                status: NkbaStatus::NotChecked,
                detail: String::new(),
            });
        }
    }
    NkbaReport { rows }
}
