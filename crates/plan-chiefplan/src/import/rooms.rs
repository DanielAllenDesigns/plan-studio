//! Rooms (class 23 inside a floor).
//!
//! Class 23 is also the room *type* definition (Kitchen, Bedroom ...: 53 of
//! them in the template, outside any floor). A room placed on a floor is a
//! class 23 object nested in a class 30 object. Layout, found by comparing
//! 70 rooms of one project with the labels of its PDF:
//!
//! ```text
//! +0x139      string  room type name ("Default" for an ordinary room, else
//!                     "WINE CELLAR", "mud room", "STAIR Hall" ...)
//!     ...     16-byte GUID of the type, a few flag bytes
//!     p-48..  f64 f64  area and interior area, square inches (only some
//!                      rooms; -541265.15 twice marks "none")
//!     p-32    f64 f64  width and height of the room's bounding box
//!     p-16    f64 f64  centre x, y of that box, plan inches  <- anchor
//!     p       u8, 00 00 00 00 00 00, 80, 01, 80   (the anchor of the search)
//!     p+0x121 string  the room's own name ("F. Porch"), when it has one
//! ```
//!
//! With the type string `Default` the room shows its label from a separate
//! text label object (class 48 holding the text, e.g. `kitchen`), whose
//! position was not decoded; those rooms import without a name.
//! The centre is the centre of the bounding box and can lie outside an
//! L-shaped room.
//!
//! X17 files use the same layout with the type string at `+0x109` and the
//! anchor 68 to 90 bytes after the end of the type string (the search span is
//! 0x200). Checked on one X17 job: 14 of 14 first-floor rooms give a centre
//! inside the walls' box, e.g. `future bedrm` 164 x 168.5 at `(1004.5, 598.4)`.
//! A room whose box was not stored (the `-541265.15` marker) reads garbage there
//! and can import with a wrong anchor.

use super::tree::{cstring_at, fin, ObjectTree};

/// Room class id.
pub const ROOM: u8 = 23;
/// First offset searched for the type string.
const TYPE_FROM: usize = 0x100;
const TYPE_TO: usize = 0x200;
/// Bytes searched after the type string for the centre anchor.
const ANCHOR_SPAN: usize = 0x200;

/// One decoded room.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefRoom {
    pub node: usize,
    pub room_type: String,
    /// The room's own name, if it has one.
    pub name: Option<String>,
    pub center: (f64, f64),
    /// Bounding box width and height.
    pub size: (f64, f64),
}

impl ChiefRoom {
    /// The name Plan Studio shows: the room's own, else its type unless the
    /// type is the generic `Default`.
    pub fn display_name(&self) -> Option<String> {
        self.name
            .clone()
            .or_else(|| (self.room_type != "Default").then(|| self.room_type.clone()))
    }
}

fn anchor_after(bytes: &[u8], from: usize, to: usize) -> Option<usize> {
    (from..to.saturating_sub(10)).find(|&p| {
        bytes[p + 1..p + 7].iter().all(|&c| c == 0)
            && bytes[p + 7] == 0x80
            && bytes[p + 8] == 0x01
            && bytes[p + 9] == 0x80
    })
}

