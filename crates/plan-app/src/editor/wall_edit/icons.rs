//! The notification icons of walls in plan view (W-132): the Off Angle
//! pointer with its Caution symbol on a wall a little off an allowed angle,
//! and the Caution symbol with the Connect Walls mark on a loose wall end.
//! Right-clicking an icon selects its wall and opens the usual menu, whose
//! Edit-toolbar entries carry Fix Off Angle Wall, Ignore, Ignore All,
//! Ignore Unconnected Wall and Delete.

use super::{angle_rules, EditorContext};
use crate::editor::camera::Camera;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Shape, Stroke};
use plan_core::geometry::Point;
use plan_core::{Id, WallEnd};

/// What an icon reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IconKind {
    /// The wall is a little off an allowed angle; the target angle is stored.
    OffAngle(f64),
    /// This end touches no other wall.
    Unconnected(WallEnd),
}

/// One icon in the plan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallIcon {
    pub wall: Id,
    pub kind: IconKind,
    /// World position of the icon's centre.
    pub at: Point,
}

/// Distance of an icon from its wall, in screen pixels.
const OFFSET_PX: f64 = 16.0;
/// Radius an icon answers a click in, screen pixels.
const HIT_PX: f64 = 11.0;

/// Every icon of the active floor, at the current zoom.
pub fn icons(cx: &EditorContext) -> Vec<WallIcon> {
    let scale = cx.px_per_in.max(1e-6);
    let (allowed, inc) = angle_rules(cx);
    let floor = cx.floor;
    let mut v = Vec::new();
    for (id, target) in cx.project.off_angle_walls(floor, &allowed, inc) {
        let Some(w) = cx.floor().wall(id) else {
            continue;
        };
        let mid = plan_core::geometry::Point::lerp(w.start, w.end, 0.5);
        let off = w.thickness * 0.5 + OFFSET_PX / scale;
        v.push(WallIcon {
            wall: id,
            kind: IconKind::OffAngle(target),
            at: mid.add(w.normal().scale(off)),
        });
    }
    for (id, end) in cx.project.unconnected_walls(floor) {
        let Some(w) = cx.floor().wall(id) else {
            continue;
        };
        let (p, away) = match end {
            WallEnd::Start => (w.start, w.direction().scale(-1.0)),
            WallEnd::End => (w.end, w.direction()),
        };
        v.push(WallIcon {
            wall: id,
            kind: IconKind::Unconnected(end),
            at: p.add(away.scale(OFFSET_PX / scale)),
        });
    }
    v
}

/// The icon under a world point, when there is one.
pub fn icon_at(cx: &EditorContext, world: Point) -> Option<WallIcon> {
    let tol = HIT_PX / cx.px_per_in.max(1e-6);
    icons(cx)
        .into_iter()
        .filter(|i| i.at.dist(world) <= tol)
        .min_by(|a, b| a.at.dist(world).total_cmp(&b.at.dist(world)))
}

/// Right click in Select Objects: an icon under the pointer selects its wall,
/// so the menu that opens is the wall's.
pub fn select_for_icon(cx: &mut EditorContext, world: Point) -> bool {
    match icon_at(cx, world) {
        Some(i) => {
            cx.selection.items = vec![crate::editor::selection::ObjectRef::Wall(i.wall)];
            true
        }
        None => false,
    }
}

const AMBER: Color32 = Color32::from_rgb(232, 168, 24);
const INK: Color32 = Color32::from_rgb(40, 32, 8);

fn caution(painter: &egui::Painter, c: Pos2, r: f32) {
    let pts = vec![
        Pos2::new(c.x, c.y - r),
        Pos2::new(c.x + r * 0.95, c.y + r * 0.7),
        Pos2::new(c.x - r * 0.95, c.y + r * 0.7),
    ];
    painter.add(Shape::convex_polygon(pts, AMBER, Stroke::new(1.0_f32, INK)));
    painter.line_segment(
        [Pos2::new(c.x, c.y - r * 0.4), Pos2::new(c.x, c.y + r * 0.2)],
        Stroke::new(1.4_f32, INK),
    );
    painter.circle_filled(Pos2::new(c.x, c.y + r * 0.45), 0.9, INK);
}

