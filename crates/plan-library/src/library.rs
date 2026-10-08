//! The in-memory index over one or more catalogs.

use crate::all_core_catalogs;
use crate::catalog::{Catalog, CatalogItem};
use crate::starter::core_catalog;

/// A node in the category tree shown by the Library Browser.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryNode {
    /// Category name (the root is named `"Library"`).
    pub name: String,
    /// Number of items in this node and all of its descendants.
    pub count: usize,
    /// Sub-categories, sorted by name.
    pub children: Vec<CategoryNode>,
    /// Ids of items filed directly in this category (not in a child).
    pub item_ids: Vec<String>,
}

impl CategoryNode {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            count: 0,
            children: Vec::new(),
            item_ids: Vec::new(),
        }
    }

    /// Looks up a direct child by exact name.
    pub fn child(&self, name: &str) -> Option<&CategoryNode> {
        self.children.iter().find(|c| c.name == name)
    }

    fn insert(&mut self, path: &[String], id: &str) {
        self.count += 1;
        match path.split_first() {
            None => self.item_ids.push(id.to_owned()),
            Some((head, rest)) => {
                let idx = match self.children.iter().position(|c| &c.name == head) {
                    Some(i) => i,
                    None => {
                        self.children.push(CategoryNode::new(head));
                        self.children.len() - 1
                    }
                };
                self.children[idx].insert(rest, id);
            }
        }
    }

    fn sort(&mut self) {
        self.children
            .sort_by_key(|c| (c.name.to_lowercase(), c.name.clone()));
        self.children.iter_mut().for_each(CategoryNode::sort);
    }
}

/// All loaded catalogs, searchable as one flat collection.
#[derive(Debug, Clone, Default)]
pub struct Library {
    catalogs: Vec<Catalog>,
}

impl Library {
    /// A library preloaded with the built-in [`core_catalog`].
    pub fn with_core() -> Self {
        let mut lib = Library::default();
        lib.add(core_catalog());
        lib
    }

    /// A library preloaded with every built-in catalog
    /// ([`all_core_catalogs`]).
    pub fn with_all_core() -> Self {
        Library {
            catalogs: all_core_catalogs(),
        }
    }

    /// Adds a catalog. Later catalogs never replace earlier ones; if two
    /// items share an id, [`Library::get`] returns the earlier.
    pub fn add(&mut self, catalog: Catalog) {
        self.catalogs.push(catalog);
    }

    /// The loaded catalogs, in the order they were added.
    pub fn catalogs(&self) -> &[Catalog] {
        &self.catalogs
    }

    /// Every item across all catalogs, in catalog order.
    pub fn all_items(&self) -> impl Iterator<Item = &CatalogItem> {
        self.catalogs.iter().flat_map(|c| c.items.iter())
    }

    /// Total number of items.
    pub fn len(&self) -> usize {
        self.catalogs.iter().map(|c| c.items.len()).sum()
    }

    /// True when no catalog contains any item.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Finds an item by exact id.
    pub fn get(&self, id: &str) -> Option<&CatalogItem> {
        self.all_items().find(|i| i.id == id)
    }

    /// Case-insensitive search over name, tags and category path.
    ///
    /// The query is split on whitespace and every term must match somewhere.
    /// Results are ranked best first: a name match beats a tag match, which
    /// beats a category match; whole-word and prefix matches beat substring
    /// matches. Ties sort by name. An empty query returns every item in
    /// catalog order.
    pub fn search(&self, query: &str) -> Vec<&CatalogItem> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if terms.is_empty() {
            return self.all_items().collect();
        }
        let mut scored: Vec<(u32, &CatalogItem)> = self
            .all_items()
            .filter_map(|item| score_item(item, &terms).map(|s| (s, item)))
            .collect();
        scored.sort_by(|(sa, a), (sb, b)| {
            sb.cmp(sa)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.id.cmp(&b.id))
        });
        scored.into_iter().map(|(_, item)| item).collect()
    }

    /// Builds the nested category tree with item counts. The returned root
    /// is a virtual node named `"Library"`; the top-level categories (such
    /// as `"Architectural"`) are its children.
    pub fn tree(&self) -> CategoryNode {
        let mut root = CategoryNode::new("Library");
        for item in self.all_items() {
            root.insert(&item.category, &item.id);
        }
        root.sort();
        root
    }
}

/// Score of one search term against one item (0 means no match).
fn term_score(item: &CatalogItem, term: &str) -> u32 {
    let name = item.name.to_lowercase();
    let mut best = 0;
    if name
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w == term)
    {
        best = 100;
    } else if name.starts_with(term) {
        best = 90;
    } else if name.contains(term) {
        best = 60;
    }
    for tag in &item.tags {
        let tag = tag.to_lowercase();
        if tag == term {
            best = best.max(40);
        } else if tag.contains(term) {
            best = best.max(25);
        }
    }
    for cat in &item.category {
        let cat = cat.to_lowercase();
        if cat == term {
            best = best.max(20);
        } else if cat.contains(term) {
            best = best.max(10);
        }
    }
    best
}

/// Sum of per-term scores, or `None` if any term fails to match.
fn score_item(item: &CatalogItem, terms: &[String]) -> Option<u32> {
    let mut total = 0;
    for t in terms {
        match term_score(item, t) {
            0 => return None,
            s => total += s,
        }
    }
    Some(total)
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

    fn sample() -> Library {
        let mut c = Catalog::new("T");
        c.items
            .push(item("t.rack", "Towel Rack", &["A", "Bath"], &["sink"]));
        c.items
            .push(item("t.sink", "Sink Unit", &["A", "Bath"], &[]));
        c.items
            .push(item("t.lamp", "Lamp", &["A", "Lighting"], &["bath"]));
        c.items.push(item("t.root", "Loose Thing", &[], &[]));
        let mut lib = Library::default();
        lib.add(c);
        lib
    }

    #[test]
    fn name_match_outranks_tag_match() {
        let lib = sample();
        let hits: Vec<&str> = lib.search("SINK").iter().map(|i| i.id.as_str()).collect();
        assert_eq!(hits, ["t.sink", "t.rack"]);
    }

    #[test]
    fn category_and_multi_term_search() {
        let lib = sample();
        assert_eq!(lib.search("bath").len(), 3);
        let hits = lib.search("bath lamp");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "t.lamp");
        assert!(lib.search("nonexistent").is_empty());
        assert_eq!(lib.search("  ").len(), 4);
    }

    #[test]
    fn get_and_len() {
        let lib = sample();
        assert_eq!(lib.len(), 4);
        assert!(!lib.is_empty());
        assert_eq!(lib.get("t.lamp").unwrap().name, "Lamp");
        assert!(lib.get("nope").is_none());
    }

    #[test]
    fn tree_counts_nest() {
        let tree = sample().tree();
        assert_eq!(tree.count, 4);
        assert_eq!(tree.item_ids, ["t.root"]);
        let a = tree.child("A").unwrap();
        assert_eq!(a.count, 3);
        let names: Vec<&str> = a.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Bath", "Lighting"]);
        assert_eq!(a.child("Bath").unwrap().count, 2);
        assert_eq!(a.child("Bath").unwrap().item_ids.len(), 2);
    }
}
