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
mod landing;
mod landing_rules;
mod layout;
mod model3d;
mod options;
mod plan;
mod railing;
mod sections;
mod spec;

use layout::Layout;
use plan_core::{Id, Point};
use serde::{Deserialize, Serialize};

pub use deck::{deck_edge_railing, Deck};
pub use landing::polygon_slab;
pub use landing_rules::{
    adjacent_edges, adjacent_height, are_adjacent, auto_height, auto_thickness, short_edge,
    ADJACENT_TOLERANCE, FREE_STANDING_THICKNESS, MIN_SHORT_EDGE,
};
pub use model3d::{meshes, tagged_meshes, tagged_meshes_skipping, StairPart};
pub use options::{
    ArrowStyle, BreakStyle, DisplayRule, EdgeRail, Flare, HandrailOptions, PlanOptions,
    PostProfile, RadiusRef, RampOptions, Runner, Starter, StringerOptions, TopLanding, ViewMode,
    Walkline,
};
pub use plan::{plan_symbol, Stroke};
pub use railing::{
    landing_edges, landing_guards, plan_symbol_railing, railing_meshes, railing_segments,
    stair_half_wall, stair_half_wall_skipping, stair_posts, stair_railing, stair_railing_geometry,
    stair_railing_skipping, LandingGuard, NewelParams, PostPlacement, PostSkip, RailSide,
    RailStyle, RailingGeometry, RailingParams, StairPosts, StairRailingGeometry, GUARD_HEIGHT,
    MAX_BALUSTER_CLEAR, STAIR_RAIL_HEIGHT,
};
pub use sections::{complete_break, disconnect, MIN_BREAK_LANDING};
pub use spec::{
    best_fit, fit_status, info, merge, rise_angle, spec_rows, BestFit, FitStatus, Info, LockEnd,
    MergeError, SpecRow, TreadMode, BEST_FIT_RISER, MERGE_TOLERANCE, SPEC_ROWS,
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
/// Maximum rise of one ramp run between landings, IBC 1012.2 (30").
pub const RAMP_MAX_RISE: f64 = 30.0;
/// The steepest ramp slope code allows, as run per inch of rise (IBC
/// 1012.2: 1:12).
pub const RAMP_MIN_SLOPE: f64 = 12.0;
/// Length of the flat landing between two ramp runs, IBC 1012.6 (60").
pub const RAMP_LANDING: f64 = 60.0;
/// Narrowest tread of a curved stair at the inside edge, IRC R311.7.5.2.1 (6").
pub const MIN_CURVED_TREAD_INSIDE: f64 = 6.0;
/// Spiral stairs, IRC R311.7.10.1: maximum riser (9 1/2").
pub const SPIRAL_MAX_RISER: f64 = 9.5;
/// Spiral stairs: minimum tread depth at the walking line (6 3/4").
pub const SPIRAL_MIN_TREAD: f64 = 6.75;
/// Spiral stairs: minimum clear width (26").
pub const SPIRAL_MIN_WIDTH: f64 = 26.0;
/// Spiral stairs: minimum headroom (6'-6").
pub const SPIRAL_MIN_HEADROOM: f64 = 78.0;
/// Radius of the centre pole of a spiral stair the tools draw, inches.
pub const SPIRAL_POLE_RADIUS: f64 = 2.0;

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
    /// A ramp instead of steps. A rise over 30" is split into runs of at most
    /// 30" joined by flat 60" landings.
    Ramp {
        /// Run per inch of rise (12 means 1:12).
        slope_1_in: f64,
    },
    /// Treads fanned around a centre point (a curved stair). The turn
    /// direction is [`StairParams::turn`]; the sweep follows from the number
    /// of treads and the tread depth on the walking line.
    Curved {
        /// Distance from the centre to the inside edge of the stair.
        inner_radius: f64,
    },
    /// A flat platform, `depth` along the direction of travel by the width
    /// (or the polygon in [`StairParams::outline`]). Its top sits
    /// [`StairParams::total_rise`] above the floor.
    Landing {
        /// Length along the direction of travel.
        depth: f64,
    },
}

