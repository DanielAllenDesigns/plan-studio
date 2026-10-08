//! Wall Detail: a 2D framing elevation for one wall.

use crate::member::{dot, Member};
use plan_core::{Point, Wall};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Text height of member labels, inches.
const LABEL_HEIGHT: f64 = 2.0;

/// A 2D drawing primitive in the elevation frame (X along the wall from its
/// start, Y up from the bottom of the wall).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stroke {
    Line(Point, Point),
    Rect {
        min: Point,
        max: Point,
    },
    Text {
        pos: Point,
        text: String,
        height: f64,
    },
}

/// Draw the framing elevation of `wall` from its `members`.
///
/// Emits the wall's bottom and top lines, a rectangle per member (members of
/// other walls are ignored) and a label at each member's centre. Repeated
/// vertical members with the same kind and label (studs, cripples) are
/// labelled once to keep the drawing readable. The wall bottom is taken as
/// the lowest point of the wall's members.
pub fn wall_detail(wall: &Wall, members: &[Member]) -> Vec<Stroke> {
    let (d, start) = (wall.direction(), wall.start);
    let dir3 = [d.x, 0.0, -d.y];
    let start3 = [start.x, 0.0, -start.y];
    let mine: Vec<&Member> = members
        .iter()
        .filter(|m| m.wall_id == Some(wall.id))
        .collect();
    let base = mine
        .iter()
        .flat_map(|m| m.corners())
        .map(|c| c[1])
        .fold(f64::INFINITY, f64::min);
    let len = wall.length();
    let mut out = Vec::new();
    if base.is_finite() {
        out.push(Stroke::Line(Point::new(0.0, 0.0), Point::new(len, 0.0)));
        out.push(Stroke::Line(
            Point::new(0.0, wall.height),
            Point::new(len, wall.height),
        ));
    }
    let mut labelled = HashSet::new();
    for m in mine {
        let pts: Vec<Point> = m
            .corners()
            .iter()
            .map(|c| {
                let rel = [c[0] - start3[0], 0.0, c[2] - start3[2]];
                Point::new(dot(rel, dir3), c[1] - base)
            })
            .collect();
        let min = pts
            .iter()
            .fold(Point::new(f64::INFINITY, f64::INFINITY), |a, p| {
                Point::new(a.x.min(p.x), a.y.min(p.y))
            });
        let max = pts
            .iter()
            .fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| {
                Point::new(a.x.max(p.x), a.y.max(p.y))
            });
        out.push(Stroke::Rect { min, max });
        let vertical = max.y - min.y > max.x - min.x;
        if !vertical || labelled.insert((m.kind, m.label.clone())) {
            out.push(Stroke::Text {
                pos: Point::lerp(min, max, 0.5),
                text: m.label.clone(),
                height: LABEL_HEIGHT,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{frame_wall, FramingDefaults, MemberKind};
    use plan_core::{Opening, WallKind};

    #[test]
    fn elevation_rects_match_members() {
        let wall = Wall {
            id: 3,
            start: Point::new(10.0, 20.0),
            end: Point::new(10.0, 140.0),
            thickness: 6.5,
            height: 109.125,
            kind: WallKind::Exterior,
            layer: "Walls, Normal".into(),
            ..Default::default()
        };
        let door = Opening::default_door(1, wall.id, 60.0);
        let m = frame_wall(&wall, &[&door], 0.0, &FramingDefaults::default());
        let strokes = wall_detail(&wall, &m);
        let rects: Vec<_> = strokes
            .iter()
            .filter_map(|s| match s {
                Stroke::Rect { min, max } => Some((*min, *max)),
                _ => None,
            })
            .collect();
        assert_eq!(rects.len(), m.len());
        // Everything lies inside the wall's elevation outline.
        for (lo, hi) in &rects {
            assert!(lo.x >= -1e-9 && hi.x <= 120.0 + 1e-9);
            assert!(lo.y >= -1e-9 && hi.y <= 109.125 + 1e-9);
        }
        // The first member is the bottom plate: full length, 1 1/2" tall.
        assert_eq!(m[0].kind, MemberKind::BottomPlate);
        assert!((rects[0].1.x - rects[0].0.x - 120.0).abs() < 1e-9);
        assert!((rects[0].1.y - 1.5).abs() < 1e-9);
        let texts = strokes
            .iter()
            .filter(|s| matches!(s, Stroke::Text { .. }))
            .count();
        assert!(texts > 0 && texts < m.len());
        // Members of other walls are ignored.
        let mut other = wall.clone();
        other.id = 99;
        assert!(wall_detail(&other, &m).is_empty());
    }
}