/// Decodes the room at tree index `node`.
pub fn decode_room(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefRoom> {
    let n = tree.node(node);
    let limit = n.len().min(0x1000);
    let (room_type, type_end) =
        (TYPE_FROM..TYPE_TO.min(limit)).find_map(|o| cstring_at(bytes, n.marker + o, 64))?;
    let hi = (type_end + ANCHOR_SPAN).min(n.marker + limit);
    let p = anchor_after(bytes, type_end, hi)?;
    let cy = fin(bytes, p - 8)?;
    let cx = fin(bytes, p - 16)?;
    let h = fin(bytes, p - 24)?;
    let w = fin(bytes, p - 32)?;
    if cx.abs() > 1.0e5
        || cy.abs() > 1.0e5
        || !(0.0..=1.0e5).contains(&w)
        || !(0.0..=1.0e5).contains(&h)
    {
        return None;
    }
    let name = (p + 0x110..=p + 0x130)
        .find_map(|q| cstring_at(bytes, q, 80))
        .map(|(s, _)| s)
        .filter(|s| s != "description" && !s.starts_with('%'));
    Some(ChiefRoom {
        node,
        room_type,
        name,
        center: (cx, cy),
        size: (w, h),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::floors::tests::floor_obj;
    use crate::import::tree::testutil::*;

    /// A room object: type string at 0x139, centre block, optional name.
    pub fn room_obj(ty: &str, name: Option<&str>, size: (f64, f64), center: (f64, f64)) -> Vec<u8> {
        sized(ROOM, 0, 0x500, |b| {
            let t = cstr(ty);
            b[0x139..0x139 + t.len()].copy_from_slice(&t);
            let end = 0x139 + t.len();
            // Block: 16-byte GUID gap, area pair, size pair, centre pair.
            let mut at = end + 0x20;
            put_f64(b, at, 10498.855);
            put_f64(b, at + 8, 6707.68);
            put_f64(b, at + 16, size.0);
            put_f64(b, at + 24, size.1);
            put_f64(b, at + 32, center.0);
            put_f64(b, at + 40, center.1);
            at += 48;
            // The anchor bytes: k, six zeros, 0x80, 0x01, 0x80.
            b[at] = 2;
            b[at + 7] = 0x80;
            b[at + 8] = 0x01;
            b[at + 9] = 0x80;
            if let Some(n) = name {
                let s = cstr(n);
                let q = at + 0x121;
                b[q..q + s.len()].copy_from_slice(&s);
            }
        })
    }

    #[test]
    fn named_unnamed_and_typed_rooms() {
        let rooms = [
            room_obj(
                "Default",
                Some("F. Porch"),
                (121.98, 54.99),
                (639.467, 511.361),
            ),
            room_obj("Default", None, (168.0, 162.0), (481.467, 616.856)),
            room_obj("WINE CELLAR", None, (60.0, 80.0), (900.0, 700.0)),
            room_obj(
                "STAIR Hall",
                Some("STAIR Hall"),
                (50.0, 90.0),
                (300.0, 400.0),
            ),
        ];
        let body = floor_obj(0.0, 121.125, &rooms);
        let tree = ObjectTree::build(&body);
        let got: Vec<ChiefRoom> = tree
            .of_kind(ROOM, 0)
            .filter_map(|i| decode_room(&body, &tree, i))
            .collect();
        assert_eq!(got.len(), 4);
        assert_eq!(got[0].name.as_deref(), Some("F. Porch"));
        assert_eq!(got[0].display_name().as_deref(), Some("F. Porch"));
        assert_eq!(got[0].center, (639.467, 511.361));
        assert_eq!(got[0].size, (121.98, 54.99));
        assert_eq!(got[1].name, None);
        assert_eq!(got[1].display_name(), None);
        assert_eq!(got[2].room_type, "WINE CELLAR");
        assert_eq!(got[2].display_name().as_deref(), Some("WINE CELLAR"));
        assert_eq!(got[3].name.as_deref(), Some("STAIR Hall"));
    }

    #[test]
    fn rejects_objects_without_the_anchor() {
        let blank = sized(ROOM, 0, 0x300, |b| {
            let t = cstr("Default");
            b[0x139..0x139 + t.len()].copy_from_slice(&t);
        });
        let tree = ObjectTree::build(&blank);
        let i = tree.of_kind(ROOM, 0).next().unwrap();
        assert!(decode_room(&blank, &tree, i).is_none());
        // A type definition object (no string in the window) is also None.
        let none = sized(ROOM, 0, 0x300, |_| {});
        let tree = ObjectTree::build(&none);
        let i = tree.of_kind(ROOM, 0).next().unwrap();
        assert!(decode_room(&none, &tree, i).is_none());
    }
}
