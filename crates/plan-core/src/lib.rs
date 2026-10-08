//! plan-core: the "plan is the model" data layer for Plan Studio.
//!
//! Everything a view (2D plan, 3D, elevation) shows is derived from the
//! structures in this crate. No GUI code lives here, so it can be unit-tested
//! headlessly and reused by exporters (DXF, PDF, glTF) later.
//!
//! Units: all lengths are stored in **inches** as `f64`. Use [`units`] to
//! format or parse feet-and-inches strings.
//!
//! Modules:
//! * [`geometry`], [`units`], [`model`], [`rooms`]: points, formatting, the
//!   project/floor/wall/opening model and automatic room detection.
//! * [`joins`]: wall outline polygons with mitered corners and clean T-junctions.
//! * [`dimension`]: manual, automatic exterior and temporary dimensions.
//! * [`cad`]: plain 2D annotation primitives.
//! * [`layers`]: Chief-style layers with display/lock state and colours.
//! * [`defaults`]: the plan defaults a new project starts from (wall types,
//!   door/window/cabinet/dimension defaults, room types, grid).
//! * [`history`]: snapshot undo/redo.
//! * [`export`]: file exporters (ASCII DXF today).

pub mod cad;
pub mod defaults;
pub mod dimension;
pub mod export;
pub mod geometry;
pub mod history;
pub mod joins;
pub mod layers;
pub mod model;
pub mod rooms;
pub mod units;

pub use cad::{CadItem, CadObject};
pub use defaults::{PlanDefaults, WallLayer, WallTypeDef};
pub use dimension::{auto_exterior_dimensions, DimFormat, Dimension, DimensionKind};
pub use export::dxf::write_dxf;
pub use geometry::Point;
pub use history::History;
pub use joins::{wall_faces, wall_outlines, WallOutline};
pub use layers::{Layer, LayerSet};
pub use model::*;
pub use rooms::{detect_rooms, Room};