fn pointer(painter: &egui::Painter, c: Pos2, wall_dir: f32, target_dir: f32, r: f32) {
    painter.circle_filled(c, r, Color32::WHITE);
    painter.circle_stroke(c, r, Stroke::new(1.2_f32, INK));
    // The wall's own direction, and where it should point.
    let tip = |a: f32, len: f32| Pos2::new(c.x + a.cos() * len, c.y + a.sin() * len);
    painter.line_segment(
        [
            tip(wall_dir + std::f32::consts::PI, r * 0.7),
            tip(wall_dir, r * 0.7),
        ],
        Stroke::new(1.4_f32, Color32::from_rgb(200, 40, 40)),
    );
    painter.line_segment(
        [
            tip(target_dir + std::f32::consts::PI, r * 0.7),
            tip(target_dir, r * 0.7),
        ],
        Stroke::new(1.4_f32, Color32::from_rgb(40, 140, 60)),
    );
}

fn connect_mark(painter: &egui::Painter, c: Pos2, r: f32) {
    painter.circle_filled(c, r, Color32::WHITE);
    painter.circle_stroke(c, r, Stroke::new(1.2_f32, INK));
    let s = Stroke::new(1.6_f32, Color32::from_rgb(40, 100, 200));
    painter.line_segment(
        [
            Pos2::new(c.x - r * 0.7, c.y),
            Pos2::new(c.x - r * 0.15, c.y),
        ],
        s,
    );
    painter.line_segment(
        [
            Pos2::new(c.x + r * 0.15, c.y),
            Pos2::new(c.x + r * 0.7, c.y),
        ],
        s,
    );
    painter.text(
        Pos2::new(c.x, c.y + r * 0.1),
        Align2::CENTER_CENTER,
        "+",
        FontId::proportional(r * 1.1),
        Color32::from_rgb(40, 100, 200),
    );
}

/// Draws the icons of the active floor over the plan (editor canvas only).
pub fn draw(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    for i in icons(cx) {
        let c = cam.world_to_screen(i.at);
        match i.kind {
            IconKind::OffAngle(target) => {
                let Some(w) = cx.floor().wall(i.wall) else {
                    continue;
                };
                // Screen y runs down: the angles flip with it.
                let wd = -(plan_core::wall_repair::wall_angle_deg(w).to_radians() as f32);
                let td = -(target.to_radians() as f32);
                pointer(painter, Pos2::new(c.x - 7.0, c.y), wd, td, 7.5);
                caution(painter, Pos2::new(c.x + 9.0, c.y), 7.5);
            }
            IconKind::Unconnected(_) => {
                caution(painter, Pos2::new(c.x - 7.0, c.y), 7.5);
                connect_mark(painter, Pos2::new(c.x + 9.0, c.y), 7.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{PlanDefaults, WallKind};

    fn cx() -> EditorContext {
        let mut cx = EditorContext::new(PlanDefaults::default());
        cx.px_per_in = 2.0;
        cx
    }

    #[test]
    fn a_slightly_turned_wall_and_a_loose_end_each_get_an_icon() {
        let mut cx = cx();
        // 2 degrees off horizontal, nothing touching it.
        let end = Point::new(
            120.0 * 2f64.to_radians().cos(),
            120.0 * 2f64.to_radians().sin(),
        );
        let w = cx
            .project
            .add_wall(0, Point::new(0.0, 0.0), end, 6.0, 96.0, WallKind::Interior);
        let v = icons(&cx);
        assert!(v.iter().any(|i| matches!(i.kind, IconKind::OffAngle(_))));
        assert_eq!(
            v.iter()
                .filter(|i| matches!(i.kind, IconKind::Unconnected(_)))
                .count(),
            2
        );
        // The pointer hits the icon, the empty plan does not.
        let off = v
            .iter()
            .find(|i| matches!(i.kind, IconKind::OffAngle(_)))
            .copied()
            .unwrap();
        assert_eq!(icon_at(&cx, off.at).map(|i| i.wall), Some(w));
        assert!(icon_at(&cx, Point::new(500.0, 500.0)).is_none());
        assert!(select_for_icon(&mut cx, off.at));
        assert_eq!(
            cx.selection.single(),
            Some(crate::editor::selection::ObjectRef::Wall(w))
        );
    }

    #[test]
    fn ignored_icons_do_not_draw() {
        let mut cx = cx();
        let end = Point::new(
            120.0 * 2f64.to_radians().cos(),
            120.0 * 2f64.to_radians().sin(),
        );
        let w = cx
            .project
            .add_wall(0, Point::new(0.0, 0.0), end, 6.0, 96.0, WallKind::Interior);
        cx.project.ignore_wall_icons(0, Some(&[w]), true, true);
        assert!(icons(&cx).is_empty());
    }
}
