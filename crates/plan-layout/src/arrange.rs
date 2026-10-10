//! Arranging layout boxes: align, distribute and copy (L-6).
//!
//! Pure functions on a page's boxes in paper inches, so the layout window
//! and the tests share them. Moving a box moves its rectangle; its source,
//! scale and settings stay.

use crate::model::{Layout, LayoutBox, LayoutPage};
use plan_core::{Id, Point};

/// Which edge or centre line boxes line up on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignEdge {
    Left,
    /// Centres on a vertical line.
    HCenter,
    Right,
    Top,
    /// Centres on a horizontal line.
    VCenter,
    Bottom,
}

impl AlignEdge {
    pub const ALL: [AlignEdge; 6] = [
        AlignEdge::Left,
        AlignEdge::HCenter,
        AlignEdge::Right,
        AlignEdge::Top,
        AlignEdge::VCenter,
        AlignEdge::Bottom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AlignEdge::Left => "Align Left",
            AlignEdge::HCenter => "Align Centers (vertical line)",
            AlignEdge::Right => "Align Right",
            AlignEdge::Top => "Align Top",
            AlignEdge::VCenter => "Align Middles (horizontal line)",
            AlignEdge::Bottom => "Align Bottom",
        }
    }
}

/// Which way boxes are spread out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spread {
    /// Equal gaps between boxes from left to right.
    Horizontal,
    /// Equal gaps between boxes from bottom to top.
    Vertical,
}

fn shift(b: &mut LayoutBox, dx: f64, dy: f64) {
    b.rect_in.0 = Point::new(b.rect_in.0.x + dx, b.rect_in.0.y + dy);
    b.rect_in.1 = Point::new(b.rect_in.1.x + dx, b.rect_in.1.y + dy);
}

fn bounds(b: &LayoutBox) -> [f64; 4] {
    b.bounds_in()
}

/// Lines up the boxes `ids` of `page` on `edge`. With `to` (a rectangle
/// `[x0, y0, x1, y1]`, the page's drawing area for a single box) they line
/// up on that rectangle's edge; without it, on the outermost edge of the
/// boxes themselves. Returns how many boxes moved.
pub fn align_boxes(
    page: &mut LayoutPage,
    ids: &[Id],
    edge: AlignEdge,
    to: Option<[f64; 4]>,
) -> usize {
    let rects: Vec<[f64; 4]> = page
        .boxes
        .iter()
        .filter(|b| ids.contains(&b.id))
        .map(bounds)
        .collect();
    if rects.is_empty() {
        return 0;
    }
    let reference = to.unwrap_or_else(|| {
        rects.iter().fold(
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            |a, r| {
                [
                    a[0].min(r[0]),
                    a[1].min(r[1]),
                    a[2].max(r[2]),
                    a[3].max(r[3]),
                ]
            },
        )
    });
    let mut moved = 0;
    for b in page.boxes.iter_mut().filter(|b| ids.contains(&b.id)) {
        let r = bounds(b);
        let (dx, dy) = match edge {
            AlignEdge::Left => (reference[0] - r[0], 0.0),
            AlignEdge::Right => (reference[2] - r[2], 0.0),
            AlignEdge::HCenter => ((reference[0] + reference[2] - r[0] - r[2]) * 0.5, 0.0),
            AlignEdge::Bottom => (0.0, reference[1] - r[1]),
            AlignEdge::Top => (0.0, reference[3] - r[3]),
            AlignEdge::VCenter => (0.0, (reference[1] + reference[3] - r[1] - r[3]) * 0.5),
        };
        if dx.abs() > 1e-9 || dy.abs() > 1e-9 {
            shift(b, dx, dy);
            moved += 1;
        }
    }
    moved
}

