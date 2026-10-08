//! Residential code rules beyond the first set in `rules.rs`: ceiling height,
//! stair handrails and guards, deck guards, landings and drops at exterior
//! doors, egress door height, openable glazing, smoke alarms on every level and
//! roof pitch.
//!
//! The plumbing fixture and kitchen rules are in `rules_fixtures.rs`. Every
//! rule here appends [`Finding`]s the same way the ones in `rules.rs` do, and
//! its `rule` string is listed in [`crate::rule_catalog`].

use plan_core::details::DetailsLayer;
use plan_core::geometry::polygon_centroid;
use plan_core::units::fmt_ft_in;
use plan_core::{FloorKind, OpeningKind, OpeningStyle, Point};
use plan_stairs::{footprint, solve, SideKind, StairShape};
use serde_json::Value;

use crate::ctx::{is_habitable, is_hall, Ctx, OpInfo};
use crate::geom::polygons_overlap;
use crate::{finding, Finding, Severity, Target};

const EPS: f64 = 1e-6;

/// Run every rule of this file.
pub(crate) fn run(ctx: &Ctx, out: &mut Vec<Finding>) {
    ceiling_height(ctx, out);
    stair_rails(ctx, out);
    deck_guards(ctx, out);
    exterior_doors(ctx, out);
    ventilation_area(ctx, out);
    smoke_alarm_levels(ctx, out);
    roof_pitch(ctx, out);
}

/// IRC R305.1: habitable rooms and halls need a 7'-0" ceiling; bathrooms,
/// toilet rooms and laundries 6'-8". The room's own ceiling height wins over
/// the floor's.
fn ceiling_height(ctx: &Ctx, out: &mut Vec<Finding>) {
    if ctx.floor.kind != FloorKind::Normal {
        return;
    }
    for (i, room) in ctx.rooms.iter().enumerate() {
        let t = &ctx.types[i];
        let min = if is_habitable(t) || is_hall(t) {
            ctx.opts.min_ceiling
        } else if ctx.is_bath(i) || t.contains("laundry") {
            ctx.opts.min_ceiling_bath
        } else {
            continue;
        };
        let h = room
            .name_entry(&ctx.floor.room_names)
            .and_then(|n| n.ceiling_height)
            .unwrap_or(ctx.floor.ceiling_height);
        if h < min - EPS {
            out.push(
                finding(
                    "IRC R305.1 ceiling height",
                    Severity::Error,
                    format!(
                        "{} has a {} ceiling, under the {} minimum.",
                        ctx.name(i),
                        fmt_ft_in(h),
                        fmt_ft_in(min)
                    ),
                    "Raise the ceiling height in the room specification, or lower the floor.",
                )
                .at(room.centroid)
                .on(Target::Room(i)),
            );
        }
    }
}

/// Whether a stair side carries a rail a hand can use.
fn has_rail(side: SideKind) -> bool {
    matches!(side, SideKind::Railing | SideKind::HalfWall)
}

/// IRC R311.7.8 handrails (a flight of four or more risers needs one on a
/// side) and R312.1.2 guard height (36", 34" on a stair side).
fn stair_rails(ctx: &Ctx, out: &mut Vec<Finding>) {
    let o = ctx.opts;
    for s in ctx.stairs {
        let p = &s.params;
        if matches!(
            p.shape,
            StairShape::Landing { .. } | StairShape::Ramp { .. }
        ) {
            continue;
        }
        let on = |f: Finding| f.at(s.origin).on(Target::Stair(s.id));
        let risers = solve(p).risers;
        let railed = has_rail(p.left_side) || has_rail(p.right_side);
        if risers >= o.handrail_risers && !p.handrail && !railed {
            out.push(on(finding(
                "IRC R311.7.8 handrails",
                Severity::Warning,
                format!(
                    "A stair of {risers} risers has no handrail; {} or more risers need one on at least one side.",
                    o.handrail_risers
                ),
                "Turn on Handrail in the stair specification, or set a side to Railing or Half Wall.",
            )));
        }
        if railed && p.railing.height < o.stair_guard_height - EPS {
            out.push(on(finding(
                "IRC R312.1.2 guard height",
                Severity::Error,
                format!(
                    "The stair railing is {:.1}\" high, under the {:.0}\" minimum.",
                    p.railing.height, o.stair_guard_height
                ),
                "Raise the railing to at least 34\" above the tread nosings (36\" on a landing).",
            )));
        }
    }
}

/// IRC R312.1.1: a deck or porch more than 30" above the floor or grade below
/// needs a guard.
fn deck_guards(ctx: &Ctx, out: &mut Vec<Finding>) {
    for d in &decks(ctx) {
        if d.elevation > ctx.opts.guard_drop + EPS && !d.railing {
            out.push(
                finding(
                    "IRC R312.1.1 guards",
                    Severity::Error,
                    format!(
                        "A deck {:.0}\" above the ground has no guard; guards are required above {:.0}\".",
                        d.elevation, ctx.opts.guard_drop
                    ),
                    "Turn on Railing for the deck, or lower the deck to 30\" or less.",
                )
                .at(polygon_centroid(&d.outline))
                .on(Target::Detail(d.id)),
            );
        }
    }
}

