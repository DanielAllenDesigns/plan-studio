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
//! * [`layer_sets`]: named layer sets and saved plan views.
//! * [`text_styles`]: named text styles that layers refer to.
//! * [`history`]: snapshot undo/redo.
//! * [`export`]: file exporters (ASCII DXF today).
//! * [`walls`]: wall flags, curves, roof directives, reference lines and wall
//!   editing (split/join/connections).
//! * [`openings`]: opening styles, labels and swing/hinge editing.
//! * [`floors`]: build/insert/delete/exchange floors and foundations.
//! * [`camera`]: camera objects placed in the plan.
//! * [`symbols`]: placed library symbols and opaque cabinet/stair slots.
//! * [`groups`]: object groups and the clipboard.
//! * [`extras`]: typed room/opening/wall/section extras and the roof, electrical,
//!   framing and terrain slots.

pub mod cad;
pub mod camera;
pub mod defaults;
pub mod dimension;
pub mod export;
pub mod extras;
pub mod floors;
pub mod geometry;
pub mod groups;
pub mod history;
pub mod joins;
pub mod layer_sets;
pub mod layers;
pub mod model;
pub mod openings;
pub mod rooms;
pub mod symbols;
pub mod text_styles;
pub mod units;
pub mod walls;

pub use cad::{CadItem, CadObject};
pub use camera::{CameraKind, CameraObject};
pub use defaults::{
    DimensionDefaultSet, EditingDefaults, PlanDefaults, WallConnectDefaults, WallLayer, WallTypeDef,
};
pub use dimension::{auto_exterior_dimensions, DimFormat, Dimension, DimensionKind};
pub use export::dxf::write_dxf;
pub use extras::{
    AreaKind, MoldingKind, MoldingRef, OpeningExtras, RoomFill, RoomLabelOptions, SectionLine,
    WallExtras,
};
pub use floors::{FloorKind, FoundationKind};
pub use geometry::Point;
pub use groups::{Clipboard, ObjectGroup, ObjectRef};
pub use history::History;
pub use joins::{
    main_layer_lines, wall_end_joins, wall_faces, wall_layer_bands, wall_layer_outlines,
    wall_outlines, ConnectionKind, LayerBand, WallLayerOutline, WallOutline,
};
pub use layer_sets::{LayerSetDef, LayerSets, LayerState, SavedPlanView};
pub use layers::{Layer, LayerSet, LineStyle};
pub use model::*;
pub use openings::{Casing, OpeningStyle};
pub use rooms::{detect_rooms, detect_rooms_inner, Room};
pub use symbols::PlacedSymbol;
pub use text_styles::{TextStyle, TextStyles};
pub use walls::{
    PonyWall, ResizeAbout, Side, WallConnection, WallCurve, WallFlags, WallRoofDirective,
};