/// Spreads the boxes `ids` of `page` so the gaps between neighbours are
/// equal: the two outermost boxes stay, the ones between move. Needs three
/// boxes; returns how many moved.
pub fn distribute_boxes(page: &mut LayoutPage, ids: &[Id], axis: Spread) -> usize {
    let key = |r: &[f64; 4]| match axis {
        Spread::Horizontal => (r[0], r[2]),
        Spread::Vertical => (r[1], r[3]),
    };
    let mut order: Vec<(Id, (f64, f64))> = page
        .boxes
        .iter()
        .filter(|b| ids.contains(&b.id))
        .map(|b| (b.id, key(&bounds(b))))
        .collect();
    if order.len() < 3 {
        return 0;
    }
    order.sort_by(|a, b| a.1 .0.total_cmp(&b.1 .0));
    let first = order[0].1;
    let last_end = order
        .iter()
        .map(|o| o.1 .1)
        .fold(f64::NEG_INFINITY, f64::max);
    let total: f64 = order.iter().map(|o| o.1 .1 - o.1 .0).sum();
    let gap = (last_end - first.0 - total) / (order.len() - 1) as f64;
    let mut at = first.0;
    let mut moved = 0;
    for (id, (lo, hi)) in order {
        let d = at - lo;
        at += hi - lo + gap;
        if d.abs() <= 1e-9 {
            continue;
        }
        if let Some(b) = page.boxes.iter_mut().find(|b| b.id == id) {
            match axis {
                Spread::Horizontal => shift(b, d, 0.0),
                Spread::Vertical => shift(b, 0.0, d),
            }
            moved += 1;
        }
    }
    moved
}

/// How far a copy on the same page lands from the original, paper inches.
pub const COPY_OFFSET_IN: f64 = 0.5;

