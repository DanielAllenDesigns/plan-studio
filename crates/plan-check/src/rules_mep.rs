//! Electrical and framing rules.
//!
//! plan-check does not depend on `plan-electrical` or `plan-framing`; both
//! layers live in the floor as JSON (`Floor.electrical`, `Floor.framing`), so
//! these rules read the few fields they need straight from the values:
//!
//! * a device: `kind` (a unit variant name, or `{"RopeLight": ..}`) and
//!   `position`;
//! * an automatic framing member: `kind`, `lumber.depth`, `length` and
//!   `transform` (`origin`, `axis_x`) in the 3D frame (X = plan x,
//!   Z = -plan y).
//!
//! The electrical rules run only when the floor has electrical devices, and
//! the framing rules only when it has framing, so a plan that has not been
//! wired or framed yet gets no findings from them.

use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::units::fmt_ft_in;
use plan_core::WallKind;
use serde::Deserialize;
use serde_json::Value;

use crate::ctx::Ctx;
use crate::{finding, Finding, Severity, Target};

/// A receptacle within this distance of a room's edge counts as in the room, inches.
const ON_EDGE: f64 = 8.0;
/// Wall length one receptacle serves (NEC 210.52(A): none farther than 6' from one), inches.
const RECEPTACLE_RUN: f64 = 144.0;
const EPS: f64 = 1e-6;

/// The fields of a stored electrical device the rules use.
struct Dev {
    kind: String,
    at: Point,
}

#[derive(Deserialize)]
struct RawDev {
    kind: Value,
    position: Point,
}

/// The devices of the floor, in storage order.
fn devices(ctx: &Ctx) -> Vec<Dev> {
    let Some(v) = ctx.floor.electrical.as_ref() else {
        return Vec::new();
    };
    let Some(list) = v.get("devices").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|d| serde_json::from_value::<RawDev>(d.clone()).ok())
        .map(|d| Dev {
            kind: match d.kind {
                Value::String(s) => s,
                Value::Object(o) => o.keys().next().cloned().unwrap_or_default(),
                _ => String::new(),
            },
            at: d.position,
        })
        .collect()
}

fn is_receptacle(kind: &str) -> bool {
    matches!(kind, "Outlet110" | "Outlet110Quad" | "Gfci" | "OutletFloor")
}

/// Is `p` in room `i` (inside its outline, or on a wall of it)?
fn in_room(ctx: &Ctx, i: usize, p: Point) -> bool {
    let poly = &ctx.rooms[i].polygon;
    if poly.len() < 3 {
        return false;
    }
    point_in_polygon(p, poly)
        || (0..poly.len()).any(|k| dist_to_segment(p, poly[k], poly[(k + 1) % poly.len()]) <= ON_EDGE)
}

fn perimeter(poly: &[Point]) -> f64 {
    (0..poly.len())
        .map(|k| poly[k].dist(poly[(k + 1) % poly.len()]))
        .sum()
}

fn is_kitchen(t: &str) -> bool {
    t.contains("kitchen")
}

fn is_laundry(t: &str) -> bool {
    t.contains("laundry") || t.contains("utility") || t.contains("mud")
}

