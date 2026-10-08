//! Plumbing fixture and kitchen rules: water closet clearances, shower and
//! tub size, a door swinging into a fixture, and the kitchen aisle and counter
//! depth.
//!
//! Fixtures are the placed library symbols whose catalog id names a toilet, a
//! tub or a shower. Cabinets are read from the floor's JSON (`kind`,
//! `position`, `angle`, `width`, `depth`, `countertop`) so plan-check needs no
//! dependency on `plan-cabinets`.

use plan_core::geometry::{point_in_polygon, segment_intersection};
use plan_core::{OpeningKind, PlacedSymbol, Point};
use serde_json::Value;

use crate::ctx::Ctx;
use crate::geom::polygons_overlap;
use crate::rules::swing_polygon;
use crate::{finding, Finding, Severity, Target};

const EPS: f64 = 1e-6;
/// Longest kitchen aisle looked for, inches.
const AISLE_REACH: f64 = 60.0;

/// Run every rule of this file.
pub(crate) fn run(ctx: &Ctx, out: &mut Vec<Finding>) {
    let cabs = cabinets(ctx);
    let fixtures = fixtures(ctx);
    if !fixtures.is_empty() {
        let obstacles = obstacles(ctx, &fixtures, &cabs);
        water_closets(ctx, &fixtures, &obstacles, out);
        shower_and_tub_size(ctx, &fixtures, out);
        door_swing_into_fixture(ctx, &fixtures, out);
    }
    kitchen(ctx, &cabs, out);
}

// ----- geometry -----

/// Rectangle with its back-left corner at `p`, `w` along `u` and `d` along `v`.
fn rect(p: Point, u: Point, v: Point, w: f64, d: f64) -> Vec<Point> {
    vec![p, p + u * w, p + u * w + v * d, p + v * d]
}

/// Distance along the ray `origin + dir * t` (t up to `max`) to the nearest
/// edge of any polygon in `polys`, skipping the polygon `skip`.
fn ray_hit(
    origin: Point,
    dir: Point,
    max: f64,
    polys: &[(usize, Vec<Point>)],
    skip: usize,
) -> Option<f64> {
    let end = origin + dir * max;
    polys
        .iter()
        .filter(|(k, _)| *k != skip)
        .flat_map(|(_, poly)| (0..poly.len()).map(move |i| (poly[i], poly[(i + 1) % poly.len()])))
        .filter_map(|(a, b)| segment_intersection(origin, end, a, b).map(|(t, _)| t * max))
        .min_by(|a, b| a.total_cmp(b))
}

// ----- fixtures -----

#[derive(Clone, Copy, PartialEq, Eq)]
enum FixKind {
    WaterCloset,
    Tub,
    Shower,
}

struct Fixture<'a> {
    sym: &'a PlacedSymbol,
    kind: FixKind,
    poly: Vec<Point>,
    /// Unit vector along the width.
    u: Point,
    /// Unit vector toward the front.
    v: Point,
}

impl Fixture<'_> {
    fn label(&self) -> String {
        if !self.sym.label.trim().is_empty() {
            return self.sym.label.clone();
        }
        match self.kind {
            FixKind::WaterCloset => "Toilet",
            FixKind::Tub => "Tub",
            FixKind::Shower => "Shower",
        }
        .to_string()
    }
}

fn classify(catalog_id: &str) -> Option<FixKind> {
    let id = catalog_id.to_lowercase();
    if id.contains("toilet") || id.contains("water_closet") {
        Some(FixKind::WaterCloset)
    } else if id.contains("shower") {
        Some(FixKind::Shower)
    } else if id.contains("bathtub") || id.contains(".tub") || id.contains("_tub") {
        Some(FixKind::Tub)
    } else {
        None
    }
}

fn fixtures<'a>(ctx: &Ctx<'a>) -> Vec<Fixture<'a>> {
    ctx.floor
        .symbols
        .iter()
        .filter(|s| s.image.is_none() && s.owner.is_none())
        .filter_map(|s| {
            let kind = classify(&s.catalog_id)?;
            let a = s.angle.to_radians();
            let u = Point::new(a.cos(), a.sin());
            Some(Fixture {
                sym: s,
                kind,
                poly: s.footprint().to_vec(),
                u,
                v: u.perp(),
            })
        })
        .collect()
}

