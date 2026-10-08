//! Edit handles of the selected object (S-11..S-28): a small model plus
//! hit-testing and drawing. The select tool interprets the drags.

use super::ops::cad_center;
use super::selection::{cad_by_id, ObjectRef, Selection};
use super::Camera;
use crate::theme::Palette;
use eframe::egui::{self, CursorIcon, Rect, Shape, Stroke, Vec2};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::{Floor, OpeningKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HandleKind {
    /// Free move of the whole object.
    Move,
    ResizeStart,
    ResizeEnd,
    /// Rotate about the object's center.
    Rotate,
    /// Move vertex `n` (polylines).
    Reshape(usize),
    /// Door swing (click flips it).
    Swing,
    /// Chief's signature wall move: perpendicular to the wall only. Also the
    /// line-offset handle of a dimension and the slide handle of an opening.
    PerpendicularMove,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Handle {
    pub kind: HandleKind,
    pub pos: Point,
    pub cursor: CursorIcon,
    pub target: ObjectRef,
}

/// Handles for a single selected object; none for an empty or multiple
/// selection. `scale` is pixels per inch (rotate handles sit a fixed number
/// of pixels off the object).
pub fn handles_for(floor: &Floor, selection: &Selection, scale: f64) -> Vec<Handle> {
    let Some(target) = selection.single() else {
        return Vec::new();
    };
    let h = |kind, pos, cursor| Handle {
        kind,
        pos,
        cursor,
        target,
    };
    match target {
        ObjectRef::Wall(id) => {
            let Some(w) = floor.wall(id) else {
                return Vec::new();
            };
            let resize = resize_cursor(w.end.sub(w.start));
            vec![
                h(HandleKind::ResizeStart, w.start, resize),
                h(HandleKind::ResizeEnd, w.end, resize),
                h(
                    HandleKind::PerpendicularMove,
                    Point::lerp(w.start, w.end, 0.5),
                    CursorIcon::Move,
                ),
            ]
        }
        ObjectRef::Opening(id) => {
            let Some(o) = floor.openings.iter().find(|o| o.id == id) else {
                return Vec::new();
            };
            let Some(w) = floor.wall(o.wall_id) else {
                return Vec::new();
            };
            let mut out = vec![h(
                HandleKind::PerpendicularMove,
                w.point_at(o.center_offset),
                resize_cursor(w.end.sub(w.start)),
            )];
            if o.kind == OpeningKind::Door {
                let hinge = if o.swing_flipped {
                    w.point_at(o.end_offset())
                } else {
                    w.point_at(o.start_offset())
                };
                let side = if o.swing_flipped {
                    w.normal() * -1.0
                } else {
                    w.normal()
                };
                out.push(h(
                    HandleKind::Swing,
                    hinge + side * o.width,
                    CursorIcon::PointingHand,
                ));
            }
            out
        }
        ObjectRef::Dimension(id) => {
            let Some(d) = floor.dimensions.iter().find(|d| d.id == id) else {
                return Vec::new();
            };
            let (a, b) = d.line_points();
            vec![h(
                HandleKind::PerpendicularMove,
                Point::lerp(a, b, 0.5),
                CursorIcon::Move,
            )]
        }
        ObjectRef::Cad(id) | ObjectRef::Text(id) => {
            let Some(c) = cad_by_id(floor, id) else {
                return Vec::new();
            };
            let (_, hi) = c.bounds();
            let center = cad_center(&c.item);
            let mut out = vec![
                h(HandleKind::Move, center, CursorIcon::Move),
                h(
                    HandleKind::Rotate,
                    Point::new(center.x, hi.y + 24.0 / scale.max(1e-6)),
                    CursorIcon::Grab,
                ),
            ];
            match &c.item {
                CadItem::Line { a, b } => {
                    out.push(h(HandleKind::ResizeStart, *a, CursorIcon::Crosshair));
                    out.push(h(HandleKind::ResizeEnd, *b, CursorIcon::Crosshair));
                }
                CadItem::Circle { center, radius } => out.push(h(
                    HandleKind::ResizeEnd,
                    Point::new(center.x + radius, center.y),
                    CursorIcon::ResizeHorizontal,
                )),
                CadItem::Polyline { points, .. } => {
                    for (i, p) in points.iter().enumerate() {
                        out.push(h(HandleKind::Reshape(i), *p, CursorIcon::Crosshair));
                    }
                }
                _ => {}
            }
            out
        }
        _ => Vec::new(),
    }
}

fn resize_cursor(v: Point) -> CursorIcon {
    let (ax, ay) = (v.x.abs(), v.y.abs());
    if ay < ax * 0.3827 {
        CursorIcon::ResizeHorizontal
    } else if ax < ay * 0.3827 {
        CursorIcon::ResizeVertical
    } else if v.x * v.y > 0.0 {
        CursorIcon::ResizeNeSw
    } else {
        CursorIcon::ResizeNwSe
    }
}

/// The handle under `p` (within `tol` inches); the nearest wins, end handles
/// before the middle one on ties.
pub fn hit_handle(handles: &[Handle], p: Point, tol: f64) -> Option<Handle> {
    handles
        .iter()
        .filter(|h| h.pos.dist(p) <= tol)
        .min_by(|a, b| {
            let da = a.pos.dist(p) + f64::from(a.kind == HandleKind::PerpendicularMove) * 1e-6;
            let db = b.pos.dist(p) + f64::from(b.kind == HandleKind::PerpendicularMove) * 1e-6;
            da.total_cmp(&db)
        })
        .copied()
}

/// Draws the handles with a fixed screen size.
pub fn draw(handles: &[Handle], painter: &egui::Painter, cam: &Camera, pal: &Palette) {
    let stroke = Stroke::new(1.5_f32, pal.selection);
    for h in handles {
        let c = cam.world_to_screen(h.pos);
        match h.kind {
            HandleKind::ResizeStart | HandleKind::ResizeEnd | HandleKind::Reshape(_) => {
                let r = Rect::from_center_size(c, Vec2::splat(9.0));
                painter.add(Shape::rect_filled(r, 0.0, pal.background));
                painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
            }
            HandleKind::Rotate => {
                painter.circle_filled(c, 5.0, pal.background);
                painter.circle_stroke(c, 5.0, stroke);
            }
            HandleKind::Swing => {
                painter.circle_filled(c, 4.5, pal.selection);
            }
            HandleKind::Move | HandleKind::PerpendicularMove => {
                let r = 6.0;
                painter.add(Shape::convex_polygon(
                    vec![
                        c + Vec2::new(0.0, -r),
                        c + Vec2::new(r, 0.0),
                        c + Vec2::new(0.0, r),
                        c + Vec2::new(-r, 0.0),
                    ],
                    pal.background,
                    stroke,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{Project, WallKind};

    #[test]
    fn wall_handles_and_hit_priority() {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            6.0,
            100.0,
            WallKind::Interior,
        );
        let mut sel = Selection::default();
        assert!(handles_for(&p.floors[0], &sel, 2.0).is_empty());
        sel.set(ObjectRef::Wall(w));
        let hs = handles_for(&p.floors[0], &sel, 2.0);
        assert_eq!(hs.len(), 3);
        let hit = hit_handle(&hs, Point::new(98.0, 1.0), 5.0).unwrap();
        assert_eq!(hit.kind, HandleKind::ResizeEnd);
        let mid = hit_handle(&hs, Point::new(51.0, 1.0), 5.0).unwrap();
        assert_eq!(mid.kind, HandleKind::PerpendicularMove);
        assert!(hit_handle(&hs, Point::new(30.0, 30.0), 5.0).is_none());
    }
}