/// How the stringers under the treads are built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StringerStyle {
    /// A full board whose top edge follows the nosing line (a housed or
    /// closed stringer).
    #[default]
    Closed,
    /// A notched (cut) stringer: the steps are cut out of the board.
    Open,
    /// No stringers (the treads span between walls).
    None,
}

/// What stands on one side of the stair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SideKind {
    /// Open and unguarded.
    #[default]
    None,
    /// A full-height wall along the side.
    Wall,
    /// A railing with newels and balusters (see [`StairParams::railing`]).
    Railing,
    /// A half-wall with a cap rail.
    HalfWall,
    /// A wall-mounted handrail only (34" above the nosing line): no guard,
    /// newels or balusters. The side beside a wall.
    Handrail,
}

impl SideKind {
    /// The names of the Stair Specification, in menu order.
    pub const ALL: [SideKind; 5] = [
        SideKind::None,
        SideKind::Wall,
        SideKind::Railing,
        SideKind::HalfWall,
        SideKind::Handrail,
    ];

    /// Does this side stop a fall? A railing, a half-wall and a wall do; a
    /// handrail is for gripping and does not.
    pub fn is_guard(self) -> bool {
        matches!(
            self,
            SideKind::Railing | SideKind::HalfWall | SideKind::Wall
        )
    }

    /// Chief's label.
    pub fn name(self) -> &'static str {
        match self {
            SideKind::None => "None",
            SideKind::Wall => "Wall",
            SideKind::Railing => "Railing",
            SideKind::HalfWall => "Half Wall",
            SideKind::Handrail => "Handrail",
        }
    }
}

/// Which ends of the bottom tread are rounded off (a bullnose starter step).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Bullnose {
    /// Square ends (or the flare, if [`StairParams::flare`] is set).
    #[default]
    None,
    /// The left end is a half-round the depth of the tread.
    Left,
    /// The right end is a half-round the depth of the tread.
    Right,
    /// Both ends.
    Both,
}

