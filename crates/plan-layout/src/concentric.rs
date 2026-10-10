//! Concentric copies of a page rectangle (Layout CAD > Concentric): the
//! border of a title block is drawn once and the inner borders are copies
//! inset by a distance, 1/8 in and 1/4 in in the lesson.

use crate::model::LayoutPage;
use plan_core::{CadItem, Id, Point};

/// The `[x0, y0, x1, y1]` of a closed four-point polyline that is an
/// axis-aligned rectangle.
fn rect_of(item: &CadItem) -> Option<[f64; 4]> {
    let CadItem::Polyline {
        points,
        closed: true,
    } = item
    else {
        return None;
    };
    if points.len() != 4 {
        return None;
    }
    let (x0, x1) = min_max(points.iter().map(|p| p.x));
    let (y0, y1) = min_max(points.iter().map(|p| p.y));
    let at = |x: f64, y: f64| {
        points
            .iter()
            .any(|p| (p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9)
    };
    (at(x0, y0) && at(x1, y0) && at(x1, y1) && at(x0, y1)).then_some([x0, y0, x1, y1])
}

fn min_max(v: impl Iterator<Item = f64>) -> (f64, f64) {
    v.fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
}

impl LayoutPage {
    /// A concentric copy of the rectangle `id`, `inset` paper inches inside
    /// it (negative: outside), keeping its layer and look. `None` when `id`
    /// is not a rectangle or the copy would have no size.
    pub fn concentric_copy(&mut self, id: Id, inset: f64) -> Option<Id> {
        let src = self.cad.iter().find(|o| o.id == id)?.clone();
        let [x0, y0, x1, y1] = rect_of(&src.item)?;
        let (a, b) = (
            Point::new(x0 + inset, y0 + inset),
            Point::new(x1 - inset, y1 - inset),
        );
        if b.x - a.x < 1e-9 || b.y - a.y < 1e-9 {
            return None;
        }
        let mut copy = src;
        copy.id = self.next_cad_id();
        copy.item = CadItem::Polyline {
            points: vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)],
            closed: true,
        };
        let id = copy.id;
        self.cad.push(copy);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Layout;
    use plan_docs::SheetSize;

    #[test]
    fn concentric_copies_step_in_by_the_inset_and_keep_their_layer() {
        let mut l = Layout::new("t", SheetSize::ArchD);
        let p = l.add_page(1, "Border");
        let outer = p.add_rect(Point::new(0.5, 0.5), Point::new(35.5, 23.5));
        let a = p.concentric_copy(outer, 0.125).unwrap();
        let b = p.concentric_copy(outer, 0.25).unwrap();
        let get =
            |p: &LayoutPage, id| rect_of(&p.cad.iter().find(|o| o.id == id).unwrap().item).unwrap();
        assert_eq!(get(p, a), [0.625, 0.625, 35.375, 23.375]);
        assert_eq!(get(p, b), [0.75, 0.75, 35.25, 23.25]);
        assert_eq!(p.cad.len(), 3);
        // Outward, and a line or a collapsed copy is refused.
        let out = p.concentric_copy(outer, -0.25).unwrap();
        assert_eq!(get(p, out), [0.25, 0.25, 35.75, 23.75]);
        assert!(p.concentric_copy(outer, 20.0).is_none());
        let line = p.add_line(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
        assert!(p.concentric_copy(line, 0.1).is_none());
    }
}
