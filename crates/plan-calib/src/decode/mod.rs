//! Decoders for the binary blobs inside Chief library catalogs.
//!
//! What is decoded (details: `README.md`, `docs/chief-library-format.md` 7):
//!
//! * [`decode_size`]: default width / depth / height from the `w d h 1 1 1`
//!   record of `Data4LibraryObjects.Data`, falling back to plant records and
//!   to mesh bounds;
//! * [`decode_symbol_2d`]: `LibrarySymbolData.symDxf`, which is a 3D polygon
//!   face stream and not DXF, projected to a plan-view drawing;
//! * [`parse_triangle_meshes`]: the `CD AB 74 00` triangle meshes in
//!   `AssociatedData` and `SymbolData`, projected the same way;
//! * [`decode_cdab_records`]: a generic walker for `CD AB` records (strings
//!   and number runs);
//! * [`decode_object`]: all of the above for one object, with a consistency
//!   check between the size and the symbol.

mod cdab;
mod mesh;
mod plan;
mod size;

pub use cdab::{decode_cdab_records, CdabRecord};
#[cfg(test)]
pub(crate) use mesh::testdata;
pub use mesh::{parse_face_stream, parse_triangle_meshes, union_bounds, Bounds3, FaceStream, Mesh};
pub use plan::{plan_view, PlanOptions, PlanView};
pub use size::{decode_size, find_size_record, Size3, SizeSource};

use crate::error::{Error, Result};
use plan_core::geometry::Point;
use plan_library::{Bounds, Stroke, Symbol2d};

/// Relative tolerance of the "symbol matches size" test.
pub const FIT_TOLERANCE: f64 = 0.10;

/// A plan-view symbol decoded from geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedSymbol {
    /// Strokes in inches, centred on the origin, front towards +Y.
    pub strokes: Vec<Stroke>,
    /// Bounds of the strokes.
    pub bounds: Bounds,
    /// Layer / material names found in the source (symDxf only).
    pub layers: Vec<String>,
    /// X, Y and Z extent of the source geometry (before any clipping to the
    /// strokes), in inches.
    pub extent: [f64; 3],
    /// Where the strokes came from.
    pub source: SymbolSource,
}

/// How a [`DecodedSymbol`] was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolSource {
    /// Plan view of the `symDxf` polygon faces.
    FaceStream,
    /// Plan view of `CD AB 74 00` triangle meshes.
    TriangleMesh,
    /// A canopy circle built from a plant record.
    PlantCanopy,
}

fn symbol_from_view(
    view: PlanView,
    layers: Vec<String>,
    source: SymbolSource,
) -> Result<DecodedSymbol> {
    let bounds = Symbol2d::new(view.strokes.clone())
        .bounds()
        .ok_or_else(|| Error::corrupt("decoded symbol has no strokes"))?;
    Ok(DecodedSymbol {
        strokes: view.strokes,
        bounds,
        layers,
        extent: [view.width, view.depth, view.height],
        source,
    })
}

/// Decodes a `LibrarySymbolData.symDxf` blob into a plan-view symbol.
///
/// `symDxf` is not DXF (no group codes, not AutoCAD binary DXF): it is a list
/// of 3D polygon faces in inches. The symbol is the top-down hidden-line view
/// of those faces, see [`plan_view`]. Fails when no face can be found.
pub fn decode_symbol_2d(sym_dxf: &[u8]) -> Result<DecodedSymbol> {
    let fs = parse_face_stream(sym_dxf)
        .ok_or_else(|| Error::corrupt("symDxf: no polygon faces found"))?;
    let view = plan_view(&[&fs.mesh], PlanOptions::default())
        .ok_or_else(|| Error::corrupt("symDxf: faces have no plan extent"))?;
    symbol_from_view(view, fs.layers, SymbolSource::FaceStream)
}

/// Plan-view symbol of triangle meshes (for example from
/// [`parse_triangle_meshes`]).
pub fn symbol_from_meshes(meshes: &[Mesh]) -> Result<DecodedSymbol> {
    let refs: Vec<&Mesh> = meshes.iter().collect();
    let view = plan_view(&refs, PlanOptions::default())
        .ok_or_else(|| Error::corrupt("meshes have no plan extent"))?;
    symbol_from_view(view, Vec::new(), SymbolSource::TriangleMesh)
}

