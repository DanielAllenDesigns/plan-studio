//! plan-calib: read-only access to Chief Architect `.calib` / `.calibz`
//! library catalogs at runtime.
//!
//! Plan Studio never ships or copies Chief content. This crate opens the
//! catalogs a user has installed under their own licence and lets the app
//! browse them (names, category trees, keywords, thumbnails) with lazy I/O
//! that is safe on multi-gigabyte files. Everything is `std`-only.
//!
//! Layers, from the bottom:
//!
//! * [`sqlite`]: minimal read-only SQLite 3 reader (positioned reads).
//! * [`inflate`] and [`zip`]: RFC 1951 inflate and a zip reader for `.calibz`.
//! * [`catalog`]: [`ChiefCatalog`], the Chief-specific table semantics.
//! * [`registry`]: `Chief Library.json` plus UUID to file resolution.
//! * [`decode`]: decoders for the binary blobs (sizes, plan symbols, `CD AB` records).
//! * [`bridge`]: conversion to [`plan_library::Catalog`].
//! * this module: [`ChiefLibrary`], discovery and cross-catalog search.
//!
//! ```no_run
//! use plan_calib::ChiefLibrary;
//!
//! let lib = ChiefLibrary::discover();
//! println!("{} catalogs", lib.catalogs().len());
//! for hit in lib.search("shaker door", 10) {
//!     println!("{} / {}", hit.catalog_name, hit.object.name);
//! }
//! ```

pub mod bridge;
pub mod catalog;
pub mod decode;
pub mod error;
pub mod inflate;
pub mod registry;
pub mod sqlite;
pub mod zip;

mod source;
#[cfg(test)]
mod testutil;

pub use bridge::{to_plan_library, BridgeResult, BridgeStats};
pub use catalog::{CategoryNode, ChiefCatalog, ObjectSummary, Objects, Tag};
pub use error::{Error, Result};
pub use registry::{CatalogKind, ChiefRegistry, RegistryEntry};
pub use zip::{CalibZ, ZipArchive, ZipEntry};

use std::path::PathBuf;

/// Every catalog the user has: the registry's entries plus their personal
/// library.
#[derive(Debug, Clone, Default)]
pub struct ChiefLibrary {
    catalogs: Vec<RegistryEntry>,
}

/// One search result.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    /// Index into [`ChiefLibrary::catalogs`].
    pub catalog: usize,
    /// Display name of that catalog.
    pub catalog_name: String,
    /// The matching object.
    pub object: ObjectSummary,
}

/// Cheap counts over the catalog list (no catalog is opened).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LibraryStats {
    /// Entries in the list, including uninstalled and deleted ones.
    pub total: usize,
    /// Entries resolved to a file on disk.
    pub installed: usize,
    /// Installed core catalogs.
    pub core: usize,
    /// Installed bonus catalogs.
    pub bonus: usize,
    /// Installed manufacturer catalogs.
    pub manufacturer: usize,
    /// Installed user catalogs.
    pub user: usize,
    /// Registered but not installed (or soft-deleted) entries.
    pub missing: usize,
    /// Combined size of all installed catalog files in bytes.
    pub bytes: u64,
}

impl ChiefLibrary {
    /// Reads the registry (X18, X17 fallback) and the user library at
    /// `~/Documents/Chief Architect Premier X18 Data/Database Libraries/User_Library.calib`.
    /// Never fails: with no Chief installation the library is simply empty.
    pub fn discover() -> ChiefLibrary {
        let mut entries = ChiefRegistry::load_default()
            .map(|r| r.entries)
            .unwrap_or_default();
        if let Some(home) = registry::home_dir() {
            let user = home.join(
                "Documents/Chief Architect Premier X18 Data/Database Libraries/User_Library.calib",
            );
            if user.is_file() {
                let uuid = ChiefCatalog::peek_id(&user)
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                entries.push(RegistryEntry {
                    uuid,
                    name: "User Library".into(),
                    kind: CatalogKind::User,
                    path: Some(user),
                });
            }
        }
        Self::from_entries(entries)
    }

    /// Builds a library from explicit entries (sorted: user, core, bonus,
    /// manufacturer, deleted; then by name).
    pub fn from_entries(mut catalogs: Vec<RegistryEntry>) -> ChiefLibrary {
        catalogs.sort_by(|a, b| {
            (a.kind.rank(), a.name.to_lowercase()).cmp(&(b.kind.rank(), b.name.to_lowercase()))
        });
        ChiefLibrary { catalogs }
    }

    /// All known catalogs in listing order.
    pub fn catalogs(&self) -> &[RegistryEntry] {
        &self.catalogs
    }

