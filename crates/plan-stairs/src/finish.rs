//! Round 17 stairs finish: Custom Stringers and Trim Against Wall (Staircase
//! Specification, Stringers panel), Railing Transitions and Brackets (Railing
//! panel). The plan-only data lives in [`Finish`] (`StairParams::finish`);
//! the functions answer what the boards, rail ends and brackets are, so the
//! 3D builders and the dialog read one source.

use serde::{Deserialize, Serialize};

use crate::{rise_angle, solve, SideKind, StairParams};

/// One row of the Custom Stringers table (Left, Middle or Right).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomStringer {
    /// How many boards in this row (0 turns the row off).
    pub count: u8,
    /// Board thickness.
    pub thickness: f64,
    /// Distance of the board from the edge of the stair it belongs to (the
    /// centre row measures from the middle of the stair).
    pub offset: f64,
    /// Height of the board below the line through the tread nosings,
    /// measured straight down.
    pub height_below: f64,
    /// The board carries on past the top riser to the upper floor.
    pub ext: bool,
    /// Height of the board above that nosing line.
    pub height_above: f64,
}

impl Default for CustomStringer {
    fn default() -> Self {
        Self {
            count: 0,
            thickness: 1.5,
            offset: 0.0,
            height_below: 8.0,
            ext: false,
            height_above: 0.0,
        }
    }
}

impl CustomStringer {
    /// Perpendicular depth of the board at the given stair slope in degrees:
    /// the vertical extent (below plus above the nosing line) times the
    /// cosine of the slope.
    pub fn depth(&self, slope_deg: f64) -> f64 {
        (self.height_below + self.height_above).max(0.0) * slope_deg.to_radians().cos()
    }
}

/// Trim Against Wall: a thin board along the side of the stair that meets a
/// wall.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrimAgainstWall {
    pub on: bool,
    /// Height of the trim above the tread.
    pub height_above: f64,
    /// Height of the trim below the tread.
    pub height_below: f64,
    pub thickness: f64,
    /// Clip the top end of the trim square to the wall on the left side.
    pub clip_top_left: bool,
    /// The same on the right side.
    pub clip_top_right: bool,
}

impl Default for TrimAgainstWall {
    fn default() -> Self {
        Self {
            on: false,
            height_above: 3.0,
            height_below: 3.5,
            thickness: 0.75,
            clip_top_left: false,
            clip_top_right: false,
        }
    }
}

/// How a stair rail meets the level rail of a landing or floor edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RailTransition {
    /// The sloped rail simply stops at a post and the level rail starts.
    #[default]
    None,
    /// The sloped rail bends up in a quarter-round before it levels off.
    Gooseneck,
    /// The sloped rail eases into the level rail in one S-curve.
    Smooth,
}

impl RailTransition {
    pub const ALL: [RailTransition; 3] = [
        RailTransition::None,
        RailTransition::Gooseneck,
        RailTransition::Smooth,
    ];

    pub fn name(self) -> &'static str {
        match self {
            RailTransition::None => "None",
            RailTransition::Gooseneck => "Gooseneck",
            RailTransition::Smooth => "Smooth",
        }
    }
}

/// The R17 stringer and railing additions of a stair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Finish {
    /// Custom Stringers: Left, Middle and Right rows.
    pub custom: [CustomStringer; 3],
    pub trim: TrimAgainstWall,
    pub transition: RailTransition,
    /// Smooth Transitions between the rails of a stair and a landing.
    pub smooth_landing_rails: bool,
    /// A bracket under every riser on each exposed stringer side.
    pub brackets: bool,
    /// Library item (id) of the bracket; empty uses a plain block.
    pub bracket_item: String,
}

/// One custom stringer board in the stair's own frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StringerBoard {
    /// Distance from the left edge of the stair to the middle of the board.
    pub lateral: f64,
    pub thickness: f64,
    /// Perpendicular depth.
    pub depth: f64,
    pub height_below: f64,
    pub height_above: f64,
    pub ext: bool,
}