/// Splits an `AssociatedData` blob into its JSON header and binary tail.
///
/// The blob starts with 12 flag bytes, a `u32` JSON length and the JSON text;
/// the binary stream follows. When the blob does not have that shape the
/// whole blob is returned as the tail.
pub fn split_associated(blob: &[u8]) -> (Option<&[u8]>, &[u8]) {
    if blob.len() >= 17 && (blob[16] == b'{' || blob[16] == b'[') {
        let len = u32::from_le_bytes([blob[12], blob[13], blob[14], blob[15]]) as usize;
        if len > 0 && 16 + len <= blob.len() {
            return (Some(&blob[16..16 + len]), &blob[16 + len..]);
        }
    }
    (None, blob)
}

/// The raw blobs of one library object.
#[derive(Debug, Clone, Default)]
pub struct ObjectBlobs {
    /// `Data4LibraryObjects.Data`.
    pub data: Option<Vec<u8>>,
    /// `AssociatedData.AssociatedDataBlob` (JSON header plus binary tail).
    pub associated: Option<Vec<u8>>,
    /// `LibrarySymbolData.symDxf`.
    pub sym_dxf: Option<Vec<u8>>,
    /// `LibrarySymbolData.symBlock` (a name plus parameters; not decoded).
    pub sym_block: Option<Vec<u8>>,
    /// `SymbolData4LibraryObjects.SymbolData`.
    pub symbol_data: Option<Vec<u8>>,
}

/// Everything decoded for one object.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedObject {
    /// Default size: the `Data` record when there is one, else the geometry
    /// bounds.
    pub size: Option<Size3>,
    /// Plan symbol, when the object has geometry or a plant record.
    pub symbol: Option<DecodedSymbol>,
    /// Whether the symbol's X/Y extent is within [`FIT_TOLERANCE`] of the
    /// size (always true when the size came from the geometry itself).
    /// `None` when there is no symbol or no size.
    pub symbol_fits_size: Option<bool>,
}

/// Do two extents agree within [`FIT_TOLERANCE`] (plus 0.1 in of slack)?
pub fn extents_agree(a: f64, b: f64) -> bool {
    (a - b).abs() <= FIT_TOLERANCE * a.max(b) + 0.1
}

/// Decodes the size and plan symbol of one object.
///
/// Geometry comes from `symDxf` faces, `SymbolData` meshes and the meshes in
/// the `AssociatedData` tail (all merged). The size prefers the `Data` record
/// and otherwise uses the geometry bounds; a symbol whose footprint disagrees
/// with the `Data` size is returned but flagged by `symbol_fits_size`.
pub fn decode_object(blobs: &ObjectBlobs) -> DecodedObject {
    let mut meshes: Vec<Mesh> = Vec::new();
    let mut layers: Vec<String> = Vec::new();
    let mut source = SymbolSource::TriangleMesh;
    if let Some(fs) = blobs.sym_dxf.as_deref().and_then(parse_face_stream) {
        layers = fs.layers;
        meshes.push(fs.mesh);
        source = SymbolSource::FaceStream;
    }
    if let Some(sd) = blobs.symbol_data.as_deref() {
        meshes.extend(parse_triangle_meshes(sd));
    }
    if let Some(a) = blobs.associated.as_deref() {
        meshes.extend(parse_triangle_meshes(split_associated(a).1));
    }

    let data = blobs.data.as_deref().unwrap_or(&[]);
    let record = decode_size(data, &[]);
    let mesh_bounds = union_bounds(&meshes);
    let mesh_size = mesh_bounds.map(|b| {
        let [w, d, h] = b.extent();
        Size3 {
            width: w,
            depth: d,
            height: h,
            elevation: 0.0,
            source: SizeSource::MeshBounds,
        }
    });
    let size = record.or(mesh_size);

    let mut symbol = if meshes.is_empty() {
        None
    } else {
        let refs: Vec<&Mesh> = meshes.iter().collect();
        plan_view(&refs, PlanOptions::default())
            .and_then(|v| symbol_from_view(v, layers, source).ok())
    };
    if symbol.is_none() {
        if let Some(s) = record.filter(|s| s.source == SizeSource::PlantRecord) {
            let r = s.width / 2.0;
            let strokes = vec![Stroke::Circle {
                center: Point::new(0.0, 0.0),
                radius: r,
            }];
            let bounds = Bounds {
                min: Point::new(-r, -r),
                max: Point::new(r, r),
            };
            symbol = Some(DecodedSymbol {
                strokes,
                bounds,
                layers: Vec::new(),
                extent: [s.width, s.depth, s.height],
                source: SymbolSource::PlantCanopy,
            });
        }
    }
    let symbol_fits_size = match (&symbol, &size) {
        (Some(sym), Some(sz)) => {
            Some(extents_agree(sym.extent[0], sz.width) && extents_agree(sym.extent[1], sz.depth))
        }
        _ => None,
    };
    DecodedObject {
        size,
        symbol,
        symbol_fits_size,
    }
}

