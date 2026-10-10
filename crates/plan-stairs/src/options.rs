//! Round 15 options of a stair: the panels of Chief's Staircase and Ramp
//! Specification that sit beside the dimensions (Style, Stringers, Plan
//! Display, Arrow, Railing, Newels/Balusters, Flare).
//!
//! Every struct is `#[serde(default)]`, so plans saved before round 15 load
//! with the old behaviour (the defaults here draw what the engine drew
//! before).

use serde::{Deserialize, Serialize};

/// Where a staircase shows on the floor above (Plan Display panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DisplayRule {
    /// Only through an Open Below room or a stairwell hole.
    #[default]
    Automatic,
    /// Always.
    Always,
    /// Never.
    Never,
}

impl DisplayRule {
    pub const ALL: [DisplayRule; 3] = [
        DisplayRule::Automatic,
        DisplayRule::Always,
        DisplayRule::Never,
    ];

    pub fn name(self) -> &'static str {
        match self {
            DisplayRule::Automatic => "Automatic",
            DisplayRule::Always => "Always",
            DisplayRule::Never => "Never",
        }
    }
}

/// How the part of a staircase beyond (or before) the break line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ViewMode {
    /// Treads and outline.
    #[default]
    Normal,
    /// The outline only.
    Outline,
    /// Not drawn.
    Nothing,
}

impl ViewMode {
    pub const ALL: [ViewMode; 3] = [ViewMode::Normal, ViewMode::Outline, ViewMode::Nothing];

    pub fn name(self) -> &'static str {
        match self {
            ViewMode::Normal => "Normal",
            ViewMode::Outline => "Outline",
            ViewMode::Nothing => "Nothing",
        }
    }
}

/// The head of the direction arrow (Arrow panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArrowStyle {
    /// A closed triangle (the engine's old arrow).
    #[default]
    Closed,
    /// Two barbs, no base.
    Open,
    /// No head: the centreline alone.
    Line,
    /// No arrow at all (the UP label stays).
    None,
}

impl ArrowStyle {
    pub const ALL: [ArrowStyle; 4] = [
        ArrowStyle::Closed,
        ArrowStyle::Open,
        ArrowStyle::Line,
        ArrowStyle::None,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ArrowStyle::Closed => "Closed Head",
            ArrowStyle::Open => "Open Head",
            ArrowStyle::Line => "Line",
            ArrowStyle::None => "None",
        }
    }
}

/// The shape of the break line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BreakStyle {
    /// The zigzag Chief draws.
    #[default]
    Zigzag,
    /// A plain line across.
    Straight,
    /// A slash: the line steps over by the symbol size.
    Slash,
}

impl BreakStyle {
    pub const ALL: [BreakStyle; 3] = [BreakStyle::Zigzag, BreakStyle::Straight, BreakStyle::Slash];

    pub fn name(self) -> &'static str {
        match self {
            BreakStyle::Zigzag => "Zigzag",
            BreakStyle::Straight => "Straight",
            BreakStyle::Slash => "Slash",
        }
    }
}

/// Plan Display, Arrow and Newels/Balusters "Plan Display" settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanOptions {
    /// Display on the floor above.
    pub floor_above: DisplayRule,
    /// The part of the staircase before the break line, seen from the floor
    /// above ("Floor Above Display").
    pub above_view: ViewMode,
    /// The part beyond the break line on the stair's own floor ("Current
    /// Floor Display").
    pub beyond_view: ViewMode,
    /// Style of the break line.
    pub break_style: BreakStyle,
    /// Angle of the break line from square across the stair, degrees.
    pub break_angle: f64,
    /// Size of the break symbol (the zigzag amplitude), inches.
    pub break_size: f64,
    /// Gap left in the treads after the break line, inches.
    pub break_gap: f64,
    /// Style of the direction arrow.
    pub arrow: ArrowStyle,
    /// Length of the arrowhead, inches.
    pub arrow_size: f64,
    /// Number the treads in plan (1 at the bottom).
    pub number_treads: bool,
    /// Draw the newels of a railing in plan.
    pub draw_newels: bool,
    /// Draw the balusters of a railing in plan (small squares).
    pub draw_balusters: bool,
    /// Draw the rails in plan.
    pub draw_rails: bool,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            floor_above: DisplayRule::Automatic,
            above_view: ViewMode::Normal,
            beyond_view: ViewMode::Normal,
            break_style: BreakStyle::Zigzag,
            break_angle: 0.0,
            break_size: 3.0,
            break_gap: 0.0,
            arrow: ArrowStyle::Closed,
            arrow_size: 6.0,
            number_treads: false,
            draw_newels: true,
            draw_balusters: false,
            draw_rails: true,
        }
    }
}

/// Which circle a curved stair's Radius field measures to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RadiusRef {
    /// The outside edge.
    OuterArc,
    /// The middle of the stair.
    Centerline,
    /// The walkline.
    #[default]
    Walkline,
    /// The inside edge.
    InnerArc,
}

impl RadiusRef {
    pub const ALL: [RadiusRef; 4] = [
        RadiusRef::OuterArc,
        RadiusRef::Centerline,
        RadiusRef::Walkline,
        RadiusRef::InnerArc,
    ];

    pub fn name(self) -> &'static str {
        match self {
            RadiusRef::OuterArc => "Outer Arc",
            RadiusRef::Centerline => "Centerline",
            RadiusRef::Walkline => "Walkline",
            RadiusRef::InnerArc => "Inner Arc",
        }
    }
}

