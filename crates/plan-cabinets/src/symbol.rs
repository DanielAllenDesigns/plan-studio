//! The 2D plan symbol for a cabinet.

use plan_core::geometry::{polygon_centroid, Point};
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

use crate::cabinet::{Cabinet, CabinetKind, CornerStyle};
use crate::face::FaceItem;
use crate::geom;
use crate::top::{CustomTop, CutoutKind, EdgeProfile};

/// Inset of the front-face line and the wall-cabinet inner outline, inches.
const INSET: f64 = 0.75;
/// Height of the label text, inches.
const LABEL_HEIGHT: f64 = 3.0;
/// How far an open drawer is drawn pulled out, at most, inches.
const DRAWER_OUT: f64 = 12.0;

/// A plan-view drawing primitive in plan coordinates (inches).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stroke {
    Line(Point, Point),
    /// A polyline; the flag closes it back to the first point.
    Polyline(Vec<Point>, bool),
    /// A circular arc counter-clockwise from `start` to `end` radians.
    Arc {
        center: Point,
        radius: f64,
        start: f64,
        end: f64,
    },
    Text {
        at: Point,
        text: String,
        height: f64,
        /// Baseline rotation in radians.
        angle: f64,
    },
}

/// Draw the cabinet in plan.
///
/// Every cabinet gets its footprint outline (stroke 0) and a centred label
/// ([`Cabinet::display_label`]). Cabinets with a countertop add the countertop
/// outline as stroke 1 (Chief shows it dashed; here it is a second solid
/// outline). Base and tall cabinets add a front-face line 3/4" in from the
/// front; wall cabinets add an inner offset outline and a diagonal cross.
/// Corner cabinets follow their L or diagonal front (a lazy susan adds its
/// circle), blind cabinets mark the hidden end, appliance openings are
/// crossed, custom tops show their edge line, and every sink or cooktop
/// cutout is drawn with its fixture. With [`Cabinet::indicators`] on, door
/// swing arcs and pulled-out drawers show the fronts.
pub fn plan_symbol(cabinet: &Cabinet) -> Vec<Stroke> {
    let (w, d) = (cabinet.width, cabinet.depth);
    let p = |x: f64, y: f64| cabinet.to_plan(Point::new(x, y));
    let q = |pt: Point| cabinet.to_plan(pt);
    let mut out = vec![Stroke::Polyline(cabinet.footprint(), true)];

    if cabinet.countertop.is_some() {
        if let Some(top) = cabinet.top_polygon() {
            out.push(Stroke::Polyline(top, true));
        }
    }

    match cabinet.kind {
        CabinetKind::Base
        | CabinetKind::FullHeight
        | CabinetKind::BaseFiller
        | CabinetKind::FullHeightFiller
        | CabinetKind::BlindBase => {
            if cabinet.appliance.is_some() {
                out.push(Stroke::Line(p(0.0, 0.0), p(w, d)));
                out.push(Stroke::Line(p(w, 0.0), p(0.0, d)));
            } else {
                out.push(Stroke::Line(p(0.0, d - INSET), p(w, d - INSET)));
            }
            blind_mark(cabinet, &mut out);
        }
        CabinetKind::Wall | CabinetKind::WallFiller | CabinetKind::BlindWall => {
            let i = INSET.min(w / 2.0).min(d / 2.0);
            out.push(Stroke::Polyline(
                vec![p(i, i), p(w - i, i), p(w - i, d - i), p(i, d - i)],
                true,
            ));
            out.push(Stroke::Line(p(0.0, 0.0), p(w, d)));
            out.push(Stroke::Line(p(w, 0.0), p(0.0, d)));
            blind_mark(cabinet, &mut out);
        }
        CabinetKind::CornerBase | CabinetKind::CornerWall => corner_strokes(cabinet, &mut out),
        CabinetKind::CustomCountertop => {
            if let Some(c) = &cabinet.custom {
                edge_line(cabinet, c, &mut out);
            }
        }
        CabinetKind::Soffit
        | CabinetKind::Shelf
        | CabinetKind::Partition
        | CabinetKind::CustomBacksplash
        | CabinetKind::CounterHole => {}
    }

    for cut in &cabinet.cutouts {
        out.push(Stroke::Polyline(
            cut.outline.iter().map(|pt| q(*pt)).collect(),
            true,
        ));
        fixture_strokes(cabinet, cut.kind, &cut.outline, &mut out);
    }

    if cabinet.indicators && !cabinet.kind.is_corner() && !cabinet.kind.is_custom() {
        opening_indicators(cabinet, &mut out);
    }

    let local = cabinet.footprint_local();
    let mut angle = cabinet.angle;
    if w < 8.0 && d > w * 2.0 && !cabinet.kind.is_custom() {
        angle += FRAC_PI_2;
    }
    out.push(Stroke::Text {
        at: q(label_point(&local, cabinet)),
        text: cabinet.display_label(),
        height: LABEL_HEIGHT,
        angle,
    });
    out
}

