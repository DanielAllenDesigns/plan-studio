//! Phase A scanner: header, thumbnail, length-prefixed strings, resource
//! table. Read-only; the template bytes never leave memory.
//!
//! Format notes (see `docs/chief-template-format.md`): a template starts with
//! the magic `01 CA 1A 10`, a small header of little-endian fields, an
//! embedded PNG thumbnail at 0x40, then a tagged stream of `u32` fields and
//! `u32`-length-prefixed 8-bit strings, and finally a table of resource paths.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// File magic at offsets 0x00 and 0x36.
pub const MAGIC: [u8; 4] = [0x01, 0xCA, 0x1A, 0x10];
/// The thumbnail PNG starts here.
pub const THUMBNAIL_OFFSET: usize = 0x40;
/// Longest string accepted by the walker.
pub const MAX_STRING_LEN: usize = 512;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Plan (`.plan`) or layout (`.layout`) file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemplateKind {
    Plan,
    Layout,
}

impl TemplateKind {
    /// Kind implied by a file extension (case-insensitive).
    pub fn from_extension(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "plan" => Some(TemplateKind::Plan),
            "layout" => Some(TemplateKind::Layout),
            _ => None,
        }
    }
}

/// The fixed 0x40-byte header. Field meanings are partly hypothesis; see the
/// format doc.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// `u16` at 0x04 (always 0 in the samples).
    pub version: u16,
    /// `u64` at 0x06: offset of the second magic (0x36 in the samples).
    pub second_magic_offset: u64,
    /// `u64` at 0x0E, 0x16, 0x1E, 0x26. `[0]` is the start of the resource
    /// table, `[3]` is the file size minus one; `[1]` and `[2]` are not decoded.
    pub offsets: [u64; 4],
    /// `u64` at 0x2E (1 in the samples).
    pub field_2e: u64,
    /// `u32` at 0x3C: byte length of the thumbnail PNG.
    pub thumbnail_len: u32,
}

/// Everything Phase A extracts from one template file.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateScan {
    /// Resolved kind: content-based when the content is conclusive, else the
    /// extension, else `Plan`.
    pub kind: TemplateKind,
    pub extension_kind: Option<TemplateKind>,
    pub content_kind: Option<TemplateKind>,
    pub header: Header,
    pub file_len: u64,
    pub thumbnail_png: Option<Vec<u8>>,
    /// First byte after the thumbnail; the string walk starts here.
    pub body_start: u64,
    /// Start of the trailing resource table.
    pub resource_table_start: u64,
    /// Every printable length-prefixed string with the offset of its length
    /// prefix, in file order.
    pub strings: Vec<(u64, String)>,
    /// Resource paths found in the tail table.
    pub resources: Vec<String>,
}

/// Reads and scans a template file.
pub fn scan(path: impl AsRef<Path>) -> Result<TemplateScan> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)?;
    scan_bytes(&bytes, TemplateKind::from_extension(path))
}

/// Scans an in-memory template. `extension_kind` is what the file name says.
pub fn scan_bytes(bytes: &[u8], extension_kind: Option<TemplateKind>) -> Result<TemplateScan> {
    let header = parse_header(bytes)?;
    let (thumbnail_png, body_start) = find_thumbnail(bytes, &header);
    let strings = walk_strings(bytes, body_start);
    let file_len = bytes.len();

    let h0 = header.offsets[0] as usize;
    let resource_table_start = if h0 >= body_start && h0 < file_len {
        h0
    } else {
        strings
            .iter()
            .find(|(_, s)| s == "Root Folder")
            .map_or(file_len, |(o, _)| *o as usize)
    };
    let resources = strings
        .iter()
        .filter(|(o, s)| *o as usize >= resource_table_start && is_resource_name(s))
        .map(|(_, s)| s.clone())
        .collect();

    let content_kind = content_kind(&strings);
    Ok(TemplateScan {
        kind: content_kind
            .or(extension_kind)
            .unwrap_or(TemplateKind::Plan),
        extension_kind,
        content_kind,
        header,
        file_len: file_len as u64,
        thumbnail_png,
        body_start: body_start as u64,
        resource_table_start: resource_table_start as u64,
        strings,
        resources,
    })
}

fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().expect("8 bytes"))
}

/// Validates the magic and decodes the fixed header.
pub fn parse_header(bytes: &[u8]) -> Result<Header> {
    if bytes.len() < THUMBNAIL_OFFSET {
        return Err(Error::Format(format!(
            "file is {} bytes, shorter than the 64-byte header",
            bytes.len()
        )));
    }
    if bytes[..4] != MAGIC {
        return Err(Error::Format("missing 01 CA 1A 10 magic".into()));
    }
    Ok(Header {
        version: u16::from_le_bytes([bytes[4], bytes[5]]),
        second_magic_offset: u64_at(bytes, 0x06),
        offsets: [
            u64_at(bytes, 0x0E),
            u64_at(bytes, 0x16),
            u64_at(bytes, 0x1E),
            u64_at(bytes, 0x26),
        ],
        field_2e: u64_at(bytes, 0x2E),
        thumbnail_len: u32::from_le_bytes(bytes[0x3C..0x40].try_into().expect("4 bytes")),
    })
}

/// Returns the thumbnail (if any) and the offset where the body starts.
fn find_thumbnail(bytes: &[u8], header: &Header) -> (Option<Vec<u8>>, usize) {
    let start = THUMBNAIL_OFFSET;
    if bytes.len() < start + 8 || bytes[start..start + 8] != PNG_SIGNATURE {
        return (None, start);
    }
    let declared = header.thumbnail_len as usize;
    if declared > 8
        && start + declared <= bytes.len()
        && ends_with_iend(&bytes[start..start + declared])
    {
        return (
            Some(bytes[start..start + declared].to_vec()),
            start + declared,
        );
    }
    // Header length missing or wrong: walk the PNG chunks.
    let mut pos = start + 8;
    while pos + 12 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().expect("4 bytes")) as usize;
        let ty = &bytes[pos + 4..pos + 8];
        let end = pos.saturating_add(12).saturating_add(len);
        if end > bytes.len() {
            break;
        }
        if ty == b"IEND" {
            return (Some(bytes[start..end].to_vec()), end);
        }
        pos = end;
    }
    (None, start)
}

fn ends_with_iend(png: &[u8]) -> bool {
    png.len() >= 12 && &png[png.len() - 8..png.len() - 4] == b"IEND"
}

/// Bytes allowed inside a name: printable ASCII plus a few MacRoman symbols
/// (copyright, registered, trademark, degree).
fn valid_byte(b: u8) -> bool {
    matches!(b, 0x20..=0x7E | 0xA1 | 0xA8 | 0xA9 | 0xAA)
}

fn decode_byte(b: u8) -> char {
    match b {
        0xA1 => '\u{B0}',
        0xA8 => '\u{AE}',
        0xA9 => '\u{A9}',
        0xAA => '\u{2122}',
        _ => b as char,
    }
}

