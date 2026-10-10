//! Linking the library objects of an imported Chief plan to the installed
//! Chief catalogs.
//!
//! A placed object in a Chief `.plan` carries the `UniqueId` of its library
//! item (`plan_chiefplan::import::SymbolQuery::unique_id`; see
//! `docs/chief-plan-format.md`, section 4.10). [`resolve_symbol`] looks it up
//! in every installed catalog (Core, Bonus, Manufacturer, the user's own) and
//! answers the Plan Studio catalog id `chief.<catalog-uuid>.<object id>`, so the
//! imported symbol is the real item (name, 2D symbol, 3D meshes) instead of a
//! `chief-plan.<name>` box. Items of the 2010-era libraries (about half of the
//! placed objects of a typical plan) have a GUID no installed catalog holds;
//! those are found by display name, preferring the user's, Core, Bonus and
//! Manufacturer catalogs in that order and, among equal names, the item whose
//! keywords share the most tags with the placed copy.
//!
//! The index is built from the catalogs in place (nothing is copied) the first
//! time an import asks, on the importing thread, and again when the library
//! changed (another install folder, a new scan): one pass over every
//! catalog's `LibraryObjects.UniqueId` column for the GUIDs, and, only when an
//! object was not found by GUID, one pass over the object names.

use crate::tools::library::chief;
use plan_calib::{CatalogKind, ChiefCatalog, ChiefLibrary};
use plan_chiefplan::import::SymbolQuery;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// One catalog item known by display name.
#[derive(Debug, Clone, PartialEq)]
struct NameHit {
    /// `chief.<catalog-uuid>.<object id>`.
    id: String,
    /// 0 user, 1 core, 2 bonus, 3 manufacturer.
    rank: u8,
    /// Lower-case keywords and category names of the item.
    words: Vec<String>,
}

/// UniqueId and name lookups over the installed catalogs.
#[derive(Debug, Default)]
pub struct Index {
    /// `UniqueId` (lower-case `8-4-4-4-12`) -> catalog id.
    by_guid: HashMap<String, String>,
    /// Normalised display name -> items.
    by_name: HashMap<String, Vec<NameHit>>,
    /// Whether [`by_name`](Self::by_name) has been filled.
    names_built: bool,
}

