//! Object size: the width / depth / height record in `Data4LibraryObjects`.
//!
//! Chief stores an object's default size as three floats followed by three
//! 1.0 scale factors, in the library-object record of the `Data` blob:
//!
//! ```text
//! FF FF FF FF [flag]  w d h  1.0 1.0 1.0
//! ```
//!
//! as `f32` in most Core Interiors / Exteriors records and as `f64` in the
//! parametric Core Architectural / MEP records (where the `f64` form is
//! `FF FF FF FF 03 0.5 w d h 1.0 1.0 1.0`). The anchor bytes are not stable
//! across record versions, so the decoder looks for the stable part: three
//! plausible dimensions followed by three exact 1.0 values.

use super::mesh::{parse_triangle_meshes, union_bounds};

/// Where a [`Size3`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeSource {
    /// The `w d h 1 1 1` record in `Data4LibraryObjects.Data` (exact).
    DataRecord,
    /// The plant record (spread and height) of a Core Plants object. Which
    /// double is the spread was inferred from tree thumbnails, not proven.
    PlantRecord,
    /// The bounding box of the object's decoded triangle meshes.
    MeshBounds,
}

/// An object's default size in inches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size3 {
    /// Extent along local X.
    pub width: f64,
    /// Extent along local Y (front to back).
    pub depth: f64,
    /// Extent along Z.
    pub height: f64,
    /// Height of the bottom above the floor. Not found in any blob yet, so
    /// this is always 0.0.
    pub elevation: f64,
    /// Which decoder produced the numbers.
    pub source: SizeSource,
}

const F32_ONE: [u8; 4] = [0x00, 0x00, 0x80, 0x3F];
const F64_ONE: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0x3F];
/// Bytes that precede the plant spread: `00 a6 91 3c` (a float near 0.018)
/// and a 1.0 double.
const PLANT_ANCHOR: [u8; 12] = [
    0x00, 0xA6, 0x91, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0x3F,
];

fn plausible_dim(v: f64) -> bool {
    v.is_finite() && (0.05..=2000.0).contains(&v)
}

fn all_ones(t: [f64; 3]) -> bool {
    t.iter().all(|v| (v - 1.0).abs() < 1e-6)
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if hay.len() < needle.len() || from > hay.len() - needle.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
}

/// Every `w d h 1 1 1` candidate as `(offset of w, [w, d, h])`, in blob order.
pub(crate) fn size_candidates(d: &[u8]) -> Vec<(usize, [f64; 3])> {
    let mut out: Vec<(usize, [f64; 3])> = Vec::new();
    let mut from = 12;
    let ones32 = [F32_ONE, F32_ONE, F32_ONE].concat();
    while let Some(p) = find(d, &ones32, from) {
        from = p + 1;
        let s = p - 12;
        let t = [0, 4, 8].map(|k| {
            f64::from(f32::from_le_bytes(
                d[s + k..s + k + 4].try_into().unwrap_or([0; 4]),
            ))
        });
        if t.iter().all(|&v| plausible_dim(v)) && !all_ones(t) {
            out.push((s, t));
        }
    }
    let ones64 = [F64_ONE, F64_ONE, F64_ONE].concat();
    from = 24;
    while let Some(p) = find(d, &ones64, from) {
        from = p + 1;
        let s = p - 24;
        let t = [0, 8, 16]
            .map(|k| f64::from_le_bytes(d[s + k..s + k + 8].try_into().unwrap_or([0; 8])));
        if t.iter().all(|&v| plausible_dim(v)) && !all_ones(t) {
            out.push((s, t));
        }
    }
    out.sort_by_key(|c| c.0);
    out
}

/// The object's own size record in a `Data` blob.
///
/// Composite objects embed the records of their parts; the object's own
/// record is the first one after the `Copyright` string of its header (when
/// there is no such string, the first record).
pub fn find_size_record(data: &[u8]) -> Option<[f64; 3]> {
    let cands = size_candidates(data);
    let after = find(data, b"Copyright", 0);
    let own = after
        .and_then(|k| cands.iter().find(|c| c.0 > k))
        .or_else(|| cands.first())?;
    Some(own.1)
}

/// Plant spread and height from a Core Plants `Data` blob, when it has the
/// plant record.
fn find_plant_record(data: &[u8]) -> Option<(f64, f64)> {
    let a = find(data, &PLANT_ANCHOR, 0)?;
    let spread = f64::from_le_bytes(data.get(a + 12..a + 20)?.try_into().ok()?);
    if !spread.is_finite() || !(1.0..=5000.0).contains(&spread) {
        return None;
    }
    // Height: the last non-zero u32 in the 48 bytes before the anchor.
    let mut height = spread;
    let lo = a.saturating_sub(48);
    let mut q = a;
    while q >= lo + 4 {
        q -= 4;
        let v = f64::from(u32::from_le_bytes(data[q..q + 4].try_into().ok()?));
        if v > 0.0 {
            if (6.0..=3000.0).contains(&v) && v >= 0.5 * spread {
                height = v;
            }
            break;
        }
    }
    Some((spread, height))
}

