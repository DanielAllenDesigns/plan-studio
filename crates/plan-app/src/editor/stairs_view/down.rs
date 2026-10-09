//! Downward stairs and stairs to a deck (CB-105, CB-109, CB-110 in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! A stair drawn with Alt (or the right mouse button) runs downward from the
//! point where it was pressed: the press is the top, at the floor, and the
//! release is the bottom. Its steps are the same as an upward stair's; its
//! plan arrow starts at the top and says DN ([`StairParams::down`]). Its
//! bottom is where the terrain is when the plan has terrain there, else a
//! stoop-height drop ([`DEFAULT_DROP`]).

use super::*;
use plan_core::deck::{ccw, deck_rooms};
use plan_core::geometry::dist_to_segment;
use plan_core::split_level::room_level;
use plan_core::Room;

/// How far a downward stair drops when the plan has no terrain under its
/// bottom: a 30" stoop.
pub const DEFAULT_DROP: f64 = 30.0;
/// Click Stairs and a plain Alt-click run this many default treads at most.
const CLICK_TREADS: f64 = 10.0;
/// How far from a room an edge may be and still take the Stairs to Deck click.
pub const DECK_PICK_TOL: f64 = 240.0;

/// The drop from `top_z` (absolute elevation) to the terrain at `bottom`,
/// when the plan has terrain there and it lies at least a minimum riser
/// below the top.
pub fn terrain_drop(project: &Project, bottom: Point, top_z: f64) -> Option<f64> {
    let z = super::super::site_view::terrain_elevation_at(project, bottom)?;
    let d = top_z - z;
    (d >= plan_stairs::MIN_RISER).then_some(d)
}

fn straight_family(kind: StairKind) -> bool {
    matches!(
        kind,
        StairKind::Draw | StairKind::Click | StairKind::Straight | StairKind::Ramp
    )
}

/// A downward stair for the tool `kind`: `a` is where the pointer went down
/// (the top, at the floor), `b` where it was released (the bottom). Curved
/// kinds keep their centre-and-radius meaning and are only marked downward.
pub fn build_down(
    project: &Project,
    fl: usize,
    kind: StairKind,
    turn: Turn,
    a: Point,
    b: Option<Point>,
) -> StairObj {
    let floor = &project.floors[fl];
    let top_z = floor.elevation;
    let b = b.filter(|b| a.dist(*b) > 1e-6);
    let reversed = straight_family(kind) || matches!(kind, StairKind::LShaped | StairKind::UShaped);
    let (start, end) = match (reversed, b) {
        (true, Some(b)) => (b, Some(a)),
        _ => (a, b),
    };
    let mut o = build(project, fl, kind, turn, start, end);
    if matches!(kind, StairKind::Landing) {
        return o;
    }
    // Where the bottom lands decides the drop.
    let bottom = if reversed { start } else { o.bottom_center() };
    let drop = match b {
        Some(_) => terrain_drop(project, bottom, top_z),
        None => None,
    }
    .unwrap_or(DEFAULT_DROP)
    .max(plan_stairs::MIN_RISER);
    o.stair.params.total_rise = drop;
    o.stair.params.down = true;
    o.stair.base = -drop;
    o.x.story_rise = drop;
    if kind == StairKind::Ramp {
        // A ramp keeps the run that was dragged: the slope follows.
        if let (Some(b), StairShape::Ramp { .. }) = (b, o.stair.params.shape) {
            o.stair.params.shape = StairShape::Ramp {
                slope_1_in: (a.dist(b) / drop).max(1.0),
            };
        }
        return o;
    }
    if reversed {
        let treads = solve(&o.stair.params).treads.max(1);
        match b {
            Some(b) => {
                o.stair.params.tread_depth = (a.dist(b) / f64::from(treads)).max(1.0);
            }
            None => {
                // A click: the stair runs from `a` down the screen.
                let run = f64::from(treads) * o.stair.params.tread_depth;
                let along = o.along();
                o.stair.origin = o.stair.origin - along * run;
            }
        }
    }
    o
}

