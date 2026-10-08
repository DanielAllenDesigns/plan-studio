//! Library Browser filters and sorting: search text plus type, catalog,
//! style keyword, size range, favorites and category, then a sort order.

use crate::catalog::{CatalogItem, ItemKind};
use crate::library::Library;
use crate::manage::UserMeta;

/// How results are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    /// Best search match first (name order without a query).
    #[default]
    Relevance,
    /// Alphabetical by name.
    Name,
    /// By type, then name.
    Type,
    /// Smallest footprint first.
    Size,
    /// Most recently used first (unused items last, by name).
    Recent,
}

impl SortKey {
    /// Every key, in menu order.
    pub const ALL: [SortKey; 5] = [
        SortKey::Relevance,
        SortKey::Name,
        SortKey::Type,
        SortKey::Size,
        SortKey::Recent,
    ];

    /// Display name.
    pub fn label(self) -> &'static str {
        match self {
            SortKey::Relevance => "Relevance",
            SortKey::Name => "Name",
            SortKey::Type => "Type",
            SortKey::Size => "Size",
            SortKey::Recent => "Recently used",
        }
    }
}

/// Everything the browser can narrow the list by. An empty filter keeps
/// everything.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Filter {
    /// Search text (all words must match; see [`Library::search`]).
    pub query: String,
    /// Keep only these types; empty keeps all.
    pub kinds: Vec<ItemKind>,
    /// Keep only items of the catalog with this name.
    pub catalog: Option<String>,
    /// Keep items whose style, manufacturer, tags or name contain this word.
    pub style: String,
    /// Smallest footprint (larger of width and depth), inches.
    pub min_size: Option<f64>,
    /// Largest footprint, inches.
    pub max_size: Option<f64>,
    /// Keep only favorites.
    pub favorites_only: bool,
    /// Keep only items filed under this category path.
    pub category: Vec<String>,
    /// Ordering of the result.
    pub sort: SortKey,
}

/// The size the size range compares: the larger of width and depth.
pub fn footprint_size(item: &CatalogItem) -> f64 {
    item.width.max(item.depth)
}

impl Filter {
    /// True when nothing narrows the list (a query, type, catalog, style,
    /// size, favorites or category).
    pub fn is_empty(&self) -> bool {
        self.query.trim().is_empty()
            && self.kinds.is_empty()
            && self.catalog.is_none()
            && self.style.trim().is_empty()
            && self.min_size.is_none()
            && self.max_size.is_none()
            && !self.favorites_only
            && self.category.is_empty()
    }

    fn keeps(&self, item: &CatalogItem, catalog_name: &str, meta: &UserMeta) -> bool {
        if !self.kinds.is_empty() && !self.kinds.contains(&item.kind) {
            return false;
        }
        if let Some(c) = &self.catalog {
            if c != catalog_name {
                return false;
            }
        }
        let style = self.style.trim().to_lowercase();
        if !style.is_empty() {
            let hit = item
                .style
                .as_ref()
                .is_some_and(|s| s.to_lowercase().contains(&style))
                || item
                    .manufacturer
                    .as_ref()
                    .is_some_and(|m| m.to_lowercase().contains(&style))
                || item.tags.iter().any(|t| t.to_lowercase().contains(&style))
                || item.name.to_lowercase().contains(&style);
            if !hit {
                return false;
            }
        }
        let size = footprint_size(item);
        if self.min_size.is_some_and(|m| size < m) || self.max_size.is_some_and(|m| size > m) {
            return false;
        }
        if self.favorites_only && !meta.is_favorite(&item.id) {
            return false;
        }
        item.category.starts_with(&self.category)
    }
}

/// The items of `lib` that pass `filter`, in the filter's sort order.
/// `meta` supplies favorites and recents.
pub fn apply<'a>(lib: &'a Library, filter: &Filter, meta: &UserMeta) -> Vec<&'a CatalogItem> {
    let query = filter.query.trim();
    let ranked = lib.search(query);
    // The catalog of each item, by id (the first catalog wins, as in
    // `Library::get`).
    let mut catalog_of: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for c in lib.catalogs() {
        for i in &c.items {
            catalog_of.entry(i.id.as_str()).or_insert(c.name.as_str());
        }
    }
    let mut out: Vec<&CatalogItem> = ranked
        .into_iter()
        .filter(|i| {
            let cat = catalog_of.get(i.id.as_str()).copied().unwrap_or("");
            filter.keeps(i, cat, meta)
        })
        .collect();
    let by_name = |a: &&CatalogItem, b: &&CatalogItem| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    };
    match filter.sort {
        SortKey::Relevance => {
            if query.is_empty() {
                out.sort_by(by_name);
            }
        }
        SortKey::Name => out.sort_by(by_name),
        SortKey::Type => out.sort_by(|a, b| {
            (a.kind.label(), a.name.to_lowercase()).cmp(&(b.kind.label(), b.name.to_lowercase()))
        }),
        SortKey::Size => out.sort_by(|a, b| {
            footprint_size(a)
                .total_cmp(&footprint_size(b))
                .then_with(|| by_name(a, b))
        }),
        SortKey::Recent => out.sort_by(|a, b| {
            let rank = |i: &CatalogItem| {
                meta.recent
                    .iter()
                    .position(|r| *r == i.id)
                    .unwrap_or(usize::MAX)
            };
            rank(a).cmp(&rank(b)).then_with(|| by_name(a, b))
        }),
    }
    out
}

