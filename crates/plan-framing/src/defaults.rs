//! Framing defaults, the equivalent of Chief's framing defaults dialogs.

use crate::lumber::{Lumber, TWO_BY_EIGHT, TWO_BY_FOUR, TWO_BY_SIX, TWO_BY_TEN, TWO_BY_TWELVE};
use plan_core::Wall;
use serde::{Deserialize, Serialize};

/// Wall thickness at which 2x6 studs replace 2x4 studs.
const TWO_BY_SIX_WALL_THICKNESS: f64 = 6.0;

/// One row of the header table: an opening up to `up_to` inches wide gets a
/// header of `lumber` (the last row also covers wider openings).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeaderRow {
    pub up_to: f64,
    pub lumber: Lumber,
}

/// Chief-like header sizes: 2x6 to 4', 2x8 to 5', 2x10 to 6', 2x12 beyond.
pub fn default_header_table() -> Vec<HeaderRow> {
    vec![
        HeaderRow {
            up_to: 48.0,
            lumber: TWO_BY_SIX,
        },
        HeaderRow {
            up_to: 60.0,
            lumber: TWO_BY_EIGHT,
        },
        HeaderRow {
            up_to: 72.0,
            lumber: TWO_BY_TEN,
        },
        HeaderRow {
            up_to: 1.0e9,
            lumber: TWO_BY_TWELVE,
        },
    ]
}

/// Parameters for [`crate::frame_wall`] and [`crate::frame_floor`].
///
/// `Default` is the plain Chief assembly (plates, studs, kings, trimmers,
/// headers, sills, cripples); [`FramingDefaults::house`] adds the corner and
/// tee backing and the wall blocking Build Framing uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FramingDefaults {
    /// Stud spacing on centre, inches.
    pub stud_spacing: f64,
    /// Stud lumber. The default 2x4 is upgraded to 2x6 for walls at least 6"
    /// thick; see [`FramingDefaults::stud_size_for`].
    pub stud_size: Lumber,
    pub top_plates: u32,
    pub bottom_plates: u32,
    /// Header depth in inches. `0.0` selects the size from `header_table`.
    pub header_depth: f64,
    /// Header size by opening width (see [`default_header_table`]).
    pub header_table: Vec<HeaderRow>,
    /// Number of 1 1/2" plies in a header.
    pub header_plies: u32,
    /// King studs on each side of an opening.
    pub king_studs: u32,
    /// Trimmer studs on each side of an opening.
    pub trimmers: u32,
    /// Spacing of cripples above headers and below sills, inches.
    pub cripple_spacing: f64,
    pub joist_spacing: f64,
    pub joist_size: Lumber,
    /// Add rim joists at the joist ends (joists are shortened to fit).
    pub rim_joist: bool,
    /// Add mid-span blocking rows, at most 8' apart.
    pub blocking: bool,
    /// Extra studs beside the end stud where a wall ends at another wall (a
    /// corner), each 1 1/2" thick, `0` for none.
    pub corner_studs: u32,
    /// Backing studs on each side of a partition that butts into the wall,
    /// `0` for none.
    pub tee_studs: u32,
    /// Horizontal blocking between the studs of a wall.
    pub wall_blocking: bool,
    /// Height of the first blocking row and the spacing of the rows, inches.
    pub wall_blocking_spacing: f64,
    /// Plies of the header and trimmer joists around a floor hole.
    pub hole_plies: u32,
}

impl Default for FramingDefaults {
    fn default() -> Self {
        Self {
            stud_spacing: 16.0,
            stud_size: TWO_BY_FOUR,
            top_plates: 2,
            bottom_plates: 1,
            header_depth: 0.0,
            header_table: default_header_table(),
            header_plies: 2,
            king_studs: 1,
            trimmers: 1,
            cripple_spacing: 16.0,
            joist_spacing: 16.0,
            joist_size: TWO_BY_TEN,
            rim_joist: true,
            blocking: false,
            corner_studs: 0,
            tee_studs: 0,
            wall_blocking: false,
            wall_blocking_spacing: 48.0,
            hole_plies: 2,
        }
    }
}

impl FramingDefaults {
    /// What Build Framing uses: the default assembly with a corner stud, tee
    /// backing and a blocking row every 48".
    pub fn house() -> Self {
        Self {
            corner_studs: 1,
            tee_studs: 1,
            wall_blocking: true,
            ..Self::default()
        }
    }

    /// Stud lumber for `wall`: the configured size, except that a 2x4 is
    /// upgraded to 2x6 when the wall is 6" or thicker.
    pub fn stud_size_for(&self, wall: &Wall) -> Lumber {
        if self.stud_size.depth < TWO_BY_SIX.depth && wall.thickness >= TWO_BY_SIX_WALL_THICKNESS {
            TWO_BY_SIX
        } else {
            self.stud_size
        }
    }

    /// Header lumber depth for an opening of the given width: the fixed
    /// `header_depth` when set, else the first `header_table` row that is wide
    /// enough (the last row for anything wider).
    pub fn header_depth_for(&self, opening_width: f64) -> f64 {
        self.header_lumber_for(opening_width).depth
    }

    /// Header lumber for an opening of the given width (see
    /// [`header_depth_for`](Self::header_depth_for)).
    pub fn header_lumber_for(&self, opening_width: f64) -> Lumber {
        if self.header_depth > 0.0 {
            return Lumber::two_by(self.header_depth);
        }
        self.header_table
            .iter()
            .find(|r| opening_width <= r.up_to + 1e-9)
            .or_else(|| self.header_table.last())
            .map_or(TWO_BY_SIX, |r| r.lumber)
    }
}