/// NEC 210.8, 210.52 and IRC R314, R315 on the floor's devices.
pub(crate) fn electrical(ctx: &Ctx, out: &mut Vec<Finding>) {
    let devs = devices(ctx);
    if devs.is_empty() {
        return;
    }
    let has_receptacles = devs.iter().any(|d| is_receptacle(&d.kind));
    let mut any_bedroom = false;
    let mut any_garage = false;
    for (i, room) in ctx.rooms.iter().enumerate() {
        let t = &ctx.types[i];
        let name = ctx.name(i);
        let here = |f: &dyn Fn(&str) -> bool| -> Vec<&Dev> {
            devs.iter()
                .filter(|d| f(&d.kind) && in_room(ctx, i, d.at))
                .collect()
        };
        any_bedroom |= ctx.is_bedroom(i);
        any_garage |= ctx.is_garage(i);

        // GFCI protection where water is near (NEC 210.8(A)).
        if is_kitchen(t) || ctx.is_bath(i) || is_laundry(t) || ctx.is_garage(i) {
            let plain = here(&|k| matches!(k, "Outlet110" | "Outlet110Quad"));
            if !plain.is_empty() {
                out.push(
                    finding(
                        "NEC 210.8(A) GFCI protection",
                        Severity::Warning,
                        format!(
                            "{name} has {} receptacle{} without GFCI protection",
                            plain.len(),
                            if plain.len() == 1 { "" } else { "s" }
                        ),
                        "Use GFCI receptacles in kitchens, baths, laundries and garages",
                    )
                    .at(plain[0].at)
                    .on(Target::Room(i)),
                );
            }
        }

        // Smoke alarms in every sleeping room (IRC R314.3).
        if ctx.is_bedroom(i) && here(&|k| k == "SmokeDetector").is_empty() {
            out.push(
                finding(
                    "IRC R314.3 smoke alarms",
                    Severity::Warning,
                    format!("{name} has no smoke alarm"),
                    "Add a smoke detector on the ceiling of each bedroom",
                )
                .at(room.centroid)
                .on(Target::Room(i)),
            );
        }

        // A receptacle for every 12' of wall (NEC 210.52(A)).
        if has_receptacles
            && (crate::ctx::is_habitable(t) || crate::ctx::is_hall(t))
            && !t.is_empty()
        {
            let wanted = (perimeter(&room.polygon) / RECEPTACLE_RUN - EPS).ceil().max(1.0) as usize;
            let have = here(&|k| is_receptacle(k)).len();
            if have < wanted {
                out.push(
                    finding(
                        "NEC 210.52(A) receptacle spacing",
                        Severity::Warning,
                        format!(
                            "{name} has {have} receptacle{}; its walls need at least {wanted} \
                             (no point along a wall more than 6' from one)",
                            if have == 1 { "" } else { "s" }
                        ),
                        "Run Auto Place Outlets or add receptacles along the walls",
                    )
                    .at(room.centroid)
                    .on(Target::Room(i)),
                );
            }
        }
    }

    // Carbon monoxide alarms with an attached garage (IRC R315.2).
    if any_bedroom && any_garage && !devs.iter().any(|d| d.kind == "CoDetector") {
        out.push(finding(
            "IRC R315.2 carbon monoxide alarms",
            Severity::Info,
            "The floor has bedrooms and an attached garage but no carbon monoxide alarm".into(),
            "Place a CO detector outside each sleeping area",
        ));
    }
}

// ----- framing -----

/// A stored automatic framing member, reduced to what the rules read.
struct Piece {
    kind: String,
    depth: f64,
    length: f64,
    /// Start of the member and its unit direction, plan frame.
    start: Point,
    dir: Point,
    wall_id: Option<u64>,
}

impl Piece {
    fn from_value(v: &Value) -> Option<Piece> {
        let o = v.as_object()?;
        let kind = o.get("kind")?.as_str()?.to_string();
        let depth = o.get("lumber")?.get("depth")?.as_f64()?;
        let length = o.get("length")?.as_f64()?;
        let t = o.get("transform")?;
        let vec3 = |name: &str| -> Option<[f64; 3]> {
            let a = t.get(name)?.as_array()?;
            Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
        };
        let (origin, axis) = (vec3("origin")?, vec3("axis_x")?);
        Some(Piece {
            kind,
            depth,
            length,
            start: Point::new(origin[0], -origin[2]),
            dir: Point::new(axis[0], -axis[2]),
            wall_id: o.get("wall_id").and_then(Value::as_u64),
        })
    }

    fn middle(&self) -> Point {
        self.start + self.dir * (self.length / 2.0)
    }
}

/// Header depth the table calls for over an opening of `width` inches
/// (IRC R602.7 / Table R602.7(1), doubled 2x headers, 30 psf snow, 28' span):
/// 2x6 to 3', 2x8 to 4', 2x10 to 6', 2x12 beyond; over 8' an engineered beam.
fn header_depth_for(width: f64) -> f64 {
    if width <= 36.0 {
        5.5
    } else if width <= 48.0 {
        7.25
    } else if width <= 72.0 {
        9.25
    } else {
        11.25
    }
}