/// The favorites that exist in `lib`, in the order they were starred.
pub fn favorites<'a>(lib: &'a Library, meta: &UserMeta) -> Vec<&'a CatalogItem> {
    meta.favorites.iter().filter_map(|id| lib.get(id)).collect()
}

/// The recently used items that exist in `lib`, most recent first.
pub fn recent<'a>(lib: &'a Library, meta: &UserMeta) -> Vec<&'a CatalogItem> {
    meta.recent.iter().filter_map(|id| lib.get(id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Catalog, Placement, Symbol2d};

    fn item(id: &str, name: &str, kind: ItemKind, w: f64, d: f64, tags: &[&str]) -> CatalogItem {
        let mut i = CatalogItem::new(id, name, Placement::FreeStanding, Symbol2d::default())
            .with_category(&["User", "Stuff"])
            .with_size(w, d, 30.0)
            .with_tags(tags);
        i.kind = kind;
        i
    }

    fn lib() -> Library {
        let mut mine = Catalog::new("User Library");
        mine.items.push(item("u.sofa", "Modern Sofa", ItemKind::Symbol, 84.0, 36.0, &["couch"]));
        mine.items.push(item("u.base", "Base 24", ItemKind::Cabinet, 24.0, 24.0, &[]));
        let mut trad = item("u.chair", "Wing Chair", ItemKind::Symbol, 30.0, 32.0, &["traditional"]);
        trad.style = Some("Traditional".into());
        trad.manufacturer = Some("Hickory Co".into());
        mine.items.push(trad);
        mine.items.push(item("u.tile", "Slate", ItemKind::Material, 12.0, 12.0, &[]));
        let mut other = Catalog::new("Other");
        other.items.push(item("o.lamp", "Lamp", ItemKind::Fixture, 14.0, 14.0, &[]));
        let mut l = Library::default();
        l.add(mine);
        l.add(other);
        l
    }

    fn ids(v: Vec<&CatalogItem>) -> Vec<String> {
        v.into_iter().map(|i| i.id.clone()).collect()
    }

    #[test]
    fn an_empty_filter_keeps_all_in_name_order() {
        let f = Filter::default();
        assert!(f.is_empty());
        let got = ids(apply(&lib(), &f, &UserMeta::default()));
        assert_eq!(got, ["u.base", "o.lamp", "u.sofa", "u.tile", "u.chair"]);
    }

    #[test]
    fn type_catalog_style_and_size_filters() {
        let l = lib();
        let m = UserMeta::default();
        let mut f = Filter {
            kinds: vec![ItemKind::Cabinet, ItemKind::Material],
            ..Filter::default()
        };
        assert_eq!(ids(apply(&l, &f, &m)), ["u.base", "u.tile"]);
        f = Filter {
            catalog: Some("Other".into()),
            ..Filter::default()
        };
        assert_eq!(ids(apply(&l, &f, &m)), ["o.lamp"]);
        f = Filter {
            style: "tradition".into(),
            ..Filter::default()
        };
        assert_eq!(ids(apply(&l, &f, &m)), ["u.chair"]);
        f = Filter {
            style: "hickory".into(),
            ..Filter::default()
        };
        assert_eq!(ids(apply(&l, &f, &m)), ["u.chair"]);
        f = Filter {
            min_size: Some(25.0),
            max_size: Some(90.0),
            ..Filter::default()
        };
        assert_eq!(ids(apply(&l, &f, &m)), ["u.sofa", "u.chair"]);
        f.query = "chair".into();
        assert_eq!(ids(apply(&l, &f, &m)), ["u.chair"]);
        f = Filter {
            category: vec!["User".into(), "Missing".into()],
            ..Filter::default()
        };
        assert!(apply(&l, &f, &m).is_empty());
    }

    #[test]
    fn favorites_and_sorting() {
        let l = lib();
        let mut m = UserMeta::default();
        m.toggle_favorite("u.tile");
        m.toggle_favorite("o.lamp");
        m.add_recent("u.chair");
        m.add_recent("u.base");
        let mut f = Filter {
            favorites_only: true,
            ..Filter::default()
        };
        assert_eq!(ids(apply(&l, &f, &m)), ["o.lamp", "u.tile"]);
        assert_eq!(ids(favorites(&l, &m)), ["u.tile", "o.lamp"]);
        assert_eq!(ids(recent(&l, &m)), ["u.base", "u.chair"]);
        f = Filter {
            sort: SortKey::Size,
            ..Filter::default()
        };
        assert_eq!(ids(apply(&l, &f, &m))[0], "u.tile");
        f.sort = SortKey::Recent;
        assert_eq!(ids(apply(&l, &f, &m))[..2], ["u.base", "u.chair"]);
        f.sort = SortKey::Type;
        assert_eq!(ids(apply(&l, &f, &m))[0], "u.base");
        f.sort = SortKey::Name;
        assert_eq!(ids(apply(&l, &f, &m))[0], "u.base");
    }
}