/// Everything a fixture's clearance can run into: visible walls, other
/// fixtures and kitchen or vanity cabinets. The key is the index of the
/// fixture the polygon belongs to (fixtures first), so it can be skipped.
fn obstacles(ctx: &Ctx, fixtures: &[Fixture], cabs: &[Cab]) -> Vec<(usize, Vec<Point>)> {
    let mut out: Vec<(usize, Vec<Point>)> = fixtures
        .iter()
        .enumerate()
        .map(|(i, f)| (i, f.poly.clone()))
        .collect();
    let base = fixtures.len();
    out.extend(
        ctx.floor
            .walls
            .iter()
            .filter(|w| !w.flags.invisible && !w.flags.room_divider && !w.flags.railing)
            .enumerate()
            .map(|(k, w)| (base + k, w.plan_polygon())),
    );
    let base = base + ctx.floor.walls.len();
    out.extend(
        cabs.iter()
            .enumerate()
            .filter(|(_, c)| c.blocks)
            .map(|(k, c)| (base + k, c.poly.clone())),
    );
    out
}

/// IRC R307.1 and P2705.1: a water closet needs 15" from its centerline to a
/// side wall or fixture (a 30" wide space) and 21" clear in front.
fn water_closets(
    ctx: &Ctx,
    fixtures: &[Fixture],
    obstacles: &[(usize, Vec<Point>)],
    out: &mut Vec<Finding>,
) {
    let o = ctx.opts;
    for (i, f) in fixtures.iter().enumerate() {
        if f.kind != FixKind::WaterCloset {
            continue;
        }
        let s = f.sym;
        let side = |dir: Point| {
            [0.25, 0.5, 0.75]
                .into_iter()
                .filter_map(|k| {
                    ray_hit(
                        s.position + f.v * (s.depth * k),
                        dir,
                        o.wc_side_clear,
                        obstacles,
                        i,
                    )
                })
                .min_by(|a, b| a.total_cmp(b))
        };
        let front = [-0.35, 0.0, 0.35]
            .into_iter()
            .filter_map(|k| {
                let from = s.position + f.v * s.depth + f.u * (s.width * k);
                ray_hit(from, f.v, o.wc_front_clear, obstacles, i)
            })
            .min_by(|a, b| a.total_cmp(b));
        let mut problems = Vec::new();
        for (name, d) in [("left", side(f.u * -1.0)), ("right", side(f.u))] {
            if let Some(d) = d.filter(|d| *d < o.wc_side_clear - EPS) {
                problems.push(format!(
                    "{d:.0}\" to the {name} (needs {:.0}\")",
                    o.wc_side_clear
                ));
            }
        }
        if let Some(d) = front.filter(|d| *d < o.wc_front_clear - EPS) {
            problems.push(format!(
                "{d:.0}\" clear in front (needs {:.0}\")",
                o.wc_front_clear
            ));
        }
        if !problems.is_empty() {
            out.push(
                finding(
                    "IRC R307.1 water closet clearance",
                    Severity::Warning,
                    format!("{} has {}.", f.label(), problems.join(" and ")),
                    "Move the toilet or the obstruction: 15\" from the toilet centerline to a wall or fixture on each side, 21\" clear in front.",
                )
                .at(s.position + f.v * (s.depth * 0.5))
                .on(Target::Symbol(s.id)),
            );
        }
    }
}

