//! Export and import of the user library as one zip file.
//!
//! The file is a plain (uncompressed, "stored") zip container, saved with the
//! `.calibz` extension so it sits next to Chief Architect libraries in a file
//! picker. **Chief Architect cannot read it**: the entries are Plan Studio's
//! own JSON catalog and `.psm` meshes ([`crate::model`]), not Chief's SQLite
//! `.calib` database. Plan Studio reads Chief catalogs in place instead (see
//! DECISIONS.md #3).
//!
//! Layout:
//!
//! * `plan-studio-library.json`: `{ "format", "version", "catalog", "meta" }`
//! * `user-models/<name>.psm`: the 3D model of each item (paths equal the
//!   items' `model3d`)
//! * `README.txt`: what this file is
//!
//! [`import_library`] reads exactly what [`export_library`] writes. Zips made
//! by other tools work only if their entries are stored (not deflated).

use crate::catalog::{Catalog, CatalogItem};
use crate::manage::UserMeta;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Name of the JSON entry.
pub const MANIFEST: &str = "plan-studio-library.json";
/// Value of the `format` field.
pub const FORMAT: &str = "plan-studio-library";
/// Folder (and path prefix) of the model entries.
pub const MODEL_DIR: &str = "user-models/";

const README: &str = "Plan Studio user library export.\n\
\n\
This zip holds Plan Studio's own catalog JSON and 3D models (.psm). It is not\n\
a Chief Architect .calib or .calibz library: Chief Architect cannot open it.\n\
Import it in Plan Studio with Library > Import Library.\n";

/// Why an archive could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveError(pub String);

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ArchiveError {}

fn err<T>(m: impl Into<String>) -> Result<T, ArchiveError> {
    Err(ArchiveError(m.into()))
}

// ----- crc32 -----

fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for (n, slot) in t.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    t
}

/// The CRC-32 (IEEE) of `data`, as zip stores it.
pub fn crc32(data: &[u8]) -> u32 {
    let table = crc_table();
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

// ----- zip -----

const LOCAL_SIG: u32 = 0x0403_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const END_SIG: u32 = 0x0605_4b50;

fn le16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn le32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Writes `entries` (name, bytes) as a zip with every entry stored.
pub fn write_zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let name = name.as_bytes();
        le32(&mut out, LOCAL_SIG);
        le16(&mut out, 20); // version needed
        le16(&mut out, 0x0800); // UTF-8 names
        le16(&mut out, 0); // stored
        le16(&mut out, 0); // time
        le16(&mut out, 0x21); // date 1980-01-01
        le32(&mut out, crc);
        le32(&mut out, data.len() as u32);
        le32(&mut out, data.len() as u32);
        le16(&mut out, name.len() as u16);
        le16(&mut out, 0);
        out.extend_from_slice(name);
        out.extend_from_slice(data);

        le32(&mut central, CENTRAL_SIG);
        le16(&mut central, 20); // made by
        le16(&mut central, 20); // needed
        le16(&mut central, 0x0800);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0x21);
        le32(&mut central, crc);
        le32(&mut central, data.len() as u32);
        le32(&mut central, data.len() as u32);
        le16(&mut central, name.len() as u16);
        le16(&mut central, 0); // extra
        le16(&mut central, 0); // comment
        le16(&mut central, 0); // disk
        le16(&mut central, 0); // internal attrs
        le32(&mut central, 0); // external attrs
        le32(&mut central, offset);
        central.extend_from_slice(name);
    }
    let start = out.len() as u32;
    let size = central.len() as u32;
    out.extend_from_slice(&central);
    le32(&mut out, END_SIG);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, entries.len() as u16);
    le16(&mut out, entries.len() as u16);
    le32(&mut out, size);
    le32(&mut out, start);
    le16(&mut out, 0);
    out
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(at)?,
        *b.get(at + 1)?,
        *b.get(at + 2)?,
        *b.get(at + 3)?,
    ]))
}

