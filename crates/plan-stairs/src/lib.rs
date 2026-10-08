//! plan-stairs: the parametric stair engine.
//!
//! Given a [`Stair`] (plan position, direction and [`StairParams`]) this crate
//! computes a code-checked rise/run ([`solve`]), the 2D plan symbol
//! ([`plan_symbol`]), the 3D meshes ([`meshes`]), the plan footprint
//! ([`footprint`]) and the arrival point ([`top_point`]).
//!
//! All lengths are inches. The plan frame is Y-up; the 3D frame is X right,
//! Y up, Z = -plan y (see `plan-3d`).

mod deck;
mod layout;
mod model3d;
mod plan;
mod railing;

use layout::Layout;
use plan_core::{Id, Point};
use serde::{Deserialize, Serialize};

pub use deck::{deck_edge_railing, Deck};
pub use model3d::{meshes, tagged_meshes, StairPart};
pub use plan::{plan_symbol, Stroke};
pub use railing::{
    plan_symbol_railing, railing_meshes, railing_segments, stair_railing, stair_railing_geometry,
    NewelParams, RailSide, RailStyle, RailingGeometry, RailingParams, StairRailingGeometry,
};

/// Maximum riser height, IRC R311.7.5.1.
pub const MAX_RISER: f64 = 7.75;
/// Minimum riser height used for the clamp.
pub const MIN_RISER: f64 = 4.0;
/// Minimum tread depth, IRC R311.7.5.2.
pub const MIN_TREAD: f64 = 10.0;
/// Minimum stair width, IRC R311.7.1.
pub const MIN_WIDTH: f64 = 36.0;
/// Minimum headroom, IRC R311.7.2 (6'-8").
pub const MIN_HEADROOM: f64 = 80.0;
/// Tolerance for float comparisons against code limits.
const EPS: f64 = 1e-9;

/// Which way a flight turns at a landing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Turn {
    /// Counter-clockwise in plan.
    Left,
    /// Clockwise in plan.
    Right,
}

/// Overall stair configuration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StairShape {
    /// One straight flight.
    Straight,
    /// Two flights at 90 degrees joined by a landing.
    LShaped {
        /// Regular treads in the first flight (clamped to what the rise allows).
        treads_before_landing: u32,
    },
    /// Two parallel flights joined by a 180 degree landing.
    UShaped {
        /// Regular treads in the first flight (clamped to what the rise allows).
        treads_before_landing: u32,
    },
    /// L-shaped stair whose landing is replaced by pie-shaped winder treads.
    Winder {
        /// Number of winder treads in the turn (at least 1).
        winders: u32,
    },
    /// A ramp instead of steps.
    Ramp {
        /// Run per inch of rise (12 means 1:12).
        slope_1_in: f64,
    },
}

/// The inputs of the Stair Specification dialog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StairParams {
    /// Floor-to-floor rise.
    pub total_rise: f64,
    /// Clear stair width.
    pub width: f64,
    /// Tread depth (run per step, excluding nosing).
    pub tread_depth: f64,
    /// Desired riser height; the solver rounds to a whole number of risers.
    pub riser_height_target: f64,
    /// Tread overhang past the riser below.
    pub nosing: f64,
    /// Tread board thickness.
    pub tread_thickness: f64,
    /// Riser board thickness.
    pub riser_thickness: f64,
    /// Perpendicular depth of the stringers.
    pub stringer_depth: f64,
    /// Omit the riser boards.
    pub open_risers: bool,
    /// Available headroom (code minimum 80").
    pub headroom_min: f64,
    /// Flight configuration.
    pub shape: StairShape,
    /// Landing depth in the direction of travel (L and U shapes).
    pub landing_depth: f64,
    /// Turn direction for L, U and winder stairs.
    pub turn: Turn,
    /// Add a handrail along both sides of each flight.
    pub handrail: bool,
}