/// Lower-case letters and digits only, so `Under-Mount` equals `Undermount`.
fn normalize(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn rank_of(kind: CatalogKind) -> u8 {
    match kind {
        CatalogKind::User => 0,
        CatalogKind::Core => 1,
        CatalogKind::Bonus => 2,
        CatalogKind::Manufacturer | CatalogKind::Deleted => 3,
    }
}

impl Index {
    /// An index over `lib`'s GUIDs (the names are read on demand).
    pub fn build(lib: &ChiefLibrary) -> Index {
        let mut ix = Index::default();
        for (i, e) in lib.catalogs().iter().enumerate() {
            if e.kind == CatalogKind::Deleted || e.path.is_none() {
                continue;
            }
            let Ok(cat) = lib.open(i) else { continue };
            ix.add_guids(&cat);
        }
        ix
    }

    /// Adds every `LibraryObjects.UniqueId` of `cat`; the first catalog (in
    /// library order) wins a duplicate.
    fn add_guids(&mut self, cat: &ChiefCatalog) {
        let db = cat.db();
        let t = "LibraryObjects";
        let (Some(id_col), Some(uid_col)) = (
            db.column_index(t, "LibraryObjectId"),
            db.column_index(t, "UniqueId"),
        ) else {
            return;
        };
        let Ok(rows) = db.table(t) else { return };
        let need = id_col.max(uid_col) + 1;
        for row in rows.with_blobs(false).with_max_columns(need) {
            let Ok(row) = row else { break };
            let (Some(id), Some(uid)) = (row[id_col].as_i64(), row[uid_col].as_str()) else {
                continue;
            };
            self.by_guid
                .entry(uid.to_ascii_lowercase())
                .or_insert_with(|| format!("{}{}.{id}", chief::ID_PREFIX, cat.id()));
        }
    }

    /// Reads the display names of every catalog of `lib` (the slow pass).
    pub fn build_names(&mut self, lib: &ChiefLibrary) {
        if self.names_built {
            return;
        }
        self.names_built = true;
        for (i, e) in lib.catalogs().iter().enumerate() {
            if e.kind == CatalogKind::Deleted || e.path.is_none() {
                continue;
            }
            let Ok(cat) = lib.open(i) else { continue };
            let Ok(objects) = cat.objects() else { continue };
            let rank = rank_of(e.kind);
            for o in objects {
                let key = normalize(&o.name);
                if key.is_empty() {
                    continue;
                }
                let words = o
                    .keywords
                    .iter()
                    .chain(o.category_path.iter())
                    .map(|w| w.to_lowercase())
                    .collect();
                self.by_name.entry(key).or_default().push(NameHit {
                    id: format!("{}{}.{}", chief::ID_PREFIX, cat.id(), o.library_object_id),
                    rank,
                    words,
                });
            }
        }
    }

    /// The catalog id for the GUID alone, trying the anchored GUID and then
    /// the candidates.
    pub fn by_guid(&self, q: &SymbolQuery) -> Option<&str> {
        q.unique_id
            .iter()
            .chain(q.candidates.iter())
            .find_map(|g| self.by_guid.get(&g.to_ascii_lowercase()))
            .map(String::as_str)
    }

    /// The catalog id for the display name: among equal names the item whose
    /// words share the most tags with the query, then the best-ranked catalog.
    pub fn by_name(&self, q: &SymbolQuery) -> Option<&str> {
        let hits = self.by_name.get(&normalize(&q.name))?;
        let tags: Vec<String> = q.tags.iter().map(|t| t.to_lowercase()).collect();
        hits.iter()
            .max_by_key(|h| {
                let shared = tags.iter().filter(|t| h.words.contains(t)).count();
                (shared, std::cmp::Reverse(h.rank))
            })
            .map(|h| h.id.as_str())
    }

    /// Whether the name pass has run.
    pub fn has_names(&self) -> bool {
        self.names_built
    }

    /// Number of GUIDs indexed.
    #[cfg(test)]
    pub fn guid_count(&self) -> usize {
        self.by_guid.len()
    }
}

type Shared = (Arc<ChiefLibrary>, Rc<RefCell<Index>>);

thread_local! {
    /// The index with the library it was built from.
    static INDEX: RefCell<Option<Shared>> = const { RefCell::new(None) };
}

/// Drops the shared index so the next import builds it again.
#[allow(dead_code)] // `index()` also notices a changed library by itself
pub fn forget() {
    INDEX.with(|c| *c.borrow_mut() = None);
}

/// The shared index, built from the discovered library on first use and again
/// when the library changed (Preferences > Folders, a new scan); `None` when
/// the Chief catalogs are off or not installed.
fn index() -> Option<Rc<RefCell<Index>>> {
    if !chief::enabled() {
        return None;
    }
    let lib = chief::library()?;
    index_for(lib, Index::build)
}

/// [`index`] for a given library and builder (the builder is a parameter so
/// a test can count the builds).
fn index_for(
    lib: Arc<ChiefLibrary>,
    build: impl FnOnce(&ChiefLibrary) -> Index,
) -> Option<Rc<RefCell<Index>>> {
    if let Some((built_from, ix)) = INDEX.with(|c| c.borrow().clone()) {
        if Arc::ptr_eq(&built_from, &lib) {
            return Some(ix);
        }
    }
    let ix = Rc::new(RefCell::new(build(&lib)));
    INDEX.with(|c| *c.borrow_mut() = Some((lib, ix.clone())));
    Some(ix)
}

/// `plan_chiefplan::import::ImportOptions::symbol_resolver` for the app: the
/// catalog id of a placed Chief library object. Chief's installed catalogs
/// are searched first (by GUID, then by name); a symbol they do not hold is
/// matched against Plan Studio's own library (the built-in catalogs and the
/// User Catalog: `tools::library::resolve_symbol_query`).
pub fn resolve_symbol(q: &SymbolQuery) -> Option<String> {
    resolve_in_chief(q).or_else(|| {
        crate::tools::library::resolve_symbol_query(
            &q.name,
            &q.tags,
            q.unique_id.as_deref(),
            &q.candidates,
        )
    })
}

/// The Chief catalogs' answer alone.
fn resolve_in_chief(q: &SymbolQuery) -> Option<String> {
    let ix = index()?;
    if let Some(id) = ix.borrow().by_guid(q) {
        return Some(id.to_owned());
    }
    if !ix.borrow().has_names() {
        let lib = chief::library()?;
        ix.borrow_mut().build_names(&lib);
    }
    let hit = ix.borrow().by_name(q).map(str::to_owned);
    hit
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(name: &str, tags: &[&str], guid: Option<&str>) -> SymbolQuery {
        SymbolQuery {
            name: name.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            unique_id: guid.map(str::to_owned),
            candidates: Vec::new(),
        }
    }

    fn hit(id: &str, rank: u8, words: &[&str]) -> NameHit {
        NameHit {
            id: id.into(),
            rank,
            words: words.iter().map(|w| w.to_string()).collect(),
        }
    }

    #[test]
    fn the_index_is_rebuilt_only_when_the_library_changes() {
        forget();
        let a = Arc::new(ChiefLibrary::default());
        let mut builds = 0;
        let first = index_for(a.clone(), |l| {
            builds += 1;
            Index::build(l)
        })
        .unwrap();
        let again = index_for(a.clone(), |_| panic!("same library: no rebuild")).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        let b = Arc::new(ChiefLibrary::default());
        let other = index_for(b, |l| {
            builds += 1;
            Index::build(l)
        })
        .unwrap();
        assert!(
            !Rc::ptr_eq(&first, &other),
            "a new library builds a new index"
        );
        assert_eq!(builds, 2);
        forget();
    }

    #[test]
    fn names_ignore_punctuation_and_case() {
        assert_eq!(normalize("Under-Mount Sink"), normalize("undermount sink"));
        assert_eq!(normalize("K-2355  Archer"), "k2355archer");
        assert_eq!(normalize("***"), "");
    }

    #[test]
    fn the_guid_wins_and_candidates_are_tried() {
        let mut ix = Index::default();
        ix.by_guid.insert(
            "4c528223-d712-45e5-b7a3-27f75f3e88c1".into(),
            "chief.a.1".into(),
        );
        ix.by_guid.insert(
            "dcc1f4d7-c11e-4042-bcc8-93db33af85f2".into(),
            "chief.b.2".into(),
        );
        let q = query(
            "Elongated Toilet",
            &[],
            Some("4C528223-D712-45E5-B7A3-27F75F3E88C1"),
        );
        assert_eq!(ix.by_guid(&q), Some("chief.a.1"));
        let mut q = query("x", &[], None);
        q.candidates = vec![
            "00000000-0000-4000-8000-000000000000".into(),
            "dcc1f4d7-c11e-4042-bcc8-93db33af85f2".into(),
        ];
        assert_eq!(ix.by_guid(&q), Some("chief.b.2"));
        assert_eq!(ix.by_guid(&query("y", &[], Some("nope"))), None);
    }

    #[test]
    fn equal_names_prefer_shared_tags_then_the_better_catalog() {
        let mut ix = Index {
            names_built: true,
            ..Index::default()
        };
        ix.by_name.insert(
            normalize("Coffee Table"),
            vec![
                hit("chief.bonus.3", 2, &["craftsman", "tables"]),
                hit("chief.core.9", 1, &["tables"]),
                hit("chief.mfr.4", 3, &["craftsman", "tables", "living room"]),
            ],
        );
        // Two tags shared: the manufacturer's item beats the better-ranked ones.
        let q = query("Coffee Table", &["Living Room", "Tables"], None);
        assert_eq!(ix.by_name(&q), Some("chief.mfr.4"));
        // No tags: the best-ranked catalog (Core).
        assert_eq!(
            ix.by_name(&query("coffee-table", &[], None)),
            Some("chief.core.9")
        );
        assert_eq!(ix.by_name(&query("Unknown", &[], None)), None);
    }

    /// Against Daniel's installed catalogs (ignored: needs Chief). Measures
    /// the two passes and checks the toilet of his plans resolves.
    #[test]
    #[ignore = "needs Daniel's Chief catalogs"]
    fn installed_catalogs_resolve_guids_and_names() {
        if !chief::install_folder_exists(None) {
            eprintln!("no Chief install: skipped");
            return;
        }
        chief::configure(&chief::ChiefSettings {
            enabled: true,
            folder: None,
        });
        let Some(lib) = chief::library() else {
            eprintln!("no catalogs: skipped");
            return;
        };
        let t = std::time::Instant::now();
        let mut ix = Index::build(&lib);
        eprintln!(
            "{} GUIDs in {:.1}s",
            ix.guid_count(),
            t.elapsed().as_secs_f64()
        );
        assert!(ix.guid_count() > 10_000);
        // Beveled Mirror (vert), CoreInteriors: a GUID read from Daniel's plans.
        let q = query(
            "Beveled Mirror (vert)",
            &[],
            Some("dcc1f4d7-c11e-4042-bcc8-93db33af85f2"),
        );
        assert!(ix.by_guid(&q).is_some_and(|id| id.starts_with("chief.")));
        let t = std::time::Instant::now();
        ix.build_names(&lib);
        eprintln!("names in {:.1}s", t.elapsed().as_secs_f64());
        // A 2010-era item whose GUID no catalog holds is found by name.
        let q = query(
            "Elongated Toilet",
            &["ADA", "Universal Design"],
            Some("4c528223-d712-45e5-b7a3-27f75f3e88c1"),
        );
        assert!(ix.by_guid(&q).is_none());
        assert!(ix.by_name(&q).is_some());
    }
}