/// Reads the entries of a zip whose entries are stored; checks every CRC.
pub fn read_zip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ArchiveError> {
    if bytes.starts_with(b"SQLite format 3") {
        return err(
            "This is a Chief Architect .calib database. Plan Studio reads Chief libraries in \
             place (Library > Catalog Settings); only Plan Studio exports can be imported.",
        );
    }
    if bytes.len() < 22
        || !(bytes.starts_with(&LOCAL_SIG.to_le_bytes())
            || bytes.starts_with(&END_SIG.to_le_bytes()))
    {
        return err("Not a zip file");
    }
    let end = (0..=bytes.len() - 22)
        .rev()
        .find(|&i| u32_at(bytes, i) == Some(END_SIG))
        .ok_or_else(|| ArchiveError("Not a zip file (no end record)".into()))?;
    let count = u16_at(bytes, end + 10).unwrap_or(0) as usize;
    let mut at = u32_at(bytes, end + 16).unwrap_or(0) as usize;
    let mut out = Vec::new();
    for _ in 0..count {
        if u32_at(bytes, at) != Some(CENTRAL_SIG) {
            return err("Damaged zip directory");
        }
        let method = u16_at(bytes, at + 10).unwrap_or(0xFFFF);
        let crc = u32_at(bytes, at + 16).unwrap_or(0);
        let csize = u32_at(bytes, at + 20).unwrap_or(0) as usize;
        let nlen = u16_at(bytes, at + 28).unwrap_or(0) as usize;
        let xlen = u16_at(bytes, at + 30).unwrap_or(0) as usize;
        let clen = u16_at(bytes, at + 32).unwrap_or(0) as usize;
        let local = u32_at(bytes, at + 42).unwrap_or(0) as usize;
        let name = bytes
            .get(at + 46..at + 46 + nlen)
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .ok_or_else(|| ArchiveError("Damaged zip directory".into()))?;
        at += 46 + nlen + xlen + clen;
        if name.ends_with('/') {
            continue;
        }
        if method != 0 {
            return err(format!(
                "\"{name}\" is compressed. Only uncompressed Plan Studio exports can be imported."
            ));
        }
        if u32_at(bytes, local) != Some(LOCAL_SIG) {
            return err("Damaged zip entry");
        }
        let lnlen = u16_at(bytes, local + 26).unwrap_or(0) as usize;
        let lxlen = u16_at(bytes, local + 28).unwrap_or(0) as usize;
        let start = local + 30 + lnlen + lxlen;
        let data = bytes
            .get(start..start.saturating_add(csize))
            .ok_or_else(|| ArchiveError("Truncated zip entry".into()))?;
        if crc32(data) != crc {
            return err(format!("\"{name}\" is damaged (checksum mismatch)"));
        }
        out.push((name, data.to_vec()));
    }
    Ok(out)
}

// ----- library archive -----

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    catalog: Catalog,
    #[serde(default)]
    meta: UserMeta,
}

/// What an export holds.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryArchive {
    /// The items.
    pub items: Vec<CatalogItem>,
    /// Folders and favorites (recents are not exported).
    pub meta: UserMeta,
    /// Model files: path (as in the item's `model3d`) and bytes.
    pub models: Vec<(String, Vec<u8>)>,
}

/// The model path an archive may carry: under [`MODEL_DIR`], no `..`.
pub fn safe_model_path(path: &str) -> bool {
    path.starts_with(MODEL_DIR)
        && !path.contains("..")
        && !path.contains('\\')
        && path.len() > MODEL_DIR.len()
}

/// Writes `items` with their `meta` and model files as an archive.
/// `model_bytes` returns the bytes of a model path (`None` if missing).
pub fn export_library(
    items: &[CatalogItem],
    meta: &UserMeta,
    model_bytes: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<Vec<u8>, ArchiveError> {
    let mut catalog = Catalog::new(crate::user::USER_CATALOG_NAME);
    catalog.items = items.to_vec();
    let manifest = Manifest {
        format: FORMAT.into(),
        version: 1,
        catalog,
        meta: UserMeta {
            recent: Vec::new(),
            ..meta.clone()
        },
    };
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| ArchiveError(e.to_string()))?;
    let mut entries = vec![
        (MANIFEST.to_string(), json),
        ("README.txt".to_string(), README.as_bytes().to_vec()),
    ];
    let mut seen: Vec<&str> = Vec::new();
    for it in items {
        let Some(path) = it.model3d.as_deref() else {
            continue;
        };
        if seen.contains(&path) {
            continue;
        }
        seen.push(path);
        if !safe_model_path(path) {
            return err(format!("\"{}\" has an unsafe model path: {path}", it.name));
        }
        match model_bytes(path) {
            Some(b) => entries.push((path.to_string(), b)),
            None => return err(format!("The model file of \"{}\" is missing", it.name)),
        }
    }
    Ok(write_zip(&entries))
}