impl Default for StairParams {
    fn default() -> Self {
        Self {
            total_rise: 109.125 + 1.0,
            width: 36.0,
            tread_depth: 10.0,
            riser_height_target: 7.5,
            nosing: 1.0,
            tread_thickness: 1.0,
            riser_thickness: 0.75,
            stringer_depth: 11.25,
            open_risers: false,
            headroom_min: 80.0,
            shape: StairShape::Straight,
            landing_depth: 36.0,
            turn: Turn::Left,
            handrail: false,
        }
    }
}

/// Result of [`solve`]: the whole-number step layout and its code check.
#[derive(Debug, Clone, PartialEq)]
pub struct StairSolution {
    /// Number of risers (0 for a ramp).
    pub risers: u32,
    /// Actual riser height, `total_rise / risers`.
    pub riser_height: f64,
    /// Number of treads (the landing counts as one tread; 0 for a ramp).
    pub treads: u32,
    /// Tread depth used.
    pub tread_depth: f64,
    /// Horizontal run along the path, including landings.
    pub total_run: f64,
    /// True when every hard IRC limit is met.
    pub code_ok: bool,
    /// Code violations and comfort-rule notes.
    pub warnings: Vec<String>,
}

/// A stair placed in a plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stair {
    /// Object id.
    pub id: Id,
    /// Plan position of the bottom riser's left corner (left when facing the direction of travel).
    pub origin: Point,
    /// Direction of travel of the first flight, radians.
    pub direction: f64,
    /// Dimensions and style.
    pub params: StairParams,
    /// Elevation of the floor the stair starts from.
    pub floor_elevation: f64,
}

impl Stair {
    /// A stair starting at floor elevation 0.
    pub fn new(id: Id, origin: Point, direction: f64, params: StairParams) -> Self {
        Self {
            id,
            origin,
            direction,
            params,
            floor_elevation: 0.0,
        }
    }
}

/// The landing length actually used: never shallower than the stair is wide.
pub(crate) fn effective_landing(params: &StairParams) -> f64 {
    params.landing_depth.max(params.width)
}

/// How a stepped stair divides into flights and a turn.
pub(crate) struct Split {
    /// Regular treads in the first flight.
    pub t1: u32,
    /// Regular treads in the second flight.
    pub t2: u32,
    /// Winder treads (0 when the turn is a flat landing).
    pub winders: u32,
}

/// Split `risers` into two flights, or `None` for a straight run (also when
/// there are too few risers for the requested turn).
pub(crate) fn split(params: &StairParams, risers: u32) -> Option<Split> {
    let (pad, winders, asked) = match params.shape {
        StairShape::LShaped {
            treads_before_landing,
        }
        | StairShape::UShaped {
            treads_before_landing,
        } => (2, 0, Some(treads_before_landing)),
        StairShape::Winder { winders } => (winders.max(1) + 1, winders.max(1), None),
        StairShape::Straight | StairShape::Ramp { .. } => return None,
    };
    if risers < pad {
        return None;
    }
    let regular = risers - pad;
    let t1 = asked.unwrap_or(regular / 2).min(regular);
    Some(Split {
        t1,
        t2: regular - t1,
        winders,
    })
}

