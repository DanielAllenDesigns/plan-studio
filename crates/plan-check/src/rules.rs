//! The individual checks. Each rule appends [`Finding`]s to `out`.

use plan_core::geometry::{dist_to_segment, polygon_area};
use plan_core::units::fmt_ft_in;
use plan_core::{OpeningKind, Point, Wall, WallKind};
use plan_stairs::{footprint, solve, StairShape};

use crate::ctx::{is_habitable, is_hall, Ctx, OpInfo};
use crate::geom::{min_dimension, polygons_overlap};
use crate::{finding, Finding, Severity, Target};

/// Egress opening area allowed on the grade floor, sq ft (R310.2.1 exception).
const GRADE_FLOOR_EGRESS_AREA: f64 = 5.0;
/// Glazing must be at least this share of the floor area (R303.1).
const LIGHT_FLOOR_RATIO: f64 = 0.08;
/// Window sill height under which fall protection is flagged.
const FALL_SILL: f64 = 18.0;
/// Floor height above grade at which fall protection applies (R312.2).
const FALL_FLOOR_HEIGHT: f64 = 72.0;
/// Walls shorter than this are probably drawing mistakes.
const TINY_WALL: f64 = 6.0;
/// Two walls whose centerlines are this close are duplicates.
const SAME_LINE: f64 = 1.0;
/// Free-end test distance around a wall end.
const JOIN_TOL: f64 = 3.0;
/// Maximum rise between landings (R311.7.3), 12'-7".
const MAX_FLIGHT_RISE: f64 = 147.0;
/// Tolerance for float comparisons against limits.
const EPS: f64 = 1e-6;
/// Note appended to room-size findings.
const CENTERLINE_NOTE: &str = "measured to wall centerlines; the clear size is smaller";

fn sq_ft(w: f64, h: f64) -> f64 {
    w * h / 144.0
}

/// Rule 1, IRC R304.1 and R304.2: habitable rooms need at least 70 sq ft and
/// 7'-0" in any horizontal dimension. Kitchens are exempt in the IRC, so they
/// are reported as Info only.
pub(crate) fn habitable_room_size(ctx: &Ctx, out: &mut Vec<Finding>) {
    for (i, room) in ctx.rooms.iter().enumerate() {
        let t = &ctx.types[i];
        if !is_habitable(t) {
            continue;
        }
        let sev = if t.contains("kitchen") {
            Severity::Info
        } else {
            Severity::Error
        };
        let name = ctx.name(i);
        let area = room.area_sq_ft();
        if area < ctx.opts.min_room_area - EPS {
            out.push(
                finding(
                    "IRC R304.1 minimum room area",
                    sev,
                    format!(
                        "{name} is {area:.1} sq ft, under the {:.0} sq ft minimum ({CENTERLINE_NOTE}).",
                        ctx.opts.min_room_area
                    ),
                    "Enlarge the room or change its use so it is not a habitable room.",
                )
                .at(room.centroid)
                .on(Target::Room(i)),
            );
        }
        let dim = min_dimension(&room.polygon);
        if dim < ctx.opts.min_room_dim - EPS {
            out.push(
                finding(
                    "IRC R304.2 minimum room dimension",
                    sev,
                    format!(
                        "{name} is only {} in its narrow direction, under {} ({CENTERLINE_NOTE}).",
                        fmt_ft_in(dim),
                        fmt_ft_in(ctx.opts.min_room_dim)
                    ),
                    "Widen the room to at least 7'-0\" in every horizontal dimension.",
                )
                .at(room.centroid)
                .on(Target::Room(i)),
            );
        }
    }
}