/// IRC P2708.1: a shower compartment is at least 30" across and 900 sq in;
/// a tub or shower space is at least 30" wide.
fn shower_and_tub_size(ctx: &Ctx, fixtures: &[Fixture], out: &mut Vec<Finding>) {
    let o = ctx.opts;
    for f in fixtures.iter().filter(|f| f.kind != FixKind::WaterCloset) {
        let s = f.sym;
        let small = s.width.min(s.depth);
        let area = s.width * s.depth;
        let mut why = Vec::new();
        if small < o.shower_min_dim - EPS {
            why.push(format!(
                "is {small:.0}\" across, needs {:.0}\"",
                o.shower_min_dim
            ));
        }
        if f.kind == FixKind::Shower && area < o.shower_min_area - EPS {
            why.push(format!(
                "has {area:.0} sq in of floor, needs {:.0}",
                o.shower_min_area
            ));
        }
        if !why.is_empty() {
            out.push(
                finding(
                    "IRC P2708.1 shower and tub size",
                    Severity::Warning,
                    format!("{} {}.", f.label(), why.join(" and ")),
                    "Enlarge the compartment to at least 30\" x 30\" (900 sq in for a shower).",
                )
                .at(s.position + f.v * (s.depth * 0.5))
                .on(Target::Symbol(s.id)),
            );
        }
    }
}

/// A door into a bathroom must not swing across a fixture.
fn door_swing_into_fixture(ctx: &Ctx, fixtures: &[Fixture], out: &mut Vec<Finding>) {
    for oi in ctx.ops.iter().filter(|o| o.op.kind == OpeningKind::Door) {
        let in_bath = [oi.left, oi.right]
            .into_iter()
            .flatten()
            .any(|r| ctx.is_bath(r));
        if !in_bath {
            continue;
        }
        let Some(mut arc) = swing_polygon(oi) else {
            continue;
        };
        // Shrink the fan a little so a fixture touching the wall beside the
        // door does not count.
        let c = plan_core::geometry::polygon_centroid(&arc);
        for p in &mut arc {
            *p = c + (*p - c) * 0.9;
        }
        if let Some(f) = fixtures.iter().find(|f| polygons_overlap(&arc, &f.poly)) {
            out.push(
                finding(
                    "IRC R307.1 door swing into fixture",
                    Severity::Warning,
                    format!(
                        "The swing of door {} reaches the {}.",
                        oi.op.id,
                        f.label().to_lowercase()
                    ),
                    "Flip the swing, use a pocket or outswing door, or move the fixture.",
                )
                .at(oi.center.unwrap_or(c))
                .on(Target::Opening(oi.op.id)),
            );
        }
    }
}

// ----- cabinets and the kitchen -----

struct Cab {
    id: u64,
    kind: String,
    poly: Vec<Point>,
    u: Point,
    /// Unit vector toward the front.
    v: Point,
    origin: Point,
    width: f64,
    depth: f64,
    /// Depth of the countertop with its front and back overhangs.
    counter_depth: Option<f64>,
    /// Stands on the floor in the way (base, tall and corner units).
    blocks: bool,
    /// Has a plain rectangular front a person can face.
    has_front: bool,
}

fn cabinets(ctx: &Ctx) -> Vec<Cab> {
    ctx.floor
        .cabinets
        .iter()
        .filter_map(cab_from_json)
        .collect()
}

fn cab_from_json(v: &Value) -> Option<Cab> {
    let num = |k: &str| v.get(k).and_then(Value::as_f64);
    let kind = v.get("kind")?.as_str()?.to_string();
    let origin: Point = serde_json::from_value(v.get("position")?.clone()).ok()?;
    let (width, depth) = (num("width")?, num("depth")?);
    let angle = num("angle").unwrap_or(0.0);
    let u = Point::new(angle.cos(), angle.sin());
    let vv = u.perp();
    let counter_depth = v.get("countertop").filter(|c| c.is_object()).map(|c| {
        let g = |k: &str| c.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        depth + g("overhang_front") + g("overhang_back")
    });
    let stands = matches!(
        kind.as_str(),
        "Base" | "FullHeight" | "CornerBase" | "BlindBase"
    );
    Some(Cab {
        id: v.get("id")?.as_u64()?,
        has_front: matches!(kind.as_str(), "Base" | "FullHeight" | "BlindBase"),
        blocks: stands,
        poly: rect(origin, u, vv, width, depth),
        kind,
        u,
        v: vv,
        origin,
        width,
        depth,
        counter_depth,
    })
}