/// Walks `buf` from `start`, collecting every `u32`-length-prefixed 8-bit
/// string (length 1..=512, charset-validated, at least one alphanumeric).
/// After an accepted string the walk continues just past it.
pub fn walk_strings(buf: &[u8], start: usize) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let mut i = start;
    while i + 4 <= buf.len() {
        // The length is < 0x300, so bytes 2 and 3 are zero.
        if buf[i + 2] == 0 && buf[i + 3] == 0 && buf[i + 1] <= 2 {
            let len = u16::from_le_bytes([buf[i], buf[i + 1]]) as usize;
            if (1..=MAX_STRING_LEN).contains(&len) && i + 4 + len <= buf.len() {
                let s = &buf[i + 4..i + 4 + len];
                if s.iter().all(|&b| valid_byte(b)) && s.iter().any(u8::is_ascii_alphanumeric) {
                    out.push((i as u64, s.iter().map(|&b| decode_byte(b)).collect()));
                    i += 4 + len;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

const RESOURCE_EXTENSIONS: [&str; 12] = [
    ".jpg", ".jpeg", ".png", ".bmp", ".tif", ".tiff", ".hdr", ".gif", ".plan", ".layout", ".pdf",
    ".dxf",
];

/// Whether a tail string is a resource reference (a path or an image/plan
/// file name) rather than table bookkeeping like `Root Folder`.
pub fn is_resource_name(s: &str) -> bool {
    if s == "Root Folder" {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    s.contains('/') || RESOURCE_EXTENSIONS.iter().any(|e| lower.ends_with(e))
}

/// Decides plan vs layout from content: a `Page Template` string only exists
/// in layouts; saved `... Plan View` entries only in plans.
fn content_kind(strings: &[(u64, String)]) -> Option<TemplateKind> {
    let mut layout = 0;
    let mut plan = 0;
    for (_, s) in strings {
        let t = s.trim();
        if t == "Page Template" || t == "Layout Text Style" || t == "Layout Dimensions" {
            layout += 1;
        } else if t.ends_with("Plan View") {
            plan += 1;
        }
    }
    match (layout, plan) {
        (0, 0) => None,
        (l, p) if l > p => Some(TemplateKind::Layout),
        (l, p) if p > l => Some(TemplateKind::Plan),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    //! Builds fake templates in memory.
    use super::*;

    /// Appends a `u32`-length-prefixed string.
    pub fn put_str(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(&(s.len() as u32).to_le_bytes());
        out.extend_from_slice(s.as_bytes());
    }

    /// Minimal valid 1x1 PNG-shaped blob: signature, empty IHDR/IEND chunks.
    pub fn fake_png() -> Vec<u8> {
        let mut p = PNG_SIGNATURE.to_vec();
        for (ty, data) in [(*b"IHDR", vec![0u8; 13]), (*b"IEND", vec![])] {
            p.extend_from_slice(&(data.len() as u32).to_be_bytes());
            p.extend_from_slice(&ty);
            p.extend_from_slice(&data);
            p.extend_from_slice(&[0, 0, 0, 0]);
        }
        p
    }

    /// Header + PNG + `body` + resource table made of `resources`.
    pub fn build_template(body: &[u8], resources: &[&str]) -> Vec<u8> {
        let png = fake_png();
        let mut out = vec![0u8; THUMBNAIL_OFFSET];
        out[..4].copy_from_slice(&MAGIC);
        out[6..14].copy_from_slice(&0x36u64.to_le_bytes());
        out[0x2E..0x36].copy_from_slice(&1u64.to_le_bytes());
        out[0x36..0x3A].copy_from_slice(&MAGIC);
        out[0x3C..0x40].copy_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&png);
        out.extend_from_slice(body);
        let table_start = out.len() as u64;
        out.extend_from_slice(&[0xF8, 0xDA, 0xDF, 0x84, 0, 0, 0, 0]);
        put_str(&mut out, "Root Folder");
        out.extend_from_slice(&[0, 0]);
        for r in resources {
            put_str(&mut out, r);
            out.extend_from_slice(&[0, 3, 0, 0, 0]);
        }
        let end = (out.len() - 1) as u64;
        out[0x0E..0x16].copy_from_slice(&table_start.to_le_bytes());
        out[0x26..0x2E].copy_from_slice(&end.to_le_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;

    fn sample_body() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&[1, 2, 3, 4, 5]);
        put_str(&mut b, "Elevation View Layer Set");
        b.extend_from_slice(&[0, 0, 7]);
        put_str(&mut b, "Siding-6");
        put_str(&mut b, " Working Plan View");
        b.extend_from_slice(&[0xFF; 3]);
        // A bogus length that points past the end must not panic.
        b.extend_from_slice(&600u32.to_le_bytes());
        b
    }

    #[test]
    fn header_thumbnail_and_strings() {
        let bytes = build_template(&sample_body(), &["/tex/Wood.jpg", "Ash.png"]);
        let s = scan_bytes(&bytes, Some(TemplateKind::Plan)).unwrap();
        assert_eq!(s.header.second_magic_offset, 0x36);
        assert_eq!(s.header.offsets[3] as usize, bytes.len() - 1);
        assert_eq!(s.thumbnail_png.as_deref(), Some(fake_png().as_slice()));
        assert_eq!(s.body_start as usize, THUMBNAIL_OFFSET + fake_png().len());
        let names: Vec<&str> = s.strings.iter().map(|(_, s)| s.as_str()).collect();
        assert!(names.contains(&"Elevation View Layer Set"));
        assert!(names.contains(&"Siding-6"));
        assert!(names.contains(&" Working Plan View"));
        // Offsets point at the length prefix.
        let (off, _) = s.strings.iter().find(|(_, s)| s == "Siding-6").unwrap();
        let o = *off as usize;
        assert_eq!(&bytes[o..o + 4], &8u32.to_le_bytes());
        assert_eq!(&bytes[o + 4..o + 12], b"Siding-6");
    }

    #[test]
    fn resource_table() {
        let bytes = build_template(&sample_body(), &["/tex/Wood.jpg", "Ash.png"]);
        let s = scan_bytes(&bytes, None).unwrap();
        assert_eq!(s.resources, vec!["/tex/Wood.jpg", "Ash.png"]);
        assert!(s.resource_table_start > s.body_start);
        // Body strings are not resources.
        assert!(!s.resources.iter().any(|r| r.contains("Siding")));
    }

    #[test]
    fn kind_by_extension_and_content() {
        let mut body = Vec::new();
        put_str(&mut body, "Page Template");
        put_str(&mut body, "Layout Text Style");
        let layout = build_template(&body, &[]);
        let s = scan_bytes(&layout, Some(TemplateKind::Plan)).unwrap();
        assert_eq!(s.content_kind, Some(TemplateKind::Layout));
        assert_eq!(s.kind, TemplateKind::Layout);
        assert_eq!(s.extension_kind, Some(TemplateKind::Plan));

        let plan = build_template(&sample_body(), &[]);
        let s = scan_bytes(&plan, Some(TemplateKind::Layout)).unwrap();
        assert_eq!(s.content_kind, Some(TemplateKind::Plan));

        let mut empty_body = Vec::new();
        put_str(&mut empty_body, "Nothing");
        let s = scan_bytes(
            &build_template(&empty_body, &[]),
            Some(TemplateKind::Layout),
        )
        .unwrap();
        assert_eq!(s.content_kind, None);
        assert_eq!(s.kind, TemplateKind::Layout);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(scan_bytes(&[0; 10], None), Err(Error::Format(_))));
        let mut bad = build_template(&[], &[]);
        bad[0] = 0;
        assert!(matches!(scan_bytes(&bad, None), Err(Error::Format(_))));
    }

    #[test]
    fn walker_filters_noise() {
        let mut b = Vec::new();
        put_str(&mut b, "%"); // no alphanumeric
        put_str(&mut b, "ok"); // short but valid
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&[b'a', 0x01, b'c']); // control byte
        put_str(&mut b, "Copyright\u{0}"); // NUL inside is rejected
        let got = walk_strings(&b, 0);
        let names: Vec<&str> = got.iter().map(|(_, s)| s.as_str()).collect();
        assert_eq!(names, vec!["ok"]);
    }

    #[test]
    fn macroman_symbols() {
        let mut b = Vec::new();
        b.extend_from_slice(&5u32.to_le_bytes());
        b.extend_from_slice(&[0xA9, b' ', b'2', b'0', b'x']);
        let got = walk_strings(&b, 0);
        assert_eq!(got[0].1, "\u{A9} 20x");
    }

    #[test]
    fn thumbnail_fallback_without_declared_length() {
        let mut bytes = build_template(&sample_body(), &[]);
        bytes[0x3C..0x40].copy_from_slice(&0u32.to_le_bytes());
        let s = scan_bytes(&bytes, None).unwrap();
        assert_eq!(s.thumbnail_png.as_deref(), Some(fake_png().as_slice()));
    }
}
