//! Tolerant DXF reader for ASCII and binary files, R12 through 2018.
//!
//! The file is read as group-code / value pairs ([`tokens`]), grouped into
//! records and turned into a [`DxfDrawing`] ([`model`]): the header
//! (`$ACADVER`, `$INSUNITS`, `$MEASUREMENT`, `$DIMSCALE`, extents), the tables
//! (layers with colour, line type and weight; line types; text styles;
//! dimension styles), block definitions (including anonymous dimension blocks
//! and external references) and the entities of model space and of the first
//! paper space page.
//!
//! Entities read: LINE, LWPOLYLINE, POLYLINE (2D, spline frames, polyface
//! meshes), CIRCLE, ARC, ELLIPSE, SPLINE, HATCH, SOLID, TRACE, 3DFACE, POINT,
//! TEXT, MTEXT (formatting codes), ATTRIB, ATTDEF, DIMENSION, LEADER,
//! MULTILEADER and INSERT/MINSERT. Anything else is skipped and tallied in
//! [`DxfDrawing::skipped`]. [`convert`](crate::convert) turns a drawing into
//! plan objects.

pub mod aci;
pub mod geom;
pub mod model;
mod read;
#[cfg(test)]
mod tests;
pub mod text;
pub mod tokens;

pub use model::*;

use crate::ImportError;

/// Parse an ASCII DXF document.
///
/// # Errors
/// [`ImportError::BinaryDxf`] for the binary sentinel (use
/// [`parse_dxf_bytes`]) and [`ImportError::NotDxf`] when no `SECTION` can be
/// found.
pub fn parse_dxf(text: &str) -> Result<DxfDrawing, ImportError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.starts_with("AutoCAD Binary DXF") {
        return Err(ImportError::BinaryDxf);
    }
    let pairs = tokens::tokenize_ascii(text);
    build(&pairs, DxfFormat::Ascii)
}

/// Parse a DXF file from its bytes, ASCII or binary. A DWG file is refused
/// with [`ImportError::Dwg`].
///
/// # Errors
/// [`ImportError::Dwg`], [`ImportError::NotDxf`].
pub fn parse_dxf_bytes(bytes: &[u8]) -> Result<DxfDrawing, ImportError> {
    if let Some(v) = dwg_version(bytes) {
        return Err(ImportError::Dwg(v));
    }
    if tokens::is_binary(bytes) {
        let pairs = tokens::tokenize_binary(bytes).map_err(|_| ImportError::NotDxf)?;
        return build(&pairs, DxfFormat::Binary);
    }
    let text = tokens::decode_text(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let pairs = tokens::tokenize_ascii(text);
    build(&pairs, DxfFormat::Ascii)
}

fn build(pairs: &[tokens::Pair<'_>], format: DxfFormat) -> Result<DxfDrawing, ImportError> {
    let recs = read::records(pairs);
    if !recs.iter().any(|r| r.kind == "SECTION") {
        return Err(ImportError::NotDxf);
    }
    let mut drawing = DxfDrawing {
        format,
        ..DxfDrawing::default()
    };
    read::read_drawing(&recs, &mut drawing);
    Ok(drawing)
}

/// The AutoCAD release a DWG file was saved by (`"AC1027"` is 2013), or
/// `None` when the bytes are not a DWG. DWG files start with `AC10` and a
/// two-digit release number, then zero bytes.
pub fn dwg_version(bytes: &[u8]) -> Option<String> {
    let head = bytes.get(..6)?;
    if &head[..4] != b"AC10" || !head[4..].iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(String::from_utf8_lossy(head).into_owned())
}

/// The AutoCAD name of a DWG/DXF release code.
pub fn release_name(code: &str) -> &'static str {
    match code {
        "AC1006" => "R10",
        "AC1009" => "R11/R12",
        "AC1012" => "R13",
        "AC1014" => "R14",
        "AC1015" => "2000",
        "AC1018" => "2004",
        "AC1021" => "2007",
        "AC1024" => "2010",
        "AC1027" => "2013",
        "AC1032" => "2018",
        _ => "an unknown release",
    }
}

/// What to tell the user about a DWG file (Plan Studio reads DXF only).
pub fn dwg_guidance(code: &str) -> String {
    format!(
        "This is an AutoCAD DWG file (release {}). Plan Studio reads DXF: in AutoCAD, BricsCAD or LibreCAD choose Save As and pick DXF (any release from R12 to 2018), then import that file.",
        release_name(code)
    )
}