/// Where the label goes: the centre of a rectangle, otherwise the centroid.
fn label_point(local: &[Point], cabinet: &Cabinet) -> Point {
    if cabinet.kind.is_corner() || cabinet.kind.is_custom() {
        polygon_centroid(local)
    } else {
        Point::new(cabinet.width / 2.0, cabinet.depth / 2.0)
    }
}

/// A line across the cabinet where the blind end meets the visible face.
fn blind_mark(cabinet: &Cabinet, out: &mut Vec<Stroke>) {
    let Some(b) = cabinet.blind else { return };
    let x = match b.side {
        crate::cabinet::BlindSide::Left => b.blind_width,
        crate::cabinet::BlindSide::Right => cabinet.width - b.blind_width,
    };
    out.push(Stroke::Line(
        cabinet.to_plan(Point::new(x, 0.0)),
        cabinet.to_plan(Point::new(x, cabinet.depth)),
    ));
}

/// Front lines of a corner cabinet and the lazy susan circle.
fn corner_strokes(cabinet: &Cabinet, out: &mut Vec<Stroke>) {
    let (w, d) = (cabinet.width, cabinet.depth);
    let spec = cabinet.corner.unwrap_or_default();
    let a = spec.arm_depth.clamp(1.0, w.min(d));
    let q = |x: f64, y: f64| cabinet.to_plan(Point::new(x, y));
    match spec.style {
        CornerStyle::PieCut => {
            out.push(Stroke::Line(q(a, a - INSET), q(w, a - INSET)));
            out.push(Stroke::Line(q(a - INSET, a), q(a - INSET, d)));
            if spec.lazy_susan {
                let r = (a / 2.0 - 1.5).max(1.0);
                out.push(Stroke::Arc {
                    center: q(a / 2.0, a / 2.0),
                    radius: r,
                    start: 0.0,
                    end: TAU,
                });
            }
        }
        CornerStyle::Diagonal => {
            let n = Point::new(d - a, w - a).normalized();
            let back = n.scale(-INSET);
            out.push(Stroke::Line(
                cabinet.to_plan(Point::new(w, a).add(back)),
                cabinet.to_plan(Point::new(a, d).add(back)),
            ));
        }
    }
    if cabinet.kind == CabinetKind::CornerWall {
        out.push(Stroke::Line(q(0.0, 0.0), q(a, a)));
    }
}

/// The inset edge line of a custom countertop with a bevel or bullnose.
fn edge_line(cabinet: &Cabinet, custom: &CustomTop, out: &mut Vec<Stroke>) {
    if custom.edge == EdgeProfile::Square || custom.edge_size <= 0.0 {
        return;
    }
    let inner = geom::offset_ring(&custom.outline, -custom.edge_size);
    if inner.len() >= 3 {
        out.push(Stroke::Polyline(
            inner.into_iter().map(|pt| cabinet.to_plan(pt)).collect(),
            true,
        ));
    }
}