fn lumber_name(depth: f64) -> String {
    let nominal = (depth + 0.5).round() as u32;
    format!("2x{nominal}")
}

/// Longest unsupported joist span, inches, by joist depth (IRC R502.3.1,
/// 40 psf live load, 16" on centre, Douglas fir-larch No. 2).
fn joist_limit(depth: f64) -> f64 {
    if depth < 6.5 {
        117.0
    } else if depth < 8.5 {
        154.0
    } else if depth < 10.5 {
        195.0
    } else {
        236.0
    }
}

/// IRC R602.7 headers over exterior-wall openings and R502.3 floor joist spans.
pub(crate) fn framing(ctx: &Ctx, out: &mut Vec<Finding>) {
    let pieces: Vec<Piece> = ctx.floor.framing.iter().filter_map(Piece::from_value).collect();
    if pieces.is_empty() {
        return;
    }

    for op in &ctx.floor.openings {
        let Some(wall) = ctx.floor.wall(op.wall_id) else {
            continue;
        };
        if wall.kind != WallKind::Exterior {
            continue;
        }
        let (a, b) = (op.start_offset(), op.end_offset());
        let dir = wall.direction();
        // The headers of this wall that sit over the opening.
        let over: Vec<&Piece> = pieces
            .iter()
            .filter(|p| p.kind == "Header" && p.wall_id == Some(wall.id))
            .filter(|p| {
                let along = p.middle().sub(wall.start).dot(dir);
                along >= a - EPS && along <= b + EPS
            })
            .collect();
        let Some(depth) = over.iter().map(|p| p.depth).reduce(f64::max) else {
            continue;
        };
        let need = header_depth_for(op.width);
        if depth + EPS < need {
            let c = wall.point_at((a + b) / 2.0);
            out.push(
                finding(
                    "IRC R602.7 header size",
                    Severity::Warning,
                    format!(
                        "The {} opening is framed with a {} header; the table calls for {}",
                        fmt_ft_in(op.width),
                        lumber_name(depth),
                        lumber_name(need)
                    ),
                    "Build Framing with a larger header (Framing Defaults > Headers)",
                )
                .at(c)
                .on(Target::Opening(op.id)),
            );
        } else if op.width > 96.0 + EPS {
            let c = wall.point_at((a + b) / 2.0);
            out.push(
                finding(
                    "IRC R602.7 header size",
                    Severity::Info,
                    format!(
                        "The {} opening is wider than the header table covers",
                        fmt_ft_in(op.width)
                    ),
                    "Size this header as an engineered beam",
                )
                .at(c)
                .on(Target::Opening(op.id)),
            );
        }
    }

    // Floor joists that span farther than their size allows.
    let mut depths: Vec<f64> = pieces
        .iter()
        .filter(|p| p.kind == "Joist" || p.kind == "TrimmerJoist")
        .map(|p| p.depth)
        .collect();
    depths.sort_by(f64::total_cmp);
    depths.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    for depth in depths {
        let limit = joist_limit(depth);
        let over: Vec<&Piece> = pieces
            .iter()
            .filter(|p| p.kind == "Joist" || p.kind == "TrimmerJoist")
            .filter(|p| (p.depth - depth).abs() < 1e-6 && p.length > limit + EPS)
            .collect();
        let Some(longest) = over.iter().map(|p| p.length).reduce(f64::max) else {
            continue;
        };
        out.push(
            finding(
                "IRC R502.3.1 joist span",
                Severity::Warning,
                format!(
                    "{} {} joist{} span{} over {} (longest {})",
                    over.len(),
                    lumber_name(depth),
                    if over.len() == 1 { "" } else { "s" },
                    if over.len() == 1 { "s" } else { "" },
                    fmt_ft_in(limit),
                    fmt_ft_in(longest)
                ),
                "Use a deeper joist, add a bearing line, or reduce the span",
            )
            .at(over[0].middle()),
        );
    }
}