impl Bullnose {
    pub const ALL: [Bullnose; 4] = [
        Bullnose::None,
        Bullnose::Left,
        Bullnose::Right,
        Bullnose::Both,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Bullnose::None => "None",
            Bullnose::Left => "Left End",
            Bullnose::Right => "Right End",
            Bullnose::Both => "Both Ends",
        }
    }

    pub fn left(self) -> bool {
        matches!(self, Bullnose::Left | Bullnose::Both)
    }

    pub fn right(self) -> bool {
        matches!(self, Bullnose::Right | Bullnose::Both)
    }
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
    /// Stringer construction.
    pub stringer: StringerStyle,
    /// The left side, facing the direction of travel.
    pub left_side: SideKind,
    /// The right side, facing the direction of travel.
    pub right_side: SideKind,
    /// Rails, newels and balusters of the sides marked [`SideKind::Railing`]
    /// (the half-wall height of a [`SideKind::HalfWall`] side is unused: the
    /// wall rises to the rail).
    pub railing: RailingParams,
    /// Thickness of landings, winder treads and ramp slabs.
    pub slab_thickness: f64,
    /// The corners of a polygon landing in plan (empty: a rectangle `width`
    /// by the landing depth).
    pub outline: Vec<Point>,
    /// A flared bottom tread (the apron): the first tread bulges this far
    /// past the stair on each side in a quarter-ellipse. `0` is a plain
    /// tread. Straight, L, U and winder stairs only.
    pub flare: f64,
    /// A bullnose bottom tread: the chosen ends are half-rounds of the
    /// tread's depth (they win over `flare` on that end).
    pub bullnose: Bullnose,
    /// Rails, newels and balusters of the left side when they differ from
    /// [`StairParams::railing`] (Newels/Balusters and Rails tabs, Left side).
    pub left_railing: Option<RailingParams>,
    /// The same for the right side.
    pub right_railing: Option<RailingParams>,
    /// A spiral stair: a [`StairShape::Curved`] stair around a centre pole
    /// (`inner_radius` is the pole's radius) that follows the spiral-stair
    /// code limits (9 1/2" risers, 6 3/4" treads at the walking line, 26"
    /// clear width) and gets a pole in 3D.
    pub spiral: bool,
    /// U-shaped stairs: the gap between the two flights (0 has them side by
    /// side).
    pub u_gap: f64,
    /// U-shaped stairs: two landings (one at the end of each flight) with the
    /// turn between them instead of one wide landing.
    pub split_landing: bool,
    /// Winders: the narrowest a tread may get where it meets the inside
    /// corner (Max Tread Contraction); 0 leaves the fan as it is.
    pub winder_contraction: f64,
    /// The walkline (tread depth is measured along it).
    pub walkline: Walkline,
    /// What the Radius field of a curved stair measures to.
    pub radius_ref: RadiusRef,
    /// The Stringers panel.
    pub stringers: StringerOptions,
    /// A carpet runner.
    pub runner: Runner,
    /// The top landing's nosing and riser.
    pub top_landing: TopLanding,
    /// Handrail extensions and returns.
    pub handrail_options: HandrailOptions,
    /// Plan Display, Arrow and the rail details of the plan.
    pub plan: PlanOptions,
    /// Flare and curved treads.
    pub flare_shape: Flare,
    /// Starter treads.
    pub starter: Starter,
    /// A curved ramp: the radius of its inside edge (`None` is a straight
    /// ramp). Only for [`StairShape::Ramp`].
    pub ramp_curve: Option<f64>,
    /// Library item (id) used for every newel instead of the built-in post;
    /// empty uses the built-in.
    pub newel_item: String,
    /// Library item (id) used for every baluster.
    pub baluster_item: String,
    /// A doorway is cut in a railing the stair meets (Automatic Railing
    /// Openings).
    pub railing_openings: bool,
    /// Sections wrap around a deck or landing corner and share attributes.
    pub allow_wrap: bool,
    /// Landings: the railing of each edge of the outline, in the order of
    /// the outline (missing entries are automatic).
    pub edge_rails: Vec<EdgeRail>,
    /// A downward stair (drawn with Alt or the right mouse button): the plan
    /// arrow starts at the top and points down, labelled DN instead of UP.
    /// The steps themselves are the same as an upward stair's.
    pub down: bool,
    /// Ramps: the Options and Tread Surface rows of the Ramp Specification.
    pub ramp: RampOptions,
    /// Treads in each subsection of a section made by merging flights
    /// (empty for a plain section); the counts add up to the section's
    /// treads.
    pub subsections: Vec<u32>,
    /// Landings: Auto Adjust Height (the top follows the sections that
    /// arrive on it).
    pub landing_auto_height: bool,
    /// Landings: Auto Adjust Thickness (one riser plus the floor finish).
    pub landing_auto_thickness: bool,
}

/// The shortest run of one stair or ramp section, inches (Chief: 6").
pub const SECTION_MIN_RUN: f64 = 6.0;
/// The longest run of one stair or ramp section, inches (Chief: 100'); a
/// longer climb needs a landing between two sections.
pub const SECTION_MAX_RUN: f64 = 1200.0;

