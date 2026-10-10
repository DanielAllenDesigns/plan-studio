//! The wall system layers that draw their own lines (LAY-76, DECISIONS LS12):
//! "Walls, Through Wall Lines", "Footings", "Brick Ledge Lines",
//! "Walls, No Locate" and "Walls, Attic". Each is read with
//! `layers().get(NAME).is_some_and(|l| l.display)`, so a plan without the
//! layer draws nothing extra. Straight walls only.

use eframe::egui::{self, Color32, Shape, Stroke};
use plan_core::geometry::Point;
use plan_core::layers::{
    BRICK_LEDGE_LAYER, FOOTINGS_LAYER, WALL_ATTIC_LAYER, WALL_NO_LOCATE_LAYER,
    WALL_THROUGH_LINES_LAYER,
};
use plan_core::{Wall, WallKind};

use super::{Camera, EditorContext};

/// How far a footing reaches past each face of its foundation wall, inches.
pub const FOOTING_PROJECTION: f64 = 4.0;
/// How far inside the exterior face the brick ledge step sits, inches.
pub const LEDGE_SETBACK: f64 = 4.0;

/// One line to draw on a system layer.
#[derive(Debug, Clone, PartialEq)]
pub struct SysLine {
    pub layer: &'static str,
    pub points: Vec<Point>,
    pub dashed: bool,
}

fn outline(wall: &Wall, half: f64, grow_ends: f64) -> Vec<Point> {
    let (d, n) = (wall.direction(), wall.normal());
    let (a, b) = (
        wall.start.sub(d.scale(grow_ends)),
        wall.end.add(d.scale(grow_ends)),
    );
    vec![
        a.add(n.scale(half)),
        b.add(n.scale(half)),
        b.sub(n.scale(half)),
        a.sub(n.scale(half)),
        a.add(n.scale(half)),
    ]
}

/// The lines `wall` contributes on the system layers `shown` says are on.
pub fn lines(wall: &Wall, shown: impl Fn(&str) -> bool) -> Vec<SysLine> {
    let mut out = Vec::new();
    if wall.is_curved() || wall.length() < 1e-6 {
        return out;
    }
    let half = wall.thickness / 2.0;
    let n = wall.normal();
    if shown(WALL_THROUGH_LINES_LAYER) {
        let s = &wall.spec.structure;
        for (through, at) in [
            (s.through_at_start, wall.start),
            (s.through_at_end, wall.end),
        ] {
            if through {
                out.push(SysLine {
                    layer: WALL_THROUGH_LINES_LAYER,
                    points: vec![at.add(n.scale(half)), at.sub(n.scale(half))],
                    dashed: false,
                });
            }
        }
    }
    if wall.is_foundation() {
        if shown(FOOTINGS_LAYER) {
            out.push(SysLine {
                layer: FOOTINGS_LAYER,
                points: outline(wall, half + FOOTING_PROJECTION, 0.0),
                dashed: true,
            });
        }
        if shown(BRICK_LEDGE_LAYER) && wall.kind == WallKind::Exterior {
            let off = half - LEDGE_SETBACK.min(half * 0.5);
            out.push(SysLine {
                layer: BRICK_LEDGE_LAYER,
                points: vec![wall.start.add(n.scale(off)), wall.end.add(n.scale(off))],
                dashed: false,
            });
        }
    }
    if wall.flags.no_locate && shown(WALL_NO_LOCATE_LAYER) {
        out.push(SysLine {
            layer: WALL_NO_LOCATE_LAYER,
            points: outline(wall, half + 1.0, 1.0),
            dashed: true,
        });
    }
    if wall.flags.attic && shown(WALL_ATTIC_LAYER) {
        out.push(SysLine {
            layer: WALL_ATTIC_LAYER,
            points: outline(wall, half + 2.0, 2.0),
            dashed: true,
        });
    }
    out
}

/// Draws the system-layer lines of one wall.
pub fn draw(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, wall: &Wall) {
    let layers = cx.layers();
    let on = |name: &str| layers.get(name).is_some_and(|l| l.display);
    for l in lines(wall, on) {
        let [r, g, b] = layers.get(l.layer).map_or([0, 0, 0], |x| x.color);
        let stroke = Stroke::new(1.0_f32, Color32::from_rgb(r, g, b));
        let pts: Vec<egui::Pos2> = l.points.iter().map(|p| cam.world_to_screen(*p)).collect();
        if l.dashed {
            painter.extend(Shape::dashed_line(&pts, stroke, 5.0, 3.0));
        } else {
            painter.add(Shape::line(pts, stroke));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::WallKind;

    fn wall() -> Wall {
        let mut p = plan_core::Project::new("w");
        let id = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        p.floors[0].wall(id).unwrap().clone()
    }

    #[test]
    fn nothing_is_drawn_while_the_layers_are_off() {
        let mut w = wall();
        w.flags.no_locate = true;
        w.flags.attic = true;
        w.flags.foundation = true;
        w.spec.structure.through_at_end = true;
        assert!(lines(&w, |_| false).is_empty());
    }

    #[test]
    fn each_flag_draws_on_its_own_layer() {
        let mut w = wall();
        w.flags.no_locate = true;
        w.flags.attic = true;
        w.flags.foundation = true;
        w.spec.structure.through_at_start = true;
        let got: Vec<&str> = lines(&w, |_| true).iter().map(|l| l.layer).collect();
        for name in [
            WALL_THROUGH_LINES_LAYER,
            FOOTINGS_LAYER,
            BRICK_LEDGE_LAYER,
            WALL_NO_LOCATE_LAYER,
            WALL_ATTIC_LAYER,
        ] {
            assert!(got.contains(&name), "{name} missing from {got:?}");
        }
    }

    #[test]
    fn a_footing_reaches_past_both_faces() {
        let mut w = wall();
        w.flags.foundation = true;
        let l = lines(&w, |n| n == FOOTINGS_LAYER);
        assert_eq!(l.len(), 1);
        let ys: Vec<f64> = l[0].points.iter().map(|p| p.y.abs()).collect();
        assert!(ys
            .iter()
            .all(|y| (y - (w.thickness / 2.0 + FOOTING_PROJECTION)).abs() < 1e-6));
    }
}
