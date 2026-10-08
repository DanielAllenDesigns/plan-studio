//! plan-import: bring outside drawings into Plan Studio.
//!
//! This crate covers the equivalent of Chief's *File ▸ Import ▸ Import Drawing
//! (DXF/DWG)* and *CAD ▸ CAD to Walls*:
//!
//! * [`dxf`]: a tolerant ASCII DXF reader ([`parse_dxf`]) producing a
//!   [`DxfDrawing`] with layers, entities and block definitions.
//! * [`convert`]: unit handling ([`to_inches_factor`]) and conversion of DXF
//!   entities into plan-core CAD objects ([`to_cad_objects`]).
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
    apply_cad, to_cad_objects, to_cad_objects_with, to_inches_factor, ImportOptions, LayerMapping,
    LayerTarget,
};
pub use dxf::{parse_dxf, DxfBlock, DxfDrawing, DxfEntity, DxfLayer, DxfUnits};
pub use formats::{is_3d_extension, parse_3d, suggest, Suggestion, SKP_MESSAGE};
pub use model::{ImportedModel, ImportedPart, ModelError, ModelOptions, UpAxis};
pub use shape::Facing;
pub use walls::{apply_walls, cad_to_walls, CadToWallsOptions, CadToWallsResult, WallProposal};

/// Why a drawing could not be imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The text does not look like an ASCII DXF file (no `SECTION` found).
    NotDxf,
    /// The file is a binary DXF, which this reader does not support.
    BinaryDxf,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::NotDxf => write!(f, "not an ASCII DXF file"),
            ImportError::BinaryDxf => write!(f, "binary DXF files are not supported"),
        }
    }
}

impl std::error::Error for ImportError {}