/// A dragged run held between the shortest and longest a section may be.
pub fn clamp_section_run(run: f64) -> f64 {
    run.clamp(SECTION_MIN_RUN, SECTION_MAX_RUN)
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
            stringer: StringerStyle::Closed,
            left_side: SideKind::None,
            right_side: SideKind::None,
            railing: RailingParams::default(),
            slab_thickness: 3.5,
            outline: Vec::new(),
            flare: 0.0,
            bullnose: Bullnose::None,
            left_railing: None,
            right_railing: None,
            spiral: false,
            u_gap: 0.0,
            split_landing: false,
            winder_contraction: 0.0,
            walkline: Walkline::default(),
            radius_ref: RadiusRef::default(),
            stringers: StringerOptions::default(),
            runner: Runner::default(),
            top_landing: TopLanding::default(),
            handrail_options: HandrailOptions::default(),
            plan: PlanOptions::default(),
            flare_shape: Flare::default(),
            starter: Starter::default(),
            ramp_curve: None,
            newel_item: String::new(),
            baluster_item: String::new(),
            railing_openings: false,
            allow_wrap: false,
            edge_rails: Vec::new(),
            down: false,
            ramp: RampOptions::default(),
            subsections: Vec::new(),
            landing_auto_height: true,
            landing_auto_thickness: true,
        }
    }
}

impl StairParams {
    /// The rails, newels and balusters of one side: its own settings when it
    /// has them, else the shared [`StairParams::railing`].
    pub fn railing_for(&self, side: RailSide) -> RailingParams {
        match side {
            RailSide::Left => self.left_railing,
            RailSide::Right => self.right_railing,
        }
        .unwrap_or(self.railing)
    }

    /// Distance of the walkline from the inside edge of a curve (and from the
    /// right edge of a straight stair): the walkline's setting when it is on,
    /// else half the width (the tread centre).
    pub fn walk_offset(&self) -> f64 {
        if self.walkline.on {
            self.walkline.distance.clamp(0.0, self.width.max(0.0))
        } else {
            self.width.max(0.0) / 2.0
        }
    }

    /// The radius of a curved stair measured to `which` circle, or `None`
    /// for a stair that is not curved.
    pub fn curve_radius(&self, which: RadiusRef) -> Option<f64> {
        let StairShape::Curved { inner_radius } = self.shape else {
            return None;
        };
        let inner = inner_radius.max(0.0);
        Some(match which {
            RadiusRef::InnerArc => inner,
            RadiusRef::Centerline => inner + self.width / 2.0,
            RadiusRef::Walkline => inner + self.walk_offset(),
            RadiusRef::OuterArc => inner + self.width,
        })
    }

    /// Sets the radius of a curved stair so that the `which` circle has the
    /// radius `r` (the inside radius never goes below zero).
    pub fn set_curve_radius(&mut self, which: RadiusRef, r: f64) {
        if let StairShape::Curved { inner_radius } = &mut self.shape {
            let off = match which {
                RadiusRef::InnerArc => 0.0,
                RadiusRef::Centerline => self.width / 2.0,
                RadiusRef::Walkline => {
                    if self.walkline.on {
                        self.walkline.distance.clamp(0.0, self.width.max(0.0))
                    } else {
                        self.width.max(0.0) / 2.0
                    }
                }
                RadiusRef::OuterArc => self.width,
            };
            *inner_radius = (r - off).max(0.0);
        }
    }