#[cfg(test)]
mod tests {
    use super::mesh::testdata::{box_mesh, face_stream, tri_mesh};
    use super::*;

    fn data_with_size(w: f32, d: f32, h: f32) -> Vec<u8> {
        let mut b = vec![0u8; 20];
        b.extend_from_slice(&[0xFF; 4]);
        b.push(1);
        for v in [w, d, h, 1.0, 1.0, 1.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b
    }

    #[test]
    fn symdxf_to_symbol() {
        let quad = |z: f64| {
            [
                [0.0, 0.0, z],
                [24.0, 0.0, z],
                [24.0, 12.0, z],
                [0.0, 12.0, z],
            ]
        };
        let sym = decode_symbol_2d(&face_stream(&[quad(0.0), quad(30.0)])).unwrap();
        assert_eq!(sym.source, SymbolSource::FaceStream);
        assert_eq!(sym.layers, ["Adjust Light"]);
        assert_eq!(sym.extent, [24.0, 12.0, 30.0]);
        assert!((sym.bounds.width() - 24.0).abs() < 0.1);
        assert!(decode_symbol_2d(b"junk").is_err());
    }

    #[test]
    fn splits_associated_blobs() {
        let json = br#"{"Version": 4}"#;
        let mut blob = vec![0u8; 12];
        blob.extend_from_slice(&(json.len() as u32).to_le_bytes());
        blob.extend_from_slice(json);
        blob.extend_from_slice(&[1, 2, 3]);
        let (j, tail) = split_associated(&blob);
        assert_eq!(j, Some(&json[..]));
        assert_eq!(tail, [1, 2, 3]);
        let (j, tail) = split_associated(b"plain");
        assert!(j.is_none());
        assert_eq!(tail, b"plain");
    }

    #[test]
    fn object_with_matching_and_mismatching_size() {
        let (v, t) = box_mesh(36.0, 24.0, 34.0);
        let mut assoc = vec![0u8; 12];
        assoc.extend_from_slice(&2u32.to_le_bytes());
        assoc.extend_from_slice(b"{}");
        assoc.extend(tri_mesh(&v, &t));

        let ok = decode_object(&ObjectBlobs {
            data: Some(data_with_size(36.0, 24.0, 34.0)),
            associated: Some(assoc.clone()),
            ..Default::default()
        });
        assert_eq!(ok.size.unwrap().source, SizeSource::DataRecord);
        assert_eq!(ok.symbol_fits_size, Some(true));

        let off = decode_object(&ObjectBlobs {
            data: Some(data_with_size(50.0, 24.0, 34.0)),
            associated: Some(assoc.clone()),
            ..Default::default()
        });
        assert_eq!(off.symbol_fits_size, Some(false));
        assert!(off.symbol.is_some());

        // No Data record: the geometry decides.
        let geo = decode_object(&ObjectBlobs {
            associated: Some(assoc),
            ..Default::default()
        });
        let s = geo.size.unwrap();
        assert_eq!(
            (s.width, s.depth, s.height, s.source),
            (36.0, 24.0, 34.0, SizeSource::MeshBounds)
        );
        assert_eq!(geo.symbol_fits_size, Some(true));

        let none = decode_object(&ObjectBlobs::default());
        assert!(none.size.is_none() && none.symbol.is_none());
        assert!(none.symbol_fits_size.is_none());
    }
}
