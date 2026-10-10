//! Cross Section Lines (manual p. 1167, C-130): CAD lines that stand where a
//! section plane cuts an object, so a dimension drawn to a cut wall locates
//! something that can be found again when the view is redrawn.
//!
//! The lines are read off the section-cut regions of a [`Drawing`]. Every
//! object cut in one piece of the cutting plane gets four lines, the edges of
//! the box around everything that was cut of it: the left face, the right
//! face, the bottom and the top. Their numbering is the `edge` of
//! `plan_core::camera_view::CutRef`: 0 left, 1 right, 2 bottom, 3 top, plus 4
//! for every piece of a stepped plane after the first.

use crate::drawing::{Drawing, RegionKind};
use plan_core::geometry::Point;
use plan_core::Id;

/// One Cross Section Line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutLine {
    /// The object that was cut.
    pub object: Id,
    /// 0 left face, 1 right face, 2 bottom, 3 top; plus 4 per piece after
    /// the first.
    pub edge: u8,
    pub a: Point,
    pub b: Point,
}

impl CutLine {
    /// Does the line stand upright (a face) rather than lie level?
    pub fn is_vertical(&self) -> bool {
        self.edge % 4 < 2
    }

    /// Where the line is: its `x` for a face, its `y` for a level edge.
    pub fn position(&self) -> f64 {
        if self.is_vertical() {
            self.a.x
        } else {
            self.a.y
        }
    }
}

/// The Cross Section Lines of `drawing`. `breaks` are the positions of the
/// breaks of a stepped cutting plane along the drawing's X (empty for a
/// straight plane); an object cut in two pieces gets lines for each.
pub fn cross_section_lines(drawing: &Drawing, breaks: &[f64]) -> Vec<CutLine> {
    // (object, piece) -> bounding box of everything cut.
    let mut boxes: Vec<(Id, usize, Point, Point)> = Vec::new();
    for r in drawing
        .regions
        .iter()
        .filter(|r| r.kind == RegionKind::Cut && r.polygon.len() >= 3)
    {
        let Some(object) = r.object_id else {
            continue;
        };
        let (mut lo, mut hi) = (r.polygon[0], r.polygon[0]);
        for p in &r.polygon {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        let piece = breaks
            .iter()
            .take_while(|b| **b <= (lo.x + hi.x) * 0.5)
            .count();
        match boxes
            .iter_mut()
            .find(|(o, pc, ..)| *o == object && *pc == piece)
        {
            Some((_, _, l, h)) => {
                *l = Point::new(l.x.min(lo.x), l.y.min(lo.y));
                *h = Point::new(h.x.max(hi.x), h.y.max(hi.y));
            }
            None => boxes.push((object, piece, lo, hi)),
        }
    }
    let mut out = Vec::with_capacity(boxes.len() * 4);
    for (object, piece, lo, hi) in boxes {
        let base = u8::try_from(piece.saturating_mul(4)).unwrap_or(0);
        let edges = [
            (Point::new(lo.x, lo.y), Point::new(lo.x, hi.y)),
            (Point::new(hi.x, lo.y), Point::new(hi.x, hi.y)),
            (Point::new(lo.x, lo.y), Point::new(hi.x, lo.y)),
            (Point::new(lo.x, hi.y), Point::new(hi.x, hi.y)),
        ];
        for (i, (a, b)) in edges.into_iter().enumerate() {
            out.push(CutLine {
                object,
                edge: base.saturating_add(i as u8),
                a,
                b,
            });
        }
    }
    out.sort_by_key(|l| (l.object, l.edge));
    out
}

/// The line `object`/`edge` stands on now, if the object is still cut.
pub fn find_cut(lines: &[CutLine], object: Id, edge: u8) -> Option<&CutLine> {
    lines.iter().find(|l| l.object == object && l.edge == edge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::Region;
    use plan_3d::Material;

    fn cut(object: Id, x0: f64, x1: f64, y0: f64, y1: f64) -> Region {
        Region {
            polygon: vec![
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ],
            material: Material::ALL[0],
            object_id: Some(object),
            kind: RegionKind::Cut,
        }
    }

    fn drawing(regions: Vec<Region>) -> Drawing {
        Drawing {
            regions,
            ..Drawing::default()
        }
    }

    #[test]
    fn a_cut_wall_gets_a_line_on_each_face_top_and_bottom() {
        let d = drawing(vec![cut(7, 10.0, 15.5, 0.0, 96.0)]);
        let lines = cross_section_lines(&d, &[]);
        assert_eq!(lines.len(), 4);
        assert_eq!(find_cut(&lines, 7, 0).unwrap().position(), 10.0);
        assert_eq!(find_cut(&lines, 7, 1).unwrap().position(), 15.5);
        assert_eq!(find_cut(&lines, 7, 2).unwrap().position(), 0.0);
        assert_eq!(find_cut(&lines, 7, 3).unwrap().position(), 96.0);
        assert!(find_cut(&lines, 7, 0).unwrap().is_vertical());
        assert!(!find_cut(&lines, 7, 3).unwrap().is_vertical());
    }

    #[test]
    fn the_pieces_of_a_wall_cut_around_an_opening_share_one_box() {
        let d = drawing(vec![
            cut(3, 0.0, 6.0, 0.0, 30.0),
            cut(3, 0.0, 6.0, 80.0, 96.0),
        ]);
        let lines = cross_section_lines(&d, &[]);
        assert_eq!(lines.len(), 4);
        assert_eq!(find_cut(&lines, 3, 3).unwrap().position(), 96.0);
        assert_eq!(find_cut(&lines, 3, 2).unwrap().position(), 0.0);
    }

    #[test]
    fn a_stepped_plane_numbers_the_edges_by_piece() {
        let d = drawing(vec![cut(1, -50.0, -40.0, 0.0, 96.0), cut(2, 40.0, 50.0, 0.0, 96.0)]);
        let lines = cross_section_lines(&d, &[0.0]);
        assert!(find_cut(&lines, 1, 0).is_some());
        assert!(find_cut(&lines, 2, 4).is_some());
        assert!(find_cut(&lines, 2, 0).is_none());
    }

    #[test]
    fn faces_without_an_object_and_shadows_make_no_lines() {
        let mut r = cut(1, 0.0, 5.0, 0.0, 9.0);
        r.object_id = None;
        let mut s = cut(2, 0.0, 5.0, 0.0, 9.0);
        s.kind = RegionKind::Shadow;
        assert!(cross_section_lines(&drawing(vec![r, s]), &[]).is_empty());
    }
}