fn decks(ctx: &Ctx) -> Vec<plan_core::details::DeckPolygon> {
    ctx.floor
        .details_as::<DetailsLayer>()
        .ok()
        .flatten()
        .map(|l| l.decks)
        .unwrap_or_default()
}

/// The part of the plan just outside an exterior door: `depth` deep and as
/// wide as the door, starting at the outer face of the wall.
fn outside_rect(oi: &OpInfo, depth: f64) -> Option<Vec<Point>> {
    let w = oi.wall?;
    let c = oi.center?;
    let out_n = match (oi.left.is_some(), oi.right.is_some()) {
        (true, false) => w.normal().scale(-1.0),
        (false, true) => w.normal(),
        _ if w.kind == plan_core::WallKind::Exterior => w.exterior_normal(),
        _ => return None,
    };
    let d = w.direction().scale(oi.op.width * 0.5);
    let near = c.add(out_n.scale(w.thickness * 0.5 + 0.5));
    let far = near.add(out_n.scale(depth - 1.0));
    Some(vec![near.sub(d), near.add(d), far.add(d), far.sub(d)])
}

/// IRC R311.2 egress door size, R311.3 landings at doors and R312.1.1 drops:
/// an exterior door is 80" high, a door on an upper floor needs a deck or
/// balcony outside it, and a stair at a door needs a landing.
fn exterior_doors(ctx: &Ctx, out: &mut Vec<Finding>) {
    let o = ctx.opts;
    let stair_polys: Vec<(bool, Vec<Point>)> = ctx
        .stairs
        .iter()
        .map(|s| {
            (
                matches!(s.params.shape, StairShape::Landing { .. }),
                footprint(s),
            )
        })
        .collect();
    let decks = decks(ctx);
    for oi in ctx.ops.iter().filter(|x| x.op.kind == OpeningKind::Door) {
        let op = oi.op;
        let touches_garage = [oi.left, oi.right]
            .into_iter()
            .flatten()
            .any(|r| ctx.is_garage(r));
        if !oi.exterior()
            || touches_garage
            || op.style == OpeningStyle::Garage
            || (oi.left.is_none() && oi.right.is_none())
        {
            continue;
        }
        let at = oi.center.unwrap_or(Point::ZERO);
        let on = Target::Opening(op.id);
        if op.height < o.entry_door_height - EPS {
            out.push(
                finding(
                    "IRC R311.2 egress door height",
                    Severity::Warning,
                    format!(
                        "Exterior door {} is {:.0}\" high, under the {:.0}\" egress door height.",
                        op.id, op.height, o.entry_door_height
                    ),
                    "Use a 6'-8\" (80\") exterior door; the egress door is 36\" x 80\".",
                )
                .at(at)
                .on(on),
            );
        }
        let Some(rect) = outside_rect(oi, o.exit_landing) else {
            continue;
        };
        if ctx.floor.kind == FloorKind::Normal && ctx.floor.elevation > o.guard_drop {
            let platform = decks.iter().any(|d| polygons_overlap(&rect, &d.outline))
                || stair_polys
                    .iter()
                    .any(|(landing, p)| *landing && polygons_overlap(&rect, p));
            if !platform {
                out.push(
                    finding(
                        "IRC R312.1.1 door to a drop",
                        Severity::Error,
                        format!(
                            "Door {} opens {:.0}\" above grade with no deck, balcony or landing outside it.",
                            op.id, ctx.floor.elevation
                        ),
                        "Add a deck or balcony with a guard outside the door, or replace the door with a window.",
                    )
                    .at(at)
                    .on(on),
                );
            }
        } else {
            let hits_stair = stair_polys
                .iter()
                .any(|(landing, p)| !*landing && polygons_overlap(&rect, p));
            let has_landing = stair_polys
                .iter()
                .any(|(landing, p)| *landing && polygons_overlap(&rect, p));
            if hits_stair && !has_landing {
                out.push(
                    finding(
                        "IRC R311.3 landing at door",
                        Severity::Warning,
                        format!(
                            "A stair starts within {:.0}\" of exterior door {} with no landing between.",
                            o.exit_landing, op.id
                        ),
                        "Add a landing at least 36\" deep and as wide as the door between the door and the first step.",
                    )
                    .at(at)
                    .on(on),
                );
            }
        }
    }
}

/// How much of a window or door's area opens for air.
fn operable_fraction(style: OpeningStyle) -> f64 {
    match style {
        OpeningStyle::Fixed | OpeningStyle::WallNiche | OpeningStyle::Garage => 0.0,
        OpeningStyle::Window
        | OpeningStyle::SlidingWindow
        | OpeningStyle::Sliding
        | OpeningStyle::BayWindow
        | OpeningStyle::BowWindow
        | OpeningStyle::BoxWindow => 0.5,
        _ => 1.0,
    }
}

