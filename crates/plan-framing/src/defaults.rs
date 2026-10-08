//! Framing defaults, the equivalent of Chief's framing defaults dialogs.

use crate::lumber::{Lumber, TWO_BY_FOUR, TWO_BY_SIX, TWO_BY_TEN};
use plan_core::Wall;
use serde::{Deserialize, Serialize};

/// Wall thickness at which 2x6 studs replace 2x4 studs.
const TWO_BY_SIX_WALL_THICKNESS: f64 = 6.0;

/// Parameters for [`crate::frame_wall`] and [`crate::frame_floor`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FramingDefaults {
    /// Stud spacing on centre, inches.
    pub stud_spacing: f64,
    /// Stud lumber. The default 2x4 is upgraded to 2x6 for walls at least 6"
    /// thick; see [`FramingDefaults::stud_size_for`].
    pub stud_size: Lumber,
    pub top_plates: u32,
    pub bottom_plates: u32,
    /// Header depth in inches. `0.0` selects automatically from the opening
    /// width: 11 1/4" over 6', 7 1/4" over 4', otherwise 5 1/2".
    pub header_depth: f64,
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
}

impl Default for FramingDefaults {
    fn default() -> Self {
        Self {
            stud_spacing: 16.0,
            stud_size: TWO_BY_FOUR,
            top_plates: 2,
            bottom_plates: 1,
            header_depth: 0.0,
            header_plies: 2,
            king_studs: 1,
            trimmers: 1,
            cripple_spacing: 16.0,
            joist_spacing: 16.0,
            joist_size: TWO_BY_TEN,
            rim_joist: true,
            blocking: false,
        }
    }
}

impl FramingDefaults {
    /// Stud lumber for `wall`: the configured size, except that a 2x4 is
    /// upgraded to 2x6 when the wall is 6" or thicker.
    pub fn stud_size_for(&self, wall: &Wall) -> Lumber {
        if self.stud_size.depth < TWO_BY_SIX.depth && wall.thickness >= TWO_BY_SIX_WALL_THICKNESS {
            TWO_BY_SIX
        } else {
            self.stud_size
        }
    }

    /// Header lumber depth for an opening of the given width.
    pub fn header_depth_for(&self, opening_width: f64) -> f64 {
        if self.header_depth > 0.0 {
            self.header_depth
        } else if opening_width > 72.0 {
            11.25
        } else if opening_width > 48.0 {
            7.25
        } else {
            5.5
        }
    }
}
