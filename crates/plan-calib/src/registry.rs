//! The Chief catalog registry (`Chief Library.json`) and resolution of its
//! UUIDs to files on disk.
//!
//! Chief lists every installed catalog in
//! `~/Library/Application Support/Chief Architect Premier X18/Chief Library.json`
//! by UUID. The files themselves live in three install folders; the UUID of
//! each `.calib` is `GlobalData.DatabaseUniqueId`. Resolving therefore opens
//! each candidate once. The result is cached in
//! `~/.plan-studio/chief-catalog-index.json`, keyed by path, size and mtime, so
//! later launches only `stat` the files.

use crate::catalog::ChiefCatalog;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Chief versions probed, newest first.
const VERSIONS: [&str; 2] = ["X18", "X17"];
/// Install sub-folders that hold catalogs.
const INSTALL_FOLDERS: [&str; 3] = [
    "Core Libraries",
    "Manufacturer Libraries",
    "Bonus Libraries",
];

/// Which family a catalog belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CatalogKind {
    /// Chief's own core libraries (registry category 1).
    Core,
    /// Manufacturer product catalogs (registry category 2).
    Manufacturer,
    /// Bonus catalogs (registry category 4).
    Bonus,
    /// The user's personal library (not in the registry).
    User,
    /// Soft-deleted or unknown-category registry entry.
    Deleted,
}

impl CatalogKind {
    /// Sort rank used to order catalogs in listings.
    pub(crate) fn rank(self) -> u8 {
        match self {
            CatalogKind::User => 0,
            CatalogKind::Core => 1,
            CatalogKind::Bonus => 2,
            CatalogKind::Manufacturer => 3,
            CatalogKind::Deleted => 4,
        }
    }
}

/// One registered catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    /// Catalog UUID (`GlobalData.DatabaseUniqueId`).
    pub uuid: String,
    /// Display name: the registry file name without `.calib`/`.calibz`.
    pub name: String,
    /// Catalog family.
    pub kind: CatalogKind,
    /// Resolved file, `None` when the catalog is registered but not installed.
    pub path: Option<PathBuf>,
}

#[derive(Deserialize)]
struct RawRegistry {
    #[serde(default)]
    catalogs: Vec<RawEntry>,
}

#[derive(Deserialize)]
struct RawEntry {
    #[serde(default)]
    category: i64,
    name: String,
    #[serde(default)]
    soft_deleted: bool,
    uuid: String,
}

/// The parsed registry with every UUID resolved to a path where possible.
#[derive(Debug, Clone, Default)]
pub struct ChiefRegistry {
    /// Entries in registry order.
    pub entries: Vec<RegistryEntry>,
    /// The JSON file they came from.
    pub registry_path: Option<PathBuf>,
}

/// Cached identification of one catalog file.
#[derive(Serialize, Deserialize, Clone)]
struct CachedFile {
    size: u64,
    mtime: u64,
    uuid: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct IndexCache {
    files: HashMap<String, CachedFile>,
}

impl ChiefRegistry {
    /// Loads the X18 registry (X17 as fallback) from the user's Library folder
    /// and resolves paths in the matching `/Library/Application Support`
    /// install, using `~/.plan-studio/chief-catalog-index.json` as cache.
    /// Returns an empty registry when no Chief installation is found.
    pub fn load_default() -> Result<ChiefRegistry> {
        let Some(home) = home_dir() else {
            return Ok(ChiefRegistry::default());
        };
        for v in VERSIONS {
            let dir = format!("Chief Architect Premier {v}");
            let json = home
                .join("Library/Application Support")
                .join(&dir)
                .join("Chief Library.json");
            if json.is_file() {
                let root = Path::new("/Library/Application Support").join(&dir);
                let cache = home.join(".plan-studio/chief-catalog-index.json");
                return Self::load(&json, &root, Some(&cache));
            }
        }
        Ok(ChiefRegistry::default())
    }

    /// Loads a registry file and resolves its entries against the install
    /// folders under `install_root` (`Core Libraries`, `Bonus Libraries`,
    /// `Manufacturer Libraries`). `cache` is read and rewritten when given.
    pub fn load(
        registry_json: &Path,
        install_root: &Path,
        cache: Option<&Path>,
    ) -> Result<ChiefRegistry> {
        let raw: RawRegistry = serde_json::from_slice(&std::fs::read(registry_json)?)?;
        let by_uuid = resolve_files(install_root, cache);

        let entries = raw
            .catalogs
            .into_iter()
            .map(|e| {
                let kind = if e.soft_deleted {
                    CatalogKind::Deleted
                } else {
                    match e.category {
                        1 => CatalogKind::Core,
                        2 => CatalogKind::Manufacturer,
                        4 => CatalogKind::Bonus,
                        _ => CatalogKind::Deleted,
                    }
                };
                let path = by_uuid
                    .by_uuid
                    .get(&e.uuid)
                    .or_else(|| by_uuid.by_name.get(&e.name.to_ascii_lowercase()))
                    .cloned();
                RegistryEntry {
                    uuid: e.uuid,
                    name: strip_ext(&e.name),
                    kind,
                    path,
                }
            })
            .collect();
        Ok(ChiefRegistry {
            entries,
            registry_path: Some(registry_json.to_owned()),
        })
    }
}

/// Result of scanning the install folders.
#[derive(Default)]
struct Resolved {
    by_uuid: HashMap<String, PathBuf>,
    /// Lower-case file name to path; the fallback for `.calibz` files (whose
    /// UUID is not cheaply readable) and unreadable catalogs.
    by_name: HashMap<String, PathBuf>,
}

fn resolve_files(install_root: &Path, cache_path: Option<&Path>) -> Resolved {
    let mut cache: IndexCache = cache_path
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut fresh = IndexCache::default();
    let mut out = Resolved::default();

    for folder in INSTALL_FOLDERS {
        let Ok(rd) = std::fs::read_dir(install_root.join(folder)) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        for path in files {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase());
            if !matches!(ext.as_deref(), Some("calib" | "calibz")) {
                continue;
            }
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            let key = path.to_string_lossy().into_owned();
            let entry = match cache.files.remove(&key) {
                Some(c) if c.size == meta.len() && c.mtime == mtime => c,
                _ => CachedFile {
                    size: meta.len(),
                    mtime,
                    // A .calibz holds its catalog compressed; skip the UUID.
                    uuid: if ext.as_deref() == Some("calib") {
                        ChiefCatalog::peek_id(&path).ok().flatten()
                    } else {
                        None
                    },
                },
            };
            if let Some(u) = &entry.uuid {
                out.by_uuid.insert(u.clone(), path.clone());
            }
            if let Some(n) = path.file_name() {
                out.by_name
                    .insert(n.to_string_lossy().to_ascii_lowercase(), path.clone());
            }
            fresh.files.insert(key, entry);
        }
    }

