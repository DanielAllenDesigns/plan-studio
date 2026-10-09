//! Sections: splitting one stair object into separate flights and landings.
//!
//! In Chief a staircase is a chain of sections (flights, landings) that edit
//! apart. Plan Studio keeps an L-shaped or U-shaped stair as one object until
//! it is taken apart:
//!
//! * [`disconnect`] (Disconnect Selected Subsection) turns an L-shaped or
//!   U-shaped stair into its flights and landing(s);
//! * [`complete_break`] (Complete Break) divides a straight flight at a riser
//!   into a lower flight, a landing and an upper flight.
//!
//! Both return the pieces from the bottom up as stairs with id `0`, except
//! the first, which keeps the id of the stair taken apart; the caller adds the
//! others to the plan and joins them.

use crate::layout::Layout;
use crate::{solve, Bullnose, Flare, Starter, Stair, StairParams, StairShape};
use plan_core::Point;

/// The smallest landing a complete break leaves, inches.
pub const MIN_BREAK_LANDING: f64 = 12.0;

/// One flight of `stair` as a straight stair of its own.
fn straight_piece(stair: &Stair, layout: &Layout, i: usize) -> Stair {
    let f = &layout.flights[i];
    let dir = layout.frame.vector(f.dir);
    let mut params = StairParams {
        shape: StairShape::Straight,
        total_rise: f64::from(f.risers) * layout.riser_height,
        riser_height_target: layout.riser_height,
        tread_depth: layout.tread_depth,
        width: f.width,
        outline: Vec::new(),
        u_gap: 0.0,
        split_landing: false,
        winder_contraction: 0.0,
        ..stair.params.clone()
    };
    if i > 0 {
        // The bottom tread's shape belongs to the first flight alone.
        params.flare = 0.0;
        params.bullnose = Bullnose::None;
        params.starter = Starter::None;
        params.flare_shape = Flare::default();
    }
    let mut piece = Stair::new(0, layout.frame.uv(f.start), dir.y.atan2(dir.x), params);
    piece.floor_elevation = stair.floor_elevation;
    piece.base = stair.base + f.base;
    piece
}

/// A landing with the plan outline `pts`, its top `top` above the floor.
fn landing_piece(stair: &Stair, pts: Vec<Point>, top: f64) -> Stair {
    let (lo, hi) = pts.iter().fold(
        (Point::new(f64::MAX, f64::MAX), Point::new(f64::MIN, f64::MIN)),
        |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        },
    );
    let params = StairParams {
        shape: StairShape::Landing {
            depth: (hi.x - lo.x).max(1.0),
        },
        width: (hi.y - lo.y).max(1.0),
        total_rise: top,
        outline: pts.clone(),
        slab_thickness: stair.params.slab_thickness,
        left_side: stair.params.left_side,
        right_side: stair.params.right_side,
        railing: stair.params.railing,
        left_railing: stair.params.left_railing,
        right_railing: stair.params.right_railing,
        ..StairParams::default()
    };
    let mut piece = Stair::new(0, pts[0], 0.0, params);
    piece.floor_elevation = stair.floor_elevation;
    piece
}

/// Disconnect Selected Subsection: the flights and landing(s) of an
/// L-shaped or U-shaped stair as separate sections, bottom to top. `None`
/// for any other stair (a winder fan is one section; a straight flight has
/// nothing to disconnect).
pub fn disconnect(stair: &Stair) -> Option<Vec<Stair>> {
    if !matches!(
        stair.params.shape,
        StairShape::LShaped { .. } | StairShape::UShaped { .. }
    ) {
        return None;
    }
    let layout = Layout::build(stair);
    if layout.flights.len() < 2 || layout.slabs.is_empty() {
        return None;
    }
    let mut out = vec![straight_piece(stair, &layout, 0)];
    out[0].id = stair.id;
    for slab in &layout.slabs {
        let pts: Vec<Point> = slab.poly.iter().map(|&p| layout.frame.uv(p)).collect();
        out.push(landing_piece(stair, pts, stair.base + slab.top));
    }
    out.push(straight_piece(stair, &layout, 1));
    Some(out)
}

/// Complete Break: a straight flight divided after `lower_risers` risers into
/// a lower flight, a landing `landing` deep and the upper flight, which
/// starts on the landing. `None` unless the stair is straight and both
/// flights keep at least two risers.
pub fn complete_break(stair: &Stair, lower_risers: u32, landing: f64) -> Option<Vec<Stair>> {
    if stair.params.shape != StairShape::Straight {
        return None;
    }
    let sol = solve(&stair.params);
    let k = lower_risers;
    if k < 2 || sol.risers < k + 2 {
        return None;
    }
    let h = sol.riser_height;
    let t = sol.tread_depth;
    let depth = landing.max(MIN_BREAK_LANDING);
    let (along, right) = {
        let a = Point::new(stair.direction.cos(), stair.direction.sin());
        (a, Point::new(a.y, -a.x))
    };
    let w = stair.params.width;
    // The lower flight's top riser line is where the landing starts.
    let s0 = f64::from(k - 1) * t;
    let at = |s: f64, lat: f64| stair.origin + along * s + right * lat;

    let mut lower = Stair {
        params: StairParams {
            total_rise: f64::from(k) * h,
            riser_height_target: h,
            ..stair.params.clone()
        },
        ..stair.clone()
    };
    lower.params.top_landing = Default::default();
    let mut upper = Stair {
        id: 0,
        origin: at(s0 + depth, 0.0),
        base: stair.base + f64::from(k) * h,
        params: StairParams {
            total_rise: f64::from(sol.risers - k) * h,
            riser_height_target: h,
            flare: 0.0,
            bullnose: Bullnose::None,
            starter: Starter::None,
            flare_shape: Flare::default(),
            ..stair.params.clone()
        },
        ..stair.clone()
    };
    upper.floor_elevation = stair.floor_elevation;
    let pts = vec![at(s0, 0.0), at(s0, w), at(s0 + depth, w), at(s0 + depth, 0.0)];
    let platform = landing_piece(stair, pts, stair.base + f64::from(k) * h);
    Some(vec![lower, platform, upper])
}
