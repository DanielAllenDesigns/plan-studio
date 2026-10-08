//! A generic walker for Chief `CD AB` records.
//!
//! Every Chief object stream (`Data4LibraryObjects.Data`, the binary tail of
//! `AssociatedData`, `SymbolData4LibraryObjects.SymbolData`) is a flat run of
//! records introduced by the magic `CD AB` and a little-endian `u16` kind.
//! Records nest (a `0x7b` wrapper holds a `0x78` and a `0x72` object record),
//! and no kind carries its own length, so the walker splits the blob at every
//! magic: a record's `raw_len` runs to the next magic, and a parent's bytes up
//! to its first child are its own.
//!
//! Inside a record:
//!
//! * **strings** are `u32` length, that many 8-bit characters, then a NUL
//!   (`"Copyright\xC2\xA9 2010, Chief Architect, Inc. "`, `"description"`,
//!   `"%automatic_description%"`, layer and material names);
//! * **numbers** are found heuristically as runs of at least three
//!   consecutive `f64` (stride 8) or `f32` (stride 4) values that look like
//!   dimensions. Sizes, positions and plant parameters show up here.

/// One `CD AB <kind>` record.
#[derive(Debug, Clone, PartialEq)]
pub struct CdabRecord {
    /// The record kind (0x72 library object, 0x7b wrapper, 0x74 mesh, ...).
    pub kind: u16,
    /// Offset of the `CD AB` magic in the blob.
    pub offset: usize,
    /// Length of the record up to the next magic (or the end of the blob).
    pub raw_len: usize,
    /// Length-prefixed strings in the record, in order.
    pub strings: Vec<String>,
    /// Plausible numeric values (non-zero members of the detected runs).
    pub numbers: Vec<f64>,
}

/// Splits `blob` into [`CdabRecord`]s. Bytes before the first magic are
/// ignored. Never fails: a blob without a magic yields an empty list.
pub fn decode_cdab_records(blob: &[u8]) -> Vec<CdabRecord> {
    let mut starts: Vec<(usize, u16)> = Vec::new();
    let mut i = 0;
    while i + 4 <= blob.len() {
        if blob[i] == 0xCD && blob[i + 1] == 0xAB {
            let kind = u16::from_le_bytes([blob[i + 2], blob[i + 3]]);
            if kind != 0 && kind != 0xABCD {
                starts.push((i, kind));
                i += 4;
                continue;
            }
        }
        i += 1;
    }
    let mut out = Vec::with_capacity(starts.len());
    for (n, &(off, kind)) in starts.iter().enumerate() {
        let end = starts.get(n + 1).map_or(blob.len(), |s| s.0);
        let body = &blob[off + 4..end];
        out.push(CdabRecord {
            kind,
            offset: off,
            raw_len: end - off,
            strings: find_strings(body),
            numbers: find_numbers(body),
        });
    }
    out
}

fn decode_text(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_owned(),
        // MacRoman / Latin-1 copyright sign and friends.
        Err(_) => b.iter().map(|&c| char::from(c)).collect(),
    }
}