    /// Opens catalog number `idx` of [`catalogs`](Self::catalogs). Fails with
    /// `NotFound` when the entry is not installed.
    pub fn open(&self, idx: usize) -> Result<ChiefCatalog> {
        let e = self
            .catalogs
            .get(idx)
            .ok_or_else(|| Error::NotFound(format!("catalog index {idx}")))?;
        let path = e
            .path
            .as_ref()
            .ok_or_else(|| Error::NotFound(format!("catalog '{}' is not installed", e.name)))?;
        Ok(ChiefCatalog::open(path)?.with_name(e.name.clone()))
    }

    /// Case-insensitive search over object names and keywords, streaming
    /// through the installed catalogs in listing order and stopping after
    /// `max` hits. Every whitespace-separated term of `query` must appear in
    /// the name or in one of the keywords. Catalogs that fail to open or read
    /// are skipped.
    pub fn search(&self, query: &str, max: usize) -> Vec<SearchHit> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut hits = Vec::new();
        if terms.is_empty() || max == 0 {
            return hits;
        }
        for (i, e) in self.catalogs.iter().enumerate() {
            if e.kind == CatalogKind::Deleted || e.path.is_none() {
                continue;
            }
            let Ok(cat) = self.open(i) else { continue };
            let Ok(objects) = cat.objects() else { continue };
            for obj in objects {
                let name = obj.name.to_lowercase();
                let found = terms.iter().all(|t| {
                    name.contains(t) || obj.keywords.iter().any(|k| k.to_lowercase().contains(t))
                });
                if found {
                    hits.push(SearchHit {
                        catalog: i,
                        catalog_name: e.name.clone(),
                        object: obj,
                    });
                    if hits.len() >= max {
                        return hits;
                    }
                }
            }
        }
        hits
    }

    /// Counts by kind and total installed size; opens nothing.
    pub fn stats(&self) -> LibraryStats {
        let mut s = LibraryStats {
            total: self.catalogs.len(),
            ..Default::default()
        };
        for e in &self.catalogs {
            let installed = e.path.is_some() && e.kind != CatalogKind::Deleted;
            if !installed {
                s.missing += 1;
                continue;
            }
            s.installed += 1;
            match e.kind {
                CatalogKind::Core => s.core += 1,
                CatalogKind::Bonus => s.bonus += 1,
                CatalogKind::Manufacturer => s.manufacturer += 1,
                CatalogKind::User => s.user += 1,
                CatalogKind::Deleted => {}
            }
            if let Some(p) = &e.path {
                s.bytes += std::fs::metadata(p).map_or(0, |m| m.len());
            }
        }
        s
    }

    /// Path of catalog `idx`, if installed.
    pub fn path(&self, idx: usize) -> Option<PathBuf> {
        self.catalogs.get(idx)?.path.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Fixture;

    fn entry(name: &str, kind: CatalogKind, path: Option<PathBuf>) -> RegistryEntry {
        RegistryEntry {
            uuid: format!("uuid-{name}"),
            name: name.into(),
            kind,
            path,
        }
    }

    #[test]
    fn search_stats_and_open() {
        let Some(fx) = Fixture::chief("lib-fixture") else {
            return;
        };
        let lib = ChiefLibrary::from_entries(vec![
            entry("Zed", CatalogKind::Manufacturer, Some(fx.path.clone())),
            entry("Ghost", CatalogKind::Bonus, None),
            entry("Old", CatalogKind::Deleted, Some(fx.path.clone())),
            entry("Mine", CatalogKind::User, Some(fx.path.clone())),
            entry("Broken", CatalogKind::Core, Some(fx.dir.join("nope.calib"))),
        ]);
        let names: Vec<&str> = lib.catalogs().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Mine", "Broken", "Ghost", "Zed", "Old"]);

        let st = lib.stats();
        assert_eq!((st.total, st.installed, st.missing), (5, 3, 2));
        assert_eq!((st.user, st.core, st.manufacturer, st.bonus), (1, 1, 1, 0));
        assert!(st.bytes > 0);

        // name match, keyword match, AND of terms, case-insensitive
        let hits = lib.search("DOOR one", 10);
        assert_eq!(hits.len(), 2, "Mine and Zed; broken/ghost/deleted skipped");
        assert_eq!(hits[0].catalog_name, "Mine");
        assert_eq!(hits[0].object.name, "Door One");
        let kw = lib.search("oak", 10);
        assert_eq!(kw.len(), 2);
        assert_eq!(kw[0].object.library_object_id, 1);
        let front = lib.search("front", 100);
        assert_eq!(front.len(), 4, "objects 1 and 2 in two catalogs");

        // early exit
        assert_eq!(lib.search("front", 1).len(), 1);
        assert!(lib.search("", 5).is_empty());
        assert!(lib.search("front", 0).is_empty());
        assert!(lib.search("nonexistent", 5).is_empty());

        let cat = lib.open(0).unwrap();
        assert_eq!(cat.name(), "Mine");
        assert!(matches!(lib.open(2), Err(Error::NotFound(_))));
        assert!(matches!(lib.open(99), Err(Error::NotFound(_))));
    }
}