/// The boards the Custom Stringers table makes for a stair: the Left row
/// stands in from the left edge by its offset, the Right row from the right
/// edge, the Middle row about the middle; a row with `count` boards spaces
/// them evenly between its first board and the middle line (Left, Right) or
/// across the width (Middle).
pub fn custom_stringer_boards(p: &StairParams) -> Vec<StringerBoard> {
    let slope = rise_angle(solve(p).riser_height, p.tread_depth);
    let mut out = Vec::new();
    for (row, c) in p.finish.custom.iter().enumerate() {
        for k in 0..c.count {
            let k = f64::from(k);
            let n = f64::from(c.count);
            let lateral = match row {
                0 => c.offset + c.thickness / 2.0 + k * (c.thickness + 2.0),
                2 => p.width - c.offset - c.thickness / 2.0 - k * (c.thickness + 2.0),
                _ => p.width * (k + 1.0) / (n + 1.0) + c.offset,
            };
            out.push(StringerBoard {
                lateral,
                thickness: c.thickness,
                depth: c.depth(slope),
                height_below: c.height_below,
                height_above: c.height_above,
                ext: c.ext,
            });
        }
    }
    out
}

/// Which sides get trim against a wall: a side with no railing, half wall
/// or other guard is the one that meets a wall. Returns (left, right).
pub fn trim_sides(p: &StairParams) -> (bool, bool) {
    if !p.finish.trim.on {
        return (false, false);
    }
    (
        p.left_side == SideKind::None,
        p.right_side == SideKind::None,
    )
}

/// Height of the sloped rail above the level rail at the junction, inches:
/// the rise of the sloped rail over one tread (one riser).
pub fn junction_step(p: &StairParams) -> f64 {
    solve(p).riser_height
}

/// The rail profile across the junction as (distance along, height) pairs
/// with 0 at the last riser line and height 0 the level rail; the sloped rail
/// arrives `junction_step` high at distance `-span` ... 0. `None` is one
/// abrupt step; `Gooseneck` rises in a quarter circle over `span`; `Smooth`
/// follows a smoothstep over `span`, which is one tread deep.
pub fn transition_profile(p: &StairParams) -> Vec<(f64, f64)> {
    let step = junction_step(p);
    let span = p.tread_depth.max(1.0);
    let kind = if p.finish.smooth_landing_rails && p.finish.transition == RailTransition::None {
        RailTransition::Smooth
    } else {
        p.finish.transition
    };
    const N: usize = 8;
    match kind {
        RailTransition::None => vec![(-span, step), (0.0, step), (0.0, 0.0)],
        RailTransition::Gooseneck => (0..=N)
            .map(|i| {
                let t = i as f64 / N as f64;
                (-span + span * t, gooseneck(step, t))
            })
            .collect(),
        RailTransition::Smooth => (0..=N)
            .map(|i| {
                let t = i as f64 / N as f64;
                (-span + span * t, step * (1.0 - t * t * (3.0 - 2.0 * t)))
            })
            .collect(),
    }
}

fn gooseneck(step: f64, t: f64) -> f64 {
    // Starts at the sloped rail's height, stays near it while the bend
    // forms, then falls away to the level rail along a quarter circle.
    step * (1.0 - (1.0 - (1.0 - t) * (1.0 - t)).max(0.0).sqrt())
}

/// Bracket stations of one side: (distance along, nosing height).
pub type Stations = Vec<(f64, f64)>;

/// Bracket stations: (distance along the stair, height of the tread
/// nosing), one under each riser, for each side that has an exposed
/// stringer, as (left, right) lists.
pub fn bracket_stations(p: &StairParams) -> (Stations, Stations) {
    if !p.finish.brackets {
        return (Vec::new(), Vec::new());
    }
    let s = solve(p);
    let list: Vec<(f64, f64)> = (0..s.risers)
        .map(|i| {
            (
                f64::from(i) * s.tread_depth,
                f64::from(i + 1) * s.riser_height,
            )
        })
        .collect();
    // A side that meets a wall (no guard) has no exposed stringer.
    let exposed = |side: SideKind| side != SideKind::None;
    (
        if exposed(p.left_side) {
            list.clone()
        } else {
            Vec::new()
        },
        if exposed(p.right_side) {
            list
        } else {
            Vec::new()
        },
    )
}