/// IRC R303.1 natural ventilation: the openable area of windows and exterior
/// doors should be 4% of the floor area of a habitable room (Info for a
/// kitchen). Mechanical ventilation is the exception (R303.3).
fn ventilation_area(ctx: &Ctx, out: &mut Vec<Finding>) {
    for (i, room) in ctx.rooms.iter().enumerate() {
        if !is_habitable(&ctx.types[i]) || room.area_sq_in <= 0.0 {
            continue;
        }
        let open: f64 = [OpeningKind::Window, OpeningKind::Door]
            .into_iter()
            .flat_map(|k| ctx.exterior_openings(i, k))
            .map(|oi| oi.op.width * oi.op.height / 144.0 * operable_fraction(oi.op.style))
            .sum();
        let need = ctx.opts.vent_ratio * room.area_sq_ft();
        if open < need - EPS {
            let sev = if ctx.types[i].contains("kitchen") {
                Severity::Info
            } else {
                Severity::Warning
            };
            out.push(
                finding(
                    "IRC R303.1 ventilation area",
                    sev,
                    format!(
                        "{} has {open:.1} sq ft of openable glazing; {:.0}% of its {:.0} sq ft floor is {need:.1} sq ft.",
                        ctx.name(i),
                        100.0 * ctx.opts.vent_ratio,
                        room.area_sq_ft()
                    ),
                    "Use operable windows (casement, awning, hung) or add one; or provide mechanical ventilation.",
                )
                .at(room.centroid)
                .on(Target::Room(i)),
            );
        }
    }
}

/// The kinds of the electrical devices stored on the floor.
fn device_kinds(ctx: &Ctx) -> Vec<String> {
    let Some(list) = ctx
        .floor
        .electrical
        .as_ref()
        .and_then(|v| v.get("devices"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|d| match d.get("kind")? {
            Value::String(s) => Some(s.clone()),
            Value::Object(m) => m.keys().next().cloned(),
            _ => None,
        })
        .collect()
}

/// IRC R314.3: a smoke alarm on every level. Runs once the floor has been
/// wired (has electrical devices) and has a habitable room.
fn smoke_alarm_levels(ctx: &Ctx, out: &mut Vec<Finding>) {
    let kinds = device_kinds(ctx);
    if kinds.is_empty()
        || kinds.iter().any(|k| k == "SmokeDetector")
        || !(0..ctx.rooms.len()).any(|i| is_habitable(&ctx.types[i]))
    {
        return;
    }
    out.push(finding(
        "IRC R314.3 smoke alarm on every level",
        Severity::Warning,
        format!(
            "{} has no smoke alarm; every level needs one.",
            ctx.floor.name
        ),
        "Add a smoke detector in the hall outside the bedrooms, and one in each bedroom.",
    ));
}

/// IRC R905.2.2 and R905.1.1 roof pitch: shingles need 2:12, and anything
/// under 4:12 needs double underlayment (Info); a roof steeper than 12:12 is
/// an advisory (Info).
fn roof_pitch(ctx: &Ctx, out: &mut Vec<Finding>) {
    let o = ctx.opts;
    for v in &ctx.floor.roofs {
        if v.get("kind").and_then(Value::as_str) != Some("plane") {
            continue;
        }
        let (Some(id), Some(pitch)) = (
            v.get("id").and_then(Value::as_u64),
            v.get("pitch").and_then(Value::as_f64),
        ) else {
            continue;
        };
        let base: Option<Vec<Point>> = v
            .get("baseline")
            .and_then(|b| serde_json::from_value(b.clone()).ok());
        let at = base
            .filter(|b| b.len() == 2)
            .map(|b| Point::lerp(b[0], b[1], 0.5));
        let mut push = |rule: &'static str, sev, msg: String, fix: &str| {
            let mut f = finding(rule, sev, msg, fix).on(Target::Roof(id));
            if let Some(p) = at {
                f = f.at(p);
            }
            out.push(f);
        };
        if pitch < o.roof_pitch_min - EPS {
            push(
                "IRC R905.2.2 roof slope",
                Severity::Warning,
                format!(
                    "Roof plane {id} is {pitch:.1}:12, under the {:.0}:12 minimum for asphalt shingles.",
                    o.roof_pitch_min
                ),
                "Raise the pitch to 2:12 or more, or specify a low-slope membrane roof.",
            );
        } else if pitch < o.roof_pitch_underlay - EPS {
            push(
                "IRC R905.1.1 underlayment",
                Severity::Info,
                format!(
                    "Roof plane {id} is {pitch:.1}:12; shingles under {:.0}:12 need double underlayment.",
                    o.roof_pitch_underlay
                ),
                "Note double underlayment on the roof plan, or raise the pitch to 4:12.",
            );
        } else if pitch > o.roof_pitch_steep + EPS {
            push(
                "Roof pitch: steep slope",
                Severity::Info,
                format!(
                    "Roof plane {id} is {pitch:.1}:12, steeper than {:.0}:12.",
                    o.roof_pitch_steep
                ),
                "Plan for steep-slope fastening, roof access and fall protection, or lower the pitch.",
            );
        }
    }
}
