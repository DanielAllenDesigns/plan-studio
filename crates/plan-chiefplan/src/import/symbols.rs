//! Placed library objects (class 123): plumbing fixtures, appliances,
//! furniture, accessories.
//!
//! A plan holds its own copy of every placed library object: a class 123
//! record with the placement block of [`blocks`](super::blocks) (back-centre
//! point, front direction, depth, width, height, top elevation) and a class 114
//! child that carries the object's name, its category tags (`Elongated Toilet`,
//! `ADA`, `Universal Design`) and **the catalog link**. The bed, nightstand and
//! mirror sets of class 16 are groups of class 123 objects; the groups are not
//! imported themselves, their members are.
//!
//! # The catalog link (stage 3)
//!
//! Stage 2 concluded that the plan stores no link because it only searched the
//! Core catalogs' `UniqueId` column (8 of 147 hits). The entry does hold one:
//!
//! ```text
//! +8        16 bytes  GUID of this placed copy (different for every copy)
//! ...       copyright line, the name, the tag count and tags
//! U         16 bytes  GUID of the library item, in the byte order of the
//!                     catalog's UniqueId text (hex of the bytes = the text)
//! U+16      01 01 00 00 00 00 (00|01) 01 (14|0e) 00 00 00 01 00 00 00 ..
//!                     the anchor: the start of a 20-entry (X17: 14-entry) property list
//! ```
//!
//! Matched against `LibraryObjects.UniqueId` of **every** catalog (Core, Bonus
//! and Manufacturer: 93,765 objects), 222 of 222 anchored GUIDs read from 14
//! projects name the same item as the placed copy (214 with the identical
//! display name, 8 differing only in `Under-Mount` against `Undermount`): High
//! confidence where it matches. About half the placed objects match (236 of
//! 435 anchored entries); the rest are items of the 2010-era libraries
//! (`Copyright 2010, Chief Architect, Inc.`: `Elongated Toilet`, `Oval
//! (undermount)`, `Washer (curved front loading)`) whose GUID no installed X13
//! to X18 catalog holds, and a few entries of a newer layout (Wayfair chairs, a
//! round drain) put the GUID elsewhere, so the importer also offers every
//! GUID-shaped 16-byte window after the name as a candidate. A caller with the
//! catalogs ([`SymbolQuery`](super::SymbolQuery)) resolves the GUID first and
//! the name second.
//!
//! Checked on one job: 65 of 65 first-floor class 123 objects have a block
//! (a 36" x 30" toilet, a queen bed 84" x 66.8", a dryer 33" x 27", the
//! armchairs 23.9" x 22" in the dining room, a 128" x 86" patio set); the ones
//! whose parent is a floor or a furniture group are in absolute plan
//! coordinates, the ones inside a cabinet (a sink, a dishwasher) are in the
//! cabinet's frame at `(0, 0)` and are left out (the cabinet carries them).
//!
//! Without a resolver, or when it finds nothing, the item is named
//! `chief-plan.<slug of the name>`, which the Library tool shows as a labelled
//! box.

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
    /// The library item's GUID read at the anchor, as the catalog's
    /// `UniqueId` text (`8-4-4-4-12`, lower case).
    pub catalog_guid: Option<String>,
    /// Other GUID-shaped windows of the entry after the name (entries of the
    /// newer layout keep the item GUID elsewhere), without the anchored one.
    pub guid_candidates: Vec<String>,
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
    let strings = strings_in(bytes, n.marker + 0x18, n.end.min(n.marker + 400), 120);
    // Where the name ends, from the entry's `CD` byte.
    let name_end = strings
        .iter()
        .find(|(_, s)| *s == name)
        .map_or(0x18, |(o, s)| 0x18 + o + 5 + s.len());
    let tags: Vec<String> = strings
        .into_iter()
        .map(|(_, s)| s)
        .skip_while(|s| *s != name)
        .skip(1)
        .filter(|s| s.len() > 1 && !s.to_ascii_lowercase().starts_with("copyright"))
        .take(4)
        .collect();
    let (catalog_guid, guid_candidates) = entry_guids(bytes, n.marker, n.end, name_end);
    Some(ChiefSymbol {
        node,
        block,
        name,
        tags,
        catalog_guid,
        guid_candidates,
    })
}

