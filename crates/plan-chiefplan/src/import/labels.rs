//! Text labels (class 48): room names and area macros.
//!
//! A class 48 object directly on a floor is a label attached to an area. Most are
//! the area readout (`%area.round%`, 12 per floor); a few hold a typed name
//! (`kitchen`, `family rm`, `3 car garage`, `pwd`: 9 on the first floor of job A,
//! `AREA BELOW GRADE`, `CATHEDRAL CEILING` in others). The text is a plain
//! length-prefixed string; the label's **frame** is a rectangle of four line
//! records (90 bytes apart, from about +680):
//!
//! ```text
//! kitchen        (711.96,1054.86) to (891.96,802.86)   centre (801.96, 928.86)
//! room object    centre (795, 929), box 181 x 265
//! ```
//!
//! The frame is the label's text area, not a wall outline: its centre is the
//! spot where the text prints (the stage-1 note that the label "sits at y =
//! 1052.96 for a printed 922" looked at the frame's top edge, `y = 1054.86`).
//! The frame centre falls inside the room it names in all 9 labels of job A
//! (the centre of the `3 car garage` frame, `(1024, 739)`, is inside the garage
//! room's 405 x 270 box centred `(1196, 680)` but 172" from its centre, so rooms
//! are matched by containment, not by nearest centre).
//!
//! Not decoded: font and size of the printed text, the leader, macros other than
//! `%area%`.

use super::lines::{find_edges, polygon_of};
use super::tree::{strings_in, ObjectTree};

/// Label class id.
pub const LABEL: u8 = 48;

/// One decoded named label.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefLabel {
    pub node: usize,
    pub text: String,
    /// Centre of the label's frame.
    pub center: (f64, f64),
    /// Frame width and height.
    pub size: (f64, f64),
}

fn is_name(s: &str) -> bool {
    s != "description"
        && s != "%automatic_description%"
        && !s.starts_with('%')
        && !s.starts_with('=')
        && s.chars().any(|c| c.is_ascii_alphabetic())
}

/// Decodes the class 48 object at `node`. `None` for the area macros and for
/// labels without a frame.
pub fn decode_label(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefLabel> {
    let n = tree.node(node);
    if n.len() > 3000 {
        return None;
    }
    let text = strings_in(bytes, n.marker, n.end, 120)
        .into_iter()
        .map(|(_, s)| s)
        .find(|s| is_name(s))?;
    let edges = find_edges(bytes, n.marker, n.marker + 0x200, n.end);
    // The frame: the first edge and the next three.
    let frame: Vec<_> = edges.iter().take(4).copied().collect();
    let pts = polygon_of(&frame);
    if frame.len() < 4 || pts.len() != 4 {
        return None;
    }
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for &(x, y) in &pts {
        lo = (lo.0.min(x), lo.1.min(y));
        hi = (hi.0.max(x), hi.1.max(y));
    }
    Some(ChiefLabel {
        node,
        text,
        center: ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0),
        size: (hi.0 - lo.0, hi.1 - lo.1),
    })
}

/// Whether a label text is an area annotation rather than a room name.
pub fn is_area_note(text: &str) -> bool {
    let t = text.to_ascii_uppercase();
    t.starts_with("AREA ") || t.contains("CEILING") || t.contains("BELOW GRADE")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::lines::tests::put_rect;
    use crate::import::tree::testutil::*;

    /// A label object with `text` and a frame rectangle at 680.
    pub fn label_obj(text: &str, x: f64, y: f64, w: f64, h: f64) -> Vec<u8> {
        sized(LABEL, 0, 1330, |b| {
            let c = cstr(text);
            b[0x40..0x40 + c.len()].copy_from_slice(&c);
            put_rect(b, 680, x, y, w, h);
        })
    }

    #[test]
    fn decodes_name_and_frame_centre() {
        let obj = label_obj("kitchen", 711.96, 802.86, 180.0, 252.0);
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(LABEL, 0).next().unwrap();
        let l = decode_label(&obj, &tree, i).unwrap();
        assert_eq!(l.text, "kitchen");
        assert!((l.center.0 - 801.96).abs() < 1e-9 && (l.center.1 - 928.86).abs() < 1e-9);
        assert!((l.size.0 - 180.0).abs() < 1e-9 && (l.size.1 - 252.0).abs() < 1e-9);
    }

    #[test]
    fn area_macros_and_frameless_labels_are_skipped() {
        let obj = label_obj("%area.round%", 0.0, 0.0, 10.0, 10.0);
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(LABEL, 0).next().unwrap();
        assert!(decode_label(&obj, &tree, i).is_none());
        let none = sized(LABEL, 0, 1330, |b| {
            let c = cstr("pwd");
            b[0x40..0x40 + c.len()].copy_from_slice(&c);
        });
        let tree = ObjectTree::build(&none);
        let i = tree.of_kind(LABEL, 0).next().unwrap();
        assert!(decode_label(&none, &tree, i).is_none());
        assert!(is_area_note("AREA BELOW GRADE"));
        assert!(is_area_note("CATHEDRAL CEILING"));
        assert!(!is_area_note("kitchen"));
    }
}
