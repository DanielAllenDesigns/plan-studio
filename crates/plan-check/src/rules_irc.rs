//! IRC 2021 rules added in round 14: the egress door, stair handrail height
//! and continuity, guard openings and landing guards, the garage separation
//! wall, alarms outside sleeping areas, and footing depth and size.
//!
//! The ones that read plumbing fixtures, receptacles or rafters are in
//! `rules_fixtures.rs` and `rules_mep.rs`. Every rule here appends
//! [`Finding`]s like the others, with the code section in its `rule` string
//! (listed in [`crate::rule_catalog`]) and a place to zoom to.

use std::collections::BTreeMap;

use plan_core::foundation::FoundationLayer;
use plan_core::geometry::dist_to_segment;
use plan_core::units::fmt_ft_in;
use plan_core::walls::Side;
use plan_core::{FloorKind, FoundationKind, OpeningKind, OpeningStyle, Point, WallTypeDef};
use plan_stairs::{footprint, solve, RailStyle, RailingParams, SideKind, Stair, StairShape};

use crate::ctx::{is_habitable, Ctx};
use crate::geom::polygons_overlap;
use crate::rules_mep::{devices, in_room};
use crate::{finding, Finding, Severity, Target};

const EPS: f64 = 1e-6;
/// Two stair pieces closer than this are joined (a landing and its flight).
const STAIR_JOIN: f64 = 4.0;
/// How far from a wall centerline to look for the room on each side.
const SIDE_PROBE: f64 = 3.0;

/// Run every rule of this file.
pub(crate) fn run(ctx: &Ctx, out: &mut Vec<Finding>) {
    egress_door(ctx, out);
    stair_handrail_height(ctx, out);
    stair_handrail_continuity(ctx, out);
    guard_openings(ctx, out);
    garage_separation(ctx, out);
    alarms_outside_sleeping_areas(ctx, out);
    footings(ctx, out);
}

// ----- doors -----

/// IRC R311.2: every dwelling needs one side-hinged egress door to the
/// outside, 32" clear wide (a 36" leaf) and 78" clear high (an 80" door), that
/// opens straight onto the grade floor. Runs on the grade floor when it has a
/// habitable room; garage doors and doors out of a garage do not count.
fn egress_door(ctx: &Ctx, out: &mut Vec<Finding>) {
    if ctx.floor.kind != FloorKind::Normal || !ctx.grade_floor() {
        return;
    }
    let Some(first) = (0..ctx.rooms.len()).find(|&i| is_habitable(&ctx.types[i])) else {
        return;
    };
    let o = ctx.opts;
    let leaf = o.min_door_width + 4.0;
    let side_hinged =
        |s: OpeningStyle| matches!(s, OpeningStyle::Hinged | OpeningStyle::DoubleDoor);
    let ok = ctx.ops.iter().any(|oi| {
        oi.op.kind == OpeningKind::Door
            && oi.exterior()
            && side_hinged(oi.op.style)
            && ![oi.left, oi.right]
                .into_iter()
                .flatten()
                .any(|r| ctx.is_garage(r))
            && oi.op.width >= leaf - EPS
            && oi.op.height >= o.entry_door_height - EPS
    });
    if !ok {
        out.push(
            finding(
                "IRC R311.2 egress door",
                Severity::Error,
                format!(
                    "{} has no side-hinged exterior door at least {} wide ({:.0}\" clear) and {} high.",
                    ctx.floor.name,
                    fmt_ft_in(leaf),
                    o.min_door_width,
                    fmt_ft_in(o.entry_door_height)
                ),
                "Add a hinged exterior door, 3'-0\" x 6'-8\" or larger, that opens directly to the outside.",
            )
            .at(ctx.rooms[first].centroid)
            .on(Target::Room(first)),
        );
    }
}

// ----- stairs -----

fn has_rail(side: SideKind) -> bool {
    matches!(side, SideKind::Railing | SideKind::HalfWall)
}

/// A flight, not a landing or a ramp.
fn is_flight(s: &Stair) -> bool {
    !matches!(
        s.params.shape,
        StairShape::Landing { .. } | StairShape::Ramp { .. }
    )
}