/// The property-list anchor that follows the item GUID: byte 6 is 0 or 1 and
/// byte 8 the entry count (`0x14` in X18 files, `0x0e` in X17 files).
const GUID_ANCHOR: [u8; 24] = [
    0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x01, 0xFF, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
];

fn anchor_at(seg: &[u8], i: usize) -> bool {
    seg.get(i..i + GUID_ANCHOR.len()).is_some_and(|w| {
        w.iter()
            .zip(GUID_ANCHOR)
            .enumerate()
            .all(|(j, (&got, want))| match j {
                6 => got <= 1,
                8 => (8..=40).contains(&got),
                _ => got == want,
            })
    })
}

/// `8-4-4-4-12` lower-case text of 16 bytes (the catalogs' `UniqueId` form).
pub fn guid_text(b: &[u8]) -> String {
    let mut s = String::with_capacity(36);
    for (i, byte) in b.iter().take(16).enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            s.push('-');
        }
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

/// Whether 16 bytes look like a random GUID in either byte order (version
/// nibble 4 at byte 6 or 7, variant bits at byte 8 or 9, few zero bytes).
fn guid_shaped(w: &[u8]) -> bool {
    let version = (w[6] >> 4 == 4) || (w[7] >> 4 == 4);
    let variant = (w[8] >> 6 == 2) || (w[9] >> 6 == 2);
    version && variant && w.iter().filter(|&&c| c == 0).count() <= 2
}