/// The fixture drawn inside a countertop hole.
fn fixture_strokes(cabinet: &Cabinet, kind: CutoutKind, hole: &[Point], out: &mut Vec<Stroke>) {
    let Some((lo, hi)) = geom::bbox(hole) else {
        return;
    };
    let (cw, ch) = (hi.x - lo.x, hi.y - lo.y);
    let centre = Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
    let q = |pt: Point| cabinet.to_plan(pt);
    match kind {
        CutoutKind::Sink => {
            let b = 1.5_f64.min(cw / 4.0).min(ch / 4.0);
            out.push(Stroke::Polyline(
                vec![
                    q(Point::new(lo.x + b, lo.y + b)),
                    q(Point::new(hi.x - b, lo.y + b)),
                    q(Point::new(hi.x - b, hi.y - b)),
                    q(Point::new(lo.x + b, hi.y - b)),
                ],
                true,
            ));
            out.push(Stroke::Arc {
                center: q(Point::new(centre.x, lo.y + b / 2.0 + 0.5)),
                radius: 0.5,
                start: 0.0,
                end: TAU,
            });
        }
        CutoutKind::Cooktop => {
            let r = (cw.min(ch) / 8.0).max(0.5);
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                out.push(Stroke::Arc {
                    center: q(Point::new(
                        centre.x + sx * cw / 4.0,
                        centre.y + sy * ch / 4.0,
                    )),
                    radius: r,
                    start: 0.0,
                    end: TAU,
                });
            }
        }
        CutoutKind::Custom => {}
    }
}