/// The two sides of a stair with the rail each one carries.
fn sides(s: &Stair) -> [(&'static str, SideKind, RailingParams, bool); 2] {
    let p = &s.params;
    [
        (
            "left",
            p.left_side,
            p.left_railing.unwrap_or(p.railing),
            p.left_railing.is_some(),
        ),
        (
            "right",
            p.right_side,
            p.right_railing.unwrap_or(p.railing),
            p.right_railing.is_some(),
        ),
    ]
}

/// IRC R311.7.8.1: a handrail is 34" to 38" above the nosings. A side's own
/// rail (set on the Rails tab for that side) is judged here for both limits;
/// the stair-wide rail is judged for the 34" limit by the R312.1.2 rule. A stair
/// with a separate handrail (Handrail ticked) may have a taller guard.
fn stair_handrail_height(ctx: &Ctx, out: &mut Vec<Finding>) {
    let o = ctx.opts;
    for s in ctx.stairs.iter().filter(|s| is_flight(s)) {
        for (name, kind, rail, own) in sides(s) {
            if !has_rail(kind) {
                continue;
            }
            let h = rail.height;
            let on = |f: Finding| f.at(s.origin).on(Target::Stair(s.id));
            if h > o.handrail_max + EPS && !s.params.handrail {
                out.push(on(finding(
                    "IRC R311.7.8.1 handrail height",
                    Severity::Warning,
                    format!(
                        "The {name} rail of the stair is {h:.1}\" high; a handrail is {:.0}\" to {:.0}\" above the nosings.",
                        o.stair_guard_height, o.handrail_max
                    ),
                    "Lower the rail to 38\" or less, or add a separate handrail at 34\" to 38\" (turn on Handrail).",
                )));
            } else if own && h < o.stair_guard_height - EPS {
                out.push(on(finding(
                    "IRC R311.7.8.1 handrail height",
                    Severity::Error,
                    format!(
                        "The {name} rail of the stair is {h:.1}\" high, under the {:.0}\" minimum for a handrail.",
                        o.stair_guard_height
                    ),
                    "Raise the rail to at least 34\" above the nosings.",
                )));
            }
        }
    }
}

/// Whether two plan outlines overlap or come within `tol` of each other.
fn near(a: &[Point], b: &[Point], tol: f64) -> bool {
    if polygons_overlap(a, b) {
        return true;
    }
    let close = |p: &[Point], q: &[Point]| {
        p.iter()
            .any(|v| (0..q.len()).any(|k| dist_to_segment(*v, q[k], q[(k + 1) % q.len()]) <= tol))
    };
    close(a, b) || close(b, a)
}

/// IRC R311.7.8.2: a handrail is continuous for the full length of the
/// flight. A flight that has a rail and runs into a landing that has none
/// (the landing is a separate stair piece) breaks the rail at the landing.
fn stair_handrail_continuity(ctx: &Ctx, out: &mut Vec<Finding>) {
    let railed = |s: &Stair| has_rail(s.params.left_side) || has_rail(s.params.right_side);
    for landing in ctx
        .stairs
        .iter()
        .filter(|s| matches!(s.params.shape, StairShape::Landing { .. }))
    {
        if railed(landing) {
            continue;
        }
        let here = footprint(landing);
        let flight = ctx.stairs.iter().find(|f| {
            is_flight(f)
                && f.id != landing.id
                && (railed(f) || f.params.handrail)
                && solve(&f.params).risers >= ctx.opts.handrail_risers
                && near(&here, &footprint(f), STAIR_JOIN)
        });
        if flight.is_some() {
            out.push(
                finding(
                    "IRC R311.7.8.2 handrail continuity",
                    Severity::Warning,
                    "The handrail stops at this landing; the rail must run the full length of the flight and continue around a landing.".to_string(),
                    "Give the landing a Railing or Half Wall side so the rail continues past it.",
                )
                .at(landing.origin)
                .on(Target::Stair(landing.id)),
            );
        }
    }
}

/// IRC R312.1.2 and R312.1.3: a guard on a landing is 36" high (the stair
/// itself is judged at 34" by the rule in `rules_code.rs`), and no opening in a
/// guard lets a 4" sphere through.
fn guard_openings(ctx: &Ctx, out: &mut Vec<Finding>) {
    let o = ctx.opts;
    for s in ctx
        .stairs
        .iter()
        .filter(|s| !matches!(s.params.shape, StairShape::Ramp { .. }))
    {
        let on = |f: Finding| f.at(s.origin).on(Target::Stair(s.id));
        let landing = matches!(s.params.shape, StairShape::Landing { .. });
        let mut low_guard = false;
        for (name, kind, rail, _) in sides(s) {
            if kind != SideKind::Railing {
                continue;
            }
            if landing && rail.height < o.guard_height - EPS && !low_guard {
                low_guard = true;
                out.push(on(finding(
                    "IRC R312.1.2 guard height",
                    Severity::Error,
                    format!(
                        "The {name} guard of the landing is {:.1}\" high, under the {:.0}\" minimum.",
                        rail.height, o.guard_height
                    ),
                    "Raise the guard to at least 36\" above the landing floor.",
                )));
            }
            let gap = match rail.style {
                RailStyle::Balusters { spacing, .. } => Some(spacing),
                RailStyle::Cable { rows } => {
                    Some((rail.height - rail.top_rail.1).max(0.0) / (f64::from(rows) + 1.0))
                }
                _ => None,
            };
            if let Some(g) = gap.filter(|g| *g > o.guard_sphere + EPS) {
                out.push(on(finding(
                    "IRC R312.1.3 opening limitation",
                    Severity::Error,
                    format!(
                        "The {name} guard of the stair has openings of {g:.1}\"; a {:.0}\" sphere must not pass through.",
                        o.guard_sphere
                    ),
                    "Space the balusters or cables so the clear opening is 4\" or less.",
                )));
            }
        }
    }
}

// ----- garage -----

/// Whether a wall layer is gypsum board.
fn is_gypsum(name: &str, material: &str) -> bool {
    let t = format!("{name} {material}").to_lowercase();
    [
        "drywall",
        "gypsum",
        "gyp ",
        "sheetrock",
        "type x",
        "wallboard",
    ]
    .iter()
    .any(|k| t.contains(k))
}

/// IRC R302.6: the wall between a garage and the house is covered on the
/// garage side with at least 1/2" gypsum board (5/8" Type X under a habitable
/// room above). A wall with no assembly gets a note; one whose garage-side layer
/// is not gypsum, or is too thin, is flagged.
fn garage_separation(ctx: &Ctx, out: &mut Vec<Finding>) {
    if !(0..ctx.rooms.len()).any(|i| ctx.is_garage(i)) {
        return;
    }
    let room_at = |p: Point| ctx.rooms.iter().position(|r| r.contains(p));
    for w in &ctx.floor.walls {
        if w.flags.invisible || w.flags.room_divider || w.flags.railing || w.length() < 12.0 {
            continue;
        }
        let n = w.normal().scale(SIDE_PROBE);
        // (garage on the left side?, the room on the other side)
        let found = [0.25, 0.5, 0.75].into_iter().find_map(|t| {
            let c = w.point_at(w.length() * t);
            let (l, r) = (room_at(c.add(n)), room_at(c.sub(n)));
            match (l, r) {
                (Some(g), Some(h)) if ctx.is_garage(g) && !ctx.is_garage(h) => Some((true, h)),
                (Some(h), Some(g)) if ctx.is_garage(g) && !ctx.is_garage(h) => Some((false, h)),
                _ => None,
            }
        });
        let Some((garage_left, house)) = found else {
            continue;
        };
        let at = w.point_at(w.length() * 0.5);
        let on = Target::Wall(w.id);
        let ty: Option<&WallTypeDef> = w
            .wall_type
            .as_deref()
            .and_then(|name| ctx.wall_types.iter().find(|t| t.name == name))
            .filter(|t| !t.layers.is_empty());
        let note = format!(
            "Wall {} separates the garage from {}",
            w.id,
            ctx.name(house)
        );
        let Some(def) = ty else {
            out.push(
                finding(
                    "IRC R302.6 garage separation",
                    Severity::Info,
                    format!(
                        "{note}: the garage side needs at least {:.1}\" gypsum board (5/8\" Type X under habitable space above).",
                        ctx.opts.garage_gypsum_min
                    ),
                    "Give the wall a wall type with gypsum board on the garage side.",
                )
                .at(at)
                .on(on),
            );
            continue;
        };
        // Layers run exterior to interior; the exterior side is the wall's own.
        let garage_is_exterior = garage_left == (w.exterior_side == Side::Left);
        let layer = if garage_is_exterior {
            def.layers.first()
        } else {
            def.layers.last()
        };
        let Some(layer) = layer else { continue };
        if !is_gypsum(&layer.name, &layer.material) {
            out.push(
                finding(
                    "IRC R302.6 garage separation",
                    Severity::Warning,
                    format!(
                        "{note}: its garage-side layer ({}) is not gypsum board.",
                        layer.name
                    ),
                    "Finish the garage side with at least 1/2\" gypsum board.",
                )
                .at(at)
                .on(on),
            );
        } else if layer.thickness < ctx.opts.garage_gypsum_min - EPS {
            out.push(
                finding(
                    "IRC R302.6 garage separation",
                    Severity::Error,
                    format!(
                        "{note}: its garage-side gypsum board is {:.2}\" thick, under the {:.1}\" minimum.",
                        layer.thickness, ctx.opts.garage_gypsum_min
                    ),
                    "Use at least 1/2\" gypsum board on the garage side (5/8\" Type X under habitable space).",
                )
                .at(at)
                .on(on),
            );
        }
    }
}

// ----- alarms -----

/// IRC R314.3 and R315.3: a smoke alarm and a carbon monoxide alarm go
/// outside each separate sleeping area, in the immediate vicinity of the
/// bedrooms. A sleeping area is the room the bedroom doors open into (a hall).
/// Runs once the floor has been wired (has electrical devices).
fn alarms_outside_sleeping_areas(ctx: &Ctx, out: &mut Vec<Finding>) {
    let devs = devices(ctx);
    if devs.is_empty() {
        return;
    }
    let mut areas: BTreeMap<usize, usize> = BTreeMap::new();
    for oi in ctx.ops.iter().filter(|o| o.op.kind == OpeningKind::Door) {
        for b in [oi.left, oi.right].into_iter().flatten() {
            if !ctx.is_bedroom(b) {
                continue;
            }
            let Some(h) = oi.other_side(b) else { continue };
            let t = &ctx.types[h];
            if ctx.is_bedroom(h) || ctx.is_bath(h) || ctx.is_garage(h) || t.contains("closet") {
                continue;
            }
            *areas.entry(h).or_default() += 1;
        }
    }
    for (h, beds) in areas {
        let has = |kind: &str| devs.iter().any(|d| d.kind == kind && in_room(ctx, h, d.at));
        let room = &ctx.rooms[h];
        let name = ctx.name(h);
        let plural = if beds == 1 { "" } else { "s" };
        if !has("SmokeDetector") {
            out.push(
                finding(
                    "IRC R314.3 smoke alarm outside sleeping area",
                    Severity::Warning,
                    format!("{name} serves {beds} bedroom door{plural} and has no smoke alarm outside the bedrooms."),
                    "Add a smoke alarm in the hall or room outside the bedrooms.",
                )
                .at(room.centroid)
                .on(Target::Room(h)),
            );
        }
        if !has("CoDetector") {
            out.push(
                finding(
                    "IRC R315.3 CO alarm outside sleeping area",
                    Severity::Info,
                    format!("{name} serves {beds} bedroom door{plural} and has no carbon monoxide alarm; one is required outside each sleeping area where there is a fuel-fired appliance or an attached garage."),
                    "Add a CO alarm in the hall or room outside the bedrooms.",
                )
                .at(room.centroid)
                .on(Target::Room(h)),
            );
        }
    }
}

// ----- footings -----

/// Footing width by number of storeys, 2000 psf soil (IRC Table R403.1(1)).
fn footing_width_for(stories: usize) -> f64 {
    match stories {
        0 | 1 => 12.0,
        2 => 15.0,
        _ => 18.0,
    }
}

/// IRC R403.1.4.1 and R403.1.1: a footing reaches at least the frost depth
/// (12" in Georgia) below finished grade and is at least 6" thick and as wide
/// as the table asks for the storeys. The plan has no grade of its own, so
/// grade is taken `grade_below_floor` below the first floor's finished floor.
/// Reads the foundation floor's build (walls with footings, a thickened slab
/// edge) and the slabs, pads and piers of the floor's foundation layer.
fn footings(ctx: &Ctx, out: &mut Vec<Finding>) {
    let f = ctx.floor;
    let o = ctx.opts;
    // Lowest allowed underside of a footing, relative to the first floor.
    let deepest_ok = -o.grade_below_floor - o.frost_depth;
    let depth_msg = |what: &str, bottom: f64| {
        format!(
            "{what} bottoms out {} below the first floor; with grade {} below the floor, the frost depth of {} puts it at {} or deeper.",
            fmt_ft_in(-bottom),
            fmt_ft_in(o.grade_below_floor),
            fmt_ft_in(o.frost_depth),
            fmt_ft_in(-deepest_ok)
        )
    };
    let depth_fix =
        "Deepen the footing or stem wall, or change the assumed grade in Plan Check Settings.";

    if f.kind == FloorKind::Foundation {
        if let Some(b) = f.settings.foundation {
            let footed = b.footing_width > 0.0 && b.footing_depth > 0.0;
            let bottom = f.elevation - if footed { b.footing_depth } else { 0.0 };
            let wall = f.walls.iter().find(|w| w.flags.foundation);
            let at = wall.map(|w| w.point_at(w.length() * 0.5));
            let mut push = |fnd: Finding| {
                let mut fnd = fnd;
                if let Some(w) = wall {
                    fnd = fnd.on(Target::Wall(w.id));
                }
                if let Some(p) = at {
                    fnd = fnd.at(p);
                }
                out.push(fnd);
            };
            // Grade beams on piers: the piers carry the load and are judged below.
            if !matches!(b.kind, FoundationKind::Pier) && bottom > deepest_ok + EPS {
                push(finding(
                    "IRC R403.1.4 footing depth",
                    Severity::Warning,
                    depth_msg("The foundation", bottom),
                    depth_fix,
                ));
            }
            if footed {
                let need = footing_width_for(ctx.stories);
                if b.footing_width < need - EPS {
                    push(finding(
                        "IRC R403.1.1 footing size",
                        Severity::Warning,
                        format!(
                            "The footing is {:.0}\" wide; {} storey{} on 2000 psf soil need {need:.0}\" (Table R403.1(1)).",
                            b.footing_width,
                            ctx.stories,
                            if ctx.stories == 1 { "" } else { "s" }
                        ),
                        "Widen the footing in Build Foundation, or have it sized for the soil.",
                    ));
                }
                if b.footing_depth < o.footing_min_thickness - EPS {
                    push(finding(
                        "IRC R403.1.1 footing size",
                        Severity::Warning,
                        format!(
                            "The footing is {:.1}\" thick, under the {:.0}\" minimum.",
                            b.footing_depth, o.footing_min_thickness
                        ),
                        "Make the footing at least 6\" thick.",
                    ));
                }
            }
        }
    }

    let layer = FoundationLayer::load(f);
    for s in layer.slabs.iter().filter(|s| s.footing.is_some()) {
        let Some(ft) = s.footing else { continue };
        let bottom = f.elevation + s.top_elevation - s.thickness - ft.depth;
        let at = plan_core::geometry::polygon_centroid(&s.outline);
        if bottom > deepest_ok + EPS {
            out.push(
                finding(
                    "IRC R403.1.4 footing depth",
                    Severity::Warning,
                    depth_msg("The slab footing", bottom),
                    depth_fix,
                )
                .at(at)
                .on(Target::Foundation(s.id)),
            );
        }
        if ft.depth < o.footing_min_thickness - EPS {
            out.push(
                finding(
                    "IRC R403.1.1 footing size",
                    Severity::Warning,
                    format!(
                        "The slab footing is {:.1}\" thick, under the {:.0}\" minimum.",
                        ft.depth, o.footing_min_thickness
                    ),
                    "Make the footing at least 6\" thick.",
                )
                .at(at)
                .on(Target::Foundation(s.id)),
            );
        }
    }
    for p in &layer.piers {
        let bottom = f.elevation
            + p.footing_solid()
                .map_or_else(|| p.bottom_elevation(), |b| b.bottom);
        if bottom > deepest_ok + EPS {
            out.push(
                finding(
                    "IRC R403.1.4 footing depth",
                    Severity::Warning,
                    depth_msg("The pier", bottom),
                    depth_fix,
                )
                .at(p.center)
                .on(Target::Foundation(p.id)),
            );
        }
    }
    for p in &layer.pads {
        let bottom = f.elevation + p.solid().bottom;
        if bottom > deepest_ok + EPS {
            out.push(
                finding(
                    "IRC R403.1.4 footing depth",
                    Severity::Warning,
                    depth_msg("The pad", bottom),
                    depth_fix,
                )
                .at(p.center)
                .on(Target::Foundation(p.id)),
            );
        }
        if p.thickness < o.footing_min_thickness - EPS {
            out.push(
                finding(
                    "IRC R403.1.1 footing size",
                    Severity::Warning,
                    format!(
                        "The pad is {:.1}\" thick, under the {:.0}\" minimum footing thickness.",
                        p.thickness, o.footing_min_thickness
                    ),
                    "Make the pad at least 6\" thick.",
                )
                .at(p.center)
                .on(Target::Foundation(p.id)),
            );
        }
    }
}