/// Compute the whole-number rise/run and check it against the IRC.
///
/// `risers = round(total_rise / riser_height_target)` clamped so the riser
/// height lies within 4"-7.75"; `treads = risers - 1` (a landing counts as one
/// tread). The comfort rule `24 <= 2R + T <= 25` only produces a warning.
pub fn solve(params: &StairParams) -> StairSolution {
    let mut warnings = Vec::new();
    let mut code_ok = true;

    if let StairShape::Ramp { slope_1_in } = params.shape {
        let run = params.total_rise.max(0.0) * slope_1_in.max(0.0);
        if slope_1_in < 12.0 {
            code_ok = false;
            warnings.push(format!(
                "ramp slope 1:{slope_1_in:.1} is steeper than the 1:12 maximum"
            ));
        }
        if params.total_rise > 30.0 {
            code_ok = false;
            warnings.push("ramp rise exceeds 30\" between landings".into());
        }
        if params.width < MIN_WIDTH - EPS {
            code_ok = false;
            warnings.push(format!(
                "width {:.2}\" is below the 36\" minimum",
                params.width
            ));
        }
        return StairSolution {
            risers: 0,
            riser_height: 0.0,
            treads: 0,
            tread_depth: 0.0,
            total_run: run,
            code_ok,
            warnings,
        };
    }

    let rise = params.total_rise.max(0.0);
    let target = if params.riser_height_target > 0.0 {
        params.riser_height_target
    } else {
        7.5
    };
    let min_risers = ((rise / MAX_RISER - EPS).ceil().max(1.0)) as u32;
    let max_risers = ((rise / MIN_RISER + EPS).floor().max(1.0)) as u32;
    let risers =
        ((rise / target).round().max(1.0) as u32).clamp(min_risers, max_risers.max(min_risers));
    let riser_height = rise / f64::from(risers);

    if riser_height > MAX_RISER + EPS {
        code_ok = false;
        warnings.push(format!(
            "riser {riser_height:.3}\" exceeds the 7 3/4\" maximum"
        ));
    }
    if riser_height < MIN_RISER - EPS {
        warnings.push(format!(
            "riser {riser_height:.3}\" is below the 4\" minimum"
        ));
    }
    if params.tread_depth < MIN_TREAD - EPS {
        code_ok = false;
        warnings.push(format!(
            "tread depth {:.2}\" is below the 10\" minimum",
            params.tread_depth
        ));
    }
    if params.width < MIN_WIDTH - EPS {
        code_ok = false;
        warnings.push(format!(
            "width {:.2}\" is below the 36\" minimum",
            params.width
        ));
    }
    if params.headroom_min < MIN_HEADROOM - EPS {
        code_ok = false;
        warnings.push(format!(
            "headroom {:.2}\" is below the 80\" minimum",
            params.headroom_min
        ));
    }
    let comfort = 2.0 * riser_height + params.tread_depth;
    if !(24.0 - EPS..=25.0 + EPS).contains(&comfort) {
        warnings.push(format!(
            "2R+T = {comfort:.2}\" is outside the 24-25\" comfort range"
        ));
    }

    let treads = risers - 1;
    let straight_run = f64::from(treads) * params.tread_depth;
    let total_run = match (split(params, risers), params.shape) {
        (Some(s), StairShape::Winder { .. }) => {
            f64::from(s.t1 + s.t2) * params.tread_depth + params.width
        }
        (Some(s), _) => f64::from(s.t1 + s.t2) * params.tread_depth + effective_landing(params),
        (None, StairShape::Straight) => straight_run,
        (None, _) => {
            warnings
                .push("too few risers for the requested turn; drawn as a straight stair".into());
            straight_run
        }
    };

    StairSolution {
        risers,
        riser_height,
        treads,
        tread_depth: params.tread_depth,
        total_run,
        code_ok,
        warnings,
    }
}

/// Plan polygon covering all flights and landings (for stairwell openings).
pub fn footprint(stair: &Stair) -> Vec<Point> {
    let layout = Layout::build(stair);
    layout
        .footprint
        .iter()
        .map(|&(u, v)| layout.frame.point(u, v))
        .collect()
}

/// Where the stair arrives: the centre of the top riser line and its elevation.
pub fn top_point(stair: &Stair) -> (Point, f64) {
    let layout = Layout::build(stair);
    let last = layout
        .flights
        .last()
        .expect("a layout always has at least one flight");
    let (u, v) = last.at(last.len, last.width / 2.0);
    (
        layout.frame.point(u, v),
        stair.floor_elevation + layout.total_rise,
    )
}

#[cfg(test)]
mod tests;