/// The walkline (Style panel).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Walkline {
    /// Measure tread depth along the walkline; off measures at the centre.
    pub on: bool,
    /// Distance from the edge (the right edge of a straight stair, the
    /// inside edge of a curve), inches.
    pub distance: f64,
    /// Draw the walkline in plan.
    pub show: bool,
}

impl Default for Walkline {
    fn default() -> Self {
        Self {
            on: false,
            distance: 12.0,
            show: false,
        }
    }
}

/// The Stringers panel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StringerOptions {
    /// Stringers down the middle of the stair as well as the sides (a steel
    /// stringer with concrete treads uses one in the middle and none beside).
    pub centre: u8,
    /// Leave out the two side stringers (only the centre ones remain).
    pub no_sides: bool,
    /// Open underneath. Off closes the underside with a soffit and a skirt
    /// along both sides.
    pub open_underneath: bool,
    /// How far the skirt stands in from the side of the stair, inches.
    pub side_inset: f64,
    /// A larger stringer at the base: the bottom of the stringer reaches the
    /// floor with a deeper board.
    pub large_base: bool,
    /// The stringer carries on up to the top floor (default on).
    pub extend_top: bool,
    /// Stringer board thickness.
    pub thickness: f64,
}

impl Default for StringerOptions {
    fn default() -> Self {
        Self {
            centre: 0,
            no_sides: false,
            open_underneath: true,
            side_inset: 0.0,
            large_base: false,
            extend_top: true,
            thickness: 1.5,
        }
    }
}

/// A carpet runner down the middle of the treads (Style panel).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Runner {
    /// Width of the runner; 0 is none.
    pub width: f64,
    /// Tucked: the runner turns down under the nosing to the riser.
    pub tucked: bool,
}

/// The top of the staircase where it meets the upper floor (Style panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TopLanding {
    /// A nosing on the edge of the top landing.
    pub nosing: bool,
    /// The top riser is surfaced like the other risers.
    pub riser_surface: bool,
}

impl Default for TopLanding {
    fn default() -> Self {
        Self {
            nosing: false,
            riser_surface: true,
        }
    }
}

/// Handrail options of the Railing panel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HandrailOptions {
    /// The handrail runs this far past the top riser.
    pub extend_top: f64,
    /// The handrail runs this far past the bottom riser.
    pub extend_bottom: f64,
    /// The top end turns back into the wall.
    pub return_top: bool,
    /// The bottom end turns back into the wall.
    pub return_bottom: bool,
}

/// The cross-section of a newel or baluster the built-in library offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PostProfile {
    /// A plain square post.
    #[default]
    Square,
    /// A square with its corners cut off.
    Chamfered,
    /// A round post (an eight-sided prism).
    Round,
    /// A turned post: a round shaft with a swelling in the middle.
    Turned,
}

impl PostProfile {
    pub const ALL: [PostProfile; 4] = [
        PostProfile::Square,
        PostProfile::Chamfered,
        PostProfile::Round,
        PostProfile::Turned,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PostProfile::Square => "Square",
            PostProfile::Chamfered => "Chamfered",
            PostProfile::Round => "Round",
            PostProfile::Turned => "Turned",
        }
    }
}

/// Flare and curved treads (the Flare/Curve Stairs edit mode).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Flare {
    /// How far each corner stands out from the stair: bottom left, bottom
    /// right, top left, top right (left and right as seen going up).
    pub corners: [f64; 4],
    /// How much the flare's start is rounded off, 0 to 1 (0 is a straight
    /// taper from the start of the flare).
    pub soften: f64,
    /// Where along the stair the flare starts, as a fraction of its run
    /// measured from the end it flares (0.0 is the whole length).
    pub start: f64,
    /// Bulge of the front edge of the bottom treads: the treads curve
    /// outward by this much at the middle.
    pub curve_bottom: f64,
    /// Bulge of every tread of the section.
    pub curve_all: f64,
}

impl Flare {
    /// True when nothing flares and no tread is curved.
    pub fn is_none(&self) -> bool {
        self.corners.iter().all(|c| c.abs() < 1e-9)
            && self.curve_bottom.abs() < 1e-9
            && self.curve_all.abs() < 1e-9
    }
}

/// Starter treads: the first (and second) tread rounded and reaching past the
/// open sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Starter {
    /// Square treads.
    #[default]
    None,
    /// The bottom tread alone.
    One,
    /// The first two treads, the second concentric with the first.
    Two,
}

impl Starter {
    pub const ALL: [Starter; 3] = [Starter::None, Starter::One, Starter::Two];

    pub fn name(self) -> &'static str {
        match self {
            Starter::None => "None",
            Starter::One => "One Starter Tread",
            Starter::Two => "Two Starter Treads",
        }
    }

    /// How many treads are starters.
    pub fn count(self) -> u32 {
        match self {
            Starter::None => 0,
            Starter::One => 1,
            Starter::Two => 2,
        }
    }
}

/// The railing of one edge of a landing (the Selected Edge panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EdgeRail {
    /// A railing on the open sides, none where a stair or landing meets it.
    #[default]
    Automatic,
    /// No railing on this edge.
    No,
    /// A railing on this edge always.
    Has,
}

impl EdgeRail {
    pub const ALL: [EdgeRail; 3] = [EdgeRail::Automatic, EdgeRail::No, EdgeRail::Has];

    pub fn name(self) -> &'static str {
        match self {
            EdgeRail::Automatic => "Automatic",
            EdgeRail::No => "No Railing",
            EdgeRail::Has => "Has Railing",
        }
    }
}
