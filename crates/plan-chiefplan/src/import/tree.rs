//! The object tree of a plan file.
//!
//! [`decode::find_objects`](crate::decode::find_objects) returns a flat list
//! of everything that looks like `[01] CD AB <class> <version> <u32 size>`.
//! A plan nests objects (a wall owns its line, its openings, its framing...)
//! and the byte pattern also turns up by chance inside textures and meshes,
//! so this module
//!
//! * keeps only objects whose version byte is 0 or 1 (every real object in the
//!   26 templates and 12 projects read so far; chance matches carry versions
//!   such as 28, 77, 107 or 255),
//! * nests by byte span and rejects an object that straddles its parent's end
//!   (a chance match whose declared size runs past the real object), and
//! * offers cheap readers for the little-endian numbers inside payloads.
//!
//! All offsets given in the docs and in the decoders are relative to the
//! object's `CD` byte (its [`Node::marker`]).

use crate::decode::{find_objects, RawObject};

/// One accepted object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    pub class: u8,
    pub version: u8,
    /// Offset of the `CD` byte.
    pub marker: usize,
    /// One past the last byte.
    pub end: usize,
    /// Index of the enclosing node, if any.
    pub parent: Option<usize>,
}

impl Node {
    /// Size in bytes, marker to end.
    pub fn len(&self) -> usize {
        self.end - self.marker
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// All accepted objects of a file, in file order, with parent links.
#[derive(Debug, Clone, Default)]
pub struct ObjectTree {
    pub nodes: Vec<Node>,
    /// Direct children of each node, in file order.
    children: Vec<Vec<usize>>,
    /// Objects the span rule rejected (diagnostics).
    pub rejected: usize,
}

impl ObjectTree {
    /// Builds the tree over `bytes`.
    pub fn build(bytes: &[u8]) -> ObjectTree {
        Self::from_raw(&find_objects(bytes))
    }

    /// Builds the tree from the flat list.
    pub fn from_raw(raw: &[RawObject]) -> ObjectTree {
        let mut nodes: Vec<Node> = Vec::with_capacity(raw.len());
        let mut stack: Vec<usize> = Vec::new();
        let mut rejected = 0;
        for o in raw {
            if o.version > 1 {
                rejected += 1;
                continue;
            }
            while let Some(&top) = stack.last() {
                if nodes[top].end <= o.marker {
                    stack.pop();
                } else {
                    break;
                }
            }
            if let Some(&top) = stack.last() {
                if o.end > nodes[top].end {
                    rejected += 1;
                    continue;
                }
            }
            let parent = stack.last().copied();
            nodes.push(Node {
                class: o.class,
                version: o.version,
                marker: o.marker,
                end: o.end,
                parent,
            });
            stack.push(nodes.len() - 1);
        }
        let mut children = vec![Vec::new(); nodes.len()];
        for (i, n) in nodes.iter().enumerate() {
            if let Some(p) = n.parent {
                children[p].push(i);
            }
        }
        ObjectTree {
            nodes,
            children,
            rejected,
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, i: usize) -> &Node {
        &self.nodes[i]
    }

    /// Direct children of `i`.
    pub fn children(&self, i: usize) -> &[usize] {
        &self.children[i]
    }

    /// Indexes of every node of `class` (any version), in file order.
    pub fn of_class(&self, class: u8) -> impl Iterator<Item = usize> + '_ {
        self.nodes
            .iter()
            .enumerate()
            .filter(move |(_, n)| n.class == class)
            .map(|(i, _)| i)
    }

    /// Indexes of every node of `class` and `version`.
    pub fn of_kind(&self, class: u8, version: u8) -> impl Iterator<Item = usize> + '_ {
        self.nodes
            .iter()
            .enumerate()
            .filter(move |(_, n)| n.class == class && n.version == version)
            .map(|(i, _)| i)
    }

    /// First direct child of `i` whose class is one of `classes`.
    pub fn first_child_of(&self, i: usize, classes: &[u8]) -> Option<usize> {
        self.children[i]
            .iter()
            .copied()
            .find(|&c| classes.contains(&self.nodes[c].class))
    }

    /// Whether `inner` lies inside `outer` (or is it).
    pub fn contains(&self, outer: usize, inner: usize) -> bool {
        let (o, n) = (&self.nodes[outer], &self.nodes[inner]);
        o.marker <= n.marker && n.end <= o.end
    }

    /// The nearest ancestor of `i` (not `i` itself) of class `class`.
    pub fn ancestor_of_class(&self, i: usize, class: u8) -> Option<usize> {
        let mut cur = self.nodes[i].parent;
        while let Some(p) = cur {
            if self.nodes[p].class == class {
                return Some(p);
            }
            cur = self.nodes[p].parent;
        }
        None
    }

    /// Byte ranges of `i`'s direct children (marker..end), in file order.
    pub fn child_spans(&self, i: usize) -> Vec<(usize, usize)> {
        self.children[i]
            .iter()
            .map(|&c| (self.nodes[c].marker, self.nodes[c].end))
            .collect()
    }
}

// ------------------------------------------------------------------ readers

pub(crate) fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(o..o.checked_add(4)?)?.try_into().ok()?,
    ))
}

