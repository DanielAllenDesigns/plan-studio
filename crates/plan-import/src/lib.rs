//! plan-import: bring outside drawings into Plan Studio.
//!
//! This crate covers the equivalent of Chief's *File ▸ Import ▸ Import Drawing
//! (DXF/DWG)* and *CAD ▸ CAD to Walls*:
//!
//! * [`dxf`]: a tolerant DXF reader, ASCII and binary, R12 through 2018
//!   ([`parse_dxf`], [`parse_dxf_bytes`]) producing a [`DxfDrawing`] with
//!   layers, tables, entities and block definitions.
//! * [`convert`]: unit handling ([`to_inches_factor`]) and conversion of a
//!   drawing into plan-core CAD objects, dimensions and CAD blocks
//!   ([`convert()`], [`to_cad_objects`]), then [`add_objects`] to a project.
//! * [`walls`]: [`cad_to_walls`] turns pairs of parallel lines into wall
//!   proposals, and [`apply_walls`] adds them to a project.
//!
//! * [`obj`], [`gltf`], [`stl`], [`tds`] and [`dae`]: readers for 3D library
//!   symbols (Wavefront OBJ, glTF 2.0 `.gltf` / `.glb`, STL binary and ASCII,
//!   Autodesk 3DS, COLLADA) that produce a [`ImportedModel`] in inches with Y
//!   up. [`formats`] is the one entry point ([`parse_3d`]) with the unit and
//!   up-axis guesses ([`suggest`]) and the SketchUp message; [`shape`] turns
//!   and resizes a model for the Import 3D Symbol dialog; [`xml`] is the
//!   small XML reader COLLADA needs.
//!
//! All lengths are inches once converted; the reader itself keeps the raw
//! drawing units.

pub mod convert;
pub mod dae;
pub mod dxf;
pub mod formats;
pub mod gltf;
pub mod model;
pub mod obj;
pub mod shape;
pub mod stl;
pub mod tds;
pub mod walls;
pub mod xml;

pub use convert::{
    add_objects, apply_cad, apply_converted, convert, default_units, drawing_bounds, hatch_style,
    layer_counts, linear_dimension, linetype_style, make_blocks, to_cad_objects,
    to_cad_objects_with, to_inches_factor, unused_blocks, ApplyReport, BlockConflict, BlockMode,
    Converted, DimensionMode, HatchSpec, ImportOptions, ImportedBlock, ImportedDimension,
    ImportedObject, LayerMapping, LayerTarget, PlanLayerSpec, Summary,
};
pub use dxf::{
    parse_dxf, parse_dxf_bytes, DxfBlock, DxfColor, DxfDrawing, DxfEntity, DxfKind, DxfLayer,
    DxfUnits,
};
pub use formats::{is_3d_extension, parse_3d, suggest, Suggestion, SKP_MESSAGE};
pub use model::{ImportedModel, ImportedPart, ModelError, ModelOptions, UpAxis};
pub use shape::Facing;
pub use walls::{apply_walls, cad_to_walls, CadToWallsOptions, CadToWallsResult, WallProposal};

/// Why a drawing could not be imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The file does not look like a DXF file (no `SECTION` found).
    NotDxf,
    /// The text is a binary DXF; read it with [`parse_dxf_bytes`].
    BinaryDxf,
    /// The file is an AutoCAD DWG (the release code, e.g. `AC1027`); Plan
    /// Studio reads DXF only.
    Dwg(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::NotDxf => write!(f, "not a DXF file"),
            ImportError::BinaryDxf => write!(f, "this is a binary DXF; read it from its bytes"),
            ImportError::Dwg(v) => write!(f, "{}", dxf::dwg_guidance(v)),
        }
    }
}

impl std::error::Error for ImportError {}