/// Decodes an object's default size.
///
/// 1. the `w d h 1 1 1` record of `data_blob` ([`SizeSource::DataRecord`]);
/// 2. the plant record ([`SizeSource::PlantRecord`]);
/// 3. the bounds of the triangle meshes in `associated_tail`, the part of the
///    `AssociatedData` blob after its JSON header ([`SizeSource::MeshBounds`]).
///
/// Sizes come back in inches with `elevation` 0.0.
pub fn decode_size(data_blob: &[u8], associated_tail: &[u8]) -> Option<Size3> {
    if let Some([w, d, h]) = find_size_record(data_blob) {
        return Some(Size3 {
            width: w,
            depth: d,
            height: h,
            elevation: 0.0,
            source: SizeSource::DataRecord,
        });
    }
    if let Some((spread, height)) = find_plant_record(data_blob) {
        return Some(Size3 {
            width: spread,
            depth: spread,
            height,
            elevation: 0.0,
            source: SizeSource::PlantRecord,
        });
    }
    let meshes = parse_triangle_meshes(associated_tail);
    let [w, d, h] = union_bounds(&meshes)?.extent();
    Some(Size3 {
        width: w,
        depth: d,
        height: h,
        elevation: 0.0,
        source: SizeSource::MeshBounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::mesh::testdata::{box_mesh, tri_mesh};

    fn f32_record(w: f32, d: f32, h: f32) -> Vec<u8> {
        let mut b = vec![0xFF; 4];
        b.push(1);
        for v in [w, d, h, 1.0, 1.0, 1.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b
    }

    fn f64_record(w: f64, d: f64, h: f64) -> Vec<u8> {
        let mut b = vec![0xFF; 4];
        b.push(3);
        for v in [0.5, w, d, h, 1.0, 1.0, 1.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b
    }

    #[test]
    fn finds_f32_and_f64_records() {
        let mut blob = vec![0u8; 40];
        blob.extend(f32_record(72.0, 37.5, 31.25));
        blob.extend_from_slice(&[0; 20]);
        let s = decode_size(&blob, &[]).unwrap();
        assert_eq!((s.width, s.depth, s.height), (72.0, 37.5, 31.25));
        assert_eq!(s.source, SizeSource::DataRecord);
        assert_eq!(s.elevation, 0.0);

        let mut blob = vec![0u8; 40];
        blob.extend(f64_record(38.0, 1.375, 79.875));
        let s = decode_size(&blob, &[]).unwrap();
        assert_eq!((s.width, s.depth, s.height), (38.0, 1.375, 79.875));
    }

    #[test]
    fn own_record_follows_the_copyright_string() {
        // A sub-object record, then the header, then the object's own record.
        let mut blob = f32_record(3.0, 2.0, 1.5);
        blob.extend_from_slice(b"..Copyright (c) test..");
        blob.extend(f32_record(60.0, 30.0, 34.0));
        blob.extend(f32_record(10.0, 10.0, 10.0));
        let s = decode_size(&blob, &[]).unwrap();
        assert_eq!((s.width, s.depth, s.height), (60.0, 30.0, 34.0));
    }

    #[test]
    fn ignores_unit_scale_blocks_and_implausible_values() {
        let mut blob = vec![0u8; 30];
        for _ in 0..3 {
            blob.extend_from_slice(&1.0f64.to_le_bytes());
        }
        for _ in 0..3 {
            blob.extend_from_slice(&1.0f64.to_le_bytes());
        }
        assert!(find_size_record(&blob).is_none());
        assert!(find_size_record(&f32_record(0.0, 5.0, 5.0)).is_none());
        assert!(find_size_record(&f32_record(1.0e6, 5.0, 5.0)).is_none());
        assert!(decode_size(&[], &[]).is_none());
    }

    #[test]
    fn falls_back_to_mesh_bounds_and_plants() {
        let (v, t) = box_mesh(18.0, 27.5, 32.0);
        let s = decode_size(b"no size here", &tri_mesh(&v, &t)).unwrap();
        assert_eq!(
            (s.width, s.depth, s.height, s.source),
            (18.0, 27.5, 32.0, SizeSource::MeshBounds)
        );

        let mut plant = vec![0u8; 30];
        plant.extend_from_slice(&180u32.to_le_bytes());
        plant.extend_from_slice(&[0u8; 20]);
        plant.extend_from_slice(&PLANT_ANCHOR);
        plant.extend_from_slice(&96.5f64.to_le_bytes());
        let s = decode_size(&plant, &[]).unwrap();
        assert_eq!(s.source, SizeSource::PlantRecord);
        assert_eq!((s.width, s.depth, s.height), (96.5, 96.5, 180.0));
    }
}