/// NKBA kitchen planning: the walkway between a run of cabinets and what
/// faces it is at least 36" (work aisle 42" with one cook), and the counter
/// is at least 24" deep.
fn kitchen(ctx: &Ctx, cabs: &[Cab], out: &mut Vec<Finding>) {
    let o = ctx.opts;
    let walls: Vec<(plan_core::Id, Vec<Point>)> = ctx
        .floor
        .walls
        .iter()
        .filter(|w| !w.flags.invisible && !w.flags.room_divider && !w.flags.railing)
        .map(|w| (w.id, w.plan_polygon()))
        .collect();
    for (i, room) in ctx.rooms.iter().enumerate() {
        if !ctx.types[i].contains("kitchen") {
            continue;
        }
        let name = ctx.name(i);
        let mut worst: Option<(f64, &Cab, Point)> = None;
        for c in cabs.iter().filter(|c| c.has_front) {
            let mid = c.origin + c.u * (c.width * 0.5) + c.v * (c.depth + 6.0);
            if !point_in_polygon(mid, &room.polygon) {
                continue;
            }
            for k in [0.1, 0.3, 0.5, 0.7, 0.9] {
                let from = c.origin + c.u * (c.width * k) + c.v * (c.depth + 0.01);
                if let Some(d) = aisle_at(ctx, cabs, &walls, c, from) {
                    if worst.is_none_or(|(w, _, _)| d < w) {
                        worst = Some((d, c, from));
                    }
                }
            }
        }
        let aisle = worst.and_then(|(d, c, from)| {
            let sev = if d < o.kitchen_walkway - EPS {
                Severity::Warning
            } else if d < o.kitchen_work_aisle - EPS {
                Severity::Info
            } else {
                return None;
            };
            Some((d, c, from, sev))
        });
        if let Some((d, c, from, sev)) = aisle {
            out.push(
                finding(
                    "NKBA kitchen aisle width",
                    sev,
                    format!(
                        "The aisle in {name} is {d:.0}\" wide at its tightest; walkways need {:.0}\" and a work aisle {:.0}\".",
                        o.kitchen_walkway, o.kitchen_work_aisle
                    ),
                    "Move the island or the opposite run back, or use shallower cabinets, to open the aisle to 42\".",
                )
                .at(from)
                .on(Target::Cabinet(c.id)),
            );
        }
        for c in cabs.iter().filter(|c| c.kind == "Base" && c.depth >= 12.0) {
            let Some(depth) = c.counter_depth else {
                continue;
            };
            let centre = c.origin + c.u * (c.width * 0.5) + c.v * (c.depth * 0.5);
            if depth < o.counter_depth_min - EPS && point_in_polygon(centre, &room.polygon) {
                out.push(
                    finding(
                        "NKBA kitchen counter depth",
                        Severity::Info,
                        format!(
                            "A base cabinet in {name} has a {depth:.1}\" deep counter, under {:.0}\".",
                            o.counter_depth_min
                        ),
                        "Use a 24\" deep base cabinet with a 1\" front overhang (25\" counter).",
                    )
                    .at(centre)
                    .on(Target::Cabinet(c.id)),
                );
            }
        }
    }
}

/// Clear distance from `from` straight out of the front of `cab`, to the next
/// cabinet or wall (a doorway is not a wall).
fn aisle_at(
    ctx: &Ctx,
    cabs: &[Cab],
    walls: &[(plan_core::Id, Vec<Point>)],
    cab: &Cab,
    from: Point,
) -> Option<f64> {
    let end = from + cab.v * AISLE_REACH;
    let hit = |poly: &[Point]| {
        (0..poly.len())
            .filter_map(|i| {
                segment_intersection(from, end, poly[i], poly[(i + 1) % poly.len()])
                    .map(|(t, _)| t * AISLE_REACH)
            })
            .min_by(|a, b| a.total_cmp(b))
    };
    let mut best: Option<f64> = None;
    let mut note = |d: f64| best = Some(best.map_or(d, |b| b.min(d)));
    for other in cabs.iter().filter(|c| c.blocks && c.id != cab.id) {
        if let Some(d) = hit(&other.poly) {
            note(d);
        }
    }
    for (wid, poly) in walls {
        let Some(d) = hit(poly) else {
            continue;
        };
        let at = from + cab.v * d;
        let in_doorway = ctx
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