pub(crate) fn f64_at(b: &[u8], o: usize) -> Option<f64> {
    Some(f64::from_le_bytes(
        b.get(o..o.checked_add(8)?)?.try_into().ok()?,
    ))
}

/// A finite `f64` or `None`.
pub(crate) fn fin(b: &[u8], o: usize) -> Option<f64> {
    f64_at(b, o).filter(|v| v.is_finite())
}

/// Whether `v` is a multiple of `1 / denom` inch within noise.
pub(crate) fn on_grid(v: f64, denom: f64) -> bool {
    (v * denom - (v * denom).round()).abs() < 1e-6
}

/// A `u32`-length-prefixed, NUL-terminated printable string at `o`. Returns
/// the text and the offset of the first byte after the NUL.
pub(crate) fn cstring_at(b: &[u8], o: usize, max: usize) -> Option<(String, usize)> {
    let len = u32_at(b, o)? as usize;
    if len == 0 || len > max {
        return None;
    }
    let s = b.get(o + 4..o + 4 + len)?;
    if *b.get(o + 4 + len)? != 0 || !s.iter().all(|&c| (0x20..=0x7E).contains(&c)) {
        return None;
    }
    Some((String::from_utf8_lossy(s).into_owned(), o + 5 + len))
}

/// Every length-prefixed printable string in `[from, to)`, in file order, as
/// `(offset from `from`, text)`. Strings with non-ASCII bytes (the copyright
/// sign in `Copyright\u{a9}2016`) are not returned.
pub(crate) fn strings_in(b: &[u8], from: usize, to: usize, max: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let to = to.min(b.len());
    let mut i = from;
    while i + 5 <= to {
        match cstring_at(b, i, max) {
            Some((s, next)) if next <= to => {
                out.push((i - from, s));
                i = next;
            }
            _ => i += 1,
        }
    }
    out
}

/// Bytes `[a, b)` that no child span covers, as ranges.
pub(crate) fn gaps(start: usize, end: usize, spans: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut pos = start;
    for &(s, e) in spans {
        if s > pos {
            out.push((pos, s.min(end)));
        }
        pos = pos.max(e);
    }
    if pos < end {
        out.push((pos, end));
    }
    out
}

#[cfg(test)]
pub(crate) mod testutil {
    //! Builders for synthetic plan bodies. Offsets are relative to the `CD`
    //! byte, like the decoders.