/// Copies the boxes `ids` of page `from` onto page `to` (page numbers) and
/// returns the new boxes' ids. On another page the copies keep their place;
/// on the same page they land [`COPY_OFFSET_IN`] right and below the
/// originals so they can be told apart. Each copy gets a fresh id; the
/// source, scale and settings are the original's. Empty when a page is
/// missing.
pub fn copy_boxes(layout: &mut Layout, from: u32, ids: &[Id], to: u32) -> Vec<Id> {
    if layout.page(to).is_none() {
        return Vec::new();
    }
    let originals: Vec<LayoutBox> = match layout.page(from) {
        Some(p) => p
            .boxes
            .iter()
            .filter(|b| ids.contains(&b.id))
            .cloned()
            .collect(),
        None => return Vec::new(),
    };
    let first_free = layout
        .pages
        .iter()
        .flat_map(|p| p.boxes.iter().map(|b| b.id))
        .max()
        .map_or(1, |m| m + 1);
    let same = from == to;
    let mut made = Vec::new();
    for (mut b, id) in originals.into_iter().zip(first_free..) {
        b.id = id;
        if same {
            shift(&mut b, COPY_OFFSET_IN, -COPY_OFFSET_IN);
        }
        made.push(b.id);
        if let Some(p) = layout.page_mut(to) {
            p.boxes.push(b);
        }
    }
    made
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::BoxSource;
    use plan_docs::{Scale, SheetSize};

    fn page_with(rects: &[(f64, f64, f64, f64)]) -> Layout {
        let mut l = Layout::new("t", SheetSize::ArchC);
        let p = l.add_page(1, "A");
        for (i, r) in rects.iter().enumerate() {
            p.boxes.push(LayoutBox::new(
                i as u64 + 1,
                (Point::new(r.0, r.1), Point::new(r.2, r.3)),
                BoxSource::text("x", 10.0),
                Scale::QuarterInch,
            ));
        }
        l.add_page(2, "B");
        l
    }

    fn rect(l: &Layout, page: u32, id: Id) -> [f64; 4] {
        l.page(page)
            .unwrap()
            .boxes
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .bounds_in()
    }

    #[test]
    fn boxes_align_on_the_outermost_edge_of_the_selection() {
        let mut l = page_with(&[
            (1.0, 1.0, 3.0, 2.0),
            (4.0, 5.0, 8.0, 7.0),
            (2.0, 9.0, 3.0, 10.0),
        ]);
        let p = l.page_mut(1).unwrap();
        assert_eq!(align_boxes(p, &[1, 2, 3], AlignEdge::Left, None), 2);
        assert_eq!(rect(&l, 1, 2)[0], 1.0);
        assert_eq!(rect(&l, 1, 3)[0], 1.0);
        // Sizes are kept.
        assert_eq!(rect(&l, 1, 2)[2] - rect(&l, 1, 2)[0], 4.0);
        let p = l.page_mut(1).unwrap();
        align_boxes(p, &[1, 2, 3], AlignEdge::Right, None);
        for id in [1, 2, 3] {
            assert_eq!(rect(&l, 1, id)[2], 5.0, "box {id}");
        }
        let p = l.page_mut(1).unwrap();
        align_boxes(p, &[1, 2, 3], AlignEdge::Top, None);
        for id in [1, 2, 3] {
            assert_eq!(rect(&l, 1, id)[3], 10.0, "box {id}");
        }
        let p = l.page_mut(1).unwrap();
        align_boxes(p, &[1, 2, 3], AlignEdge::VCenter, None);
        let mids: Vec<f64> = [1, 2, 3]
            .iter()
            .map(|&i| (rect(&l, 1, i)[1] + rect(&l, 1, i)[3]) / 2.0)
            .collect();
        assert!(mids.iter().all(|m| (m - mids[0]).abs() < 1e-9), "{mids:?}");
    }

    #[test]
    fn a_single_box_aligns_to_the_drawing_area() {
        let mut l = page_with(&[(1.0, 1.0, 3.0, 2.0)]);
        let area = l.drawing_area();
        let to = [area.0.x, area.0.y, area.1.x, area.1.y];
        let p = l.page_mut(1).unwrap();
        align_boxes(p, &[1], AlignEdge::HCenter, Some(to));
        let r = rect(&l, 1, 1);
        assert!(((r[0] + r[2]) / 2.0 - (to[0] + to[2]) / 2.0).abs() < 1e-9);
        let p = l.page_mut(1).unwrap();
        align_boxes(p, &[1], AlignEdge::Bottom, Some(to));
        assert_eq!(rect(&l, 1, 1)[1], to[1]);
        // Nothing to move twice.
        let p = l.page_mut(1).unwrap();
        assert_eq!(align_boxes(p, &[1], AlignEdge::Bottom, Some(to)), 0);
        assert_eq!(align_boxes(p, &[], AlignEdge::Left, None), 0);
    }

    #[test]
    fn distributing_equalises_the_gaps_between_three_or_more() {
        let mut l = page_with(&[
            (0.0, 0.0, 2.0, 1.0),
            (3.0, 0.0, 4.0, 1.0),
            (4.5, 0.0, 6.0, 1.0),
            (10.0, 0.0, 12.0, 1.0),
        ]);
        let p = l.page_mut(1).unwrap();
        assert_eq!(distribute_boxes(p, &[1, 2, 3, 4], Spread::Horizontal), 2);
        let r: Vec<[f64; 4]> = (1..=4).map(|i| rect(&l, 1, i)).collect();
        // The ends stay.
        assert_eq!(r[0][0], 0.0);
        assert_eq!(r[3][2], 12.0);
        let gaps: Vec<f64> = (0..3).map(|i| r[i + 1][0] - r[i][2]).collect();
        assert!(gaps.iter().all(|g| (g - gaps[0]).abs() < 1e-9), "{gaps:?}");
        // Two boxes cannot be distributed.
        let p = l.page_mut(1).unwrap();
        assert_eq!(distribute_boxes(p, &[1, 2], Spread::Vertical), 0);
        // Vertically, in whatever order the ids come.
        let mut l = page_with(&[
            (0.0, 0.0, 1.0, 1.0),
            (0.0, 5.0, 1.0, 6.0),
            (0.0, 9.0, 1.0, 10.0),
        ]);
        let p = l.page_mut(1).unwrap();
        distribute_boxes(p, &[3, 1, 2], Spread::Vertical);
        assert!((rect(&l, 1, 2)[1] - 4.5).abs() < 1e-9);
    }

    #[test]
    fn copies_get_fresh_ids_and_land_on_the_chosen_page() {
        let mut l = page_with(&[(1.0, 1.0, 3.0, 2.0), (4.0, 5.0, 8.0, 7.0)]);
        let made = copy_boxes(&mut l, 1, &[1, 2], 2);
        assert_eq!(made, vec![3, 4]);
        let on_b = &l.page(2).unwrap().boxes;
        assert_eq!(on_b.len(), 2);
        assert_eq!(on_b[0].rect_in, l.page(1).unwrap().boxes[0].rect_in);
        assert_eq!(on_b[0].source, l.page(1).unwrap().boxes[0].source);
        // Same page: offset so the copy shows.
        let made = copy_boxes(&mut l, 1, &[1], 1);
        assert_eq!(made, vec![5]);
        let r = rect(&l, 1, 5);
        assert_eq!((r[0], r[3]), (1.0 + COPY_OFFSET_IN, 2.0 - COPY_OFFSET_IN));
        // Ids stay unique across the layout.
        let mut all: Vec<Id> = l
            .pages
            .iter()
            .flat_map(|p| p.boxes.iter().map(|b| b.id))
            .collect();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 5);
        // A missing page copies nothing.
        assert!(copy_boxes(&mut l, 1, &[1], 9).is_empty());
        assert!(copy_boxes(&mut l, 9, &[1], 1).is_empty());
    }
}