/// A stair from the edge of a deck (or of any room) down to the terrain.
///
/// `click` picks the room (the one it is in, else the nearest within
/// [`DECK_PICK_TOL`]) and the edge (the nearest to it); the stair is centred
/// on the click's projection along that edge, kept inside the edge. Its top is
/// the room's finished floor and its bottom is the terrain when there is any,
/// else the deck's height above grade.
pub fn build_to_deck(
    project: &Project,
    fl: usize,
    rooms: &[Room],
    click: Point,
) -> Result<StairObj, String> {
    let floor = &project.floors[fl];
    if rooms.is_empty() {
        return Err("Stairs to Deck: draw a deck or a room first".into());
    }
    let decks = deck_rooms(floor, rooms);
    let near = |poly: &[Point]| -> f64 {
        let n = poly.len();
        (0..n)
            .map(|i| dist_to_segment(click, poly[i], poly[(i + 1) % n]))
            .fold(f64::INFINITY, f64::min)
    };
    // The room: the one holding the click (a deck first), else the nearest.
    let inside = |r: &Room| point_in_polygon(click, &r.polygon);
    let pick = rooms
        .iter()
        .filter(|r| inside(r))
        .min_by_key(|r| {
            let deck = r
                .name_entry(&floor.room_names)
                .is_some_and(|n| n.deck.is_some());
            (!deck, r.area_sq_in as i64)
        })
        .or_else(|| {
            rooms
                .iter()
                .filter(|r| near(&r.polygon) <= DECK_PICK_TOL)
                .min_by(|x, y| near(&x.polygon).total_cmp(&near(&y.polygon)))
        })
        .ok_or("Stairs to Deck: click on or near a deck or room")?;
    let deck = decks.iter().find(|d| pick.contains(d.name.anchor)).cloned();
    let outline = match &deck {
        Some(d) => d.outline.clone(),
        None if pick.inner_polygon.len() >= 3 => ccw(&pick.inner_polygon),
        None => ccw(&pick.polygon),
    };
    let n = outline.len();
    let (i, _) = (0..n)
        .map(|i| (i, dist_to_segment(click, outline[i], outline[(i + 1) % n])))
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .ok_or("Stairs to Deck: the room has no edge")?;
    let (p, q) = (outline[i], outline[(i + 1) % n]);
    let len = p.dist(q);
    if len < 24.0 {
        return Err("Stairs to Deck: that edge is too short for stairs".into());
    }
    let along_edge = (q - p).normalized();
    // The outline is counter-clockwise: the right side is outward.
    let out = Point::new(along_edge.y, -along_edge.x);

    let level = room_level(floor, pick);
    let top_z = floor.elevation + level;
    let (width0, tread0, grade0) = match &deck {
        Some(d) => (
            d.spec.stairs.width,
            d.spec.stairs.tread,
            d.spec.framing.height_above_grade,
        ),
        None => (DEFAULT_WIDTH, CLICK_TREAD, DEFAULT_DROP),
    };
    let width = width0.min(len - 6.0).max(24.0);
    let mut params = StairParams {
        width,
        tread_depth: tread0.max(8.0),
        down: true,
        ..StairParams::default()
    };
    // Where along the edge: the click's projection, kept inside the edge.
    let t = (click - p)
        .dot(along_edge)
        .clamp(width * 0.5, len - width * 0.5);
    let top_centre = p + along_edge * t;
    let mut rise = grade0.max(plan_stairs::MIN_RISER);
    for _ in 0..3 {
        params.total_rise = rise;
        let run = solve(&params).total_run;
        let bottom = top_centre + out * run;
        match terrain_drop(project, bottom, top_z) {
            Some(d) if (d - rise).abs() > 0.01 => rise = d,
            _ => break,
        }
    }
    params.total_rise = rise;
    // Exterior stairs: a guard with balusters over 30", else a wall rail.
    let side = if rise > 30.0 {
        SideKind::Railing
    } else {
        SideKind::Handrail
    };
    params.left_side = side;
    params.right_side = side;
    super::super::code::legalize_stair_params(&mut params);
    params.total_rise = rise;
    let run = solve(&params).total_run;
    let bottom_centre = top_centre + out * run;
    let travel = -out;
    let right = Point::new(travel.y, -travel.x);
    let mut stair = Stair::new(
        0,
        bottom_centre - right * (width * 0.5),
        travel.angle(),
        params,
    );
    stair.floor_elevation = floor.elevation;
    stair.base = level - rise;
    let x = StairExtras {
        story_rise: rise,
        ..StairExtras::default()
    };
    Ok(StairObj { stair, x })
}