    /// `[01] CD AB class version size payload`, with `size` counting itself.
    pub fn obj(class: u8, version: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0x01, 0xCD, 0xAB, class, version];
        v.extend_from_slice(&((payload.len() + 4) as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    /// An object whose bytes from the `CD` byte total `total` (padding with
    /// zeros), with `fill` applied to the buffer that starts at the `CD`.
    pub fn sized(class: u8, version: u8, total: usize, fill: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut body = vec![0u8; total - 8];
        let mut full = vec![0xCD, 0xAB, class, version, 0, 0, 0, 0];
        full.append(&mut body);
        fill(&mut full);
        let size = (total - 4) as u32;
        full[4..8].copy_from_slice(&size.to_le_bytes());
        let mut out = vec![0x01];
        out.extend_from_slice(&full);
        out
    }

    pub fn put_f64(buf: &mut [u8], off: usize, v: f64) {
        buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
    }

    pub fn put_u32(buf: &mut [u8], off: usize, v: u32) {
        buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// `u32` length, bytes, NUL.
    pub fn cstr(s: &str) -> Vec<u8> {
        let mut v = (s.len() as u32).to_le_bytes().to_vec();
        v.extend_from_slice(s.as_bytes());
        v.push(0);
        v
    }

    /// Wraps `inner` (an object produced by [`sized`] or [`obj`]) in the
    /// serial-id wrapper `01 <id u32> 00 00 00 00` that precedes shared
    /// objects (wall types).
    pub fn with_id(id: u32, inner: Vec<u8>) -> Vec<u8> {
        let mut v = vec![0x01];
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&[0, 0, 0, 0]);
        // `inner` already starts with its own 0x01 prefix byte; the wrapper
        // replaces it.
        v.extend_from_slice(&inner[1..]);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;

    #[test]
    fn nests_by_span_and_drops_straddlers() {
        let inner = obj(31, 0, &[0u8; 8]);
        let mut outer_payload = vec![0u8; 4];
        outer_payload.extend_from_slice(&inner);
        outer_payload.extend_from_slice(&[0u8; 4]);
        let outer = obj(6, 0, &outer_payload);
        // A chance match: version 107 (rejected) and a straddler.
        let junk = obj(43, 107, &[0u8; 8]);
        let mut straddler = obj(9, 0, &[0u8; 8]);
        straddler[5..9].copy_from_slice(&0x1000u32.to_le_bytes());
        let mut body = outer.clone();
        body.extend_from_slice(&junk);
        body.extend_from_slice(&straddler);
        let tree = ObjectTree::build(&body);
        assert!(tree.rejected >= 1);
        let wall = tree.of_kind(6, 0).next().unwrap();
        assert_eq!(tree.children(wall).len(), 1);
        let line = tree.children(wall)[0];
        assert_eq!(tree.node(line).class, 31);
        assert_eq!(tree.first_child_of(wall, &[31, 40]), Some(line));
        assert!(tree.contains(wall, line));
        assert_eq!(tree.ancestor_of_class(line, 6), Some(wall));
        assert_eq!(tree.child_spans(wall).len(), 1);
    }

    #[test]
    fn strings_in_skips_non_ascii_and_reports_offsets() {
        let mut b = vec![0u8; 3];
        b.extend(cstr("Copyright\u{a9} 2016"));
        let at = b.len();
        b.extend(cstr("Bancroft Bed"));
        b.extend([0u8; 4]);
        b.extend(cstr("bedroom"));
        let got = strings_in(&b, 0, b.len(), 200);
        assert_eq!(
            got,
            vec![
                (at, "Bancroft Bed".to_string()),
                (at + 17 + 4, "bedroom".to_string())
            ]
        );
        // The range limits the search.
        assert!(strings_in(&b, 0, at + 6, 200).is_empty());
    }

    #[test]
    fn readers_are_bounds_checked() {
        let b = [1u8, 0, 0, 0, b'a', 0];
        assert_eq!(cstring_at(&b, 0, 10), Some(("a".to_string(), 6)));
        assert_eq!(cstring_at(&b, 2, 10), None);
        assert_eq!(u32_at(&b, 4), None);
        assert_eq!(f64_at(&b, 0), None);
        assert!(on_grid(0.625, 16.0) && !on_grid(0.6, 16.0));
        assert_eq!(
            gaps(0, 10, &[(2, 4), (6, 8)]),
            vec![(0, 2), (4, 6), (8, 10)]
        );
    }
}
