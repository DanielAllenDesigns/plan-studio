//! Placed library objects (class 123): plumbing fixtures, appliances,
//! furniture, accessories.
//!
//! A plan holds its own copy of every placed library object, so there is no
//! reference back to a catalog: the object is a class 123 record with the
//! placement block of [`blocks`](super::blocks) (back-centre point, front
//! direction, depth, width, height, top elevation) and a class 114 child that
//! carries the object's name and its category tags (`Elongated Toilet`,
//! `ADA`, `Universal Design`). The bed, nightstand and mirror sets of class 16
//! are groups of class 123 objects; the groups are not imported themselves, their
//! members are.
//!
//! Checked on one job: 65 of 65 first-floor class 123 objects have a block
//! (a 36" x 30" toilet, a queen bed 84" x 66.8", a dryer 33" x 27", the
//! armchairs 23.9" x 22" in the dining room, a 128" x 86" patio set); the ones
//! whose parent is a floor or a furniture group are in absolute plan
//! coordinates, the ones inside a cabinet (a sink, a dishwasher) are in the
//! cabinet's frame at `(0, 0)` and are left out (the cabinet carries them).
//!
//! The catalog identity is not stored: the 114 entry holds a GUID per placed
//! copy and the tag GUIDs, none of which occurs in the Core catalogs'
//! `LibraryObjects.UniqueId` column (the toilet's two catalog GUIDs are absent
//! from the plan). The importer therefore names the item
//! `chief-plan.<slug of the name>`, which the Library tool shows as a labelled
//! box, and offers [`ImportOptions::symbol_resolver`](super::ImportOptions) so
//! an app that has the catalogs can map name and tags to a real
//! `chief.<catalog-uuid>.<object id>`.

use super::blocks::{own_block, BoxBlock};
use super::cabinets::entry_name;
use super::tree::{strings_in, ObjectTree};

/// Library object class id.
pub const SYMBOL: u8 = 123;
/// The entry class holding the object's name and tags.
const ENTRY: u8 = 114;

/// One decoded library object.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefSymbol {
    pub node: usize,
    pub block: BoxBlock,
    pub name: String,
    /// The tags after the name (`ADA`, `Universal Design`, ...).
    pub tags: Vec<String>,
}

/// Decodes the class 123 object at `node`; `None` without a block or a name.
pub fn decode_symbol(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefSymbol> {
    let block = own_block(bytes, tree, node)?;
    let entry = tree
        .children(node)
        .iter()
        .copied()
        .find(|&c| tree.node(c).class == ENTRY)?;
    let name = entry_name(bytes, tree, entry)?;
    let n = tree.node(entry);
    let tags: Vec<String> = strings_in(bytes, n.marker + 0x18, n.end.min(n.marker + 400), 120)
        .into_iter()
        .map(|(_, s)| s)
        .skip_while(|s| *s != name)
        .skip(1)
        .filter(|s| s.len() > 1 && !s.to_ascii_lowercase().starts_with("copyright"))
        .take(4)
        .collect();
    Some(ChiefSymbol {
        node,
        block,
        name,
        tags,
    })
}

/// `chief-plan.<slug>`: the stand-in catalog id of an item whose catalog is
/// unknown.
pub fn fallback_catalog_id(name: &str) -> String {
    let mut slug = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    format!(
        "chief-plan.{}",
        if slug.is_empty() { "object" } else { slug }
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::blocks::tests::put_block;
    use crate::import::tree::testutil::*;

    /// A class 123 object with a class 114 entry child (name and tags).
    pub fn symbol_obj(v: [f64; 8], name: &str, tags: &[&str]) -> Vec<u8> {
        let entry = sized(ENTRY, 0, 0x180, |b| {
            // The copyright line has a non-ASCII sign: unreadable on purpose.
            let c = cstr(name);
            b[0x3a..0x3a + c.len()].copy_from_slice(&c);
            let mut at = 0x3a + c.len() + 8;
            for t in tags {
                let c = cstr(t);
                b[at..at + c.len()].copy_from_slice(&c);
                at += c.len() + 4;
            }
        });
        let total = 0x500 + entry.len() - 1;
        sized(SYMBOL, 0, total, |b| {
            put_block(b, 0x300, v);
            b[0x500..0x500 + entry.len() - 1].copy_from_slice(&entry[1..]);
        })
    }

    #[test]
    fn decodes_name_tags_and_block() {
        let obj = symbol_obj(
            [649.5, 416.7, -1.0, 0.0, 36.0, 30.0, 29.85, 29.85],
            "Elongated Toilet",
            &["ADA", "Universal Design"],
        );
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(SYMBOL, 0).next().unwrap();
        let s = decode_symbol(&obj, &tree, i).unwrap();
        assert_eq!(s.name, "Elongated Toilet");
        assert_eq!(s.tags, vec!["ADA", "Universal Design"]);
        assert_eq!((s.block.depth, s.block.width), (36.0, 30.0));
        assert_eq!(s.block.elevation(), 0.0);
    }

    #[test]
    fn symbols_without_block_or_entry_are_none() {
        let obj = sized(SYMBOL, 0, 0x400, |_| {});
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(SYMBOL, 0).next().unwrap();
        assert!(decode_symbol(&obj, &tree, i).is_none());
        // A block but no 114 child.
        let obj = sized(SYMBOL, 0, 0x400, |b| {
            put_block(b, 0x100, [1.0, 2.0, 0.0, 1.0, 5.0, 5.0, 5.0, 5.0]);
        });
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(SYMBOL, 0).next().unwrap();
        assert!(decode_symbol(&obj, &tree, i).is_none());
    }

    #[test]
    fn fallback_ids_are_slugs() {
        assert_eq!(
            fallback_catalog_id("Washer (curved front loading)"),
            "chief-plan.washer-curved-front-loading"
        );
        assert_eq!(fallback_catalog_id("K-1966-GHW"), "chief-plan.k-1966-ghw");
        assert_eq!(fallback_catalog_id("***"), "chief-plan.object");
    }
}
