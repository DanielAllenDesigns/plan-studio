//! Where a door or window may stand on its wall (DW-4, DW-10, DW-73..DW-75,
//! DW-87). The rules live in `plan_core::openings::placement`, shared with
//! the Select tool's drag, the Specification dialog and
//! `Project::slide_opening`; this module re-exports them for the opening tools.

pub use plan_core::openings::placement::*;
