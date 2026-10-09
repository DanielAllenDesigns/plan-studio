//! The Library Browser's search, honouring Preferences > Library Browser
//! (names, descriptions, keywords, catalog names; whole words; every word or
//! any word). docs/parity CB-53..CB-61, DECISIONS LB5.
//!
//! The catalog items carry no free-text description, so "descriptions" looks
//! at the style and the manufacturer, and "keywords" at the tags and the
//! category path.

use crate::dialogs::preferences::pages::LibraryBrowserPrefs;
use plan_library::CatalogItem;
use std::collections::HashMap;

/// Splits `text` into lower-case words.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// True when `term` is in `text` (as a whole word when `whole`).
fn found(text: &str, term: &str, whole: bool) -> bool {
    if whole {
        words(text).iter().any(|w| w == term)
    } else {
        text.to_lowercase().contains(term)
    }
}

/// Relevance of `item` for one `term` under `opts` (0 means no match): a name
/// hit beats a keyword, a description and a catalog name.
fn term_score(item: &CatalogItem, catalog: &str, term: &str, opts: &LibraryBrowserPrefs) -> u32 {
    let whole = opts.whole_words;
    let mut best = 0;
    if opts.search_names && found(&item.name, term, whole) {
        let exact = words(&item.name).iter().any(|w| w == term);
        best = if exact { 100 } else { 60 };
    }
    if opts.search_keywords {
        if item.tags.iter().any(|t| found(t, term, whole)) {
            best = best.max(40);
        }
        if item.category.iter().any(|c| found(c, term, whole)) {
            best = best.max(20);
        }
    }
    if opts.search_descriptions {
        let hit = item.style.as_deref().is_some_and(|s| found(s, term, whole))
            || item
                .manufacturer
                .as_deref()
                .is_some_and(|m| found(m, term, whole));
        if hit {
            best = best.max(30);
        }
    }
    if opts.search_catalog_names && found(catalog, term, whole) {
        best = best.max(15);
    }
    best
}

/// The score of `item` for `terms`, or `None` when it does not match: every
/// term must match when `opts.match_all_words`, else one is enough.
pub fn score(
    item: &CatalogItem,
    catalog: &str,
    terms: &[String],
    opts: &LibraryBrowserPrefs,
) -> Option<u32> {
    let mut total = 0;
    let mut hits = 0;
    for t in terms {
        let s = term_score(item, catalog, t, opts);
        if s > 0 {
            hits += 1;
            total += s;
        } else if opts.match_all_words {
            return None;
        }
    }
    (hits > 0).then_some(total)
}

/// The search terms of `query`.
pub fn terms(query: &str) -> Vec<String> {
    query.split_whitespace().map(str::to_lowercase).collect()
}

/// Keeps the `items` that match `query` under `opts`; with `ranked` the best
/// matches come first (ties keep the incoming order).
pub fn narrow<'a>(
    items: Vec<&'a CatalogItem>,
    catalogs: &HashMap<&str, &str>,
    query: &str,
    opts: &LibraryBrowserPrefs,
    ranked: bool,
) -> Vec<&'a CatalogItem> {
    let terms = terms(query);
    if terms.is_empty() {
        return items;
    }
    let mut scored: Vec<(u32, &CatalogItem)> = items
        .into_iter()
        .filter_map(|i| {
            let cat = catalogs.get(i.id.as_str()).copied().unwrap_or("");
            score(i, cat, &terms, opts).map(|s| (s, i))
        })
        .collect();
    if ranked {
        scored.sort_by(|a, b| b.0.cmp(&a.0));
    }
    scored.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_library::{Placement, Symbol2d};

    fn item(id: &str, name: &str, tags: &[&str]) -> CatalogItem {
        CatalogItem::new(id, name, Placement::FreeStanding, Symbol2d::default())
            .with_category(&["Architectural", "Bath"])
            .with_tags(tags)
    }

    fn all() -> LibraryBrowserPrefs {
        LibraryBrowserPrefs::default()
    }

    #[test]
    fn whole_words_reject_a_part_of_a_word() {
        let sink = item("a", "Sinks", &[]);
        let mut o = all();
        assert!(score(&sink, "", &terms("sink"), &o).is_some());
        o.whole_words = true;
        assert!(score(&sink, "", &terms("sink"), &o).is_none());
        assert!(score(&sink, "", &terms("sinks"), &o).is_some());
    }

    #[test]
    fn keywords_and_names_switch_independently() {
        let couch = item("a", "Modern Sofa", &["couch"]);
        let mut o = all();
        assert!(score(&couch, "", &terms("couch"), &o).is_some());
        o.search_keywords = false;
        assert!(score(&couch, "", &terms("couch"), &o).is_none());
        assert!(score(&couch, "", &terms("sofa"), &o).is_some());
        o.search_names = false;
        assert!(score(&couch, "", &terms("sofa"), &o).is_none());
    }

    #[test]
    fn catalog_names_count_only_when_on() {
        let i = item("a", "Lamp", &[]);
        let mut o = all();
        assert!(score(&i, "Core Catalog", &terms("core"), &o).is_none());
        o.search_catalog_names = true;
        assert!(score(&i, "Core Catalog", &terms("core"), &o).is_some());
    }

    #[test]
    fn every_word_or_any_word() {
        let i = item("a", "Towel Rack", &[]);
        let mut o = all();
        assert!(score(&i, "", &terms("towel lamp"), &o).is_none());
        o.match_all_words = false;
        assert!(score(&i, "", &terms("towel lamp"), &o).is_some());
    }

    #[test]
    fn names_rank_above_keywords() {
        let by_name = item("n", "Sink Unit", &[]);
        let by_tag = item("t", "Towel Rack", &["sink"]);
        let map: HashMap<&str, &str> = HashMap::new();
        let out = narrow(vec![&by_tag, &by_name], &map, "sink", &all(), true);
        assert_eq!(out[0].id, "n");
    }
}
