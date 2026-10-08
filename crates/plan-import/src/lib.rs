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
//! All lengths are inches once converted; the reader itself keeps the raw
//! drawing units.

pub mod convert;
pub mod dxf;
pub mod walls;

pub use convert::{apply_cad, to_cad_objects, to_inches_factor};
pub use dxf::{parse_dxf, DxfBlock, DxfDrawing, DxfEntity, DxfLayer, DxfUnits};
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