    if let Some(p) = cache_path {
        // The cache is an optimisation; failing to write it is not an error.
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(&fresh) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, bytes).is_ok() {
                let _ = std::fs::rename(&tmp, p);
            }
        }
    }
    out
}

fn strip_ext(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    for ext in [".calibz", ".calib"] {
        if lower.ends_with(ext) {
            return name[..name.len() - ext.len()].to_owned();
        }
    }
    name.to_owned()
}

/// The user's home directory from `$HOME`.
pub(crate) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, Fixture};

    #[test]
    fn strips_extensions() {
        assert_eq!(strip_ext("BonusTables.calib"), "BonusTables");
        assert_eq!(strip_ext("Pack.CALIBZ"), "Pack");
        assert_eq!(strip_ext("Plain"), "Plain");
    }

    #[test]
    fn resolves_by_uuid_name_and_caches() {
        let Some(fx) = Fixture::chief("registry-fixture") else {
            return;
        };
        let root = testutil::scratch_dir("registry-root");
        let core = root.join("Core Libraries");
        let bonus = root.join("Bonus Libraries");
        std::fs::create_dir_all(&core).unwrap();
        std::fs::create_dir_all(&bonus).unwrap();
        // Same catalog twice under different names: UUID match wins for the
        // first registry entry; the second entry has an unknown UUID and must
        // fall back to its file name. A stray text file is ignored.
        std::fs::copy(&fx.path, core.join("CoreFixture.calib")).unwrap();
        std::fs::copy(&fx.path, bonus.join("Other.calib")).unwrap();
        std::fs::write(core.join("catalog_hash.txt"), b"x").unwrap();

        let json = root.join("Chief Library.json");
        std::fs::write(
            &json,
            format!(
                r#"{{"version": 1, "catalogs": [
                  {{"category": 1, "name": "CoreFixture.calib", "soft_deleted": false, "uuid": "{}"}},
                  {{"category": 4, "name": "Other.calib", "soft_deleted": false, "uuid": "deadbeef-0000-0000-0000-000000000000"}},
                  {{"category": 2, "name": "Missing.calib", "soft_deleted": false, "uuid": "11111111-0000-0000-0000-000000000000"}},
                  {{"category": 4, "name": "Gone.calib", "soft_deleted": true, "uuid": "22222222-0000-0000-0000-000000000000"}}
                ]}}"#,
                testutil::ROOT_UUID
            ),
        )
        .unwrap();

        let cache = root.join("cache").join("index.json");
        let reg = ChiefRegistry::load(&json, &root, Some(&cache)).unwrap();
        assert_eq!(reg.entries.len(), 4);
        assert_eq!(reg.entries[0].kind, CatalogKind::Core);
        assert_eq!(reg.entries[0].name, "CoreFixture");
        assert!(
            reg.entries[0]
                .path
                .as_ref()
                .unwrap()
                .ends_with("CoreFixture.calib")
                || reg.entries[0]
                    .path
                    .as_ref()
                    .unwrap()
                    .ends_with("Other.calib")
        );
        assert!(reg.entries[1]
            .path
            .as_ref()
            .unwrap()
            .ends_with("Other.calib"));
        assert_eq!(reg.entries[1].kind, CatalogKind::Bonus);
        assert_eq!(reg.entries[2].path, None);
        assert_eq!(reg.entries[2].kind, CatalogKind::Manufacturer);
        assert_eq!(reg.entries[3].kind, CatalogKind::Deleted);

        // The cache was written and a reload reproduces the result.
        let cached: IndexCache = serde_json::from_slice(&std::fs::read(&cache).unwrap()).unwrap();
        assert_eq!(cached.files.len(), 2);
        assert!(cached
            .files
            .values()
            .all(|c| c.uuid.as_deref() == Some(testutil::ROOT_UUID)));
        let again = ChiefRegistry::load(&json, &root, Some(&cache)).unwrap();
        assert_eq!(again.entries, reg.entries);
    }
}