    /// How far the bottom tread reaches past the left and right edges of the
    /// stair for a given tread depth and nosing: the bullnose's half-round on
    /// a rounded end, else the flare.
    pub fn apron_reach(&self) -> (f64, f64) {
        let round = (self.tread_depth + self.nosing) * 0.5;
        // Starter treads are rounded at both ends unless an end says
        // otherwise.
        let starter = if self.starter == Starter::None || self.flare > 0.0 {
            0.0
        } else {
            round
        };
        (
            if self.bullnose.left() {
                round
            } else {
                self.flare.max(starter)
            },
            if self.bullnose.right() {
                round
            } else {
                self.flare.max(starter)
            },
        )
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
    /// Flat landings between flights (L and U stairs, ramp landings).
    pub landings: u32,
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
    /// Height of the bottom of the stair above that floor: 0 for a stair that
    /// starts on the floor, the height of the landing for a section that
    /// starts on one.
    #[serde(default)]
    pub base: f64,
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
            base: 0.0,
        }
    }

    /// Absolute elevation of the bottom of the stair.
    pub fn bottom_elevation(&self) -> f64 {
        self.floor_elevation + self.base
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
        StairShape::UShaped {
            treads_before_landing,
        } if params.split_landing => (3, 0, Some(treads_before_landing)),
        StairShape::LShaped {
            treads_before_landing,
        }
        | StairShape::UShaped {
            treads_before_landing,
        } => (2, 0, Some(treads_before_landing)),
        StairShape::Winder { winders } => (winders.max(1) + 1, winders.max(1), None),
        StairShape::Straight
        | StairShape::Ramp { .. }
        | StairShape::Curved { .. }
        | StairShape::Landing { .. } => return None,
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

    if let StairShape::Landing { depth } = params.shape {
        if params.width < MIN_WIDTH - EPS {
            code_ok = false;
            warnings.push(format!(
                "landing width {:.2}\" is below the 36\" minimum",
                params.width
            ));
        }
        return StairSolution {
            risers: 0,
            riser_height: 0.0,
            treads: 0,
            tread_depth: 0.0,
            total_run: depth.max(0.0),
            landings: 0,
            code_ok,
            warnings,
        };
    }

    if let StairShape::Ramp { slope_1_in } = params.shape {
        let runs = ramp_runs(params.total_rise);
        let run =
            params.total_rise.max(0.0) * slope_1_in.max(0.0) + f64::from(runs - 1) * RAMP_LANDING;
        if slope_1_in < RAMP_MIN_SLOPE {
            code_ok = false;
            warnings.push(format!(
                "ramp slope 1:{slope_1_in:.1} is steeper than the 1:12 maximum"
            ));
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
            landings: runs - 1,
            code_ok,
            warnings,
        };
    }

    let rise = params.total_rise.max(0.0);
    let spiral = params.spiral && matches!(params.shape, StairShape::Curved { .. });
    let (max_riser, min_tread, min_width, min_headroom) = if spiral {
        (
            SPIRAL_MAX_RISER,
            SPIRAL_MIN_TREAD,
            SPIRAL_MIN_WIDTH,
            SPIRAL_MIN_HEADROOM,
        )
    } else {
        (MAX_RISER, MIN_TREAD, MIN_WIDTH, MIN_HEADROOM)
    };
    let target = if params.riser_height_target > 0.0 {
        params.riser_height_target
    } else {
        7.5
    };
    let min_risers = ((rise / max_riser - EPS).ceil().max(1.0)) as u32;
    let max_risers = ((rise / MIN_RISER + EPS).floor().max(1.0)) as u32;
    let risers =
        ((rise / target).round().max(1.0) as u32).clamp(min_risers, max_risers.max(min_risers));
    let riser_height = rise / f64::from(risers);

    if riser_height > max_riser + EPS {
        code_ok = false;
        warnings.push(if spiral {
            format!("riser {riser_height:.3}\" exceeds the 9 1/2\" maximum of a spiral stair")
        } else {
            format!("riser {riser_height:.3}\" exceeds the 7 3/4\" maximum")
        });
    }
    if riser_height < MIN_RISER - EPS {
        warnings.push(format!(
            "riser {riser_height:.3}\" is below the 4\" minimum"
        ));
    }
    if params.tread_depth < min_tread - EPS {
        code_ok = false;
        warnings.push(format!(
            "tread depth {:.2}\" is below the {} minimum",
            params.tread_depth,
            if spiral { "6 3/4\"" } else { "10\"" }
        ));
    }
    if params.width < min_width - EPS {
        code_ok = false;
        warnings.push(format!(
            "width {:.2}\" is below the {}\" minimum",
            params.width, min_width
        ));
    }
    if params.headroom_min < min_headroom - EPS {
        code_ok = false;
        warnings.push(format!(
            "headroom {:.2}\" is below the {}\" minimum",
            params.headroom_min, min_headroom
        ));
    }
    let comfort = 2.0 * riser_height + params.tread_depth;
    if !spiral && !(24.0 - EPS..=25.0 + EPS).contains(&comfort) {
        warnings.push(format!(
            "2R+T = {comfort:.2}\" is outside the 24-25\" comfort range"
        ));
    }

    let treads = risers - 1;
    let straight_run = f64::from(treads) * params.tread_depth;
    if let (StairShape::Curved { inner_radius }, false) = (params.shape, spiral) {
        let walk = inner_radius.max(0.0) + params.walk_offset();
        let inside = params.tread_depth * inner_radius.max(0.0) / walk.max(1e-9);
        if inside < MIN_CURVED_TREAD_INSIDE - EPS {
            code_ok = false;
            warnings.push(format!(
                "tread is {inside:.2}\" deep at the inside edge, under the 6\" minimum"
            ));
        }
    }
    let total_run = match (split(params, risers), params.shape) {
        (Some(s), StairShape::Winder { .. }) => {
            f64::from(s.t1 + s.t2) * params.tread_depth + params.width
        }
        (Some(s), _) => f64::from(s.t1 + s.t2) * params.tread_depth + effective_landing(params),
        (None, StairShape::Straight | StairShape::Curved { .. }) => straight_run,
        (None, _) => {
            warnings
                .push("too few risers for the requested turn; drawn as a straight stair".into());
            straight_run
        }
    };

    let landings = match (split(params, risers), params.shape) {
        (Some(_), StairShape::UShaped { .. }) if params.split_landing => 2,
        (Some(_), StairShape::LShaped { .. } | StairShape::UShaped { .. }) => 1,
        _ => 0,
    };
    StairSolution {
        risers,
        riser_height,
        treads,
        tread_depth: params.tread_depth,
        total_run,
        landings,
        code_ok,
        warnings,
    }
}

