//! Text objects (class 25).
//!
//! A class 25 object placed on a floor is a text note (`10' CEILING HT`,
//! `TEMP GL. / EGRESS`, a room name typed as text). Fields, checked against
//! 13 notes of one project and the PDF it printed:
//!
//! ```text
//! +0x22  ff ff 00 00 00 ff ff ff   marker, then at +0x2a an i32 layer id
//!                                  (-55 "Default", others index the layer table)
//! +246   f64  x of the horizontal centre of the text, plan inches
//! +254   f64  y of the TOP of the text (the PDF text top is within 1.2")
//! then   strings: [0 or " "] font ("Avenir") style ("Book"/"Heavy") text
//! ```
//!
//! Not decoded: the text size (Plan Studio gets 4.5" for `Book` and 8" for
//! `Heavy`, the heights of Daniel's `Default Text Style` and `Room Label
//! Style`), rotation, box width, alignment and multi-line text. Objects
//! named `Wall Layer N - Viewed From Outside` are automatic labels of the
//! wall layer display and are skipped.

use super::tree::{cstring_at, fin, ObjectTree};

/// Text class id.
pub const TEXT: u8 = 25;
const X_AT: usize = 246;
const Y_AT: usize = 254;
const STRINGS_FROM: usize = 0x30;
const STRINGS_TO: usize = 0x300;
const FONTS: [&str; 5] = [
    "Avenir",
    "Arial",
    "Arial Narrow",
    "Chief Blueprint",
    "Times New Roman",
];
const STYLES: [&str; 6] = ["Book", "Heavy", "Bold", "Regular", "Light", "Italic"];

/// One decoded note.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefText {
    pub node: usize,
    pub text: String,
    /// Horizontal centre and top of the text.
    pub center_x: f64,
    pub top_y: f64,
    pub bold: bool,
    /// Chief layer id from the object header, if the header marker is there.
    pub layer_id: Option<i32>,
}

/// Decodes the text at tree index `node`.
pub fn decode_text(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefText> {
    let n = tree.node(node);
    if n.len() < Y_AT + 8 {
        return None;
    }
    let x = fin(bytes, n.marker + X_AT)?;
    let y = fin(bytes, n.marker + Y_AT)?;
    if x.abs() > 1.0e5 || y.abs() > 1.0e5 || (x == 0.0 && y == 0.0) {
        return None;
    }
    let end = n.end.min(n.marker + STRINGS_TO);
    let mut found: Vec<String> = Vec::new();
    let mut i = n.marker + STRINGS_FROM;
    while i + 5 <= end {
        match cstring_at(bytes, i, 300) {
            Some((s, next)) if next <= n.end => {
                found.push(s);
                i = next;
            }
            _ => i += 1,
        }
    }
    let bold = found.iter().any(|s| s == "Heavy" || s == "Bold");
    let text = found.into_iter().find(|s| {
        !FONTS.contains(&s.as_str())
            && !STYLES.contains(&s.as_str())
            && !s.trim().is_empty()
            && s != "0"
            && s != "description"
            && !s.starts_with('%')
            && !s.starts_with("Wall Layer ")
    })?;
    let marker_ok = bytes.get(n.marker + 0x22..n.marker + 0x2A)
        == Some(&[0xFF, 0xFF, 0, 0, 0, 0xFF, 0xFF, 0xFF]);
    let layer_id = marker_ok
        .then(|| super::tree::u32_at(bytes, n.marker + 0x2A).map(|v| v as i32))
        .flatten();
    Some(ChiefText {
        node,
        text,
        center_x: x,
        top_y: y,
        bold,
        layer_id,
    })
}

/// Height assumed for a note (inches): the `Room Label Style` for bold
/// notes, else the `Default Text Style`.
pub fn assumed_height(t: &ChiefText) -> f64 {
    if t.bold {
        8.0
    } else {
        4.5
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::tree::testutil::*;

    /// A text object with the strings after the header.
    pub fn text_obj(text: &str, bold: bool, x: f64, y: f64, layer: i32) -> Vec<u8> {
        sized(TEXT, 0, 0x1e0, |b| {
            b[0x22..0x2A].copy_from_slice(&[0xFF, 0xFF, 0, 0, 0, 0xFF, 0xFF, 0xFF]);
            put_u32(b, 0x2A, layer as u32);
            put_f64(b, X_AT, x);
            put_f64(b, Y_AT, y);
            let mut at = 0x120;
            for s in ["0", "Avenir", if bold { "Heavy" } else { "Book" }, text] {
                let c = cstr(s);
                b[at..at + c.len()].copy_from_slice(&c);
                at += c.len() + 2;
            }
        })
    }

    #[test]
    fn decodes_a_note() {
        let body = text_obj("10' CEILING HT", false, 494.09, 922.55, -55);
        let tree = ObjectTree::build(&body);
        let i = tree.of_kind(TEXT, 0).next().unwrap();
        let t = decode_text(&body, &tree, i).unwrap();
        assert_eq!(t.text, "10' CEILING HT");
        assert_eq!((t.center_x, t.top_y), (494.09, 922.55));
        assert!(!t.bold);
        assert_eq!(t.layer_id, Some(-55));
        assert_eq!(assumed_height(&t), 4.5);
        let b = text_obj("breakfast", true, 684.45, 929.03, -4);
        let tree = ObjectTree::build(&b);
        let t = decode_text(&b, &tree, tree.of_kind(TEXT, 0).next().unwrap()).unwrap();
        assert!(t.bold && assumed_height(&t) == 8.0);
        assert_eq!(t.layer_id, Some(-4));
    }

    #[test]
    fn skips_wall_layer_labels_and_empty_text() {
        let body = text_obj("Wall Layer 5 - Viewed From Outside", false, 10.0, 10.0, -23);
        let tree = ObjectTree::build(&body);
        let i = tree.of_kind(TEXT, 0).next().unwrap();
        assert!(decode_text(&body, &tree, i).is_none());
        let empty = text_obj(" ", false, 10.0, 10.0, -55);
        let tree = ObjectTree::build(&empty);
        assert!(decode_text(&empty, &tree, tree.of_kind(TEXT, 0).next().unwrap()).is_none());
        let origin = text_obj("X", false, 0.0, 0.0, -55);
        let tree = ObjectTree::build(&origin);
        assert!(decode_text(&origin, &tree, tree.of_kind(TEXT, 0).next().unwrap()).is_none());
        let tiny = sized(TEXT, 0, 0x80, |_| {});
        let tree = ObjectTree::build(&tiny);
        assert!(decode_text(&tiny, &tree, tree.of_kind(TEXT, 0).next().unwrap()).is_none());
    }
}