fn find_strings(b: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 5 <= b.len() {
        let len = u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
        if (2..=400).contains(&len) && i + 4 + len < b.len() && b[i + 4 + len] == 0 {
            let s = &b[i + 4..i + 4 + len];
            let printable = s.iter().all(|&c| c >= 0x20 && c != 0x7F);
            if printable && s.iter().any(u8::is_ascii_alphanumeric) {
                out.push(decode_text(s));
                i += 4 + len + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn dimension_like(v: f64) -> bool {
    v.is_finite() && (0.001..=1.0e5).contains(&v.abs())
}

/// Does the double look like a float widened to 64 bits (low 29 bits clear)
/// or a short binary fraction?
fn float_derived(bits: u64) -> bool {
    bits & 0x1FFF_FFFF == 0
}

fn find_numbers(b: &[u8]) -> Vec<f64> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 24 <= b.len() {
        // f64 run
        let mut j = i;
        let mut run: Vec<f64> = Vec::new();
        while j + 8 <= b.len() {
            let bits = u64::from_le_bytes(b[j..j + 8].try_into().unwrap_or([0; 8]));
            let v = f64::from_bits(bits);
            if bits == 0 || (dimension_like(v) && float_derived(bits)) {
                run.push(v);
                j += 8;
            } else {
                break;
            }
        }
        let nonzero: Vec<f64> = run.iter().copied().filter(|v| *v != 0.0).collect();
        if run.len() >= 3 && nonzero.len() >= 2 {
            out.extend(nonzero);
            i = j;
            continue;
        }
        i += 1;
    }
    // f32 runs over the bytes not already explained: keep only runs of three
    // or more non-zero floats in a plausible range.
    let mut i = 0;
    while i + 12 <= b.len() {
        let mut j = i;
        let mut run: Vec<f64> = Vec::new();
        while j + 4 <= b.len() {
            let v = f64::from(f32::from_le_bytes(b[j..j + 4].try_into().unwrap_or([0; 4])));
            if v.is_finite() && (0.05..=5000.0).contains(&v.abs()) {
                run.push(v);
                j += 4;
            } else {
                break;
            }
        }
        if run.len() >= 3 {
            for v in run {
                if !out.iter().any(|o: &f64| (o - v).abs() < 1e-6) {
                    out.push(v);
                }
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(s: &str) -> Vec<u8> {
        let mut b = (s.len() as u32).to_le_bytes().to_vec();
        b.extend_from_slice(s.as_bytes());
        b.push(0);
        b
    }

    #[test]
    fn walks_records_strings_and_numbers() {
        let mut blob = vec![7u8, 7, 7];
        blob.extend_from_slice(&[0xCD, 0xAB, 0x72, 0x00]);
        blob.extend_from_slice(&[0; 9]);
        blob.extend(string("Copyright\u{a9} 2010, Chief Architect, Inc. "));
        blob.extend(string("Door E29"));
        blob.extend_from_slice(&[0; 5]);
        let second = blob.len();
        blob.extend_from_slice(&[0xCD, 0xAB, 0x74, 0x00]);
        for v in [38.0f64, 1.375, 79.875, 1.0, 1.0, 1.0] {
            blob.extend_from_slice(&v.to_le_bytes());
        }
        let recs = decode_cdab_records(&blob);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].kind, 0x72);
        assert_eq!(recs[0].offset, 3);
        assert_eq!(recs[0].raw_len, second - 3);
        assert_eq!(
            recs[0].strings,
            ["Copyright\u{a9} 2010, Chief Architect, Inc. ", "Door E29"]
        );
        assert_eq!(recs[1].kind, 0x74);
        assert_eq!(recs[1].raw_len, blob.len() - second);
        assert_eq!(recs[1].numbers, [38.0, 1.375, 79.875, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn latin1_strings_and_empty_input() {
        let mut blob = vec![0xCD, 0xAB, 0x61, 0x00];
        blob.extend_from_slice(&[9, 0, 0, 0]);
        blob.extend_from_slice(b"Copy\xA9 txt");
        blob.push(0);
        let recs = decode_cdab_records(&blob);
        assert_eq!(recs[0].strings, ["Copy\u{a9} txt"]);
        assert!(decode_cdab_records(&[]).is_empty());
        assert!(decode_cdab_records(b"no magic here").is_empty());
    }

    #[test]
    fn f32_runs() {
        let mut blob = vec![0xCD, 0xAB, 0x72, 0x00, 0, 0];
        for v in [72.0f32, 37.125, 31.25] {
            blob.extend_from_slice(&v.to_le_bytes());
        }
        let recs = decode_cdab_records(&blob);
        assert_eq!(recs[0].numbers, [72.0, 37.125, 31.25]);
    }
}