/// How many runs a ramp of `total_rise` is built in: at most 30" of rise
/// each, joined by flat landings.
pub fn ramp_runs(total_rise: f64) -> u32 {
    ((total_rise.max(0.0) / RAMP_MAX_RISE - EPS).ceil().max(1.0)) as u32
}

/// The fewest risers that keep the riser height within `max_riser` for a
/// floor-to-floor `rise` (15 for 109 1/8" with the 7 3/4" maximum).
pub fn min_risers(rise: f64, max_riser: f64) -> u32 {
    ((rise.max(0.0) / max_riser.max(MIN_RISER) - EPS)
        .ceil()
        .max(1.0)) as u32
}

/// The centre of a curved stair in plan, or `None` for other shapes.
pub fn curve_center(stair: &Stair) -> Option<Point> {
    let layout = Layout::build(stair);
    layout
        .curve
        .or_else(|| layout.ramp_arc.as_ref().map(|r| r.curve))
        .map(|c| layout.frame.uv(c.center))
}

/// Angle swept by a curved stair from the first to the last riser, radians.
pub fn curve_sweep(stair: &Stair) -> Option<f64> {
    let layout = Layout::build(stair);
    layout
        .curve
        .or_else(|| layout.ramp_arc.as_ref().map(|r| r.curve))
        .map(|c| c.sweep())
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
    if let Some(c) = layout
        .curve
        .as_ref()
        .or_else(|| layout.ramp_arc.as_ref().map(|r| &r.curve))
    {
        let p = layout.frame.uv(c.at(c.sweep(), c.walk()));
        return (p, stair.bottom_elevation() + layout.total_rise);
    }
    let last = layout
        .flights
        .last()
        .expect("a layout always has at least one flight");
    let (u, v) = last.at(last.len, last.width / 2.0);
    (
        layout.frame.point(u, v),
        stair.bottom_elevation() + layout.total_rise,
    )
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_r15;
#[cfg(test)]
mod tests_sections;

#[cfg(test)]
mod tests_engine;