/// Door swing arcs and pulled-out drawers (Chief's Opening Indicators).
fn opening_indicators(cabinet: &Cabinet, out: &mut Vec<Stroke>) {
    let (w, d) = (cabinet.width, cabinet.depth);
    let fw = if cabinet.framed {
        cabinet.face.frame_width
    } else {
        0.0
    };
    let x_off = cabinet.blind.map_or(0.0, |b| match b.side {
        crate::cabinet::BlindSide::Left => b.blind_width,
        crate::cabinet::BlindSide::Right => 0.0,
    }) + fw;
    let face_w = cabinet.face_width() - 2.0 * fw;
    if face_w <= 0.0 {
        return;
    }
    let Ok(items) = cabinet.face.resolve(cabinet.face_height(), face_w) else {
        return;
    };
    let q = |x: f64, y: f64| cabinet.to_plan(Point::new(x, y));
    let seen: std::cell::RefCell<Vec<(i64, i64, u8)>> = std::cell::RefCell::new(Vec::new());
    let fresh = |x0: f64, x1: f64, tag: u8| {
        let key = (
            (x0 * 100.0).round() as i64,
            (x1 * 100.0).round() as i64,
            tag,
        );
        let mut seen = seen.borrow_mut();
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    };
    let swing = |x0: f64, x1: f64, left: bool, out: &mut Vec<Stroke>| {
        let r = x1 - x0;
        if r <= 0.0 || !fresh(x0, x1, u8::from(left)) {
            return;
        }
        let (hx, start, end) = if left {
            (x0, 0.0, FRAC_PI_2)
        } else {
            (x1, FRAC_PI_2, PI)
        };
        out.push(Stroke::Line(q(hx, d), q(hx, d + r)));
        out.push(Stroke::Arc {
            center: q(hx, d),
            radius: r,
            start: cabinet.angle + start,
            end: cabinet.angle + end,
        });
    };
    for item in items {
        let (x, _, iw, _) = item.rect;
        let (x0, x1) = (x_off + x, x_off + x + iw);
        match item.item {
            FaceItem::DoorLeft { .. } => swing(x0, x1, true, out),
            FaceItem::DoorRight { .. } => swing(x0, x1, false, out),
            FaceItem::DoorAuto { .. } => swing(x0, x1, (x0 + x1) / 2.0 < w / 2.0 - 1e-9, out),
            FaceItem::DoubleDoor { .. } => {
                let mid = (x0 + x1) / 2.0;
                swing(x0, mid, true, out);
                swing(mid, x1, false, out);
            }
            FaceItem::Drawer { .. } if fresh(x0, x1, 2) => {
                let o = DRAWER_OUT.min(d / 2.0);
                out.push(Stroke::Polyline(
                    vec![q(x0, d), q(x1, d), q(x1, d + o), q(x0, d + o)],
                    true,
                ));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    fn bbox(points: &[Point]) -> (f64, f64, f64, f64) {
        let xs = points.iter().map(|p| p.x);
        let ys = points.iter().map(|p| p.y);
        (
            xs.clone().fold(f64::MAX, f64::min),
            ys.clone().fold(f64::MAX, f64::min),
            xs.fold(f64::MIN, f64::max),
            ys.fold(f64::MIN, f64::max),
        )
    }

    #[test]
    fn outline_is_36_by_24_rectangle() {
        let c = Cabinet::base(36.0);
        let Stroke::Polyline(pts, true) = &plan_symbol(&c)[0] else {
            panic!("first stroke must be the closed outline");
        };
        assert_eq!(bbox(pts), (0.0, 0.0, 36.0, 24.0));
    }

    #[test]
    fn outline_rotates_ninety_degrees() {
        let mut c = Cabinet::base(36.0);
        c.position = Point::new(100.0, 100.0);
        c.angle = FRAC_PI_2;
        let Stroke::Polyline(pts, true) = &plan_symbol(&c)[0] else {
            panic!("first stroke must be the closed outline");
        };
        let (x0, y0, x1, y1) = bbox(pts);
        assert!((x0 - 76.0).abs() < 1e-9 && (x1 - 100.0).abs() < 1e-9);
        assert!((y0 - 100.0).abs() < 1e-9 && (y1 - 136.0).abs() < 1e-9);
        // Back-left corner stays at the position; width runs along +Y.
        assert!(pts[0].dist(Point::new(100.0, 100.0)) < 1e-9);
        assert!(pts[1].dist(Point::new(100.0, 136.0)) < 1e-9);
    }

    #[test]
    fn symbol_parts_per_kind() {
        let base = plan_symbol(&Cabinet::base(24.0));
        // outline, countertop outline, front line, label
        assert_eq!(base.len(), 4);
        let wall = plan_symbol(&Cabinet::wall(30.0));
        // outline, inner offset, two diagonals, label
        assert_eq!(wall.len(), 5);
        let Some(Stroke::Text { text, .. }) = wall.last() else {
            panic!("label last");
        };
        assert_eq!(text, "W3030");
    }

    fn arcs(strokes: &[Stroke]) -> Vec<&Stroke> {
        strokes
            .iter()
            .filter(|s| matches!(s, Stroke::Arc { .. }))
            .collect()
    }

    fn label_of(strokes: &[Stroke]) -> String {
        match strokes.last() {
            Some(Stroke::Text { text, .. }) => text.clone(),
            other => panic!("label last, got {other:?}"),
        }
    }

    #[test]
    fn corner_outlines_follow_the_l_and_the_diagonal() {
        let pie = plan_symbol(&Cabinet::corner_base(36.0).with_pie_cut(true));
        let Stroke::Polyline(pts, true) = &pie[0] else {
            panic!("outline first");
        };
        assert_eq!(pts.len(), 6);
        // Top outline at index 1, then both arm fronts and the lazy susan circle.
        assert!(matches!(&pie[1], Stroke::Polyline(t, true) if t.len() == 6));
        assert_eq!(arcs(&pie).len(), 1);
        let no_susan = plan_symbol(&Cabinet::corner_base(36.0).with_pie_cut(false));
        assert!(arcs(&no_susan).is_empty());
        let diag = plan_symbol(&Cabinet::corner_base(36.0));
        let Stroke::Polyline(pts, true) = &diag[0] else {
            panic!("outline first");
        };
        assert_eq!(pts.len(), 5);
        assert_eq!(label_of(&diag), "BDC36");
    }

    #[test]
    fn indicators_draw_door_swings_and_open_drawers() {
        let mut c = Cabinet::base(36.0);
        let off = plan_symbol(&c).len();
        c.indicators = true;
        let on = plan_symbol(&c);
        // The drawer outline, plus the door's swing line and arc.
        assert_eq!(on.len(), off + 3);
        let Some(Stroke::Arc {
            radius, start, end, ..
        }) = arcs(&on).first().copied().cloned()
        else {
            panic!("a swing arc");
        };
        // The door fills the 33" between the stiles; a quarter turn.
        assert!((radius - 33.0).abs() < 1e-9, "{radius}");
        assert!((end - start - FRAC_PI_2).abs() < 1e-9);
        // A double door gets two arcs that hinge on opposite ends.
        c.face = crate::face::FaceLayout::sink_base();
        let sink = plan_symbol(&c);
        assert_eq!(arcs(&sink).len(), 2);
        // Corner cabinets skip the indicators.
        let mut corner = Cabinet::corner_base(36.0);
        corner.indicators = true;
        assert!(arcs(&plan_symbol(&corner)).is_empty());
    }

    #[test]
    fn cutouts_draw_their_fixtures() {
        let mut c = Cabinet::base(36.0);
        let before = plan_symbol(&c).len();
        assert!(c.add_cutout(CutoutKind::Sink));
        // Hole outline, bowl, faucet.
        assert_eq!(plan_symbol(&c).len(), before + 3);
        let mut ck = Cabinet::base(36.0);
        assert!(ck.add_cutout(CutoutKind::Cooktop));
        assert_eq!(arcs(&plan_symbol(&ck)).len(), 4);
    }

    #[test]
    fn fillers_blind_bays_and_custom_tops() {
        let f = plan_symbol(&Cabinet::filler(CabinetKind::BaseFiller, 3.0));
        assert_eq!(label_of(&f), "BF3");
        // A 3" strip's label turns to run along it.
        let Some(Stroke::Text { angle, .. }) = f.last() else {
            panic!()
        };
        assert!((angle - FRAC_PI_2).abs() < 1e-9);
        let blind = plan_symbol(&Cabinet::blind_base(
            48.0,
            15.0,
            crate::cabinet::BlindSide::Left,
        ));
        assert!(blind
            .iter()
            .any(|s| matches!(s, Stroke::Line(a, b) if (a.x - 15.0).abs() < 1e-9 && (b.x - 15.0).abs() < 1e-9)));
        let dw = plan_symbol(&Cabinet::dishwasher_opening());
        assert_eq!(label_of(&dw), "DW24");
        assert_eq!(
            dw.iter().filter(|s| matches!(s, Stroke::Line(..))).count(),
            2
        );
        let ring = [
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(60.0, 30.0),
            Point::new(0.0, 30.0),
        ];
        let mut ct = Cabinet::custom_countertop(&ring, 1.5, 36.0).unwrap();
        let plain = plan_symbol(&ct).len();
        ct.custom.as_mut().unwrap().edge = EdgeProfile::Beveled;
        assert_eq!(plan_symbol(&ct).len(), plain + 1);
        assert_eq!(label_of(&plan_symbol(&ct)), "CT");
        let bs = Cabinet::custom_backsplash(&[Point::ZERO, Point::new(48.0, 0.0)], 4.0, 0.5, 36.0)
            .unwrap();
        let Stroke::Polyline(strip, true) = &plan_symbol(&bs)[0] else {
            panic!()
        };
        assert!((geom::area(strip) - 24.0).abs() < 1e-9);
    }
}
