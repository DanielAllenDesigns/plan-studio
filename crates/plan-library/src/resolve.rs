//! Finding a catalog item for a library object that names one only by its
//! catalog GUID and name: the lookup behind the Chief `.plan` importer's
//! `symbol_resolver`.
//!
//! A Chief plan stores no link to the catalog item a symbol came from, only
//! the item's name, its category tags and (for the X13 to X18 catalogs) its
//! catalog `UniqueId`. [`ItemIndex`] maps both onto the items Plan Studio has
//! itself, the built-in catalogs and the User Catalog:
//!
//! * a GUID matches an item that carries the tag `guid:<uuid>` ([`guid_tag`]),
//!   which is how an item made from a Chief object remembers where it came from;
//! * a name matches when it is equal to an item's name once case and
//!   punctuation are ignored ([`normalize`]). Several items with the same name
//!   are told apart by the tags they share with the plan's, then User Catalog
//!   items before built-in ones.
//!
//! Chief's own installed catalogs are searched by the app first (they hold the
//! real item); this index is the fallback that keeps a plan's chairs and
//! toilets linked when the install is absent.

use crate::catalog::CatalogItem;
use std::collections::HashMap;

/// Prefix of the tag that stores a Chief catalog `UniqueId` on an item.
pub const GUID_PREFIX: &str = "guid:";

/// The tag for the catalog GUID `guid` (any case; stored lower case).
pub fn guid_tag(guid: &str) -> String {
    format!("{GUID_PREFIX}{}", guid.trim().to_lowercase())
}

/// A name reduced to lower-case letters and digits: `Under-Mount Sink` and
/// `undermount  sink` agree.
pub fn normalize(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Hit {
    id: String,
    /// 0 for User Catalog items, 1 for the rest.
    rank: u8,
    /// Lower-case tags and category names.
    words: Vec<String>,
}

/// GUID and name lookups over a set of catalog items.
#[derive(Debug, Clone, Default)]
pub struct ItemIndex {
    by_guid: HashMap<String, String>,
    by_name: HashMap<String, Vec<Hit>>,
}

impl ItemIndex {
    /// Indexes `items` (all of them: built-in and user).
    pub fn build<'a>(items: impl IntoIterator<Item = &'a CatalogItem>) -> ItemIndex {
        let mut ix = ItemIndex::default();
        for item in items {
            for t in &item.tags {
                if let Some(g) = t.strip_prefix(GUID_PREFIX) {
                    ix.by_guid
                        .entry(g.trim().to_lowercase())
                        .or_insert_with(|| item.id.clone());
                }
            }
            let key = normalize(&item.name);
            if key.is_empty() {
                continue;
            }
            let words = item
                .tags
                .iter()
                .chain(item.category.iter())
                .map(|w| w.to_lowercase())
                .collect();
            ix.by_name.entry(key).or_default().push(Hit {
                id: item.id.clone(),
                rank: u8::from(!item.id.starts_with("user.")),
                words,
            });
        }
        ix
    }

    /// Number of GUIDs indexed.
    pub fn guid_count(&self) -> usize {
        self.by_guid.len()
    }

    /// Number of distinct names indexed.
    pub fn name_count(&self) -> usize {
        self.by_name.len()
    }

    /// The item with catalog GUID `guid`.
    pub fn by_guid(&self, guid: &str) -> Option<&str> {
        self.by_guid
            .get(&guid.trim().to_lowercase())
            .map(String::as_str)
    }

    /// The item named `name`; among equal names the one sharing most of
    /// `tags`, then a User Catalog item, then the smallest id.
    pub fn by_name(&self, name: &str, tags: &[String]) -> Option<&str> {
        let hits = self.by_name.get(&normalize(name))?;
        let want: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();
        hits.iter()
            .min_by_key(|h| {
                let shared = want.iter().filter(|t| h.words.contains(t)).count();
                (std::cmp::Reverse(shared), h.rank, h.id.as_str())
            })
            .map(|h| h.id.as_str())
    }

    /// The full lookup of the importer: `guid` and then each of
    /// `candidates` by GUID, then `name` with `tags`.
    pub fn resolve(
        &self,
        name: &str,
        tags: &[String],
        guid: Option<&str>,
        candidates: &[String],
    ) -> Option<&str> {
        guid.into_iter()
            .chain(candidates.iter().map(String::as_str))
            .find_map(|g| self.by_guid(g))
            .or_else(|| self.by_name(name, tags))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Placement;
    use crate::symbol::Symbol2d;

    fn item(id: &str, name: &str, cat: &[&str], tags: &[&str]) -> CatalogItem {
        CatalogItem::new(id, name, Placement::FreeStanding, Symbol2d::default())
            .with_category(cat)
            .with_tags(tags)
    }

    #[test]
    fn names_ignore_case_and_punctuation() {
        assert_eq!(normalize("Under-Mount Sink"), normalize("undermount  SINK"));
        assert_eq!(normalize("K-2355 Archer"), "k2355archer");
        assert_eq!(normalize("***"), "");
    }

    #[test]
    fn a_guid_tag_finds_its_item_first() {
        let g = "4C528223-D712-45E5-B7A3-27F75F3E88C1";
        let tag = guid_tag(g);
        let items = vec![
            item("core.a", "Elongated Toilet", &["Plumbing"], &[]),
            item("user.7", "My Toilet", &["User"], &[&tag]),
        ];
        let ix = ItemIndex::build(&items);
        assert_eq!(ix.guid_count(), 1);
        assert_eq!(ix.by_guid(&g.to_lowercase()), Some("user.7"));
        assert_eq!(
            ix.resolve("Elongated Toilet", &[], Some(g), &[]),
            Some("user.7")
        );
        // A candidate GUID is tried too; with none matching the name decides.
        let other = vec![
            "00000000-0000-4000-8000-000000000000".to_string(),
            g.to_string(),
        ];
        assert_eq!(ix.resolve("zzz", &[], None, &other), Some("user.7"));
        assert_eq!(
            ix.resolve("Elongated Toilet", &[], Some("nope"), &[]),
            Some("core.a")
        );
        assert_eq!(ix.resolve("Unknown", &[], None, &[]), None);
    }

    #[test]
    fn equal_names_prefer_shared_tags_then_user_items() {
        let items = vec![
            item("core.z", "Coffee Table", &["Furniture", "Tables"], &[]),
            item("core.a", "Coffee Table", &["Furniture"], &["craftsman"]),
            item("user.3", "Coffee Table", &["User"], &[]),
        ];
        let ix = ItemIndex::build(&items);
        assert_eq!(ix.name_count(), 1);
        let tags = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            ix.by_name("coffee-table", &tags(&["Tables"])),
            Some("core.z")
        );
        assert_eq!(
            ix.by_name("Coffee Table", &tags(&["Craftsman", "Furniture"])),
            Some("core.a")
        );
        // No tags: the user's own item wins.
        assert_eq!(ix.by_name("Coffee Table", &[]), Some("user.3"));
    }

    #[test]
    fn the_built_in_catalogs_resolve_their_own_names() {
        let lib = crate::Library::with_all_core();
        let ix = ItemIndex::build(lib.all_items());
        let toilet = lib.get("core.plumbing.toilet_elongated").unwrap();
        assert_eq!(
            ix.by_name(&toilet.name, &[])
                .and_then(|id| lib.get(id))
                .map(|i| &i.name),
            Some(&toilet.name)
        );
    }
}
