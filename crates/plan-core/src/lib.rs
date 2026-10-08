//! plan-core: the "plan is the model" data layer for Plan Studio.
//!
//! Everything a view (2D plan, 3D, elevation) shows is derived from the
//! structures in this crate. No GUI code lives here, so it can be unit-tested
//! headlessly and reused by exporters (DXF, PDF, glTF) later.
//!
//! Units: all lengths are stored in **inches** as `f64`. Use [`units`] to
//! format or parse feet-and-inches strings.

pub mod geometry;
pub mod model;
pub mod rooms;
pub mod units;

pub use geometry::Point;
pub use model::*;
pub use rooms::{detect_rooms, Room};