/// Reads an archive written by [`export_library`].
pub fn import_library(bytes: &[u8]) -> Result<LibraryArchive, ArchiveError> {
    let entries = read_zip(bytes)?;
    let manifest = entries.iter().find(|(n, _)| n == MANIFEST).ok_or_else(|| {
        ArchiveError(
            "This zip is not a Plan Studio library export (no plan-studio-library.json)".into(),
        )
    })?;
    let m: Manifest = serde_json::from_slice(&manifest.1)
        .map_err(|e| ArchiveError(format!("Damaged library file: {e}")))?;
    if m.format != FORMAT {
        return err("This is not a Plan Studio library export");
    }
    if m.version > 1 {
        return err("This export was made by a newer Plan Studio");
    }
    let mut models = Vec::new();
    for it in &m.catalog.items {
        if let Some(path) = &it.model3d {
            if !safe_model_path(path) {
                return err(format!("\"{}\" has an unsafe model path", it.name));
            }
            if models.iter().any(|(p, _): &(String, Vec<u8>)| p == path) {
                continue;
            }
            match entries.iter().find(|(n, _)| n == path) {
                Some((_, b)) => models.push((path.clone(), b.clone())),
                None => return err(format!("The model file of \"{}\" is missing", it.name)),
            }
        }
    }
    Ok(LibraryArchive {
        items: m.catalog.items,
        meta: m.meta,
        models,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ItemKind, Model3d, Placement, Symbol2d};

    #[test]
    fn crc_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn zip_round_trips_and_detects_damage() {
        let entries = vec![
            ("a.txt".to_string(), b"hello".to_vec()),
            ("dir/b.bin".to_string(), (0..=255u8).collect()),
            ("empty".to_string(), Vec::new()),
        ];
        let z = write_zip(&entries);
        assert_eq!(read_zip(&z).unwrap(), entries);
        let mut bad = z.clone();
        bad[40] ^= 0xFF; // inside the first entry's name or data
        let _ = read_zip(&bad);
        let mut bad = z.clone();
        let at = z.windows(5).position(|w| w == b"hello").unwrap();
        bad[at] = b'j';
        assert!(read_zip(&bad).unwrap_err().0.contains("checksum"));
        assert!(read_zip(b"nope").is_err());
        assert!(read_zip(b"SQLite format 3\0 and more bytes here....")
            .unwrap_err()
            .0
            .contains("Chief"));
        assert_eq!(read_zip(&write_zip(&[])).unwrap(), Vec::new());
    }

    fn sample_items() -> (Vec<CatalogItem>, Model3d) {
        let model = Model3d::box_model(30.0, 20.0, 40.0, Some([10, 200, 30]));
        let mut a = CatalogItem::new(
            "user.model.1",
            "Sofa",
            Placement::FreeStanding,
            Symbol2d::default(),
        )
        .with_category(&["User", "Furniture"])
        .with_size(30.0, 20.0, 40.0)
        .with_tags(&["couch"]);
        a.kind = ItemKind::Model;
        a.model3d = Some("user-models/user.model.1.psm".into());
        a.style = Some("modern".into());
        a.model_rotation = 90.0;
        let b = CatalogItem::new(
            "user.plain.1",
            "Plain",
            Placement::Ceiling,
            Symbol2d::default(),
        )
        .with_category(&["User"]);
        (vec![a, b], model)
    }

    #[test]
    fn an_exported_library_imports_identical() {
        let (items, model) = sample_items();
        let mut meta = UserMeta::default();
        meta.create_folder(&["User".into(), "Empty".into()])
            .unwrap();
        meta.toggle_favorite("user.model.1");
        meta.add_recent("user.plain.1");
        let bytes = model.to_bytes();
        let zip = export_library(&items, &meta, &|p| {
            (p == "user-models/user.model.1.psm").then(|| bytes.clone())
        })
        .unwrap();
        let back = import_library(&zip).unwrap();
        assert_eq!(back.items, items);
        assert_eq!(back.meta.folders, meta.folders);
        assert_eq!(back.meta.favorites, meta.favorites);
        assert!(back.meta.recent.is_empty());
        assert_eq!(back.models.len(), 1);
        assert_eq!(Model3d::from_bytes(&back.models[0].1).unwrap(), model);
        // The README says Chief cannot read it.
        let names: Vec<String> = read_zip(&zip).unwrap().into_iter().map(|e| e.0).collect();
        assert!(names.contains(&"README.txt".to_string()));
    }

    #[test]
    fn missing_models_and_foreign_zips_are_reported() {
        let (items, _) = sample_items();
        assert!(export_library(&items, &UserMeta::default(), &|_| None)
            .unwrap_err()
            .0
            .contains("missing"));
        let foreign = write_zip(&[("x.txt".into(), b"x".to_vec())]);
        assert!(import_library(&foreign)
            .unwrap_err()
            .0
            .contains("not a Plan Studio"));
        let mut evil = items[0].clone();
        evil.model3d = Some("../../etc/passwd".into());
        assert!(export_library(&[evil], &UserMeta::default(), &|_| Some(vec![])).is_err());
        assert!(safe_model_path("user-models/a.psm"));
        assert!(!safe_model_path("user-models/"));
        assert!(!safe_model_path("/abs/a.psm"));
    }
}