/// Rule 2, IRC R310.2 emergency escape and rescue openings: every bedroom
/// needs an exterior door or a window with at least 5.7 sq ft (5.0 on the
/// grade floor), 24" clear height, 20" clear width and a sill no higher
/// than 44". Opening sizes are used as the clear sizes.
pub(crate) fn bedroom_egress(ctx: &Ctx, out: &mut Vec<Finding>) {
    let o = ctx.opts;
    let min_area = if ctx.grade_floor() {
        o.egress_min_area.min(GRADE_FLOOR_EGRESS_AREA)
    } else {
        o.egress_min_area
    };
    for (i, room) in ctx.rooms.iter().enumerate() {
        if !ctx.is_bedroom(i) {
            continue;
        }
        if !ctx.exterior_openings(i, OpeningKind::Door).is_empty() {
            continue;
        }
        let windows = ctx.exterior_openings(i, OpeningKind::Window);
        let name = ctx.name(i);
        let best = windows.iter().max_by(|a, b| {
            let area = |w: &&OpInfo| w.op.width * w.op.height;
            area(a)
                .partial_cmp(&area(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let Some(best) = best else {
            out.push(
                finding(
                    "IRC R310.2 egress",
                    Severity::Error,
                    format!("{name} has no egress window or exterior door."),
                    "Add an exterior window that meets egress sizes, or an exterior door.",
                )
                .at(room.centroid)
                .on(Target::Room(i)),
            );
            continue;
        };
        if windows.iter().any(|w| {
            let op = w.op;
            let mut v = Vec::new();
            egress_shortfalls(op.width, op.height, op.sill_height, min_area, o, &mut v);
            v.is_empty()
        }) {
            continue;
        }
        let op = best.op;
        let mut why = Vec::new();
        egress_shortfalls(op.width, op.height, op.sill_height, min_area, o, &mut why);
        let mut f = finding(
            "IRC R310.2 egress",
            Severity::Error,
            format!(
                "No window in {name} meets egress. The largest ({}) {}.",
                dims(op.width, op.height),
                why.join("; ")
            ),
            "Enlarge the window or lower its sill: 5.7 sq ft (5.0 on the grade floor), 20\" wide, 24\" high, sill at most 44\".",
        )
        .on(Target::Opening(op.id));
        if let Some(c) = best.center {
            f = f.at(c);
        }
        out.push(f);
    }
}

fn dims(w: f64, h: f64) -> String {
    format!("{w:.0}\" x {h:.0}\"")
}

fn egress_shortfalls(
    w: f64,
    h: f64,
    sill: f64,
    min_area: f64,
    o: &crate::CheckOptions,
    why: &mut Vec<String>,
) {
    let area = sq_ft(w, h);
    if area < min_area - EPS {
        why.push(format!("has {area:.1} sq ft, needs {min_area:.1}"));
    }
    if w < o.egress_min_w - EPS {
        why.push(format!("is {w:.0}\" wide, needs {:.0}\"", o.egress_min_w));
    }
    if h < o.egress_min_h - EPS {
        why.push(format!("is {h:.0}\" high, needs {:.0}\"", o.egress_min_h));
    }
    if sill > o.egress_max_sill + EPS {
        why.push(format!(
            "has a {sill:.0}\" sill, at most {:.0}\" allowed",
            o.egress_max_sill
        ));
    }
}

/// Rule 3, IRC R303.3 and R303.4 ventilation: bathrooms, kitchens and
/// bedrooms without an exterior window need mechanical ventilation (Info).
pub(crate) fn ventilation(ctx: &Ctx, out: &mut Vec<Finding>) {
    for (i, room) in ctx.rooms.iter().enumerate() {
        let needs = ctx.is_bath(i) || ctx.is_bedroom(i) || ctx.types[i].contains("kitchen");
        if needs && ctx.exterior_openings(i, OpeningKind::Window).is_empty() {
            out.push(
                finding(
                    "IRC R303.3 ventilation",
                    Severity::Info,
                    format!(
                        "{} has no window: mechanical ventilation required.",
                        ctx.name(i)
                    ),
                    "Add an exterior window, or an exhaust fan ducted to the outdoors.",
                )
                .at(room.centroid)
                .on(Target::Room(i)),
            );
        }
    }
}

/// Rule 4, IRC R311.2 egress doors and door widths: the entry door should
/// have a 36" leaf (32" clear), interior doors 28" (Info; under 24" is a
/// Warning) and bathroom doors at least `bath_door`.
pub(crate) fn door_widths(ctx: &Ctx, out: &mut Vec<Finding>) {
    const RULE: &str = "IRC R311.2 door width";
    let leaf = ctx.opts.min_door_width + 4.0;
    for oi in ctx.ops.iter().filter(|o| o.op.kind == OpeningKind::Door) {
        let op = oi.op;
        let touches =
            |pred: &dyn Fn(usize) -> bool| [oi.left, oi.right].into_iter().flatten().any(pred);
        if !touches(&|_| true) {
            continue; // reported by opening_geometry
        }
        let garage = touches(&|r| ctx.is_garage(r));
        let (sev, msg, fix) = if oi.exterior() {
            if garage || op.width >= leaf - EPS {
                continue;
            }
            (
                Severity::Warning,
                format!(
                    "Entry door is {:.0}\" wide, under the {leaf:.0}\" leaf that gives {:.0}\" clear.",
                    op.width, ctx.opts.min_door_width
                ),
                "Use a 3'-0\" entry door.",
            )
        } else if touches(&|r| ctx.is_bath(r)) {
            if op.width >= ctx.opts.bath_door - EPS {
                continue;
            }
            (
                Severity::Warning,
                format!(
                    "Bathroom door is {:.0}\" wide, under the {:.0}\" minimum.",
                    op.width, ctx.opts.bath_door
                ),
                "Widen the door to at least 24\"; 30\" is typical.",
            )
        } else if op.width < 24.0 - EPS {
            (
                Severity::Warning,
                format!("Interior door is {:.0}\" wide, under 24\".", op.width),
                "Use a door at least 28\" wide.",
            )
        } else if op.width < 28.0 - EPS {
            (
                Severity::Info,
                format!(
                    "Interior door is {:.0}\" wide; 28\" or more is advised.",
                    op.width
                ),
                "Use a 28\" or wider door where space allows.",
            )
        } else {
            continue;
        };
        let mut f = finding(RULE, sev, msg, fix).on(Target::Opening(op.id));
        if let Some(c) = oi.center {
            f = f.at(c);
        }
        out.push(f);
    }
}

/// Plan polygon of a door's swing: the quarter disc about its hinge.
fn swing_polygon(oi: &OpInfo) -> Option<Vec<Point>> {
    let (w, op) = (oi.wall?, oi.op);
    let (d, n) = (w.direction(), w.normal());
    let (hinge, along) = if op.hinge_at_end {
        (w.point_at(op.end_offset()), d.scale(-1.0))
    } else {
        (w.point_at(op.start_offset()), d)
    };
    let side = if op.swing_flipped { n.scale(-1.0) } else { n };
    let mut pts = vec![hinge];
    for k in 0..=8 {
        let a = f64::from(k) / 8.0 * std::f64::consts::FRAC_PI_2;
        let v = along.scale(a.cos()).add(side.scale(a.sin()));
        pts.push(hinge.add(v.scale(op.width)));
    }
    Some(pts)
}

/// Rule 5, IRC R311.7.6 landings and door swings: a door should not swing
/// into a stair, and bedroom doors should swing into the bedroom (Info).
pub(crate) fn door_swings(ctx: &Ctx, out: &mut Vec<Finding>) {
    let stair_polys: Vec<_> = ctx.stairs.iter().map(|s| (s.id, footprint(s))).collect();
    for oi in ctx.ops.iter().filter(|o| o.op.kind == OpeningKind::Door) {
        let op = oi.op;
        let Some(arc) = swing_polygon(oi) else {
            continue;
        };
        let at = oi.center.unwrap_or(arc[0]);
        if let Some((sid, _)) = stair_polys.iter().find(|(_, p)| polygons_overlap(&arc, p)) {
            out.push(
                finding(
                    "IRC R311.7.6 door swing over stair",
                    Severity::Warning,
                    format!("The door's swing reaches stair {sid}."),
                    "Flip the swing, move the door, or add a landing at least as deep as the door is wide.",
                )
                .at(at)
                .on(Target::Opening(op.id)),
            );
        }
        let bedroom = [oi.left, oi.right]
            .into_iter()
            .flatten()
            .find(|&r| ctx.is_bedroom(r));
        if let Some(b) = bedroom {
            let other = oi.other_side(b);
            let other_is_bedroom = other.is_some_and(|r| ctx.is_bedroom(r));
            if oi.swing_room() != Some(b) && !other_is_bedroom {
                out.push(
                    finding(
                        "IRC R311.2 bedroom door swing",
                        Severity::Info,
                        format!("The door to {} swings out of the bedroom.", ctx.name(b)),
                        "Bedroom doors normally swing into the room; flip the swing if space allows.",
                    )
                    .at(at)
                    .on(Target::Opening(op.id)),
                );
            }
        }
    }
}

/// Rule 6, IRC R311.6 hallways: at least 36" wide.
pub(crate) fn hallway_width(ctx: &Ctx, out: &mut Vec<Finding>) {
    for (i, room) in ctx.rooms.iter().enumerate() {
        if !is_hall(&ctx.types[i]) {
            continue;
        }
        let dim = min_dimension(&room.polygon);
        if dim < ctx.opts.min_hall_width - EPS {
            out.push(
                finding(
                    "IRC R311.6 hallway width",
                    Severity::Error,
                    format!(
                        "{} is {} wide, under {} ({CENTERLINE_NOTE}).",
                        ctx.name(i),
                        fmt_ft_in(dim),
                        fmt_ft_in(ctx.opts.min_hall_width)
                    ),
                    "Widen the hallway to at least 3'-0\" clear.",
                )
                .at(room.centroid)
                .on(Target::Room(i)),
            );
        }
    }
}

/// Rule 7, IRC R311.7 stairways: riser at most 7 3/4", tread at least 10",
/// width 36", headroom 6'-8", landings 36" and no more than 12'-7" of rise
/// between landings. The 2R+T comfort rule is Info. Stairs are solved with
/// [`plan_stairs::solve`]; a riser target over the limit is an error even
/// though the solver would add risers, because the stair as specified fails.
pub(crate) fn stairs(ctx: &Ctx, out: &mut Vec<Finding>) {
    let o = ctx.opts;
    for s in ctx.stairs {
        let p = &s.params;
        let sol = solve(p);
        let on = |f: Finding| f.at(s.origin).on(Target::Stair(s.id));
        if let StairShape::Ramp { .. } = p.shape {
            for w in sol.warnings.iter().filter(|_| !sol.code_ok) {
                out.push(on(finding(
                    "IRC R311.8 ramps",
                    Severity::Error,
                    format!("Ramp: {w}."),
                    "Flatten the slope to 1:12 or flatter and add landings every 30\" of rise.",
                )));
            }
            continue;
        }
        let riser = sol.riser_height.max(p.riser_height_target);
        if riser > o.riser_max + EPS {
            out.push(on(finding(
                "IRC R311.7.5.1 riser height",
                Severity::Error,
                format!(
                    "Riser height {riser:.2}\" exceeds the {:.2}\" maximum.",
                    o.riser_max
                ),
                "Use more risers so each is 7 3/4\" or less.",
            )));
        }
        if p.tread_depth < o.tread_min - EPS {
            out.push(on(finding(
                "IRC R311.7.5.2 tread depth",
                Severity::Error,
                format!(
                    "Tread depth {:.2}\" is under the {:.0}\" minimum.",
                    p.tread_depth, o.tread_min
                ),
                "Increase the tread depth to at least 10\".",
            )));
        }
        if p.width < o.stair_min_width - EPS {
            out.push(on(finding(
                "IRC R311.7.1 stair width",
                Severity::Error,
                format!(
                    "Stair width {:.1}\" is under the {:.0}\" minimum.",
                    p.width, o.stair_min_width
                ),
                "Widen the stair to at least 36\" clear.",
            )));
        }
        if p.headroom_min < o.headroom - EPS {
            out.push(on(finding(
                "IRC R311.7.2 headroom",
                Severity::Error,
                format!(
                    "Headroom {:.1}\" is under the {:.0}\" minimum.",
                    p.headroom_min, o.headroom
                ),
                "Raise the ceiling or move the stairwell opening to give 6'-8\" headroom.",
            )));
        }
        if matches!(
            p.shape,
            StairShape::LShaped { .. } | StairShape::UShaped { .. }
        ) {
            let landing = p.landing_depth.max(p.width);
            if landing < o.landing_min - EPS {
                out.push(on(finding(
                    "IRC R311.7.6 landings",
                    Severity::Error,
                    format!(
                        "Landing depth {landing:.1}\" is under the {:.0}\" minimum.",
                        o.landing_min
                    ),
                    "Make the landing at least as deep as the stair is wide, and 36\" minimum.",
                )));
            }
        }
        let flight_rise = longest_flight_rise(p.shape, sol.risers, sol.riser_height);
        if flight_rise > MAX_FLIGHT_RISE + EPS {
            out.push(on(finding(
                "IRC R311.7.3 vertical rise",
                Severity::Error,
                format!(
                    "A flight rises {} between landings, over the {} maximum.",
                    fmt_ft_in(flight_rise),
                    fmt_ft_in(MAX_FLIGHT_RISE)
                ),
                "Add an intermediate landing so no flight rises more than 12'-7\".",
            )));
        }
        let comfort = 2.0 * sol.riser_height + p.tread_depth;
        if !(24.0 - EPS..=25.0 + EPS).contains(&comfort) {
            out.push(on(finding(
                "IRC R311.7.5 stair comfort (2R+T)",
                Severity::Info,
                format!("2R+T is {comfort:.2}\", outside the 24\" to 25\" comfort range."),
                "Adjust riser or tread so 2R+T falls between 24\" and 25\".",
            )));
        }
    }
}

/// Rise of the tallest flight between landings or floors.
fn longest_flight_rise(shape: StairShape, risers: u32, riser_height: f64) -> f64 {
    let total = f64::from(risers) * riser_height;
    match shape {
        StairShape::LShaped {
            treads_before_landing,
        }
        | StairShape::UShaped {
            treads_before_landing,
        } if risers >= 2 => {
            let first = treads_before_landing.min(risers - 2) + 1;
            f64::from(first.max(risers - first)) * riser_height
        }
        _ => total,
    }
}

/// Rule 8, IRC R302.5.1 and R309.1 garage: the door between garage and house
/// must be at least 32" wide and solid or 20-minute rated (Info), no garage
/// door may open into a bedroom, and the garage floor should sit lower than
/// the house (Info). Runs only when a room is typed Garage.
pub(crate) fn garage(ctx: &Ctx, out: &mut Vec<Finding>) {
    for (g, room) in ctx
        .rooms
        .iter()
        .enumerate()
        .filter(|(g, _)| ctx.is_garage(*g))
    {
        out.push(
            finding(
                "IRC R309.1 garage floor",
                Severity::Info,
                format!(
                    "{}: the floor should be lower than the house floor and slope to the door.",
                    ctx.name(g)
                ),
                "Step the garage slab down at least 4\" from the house and slope it to drain.",
            )
            .at(room.centroid)
            .on(Target::Room(g)),
        );
        for oi in ctx
            .ops
            .iter()
            .filter(|o| o.op.kind == OpeningKind::Door && o.touches(g))
        {
            let Some(h) = oi.other_side(g).filter(|&h| !ctx.is_garage(h)) else {
                continue;
            };
            let at = oi.center.unwrap_or(room.centroid);
            let on = Target::Opening(oi.op.id);
            if ctx.is_bedroom(h) {
                out.push(
                    finding(
                        "IRC R302.5.1 garage opening",
                        Severity::Error,
                        format!("The garage opens directly into {}, a sleeping room.", ctx.name(h)),
                        "Move the door so the garage opens to a hall, mudroom or other non-sleeping space.",
                    )
                    .at(at)
                    .on(on),
                );
            }
            if oi.op.width < ctx.opts.garage_house_door_min - EPS {
                out.push(
                    finding(
                        "IRC R302.5.1 garage door width",
                        Severity::Error,
                        format!(
                            "The garage-to-house door is {:.0}\" wide, under the {:.0}\" minimum.",
                            oi.op.width, ctx.opts.garage_house_door_min
                        ),
                        "Use a door at least 32\" wide between garage and house.",
                    )
                    .at(at)
                    .on(on),
                );
            }
            out.push(
                finding(
                    "IRC R302.5.1 garage door rating",
                    Severity::Info,
                    "The garage-to-house door must be solid wood 1 3/8\" thick, solid or honeycomb steel, or 20-minute fire rated, and self-closing where required.".to_string(),
                    "Specify a 20-minute rated or solid-core door with a self-closing hinge.",
                )
                .at(at)
                .on(on),
            );
        }
    }
}

/// Rule 9, wall geometry (drawing quality): dangling exterior walls, duplicate
/// or overlapping walls and tiny wall segments.
pub(crate) fn wall_geometry(ctx: &Ctx, out: &mut Vec<Finding>) {
    let walls = &ctx.floor.walls;
    for (i, w) in walls.iter().enumerate() {
        if w.length() < TINY_WALL {
            out.push(
                finding(
                    "Plan geometry: tiny wall",
                    Severity::Warning,
                    format!("Wall {} is only {:.1}\" long.", w.id, w.length()),
                    "Delete the wall or extend it; short segments are usually drawing slips.",
                )
                .at(w.point_at(w.length() / 2.0))
                .on(Target::Wall(w.id)),
            );
        }
        if w.kind == WallKind::Exterior {
            for end in [w.start, w.end] {
                let joined = walls
                    .iter()
                    .enumerate()
                    .any(|(j, o)| j != i && dist_to_segment(end, o.start, o.end) <= JOIN_TOL);
                if !joined {
                    out.push(
                        finding(
                            "Plan geometry: dangling exterior wall",
                            Severity::Warning,
                            format!("Exterior wall {} has a free end that touches no other wall.", w.id),
                            "Extend the end to the adjoining wall, or close the gap so the room encloses.",
                        )
                        .at(end)
                        .on(Target::Wall(w.id)),
                    );
                }
            }
        }
        for o in &walls[i + 1..] {
            if let Some(len) = collinear_overlap(w, o) {
                out.push(
                    finding(
                        "Plan geometry: duplicate wall",
                        Severity::Warning,
                        format!(
                            "Wall {} overlaps wall {} along {len:.1}\" on the same centerline.",
                            o.id, w.id
                        ),
                        "Delete one of the two walls.",
                    )
                    .at(o.point_at(o.length() / 2.0))
                    .on(Target::Wall(o.id)),
                );
            }
        }
    }
}

/// Length two walls share along one centerline (within 1"), if more than 1".
fn collinear_overlap(a: &Wall, b: &Wall) -> Option<f64> {
    if a.length() < EPS || b.length() < EPS {
        return None;
    }
    let (d, n) = (a.direction(), a.normal());
    let off = |p: Point| p.sub(a.start).dot(n).abs();
    if off(b.start) > SAME_LINE || off(b.end) > SAME_LINE {
        return None;
    }
    let (t0, t1) = (b.start.sub(a.start).dot(d), b.end.sub(a.start).dot(d));
    let overlap = t0.max(t1).min(a.length()) - t0.min(t1).max(0.0);
    (overlap > SAME_LINE).then_some(overlap)
}

/// Rule 10, opening geometry: openings must fit their wall, must not
/// overlap, doors need a room on a side, and low windows on a floor over 72"
/// above grade need fall protection (IRC R312.2, Info).
pub(crate) fn opening_geometry(ctx: &Ctx, out: &mut Vec<Finding>) {
    const RULE: &str = "Plan geometry: opening";
    for (i, oi) in ctx.ops.iter().enumerate() {
        let op = oi.op;
        let on = Target::Opening(op.id);
        let Some(w) = oi.wall else {
            out.push(
                finding(
                    RULE,
                    Severity::Error,
                    format!(
                        "Opening {} refers to wall {}, which does not exist.",
                        op.id, op.wall_id
                    ),
                    "Delete the opening or re-place it on a wall.",
                )
                .on(on),
            );
            continue;
        };
        let at = oi.center.unwrap_or(w.start);
        if op.start_offset() < -EPS || op.end_offset() > w.length() + EPS {
            out.push(
                finding(
                    RULE,
                    Severity::Error,
                    format!(
                        "Opening {} spans {:.1}\" to {:.1}\" on wall {}, which is {:.1}\" long.",
                        op.id,
                        op.start_offset(),
                        op.end_offset(),
                        w.id,
                        w.length()
                    ),
                    "Narrow the opening or move it so it lies inside its wall.",
                )
                .at(at)
                .on(on),
            );
        }
        for other in &ctx.ops[i + 1..] {
            let q = other.op;
            if q.wall_id == op.wall_id
                && op.start_offset() < q.end_offset() - EPS
                && op.end_offset() > q.start_offset() + EPS
            {
                out.push(
                    finding(
                        RULE,
                        Severity::Error,
                        format!("Openings {} and {} overlap on wall {}.", op.id, q.id, w.id),
                        "Move one opening so they clear each other.",
                    )
                    .at(at)
                    .on(on),
                );
            }
        }
        if op.kind == OpeningKind::Window
            && op.sill_height < FALL_SILL
            && ctx.floor.elevation > FALL_FLOOR_HEIGHT
        {
            out.push(
                finding(
                    "IRC R312.2 window fall protection",
                    Severity::Info,
                    format!("Window {} has a {:.0}\" sill on a floor over 72\" above grade.", op.id, op.sill_height),
                    "Raise the sill, or fit a window opening control device or fall-protection guard.",
                )
                .at(at)
                .on(on),
            );
        }
        if op.kind == OpeningKind::Door && oi.left.is_none() && oi.right.is_none() {
            out.push(
                finding(
                    RULE,
                    Severity::Warning,
                    format!("Door {} has no room on either side.", op.id),
                    "Move the door to a wall that bounds a room, or close the room boundary.",
                )
                .at(at)
                .on(on),
            );
        }
    }
}

/// Rule 11, room access (IRC R311.1 means of egress): unnamed rooms (Info),
/// rooms with no door (Error: unreachable) and rooms entered only through a
/// bathroom (Warning).
pub(crate) fn room_access(ctx: &Ctx, out: &mut Vec<Finding>) {
    let stair_polys: Vec<_> = ctx.stairs.iter().map(footprint).collect();
    for (i, room) in ctx.rooms.iter().enumerate() {
        let name = ctx.name(i);
        let mut f = |sev, rule, msg: String, fix: &str| {
            out.push(
                finding(rule, sev, msg, fix)
                    .at(room.centroid)
                    .on(Target::Room(i)),
            );
        };
        if ctx.types[i].is_empty() {
            f(
                Severity::Info,
                "Plan hygiene: unnamed room",
                format!("{name} has no room name or type."),
                "Name the room so room-specific checks can run.",
            );
        }
        let doors: Vec<&OpInfo> = ctx
            .ops
            .iter()
            .filter(|o| o.op.kind == OpeningKind::Door && o.touches(i))
            .collect();
        let on_stair = stair_polys
            .iter()
            .any(|p| polygons_overlap(&room.polygon, p));
        if doors.is_empty() {
            if !on_stair {
                f(
                    Severity::Error,
                    "IRC R311.1 means of egress",
                    format!("{name} has no door: it cannot be reached."),
                    "Add a door or opening to the room.",
                );
            }
        } else if !ctx.is_bath(i)
            && doors
                .iter()
                .all(|d| d.other_side(i).is_some_and(|r| ctx.is_bath(r)))
        {
            f(
                Severity::Warning,
                "IRC R311.1 access through bathroom",
                format!("The only access to {name} is through a bathroom."),
                "Add a door from a hall or another room.",
            );
        }
    }
}

/// Rule 12, IRC R303.1 natural light: glazing should be at least 8% of the
/// floor area, and glazing should reach `window_to_wall_ratio_warn` of the
/// room's exterior wall area (Info). Applies to habitable rooms.
pub(crate) fn natural_light(ctx: &Ctx, out: &mut Vec<Finding>) {
    let exterior: Vec<&Wall> = ctx
        .floor
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior)
        .collect();
    for (i, room) in ctx.rooms.iter().enumerate() {
        if !is_habitable(&ctx.types[i]) || polygon_area(&room.polygon) <= 0.0 {
            continue;
        }
        let glazing: f64 = ctx
            .exterior_openings(i, OpeningKind::Window)
            .iter()
            .map(|w| sq_ft(w.op.width, w.op.height))
            .sum();
        let floor_area = room.area_sq_ft();
        let name = ctx.name(i);
        let mut note = |msg: String, fix: &str| {
            out.push(
                finding("IRC R303.1 natural light", Severity::Info, msg, fix)
                    .at(room.centroid)
                    .on(Target::Room(i)),
            );
        };
        if glazing < LIGHT_FLOOR_RATIO * floor_area - EPS {
            note(
                format!(
                    "{name} has {glazing:.1} sq ft of glazing, under 8% of its {floor_area:.0} sq ft floor ({:.1} sq ft).",
                    LIGHT_FLOOR_RATIO * floor_area
                ),
                "Add or enlarge exterior windows.",
            );
            continue;
        }
        let ext_len = exterior_edge_length(&room.polygon, &exterior);
        let wall_area = sq_ft(ext_len, ctx.floor.ceiling_height);
        if wall_area > 0.0 && glazing / wall_area < ctx.opts.window_to_wall_ratio_warn - EPS {
            note(
                format!(
                    "Glazing is {:.0}% of {name}'s exterior wall area, under {:.0}%.",
                    100.0 * glazing / wall_area,
                    100.0 * ctx.opts.window_to_wall_ratio_warn
                ),
                "Consider larger or additional windows for daylight.",
            );
        }
    }
}

/// Total length of polygon edges that lie along exterior walls.
fn exterior_edge_length(poly: &[Point], exterior: &[&Wall]) -> f64 {
    (0..poly.len())
        .map(|k| (poly[k], poly[(k + 1) % poly.len()]))
        .filter(|&(a, b)| {
            let mid = Point::lerp(a, b, 0.5);
            exterior
                .iter()
                .any(|w| dist_to_segment(mid, w.start, w.end) < 1.5)
        })
        .map(|(a, b)| a.dist(b))
        .sum()
}