/// The library item GUID of the entry spanning `[from, to)` (found at the
/// property-list anchor, 16 bytes before it) and the other GUID-shaped windows
/// after the entry's name and tags.
fn entry_guids(
    bytes: &[u8],
    from: usize,
    to: usize,
    name_end: usize,
) -> (Option<String>, Vec<String>) {
    let to = to.min(bytes.len());
    let Some(seg) = bytes.get(from..to) else {
        return (None, Vec::new());
    };
    let anchored = (16..seg.len()).find(|&i| anchor_at(seg, i)).map(|i| i - 16);
    let catalog_guid = anchored.map(|a| guid_text(&seg[a..a + 16]));
    // Candidates start after the name (the placed copy's own GUID sits at +8).
    let start = name_end;
    let mut candidates = Vec::new();
    let mut k = start;
    // Every offset is tried (a GUID may follow zero padding, and the shape
    // test cannot tell a GUID from itself shifted by a byte); the caller
    // checks each against its catalog, so extra windows cost nothing.
    while k + 16 <= seg.len() && candidates.len() < 12 {
        if guid_shaped(&seg[k..k + 16]) {
            let t = guid_text(&seg[k..k + 16]);
            if Some(&t) != catalog_guid.as_ref() && !candidates.contains(&t) {
                candidates.push(t);
            }
        }
        k += 1;
    }
    (catalog_guid, candidates)
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
        symbol_obj_guid(v, name, tags, None)
    }

    /// The same with the library item's GUID and the property-list anchor
    /// after the tags, as the plans store them.
    pub fn symbol_obj_guid(
        v: [f64; 8],
        name: &str,
        tags: &[&str],
        guid: Option<[u8; 16]>,
    ) -> Vec<u8> {
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
            // The placed copy's own GUID at +8 (not the item's).
            b[8..24].copy_from_slice(&[
                0x0d, 0x5c, 0x67, 0x44, 0xe7, 0x76, 0x4c, 0x3a, 0x94, 0x70, 0xdc, 0x83, 0x89, 0x72,
                0xae, 0x3a,
            ]);
            if let Some(g) = guid {
                let g_at = at + 16;
                b[g_at..g_at + 16].copy_from_slice(&g);
                b[g_at + 16..g_at + 16 + GUID_ANCHOR.len()].copy_from_slice(&GUID_ANCHOR);
                b[g_at + 22] = 0;
                b[g_at + 24] = 0x14;
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

    #[test]
    fn reads_the_item_guid_at_the_anchor_and_other_candidates() {
        // 4c528223-d712-45e5-b7a3-27f75f3e88c1, the toilet of a real plan.
        let g = [
            0x4c, 0x52, 0x82, 0x23, 0xd7, 0x12, 0x45, 0xe5, 0xb7, 0xa3, 0x27, 0xf7, 0x5f, 0x3e,
            0x88, 0xc1,
        ];
        let obj = symbol_obj_guid(
            [649.5, 416.7, -1.0, 0.0, 36.0, 30.0, 29.85, 29.85],
            "Elongated Toilet",
            &["ADA", "Universal Design"],
            Some(g),
        );
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(SYMBOL, 0).next().unwrap();
        let s = decode_symbol(&obj, &tree, i).unwrap();
        assert_eq!(
            s.catalog_guid.as_deref(),
            Some("4c528223-d712-45e5-b7a3-27f75f3e88c1")
        );
        // The anchored GUID is left out of the candidates; the placed copy's
        // own GUID (+8) is before the name and is never one.
        assert!(
            !s.guid_candidates
                .contains(&"4c528223-d712-45e5-b7a3-27f75f3e88c1".to_string())
                && !s.guid_candidates.iter().any(|c| c.starts_with("0d5c6744")),
            "{:?}",
            s.guid_candidates
        );
        assert_eq!(s.tags, vec!["ADA", "Universal Design"]);

        // An X17 file counts 14 properties (0x0e) instead of 20: same anchor.
        let mut x17 = obj.clone();
        let at = (0..x17.len()).find(|&i| anchor_at(&x17, i)).unwrap();
        x17[at + 8] = 0x0e;
        let tree = ObjectTree::build(&x17);
        let i = tree.of_kind(SYMBOL, 0).next().unwrap();
        let s17 = decode_symbol(&x17, &tree, i).unwrap();
        assert_eq!(s17.catalog_guid, s.catalog_guid);

        // An entry without the anchor: the GUID-shaped window after the name is
        // offered as a candidate instead.
        let mut obj = symbol_obj_guid(
            [1.0, 2.0, 0.0, 1.0, 5.0, 5.0, 5.0, 5.0],
            "Wayfair Chair",
            &[],
            Some(g),
        );
        let at = (0..obj.len()).find(|&i| anchor_at(&obj, i)).unwrap();
        obj[at] = 0xEE;
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(SYMBOL, 0).next().unwrap();
        let s = decode_symbol(&obj, &tree, i).unwrap();
        assert!(s.catalog_guid.is_none());
        assert!(
            s.guid_candidates
                .contains(&"4c528223-d712-45e5-b7a3-27f75f3e88c1".to_string()),
            "{:?}",
            s.guid_candidates
        );
        // No GUID at all: nothing.
        let obj = symbol_obj([1.0, 2.0, 0.0, 1.0, 5.0, 5.0, 5.0, 5.0], "Plain", &["x1"]);
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(SYMBOL, 0).next().unwrap();
        let s = decode_symbol(&obj, &tree, i).unwrap();
        assert!(s.catalog_guid.is_none() && s.guid_candidates.is_empty());
    }

    #[test]
    fn guid_text_is_the_catalog_unique_id_form() {
        let b: Vec<u8> = (0u8..16).collect();
        assert_eq!(guid_text(&b), "00010203-0405-0607-0809-0a0b0c0d0e0f");
        assert!(guid_shaped(&[
            0x4c, 0x52, 0x82, 0x23, 0xd7, 0x12, 0x45, 0xe5, 0xb7, 0xa3, 0x27, 0xf7, 0x5f, 0x3e,
            0x88, 0xc1
        ]));
        assert!(!guid_shaped(&[0u8; 16]));
        assert!(!guid_shaped(b"Elongated Toilet"));
    }
}
